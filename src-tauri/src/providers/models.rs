use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite, Row};
use tauri::{WebviewWindow, Emitter};
use std::sync::Arc;
use std::collections::HashMap;
use crate::models::{Conversation, ConversationResponse, ConversationsResponse, Profile};
use crate::providers::traits::{ProviderTrait, ConversationType, ResponseMessage};
use crate::utils::text_extraction::{extract_text_from_content, generate_summary};
use crate::utils::theme_generator::ProviderTheme;
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub profile_id: String,
    pub provider_type: String,
    pub name: String,
    pub external_id: Option<String>,
    pub logo_id: Option<String>,
    pub logo: Option<crate::models::Asset>,
    pub capabilities: ProviderCapabilities,
    pub is_enabled: bool,
    pub is_connected: bool,
    #[serde(skip_serializing)]
    pub auth: Option<ProviderAuth>,
    pub sync_interval: u64,
    pub namespace: Option<String>,
    pub delta_token: Option<String>,
    pub last_sync_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing)]
    pub config: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub theme: Option<ProviderTheme>,
    #[serde(default)]
    pub missing_scopes: Vec<String>,
    #[serde(default)]
    pub account: Option<crate::models::Account>,
    pub last_used_at: Option<DateTime<Utc>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAuth {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub token_type: String,
    pub scope: Option<String>,
}
impl ProviderAuth {
    fn secret_key(provider_id: &str, part: &str) -> String {
        format!("provider:{}:{}", provider_id, part)
    }

    pub fn from_columns(profile_id: &str, provider_id: &str, access: Option<String>, refresh: Option<String>, expires_at: DateTime<Utc>, token_type: String, scope: Option<String>) -> Option<Self> {
        use crate::services::security_service::{profile_vault, reveal};
        let vault = profile_vault(profile_id);
        let access_token = reveal(&vault, &Self::secret_key(provider_id, "access"), access)?;
        let refresh_token = reveal(&vault, &Self::secret_key(provider_id, "refresh"), refresh);
        Some(ProviderAuth { access_token, refresh_token, expires_at, token_type, scope })
    }

    pub fn mock(provider_id: &str) -> Self {
        ProviderAuth { access_token: format!("mock:{}", provider_id), refresh_token: None, expires_at: Utc::now() + chrono::Duration::days(36500), token_type: "Mock".into(), scope: None }
    }

    pub fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Option<Self>, String> {
        if row.try_get::<String, _>("provider_type").ok().as_deref() == Some("mock") {
            return Ok(Some(Self::mock(&row.get::<String, _>("id"))));
        }
        let access: Option<String> = row.try_get("access_token").ok().flatten();
        if access.is_none() {
            return Ok(None);
        }
        let expires_at_str: String = row.get("expires_at");
        let expires_at = DateTime::parse_from_rfc3339(&expires_at_str)
            .map_err(|e| format!("Failed to parse expires_at: {}", e))?
            .with_timezone(&Utc);
        Ok(Self::from_columns(
            &row.get::<String, _>("profile_id"),
            &row.get::<String, _>("id"),
            access,
            row.try_get("refresh_token").ok().flatten(),
            expires_at,
            row.try_get::<Option<String>, _>("token_type").ok().flatten().unwrap_or_else(|| "Bearer".to_string()),
            row.try_get("scope").ok().flatten(),
        ))
    }

    pub fn store(&self, profile_id: &str, provider_id: &str) -> Result<(String, Option<String>), String> {
        use crate::services::security_service::{profile_vault, seal};
        let vault = profile_vault(profile_id);
        let access = seal(&vault, &Self::secret_key(provider_id, "access"), Some(&self.access_token))?.unwrap_or_default();
        let refresh = seal(&vault, &Self::secret_key(provider_id, "refresh"), self.refresh_token.as_deref())?;
        Ok((access, refresh))
    }

    pub fn delete(profile_id: &str, provider_id: &str) -> Result<(), String> {
        use crate::services::security_service::{profile_vault, secrets};
        let vault = profile_vault(profile_id);
        secrets().delete(&vault, &Self::secret_key(provider_id, "access"))?;
        secrets().delete(&vault, &Self::secret_key(provider_id, "refresh"))
    }
}
pub const CURSOR_EMOJI: &str = "emoji_at";
pub const CURSOR_DIRECTORY: &str = "directory_at";
pub const CURSOR_FULL: &str = "full_at";
pub const FULL_SYNC_SECONDS: u64 = 3600;

pub fn declared_scopes(provider_type: &str) -> Vec<&'static str> {
    match provider_type {
        "microsoft" => crate::providers::microsoft::BASE_SCOPES.to_vec(),
        "slack" => crate::providers::slack::USER_SCOPES.to_vec(),
        "google" => crate::providers::google::SCOPES.to_vec(),
        _ => Vec::new(),
    }
}

fn scope_key(provider_type: &str, scope: &str) -> String {
    let scope = scope.trim().trim_start_matches("https://graph.microsoft.com/").to_lowercase();
    match (provider_type, scope.as_str()) {
        ("google", "email") => "https://www.googleapis.com/auth/userinfo.email".to_string(),
        ("google", "profile") => "https://www.googleapis.com/auth/userinfo.profile".to_string(),
        _ => scope,
    }
}

pub fn missing_scopes(provider_type: &str, granted: Option<&str>) -> Vec<String> {
    let Some(granted) = granted.map(str::trim).filter(|g| !g.is_empty()) else { return Vec::new() };
    let have: Vec<String> = granted.split([' ', ',']).filter(|s| !s.is_empty()).map(|s| scope_key(provider_type, s)).collect();
    declared_scopes(provider_type)
        .into_iter()
        .filter(|s| !matches!(*s, "offline_access" | "openid"))
        .filter(|s| !have.contains(&scope_key(provider_type, s)))
        .map(String::from)
        .collect()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    #[default]
    Unsupported,
    Native,
    Repost,
    Mail,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProviderCapabilities {
    pub can_send_messages: bool,
    pub can_edit_messages: bool,
    pub can_delete_messages: bool,
    pub can_create_conversations: bool,
    pub can_add_members: bool,
    pub supports_reactions: bool,
    pub supports_threads: bool,
    pub supports_attachments: bool,
    pub supports_presence: bool,
    pub supports_typing_indicators: bool,
    pub supports_read_receipts: bool,
    pub supports_reply_quote: bool,
    pub supports_forward: bool,
    pub reply_quote: Delivery,
    pub forward: Delivery,
    pub supports_calendar_responses: bool,
    pub live: bool,
    pub supports_unread_sync: bool,
    pub thread_broadcast: Delivery,
    pub drafts: Delivery,
    pub archive: Delivery,
    pub supports_flags: bool,
    pub supports_channel_create: bool,
    pub supports_channel_join: bool,
    pub supports_channel_archive: bool,
    pub supports_topic: bool,
    pub supports_purpose: bool,
    pub supports_search: bool,
    pub supports_user_groups: bool,
    pub supports_aliases: bool,
    pub signature_sync: bool,
    pub supports_auto_reply: bool,
}
impl Provider {
    pub fn from_row(row: sqlx::sqlite::SqliteRow) -> Result<Self, String> {
        use sqlx::Row;
        let auth = ProviderAuth::from_row(&row)?;
        let is_connected = auth.is_some();
        Ok(Provider {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            provider_type: row.get("provider_type"),
            name: row.get("name"),
            external_id: row.try_get::<Option<String>, _>("external_id").ok().flatten(),
            logo_id: row.try_get::<Option<String>, _>("logo_id").ok().flatten(),
            logo: None,
            capabilities: row.try_get::<String, _>("capabilities")
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            is_enabled: row.get("is_enabled"),
            is_connected,
            auth,
            sync_interval: row.get::<i64, _>("sync_interval") as u64,
            namespace: row.try_get::<Option<String>, _>("namespace").ok().flatten(),
            delta_token: row.try_get::<Option<String>, _>("delta_token").ok().flatten(),
            last_sync_at: row.try_get::<Option<String>, _>("last_sync_at").ok().flatten()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            config: row.try_get::<Option<String>, _>("config").ok().flatten(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            theme: None,
            missing_scopes: missing_scopes(&row.get::<String, _>("provider_type"), row.try_get::<Option<String>, _>("scope").ok().flatten().as_deref()),
            account: None,
            last_used_at: row.try_get::<Option<String>, _>("last_used_at").ok().flatten()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
        })
    }
    pub async fn load_account(&mut self, pool: &Pool<Sqlite>) -> Result<(), String> {
        self.account = crate::models::Account::get(pool, &self.id).await?;
        Ok(())
    }

    pub async fn put_last_used(pool: &Pool<Sqlite>, provider_id: &str) -> Result<(), String> {
        sqlx::query("UPDATE provider SET last_used_at = ?1 WHERE id = ?2")
            .bind(Utc::now().to_rfc3339())
            .bind(provider_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to note the account as used: {}", e))?;
        Ok(())
    }

    pub async fn load_logo(&mut self, pool: &Pool<Sqlite>) -> Result<(), String> {
        if let Some(logo_id) = &self.logo_id {
            let row = sqlx::query("SELECT * FROM assets WHERE id = ?1")
                .bind(logo_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("Failed to load logo asset: {}", e))?;
            
            if let Some(row) = row {
                let path: Option<String> = row.get("path");
                let url: Option<String> = row.get("url");
                
                println!("📸 Loading logo for provider {}: path={:?}, url={:?}", 
                    self.name, &path, &url);
                
                self.logo = Some(crate::models::Asset {
                    id: row.get("id"),
                    profile_id: row.get("profile_id"),
                    name: row.get("name"),
                    url,
                    asset_type: row.get("type"),
                    mime_type: row.get("mime_type"),
                    path,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                });
            } else {
                println!("⚠️ No logo asset found for logo_id: {}", logo_id);
            }
        } else {
            println!("ℹ️ Provider {} has no logo_id", self.name);
        }
        Ok(())
    }
    
    pub async fn load_from_db(pool: &Pool<Sqlite>, provider_id: &str) -> Result<Self, String> {
        let row = sqlx::query(
            "SELECT * FROM provider WHERE id = ?1"
        )
        .bind(provider_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to load provider: {}", e))?
        .ok_or_else(|| "Provider not found".to_string())?;
        let auth = ProviderAuth::from_row(&row)?;
        let is_connected = auth.is_some();
        Ok(Provider {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            provider_type: row.get("provider_type"),
            name: row.get("name"),
            external_id: row.try_get::<Option<String>, _>("external_id").ok().flatten(),
            logo_id: row.try_get::<Option<String>, _>("logo_id").ok().flatten(),
            logo: None,
            capabilities: row.try_get::<String, _>("capabilities")
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            is_enabled: row.get("is_enabled"),
            is_connected,
            auth,
            sync_interval: row.get::<i64, _>("sync_interval") as u64,
            namespace: row.try_get::<Option<String>, _>("namespace").ok().flatten(),
            delta_token: row.try_get::<Option<String>, _>("delta_token").ok().flatten(),
            last_sync_at: row.try_get::<Option<String>, _>("last_sync_at").ok().flatten()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            config: row.try_get::<Option<String>, _>("config").ok().flatten(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            theme: None,
            missing_scopes: missing_scopes(&row.get::<String, _>("provider_type"), row.try_get::<Option<String>, _>("scope").ok().flatten().as_deref()),
            account: None,
            last_used_at: row.try_get::<Option<String>, _>("last_used_at").ok().flatten()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
        })
    }
    pub async fn save_to_db(&self, pool: &Pool<Sqlite>) -> Result<(), String> {
        let (access_token, refresh_token, expires_at) = if let Some(auth) = &self.auth {
            let (access, refresh) = auth.store(&self.profile_id, &self.id)?;
            (Some(access), refresh, auth.expires_at.to_rfc3339())
        } else {
            (None, None, Utc::now().to_rfc3339())
        };
        sqlx::query(
            "INSERT OR REPLACE INTO provider (
                id, profile_id, provider_type, name, capabilities, is_enabled, 
                sync_interval, access_token, refresh_token, expires_at, 
                namespace, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
        )
        .bind(&self.id)
        .bind(&self.profile_id)
        .bind(&self.provider_type)
        .bind(&self.name)
        .bind(serde_json::to_string(&self.capabilities).unwrap_or_else(|_| "{}".to_string()))
        .bind(self.is_enabled)
        .bind(self.sync_interval as i64)
        .bind(access_token)
        .bind(refresh_token)
        .bind(expires_at)
        .bind(&self.namespace)
        .bind(self.created_at.to_rfc3339())
        .bind(self.updated_at.to_rfc3339())
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to save provider: {}", e))?;
        Ok(())
    }
    pub async fn update_auth(&mut self, pool: &Pool<Sqlite>, auth: ProviderAuth) -> Result<(), String> {
        self.auth = Some(auth.clone());
        self.is_connected = true;
        self.updated_at = Utc::now();
        let (access, refresh) = auth.store(&self.profile_id, &self.id)?;
        sqlx::query(
            "UPDATE provider SET access_token = ?1, refresh_token = ?2, expires_at = ?3, updated_at = ?4, scope = COALESCE(?6, scope) WHERE id = ?5"
        )
        .bind(&access)
        .bind(&refresh)
        .bind(auth.expires_at.to_rfc3339())
        .bind(self.updated_at.to_rfc3339())
        .bind(&self.id)
        .bind(&auth.scope)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update auth: {}", e))?;
        Ok(())
    }
    
    pub fn cursors(&self) -> HashMap<String, String> {
        self.delta_token.as_deref().and_then(|t| serde_json::from_str::<HashMap<String, String>>(t).ok()).unwrap_or_default().into_iter().filter(|(_, v)| !v.is_empty()).collect()
    }

    pub async fn put_cursors(&mut self, pool: &Pool<Sqlite>, updates: &HashMap<String, String>) -> Result<(), String> {
        let stored: Option<String> = sqlx::query_scalar("SELECT delta_token FROM provider WHERE id = ?1")
            .bind(&self.id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to read the sync cursors: {}", e))?
            .flatten();
        let mut cursors: HashMap<String, String> = stored.as_deref().and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default();
        cursors.extend(updates.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.delta_token = Some(serde_json::to_string(&cursors).map_err(|e| e.to_string())?);
        sqlx::query("UPDATE provider SET delta_token = ?1 WHERE id = ?2")
            .bind(&self.delta_token)
            .bind(&self.id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to store the sync cursors: {}", e))?;
        Ok(())
    }

    pub fn due(&self, cursor: &str, interval_seconds: u64, now: DateTime<Utc>) -> bool {
        match self.cursors().get(cursor).and_then(|at| DateTime::parse_from_rfc3339(at).ok()) {
            Some(at) => now - at.with_timezone(&Utc) >= chrono::Duration::seconds(interval_seconds as i64),
            None => true,
        }
    }
    
    pub async fn validate(&mut self, pool: &Pool<Sqlite>) -> Result<(), String> {
        let provider_impl = crate::services::connection_service::provider_impl(pool, &self.provider_type, self.config.as_deref()).await?;
        crate::services::connection_service::ensure_fresh(pool, self, provider_impl).await?;
        Ok(())
    }
    
    pub async fn sync(
        &self,
        pool: &Pool<Sqlite>,
        window: &WebviewWindow,
        provider_impl: Arc<dyn ProviderTrait>,
    ) -> Result<(), String> {
        self.ingest(pool, Some(window), provider_impl).await
    }

    pub async fn ingest(
        &self,
        pool: &Pool<Sqlite>,
        window: Option<&WebviewWindow>,
        provider_impl: Arc<dyn ProviderTrait>,
    ) -> Result<(), String> {
        println!("🔄 Starting sync for provider {} ({})", self.id, self.provider_type);
        let auth = self.auth.as_ref()
            .ok_or_else(|| "No auth tokens available".to_string())?;
        
        let history_days = crate::models::AppSetting::get_u64(pool, crate::models::app_setting::SYNC_HISTORY_DAYS).await.unwrap_or_else(|_| crate::models::app_setting::get_default_u64(crate::models::app_setting::SYNC_HISTORY_DAYS));
        let since = match self.last_sync_at {
            Some(at) => at - chrono::Duration::hours(1),
            None => Utc::now() - chrono::Duration::days(history_days as i64),
        };
        let directory_every = crate::models::AppSetting::get_u64(pool, crate::models::app_setting::DIRECTORY_SYNC_INTERVAL_SECONDS).await.unwrap_or_else(|_| crate::models::app_setting::get_default_u64(crate::models::app_setting::DIRECTORY_SYNC_INTERVAL_SECONDS));
        if self.due(CURSOR_DIRECTORY, directory_every, Utc::now()) {
            match provider_impl.get_people(auth).await {
                Ok(people) => {
                    for person in people.iter().filter(|p| !p.id.is_empty()) {
                        let user_id = crate::models::user::User::upsert_from_response(pool, person).await?;
                        let _ = crate::models::user::ProviderUser::create_or_update(pool, user_id, self.id.clone(), person.id.clone()).await;
                    }
                    self.clone().put_cursors(pool, &HashMap::from([(CURSOR_DIRECTORY.to_string(), Utc::now().to_rfc3339())])).await?;
                }
                Err(e) => eprintln!("The directory of {} was not read: {}", self.name, e),
            }
        }
        let cursors = self.cursors();
        let delta = match provider_impl.get_delta(auth, &cursors, since).await {
            Ok(delta) => Some(delta),
            Err(e) => {
                eprintln!("Changes of {} were not read: {}", self.name, e);
                None
            }
        };
        let full = self.due(CURSOR_FULL, FULL_SYNC_SECONDS, Utc::now());
        let narrowed: Option<Vec<String>> = delta.as_ref().filter(|d| d.complete && !full).map(|d| d.conversations.clone());
        let conversations = provider_impl.sync_conversations(auth, Some(since)).await
            .map_err(|e| format!("Failed to sync conversations: {}", e))?;
        
        println!("📊 Fetched {} conversations from provider", conversations.len());
        
        let mut tx = pool.begin().await
            .map_err(|e| format!("Failed to start transaction: {}", e))?;
        
        println!("📝 Transaction started");
        
        let mut message_fetch_tasks = Vec::new();
        let mut member_tasks = Vec::new();
        let mut processed_count = 0;
        
        let total_conversations = conversations.len();
        for conversation in conversations {
            processed_count += 1;
            let mut fresh = false;
            println!("  Processing conversation {}/{}: {}", processed_count, total_conversations, conversation.name);
            let conversation_id = if conversation.conversation_type == ConversationType::Email 
                && conversation.external_id.is_empty() {
                let generated_id = crate::utils::content_hash(format!("{}{}{}", self.id, conversation.name, serde_json::to_string(&conversation.members).unwrap_or_default()).as_bytes());
                
                let existing_row = sqlx::query(
                    "SELECT id FROM conversation WHERE id = ?1"
                )
                .bind(&generated_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| format!("Failed to check existing email conversation: {}", e))?;
                
                if existing_row.is_some() {
                    generated_id
                } else {
                    let now = Utc::now().to_rfc3339();
                    
                    sqlx::query(
                        r#"INSERT INTO conversation (
                            id, name, type, namespace, last_activity, created_at, updated_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#
                    )
                    .bind(&generated_id)
                    .bind(&conversation.name)
                    .bind(serde_json::to_string(&conversation.conversation_type).unwrap_or_else(|_| "\"unknown\"".to_string()).trim_matches('"'))
                    .bind(&self.namespace)
                    .bind(conversation.last_activity.to_rfc3339())
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to insert email conversation: {}", e))?;
                    
                    let cp_id = Uuid::new_v4().to_string();
                    sqlx::query(
                        r#"INSERT INTO conversation_provider (
                            id, conversation_id, provider_id, external_id, is_primary,
                            created_at, updated_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#
                    )
                    .bind(&cp_id)
                    .bind(&generated_id)
                    .bind(&self.id)
                    .bind(&conversation.external_id)
                    .bind(true)
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to insert conversation_provider: {}", e))?;
                    
                    fresh = true;
                    generated_id
                }
            } else {
                let existing_row = sqlx::query(
                    "SELECT c.id FROM conversation c 
                     JOIN conversation_provider cp ON c.id = cp.conversation_id 
                     WHERE cp.provider_id = ?1 AND cp.external_id = ?2"
                )
                .bind(&self.id)
                .bind(&conversation.external_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| format!("Failed to check existing conversation: {}", e))?;
                
                if let Some(row) = existing_row {
                    row.get("id")
                } else {
                    let id = Uuid::new_v4().to_string();
                    let now = Utc::now().to_rfc3339();
                    
                    sqlx::query(
                        r#"INSERT INTO conversation (
                            id, name, type, namespace, last_activity, created_at, updated_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#
                    )
                    .bind(&id)
                    .bind(&conversation.name)
                    .bind(serde_json::to_string(&conversation.conversation_type).unwrap_or_else(|_| "\"unknown\"".to_string()).trim_matches('"'))
                    .bind(&self.namespace)
                    .bind(conversation.last_activity.to_rfc3339())
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to insert conversation: {}", e))?;
                    
                    let cp_id = Uuid::new_v4().to_string();
                    sqlx::query(
                        r#"INSERT INTO conversation_provider (
                            id, conversation_id, provider_id, external_id, is_primary,
                            created_at, updated_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#
                    )
                    .bind(&cp_id)
                    .bind(&id)
                    .bind(&self.id)
                    .bind(&conversation.external_id)
                    .bind(true)
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to insert conversation_provider: {}", e))?;
                    
                    fresh = true;
                    id
                }
            };
            
            // Store members to process after transaction
            member_tasks.push((conversation_id.clone(), conversation.members.clone()));
            
            let external_id = conversation.external_id.clone();
            if !fresh && !external_id.is_empty() && narrowed.as_ref().is_some_and(|touched| !touched.contains(&external_id)) {
                continue;
            }
            message_fetch_tasks.push((
                conversation_id.clone(),
                external_id,
                provider_impl.clone(),
                auth.clone(),
                pool.clone(),
                window.cloned(),
                self.id.clone(),
                self.name.clone(),
                since
            ));
        }
        
        tx.commit().await
            .map_err(|e| format!("Failed to commit transaction: {}", e))?;
        
        println!("✅ Transaction committed");
        
        // Process members after transaction to avoid deadlock
        println!("👥 Processing members for {} conversations", member_tasks.len());
        use crate::models::user::{User, ProviderUser};
        for (conversation_id, members) in member_tasks {
            for member in members {
                let user_id = User::upsert_from_response(pool, &member).await?;
                let _ = ProviderUser::create_or_update(
                    pool,
                    user_id.clone(),
                    self.id.clone(),
                    member.id.clone(),
                ).await;
                sqlx::query("INSERT OR IGNORE INTO conversation_member (id, conversation_id, user_id, role, added_at) VALUES (?1, ?2, ?3, 'member', ?4)")
                    .bind(Uuid::new_v4().to_string())
                    .bind(&conversation_id)
                    .bind(&user_id)
                    .bind(Utc::now().to_rfc3339())
                    .execute(pool)
                    .await
                    .map_err(|e| format!("Failed to save member: {}", e))?;
            }
        }
        
        println!("📥 Now fetching messages for {} conversations", message_fetch_tasks.len());
        
        for (conversation_id_clone, external_id, provider_impl_clone, auth_clone, pool_clone, window_clone, provider_id, provider_name, since) in message_fetch_tasks {
            if let Err(e) = fetch_and_update_conversation_messages(
                conversation_id_clone,
                external_id, 
                provider_impl_clone,
                auth_clone,
                pool_clone,
                window_clone,
                provider_id,
                provider_name,
                since
            ).await {
                eprintln!("❌ Failed to fetch messages: {}", e);
            }
        }
        
        if let Some(delta) = delta {
            put_changes(pool, &self.id, &delta.changes).await?;
            if delta.cursors != cursors {
                let mut updates: HashMap<String, String> = delta.cursors.iter().filter(|(k, v)| cursors.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), v.clone())).collect();
                for gone in cursors.keys().filter(|k| !delta.cursors.contains_key(*k)) {
                    updates.insert(gone.clone(), String::new());
                }
                self.clone().put_cursors(pool, &updates).await?;
            }
        }
        if full {
            self.clone().put_cursors(pool, &HashMap::from([(CURSOR_FULL.to_string(), Utc::now().to_rfc3339())])).await?;
        }

        let rows = Conversation::get_rows_for_active_profile(pool).await?;
        
        let mut conversations = Vec::new();
        for row in rows {
            if let Ok(mut conversation) = Conversation::from_row(row) {
                // Update summary if needed
                conversation.update_summary(pool).await.ok();
                
                // Load providers for each conversation
                if let Ok(provider_rows) = sqlx::query(
                    "SELECT p.* FROM provider p 
                     JOIN conversation_provider cp ON p.id = cp.provider_id 
                     WHERE cp.conversation_id = ?1"
                )
                .bind(&conversation.id)
                .fetch_all(pool)
                .await {
                    for prow in provider_rows {
                        if let Ok(mut provider) = Provider::from_row(prow) {
                            let _ = provider.load_logo(pool).await;
                            conversation.providers.push(provider);
                        }
                    }
                }
                conversation.tags = crate::models::ObjectTag::get_conversation(pool, &conversation.id).await.unwrap_or_default();
                conversations.push(conversation);
            }
        }
        
        if let Some(window) = window {
            window.emit("conversations-updated", &ConversationsResponse {
                data: conversations.clone(),
                is_loading: false,
            })
                .map_err(|e| format!("Failed to emit event: {}", e))?;
        }
        
        println!("✅ Sync complete for provider {}, emitted {} conversations", self.id, conversations.len());
        
        Ok(())
    }
    
    pub async fn more_messages(&mut self, _pool: &Pool<Sqlite>, conversation_id: &str) -> Result<usize, String> {
        println!("More messages not implemented for conversation {}", conversation_id);
        Ok(0)
    }
    
    pub async fn sync_conversation(
        &self,
        pool: &Pool<Sqlite>,
        conversation_id: &str,
        window: &tauri::Window,
    ) -> Result<(), String> {
        use crate::providers::traits::ProviderTrait;
        use std::sync::Arc;
        
        if !self.is_enabled || self.auth.is_none() {
            return Ok(());
        }
        
        // Get external_id for this conversation
        let external_id = sqlx::query_scalar::<_, String>(
            "SELECT external_id FROM conversation_provider WHERE conversation_id = ?1 AND provider_id = ?2"
        )
        .bind(conversation_id)
        .bind(&self.id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to get external_id: {}", e))?;
        
        let Some(ext_id) = external_id else {
            return Ok(());
        };
        
        let provider_impl: Arc<dyn ProviderTrait> = crate::services::connection_service::provider_impl(pool, &self.provider_type, self.config.as_deref()).await?;
        
        // Sync messages for this conversation
        let messages = provider_impl.sync_messages(
            self.auth.as_ref().unwrap(),
            &ext_id,
            None,
            Some(50)
        ).await?;
        
        println!("✅ Synced {} messages for conversation {} from provider {}", 
            messages.len(), conversation_id, self.name);
        
        // Store messages in database
        use crate::services::{MessageService, MessageLoadOptions};
        // Save messages to database (this now includes user creation)
        save_messages_to_db(
            pool,
            conversation_id,
            &self.id,
            &self.name,
            &messages
        ).await?;
        
        // Load and emit updated messages
        let message_service = MessageService::new(pool.clone());
        let load_options = MessageLoadOptions {
            load_reactions: true,
            load_assets: true,
            load_sender_details: true,
            limit: Some(50),
            offset: None,
        };
        
        if let Ok(messages) = message_service.load_conversation_messages(conversation_id, load_options).await {
            window.emit("conversation-messages-updated", &messages)
                .map_err(|e| format!("Failed to emit messages event: {}", e))?;
        }
        
        Ok(())
    }
}


async fn fetch_and_update_conversation_messages(
    conversation_id: String,
    external_id: String,
    provider_impl: Arc<dyn ProviderTrait>,
    auth: ProviderAuth,
    pool: Pool<Sqlite>,
    window: Option<WebviewWindow>,
    provider_id: String,
    provider_name: String,
    since: DateTime<Utc>,
) -> Result<(), String> {
    let messages = provider_impl.sync_messages(&auth, &external_id, Some(since), None).await?;
    if messages.is_empty() {
        return Ok(());
    }
    let pins = match provider_impl.get_pins(&auth, &external_id).await {
        Ok(pins) => pins,
        Err(e) => {
            eprintln!("Pins of {} were not read: {}", external_id, e);
            None
        }
    };
    put_arrived(&pool, window.as_ref(), &conversation_id, &provider_id, &provider_name, &messages, pins.as_deref()).await
}

pub(crate) async fn put_arrived(
    pool: &Pool<Sqlite>,
    window: Option<&WebviewWindow>,
    conversation_id: &str,
    provider_id: &str,
    provider_name: &str,
    messages: &[ResponseMessage],
    pins: Option<&[String]>,
) -> Result<(), String> {
    let pool = pool.clone();
    let conversation_id = conversation_id.to_string();
    let provider_id = provider_id.to_string();
    let saved = save_messages_to_db(&pool, &conversation_id, &provider_id, provider_name, messages).await?;
    let last_message_id = match saved {
        Some(_) => sqlx::query_scalar::<_, String>("SELECT id FROM message WHERE conversation_id = ?1 AND deleted_datetime IS NULL ORDER BY timestamp DESC LIMIT 1")
            .bind(&conversation_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| format!("Failed to find the newest message: {}", e))?,
        None => None,
    };
    let fresh: Vec<String> = {
        let mut ids = Vec::new();
        for message in messages {
            if let Some(id) = sqlx::query_scalar::<_, String>("SELECT id FROM message WHERE provider_id = ?1 AND external_id = ?2 AND is_read = 0")
                .bind(&provider_id)
                .bind(&message.external_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| format!("Failed to read the arrived message: {}", e))?
            {
                ids.push(id);
            }
        }
        ids
    };

    let current_last_id = sqlx::query("SELECT last_id FROM conversation WHERE id = ?1")
        .bind(&conversation_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| format!("Failed to get conversation: {}", e))?
        .and_then(|row| row.try_get::<Option<String>, _>("last_id").ok())
        .flatten();
    
    let last_id_changed = last_message_id.is_some() && current_last_id != last_message_id;
    if let Some(pinned) = pins {
        put_pins(&pool, &conversation_id, &provider_id, pinned).await?;
    }
    
    // Always generate summary when last_id changes
    let summary = if last_id_changed && last_message_id.is_some() {
        generate_summary_from_db_messages(&pool, &conversation_id, last_message_id.as_ref()).await?
    } else {
        None
    };
    
    if last_id_changed {
        update_conversation(&pool, &conversation_id, last_message_id.as_ref(), summary.as_ref()).await?;
    }
    sqlx::query(
        r#"UPDATE conversation SET
            last_activity = COALESCE((SELECT MAX(timestamp) FROM message WHERE conversation_id = ?1 AND deleted_datetime IS NULL), last_activity),
            unread_count = (SELECT COUNT(*) FROM message WHERE conversation_id = ?1 AND is_read = 0 AND deleted_datetime IS NULL
                AND sender_id != COALESCE((SELECT user_id FROM profile WHERE active = 1), ''))
        WHERE id = ?1"#,
    )
    .bind(&conversation_id)
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to refresh conversation counters: {}", e))?;
    let Some(window) = window else {
        return Ok(());
    };
    if !fresh.is_empty() {
        let notify_pool = pool.clone();
        let notify_app = tauri::Manager::app_handle(window).clone();
        let notify_id = conversation_id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::services::notification_service::put_messages(&notify_app, &notify_pool, &notify_id, &fresh).await {
                eprintln!("New messages were not announced: {}", e);
            }
        });
    }
    if last_id_changed {
        let sort_pool = pool.clone();
        let sort_id = conversation_id.clone();
        let sort_window = window.clone();
        let arrived = last_message_id.clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(Some(profile)) = crate::models::Profile::get_active(&sort_pool).await {
                match crate::services::people_service::rematch(&sort_pool, &profile.id).await {
                    Ok(_) => {
                        if let Ok(people) = crate::models::Person::all(&sort_pool, &profile.id).await {
                            sort_window.emit("people-updated", &people).ok();
                        }
                    }
                    Err(e) => eprintln!("People match failed: {}", e),
                }
            }
            if let Some(message_id) = arrived {
                crate::services::action_service::on_arrival(tauri::Manager::app_handle(&sort_window).clone(), message_id);
            }
            if let Ok(Some(profile)) = crate::models::Profile::get_active(&sort_pool).await {
                match crate::services::case_service::scan_conversation(&sort_pool, &profile.id, &sort_id).await {
                    Ok(true) => {
                        if let Ok(cases) = crate::models::case::CaseFile::all(&sort_pool, &profile.id).await {
                            sort_window.emit("cases-updated", &cases).ok();
                        }
                    }
                    Ok(false) => {}
                    Err(e) => eprintln!("Case matching failed: {}", e),
                }
            }
        });
    }
    
    // Emit updated conversation to frontend
    emit_conversation_update(&pool, &window, &conversation_id).await?;
    
    Ok(())
}


pub const CHANGED_EVENT: &str = "conversation-changed";
pub const TYPING_EVENT: &str = "typing-updated";
pub const PRESENCE_EVENT: &str = "presence-updated";

#[derive(Debug, Clone, Serialize)]
pub struct TypingUpdate {
    pub conversation_id: String,
    pub user_id: Option<String>,
    pub user_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PresenceUpdate {
    pub user_id: String,
    pub status: String,
    pub status_text: Option<String>,
    pub status_emoji: Option<String>,
}

pub(crate) async fn get_conversation_id(pool: &Pool<Sqlite>, provider_id: &str, external_id: &str) -> Result<Option<String>, String> {
    sqlx::query_scalar("SELECT conversation_id FROM conversation_provider WHERE provider_id = ?1 AND external_id = ?2")
        .bind(provider_id)
        .bind(external_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to find the conversation: {}", e))
}

pub(crate) async fn put_live(
    pool: &Pool<Sqlite>,
    window: Option<&WebviewWindow>,
    provider: &Provider,
    provider_impl: Arc<dyn ProviderTrait>,
    event: crate::providers::traits::LiveEvent,
) -> Result<bool, String> {
    use crate::providers::traits::LiveEvent;
    use crate::models::user::ProviderUser;
    match event {
        LiveEvent::Message { message } => {
            let Some(conversation_id) = get_conversation_id(pool, &provider.id, &message.conversation_external_id).await? else { return Ok(true) };
            put_arrived(pool, window, &conversation_id, &provider.id, &provider.name, std::slice::from_ref(&message), None).await?;
            if let Some(window) = window {
                window.emit(CHANGED_EVENT, &conversation_id).ok();
            }
            Ok(false)
        }
        LiveEvent::Change { change } => {
            for conversation_id in put_changes(pool, &provider.id, std::slice::from_ref(&change)).await? {
                if let Some(window) = window {
                    emit_conversation_update(pool, window, &conversation_id).await?;
                    window.emit(CHANGED_EVENT, &conversation_id).ok();
                }
            }
            Ok(false)
        }
        LiveEvent::Touched { conversation_external_id } => {
            let Some(conversation_id) = get_conversation_id(pool, &provider.id, &conversation_external_id).await? else { return Ok(true) };
            let auth = provider.auth.as_ref().ok_or_else(|| "No auth tokens available".to_string())?;
            let newest: Option<String> = sqlx::query_scalar("SELECT MAX(timestamp) FROM message WHERE conversation_id = ?1 AND provider_id = ?2")
                .bind(&conversation_id)
                .bind(&provider.id)
                .fetch_one(pool)
                .await
                .map_err(|e| format!("Failed to read the newest message: {}", e))?;
            let since = newest.and_then(|n| DateTime::parse_from_rfc3339(&n).ok()).map(|n| n.with_timezone(&Utc) - chrono::Duration::minutes(5)).unwrap_or_else(|| Utc::now() - chrono::Duration::days(1));
            let messages = provider_impl.sync_messages(auth, &conversation_external_id, Some(since), None).await?;
            if !messages.is_empty() {
                put_arrived(pool, window, &conversation_id, &provider.id, &provider.name, &messages, None).await?;
            }
            if let Some(window) = window {
                window.emit(CHANGED_EVENT, &conversation_id).ok();
            }
            Ok(false)
        }
        LiveEvent::Conversation { conversation } => Ok(get_conversation_id(pool, &provider.id, &conversation.external_id).await?.is_none()),
        LiveEvent::Typing { conversation_external_id, user } => {
            let Some(conversation_id) = get_conversation_id(pool, &provider.id, &conversation_external_id).await? else { return Ok(false) };
            let local = ProviderUser::find_by_external_id(pool, &provider.id, &user.id).await?;
            let user_name = match &local {
                Some(found) => sqlx::query_scalar::<_, Option<String>>("SELECT display_name FROM user WHERE id = ?1").bind(&found.user_id).fetch_optional(pool).await.map_err(|e| e.to_string())?.flatten(),
                None => None,
            }
            .or(user.display_name.clone())
            .unwrap_or_else(|| user.id.clone());
            if let Some(window) = window {
                window.emit(TYPING_EVENT, &TypingUpdate { conversation_id, user_id: local.map(|l| l.user_id), user_name }).ok();
            }
            Ok(false)
        }
        LiveEvent::Presence { user_external_id, status, text, emoji } => {
            let Some(local) = ProviderUser::find_by_external_id(pool, &provider.id, &user_external_id).await? else { return Ok(false) };
            if let Some(window) = window {
                window.emit(PRESENCE_EVENT, &PresenceUpdate { user_id: local.user_id, status: crate::services::people_service::presence_status(&status), status_text: text, status_emoji: emoji }).ok();
            }
            Ok(false)
        }
    }
}

pub async fn generate_summary_from_db_messages(
    pool: &Pool<Sqlite>,
    conversation_id: &str,
    last_message_id: Option<&String>,
) -> Result<Option<String>, String> {
    // Get the 5 most recent messages from the database
    let query = if let Some(last_id) = last_message_id {
        // Start from last_id and get 5 messages including it
        sqlx::query(
            r#"SELECT m.content, m.content_type, m.sender_name, m.timestamp
               FROM message m
               WHERE m.conversation_id = ?1
               AND m.timestamp <= (SELECT timestamp FROM message WHERE id = ?2)
               ORDER BY m.timestamp DESC
               LIMIT 5"#
        )
        .bind(conversation_id)
        .bind(last_id)
    } else {
        // Just get the 5 most recent messages
        sqlx::query(
            r#"SELECT m.content, m.content_type, m.sender_name, m.timestamp
               FROM message m
               WHERE m.conversation_id = ?1
               ORDER BY m.timestamp DESC
               LIMIT 5"#
        )
        .bind(conversation_id)
    };
    
    let rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch messages for summary: {}", e))?;
    
    if rows.is_empty() {
        return Ok(None);
    }
    
    let mut text_parts = Vec::new();
    
    // Process messages in reverse order (oldest to newest) for better summary
    for row in rows.iter().rev() {
        let content: String = row.get("content");
        let content_type: String = row.get("content_type");
        let sender_name: Option<String> = row.try_get("sender_name").ok();
        
        let unsigned = if content_type == "html" { crate::content::html::split_signature(&content).0 } else { crate::content::mail::split_signature_text(&content).0 };
        let text = extract_text_from_content(&unsigned, &content_type);
        if !text.is_empty() {
            let sender = sender_name.as_deref().unwrap_or("Unknown");
            text_parts.push(format!("{}: {}", sender, text));
        }
    }
    
    if text_parts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(generate_summary(&text_parts.join(" "), 200)))
    }
}


pub(crate) async fn put_changes(pool: &Pool<Sqlite>, provider_id: &str, changes: &[crate::providers::traits::RemoteChange]) -> Result<Vec<String>, String> {
    let now = Utc::now().to_rfc3339();
    let mut touched: Vec<String> = Vec::new();
    let profile_id: Option<String> = sqlx::query_scalar("SELECT profile_id FROM provider WHERE id = ?1")
        .bind(provider_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to read the connection: {}", e))?;
    for change in changes {
        let row: Option<(String, Option<String>)> = sqlx::query_as("SELECT id, conversation_id FROM message WHERE provider_id = ?1 AND external_id = ?2")
            .bind(provider_id)
            .bind(&change.message_external_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to find the changed message: {}", e))?;
        let Some((id, conversation_id)) = row else { continue };
        if change.deleted {
            sqlx::query("UPDATE message SET deleted_datetime = COALESCE(deleted_datetime, ?1), updated_at = ?1 WHERE id = ?2").bind(&now).bind(&id).execute(pool).await.map_err(|e| format!("Failed to mark the message deleted: {}", e))?;
        }
        if let Some(read) = change.is_read {
            sqlx::query("UPDATE message SET is_read = ?1, updated_at = ?2 WHERE id = ?3").bind(read).bind(&now).bind(&id).execute(pool).await.map_err(|e| format!("Failed to store the read state: {}", e))?;
        }
        if let Some(labels) = &change.labels {
            sqlx::query("UPDATE message SET metadata = json_set(CASE WHEN json_valid(metadata) THEN metadata ELSE '{}' END, '$.labels', json(?1)), updated_at = ?2 WHERE id = ?3")
                .bind(serde_json::to_string(labels).unwrap_or_else(|_| "[]".to_string()))
                .bind(&now)
                .bind(&id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to store the labels: {}", e))?;
        }
        if let Some(reactions) = &change.reactions {
            let list: Vec<serde_json::Value> = reactions.iter().map(|(emoji, user)| serde_json::json!({ "emoji": crate::content::emoji::canonical_reaction(emoji), "user": user })).collect();
            sqlx::query("UPDATE message SET metadata = json_set(CASE WHEN json_valid(metadata) THEN metadata ELSE '{}' END, '$.reactions', json(?1)), updated_at = ?2 WHERE id = ?3")
                .bind(serde_json::Value::Array(list).to_string())
                .bind(&now)
                .bind(&id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to store the reactions: {}", e))?;
        }
        if let Some(content) = &change.content {
            let edited = change.edited_at.clone().unwrap_or_else(|| now.clone());
            sqlx::query("UPDATE message SET content = ?1, summary = NULL, last_modified_datetime = ?2, metadata = json_set(CASE WHEN json_valid(metadata) THEN metadata ELSE '{}' END, '$.edited_at', ?2), updated_at = ?3 WHERE id = ?4")
                .bind(content)
                .bind(&edited)
                .bind(&now)
                .bind(&id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to store the edit: {}", e))?;
        }
        if let (Some(pinned), Some(conversation_id)) = (change.pinned, conversation_id.as_ref()) {
            let statement = if pinned {
                "INSERT OR IGNORE INTO pinned_message (id, conversation_id, message_id, pinned_by, created_at) VALUES (?1, ?2, ?3, 'provider', ?4)"
            } else {
                "DELETE FROM pinned_message WHERE ?1 IS NOT NULL AND conversation_id = ?2 AND message_id = ?3 AND ?4 IS NOT NULL"
            };
            sqlx::query(statement).bind(Uuid::new_v4().to_string()).bind(conversation_id).bind(&id).bind(&now).execute(pool).await.map_err(|e| format!("Failed to store the pin: {}", e))?;
        }
        if let (Some(flagged), Some(profile_id)) = (change.flagged, profile_id.as_ref()) {
            let statement = if flagged {
                "INSERT OR IGNORE INTO bookmarked_message (id, message_id, profile_id, created_at) VALUES (?1, ?2, ?3, ?4)"
            } else {
                "DELETE FROM bookmarked_message WHERE ?1 IS NOT NULL AND message_id = ?2 AND profile_id = ?3 AND ?4 IS NOT NULL"
            };
            sqlx::query(statement).bind(Uuid::new_v4().to_string()).bind(&id).bind(profile_id).bind(&now).execute(pool).await.map_err(|e| format!("Failed to store the flag: {}", e))?;
        }
        if let Some(conversation_id) = conversation_id.filter(|c| !touched.contains(c)) {
            touched.push(conversation_id);
        }
    }
    for conversation_id in &touched {
        sqlx::query(
            "UPDATE conversation SET unread_count = (SELECT COUNT(*) FROM message WHERE conversation_id = ?1 AND is_read = 0 AND deleted_datetime IS NULL
                AND sender_id != COALESCE((SELECT user_id FROM profile WHERE active = 1), '')) WHERE id = ?1",
        )
        .bind(conversation_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to refresh the unread count: {}", e))?;
    }
    Ok(touched)
}

async fn put_pins(pool: &Pool<Sqlite>, conversation_id: &str, provider_id: &str, pinned: &[String]) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let mut kept: Vec<String> = Vec::new();
    for external_id in pinned {
        let id: Option<String> = sqlx::query_scalar("SELECT id FROM message WHERE provider_id = ?1 AND external_id = ?2")
            .bind(provider_id)
            .bind(external_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to find the pinned message: {}", e))?;
        let Some(id) = id else { continue };
        sqlx::query("INSERT OR IGNORE INTO pinned_message (id, conversation_id, message_id, pinned_by, created_at) VALUES (?1, ?2, ?3, 'provider', ?4)")
            .bind(Uuid::new_v4().to_string())
            .bind(conversation_id)
            .bind(&id)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to keep the pin: {}", e))?;
        kept.push(id);
    }
    let stale: Vec<String> = sqlx::query_scalar("SELECT pm.message_id FROM pinned_message pm JOIN message m ON m.id = pm.message_id WHERE pm.conversation_id = ?1 AND m.provider_id = ?2")
        .bind(conversation_id)
        .bind(provider_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to read the pins: {}", e))?;
    for message_id in stale.into_iter().filter(|id| !kept.contains(id)) {
        sqlx::query("DELETE FROM pinned_message WHERE conversation_id = ?1 AND message_id = ?2").bind(conversation_id).bind(&message_id).execute(pool).await.map_err(|e| format!("Failed to drop the pin: {}", e))?;
    }
    Ok(())
}

async fn save_messages_to_db(
    pool: &Pool<Sqlite>,
    conversation_id: &str,
    provider_id: &str,
    provider_name: &str,
    messages: &[ResponseMessage],
) -> Result<Option<String>, String> {
    use crate::models::user::{User, ProviderUser};
    
    let now = Utc::now().to_rfc3339();
    let mut last_message_id = None;
    let read_from_provider = sqlx::query_scalar::<_, String>("SELECT capabilities FROM provider WHERE id = ?1")
        .bind(provider_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to read the connection: {}", e))?
        .and_then(|c| serde_json::from_str::<ProviderCapabilities>(&c).ok())
        .is_some_and(|c| c.supports_unread_sync);
    
    for (idx, message) in messages.iter().enumerate() {
        let mut message = message.clone();
        crate::content::emoji::canonical_message(&mut message);
        let message = &message;
        let message_id = get_or_create_message_id(pool, provider_id, &message.external_id).await?;
        
        // First message is the newest
        if idx == 0 {
            last_message_id = Some(message_id.clone());
        }
        
        let user_id = User::upsert_from_response(pool, &message.sender).await?;
        
        // Create or update ProviderUser to link external ID to our user ID
        let _ = ProviderUser::create_or_update(
            pool,
            user_id.clone(),
            provider_id.to_string(),
            message.sender.id.clone(), // This is the external provider ID
        ).await;
        
        let recipients = |people: &Vec<crate::providers::traits::ResponseUser>| -> String {
            serde_json::to_string(&people.iter().map(|p| serde_json::json!({ "name": p.display_name, "email": p.email })).collect::<Vec<_>>()).unwrap_or_else(|_| "[]".to_string())
        };
        let attachments = serde_json::to_string(
            &message
                .attachments
                .iter()
                .map(|a| crate::models::Attachment { id: a.external_id.clone(), name: a.name.clone(), content_type: a.content_type.clone(), size: a.size as i64, is_inline: a.content_id.is_some(), content_id: a.content_id.clone(), url: a.url.clone() })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| "[]".to_string());
        let edited_at = message.metadata.get("edited_at").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| message.timestamp.to_rfc3339());
        let deleted_at = message.metadata.get("deleted_at").and_then(|v| v.as_str()).map(String::from);
        let mut stored = message.metadata.clone();
        if let Some(mut event) = message.system_event() {
            for person in event.people_mut() {
                person.user_id = if person.external_id == message.sender.id {
                    Some(user_id.clone())
                } else {
                    ProviderUser::find_by_external_id(pool, provider_id, &person.external_id).await?.map(|p| p.user_id)
                };
            }
            stored.insert("system".to_string(), serde_json::to_value(&event).unwrap_or(serde_json::Value::Null));
        }
        if let Some(serde_json::Value::Array(reactions)) = stored.get_mut("reactions") {
            for reaction in reactions.iter_mut() {
                let Some(user) = reaction["user"].as_str().filter(|u| !u.is_empty()).map(String::from) else { continue };
                let name: Option<String> = sqlx::query_scalar::<_, Option<String>>("SELECT COALESCE(u.display_name, u.email) FROM provider_user pu JOIN user u ON u.id = pu.user_id WHERE pu.provider_id = ?1 AND pu.external_id = ?2")
                    .bind(provider_id)
                    .bind(&user)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| format!("Failed to name who reacted: {}", e))?
                    .flatten();
                if let Some(name) = name {
                    reaction["name"] = serde_json::Value::String(name);
                }
            }
        }
        let metadata = serde_json::to_string(&stored).unwrap_or_else(|_| "{}".to_string());
        let existing: Option<(bool, Option<String>, Option<String>, Option<String>)> = sqlx::query_as("SELECT is_read, content, summary, presentation FROM message WHERE id = ?1")
            .bind(&message_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to read message state: {}", e))?;
        let is_read = message.is_read || (!read_from_provider && existing.as_ref().map(|(read, _, _, _)| *read).unwrap_or(false));
        let presentation = existing.as_ref().and_then(|(_, _, _, presentation)| presentation.clone());
        let summary = existing.and_then(|(_, content, summary, _)| if content.as_deref() == Some(message.content.as_str()) { summary } else { None });
        sqlx::query(
            r#"INSERT OR REPLACE INTO message (
                id, provider_id, external_id, type,
                conversation_id, sender_id, sender_name, sender_email,
                content, content_type, subject, recipients, cc_recipients,
                is_read, has_attachments, attachments, thread_id, reply_to_id, metadata,
                created_datetime, last_modified_datetime, timestamp,
                cached_at, created_at, updated_at, provider_name,
                headers, origin_ip, origin_lat, origin_lon, origin_city, origin_country, deleted_datetime, summary, presentation
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35)"#
        )
        .bind(&message_id)
        .bind(provider_id)
        .bind(&message.external_id)
        .bind(format!("{:?}", message.r#type).to_lowercase())
        .bind(conversation_id)
        .bind(&user_id)
        .bind(message.sender.display_name.as_deref().or(message.sender.email.as_deref()).unwrap_or("Unknown"))
        .bind(&message.sender.email)
        .bind(&message.content)
        .bind(&message.content_type)
        .bind(&message.subject)
        .bind(recipients(&message.recipients))
        .bind(recipients(&message.cc))
        .bind(is_read)
        .bind(!message.attachments.is_empty())
        .bind(attachments)
        .bind(&message.thread_id)
        .bind(&message.reply_to_id)
        .bind(metadata)
        .bind(message.timestamp.to_rfc3339())
        .bind(&edited_at)
        .bind(message.timestamp.to_rfc3339())
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(provider_name)
        .bind(&message.headers)
        .bind(message.origin.as_ref().and_then(|o| o.ip.clone()))
        .bind(message.origin.as_ref().and_then(|o| o.lat))
        .bind(message.origin.as_ref().and_then(|o| o.lon))
        .bind(message.origin.as_ref().and_then(|o| o.city.clone()))
        .bind(message.origin.as_ref().and_then(|o| o.country.clone()))
        .bind(&deleted_at)
        .bind(&summary)
        .bind(&presentation)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to save message: {}", e))?;
    }
    
    Ok(last_message_id)
}

async fn get_or_create_message_id(
    pool: &Pool<Sqlite>,
    provider_id: &str,
    external_id: &str,
) -> Result<String, String> {
    let existing = sqlx::query("SELECT id FROM message WHERE provider_id = ?1 AND external_id = ?2")
        .bind(provider_id)
        .bind(external_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to check existing message: {}", e))?;
    
    Ok(if let Some(row) = existing {
        row.get("id")
    } else {
        Uuid::new_v4().to_string()
    })
}

async fn update_conversation(
    pool: &Pool<Sqlite>,
    conversation_id: &str,
    last_message_id: Option<&String>,
    summary: Option<&String>,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    
    match (last_message_id, summary) {
        (Some(msg_id), _) => {
            sqlx::query(
                "UPDATE conversation SET last_id = ?1, summary = ?2, updated_at = ?3 WHERE id = ?4"
            )
            .bind(msg_id)
            .bind(summary)
            .bind(&now)
            .bind(conversation_id)
            .execute(pool)
            .await
        }
        (None, Some(_)) => {
            sqlx::query(
                "UPDATE conversation SET summary = ?1, updated_at = ?2 WHERE id = ?3"
            )
            .bind(summary)
            .bind(&now)
            .bind(conversation_id)
            .execute(pool)
            .await
        }
        _ => return Ok(()),
    }
    .map(|_| ())
    .map_err(|e| format!("Failed to update conversation: {}", e))
}

async fn emit_conversation_update(
    pool: &Pool<Sqlite>,
    window: &WebviewWindow,
    conversation_id: &str,
) -> Result<(), String> {
    // Load conversation with providers
    let row = sqlx::query("SELECT * FROM conversation WHERE id = ?1")
        .bind(conversation_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to load conversation: {}", e))?;
    
    if let Some(row) = row {
        let mut conversation = Conversation::from_row(row)?;
        
        // Ensure summary is up to date
        conversation.update_summary(pool).await.ok();
        
        // Load providers
        let provider_rows = sqlx::query(
            "SELECT p.* FROM provider p 
             JOIN conversation_provider cp ON p.id = cp.provider_id 
             WHERE cp.conversation_id = ?1"
        )
        .bind(conversation_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        
        for row in provider_rows {
            if let Ok(mut provider) = Provider::from_row(row) {
                let _ = provider.load_logo(pool).await;
                conversation.providers.push(provider);
            }
        }
        
        let open = Profile::get_active(pool).await?.and_then(|p| p.last_id);
        if open.as_deref() != Some(conversation.id.as_str()) {
            return Ok(());
        }
        conversation.is_current = true;
        conversation.tags = crate::models::ObjectTag::get_conversation(pool, &conversation.id).await.unwrap_or_default();
        conversation.load_display(pool).await.ok();
        window.emit("conversation-updated", &ConversationResponse { data: Some(conversation.clone()), is_loading: false })
            .map_err(|e| format!("Failed to emit conversation update: {}", e))?;
    }
    
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::traits::ResponseMessage;
    use crate::test_helpers::{test_conversation, test_pool, test_profile, test_provider};

    #[tokio::test]
    async fn a_resync_keeps_the_summary_until_the_content_changes() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let provider_id = test_provider(&pool, &profile_id, "mock").await;
        let conversation_id = test_conversation(&pool, "Avtale", "email").await;
        let mut message = ResponseMessage { external_id: "m1".into(), content: "Hei".into(), content_type: "text".into(), timestamp: Utc::now(), ..Default::default() };
        let id = save_messages_to_db(&pool, &conversation_id, &provider_id, "Mock", std::slice::from_ref(&message)).await.unwrap().unwrap();
        sqlx::query("UPDATE message SET summary = 'Hilser' WHERE id = ?1").bind(&id).execute(&pool).await.unwrap();
        save_messages_to_db(&pool, &conversation_id, &provider_id, "Mock", std::slice::from_ref(&message)).await.unwrap();
        let kept: Option<String> = sqlx::query_scalar("SELECT summary FROM message WHERE id = ?1").bind(&id).fetch_one(&pool).await.unwrap();
        assert_eq!(kept.as_deref(), Some("Hilser"));
        message.content = "Hei igjen".into();
        save_messages_to_db(&pool, &conversation_id, &provider_id, "Mock", std::slice::from_ref(&message)).await.unwrap();
        let cleared: Option<String> = sqlx::query_scalar("SELECT summary FROM message WHERE id = ?1").bind(&id).fetch_one(&pool).await.unwrap();
        assert_eq!(cleared, None);
    }

    #[tokio::test]
    async fn system_event_people_resolve_to_chatnip_users_through_their_provider_ids() {
        use crate::providers::traits::{ResponseUser, SystemEvent, SystemEventKind, SystemPerson};
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let provider_id = test_provider(&pool, &profile_id, "mock").await;
        let conversation_id = test_conversation(&pool, "Team", "group_message").await;
        let mia = ResponseUser { id: "u-mia".into(), email: Some("mia@tidewater.example".into()), display_name: Some("Mia Sund".into()), ..Default::default() };
        let hello = ResponseMessage { external_id: "m1".into(), sender: mia, content: "Hei".into(), timestamp: Utc::now(), ..Default::default() };
        save_messages_to_db(&pool, &conversation_id, &provider_id, "Mock", std::slice::from_ref(&hello)).await.unwrap();
        let event = SystemEvent::new(SystemEventKind::MemberAdded, vec![SystemPerson::new("u-anna", "Anna Berg")], vec![SystemPerson::new("u-mia", "Mia Sund"), SystemPerson::new("u-ghost", "Ghost")], None);
        let mut added = ResponseMessage::system("m2", "c1", Utc::now(), event);
        added.sender.email = Some("anna@tidewater.example".into());
        let id = save_messages_to_db(&pool, &conversation_id, &provider_id, "Mock", std::slice::from_ref(&added)).await.unwrap().unwrap();
        let (metadata, sender_id, content): (String, String, String) = sqlx::query_as("SELECT metadata, sender_id, content FROM message WHERE id = ?1").bind(&id).fetch_one(&pool).await.unwrap();
        let mia_user: String = sqlx::query_scalar("SELECT user_id FROM provider_user WHERE external_id = 'u-mia'").fetch_one(&pool).await.unwrap();
        let system: serde_json::Value = serde_json::from_str::<serde_json::Value>(&metadata).unwrap()["system"].clone();
        assert_eq!(content, "Anna Berg added Mia Sund and Ghost");
        assert_eq!(system["actors"][0]["user_id"], serde_json::Value::String(sender_id));
        assert_eq!(system["targets"][0]["user_id"], serde_json::Value::String(mia_user));
        assert!(system["targets"][1]["user_id"].is_null());
        assert_eq!(system["targets"][1]["external_id"], "u-ghost");
    }

    #[test]
    fn granted_scopes_are_compared_with_what_each_registration_declares() {
        assert!(missing_scopes("microsoft", None).is_empty());
        assert!(missing_scopes("imap", Some("anything")).is_empty());
        let granted = "openid profile email https://graph.microsoft.com/User.Read Chat.ReadWrite ChannelMessage.Send Team.ReadBasic.All Channel.ReadBasic.All Mail.ReadWrite Mail.Send Calendars.ReadWrite OnlineMeetings.ReadWrite Presence.Read.All";
        assert_eq!(missing_scopes("microsoft", Some(granted)), vec!["User.ReadBasic.All", "People.Read", "Presence.ReadWrite", "Files.ReadWrite", "Files.Read.All", "MailboxSettings.ReadWrite"]);
        let full = crate::providers::microsoft::BASE_SCOPES.join(" ").to_lowercase();
        assert!(missing_scopes("microsoft", Some(&full)).is_empty());
        let slack = "channels:history,channels:read,channels:write,groups:history,groups:read,groups:write,im:history,im:read,im:write,mpim:history,mpim:read,mpim:write,users:read,users:read.email,chat:write,reactions:read,reactions:write,files:read,files:write,team:read";
        assert_eq!(missing_scopes("slack", Some(slack)), vec!["users.profile:write", "pins:read", "pins:write", "emoji:read", "search:read", "usergroups:read"]);
        let google = "openid email profile https://www.googleapis.com/auth/gmail.modify https://www.googleapis.com/auth/calendar";
        assert_eq!(missing_scopes("google", Some(google)), vec!["https://www.googleapis.com/auth/contacts.readonly", "https://www.googleapis.com/auth/contacts.other.readonly", "https://www.googleapis.com/auth/gmail.settings.basic"]);
    }

    #[tokio::test]
    async fn a_connection_whose_grant_lacks_a_declared_scope_asks_for_consent_again() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let provider_id = test_provider(&pool, &profile_id, "slack").await;
        sqlx::query("UPDATE provider SET scope = 'channels:history,chat:write' WHERE id = ?1").bind(&provider_id).execute(&pool).await.unwrap();
        let provider = Provider::load_from_db(&pool, &provider_id).await.unwrap();
        assert!(provider.missing_scopes.contains(&"emoji:read".to_string()) && provider.missing_scopes.contains(&"pins:write".to_string()));
        assert!(!provider.missing_scopes.contains(&"chat:write".to_string()));
        let serialised = serde_json::to_value(&provider).unwrap();
        assert!(serialised["missing_scopes"].as_array().unwrap().iter().any(|s| s == "emoji:read"));
        sqlx::query("UPDATE provider SET scope = ?1 WHERE id = ?2").bind(crate::providers::slack::USER_SCOPES.join(",")).bind(&provider_id).execute(&pool).await.unwrap();
        assert!(Provider::load_from_db(&pool, &provider_id).await.unwrap().missing_scopes.is_empty());
    }

    struct Fake {
        provider: Provider,
        round: std::sync::Mutex<u32>,
    }

    #[async_trait::async_trait]
    impl ProviderTrait for Fake {
        fn get_provider(&self) -> &Provider {
            &self.provider
        }
        async fn get_auth_url(&self, _: &str, _: Option<&str>) -> Result<String, String> {
            Err("no".into())
        }
        async fn exchange_code(&self, _: &str, _: Option<&str>) -> Result<ProviderAuth, String> {
            Err("no".into())
        }
        async fn identity(&self, _: &ProviderAuth) -> Result<crate::providers::traits::ConnectionIdentity, String> {
            Err("no".into())
        }
        async fn refresh_token(&self, _: &str) -> Result<ProviderAuth, String> {
            Err("no".into())
        }
        async fn validate_tokens(&self, _: &ProviderAuth) -> Result<bool, String> {
            Ok(true)
        }
        async fn sync_conversations(&self, _: &ProviderAuth, _: Option<DateTime<Utc>>) -> Result<Vec<crate::providers::traits::ResponseConversation>, String> {
            Ok(vec![crate::providers::traits::ResponseConversation { external_id: "C1".into(), name: "dev".into(), conversation_type: ConversationType::Channel, members: Vec::new(), last_activity: Utc::now(), metadata: HashMap::new() }])
        }
        async fn sync_messages(&self, _: &ProviderAuth, _: &str, _: Option<DateTime<Utc>>, _: Option<u32>) -> Result<Vec<ResponseMessage>, String> {
            let anna = crate::providers::traits::ResponseUser { id: "U0ANNA".into(), email: Some("anna.berg@client.example".into()), display_name: Some("Anna Berg".into()), ..Default::default() };
            let mut first = ResponseMessage { external_id: "m1".into(), conversation_external_id: "C1".into(), sender: anna.clone(), content: "Ny release 🎉".into(), is_read: false, timestamp: Utc::now() - chrono::Duration::minutes(2), r#type: crate::providers::traits::MessageType::Chat, ..Default::default() };
            first.metadata.insert("reactions".into(), serde_json::json!([{ "emoji": "👍", "user": "U0ME" }, { "emoji": ":party_parrot:", "user": "U0ANNA" }]));
            let second = ResponseMessage { external_id: "m2".into(), conversation_external_id: "C1".into(), sender: anna, content: "Notater".into(), timestamp: Utc::now() - chrono::Duration::minutes(1), r#type: crate::providers::traits::MessageType::Chat, ..Default::default() };
            Ok(vec![second, first])
        }
        async fn send_message(&self, _: &ProviderAuth, _: &str, _: &crate::providers::traits::OutgoingMessage) -> Result<ResponseMessage, String> {
            Err("no".into())
        }
        async fn get_pins(&self, _: &ProviderAuth, _: &str) -> Result<Option<Vec<String>>, String> {
            Ok(Some(vec!["m2".into()]))
        }
        async fn get_people(&self, _: &ProviderAuth) -> Result<Vec<crate::providers::traits::ResponseUser>, String> {
            Ok(vec![crate::providers::traits::ResponseUser { id: "U0OLA".into(), email: Some("ola@northwind.example".into()), display_name: Some("Ola".into()), avatar_url: Some("https://avatars.slack-edge.com/ola.png".into()), ..Default::default() }])
        }
        async fn get_delta(&self, _: &ProviderAuth, cursors: &HashMap<String, String>, _: DateTime<Utc>) -> Result<crate::providers::traits::Delta, String> {
            let mut round = self.round.lock().unwrap();
            *round += 1;
            let changes = if *round == 1 { Vec::new() } else { vec![crate::providers::traits::RemoteChange { message_external_id: "m1".into(), is_read: Some(true), ..Default::default() }, crate::providers::traits::RemoteChange { message_external_id: "m2".into(), labels: Some(vec!["Viktig".into()]), ..Default::default() }] };
            let mut cursors = cursors.clone();
            cursors.insert("fake".into(), round.to_string());
            Ok(crate::providers::traits::Delta { cursors, changes, ..Default::default() })
        }
    }

    #[tokio::test]
    async fn ingest_reads_changes_pins_the_directory_and_stores_canonical_emoji() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let provider_id = test_provider(&pool, &profile_id, "mock").await;
        let fake = Arc::new(Fake { provider: Provider::load_from_db(&pool, &provider_id).await.unwrap(), round: std::sync::Mutex::new(0) });
        Provider::load_from_db(&pool, &provider_id).await.unwrap().ingest(&pool, None, fake.clone()).await.unwrap();
        let (content, metadata, is_read): (String, String, bool) = sqlx::query_as("SELECT content, metadata, is_read FROM message WHERE provider_id = ?1 AND external_id = 'm1'").bind(&provider_id).fetch_one(&pool).await.unwrap();
        assert_eq!(content, "Ny release :tada:");
        let metadata: serde_json::Value = serde_json::from_str(&metadata).unwrap();
        assert_eq!(metadata["original_content"], "Ny release 🎉");
        assert_eq!(metadata["reactions"], serde_json::json!([{ "emoji": ":+1:", "user": "U0ME", "original": "👍" }, { "emoji": ":party_parrot:", "user": "U0ANNA", "name": "Anna Berg" }]));
        assert!(!is_read);
        let pinned: Vec<String> = sqlx::query_scalar("SELECT m.external_id FROM pinned_message p JOIN message m ON m.id = p.message_id").fetch_all(&pool).await.unwrap();
        assert_eq!(pinned, vec!["m2".to_string()]);
        let ola: Option<String> = sqlx::query_scalar("SELECT pu.external_id FROM provider_user pu JOIN user u ON u.id = pu.user_id WHERE pu.provider_id = ?1 AND u.email = 'ola@northwind.example'").bind(&provider_id).fetch_optional(&pool).await.unwrap();
        assert_eq!(ola.as_deref(), Some("U0OLA"));
        let stored = Provider::load_from_db(&pool, &provider_id).await.unwrap();
        assert_eq!(stored.cursors().get("fake").map(String::as_str), Some("1"));
        assert!(stored.cursors().contains_key(CURSOR_DIRECTORY));
        assert!(!stored.due(CURSOR_DIRECTORY, 86400, Utc::now()));
        stored.ingest(&pool, None, fake.clone()).await.unwrap();
        let read: bool = sqlx::query_scalar("SELECT is_read FROM message WHERE provider_id = ?1 AND external_id = 'm1'").bind(&provider_id).fetch_one(&pool).await.unwrap();
        assert!(read);
        let labels: String = sqlx::query_scalar("SELECT json_extract(metadata, '$.labels') FROM message WHERE provider_id = ?1 AND external_id = 'm2'").bind(&provider_id).fetch_one(&pool).await.unwrap();
        assert_eq!(labels, "[\"Viktig\"]");
        assert_eq!(Provider::load_from_db(&pool, &provider_id).await.unwrap().cursors().get("fake").map(String::as_str), Some("2"));
    }
}
