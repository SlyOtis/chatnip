use crate::models::app_setting::{self, AppSetting};
use crate::models::{Profile, ProvidersResponse, ConversationsResponse, Conversation};
use crate::models::user::{ProviderUser, User};
use crate::providers::google::GoogleProvider;
use crate::providers::imap::{ImapConfig, ImapProvider};
use crate::providers::mock::{MockKind, MockProvider};
use crate::providers::microsoft::{self, MicrosoftProvider};
use crate::providers::models::{Provider, ProviderAuth};
use crate::providers::slack::SlackProvider;
use crate::providers::traits::ProviderTrait;
use crate::utils::theme_generator::get_or_create_provider_theme;
use crate::ChatnipService;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Pool, Row, Sqlite};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use uuid::Uuid;

pub const OAUTH_PORT: u16 = 48731;
const PENDING_TTL_MINUTES: i64 = 15;
const LOOP_TICK_SECONDS: u64 = 20;

pub struct PendingAuth {
    pub provider_type: String,
    pub verifier: String,
    pub started_at: DateTime<Utc>,
}

pub type PendingAuths = Arc<Mutex<HashMap<String, PendingAuth>>>;

pub fn get_oauth_port() -> u16 {
    std::env::var("CHATNIP_OAUTH_PORT").ok().and_then(|port| port.trim().parse().ok()).unwrap_or(OAUTH_PORT)
}

pub fn redirect_uri(provider_type: &str) -> String {
    format!("http://localhost:{}/callback/{}", get_oauth_port(), provider_type)
}

pub async fn microsoft_provider(pool: &Pool<Sqlite>) -> Result<MicrosoftProvider, String> {
    let client_id = AppSetting::get_or(pool, app_setting::MICROSOFT_CLIENT_ID, "").await?;
    let client_secret = AppSetting::get_or(pool, app_setting::MICROSOFT_CLIENT_SECRET, "").await?;
    let tenant = microsoft::normalize_tenant(&AppSetting::get_or(pool, app_setting::MICROSOFT_TENANT, "").await?)?;
    let admin = AppSetting::get_or(pool, app_setting::MICROSOFT_ADMIN_SCOPES, "false").await? == "true";
    Ok(MicrosoftProvider::new(client_id, client_secret, redirect_uri("microsoft"), tenant, admin))
}

pub fn admin_consent_tenant(tenant: &str) -> &str {
    if matches!(tenant, "common" | "consumers") { microsoft::DEFAULT_TENANT } else { tenant }
}

pub async fn admin_consent_link(pool: &Pool<Sqlite>) -> Result<String, String> {
    let client_id = AppSetting::get(pool, app_setting::MICROSOFT_CLIENT_ID).await?.ok_or_else(|| "Add the Microsoft client id under Settings, Connections first".to_string())?;
    let tenant = microsoft::normalize_tenant(&AppSetting::get_or(pool, app_setting::MICROSOFT_TENANT, "").await?)?;
    Ok(microsoft::admin_consent_url(admin_consent_tenant(&tenant), &client_id, &redirect_uri("microsoft")))
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderSetup {
    pub provider_type: String,
    pub ready: bool,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub admin_scopes: Vec<String>,
    pub tenant: Option<String>,
    pub admin_consent_url: Option<String>,
    pub console_url: String,
}

pub async fn connection_setup(pool: &Pool<Sqlite>) -> Result<Vec<ProviderSetup>, String> {
    let has = |key: &'static str| async move { AppSetting::get(pool, key).await.map(|v| v.is_some()) };
    let tenant = microsoft::normalize_tenant(&AppSetting::get_or(pool, app_setting::MICROSOFT_TENANT, "").await?).unwrap_or_else(|_| microsoft::DEFAULT_TENANT.to_string());
    let owned = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<String>>();
    Ok(vec![
        ProviderSetup {
            provider_type: "microsoft".into(),
            ready: has(app_setting::MICROSOFT_CLIENT_ID).await?,
            redirect_uri: redirect_uri("microsoft"),
            scopes: owned(&microsoft::BASE_SCOPES),
            admin_scopes: owned(&microsoft::ADMIN_SCOPES),
            tenant: Some(tenant),
            admin_consent_url: admin_consent_link(pool).await.ok(),
            console_url: "https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade".into(),
        },
        ProviderSetup {
            provider_type: "google".into(),
            ready: has(app_setting::GOOGLE_CLIENT_ID).await?,
            redirect_uri: redirect_uri("google"),
            scopes: owned(&crate::providers::google::SCOPES),
            admin_scopes: Vec::new(),
            tenant: None,
            admin_consent_url: None,
            console_url: "https://console.cloud.google.com/auth/clients".into(),
        },
        ProviderSetup {
            provider_type: "slack".into(),
            ready: has(app_setting::SLACK_CLIENT_ID).await?,
            redirect_uri: redirect_uri("slack"),
            scopes: owned(&crate::providers::slack::USER_SCOPES),
            admin_scopes: Vec::new(),
            tenant: None,
            admin_consent_url: None,
            console_url: "https://api.slack.com/apps".into(),
        },
    ])
}

pub async fn put_capabilities(pool: &Pool<Sqlite>) -> Result<usize, String> {
    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as("SELECT id, provider_type, config FROM provider")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list connections: {}", e))?;
    let mut changed = 0;
    for (id, provider_type, config) in rows {
        let Ok(client) = provider_impl(pool, &provider_type, config.as_deref()).await else { continue };
        let capabilities = serde_json::to_string(&client.get_provider().capabilities).map_err(|e| e.to_string())?;
        changed += sqlx::query("UPDATE provider SET capabilities = ?1 WHERE id = ?2 AND capabilities != ?1")
            .bind(&capabilities)
            .bind(&id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to refresh what {} can do: {}", id, e))?
            .rows_affected() as usize;
    }
    Ok(changed)
}

pub async fn provider_impl(pool: &Pool<Sqlite>, provider_type: &str, config: Option<&str>) -> Result<Arc<dyn ProviderTrait>, String> {
    crate::providers::http::put_policy(AppSetting::get_u64(pool, app_setting::PROVIDER_RETRY_ATTEMPTS).await?, AppSetting::get_u64(pool, app_setting::PROVIDER_RETRY_MAX_SECONDS).await?);
    match provider_type {
        "imap" => {
            let raw = config.ok_or_else(|| "The mail connection has no settings".to_string())?;
            Ok(Arc::new(ImapProvider::new(ImapProvider::config_from_json(raw)?)))
        }
        "microsoft" => Ok(Arc::new(microsoft_provider(pool).await?)),
        "google" => {
            let client_id = AppSetting::get_or(pool, app_setting::GOOGLE_CLIENT_ID, "").await?;
            let client_secret = AppSetting::get_or(pool, app_setting::GOOGLE_CLIENT_SECRET, "").await?;
            Ok(Arc::new(GoogleProvider::new(client_id, client_secret, redirect_uri("google"))))
        }
        "slack" => {
            let client_id = AppSetting::get_or(pool, app_setting::SLACK_CLIENT_ID, "").await?;
            let client_secret = AppSetting::get_or(pool, app_setting::SLACK_CLIENT_SECRET, "").await?;
            let app_token = AppSetting::get_or(pool, app_setting::SLACK_APP_TOKEN, "").await?;
            Ok(Arc::new(SlackProvider::new(client_id, client_secret, redirect_uri("slack")).with_app_token(&app_token)))
        }
        "mock" => {
            if !crate::commands_people::mock_enabled(pool).await {
                return Err("Mock clients are switched off".to_string());
            }
            let kind = MockKind::parse(config.ok_or_else(|| "The mock connection has no kind".to_string())?)?;
            Ok(Arc::new(MockProvider::new(kind, pool.clone())))
        }
        other => Err(format!("Unknown provider type: {}", other)),
    }
}

fn pkce_pair() -> (String, String) {
    let verifier = URL_SAFE_NO_PAD.encode(Uuid::new_v4().as_bytes().iter().chain(Uuid::new_v4().as_bytes()).copied().collect::<Vec<u8>>());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

pub async fn start_auth(pool: &Pool<Sqlite>, pending: &PendingAuths, provider_type: &str) -> Result<String, String> {
    let client_id_key = match provider_type {
        "microsoft" => app_setting::MICROSOFT_CLIENT_ID,
        "google" => app_setting::GOOGLE_CLIENT_ID,
        "slack" => app_setting::SLACK_CLIENT_ID,
        other => return Err(format!("{} connects with a form in settings", other)),
    };
    let provider = provider_impl(pool, provider_type, None).await?;
    if AppSetting::get(pool, client_id_key).await?.is_none() {
        return Err(format!("Add the {} client id under Settings before connecting", provider_type));
    }
    let (verifier, challenge) = pkce_pair();
    let state = Uuid::new_v4().to_string();
    let url = provider.get_auth_url(&state, Some(&challenge)).await?;
    let mut map = pending.lock().await;
    map.retain(|_, p| Utc::now() - p.started_at < Duration::minutes(PENDING_TTL_MINUTES));
    map.insert(state, PendingAuth { provider_type: provider_type.to_string(), verifier, started_at: Utc::now() });
    drop(map);
    open::that(&url).map_err(|e| format!("Could not open the browser: {}", e))?;
    Ok(url)
}

pub async fn connect_with_token(app: &AppHandle, pool: &Pool<Sqlite>, provider_type: &str, token: &str) -> Result<Provider, String> {
    let provider = provider_impl(pool, provider_type, None).await?;
    if !provider.connects_with_token() {
        return Err(format!("{} connects through the browser", provider_type));
    }
    let token = token.trim();
    if token.is_empty() {
        return Err("Paste the token first".to_string());
    }
    let auth = ProviderAuth {
        access_token: token.to_string(),
        refresh_token: None,
        expires_at: Utc::now() + Duration::days(36500),
        token_type: "Bearer".to_string(),
        scope: None,
    };
    complete_auth(app, pool, provider_type, auth, None).await
}

pub async fn connect_imap(app: &AppHandle, pool: &Pool<Sqlite>, config: ImapConfig, password: &str) -> Result<Provider, String> {
    if config.email.trim().is_empty() || config.imap_host.trim().is_empty() || config.smtp_host.trim().is_empty() {
        return Err("Mail needs an address, an IMAP host and an SMTP host".to_string());
    }
    if password.trim().is_empty() {
        return Err("Enter the password or app password".to_string());
    }
    let mut config = config;
    if config.username.trim().is_empty() {
        config.username = config.email.clone();
    }
    if config.imap_port == 0 {
        config.imap_port = 993;
    }
    if config.smtp_port == 0 {
        config.smtp_port = 587;
    }
    let raw = serde_json::to_string(&config).map_err(|e| format!("Could not store the mail settings: {}", e))?;
    let auth = ProviderAuth {
        access_token: password.to_string(),
        refresh_token: None,
        expires_at: Utc::now() + Duration::days(36500),
        token_type: "Password".to_string(),
        scope: None,
    };
    complete_auth(app, pool, "imap", auth, Some(raw)).await
}

pub async fn complete_auth(app: &AppHandle, pool: &Pool<Sqlite>, provider_type: &str, auth: ProviderAuth, config: Option<String>) -> Result<Provider, String> {
    let provider = provider_impl(pool, provider_type, config.as_deref()).await?;
    let identity = provider.identity(&auth).await?;
    let profile = Profile::get_or_post(pool, &identity.display_name()).await?;
    let now = Utc::now().to_rfc3339();

    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM provider WHERE profile_id = ?1 AND provider_type = ?2 AND namespace = ?3",
    )
    .bind(&profile.id)
    .bind(provider_type)
    .bind(&identity.namespace)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to look up connection: {}", e))?;

    let capabilities = serde_json::to_string(&provider.get_provider().capabilities).unwrap_or_else(|_| "{}".to_string());
    let provider_id = existing.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
    let (access_token, refresh_token) = auth.store(&profile.id, &provider_id)?;
    let provider_id = match existing {
        Some(id) => {
            sqlx::query(
                "UPDATE provider SET name = ?1, external_id = ?2, capabilities = ?3, is_enabled = 1,
                 access_token = ?4, refresh_token = ?5, expires_at = ?6, token_type = ?7, scope = ?8, updated_at = ?9, config = ?11
                 WHERE id = ?10",
            )
            .bind(&identity.name)
            .bind(&identity.external_id)
            .bind(&capabilities)
            .bind(&access_token)
            .bind(&refresh_token)
            .bind(auth.expires_at.to_rfc3339())
            .bind(&auth.token_type)
            .bind(&auth.scope)
            .bind(&now)
            .bind(&id)
            .bind(&config)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update connection: {}", e))?;
            id
        }
        None => {
            let id = provider_id.clone();
            sqlx::query(
                "INSERT INTO provider (
                    id, profile_id, provider_type, name, external_id, capabilities, is_enabled,
                    access_token, refresh_token, expires_at, token_type, scope,
                    sync_interval, namespace, created_at, updated_at, config
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14, ?15)",
            )
            .bind(&id)
            .bind(&profile.id)
            .bind(provider_type)
            .bind(&identity.name)
            .bind(&identity.external_id)
            .bind(&capabilities)
            .bind(&access_token)
            .bind(&refresh_token)
            .bind(auth.expires_at.to_rfc3339())
            .bind(&auth.token_type)
            .bind(&auth.scope)
            .bind(app_setting::get_default_u64(app_setting::SYNC_INTERVAL_SECONDS) as i64)
            .bind(&identity.namespace)
            .bind(&now)
            .bind(&config)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to store connection: {}", e))?;
            id
        }
    };

    if let Some(user) = &identity.user {
        let user_id = User::upsert_from_response(pool, user).await?;
        ProviderUser::create_or_update(pool, user_id.clone(), provider_id.clone(), user.id.clone()).await?;
        sqlx::query("UPDATE provider SET user_external_id = ?1 WHERE id = ?2")
            .bind(&user.id)
            .bind(&provider_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to note the account on the connection: {}", e))?;
        sqlx::query("UPDATE profile SET user_id = ?1 WHERE id = ?2 AND user_id IS NULL")
            .bind(&user_id)
            .bind(&profile.id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to link the profile user: {}", e))?;
    }

    match provider.fetch_logo(&auth, &identity).await {
        Ok(Some((bytes, content_type))) => {
            if let Err(e) = store_logo(app, pool, &profile.id, &provider_id, bytes, &content_type).await {
                eprintln!("Logo for {} not stored: {}", provider_id, e);
            }
        }
        Ok(None) => {}
        Err(e) => eprintln!("Logo for {} not fetched: {}", provider_id, e),
    }

    if let Err(e) = get_or_create_provider_theme(pool, &provider_id, &profile.id).await {
        eprintln!("Theme for {} not created: {}", provider_id, e);
    }
    profile.update_setup_step(pool).await?;

    emit_providers(app, pool).await?;
    emit_profile(app, pool).await;

    let background = app.clone();
    let background_pool = pool.clone();
    let background_id = provider_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = sync_provider(&background, &background_pool, &background_id).await {
            eprintln!("First sync of {} failed: {}", background_id, e);
        }
    });

    let mut stored = Provider::load_from_db(pool, &provider_id).await?;
    stored.load_logo(pool).await.ok();
    Ok(stored)
}

pub async fn store_logo(app: &AppHandle, pool: &Pool<Sqlite>, profile_id: &str, provider_id: &str, bytes: Vec<u8>, content_type: &str) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("Empty logo".to_string());
    }
    let extension = match content_type {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        _ => "png",
    };
    let app_dir = app.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let dir = app_dir.join("assets").join(provider_id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create assets directory: {}", e))?;
    let logo_id = Uuid::new_v4().to_string();
    let filename = format!("{}.{}", logo_id, extension);
    std::fs::write(dir.join(&filename), &bytes).map_err(|e| format!("Failed to write logo: {}", e))?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO assets (id, profile_id, name, url, type, mime_type, path, created_at, updated_at)
         VALUES (?1, ?2, ?3, NULL, 'logo', ?4, ?5, ?6, ?6)",
    )
    .bind(&logo_id)
    .bind(profile_id)
    .bind(&filename)
    .bind(content_type)
    .bind(format!("assets/{}/{}", provider_id, filename))
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create logo asset: {}", e))?;
    sqlx::query("UPDATE provider SET logo_id = ?1 WHERE id = ?2")
        .bind(&logo_id)
        .bind(provider_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to attach logo: {}", e))?;
    Ok(logo_id)
}

pub async fn refresh_logo(app: &AppHandle, pool: &Pool<Sqlite>, provider_id: &str) -> Result<(), String> {
    let mut provider = Provider::load_from_db(pool, provider_id).await?;
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    let identity = provider_impl.identity(&auth).await?;
    if let Some((bytes, content_type)) = provider_impl.fetch_logo(&auth, &identity).await? {
        store_logo(app, pool, &provider.profile_id, provider_id, bytes, &content_type).await?;
        emit_providers(app, pool).await?;
    }
    Ok(())
}

pub async fn ensure_fresh(pool: &Pool<Sqlite>, provider: &mut Provider, provider_impl: Arc<dyn ProviderTrait>) -> Result<ProviderAuth, String> {
    if provider.provider_type == "mock" {
        return Ok(ProviderAuth::mock(&provider.id));
    }
    let auth = provider.auth.clone().ok_or_else(|| "The connection has no tokens".to_string())?;
    if auth.expires_at >= Utc::now() + Duration::minutes(5) {
        return Ok(auth);
    }
    let refresh_token = match &auth.refresh_token {
        Some(token) => token.clone(),
        None => return Ok(auth),
    };
    let fresh = provider_impl.refresh_token(&refresh_token).await?;
    provider.update_auth(pool, fresh.clone()).await?;
    Ok(fresh)
}

pub async fn sync_provider(app: &AppHandle, pool: &Pool<Sqlite>, provider_id: &str) -> Result<(), String> {
    let service = app.state::<ChatnipService>();
    let window = app.get_webview_window("main").ok_or_else(|| "No main window".to_string())?;
    let mut provider = Provider::load_from_db(pool, provider_id).await?;
    if !provider.is_enabled {
        return Ok(());
    }
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    let calendar_provider = provider.clone();
    let calendar_impl = provider_impl.clone();
    let emoji_impl = provider_impl.clone();
    let app_for_sync = app.clone();
    let id = provider_id.to_string();
    let result = service
        .with_write_lock(move |pool| async move {
            let _ = &app_for_sync;
            provider.sync(&pool, &window, provider_impl).await
        })
        .await;
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE provider SET last_sync_at = ?1, updated_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to stamp the sync: {}", e))?;
    match crate::services::calendar_service::sync_provider_calendars(pool, &calendar_provider, calendar_impl, &auth).await {
        Ok(0) => {}
        Ok(count) => {
            let _ = app.emit("events-updated", count);
        }
        Err(e) => eprintln!("Calendars of {} did not sync: {}", calendar_provider.name, e),
    }
    if let Ok(app_data) = app.path().app_data_dir() {
        let mut emoji_provider = Provider::load_from_db(pool, provider_id).await?;
        match crate::services::emoji_service::sync(&app_data, pool, &mut emoji_provider, emoji_impl, &auth).await {
            Ok(true) => {
                if let Ok(all) = crate::services::emoji_service::get_all(pool, &app_data, &emoji_provider.profile_id).await {
                    let _ = app.emit(crate::services::emoji_service::EVENT, &all);
                }
            }
            Ok(false) => {}
            Err(e) => eprintln!("Custom emoji of {} did not sync: {}", emoji_provider.name, e),
        }
    }
    if calendar_provider.provider_type == "mock" && crate::commands_people::mock_enabled(pool).await {
        let app_data = app.path().app_data_dir().ok();
        if let Err(e) = crate::providers::mock::seed::seed(pool, &calendar_provider.profile_id, app_data.as_deref()).await {
            eprintln!("Mock data was not seeded: {}", e);
        }
    }
    emit_providers(app, pool).await?;
    result
}

pub async fn connected(pool: &Pool<Sqlite>, provider_id: &str) -> Result<(Provider, Arc<dyn ProviderTrait>, ProviderAuth), String> {
    let mut provider = Provider::load_from_db(pool, provider_id).await?;
    if !provider.is_enabled {
        return Err(format!("{} is switched off", provider.name));
    }
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    Ok((provider, provider_impl, auth))
}

pub async fn delete_provider(app: &AppHandle, pool: &Pool<Sqlite>, provider_id: &str) -> Result<(), String> {
    let provider = Provider::load_from_db(pool, provider_id).await?;
    let mut tx = pool.begin().await.map_err(|e| format!("Failed to start transaction: {}", e))?;
    let owned = "SELECT conversation_id FROM conversation_provider WHERE provider_id = ?1
         AND conversation_id NOT IN (SELECT conversation_id FROM conversation_provider WHERE provider_id != ?1)";
    for (what, statement) in [
        ("attachments", format!("DELETE FROM assets WHERE message_id IN (SELECT id FROM message WHERE provider_id = ?1 OR conversation_id IN ({owned}))")),
        ("messages", format!("DELETE FROM message WHERE provider_id = ?1 OR conversation_id IN ({owned})")),
        ("meetings", format!("DELETE FROM meeting WHERE provider_id = ?1 OR conversation_id IN ({owned})")),
        ("conversations", format!("DELETE FROM conversation WHERE id IN ({owned})")),
    ] {
        sqlx::query(&statement)
            .bind(provider_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to remove {}: {}", what, e))?;
    }
    for table in ["conversation_provider", "provider_user", "provider_theme", "event", "calendar", "calendar_event", "assets", "workspace_emoji"] {
        sqlx::query(&format!("DELETE FROM {} WHERE provider_id = ?1", table))
            .bind(provider_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to clean {}: {}", table, e))?;
    }
    if let Some(logo_id) = &provider.logo_id {
        sqlx::query("DELETE FROM assets WHERE id = ?1")
            .bind(logo_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to remove the logo: {}", e))?;
    }
    sqlx::query("DELETE FROM provider WHERE id = ?1")
        .bind(provider_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to remove the connection: {}", e))?;
    sqlx::query(
        "UPDATE profile SET last_id = NULL WHERE id = ?1 AND last_id IS NOT NULL AND last_id NOT IN (SELECT id FROM conversation)",
    )
    .bind(&provider.profile_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to clear the open conversation: {}", e))?;
    tx.commit().await.map_err(|e| format!("Failed to commit: {}", e))?;
    ProviderAuth::delete(&provider.profile_id, provider_id)?;
    if provider.provider_type == "mock" {
        if let (Some(auth), Some(kind)) = (provider.auth.as_ref(), provider.config.as_deref().and_then(|c| MockKind::parse(c).ok())) {
            crate::providers::mock::server::delete_account(pool, &crate::providers::mock::server::account(kind, auth)).await?;
        }
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider WHERE profile_id = ?1 AND provider_type = 'mock'")
            .bind(&provider.profile_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to count the mock connections: {}", e))?;
        if remaining == 0 {
            let app_data = app.path().app_data_dir().ok();
            crate::providers::mock::seed::unseed(pool, &provider.profile_id, app_data.as_deref()).await?;
        }
    }

    if let Some(profile) = Profile::get_active(pool).await? {
        profile.update_setup_step(pool).await?;
    }
    emit_providers(app, pool).await?;
    emit_conversations(app, pool).await?;
    emit_profile(app, pool).await;
    Ok(())
}

pub async fn emit_providers(app: &AppHandle, pool: &Pool<Sqlite>) -> Result<(), String> {
    let profile_id: Option<String> = Profile::get_active(pool).await?.map(|p| p.id);
    let mut data = Vec::new();
    if let Some(profile_id) = profile_id {
        let hide_mock = !crate::commands_people::mock_enabled(pool).await;
        let rows = sqlx::query(if hide_mock { "SELECT id FROM provider WHERE profile_id = ?1 AND provider_type != 'mock' ORDER BY provider_type, name" } else { "SELECT id FROM provider WHERE profile_id = ?1 ORDER BY provider_type, name" })
            .bind(&profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list connections: {}", e))?;
        for row in rows {
            let id: String = row.get("id");
            if let Ok(mut provider) = Provider::load_from_db(pool, &id).await {
                provider.theme = crate::utils::theme_generator::get_provider_theme(pool, &id).await?;
                provider.load_logo(pool).await.ok();
                provider.load_account(pool).await.ok();
                data.push(provider);
            }
        }
    }
    app.emit("providers-updated", &ProvidersResponse { data, is_loading: false })
        .map_err(|e| format!("Failed to emit providers: {}", e))
}

async fn emit_conversations(app: &AppHandle, pool: &Pool<Sqlite>) -> Result<(), String> {
    let rows = Conversation::get_rows_for_active_profile(pool).await?;
    let mut data = Vec::new();
    for row in rows {
        if let Ok(mut conversation) = Conversation::from_row(row) {
            conversation.load_providers(pool).await.ok();
            conversation.tags = crate::models::ObjectTag::get_conversation(pool, &conversation.id).await.unwrap_or_default();
            conversation.load_display(pool).await.ok();
            data.push(conversation);
        }
    }
    app.emit("conversations-updated", &ConversationsResponse { data, is_loading: false })
        .map_err(|e| format!("Failed to emit conversations: {}", e))
}

async fn emit_profile(app: &AppHandle, pool: &Pool<Sqlite>) {
    if let Ok(Some(profile)) = Profile::get_active(pool).await {
        let _ = app.emit("profile-updated", &crate::models::ProfileResponse { data: Some(profile), is_loading: false });
    }
}

pub fn spawn_listener(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let port = get_oauth_port();
        let listener = match TcpListener::bind(("127.0.0.1", port)).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Consent listener could not bind port {}: {}", port, e);
                return;
            }
        };
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { continue };
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let request = read_request(&mut socket).await;
                if request.starts_with("POST /hook/") {
                    let (status, body) = match hook(&app, &request).await {
                        Ok(fired) => ("200 OK", serde_json::json!({ "ok": true, "fired": fired }).to_string()),
                        Err(e) => ("400 Bad Request", serde_json::json!({ "ok": false, "error": e }).to_string()),
                    };
                    let response = format!("HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, body.len(), body);
                    let _ = socket.write_all(response.as_bytes()).await;
                    let _ = socket.shutdown().await;
                    return;
                }
                let page = handle_callback(&app, &request).await;
                let status = if page.ok { "200 OK" } else { "400 Bad Request" };
                let body = callback_page(&page.title, &page.detail, page.link.as_ref());
                let response = format!(
                    "HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status,
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
                if !page.ok {
                    let _ = app.emit("auth-failed", format!("{} {}", page.title, page.detail));
                }
            });
        }
    });
}

const HOOK_LIMIT: usize = 1 << 20;

async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut data = Vec::new();
    let mut buffer = vec![0u8; 8192];
    loop {
        let read = socket.read(&mut buffer).await.unwrap_or(0);
        if read == 0 {
            break;
        }
        data.extend_from_slice(&buffer[..read]);
        let text = String::from_utf8_lossy(&data);
        let Some(end) = text.find("\r\n\r\n") else {
            if data.len() > HOOK_LIMIT {
                break;
            }
            continue;
        };
        let length = text[..end].lines().find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.trim().parse::<usize>().ok())).unwrap_or(0);
        if data.len() >= end + 4 + length.min(HOOK_LIMIT) {
            break;
        }
    }
    String::from_utf8_lossy(&data).to_string()
}

async fn hook(app: &AppHandle, request: &str) -> Result<bool, String> {
    use tauri::Manager;
    let (head, body) = request.split_once("\r\n\r\n").unwrap_or((request, ""));
    let target = head.lines().next().and_then(|l| l.split_whitespace().nth(1)).ok_or("Malformed request")?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut parts = path.trim_start_matches("/hook/").split('/');
    let (Some(action_id), Some(node_id)) = (parts.next().filter(|p| !p.is_empty()), parts.next().filter(|p| !p.is_empty())) else {
        return Err("The address is /hook/<action>/<trigger node>".to_string());
    };
    let token = query.split('&').find_map(|pair| pair.strip_prefix("token=")).map(String::from).or_else(|| head.lines().find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("x-chatnip-token")).map(|(_, v)| v.trim().to_string())));
    let payload = serde_json::from_str(body.trim()).unwrap_or_else(|_| serde_json::Value::String(body.to_string()));
    let pool = app.state::<crate::ChatnipService>().db().clone();
    let runner = crate::services::action_service::runner_for(&pool, app, false).await?;
    Ok(crate::services::action_service::post_webhook(&runner, action_id, node_id, token.as_deref(), payload).await?.is_some())
}

pub struct CallbackPage {
    pub ok: bool,
    pub title: String,
    pub detail: String,
    pub link: Option<(String, String)>,
}

impl CallbackPage {
    fn failed(detail: String) -> Self {
        Self { ok: false, title: "Chatnip could not finish the connection.".into(), detail, link: None }
    }
}

async fn handle_callback(app: &AppHandle, request: &str) -> CallbackPage {
    finish_callback(app, request).await.unwrap_or_else(CallbackPage::failed)
}

pub async fn consent_failure_page(pool: &Pool<Sqlite>, provider_type: &str, error: &str, description: &str) -> CallbackPage {
    let issue = if provider_type == "microsoft" { microsoft::consent_issue(error, description) } else { None };
    match issue {
        Some(issue) => {
            let (title, detail) = microsoft::consent_text(issue);
            let link = match issue {
                microsoft::ConsentIssue::NeedsAdmin | microsoft::ConsentIssue::UnknownApp => admin_consent_link(pool).await.ok().map(|url| ("Approval link for your administrator".to_string(), url)),
                _ => None,
            };
            CallbackPage { ok: false, title: title.into(), detail: detail.into(), link }
        }
        None => CallbackPage::failed(format!("{} {}", error, description).trim().to_string()),
    }
}

async fn finish_callback(app: &AppHandle, request: &str) -> Result<CallbackPage, String> {
    let line = request.lines().next().unwrap_or("");
    let path = line.split_whitespace().nth(1).ok_or_else(|| "Malformed request".to_string())?;
    let url = url::Url::parse(&format!("http://localhost{}", path)).map_err(|e| format!("Bad callback url: {}", e))?;
    let provider_type = url
        .path()
        .strip_prefix("/callback/")
        .ok_or_else(|| "Not a consent callback".to_string())?
        .to_string();
    let query: HashMap<String, String> = url.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let service = app.state::<ChatnipService>();
    let pool = service.db().clone();
    if let Some(error) = query.get("error") {
        let description = query.get("error_description").cloned().unwrap_or_default();
        return Ok(consent_failure_page(&pool, &provider_type, error, &description).await);
    }
    if query.get("state").map(String::as_str) == Some(microsoft::ADMIN_CONSENT_STATE) {
        let tenant = query.get("tenant").cloned().unwrap_or_default();
        let _ = app.emit("admin-consent-granted", &tenant);
        return Ok(CallbackPage {
            ok: true,
            title: "Your organisation approved Chatnip.".into(),
            detail: "People in this organisation can now connect from Chatnip, each with their own account. Nothing is shared between them. You can close this tab.".into(),
            link: None,
        });
    }
    let code = query.get("code").ok_or_else(|| "No code in the callback".to_string())?;
    let state = query.get("state").ok_or_else(|| "No state in the callback".to_string())?;
    let pending = service.pending_auth().lock().await.remove(state).ok_or_else(|| "The consent request is unknown or expired. Start again from settings.".to_string())?;
    if pending.provider_type != provider_type {
        return Err("The callback does not match the pending request".to_string());
    }
    let provider = provider_impl(&pool, &provider_type, None).await?;
    let auth = provider.exchange_code(code, Some(&pending.verifier)).await?;
    let stored = complete_auth(app, &pool, &provider_type, auth, None).await?;
    let _ = app.emit("auth-complete", &stored);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_focus();
    }
    Ok(CallbackPage { ok: true, title: format!("{} is connected.", stored.name), detail: "You can close this tab and go back to Chatnip.".into(), link: None })
}

fn callback_page(title: &str, detail: &str, link: Option<&(String, String)>) -> String {
    let action = link
        .map(|(label, url)| format!("<p class=\"link\"><a href=\"{0}\">{1}</a></p><p class=\"url\">{0}</p>", html_escape(url), html_escape(label)))
        .unwrap_or_default();
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Chatnip</title>\
<style>html{{background:#2b3038;color:#f2f1ec;font:16px/1.5 system-ui,sans-serif}}body{{display:grid;place-items:center;min-height:100vh;margin:0}}\
main{{padding:2rem 2.5rem;border-radius:1.25rem;background:rgba(255,255,255,.06);box-shadow:inset 1px 1px 0 rgba(255,255,255,.18)}}\
h1{{font-size:1rem;font-weight:650;margin:0 0 .5rem}}p{{margin:0;opacity:.8;max-width:60ch}}a{{color:#9db4ff}}.link{{margin-top:1rem;opacity:1}}.url{{margin-top:.5rem;font:12px/1.4 ui-monospace,monospace;word-break:break-all;user-select:all}}</style></head>\
<body><main><h1>{}</h1><p>{}</p>{}</main></body></html>",
        html_escape(title),
        html_escape(detail),
        action
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn spawn_sync_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(LOOP_TICK_SECONDS)).await;
            let service = app.state::<ChatnipService>();
            let pool = service.db().clone();
            if crate::services::security_service::is_locked() {
                continue;
            }
            let rows = match sqlx::query("SELECT id, last_sync_at FROM provider WHERE is_enabled = 1 AND access_token IS NOT NULL AND profile_id = (SELECT id FROM profile WHERE active = 1 LIMIT 1)")
                .fetch_all(&pool)
                .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    eprintln!("Sync loop could not list connections: {}", e);
                    continue;
                }
            };
            for row in rows {
                let id: String = row.get("id");
                let interval = crate::services::live_service::get_sync_interval(&pool, &id).await;
                let last: Option<String> = row.try_get("last_sync_at").ok().flatten();
                let due = match last.and_then(|s| DateTime::parse_from_rfc3339(&s).ok()) {
                    Some(at) => Utc::now() - at.with_timezone(&Utc) >= Duration::seconds(interval as i64),
                    None => true,
                };
                if !due {
                    continue;
                }
                if let Err(e) = sync_provider(&app, &pool, &id).await {
                    eprintln!("Sync of {} failed: {}", id, e);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_consent_needs_an_organisation_authority() {
        assert_eq!(admin_consent_tenant("common"), "organizations");
        assert_eq!(admin_consent_tenant("consumers"), "organizations");
        assert_eq!(admin_consent_tenant("contoso.com"), "contoso.com");
    }

    #[test]
    fn the_callback_page_offers_the_admin_link() {
        let page = callback_page("t", "d", Some(&("Approve".to_string(), "https://login.microsoftonline.com/x?a=1&b=\"2\"".to_string())));
        assert!(page.contains("<a href=\"https://login.microsoftonline.com/x?a=1&amp;b=&quot;2&quot;\">Approve</a>"));
    }

    #[tokio::test]
    async fn setup_reports_redirects_scopes_and_the_approval_link() {
        let pool = crate::test_helpers::test_pool().await;
        let before = connection_setup(&pool).await.unwrap();
        assert!(before.iter().all(|p| !p.ready));
        assert!(before[0].admin_consent_url.is_none());
        AppSetting::put(&pool, app_setting::MICROSOFT_CLIENT_ID, "abc").await.unwrap();
        AppSetting::put(&pool, app_setting::MICROSOFT_TENANT, "common").await.unwrap();
        AppSetting::put(&pool, app_setting::SLACK_CLIENT_ID, "1.2").await.unwrap();
        let after = connection_setup(&pool).await.unwrap();
        let microsoft = &after[0];
        assert!(microsoft.ready && microsoft.tenant.as_deref() == Some("common"));
        assert!(microsoft.admin_consent_url.as_deref().unwrap().starts_with("https://login.microsoftonline.com/organizations/v2.0/adminconsent?client_id=abc"));
        assert_eq!(microsoft.admin_scopes, vec!["ChannelMessage.ReadWrite".to_string(), "ChannelMessage.Read.All".to_string()]);
        assert_eq!(after[2].redirect_uri, "http://localhost:48731/callback/slack");
        assert!(after[2].ready && after[2].scopes.contains(&"chat:write".to_string()));
        assert!(after[1].scopes.iter().any(|s| s.ends_with("gmail.modify")));
    }

    #[tokio::test]
    async fn a_consent_refusal_page_carries_the_approval_link_when_an_admin_must_act() {
        let pool = crate::test_helpers::test_pool().await;
        AppSetting::put(&pool, app_setting::MICROSOFT_CLIENT_ID, "abc").await.unwrap();
        let needs_admin = consent_failure_page(&pool, "microsoft", "consent_required", "AADSTS65001: not consented").await;
        assert!(!needs_admin.ok && needs_admin.title.contains("approve"));
        assert!(needs_admin.link.unwrap().1.contains("/adminconsent?client_id=abc"));
        let declined = consent_failure_page(&pool, "microsoft", "access_denied", "AADSTS65004: declined").await;
        assert!(declined.link.is_none());
        let google = consent_failure_page(&pool, "google", "access_denied", "").await;
        assert_eq!(google.detail, "access_denied");
    }

    #[tokio::test]
    async fn browser_sign_in_needs_a_client_id_and_skips_form_providers() {
        let pool = crate::test_helpers::test_pool().await;
        let pending: PendingAuths = Arc::new(Mutex::new(HashMap::new()));
        assert!(start_auth(&pool, &pending, "slack").await.unwrap_err().contains("client id"));
        assert!(start_auth(&pool, &pending, "imap").await.unwrap_err().contains("form"));
        AppSetting::put(&pool, app_setting::MICROSOFT_TENANT, "no tenant here").await.unwrap();
        AppSetting::put(&pool, app_setting::MICROSOFT_CLIENT_ID, "abc").await.unwrap();
        assert!(start_auth(&pool, &pending, "microsoft").await.unwrap_err().contains("not a tenant"));
    }

    #[test]
    fn pkce_challenge_is_sha256_of_verifier() {
        let (verifier, challenge) = pkce_pair();
        assert!(verifier.len() >= 43);
        assert_eq!(challenge, URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())));
    }

    #[test]
    fn redirect_uri_carries_the_provider() {
        assert_eq!(redirect_uri("microsoft"), format!("http://localhost:{}/callback/microsoft", get_oauth_port()));
    }

    #[test]
    fn callback_page_escapes_detail() {
        let page = callback_page("x", "<b>", None);
        assert!(page.contains("&lt;b&gt;"));
    }
}
