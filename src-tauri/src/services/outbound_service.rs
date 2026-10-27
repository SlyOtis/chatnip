use crate::models::content::ContentItem;
use crate::models::{Attachment, Message, OutgoingAttachmentInput, Profile, SendMessageInput};
use crate::providers::models::{Delivery, Provider};
use crate::providers::traits::{OutgoingAttachment, OutgoingMessage, OutgoingReference, ProviderTrait, ResponseMessage};
use crate::services::attachment_service;
use crate::services::connection_service::{ensure_fresh, provider_impl};
use crate::services::content_service;
use crate::services::{MessageLoadOptions, MessageService};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use crate::models::app_setting::{self, AppSetting};
use crate::models::outbound_hold::{self, OutboundHold, PostOutboundHoldInput};
use crate::services::notification_service;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Pool, Row, Sqlite};
use std::path::Path;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

pub struct Route {
    pub provider: Provider,
    pub conversation_external_id: String,
}

pub async fn route(pool: &Pool<Sqlite>, conversation_id: &str, provider_id: Option<&str>) -> Result<Route, String> {
    let row = match provider_id {
        Some(id) => sqlx::query("SELECT provider_id, external_id FROM conversation_provider WHERE conversation_id = ?1 AND provider_id = ?2")
            .bind(conversation_id)
            .bind(id)
            .fetch_optional(pool)
            .await,
        None => sqlx::query("SELECT provider_id, external_id FROM conversation_provider WHERE conversation_id = ?1 ORDER BY is_primary DESC, created_at ASC LIMIT 1")
            .bind(conversation_id)
            .fetch_optional(pool)
            .await,
    }
    .map_err(|e| format!("Failed to find where the conversation lives: {}", e))?
    .ok_or_else(|| "This conversation is local only; link it to a connection before sending".to_string())?;
    let provider_id: String = row.get("provider_id");
    let conversation_external_id: String = row.get("external_id");
    let provider = Provider::load_from_db(pool, &provider_id).await?;
    Ok(Route { provider, conversation_external_id })
}

pub async fn route_for_message(pool: &Pool<Sqlite>, message_id: &str) -> Result<(Route, String), String> {
    let row = sqlx::query("SELECT provider_id, external_id, conversation_id FROM message WHERE id = ?1")
        .bind(message_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to load the message: {}", e))?
        .ok_or_else(|| "Message not found".to_string())?;
    let provider_id: String = row.get("provider_id");
    let external_id: String = row.get("external_id");
    let conversation_id: Option<String> = row.get("conversation_id");
    let conversation_id = conversation_id.ok_or_else(|| "The message belongs to no conversation".to_string())?;
    let route = route(pool, &conversation_id, Some(&provider_id)).await?;
    Ok((route, external_id))
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str()
                        .map(String::from)
                        .or_else(|| item["email"].as_str().map(String::from))
                        .or_else(|| item["address"].as_str().map(String::from))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub async fn attachments(pool: &Pool<Sqlite>, app_data: &Path, inputs: &[OutgoingAttachmentInput]) -> Result<(Vec<OutgoingAttachment>, Vec<String>), String> {
    let mut out = Vec::new();
    let mut used = Vec::new();
    for input in inputs {
        if let Some(content_id) = input.content_id.as_deref().filter(|id| !id.is_empty()) {
            let item = ContentItem::get(pool, content_id).await?.ok_or_else(|| format!("{} is not in your content", content_id))?;
            let bytes = content_service::read(app_data, &item)?;
            let extension = item.path.rsplit('.').next().unwrap_or("bin");
            let name = input.name.clone().unwrap_or_else(|| format!("{}.{}", item.name.replace(['/', '\\'], "-"), extension));
            out.push(OutgoingAttachment { name, mime_type: item.mime_type.clone(), bytes });
            used.push(item.id);
            continue;
        }
        let data = input.data.as_deref().filter(|d| !d.trim().is_empty()).ok_or_else(|| "An attachment needs a content_id or data".to_string())?;
        let raw = data.split_once(";base64,").map(|(_, rest)| rest).unwrap_or(data);
        let bytes = STANDARD.decode(raw.trim()).map_err(|e| format!("The attachment data is not valid base64: {}", e))?;
        let mime_type = crate::models::content::sniff_mime(&bytes).map(String::from).or_else(|| input.mime_type.clone()).unwrap_or_else(|| "application/octet-stream".to_string());
        let name = input.name.clone().unwrap_or_else(|| format!("attachment.{}", crate::models::content::extension_for(&mime_type)));
        out.push(OutgoingAttachment { name, mime_type, bytes });
    }
    Ok((out, used))
}

pub struct Quoted {
    pub message_id: String,
    pub provider_id: String,
    pub conversation_id: Option<String>,
    pub is_mail: bool,
    pub content: String,
    pub files: Vec<Attachment>,
    pub reference: OutgoingReference,
}

pub async fn quoted(pool: &Pool<Sqlite>, message_id: &str) -> Result<Quoted, String> {
    let row = sqlx::query(
        "SELECT m.id, m.external_id, m.provider_id, m.type, m.conversation_id, m.sender_id, m.sender_name, m.sender_email, m.content, m.content_type,
                m.subject, m.timestamp, m.metadata, m.headers, m.attachments, m.recipients, c.name AS conversation_name, cp.external_id AS conversation_external_id,
                (SELECT pu.external_id FROM provider_user pu WHERE pu.user_id = m.sender_id AND pu.provider_id = m.provider_id LIMIT 1) AS sender_external_id
         FROM message m
         LEFT JOIN conversation c ON c.id = m.conversation_id
         LEFT JOIN conversation_provider cp ON cp.conversation_id = m.conversation_id AND cp.provider_id = m.provider_id
         WHERE m.id = ?1",
    )
    .bind(message_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to load the message: {}", e))?
    .ok_or_else(|| format!("No message with id {}", message_id))?;
    let content: String = row.try_get::<Option<String>, _>("content").ok().flatten().unwrap_or_default();
    let content_type: String = row.try_get::<Option<String>, _>("content_type").ok().flatten().unwrap_or_else(|| "text".to_string());
    let text = crate::utils::text_extraction::extract_text_from_content(&content, &content_type).trim().to_string();
    let metadata: Value = row.try_get::<Option<String>, _>("metadata").ok().flatten().and_then(|m| serde_json::from_str(&m).ok()).unwrap_or(Value::Null);
    let headers: Option<String> = row.try_get("headers").ok().flatten();
    let files = Message::parse_attachments(row.try_get::<Option<String>, _>("attachments").ok().flatten().as_deref());
    let to = strings(serde_json::from_str::<Value>(&row.try_get::<Option<String>, _>("recipients").ok().flatten().unwrap_or_default()).ok().as_ref());
    let timestamp: String = row.try_get::<Option<String>, _>("timestamp").ok().flatten().unwrap_or_default();
    let kind: String = row.try_get::<Option<String>, _>("type").ok().flatten().unwrap_or_default();
    let reference = OutgoingReference {
        external_id: row.get("external_id"),
        conversation_external_id: row.try_get("conversation_external_id").ok().flatten(),
        conversation_name: row.try_get("conversation_name").ok().flatten(),
        sender_id: row.try_get("sender_external_id").ok().flatten(),
        sender_name: row.try_get::<Option<String>, _>("sender_name").ok().flatten().or_else(|| row.try_get("sender_email").ok().flatten()).unwrap_or_else(|| "Unknown".to_string()),
        sender_email: row.try_get("sender_email").ok().flatten(),
        text,
        subject: row.try_get("subject").ok().flatten(),
        timestamp: chrono::DateTime::parse_from_rfc3339(&timestamp).map(|t| t.with_timezone(&Utc)).unwrap_or_default(),
        internet_message_id: metadata["internet_message_id"].as_str().map(String::from),
        references: headers.as_deref().and_then(|h| crate::content::headers::value(h, "References")),
        to,
        attachments: files.iter().map(|f| f.name.clone()).collect(),
        native: false,
    };
    Ok(Quoted {
        message_id: row.get("id"),
        provider_id: row.get("provider_id"),
        conversation_id: row.try_get("conversation_id").ok().flatten(),
        is_mail: kind == "email",
        content,
        files,
        reference,
    })
}

fn excerpt(text: &str, limit: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= limit {
        return flat;
    }
    format!("{}…", flat.chars().take(limit).collect::<String>().trim_end())
}

pub fn quote_block(reference: &OutgoingReference) -> String {
    format!("> **{}**: {}\n\n", reference.sender_name, excerpt(&reference.text, 200))
}

pub fn attribution(reference: &OutgoingReference) -> String {
    let place = reference.conversation_name.as_deref().filter(|n| !n.trim().is_empty()).map(|n| format!(" in {}", n)).unwrap_or_default();
    let mut out = format!("**Forwarded from {}{}** · {}\n", reference.sender_name, place, reference.timestamp.format("%Y-%m-%d %H:%M"));
    for line in reference.text.lines() {
        out.push_str(&format!("> {}\n", line));
    }
    if !reference.attachments.is_empty() {
        out.push_str(&format!("\nAttachments: {}\n", reference.attachments.join(", ")));
    }
    out
}

pub fn forward_delivery(sources: &[&str], target: &Provider, target_is_mail: bool) -> Delivery {
    if target_is_mail {
        return Delivery::Mail;
    }
    if target.capabilities.forward == Delivery::Native && !sources.is_empty() && sources.iter().all(|id| *id == target.id) {
        return Delivery::Native;
    }
    Delivery::Repost
}

pub fn reply_delivery(quoted_provider: &str, quoted_is_mail: bool, target: &Provider) -> Delivery {
    if quoted_provider != target.id {
        return Delivery::Repost;
    }
    match target.capabilities.reply_quote {
        Delivery::Native | Delivery::Mail => target.capabilities.reply_quote,
        _ if quoted_is_mail => Delivery::Mail,
        _ => Delivery::Repost,
    }
}

fn carries(provider: &Provider, is_mail: bool, mime: &str) -> bool {
    provider.capabilities.supports_attachments && (is_mail || provider.provider_type != "microsoft" || mime.starts_with("image/"))
}

async fn originals(pool: &Pool<Sqlite>, app_data: &Path, quoted: &Quoted, provider: &Provider, is_mail: bool) -> Vec<OutgoingAttachment> {
    let mut out = Vec::new();
    for file in quoted.files.iter().filter(|f| !f.is_inline && carries(provider, is_mail, &f.content_type)) {
        if let Ok(bytes) = attachment_service::bytes(pool, app_data, &quoted.message_id, file).await {
            out.push(OutgoingAttachment { name: file.name.clone(), mime_type: file.content_type.clone(), bytes });
        }
    }
    out
}

async fn apply_quote(pool: &Pool<Sqlite>, provider: &Provider, draft: &mut OutgoingMessage, local_id: &str) -> Result<Option<String>, String> {
    let quoted = quoted(pool, local_id).await?;
    match reply_delivery(&quoted.provider_id, quoted.is_mail, provider) {
        Delivery::Repost => {
            draft.content = format!("{}{}", quote_block(&quoted.reference), draft.content);
            if draft.content_type != "html" {
                draft.content_type = "markdown".to_string();
            }
            Ok(Some(quoted.message_id))
        }
        _ => {
            draft.in_reply_to = Some(quoted.reference.external_id.clone());
            draft.quote = Some(OutgoingReference { native: true, ..quoted.reference });
            Ok(None)
        }
    }
}

struct Connected {
    route: Route,
    provider: Provider,
    client: std::sync::Arc<dyn crate::providers::traits::ProviderTrait>,
    auth: crate::providers::models::ProviderAuth,
}

async fn connect(pool: &Pool<Sqlite>, conversation_id: &str, provider_id: Option<&str>) -> Result<Connected, String> {
    let route = route(pool, conversation_id, provider_id).await?;
    let mut provider = route.provider.clone();
    let client = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, client.clone()).await?;
    Ok(Connected { route, provider, client, auth })
}

async fn mention_targets(pool: &Pool<Sqlite>, provider_id: &str, content: &str) -> Result<String, String> {
    let mut channels = std::collections::HashMap::new();
    for id in crate::content::markdown::channel_ids(content) {
        let external: Option<String> = sqlx::query_scalar("SELECT external_id FROM conversation_provider WHERE provider_id = ?1 AND (conversation_id = ?2 OR external_id = ?2) LIMIT 1")
            .bind(provider_id)
            .bind(&id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to resolve the channel: {}", e))?;
        if let Some(external) = external {
            channels.insert(id, external);
        }
    }
    let content = crate::content::markdown::with_channel_ids(content, &channels);
    let content = content.as_str();
    let ids = crate::content::markdown::mention_ids(content);
    if ids.is_empty() {
        return Ok(content.to_string());
    }
    let mut found = std::collections::HashMap::new();
    for id in ids {
        let external: Option<String> = sqlx::query_scalar(
            "SELECT pu.external_id FROM provider_user pu WHERE pu.provider_id = ?1 AND (pu.user_id = ?2 OR pu.external_id = ?2)
             UNION ALL
             SELECT pu.external_id FROM provider_user pu
               JOIN person_identity target ON target.user_id = pu.user_id
               JOIN person_identity source ON source.person_id = target.person_id
               LEFT JOIN provider_user sp ON sp.user_id = source.user_id
             WHERE pu.provider_id = ?1 AND (source.user_id = ?2 OR source.external_id = ?2 OR sp.external_id = ?2 OR source.person_id = ?2)
             LIMIT 1",
        )
        .bind(provider_id)
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to resolve who is mentioned: {}", e))?;
        if let Some(external) = external {
            found.insert(id, external);
        }
    }
    Ok(crate::content::markdown::with_mention_ids(content, &found))
}

pub async fn send(pool: &Pool<Sqlite>, app_data: &Path, input: SendMessageInput) -> Result<Message, String> {
    let connected = connect(pool, &input.conversation_id, input.provider_id.as_deref()).await?;
    let provider = &connected.provider;
    let (files, used_content) = attachments(pool, app_data, input.attachments.as_deref().unwrap_or(&[])).await?;
    if !files.is_empty() && !provider.capabilities.supports_attachments {
        return Err(format!("{} cannot carry files; the GIF would be dropped", provider.name));
    }
    let metadata = input.metadata.unwrap_or(Value::Null);
    let thread_external_id = match metadata["thread_message_id"].as_str() {
        Some(local_id) => sqlx::query_scalar::<_, String>("SELECT external_id FROM message WHERE id = ?1")
            .bind(local_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to resolve the thread: {}", e))?,
        None => None,
    };
    let mut draft = OutgoingMessage {
        content: input.content.clone(),
        content_type: input.content_type.clone().unwrap_or_else(|| "text".to_string()),
        html: metadata["html"].as_str().filter(|h| !h.trim().is_empty()).map(String::from),
        subject: metadata["subject"].as_str().map(String::from),
        to: strings(metadata.get("to")),
        cc: strings(metadata.get("cc")),
        bcc: strings(metadata.get("bcc")),
        thread_external_id,
        attachments: files,
        broadcast: metadata["broadcast"].as_bool().unwrap_or(false),
        from: metadata["from"].as_str().filter(|f| !f.trim().is_empty()).map(String::from),
        ..Default::default()
    };
    let mut local_reply = None;
    if let Some(local_id) = metadata["in_reply_to"].as_str().filter(|id| !id.is_empty()) {
        local_reply = apply_quote(pool, provider, &mut draft, local_id).await?;
        if local_reply.is_some() {
            draft.html = None;
        }
    }
    let mut forwarded_from = Vec::new();
    for local_id in strings(metadata.get("forward_of")) {
        let quoted = quoted(pool, &local_id).await?;
        let is_mail = draft.subject.is_some() || quoted.is_mail;
        draft.attachments.extend(originals(pool, app_data, &quoted, provider, is_mail).await);
        draft.forwarded.push(OutgoingReference { native: quoted.provider_id == provider.id, ..quoted.reference });
        forwarded_from.push(local_id);
    }
    if draft.content_type == "markdown" {
        draft.content = mention_targets(pool, &provider.id, &draft.content).await?;
    }
    let mailed = provider.capabilities.reply_quote == Delivery::Mail || draft.subject.is_some() || !draft.to.is_empty() || conversation_kind(pool, &input.conversation_id).await?.as_deref() == Some("email");
    if mailed && metadata["signature"].as_bool() != Some(false) {
        if let Some(signature) = signature_for(pool, provider, &input.conversation_id, &draft, &metadata).await? {
            content_service::put_signed(&mut draft, &signature);
        }
    }
    let original = crate::services::emoji_service::put_draft(pool, provider, &connected.route.conversation_external_id, &mut draft).await?;
    let mut sent = connected.client.send_message(&connected.auth, &connected.route.conversation_external_id, &draft).await?;
    crate::services::emoji_service::put_sent(&mut sent, &draft.content, original);
    for content_id in &used_content {
        ContentItem::touch(pool, content_id).await?;
    }
    if let Some(local) = local_reply {
        sent.reply_to_id = Some(local);
    }
    if !forwarded_from.is_empty() {
        sent.metadata.insert("forwarded_from".to_string(), serde_json::json!(forwarded_from));
    }
    store(pool, provider, &input.conversation_id, &sent).await
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ForwardTarget {
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub person_id: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub to: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PostMessageForwardInput {
    pub message_ids: Vec<String>,
    pub targets: Vec<ForwardTarget>,
    #[serde(default)]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PostMessageReplyInput {
    pub message_id: String,
    pub content: String,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub reply_all: bool,
    #[serde(default)]
    pub attachments: Option<Vec<OutgoingAttachmentInput>>,
}

async fn target_conversation(pool: &Pool<Sqlite>, target: &ForwardTarget) -> Result<String, String> {
    if let Some(id) = target.conversation_id.as_deref().filter(|id| !id.is_empty()) {
        return Ok(id.to_string());
    }
    let person = target.person_id.as_deref().filter(|id| !id.is_empty()).ok_or_else(|| "A forward target needs a conversation or a person".to_string())?;
    sqlx::query_scalar::<_, String>("SELECT c.id FROM conversation c JOIN conversation_provider cp ON cp.conversation_id = c.id WHERE c.person_id = ?1 AND c.deleted_at IS NULL ORDER BY c.last_activity DESC LIMIT 1")
        .bind(person)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to find a conversation with the person: {}", e))?
        .ok_or_else(|| "There is no conversation with that person to forward into".to_string())
}

async fn mail_recipients(pool: &Pool<Sqlite>, conversation_id: &str, provider: &Provider) -> Result<Vec<String>, String> {
    let me = provider.namespace.clone().unwrap_or_default().to_lowercase();
    let rows: Vec<Option<String>> = sqlx::query_scalar("SELECT sender_email FROM message WHERE conversation_id = ?1 AND sender_email IS NOT NULL ORDER BY timestamp DESC LIMIT 20")
        .bind(conversation_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to read who the conversation is with: {}", e))?;
    Ok(rows.into_iter().flatten().find(|email| email.to_lowercase() != me).into_iter().collect())
}

pub async fn forward(pool: &Pool<Sqlite>, app_data: &Path, input: PostMessageForwardInput) -> Result<Vec<Message>, String> {
    if input.message_ids.is_empty() {
        return Err("Pick at least one message to forward".to_string());
    }
    if input.targets.is_empty() {
        return Err("Pick at least one chat or person to forward to".to_string());
    }
    let mut picked = Vec::new();
    for id in &input.message_ids {
        picked.push(quoted(pool, id).await?);
    }
    picked.sort_by_key(|q| q.reference.timestamp);
    let comment = input.comment.clone().unwrap_or_default().trim().to_string();
    let mut out = Vec::new();
    for target in &input.targets {
        let conversation_id = target_conversation(pool, target).await?;
        let connected = connect(pool, &conversation_id, target.provider_id.as_deref()).await?;
        let provider = &connected.provider;
        let kind: Option<String> = sqlx::query_scalar("SELECT type FROM conversation WHERE id = ?1").bind(&conversation_id).fetch_optional(pool).await.map_err(|e| e.to_string())?;
        let is_mail = kind.as_deref() == Some("email");
        let sources: Vec<&str> = picked.iter().map(|q| q.provider_id.as_str()).collect();
        let delivery = forward_delivery(&sources, provider, is_mail);
        let mut draft = OutgoingMessage { content_type: "markdown".to_string(), ..Default::default() };
        match delivery {
            Delivery::Native => {
                draft.content = comment.clone();
                draft.forwarded = picked.iter().map(|q| OutgoingReference { native: true, ..q.reference.clone() }).collect();
            }
            Delivery::Mail => {
                let first = &picked[0].reference;
                let base = first.subject.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| excerpt(&first.text, 60));
                draft.subject = Some(crate::providers::traits::prefixed_subject("Fwd", &base));
                draft.content_type = "text".to_string();
                draft.content = comment.clone();
                draft.to = if target.to.is_empty() { mail_recipients(pool, &conversation_id, provider).await? } else { target.to.clone() };
                draft.forwarded = picked.iter().map(|q| OutgoingReference { native: q.provider_id == provider.id, ..q.reference.clone() }).collect();
            }
            _ => {
                let blocks: Vec<String> = picked.iter().map(|q| attribution(&q.reference)).collect();
                draft.content = if comment.is_empty() { blocks.join("\n") } else { format!("{}\n\n{}", comment, blocks.join("\n")) };
            }
        }
        if delivery != Delivery::Native {
            for quoted in &picked {
                draft.attachments.extend(originals(pool, app_data, quoted, provider, is_mail).await);
            }
        }
        if draft.content_type == "markdown" {
            draft.content = mention_targets(pool, &provider.id, &draft.content).await?;
        }
        let original = crate::services::emoji_service::put_draft(pool, provider, &connected.route.conversation_external_id, &mut draft).await?;
        let mut sent = connected.client.send_message(&connected.auth, &connected.route.conversation_external_id, &draft).await?;
        crate::services::emoji_service::put_sent(&mut sent, &draft.content, original);
        sent.metadata.insert("forwarded_from".to_string(), serde_json::json!(picked.iter().map(|q| q.message_id.clone()).collect::<Vec<_>>()));
        sent.metadata.insert("forward_delivery".to_string(), serde_json::to_value(delivery).unwrap_or(Value::Null));
        out.push(store(pool, provider, &conversation_id, &sent).await?);
    }
    Ok(out)
}

pub async fn reply(pool: &Pool<Sqlite>, app_data: &Path, input: PostMessageReplyInput) -> Result<Message, String> {
    let quoted = quoted(pool, &input.message_id).await?;
    let conversation_id = quoted.conversation_id.clone().ok_or_else(|| "The message belongs to no conversation".to_string())?;
    let mut metadata = serde_json::json!({ "in_reply_to": quoted.message_id });
    if quoted.is_mail {
        let provider_id = input.provider_id.clone().unwrap_or_else(|| quoted.provider_id.clone());
        let me = Provider::load_from_db(pool, &provider_id).await?.namespace.unwrap_or_default().to_lowercase();
        let to: Vec<String> = quoted.reference.sender_email.clone().into_iter().collect();
        let cc: Vec<String> = if input.reply_all { quoted.reference.to.iter().filter(|a| a.to_lowercase() != me && !to.contains(a)).cloned().collect() } else { Vec::new() };
        metadata["to"] = serde_json::json!(to);
        metadata["cc"] = serde_json::json!(cc);
        metadata["subject"] = Value::String(crate::providers::traits::prefixed_subject("Re", quoted.reference.subject.as_deref().unwrap_or_default()));
    }
    send(pool, app_data, SendMessageInput {
        conversation_id,
        content: input.content,
        content_type: input.content_type.or(Some("markdown".to_string())),
        provider_id: input.provider_id.or(Some(quoted.provider_id)),
        metadata: Some(metadata),
        attachments: input.attachments,
    })
    .await
}

pub const HOLD_EVENT: &str = "outbound-hold-updated";
pub const OPEN_EVENT: &str = "outbound-hold-open";
pub const QUIT_SEND: &str = "send";
pub const SURFACE_ACTION: &str = "action";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "input", rename_all = "snake_case")]
pub enum Outbound {
    Send(SendMessageInput),
    Reply(PostMessageReplyInput),
    Forward(PostMessageForwardInput),
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HoldRequest {
    pub surface: String,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub draft: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Sent {
    Message(Message),
    Held(OutboundHold),
}

static OPENED: Mutex<Option<String>> = Mutex::new(None);

pub fn notice(template: &str, subject: &str, recipient: &str, seconds: u64) -> String {
    let subject = subject.trim();
    let recipient = recipient.trim();
    let named = if subject.is_empty() { recipient } else { subject };
    let addressed = if recipient.is_empty() { subject } else { recipient };
    template.replace("{subject}", named).replace("{recipient}", addressed).replace("{seconds}", &seconds.to_string())
}

pub async fn signature_for(pool: &Pool<Sqlite>, provider: &Provider, conversation_id: &str, draft: &OutgoingMessage, metadata: &Value) -> Result<Option<content_service::RenderedSignature>, String> {
    use crate::models::content::{SIGNATURE_FORWARD, SIGNATURE_NEW, SIGNATURE_REPLY};
    let profile_id = Profile::get_active_id(pool).await?;
    let alias = draft.from.clone().or_else(|| provider.namespace.clone()).unwrap_or_default().to_lowercase();
    if let Some(chosen) = metadata["signature_id"].as_str().filter(|id| !id.is_empty()) {
        return content_service::render_signature(pool, &profile_id, chosen, &provider.id, &alias, Some(conversation_id)).await.map(Some);
    }
    let earlier: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM message WHERE conversation_id = ?1 AND deleted_datetime IS NULL")
        .bind(conversation_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to read the conversation: {}", e))?;
    let mode = if !draft.forwarded.is_empty() {
        SIGNATURE_FORWARD
    } else if draft.quote.is_some() || earlier {
        SIGNATURE_REPLY
    } else {
        SIGNATURE_NEW
    };
    content_service::get_signature(pool, &profile_id, &provider.id, &alias, mode, Some(conversation_id)).await
}

async fn conversation_kind(pool: &Pool<Sqlite>, conversation_id: &str) -> Result<Option<String>, String> {
    sqlx::query_scalar("SELECT type FROM conversation WHERE id = ?1")
        .bind(conversation_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to read the conversation: {}", e))
}

async fn mail_target(pool: &Pool<Sqlite>, target: &ForwardTarget) -> Result<bool, String> {
    if !target.to.is_empty() {
        return Ok(true);
    }
    let conversation_id = target_conversation(pool, target).await?;
    Ok(conversation_kind(pool, &conversation_id).await?.as_deref() == Some("email"))
}

pub async fn is_mail(pool: &Pool<Sqlite>, outbound: &Outbound) -> Result<bool, String> {
    match outbound {
        Outbound::Send(input) => {
            let metadata = input.metadata.clone().unwrap_or(Value::Null);
            Ok(input.content_type.as_deref() == Some("email") || !strings(metadata.get("to")).is_empty() || conversation_kind(pool, &input.conversation_id).await?.as_deref() == Some("email"))
        }
        Outbound::Reply(input) => Ok(quoted(pool, &input.message_id).await?.is_mail),
        Outbound::Forward(input) => {
            for target in &input.targets {
                if mail_target(pool, target).await? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

async fn label(pool: &Pool<Sqlite>, outbound: &Outbound) -> Result<(String, String, String), String> {
    match outbound {
        Outbound::Send(input) => {
            let metadata = input.metadata.clone().unwrap_or(Value::Null);
            let to = strings(metadata.get("to")).into_iter().next();
            let recipient = match to {
                Some(to) => to,
                None => crate::models::Conversation::get_name(pool, &input.conversation_id).await?.unwrap_or_default(),
            };
            Ok((input.conversation_id.clone(), metadata["subject"].as_str().unwrap_or_default().to_string(), recipient))
        }
        Outbound::Reply(input) => {
            let quoted = quoted(pool, &input.message_id).await?;
            let subject = crate::providers::traits::prefixed_subject("Re", quoted.reference.subject.as_deref().unwrap_or_default());
            Ok((quoted.conversation_id.unwrap_or_default(), subject, quoted.reference.sender_email.unwrap_or(quoted.reference.sender_name)))
        }
        Outbound::Forward(input) => {
            let first = input.targets.first().ok_or_else(|| "Pick at least one chat or person to forward to".to_string())?;
            let conversation_id = target_conversation(pool, first).await?;
            let subject = match input.message_ids.first() {
                Some(id) => quoted(pool, id).await?.reference.subject.map(|s| crate::providers::traits::prefixed_subject("Fwd", &s)).unwrap_or_default(),
                None => String::new(),
            };
            let recipient = match first.to.first() {
                Some(to) => to.clone(),
                None => crate::models::Conversation::get_name(pool, &conversation_id).await?.unwrap_or_default(),
            };
            Ok((conversation_id, subject, recipient))
        }
    }
}

pub async fn post_outbound(pool: &Pool<Sqlite>, app_data: &Path, outbound: Outbound) -> Result<Vec<Message>, String> {
    match outbound {
        Outbound::Send(input) => Ok(vec![send(pool, app_data, input).await?]),
        Outbound::Reply(input) => Ok(vec![reply(pool, app_data, input).await?]),
        Outbound::Forward(input) => forward(pool, app_data, input).await,
    }
}

async fn now_sent(pool: &Pool<Sqlite>, app_data: &Path, outbound: Outbound) -> Result<Vec<Sent>, String> {
    Ok(post_outbound(pool, app_data, outbound).await?.into_iter().map(Sent::Message).collect())
}

async fn post_hold(pool: &Pool<Sqlite>, outbound: Outbound, request: &HoldRequest, due_at: DateTime<Utc>) -> Result<OutboundHold, String> {
    let (origin, subject, recipient) = label(pool, &outbound).await?;
    let conversation_id = request.conversation_id.clone().filter(|id| !id.is_empty()).unwrap_or(origin);
    let payload = serde_json::to_value(&outbound).map_err(|e| format!("The email could not be held: {}", e))?;
    OutboundHold::post(pool, &PostOutboundHoldInput { surface: request.surface.clone(), conversation_id, subject, recipient, payload, draft: request.draft.clone(), due_at }).await
}

pub async fn put_outbound(pool: &Pool<Sqlite>, app_data: &Path, outbound: Outbound, request: Option<HoldRequest>, now: DateTime<Utc>) -> Result<Vec<Sent>, String> {
    let Some(request) = request else { return now_sent(pool, app_data, outbound).await };
    let seconds = AppSetting::get_u64(pool, app_setting::MAIL_SEND_DELAY_SECONDS).await?;
    if seconds == 0 {
        return now_sent(pool, app_data, outbound).await;
    }
    let due_at = now + chrono::Duration::seconds(seconds as i64);
    if let Outbound::Forward(input) = &outbound {
        let mut mail = Vec::new();
        let mut chat = Vec::new();
        for target in &input.targets {
            if mail_target(pool, target).await? {
                mail.push(target.clone());
            } else {
                chat.push(target.clone());
            }
        }
        let mut out = Vec::new();
        if !chat.is_empty() {
            out.extend(now_sent(pool, app_data, Outbound::Forward(PostMessageForwardInput { targets: chat, ..input.clone() })).await?);
        }
        if !mail.is_empty() {
            out.push(Sent::Held(post_hold(pool, Outbound::Forward(PostMessageForwardInput { targets: mail, ..input.clone() }), &request, due_at).await?));
        }
        return Ok(out);
    }
    if !is_mail(pool, &outbound).await? {
        return now_sent(pool, app_data, outbound).await;
    }
    Ok(vec![Sent::Held(post_hold(pool, outbound, &request, due_at).await?)])
}

pub async fn delays_actions(pool: &Pool<Sqlite>) -> bool {
    AppSetting::get_value(pool, app_setting::MAIL_DELAY_ACTIONS).await.is_ok_and(|v| v == "on" || v == "true")
}

pub async fn post_hold_send(pool: &Pool<Sqlite>, app_data: &Path, id: &str) -> Result<Vec<Message>, String> {
    if !OutboundHold::put_status(pool, id, &[outbound_hold::STATUS_PENDING], outbound_hold::STATUS_SENDING, None, None).await? {
        return Err("This email is no longer waiting to be sent".to_string());
    }
    let held = OutboundHold::get(pool, id).await?;
    let outbound: Outbound = serde_json::from_value(held.payload).map_err(|e| format!("The held email cannot be read: {}", e))?;
    match post_outbound(pool, app_data, outbound).await {
        Ok(messages) => {
            OutboundHold::put_status(pool, id, &[outbound_hold::STATUS_SENDING], outbound_hold::STATUS_SENT, None, messages.first().map(|m| m.id.as_str())).await?;
            Ok(messages)
        }
        Err(e) => {
            OutboundHold::put_status(pool, id, &[outbound_hold::STATUS_SENDING], outbound_hold::STATUS_FAILED, Some(&e), None).await?;
            Err(e)
        }
    }
}

pub async fn delete_hold(pool: &Pool<Sqlite>, id: &str) -> Result<OutboundHold, String> {
    if OutboundHold::put_status(pool, id, &[outbound_hold::STATUS_PENDING, outbound_hold::STATUS_DRAFT, outbound_hold::STATUS_FAILED], outbound_hold::STATUS_CANCELLED, None, None).await? {
        return OutboundHold::get(pool, id).await;
    }
    let held = OutboundHold::get(pool, id).await?;
    Err(match held.status.as_str() {
        outbound_hold::STATUS_CANCELLED => "This email was already stopped".to_string(),
        _ => "This email has already been sent".to_string(),
    })
}

pub async fn put_quit(pool: &Pool<Sqlite>, app_data: &Path, policy: &str) -> Result<Vec<OutboundHold>, String> {
    let mut out = Vec::new();
    for held in OutboundHold::get_holds(pool, &[outbound_hold::STATUS_PENDING], None).await? {
        if policy == QUIT_SEND {
            if let Err(e) = post_hold_send(pool, app_data, &held.id).await {
                eprintln!("Held email {} was not sent on quit: {}", held.id, e);
            }
        } else {
            OutboundHold::put_status(pool, &held.id, &[outbound_hold::STATUS_PENDING], outbound_hold::STATUS_DRAFT, None, None).await?;
        }
        out.push(OutboundHold::get(pool, &held.id).await?);
    }
    Ok(out)
}

pub async fn get_resumed(pool: &Pool<Sqlite>, app_data: &Path, now: DateTime<Utc>) -> Result<Vec<OutboundHold>, String> {
    for held in OutboundHold::get_holds(pool, &[outbound_hold::STATUS_SENDING], None).await? {
        OutboundHold::put_status(pool, &held.id, &[outbound_hold::STATUS_SENDING], outbound_hold::STATUS_FAILED, Some("Chatnip stopped while this email was being sent"), None).await?;
    }
    let policy = AppSetting::get_value(pool, app_setting::MAIL_QUIT_POLICY).await?;
    let mut out = Vec::new();
    for held in OutboundHold::get_due(pool, now).await? {
        if policy == QUIT_SEND {
            if let Err(e) = post_hold_send(pool, app_data, &held.id).await {
                eprintln!("Held email {} was not sent after the restart: {}", held.id, e);
            }
        } else {
            OutboundHold::put_status(pool, &held.id, &[outbound_hold::STATUS_PENDING], outbound_hold::STATUS_DRAFT, None, None).await?;
        }
        out.push(OutboundHold::get(pool, &held.id).await?);
    }
    out.extend(OutboundHold::get_holds(pool, &[outbound_hold::STATUS_PENDING], None).await?);
    Ok(out)
}

fn pool_of(app: &AppHandle) -> Option<Pool<Sqlite>> {
    app.try_state::<crate::ChatnipService>().map(|service| service.db().clone())
}

fn title(app: &AppHandle) -> String {
    app.config().product_name.clone().unwrap_or_else(|| app.package_info().name.clone())
}

async fn refresh(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main").map(|w| w.as_ref().window()) else { return };
    let service = app.state::<crate::ChatnipService>();
    if let Ok(messages) = crate::commands::get_current_messages(service.clone(), window.clone()).await {
        window.emit("messages-updated", &messages).ok();
    }
    if let Ok(conversations) = crate::commands::get_conversations(service).await {
        window.emit("conversations-updated", &conversations).ok();
    }
}

async fn draft_notice(app: &AppHandle, pool: &Pool<Sqlite>, held: &OutboundHold) {
    if held.status != outbound_hold::STATUS_DRAFT || !notification_service::allowed().await {
        return;
    }
    let template = AppSetting::get_value(pool, app_setting::MAIL_DRAFT_NOTICE).await.unwrap_or_default();
    notification_service::put(&held.id, &title(app), &notice(&template, &held.subject, &held.recipient, 0));
}

pub fn countdown(app: AppHandle, id: String) {
    tauri::async_runtime::spawn(async move {
        let Some(pool) = pool_of(&app) else { return };
        let template = AppSetting::get_value(&pool, app_setting::MAIL_SEND_NOTICE).await.unwrap_or_default();
        let shown = notification_service::allowed().await;
        let live = notification_service::updates_in_place();
        let mut first = true;
        loop {
            let Ok(held) = OutboundHold::get(&pool, &id).await else { return };
            if held.status != outbound_hold::STATUS_PENDING {
                if held.status != outbound_hold::STATUS_DRAFT {
                    notification_service::delete(&id);
                }
                app.emit(HOLD_EVENT, &held).ok();
                return;
            }
            let now = Utc::now();
            let left = held.seconds_left(now);
            if left == 0 {
                break;
            }
            if shown && (first || live) {
                let heading = if live { title(&app) } else { format!("{} · {}", title(&app), held.due().with_timezone(&chrono::Local).format("%H:%M:%S")) };
                notification_service::put(&id, &heading, &notice(&template, &held.subject, &held.recipient, left));
            }
            first = false;
            let wait = (held.due() - now).num_milliseconds().clamp(50, 1000) as u64;
            tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
        }
        notification_service::delete(&id);
        let Ok(app_data) = app.path().app_data_dir() else { return };
        match post_hold_send(&pool, &app_data, &id).await {
            Ok(messages) => {
                for message in messages {
                    crate::services::action_service::on_sent(app.clone(), message.id);
                }
            }
            Err(e) => {
                eprintln!("Held email {} was not sent: {}", id, e);
                if let Ok(held) = OutboundHold::get(&pool, &id).await {
                    crate::services::history_service::record_failed_hold(&pool, Some(&app), &id, &held.conversation_id, &held.subject, &e).await.ok();
                }
            }
        }
        if let Ok(held) = OutboundHold::get(&pool, &id).await {
            app.emit(HOLD_EVENT, &held).ok();
        }
        refresh(&app).await;
    });
}

pub fn started(app: &AppHandle, sent: &[Sent]) {
    for item in sent {
        match item {
            Sent::Message(message) => crate::services::action_service::on_sent(app.clone(), message.id.clone()),
            Sent::Held(held) => {
                app.emit(HOLD_EVENT, held).ok();
                countdown(app.clone(), held.id.clone());
            }
        }
    }
}

pub fn opened(app: AppHandle, id: String) {
    tauri::async_runtime::spawn(async move {
        let mut pool = pool_of(&app);
        for _ in 0..100 {
            if pool.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            pool = pool_of(&app);
        }
        let Some(pool) = pool else { return };
        if let Ok(held) = delete_hold(&pool, &id).await {
            app.emit(HOLD_EVENT, &held).ok();
        }
        notification_service::delete(&id);
        if let Ok(mut opened) = OPENED.lock() {
            *opened = Some(id.clone());
        }
        if let Some(window) = app.get_webview_window("main") {
            window.unminimize().ok();
            window.show().ok();
            window.set_focus().ok();
        }
        app.emit(OPEN_EVENT, &id).ok();
    });
}

pub async fn get_opened(pool: &Pool<Sqlite>) -> Result<Option<OutboundHold>, String> {
    let id = OPENED.lock().map_err(|_| "locked".to_string())?.take();
    match id {
        Some(id) => Ok(Some(OutboundHold::get(pool, &id).await?)),
        None => Ok(None),
    }
}

pub fn spawn_resume(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Some(pool) = pool_of(&app) else { return };
        let Ok(app_data) = app.path().app_data_dir() else { return };
        match get_resumed(&pool, &app_data, Utc::now()).await {
            Ok(holds) => {
                for held in holds {
                    match held.status.as_str() {
                        outbound_hold::STATUS_PENDING => countdown(app.clone(), held.id.clone()),
                        _ => draft_notice(&app, &pool, &held).await,
                    }
                }
            }
            Err(e) => eprintln!("Held emails were not resumed: {}", e),
        }
    });
}

pub fn quit(app: &AppHandle) {
    let Some(pool) = pool_of(app) else { return };
    let Ok(app_data) = app.path().app_data_dir() else { return };
    let app = app.clone();
    tauri::async_runtime::block_on(async move {
        let policy = AppSetting::get_value(&pool, app_setting::MAIL_QUIT_POLICY).await.unwrap_or_default();
        match put_quit(&pool, &app_data, &policy).await {
            Ok(holds) => {
                for held in holds {
                    if held.status == outbound_hold::STATUS_DRAFT {
                        draft_notice(&app, &pool, &held).await;
                    } else {
                        notification_service::delete(&held.id);
                    }
                }
            }
            Err(e) => eprintln!("Held emails were not settled on quit: {}", e),
        }
    });
}

pub async fn quote_of(pool: &Pool<Sqlite>, message_id: &str) -> Result<Option<Message>, String> {
    let row = sqlx::query("SELECT reply_to_id, provider_id, conversation_id, thread_id FROM message WHERE id = ?1")
        .bind(message_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to load the message: {}", e))?
        .ok_or_else(|| "Message not found".to_string())?;
    let Some(reply_to) = row.try_get::<Option<String>, _>("reply_to_id").ok().flatten().filter(|r| !r.is_empty()) else { return Ok(None) };
    let provider_id: String = row.get("provider_id");
    let found: Option<String> = sqlx::query_scalar(
        "SELECT id FROM message WHERE deleted_datetime IS NULL AND id != ?3 AND (id = ?1 OR (provider_id = ?2 AND (external_id = ?1 OR (CASE WHEN json_valid(metadata) THEN json_extract(metadata, '$.internet_message_id') END) = ?1))) LIMIT 1",
    )
    .bind(&reply_to)
    .bind(&provider_id)
    .bind(message_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to find the replied message: {}", e))?;
    let Some(id) = found else { return Ok(None) };
    let service = MessageService::new(pool.clone());
    Ok(Some(service.load_message(&id, MessageLoadOptions::default()).await.map_err(|e| format!("Failed to load the replied message: {}", e))?.message))
}

async fn store(pool: &Pool<Sqlite>, provider: &Provider, conversation_id: &str, sent: &ResponseMessage) -> Result<Message, String> {
    let attachments_json = serde_json::to_string(
        &sent
            .attachments
            .iter()
            .map(|a| serde_json::json!({ "id": a.external_id, "name": a.name, "content_type": a.content_type, "size": a.size, "is_inline": false, "content_id": null }))
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());

    let profile = Profile::get_active(pool).await?.ok_or_else(|| "No active profile found".to_string())?;
    let account = crate::models::Account::get(pool, &provider.id).await?;
    let sender_id = account.as_ref().map(|a| a.user_id.clone()).or(profile.user_id.clone()).unwrap_or_else(|| "current_user".to_string());
    let sender_name = sent.sender.display_name.clone().or(sent.sender.email.clone()).or(account.as_ref().and_then(|a| a.name())).unwrap_or_else(|| profile.name.clone());
    Provider::put_last_used(pool, &provider.id).await?;
    let message_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let recipients = serde_json::to_string(&sent.recipients.iter().map(|p| serde_json::json!({ "name": p.display_name, "email": p.email })).collect::<Vec<_>>()).unwrap_or_else(|_| "[]".to_string());
    let cc = serde_json::to_string(&sent.cc.iter().map(|p| serde_json::json!({ "name": p.display_name, "email": p.email })).collect::<Vec<_>>()).unwrap_or_else(|_| "[]".to_string());
    let metadata = (!sent.metadata.is_empty()).then(|| serde_json::to_string(&sent.metadata).unwrap_or_else(|_| "{}".to_string()));
    sqlx::query(
        r#"INSERT INTO message (
            id, external_id, provider_id, type,
            subject, content, content_type, conversation_id,
            sender_id, sender_name, sender_email, recipients, cc_recipients,
            thread_id, reply_to_id,
            importance, is_read, is_draft, has_attachments, attachments,
            created_datetime, last_modified_datetime, sent_datetime,
            timestamp, cached_at, created_at, updated_at, provider_name, metadata
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'normal', 1, 0, ?18, ?19, ?16, ?16, ?16, ?16, ?16, ?16, ?16, ?17, ?20)"#,
    )
    .bind(&message_id)
    .bind(&sent.external_id)
    .bind(&provider.id)
    .bind(format!("{:?}", sent.r#type).to_lowercase())
    .bind(&sent.subject)
    .bind(&sent.content)
    .bind(&sent.content_type)
    .bind(conversation_id)
    .bind(&sender_id)
    .bind(&sender_name)
    .bind(sent.sender.email.clone().or(account.as_ref().and_then(|a| a.email.clone())))
    .bind(recipients)
    .bind(cc)
    .bind(&sent.thread_id)
    .bind(&sent.reply_to_id)
    .bind(&now)
    .bind(&provider.name)
    .bind(!sent.attachments.is_empty())
    .bind(attachments_json)
    .bind(metadata)
    .execute(pool)
    .await
    .map_err(|e| format!("Sent, but the copy could not be stored: {}", e))?;
    sqlx::query("UPDATE conversation SET last_id = ?1, last_activity = ?2, updated_at = ?2 WHERE id = ?3")
        .bind(&message_id)
        .bind(&now)
        .bind(conversation_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update the conversation: {}", e))?;

    let service = MessageService::new(pool.clone());
    let loaded = service
        .load_message(&message_id, MessageLoadOptions::default())
        .await
        .map_err(|e| format!("Failed to reload the sent message: {}", e))?;
    Ok(loaded.message)
}

pub struct Reacted {
    pub reaction: String,
    pub active: bool,
}

pub fn put_reactions(metadata: &mut Value, reaction: &str, me: &[String], active: bool) {
    if !metadata.is_object() {
        *metadata = serde_json::json!({});
    }
    let mut list: Vec<Value> = metadata["reactions"].as_array().cloned().unwrap_or_default();
    list.retain(|r| !(r["emoji"].as_str().map(crate::content::emoji::canonical_reaction).as_deref() == Some(reaction) && r["user"].as_str().is_some_and(|u| me.iter().any(|m| m == u))));
    if active {
        list.push(serde_json::json!({ "emoji": reaction, "user": me.first().cloned().unwrap_or_default(), "at": Utc::now().to_rfc3339() }));
    }
    metadata["reactions"] = Value::Array(list);
}

pub async fn react(pool: &Pool<Sqlite>, message_id: &str, reaction: &str) -> Result<Reacted, String> {
    let (route, external_id) = route_for_message(pool, message_id).await?;
    let mut provider = route.provider;
    let reaction = crate::services::emoji_service::reaction(pool, &provider, reaction).await?;
    let profile = Profile::get_active(pool).await?.ok_or_else(|| "No active profile found".to_string())?;
    let account = crate::models::Account::get(pool, &provider.id).await?.map(|a| a.user_id).or(profile.user_id.clone());
    let mut me: Vec<String> = sqlx::query_scalar("SELECT external_id FROM provider_user WHERE provider_id = ?1 AND user_id = ?2")
        .bind(&provider.id)
        .bind(account.clone().unwrap_or_default())
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to find you on {}: {}", provider.name, e))?;
    me.extend(account);
    me.extend(profile.user_id.clone());
    me.push("current_user".to_string());
    let raw: Option<String> = sqlx::query_scalar("SELECT metadata FROM message WHERE id = ?1").bind(message_id).fetch_optional(pool).await.map_err(|e| format!("Failed to get message: {}", e))?.flatten();
    let mut metadata: Value = raw.as_deref().and_then(|m| serde_json::from_str(m).ok()).unwrap_or_else(|| serde_json::json!({}));
    let mine = metadata["reactions"].as_array().is_some_and(|list| list.iter().any(|r| r["emoji"].as_str().map(crate::content::emoji::canonical_reaction).as_deref() == Some(reaction.as_str()) && r["user"].as_str().is_some_and(|u| me.iter().any(|m| m == u))));
    let active = !mine;
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    provider_impl.send_reaction(&auth, &route.conversation_external_id, &external_id, &reaction, active).await?;
    Provider::put_last_used(pool, &provider.id).await?;
    put_reactions(&mut metadata, &reaction, &me, active);
    sqlx::query("UPDATE message SET metadata = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(serde_json::to_string(&metadata).map_err(|e| e.to_string())?)
        .bind(Utc::now().to_rfc3339())
        .bind(message_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update message: {}", e))?;
    Ok(Reacted { reaction, active })
}

pub async fn mark_read(pool: &Pool<Sqlite>, message_ids: &[String], read: bool) -> Result<(), String> {
    let mut picked: Vec<(String, String, String, String)> = Vec::new();
    for id in message_ids {
        let Some(row) = sqlx::query("SELECT provider_id, conversation_id, type, timestamp FROM message WHERE id = ?1").bind(id).fetch_optional(pool).await.map_err(|e| format!("Failed to load the message: {}", e))? else { continue };
        let provider_id: String = row.get("provider_id");
        let Some(conversation_id) = row.try_get::<Option<String>, _>("conversation_id").ok().flatten() else { continue };
        let kind: String = row.try_get("type").unwrap_or_default();
        let timestamp: String = row.get("timestamp");
        if kind == "email" {
            picked.push((provider_id, conversation_id, id.clone(), timestamp));
            continue;
        }
        let same = picked.iter().position(|(p, c, _, _)| *p == provider_id && *c == conversation_id);
        match same {
            Some(at) if (read && timestamp > picked[at].3) || (!read && timestamp < picked[at].3) => picked[at] = (provider_id, conversation_id, id.clone(), timestamp),
            Some(_) => {}
            None => picked.push((provider_id, conversation_id, id.clone(), timestamp)),
        }
    }
    let mut clients: HashMap<String, (Arc<dyn ProviderTrait>, crate::providers::models::ProviderAuth, String)> = HashMap::new();
    for (provider_id, conversation_id, message_id, _) in picked {
        if !clients.contains_key(&provider_id) {
            let Ok(route) = route(pool, &conversation_id, Some(&provider_id)).await else { continue };
            let mut provider = route.provider;
            let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
            let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
            clients.insert(provider_id.clone(), (provider_impl, auth, provider.name.clone()));
        }
        let Some((provider_impl, auth, name)) = clients.get(&provider_id) else { continue };
        let Ok((message_route, external_id)) = route_for_message(pool, &message_id).await else { continue };
        if let Err(e) = provider_impl.mark_read(auth, &message_route.conversation_external_id, &external_id, read).await {
            eprintln!("Read state for {} not sent to {}: {}", message_id, name, e);
        }
    }
    Ok(())
}

pub async fn edit(pool: &Pool<Sqlite>, message_id: &str, content: &str) -> Result<(), String> {
    let (route, external_id) = route_for_message(pool, message_id).await?;
    let mut provider = route.provider;
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    provider_impl.edit_message(&auth, &route.conversation_external_id, &external_id, content).await?;
    Provider::put_last_used(pool, &provider.id).await
}

pub async fn put_member(pool: &Pool<Sqlite>, conversation_id: &str, user_id: &str, member: bool) -> Result<bool, String> {
    let Ok(route) = route(pool, conversation_id, None).await else { return Ok(false) };
    let external: Option<String> = sqlx::query_scalar("SELECT external_id FROM provider_user WHERE provider_id = ?1 AND user_id = ?2 LIMIT 1")
        .bind(&route.provider.id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to find the person on {}: {}", route.provider.name, e))?;
    let Some(external) = external else { return Ok(false) };
    let mut provider = route.provider;
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    provider_impl.put_member(&auth, &route.conversation_external_id, &external, member).await
}

pub async fn put_name(pool: &Pool<Sqlite>, conversation_id: &str, name: &str) -> Result<bool, String> {
    let Ok(route) = route(pool, conversation_id, None).await else { return Ok(false) };
    let mut provider = route.provider;
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    provider_impl.put_conversation_name(&auth, &route.conversation_external_id, name).await
}

pub async fn put_pin(pool: &Pool<Sqlite>, message_id: &str, pinned: bool) -> Result<bool, String> {
    let Ok((route, external_id)) = route_for_message(pool, message_id).await else { return Ok(false) };
    let mut provider = route.provider;
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    provider_impl.put_pin(&auth, &route.conversation_external_id, &external_id, pinned).await
}

pub async fn put_flag(pool: &Pool<Sqlite>, message_id: &str, flagged: bool) -> Result<bool, String> {
    let Ok((route, external_id)) = route_for_message(pool, message_id).await else { return Ok(false) };
    if !route.provider.capabilities.supports_flags {
        return Ok(false);
    }
    let mut provider = route.provider;
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    provider_impl.put_flag(&auth, &route.conversation_external_id, &external_id, flagged).await
}

pub async fn put_archive(pool: &Pool<Sqlite>, conversation_id: &str, archived: bool) -> Result<usize, String> {
    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM message WHERE conversation_id = ?1 AND deleted_datetime IS NULL AND type = 'email'")
        .bind(conversation_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list the mail to archive: {}", e))?;
    let mut clients: HashMap<String, (Arc<dyn ProviderTrait>, crate::providers::models::ProviderAuth)> = HashMap::new();
    let mut moved = 0;
    for id in ids {
        let Ok((route, external_id)) = route_for_message(pool, &id).await else { continue };
        if route.provider.capabilities.archive != Delivery::Native {
            continue;
        }
        if !clients.contains_key(&route.provider.id) {
            let mut provider = route.provider.clone();
            let client = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
            let auth = ensure_fresh(pool, &mut provider, client.clone()).await?;
            clients.insert(provider.id.clone(), (client, auth));
        }
        let Some((client, auth)) = clients.get(&route.provider.id) else { continue };
        match client.put_archive(auth, &route.conversation_external_id, &external_id, archived).await {
            Ok(true) => moved += 1,
            Ok(false) => {}
            Err(e) => eprintln!("{} was not archived on {}: {}", id, route.provider.name, e),
        }
    }
    Ok(moved)
}

pub async fn put_membership(pool: &Pool<Sqlite>, conversation_id: &str, joined: bool) -> Result<bool, String> {
    let Ok(route) = route(pool, conversation_id, None).await else { return Ok(false) };
    let mut provider = route.provider;
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    provider_impl.put_membership(&auth, &route.conversation_external_id, joined).await
}

pub async fn put_conversation_state(pool: &Pool<Sqlite>, conversation_id: &str, archived: bool) -> Result<bool, String> {
    let Ok(route) = route(pool, conversation_id, None).await else { return Ok(false) };
    if !route.provider.capabilities.supports_channel_archive {
        return Ok(false);
    }
    let mut provider = route.provider;
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    provider_impl.put_conversation_state(&auth, &route.conversation_external_id, archived).await
}

pub async fn put_topic(pool: &Pool<Sqlite>, conversation_id: &str, topic: Option<&str>, purpose: Option<&str>) -> Result<bool, String> {
    let Ok(route) = route(pool, conversation_id, None).await else { return Ok(false) };
    let topic = topic.filter(|_| route.provider.capabilities.supports_topic);
    let purpose = purpose.filter(|_| route.provider.capabilities.supports_purpose);
    if topic.is_none() && purpose.is_none() {
        return Ok(false);
    }
    let mut provider = route.provider;
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    provider_impl.put_conversation_topic(&auth, &route.conversation_external_id, topic, purpose).await
}

pub async fn client_of(pool: &Pool<Sqlite>, provider_id: &str) -> Result<(Provider, Arc<dyn ProviderTrait>, crate::providers::models::ProviderAuth), String> {
    let mut provider = Provider::load_from_db(pool, provider_id).await?;
    let client = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, client.clone()).await?;
    Ok((provider, client, auth))
}

pub async fn put_remote_draft(pool: &Pool<Sqlite>, draft: &crate::models::draft::Draft) -> Result<Option<String>, String> {
    let Ok(route) = route(pool, &draft.conversation_id, draft.metadata["provider_id"].as_str()).await else { return Ok(None) };
    if route.provider.capabilities.drafts != Delivery::Native {
        return Ok(None);
    }
    let mut provider = route.provider;
    let client = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, client.clone()).await?;
    let outgoing = OutgoingMessage {
        content: draft.markdown.clone(),
        content_type: "markdown".to_string(),
        subject: draft.metadata["subject"].as_str().map(String::from),
        to: strings(draft.metadata.get("to")),
        cc: strings(draft.metadata.get("cc")),
        bcc: strings(draft.metadata.get("bcc")),
        from: draft.metadata["from"].as_str().map(String::from),
        ..Default::default()
    };
    client.put_remote_draft(&auth, &route.conversation_external_id, &outgoing, draft.remote_id.as_deref()).await
}

pub async fn delete_remote_draft(pool: &Pool<Sqlite>, draft: &crate::models::draft::Draft) -> Result<bool, String> {
    let Some(remote) = draft.remote_id.as_deref() else { return Ok(false) };
    let Ok(route) = route(pool, &draft.conversation_id, draft.metadata["provider_id"].as_str()).await else { return Ok(false) };
    let mut provider = route.provider;
    let client = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, client.clone()).await?;
    client.delete_remote_draft(&auth, remote).await
}

pub async fn get_presences(pool: &Pool<Sqlite>, user_ids: &[String]) -> Result<Vec<crate::providers::models::PresenceUpdate>, String> {
    let mut wanted: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for user_id in user_ids {
        let rows = sqlx::query("SELECT pu.provider_id, pu.external_id, p.capabilities FROM provider_user pu JOIN provider p ON p.id = pu.provider_id WHERE pu.user_id = ?1 AND p.is_enabled = 1")
            .bind(user_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to find where {} is present: {}", user_id, e))?;
        for row in rows {
            let capabilities: crate::providers::models::ProviderCapabilities = row.try_get::<String, _>("capabilities").ok().and_then(|c| serde_json::from_str(&c).ok()).unwrap_or_default();
            if capabilities.supports_presence {
                wanted.entry(row.get("provider_id")).or_default().push((row.get("external_id"), user_id.clone()));
            }
        }
    }
    let mut out: Vec<crate::providers::models::PresenceUpdate> = Vec::new();
    for (provider_id, people) in wanted {
        let Ok((_, client, auth)) = client_of(pool, &provider_id).await else { continue };
        let externals: Vec<String> = people.iter().map(|(external, _)| external.clone()).collect();
        let Ok(found) = client.get_presences(&auth, &externals).await else { continue };
        for (external, status) in found {
            let Some((_, user_id)) = people.iter().find(|(e, _)| *e == external) else { continue };
            let status = crate::services::people_service::presence_status(&status);
            match out.iter_mut().find(|p| p.user_id == *user_id) {
                Some(existing) if existing.status == "offline" => existing.status = status,
                Some(_) => {}
                None => out.push(crate::providers::models::PresenceUpdate { user_id: user_id.clone(), status, status_text: None, status_emoji: None }),
            }
        }
    }
    Ok(out)
}

pub async fn delete(pool: &Pool<Sqlite>, message_id: &str) -> Result<(), String> {
    let (route, external_id) = route_for_message(pool, message_id).await?;
    let mut provider = route.provider;
    let provider_impl = provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = ensure_fresh(pool, &mut provider, provider_impl.clone()).await?;
    provider_impl.delete_message(&auth, &route.conversation_external_id, &external_id).await?;
    Provider::put_last_used(pool, &provider.id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::traits::ProviderTrait;

    #[test]
    fn recipient_lists_accept_strings_and_objects() {
        let value = serde_json::json!(["a@x", { "email": "b@x" }, { "address": "c@x" }, 7]);
        assert_eq!(strings(Some(&value)), vec!["a@x", "b@x", "c@x"]);
        assert!(strings(None).is_empty());
    }

    fn reference() -> OutgoingReference {
        OutgoingReference {
            external_id: "m1".into(),
            conversation_name: Some("Deploy".into()),
            sender_name: "Nora Gale".into(),
            sender_email: Some("nora.gale@northwind.example".into()),
            text: "Staging is green.\nShip it?".into(),
            subject: Some("Release".into()),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-10-08T09:40:00Z").unwrap().with_timezone(&Utc),
            to: vec!["milo@northwind.example".into()],
            attachments: vec!["release-notes.pdf".into()],
            ..Default::default()
        }
    }

    fn provider(id: &str, forward: Delivery, reply_quote: Delivery) -> Provider {
        let mut provider = crate::providers::mock::MockKind::Teams.provider();
        provider.id = id.into();
        provider.capabilities.forward = forward;
        provider.capabilities.reply_quote = reply_quote;
        provider
    }

    #[test]
    fn reposts_carry_the_author_the_chat_the_text_and_the_attachment_names() {
        let text = attribution(&reference());
        assert_eq!(text, "**Forwarded from Nora Gale in Deploy** · 2026-10-08 09:40\n> Staging is green.\n> Ship it?\n\nAttachments: release-notes.pdf\n");
        assert_eq!(quote_block(&reference()), "> **Nora Gale**: Staging is green. Ship it?\n\n");
        let mail = reference().forward_block();
        assert!(mail.starts_with("---------- Forwarded message ---------\nFrom: Nora Gale <nora.gale@northwind.example>\nDate: Thu, 8 Oct 2026 09:40:00 +0000\nSubject: Release\nTo: milo@northwind.example\nAttachments: release-notes.pdf\n\nStaging is green."));
        assert_eq!(excerpt(&"word ".repeat(100), 12), "word word wo…");
        let draft = OutgoingMessage { content: "FYI".into(), content_type: "text".into(), html: Some("<p>FYI</p>".into()), forwarded: vec![reference()], ..Default::default() };
        assert!(draft.mail_text().starts_with("FYI\n\n---------- Forwarded message ---------\nFrom: Nora Gale"));
        assert!(draft.mail_html().is_none());
        let html = OutgoingMessage { content: "<p>FYI</p>".into(), content_type: "html".into(), forwarded: vec![reference()], ..Default::default() };
        assert!(html.mail_text().starts_with("<p>FYI</p><br><br>---------- Forwarded message ---------<br>From: Nora Gale &lt;nora.gale@northwind.example&gt;"));
        assert_eq!(OutgoingMessage { content: "Hei".into(), html: Some("<p>Hei</p>".into()), ..Default::default() }.mail_html().as_deref(), Some("<p>Hei</p>"));
    }

    #[test]
    fn forwards_go_native_only_inside_one_connection_and_mail_always_by_mail() {
        let slack = provider("slack-a", Delivery::Native, Delivery::Native);
        assert_eq!(forward_delivery(&["slack-a", "slack-a"], &slack, false), Delivery::Native);
        assert_eq!(forward_delivery(&["slack-a", "teams"], &slack, false), Delivery::Repost);
        assert_eq!(forward_delivery(&["teams"], &slack, true), Delivery::Mail);
        let teams = provider("teams", Delivery::Repost, Delivery::Native);
        assert_eq!(forward_delivery(&["teams"], &teams, false), Delivery::Repost);
        assert_eq!(reply_delivery("teams", false, &teams), Delivery::Native);
        assert_eq!(reply_delivery("slack-a", false, &teams), Delivery::Repost);
        let old = provider("old", Delivery::Unsupported, Delivery::Unsupported);
        assert_eq!(reply_delivery("old", true, &old), Delivery::Mail);
        assert_eq!(reply_delivery("old", false, &old), Delivery::Repost);
    }

    async fn mock_connection(pool: &Pool<Sqlite>, profile: &str, kind: crate::providers::mock::MockKind) -> String {
        use crate::providers::mock::MockProvider;
        let id = crate::test_helpers::test_provider(pool, profile, "mock").await;
        let client = MockProvider::new(kind, pool.clone());
        let capabilities = serde_json::to_string(&client.get_provider().capabilities).unwrap();
        sqlx::query("UPDATE provider SET external_id = ?1, namespace = ?2, config = ?2, name = ?3, capabilities = ?4, access_token = 'mock', expires_at = ?5 WHERE id = ?6")
            .bind(format!("mock-{}-me", kind.key()))
            .bind(kind.key())
            .bind(kind.label())
            .bind(capabilities)
            .bind((Utc::now() + chrono::Duration::days(365)).to_rfc3339())
            .bind(&id)
            .execute(pool)
            .await
            .unwrap();
        let provider = Provider::load_from_db(pool, &id).await.unwrap();
        provider.ingest(pool, None, std::sync::Arc::new(MockProvider::new(kind, pool.clone()))).await.unwrap();
        id
    }

    async fn latest_in(pool: &Pool<Sqlite>, provider: &str, kind: &str) -> (String, String) {
        sqlx::query_as("SELECT m.id, m.conversation_id FROM message m JOIN conversation c ON c.id = m.conversation_id WHERE m.provider_id = ?1 AND c.type = ?2 AND m.content IS NOT NULL AND m.content != '' ORDER BY m.timestamp DESC LIMIT 1")
            .bind(provider)
            .bind(kind)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    fn metadata(message: &Message) -> Value {
        message.metadata.as_deref().and_then(|m| serde_json::from_str(m).ok()).unwrap_or(Value::Null)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_mock_replies_with_quotes_and_forwards_natively_by_repost_and_by_mail() {
        use crate::providers::mock::MockKind;
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        crate::models::AppSetting::put(&pool, crate::models::app_setting::MOCK_ENABLED, "true").await.unwrap();
        let teams = mock_connection(&pool, &profile, MockKind::Teams).await;
        let slack = mock_connection(&pool, &profile, MockKind::Slack).await;
        let gmail = mock_connection(&pool, &profile, MockKind::Gmail).await;
        let app_data = std::env::temp_dir();

        let (teams_message, teams_chat) = latest_in(&pool, &teams, "direct_message").await;
        let replied = reply(&pool, &app_data, PostMessageReplyInput { message_id: teams_message.clone(), content: "Ja, det ser bra ut".into(), ..Default::default() }).await.unwrap();
        assert_eq!(replied.conversation_id.as_deref(), Some(teams_chat.as_str()));
        let external: String = sqlx::query_scalar("SELECT external_id FROM message WHERE id = ?1").bind(&teams_message).fetch_one(&pool).await.unwrap();
        assert_eq!(replied.reply_to_id.as_deref(), Some(external.as_str()));
        assert_eq!(metadata(&replied)["quote"]["native"], true);
        assert!(replied.thread_id.is_none());
        assert_eq!(quote_of(&pool, &replied.id).await.unwrap().map(|m| m.id), Some(teams_message.clone()));

        let (slack_message, slack_chat) = latest_in(&pool, &slack, "direct_message").await;
        let crossed = send(&pool, &app_data, SendMessageInput { conversation_id: slack_chat.clone(), content: "Sett dette".into(), content_type: Some("markdown".into()), provider_id: None, metadata: Some(serde_json::json!({ "in_reply_to": teams_message })), attachments: None }).await.unwrap();
        assert!(crossed.content.as_deref().unwrap().starts_with("> **"));
        assert_eq!(crossed.reply_to_id.as_deref(), Some(teams_message.as_str()));
        assert_eq!(quote_of(&pool, &crossed.id).await.unwrap().map(|m| m.id), Some(teams_message.clone()));

        let reposted = forward(&pool, &app_data, PostMessageForwardInput { message_ids: vec![teams_message.clone()], targets: vec![ForwardTarget { conversation_id: Some(slack_chat.clone()), ..Default::default() }], comment: Some("Se her".into()) }).await.unwrap();
        let body = reposted[0].content.clone().unwrap();
        assert!(body.starts_with("Se her\n\n**Forwarded from "), "{}", body);
        assert!(body.contains(" in "));
        assert_eq!(metadata(&reposted[0])["forward_delivery"], "repost");
        assert_eq!(metadata(&reposted[0])["forwarded_from"][0], teams_message.as_str());

        let (older, _) = sqlx::query_as::<_, (String, String)>("SELECT id, conversation_id FROM message WHERE provider_id = ?1 AND timestamp < (SELECT timestamp FROM message WHERE id = ?2) AND content IS NOT NULL AND content != '' ORDER BY timestamp DESC LIMIT 1").bind(&slack).bind(&slack_message).fetch_one(&pool).await.unwrap();
        let native = forward(&pool, &app_data, PostMessageForwardInput { message_ids: vec![slack_message.clone(), older.clone()], targets: vec![ForwardTarget { conversation_id: Some(slack_chat.clone()), ..Default::default() }], comment: None }).await.unwrap();
        let meta = metadata(&native[0]);
        assert_eq!(meta["forward_delivery"], "native");
        let forwarded: Vec<bool> = meta["forwarded"].as_array().unwrap().iter().map(|f| f["native"].as_bool().unwrap()).collect();
        assert_eq!(forwarded, vec![true, true]);
        assert_eq!(meta["forwarded_from"][0], older.as_str());

        let (_, mail_chat) = latest_in(&pool, &gmail, "email").await;
        let mailed = forward(&pool, &app_data, PostMessageForwardInput { message_ids: vec![teams_message.clone()], targets: vec![ForwardTarget { conversation_id: Some(mail_chat), to: vec!["anna.berg@client.example".into()], ..Default::default() }], comment: None }).await.unwrap();
        assert!(mailed[0].subject.as_deref().unwrap().starts_with("Fwd: "));
        assert!(mailed[0].content.as_deref().unwrap().contains("---------- Forwarded message ---------"));
        assert_eq!(metadata(&mailed[0])["forward_delivery"], "mail");
        assert_eq!(mailed[0].recipients.iter().map(|r| r.address.clone()).collect::<Vec<_>>(), vec!["anna.berg@client.example".to_string()]);

        assert!(forward(&pool, &app_data, PostMessageForwardInput { message_ids: vec![], targets: vec![], comment: None }).await.unwrap_err().contains("at least one message"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn mentions_resolve_to_the_sending_providers_user_ids() {
        use crate::providers::mock::MockKind;
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        crate::models::AppSetting::put(&pool, crate::models::app_setting::MOCK_ENABLED, "true").await.unwrap();
        let slack = mock_connection(&pool, &profile, MockKind::Slack).await;
        let app_data = std::env::temp_dir();
        let (_, slack_chat) = latest_in(&pool, &slack, "direct_message").await;
        let (user, external): (String, String) = sqlx::query_as("SELECT user_id, external_id FROM provider_user WHERE provider_id = ?1 LIMIT 1").bind(&slack).fetch_one(&pool).await.unwrap();
        let content = format!("Hei [@Kari](mention:{}) og [@Ukjent](mention:nobody) **nå**", user);
        let sent = send(&pool, &app_data, SendMessageInput { conversation_id: slack_chat, content, content_type: Some("markdown".into()), provider_id: None, metadata: None, attachments: None }).await.unwrap();
        let body = sent.content.unwrap();
        assert_eq!(body, format!("Hei [@Kari](mention:{}) og [@Ukjent](mention:) **nå**", external));
        assert_eq!(crate::content::markdown::slack(&body), format!("Hei <@{}> og @Ukjent *nå*", external));
    }

    #[test]
    fn my_reactions_toggle_in_the_array_and_leave_everyone_elses() {
        let mut metadata = serde_json::json!({ "reactions": [{ "emoji": "👍", "user": "U0ME" }, { "emoji": ":+1:", "user": "U0ANNA" }] });
        put_reactions(&mut metadata, ":+1:", &["U0ME".to_string()], false);
        assert_eq!(metadata["reactions"], serde_json::json!([{ "emoji": ":+1:", "user": "U0ANNA" }]));
        put_reactions(&mut metadata, ":tada:", &["U0ME".to_string()], true);
        assert_eq!(metadata["reactions"][1]["emoji"], ":tada:");
        assert_eq!(metadata["reactions"][1]["user"], "U0ME");
        let mut empty = serde_json::Value::Null;
        put_reactions(&mut empty, ":eyes:", &["current_user".to_string()], true);
        assert_eq!(empty["reactions"].as_array().unwrap().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reacting_twice_adds_then_removes_through_the_provider_in_canonical_form() {
        use crate::providers::mock::MockKind;
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        crate::models::AppSetting::put(&pool, crate::models::app_setting::MOCK_ENABLED, "true").await.unwrap();
        let slack = mock_connection(&pool, &profile, MockKind::Slack).await;
        let (message, _) = latest_in(&pool, &slack, "direct_message").await;
        let added = react(&pool, &message, "🎉").await.unwrap();
        assert_eq!((added.reaction.as_str(), added.active), (":tada:", true));
        let stored: String = sqlx::query_scalar("SELECT metadata FROM message WHERE id = ?1").bind(&message).fetch_one(&pool).await.unwrap();
        let stored: Value = serde_json::from_str(&stored).unwrap();
        assert!(stored["reactions"].as_array().unwrap().iter().any(|r| r["emoji"] == ":tada:" && r["user"] == "mock-slack-me"));
        let removed = react(&pool, &message, ":tada:").await.unwrap();
        assert!(!removed.active);
        let stored: String = sqlx::query_scalar("SELECT metadata FROM message WHERE id = ?1").bind(&message).fetch_one(&pool).await.unwrap();
        assert!(!serde_json::from_str::<Value>(&stored).unwrap()["reactions"].as_array().unwrap().iter().any(|r| r["emoji"] == ":tada:" && r["user"] == "mock-slack-me"));
    }

    #[test]
    fn the_notification_text_fills_the_subject_the_recipient_and_the_seconds() {
        let template = app_setting::get_default(app_setting::MAIL_SEND_NOTICE).unwrap();
        assert_eq!(notice(template, "Kontrakt", "anna@client.example", 7), "Sending email Kontrakt in 7");
        assert_eq!(notice(template, "  ", "anna@client.example", 10), "Sending email anna@client.example in 10");
        assert_eq!(notice("{recipient} · {subject} · {seconds}s", "Kontrakt", "", 3), "Kontrakt · Kontrakt · 3s");
        assert_eq!(notice("Sender {subject} til {recipient} om {seconds}", "Tilbud", "ola@x.no", 1), "Sender Tilbud til ola@x.no om 1");
        assert_eq!(notice(app_setting::get_default(app_setting::MAIL_DRAFT_NOTICE).unwrap(), "Tilbud", "ola@x.no", 0), "Kept email Tilbud as a draft");
    }

    fn mail_to(conversation_id: &str, subject: &str) -> Outbound {
        Outbound::Send(SendMessageInput {
            conversation_id: conversation_id.to_string(),
            content: "Hei, her er tilbudet".into(),
            content_type: Some("markdown".into()),
            provider_id: None,
            metadata: Some(serde_json::json!({ "subject": subject, "to": ["anna.berg@client.example"] })),
            attachments: None,
        })
    }

    fn composer() -> Option<HoldRequest> {
        Some(HoldRequest { surface: "composer".into(), conversation_id: None, draft: serde_json::json!({ "markdown": "Hei, her er tilbudet" }) })
    }

    fn held(sent: Vec<Sent>) -> OutboundHold {
        match sent.into_iter().next() {
            Some(Sent::Held(hold)) => hold,
            other => panic!("expected a held email, got {:?}", other),
        }
    }

    async fn stored(pool: &Pool<Sqlite>, conversation_id: &str) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE conversation_id = ?1").bind(conversation_id).fetch_one(pool).await.unwrap()
    }

    async fn mail_and_chat(pool: &Pool<Sqlite>) -> (String, String) {
        use crate::providers::mock::MockKind;
        let profile = crate::test_helpers::test_profile(pool).await;
        crate::models::AppSetting::put(pool, crate::models::app_setting::MOCK_ENABLED, "true").await.unwrap();
        let gmail = mock_connection(pool, &profile, MockKind::Gmail).await;
        let slack = mock_connection(pool, &profile, MockKind::Slack).await;
        let (_, mail) = latest_in(pool, &gmail, "email").await;
        let (_, chat) = latest_in(pool, &slack, "direct_message").await;
        (mail, chat)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn chat_sends_go_at_once_and_mail_sends_are_held() {
        let pool = crate::test_helpers::test_pool().await;
        let (mail, chat) = mail_and_chat(&pool).await;
        let app_data = std::env::temp_dir();
        let chat_input = Outbound::Send(SendMessageInput { conversation_id: chat.clone(), content: "Straks".into(), content_type: Some("markdown".into()), provider_id: None, metadata: None, attachments: None });
        assert!(!is_mail(&pool, &chat_input).await.unwrap());
        let before = stored(&pool, &chat).await;
        let sent = put_outbound(&pool, &app_data, chat_input, composer(), Utc::now()).await.unwrap();
        assert!(matches!(sent.as_slice(), [Sent::Message(_)]));
        assert_eq!(stored(&pool, &chat).await, before + 1);

        assert!(is_mail(&pool, &mail_to(&mail, "Tilbud")).await.unwrap());
        let before = stored(&pool, &mail).await;
        let now = Utc::now();
        let hold = held(put_outbound(&pool, &app_data, mail_to(&mail, "Tilbud"), composer(), now).await.unwrap());
        assert_eq!(hold.status, outbound_hold::STATUS_PENDING);
        assert_eq!((hold.subject.as_str(), hold.recipient.as_str(), hold.surface.as_str(), hold.conversation_id.as_str()), ("Tilbud", "anna.berg@client.example", "composer", mail.as_str()));
        assert_eq!(hold.seconds_left(now), 10);
        assert_eq!(hold.draft["markdown"], "Hei, her er tilbudet");
        assert_eq!(stored(&pool, &mail).await, before);

        let unheld = put_outbound(&pool, &app_data, mail_to(&mail, "Uten vent"), None, Utc::now()).await.unwrap();
        assert!(matches!(unheld.as_slice(), [Sent::Message(_)]));
        AppSetting::put(&pool, app_setting::MAIL_SEND_DELAY_SECONDS, "0").await.unwrap();
        let immediate = put_outbound(&pool, &app_data, mail_to(&mail, "Null"), composer(), Utc::now()).await.unwrap();
        assert!(matches!(immediate.as_slice(), [Sent::Message(_)]));
        assert_eq!(stored(&pool, &mail).await, before + 2);

        assert!(!delays_actions(&pool).await);
        AppSetting::put(&pool, app_setting::MAIL_DELAY_ACTIONS, "on").await.unwrap();
        assert!(delays_actions(&pool).await);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_send_speaks_as_the_sending_account_and_marks_it_last_used() {
        let pool = crate::test_helpers::test_pool().await;
        let (mail, chat) = mail_and_chat(&pool).await;
        let slack: String = sqlx::query_scalar("SELECT provider_id FROM conversation_provider WHERE conversation_id = ?1").bind(&chat).fetch_one(&pool).await.unwrap();
        let gmail: String = sqlx::query_scalar("SELECT provider_id FROM conversation_provider WHERE conversation_id = ?1").bind(&mail).fetch_one(&pool).await.unwrap();
        assert!(Provider::load_from_db(&pool, &slack).await.unwrap().last_used_at.is_none());
        let account = crate::models::Account::get(&pool, &slack).await.unwrap().expect("the mock account is known");
        let input = SendMessageInput { conversation_id: chat.clone(), content: "Straks".into(), content_type: Some("markdown".into()), provider_id: None, metadata: None, attachments: None };
        let message = send(&pool, &std::env::temp_dir(), input).await.unwrap();
        assert_eq!(message.sender_id.as_deref(), Some(account.user_id.as_str()));
        assert!(message.sender_name.is_some_and(|name| !name.trim().is_empty()));
        let used = Provider::load_from_db(&pool, &slack).await.unwrap().last_used_at.expect("sending marks the account");
        assert!(Utc::now().signed_duration_since(used).num_seconds() < 60);
        assert!(Provider::load_from_db(&pool, &gmail).await.unwrap().last_used_at.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_held_mail_is_stopped_by_id_or_sent_once_at_its_deadline() {
        let pool = crate::test_helpers::test_pool().await;
        let (mail, _) = mail_and_chat(&pool).await;
        let app_data = std::env::temp_dir();
        let now = Utc::now();
        let stopped = held(put_outbound(&pool, &app_data, mail_to(&mail, "Angres"), composer(), now).await.unwrap());
        let kept = held(put_outbound(&pool, &app_data, mail_to(&mail, "Sendes"), composer(), now).await.unwrap());
        let before = stored(&pool, &mail).await;

        assert!(OutboundHold::get_due(&pool, now + chrono::Duration::seconds(9)).await.unwrap().is_empty());
        let cancelled = delete_hold(&pool, &stopped.id).await.unwrap();
        assert_eq!(cancelled.status, outbound_hold::STATUS_CANCELLED);
        assert_eq!(cancelled.draft["markdown"], "Hei, her er tilbudet");
        assert!(delete_hold(&pool, &stopped.id).await.unwrap_err().contains("already stopped"));
        assert!(post_hold_send(&pool, &app_data, &stopped.id).await.is_err());

        let due = OutboundHold::get_due(&pool, now + chrono::Duration::seconds(10)).await.unwrap();
        assert_eq!(due.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), vec![kept.id.as_str()]);
        let messages = post_hold_send(&pool, &app_data, &kept.id).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].subject.as_deref(), Some("Sendes"));
        let sent = OutboundHold::get(&pool, &kept.id).await.unwrap();
        assert_eq!((sent.status.as_str(), sent.message_id.as_deref()), (outbound_hold::STATUS_SENT, Some(messages[0].id.as_str())));
        assert!(post_hold_send(&pool, &app_data, &kept.id).await.is_err());
        assert!(delete_hold(&pool, &kept.id).await.unwrap_err().contains("already been sent"));
        assert_eq!(stored(&pool, &mail).await, before + 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn quitting_keeps_held_mail_as_a_draft_or_sends_it_by_the_setting() {
        let pool = crate::test_helpers::test_pool().await;
        let (mail, _) = mail_and_chat(&pool).await;
        let app_data = std::env::temp_dir();
        let draft = held(put_outbound(&pool, &app_data, mail_to(&mail, "Utkast"), composer(), Utc::now()).await.unwrap());
        assert_eq!(AppSetting::get_value(&pool, app_setting::MAIL_QUIT_POLICY).await.unwrap(), "draft");
        let settled = put_quit(&pool, &app_data, "draft").await.unwrap();
        assert_eq!(settled.iter().map(|h| (h.id.as_str(), h.status.as_str())).collect::<Vec<_>>(), vec![(draft.id.as_str(), outbound_hold::STATUS_DRAFT)]);
        assert!(OutboundHold::get_due(&pool, Utc::now() + chrono::Duration::minutes(5)).await.unwrap().is_empty());
        assert_eq!(delete_hold(&pool, &draft.id).await.unwrap().status, outbound_hold::STATUS_CANCELLED);

        let before = stored(&pool, &mail).await;
        let gone = held(put_outbound(&pool, &app_data, mail_to(&mail, "Straks"), composer(), Utc::now()).await.unwrap());
        let settled = put_quit(&pool, &app_data, QUIT_SEND).await.unwrap();
        assert_eq!(settled[0].id, gone.id);
        assert_eq!(settled[0].status, outbound_hold::STATUS_SENT);
        assert_eq!(stored(&pool, &mail).await, before + 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn held_mail_survives_a_restart_and_resumes_its_countdown() {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("chatnip.db").display());
        let open = || async {
            let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect(&url).await.unwrap();
            crate::database::init_database(&pool).await.unwrap();
            pool
        };
        let app_data = std::env::temp_dir();
        let now = Utc::now();
        let pool = open().await;
        let (mail, _) = mail_and_chat(&pool).await;
        let waiting = held(put_outbound(&pool, &app_data, mail_to(&mail, "Etter omstart"), composer(), now).await.unwrap());
        let overdue = held(put_outbound(&pool, &app_data, mail_to(&mail, "Forfalt"), composer(), now - chrono::Duration::minutes(5)).await.unwrap());
        pool.close().await;

        let pool = open().await;
        let resumed = get_resumed(&pool, &app_data, now).await.unwrap();
        let status = |id: &str| resumed.iter().find(|h| h.id == id).map(|h| h.status.clone());
        assert_eq!(status(&waiting.id).as_deref(), Some(outbound_hold::STATUS_PENDING));
        assert_eq!(status(&overdue.id).as_deref(), Some(outbound_hold::STATUS_DRAFT));
        assert_eq!(OutboundHold::get(&pool, &waiting.id).await.unwrap().payload, waiting.payload);

        AppSetting::put(&pool, app_setting::MAIL_QUIT_POLICY, QUIT_SEND).await.unwrap();
        let resumed = get_resumed(&pool, &app_data, now + chrono::Duration::seconds(30)).await.unwrap();
        assert_eq!(resumed.iter().find(|h| h.id == waiting.id).map(|h| h.status.as_str()), Some(outbound_hold::STATUS_SENT));
        pool.close().await;
    }
}
