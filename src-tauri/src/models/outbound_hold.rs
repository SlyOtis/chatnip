use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

pub const STATUS_PENDING: &str = "pending";
pub const STATUS_SENDING: &str = "sending";
pub const STATUS_SENT: &str = "sent";
pub const STATUS_DRAFT: &str = "draft";
pub const STATUS_CANCELLED: &str = "cancelled";
pub const STATUS_FAILED: &str = "failed";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutboundHold {
    pub id: String,
    pub surface: String,
    pub conversation_id: String,
    pub subject: String,
    pub recipient: String,
    pub payload: Value,
    pub draft: Value,
    pub due_at: String,
    pub status: String,
    pub error: Option<String>,
    pub message_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct PostOutboundHoldInput {
    pub surface: String,
    pub conversation_id: String,
    pub subject: String,
    pub recipient: String,
    pub payload: Value,
    pub draft: Value,
    pub due_at: DateTime<Utc>,
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS outbound_hold (
            id TEXT PRIMARY KEY,
            surface TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            subject TEXT NOT NULL DEFAULT '',
            recipient TEXT NOT NULL DEFAULT '',
            payload TEXT NOT NULL,
            draft TEXT NOT NULL DEFAULT 'null',
            due_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            error TEXT,
            message_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"#,
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_outbound_hold_status ON outbound_hold (status, due_at)").execute(pool).await?;
    Ok(())
}

fn json(text: Option<String>) -> Value {
    text.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null)
}

impl OutboundHold {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> OutboundHold {
        OutboundHold {
            id: row.get("id"),
            surface: row.get("surface"),
            conversation_id: row.get("conversation_id"),
            subject: row.get("subject"),
            recipient: row.get("recipient"),
            payload: json(row.try_get("payload").ok()),
            draft: json(row.try_get("draft").ok()),
            due_at: row.get("due_at"),
            status: row.get("status"),
            error: row.try_get("error").ok().flatten(),
            message_id: row.try_get("message_id").ok().flatten(),
            created_at: row.get("created_at"),
        }
    }

    pub fn due(&self) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(&self.due_at).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now())
    }

    pub fn seconds_left(&self, now: DateTime<Utc>) -> u64 {
        let millis = (self.due() - now).num_milliseconds();
        if millis <= 0 {
            0
        } else {
            ((millis + 999) / 1000) as u64
        }
    }

    pub async fn post(pool: &Pool<Sqlite>, input: &PostOutboundHoldInput) -> Result<OutboundHold, String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO outbound_hold (id, surface, conversation_id, subject, recipient, payload, draft, due_at, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', ?9, ?9)",
        )
        .bind(&id)
        .bind(&input.surface)
        .bind(&input.conversation_id)
        .bind(&input.subject)
        .bind(&input.recipient)
        .bind(input.payload.to_string())
        .bind(input.draft.to_string())
        .bind(input.due_at.to_rfc3339())
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to hold the email: {}", e))?;
        OutboundHold::get(pool, &id).await
    }

    pub async fn get(pool: &Pool<Sqlite>, id: &str) -> Result<OutboundHold, String> {
        sqlx::query("SELECT * FROM outbound_hold WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load the held email: {}", e))?
            .map(OutboundHold::from_row)
            .ok_or_else(|| "The held email is gone".to_string())
    }

    pub async fn get_holds(pool: &Pool<Sqlite>, statuses: &[&str], conversation_id: Option<&str>) -> Result<Vec<OutboundHold>, String> {
        let rows = sqlx::query("SELECT * FROM outbound_hold WHERE (?1 IS NULL OR conversation_id = ?1) ORDER BY due_at")
            .bind(conversation_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list held emails: {}", e))?;
        Ok(rows.into_iter().map(OutboundHold::from_row).filter(|h| statuses.contains(&h.status.as_str())).collect())
    }

    pub async fn get_due(pool: &Pool<Sqlite>, now: DateTime<Utc>) -> Result<Vec<OutboundHold>, String> {
        Ok(Self::get_holds(pool, &[STATUS_PENDING], None).await?.into_iter().filter(|h| h.due() <= now).collect())
    }

    pub async fn put_status(pool: &Pool<Sqlite>, id: &str, from: &[&str], to: &str, error: Option<&str>, message_id: Option<&str>) -> Result<bool, String> {
        let mut moved = false;
        for status in from {
            let done = sqlx::query("UPDATE outbound_hold SET status = ?1, error = ?2, message_id = COALESCE(?3, message_id), updated_at = ?4 WHERE id = ?5 AND status = ?6")
                .bind(to)
                .bind(error)
                .bind(message_id)
                .bind(Utc::now().to_rfc3339())
                .bind(id)
                .bind(status)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to update the held email: {}", e))?;
            if done.rows_affected() > 0 {
                moved = true;
                break;
            }
        }
        Ok(moved)
    }
}
