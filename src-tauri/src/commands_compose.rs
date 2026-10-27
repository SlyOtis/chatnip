use crate::models::content::{ContentItem, ContentSignature};
use crate::models::draft::{Draft, PutDraftInput};
use crate::models::reminder::{PostReminderInput, Reminder, STATUS_CANCELLED, STATUS_PENDING};
use crate::models::Profile;
use crate::providers::traits::{RemoteAlias, RemoteAutoReply};
use crate::services::content_service::{self, RenderedSignature};
use crate::services::outbound_service;
use crate::ChatnipService;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Deserialize)]
pub struct GetSignatureInput {
    pub provider_id: String,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub content_id: Option<String>,
}

#[tauri::command]
pub async fn get_drafts(service: State<'_, ChatnipService>, conversation_id: Option<String>) -> Result<Vec<Draft>, String> {
    Draft::get_all(service.db(), &Profile::get_active_id(service.db()).await?, conversation_id.as_deref()).await
}

#[tauri::command]
pub async fn put_draft(app: AppHandle, service: State<'_, ChatnipService>, input: PutDraftInput) -> Result<Draft, String> {
    let profile_id = Profile::get_active_id(service.db()).await?;
    let draft = service.with_write_lock(move |pool| async move { Draft::put(&pool, &profile_id, &input).await }).await?;
    let pool = service.db().clone();
    let mirrored = draft.clone();
    tauri::async_runtime::spawn(async move {
        match outbound_service::put_remote_draft(&pool, &mirrored).await {
            Ok(remote) if remote != mirrored.remote_id => {
                if let Err(e) = Draft::put_remote_id(&pool, &mirrored.id, remote.as_deref()).await {
                    eprintln!("The draft was not linked to the provider: {}", e);
                }
            }
            Ok(_) => {}
            Err(e) => eprintln!("The draft was not mirrored to the provider: {}", e),
        }
    });
    app.emit("drafts-updated", &draft.conversation_id).ok();
    Ok(draft)
}

#[tauri::command]
pub async fn delete_draft(app: AppHandle, service: State<'_, ChatnipService>, conversation_id: String, thread_id: Option<String>) -> Result<(), String> {
    let profile_id = Profile::get_active_id(service.db()).await?;
    let thread = thread_id.unwrap_or_default();
    let target = conversation_id.clone();
    let removed = service.with_write_lock(move |pool| async move { Draft::delete(&pool, &profile_id, &target, &thread).await }).await?;
    if let Some(removed) = removed {
        if let Err(e) = outbound_service::delete_remote_draft(service.db(), &removed).await {
            eprintln!("The provider draft was not removed: {}", e);
        }
    }
    app.emit("drafts-updated", &conversation_id).ok();
    Ok(())
}

#[tauri::command]
pub async fn get_reminders(service: State<'_, ChatnipService>, pending: Option<bool>) -> Result<Vec<Reminder>, String> {
    Reminder::get_all(service.db(), &Profile::get_active_id(service.db()).await?, pending.unwrap_or(true)).await
}

#[tauri::command]
pub async fn post_reminder(app: AppHandle, service: State<'_, ChatnipService>, input: PostReminderInput) -> Result<Reminder, String> {
    let profile_id = Profile::get_active_id(service.db()).await?;
    let reminder = service.with_write_lock(move |pool| async move { Reminder::post(&pool, &profile_id, &input).await }).await?;
    app.emit("reminders-updated", &reminder.id).ok();
    Ok(reminder)
}

#[tauri::command]
pub async fn delete_reminder(app: AppHandle, service: State<'_, ChatnipService>, reminder_id: String) -> Result<bool, String> {
    let id = reminder_id.clone();
    let cancelled = service.with_write_lock(move |pool| async move { Reminder::put_status(&pool, &id, STATUS_PENDING, STATUS_CANCELLED).await }).await?;
    app.emit("reminders-updated", &reminder_id).ok();
    Ok(cancelled)
}

#[tauri::command]
pub async fn get_signature(service: State<'_, ChatnipService>, input: GetSignatureInput) -> Result<Option<RenderedSignature>, String> {
    let pool = service.db();
    let profile_id = Profile::get_active_id(pool).await?;
    let provider = crate::providers::models::Provider::load_from_db(pool, &input.provider_id).await?;
    let alias = input.alias.clone().or(provider.namespace.clone()).unwrap_or_default().to_lowercase();
    match &input.content_id {
        Some(content_id) => content_service::render_signature(pool, &profile_id, content_id, &provider.id, &alias, input.conversation_id.as_deref()).await.map(Some),
        None => content_service::get_signature(pool, &profile_id, &provider.id, &alias, input.mode.as_deref().unwrap_or(crate::models::content::SIGNATURE_NEW), input.conversation_id.as_deref()).await,
    }
}

#[tauri::command]
pub async fn get_signatures(service: State<'_, ChatnipService>, content_id: Option<String>) -> Result<Vec<ContentSignature>, String> {
    ContentSignature::get_all(service.db(), &Profile::get_active_id(service.db()).await?, content_id.as_deref()).await
}

#[tauri::command]
pub async fn put_signature(app: AppHandle, service: State<'_, ChatnipService>, input: ContentSignature) -> Result<Vec<ContentSignature>, String> {
    let pool = service.db();
    let profile_id = Profile::get_active_id(pool).await?;
    let item = content_service::owned(pool, &profile_id, &input.content_id).await?;
    if item.kind != crate::models::content::KIND_SIGNATURE {
        return Err(format!("{} is not a signature", item.name));
    }
    let binding = input.clone();
    service.with_write_lock(move |pool| async move { ContentSignature::put(&pool, &binding).await }).await?;
    if let Err(e) = content_service::put_signature_sync(pool, &profile_id, &input.content_id).await {
        eprintln!("The signature was not written to the provider: {}", e);
    }
    app.emit("signatures-updated", &input.content_id).ok();
    ContentSignature::get_all(pool, &profile_id, None).await
}

#[tauri::command]
pub async fn delete_signature(app: AppHandle, service: State<'_, ChatnipService>, content_id: String, provider_id: String, alias: Option<String>) -> Result<(), String> {
    let target = content_id.clone();
    service.with_write_lock(move |pool| async move { ContentSignature::delete(&pool, &target, &provider_id, alias.as_deref().unwrap_or_default()).await }).await?;
    app.emit("signatures-updated", &content_id).ok();
    Ok(())
}

#[tauri::command]
pub async fn post_signature_import(app: AppHandle, service: State<'_, ChatnipService>, provider_id: String) -> Result<Vec<ContentItem>, String> {
    let profile_id = Profile::get_active_id(service.db()).await?;
    let imported = service.with_write_lock(move |pool| async move { content_service::post_signature_import(&pool, &profile_id, &provider_id).await }).await?;
    app.emit("signatures-updated", "").ok();
    Ok(imported)
}

#[tauri::command]
pub async fn put_signature_sync(service: State<'_, ChatnipService>, content_id: String) -> Result<usize, String> {
    content_service::put_signature_sync(service.db(), &Profile::get_active_id(service.db()).await?, &content_id).await
}

#[tauri::command]
pub async fn get_aliases(service: State<'_, ChatnipService>, provider_id: String) -> Result<Vec<RemoteAlias>, String> {
    let (provider, client, auth) = outbound_service::client_of(service.db(), &provider_id).await?;
    if !provider.capabilities.supports_aliases {
        return Ok(Vec::new());
    }
    client.get_aliases(&auth).await
}

#[tauri::command]
pub async fn get_auto_reply(service: State<'_, ChatnipService>, provider_id: String) -> Result<Option<RemoteAutoReply>, String> {
    let (provider, client, auth) = outbound_service::client_of(service.db(), &provider_id).await?;
    if !provider.capabilities.supports_auto_reply {
        return Ok(None);
    }
    client.get_auto_reply(&auth).await
}

#[tauri::command]
pub async fn put_auto_reply(service: State<'_, ChatnipService>, provider_id: String, reply: RemoteAutoReply) -> Result<bool, String> {
    let (provider, client, auth) = outbound_service::client_of(service.db(), &provider_id).await?;
    if !provider.capabilities.supports_auto_reply {
        return Err(format!("{} has no automatic replies", provider.name));
    }
    client.put_auto_reply(&auth, &reply).await
}
