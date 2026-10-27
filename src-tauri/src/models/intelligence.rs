use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const KIND_CLAUDE_CLI: &str = "claude_cli";
pub const KIND_REPLICATE: &str = "replicate";
pub const KIND_OPENAI_COMPATIBLE: &str = "openai_compatible";
pub const KIND_COMMAND: &str = "command";
pub const KIND_OLLAMA: &str = "ollama";
pub const KINDS: [&str; 5] = [KIND_CLAUDE_CLI, KIND_OLLAMA, KIND_REPLICATE, KIND_OPENAI_COMPATIBLE, KIND_COMMAND];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceConnection {
    pub id: String,
    pub profile_id: String,
    pub kind: String,
    pub name: String,
    pub config: BTreeMap<String, String>,
    pub is_default: bool,
    pub available: bool,
    pub detail: Option<String>,
    pub models: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostIntelligenceConnectionInput {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub config: BTreeMap<String, String>,
    #[serde(default)]
    pub is_default: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutIntelligenceConnectionInput {
    pub connection_id: String,
    pub name: Option<String>,
    pub config: Option<BTreeMap<String, String>>,
    pub is_default: Option<bool>,
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS intelligence_connection (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            config TEXT NOT NULL DEFAULT '{}',
            is_default BOOLEAN NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profile (id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    let columns = sqlx::query("PRAGMA table_info(assistant_run)").fetch_all(pool).await?;
    let has_connection = columns.iter().any(|c| c.get::<String, _>("name") == "connection");
    if !has_connection {
        sqlx::query("ALTER TABLE assistant_run ADD COLUMN connection TEXT NOT NULL DEFAULT ''").execute(pool).await?;
    }
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS section_summary (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            scope TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'section',
            start_message_id TEXT NOT NULL,
            end_message_id TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            message_count INTEGER NOT NULL DEFAULT 0,
            summary TEXT NOT NULL,
            action_items TEXT NOT NULL DEFAULT '[]',
            connection TEXT NOT NULL DEFAULT '',
            model TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE (profile_id, scope, kind, start_message_id),
            FOREIGN KEY (profile_id) REFERENCES profile (id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub const SECTION_KIND_SECTION: &str = "section";
pub const SECTION_KIND_ALL: &str = "all";
pub const SECTION_KIND_SELECTION: &str = "selection";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SectionSummary {
    pub id: String,
    pub scope: String,
    pub kind: String,
    pub start_message_id: String,
    pub end_message_id: String,
    pub content_hash: String,
    pub message_count: i64,
    pub summary: String,
    pub action_items: Vec<String>,
    pub connection: String,
    pub model: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub stale: bool,
}

pub struct PutSectionSummary<'a> {
    pub scope: &'a str,
    pub kind: &'a str,
    pub start_message_id: &'a str,
    pub end_message_id: &'a str,
    pub content_hash: &'a str,
    pub message_count: i64,
    pub summary: &'a str,
    pub action_items: &'a [String],
    pub connection: &'a str,
    pub model: Option<&'a str>,
}

impl SectionSummary {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        let items: String = row.get("action_items");
        Self {
            id: row.get("id"),
            scope: row.get("scope"),
            kind: row.get("kind"),
            start_message_id: row.get("start_message_id"),
            end_message_id: row.get("end_message_id"),
            content_hash: row.get("content_hash"),
            message_count: row.get("message_count"),
            summary: row.get("summary"),
            action_items: serde_json::from_str(&items).unwrap_or_default(),
            connection: row.get("connection"),
            model: row.try_get("model").ok().flatten(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            stale: false,
        }
    }

    pub fn checked(mut self, end_message_id: &str, content_hash: &str) -> Self {
        self.stale = self.end_message_id != end_message_id || self.content_hash != content_hash;
        self
    }

    pub async fn get(pool: &Pool<Sqlite>, profile_id: &str, scope: &str, kind: &str, start_message_id: &str) -> Result<Option<Self>, String> {
        let row = sqlx::query("SELECT * FROM section_summary WHERE profile_id = ?1 AND scope = ?2 AND kind = ?3 AND start_message_id = ?4")
            .bind(profile_id)
            .bind(scope)
            .bind(kind)
            .bind(start_message_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to read the section summary: {}", e))?;
        Ok(row.map(Self::from_row))
    }

    pub async fn put(pool: &Pool<Sqlite>, profile_id: &str, input: PutSectionSummary<'_>) -> Result<Self, String> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO section_summary (id, profile_id, scope, kind, start_message_id, end_message_id, content_hash, message_count, summary, action_items, connection, model, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
             ON CONFLICT (profile_id, scope, kind, start_message_id) DO UPDATE SET end_message_id = excluded.end_message_id, content_hash = excluded.content_hash, message_count = excluded.message_count,
             summary = excluded.summary, action_items = excluded.action_items, connection = excluded.connection, model = excluded.model, updated_at = excluded.updated_at",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(profile_id)
        .bind(input.scope)
        .bind(input.kind)
        .bind(input.start_message_id)
        .bind(input.end_message_id)
        .bind(input.content_hash)
        .bind(input.message_count)
        .bind(input.summary)
        .bind(serde_json::to_string(input.action_items).unwrap_or_else(|_| "[]".to_string()))
        .bind(input.connection)
        .bind(input.model)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to keep the section summary: {}", e))?;
        Self::get(pool, profile_id, input.scope, input.kind, input.start_message_id).await?.ok_or_else(|| "The section summary vanished after saving".to_string())
    }
}

fn parse_config(raw: &str) -> BTreeMap<String, String> {
    serde_json::from_str::<BTreeMap<String, serde_json::Value>>(raw)
        .map(|m| {
            m.into_iter()
                .map(|(k, v)| (k, match v {
                    serde_json::Value::String(s) => s,
                    other => other.to_string(),
                }))
                .collect()
        })
        .unwrap_or_default()
}

impl IntelligenceConnection {
    pub fn is_secret_field(key: &str) -> bool {
        matches!(key, "api_key" | "api_token") || key.ends_with("_secret")
    }

    fn seal_config(profile_id: &str, connection_id: &str, config: BTreeMap<String, String>) -> Result<BTreeMap<String, String>, String> {
        use crate::services::security_service::{profile_vault, seal};
        let vault = profile_vault(profile_id);
        let mut sealed = BTreeMap::new();
        for (key, value) in config {
            if Self::is_secret_field(&key) {
                if let Some(marker) = seal(&vault, &format!("intelligence:{}:{}", connection_id, key), Some(&value))? {
                    sealed.insert(key, marker);
                }
            } else {
                sealed.insert(key, value);
            }
        }
        Ok(sealed)
    }

    pub async fn seal_existing(pool: &Pool<Sqlite>) -> Result<usize, String> {
        let rows = sqlx::query("SELECT * FROM intelligence_connection").fetch_all(pool).await.map_err(|e| format!("Failed to read intelligence connections: {}", e))?;
        let mut moved = 0;
        for connection in rows.into_iter().map(Self::from_row) {
            let sealed = Self::seal_config(&connection.profile_id, &connection.id, connection.config.clone())?;
            if sealed != connection.config {
                sqlx::query("UPDATE intelligence_connection SET config = ?1 WHERE id = ?2")
                    .bind(serde_json::to_string(&sealed).unwrap_or_else(|_| "{}".to_string()))
                    .bind(&connection.id)
                    .execute(pool)
                    .await
                    .map_err(|e| format!("Failed to mark the connection as moved: {}", e))?;
                moved += 1;
            }
        }
        Ok(moved)
    }

    fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        let raw: String = row.get("config");
        Self {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            kind: row.get("kind"),
            name: row.get("name"),
            config: parse_config(&raw),
            is_default: row.get::<bool, _>("is_default"),
            available: false,
            detail: None,
            models: Vec::new(),
            created_at: row.get("created_at"),
        }
    }

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Self>, String> {
        let rows = sqlx::query("SELECT * FROM intelligence_connection WHERE profile_id = ?1 ORDER BY is_default DESC, created_at ASC")
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list intelligence connections: {}", e))?;
        Ok(rows.into_iter().map(Self::from_row).collect())
    }

    pub async fn get(pool: &Pool<Sqlite>, connection_id: &str) -> Result<Option<Self>, String> {
        let row = sqlx::query("SELECT * FROM intelligence_connection WHERE id = ?1")
            .bind(connection_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load the intelligence connection: {}", e))?;
        Ok(row.map(Self::from_row))
    }

    pub async fn default_for(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Option<Self>, String> {
        Ok(Self::all(pool, profile_id).await?.into_iter().next())
    }

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, input: PostIntelligenceConnectionInput) -> Result<Self, String> {
        if !KINDS.contains(&input.kind.as_str()) {
            return Err(format!("Unknown intelligence kind {}", input.kind));
        }
        if input.name.trim().is_empty() {
            return Err("A connection needs a name".to_string());
        }
        let existing = Self::all(pool, profile_id).await?;
        let make_default = input.is_default.unwrap_or(existing.is_empty());
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        if make_default {
            Self::clear_default(pool, profile_id).await?;
        }
        let config = Self::seal_config(profile_id, &id, input.config.clone())?;
        sqlx::query("INSERT INTO intelligence_connection (id, profile_id, kind, name, config, is_default, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")
            .bind(&id)
            .bind(profile_id)
            .bind(&input.kind)
            .bind(input.name.trim())
            .bind(serde_json::to_string(&config).unwrap_or_else(|_| "{}".to_string()))
            .bind(make_default)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to create the intelligence connection: {}", e))?;
        Self::get(pool, &id).await?.ok_or_else(|| "The connection vanished after creation".to_string())
    }

    pub async fn put(pool: &Pool<Sqlite>, input: PutIntelligenceConnectionInput) -> Result<Self, String> {
        let current = Self::get(pool, &input.connection_id).await?.ok_or_else(|| "Connection not found".to_string())?;
        let name = input.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or(current.name.clone());
        let config = Self::seal_config(&current.profile_id, &current.id, input.config.unwrap_or(current.config.clone()))?;
        if input.is_default == Some(true) {
            Self::clear_default(pool, &current.profile_id).await?;
        }
        let is_default = input.is_default.unwrap_or(current.is_default);
        sqlx::query("UPDATE intelligence_connection SET name = ?1, config = ?2, is_default = ?3 WHERE id = ?4")
            .bind(&name)
            .bind(serde_json::to_string(&config).unwrap_or_else(|_| "{}".to_string()))
            .bind(is_default)
            .bind(&input.connection_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update the intelligence connection: {}", e))?;
        Self::get(pool, &input.connection_id).await?.ok_or_else(|| "Connection not found".to_string())
    }

    pub async fn delete(pool: &Pool<Sqlite>, connection_id: &str) -> Result<(), String> {
        let current = Self::get(pool, connection_id).await?.ok_or_else(|| "Connection not found".to_string())?;
        sqlx::query("DELETE FROM intelligence_connection WHERE id = ?1")
            .bind(connection_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to delete the intelligence connection: {}", e))?;
        Self::seal_config(&current.profile_id, &current.id, current.config.keys().filter(|k| Self::is_secret_field(k)).map(|k| (k.clone(), String::new())).collect())?;
        if current.is_default {
            if let Some(next) = Self::all(pool, &current.profile_id).await?.into_iter().next() {
                sqlx::query("UPDATE intelligence_connection SET is_default = 1 WHERE id = ?1")
                    .bind(&next.id)
                    .execute(pool)
                    .await
                    .map_err(|e| format!("Failed to pick the next default connection: {}", e))?;
            }
        }
        Ok(())
    }

    async fn clear_default(pool: &Pool<Sqlite>, profile_id: &str) -> Result<(), String> {
        sqlx::query("UPDATE intelligence_connection SET is_default = 0 WHERE profile_id = ?1")
            .bind(profile_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to clear the default connection: {}", e))?;
        Ok(())
    }

    pub fn setting(&self, key: &str) -> Option<String> {
        let stored = self.config.get(key).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        crate::services::security_service::reveal(&crate::services::security_service::profile_vault(&self.profile_id), &format!("intelligence:{}:{}", self.id, key), stored)
    }
}

