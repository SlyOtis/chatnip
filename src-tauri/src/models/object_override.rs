use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::collections::BTreeMap;

use crate::models::tag::{OBJECT_CONVERSATION, OBJECT_PERSON};

pub const SCOPE_NONE: &str = "";
pub const SCOPE_CONVERSATION: &str = "conversation";

pub const FIELD_DISPLAY_NAME: &str = "display_name";
pub const FIELD_NAME: &str = "name";
pub const FIELD_AVATAR: &str = "avatar_data";
pub const FIELD_COLOR: &str = "color";
pub const FIELD_JOB_TITLE: &str = "job_title";
pub const FIELD_COMPANY: &str = "company";
pub const FIELD_NOTES: &str = "notes";
pub const FIELD_DESCRIPTION: &str = "description";

pub const PERSON_FIELDS: [&str; 6] = [FIELD_DISPLAY_NAME, FIELD_AVATAR, FIELD_COLOR, FIELD_JOB_TITLE, FIELD_COMPANY, FIELD_NOTES];
pub const PERSON_SCOPED_FIELDS: [&str; 1] = [FIELD_DISPLAY_NAME];
pub const CONVERSATION_FIELDS: [&str; 4] = [FIELD_NAME, FIELD_AVATAR, FIELD_COLOR, FIELD_DESCRIPTION];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObjectOverride {
    pub object_type: String,
    pub object_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub field: String,
    pub value: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutObjectOverrideInput {
    pub object_type: String,
    pub object_id: String,
    #[serde(default)]
    pub scope_type: Option<String>,
    #[serde(default)]
    pub scope_id: Option<String>,
    pub field: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DeleteObjectOverrideInput {
    pub object_type: String,
    pub object_id: String,
    #[serde(default)]
    pub scope_type: Option<String>,
    #[serde(default)]
    pub scope_id: Option<String>,
    #[serde(default)]
    pub field: Option<String>,
}

pub fn resolve<'a>(provider: &'a str, local: Option<&'a str>, scoped: Option<&'a str>) -> &'a str {
    let present = |value: Option<&'a str>| value.filter(|v| !v.trim().is_empty());
    present(scoped).or(present(local)).unwrap_or(provider)
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS object_override (
            object_type TEXT NOT NULL,
            object_id TEXT NOT NULL,
            scope_type TEXT NOT NULL DEFAULT '',
            scope_id TEXT NOT NULL DEFAULT '',
            field TEXT NOT NULL,
            value TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (object_type, object_id, scope_type, scope_id, field)
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_object_override_scope ON object_override (scope_type, scope_id)").execute(pool).await?;
    Ok(())
}

fn allowed(object_type: &str, scope_type: &str, field: &str) -> Result<(), String> {
    let fields: &[&str] = match (object_type, scope_type) {
        (OBJECT_PERSON, SCOPE_NONE) => &PERSON_FIELDS,
        (OBJECT_PERSON, SCOPE_CONVERSATION) => &PERSON_SCOPED_FIELDS,
        (OBJECT_CONVERSATION, SCOPE_NONE) => &CONVERSATION_FIELDS,
        _ => return Err(format!("A {} cannot be overridden within a {}", object_type, if scope_type.is_empty() { "profile" } else { scope_type })),
    };
    if fields.contains(&field) {
        Ok(())
    } else {
        Err(format!("{} is not a local field of a {}; use one of {}", field, object_type, fields.join(", ")))
    }
}

impl ObjectOverride {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        Self {
            object_type: row.get("object_type"),
            object_id: row.get("object_id"),
            scope_type: row.get("scope_type"),
            scope_id: row.get("scope_id"),
            field: row.get("field"),
            value: row.get("value"),
            updated_at: row.get("updated_at"),
        }
    }

    pub async fn get(pool: &Pool<Sqlite>, object_type: &str, object_id: &str) -> Result<Vec<ObjectOverride>, String> {
        let rows = sqlx::query("SELECT * FROM object_override WHERE object_type = ?1 AND object_id = ?2 ORDER BY scope_type, scope_id, field")
            .bind(object_type)
            .bind(object_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load local overrides: {}", e))?;
        Ok(rows.into_iter().map(Self::from_row).collect())
    }

    pub async fn get_all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<ObjectOverride>, String> {
        let rows = sqlx::query(&format!(
            "SELECT o.* FROM object_override o WHERE (o.object_type = ?2 AND o.object_id IN (SELECT id FROM person WHERE profile_id = ?1))
             OR (o.object_type = ?3 AND o.object_id IN (SELECT c.id FROM conversation c WHERE {})) ORDER BY o.object_type, o.object_id, o.scope_type, o.scope_id, o.field",
            crate::models::OWNED_BY_PROFILE
        ))
        .bind(profile_id)
        .bind(OBJECT_PERSON)
        .bind(OBJECT_CONVERSATION)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load local overrides: {}", e))?;
        Ok(rows.into_iter().map(Self::from_row).collect())
    }

    pub async fn get_fields(pool: &Pool<Sqlite>, object_type: &str, object_id: &str) -> Result<BTreeMap<String, String>, String> {
        Ok(Self::get(pool, object_type, object_id).await?.into_iter().filter(|o| o.scope_type.is_empty()).map(|o| (o.field, o.value)).collect())
    }

    pub async fn get_nicknames(pool: &Pool<Sqlite>, conversation_id: &str) -> Result<BTreeMap<String, String>, String> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT pi.user_id, o.value FROM object_override o JOIN person_identity pi ON pi.person_id = o.object_id
             WHERE o.object_type = ?1 AND o.scope_type = ?2 AND o.scope_id = ?3 AND o.field = ?4 AND pi.user_id IS NOT NULL",
        )
        .bind(OBJECT_PERSON)
        .bind(SCOPE_CONVERSATION)
        .bind(conversation_id)
        .bind(FIELD_DISPLAY_NAME)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load nicknames: {}", e))?;
        Ok(rows.into_iter().collect())
    }

    pub async fn get_name(pool: &Pool<Sqlite>, user_id: &str, conversation_id: Option<&str>, provider: &str) -> Result<String, String> {
        let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT
                (SELECT o.value FROM object_override o WHERE o.object_type = ?1 AND o.object_id = pi.person_id AND o.scope_type = '' AND o.field = ?2),
                (SELECT o.value FROM object_override o WHERE o.object_type = ?1 AND o.object_id = pi.person_id AND o.scope_type = ?3 AND o.scope_id = ?4 AND o.field = ?2)
             FROM person_identity pi WHERE pi.user_id = ?5 LIMIT 1",
        )
        .bind(OBJECT_PERSON)
        .bind(FIELD_DISPLAY_NAME)
        .bind(SCOPE_CONVERSATION)
        .bind(conversation_id.unwrap_or_default())
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to resolve the name: {}", e))?;
        let (local, scoped) = row.unwrap_or((None, None));
        Ok(resolve(provider, local.as_deref(), scoped.as_deref()).to_string())
    }

    pub async fn put(pool: &Pool<Sqlite>, input: &PutObjectOverrideInput) -> Result<(), String> {
        let scope_type = input.scope_type.as_deref().unwrap_or(SCOPE_NONE);
        let scope_id = if scope_type.is_empty() { "" } else { input.scope_id.as_deref().unwrap_or_default() };
        allowed(&input.object_type, scope_type, &input.field)?;
        if !scope_type.is_empty() && scope_id.is_empty() {
            return Err(format!("A {} override needs the {} it applies to", scope_type, scope_type));
        }
        let value = input.value.as_deref().map(str::trim).unwrap_or_default();
        if value.is_empty() {
            return Self::delete(pool, &DeleteObjectOverrideInput { object_type: input.object_type.clone(), object_id: input.object_id.clone(), scope_type: Some(scope_type.to_string()), scope_id: Some(scope_id.to_string()), field: Some(input.field.clone()) }).await;
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO object_override (object_type, object_id, scope_type, scope_id, field, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT(object_type, object_id, scope_type, scope_id, field) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(&input.object_type)
        .bind(&input.object_id)
        .bind(scope_type)
        .bind(scope_id)
        .bind(&input.field)
        .bind(value)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to keep the local {}: {}", input.field, e))?;
        Ok(())
    }

    pub async fn delete(pool: &Pool<Sqlite>, input: &DeleteObjectOverrideInput) -> Result<(), String> {
        let scope_type = input.scope_type.as_deref().unwrap_or(SCOPE_NONE);
        let scope_id = input.scope_id.as_deref().unwrap_or_default();
        sqlx::query("DELETE FROM object_override WHERE object_type = ?1 AND object_id = ?2 AND scope_type = ?3 AND scope_id = ?4 AND (?5 IS NULL OR field = ?5)")
            .bind(&input.object_type)
            .bind(&input.object_id)
            .bind(scope_type)
            .bind(scope_id)
            .bind(&input.field)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to reset the local value: {}", e))?;
        Ok(())
    }

    pub async fn put_object_id(pool: &Pool<Sqlite>, object_type: &str, from: &str, to: &str) -> Result<(), String> {
        sqlx::query("UPDATE OR IGNORE object_override SET object_id = ?3 WHERE object_type = ?1 AND object_id = ?2")
            .bind(object_type)
            .bind(from)
            .bind(to)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to move the local values: {}", e))?;
        sqlx::query("DELETE FROM object_override WHERE object_type = ?1 AND object_id = ?2")
            .bind(object_type)
            .bind(from)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to clear the local values: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_conversation, test_pool, test_profile, test_provider};
    use uuid::Uuid;

    async fn owned(pool: &Pool<Sqlite>, provider: &str, name: &str) -> String {
        let id = test_conversation(pool, name, "group_message").await;
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)").bind(Uuid::new_v4().to_string()).bind(&id).bind(provider).bind(Uuid::new_v4().to_string()).bind(Utc::now().to_rfc3339()).execute(pool).await.unwrap();
        id
    }

    fn put(object_type: &str, object_id: &str, scope: Option<&str>, field: &str, value: &str) -> PutObjectOverrideInput {
        PutObjectOverrideInput {
            object_type: object_type.into(),
            object_id: object_id.into(),
            scope_type: scope.map(|_| SCOPE_CONVERSATION.to_string()),
            scope_id: scope.map(String::from),
            field: field.into(),
            value: Some(value.into()),
        }
    }

    #[test]
    fn a_chat_nickname_wins_over_the_local_name_which_wins_over_the_provider() {
        assert_eq!(resolve("Ingrid Hansen", None, None), "Ingrid Hansen");
        assert_eq!(resolve("Ingrid Hansen", Some("Ingrid"), None), "Ingrid");
        assert_eq!(resolve("Ingrid Hansen", Some("Ingrid"), Some("Inga")), "Inga");
        assert_eq!(resolve("Ingrid Hansen", None, Some("Inga")), "Inga");
        assert_eq!(resolve("Ingrid Hansen", Some("  "), Some("")), "Ingrid Hansen");
    }

    #[tokio::test]
    async fn names_resolve_per_chat_and_never_touch_the_provider_rows() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "slack").await;
        let chat = owned(&pool, &provider, "Design").await;
        let other = owned(&pool, &provider, "Sales").await;
        let now = Utc::now().to_rfc3339();
        let user = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO user (id, display_name, first_seen, last_updated) VALUES (?1, 'Ingrid Hansen', ?2, ?2)").bind(&user).bind(&now).execute(&pool).await.unwrap();
        let person = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO person (id, profile_id, display_name, created_at, updated_at) VALUES (?1, ?2, 'Ingrid Hansen', ?3, ?3)").bind(&person).bind(&profile).bind(&now).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO person_identity (id, person_id, user_id, provider_type, external_id) VALUES (?1, ?2, ?3, 'slack', 'U1')").bind(Uuid::new_v4().to_string()).bind(&person).bind(&user).execute(&pool).await.unwrap();

        assert_eq!(ObjectOverride::get_name(&pool, &user, Some(&chat), "Ingrid Hansen").await.unwrap(), "Ingrid Hansen");
        ObjectOverride::put(&pool, &put(OBJECT_PERSON, &person, None, FIELD_DISPLAY_NAME, "Ingrid")).await.unwrap();
        assert_eq!(ObjectOverride::get_name(&pool, &user, Some(&chat), "Ingrid Hansen").await.unwrap(), "Ingrid");
        ObjectOverride::put(&pool, &put(OBJECT_PERSON, &person, Some(&chat), FIELD_DISPLAY_NAME, "Inga")).await.unwrap();
        assert_eq!(ObjectOverride::get_name(&pool, &user, Some(&chat), "Ingrid Hansen").await.unwrap(), "Inga");
        assert_eq!(ObjectOverride::get_name(&pool, &user, Some(&other), "Ingrid Hansen").await.unwrap(), "Ingrid", "the nickname stays in its chat");
        assert_eq!(ObjectOverride::get_nicknames(&pool, &chat).await.unwrap().get(&user).map(String::as_str), Some("Inga"));
        assert!(ObjectOverride::get_nicknames(&pool, &other).await.unwrap().is_empty());

        ObjectOverride::delete(&pool, &DeleteObjectOverrideInput { object_type: OBJECT_PERSON.into(), object_id: person.clone(), scope_type: Some(SCOPE_CONVERSATION.into()), scope_id: Some(chat.clone()), field: Some(FIELD_DISPLAY_NAME.into()) }).await.unwrap();
        assert_eq!(ObjectOverride::get_name(&pool, &user, Some(&chat), "Ingrid Hansen").await.unwrap(), "Ingrid");
        ObjectOverride::put(&pool, &put(OBJECT_PERSON, &person, None, FIELD_DISPLAY_NAME, "")).await.unwrap();
        assert_eq!(ObjectOverride::get_name(&pool, &user, Some(&chat), "Ingrid Hansen").await.unwrap(), "Ingrid Hansen", "an empty value resets the field");

        let names: (String, String) = sqlx::query_as("SELECT u.display_name, p.display_name FROM user u, person p WHERE u.id = ?1 AND p.id = ?2").bind(&user).bind(&person).fetch_one(&pool).await.unwrap();
        assert_eq!(names, ("Ingrid Hansen".to_string(), "Ingrid Hansen".to_string()));
    }

    #[tokio::test]
    async fn only_known_fields_are_kept_and_listing_stays_in_the_profile() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "slack").await;
        let chat = owned(&pool, &provider, "Design").await;
        assert!(ObjectOverride::put(&pool, &put(OBJECT_CONVERSATION, &chat, None, "external_id", "x")).await.is_err());
        assert!(ObjectOverride::put(&pool, &put(OBJECT_CONVERSATION, &chat, Some(&chat), FIELD_NAME, "x")).await.is_err());
        assert!(ObjectOverride::put(&pool, &put(OBJECT_PERSON, "p", Some(&chat), FIELD_AVATAR, "x")).await.is_err());
        ObjectOverride::put(&pool, &put(OBJECT_CONVERSATION, &chat, None, FIELD_NAME, "Designers")).await.unwrap();
        ObjectOverride::put(&pool, &put(OBJECT_CONVERSATION, &chat, None, FIELD_NAME, "Design crew")).await.unwrap();
        let all = ObjectOverride::get_all(&pool, &profile).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].value, "Design crew");
        let elsewhere = test_profile(&pool).await;
        assert!(ObjectOverride::get_all(&pool, &elsewhere).await.unwrap().is_empty());
    }
}
