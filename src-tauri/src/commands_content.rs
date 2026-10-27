use crate::models::content::{ContentGroup, ContentGroupItemsInput, ContentItem, ContentTag, PostContentGroupInput, PostContentInput, PutContentBulkInput, PutContentGroupInput, PutContentInput, ReactionUse, SORT_RELEVANCE};
use crate::services::content_service::{self, ContentUse, GifPage, PostContentRenderInput, RenderedContent};
use crate::ChatnipService;
use std::collections::BTreeMap;
use tauri::{AppHandle, Manager, State};

pub(crate) fn app_data(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))
}

#[tauri::command]
pub async fn get_gifs(service: State<'_, ChatnipService>, app: AppHandle, query: String, limit: Option<u32>, cursor: Option<BTreeMap<String, String>>) -> Result<GifPage, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await.ok();
    let mut page = content_service::search_gifs(service.db(), profile_id.as_deref(), &query, limit.unwrap_or(24), cursor).await?;
    let dir = app_data(&app)?;
    for result in page.results.iter_mut().filter(|r| r.source == "library") {
        let file = dir.join(&result.url).to_string_lossy().to_string();
        result.url = file.clone();
        result.preview_url = file;
    }
    Ok(page)
}

#[tauri::command]
pub async fn get_content(service: State<'_, ChatnipService>, app: AppHandle, kind: Option<String>, query: Option<String>, sort: Option<String>) -> Result<Vec<ContentItem>, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    let dir = app_data(&app)?;
    let items = match sort.as_deref().filter(|s| !s.is_empty()) {
        Some(sort) => ContentItem::search(service.db(), &profile_id, kind.as_deref(), query.as_deref(), sort).await,
        None if query.as_deref().is_some_and(|q| !q.trim().is_empty()) => ContentItem::search(service.db(), &profile_id, kind.as_deref(), query.as_deref(), SORT_RELEVANCE).await,
        None => ContentItem::library(service.db(), &profile_id, kind.as_deref(), None).await,
    }?;
    Ok(items.into_iter().map(|i| i.located(&dir)).collect())
}

#[tauri::command]
pub async fn post_content(service: State<'_, ChatnipService>, app: AppHandle, input: PostContentInput) -> Result<ContentItem, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    let dir = app_data(&app)?;
    Ok(content_service::store(service.db(), &dir, &profile_id, input).await?.located(&dir))
}

#[tauri::command]
pub async fn put_content(service: State<'_, ChatnipService>, app: AppHandle, input: PutContentInput) -> Result<ContentItem, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    content_service::owned(service.db(), &profile_id, &input.content_id).await?;
    Ok(ContentItem::update(service.db(), &input).await?.located(&app_data(&app)?))
}

#[tauri::command]
pub async fn put_content_order(service: State<'_, ChatnipService>, content_ids: Vec<String>) -> Result<(), String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    ContentItem::put_order(service.db(), &profile_id, &content_ids).await
}

#[tauri::command]
pub async fn delete_content(service: State<'_, ChatnipService>, app: AppHandle, content_id: String) -> Result<(), String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    content_service::owned(service.db(), &profile_id, &content_id).await?;
    if let Some(item) = ContentItem::delete(service.db(), &content_id).await? {
        content_service::remove_file(&app_data(&app)?, &item);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_content_tags(service: State<'_, ChatnipService>) -> Result<Vec<ContentTag>, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    ContentItem::tags(service.db(), &profile_id).await
}

#[tauri::command]
pub async fn put_content_bulk(service: State<'_, ChatnipService>, app: AppHandle, input: PutContentBulkInput) -> Result<Vec<ContentItem>, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    let dir = app_data(&app)?;
    Ok(ContentItem::put_bulk(service.db(), &profile_id, &input).await?.into_iter().map(|i| i.located(&dir)).collect())
}

#[tauri::command]
pub async fn post_content_from_message(service: State<'_, ChatnipService>, app: AppHandle, message_id: String, attachment_index: Option<usize>) -> Result<ContentItem, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    let dir = app_data(&app)?;
    Ok(content_service::save_from_message(service.db(), &dir, &profile_id, &message_id, attachment_index).await?.located(&dir))
}

#[tauri::command]
pub async fn post_content_from_url(service: State<'_, ChatnipService>, app: AppHandle, url: String, name: Option<String>, tags: Option<Vec<String>>, keywords: Option<Vec<String>>) -> Result<ContentItem, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    let input = PostContentInput { kind: "auto".into(), name: name.unwrap_or_default(), source_url: Some(url), tags, keywords, ..PostContentInput::default() };
    let dir = app_data(&app)?;
    Ok(content_service::store(service.db(), &dir, &profile_id, input).await?.located(&dir))
}

#[tauri::command]
pub async fn post_content_send(service: State<'_, ChatnipService>, app: AppHandle, content_id: String, conversation_id: String, provider_id: Option<String>) -> Result<crate::models::Message, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    content_service::send(service.db(), &app_data(&app)?, &profile_id, &content_id, &conversation_id, provider_id.as_deref()).await
}

#[tauri::command]
pub async fn post_content_render(service: State<'_, ChatnipService>, input: PostContentRenderInput) -> Result<RenderedContent, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    content_service::render(service.db(), &profile_id, &input).await
}

#[tauri::command]
pub async fn get_content_uses(service: State<'_, ChatnipService>, content_id: String) -> Result<Vec<ContentUse>, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    let item = content_service::owned(service.db(), &profile_id, &content_id).await?;
    content_service::uses(service.db(), &profile_id, &item).await
}

#[tauri::command]
pub async fn get_content_groups(service: State<'_, ChatnipService>) -> Result<Vec<ContentGroup>, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    ContentGroup::ensure_defaults(service.db(), &profile_id).await?;
    ContentGroup::all(service.db(), &profile_id).await
}

#[tauri::command]
pub async fn post_content_group(service: State<'_, ChatnipService>, input: PostContentGroupInput) -> Result<ContentGroup, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    ContentGroup::post(service.db(), &profile_id, &input).await
}

#[tauri::command]
pub async fn put_content_group(service: State<'_, ChatnipService>, input: PutContentGroupInput) -> Result<ContentGroup, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    ContentGroup::put(service.db(), &profile_id, &input).await
}

#[tauri::command]
pub async fn delete_content_group(service: State<'_, ChatnipService>, content_group_id: String) -> Result<(), String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    ContentGroup::delete(service.db(), &profile_id, &content_group_id).await
}

#[tauri::command]
pub async fn put_content_group_items(service: State<'_, ChatnipService>, input: ContentGroupItemsInput) -> Result<ContentGroup, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    ContentGroup::put_items(service.db(), &profile_id, &input.content_group_id, &input.content_ids, input.position).await
}

#[tauri::command]
pub async fn delete_content_group_items(service: State<'_, ChatnipService>, input: ContentGroupItemsInput) -> Result<ContentGroup, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    ContentGroup::remove_items(service.db(), &profile_id, &input.content_group_id, &input.content_ids).await
}

#[tauri::command]
pub async fn get_reaction_uses(service: State<'_, ChatnipService>, limit: Option<i64>) -> Result<Vec<ReactionUse>, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    ReactionUse::all(service.db(), &profile_id, limit.unwrap_or(24).clamp(1, 200)).await
}

#[tauri::command]
pub async fn get_workspace_emoji(service: State<'_, ChatnipService>, app: AppHandle) -> Result<Vec<crate::models::content::WorkspaceEmoji>, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    crate::services::emoji_service::get_all(service.db(), &app_data(&app)?, &profile_id).await
}
