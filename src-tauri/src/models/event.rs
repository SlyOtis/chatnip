use crate::providers::traits::RemoteEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

pub const STATUS_PROPOSED: &str = "proposed";
pub const STATUS_CONFIRMED: &str = "confirmed";
pub const STATUS_DISMISSED: &str = "dismissed";
pub const ORIGIN_MANUAL: &str = "manual";
pub const ORIGIN_PARSED: &str = "parsed";
pub const ORIGIN_SYNCED: &str = "synced";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub profile_id: String,
    pub title: String,
    pub notes: String,
    pub location: String,
    pub starts_at: String,
    pub ends_at: String,
    pub all_day: bool,
    pub status: String,
    pub origin: String,
    pub confidence: f64,
    pub source_message_id: Option<String>,
    pub conversation_id: Option<String>,
    pub attendees: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub provider_id: Option<String>,
    pub calendar_id: Option<String>,
    pub external_id: Option<String>,
    pub ical_uid: Option<String>,
    pub online_meeting_url: Option<String>,
    pub organizer: Option<String>,
    pub response_status: Option<String>,
    pub link_id: Option<String>,
    pub unmerged: bool,
    #[serde(default)]
    pub sources: Vec<EventSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventSource {
    pub event_id: String,
    pub calendar_id: Option<String>,
    pub calendar_name: String,
    pub colour: Option<String>,
    pub provider_id: Option<String>,
    pub provider_type: Option<String>,
    pub provider_external_id: Option<String>,
    pub response_status: Option<String>,
    pub online_meeting_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventDraft {
    pub title: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub location: String,
    pub starts_at: String,
    #[serde(default)]
    pub ends_at: Option<String>,
    #[serde(default)]
    pub all_day: bool,
    #[serde(default)]
    pub attendees: Vec<String>,
}

impl Event {
    pub fn from_row(row: sqlx::sqlite::SqliteRow) -> Event {
        let attendees: String = row.try_get("attendees").unwrap_or_else(|_| "[]".to_string());
        Event {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            title: row.get("title"),
            notes: row.try_get("notes").unwrap_or_default(),
            location: row.try_get("location").unwrap_or_default(),
            starts_at: row.get("starts_at"),
            ends_at: row.get("ends_at"),
            all_day: row.try_get("all_day").unwrap_or(false),
            status: row.get("status"),
            origin: row.get("origin"),
            confidence: row.try_get("confidence").unwrap_or(1.0),
            source_message_id: row.try_get("source_message_id").ok().flatten(),
            conversation_id: row.try_get("conversation_id").ok().flatten(),
            attendees: serde_json::from_str(&attendees).unwrap_or_default(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            provider_id: row.try_get("provider_id").ok().flatten(),
            calendar_id: row.try_get("calendar_id").ok().flatten(),
            external_id: row.try_get("external_id").ok().flatten(),
            ical_uid: row.try_get("ical_uid").ok().flatten(),
            online_meeting_url: row.try_get("online_meeting_url").ok().flatten(),
            organizer: row.try_get("organizer").ok().flatten(),
            response_status: row.try_get("response_status").ok().flatten(),
            link_id: row.try_get("link_id").ok().flatten(),
            unmerged: row.try_get("unmerged").unwrap_or(false),
            sources: Vec::new(),
        }
    }

    pub fn ends_at_or_default(draft: &EventDraft) -> Result<(DateTime<Utc>, DateTime<Utc>), String> {
        let starts = DateTime::parse_from_rfc3339(&draft.starts_at)
            .map_err(|_| format!("{} is not a valid start time", draft.starts_at))?
            .with_timezone(&Utc);
        let ends = match &draft.ends_at {
            Some(e) if !e.trim().is_empty() => DateTime::parse_from_rfc3339(e).map_err(|_| format!("{} is not a valid end time", e))?.with_timezone(&Utc),
            _ => starts + if draft.all_day { chrono::Duration::days(1) } else { chrono::Duration::hours(1) },
        };
        if ends < starts {
            return Err("The event ends before it starts".to_string());
        }
        Ok((starts, ends))
    }

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, draft: &EventDraft, origin: &str, status: &str, confidence: f64, source_message_id: Option<&str>, conversation_id: Option<&str>) -> Result<Event, String> {
        if draft.title.trim().is_empty() {
            return Err("An event needs a title".to_string());
        }
        let (starts, ends) = Self::ends_at_or_default(draft)?;
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO event (id, profile_id, title, notes, location, starts_at, ends_at, all_day, status, origin, confidence, source_message_id, conversation_id, attendees, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
        )
        .bind(&id)
        .bind(profile_id)
        .bind(draft.title.trim())
        .bind(&draft.notes)
        .bind(&draft.location)
        .bind(starts.to_rfc3339())
        .bind(ends.to_rfc3339())
        .bind(draft.all_day)
        .bind(status)
        .bind(origin)
        .bind(confidence)
        .bind(source_message_id)
        .bind(conversation_id)
        .bind(serde_json::to_string(&draft.attendees).unwrap_or_else(|_| "[]".to_string()))
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create event: {}", e))?;
        Event::get(pool, &id).await
    }

    pub async fn get(pool: &Pool<Sqlite>, id: &str) -> Result<Event, String> {
        let row = sqlx::query("SELECT * FROM event WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load event: {}", e))?
            .ok_or_else(|| "Event not found".to_string())?;
        Ok(Event::from_row(row))
    }

    pub async fn between(pool: &Pool<Sqlite>, profile_id: &str, start: &str, end: &str, include_dismissed: bool) -> Result<Vec<Event>, String> {
        let rows = sqlx::query(
            "SELECT * FROM event WHERE profile_id = ?1 AND ends_at >= ?2 AND starts_at <= ?3 AND (?4 OR status != 'dismissed') AND (calendar_id IS NULL OR calendar_id IN (SELECT id FROM calendar WHERE enabled = 1)) ORDER BY starts_at ASC",
        )
        .bind(profile_id)
        .bind(start)
        .bind(end)
        .bind(include_dismissed)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list events: {}", e))?;
        Ok(rows.into_iter().map(Event::from_row).collect())
    }

    pub async fn proposed(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Event>, String> {
        let rows = sqlx::query("SELECT * FROM event WHERE profile_id = ?1 AND status = ?2 ORDER BY starts_at ASC")
            .bind(profile_id)
            .bind(STATUS_PROPOSED)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list proposed events: {}", e))?;
        Ok(rows.into_iter().map(Event::from_row).collect())
    }

    pub async fn put(pool: &Pool<Sqlite>, id: &str, draft: Option<&EventDraft>, status: Option<&str>) -> Result<Event, String> {
        let current = Event::get(pool, id).await?;
        let (title, notes, location, starts, ends, all_day, attendees) = match draft {
            Some(d) => {
                let (s, e) = Self::ends_at_or_default(d)?;
                (d.title.trim().to_string(), d.notes.clone(), d.location.clone(), s.to_rfc3339(), e.to_rfc3339(), d.all_day, serde_json::to_string(&d.attendees).unwrap_or_else(|_| "[]".to_string()))
            }
            None => (current.title.clone(), current.notes.clone(), current.location.clone(), current.starts_at.clone(), current.ends_at.clone(), current.all_day, serde_json::to_string(&current.attendees).unwrap_or_else(|_| "[]".to_string())),
        };
        sqlx::query("UPDATE event SET title = ?1, notes = ?2, location = ?3, starts_at = ?4, ends_at = ?5, all_day = ?6, attendees = ?7, status = ?8, updated_at = ?9 WHERE id = ?10")
            .bind(&title)
            .bind(&notes)
            .bind(&location)
            .bind(&starts)
            .bind(&ends)
            .bind(all_day)
            .bind(&attendees)
            .bind(status.unwrap_or(&current.status))
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update event: {}", e))?;
        Event::get(pool, id).await
    }

    pub async fn delete(pool: &Pool<Sqlite>, id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM event WHERE id = ?1").bind(id).execute(pool).await.map_err(|e| format!("Failed to delete event: {}", e))?;
        Ok(())
    }


    pub async fn put_remote(pool: &Pool<Sqlite>, profile_id: &str, provider_id: &str, calendar_id: &str, remote: &RemoteEvent) -> Result<Option<String>, String> {
        if remote.cancelled {
            sqlx::query("DELETE FROM event WHERE calendar_id = ?1 AND external_id = ?2")
                .bind(calendar_id)
                .bind(&remote.external_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to drop a cancelled event: {}", e))?;
            return Ok(None);
        }
        let now = Utc::now().to_rfc3339();
        let existing: Option<String> = sqlx::query_scalar("SELECT id FROM event WHERE calendar_id = ?1 AND external_id = ?2")
            .bind(calendar_id)
            .bind(&remote.external_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to look up an event: {}", e))?;
        let id = existing.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        let attendees = serde_json::to_string(&remote.attendees).unwrap_or_else(|_| "[]".to_string());
        if existing.is_some() {
            sqlx::query(
                "UPDATE event SET title = ?1, notes = ?2, location = ?3, starts_at = ?4, ends_at = ?5, all_day = ?6, attendees = ?7, ical_uid = ?8, online_meeting_url = ?9, organizer = ?10, response_status = ?11, remote_updated_at = ?12, updated_at = ?13 WHERE id = ?14",
            )
            .bind(&remote.title)
            .bind(&remote.notes)
            .bind(&remote.location)
            .bind(remote.starts_at.to_rfc3339())
            .bind(remote.ends_at.to_rfc3339())
            .bind(remote.all_day)
            .bind(&attendees)
            .bind(&remote.ical_uid)
            .bind(&remote.online_meeting_url)
            .bind(&remote.organizer)
            .bind(&remote.response_status)
            .bind(remote.updated_at.to_rfc3339())
            .bind(&now)
            .bind(&id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update an event: {}", e))?;
        } else {
            sqlx::query(
                "INSERT INTO event (id, profile_id, title, notes, location, starts_at, ends_at, all_day, status, origin, confidence, attendees, provider_id, calendar_id, external_id, ical_uid, online_meeting_url, organizer, response_status, remote_updated_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?20)",
            )
            .bind(&id)
            .bind(profile_id)
            .bind(&remote.title)
            .bind(&remote.notes)
            .bind(&remote.location)
            .bind(remote.starts_at.to_rfc3339())
            .bind(remote.ends_at.to_rfc3339())
            .bind(remote.all_day)
            .bind(STATUS_CONFIRMED)
            .bind(ORIGIN_SYNCED)
            .bind(&attendees)
            .bind(provider_id)
            .bind(calendar_id)
            .bind(&remote.external_id)
            .bind(&remote.ical_uid)
            .bind(&remote.online_meeting_url)
            .bind(&remote.organizer)
            .bind(&remote.response_status)
            .bind(remote.updated_at.to_rfc3339())
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to save an event: {}", e))?;
        }
        Ok(Some(id))
    }

    pub async fn prune_remote(pool: &Pool<Sqlite>, calendar_id: &str, start: &str, end: &str, keep: &[String]) -> Result<usize, String> {
        let rows = sqlx::query("SELECT id, external_id FROM event WHERE calendar_id = ?1 AND external_id IS NOT NULL AND ends_at >= ?2 AND starts_at <= ?3")
            .bind(calendar_id)
            .bind(start)
            .bind(end)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list synced events: {}", e))?;
        let mut removed = 0;
        for row in rows {
            let external: String = row.get("external_id");
            if keep.contains(&external) {
                continue;
            }
            Event::delete(pool, &row.get::<String, _>("id")).await?;
            removed += 1;
        }
        Ok(removed)
    }

    pub async fn put_link(pool: &Pool<Sqlite>, ids: &[String], link_id: Option<&str>, unmerged: bool) -> Result<(), String> {
        for id in ids {
            sqlx::query("UPDATE event SET link_id = ?1, unmerged = ?2, updated_at = ?3 WHERE id = ?4")
                .bind(link_id)
                .bind(unmerged)
                .bind(Utc::now().to_rfc3339())
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to link events: {}", e))?;
        }
        Ok(())
    }

    pub async fn exists_for_message(pool: &Pool<Sqlite>, message_id: &str, starts_at: &str) -> Result<bool, String> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event WHERE source_message_id = ?1 AND starts_at = ?2")
            .bind(message_id)
            .bind(starts_at)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to check events: {}", e))?;
        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_pool, test_profile};

    #[tokio::test]
    async fn events_round_trip_and_default_their_end() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let draft = EventDraft { title: " Lunch ".into(), starts_at: "2026-10-10T11:00:00Z".into(), ..Default::default() };
        let created = Event::post(&pool, &profile, &draft, ORIGIN_MANUAL, STATUS_CONFIRMED, 1.0, None, None).await.unwrap();
        assert_eq!(created.title, "Lunch");
        assert_eq!(created.ends_at, "2026-10-10T12:00:00+00:00");
        let day = Event::between(&pool, &profile, "2026-10-10T00:00:00Z", "2026-10-10T23:59:59Z", false).await.unwrap();
        assert_eq!(day.len(), 1);
        let dismissed = Event::put(&pool, &created.id, None, Some(STATUS_DISMISSED)).await.unwrap();
        assert_eq!(dismissed.status, STATUS_DISMISSED);
        assert!(Event::between(&pool, &profile, "2026-10-10T00:00:00Z", "2026-10-10T23:59:59Z", false).await.unwrap().is_empty());
        Event::delete(&pool, &created.id).await.unwrap();
        assert!(Event::get(&pool, &created.id).await.is_err());
    }

    #[tokio::test]
    async fn an_event_cannot_end_before_it_starts() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let draft = EventDraft { title: "Backwards".into(), starts_at: "2026-10-10T11:00:00Z".into(), ends_at: Some("2026-10-10T10:00:00Z".into()), ..Default::default() };
        assert!(Event::post(&pool, &profile, &draft, ORIGIN_MANUAL, STATUS_CONFIRMED, 1.0, None, None).await.is_err());
    }
}
