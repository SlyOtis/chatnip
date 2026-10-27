use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

pub const KIND_MANUAL: &str = "manual";
pub const KIND_TAG: &str = "tag";
pub const KIND_QUERY: &str = "query";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum GroupSort {
    #[default]
    Activity,
    Name,
    Unread,
    Messages,
    Tags,
    Created,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct GroupQuery {
    #[serde(default)]
    pub providers: Vec<String>,
    #[serde(default)]
    pub connection_ids: Vec<String>,
    #[serde(default)]
    pub conversation_ids: Vec<String>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(default)]
    pub person_ids: Vec<String>,
    #[serde(default)]
    pub unread: Option<bool>,
    #[serde(default)]
    pub has_attachments: Option<bool>,
    #[serde(default)]
    pub humans_only: Option<bool>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub sender: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub regex: Option<String>,
    #[serde(default)]
    pub sort: GroupSort,
    #[serde(default)]
    pub direction: SortDirection,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct QueryFacts {
    pub conversation_id: String,
    pub provider_type: String,
    pub provider_id: String,
    pub conversation_type: String,
    pub tag_ids: Vec<String>,
    pub person_id: Option<String>,
    pub unread: bool,
    pub has_attachments: bool,
    pub human: bool,
    pub sender: String,
    pub text: String,
    pub at: String,
}

impl GroupQuery {
    pub fn admits(&self, facts: &QueryFacts) -> bool {
        let any_of = |wanted: &[String], actual: &str| wanted.is_empty() || wanted.iter().any(|w| w.eq_ignore_ascii_case(actual));
        let provider = any_of(&self.providers, &facts.provider_type) && (self.connection_ids.is_empty() || self.connection_ids.contains(&facts.provider_id));
        if !provider || !any_of(&self.types, &facts.conversation_type) {
            return false;
        }
        if !self.conversation_ids.is_empty() && !self.conversation_ids.contains(&facts.conversation_id) {
            return false;
        }
        if !self.tag_ids.is_empty() && !self.tag_ids.iter().any(|l| facts.tag_ids.contains(l)) {
            return false;
        }
        if !self.person_ids.is_empty() && !facts.person_id.as_ref().is_some_and(|p| self.person_ids.contains(p)) {
            return false;
        }
        if self.unread.is_some_and(|u| u != facts.unread) || self.has_attachments.is_some_and(|a| a != facts.has_attachments) || (self.humans_only == Some(true) && !facts.human) {
            return false;
        }
        if self.from.as_deref().is_some_and(|from| !from.is_empty() && facts.at.as_str() < from) || self.to.as_deref().is_some_and(|to| !to.is_empty() && facts.at.as_str() > to) {
            return false;
        }
        let contains = |needle: &Option<String>, hay: &str| needle.as_deref().map(|n| n.trim()).filter(|n| !n.is_empty()).is_none_or(|n| hay.to_lowercase().contains(&n.to_lowercase()));
        if !contains(&self.sender, &facts.sender) || !contains(&self.text, &facts.text) {
            return false;
        }
        match self.regex.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
            Some(pattern) => regex::RegexBuilder::new(pattern).case_insensitive(true).build().map(|re| re.is_match(&facts.text)).unwrap_or(false),
            None => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub profile_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub tag_id: Option<String>,
    pub query: Option<GroupQuery>,
    pub position: i64,
    pub is_collapsed: bool,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub mode: crate::models::chat::ChatMode,
    pub conversation_ids: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostGroupInput {
    pub name: String,
    pub kind: Option<String>,
    pub parent_id: Option<String>,
    pub tag_id: Option<String>,
    pub query: Option<GroupQuery>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutGroupInput {
    pub group_id: String,
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub parent_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub tag_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable_query")]
    pub query: Option<Option<GroupQuery>>,
    pub position: Option<i64>,
    pub is_collapsed: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub icon: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub color: Option<Option<String>>,
    #[serde(default)]
    pub mode: Option<crate::models::chat::ChatMode>,
}

pub(crate) fn deserialize_nullable<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<String>::deserialize(deserializer)?))
}

fn deserialize_nullable_query<'de, D>(deserializer: D) -> Result<Option<Option<GroupQuery>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<GroupQuery>::deserialize(deserializer)?))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GroupItemsInput {
    pub group_id: String,
    pub conversation_ids: Vec<String>,
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS conversation_group (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            parent_id TEXT,
            name TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'manual',
            tag_id TEXT,
            query TEXT,
            position INTEGER NOT NULL DEFAULT 0,
            is_collapsed BOOLEAN NOT NULL DEFAULT 0,
            icon TEXT,
            color TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profile(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS conversation_group_item (
            group_id TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (group_id, conversation_id),
            FOREIGN KEY (group_id) REFERENCES conversation_group(id) ON DELETE CASCADE,
            FOREIGN KEY (conversation_id) REFERENCES conversation(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    crate::database::ensure_columns(pool, "conversation_group", &[("mode", "TEXT NOT NULL DEFAULT 'write'")]).await;
    Ok(())
}

impl Group {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        let query: Option<String> = row.try_get("query").ok().flatten();
        Self {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            parent_id: row.try_get("parent_id").ok().flatten(),
            name: row.get("name"),
            kind: row.get("kind"),
            tag_id: row.try_get("tag_id").ok().flatten(),
            query: query.and_then(|q| serde_json::from_str(&q).ok()),
            position: row.get("position"),
            is_collapsed: row.get("is_collapsed"),
            icon: row.try_get("icon").ok().flatten(),
            color: row.try_get("color").ok().flatten(),
            mode: crate::models::chat::ChatMode::parse(&row.try_get::<String, _>("mode").unwrap_or_default()),
            conversation_ids: Vec::new(),
            created_at: row.get("created_at"),
        }
    }

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Self>, String> {
        let rows = sqlx::query("SELECT * FROM conversation_group WHERE profile_id = ?1 ORDER BY position, created_at")
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load groups: {}", e))?;
        let mut groups: Vec<Self> = rows.into_iter().map(Self::from_row).collect();
        for group in &mut groups {
            group.conversation_ids = Self::items(pool, &group.id).await?;
        }
        Ok(groups)
    }

    pub async fn get(pool: &Pool<Sqlite>, group_id: &str) -> Result<Self, String> {
        let row = sqlx::query("SELECT * FROM conversation_group WHERE id = ?1")
            .bind(group_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load group: {}", e))?
            .ok_or_else(|| "Group not found".to_string())?;
        let mut group = Self::from_row(row);
        group.conversation_ids = Self::items(pool, &group.id).await?;
        Ok(group)
    }

    pub async fn items(pool: &Pool<Sqlite>, group_id: &str) -> Result<Vec<String>, String> {
        sqlx::query_scalar("SELECT conversation_id FROM conversation_group_item WHERE group_id = ?1 ORDER BY position, conversation_id")
            .bind(group_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load group items: {}", e))
    }

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, input: &PostGroupInput) -> Result<Self, String> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err("A group needs a name".to_string());
        }
        let kind = input.kind.clone().unwrap_or_else(|| KIND_MANUAL.to_string());
        if ![KIND_MANUAL, KIND_TAG, KIND_QUERY].contains(&kind.as_str()) {
            return Err(format!("Unknown group kind: {}", kind));
        }
        if kind == KIND_TAG && input.tag_id.is_none() {
            return Err("A tag group needs a label".to_string());
        }
        if kind == KIND_QUERY && input.query.is_none() {
            return Err("A query group needs a query".to_string());
        }
        if let Some(parent) = &input.parent_id {
            Self::get(pool, parent).await?;
        }
        let position: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), -1) + 1 FROM conversation_group WHERE profile_id = ?1 AND parent_id IS ?2")
            .bind(profile_id)
            .bind(&input.parent_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to place the group: {}", e))?;
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO conversation_group (id, profile_id, parent_id, name, kind, tag_id, query, position, is_collapsed, icon, color, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10, ?11)",
        )
        .bind(&id)
        .bind(profile_id)
        .bind(&input.parent_id)
        .bind(name)
        .bind(&kind)
        .bind(&input.tag_id)
        .bind(input.query.as_ref().map(|q| serde_json::to_string(q).unwrap_or_default()))
        .bind(position)
        .bind(&input.icon)
        .bind(&input.color)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create the group: {}", e))?;
        Self::get(pool, &id).await
    }

    pub async fn put(pool: &Pool<Sqlite>, input: &PutGroupInput) -> Result<Self, String> {
        let current = Self::get(pool, &input.group_id).await?;
        if let Some(parent) = &input.parent_id {
            if let Some(parent) = parent {
                if parent == &input.group_id || Self::descends_from(pool, "conversation_group", parent, &input.group_id).await? {
                    return Err("A group cannot be moved into itself".to_string());
                }
                Self::get(pool, parent).await?;
            }
        }
        let parent_id = input.parent_id.clone().unwrap_or(current.parent_id.clone());
        let name = input.name.clone().map(|n| n.trim().to_string()).unwrap_or(current.name.clone());
        if name.is_empty() {
            return Err("A group needs a name".to_string());
        }
        let tag_id = input.tag_id.clone().unwrap_or(current.tag_id.clone());
        let query = input.query.clone().unwrap_or(current.query.clone());
        let position = match input.position {
            Some(position) => position,
            None if input.parent_id.is_some() && parent_id != current.parent_id => {
                sqlx::query_scalar("SELECT COALESCE(MAX(position), -1) + 1 FROM conversation_group WHERE profile_id = ?1 AND parent_id IS ?2")
                    .bind(&current.profile_id)
                    .bind(&parent_id)
                    .fetch_one(pool)
                    .await
                    .map_err(|e| format!("Failed to place the group: {}", e))?
            }
            None => current.position,
        };
        sqlx::query(
            "UPDATE conversation_group SET parent_id = ?1, name = ?2, tag_id = ?3, query = ?4, position = ?5, is_collapsed = ?6, icon = ?7, color = ?8, mode = ?9 WHERE id = ?10",
        )
        .bind(&parent_id)
        .bind(&name)
        .bind(&tag_id)
        .bind(query.as_ref().map(|q| serde_json::to_string(q).unwrap_or_default()))
        .bind(position)
        .bind(input.is_collapsed.unwrap_or(current.is_collapsed))
        .bind(input.icon.clone().unwrap_or(current.icon.clone()))
        .bind(input.color.clone().unwrap_or(current.color.clone()))
        .bind(input.mode.unwrap_or(current.mode).name())
        .bind(&input.group_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update the group: {}", e))?;
        if input.position.is_some() {
            Self::compact(pool, "conversation_group", &current.profile_id, parent_id.as_deref(), &input.group_id, position).await?;
        }
        Self::get(pool, &input.group_id).await
    }

    pub(crate) async fn compact(pool: &Pool<Sqlite>, table: &str, profile_id: &str, parent_id: Option<&str>, moved: &str, position: i64) -> Result<(), String> {
        let siblings: Vec<String> = sqlx::query_scalar(&format!("SELECT id FROM {} WHERE profile_id = ?1 AND parent_id IS ?2 AND id != ?3 ORDER BY position, created_at", table))
            .bind(profile_id)
            .bind(parent_id)
            .bind(moved)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to order groups: {}", e))?;
        let mut ordered = siblings;
        let at = (position.max(0) as usize).min(ordered.len());
        ordered.insert(at, moved.to_string());
        for (index, id) in ordered.iter().enumerate() {
            sqlx::query(&format!("UPDATE {} SET position = ?1 WHERE id = ?2", table))
                .bind(index as i64)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to order groups: {}", e))?;
        }
        Ok(())
    }

    pub(crate) async fn descends_from(pool: &Pool<Sqlite>, table: &str, group_id: &str, ancestor: &str) -> Result<bool, String> {
        let mut current = Some(group_id.to_string());
        let mut hops = 0;
        while let Some(id) = current {
            if id == ancestor {
                return Ok(true);
            }
            hops += 1;
            if hops > 64 {
                return Ok(true);
            }
            current = sqlx::query_scalar::<_, Option<String>>(&format!("SELECT parent_id FROM {} WHERE id = ?1", table))
                .bind(&id)
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("Failed to walk groups: {}", e))?
                .flatten();
        }
        Ok(false)
    }

    pub async fn delete(pool: &Pool<Sqlite>, group_id: &str) -> Result<(), String> {
        let group = Self::get(pool, group_id).await?;
        sqlx::query("UPDATE conversation_group SET parent_id = ?1 WHERE parent_id = ?2")
            .bind(&group.parent_id)
            .bind(group_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to lift child groups: {}", e))?;
        sqlx::query("DELETE FROM conversation_group_item WHERE group_id = ?1")
            .bind(group_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to clear the group: {}", e))?;
        sqlx::query("DELETE FROM conversation_group WHERE id = ?1")
            .bind(group_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to delete the group: {}", e))?;
        Ok(())
    }

    pub async fn add_items(pool: &Pool<Sqlite>, group_id: &str, conversation_ids: &[String]) -> Result<(), String> {
        let start: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), -1) + 1 FROM conversation_group_item WHERE group_id = ?1")
            .bind(group_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to place items: {}", e))?;
        for (offset, conversation_id) in conversation_ids.iter().enumerate() {
            sqlx::query("INSERT OR IGNORE INTO conversation_group_item (group_id, conversation_id, position) VALUES (?1, ?2, ?3)")
                .bind(group_id)
                .bind(conversation_id)
                .bind(start + offset as i64)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to add to the group: {}", e))?;
        }
        Ok(())
    }

    pub async fn remove_items(pool: &Pool<Sqlite>, group_id: &str, conversation_ids: &[String]) -> Result<(), String> {
        for conversation_id in conversation_ids {
            sqlx::query("DELETE FROM conversation_group_item WHERE group_id = ?1 AND conversation_id = ?2")
                .bind(group_id)
                .bind(conversation_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to remove from the group: {}", e))?;
        }
        Ok(())
    }

    pub async fn migrate_sections(pool: &Pool<Sqlite>, profile_id: &str) -> Result<usize, String> {
        let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM conversation_group WHERE profile_id = ?1")
            .bind(profile_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to count groups: {}", e))?;
        if existing > 0 {
            return Ok(0);
        }
        let sections = sqlx::query("SELECT id, name, position, is_collapsed, created_at FROM sidebar_section WHERE profile_id = ?1 ORDER BY position")
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read sections: {}", e))?;
        let mut migrated = 0;
        for section in sections {
            let section_id: String = section.get("id");
            let group_id = Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO conversation_group (id, profile_id, parent_id, name, kind, position, is_collapsed, created_at) VALUES (?1, ?2, NULL, ?3, 'manual', ?4, ?5, ?6)")
                .bind(&group_id)
                .bind(profile_id)
                .bind(section.get::<String, _>("name"))
                .bind(section.get::<i64, _>("position"))
                .bind(section.get::<bool, _>("is_collapsed"))
                .bind(section.get::<String, _>("created_at"))
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to migrate a section: {}", e))?;
            let items: Vec<String> = sqlx::query_scalar("SELECT conversation_id FROM sidebar_section_item WHERE section_id = ?1 ORDER BY position")
                .bind(&section_id)
                .fetch_all(pool)
                .await
                .map_err(|e| format!("Failed to read section items: {}", e))?;
            Self::add_items(pool, &group_id, &items).await?;
            migrated += 1;
        }
        Ok(migrated)
    }

}

