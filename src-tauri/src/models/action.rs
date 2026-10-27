use crate::models::group::GroupQuery;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

pub const STATUS_SENT: &str = "sent";
pub const STATUS_FAILED: &str = "failed";
pub const STATUS_CANCELLED: &str = "cancelled";
pub const SENT_PREFIX: &str = "Sent in ";

pub const NODE_TRIGGER: &str = "trigger";
pub const NODE_SCRIPT: &str = "script";
pub const NODE_CHAT: &str = "chat";
pub const BUILTIN_PREFIX: &str = "builtin:";
pub const ACTION_PREFIX: &str = "action:";
pub const BUILTIN_KEY_PREFIX: &str = "trigger:";
pub const BUILTIN_GROUP: &str = "builtin";
pub const BUILTIN_ACTION_KEYS: &[&str] = &["builtin:receipts", "classify_senders", "detect_newsletters", "recognise_invites", "sort_splits", "summarise_mail", "draft_reply"];
pub const STARTER_GROUP: &str = "starter";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PortType {
    Message,
    Conversation,
    Person,
    Text,
    Number,
    Bool,
    List,
    Json,
    Tag,
    Any,
}

impl PortType {
    pub const ALL: [PortType; 10] = [
        PortType::Message,
        PortType::Conversation,
        PortType::Person,
        PortType::Text,
        PortType::Number,
        PortType::Bool,
        PortType::List,
        PortType::Json,
        PortType::Tag,
        PortType::Any,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PortType::Message => "message",
            PortType::Conversation => "conversation",
            PortType::Person => "person",
            PortType::Text => "text",
            PortType::Number => "number",
            PortType::Bool => "bool",
            PortType::List => "list",
            PortType::Json => "json",
            PortType::Tag => "tag",
            PortType::Any => "any",
        }
    }

    pub fn accepts(self, from: PortType) -> bool {
        self == from || self == PortType::Any || from == PortType::Any || self == PortType::Json || (self == PortType::Text && matches!(from, PortType::Tag | PortType::Number | PortType::Bool))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub standalone: bool,
    pub chat: bool,
    pub step: bool,
    pub landing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Port {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: PortType,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_port: Option<String>,
}

impl Port {
    pub fn new(name: &str, kind: PortType) -> Port {
        Port { name: name.to_string(), kind, description: String::new(), node: None, source_port: None }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TriggerOn {
    #[default]
    Arrival,
    Sent,
    Edited,
    Deleted,
    Reaction,
    Mention,
    Filter,
    Schedule,
    EventSoon,
    Manual,
    Called,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TriggerSchedule {
    #[serde(default)]
    pub every_minutes: Option<u32>,
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub days: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TriggerConfig {
    #[serde(default)]
    pub on: TriggerOn,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub scope: GroupQuery,
    #[serde(default)]
    pub group_ids: Vec<String>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub message_kinds: Vec<String>,
    #[serde(default)]
    pub schedule: Option<TriggerSchedule>,
    #[serde(default)]
    pub minutes_before: Option<u32>,
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub watch: Option<TriggerWatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TriggerWatch {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub every_minutes: Option<u32>,
    #[serde(default)]
    pub pick: Option<String>,
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub token: Option<String>,
}

pub const WATCH_WEBHOOK: &str = "webhook";
pub const WATCH_POLL: &str = "poll";
pub const DEFAULT_WATCH_MINUTES: u32 = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct ActionWatch {
    pub digest: String,
    pub value: Value,
    pub checked_at: String,
}

pub const DIRECTION_INCOMING: &str = "incoming";
pub const DIRECTION_OUTGOING: &str = "outgoing";
pub const DIRECTION_ANY: &str = "any";
pub const DEFAULT_MINUTES_BEFORE: u32 = 15;

impl TriggerOn {
    pub fn is_invocable(self) -> bool {
        matches!(self, TriggerOn::Manual | TriggerOn::Called)
    }
}

impl TriggerConfig {
    pub fn problem(&self) -> Option<&'static str> {
        match self.on {
            TriggerOn::Filter if self.group_ids.is_empty() => Some("Choose the filter group this trigger listens to"),
            TriggerOn::Schedule if self.schedule.as_ref().is_none_or(|s| s.every_minutes.is_none() && s.at.is_none()) => Some("Set how often, or at what time, the schedule runs"),
            TriggerOn::Custom => match self.watch.as_ref() {
                None => Some("Choose whether this trigger receives a webhook or polls a URL"),
                Some(w) if w.mode == WATCH_POLL && w.url.as_deref().is_none_or(|u| !u.starts_with("http")) => Some("Set the http or https URL this trigger polls"),
                Some(w) if w.mode != WATCH_POLL && w.mode != WATCH_WEBHOOK => Some("A custom trigger either receives a webhook or polls a URL"),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn direction(&self) -> &str {
        match self.direction.as_deref() {
            Some(DIRECTION_OUTGOING) => DIRECTION_OUTGOING,
            Some(DIRECTION_ANY) => DIRECTION_ANY,
            _ if matches!(self.on, TriggerOn::Sent | TriggerOn::Edited | TriggerOn::Deleted | TriggerOn::Reaction) => DIRECTION_ANY,
            _ => DIRECTION_INCOMING,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub config: Value,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
}

impl Node {
    pub fn trigger(&self) -> Option<TriggerConfig> {
        (self.kind == NODE_TRIGGER).then(|| serde_json::from_value(self.config.clone()).unwrap_or_default())
    }

    pub fn declared(&self, key: &str) -> Vec<Port> {
        self.config.get(key).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
    }

    pub fn constant(&self, name: &str) -> Option<Value> {
        self.config.get("values").and_then(|v| v.get(name)).filter(|v| !v.is_null()).cloned()
    }

    pub fn chat(&self) -> Option<ChatNodeConfig> {
        (self.kind == NODE_CHAT).then(|| serde_json::from_value(self.config.clone()).unwrap_or_default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ChatNodeConfig {
    #[serde(default)]
    pub mode: crate::models::chat::ChatMode,
    #[serde(default)]
    pub conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub source_port: String,
    pub target: String,
    pub target_port: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Graph {
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AiChoice {
    #[serde(default)]
    pub connection_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

impl AiChoice {
    pub fn is_empty(&self) -> bool {
        [&self.connection_id, &self.model].iter().all(|v| v.as_deref().is_none_or(|v| v.trim().is_empty()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub profile_id: String,
    pub key: Option<String>,
    #[serde(default)]
    pub builtin: bool,
    pub group_id: Option<String>,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub enabled: bool,
    pub position: i64,
    pub inputs: Vec<Port>,
    #[serde(default)]
    pub outputs: Vec<Port>,
    pub graph: Graph,
    pub max_sends_per_hour: i64,
    pub run_count: i64,
    pub last_run_at: Option<String>,
    pub last_status: Option<String>,
    #[serde(default)]
    pub usage: Usage,
    #[serde(default)]
    pub builtin_kind: Option<String>,
    #[serde(default)]
    pub ai: AiChoice,
    #[serde(default)]
    pub chats: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Action {
    pub fn triggers(&self) -> Vec<(&Node, TriggerConfig)> {
        self.graph.nodes.iter().filter_map(|n| n.trigger().filter(|t| !t.disabled && t.problem().is_none()).map(|t| (n, t))).collect()
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PostActionInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub inputs: Option<Vec<Port>>,
    #[serde(default)]
    pub graph: Option<Graph>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub max_sends_per_hour: Option<i64>,
    #[serde(default)]
    pub ai: Option<AiChoice>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PutActionInput {
    pub action_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<Option<String>>,
    #[serde(default)]
    pub group_id: Option<Option<String>>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub position: Option<i64>,
    #[serde(default)]
    pub inputs: Option<Vec<Port>>,
    #[serde(default)]
    pub graph: Option<Graph>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub max_sends_per_hour: Option<i64>,
    #[serde(default)]
    pub ai: Option<AiChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionGroup {
    pub id: String,
    pub profile_id: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub builtin: bool,
    pub parent_id: Option<String>,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub position: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostActionGroupInput {
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PutActionGroupInput {
    pub group_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::models::group::deserialize_nullable")]
    pub parent_id: Option<Option<String>>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "crate::models::group::deserialize_nullable")]
    pub icon: Option<Option<String>>,
    #[serde(default)]
    pub position: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogEntry {
    pub node: String,
    pub kind: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRun {
    pub id: String,
    pub action_id: String,
    pub action_name: String,
    pub status: String,
    pub dry_run: bool,
    pub trigger: Value,
    pub log: Vec<LogEntry>,
    pub outputs: Value,
    pub state: Value,
    pub error: Option<String>,
    pub resume_at: Option<String>,
    pub depth: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    #[serde(default)]
    pub result: Value,
    #[serde(skip_deserializing)]
    pub requirement: Option<crate::services::action_script::Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSchedule {
    pub id: String,
    pub conversation_id: String,
    pub conversation_name: Option<String>,
    pub provider_id: Option<String>,
    pub content: String,
    pub content_type: String,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub attachments: Option<Vec<crate::models::OutgoingAttachmentInput>>,
    pub at: Option<String>,
    pub action_id: Option<String>,
    pub action_name: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostMessageScheduleInput {
    pub conversation_id: String,
    #[serde(default)]
    pub provider_id: Option<String>,
    pub content: String,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub action_id: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub attachments: Option<Vec<crate::models::OutgoingAttachmentInput>>,
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    for statement in [
        r#"CREATE TABLE IF NOT EXISTS action_group (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            parent_id TEXT,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            icon TEXT,
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profile (id) ON DELETE CASCADE
        )"#,
        r#"CREATE TABLE IF NOT EXISTS action (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            group_id TEXT,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            icon TEXT,
            enabled BOOLEAN NOT NULL DEFAULT 1,
            position INTEGER NOT NULL DEFAULT 0,
            inputs TEXT NOT NULL DEFAULT '[]',
            graph TEXT NOT NULL DEFAULT '{}',
            max_sends_per_hour INTEGER NOT NULL DEFAULT 10,
            last_scheduled_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profile (id) ON DELETE CASCADE
        )"#,
        r#"CREATE TABLE IF NOT EXISTS action_watch (
            action_id TEXT NOT NULL,
            node_id TEXT NOT NULL,
            digest TEXT NOT NULL,
            value TEXT NOT NULL DEFAULT 'null',
            checked_at TEXT NOT NULL,
            PRIMARY KEY (action_id, node_id),
            FOREIGN KEY (action_id) REFERENCES action (id) ON DELETE CASCADE
        )"#,
        r#"CREATE TABLE IF NOT EXISTS action_run (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            action_id TEXT NOT NULL,
            status TEXT NOT NULL,
            dry_run BOOLEAN NOT NULL DEFAULT 0,
            trigger TEXT NOT NULL DEFAULT 'null',
            log TEXT NOT NULL DEFAULT '[]',
            outputs TEXT NOT NULL DEFAULT '{}',
            state TEXT NOT NULL DEFAULT '{}',
            error TEXT,
            resume_at TEXT,
            depth INTEGER NOT NULL DEFAULT 0,
            started_at TEXT NOT NULL,
            finished_at TEXT,
            FOREIGN KEY (action_id) REFERENCES action (id) ON DELETE CASCADE
        )"#,
        "CREATE INDEX IF NOT EXISTS idx_action_run_action ON action_run (action_id, started_at)",
        r#"CREATE TABLE IF NOT EXISTS message_schedule (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            provider_id TEXT,
            content TEXT NOT NULL,
            content_type TEXT NOT NULL DEFAULT 'text',
            at TEXT,
            action_id TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            error TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"#,
    ] {
        sqlx::query(statement).execute(pool).await?;
    }
    crate::database::ensure_columns(pool, "action", &[("key", "TEXT"), ("ai_connection_id", "TEXT"), ("ai_model", "TEXT")]).await;
    crate::database::ensure_columns(pool, "action_group", &[("key", "TEXT")]).await;
    crate::database::ensure_columns(pool, "action_run", &[("result", "TEXT NOT NULL DEFAULT '{}'")]).await;
    crate::database::ensure_columns(pool, "message_schedule", &[("metadata", "TEXT"), ("attachments", "TEXT")]).await;
    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_action_key ON action (profile_id, key) WHERE key IS NOT NULL").execute(pool).await?;
    crate::services::action_service::retire_outputs(pool).await?;
    crate::services::action_service::retire_outbox(pool).await?;
    Ok(())
}

fn filled(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

fn json_or<T: serde::de::DeserializeOwned + Default>(raw: Option<String>) -> T {
    raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn value_or_null(raw: Option<String>) -> Value {
    raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null)
}

impl ActionGroup {
    pub fn is_builtin_key(key: Option<&str>) -> bool {
        key.is_some_and(|k| k == BUILTIN_GROUP || k.starts_with(&format!("{}.", BUILTIN_GROUP)))
    }

    fn from_row(row: sqlx::sqlite::SqliteRow) -> ActionGroup {
        let key: Option<String> = row.try_get("key").ok().flatten();
        ActionGroup {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            builtin: ActionGroup::is_builtin_key(key.as_deref()),
            key,
            parent_id: row.try_get("parent_id").ok().flatten(),
            name: row.get("name"),
            description: row.try_get("description").unwrap_or_default(),
            icon: row.try_get("icon").ok().flatten(),
            position: row.try_get("position").unwrap_or(0),
        }
    }

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<ActionGroup>, String> {
        let rows = sqlx::query("SELECT * FROM action_group WHERE profile_id = ?1 ORDER BY position, name COLLATE NOCASE")
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list action groups: {}", e))?;
        Ok(rows.into_iter().map(ActionGroup::from_row).collect())
    }

    pub async fn get(pool: &Pool<Sqlite>, id: &str) -> Result<ActionGroup, String> {
        sqlx::query("SELECT * FROM action_group WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load the action group: {}", e))?
            .map(ActionGroup::from_row)
            .ok_or_else(|| "Action group not found".to_string())
    }

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, input: &PostActionGroupInput) -> Result<ActionGroup, String> {
        if input.name.trim().is_empty() {
            return Err("A group needs a name".to_string());
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let position: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), -1) + 1 FROM action_group WHERE profile_id = ?1 AND parent_id IS ?2")
            .bind(profile_id)
            .bind(&input.parent_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to place the group: {}", e))?;
        sqlx::query("INSERT INTO action_group (id, profile_id, parent_id, name, description, icon, position, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)")
            .bind(&id)
            .bind(profile_id)
            .bind(&input.parent_id)
            .bind(input.name.trim())
            .bind(input.description.clone().unwrap_or_default())
            .bind(&input.icon)
            .bind(position)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to create the group: {}", e))?;
        ActionGroup::get(pool, &id).await
    }

    pub async fn put(pool: &Pool<Sqlite>, input: &PutActionGroupInput) -> Result<ActionGroup, String> {
        let mut group = ActionGroup::get(pool, &input.group_id).await?;
        if group.builtin {
            return Err("Built in groups are read only".to_string());
        }
        if let Some(name) = &input.name {
            group.name = name.trim().to_string();
        }
        if let Some(parent) = &input.parent_id {
            if parent.as_deref() == Some(group.id.as_str()) || ActionGroup::descends_from(pool, parent.as_deref(), &group.id).await? {
                return Err("A group cannot move inside itself".to_string());
            }
            group.parent_id = parent.clone();
        }
        if let Some(description) = &input.description {
            group.description = description.clone();
        }
        if let Some(icon) = &input.icon {
            group.icon = icon.clone();
        }
        if let Some(position) = input.position {
            group.position = position;
        }
        sqlx::query("UPDATE action_group SET name = ?1, parent_id = ?2, description = ?3, icon = ?4, position = ?5, updated_at = ?6 WHERE id = ?7")
            .bind(&group.name)
            .bind(&group.parent_id)
            .bind(&group.description)
            .bind(&group.icon)
            .bind(group.position)
            .bind(Utc::now().to_rfc3339())
            .bind(&group.id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to save the group: {}", e))?;
        Ok(group)
    }

    async fn descends_from(pool: &Pool<Sqlite>, start: Option<&str>, ancestor: &str) -> Result<bool, String> {
        let mut cursor = start.map(String::from);
        let mut hops = 0;
        while let Some(id) = cursor {
            if id == ancestor {
                return Ok(true);
            }
            hops += 1;
            if hops > 64 {
                return Ok(true);
            }
            cursor = sqlx::query_scalar::<_, Option<String>>("SELECT parent_id FROM action_group WHERE id = ?1")
                .bind(&id)
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("Failed to walk the groups: {}", e))?
                .flatten();
        }
        Ok(false)
    }

    pub async fn delete(pool: &Pool<Sqlite>, id: &str) -> Result<(), String> {
        let group = ActionGroup::get(pool, id).await?;
        if group.builtin {
            return Err("The built in group cannot be removed".to_string());
        }
        for statement in [
            "UPDATE action SET group_id = ?2 WHERE group_id = ?1",
            "UPDATE action_group SET parent_id = ?2 WHERE parent_id = ?1",
        ] {
            sqlx::query(statement).bind(id).bind(&group.parent_id).execute(pool).await.map_err(|e| format!("Failed to move the group's items: {}", e))?;
        }
        sqlx::query("DELETE FROM action_group WHERE id = ?1").bind(id).execute(pool).await.map_err(|e| format!("Failed to delete the group: {}", e))?;
        Ok(())
    }
}

impl Action {
    pub fn is_builtin_key(key: Option<&str>) -> bool {
        key.is_some_and(|k| k.starts_with(BUILTIN_KEY_PREFIX) || k.starts_with(BUILTIN_PREFIX) || BUILTIN_ACTION_KEYS.contains(&k))
    }

    fn from_row(row: sqlx::sqlite::SqliteRow) -> Action {
        let key: Option<String> = row.try_get("key").ok().flatten();
        let graph: Graph = json_or(row.try_get("graph").ok());
        let builtin = Action::is_builtin_key(key.as_deref());
        let chats: String = row.try_get::<Option<String>, _>("chats").ok().flatten().unwrap_or_else(|| "[]".to_string());
        Action {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            builtin,
            usage: Usage::default(),
            builtin_kind: None,
            ai: AiChoice { connection_id: row.try_get("ai_connection_id").ok().flatten(), model: row.try_get("ai_model").ok().flatten() },
            chats: serde_json::from_str(&chats).unwrap_or_default(),
            key,
            group_id: row.try_get("group_id").ok().flatten(),
            name: row.get("name"),
            description: row.try_get("description").unwrap_or_default(),
            icon: row.try_get("icon").ok().flatten(),
            enabled: row.get("enabled"),
            position: row.try_get("position").unwrap_or(0),
            inputs: json_or(row.try_get("inputs").ok()),
            outputs: Vec::new(),
            graph,
            max_sends_per_hour: row.try_get("max_sends_per_hour").unwrap_or_else(|_| crate::models::app_setting::get_default_u64(crate::models::app_setting::ACTIONS_MAX_SENDS_PER_HOUR) as i64),
            run_count: row.try_get("run_count").unwrap_or(0),
            last_run_at: row.try_get("last_run_at").ok().flatten(),
            last_status: row.try_get("last_status").ok().flatten(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    const SELECT: &'static str = "SELECT a.*,
            (SELECT COUNT(*) FROM action_run r WHERE r.action_id = a.id AND r.dry_run = 0) AS run_count,
            (SELECT MAX(started_at) FROM action_run r WHERE r.action_id = a.id AND r.dry_run = 0) AS last_run_at,
            (SELECT status FROM action_run r WHERE r.action_id = a.id AND r.dry_run = 0 ORDER BY started_at DESC LIMIT 1) AS last_status,
            (SELECT json_group_array(ca.conversation_id) FROM chat_action ca WHERE ca.action_id = a.id) AS chats
         FROM action a";

    pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Action>, String> {
        let rows = sqlx::query(&format!("{} WHERE a.profile_id = ?1 ORDER BY a.position, a.name COLLATE NOCASE", Self::SELECT))
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list actions: {}", e))?;
        let mut actions: Vec<Action> = rows.into_iter().map(Action::from_row).collect();
        crate::services::action_service::sign(&mut actions);
        Ok(actions)
    }

    async fn signed(pool: &Pool<Sqlite>, action: Action) -> Result<Action, String> {
        if action.graph.nodes.iter().any(|n| n.kind.starts_with(ACTION_PREFIX)) {
            return Action::all(pool, &action.profile_id).await?.into_iter().find(|a| a.id == action.id).ok_or_else(|| format!("Action {} not found", action.id));
        }
        let mut one = vec![action];
        crate::services::action_service::sign(&mut one);
        Ok(one.remove(0))
    }

    pub async fn find(pool: &Pool<Sqlite>, profile_id: &str, id_or_key: &str) -> Result<Action, String> {
        let action = sqlx::query(&format!("{} WHERE a.id = ?1 OR (a.profile_id = ?2 AND a.key = ?1)", Self::SELECT))
            .bind(id_or_key)
            .bind(profile_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load the action: {}", e))?
            .map(Action::from_row)
            .ok_or_else(|| format!("Action {} not found", id_or_key))?;
        Action::signed(pool, action).await
    }

    pub async fn get(pool: &Pool<Sqlite>, id: &str) -> Result<Action, String> {
        let action = sqlx::query(&format!("{} WHERE a.id = ?1", Self::SELECT))
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load the action: {}", e))?
            .map(Action::from_row)
            .ok_or_else(|| format!("Action {} not found", id))?;
        Action::signed(pool, action).await
    }

    pub async fn insert(pool: &Pool<Sqlite>, action: &Action) -> Result<Action, String> {
        let position: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), -1) + 1 FROM action WHERE profile_id = ?1 AND group_id IS ?2")
            .bind(&action.profile_id)
            .bind(&action.group_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to place the action: {}", e))?;
        sqlx::query(
            "INSERT INTO action (id, profile_id, group_id, name, description, icon, enabled, position, inputs, graph, max_sends_per_hour, created_at, updated_at, key, ai_connection_id, ai_model)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, ?13, ?14, ?15)",
        )
        .bind(&action.id)
        .bind(&action.profile_id)
        .bind(&action.group_id)
        .bind(action.name.trim())
        .bind(&action.description)
        .bind(&action.icon)
        .bind(action.enabled)
        .bind(position)
        .bind(serde_json::to_string(&action.inputs).map_err(|e| e.to_string())?)
        .bind(serde_json::to_string(&action.graph).map_err(|e| e.to_string())?)
        .bind(action.max_sends_per_hour.max(0))
        .bind(Utc::now().to_rfc3339())
        .bind(&action.key)
        .bind(filled(&action.ai.connection_id))
        .bind(filled(&action.ai.model))
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create the action: {}", e))?;
        Action::get(pool, &action.id).await
    }

    pub async fn save(pool: &Pool<Sqlite>, action: &Action) -> Result<Action, String> {
        sqlx::query(
            "UPDATE action SET group_id = ?1, name = ?2, description = ?3, icon = ?4, enabled = ?5, position = ?6, inputs = ?7, graph = ?8,
                max_sends_per_hour = ?9, ai_connection_id = ?10, ai_model = ?11, updated_at = ?12 WHERE id = ?13",
        )
        .bind(&action.group_id)
        .bind(action.name.trim())
        .bind(&action.description)
        .bind(&action.icon)
        .bind(action.enabled)
        .bind(action.position)
        .bind(serde_json::to_string(&action.inputs).map_err(|e| e.to_string())?)
        .bind(serde_json::to_string(&action.graph).map_err(|e| e.to_string())?)
        .bind(action.max_sends_per_hour.max(0))
        .bind(filled(&action.ai.connection_id))
        .bind(filled(&action.ai.model))
        .bind(Utc::now().to_rfc3339())
        .bind(&action.id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to save the action: {}", e))?;
        Action::get(pool, &action.id).await
    }

    pub fn blank(profile_id: &str, name: &str) -> Action {
        let now = Utc::now().to_rfc3339();
        Action {
            id: Uuid::new_v4().to_string(),
            profile_id: profile_id.to_string(),
            key: None,
            builtin: false,
            group_id: None,
            name: name.to_string(),
            description: String::new(),
            icon: None,
            enabled: true,
            position: 0,
            inputs: Vec::new(),
            outputs: Vec::new(),
            graph: Graph::default(),
            max_sends_per_hour: crate::models::app_setting::get_default_u64(crate::models::app_setting::ACTIONS_MAX_SENDS_PER_HOUR) as i64,
            run_count: 0,
            last_run_at: None,
            last_status: None,
            usage: Usage::default(),
            builtin_kind: None,
            ai: AiChoice::default(),
            chats: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub async fn delete(pool: &Pool<Sqlite>, id: &str) -> Result<(), String> {
        if Action::get(pool, id).await?.builtin {
            return Err("Built in actions cannot be deleted".to_string());
        }
        Action::purge(pool, id).await
    }

    pub async fn purge(pool: &Pool<Sqlite>, id: &str) -> Result<(), String> {
        for statement in [
            "DELETE FROM chat_action WHERE action_id = ?1",
            "DELETE FROM action_run WHERE action_id = ?1",
            "UPDATE message_schedule SET status = 'cancelled' WHERE action_id = ?1 AND status = 'pending'",
            "DELETE FROM action WHERE id = ?1",
        ] {
            sqlx::query(statement).bind(id).execute(pool).await.map_err(|e| format!("Failed to delete the action: {}", e))?;
        }
        Ok(())
    }

    pub async fn mark_scheduled(pool: &Pool<Sqlite>, id: &str, at: &str) -> Result<(), String> {
        sqlx::query("UPDATE action SET last_scheduled_at = ?1 WHERE id = ?2")
            .bind(at)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to mark the schedule: {}", e))?;
        Ok(())
    }

    pub async fn last_scheduled(pool: &Pool<Sqlite>, id: &str) -> Result<Option<String>, String> {
        sqlx::query_scalar("SELECT last_scheduled_at FROM action WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map(|v: Option<Option<String>>| v.flatten())
            .map_err(|e| format!("Failed to read the schedule: {}", e))
    }

    pub async fn get_watch(pool: &Pool<Sqlite>, action_id: &str, node_id: &str) -> Result<Option<ActionWatch>, String> {
        let row = sqlx::query("SELECT digest, value, checked_at FROM action_watch WHERE action_id = ?1 AND node_id = ?2")
            .bind(action_id)
            .bind(node_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to read what the trigger last saw: {}", e))?;
        Ok(row.map(|row| ActionWatch { digest: row.get("digest"), value: serde_json::from_str(&row.get::<String, _>("value")).unwrap_or(Value::Null), checked_at: row.get("checked_at") }))
    }

    pub async fn put_watch(pool: &Pool<Sqlite>, action_id: &str, node_id: &str, digest: &str, value: &Value, at: &str) -> Result<(), String> {
        sqlx::query("INSERT INTO action_watch (action_id, node_id, digest, value, checked_at) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (action_id, node_id) DO UPDATE SET digest = excluded.digest, value = excluded.value, checked_at = excluded.checked_at")
            .bind(action_id)
            .bind(node_id)
            .bind(digest)
            .bind(value.to_string())
            .bind(at)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to keep what the trigger saw: {}", e))?;
        Ok(())
    }
}

impl ActionRun {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> ActionRun {
        ActionRun {
            id: row.get("id"),
            action_id: row.get("action_id"),
            action_name: row.try_get::<Option<String>, _>("action_name").ok().flatten().unwrap_or_default(),
            status: row.get("status"),
            dry_run: row.get("dry_run"),
            trigger: value_or_null(row.try_get("trigger").ok()),
            log: json_or(row.try_get("log").ok()),
            outputs: value_or_null(row.try_get("outputs").ok()),
            state: value_or_null(row.try_get("state").ok()),
            error: row.try_get("error").ok().flatten(),
            resume_at: row.try_get("resume_at").ok().flatten(),
            depth: row.try_get("depth").unwrap_or(0),
            started_at: row.get("started_at"),
            finished_at: row.try_get("finished_at").ok().flatten(),
            result: value_or_null(row.try_get("result").ok()),
            requirement: row.try_get::<Option<String>, _>("error").ok().flatten().as_deref().and_then(crate::services::action_script::unmet_of),
        }
    }

    const SELECT: &'static str = "SELECT r.*, a.name AS action_name FROM action_run r LEFT JOIN action a ON a.id = r.action_id";

    pub async fn save(pool: &Pool<Sqlite>, profile_id: &str, run: &ActionRun) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO action_run (id, profile_id, action_id, status, dry_run, trigger, log, outputs, state, error, resume_at, depth, started_at, finished_at, result)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(id) DO UPDATE SET status = excluded.status, log = excluded.log, outputs = excluded.outputs, state = excluded.state,
                error = excluded.error, resume_at = excluded.resume_at, finished_at = excluded.finished_at, result = excluded.result",
        )
        .bind(&run.id)
        .bind(profile_id)
        .bind(&run.action_id)
        .bind(&run.status)
        .bind(run.dry_run)
        .bind(run.trigger.to_string())
        .bind(serde_json::to_string(&run.log).map_err(|e| e.to_string())?)
        .bind(run.outputs.to_string())
        .bind(run.state.to_string())
        .bind(&run.error)
        .bind(&run.resume_at)
        .bind(run.depth)
        .bind(&run.started_at)
        .bind(&run.finished_at)
        .bind(run.result.to_string())
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to record the run: {}", e))?;
        Ok(())
    }

    pub async fn list(pool: &Pool<Sqlite>, profile_id: &str, action_id: Option<&str>, limit: i64) -> Result<Vec<ActionRun>, String> {
        let rows = sqlx::query(&format!("{} WHERE r.profile_id = ?1 AND (?2 IS NULL OR r.action_id = ?2) ORDER BY r.started_at DESC LIMIT ?3", Self::SELECT))
            .bind(profile_id)
            .bind(action_id)
            .bind(limit.clamp(1, 500))
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list runs: {}", e))?;
        Ok(rows.into_iter().map(ActionRun::from_row).collect())
    }

    pub async fn waiting_due(pool: &Pool<Sqlite>, profile_id: &str, now: &str) -> Result<Vec<ActionRun>, String> {
        let rows = sqlx::query(&format!("{} WHERE r.profile_id = ?1 AND r.status = 'waiting' AND r.resume_at <= ?2 ORDER BY r.resume_at", Self::SELECT))
            .bind(profile_id)
            .bind(now)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list waiting runs: {}", e))?;
        Ok(rows.into_iter().map(ActionRun::from_row).collect())
    }

    pub async fn sent_since(pool: &Pool<Sqlite>, action_id: &str, since: &str) -> Result<i64, String> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM action_run r, json_each(r.log) e WHERE r.action_id = ?1 AND r.dry_run = 0 AND r.started_at >= ?2
             AND json_extract(e.value, '$.kind') = 'effect' AND json_extract(e.value, '$.status') = 'done' AND substr(json_extract(e.value, '$.detail'), 1, length(?3)) = ?3",
        )
        .bind(action_id)
        .bind(since)
        .bind(SENT_PREFIX)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to count sends: {}", e))
    }
}

impl MessageSchedule {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> MessageSchedule {
        MessageSchedule {
            id: row.get("id"),
            conversation_id: row.get("conversation_id"),
            conversation_name: row.try_get("conversation_name").ok().flatten(),
            provider_id: row.try_get("provider_id").ok().flatten(),
            content: row.get("content"),
            content_type: row.get("content_type"),
            metadata: row.try_get::<Option<String>, _>("metadata").ok().flatten().and_then(|m| serde_json::from_str(&m).ok()),
            attachments: row.try_get::<Option<String>, _>("attachments").ok().flatten().and_then(|a| serde_json::from_str(&a).ok()),
            at: row.try_get("at").ok().flatten(),
            action_id: row.try_get("action_id").ok().flatten(),
            action_name: row.try_get("action_name").ok().flatten(),
            status: row.get("status"),
            error: row.try_get("error").ok().flatten(),
            created_at: row.get("created_at"),
        }
    }

    const SELECT: &'static str = "SELECT s.*, c.name AS conversation_name, a.name AS action_name FROM message_schedule s
         LEFT JOIN conversation c ON c.id = s.conversation_id LEFT JOIN action a ON a.id = s.action_id";

    pub async fn post(pool: &Pool<Sqlite>, profile_id: &str, input: &PostMessageScheduleInput) -> Result<MessageSchedule, String> {
        if input.content.trim().is_empty() {
            return Err("A scheduled message needs content".to_string());
        }
        if input.at.is_none() && input.action_id.is_none() {
            return Err("Choose a time or an action trigger for the scheduled message".to_string());
        }
        if let Some(at) = &input.at {
            chrono::DateTime::parse_from_rfc3339(at).map_err(|_| format!("{} is not an RFC 3339 time", at))?;
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO message_schedule (id, profile_id, conversation_id, provider_id, content, content_type, at, action_id, metadata, attachments, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11, ?11)",
        )
        .bind(&id)
        .bind(profile_id)
        .bind(&input.conversation_id)
        .bind(&input.provider_id)
        .bind(&input.content)
        .bind(input.content_type.clone().unwrap_or_else(|| "text".to_string()))
        .bind(input.at.as_ref().map(|at| chrono::DateTime::parse_from_rfc3339(at).map(|d| d.with_timezone(&Utc).to_rfc3339()).unwrap_or_else(|_| at.clone())))
        .bind(&input.action_id)
        .bind(input.metadata.as_ref().map(|m| m.to_string()))
        .bind(input.attachments.as_ref().map(|a| serde_json::to_string(a).unwrap_or_default()))
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to schedule the message: {}", e))?;
        MessageSchedule::get(pool, &id).await
    }

    pub async fn get(pool: &Pool<Sqlite>, id: &str) -> Result<MessageSchedule, String> {
        sqlx::query(&format!("{} WHERE s.id = ?1", Self::SELECT))
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load the scheduled message: {}", e))?
            .map(MessageSchedule::from_row)
            .ok_or_else(|| "Scheduled message not found".to_string())
    }

    pub async fn list(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: Option<&str>) -> Result<Vec<MessageSchedule>, String> {
        let rows = sqlx::query(&format!("{} WHERE s.profile_id = ?1 AND (?2 IS NULL OR s.conversation_id = ?2) ORDER BY COALESCE(s.at, s.created_at) DESC LIMIT 200", Self::SELECT))
            .bind(profile_id)
            .bind(conversation_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list scheduled messages: {}", e))?;
        Ok(rows.into_iter().map(MessageSchedule::from_row).collect())
    }

    pub async fn due(pool: &Pool<Sqlite>, profile_id: &str, now: &str) -> Result<Vec<MessageSchedule>, String> {
        let rows = sqlx::query(&format!("{} WHERE s.profile_id = ?1 AND s.status = 'pending' AND s.at IS NOT NULL AND s.at <= ?2 ORDER BY s.at", Self::SELECT))
            .bind(profile_id)
            .bind(now)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list due messages: {}", e))?;
        Ok(rows.into_iter().map(MessageSchedule::from_row).collect())
    }

    pub async fn awaiting(pool: &Pool<Sqlite>, action_id: &str, conversation_id: &str) -> Result<Vec<MessageSchedule>, String> {
        let rows = sqlx::query(&format!("{} WHERE s.action_id = ?1 AND s.conversation_id = ?2 AND s.status = 'pending' AND s.at IS NULL ORDER BY s.created_at", Self::SELECT))
            .bind(action_id)
            .bind(conversation_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list waiting messages: {}", e))?;
        Ok(rows.into_iter().map(MessageSchedule::from_row).collect())
    }

    pub async fn finish(pool: &Pool<Sqlite>, id: &str, status: &str, error: Option<&str>) -> Result<(), String> {
        sqlx::query("UPDATE message_schedule SET status = ?1, error = ?2, updated_at = ?3 WHERE id = ?4")
            .bind(status)
            .bind(error)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update the scheduled message: {}", e))?;
        Ok(())
    }
}
