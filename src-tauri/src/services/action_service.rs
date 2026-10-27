use crate::models::action::{
    Action, ActionGroup, ActionRun, AiChoice, Edge, Graph, LogEntry, MessageSchedule, Node, Port, PortType, PostActionInput, PostMessageScheduleInput, PutActionInput,
    TriggerConfig, TriggerOn, TriggerSchedule, TriggerWatch, Usage, DEFAULT_WATCH_MINUTES, WATCH_POLL, WATCH_WEBHOOK, ACTION_PREFIX, BUILTIN_PREFIX, BUILTIN_GROUP, BUILTIN_KEY_PREFIX, DEFAULT_MINUTES_BEFORE, DIRECTION_ANY, DIRECTION_OUTGOING, STARTER_GROUP, NODE_CHAT, NODE_SCRIPT, NODE_TRIGGER, SENT_PREFIX, STATUS_FAILED, STATUS_SENT,
};
use crate::models::chat::{self as chat_model, ChatMode};
use crate::services::receipt_service;
use crate::models::app_setting::{self, AppSetting};
use crate::models::group::{Group, QueryFacts};
use crate::models::tag::{tag_named, ObjectTag, Tag, KIND_SPLIT, OBJECT_CONVERSATION, OBJECT_MESSAGE, OBJECT_PERSON};
use crate::models::rule::{Condition, Field, Match};
use crate::models::{Profile, SendMessageInput};
use crate::services::action_script::{self, ActionDraft, Diagnostic, Effects};
use crate::services::intelligence_service;
use crate::models::content::{is_template, ContentItem, KIND_PROMPT};
use crate::services::content_service::{self, PostContentRenderInput};
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, Utc};
use serde::Serialize;
use crate::utils::text;
use serde_json::{json, Map, Value};
use sqlx::{Pool, Row, Sqlite};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

pub const MAX_DEPTH: i64 = 8;
pub const SCHEDULER_TICK_SECONDS: u64 = 30;

#[derive(Debug, Clone, Serialize)]
pub struct Builtin {
    pub key: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub description: &'static str,
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
    pub body: &'static str,
    pub kind: &'static str,
}

fn ports(list: &[(&str, PortType)]) -> Vec<Port> {
    list.iter().map(|(n, t)| Port::new(n, *t)).collect()
}

pub fn builtins() -> &'static [Builtin] {
    static BUILTINS: std::sync::OnceLock<Vec<Builtin>> = std::sync::OnceLock::new();
    BUILTINS.get_or_init(|| {
    use PortType::*;
    let b = |key, label, icon, description, inputs: &[(&str, PortType)], outputs: &[(&str, PortType)], body| Builtin { key, label, icon, description, inputs: ports(inputs), outputs: ports(outputs), body, kind: builtin_place(key).1 };
    vec![
        b("tag_add", "Add tag", "lucide:tag", "Adds a tag to the conversation, or to the object wired in: a message, an attachment, a person or anything else that takes tags. Picks the tag by name or id, and a name no tag has yet becomes a new tag.", &[("conversation", Conversation), ("tag", Tag), ("object", Any), ("object_type", Text)], &[], "#{ conversation: tag_add(if object == () { conversation } else { object }, tag, if object_type == () { \"\" } else { object_type }) }"),
        b("tag_remove", "Remove tag", "lucide:tag", "Removes a tag from the conversation, or from the object wired in. Picks the tag by name or id.", &[("conversation", Conversation), ("tag", Tag), ("object", Any), ("object_type", Text)], &[], "#{ conversation: tag_remove(if object == () { conversation } else { object }, tag, if object_type == () { \"\" } else { object_type }) }"),
        b("move", "Move to split", "lucide:inbox", "Moves the conversation to an inbox split.", &[("conversation", Conversation), ("split", Tag)], &[], "#{ conversation: move(conversation, split) }"),
        b("snooze", "Snooze", "lucide:clock", "Snoozes the conversation until a time.", &[("conversation", Conversation), ("wake_at", Text)], &[], "#{ conversation: snooze(conversation, wake_at) }"),
        b("done", "Mark done", "lucide:check", "Marks the conversation done.", &[("conversation", Conversation)], &[], "#{ conversation: done(conversation) }"),
        b("reply", "Reply", "lucide:reply", "Replies in the conversation, quoting the message when one is given.", &[("conversation", Conversation), ("text", Text), ("quote", Message)], &[("message", Message)], "#{ message: reply(conversation, text, #{ message: quote }) }"),
        b("send", "Send", "lucide:send", "Sends to a conversation or a person.", &[("target", Any), ("text", Text), ("channel", Text)], &[("message", Message)], "#{ message: if channel == () { send(target, text) } else { send(target, text, channel) } }"),
        b("draft", "Draft", "lucide:file-pen", "Keeps the message as a draft without sending it.", &[("conversation", Conversation), ("text", Text)], &[], "#{ draft: draft(conversation, text) }"),
        b("forward", "Forward", "lucide:forward", "Forwards a message natively where the connection can, else re-posts it with attribution.", &[("message", Message), ("target", Any), ("comment", Text)], &[("message", Message)], "#{ message: forward(message, target, #{ comment: comment }) }"),
        b("schedule_send", "Send later", "lucide:calendar-clock", "Sends the text in the conversation at a later time.", &[("conversation", Conversation), ("text", Text), ("at", Text)], &[], "#{ schedule: schedule_send(conversation, text, at) }"),
        b("post_status", "Post status", "lucide:activity", "Writes a status card into a chat, the way a build, deploy or ticket bot would: a title, a line of text and a state of ok, warn, fail or info.", &[("conversation", Conversation), ("title", Text), ("text", Text), ("state", Text)], &[], "#{ status: post_status(conversation, #{ title: title, text: if text == () { \"\" } else { text }, state: if state == () { \"info\" } else { state } }) }"),
        b("notify", "Notify me", "lucide:bell", "Shows a notification in Chatnip.", &[("title", Text), ("body", Text)], &[], "#{ sent: notify(title, if body == () { \"\" } else { body }) }"),
        b("ai", "Ask AI", "lucide:sparkles", "Asks the chosen AI and returns its answer.", &[("prompt", Text), ("material", Any)], &[("answer", Text)], "#{ answer: ai(prompt, #{ material: material }) }"),
        b("ai_reply", "AI reply draft", "lucide:sparkles", "Writes a reply to the conversation in your tone.", &[("conversation", Conversation), ("instruction", Text)], &[("text", Text)], "#{ text: ai(if instruction == () { \"Write a short, friendly reply.\" } else { instruction }, #{ conversation: conversation }) }"),
        b("classify", "Classify", "lucide:shapes", "Picks the label from a list that fits the text.", &[("text", Text), ("labels", List)], &[("label", Tag)], "#{ label: classify(text, labels) }"),
        b("summarize", "Summarize", "lucide:align-left", "Sums up the text in a sentence or two.", &[("text", Text)], &[("summary", Text)], "#{ summary: summarize(text) }"),
        b("mute", "Mute", "lucide:bell-off", "Mutes the conversation.", &[("conversation", Conversation)], &[], "#{ conversation: mute(conversation) }"),
        b("pin", "Pin", "lucide:pin", "Pins the conversation.", &[("conversation", Conversation)], &[], "#{ conversation: pin(conversation) }"),
        b("mark_read", "Mark read", "lucide:mail-open", "Marks everything in the conversation read.", &[("conversation", Conversation)], &[], "#{ conversation: mark_read(conversation) }"),
        b("group_add", "Add to group", "lucide:folder-plus", "Adds the conversation to one of your groups.", &[("conversation", Conversation), ("group", Text)], &[], "#{ conversation: group_add(conversation, group) }"),
        b("present_chat", "Show as chat", "lucide:message-circle", "Ends with the message shown as a chat bubble, through this node's template when it has one.", &[("message", Message), ("options", Json)], &[], "#{ message: present(message, \"chat\", if options == () { #{} } else { options }) }"),
        b("present_mail", "Show as mail", "lucide:mail", "Ends with the message shown as a mail, through this node's template when it has one.", &[("message", Message), ("options", Json)], &[], "#{ message: present(message, \"mail\", if options == () { #{} } else { options }) }"),
        b("present_invite", "Show as invite", "lucide:calendar-check", "Ends with the message shown as an invite card, through this node's template when it has one.", &[("message", Message), ("options", Json)], &[], "#{ message: present(message, \"invite\", if options == () { #{} } else { options }) }"),
        b("present_notification", "Show as notification", "lucide:bell-ring", "Ends with the message shown as a quiet notification line, through this node's template when it has one.", &[("message", Message), ("options", Json)], &[], "#{ message: present(message, \"notification\", if options == () { #{} } else { options }) }"),
        b("present_bot", "Show as bot", "lucide:bot", "Ends with the message shown as an automated message, through this node's template when it has one.", &[("message", Message), ("options", Json)], &[], "#{ message: present(message, \"bot\", if options == () { #{} } else { options }) }"),
        b("present_digest", "Show as digest", "lucide:newspaper", "Ends with the message shown as a digest with its summary, through this node's template when it has one.", &[("message", Message), ("options", Json)], &[], "#{ message: present(message, \"digest\", if options == () { #{} } else { options }) }"),
        b("present_system", "Show as system line", "lucide:info", "Ends with the message shown as a centred system line, through this node's template when it has one.", &[("message", Message), ("options", Json)], &[], "#{ message: present(message, \"system\", if options == () { #{} } else { options }) }"),
        b("present_media", "Show as media", "lucide:image", "Ends with the message shown as its image, GIF or video, through this node's template when it has one.", &[("message", Message), ("options", Json)], &[], "#{ message: present(message, \"media\", if options == () { #{} } else { options }) }"),
        b("wait", "Wait", "lucide:hourglass", "Pauses the action, then passes the value on.", &[("minutes", Number), ("value", Any)], &[("value", Any)], ""),
        b("receipt_extract", "Extract", "lucide:file-search", "Opens the message body and its attachments and turns them into readable text: HTML mail to text, PDF text, images through macOS text recognition, plain text as is. Records what came out of each file.", &[("message", Message)], &[("material", Json)], "#{ material: receipt_extract(message) }"),
        b("receipt_match", "Match receipts", "lucide:regex", "Runs the patterns over the subject, the body and the text of each file, and keeps what scores as a receipt, with the patterns that hit.", &[("material", Json), ("patterns", List), ("threshold", Number)], &[("candidates", List)], "#{ candidates: receipt_match(material, patterns, threshold) }"),
        b("receipt_confirm", "Confirm with a local model", "lucide:sparkles", "Asks a small local model through Ollama whether each candidate is a receipt, a batch at a time. Without a local model the candidates pass on unconfirmed and the run log says so.", &[("candidates", List), ("batch", Number)], &[("candidates", List)], "#{ candidates: receipt_confirm(candidates, batch) }"),
        b("receipt_organise", "Organise receipts", "lucide:receipt", "Turns each receipt into one record: vendor, date, total, currency, VAT, order number, card ending, where it came from, the file and an excerpt.", &[("candidates", List)], &[("records", List)], "#{ records: receipt_organise(candidates) }"),
        b("ai_json", "Ask AI for fields", "lucide:braces", "Asks the chosen AI for an answer shaped by a JSON schema and returns the fields.", &[("prompt", Text), ("schema", Json)], &[("answer", Json)], "#{ answer: ai_json(prompt, if schema == () { #{ type: \"object\" } } else { schema }) }"),
        b("prompt", "Run a prompt", "lucide:message-square-text", "Runs one of your prompts, or an AI prompt from Content, over the material and returns its structured answer.", &[("key", Text), ("material", Any)], &[("answer", Json)], "#{ answer: prompt(key, #{ material: material }) }"),
        b("content", "Fill from Content", "lucide:library", "Fills in a reply, mail or snippet template from Content for the conversation and the message.", &[("name", Text), ("conversation", Conversation), ("message", Message)], &[("text", Text)], "#{ text: content(name, #{ conversation: conversation, message: message }) }"),
        b("regex_match", "Pattern test", "lucide:regex", "Whether a regular expression matches the text, case insensitive.", &[("text", Text), ("pattern", Text)], &[("matched", Bool)], "#{ matched: regex_match(text, pattern) }"),
        b("regex_find", "Find with a pattern", "lucide:text-search", "Every match of a regular expression in the text, with its capture groups.", &[("text", Text), ("pattern", Text)], &[("matches", List)], "#{ matches: regex_find(text, pattern) }"),
        b("regex_replace", "Replace with a pattern", "lucide:replace", "Replaces every match of a regular expression in the text.", &[("text", Text), ("pattern", Text), ("replacement", Text)], &[("text", Text)], "#{ text: regex_replace(text, pattern, if replacement == () { \"\" } else { replacement }) }"),
        b("has_time", "Mentions a time", "lucide:clock-alert", "Whether the text mentions a time, a day or a meeting, cheaply and without AI.", &[("text", Text)], &[("mentions", Bool)], "#{ mentions: has_time(text) }"),
        b("parse_date", "Read a date", "lucide:calendar-days", "Reads a date or time out of the text as RFC 3339, or nothing when none can be read.", &[("text", Text)], &[("at", Text)], "#{ at: parse_date(text) }"),
        b("invite_details", "Read invite details", "lucide:calendar-search", "Whether a message reads like an invitation, without AI: confidence, title, start, end, location, organizer and join link.", &[("message", Message)], &[("invite", Json)], "#{ invite: invite_details(message) }"),
        b("find_events", "Find appointments", "lucide:calendar-search", "Asks your AI for the meetings, calls and deadlines a message mentions, each with a confidence.", &[("message", Message)], &[("events", List)], "#{ events: prompt(\"parse_events\", #{ message: message }).events }"),
        b("sender_kind", "Human or service", "lucide:user-search", "Asks your AI whether the sender is a person or an automated service.", &[("message", Message), ("conversation", Conversation)], &[("human", Bool)], "let provider = if conversation == () { \"\" } else { conversation.provider };\nlet answer = prompt(\"classify_sender\", #{ sender: #{ name: message.sender, email: message.sender_email, provider_type: provider, sample: message.text } });\n#{ human: answer.kind == \"human\" }"),
        b("get_conversation", "Get conversation", "lucide:messages-square", "A conversation by id: name, kind, kind of connection, tags and person.", &[("id", Text)], &[("conversation", Conversation)], "#{ conversation: get_conversation(id) }"),
        b("list_messages", "Recent messages", "lucide:list", "The newest messages of a conversation.", &[("conversation", Conversation), ("limit", Number)], &[("messages", List)], "#{ messages: list_messages(conversation, #{ limit: if limit == () { 20 } else { limit } }) }"),
        b("people_of", "People in the chat", "lucide:users", "The people taking part in a conversation.", &[("conversation", Conversation)], &[("people", List)], "#{ people: people_of(conversation) }"),
        b("create_event", "Add to calendar", "lucide:calendar-plus", "Puts the event in your calendar, linked to the message and never added twice.", &[("event", Json), ("message", Message)], &[], "#{ event: create_event(event, message) }"),
        b("propose_event", "Propose an event", "lucide:calendar-clock", "Suggests the event in your calendar for you to confirm.", &[("event", Json), ("message", Message)], &[], "#{ event: propose_event(event, message) }"),
        b("store_summary", "Keep summary", "lucide:align-left", "Keeps a one line summary on the message, which folded mail shows.", &[("message", Message), ("text", Text)], &[], "#{ summary: store_summary(message, text) }"),
        b("mark_person", "Mark person", "lucide:user-check", "Marks the person as a human or as an automated service.", &[("person", Person), ("human", Bool)], &[], "#{ person: mark_person(person, #{ human: human == true }) }"),
        b("highlight", "Highlight", "lucide:highlighter", "Gives the message a highlight colour.", &[("message", Message), ("colour", Text)], &[], "highlight(message, if colour == () { \"accent\" } else { colour });\n#{ message: message }"),
        b("collapse", "Fold", "lucide:fold-vertical", "Shows the message folded.", &[("message", Message)], &[], "collapse(message);\n#{ message: message }"),
        b("hide", "Hide", "lucide:eye-off", "Hides the message from the thread.", &[("message", Message)], &[], "hide(message);\n#{ message: message }"),
        b("present_conversation", "Show the chat as", "lucide:panels-top-left", "How every message in the conversation shows by default: kind, folded, summary only, highlight and badge.", &[("conversation", Conversation), ("defaults", Json)], &[], "present_conversation(conversation, if defaults == () { #{} } else { defaults });\n#{ conversation: conversation }"),
        b("style_conversation", "Chat look", "lucide:palette", "Changes how the conversation looks: surface, background, accent, bubbles, tint and theme.", &[("conversation", Conversation), ("appearance", Json)], &[], "style_conversation(conversation, if appearance == () { #{} } else { appearance });\n#{ conversation: conversation }"),
    ]
    })
}

pub fn builtin(key: &str) -> Option<&'static Builtin> {
    builtins().iter().find(|b| b.key == key)
}

pub fn body_of(node: &Node) -> &str {
    node.kind.strip_prefix(BUILTIN_PREFIX).and_then(builtin).map_or(node.body.as_str(), |b| b.body)
}

pub fn trigger_outputs(on: TriggerOn) -> Vec<Port> {
    use PortType::*;
    let mut outputs = ports(&[("message", Message), ("conversation", Conversation), ("person", Person), ("text", Text)]);
    match on {
        TriggerOn::Reaction => outputs.push(Port::new("reaction", Text)),
        TriggerOn::EventSoon => outputs.push(Port::new("event", Json)),
        TriggerOn::Schedule => outputs.push(Port::new("at", Text)),
        TriggerOn::Custom => outputs.extend(ports(&[("payload", Json), ("value", Json), ("previous", Json)])),
        _ => {}
    }
    outputs
}

pub const KIND_TRIGGER: &str = "trigger";
pub const KIND_STEP: &str = "step";
pub const KIND_LANDING: &str = "landing";
pub const KIND_ACTION: &str = "action";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BuiltinGroup {
    pub key: &'static str,
    pub parent: Option<&'static str>,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
}

pub fn builtin_groups() -> Vec<BuiltinGroup> {
    let g = |key, parent, name, description, icon| BuiltinGroup { key, parent, name, description, icon };
    vec![
        g(BUILTIN_GROUP, None, "Built in", "What Chatnip ships with. Read only; drag one into an action, nest it, or duplicate it to change it.", "lucide:package"),
        g("builtin.triggers", Some(BUILTIN_GROUP), "Triggers", "Where an action starts.", "lucide:radio-tower"),
        g("builtin.triggers.messages", Some("builtin.triggers"), "Messages", "Something happens to a message.", "lucide:message-square"),
        g("builtin.triggers.chats", Some("builtin.triggers"), "Chats", "Something flows into a chat.", "lucide:messages-square"),
        g("builtin.triggers.time", Some("builtin.triggers"), "Time", "A time comes round.", "lucide:clock"),
        g("builtin.triggers.calendar", Some("builtin.triggers"), "Calendar", "An event comes up.", "lucide:calendar"),
        g("builtin.triggers.calls", Some("builtin.triggers"), "By hand or by an action", "You or another action start it.", "lucide:play"),
        g("builtin.steps", Some(BUILTIN_GROUP), "Steps", "What happens in between.", "lucide:workflow"),
        g("builtin.steps.ai", Some("builtin.steps"), "Smart", "Asks your AI.", "lucide:sparkles"),
        g("builtin.steps.text", Some("builtin.steps"), "Text and files", "Reads messages, conversations, files and templates.", "lucide:file-text"),
        g("builtin.steps.match", Some("builtin.steps"), "Match and filter", "Patterns, times and conditions.", "lucide:filter"),
        g("builtin.steps.flow", Some("builtin.steps"), "Flow", "Waits and your own code.", "lucide:git-fork"),
        g("builtin.landings", Some(BUILTIN_GROUP), "Landings", "Where the result ends up.", "lucide:map-pin"),
        g("builtin.landings.tags", Some("builtin.landings"), "Tags and groups", "Tags on any object, and groups.", "lucide:tag"),
        g("builtin.landings.inbox", Some("builtin.landings"), "Inbox", "Splits, done, snooze, mute, pin, read and summaries.", "lucide:inbox"),
        g("builtin.landings.chat", Some("builtin.landings"), "Chat", "Into a chat, a reply or a message.", "lucide:message-circle"),
        g("builtin.landings.notify", Some("builtin.landings"), "Notify", "A notification for you.", "lucide:bell"),
        g("builtin.landings.calendar", Some("builtin.landings"), "Calendar", "Events in your calendar.", "lucide:calendar-plus"),
        g("builtin.landings.look", Some("builtin.landings"), "Look and feel", "How messages and chats show.", "lucide:palette"),
        g("builtin.landings.people", Some("builtin.landings"), "People", "What Chatnip knows about a person.", "lucide:user"),
        g("builtin.actions", Some(BUILTIN_GROUP), "Organization", "Whole actions Chatnip ships with that sort what arrives: who writes, receipts, newsletters and invites. Switch them on or off, nest them or duplicate them.", "lucide:zap"),
    ]
}

fn builtin_place(key: &str) -> (&'static str, &'static str) {
    match key {
        "tag_add" | "tag_remove" | "group_add" => ("builtin.landings.tags", KIND_LANDING),
        "move" | "done" | "snooze" | "mute" | "pin" | "mark_read" | "store_summary" => ("builtin.landings.inbox", KIND_LANDING),
        "reply" | "send" | "draft" | "forward" | "schedule_send" => ("builtin.landings.chat", KIND_LANDING),
        "notify" | "post_status" => ("builtin.landings.notify", KIND_LANDING),
        "create_event" | "propose_event" => ("builtin.landings.calendar", KIND_LANDING),
        "highlight" | "collapse" | "hide" | "present_conversation" | "style_conversation" => ("builtin.landings.look", KIND_LANDING),
        k if k.starts_with("present_") => ("builtin.landings.look", KIND_LANDING),
        "mark_person" => ("builtin.landings.people", KIND_LANDING),
        "ai" | "ai_json" | "ai_reply" | "classify" | "summarize" | "prompt" | "find_events" | "sender_kind" | "receipt_confirm" => ("builtin.steps.ai", KIND_STEP),
        "regex_match" | "regex_find" | "regex_replace" | "has_time" | "receipt_match" => ("builtin.steps.match", KIND_STEP),
        "wait" => ("builtin.steps.flow", KIND_STEP),
        _ => ("builtin.steps.text", KIND_STEP),
    }
}

pub fn landing_keys() -> Vec<&'static str> {
    builtins().iter().filter(|b| builtin_place(b.key).1 == KIND_LANDING).map(|b| b.key).collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct PickerItem {
    pub key: String,
    pub group: &'static str,
    pub kind: &'static str,
    pub label: String,
    pub description: String,
    pub icon: String,
    pub node: Node,
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
    pub requires: Vec<&'static str>,
}

fn picker_node(kind: &str, label: &str, config: Value, body: &str) -> Node {
    Node { id: String::new(), kind: kind.to_string(), label: label.to_string(), config, body: body.to_string(), x: 0.0, y: 0.0 }
}

fn picker_item(key: String, group: &'static str, kind: &'static str, description: &str, icon: &str, node: Node) -> PickerItem {
    let (inputs, outputs) = node_ports(&node, &HashMap::new(), None).unwrap_or_default();
    let requires = node_requirements(&node, &HashMap::new());
    PickerItem { key, group, kind, label: node.label.clone(), description: description.into(), icon: icon.into(), node, inputs, outputs, requires }
}

pub fn picker_defaults() -> Vec<PickerItem> {
    let trigger = |key: &str, group: &'static str, label: &str, icon: &str, description: &str, config: Value, body: &str| {
        picker_item(format!("{}{}", BUILTIN_KEY_PREFIX, key), group, KIND_TRIGGER, description, icon, picker_node(NODE_TRIGGER, label, config, body))
    };
    let template = |key: &str, label: &str, icon: &str, description: &str, inputs: Value, outputs: Value, body: &str| {
        picker_item(format!("{}{}", BUILTIN_PREFIX, key), "builtin.steps.flow", KIND_STEP, description, icon, picker_node(NODE_SCRIPT, label, json!({ "inputs": inputs, "outputs": outputs }), body))
    };
    let messages = "builtin.triggers.messages";
    let mut items = vec![
        trigger("arrival", messages, "Message arrived", "lucide:inbox", "A message arrives. Narrow it in the node: kinds of connection, connections, chats, groups, tags, people, direction, attachments and kinds of message.", json!({ "on": "arrival" }), ""),
        trigger("sent", messages, "Message sent", "lucide:send", "You send a message, from Chatnip or anywhere else, for follow ups.", json!({ "on": "sent" }), ""),
        trigger("edited", messages, "Message edited", "lucide:pencil", "A message is edited.", json!({ "on": "edited" }), ""),
        trigger("deleted", messages, "Message deleted", "lucide:trash", "A message is deleted.", json!({ "on": "deleted" }), ""),
        trigger("reaction", messages, "Reaction added", "lucide:smile-plus", "A reaction is added to a message.", json!({ "on": "reaction" }), ""),
        trigger("mention", messages, "Mention", "lucide:at-sign", "A message that arrives mentions you.", json!({ "on": "mention" }), ""),
        trigger("pattern", messages, "Pattern matches", "lucide:regex", "A message whose text matches a regular expression.", json!({ "on": "arrival" }), "regex_match(text, \"invoice|faktura\")"),
        trigger(
            "ai",
            messages,
            "AI decides",
            "lucide:sparkles",
            "The chosen AI reads the message that arrives and decides.",
            json!({ "on": "arrival" }),
            "let answer = ai_json(\"Does this message ask me to do something?\", #{ type: \"object\", properties: #{ yes: #{ type: \"boolean\" } }, required: [\"yes\"] });\nanswer.yes",
        ),
        trigger("filter", messages, "Filter matches", "lucide:filter", "A message arrives that one of your saved filter groups admits.", json!({ "on": "filter", "group_ids": [] }), ""),
        trigger("schedule", "builtin.triggers.time", "On a schedule", "lucide:calendar-clock", "Runs every so many minutes, or at a time of day on the days you pick.", json!({ "on": "schedule", "schedule": { "every_minutes": 60, "at": null, "days": [] } }), ""),
        trigger("event_soon", "builtin.triggers.calendar", "Calendar event soon", "lucide:alarm-clock", "An event in your calendar starts within the minutes you set.", json!({ "on": "event_soon", "minutes_before": DEFAULT_MINUTES_BEFORE }), ""),
        trigger("manual", "builtin.triggers.calls", "Run by hand", "lucide:play", "Runs when you start it, from the page, a chat or the command line.", json!({ "on": "manual" }), ""),
        trigger("called", "builtin.triggers.calls", "Called by another action", "lucide:git-branch", "Runs when another action uses this one as a node.", json!({ "on": "called" }), ""),
    ];
    let spec = chat_model::node_spec();
    let read = spec.variant(ChatMode::Read);
    items.push(picker_item(format!("{}chat_read", BUILTIN_KEY_PREFIX), "builtin.triggers.chats", KIND_TRIGGER, &read.start, &read.icon, picker_node(NODE_CHAT, "Read chat item arrived", json!({ "mode": ChatMode::Read.name(), "conversation_id": null }), "")));
    items.push(template("condition", "Condition", "lucide:git-fork", "Sends the value down true or false.", json!([{ "name": "value", "type": "any" }]), json!([{ "name": "true", "type": "any" }, { "name": "false", "type": "any" }]), "if value != () {\n    #{ \"true\": value }\n} else {\n    #{ \"false\": value }\n}"));
    items.push(template("script", "Script", "lucide:code", "Your own Rhai code with the inputs and outputs you declare.", json!([{ "name": "text", "type": "text" }]), json!([{ "name": "result", "type": "text" }]), "#{ result: text }"));
    for b in builtins() {
        let (group, kind) = builtin_place(b.key);
        items.push(picker_item(format!("{}{}", BUILTIN_PREFIX, b.key), group, kind, b.description, b.icon, picker_node(&format!("{}{}", BUILTIN_PREFIX, b.key), b.label, json!({ "values": {} }), b.body)));
    }
    for mode in [ChatMode::Read, ChatMode::Write] {
        let variant = spec.variant(mode);
        items.push(picker_item(format!("{}chat_{}", BUILTIN_PREFIX, mode.name()), "builtin.landings.chat", KIND_LANDING, &variant.end, &variant.icon, picker_node(NODE_CHAT, &variant.label, json!({ "mode": mode.name(), "conversation_id": null }), "")));
    }
    items
}

pub fn builtin_kind(key: &str) -> Option<&'static str> {
    if crate::models::action::BUILTIN_ACTION_KEYS.contains(&key) {
        return Some(KIND_ACTION);
    }
    if key.starts_with(BUILTIN_KEY_PREFIX) {
        return Some(KIND_TRIGGER);
    }
    Some(match key.strip_prefix(BUILTIN_PREFIX)? {
        "chat_read" | "chat_write" => KIND_LANDING,
        "condition" | "script" => KIND_STEP,
        other => builtin_place(other).1,
    })
}

pub const KIND_CHAT: &str = "chat";

fn starts(graph: &Graph) -> Vec<TriggerOn> {
    graph.nodes.iter().filter_map(|n| n.trigger()).filter(|t| !t.disabled).map(|t| t.on).collect()
}

fn invocable(on: &[TriggerOn]) -> bool {
    on.is_empty() || on.iter().any(|o| o.is_invocable())
}

pub fn is_component(action: &Action) -> bool {
    action.builtin && action.builtin_kind.as_deref() != Some(KIND_ACTION)
}

pub fn action_role(action: &Action) -> &'static str {
    if is_component(action) {
        if let Some(kind) = action.key.as_deref().and_then(builtin_kind).filter(|k| *k != KIND_ACTION) {
            return kind;
        }
    }
    if !invocable(&starts(&action.graph)) {
        return KIND_TRIGGER;
    }
    if action.outputs.is_empty() {
        KIND_LANDING
    } else {
        KIND_STEP
    }
}

pub fn node_role(node: &Node, actions: &HashMap<String, Action>) -> &'static str {
    match node.kind.as_str() {
        NODE_TRIGGER => KIND_TRIGGER,
        NODE_CHAT => KIND_CHAT,
        NODE_SCRIPT => KIND_STEP,
        kind => match kind.strip_prefix(ACTION_PREFIX) {
            Some(id) => actions.get(id).map(action_role).unwrap_or(KIND_STEP),
            None => builtin_kind(kind).filter(|k| *k != KIND_ACTION).unwrap_or(KIND_STEP),
        },
    }
}

fn ends_in_landings(graph: &Graph, actions: &HashMap<String, Action>) -> bool {
    let fed = |id: &str| graph.edges.iter().any(|e| e.target == id);
    let finals: Vec<&Node> = graph.nodes.iter().filter(|n| n.kind != NODE_TRIGGER && !graph.edges.iter().any(|e| e.source == n.id) && (n.kind != NODE_CHAT || fed(&n.id))).collect();
    !finals.is_empty() && finals.iter().all(|n| matches!(node_role(n, actions), KIND_LANDING | KIND_CHAT))
}

pub fn usage(action: &Action, actions: &HashMap<String, Action>) -> Usage {
    if is_component(action) {
        return Usage::default();
    }
    let on = starts(&action.graph);
    let called = invocable(&on);
    let ends = ends_in_landings(&action.graph, actions);
    Usage {
        standalone: on.iter().any(|o| *o != TriggerOn::Called),
        chat: !action.builtin && on.is_empty() && (ends || !action.outputs.is_empty()),
        step: called && !action.outputs.is_empty(),
        landing: called && action.outputs.is_empty() && !action.inputs.is_empty() && ends,
    }
}

pub fn node_ports(node: &Node, actions: &HashMap<String, Action>, owner: Option<&[Port]>) -> Option<(Vec<Port>, Vec<Port>)> {
    if node.kind == NODE_TRIGGER {
        let on = node.trigger().map(|t| t.on).unwrap_or_default();
        let mut outputs = trigger_outputs(on);
        if on.is_invocable() {
            if let Some(inputs) = owner {
                let extra: Vec<Port> = inputs.iter().filter(|p| !outputs.iter().any(|o| o.name == p.name)).cloned().collect();
                outputs.extend(extra);
            }
        }
        return Some((Vec::new(), outputs));
    }
    if node.kind == NODE_SCRIPT {
        return Some((node.declared("inputs"), node.declared("outputs")));
    }
    if let Some(config) = node.chat() {
        let variant = chat_model::node_spec().variant(config.mode);
        return Some((variant.inputs.clone(), variant.outputs.clone()));
    }
    if let Some(key) = node.kind.strip_prefix(BUILTIN_PREFIX) {
        return builtin(key).map(|b| (b.inputs.clone(), b.outputs.clone()));
    }
    if let Some(id) = node.kind.strip_prefix(ACTION_PREFIX) {
        return actions.get(id).map(|a| (if action_role(a) == KIND_TRIGGER { Vec::new() } else { a.inputs.clone() }, a.outputs.iter().map(|p| Port { node: None, source_port: None, ..p.clone() }).collect()));
    }
    None
}

pub const RETURNS: &str = "returns";
pub const RETIRED_OUTPUTS: &str = "outputs_retired";
pub const RETIRED_OUTBOX: &str = "action_outbox_retired";

fn slug(text: &str) -> String {
    let raw: String = text.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    raw.split('_').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_")
}

pub fn terminal(graph: &Graph, node: &Node) -> bool {
    node.kind != NODE_TRIGGER && node.kind != NODE_CHAT && !graph.edges.iter().any(|e| e.source == node.id)
}

pub fn signature(graph: &Graph, actions: &HashMap<String, Action>) -> Vec<Port> {
    let mut found: Vec<(&Node, String, Port)> = Vec::new();
    for node in graph.nodes.iter().filter(|n| terminal(graph, n)) {
        let Some((_, outputs)) = node_ports(node, actions, None) else { continue };
        let returns = node.config.get(RETURNS);
        for port in outputs {
            let custom = returns.and_then(|r| r.get(&port.name));
            let field = |key: &str| custom.and_then(|c| c.get(key)).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from);
            let name = field("name").unwrap_or_else(|| port.name.clone());
            found.push((node, name.clone(), Port { name, kind: port.kind, description: field("description").unwrap_or(port.description), node: Some(node.id.clone()), source_port: Some(port.name) }));
        }
    }
    let prefixes: [fn(&Node) -> String; 2] = [|n| slug(&node_label(n)), |n| slug(&n.id)];
    for prefix in prefixes {
        let clashing: Vec<bool> = found.iter().map(|(_, _, p)| found.iter().filter(|(_, _, o)| o.name == p.name).count() > 1).collect();
        for ((node, base, port), clash) in found.iter_mut().zip(clashing) {
            if clash {
                let head = prefix(node);
                port.name = format!("{}_{}", if head.is_empty() { node.id.clone() } else { head }, base);
            }
        }
    }
    found.into_iter().map(|(_, _, p)| p).collect()
}

pub fn sign(actions: &mut [Action]) {
    for action in actions.iter_mut().filter(|a| a.builtin) {
        action.builtin_kind = action.key.as_deref().and_then(builtin_kind).map(String::from);
    }
    for _ in 0..=MAX_DEPTH {
        let known: HashMap<String, Action> = actions.iter().map(|a| (a.id.clone(), a.clone())).collect();
        let mut moved = false;
        for action in actions.iter_mut() {
            let outputs = signature(&action.graph, &known);
            if outputs != action.outputs {
                action.outputs = outputs;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    let known: HashMap<String, Action> = actions.iter().map(|a| (a.id.clone(), a.clone())).collect();
    for action in actions.iter_mut() {
        action.usage = usage(action, &known);
    }
}

pub fn adopt_outputs(graph: &mut Graph, derived: &[Port], stored: &[Port]) -> bool {
    let mut changed = false;
    for old in stored.iter().filter(|p| !p.description.trim().is_empty()) {
        let Some(port) = derived.iter().find(|p| p.name == old.name).or_else(|| derived.iter().find(|p| p.source_port.as_deref() == Some(old.name.as_str()))) else { continue };
        if port.description == old.description {
            continue;
        }
        let (Some(node_id), Some(source)) = (&port.node, &port.source_port) else { continue };
        let Some(node) = graph.nodes.iter_mut().find(|n| &n.id == node_id) else { continue };
        if !node.config.is_object() {
            node.config = json!({});
        }
        let Some(config) = node.config.as_object_mut() else { continue };
        if node.kind == NODE_SCRIPT {
            if let Some(declared) = config.get_mut("outputs").and_then(Value::as_array_mut).and_then(|list| list.iter_mut().find(|p| p.get("name").and_then(Value::as_str) == Some(source.as_str()))) {
                declared["description"] = json!(old.description);
                changed = true;
            }
            continue;
        }
        let returns = config.entry(RETURNS).or_insert_with(|| json!({}));
        if !returns.is_object() {
            *returns = json!({});
        }
        let entry = returns.as_object_mut().map(|r| r.entry(source.clone()).or_insert_with(|| json!({})));
        if let Some(entry) = entry.filter(|e| e.is_object()) {
            entry["description"] = json!(old.description);
            changed = true;
        }
    }
    changed
}

pub async fn retire_outputs(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    let present = crate::database::column_exists(pool, "action", "outputs").await?;
    if !present {
        return Ok(());
    }
    let rows = sqlx::query("SELECT id, profile_id, inputs, outputs, graph FROM action").fetch_all(pool).await?;
    let mut actions = Vec::new();
    let mut stored: HashMap<String, Vec<Port>> = HashMap::new();
    for row in rows {
        let read = |column: &str| row.try_get::<Option<String>, _>(column).ok().flatten().unwrap_or_default();
        let mut action = Action::blank(&row.get::<String, _>("profile_id"), "");
        action.id = row.get("id");
        action.inputs = serde_json::from_str(&read("inputs")).unwrap_or_default();
        action.graph = serde_json::from_str(&read("graph")).unwrap_or_default();
        stored.insert(action.id.clone(), serde_json::from_str(&read("outputs")).unwrap_or_default());
        actions.push(action);
    }
    sign(&mut actions);
    for action in actions.iter_mut() {
        let old = stored.remove(&action.id).unwrap_or_default();
        if adopt_outputs(&mut action.graph, &action.outputs, &old) {
            sqlx::query("UPDATE action SET graph = ?1 WHERE id = ?2").bind(serde_json::to_string(&action.graph).unwrap_or_default()).bind(&action.id).execute(pool).await?;
        }
    }
    sqlx::query(&format!("ALTER TABLE action RENAME COLUMN outputs TO {}", RETIRED_OUTPUTS)).execute(pool).await?;
    Ok(())
}

pub async fn retire_outbox(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    let exists = |name: &'static str| crate::database::table_exists(pool, name);
    if exists("action_outbox").await? && !exists(RETIRED_OUTBOX).await? {
        sqlx::query(&format!("ALTER TABLE action_outbox RENAME TO {}", RETIRED_OUTBOX)).execute(pool).await?;
    }
    if exists("mock_seed").await? {
        sqlx::query("DELETE FROM mock_seed WHERE kind = 'approval'").execute(pool).await?;
        let mention = regex::Regex::new(r"(?i)\s*for approval").map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let rows: Vec<(String, String, String)> = sqlx::query_as("SELECT a.id, a.description, a.graph FROM action a JOIN mock_seed s ON s.item_id = a.id AND s.kind = 'action' WHERE a.updated_at = a.created_at AND (a.description LIKE '%approval%' OR a.graph LIKE '%approval%')").fetch_all(pool).await?;
        for (id, description, raw) in rows {
            let mut graph: Graph = serde_json::from_str(&raw).unwrap_or_default();
            for node in graph.nodes.iter_mut().filter(|n| n.kind == format!("{}draft", BUILTIN_PREFIX) && n.label.to_lowercase().contains("approval")) {
                node.label = "Keep as draft".into();
            }
            let description = mention.replace_all(&description, "").to_string();
            sqlx::query("UPDATE action SET description = ?1, graph = ?2 WHERE id = ?3").bind(description).bind(serde_json::to_string(&graph).unwrap_or(raw)).bind(&id).execute(pool).await?;
        }
    }
    Ok(())
}

pub fn topo_order(graph: &Graph) -> Result<Vec<String>, String> {
    let ids: Vec<&str> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
    let index: HashMap<&str, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut indegree: HashMap<&str, usize> = ids.iter().map(|id| (*id, 0)).collect();
    for edge in &graph.edges {
        if let Some(d) = indegree.get_mut(edge.target.as_str()) {
            *d += 1;
        }
    }
    let mut ready: Vec<&str> = ids.iter().copied().filter(|id| indegree[id] == 0).collect();
    let mut order = Vec::new();
    while !ready.is_empty() {
        ready.sort_by_key(|id| std::cmp::Reverse(index.get(id).copied().unwrap_or(usize::MAX)));
        let Some(id) = ready.pop() else { break };
        order.push(id.to_string());
        for edge in graph.edges.iter().filter(|e| e.source == id) {
            if let Some(d) = indegree.get_mut(edge.target.as_str()) {
                *d -= 1;
                if *d == 0 {
                    ready.push(edge.target.as_str());
                }
            }
        }
    }
    if order.len() != ids.len() {
        let stuck: Vec<&str> = ids.iter().copied().filter(|id| !order.iter().any(|o| o == id)).collect();
        return Err(format!("The graph has a loop through {}", stuck.join(", ")));
    }
    Ok(order)
}

pub fn check(draft: &ActionDraft, actions: &HashMap<String, Action>, self_id: Option<&str>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut port_map: HashMap<&str, (Vec<Port>, Vec<Port>)> = HashMap::new();
    for node in &draft.graph.nodes {
        if node.id.trim().is_empty() || !seen.insert(node.id.as_str()) {
            out.push(Diagnostic::error(format!("Node id {:?} is empty or used twice", node.id)).at_node(&node.id));
            continue;
        }
        match node_ports(node, actions, Some(&draft.inputs)) {
            Some(p) => {
                port_map.insert(node.id.as_str(), p);
            }
            None => out.push(Diagnostic::error(format!("{} is not a known node type", node.kind)).at_node(&node.id)),
        }
        let editable = node.kind == NODE_TRIGGER || node.kind == NODE_SCRIPT;
        if editable && !node.body.trim().is_empty() {
            for mut d in action_script::check_script(&node.body) {
                d.node = Some(node.id.clone());
                out.push(d);
            }
        }
        if node.kind == NODE_TRIGGER {
            if let Some(function) = action_script::calls(&node.body).into_iter().find(|name| action_script::is_effect(name)) {
                out.push(Diagnostic::error(format!("A trigger only decides whether to run; {}() changes things and belongs in a later node", function)).at_node(&node.id));
            }
            if let Some(problem) = node.trigger().and_then(|t| t.problem()) {
                out.push(Diagnostic::error(problem).at_node(&node.id));
            }
        }
        if let Some(id) = node.kind.strip_prefix(ACTION_PREFIX) {
            if Some(id) == self_id || self_id.is_some_and(|me| reaches(actions, id, me, 0)) {
                out.push(Diagnostic::error("An action cannot use itself, directly or through another action").at_node(&node.id));
            }
        }
    }
    let graph = &draft.graph;
    for edge in &graph.edges {
        if edge.source == edge.target {
            out.push(Diagnostic::error("A node cannot connect to itself").at_edge(&edge.id));
            continue;
        }
        let source = port_map.get(edge.source.as_str());
        let target = port_map.get(edge.target.as_str());
        let (Some((_, outs)), Some((ins, _))) = (source, target) else {
            out.push(Diagnostic::error(format!("The connection {} points to a missing node", edge.id)).at_edge(&edge.id));
            continue;
        };
        let into = graph.nodes.iter().find(|n| n.id == edge.target);
        let from_node = graph.nodes.iter().find(|n| n.id == edge.source);
        if into.is_some_and(|n| node_role(n, actions) == KIND_TRIGGER) {
            out.push(Diagnostic::error(format!("{} is a trigger; a trigger cannot receive input", edge.target)).at_edge(&edge.id));
            continue;
        }
        if from_node.is_some_and(|n| n.kind == NODE_CHAT && graph.edges.iter().any(|e| e.target == n.id)) {
            out.push(Diagnostic::error(format!("{} receives input, so it ends the flow; nothing continues from it", edge.source)).at_edge(&edge.id));
            continue;
        }
        if from_node.is_some_and(|n| node_role(n, actions) == KIND_LANDING) && !outs.iter().any(|p| p.name == edge.source_port) {
            out.push(Diagnostic::error(format!("{} is a landing; nothing continues from it", edge.source)).at_edge(&edge.id));
            continue;
        }
        let from = outs.iter().find(|p| p.name == edge.source_port);
        let to = ins.iter().find(|p| p.name == edge.target_port);
        match (from, to) {
            (None, _) => out.push(Diagnostic::error(format!("{} has no output {}", edge.source, edge.source_port)).at_edge(&edge.id)),
            (_, None) => out.push(Diagnostic::error(format!("{} has no input {}", edge.target, edge.target_port)).at_edge(&edge.id)),
            (Some(f), Some(t)) if !t.kind.accepts(f.kind) => out.push(Diagnostic::error(format!("{} gives {}, but {} needs {}", edge.source_port, f.kind.name(), edge.target_port, t.kind.name())).at_edge(&edge.id)),
            _ => {}
        }
    }
    let refused: HashSet<String> = wiring(&out).into_iter().filter_map(|d| d.edge.clone()).collect();
    let looped = Graph { nodes: graph.nodes.clone(), edges: graph.edges.iter().filter(|e| !refused.contains(&e.id)).cloned().collect() };
    if let Err(e) = topo_order(&looped) {
        out.push(Diagnostic::error(e));
    }
    out
}

pub fn wiring(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics.iter().filter(|d| d.severity == "error" && d.edge.is_some()).collect()
}

fn refuse_wiring(after: &[Diagnostic], before: &[Diagnostic]) -> Result<(), String> {
    let known: HashSet<&str> = wiring(before).into_iter().filter_map(|d| d.edge.as_deref()).collect();
    let fresh: Vec<String> = wiring(after).into_iter().filter(|d| d.edge.as_deref().is_some_and(|e| !known.contains(e))).map(|d| d.message.clone()).collect();
    if fresh.is_empty() {
        Ok(())
    } else {
        Err(format!("Fix the connections first: {}", fresh.join("; ")))
    }
}

fn reaches(actions: &HashMap<String, Action>, from: &str, target: &str, depth: i64) -> bool {
    if depth > MAX_DEPTH {
        return true;
    }
    let Some(action) = actions.get(from) else { return false };
    action.graph.nodes.iter().filter_map(|n| n.kind.strip_prefix(ACTION_PREFIX)).any(|id| id == target || reaches(actions, id, target, depth + 1))
}

fn gather_requirements(node: &Node, actions: &HashMap<String, Action>, depth: i64, nested: bool, out: &mut Vec<&'static str>) {
    let mut add = |keys: &[&'static str]| {
        for key in keys {
            if !out.contains(key) {
                out.push(key);
            }
        }
    };
    if let Some(trigger) = node.trigger() {
        if nested {
            return;
        }
        if !trigger.on.is_invocable() {
            add(&[action_script::REQUIRES_ACTIONS_ENABLED]);
        }
    }
    if node.chat().is_some_and(|c| c.mode == ChatMode::Write) {
        add(&[action_script::REQUIRES_ACCOUNT]);
    }
    for function in action_script::calls(body_of(node)) {
        add(action_script::function_requirements(function));
    }
    if let Some(id) = node.kind.strip_prefix(ACTION_PREFIX) {
        if depth >= MAX_DEPTH {
            return;
        }
        if let Some(action) = actions.get(id) {
            for inner in &action.graph.nodes {
                gather_requirements(inner, actions, depth + 1, true, out);
            }
        }
    }
}

pub fn node_requirements(node: &Node, actions: &HashMap<String, Action>) -> Vec<&'static str> {
    let mut found = Vec::new();
    gather_requirements(node, actions, 0, false, &mut found);
    action_script::requirements().into_iter().map(|r| r.key).filter(|key| found.contains(key)).collect()
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RequirementState {
    pub actions: bool,
    pub intelligence: bool,
    pub connection: bool,
    pub model: bool,
    pub accounts: bool,
    pub voice: bool,
    pub voice_models: bool,
}

impl RequirementState {
    pub fn met(&self, key: &str) -> bool {
        match key {
            action_script::REQUIRES_ACTIONS_ENABLED => self.actions,
            action_script::REQUIRES_AI_ENABLED => self.intelligence,
            action_script::REQUIRES_AI_CONNECTION => self.connection,
            action_script::REQUIRES_AI_MODEL => self.model,
            action_script::REQUIRES_ACCOUNT => self.accounts,
            action_script::REQUIRES_VOICE_ENABLED => self.voice,
            action_script::REQUIRES_VOICE_MODELS => self.voice_models,
            _ => true,
        }
    }
}

pub async fn requirement_state(pool: &Pool<Sqlite>, profile_id: &str) -> Result<RequirementState, String> {
    let default = crate::models::intelligence::IntelligenceConnection::default_for(pool, profile_id).await?;
    let voice = crate::services::voice_service::VoiceSettings::get(pool).await?.unmet(crate::services::voice_service::file_exists);
    let accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider WHERE profile_id = ?1").bind(profile_id).fetch_one(pool).await.map_err(|e| format!("Failed to count connections: {}", e))?;
    Ok(RequirementState {
        actions: enabled(pool).await,
        intelligence: crate::services::assistant_service::enabled(pool).await,
        connection: default.is_some(),
        model: default.is_some_and(|c| c.kind == crate::models::intelligence::KIND_CLAUDE_CLI || c.setting("model").is_some_and(|m| !m.trim().is_empty())),
        accounts: accounts > 0,
        voice: !voice.contains(&action_script::REQUIRES_VOICE_ENABLED),
        voice_models: !voice.contains(&action_script::REQUIRES_VOICE_MODELS),
    })
}

pub fn unmet(draft: &ActionDraft, actions: &HashMap<String, Action>, state: &RequirementState) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for node in &draft.graph.nodes {
        for key in node_requirements(node, actions) {
            if state.met(key) {
                continue;
            }
            if let Some(requirement) = action_script::requirement(key) {
                out.push(Diagnostic::requirement(&requirement).at_node(&node.id));
            }
        }
    }
    out
}

fn normalize(graph: &mut Graph) {
    for node in &mut graph.nodes {
        if let Some(b) = node.kind.strip_prefix(BUILTIN_PREFIX).and_then(builtin) {
            node.body = b.body.to_string();
        }
        if node.kind.starts_with(ACTION_PREFIX) || node.kind == NODE_CHAT {
            node.body.clear();
        }
        if let Some(prompts) = node.config.get_mut("prompts").and_then(Value::as_object_mut) {
            for prompt in prompts.values_mut().filter_map(Value::as_object_mut) {
                if prompt.get("content_id").and_then(Value::as_str).is_some_and(|id| !id.trim().is_empty()) {
                    prompt.remove("template");
                } else {
                    prompt.remove("content_id");
                }
            }
        }
    }
}

fn field_path(field: &Field) -> &'static str {
    match field {
        Field::ProviderType => "conversation.provider",
        Field::ConnectionName => "conversation.connection",
        Field::ConversationType => "conversation.kind",
        Field::ConversationName => "conversation.name",
        Field::SenderEmail => "message.sender_email",
        Field::SenderDomain => "message.sender_domain",
        Field::SenderName => "message.sender",
        Field::Subject => "message.subject",
        Field::Body => "text",
        Field::MemberCount => "conversation.members",
        Field::Namespace => "conversation.namespace",
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct LegacyRuleSteps {
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub archive: bool,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub pin: bool,
    #[serde(default)]
    pub mark_read: bool,
    #[serde(default)]
    pub stop: bool,
}

#[derive(Debug, Clone, Default)]
pub struct LegacyRule {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub enabled: bool,
    pub position: i64,
    pub r#match: Match,
    pub conditions: Vec<Condition>,
    pub steps: LegacyRuleSteps,
    pub explanation: String,
}

pub const RETIRED_RULES: &str = "rule_retired";
pub const RETIRED_SCENARIOS: &str = "scenario_retired";

pub fn script_node(id: &str, label: &str, inputs: Value, outputs: Value, body: &str, y: f64) -> Node {
    Node { id: id.into(), kind: NODE_SCRIPT.into(), label: label.into(), config: json!({ "inputs": inputs, "outputs": outputs }), body: body.into(), x: 320.0, y }
}

pub fn trigger_node(id: &str, label: &str, config: Value, body: &str, y: f64) -> Node {
    Node { id: id.into(), kind: NODE_TRIGGER.into(), label: label.into(), config, body: body.into(), x: 0.0, y }
}

pub fn builtin_node(id: &str, key: &str, label: &str, config: Value, x: f64, y: f64) -> Node {
    Node { id: id.into(), kind: format!("{}{}", BUILTIN_PREFIX, key), label: label.into(), config, body: builtin(key).map(|b| b.body.to_string()).unwrap_or_default(), x, y }
}

pub fn wire(source: &str, source_port: &str, target: &str, target_port: &str) -> Edge {
    Edge { id: format!("{}.{}-{}.{}", source, source_port, target, target_port), source: source.into(), source_port: source_port.into(), target: target.into(), target_port: target_port.into() }
}

pub fn draft_from_rule(rule: &LegacyRule, labels: &[Tag], groups: &[Group]) -> ActionDraft {
    let joiner = if rule.r#match == Match::Any { " ||\n" } else { " &&\n" };
    let predicate = rule
        .conditions
        .iter()
        .map(|c| format!("test({}, {}, {})", field_path(&c.field), serde_json::to_string(&c.operator).unwrap_or_default(), serde_json::to_string(&c.value).unwrap_or_default()))
        .collect::<Vec<_>>()
        .join(joiner);
    let mut nodes = vec![Node { id: "trigger".into(), kind: NODE_TRIGGER.into(), label: "Message arrives".into(), config: json!({ "on": "arrival" }), body: predicate, x: 0.0, y: 0.0 }];
    let mut steps: Vec<(&str, Value)> = Vec::new();
    for id in &rule.steps.tag_ids {
        if let Some(label) = labels.iter().find(|l| &l.id == id) {
            if label.kind == KIND_SPLIT {
                steps.push(("move", json!({ "values": { "split": label.name } })));
            } else {
                steps.push(("tag_add", json!({ "values": { "tag": label.name } })));
            }
        }
    }
    if let Some(group) = rule.steps.group_id.as_ref().and_then(|id| groups.iter().find(|g| &g.id == id)) {
        steps.push(("group_add", json!({ "values": { "group": group.name } })));
    }
    for (flag, kind) in [(rule.steps.mute, "mute"), (rule.steps.pin, "pin"), (rule.steps.mark_read, "mark_read"), (rule.steps.archive, "done")] {
        if flag {
            steps.push((kind, json!({ "values": {} })));
        }
    }
    let mut edges = Vec::new();
    for (i, (kind, config)) in steps.into_iter().enumerate() {
        let id = format!("step{}", i + 1);
        nodes.push(builtin_node(&id, kind, "", config, 0.0, 160.0 * (i as f64 + 1.0)));
        edges.push(wire("trigger", "conversation", &id, "conversation"));
    }
    let mut graph = Graph { nodes, edges };
    normalize(&mut graph);
    let mut description = rule.explanation.clone();
    if rule.steps.stop {
        description = format!("{}{}This was a sorting rule that stopped later rules; actions all run side by side.", description, if description.is_empty() { "" } else { " " });
    }
    ActionDraft { name: rule.name.clone(), description, icon: Some("lucide:list-filter".into()), max_sends_per_hour: None, inputs: Vec::new(), graph }
}

pub struct DefaultAction {
    pub key: &'static str,
    pub group: &'static str,
    pub enabled: bool,
    pub draft: ActionDraft,
}

pub fn default_actions() -> Vec<DefaultAction> {
    let message = || json!([{ "name": "message", "type": "message" }]);
    vec![
        DefaultAction {
            key: "sort_splits",
            group: "builtin.steps.ai",
            enabled: true,
            draft: ActionDraft {
                name: "Sort into splits".into(),
                description: "When a message arrives in a conversation that has no split yet, your AI picks the split with the Sort prompt.".into(),
                icon: Some("lucide:inbox".into()),
                max_sends_per_hour: None,
                inputs: Vec::new(),
                graph: Graph {
                    nodes: vec![
                        trigger_node("trigger", "Message arrives without a split", json!({ "on": "arrival" }), "!has_split(conversation)", 0.0),
                        script_node("sort", "Pick a split", json!([{ "name": "conversation", "type": "conversation" }]), json!([{ "name": "split", "type": "tag" }]), "let answer = prompt(\"sort\", #{ conversation: conversation });\nif answer.confidence >= 0.5 {\n    move(conversation, answer.label);\n    #{ split: answer.label }\n} else {\n    #{}\n}", 0.0),
                    ],
                    edges: vec![wire("trigger", "conversation", "sort", "conversation")],
                },
            },
        },
        DefaultAction {
            key: "summarise_mail",
            group: "builtin.steps.ai",
            enabled: true,
            draft: ActionDraft {
                name: "Summarise mail".into(),
                description: "Keeps a one line summary on every mail, which folded mails in a thread show. Chatnip also runs it by hand when a folded mail has none yet.".into(),
                icon: Some("lucide:sparkles".into()),
                max_sends_per_hour: None,
                inputs: Vec::new(),
                graph: Graph {
                    nodes: vec![
                        trigger_node("arrival", "Mail arrives", json!({ "on": "arrival", "scope": { "types": ["email"] } }), "", 0.0),
                        trigger_node("manual", "Summarise a mail", json!({ "on": "manual" }), "", 200.0),
                        script_node("summarise", "Summarise once", message(), json!([{ "name": "summary", "type": "text" }]), "let m = get_message(message.id);\nif m.summary != \"\" {\n    #{ summary: m.summary }\n} else {\n    let line = prompt(\"summarise_mail\", #{ message: m }).line;\n    #{ summary: store_summary(m, line) }\n}", 100.0),
                    ],
                    edges: vec![wire("arrival", "message", "summarise", "message"), wire("manual", "message", "summarise", "message")],
                },
            },
        },
        DefaultAction {
            key: "draft_reply",
            group: "builtin.steps.ai",
            enabled: true,
            draft: ActionDraft {
                name: "Draft a reply".into(),
                description: "Writes a reply in your tone from the thread and an instruction. The composer's AI draft button runs it.".into(),
                icon: Some("lucide:pen-line".into()),
                max_sends_per_hour: None,
                inputs: vec![Port::new("conversation", PortType::Conversation), Port::new("instruction", PortType::Text)],
                graph: Graph {
                    nodes: vec![
                        trigger_node("manual", "Draft from the composer", json!({ "on": "manual" }), "", 0.0),
                        script_node("draft", "Draft", json!([{ "name": "conversation", "type": "conversation" }, { "name": "instruction", "type": "text" }]), json!([{ "name": "reply", "type": "text" }]), "#{ reply: prompt(\"reply_draft\", #{ conversation: conversation, instruction: instruction }).reply }", 0.0),
                    ],
                    edges: vec![wire("manual", "conversation", "draft", "conversation")],
                },
            },
        },
        DefaultAction {
            key: "recognise_invites",
            group: "builtin.actions",
            enabled: true,
            draft: ActionDraft {
                name: "Recognise invites".into(),
                description: "Spots mail that invites you to a meeting, call or event, from a calendar file, a Teams, Meet or Zoom link with a time, or an invitation subject, and shows it as an invite card. Your AI decides only when the signs are unsure.".into(),
                icon: Some("lucide:calendar-check".into()),
                max_sends_per_hour: None,
                inputs: Vec::new(),
                graph: Graph {
                    nodes: vec![
                        trigger_node("arrival", "Mail arrives", json!({ "on": "arrival", "scope": { "types": ["email"] } }), "", 0.0),
                        trigger_node("manual", "Check a mail", json!({ "on": "manual" }), "", 200.0),
                        script_node("recognise", "Recognise the invite", message(), json!([{ "name": "invite", "type": "bool" }, { "name": "message", "type": "message" }, { "name": "options", "type": "json" }]), "let found = invite_details(message);\nif found.confidence < 0.5 {\n    #{ invite: false }\n} else {\n    let details = found;\n    let sure = found.confidence >= 0.85;\n    if !sure || found.starts_at == () {\n        let answer = ai_json(\"Decide whether this message invites the reader to a meeting, call or event. If it does, give its title, start and end in RFC 3339, location and join link.\\n\\nSubject: \" + message.subject + \"\\n\\n\" + message.text, #{ type: \"object\", properties: #{ invite: #{ type: \"boolean\" }, title: #{ type: \"string\" }, starts_at: #{ type: \"string\" }, ends_at: #{ type: \"string\" }, location: #{ type: \"string\" }, join_url: #{ type: \"string\" } }, required: [\"invite\"] });\n        sure = sure || answer.invite;\n        if answer.invite {\n            details.mixin(answer);\n        }\n    }\n    if sure {\n        #{ invite: true, message: message, options: #{ invite: details } }\n    } else {\n        #{ invite: false }\n    }\n}", 100.0),
                        builtin_node("show", "present_invite", "Show as invite", json!({ "template": "" }), 0.0, 320.0),
                        script_node("found", "Invite found", json!([{ "name": "invite", "type": "bool" }]), json!([{ "name": "invite", "type": "bool" }]), "#{ invite: invite }", 480.0),
                    ],
                    edges: vec![wire("arrival", "message", "recognise", "message"), wire("manual", "message", "recognise", "message"), wire("recognise", "message", "show", "message"), wire("recognise", "options", "show", "options"), wire("recognise", "invite", "found", "invite")],
                },
            },
        },
    ]
}

pub async fn legacy_rules(pool: &Pool<Sqlite>) -> Result<Vec<LegacyRule>, String> {
    if !crate::database::table_exists(pool, "rule").await.map_err(|e| e.to_string())? {
        return Ok(Vec::new());
    }
    let rows = sqlx::query("SELECT * FROM rule ORDER BY position, created_at").fetch_all(pool).await.map_err(|e| format!("Failed to read the old rules: {}", e))?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let conditions: String = row.try_get("conditions").unwrap_or_default();
            let steps: String = row.try_get("actions").unwrap_or_default();
            LegacyRule {
                id: row.get("id"),
                profile_id: row.get("profile_id"),
                name: row.get("name"),
                enabled: row.try_get("enabled").unwrap_or(true),
                position: row.try_get("position").unwrap_or(0),
                r#match: if row.try_get::<String, _>("match_mode").unwrap_or_default() == "any" { Match::Any } else { Match::All },
                conditions: serde_json::from_str(&conditions).unwrap_or_default(),
                steps: serde_json::from_str(&steps).unwrap_or_default(),
                explanation: row.try_get("explanation").unwrap_or_default(),
            }
        })
        .collect())
}

async fn group_named(pool: &Pool<Sqlite>, profile_id: &str, name: &str, description: &str, icon: &str) -> Result<String, String> {
    if let Some(group) = ActionGroup::all(pool, profile_id).await?.into_iter().find(|g| g.name == name) {
        return Ok(group.id);
    }
    let input = crate::models::action::PostActionGroupInput { name: name.into(), parent_id: None, description: Some(description.into()), icon: Some(icon.into()) };
    Ok(ActionGroup::post(pool, profile_id, &input).await?.id)
}

async fn retire_built_in_group(pool: &Pool<Sqlite>, profile_id: &str) -> Result<(), String> {
    sqlx::query("UPDATE action_group SET key = ?1, name = 'Starter actions', description = 'Actions Chatnip starts you off with. Edit, switch off, copy or delete them.' WHERE profile_id = ?2 AND key IS NULL AND name = 'Built in'")
        .bind(STARTER_GROUP)
        .bind(profile_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to rename the starter group: {}", e))?;
    Ok(())
}

async fn ensure_builtin_groups(pool: &Pool<Sqlite>, profile_id: &str) -> Result<HashMap<&'static str, String>, String> {
    let existing = ActionGroup::all(pool, profile_id).await?;
    let mut ids: HashMap<&'static str, String> = HashMap::new();
    for (position, group) in builtin_groups().into_iter().enumerate() {
        let parent = group.parent.and_then(|p| ids.get(p).cloned());
        let id = match existing.iter().find(|g| g.key.as_deref() == Some(group.key)) {
            Some(found) => found.id.clone(),
            None => {
                let input = crate::models::action::PostActionGroupInput { name: group.name.into(), parent_id: parent.clone(), description: Some(group.description.into()), icon: Some(group.icon.into()) };
                ActionGroup::post(pool, profile_id, &input).await?.id
            }
        };
        let position = if group.parent.is_none() { 10_000 } else { position as i64 };
        sqlx::query("UPDATE action_group SET key = ?1, name = ?2, description = ?3, icon = ?4, parent_id = ?5, position = ?6 WHERE id = ?7")
            .bind(group.key)
            .bind(group.name)
            .bind(group.description)
            .bind(group.icon)
            .bind(&parent)
            .bind(position)
            .bind(&id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to place the built in group {}: {}", group.name, e))?;
        ids.insert(group.key, id);
    }
    Ok(ids)
}

fn connection_scope(config: &mut Value, connections: &HashSet<String>) -> bool {
    let Some(object) = config.as_object_mut() else { return false };
    let mut changed = false;
    if let Some(group) = object.remove("group_id") {
        changed = true;
        if let Some(id) = group.as_str().filter(|id| !id.is_empty()) {
            let list = object.entry("group_ids").or_insert_with(|| json!([]));
            if let Some(list) = list.as_array_mut().filter(|l| !l.iter().any(|v| v.as_str() == Some(id))) {
                list.push(json!(id));
            }
        }
    }
    let Some(scope) = object.get_mut("scope").and_then(|s| s.as_object_mut()) else { return changed };
    let providers: Vec<String> = scope.get("providers").and_then(|p| p.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    let (ids, kinds): (Vec<String>, Vec<String>) = providers.into_iter().partition(|p| connections.contains(p));
    if ids.is_empty() {
        return changed;
    }
    let mut chosen: Vec<String> = scope.get("connection_ids").and_then(|c| c.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    for id in ids {
        if !chosen.contains(&id) {
            chosen.push(id);
        }
    }
    scope.insert("providers".into(), json!(kinds));
    scope.insert("connection_ids".into(), json!(chosen));
    true
}

pub async fn migrate_connection_triggers(pool: &Pool<Sqlite>, profile_id: &str) -> Result<usize, String> {
    let connections: HashSet<String> = sqlx::query_scalar("SELECT id FROM provider WHERE profile_id = ?1")
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list connections: {}", e))?
        .into_iter()
        .collect();
    let actions = Action::all(pool, profile_id).await?;
    let stale: HashMap<String, String> = actions
        .iter()
        .filter_map(|a| a.key.as_deref().and_then(|k| k.strip_prefix(&format!("{}arrival:", BUILTIN_KEY_PREFIX))).map(|connection| (a.id.clone(), connection.to_string())))
        .collect();
    let mut moved = 0;
    for mut action in actions.into_iter().filter(|a| !stale.contains_key(&a.id)) {
        let mut changed = false;
        for node in action.graph.nodes.iter_mut() {
            if let Some(connection) = node.kind.strip_prefix(ACTION_PREFIX).and_then(|id| stale.get(id)) {
                node.kind = NODE_TRIGGER.into();
                node.config = json!({ "on": "arrival", "scope": { "connection_ids": [connection] } });
                node.body = String::new();
                changed = true;
            } else if node.kind == NODE_TRIGGER {
                changed |= connection_scope(&mut node.config, &connections);
            }
        }
        if changed {
            Action::save(pool, &action).await?;
            moved += 1;
        }
    }
    for id in stale.keys() {
        Action::purge(pool, id).await?;
    }
    Ok(moved)
}

fn retag_script(body: &str) -> String {
    let call = "add_to_case(";
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(at) = rest.find(call) {
        let bounded = rest[..at].chars().last().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        out.push_str(&rest[..at]);
        let after = &rest[at + call.len()..];
        if !bounded {
            out.push_str(call);
            rest = after;
            continue;
        }
        let (mut depth, mut quote, mut escaped, mut split, mut end) = (0i32, None::<char>, false, None, None);
        for (i, c) in after.char_indices() {
            if let Some(q) = quote {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
                continue;
            }
            match c {
                '"' | '`' | '\'' => quote = Some(c),
                '(' | '[' | '{' => depth += 1,
                ')' if depth == 0 => {
                    end = Some(i);
                    break;
                }
                ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 && split.is_none() => split = Some(i),
                _ => {}
            }
        }
        match (split, end) {
            (Some(s), Some(e)) => {
                out.push_str(&format!("tag_add({}, {})", after[s + 1..e].trim(), after[..s].trim()));
                rest = &after[e + 1..];
            }
            _ => {
                out.push_str("tag_add(");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

pub async fn migrate_case_nodes(pool: &Pool<Sqlite>, profile_id: &str) -> Result<usize, String> {
    let old_kind = format!("{}add_to_case", BUILTIN_PREFIX);
    let new_kind = format!("{}tag_add", BUILTIN_PREFIX);
    let tag_add = builtin("tag_add").ok_or("Add tag is missing from the built ins")?;
    let actions = Action::all(pool, profile_id).await?;
    let stale: HashSet<String> = actions.iter().filter(|a| a.builtin && a.key.as_deref() == Some(old_kind.as_str())).map(|a| a.id.clone()).collect();
    let mut moved = 0;
    for mut action in actions.into_iter().filter(|a| !a.builtin) {
        let mut renamed = Vec::new();
        let mut changed = false;
        for node in action.graph.nodes.iter_mut() {
            if node.kind == old_kind || node.kind.strip_prefix(ACTION_PREFIX).is_some_and(|id| stale.contains(id)) {
                node.kind = new_kind.clone();
                if node.label.is_empty() || node.label == "Add to case" {
                    node.label = tag_add.label.to_string();
                }
                if let Some(values) = node.config.get_mut("values").and_then(|v| v.as_object_mut()) {
                    if let Some(value) = values.remove("case_file") {
                        values.insert("tag".into(), value);
                    }
                }
                if !node.body.is_empty() {
                    node.body = tag_add.body.to_string();
                }
                renamed.push(node.id.clone());
                changed = true;
            } else if node.body.contains("add_to_case(") {
                node.body = retag_script(&node.body);
                changed = true;
            }
        }
        for edge in action.graph.edges.iter_mut().filter(|e| renamed.contains(&e.target) && e.target_port == "case_file") {
            edge.target_port = "tag".into();
        }
        if changed {
            Action::save(pool, &action).await?;
            moved += 1;
        }
    }
    if crate::database::table_exists(pool, "chat_action").await.map_err(|e| e.to_string())? {
        sqlx::query("UPDATE chat_action SET routes = REPLACE(REPLACE(routes, ?1, ?2), '\"case_file\"', '\"tag\"') WHERE instr(routes, ?1) > 0")
            .bind(format!("\"{}\"", old_kind))
            .bind(format!("\"{}\"", new_kind))
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to move chat routes from cases to tags: {}", e))?;
    }
    Ok(moved)
}

pub struct BuiltinAction {
    pub key: &'static str,
    pub group: &'static str,
    pub enabled: bool,
    pub draft: ActionDraft,
}

pub fn builtin_actions() -> Vec<BuiltinAction> {
    let organisation = "builtin.actions";
    vec![
        BuiltinAction { key: CLASSIFY_SENDERS_KEY, group: organisation, enabled: true, draft: classify_senders_draft() },
        BuiltinAction { key: RECEIPTS_KEY, group: organisation, enabled: false, draft: receipts_draft() },
        BuiltinAction { key: DETECT_NEWSLETTERS_KEY, group: organisation, enabled: true, draft: detect_newsletters_draft() },
    ]
    .into_iter()
    .chain(default_actions().into_iter().map(|d| BuiltinAction { key: d.key, group: d.group, enabled: d.enabled, draft: d.draft }))
    .collect()
}

pub async fn ensure_sources(pool: &Pool<Sqlite>, profile_id: &str) -> Result<(), String> {
    let fresh = !ActionGroup::all(pool, profile_id).await?.iter().any(|g| g.key.as_deref() == Some("builtin.triggers"));
    let groups = ensure_builtin_groups(pool, profile_id).await?;
    migrate_connection_triggers(pool, profile_id).await?;
    migrate_case_nodes(pool, profile_id).await?;
    let items = picker_defaults();
    let shipped = builtin_actions();
    let existing: HashMap<String, Action> = Action::all(pool, profile_id).await?.into_iter().filter_map(|a| a.key.clone().map(|k| (k, a))).collect();
    for action in existing.values().filter(|a| a.builtin && !items.iter().any(|i| Some(&i.key) == a.key.as_ref()) && !shipped.iter().any(|s| a.key.as_deref() == Some(s.key))) {
        Action::purge(pool, &action.id).await?;
    }
    let mut placed: HashMap<&str, i64> = HashMap::new();
    for item in items {
        let group_id = groups.get(item.group).cloned();
        let position = placed.entry(item.group).or_insert(0);
        let mut node = item.node.clone();
        node.id = if item.kind == KIND_TRIGGER { "trigger".into() } else { "node".into() };
        let graph = Graph { nodes: vec![node], edges: Vec::new() };
        let inputs = if item.kind == KIND_TRIGGER { Vec::new() } else { item.inputs.clone() };
        match existing.get(&item.key) {
            Some(found) => {
                if found.name != item.label || found.icon.as_deref() != Some(item.icon.as_str()) || found.description != item.description || found.graph != graph || found.inputs != inputs || found.group_id != group_id || found.position != *position {
                    let mut updated = found.clone();
                    updated.name = item.label.clone();
                    updated.icon = Some(item.icon.clone());
                    updated.description = item.description.clone();
                    updated.graph = graph;
                    updated.inputs = inputs;
                    updated.group_id = group_id;
                    updated.position = *position;
                    Action::save(pool, &updated).await?;
                }
            }
            None => {
                let mut action = Action::blank(profile_id, &item.label);
                action.key = Some(item.key.clone());
                action.group_id = group_id;
                action.icon = Some(item.icon.clone());
                action.description = item.description.clone();
                action.graph = graph;
                action.inputs = inputs;
                let id = Action::insert(pool, &action).await?.id;
                sqlx::query("UPDATE action SET position = ?1 WHERE id = ?2").bind(*position).bind(&id).execute(pool).await.map_err(|e| format!("Failed to place a built in: {}", e))?;
            }
        }
        *position += 1;
    }
    let mut placed_shipped: HashMap<&str, i64> = HashMap::new();
    for shipped in shipped.into_iter() {
        let group_id = groups.get(shipped.group).cloned();
        let position = *placed_shipped.entry(shipped.group).and_modify(|p| *p += 1).or_insert(0) as usize;
        let mut draft = shipped.draft;
        normalize(&mut draft.graph);
        match existing.get(shipped.key) {
            Some(found) => {
                let mut updated = found.clone();
                let starter = !groups.values().any(|id| found.group_id.as_ref() == Some(id));
                if starter {
                    if found.graph != draft.graph {
                        let mut mine = Action::blank(profile_id, &format!("{} (mine)", found.name));
                        mine.description = found.description.clone();
                        mine.icon = found.icon.clone();
                        mine.group_id = found.group_id.clone();
                        mine.inputs = found.inputs.clone();
                        mine.graph = found.graph.clone();
                        mine.max_sends_per_hour = found.max_sends_per_hour;
                        mine.enabled = false;
                        Action::insert(pool, &mine).await?;
                    }
                } else if fresh {
                    updated.enabled = shipped.enabled;
                }
                let differs = updated.graph != draft.graph || updated.description != draft.description || updated.name != draft.name || updated.group_id != group_id || updated.position != position as i64 || updated.enabled != found.enabled;
                if differs {
                    apply_draft(&mut updated, draft);
                    updated.group_id = group_id.clone();
                    updated.position = position as i64;
                    Action::save(pool, &updated).await?;
                }
            }
            None => {
                let mut action = Action::blank(profile_id, &draft.name);
                action.key = Some(shipped.key.to_string());
                action.group_id = group_id.clone();
                apply_draft(&mut action, draft);
                action.enabled = shipped.enabled;
                let id = Action::insert(pool, &action).await?.id;
                sqlx::query("UPDATE action SET position = ?1 WHERE id = ?2").bind(position as i64).bind(&id).execute(pool).await.map_err(|e| format!("Failed to place a built in action: {}", e))?;
                if shipped.key == "recognise_invites" {
                    if let Err(e) = recognise_invites(pool, profile_id).await {
                        eprintln!("Invites were not recognised in existing mail: {}", e);
                    }
                }
            }
        }
    }
    Ok(())
}

pub const RECEIPTS_KEY: &str = "builtin:receipts";
pub const CLASSIFY_SENDERS_KEY: &str = "classify_senders";
pub const DETECT_NEWSLETTERS_KEY: &str = "detect_newsletters";

pub fn detect_newsletters_draft() -> ActionDraft {
    let landing = |id: &str, key: &str, label: &str, values: Value, y: f64| builtin_node(id, key, label, json!({ "values": values }), 320.0, y);
    ActionDraft {
        name: "Detect newsletters".into(),
        description: "Spots a newsletter from its unsubscribe footer, its list wording or its subject, tags the conversation Newsletters and shows the mail as a digest. No AI. Built in; switch it off here, or duplicate it to change it.".into(),
        icon: Some("lucide:newspaper".into()),
        max_sends_per_hour: None,
        inputs: Vec::new(),
        graph: Graph {
            nodes: vec![
                trigger_node("arrival", "Mail arrives", json!({ "on": "arrival", "scope": { "types": ["email"] } }), "regex_match(text, \"(?i)unsubscribe|avmeld|meld deg av|nyhetsbrev|newsletter|view (this|it) in (your )?browser|manage (your )?preferences\")", 0.0),
                script_node("mark", "Newsletter", json!([{ "name": "conversation", "type": "conversation" }]), json!([{ "name": "conversation", "type": "conversation" }, { "name": "newsletter", "type": "bool" }]), "#{ conversation: conversation, newsletter: true }", 360.0),
                landing("tag", "tag_add", "Tag Newsletters", json!({ "tag": "Newsletters" }), 0.0),
                landing("digest", "present_digest", "Show as digest", json!({}), 180.0),
            ],
            edges: vec![wire("arrival", "conversation", "tag", "conversation"), wire("arrival", "message", "digest", "message"), wire("arrival", "conversation", "mark", "conversation")],
        },
    }
}

pub fn classify_senders_draft() -> ActionDraft {
    ActionDraft {
        name: "Human or service".into(),
        description: "Asks your AI whether a new sender is a person or an automated service, and marks the person accordingly, so what people write comes first. Built in; switch it off here, or duplicate it to change it.".into(),
        icon: Some("lucide:user-search".into()),
        max_sends_per_hour: None,
        inputs: Vec::new(),
        graph: Graph {
            nodes: vec![
                trigger_node("trigger", "Message from a person", json!({ "on": "arrival" }), "person != () && message.sender_email != \"\"", 0.0),
                script_node("decide", "Decide", json!([{ "name": "message", "type": "message" }, { "name": "person", "type": "person" }, { "name": "conversation", "type": "conversation" }]), json!([{ "name": "human", "type": "bool" }]), "let answer = prompt(\"classify_sender\", #{ sender: #{ name: message.sender, email: message.sender_email, provider_type: conversation.provider, sample: message.text } });\nlet human = answer.kind == \"human\";\nmark_person(person, #{ human: human });\n#{ human: human }", 0.0),
            ],
            edges: vec![wire("trigger", "message", "decide", "message"), wire("trigger", "person", "decide", "person"), wire("trigger", "conversation", "decide", "conversation")],
        },
    }
}

pub fn receipts_draft() -> ActionDraft {
    let step = |id: &str, key: &str, label: &str, values: Value, y: f64| builtin_node(id, key, label, json!({ "values": values }), 0.0, y);
    let patterns = serde_json::to_value(receipt_service::default_patterns()).unwrap_or(Value::Null);
    let mut graph = Graph {
        nodes: vec![
            trigger_node("start", "Message arrived", json!({ "on": "arrival" }), "", 0.0),
            step("extract", "receipt_extract", "Extract", json!({}), 180.0),
            step("match", "receipt_match", "Match", json!({ "patterns": patterns, "threshold": app_setting::get_default_f64(app_setting::RECEIPTS_THRESHOLD) }), 360.0),
            step("confirm", "receipt_confirm", "Confirm", json!({ "batch": app_setting::get_default_u64(app_setting::RECEIPTS_BATCH) }), 540.0),
            step("organise", "receipt_organise", "Organise", json!({}), 720.0),
        ],
        edges: vec![
            wire("start", "message", "extract", "message"),
            wire("extract", "material", "match", "material"),
            wire("match", "candidates", "confirm", "candidates"),
            wire("confirm", "candidates", "organise", "candidates"),
        ],
    };
    normalize(&mut graph);
    ActionDraft {
        name: "Receipts".into(),
        description: "Finds receipts in what arrives from every connection: reads the mail and its PDFs, HTML and images, matches the patterns of a receipt in Norwegian and English, lets a small local model confirm them, and gives one record per receipt. Read only; duplicate it to change it, or duplicate it without its trigger and attach it to a read chat.".into(),
        icon: Some("lucide:receipt".into()),
        max_sends_per_hour: None,
        inputs: Vec::new(),
        graph,
    }
}

pub async fn ensure_defaults(pool: &Pool<Sqlite>, profile_id: &str) -> Result<(), String> {
    retire_built_in_group(pool, profile_id).await?;
    if crate::database::table_exists(pool, "rule").await.map_err(|e| e.to_string())? {
        for rule in legacy_rules(pool).await? {
            let key = format!("rule:{}", rule.id);
            if Action::find(pool, &rule.profile_id, &key).await.is_ok() {
                continue;
            }
            let group_id = group_named(pool, &rule.profile_id, "Sorting rules", "Your sorting rules, now actions.", "lucide:list-filter").await?;
            let labels = Tag::all(pool, &rule.profile_id).await?;
            let groups = Group::all(pool, &rule.profile_id).await.unwrap_or_default();
            let mut action = Action::blank(&rule.profile_id, &rule.name);
            action.key = Some(key);
            action.group_id = Some(group_id);
            apply_draft(&mut action, draft_from_rule(&rule, &labels, &groups));
            action.enabled = rule.enabled;
            action.position = rule.position;
            Action::insert(pool, &action).await?;
        }
        if !crate::database::table_exists(pool, RETIRED_RULES).await.map_err(|e| e.to_string())? {
            sqlx::query(&format!("ALTER TABLE rule RENAME TO {}", RETIRED_RULES)).execute(pool).await.map_err(|e| format!("Failed to retire the old rules: {}", e))?;
        }
    }
    sqlx::query("DELETE FROM app_setting WHERE key = 'intelligence.classify_senders'").execute(pool).await.ok();
    adopt_scenarios(pool, profile_id).await?;
    ensure_sources(pool, profile_id).await?;
    adopt_feature_models(pool, profile_id).await
}

pub async fn adopt_feature_models(pool: &Pool<Sqlite>, profile_id: &str) -> Result<(), String> {
    let stored: HashMap<String, String> = sqlx::query("SELECT key, value FROM app_setting WHERE key LIKE ?1")
        .bind(format!("{}%", app_setting::FEATURE_PREFIX))
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to read the AI choices per feature: {}", e))?
        .into_iter()
        .map(|row| (row.get("key"), row.get("value")))
        .collect();
    let moved: Vec<(&[&str], AiChoice)> = app_setting::MOVED_FEATURES
        .iter()
        .map(|(id, needles)| (*needles, AiChoice { connection_id: stored.get(&app_setting::feature_key(id, app_setting::FEATURE_CONNECTION)).cloned(), model: stored.get(&app_setting::feature_key(id, app_setting::FEATURE_MODEL)).cloned() }))
        .filter(|(_, ai)| !ai.is_empty())
        .collect();
    if moved.is_empty() {
        return Ok(());
    }
    for mut action in Action::all(pool, profile_id).await?.into_iter().filter(|a| a.ai.is_empty() && !is_component(a)) {
        let bodies: Vec<&str> = action.graph.nodes.iter().map(body_of).collect();
        if let Some((_, ai)) = moved.iter().find(|(needles, _)| needles.iter().any(|needle| bodies.iter().any(|body| body.contains(needle)))) {
            action.ai = ai.clone();
            Action::save(pool, &action).await?;
        }
    }
    for (id, _) in app_setting::MOVED_FEATURES {
        for part in [app_setting::FEATURE_CONNECTION, app_setting::FEATURE_MODEL] {
            let key = app_setting::feature_key(id, part);
            let renamed = match (id == app_setting::THREADS_FROM).then(|| app_setting::get_model_feature(intelligence_service::SECTION_PROMPT)).flatten() {
                Some(feature) => app_setting::feature_key(feature.id, part),
                None => format!("{}_retired", key),
            };
            sqlx::query("UPDATE OR IGNORE app_setting SET key = ?1 WHERE key = ?2").bind(&renamed).bind(&key).execute(pool).await.map_err(|e| format!("Failed to move the AI choice {}: {}", key, e))?;
        }
    }
    Ok(())
}

async fn adopt_scenarios(pool: &Pool<Sqlite>, profile_id: &str) -> Result<(), String> {
    if !crate::database::table_exists(pool, "scenario").await.map_err(|e| e.to_string())? {
        return Ok(());
    }
    crate::database::ensure_columns(pool, "scenario", &[("adopted_at", "TEXT")]).await;
    let rows = sqlx::query("SELECT * FROM scenario WHERE profile_id = ?1 AND adopted_at IS NULL").bind(profile_id).fetch_all(pool).await.map_err(|e| format!("Failed to read the old prompts: {}", e))?;
    let mut actions = Action::all(pool, profile_id).await?;
    let mut touched = HashSet::new();
    for row in rows {
        let key: String = row.get("key");
        let optional = |column: &str| row.try_get::<Option<String>, _>(column).ok().flatten();
        let Some(overrides) = intelligence_service::overrides(&key, &row.get::<String, _>("system"), &row.get::<String, _>("template"), optional("output_schema").as_deref(), optional("connection_id").as_deref(), optional("model").as_deref()) else { continue };
        let mut needles = vec![format!("prompt(\"{}\"", key)];
        match key.as_str() {
            "script_ai" | "reply_draft" => needles.push("ai(".into()),
            "script_ai_json" => needles.push("ai_json(".into()),
            _ => {}
        }
        for action in actions.iter_mut() {
            for node in action.graph.nodes.iter_mut().filter(|n| n.kind == NODE_SCRIPT && needles.iter().any(|needle| n.body.contains(needle.as_str()))) {
                let Some(config) = node.config.as_object_mut() else { continue };
                if let Some(prompts) = config.entry("prompts").or_insert_with(|| json!({})).as_object_mut() {
                    prompts.entry(key.clone()).or_insert_with(|| overrides.clone());
                    touched.insert(action.id.clone());
                }
            }
        }
    }
    for action in actions.iter().filter(|a| touched.contains(&a.id)) {
        Action::save(pool, action).await?;
    }
    sqlx::query("UPDATE scenario SET adopted_at = ?1 WHERE profile_id = ?2").bind(Utc::now().to_rfc3339()).bind(profile_id).execute(pool).await.map_err(|e| format!("Failed to mark the old prompts as moved: {}", e))?;
    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM scenario WHERE adopted_at IS NULL").fetch_one(pool).await.map_err(|e| e.to_string())?;
    if pending == 0 && !crate::database::table_exists(pool, RETIRED_SCENARIOS).await.map_err(|e| e.to_string())? {
        sqlx::query(&format!("ALTER TABLE scenario RENAME TO {}", RETIRED_SCENARIOS)).execute(pool).await.map_err(|e| format!("Failed to retire the old prompts: {}", e))?;
    }
    Ok(())
}

#[derive(Debug, Default, Serialize)]
pub struct BackfillReport {
    pub messages: usize,
    pub runs: usize,
    pub failed: usize,
}

pub async fn backfill(runner: &Runner, action: Option<&str>, limit: i64) -> Result<BackfillReport, String> {
    let only = match action {
        Some(id) => Some(Action::find(&runner.pool, &runner.profile_id, id).await?.id),
        None => None,
    };
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT c.last_id FROM conversation c JOIN conversation_provider cp ON cp.conversation_id = c.id JOIN provider p ON p.id = cp.provider_id
         WHERE p.profile_id = ?1 AND c.last_id IS NOT NULL AND c.deleted_at IS NULL GROUP BY c.id ORDER BY c.last_activity DESC LIMIT ?2",
    )
    .bind(&runner.profile_id)
    .bind(limit.clamp(1, 500))
    .fetch_all(&runner.pool)
    .await
    .map_err(|e| format!("Failed to list recent messages: {}", e))?;
    let mut report = BackfillReport::default();
    for id in ids {
        report.messages += 1;
        match dispatch_to(runner, &[TriggerOn::Arrival, TriggerOn::Filter], &id, only.as_deref()).await {
            Ok(runs) => {
                report.failed += runs.iter().filter(|r| r.status == "failed").count();
                report.runs += runs.len();
            }
            Err(e) => {
                eprintln!("Backfill of {} failed: {}", id, e);
                report.failed += 1;
            }
        }
    }
    Ok(report)
}

pub fn schedule_due(schedule: &TriggerSchedule, last: Option<DateTime<Utc>>, now: DateTime<Local>) -> bool {
    if let Some(minutes) = schedule.every_minutes.filter(|m| *m > 0) {
        return last.is_none_or(|l| now.with_timezone(&Utc) - l >= Duration::minutes(minutes as i64));
    }
    let Some(at) = schedule.at.as_deref().and_then(|a| NaiveTime::parse_from_str(a, "%H:%M").ok()) else { return false };
    let weekday = now.weekday().number_from_monday() as u8;
    if !schedule.days.is_empty() && !schedule.days.contains(&weekday) {
        return false;
    }
    if now.time() < at {
        return false;
    }
    let Some(slot) = now.date_naive().and_time(at).and_local_timezone(Local).single() else { return false };
    last.is_none_or(|l| l < slot.with_timezone(&Utc))
}

pub type AiStub = Arc<dyn Fn(&str, &Value) -> Result<Value, String> + Send + Sync>;

#[derive(Clone)]
pub struct Runner {
    pub pool: Pool<Sqlite>,
    pub app: Option<AppHandle>,
    pub app_data: PathBuf,
    pub cache: PathBuf,
    pub profile_id: String,
    pub dry_run: bool,
    pub ai: Option<AiStub>,
    pub test_delay: Option<std::time::Duration>,
}

struct RunEffects {
    runner: Runner,
    action_id: String,
    ai: AiChoice,
    max_sends: i64,
    predicate: bool,
    log: Mutex<Vec<LogEntry>>,
    sends: Mutex<i64>,
    node: Mutex<String>,
    prompts: Mutex<Value>,
}

fn id_of(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Object(o) => o.get("id").and_then(|v| v.as_str()).map(String::from),
        _ => None,
    }
}

fn object_type_of(value: &Value) -> &'static str {
    if value.get("sender_id").is_some() {
        OBJECT_MESSAGE
    } else if value.get("emails").is_some() {
        OBJECT_PERSON
    } else {
        OBJECT_CONVERSATION
    }
}


impl RunEffects {
    fn new(runner: &Runner, action: &Action, predicate: Option<&Node>) -> RunEffects {
        RunEffects {
            runner: Runner { dry_run: runner.dry_run || predicate.is_some(), ..runner.clone() },
            action_id: action.id.clone(),
            ai: action.ai.clone(),
            max_sends: if predicate.is_some() { 0 } else { action.max_sends_per_hour },
            predicate: predicate.is_some(),
            log: Mutex::new(Vec::new()),
            sends: Mutex::new(0),
            node: Mutex::new(predicate.map(|n| n.id.clone()).unwrap_or_default()),
            prompts: Mutex::new(predicate.and_then(|n| n.config.get("prompts").cloned()).unwrap_or(Value::Null)),
        }
    }

    async fn template_ref(&self) -> Result<Value, String> {
        let node = self.node.lock().map(|n| n.clone()).unwrap_or_default();
        let action = Action::find(&self.runner.pool, &self.runner.profile_id, &self.action_id).await?;
        let has = action.graph.nodes.iter().find(|n| n.id == node).and_then(|n| n.config.get("template")).and_then(|t| t.as_str()).is_some_and(|t| !t.trim().is_empty());
        Ok(if has { json!({ "action_id": self.action_id, "node_id": node }) } else { Value::Null })
    }

    fn note(&self, status: &str, detail: String) {
        let node = self.node.lock().map(|n| n.clone()).unwrap_or_default();
        self.log.lock().map(|mut l| l.push(LogEntry { node, kind: "effect".into(), status: status.into(), detail })).ok();
    }

    fn would(&self, detail: String) -> Value {
        self.note("would", detail.clone());
        json!({ "dry_run": true, "would": detail })
    }

    async fn run_prompt(&self, key: &str, conversation_id: Option<&str>, extra: Value) -> Result<Value, String> {
        let overrides = self.prompts.lock().map(|p| p.get(key).cloned().unwrap_or(Value::Null)).unwrap_or(Value::Null);
        let content = match overrides.get("content_id").and_then(Value::as_str) {
            Some(id) => Some(content_service::owned(&self.runner.pool, &self.runner.profile_id, id).await.map_err(|_| format!("The AI prompt this node takes from Content is gone ({})", id))?),
            None => None,
        };
        self.ask(key, overrides, content, conversation_id, extra).await
    }

    async fn ask(&self, key: &str, overrides: Value, content: Option<ContentItem>, conversation_id: Option<&str>, extra: Value) -> Result<Value, String> {
        let mut overrides = overrides.as_object().cloned().unwrap_or_default();
        if let Some(item) = content {
            if item.kind != KIND_PROMPT {
                return Err(format!("{} in Content is not an AI prompt", item.name));
            }
            overrides.insert("template".into(), Value::String(item.body.unwrap_or_default()));
        }
        if let Some(stub) = &self.runner.ai {
            return stub(key, &extra);
        }
        Ok(intelligence_service::run(&self.runner.pool, key, &Value::Object(overrides), conversation_id, extra, Some(&self.ai)).await?.output)
    }

    async fn ai_json(&self, prompt: &str, material: &str, schema: &Value) -> Result<Value, String> {
        self.run_prompt("script_ai_json", None, json!({ "prompt": prompt, "material": material, "schema_json": schema.to_string() })).await
    }

    async fn conversation_id(&self, value: &Value) -> Result<String, String> {
        id_of(value).ok_or_else(|| "Pass a conversation or its id".to_string())
    }

    async fn tag(&self, value: &Value, kind: Option<&str>) -> Result<Tag, String> {
        let wanted = text(Some(value));
        Tag::all(&self.runner.pool, &self.runner.profile_id)
            .await?
            .into_iter()
            .filter(|l| kind.is_none_or(|k| l.kind == k))
            .find(|l| l.id == wanted || l.name.eq_ignore_ascii_case(&wanted) || id_of(value).as_deref() == Some(l.id.as_str()))
            .ok_or_else(|| format!("There is no {} called {}", kind.unwrap_or("tag"), wanted))
    }

    async fn conversation_for_target(&self, target: &Value, channel: &str) -> Result<String, String> {
        if let Some(id) = id_of(target) {
            let is_conversation: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM conversation WHERE id = ?1").bind(&id).fetch_one(&self.runner.pool).await.map_err(|e| e.to_string())?;
            if is_conversation && !target.get("emails").is_some() {
                return Ok(id);
            }
            let rows = sqlx::query(
                "SELECT c.id, c.type, p.provider_type, p.id AS provider_id FROM conversation c
                 LEFT JOIN conversation_provider cp ON cp.conversation_id = c.id AND cp.is_primary = 1
                 LEFT JOIN provider p ON p.id = cp.provider_id
                 WHERE c.person_id = ?1 ORDER BY c.last_activity DESC",
            )
            .bind(&id)
            .fetch_all(&self.runner.pool)
            .await
            .map_err(|e| e.to_string())?;
            let channel = channel.to_lowercase();
            let fits = |kind: &str, provider: &str, provider_id: &str| match channel.as_str() {
                "" => true,
                "mail" | "email" => kind == "email",
                "chat" => kind != "email",
                other => provider == other || provider_id == other,
            };
            return rows
                .into_iter()
                .find(|r| fits(&r.get::<String, _>("type"), &r.try_get::<Option<String>, _>("provider_type").ok().flatten().unwrap_or_default(), &r.try_get::<Option<String>, _>("provider_id").ok().flatten().unwrap_or_default()))
                .map(|r| r.get("id"))
                .ok_or_else(|| format!("No conversation with that person fits the channel {:?}", channel));
        }
        Err("Pass a conversation or a person".to_string())
    }

    async fn outgoing(&self, conversation_id: &str, provider_id: Option<&str>, content: &str, metadata: Option<Value>) -> Result<Value, String> {
        let forward_of = metadata.as_ref().and_then(|m| m["forward_of"].as_str()).map(String::from);
        if content.trim().is_empty() && forward_of.is_none() {
            return Err("There is nothing to send".to_string());
        }
        let since = (Utc::now() - Duration::hours(1)).to_rfc3339();
        let sent = ActionRun::sent_since(&self.runner.pool, &self.action_id, &since).await? + *self.sends.lock().map_err(|_| "locked")?;
        if sent >= self.max_sends {
            return Err(format!("This action reached its limit of {} messages an hour", self.max_sends));
        }
        if self.runner.dry_run {
            return Ok(self.would(match &forward_of {
                Some(id) => format!("forward {} to {}: {}", id, conversation_id, content),
                None => format!("send in {}: {}", conversation_id, content),
            }));
        }
        *self.sends.lock().map_err(|_| "locked")? += 1;
        use crate::services::outbound_service::{self as outbound, Outbound};
        let forward_input = forward_of.map(|id| outbound::PostMessageForwardInput { message_ids: vec![id], targets: vec![outbound::ForwardTarget { conversation_id: Some(conversation_id.to_string()), provider_id: provider_id.map(String::from), ..Default::default() }], comment: Some(content.to_string()) });
        if let Some(app) = self.runner.app.as_ref() {
            let held = match &forward_input {
                Some(input) => Outbound::Forward(input.clone()),
                None => Outbound::Send(SendMessageInput { conversation_id: conversation_id.to_string(), content: content.to_string(), content_type: Some("text".into()), provider_id: provider_id.map(String::from), metadata: metadata.clone(), attachments: None }),
            };
            if outbound::delays_actions(&self.runner.pool).await && outbound::is_mail(&self.runner.pool, &held).await? {
                let request = outbound::HoldRequest { surface: outbound::SURFACE_ACTION.to_string(), conversation_id: Some(conversation_id.to_string()), draft: Value::Null };
                let sent = outbound::put_outbound(&self.runner.pool, &self.runner.app_data, held, Some(request), Utc::now()).await?;
                outbound::started(app, &sent);
                let hold = sent.iter().find_map(|s| match s {
                    outbound::Sent::Held(h) => Some(h.clone()),
                    outbound::Sent::Message(_) => None,
                });
                self.note("done", format!("{}{}", SENT_PREFIX, conversation_id));
                return Ok(json!({ "held": hold.as_ref().map(|h| h.id.clone()), "conversation_id": conversation_id, "text": content, "at": hold.map(|h| h.due_at) }));
            }
        }
        let message = match forward_input {
            Some(input) => {
                let sent = crate::services::outbound_service::forward(&self.runner.pool, &self.runner.app_data, input).await?.into_iter().next().ok_or("Nothing was forwarded")?;
                if let Some(app) = &self.runner.app {
                    app.emit("conversation-sent", &conversation_id).ok();
                }
                json!({ "id": sent.id, "conversation_id": conversation_id, "text": sent.content.unwrap_or_default(), "at": sent.timestamp })
            }
            None => send_now(&self.runner, conversation_id, provider_id, content, "text", metadata, None).await?,
        };
        self.note("done", format!("{}{}", SENT_PREFIX, conversation_id));
        Ok(message)
    }
}

pub async fn send_now(runner: &Runner, conversation_id: &str, provider_id: Option<&str>, content: &str, content_type: &str, metadata: Option<Value>, attachments: Option<Vec<crate::models::OutgoingAttachmentInput>>) -> Result<Value, String> {
    let input = SendMessageInput { conversation_id: conversation_id.to_string(), content: content.to_string(), content_type: Some(content_type.to_string()), provider_id: provider_id.map(String::from), metadata, attachments };
    let message = crate::services::outbound_service::send(&runner.pool, &runner.app_data, input).await?;
    if let Some(app) = &runner.app {
        app.emit("conversation-sent", &conversation_id).ok();
    }
    Ok(json!({ "id": message.id, "conversation_id": conversation_id, "text": content, "at": message.timestamp }))
}

#[async_trait]
impl Effects for RunEffects {
    async fn call(&self, function: &str, args: Vec<Value>) -> Result<Value, String> {
        let arg = |i: usize| args.get(i).cloned().unwrap_or(Value::Null);
        let pool = &self.runner.pool;
        let effect = action_script::is_effect(function);
        if self.predicate && effect {
            return Err(format!("{}() changes things and cannot run inside a trigger", function));
        }
        if effect && self.runner.dry_run && !matches!(function, "reply" | "send" | "draft" | "forward") {
            return Ok(self.would(format!("{}({})", function, args.iter().map(|a| text(Some(a))).collect::<Vec<_>>().join(", "))));
        }
        match function {
            "ai" => {
                let options = arg(1);
                let prompt = text(Some(&arg(0)));
                match options.get("conversation").and_then(id_of) {
                    Some(conversation) => Ok(Value::String(text(self.run_prompt("reply_draft", Some(&conversation), json!({ "instruction": prompt })).await?.get("reply")))),
                    None => Ok(Value::String(text(self.run_prompt("script_ai", None, json!({ "prompt": prompt, "material": text(options.get("material")) })).await?.get("answer")))),
                }
            }
            "ai_json" => self.ai_json(&text(Some(&arg(0))), "", &arg(1)).await,
            "classify" => {
                let labels: Vec<String> = arg(1).as_array().map(|a| a.iter().map(|v| text(Some(v))).collect()).unwrap_or_default();
                if labels.is_empty() {
                    return Err("classify needs a list of labels".to_string());
                }
                let answer = self.ai_json("Pick the one label that fits the material best.", &text(Some(&arg(0))), &json!({ "type": "object", "properties": { "label": { "type": "string", "enum": labels } }, "required": ["label"] })).await?;
                Ok(answer.get("label").cloned().unwrap_or(Value::Null))
            }
            "summarize" => {
                let answer = self.ai_json("Sum up the material in one or two sentences.", &text(Some(&arg(0))), &json!({ "type": "object", "properties": { "summary": { "type": "string" } }, "required": ["summary"] })).await?;
                Ok(answer.get("summary").cloned().unwrap_or(Value::Null))
            }
            "get_message" => message_value(pool, &id_of(&arg(0)).ok_or("Pass a message id")?).await,
            "get_conversation" => conversation_value(pool, &id_of(&arg(0)).ok_or("Pass a conversation id")?).await,
            "list_messages" => {
                let conversation = self.conversation_id(&arg(0)).await?;
                let filter = arg(1);
                let limit = filter.get("limit").and_then(|v| v.as_i64()).unwrap_or(20).clamp(1, 200);
                let ids: Vec<String> = sqlx::query_scalar(
                    "SELECT id FROM message WHERE conversation_id = ?1 AND deleted_datetime IS NULL AND (?2 IS NULL OR timestamp >= ?2) AND (?3 IS NULL OR is_read = 0)
                     AND (?4 IS NULL OR LOWER(COALESCE(content, '') || ' ' || COALESCE(subject, '')) LIKE '%' || LOWER(?4) || '%') ORDER BY timestamp DESC LIMIT ?5",
                )
                .bind(&conversation)
                .bind(filter.get("from").and_then(|v| v.as_str()))
                .bind(filter.get("unread").and_then(|v| v.as_bool()).filter(|u| *u))
                .bind(filter.get("contains").and_then(|v| v.as_str()))
                .bind(limit)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
                let mut out = Vec::new();
                for id in ids {
                    out.push(message_value(pool, &id).await?);
                }
                Ok(Value::Array(out))
            }
            "person" => person_value(pool, &self.runner.profile_id, &id_of(&arg(0)).ok_or("Pass a person id")?).await,
            "people_of" => {
                let conversation = self.conversation_id(&arg(0)).await?;
                let rows = sqlx::query("SELECT u.id, COALESCE(u.display_name, u.email, '') AS name, u.email FROM conversation_member cm JOIN user u ON u.id = cm.user_id WHERE cm.conversation_id = ?1")
                    .bind(&conversation)
                    .fetch_all(pool)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(Value::Array(rows.into_iter().map(|r| json!({ "id": r.get::<String, _>("id"), "name": r.get::<String, _>("name"), "email": r.try_get::<Option<String>, _>("email").ok().flatten() })).collect()))
            }
            "search" => {
                let query = text(Some(&arg(0)));
                let ids: Vec<String> = sqlx::query_scalar(
                    "SELECT DISTINCT c.id FROM conversation c
                     JOIN conversation_provider cp ON cp.conversation_id = c.id JOIN provider p ON p.id = cp.provider_id
                     WHERE p.profile_id = ?1 AND (LOWER(c.name) LIKE '%' || LOWER(?2) || '%' OR EXISTS (
                        SELECT 1 FROM message m WHERE m.conversation_id = c.id AND LOWER(COALESCE(m.sender_name, '') || ' ' || COALESCE(m.subject, '') || ' ' || COALESCE(m.content, '')) LIKE '%' || LOWER(?2) || '%'))
                     ORDER BY c.last_activity DESC LIMIT 20",
                )
                .bind(&self.runner.profile_id)
                .bind(&query)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
                let mut out = Vec::new();
                for id in ids {
                    out.push(conversation_value(pool, &id).await?);
                }
                Ok(Value::Array(out))
            }
            "reply" => {
                let conversation = self.conversation_id(&arg(0)).await?;
                let provider = arg(2).get("channel").and_then(|v| v.as_str()).map(String::from);
                let quoted = arg(2).get("message").and_then(id_of).map(|id| json!({ "in_reply_to": id }));
                self.outgoing(&conversation, provider.as_deref(), &text(Some(&arg(1))), quoted).await
            }
            "send" => {
                let channel = match arg(2) {
                    Value::Object(o) => text(o.get("channel")),
                    other => text(Some(&other)),
                };
                let conversation = self.conversation_for_target(&arg(0), &channel).await?;
                self.outgoing(&conversation, None, &text(Some(&arg(1))), None).await
            }
            "draft" => {
                let conversation = self.conversation_id(&arg(0)).await?;
                let content = text(Some(&arg(1)));
                self.note("done", format!("Drafted for {}", conversation));
                Ok(json!({ "conversation_id": conversation, "text": content }))
            }
            "forward" => {
                let message = id_of(&arg(0)).ok_or("Pass a message")?;
                let conversation = self.conversation_for_target(&arg(1), "").await?;
                let comment = arg(2).get("comment").map(|c| text(Some(c))).unwrap_or_default();
                self.outgoing(&conversation, None, &comment, Some(json!({ "forward_of": message }))).await
            }
            "tag_add" | "tag_remove" => {
                let object = id_of(&arg(0)).ok_or("Pass the object to tag or its id")?;
                let object_type = match text(Some(&arg(2))) {
                    given if given.trim().is_empty() => object_type_of(&arg(0)).to_string(),
                    given => given.trim().to_string(),
                };
                let wanted = arg(1).get("name").map(|n| text(Some(n))).unwrap_or_else(|| text(Some(&arg(1))));
                let label = match self.tag(&arg(1), None).await {
                    Ok(label) => label,
                    Err(_) if function == "tag_add" && !wanted.trim().is_empty() => Tag::get(pool, &tag_named(pool, &self.runner.profile_id, wanted.trim()).await.map_err(|e| e.to_string())?).await?,
                    Err(e) => return Err(e),
                };
                if function == "tag_add" {
                    ObjectTag::put(pool, &object_type, &object, &label.id, "action", None, Some(&self.action_id)).await?;
                } else {
                    ObjectTag::delete(pool, &object_type, &object, &label.id).await?;
                }
                self.note("done", format!("{} {} on {} {}", function, label.name, object_type, object));
                if let Some(app) = &self.runner.app {
                    app.emit("object-tags-updated", &object_type).ok();
                    if object_type == OBJECT_CONVERSATION {
                        app.emit("sorted", &object).ok();
                    }
                }
                if object_type == OBJECT_CONVERSATION {
                    conversation_value(pool, &object).await
                } else {
                    Ok(if arg(0).is_object() { arg(0) } else { json!({ "id": object }) })
                }
            }
            "move" => {
                let conversation = self.conversation_id(&arg(0)).await?;
                let split = self.tag(&arg(1), Some(KIND_SPLIT)).await?;
                sqlx::query("DELETE FROM object_tag WHERE object_type = 'conversation' AND object_id = ?1 AND tag_id IN (SELECT id FROM tag WHERE kind = ?2)")
                    .bind(&conversation)
                    .bind(KIND_SPLIT)
                    .execute(pool)
                    .await
                    .map_err(|e| e.to_string())?;
                ObjectTag::put(pool, OBJECT_CONVERSATION, &conversation, &split.id, "action", None, Some(&self.action_id)).await?;
                self.note("done", format!("Moved {} to {}", conversation, split.name));
                if let Some(app) = &self.runner.app {
                    app.emit("sorted", &conversation).ok();
                }
                conversation_value(pool, &conversation).await
            }
            "snooze" | "done" => {
                let conversation = self.conversation_id(&arg(0)).await?;
                let now = Utc::now().to_rfc3339();
                if function == "snooze" {
                    let until = action_script::parse_date(&text(Some(&arg(1)))).ok_or("Snooze needs a time it can read, like tomorrow 08:00")?;
                    sqlx::query("UPDATE conversation SET snoozed_until = ?1, updated_at = ?2 WHERE id = ?3").bind(&until).bind(&now).bind(&conversation).execute(pool).await.map_err(|e| e.to_string())?;
                } else {
                    sqlx::query("UPDATE conversation SET is_archived = 1, updated_at = ?1 WHERE id = ?2").bind(&now).bind(&conversation).execute(pool).await.map_err(|e| e.to_string())?;
                }
                self.note("done", format!("{} {}", function, conversation));
                conversation_value(pool, &conversation).await
            }
            "post_status" => {
                let conversation = self.conversation_id(&arg(0)).await?;
                let options = arg(1);
                let title = text(options.get("title"));
                let state = match text(options.get("state")).as_str() {
                    s @ ("ok" | "warn" | "fail") => s.to_string(),
                    _ => "info".to_string(),
                };
                let body = text(options.get("text"));
                let now = Utc::now().to_rfc3339();
                let record = json!({ "kind": "status", "title": title, "text": body, "state": state });
                let (id, _) = chat_model::Chat::put_item(pool, &conversation, &uuid::Uuid::new_v4().to_string(), chat_model::ITEM_RECORD, &title, Some(&body), &record, &now, (Some(&self.action_id), None, None)).await?;
                if let Some(app) = &self.runner.app {
                    app.emit("chat-items-updated", &conversation).ok();
                }
                self.note("done", format!("Posted status {}: {}", state, title));
                Ok(json!({ "id": id, "state": state, "title": title }))
            }
            "notify" => {
                let title = text(Some(&arg(0)));
                if let Some(app) = &self.runner.app {
                    app.emit("action-notify", &json!({ "title": title, "body": text(Some(&arg(1))), "action_id": self.action_id })).ok();
                }
                self.note("done", format!("Notified: {}", title));
                Ok(Value::Bool(true))
            }
            "schedule_send" => {
                let conversation = self.conversation_id(&arg(0)).await?;
                let at = action_script::parse_date(&text(Some(&arg(2)))).ok_or("Send later needs a time it can read")?;
                let schedule = MessageSchedule::post(pool, &self.runner.profile_id, &PostMessageScheduleInput { conversation_id: conversation, provider_id: None, content: text(Some(&arg(1))), content_type: None, at: Some(at), action_id: None, metadata: None, attachments: None }).await?;
                self.note("done", format!("Scheduled for {}", schedule.at.clone().unwrap_or_default()));
                serde_json::to_value(schedule).map_err(|e| e.to_string())
            }
            "prompt" => {
                let key = text(Some(&arg(0)));
                let options = arg(1);
                let conversation = options.get("conversation").and_then(id_of);
                let mut extra = options.as_object().cloned().unwrap_or_default();
                extra.remove("conversation");
                if let Some(conversation) = &conversation {
                    let subject = crate::services::sorting_service::subject(pool, conversation).await?;
                    extra.insert("subject".into(), serde_json::to_value(subject).map_err(|e| e.to_string())?);
                }
                if let Some(message) = options.get("message").and_then(id_of) {
                    extra.insert("message".into(), prompt_message(pool, &message).await?);
                }
                if intelligence_service::seeds().iter().any(|s| s.key == key) {
                    return self.run_prompt(&key, conversation.as_deref(), Value::Object(extra)).await;
                }
                let item = ContentItem::get_reference(pool, &self.runner.profile_id, &key).await?.filter(|i| i.kind == KIND_PROMPT).ok_or_else(|| format!("There is no prompt, and no AI prompt in Content, called {}", key))?;
                self.ask("script_ai", Value::Null, Some(item), conversation.as_deref(), Value::Object(extra)).await
            }
            "content" => {
                let reference = text(Some(&arg(0)));
                let options = arg(1);
                let item = ContentItem::get_reference(pool, &self.runner.profile_id, &reference)
                    .await?
                    .filter(|i| is_template(&i.kind) && i.kind != KIND_PROMPT)
                    .ok_or_else(|| format!("There is no reply, email or snippet in Content called {}", reference))?;
                let mut values = options.as_object().cloned().unwrap_or_default();
                let conversation = values.remove("conversation").as_ref().and_then(id_of);
                let message = values.remove("message").as_ref().and_then(id_of);
                let input = PostContentRenderInput { content_id: Some(item.id), conversation_id: conversation, message_id: message, values: Some(Value::Object(values)), used: Some(!self.runner.dry_run), ..PostContentRenderInput::default() };
                Ok(Value::String(content_service::render(pool, &self.runner.profile_id, &input).await?.text))
            }
            "has_split" => Ok(Value::Bool(crate::services::sorting_service::has_split(pool, &self.conversation_id(&arg(0)).await?).await?)),
            "splits" => Ok(json!(Tag::all(pool, &self.runner.profile_id).await?.into_iter().filter(|l| l.kind == KIND_SPLIT).map(|l| l.name).collect::<Vec<_>>())),
            "present" | "highlight" | "collapse" | "hide" => {
                let message = id_of(&arg(0)).ok_or("Pass the message to present")?;
                let patch = match function {
                    "present" => {
                        let mut patch = arg(2).as_object().cloned().unwrap_or_default();
                        patch.insert("kind".into(), Value::String(text(Some(&arg(1)))));
                        patch.insert("template".into(), self.template_ref().await?);
                        Value::Object(patch)
                    }
                    "highlight" => json!({ "highlight": text(Some(&arg(1))) }),
                    "collapse" => json!({ "collapsed": true }),
                    _ => json!({ "kind": "hidden" }),
                };
                let shown = crate::models::presentation::put_message(pool, &message, &patch).await?;
                if let Some(app) = &self.runner.app {
                    app.emit("presentation-updated", &shown).ok();
                }
                self.note("done", format!("{} {}", function, message));
                Ok(shown.effective)
            }
            "present_conversation" => {
                let conversation = self.conversation_id(&arg(0)).await?;
                let defaults = crate::models::presentation::put_conversation(pool, &conversation, &arg(1)).await?;
                if let Some(app) = &self.runner.app {
                    app.emit("presentation-updated", &json!({ "conversation_id": conversation, "conversation": defaults })).ok();
                }
                self.note("done", format!("Presentation defaults for {}", conversation));
                Ok(defaults)
            }
            "style_conversation" => {
                let conversation = self.conversation_id(&arg(0)).await?;
                let key = format!("appearance.conversation.{}", conversation);
                let current: Value = crate::models::AppSetting::get(pool, &key).await?.and_then(|raw| serde_json::from_str(&raw).ok()).unwrap_or_else(|| json!({}));
                let wanted = arg(1);
                let mut next = current.as_object().cloned().unwrap_or_default();
                for field in ["surface", "background", "accent", "bubbleMine", "bubbleTheirs", "tint", "theme", "blur", "opacity"] {
                    match wanted.get(field) {
                        Some(Value::Null) => {
                            next.remove(field);
                        }
                        Some(value) => {
                            next.insert(field.to_string(), value.clone());
                        }
                        None => {}
                    }
                }
                let next = Value::Object(next);
                crate::models::AppSetting::put(pool, &key, &next.to_string()).await?;
                if let Some(app) = &self.runner.app {
                    app.emit("appearance-updated", &json!({ "key": key, "value": next })).ok();
                }
                self.note("done", format!("Styled {}", conversation));
                Ok(next)
            }
            "invite_details" => {
                let message = id_of(&arg(0)).ok_or("Pass the message to read")?;
                Ok(serde_json::to_value(invite_guess(pool, &message).await?).map_err(|e| e.to_string())?)
            }
            "store_summary" => {
                let message = id_of(&arg(0)).ok_or("Pass the message to keep the summary on")?;
                let line = intelligence_service::one_line(&text(Some(&arg(1))), 120);
                sqlx::query("UPDATE message SET summary = ?1 WHERE id = ?2").bind(&line).bind(&message).execute(pool).await.map_err(|e| e.to_string())?;
                self.note("done", format!("Summary kept: {}", line));
                Ok(Value::String(line))
            }
            "create_event" | "propose_event" => {
                let message = id_of(&arg(1));
                let conversation = match &message {
                    Some(id) => crate::models::Message::get_conversation_id(pool, id).await?,
                    None => None,
                };
                let made = crate::services::calendar_service::post_found_event(pool, &self.runner.profile_id, &arg(0), function == "create_event", message.as_deref(), conversation.as_deref()).await?;
                if let (Some(event), Some(app)) = (&made, &self.runner.app) {
                    app.emit("events-updated", &event.id).ok();
                }
                self.note("done", match &made {
                    Some(event) => format!("{} {}", if function == "create_event" { "Added" } else { "Proposed" }, event.title),
                    None => "Already in the calendar".to_string(),
                });
                serde_json::to_value(made).map_err(|e| e.to_string())
            }
            "mark_person" => {
                let person = id_of(&arg(0)).ok_or("Pass a person")?;
                let fields = arg(1);
                let input = crate::models::PutPersonInput { person_id: person.clone(), display_name: fields.get("name").and_then(|v| v.as_str()).map(String::from), is_human: fields.get("human").and_then(|v| v.as_bool()), favourite_position: None };
                crate::models::Person::put(pool, &input).await?;
                self.note("done", format!("Updated person {}", person));
                person_value(pool, &self.runner.profile_id, &person).await
            }
            "mute" | "pin" | "mark_read" => {
                let conversation = self.conversation_id(&arg(0)).await?;
                let now = Utc::now().to_rfc3339();
                let statements: &[&str] = match function {
                    "mute" => &["UPDATE conversation SET is_muted = 1, updated_at = ?1 WHERE id = ?2"],
                    "pin" => &["UPDATE conversation SET is_pinned = 1, updated_at = ?1 WHERE id = ?2"],
                    _ => &["UPDATE message SET is_read = 1 WHERE conversation_id = ?2 AND ?1 IS NOT NULL", "UPDATE conversation SET unread_count = 0, updated_at = ?1 WHERE id = ?2"],
                };
                for statement in statements {
                    sqlx::query(statement).bind(&now).bind(&conversation).execute(pool).await.map_err(|e| e.to_string())?;
                }
                self.note("done", format!("{} {}", function, conversation));
                conversation_value(pool, &conversation).await
            }
            "group_add" => {
                let conversation = self.conversation_id(&arg(0)).await?;
                let wanted = text(Some(&arg(1)));
                let group = Group::all(pool, &self.runner.profile_id).await?.into_iter().find(|g| g.id == wanted || g.name.eq_ignore_ascii_case(&wanted)).ok_or_else(|| format!("There is no group called {}", wanted))?;
                Group::add_items(pool, &group.id, &[conversation.clone()]).await?;
                self.note("done", format!("Added {} to {}", conversation, group.name));
                conversation_value(pool, &conversation).await
            }
            "receipt_extract" => {
                let message = id_of(&arg(0)).ok_or("Pass the message to read")?;
                let material = receipt_service::material(pool, &self.runner.app_data, &self.runner.cache, &message).await?;
                self.note("done", format!("Body: {} characters", text(material.get("body")).chars().count()));
                for file in material["files"].as_array().into_iter().flatten() {
                    let name = text(file.get("name"));
                    let method = text(file.get("method"));
                    self.note(
                        if file["error"].is_null() { "done" } else { "skipped" },
                        match file["error"].as_str() {
                            Some(error) => format!("{}: {}", name, error),
                            None => format!("{}: {} characters by {}", name, file["chars"].as_u64().unwrap_or(0), method),
                        },
                    );
                }
                Ok(material)
            }
            "receipt_match" => {
                let patterns = receipt_service::patterns_from(&arg(1))?;
                let threshold = match arg(2).as_f64() {
                    Some(threshold) => threshold,
                    None => AppSetting::get_f64(pool, app_setting::RECEIPTS_THRESHOLD).await?,
                };
                let found = receipt_service::candidates(&arg(0), &patterns, threshold)?;
                self.note("done", format!("{} candidates scored {} or more with {} patterns", found.len(), threshold, patterns.len()));
                Ok(Value::Array(found))
            }
            "receipt_confirm" => {
                let mut overrides = self.prompts.lock().map(|p| p.get(receipt_service::PROMPT_KEY).cloned().unwrap_or(Value::Null)).unwrap_or(Value::Null);
                if let Some(id) = overrides.get("content_id").and_then(Value::as_str).map(String::from) {
                    let item = content_service::owned(pool, &self.runner.profile_id, &id).await.map_err(|_| format!("The AI prompt this node takes from Content is gone ({})", id))?;
                    overrides["template"] = Value::String(item.body.unwrap_or_default());
                }
                let candidates = arg(0).as_array().cloned().unwrap_or_default();
                let batch = match arg(1).as_u64() {
                    Some(batch) => batch as usize,
                    None => AppSetting::get_u64(pool, app_setting::RECEIPTS_BATCH).await? as usize,
                };
                let (confirmed, log) = receipt_service::confirm(pool, &self.runner.profile_id, candidates, batch, &overrides, Some(&self.ai), self.runner.ai.clone()).await;
                for line in log {
                    self.note("done", line);
                }
                Ok(Value::Array(confirmed))
            }
            "receipt_organise" => {
                let records = receipt_service::organise(&arg(0).as_array().cloned().unwrap_or_default());
                self.note("done", format!("{} receipts organised", records.len()));
                Ok(Value::Array(records))
            }
            other => Err(format!("{} is not a known function", other)),
        }
    }
}

pub async fn invite_guess(pool: &Pool<Sqlite>, message_id: &str) -> Result<crate::content::calendar::InviteGuess, String> {
    let row = sqlx::query("SELECT subject, content, content_type, metadata, sender_name, sender_email FROM message WHERE id = ?1")
        .bind(message_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to read the message: {}", e))?
        .ok_or_else(|| format!("No message with id {}", message_id))?;
    let subject: String = row.try_get::<Option<String>, _>("subject").ok().flatten().unwrap_or_default();
    let content: String = row.try_get::<Option<String>, _>("content").ok().flatten().unwrap_or_default();
    let content_type: String = row.try_get::<Option<String>, _>("content_type").ok().flatten().unwrap_or_else(|| "text".into());
    let metadata: Value = row.try_get::<Option<String>, _>("metadata").ok().flatten().and_then(|m| serde_json::from_str(&m).ok()).unwrap_or(Value::Null);
    let plain = crate::utils::text_extraction::extract_text_from_content(&content, &content_type);
    let material = format!("{}\n{}", plain, content);
    let mut guess = crate::content::calendar::recognise_invite(&subject, &material, metadata.get("event"));
    if guess.organizer.is_empty() {
        guess.organizer = row.try_get::<Option<String>, _>("sender_name").ok().flatten().or_else(|| row.try_get::<Option<String>, _>("sender_email").ok().flatten()).unwrap_or_default();
    }
    Ok(guess)
}

pub async fn recognise_invites(pool: &Pool<Sqlite>, profile_id: &str) -> Result<usize, String> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT m.id FROM message m JOIN provider p ON p.id = m.provider_id WHERE p.profile_id = ?1 AND m.type = 'email' AND (m.presentation IS NULL OR m.presentation NOT LIKE '%\"kind\"%')",
    )
    .bind(profile_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to list mail: {}", e))?;
    let mut marked = 0;
    for id in ids {
        let guess = invite_guess(pool, &id).await?;
        if guess.confidence >= 0.85 {
            crate::models::presentation::put_message(pool, &id, &json!({ "kind": "invite", "invite": guess })).await?;
            marked += 1;
        }
    }
    Ok(marked)
}

pub async fn message_value(pool: &Pool<Sqlite>, id: &str) -> Result<Value, String> {
    let row = sqlx::query("SELECT id, conversation_id, type, sender_id, sender_name, sender_email, subject, content, content_type, timestamp, is_read, has_attachments, summary, metadata FROM message WHERE id = ?1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Message {} not found", id))?;
    let content: Option<String> = row.get("content");
    let content_type: Option<String> = row.get("content_type");
    let body = crate::utils::text_extraction::extract_text_from_content(content.as_deref().unwrap_or(""), content_type.as_deref().unwrap_or("text"));
    let email = row.try_get::<Option<String>, _>("sender_email").ok().flatten().unwrap_or_default().to_lowercase();
    let metadata: Value = row.try_get::<Option<String>, _>("metadata").ok().flatten().and_then(|m| serde_json::from_str(&m).ok()).unwrap_or(Value::Null);
    let latest = match &metadata["reactions"] {
        Value::Array(list) => list.iter().filter(|r| r["at"].is_string()).max_by_key(|r| text(r.get("at"))).and_then(|r| r["emoji"].as_str()).map(String::from),
        Value::Object(all) => all.iter().max_by_key(|(_, v)| text(v.get("timestamp"))).map(|(k, _)| k.clone()),
        _ => None,
    }
    .unwrap_or_default();
    let reaction = crate::content::emoji::unicode(&crate::content::emoji::reaction_name(&latest)).map(String::from).unwrap_or(latest);
    Ok(json!({
        "id": row.get::<String, _>("id"),
        "conversation_id": row.try_get::<Option<String>, _>("conversation_id").ok().flatten(),
        "kind": row.try_get::<Option<String>, _>("type").ok().flatten().unwrap_or_default(),
        "reaction": reaction,
        "sender": row.try_get::<Option<String>, _>("sender_name").ok().flatten().unwrap_or_default(),
        "sender_id": row.try_get::<Option<String>, _>("sender_id").ok().flatten(),
        "sender_email": email,
        "sender_domain": email.rsplit('@').next().unwrap_or("").to_string(),
        "subject": row.try_get::<Option<String>, _>("subject").ok().flatten().unwrap_or_default(),
        "text": body.chars().take(8000).collect::<String>(),
        "at": row.get::<String, _>("timestamp"),
        "unread": !row.try_get::<bool, _>("is_read").unwrap_or(true),
        "has_attachments": row.try_get::<bool, _>("has_attachments").unwrap_or(false),
        "summary": row.try_get::<Option<String>, _>("summary").ok().flatten().unwrap_or_default(),
    }))
}

pub async fn prompt_message(pool: &Pool<Sqlite>, id: &str) -> Result<Value, String> {
    let row = sqlx::query(
        "SELECT m.subject, m.content, m.content_type, m.sender_name, m.sender_email, m.timestamp, c.name AS conversation_name
         FROM message m LEFT JOIN conversation c ON c.id = m.conversation_id WHERE m.id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("Message {} not found", id))?;
    let raw: Option<String> = row.get("content");
    let content_type: Option<String> = row.get("content_type");
    let raw = raw.unwrap_or_default();
    let text = if matches!(content_type.as_deref(), Some("html") | Some("text/html")) {
        let (body, _) = crate::content::html::split_quoted(&raw);
        let (body, _) = crate::content::html::split_signature(&body);
        crate::content::html::visible_text(&body)
    } else {
        let (body, _) = crate::content::mail::split_quoted_text(&raw);
        crate::content::mail::split_signature_text(&body).0
    };
    let sent: String = row.get("timestamp");
    let local = DateTime::parse_from_rfc3339(&sent).map(|d| d.with_timezone(&Local)).unwrap_or_else(|_| Local::now());
    Ok(json!({
        "id": id,
        "tz_offset": local.format("%:z").to_string(),
        "date": local.format("%A %Y-%m-%d %H:%M").to_string(),
        "conversation_name": row.try_get::<Option<String>, _>("conversation_name").ok().flatten().unwrap_or_default(),
        "from_name": row.try_get::<Option<String>, _>("sender_name").ok().flatten().unwrap_or_default(),
        "from_email": row.try_get::<Option<String>, _>("sender_email").ok().flatten().unwrap_or_default(),
        "subject": row.try_get::<Option<String>, _>("subject").ok().flatten().unwrap_or_default(),
        "text": text.chars().take(6000).collect::<String>(),
    }))
}

pub async fn conversation_value(pool: &Pool<Sqlite>, id: &str) -> Result<Value, String> {
    let row = sqlx::query(
        "SELECT c.id, c.name, c.type, c.namespace, c.person_id, c.is_archived, c.unread_count,
            (SELECT COUNT(*) FROM conversation_member WHERE conversation_id = c.id) AS members,
            p.provider_type, p.name AS connection, p.id AS provider_id
         FROM conversation c
         LEFT JOIN conversation_provider cp ON cp.conversation_id = c.id AND cp.is_primary = 1
         LEFT JOIN provider p ON p.id = cp.provider_id WHERE c.id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("Conversation {} not found", id))?;
    let tag_ids: Vec<String> = ObjectTag::get_conversation(pool, id).await?.into_iter().map(|t| t.tag_id).collect();
    let mut labels: Vec<String> = Vec::new();
    for tag_id in &tag_ids {
        labels.push(Tag::get(pool, tag_id).await?.name);
    }
    Ok(json!({
        "id": row.get::<String, _>("id"),
        "name": row.get::<String, _>("name"),
        "kind": row.get::<String, _>("type"),
        "provider": row.try_get::<Option<String>, _>("provider_type").ok().flatten().unwrap_or_default(),
        "provider_id": row.try_get::<Option<String>, _>("provider_id").ok().flatten(),
        "connection": row.try_get::<Option<String>, _>("connection").ok().flatten().unwrap_or_default(),
        "namespace": row.try_get::<Option<String>, _>("namespace").ok().flatten().unwrap_or_default(),
        "person_id": row.try_get::<Option<String>, _>("person_id").ok().flatten(),
        "members": row.try_get::<i64, _>("members").unwrap_or(0),
        "done": row.try_get::<bool, _>("is_archived").unwrap_or(false),
        "unread": row.try_get::<i64, _>("unread_count").unwrap_or(0),
        "tags": labels,
        "tag_ids": tag_ids,
    }))
}

pub async fn person_value(pool: &Pool<Sqlite>, profile_id: &str, id: &str) -> Result<Value, String> {
    let person = crate::models::Person::get(pool, profile_id, id).await?;
    let emails: Vec<String> = person.identities.iter().filter_map(|i| i.email.clone()).collect();
    Ok(json!({ "id": person.id, "name": person.display_name, "emails": emails, "human": person.is_human, "conversation_ids": person.conversation_ids, "identities": person.identities.iter().map(|i| json!({ "provider": i.provider_type, "email": i.email, "name": i.display_name })).collect::<Vec<_>>() }))
}

pub fn mentions(content: &str, me: &[String]) -> bool {
    let hay = content.to_lowercase();
    me.iter().map(|m| m.trim().to_lowercase()).filter(|m| m.len() > 1).any(|m| hay.contains(&m))
}

pub async fn payload_for_message(pool: &Pool<Sqlite>, profile_id: &str, message_id: &str) -> Result<Value, String> {
    let mut message = message_value(pool, message_id).await?;
    let mine = Profile::get_own_user_ids(pool, profile_id).await?.contains(&text(message.get("sender_id")));
    message["mine"] = Value::Bool(mine);
    let me: Vec<String> = sqlx::query_scalar(
        "SELECT pu.external_id FROM provider_user pu JOIN provider p ON p.id = pu.provider_id WHERE p.profile_id = ?1 AND pu.external_id = COALESCE(p.user_external_id, p.external_id)
         UNION SELECT pu.user_id FROM provider_user pu JOIN provider p ON p.id = pu.provider_id WHERE p.profile_id = ?1 AND pu.external_id = COALESCE(p.user_external_id, p.external_id)
         UNION SELECT user_id FROM profile WHERE id = ?1 AND user_id IS NOT NULL
         UNION SELECT '@' || name FROM profile WHERE id = ?1",
    )
    .bind(profile_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let raw: String = sqlx::query_scalar("SELECT COALESCE(content, '') FROM message WHERE id = ?1").bind(message_id).fetch_one(pool).await.map_err(|e| e.to_string())?;
    message["mentioned"] = Value::Bool(mentions(&raw, &me));
    let conversation_id = text(message.get("conversation_id"));
    let conversation = conversation_value(pool, &conversation_id).await?;
    let person = match conversation.get("person_id").and_then(|p| p.as_str()) {
        Some(person_id) => person_value(pool, profile_id, person_id).await.unwrap_or(Value::Null),
        None => Value::Null,
    };
    let text_value = format!("{}\n{}", text(message.get("subject")), text(message.get("text"))).trim().to_string();
    let reaction = message["reaction"].clone();
    Ok(json!({ "message": message, "conversation": conversation, "person": person, "text": text_value, "reaction": reaction }))
}

fn facts(payload: &Value) -> QueryFacts {
    let message = &payload["message"];
    let conversation = &payload["conversation"];
    QueryFacts {
        conversation_id: text(conversation.get("id")),
        provider_type: text(conversation.get("provider")),
        provider_id: text(conversation.get("provider_id")),
        conversation_type: text(conversation.get("kind")),
        tag_ids: conversation["tag_ids"].as_array().map(|a| a.iter().map(|v| text(Some(v))).collect()).unwrap_or_default(),
        person_id: conversation.get("person_id").and_then(|p| p.as_str()).map(String::from),
        unread: message["unread"].as_bool().unwrap_or(false),
        has_attachments: message["has_attachments"].as_bool().unwrap_or(false),
        human: payload["person"]["human"].as_bool().unwrap_or(true),
        sender: format!("{} {}", text(message.get("sender")), text(message.get("sender_email"))),
        text: text(payload.get("text")),
        at: text(message.get("at")),
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
struct State {
    order: Vec<String>,
    cursor: usize,
    produced: HashMap<String, Map<String, Value>>,
    dead: Vec<String>,
    payload: Value,
    inputs: Map<String, Value>,
    trigger: Option<String>,
    #[serde(default)]
    chat: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Start {
    Trigger { node: String, payload: Value },
    Invoke { inputs: Map<String, Value>, payload: Value },
    Chat { conversation_id: String, mode: ChatMode, payload: Value },
}

fn new_run(action: &Action, dry_run: bool, depth: i64, trigger: Value) -> ActionRun {
    ActionRun {
        id: Uuid::new_v4().to_string(),
        action_id: action.id.clone(),
        action_name: action.name.clone(),
        status: "running".into(),
        dry_run,
        trigger,
        log: Vec::new(),
        outputs: Value::Object(Map::new()),
        state: Value::Null,
        error: None,
        resume_at: None,
        depth,
        started_at: Utc::now().to_rfc3339(),
        finished_at: None,
        result: Value::Null,
        requirement: None,
    }
}

pub fn result_schema() -> Value {
    json!({
        "type": "object",
        "required": ["kind", "message", "commands", "content", "outputs"],
        "properties": {
            "kind": { "type": "string", "enum": ["command", "content", "data", "none"], "description": "command when the run changed something, content when it produced text to show or send, data when it only returned values, none when nothing applied" },
            "message": { "type": "string", "description": "one sentence on what the run did" },
            "commands": { "type": "array", "items": { "type": "object", "required": ["function", "detail", "status"], "properties": { "function": { "type": "string" }, "detail": { "type": "string" }, "status": { "type": "string", "enum": ["done", "would"] } } } },
            "content": { "type": ["object", "null"], "properties": { "text": { "type": "string" } } },
            "outputs": { "type": "object" }
        }
    })
}

pub fn result_of(run: &ActionRun) -> Value {
    let commands: Vec<Value> = run
        .log
        .iter()
        .filter(|l| l.kind == "effect")
        .map(|l| json!({ "function": l.detail.split('(').next().unwrap_or("").trim(), "detail": l.detail, "status": l.status }))
        .collect();
    let outputs = run.outputs.as_object().cloned().unwrap_or_default();
    let text = ["reply", "text", "summary", "body", "message", "line", "draft", "title"].iter().find_map(|key| outputs.get(*key).and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(String::from));
    let kind = if run.error.is_some() { "none" } else if text.is_some() { "content" } else if !commands.is_empty() { "command" } else if !outputs.is_empty() { "data" } else { "none" };
    let skipped: Vec<&str> = run.log.iter().filter(|l| l.status == "skipped").map(|l| l.detail.as_str()).collect();
    let message = match (&run.error, &text) {
        (Some(error), _) => error.clone(),
        (None, Some(text)) => text.clone(),
        (None, None) if !commands.is_empty() => {
            let names: Vec<&str> = commands.iter().filter_map(|c| c["function"].as_str()).collect();
            format!("{} effect{}: {}", commands.len(), if commands.len() == 1 { "" } else { "s" }, names.join(", "))
        }
        (None, None) if !outputs.is_empty() => outputs.iter().map(|(k, v)| format!("{}: {}", k, match v { Value::String(s) => s.clone(), Value::Array(a) => format!("{} items", a.len()), other => other.to_string() })).collect::<Vec<_>>().join(" · "),
        (None, None) if !skipped.is_empty() => format!("Nothing to do: {}", skipped[0]),
        (None, None) => "Nothing to do".to_string(),
    };
    json!({ "kind": kind, "message": message, "commands": commands, "content": text.map(|t| json!({ "text": t })), "outputs": run.outputs })
}

pub async fn isolate_node(pool: &Pool<Sqlite>, profile_id: &str, action: &Action, node_id: &str) -> Result<Action, String> {
    let node = action.graph.nodes.iter().find(|n| n.id == node_id).ok_or_else(|| format!("{} has no node {}", action.name, node_id))?;
    if node.kind == NODE_TRIGGER {
        return Err("A trigger has nothing to run on its own; run the whole action instead".to_string());
    }
    let actions = others(pool, profile_id).await?;
    let (inputs, _) = node_ports(node, &actions, Some(&action.inputs)).ok_or_else(|| format!("{} is not a known node type", node.kind))?;
    let mut single = action.clone();
    single.inputs = inputs;
    single.graph = Graph { nodes: vec![node.clone()], edges: Vec::new() };
    single.outputs = signature(&single.graph, &actions);
    Ok(single)
}

pub fn dry_start(action: &Action, inputs: Map<String, Value>, payload: Value) -> Start {
    let triggers = action.triggers();
    if triggers.is_empty() || triggers.iter().any(|(_, t)| t.on.is_invocable()) {
        return Start::Invoke { inputs, payload };
    }
    let mut merged = payload.as_object().cloned().unwrap_or_default();
    merged.extend(inputs);
    Start::Trigger { node: triggers[0].0.id.clone(), payload: Value::Object(merged) }
}

pub async fn sample(pool: &Pool<Sqlite>, profile_id: &str, message_id: Option<&str>, person_id: Option<&str>) -> Result<Value, String> {
    let message: Option<String> = match message_id {
        Some(id) => Some(id.to_string()),
        None => sqlx::query_scalar(
            "SELECT m.id FROM message m
             JOIN conversation c ON c.id = m.conversation_id
             JOIN conversation_provider cp ON cp.conversation_id = c.id AND cp.is_primary = 1
             JOIN provider p ON p.id = cp.provider_id
             WHERE p.profile_id = ?1 AND p.provider_type = 'mock'
             ORDER BY c.name, m.timestamp, m.id LIMIT 1",
        )
        .bind(profile_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to read a sample message: {}", e))?,
    };
    let Some(message) = message else { return Ok(Value::Null) };
    let mut out = payload_for_message(pool, profile_id, &message).await?;
    if let Some(person) = person_id {
        out["person"] = person_value(pool, profile_id, person).await?;
    }
    let tag: Option<String> = sqlx::query_scalar("SELECT name FROM tag WHERE profile_id = ?1 AND kind = 'tag' ORDER BY position, name LIMIT 1")
        .bind(profile_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to read a sample tag: {}", e))?;
    out["tag"] = tag.map(Value::String).unwrap_or(Value::Null);
    Ok(out)
}

pub fn execute<'a>(runner: &'a Runner, action: &'a Action, start: Start, depth: i64, stack: Vec<String>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ActionRun, String>> + Send + 'a>> {
    Box::pin(async move {
        let (state, run) = match start {
            Start::Trigger { node, payload } => {
                let run = new_run(action, runner.dry_run, depth, json!({ "trigger": node, "payload": payload }));
                (State { order: topo_order(&action.graph)?, payload, trigger: Some(node), ..Default::default() }, run)
            }
            Start::Invoke { inputs, payload } => {
                let run = new_run(action, runner.dry_run, depth, json!({ "inputs": inputs, "payload": payload }));
                (State { order: topo_order(&action.graph)?, payload, inputs, ..Default::default() }, run)
            }
            Start::Chat { conversation_id, mode, payload } => {
                let mut run = new_run(action, runner.dry_run, depth, json!({ "chat": conversation_id, "mode": mode.name(), "payload": payload }));
                let name = crate::models::Conversation::get_name(&runner.pool, &conversation_id).await?;
                let variant = chat_model::node_spec().variant(mode);
                run.log.push(LogEntry { node: NODE_CHAT.into(), kind: NODE_CHAT.into(), status: "done".into(), detail: format!("{} {} started it", variant.label, name.unwrap_or(conversation_id.clone())) });
                (State { order: topo_order(&action.graph)?, payload, chat: Some(conversation_id), ..Default::default() }, run)
            }
        };
        proceed(runner, action, state, run, stack).await
    })
}

pub async fn resume(runner: &Runner, action: &Action, mut run: ActionRun) -> Result<ActionRun, String> {
    let state: State = serde_json::from_value(run.state.clone()).map_err(|e| format!("The paused run cannot be read: {}", e))?;
    run.status = "running".into();
    run.resume_at = None;
    proceed(runner, action, state, run, vec![action.id.clone()]).await
}

async fn proceed(runner: &Runner, action: &Action, mut state: State, mut run: ActionRun, stack: Vec<String>) -> Result<ActionRun, String> {
    let actions = others(&runner.pool, &runner.profile_id).await?;
    let effects = Arc::new(RunEffects::new(runner, action, None));
    let nodes: HashMap<&str, &Node> = action.graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut failure = None;
    while state.cursor < state.order.len() {
        let node_id = state.order[state.cursor].clone();
        state.cursor += 1;
        let Some(node) = nodes.get(node_id.as_str()) else { continue };
        *effects.node.lock().map_err(|_| "locked")? = node.id.clone();
        *effects.prompts.lock().map_err(|_| "locked")? = node.config.get("prompts").cloned().unwrap_or(Value::Null);
        let incoming: Vec<&Edge> = action.graph.edges.iter().filter(|e| e.target == node.id).collect();
        if let Some(config) = node.chat() {
            let fixed = config.conversation_id.clone().filter(|id| !id.is_empty());
            if incoming.is_empty() {
                let fired = match &state.trigger {
                    Some(t) => t == &node.id,
                    None => state.chat.is_some() && (fixed.is_none() || fixed == state.chat),
                };
                if fired {
                    let mut out = state.payload.as_object().cloned().unwrap_or_default();
                    out.extend(state.inputs.clone());
                    state.produced.insert(node.id.clone(), out);
                    run.log.push(LogEntry { node: node.id.clone(), kind: NODE_CHAT.into(), status: "done".into(), detail: format!("{} fired", node_label(node)) });
                } else {
                    state.dead.push(node.id.clone());
                }
                continue;
            }
            if !incoming.iter().any(|e| edge_live(&state, e)) {
                state.dead.push(node.id.clone());
                run.log.push(LogEntry { node: node.id.clone(), kind: NODE_CHAT.into(), status: "skipped".into(), detail: format!("{} did not run; nothing reached it", node_label(node)) });
                continue;
            }
            let wired = |port: &str| incoming.iter().filter(|e| e.target_port == port && edge_live(&state, e)).filter_map(|e| state.produced.get(&e.source).and_then(|p| p.get(&e.source_port))).last().cloned();
            let Some(target) = fixed.or_else(|| state.chat.clone()) else {
                run.log.push(LogEntry { node: node.id.clone(), kind: NODE_CHAT.into(), status: "failed".into(), detail: "No chat to deliver to".into() });
                failure = Some(format!("{}: choose the chat it delivers to, or attach the action to a chat", node_label(node)));
                break;
            };
            let outcome = match config.mode {
                ChatMode::Read => {
                    let mut records: Vec<Value> = wired("items").and_then(|v| v.as_array().cloned()).unwrap_or_default();
                    if let Some(item) = wired("item").filter(|v| !v.is_null()) {
                        records.push(item);
                    }
                    if runner.dry_run {
                        run.log.push(LogEntry { node: node.id.clone(), kind: NODE_CHAT.into(), status: "would".into(), detail: format!("Deliver {} records to {}", records.len(), target) });
                        Ok(())
                    } else {
                        crate::services::chat_service::deliver(runner, &target, &action.id, &run.id, records, run.depth + 1).await.map(|count| {
                            run.log.push(LogEntry { node: node.id.clone(), kind: NODE_CHAT.into(), status: "done".into(), detail: format!("Delivered {} records to {}", count, target) });
                        })
                    }
                }
                ChatMode::Write => {
                    let content = text(wired("text").as_ref());
                    effects.outgoing(&target, None, &content, None).await.map(|_| ())
                }
            };
            run.log.extend(effects.log.lock().map(|mut l| std::mem::take(&mut *l)).unwrap_or_default());
            match outcome {
                Ok(()) => {
                    state.produced.insert(node.id.clone(), Map::new());
                }
                Err(e) => {
                    run.log.push(LogEntry { node: node.id.clone(), kind: NODE_CHAT.into(), status: "failed".into(), detail: e.clone() });
                    failure = Some(format!("{}: {}", node_label(node), e));
                    break;
                }
            }
            continue;
        }
        if let Some(trigger) = node.trigger() {
            let fired = match &state.trigger {
                Some(t) => t == &node.id,
                None => trigger.on.is_invocable(),
            };
            if fired {
                let mut out = state.payload.as_object().cloned().unwrap_or_default();
                out.extend(state.inputs.clone());
                state.produced.insert(node.id.clone(), out);
                run.log.push(LogEntry { node: node.id.clone(), kind: NODE_TRIGGER.into(), status: "done".into(), detail: format!("{} fired", node_label(node)) });
            } else {
                state.dead.push(node.id.clone());
            }
            continue;
        }
        if !incoming.is_empty() && !incoming.iter().any(|e| edge_live(&state, e)) {
            state.dead.push(node.id.clone());
            run.log.push(LogEntry { node: node.id.clone(), kind: node.kind.clone(), status: "skipped".into(), detail: format!("{} did not run; nothing reached it", node_label(node)) });
            continue;
        }
        let Some((inputs, outputs)) = node_ports(node, &actions, Some(&action.inputs)) else {
            failure = Some(format!("{} is not a known node type", node.kind));
            break;
        };
        let mut values = Map::new();
        for port in &inputs {
            let wired = incoming.iter().filter(|e| e.target_port == port.name && edge_live(&state, e)).filter_map(|e| state.produced.get(&e.source).and_then(|p| p.get(&e.source_port))).last().cloned();
            let value = wired
                .or_else(|| node.constant(&port.name))
                .or_else(|| state.inputs.get(&port.name).cloned())
                .or_else(|| state.payload.get(&port.name).cloned())
                .unwrap_or(Value::Null);
            values.insert(port.name.clone(), value);
        }
        if node.kind == format!("{}wait", BUILTIN_PREFIX) {
            let minutes = values.get("minutes").and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0);
            state.produced.insert(node.id.clone(), Map::from_iter([("value".to_string(), values.get("value").cloned().unwrap_or(Value::Null))]));
            if runner.dry_run || minutes == 0.0 {
                if let Some(delay) = runner.test_delay.filter(|_| minutes > 0.0) {
                    tokio::time::sleep(delay).await;
                    run.log.push(LogEntry { node: node.id.clone(), kind: node.kind.clone(), status: "done".into(), detail: format!("Waited {} seconds in the test instead of {} minutes", delay.as_secs(), minutes) });
                    continue;
                }
                run.log.push(LogEntry { node: node.id.clone(), kind: node.kind.clone(), status: if runner.dry_run { "would" } else { "done" }.into(), detail: format!("Wait {} minutes", minutes) });
                continue;
            }
            run.status = "waiting".into();
            run.resume_at = Some((Utc::now() + Duration::seconds((minutes * 60.0) as i64)).to_rfc3339());
            run.log.push(LogEntry { node: node.id.clone(), kind: node.kind.clone(), status: "waiting".into(), detail: format!("Paused for {} minutes", minutes) });
            run.state = serde_json::to_value(&state).map_err(|e| e.to_string())?;
            run.log.extend(effects.log.lock().map(|l| l.clone()).unwrap_or_default());
            run.result = result_of(&run);
            return Ok(run);
        }
        let began = std::time::Instant::now();
        let result = if let Some(child_id) = node.kind.strip_prefix(ACTION_PREFIX) {
            match actions.get(child_id) {
                Some(child) if stack.contains(&child.id) || run.depth + 1 > MAX_DEPTH => Err(format!("{} would call itself or nest too deep", child.name)),
                Some(child) => {
                    let mut child_stack = stack.clone();
                    child_stack.push(child.id.clone());
                    let child_run = execute(runner, child, Start::Invoke { inputs: values.clone(), payload: state.payload.clone() }, run.depth + 1, child_stack).await?;
                    run.log.extend(child_run.log.iter().map(|l| LogEntry { node: format!("{}/{}", node.id, l.node), ..l.clone() }));
                    match child_run.status.as_str() {
                        "failed" => Err(child_run.error.unwrap_or_else(|| format!("{} failed", child.name))),
                        _ => Ok(child_run.outputs),
                    }
                }
                None => Err(format!("The action {} used here no longer exists", child_id)),
            }
        } else {
            action_script::run_script(&node.body, &values, effects.clone()).await.map(|outcome| {
                for line in outcome.logs {
                    run.log.push(LogEntry { node: node.id.clone(), kind: "log".into(), status: "done".into(), detail: line });
                }
                outcome.value
            })
        };
        run.log.extend(effects.log.lock().map(|mut l| std::mem::take(&mut *l)).unwrap_or_default());
        match result {
            Ok(value) => {
                let produced = match value {
                    Value::Object(map) => map,
                    Value::Null => Map::new(),
                    other if outputs.len() == 1 => Map::from_iter([(outputs[0].name.clone(), other)]),
                    other => Map::from_iter([("value".to_string(), other)]),
                };
                let names: Vec<&String> = produced.keys().collect();
                run.log.push(LogEntry { node: node.id.clone(), kind: node.kind.clone(), status: "done".into(), detail: if names.is_empty() { node_label(node) } else { format!("{} gave {}", node_label(node), names.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")) } });
                run.log.push(step(node, &values, Value::Object(produced.clone()), "done", began));
                state.produced.insert(node.id.clone(), produced);
            }
            Err(e) => {
                let (status, detail) = match action_script::unmet_of(&e) {
                    Some(needed) => (action_script::REQUIREMENT, needed.message),
                    None => ("failed", e.clone()),
                };
                run.log.push(LogEntry { node: node.id.clone(), kind: node.kind.clone(), status: status.into(), detail });
                run.log.push(step(node, &values, Value::Null, status, began));
                failure = Some(format!("{}: {}", node_label(node), e));
                break;
            }
        }
    }
    let mut outputs = Map::new();
    for port in signature(&action.graph, &actions) {
        if let Some(value) = port.node.as_ref().zip(port.source_port.as_ref()).and_then(|(node, source)| state.produced.get(node).and_then(|p| p.get(source))) {
            outputs.insert(port.name, value.clone());
        }
    }
    run.outputs = Value::Object(outputs);
    run.status = if failure.is_some() { "failed".into() } else { "done".into() };
    run.requirement = failure.as_deref().and_then(action_script::unmet_of);
    run.error = failure;
    run.state = Value::Null;
    run.finished_at = Some(Utc::now().to_rfc3339());
    run.result = result_of(&run);
    Ok(run)
}

pub const LOG_STEP: &str = "step";

fn step(node: &Node, inputs: &Map<String, Value>, outputs: Value, status: &str, began: std::time::Instant) -> LogEntry {
    LogEntry { node: node.id.clone(), kind: LOG_STEP.into(), status: status.into(), detail: json!({ "inputs": inputs, "outputs": outputs, "ms": began.elapsed().as_millis() as u64 }).to_string() }
}

fn edge_live(state: &State, edge: &Edge) -> bool {
    !state.dead.contains(&edge.source) && state.produced.get(&edge.source).is_some_and(|p| p.contains_key(&edge.source_port))
}

fn node_label(node: &Node) -> String {
    if !node.label.is_empty() {
        return node.label.clone();
    }
    if let Some(config) = node.chat() {
        return chat_model::node_spec().variant(config.mode).label.clone();
    }
    node.kind.strip_prefix(BUILTIN_PREFIX).and_then(builtin).map(|b| b.label.to_string()).unwrap_or_else(|| node.id.clone())
}

async fn predicate(runner: &Runner, action: &Action, node: &Node, payload: &Value) -> Result<bool, String> {
    if node.body.trim().is_empty() {
        return Ok(true);
    }
    let effects = Arc::new(RunEffects::new(runner, action, Some(node)));
    let inputs = payload.as_object().cloned().unwrap_or_default();
    Ok(action_script::truthy(&action_script::run_script(&node.body, &inputs, effects).await?.value))
}

pub async fn enabled(pool: &Pool<Sqlite>) -> bool {
    AppSetting::get_or(pool, app_setting::ACTIONS_ENABLED, "true").await.map(|v| v != "false").unwrap_or(true)
}

pub fn passes(trigger: &TriggerConfig, payload: &Value) -> bool {
    let message = &payload["message"];
    let mine = message["mine"].as_bool().unwrap_or(false);
    let direction = match trigger.direction() {
        DIRECTION_OUTGOING => mine,
        DIRECTION_ANY => true,
        _ => !mine,
    };
    let kind = text(message.get("kind"));
    let kinds = trigger.message_kinds.is_empty() || trigger.message_kinds.iter().any(|k| k.eq_ignore_ascii_case(&kind));
    let mention = trigger.on != TriggerOn::Mention || message["mentioned"].as_bool().unwrap_or(false);
    direction && kinds && mention && trigger.scope.admits(&facts(payload))
}

async fn admitted(pool: &Pool<Sqlite>, trigger: &TriggerConfig, payload: &Value) -> Result<bool, String> {
    if !passes(trigger, payload) {
        return Ok(false);
    }
    if trigger.group_ids.is_empty() {
        return Ok(true);
    }
    let facts = facts(payload);
    for group_id in &trigger.group_ids {
        let group = Group::get(pool, group_id).await?;
        let allowed = match &group.query {
            Some(query) => query.admits(&facts),
            None => group.conversation_ids.contains(&facts.conversation_id),
        };
        if allowed {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn template_source(action: &Action, node_id: &str) -> Result<(String, String), String> {
    let node = action.graph.nodes.iter().find(|n| n.id == node_id).ok_or_else(|| format!("{} has no node {}", action.name, node_id))?;
    let kind = node.kind.strip_prefix(BUILTIN_PREFIX).and_then(|k| k.strip_prefix("present_")).unwrap_or("custom").to_string();
    Ok((kind, node.config.get("template").and_then(|t| t.as_str()).unwrap_or("").to_string()))
}

pub fn template_failure(action: &Action, node_id: &str, message_id: &str, error: &str) -> ActionRun {
    let mut run = new_run(action, false, 0, json!({ "template": node_id, "message_id": message_id }));
    run.status = "failed".into();
    run.error = Some(format!("The template failed to render: {}", error));
    run.log.push(LogEntry { node: node_id.to_string(), kind: "template".into(), status: "failed".into(), detail: error.to_string() });
    run.finished_at = Some(Utc::now().to_rfc3339());
    run.result = result_of(&run);
    run
}

pub async fn record(runner: &Runner, run: &ActionRun) -> Result<(), String> {
    ActionRun::save(&runner.pool, &runner.profile_id, run).await?;
    if run.status == "failed" && !run.dry_run {
        crate::services::history_service::record_error(&runner.pool, runner.app.as_ref(), &format!("{} failed", run.action_name), run.error.as_deref().unwrap_or(""), Some(format!("/main/actions?id={}", run.action_id))).await.ok();
    }
    if let Some(app) = &runner.app {
        app.emit("action-runs-updated", &run.action_id).ok();
    }
    Ok(())
}

pub async fn dispatch(runner: &Runner, on: &[TriggerOn], message_id: &str) -> Result<Vec<ActionRun>, String> {
    dispatch_to(runner, on, message_id, None).await
}

pub fn trigger_payload(message: Value, conversation: Value, text: &str, extra: Value) -> Value {
    let mut payload = json!({ "message": message, "conversation": conversation, "person": null, "text": text });
    if let (Some(into), Value::Object(more)) = (payload.as_object_mut(), extra) {
        into.extend(more);
    }
    payload
}

async fn fire(runner: &Runner, action: &Action, node: &Node, payload: Value) -> Result<Option<ActionRun>, String> {
    match predicate(runner, action, node, &payload).await {
        Ok(true) => {}
        Ok(false) => return Ok(None),
        Err(e) => {
            eprintln!("Trigger {} of {} failed: {}", node.id, action.name, e);
            return Ok(None);
        }
    }
    let run = execute(runner, action, Start::Trigger { node: node.id.clone(), payload }, 0, vec![action.id.clone()]).await?;
    record(runner, &run).await?;
    Ok(Some(run))
}

pub async fn dispatch_to(runner: &Runner, on: &[TriggerOn], message_id: &str, only: Option<&str>) -> Result<Vec<ActionRun>, String> {
    if !enabled(&runner.pool).await {
        return Ok(Vec::new());
    }
    let payload = payload_for_message(&runner.pool, &runner.profile_id, message_id).await?;
    let conversation_id = text(payload["conversation"].get("id"));
    let mut runs = Vec::new();
    let arrival = on.iter().any(|o| matches!(o, TriggerOn::Arrival | TriggerOn::Filter | TriggerOn::Mention));
    let mine = payload["message"]["mine"].as_bool().unwrap_or(false);
    for action in Action::all(&runner.pool, &runner.profile_id).await?.into_iter().filter(|a| runs_itself(a) && only.is_none_or(|id| id == a.id)) {
        for (node, trigger) in action.triggers() {
            if !on.contains(&trigger.on) || !admitted(&runner.pool, &trigger, &payload).await? {
                continue;
            }
            let Some(run) = fire(runner, &action, node, payload.clone()).await? else { continue };
            if !runner.dry_run {
                for waiting in MessageSchedule::awaiting(&runner.pool, &action.id, &conversation_id).await? {
                    deliver(runner, &waiting).await;
                }
            }
            runs.push(run);
            break;
        }
    }
    if arrival && mine {
        return Ok(runs);
    }
    runs.extend(crate::services::chat_service::dispatch_message(runner, on, &payload, only).await?);
    Ok(runs)
}

pub fn runs_itself(action: &Action) -> bool {
    action.enabled && !is_component(action)
}

async fn remind(runner: &Runner, reminder: &crate::models::reminder::Reminder) {
    use crate::models::reminder::{Reminder, STATUS_DONE, STATUS_FIRING, STATUS_PENDING};
    if !Reminder::put_status(&runner.pool, &reminder.id, STATUS_PENDING, STATUS_FIRING).await.unwrap_or(false) {
        return;
    }
    let quoted: Option<String> = match &reminder.message_id {
        Some(id) => sqlx::query_scalar::<_, Option<String>>("SELECT COALESCE(summary, substr(content, 1, 160)) FROM message WHERE id = ?1").bind(id).fetch_optional(&runner.pool).await.ok().flatten().flatten(),
        None => None,
    };
    let title = reminder.conversation_name.clone().unwrap_or_else(|| "Reminder".to_string());
    let body = [Some(reminder.note.clone()).filter(|n| !n.is_empty()), quoted.map(|q| crate::utils::text_extraction::extract_text_from_content(&q, "html"))].into_iter().flatten().collect::<Vec<_>>().join("\n");
    let id = match &reminder.conversation_id {
        Some(conversation) => format!("{}{}:{}", crate::services::notification_service::MESSAGE_PREFIX, conversation, reminder.message_id.clone().unwrap_or_default()),
        None => format!("reminder:{}", reminder.id),
    };
    crate::services::notification_service::put(&id, &title, &body);
    if let Err(e) = Reminder::put_status(&runner.pool, &reminder.id, STATUS_FIRING, STATUS_DONE).await {
        eprintln!("Reminder {} could not be closed: {}", reminder.id, e);
    }
    if let Some(app) = &runner.app {
        app.emit("reminders-updated", &reminder.id).ok();
    }
}

async fn deliver(runner: &Runner, schedule: &MessageSchedule) {
    let outcome = send_now(runner, &schedule.conversation_id, schedule.provider_id.as_deref(), &schedule.content, &schedule.content_type, schedule.metadata.clone(), schedule.attachments.clone()).await;
    let (status, error) = match &outcome {
        Ok(_) => (STATUS_SENT, None),
        Err(e) => (STATUS_FAILED, Some(e.as_str())),
    };
    if let Err(e) = MessageSchedule::finish(&runner.pool, &schedule.id, status, error).await {
        eprintln!("Scheduled message {} could not be updated: {}", schedule.id, e);
    }
    if let Some(app) = &runner.app {
        app.emit("schedules-updated", &schedule.conversation_id).ok();
    }
}

#[derive(Debug, Default, Serialize)]
pub struct TickReport {
    pub sent: usize,
    pub reminded: usize,
    pub resumed: usize,
    pub scheduled: usize,
    pub events: usize,
}

async fn events_soon(runner: &Runner, action: &Action, node: &Node, trigger: &TriggerConfig, now: DateTime<Local>) -> Result<usize, String> {
    let minutes = trigger.minutes_before.unwrap_or(DEFAULT_MINUTES_BEFORE) as i64;
    let start = now.with_timezone(&Utc);
    let end = start + Duration::minutes(minutes);
    let mut fired = 0;
    for event in crate::models::event::Event::between(&runner.pool, &runner.profile_id, &start.to_rfc3339(), &end.to_rfc3339(), false).await? {
        let Some(begins) = DateTime::parse_from_rfc3339(&event.starts_at).ok().map(|d| d.with_timezone(&Utc)) else { continue };
        if event.all_day || begins < start || begins > end {
            continue;
        }
        if !trigger.scope.conversation_ids.is_empty() && !event.conversation_id.as_ref().is_some_and(|c| trigger.scope.conversation_ids.contains(c)) {
            continue;
        }
        let seen: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM action_run WHERE action_id = ?1 AND dry_run = 0 AND json_extract(trigger, '$.payload.event.id') = ?2")
            .bind(&action.id)
            .bind(&event.id)
            .fetch_one(&runner.pool)
            .await
            .map_err(|e| e.to_string())?;
        if seen {
            continue;
        }
        let conversation = match &event.conversation_id {
            Some(id) => conversation_value(&runner.pool, id).await.unwrap_or(Value::Null),
            None => Value::Null,
        };
        let message = match &event.source_message_id {
            Some(id) => message_value(&runner.pool, id).await.unwrap_or(Value::Null),
            None => Value::Null,
        };
        let payload = trigger_payload(message, conversation, &event.title, json!({ "event": serde_json::to_value(&event).map_err(|e| e.to_string())?, "at": start.to_rfc3339() }));
        if fire(runner, action, node, payload).await?.is_some() {
            fired += 1;
        }
    }
    Ok(fired)
}

pub async fn tick(runner: &Runner, now: DateTime<Local>) -> Result<TickReport, String> {
    let mut report = TickReport::default();
    let stamp = now.with_timezone(&Utc).to_rfc3339();
    for schedule in MessageSchedule::due(&runner.pool, &runner.profile_id, &stamp).await? {
        deliver(runner, &schedule).await;
        report.sent += 1;
    }
    for reminder in crate::models::reminder::Reminder::get_due(&runner.pool, &runner.profile_id, &stamp).await? {
        remind(runner, &reminder).await;
        report.reminded += 1;
    }
    if !enabled(&runner.pool).await {
        return Ok(report);
    }
    for run in ActionRun::waiting_due(&runner.pool, &runner.profile_id, &stamp).await? {
        if let Ok(action) = Action::get(&runner.pool, &run.action_id).await {
            let resumed = resume(runner, &action, run).await?;
            record(runner, &resumed).await?;
            report.resumed += 1;
        }
    }
    for action in Action::all(&runner.pool, &runner.profile_id).await?.into_iter().filter(runs_itself) {
        for (node, trigger) in action.triggers().into_iter().filter(|(_, t)| t.on == TriggerOn::EventSoon) {
            report.events += events_soon(runner, &action, node, &trigger, now).await?;
        }
        for (node, trigger) in action.triggers().into_iter().filter(|(_, t)| t.on == TriggerOn::Custom) {
            let Some(watch) = trigger.watch.as_ref().filter(|w| w.mode == WATCH_POLL) else { continue };
            let last = Action::get_watch(&runner.pool, &action.id, &node.id).await?.and_then(|w| DateTime::parse_from_rfc3339(&w.checked_at).ok()).map(|l| l.with_timezone(&Utc));
            let every = Duration::minutes(i64::from(watch.every_minutes.unwrap_or(DEFAULT_WATCH_MINUTES).max(1)));
            if last.is_some_and(|l| now.with_timezone(&Utc) - l < every) {
                continue;
            }
            match poll(watch).await {
                Ok(body) => {
                    if watched(runner, &action, node, &trigger, body).await?.is_some() {
                        report.scheduled += 1;
                    }
                }
                Err(e) => eprintln!("{} could not poll {}: {}", action.name, watch.url.as_deref().unwrap_or_default(), e),
            }
        }
        for (node, trigger) in action.triggers() {
            let Some(schedule) = trigger.schedule.as_ref().filter(|_| trigger.on == TriggerOn::Schedule) else { continue };
            let last = Action::last_scheduled(&runner.pool, &action.id).await?.and_then(|l| DateTime::parse_from_rfc3339(&l).ok()).map(|l| l.with_timezone(&Utc));
            if !schedule_due(schedule, last, now) {
                continue;
            }
            Action::mark_scheduled(&runner.pool, &action.id, &stamp).await?;
            if fire(runner, &action, node, trigger_payload(Value::Null, Value::Null, "", json!({ "at": stamp }))).await?.is_some() {
                report.scheduled += 1;
            }
            break;
        }
    }
    Ok(report)
}

pub async fn poll(watch: &TriggerWatch) -> Result<Value, String> {
    let url = watch.url.as_deref().ok_or("The trigger has no URL to poll")?;
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build().map_err(|e| e.to_string())?;
    let mut request = client.get(url);
    for (name, value) in &watch.headers {
        request = request.header(name, value);
    }
    let response = request.send().await.map_err(|e| format!("The request failed: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("The URL answered {}", response.status()));
    }
    let text = response.text().await.map_err(|e| format!("The answer could not be read: {}", e))?;
    Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

pub fn watch_value(watch: &TriggerWatch, body: &Value) -> Value {
    match watch.pick.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(pointer) => body.pointer(&if pointer.starts_with('/') { pointer.to_string() } else { format!("/{}", pointer.replace('.', "/")) }).cloned().unwrap_or(Value::Null),
        None => body.clone(),
    }
}

pub async fn watched(runner: &Runner, action: &Action, node: &Node, trigger: &TriggerConfig, body: Value) -> Result<Option<ActionRun>, String> {
    let watch = trigger.watch.clone().unwrap_or_default();
    let value = watch_value(&watch, &body);
    let digest = crate::utils::content_hash(value.to_string().as_bytes());
    let previous = Action::get_watch(&runner.pool, &action.id, &node.id).await?;
    let at = Utc::now().to_rfc3339();
    if !runner.dry_run {
        Action::put_watch(&runner.pool, &action.id, &node.id, &digest, &value, &at).await?;
    }
    let changed = match &previous {
        Some(p) => p.digest != digest,
        None => watch.mode == WATCH_WEBHOOK,
    };
    if !changed {
        return Ok(None);
    }
    let text = match &value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    fire(runner, action, node, trigger_payload(Value::Null, Value::Null, &text, json!({ "payload": body, "value": value, "previous": previous.map(|p| p.value).unwrap_or(Value::Null), "at": at }))).await
}

pub async fn post_webhook(runner: &Runner, action_id: &str, node_id: &str, token: Option<&str>, body: Value) -> Result<Option<ActionRun>, String> {
    let action = Action::find(&runner.pool, &runner.profile_id, action_id).await?;
    if !action.enabled {
        return Err(format!("{} is turned off", action.name));
    }
    let (node, trigger) = action.triggers().into_iter().find(|(n, t)| n.id == node_id && t.on == TriggerOn::Custom).ok_or("No webhook trigger has that address")?;
    let watch = trigger.watch.as_ref().filter(|w| w.mode == WATCH_WEBHOOK).ok_or("That trigger polls a URL and takes no webhook")?;
    if watch.token.as_deref().is_some_and(|t| !t.is_empty() && Some(t) != token) {
        return Err("The webhook token does not match".to_string());
    }
    watched(runner, &action, node, &trigger, body).await
}

pub async fn runner_for(pool: &Pool<Sqlite>, app: &AppHandle, dry_run: bool) -> Result<Runner, String> {
    use tauri::Manager;
    let profile = Profile::get_active(pool).await?.ok_or_else(|| "No active profile".to_string())?;
    let app_data = app.path().app_data_dir().map_err(|e| format!("Failed to find the app data folder: {}", e))?;
    let cache = app.path().app_cache_dir().map_err(|e| format!("Failed to find the app cache folder: {}", e))?;
    Ok(Runner { pool: pool.clone(), app: Some(app.clone()), app_data, cache, profile_id: profile.id, dry_run, ai: None, test_delay: None })
}

pub fn spawn_scheduler(app: AppHandle) {
    use tauri::Manager;
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(SCHEDULER_TICK_SECONDS)).await;
            if crate::services::security_service::is_locked() {
                continue;
            }
            let pool = app.state::<crate::ChatnipService>().db().clone();
            match runner_for(&pool, &app, false).await {
                Ok(runner) => {
                    if let Err(e) = tick(&runner, Local::now()).await {
                        eprintln!("Actions scheduler failed: {}", e);
                    }
                }
                Err(e) => eprintln!("Actions scheduler has no profile: {}", e),
            }
        }
    });
}

pub fn on_arrival(app: AppHandle, message_id: String) {
    on_event(app, message_id, vec![TriggerOn::Arrival, TriggerOn::Filter, TriggerOn::Mention]);
}

pub fn on_sent(app: AppHandle, message_id: String) {
    on_event(app, message_id, vec![TriggerOn::Sent]);
}

pub fn on_edited(app: AppHandle, message_id: String) {
    on_event(app, message_id, vec![TriggerOn::Edited]);
}

pub fn on_deleted(app: AppHandle, message_id: String) {
    on_event(app, message_id, vec![TriggerOn::Deleted]);
}

pub fn on_reaction(app: AppHandle, message_id: String) {
    on_event(app, message_id, vec![TriggerOn::Reaction]);
}

fn on_event(app: AppHandle, message_id: String, on: Vec<TriggerOn>) {
    use tauri::Manager;
    tauri::async_runtime::spawn(async move {
        if crate::services::security_service::is_locked() {
            return;
        }
        let pool = app.state::<crate::ChatnipService>().db().clone();
        let outcome = match runner_for(&pool, &app, false).await {
            Ok(runner) => dispatch(&runner, &on, &message_id).await.map(|_| ()),
            Err(e) => Err(e),
        };
        if let Err(e) = outcome {
            eprintln!("Actions for {} failed: {}", message_id, e);
        }
    });
}

fn draft_of(action: &Action) -> ActionDraft {
    ActionDraft {
        name: action.name.clone(),
        description: action.description.clone(),
        icon: action.icon.clone(),
        max_sends_per_hour: Some(action.max_sends_per_hour),
        inputs: action.inputs.clone(),
        graph: action.graph.clone(),
    }
}

pub fn code_of(action: &Action) -> String {
    action_script::render(&draft_of(action))
}

pub async fn others(pool: &Pool<Sqlite>, profile_id: &str) -> Result<HashMap<String, Action>, String> {
    Ok(Action::all(pool, profile_id).await?.into_iter().map(|a| (a.id.clone(), a)).collect())
}

fn apply_draft(action: &mut Action, draft: ActionDraft) {
    action.name = draft.name;
    action.description = draft.description;
    action.icon = draft.icon;
    if let Some(max) = draft.max_sends_per_hour {
        action.max_sends_per_hour = max;
    }
    action.inputs = draft.inputs;
    action.graph = draft.graph;
}

fn code_error(diagnostics: Vec<Diagnostic>) -> String {
    diagnostics.into_iter().map(|d| match d.line {
        Some(line) => format!("line {}: {}", line, d.message),
        None => d.message,
    }).collect::<Vec<_>>().join("; ")
}

pub async fn post_action(pool: &Pool<Sqlite>, profile_id: &str, input: PostActionInput) -> Result<Action, String> {
    let mut action = Action::blank(profile_id, &input.name);
    action.max_sends_per_hour = AppSetting::get_u64(pool, app_setting::ACTIONS_MAX_SENDS_PER_HOUR).await? as i64;
    if let Some(code) = &input.code {
        apply_draft(&mut action, action_script::parse(code).map_err(code_error)?);
    }
    if !input.name.trim().is_empty() {
        action.name = input.name.trim().to_string();
    }
    if action.name.trim().is_empty() {
        return Err("An action needs a name".to_string());
    }
    if let Some(group) = &input.group_id {
        if ActionGroup::get(pool, group).await?.builtin {
            return Err("Your actions stay out of the built in groups".to_string());
        }
    }
    action.group_id = input.group_id.clone();
    action.description = input.description.clone().unwrap_or(action.description);
    action.icon = input.icon.clone().or(action.icon);
    action.inputs = input.inputs.clone().unwrap_or(action.inputs);
    action.graph = input.graph.clone().unwrap_or(action.graph);
    action.max_sends_per_hour = input.max_sends_per_hour.unwrap_or(action.max_sends_per_hour);
    action.ai = input.ai.clone().unwrap_or(action.ai);
    normalize(&mut action.graph);
    let diagnostics = check(&draft_of(&action), &others(pool, profile_id).await?, Some(&action.id));
    refuse_wiring(&diagnostics, &[])?;
    let errors = diagnostics.iter().any(|d| d.severity == "error");
    action.enabled = input.enabled.unwrap_or(true) && !errors;
    Action::insert(pool, &action).await
}

pub async fn put_action(pool: &Pool<Sqlite>, input: PutActionInput) -> Result<Action, String> {
    let mut action = Action::get(pool, &input.action_id).await?;
    if action.builtin {
        let only_switch = input.name.is_none() && input.description.is_none() && input.icon.is_none() && input.group_id.is_none() && input.position.is_none() && input.inputs.is_none() && input.graph.is_none() && input.code.is_none() && input.max_sends_per_hour.is_none();
        if let (true, Some(KIND_ACTION)) = (only_switch && (input.enabled.is_some() || input.ai.is_some()), action.builtin_kind.as_deref()) {
            action.enabled = input.enabled.unwrap_or(action.enabled);
            action.ai = input.ai.clone().unwrap_or(action.ai);
            return Action::save(pool, &action).await;
        }
        return Err("Built in actions are read only. Switch a built in action on or off, choose its AI, or duplicate one to change it.".to_string());
    }
    if let Some(Some(group)) = &input.group_id {
        if ActionGroup::get(pool, group).await?.builtin {
            return Err("Your actions stay out of the built in groups".to_string());
        }
    }
    let known = others(pool, &action.profile_id).await?;
    let before = check(&draft_of(&action), &known, Some(&action.id));
    if let Some(code) = &input.code {
        apply_draft(&mut action, action_script::parse(code).map_err(code_error)?);
    }
    if let Some(name) = &input.name {
        action.name = name.trim().to_string();
    }
    if let Some(description) = &input.description {
        action.description = description.clone();
    }
    if let Some(icon) = &input.icon {
        action.icon = icon.clone();
    }
    if let Some(group) = &input.group_id {
        action.group_id = group.clone();
    }
    if let Some(position) = input.position {
        action.position = position;
    }
    if let Some(inputs) = &input.inputs {
        action.inputs = inputs.clone();
    }
    if let Some(graph) = &input.graph {
        action.graph = graph.clone();
    }
    if let Some(max) = input.max_sends_per_hour {
        action.max_sends_per_hour = max;
    }
    if let Some(ai) = &input.ai {
        action.ai = ai.clone();
    }
    normalize(&mut action.graph);
    let diagnostics = check(&draft_of(&action), &known, Some(&action.id));
    refuse_wiring(&diagnostics, &before)?;
    let errors: Vec<Diagnostic> = diagnostics.into_iter().filter(|d| d.severity == "error").collect();
    let wants = input.enabled.unwrap_or(action.enabled);
    if input.enabled == Some(true) && !errors.is_empty() {
        return Err(format!("Fix the action before switching it on: {}", code_error(errors)));
    }
    action.enabled = wants && errors.is_empty();
    Action::save(pool, &action).await
}

pub fn without_start(graph: &Graph) -> Graph {
    let starts: HashSet<&str> = graph.nodes.iter().filter(|n| n.kind == NODE_TRIGGER).map(|n| n.id.as_str()).collect();
    Graph {
        nodes: graph.nodes.iter().filter(|n| !starts.contains(n.id.as_str())).cloned().collect(),
        edges: graph.edges.iter().filter(|e| !starts.contains(e.source.as_str()) && !starts.contains(e.target.as_str())).cloned().collect(),
    }
}

pub async fn post_action_copy(pool: &Pool<Sqlite>, profile_id: &str, action_id: &str, without_trigger: bool, name: Option<&str>) -> Result<Action, String> {
    let source = Action::find(pool, profile_id, action_id).await?;
    let mut copy = Action::blank(profile_id, name.map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&if without_trigger { format!("{} for a chat", source.name) } else { format!("{} copy", source.name) }));
    copy.description = source.description.clone();
    copy.icon = source.icon.clone();
    copy.group_id = if source.builtin { None } else { source.group_id.clone() };
    copy.inputs = source.inputs.clone();
    copy.max_sends_per_hour = source.max_sends_per_hour;
    copy.ai = source.ai.clone();
    copy.graph = if without_trigger { without_start(&source.graph) } else { source.graph.clone() };
    normalize(&mut copy.graph);
    let errors = check(&draft_of(&copy), &others(pool, profile_id).await?, Some(&copy.id)).into_iter().any(|d| d.severity == "error");
    copy.enabled = without_trigger && !errors;
    Action::insert(pool, &copy).await
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
pub struct CheckInput {
    #[serde(default)]
    pub action_id: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub draft: Option<ActionDraft>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckOutcome {
    pub diagnostics: Vec<Diagnostic>,
    pub draft: Option<ActionDraft>,
    pub outputs: Vec<Port>,
    pub code: Option<String>,
}

pub async fn check_input(pool: &Pool<Sqlite>, profile_id: &str, input: CheckInput) -> Result<CheckOutcome, String> {
    let mut draft = match (&input.code, &input.draft, &input.action_id) {
        (Some(code), _, _) => match action_script::parse(code) {
            Ok(draft) => draft,
            Err(diagnostics) => return Ok(CheckOutcome { diagnostics, draft: None, outputs: Vec::new(), code: Some(code.clone()) }),
        },
        (None, Some(draft), _) => draft.clone(),
        (None, None, Some(id)) => draft_of(&Action::get(pool, id).await?),
        _ => return Err("Pass an action id, its code, or a draft".to_string()),
    };
    normalize(&mut draft.graph);
    let known = others(pool, profile_id).await?;
    let mut diagnostics = check(&draft, &known, input.action_id.as_deref());
    diagnostics.extend(unmet(&draft, &known, &requirement_state(pool, profile_id).await?));
    let code = action_script::render(&draft);
    let outputs = signature(&draft.graph, &known);
    Ok(CheckOutcome { diagnostics, draft: Some(draft), outputs, code: Some(code) })
}

#[derive(Debug, Clone, Serialize)]
pub struct Catalog {
    pub types: Vec<&'static str>,
    pub chat_node: chat_model::ChatNodeSpec,
    pub functions: Vec<action_script::FunctionSpec>,
    pub builtins: Vec<Builtin>,
    pub prompts: Vec<intelligence_service::Prompt>,
    pub content_prompts: Vec<Value>,
    pub tags: Vec<Value>,
    pub splits: Vec<Value>,
    pub groups: Vec<Value>,
    pub actions: Vec<Value>,
    pub action_groups: Vec<ActionGroup>,
    pub enabled: bool,
    pub requirements: Vec<RequirementStatus>,
    pub unmet: Vec<UnmetRequirement>,
    pub result_schema: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RequirementStatus {
    pub key: &'static str,
    pub label: &'static str,
    pub met: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UnmetRequirement {
    pub action_id: String,
    pub action: String,
    pub node: String,
    pub setting: String,
    pub label: String,
}

pub fn unmet_across(actions: &[Action], state: &RequirementState) -> Vec<UnmetRequirement> {
    let known: HashMap<String, Action> = actions.iter().map(|a| (a.id.clone(), a.clone())).collect();
    let mut out = Vec::new();
    for action in actions {
        for d in unmet(&draft_of(action), &known, state) {
            out.push(UnmetRequirement { action_id: action.id.clone(), action: action.name.clone(), node: d.node.unwrap_or_default(), setting: d.setting.unwrap_or_default(), label: d.message });
        }
    }
    out
}

pub fn catalog_entry(action: &Action, known: &HashMap<String, Action>, state: &RequirementState) -> Value {
    let draft = draft_of(action);
    let mut diagnostics = check(&draft, known, Some(&action.id));
    diagnostics.extend(unmet(&draft, known, state));
    let mut requires: Vec<&'static str> = Vec::new();
    for node in &action.graph.nodes {
        for key in node_requirements(node, known) {
            if !requires.contains(&key) {
                requires.push(key);
            }
        }
    }
    json!({
        "id": action.id,
        "name": action.name,
        "description": action.description,
        "icon": action.icon,
        "key": action.key,
        "builtin": action.builtin,
        "builtin_kind": action.builtin_kind,
        "position": action.position,
        "usage": action.usage,
        "role": action_role(action),
        "requires": requires,
        "enabled": action.enabled,
        "inputs": action.inputs,
        "outputs": action.outputs,
        "group_id": action.group_id,
        "graph": action.graph,
        "diagnostics": diagnostics,
    })
}

pub async fn catalog(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Catalog, String> {
    let labels = Tag::all(pool, profile_id).await?;
    let actions = Action::all(pool, profile_id).await?;
    let state = requirement_state(pool, profile_id).await?;
    Ok(Catalog {
        types: PortType::ALL.iter().map(|t| t.name()).collect(),
        functions: action_script::library().to_vec(),
        builtins: builtins().to_vec(),
        chat_node: chat_model::node_spec().clone(),
        prompts: intelligence_service::prompts(),
        content_prompts: ContentItem::library(pool, profile_id, Some(KIND_PROMPT), None).await?.into_iter().map(|i| json!({ "id": i.id, "name": i.name, "body": i.body.unwrap_or_default() })).collect(),
        tags: labels.iter().map(|l| json!({ "id": l.id, "name": l.name, "kind": l.kind })).collect(),
        splits: labels.iter().filter(|l| l.kind == KIND_SPLIT).map(|l| json!({ "id": l.id, "name": l.name })).collect(),
        groups: Group::all(pool, profile_id).await.unwrap_or_default().into_iter().map(|g| json!({ "id": g.id, "name": g.name, "kind": g.kind })).collect(),
        actions: {
            let known: HashMap<String, Action> = actions.iter().map(|a| (a.id.clone(), a.clone())).collect();
            actions.iter().map(|a| catalog_entry(a, &known, &state)).collect()
        },
        action_groups: ActionGroup::all(pool, profile_id).await?,
        enabled: enabled(pool).await,
        requirements: action_script::requirements().into_iter().map(|r| RequirementStatus { key: r.key, label: r.label, met: state.met(r.key) }).collect(),
        unmet: unmet_across(&actions, &state),
        result_schema: result_schema(),
    })
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
pub struct AssistMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct AssistInput {
    #[serde(default)]
    pub action_id: Option<String>,
    #[serde(default)]
    pub draft: Option<ActionDraft>,
    pub instruction: String,
    #[serde(default)]
    pub messages: Vec<AssistMessage>,
}

pub const OPERATIONS: [&str; 8] = ["create", "update", "copy", "split", "merge", "delete", "move", "rename"];

#[derive(Debug, Clone, Serialize, serde::Deserialize, Default, PartialEq)]
pub struct ActionPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_sends_per_hour: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<Port>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<Graph>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ActionOperation {
    Create {
        #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
        #[serde(default)]
        group_id: Option<String>,
        action: ActionDraft,
    },
    Update {
        action_id: String,
        action: ActionPatch,
    },
    Copy {
        action_id: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        without_trigger: bool,
        #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
    },
    Split {
        action_id: String,
        node_ids: Vec<String>,
        name: String,
        #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
    },
    Merge {
        action_ids: Vec<String>,
        name: String,
        #[serde(default)]
        remove_sources: bool,
        #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
    },
    Delete {
        action_id: String,
    },
    Move {
        action_ids: Vec<String>,
        #[serde(default)]
        group_id: Option<String>,
    },
    Rename {
        action_id: String,
        name: String,
    },
    Restore {
        action: Box<Action>,
    },
}

impl ActionOperation {
    pub fn name(&self) -> &'static str {
        match self {
            ActionOperation::Create { .. } => "create",
            ActionOperation::Update { .. } => "update",
            ActionOperation::Copy { .. } => "copy",
            ActionOperation::Split { .. } => "split",
            ActionOperation::Merge { .. } => "merge",
            ActionOperation::Delete { .. } => "delete",
            ActionOperation::Move { .. } => "move",
            ActionOperation::Rename { .. } => "rename",
            ActionOperation::Restore { .. } => "restore",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationPreview {
    pub index: usize,
    pub op: &'static str,
    pub action_id: String,
    pub name: String,
    pub group_before: Option<String>,
    pub group_after: Option<String>,
    pub before: Option<ActionDraft>,
    pub after: Option<ActionDraft>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct AssistOutcome {
    pub reply: String,
    pub operations: Vec<ActionOperation>,
    pub previews: Vec<OperationPreview>,
    pub errors: Vec<String>,
    pub requirement: Option<Diagnostic>,
}

fn fill_edge_ids(operation: &mut Value) {
    let Some(edges) = operation.pointer_mut("/action/graph/edges").and_then(Value::as_array_mut) else { return };
    for edge in edges.iter_mut().filter_map(Value::as_object_mut) {
        if edge.get("id").and_then(Value::as_str).is_some_and(|id| !id.trim().is_empty()) {
            continue;
        }
        let part = |key: &str| edge.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
        let id = format!("{}.{}-{}.{}", part("source"), part("source_port"), part("target"), part("target_port"));
        edge.insert("id".into(), Value::String(id));
    }
}

pub fn parse_operations(output: &Value) -> Result<(String, Vec<ActionOperation>), Vec<String>> {
    let Some(answer) = output.as_object() else {
        return Err(vec!["Answer with one JSON object holding reply and operations, and nothing else".to_string()]);
    };
    let reply = answer.get("reply").and_then(Value::as_str).map(str::trim).unwrap_or_default().to_string();
    let mut errors = Vec::new();
    if reply.is_empty() {
        errors.push("reply must be one short sentence".to_string());
    }
    let mut operations = Vec::new();
    match answer.get("operations").and_then(Value::as_array) {
        None => errors.push("operations must be a list, empty when nothing changes".to_string()),
        Some(list) => {
            for (index, raw) in list.iter().enumerate() {
                let op = raw.get("op").and_then(Value::as_str).unwrap_or_default();
                if !OPERATIONS.contains(&op) {
                    errors.push(format!("operation {}: op {:?} is not one of {}", index + 1, op, OPERATIONS.join(", ")));
                    continue;
                }
                let mut raw = raw.clone();
                fill_edge_ids(&mut raw);
                match serde_json::from_value::<ActionOperation>(raw) {
                    Ok(operation) => operations.push(operation),
                    Err(e) => errors.push(format!("operation {} ({}): {}", index + 1, op, e)),
                }
            }
        }
    }
    if errors.is_empty() {
        Ok((reply, operations))
    } else {
        Err(errors)
    }
}

fn placeholder(index: usize, reference: &Option<String>) -> String {
    reference.as_deref().map(str::trim).filter(|r| !r.is_empty()).map(String::from).unwrap_or_else(|| format!("new-{}", index + 1))
}

fn resolved(id: &str, ids: &HashMap<String, String>) -> String {
    ids.get(id).cloned().unwrap_or_else(|| id.to_string())
}

fn with_ids(graph: &mut Graph, ids: &HashMap<String, String>) {
    for node in &mut graph.nodes {
        let real = node.kind.strip_prefix(ACTION_PREFIX).and_then(|id| ids.get(id)).cloned();
        if let Some(real) = real {
            node.kind = format!("{}{}", ACTION_PREFIX, real);
        }
    }
}

fn unique_node_id(base: &str, taken: &[&str]) -> String {
    let base = if base.is_empty() { "part" } else { base };
    let mut id = base.to_string();
    let mut n = 2;
    while taken.contains(&id.as_str()) {
        id = format!("{}_{}", base, n);
        n += 1;
    }
    id
}

pub fn split_graph(source: &Action, node_ids: &[String], name: &str, part_id: &str, actions: &HashMap<String, Action>) -> Result<(ActionDraft, Graph), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("the new action needs a name".to_string());
    }
    let picked: HashSet<&str> = node_ids.iter().map(String::as_str).collect();
    if picked.is_empty() {
        return Err("name the nodes that move into the new action".to_string());
    }
    if let Some(id) = picked.iter().find(|id| !source.graph.nodes.iter().any(|n| n.id == **id)) {
        return Err(format!("{} has no node {}", source.name, id));
    }
    if picked.len() == source.graph.nodes.len() {
        return Err("keep at least one node in the original; copy the action to duplicate it whole".to_string());
    }
    let inside = |id: &str| picked.contains(id);
    let mut nodes: Vec<Node> = source.graph.nodes.iter().filter(|n| inside(&n.id)).cloned().collect();
    let mut edges: Vec<Edge> = source.graph.edges.iter().filter(|e| inside(&e.source) && inside(&e.target)).cloned().collect();
    let incoming: Vec<&Edge> = source.graph.edges.iter().filter(|e| !inside(&e.source) && inside(&e.target)).collect();
    let outgoing: Vec<&Edge> = source.graph.edges.iter().filter(|e| inside(&e.source) && !inside(&e.target)).collect();
    let mut inputs: Vec<Port> = Vec::new();
    let mut feeds: Vec<((String, String), String)> = Vec::new();
    if !incoming.is_empty() {
        if nodes.iter().any(|n| n.kind == NODE_TRIGGER) {
            return Err("the nodes include a trigger, so they cannot also take input from the rest".to_string());
        }
        let start = unique_node_id("start", &nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>());
        let reserved = trigger_outputs(TriggerOn::Called);
        for edge in &incoming {
            let key = (edge.source.clone(), edge.source_port.clone());
            let port = match feeds.iter().find(|(k, _)| *k == key) {
                Some((_, port)) => port.clone(),
                None => {
                    let kind = nodes.iter().find(|n| n.id == edge.target).and_then(|n| node_ports(n, actions, Some(&source.inputs))).and_then(|(ins, _)| ins.into_iter().find(|p| p.name == edge.target_port)).map(|p| p.kind).unwrap_or(PortType::Any);
                    let clashes = |name: &str| inputs.iter().any(|p| p.name == name) || reserved.iter().any(|p| p.name == name && p.kind != kind);
                    let mut port = edge.target_port.clone();
                    let mut n = 2;
                    while clashes(&port) {
                        port = format!("{}_{}", edge.target_port, n);
                        n += 1;
                    }
                    inputs.push(Port::new(&port, kind));
                    feeds.push((key, port.clone()));
                    port
                }
            };
            edges.push(wire(&start, &port, &edge.target, &edge.target_port));
        }
        let top = nodes.iter().map(|n| n.y).fold(f64::INFINITY, f64::min);
        let left = nodes.iter().map(|n| n.x).fold(f64::INFINITY, f64::min);
        nodes.insert(0, Node { x: left - 300.0, ..trigger_node(&start, "Start", json!({ "on": "called" }), "", top) });
    }
    let inner = Graph { nodes, edges };
    let outputs = signature(&inner, actions);
    let mut gives: Vec<(&Edge, String)> = Vec::new();
    for edge in &outgoing {
        if inner.edges.iter().any(|e| e.source == edge.source) {
            return Err(format!("{} feeds both parts; move the nodes after it as well, or split after it", edge.source));
        }
        let port = outputs.iter().find(|p| p.node.as_deref() == Some(edge.source.as_str()) && p.source_port.as_deref() == Some(edge.source_port.as_str())).ok_or_else(|| format!("{} gives no output {}", edge.source, edge.source_port))?;
        gives.push((*edge, port.name.clone()));
    }
    let kept: Vec<&Node> = source.graph.nodes.iter().filter(|n| !inside(&n.id)).collect();
    let moved: Vec<&Node> = source.graph.nodes.iter().filter(|n| inside(&n.id)).collect();
    let host_id = unique_node_id(&slug(name), &kept.iter().map(|n| n.id.as_str()).collect::<Vec<_>>());
    let host = Node {
        id: host_id.clone(),
        kind: format!("{}{}", ACTION_PREFIX, part_id),
        label: name.to_string(),
        config: json!({}),
        body: String::new(),
        x: (moved.iter().map(|n| n.x).sum::<f64>() / moved.len() as f64).round(),
        y: moved.iter().map(|n| n.y).fold(f64::INFINITY, f64::min),
    };
    let mut rest: Vec<Edge> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut keep = |edge: Edge, rest: &mut Vec<Edge>| {
        if seen.insert(edge.id.clone()) {
            rest.push(edge);
        }
    };
    for edge in source.graph.edges.iter().filter(|e| !inside(&e.source) && !inside(&e.target)) {
        keep(edge.clone(), &mut rest);
    }
    for edge in &incoming {
        if let Some((_, port)) = feeds.iter().find(|(k, _)| k.0 == edge.source && k.1 == edge.source_port) {
            keep(wire(&edge.source, &edge.source_port, &host_id, port), &mut rest);
        }
    }
    for (edge, port) in &gives {
        keep(wire(&host_id, port, &edge.target, &edge.target_port), &mut rest);
    }
    let mut remaining: Vec<Node> = kept.into_iter().cloned().collect();
    remaining.push(host);
    let draft = ActionDraft { name: name.to_string(), description: format!("Split out of {}", source.name), icon: source.icon.clone(), max_sends_per_hour: Some(source.max_sends_per_hour), inputs, graph: inner };
    Ok((draft, Graph { nodes: remaining, edges: rest }))
}

pub fn merge_graphs(sources: &[Action], name: &str) -> ActionDraft {
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();
    let mut inputs: Vec<Port> = Vec::new();
    let mut left = 0.0;
    for source in sources {
        let low = source.graph.nodes.iter().map(|n| n.x).fold(f64::INFINITY, f64::min);
        let high = source.graph.nodes.iter().map(|n| n.x).fold(f64::NEG_INFINITY, f64::max);
        let low = if low.is_finite() { low } else { 0.0 };
        let mut ids: HashMap<&str, String> = HashMap::new();
        for node in &source.graph.nodes {
            let id = if nodes.iter().any(|n| n.id == node.id) {
                unique_node_id(&format!("{}_{}", slug(&source.name), node.id), &nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>())
            } else {
                node.id.clone()
            };
            ids.insert(node.id.as_str(), id.clone());
            nodes.push(Node { id, x: node.x - low + left, ..node.clone() });
        }
        for edge in &source.graph.edges {
            if let (Some(from), Some(to)) = (ids.get(edge.source.as_str()), ids.get(edge.target.as_str())) {
                edges.push(wire(from, &edge.source_port, to, &edge.target_port));
            }
        }
        for port in &source.inputs {
            if !inputs.iter().any(|p| p.name == port.name) {
                inputs.push(port.clone());
            }
        }
        left += (if high.is_finite() { high - low } else { 0.0 }) + 600.0;
    }
    ActionDraft {
        name: name.trim().to_string(),
        description: sources.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(" + "),
        icon: sources.first().and_then(|s| s.icon.clone()),
        max_sends_per_hour: sources.iter().map(|s| s.max_sends_per_hour).max(),
        inputs,
        graph: Graph { nodes, edges },
    }
}

fn group_ok(groups: &[ActionGroup], group: Option<&str>) -> Result<(), String> {
    let Some(id) = group else { return Ok(()) };
    match groups.iter().find(|g| g.id == id) {
        None => Err(format!("there is no action group {}", id)),
        Some(g) if g.builtin => Err(format!("{} is a built in group and read only", g.name)),
        Some(_) => Ok(()),
    }
}

fn editable<'a>(state: &'a HashMap<String, Action>, id: &str) -> Result<&'a Action, String> {
    match state.get(id) {
        None => Err(format!("there is no action {}", id)),
        Some(action) if action.builtin => Err(format!("{} is built in and read only; copy it to change it", action.name)),
        Some(action) => Ok(action),
    }
}

fn fresh(state: &HashMap<String, Action>, index: usize, reference: &Option<String>) -> Result<String, String> {
    let id = placeholder(index, reference);
    if state.contains_key(&id) {
        return Err(format!("ref {} is already an action id", id));
    }
    Ok(id)
}

fn named(action: &Action) -> Result<(), String> {
    if action.name.trim().is_empty() {
        return Err("an action needs a name".to_string());
    }
    Ok(())
}

fn preview(index: usize, op: &'static str, before: Option<&Action>, after: Option<&Action>) -> OperationPreview {
    let shown = after.or(before);
    OperationPreview {
        index,
        op,
        action_id: shown.map(|a| a.id.clone()).unwrap_or_default(),
        name: shown.map(|a| a.name.clone()).unwrap_or_default(),
        group_before: before.and_then(|a| a.group_id.clone()),
        group_after: after.and_then(|a| a.group_id.clone()),
        before: before.map(draft_of),
        after: after.map(draft_of),
        diagnostics: Vec::new(),
    }
}

fn plan_one(index: usize, operation: &ActionOperation, state: &mut HashMap<String, Action>, groups: &[ActionGroup], profile_id: &str) -> Result<Vec<OperationPreview>, String> {
    let op = operation.name();
    let mut shown = Vec::new();
    match operation {
        ActionOperation::Create { reference, group_id, action } => {
            group_ok(groups, group_id.as_deref())?;
            let mut made = Action::blank(profile_id, &action.name);
            made.id = fresh(state, index, reference)?;
            apply_draft(&mut made, action.clone());
            made.group_id = group_id.clone();
            made.enabled = false;
            named(&made)?;
            normalize(&mut made.graph);
            shown.push(preview(index, op, None, Some(&made)));
            state.insert(made.id.clone(), made);
        }
        ActionOperation::Update { action_id, action: patch } => {
            let before = editable(state, action_id)?.clone();
            let mut after = before.clone();
            if let Some(name) = &patch.name {
                after.name = name.trim().to_string();
            }
            if let Some(description) = &patch.description {
                after.description = description.clone();
            }
            if let Some(icon) = &patch.icon {
                after.icon = Some(icon.clone());
            }
            if let Some(max) = patch.max_sends_per_hour {
                after.max_sends_per_hour = max;
            }
            if let Some(inputs) = &patch.inputs {
                after.inputs = inputs.clone();
            }
            if let Some(graph) = &patch.graph {
                after.graph = graph.clone();
            }
            named(&after)?;
            normalize(&mut after.graph);
            shown.push(preview(index, op, Some(&before), Some(&after)));
            state.insert(after.id.clone(), after);
        }
        ActionOperation::Copy { action_id, name, without_trigger, reference } => {
            let source = state.get(action_id).cloned().ok_or_else(|| format!("there is no action {}", action_id))?;
            let fallback = if *without_trigger { format!("{} for a chat", source.name) } else { format!("{} copy", source.name) };
            let mut copy = Action::blank(profile_id, name.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&fallback));
            copy.id = fresh(state, index, reference)?;
            copy.description = source.description.clone();
            copy.icon = source.icon.clone();
            copy.group_id = if source.builtin { None } else { source.group_id.clone() };
            copy.inputs = source.inputs.clone();
            copy.max_sends_per_hour = source.max_sends_per_hour;
            copy.ai = source.ai.clone();
            copy.graph = if *without_trigger { without_start(&source.graph) } else { source.graph.clone() };
            copy.enabled = false;
            normalize(&mut copy.graph);
            shown.push(preview(index, op, None, Some(&copy)));
            state.insert(copy.id.clone(), copy);
        }
        ActionOperation::Split { action_id, node_ids, name, reference } => {
            let source = editable(state, action_id)?.clone();
            let id = fresh(state, index, reference)?;
            let (draft, graph) = split_graph(&source, node_ids, name, &id, state)?;
            let mut part = Action::blank(profile_id, &draft.name);
            part.id = id;
            apply_draft(&mut part, draft);
            part.group_id = source.group_id.clone();
            normalize(&mut part.graph);
            let mut rest = source.clone();
            rest.graph = graph;
            normalize(&mut rest.graph);
            shown.push(preview(index, op, None, Some(&part)));
            shown.push(preview(index, op, Some(&source), Some(&rest)));
            state.insert(part.id.clone(), part);
            state.insert(rest.id.clone(), rest);
        }
        ActionOperation::Merge { action_ids, name, remove_sources, reference } => {
            if action_ids.len() < 2 {
                return Err("merge needs two or more action_ids".to_string());
            }
            let mut sources = Vec::new();
            for id in action_ids {
                let source = if *remove_sources { editable(state, id)?.clone() } else { state.get(id).cloned().ok_or_else(|| format!("there is no action {}", id))? };
                sources.push(source);
            }
            let draft = merge_graphs(&sources, name);
            let mut made = Action::blank(profile_id, &draft.name);
            made.id = fresh(state, index, reference)?;
            apply_draft(&mut made, draft);
            made.group_id = sources.iter().find(|s| !s.builtin).and_then(|s| s.group_id.clone());
            made.enabled = false;
            named(&made)?;
            normalize(&mut made.graph);
            shown.push(preview(index, op, None, Some(&made)));
            state.insert(made.id.clone(), made);
            if *remove_sources {
                for source in &sources {
                    shown.push(preview(index, op, Some(source), None));
                    state.remove(&source.id);
                }
            }
        }
        ActionOperation::Delete { action_id } => {
            let before = editable(state, action_id)?.clone();
            shown.push(preview(index, op, Some(&before), None));
            state.remove(&before.id);
        }
        ActionOperation::Move { action_ids, group_id } => {
            group_ok(groups, group_id.as_deref())?;
            if action_ids.is_empty() {
                return Err("move needs action_ids".to_string());
            }
            for id in action_ids {
                let before = editable(state, id)?.clone();
                let mut after = before.clone();
                after.group_id = group_id.clone();
                shown.push(preview(index, op, Some(&before), Some(&after)));
                state.insert(after.id.clone(), after);
            }
        }
        ActionOperation::Rename { action_id, name } => {
            let before = editable(state, action_id)?.clone();
            let mut after = before.clone();
            after.name = name.trim().to_string();
            named(&after)?;
            shown.push(preview(index, op, Some(&before), Some(&after)));
            state.insert(after.id.clone(), after);
        }
        ActionOperation::Restore { action } => {
            let before = state.get(&action.id).cloned();
            shown.push(preview(index, op, before.as_ref(), Some(&**action)));
            state.insert(action.id.clone(), (**action).clone());
        }
    }
    Ok(shown)
}

pub fn plan_operations(operations: &[ActionOperation], known: &HashMap<String, Action>, groups: &[ActionGroup], profile_id: &str) -> (Vec<OperationPreview>, Vec<String>) {
    let mut state = known.clone();
    let mut previews = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for (index, operation) in operations.iter().enumerate() {
        match plan_one(index, operation, &mut state, groups, profile_id) {
            Ok(mut shown) => previews.append(&mut shown),
            Err(e) => errors.push(format!("operation {} ({}): {}", index + 1, operation.name(), e)),
        }
        let mut all: Vec<Action> = state.drain().map(|(_, a)| a).collect();
        sign(&mut all);
        state = all.into_iter().map(|a| (a.id.clone(), a)).collect();
    }
    for shown in previews.iter_mut().filter(|p| p.op != "restore" && p.after.is_some()) {
        let Some(after) = state.get(&shown.action_id) else { continue };
        let found = check(&draft_of(after), &state, Some(&after.id));
        let existing: HashSet<String> = shown.before.as_ref().map(|b| check(b, &state, Some(&after.id)).into_iter().map(|d| d.message).collect()).unwrap_or_default();
        for d in found.iter().filter(|d| d.severity == "error" && !existing.contains(&d.message)) {
            errors.push(format!("operation {} ({}) on {:?}: {}{}", shown.index + 1, shown.op, after.name, d.node.as_ref().map(|n| format!("node {}: ", n)).unwrap_or_default(), d.message));
        }
        shown.diagnostics = found;
    }
    for action in state.values() {
        for node in &action.graph.nodes {
            let Some(id) = node.kind.strip_prefix(ACTION_PREFIX) else { continue };
            if let (Some(gone), false) = (known.get(id), state.contains_key(id)) {
                errors.push(format!("{:?} still uses {:?} in node {}; remove that node or keep the action", action.name, gone.name, node.id));
            }
        }
    }
    let mut seen = HashSet::new();
    errors.retain(|e| seen.insert(e.clone()));
    (previews, errors)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PutActionsInput {
    pub operations: Vec<ActionOperation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionsOutcome {
    pub actions: Vec<Action>,
    pub undo: Vec<ActionOperation>,
    pub ids: HashMap<String, String>,
}

#[derive(Default)]
struct Applied {
    created: Vec<String>,
    before: Vec<Action>,
}

impl Applied {
    fn keep(&mut self, action: &Action) {
        if !self.created.contains(&action.id) && !self.before.iter().any(|a| a.id == action.id) {
            self.before.push(action.clone());
        }
    }
}

async fn restore_action(pool: &Pool<Sqlite>, action: &Action) -> Result<Action, String> {
    let existed = Action::get(pool, &action.id).await.is_ok();
    if !existed {
        Action::insert(pool, action).await?;
    }
    let saved = Action::save(pool, action).await?;
    if !existed {
        for conversation in &action.chats {
            chat_model::put_action(pool, conversation, &action.id).await?;
        }
    }
    Ok(saved)
}

async fn apply_operation(pool: &Pool<Sqlite>, profile_id: &str, index: usize, operation: &ActionOperation, ids: &mut HashMap<String, String>, applied: &mut Applied) -> Result<(), String> {
    match operation {
        ActionOperation::Create { reference, group_id, action } => {
            let mut graph = action.graph.clone();
            with_ids(&mut graph, ids);
            let made = post_action(pool, profile_id, PostActionInput { name: action.name.clone(), description: Some(action.description.clone()), icon: action.icon.clone(), group_id: group_id.clone(), enabled: Some(false), inputs: Some(action.inputs.clone()), graph: Some(graph), max_sends_per_hour: action.max_sends_per_hour, ..Default::default() }).await?;
            applied.created.push(made.id.clone());
            ids.insert(placeholder(index, reference), made.id);
        }
        ActionOperation::Update { action_id, action: patch } => {
            let target = Action::get(pool, &resolved(action_id, ids)).await?;
            applied.keep(&target);
            let graph = patch.graph.clone().map(|mut graph| {
                with_ids(&mut graph, ids);
                graph
            });
            put_action(pool, PutActionInput { action_id: target.id.clone(), name: patch.name.clone(), description: patch.description.clone(), icon: patch.icon.clone().map(Some), inputs: patch.inputs.clone(), graph, max_sends_per_hour: patch.max_sends_per_hour, ..Default::default() }).await?;
        }
        ActionOperation::Copy { action_id, name, without_trigger, reference } => {
            let made = post_action_copy(pool, profile_id, &resolved(action_id, ids), *without_trigger, name.as_deref()).await?;
            applied.created.push(made.id.clone());
            ids.insert(placeholder(index, reference), made.id);
        }
        ActionOperation::Split { action_id, node_ids, name, reference } => {
            let source = Action::get(pool, &resolved(action_id, ids)).await?;
            applied.keep(&source);
            let marker = placeholder(index, reference);
            let (draft, mut graph) = split_graph(&source, node_ids, name, &marker, &others(pool, profile_id).await?)?;
            let made = post_action(pool, profile_id, PostActionInput { name: draft.name.clone(), description: Some(draft.description.clone()), icon: draft.icon.clone(), group_id: source.group_id.clone(), enabled: Some(true), inputs: Some(draft.inputs.clone()), graph: Some(draft.graph.clone()), max_sends_per_hour: draft.max_sends_per_hour, ..Default::default() }).await?;
            applied.created.push(made.id.clone());
            ids.insert(marker, made.id);
            with_ids(&mut graph, ids);
            put_action(pool, PutActionInput { action_id: source.id.clone(), graph: Some(graph), ..Default::default() }).await?;
        }
        ActionOperation::Merge { action_ids, name, remove_sources, reference } => {
            let mut sources = Vec::new();
            for id in action_ids {
                sources.push(Action::get(pool, &resolved(id, ids)).await?);
            }
            let draft = merge_graphs(&sources, name);
            let group_id = sources.iter().find(|s| !s.builtin).and_then(|s| s.group_id.clone());
            let made = post_action(pool, profile_id, PostActionInput { name: draft.name.clone(), description: Some(draft.description.clone()), icon: draft.icon.clone(), group_id, enabled: Some(false), inputs: Some(draft.inputs.clone()), graph: Some(draft.graph.clone()), max_sends_per_hour: draft.max_sends_per_hour, ..Default::default() }).await?;
            applied.created.push(made.id.clone());
            ids.insert(placeholder(index, reference), made.id);
            if *remove_sources {
                for source in &sources {
                    applied.keep(source);
                    Action::delete(pool, &source.id).await?;
                }
            }
        }
        ActionOperation::Delete { action_id } => {
            let target = Action::get(pool, &resolved(action_id, ids)).await?;
            applied.keep(&target);
            Action::delete(pool, &target.id).await?;
        }
        ActionOperation::Move { action_ids, group_id } => {
            for id in action_ids {
                let target = Action::get(pool, &resolved(id, ids)).await?;
                applied.keep(&target);
                put_action(pool, PutActionInput { action_id: target.id.clone(), group_id: Some(group_id.clone()), ..Default::default() }).await?;
            }
        }
        ActionOperation::Rename { action_id, name } => {
            let target = Action::get(pool, &resolved(action_id, ids)).await?;
            applied.keep(&target);
            put_action(pool, PutActionInput { action_id: target.id.clone(), name: Some(name.clone()), ..Default::default() }).await?;
        }
        ActionOperation::Restore { action } => {
            match Action::get(pool, &action.id).await {
                Ok(current) => applied.keep(&current),
                Err(_) => applied.created.push(action.id.clone()),
            }
            restore_action(pool, action).await?;
        }
    }
    Ok(())
}

pub async fn put_actions(pool: &Pool<Sqlite>, profile_id: &str, operations: Vec<ActionOperation>) -> Result<ActionsOutcome, String> {
    let known = others(pool, profile_id).await?;
    let groups = ActionGroup::all(pool, profile_id).await?;
    let (_, errors) = plan_operations(&operations, &known, &groups, profile_id);
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    let mut ids = HashMap::new();
    let mut applied = Applied::default();
    for (index, operation) in operations.iter().enumerate() {
        if let Err(e) = apply_operation(pool, profile_id, index, operation, &mut ids, &mut applied).await {
            for action in &applied.before {
                restore_action(pool, action).await.ok();
            }
            for id in &applied.created {
                Action::delete(pool, id).await.ok();
            }
            return Err(format!("Nothing was changed; operation {} ({}) failed: {}", index + 1, operation.name(), e));
        }
    }
    let mut actions = Vec::new();
    for id in applied.created.iter().chain(applied.before.iter().map(|a| &a.id)) {
        if let Ok(action) = Action::get(pool, id).await {
            actions.push(action);
        }
    }
    let undo = applied.before.iter().cloned().map(|a| ActionOperation::Restore { action: Box::new(a) }).chain(applied.created.iter().map(|id| ActionOperation::Delete { action_id: id.clone() })).collect();
    Ok(ActionsOutcome { actions, undo, ids })
}

fn ports_line(ports: &[Port]) -> String {
    if ports.is_empty() {
        return "none".to_string();
    }
    ports.iter().map(|p| format!("{}: {}", p.name, p.kind.name())).collect::<Vec<_>>().join(", ")
}

fn sorted(actions: &HashMap<String, Action>) -> Vec<&Action> {
    let mut list: Vec<&Action> = actions.values().collect();
    list.sort_by(|a, b| (a.builtin, a.name.to_lowercase()).cmp(&(b.builtin, b.name.to_lowercase())));
    list
}

pub fn node_catalogue(catalog: &Catalog, actions: &HashMap<String, Action>) -> String {
    let on = [TriggerOn::Arrival, TriggerOn::Sent, TriggerOn::Edited, TriggerOn::Deleted, TriggerOn::Reaction, TriggerOn::Mention, TriggerOn::Filter, TriggerOn::Schedule, TriggerOn::EventSoon, TriggerOn::Manual, TriggerOn::Called, TriggerOn::Custom]
        .iter()
        .filter_map(|o| serde_json::to_value(o).ok().and_then(|v| v.as_str().map(String::from)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut out = vec![
        format!("Port types: {}.", catalog.types.join(", ")),
        format!("trigger: type \"trigger\". config.on is one of {}. config.scope narrows it: providers, connection_ids, conversation_ids, types, tag_ids, person_ids, unread, has_attachments, humans_only, from. config.group_ids names the conversation groups a filter trigger listens to, and filter needs one. config.direction is incoming, outgoing or any. config.message_kinds lists message kinds. config.schedule is {{ every_minutes, at as HH:MM, days as weekday numbers 1 for Monday to 7 for Sunday }}, and schedule needs every_minutes or at. config.minutes_before belongs to event_soon. config.disabled switches it off. Outputs: {}, plus reaction: text on reaction, event: json on event_soon, at: text on schedule, and the action inputs on manual and called. body is an optional Rhai predicate over message, conversation, person and text that decides whether the action runs; it never changes anything.", on, ports_line(&trigger_outputs(TriggerOn::Arrival))),
        "A custom trigger (config.on custom) watches the outside world through config.watch: mode webhook receives a POST at the address the inspector shows, mode poll GETs url every every_minutes (5 by default) with optional headers. pick is a JSON pointer such as /status or data.state that names the part that counts; the action runs only when that part changes, and a webhook runs on its first delivery. token, when set, must arrive as the token query or the X-Chatnip-Token header. It outputs payload (the whole body), value (the picked part), previous (the value before) and text. Post status writes the change into a local chat as a status card.".to_string(),
        "script: type \"script\". config.inputs and config.outputs are port lists [{ name, type, description }]; body is Rhai returning a map with one key per output, such as #{ summary: summarize(text) }.".to_string(),
        format!("chat: type \"chat\". {} config.mode is {}; config.conversation_id is a chat id, or null for the chat the action is attached to.", catalog.chat_node.description, catalog.chat_node.variants.keys().cloned().collect::<Vec<_>>().join(" or ")),
    ];
    for (mode, variant) in &catalog.chat_node.variants {
        out.push(format!("  chat in {} mode: {} Inputs: {}. Outputs: {}.", mode, variant.description, ports_line(&variant.inputs), ports_line(&variant.outputs)));
    }
    out.push("Built in nodes, type \"builtin:<key>\"; the app fills in the body, and config.values holds constant inputs:".to_string());
    for b in &catalog.builtins {
        out.push(format!("- builtin:{} ({}, a {}): {} Inputs: {}. Outputs: {}.", b.key, b.label, b.kind, b.description, ports_line(&b.inputs), ports_line(&b.outputs)));
    }
    out.push("Actions usable as nodes, type \"action:<id>\":".to_string());
    for a in sorted(actions) {
        out.push(format!("- action:{} {:?}, a {}: inputs {}; outputs {}.", a.id, a.name, action_role(a), if action_role(a) == KIND_TRIGGER { "none".to_string() } else { ports_line(&a.inputs) }, ports_line(&a.outputs)));
    }
    out.push("Functions for script bodies and trigger predicates:".to_string());
    for f in &catalog.functions {
        out.push(format!("- {}: {}{}", f.signature, f.description, if f.effect { " Changes things, so never in a trigger." } else { "" }));
    }
    out.push("Tags, for tag inputs and config.scope.tag_ids:".to_string());
    for tag in &catalog.tags {
        out.push(format!("- {} ({}) id {}", text(tag.get("name")), text(tag.get("kind")), text(tag.get("id"))));
    }
    out.push("Conversation groups, for config.group_ids and builtin:group_add:".to_string());
    for group in &catalog.groups {
        out.push(format!("- {} id {}", text(group.get("name")), text(group.get("id"))));
    }
    out.join("\n")
}

fn action_listing(actions: &HashMap<String, Action>) -> String {
    sorted(actions)
        .into_iter()
        .map(|a| format!("- {} {:?}{} in {}, {}, a {}: ({}) gives ({})", a.id, a.name, if a.builtin { " built in, read only" } else { "" }, a.group_id.as_deref().unwrap_or("the top level"), if a.enabled { "on" } else { "off" }, action_role(a), ports_line(&a.inputs), ports_line(&a.outputs)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn group_listing(groups: &[ActionGroup]) -> String {
    groups.iter().map(|g| format!("- {} {:?}{} in {}", g.id, g.name, if g.builtin { " built in, read only" } else { "" }, g.parent_id.as_deref().unwrap_or("the top level"))).collect::<Vec<_>>().join("\n")
}

fn action_json(action: &Action, draft: Option<&ActionDraft>) -> Value {
    let shown = draft.cloned().unwrap_or_else(|| draft_of(action));
    json!({ "id": action.id, "name": shown.name, "description": shown.description, "icon": shown.icon, "group_id": action.group_id, "enabled": action.enabled, "max_sends_per_hour": shown.max_sends_per_hour, "inputs": shown.inputs, "graph": shown.graph })
}

pub const ASSIST_DETAIL_LIMIT: usize = 20;

async fn assist_prompt(pool: &Pool<Sqlite>) -> Value {
    AppSetting::get(pool, app_setting::ASSISTANT_PROMPT).await.ok().flatten().and_then(|raw| serde_json::from_str(&raw).ok()).unwrap_or(Value::Null)
}

pub async fn assist(pool: &Pool<Sqlite>, profile_id: &str, input: AssistInput, ai: Option<AiStub>) -> Result<AssistOutcome, String> {
    let known = others(pool, profile_id).await?;
    let groups = ActionGroup::all(pool, profile_id).await?;
    let listed = catalog(pool, profile_id).await?;
    let scoped = match &input.action_id {
        Some(id) => Some(Action::find(pool, profile_id, id).await?),
        None => None,
    };
    let mine: Vec<Value> = sorted(&known).into_iter().filter(|a| !a.builtin).map(|a| action_json(a, None)).collect();
    let detail = if scoped.is_none() && mine.len() <= ASSIST_DETAIL_LIMIT { Value::Array(mine).to_string() } else { String::new() };
    let mut extra = json!({
        "instruction": input.instruction,
        "catalogue": node_catalogue(&listed, &known),
        "actions": action_listing(&known),
        "groups": group_listing(&groups),
        "scope": scoped.as_ref().map(|a| action_json(a, input.draft.as_ref()).to_string()).unwrap_or_default(),
        "detail": detail,
        "messages": input.messages,
        "errors": [],
        "previous": "",
    });
    let overrides = assist_prompt(pool).await;
    let choice = scoped.as_ref().map(|a| a.ai.clone()).filter(|c| !c.is_empty());
    let mut outcome = AssistOutcome::default();
    for _ in 0..2 {
        let answer = match &ai {
            Some(stub) => stub("author_action", &extra),
            None => intelligence_service::run(pool, "author_action", &overrides, None, extra.clone(), choice.as_ref()).await.map(|run| run.output),
        };
        let output = match answer {
            Ok(output) => output,
            Err(e) => match action_script::unmet_of(&e) {
                Some(needed) => return Ok(AssistOutcome { requirement: Some(needed), ..AssistOutcome::default() }),
                None => return Err(e),
            },
        };
        let errors = match parse_operations(&output) {
            Ok((reply, operations)) => {
                let (previews, errors) = plan_operations(&operations, &known, &groups, profile_id);
                if errors.is_empty() {
                    return Ok(AssistOutcome { reply, operations, previews, errors, requirement: None });
                }
                outcome.reply = reply;
                errors
            }
            Err(errors) => {
                outcome.reply = text(output.get("reply"));
                errors
            }
        };
        extra["errors"] = json!(errors);
        extra["previous"] = json!(output.to_string());
        outcome.errors = errors;
    }
    Ok(outcome)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TemplateAssistInput {
    #[serde(default)]
    pub action_id: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    pub instruction: String,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateAssistOutcome {
    pub source: String,
    pub explanation: String,
    pub requirement: Option<Diagnostic>,
}

pub async fn assist_template(pool: &Pool<Sqlite>, profile_id: &str, input: TemplateAssistInput, ai: Option<AiStub>) -> Result<TemplateAssistOutcome, String> {
    let (kind, stored, guidance) = match (input.action_id.as_deref(), input.node_id.as_deref()) {
        (Some(action_id), Some(node_id)) => {
            let action = Action::find(pool, profile_id, action_id).await?;
            let (kind, stored) = template_source(&action, node_id)?;
            let guidance = action.graph.nodes.iter().find(|n| n.id == node_id).and_then(|n| n.config.get("prompt")).and_then(|p| p.as_str()).unwrap_or("").to_string();
            (kind, stored, guidance)
        }
        _ => (input.kind.clone().ok_or("Say which kind of message the layout is for")?, String::new(), String::new()),
    };
    let extra = json!({ "instruction": input.instruction, "kind": kind, "source": input.source.unwrap_or(stored), "guidance": guidance });
    let answer = match ai {
        Some(stub) => stub("author_template", &extra),
        None => intelligence_service::run(pool, "author_template", &Value::Null, None, extra.clone(), None).await.map(|run| run.output),
    };
    let output = match answer {
        Ok(output) => output,
        Err(e) => match action_script::unmet_of(&e) {
            Some(needed) => return Ok(TemplateAssistOutcome { source: text(extra.get("source")), explanation: String::new(), requirement: Some(needed) }),
            None => return Err(e),
        },
    };
    Ok(TemplateAssistOutcome { source: text(output.get("source")), explanation: text(output.get("explanation")), requirement: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::action::PostActionGroupInput;
    use crate::test_helpers::{test_conversation, test_pool, test_profile, test_provider};

    fn node(id: &str, kind: &str, config: Value, body: &str) -> Node {
        Node { id: id.into(), kind: kind.into(), label: String::new(), config, body: body.into(), x: 0.0, y: 0.0 }
    }

    fn edge(source: &str, sp: &str, target: &str, tp: &str) -> Edge {
        Edge { id: format!("{}.{}-{}.{}", source, sp, target, tp), source: source.into(), source_port: sp.into(), target: target.into(), target_port: tp.into() }
    }

    async fn seed(pool: &Pool<Sqlite>) -> (String, String, String, String) {
        let profile = test_profile(pool).await;
        let provider = test_provider(pool, &profile, "mock").await;
        let conversation = test_conversation(pool, "Kunde AS", "email").await;
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id, is_primary) VALUES (?1, ?2, ?3, ?2, 1)").bind(uuid::Uuid::new_v4().to_string()).bind(&conversation).bind(&provider).execute(pool).await.unwrap();
        let message = crate::test_helpers::test_message(pool, &conversation, &provider, "Invoice 42 is overdue").await;
        sqlx::query("UPDATE message SET sender_email = 'anna@client.example', sender_name = 'Anna', subject = 'Invoice' WHERE id = ?1").bind(&message).execute(pool).await.unwrap();
        Tag::ensure_defaults(pool, &profile).await.unwrap();
        (profile, provider, conversation, message)
    }


    #[tokio::test(flavor = "multi_thread")]
    async fn invites_are_recognised_and_presented_by_the_default_action() {
        let pool = test_pool().await;
        let (profile, provider, conversation, message) = seed(&pool).await;
        sqlx::query("UPDATE message SET type = 'email', subject = 'Sprintdemo', content = 'Microsoft Teams-møte. Bli med i møtet nå https://teams.microsoft.com/l/meetup-join/19%3ameeting_x/0 Fre 16. okt 14:00' WHERE id = ?1").bind(&message).execute(&pool).await.unwrap();
        let plain = crate::test_helpers::test_message(&pool, &conversation, &provider, "Faktura vedlagt").await;
        sqlx::query("UPDATE message SET type = 'email', subject = 'Faktura' WHERE id = ?1").bind(&plain).execute(&pool).await.unwrap();
        ensure_defaults(&pool, &profile).await.unwrap();
        let action = Action::find(&pool, &profile, "recognise_invites").await.unwrap();
        assert!(action.enabled);
        let shown = crate::models::presentation::get_message(&pool, &message).await.unwrap();
        assert_eq!(shown.effective["kind"], "invite");
        assert_eq!(shown.effective["invite"]["provider"], "teams");
        assert_eq!(shown.effective["invite"]["organizer"], "Anna");
        let untouched = crate::models::presentation::get_message(&pool, &plain).await.unwrap();
        assert!(untouched.effective.get("kind").is_none());

        let unsure = crate::test_helpers::test_message(&pool, &conversation, &provider, "Vi møtes i rom 4 på torsdag kl 10:00.").await;
        sqlx::query("UPDATE message SET type = 'email', subject = 'Møteinnkalling: Budsjett 2027' WHERE id = ?1").bind(&unsure).execute(&pool).await.unwrap();
        let mut dry = runner(&pool, &profile, true);
        dry.ai = Some(Arc::new(|_: &str, _: &Value| Ok(json!({ "invite": true, "title": "Budsjett 2027", "starts_at": "2026-10-22T08:00:00Z" }))));
        let runs = dispatch_to(&dry, &[TriggerOn::Arrival], &unsure, Some(&action.id)).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert!(runs[0].log.iter().any(|l| l.status == "would" && l.detail.starts_with("present(")), "{:?}", runs[0].log);
        assert!(crate::models::presentation::get_message(&pool, &unsure).await.unwrap().effective.get("kind").is_none());

        let mut live = runner(&pool, &profile, false);
        live.ai = dry.ai.clone();
        dispatch_to(&live, &[TriggerOn::Arrival], &unsure, Some(&action.id)).await.unwrap();
        let shown = crate::models::presentation::get_message(&pool, &unsure).await.unwrap();
        assert_eq!(shown.effective["kind"], "invite");
        assert_eq!(shown.effective["invite"]["starts_at"], "2026-10-22T08:00:00Z");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn scripts_present_messages_and_style_conversations() {
        let pool = test_pool().await;
        let (profile, _, conversation, message) = seed(&pool).await;
        let mut action = Action::blank(&profile, "Kunde look");
        action.graph = Graph {
            nodes: vec![
                trigger_node("trigger", "Arrives", json!({ "on": "arrival" }), "", 0.0),
                script_node("look", "Look", json!([{ "name": "message", "type": "message" }, { "name": "conversation", "type": "conversation" }]), json!([]), "highlight(message, \"#e0612f\");\ncollapse(message);\npresent_conversation(conversation, #{ kind: \"mail\", badge: \"Kunde\" });\nstyle_conversation(conversation, #{ accent: \"#e0612f\", surface: \"glass\" });\n#{}", 0.0),
            ],
            edges: vec![wire("trigger", "message", "look", "message"), wire("trigger", "conversation", "look", "conversation")],
        };
        normalize(&mut action.graph);
        Action::insert(&pool, &action).await.unwrap();
        let runs = dispatch_to(&runner(&pool, &profile, false), &[TriggerOn::Arrival], &message, Some(&action.id)).await.unwrap();
        assert_eq!(runs[0].status, "done", "{:?}", runs[0]);
        let shown = crate::models::presentation::get_message(&pool, &message).await.unwrap();
        assert_eq!(shown.effective["highlight"], "#e0612f");
        assert_eq!(shown.effective["collapsed"], true);
        assert_eq!(shown.effective["kind"], "mail");
        assert_eq!(shown.effective["badge"], "Kunde");
        let look: Value = serde_json::from_str(&crate::models::AppSetting::get(&pool, &format!("appearance.conversation.{}", conversation)).await.unwrap().unwrap()).unwrap();
        assert_eq!(look, json!({ "accent": "#e0612f", "surface": "glass" }));
    }


    #[tokio::test]
    async fn scripts_use_prompts_and_replies_from_content() {
        let pool = test_pool().await;
        let (profile, _, _, message) = seed(&pool).await;
        let dir = std::env::temp_dir().join(format!("chatnip-content-{}", Uuid::new_v4()));
        let post = |kind: &str, name: &str, body: &str| crate::models::content::PostContentInput { kind: kind.into(), name: name.into(), body: Some(body.into()), ..Default::default() };
        let decline = content_service::store(&pool, &dir, &profile, post(KIND_PROMPT, "Polite no", "Say no to {{ .conversation.name }}")).await.unwrap();
        let thanks = content_service::store(&pool, &dir, &profile, post("reply", "Thanks", "Takk, {{ .conversation.name | lower }}{{ .extra }}")).await.unwrap();
        let build = |body: &str, config: Value| {
            let mut action = Action::blank(&profile, "Answer from content");
            let mut look = script_node("use", "Use", json!([{ "name": "conversation", "type": "conversation" }]), json!([]), body, 0.0);
            if let Value::Object(extra) = config {
                if let Value::Object(map) = &mut look.config {
                    map.extend(extra);
                }
            }
            action.graph = Graph { nodes: vec![trigger_node("trigger", "Arrives", json!({ "on": "arrival" }), "", 0.0), look], edges: vec![wire("trigger", "conversation", "use", "conversation")] };
            normalize(&mut action.graph);
            action
        };
        let action = build("let a = prompt(\"polite no\", #{ conversation: conversation });\nlet t = content(\"Thanks\", #{ conversation: conversation, extra: \"!\" });\nif t != \"Takk, kunde as!\" { throw t; }\n#{}", Value::Null);
        Action::insert(&pool, &action).await.unwrap();
        let runs = dispatch_to(&runner(&pool, &profile, false), &[TriggerOn::Arrival], &message, Some(&action.id)).await.unwrap();
        assert_eq!(runs[0].status, "done", "{:?}", runs[0]);
        assert_eq!(ContentItem::get(&pool, &thanks.id).await.unwrap().unwrap().used_count, 1);
        let linked = build("#{ answer: ai(\"anything\") }", json!({ "prompts": { "script_ai": { "content_id": decline.id, "template": "dropped" } } }));
        assert_eq!(linked.graph.nodes[1].config["prompts"]["script_ai"], json!({ "content_id": decline.id }));
        Action::insert(&pool, &linked).await.unwrap();
        let runs = dispatch_to(&runner(&pool, &profile, false), &[TriggerOn::Arrival], &message, Some(&linked.id)).await.unwrap();
        assert_eq!(runs[0].status, "done", "{:?}", runs[0]);
        let gone = build("#{ answer: ai(\"anything\") }", json!({ "prompts": { "script_ai": { "content_id": "missing" } } }));
        Action::insert(&pool, &gone).await.unwrap();
        let runs = dispatch_to(&runner(&pool, &profile, false), &[TriggerOn::Arrival], &message, Some(&gone.id)).await.unwrap();
        assert!(runs[0].error.as_deref().unwrap_or("").contains("is gone"), "{:?}", runs[0]);
        let unknown = build("let a = prompt(\"Nope\", #{});\n#{}", Value::Null);
        Action::insert(&pool, &unknown).await.unwrap();
        let runs = dispatch_to(&runner(&pool, &profile, false), &[TriggerOn::Arrival], &message, Some(&unknown.id)).await.unwrap();
        assert!(runs[0].error.as_deref().unwrap_or("").contains("no AI prompt in Content, called Nope"), "{:?}", runs[0]);
        let uses = content_service::uses(&pool, &profile, &decline).await.unwrap();
        let mut via: Vec<&str> = uses.iter().map(|u| u.via.as_str()).collect();
        via.sort();
        assert_eq!(via, vec!["prompt", "script"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_single_node_runs_alone_and_a_template_is_recorded_on_the_message() {
        let pool = test_pool().await;
        let (profile, _, _, message) = seed(&pool).await;
        ensure_defaults(&pool, &profile).await.unwrap();
        let mut action = Action::find(&pool, &profile, "recognise_invites").await.unwrap();
        let show = action.graph.nodes.iter_mut().find(|n| n.id == "show").unwrap();
        show.config = json!({ "template": "<p>{message.subject}</p>" });
        Action::save(&pool, &action).await.unwrap();
        let single = isolate_node(&pool, &profile, &action, "show").await.unwrap();
        assert_eq!(single.graph.nodes.len(), 1);
        assert_eq!(single.inputs.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["message", "options"]);
        assert!(isolate_node(&pool, &profile, &action, "arrival").await.is_err());
        let inputs = Map::from_iter([("message".to_string(), json!({ "id": message })), ("options".to_string(), json!({ "badge": "Møte" }))]);
        let dry = execute(&runner(&pool, &profile, true), &single, Start::Invoke { inputs: inputs.clone(), payload: Value::Null }, 0, vec![action.id.clone()]).await.unwrap();
        assert_eq!(dry.status, "done", "{:?}", dry);
        assert!(crate::models::presentation::get_message(&pool, &message).await.unwrap().effective.get("kind").is_none());
        let live = execute(&runner(&pool, &profile, false), &single, Start::Invoke { inputs, payload: Value::Null }, 0, vec![action.id.clone()]).await.unwrap();
        assert_eq!(live.status, "done", "{:?}", live);
        let shown = crate::models::presentation::get_message(&pool, &message).await.unwrap();
        assert_eq!(shown.effective["kind"], "invite");
        assert_eq!(shown.effective["badge"], "Møte");
        assert_eq!(shown.effective["template"], json!({ "action_id": action.id, "node_id": "show" }));
        assert_eq!(template_source(&action, "show").unwrap(), ("invite".to_string(), "<p>{message.subject}</p>".to_string()));
        let failed = template_failure(&action, "show", &message, "Unexpected token");
        assert_eq!(failed.status, "failed");
        assert!(failed.error.unwrap().contains("Unexpected token"));
    }

    #[tokio::test]
    async fn the_template_assistant_sees_the_node_prompt_and_current_source() {
        let pool = test_pool().await;
        let (profile, _, _, _) = seed(&pool).await;
        ensure_defaults(&pool, &profile).await.unwrap();
        let mut action = Action::find(&pool, &profile, "recognise_invites").await.unwrap();
        let show = action.graph.nodes.iter_mut().find(|n| n.id == "show").unwrap();
        show.config = json!({ "template": "<p>{message.subject}</p>", "prompt": "Keep it to one line" });
        Action::save(&pool, &action).await.unwrap();
        let stub: AiStub = Arc::new(|key: &str, extra: &Value| {
            assert_eq!(key, "author_template");
            assert_eq!(extra["kind"], "invite");
            assert_eq!(extra["guidance"], "Keep it to one line");
            assert_eq!(extra["source"], "<p>{message.subject}</p>");
            Ok(json!({ "source": "<strong>{message.subject}</strong>", "explanation": "Bolder title" }))
        });
        let input = TemplateAssistInput { action_id: Some(action.id.clone()), node_id: Some("show".into()), kind: None, instruction: "Make it bold".into(), source: None };
        let outcome = assist_template(&pool, &profile, input, Some(stub)).await.unwrap();
        assert_eq!(outcome.source, "<strong>{message.subject}</strong>");
        assert_eq!(outcome.explanation, "Bolder title");
    }

    fn known_of(actions: Vec<Action>) -> HashMap<String, Action> {
        let mut all = actions;
        sign(&mut all);
        all.into_iter().map(|a| (a.id.clone(), a)).collect()
    }

    fn group(id: &str, builtin: bool) -> ActionGroup {
        ActionGroup { id: id.into(), profile_id: "p".into(), key: None, builtin, parent_id: None, name: id.into(), description: String::new(), icon: None, position: 0 }
    }

    #[test]
    fn operations_parse_strictly_and_get_their_edge_ids() {
        let good = json!({ "reply": "Adds a tag step", "operations": [{ "op": "create", "ref": "tagger", "action": { "name": "Tagger", "graph": { "nodes": [{ "id": "t", "type": "trigger", "config": { "on": "arrival" } }, { "id": "tag", "type": "builtin:tag_add", "config": { "values": { "tag": "Important" } } }], "edges": [{ "source": "t", "source_port": "conversation", "target": "tag", "target_port": "conversation" }] } } }, { "op": "move", "action_ids": ["tagger"], "group_id": null }] });
        let (reply, operations) = parse_operations(&good).unwrap();
        assert_eq!(reply, "Adds a tag step");
        assert_eq!(operations.len(), 2);
        let ActionOperation::Create { reference, action, .. } = &operations[0] else { panic!("not a create") };
        assert_eq!(reference.as_deref(), Some("tagger"));
        assert_eq!(action.graph.edges[0].id, "t.conversation-tag.conversation");
        assert!(matches!(&operations[1], ActionOperation::Move { group_id: None, .. }));
        let errors = parse_operations(&json!({ "operations": [{ "op": "explode" }, { "op": "restore", "action": {} }, { "op": "rename", "action_id": "a" }] })).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("reply")), "{:?}", errors);
        assert!(errors.iter().any(|e| e.contains("operation 1") && e.contains("explode")), "{:?}", errors);
        assert!(errors.iter().any(|e| e.contains("operation 2") && e.contains("restore")), "{:?}", errors);
        assert!(errors.iter().any(|e| e.contains("operation 3 (rename)") && e.contains("name")), "{:?}", errors);
        let typed = parse_operations(&json!({ "reply": "x", "operations": [{ "op": "create", "action": { "name": "x", "inputs": [{ "name": "a", "type": "colour" }] } }] })).unwrap_err();
        assert!(typed[0].contains("colour"), "{:?}", typed);
        assert!(parse_operations(&Value::String("Sure, here you go".into())).is_err());
        assert!(parse_operations(&json!({ "reply": "Nothing to change", "operations": [] })).unwrap().1.is_empty());
    }

    #[test]
    fn the_plan_refuses_unknown_nodes_missing_ports_bad_wiring_and_read_only_targets() {
        let mut builtin = Action::blank("p", "Shipped");
        builtin.builtin = true;
        let mine = reply_action("p");
        let mut host = Action::blank("p", "Host");
        host.graph = Graph { nodes: vec![node("inner", &format!("{}{}", ACTION_PREFIX, mine.id), json!({}), "")], edges: vec![] };
        let known = known_of(vec![builtin.clone(), mine.clone(), host.clone()]);
        let groups = vec![group("mine", false), group("shipped", true)];
        let create = |graph: Value| serde_json::from_value::<ActionOperation>(json!({ "op": "create", "action": { "name": "New", "graph": graph } })).unwrap();
        let (_, errors) = plan_operations(&[create(json!({ "nodes": [{ "id": "x", "type": "builtin:nope" }], "edges": [] }))], &known, &groups, "p");
        assert!(errors.iter().any(|e| e.contains("not a known node type")), "{:?}", errors);
        let (_, errors) = plan_operations(&[create(json!({ "nodes": [{ "id": "t", "type": "trigger", "config": { "on": "arrival" } }, { "id": "d", "type": "builtin:done" }], "edges": [{ "id": "e", "source": "t", "source_port": "conversation", "target": "d", "target_port": "chat" }] }))], &known, &groups, "p");
        assert!(errors.iter().any(|e| e.contains("has no input chat")), "{:?}", errors);
        let (_, errors) = plan_operations(&[create(json!({ "nodes": [{ "id": "t", "type": "trigger", "config": { "on": "arrival" } }, { "id": "s", "type": "builtin:summarize" }], "edges": [{ "id": "e", "source": "t", "source_port": "message", "target": "s", "target_port": "text" }] }))], &known, &groups, "p");
        assert!(errors.iter().any(|e| e.contains("gives message, but text needs text")), "{:?}", errors);
        let (_, errors) = plan_operations(&[ActionOperation::Rename { action_id: builtin.id.clone(), name: "Mine now".into() }], &known, &groups, "p");
        assert!(errors[0].contains("read only"), "{:?}", errors);
        let (_, errors) = plan_operations(&[ActionOperation::Move { action_ids: vec![mine.id.clone()], group_id: Some("shipped".into()) }], &known, &groups, "p");
        assert!(errors[0].contains("built in group"), "{:?}", errors);
        let (_, errors) = plan_operations(&[ActionOperation::Delete { action_id: mine.id.clone() }], &known, &groups, "p");
        assert!(errors.iter().any(|e| e.contains("still uses")), "{:?}", errors);
        let (_, errors) = plan_operations(&[ActionOperation::Delete { action_id: "nobody".into() }], &known, &groups, "p");
        assert!(errors[0].contains("no action nobody"), "{:?}", errors);
        let (previews, errors) = plan_operations(&[ActionOperation::Copy { action_id: builtin.id.clone(), name: None, without_trigger: false, reference: Some("mine".into()) }, ActionOperation::Rename { action_id: "mine".into(), name: "Changed".into() }, ActionOperation::Move { action_ids: vec!["mine".into()], group_id: Some("mine".into()) }], &known, &groups, "p");
        assert!(errors.is_empty(), "{:?}", errors);
        assert_eq!(previews.iter().map(|p| p.op).collect::<Vec<_>>(), vec!["copy", "rename", "move"]);
        assert_eq!(previews[1].after.as_ref().unwrap().name, "Changed");
        assert_eq!(previews[2].group_after.as_deref(), Some("mine"));
    }

    #[test]
    fn a_split_moves_nodes_into_a_called_action_and_a_merge_joins_graphs() {
        let source = reply_action("p");
        let known = known_of(vec![source.clone()]);
        let (part, rest) = split_graph(&source, &["draft".into(), "send".into()], "Answer", "part", &known).unwrap();
        let start = part.graph.nodes.iter().find(|n| n.kind == NODE_TRIGGER).unwrap();
        assert_eq!(start.trigger().unwrap().on, TriggerOn::Called);
        assert_eq!(part.inputs.iter().map(|p| (p.name.as_str(), p.kind)).collect::<Vec<_>>(), vec![("conversation", PortType::Conversation)]);
        assert_eq!(part.graph.edges.iter().filter(|e| e.source == start.id).count(), 2);
        let host = rest.nodes.iter().find(|n| n.kind == "action:part").unwrap();
        assert_eq!(host.label, "Answer");
        assert!(rest.nodes.iter().all(|n| n.id != "draft" && n.id != "send"));
        assert!(rest.edges.iter().any(|e| e.source == "t" && e.target == host.id && e.target_port == "conversation"));
        assert!(rest.edges.iter().any(|e| e.target == "tag"));
        assert!(split_graph(&source, &["t".into(), "draft".into(), "send".into(), "tag".into()], "All", "part", &known).is_err());
        assert!(split_graph(&source, &["t".into(), "draft".into()], "Half", "part", &known).unwrap_err().contains("t feeds both parts"));
        let (previews, errors) = plan_operations(&[ActionOperation::Split { action_id: source.id.clone(), node_ids: vec!["draft".into(), "send".into()], name: "Answer".into(), reference: None }], &known, &[], "p");
        assert!(errors.is_empty(), "{:?}", errors);
        assert_eq!(previews.len(), 2);
        assert_eq!(previews[0].action_id, "new-1");
        let mut other = reply_action("p");
        other.name = "Second".into();
        let merged = merge_graphs(&[source.clone(), other.clone()], "Both");
        assert_eq!(merged.graph.nodes.len(), source.graph.nodes.len() * 2);
        assert_eq!(merged.graph.edges.len(), source.graph.edges.len() * 2);
        let ids: HashSet<&str> = merged.graph.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids.len(), merged.graph.nodes.len());
        assert!(ids.contains("second_t"));
        assert!(merged.graph.edges.iter().all(|e| ids.contains(e.source.as_str()) && ids.contains(e.target.as_str())));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn applied_operations_write_through_the_action_paths_and_undo_restores_them() {
        let pool = test_pool().await;
        let (profile, _, _, _) = seed(&pool).await;
        let source = post_action(&pool, &profile, PostActionInput { name: "Acknowledge".into(), graph: Some(reply_action(&profile).graph), ..Default::default() }).await.unwrap();
        let spare = post_action(&pool, &profile, PostActionInput { name: "Spare".into(), ..Default::default() }).await.unwrap();
        let folder = ActionGroup::post(&pool, &profile, &PostActionGroupInput { name: "Mail".into(), parent_id: None, description: None, icon: None }).await.unwrap();
        let operations: Vec<ActionOperation> = serde_json::from_value(json!([
            { "op": "create", "ref": "fresh", "action": { "name": "Fresh", "graph": { "nodes": [{ "id": "t", "type": "trigger", "config": { "on": "manual" } }], "edges": [] } } },
            { "op": "move", "action_ids": ["fresh", source.id], "group_id": folder.id },
            { "op": "rename", "action_id": source.id, "name": "Acknowledge politely" },
            { "op": "split", "action_id": source.id, "node_ids": ["draft", "send"], "name": "Answer" },
            { "op": "delete", "action_id": spare.id }
        ]))
        .unwrap();
        let outcome = put_actions(&pool, &profile, operations).await.unwrap();
        let all = Action::all(&pool, &profile).await.unwrap();
        let fresh = all.iter().find(|a| a.name == "Fresh").unwrap();
        assert!(!fresh.enabled);
        assert_eq!(fresh.group_id.as_deref(), Some(folder.id.as_str()));
        let renamed = all.iter().find(|a| a.id == source.id).unwrap();
        assert_eq!(renamed.name, "Acknowledge politely");
        assert_eq!(renamed.group_id.as_deref(), Some(folder.id.as_str()));
        let part = all.iter().find(|a| a.name == "Answer").unwrap();
        assert!(renamed.graph.nodes.iter().any(|n| n.kind == format!("{}{}", ACTION_PREFIX, part.id)));
        assert!(!all.iter().any(|a| a.id == spare.id));
        assert!(outcome.actions.iter().any(|a| a.id == part.id));
        put_actions(&pool, &profile, outcome.undo).await.unwrap();
        let back = Action::all(&pool, &profile).await.unwrap();
        assert!(!back.iter().any(|a| a.name == "Fresh" || a.name == "Answer"));
        let restored = back.iter().find(|a| a.id == source.id).unwrap();
        assert_eq!(restored.name, "Acknowledge");
        assert_eq!(restored.group_id, None);
        assert_eq!(restored.graph, source.graph);
        assert!(back.iter().any(|a| a.id == spare.id && a.name == "Spare"));
        let refused = put_actions(&pool, &profile, vec![ActionOperation::Rename { action_id: source.id.clone(), name: " ".into() }]).await.unwrap_err();
        assert!(refused.contains("needs a name"), "{}", refused);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_assistant_retries_once_with_the_errors_then_shows_them_and_reports_missing_setup() {
        let pool = test_pool().await;
        let (profile, _, _, _) = seed(&pool).await;
        let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
        let seen = calls.clone();
        let stub: AiStub = Arc::new(move |key: &str, extra: &Value| {
            assert_eq!(key, "author_action");
            seen.lock().unwrap().push(extra.clone());
            let kind = if seen.lock().unwrap().len() == 1 { "builtin:nope" } else { "builtin:done" };
            Ok(json!({ "reply": "Marks it done", "operations": [{ "op": "create", "action": { "name": "Done", "graph": { "nodes": [{ "id": "d", "type": kind }], "edges": [] } } }] }))
        });
        let input = AssistInput { instruction: "Mark things done".into(), messages: vec![AssistMessage { role: "user".into(), content: "Hi".into(), status: None }, AssistMessage { role: "assistant".into(), content: "{\"reply\":\"Hello\",\"operations\":[]}".into(), status: Some("discarded".into()) }], ..Default::default() };
        let outcome = assist(&pool, &profile, input.clone(), Some(stub)).await.unwrap();
        assert!(outcome.errors.is_empty(), "{:?}", outcome.errors);
        assert_eq!(outcome.operations.len(), 1);
        let sent = calls.lock().unwrap().clone();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0]["messages"][1]["status"], "discarded");
        assert!(sent[0]["catalogue"].as_str().unwrap().contains("builtin:tag_add"));
        assert!(sent[0]["errors"].as_array().unwrap().is_empty());
        assert!(sent[1]["errors"][0].as_str().unwrap().contains("not a known node type"));
        assert!(sent[1]["previous"].as_str().unwrap().contains("builtin:nope"));
        let stubborn: AiStub = Arc::new(|_: &str, _: &Value| Ok(json!({ "reply": "Here", "operations": [{ "op": "delete", "action_id": "nobody" }] })));
        let failed = assist(&pool, &profile, input.clone(), Some(stubborn)).await.unwrap();
        assert!(failed.operations.is_empty() && failed.previews.is_empty());
        assert!(failed.errors[0].contains("no action nobody"), "{:?}", failed.errors);
        let unset: AiStub = Arc::new(|_: &str, _: &Value| Err(action_script::REQUIREMENT_AI_CONNECTION.error()));
        let blocked = assist(&pool, &profile, input, Some(unset)).await.unwrap();
        assert_eq!(blocked.requirement.unwrap().setting.as_deref(), Some(action_script::REQUIRES_AI_CONNECTION));
    }

    fn runner(pool: &Pool<Sqlite>, profile: &str, dry_run: bool) -> Runner {
        Runner {
            test_delay: None,
            pool: pool.clone(),
            app: None,
            app_data: std::env::temp_dir(),
            cache: std::env::temp_dir().join("chatnip-test-cache"),
            profile_id: profile.into(),
            dry_run,
            ai: Some(Arc::new(|key: &str, extra: &Value| match key {
                "script_ai_json" => Ok(json!({ "label": "Important", "summary": "An invoice is overdue", "yes": extra["prompt"].as_str().unwrap_or("").contains("ask") })),
                "reply_draft" => Ok(json!({ "reply": "Thanks, we will pay today" })),
                "author_action" => Ok(json!({ "reply": "Adds a trigger", "operations": [{ "op": "create", "action": { "name": "Thanks", "graph": { "nodes": [{ "id": "t", "type": "trigger", "config": { "on": "arrival" } }], "edges": [] } } }] })),
                _ => Ok(json!({ "answer": "ok" })),
            })),
        }
    }

    fn reply_action(profile: &str) -> Action {
        let mut action = Action::blank(profile, "Acknowledge invoices");
        action.graph = Graph {
            nodes: vec![
                node("t", NODE_TRIGGER, json!({ "on": "arrival", "scope": { "providers": ["mock"] } }), "regex_match(text, \"invoice\")"),
                node("draft", "builtin:ai_reply", json!({ "values": {} }), ""),
                node("send", NODE_SCRIPT, json!({ "inputs": [{ "name": "conversation", "type": "conversation" }, { "name": "text", "type": "text" }], "outputs": [{ "name": "message", "type": "message" }] }), "#{ message: reply(conversation, text) }"),
                node("tag", "builtin:tag_add", json!({ "values": { "tag": "Important" } }), ""),
            ],
            edges: vec![edge("t", "conversation", "draft", "conversation"), edge("t", "conversation", "send", "conversation"), edge("draft", "text", "send", "text"), edge("t", "conversation", "tag", "conversation")],
        };
        normalize(&mut action.graph);
        action
    }

    fn names(ports: &[Port]) -> Vec<(String, String, String)> {
        ports.iter().map(|p| (p.name.clone(), p.node.clone().unwrap_or_default(), p.source_port.clone().unwrap_or_default())).collect()
    }

    fn script(id: &str, label: &str, inputs: Value, outputs: Value) -> Node {
        Node { label: label.into(), ..node(id, NODE_SCRIPT, json!({ "inputs": inputs, "outputs": outputs }), "#{}") }
    }

    #[test]
    fn the_last_node_gives_the_actions_outputs() {
        let graph = Graph {
            nodes: vec![
                node("t", NODE_TRIGGER, json!({ "on": "arrival" }), ""),
                script("a", "Shape", json!([{ "name": "text", "type": "text" }]), json!([{ "name": "draft", "type": "text" }])),
                script("b", "Finish", json!([{ "name": "draft", "type": "text" }]), json!([{ "name": "summary", "type": "text", "description": "One line" }, { "name": "records", "type": "list" }])),
            ],
            edges: vec![edge("t", "text", "a", "text"), edge("a", "draft", "b", "draft")],
        };
        let outputs = signature(&graph, &HashMap::new());
        assert_eq!(names(&outputs), vec![("summary".into(), "b".into(), "summary".into()), ("records".into(), "b".into(), "records".into())]);
        assert_eq!(outputs[0].description, "One line");
        assert_eq!(outputs[1].kind, PortType::List);
    }

    #[test]
    fn two_last_nodes_with_the_same_port_are_told_apart_by_their_label() {
        let graph = Graph {
            nodes: vec![
                node("t", NODE_TRIGGER, json!({ "on": "manual" }), ""),
                script("a", "Short form", json!([{ "name": "text", "type": "text" }]), json!([{ "name": "summary", "type": "text" }, { "name": "tone", "type": "text" }])),
                script("b", "Long form", json!([{ "name": "text", "type": "text" }]), json!([{ "name": "summary", "type": "text" }])),
                script("c", "Long form", json!([{ "name": "text", "type": "text" }]), json!([{ "name": "summary", "type": "text" }])),
            ],
            edges: vec![edge("t", "text", "a", "text"), edge("t", "text", "b", "text"), edge("t", "text", "c", "text")],
        };
        let outputs: Vec<String> = signature(&graph, &HashMap::new()).into_iter().map(|p| p.name).collect();
        assert_eq!(outputs, vec!["short_form_summary", "tone", "b_summary", "c_summary"]);
    }

    #[test]
    fn the_final_node_renames_and_describes_its_ports_while_starts_and_chats_give_nothing() {
        let graph = Graph {
            nodes: vec![
                node("t", NODE_TRIGGER, json!({ "on": "arrival" }), ""),
                node("sum", "builtin:summarize", json!({ "values": {}, "returns": { "summary": { "name": "gist", "description": "The short of it" } } }), ""),
                node("log", NODE_CHAT, json!({ "mode": "read", "conversation_id": null }), ""),
                node("ask", "builtin:ai", json!({ "values": {} }), ""),
            ],
            edges: vec![edge("t", "text", "sum", "text"), edge("t", "text", "ask", "prompt"), edge("ask", "answer", "log", "item")],
        };
        let outputs = signature(&graph, &HashMap::new());
        assert_eq!(names(&outputs), vec![("gist".into(), "sum".into(), "summary".into())]);
        assert_eq!(outputs[0].description, "The short of it");
        assert!(signature(&Graph { nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "manual" }), "")], edges: Vec::new() }, &HashMap::new()).is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_nested_action_node_offers_the_signature_its_final_node_gives() {
        let pool = test_pool().await;
        let (profile, _, _, _) = seed(&pool).await;
        let mut inner = Action::blank(&profile, "Shout");
        inner.inputs = vec![Port::new("text", PortType::Text)];
        inner.graph = Graph { nodes: vec![script("s", "Loud", json!([{ "name": "text", "type": "text" }]), json!([{ "name": "loud", "type": "text" }]))], edges: vec![] };
        let inner = Action::insert(&pool, &inner).await.unwrap();
        assert_eq!(names(&inner.outputs), vec![("loud".into(), "s".into(), "loud".into())]);
        let mut outer = Action::blank(&profile, "Outer");
        outer.graph = Graph { nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "manual" }), ""), node("n", &format!("action:{}", inner.id), json!({}), "")], edges: vec![edge("t", "text", "n", "text")] };
        let outer = Action::insert(&pool, &outer).await.unwrap();
        assert_eq!(names(&outer.outputs), vec![("loud".into(), "n".into(), "loud".into())]);
        let actions = others(&pool, &profile).await.unwrap();
        let (_, ports) = node_ports(&outer.graph.nodes[1], &actions, None).unwrap();
        assert_eq!(ports, vec![Port::new("loud", PortType::Text)]);
        let mut renamed = Action::get(&pool, &inner.id).await.unwrap();
        renamed.graph.nodes[0].config["outputs"] = json!([{ "name": "shout", "type": "text" }]);
        Action::save(&pool, &renamed).await.unwrap();
        let outer = Action::get(&pool, &outer.id).await.unwrap();
        assert_eq!(names(&outer.outputs), vec![("shout".into(), "n".into(), "shout".into())]);
        let catalog = catalog(&pool, &profile).await.unwrap();
        let listed = catalog.actions.iter().find(|a| a["id"] == json!(outer.id)).unwrap();
        assert_eq!(listed["outputs"][0]["name"], "shout");
    }

    #[tokio::test]
    async fn the_outbox_retires_by_rename_and_fresh_installs_never_create_it() {
        let pool = test_pool().await;
        let tables = |pool: Pool<Sqlite>| async move { sqlx::query_scalar::<_, String>("SELECT name FROM sqlite_master WHERE type = 'table'").fetch_all(&pool).await.unwrap() };
        assert!(!tables(pool.clone()).await.iter().any(|t| t.starts_with("action_outbox")));
        sqlx::query("CREATE TABLE action_outbox (id TEXT PRIMARY KEY, content TEXT NOT NULL)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO action_outbox (id, content) VALUES ('o1', 'Hei')").execute(&pool).await.unwrap();

        retire_outbox(&pool).await.unwrap();
        retire_outbox(&pool).await.unwrap();

        let after = tables(pool.clone()).await;
        assert!(!after.contains(&"action_outbox".to_string()) && after.contains(&RETIRED_OUTBOX.to_string()), "{:?}", after);
        let kept: String = sqlx::query_scalar(&format!("SELECT content FROM {} WHERE id = 'o1'", RETIRED_OUTBOX)).fetch_one(&pool).await.unwrap();
        assert_eq!(kept, "Hei");
    }

    #[tokio::test]
    async fn stored_outputs_retire_and_their_descriptions_move_to_the_final_node() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        crate::database::ensure_columns(&pool, "action", &[("outputs", "TEXT NOT NULL DEFAULT '[]'")]).await;
        let graph = Graph {
            nodes: vec![
                node("t", NODE_TRIGGER, json!({ "on": "manual" }), ""),
                script("s", "Find", json!([{ "name": "text", "type": "text" }]), json!([{ "name": "events", "type": "list" }])),
                node("sum", "builtin:summarize", json!({ "values": {} }), ""),
            ],
            edges: vec![edge("t", "text", "s", "text"), edge("t", "text", "sum", "text")],
        };
        let stored = json!([{ "name": "events", "type": "list", "description": "What went in the calendar" }, { "name": "summary", "type": "text", "description": "In a sentence" }, { "name": "gone", "type": "bool", "description": "Nothing gives this" }]);
        sqlx::query("INSERT INTO action (id, profile_id, name, inputs, outputs, graph, created_at, updated_at) VALUES ('a1', ?1, 'Old', '[]', ?2, ?3, '', '')").bind(&profile).bind(stored.to_string()).bind(serde_json::to_string(&graph).unwrap()).execute(&pool).await.unwrap();

        retire_outputs(&pool).await.unwrap();
        retire_outputs(&pool).await.unwrap();

        let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('action')").fetch_all(&pool).await.unwrap();
        assert!(!columns.contains(&"outputs".to_string()) && columns.contains(&RETIRED_OUTPUTS.to_string()), "{:?}", columns);
        let kept: String = sqlx::query_scalar(&format!("SELECT {} FROM action WHERE id = 'a1'", RETIRED_OUTPUTS)).fetch_one(&pool).await.unwrap();
        assert_eq!(serde_json::from_str::<Value>(&kept).unwrap(), stored);
        let action = Action::get(&pool, "a1").await.unwrap();
        assert_eq!(action.graph.nodes[1].config["outputs"][0]["description"], "What went in the calendar");
        assert_eq!(action.graph.nodes[2].config[RETURNS]["summary"]["description"], "In a sentence");
        assert_eq!(action.outputs.iter().map(|p| (p.name.as_str(), p.description.as_str())).collect::<Vec<_>>(), vec![("events", "What went in the calendar"), ("summary", "In a sentence")]);
    }

    #[test]
    fn ports_connect_by_type() {
        assert!(PortType::Text.accepts(PortType::Tag));
        assert!(PortType::Any.accepts(PortType::Message));
        assert!(PortType::Json.accepts(PortType::Person));
        assert!(!PortType::Conversation.accepts(PortType::Message));
        let draft = ActionDraft {
            name: "x".into(),
            graph: Graph {
                nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "arrival" }), ""), node("d", "builtin:done", json!({}), "")],
                edges: vec![edge("t", "message", "d", "conversation"), edge("t", "conversation", "d", "nope")],
            },
            ..Default::default()
        };
        let messages: Vec<String> = check(&draft, &HashMap::new(), None).into_iter().map(|d| d.message).collect();
        assert!(messages.iter().any(|m| m.contains("message gives message, but conversation needs conversation")), "{:?}", messages);
        assert!(messages.iter().any(|m| m.contains("has no input nope")), "{:?}", messages);
    }

    #[test]
    fn checks_catch_loops_bad_scripts_and_effectful_triggers() {
        let draft = ActionDraft {
            name: "x".into(),
            graph: Graph {
                nodes: vec![
                    node("t", NODE_TRIGGER, json!({ "on": "arrival" }), "done(conversation)"),
                    node("a", NODE_SCRIPT, json!({ "inputs": [{ "name": "v", "type": "any" }], "outputs": [{ "name": "v", "type": "any" }] }), "#{ v: v"),
                    node("b", NODE_SCRIPT, json!({ "inputs": [{ "name": "v", "type": "any" }], "outputs": [{ "name": "v", "type": "any" }] }), "#{ v: v }"),
                ],
                edges: vec![edge("a", "v", "b", "v"), edge("b", "v", "a", "v")],
            },
            ..Default::default()
        };
        let diagnostics = check(&draft, &HashMap::new(), None);
        assert!(diagnostics.iter().any(|d| d.message.contains("loop")));
        assert!(diagnostics.iter().any(|d| d.node.as_deref() == Some("a") && d.line.is_some()));
        assert!(diagnostics.iter().any(|d| d.node.as_deref() == Some("t") && d.message.contains("done()")));
    }

    fn edge_errors(draft: &ActionDraft, actions: &HashMap<String, Action>) -> HashMap<String, String> {
        check(draft, actions, None).into_iter().filter(|d| d.severity == "error").filter_map(|d| d.edge.clone().map(|e| (e, d.message))).collect()
    }

    #[test]
    fn wiring_refuses_self_loops_cycles_input_into_triggers_flow_out_of_landings_and_type_mismatches() {
        let any = || json!([{ "name": "v", "type": "any" }]);
        let draft = ActionDraft {
            name: "x".into(),
            graph: Graph {
                nodes: vec![
                    node("t", NODE_TRIGGER, json!({ "on": "arrival" }), ""),
                    node("s", "builtin:summarize", json!({}), ""),
                    node("l", "builtin:tag_add", json!({}), ""),
                    node("r", "builtin:reply", json!({}), ""),
                    node("q", NODE_SCRIPT, json!({ "inputs": [{ "name": "m", "type": "message" }], "outputs": [] }), "#{}"),
                    node("a", NODE_SCRIPT, json!({ "inputs": any(), "outputs": any() }), "#{ v: v }"),
                    node("b", NODE_SCRIPT, json!({ "inputs": any(), "outputs": any() }), "#{ v: v }"),
                    node("w", NODE_CHAT, json!({ "mode": "write", "conversation_id": null }), ""),
                ],
                edges: vec![
                    edge("t", "text", "s", "text"),
                    edge("s", "summary", "s", "text"),
                    edge("s", "summary", "t", "text"),
                    edge("l", "conversation", "s", "text"),
                    edge("t", "message", "l", "conversation"),
                    edge("t", "conversation", "r", "conversation"),
                    edge("r", "message", "q", "m"),
                    edge("a", "v", "b", "v"),
                    edge("b", "v", "a", "v"),
                    edge("t", "text", "w", "text"),
                    edge("w", "text", "b", "v"),
                ],
            },
            ..Default::default()
        };
        let errors = edge_errors(&draft, &HashMap::new());
        assert_eq!(errors["s.summary-s.text"], "A node cannot connect to itself");
        assert!(errors["s.summary-t.text"].contains("a trigger cannot receive input"), "{:?}", errors);
        assert!(errors["l.conversation-s.text"].contains("is a landing; nothing continues from it"), "{:?}", errors);
        assert!(errors["t.message-l.conversation"].contains("message gives message, but conversation needs conversation"), "{:?}", errors);
        assert!(errors["w.text-b.v"].contains("ends the flow"), "{:?}", errors);
        for fine in ["t.text-s.text", "t.conversation-r.conversation", "r.message-q.m", "t.text-w.text", "a.v-b.v", "b.v-a.v"] {
            assert!(!errors.contains_key(fine), "{} {:?}", fine, errors);
        }
        let loops: Vec<Diagnostic> = check(&draft, &HashMap::new(), None).into_iter().filter(|d| d.message.contains("loop")).collect();
        assert_eq!(loops.len(), 1, "{:?}", loops);
        assert!(loops[0].message.contains("a") && loops[0].message.contains("b") && !loops[0].message.contains(" s"), "{:?}", loops);
        assert!(builtin("tag_add").unwrap().outputs.is_empty() && builtin("draft").unwrap().outputs.is_empty() && builtin("reply").unwrap().outputs.len() == 1);
        assert_eq!(builtin("draft").unwrap().kind, KIND_LANDING);
    }

    fn signed(name: &str, inputs: Vec<Port>, graph: Graph) -> Action {
        let mut action = Action::blank("p", name);
        action.id = name.into();
        action.inputs = inputs;
        action.graph = graph;
        action
    }

    #[test]
    fn roles_and_usage_come_from_the_trigger_and_the_final_nodes() {
        let text = || json!([{ "name": "text", "type": "text" }]);
        let conversation = || vec![Port::new("conversation", PortType::Conversation)];
        let mut actions = vec![
            signed("own", Vec::new(), Graph { nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "arrival" }), ""), node("l", "builtin:tag_add", json!({}), "")], edges: vec![edge("t", "conversation", "l", "conversation")] }),
            signed("found", Vec::new(), Graph { nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "arrival" }), ""), node("s", "builtin:summarize", json!({}), "")], edges: vec![edge("t", "text", "s", "text")] }),
            signed("chat", conversation(), Graph { nodes: vec![node("l", "builtin:done", json!({}), ""), node("m", "builtin:mute", json!({}), "")], edges: Vec::new() }),
            signed("step", vec![Port::new("text", PortType::Text)], Graph { nodes: vec![node("s", NODE_SCRIPT, json!({ "inputs": text(), "outputs": text() }), "#{ text: text }")], edges: Vec::new() }),
            signed("records", Vec::new(), Graph { nodes: vec![node("s", NODE_SCRIPT, json!({ "inputs": [], "outputs": [{ "name": "items", "type": "list" }] }), "#{ items: [] }")], edges: Vec::new() }),
            signed("called", vec![Port::new("text", PortType::Text)], Graph { nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "called" }), ""), node("s", "builtin:summarize", json!({}), "")], edges: vec![edge("t", "text", "s", "text")] }),
            signed("by_hand", Vec::new(), Graph { nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "manual" }), ""), node("s", "builtin:summarize", json!({}), "")], edges: vec![edge("t", "text", "s", "text")] }),
            signed("written", Vec::new(), Graph { nodes: vec![node("w", NODE_CHAT, json!({ "mode": "write", "conversation_id": null }), ""), node("s", "builtin:summarize", json!({}), ""), node("o", NODE_CHAT, json!({ "mode": "write", "conversation_id": null }), "")], edges: vec![edge("w", "text", "s", "text"), edge("s", "summary", "o", "text")] }),
            signed("nested", conversation(), Graph { nodes: vec![node("n", "action:chat", json!({}), "")], edges: Vec::new() }),
        ];
        let mut shipped = signed("shipped", Vec::new(), Graph { nodes: vec![node("l", "builtin:tag_add", json!({}), "")], edges: Vec::new() });
        shipped.builtin = true;
        shipped.key = Some("builtin:tag_add".into());
        actions.push(shipped);
        sign(&mut actions);
        let of = |id: &str| actions.iter().find(|a| a.id == id).unwrap();
        let usage = |standalone, chat, step, landing| Usage { standalone, chat, step, landing };
        assert_eq!(of("own").usage, usage(true, false, false, false));
        assert_eq!(of("found").usage, usage(true, false, false, false));
        assert_eq!(of("chat").usage, usage(false, true, false, true));
        assert_eq!(of("step").usage, usage(false, true, true, false));
        assert_eq!(of("records").usage, usage(false, true, true, false));
        assert_eq!(of("called").usage, usage(false, false, true, false));
        assert_eq!(of("by_hand").usage, usage(true, false, true, false));
        assert_eq!(of("written").usage, usage(false, true, false, false));
        assert_eq!(of("nested").usage, usage(false, true, false, true));
        assert_eq!(of("shipped").usage, Usage::default());
        assert_eq!(["own", "found", "chat", "step", "called", "shipped"].map(|id| action_role(of(id))), [KIND_TRIGGER, KIND_TRIGGER, KIND_LANDING, KIND_STEP, KIND_STEP, KIND_LANDING]);
        let known: HashMap<String, Action> = actions.iter().map(|a| (a.id.clone(), a.clone())).collect();
        let into = |id: &str| node("n", &format!("{}{}", ACTION_PREFIX, id), json!({}), "");
        assert!(node_ports(&into("found"), &known, None).unwrap().0.is_empty());
        assert_eq!(node_ports(&into("called"), &known, None).unwrap().0, vec![Port::new("text", PortType::Text)]);
        let draft = ActionDraft {
            name: "outer".into(),
            graph: Graph {
                nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "arrival" }), ""), into("found"), node("c", "action:chat", json!({}), ""), node("s", "builtin:summarize", json!({}), "")],
                edges: vec![edge("t", "text", "n", "text"), edge("t", "conversation", "c", "conversation"), edge("n", "summary", "s", "text")],
            },
            ..Default::default()
        };
        let errors = edge_errors(&draft, &known);
        assert!(errors["t.text-n.text"].contains("a trigger cannot receive input"), "{:?}", errors);
        assert!(!errors.contains_key("t.conversation-c.conversation") && !errors.contains_key("n.summary-s.text"), "{:?}", errors);
    }

    #[tokio::test]
    async fn saving_refuses_new_bad_connections_but_keeps_old_ones_editable() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let mut stale = Action::blank(&profile, "Stale");
        stale.graph = Graph { nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "arrival" }), ""), node("q", "builtin:draft", json!({ "values": {} }), "")], edges: vec![edge("q", "draft", "q", "text")] };
        let stale = Action::insert(&pool, &stale).await.unwrap();
        let renamed = put_action(&pool, PutActionInput { action_id: stale.id.clone(), name: Some("Still stale".into()), ..Default::default() }).await.unwrap();
        assert!(!renamed.enabled && renamed.name == "Still stale");
        let mut worse = renamed.graph.clone();
        worse.edges.push(edge("q", "conversation", "t", "text"));
        let refused = put_action(&pool, PutActionInput { action_id: stale.id.clone(), graph: Some(worse), ..Default::default() }).await.unwrap_err();
        assert!(refused.contains("a trigger cannot receive input"), "{}", refused);
        let fixed = put_action(&pool, PutActionInput { action_id: stale.id.clone(), graph: Some(Graph { edges: Vec::new(), ..renamed.graph.clone() }), ..Default::default() }).await.unwrap();
        assert!(fixed.graph.edges.is_empty());
        let posted = post_action(&pool, &profile, PostActionInput { name: "Loop".into(), graph: Some(Graph { nodes: vec![node("s", "builtin:summarize", json!({}), "")], edges: vec![edge("s", "summary", "s", "text")] }), ..Default::default() }).await.unwrap_err();
        assert!(posted.contains("A node cannot connect to itself"), "{}", posted);
    }

    #[tokio::test]
    async fn seeded_actions_lose_the_retired_approval_wording_unless_edited() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        crate::providers::mock::seed::ledger(&pool).await.unwrap();
        let graph = Graph { nodes: vec![Node { label: "Queue for approval".into(), ..node("queue", "builtin:draft", json!({ "values": {} }), "") }], edges: Vec::new() };
        let mut ids = Vec::new();
        for (name, edited) in [("Polite", false), ("Mine", true)] {
            let mut action = Action::blank(&profile, name);
            action.description = "Drafts a short, friendly reply for approval.".into();
            action.graph = graph.clone();
            if edited {
                action.updated_at = "2026-10-15T00:00:00Z".into();
            }
            let action = Action::insert(&pool, &action).await.unwrap();
            sqlx::query("UPDATE action SET updated_at = CASE WHEN ?2 THEN '2026-10-15T00:00:00Z' ELSE created_at END WHERE id = ?1").bind(&action.id).bind(edited).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO mock_seed (profile_id, key, kind, item_id, created_at) VALUES (?1, ?2, 'action', ?3, '')").bind(&profile).bind(format!("action:{}", name)).bind(&action.id).execute(&pool).await.unwrap();
            ids.push(action.id);
        }
        retire_outbox(&pool).await.unwrap();
        let polite = Action::get(&pool, &ids[0]).await.unwrap();
        assert_eq!((polite.description.as_str(), polite.graph.nodes[0].label.as_str()), ("Drafts a short, friendly reply.", "Keep as draft"));
        let mine = Action::get(&pool, &ids[1]).await.unwrap();
        assert_eq!(mine.graph.nodes[0].label, "Queue for approval");
    }

    #[test]
    fn execution_follows_the_graph_order() {
        let graph = Graph {
            nodes: vec![node("c", NODE_SCRIPT, json!({}), ""), node("a", NODE_TRIGGER, json!({}), ""), node("b", NODE_SCRIPT, json!({}), "")],
            edges: vec![edge("a", "x", "b", "x"), edge("b", "x", "c", "x")],
        };
        assert_eq!(topo_order(&graph).unwrap(), vec!["a", "b", "c"]);
    }

    #[test]
    fn every_built_in_is_fully_defined_and_builtins_keep_their_body() {
        let picker = picker_defaults();
        let groups: Vec<&str> = builtin_groups().iter().map(|g| g.key).collect();
        let mut keys = HashSet::new();
        for item in &picker {
            assert!(keys.insert(item.key.clone()), "{} twice", item.key);
            assert!(Action::is_builtin_key(Some(&item.key)), "{}", item.key);
            assert!(!item.label.trim().is_empty() && item.icon.starts_with("lucide:") && !item.description.trim().is_empty(), "{}", item.key);
            assert!(groups.contains(&item.group) && item.group.starts_with(&format!("builtin.{}s", item.kind)), "{} sits in {}", item.key, item.group);
            assert_eq!(builtin_kind(&item.key), Some(item.kind), "{}", item.key);
            let (inputs, outputs) = node_ports(&item.node, &HashMap::new(), None).unwrap();
            assert_eq!((&inputs, &outputs), (&item.inputs, &item.outputs), "{}", item.key);
            assert!(!item.outputs.is_empty() || item.kind == KIND_LANDING, "{} has no output", item.key);
            if item.kind == KIND_TRIGGER && item.node.kind == NODE_TRIGGER {
                assert!(item.inputs.is_empty(), "{}", item.key);
            } else if item.node.kind != NODE_CHAT {
                assert!(!item.inputs.is_empty(), "{} has no input", item.key);
            }
            assert_eq!(item.requires, node_requirements(&item.node, &HashMap::new()), "{}", item.key);
        }
        for b in builtins() {
            assert!(keys.contains(&format!("{}{}", BUILTIN_PREFIX, b.key)), "{} is not offered", b.key);
        }
        let need = |key: &str| picker.iter().find(|i| i.key == key).unwrap().requires.clone();
        assert_eq!(need("builtin:ai"), vec![action_script::REQUIRES_AI_ENABLED, action_script::REQUIRES_AI_CONNECTION, action_script::REQUIRES_AI_MODEL]);
        assert_eq!(need("builtin:reply"), vec![action_script::REQUIRES_ACCOUNT]);
        assert_eq!(need("trigger:arrival"), vec![action_script::REQUIRES_ACTIONS_ENABLED]);
        assert!(need("trigger:manual").is_empty());
        assert!(picker.iter().find(|i| i.key == "trigger:reaction").unwrap().outputs.iter().any(|p| p.name == "reaction"));
        assert!(picker.iter().find(|i| i.key == "trigger:event_soon").unwrap().outputs.iter().any(|p| p.name == "event" && p.kind == PortType::Json));
        for (key, kind) in [("trigger:arrival", KIND_TRIGGER), ("trigger:sent", KIND_TRIGGER), ("trigger:edited", KIND_TRIGGER), ("trigger:deleted", KIND_TRIGGER), ("trigger:reaction", KIND_TRIGGER), ("trigger:mention", KIND_TRIGGER), ("trigger:schedule", KIND_TRIGGER), ("trigger:event_soon", KIND_TRIGGER), ("trigger:chat_read", KIND_TRIGGER), ("trigger:manual", KIND_TRIGGER), ("trigger:called", KIND_TRIGGER), ("builtin:find_events", KIND_STEP), ("builtin:sender_kind", KIND_STEP), ("builtin:content", KIND_STEP), ("builtin:regex_find", KIND_STEP), ("builtin:create_event", KIND_LANDING), ("builtin:style_conversation", KIND_LANDING), ("builtin:chat_write", KIND_LANDING), ("builtin:mark_person", KIND_LANDING)] {
            assert_eq!(picker.iter().find(|i| i.key == key).map(|i| i.kind), Some(kind), "{}", key);
        }
        assert_eq!(builtin_kind(RECEIPTS_KEY), Some(KIND_ACTION));
        assert_eq!(builtin_kind(CLASSIFY_SENDERS_KEY), Some(KIND_ACTION));
        for shipped in builtin_actions() {
            assert!(!shipped.draft.name.is_empty() && !shipped.draft.description.is_empty() && shipped.draft.icon.is_some() && !shipped.draft.graph.nodes.is_empty(), "{}", shipped.key);
        }
        let silent: Vec<&str> = builtin_actions().into_iter().filter(|shipped| signature(&shipped.draft.graph, &HashMap::new()).is_empty()).map(|shipped| shipped.key).collect();
        assert!(silent.is_empty(), "no output from {:?}", silent);
        let unflagged: Vec<&str> = builtin_actions().into_iter().map(|shipped| shipped.key).filter(|key| !crate::models::action::BUILTIN_ACTION_KEYS.contains(key)).collect();
        assert!(unflagged.is_empty(), "shipped but editable: {:?}", unflagged);
        let mut graph = Graph { nodes: vec![node("x", "builtin:done", json!({}), "send(target, \"pwned\")")], edges: vec![] };
        normalize(&mut graph);
        assert_eq!(graph.nodes[0].body, builtin("done").unwrap().body);
    }

    #[test]
    fn schedules_fire_on_interval_and_time_of_day() {
        let now = Local::now();
        let every = TriggerSchedule { every_minutes: Some(30), at: None, days: vec![] };
        assert!(schedule_due(&every, None, now));
        assert!(!schedule_due(&every, Some(now.with_timezone(&Utc) - Duration::minutes(10)), now));
        assert!(schedule_due(&every, Some(now.with_timezone(&Utc) - Duration::minutes(31)), now));
        let morning = Local::now().date_naive().and_hms_opt(9, 30, 0).unwrap().and_local_timezone(Local).single().unwrap();
        let daily = TriggerSchedule { every_minutes: None, at: Some("08:00".into()), days: vec![] };
        assert!(schedule_due(&daily, None, morning));
        assert!(!schedule_due(&daily, Some((morning - Duration::minutes(20)).with_timezone(&Utc)), morning));
        assert!(schedule_due(&daily, Some((morning - Duration::days(1)).with_timezone(&Utc)), morning));
        let other_day = TriggerSchedule { days: vec![(morning.weekday().number_from_monday() % 7 + 1) as u8], ..daily };
        assert!(!schedule_due(&other_day, None, morning));
    }

    #[test]
    fn rules_become_actions() {
        let label = Tag { id: "l1".into(), profile_id: "p".into(), name: "Team".into(), color: String::new(), kind: KIND_SPLIT.into(), position: 0, description: String::new(), icon: None, favourite: false, favourite_position: None, created_at: String::new(), updated_at: String::new() };
        let rule = LegacyRule {
            id: "r".into(),
            profile_id: "p".into(),
            name: "Kunde".into(),
            enabled: true,
            position: 0,
            r#match: Match::All,
            conditions: vec![Condition { field: Field::SenderDomain, operator: crate::models::rule::Operator::Is, value: "client.example".into() }],
            steps: LegacyRuleSteps { tag_ids: vec!["l1".into()], archive: true, mute: true, stop: true, ..Default::default() },
            explanation: "Mail from Kunde".into(),
        };
        let draft = draft_from_rule(&rule, &[label], &[]);
        assert_eq!(draft.graph.nodes[0].body, "test(message.sender_domain, \"is\", \"client.example\")");
        assert_eq!(draft.graph.nodes.iter().map(|n| n.kind.as_str()).collect::<Vec<_>>(), vec!["trigger", "builtin:move", "builtin:mute", "builtin:done"]);
        assert!(draft.description.contains("stopped later rules"));
        assert!(check(&draft, &HashMap::new(), None).is_empty(), "{:?}", check(&draft, &HashMap::new(), None));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn old_rules_migrate_once_and_sort_like_before() {
        let pool = test_pool().await;
        let (profile, _, conversation, message) = seed(&pool).await;
        let newsletters = Tag::all(&pool, &profile).await.unwrap().into_iter().find(|l| l.name == "Newsletters").unwrap();
        sqlx::query(
            "CREATE TABLE rule (id TEXT PRIMARY KEY, profile_id TEXT NOT NULL, name TEXT NOT NULL, enabled BOOLEAN NOT NULL DEFAULT 1, position INTEGER NOT NULL DEFAULT 0,
             match_mode TEXT NOT NULL DEFAULT 'all', conditions TEXT NOT NULL DEFAULT '[]', actions TEXT NOT NULL DEFAULT '{}', origin TEXT NOT NULL DEFAULT 'manual',
             example_conversation_id TEXT, explanation TEXT NOT NULL DEFAULT '', hits INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO rule (id, profile_id, name, conditions, actions, created_at, updated_at) VALUES ('r1', ?1, 'Kunde mail', ?2, ?3, '', '')")
            .bind(&profile)
            .bind(json!([{ "field": "sender_domain", "operator": "is", "value": "client.example" }]).to_string())
            .bind(json!({ "tag_ids": [newsletters.id], "mark_read": true }).to_string())
            .execute(&pool)
            .await
            .unwrap();
        ensure_defaults(&pool, &profile).await.unwrap();
        ensure_defaults(&pool, &profile).await.unwrap();
        let actions = Action::all(&pool, &profile).await.unwrap();
        let migrated = actions.iter().find(|a| a.key.as_deref() == Some("rule:r1")).unwrap();
        assert_eq!(migrated.name, "Kunde mail");
        assert!(migrated.enabled);
        for key in ["sort_splits", "summarise_mail", "classify_senders", "recognise_invites", DETECT_NEWSLETTERS_KEY] {
            assert_eq!(actions.iter().filter(|a| a.key.as_deref() == Some(key)).count(), 1, "{}", key);
        }
        assert!(actions.iter().find(|a| a.key.as_deref() == Some("classify_senders")).unwrap().enabled);
        assert!(!crate::database::table_exists(&pool, "rule").await.unwrap());
        assert!(crate::database::table_exists(&pool, RETIRED_RULES).await.unwrap());
        let groups: Vec<String> = ActionGroup::all(&pool, &profile).await.unwrap().into_iter().map(|g| g.name).collect();
        assert!(groups.contains(&"Sorting rules".to_string()) && groups.contains(&"Built in".to_string()));
        let mut r = runner(&pool, &profile, false);
        r.ai = Some(Arc::new(|_: &str, _: &Value| Err("no AI in this test".to_string())));
        let only = dispatch_to(&r, &[TriggerOn::Arrival], &message, Some(&migrated.id)).await.unwrap();
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].status, "done", "{:?}", only[0]);
        let split: String = sqlx::query_scalar("SELECT l.name FROM object_tag cl JOIN tag l ON l.id = cl.tag_id WHERE cl.object_type = 'conversation' AND cl.object_id = ?1").bind(&conversation).fetch_one(&pool).await.unwrap();
        assert_eq!(split, "Newsletters");
        let unread: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE conversation_id = ?1 AND is_read = 0").bind(&conversation).fetch_one(&pool).await.unwrap();
        assert_eq!(unread, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_built_in_actions_sort_and_summarise() {
        let pool = test_pool().await;
        let (profile, _, conversation, message) = seed(&pool).await;
        sqlx::query("UPDATE message SET type = 'email', content = 'Kan vi møtes torsdag kl 13:00?' WHERE id = ?1").bind(&message).execute(&pool).await.unwrap();
        ensure_defaults(&pool, &profile).await.unwrap();
        let mut r = runner(&pool, &profile, false);
        r.ai = Some(Arc::new(|key: &str, extra: &Value| match key {
            "sort" => {
                assert!(extra["subject"]["sender_email"].as_str().unwrap_or("").contains("client.example"));
                Ok(json!({ "label": "Team", "confidence": 0.9, "reason": "colleague" }))
            }
            "summarise_mail" => Ok(json!({ "line": "Foreslår møte torsdag 13:00" })),
            "parse_events" => {
                assert_eq!(extra["message"]["from_email"], "anna@client.example");
                Ok(json!({ "events": [
                    { "title": "Møte med Anna", "starts_at": "2026-10-15T13:00:00+02:00", "confidence": 0.9 },
                    { "title": "Kanskje lunsj", "starts_at": "2026-10-16T11:30:00+02:00", "confidence": 0.5 },
                    { "title": "Gjetning", "starts_at": "2026-10-17T09:00:00+02:00", "confidence": 0.1 }
                ] }))
            }
            other => Err(format!("unexpected prompt {}", other)),
        }));
        let runs = dispatch(&r, &[TriggerOn::Arrival], &message).await.unwrap();
        let names: Vec<String> = runs.iter().map(|r| r.action_name.clone()).collect();
        assert!(names.contains(&"Sort into splits".to_string()), "{:?}", names);
        assert!(runs.iter().all(|r| r.status == "done"), "{:?}", runs);
        let split: String = sqlx::query_scalar("SELECT l.name FROM object_tag cl JOIN tag l ON l.id = cl.tag_id WHERE cl.object_type = 'conversation' AND cl.object_id = ?1 AND l.kind = 'split'").bind(&conversation).fetch_one(&pool).await.unwrap();
        assert_eq!(split, "Team");
        let summary = Action::find(&pool, &profile, "summarise_mail").await.unwrap();
        let payload = payload_for_message(&pool, &profile, &message).await.unwrap();
        let run = execute(&r, &summary, Start::Invoke { inputs: Map::new(), payload }, 0, vec![summary.id.clone()]).await.unwrap();
        assert_eq!(run.outputs, json!({ "summary": "Foreslår møte torsdag 13:00" }), "{:?}", run);
        let stored: Option<String> = sqlx::query_scalar("SELECT summary FROM message WHERE id = ?1").bind(&message).fetch_one(&pool).await.unwrap();
        assert_eq!(stored.as_deref(), Some("Foreslår møte torsdag 13:00"));
        r.ai = Some(Arc::new(|_: &str, _: &Value| Err("the summary is cached".to_string())));
        let cached = execute(&r, &summary, Start::Invoke { inputs: Map::new(), payload: payload_for_message(&pool, &profile, &message).await.unwrap() }, 0, vec![summary.id.clone()]).await.unwrap();
        assert_eq!(cached.status, "done", "{:?}", cached);
        sqlx::query("UPDATE conversation SET last_id = ?1 WHERE id = ?2").bind(&message).bind(&conversation).execute(&pool).await.unwrap();
        let report = backfill(&r, Some("sort_splits"), 10).await.unwrap();
        assert_eq!(report.messages, 1);
        assert_eq!(report.runs, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_mock_dry_run_fires_the_trigger_with_the_seeded_sample() {
        let pool = test_pool().await;
        assert_eq!(sample(&pool, "nobody", None, None).await.unwrap(), Value::Null);
        let (profile, _, conversation, message) = seed(&pool).await;
        sqlx::query("INSERT INTO tag (id, profile_id, name, kind, created_at, updated_at) VALUES ('t-seed', ?1, 'Customer', 'tag', 'now', 'now')").bind(&profile).execute(&pool).await.unwrap();
        let found = sample(&pool, &profile, None, None).await.unwrap();
        assert_eq!(found["message"]["id"], json!(message));
        assert_eq!(found["message"]["subject"], json!("Invoice"));
        assert_eq!(found["conversation"]["id"], json!(conversation));
        assert_eq!(found["tag"], json!("Customer"), "{:?}", found);
        assert_eq!(sample(&pool, &profile, Some(&message), None).await.unwrap()["message"]["id"], json!(message));
        let action = Action::insert(&pool, &reply_action(&profile)).await.unwrap();
        let start = dry_start(&action, found.as_object().cloned().unwrap(), Value::Null);
        assert!(matches!(&start, Start::Trigger { node, .. } if node == "t"));
        let r = runner(&pool, &profile, true);
        let run = execute(&r, &action, start, 0, vec![action.id.clone()]).await.unwrap();
        assert_eq!(run.status, "done", "{:?}", run);
        assert!(run.log.iter().any(|l| l.status == "would" && l.detail.contains("tag_add")), "{:?}", run.log);
        let tagged = run.log.iter().find(|l| l.kind == LOG_STEP && l.node == "tag").map(|l| serde_json::from_str::<Value>(&l.detail).unwrap()).unwrap();
        assert_eq!(tagged["inputs"]["conversation"]["id"], json!(conversation));
        assert!(tagged["ms"].is_u64());
        let labels: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM object_tag WHERE object_type = 'conversation' AND object_id = ?1").bind(&conversation).fetch_one(&pool).await.unwrap();
        assert_eq!(labels, 0);
        let mut manual = reply_action(&profile);
        manual.graph.nodes[0].config = json!({ "on": "manual" });
        assert!(matches!(dry_start(&manual, Map::new(), Value::Null), Start::Invoke { .. }));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_dry_run_changes_nothing_and_logs_what_it_would_do() {
        let pool = test_pool().await;
        let (profile, _, conversation, message) = seed(&pool).await;
        let action = Action::insert(&pool, &reply_action(&profile)).await.unwrap();
        let r = runner(&pool, &profile, true);
        let runs = dispatch(&r, &[TriggerOn::Arrival], &message).await.unwrap();
        assert_eq!(runs.len(), 1, "{:?}", runs);
        let run = &runs[0];
        assert_eq!(run.status, "done", "{:?}", run);
        assert!(run.log.iter().any(|l| l.status == "would" && l.detail.contains("send in") && l.detail.contains("Thanks, we will pay today")), "{:?}", run.log);
        assert!(run.log.iter().any(|l| l.status == "would" && l.detail.contains("tag_add")));
        let mut gave: Vec<&String> = run.outputs.as_object().map(|o| o.keys().collect()).unwrap_or_default();
        gave.sort();
        assert_eq!(gave, vec!["message"], "{:?}", run.outputs);
        let labels: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM object_tag WHERE object_type = 'conversation' AND object_id = ?1").bind(&conversation).fetch_one(&pool).await.unwrap();
        assert_eq!(labels, 0);
        assert_eq!(action.run_count, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn triggers_respect_scope_and_predicate() {
        let pool = test_pool().await;
        let (profile, _, _, message) = seed(&pool).await;
        let mut other = reply_action(&profile);
        other.graph.nodes[0].config = json!({ "on": "arrival", "scope": { "providers": ["slack"] } });
        Action::insert(&pool, &other).await.unwrap();
        let mut silent = reply_action(&profile);
        silent.graph.nodes[0].body = "regex_match(text, \"no such words\")".into();
        Action::insert(&pool, &silent).await.unwrap();
        let r = runner(&pool, &profile, true);
        assert!(dispatch(&r, &[TriggerOn::Arrival], &message).await.unwrap().is_empty());
        assert!(dispatch(&r, &[TriggerOn::Sent], &message).await.unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_switched_off_trigger_is_not_dispatched() {
        let pool = test_pool().await;
        let (profile, _, _, message) = seed(&pool).await;
        let mut action = reply_action(&profile);
        action.graph.nodes[0].config["disabled"] = json!(true);
        let action = Action::insert(&pool, &action).await.unwrap();
        let r = runner(&pool, &profile, true);
        assert!(dispatch_to(&r, &[TriggerOn::Arrival], &message, Some(&action.id)).await.unwrap().is_empty());
        let mut on = Action::find(&pool, &profile, &action.id).await.unwrap();
        on.graph.nodes[0].config["disabled"] = json!(false);
        Action::save(&pool, &on).await.unwrap();
        assert_eq!(dispatch_to(&r, &[TriggerOn::Arrival], &message, Some(&action.id)).await.unwrap().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn starters_stay_mine_and_built_ins_sit_in_their_own_nested_groups() {
        let pool = test_pool().await;
        let (profile, provider, _, message) = seed(&pool).await;
        let old = ActionGroup::post(&pool, &profile, &PostActionGroupInput { name: "Built in".into(), description: None, icon: None, parent_id: None }).await.unwrap();
        ensure_defaults(&pool, &profile).await.unwrap();
        ensure_defaults(&pool, &profile).await.unwrap();
        let groups = ActionGroup::all(&pool, &profile).await.unwrap();
        assert!(groups.iter().any(|g| g.id == old.id && !g.builtin));
        let keyed = |key: &str| groups.iter().find(|g| g.key.as_deref() == Some(key)).unwrap();
        let root = keyed(BUILTIN_GROUP);
        assert_eq!(root.name, "Built in");
        assert!(root.builtin && root.parent_id.is_none());
        for group in builtin_groups() {
            let found = keyed(group.key);
            assert!(found.builtin, "{}", group.key);
            assert_eq!(found.parent_id.as_deref(), group.parent.map(|p| keyed(p).id.as_str()), "{}", group.key);
        }
        assert_eq!(groups.iter().filter(|g| g.builtin).count(), builtin_groups().len());
        assert!(ActionGroup::delete(&pool, &root.id).await.is_err());
        assert!(ActionGroup::put(&pool, &crate::models::action::PutActionGroupInput { group_id: keyed("builtin.steps").id.clone(), name: Some("Mine".into()), parent_id: None, description: None, icon: None, position: None }).await.is_err());
        let actions = Action::all(&pool, &profile).await.unwrap();
        for (key, group) in [("sort_splits", "builtin.steps.ai"), ("summarise_mail", "builtin.steps.ai"), ("draft_reply", "builtin.steps.ai"), ("recognise_invites", "builtin.actions"), (DETECT_NEWSLETTERS_KEY, "builtin.actions")] {
            let action = actions.iter().find(|a| a.key.as_deref() == Some(key)).unwrap();
            assert_eq!(action.group_id.as_deref(), Some(keyed(group).id.as_str()), "{}", key);
            assert!(action.builtin, "{}", key);
        }
        let receipts = actions.iter().find(|a| a.key.as_deref() == Some(RECEIPTS_KEY)).unwrap();
        assert!(receipts.builtin && !receipts.usage.chat && !receipts.enabled && receipts.group_id.as_deref() == Some(keyed("builtin.actions").id.as_str()));
        assert!(put_action(&pool, PutActionInput { action_id: receipts.id.clone(), name: Some("Mine".into()), ..Default::default() }).await.is_err());
        assert!(Action::delete(&pool, &receipts.id).await.is_err());
        let picker = picker_defaults();
        for item in &picker {
            let action = actions.iter().find(|a| a.key.as_deref() == Some(item.key.as_str())).unwrap_or_else(|| panic!("{} is not stored", item.key));
            assert!(action.builtin && action.builtin_kind.as_deref() == Some(item.kind), "{}", item.key);
            assert_eq!(action.group_id.as_deref(), Some(keyed(item.group).id.as_str()), "{}", item.key);
            assert_eq!(action.graph.nodes.len(), 1, "{}", item.key);
            assert_eq!(action.graph.nodes[0].kind, item.node.kind, "{}", item.key);
        }
        assert!(!actions.iter().any(|a| a.key.as_deref().is_some_and(|k| k.starts_with(&format!("{}arrival:", BUILTIN_KEY_PREFIX)))));
        let arrived = actions.iter().find(|a| a.key.as_deref() == Some("trigger:arrival")).unwrap();
        assert!(put_action(&pool, PutActionInput { action_id: arrived.id.clone(), name: Some("Mine".into()), ..Default::default() }).await.is_err());
        assert!(put_action(&pool, PutActionInput { action_id: arrived.id.clone(), enabled: Some(false), ..Default::default() }).await.is_err());
        assert!(Action::delete(&pool, &arrived.id).await.is_err());
        assert!(post_action(&pool, &profile, PostActionInput { name: "Sneaky".into(), group_id: Some(root.id.clone()), ..Default::default() }).await.is_err());
        let r = runner(&pool, &profile, true);
        assert!(dispatch_to(&r, &[TriggerOn::Arrival], &message, Some(&arrived.id)).await.unwrap().is_empty());
        let names: Vec<String> = sqlx::query_scalar("SELECT name FROM provider").fetch_all(&pool).await.unwrap();
        let kinds: Vec<String> = sqlx::query_scalar("SELECT provider_type FROM provider").fetch_all(&pool).await.unwrap();
        let banned: Vec<String> = ["gmail", "outlook", "slack", "teams", "northwind", "google", "microsoft", "imap", "mock"].iter().map(|s| s.to_string()).chain(names).chain(kinds).chain([provider]).map(|s| s.to_lowercase()).collect();
        for action in actions.iter().filter(|a| a.builtin) {
            let name = action.name.to_lowercase();
            assert!(!banned.iter().any(|b| name.contains(b.as_str())), "{} names a connection", action.name);
        }
        for group in groups.iter().filter(|g| g.builtin) {
            assert!(!banned.iter().any(|b| group.name.to_lowercase().contains(b.as_str())), "{} names a connection", group.name);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn case_nodes_become_tag_nodes_once() {
        let pool = test_pool().await;
        let (profile, _, _, _) = seed(&pool).await;
        let mut filed = Action::blank(&profile, "File it");
        filed.graph = Graph {
            nodes: vec![
                node("t", NODE_TRIGGER, json!({ "on": "arrival" }), ""),
                Node { id: "case".into(), kind: "builtin:add_to_case".into(), label: "Add to case".into(), config: json!({ "values": { "case_file": "Kunde" } }), body: String::new(), x: 0.0, y: 0.0 },
                node("s", NODE_SCRIPT, json!({ "inputs": [{ "name": "conversation", "type": "conversation" }], "outputs": [] }), "add_to_case(\"Avtale, 2027\", conversation); #{}"),
            ],
            edges: vec![edge("t", "conversation", "case", "conversation"), edge("t", "text", "case", "case_file"), edge("t", "conversation", "s", "conversation")],
        };
        let filed = Action::insert(&pool, &filed).await.unwrap();
        assert_eq!(migrate_case_nodes(&pool, &profile).await.unwrap(), 1);
        let moved = Action::get(&pool, &filed.id).await.unwrap();
        let tagged = moved.graph.nodes.iter().find(|n| n.id == "case").unwrap();
        assert_eq!((tagged.kind.as_str(), tagged.label.as_str()), ("builtin:tag_add", "Add tag"));
        assert_eq!(tagged.config["values"], json!({ "tag": "Kunde" }));
        assert!(moved.graph.edges.iter().any(|e| e.target == "case" && e.target_port == "tag"));
        assert!(!moved.graph.edges.iter().any(|e| e.target_port == "case_file"));
        assert_eq!(moved.graph.nodes.iter().find(|n| n.id == "s").unwrap().body, "tag_add(conversation, \"Avtale, 2027\"); #{}");
        assert_eq!(migrate_case_nodes(&pool, &profile).await.unwrap(), 0);
        assert!(!picker_defaults().iter().any(|i| i.key == "builtin:add_to_case"));
        assert!(!action_script::library().iter().any(|f| f.name == "add_to_case"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn per_connection_triggers_become_the_generic_trigger_with_a_connection_filter() {
        let pool = test_pool().await;
        let (profile, provider, _, message) = seed(&pool).await;
        let slack = test_provider(&pool, &profile, "slack").await;
        let root = ActionGroup::post(&pool, &profile, &PostActionGroupInput { name: "Built in".into(), description: None, icon: None, parent_id: None }).await.unwrap();
        sqlx::query("UPDATE action_group SET key = ?1 WHERE id = ?2").bind(BUILTIN_GROUP).bind(&root.id).execute(&pool).await.unwrap();
        let mut source = Action::blank(&profile, "Message arrived from Mock Gmail");
        source.key = Some(format!("{}arrival:{}", BUILTIN_KEY_PREFIX, provider));
        source.group_id = Some(root.id.clone());
        source.graph = Graph { nodes: vec![node("trigger", NODE_TRIGGER, json!({ "on": "arrival", "scope": { "providers": [provider] } }), "")], edges: vec![] };
        let source = Action::insert(&pool, &source).await.unwrap();
        let mut dragged = reply_action(&profile);
        dragged.name = "From the dragged source".into();
        dragged.graph.nodes[0].config = json!({ "on": "arrival", "scope": { "providers": [provider, "email"] } });
        let dragged = Action::insert(&pool, &dragged).await.unwrap();
        let mut nested = reply_action(&profile);
        nested.name = "Nested source".into();
        nested.graph.nodes[0] = node("t", &format!("{}{}", ACTION_PREFIX, source.id), json!({}), "");
        let nested = Action::insert(&pool, &nested).await.unwrap();
        let mut filtered = reply_action(&profile);
        filtered.name = "Old filter".into();
        filtered.graph.nodes[0].config = json!({ "on": "filter", "group_id": "g1" });
        let filtered = Action::insert(&pool, &filtered).await.unwrap();
        ensure_defaults(&pool, &profile).await.unwrap();
        assert!(Action::get(&pool, &source.id).await.is_err());
        let moved = Action::get(&pool, &dragged.id).await.unwrap();
        let trigger = moved.graph.nodes[0].trigger().unwrap();
        assert_eq!(trigger.on, TriggerOn::Arrival);
        assert_eq!(trigger.scope.providers, vec!["email".to_string()]);
        assert_eq!(trigger.scope.connection_ids, vec![provider.clone()]);
        let unnested = Action::get(&pool, &nested.id).await.unwrap();
        assert_eq!(unnested.graph.nodes[0].kind, NODE_TRIGGER);
        assert_eq!(unnested.graph.nodes[0].id, "t");
        assert_eq!(unnested.graph.nodes[0].trigger().unwrap().scope.connection_ids, vec![provider.clone()]);
        assert_eq!(Action::get(&pool, &filtered.id).await.unwrap().graph.nodes[0].trigger().unwrap().group_ids, vec!["g1".to_string()]);
        assert!(Action::get(&pool, &filtered.id).await.unwrap().graph.nodes[0].config.get("group_id").is_none());
        let before = Action::get(&pool, &dragged.id).await.unwrap().updated_at;
        assert_eq!(migrate_connection_triggers(&pool, &profile).await.unwrap(), 0);
        assert_eq!(Action::get(&pool, &dragged.id).await.unwrap().updated_at, before);
        let mut only = Action::get(&pool, &unnested.id).await.unwrap();
        only.graph.nodes[0].config["scope"] = json!({ "connection_ids": [slack] });
        Action::save(&pool, &only).await.unwrap();
        let r = runner(&pool, &profile, true);
        assert!(dispatch_to(&r, &[TriggerOn::Arrival], &message, Some(&only.id)).await.unwrap().is_empty());
        assert_eq!(dispatch_to(&r, &[TriggerOn::Arrival], &message, Some(&dragged.id)).await.unwrap().len(), 0);
        let mut mail = Action::get(&pool, &dragged.id).await.unwrap();
        mail.graph.nodes[0].config["scope"]["providers"] = json!([]);
        Action::save(&pool, &mail).await.unwrap();
        assert_eq!(dispatch_to(&r, &[TriggerOn::Arrival], &message, Some(&mail.id)).await.unwrap().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_generic_trigger_filters_by_kind_connection_chat_group_tag_person_attachments_and_direction() {
        let pool = test_pool().await;
        let (profile, provider, conversation, message) = seed(&pool).await;
        let other = test_conversation(&pool, "Elsewhere", "direct_message").await;
        let tag = Tag::all(&pool, &profile).await.unwrap().remove(0);
        sqlx::query("INSERT INTO object_tag (object_id, tag_id, object_type, created_at) VALUES (?1, ?2, 'conversation', '')").bind(&conversation).bind(&tag.id).execute(&pool).await.unwrap();
        sqlx::query("UPDATE conversation SET person_id = 'p1' WHERE id = ?1").bind(&conversation).execute(&pool).await.unwrap();
        let mail = Group::post(&pool, &profile, &crate::models::group::PostGroupInput { name: "Mail".into(), kind: Some("query".into()), parent_id: None, tag_id: None, query: Some(crate::models::group::GroupQuery { types: vec!["email".into()], ..Default::default() }), icon: None, color: None }).await.unwrap();
        let chats = Group::post(&pool, &profile, &crate::models::group::PostGroupInput { name: "Chats".into(), kind: Some("query".into()), parent_id: None, tag_id: None, query: Some(crate::models::group::GroupQuery { types: vec!["channel".into()], ..Default::default() }), icon: None, color: None }).await.unwrap();
        let payload = payload_for_message(&pool, &profile, &message).await.unwrap();
        let admits = |config: Value| {
            let pool = pool.clone();
            let payload = payload.clone();
            async move { admitted(&pool, &serde_json::from_value::<TriggerConfig>(config).unwrap(), &payload).await.unwrap() }
        };
        assert!(admits(json!({ "on": "arrival" })).await);
        assert!(admits(json!({ "on": "arrival", "scope": { "providers": ["mock"] } })).await);
        assert!(!admits(json!({ "on": "arrival", "scope": { "providers": ["slack"] } })).await);
        assert!(!admits(json!({ "on": "arrival", "scope": { "providers": [provider] } })).await);
        assert!(admits(json!({ "on": "arrival", "scope": { "connection_ids": [provider] } })).await);
        assert!(!admits(json!({ "on": "arrival", "scope": { "connection_ids": ["someone else"] } })).await);
        assert!(admits(json!({ "on": "arrival", "scope": { "conversation_ids": [conversation] } })).await);
        assert!(!admits(json!({ "on": "arrival", "scope": { "conversation_ids": [other] } })).await);
        assert!(admits(json!({ "on": "arrival", "group_ids": [chats.id, mail.id] })).await);
        assert!(!admits(json!({ "on": "arrival", "group_ids": [chats.id] })).await);
        assert!(admits(json!({ "on": "arrival", "scope": { "tag_ids": [tag.id] } })).await);
        assert!(!admits(json!({ "on": "arrival", "scope": { "tag_ids": ["nope"] } })).await);
        assert!(admits(json!({ "on": "arrival", "scope": { "person_ids": ["p1"] } })).await);
        assert!(!admits(json!({ "on": "arrival", "scope": { "person_ids": ["p2"] } })).await);
        assert!(admits(json!({ "on": "arrival", "scope": { "has_attachments": false } })).await);
        assert!(!admits(json!({ "on": "arrival", "scope": { "has_attachments": true } })).await);
        assert!(admits(json!({ "on": "arrival", "message_kinds": ["chat"] })).await);
        assert!(!admits(json!({ "on": "arrival", "message_kinds": ["email"] })).await);
        assert!(admits(json!({ "on": "arrival", "direction": "incoming" })).await);
        assert!(!admits(json!({ "on": "arrival", "direction": "outgoing" })).await);
        assert!(admits(json!({ "on": "arrival", "direction": "any" })).await);
        assert!(!admits(json!({ "on": "mention" })).await);
        sqlx::query("UPDATE message SET has_attachments = 1, type = 'email', content = 'Hei @Test Profile, se vedlegget' WHERE id = ?1").bind(&message).execute(&pool).await.unwrap();
        let payload = payload_for_message(&pool, &profile, &message).await.unwrap();
        let config = |value: Value| serde_json::from_value::<TriggerConfig>(value).unwrap();
        assert!(passes(&config(json!({ "on": "arrival", "scope": { "has_attachments": true }, "message_kinds": ["email"] })), &payload));
        assert!(passes(&config(json!({ "on": "mention" })), &payload));
        let mut own = payload.clone();
        own["message"]["mine"] = json!(true);
        assert!(!passes(&config(json!({ "on": "arrival" })), &own));
        assert!(passes(&config(json!({ "on": "arrival", "direction": "outgoing" })), &own));
        assert!(passes(&config(json!({ "on": "sent" })), &own));
        assert!(passes(&config(json!({ "on": "reaction" })), &own));
        let mut action = reply_action(&profile);
        action.graph.nodes[0].config = json!({ "on": "arrival", "scope": { "connection_ids": [provider], "tag_ids": [tag.id] }, "message_kinds": ["email"] });
        let action = Action::insert(&pool, &action).await.unwrap();
        let r = runner(&pool, &profile, true);
        assert_eq!(dispatch_to(&r, &[TriggerOn::Arrival], &message, Some(&action.id)).await.unwrap().len(), 1);
        assert!(dispatch_to(&r, &[TriggerOn::Sent], &message, Some(&action.id)).await.unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn human_or_service_becomes_a_built_in_that_runs_when_switched_on() {
        let pool = test_pool().await;
        let (profile, _, _, message) = seed(&pool).await;
        sqlx::query("INSERT INTO person (id, profile_id, display_name, is_human, created_at, updated_at) VALUES ('p1', ?1, 'Anna', 1, '', '')").bind(&profile).execute(&pool).await.unwrap();
        sqlx::query("UPDATE conversation SET person_id = 'p1'").execute(&pool).await.unwrap();
        let starter = ActionGroup::post(&pool, &profile, &PostActionGroupInput { name: "Starter actions".into(), description: None, icon: None, parent_id: None }).await.unwrap();
        sqlx::query("UPDATE action_group SET key = ?1 WHERE id = ?2").bind(STARTER_GROUP).bind(&starter.id).execute(&pool).await.unwrap();
        let mut old = Action::blank(&profile, "Human or service");
        old.key = Some(CLASSIFY_SENDERS_KEY.into());
        old.group_id = Some(starter.id.clone());
        let mut draft = classify_senders_draft();
        draft.graph.nodes[0].body = "person != ()".into();
        apply_draft(&mut old, draft);
        normalize(&mut old.graph);
        old.enabled = false;
        let old = Action::insert(&pool, &old).await.unwrap();
        ensure_defaults(&pool, &profile).await.unwrap();
        ensure_defaults(&pool, &profile).await.unwrap();
        let groups = ActionGroup::all(&pool, &profile).await.unwrap();
        let shipped = groups.iter().find(|g| g.key.as_deref() == Some("builtin.actions")).unwrap();
        let built = Action::find(&pool, &profile, CLASSIFY_SENDERS_KEY).await.unwrap();
        assert_eq!(built.id, old.id);
        assert!(built.builtin && built.builtin_kind.as_deref() == Some(KIND_ACTION));
        assert_eq!(built.group_id.as_deref(), Some(shipped.id.as_str()));
        assert!(!built.enabled);
        let mut expected = classify_senders_draft().graph;
        normalize(&mut expected);
        assert_eq!(built.graph, expected);
        let all = Action::all(&pool, &profile).await.unwrap();
        let mine: Vec<&Action> = all.iter().filter(|a| a.name == "Human or service (mine)").collect();
        assert_eq!(mine.len(), 1);
        assert!(!mine[0].builtin && mine[0].group_id.as_deref() == Some(starter.id.as_str()) && mine[0].graph.nodes[0].body == "person != ()");
        let catalog_entry = catalog_entry(&built, &all.iter().map(|a| (a.id.clone(), a.clone())).collect(), &RequirementState::default());
        let requires: Vec<String> = serde_json::from_value(catalog_entry["requires"].clone()).unwrap();
        for key in [action_script::REQUIRES_AI_CONNECTION, action_script::REQUIRES_AI_MODEL] {
            assert!(requires.iter().any(|r| r == key), "{:?}", requires);
        }
        assert!(put_action(&pool, PutActionInput { action_id: built.id.clone(), name: Some("Mine".into()), ..Default::default() }).await.is_err());
        let mut r = runner(&pool, &profile, true);
        r.ai = Some(Arc::new(|_: &str, _: &Value| Ok(json!({ "kind": "human" }))));
        assert!(dispatch_to(&r, &[TriggerOn::Arrival], &message, Some(&built.id)).await.unwrap().is_empty());
        let on = put_action(&pool, PutActionInput { action_id: built.id.clone(), enabled: Some(true), ..Default::default() }).await.unwrap();
        assert!(on.enabled && on.builtin);
        let runs = dispatch_to(&r, &[TriggerOn::Arrival], &message, Some(&built.id)).await.unwrap();
        assert_eq!(runs.len(), 1, "{:?}", runs);
        assert!(dispatch_to(&r, &[TriggerOn::Arrival], &message, Some(&Action::find(&pool, &profile, "trigger:arrival").await.unwrap().id)).await.unwrap().is_empty());
        let fresh = test_pool().await;
        let (other, _, _, _) = seed(&fresh).await;
        ensure_defaults(&fresh, &other).await.unwrap();
        let installed = Action::find(&fresh, &other, CLASSIFY_SENDERS_KEY).await.unwrap();
        assert!(installed.builtin && installed.enabled);
        assert!(!Action::find(&fresh, &other, RECEIPTS_KEY).await.unwrap().enabled);
        assert!(!Action::all(&fresh, &other).await.unwrap().iter().any(|a| a.name.ends_with("(mine)")));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn old_scenarios_are_retired_and_built_ins_keep_their_shipped_prompts() {
        let pool = test_pool().await;
        let (profile, _, _, _) = seed(&pool).await;
        sqlx::query(
            "CREATE TABLE scenario (key TEXT NOT NULL, profile_id TEXT NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '', connection_id TEXT, model TEXT,
             system TEXT NOT NULL DEFAULT '', template TEXT NOT NULL, output_schema TEXT, enabled BOOLEAN NOT NULL DEFAULT 1, updated_at TEXT NOT NULL, PRIMARY KEY (key, profile_id))",
        )
        .execute(&pool)
        .await
        .unwrap();
        let summary = intelligence_service::seeds().into_iter().find(|s| s.key == "summarise_mail").unwrap();
        let sort = intelligence_service::seeds().into_iter().find(|s| s.key == "sort").unwrap();
        for (key, system, template, schema, model) in [("summarise_mail", "Ten words at most", summary.template, summary.schema, Some("small")), ("sort", sort.system, sort.template, sort.schema, None)] {
            sqlx::query("INSERT INTO scenario (key, profile_id, name, system, template, output_schema, model, updated_at) VALUES (?1, ?2, ?1, ?3, ?4, ?5, ?6, '')")
                .bind(key)
                .bind(&profile)
                .bind(system)
                .bind(template)
                .bind(schema)
                .bind(model)
                .execute(&pool)
                .await
                .unwrap();
        }
        ensure_defaults(&pool, &profile).await.unwrap();
        ensure_defaults(&pool, &profile).await.unwrap();
        let summarise = Action::find(&pool, &profile, "summarise_mail").await.unwrap();
        assert!(summarise.builtin);
        assert!(summarise.graph.nodes.iter().all(|n| n.config.get("prompts").is_none()));
        let sorter = Action::find(&pool, &profile, "sort_splits").await.unwrap();
        assert!(sorter.graph.nodes.iter().all(|n| n.config.get("prompts").is_none()));
        assert!(!crate::database::table_exists(&pool, "scenario").await.unwrap());
        assert!(crate::database::table_exists(&pool, RETIRED_SCENARIOS).await.unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn actions_compose_with_a_cycle_guard() {
        let pool = test_pool().await;
        let (profile, _, _, _) = seed(&pool).await;
        let mut inner = Action::blank(&profile, "Shout");
        inner.inputs = vec![Port::new("text", PortType::Text)];
        inner.graph = Graph { nodes: vec![node("s", NODE_SCRIPT, json!({ "inputs": [{ "name": "text", "type": "text" }], "outputs": [{ "name": "loud", "type": "text" }] }), "#{ loud: text.to_upper() + \"!\" }")], edges: vec![] };
        let inner = Action::insert(&pool, &inner).await.unwrap();
        let mut outer = Action::blank(&profile, "Outer");
        outer.inputs = vec![Port::new("text", PortType::Text)];
        outer.graph = Graph {
            nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "manual" }), ""), node("n", &format!("action:{}", inner.id), json!({}), "")],
            edges: vec![edge("t", "text", "n", "text")],
        };
        let outer = Action::insert(&pool, &outer).await.unwrap();
        let r = runner(&pool, &profile, false);
        let run = execute(&r, &outer, Start::Invoke { inputs: Map::from_iter([("text".to_string(), json!("hei"))]), payload: Value::Null }, 0, vec![outer.id.clone()]).await.unwrap();
        assert_eq!(run.outputs, json!({ "loud": "HEI!" }), "{:?}", run);
        let mut looping = inner.clone();
        looping.graph.nodes.push(node("back", &format!("action:{}", outer.id), json!({}), ""));
        let actions: HashMap<String, Action> = [(outer.id.clone(), outer.clone()), (inner.id.clone(), looping.clone())].into_iter().collect();
        assert!(check(&draft_of(&looping), &actions, Some(&inner.id)).iter().any(|d| d.message.contains("cannot use itself")));
        let guarded = execute(&r, &looping, Start::Invoke { inputs: Map::from_iter([("text".to_string(), json!("x"))]), payload: Value::Null }, MAX_DEPTH, vec![inner.id.clone()]).await.unwrap();
        assert_eq!(guarded.status, "failed");
        assert!(guarded.error.unwrap_or_default().contains("nest too deep"));
    }

    #[tokio::test]
    async fn catalog_entries_carry_the_nested_graph_and_its_diagnostics() {
        let pool = test_pool().await;
        let (profile, _, _, _) = seed(&pool).await;
        let mut inner = Action::blank(&profile, "Shout");
        inner.key = Some("shout".into());
        inner.graph = Graph { nodes: vec![node("a", NODE_SCRIPT, json!({}), "#{"), node("b", NODE_SCRIPT, json!({}), "")], edges: vec![] };
        let inner = Action::insert(&pool, &inner).await.unwrap();
        let listed = catalog(&pool, &profile).await.unwrap();
        let entry = listed.actions.iter().find(|a| a["id"] == json!(inner.id)).unwrap();
        assert_eq!(entry["key"], json!("shout"));
        assert_eq!(entry["enabled"], json!(inner.enabled));
        assert_eq!(entry["builtin"], json!(false));
        assert_eq!(entry["graph"]["nodes"].as_array().map(Vec::len), Some(2));
        assert!(entry["diagnostics"].as_array().unwrap().iter().any(|d| d["node"] == json!("a") && d["severity"] == json!("error")));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn waits_pause_and_resume() {
        let pool = test_pool().await;
        let (profile, _, conversation, _) = seed(&pool).await;
        let mut action = Action::blank(&profile, "Later");
        action.graph = Graph {
            nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "manual" }), ""), node("w", "builtin:wait", json!({ "values": { "minutes": 5 } }), ""), node("d", "builtin:done", json!({ "values": {} }), "")],
            edges: vec![edge("t", "conversation", "w", "value"), edge("w", "value", "d", "conversation")],
        };
        normalize(&mut action.graph);
        let action = Action::insert(&pool, &action).await.unwrap();
        let r = runner(&pool, &profile, false);
        let payload = json!({ "conversation": { "id": conversation } });
        let run = execute(&r, &action, Start::Invoke { inputs: Map::new(), payload }, 0, vec![action.id.clone()]).await.unwrap();
        assert_eq!(run.status, "waiting");
        record(&r, &run).await.unwrap();
        let later = Local::now() + Duration::minutes(6);
        let report = tick(&r, later).await.unwrap();
        assert_eq!(report.resumed, 1);
        let done: bool = sqlx::query_scalar("SELECT is_archived FROM conversation WHERE id = ?1").bind(&conversation).fetch_one(&pool).await.unwrap();
        assert!(done);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn scheduled_sends_are_picked_when_due() {
        let pool = test_pool().await;
        let (profile, _, conversation, _) = seed(&pool).await;
        let soon = (Utc::now() + Duration::minutes(10)).to_rfc3339();
        let schedule = MessageSchedule::post(&pool, &profile, &PostMessageScheduleInput { conversation_id: conversation.clone(), provider_id: None, content: "Hei".into(), content_type: None, at: Some(soon), action_id: None, metadata: None, attachments: None }).await.unwrap();
        assert!(MessageSchedule::due(&pool, &profile, &Utc::now().to_rfc3339()).await.unwrap().is_empty());
        let due = MessageSchedule::due(&pool, &profile, &(Utc::now() + Duration::minutes(11)).to_rfc3339()).await.unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, schedule.id);
        assert!(MessageSchedule::post(&pool, &profile, &PostMessageScheduleInput { conversation_id: conversation, provider_id: None, content: "x".into(), content_type: None, at: None, action_id: None, metadata: None, attachments: None }).await.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_library_reads_and_files_in_a_live_run() {
        let pool = test_pool().await;
        let (profile, _, conversation, message) = seed(&pool).await;
        let mut action = Action::blank(&profile, "File it");
        action.graph = Graph {
            nodes: vec![
                node("t", NODE_TRIGGER, json!({ "on": "manual" }), ""),
                node(
                    "s",
                    NODE_SCRIPT,
                    json!({ "inputs": [{ "name": "message", "type": "message" }], "outputs": [{ "name": "summary", "type": "text" }] }),
                    "let m = get_message(message.id); let c = get_conversation(m.conversation_id); tag_add(c, \"Kunde\"); tag_add(m, \"Faktura\"); move(c, \"Team\"); #{ summary: summarize(m.text) + \" (\" + list_messages(c).len() + \")\" }",
                ),
            ],
            edges: vec![edge("t", "message", "s", "message")],
        };
        let action = Action::insert(&pool, &action).await.unwrap();
        let r = runner(&pool, &profile, false);
        let payload = payload_for_message(&pool, &profile, &message).await.unwrap();
        let run = execute(&r, &action, Start::Invoke { inputs: Map::new(), payload }, 0, vec![action.id.clone()]).await.unwrap();
        assert_eq!(run.status, "done", "{:?}", run);
        assert_eq!(run.outputs, json!({ "summary": "An invoice is overdue (1)" }));
        let split: String = sqlx::query_scalar("SELECT l.name FROM object_tag cl JOIN tag l ON l.id = cl.tag_id WHERE cl.object_type = 'conversation' AND cl.object_id = ?1 AND l.kind = 'split'").bind(&conversation).fetch_one(&pool).await.unwrap();
        assert_eq!(split, "Team");
        let tagged: Vec<(String, String, String)> = sqlx::query_as("SELECT ot.object_type, ot.object_id, t.name FROM object_tag ot JOIN tag t ON t.id = ot.tag_id WHERE t.kind = 'tag' ORDER BY t.name").fetch_all(&pool).await.unwrap();
        assert_eq!(tagged, vec![("message".to_string(), message.clone(), "Faktura".to_string()), ("conversation".to_string(), conversation.clone(), "Kunde".to_string())]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sends_stop_at_the_hourly_limit() {
        let pool = test_pool().await;
        let (profile, _, conversation, _) = seed(&pool).await;
        let mut action = reply_action(&profile);
        action.max_sends_per_hour = 1;
        let action = Action::insert(&pool, &action).await.unwrap();
        let log = json!([{ "node": "send", "kind": "effect", "status": "done", "detail": format!("{}{}", SENT_PREFIX, conversation) }]).to_string();
        sqlx::query("INSERT INTO action_run (id, profile_id, action_id, status, dry_run, log, started_at) VALUES ('earlier', ?1, ?2, 'done', 0, ?3, ?4)")
            .bind(&profile)
            .bind(&action.id)
            .bind(&log)
            .bind(Utc::now().to_rfc3339())
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(ActionRun::sent_since(&pool, &action.id, &(Utc::now() - Duration::hours(1)).to_rfc3339()).await.unwrap(), 1);
        let r = runner(&pool, &profile, false);
        let payload = json!({ "conversation": { "id": conversation }, "text": "invoice" });
        let run = execute(&r, &action, Start::Trigger { node: "t".into(), payload }, 0, vec![action.id.clone()]).await.unwrap();
        assert_eq!(run.status, "failed");
        assert!(run.error.unwrap().contains("limit of 1"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn code_round_trips_through_storage_and_groups_nest() {
        let pool = test_pool().await;
        let (profile, _, _, _) = seed(&pool).await;
        let created = post_action(&pool, &profile, PostActionInput { name: "Acknowledge".into(), graph: Some(reply_action(&profile).graph), ..Default::default() }).await.unwrap();
        assert!(created.enabled);
        let code = code_of(&created);
        let edited = code.replace("Acknowledge", "Acknowledge politely");
        let saved = put_action(&pool, PutActionInput { action_id: created.id.clone(), code: Some(edited), ..Default::default() }).await.unwrap();
        assert_eq!(saved.name, "Acknowledge politely");
        assert_eq!(saved.graph, created.graph);
        let broken = put_action(&pool, PutActionInput { action_id: created.id.clone(), code: Some("action(#{ name: \"x\" }".into()), ..Default::default() }).await.unwrap_err();
        assert!(broken.contains("line"));
        let parent = ActionGroup::post(&pool, &profile, &PostActionGroupInput { name: "Mail".into(), parent_id: None, description: None, icon: Some("lucide:mail".into()) }).await.unwrap();
        let child = ActionGroup::post(&pool, &profile, &PostActionGroupInput { name: "Invoices".into(), parent_id: Some(parent.id.clone()), description: None, icon: None }).await.unwrap();
        assert!(ActionGroup::put(&pool, &crate::models::action::PutActionGroupInput { group_id: parent.id.clone(), name: None, parent_id: Some(Some(child.id.clone())), description: None, icon: None, position: None }).await.is_err());
        ActionGroup::delete(&pool, &parent.id).await.unwrap();
        assert_eq!(ActionGroup::get(&pool, &child.id).await.unwrap().parent_id, None);
        let assisted = assist(&pool, &profile, AssistInput { action_id: Some(created.id.clone()), instruction: "Add a trigger".into(), ..Default::default() }, runner(&pool, &profile, true).ai).await.unwrap();
        assert_eq!(assisted.reply, "Adds a trigger");
        assert!(assisted.errors.is_empty(), "{:?}", assisted.errors);
        assert_eq!(assisted.previews[0].after.as_ref().unwrap().graph.nodes[0].kind, NODE_TRIGGER);
    }

    #[test]
    fn nodes_need_the_settings_their_functions_and_types_use() {
        use action_script::{REQUIRES_ACCOUNT, REQUIRES_ACTIONS_ENABLED, REQUIRES_AI_CONNECTION, REQUIRES_AI_ENABLED, REQUIRES_AI_MODEL};
        let none = HashMap::new();
        let ai = vec![REQUIRES_AI_ENABLED, REQUIRES_AI_CONNECTION, REQUIRES_AI_MODEL];
        assert_eq!(node_requirements(&node("t", NODE_TRIGGER, json!({ "on": "arrival" }), ""), &none), vec![REQUIRES_ACTIONS_ENABLED]);
        assert!(node_requirements(&node("t", NODE_TRIGGER, json!({ "on": "manual" }), ""), &none).is_empty());
        let mut judged = vec![REQUIRES_ACTIONS_ENABLED];
        judged.extend(ai.clone());
        assert_eq!(node_requirements(&node("t", NODE_TRIGGER, json!({ "on": "schedule" }), "ai_json(\"Urgent?\", #{}).yes"), &none), judged);
        assert_eq!(node_requirements(&node("a", "builtin:ai_reply", json!({}), ""), &none), ai);
        assert_eq!(node_requirements(&node("r", "builtin:reply", json!({}), "ignored()"), &none), vec![REQUIRES_ACCOUNT]);
        assert!(node_requirements(&node("d", "builtin:done", json!({}), ""), &none).is_empty());
        assert_eq!(node_requirements(&node("s", NODE_SCRIPT, json!({}), "// ai(text)\nsend(person, \"ai(x)\")"), &none), vec![REQUIRES_ACCOUNT]);
        assert_eq!(node_requirements(&node("c", NODE_CHAT, json!({ "mode": "write" }), ""), &none), vec![REQUIRES_ACCOUNT]);
        assert!(node_requirements(&node("c", NODE_CHAT, json!({ "mode": "read" }), ""), &none).is_empty());
        let mut inner = Action::blank("p", "Inner");
        inner.graph = Graph { nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "arrival" }), ""), node("s", NODE_SCRIPT, json!({}), "let x = summarize(text);\nreply(conversation, x)")], edges: Vec::new() };
        let known: HashMap<String, Action> = [(inner.id.clone(), inner.clone())].into_iter().collect();
        let mut both = ai.clone();
        both.push(REQUIRES_ACCOUNT);
        assert_eq!(node_requirements(&node("n", &format!("{}{}", ACTION_PREFIX, inner.id), json!({}), ""), &known), both);
        let draft = ActionDraft { name: "x".into(), description: String::new(), icon: None, max_sends_per_hour: None, inputs: Vec::new(), graph: Graph { nodes: vec![node("t", NODE_TRIGGER, json!({ "on": "arrival" }), ""), node("a", "builtin:ai", json!({}), "")], edges: Vec::new() } };
        let state = RequirementState { actions: true, intelligence: true, connection: false, model: false, accounts: true, voice: true, voice_models: true };
        let found = unmet(&draft, &known, &state);
        assert_eq!(found.iter().map(|d| (d.node.as_deref().unwrap(), d.setting.as_deref().unwrap(), d.severity.as_str())).collect::<Vec<_>>(), vec![("a", REQUIRES_AI_CONNECTION, "requirement"), ("a", REQUIRES_AI_MODEL, "requirement")]);
        assert!(unmet(&draft, &known, &RequirementState { actions: true, intelligence: true, connection: true, model: true, accounts: true, voice: true, voice_models: true }).is_empty());
        assert!(check(&draft, &known, None).iter().all(|d| d.severity != "requirement"));
    }

    #[tokio::test]
    async fn requirements_resolve_against_settings_and_connections() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let state = requirement_state(&pool, &profile).await.unwrap();
        assert!(state.actions && state.intelligence);
        assert!(!state.accounts);
        AppSetting::put(&pool, app_setting::ACTIONS_ENABLED, "false").await.unwrap();
        AppSetting::put(&pool, app_setting::ASSISTANT_ENABLED, "false").await.unwrap();
        test_provider(&pool, &profile, "slack").await;
        let state = requirement_state(&pool, &profile).await.unwrap();
        assert!(!state.actions && !state.intelligence && state.accounts);
        let listed = catalog(&pool, &profile).await.unwrap();
        assert!(listed.requirements.iter().any(|r| r.key == action_script::REQUIRES_ACTIONS_ENABLED && !r.met));
        assert!(listed.requirements.iter().any(|r| r.key == action_script::REQUIRES_ACCOUNT && r.met));
    }

    #[tokio::test]
    async fn the_ai_choice_per_feature_moves_onto_its_actions_once() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let put = |feature: &str, part: &str| app_setting::feature_key(feature, part);
        for (key, value) in [
            (put("sorting", app_setting::FEATURE_CONNECTION), "sorter"),
            (put("sorting", app_setting::FEATURE_MODEL), "sort-model"),
            (put("summaries", app_setting::FEATURE_MODEL), "summary-model"),
            (put("receipts", app_setting::FEATURE_MODEL), "qwen3:4b"),
            (put("scripts", app_setting::FEATURE_MODEL), "script-model"),
        ] {
            sqlx::query("INSERT INTO app_setting (key, value, updated_at) VALUES (?1, ?2, ?3)").bind(&key).bind(value).bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
        }
        ensure_defaults(&pool, &profile).await.unwrap();
        let ai = |connection: Option<&str>, model: &str| AiChoice { connection_id: connection.map(String::from), model: Some(model.into()) };
        assert_eq!(Action::find(&pool, &profile, "sort_splits").await.unwrap().ai, ai(Some("sorter"), "sort-model"));
        assert_eq!(Action::find(&pool, &profile, CLASSIFY_SENDERS_KEY).await.unwrap().ai, ai(Some("sorter"), "sort-model"));
        assert_eq!(Action::find(&pool, &profile, "summarise_mail").await.unwrap().ai, ai(None, "summary-model"));
        assert_eq!(Action::find(&pool, &profile, RECEIPTS_KEY).await.unwrap().ai, ai(None, "qwen3:4b"));
        assert_eq!(Action::find(&pool, &profile, "recognise_invites").await.unwrap().ai, ai(None, "script-model"));
        assert!(Action::find(&pool, &profile, DETECT_NEWSLETTERS_KEY).await.unwrap().ai.is_empty());
        assert_eq!(AppSetting::get(&pool, &put("threads", app_setting::FEATURE_MODEL)).await.unwrap().as_deref(), Some("summary-model"));
        assert_eq!(AppSetting::get(&pool, &format!("{}_retired", put("sorting", app_setting::FEATURE_MODEL))).await.unwrap().as_deref(), Some("sort-model"));
        assert_eq!(AppSetting::get(&pool, &put("sorting", app_setting::FEATURE_MODEL)).await.unwrap(), None);
        let mut sort = Action::find(&pool, &profile, "sort_splits").await.unwrap();
        sort = put_action(&pool, PutActionInput { action_id: sort.id.clone(), ai: Some(AiChoice::default()), ..Default::default() }).await.unwrap();
        assert!(sort.ai.is_empty());
        let builtin = Action::find(&pool, &profile, RECEIPTS_KEY).await.unwrap();
        let reset = put_action(&pool, PutActionInput { action_id: builtin.id.clone(), ai: Some(AiChoice::default()), ..Default::default() }).await.unwrap();
        assert!(reset.ai.is_empty() && reset.graph == builtin.graph);
        ensure_defaults(&pool, &profile).await.unwrap();
        assert!(Action::find(&pool, &profile, "sort_splits").await.unwrap().ai.is_empty());
        assert!(Action::find(&pool, &profile, RECEIPTS_KEY).await.unwrap().ai.is_empty());
    }
}
