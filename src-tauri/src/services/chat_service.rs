use crate::models::action::{Action, ActionRun, Graph, Port, PortType, TriggerOn, ACTION_PREFIX, BUILTIN_PREFIX, NODE_CHAT};
use crate::models::chat::{self, Chat, ChatItem, GridItem, ChatMode, ChatPage, ChatRoute, ChatSource, Intake, DEFAULT_PAGE, ITEM_MESSAGE, ITEM_NOTE, ITEM_RECORD};
use crate::models::{Conversation, SendMessageInput};
use crate::services::action_service::{self, Runner, Start, MAX_DEPTH};
use chrono::Utc;
use serde::Serialize;
use crate::utils::text;
use serde_json::{json, Map, Value};
use sqlx::{Pool, Row, Sqlite};
use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::pin::Pin;
use tauri::Emitter;
use uuid::Uuid;

pub const BACKFILL_LIMIT: i64 = 200;

type Boxed<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;


pub fn default_routes(action: &Action) -> Vec<ChatRoute> {
    action
        .outputs
        .iter()
        .filter(|p| matches!(p.kind, PortType::List | PortType::Json))
        .map(|p| ChatRoute { output: p.name.clone(), node: NODE_CHAT.to_string(), input: if p.kind == PortType::List { "items".into() } else { "item".into() }, values: Map::new() })
        .collect()
}

pub fn routes_for(action: &Action, stored: &[ChatRoute]) -> Vec<ChatRoute> {
    if stored.is_empty() {
        default_routes(action)
    } else {
        stored.to_vec()
    }
}

pub async fn feeds(pool: &Pool<Sqlite>, chat: &Chat) -> Result<bool, String> {
    if chat.mode != ChatMode::Read {
        return Ok(false);
    }
    for attachment in &chat.attachments {
        if let Ok(action) = Action::get(pool, &attachment.action_id).await {
            if routes_for(&action, &attachment.routes).iter().any(|r| r.node == NODE_CHAT) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[derive(Debug, Clone, Serialize)]
pub struct LandingNode {
    pub node: String,
    pub label: String,
    pub icon: String,
    pub inputs: Vec<Port>,
}

pub fn landing_nodes(actions: &[Action]) -> Vec<LandingNode> {
    let read = chat::node_spec().variant(ChatMode::Read);
    let mut out = vec![LandingNode { node: NODE_CHAT.to_string(), label: read.label.clone(), icon: read.icon.clone(), inputs: read.inputs.clone() }];
    let landings = action_service::landing_keys();
    for b in action_service::builtins().iter().filter(|b| landings.contains(&b.key)) {
        out.push(LandingNode { node: format!("{}{}", BUILTIN_PREFIX, b.key), label: b.label.to_string(), icon: b.icon.to_string(), inputs: b.inputs.clone() });
    }
    for action in actions.iter().filter(|a| a.usage.landing) {
        out.push(LandingNode { node: format!("{}{}", ACTION_PREFIX, action.id), label: action.name.clone(), icon: action.icon.clone().unwrap_or_else(|| "lucide:zap".into()), inputs: action.inputs.clone() });
    }
    out
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RouteProblem {
    pub index: usize,
    pub message: String,
}

pub fn validate_routes(action: &Action, routes: &[ChatRoute], landings: &[LandingNode]) -> Vec<RouteProblem> {
    let mut out = Vec::new();
    for (index, route) in routes.iter().enumerate() {
        let problem = |message: String| RouteProblem { index, message };
        let Some(output) = action.outputs.iter().find(|p| p.name == route.output) else {
            out.push(problem(format!("{} gives no output {}", action.name, route.output)));
            continue;
        };
        let Some(found) = landings.iter().find(|l| l.node == route.node && route.node != format!("{}{}", ACTION_PREFIX, action.id)) else {
            out.push(problem(format!("{} is not a place an action can land", route.node)));
            continue;
        };
        let landing = found.label.clone();
        let Some(input) = found.inputs.iter().find(|p| p.name == route.input) else {
            out.push(problem(format!("{} has no input {}", landing, route.input)));
            continue;
        };
        let each = output.kind == PortType::List && !matches!(input.kind, PortType::List | PortType::Json | PortType::Any);
        if !each && !input.kind.accepts(output.kind) {
            out.push(problem(format!("{} gives {}, but {} needs {} for {}", output.name, output.kind.name(), landing, input.kind.name(), input.name)));
        }
    }
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct PipeStep {
    pub action_id: String,
    pub name: String,
    pub icon: Option<String>,
    pub enabled: bool,
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
    pub routes: Vec<ChatRoute>,
    pub stored: bool,
    pub problems: Vec<RouteProblem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatPipe {
    pub conversation_id: String,
    pub mode: ChatMode,
    pub sources: Vec<ChatSource>,
    pub steps: Vec<PipeStep>,
    pub landings: Vec<LandingNode>,
}

pub async fn pipe(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str) -> Result<ChatPipe, String> {
    let chat = Chat::find(pool, profile_id, conversation_id).await?;
    let mode = mode_of(pool, profile_id, conversation_id).await?;
    let landings = landing_nodes(&Action::all(pool, profile_id).await?);
    let mut steps = Vec::new();
    for action_id in chat::action_ids(pool, conversation_id).await? {
        let Ok(action) = Action::get(pool, &action_id).await else { continue };
        let stored = chat::routes_of(pool, conversation_id, &action_id).await?;
        let routes = routes_for(&action, &stored);
        steps.push(PipeStep { problems: validate_routes(&action, &routes, &landings), action_id, name: action.name.clone(), icon: action.icon.clone(), enabled: action.enabled, inputs: action.inputs.clone(), outputs: action.outputs.clone(), stored: !stored.is_empty(), routes });
    }
    Ok(ChatPipe { conversation_id: conversation_id.to_string(), mode, sources: chat.map(|c| c.sources).unwrap_or_default(), steps, landings })
}

pub async fn put_routes(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str, action_id: &str, routes: &[ChatRoute]) -> Result<ChatPipe, String> {
    let action = Action::find(pool, profile_id, action_id).await?;
    let problems = validate_routes(&action, routes, &landing_nodes(&Action::all(pool, profile_id).await?));
    if !problems.is_empty() {
        return Err(problems.iter().map(|p| format!("landing {}: {}", p.index + 1, p.message)).collect::<Vec<_>>().join("; "));
    }
    chat::put_routes(pool, conversation_id, &action.id, routes).await?;
    pipe(pool, profile_id, conversation_id).await
}

async fn land(runner: &Runner, conversation_id: &str, action: &Action, run: &ActionRun, payload: &Value, local: bool, depth: i64) -> Result<(), String> {
    let stored = chat::routes_of(&runner.pool, conversation_id, &action.id).await?;
    for route in routes_for(action, &stored) {
        let Some(value) = run.outputs.get(&route.output).filter(|v| !v.is_null()).cloned() else { continue };
        if route.node == NODE_CHAT {
            if local {
                let records = match value {
                    Value::Array(list) => list,
                    other => vec![other],
                };
                deliver(runner, conversation_id, &action.id, &run.id, records, depth + 1).await?;
            }
            continue;
        }
        let (landing, constants) = match route.node.strip_prefix(ACTION_PREFIX) {
            Some(id) if id != action.id && depth + 1 <= MAX_DEPTH => match Action::get(&runner.pool, id).await {
                Ok(target) if target.usage.landing => (target, route.values.clone()),
                _ => continue,
            },
            Some(_) => continue,
            None => {
                let Some(builtin) = route.node.strip_prefix(BUILTIN_PREFIX).and_then(action_service::builtin) else { continue };
                let mut landing = action.clone();
                landing.inputs = builtin.inputs.clone();
                landing.graph = Graph { nodes: vec![action_service::builtin_node("landing", builtin.key, builtin.label, json!({ "values": route.values }), 0.0, 0.0)], edges: Vec::new() };
                (landing, Map::new())
            }
        };
        let kind = landing.inputs.iter().find(|p| p.name == route.input).map(|p| p.kind);
        let values = match (&value, kind) {
            (Value::Array(list), Some(kind)) if !matches!(kind, PortType::List | PortType::Json | PortType::Any) => list.clone(),
            _ => vec![value.clone()],
        };
        let stack = if landing.id == action.id { vec![action.id.clone()] } else { vec![action.id.clone(), landing.id.clone()] };
        for value in values {
            let mut inputs = constants.clone();
            inputs.insert(route.input.clone(), value);
            let landed = action_service::execute(runner, &landing, Start::Invoke { inputs, payload: payload.clone() }, depth + 1, stack.clone()).await?;
            action_service::record(runner, &landed).await?;
        }
    }
    Ok(())
}

pub fn items<'a>(pool: &'a Pool<Sqlite>, profile_id: &'a str, chat: &'a Chat, stack: Vec<String>, through: bool) -> Boxed<'a, Vec<ChatItem>> {
    Box::pin(async move {
        let mut out = Chat::own_items(pool, &chat.conversation_id).await?;
        if !through && feeds(pool, chat).await? {
            return Ok(out);
        }
        let intake = chat.intake(pool).await?;
        out.extend(Chat::messages_of(pool, &intake.conversations).await?);
        let mut stack = stack;
        stack.push(chat.conversation_id.clone());
        for source in intake.chats {
            if stack.contains(&source) {
                continue;
            }
            if let Some(inner) = Chat::find(pool, profile_id, &source).await? {
                out.extend(items(pool, profile_id, &inner, stack.clone(), through).await?);
            }
        }
        Ok(out)
    })
}

async fn all_items(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str, through: bool) -> Result<Vec<ChatItem>, String> {
    match Chat::find(pool, profile_id, conversation_id).await? {
        Some(chat) => items(pool, profile_id, &chat, Vec::new(), through).await,
        None => {
            if !Conversation::is_owned_by(pool, conversation_id, profile_id).await? {
                return Err("Conversation not found".to_string());
            }
            let mut own = Chat::own_items(pool, conversation_id).await?;
            own.extend(Chat::messages_of(pool, &BTreeSet::from([conversation_id.to_string()])).await?);
            Ok(own)
        }
    }
}

pub async fn page(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str, before: Option<&str>, limit: Option<usize>) -> Result<ChatPage, String> {
    let all = all_items(pool, profile_id, conversation_id, false).await?;
    let (items, next_before) = chat::page(all, before, limit.unwrap_or(DEFAULT_PAGE));
    if let Some(latest) = items.first().filter(|_| before.is_none()) {
        sqlx::query("UPDATE conversation SET last_activity = ?1 WHERE id = ?2 AND last_activity < ?1 AND type = ?3")
            .bind(&latest.at)
            .bind(conversation_id)
            .bind(chat::CONVERSATION_TYPE)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to touch the chat: {}", e))?;
    }
    Ok(ChatPage { chat_id: conversation_id.to_string(), items, next_before })
}

pub async fn grid(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str) -> Result<Vec<GridItem>, String> {
    let all = all_items(pool, profile_id, conversation_id, true).await?;
    let ids: BTreeSet<String> = all.iter().map(|i| i.conversation_id.clone()).collect();
    let mut providers = HashMap::new();
    for id in ids {
        let kind: Option<String> = sqlx::query_scalar("SELECT p.provider_type FROM conversation_provider cp JOIN provider p ON p.id = cp.provider_id WHERE cp.conversation_id = ?1 ORDER BY cp.is_primary DESC LIMIT 1")
            .bind(&id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to read the connection: {}", e))?;
        providers.insert(id, kind.unwrap_or_default());
    }
    let mut out = chat::grid(&all, &providers);
    out.sort_by(|a, b| b.at.cmp(&a.at));
    let mut seen = BTreeSet::new();
    out.retain(|f| seen.insert(f.id.clone()));
    Ok(out)
}

struct Flow {
    chats: Vec<Chat>,
    intakes: HashMap<String, Intake>,
    feeding: HashMap<String, bool>,
}

async fn flow(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Flow, String> {
    let chats = Chat::all(pool, profile_id).await?;
    let mut intakes = HashMap::new();
    let mut feeding = HashMap::new();
    for chat in &chats {
        intakes.insert(chat.conversation_id.clone(), chat.intake(pool).await?);
        feeding.insert(chat.conversation_id.clone(), feeds(pool, chat).await?);
    }
    Ok(Flow { chats, intakes, feeding })
}

fn shows(map: &Flow, chat: &str, wanted: &dyn Fn(&Intake) -> bool, seen: &mut BTreeSet<String>) -> bool {
    if !seen.insert(chat.to_string()) {
        return false;
    }
    let Some(intake) = map.intakes.get(chat) else { return false };
    if wanted(intake) {
        return true;
    }
    intake.chats.iter().any(|source| !map.feeding.get(source).copied().unwrap_or(false) && shows(map, source, wanted, seen))
}

pub async fn receivers(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str) -> Result<Vec<Chat>, String> {
    let map = flow(pool, profile_id).await?;
    let wanted = |intake: &Intake| intake.conversations.contains(conversation_id);
    Ok(map.chats.iter().filter(|c| shows(&map, &c.conversation_id, &wanted, &mut BTreeSet::new())).cloned().collect())
}

pub async fn readers(pool: &Pool<Sqlite>, profile_id: &str, chat_id: &str) -> Result<Vec<Chat>, String> {
    let map = flow(pool, profile_id).await?;
    let wanted = |intake: &Intake| intake.chats.iter().any(|c| c == chat_id);
    Ok(map.chats.iter().filter(|c| c.conversation_id != chat_id && shows(&map, &c.conversation_id, &wanted, &mut BTreeSet::new())).cloned().collect())
}

pub async fn mode_of(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str) -> Result<ChatMode, String> {
    let own: Option<String> = sqlx::query_scalar("SELECT mode FROM conversation WHERE id = ?1").bind(conversation_id).fetch_optional(pool).await.map_err(|e| e.to_string())?;
    if ChatMode::parse(&own.unwrap_or_default()) == ChatMode::Read {
        return Ok(ChatMode::Read);
    }
    let read = crate::services::group_service::read_conversation_ids(pool, profile_id).await?;
    Ok(if read.iter().any(|id| id == conversation_id) { ChatMode::Read } else { ChatMode::Write })
}

fn title_of(record: &Value) -> String {
    ["title", "name", "subject", "vendor"].iter().map(|k| text(record.get(*k))).find(|t| !t.trim().is_empty()).unwrap_or_else(|| "Record".to_string())
}

async fn record_payload(pool: &Pool<Sqlite>, profile_id: &str, record: &Value) -> Result<Value, String> {
    let summary = format!("{}\n{}", title_of(record), text(record.get("excerpt"))).trim().to_string();
    let mut payload = match record.get("message_id").and_then(Value::as_str) {
        Some(id) => action_service::payload_for_message(pool, profile_id, id).await.unwrap_or_else(|_| action_service::trigger_payload(Value::Null, Value::Null, "", Value::Null)),
        None => action_service::trigger_payload(Value::Null, Value::Null, "", Value::Null),
    };
    payload["item"] = record.clone();
    payload["text"] = json!(summary);
    Ok(payload)
}

pub async fn run_attached(runner: &Runner, conversation_id: &str, mode: ChatMode, payload: Value, depth: i64, only: Option<&str>) -> Result<Vec<ActionRun>, String> {
    if depth > MAX_DEPTH {
        return Ok(Vec::new());
    }
    let attached = chat::action_ids(&runner.pool, conversation_id).await?;
    let local = Chat::find(&runner.pool, &runner.profile_id, conversation_id).await?.is_some();
    let mut runs = Vec::new();
    for action in Action::all(&runner.pool, &runner.profile_id).await?.into_iter().filter(|a| a.enabled && !a.builtin && only.is_none_or(|id| id == a.id)) {
        let is_attached = attached.contains(&action.id);
        let start = if is_attached {
            Some(Start::Chat { conversation_id: conversation_id.to_string(), mode, payload: payload.clone() })
        } else {
            action
                .graph
                .nodes
                .iter()
                .find(|n| n.chat().is_some_and(|c| c.mode == mode && c.conversation_id.as_deref() == Some(conversation_id)) && !action.graph.edges.iter().any(|e| e.target == n.id))
                .map(|n| Start::Trigger { node: n.id.clone(), payload: payload.clone() })
        };
        let Some(start) = start else { continue };
        let run = action_service::execute(runner, &action, start, depth, vec![action.id.clone()]).await?;
        action_service::record(runner, &run).await?;
        if is_attached && run.status == "done" && !runner.dry_run {
            land(runner, conversation_id, &action, &run, &payload, local, depth).await?;
        }
        runs.push(run);
    }
    Ok(runs)
}

pub fn deliver<'a>(runner: &'a Runner, conversation_id: &'a str, action_id: &'a str, run_id: &'a str, records: Vec<Value>, depth: i64) -> Boxed<'a, usize> {
    Box::pin(async move {
        let pool = &runner.pool;
        if Chat::find(pool, &runner.profile_id, conversation_id).await?.is_none() {
            return Err("Only a chat made in Chatnip keeps records; choose one of those".to_string());
        }
        let mut fresh = Vec::new();
        let count = records.len();
        for (index, record) in records.into_iter().enumerate() {
            let record = match record {
                Value::Object(_) => record,
                Value::String(text) => json!({ "title": text.lines().next().unwrap_or("").chars().take(120).collect::<String>(), "text": text }),
                other => json!({ "title": other.to_string() }),
            };
            let own = record.get("id").and_then(Value::as_str).map(String::from).unwrap_or_else(|| format!("{}:{}", run_id, index));
            let key = format!("{}:{}", action_id, own);
            let at = text(record.get("at")).trim().to_string();
            let at = if at.is_empty() { Utc::now().to_rfc3339() } else { at };
            let message_id = record.get("message_id").and_then(Value::as_str).map(String::from);
            let (_, new) = Chat::put_item(pool, conversation_id, &key, ITEM_RECORD, &title_of(&record), None, &record, &at, (Some(action_id), Some(run_id), message_id.as_deref())).await?;
            if new {
                fresh.push(record);
            }
        }
        if let Some(app) = &runner.app {
            app.emit("chat-items-updated", &conversation_id).ok();
        }
        if !fresh.is_empty() {
            for reader in readers(pool, &runner.profile_id, conversation_id).await? {
                for record in &fresh {
                    let payload = record_payload(pool, &runner.profile_id, record).await?;
                    run_attached(runner, &reader.conversation_id, ChatMode::Read, payload, depth, None).await?;
                }
                if let Some(app) = &runner.app {
                    app.emit("chat-items-updated", &reader.conversation_id).ok();
                }
            }
        }
        Ok(count)
    })
}

async fn has_chat_start(runner: &Runner, conversation_id: &str) -> Result<bool, String> {
    if !chat::action_ids(&runner.pool, conversation_id).await?.is_empty() {
        return Ok(true);
    }
    Ok(Action::all(&runner.pool, &runner.profile_id).await?.iter().any(|a| a.graph.nodes.iter().any(|n| n.chat().is_some_and(|c| c.conversation_id.as_deref() == Some(conversation_id)))))
}

pub async fn dispatch_message(runner: &Runner, on: &[TriggerOn], payload: &Value, only: Option<&str>) -> Result<Vec<ActionRun>, String> {
    let arrival = on.contains(&TriggerOn::Arrival);
    let sent = on.contains(&TriggerOn::Sent);
    if !arrival && !sent {
        return Ok(Vec::new());
    }
    let conversation_id = text(payload["conversation"].get("id"));
    let mut runs = Vec::new();
    if !conversation_id.is_empty() && has_chat_start(runner, &conversation_id).await? {
        let mode = mode_of(&runner.pool, &runner.profile_id, &conversation_id).await?;
        if mode == ChatMode::Write || arrival {
            runs.extend(run_attached(runner, &conversation_id, mode, payload.clone(), 0, only).await?);
        }
    }
    if !conversation_id.is_empty() {
        for chat in receivers(&runner.pool, &runner.profile_id, &conversation_id).await? {
            if chat.mode == ChatMode::Read && !arrival {
                continue;
            }
            let mut item = payload.clone();
            item["item"] = payload["message"].clone();
            runs.extend(run_attached(runner, &chat.conversation_id, chat.mode, item, 0, only).await?);
        }
    }
    Ok(runs)
}

pub async fn post_message(runner: &Runner, conversation_id: &str, body: &str, target: Option<&str>, reply_to: Option<&str>) -> Result<ChatItem, String> {
    let body = body.trim();
    if body.is_empty() {
        return Err("There is nothing to write".to_string());
    }
    let chat = Chat::get(&runner.pool, &runner.profile_id, conversation_id).await?;
    if mode_of(&runner.pool, &runner.profile_id, &chat.conversation_id).await? == ChatMode::Read {
        return Err(format!("{} is a read chat; switch it to write to write in it", chat.name));
    }
    let through = chat.links();
    if !through.is_empty() {
        let reply_to = reply_to.filter(|r| !r.is_empty());
        let replied: Option<String> = match reply_to {
            Some(id) => crate::models::Message::get_conversation_id(&runner.pool, id).await?,
            None => None,
        };
        let target = match (target.filter(|t| !t.is_empty()), replied.filter(|r| through.iter().any(|c| c == r))) {
            (Some(t), _) if through.iter().any(|c| c == t) => t.to_string(),
            (Some(t), _) => return Err(format!("{} is not one of the chats {} writes through", t, chat.name)),
            (None, Some(r)) => r,
            (None, None) => latest(&runner.pool, &through).await?,
        };
        let metadata = reply_to.map(|id| json!({ "in_reply_to": id }));
        let input = SendMessageInput { conversation_id: target.clone(), content: body.to_string(), content_type: Some("markdown".into()), provider_id: None, metadata, attachments: None };
        let message = crate::services::outbound_service::send(&runner.pool, &runner.app_data, input).await?;
        let name = crate::models::Conversation::get_name(&runner.pool, &target).await?.unwrap_or_default();
        if !runner.dry_run {
            action_service::dispatch(runner, &[TriggerOn::Sent], &message.id).await?;
        }
        if let Some(app) = &runner.app {
            app.emit("chat-items-updated", &chat.conversation_id).ok();
            app.emit("conversation-sent", &target).ok();
        }
        return Ok(ChatItem { id: message.id.clone(), kind: ITEM_MESSAGE.into(), title: message.subject.clone().unwrap_or_default(), at: message.timestamp.clone(), message: Some(message), record: None, text: None, conversation_id: target, conversation_name: name, conversation_type: String::new(), action_id: None, reactions: 0 });
    }
    let now = Utc::now().to_rfc3339();
    let title: String = body.lines().next().unwrap_or("").chars().take(120).collect();
    let (id, _) = Chat::put_item(&runner.pool, &chat.conversation_id, &Uuid::new_v4().to_string(), ITEM_NOTE, &title, Some(body), &json!({ "text": body, "reply_to_id": reply_to }), &now, (None, None, None)).await?;
    let conversation = action_service::conversation_value(&runner.pool, &chat.conversation_id).await?;
    let payload = action_service::trigger_payload(Value::Null, conversation, body, json!({ "item": { "id": id, "text": body } }));
    run_attached(runner, &chat.conversation_id, ChatMode::Write, payload, 0, None).await?;
    if let Some(app) = &runner.app {
        app.emit("chat-items-updated", &chat.conversation_id).ok();
    }
    Chat::own_items(&runner.pool, &chat.conversation_id).await?.into_iter().find(|i| i.id == id).ok_or_else(|| "The note was not kept".to_string())
}

async fn latest(pool: &Pool<Sqlite>, conversations: &[String]) -> Result<String, String> {
    let marks = (0..conversations.len()).map(|i| format!("?{}", i + 1)).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT c.id FROM conversation c WHERE c.id IN ({}) ORDER BY COALESCE((SELECT MAX(m.timestamp) FROM message m JOIN provider p ON p.id = m.provider_id WHERE m.conversation_id = c.id AND m.sender_id = p.external_id), '') DESC, c.last_activity DESC LIMIT 1",
        marks
    );
    let mut query = sqlx::query_scalar::<_, String>(&sql);
    for id in conversations {
        query = query.bind(id);
    }
    query.fetch_optional(pool).await.map_err(|e| format!("Failed to pick where to send: {}", e))?.ok_or_else(|| "None of the chats it writes through exists any more".to_string())
}

pub async fn backfill(runner: &Runner, conversation_id: &str, limit: i64) -> Result<usize, String> {
    let pool = &runner.pool;
    let (sources, mode) = match Chat::find(pool, &runner.profile_id, conversation_id).await? {
        Some(chat) => (chat.intake(pool).await?.conversations.into_iter().collect::<Vec<_>>(), ChatMode::Read),
        None => (vec![conversation_id.to_string()], mode_of(pool, &runner.profile_id, conversation_id).await?),
    };
    let mut runs = 0;
    if !sources.is_empty() {
        let marks = (0..sources.len()).map(|i| format!("?{}", i + 2)).collect::<Vec<_>>().join(", ");
        let sql = format!("SELECT id FROM message WHERE conversation_id IN ({}) AND deleted_datetime IS NULL ORDER BY timestamp DESC LIMIT ?1", marks);
        let mut query = sqlx::query(&sql).bind(limit.clamp(1, 2000));
        for source in &sources {
            query = query.bind(source);
        }
        let ids: Vec<String> = query.fetch_all(pool).await.map_err(|e| format!("Failed to list what flows in: {}", e))?.into_iter().map(|r| r.get("id")).collect();
        for id in ids.into_iter().rev() {
            let mut payload = action_service::payload_for_message(pool, &runner.profile_id, &id).await?;
            payload["item"] = payload["message"].clone();
            runs += run_attached(runner, conversation_id, mode, payload, 0, None).await?.len();
        }
    }
    Chat::mark_backfilled(pool, conversation_id).await?;
    if let Some(app) = &runner.app {
        app.emit("chat-items-updated", &conversation_id).ok();
    }
    Ok(runs)
}

pub async fn attach(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str, action_id: &str) -> Result<(), String> {
    let action = Action::find(pool, profile_id, action_id).await?;
    if action.builtin {
        return Err(format!("{} is built in and read only; duplicate it without its trigger and attach the copy", action.name));
    }
    if !action.usage.chat {
        return Err(format!("{} does not fit a chat: it starts from its own trigger, or it neither ends in a landing nor gives outputs to land. Remove the trigger, or duplicate it without its trigger, to attach it to a chat", action.name));
    }
    if !Conversation::is_owned_by(pool, conversation_id, profile_id).await? {
        return Err("Conversation not found".to_string());
    }
    chat::put_action(pool, conversation_id, &action.id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chat::{ChatSource, PostChatInput, PutChatInput, SOURCE_CONVERSATION};
    use crate::models::action::Node;
    use crate::test_helpers::{test_conversation, test_message, test_pool, test_profile, test_provider};
    use std::sync::Arc;

    fn runner(pool: &Pool<Sqlite>, profile: &str) -> Runner {
        Runner {
            test_delay: None,
            pool: pool.clone(),
            app: None,
            app_data: std::env::temp_dir(),
            cache: std::env::temp_dir().join("chatnip-chat-test-cache"),
            profile_id: profile.into(),
            dry_run: false,
            ai: Some(Arc::new(|key: &str, extra: &Value| match key {
                "receipt_confirm" => Ok(json!({ "verdicts": extra["candidates"].as_array().cloned().unwrap_or_default().into_iter().map(|id| json!({ "id": id, "receipt": true, "confidence": 0.95 })).collect::<Vec<_>>() })),
                other => Err(format!("unexpected prompt {}", other)),
            })),
        }
    }

    async fn owned(pool: &Pool<Sqlite>, conversation: &str, provider: &str) {
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id, is_primary) VALUES (?1, ?2, ?3, ?1, 1)").bind(Uuid::new_v4().to_string()).bind(conversation).bind(provider).execute(pool).await.unwrap();
    }

    fn source(id: &str) -> ChatSource {
        ChatSource { kind: SOURCE_CONVERSATION.into(), id: Some(id.into()), query: None }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_untriggered_action_attaches_to_a_read_chat_and_feeds_it() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "mock").await;
        action_service::ensure_defaults(&pool, &profile).await.unwrap();
        let mail = test_conversation(&pool, "Nordlys Elektro", "email").await;
        owned(&pool, &mail, &provider).await;
        let receipt = test_message(&pool, &mail, &provider, "<p>Takk for handelen! Kvittering for ordre 2026-48213.</p><p>Totalt 11 916,00 kr</p><p>Mva 25 % 2 383,20</p><p>Org.nr. 912 345 678 MVA</p>").await;
        sqlx::query("UPDATE message SET type = 'email', content_type = 'html', subject = 'Kvittering for ordre 2026-48213', sender_name = 'Nordlys Elektro', sender_email = 'kvittering@nordlyselektro.no' WHERE id = ?1").bind(&receipt).execute(&pool).await.unwrap();
        let other = test_message(&pool, &mail, &provider, "Ukens tilbud! Meld deg av nyhetsbrevet her.").await;

        let builtin = Action::find(&pool, &profile, action_service::RECEIPTS_KEY).await.unwrap();
        assert!(builtin.builtin && !builtin.usage.chat);
        let reading = Chat::post(&pool, &profile, &PostChatInput { name: "Receipts".into(), mode: ChatMode::Read, sources: vec![source(&mail)], ..Default::default() }).await.unwrap();
        assert!(attach(&pool, &profile, &reading.conversation_id, &builtin.id).await.unwrap_err().contains("read only"));
        let triggered = action_service::post_action_copy(&pool, &profile, &builtin.id, false, None).await.unwrap();
        assert!(!triggered.usage.chat);
        assert!(attach(&pool, &profile, &reading.conversation_id, &triggered.id).await.unwrap_err().contains("own trigger"));

        let copy = action_service::post_action_copy(&pool, &profile, &builtin.id, true, Some("Receipts for my chat")).await.unwrap();
        assert!(copy.usage.chat && copy.enabled, "{:?}", copy);
        assert!(copy.graph.nodes.iter().all(|n| n.kind != crate::models::action::NODE_TRIGGER));
        attach(&pool, &profile, &reading.conversation_id, &copy.id).await.unwrap();
        assert_eq!(Action::get(&pool, &copy.id).await.unwrap().chats, vec![reading.conversation_id.clone()]);

        let r = runner(&pool, &profile);
        let runs = action_service::dispatch_to(&r, &[TriggerOn::Arrival], &receipt, None).await.unwrap();
        let mine: Vec<&ActionRun> = runs.iter().filter(|run| run.action_id == copy.id).collect();
        assert_eq!(mine.len(), 1, "{:?}", runs.iter().map(|r| &r.action_name).collect::<Vec<_>>());
        assert_eq!(mine[0].status, "done", "{:?}", mine[0]);
        assert!(mine[0].log.iter().any(|l| l.kind == crate::models::action::NODE_CHAT && l.detail.contains("Receipts")), "{:?}", mine[0].log);
        action_service::dispatch_to(&r, &[TriggerOn::Arrival], &other, Some(&copy.id)).await.unwrap();

        let page = page(&pool, &profile, &reading.conversation_id, None, None).await.unwrap();
        assert_eq!(page.items.len(), 1, "{:?}", page.items);
        let record = page.items[0].record.as_ref().unwrap();
        assert_eq!(record["vendor"], "Nordlys Elektro");
        assert_eq!(record["total"], 11916.0);
        assert_eq!(record["confirmed"], true);
        assert_eq!(page.items[0].kind, ITEM_RECORD);
        let tiles = grid(&pool, &profile, &reading.conversation_id).await.unwrap();
        let kinds: Vec<&str> = tiles.iter().map(|t| t.kind.as_str()).collect();
        assert!(kinds.contains(&chat::GRID_RECORD) && kinds.contains(&chat::GRID_MESSAGE), "{:?}", kinds);
        assert!(tiles.iter().find(|t| t.kind == chat::GRID_RECORD).unwrap().tags.contains(&"Nordlys Elektro".to_string()));

        action_service::dispatch_to(&r, &[TriggerOn::Arrival], &receipt, Some(&copy.id)).await.unwrap();
        assert_eq!(Chat::own_items(&pool, &reading.conversation_id).await.unwrap().len(), 1);

        let digest = Chat::post(&pool, &profile, &PostChatInput { name: "Digest".into(), mode: ChatMode::Read, sources: vec![source(&reading.conversation_id)], ..Default::default() }).await.unwrap();
        let shown = page_items(&pool, &profile, &digest.conversation_id).await;
        assert_eq!(shown, 1);
        assert_eq!(readers(&pool, &profile, &reading.conversation_id).await.unwrap().len(), 1);
        assert_eq!(receivers(&pool, &profile, &mail).await.unwrap().iter().map(|c| c.name.clone()).collect::<Vec<_>>(), vec!["Receipts".to_string()]);
    }

    async fn page_items(pool: &Pool<Sqlite>, profile: &str, conversation: &str) -> usize {
        page(pool, profile, conversation, None, None).await.unwrap().items.len()
    }

    #[tokio::test]
    async fn read_chats_source_read_chats_to_any_depth_and_refuse_loops() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "mock").await;
        let channel = test_conversation(&pool, "Releases", "channel").await;
        owned(&pool, &channel, &provider).await;
        test_message(&pool, &channel, &provider, "v1.2 shipped").await;
        let read = |name: &str, sources: Vec<ChatSource>| PostChatInput { name: name.into(), mode: ChatMode::Read, sources, ..Default::default() };
        let a = Chat::post(&pool, &profile, &read("A", vec![source(&channel)])).await.unwrap();
        let b = Chat::post(&pool, &profile, &read("B", vec![source(&a.conversation_id)])).await.unwrap();
        let c = Chat::post(&pool, &profile, &read("C", vec![source(&b.conversation_id)])).await.unwrap();
        let d = Chat::post(&pool, &profile, &read("D", vec![source(&c.conversation_id)])).await.unwrap();
        assert_eq!(page_items(&pool, &profile, &d.conversation_id).await, 1);
        assert_eq!(receivers(&pool, &profile, &channel).await.unwrap().len(), 4);

        let looped = Chat::put(&pool, &profile, &PutChatInput { chat_id: a.id.clone(), name: None, icon: None, sources: Some(vec![source(&channel), source(&d.conversation_id)]) }).await.unwrap_err();
        assert!(looped.contains("loop") && looped.contains("A → D → C → B → A"), "{}", looped);
        let itself = Chat::put(&pool, &profile, &PutChatInput { chat_id: b.id.clone(), name: None, icon: None, sources: Some(vec![source(&b.conversation_id)]) }).await.unwrap_err();
        assert!(itself.contains("B → B"), "{}", itself);
        assert_eq!(Chat::get(&pool, &profile, &a.id).await.unwrap().sources, vec![source(&channel)]);
        let fine = Chat::put(&pool, &profile, &PutChatInput { chat_id: d.id.clone(), name: None, icon: None, sources: Some(vec![source(&a.conversation_id), source(&c.conversation_id)]) }).await.unwrap();
        assert_eq!(fine.sources.len(), 2);
        assert_eq!(page_items(&pool, &profile, &d.conversation_id).await, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_write_chat_keeps_notes_and_refuses_them_in_read_mode() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let r = runner(&pool, &profile);
        let console = Chat::post(&pool, &profile, &PostChatInput { name: "Console".into(), mode: ChatMode::Write, ..Default::default() }).await.unwrap();
        let note = post_message(&r, &console.conversation_id, "Husk kvitteringen fra Elkjøp", None, None).await.unwrap();
        assert_eq!(note.kind, ITEM_NOTE);
        assert_eq!(note.text.as_deref(), Some("Husk kvitteringen fra Elkjøp"));
        sqlx::query("UPDATE conversation SET mode = 'read' WHERE id = ?1").bind(&console.conversation_id).execute(&pool).await.unwrap();
        assert!(post_message(&r, &console.conversation_id, "nope", None, None).await.unwrap_err().contains("read chat"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn chat_nodes_start_from_a_write_chat_and_deliver_into_a_read_chat() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "mock").await;
        let team = test_conversation(&pool, "Team", "group_message").await;
        owned(&pool, &team, &provider).await;
        let message = test_message(&pool, &team, &provider, "Lunsj på fredag?").await;
        let log = Chat::post(&pool, &profile, &PostChatInput { name: "Team log".into(), mode: ChatMode::Read, ..Default::default() }).await.unwrap();
        let node = |id: &str, kind: &str, config: Value, body: &str| crate::models::action::Node { id: id.into(), kind: kind.into(), label: String::new(), config, body: body.into(), x: 0.0, y: 0.0 };
        let mut action = Action::blank(&profile, "Log the team");
        action.graph = crate::models::action::Graph {
            nodes: vec![
                node("from", crate::models::action::NODE_CHAT, json!({ "mode": "write", "conversation_id": team }), ""),
                node("shape", crate::models::action::NODE_SCRIPT, json!({ "inputs": [{ "name": "text", "type": "text" }], "outputs": [{ "name": "item", "type": "json" }] }), "#{ item: #{ id: \"line\", title: text } }"),
                node("into", crate::models::action::NODE_CHAT, json!({ "mode": "read", "conversation_id": log.conversation_id }), ""),
            ],
            edges: vec![action_service::wire("from", "text", "shape", "text"), action_service::wire("shape", "item", "into", "item")],
        };
        let diagnostics = action_service::check(&crate::services::action_script::ActionDraft { name: action.name.clone(), graph: action.graph.clone(), ..Default::default() }, &HashMap::new(), None);
        assert!(diagnostics.is_empty(), "{:?}", diagnostics);
        let action = Action::insert(&pool, &action).await.unwrap();
        assert!(action.usage.chat);
        let runs = action_service::dispatch_to(&runner(&pool, &profile), &[TriggerOn::Arrival], &message, Some(&action.id)).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "done", "{:?}", runs[0]);
        let items = Chat::own_items(&pool, &log.conversation_id).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Lunsj på fredag?");
        let elsewhere = test_conversation(&pool, "Other", "group_message").await;
        owned(&pool, &elsewhere, &provider).await;
        let other = test_message(&pool, &elsewhere, &provider, "Hei").await;
        assert!(action_service::dispatch_to(&runner(&pool, &profile), &[TriggerOn::Arrival], &other, Some(&action.id)).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_receipts_built_in_checks_clean_and_never_runs_on_its_own() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        action_service::ensure_defaults(&pool, &profile).await.unwrap();
        let receipts = Action::find(&pool, &profile, action_service::RECEIPTS_KEY).await.unwrap();
        let ids: Vec<&str> = receipts.graph.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["start", "extract", "match", "confirm", "organise"]);
        assert_eq!(action_service::topo_order(&receipts.graph).unwrap(), vec!["start", "extract", "match", "confirm", "organise"]);
        let all = action_service::others(&pool, &profile).await.unwrap();
        let draft = crate::services::action_script::ActionDraft { name: receipts.name.clone(), graph: receipts.graph.clone(), ..Default::default() };
        assert!(action_service::check(&draft, &all, Some(&receipts.id)).is_empty());
        let patterns = receipts.graph.nodes.iter().find(|n| n.id == "match").unwrap().constant("patterns").unwrap();
        assert_eq!(crate::services::receipt_service::patterns_from(&patterns).unwrap(), crate::services::receipt_service::default_patterns());
        action_service::ensure_defaults(&pool, &profile).await.unwrap();
        assert_eq!(Action::all(&pool, &profile).await.unwrap().iter().filter(|a| a.key.as_deref() == Some(action_service::RECEIPTS_KEY)).count(), 1);
    }

    fn route(output: &str, node: &str, input: &str, values: Value) -> ChatRoute {
        ChatRoute { output: output.into(), node: node.into(), input: input.into(), values: values.as_object().cloned().unwrap_or_default() }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn attached_outputs_route_to_landings_once_their_schemas_match() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "mock").await;
        crate::models::Tag::ensure_defaults(&pool, &profile).await.unwrap();
        let mail = test_conversation(&pool, "Elkjøp", "email").await;
        owned(&pool, &mail, &provider).await;
        let message = test_message(&pool, &mail, &provider, "Kvitteringen ligger vedlagt").await;
        sqlx::query("UPDATE message SET subject = 'ordre 5521093' WHERE id = ?1").bind(&message).execute(&pool).await.unwrap();
        let mut action = Action::blank(&profile, "Shape");
        action.graph = Graph {
            nodes: vec![Node {
                id: "shape".into(),
                kind: crate::models::action::NODE_SCRIPT.into(),
                label: String::new(),
                config: json!({ "inputs": [{ "name": "message", "type": "message" }, { "name": "conversation", "type": "conversation" }], "outputs": [{ "name": "summary", "type": "text" }, { "name": "conversation", "type": "conversation" }, { "name": "records", "type": "list" }] }),
                body: "#{ summary: \"Kvittering \" + message.subject, conversation: conversation, records: [#{ id: \"r1\", title: \"R1\" }] }".into(),
                x: 0.0,
                y: 0.0,
            }],
            edges: Vec::new(),
        };
        let action = Action::insert(&pool, &action).await.unwrap();
        let reading = Chat::post(&pool, &profile, &PostChatInput { name: "Digest".into(), mode: ChatMode::Read, sources: vec![source(&mail)], ..Default::default() }).await.unwrap();
        attach(&pool, &profile, &reading.conversation_id, &action.id).await.unwrap();

        let defaults = pipe(&pool, &profile, &reading.conversation_id).await.unwrap();
        assert_eq!(defaults.steps[0].routes, vec![route("records", NODE_CHAT, "items", json!({}))]);
        assert!(!defaults.steps[0].stored && defaults.steps[0].problems.is_empty());
        assert!(defaults.landings.iter().any(|l| l.node == NODE_CHAT) && defaults.landings.iter().any(|l| l.node == "builtin:tag_add"));

        let wrong = vec![route("summary", "builtin:tag_add", "conversation", json!({})), route("nope", NODE_CHAT, "item", json!({})), route("summary", "builtin:wait", "value", json!({})), route("summary", "builtin:tag_add", "planet", json!({}))];
        let problems = validate_routes(&action, &wrong, &landing_nodes(&[]));
        assert_eq!(problems.iter().map(|p| p.index).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        assert!(problems[0].message.contains("summary gives text, but Add tag needs conversation"), "{:?}", problems);
        assert!(problems[1].message.contains("no output nope"));
        assert!(problems[2].message.contains("not a place"));
        assert!(problems[3].message.contains("no input planet"));
        assert!(put_routes(&pool, &profile, &reading.conversation_id, &action.id, &wrong).await.unwrap_err().contains("landing 1"));
        assert!(validate_routes(&action, &[route("records", "builtin:done", "conversation", json!({}))], &landing_nodes(&[])).is_empty());

        let routes = vec![route("conversation", "builtin:tag_add", "conversation", json!({ "tag": "Important" })), route("summary", NODE_CHAT, "item", json!({})), route("summary", "builtin:notify", "title", json!({}))];
        let saved = put_routes(&pool, &profile, &reading.conversation_id, &action.id, &routes).await.unwrap();
        assert!(saved.steps[0].stored && saved.steps[0].routes == routes);

        let runs = action_service::dispatch_to(&runner(&pool, &profile), &[TriggerOn::Arrival], &message, Some(&action.id)).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "done", "{:?}", runs[0]);
        let tags: Vec<String> = sqlx::query_scalar("SELECT t.name FROM object_tag ct JOIN tag t ON t.id = ct.tag_id WHERE ct.object_type = 'conversation' AND ct.object_id = ?1").bind(&mail).fetch_all(&pool).await.unwrap();
        assert_eq!(tags, vec!["Important".to_string()]);
        let items = Chat::own_items(&pool, &reading.conversation_id).await.unwrap();
        assert_eq!(items.iter().map(|i| i.title.clone()).collect::<Vec<_>>(), vec!["Kvittering ordre 5521093".to_string()]);
        let notified: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM action_run WHERE action_id = ?1 AND log LIKE '%Notified: Kvittering ordre 5521093%'").bind(&action.id).fetch_one(&pool).await.unwrap();
        assert_eq!(notified, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_write_chat_sends_through_the_chats_merged_into_it() {
        use crate::providers::mock::{MockKind, MockProvider};
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let id = test_provider(&pool, &profile, "mock").await;
        sqlx::query("UPDATE provider SET external_id = 'mock-teams-me', namespace = 'teams', config = 'teams', name = 'Mock Teams', access_token = 'mock', expires_at = ?1 WHERE id = ?2")
            .bind((Utc::now() + chrono::Duration::days(365)).to_rfc3339())
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        let provider = crate::providers::models::Provider::load_from_db(&pool, &id).await.unwrap();
        provider.ingest(&pool, None, std::sync::Arc::new(MockProvider::new(MockKind::Teams, pool.clone()))).await.unwrap();
        crate::models::AppSetting::put(&pool, crate::models::app_setting::MOCK_ENABLED, "true").await.unwrap();
        let picked: Vec<String> = sqlx::query_scalar("SELECT c.id FROM conversation c JOIN conversation_provider cp ON cp.conversation_id = c.id WHERE cp.provider_id = ?1 AND c.type = 'direct_message' ORDER BY c.last_activity DESC LIMIT 2").bind(&id).fetch_all(&pool).await.unwrap();
        assert_eq!(picked.len(), 2);
        let team = Chat::post(&pool, &profile, &PostChatInput { name: "Mia everywhere".into(), mode: ChatMode::Write, sources: vec![source(&picked[0]), source(&picked[1])], ..Default::default() }).await.unwrap();
        let links: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM conversation_link WHERE source_conversation_id = ?1 AND link_type = 'chat'").bind(&team.conversation_id).fetch_one(&pool).await.unwrap();
        assert_eq!(links, 2);
        let mut r = runner(&pool, &profile);
        r.app_data = std::env::temp_dir();
        let sent = post_message(&r, &team.conversation_id, "Hei fra Chatnip", Some(&picked[1]), None).await.unwrap();
        assert_eq!(sent.kind, ITEM_MESSAGE);
        assert_eq!(sent.conversation_id, picked[1]);
        let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE conversation_id = ?1 AND content LIKE '%Hei fra Chatnip%'").bind(&picked[1]).fetch_one(&pool).await.unwrap();
        assert_eq!(stored, 1);
        let again = post_message(&r, &team.conversation_id, "Og en til", None, None).await.unwrap();
        assert_eq!(again.conversation_id, picked[1]);
        let elsewhere = test_conversation(&pool, "Elsewhere", "direct_message").await;
        assert!(post_message(&r, &team.conversation_id, "Nei", Some(&elsewhere), None).await.unwrap_err().contains("writes through"));
        let (quoted, quoted_external): (String, String) = sqlx::query_as("SELECT id, external_id FROM message WHERE conversation_id = ?1 ORDER BY timestamp DESC LIMIT 1").bind(&picked[0]).fetch_one(&pool).await.unwrap();
        let reposted = post_message(&r, &team.conversation_id, "Svar på den", None, Some(&quoted)).await.unwrap();
        assert_eq!(reposted.message.as_ref().and_then(|m| m.reply_to_id.clone()), Some(quoted.clone()));
        assert_eq!(crate::services::connection_service::put_capabilities(&pool).await.unwrap(), 1);
        let answer = post_message(&r, &team.conversation_id, "Svar på den", None, Some(&quoted)).await.unwrap();
        assert_eq!(answer.conversation_id, picked[0]);
        assert_eq!(answer.message.as_ref().and_then(|m| m.reply_to_id.clone()), Some(quoted_external));
        let timeline = page(&pool, &profile, &team.conversation_id, None, Some(500)).await.unwrap();
        assert!(timeline.items.iter().any(|i| i.message.as_ref().is_some_and(|m| m.content.as_deref().unwrap_or("").contains("Hei fra Chatnip"))));
        assert!(timeline.items.iter().any(|i| i.conversation_id == picked[0]));
        assert!(Chat::own_items(&pool, &team.conversation_id).await.unwrap().is_empty());
        Chat::delete(&pool, &profile, &team.id).await.unwrap();
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM conversation_link WHERE source_conversation_id = ?1").bind(&team.conversation_id).fetch_one(&pool).await.unwrap();
        assert_eq!(left, 0);
    }
}
