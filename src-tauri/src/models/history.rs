use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

pub const KIND_SENT: &str = "sent";
pub const KIND_FAILED: &str = "failed";
pub const KIND_ERROR: &str = "error";
pub const KIND_WARNING: &str = "warning";
pub const KIND_INFO: &str = "info";

pub const STATUS_OPEN: &str = "open";
pub const STATUS_WAITING: &str = "waiting";
pub const STATUS_RESOLVED: &str = "resolved";
pub const STATUS_CANCELLED: &str = "cancelled";

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS history_entry (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            detail TEXT NOT NULL DEFAULT '',
            conversation_id TEXT,
            message_id TEXT,
            href TEXT,
            status TEXT NOT NULL DEFAULT 'open',
            options TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"#,
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS history_entry_profile_created ON history_entry (profile_id, created_at DESC)").execute(pool).await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    pub href: Option<String>,
    pub status: String,
    pub options: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PostHistoryInput {
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub href: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub options: Option<Value>,
}

impl HistoryEntry {
    fn read(row: &sqlx::sqlite::SqliteRow) -> Self {
        let options: String = row.get("options");
        Self {
            id: row.get("id"),
            kind: row.get("kind"),
            title: row.get("title"),
            detail: row.get("detail"),
            conversation_id: row.get("conversation_id"),
            message_id: row.get("message_id"),
            href: row.get("href"),
            status: row.get("status"),
            options: serde_json::from_str(&options).unwrap_or(Value::Object(Default::default())),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    pub async fn get(pool: &Pool<Sqlite>, profile_id: &str, limit: i64) -> Result<Vec<Self>, String> {
        let rows = sqlx::query("SELECT * FROM history_entry WHERE profile_id = ?1 ORDER BY created_at DESC LIMIT ?2")
            .bind(profile_id)
            .bind(limit)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read the history: {}", e))?;
        Ok(rows.iter().map(Self::read).collect())
    }

    pub async fn get_one(pool: &Pool<Sqlite>, id: &str) -> Result<Self, String> {
        let row = sqlx::query("SELECT * FROM history_entry WHERE id = ?1").bind(id).fetch_one(pool).await.map_err(|e| format!("No history entry {}: {}", id, e))?;
        Ok(Self::read(&row))
    }

    pub async fn get_waiting(pool: &Pool<Sqlite>) -> Result<Vec<Self>, String> {
        let rows = sqlx::query("SELECT * FROM history_entry WHERE status = ?1 ORDER BY created_at ASC")
            .bind(STATUS_WAITING)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read the waiting sends: {}", e))?;
        Ok(rows.iter().map(Self::read).collect())
    }

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, input: &PostHistoryInput) -> Result<Self, String> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let options = input.options.clone().unwrap_or(Value::Object(Default::default()));
        sqlx::query(
            "INSERT INTO history_entry (id, profile_id, kind, title, detail, conversation_id, message_id, href, status, options, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
        )
        .bind(&id)
        .bind(profile_id)
        .bind(&input.kind)
        .bind(&input.title)
        .bind(&input.detail)
        .bind(&input.conversation_id)
        .bind(&input.message_id)
        .bind(&input.href)
        .bind(input.status.as_deref().unwrap_or(STATUS_OPEN))
        .bind(options.to_string())
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to keep the history entry: {}", e))?;
        Self::get_one(pool, &id).await
    }

    pub async fn put(pool: &Pool<Sqlite>, id: &str, kind: Option<&str>, status: Option<&str>, detail: Option<&str>, message_id: Option<&str>) -> Result<Self, String> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE history_entry SET kind = COALESCE(?2, kind), status = COALESCE(?3, status), detail = COALESCE(?4, detail), message_id = COALESCE(?5, message_id), updated_at = ?6 WHERE id = ?1")
            .bind(id)
            .bind(kind)
            .bind(status)
            .bind(detail)
            .bind(message_id)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update the history entry: {}", e))?;
        Self::get_one(pool, id).await
    }
}
