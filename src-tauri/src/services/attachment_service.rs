use crate::models::content::PostContentInput;
use crate::models::Attachment;
use crate::services::content_service;
use sqlx::{Pool, Row, Sqlite};
use std::path::{Path, PathBuf};

pub const CACHE_FOLDER: &str = "attachments";

pub fn safe_name(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c == '/' || c == '\\' || c == ':' || c.is_control() { '_' } else { c }).collect();
    let trimmed = cleaned.trim().trim_start_matches('.').to_string();
    if trimmed.is_empty() {
        "attachment".to_string()
    } else {
        trimmed
    }
}

pub fn cache_root(cache: &Path) -> PathBuf {
    cache.join(CACHE_FOLDER)
}

pub fn cache_path(cache: &Path, message_id: &str, attachment: &Attachment) -> PathBuf {
    let folder = format!("{}-{}", safe_name(message_id), safe_name(&attachment.id));
    cache_root(cache).join(folder).join(safe_name(&attachment.name))
}

pub fn inside_cache(cache: &Path, path: &Path) -> bool {
    let root = cache_root(cache);
    match (root.canonicalize(), path.canonicalize()) {
        (Ok(root), Ok(path)) => path.starts_with(root),
        _ => false,
    }
}

pub async fn attachments(pool: &Pool<Sqlite>, message_id: &str) -> Result<Vec<Attachment>, String> {
    let row = sqlx::query("SELECT attachments FROM message WHERE id = ?1")
        .bind(message_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to load the message: {}", e))?
        .ok_or_else(|| format!("No message with id {}", message_id))?;
    Ok(crate::models::Message::parse_attachments(row.try_get::<Option<String>, _>("attachments").ok().flatten().as_deref()))
}

pub async fn bytes(pool: &Pool<Sqlite>, app_data: &Path, message_id: &str, attachment: &Attachment) -> Result<Vec<u8>, String> {
    if let Some(bytes) = content_service::asset_bytes(pool, app_data, message_id, &attachment.name).await {
        return Ok(bytes);
    }
    if let Some(bytes) = from_provider(pool, message_id, attachment).await? {
        return Ok(bytes);
    }
    let url = attachment.url.clone().filter(|u| !u.trim().is_empty()).ok_or_else(|| format!("{} has not been downloaded and has no link to fetch it from", attachment.name))?;
    let input = PostContentInput { data: url.starts_with("data:").then(|| url.clone()), source_url: Some(url), ..PostContentInput::default() };
    content_service::fetch_bytes(&input).await
}

async fn from_provider(pool: &Pool<Sqlite>, message_id: &str, attachment: &Attachment) -> Result<Option<Vec<u8>>, String> {
    if attachment.url.as_deref().is_some_and(|u| u.starts_with("data:") || u.starts_with("file://")) {
        return Ok(None);
    }
    let Ok((route, external_id)) = crate::services::outbound_service::route_for_message(pool, message_id).await else { return Ok(None) };
    if route.provider.provider_type == "mock" {
        return Ok(None);
    }
    let mut provider = route.provider;
    let client = crate::services::connection_service::provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = crate::services::connection_service::ensure_fresh(pool, &mut provider, client.clone()).await?;
    let remote = crate::providers::traits::ResponseAttachment { external_id: attachment.id.clone(), name: attachment.name.clone(), content_type: attachment.content_type.clone(), size: attachment.size as u64, url: attachment.url.clone(), content_id: attachment.content_id.clone() };
    client.get_attachment(&auth, &route.conversation_external_id, &external_id, &remote).await
}

pub async fn cached(pool: &Pool<Sqlite>, app_data: &Path, cache: &Path, message_id: &str, attachment: &Attachment) -> Result<PathBuf, String> {
    let path = cache_path(cache, message_id, attachment);
    if path.is_file() {
        return Ok(path);
    }
    let data = bytes(pool, app_data, message_id, attachment).await?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Could not make the cache folder: {}", e))?;
    }
    std::fs::write(&path, data).map_err(|e| format!("Could not keep {} in the cache: {}", attachment.name, e))?;
    Ok(path)
}

pub async fn cached_by_id(pool: &Pool<Sqlite>, app_data: &Path, cache: &Path, message_id: &str, attachment_id: &str) -> Result<PathBuf, String> {
    let attachment = attachments(pool, message_id).await?.into_iter().find(|a| a.id == attachment_id).ok_or_else(|| format!("The message has no attachment {}", attachment_id))?;
    cached(pool, app_data, cache, message_id, &attachment).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_stay_inside_their_folder() {
        assert_eq!(safe_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(safe_name("Kvittering 48213.pdf"), "Kvittering 48213.pdf");
        assert_eq!(safe_name(""), "attachment");
        let attachment = Attachment { id: "a/1".into(), name: "x.pdf".into(), content_type: "application/pdf".into(), size: 1, is_inline: false, content_id: None, url: None };
        let path = cache_path(Path::new("/tmp/cache"), "m1", &attachment);
        assert_eq!(path, PathBuf::from("/tmp/cache/attachments/m1-a_1/x.pdf"));
    }

    #[tokio::test]
    async fn data_links_land_in_the_cache_once() {
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        let provider = crate::test_helpers::test_provider(&pool, &profile, "mock").await;
        let conversation = crate::test_helpers::test_conversation(&pool, "Mail", "email").await;
        let message = crate::test_helpers::test_message(&pool, &conversation, &provider, "Kvittering").await;
        let attachments = serde_json::json!([{ "id": "a1", "name": "kvittering.txt", "content_type": "text/plain", "size": 5, "is_inline": false, "content_id": null, "url": "data:text/plain;base64,aGVpIQ==" }]);
        sqlx::query("UPDATE message SET attachments = ?1, has_attachments = 1 WHERE id = ?2").bind(attachments.to_string()).bind(&message).execute(&pool).await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = cached_by_id(&pool, dir.path(), dir.path(), &message, "a1").await.unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hei!");
        assert!(inside_cache(dir.path(), &path));
        assert!(!inside_cache(dir.path(), dir.path()));
    }
}
