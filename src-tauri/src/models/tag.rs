use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::collections::HashMap;
use uuid::Uuid;

pub const KIND_SPLIT: &str = "split";
pub const KIND_TAG: &str = "tag";

pub const OBJECT_CONVERSATION: &str = "conversation";
pub const OBJECT_MESSAGE: &str = "message";
pub const OBJECT_PERSON: &str = "person";
pub const OBJECT_IDENTITY: &str = "identity";
pub const OBJECT_CONTENT: &str = "content";
pub const OBJECT_ACTION: &str = "action";
pub const OBJECT_ACTION_GROUP: &str = "action_group";
pub const OBJECT_ATTACHMENT: &str = "attachment";
pub const OBJECT_EVENT: &str = "event";
pub const OBJECT_GROUP: &str = "group";
pub const OBJECT_INBOX: &str = "inbox";
pub const OBJECT_MERGE_GROUP: &str = "merge_group";
pub const OBJECT_TYPES: [&str; 12] = [
    OBJECT_CONVERSATION,
    OBJECT_MESSAGE,
    OBJECT_PERSON,
    OBJECT_IDENTITY,
    OBJECT_CONTENT,
    OBJECT_ACTION,
    OBJECT_ACTION_GROUP,
    OBJECT_ATTACHMENT,
    OBJECT_EVENT,
    OBJECT_GROUP,
    OBJECT_INBOX,
    OBJECT_MERGE_GROUP,
];

pub const RETIRED_CONVERSATION_TAGS: &str = "conversation_tag_retired";
pub const RETIRED_PERSON_TAGS: &str = "person_tag_retired";
pub const RETIRED_CONTENT_TAGS: &str = "tags_retired";

pub const DEFAULT_SPLITS: [(&str, &str); 5] = [
    ("Important", "#f5b84b"),
    ("Team", "#5aa9e6"),
    ("Newsletters", "#8bc38a"),
    ("Notifications", "#9a8fd6"),
    ("Other", "#8c93a3"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub color: String,
    pub kind: String,
    pub position: i64,
    pub description: String,
    pub icon: Option<String>,
    pub favourite: bool,
    pub favourite_position: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObjectTag {
    pub tag_id: String,
    pub object_type: String,
    pub object_id: String,
    pub source: String,
    pub rule_id: Option<String>,
    pub reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ObjectTagInput {
    pub object_type: String,
    pub object_id: String,
    pub tag_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TaggedObject {
    pub tag_id: String,
    pub object_type: String,
    pub object_id: String,
    pub title: String,
    pub detail: Option<String>,
    pub conversation_id: Option<String>,
    pub conversation_name: Option<String>,
    pub message_id: Option<String>,
    pub person_id: Option<String>,
    pub author: Option<String>,
    pub timestamp: Option<String>,
    pub excerpt: Option<String>,
    pub created_at: String,
}

impl Tag {
    pub fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        Self {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            name: row.get("name"),
            color: row.get("color"),
            kind: row.get("kind"),
            position: row.get("position"),
            description: row.try_get("description").unwrap_or_default(),
            icon: row.try_get::<Option<String>, _>("icon").ok().flatten().filter(|icon| !icon.is_empty()),
            favourite: row.try_get::<i64, _>("favourite").map(|v| v != 0).unwrap_or(false),
            favourite_position: row.try_get::<Option<i64>, _>("favourite_position").ok().flatten(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Tag>, String> {
        let rows = sqlx::query("SELECT * FROM tag WHERE profile_id = ?1 ORDER BY kind DESC, position ASC, name ASC")
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list tags: {}", e))?;
        Ok(rows.into_iter().map(Tag::from_row).collect())
    }

    pub async fn get(pool: &Pool<Sqlite>, id: &str) -> Result<Tag, String> {
        let row = sqlx::query("SELECT * FROM tag WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load tag: {}", e))?
            .ok_or_else(|| "Tag not found".to_string())?;
        Ok(Tag::from_row(row))
    }

    pub async fn ensure_defaults(pool: &Pool<Sqlite>, profile_id: &str) -> Result<(), String> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tag WHERE profile_id = ?1 AND kind = ?2")
            .bind(profile_id)
            .bind(KIND_SPLIT)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to count splits: {}", e))?;
        if count > 0 {
            return Ok(());
        }
        for (position, (name, color)) in DEFAULT_SPLITS.iter().enumerate() {
            let description = match *name {
                "Important" => "Mail and messages that need me: people writing to me directly, questions, decisions, anything from my teams and clients that is not noise.",
                "Team" => "Channel and group chatter from my teams that I follow but that does not need a reply from me.",
                "Newsletters" => "Digests, newsletters, marketing and announcements sent to a list.",
                "Notifications" => "Automated notifications from tools and services: build results, tickets, calendar invites, receipts, security alerts.",
                _ => "Everything that fits nowhere else.",
            };
            Tag::post(pool, profile_id, name, color, KIND_SPLIT, position as i64, description).await?;
        }
        Ok(())
    }

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, name: &str, color: &str, kind: &str, position: i64, description: &str) -> Result<Tag, String> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO tag (id, profile_id, name, color, kind, position, description, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)")
            .bind(&id)
            .bind(profile_id)
            .bind(name.trim())
            .bind(color)
            .bind(kind)
            .bind(position)
            .bind(description)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to create tag: {}", e))?;
        Tag::get(pool, &id).await
    }

    pub async fn put(pool: &Pool<Sqlite>, id: &str, name: Option<&str>, color: Option<&str>, position: Option<i64>, description: Option<&str>, icon: Option<&str>) -> Result<Tag, String> {
        let current = Tag::get(pool, id).await?;
        sqlx::query("UPDATE tag SET name = ?1, color = ?2, position = ?3, description = ?4, icon = ?7, updated_at = ?5 WHERE id = ?6")
            .bind(name.map(str::trim).unwrap_or(&current.name))
            .bind(color.unwrap_or(&current.color))
            .bind(position.unwrap_or(current.position))
            .bind(description.unwrap_or(&current.description))
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .bind(match icon { Some(icon) => Some(icon.trim()).filter(|icon| !icon.is_empty()).map(str::to_string), None => current.icon.clone() })
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update tag: {}", e))?;
        Tag::get(pool, id).await
    }

    pub async fn put_favourite(pool: &Pool<Sqlite>, id: &str, favourite: bool) -> Result<Tag, String> {
        let next: Option<i64> = if favourite {
            let label = Tag::get(pool, id).await?;
            let max: Option<i64> = sqlx::query_scalar("SELECT MAX(favourite_position) FROM tag WHERE profile_id = ?1 AND favourite = 1")
                .bind(&label.profile_id)
                .fetch_one(pool)
                .await
                .map_err(|e| format!("Failed to read favourites: {}", e))?;
            Some(label.favourite_position.unwrap_or(max.map(|m| m + 1).unwrap_or(0)))
        } else {
            None
        };
        sqlx::query("UPDATE tag SET favourite = ?1, favourite_position = ?2, updated_at = ?3 WHERE id = ?4")
            .bind(favourite as i64)
            .bind(next)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update favourite: {}", e))?;
        Tag::get(pool, id).await
    }

    pub async fn put_favourite_order(pool: &Pool<Sqlite>, profile_id: &str, order: &[String]) -> Result<(), String> {
        for (index, id) in order.iter().enumerate() {
            sqlx::query("UPDATE tag SET favourite = 1, favourite_position = ?1 WHERE id = ?2 AND profile_id = ?3")
                .bind(index as i64)
                .bind(id)
                .bind(profile_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to order favourites: {}", e))?;
        }
        Ok(())
    }

    async fn favourite_tag(pool: &Pool<Sqlite>, profile_id: &str, name: &str, icon: &str, color: &str) -> Result<Tag, String> {
        let existing: Option<String> = sqlx::query_scalar("SELECT id FROM tag WHERE profile_id = ?1 AND name = ?2")
            .bind(profile_id)
            .bind(name)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to look up tag {}: {}", name, e))?;
        let label = match existing {
            Some(id) => Tag::get(pool, &id).await?,
            None => {
                let created = Tag::post(pool, profile_id, name, color, KIND_TAG, 0, "").await?;
                Tag::put(pool, &created.id, None, None, None, None, Some(icon)).await?
            }
        };
        if label.favourite {
            Ok(label)
        } else {
            Tag::put_favourite(pool, &label.id, true).await
        }
    }

    pub async fn migrate_favourites(pool: &Pool<Sqlite>, profile_id: &str) -> Result<(), String> {
        let owned = "SELECT DISTINCT c.id FROM conversation c JOIN conversation_provider cp ON cp.conversation_id = c.id JOIN provider p ON p.id = cp.provider_id WHERE p.profile_id = ?1";
        let starred: Vec<String> = sqlx::query_scalar(&format!("{owned} AND c.is_pinned = 1"))
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read starred conversations: {}", e))?;
        if !starred.is_empty() {
            let label = Tag::favourite_tag(pool, profile_id, "Starred", "lucide:star", "#d9a400").await?;
            for id in &starred {
                ObjectTag::put(pool, OBJECT_CONVERSATION, id, &label.id, "manual", None, None).await?;
            }
            sqlx::query(&format!("UPDATE conversation SET is_pinned = 0 WHERE id IN ({owned} AND c.is_pinned = 1)"))
                .bind(profile_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to clear stars: {}", e))?;
        }
        let favourites: Vec<String> = sqlx::query_scalar(&format!(
            "{owned} AND c.favourite_position IS NOT NULL UNION SELECT c.id FROM conversation c JOIN person pe ON pe.id = c.person_id WHERE pe.profile_id = ?1 AND pe.favourite_position IS NOT NULL"
        ))
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to read favourites: {}", e))?;
        if !favourites.is_empty() {
            let label = Tag::favourite_tag(pool, profile_id, "Favourites", "lucide:heart", "#d0587e").await?;
            for id in &favourites {
                ObjectTag::put(pool, OBJECT_CONVERSATION, id, &label.id, "manual", None, None).await?;
            }
            sqlx::query(&format!("UPDATE conversation SET favourite_position = NULL WHERE id IN ({owned} AND c.favourite_position IS NOT NULL)"))
                .bind(profile_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to clear conversation favourites: {}", e))?;
            sqlx::query("UPDATE person SET favourite_position = NULL WHERE profile_id = ?1")
                .bind(profile_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to clear person favourites: {}", e))?;
        }
        Ok(())
    }

    pub async fn delete(pool: &Pool<Sqlite>, id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM object_tag WHERE tag_id = ?1").bind(id).execute(pool).await.map_err(|e| format!("Failed to clear tag: {}", e))?;
        sqlx::query("DELETE FROM tag WHERE id = ?1").bind(id).execute(pool).await.map_err(|e| format!("Failed to delete tag: {}", e))?;
        Ok(())
    }
}

pub fn get_conversation_sql(list: &str) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM object_tag ot WHERE ot.object_type = '{conversation}' AND ot.object_id = c.id AND ot.tag_id IN ({list})) \
         OR EXISTS (SELECT 1 FROM object_tag ot WHERE ot.object_type = '{person}' AND ot.object_id = c.person_id AND ot.tag_id IN ({list})) \
         OR EXISTS (SELECT 1 FROM object_tag ot JOIN person_identity pi ON pi.id = ot.object_id JOIN conversation_member cm ON cm.user_id = pi.user_id WHERE ot.object_type = '{identity}' AND cm.conversation_id = c.id AND ot.tag_id IN ({list})) \
         OR EXISTS (SELECT 1 FROM object_tag ot JOIN merge_group_conversation mgc ON mgc.merge_group_id = ot.object_id WHERE ot.object_type = '{merge_group}' AND mgc.conversation_id = c.id AND ot.tag_id IN ({list})))",
        conversation = OBJECT_CONVERSATION,
        person = OBJECT_PERSON,
        identity = OBJECT_IDENTITY,
        merge_group = OBJECT_MERGE_GROUP,
        list = list,
    )
}

const NO_DETAILS: &str = "NULL AS detail, NULL AS conversation_id, NULL AS conversation_name, NULL AS message_id, NULL AS person_id, NULL AS author, NULL AS timestamp, NULL AS excerpt";

fn tagged_sql(object_type: &str) -> Option<String> {
    let (columns, join) = match object_type {
        OBJECT_CONVERSATION => ("x.name AS title, x.type AS detail, x.id AS conversation_id, x.name AS conversation_name, NULL AS message_id, x.person_id AS person_id, NULL AS author, x.last_activity AS timestamp, NULL AS excerpt".to_string(), "JOIN conversation x ON x.id = ot.object_id"),
        OBJECT_MESSAGE => ("COALESCE(NULLIF(x.subject, ''), c.name, '') AS title, x.type AS detail, x.conversation_id AS conversation_id, c.name AS conversation_name, x.id AS message_id, NULL AS person_id, COALESCE(NULLIF(x.sender_name, ''), x.sender_email) AS author, x.timestamp AS timestamp, substr(x.content, 1, 600) AS excerpt".to_string(), "JOIN message x ON x.id = ot.object_id LEFT JOIN conversation c ON c.id = x.conversation_id"),
        OBJECT_ATTACHMENT => ("COALESCE((SELECT json_extract(a.value, '$.name') FROM json_each(COALESCE(x.attachments, '[]')) a WHERE json_extract(a.value, '$.id') = substr(ot.object_id, instr(ot.object_id, '/') + 1)), '') AS title, NULL AS detail, x.conversation_id AS conversation_id, c.name AS conversation_name, x.id AS message_id, NULL AS person_id, COALESCE(NULLIF(x.sender_name, ''), x.sender_email) AS author, x.timestamp AS timestamp, NULL AS excerpt".to_string(), "JOIN message x ON x.id = substr(ot.object_id, 1, instr(ot.object_id, '/') - 1) LEFT JOIN conversation c ON c.id = x.conversation_id"),
        OBJECT_PERSON => ("x.display_name AS title, x.primary_email AS detail, NULL AS conversation_id, NULL AS conversation_name, NULL AS message_id, x.id AS person_id, NULL AS author, NULL AS timestamp, NULL AS excerpt".to_string(), "JOIN person x ON x.id = ot.object_id"),
        OBJECT_IDENTITY => ("COALESCE(NULLIF(x.display_name, ''), x.email, x.external_id, '') AS title, x.provider_type AS detail, NULL AS conversation_id, NULL AS conversation_name, NULL AS message_id, x.person_id AS person_id, NULL AS author, NULL AS timestamp, x.email AS excerpt".to_string(), "JOIN person_identity x ON x.id = ot.object_id"),
        OBJECT_CONTENT => ("x.name AS title, x.kind AS detail, x.source_conversation_id AS conversation_id, NULL AS conversation_name, x.source_message_id AS message_id, x.source_person_id AS person_id, NULL AS author, x.created_at AS timestamp, substr(x.body, 1, 600) AS excerpt".to_string(), "JOIN content_item x ON x.id = ot.object_id"),
        OBJECT_EVENT => ("x.title AS title, x.location AS detail, x.conversation_id AS conversation_id, NULL AS conversation_name, x.source_message_id AS message_id, NULL AS person_id, NULL AS author, x.starts_at AS timestamp, substr(x.notes, 1, 600) AS excerpt".to_string(), "JOIN event x ON x.id = ot.object_id"),
        OBJECT_ACTION => (format!("x.name AS title, {}", NO_DETAILS), "JOIN action x ON x.id = ot.object_id"),
        OBJECT_ACTION_GROUP => (format!("x.name AS title, {}", NO_DETAILS), "JOIN action_group x ON x.id = ot.object_id"),
        OBJECT_GROUP => (format!("x.name AS title, {}", NO_DETAILS), "JOIN conversation_group x ON x.id = ot.object_id"),
        OBJECT_INBOX => (format!("x.name AS title, {}", NO_DETAILS), "JOIN inbox x ON x.id = ot.object_id"),
        OBJECT_MERGE_GROUP => ("COALESCE(NULLIF(x.name, ''), c.name, '') AS title, NULL AS detail, x.lead_conversation_id AS conversation_id, c.name AS conversation_name, NULL AS message_id, NULL AS person_id, NULL AS author, c.last_activity AS timestamp, NULL AS excerpt".to_string(), "JOIN merge_group x ON x.id = ot.object_id LEFT JOIN conversation c ON c.id = x.lead_conversation_id"),
        _ => return None,
    };
    Some(format!(
        "SELECT ot.tag_id, ot.object_id, ot.created_at, {columns} FROM object_tag ot JOIN tag t ON t.id = ot.tag_id {join}
         WHERE t.profile_id = ?1 AND ot.object_type = ?2 AND (?3 IS NULL OR ot.tag_id = ?3)
         ORDER BY ot.created_at DESC"
    ))
}

pub async fn tag_named(pool: &Pool<Sqlite>, profile_id: &str, name: &str) -> Result<String, sqlx::Error> {
    let existing: Option<String> = sqlx::query_scalar("SELECT id FROM tag WHERE profile_id = ?1 AND lower(name) = lower(?2) ORDER BY kind = ?3 DESC LIMIT 1")
        .bind(profile_id)
        .bind(name)
        .bind(KIND_TAG)
        .fetch_optional(pool)
        .await?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let position: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tag WHERE profile_id = ?1 AND kind = ?2").bind(profile_id).bind(KIND_TAG).fetch_one(pool).await?;
    sqlx::query("INSERT INTO tag (id, profile_id, name, kind, position, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)")
        .bind(&id)
        .bind(profile_id)
        .bind(name)
        .bind(KIND_TAG)
        .bind(position)
        .bind(&now)
        .execute(pool)
        .await?;
    Ok(id)
}

impl ObjectTag {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        Self {
            tag_id: row.get("tag_id"),
            object_type: row.get("object_type"),
            object_id: row.get("object_id"),
            source: row.get("source"),
            rule_id: row.try_get("rule_id").ok().flatten(),
            reason: row.try_get("reason").ok().flatten(),
            created_at: row.get("created_at"),
        }
    }

    pub async fn get(pool: &Pool<Sqlite>, object_type: &str, object_id: &str) -> Result<Vec<ObjectTag>, String> {
        let rows = sqlx::query("SELECT * FROM object_tag WHERE object_type = ?1 AND object_id = ?2 ORDER BY created_at, rowid")
            .bind(object_type)
            .bind(object_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load tags: {}", e))?;
        Ok(rows.into_iter().map(ObjectTag::from_row).collect())
    }

    pub async fn get_all(pool: &Pool<Sqlite>, profile_id: &str, object_type: Option<&str>) -> Result<Vec<ObjectTag>, String> {
        let rows = sqlx::query("SELECT ot.* FROM object_tag ot JOIN tag t ON t.id = ot.tag_id WHERE t.profile_id = ?1 AND (?2 IS NULL OR ot.object_type = ?2) ORDER BY ot.created_at")
            .bind(profile_id)
            .bind(object_type)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load tags: {}", e))?;
        Ok(rows.into_iter().map(ObjectTag::from_row).collect())
    }

    pub async fn get_conversation(pool: &Pool<Sqlite>, conversation_id: &str) -> Result<Vec<ObjectTag>, String> {
        let mut tags = ObjectTag::get(pool, OBJECT_CONVERSATION, conversation_id).await?;
        let derived: Vec<(String, String)> = sqlx::query_as(
            r#"SELECT ot.tag_id, ot.object_type FROM object_tag ot JOIN conversation c ON c.person_id = ot.object_id
               WHERE ot.object_type = ?2 AND c.id = ?1
               UNION
               SELECT ot.tag_id, ot.object_type FROM object_tag ot
               JOIN person_identity pi ON pi.id = ot.object_id
               JOIN conversation_member cm ON cm.user_id = pi.user_id
               WHERE ot.object_type = ?3 AND cm.conversation_id = ?1"#,
        )
        .bind(conversation_id)
        .bind(OBJECT_PERSON)
        .bind(OBJECT_IDENTITY)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load person tags for the conversation: {}", e))?;
        for (tag_id, source) in derived {
            if !tags.iter().any(|tag| tag.tag_id == tag_id) {
                tags.push(ObjectTag { tag_id, object_type: OBJECT_CONVERSATION.to_string(), object_id: conversation_id.to_string(), source, rule_id: None, reason: None, created_at: String::new() });
            }
        }
        Ok(tags)
    }

    pub async fn get_conversation_ids(pool: &Pool<Sqlite>, tag_id: &str) -> Result<Vec<String>, String> {
        sqlx::query_scalar(&format!("SELECT c.id FROM conversation c WHERE c.deleted_at IS NULL AND {} ORDER BY c.last_activity DESC", get_conversation_sql("?1")))
            .bind(tag_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read the tag: {}", e))
    }

    pub async fn put(pool: &Pool<Sqlite>, object_type: &str, object_id: &str, tag_id: &str, source: &str, rule_id: Option<&str>, reason: Option<&str>) -> Result<(), String> {
        if !OBJECT_TYPES.contains(&object_type) {
            return Err(format!("Unknown object type {}", object_type));
        }
        let label = Tag::get(pool, tag_id).await?;
        if label.kind == KIND_SPLIT {
            sqlx::query("DELETE FROM object_tag WHERE object_type = ?1 AND object_id = ?2 AND tag_id IN (SELECT id FROM tag WHERE kind = ?3 AND profile_id = ?4)")
                .bind(object_type)
                .bind(object_id)
                .bind(KIND_SPLIT)
                .bind(&label.profile_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to move the conversation between splits: {}", e))?;
        }
        sqlx::query(
            "INSERT INTO object_tag (tag_id, object_type, object_id, source, rule_id, reason, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(tag_id, object_type, object_id) DO UPDATE SET source = excluded.source, rule_id = excluded.rule_id, reason = excluded.reason, created_at = excluded.created_at",
        )
        .bind(tag_id)
        .bind(object_type)
        .bind(object_id)
        .bind(source)
        .bind(rule_id)
        .bind(reason)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to tag the {}: {}", object_type, e))?;
        let pin = match object_type {
            OBJECT_PERSON => Some("UPDATE person_identity SET manual = 1 WHERE person_id = ?1"),
            OBJECT_IDENTITY => Some("UPDATE person_identity SET manual = 1 WHERE person_id = (SELECT person_id FROM person_identity WHERE id = ?1)"),
            _ => None,
        };
        if let Some(pin) = pin {
            sqlx::query(pin).bind(object_id).execute(pool).await.map_err(|e| format!("Failed to pin the person: {}", e))?;
        }
        Ok(())
    }

    pub async fn delete(pool: &Pool<Sqlite>, object_type: &str, object_id: &str, tag_id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM object_tag WHERE object_type = ?1 AND object_id = ?2 AND tag_id = ?3")
            .bind(object_type)
            .bind(object_id)
            .bind(tag_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to remove the tag: {}", e))?;
        Ok(())
    }

    pub async fn delete_object(pool: &Pool<Sqlite>, object_type: &str, object_id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM object_tag WHERE object_type = ?1 AND object_id = ?2")
            .bind(object_type)
            .bind(object_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to clear the tags: {}", e))?;
        Ok(())
    }

    pub async fn put_object_id(pool: &Pool<Sqlite>, object_type: &str, from: &str, to: &str) -> Result<(), String> {
        sqlx::query("UPDATE OR IGNORE object_tag SET object_id = ?3 WHERE object_type = ?1 AND object_id = ?2")
            .bind(object_type)
            .bind(from)
            .bind(to)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to move the tags: {}", e))?;
        ObjectTag::delete_object(pool, object_type, from).await
    }

    pub async fn get_names(pool: &Pool<Sqlite>, profile_id: &str, object_type: &str) -> Result<HashMap<String, Vec<String>>, String> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT ot.object_id, t.name FROM object_tag ot JOIN tag t ON t.id = ot.tag_id WHERE t.profile_id = ?1 AND ot.object_type = ?2 ORDER BY ot.created_at, ot.rowid")
            .bind(profile_id)
            .bind(object_type)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load tags: {}", e))?;
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        for (object_id, name) in rows {
            out.entry(object_id).or_default().push(name);
        }
        Ok(out)
    }

    pub async fn put_names(pool: &Pool<Sqlite>, profile_id: &str, object_type: &str, object_id: &str, names: &[String]) -> Result<(), String> {
        let mut wanted = Vec::new();
        for name in names.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
            let id = tag_named(pool, profile_id, name).await.map_err(|e| format!("Failed to resolve the tag {}: {}", name, e))?;
            if !wanted.contains(&id) {
                wanted.push(id);
            }
        }
        for current in ObjectTag::get(pool, object_type, object_id).await? {
            if !wanted.contains(&current.tag_id) {
                ObjectTag::delete(pool, object_type, object_id, &current.tag_id).await?;
            }
        }
        let now = Utc::now().to_rfc3339();
        for id in &wanted {
            sqlx::query("INSERT OR IGNORE INTO object_tag (tag_id, object_type, object_id, source, created_at) VALUES (?1, ?2, ?3, 'manual', ?4)")
                .bind(id)
                .bind(object_type)
                .bind(object_id)
                .bind(&now)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to tag the {}: {}", object_type, e))?;
        }
        Ok(())
    }

    pub async fn get_tagged(pool: &Pool<Sqlite>, profile_id: &str, tag_id: Option<&str>) -> Result<Vec<TaggedObject>, String> {
        let mut out = Vec::new();
        for object_type in OBJECT_TYPES {
            let Some(sql) = tagged_sql(object_type) else { continue };
            let rows = sqlx::query(&sql)
                .bind(profile_id)
                .bind(object_type)
                .bind(tag_id)
                .fetch_all(pool)
                .await
                .map_err(|e| format!("Failed to list tagged {}: {}", object_type, e))?;
            for row in rows {
                out.push(TaggedObject {
                    tag_id: row.get("tag_id"),
                    object_type: object_type.to_string(),
                    object_id: row.get("object_id"),
                    title: row.try_get::<Option<String>, _>("title").ok().flatten().unwrap_or_default(),
                    detail: row.try_get("detail").ok().flatten(),
                    conversation_id: row.try_get("conversation_id").ok().flatten(),
                    conversation_name: row.try_get("conversation_name").ok().flatten(),
                    message_id: row.try_get("message_id").ok().flatten(),
                    person_id: row.try_get("person_id").ok().flatten(),
                    author: row.try_get("author").ok().flatten(),
                    timestamp: row.try_get("timestamp").ok().flatten(),
                    excerpt: row.try_get("excerpt").ok().flatten(),
                    created_at: row.get("created_at"),
                });
            }
        }
        Ok(out)
    }
}

pub fn identity_key(provider_type: &str, external_id: Option<&str>, email: Option<&str>) -> String {
    format!("{}:{}", provider_type, external_id.filter(|v| !v.is_empty()).or(email).unwrap_or_default()).to_lowercase()
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS object_tag (
            tag_id TEXT NOT NULL,
            object_type TEXT NOT NULL,
            object_id TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'manual',
            rule_id TEXT,
            reason TEXT,
            created_at TEXT NOT NULL,
            PRIMARY KEY (tag_id, object_type, object_id),
            FOREIGN KEY (tag_id) REFERENCES tag (id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_object_tag_object ON object_tag (object_type, object_id)").execute(pool).await?;
    retire_tag_tables(pool).await
}

pub async fn retire_tag_tables(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    const CONVERSATION_TAGS: &str = "conversation_tag";
    const PERSON_TAGS: &str = "person_tag";
    const CONTENT_TAGS: &str = "tags";
    if crate::database::table_exists(pool, CONVERSATION_TAGS).await? && !crate::database::table_exists(pool, RETIRED_CONVERSATION_TAGS).await? {
        sqlx::query(&format!(
            "INSERT OR IGNORE INTO object_tag (tag_id, object_type, object_id, source, rule_id, reason, created_at)
             SELECT tag_id, ?1, conversation_id, COALESCE(source, 'manual'), rule_id, reason, COALESCE(created_at, '') FROM {}",
            CONVERSATION_TAGS
        ))
        .bind(OBJECT_CONVERSATION)
        .execute(pool)
        .await?;
        sqlx::query(&format!("ALTER TABLE {} RENAME TO {}", CONVERSATION_TAGS, RETIRED_CONVERSATION_TAGS)).execute(pool).await?;
    }
    if crate::database::table_exists(pool, PERSON_TAGS).await? && !crate::database::table_exists(pool, RETIRED_PERSON_TAGS).await? {
        sqlx::query(&format!(
            "INSERT OR IGNORE INTO object_tag (tag_id, object_type, object_id, source, created_at)
             SELECT tag_id, ?1, person_id, 'manual', created_at FROM {} WHERE identity_key = ''",
            PERSON_TAGS
        ))
        .bind(OBJECT_PERSON)
        .execute(pool)
        .await?;
        sqlx::query(&format!(
            "INSERT OR IGNORE INTO object_tag (tag_id, object_type, object_id, source, created_at)
             SELECT pt.tag_id, ?1, pi.id, 'manual', pt.created_at FROM {} pt
             JOIN person_identity pi ON pi.person_id = pt.person_id
               AND lower(pi.provider_type || ':' || COALESCE(NULLIF(pi.external_id, ''), pi.email, '')) = pt.identity_key
             WHERE pt.identity_key != ''",
            PERSON_TAGS
        ))
        .bind(OBJECT_IDENTITY)
        .execute(pool)
        .await?;
        sqlx::query("UPDATE person_identity SET manual = 1 WHERE person_id IN (SELECT object_id FROM object_tag WHERE object_type = ?1) OR person_id IN (SELECT pi.person_id FROM person_identity pi JOIN object_tag ot ON ot.object_id = pi.id AND ot.object_type = ?2)")
            .bind(OBJECT_PERSON)
            .bind(OBJECT_IDENTITY)
            .execute(pool)
            .await?;
        sqlx::query(&format!("ALTER TABLE {} RENAME TO {}", PERSON_TAGS, RETIRED_PERSON_TAGS)).execute(pool).await?;
    }
    let content_columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('content_item')").fetch_all(pool).await?;
    if content_columns.iter().any(|c| c == CONTENT_TAGS) && !content_columns.iter().any(|c| c == RETIRED_CONTENT_TAGS) {
        let rows: Vec<(String, String, String)> = sqlx::query_as(&format!("SELECT id, profile_id, COALESCE({}, '[]') FROM content_item", CONTENT_TAGS)).fetch_all(pool).await?;
        for (id, profile_id, raw) in rows {
            let names: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
            for name in names.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
                let tag_id = tag_named(pool, &profile_id, name).await?;
                sqlx::query("INSERT OR IGNORE INTO object_tag (tag_id, object_type, object_id, source, created_at) VALUES (?1, ?2, ?3, 'manual', ?4)")
                    .bind(&tag_id)
                    .bind(OBJECT_CONTENT)
                    .bind(&id)
                    .bind(Utc::now().to_rfc3339())
                    .execute(pool)
                    .await?;
            }
        }
        sqlx::query(&format!("ALTER TABLE content_item RENAME COLUMN {} TO {}", CONTENT_TAGS, RETIRED_CONTENT_TAGS)).execute(pool).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_conversation, test_message, test_pool, test_profile, test_provider, test_user};

    async fn person_with(pool: &Pool<Sqlite>, profile: &str, name: &str, identities: &[(&str, &str, &str)]) -> String {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO person (id, profile_id, display_name, is_human, created_at, updated_at) VALUES (?1, ?2, ?3, 1, ?4, ?4)")
            .bind(&id)
            .bind(profile)
            .bind(name)
            .bind(&now)
            .execute(pool)
            .await
            .unwrap();
        for (provider, email, user) in identities {
            sqlx::query("INSERT INTO person_identity (id, person_id, user_id, provider_type, external_id, email, display_name, manual) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, 0)")
                .bind(Uuid::new_v4().to_string())
                .bind(&id)
                .bind(user)
                .bind(provider)
                .bind(email)
                .bind(name)
                .execute(pool)
                .await
                .unwrap();
        }
        id
    }

    async fn identity_of(pool: &Pool<Sqlite>, external: &str) -> String {
        sqlx::query_scalar("SELECT id FROM person_identity WHERE external_id = ?1").bind(external).fetch_one(pool).await.unwrap()
    }

    async fn conversation_with(pool: &Pool<Sqlite>, name: &str, person: Option<&str>, member: &str) -> String {
        let id = test_conversation(pool, name, "direct_message").await;
        sqlx::query("UPDATE conversation SET person_id = ?1 WHERE id = ?2").bind(person).bind(&id).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO conversation_member (id, conversation_id, user_id) VALUES (?1, ?2, ?3)").bind(Uuid::new_v4().to_string()).bind(&id).bind(member).execute(pool).await.unwrap();
        id
    }

    fn ids(tags: Vec<ObjectTag>) -> Vec<String> {
        tags.into_iter().map(|t| t.tag_id).collect()
    }

    #[tokio::test]
    async fn person_tags_cover_every_conversation_and_identity_tags_only_their_own() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let mail = test_user(&pool, "anna@client.example", "Anna").await;
        let chat = test_user(&pool, "anna.chat@client.example", "Anna Teams").await;
        let anna = person_with(&pool, &profile, "Anna", &[("microsoft", "anna@client.example", &mail), ("slack", "U1", &chat)]).await;
        let thread = conversation_with(&pool, "Mail thread", Some(&anna), &mail).await;
        let dm = conversation_with(&pool, "Slack DM", None, &chat).await;
        let client = Tag::post(&pool, &profile, "Client", "#1f9d55", KIND_TAG, 0, "").await.unwrap();
        let slack_only = Tag::post(&pool, &profile, "Chatty", "#e8743b", KIND_TAG, 1, "").await.unwrap();
        let slack = identity_of(&pool, "U1").await;

        ObjectTag::put(&pool, OBJECT_PERSON, &anna, &client.id, "manual", None, None).await.unwrap();
        ObjectTag::put(&pool, OBJECT_IDENTITY, &slack, &slack_only.id, "manual", None, None).await.unwrap();

        assert_eq!(ids(ObjectTag::get_conversation(&pool, &thread).await.unwrap()), vec![client.id.clone()]);
        let on_dm = ObjectTag::get_conversation(&pool, &dm).await.unwrap();
        assert_eq!(on_dm.len(), 1);
        assert_eq!((on_dm[0].tag_id.as_str(), on_dm[0].source.as_str()), (slack_only.id.as_str(), OBJECT_IDENTITY));

        let pinned: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM person_identity WHERE person_id = ?1 AND manual = 1").bind(&anna).fetch_one(&pool).await.unwrap();
        assert_eq!(pinned, 2);

        ObjectTag::delete(&pool, OBJECT_PERSON, &anna, &client.id).await.unwrap();
        assert!(ObjectTag::get_conversation(&pool, &thread).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn tag_filters_match_person_and_identity_tags() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "mock").await;
        let mail = test_user(&pool, "ola@client.example", "Ola").await;
        let chat = test_user(&pool, "ola.chat@client.example", "Ola Slack").await;
        let ola = person_with(&pool, &profile, "Ola", &[("microsoft", "ola@client.example", &mail), ("slack", "U3", &chat)]).await;
        let thread = conversation_with(&pool, "Mail thread", Some(&ola), &mail).await;
        let dm = conversation_with(&pool, "Slack DM", None, &chat).await;
        let other = conversation_with(&pool, "Someone else", None, &mail).await;
        for conversation in [&thread, &dm, &other] {
            sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id) VALUES (?1, ?2, ?3, ?1)").bind(Uuid::new_v4().to_string()).bind(conversation).bind(&provider).execute(&pool).await.unwrap();
        }
        let client = Tag::post(&pool, &profile, "Client", "#1f9d55", KIND_TAG, 0, "").await.unwrap();
        let chatty = Tag::post(&pool, &profile, "Chatty", "#e8743b", KIND_TAG, 1, "").await.unwrap();
        ObjectTag::put(&pool, OBJECT_PERSON, &ola, &client.id, "manual", None, None).await.unwrap();
        ObjectTag::put(&pool, OBJECT_IDENTITY, &identity_of(&pool, "U3").await, &chatty.id, "manual", None, None).await.unwrap();

        let matching = |tag: &str| serde_json::from_value::<crate::models::group::GroupQuery>(serde_json::json!({ "tag_ids": [tag] })).unwrap();
        assert_eq!(crate::services::group_service::evaluate(&pool, &profile, &matching(&client.id)).await.unwrap(), vec![thread.clone()]);
        assert_eq!(crate::services::group_service::evaluate(&pool, &profile, &matching(&chatty.id)).await.unwrap(), vec![dm.clone()]);
        assert_eq!(ObjectTag::get_conversation_ids(&pool, &client.id).await.unwrap(), vec![thread.clone()]);
    }

    #[tokio::test]
    async fn person_tags_follow_merges_and_identity_tags_follow_a_split() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let mail = test_user(&pool, "jonas@client.example", "Jonas").await;
        let chat = test_user(&pool, "jonas.chat@client.example", "Jonas Slack").await;
        let target = person_with(&pool, &profile, "Jonas", &[("microsoft", "jonas@client.example", &mail)]).await;
        let source = person_with(&pool, &profile, "Jonas Slack", &[("slack", "U2", &chat)]).await;
        let vendor = Tag::post(&pool, &profile, "Vendor", "#1f9d55", KIND_TAG, 0, "").await.unwrap();
        let friend = Tag::post(&pool, &profile, "Friend", "#1f9d55", KIND_TAG, 1, "").await.unwrap();
        let identity = identity_of(&pool, "U2").await;
        ObjectTag::put(&pool, OBJECT_IDENTITY, &identity, &vendor.id, "manual", None, None).await.unwrap();
        ObjectTag::put(&pool, OBJECT_PERSON, &source, &friend.id, "manual", None, None).await.unwrap();

        crate::models::Person::merge(&pool, &source, &target).await.unwrap();
        assert_eq!(ids(ObjectTag::get(&pool, OBJECT_PERSON, &target).await.unwrap()), vec![friend.id.clone()]);
        assert!(ObjectTag::get(&pool, OBJECT_PERSON, &source).await.unwrap().is_empty());

        crate::models::Person::split(&pool, &identity).await.unwrap();
        assert_eq!(ids(ObjectTag::get(&pool, OBJECT_IDENTITY, &identity).await.unwrap()), vec![vendor.id.clone()]);
    }

    #[tokio::test]
    async fn any_object_takes_tags_and_the_listing_groups_them_by_type() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "mock").await;
        let conversation = test_conversation(&pool, "Kunde chat", "group_message").await;
        let message = test_message(&pool, &conversation, &provider, "The contract is signed, see attached").await;
        let tag = Tag::post(&pool, &profile, "Contract", "#1f9d55", KIND_TAG, 0, "").await.unwrap();
        let other = Tag::post(&pool, &profile, "Other", "#e8743b", KIND_TAG, 1, "").await.unwrap();
        ObjectTag::put(&pool, OBJECT_MESSAGE, &message, &tag.id, "manual", None, None).await.unwrap();
        ObjectTag::put(&pool, OBJECT_CONVERSATION, &conversation, &tag.id, "manual", None, None).await.unwrap();
        ObjectTag::put(&pool, OBJECT_MESSAGE, &message, &other.id, "manual", None, None).await.unwrap();
        assert!(ObjectTag::put(&pool, "nonsense", &message, &tag.id, "manual", None, None).await.is_err());

        let tagged = ObjectTag::get_tagged(&pool, &profile, Some(&tag.id)).await.unwrap();
        assert_eq!(tagged.iter().map(|t| t.object_type.as_str()).collect::<Vec<_>>(), vec![OBJECT_CONVERSATION, OBJECT_MESSAGE]);
        let hit = tagged.iter().find(|t| t.object_type == OBJECT_MESSAGE).unwrap();
        assert_eq!(hit.conversation_id.as_deref(), Some(conversation.as_str()));
        assert_eq!(hit.conversation_name.as_deref(), Some("Kunde chat"));
        assert_eq!(hit.message_id.as_deref(), Some(message.as_str()));
        assert_eq!(hit.author.as_deref(), Some("Test User"));
        assert!(hit.excerpt.as_deref().unwrap().contains("contract is signed"));
        assert!(hit.timestamp.is_some());
        assert_eq!(ObjectTag::get_tagged(&pool, &profile, None).await.unwrap().len(), 3);
        assert_eq!(ObjectTag::get_all(&pool, &profile, Some(OBJECT_MESSAGE)).await.unwrap().len(), 2);
        assert_eq!(ids(ObjectTag::get_conversation(&pool, &conversation).await.unwrap()), vec![tag.id.clone()]);

        Tag::delete(&pool, &tag.id).await.unwrap();
        assert_eq!(ids(ObjectTag::get(&pool, OBJECT_MESSAGE, &message).await.unwrap()), vec![other.id.clone()]);
    }

    #[tokio::test]
    async fn names_resolve_to_tags_case_insensitively_and_replace_the_set() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let kunde = Tag::post(&pool, &profile, "Kunde AS", "#1f9d55", KIND_TAG, 0, "").await.unwrap();
        ObjectTag::put_names(&pool, &profile, OBJECT_CONTENT, "item", &["kunde as".into(), "party".into(), "Party".into()]).await.unwrap();
        let names = ObjectTag::get_names(&pool, &profile, OBJECT_CONTENT).await.unwrap();
        assert_eq!(names.get("item").unwrap(), &vec!["Kunde AS".to_string(), "party".to_string()]);
        assert!(ObjectTag::get(&pool, OBJECT_CONTENT, "item").await.unwrap().iter().any(|t| t.tag_id == kunde.id));
        ObjectTag::put_names(&pool, &profile, OBJECT_CONTENT, "item", &["party".into()]).await.unwrap();
        assert_eq!(ObjectTag::get_names(&pool, &profile, OBJECT_CONTENT).await.unwrap().get("item").unwrap(), &vec!["party".to_string()]);
        assert_eq!(Tag::all(&pool, &profile).await.unwrap().iter().filter(|t| t.name.eq_ignore_ascii_case("party")).count(), 1);
    }

    #[tokio::test]
    async fn conversation_person_and_content_tags_move_into_the_object_table() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let mail = test_user(&pool, "kari@client.example", "Kari").await;
        let chat = test_user(&pool, "kari.chat@client.example", "Kari Slack").await;
        let kari = person_with(&pool, &profile, "Kari", &[("microsoft", "kari@client.example", &mail), ("slack", "U9", &chat)]).await;
        let conversation = test_conversation(&pool, "Kari", "direct_message").await;
        let client = Tag::post(&pool, &profile, "Client", "#1f9d55", KIND_TAG, 0, "").await.unwrap();
        let support = Tag::post(&pool, &profile, "Support", "#e8743b", KIND_TAG, 1, "").await.unwrap();
        sqlx::query("CREATE TABLE conversation_tag (conversation_id TEXT NOT NULL, tag_id TEXT NOT NULL, source TEXT NOT NULL DEFAULT 'manual', rule_id TEXT, reason TEXT, created_at TEXT NOT NULL, PRIMARY KEY (conversation_id, tag_id))").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO conversation_tag VALUES (?1, ?2, 'rule', 'r1', 'matched', '2026-01-01')").bind(&conversation).bind(&client.id).execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE person_tag (tag_id TEXT NOT NULL, person_id TEXT NOT NULL, identity_key TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL, PRIMARY KEY (tag_id, person_id, identity_key))").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO person_tag VALUES (?1, ?2, '', '2026-01-02'), (?3, ?2, ?4, '2026-01-03')").bind(&client.id).bind(&kari).bind(&support.id).bind(identity_key("slack", Some("U9"), None)).execute(&pool).await.unwrap();
        sqlx::query("ALTER TABLE content_item ADD COLUMN tags TEXT NOT NULL DEFAULT '[]'").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO content_item (id, profile_id, kind, name, path, mime_type, created_at, tags) VALUES ('gif', ?1, 'gif', 'Parrot', 'media/p.gif', 'image/gif', '2026-01-04', '[\"party\",\"client\"]')").bind(&profile).execute(&pool).await.unwrap();

        retire_tag_tables(&pool).await.unwrap();
        retire_tag_tables(&pool).await.unwrap();

        let moved = ObjectTag::get(&pool, OBJECT_CONVERSATION, &conversation).await.unwrap();
        assert_eq!(moved, vec![ObjectTag { tag_id: client.id.clone(), object_type: OBJECT_CONVERSATION.into(), object_id: conversation.clone(), source: "rule".into(), rule_id: Some("r1".into()), reason: Some("matched".into()), created_at: "2026-01-01".into() }]);
        assert_eq!(ids(ObjectTag::get(&pool, OBJECT_PERSON, &kari).await.unwrap()), vec![client.id.clone()]);
        assert_eq!(ids(ObjectTag::get(&pool, OBJECT_IDENTITY, &identity_of(&pool, "U9").await).await.unwrap()), vec![support.id.clone()]);
        let pinned: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM person_identity WHERE person_id = ?1 AND manual = 1").bind(&kari).fetch_one(&pool).await.unwrap();
        assert_eq!(pinned, 2);
        let content = ObjectTag::get_names(&pool, &profile, OBJECT_CONTENT).await.unwrap();
        assert_eq!(content.get("gif").unwrap(), &vec!["party".to_string(), "Client".to_string()]);
        assert!(crate::database::table_exists(&pool, RETIRED_CONVERSATION_TAGS).await.unwrap());
        assert!(crate::database::table_exists(&pool, RETIRED_PERSON_TAGS).await.unwrap());
        assert!(!crate::database::table_exists(&pool, "conversation_tag").await.unwrap());
        let kept: String = sqlx::query_scalar(&format!("SELECT {} FROM content_item WHERE id = 'gif'", RETIRED_CONTENT_TAGS)).fetch_one(&pool).await.unwrap();
        assert!(kept.contains("party"));
        let retired: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {}", RETIRED_PERSON_TAGS)).fetch_one(&pool).await.unwrap();
        assert_eq!(retired, 2);
    }

    async fn owned_conversation(pool: &Pool<Sqlite>, provider_id: &str, name: &str) -> String {
        let id = test_conversation(pool, name, "direct_message").await;
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id) VALUES (?1 || '-cp', ?1, ?2, ?1)")
            .bind(&id)
            .bind(provider_id)
            .execute(pool)
            .await
            .unwrap();
        id
    }

    #[tokio::test]
    async fn stars_and_favourites_become_favourite_tags_once() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "mock").await;
        let starred = owned_conversation(&pool, &provider, "Starred chat").await;
        let favourite = owned_conversation(&pool, &provider, "Favourite chat").await;
        let plain = owned_conversation(&pool, &provider, "Plain chat").await;
        sqlx::query("UPDATE conversation SET is_pinned = 1 WHERE id = ?1").bind(&starred).execute(&pool).await.unwrap();
        sqlx::query("UPDATE conversation SET favourite_position = 0 WHERE id = ?1").bind(&favourite).execute(&pool).await.unwrap();

        Tag::migrate_favourites(&pool, &profile).await.unwrap();
        Tag::migrate_favourites(&pool, &profile).await.unwrap();

        let tags = Tag::all(&pool, &profile).await.unwrap();
        let star = tags.iter().find(|l| l.name == "Starred").unwrap();
        let fav = tags.iter().find(|l| l.name == "Favourites").unwrap();
        assert!(star.favourite && fav.favourite);
        assert_eq!(star.icon.as_deref(), Some("lucide:star"));
        assert_ne!(star.favourite_position, fav.favourite_position);
        assert_eq!(ids(ObjectTag::get_conversation(&pool, &starred).await.unwrap()), vec![star.id.clone()]);
        assert_eq!(ids(ObjectTag::get_conversation(&pool, &favourite).await.unwrap()), vec![fav.id.clone()]);
        assert!(ObjectTag::get_conversation(&pool, &plain).await.unwrap().is_empty());
        let pinned: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM conversation WHERE is_pinned = 1 OR favourite_position IS NOT NULL").fetch_one(&pool).await.unwrap();
        assert_eq!(pinned, 0);
        assert_eq!(tags.iter().filter(|l| l.name == "Starred").count(), 1);
    }

    #[tokio::test]
    async fn favourite_tags_keep_a_manual_order() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let a = Tag::post(&pool, &profile, "A", "#000000", KIND_TAG, 0, "").await.unwrap();
        let b = Tag::post(&pool, &profile, "B", "#000000", KIND_TAG, 1, "").await.unwrap();
        Tag::put_favourite(&pool, &a.id, true).await.unwrap();
        Tag::put_favourite(&pool, &b.id, true).await.unwrap();
        Tag::put_favourite_order(&pool, &profile, &[b.id.clone(), a.id.clone()]).await.unwrap();
        assert_eq!(Tag::get(&pool, &b.id).await.unwrap().favourite_position, Some(0));
        assert_eq!(Tag::get(&pool, &a.id).await.unwrap().favourite_position, Some(1));
        let off = Tag::put_favourite(&pool, &a.id, false).await.unwrap();
        assert!(!off.favourite && off.favourite_position.is_none());
    }

    #[tokio::test]
    async fn defaults_are_seeded_once() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        Tag::ensure_defaults(&pool, &profile).await.unwrap();
        Tag::ensure_defaults(&pool, &profile).await.unwrap();
        let tags = Tag::all(&pool, &profile).await.unwrap();
        assert_eq!(tags.len(), DEFAULT_SPLITS.len());
        assert_eq!(tags[0].name, "Important");
    }

    #[tokio::test]
    async fn a_conversation_sits_in_one_split_but_many_tags() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        Tag::ensure_defaults(&pool, &profile).await.unwrap();
        let conversation = test_conversation(&pool, "Lunch", "email").await;
        let tags = Tag::all(&pool, &profile).await.unwrap();
        let important = tags.iter().find(|l| l.name == "Important").unwrap();
        let other = tags.iter().find(|l| l.name == "Other").unwrap();
        let tag = Tag::post(&pool, &profile, "Travel", "#ccc", KIND_TAG, 0, "").await.unwrap();
        ObjectTag::put(&pool, OBJECT_CONVERSATION, &conversation, &important.id, "manual", None, None).await.unwrap();
        ObjectTag::put(&pool, OBJECT_CONVERSATION, &conversation, &tag.id, "manual", None, None).await.unwrap();
        ObjectTag::put(&pool, OBJECT_CONVERSATION, &conversation, &other.id, "rule", None, Some("matched")).await.unwrap();
        let attached = ids(ObjectTag::get_conversation(&pool, &conversation).await.unwrap());
        assert!(attached.contains(&other.id));
        assert!(attached.contains(&tag.id));
        assert!(!attached.contains(&important.id));
        Tag::delete(&pool, &tag.id).await.unwrap();
        assert_eq!(ObjectTag::get_conversation(&pool, &conversation).await.unwrap().len(), 1);
    }
}
