use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite, Row};

pub mod user;
pub mod app_setting;
pub mod tag;
pub mod rule;
pub mod event;
pub mod calendar;
pub mod content;
pub mod person;
pub mod intelligence;
pub mod group;
pub mod inbox;
pub mod sidebar;
pub mod case;
pub mod action;
pub mod presentation;
pub mod chat;
pub mod outbound_hold;
pub mod draft;
pub mod reminder;
pub mod history;
pub mod object_override;
pub use user::{User, ProviderUser, Account};
pub use person::{Person, PersonIdentity, PersonSignature, PostIdentityInput, PutPersonInput, FavouriteOrderItem};
pub use app_setting::AppSetting;
pub use history::{HistoryEntry, PostHistoryInput};
pub use tag::{Tag, ObjectTag, ObjectTagInput, TaggedObject};
pub use object_override::ObjectOverride;
pub use event::{Event, EventDraft};
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Profile {
    #[serde(skip_serializing)]
    pub id: String,
    pub name: String,
    pub user_id: Option<String>,
    #[serde(skip_serializing)]
    pub avatar_id: Option<String>,
    pub avatar: Option<Asset>,
    #[serde(skip_serializing)]
    pub theme_id: Option<String>,
    pub setup_step: i32,
    #[serde(skip_serializing)]
    pub active: bool,
    pub last_id: Option<String>,
    #[serde(skip_serializing)]
    pub created_at: String,
    pub settings: Option<ProfileSetting>,
    pub theme: Option<ProfileTheme>,
}
impl Profile {
    pub async fn get_active_id(pool: &Pool<Sqlite>) -> Result<String, String> {
        sqlx::query_scalar("SELECT id FROM profile WHERE active = true ORDER BY created_at DESC LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to read the active profile: {}", e))?
            .ok_or_else(|| "No active profile found".to_string())
    }

    pub async fn get_own_user_ids(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<String>, String> {
        sqlx::query_scalar(
            "SELECT user_id FROM profile WHERE id = ?1 AND user_id IS NOT NULL
             UNION SELECT pu.user_id FROM provider_user pu JOIN provider p ON p.id = pu.provider_id WHERE p.profile_id = ?1 AND pu.external_id = COALESCE(p.user_external_id, p.external_id)
             ORDER BY 1",
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load your own identities: {}", e))
    }

    pub async fn get_active(pool: &Pool<Sqlite>) -> Result<Option<Self>, String> {
        let row = sqlx::query("SELECT * FROM profile WHERE active = true ORDER BY created_at DESC LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to fetch active profile: {}", e))?;
        if let Some(row) = row {
            let avatar_id: Option<String> = row.get("avatar_id");
            let avatar = if let Some(aid) = &avatar_id {
                let asset_row = sqlx::query("SELECT * FROM assets WHERE id = ?1")
                    .bind(aid)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();
                asset_row.map(|r| Asset {
                    id: r.get("id"),
                    profile_id: r.get("profile_id"),
                    name: r.get("name"),
                    url: r.get("url"),
                    asset_type: r.get("type"),
                    mime_type: r.get("mime_type"),
                    path: r.get("path"),
                    created_at: r.get("created_at"),
                    updated_at: r.get("updated_at"),
                })
            } else {
                None
            };
            
            Ok(Some(Profile {
                id: row.get("id"),
                name: row.get("name"),
                user_id: row.get("user_id"),
                avatar_id,
                avatar,
                theme_id: row.get("theme_id"),
                setup_step: row.get("setup_step"),
                active: row.get("active"),
                last_id: row.get("last_id"),
                created_at: row.get("created_at"),
                settings: None,
                theme: None,
            }))
        } else {
            Ok(None)
        }
    }
    pub async fn post(pool: &Pool<Sqlite>, name: &str) -> Result<Self, String> {
        let profile_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE profile SET active = 0 WHERE active = 1")
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to step out of the current profile: {}", e))?;
        sqlx::query("INSERT INTO profile (id, name, active, created_at) VALUES (?1, ?2, ?3, ?4)")
            .bind(&profile_id)
            .bind(name)
            .bind(true)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to create profile: {}", e))?;
        sqlx::query(
            r#"INSERT INTO profile_setting (
                id, profile_id,
                email_content_threshold, navigation_width, navigation_collapsed, details_width, details_panel_open,
                message_timestamp_interval, time_format, window_controls_style, max_input_lines,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&profile_id)
        .bind(500)
        .bind(256)
        .bind(false)
        .bind(320)
        .bind(false)
        .bind(5)
        .bind("24h")
        .bind("macos")
        .bind(10)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create settings: {}", e))?;
        Self::get_active(pool).await?.ok_or_else(|| "Failed to load created profile".to_string())
    }

    pub async fn get_or_post(pool: &Pool<Sqlite>, name: &str) -> Result<Self, String> {
        match Self::get_active(pool).await? {
            Some(profile) => Ok(profile),
            None => Self::post(pool, name).await,
        }
    }

    pub async fn load_providers(&self, pool: &Pool<Sqlite>) -> Result<Vec<crate::providers::models::Provider>, String> {
        let rows = sqlx::query(
            "SELECT * FROM provider 
             WHERE profile_id = ?1 AND access_token IS NOT NULL"
        )
        .bind(&self.id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load providers: {}", e))?;
        let mut providers = Vec::new();
        for row in rows {
            providers.push(crate::providers::models::Provider::from_row(row)?);
        }
        Ok(providers)
    }
    pub async fn validate_providers(&self, pool: &Pool<Sqlite>) -> Result<(), String> {
        let mut providers = self.load_providers(pool).await?;
        println!("🔍 Validating {} providers for profile {}", providers.len(), self.name);
        for provider in providers.iter_mut() {
            println!("🔐 Validating provider {} ({})", provider.id, provider.provider_type);
            if let Err(e) = provider.validate(pool).await {
                eprintln!("⚠️ Failed to validate provider {}: {}", provider.id, e);
            }
        }
        Ok(())
    }
    
    pub async fn update_setup_step(&self, pool: &Pool<Sqlite>) -> Result<i32, String> {
        let has_provider = sqlx::query("SELECT COUNT(*) as count FROM provider WHERE profile_id = ?1 AND access_token IS NOT NULL")
            .bind(&self.id)
            .fetch_one(pool)
            .await
            .map(|row| row.get::<i64, _>("count") > 0)
            .unwrap_or(false);
        
        let new_step = if has_provider { 3 } else { 2 };
        
        if new_step != self.setup_step {
            sqlx::query("UPDATE profile SET setup_step = ?1 WHERE id = ?2")
                .bind(new_step)
                .bind(&self.id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to update setup_step: {}", e))?;
        }
        
        Ok(new_step)
    }
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct WindowPosition {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub monitor: String,
    pub scale: f64,
    pub maximized: bool,
    pub fullscreen: bool,
}
impl WindowPosition {
    pub fn from_row(row: &sqlx::sqlite::SqliteRow) -> Option<Self> {
        let ratio = |column: &str| row.try_get_unchecked::<Option<f64>, _>(column).ok().flatten();
        Some(Self {
            x: ratio("window_x")?,
            y: ratio("window_y")?,
            width: ratio("window_width")?,
            height: ratio("window_height")?,
            monitor: row.try_get::<Option<String>, _>("window_monitor").ok().flatten()?,
            scale: ratio("window_scale")?,
            maximized: row.try_get::<Option<bool>, _>("window_maximized").ok().flatten().unwrap_or(false),
            fullscreen: row.try_get::<Option<bool>, _>("window_fullscreen").ok().flatten().unwrap_or(false),
        })
    }

    pub async fn get(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Option<Self>, String> {
        let row = sqlx::query("SELECT * FROM profile_setting WHERE profile_id = ?1")
            .bind(profile_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to get window position: {}", e))?;
        Ok(row.as_ref().and_then(Self::from_row))
    }

    pub async fn put(&self, pool: &Pool<Sqlite>, profile_id: &str) -> Result<(), String> {
        sqlx::query(
            r#"UPDATE profile_setting SET
                window_x = ?1,
                window_y = ?2,
                window_width = ?3,
                window_height = ?4,
                window_monitor = ?5,
                window_scale = ?6,
                window_maximized = ?7,
                window_fullscreen = ?8,
                updated_at = ?9
            WHERE profile_id = ?10"#
        )
        .bind(self.x)
        .bind(self.y)
        .bind(self.width)
        .bind(self.height)
        .bind(&self.monitor)
        .bind(self.scale)
        .bind(self.maximized)
        .bind(self.fullscreen)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(profile_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update window position: {}", e))?;
        Ok(())
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProfileSetting {
    #[serde(skip_serializing)]
    pub id: String,
    #[serde(skip_serializing)]
    pub profile_id: String,
    pub email_content_threshold: i32,
    pub navigation_width: i32,
    pub navigation_collapsed: bool,
    pub details_width: i32,
    pub details_panel_open: bool,
    pub window_position: Option<WindowPosition>,
    pub message_timestamp_interval: i32,
    pub time_format: String,
    pub window_controls_style: String,
    pub max_input_lines: i32,
    #[serde(skip_serializing)]
    pub created_at: String,
    #[serde(skip_serializing)]
    pub updated_at: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProfileTheme {
    #[serde(skip_serializing)]
    pub id: String,
    #[serde(skip_serializing)]
    pub profile_id: String,
    pub theme_name: String,
    pub is_active: bool,
    pub primary_50: String,
    pub primary_100: String,
    pub primary_200: String,
    pub primary_300: String,
    pub primary_400: String,
    pub primary_500: String,
    pub primary_600: String,
    pub primary_700: String,
    pub primary_800: String,
    pub primary_900: String,
    pub primary_950: String,
    pub secondary_50: String,
    pub secondary_100: String,
    pub secondary_200: String,
    pub secondary_300: String,
    pub secondary_400: String,
    pub secondary_500: String,
    pub secondary_600: String,
    pub secondary_700: String,
    pub secondary_800: String,
    pub secondary_900: String,
    pub secondary_950: String,
    pub accent_50: String,
    pub accent_100: String,
    pub accent_200: String,
    pub accent_300: String,
    pub accent_400: String,
    pub accent_500: String,
    pub accent_600: String,
    pub accent_700: String,
    pub accent_800: String,
    pub accent_900: String,
    pub accent_950: String,
    pub background_50: String,
    pub background_100: String,
    pub background_200: String,
    pub background_300: String,
    pub background_400: String,
    pub background_500: String,
    pub background_600: String,
    pub background_700: String,
    pub background_800: String,
    pub background_900: String,
    pub background_950: String,
    pub content_50: String,
    pub content_100: String,
    pub content_200: String,
    pub content_300: String,
    pub content_400: String,
    pub content_500: String,
    pub content_600: String,
    pub content_700: String,
    pub content_800: String,
    pub content_900: String,
    pub content_950: String,
    pub control_50: String,
    pub control_100: String,
    pub control_200: String,
    pub control_300: String,
    pub control_400: String,
    pub control_500: String,
    pub control_600: String,
    pub control_700: String,
    pub control_800: String,
    pub control_900: String,
    pub control_950: String,
    pub error_50: String,
    pub error_100: String,
    pub error_200: String,
    pub error_300: String,
    pub error_400: String,
    pub error_500: String,
    pub error_600: String,
    pub error_700: String,
    pub error_800: String,
    pub error_900: String,
    pub error_950: String,
    pub warn_50: String,
    pub warn_100: String,
    pub warn_200: String,
    pub warn_300: String,
    pub warn_400: String,
    pub warn_500: String,
    pub warn_600: String,
    pub warn_700: String,
    pub warn_800: String,
    pub warn_900: String,
    pub warn_950: String,
    pub success_50: String,
    pub success_100: String,
    pub success_200: String,
    pub success_300: String,
    pub success_400: String,
    pub success_500: String,
    pub success_600: String,
    pub success_700: String,
    pub success_800: String,
    pub success_900: String,
    pub success_950: String,
    #[serde(skip_serializing)]
    pub created_at: String,
    #[serde(skip_serializing)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub provider_name: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub overrides: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub nicknames: std::collections::BTreeMap<String, String>,
    #[serde(rename = "type")]
    pub conversation_type: String,
    pub group_type: String,
    pub last_id: Option<String>,
    pub last_activity: String,
    pub providers: Vec<crate::providers::models::Provider>,
    pub theme: Option<ProfileTheme>,
    pub namespace: Option<String>,
    pub metadata: Option<String>,
    pub unread_count: i32,
    pub members: Vec<User>,
    pub is_current: bool,
    pub is_pinned: bool,
    pub is_muted: bool,
    pub mode: chat::ChatMode,
    pub view: chat::ChatView,
    pub view_settings: chat::ViewSettings,
    pub is_archived: bool,
    pub snoozed_until: Option<String>,
    pub deleted_at: Option<String>,
    pub person_id: Option<String>,
    pub favourite_position: Option<i64>,
    pub tags: Vec<tag::ObjectTag>,
    pub notification_level: String,
    pub summary: Option<String>,
    pub presentation: Option<serde_json::Value>,
    pub threads: Vec<Thread>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    pub conversation_id: String,
    pub root_message_id: String,
    pub name: Option<String>,
    pub last_activity: String,
    pub message_count: i32,
    pub is_resolved: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationLink {
    pub id: String,
    pub source_conversation_id: String,
    pub target_conversation_id: String,
    pub link_type: String,
    pub created_at: String,
}

impl ConversationLink {
    pub async fn post(pool: &Pool<Sqlite>, source_conversation_id: &str, target_conversation_id: &str, link_type: &str) -> Result<(), String> {
        sqlx::query("INSERT OR IGNORE INTO conversation_link (id, source_conversation_id, target_conversation_id, link_type, created_at) VALUES (?1, ?2, ?3, ?4, ?5)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(source_conversation_id)
            .bind(target_conversation_id)
            .bind(link_type)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to link conversations: {}", e))?;
        Ok(())
    }
}

pub const MERGE_LINK: &str = "merge";
pub const NEST_LINK: &str = "nest";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MergeGroup {
    pub id: String,
    pub lead_conversation_id: String,
    pub name: Option<String>,
    pub parent_id: Option<String>,
    pub conversation_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeGroupInput {
    pub lead_conversation_id: String,
    #[serde(default)]
    pub conversation_ids: Vec<String>,
    #[serde(default)]
    pub group_lead_ids: Vec<String>,
}

type LinkTree = (Vec<(String, Vec<String>)>, Vec<(String, String)>);

impl MergeGroup {
    pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
        let fresh = !crate::database::table_exists(pool, "merge_group").await?;
        for statement in [
            "CREATE TABLE IF NOT EXISTS merge_group (
                id TEXT PRIMARY KEY,
                profile_id TEXT NOT NULL,
                lead_conversation_id TEXT NOT NULL,
                name TEXT,
                parent_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (profile_id) REFERENCES profile (id),
                FOREIGN KEY (parent_id) REFERENCES merge_group (id) ON DELETE SET NULL
            )",
            "CREATE TABLE IF NOT EXISTS merge_group_member (
                merge_group_id TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (merge_group_id, conversation_id),
                FOREIGN KEY (merge_group_id) REFERENCES merge_group (id) ON DELETE CASCADE,
                FOREIGN KEY (conversation_id) REFERENCES conversation (id) ON DELETE CASCADE
            )",
            "CREATE TABLE IF NOT EXISTS merge_group_conversation (
                merge_group_id TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                PRIMARY KEY (merge_group_id, conversation_id),
                FOREIGN KEY (merge_group_id) REFERENCES merge_group (id) ON DELETE CASCADE,
                FOREIGN KEY (conversation_id) REFERENCES conversation (id) ON DELETE CASCADE
            )",
            "CREATE INDEX IF NOT EXISTS idx_merge_group_lead ON merge_group (profile_id, lead_conversation_id)",
            "CREATE INDEX IF NOT EXISTS idx_merge_group_parent ON merge_group (parent_id)",
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_merge_group_member_conversation ON merge_group_member (conversation_id)",
            "CREATE INDEX IF NOT EXISTS idx_merge_group_conversation_conversation ON merge_group_conversation (conversation_id)",
            "CREATE INDEX IF NOT EXISTS idx_message_conversation_timestamp ON message (conversation_id, timestamp)",
        ] {
            sqlx::query(statement).execute(pool).await?;
        }
        if fresh {
            Self::post_links(pool).await.map_err(sqlx::Error::Protocol)?;
        }
        Ok(())
    }

    pub async fn get_all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Self>, String> {
        let rows = sqlx::query(
            "SELECT g.id, g.lead_conversation_id, g.name, g.parent_id, g.created_at, g.updated_at,
                    (SELECT json_group_array(conversation_id) FROM (SELECT m.conversation_id FROM merge_group_member m WHERE m.merge_group_id = g.id ORDER BY m.position, m.rowid)) AS conversation_ids
             FROM merge_group g WHERE g.profile_id = ?1 ORDER BY g.created_at, g.rowid",
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load groups: {}", e))?;
        Ok(rows
            .into_iter()
            .map(|row| Self {
                id: row.get("id"),
                lead_conversation_id: row.get("lead_conversation_id"),
                name: row.get("name"),
                parent_id: row.get("parent_id"),
                conversation_ids: serde_json::from_str(&row.get::<String, _>("conversation_ids")).unwrap_or_default(),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, input: &MergeGroupInput) -> Result<(), String> {
        let mut tx = pool.begin().await.map_err(|e| format!("Failed to start grouping: {}", e))?;
        let now = chrono::Utc::now().to_rfc3339();
        let group = match Self::get_group(&mut tx, profile_id, &input.lead_conversation_id).await? {
            Some((id, _)) => id,
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                sqlx::query("INSERT INTO merge_group (id, profile_id, lead_conversation_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)")
                    .bind(&id)
                    .bind(profile_id)
                    .bind(&input.lead_conversation_id)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to create the group: {}", e))?;
                Self::put_member(&mut tx, &id, &input.lead_conversation_id).await?;
                id
            }
        };
        for conversation_id in &input.conversation_ids {
            Self::put_member(&mut tx, &group, conversation_id).await?;
        }
        for lead in &input.group_lead_ids {
            let Some((child, parent)) = Self::get_group(&mut tx, profile_id, lead).await? else { continue };
            if child == group || parent.is_some() || Self::get_ancestors(&mut tx, &group).await?.contains(&child) {
                continue;
            }
            Self::put_parent(&mut tx, &child, Some(&group)).await?;
        }
        Self::put_membership(&mut tx, profile_id).await?;
        tx.commit().await.map_err(|e| format!("Failed to save the group: {}", e))
    }

    pub async fn delete_items(pool: &Pool<Sqlite>, profile_id: &str, input: &MergeGroupInput) -> Result<(), String> {
        let mut tx = pool.begin().await.map_err(|e| format!("Failed to start ungrouping: {}", e))?;
        let Some((group, _)) = Self::get_group(&mut tx, profile_id, &input.lead_conversation_id).await? else { return Ok(()) };
        for conversation_id in &input.conversation_ids {
            match Self::get_member_group(&mut tx, profile_id, conversation_id).await? {
                Some((id, _)) if id == group => {
                    sqlx::query("DELETE FROM merge_group_member WHERE merge_group_id = ?1 AND conversation_id = ?2")
                        .bind(&group)
                        .bind(conversation_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| format!("Failed to ungroup the chat: {}", e))?;
                }
                Some((id, Some(parent))) if parent == group => Self::put_parent(&mut tx, &id, None).await?,
                _ => {}
            }
        }
        for lead in &input.group_lead_ids {
            if let Some((child, Some(parent))) = Self::get_group(&mut tx, profile_id, lead).await? {
                if parent == group {
                    Self::put_parent(&mut tx, &child, None).await?;
                }
            }
        }
        Self::put_membership(&mut tx, profile_id).await?;
        tx.commit().await.map_err(|e| format!("Failed to save the group: {}", e))
    }

    async fn get_group(conn: &mut sqlx::SqliteConnection, profile_id: &str, conversation_id: &str) -> Result<Option<(String, Option<String>)>, String> {
        sqlx::query_as(
            "SELECT g.id, g.parent_id FROM merge_group g
             WHERE g.profile_id = ?1 AND (g.lead_conversation_id = ?2 OR EXISTS (SELECT 1 FROM merge_group_member m WHERE m.merge_group_id = g.id AND m.conversation_id = ?2))
             ORDER BY g.lead_conversation_id = ?2 DESC LIMIT 1",
        )
        .bind(profile_id)
        .bind(conversation_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| format!("Failed to load the group: {}", e))
    }

    async fn get_member_group(conn: &mut sqlx::SqliteConnection, profile_id: &str, conversation_id: &str) -> Result<Option<(String, Option<String>)>, String> {
        sqlx::query_as("SELECT g.id, g.parent_id FROM merge_group g JOIN merge_group_member m ON m.merge_group_id = g.id WHERE g.profile_id = ?1 AND m.conversation_id = ?2")
            .bind(profile_id)
            .bind(conversation_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| format!("Failed to load the group: {}", e))
    }

    async fn get_ancestors(conn: &mut sqlx::SqliteConnection, group_id: &str) -> Result<Vec<String>, String> {
        sqlx::query_scalar("WITH RECURSIVE up(id) AS (SELECT ?1 UNION SELECT g.parent_id FROM merge_group g JOIN up ON g.id = up.id WHERE g.parent_id IS NOT NULL) SELECT id FROM up")
            .bind(group_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| format!("Failed to load the group tree: {}", e))
    }

    async fn put_member(conn: &mut sqlx::SqliteConnection, group_id: &str, conversation_id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM merge_group_member WHERE conversation_id = ?2 AND merge_group_id <> ?1")
            .bind(group_id)
            .bind(conversation_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("Failed to move the chat: {}", e))?;
        sqlx::query("INSERT OR IGNORE INTO merge_group_member (merge_group_id, conversation_id, position) VALUES (?1, ?2, (SELECT COALESCE(MAX(position), -1) + 1 FROM merge_group_member WHERE merge_group_id = ?1))")
            .bind(group_id)
            .bind(conversation_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("Failed to group the chat: {}", e))?;
        Ok(())
    }

    async fn put_parent(conn: &mut sqlx::SqliteConnection, group_id: &str, parent_id: Option<&str>) -> Result<(), String> {
        sqlx::query("UPDATE merge_group SET parent_id = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(parent_id)
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(group_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("Failed to nest the group: {}", e))?;
        Ok(())
    }

    async fn put_membership(conn: &mut sqlx::SqliteConnection, profile_id: &str) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        let fail = |e: sqlx::Error| format!("Failed to store the group: {}", e);
        sqlx::query(
            "UPDATE merge_group SET lead_conversation_id = (SELECT m.conversation_id FROM merge_group_member m WHERE m.merge_group_id = merge_group.id ORDER BY m.position, m.rowid LIMIT 1), updated_at = ?2
             WHERE profile_id = ?1
               AND NOT EXISTS (SELECT 1 FROM merge_group_member m WHERE m.merge_group_id = merge_group.id AND m.conversation_id = merge_group.lead_conversation_id)
               AND EXISTS (SELECT 1 FROM merge_group_member m WHERE m.merge_group_id = merge_group.id)",
        )
        .bind(profile_id)
        .bind(&now)
        .execute(&mut *conn)
        .await
        .map_err(fail)?;
        loop {
            let lonely = sqlx::query(
                "DELETE FROM merge_group_member WHERE merge_group_id IN (
                    SELECT g.id FROM merge_group g WHERE g.profile_id = ?1 AND g.parent_id IS NULL
                      AND NOT EXISTS (SELECT 1 FROM merge_group c WHERE c.parent_id = g.id)
                      AND (SELECT COUNT(*) FROM merge_group_member m WHERE m.merge_group_id = g.id) = 1)",
            )
            .bind(profile_id)
            .execute(&mut *conn)
            .await
            .map_err(fail)?
            .rows_affected();
            let empty: Vec<(String, Option<String>)> = sqlx::query_as("SELECT g.id, g.parent_id FROM merge_group g WHERE g.profile_id = ?1 AND NOT EXISTS (SELECT 1 FROM merge_group_member m WHERE m.merge_group_id = g.id)")
                .bind(profile_id)
                .fetch_all(&mut *conn)
                .await
                .map_err(fail)?;
            if lonely == 0 && empty.is_empty() {
                break;
            }
            for (id, parent) in empty {
                sqlx::query("UPDATE merge_group SET parent_id = ?1, updated_at = ?2 WHERE parent_id = ?3")
                    .bind(parent.as_deref())
                    .bind(&now)
                    .bind(&id)
                    .execute(&mut *conn)
                    .await
                    .map_err(fail)?;
                sqlx::query("DELETE FROM object_tag WHERE object_type = ?1 AND object_id = ?2")
                    .bind(crate::models::tag::OBJECT_MERGE_GROUP)
                    .bind(&id)
                    .execute(&mut *conn)
                    .await
                    .map_err(fail)?;
                sqlx::query("DELETE FROM merge_group WHERE id = ?1").bind(&id).execute(&mut *conn).await.map_err(fail)?;
            }
        }
        sqlx::query("DELETE FROM merge_group_conversation WHERE merge_group_id NOT IN (SELECT id FROM merge_group) OR merge_group_id IN (SELECT id FROM merge_group WHERE profile_id = ?1)")
            .bind(profile_id)
            .execute(&mut *conn)
            .await
            .map_err(fail)?;
        sqlx::query(
            "WITH RECURSIVE tree(merge_group_id, descendant_id) AS (
                SELECT id, id FROM merge_group WHERE profile_id = ?1
                UNION SELECT t.merge_group_id, g.id FROM tree t JOIN merge_group g ON g.parent_id = t.descendant_id
             )
             INSERT OR IGNORE INTO merge_group_conversation (merge_group_id, conversation_id)
             SELECT t.merge_group_id, m.conversation_id FROM tree t JOIN merge_group_member m ON m.merge_group_id = t.descendant_id",
        )
        .bind(profile_id)
        .execute(&mut *conn)
        .await
        .map_err(fail)?;
        Ok(())
    }

    async fn post_links(pool: &Pool<Sqlite>) -> Result<(), String> {
        let profiles: Vec<String> = sqlx::query_scalar("SELECT id FROM profile").fetch_all(pool).await.map_err(|e| format!("Failed to read profiles: {}", e))?;
        for profile_id in profiles {
            let links: Vec<(String, String, String)> = sqlx::query_as(
                "SELECT l.source_conversation_id, l.target_conversation_id, l.link_type FROM conversation_link l
                 WHERE l.link_type IN (?2, ?3)
                   AND EXISTS (SELECT 1 FROM conversation_provider cp JOIN provider p ON p.id = cp.provider_id WHERE cp.conversation_id = l.target_conversation_id AND p.profile_id = ?1)
                 ORDER BY l.link_type = ?3, l.rowid",
            )
            .bind(&profile_id)
            .bind(MERGE_LINK)
            .bind(NEST_LINK)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read merge links: {}", e))?;
            if links.is_empty() {
                continue;
            }
            let (groups, parents) = Self::get_link_tree(&links);
            let mut tx = pool.begin().await.map_err(|e| format!("Failed to start the group migration: {}", e))?;
            let now = chrono::Utc::now().to_rfc3339();
            let mut ids = std::collections::HashMap::new();
            for (lead, members) in &groups {
                let id = uuid::Uuid::new_v4().to_string();
                sqlx::query("INSERT INTO merge_group (id, profile_id, lead_conversation_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)")
                    .bind(&id)
                    .bind(&profile_id)
                    .bind(lead)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to migrate a group: {}", e))?;
                for member in members {
                    Self::put_member(&mut tx, &id, member).await?;
                }
                ids.insert(lead.clone(), id);
            }
            for (child, above) in &parents {
                if let (Some(child), Some(above)) = (ids.get(child), ids.get(above)) {
                    Self::put_parent(&mut tx, child, Some(above)).await?;
                }
            }
            Self::put_membership(&mut tx, &profile_id).await?;
            tx.commit().await.map_err(|e| format!("Failed to migrate groups: {}", e))?;
        }
        Ok(())
    }

    pub fn get_link_tree(links: &[(String, String, String)]) -> LinkTree {
        use std::collections::{HashMap, HashSet};
        fn root(parent: &mut HashMap<String, String>, id: &str) -> String {
            let mut at = id.to_string();
            while let Some(next) = parent.get(&at).filter(|next| **next != at).cloned() {
                at = next;
            }
            parent.insert(id.to_string(), at.clone());
            at
        }
        let mut order: Vec<String> = Vec::new();
        let mut parent: HashMap<String, String> = HashMap::new();
        let mut sources: HashSet<String> = HashSet::new();
        for (source, target, kind) in links {
            if kind != MERGE_LINK {
                continue;
            }
            sources.insert(source.clone());
            for id in [source, target] {
                if !parent.contains_key(id) {
                    parent.insert(id.clone(), id.clone());
                    order.push(id.clone());
                }
            }
            let a = root(&mut parent, target);
            let b = root(&mut parent, source);
            if a != b {
                parent.insert(b, a);
            }
        }
        let mut leads: HashMap<String, String> = HashMap::new();
        for id in &order {
            let top = root(&mut parent, id);
            if !sources.contains(id) && !leads.contains_key(&top) {
                leads.insert(top, id.clone());
            }
        }
        let mut cluster: HashMap<String, String> = HashMap::new();
        for id in &order {
            let top = root(&mut parent, id);
            cluster.insert(id.clone(), leads.get(&top).cloned().unwrap_or(top));
        }
        let nests: Vec<&(String, String, String)> = links.iter().filter(|(_, _, kind)| kind == NEST_LINK).collect();
        for (source, target, _) in &nests {
            for id in [source, target] {
                if !cluster.contains_key(id) {
                    cluster.insert(id.clone(), id.clone());
                    order.push(id.clone());
                }
            }
        }
        let mut above: HashMap<String, String> = HashMap::new();
        let mut parents: Vec<(String, String)> = Vec::new();
        for (source, target, _) in &nests {
            let child = cluster[source].clone();
            let up = cluster[target].clone();
            if child == up || above.contains_key(&child) {
                continue;
            }
            let mut at = Some(up.clone());
            while let Some(node) = at.clone() {
                if node == child {
                    break;
                }
                at = above.get(&node).cloned();
            }
            if at.as_deref() != Some(child.as_str()) {
                above.insert(child.clone(), up.clone());
                parents.push((child, up));
            }
        }
        let mut groups: Vec<(String, Vec<String>)> = Vec::new();
        for id in &order {
            let lead = &cluster[id];
            let at = match groups.iter().position(|(l, _)| l == lead) {
                Some(at) => at,
                None => {
                    groups.push((lead.clone(), vec![lead.clone()]));
                    groups.len() - 1
                }
            };
            if !groups[at].1.contains(id) {
                groups[at].1.push(id.clone());
            }
        }
        (groups, parents)
    }
}

pub const OWNED_BY_PROFILE: &str = "(EXISTS (SELECT 1 FROM conversation_provider cp JOIN provider p ON p.id = cp.provider_id WHERE cp.conversation_id = c.id AND p.profile_id = ?1) OR EXISTS (SELECT 1 FROM chat ch WHERE ch.conversation_id = c.id AND ch.profile_id = ?1))";

pub fn provider_image(metadata: Option<&str>) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(metadata?).ok()?;
    ["image", "avatar_url", "picture", "icon_url"].iter().find_map(|key| value.get(*key).and_then(|v| v.as_str()).filter(|v| !v.trim().is_empty()).map(String::from))
}

impl Conversation {
    pub async fn get_name(pool: &Pool<Sqlite>, id: &str) -> Result<Option<String>, String> {
        sqlx::query_scalar("SELECT name FROM conversation WHERE id = ?1").bind(id).fetch_optional(pool).await.map_err(|e| format!("Failed to read the conversation name: {}", e))
    }

    pub async fn get_rows_for_profile(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<sqlx::sqlite::SqliteRow>, String> {
        sqlx::query(&format!("SELECT c.* FROM conversation c WHERE {} ORDER BY c.last_activity DESC", OWNED_BY_PROFILE))
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list conversations: {}", e))
    }

    pub async fn get_rows_for_active_profile(pool: &Pool<Sqlite>) -> Result<Vec<sqlx::sqlite::SqliteRow>, String> {
        match Profile::get_active(pool).await? {
            Some(profile) => Self::get_rows_for_profile(pool, &profile.id).await,
            None => Ok(Vec::new()),
        }
    }

    pub async fn is_owned_by(pool: &Pool<Sqlite>, conversation_id: &str, profile_id: &str) -> Result<bool, String> {
        sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM conversation c WHERE c.id = ?2 AND {}", OWNED_BY_PROFILE))
            .bind(profile_id)
            .bind(conversation_id)
            .fetch_one(pool)
            .await
            .map(|n| n > 0)
            .map_err(|e| format!("Failed to check the conversation owner: {}", e))
    }

    pub fn from_row(row: sqlx::sqlite::SqliteRow) -> Result<Self, String> {
        Ok(Self {
            id: row.get("id"),
            name: row.get("name"),
            provider_name: row.get("name"),
            image: provider_image(row.try_get::<Option<String>, _>("metadata").ok().flatten().as_deref()),
            overrides: std::collections::BTreeMap::new(),
            nicknames: std::collections::BTreeMap::new(),
            conversation_type: row.get("type"),
            group_type: row.try_get("group_type").unwrap_or("auto".to_string()),
            last_id: row.get("last_id"),
            last_activity: row.get("last_activity"),
            providers: vec![],
            theme: None,
            namespace: row.get("namespace"),
            metadata: row.get("metadata"),
            unread_count: row.get("unread_count"),
            members: vec![],
            is_current: row.get("is_current"),
            is_pinned: row.try_get("is_pinned").unwrap_or(false),
            is_muted: row.try_get("is_muted").unwrap_or(false),
            mode: chat::ChatMode::parse(&row.try_get::<String, _>("mode").unwrap_or_default()),
            view: chat::ChatView::parse(&row.try_get::<String, _>("view").unwrap_or_default()),
            view_settings: chat::ViewSettings::parse(row.try_get("view_settings").ok().flatten()),
            is_archived: row.try_get("is_archived").unwrap_or(false),
            snoozed_until: row.try_get("snoozed_until").ok().flatten(),
            deleted_at: row.try_get("deleted_at").ok().flatten(),
            person_id: row.try_get("person_id").ok().flatten(),
            favourite_position: row.try_get("favourite_position").ok().flatten(),
            tags: Vec::new(),
            notification_level: row.try_get("notification_level").unwrap_or("all".to_string()),
            summary: row.get("summary"),
            presentation: row.try_get::<Option<String>, _>("presentation").ok().flatten().and_then(|p| serde_json::from_str(&p).ok()),
            threads: vec![],
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
    
    pub async fn update_summary(&mut self, pool: &Pool<Sqlite>) -> Result<bool, String> {
        // Only generate summary if we have a last_id but no summary
        if self.last_id.is_some() && (self.summary.is_none() || self.summary.as_ref().map(|s| s.is_empty()).unwrap_or(false)) {
            use crate::providers::models::generate_summary_from_db_messages;
            
            if let Some(summary) = generate_summary_from_db_messages(pool, &self.id, self.last_id.as_ref()).await? {
                self.summary = Some(summary.clone());
                
                // Update the database
                sqlx::query(
                    "UPDATE conversation SET summary = ?1, updated_at = ?2 WHERE id = ?3"
                )
                .bind(&summary)
                .bind(&chrono::Utc::now().to_rfc3339())
                .bind(&self.id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to update conversation summary: {}", e))?;
                
                return Ok(true);
            }
        }
        Ok(false)
    }
    
    pub async fn load_with_providers(pool: &Pool<Sqlite>, conversation_id: &str) -> Result<Option<Self>, String> {
        let row = sqlx::query("SELECT * FROM conversation WHERE id = ?1")
            .bind(conversation_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load conversation: {}", e))?;
        
        if let Some(row) = row {
            let mut conversation = Self::from_row(row)?;
            
            let provider_rows = sqlx::query(
                r#"SELECT p.* FROM provider p
                   JOIN conversation_provider cp ON p.id = cp.provider_id
                   WHERE cp.conversation_id = ?1"#
            )
            .bind(conversation_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load providers: {}", e))?;
            
            for row in provider_rows {
                let mut provider = crate::providers::models::Provider::from_row(row)?;
                provider.load_logo(pool).await.ok();
                conversation.providers.push(provider);
            }
            
            let theme_row = sqlx::query(
                r#"SELECT pt.* FROM profile_theme pt
                   JOIN conversation_provider cp ON pt.id = cp.theme_id
                   WHERE cp.conversation_id = ?1 AND cp.is_primary = 1
                   LIMIT 1"#
            )
            .bind(conversation_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load theme: {}", e))?;
            
            if let Some(theme_row) = theme_row {
                conversation.theme = Some(ProfileTheme {
                    id: theme_row.get("id"),
                    profile_id: theme_row.get("profile_id"),
                    theme_name: theme_row.get("theme_name"),
                    is_active: theme_row.get("is_active"),
                    primary_50: theme_row.get("primary_50"),
                    primary_100: theme_row.get("primary_100"),
                    primary_200: theme_row.get("primary_200"),
                    primary_300: theme_row.get("primary_300"),
                    primary_400: theme_row.get("primary_400"),
                    primary_500: theme_row.get("primary_500"),
                    primary_600: theme_row.get("primary_600"),
                    primary_700: theme_row.get("primary_700"),
                    primary_800: theme_row.get("primary_800"),
                    primary_900: theme_row.get("primary_900"),
                    primary_950: theme_row.get("primary_950"),
                    secondary_50: theme_row.get("secondary_50"),
                    secondary_100: theme_row.get("secondary_100"),
                    secondary_200: theme_row.get("secondary_200"),
                    secondary_300: theme_row.get("secondary_300"),
                    secondary_400: theme_row.get("secondary_400"),
                    secondary_500: theme_row.get("secondary_500"),
                    secondary_600: theme_row.get("secondary_600"),
                    secondary_700: theme_row.get("secondary_700"),
                    secondary_800: theme_row.get("secondary_800"),
                    secondary_900: theme_row.get("secondary_900"),
                    secondary_950: theme_row.get("secondary_950"),
                    accent_50: theme_row.get("accent_50"),
                    accent_100: theme_row.get("accent_100"),
                    accent_200: theme_row.get("accent_200"),
                    accent_300: theme_row.get("accent_300"),
                    accent_400: theme_row.get("accent_400"),
                    accent_500: theme_row.get("accent_500"),
                    accent_600: theme_row.get("accent_600"),
                    accent_700: theme_row.get("accent_700"),
                    accent_800: theme_row.get("accent_800"),
                    accent_900: theme_row.get("accent_900"),
                    accent_950: theme_row.get("accent_950"),
                    background_50: theme_row.get("background_50"),
                    background_100: theme_row.get("background_100"),
                    background_200: theme_row.get("background_200"),
                    background_300: theme_row.get("background_300"),
                    background_400: theme_row.get("background_400"),
                    background_500: theme_row.get("background_500"),
                    background_600: theme_row.get("background_600"),
                    background_700: theme_row.get("background_700"),
                    background_800: theme_row.get("background_800"),
                    background_900: theme_row.get("background_900"),
                    background_950: theme_row.get("background_950"),
                    content_50: theme_row.get("content_50"),
                    content_100: theme_row.get("content_100"),
                    content_200: theme_row.get("content_200"),
                    content_300: theme_row.get("content_300"),
                    content_400: theme_row.get("content_400"),
                    content_500: theme_row.get("content_500"),
                    content_600: theme_row.get("content_600"),
                    content_700: theme_row.get("content_700"),
                    content_800: theme_row.get("content_800"),
                    content_900: theme_row.get("content_900"),
                    content_950: theme_row.get("content_950"),
                    control_50: theme_row.get("control_50"),
                    control_100: theme_row.get("control_100"),
                    control_200: theme_row.get("control_200"),
                    control_300: theme_row.get("control_300"),
                    control_400: theme_row.get("control_400"),
                    control_500: theme_row.get("control_500"),
                    control_600: theme_row.get("control_600"),
                    control_700: theme_row.get("control_700"),
                    control_800: theme_row.get("control_800"),
                    control_900: theme_row.get("control_900"),
                    control_950: theme_row.get("control_950"),
                    error_50: theme_row.get("error_50"),
                    error_100: theme_row.get("error_100"),
                    error_200: theme_row.get("error_200"),
                    error_300: theme_row.get("error_300"),
                    error_400: theme_row.get("error_400"),
                    error_500: theme_row.get("error_500"),
                    error_600: theme_row.get("error_600"),
                    error_700: theme_row.get("error_700"),
                    error_800: theme_row.get("error_800"),
                    error_900: theme_row.get("error_900"),
                    error_950: theme_row.get("error_950"),
                    warn_50: theme_row.get("warn_50"),
                    warn_100: theme_row.get("warn_100"),
                    warn_200: theme_row.get("warn_200"),
                    warn_300: theme_row.get("warn_300"),
                    warn_400: theme_row.get("warn_400"),
                    warn_500: theme_row.get("warn_500"),
                    warn_600: theme_row.get("warn_600"),
                    warn_700: theme_row.get("warn_700"),
                    warn_800: theme_row.get("warn_800"),
                    warn_900: theme_row.get("warn_900"),
                    warn_950: theme_row.get("warn_950"),
                    success_50: theme_row.get("success_50"),
                    success_100: theme_row.get("success_100"),
                    success_200: theme_row.get("success_200"),
                    success_300: theme_row.get("success_300"),
                    success_400: theme_row.get("success_400"),
                    success_500: theme_row.get("success_500"),
                    success_600: theme_row.get("success_600"),
                    success_700: theme_row.get("success_700"),
                    success_800: theme_row.get("success_800"),
                    success_900: theme_row.get("success_900"),
                    success_950: theme_row.get("success_950"),
                    created_at: theme_row.get("created_at"),
                    updated_at: theme_row.get("updated_at"),
                });
            }
            
            conversation.load_display(pool).await.ok();
            Ok(Some(conversation))
        } else {
            Ok(None)
        }
    }
    
    pub async fn get_members(&mut self, pool: &Pool<Sqlite>) -> Result<Vec<User>, String> {
        let member_rows = sqlx::query(
            r#"SELECT DISTINCT u.* FROM user u
               WHERE u.id IN (
                   SELECT DISTINCT sender_id FROM message 
                   WHERE conversation_id = ?1 
                   AND sender_id IS NOT NULL
               )"#
        )
        .bind(&self.id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to get conversation members from database: {}", e))?;
        
        if !member_rows.is_empty() {
            self.members.clear();
            for row in member_rows {
                self.members.push(User {
                    id: row.get("id"),
                    email: row.get("email"),
                    phone: row.get("phone"),
                    display_name: row.get("display_name"),
                    first_name: row.get("first_name"),
                    last_name: row.get("last_name"),
                    avatar_url: row.get("avatar_url"),
                    avatar_data: row.get("avatar_data"),
                    job_title: row.get("job_title"),
                    department: row.get("department"),
                    company: row.get("company"),
                    office_location: row.get("office_location"),
                    first_seen: row.get("first_seen"),
                    last_updated: row.get("last_updated"),
                });
            }
        }
        
        if self.providers.is_empty() {
            let provider_rows = sqlx::query(
                r#"SELECT p.* FROM provider p
                   JOIN conversation_provider cp ON p.id = cp.provider_id
                   WHERE cp.conversation_id = ?1"#
            )
            .bind(&self.id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load providers: {}", e))?;
            
            for row in provider_rows {
                let provider = crate::providers::models::Provider::from_row(row)?;
                self.providers.push(provider);
            }
        }
        
        if let Some(provider) = self.providers.first() {
            if provider.provider_type == "microsoft" && provider.auth.is_some() {
                let external_id = sqlx::query_scalar::<_, String>(
                    "SELECT external_id FROM conversation_provider WHERE conversation_id = ?1 AND provider_id = ?2"
                )
                .bind(&self.id)
                .bind(&provider.id)
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("Failed to get external conversation id: {}", e))?;
                
                if let Some(external_id) = external_id {
                    let microsoft_provider = crate::services::connection_service::microsoft_provider(pool).await?;
                    
                    if let Ok(api_members) = microsoft_provider.fetch_members(provider.auth.as_ref().unwrap(), &external_id).await {
                        use crate::models::user::{User, ProviderUser};
                        
                        for api_member in api_members {
                            let user = User::find_by_email_or_phone(
                                pool,
                                api_member.email.as_deref(),
                                api_member.phone.as_deref()
                            ).await?;
                            
                            let user_id = if let Some(existing_user) = user {
                                sqlx::query(
                                    r#"UPDATE user SET 
                                        display_name = COALESCE(?1, display_name),
                                        first_name = COALESCE(?2, first_name),
                                        last_name = COALESCE(?3, last_name),
                                        job_title = COALESCE(?4, job_title),
                                        department = COALESCE(?5, department),
                                        company = COALESCE(?6, company),
                                        office_location = COALESCE(?7, office_location),
                                        last_updated = ?8
                                    WHERE id = ?9"#
                                )
                                .bind(&api_member.display_name)
                                .bind(&api_member.first_name)
                                .bind(&api_member.last_name)
                                .bind(&api_member.job_title)
                                .bind(&api_member.department)
                                .bind(&api_member.company)
                                .bind(&api_member.office_location)
                                .bind(chrono::Utc::now().to_rfc3339())
                                .bind(&existing_user.id)
                                .execute(pool)
                                .await
                                .map_err(|e| format!("Failed to update user: {}", e))?;
                                existing_user.id
                            } else {
                                let new_user = User::create(
                                    pool,
                                    api_member.email.clone(),
                                    api_member.phone.clone(),
                                    api_member.display_name.clone(),
                                    api_member.first_name.clone(),
                                    api_member.last_name.clone(),
                                    api_member.avatar_url.clone(),
                                    None,
                                    api_member.job_title.clone(),
                                    api_member.department.clone(),
                                    api_member.company.clone(),
                                    api_member.office_location.clone(),
                                ).await?;
                                new_user.id
                            };
                            
                            let _ = ProviderUser::create_or_update(
                                pool,
                                user_id.clone(),
                                provider.id.clone(),
                                api_member.id.clone(),
                            ).await;
                            
                            if !self.members.iter().any(|m| m.id == user_id) {
                                if let Ok(Some(user)) = User::find_by_id(pool, &user_id).await {
                                    self.members.push(user);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(self.members.clone())
    }
    
    pub async fn sync(&self, pool: &Pool<Sqlite>, window: &tauri::Window) -> Result<(), String> {
        use crate::providers::models::Provider;
        
        // Get all providers associated with this conversation via conversation_provider table
        let provider_rows = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT cp.provider_id FROM conversation_provider cp 
             INNER JOIN provider p ON cp.provider_id = p.id 
             WHERE cp.conversation_id = ?1 AND p.is_enabled = 1"
        )
        .bind(&self.id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to get conversation providers: {}", e))?;
        
        // If no providers found for this conversation, skip sync
        if provider_rows.is_empty() {
            return Ok(());
        }
        
        for provider_id in provider_rows {
            if let Ok(provider) = Provider::load_from_db(pool, &provider_id).await {
                if provider.is_enabled {
                    provider.sync_conversation(pool, &self.id, window).await?;
                }
            }
        }
        
        Ok(())
    }
    
    pub async fn load_display(&mut self, pool: &Pool<Sqlite>) -> Result<(), String> {
        use crate::models::object_override::{resolve, FIELD_AVATAR, FIELD_DISPLAY_NAME, FIELD_NAME};
        if self.overrides.get(FIELD_NAME) != Some(&self.name) {
            self.provider_name = self.name.clone();
        }
        self.overrides = ObjectOverride::get_fields(pool, tag::OBJECT_CONVERSATION, &self.id).await?;
        self.nicknames = ObjectOverride::get_nicknames(pool, &self.id).await?;
        if let Some(name) = self.overrides.get(FIELD_NAME) {
            self.name = name.clone();
        }
        if let Some(image) = self.overrides.get(FIELD_AVATAR) {
            self.image = Some(image.clone());
        }
        if self.members.is_empty() {
            let rows = sqlx::query("SELECT u.* FROM conversation_member cm JOIN user u ON u.id = cm.user_id WHERE cm.conversation_id = ?1 ORDER BY COALESCE(u.display_name, u.email)")
                .bind(&self.id)
                .fetch_all(pool)
                .await
                .map_err(|e| format!("Failed to load the members: {}", e))?;
            for row in rows {
                self.members.push(User::from_row(row)?);
            }
        }
        let locals: std::collections::HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
            "SELECT pi.user_id, o.value FROM object_override o JOIN person_identity pi ON pi.person_id = o.object_id
             WHERE o.object_type = ?1 AND o.scope_type = '' AND o.field = ?2 AND pi.user_id IN (SELECT user_id FROM conversation_member WHERE conversation_id = ?3)",
        )
        .bind(tag::OBJECT_PERSON)
        .bind(FIELD_DISPLAY_NAME)
        .bind(&self.id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load local names: {}", e))?
        .into_iter()
        .collect();
        for member in &mut self.members {
            let local = locals.get(&member.id).map(String::as_str);
            let scoped = self.nicknames.get(&member.id).map(String::as_str);
            if local.is_some() || scoped.is_some() {
                let provider = member.display_name.clone().unwrap_or_default();
                member.display_name = Some(resolve(&provider, local, scoped).to_string());
            }
        }
        Ok(())
    }

    pub async fn put_name(pool: &Pool<Sqlite>, conversation_id: &str, name: &str, on_provider: bool) -> Result<bool, String> {
        use crate::models::object_override::{DeleteObjectOverrideInput, PutObjectOverrideInput, FIELD_NAME};
        let name = name.trim();
        let renamed = on_provider && !name.is_empty() && crate::services::outbound_service::put_name(pool, conversation_id, name).await?;
        if renamed {
            sqlx::query("UPDATE conversation SET name = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(name)
                .bind(chrono::Utc::now().to_rfc3339())
                .bind(conversation_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to update conversation name: {}", e))?;
            ObjectOverride::delete(pool, &DeleteObjectOverrideInput { object_type: tag::OBJECT_CONVERSATION.to_string(), object_id: conversation_id.to_string(), scope_type: None, scope_id: None, field: Some(FIELD_NAME.to_string()) }).await?;
        } else {
            ObjectOverride::put(pool, &PutObjectOverrideInput { object_type: tag::OBJECT_CONVERSATION.to_string(), object_id: conversation_id.to_string(), scope_type: None, scope_id: None, field: FIELD_NAME.to_string(), value: Some(name.to_string()) }).await?;
        }
        Ok(renamed)
    }

    pub async fn load_providers(&mut self, pool: &Pool<Sqlite>) -> Result<(), String> {
        let provider_rows = sqlx::query(
            r#"SELECT p.* FROM provider p
               JOIN conversation_provider cp ON p.id = cp.provider_id
               WHERE cp.conversation_id = ?1"#
        )
        .bind(&self.id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load providers: {}", e))?;
        
        self.providers.clear();
        
        for provider_row in provider_rows {
            let mut provider = crate::providers::models::Provider::from_row(provider_row)?;
            provider.load_logo(pool).await.ok();
            self.providers.push(provider);
        }
        
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailRecipient {
    pub name: Option<String>,
    #[serde(alias = "email", default, deserialize_with = "address_or_empty")]
    pub address: String,
}

fn address_or_empty<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(<Option<String> as Deserialize>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub content_type: String,
    pub size: i64,
    pub is_inline: bool,
    pub content_id: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: String,
    pub provider_id: String,
    pub external_id: String,

    // Message Type: email, chat, channel, other
    #[serde(rename = "type")]
    pub r#type: String,

    // Common fields
    pub subject: Option<String>,
    pub content: Option<String>,
    pub content_type: String,
    pub summary: Option<String>,
    pub presentation: Option<serde_json::Value>,

    // Sender info
    pub sender_id: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: Option<String>,

    // Recipients info (mainly for emails)
    pub recipients: Vec<EmailRecipient>,
    pub cc_recipients: Vec<EmailRecipient>,
    pub bcc_recipients: Vec<EmailRecipient>,

    // Chat/Teams specific fields
    pub conversation_id: Option<String>,
    pub chat_id: Option<String>,
    pub team_id: Option<String>,
    pub channel_id: Option<String>,

    // Metadata
    pub importance: String,

    // Flags
    pub is_read: bool,
    pub is_draft: bool,
    pub has_attachments: bool,
    pub attachments: Vec<Attachment>,

    // Threading
    pub thread_id: Option<String>,
    pub reply_to_id: Option<String>,
    pub conversation_index: Option<String>,

    // Timestamps
    pub created_datetime: String,
    pub last_modified_datetime: String,
    pub received_datetime: Option<String>,
    pub sent_datetime: Option<String>,
    pub deleted_datetime: Option<String>,
    pub timestamp: String, // Backwards compatibility

    // Sync metadata
    pub cached_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,

    // Legacy fields for backwards compatibility
    pub provider_name: Option<String>,
    pub metadata: Option<String>,
}

impl Message {
    pub async fn get_conversation_id(pool: &Pool<Sqlite>, id: &str) -> Result<Option<String>, String> {
        sqlx::query_scalar("SELECT conversation_id FROM message WHERE id = ?1").bind(id).fetch_optional(pool).await.map_err(|e| format!("Failed to find the message: {}", e))
    }

    /// Create a new message instance with default values
    pub fn new(
        id: String,
        provider_id: String,
        external_id: String,
        r#type: String,
        content: Option<String>,
        timestamp: String,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        
        Self {
            id,
            provider_id,
            external_id,
            r#type,
            subject: None,
            content,
            content_type: "text".to_string(),
            summary: None,
            presentation: None,
            sender_id: None,
            sender_name: None,
            sender_email: None,
            recipients: Vec::new(),
            cc_recipients: Vec::new(),
            bcc_recipients: Vec::new(),
            conversation_id: None,
            chat_id: None,
            team_id: None,
            channel_id: None,
            importance: "normal".to_string(),
            is_read: false,
            is_draft: false,
            has_attachments: false,
            attachments: Vec::new(),
            thread_id: None,
            reply_to_id: None,
            conversation_index: None,
            created_datetime: timestamp.clone(),
            last_modified_datetime: timestamp.clone(),
            received_datetime: None,
            sent_datetime: None,
            deleted_datetime: None,
            timestamp,
            cached_at: Some(now.clone()),
            created_at: now.clone(),
            updated_at: now,
            provider_name: None,
            metadata: None,
        }
    }
    
    /// Parse recipients from JSON string
    pub fn parse_recipients(json_str: Option<&str>) -> Vec<EmailRecipient> {
        match json_str {
            Some(json) if !json.trim().is_empty() => {
                serde_json::from_str(json).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }
    
    /// Parse attachments from JSON string
    pub fn parse_attachments(json_str: Option<&str>) -> Vec<Attachment> {
        match json_str {
            Some(json) if !json.trim().is_empty() => {
                serde_json::from_str(json).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }
    
    /// Check if this is an email message
    pub fn is_email(&self) -> bool {
        self.r#type == "email"
    }
    
    /// Check if this is a chat message
    pub fn is_chat(&self) -> bool {
        self.r#type == "chat"
    }
    
    /// Check if this is a channel/team message
    pub fn is_channel(&self) -> bool {
        self.r#type == "channel"
    }
    
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    #[serde(skip_serializing)]
    pub id: String,
    #[serde(skip_serializing)]
    pub profile_id: String,
    pub name: String,
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub asset_type: String,
    pub mime_type: String,
    pub path: Option<String>,
    #[serde(skip_serializing)]
    pub created_at: String,
    #[serde(skip_serializing)]
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationsResponse {
    pub data: Vec<Conversation>,
    pub is_loading: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationResponse {
    pub data: Option<Conversation>,
    pub is_loading: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileResponse {
    pub data: Option<Profile>,
    pub is_loading: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsersResponse {
    pub data: Vec<User>,
    pub is_loading: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersResponse {
    pub data: Vec<crate::providers::models::Provider>,
    pub is_loading: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesResponse {
    pub data: Vec<Message>,
    pub is_loading: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostProfileInput {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ThemeInput {
    pub theme_name: String,
    pub is_active: bool,
    pub primary_50: String,
    pub primary_100: String,
    pub primary_200: String,
    pub primary_300: String,
    pub primary_400: String,
    pub primary_500: String,
    pub primary_600: String,
    pub primary_700: String,
    pub primary_800: String,
    pub primary_900: String,
    pub primary_950: String,
    pub secondary_50: String,
    pub secondary_100: String,
    pub secondary_200: String,
    pub secondary_300: String,
    pub secondary_400: String,
    pub secondary_500: String,
    pub secondary_600: String,
    pub secondary_700: String,
    pub secondary_800: String,
    pub secondary_900: String,
    pub secondary_950: String,
    pub accent_50: String,
    pub accent_100: String,
    pub accent_200: String,
    pub accent_300: String,
    pub accent_400: String,
    pub accent_500: String,
    pub accent_600: String,
    pub accent_700: String,
    pub accent_800: String,
    pub accent_900: String,
    pub accent_950: String,
    pub background_50: String,
    pub background_100: String,
    pub background_200: String,
    pub background_300: String,
    pub background_400: String,
    pub background_500: String,
    pub background_600: String,
    pub background_700: String,
    pub background_800: String,
    pub background_900: String,
    pub background_950: String,
    pub content_50: String,
    pub content_100: String,
    pub content_200: String,
    pub content_300: String,
    pub content_400: String,
    pub content_500: String,
    pub content_600: String,
    pub content_700: String,
    pub content_800: String,
    pub content_900: String,
    pub content_950: String,
    pub control_50: String,
    pub control_100: String,
    pub control_200: String,
    pub control_300: String,
    pub control_400: String,
    pub control_500: String,
    pub control_600: String,
    pub control_700: String,
    pub control_800: String,
    pub control_900: String,
    pub control_950: String,
    pub error_50: String,
    pub error_100: String,
    pub error_200: String,
    pub error_300: String,
    pub error_400: String,
    pub error_500: String,
    pub error_600: String,
    pub error_700: String,
    pub error_800: String,
    pub error_900: String,
    pub error_950: String,
    pub warn_50: String,
    pub warn_100: String,
    pub warn_200: String,
    pub warn_300: String,
    pub warn_400: String,
    pub warn_500: String,
    pub warn_600: String,
    pub warn_700: String,
    pub warn_800: String,
    pub warn_900: String,
    pub warn_950: String,
    pub success_50: String,
    pub success_100: String,
    pub success_200: String,
    pub success_300: String,
    pub success_400: String,
    pub success_500: String,
    pub success_600: String,
    pub success_700: String,
    pub success_800: String,
    pub success_900: String,
    pub success_950: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostProfileThemeInput {
    pub theme: ThemeInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutProfileInput {
    pub name: Option<String>,
    pub avatar_id: Option<String>,
    pub theme_id: Option<String>,
    pub setup_step: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProfileSettingInput {
    pub email_content_threshold: i32,
    pub navigation_width: i32,
    pub navigation_collapsed: bool,
    pub details_width: i32,
    pub details_panel_open: bool,
    pub message_timestamp_interval: i32,
    pub time_format: String,
    pub window_controls_style: String,
    pub max_input_lines: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutProfileSettingsInput {
    pub settings: ProfileSettingInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutProfileThemeInput {
    pub theme_id: String,
    pub theme: ThemeInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostProfileAvatarInput {
    pub avatar_data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutProfileAvatarInput {
    pub avatar_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SendMessageInput {
    pub conversation_id: String,
    pub content: String,
    pub content_type: Option<String>,
    pub provider_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub attachments: Option<Vec<OutgoingAttachmentInput>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OutgoingAttachmentInput {
    pub content_id: Option<String>,
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub data: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SendMessageReactionInput {
    pub conversation_id: String,
    pub message_id: String,
    pub reaction: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConnectProviderInput {
    pub provider_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConnectImapInput {
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
    pub username: Option<String>,
    pub imap_host: String,
    pub imap_port: Option<u16>,
    pub smtp_host: String,
    pub smtp_port: Option<u16>,
    #[serde(default)]
    pub aliases: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GetConversationInput {
    pub conversation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SetCurrentConversationInput {
    pub conversation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct StartProviderAuthInput {
    pub provider_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateConversationInput {
    pub name: String,
    pub conversation_type: String,
    pub group_type: String,
    pub member_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MergeConversationsInput {
    pub source_conversation_id: String,
    pub target_conversation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LinkConversationsInput {
    pub source_conversation_id: String,
    pub target_conversation_id: String,
    pub link_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateThreadInput {
    pub message_id: String,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ReplyInThreadInput {
    pub thread_id: String,
    pub content: String,
    pub content_type: Option<String>,
    #[serde(default)]
    pub broadcast: Option<bool>,
    #[serde(default)]
    pub attachments: Option<Vec<OutgoingAttachmentInput>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GetThreadInput {
    pub thread_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateConversationInput {
    pub conversation_id: String,
    pub name: Option<String>,
    pub rename_on_provider: Option<bool>,
    pub group_type: Option<String>,
    pub is_pinned: Option<bool>,
    pub is_muted: Option<bool>,
    pub mode: Option<chat::ChatMode>,
    pub view: Option<chat::ChatView>,
    pub view_settings: Option<chat::ViewSettings>,
    pub is_archived: Option<bool>,
    #[serde(default, deserialize_with = "person::deserialize_double_option_string")]
    pub snoozed_until: Option<Option<String>>,
    #[serde(default, deserialize_with = "person::deserialize_double_option_string")]
    pub deleted_at: Option<Option<String>>,
    #[serde(default, deserialize_with = "person::deserialize_double_option")]
    pub favourite_position: Option<Option<i64>>,
    pub metadata: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConversationMemberInput {
    pub conversation_id: String,
    pub user_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SearchInput {
    pub query: String,
    pub limit: Option<u32>,
    pub conversation_ids: Option<Vec<String>>,
    pub tag_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct SearchResults {
    pub conversations: Vec<Conversation>,
    pub messages: Vec<Message>,
    pub snippets: Vec<crate::services::message_service::MessageSnippet>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EditMessageInput {
    pub message_id: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DeleteMessageInput {
    pub message_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PinMessageInput {
    pub conversation_id: String,
    pub message_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GetPinnedInput {
    pub conversation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BookmarkMessageInput {
    pub message_id: String,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UnbookmarkMessageInput {
    pub message_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MarkMessageUnreadInput {
    pub message_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MarkConversationReadInput {
    pub conversation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SetNotificationInput {
    pub conversation_id: String,
    pub notification_level: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateSidebarSectionInput {
    pub name: String,
    pub position: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateSidebarSectionInput {
    pub id: String,
    pub name: Option<String>,
    pub position: Option<i32>,
    pub is_collapsed: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DeleteSidebarSectionInput {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SidebarSectionItemInput {
    pub section_id: String,
    pub conversation_id: String,
    pub position: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RemoveSidebarSectionItemInput {
    pub section_id: String,
    pub conversation_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SidebarSection {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub position: i32,
    pub is_collapsed: bool,
    pub conversations: Vec<Conversation>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinnedMessage {
    pub id: String,
    pub conversation_id: String,
    pub message_id: String,
    pub pinned_by: Option<String>,
    pub created_at: String,
    pub message: Option<Message>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BookmarkedMessage {
    pub id: String,
    pub message_id: String,
    pub profile_id: String,
    pub note: Option<String>,
    pub created_at: String,
    pub message: Option<Message>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SetUserStatusInput {
    pub status: String,
    pub status_text: String,
    pub status_emoji: String,
    pub status_expiration: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GetUserStatusInput {
    pub user_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserStatus {
    pub user_id: String,
    pub status: String,
    pub status_text: String,
    pub status_emoji: String,
    pub status_expiration: Option<String>,
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[tokio::test]
    async fn conversation_from_row() {
        let pool = test_pool().await;
        let conv_id = test_conversation(&pool, "Test Chat", "chat").await;

        let row = sqlx::query("SELECT * FROM conversation WHERE id = ?1")
            .bind(&conv_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        let conversation = Conversation::from_row(row).unwrap();
        assert_eq!(conversation.id, conv_id);
        assert_eq!(conversation.name, "Test Chat");
        assert_eq!(conversation.conversation_type, "chat");
        assert_eq!(conversation.group_type, "auto");
    }

    fn merge_input(lead: &str, conversations: &[&str], groups: &[&str]) -> MergeGroupInput {
        MergeGroupInput { lead_conversation_id: lead.to_string(), conversation_ids: conversations.iter().map(|c| c.to_string()).collect(), group_lead_ids: groups.iter().map(|g| g.to_string()).collect() }
    }

    async fn flattened(pool: &Pool<Sqlite>, profile: &str, lead: &str) -> Vec<String> {
        let mut ids: Vec<String> = sqlx::query_scalar("SELECT gc.conversation_id FROM merge_group g JOIN merge_group_conversation gc ON gc.merge_group_id = g.id WHERE g.profile_id = ?1 AND g.lead_conversation_id = ?2")
            .bind(profile)
            .bind(lead)
            .fetch_all(pool)
            .await
            .unwrap();
        ids.sort();
        ids
    }

    #[tokio::test]
    async fn merge_groups_store_members_nesting_and_flattened_membership() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let mut ids = Vec::new();
        for name in ["a", "b", "c", "d"] {
            ids.push(test_conversation(&pool, name, "direct_message").await);
        }
        let (a, b, c, d) = (ids[0].as_str(), ids[1].as_str(), ids[2].as_str(), ids[3].as_str());
        MergeGroup::post(&pool, &profile, &merge_input(a, &[b], &[])).await.unwrap();
        MergeGroup::post(&pool, &profile, &merge_input(c, &[d], &[])).await.unwrap();
        MergeGroup::post(&pool, &profile, &merge_input(a, &[], &[c])).await.unwrap();
        let groups = MergeGroup::get_all(&pool, &profile).await.unwrap();
        assert_eq!(groups.len(), 2);
        let top = groups.iter().find(|g| g.lead_conversation_id == a).unwrap();
        let child = groups.iter().find(|g| g.lead_conversation_id == c).unwrap();
        assert_eq!(top.conversation_ids, vec![a.to_string(), b.to_string()]);
        assert_eq!(child.parent_id.as_deref(), Some(top.id.as_str()));
        let mut all = vec![a.to_string(), b.to_string(), c.to_string(), d.to_string()];
        all.sort();
        assert_eq!(flattened(&pool, &profile, a).await, all);

        MergeGroup::post(&pool, &profile, &merge_input(c, &[], &[a])).await.unwrap();
        assert!(MergeGroup::get_all(&pool, &profile).await.unwrap().iter().all(|g| g.lead_conversation_id != a || g.parent_id.is_none()));

        MergeGroup::delete_items(&pool, &profile, &merge_input(a, &[a], &[])).await.unwrap();
        let groups = MergeGroup::get_all(&pool, &profile).await.unwrap();
        let top = groups.iter().find(|g| g.conversation_ids == vec![b.to_string()]).unwrap();
        assert_eq!(top.lead_conversation_id, b);
        assert_eq!(groups.iter().find(|g| g.lead_conversation_id == c).unwrap().parent_id.as_deref(), Some(top.id.as_str()));

        MergeGroup::delete_items(&pool, &profile, &merge_input(b, &[], &[c])).await.unwrap();
        let groups = MergeGroup::get_all(&pool, &profile).await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].lead_conversation_id, c);
        assert!(flattened(&pool, &profile, b).await.is_empty());
    }

    #[tokio::test]
    async fn merge_group_tags_reach_every_member_and_leave_with_the_group() {
        use crate::models::tag::{ObjectTag, Tag, KIND_TAG, OBJECT_MERGE_GROUP};
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let mut ids = Vec::new();
        for name in ["a", "b", "c", "d", "e"] {
            ids.push(test_conversation(&pool, name, "direct_message").await);
        }
        let (a, b, c, d) = (ids[0].as_str(), ids[1].as_str(), ids[2].as_str(), ids[3].as_str());
        MergeGroup::post(&pool, &profile, &merge_input(a, &[b], &[])).await.unwrap();
        MergeGroup::post(&pool, &profile, &merge_input(c, &[d], &[])).await.unwrap();
        MergeGroup::post(&pool, &profile, &merge_input(a, &[], &[c])).await.unwrap();
        let groups = MergeGroup::get_all(&pool, &profile).await.unwrap();
        let top = groups.iter().find(|g| g.lead_conversation_id == a).unwrap().id.clone();
        let launch = Tag::post(&pool, &profile, "Launch", "#1f9d55", KIND_TAG, 0, "").await.unwrap();
        ObjectTag::put(&pool, OBJECT_MERGE_GROUP, &top, &launch.id, "manual", None, None).await.unwrap();
        let mut tagged = ObjectTag::get_conversation_ids(&pool, &launch.id).await.unwrap();
        tagged.sort();
        let mut all = vec![a.to_string(), b.to_string(), c.to_string(), d.to_string()];
        all.sort();
        assert_eq!(tagged, all);
        assert!(ObjectTag::get(&pool, crate::models::tag::OBJECT_CONVERSATION, a).await.unwrap().is_empty());
        let listed = ObjectTag::get_tagged(&pool, &profile, Some(&launch.id)).await.unwrap();
        assert_eq!(listed.iter().map(|t| (t.object_type.as_str(), t.object_id.as_str(), t.conversation_id.as_deref())).collect::<Vec<_>>(), vec![(OBJECT_MERGE_GROUP, top.as_str(), Some(a))]);

        MergeGroup::delete_items(&pool, &profile, &merge_input(a, &[b], &[c])).await.unwrap();
        assert!(ObjectTag::get(&pool, OBJECT_MERGE_GROUP, &top).await.unwrap().is_empty());
        assert!(ObjectTag::get_conversation_ids(&pool, &launch.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn merge_group_messages_come_from_every_member_in_one_timeline() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "mock").await;
        let a = test_conversation(&pool, "a", "direct_message").await;
        let b = test_conversation(&pool, "b", "direct_message").await;
        let c = test_conversation(&pool, "c", "direct_message").await;
        MergeGroup::post(&pool, &profile, &merge_input(&a, &[&b], &[])).await.unwrap();
        test_message(&pool, &a, &provider, "from a").await;
        test_message(&pool, &b, &provider, "from b").await;
        test_message(&pool, &c, &provider, "from c").await;
        let service = crate::services::MessageService::new(pool.clone());
        let messages = service.get_merge_group_messages(&profile, &a, 200, 0).await.unwrap();
        let mut owners: Vec<String> = messages.iter().filter_map(|m| m.conversation_id.clone()).collect();
        owners.sort();
        let mut expected = vec![a.clone(), b.clone()];
        expected.sort();
        assert_eq!(owners, expected);
        assert_eq!(service.get_merge_group_messages(&profile, &a, 1, 0).await.unwrap().len(), 1);
    }

    #[test]
    fn link_tree_leads_with_the_drop_target_and_refuses_nest_cycles() {
        let link = |s: &str, t: &str, k: &str| (s.to_string(), t.to_string(), k.to_string());
        let (groups, parents) = MergeGroup::get_link_tree(&[
            link("b", "x", MERGE_LINK),
            link("b", "r", MERGE_LINK),
            link("r", "x", MERGE_LINK),
            link("d", "c", MERGE_LINK),
            link("c", "x", NEST_LINK),
            link("x", "d", NEST_LINK),
            link("e", "x", NEST_LINK),
        ]);
        assert_eq!(groups, vec![("x".to_string(), vec!["x".to_string(), "b".to_string(), "r".to_string()]), ("c".to_string(), vec!["c".to_string(), "d".to_string()]), ("e".to_string(), vec!["e".to_string()])]);
        assert_eq!(parents, vec![("c".to_string(), "x".to_string()), ("e".to_string(), "x".to_string())]);
    }

    #[tokio::test]
    async fn merge_groups_are_built_once_from_existing_links() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "mock").await;
        let a = test_conversation(&pool, "a", "direct_message").await;
        let b = test_conversation(&pool, "b", "direct_message").await;
        let c = test_conversation(&pool, "c", "direct_message").await;
        for (index, conversation) in [&a, &b, &c].iter().enumerate() {
            sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id) VALUES (?1, ?2, ?3, ?4)")
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(conversation)
                .bind(&provider)
                .bind(format!("ext-{}", index))
                .execute(&pool)
                .await
                .unwrap();
        }
        ConversationLink::post(&pool, &b, &a, MERGE_LINK).await.unwrap();
        ConversationLink::post(&pool, &c, &a, NEST_LINK).await.unwrap();
        MergeGroup::post_links(&pool).await.unwrap();
        let groups = MergeGroup::get_all(&pool, &profile).await.unwrap();
        let top = groups.iter().find(|g| g.lead_conversation_id == a).unwrap();
        assert_eq!(top.conversation_ids, vec![a.clone(), b.clone()]);
        assert_eq!(groups.iter().find(|g| g.lead_conversation_id == c).unwrap().parent_id.as_deref(), Some(top.id.as_str()));
        let mut all = vec![a.clone(), b.clone(), c.clone()];
        all.sort();
        assert_eq!(flattened(&pool, &profile, &a).await, all);
    }

    #[test]
    fn message_new_defaults() {
        let msg = Message::new(
            "msg-1".to_string(),
            "prov-1".to_string(),
            "ext-1".to_string(),
            "chat".to_string(),
            Some("Hello".to_string()),
            "2024-01-01T00:00:00Z".to_string(),
        );

        assert_eq!(msg.id, "msg-1");
        assert_eq!(msg.provider_id, "prov-1");
        assert_eq!(msg.external_id, "ext-1");
        assert_eq!(msg.r#type, "chat");
        assert_eq!(msg.content, Some("Hello".to_string()));
        assert_eq!(msg.content_type, "text");
        assert_eq!(msg.importance, "normal");
        assert!(!msg.is_read);
        assert!(!msg.is_draft);
        assert!(!msg.has_attachments);
        assert!(msg.recipients.is_empty());
        assert!(msg.attachments.is_empty());
        assert_eq!(msg.timestamp, "2024-01-01T00:00:00Z");
        assert_eq!(msg.created_datetime, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn message_is_email() {
        let msg = Message::new(
            "m".into(), "p".into(), "e".into(),
            "email".into(), None, "t".into(),
        );
        assert!(msg.is_email());
        assert!(!msg.is_chat());
        assert!(!msg.is_channel());
    }

    #[test]
    fn message_is_chat() {
        let msg = Message::new(
            "m".into(), "p".into(), "e".into(),
            "chat".into(), None, "t".into(),
        );
        assert!(msg.is_chat());
        assert!(!msg.is_email());
        assert!(!msg.is_channel());
    }

    #[test]
    fn message_is_channel() {
        let msg = Message::new(
            "m".into(), "p".into(), "e".into(),
            "channel".into(), None, "t".into(),
        );
        assert!(msg.is_channel());
        assert!(!msg.is_email());
        assert!(!msg.is_chat());
    }

    #[test]
    fn parse_recipients_valid_json() {
        let json = r#"[{"name":"Alice","address":"alice@example.com"}]"#;
        let result = Message::parse_recipients(Some(json));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].address, "alice@example.com");
        assert_eq!(result[0].name, Some("Alice".to_string()));
    }

    #[test]
    fn parse_recipients_empty_string() {
        let result = Message::parse_recipients(Some(""));
        assert!(result.is_empty());
    }

    #[test]
    fn parse_recipients_none() {
        let result = Message::parse_recipients(None);
        assert!(result.is_empty());
    }

    #[test]
    fn parse_recipients_whitespace_only() {
        let result = Message::parse_recipients(Some("   "));
        assert!(result.is_empty());
    }

    #[test]
    fn parse_recipients_invalid_json() {
        let result = Message::parse_recipients(Some("not json"));
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn thread_creation_and_retrieval() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let conv_id = test_conversation(&pool, "Thread Test", "chat").await;
        let provider_id = test_provider(&pool, &profile_id, "microsoft").await;
        let msg_id = test_message(&pool, &conv_id, &provider_id, "Root message").await;

        let thread_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO thread (id, conversation_id, root_message_id, last_activity, message_count, is_resolved, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 0, 0, ?4, ?4)"
        )
        .bind(&thread_id)
        .bind(&conv_id)
        .bind(&msg_id)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let reply_id = test_message(&pool, &conv_id, &provider_id, "Reply message").await;
        let tm_id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO thread_message (id, thread_id, message_id, position, created_at) VALUES (?1, ?2, ?3, 1, ?4)"
        )
        .bind(&tm_id)
        .bind(&thread_id)
        .bind(&reply_id)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let row = sqlx::query("SELECT * FROM thread WHERE id = ?1")
            .bind(&thread_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        let fetched_conv_id: String = row.get("conversation_id");
        let fetched_root: String = row.get("root_message_id");
        assert_eq!(fetched_conv_id, conv_id);
        assert_eq!(fetched_root, msg_id);

        let thread_msgs: Vec<sqlx::sqlite::SqliteRow> =
            sqlx::query("SELECT * FROM thread_message WHERE thread_id = ?1")
                .bind(&thread_id)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(thread_msgs.len(), 1);
    }

    #[tokio::test]
    async fn conversation_pinned_and_muted_flags() {
        let pool = test_pool().await;
        let conv_id = test_conversation(&pool, "Flags Test", "chat").await;

        sqlx::query("UPDATE conversation SET is_pinned = 1, is_muted = 1 WHERE id = ?1")
            .bind(&conv_id)
            .execute(&pool)
            .await
            .unwrap();

        let row = sqlx::query("SELECT * FROM conversation WHERE id = ?1")
            .bind(&conv_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        let conversation = Conversation::from_row(row).unwrap();
        assert!(conversation.is_pinned);
        assert!(conversation.is_muted);
    }

    async fn mocked(pool: &Pool<Sqlite>, profile: &str, kind: crate::providers::mock::MockKind) -> String {
        let id = test_provider(pool, profile, "mock").await;
        sqlx::query("UPDATE provider SET external_id = ?1, namespace = ?2, config = ?2, name = ?3, access_token = 'mock', expires_at = ?4 WHERE id = ?5")
            .bind(format!("mock-{}-me", kind.key()))
            .bind(kind.key())
            .bind(kind.label())
            .bind((chrono::Utc::now() + chrono::Duration::days(365)).to_rfc3339())
            .bind(&id)
            .execute(pool)
            .await
            .unwrap();
        let provider = crate::providers::models::Provider::load_from_db(pool, &id).await.unwrap();
        provider.ingest(pool, None, std::sync::Arc::new(crate::providers::mock::MockProvider::new(kind, pool.clone()))).await.unwrap();
        id
    }

    async fn shown(pool: &Pool<Sqlite>, id: &str) -> Conversation {
        let mut conversation = Conversation::load_with_providers(pool, id).await.unwrap().unwrap();
        conversation.load_display(pool).await.unwrap();
        conversation
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_chat_is_renamed_locally_unless_the_provider_takes_the_name() {
        use crate::providers::mock::MockKind;
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        AppSetting::put(&pool, app_setting::MOCK_ENABLED, "true").await.unwrap();
        let teams = mocked(&pool, &profile, MockKind::Teams).await;
        let group: String = sqlx::query_scalar("SELECT conversation_id FROM conversation_provider WHERE provider_id = ?1 AND external_id = 'mock-teams-group-0'").bind(&teams).fetch_one(&pool).await.unwrap();
        let original = shown(&pool, &group).await.name;

        assert!(!Conversation::put_name(&pool, &group, "Kaffeklubben", false).await.unwrap());
        let local = shown(&pool, &group).await;
        assert_eq!((local.name.as_str(), local.provider_name.as_str()), ("Kaffeklubben", original.as_str()));
        assert_eq!(local.overrides.get("name").map(String::as_str), Some("Kaffeklubben"));
        let stored: String = sqlx::query_scalar("SELECT name FROM conversation WHERE id = ?1").bind(&group).fetch_one(&pool).await.unwrap();
        assert_eq!(stored, original, "a local rename never touches the synced name");

        assert!(Conversation::put_name(&pool, &group, "Mandagsmøtet", true).await.unwrap());
        let renamed = shown(&pool, &group).await;
        assert_eq!((renamed.name.as_str(), renamed.provider_name.as_str()), ("Mandagsmøtet", "Mandagsmøtet"));
        assert!(renamed.overrides.get("name").is_none(), "the provider name replaces the local one");

        let outlook = mocked(&pool, &profile, MockKind::Outlook).await;
        let thread: String = sqlx::query_scalar("SELECT c.id FROM conversation c JOIN conversation_provider cp ON cp.conversation_id = c.id WHERE cp.provider_id = ?1 AND c.type = 'email' LIMIT 1").bind(&outlook).fetch_one(&pool).await.unwrap();
        assert!(!Conversation::put_name(&pool, &thread, "Ny tittel", true).await.unwrap(), "mail keeps renames in Chatnip");
        assert_eq!(shown(&pool, &thread).await.name, "Ny tittel");
    }

    #[tokio::test]
    async fn the_display_resolves_member_names_and_the_provider_picture() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "slack").await;
        let chat = test_conversation(&pool, "Design", "group_message").await;
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id, created_at) VALUES ('cp', ?1, ?2, 'C1', ?3)").bind(&chat).bind(&provider).bind(chrono::Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
        sqlx::query("UPDATE conversation SET metadata = '{\"image\":\"https://slack/pic.png\",\"topic\":\"x\"}' WHERE id = ?1").bind(&chat).execute(&pool).await.unwrap();
        let ingrid = test_user(&pool, "ingrid@x.no", "Ingrid Hansen").await;
        let ola = test_user(&pool, "ola@x.no", "Ola Nordmann").await;
        let now = chrono::Utc::now().to_rfc3339();
        for (user, person) in [(&ingrid, "p-ingrid"), (&ola, "p-ola")] {
            sqlx::query("INSERT INTO conversation_member (id, conversation_id, user_id) VALUES (?1, ?2, ?3)").bind(uuid::Uuid::new_v4().to_string()).bind(&chat).bind(user).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO person (id, profile_id, display_name, created_at, updated_at) VALUES (?1, ?2, 'x', ?3, ?3)").bind(person).bind(&profile).bind(&now).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO person_identity (id, person_id, user_id, provider_type, external_id) VALUES (?1, ?2, ?3, 'slack', ?1)").bind(uuid::Uuid::new_v4().to_string()).bind(person).bind(user).execute(&pool).await.unwrap();
        }
        let put = |object_id: &str, scope: Option<&str>, value: &str| object_override::PutObjectOverrideInput { object_type: "person".into(), object_id: object_id.into(), scope_type: scope.map(|_| "conversation".to_string()), scope_id: scope.map(String::from), field: "display_name".into(), value: Some(value.into()) };
        ObjectOverride::put(&pool, &put("p-ingrid", None, "Ingrid")).await.unwrap();
        ObjectOverride::put(&pool, &put("p-ingrid", Some(&chat), "Inga")).await.unwrap();
        ObjectOverride::put(&pool, &put("p-ola", None, "Ola N.")).await.unwrap();
        let shown = shown(&pool, &chat).await;
        assert_eq!(shown.image.as_deref(), Some("https://slack/pic.png"));
        let names: std::collections::BTreeMap<String, String> = shown.members.iter().map(|m| (m.id.clone(), m.display_name.clone().unwrap_or_default())).collect();
        assert_eq!(names.get(&ingrid).map(String::as_str), Some("Inga"));
        assert_eq!(names.get(&ola).map(String::as_str), Some("Ola N."));
        assert_eq!(shown.nicknames.get(&ingrid).map(String::as_str), Some("Inga"));
        assert!(!shown.nicknames.contains_key(&ola));
    }

    #[tokio::test]
    async fn the_first_connection_creates_the_profile_from_its_identity() {
        use crate::providers::traits::{ConnectionIdentity, ResponseUser};
        let pool = test_pool().await;
        assert!(Profile::get_active(&pool).await.unwrap().is_none());
        let identity = ConnectionIdentity {
            namespace: "tenant".into(),
            name: "Northwind".into(),
            external_id: Some("tenant".into()),
            user: Some(ResponseUser { id: "me-1".into(), display_name: Some("Milo Oakes".into()), email: Some("milo@northwind.example".into()), ..Default::default() }),
            logo_url: None,
        };
        let profile = Profile::get_or_post(&pool, &identity.display_name()).await.unwrap();
        assert_eq!(profile.name, "Milo Oakes");
        assert_eq!(profile.update_setup_step(&pool).await.unwrap(), 2);
        let provider = test_provider(&pool, &profile.id, "microsoft").await;
        sqlx::query("UPDATE provider SET access_token = 'x', expires_at = ?1 WHERE id = ?2").bind(chrono::Utc::now().to_rfc3339()).bind(&provider).execute(&pool).await.unwrap();
        assert_eq!(profile.update_setup_step(&pool).await.unwrap(), 3);
        let again = Profile::get_or_post(&pool, "Someone else").await.unwrap();
        assert_eq!(again.id, profile.id);
        assert_eq!(again.name, "Milo Oakes");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM profile").fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
        let settings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM profile_setting WHERE profile_id = ?1").bind(&profile.id).fetch_one(&pool).await.unwrap();
        assert_eq!(settings, 1);
    }

    #[test]
    fn an_identity_names_the_profile_by_person_then_address_then_connection() {
        use crate::providers::traits::{ConnectionIdentity, ResponseUser};
        let mut identity = ConnectionIdentity { namespace: "t".into(), name: "Acme Slack".into(), external_id: None, user: None, logo_url: None };
        assert_eq!(identity.display_name(), "Acme Slack");
        identity.user = Some(ResponseUser { id: "u".into(), email: Some("m@acme.no".into()), display_name: Some("  ".into()), ..Default::default() });
        assert_eq!(identity.display_name(), "m@acme.no");
        identity.user = Some(ResponseUser { id: "u".into(), email: Some("m@acme.no".into()), display_name: Some("Milo".into()), ..Default::default() });
        assert_eq!(identity.display_name(), "Milo");
    }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostTagInput {
    pub name: String,
    pub color: Option<String>,
    pub kind: Option<String>,
    pub position: Option<i64>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutTagInput {
    pub tag_id: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub position: Option<i64>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub favourite: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EventRangeInput {
    pub start: String,
    pub end: String,
    pub include_dismissed: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PostEventInput {
    pub draft: event::EventDraft,
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub online_meeting: bool,
}

#[derive(Debug, Serialize)]
pub struct PostEventResult {
    pub events: Vec<event::Event>,
    pub failures: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutEventInput {
    pub event_id: String,
    pub draft: Option<event::EventDraft>,
    pub status: Option<String>,
}
