use crate::providers::traits::RemoteCalendar;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Calendar {
    pub id: String,
    pub profile_id: String,
    pub provider_id: String,
    pub provider_type: String,
    pub provider_name: String,
    pub provider_external_id: Option<String>,
    pub external_id: String,
    pub name: String,
    pub colour: Option<String>,
    pub is_primary: bool,
    pub can_write: bool,
    pub enabled: bool,
}

const SELECT: &str = "SELECT c.*, p.provider_type, p.name AS provider_name, p.external_id AS provider_external_id FROM calendar c JOIN provider p ON p.id = c.provider_id";

impl Calendar {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> Calendar {
        Calendar {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            provider_id: row.get("provider_id"),
            provider_type: row.try_get("provider_type").unwrap_or_default(),
            provider_name: row.try_get("provider_name").unwrap_or_default(),
            provider_external_id: row.try_get("provider_external_id").ok().flatten(),
            external_id: row.get("external_id"),
            name: row.get("name"),
            colour: row.try_get("colour").ok().flatten(),
            is_primary: row.try_get("is_primary").unwrap_or(false),
            can_write: row.try_get("can_write").unwrap_or(false),
            enabled: row.try_get("enabled").unwrap_or(true),
        }
    }

    pub async fn put_remote(pool: &Pool<Sqlite>, profile_id: &str, provider_id: &str, remote: &RemoteCalendar) -> Result<Calendar, String> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO calendar (id, profile_id, provider_id, external_id, name, colour, is_primary, can_write, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)
             ON CONFLICT (provider_id, external_id) DO UPDATE SET name = excluded.name, colour = excluded.colour, is_primary = excluded.is_primary, can_write = excluded.can_write, updated_at = excluded.updated_at",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(profile_id)
        .bind(provider_id)
        .bind(&remote.external_id)
        .bind(&remote.name)
        .bind(&remote.colour)
        .bind(remote.is_primary)
        .bind(remote.can_write)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to save calendar {}: {}", remote.name, e))?;
        let row = sqlx::query(&format!("{} WHERE c.provider_id = ?1 AND c.external_id = ?2", SELECT))
            .bind(provider_id)
            .bind(&remote.external_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to load calendar {}: {}", remote.name, e))?;
        Ok(Calendar::from_row(row))
    }

    pub async fn for_profile(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Calendar>, String> {
        let rows = sqlx::query(&format!("{} WHERE c.profile_id = ?1 ORDER BY p.name, c.is_primary DESC, c.name", SELECT))
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list calendars: {}", e))?;
        Ok(rows.into_iter().map(Calendar::from_row).collect())
    }

    pub async fn get(pool: &Pool<Sqlite>, id: &str) -> Result<Calendar, String> {
        let row = sqlx::query(&format!("{} WHERE c.id = ?1", SELECT))
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load calendar: {}", e))?
            .ok_or_else(|| "Calendar not found".to_string())?;
        Ok(Calendar::from_row(row))
    }

    pub async fn put_enabled(pool: &Pool<Sqlite>, id: &str, enabled: bool) -> Result<Calendar, String> {
        sqlx::query("UPDATE calendar SET enabled = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(enabled)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update calendar: {}", e))?;
        Calendar::get(pool, id).await
    }

    pub async fn prune(pool: &Pool<Sqlite>, provider_id: &str, keep: &[String]) -> Result<usize, String> {
        let rows = sqlx::query("SELECT id, external_id FROM calendar WHERE provider_id = ?1")
            .bind(provider_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list calendars: {}", e))?;
        let mut removed = 0;
        for row in rows {
            let external: String = row.get("external_id");
            if keep.contains(&external) {
                continue;
            }
            let id: String = row.get("id");
            sqlx::query("DELETE FROM event WHERE calendar_id = ?1").bind(&id).execute(pool).await.map_err(|e| format!("Failed to drop events: {}", e))?;
            sqlx::query("DELETE FROM calendar WHERE id = ?1").bind(&id).execute(pool).await.map_err(|e| format!("Failed to drop calendar: {}", e))?;
            removed += 1;
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_pool, test_profile, test_provider};

    fn remote(id: &str, name: &str) -> RemoteCalendar {
        RemoteCalendar { external_id: id.into(), name: name.into(), colour: Some("#0f6cbd".into()), is_primary: true, can_write: true }
    }

    #[tokio::test]
    async fn calendars_upsert_keep_their_toggle_and_prune() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "microsoft").await;
        let first = Calendar::put_remote(&pool, &profile, &provider, &remote("a", "Work")).await.unwrap();
        assert_eq!(first.provider_type, "microsoft");
        Calendar::put_enabled(&pool, &first.id, false).await.unwrap();
        let again = Calendar::put_remote(&pool, &profile, &provider, &remote("a", "Work renamed")).await.unwrap();
        assert_eq!(again.id, first.id);
        assert_eq!(again.name, "Work renamed");
        assert!(!again.enabled);
        Calendar::put_remote(&pool, &profile, &provider, &remote("b", "Other")).await.unwrap();
        assert_eq!(Calendar::prune(&pool, &provider, &["a".to_string()]).await.unwrap(), 1);
        assert_eq!(Calendar::for_profile(&pool, &profile).await.unwrap().len(), 1);
    }
}
