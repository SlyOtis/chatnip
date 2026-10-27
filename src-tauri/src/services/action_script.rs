use crate::models::action::{Edge, Graph, Node, Port, PortType};
use async_trait::async_trait;
use rhai::{ASTNode, Dynamic, Engine, EvalAltResult, Expr, Position, Scope};
use std::collections::HashSet;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MAX_OPERATIONS: u64 = 500_000;
pub const MAX_SECONDS: u64 = 20;
pub const MAX_STRING: usize = 1_000_000;
pub const MAX_COLLECTION: usize = 10_000;

#[async_trait]
pub trait Effects: Send + Sync {
    async fn call(&self, function: &str, args: Vec<Value>) -> Result<Value, String>;
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionSpec {
    pub name: &'static str,
    pub params: Vec<(&'static str, PortType)>,
    pub required: usize,
    pub returns: PortType,
    pub description: &'static str,
    pub effect: bool,
    pub signature: String,
    pub requires: Vec<&'static str>,
}

pub const REQUIRES_AI_ENABLED: &str = "ai.enabled";
pub const REQUIRES_AI_CONNECTION: &str = "ai.connection";
pub const REQUIRES_AI_MODEL: &str = "ai.default-model";
pub const REQUIRES_AI_CLAUDE: &str = "ai.claude";
pub const REQUIRES_AI_OLLAMA: &str = "ai.ollama";
pub const REQUIRES_ACTIONS_ENABLED: &str = "actions.enabled";
pub const REQUIRES_ACCOUNT: &str = "accounts.connections";
pub const REQUIRES_VOICE_ENABLED: &str = "voice.enabled";
pub const REQUIRES_VOICE_MODELS: &str = "voice.models";

const AI_REQUIREMENTS: &[&str] = &[REQUIRES_AI_ENABLED, REQUIRES_AI_CONNECTION, REQUIRES_AI_MODEL];
const SEND_REQUIREMENTS: &[&str] = &[REQUIRES_ACCOUNT];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Requirement {
    pub key: &'static str,
    pub label: &'static str,
}

pub const REQUIREMENT_AI_ENABLED: Requirement = Requirement { key: REQUIRES_AI_ENABLED, label: "Intelligence switched on" };
pub const REQUIREMENT_AI_CONNECTION: Requirement = Requirement { key: REQUIRES_AI_CONNECTION, label: "An AI connection" };
pub const REQUIREMENT_AI_MODEL: Requirement = Requirement { key: REQUIRES_AI_MODEL, label: "A default AI model" };
pub const REQUIREMENT_AI_ACCESS: Requirement = Requirement { key: REQUIRES_AI_CONNECTION, label: "An AI connection that accepts its key" };
pub const REQUIREMENT_CLAUDE_INSTALLED: Requirement = Requirement { key: REQUIRES_AI_CLAUDE, label: "Claude Code installed" };
pub const REQUIREMENT_CLAUDE_SIGNED_IN: Requirement = Requirement { key: REQUIRES_AI_CLAUDE, label: "Claude Code signed in" };
pub const REQUIREMENT_OLLAMA_RUNNING: Requirement = Requirement { key: REQUIRES_AI_OLLAMA, label: "Ollama running" };
pub const REQUIREMENT_OLLAMA_MODEL: Requirement = Requirement { key: REQUIRES_AI_OLLAMA, label: "The model pulled in Ollama" };

pub const REQUIREMENT: &str = "requirement";
const UNMET: &str = "Needs setting ";

impl Requirement {
    pub fn error(&self) -> String {
        format!("{}{}: {}", UNMET, self.key, self.label)
    }
}

pub fn requirements() -> Vec<Requirement> {
    vec![
        Requirement { key: REQUIRES_ACTIONS_ENABLED, label: "Actions switched on" },
        REQUIREMENT_AI_ENABLED,
        REQUIREMENT_AI_CONNECTION,
        REQUIREMENT_AI_MODEL,
        Requirement { key: REQUIRES_AI_CLAUDE, label: "Claude Code installed and signed in" },
        Requirement { key: REQUIRES_AI_OLLAMA, label: "Ollama running with the model pulled" },
        Requirement { key: REQUIRES_ACCOUNT, label: "A connected account" },
        Requirement { key: REQUIRES_VOICE_ENABLED, label: "Voice switched on" },
        Requirement { key: REQUIRES_VOICE_MODELS, label: "The voice models set up" },
    ]
}

pub fn requirement(key: &str) -> Option<Requirement> {
    requirements().into_iter().find(|r| r.key == key)
}

pub fn unmet_of(error: &str) -> Option<Diagnostic> {
    let rest = &error[error.find(UNMET)? + UNMET.len()..];
    let (key, label) = rest.split_once(": ")?;
    let known = requirement(key)?;
    let label = label.lines().next().unwrap_or_default().trim();
    Some(Diagnostic { message: if label.is_empty() { known.label.to_string() } else { label.to_string() }, ..Diagnostic::requirement(&known) })
}

pub fn function_requirements(name: &str) -> &'static [&'static str] {
    match name {
        "ai" | "ai_json" | "classify" | "summarize" | "prompt" => AI_REQUIREMENTS,
        "reply" | "send" | "draft" | "forward" | "schedule_send" => SEND_REQUIREMENTS,
        _ => &[],
    }
}

fn spec(name: &'static str, params: &[(&'static str, PortType)], required: usize, returns: PortType, effect: bool, description: &'static str) -> FunctionSpec {
    let args: Vec<String> = params
        .iter()
        .enumerate()
        .map(|(i, (p, t))| if i < required { format!("{}: {}", p, t.name()) } else { format!("{}?: {}", p, t.name()) })
        .collect();
    FunctionSpec { name, params: params.to_vec(), required, returns, description, effect, signature: format!("{}({}) -> {}", name, args.join(", "), returns.name()), requires: function_requirements(name).to_vec() }
}

fn code_only(body: &str) -> String {
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::with_capacity(body.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i += 2;
            out.push(' ');
            continue;
        }
        if c == '"' || c == '`' || c == '\'' {
            i += 1;
            while i < chars.len() && chars[i] != c {
                if chars[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            out.push_str("\"\"");
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

pub fn calls(body: &str) -> Vec<&'static str> {
    let code = code_only(body);
    let chars: Vec<char> = code.chars().collect();
    let mut found: Vec<String> = Vec::new();
    let mut previous = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let before = chars[..start].iter().rev().find(|c| !c.is_whitespace()).copied();
            let after = chars[i..].iter().find(|c| !c.is_whitespace()).copied();
            if after == Some('(') && before != Some('.') && previous != "fn" {
                found.push(word.clone());
            }
            previous = word;
            continue;
        }
        if !chars[i].is_whitespace() {
            previous.clear();
        }
        i += 1;
    }
    library().iter().map(|f| f.name).filter(|name| found.iter().any(|w| w == name)).collect()
}

pub fn is_effect(name: &str) -> bool {
    library().iter().any(|f| f.name == name && f.effect)
}

pub fn library() -> &'static [FunctionSpec] {
    static LIBRARY: std::sync::OnceLock<Vec<FunctionSpec>> = std::sync::OnceLock::new();
    LIBRARY.get_or_init(|| {
    use PortType::*;
    vec![
        spec("ai", &[("prompt", Text), ("options", Json)], 1, Text, false, "Ask the chosen AI. options.conversation adds that conversation as context and answers as a reply draft."),
        spec("ai_json", &[("prompt", Text), ("schema", Json)], 2, Json, false, "Ask the chosen AI for an answer shaped by a JSON schema; returns the parsed object."),
        spec("classify", &[("text", Text), ("labels", List)], 2, Tag, false, "Pick the one label from the list that fits the text best."),
        spec("summarize", &[("text", Text)], 1, Text, false, "One or two sentences on what the text says."),
        spec("get_message", &[("id", Text)], 1, Message, false, "A message by id: id, conversation_id, sender, sender_email, subject, text, at."),
        spec("get_conversation", &[("id", Text)], 1, Conversation, false, "A conversation by id: id, name, kind, provider, tags, person_id."),
        spec("list_messages", &[("conversation", Conversation), ("filter", Json)], 1, List, false, "Messages of a conversation, newest first. filter: limit, from, unread, contains."),
        spec("person", &[("id", Text)], 1, Person, false, "A person by id: id, name, emails, identities."),
        spec("people_of", &[("conversation", Conversation)], 1, List, false, "The people taking part in a conversation."),
        spec("search", &[("query", Text)], 1, List, false, "Conversations whose name, sender or recent text matches the query."),
        spec("reply", &[("conversation", Conversation), ("text", Text), ("options", Json)], 2, Message, true, "Reply in the conversation. options.channel picks the connection; options.message quotes that message the way its connection quotes."),
        spec("send", &[("target", Any), ("text", Text), ("channel", Text)], 2, Message, true, "Send to a conversation or a person. With a person, channel (mail, chat, or a connection id) picks the conversation."),
        spec("draft", &[("conversation", Conversation), ("text", Text)], 2, Json, true, "Keep a message as a draft without sending it. Returns the conversation id and the text."),
        spec("forward", &[("message", Message), ("target", Any), ("options", Json)], 2, Message, true, "Forward a message to a conversation or person, natively where the connection can, else re-posted with attribution and attachments. options.comment adds a comment."),
        spec("tag_add", &[("object", Any), ("tag", Tag), ("object_type", Text)], 2, Any, true, "Add a tag to a conversation, message, person or any other object, by tag name or id; a name no tag has yet becomes a new tag. object_type (conversation, message, attachment, person, merge_group and so on) is read from the object when left out."),
        spec("tag_remove", &[("object", Any), ("tag", Tag), ("object_type", Text)], 2, Any, true, "Remove a tag from a conversation, message, person or any other object, by tag name or id. object_type is read from the object when left out."),
        spec("move", &[("conversation", Conversation), ("split", Tag)], 2, Conversation, true, "Move the conversation to an inbox split."),
        spec("snooze", &[("conversation", Conversation), ("wake_at", Text)], 2, Conversation, true, "Snooze until a time (RFC 3339 or 'tomorrow 08:00')."),
        spec("done", &[("conversation", Conversation)], 1, Conversation, true, "Mark the conversation done."),
        spec("notify", &[("title", Text), ("body", Text)], 1, Bool, true, "Show a notification in Chatnip."),
        spec("post_status", &[("conversation", Conversation), ("options", Json)], 2, Json, true, "Write a status card into a chat. options.title, options.text and options.state (ok, warn, fail or info)."),
        spec("schedule_send", &[("conversation", Conversation), ("text", Text), ("at", Text)], 3, Json, true, "Send the text in the conversation at a later time."),
        spec("prompt", &[("key", Text), ("options", Json)], 1, Json, false, "Run one of your prompts (Actions, Prompts), or an AI prompt from Content by its name or id, and return its structured answer; a Content prompt answers #{ answer }. options.conversation and options.message give it context; other keys are passed to the template."),
        spec("content", &[("name", Text), ("options", Json)], 1, Text, false, "Fill in a reply, email or snippet from Content, by its name or id, and return the text. options.conversation and options.message fill in the person, the conversation and the message; other keys are passed to the template."),
        spec("has_split", &[("conversation", Conversation)], 1, Bool, false, "Whether the conversation already sits in an inbox split."),
        spec("splits", &[], 0, List, false, "The names of your inbox splits."),
        spec("has_time", &[("text", Text)], 1, Bool, false, "Whether the text mentions a time, a day or a meeting, cheaply and without AI."),
        spec("store_summary", &[("message", Message), ("text", Text)], 2, Text, true, "Keep a one line summary on the message; folded mail shows it."),
        spec("present", &[("message", Message), ("kind", Text), ("options", Json)], 2, Json, true, "Choose how a message shows: kind auto, chat, mail, invite, notification, bot, digest or hidden. options: collapsed, summary_only, highlight, pinned, badge, icon, invite."),
        spec("highlight", &[("message", Message), ("colour", Text)], 2, Json, true, "Give a message a highlight colour."),
        spec("collapse", &[("message", Message)], 1, Json, true, "Show a message folded."),
        spec("hide", &[("message", Message)], 1, Json, true, "Hide a message from the thread."),
        spec("present_conversation", &[("conversation", Conversation), ("defaults", Json)], 2, Json, true, "How every message in a conversation shows by default: kind and the same options as present()."),
        spec("style_conversation", &[("conversation", Conversation), ("appearance", Json)], 2, Json, true, "Change how a conversation looks: surface (solid or glass), background, accent, bubbleMine, bubbleTheirs, tint, theme."),
        spec("invite_details", &[("message", Message)], 1, Json, false, "Whether a message reads like a meeting invitation, without AI: confidence 0 to 1, title, starts_at, ends_at, location, organizer, join_url, provider."),
        spec("create_event", &[("event", Json), ("message", Message)], 1, Json, true, "Put an event in the calendar. event: title, starts_at, ends_at, all_day, location, attendees, notes, confidence. With a message it is linked and never added twice."),
        spec("propose_event", &[("event", Json), ("message", Message)], 1, Json, true, "Suggest an event for you to confirm in the calendar."),
        spec("mark_person", &[("person", Person), ("fields", Json)], 2, Person, true, "Change a person: fields.human true or false, fields.name."),
        spec("mute", &[("conversation", Conversation)], 1, Conversation, true, "Mute the conversation."),
        spec("pin", &[("conversation", Conversation)], 1, Conversation, true, "Pin the conversation."),
        spec("mark_read", &[("conversation", Conversation)], 1, Conversation, true, "Mark everything in the conversation read."),
        spec("group_add", &[("conversation", Conversation), ("group", Text)], 2, Conversation, true, "Add the conversation to one of your groups, by name or id."),
        spec("now", &[], 0, Text, false, "The current time, RFC 3339."),
        spec("parse_date", &[("text", Text)], 1, Text, false, "Parse a date or time into RFC 3339, or () when it cannot be read."),
        spec("regex_match", &[("text", Text), ("pattern", Text)], 2, Bool, false, "Whether the pattern matches the text, case insensitive."),
        spec("regex_find", &[("text", Text), ("pattern", Text)], 2, List, false, "All matches of the pattern; capture groups when the pattern has them."),
        spec("regex_replace", &[("text", Text), ("pattern", Text), ("replacement", Text)], 3, Text, false, "Replace every match of the pattern."),
        spec("test", &[("value", Any), ("operator", Text), ("wanted", Text)], 3, Bool, false, "Compare like a sorting rule: is, is_not, contains, not_contains, starts_with, ends_with, matches, greater_than, less_than."),
        spec("log", &[("value", Any)], 1, Bool, false, "Write a line to the run log."),
        spec("receipt_extract", &[("message", Message)], 1, Json, false, "Read a message as text for the receipt nodes: its body, and every attachment downloaded to the cache and turned into text (HTML, PDF, images through macOS text recognition, plain text), with what came out of each file."),
        spec("receipt_match", &[("material", Json), ("patterns", List), ("threshold", Number)], 1, List, false, "Score what receipt_extract read against the patterns (name, pattern, weight) and return the candidates that reach the threshold, each with its score and the patterns that hit."),
        spec("receipt_confirm", &[("candidates", List), ("batch", Number)], 1, List, false, "Ask a small local model through Ollama for a verdict on each candidate, a batch at a time; candidates pass on unconfirmed when no local model answers."),
        spec("receipt_organise", &[("candidates", List)], 1, List, false, "One record per receipt: vendor, date, total, currency, VAT, order number, card ending, source, conversation, message, attachment, excerpt and confidence."),
    ]
    })
}

pub struct Bridge {
    pub effects: Arc<dyn Effects>,
    pub handle: tokio::runtime::Handle,
    pub away: Arc<Mutex<Duration>>,
}

impl Bridge {
    fn call(&self, name: &str, args: &[Dynamic]) -> Result<Dynamic, Box<EvalAltResult>> {
        let values: Vec<Value> = args.iter().map(to_value).collect();
        let began = Instant::now();
        let answer = self.handle.block_on(self.effects.call(name, values));
        if let Ok(mut away) = self.away.lock() {
            *away += began.elapsed();
        }
        let result = answer.map_err(|e| Box::new(EvalAltResult::ErrorRuntime(Dynamic::from(e), Position::NONE)))?;
        to_dynamic(&result)
    }
}

pub fn to_value(value: &Dynamic) -> Value {
    rhai::serde::from_dynamic::<Value>(value).unwrap_or(Value::Null)
}

pub fn to_dynamic(value: &Value) -> Result<Dynamic, Box<EvalAltResult>> {
    rhai::serde::to_dynamic(value)
}

fn runtime(message: String) -> Box<EvalAltResult> {
    Box::new(EvalAltResult::ErrorRuntime(Dynamic::from(message), Position::NONE))
}

fn text_of(value: &Dynamic) -> String {
    if value.is_unit() {
        return String::new();
    }
    match value.clone().into_string() {
        Ok(s) => s,
        Err(_) => to_value(value).to_string(),
    }
}

pub fn parse_date(text: &str) -> Option<String> {
    let text = text.trim();
    if let Ok(at) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(at.with_timezone(&chrono::Utc).to_rfc3339());
    }
    let local = chrono::Local::now();
    let lower = text.to_lowercase();
    let (day, rest) = if let Some(rest) = lower.strip_prefix("tomorrow") {
        (local.date_naive() + chrono::Duration::days(1), rest.trim().to_string())
    } else if let Some(rest) = lower.strip_prefix("today") {
        (local.date_naive(), rest.trim().to_string())
    } else if let Ok(date) = chrono::NaiveDate::parse_from_str(lower.split_whitespace().next().unwrap_or(""), "%Y-%m-%d") {
        (date, lower.split_whitespace().skip(1).collect::<Vec<_>>().join(" "))
    } else {
        return None;
    };
    let time = if rest.is_empty() { chrono::NaiveTime::from_hms_opt(9, 0, 0)? } else { chrono::NaiveTime::parse_from_str(&rest, "%H:%M").ok()? };
    day.and_time(time).and_local_timezone(chrono::Local).single().map(|t| t.with_timezone(&chrono::Utc).to_rfc3339())
}

fn regex(pattern: &str) -> Result<regex::Regex, Box<EvalAltResult>> {
    regex::RegexBuilder::new(pattern).case_insensitive(true).size_limit(1 << 20).build().map_err(|e| runtime(format!("Bad pattern {}: {}", pattern, e)))
}

fn limited(engine: &mut Engine, started: Instant, away: Arc<Mutex<Duration>>) {
    engine.set_max_operations(MAX_OPERATIONS);
    engine.set_max_call_levels(32);
    engine.set_max_expr_depths(64, 32);
    engine.set_max_string_size(MAX_STRING);
    engine.set_max_array_size(MAX_COLLECTION);
    engine.set_max_map_size(MAX_COLLECTION);
    engine.disable_symbol("eval");
    engine.on_progress(move |_| {
        let waited = away.lock().map(|a| *a).unwrap_or_default();
        if started.elapsed().saturating_sub(waited) > Duration::from_secs(MAX_SECONDS) {
            Some(Dynamic::from("The script ran out of time"))
        } else {
            None
        }
    });
}

fn register_pure(engine: &mut Engine, logs: Arc<Mutex<Vec<String>>>) {
    engine.register_fn("now", || chrono::Utc::now().to_rfc3339());
    engine.register_fn("has_time", |text: &str| crate::services::calendar_service::looks_like_it_has_a_time(text));
    engine.register_fn("parse_date", |text: &str| -> Dynamic { parse_date(text).map(Dynamic::from).unwrap_or(Dynamic::UNIT) });
    engine.register_fn("regex_match", |text: &str, pattern: &str| -> Result<bool, Box<EvalAltResult>> { Ok(regex(pattern)?.is_match(text)) });
    engine.register_fn("regex_find", |text: &str, pattern: &str| -> Result<rhai::Array, Box<EvalAltResult>> {
        let re = regex(pattern)?;
        Ok(re
            .captures_iter(text)
            .map(|c| {
                if c.len() > 1 {
                    Dynamic::from(c.iter().skip(1).map(|g| Dynamic::from(g.map(|m| m.as_str().to_string()).unwrap_or_default())).collect::<rhai::Array>())
                } else {
                    Dynamic::from(c.get(0).map(|m| m.as_str().to_string()).unwrap_or_default())
                }
            })
            .collect())
    });
    engine.register_fn("test", |value: Dynamic, operator: &str, wanted: &str| -> Result<bool, Box<EvalAltResult>> {
        let operator: crate::models::rule::Operator = serde_json::from_value(Value::String(operator.to_string())).map_err(|_| runtime(format!("Unknown operator {}", operator)))?;
        Ok(operator.test(&text_of(&value), wanted))
    });
    engine.register_fn("regex_replace", |text: &str, pattern: &str, with: &str| -> Result<String, Box<EvalAltResult>> { Ok(regex(pattern)?.replace_all(text, with).to_string()) });
    let sink = logs.clone();
    engine.register_fn("log", move |value: Dynamic| {
        sink.lock().map(|mut l| l.push(text_of(&value))).ok();
        true
    });
    engine.on_print(move |line| {
        logs.lock().map(|mut l| l.push(line.to_string())).ok();
    });
}

fn register_bridge(engine: &mut Engine, bridge: Arc<Bridge>) {
    let pure = ["now", "parse_date", "regex_match", "regex_find", "regex_replace", "log", "test", "has_time"];
    for function in library().iter().filter(|f| !pure.contains(&f.name)) {
        let name = function.name;
        for arity in function.required..=function.params.len() {
            let b = bridge.clone();
            match arity {
                0 => {
                    engine.register_fn(name, move || b.call(name, &[]));
                }
                1 => {
                    engine.register_fn(name, move |a: Dynamic| b.call(name, &[a]));
                }
                2 => {
                    engine.register_fn(name, move |a: Dynamic, c: Dynamic| b.call(name, &[a, c]));
                }
                _ => {
                    engine.register_fn(name, move |a: Dynamic, c: Dynamic, d: Dynamic| b.call(name, &[a, c, d]));
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Diagnostic {
    pub severity: String,
    pub message: String,
    pub node: Option<String>,
    pub edge: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub setting: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Diagnostic {
        Diagnostic { severity: "error".into(), message: message.into(), node: None, edge: None, line: None, column: None, setting: None }
    }

    pub fn warning(message: impl Into<String>) -> Diagnostic {
        Diagnostic { severity: "warning".into(), ..Diagnostic::error(message) }
    }

    pub fn requirement(requirement: &Requirement) -> Diagnostic {
        Diagnostic { severity: REQUIREMENT.into(), setting: Some(requirement.key.to_string()), ..Diagnostic::error(requirement.label) }
    }

    pub fn at_node(mut self, node: &str) -> Diagnostic {
        self.node = Some(node.to_string());
        self
    }

    pub fn at_edge(mut self, edge: &str) -> Diagnostic {
        self.edge = Some(edge.to_string());
        self
    }
}

fn checking_engine() -> Engine {
    let mut engine = Engine::new();
    limited(&mut engine, Instant::now(), Arc::new(Mutex::new(Duration::ZERO)));
    register_pure(&mut engine, Arc::new(Mutex::new(Vec::new())));
    engine
}

pub fn check_script(body: &str) -> Vec<Diagnostic> {
    let engine = checking_engine();
    let ast = match engine.compile(body) {
        Ok(ast) => ast,
        Err(err) => {
            let position = err.position();
            return vec![Diagnostic { severity: "error".into(), message: err.err_type().to_string(), node: None, edge: None, line: position.line(), column: position.position(), setting: None }];
        }
    };
    let mut known: HashSet<String> = engine.gen_fn_signatures(true).into_iter().map(|signature| signature.split('(').next().unwrap_or("").trim().to_string()).collect();
    known.extend(library().iter().map(|f| f.name.to_string()));
    known.extend(ast.iter_functions().map(|f| f.name.to_string()));
    let mut out = Vec::new();
    ast.walk(&mut |path| {
        if let Some(ASTNode::Expr(Expr::FnCall(call, position) | Expr::MethodCall(call, position))) = path.last() {
            if call.op_token.is_none() && call.namespace.is_empty() && !known.contains(call.name.as_str()) {
                out.push(Diagnostic { severity: "error".into(), message: format!("{} is not a function actions support", call.name), node: None, edge: None, line: position.line(), column: position.position(), setting: None });
            }
        }
        true
    });
    out
}

#[derive(Debug, Clone)]
pub struct ScriptOutcome {
    pub value: Value,
    pub logs: Vec<String>,
}

pub async fn run_script(body: &str, inputs: &Map<String, Value>, effects: Arc<dyn Effects>) -> Result<ScriptOutcome, String> {
    let body = body.to_string();
    let inputs = inputs.clone();
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        let logs = Arc::new(Mutex::new(Vec::new()));
        let mut engine = Engine::new();
        let away = Arc::new(Mutex::new(Duration::ZERO));
        limited(&mut engine, Instant::now(), away.clone());
        register_pure(&mut engine, logs.clone());
        register_bridge(&mut engine, Arc::new(Bridge { effects, handle, away }));
        let ast = engine.compile(&body).map_err(|e| format!("The script does not compile: {}", e))?;
        let mut scope = Scope::new();
        for (name, value) in &inputs {
            scope.push_dynamic(name.clone(), to_dynamic(value).map_err(|e| e.to_string())?);
        }
        let result: Dynamic = engine.eval_ast_with_scope(&mut scope, &ast).map_err(|e| e.to_string())?;
        let logs = logs.lock().map(|l| l.clone()).unwrap_or_default();
        Ok(ScriptOutcome { value: to_value(&result), logs })
    })
    .await
    .map_err(|e| format!("The script stopped: {}", e))?
}

pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

#[derive(Debug, Clone, Default, Serialize, serde::Deserialize, PartialEq)]
pub struct ActionDraft {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub max_sends_per_hour: Option<i64>,
    #[serde(default)]
    pub inputs: Vec<Port>,
    #[serde(default)]
    pub graph: Graph,
}

fn literal(value: &Value) -> String {
    match value {
        Value::Null => "()".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into()),
        Value::Array(items) => format!("[{}]", items.iter().map(literal).collect::<Vec<_>>().join(", ")),
        Value::Object(map) => {
            if map.is_empty() {
                return "#{}".into();
            }
            format!("#{{ {} }}", map.iter().map(|(k, v)| format!("{}: {}", serde_json::to_string(k).unwrap_or_default(), literal(v))).collect::<Vec<_>>().join(", "))
        }
    }
}

fn raw(body: &str) -> String {
    let mut hashes = 1;
    while body.contains(&format!("\"{}", "#".repeat(hashes))) {
        hashes += 1;
    }
    let fence = "#".repeat(hashes);
    let body = body.trim_matches('\n');
    format!("{fence}\"\n{body}\n\"{fence}")
}

fn float(value: f64) -> String {
    if value.fract() == 0.0 { format!("{:.1}", value) } else { value.to_string() }
}

pub fn render(draft: &ActionDraft) -> String {
    let mut head = Map::new();
    head.insert("name".into(), Value::String(draft.name.clone()));
    if !draft.description.is_empty() {
        head.insert("description".into(), Value::String(draft.description.clone()));
    }
    if let Some(icon) = &draft.icon {
        head.insert("icon".into(), Value::String(icon.clone()));
    }
    if let Some(max) = draft.max_sends_per_hour {
        head.insert("max_sends_per_hour".into(), json!(max));
    }
    let mut lines = vec![format!("action({});", literal(&Value::Object(head))), String::new()];
    for port in &draft.inputs {
        if port.description.is_empty() {
            lines.push(format!("input({}, {});", literal(&json!(port.name)), literal(&json!(port.kind.name()))));
        } else {
            lines.push(format!("input({}, {}, {});", literal(&json!(port.name)), literal(&json!(port.kind.name())), literal(&json!(port.description))));
        }
    }
    if !draft.inputs.is_empty() {
        lines.push(String::new());
    }
    for node in &draft.graph.nodes {
        let config = if node.config.is_null() { String::from("#{}") } else { literal(&node.config) };
        let body = if node.kind.starts_with(crate::models::action::BUILTIN_PREFIX) || node.body.trim().is_empty() { "\"\"".to_string() } else { raw(&node.body) };
        lines.push(format!(
            "node({}, {}, #{{ \"label\": {}, \"x\": {}, \"y\": {}, \"config\": {} }}, {});",
            literal(&json!(node.id)),
            literal(&json!(node.kind)),
            literal(&json!(node.label)),
            float(node.x),
            float(node.y),
            config,
            body
        ));
    }
    if !draft.graph.edges.is_empty() {
        lines.push(String::new());
    }
    for edge in &draft.graph.edges {
        lines.push(format!("connect({}, {}, {}, {});", literal(&json!(edge.source)), literal(&json!(edge.source_port)), literal(&json!(edge.target)), literal(&json!(edge.target_port))));
    }
    lines.join("\n") + "\n"
}

fn port_of(name: &str, kind: &str, description: &str) -> Result<Port, Box<EvalAltResult>> {
    let kind: PortType = serde_json::from_value(Value::String(kind.to_string())).map_err(|_| runtime(format!("Unknown type {}; use one of {}", kind, PortType::ALL.map(|t| t.name()).join(", "))))?;
    Ok(Port { name: name.to_string(), kind, description: description.to_string(), node: None, source_port: None })
}

pub fn parse(code: &str) -> Result<ActionDraft, Vec<Diagnostic>> {
    let draft = Arc::new(Mutex::new(ActionDraft::default()));
    let mut engine = Engine::new_raw();
    limited(&mut engine, Instant::now(), Arc::new(Mutex::new(Duration::ZERO)));
    let d = draft.clone();
    engine.register_fn("action", move |meta: rhai::Map| -> Result<(), Box<EvalAltResult>> {
        let meta = to_value(&Dynamic::from(meta));
        let mut draft = d.lock().map_err(|_| runtime("locked".into()))?;
        draft.name = meta["name"].as_str().unwrap_or_default().to_string();
        draft.description = meta["description"].as_str().unwrap_or_default().to_string();
        draft.icon = meta["icon"].as_str().map(String::from);
        draft.max_sends_per_hour = meta["max_sends_per_hour"].as_i64();
        Ok(())
    });
    let d2 = draft.clone();
    engine.register_fn("input", move |name: &str, kind: &str| -> Result<(), Box<EvalAltResult>> {
        let port = port_of(name, kind, "")?;
        d2.lock().map_err(|_| runtime("locked".into()))?.inputs.push(port);
        Ok(())
    });
    let d3 = draft.clone();
    engine.register_fn("input", move |name: &str, kind: &str, description: &str| -> Result<(), Box<EvalAltResult>> {
        let port = port_of(name, kind, description)?;
        d3.lock().map_err(|_| runtime("locked".into()))?.inputs.push(port);
        Ok(())
    });
    let d4 = draft.clone();
    engine.register_fn("node", move |id: &str, kind: &str, meta: rhai::Map, body: &str| -> Result<(), Box<EvalAltResult>> {
        let meta = to_value(&Dynamic::from(meta));
        let mut draft = d4.lock().map_err(|_| runtime("locked".into()))?;
        if draft.graph.nodes.iter().any(|n| n.id == id) {
            return Err(runtime(format!("Node {} is declared twice", id)));
        }
        draft.graph.nodes.push(Node {
            id: id.to_string(),
            kind: kind.to_string(),
            label: meta["label"].as_str().unwrap_or_default().to_string(),
            config: meta.get("config").cloned().unwrap_or(Value::Null),
            body: body.trim_matches('\n').to_string(),
            x: meta["x"].as_f64().unwrap_or(0.0),
            y: meta["y"].as_f64().unwrap_or(0.0),
        });
        Ok(())
    });
    let d5 = draft.clone();
    engine.register_fn("connect", move |source: &str, source_port: &str, target: &str, target_port: &str| -> Result<(), Box<EvalAltResult>> {
        let mut draft = d5.lock().map_err(|_| runtime("locked".into()))?;
        let id = format!("{}.{}-{}.{}", source, source_port, target, target_port);
        draft.graph.edges.push(Edge { id, source: source.into(), source_port: source_port.into(), target: target.into(), target_port: target_port.into() });
        Ok(())
    });
    let ast = engine.compile(code).map_err(|err| {
        let p = err.position();
        vec![Diagnostic { severity: "error".into(), message: err.err_type().to_string(), node: None, edge: None, line: p.line(), column: p.position(), setting: None }]
    })?;
    engine.run_ast(&ast).map_err(|err| {
        let p = err.position();
        vec![Diagnostic { severity: "error".into(), message: err.to_string(), node: None, edge: None, line: p.line(), column: p.position(), setting: None }]
    })?;
    let draft = draft.lock().map(|d| d.clone()).map_err(|_| vec![Diagnostic::error("The action could not be read")])?;
    if draft.name.trim().is_empty() {
        return Err(vec![Diagnostic::error("The action needs action(#{ name: \"…\" }) with a name")]);
    }
    Ok(draft)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Recorder(Mutex<Vec<(String, Vec<Value>)>>);

    #[async_trait]
    impl Effects for Recorder {
        async fn call(&self, function: &str, args: Vec<Value>) -> Result<Value, String> {
            self.0.lock().unwrap().push((function.to_string(), args.clone()));
            Ok(match function {
                "classify" => json!("Important"),
                "get_message" => json!({ "id": args[0], "text": "Invoice 42 is due" }),
                _ => json!({ "ok": true }),
            })
        }
    }

    fn inputs(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn scripts_see_inputs_and_call_the_library() {
        let recorder = Arc::new(Recorder(Mutex::new(Vec::new())));
        let outcome = run_script(
            r#"let m = get_message(id); let found = regex_find(m.text, "invoice (\\d+)"); log(found[0][0]); #{ label: classify(m.text, ["Important", "Other"]), number: found[0][0] }"#,
            &inputs(&[("id", json!("m1"))]),
            recorder.clone(),
        )
        .await
        .unwrap();
        assert_eq!(outcome.value, json!({ "label": "Important", "number": "42" }));
        assert_eq!(outcome.logs, vec!["42".to_string()]);
        let calls = recorder.0.lock().unwrap();
        assert_eq!(calls.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), vec!["get_message", "classify"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_sandbox_stops_runaway_scripts() {
        let recorder = Arc::new(Recorder(Mutex::new(Vec::new())));
        let looped = run_script("let x = 0; loop { x += 1; }", &Map::new(), recorder.clone()).await.unwrap_err();
        assert!(looped.to_lowercase().contains("operations"), "{}", looped);
        let big = run_script("let s = \"x\"; loop { s += s; }", &Map::new(), recorder.clone()).await.unwrap_err();
        assert!(big.to_lowercase().contains("string") || big.to_lowercase().contains("length"), "{}", big);
        let eval = run_script("eval(\"1 + 1\")", &Map::new(), recorder).await.unwrap_err();
        assert!(eval.to_lowercase().contains("eval"), "{}", eval);
    }

    #[test]
    fn compile_errors_carry_a_position() {
        let diagnostics = check_script("let a = 1;\nlet b = ;");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, Some(2));
        assert!(check_script("let a = regex_match(\"x\", \"x\");").is_empty());
        let unknown = check_script("let a = regex_matchh(\"x\", \"x\");\nlet b = \"x\".len();");
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].line, Some(1));
        assert!(unknown[0].message.contains("regex_matchh"));
        assert!(check_script("fn mine(x) { x }\nmine(1)").is_empty());
    }

    #[test]
    fn the_code_form_round_trips() {
        let draft = ActionDraft {
            name: "Acknowledge invoices".into(),
            description: "Says thanks \"quickly\"".into(),
            icon: Some("lucide:receipt".into()),
            max_sends_per_hour: Some(3),
            inputs: vec![Port { name: "tone".into(), kind: PortType::Text, description: "how it should sound".into(), node: None, source_port: None }],
            graph: Graph {
                nodes: vec![
                    Node { id: "t".into(), kind: "trigger".into(), label: "Message arrives".into(), config: json!({ "on": "arrival", "scope": { "providers": ["mock"] } }), body: "regex_match(text, \"invoice\")".into(), x: 0.0, y: 0.0 },
                    Node { id: "s".into(), kind: "script".into(), label: "Draft".into(), config: json!({ "inputs": [{ "name": "text", "type": "text" }], "outputs": [{ "name": "reply", "type": "text" }] }), body: "let quote = \"#\\\"\";\n#{ reply: `Thanks! ${text}` }".into(), x: 0.0, y: 160.5 },
                    Node { id: "r".into(), kind: "builtin:reply".into(), label: String::new(), config: json!({ "values": { "text": null } }), body: String::new(), x: 0.0, y: 320.0 },
                ],
                edges: vec![
                    Edge { id: "t.text-s.text".into(), source: "t".into(), source_port: "text".into(), target: "s".into(), target_port: "text".into() },
                    Edge { id: "s.reply-r.text".into(), source: "s".into(), source_port: "reply".into(), target: "r".into(), target_port: "text".into() },
                ],
            },
        };
        let code = render(&draft);
        let parsed = parse(&code).unwrap_or_else(|d| panic!("{:?}\n{}", d, code));
        assert_eq!(parsed, draft);
        assert_eq!(render(&parsed), code);
    }

    #[test]
    fn bad_code_reports_where() {
        let err = parse("action(#{ name: \"x\" });\ninput(\"a\", \"colour\");").unwrap_err();
        assert!(err[0].message.contains("Unknown type colour"));
        assert_eq!(err[0].line, Some(2));
        assert!(parse("node(\"a\", \"script\", #{}, \"\");").is_err());
    }

    #[test]
    fn dates_parse_in_plain_words() {
        assert!(parse_date("2026-10-14T08:00:00+02:00").unwrap().starts_with("2026-10-14T06:00:00"));
        assert!(parse_date("tomorrow 08:00").is_some());
        assert!(parse_date("someday").is_none());
    }

    #[test]
    fn calls_are_found_outside_strings_comments_and_methods() {
        let body = "// send(conversation, \"x\")\nlet answer = ai(\"reply(nothing)\");\nlet m = answer.done();\nfn move(x) { x }\n/* forward(a, b) */\nreply (conversation, answer)";
        assert_eq!(calls(body), vec!["ai", "reply"]);
        assert!(calls("let text = \"prompt(\\\"sort\\\")\";").is_empty());
        assert_eq!(calls("prompt(\"sort\", #{});\nsummarize(text)"), vec!["summarize", "prompt"]);
    }

    #[test]
    fn every_function_requirement_is_declared_once() {
        let declared: Vec<&str> = requirements().iter().map(|r| r.key).collect();
        let unique: std::collections::HashSet<&str> = declared.iter().copied().collect();
        assert_eq!(unique.len(), declared.len());
        assert!(requirements().iter().all(|r| !r.label.trim().is_empty() && r.key.contains('.')));
        for function in library() {
            assert!(function.requires.iter().all(|key| requirement(key).is_some()), "{} needs an undeclared setting", function.name);
        }
        let ai = library().iter().find(|f| f.name == "ai").unwrap();
        assert_eq!(ai.requires, vec![REQUIRES_AI_ENABLED, REQUIRES_AI_CONNECTION, REQUIRES_AI_MODEL]);
        assert_eq!(function_requirements("send"), &[REQUIRES_ACCOUNT]);
        assert!(function_requirements("regex_match").is_empty());
        assert_eq!(Diagnostic::requirement(&requirement(REQUIRES_ACCOUNT).unwrap()).setting.as_deref(), Some(REQUIRES_ACCOUNT));
    }
}
