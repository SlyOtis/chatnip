use crate::models::{app_setting, AppSetting};
use crate::providers::models::Provider;
use crate::ChatnipService;
use sqlx::{Pool, Row, Sqlite};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};

const LOOP_TICK_SECONDS: u64 = 30;
const BACKOFF_FIRST_SECONDS: u64 = 5;
const BACKOFF_MAX_SECONDS: u64 = 300;
const QUEUE: usize = 256;

static RUNNING: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static CONNECTED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn running() -> &'static Mutex<HashSet<String>> {
    RUNNING.get_or_init(|| Mutex::new(HashSet::new()))
}

fn connected() -> &'static Mutex<HashSet<String>> {
    CONNECTED.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn is_live(provider_id: &str) -> bool {
    connected().lock().map(|c| c.contains(provider_id)).unwrap_or(false)
}

fn put_connected(provider_id: &str, live: bool) {
    if let Ok(mut c) = connected().lock() {
        if live {
            c.insert(provider_id.to_string());
        } else {
            c.remove(provider_id);
        }
    }
}

pub async fn get_live_providers(pool: &Pool<Sqlite>) -> Result<Vec<String>, String> {
    let rows = sqlx::query("SELECT id, capabilities FROM provider WHERE is_enabled = 1 AND (access_token IS NOT NULL OR provider_type = 'mock') AND profile_id = (SELECT id FROM profile WHERE active = 1 LIMIT 1)")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Live updates could not list connections: {}", e))?;
    Ok(rows
        .into_iter()
        .filter(|row| {
            row.try_get::<String, _>("capabilities")
                .ok()
                .and_then(|c| serde_json::from_str::<crate::providers::models::ProviderCapabilities>(&c).ok())
                .is_some_and(|c| c.live)
        })
        .map(|row| row.get("id"))
        .collect())
}

pub fn spawn_live(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let pool = app.state::<ChatnipService>().db().clone();
            if !crate::services::security_service::is_locked() {
                match get_live_providers(&pool).await {
                    Ok(ids) => {
                        for id in ids {
                            let fresh = running().lock().map(|mut r| r.insert(id.clone())).unwrap_or(false);
                            if fresh {
                                let app = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    run(app, id.clone()).await;
                                    if let Ok(mut r) = running().lock() {
                                        r.remove(&id);
                                    }
                                    put_connected(&id, false);
                                });
                            }
                        }
                    }
                    Err(e) => eprintln!("{}", e),
                }
            }
            tokio::time::sleep(Duration::from_secs(LOOP_TICK_SECONDS)).await;
        }
    });
}

async fn run(app: AppHandle, provider_id: String) {
    let mut backoff = BACKOFF_FIRST_SECONDS;
    loop {
        let pool = app.state::<ChatnipService>().db().clone();
        if crate::services::security_service::is_locked() {
            tokio::time::sleep(Duration::from_secs(LOOP_TICK_SECONDS)).await;
            continue;
        }
        match get_live_providers(&pool).await {
            Ok(ids) if ids.contains(&provider_id) => {}
            _ => return,
        }
        match listen(&app, &pool, &provider_id).await {
            Ok(false) => return,
            Ok(true) => backoff = BACKOFF_FIRST_SECONDS,
            Err(e) => {
                eprintln!("Live updates of {} stopped: {}", provider_id, e);
                backoff = (backoff * 2).min(BACKOFF_MAX_SECONDS);
            }
        }
        put_connected(&provider_id, false);
        tokio::time::sleep(Duration::from_secs(backoff)).await;
    }
}

async fn listen(app: &AppHandle, pool: &Pool<Sqlite>, provider_id: &str) -> Result<bool, String> {
    let mut provider = Provider::load_from_db(pool, provider_id).await?;
    let provider_impl = crate::services::connection_service::provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    if !provider_impl.is_live() {
        return Ok(false);
    }
    let auth = crate::services::connection_service::ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    provider.auth = Some(auth.clone());
    let (tx, mut rx) = tokio::sync::mpsc::channel(QUEUE);
    let listener_impl = provider_impl.clone();
    let listener = tokio::spawn(async move { listener_impl.get_live(&auth, tx).await });
    put_connected(provider_id, true);
    if let Err(e) = crate::services::connection_service::sync_provider(app, pool, provider_id).await {
        eprintln!("Catching up {} failed: {}", provider_id, e);
    }
    while let Some(event) = rx.recv().await {
        let window = app.get_webview_window("main");
        let service = app.state::<ChatnipService>();
        let event_provider = provider.clone();
        let event_impl = provider_impl.clone();
        let outcome = service
            .with_write_lock(move |pool| async move { crate::providers::models::put_live(&pool, window.as_ref(), &event_provider, event_impl, event).await })
            .await;
        match outcome {
            Ok(true) => {
                if let Err(e) = crate::services::connection_service::sync_provider(app, pool, provider_id).await {
                    eprintln!("Syncing {} after a new conversation failed: {}", provider_id, e);
                }
            }
            Ok(false) => {}
            Err(e) => eprintln!("A live change of {} was not stored: {}", provider_id, e),
        }
        if crate::services::security_service::is_locked() {
            listener.abort();
            return Ok(true);
        }
    }
    listener.await.map_err(|e| e.to_string())??;
    Ok(true)
}

pub async fn get_sync_interval(pool: &Pool<Sqlite>, provider_id: &str) -> u64 {
    let key = if is_live(provider_id) { app_setting::SYNC_LIVE_INTERVAL_SECONDS } else { app_setting::SYNC_INTERVAL_SECONDS };
    AppSetting::get_u64(pool, key).await.unwrap_or_else(|_| app_setting::get_default_u64(key))
}
