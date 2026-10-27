use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

pub const STATUS_PENDING: &str = "pending";
pub const STATUS_FIRING: &str = "firing";
pub const STATUS_DONE: &str = "done";
pub const STATUS_CANCELLED: &str = "cancelled";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reminder {
    pub id: String,
    pub message_id: Option<String>,
    pub conversation_id: Option<String>,
    pub conversation_name: Option<String>,
    pub note: String,
    pub at: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PostReminderInput {
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    pub at: String,
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS reminder (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            message_id TEXT,
            conversation_id TEXT,
            note TEXT NOT NULL DEFAULT '',
            at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"#,
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_reminder_due ON reminder (status, at)").execute(pool).await?;
    Ok(())
}

impl Reminder {
    const SELECT: &'static str = "SELECT r.*, c.name AS conversation_name FROM reminder r LEFT JOIN conversation c ON c.id = COALESCE(r.conversation_id, (SELECT conversation_id FROM message WHERE id = r.message_id))";

    fn from_row(row: sqlx::sqlite::SqliteRow) -> Reminder {
        Reminder {
            id: row.get("id"),
            message_id: row.try_get("message_id").ok().flatten(),
            conversation_id: row.try_get("conversation_id").ok().flatten(),
            conversation_name: row.try_get("conversation_name").ok().flatten(),
            note: row.get("note"),
            at: row.get("at"),
            status: row.get("status"),
            created_at: row.get("created_at"),
        }
    }

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, input: &PostReminderInput) -> Result<Reminder, String> {
        let at = chrono::DateTime::parse_from_rfc3339(&input.at).map_err(|_| format!("{} is not an RFC 3339 time", input.at))?.with_timezone(&Utc).to_rfc3339();
        if input.message_id.is_none() && input.conversation_id.is_none() && input.note.as_deref().is_none_or(|n| n.trim().is_empty()) {
            return Err("A reminder needs a message, a conversation or a note".to_string());
        }
        let conversation_id = match (&input.conversation_id, &input.message_id) {
            (Some(id), _) => Some(id.clone()),
            (None, Some(message)) => crate::models::Message::get_conversation_id(pool, message).await?,
            (None, None) => None,
        };
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO reminder (id, profile_id, message_id, conversation_id, note, at, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7, ?7)")
            .bind(&id)
            .bind(profile_id)
            .bind(&input.message_id)
            .bind(&conversation_id)
            .bind(input.note.clone().unwrap_or_default().trim())
            .bind(&at)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to set the reminder: {}", e))?;
        Reminder::get(pool, &id).await
    }

    pub async fn get(pool: &Pool<Sqlite>, id: &str) -> Result<Reminder, String> {
        sqlx::query(&format!("{} WHERE r.id = ?1", Self::SELECT))
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to read the reminder: {}", e))?
            .map(Reminder::from_row)
            .ok_or_else(|| "Reminder not found".to_string())
    }

    pub async fn get_all(pool: &Pool<Sqlite>, profile_id: &str, pending: bool) -> Result<Vec<Reminder>, String> {
        sqlx::query(&format!("{} WHERE r.profile_id = ?1 AND (?2 = 0 OR r.status = 'pending') ORDER BY r.at LIMIT 500", Self::SELECT))
            .bind(profile_id)
            .bind(pending)
            .fetch_all(pool)
            .await
            .map(|rows| rows.into_iter().map(Reminder::from_row).collect())
            .map_err(|e| format!("Failed to list the reminders: {}", e))
    }

    pub async fn get_due(pool: &Pool<Sqlite>, profile_id: &str, now: &str) -> Result<Vec<Reminder>, String> {
        sqlx::query(&format!("{} WHERE r.profile_id = ?1 AND r.status = 'pending' AND r.at <= ?2 ORDER BY r.at", Self::SELECT))
            .bind(profile_id)
            .bind(now)
            .fetch_all(pool)
            .await
            .map(|rows| rows.into_iter().map(Reminder::from_row).collect())
            .map_err(|e| format!("Failed to list the due reminders: {}", e))
    }

    pub async fn put_status(pool: &Pool<Sqlite>, id: &str, from: &str, to: &str) -> Result<bool, String> {
        sqlx::query("UPDATE reminder SET status = ?1, updated_at = ?2 WHERE id = ?3 AND status = ?4")
            .bind(to)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .bind(from)
            .execute(pool)
            .await
            .map(|r| r.rows_affected() > 0)
            .map_err(|e| format!("Failed to update the reminder: {}", e))
    }
}
