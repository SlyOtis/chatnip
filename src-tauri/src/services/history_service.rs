use std::path::Path;

use serde_json::{json, Value};
use sqlx::{Pool, Sqlite};
use tauri::{AppHandle, Emitter, Manager};

use crate::models::app_setting::{self, AppSetting};
use crate::models::history::{HistoryEntry, PostHistoryInput, KIND_ERROR, KIND_FAILED, KIND_SENT, STATUS_CANCELLED, STATUS_RESOLVED, STATUS_WAITING};
use crate::models::SendMessageInput;
use crate::services::outbound_service::{self, Outbound};

pub const HISTORY_EVENT: &str = "history-updated";

pub fn is_offline(error: &str) -> bool {
    let lower = error.to_lowercase();
    ["connect", "connection", "network", "dns", "timed out", "timeout", "offline", "unreachable", "resolve host", "no route"].iter().any(|needle| lower.contains(needle))
}

pub async fn record(pool: &Pool<Sqlite>, app: Option<&AppHandle>, input: PostHistoryInput) -> Result<HistoryEntry, String> {
    let profile = crate::models::Profile::get_active_id(pool).await?;
    let entry = HistoryEntry::post(pool, &profile, &input).await?;
    if let Some(app) = app {
        app.emit(HISTORY_EVENT, &entry).ok();
    }
    Ok(entry)
}

pub fn conversation_href(conversation_id: &str) -> String {
    format!("/main/conversation/{}", conversation_id)
}

pub async fn record_sent(pool: &Pool<Sqlite>, app: Option<&AppHandle>, conversation_id: &str, message_id: Option<&str>, title: &str, detail: &str) -> Result<HistoryEntry, String> {
    record(
        pool,
        app,
        PostHistoryInput {
            kind: KIND_SENT.into(),
            title: title.into(),
            detail: detail.into(),
            conversation_id: Some(conversation_id.into()),
            message_id: message_id.map(String::from),
            href: Some(conversation_href(conversation_id)),
            ..PostHistoryInput::default()
        },
    )
    .await
}

pub async fn record_failed_send(pool: &Pool<Sqlite>, app: Option<&AppHandle>, input: &SendMessageInput, error: &str) -> Result<HistoryEntry, String> {
    let wait_always = AppSetting::get(pool, app_setting::OUTBOUND_WAIT_FOR_NETWORK).await.ok().flatten().as_deref() == Some("true");
    let waiting = wait_always && is_offline(error);
    record(
        pool,
        app,
        PostHistoryInput {
            kind: KIND_FAILED.into(),
            title: snippet(&input.content),
            detail: error.into(),
            conversation_id: Some(input.conversation_id.clone()),
            message_id: None,
            href: Some(conversation_href(&input.conversation_id)),
            status: Some(if waiting { STATUS_WAITING } else { crate::models::history::STATUS_OPEN }.into()),
            options: Some(json!({ "send": input, "offline": is_offline(error) })),
        },
    )
    .await
}

pub async fn record_failed_hold(pool: &Pool<Sqlite>, app: Option<&AppHandle>, hold_id: &str, conversation_id: &str, subject: &str, error: &str) -> Result<HistoryEntry, String> {
    record(
        pool,
        app,
        PostHistoryInput {
            kind: KIND_FAILED.into(),
            title: subject.into(),
            detail: error.into(),
            conversation_id: Some(conversation_id.into()),
            message_id: None,
            href: Some(conversation_href(conversation_id)),
            status: None,
            options: Some(json!({ "hold_id": hold_id, "offline": is_offline(error) })),
        },
    )
    .await
}

pub async fn record_error(pool: &Pool<Sqlite>, app: Option<&AppHandle>, title: &str, detail: &str, href: Option<String>) -> Result<HistoryEntry, String> {
    record(pool, app, PostHistoryInput { kind: KIND_ERROR.into(), title: title.into(), detail: detail.into(), href, ..PostHistoryInput::default() }).await
}

pub fn snippet(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 80 {
        format!("{}…", flat.chars().take(79).collect::<String>())
    } else {
        flat
    }
}

async fn resend(pool: &Pool<Sqlite>, app_data: &Path, entry: &HistoryEntry) -> Result<Option<String>, String> {
    if let Some(hold_id) = entry.options.get("hold_id").and_then(Value::as_str) {
        let messages = outbound_service::post_hold_send(pool, app_data, hold_id).await?;
        return Ok(messages.first().map(|m| m.id.clone()));
    }
    let input: SendMessageInput = serde_json::from_value(entry.options.get("send").cloned().unwrap_or(Value::Null)).map_err(|e| format!("The failed send cannot be read: {}", e))?;
    let messages = outbound_service::post_outbound(pool, app_data, Outbound::Send(input)).await?;
    Ok(messages.first().map(|m| m.id.clone()))
}

pub async fn put_option(pool: &Pool<Sqlite>, app: &AppHandle, id: &str, option: &str) -> Result<HistoryEntry, String> {
    let entry = HistoryEntry::get_one(pool, id).await?;
    let updated = match option {
        "send_when_online" => HistoryEntry::put(pool, id, None, Some(STATUS_WAITING), None, None).await?,
        "always_wait" => {
            AppSetting::put(pool, app_setting::OUTBOUND_WAIT_FOR_NETWORK, "true").await?;
            HistoryEntry::put(pool, id, None, Some(STATUS_WAITING), None, None).await?
        }
        "cancel" => {
            if let Some(hold_id) = entry.options.get("hold_id").and_then(Value::as_str) {
                outbound_service::delete_hold(pool, hold_id).await.ok();
            }
            HistoryEntry::put(pool, id, None, Some(STATUS_CANCELLED), None, None).await?
        }
        "resolve" => HistoryEntry::put(pool, id, None, Some(STATUS_RESOLVED), None, None).await?,
        "send_now" => {
            let app_data = app.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
            match resend(pool, &app_data, &entry).await {
                Ok(message_id) => HistoryEntry::put(pool, id, Some(KIND_SENT), Some(STATUS_RESOLVED), Some(""), message_id.as_deref()).await?,
                Err(e) => HistoryEntry::put(pool, id, None, None, Some(&e), None).await?,
            }
        }
        other => return Err(format!("No history option called {}", other)),
    };
    app.emit(HISTORY_EVENT, &updated).ok();
    Ok(updated)
}

pub async fn retry_waiting(pool: &Pool<Sqlite>, app: &AppHandle) -> Result<usize, String> {
    let app_data = app.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let mut sent = 0;
    for entry in HistoryEntry::get_waiting(pool).await? {
        match resend(pool, &app_data, &entry).await {
            Ok(message_id) => {
                let updated = HistoryEntry::put(pool, &entry.id, Some(KIND_SENT), Some(STATUS_RESOLVED), Some(""), message_id.as_deref()).await?;
                app.emit(HISTORY_EVENT, &updated).ok();
                sent += 1;
            }
            Err(e) => {
                if !is_offline(&e) {
                    let updated = HistoryEntry::put(pool, &entry.id, None, Some(crate::models::history::STATUS_OPEN), Some(&e), None).await?;
                    app.emit(HISTORY_EVENT, &updated).ok();
                }
            }
        }
    }
    Ok(sent)
}

pub fn spawn_retry(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let Some(service) = app.try_state::<crate::ChatnipService>() else { continue };
            if let Err(e) = retry_waiting(service.db(), &app).await {
                eprintln!("Waiting sends were not retried: {}", e);
            }
        }
    });
}
