use crate::services::voice_service::{self, VoiceSetup};
use crate::ChatnipService;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn get_voice_setup(app: AppHandle, service: State<'_, ChatnipService>) -> Result<VoiceSetup, String> {
    voice_service::get_voice_setup(&app, service.db()).await
}

#[tauri::command]
pub async fn post_voice_setup(app: AppHandle, force: Option<bool>) -> Result<(), String> {
    voice_service::post_voice_setup(app, force.unwrap_or(false))
}

#[tauri::command]
pub async fn post_voice(app: AppHandle, service: State<'_, ChatnipService>, conversation_id: Option<String>) -> Result<String, String> {
    voice_service::post_voice(app, service.db().clone(), conversation_id).await
}

#[tauri::command]
pub async fn delete_voice(voice_id: String) -> Result<String, String> {
    voice_service::delete_voice(&voice_id)
}
