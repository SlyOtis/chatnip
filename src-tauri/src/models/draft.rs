use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Draft {
    pub id: String,
    pub conversation_id: String,
    pub thread_id: String,
    pub doc: Value,
    pub markdown: String,
    pub metadata: Value,
    pub remote_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PutDraftInput {
    pub conversation_id: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub doc: Value,
    #[serde(default)]
    pub markdown: String,
    #[serde(default)]
    pub metadata: Value,
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS draft (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            thread_id TEXT NOT NULL DEFAULT '',
            doc TEXT NOT NULL DEFAULT 'null',
            markdown TEXT NOT NULL DEFAULT '',
            metadata TEXT NOT NULL DEFAULT 'null',
            remote_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE (profile_id, conversation_id, thread_id)
        )"#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

fn json(text: Option<String>) -> Value {
    text.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null)
}

impl Draft {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> Draft {
        Draft {
            id: row.get("id"),
            conversation_id: row.get("conversation_id"),
            thread_id: row.get("thread_id"),
            doc: json(row.try_get("doc").ok()),
            markdown: row.get("markdown"),
            metadata: json(row.try_get("metadata").ok()),
            remote_id: row.try_get("remote_id").ok().flatten(),
            updated_at: row.get("updated_at"),
        }
    }

    pub async fn get(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str, thread_id: &str) -> Result<Option<Draft>, String> {
        sqlx::query("SELECT * FROM draft WHERE profile_id = ?1 AND conversation_id = ?2 AND thread_id = ?3")
            .bind(profile_id)
            .bind(conversation_id)
            .bind(thread_id)
            .fetch_optional(pool)
            .await
            .map(|row| row.map(Draft::from_row))
            .map_err(|e| format!("Failed to read the draft: {}", e))
    }

    pub async fn get_all(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: Option<&str>) -> Result<Vec<Draft>, String> {
        sqlx::query("SELECT * FROM draft WHERE profile_id = ?1 AND (?2 IS NULL OR conversation_id = ?2) ORDER BY updated_at DESC LIMIT 500")
            .bind(profile_id)
            .bind(conversation_id)
            .fetch_all(pool)
            .await
            .map(|rows| rows.into_iter().map(Draft::from_row).collect())
            .map_err(|e| format!("Failed to list the drafts: {}", e))
    }

    pub async fn put(pool: &Pool<Sqlite>, profile_id: &str, input: &PutDraftInput) -> Result<Draft, String> {
        let now = Utc::now().to_rfc3339();
        let thread_id = input.thread_id.clone().unwrap_or_default();
        sqlx::query(
            "INSERT INTO draft (id, profile_id, conversation_id, thread_id, doc, markdown, metadata, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             ON CONFLICT(profile_id, conversation_id, thread_id) DO UPDATE SET doc = excluded.doc, markdown = excluded.markdown, metadata = excluded.metadata, updated_at = excluded.updated_at",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(profile_id)
        .bind(&input.conversation_id)
        .bind(&thread_id)
        .bind(input.doc.to_string())
        .bind(&input.markdown)
        .bind(input.metadata.to_string())
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to keep the draft: {}", e))?;
        Draft::get(pool, profile_id, &input.conversation_id, &thread_id).await?.ok_or_else(|| "The draft was not kept".to_string())
    }

    pub async fn put_remote_id(pool: &Pool<Sqlite>, id: &str, remote_id: Option<&str>) -> Result<(), String> {
        sqlx::query("UPDATE draft SET remote_id = ?1 WHERE id = ?2").bind(remote_id).bind(id).execute(pool).await.map(|_| ()).map_err(|e| format!("Failed to link the draft: {}", e))
    }

    pub async fn delete(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str, thread_id: &str) -> Result<Option<Draft>, String> {
        let found = Draft::get(pool, profile_id, conversation_id, thread_id).await?;
        sqlx::query("DELETE FROM draft WHERE profile_id = ?1 AND conversation_id = ?2 AND thread_id = ?3")
            .bind(profile_id)
            .bind(conversation_id)
            .bind(thread_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to drop the draft: {}", e))?;
        Ok(found)
    }
}
