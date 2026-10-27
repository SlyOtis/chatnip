use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

use super::person::deserialize_double_option_string;
use super::rule::{Condition, Match};

pub const STATUS_OPEN: &str = "open";
pub const STATUS_WAITING: &str = "waiting";
pub const STATUS_DONE: &str = "done";
pub const EXPECT_PENDING: &str = "pending";
pub const EXPECT_ARRIVED: &str = "arrived";
pub const EXPECT_OVERDUE: &str = "overdue";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaseItem {
    pub id: String,
    pub case_id: String,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    pub note: Option<String>,
    pub title: String,
    pub conversation_type: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaseExpectation {
    pub id: String,
    pub case_id: String,
    pub title: String,
    pub kind: String,
    pub from_match: Option<String>,
    pub subject_match: Option<String>,
    pub filename_match: Option<String>,
    pub mime_match: Option<String>,
    pub due_at: Option<String>,
    pub status: String,
    pub arrived_message_id: Option<String>,
    pub arrived_attachment_name: Option<String>,
    pub arrived_at: Option<String>,
    pub arrived_from: Option<String>,
    pub arrived_conversation_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaseRule {
    pub id: String,
    pub case_id: String,
    pub r#match: Match,
    pub conditions: Vec<Condition>,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseFile {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub color: String,
    pub due_at: Option<String>,
    pub items: Vec<CaseItem>,
    pub expectations: Vec<CaseExpectation>,
    pub rules: Vec<CaseRule>,
    pub conversation_count: i64,
    pub message_count: i64,
    pub pending_count: i64,
    pub arrived_count: i64,
    pub overdue_count: i64,
    pub last_activity: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostCaseInput {
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub due_at: Option<String>,
    #[serde(default)]
    pub conversation_ids: Vec<String>,
    #[serde(default)]
    pub message_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct PutCaseInput {
    pub case_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub color: Option<String>,
    #[serde(default, deserialize_with = "deserialize_double_option_string")]
    pub due_at: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct CaseItemsInput {
    pub case_id: String,
    #[serde(default)]
    pub conversation_ids: Vec<String>,
    #[serde(default)]
    pub message_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct PostExpectationInput {
    pub case_id: String,
    pub title: String,
    pub kind: Option<String>,
    pub from_match: Option<String>,
    pub subject_match: Option<String>,
    pub filename_match: Option<String>,
    pub mime_match: Option<String>,
    pub due_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct PutExpectationInput {
    pub expectation_id: String,
    pub title: Option<String>,
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "deserialize_double_option_string")]
    pub from_match: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option_string")]
    pub subject_match: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option_string")]
    pub filename_match: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option_string")]
    pub mime_match: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option_string")]
    pub due_at: Option<Option<String>>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostCaseRuleInput {
    pub case_id: String,
    pub r#match: Option<Match>,
    pub conditions: Vec<Condition>,
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS case_file (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'open',
            color TEXT NOT NULL DEFAULT '#5a57d6',
            due_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS case_item (
            id TEXT PRIMARY KEY,
            case_id TEXT NOT NULL,
            conversation_id TEXT,
            message_id TEXT,
            note TEXT,
            added_at TEXT NOT NULL,
            FOREIGN KEY (case_id) REFERENCES case_file (id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS case_item_unique ON case_item (case_id, COALESCE(conversation_id, ''), COALESCE(message_id, ''))")
        .execute(pool)
        .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS case_expectation (
            id TEXT PRIMARY KEY,
            case_id TEXT NOT NULL,
            title TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'file',
            from_match TEXT,
            subject_match TEXT,
            filename_match TEXT,
            mime_match TEXT,
            due_at TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            arrived_message_id TEXT,
            arrived_attachment_name TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (case_id) REFERENCES case_file (id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS case_rule (
            id TEXT PRIMARY KEY,
            case_id TEXT NOT NULL,
            match_mode TEXT NOT NULL DEFAULT 'all',
            conditions TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            FOREIGN KEY (case_id) REFERENCES case_file (id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

fn err(context: &str) -> impl Fn(sqlx::Error) -> String + '_ {
    move |e| format!("{}: {}", context, e)
}

impl CaseExpectation {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> CaseExpectation {
        CaseExpectation {
            id: row.get("id"),
            case_id: row.get("case_id"),
            title: row.get("title"),
            kind: row.get("kind"),
            from_match: row.try_get("from_match").ok().flatten(),
            subject_match: row.try_get("subject_match").ok().flatten(),
            filename_match: row.try_get("filename_match").ok().flatten(),
            mime_match: row.try_get("mime_match").ok().flatten(),
            due_at: row.try_get("due_at").ok().flatten(),
            status: row.get("status"),
            arrived_message_id: row.try_get("arrived_message_id").ok().flatten(),
            arrived_attachment_name: row.try_get("arrived_attachment_name").ok().flatten(),
            arrived_at: row.try_get("arrived_at").ok().flatten(),
            arrived_from: row.try_get("arrived_from").ok().flatten(),
            arrived_conversation_id: row.try_get("arrived_conversation_id").ok().flatten(),
            created_at: row.get("created_at"),
        }
    }

    pub fn effective_status(&self, now: &str) -> String {
        if self.status == EXPECT_PENDING {
            if let Some(due) = &self.due_at {
                if due.as_str() < now {
                    return EXPECT_OVERDUE.to_string();
                }
            }
        }
        self.status.clone()
    }

    pub async fn for_case(pool: &Pool<Sqlite>, case_id: &str) -> Result<Vec<CaseExpectation>, String> {
        let rows = sqlx::query(
            "SELECT e.*, m.timestamp AS arrived_at, COALESCE(m.sender_name, m.sender_email) AS arrived_from, m.conversation_id AS arrived_conversation_id
             FROM case_expectation e LEFT JOIN message m ON m.id = e.arrived_message_id
             WHERE e.case_id = ?1 ORDER BY e.created_at",
        )
        .bind(case_id)
        .fetch_all(pool)
        .await
        .map_err(err("Failed to load expectations"))?;
        let now = Utc::now().to_rfc3339();
        Ok(rows
            .iter()
            .map(|r| {
                let mut e = CaseExpectation::from_row(r);
                e.status = e.effective_status(&now);
                e
            })
            .collect())
    }

    pub async fn pending(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<CaseExpectation>, String> {
        let rows = sqlx::query(
            "SELECT e.* FROM case_expectation e JOIN case_file c ON c.id = e.case_id
             WHERE c.profile_id = ?1 AND e.status = 'pending' ORDER BY e.created_at",
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(err("Failed to load pending expectations"))?;
        Ok(rows.iter().map(CaseExpectation::from_row).collect())
    }

    pub async fn post(pool: &Pool<Sqlite>, input: &PostExpectationInput) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO case_expectation (id, case_id, title, kind, from_match, subject_match, filename_match, mime_match, due_at, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending', ?10)",
        )
        .bind(&id)
        .bind(&input.case_id)
        .bind(input.title.trim())
        .bind(input.kind.clone().unwrap_or_else(|| "file".to_string()))
        .bind(blank(&input.from_match))
        .bind(blank(&input.subject_match))
        .bind(blank(&input.filename_match))
        .bind(blank(&input.mime_match))
        .bind(blank(&input.due_at))
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .map_err(err("Failed to save the expectation"))?;
        touch(pool, &input.case_id).await?;
        Ok(id)
    }

    pub async fn put(pool: &Pool<Sqlite>, input: &PutExpectationInput) -> Result<String, String> {
        let row = sqlx::query("SELECT * FROM case_expectation WHERE id = ?1")
            .bind(&input.expectation_id)
            .fetch_optional(pool)
            .await
            .map_err(err("Failed to load the expectation"))?
            .ok_or_else(|| "Expectation not found".to_string())?;
        let mut e = CaseExpectation::from_row(&row);
        if let Some(v) = &input.title {
            e.title = v.trim().to_string();
        }
        if let Some(v) = &input.kind {
            e.kind = v.clone();
        }
        if let Some(v) = &input.from_match {
            e.from_match = blank(v);
        }
        if let Some(v) = &input.subject_match {
            e.subject_match = blank(v);
        }
        if let Some(v) = &input.filename_match {
            e.filename_match = blank(v);
        }
        if let Some(v) = &input.mime_match {
            e.mime_match = blank(v);
        }
        if let Some(v) = &input.due_at {
            e.due_at = blank(v);
        }
        if let Some(v) = &input.status {
            e.status = v.clone();
            if v == EXPECT_PENDING {
                e.arrived_message_id = None;
                e.arrived_attachment_name = None;
            }
        }
        sqlx::query(
            "UPDATE case_expectation SET title = ?1, kind = ?2, from_match = ?3, subject_match = ?4, filename_match = ?5, mime_match = ?6,
             due_at = ?7, status = ?8, arrived_message_id = ?9, arrived_attachment_name = ?10 WHERE id = ?11",
        )
        .bind(&e.title)
        .bind(&e.kind)
        .bind(&e.from_match)
        .bind(&e.subject_match)
        .bind(&e.filename_match)
        .bind(&e.mime_match)
        .bind(&e.due_at)
        .bind(&e.status)
        .bind(&e.arrived_message_id)
        .bind(&e.arrived_attachment_name)
        .bind(&e.id)
        .execute(pool)
        .await
        .map_err(err("Failed to update the expectation"))?;
        touch(pool, &e.case_id).await?;
        Ok(e.case_id)
    }

    pub async fn mark_arrived(pool: &Pool<Sqlite>, id: &str, message_id: &str, attachment: Option<&str>) -> Result<(), String> {
        sqlx::query("UPDATE case_expectation SET status = 'arrived', arrived_message_id = ?1, arrived_attachment_name = ?2 WHERE id = ?3 AND status = 'pending'")
            .bind(message_id)
            .bind(attachment)
            .bind(id)
            .execute(pool)
            .await
            .map_err(err("Failed to mark the expectation"))?;
        Ok(())
    }

    pub async fn delete(pool: &Pool<Sqlite>, id: &str) -> Result<Option<String>, String> {
        let case_id: Option<String> = sqlx::query("SELECT case_id FROM case_expectation WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(err("Failed to load the expectation"))?
            .map(|r| r.get("case_id"));
        sqlx::query("DELETE FROM case_expectation WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await
            .map_err(err("Failed to delete the expectation"))?;
        Ok(case_id)
    }
}

impl CaseRule {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> CaseRule {
        let conditions: String = row.get("conditions");
        let mode: String = row.get("match_mode");
        CaseRule {
            id: row.get("id"),
            case_id: row.get("case_id"),
            r#match: if mode == "any" { Match::Any } else { Match::All },
            conditions: serde_json::from_str(&conditions).unwrap_or_default(),
            enabled: row.get("enabled"),
            created_at: row.get("created_at"),
        }
    }

    pub fn matches(&self, subject: &super::rule::Subject) -> bool {
        if !self.enabled || self.conditions.is_empty() {
            return false;
        }
        match self.r#match {
            Match::All => self.conditions.iter().all(|c| c.holds(subject)),
            Match::Any => self.conditions.iter().any(|c| c.holds(subject)),
        }
    }

    pub async fn for_case(pool: &Pool<Sqlite>, case_id: &str) -> Result<Vec<CaseRule>, String> {
        let rows = sqlx::query("SELECT * FROM case_rule WHERE case_id = ?1 ORDER BY created_at")
            .bind(case_id)
            .fetch_all(pool)
            .await
            .map_err(err("Failed to load case rules"))?;
        Ok(rows.iter().map(CaseRule::from_row).collect())
    }

    pub async fn for_profile(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<CaseRule>, String> {
        let rows = sqlx::query(
            "SELECT r.* FROM case_rule r JOIN case_file c ON c.id = r.case_id WHERE c.profile_id = ?1 AND r.enabled = 1 AND c.status != 'done'",
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(err("Failed to load case rules"))?;
        Ok(rows.iter().map(CaseRule::from_row).collect())
    }

    pub async fn post(pool: &Pool<Sqlite>, input: &PostCaseRuleInput) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        let mode = match input.r#match.clone().unwrap_or_default() {
            Match::Any => "any",
            Match::All => "all",
        };
        sqlx::query("INSERT INTO case_rule (id, case_id, match_mode, conditions, enabled, created_at) VALUES (?1, ?2, ?3, ?4, 1, ?5)")
            .bind(&id)
            .bind(&input.case_id)
            .bind(mode)
            .bind(serde_json::to_string(&input.conditions).unwrap_or_else(|_| "[]".to_string()))
            .bind(Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(err("Failed to save the case rule"))?;
        Ok(id)
    }

    pub async fn delete(pool: &Pool<Sqlite>, id: &str) -> Result<Option<String>, String> {
        let case_id: Option<String> = sqlx::query("SELECT case_id FROM case_rule WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(err("Failed to load the case rule"))?
            .map(|r| r.get("case_id"));
        sqlx::query("DELETE FROM case_rule WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await
            .map_err(err("Failed to delete the case rule"))?;
        Ok(case_id)
    }
}

impl CaseItem {
    pub async fn for_case(pool: &Pool<Sqlite>, case_id: &str) -> Result<Vec<CaseItem>, String> {
        let rows = sqlx::query(
            "SELECT i.*, COALESCE(c.name, mc.name, '') AS conversation_name, COALESCE(c.type, mc.type) AS conversation_type,
                    m.subject AS message_subject, m.content AS message_content, m.content_type AS message_content_type
             FROM case_item i
             LEFT JOIN conversation c ON c.id = i.conversation_id
             LEFT JOIN message m ON m.id = i.message_id
             LEFT JOIN conversation mc ON mc.id = m.conversation_id
             WHERE i.case_id = ?1 ORDER BY i.added_at",
        )
        .bind(case_id)
        .fetch_all(pool)
        .await
        .map_err(err("Failed to load case items"))?;
        Ok(rows
            .iter()
            .map(|r| {
                let message_id: Option<String> = r.try_get("message_id").ok().flatten();
                let conversation_name: String = r.try_get("conversation_name").unwrap_or_default();
                let title = if message_id.is_some() {
                    let subject: Option<String> = r.try_get("message_subject").ok().flatten();
                    let content: Option<String> = r.try_get("message_content").ok().flatten();
                    let kind: Option<String> = r.try_get("message_content_type").ok().flatten();
                    let text = crate::utils::text_extraction::extract_text_from_content(content.as_deref().unwrap_or(""), kind.as_deref().unwrap_or("text"));
                    let line: String = subject.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| text.chars().take(80).collect());
                    if conversation_name.is_empty() { line } else { format!("{} · {}", line, conversation_name) }
                } else {
                    conversation_name
                };
                CaseItem {
                    id: r.get("id"),
                    case_id: r.get("case_id"),
                    conversation_id: r.try_get("conversation_id").ok().flatten(),
                    message_id,
                    note: r.try_get("note").ok().flatten(),
                    title,
                    conversation_type: r.try_get("conversation_type").ok().flatten(),
                    added_at: r.get("added_at"),
                }
            })
            .collect())
    }

    pub async fn add(pool: &Pool<Sqlite>, case_id: &str, conversation_id: Option<&str>, message_id: Option<&str>) -> Result<bool, String> {
        let done = sqlx::query("INSERT OR IGNORE INTO case_item (id, case_id, conversation_id, message_id, added_at) VALUES (?1, ?2, ?3, ?4, ?5)")
            .bind(Uuid::new_v4().to_string())
            .bind(case_id)
            .bind(conversation_id)
            .bind(message_id)
            .bind(Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(err("Failed to add to the case"))?;
        Ok(done.rows_affected() > 0)
    }

    pub async fn remove(pool: &Pool<Sqlite>, case_id: &str, conversation_id: Option<&str>, message_id: Option<&str>) -> Result<(), String> {
        sqlx::query("DELETE FROM case_item WHERE case_id = ?1 AND COALESCE(conversation_id, '') = COALESCE(?2, '') AND COALESCE(message_id, '') = COALESCE(?3, '')")
            .bind(case_id)
            .bind(conversation_id)
            .bind(message_id)
            .execute(pool)
            .await
            .map_err(err("Failed to remove from the case"))?;
        Ok(())
    }
}

fn blank(value: &Option<String>) -> Option<String> {
    value.as_ref().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

pub async fn touch(pool: &Pool<Sqlite>, case_id: &str) -> Result<(), String> {
    sqlx::query("UPDATE case_file SET updated_at = ?1 WHERE id = ?2")
        .bind(Utc::now().to_rfc3339())
        .bind(case_id)
        .execute(pool)
        .await
        .map_err(err("Failed to update the case"))?;
    Ok(())
}

impl CaseFile {
    async fn hydrate(pool: &Pool<Sqlite>, row: &sqlx::sqlite::SqliteRow) -> Result<CaseFile, String> {
        let id: String = row.get("id");
        let items = CaseItem::for_case(pool, &id).await?;
        let expectations = CaseExpectation::for_case(pool, &id).await?;
        let rules = CaseRule::for_case(pool, &id).await?;
        let last_activity: Option<String> = sqlx::query(
            "SELECT MAX(m.timestamp) AS last FROM message m
             WHERE m.deleted_datetime IS NULL AND (m.conversation_id IN (SELECT conversation_id FROM case_item WHERE case_id = ?1 AND conversation_id IS NOT NULL)
                OR m.id IN (SELECT message_id FROM case_item WHERE case_id = ?1 AND message_id IS NOT NULL))",
        )
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(err("Failed to read case activity"))?
        .try_get("last")
        .ok()
        .flatten();
        Ok(CaseFile {
            profile_id: row.get("profile_id"),
            name: row.get("name"),
            description: row.get("description"),
            status: row.get("status"),
            color: row.get("color"),
            due_at: row.try_get("due_at").ok().flatten(),
            conversation_count: items.iter().filter(|i| i.conversation_id.is_some()).count() as i64,
            message_count: items.iter().filter(|i| i.message_id.is_some()).count() as i64,
            pending_count: expectations.iter().filter(|e| e.status == EXPECT_PENDING).count() as i64,
            arrived_count: expectations.iter().filter(|e| e.status == EXPECT_ARRIVED).count() as i64,
            overdue_count: expectations.iter().filter(|e| e.status == EXPECT_OVERDUE).count() as i64,
            last_activity,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            items,
            expectations,
            rules,
            id,
        })
    }

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<CaseFile>, String> {
        let rows = sqlx::query("SELECT * FROM case_file WHERE profile_id = ?1 ORDER BY CASE status WHEN 'done' THEN 1 ELSE 0 END, updated_at DESC")
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(err("Failed to load cases"))?;
        let mut cases = Vec::new();
        for row in &rows {
            cases.push(CaseFile::hydrate(pool, row).await?);
        }
        Ok(cases)
    }

    pub async fn get(pool: &Pool<Sqlite>, case_id: &str) -> Result<CaseFile, String> {
        let row = sqlx::query("SELECT * FROM case_file WHERE id = ?1")
            .bind(case_id)
            .fetch_optional(pool)
            .await
            .map_err(err("Failed to load the case"))?
            .ok_or_else(|| "Case not found".to_string())?;
        CaseFile::hydrate(pool, &row).await
    }

    pub async fn for_conversation(pool: &Pool<Sqlite>, conversation_id: &str) -> Result<Vec<String>, String> {
        let rows = sqlx::query(
            "SELECT DISTINCT i.case_id FROM case_item i LEFT JOIN message m ON m.id = i.message_id
             WHERE i.conversation_id = ?1 OR m.conversation_id = ?1",
        )
        .bind(conversation_id)
        .fetch_all(pool)
        .await
        .map_err(err("Failed to load the conversation's cases"))?;
        Ok(rows.into_iter().map(|r| r.get("case_id")).collect())
    }

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, input: &PostCaseInput) -> Result<String, String> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err("A case needs a name".to_string());
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO case_file (id, profile_id, name, description, status, color, due_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'open', ?5, ?6, ?7, ?7)")
            .bind(&id)
            .bind(profile_id)
            .bind(name)
            .bind(input.description.clone().unwrap_or_default())
            .bind(input.color.clone().unwrap_or_else(|| "#5a57d6".to_string()))
            .bind(blank(&input.due_at))
            .bind(&now)
            .execute(pool)
            .await
            .map_err(err("Failed to create the case"))?;
        for c in &input.conversation_ids {
            CaseItem::add(pool, &id, Some(c), None).await?;
        }
        for m in &input.message_ids {
            CaseItem::add(pool, &id, None, Some(m)).await?;
        }
        Ok(id)
    }

    pub async fn put(pool: &Pool<Sqlite>, input: &PutCaseInput) -> Result<(), String> {
        let current = CaseFile::get(pool, &input.case_id).await?;
        let status = input.status.clone().unwrap_or(current.status);
        if ![STATUS_OPEN, STATUS_WAITING, STATUS_DONE].contains(&status.as_str()) {
            return Err(format!("Unknown case status {}", status));
        }
        let due_at = match &input.due_at {
            Some(v) => blank(v),
            None => current.due_at,
        };
        sqlx::query("UPDATE case_file SET name = ?1, description = ?2, status = ?3, color = ?4, due_at = ?5, updated_at = ?6 WHERE id = ?7")
            .bind(input.name.as_deref().map(str::trim).filter(|v| !v.is_empty()).unwrap_or(&current.name))
            .bind(input.description.clone().unwrap_or(current.description))
            .bind(status)
            .bind(input.color.clone().unwrap_or(current.color))
            .bind(due_at)
            .bind(Utc::now().to_rfc3339())
            .bind(&input.case_id)
            .execute(pool)
            .await
            .map_err(err("Failed to update the case"))?;
        Ok(())
    }

    pub async fn delete(pool: &Pool<Sqlite>, case_id: &str) -> Result<(), String> {
        for table in ["case_item", "case_expectation", "case_rule"] {
            sqlx::query(&format!("DELETE FROM {} WHERE case_id = ?1", table))
                .bind(case_id)
                .execute(pool)
                .await
                .map_err(err("Failed to delete the case"))?;
        }
        sqlx::query("DELETE FROM case_file WHERE id = ?1")
            .bind(case_id)
            .execute(pool)
            .await
            .map_err(err("Failed to delete the case"))?;
        Ok(())
    }
}
