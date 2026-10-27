use crate::models::{Conversation, Message, Profile};
use crate::providers::models::PresenceUpdate;
use crate::providers::traits::{NewRemoteConversation, RemoteSearch, RemoteUserGroup, ResponseConversation};
use crate::services::outbound_service;
use crate::ChatnipService;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::{AppHandle, Emitter, Manager, State};

const ACTIVITY_LIMIT: u32 = 50;

#[derive(Debug, Clone, Serialize)]
pub struct JoinableConversation {
    pub provider_id: String,
    pub provider_name: String,
    pub conversation: ResponseConversation,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostConversationMembershipInput {
    pub provider_id: Option<String>,
    pub conversation_id: Option<String>,
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PutConversationTopicInput {
    pub conversation_id: String,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub purpose: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostProviderConversationInput {
    pub provider_id: String,
    pub conversation: NewRemoteConversation,
    #[serde(default)]
    pub member_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityItem {
    pub kind: String,
    pub at: String,
    pub actor: Option<String>,
    pub detail: Option<String>,
    pub conversation_id: Option<String>,
    pub conversation_name: Option<String>,
    pub message: Message,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityPage {
    pub items: Vec<ActivityItem>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetProviderMessagesInput {
    pub query: String,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[tauri::command]
pub async fn get_joinable_conversations(service: State<'_, ChatnipService>, provider_id: Option<String>) -> Result<Vec<JoinableConversation>, String> {
    let pool = service.db();
    let profile_id = Profile::get_active_id(pool).await?;
    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM provider WHERE profile_id = ?1 AND is_enabled = 1 AND (?2 IS NULL OR id = ?2)")
        .bind(&profile_id)
        .bind(&provider_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list the connections: {}", e))?;
    let mut out = Vec::new();
    for id in ids {
        let Ok((provider, client, auth)) = outbound_service::client_of(pool, &id).await else { continue };
        if !provider.capabilities.supports_channel_join {
            continue;
        }
        match client.get_joinable_conversations(&auth).await {
            Ok(found) => out.extend(found.into_iter().map(|conversation| JoinableConversation { provider_id: provider.id.clone(), provider_name: provider.name.clone(), conversation })),
            Err(e) => eprintln!("Channels on {} were not listed: {}", provider.name, e),
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn post_conversation_membership(app: AppHandle, service: State<'_, ChatnipService>, input: PostConversationMembershipInput) -> Result<bool, String> {
    let pool = service.db();
    let joined = match (&input.conversation_id, &input.provider_id, &input.external_id) {
        (Some(conversation_id), _, _) => outbound_service::put_membership(pool, conversation_id, true).await?,
        (None, Some(provider_id), Some(external_id)) => {
            let (provider, client, auth) = outbound_service::client_of(pool, provider_id).await?;
            if !provider.capabilities.supports_channel_join {
                return Err(format!("{} cannot join channels", provider.name));
            }
            client.put_membership(&auth, external_id, true).await?
        }
        _ => return Err("Name the conversation, or the connection and the channel to join".to_string()),
    };
    if let Some(provider_id) = input.provider_id.clone().or(match &input.conversation_id {
        Some(id) => outbound_service::route(pool, id, None).await.ok().map(|r| r.provider.id),
        None => None,
    }) {
        let pool = pool.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::services::connection_service::sync_provider(&app, &pool, &provider_id).await {
                eprintln!("Syncing after joining failed: {}", e);
            }
        });
    }
    Ok(joined)
}

#[tauri::command]
pub async fn delete_conversation_membership(app: AppHandle, service: State<'_, ChatnipService>, conversation_id: String) -> Result<bool, String> {
    let pool = service.db();
    let left = outbound_service::put_membership(pool, &conversation_id, false).await?;
    if left {
        sqlx::query("UPDATE conversation SET is_archived = 1, updated_at = ?1 WHERE id = ?2")
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(&conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to put the conversation away: {}", e))?;
        app.emit("conversation-changed", &conversation_id).ok();
    }
    Ok(left)
}

#[tauri::command]
pub async fn put_conversation_state(app: AppHandle, service: State<'_, ChatnipService>, conversation_id: String, archived: bool) -> Result<bool, String> {
    let changed = outbound_service::put_conversation_state(service.db(), &conversation_id, archived).await?;
    if changed {
        app.emit("conversation-changed", &conversation_id).ok();
    }
    Ok(changed)
}

#[tauri::command]
pub async fn put_conversation_topic(app: AppHandle, service: State<'_, ChatnipService>, input: PutConversationTopicInput) -> Result<bool, String> {
    let pool = service.db();
    let written = outbound_service::put_topic(pool, &input.conversation_id, input.topic.as_deref(), input.purpose.as_deref()).await?;
    let mut fields: Vec<(&str, &str)> = Vec::new();
    if let Some(topic) = input.topic.as_deref() {
        fields.push(("$.topic", topic));
    }
    if let Some(purpose) = input.purpose.as_deref() {
        fields.push(("$.purpose", purpose));
    }
    for (path, value) in fields {
        sqlx::query("UPDATE conversation SET metadata = json_set(CASE WHEN json_valid(metadata) THEN metadata ELSE '{}' END, ?1, ?2), updated_at = ?3 WHERE id = ?4")
            .bind(path)
            .bind(value.trim())
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(&input.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to keep the topic: {}", e))?;
    }
    app.emit("conversation-changed", &input.conversation_id).ok();
    Ok(written)
}

#[tauri::command]
pub async fn post_provider_conversation(app: AppHandle, service: State<'_, ChatnipService>, input: PostProviderConversationInput) -> Result<Option<Conversation>, String> {
    let pool = service.db();
    let (provider, client, auth) = outbound_service::client_of(pool, &input.provider_id).await?;
    if !provider.capabilities.can_create_conversations && !provider.capabilities.supports_channel_create {
        return Err(format!("{} cannot start conversations", provider.name));
    }
    let mut conversation = input.conversation.clone();
    for user_id in &input.member_ids {
        let external: Option<String> = sqlx::query_scalar("SELECT external_id FROM provider_user WHERE provider_id = ?1 AND user_id = ?2 LIMIT 1")
            .bind(&provider.id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to find the person on {}: {}", provider.name, e))?;
        if let Some(external) = external.filter(|e| !conversation.members.contains(e)) {
            conversation.members.push(external);
        }
    }
    let Some(created) = client.post_conversation(&auth, &conversation).await? else { return Ok(None) };
    let window = app.get_webview_window("main").ok_or_else(|| "No main window".to_string())?;
    crate::services::connection_service::sync_provider(&app, pool, &provider.id).await?;
    let local = crate::providers::models::get_conversation_id(pool, &provider.id, &created.external_id).await?;
    let Some(local) = local else { return Ok(None) };
    let found = Conversation::load_with_providers(pool, &local).await?;
    window.emit("conversation-changed", &local).ok();
    Ok(found)
}

#[tauri::command]
pub async fn get_presences(service: State<'_, ChatnipService>, user_ids: Vec<String>) -> Result<Vec<PresenceUpdate>, String> {
    outbound_service::get_presences(service.db(), &user_ids).await
}

#[tauri::command]
pub async fn get_user_groups(service: State<'_, ChatnipService>, provider_id: String) -> Result<Vec<RemoteUserGroup>, String> {
    let (provider, client, auth) = outbound_service::client_of(service.db(), &provider_id).await?;
    if !provider.capabilities.supports_user_groups {
        return Ok(Vec::new());
    }
    client.get_user_groups(&auth).await
}

#[tauri::command]
pub async fn get_activity(service: State<'_, ChatnipService>, before: Option<String>, limit: Option<u32>) -> Result<ActivityPage, String> {
    let pool = service.db();
    let profile_id = Profile::get_active_id(pool).await?;
    let mine = Profile::get_own_user_ids(pool, &profile_id).await?;
    let mut handles: Vec<String> = Vec::new();
    for user_id in &mine {
        let ids: Vec<String> = sqlx::query_scalar("SELECT external_id FROM provider_user WHERE user_id = ?1").bind(user_id).fetch_all(pool).await.map_err(|e| format!("Failed to read your accounts: {}", e))?;
        handles.extend(ids);
    }
    let mine_json = serde_json::to_string(&mine).unwrap_or_else(|_| "[]".to_string());
    let handles_json = serde_json::to_string(&handles).unwrap_or_else(|_| "[]".to_string());
    let limit = limit.unwrap_or(ACTIVITY_LIMIT).clamp(1, 200) as i64;
    let rows = sqlx::query(
        r#"SELECT m.*, c.name AS conversation_name, 'mention' AS activity, m.sender_name AS actor, NULL AS detail, m.timestamp AS at
             FROM message m JOIN provider p ON p.id = m.provider_id LEFT JOIN conversation c ON c.id = m.conversation_id
             WHERE p.profile_id = ?1 AND m.deleted_datetime IS NULL AND m.sender_id NOT IN (SELECT value FROM json_each(?2))
               AND EXISTS (SELECT 1 FROM json_each(json_extract(CASE WHEN json_valid(m.metadata) THEN m.metadata ELSE '{}' END, '$.structure.mentions')) x
                           WHERE json_extract(x.value, '$.id') IN (SELECT value FROM json_each(?3)) OR json_extract(x.value, '$.id') IN ('here', 'channel', 'everyone'))
               AND (?4 IS NULL OR m.timestamp < ?4)
           UNION ALL
           SELECT m.*, c.name AS conversation_name, 'reply' AS activity, m.sender_name AS actor, NULL AS detail, m.timestamp AS at
             FROM message m JOIN provider p ON p.id = m.provider_id LEFT JOIN conversation c ON c.id = m.conversation_id
             JOIN message parent ON parent.provider_id = m.provider_id AND (parent.external_id = m.reply_to_id OR parent.id = m.reply_to_id)
             WHERE p.profile_id = ?1 AND m.deleted_datetime IS NULL AND m.sender_id NOT IN (SELECT value FROM json_each(?2))
               AND parent.sender_id IN (SELECT value FROM json_each(?2))
               AND (?4 IS NULL OR m.timestamp < ?4)
           UNION ALL
           SELECT m.*, c.name AS conversation_name, 'reaction' AS activity, json_extract(r.value, '$.name') AS actor, json_extract(r.value, '$.emoji') AS detail, COALESCE(m.updated_at, m.timestamp) AS at
             FROM message m JOIN provider p ON p.id = m.provider_id LEFT JOIN conversation c ON c.id = m.conversation_id
             JOIN json_each(json_extract(CASE WHEN json_valid(m.metadata) THEN m.metadata ELSE '{}' END, '$.reactions')) r
             WHERE p.profile_id = ?1 AND m.deleted_datetime IS NULL AND m.sender_id IN (SELECT value FROM json_each(?2))
               AND json_extract(r.value, '$.user') NOT IN (SELECT value FROM json_each(?3))
               AND (?4 IS NULL OR COALESCE(m.updated_at, m.timestamp) < ?4)
           ORDER BY at DESC
           LIMIT ?5"#,
    )
    .bind(&profile_id)
    .bind(&mine_json)
    .bind(&handles_json)
    .bind(&before)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to read the activity: {}", e))?;
    let messages = crate::services::MessageService::new(pool.clone());
    let items: Vec<ActivityItem> = rows
        .into_iter()
        .map(|row| ActivityItem {
            kind: row.get("activity"),
            at: row.get("at"),
            actor: row.try_get("actor").ok().flatten(),
            detail: row.try_get("detail").ok().flatten(),
            conversation_id: row.try_get("conversation_id").ok().flatten(),
            conversation_name: row.try_get("conversation_name").ok().flatten(),
            message: messages.row_to_message(row),
        })
        .collect();
    let cursor = (items.len() as i64 == limit).then(|| items.last().map(|i| i.at.clone())).flatten();
    Ok(ActivityPage { items, cursor })
}

#[tauri::command]
pub async fn get_provider_messages(service: State<'_, ChatnipService>, input: GetProviderMessagesInput) -> Result<Vec<Message>, String> {
    let pool = service.db();
    let profile_id = Profile::get_active_id(pool).await?;
    let filter = crate::services::message_service::SearchFilter::parse(&input.query);
    let routes: Vec<(String, Option<String>)> = match &input.conversation_id {
        Some(conversation_id) => sqlx::query("SELECT provider_id, external_id FROM conversation_provider WHERE conversation_id = ?1")
            .bind(conversation_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to find where the conversation lives: {}", e))?
            .into_iter()
            .map(|row| (row.get::<String, _>("provider_id"), Some(row.get::<String, _>("external_id"))))
            .collect(),
        None => sqlx::query_scalar::<_, String>("SELECT id FROM provider WHERE profile_id = ?1 AND is_enabled = 1 AND (?2 IS NULL OR id = ?2)")
            .bind(&profile_id)
            .bind(&input.provider_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list the connections: {}", e))?
            .into_iter()
            .map(|id| (id, None))
            .collect(),
    };
    let parse_day = |day: &Option<String>| day.as_deref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).and_then(|d| d.and_hms_opt(0, 0, 0)).map(|d| d.and_utc());
    let mut out = Vec::new();
    for (provider_id, external_id) in routes {
        let Ok((provider, client, auth)) = outbound_service::client_of(pool, &provider_id).await else { continue };
        if !provider.capabilities.supports_search {
            continue;
        }
        let search = RemoteSearch { text: filter.text.clone(), from: filter.from.first().cloned(), conversation_external_id: external_id, before: parse_day(&filter.before), after: parse_day(&filter.after), limit: input.limit.unwrap_or(25) };
        match client.get_search(&auth, &search).await {
            Ok(found) => {
                for message in found {
                    let local: Option<String> = sqlx::query_scalar("SELECT id FROM message WHERE provider_id = ?1 AND external_id = ?2").bind(&provider.id).bind(&message.external_id).fetch_optional(pool).await.map_err(|e| e.to_string())?;
                    let conversation_id = crate::providers::models::get_conversation_id(pool, &provider.id, &message.conversation_external_id).await?;
                    let mut found = Message::new(local.unwrap_or_else(|| format!("remote:{}:{}", provider.id, message.external_id)), provider.id.clone(), message.external_id.clone(), "chat".to_string(), Some(message.content.clone()), message.timestamp.to_rfc3339());
                    found.content_type = message.content_type.clone();
                    found.subject = message.subject.clone();
                    found.sender_name = message.sender.display_name.clone().or(message.sender.email.clone());
                    found.sender_email = message.sender.email.clone();
                    found.conversation_id = conversation_id;
                    found.provider_name = Some(provider.name.clone());
                    out.push(found);
                }
            }
            Err(e) => eprintln!("{} was not searched: {}", provider.name, e),
        }
    }
    out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(out)
}
