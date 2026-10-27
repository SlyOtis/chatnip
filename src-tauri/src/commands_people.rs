use crate::models::object_override::{DeleteObjectOverrideInput, PutObjectOverrideInput};
use crate::models::{app_setting, AppSetting, FavouriteOrderItem, Message, ObjectOverride, Person, PostIdentityInput, Profile, PutPersonInput};
use crate::providers::mock::MockKind;
use crate::providers::models::{Provider, ProviderAuth};
use crate::services::link_service::{self, SharedLink};
use crate::services::message_service::{self, GetSharedInput, SharedFile};
use crate::services::{connection_service, people_service, MessageLoadOptions, MessageService};
use crate::ChatnipService;
use chrono::{Duration, Utc};
use tauri::{AppHandle, Emitter, State, Window};

async fn emit_people(service: &ChatnipService, window: &Window) -> Result<(), String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let people = Person::all(service.db(), &profile).await?;
    window.emit("people-updated", &people).map_err(|e| format!("Failed to emit people: {}", e))?;
    Ok(())
}

async fn emit_conversations(window: &Window) -> Result<(), String> {
    use tauri::Manager;
    let conversations = crate::commands::get_conversations(window.state::<ChatnipService>()).await?;
    window.emit("conversations-updated", &conversations).map_err(|e| format!("Failed to emit conversations: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn get_people(service: State<'_, ChatnipService>) -> Result<Vec<Person>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    Person::all(service.db(), &profile).await
}

#[tauri::command]
pub async fn get_person(service: State<'_, ChatnipService>, person_id: String) -> Result<Person, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    Person::get(service.db(), &profile, &person_id).await
}

#[tauri::command]
pub async fn get_person_messages(service: State<'_, ChatnipService>, person_id: String, limit: Option<u32>) -> Result<Vec<Message>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let person = Person::get(service.db(), &profile, &person_id).await?;
    get_conversation_messages(service, person.conversation_ids, limit).await
}

#[tauri::command]
pub async fn get_conversation_messages(service: State<'_, ChatnipService>, conversation_ids: Vec<String>, limit: Option<u32>) -> Result<Vec<Message>, String> {
    let loader = MessageService::new(service.db().clone());
    let mut messages: Vec<Message> = Vec::new();
    for conversation_id in &conversation_ids {
        let loaded = loader
            .load_conversation_messages(conversation_id, MessageLoadOptions { load_reactions: true, load_assets: true, load_sender_details: true, limit: None, offset: None })
            .await?;
        messages.extend(loaded.into_iter().map(|m| m.message));
    }
    messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    if let Some(limit) = limit {
        let keep = messages.len().saturating_sub(limit as usize);
        messages.drain(..keep);
    }
    Ok(messages)
}

#[tauri::command]
pub async fn get_merge_group_messages(service: State<'_, ChatnipService>, conversation_id: String, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<Message>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    MessageService::new(service.db().clone()).get_merge_group_messages(&profile, &conversation_id, limit.unwrap_or(200), offset.unwrap_or(0)).await
}

#[tauri::command]
pub async fn get_shared_files(service: State<'_, ChatnipService>, input: GetSharedInput) -> Result<Vec<SharedFile>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    message_service::get_shared_files(service.db(), &profile, &input).await
}

#[tauri::command]
pub async fn get_shared_links(service: State<'_, ChatnipService>, input: GetSharedInput) -> Result<Vec<SharedLink>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    link_service::get_shared_links(service.db(), &profile, &input).await
}

#[tauri::command]
pub async fn put_person(service: State<'_, ChatnipService>, window: Window, input: PutPersonInput) -> Result<Person, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    Person::put(service.db(), &input).await?;
    emit_people(&service, &window).await?;
    Person::get(service.db(), &profile, &input.person_id).await
}

#[tauri::command]
pub async fn post_person_merge(service: State<'_, ChatnipService>, window: Window, source_person_id: String, target_person_id: String) -> Result<Person, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    Person::merge(service.db(), &source_person_id, &target_person_id).await?;
    emit_people(&service, &window).await?;
    emit_conversations(&window).await?;
    Person::get(service.db(), &profile, &target_person_id).await
}

#[tauri::command]
pub async fn delete_person_identity(service: State<'_, ChatnipService>, window: Window, identity_id: String) -> Result<Person, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let new_id = Person::split(service.db(), &identity_id).await?;
    emit_people(&service, &window).await?;
    emit_conversations(&window).await?;
    Person::get(service.db(), &profile, &new_id).await
}

#[tauri::command]
pub async fn post_person_identity(service: State<'_, ChatnipService>, window: Window, input: PostIdentityInput) -> Result<Person, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    people_service::post_identity(service.db(), &profile, &input).await?;
    emit_people(&service, &window).await?;
    emit_conversations(&window).await?;
    Person::get(service.db(), &profile, &input.person_id).await
}

#[tauri::command]
pub async fn put_people_match(service: State<'_, ChatnipService>, window: Window) -> Result<usize, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let count = people_service::rematch(service.db(), &profile).await?;
    emit_people(&service, &window).await?;
    emit_conversations(&window).await?;
    Ok(count)
}

#[tauri::command]
pub async fn put_favourites(service: State<'_, ChatnipService>, window: Window, order: Vec<FavouriteOrderItem>) -> Result<(), String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    Person::put_favourites(service.db(), &profile, &order).await?;
    emit_people(&service, &window).await?;
    emit_conversations(&window).await
}

pub fn mock_forced() -> Option<bool> {
    match std::env::var("CHATNIP_USE_DEBUG").map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Ok("1" | "true") => Some(true),
        Ok("0" | "false") => Some(false),
        _ => None,
    }
}

pub async fn mock_enabled(pool: &sqlx::Pool<sqlx::Sqlite>) -> bool {
    if let Some(forced) = mock_forced() {
        return forced;
    }
    AppSetting::get_or(pool, app_setting::MOCK_ENABLED, "false").await.map(|v| v == "true").unwrap_or(false)
}

#[tauri::command]
pub async fn post_mock_connection(app: AppHandle, service: State<'_, ChatnipService>, kind: String) -> Result<Provider, String> {
    if !mock_enabled(service.db()).await {
        return Err("Mock clients are switched off. Enable them under Settings before connecting.".to_string());
    }
    let kind = MockKind::parse(&kind)?;
    let auth = ProviderAuth {
        access_token: "mock".to_string(),
        refresh_token: None,
        expires_at: Utc::now() + Duration::days(36500),
        token_type: "Mock".to_string(),
        scope: None,
    };
    let provider = connection_service::complete_auth(&app, service.db(), "mock", auth, Some(kind.key().to_string())).await?;
    let account = crate::providers::mock::server::account(kind, &ProviderAuth::mock(&provider.id));
    crate::providers::mock::server::put_state(service.db(), &account, kind, crate::providers::mock::server::STATE_OK).await?;
    Ok(provider)
}

#[tauri::command]
pub async fn put_mock_state(app: AppHandle, service: State<'_, ChatnipService>, provider_id: String, state: String) -> Result<MockMode, String> {
    if !mock_enabled(service.db()).await {
        return Err("Mock clients are switched off. Enable them under Settings first.".to_string());
    }
    let provider = Provider::load_from_db(service.db(), &provider_id).await?;
    let kind = MockKind::parse(provider.config.as_deref().unwrap_or_default())?;
    let auth = provider.auth.as_ref().ok_or_else(|| format!("{} has no mock token", provider.name))?;
    crate::providers::mock::server::put_state(service.db(), &crate::providers::mock::server::account(kind, auth), kind, &state).await?;
    connection_service::emit_providers(&app, service.db()).await?;
    get_mock_mode(service).await
}

#[tauri::command]
pub async fn get_my_user_ids(service: State<'_, ChatnipService>) -> Result<Vec<String>, String> {
    crate::models::Profile::get_own_user_ids(service.db(), &crate::models::Profile::get_active_id(service.db()).await?).await
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MockMode {
    pub enabled: bool,
    pub forced: Option<bool>,
    pub states: std::collections::HashMap<String, String>,
}

#[tauri::command]
pub async fn get_mock_mode(service: State<'_, ChatnipService>) -> Result<MockMode, String> {
    let mut states = std::collections::HashMap::new();
    if let Some(profile) = Profile::get_active(service.db()).await? {
        for (provider_id, account) in crate::providers::mock::server::accounts(service.db(), &profile.id).await? {
            states.insert(provider_id, crate::providers::mock::server::get_state(service.db(), &account).await?);
        }
    }
    Ok(MockMode { enabled: mock_enabled(service.db()).await, forced: mock_forced(), states })
}

#[tauri::command]
pub async fn post_mock_seed(app: AppHandle, service: State<'_, ChatnipService>) -> Result<crate::providers::mock::seed::SeedReport, String> {
    if !mock_enabled(service.db()).await {
        return Err("Mock clients are switched off. Set CHATNIP_USE_DEBUG=true or enable them under Settings.".to_string());
    }
    let profile = Profile::get_active(service.db()).await?.ok_or_else(|| "No active profile".to_string())?;
    let app_data = tauri::Manager::path(&app).app_data_dir().ok();
    crate::providers::mock::seed::seed(service.db(), &profile.id, app_data.as_deref()).await
}

#[tauri::command]
pub async fn get_person_profile(service: State<'_, ChatnipService>, person_id: String) -> Result<people_service::PersonProfile, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    people_service::person_profile(service.db(), &profile, &person_id).await
}


#[tauri::command]
pub async fn get_object_overrides(service: State<'_, ChatnipService>) -> Result<Vec<ObjectOverride>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    ObjectOverride::get_all(service.db(), &profile).await
}

#[tauri::command]
pub async fn put_object_override(service: State<'_, ChatnipService>, window: Window, input: PutObjectOverrideInput) -> Result<Vec<ObjectOverride>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    ObjectOverride::put(service.db(), &input).await?;
    emit_people(&service, &window).await?;
    emit_conversations(&window).await?;
    ObjectOverride::get_all(service.db(), &profile).await
}

#[tauri::command]
pub async fn delete_object_override(service: State<'_, ChatnipService>, window: Window, input: DeleteObjectOverrideInput) -> Result<Vec<ObjectOverride>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    ObjectOverride::delete(service.db(), &input).await?;
    emit_people(&service, &window).await?;
    emit_conversations(&window).await?;
    ObjectOverride::get_all(service.db(), &profile).await
}
