use crate::models::app_setting::{self, AppSetting};
use crate::services::security_service;
use crate::ChatnipService;
use serde::Serialize;
use sqlx::{Pool, Sqlite};
use tauri::{AppHandle, Emitter, Manager, State};


#[derive(Debug, Clone, Serialize)]
pub struct LockState {
    pub enabled: bool,
    pub locked: bool,
    pub idle_minutes: u64,
}

pub async fn lock_enabled(pool: &Pool<Sqlite>) -> bool {
    AppSetting::get_or(pool, app_setting::SECURITY_LOCK, "false").await.map(|v| v == "true").unwrap_or(false)
}

async fn state(pool: &Pool<Sqlite>) -> LockState {
    LockState {
        enabled: lock_enabled(pool).await,
        locked: security_service::is_locked(),
        idle_minutes: AppSetting::get_u64(pool, app_setting::SECURITY_IDLE_MINUTES).await.unwrap_or_else(|_| app_setting::get_default_u64(app_setting::SECURITY_IDLE_MINUTES)),
    }
}

pub async fn lock_now(app: &AppHandle, pool: &Pool<Sqlite>) -> Result<LockState, String> {
    if lock_enabled(pool).await {
        security_service::set_locked(true);
    }
    let current = state(pool).await;
    app.emit("lock-changed", &current).map_err(|e| format!("Failed to announce the lock: {}", e))?;
    Ok(current)
}

#[tauri::command]
pub async fn get_lock_state(service: State<'_, ChatnipService>) -> Result<LockState, String> {
    Ok(state(service.db()).await)
}

#[tauri::command]
pub async fn post_lock(app: AppHandle, service: State<'_, ChatnipService>) -> Result<LockState, String> {
    lock_now(&app, service.db()).await
}

#[tauri::command]
pub async fn delete_lock(app: AppHandle, service: State<'_, ChatnipService>) -> Result<LockState, String> {
    if security_service::is_locked() {
        security_service::authenticate("unlock Chatnip".to_string()).await?;
        security_service::set_locked(false);
    }
    let current = state(service.db()).await;
    app.emit("lock-changed", &current).map_err(|e| format!("Failed to announce the unlock: {}", e))?;
    Ok(current)
}

#[tauri::command]
pub async fn put_lock(app: AppHandle, service: State<'_, ChatnipService>, enabled: bool, idle_minutes: Option<u64>) -> Result<LockState, String> {
    if enabled != lock_enabled(service.db()).await {
        security_service::authenticate(if enabled { "turn on the Chatnip lock" } else { "turn off the Chatnip lock" }.to_string()).await?;
    }
    AppSetting::put(service.db(), app_setting::SECURITY_LOCK, if enabled { "true" } else { "false" }).await?;
    if let Some(minutes) = idle_minutes {
        AppSetting::put(service.db(), app_setting::SECURITY_IDLE_MINUTES, &minutes.to_string()).await?;
    }
    let current = state(service.db()).await;
    app.emit("lock-changed", &current).map_err(|e| format!("Failed to announce the lock: {}", e))?;
    Ok(current)
}

#[cfg(target_os = "macos")]
pub fn watch_screen_lock(app: &AppHandle) {
    use objc2_foundation::{NSDistributedNotificationCenter, NSString};
    let handle = app.clone();
    let block = block2::RcBlock::new(move |_notification: std::ptr::NonNull<objc2_foundation::NSNotification>| {
        let handle = handle.clone();
        tauri::async_runtime::spawn(async move {
            if let Some(service) = handle.try_state::<ChatnipService>() {
                let pool = service.db().clone();
                let _ = lock_now(&handle, &pool).await;
            }
        });
    });
    let name = NSString::from_str("com.apple.screenIsLocked");
    let token = unsafe { NSDistributedNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(Some(&name), None, None, &block) };
    std::mem::forget(token);
}

#[cfg(not(target_os = "macos"))]
pub fn watch_screen_lock(_app: &AppHandle) {}
