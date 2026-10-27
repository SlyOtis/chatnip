use crate::services::link_service::{self, LinkPreview};
use crate::commands_content::app_data;
use crate::ChatnipService;
use serde::Serialize;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Serialize)]
pub struct LinkPreviewEntry {
    pub url: String,
    pub preview: Option<LinkPreview>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn get_link_preview(service: State<'_, ChatnipService>, app: AppHandle, url: String) -> Result<Option<LinkPreview>, String> {
    link_service::preview(service.db(), &app_data(&app)?, &url).await
}

#[tauri::command]
pub async fn get_link_previews(service: State<'_, ChatnipService>, app: AppHandle, urls: Vec<String>) -> Result<Vec<LinkPreviewEntry>, String> {
    let dir = app_data(&app)?;
    let urls: Vec<String> = urls.into_iter().take(20).collect();
    let results = futures::future::join_all(urls.iter().map(|url| link_service::preview(service.db(), &dir, url))).await;
    Ok(urls
        .into_iter()
        .zip(results)
        .map(|(url, result)| match result {
            Ok(preview) => LinkPreviewEntry { url, preview, error: None },
            Err(error) => LinkPreviewEntry { url, preview: None, error: Some(error) },
        })
        .collect())
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GetMessageLinkInput {
    pub message_id: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GetLinkTargetInput {
    pub url: String,
}

#[tauri::command]
pub async fn get_message_link(service: State<'_, ChatnipService>, input: GetMessageLinkInput) -> Result<Option<String>, String> {
    link_service::get_message_link(service.db(), &input.message_id).await
}

#[tauri::command]
pub async fn get_link_target(service: State<'_, ChatnipService>, input: GetLinkTargetInput) -> Result<Option<link_service::LinkTarget>, String> {
    link_service::get_link_target(service.db(), &input.url).await
}
