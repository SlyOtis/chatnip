use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

pub const KIND_PROVIDER: &str = "provider";
pub const KIND_PERSON: &str = "person";
pub const KIND_CONVERSATION: &str = "conversation";
pub const KIND_TAG: &str = "tag";
pub const KIND_GROUP: &str = "group";
pub const MEMBER_KINDS: [&str; 5] = [KIND_PROVIDER, KIND_PERSON, KIND_CONVERSATION, KIND_TAG, KIND_GROUP];
pub const DEFAULT_NAME: &str = "Inbox";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct InboxMember {
    pub kind: String,
    pub member_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inbox {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub icon: Option<String>,
    pub position: i64,
    pub is_default: bool,
    pub members: Vec<InboxMember>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostInboxInput {
    pub name: String,
    pub icon: Option<String>,
    pub members: Option<Vec<InboxMember>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PutInboxInput {
    pub inbox_id: String,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub position: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PutInboxMembersInput {
    pub inbox_id: String,
    pub add: Option<Vec<InboxMember>>,
    pub remove: Option<Vec<InboxMember>>,
    pub replace: Option<Vec<InboxMember>>,
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS inbox (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            name TEXT NOT NULL,
            icon TEXT,
            position INTEGER NOT NULL DEFAULT 0,
            is_default BOOLEAN NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profile(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS inbox_member (
            inbox_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            member_id TEXT NOT NULL,
            PRIMARY KEY (inbox_id, kind, member_id),
            FOREIGN KEY (inbox_id) REFERENCES inbox(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

fn valid(member: &InboxMember) -> Result<(), String> {
    if !MEMBER_KINDS.contains(&member.kind.as_str()) {
        return Err(format!("An inbox cannot link a {}", member.kind));
    }
    if member.member_id.trim().is_empty() {
        return Err("An inbox member needs an id".to_string());
    }
    Ok(())
}

pub fn default_icon(name: &str) -> &'static str {
    match name.to_ascii_lowercase().as_str() {
        "inbox" => "lucide:inbox",
        "important" => "lucide:flame",
        "team" => "lucide:users",
        "newsletters" | "news" => "lucide:newspaper",
        "service notifications" | "notifications" => "lucide:bell-ring",
        "other" => "lucide:layers",
        "private" => "lucide:house",
        "work" => "lucide:briefcase",
        _ => "lucide:folder",
    }
}

impl Inbox {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        Self {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            name: row.get("name"),
            icon: row.try_get::<Option<String>, _>("icon").ok().flatten().filter(|icon| !icon.is_empty()),
            position: row.get("position"),
            is_default: row.get("is_default"),
            members: Vec::new(),
            created_at: row.get("created_at"),
        }
    }

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Self>, String> {
        let mut inboxes: Vec<Self> = sqlx::query("SELECT * FROM inbox WHERE profile_id = ?1 ORDER BY is_default ASC, position ASC, created_at ASC")
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load inboxes: {}", e))?
            .into_iter()
            .map(Self::from_row)
            .collect();
        let members = sqlx::query("SELECT m.inbox_id, m.kind, m.member_id FROM inbox_member m JOIN inbox i ON i.id = m.inbox_id WHERE i.profile_id = ?1 ORDER BY m.kind, m.member_id")
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load inbox members: {}", e))?;
        for row in members {
            let inbox_id: String = row.get("inbox_id");
            if let Some(inbox) = inboxes.iter_mut().find(|inbox| inbox.id == inbox_id) {
                inbox.members.push(InboxMember { kind: row.get("kind"), member_id: row.get("member_id") });
            }
        }
        Ok(inboxes)
    }

    pub async fn get(pool: &Pool<Sqlite>, inbox_id: &str) -> Result<Self, String> {
        let row = sqlx::query("SELECT * FROM inbox WHERE id = ?1")
            .bind(inbox_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load inbox: {}", e))?
            .ok_or_else(|| "Inbox not found".to_string())?;
        let inbox = Self::from_row(row);
        Self::all(pool, &inbox.profile_id).await?.into_iter().find(|candidate| candidate.id == inbox_id).ok_or_else(|| "Inbox not found".to_string())
    }

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, input: &PostInboxInput) -> Result<Self, String> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err("An inbox needs a name".to_string());
        }
        let position: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position) + 1, 0) FROM inbox WHERE profile_id = ?1 AND is_default = 0")
            .bind(profile_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to place inbox: {}", e))?;
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO inbox (id, profile_id, name, icon, position, is_default, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)")
            .bind(&id)
            .bind(profile_id)
            .bind(name)
            .bind(input.icon.as_deref().filter(|icon| !icon.is_empty()))
            .bind(position)
            .bind(Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to create inbox: {}", e))?;
        if let Some(members) = &input.members {
            Self::put_members(pool, &PutInboxMembersInput { inbox_id: id.clone(), add: Some(members.clone()), remove: None, replace: None }).await?;
        }
        Self::get(pool, &id).await
    }

    pub async fn put(pool: &Pool<Sqlite>, input: &PutInboxInput) -> Result<Self, String> {
        let current = Self::get(pool, &input.inbox_id).await?;
        let name = input.name.as_deref().map(str::trim).filter(|name| !name.is_empty()).unwrap_or(&current.name).to_string();
        let icon = match &input.icon {
            Some(icon) => Some(icon.trim().to_string()).filter(|icon| !icon.is_empty()),
            None => current.icon.clone(),
        };
        sqlx::query("UPDATE inbox SET name = ?1, icon = ?2, position = ?3 WHERE id = ?4")
            .bind(&name)
            .bind(icon)
            .bind(input.position.unwrap_or(current.position))
            .bind(&input.inbox_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update inbox: {}", e))?;
        Self::get(pool, &input.inbox_id).await
    }

    pub async fn put_order(pool: &Pool<Sqlite>, profile_id: &str, order: &[String]) -> Result<(), String> {
        for (index, id) in order.iter().enumerate() {
            sqlx::query("UPDATE inbox SET position = ?1 WHERE id = ?2 AND profile_id = ?3 AND is_default = 0")
                .bind(index as i64)
                .bind(id)
                .bind(profile_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to order inboxes: {}", e))?;
        }
        Ok(())
    }

    pub async fn delete(pool: &Pool<Sqlite>, inbox_id: &str) -> Result<(), String> {
        let inbox = Self::get(pool, inbox_id).await?;
        if inbox.is_default {
            return Err("The default Inbox cannot be deleted".to_string());
        }
        sqlx::query("DELETE FROM inbox_member WHERE inbox_id = ?1").bind(inbox_id).execute(pool).await.map_err(|e| format!("Failed to clear inbox: {}", e))?;
        sqlx::query("DELETE FROM inbox WHERE id = ?1").bind(inbox_id).execute(pool).await.map_err(|e| format!("Failed to delete inbox: {}", e))?;
        Ok(())
    }

    pub async fn put_members(pool: &Pool<Sqlite>, input: &PutInboxMembersInput) -> Result<Self, String> {
        let inbox = Self::get(pool, &input.inbox_id).await?;
        if inbox.is_default && (input.add.as_ref().is_some_and(|a| !a.is_empty()) || input.replace.as_ref().is_some_and(|r| !r.is_empty())) {
            return Err("The default Inbox holds everything no other inbox links; it has no members of its own".to_string());
        }
        for member in input.add.iter().flatten().chain(input.remove.iter().flatten()).chain(input.replace.iter().flatten()) {
            valid(member)?;
        }
        let mut tx = pool.begin().await.map_err(|e| format!("Failed to start transaction: {}", e))?;
        if input.replace.is_some() {
            sqlx::query("DELETE FROM inbox_member WHERE inbox_id = ?1").bind(&input.inbox_id).execute(&mut *tx).await.map_err(|e| format!("Failed to clear inbox: {}", e))?;
        }
        for member in input.remove.iter().flatten() {
            sqlx::query("DELETE FROM inbox_member WHERE inbox_id = ?1 AND kind = ?2 AND member_id = ?3")
                .bind(&input.inbox_id)
                .bind(&member.kind)
                .bind(&member.member_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to unlink: {}", e))?;
        }
        for member in input.replace.iter().flatten().chain(input.add.iter().flatten()) {
            sqlx::query("INSERT OR IGNORE INTO inbox_member (inbox_id, kind, member_id) VALUES (?1, ?2, ?3)")
                .bind(&input.inbox_id)
                .bind(&member.kind)
                .bind(&member.member_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to link: {}", e))?;
        }
        tx.commit().await.map_err(|e| format!("Failed to commit: {}", e))?;
        Self::get(pool, &input.inbox_id).await
    }

    pub async fn ensure_defaults(pool: &Pool<Sqlite>, profile_id: &str) -> Result<bool, String> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inbox WHERE profile_id = ?1")
            .bind(profile_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to count inboxes: {}", e))?;
        if count > 0 {
            Self::fill_icons(pool, profile_id).await?;
            return Ok(false);
        }
        sqlx::query("INSERT INTO inbox (id, profile_id, name, icon, position, is_default, created_at) VALUES (?1, ?2, ?3, ?4, 0, 1, ?5)")
            .bind(Uuid::new_v4().to_string())
            .bind(profile_id)
            .bind(DEFAULT_NAME)
            .bind(default_icon(DEFAULT_NAME))
            .bind(Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to create the default inbox: {}", e))?;
        let splits = crate::models::Tag::all(pool, profile_id).await?.into_iter().filter(|label| label.kind == crate::models::tag::KIND_SPLIT);
        for split in splits {
            let name = if split.name.eq_ignore_ascii_case("notifications") { "Service notifications".to_string() } else { split.name.clone() };
            let icon = split.icon.clone().or_else(|| Some(default_icon(&name).to_string()));
            let input = PostInboxInput { name, icon, members: Some(vec![InboxMember { kind: KIND_TAG.to_string(), member_id: split.id.clone() }]) };
            Self::post(pool, profile_id, &input).await?;
        }
        Ok(true)
    }

    async fn fill_icons(pool: &Pool<Sqlite>, profile_id: &str) -> Result<(), String> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT id, name FROM inbox WHERE profile_id = ?1 AND (icon IS NULL OR icon = '')")
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read inboxes without an icon: {}", e))?;
        for (id, name) in rows {
            sqlx::query("UPDATE inbox SET icon = ?1 WHERE id = ?2")
                .bind(default_icon(&name))
                .bind(&id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to set an inbox icon: {}", e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_pool, test_profile};

    fn member(kind: &str, id: &str) -> InboxMember {
        InboxMember { kind: kind.to_string(), member_id: id.to_string() }
    }

    #[test]
    fn known_inboxes_get_their_own_icons() {
        let icons: Vec<&str> = ["Inbox", "Important", "Team", "Newsletters", "Service notifications", "Other"].iter().map(|n| default_icon(n)).collect();
        let unique: std::collections::BTreeSet<&str> = icons.iter().copied().collect();
        assert_eq!(unique.len(), icons.len());
        assert_eq!(default_icon("Something new"), "lucide:folder");
    }

    #[tokio::test]
    async fn splits_become_user_inboxes_beside_a_default_inbox_once() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        crate::models::Tag::ensure_defaults(&pool, &profile).await.unwrap();
        let splits: Vec<_> = crate::models::Tag::all(&pool, &profile).await.unwrap().into_iter().filter(|l| l.kind == crate::models::tag::KIND_SPLIT).collect();
        assert!(Inbox::ensure_defaults(&pool, &profile).await.unwrap());
        assert!(!Inbox::ensure_defaults(&pool, &profile).await.unwrap());
        let inboxes = Inbox::all(&pool, &profile).await.unwrap();
        assert_eq!(inboxes.len(), splits.len() + 1);
        let default = inboxes.last().unwrap();
        assert!(default.is_default && default.members.is_empty() && default.name == DEFAULT_NAME);
        for split in &splits {
            let inbox = inboxes.iter().find(|i| i.members.contains(&member(KIND_TAG, &split.id))).unwrap();
            if split.name.eq_ignore_ascii_case("notifications") {
                assert_eq!(inbox.name, "Service notifications");
            } else {
                assert_eq!(inbox.name, split.name);
            }
        }
    }

    #[tokio::test]
    async fn members_link_overlap_and_the_default_cannot_be_deleted_or_linked() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        Inbox::ensure_defaults(&pool, &profile).await.unwrap();
        let work = Inbox::post(&pool, &profile, &PostInboxInput { name: "Work".into(), icon: None, members: Some(vec![member(KIND_PROVIDER, "p1")]) }).await.unwrap();
        let private = Inbox::post(&pool, &profile, &PostInboxInput { name: "Private".into(), icon: Some("lucide:house".into()), members: None }).await.unwrap();
        let private = Inbox::put_members(&pool, &PutInboxMembersInput { inbox_id: private.id.clone(), add: Some(vec![member(KIND_PERSON, "anna"), member(KIND_PROVIDER, "p1")]), remove: None, replace: None }).await.unwrap();
        assert_eq!(private.members.len(), 2);
        assert!(Inbox::get(&pool, &work.id).await.unwrap().members.contains(&member(KIND_PROVIDER, "p1")));
        let private = Inbox::put_members(&pool, &PutInboxMembersInput { inbox_id: private.id.clone(), add: None, remove: Some(vec![member(KIND_PROVIDER, "p1")]), replace: None }).await.unwrap();
        assert_eq!(private.members, vec![member(KIND_PERSON, "anna")]);
        assert!(Inbox::put_members(&pool, &PutInboxMembersInput { inbox_id: private.id.clone(), add: Some(vec![member("account", "x")]), remove: None, replace: None }).await.is_err());
        let default = Inbox::all(&pool, &profile).await.unwrap().into_iter().find(|i| i.is_default).unwrap();
        assert!(Inbox::delete(&pool, &default.id).await.is_err());
        assert!(Inbox::put_members(&pool, &PutInboxMembersInput { inbox_id: default.id.clone(), add: Some(vec![member(KIND_PERSON, "anna")]), remove: None, replace: None }).await.is_err());
        Inbox::put_order(&pool, &profile, &[private.id.clone(), work.id.clone()]).await.unwrap();
        let order: Vec<String> = Inbox::all(&pool, &profile).await.unwrap().into_iter().map(|i| i.name).collect();
        assert_eq!(order, vec!["Private", "Work", DEFAULT_NAME]);
        Inbox::delete(&pool, &work.id).await.unwrap();
        assert_eq!(Inbox::all(&pool, &profile).await.unwrap().len(), 2);
    }
}
