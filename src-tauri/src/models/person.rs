use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::models::object_override::{ObjectOverride, PutObjectOverrideInput, FIELD_AVATAR, FIELD_DISPLAY_NAME, SCOPE_CONVERSATION};
use crate::models::tag::OBJECT_PERSON;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonIdentity {
    pub id: String,
    pub person_id: String,
    pub user_id: Option<String>,
    pub provider_type: String,
    pub external_id: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub manual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub profile_id: String,
    pub display_name: String,
    pub provider_name: String,
    pub primary_email: Option<String>,
    pub avatar_url: Option<String>,
    pub avatar_data: Option<String>,
    pub overrides: BTreeMap<String, String>,
    pub nicknames: BTreeMap<String, String>,
    pub is_human: bool,
    pub favourite_position: Option<i64>,
    pub identities: Vec<PersonIdentity>,
    pub conversation_ids: Vec<String>,
    pub provider_types: Vec<String>,
    pub last_activity: Option<String>,
    pub unread_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutPersonInput {
    pub person_id: String,
    pub display_name: Option<String>,
    pub is_human: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub favourite_position: Option<Option<i64>>,
}

pub fn deserialize_double_option<'de, D>(deserializer: D) -> Result<Option<Option<i64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<i64>::deserialize(deserializer).map(Some)
}

pub fn deserialize_double_option_string<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostIdentityInput {
    pub person_id: String,
    pub provider_type: String,
    pub email: Option<String>,
    pub external_id: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FavouriteOrderItem {
    pub kind: String,
    pub id: String,
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS person (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            display_name TEXT NOT NULL,
            primary_email TEXT,
            avatar_url TEXT,
            avatar_data TEXT,
            is_human BOOLEAN NOT NULL DEFAULT 1,
            human_override BOOLEAN,
            favourite_position INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profile (id)
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS person_identity (
            id TEXT PRIMARY KEY,
            person_id TEXT NOT NULL,
            user_id TEXT,
            provider_type TEXT NOT NULL,
            external_id TEXT,
            email TEXT,
            display_name TEXT,
            manual BOOLEAN NOT NULL DEFAULT 0,
            UNIQUE(provider_type, external_id),
            FOREIGN KEY (person_id) REFERENCES person (id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    crate::database::ensure_columns(pool, "person_identity", &[("manual", "BOOLEAN NOT NULL DEFAULT 0")]).await;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS person_signature (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            person_id TEXT NOT NULL,
            hash TEXT NOT NULL,
            text TEXT NOT NULL,
            sender_email TEXT,
            provider_id TEXT,
            conversation_id TEXT,
            message_id TEXT,
            first_seen TEXT NOT NULL,
            last_seen TEXT NOT NULL,
            use_count INTEGER NOT NULL DEFAULT 1,
            UNIQUE(profile_id, person_id, hash),
            FOREIGN KEY (person_id) REFERENCES person (id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_person_identity_person ON person_identity(person_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_conversation_person ON conversation(person_id)").execute(pool).await.ok();
    Ok(())
}

impl PersonIdentity {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        Self {
            id: row.get("id"),
            person_id: row.get("person_id"),
            user_id: row.get("user_id"),
            provider_type: row.get("provider_type"),
            external_id: row.get("external_id"),
            email: row.get("email"),
            display_name: row.get("display_name"),
            manual: row.try_get("manual").unwrap_or(false),
        }
    }

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Self>, String> {
        let rows = sqlx::query(
            "SELECT i.* FROM person_identity i JOIN person p ON p.id = i.person_id WHERE p.profile_id = ?1 ORDER BY i.provider_type, i.display_name",
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load identities: {}", e))?;
        Ok(rows.into_iter().map(Self::from_row).collect())
    }

    pub async fn get(pool: &Pool<Sqlite>, identity_id: &str) -> Result<Option<Self>, String> {
        let row = sqlx::query("SELECT * FROM person_identity WHERE id = ?1")
            .bind(identity_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load identity: {}", e))?;
        Ok(row.map(Self::from_row))
    }
}

impl Person {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        Self {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            display_name: row.get("display_name"),
            provider_name: row.get("display_name"),
            primary_email: row.get("primary_email"),
            avatar_url: row.get("avatar_url"),
            avatar_data: row.get("avatar_data"),
            overrides: BTreeMap::new(),
            nicknames: BTreeMap::new(),
            is_human: row.get("is_human"),
            favourite_position: row.get("favourite_position"),
            identities: Vec::new(),
            conversation_ids: Vec::new(),
            provider_types: Vec::new(),
            last_activity: None,
            unread_count: 0,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Self>, String> {
        let rows = sqlx::query("SELECT * FROM person WHERE profile_id = ?1 ORDER BY display_name")
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load people: {}", e))?;
        let mut people: Vec<Self> = rows.into_iter().map(Self::from_row).collect();
        let identities = PersonIdentity::all(pool, profile_id).await?;
        let overrides: Vec<ObjectOverride> = ObjectOverride::get_all(pool, profile_id).await?.into_iter().filter(|o| o.object_type == OBJECT_PERSON).collect();
        for person in &mut people {
            for local in overrides.iter().filter(|o| o.object_id == person.id) {
                match local.scope_type.as_str() {
                    SCOPE_CONVERSATION => {
                        person.nicknames.insert(local.scope_id.clone(), local.value.clone());
                    }
                    _ => {
                        person.overrides.insert(local.field.clone(), local.value.clone());
                    }
                }
            }
            if let Some(name) = person.overrides.get(FIELD_DISPLAY_NAME) {
                person.display_name = name.clone();
            }
            if let Some(avatar) = person.overrides.get(FIELD_AVATAR) {
                person.avatar_data = Some(avatar.clone());
            }
        }
        let conversations = sqlx::query(
            "SELECT id, person_id, last_activity, unread_count FROM conversation WHERE person_id IS NOT NULL AND deleted_at IS NULL",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load person conversations: {}", e))?;
        for person in &mut people {
            person.identities = identities.iter().filter(|i| i.person_id == person.id).cloned().collect();
            let mut types: Vec<String> = person.identities.iter().map(|i| i.provider_type.clone()).collect();
            types.sort();
            types.dedup();
            person.provider_types = types;
            for row in &conversations {
                let person_id: String = row.get("person_id");
                if person_id != person.id {
                    continue;
                }
                person.conversation_ids.push(row.get("id"));
                let activity: String = row.get("last_activity");
                if person.last_activity.as_deref().map(|a| activity.as_str() > a).unwrap_or(true) {
                    person.last_activity = Some(activity);
                }
                person.unread_count += row.get::<i64, _>("unread_count");
            }
        }
        Ok(people)
    }

    pub async fn get(pool: &Pool<Sqlite>, profile_id: &str, person_id: &str) -> Result<Self, String> {
        Self::all(pool, profile_id)
            .await?
            .into_iter()
            .find(|p| p.id == person_id)
            .ok_or_else(|| "Person not found".to_string())
    }

    pub async fn put(pool: &Pool<Sqlite>, input: &PutPersonInput) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        if let Some(name) = &input.display_name {
            ObjectOverride::put(pool, &PutObjectOverrideInput { object_type: OBJECT_PERSON.to_string(), object_id: input.person_id.clone(), scope_type: None, scope_id: None, field: FIELD_DISPLAY_NAME.to_string(), value: Some(name.clone()) }).await?;
        }
        if let Some(is_human) = input.is_human {
            sqlx::query("UPDATE person SET is_human = ?1, human_override = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(is_human)
                .bind(&now)
                .bind(&input.person_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to flag person: {}", e))?;
        }
        if let Some(position) = &input.favourite_position {
            sqlx::query("UPDATE person SET favourite_position = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(position)
                .bind(&now)
                .bind(&input.person_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to order person: {}", e))?;
        }
        Ok(())
    }

    pub async fn merge(pool: &Pool<Sqlite>, source_id: &str, target_id: &str) -> Result<(), String> {
        if source_id == target_id {
            return Ok(());
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE person_identity SET person_id = ?1, manual = 1 WHERE person_id IN (?1, ?2)")
            .bind(target_id)
            .bind(source_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to move identities: {}", e))?;
        sqlx::query("UPDATE conversation SET person_id = ?1, updated_at = ?3 WHERE person_id = ?2")
            .bind(target_id)
            .bind(source_id)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to move conversations: {}", e))?;
        sqlx::query("UPDATE person SET is_human = 1, human_override = 1, updated_at = ?1 WHERE id = ?2 AND is_human = 0 AND EXISTS (SELECT 1 FROM person WHERE id = ?3 AND is_human = 1)")
            .bind(&now)
            .bind(target_id)
            .bind(source_id)
            .execute(pool)
            .await
            .ok();
        crate::models::ObjectTag::put_object_id(pool, crate::models::tag::OBJECT_PERSON, source_id, target_id).await?;
        ObjectOverride::put_object_id(pool, OBJECT_PERSON, source_id, target_id).await?;
        sqlx::query("DELETE FROM person WHERE id = ?1")
            .bind(source_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to remove merged person: {}", e))?;
        Ok(())
    }

    pub async fn split(pool: &Pool<Sqlite>, identity_id: &str) -> Result<String, String> {
        let identity = PersonIdentity::get(pool, identity_id).await?.ok_or_else(|| "Identity not found".to_string())?;
        let old: Option<(String, bool)> = sqlx::query_as("SELECT profile_id, is_human FROM person WHERE id = ?1")
            .bind(&identity.person_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load person: {}", e))?;
        let (profile_id, is_human) = old.ok_or_else(|| "Person not found".to_string())?;
        let now = Utc::now().to_rfc3339();
        let new_id = Uuid::new_v4().to_string();
        let name = identity
            .display_name
            .clone()
            .or_else(|| identity.email.clone())
            .unwrap_or_else(|| "Unknown".to_string());
        sqlx::query(
            "INSERT INTO person (id, profile_id, display_name, primary_email, is_human, human_override, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
        )
        .bind(&new_id)
        .bind(&profile_id)
        .bind(&name)
        .bind(&identity.email)
        .bind(is_human)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create person: {}", e))?;
        sqlx::query("UPDATE person_identity SET manual = 1 WHERE person_id = ?1")
            .bind(&identity.person_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to pin identities: {}", e))?;
        sqlx::query("UPDATE person_identity SET person_id = ?1, manual = 1 WHERE id = ?2")
            .bind(&new_id)
            .bind(identity_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to move identity: {}", e))?;
        if let Some(user_id) = &identity.user_id {
            sqlx::query(
                r#"UPDATE conversation SET person_id = ?1, updated_at = ?3
                   WHERE person_id = ?2 AND id IN (SELECT conversation_id FROM conversation_member WHERE user_id = ?4)"#,
            )
            .bind(&new_id)
            .bind(&identity.person_id)
            .bind(&now)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to move conversations: {}", e))?;
        }
        sqlx::query("UPDATE person SET human_override = 1, updated_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(&identity.person_id)
            .execute(pool)
            .await
            .ok();
        Ok(new_id)
    }

    pub async fn put_favourites(pool: &Pool<Sqlite>, profile_id: &str, order: &[FavouriteOrderItem]) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE person SET favourite_position = NULL, updated_at = ?1 WHERE profile_id = ?2")
            .bind(&now)
            .bind(profile_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to clear favourites: {}", e))?;
        sqlx::query("UPDATE conversation SET favourite_position = NULL, updated_at = ?1 WHERE favourite_position IS NOT NULL")
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to clear favourites: {}", e))?;
        for (position, item) in order.iter().enumerate() {
            let table = match item.kind.as_str() {
                "person" => "person",
                "conversation" => "conversation",
                other => return Err(format!("Unknown favourite kind: {}", other)),
            };
            sqlx::query(&format!("UPDATE {} SET favourite_position = ?1, updated_at = ?2 WHERE id = ?3", table))
                .bind(position as i64)
                .bind(&now)
                .bind(&item.id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to order favourites: {}", e))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonSignature {
    pub id: String,
    pub person_id: String,
    pub hash: String,
    pub text: String,
    pub sender_email: Option<String>,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    pub first_seen: String,
    pub last_seen: String,
    pub use_count: i64,
}

impl PersonSignature {
    pub async fn get(pool: &Pool<Sqlite>, profile_id: &str, person_id: &str) -> Result<Vec<Self>, String> {
        let rows = sqlx::query("SELECT s.*, p.name AS provider_name FROM person_signature s LEFT JOIN provider p ON p.id = s.provider_id WHERE s.profile_id = ?1 AND s.person_id = ?2 ORDER BY s.last_seen DESC, s.use_count DESC")
            .bind(profile_id)
            .bind(person_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read signatures: {}", e))?;
        Ok(rows
            .iter()
            .map(|row| Self {
                id: row.get("id"),
                person_id: row.get("person_id"),
                hash: row.get("hash"),
                text: row.get("text"),
                sender_email: row.get("sender_email"),
                provider_id: row.get("provider_id"),
                provider_name: row.get("provider_name"),
                conversation_id: row.get("conversation_id"),
                message_id: row.get("message_id"),
                first_seen: row.get("first_seen"),
                last_seen: row.get("last_seen"),
                use_count: row.get("use_count"),
            })
            .collect())
    }

    pub async fn put(pool: &Pool<Sqlite>, profile_id: &str, signature: &Self) -> Result<(), String> {
        sqlx::query(
            r#"INSERT INTO person_signature (id, profile_id, person_id, hash, text, sender_email, provider_id, conversation_id, message_id, first_seen, last_seen, use_count)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
               ON CONFLICT(profile_id, person_id, hash) DO UPDATE SET
                 text = excluded.text,
                 sender_email = excluded.sender_email,
                 provider_id = excluded.provider_id,
                 conversation_id = excluded.conversation_id,
                 message_id = excluded.message_id,
                 first_seen = MIN(person_signature.first_seen, excluded.first_seen),
                 last_seen = MAX(person_signature.last_seen, excluded.last_seen),
                 use_count = MAX(person_signature.use_count, excluded.use_count)"#,
        )
        .bind(&signature.id)
        .bind(profile_id)
        .bind(&signature.person_id)
        .bind(&signature.hash)
        .bind(&signature.text)
        .bind(&signature.sender_email)
        .bind(&signature.provider_id)
        .bind(&signature.conversation_id)
        .bind(&signature.message_id)
        .bind(&signature.first_seen)
        .bind(&signature.last_seen)
        .bind(signature.use_count)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to keep the signature: {}", e))?;
        Ok(())
    }
}
