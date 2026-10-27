use std::sync::OnceLock;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn bundled() -> bool {
    cfg!(target_os = "macos") && std::env::current_exe().ok().is_some_and(|exe| exe.to_string_lossy().contains(".app/Contents/MacOS/"))
}

pub fn updates_in_place() -> bool {
    bundled()
}

#[cfg(target_os = "macos")]
mod center {
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{define_class, msg_send, AllocAnyThread};
    use objc2_foundation::{NSArray, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent, UNNotification, UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse, UNNotificationSettings, UNUserNotificationCenter,
        UNUserNotificationCenterDelegate,
    };
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    static REQUESTED: AtomicBool = AtomicBool::new(false);

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "ChatnipNotificationDelegate"]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl UNUserNotificationCenterDelegate for Delegate {
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(&self, _center: &UNUserNotificationCenter, _notification: &UNNotification, handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>) {
                handler.call((UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::List,));
            }

            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(&self, _center: &UNUserNotificationCenter, response: &UNNotificationResponse, handler: &block2::DynBlock<dyn Fn()>) {
                let id = response.notification().request().identifier().to_string();
                super::clicked(id);
                handler.call(());
            }
        }
    );

    impl Delegate {
        fn new() -> Retained<Self> {
            let this = Self::alloc().set_ivars(());
            unsafe { msg_send![super(this), init] }
        }
    }

    pub fn listen() {
        let delegate = Delegate::new();
        UNUserNotificationCenter::currentNotificationCenter().setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        std::mem::forget(delegate);
    }

    fn status() -> Option<UNAuthorizationStatus> {
        let (tx, rx) = std::sync::mpsc::channel();
        let block = block2::RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            let _ = tx.send(unsafe { settings.as_ref() }.authorizationStatus());
        });
        UNUserNotificationCenter::currentNotificationCenter().getNotificationSettingsWithCompletionHandler(&block);
        rx.recv_timeout(Duration::from_secs(5)).ok()
    }

    pub fn allowed() -> bool {
        match status() {
            Some(UNAuthorizationStatus::Authorized) | Some(UNAuthorizationStatus::Provisional) | Some(UNAuthorizationStatus::Ephemeral) => true,
            Some(UNAuthorizationStatus::NotDetermined) if !REQUESTED.swap(true, Ordering::SeqCst) => {
                let (tx, rx) = std::sync::mpsc::channel();
                let block = block2::RcBlock::new(move |granted: Bool, _error: *mut NSError| {
                    let _ = tx.send(granted.as_bool());
                });
                UNUserNotificationCenter::currentNotificationCenter().requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert, &block);
                rx.recv_timeout(Duration::from_secs(120)).unwrap_or(false)
            }
            _ => false,
        }
    }

    pub fn put(id: &str, title: &str, body: &str) {
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        content.setThreadIdentifier(&NSString::from_str(id));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&NSString::from_str(id), &content, None);
        UNUserNotificationCenter::currentNotificationCenter().addNotificationRequest_withCompletionHandler(&request, None);
    }

    pub fn delete(id: &str) {
        let ids = NSArray::from_retained_slice(&[NSString::from_str(id)]);
        let center = UNUserNotificationCenter::currentNotificationCenter();
        center.removePendingNotificationRequestsWithIdentifiers(&ids);
        center.removeDeliveredNotificationsWithIdentifiers(&ids);
    }
}

fn clicked(id: String) {
    let Some(app) = APP.get() else { return };
    match conversation_of(&id) {
        Some(conversation_id) => {
            use tauri::{Emitter, Manager};
            if let Some(window) = app.get_webview_window("main") {
                window.unminimize().ok();
                window.show().ok();
                window.set_focus().ok();
            }
            app.emit(OPEN_CONVERSATION_EVENT, &conversation_id).ok();
        }
        None => crate::services::outbound_service::opened(app.clone(), id),
    }
}

pub fn listen(app: &AppHandle) {
    let _ = APP.set(app.clone());
    #[cfg(target_os = "macos")]
    if bundled() {
        center::listen();
    }
}

pub async fn allowed() -> bool {
    #[cfg(target_os = "macos")]
    if bundled() {
        return tokio::task::spawn_blocking(center::allowed).await.unwrap_or(false);
    }
    APP.get().and_then(|app| app.notification().permission_state().ok()).is_some_and(|state| state == tauri_plugin_notification::PermissionState::Granted)
}

pub fn put(id: &str, title: &str, body: &str) {
    #[cfg(target_os = "macos")]
    if bundled() {
        center::put(id, title, body);
        return;
    }
    let _ = id;
    if let Some(app) = APP.get() {
        if let Err(e) = app.notification().builder().title(title).body(body).show() {
            eprintln!("Notification not shown: {}", e);
        }
    }
}

pub fn delete(id: &str) {
    #[cfg(target_os = "macos")]
    if bundled() {
        center::delete(id);
    }
    let _ = id;
}

pub const MESSAGE_PREFIX: &str = "conversation:";
pub const OPEN_CONVERSATION_EVENT: &str = "conversation-open";
const FRESH_MINUTES: i64 = 10;

pub fn conversation_of(id: &str) -> Option<String> {
    id.strip_prefix(MESSAGE_PREFIX).and_then(|rest| rest.split(':').next()).filter(|c| !c.is_empty()).map(String::from)
}

pub async fn put_messages(app: &AppHandle, pool: &sqlx::Pool<sqlx::Sqlite>, conversation_id: &str, message_ids: &[String]) -> Result<(), String> {
    use sqlx::Row;
    use tauri::Manager;
    let Some(profile) = crate::models::Profile::get_active(pool).await? else { return Ok(()) };
    let Some(row) = sqlx::query("SELECT name, is_muted, snoozed_until, notification_level FROM conversation WHERE id = ?1").bind(conversation_id).fetch_optional(pool).await.map_err(|e| format!("Failed to read the conversation: {}", e))? else {
        return Ok(());
    };
    let level: String = row.try_get("notification_level").unwrap_or_else(|_| "all".to_string());
    let muted: bool = row.try_get("is_muted").unwrap_or(false);
    let snoozed = row.try_get::<Option<String>, _>("snoozed_until").ok().flatten().and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok()).is_some_and(|until| until > chrono::Utc::now());
    if muted || snoozed || level == "nothing" {
        return Ok(());
    }
    let focused = app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
    if focused && profile.last_id.as_deref() == Some(conversation_id) {
        return Ok(());
    }
    let mine = crate::models::Profile::get_own_user_ids(pool, &profile.id).await?;
    let mut handles: Vec<String> = Vec::new();
    for user_id in &mine {
        let ids: Vec<String> = sqlx::query_scalar("SELECT external_id FROM provider_user WHERE user_id = ?1").bind(user_id).fetch_all(pool).await.map_err(|e| format!("Failed to read your accounts: {}", e))?;
        handles.extend(ids);
    }
    let name: String = row.try_get("name").unwrap_or_default();
    let floor = (chrono::Utc::now() - chrono::Duration::minutes(FRESH_MINUTES)).to_rfc3339();
    for id in message_ids {
        let Some(message) = sqlx::query("SELECT sender_id, sender_name, content, content_type, metadata, timestamp FROM message WHERE id = ?1 AND timestamp >= ?2 AND deleted_datetime IS NULL").bind(id).bind(&floor).fetch_optional(pool).await.map_err(|e| format!("Failed to read the message: {}", e))? else {
            continue;
        };
        let sender: String = message.try_get("sender_id").unwrap_or_default();
        if mine.contains(&sender) {
            continue;
        }
        let metadata: serde_json::Value = message.try_get::<Option<String>, _>("metadata").ok().flatten().and_then(|m| serde_json::from_str(&m).ok()).unwrap_or_default();
        if level == "mentions" && !mentions_me(&metadata, &handles) {
            continue;
        }
        if !allowed().await {
            return Ok(());
        }
        let content: String = message.try_get("content").unwrap_or_default();
        let content_type: String = message.try_get("content_type").unwrap_or_default();
        let text = crate::utils::text_extraction::generate_summary(&crate::utils::text_extraction::extract_text_from_content(&content, &content_type), 160);
        let from: String = message.try_get("sender_name").unwrap_or_default();
        let title = if name.is_empty() || name == from { from.clone() } else { format!("{} in {}", from, name) };
        put(&format!("{}{}:{}", MESSAGE_PREFIX, conversation_id, id), &title, &text);
    }
    Ok(())
}

pub fn mentions_me(metadata: &serde_json::Value, handles: &[String]) -> bool {
    metadata["structure"]["mentions"].as_array().is_some_and(|mentions| {
        mentions.iter().any(|m| {
            let id = m["id"].as_str().unwrap_or_default();
            matches!(id, "here" | "channel" | "everyone") || handles.iter().any(|h| h == id)
        })
    })
}
