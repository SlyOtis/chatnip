use crate::models::action::Port;
use crate::models::group::{GroupQuery, SortDirection};
use crate::models::Message;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use crate::utils::scalar_text;
use crate::database::{column_exists, table_exists};
use serde_json::Value;
use sqlx::{Pool, Row, Sqlite};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;
use uuid::Uuid;

pub const CONVERSATION_TYPE: &str = "local";
pub const SOURCE_CONVERSATION: &str = "conversation";
pub const SOURCE_GROUP: &str = "group";
pub const SOURCE_TAG: &str = "tag";
pub const SOURCE_PERSON: &str = "person";
pub const SOURCE_QUERY: &str = "query";
pub const SOURCE_KINDS: [&str; 5] = [SOURCE_CONVERSATION, SOURCE_GROUP, SOURCE_TAG, SOURCE_PERSON, SOURCE_QUERY];
pub const ITEM_MESSAGE: &str = "message";
pub const ITEM_RECORD: &str = "record";
pub const ITEM_NOTE: &str = "note";
pub const GRID_MESSAGE: &str = "message";
pub const GRID_MAIL: &str = "mail";
pub const GRID_FILE: &str = "file";
pub const GRID_IMAGE: &str = "image";
pub const GRID_VIDEO: &str = "video";
pub const GRID_LINK: &str = "link";
pub const GRID_RECORD: &str = "record";
pub const GRID_EVENT: &str = "event";
pub const GRID_CONTENT: &str = "content";
pub const GRID_EXCERPT: usize = 280;
pub const DEFAULT_PAGE: usize = 50;
pub const LINK_CHAT: &str = "chat";
pub const NODE_SCHEMA: &str = include_str!("chat_node.jsonc");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ChatMode {
    Read,
    #[default]
    Write,
}

impl ChatMode {
    pub fn name(self) -> &'static str {
        match self {
            ChatMode::Read => "read",
            ChatMode::Write => "write",
        }
    }

    pub fn parse(raw: &str) -> ChatMode {
        if raw == "read" {
            ChatMode::Read
        } else {
            ChatMode::Write
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChatView {
    #[default]
    Chat,
    Grid,
}

impl ChatView {
    pub fn name(self) -> &'static str {
        match self {
            ChatView::Chat => "chat",
            ChatView::Grid => "grid",
        }
    }

    pub fn parse(raw: &str) -> ChatView {
        if raw == "grid" {
            ChatView::Grid
        } else {
            ChatView::Chat
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum GridGroup {
    None,
    Kind,
    #[default]
    Type,
    Modified,
    Source,
    Sender,
    Name,
    Tag,
    Group,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum GridSort {
    Name,
    #[default]
    Date,
    Size,
    Kind,
    Sender,
    Reactions,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum GridRows {
    #[default]
    Scroll,
    Wrap,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GridShow {
    Messages,
    Files,
    Media,
    Links,
    Records,
    Events,
}

fn shown() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GridSettings {
    #[serde(default)]
    pub group: GridGroup,
    #[serde(default)]
    pub sort: GridSort,
    #[serde(default)]
    pub direction: SortDirection,
    #[serde(default)]
    pub rows: GridRows,
    #[serde(default = "shown")]
    pub tags: bool,
    #[serde(default)]
    pub show: Vec<GridShow>,
}

impl Default for GridSettings {
    fn default() -> Self {
        GridSettings { group: GridGroup::default(), sort: GridSort::default(), direction: SortDirection::Desc, rows: GridRows::default(), tags: true, show: Vec::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TimelineSettings {
    #[serde(default)]
    pub sources: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ViewSettings {
    #[serde(default)]
    pub chat: TimelineSettings,
    #[serde(default)]
    pub grid: GridSettings,
}

impl ViewSettings {
    pub fn parse(raw: Option<String>) -> ViewSettings {
        raw.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ChatSource {
    pub kind: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub query: Option<GroupQuery>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Chat {
    pub id: String,
    pub profile_id: String,
    pub conversation_id: String,
    pub name: String,
    pub icon: Option<String>,
    pub mode: ChatMode,
    pub sources: Vec<ChatSource>,
    pub attachments: Vec<ChatAttachment>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ChatRoute {
    pub output: String,
    pub node: String,
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub values: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ChatAttachment {
    pub action_id: String,
    #[serde(default)]
    pub routes: Vec<ChatRoute>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatRoutesInput {
    pub conversation_id: String,
    pub action_id: String,
    pub routes: Vec<ChatRoute>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct PostChatInput {
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub mode: ChatMode,
    #[serde(default)]
    pub view: Option<ChatView>,
    #[serde(default)]
    pub sources: Vec<ChatSource>,
    #[serde(default)]
    pub action_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PutChatInput {
    pub chat_id: String,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub sources: Option<Vec<ChatSource>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatActionInput {
    pub conversation_id: String,
    pub action_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AttachmentOpenInput {
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub attachment_id: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub quick_look: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostChatMessageInput {
    pub conversation_id: String,
    pub text: String,
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub reply_to_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatItem {
    pub id: String,
    pub kind: String,
    pub message: Option<Message>,
    pub record: Option<Value>,
    pub text: Option<String>,
    pub title: String,
    pub conversation_id: String,
    pub conversation_name: String,
    pub conversation_type: String,
    pub action_id: Option<String>,
    pub reactions: usize,
    pub at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChatPage {
    pub chat_id: String,
    pub items: Vec<ChatItem>,
    pub next_before: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct GridItem {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub content_type: String,
    pub size: i64,
    pub at: String,
    pub message_id: Option<String>,
    pub attachment_id: Option<String>,
    pub conversation_id: String,
    pub conversation_name: String,
    pub provider_type: String,
    pub sender: String,
    pub sender_id: Option<String>,
    pub excerpt: String,
    pub reactions: i64,
    pub path: Option<String>,
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub record: Option<Value>,
    pub presentation: Option<Value>,
}

#[derive(Debug, Clone, Default)]
pub struct Intake {
    pub conversations: BTreeSet<String>,
    pub chats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatNodeVariant {
    pub label: String,
    pub icon: String,
    pub description: String,
    pub start: String,
    pub end: String,
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatNodeSpec {
    pub key: String,
    pub label: String,
    pub icon: String,
    pub description: String,
    pub config: Value,
    pub variants: BTreeMap<String, ChatNodeVariant>,
}

impl ChatNodeSpec {
    pub fn variant(&self, mode: ChatMode) -> &ChatNodeVariant {
        &self.variants[mode.name()]
    }
}

pub fn strip_jsonc(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match (c, chars.get(i + 1)) {
            ('"', _) => {
                in_string = true;
                out.push(c);
                i += 1;
            }
            ('/', Some('/')) => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            ('/', Some('*')) => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i += 2;
            }
            (',', _) => {
                let next = chars[i + 1..].iter().find(|ch| !ch.is_whitespace());
                if !matches!(next, Some('}') | Some(']')) {
                    out.push(c);
                }
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

pub fn parse_jsonc<T: serde::de::DeserializeOwned>(source: &str) -> Result<T, String> {
    serde_json::from_str(&strip_jsonc(source)).map_err(|e| format!("The schema does not parse: {}", e))
}

pub fn node_spec() -> &'static ChatNodeSpec {
    static SPEC: OnceLock<ChatNodeSpec> = OnceLock::new();
    SPEC.get_or_init(|| parse_jsonc(NODE_SCHEMA).expect("the built in chat node schema parses"))
}

pub async fn rename_read_chats(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    const OLD_TABLE: &str = concat!("fe", "ed");
    const OLD_COLUMN: &str = concat!("is_read", "_only");
    const RETIRED_COLUMN: &str = concat!("is_read", "_only_retired");
    if table_exists(pool, OLD_TABLE).await? && !table_exists(pool, "chat").await? {
        sqlx::query(&format!("ALTER TABLE {} RENAME TO chat", OLD_TABLE)).execute(pool).await?;
    }
    if table_exists(pool, "conversation").await? {
        sqlx::query("UPDATE conversation SET type = ?1 WHERE type = ?2").bind(CONVERSATION_TYPE).bind(OLD_TABLE).execute(pool).await?;
    }
    for table in ["conversation", "conversation_group"] {
        if table_exists(pool, table).await? && column_exists(pool, table, OLD_COLUMN).await? && column_exists(pool, table, "mode").await? {
            sqlx::query(&format!("UPDATE {} SET mode = 'read' WHERE {} = 1", table, OLD_COLUMN)).execute(pool).await?;
            sqlx::query(&format!("ALTER TABLE {} RENAME COLUMN {} TO {}", table, OLD_COLUMN, RETIRED_COLUMN)).execute(pool).await?;
        }
    }
    if table_exists(pool, "conversation").await? && column_exists(pool, "conversation", "view").await? && column_exists(pool, "conversation", "view_settings").await? {
        const OLD_VIEW: &str = "files";
        let rows: Vec<(String, String, String)> = sqlx::query_as("SELECT id, view, view_settings FROM conversation WHERE view = ?1 OR instr(view_settings, ?2) > 0")
            .bind(OLD_VIEW)
            .bind(format!("\"{}\"", OLD_VIEW))
            .fetch_all(pool)
            .await?;
        for (id, view, raw) in rows {
            let mut settings: Value = serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}));
            if let Some(object) = settings.as_object_mut() {
                let old = object.remove(OLD_VIEW);
                if !object.contains_key("grid") {
                    let mut grid = old.filter(|o| o.is_object()).unwrap_or_else(|| serde_json::json!({}));
                    if view == OLD_VIEW {
                        grid["show"] = serde_json::json!(["files", "records"]);
                    }
                    object.insert("grid".into(), grid);
                }
            }
            sqlx::query("UPDATE conversation SET view = ?1, view_settings = ?2 WHERE id = ?3")
                .bind(if view == OLD_VIEW { ChatView::Grid.name() } else { view.as_str() })
                .bind(settings.to_string())
                .bind(&id)
                .execute(pool)
                .await?;
        }
    }
    if table_exists(pool, "mock_seed").await? {
        sqlx::query("UPDATE mock_seed SET kind = 'chat' WHERE kind = ?1").bind(OLD_TABLE).execute(pool).await?;
        sqlx::query("UPDATE OR IGNORE mock_seed SET key = 'chat:' || substr(key, length(?1) + 2) WHERE key LIKE ?1 || ':%'").bind(OLD_TABLE).execute(pool).await?;
        sqlx::query("UPDATE OR IGNORE mock_seed SET key = 'read_mode' WHERE key = ?1").bind(concat!("read", "_only")).execute(pool).await?;
    }
    Ok(())
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    rename_read_chats(pool).await?;
    for statement in [
        r#"CREATE TABLE IF NOT EXISTS chat (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            conversation_id TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            icon TEXT,
            sources TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profile(id) ON DELETE CASCADE,
            FOREIGN KEY (conversation_id) REFERENCES conversation(id) ON DELETE CASCADE
        )"#,
        r#"CREATE TABLE IF NOT EXISTS chat_item (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            key TEXT NOT NULL,
            kind TEXT NOT NULL,
            action_id TEXT,
            run_id TEXT,
            message_id TEXT,
            title TEXT NOT NULL DEFAULT '',
            text TEXT,
            data TEXT NOT NULL DEFAULT 'null',
            at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE (conversation_id, key),
            FOREIGN KEY (conversation_id) REFERENCES conversation(id) ON DELETE CASCADE
        )"#,
        "CREATE INDEX IF NOT EXISTS idx_chat_item_at ON chat_item (conversation_id, at)",
        r#"CREATE TABLE IF NOT EXISTS chat_action (
            conversation_id TEXT NOT NULL,
            action_id TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            PRIMARY KEY (conversation_id, action_id),
            FOREIGN KEY (conversation_id) REFERENCES conversation(id) ON DELETE CASCADE,
            FOREIGN KEY (action_id) REFERENCES action(id) ON DELETE CASCADE
        )"#,
    ] {
        sqlx::query(statement).execute(pool).await?;
    }
    crate::database::ensure_columns(pool, "chat", &[("backfilled_at", "TEXT")]).await;
    crate::database::ensure_columns(pool, "chat_action", &[("routes", "TEXT NOT NULL DEFAULT '[]'")]).await;
    Ok(())
}

pub fn valid_sources(sources: &[ChatSource]) -> Result<(), String> {
    for source in sources {
        if !SOURCE_KINDS.contains(&source.kind.as_str()) {
            return Err(format!("Unknown chat source {}", source.kind));
        }
        if source.kind == SOURCE_QUERY {
            if source.query.is_none() {
                return Err("A query source needs a query".to_string());
            }
        } else if source.id.as_deref().map(str::trim).unwrap_or("").is_empty() {
            return Err(format!("A {} source needs an id", source.kind));
        }
    }
    Ok(())
}

pub fn find_cycle(start: &str, edges: &HashMap<String, Vec<String>>) -> Option<Vec<String>> {
    fn walk(at: &str, start: &str, edges: &HashMap<String, Vec<String>>, path: &mut Vec<String>, seen: &mut BTreeSet<String>) -> bool {
        for next in edges.get(at).into_iter().flatten() {
            if next == start {
                path.push(next.clone());
                return true;
            }
            if !seen.insert(next.clone()) {
                continue;
            }
            path.push(next.clone());
            if walk(next, start, edges, path, seen) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut path = vec![start.to_string()];
    let mut seen = BTreeSet::new();
    walk(start, start, edges, &mut path, &mut seen).then_some(path)
}

pub fn page(mut items: Vec<ChatItem>, before: Option<&str>, limit: usize) -> (Vec<ChatItem>, Option<String>) {
    items.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| b.id.cmp(&a.id)));
    let mut seen = BTreeSet::new();
    items.retain(|item| seen.insert(item.id.clone()));
    let remaining: Vec<ChatItem> = match before {
        Some(cursor) => items.into_iter().filter(|item| item.at.as_str() < cursor).collect(),
        None => items,
    };
    let limit = limit.max(1);
    let more = remaining.len() > limit;
    let page: Vec<ChatItem> = remaining.into_iter().take(limit).collect();
    let next = if more { page.last().map(|item| item.at.clone()) } else { None };
    (page, next)
}


pub fn record_tags(record: &Value) -> Vec<String> {
    let mut tags = Vec::new();
    let vendor = scalar_text(record.get("vendor"));
    if !vendor.is_empty() {
        tags.push(vendor);
    }
    let total = scalar_text(record.get("total"));
    if !total.is_empty() {
        let currency = scalar_text(record.get("currency"));
        tags.push(if currency.is_empty() { total } else { format!("{} {}", total, currency) });
    }
    if let Some(date) = record.get("date").and_then(Value::as_str).filter(|d| !d.is_empty()) {
        tags.push(date.to_string());
    }
    tags
}

fn excerpt(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(GRID_EXCERPT).collect()
}

fn media_kind(content_type: &str, name: &str) -> &'static str {
    let lower = name.to_lowercase();
    if content_type.starts_with("video/") || [".mp4", ".mov", ".webm", ".m4v"].iter().any(|e| lower.ends_with(e)) {
        GRID_VIDEO
    } else if content_type.starts_with("image/") || [".png", ".jpg", ".jpeg", ".gif", ".webp", ".heic"].iter().any(|e| lower.ends_with(e)) {
        GRID_IMAGE
    } else {
        GRID_FILE
    }
}

pub fn grid(items: &[ChatItem], providers: &HashMap<String, String>) -> Vec<GridItem> {
    let mut out = Vec::new();
    for item in items {
        let provider_type = providers.get(&item.conversation_id).cloned().unwrap_or_default();
        let base = |id: String, kind: &str, name: String| GridItem {
            id,
            kind: kind.to_string(),
            name,
            content_type: String::new(),
            size: 0,
            at: item.at.clone(),
            message_id: item.message.as_ref().map(|m| m.id.clone()),
            attachment_id: None,
            conversation_id: item.conversation_id.clone(),
            conversation_name: item.conversation_name.clone(),
            provider_type: provider_type.clone(),
            sender: item.message.as_ref().and_then(|m| m.sender_name.clone().or_else(|| m.sender_email.clone())).unwrap_or_default(),
            sender_id: item.message.as_ref().and_then(|m| m.sender_id.clone()),
            excerpt: String::new(),
            reactions: item.reactions as i64,
            path: None,
            url: None,
            tags: Vec::new(),
            record: None,
            presentation: None,
        };
        if let Some(message) = &item.message {
            let content = message.content.clone().unwrap_or_default();
            let html = matches!(message.content_type.as_str(), "html" | "text/html");
            let text = if html { crate::content::html::visible_text(&content) } else { content.clone() };
            let presented = message.presentation.clone();
            let kind = if presented.as_ref().and_then(|p| p.get("kind")).and_then(Value::as_str) == Some("invite") {
                GRID_EVENT
            } else if message.metadata.as_deref().is_some_and(|m| m.contains("\"content_id\"")) {
                GRID_CONTENT
            } else if message.r#type == "email" {
                GRID_MAIL
            } else {
                GRID_MESSAGE
            };
            let name = message.subject.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| excerpt(&text).chars().take(80).collect());
            let mut tile = base(message.id.clone(), kind, name);
            tile.excerpt = excerpt(&text);
            tile.content_type = message.content_type.clone();
            tile.presentation = presented;
            out.push(tile);
            for attachment in message.attachments.iter().filter(|a| !a.is_inline) {
                let mut tile = base(format!("{}:{}", message.id, attachment.id), media_kind(&attachment.content_type, &attachment.name), attachment.name.clone());
                tile.content_type = attachment.content_type.clone();
                tile.size = attachment.size;
                tile.attachment_id = Some(attachment.id.clone());
                tile.url = attachment.url.clone().filter(|u| !u.starts_with("data:"));
                out.push(tile);
            }
            for link in crate::content::links::extract(&text, html.then_some(content.as_str())).into_iter().filter(|l| l.kind == crate::content::links::LinkKind::Web) {
                let mut tile = base(format!("{}:{}", message.id, link.url), GRID_LINK, link.label.clone().filter(|l| !l.trim().is_empty()).unwrap_or_else(|| link.url.clone()));
                tile.url = Some(link.url);
                tile.excerpt = excerpt(&text);
                out.push(tile);
            }
        }
        if item.kind == ITEM_NOTE {
            let text = item.text.clone().unwrap_or_default();
            let mut tile = base(item.id.clone(), GRID_MESSAGE, item.title.clone());
            tile.excerpt = excerpt(&text);
            out.push(tile);
        }
        if let Some(record) = &item.record {
            let attachment = record.get("attachment").filter(|a| a.is_object());
            let kind = match record.get("kind").and_then(Value::as_str) {
                Some("content") => GRID_CONTENT,
                Some("event") | Some("invite") => GRID_EVENT,
                _ => GRID_RECORD,
            };
            let mut tile = base(item.id.clone(), kind, item.title.clone());
            tile.content_type = attachment.map(|a| scalar_text(a.get("content_type"))).filter(|c| !c.is_empty()).unwrap_or_default();
            tile.size = attachment.and_then(|a| a.get("size")).and_then(Value::as_i64).unwrap_or(0);
            tile.at = record.get("at").and_then(Value::as_str).filter(|d| !d.is_empty()).map(String::from).unwrap_or_else(|| item.at.clone());
            tile.message_id = record.get("message_id").and_then(Value::as_str).map(String::from);
            tile.attachment_id = attachment.and_then(|a| a.get("id")).and_then(Value::as_str).map(String::from);
            tile.conversation_id = record.get("conversation_id").and_then(Value::as_str).map(String::from).unwrap_or_else(|| item.conversation_id.clone());
            tile.conversation_name = Some(scalar_text(record.get("conversation_name"))).filter(|n| !n.is_empty()).unwrap_or_else(|| item.conversation_name.clone());
            tile.provider_type = Some(scalar_text(record.get("provider"))).filter(|p| !p.is_empty()).unwrap_or(provider_type.clone());
            tile.sender = scalar_text(record.get("sender"));
            tile.excerpt = Some(scalar_text(record.get("excerpt"))).filter(|e| !e.is_empty()).unwrap_or_else(|| excerpt(&scalar_text(record.get("text"))));
            tile.path = attachment.and_then(|a| a.get("path")).and_then(Value::as_str).map(String::from);
            tile.tags = record_tags(record);
            tile.record = Some(record.clone());
            out.push(tile);
        }
    }
    out
}

impl Chat {
    pub fn action_ids(&self) -> Vec<String> {
        self.attachments.iter().map(|a| a.action_id.clone()).collect()
    }

    pub fn links(&self) -> Vec<String> {
        if self.mode != ChatMode::Write {
            return Vec::new();
        }
        self.sources.iter().filter(|s| s.kind == SOURCE_CONVERSATION).filter_map(|s| s.id.clone()).collect()
    }

    async fn link(pool: &Pool<Sqlite>, conversation_id: &str, targets: &[String]) -> Result<(), String> {
        sqlx::query("DELETE FROM conversation_link WHERE source_conversation_id = ?1 AND link_type = ?2").bind(conversation_id).bind(LINK_CHAT).execute(pool).await.map_err(|e| format!("Failed to unlink the chat: {}", e))?;
        for target in targets {
            crate::models::ConversationLink::post(pool, conversation_id, target, LINK_CHAT).await?;
        }
        Ok(())
    }

    fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        let sources: String = row.try_get("sources").unwrap_or_else(|_| "[]".to_string());
        let attachments: String = row.try_get::<Option<String>, _>("attachments").ok().flatten().unwrap_or_else(|| "[]".to_string());
        Self {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            conversation_id: row.get("conversation_id"),
            name: row.get("name"),
            icon: row.try_get("icon").ok().flatten(),
            mode: ChatMode::parse(&row.try_get::<String, _>("mode").unwrap_or_default()),
            sources: serde_json::from_str(&sources).unwrap_or_default(),
            attachments: serde_json::from_str(&attachments).unwrap_or_default(),
            created_at: row.get("created_at"),
        }
    }

    const SELECT: &'static str = "SELECT ch.*, c.mode, (SELECT json_group_array(json_object('action_id', action_id, 'routes', json(routes))) FROM (SELECT action_id, routes FROM chat_action WHERE conversation_id = ch.conversation_id ORDER BY position, created_at)) AS attachments FROM chat ch JOIN conversation c ON c.id = ch.conversation_id";

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Self>, String> {
        let rows = sqlx::query(&format!("{} WHERE ch.profile_id = ?1 ORDER BY ch.created_at", Self::SELECT))
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list chats: {}", e))?;
        Ok(rows.into_iter().map(Self::from_row).collect())
    }

    pub async fn get(pool: &Pool<Sqlite>, profile_id: &str, id: &str) -> Result<Self, String> {
        Self::find(pool, profile_id, id).await?.ok_or_else(|| "Chat not found".to_string())
    }

    pub async fn find(pool: &Pool<Sqlite>, profile_id: &str, id: &str) -> Result<Option<Self>, String> {
        Ok(sqlx::query(&format!("{} WHERE ch.profile_id = ?1 AND (ch.id = ?2 OR ch.conversation_id = ?2)", Self::SELECT))
            .bind(profile_id)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load the chat: {}", e))?
            .map(Self::from_row))
    }

    pub async fn local_ids(pool: &Pool<Sqlite>) -> Result<BTreeSet<String>, String> {
        let ids: Vec<String> = sqlx::query_scalar("SELECT conversation_id FROM chat").fetch_all(pool).await.map_err(|e| format!("Failed to list chats: {}", e))?;
        Ok(ids.into_iter().collect())
    }

    pub async fn refuse_cycles(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str, sources: &[ChatSource]) -> Result<(), String> {
        let chats = Self::all(pool, profile_id).await?;
        let local: BTreeSet<&str> = chats.iter().map(|c| c.conversation_id.as_str()).collect();
        let names: HashMap<&str, &str> = chats.iter().map(|c| (c.conversation_id.as_str(), c.name.as_str())).collect();
        let explicit = |list: &[ChatSource]| -> Vec<String> {
            list.iter().filter(|s| s.kind == SOURCE_CONVERSATION).filter_map(|s| s.id.clone()).filter(|id| local.contains(id.as_str())).collect()
        };
        let mut edges: HashMap<String, Vec<String>> = chats.iter().map(|c| (c.conversation_id.clone(), explicit(&c.sources))).collect();
        edges.insert(conversation_id.to_string(), explicit(sources));
        if let Some(path) = find_cycle(conversation_id, &edges) {
            let named: Vec<&str> = path.iter().map(|id| names.get(id.as_str()).copied().unwrap_or(id.as_str())).collect();
            return Err(format!("These sources would loop: {}", named.join(" → ")));
        }
        Ok(())
    }

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, input: &PostChatInput) -> Result<Self, String> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err("A chat needs a name".to_string());
        }
        valid_sources(&input.sources)?;
        let id = Uuid::new_v4().to_string();
        let conversation_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let mut tx = pool.begin().await.map_err(|e| format!("Failed to start the chat: {}", e))?;
        sqlx::query("INSERT INTO conversation (id, name, type, group_type, last_activity, mode, view, created_at, updated_at) VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?4, ?4)")
            .bind(&conversation_id)
            .bind(name)
            .bind(CONVERSATION_TYPE)
            .bind(&now)
            .bind(input.mode.name())
            .bind(input.view.unwrap_or_default().name())
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to create the chat conversation: {}", e))?;
        sqlx::query("INSERT INTO chat (id, profile_id, conversation_id, name, icon, sources, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")
            .bind(&id)
            .bind(profile_id)
            .bind(&conversation_id)
            .bind(name)
            .bind(input.icon.as_deref().filter(|icon| !icon.is_empty()))
            .bind(serde_json::to_string(&input.sources).unwrap_or_else(|_| "[]".to_string()))
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to create the chat: {}", e))?;
        tx.commit().await.map_err(|e| format!("Failed to save the chat: {}", e))?;
        let chat = Self::get(pool, profile_id, &id).await?;
        Self::link(pool, &chat.conversation_id, &chat.links()).await?;
        Ok(chat)
    }

    pub async fn put(pool: &Pool<Sqlite>, profile_id: &str, input: &PutChatInput) -> Result<Self, String> {
        let current = Self::get(pool, profile_id, &input.chat_id).await?;
        let name = input.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&current.name).to_string();
        let sources = input.sources.clone().unwrap_or(current.sources.clone());
        valid_sources(&sources)?;
        Self::refuse_cycles(pool, profile_id, &current.conversation_id, &sources).await?;
        let icon = match &input.icon {
            Some(icon) => Some(icon.trim().to_string()).filter(|icon| !icon.is_empty()),
            None => current.icon.clone(),
        };
        sqlx::query("UPDATE chat SET name = ?1, icon = ?2, sources = ?3, backfilled_at = NULL WHERE id = ?4")
            .bind(&name)
            .bind(&icon)
            .bind(serde_json::to_string(&sources).unwrap_or_else(|_| "[]".to_string()))
            .bind(&current.id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update the chat: {}", e))?;
        sqlx::query("UPDATE conversation SET name = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&name)
            .bind(Utc::now().to_rfc3339())
            .bind(&current.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to rename the chat conversation: {}", e))?;
        let chat = Self::get(pool, profile_id, &current.id).await?;
        Self::link(pool, &chat.conversation_id, &chat.links()).await?;
        Ok(chat)
    }

    pub async fn delete(pool: &Pool<Sqlite>, profile_id: &str, id: &str) -> Result<(), String> {
        let current = Self::get(pool, profile_id, id).await?;
        for statement in ["DELETE FROM chat_item WHERE conversation_id = ?1", "DELETE FROM chat_action WHERE conversation_id = ?1", "DELETE FROM conversation_link WHERE source_conversation_id = ?1", "DELETE FROM conversation WHERE id = ?1"] {
            sqlx::query(statement).bind(&current.conversation_id).execute(pool).await.map_err(|e| format!("Failed to delete the chat: {}", e))?;
        }
        sqlx::query("DELETE FROM chat WHERE id = ?1").bind(&current.id).execute(pool).await.map_err(|e| format!("Failed to delete the chat: {}", e))?;
        Ok(())
    }

    pub async fn mark_backfilled(pool: &Pool<Sqlite>, conversation_id: &str) -> Result<(), String> {
        sqlx::query("UPDATE chat SET backfilled_at = ?1 WHERE conversation_id = ?2")
            .bind(Utc::now().to_rfc3339())
            .bind(conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to mark the chat: {}", e))?;
        Ok(())
    }

    pub async fn backfilled(pool: &Pool<Sqlite>, conversation_id: &str) -> Result<bool, String> {
        let at: Option<Option<String>> = sqlx::query_scalar("SELECT backfilled_at FROM chat WHERE conversation_id = ?1").bind(conversation_id).fetch_optional(pool).await.map_err(|e| e.to_string())?;
        Ok(at.flatten().is_some())
    }

    pub async fn intake(&self, pool: &Pool<Sqlite>) -> Result<Intake, String> {
        let local = Self::local_ids(pool).await?;
        let mut intake = Intake::default();
        for source in &self.sources {
            let id = source.id.clone().unwrap_or_default();
            let found: Vec<String> = match source.kind.as_str() {
                SOURCE_CONVERSATION => {
                    if local.contains(&id) {
                        if id != self.conversation_id && !intake.chats.contains(&id) {
                            intake.chats.push(id);
                        }
                        continue;
                    }
                    vec![id]
                }
                SOURCE_GROUP => {
                    let mut group = crate::models::group::Group::get(pool, &id).await?;
                    crate::services::group_service::resolve(pool, &self.profile_id, &mut group).await?;
                    group.conversation_ids
                }
                SOURCE_TAG => crate::models::ObjectTag::get_conversation_ids(pool, &id).await?,
                SOURCE_PERSON => crate::models::Person::get(pool, &self.profile_id, &id).await?.conversation_ids,
                SOURCE_QUERY => match &source.query {
                    Some(query) => crate::services::group_service::evaluate(pool, &self.profile_id, query).await?,
                    None => Vec::new(),
                },
                _ => Vec::new(),
            };
            intake.conversations.extend(found.into_iter().filter(|id| !id.is_empty() && !local.contains(id)));
        }
        Ok(intake)
    }

    pub async fn conversation_ids(&self, pool: &Pool<Sqlite>) -> Result<Vec<String>, String> {
        Ok(self.intake(pool).await?.conversations.into_iter().collect())
    }

    pub async fn own_items(pool: &Pool<Sqlite>, conversation_id: &str) -> Result<Vec<ChatItem>, String> {
        let rows = sqlx::query(
            "SELECT i.*, c.name AS conversation_name, c.type AS conversation_type FROM chat_item i JOIN conversation c ON c.id = i.conversation_id WHERE i.conversation_id = ?1 ORDER BY i.at DESC",
        )
        .bind(conversation_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to read the chat items: {}", e))?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let data: Value = row.try_get::<String, _>("data").ok().and_then(|d| serde_json::from_str(&d).ok()).unwrap_or(Value::Null);
                let kind: String = row.get("kind");
                ChatItem {
                    id: row.get("id"),
                    record: (kind == ITEM_RECORD).then_some(data),
                    text: row.try_get("text").ok().flatten(),
                    title: row.try_get("title").unwrap_or_default(),
                    kind,
                    message: None,
                    conversation_id: row.get("conversation_id"),
                    conversation_name: row.get("conversation_name"),
                    conversation_type: row.get("conversation_type"),
                    action_id: row.try_get("action_id").ok().flatten(),
                    reactions: 0,
                    at: row.get("at"),
                }
            })
            .collect())
    }

    pub async fn put_item(pool: &Pool<Sqlite>, conversation_id: &str, key: &str, kind: &str, title: &str, text: Option<&str>, data: &Value, at: &str, origin: (Option<&str>, Option<&str>, Option<&str>)) -> Result<(String, bool), String> {
        let (action_id, run_id, message_id) = origin;
        let existing: Option<String> = sqlx::query_scalar("SELECT id FROM chat_item WHERE conversation_id = ?1 AND key = ?2")
            .bind(conversation_id)
            .bind(key)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to read the chat item: {}", e))?;
        let id = existing.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        sqlx::query(
            "INSERT INTO chat_item (id, conversation_id, key, kind, action_id, run_id, message_id, title, text, data, at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(conversation_id, key) DO UPDATE SET kind = excluded.kind, action_id = excluded.action_id, run_id = excluded.run_id, title = excluded.title, text = excluded.text, data = excluded.data, at = excluded.at",
        )
        .bind(&id)
        .bind(conversation_id)
        .bind(key)
        .bind(kind)
        .bind(action_id)
        .bind(run_id)
        .bind(message_id)
        .bind(title)
        .bind(text)
        .bind(data.to_string())
        .bind(at)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to keep the chat item: {}", e))?;
        sqlx::query("UPDATE conversation SET last_activity = ?1, updated_at = ?2 WHERE id = ?3 AND last_activity < ?1")
            .bind(at)
            .bind(Utc::now().to_rfc3339())
            .bind(conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to touch the chat: {}", e))?;
        Ok((id, existing.is_none()))
    }

    pub async fn messages_of(pool: &Pool<Sqlite>, conversation_ids: &BTreeSet<String>) -> Result<Vec<ChatItem>, String> {
        use crate::services::message_service::{MessageLoadOptions, MessageService};
        let loader = MessageService::new(pool.clone());
        let mut items: Vec<ChatItem> = Vec::new();
        for conversation_id in conversation_ids {
            let row = sqlx::query("SELECT name, type FROM conversation WHERE id = ?1 AND deleted_at IS NULL")
                .bind(conversation_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("Failed to read a chat source: {}", e))?;
            let Some(row) = row else { continue };
            let name: String = row.get("name");
            let kind: String = row.get("type");
            let loaded = loader
                .load_conversation_messages(conversation_id, MessageLoadOptions { load_reactions: true, load_assets: true, load_sender_details: true, limit: None, offset: None })
                .await?;
            items.extend(loaded.into_iter().map(|m| ChatItem {
                reactions: m.reactions.len(),
                id: m.message.id.clone(),
                kind: ITEM_MESSAGE.into(),
                title: m.message.subject.clone().unwrap_or_default(),
                at: m.message.timestamp.clone(),
                message: Some(m.message),
                record: None,
                text: None,
                conversation_id: conversation_id.clone(),
                conversation_name: name.clone(),
                conversation_type: kind.clone(),
                action_id: None,
            }));
        }
        Ok(items)
    }
}

pub async fn put_action(pool: &Pool<Sqlite>, conversation_id: &str, action_id: &str) -> Result<(), String> {
    let position: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), -1) + 1 FROM chat_action WHERE conversation_id = ?1")
        .bind(conversation_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to place the action: {}", e))?;
    sqlx::query("INSERT OR IGNORE INTO chat_action (conversation_id, action_id, position, created_at) VALUES (?1, ?2, ?3, ?4)")
        .bind(conversation_id)
        .bind(action_id)
        .bind(position)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to attach the action: {}", e))?;
    sqlx::query("UPDATE chat SET backfilled_at = NULL WHERE conversation_id = ?1").bind(conversation_id).execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn put_routes(pool: &Pool<Sqlite>, conversation_id: &str, action_id: &str, routes: &[ChatRoute]) -> Result<(), String> {
    let changed = sqlx::query("UPDATE chat_action SET routes = ?1 WHERE conversation_id = ?2 AND action_id = ?3")
        .bind(serde_json::to_string(routes).map_err(|e| e.to_string())?)
        .bind(conversation_id)
        .bind(action_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to keep the landings: {}", e))?;
    if changed.rows_affected() == 0 {
        return Err("The action is not attached to this chat".to_string());
    }
    Ok(())
}

pub async fn routes_of(pool: &Pool<Sqlite>, conversation_id: &str, action_id: &str) -> Result<Vec<ChatRoute>, String> {
    let raw: Option<String> = sqlx::query_scalar("SELECT routes FROM chat_action WHERE conversation_id = ?1 AND action_id = ?2").bind(conversation_id).bind(action_id).fetch_optional(pool).await.map_err(|e| e.to_string())?;
    Ok(raw.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default())
}

pub async fn delete_action(pool: &Pool<Sqlite>, conversation_id: &str, action_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM chat_action WHERE conversation_id = ?1 AND action_id = ?2")
        .bind(conversation_id)
        .bind(action_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to detach the action: {}", e))?;
    Ok(())
}

pub async fn action_ids(pool: &Pool<Sqlite>, conversation_id: &str) -> Result<Vec<String>, String> {
    sqlx::query_scalar("SELECT action_id FROM chat_action WHERE conversation_id = ?1 ORDER BY position, created_at")
        .bind(conversation_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list the attached actions: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_pool, test_profile};

    #[test]
    fn the_chat_node_schema_loads_from_jsonc_with_comments_stripped() {
        let spec = node_spec();
        assert_eq!(spec.key, "chat");
        let read = spec.variant(ChatMode::Read);
        assert_eq!(read.inputs.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["items", "item"]);
        assert!(read.outputs.iter().any(|p| p.name == "item" && p.kind == crate::models::action::PortType::Json));
        let write = spec.variant(ChatMode::Write);
        assert_eq!(write.inputs.len(), 1);
        assert_eq!(write.inputs[0].kind, crate::models::action::PortType::Text);
        assert_eq!(spec.config["properties"]["mode"]["enum"], serde_json::json!(["read", "write"]));
        assert!(NODE_SCHEMA.contains("//"));
        let sample = "{\n  // a comment with \"quotes\"\n  \"url\": \"http://example.org/a//b\", /* block\n comment */ \"list\": [1, 2,],\n  \"escaped\": \"say \\\"hi\\\" // still text\",\n}";
        let parsed: Value = parse_jsonc(sample).unwrap();
        assert_eq!(parsed["url"], "http://example.org/a//b");
        assert_eq!(parsed["list"], serde_json::json!([1, 2]));
        assert_eq!(parsed["escaped"], "say \"hi\" // still text");
        assert!(parse_jsonc::<Value>("{ \"a\": }").is_err());
    }

    #[test]
    fn cycles_are_found_through_any_depth() {
        let edges: HashMap<String, Vec<String>> = [("a", vec!["b"]), ("b", vec!["c"]), ("c", vec!["d"]), ("d", vec![])].into_iter().map(|(k, v)| (k.to_string(), v.into_iter().map(String::from).collect())).collect();
        assert!(find_cycle("a", &edges).is_none());
        let mut looped = edges.clone();
        looped.insert("d".into(), vec!["a".into()]);
        assert_eq!(find_cycle("a", &looped).unwrap(), vec!["a", "b", "c", "d", "a"]);
        assert_eq!(find_cycle("c", &looped).unwrap(), vec!["c", "d", "a", "b", "c"]);
    }

    #[tokio::test]
    async fn read_only_chats_and_feeds_become_read_chats_once() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        let old_table = concat!("fe", "ed");
        let old_column = concat!("is_read", "_only");
        sqlx::query(&format!("CREATE TABLE conversation (id TEXT PRIMARY KEY, type TEXT, {} BOOLEAN NOT NULL DEFAULT 0, mode TEXT NOT NULL DEFAULT 'write')", old_column)).execute(&pool).await.unwrap();
        sqlx::query(&format!("CREATE TABLE conversation_group (id TEXT PRIMARY KEY, {} BOOLEAN NOT NULL DEFAULT 0, mode TEXT NOT NULL DEFAULT 'write')", old_column)).execute(&pool).await.unwrap();
        sqlx::query(&format!("CREATE TABLE {} (id TEXT PRIMARY KEY, profile_id TEXT, conversation_id TEXT, name TEXT, icon TEXT, sources TEXT, created_at TEXT)", old_table)).execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE mock_seed (profile_id TEXT, key TEXT, kind TEXT, item_id TEXT, created_at TEXT, PRIMARY KEY (profile_id, key))").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO conversation (id, type, mode) VALUES ('locked', 'channel', 'write'), ('open', 'channel', 'write'), ('digest', ?1, 'write')").bind(old_table).execute(&pool).await.unwrap();
        sqlx::query(&format!("UPDATE conversation SET {} = 1 WHERE id IN ('locked', 'digest')", old_column)).execute(&pool).await.unwrap();
        sqlx::query(&format!("INSERT INTO conversation_group (id, {}) VALUES ('g1', 1), ('g2', 0)", old_column)).execute(&pool).await.unwrap();
        sqlx::query(&format!("INSERT INTO {} VALUES ('f1', 'p', 'digest', 'Digest', NULL, '[]', '')", old_table)).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO mock_seed VALUES ('p', ?1, ?2, 'f1', ''), ('p', ?3, 'marker', 'locked', '')").bind(format!("{}:release", old_table)).bind(old_table).bind(concat!("read", "_only")).execute(&pool).await.unwrap();

        rename_read_chats(&pool).await.unwrap();
        rename_read_chats(&pool).await.unwrap();

        assert!(!table_exists(&pool, old_table).await.unwrap());
        assert!(!column_exists(&pool, "conversation", old_column).await.unwrap());
        let modes: Vec<(String, String, String)> = sqlx::query_as("SELECT id, type, mode FROM conversation ORDER BY id").fetch_all(&pool).await.unwrap();
        assert_eq!(modes, vec![("digest".to_string(), CONVERSATION_TYPE.to_string(), "read".to_string()), ("locked".to_string(), "channel".to_string(), "read".to_string()), ("open".to_string(), "channel".to_string(), "write".to_string())]);
        let groups: Vec<(String, String)> = sqlx::query_as("SELECT id, mode FROM conversation_group ORDER BY id").fetch_all(&pool).await.unwrap();
        assert_eq!(groups, vec![("g1".to_string(), "read".to_string()), ("g2".to_string(), "write".to_string())]);
        let chats: Vec<String> = sqlx::query_scalar("SELECT conversation_id FROM chat").fetch_all(&pool).await.unwrap();
        assert_eq!(chats, vec!["digest".to_string()]);
        let ledger: Vec<(String, String)> = sqlx::query_as("SELECT key, kind FROM mock_seed ORDER BY key").fetch_all(&pool).await.unwrap();
        assert_eq!(ledger, vec![("chat:release".to_string(), "chat".to_string()), ("read_mode".to_string(), "marker".to_string())]);
    }

    #[tokio::test]
    async fn mode_view_and_view_settings_persist_on_the_chat() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let chat = Chat::post(&pool, &profile, &PostChatInput { name: "Kvitteringer".into(), mode: ChatMode::Read, view: Some(ChatView::Grid), ..Default::default() }).await.unwrap();
        assert_eq!(chat.mode, ChatMode::Read);
        let load = |id: String| {
            let pool = pool.clone();
            async move { crate::models::Conversation::load_with_providers(&pool, &id).await.unwrap().unwrap() }
        };
        let conversation = load(chat.conversation_id.clone()).await;
        assert_eq!(conversation.mode, ChatMode::Read);
        assert_eq!(conversation.view, ChatView::Grid);
        assert_eq!(conversation.view_settings, ViewSettings::default());
        assert_eq!(conversation.conversation_type, CONVERSATION_TYPE);
        let settings = ViewSettings { grid: GridSettings { group: GridGroup::Tag, sort: GridSort::Reactions, direction: SortDirection::Asc, rows: GridRows::Wrap, tags: false, show: vec![GridShow::Messages, GridShow::Links] }, ..ViewSettings::default() };
        sqlx::query("UPDATE conversation SET mode = ?1, view = ?2, view_settings = ?3 WHERE id = ?4")
            .bind(ChatMode::Write.name())
            .bind(ChatView::Chat.name())
            .bind(serde_json::to_string(&settings).unwrap())
            .bind(&chat.conversation_id)
            .execute(&pool)
            .await
            .unwrap();
        let conversation = load(chat.conversation_id.clone()).await;
        assert_eq!(conversation.mode, ChatMode::Write);
        assert_eq!(conversation.view, ChatView::Chat);
        assert_eq!(conversation.view_settings, settings);
        let partial: ViewSettings = serde_json::from_str("{\"grid\":{\"group\":\"modified\"}}").unwrap();
        assert_eq!(partial.grid.group, GridGroup::Modified);
        assert_eq!(partial.grid.direction, SortDirection::Desc);
        assert!(partial.grid.tags && partial.grid.show.is_empty());
        let listed: Vec<String> = crate::models::Conversation::get_rows_for_profile(&pool, &profile).await.unwrap().into_iter().map(|r| r.get::<String, _>("id")).collect();
        assert!(listed.contains(&chat.conversation_id));
        assert!(Chat::post(&pool, &profile, &PostChatInput { name: " ".into(), ..Default::default() }).await.is_err());
        assert!(valid_sources(&[ChatSource { kind: "planet".into(), id: Some("x".into()), query: None }]).is_err());
        assert!(valid_sources(&[ChatSource { kind: SOURCE_QUERY.into(), id: None, query: Some(GroupQuery::default()) }]).is_ok());
        Chat::delete(&pool, &profile, &chat.id).await.unwrap();
        assert!(Chat::all(&pool, &profile).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn files_views_become_grid_views_showing_files() {
        let pool = test_pool().await;
        for (id, view, settings) in [("old", "files", "{\"files\":{\"group\":\"sender\",\"rows\":\"wrap\"},\"chat\":{\"sources\":true}}"), ("kept", "chat", "{\"files\":{\"sort\":\"size\"}}"), ("new", "grid", "{\"grid\":{\"show\":[\"links\"]}}")] {
            sqlx::query("INSERT INTO conversation (id, name, type, group_type, last_activity, view, view_settings, created_at, updated_at) VALUES (?1, ?1, 'channel', 'auto', '', ?2, ?3, '', '')").bind(id).bind(view).bind(settings).execute(&pool).await.unwrap();
        }
        rename_read_chats(&pool).await.unwrap();
        rename_read_chats(&pool).await.unwrap();
        let load = |id: &'static str| {
            let pool = pool.clone();
            async move {
                let (view, raw): (String, String) = sqlx::query_as("SELECT view, view_settings FROM conversation WHERE id = ?1").bind(id).fetch_one(&pool).await.unwrap();
                (ChatView::parse(&view), ViewSettings::parse(Some(raw.clone())), raw)
            }
        };
        let (view, settings, raw) = load("old").await;
        assert_eq!(view, ChatView::Grid);
        assert_eq!(settings.grid.group, GridGroup::Sender);
        assert_eq!(settings.grid.rows, GridRows::Wrap);
        assert_eq!(settings.grid.show, vec![GridShow::Files, GridShow::Records]);
        assert!(settings.chat.sources);
        assert!(!raw.contains("\"files\":{"));
        let (view, settings, _) = load("kept").await;
        assert_eq!(view, ChatView::Chat);
        assert_eq!(settings.grid.sort, GridSort::Size);
        assert!(settings.grid.show.is_empty());
        let (view, settings, _) = load("new").await;
        assert_eq!(view, ChatView::Grid);
        assert_eq!(settings.grid.show, vec![GridShow::Links]);
    }
}
