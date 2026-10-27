use crate::models::inbox::{Inbox, PostInboxInput, PutInboxInput, PutInboxMembersInput};
use crate::ChatnipService;
use tauri::{Emitter, State, Window};

async fn emit_inboxes(service: &ChatnipService, window: &Window) -> Result<Vec<Inbox>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    Inbox::ensure_defaults(service.db(), &profile).await?;
    let inboxes = Inbox::all(service.db(), &profile).await?;
    window.emit("inboxes-updated", &inboxes).map_err(|e| format!("Failed to emit inboxes: {}", e))?;
    Ok(inboxes)
}

#[tauri::command]
pub async fn get_inboxes(service: State<'_, ChatnipService>) -> Result<Vec<Inbox>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    Inbox::ensure_defaults(service.db(), &profile).await?;
    Inbox::all(service.db(), &profile).await
}

#[tauri::command]
pub async fn post_inbox(service: State<'_, ChatnipService>, window: Window, input: PostInboxInput) -> Result<Inbox, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let created = Inbox::post(service.db(), &profile, &input).await?;
    emit_inboxes(&service, &window).await?;
    Ok(created)
}

#[tauri::command]
pub async fn put_inbox(service: State<'_, ChatnipService>, window: Window, input: PutInboxInput) -> Result<Inbox, String> {
    let updated = Inbox::put(service.db(), &input).await?;
    emit_inboxes(&service, &window).await?;
    Ok(updated)
}

#[tauri::command]
pub async fn put_inbox_order(service: State<'_, ChatnipService>, window: Window, order: Vec<String>) -> Result<Vec<Inbox>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    Inbox::put_order(service.db(), &profile, &order).await?;
    emit_inboxes(&service, &window).await
}

#[tauri::command]
pub async fn delete_inbox(service: State<'_, ChatnipService>, window: Window, inbox_id: String) -> Result<(), String> {
    Inbox::delete(service.db(), &inbox_id).await?;
    emit_inboxes(&service, &window).await?;
    Ok(())
}

#[tauri::command]
pub async fn put_inbox_members(service: State<'_, ChatnipService>, window: Window, input: PutInboxMembersInput) -> Result<Inbox, String> {
    let updated = Inbox::put_members(service.db(), &input).await?;
    emit_inboxes(&service, &window).await?;
    Ok(updated)
}

#[tauri::command]
pub async fn get_sidebar_layout(service: State<'_, ChatnipService>) -> Result<crate::models::sidebar::SidebarLayout, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    Inbox::ensure_defaults(service.db(), &profile).await?;
    crate::models::sidebar::SidebarLayout::get(service.db(), &profile).await
}

#[tauri::command]
pub async fn put_sidebar_layout(service: State<'_, ChatnipService>, window: Window, layout: crate::models::sidebar::SidebarLayout) -> Result<crate::models::sidebar::SidebarLayout, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    crate::models::sidebar::SidebarLayout::put(service.db(), &profile, &layout).await?;
    let saved = crate::models::sidebar::SidebarLayout::get(service.db(), &profile).await?;
    window.emit("sidebar-layout-updated", &saved).map_err(|e| format!("Failed to emit the sidebar layout: {}", e))?;
    Ok(saved)
}
