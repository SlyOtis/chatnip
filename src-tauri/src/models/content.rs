use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::collections::HashMap;
use uuid::Uuid;
use crate::models::tag::{ObjectTag, OBJECT_CONTENT};

pub const KIND_GIF: &str = "gif";
pub const KIND_EMOJI: &str = "emoji";
pub const KIND_IMAGE: &str = "image";
pub const KIND_VIDEO: &str = "video";
pub const KIND_STICKER: &str = "sticker";
pub const KIND_FILE: &str = "file";
pub const KIND_REPLY: &str = "reply";
pub const KIND_EMAIL: &str = "email";
pub const KIND_PROMPT: &str = "prompt";
pub const KIND_SNIPPET: &str = "snippet";
pub const KIND_SIGNATURE: &str = "signature";
pub const KINDS: [&str; 11] = [KIND_GIF, KIND_EMOJI, KIND_IMAGE, KIND_VIDEO, KIND_STICKER, KIND_FILE, KIND_REPLY, KIND_EMAIL, KIND_PROMPT, KIND_SNIPPET, KIND_SIGNATURE];
pub const TEMPLATE_KINDS: [&str; 5] = [KIND_REPLY, KIND_EMAIL, KIND_PROMPT, KIND_SNIPPET, KIND_SIGNATURE];

pub const SOURCE_UPLOAD: &str = "upload";
pub const SOURCE_MESSAGE: &str = "message";
pub const SOURCE_TENOR: &str = "tenor";
pub const SOURCE_GIPHY: &str = "giphy";
pub const SOURCE_LIBRARY: &str = "library";
pub const SOURCE_URL: &str = "url";
pub const SOURCE_CONVERSATION: &str = "conversation";
pub const SOURCE_PERSON: &str = "person";
pub const SOURCE_ACTION: &str = "action";
pub const SOURCE_MANUAL: &str = "manual";
pub const SOURCES: [&str; 10] = [SOURCE_UPLOAD, SOURCE_MESSAGE, SOURCE_TENOR, SOURCE_GIPHY, SOURCE_LIBRARY, SOURCE_URL, SOURCE_CONVERSATION, SOURCE_PERSON, SOURCE_ACTION, SOURCE_MANUAL];

pub const SORT_RELEVANCE: &str = "relevance";
pub const SORT_USED: &str = "used";
pub const SORT_RECENT: &str = "recent";
pub const SORT_NEWEST: &str = "newest";

pub const GROUP_MANUAL: &str = "manual";
pub const GROUP_KIND: &str = "kind";
pub const GROUP_TAG: &str = "tag";
pub const GROUP_KINDS: [&str; 3] = [GROUP_MANUAL, GROUP_KIND, GROUP_TAG];

pub const DEFAULT_GROUPS: [(&str, &str, &str); 10] = [
    (KIND_GIF, "GIFs", "lucide:sparkles"),
    (KIND_STICKER, "Stickers", "lucide:sticker"),
    (KIND_IMAGE, "Images", "lucide:image"),
    (KIND_VIDEO, "Videos", "lucide:film"),
    (KIND_FILE, "Files", "lucide:file"),
    (KIND_REPLY, "Message replies", "lucide:message-square-reply"),
    (KIND_EMAIL, "Email replies", "lucide:mail"),
    (KIND_PROMPT, "AI prompts", "lucide:sparkle"),
    (KIND_SNIPPET, "Snippets", "lucide:text-quote"),
    (KIND_SIGNATURE, "Signatures", "lucide:signature"),
];

pub fn is_template(kind: &str) -> bool {
    TEMPLATE_KINDS.contains(&kind)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    pub id: String,
    pub profile_id: String,
    pub kind: String,
    pub name: String,
    pub source_url: Option<String>,
    pub path: String,
    pub mime_type: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub tags: Vec<String>,
    pub is_favourite: bool,
    pub used_count: i64,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub favourite_position: Option<i64>,
    pub query: Option<String>,
    pub keywords: Vec<String>,
    pub body: Option<String>,
    pub source_kind: Option<String>,
    pub source_message_id: Option<String>,
    pub source_conversation_id: Option<String>,
    pub source_provider_id: Option<String>,
    pub source_person_id: Option<String>,
    pub source_action_id: Option<String>,
    pub content_hash: Option<String>,
    pub size_bytes: Option<i64>,
    pub duration_ms: Option<i64>,
    #[serde(default)]
    pub group_ids: Vec<String>,
    #[serde(default)]
    pub file_path: Option<String>,
}

impl ContentItem {
    pub fn located(mut self, app_data: &std::path::Path) -> Self {
        if !self.path.is_empty() {
            self.file_path = Some(app_data.join(&self.path).to_string_lossy().to_string());
        }
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContentOrigin {
    pub source_kind: Option<String>,
    pub source_message_id: Option<String>,
    pub source_conversation_id: Option<String>,
    pub source_provider_id: Option<String>,
    pub source_person_id: Option<String>,
    pub source_action_id: Option<String>,
    pub content_hash: Option<String>,
    pub size_bytes: Option<i64>,
    pub duration_ms: Option<i64>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostContentInput {
    pub kind: String,
    pub name: String,
    pub source_url: Option<String>,
    pub data: Option<String>,
    pub mime_type: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub is_favourite: Option<bool>,
    #[serde(default)]
    pub group_ids: Option<Vec<String>>,
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    #[serde(default)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub source_message_id: Option<String>,
    #[serde(default)]
    pub source_conversation_id: Option<String>,
    #[serde(default)]
    pub source_person_id: Option<String>,
    #[serde(default)]
    pub source_action_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutContentInput {
    pub content_id: String,
    pub name: Option<String>,
    pub is_favourite: Option<bool>,
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutContentBulkInput {
    pub content_ids: Vec<String>,
    #[serde(default)]
    pub add_tags: Vec<String>,
    #[serde(default)]
    pub remove_tags: Vec<String>,
    #[serde(default)]
    pub add_group_ids: Vec<String>,
    #[serde(default)]
    pub remove_group_ids: Vec<String>,
    #[serde(default)]
    pub is_favourite: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ContentTag {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContentGroup {
    pub id: String,
    pub profile_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub rule: Option<String>,
    pub position: i64,
    pub is_collapsed: bool,
    pub icon: Option<String>,
    pub content_ids: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostContentGroupInput {
    pub name: String,
    pub kind: Option<String>,
    pub parent_id: Option<String>,
    pub rule: Option<String>,
    pub icon: Option<String>,
    #[serde(default)]
    pub content_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutContentGroupInput {
    pub content_group_id: String,
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::models::group::deserialize_nullable")]
    pub parent_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "crate::models::group::deserialize_nullable")]
    pub rule: Option<Option<String>>,
    pub position: Option<i64>,
    pub is_collapsed: Option<bool>,
    #[serde(default, deserialize_with = "crate::models::group::deserialize_nullable")]
    pub icon: Option<Option<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ContentGroupItemsInput {
    pub content_group_id: String,
    pub content_ids: Vec<String>,
    #[serde(default)]
    pub position: Option<i64>,
}

pub fn normalize_words(words: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in words {
        let word = word.trim().to_lowercase();
        if !word.is_empty() && !out.contains(&word) {
            out.push(word);
        }
    }
    out
}

pub fn edit_words(current: &[String], add: &[String], remove: &[String]) -> Vec<String> {
    let remove = normalize_words(remove);
    normalize_words(&[current.to_vec(), add.to_vec()].concat()).into_iter().filter(|w| !remove.contains(w)).collect()
}

fn term_score(item: &ContentItem, term: &str) -> i64 {
    let exact = item.tags.iter().chain(item.keywords.iter()).any(|w| w.to_lowercase() == term);
    if exact {
        return 3;
    }
    let name = item.name.to_lowercase();
    let starts = name.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(term)) || item.tags.iter().chain(item.keywords.iter()).any(|w| w.to_lowercase().starts_with(term));
    if starts {
        return 2;
    }
    let contains = name.contains(term)
        || item.tags.iter().chain(item.keywords.iter()).any(|w| w.to_lowercase().contains(term))
        || item.query.as_deref().is_some_and(|q| q.to_lowercase().contains(term))
        || item.body.as_deref().is_some_and(|b| b.to_lowercase().contains(term));
    if contains {
        1
    } else {
        0
    }
}

pub fn match_score(item: &ContentItem, query: &str) -> Option<i64> {
    let terms: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();
    if terms.is_empty() {
        return Some(0);
    }
    let mut total = 0;
    for term in &terms {
        let score = term_score(item, term);
        if score == 0 {
            return None;
        }
        total += score;
    }
    Some(total)
}

pub fn rank_score(item: &ContentItem, matched: i64, now: chrono::DateTime<Utc>) -> f64 {
    let used = (1.0 + item.used_count.max(0) as f64).ln() * 10.0;
    let recency = item
        .last_used_at
        .as_deref()
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map(|t| {
            let days = (now - t.with_timezone(&Utc)).num_minutes().max(0) as f64 / 1440.0;
            10.0 / (1.0 + days)
        })
        .unwrap_or(0.0);
    matched as f64 * 100.0 + used + recency + if item.is_favourite { 5.0 } else { 0.0 }
}

fn name_order(a: &ContentItem, b: &ContentItem) -> std::cmp::Ordering {
    a.name.to_lowercase().cmp(&b.name.to_lowercase())
}

pub fn sort_content(items: Vec<ContentItem>, query: Option<&str>, sort: &str, now: chrono::DateTime<Utc>) -> Vec<ContentItem> {
    let query = query.map(str::trim).unwrap_or("");
    let mut scored: Vec<(f64, ContentItem)> = items
        .into_iter()
        .filter_map(|item| match_score(&item, query).map(|m| (rank_score(&item, m, now), item)))
        .collect();
    match sort {
        SORT_USED => scored.sort_by(|a, b| b.1.used_count.cmp(&a.1.used_count).then_with(|| b.1.is_favourite.cmp(&a.1.is_favourite)).then_with(|| name_order(&a.1, &b.1))),
        SORT_RECENT => scored.sort_by(|a, b| b.1.last_used_at.cmp(&a.1.last_used_at).then_with(|| b.1.created_at.cmp(&a.1.created_at)).then_with(|| name_order(&a.1, &b.1))),
        SORT_NEWEST => scored.sort_by(|a, b| b.1.created_at.cmp(&a.1.created_at).then_with(|| name_order(&a.1, &b.1))),
        _ => scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| b.1.created_at.cmp(&a.1.created_at)).then_with(|| name_order(&a.1, &b.1))),
    }
    scored.into_iter().map(|(_, item)| item).collect()
}

pub fn kind_for_mime(mime_type: &str) -> &'static str {
    if mime_type == "image/gif" {
        KIND_GIF
    } else if mime_type.starts_with("video/") {
        KIND_VIDEO
    } else if mime_type.starts_with("image/") {
        KIND_IMAGE
    } else {
        KIND_FILE
    }
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS content_item (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            source_url TEXT,
            path TEXT NOT NULL,
            mime_type TEXT NOT NULL,
            width INTEGER,
            height INTEGER,
            is_favourite BOOLEAN NOT NULL DEFAULT 0,
            used_count INTEGER NOT NULL DEFAULT 0,
            last_used_at TEXT,
            created_at TEXT NOT NULL,
            favourite_position INTEGER,
            query TEXT,
            FOREIGN KEY (profile_id) REFERENCES profile (id)
        )
        "#,
    )
    .execute(pool)
    .await?;
    crate::database::ensure_columns(
        pool,
        "content_item",
        &[
            ("favourite_position", "INTEGER"),
            ("query", "TEXT"),
            ("keywords", "TEXT NOT NULL DEFAULT '[]'"),
            ("body", "TEXT"),
            ("source_kind", "TEXT"),
            ("source_message_id", "TEXT"),
            ("source_conversation_id", "TEXT"),
            ("source_provider_id", "TEXT"),
            ("source_person_id", "TEXT"),
            ("source_action_id", "TEXT"),
            ("content_hash", "TEXT"),
            ("size_bytes", "INTEGER"),
            ("duration_ms", "INTEGER"),
        ],
    )
    .await;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_content_item_hash ON content_item (profile_id, content_hash)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_content_item_profile_kind ON content_item (profile_id, kind)").execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS content_group (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            parent_id TEXT,
            name TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'manual',
            rule TEXT,
            position INTEGER NOT NULL DEFAULT 0,
            is_collapsed BOOLEAN NOT NULL DEFAULT 0,
            icon TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profile(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS content_group_item (
            group_id TEXT NOT NULL,
            content_id TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (group_id, content_id),
            FOREIGN KEY (group_id) REFERENCES content_group(id) ON DELETE CASCADE,
            FOREIGN KEY (content_id) REFERENCES content_item(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_content_group_item_content ON content_group_item (content_id)").execute(pool).await?;
    if crate::database::column_exists(pool, "content_item", "collections").await? {
        collections_into_groups(pool).await?;
    }
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS reaction_use (
            profile_id TEXT NOT NULL,
            emoji TEXT NOT NULL,
            used_count INTEGER NOT NULL DEFAULT 0,
            last_used_at TEXT NOT NULL,
            PRIMARY KEY (profile_id, emoji)
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS workspace_emoji (
            provider_id TEXT NOT NULL,
            name TEXT NOT NULL,
            url TEXT,
            alias_for TEXT,
            path TEXT,
            mime_type TEXT,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (provider_id, name)
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS content_signature (
            content_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            alias TEXT NOT NULL DEFAULT '',
            use_new BOOLEAN NOT NULL DEFAULT 1,
            use_reply BOOLEAN NOT NULL DEFAULT 1,
            use_forward BOOLEAN NOT NULL DEFAULT 1,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (content_id, provider_id, alias),
            FOREIGN KEY (content_id) REFERENCES content_item(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn collections_into_groups(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    const OLD_SETTING: &str = concat!("gif.", "collections");
    let ordered: Vec<String> = match sqlx::query_scalar::<_, String>("SELECT value FROM app_setting WHERE key = ?1").bind(OLD_SETTING).fetch_optional(pool).await {
        Ok(Some(value)) => serde_json::from_str::<Vec<String>>(&value).unwrap_or_default(),
        _ => Vec::new(),
    };
    let rows = sqlx::query("SELECT ci.id, ci.profile_id, ci.collections FROM content_item ci JOIN profile p ON p.id = ci.profile_id ORDER BY ci.created_at").fetch_all(pool).await?;
    let mut members: Vec<(String, String, Vec<String>)> = Vec::new();
    for row in rows {
        let names: Vec<String> = serde_json::from_str::<Vec<String>>(&row.get::<Option<String>, _>("collections").unwrap_or_default()).unwrap_or_default();
        members.push((row.get("profile_id"), row.get("id"), names.into_iter().map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect()));
    }
    let mut profiles: Vec<String> = Vec::new();
    for (profile, _, _) in &members {
        if !profiles.contains(profile) {
            profiles.push(profile.clone());
        }
    }
    let now = Utc::now().to_rfc3339();
    for profile in profiles {
        let mut names = ordered.clone();
        let mut extra: Vec<String> = members.iter().filter(|(p, _, _)| *p == profile).flat_map(|(_, _, n)| n.clone()).filter(|n| !names.contains(n)).collect();
        extra.sort();
        extra.dedup();
        names.extend(extra);
        for (position, name) in names.iter().enumerate() {
            let id = Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO content_group (id, profile_id, parent_id, name, kind, rule, position, is_collapsed, icon, created_at) VALUES (?1, ?2, NULL, ?3, 'manual', NULL, ?4, 0, 'lucide:folder', ?5)")
                .bind(&id)
                .bind(&profile)
                .bind(name)
                .bind(position as i64)
                .bind(&now)
                .execute(pool)
                .await?;
            let held: Vec<&String> = members.iter().filter(|(p, _, n)| *p == profile && n.contains(name)).map(|(_, item, _)| item).collect();
            for (index, item) in held.into_iter().enumerate() {
                sqlx::query("INSERT OR IGNORE INTO content_group_item (group_id, content_id, position) VALUES (?1, ?2, ?3)").bind(&id).bind(item).bind(index as i64).execute(pool).await?;
            }
        }
    }
    sqlx::query("ALTER TABLE content_item DROP COLUMN collections").execute(pool).await?;
    sqlx::query("DELETE FROM app_setting WHERE key = ?1").bind(OLD_SETTING).execute(pool).await.ok();
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReactionUse {
    pub emoji: String,
    pub used_count: i64,
    pub last_used_at: String,
}

impl ReactionUse {
    pub async fn put(pool: &Pool<Sqlite>, profile_id: &str, emoji: &str) -> Result<(), String> {
        let emoji = emoji.trim();
        if emoji.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO reaction_use (profile_id, emoji, used_count, last_used_at) VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(profile_id, emoji) DO UPDATE SET used_count = used_count + 1, last_used_at = excluded.last_used_at",
        )
        .bind(profile_id)
        .bind(emoji)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to record the reaction: {}", e))?;
        Ok(())
    }

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str, limit: i64) -> Result<Vec<Self>, String> {
        let rows = sqlx::query("SELECT emoji, used_count, last_used_at FROM reaction_use WHERE profile_id = ?1 ORDER BY used_count DESC, last_used_at DESC LIMIT ?2")
            .bind(profile_id)
            .bind(limit)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read reaction use: {}", e))?;
        Ok(rows.into_iter().map(|row| Self { emoji: row.get("emoji"), used_count: row.get("used_count"), last_used_at: row.get("last_used_at") }).collect())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceEmoji {
    pub provider_id: String,
    pub provider_name: String,
    pub provider_type: String,
    pub name: String,
    pub alias_for: Option<String>,
    pub url: Option<String>,
    pub path: Option<String>,
    pub file_path: Option<String>,
    pub mime_type: Option<String>,
    pub updated_at: String,
}

impl WorkspaceEmoji {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Self {
        Self {
            provider_id: row.get("provider_id"),
            provider_name: row.try_get("provider_name").unwrap_or_default(),
            provider_type: row.try_get("provider_type").unwrap_or_default(),
            name: row.get("name"),
            alias_for: row.get("alias_for"),
            url: row.get("url"),
            path: row.get("path"),
            file_path: None,
            mime_type: row.get("mime_type"),
            updated_at: row.get("updated_at"),
        }
    }

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Self>, String> {
        let rows = sqlx::query(
            "SELECT e.*, p.name AS provider_name, p.provider_type FROM workspace_emoji e JOIN provider p ON p.id = e.provider_id
             WHERE p.profile_id = ?1 AND p.is_enabled = 1 ORDER BY p.name, e.name",
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to read the workspace emoji: {}", e))?;
        Ok(rows.iter().map(Self::from_row).collect())
    }

    pub async fn get_workspace(pool: &Pool<Sqlite>, provider_id: &str) -> Result<Vec<Self>, String> {
        let rows = sqlx::query("SELECT e.*, p.name AS provider_name, p.provider_type FROM workspace_emoji e JOIN provider p ON p.id = e.provider_id WHERE e.provider_id = ?1 ORDER BY e.name")
            .bind(provider_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read the workspace emoji: {}", e))?;
        Ok(rows.iter().map(Self::from_row).collect())
    }

    pub fn image_url(&self, all: &[Self]) -> Option<String> {
        match &self.alias_for {
            Some(target) => all.iter().find(|e| e.provider_id == self.provider_id && e.name == *target && e.alias_for.is_none()).and_then(|e| e.url.clone()),
            None => self.url.clone(),
        }
    }

    pub async fn put_workspace(pool: &Pool<Sqlite>, provider_id: &str, emoji: &[crate::providers::traits::RemoteEmoji]) -> Result<Vec<String>, String> {
        let existing = Self::get_workspace(pool, provider_id).await?;
        let now = Utc::now().to_rfc3339();
        let mut fetch = Vec::new();
        for item in emoji.iter().filter(|e| !e.name.trim().is_empty()) {
            let before = existing.iter().find(|e| e.name == item.name);
            let changed = before.map(|b| b.url != item.url || b.path.is_none()).unwrap_or(true);
            if changed && item.alias_for.is_none() && item.url.is_some() {
                fetch.push(item.name.clone());
            }
            sqlx::query(
                "INSERT INTO workspace_emoji (provider_id, name, url, alias_for, path, mime_type, updated_at) VALUES (?1, ?2, ?3, ?4, NULL, NULL, ?5)
                 ON CONFLICT(provider_id, name) DO UPDATE SET url = excluded.url, alias_for = excluded.alias_for, updated_at = excluded.updated_at,
                 path = CASE WHEN workspace_emoji.url IS excluded.url THEN workspace_emoji.path ELSE NULL END,
                 mime_type = CASE WHEN workspace_emoji.url IS excluded.url THEN workspace_emoji.mime_type ELSE NULL END",
            )
            .bind(provider_id)
            .bind(&item.name)
            .bind(&item.url)
            .bind(&item.alias_for)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to store the emoji {}: {}", item.name, e))?;
        }
        for gone in existing.iter().filter(|e| !emoji.iter().any(|item| item.name == e.name)) {
            sqlx::query("DELETE FROM workspace_emoji WHERE provider_id = ?1 AND name = ?2")
                .bind(provider_id)
                .bind(&gone.name)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to forget the emoji {}: {}", gone.name, e))?;
        }
        Ok(fetch)
    }

    pub async fn put_path(pool: &Pool<Sqlite>, provider_id: &str, name: &str, path: &str, mime_type: &str) -> Result<(), String> {
        sqlx::query("UPDATE workspace_emoji SET path = ?1, mime_type = ?2 WHERE provider_id = ?3 AND name = ?4")
            .bind(path)
            .bind(mime_type)
            .bind(provider_id)
            .bind(name)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to keep the emoji image {}: {}", name, e))?;
        Ok(())
    }
}

pub fn extension_for(mime_type: &str) -> &'static str {
    match mime_type {
        "image/gif" => "gif",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        "video/quicktime" => "mov",
        "video/webm" => "webm",
        "application/pdf" => "pdf",
        "text/plain" => "txt",
        "text/csv" => "csv",
        "application/json" => "json",
        "application/zip" => "zip",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx",
        _ => "bin",
    }
}

pub fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        Some(if &bytes[8..12] == b"qt  " { "video/quicktime" } else { "video/mp4" })
    } else if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        Some("video/webm")
    } else {
        None
    }
}

pub fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    match sniff_mime(bytes)? {
        "image/gif" => {
            if bytes.len() < 10 {
                return None;
            }
            Some((u16::from_le_bytes([bytes[6], bytes[7]]) as u32, u16::from_le_bytes([bytes[8], bytes[9]]) as u32))
        }
        "image/png" => {
            if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
                return None;
            }
            Some((u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]), u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]])))
        }
        "image/jpeg" => jpeg_dimensions(bytes),
        "image/webp" => webp_dimensions(bytes),
        _ => None,
    }
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2;
    while i + 9 < bytes.len() {
        if bytes[i] != 0xff {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        if marker == 0xff {
            i += 1;
            continue;
        }
        if matches!(marker, 0xd8 | 0x01 | 0xd0..=0xd7) {
            i += 2;
            continue;
        }
        let length = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            let height = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            let width = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
            return Some((width, height));
        }
        i += 2 + length;
    }
    None
}

fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 30 {
        return None;
    }
    let chunk = &bytes[12..16];
    let data = &bytes[20..];
    match chunk {
        b"VP8 " => {
            if data.len() < 10 || data[3..6] != [0x9d, 0x01, 0x2a] {
                return None;
            }
            let width = (u16::from_le_bytes([data[6], data[7]]) & 0x3fff) as u32;
            let height = (u16::from_le_bytes([data[8], data[9]]) & 0x3fff) as u32;
            Some((width, height))
        }
        b"VP8L" => {
            if data.len() < 5 || data[0] != 0x2f {
                return None;
            }
            let bits = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
            Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
        }
        b"VP8X" => {
            if data.len() < 10 {
                return None;
            }
            let width = 1 + (data[4] as u32 | (data[5] as u32) << 8 | (data[6] as u32) << 16);
            let height = 1 + (data[7] as u32 | (data[8] as u32) << 8 | (data[9] as u32) << 16);
            Some((width, height))
        }
        _ => None,
    }
}


impl ContentItem {
    pub fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        let keywords: Option<String> = row.try_get("keywords").ok();
        Self {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            kind: row.get("kind"),
            name: row.get("name"),
            source_url: row.get("source_url"),
            path: row.get("path"),
            mime_type: row.get("mime_type"),
            width: row.get("width"),
            height: row.get("height"),
            tags: Vec::new(),
            is_favourite: row.get("is_favourite"),
            used_count: row.get("used_count"),
            last_used_at: row.get("last_used_at"),
            created_at: row.get("created_at"),
            favourite_position: row.get("favourite_position"),
            query: row.get("query"),
            keywords: keywords.and_then(|k| serde_json::from_str(&k).ok()).unwrap_or_default(),
            body: row.try_get("body").ok().flatten(),
            source_kind: row.try_get("source_kind").ok().flatten(),
            source_message_id: row.try_get("source_message_id").ok().flatten(),
            source_conversation_id: row.try_get("source_conversation_id").ok().flatten(),
            source_provider_id: row.try_get("source_provider_id").ok().flatten(),
            source_person_id: row.try_get("source_person_id").ok().flatten(),
            source_action_id: row.try_get("source_action_id").ok().flatten(),
            content_hash: row.try_get("content_hash").ok().flatten(),
            size_bytes: row.try_get("size_bytes").ok().flatten(),
            duration_ms: row.try_get("duration_ms").ok().flatten(),
            group_ids: Vec::new(),
            file_path: None,
        }
    }

    pub async fn insert(
        pool: &Pool<Sqlite>,
        profile_id: &str,
        kind: &str,
        name: &str,
        source_url: Option<&str>,
        path: &str,
        mime_type: &str,
        size: Option<(u32, u32)>,
        tags: &[String],
        query: Option<&str>,
        body: Option<&str>,
    ) -> Result<ContentItem, String> {
        if !KINDS.contains(&kind) {
            return Err(format!("Unknown content kind {}", kind));
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO content_item (id, profile_id, kind, name, source_url, path, mime_type, width, height, is_favourite, used_count, last_used_at, created_at, query, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, 0, NULL, ?10, ?11, ?12)",
        )
        .bind(&id)
        .bind(profile_id)
        .bind(kind)
        .bind(name)
        .bind(source_url)
        .bind(path)
        .bind(mime_type)
        .bind(size.map(|s| s.0 as i64))
        .bind(size.map(|s| s.1 as i64))
        .bind(&now)
        .bind(query.map(str::trim).filter(|q| !q.is_empty()))
        .bind(body)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to store the content: {}", e))?;
        ObjectTag::put_names(pool, profile_id, OBJECT_CONTENT, &id, &normalize_words(tags)).await?;
        Self::get(pool, &id).await?.ok_or_else(|| "Content not found after insert".to_string())
    }

    async fn memberships(pool: &Pool<Sqlite>, profile_id: &str) -> Result<HashMap<String, Vec<String>>, String> {
        let rows = sqlx::query("SELECT gi.content_id, gi.group_id FROM content_group_item gi JOIN content_group g ON g.id = gi.group_id WHERE g.profile_id = ?1 ORDER BY g.position, g.created_at")
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read content groups: {}", e))?;
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            out.entry(row.get("content_id")).or_default().push(row.get("group_id"));
        }
        Ok(out)
    }

    pub async fn get(pool: &Pool<Sqlite>, id: &str) -> Result<Option<ContentItem>, String> {
        let row = sqlx::query("SELECT * FROM content_item WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load the content: {}", e))?;
        let Some(mut item) = row.map(Self::from_row) else { return Ok(None) };
        item.tags = sqlx::query_scalar("SELECT t.name FROM object_tag ot JOIN tag t ON t.id = ot.tag_id WHERE ot.object_type = ?1 AND ot.object_id = ?2 ORDER BY ot.created_at, ot.rowid")
            .bind(OBJECT_CONTENT)
            .bind(id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read content tags: {}", e))?;
        item.group_ids = sqlx::query_scalar("SELECT gi.group_id FROM content_group_item gi JOIN content_group g ON g.id = gi.group_id WHERE gi.content_id = ?1 ORDER BY g.position, g.created_at")
            .bind(id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read content groups: {}", e))?;
        Ok(Some(item))
    }

    pub async fn get_source(pool: &Pool<Sqlite>, profile_id: &str, source_url: &str) -> Result<Option<ContentItem>, String> {
        let id: Option<String> = sqlx::query_scalar("SELECT id FROM content_item WHERE profile_id = ?1 AND source_url = ?2 LIMIT 1")
            .bind(profile_id)
            .bind(source_url)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load the content: {}", e))?;
        match id {
            Some(id) => Self::get(pool, &id).await,
            None => Ok(None),
        }
    }

    pub async fn get_reference(pool: &Pool<Sqlite>, profile_id: &str, reference: &str) -> Result<Option<ContentItem>, String> {
        let id: Option<String> = sqlx::query_scalar("SELECT id FROM content_item WHERE profile_id = ?1 AND (id = ?2 OR LOWER(name) = LOWER(TRIM(?2))) ORDER BY id = ?2 DESC, created_at LIMIT 1")
            .bind(profile_id)
            .bind(reference)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load the content: {}", e))?;
        match id {
            Some(id) => Self::get(pool, &id).await,
            None => Ok(None),
        }
    }

    pub async fn library(pool: &Pool<Sqlite>, profile_id: &str, kind: Option<&str>, query: Option<&str>) -> Result<Vec<ContentItem>, String> {
        let rows = sqlx::query(
            "SELECT * FROM content_item WHERE profile_id = ?1 AND (?2 IS NULL OR kind = ?2)
             ORDER BY is_favourite DESC, COALESCE(favourite_position, 0) ASC, last_used_at DESC, created_at DESC",
        )
        .bind(profile_id)
        .bind(kind)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list the content: {}", e))?;
        let mut groups = Self::memberships(pool, profile_id).await?;
        let mut tags = ObjectTag::get_names(pool, profile_id, OBJECT_CONTENT).await?;
        let needle = query.map(|q| q.trim().to_lowercase()).filter(|q| !q.is_empty());
        Ok(rows
            .into_iter()
            .map(Self::from_row)
            .map(|mut item| {
                item.tags = tags.remove(&item.id).unwrap_or_default();
                item
            })
            .filter(|item| match &needle {
                None => true,
                Some(q) => {
                    item.name.to_lowercase().contains(q)
                        || item.tags.iter().chain(item.keywords.iter()).any(|t| t.to_lowercase().contains(q))
                        || item.query.as_deref().is_some_and(|s| s.to_lowercase().contains(q))
                        || item.body.as_deref().is_some_and(|s| s.to_lowercase().contains(q))
                }
            })
            .map(|mut item| {
                item.group_ids = groups.remove(&item.id).unwrap_or_default();
                item
            })
            .collect())
    }

    pub async fn set_origin(pool: &Pool<Sqlite>, id: &str, origin: &ContentOrigin) -> Result<ContentItem, String> {
        if let Some(kind) = origin.source_kind.as_deref() {
            if !SOURCES.contains(&kind) {
                return Err(format!("Unknown source {}", kind));
            }
        }
        sqlx::query(
            "UPDATE content_item SET source_kind = COALESCE(?1, source_kind), source_message_id = COALESCE(?2, source_message_id), source_conversation_id = COALESCE(?3, source_conversation_id),
             source_provider_id = COALESCE(?4, source_provider_id), content_hash = COALESCE(?5, content_hash), size_bytes = COALESCE(?6, size_bytes), duration_ms = COALESCE(?7, duration_ms), keywords = ?8,
             source_person_id = COALESCE(?10, source_person_id), source_action_id = COALESCE(?11, source_action_id) WHERE id = ?9",
        )
        .bind(&origin.source_kind)
        .bind(&origin.source_message_id)
        .bind(&origin.source_conversation_id)
        .bind(&origin.source_provider_id)
        .bind(&origin.content_hash)
        .bind(origin.size_bytes)
        .bind(origin.duration_ms)
        .bind(serde_json::to_string(&normalize_words(&origin.keywords)).unwrap_or_else(|_| "[]".to_string()))
        .bind(id)
        .bind(&origin.source_person_id)
        .bind(&origin.source_action_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to record where the content came from: {}", e))?;
        Self::get(pool, id).await?.ok_or_else(|| "Content not found".to_string())
    }

    pub async fn get_hash(pool: &Pool<Sqlite>, profile_id: &str, hash: &str) -> Result<Option<ContentItem>, String> {
        let id: Option<String> = sqlx::query_scalar("SELECT id FROM content_item WHERE profile_id = ?1 AND content_hash = ?2 LIMIT 1")
            .bind(profile_id)
            .bind(hash)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load the content: {}", e))?;
        match id {
            Some(id) => Self::get(pool, &id).await,
            None => Ok(None),
        }
    }

    pub async fn search(pool: &Pool<Sqlite>, profile_id: &str, kind: Option<&str>, query: Option<&str>, sort: &str) -> Result<Vec<ContentItem>, String> {
        let all = Self::library(pool, profile_id, kind, None).await?;
        Ok(sort_content(all, query, sort, Utc::now()))
    }

    pub async fn tags(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<ContentTag>, String> {
        let mut counts: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
        for item in Self::library(pool, profile_id, None, None).await? {
            for tag in normalize_words(&item.tags) {
                *counts.entry(tag).or_default() += 1;
            }
        }
        let mut out: Vec<ContentTag> = counts.into_iter().map(|(name, count)| ContentTag { name, count }).collect();
        out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
        Ok(out)
    }

    pub async fn put_bulk(pool: &Pool<Sqlite>, profile_id: &str, input: &PutContentBulkInput) -> Result<Vec<ContentItem>, String> {
        let mut owned = Vec::new();
        for id in &input.content_ids {
            let Some(item) = Self::get(pool, id).await? else { continue };
            if item.profile_id != profile_id {
                continue;
            }
            let update = PutContentInput { content_id: item.id.clone(), is_favourite: input.is_favourite, tags: Some(edit_words(&item.tags, &input.add_tags, &input.remove_tags)), ..PutContentInput::default() };
            Self::update(pool, &update).await?;
            owned.push(item.id);
        }
        for group_id in &input.add_group_ids {
            ContentGroup::put_items(pool, profile_id, group_id, &owned, None).await?;
        }
        for group_id in &input.remove_group_ids {
            ContentGroup::remove_items(pool, profile_id, group_id, &owned).await?;
        }
        let mut out = Vec::new();
        for id in &owned {
            out.extend(Self::get(pool, id).await?);
        }
        Ok(out)
    }

    pub async fn touch(pool: &Pool<Sqlite>, id: &str) -> Result<(), String> {
        sqlx::query("UPDATE content_item SET used_count = used_count + 1, last_used_at = ?1 WHERE id = ?2")
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to record the use: {}", e))?;
        Ok(())
    }

    pub async fn update(pool: &Pool<Sqlite>, input: &PutContentInput) -> Result<ContentItem, String> {
        let current = Self::get(pool, &input.content_id).await?.ok_or_else(|| "Content not found".to_string())?;
        let keywords = input.keywords.as_deref().map(normalize_words).unwrap_or(current.keywords);
        let is_favourite = input.is_favourite.unwrap_or(current.is_favourite);
        let kind = match input.kind.as_deref().filter(|k| *k != current.kind) {
            Some(kind) if is_template(kind) && is_template(&current.kind) => kind.to_string(),
            Some(kind) => return Err(format!("A {} cannot become a {}", current.kind, kind)),
            None => current.kind.clone(),
        };
        if input.body.is_some() && !is_template(&kind) {
            return Err("Only replies, emails, prompts and snippets carry text".to_string());
        }
        let position = match (current.is_favourite, is_favourite) {
            (_, false) => None,
            (true, true) => current.favourite_position,
            (false, true) => {
                let first: Option<i64> = sqlx::query_scalar("SELECT MIN(favourite_position) FROM content_item WHERE profile_id = ?1 AND is_favourite = 1")
                    .bind(&current.profile_id)
                    .fetch_one(pool)
                    .await
                    .map_err(|e| format!("Failed to order favourites: {}", e))?;
                Some(first.unwrap_or(0) - 1)
            }
        };
        sqlx::query("UPDATE content_item SET name = ?1, is_favourite = ?2, favourite_position = ?3, keywords = ?5, body = ?6, kind = ?7 WHERE id = ?4")
            .bind(input.name.clone().map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or(current.name))
            .bind(is_favourite)
            .bind(position)
            .bind(&input.content_id)
            .bind(serde_json::to_string(&keywords).unwrap_or_else(|_| "[]".to_string()))
            .bind(input.body.clone().or(current.body))
            .bind(&kind)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update the content: {}", e))?;
        if let Some(tags) = &input.tags {
            ObjectTag::put_names(pool, &current.profile_id, OBJECT_CONTENT, &input.content_id, &normalize_words(tags)).await?;
        }
        Self::get(pool, &input.content_id).await?.ok_or_else(|| "Content not found".to_string())
    }

    pub async fn put_order(pool: &Pool<Sqlite>, profile_id: &str, content_ids: &[String]) -> Result<(), String> {
        let mut tx = pool.begin().await.map_err(|e| format!("Failed to order favourites: {}", e))?;
        for (index, id) in content_ids.iter().enumerate() {
            sqlx::query("UPDATE content_item SET favourite_position = ?1 WHERE id = ?2 AND profile_id = ?3 AND is_favourite = 1")
                .bind(index as i64)
                .bind(id)
                .bind(profile_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to order favourites: {}", e))?;
        }
        tx.commit().await.map_err(|e| format!("Failed to order favourites: {}", e))
    }

    pub async fn delete(pool: &Pool<Sqlite>, id: &str) -> Result<Option<ContentItem>, String> {
        let current = Self::get(pool, id).await?;
        sqlx::query("DELETE FROM content_group_item WHERE content_id = ?1")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to take the content out of its groups: {}", e))?;
        sqlx::query("DELETE FROM content_item WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to delete the content: {}", e))?;
        ObjectTag::delete_object(pool, OBJECT_CONTENT, id).await?;
        Ok(current)
    }
}

impl ContentGroup {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        Self {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            parent_id: row.try_get("parent_id").ok().flatten(),
            name: row.get("name"),
            kind: row.get("kind"),
            rule: row.try_get("rule").ok().flatten(),
            position: row.get("position"),
            is_collapsed: row.get("is_collapsed"),
            icon: row.try_get("icon").ok().flatten(),
            content_ids: Vec::new(),
            created_at: row.get("created_at"),
        }
    }

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Self>, String> {
        let rows = sqlx::query("SELECT * FROM content_group WHERE profile_id = ?1 ORDER BY position, created_at")
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load content groups: {}", e))?;
        let mut groups: Vec<Self> = rows.into_iter().map(Self::from_row).collect();
        for group in &mut groups {
            group.content_ids = Self::items(pool, &group.id).await?;
        }
        Ok(groups)
    }

    pub async fn get(pool: &Pool<Sqlite>, profile_id: &str, id: &str) -> Result<Self, String> {
        let row = sqlx::query("SELECT * FROM content_group WHERE id = ?1 AND profile_id = ?2")
            .bind(id)
            .bind(profile_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load the content group: {}", e))?
            .ok_or_else(|| "Content group not found".to_string())?;
        let mut group = Self::from_row(row);
        group.content_ids = Self::items(pool, &group.id).await?;
        Ok(group)
    }

    pub async fn items(pool: &Pool<Sqlite>, id: &str) -> Result<Vec<String>, String> {
        sqlx::query_scalar("SELECT content_id FROM content_group_item WHERE group_id = ?1 ORDER BY position, content_id")
            .bind(id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load the group's content: {}", e))
    }

    fn check_rule(kind: &str, rule: Option<&str>) -> Result<Option<String>, String> {
        let rule = rule.map(str::trim).filter(|r| !r.is_empty());
        match kind {
            GROUP_MANUAL => Ok(None),
            GROUP_KIND => match rule {
                Some(value) if KINDS.contains(&value) => Ok(Some(value.to_string())),
                _ => Err("A kind group needs a known kind".to_string()),
            },
            GROUP_TAG => rule.map(|r| Some(r.to_lowercase())).ok_or_else(|| "A tag group needs a tag".to_string()),
            other => Err(format!("Unknown group kind {}", other)),
        }
    }

    async fn next_position(pool: &Pool<Sqlite>, profile_id: &str, parent_id: Option<&str>) -> Result<i64, String> {
        sqlx::query_scalar("SELECT COALESCE(MAX(position), -1) + 1 FROM content_group WHERE profile_id = ?1 AND parent_id IS ?2")
            .bind(profile_id)
            .bind(parent_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to place the group: {}", e))
    }

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, input: &PostContentGroupInput) -> Result<Self, String> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err("A group needs a name".to_string());
        }
        let kind = input.kind.clone().unwrap_or_else(|| GROUP_MANUAL.to_string());
        let rule = Self::check_rule(&kind, input.rule.as_deref())?;
        if let Some(parent) = &input.parent_id {
            Self::get(pool, profile_id, parent).await?;
        }
        let position = Self::next_position(pool, profile_id, input.parent_id.as_deref()).await?;
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO content_group (id, profile_id, parent_id, name, kind, rule, position, is_collapsed, icon, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9)")
            .bind(&id)
            .bind(profile_id)
            .bind(&input.parent_id)
            .bind(name)
            .bind(&kind)
            .bind(&rule)
            .bind(position)
            .bind(&input.icon)
            .bind(Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to create the group: {}", e))?;
        if !input.content_ids.is_empty() {
            Self::put_items(pool, profile_id, &id, &input.content_ids, None).await?;
        }
        Self::get(pool, profile_id, &id).await
    }

    pub async fn put(pool: &Pool<Sqlite>, profile_id: &str, input: &PutContentGroupInput) -> Result<Self, String> {
        const TABLE: &str = "content_group";
        let current = Self::get(pool, profile_id, &input.content_group_id).await?;
        if let Some(Some(parent)) = &input.parent_id {
            if parent == &input.content_group_id || crate::models::group::Group::descends_from(pool, TABLE, parent, &input.content_group_id).await? {
                return Err("A group cannot be moved into itself".to_string());
            }
            Self::get(pool, profile_id, parent).await?;
        }
        let parent_id = input.parent_id.clone().unwrap_or(current.parent_id.clone());
        let name = input.name.clone().map(|n| n.trim().to_string()).unwrap_or(current.name.clone());
        if name.is_empty() {
            return Err("A group needs a name".to_string());
        }
        let rule = match &input.rule {
            Some(rule) => Self::check_rule(&current.kind, rule.as_deref())?,
            None => current.rule.clone(),
        };
        let moved = input.parent_id.is_some() && parent_id != current.parent_id;
        let position = match input.position {
            Some(position) => position,
            None if moved => Self::next_position(pool, profile_id, parent_id.as_deref()).await?,
            None => current.position,
        };
        sqlx::query("UPDATE content_group SET parent_id = ?1, name = ?2, rule = ?3, position = ?4, is_collapsed = ?5, icon = ?6 WHERE id = ?7")
            .bind(&parent_id)
            .bind(&name)
            .bind(&rule)
            .bind(position)
            .bind(input.is_collapsed.unwrap_or(current.is_collapsed))
            .bind(input.icon.clone().unwrap_or(current.icon.clone()))
            .bind(&input.content_group_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update the group: {}", e))?;
        if input.position.is_some() {
            crate::models::group::Group::compact(pool, TABLE, profile_id, parent_id.as_deref(), &input.content_group_id, position).await?;
        }
        Self::get(pool, profile_id, &input.content_group_id).await
    }

    pub async fn delete(pool: &Pool<Sqlite>, profile_id: &str, id: &str) -> Result<(), String> {
        let group = Self::get(pool, profile_id, id).await?;
        let mut position = Self::next_position(pool, profile_id, group.parent_id.as_deref()).await?;
        let children: Vec<String> = sqlx::query_scalar("SELECT id FROM content_group WHERE parent_id = ?1 ORDER BY position, created_at")
            .bind(id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read child groups: {}", e))?;
        for child in children {
            sqlx::query("UPDATE content_group SET parent_id = ?1, position = ?2 WHERE id = ?3")
                .bind(&group.parent_id)
                .bind(position)
                .bind(&child)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to lift child groups: {}", e))?;
            position += 1;
        }
        sqlx::query("DELETE FROM content_group_item WHERE group_id = ?1")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to clear the group: {}", e))?;
        sqlx::query("DELETE FROM content_group WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to delete the group: {}", e))?;
        Ok(())
    }

    pub async fn put_items(pool: &Pool<Sqlite>, profile_id: &str, id: &str, content_ids: &[String], position: Option<i64>) -> Result<Self, String> {
        let group = Self::get(pool, profile_id, id).await?;
        if group.kind != GROUP_MANUAL {
            return Err(format!("{} fills itself from its rule; items cannot be placed in it by hand", group.name));
        }
        let mut wanted: Vec<String> = Vec::new();
        for content_id in content_ids {
            let owned: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM content_item WHERE id = ?1 AND profile_id = ?2")
                .bind(content_id)
                .bind(profile_id)
                .fetch_one(pool)
                .await
                .map_err(|e| format!("Failed to read the content: {}", e))?;
            if owned && !wanted.contains(content_id) {
                wanted.push(content_id.clone());
            }
        }
        let mut ordered: Vec<String> = group.content_ids.into_iter().filter(|c| !wanted.contains(c)).collect();
        let at = position.map(|p| (p.max(0) as usize).min(ordered.len())).unwrap_or(ordered.len());
        ordered.splice(at..at, wanted);
        let mut tx = pool.begin().await.map_err(|e| format!("Failed to place the content: {}", e))?;
        for (index, content_id) in ordered.iter().enumerate() {
            sqlx::query("INSERT INTO content_group_item (group_id, content_id, position) VALUES (?1, ?2, ?3) ON CONFLICT(group_id, content_id) DO UPDATE SET position = excluded.position")
                .bind(id)
                .bind(content_id)
                .bind(index as i64)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to place the content: {}", e))?;
        }
        tx.commit().await.map_err(|e| format!("Failed to place the content: {}", e))?;
        Self::get(pool, profile_id, id).await
    }

    pub async fn remove_items(pool: &Pool<Sqlite>, profile_id: &str, id: &str, content_ids: &[String]) -> Result<Self, String> {
        Self::get(pool, profile_id, id).await?;
        for content_id in content_ids {
            sqlx::query("DELETE FROM content_group_item WHERE group_id = ?1 AND content_id = ?2")
                .bind(id)
                .bind(content_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to take the content out of the group: {}", e))?;
        }
        Self::get(pool, profile_id, id).await
    }

    pub async fn ensure_defaults(pool: &Pool<Sqlite>, profile_id: &str) -> Result<(), String> {
        let placed = sqlx::query("INSERT OR IGNORE INTO app_setting (key, value, updated_at) VALUES (?1, 'placed', ?2)")
            .bind(format!("content.default_groups.{}", profile_id))
            .bind(Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to place the default groups: {}", e))?;
        if placed.rows_affected() == 0 {
            return Ok(());
        }
        for (kind, name, icon) in DEFAULT_GROUPS {
            Self::post(pool, profile_id, &PostContentGroupInput { name: name.to_string(), kind: Some(GROUP_KIND.to_string()), parent_id: None, rule: Some(kind.to_string()), icon: Some(icon.to_string()), content_ids: Vec::new() }).await?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContentSignature {
    pub content_id: String,
    pub provider_id: String,
    pub alias: String,
    pub use_new: bool,
    pub use_reply: bool,
    pub use_forward: bool,
}

pub const SIGNATURE_NEW: &str = "new";
pub const SIGNATURE_REPLY: &str = "reply";
pub const SIGNATURE_FORWARD: &str = "forward";

impl ContentSignature {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> ContentSignature {
        ContentSignature {
            content_id: row.get("content_id"),
            provider_id: row.get("provider_id"),
            alias: row.get("alias"),
            use_new: row.get("use_new"),
            use_reply: row.get("use_reply"),
            use_forward: row.get("use_forward"),
        }
    }

    pub async fn get_all(pool: &Pool<Sqlite>, profile_id: &str, content_id: Option<&str>) -> Result<Vec<ContentSignature>, String> {
        sqlx::query("SELECT s.* FROM content_signature s JOIN content_item c ON c.id = s.content_id WHERE c.profile_id = ?1 AND (?2 IS NULL OR s.content_id = ?2) ORDER BY s.provider_id, s.alias")
            .bind(profile_id)
            .bind(content_id)
            .fetch_all(pool)
            .await
            .map(|rows| rows.into_iter().map(ContentSignature::from_row).collect())
            .map_err(|e| format!("Failed to read the signatures: {}", e))
    }

    pub async fn get_bound(pool: &Pool<Sqlite>, profile_id: &str, provider_id: &str, alias: &str, mode: &str) -> Result<Option<ContentSignature>, String> {
        let column = match mode {
            SIGNATURE_REPLY => "use_reply",
            SIGNATURE_FORWARD => "use_forward",
            _ => "use_new",
        };
        sqlx::query(&format!(
            "SELECT s.* FROM content_signature s JOIN content_item c ON c.id = s.content_id
             WHERE c.profile_id = ?1 AND s.provider_id = ?2 AND (s.alias = ?3 OR s.alias = '') AND s.{} = 1
             ORDER BY (s.alias = ?3) DESC, s.updated_at DESC LIMIT 1",
            column
        ))
        .bind(profile_id)
        .bind(provider_id)
        .bind(alias.to_lowercase())
        .fetch_optional(pool)
        .await
        .map(|row| row.map(ContentSignature::from_row))
        .map_err(|e| format!("Failed to find the signature: {}", e))
    }

    pub async fn put(pool: &Pool<Sqlite>, signature: &ContentSignature) -> Result<(), String> {
        let alias = signature.alias.trim().to_lowercase();
        let clash = [("use_new", signature.use_new), ("use_reply", signature.use_reply), ("use_forward", signature.use_forward)];
        for (column, on) in clash {
            if on {
                sqlx::query(&format!("UPDATE content_signature SET {} = 0 WHERE provider_id = ?1 AND alias = ?2 AND content_id != ?3", column))
                    .bind(&signature.provider_id)
                    .bind(&alias)
                    .bind(&signature.content_id)
                    .execute(pool)
                    .await
                    .map_err(|e| format!("Failed to move the default signature: {}", e))?;
            }
        }
        sqlx::query(
            "INSERT INTO content_signature (content_id, provider_id, alias, use_new, use_reply, use_forward, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(content_id, provider_id, alias) DO UPDATE SET use_new = excluded.use_new, use_reply = excluded.use_reply, use_forward = excluded.use_forward, updated_at = excluded.updated_at",
        )
        .bind(&signature.content_id)
        .bind(&signature.provider_id)
        .bind(&alias)
        .bind(signature.use_new)
        .bind(signature.use_reply)
        .bind(signature.use_forward)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| format!("Failed to bind the signature: {}", e))
    }

    pub async fn delete(pool: &Pool<Sqlite>, content_id: &str, provider_id: &str, alias: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM content_signature WHERE content_id = ?1 AND provider_id = ?2 AND alias = ?3")
            .bind(content_id)
            .bind(provider_id)
            .bind(alias.trim().to_lowercase())
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(|e| format!("Failed to unbind the signature: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_pool, test_profile};

    #[tokio::test]
    async fn reactions_rank_by_use_then_recency_per_profile() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        for emoji in ["🎉", "👍", "🎉", "❤️", "🎉", "👍"] {
            ReactionUse::put(&pool, &profile, emoji).await.unwrap();
        }
        ReactionUse::put(&pool, &profile, "  ").await.unwrap();
        let ranked: Vec<(String, i64)> = ReactionUse::all(&pool, &profile, 10).await.unwrap().into_iter().map(|r| (r.emoji, r.used_count)).collect();
        assert_eq!(ranked, vec![("🎉".to_string(), 3), ("👍".to_string(), 2), ("❤️".to_string(), 1)]);
        assert!(ReactionUse::all(&pool, "other", 10).await.unwrap().is_empty());
        assert_eq!(ReactionUse::all(&pool, &profile, 1).await.unwrap().len(), 1);
    }

    pub fn tiny_gif() -> Vec<u8> {
        let mut bytes = b"GIF89a".to_vec();
        bytes.extend_from_slice(&[0x10, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x3b]);
        bytes
    }

    pub fn tiny_png() -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13];
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&640u32.to_be_bytes());
        bytes.extend_from_slice(&480u32.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes
    }

    pub fn tiny_jpeg() -> Vec<u8> {
        let mut bytes = vec![0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0x00, 0x00];
        bytes.extend_from_slice(&[0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x2a, 0x00, 0x3c, 0x03]);
        bytes.extend_from_slice(&[0; 12]);
        bytes
    }

    pub fn tiny_webp() -> Vec<u8> {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&30u32.to_le_bytes());
        bytes.extend_from_slice(b"WEBPVP8L");
        bytes.extend_from_slice(&13u32.to_le_bytes());
        let bits: u32 = (199) | (99 << 14);
        bytes.push(0x2f);
        bytes.extend_from_slice(&bits.to_le_bytes());
        bytes.extend_from_slice(&[0; 8]);
        bytes
    }

    #[test]
    fn sniffs_mime_and_dimensions() {
        assert_eq!(sniff_mime(&tiny_gif()), Some("image/gif"));
        assert_eq!(dimensions(&tiny_gif()), Some((16, 8)));
        assert_eq!(dimensions(&tiny_png()), Some((640, 480)));
        assert_eq!(dimensions(&tiny_jpeg()), Some((60, 42)));
        assert_eq!(dimensions(&tiny_webp()), Some((200, 100)));
        assert_eq!(sniff_mime(b"hello"), None);
        assert_eq!(dimensions(b"hello"), None);
    }

    async fn gif_item(pool: &Pool<Sqlite>, profile: &str, name: &str, tags: &[&str]) -> ContentItem {
        let tags: Vec<String> = tags.iter().map(|t| t.to_string()).collect();
        ContentItem::insert(pool, profile, KIND_GIF, name, None, &format!("media/{}.gif", name), "image/gif", None, &tags, None, None).await.unwrap()
    }

    fn favourite(id: &str) -> PutContentInput {
        PutContentInput { content_id: id.to_string(), is_favourite: Some(true), ..PutContentInput::default() }
    }

    #[tokio::test]
    async fn library_orders_favourites_then_recent_and_filters() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let old = gif_item(&pool, &profile, "Slow clap", &["applause"]).await;
        let recent = gif_item(&pool, &profile, "Thumbs up", &[]).await;
        let star = gif_item(&pool, &profile, "Party", &[]).await;
        ContentItem::touch(&pool, &recent.id).await.unwrap();
        ContentItem::update(&pool, &favourite(&star.id)).await.unwrap();
        let all = ContentItem::library(&pool, &profile, Some(KIND_GIF), None).await.unwrap();
        assert_eq!(all.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec![star.id.as_str(), recent.id.as_str(), old.id.as_str()]);
        let found = ContentItem::library(&pool, &profile, None, Some("appla")).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, old.id);
        assert!(ContentItem::library(&pool, &profile, Some(KIND_EMOJI), None).await.unwrap().is_empty());
        assert!(ContentItem::insert(&pool, &profile, "audio", "x", None, "p", "audio/mp3", None, &[], None, None).await.is_err());
    }

    #[tokio::test]
    async fn new_favourites_go_first_and_order_is_kept() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let a = gif_item(&pool, &profile, "A", &[]).await;
        let b = gif_item(&pool, &profile, "B", &[]).await;
        let c = gif_item(&pool, &profile, "C", &[]).await;
        for item in [&a, &b, &c] {
            ContentItem::update(&pool, &favourite(&item.id)).await.unwrap();
        }
        let ids = |items: Vec<ContentItem>| items.into_iter().map(|m| m.name).collect::<Vec<_>>();
        assert_eq!(ids(ContentItem::library(&pool, &profile, Some(KIND_GIF), None).await.unwrap()), vec!["C", "B", "A"]);
        ContentItem::put_order(&pool, &profile, &[a.id.clone(), c.id.clone(), b.id.clone()]).await.unwrap();
        assert_eq!(ids(ContentItem::library(&pool, &profile, Some(KIND_GIF), None).await.unwrap()), vec!["A", "C", "B"]);
        ContentItem::update(&pool, &favourite(&c.id)).await.unwrap();
        assert_eq!(ids(ContentItem::library(&pool, &profile, Some(KIND_GIF), None).await.unwrap()), vec!["A", "C", "B"]);
        let off = PutContentInput { content_id: c.id.clone(), is_favourite: Some(false), ..PutContentInput::default() };
        let c = ContentItem::update(&pool, &off).await.unwrap();
        assert_eq!(c.favourite_position, None);
        assert_eq!(ids(ContentItem::library(&pool, &profile, Some(KIND_GIF), None).await.unwrap()), vec!["A", "B", "C"]);
    }

    #[tokio::test]
    async fn library_search_reads_tags_keywords_body_and_saved_query() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let clap = ContentItem::insert(&pool, &profile, KIND_GIF, "clip-1", Some("https://t/1.gif"), "media/1.gif", "image/gif", None, &["applause".to_string()], Some("slow clap"), None).await.unwrap();
        let team = gif_item(&pool, &profile, "clip-2", &[]).await;
        let input = PutContentInput { content_id: team.id.clone(), name: Some("  ".into()), keywords: Some(vec!["Team".into()]), ..PutContentInput::default() };
        let team = ContentItem::update(&pool, &input).await.unwrap();
        assert_eq!(team.name, "clip-2");
        let thanks = ContentItem::insert(&pool, &profile, KIND_REPLY, "Thanks", None, "", "text/plain", None, &[], None, Some("Takk {{ .person.first_name }}, jeg ser på det")).await.unwrap();
        let names = |items: Vec<ContentItem>| items.into_iter().map(|m| m.id).collect::<Vec<_>>();
        assert_eq!(names(ContentItem::library(&pool, &profile, None, Some("CLAP")).await.unwrap()), vec![clap.id.clone()]);
        assert_eq!(names(ContentItem::library(&pool, &profile, None, Some("applause")).await.unwrap()), vec![clap.id.clone()]);
        assert_eq!(names(ContentItem::library(&pool, &profile, None, Some("team")).await.unwrap()), vec![team.id.clone()]);
        assert_eq!(names(ContentItem::library(&pool, &profile, None, Some("ser på")).await.unwrap()), vec![thanks.id.clone()]);
        assert_eq!(ContentItem::get_source(&pool, &profile, "https://t/1.gif").await.unwrap().map(|m| m.id), Some(clap.id));
        assert!(ContentItem::get_source(&pool, &profile, "https://t/2.gif").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn templates_keep_their_text_and_switch_only_between_template_kinds() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let reply = ContentItem::insert(&pool, &profile, KIND_REPLY, "Thanks", None, "", "text/plain", None, &[], None, Some("Takk!")).await.unwrap();
        let edit = PutContentInput { content_id: reply.id.clone(), body: Some("Tusen takk, {{ .person.first_name }}".into()), kind: Some(KIND_EMAIL.into()), ..PutContentInput::default() };
        let email = ContentItem::update(&pool, &edit).await.unwrap();
        assert_eq!((email.kind.as_str(), email.body.as_deref()), (KIND_EMAIL, Some("Tusen takk, {{ .person.first_name }}")));
        let gif = gif_item(&pool, &profile, "Wave", &[]).await;
        assert!(ContentItem::update(&pool, &PutContentInput { content_id: gif.id.clone(), kind: Some(KIND_REPLY.into()), ..PutContentInput::default() }).await.is_err());
        assert!(ContentItem::update(&pool, &PutContentInput { content_id: gif.id.clone(), body: Some("x".into()), ..PutContentInput::default() }).await.is_err());
        assert!(ContentItem::update(&pool, &PutContentInput { content_id: reply.id.clone(), kind: Some(KIND_GIF.into()), ..PutContentInput::default() }).await.is_err());
        assert!(is_template(KIND_PROMPT) && !is_template(KIND_FILE));
    }

    #[tokio::test]
    async fn provenance_records_every_source_and_refuses_unknown_ones() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let item = gif_item(&pool, &profile, "Clip", &[]).await;
        let origin = ContentOrigin { source_kind: Some(SOURCE_PERSON.into()), source_person_id: Some("person-1".into()), source_conversation_id: Some("conversation-1".into()), ..ContentOrigin::default() };
        let item = ContentItem::set_origin(&pool, &item.id, &origin).await.unwrap();
        assert_eq!((item.source_kind.as_deref(), item.source_person_id.as_deref(), item.source_conversation_id.as_deref()), (Some(SOURCE_PERSON), Some("person-1"), Some("conversation-1")));
        let later = ContentOrigin { source_kind: Some(SOURCE_ACTION.into()), source_action_id: Some("action-1".into()), ..ContentOrigin::default() };
        let item = ContentItem::set_origin(&pool, &item.id, &later).await.unwrap();
        assert_eq!((item.source_kind.as_deref(), item.source_action_id.as_deref(), item.source_person_id.as_deref()), (Some(SOURCE_ACTION), Some("action-1"), Some("person-1")));
        assert!(ContentItem::set_origin(&pool, &item.id, &ContentOrigin { source_kind: Some("pigeon".into()), ..ContentOrigin::default() }).await.is_err());
    }

    fn item(name: &str, tags: &[&str], keywords: &[&str], used: i64, last_used: Option<&str>, created: &str) -> ContentItem {
        ContentItem {
            id: name.into(),
            profile_id: "p".into(),
            kind: KIND_GIF.into(),
            name: name.into(),
            source_url: None,
            path: String::new(),
            mime_type: "image/gif".into(),
            width: None,
            height: None,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            is_favourite: false,
            used_count: used,
            last_used_at: last_used.map(String::from),
            created_at: created.into(),
            favourite_position: None,
            query: None,
            keywords: keywords.iter().map(|t| t.to_string()).collect(),
            body: None,
            source_kind: None,
            source_message_id: None,
            source_conversation_id: None,
            source_provider_id: None,
            source_person_id: None,
            source_action_id: None,
            content_hash: None,
            size_bytes: None,
            duration_ms: None,
            group_ids: Vec::new(),
            file_path: None,
        }
    }

    #[test]
    fn ranking_prefers_exact_words_then_use_then_recency() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-14T12:00:00Z").unwrap().with_timezone(&Utc);
        let exact = item("Office party", &["party"], &[], 0, None, "2026-01-01T00:00:00Z");
        let prefix = item("Partytime dance", &[], &[], 0, None, "2026-01-02T00:00:00Z");
        let busy = item("Partytime crowd", &[], &[], 40, Some("2026-10-14T11:00:00Z"), "2026-01-03T00:00:00Z");
        let miss = item("Cat", &["meow"], &[], 99, None, "2026-01-04T00:00:00Z");
        let names = |items: Vec<ContentItem>| items.into_iter().map(|i| i.name).collect::<Vec<_>>();
        let ranked = sort_content(vec![prefix.clone(), miss.clone(), busy.clone(), exact.clone()], Some("party"), SORT_RELEVANCE, now);
        assert_eq!(names(ranked), vec!["Office party", "Partytime crowd", "Partytime dance"]);
        assert_eq!(match_score(&exact, "office party"), Some(5));
        assert_eq!(match_score(&exact, "party dog"), None);
        let keyworded = item("x", &[], &["Monday mood"], 0, None, "2026-01-01T00:00:00Z");
        assert_eq!(match_score(&keyworded, "monday mood"), Some(3));
        assert_eq!(match_score(&keyworded, "monday mood".split(' ').next().unwrap()), Some(2));
        let all = vec![exact.clone(), prefix.clone(), busy.clone(), miss.clone()];
        assert_eq!(names(sort_content(all.clone(), None, SORT_USED, now)), vec!["Cat", "Partytime crowd", "Office party", "Partytime dance"]);
        assert_eq!(names(sort_content(all.clone(), None, SORT_RECENT, now))[0], "Partytime crowd");
        assert_eq!(names(sort_content(all, None, SORT_NEWEST, now)), vec!["Cat", "Partytime crowd", "Partytime dance", "Office party"]);
    }

    #[test]
    fn sort_ties_fall_back_to_favourites_then_newest_then_name() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-14T12:00:00Z").unwrap().with_timezone(&Utc);
        let mut star = item("Zebra", &[], &[], 3, Some("2026-10-01T00:00:00Z"), "2026-01-01T00:00:00Z");
        star.is_favourite = true;
        let late = item("Mango", &[], &[], 3, Some("2026-10-10T00:00:00Z"), "2026-01-01T00:00:00Z");
        let early = item("Apple", &[], &[], 3, Some("2026-09-01T00:00:00Z"), "2026-01-01T00:00:00Z");
        let names = |items: Vec<ContentItem>| items.into_iter().map(|i| i.name).collect::<Vec<_>>();
        assert_eq!(names(sort_content(vec![early.clone(), late.clone(), star.clone()], None, SORT_USED, now)), vec!["Zebra", "Apple", "Mango"]);
        let a = item("Beta", &[], &[], 0, None, "2026-02-01T00:00:00Z");
        let b = item("alpha", &[], &[], 0, None, "2026-02-01T00:00:00Z");
        let c = item("Gamma", &[], &[], 0, None, "2026-03-01T00:00:00Z");
        assert_eq!(names(sort_content(vec![a.clone(), b.clone(), c.clone()], None, SORT_RECENT, now)), vec!["Gamma", "alpha", "Beta"]);
        assert_eq!(names(sort_content(vec![a, b, c], None, SORT_NEWEST, now)), vec!["Gamma", "alpha", "Beta"]);
    }

    #[test]
    fn words_are_normalised_added_and_removed() {
        assert_eq!(normalize_words(&[" Party ".into(), "party".into(), "".into(), "Dog".into()]), vec!["party", "dog"]);
        assert_eq!(edit_words(&["party".into(), "dog".into()], &["Cat".into()], &["DOG".into()]), vec!["party", "cat"]);
        assert_eq!(kind_for_mime("image/gif"), KIND_GIF);
        assert_eq!(kind_for_mime("video/webm"), KIND_VIDEO);
        assert_eq!(kind_for_mime("image/png"), KIND_IMAGE);
        assert_eq!(kind_for_mime("application/pdf"), KIND_FILE);
        assert_eq!(crate::utils::content_hash(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn sniffs_video_containers() {
        let mut mp4 = vec![0, 0, 0, 0x18];
        mp4.extend_from_slice(b"ftypmp42");
        assert_eq!(sniff_mime(&mp4), Some("video/mp4"));
        let mut mov = vec![0, 0, 0, 0x14];
        mov.extend_from_slice(b"ftypqt  ");
        assert_eq!(sniff_mime(&mov), Some("video/quicktime"));
        assert_eq!(sniff_mime(&[0x1a, 0x45, 0xdf, 0xa3, 0, 0]), Some("video/webm"));
        assert_eq!(extension_for("video/quicktime"), "mov");
    }

    fn group(name: &str, parent: Option<&str>) -> PostContentGroupInput {
        PostContentGroupInput { name: name.into(), parent_id: parent.map(String::from), ..PostContentGroupInput::default() }
    }

    #[tokio::test]
    async fn bulk_edits_tags_and_groups_and_counts_tags() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let a = gif_item(&pool, &profile, "A", &["party", "dog"]).await;
        let b = gif_item(&pool, &profile, "B", &["party"]).await;
        let other = test_profile(&pool).await;
        let theirs = gif_item(&pool, &other, "C", &[]).await;
        let team = ContentGroup::post(&pool, &profile, &group("Team", None)).await.unwrap();
        let input = PutContentBulkInput { content_ids: vec![a.id.clone(), b.id.clone(), theirs.id.clone()], add_tags: vec!["Monday".into()], remove_tags: vec!["dog".into()], add_group_ids: vec![team.id.clone()], remove_group_ids: vec![], is_favourite: Some(true) };
        let changed = ContentItem::put_bulk(&pool, &profile, &input).await.unwrap();
        assert_eq!(changed.len(), 2);
        assert_eq!(changed[0].tags, vec!["party".to_string(), "monday".to_string()]);
        assert!(changed.iter().all(|m| m.is_favourite && m.group_ids == vec![team.id.clone()]));
        assert!(ContentItem::get(&pool, &theirs.id).await.unwrap().unwrap().tags.is_empty());
        assert_eq!(ContentGroup::get(&pool, &profile, &team.id).await.unwrap().content_ids, vec![a.id.clone(), b.id.clone()]);
        let tags = ContentItem::tags(&pool, &profile).await.unwrap();
        assert_eq!(tags, vec![ContentTag { name: "monday".into(), count: 2 }, ContentTag { name: "party".into(), count: 2 }]);
        let found = ContentItem::search(&pool, &profile, None, Some("monday"), SORT_RELEVANCE).await.unwrap();
        assert_eq!(found.len(), 2);
    }

    #[tokio::test]
    async fn groups_nest_hold_items_in_many_places_and_keep_their_order() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let x = gif_item(&pool, &profile, "X", &[]).await;
        let y = gif_item(&pool, &profile, "Y", &[]).await;
        let z = gif_item(&pool, &profile, "Z", &[]).await;
        let work = ContentGroup::post(&pool, &profile, &group("Work", None)).await.unwrap();
        let replies = ContentGroup::post(&pool, &profile, &group("Replies", Some(&work.id))).await.unwrap();
        let deep = ContentGroup::post(&pool, &profile, &group("Deep", Some(&replies.id))).await.unwrap();
        ContentGroup::put_items(&pool, &profile, &work.id, &[x.id.clone()], None).await.unwrap();
        ContentGroup::put_items(&pool, &profile, &replies.id, &[y.id.clone(), x.id.clone()], None).await.unwrap();
        ContentGroup::put_items(&pool, &profile, &deep.id, &[x.id.clone()], None).await.unwrap();
        let x_now = ContentItem::get(&pool, &x.id).await.unwrap().unwrap();
        assert_eq!(x_now.group_ids.len(), 3);
        assert!([&work.id, &replies.id, &deep.id].iter().all(|g| x_now.group_ids.contains(g)));
        let reordered = ContentGroup::put_items(&pool, &profile, &replies.id, &[z.id.clone(), x.id.clone()], Some(0)).await.unwrap();
        assert_eq!(reordered.content_ids, vec![z.id.clone(), x.id.clone(), y.id.clone()]);
        assert_eq!(ContentGroup::get(&pool, &profile, &work.id).await.unwrap().content_ids, vec![x.id.clone()]);
        let moved = ContentGroup::put_items(&pool, &profile, &replies.id, &[y.id.clone()], Some(1)).await.unwrap();
        assert_eq!(moved.content_ids, vec![z.id.clone(), y.id.clone(), x.id.clone()]);
        assert!(ContentGroup::put(&pool, &profile, &PutContentGroupInput { content_group_id: work.id.clone(), parent_id: Some(Some(deep.id.clone())), ..PutContentGroupInput::default() }).await.is_err());
        let loose = ContentGroup::post(&pool, &profile, &group("Loose", None)).await.unwrap();
        let placed = ContentGroup::put(&pool, &profile, &PutContentGroupInput { content_group_id: loose.id.clone(), parent_id: Some(Some(deep.id.clone())), position: Some(0), ..PutContentGroupInput::default() }).await.unwrap();
        assert_eq!(placed.parent_id.as_deref(), Some(deep.id.as_str()));
        let first = ContentGroup::put(&pool, &profile, &PutContentGroupInput { content_group_id: loose.id.clone(), parent_id: Some(None), position: Some(0), ..PutContentGroupInput::default() }).await.unwrap();
        assert_eq!((first.parent_id.clone(), first.position), (None, 0));
        assert_eq!(ContentGroup::get(&pool, &profile, &work.id).await.unwrap().position, 1);
        ContentGroup::remove_items(&pool, &profile, &replies.id, &[x.id.clone()]).await.unwrap();
        let x_now = ContentItem::get(&pool, &x.id).await.unwrap().unwrap();
        assert!(x_now.group_ids.contains(&work.id) && x_now.group_ids.contains(&deep.id) && !x_now.group_ids.contains(&replies.id));
        ContentGroup::delete(&pool, &profile, &replies.id).await.unwrap();
        let deep_now = ContentGroup::get(&pool, &profile, &deep.id).await.unwrap();
        assert_eq!(deep_now.parent_id.as_deref(), Some(work.id.as_str()));
        assert_eq!(deep_now.content_ids, vec![x.id.clone()]);
        ContentItem::delete(&pool, &x.id).await.unwrap();
        assert!(ContentGroup::get(&pool, &profile, &work.id).await.unwrap().content_ids.is_empty());
        let stranger = test_profile(&pool).await;
        assert!(ContentGroup::get(&pool, &stranger, &work.id).await.is_err());
        assert!(ContentGroup::put_items(&pool, &stranger, &work.id, &[y.id.clone()], None).await.is_err());
    }

    #[tokio::test]
    async fn automatic_groups_are_placed_once_and_refuse_hand_placed_items() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        ContentGroup::ensure_defaults(&pool, &profile).await.unwrap();
        let groups = ContentGroup::all(&pool, &profile).await.unwrap();
        assert_eq!(groups.len(), DEFAULT_GROUPS.len());
        assert!(groups.iter().all(|g| g.kind == GROUP_KIND && g.rule.is_some()));
        ContentGroup::delete(&pool, &profile, &groups[0].id).await.unwrap();
        ContentGroup::ensure_defaults(&pool, &profile).await.unwrap();
        assert_eq!(ContentGroup::all(&pool, &profile).await.unwrap().len(), DEFAULT_GROUPS.len() - 1);
        let item = gif_item(&pool, &profile, "X", &[]).await;
        assert!(ContentGroup::put_items(&pool, &profile, &groups[1].id, &[item.id.clone()], None).await.is_err());
        assert!(ContentGroup::post(&pool, &profile, &PostContentGroupInput { name: "Bad".into(), kind: Some(GROUP_KIND.into()), rule: Some("audio".into()), ..PostContentGroupInput::default() }).await.is_err());
        let tagged = ContentGroup::post(&pool, &profile, &PostContentGroupInput { name: "Mondays".into(), kind: Some(GROUP_TAG.into()), rule: Some(" Monday ".into()), ..PostContentGroupInput::default() }).await.unwrap();
        assert_eq!(tagged.rule.as_deref(), Some("monday"));
    }
}
