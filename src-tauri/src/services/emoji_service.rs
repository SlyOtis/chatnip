use crate::content::emoji::{self, Platform};
use crate::models::app_setting::{self, AppSetting};
use crate::models::content::{extension_for, sniff_mime, PostContentInput, WorkspaceEmoji};
use crate::providers::models::{Provider, ProviderAuth, CURSOR_EMOJI};
use crate::providers::traits::ProviderTrait;
use crate::services::attachment_service::safe_name;
use chrono::Utc;
use sqlx::{Pool, Sqlite};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

pub const EVENT: &str = "workspace-emoji-updated";
pub const FOLDER: &str = "emoji";

pub fn relative_path(provider_id: &str, name: &str, mime_type: &str) -> String {
    format!("assets/{}/{}/{}.{}", FOLDER, safe_name(provider_id), safe_name(name), extension_for(mime_type))
}

pub async fn sync(app_data: &Path, pool: &Pool<Sqlite>, provider: &mut Provider, client: Arc<dyn ProviderTrait>, auth: &ProviderAuth) -> Result<bool, String> {
    let interval = AppSetting::get_u64(pool, app_setting::EMOJI_SYNC_INTERVAL_SECONDS).await?;
    let now = Utc::now();
    if !provider.due(CURSOR_EMOJI, interval, now) {
        return Ok(false);
    }
    let remote = client.get_custom_emoji(auth).await?;
    let fetch = WorkspaceEmoji::put_workspace(pool, &provider.id, &remote).await?;
    for name in fetch {
        let Some(url) = remote.iter().find(|e| e.name == name).and_then(|e| e.url.clone()) else { continue };
        let input = PostContentInput { data: url.starts_with("data:").then(|| url.clone()), source_url: Some(url.clone()), ..PostContentInput::default() };
        let bytes = match crate::services::content_service::fetch_bytes(&input).await {
            Ok(bytes) if !bytes.is_empty() => bytes,
            Ok(_) => continue,
            Err(e) => {
                eprintln!("Emoji :{}: of {} was not cached: {}", name, provider.name, e);
                continue;
            }
        };
        let mime = sniff_mime(&bytes).unwrap_or("image/png");
        let relative = relative_path(&provider.id, &name, mime);
        let target = app_data.join(&relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Could not make the emoji folder: {}", e))?;
        }
        std::fs::write(&target, &bytes).map_err(|e| format!("Could not keep the emoji :{}:: {}", name, e))?;
        WorkspaceEmoji::put_path(pool, &provider.id, &name, &relative, mime).await?;
    }
    provider.put_cursors(pool, &HashMap::from([(CURSOR_EMOJI.to_string(), now.to_rfc3339())])).await?;
    Ok(true)
}

pub async fn get_all(pool: &Pool<Sqlite>, app_data: &Path, profile_id: &str) -> Result<Vec<WorkspaceEmoji>, String> {
    let mut all = WorkspaceEmoji::all(pool, profile_id).await?;
    let snapshot = all.clone();
    for item in all.iter_mut() {
        let own = match &item.alias_for {
            Some(target) => snapshot.iter().find(|e| e.provider_id == item.provider_id && e.name == *target),
            None => None,
        };
        let path = own.and_then(|e| e.path.clone()).or_else(|| item.path.clone());
        item.file_path = path.map(|p| app_data.join(p)).filter(|p| p.is_file()).map(|p| p.display().to_string());
        if item.url.is_none() {
            item.url = item.image_url(&snapshot);
        }
    }
    Ok(all)
}

pub struct Outbound {
    pub own: HashSet<String>,
    pub images: HashMap<String, String>,
}

pub async fn outbound_sets(pool: &Pool<Sqlite>, provider: &Provider) -> Result<Outbound, String> {
    let all = WorkspaceEmoji::all(pool, &provider.profile_id).await?;
    let own: HashSet<String> = all.iter().filter(|e| e.provider_id == provider.id).map(|e| e.name.clone()).collect();
    let mut images: HashMap<String, String> = HashMap::new();
    for item in all.iter().filter(|e| e.provider_id != provider.id).chain(all.iter().filter(|e| e.provider_id == provider.id)) {
        if let Some(url) = item.image_url(&all).filter(|u| u.starts_with("http")) {
            images.insert(item.name.clone(), url);
        }
    }
    Ok(Outbound { own, images })
}

pub async fn outbound(pool: &Pool<Sqlite>, provider: &Provider, platform: Platform, content: &str, content_type: &str) -> Result<String, String> {
    if emoji::custom_names(content).is_empty() && platform == Platform::Slack {
        return Ok(content.to_string());
    }
    let sets = outbound_sets(pool, provider).await?;
    Ok(emoji::outbound(content, content_type, platform, &sets.own, &sets.images))
}

pub fn platform(provider: &Provider, conversation_external_id: &str, is_mail: bool) -> Platform {
    if provider.provider_type == "mock" {
        return match provider.config.as_deref().and_then(|c| crate::providers::mock::MockKind::parse(c).ok()) {
            Some(crate::providers::mock::MockKind::Slack) => Platform::Slack,
            Some(crate::providers::mock::MockKind::Teams) if !is_mail => Platform::Teams,
            Some(crate::providers::mock::MockKind::GoogleChat) if !is_mail => Platform::GoogleChat,
            _ => Platform::Mail,
        };
    }
    Platform::of(&provider.provider_type, conversation_external_id, is_mail)
}

pub async fn put_draft(pool: &Pool<Sqlite>, provider: &Provider, conversation_external_id: &str, draft: &mut crate::providers::traits::OutgoingMessage) -> Result<String, String> {
    let original = draft.content.clone();
    let is_mail = draft.subject.is_some() || !draft.to.is_empty();
    let platform = platform(provider, conversation_external_id, is_mail);
    draft.content = outbound(pool, provider, platform, &draft.content, &draft.content_type).await?;
    if let Some(html) = draft.html.take() {
        draft.html = Some(outbound(pool, provider, platform, &html, "html").await?);
    }
    Ok(original)
}

pub fn put_sent(sent: &mut crate::providers::traits::ResponseMessage, mapped: &str, original: String) {
    if sent.content == mapped {
        sent.content = original;
    } else {
        emoji::canonical_message(sent);
    }
}

pub async fn reaction(pool: &Pool<Sqlite>, provider: &Provider, reaction: &str) -> Result<String, String> {
    let canonical = emoji::canonical_reaction(reaction);
    let name = emoji::reaction_name(&canonical);
    if emoji::is_standard(&name) || emoji::is_standard(name.split("::").next().unwrap_or(&name)) {
        return Ok(canonical);
    }
    let sets = outbound_sets(pool, provider).await?;
    if provider.provider_type == "slack" && sets.own.contains(&name) {
        return Ok(canonical);
    }
    if provider.provider_type == "mock" {
        return Ok(canonical);
    }
    Err(format!("{} has no :{}: emoji; react with a standard one", provider.name, name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::traits::RemoteEmoji;

    async fn workspace(pool: &Pool<Sqlite>, profile: &str, kind: &str, name: &str) -> Provider {
        let id = crate::test_helpers::test_provider(pool, profile, kind).await;
        sqlx::query("UPDATE provider SET name = ?1, namespace = ?1 WHERE id = ?2").bind(name).bind(&id).execute(pool).await.unwrap();
        Provider::load_from_db(pool, &id).await.unwrap()
    }

    #[tokio::test]
    async fn custom_emoji_are_stored_per_workspace_with_aliases_and_resolved_for_sending() {
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        let northwind = workspace(&pool, &profile, "slack", "Northwind").await;
        let sundial = workspace(&pool, &profile, "slack", "Sundial").await;
        let teams = workspace(&pool, &profile, "microsoft", "Tidewater").await;
        let parrot = RemoteEmoji { name: "party_parrot".into(), url: Some("https://emoji.slack-edge.com/T1/party_parrot/a.gif".into()), alias_for: None };
        let alias = RemoteEmoji { name: "partyparrot".into(), url: None, alias_for: Some("party_parrot".into()) };
        let fetch = WorkspaceEmoji::put_workspace(&pool, &northwind.id, &[parrot.clone(), alias]).await.unwrap();
        assert_eq!(fetch, vec!["party_parrot".to_string()]);
        WorkspaceEmoji::put_path(&pool, &northwind.id, "party_parrot", &relative_path(&northwind.id, "party_parrot", "image/gif"), "image/gif").await.unwrap();
        assert!(WorkspaceEmoji::put_workspace(&pool, &northwind.id, &[parrot.clone(), RemoteEmoji { name: "partyparrot".into(), url: None, alias_for: Some("party_parrot".into()) }]).await.unwrap().is_empty());

        let all = WorkspaceEmoji::all(&pool, &profile).await.unwrap();
        assert_eq!(all.iter().map(|e| (e.provider_name.as_str(), e.name.as_str())).collect::<Vec<_>>(), vec![("Northwind", "party_parrot"), ("Northwind", "partyparrot")]);
        assert_eq!(all[1].image_url(&all).as_deref(), Some("https://emoji.slack-edge.com/T1/party_parrot/a.gif"));

        assert_eq!(reaction(&pool, &northwind, "party_parrot").await.unwrap(), ":party_parrot:");
        assert!(reaction(&pool, &sundial, ":party_parrot:").await.unwrap_err().contains("Sundial has no :party_parrot:"));
        assert_eq!(reaction(&pool, &teams, "👍").await.unwrap(), ":+1:");
        assert_eq!(reaction(&pool, &sundial, ":+1::skin-tone-2:").await.unwrap(), ":+1::skin-tone-2:");

        assert_eq!(outbound(&pool, &northwind, Platform::Slack, "Takk :party_parrot:", "markdown").await.unwrap(), "Takk :party_parrot:");
        assert_eq!(outbound(&pool, &sundial, Platform::Slack, "Takk :partyparrot:", "markdown").await.unwrap(), "Takk ![:partyparrot:](https://emoji.slack-edge.com/T1/party_parrot/a.gif)");
        assert_eq!(outbound(&pool, &teams, Platform::Teams, "Takk :party_parrot: :tada:", "text").await.unwrap(), "Takk :party_parrot: 🎉");

        WorkspaceEmoji::put_workspace(&pool, &northwind.id, &[parrot]).await.unwrap();
        assert_eq!(WorkspaceEmoji::get_workspace(&pool, &northwind.id).await.unwrap().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_mock_slack_workspace_serves_custom_emoji_that_sync_into_the_local_cache_on_an_interval() {
        use crate::providers::mock::{MockKind, MockProvider};
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        AppSetting::put(&pool, app_setting::MOCK_ENABLED, "true").await.unwrap();
        let id = crate::test_helpers::test_provider(&pool, &profile, "mock").await;
        sqlx::query("UPDATE provider SET config = 'slack', namespace = 'slack', name = 'Mock Slack' WHERE id = ?1").bind(&id).execute(&pool).await.unwrap();
        let mut provider = Provider::load_from_db(&pool, &id).await.unwrap();
        let client: Arc<dyn ProviderTrait> = Arc::new(MockProvider::new(MockKind::Slack, pool.clone()));
        let auth = ProviderAuth::mock(&id);
        let dir = tempfile::tempdir().unwrap();
        assert!(sync(dir.path(), &pool, &mut provider, client.clone(), &auth).await.unwrap());
        let all = get_all(&pool, dir.path(), &profile).await.unwrap();
        let names: Vec<&str> = all.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["chatnip", "party-parrot", "party_parrot", "partyparrot", "shipit"]);
        let alias = all.iter().find(|e| e.name == "partyparrot").unwrap();
        assert_eq!(alias.alias_for.as_deref(), Some("party_parrot"));
        assert!(alias.file_path.as_deref().unwrap().ends_with("party_parrot.gif"));
        assert_eq!(std::fs::read(alias.file_path.as_deref().unwrap()).unwrap()[..6], *b"GIF89a");
        assert!(!sync(dir.path(), &pool, &mut provider, client.clone(), &auth).await.unwrap());
        AppSetting::put(&pool, app_setting::EMOJI_SYNC_INTERVAL_SECONDS, "1").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        assert!(sync(dir.path(), &pool, &mut provider, client, &auth).await.unwrap());
        let teams: Arc<dyn ProviderTrait> = Arc::new(MockProvider::new(MockKind::Teams, pool.clone()));
        assert!(teams.get_custom_emoji(&auth).await.unwrap().is_empty());
    }
}
