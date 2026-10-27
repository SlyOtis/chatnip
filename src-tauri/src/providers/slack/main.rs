use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use reqwest::Client;
use serde_json::Value;
use crate::providers::traits::{
    ProviderTrait, ConversationType, MessageType,
    ResponseConversation, ResponseMessage, ResponseUser, ConnectionIdentity, ResponseAttachment, OutgoingMessage, SystemEvent, SystemEventKind, SystemPerson, RemoteEmoji,
    LiveEvent, LiveSink, NewRemoteConversation, RemoteChange, RemoteSearch, RemoteUserGroup
};
use crate::providers::models::{Delivery, Provider, ProviderCapabilities, ProviderAuth};
use std::collections::HashMap;
use tokio::sync::Mutex;

pub const AUTHORIZE_URL: &str = "https://slack.com/oauth/v2/authorize";
pub const API: &str = "https://slack.com/api";
pub const USER_SCOPES: [&str; 26] = [
    "channels:history",
    "channels:read",
    "channels:write",
    "groups:history",
    "groups:read",
    "groups:write",
    "im:history",
    "im:read",
    "im:write",
    "mpim:history",
    "mpim:read",
    "mpim:write",
    "users:read",
    "users:read.email",
    "users.profile:write",
    "chat:write",
    "reactions:read",
    "reactions:write",
    "pins:read",
    "pins:write",
    "files:read",
    "files:write",
    "emoji:read",
    "team:read",
    "search:read",
    "usergroups:read",
];
const NON_EXPIRING_DAYS: i64 = 36500;
const PAGE: u32 = 200;

pub fn auth_url(client_id: &str, redirect_uri: &str, state: &str, code_challenge: Option<&str>) -> String {
    let mut url = format!(
        "{}?client_id={}&user_scope={}&redirect_uri={}&state={}",
        AUTHORIZE_URL,
        urlencoding::encode(client_id),
        urlencoding::encode(&USER_SCOPES.join(",")),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(state)
    );
    if let Some(challenge) = code_challenge {
        url.push_str(&format!("&code_challenge={}&code_challenge_method=S256", challenge));
    }
    url
}

pub fn token_from_response(data: &Value) -> Result<ProviderAuth, String> {
    if data["ok"].as_bool() != Some(true) {
        return Err(format!("Slack refused the request: {}", data["error"].as_str().unwrap_or("unknown")));
    }
    let user = if data["authed_user"]["access_token"].is_string() { &data["authed_user"] } else { data };
    let access_token = user["access_token"].as_str().ok_or_else(|| "Slack returned no user token. Add user scopes to the app, not bot scopes.".to_string())?;
    let expires_at = match user["expires_in"].as_i64() {
        Some(seconds) => Utc::now() + Duration::seconds(seconds),
        None => Utc::now() + Duration::days(NON_EXPIRING_DAYS),
    };
    Ok(ProviderAuth {
        access_token: access_token.to_string(),
        refresh_token: user["refresh_token"].as_str().map(String::from),
        expires_at,
        token_type: "Bearer".to_string(),
        scope: user["scope"].as_str().map(String::from),
    })
}

pub fn emoji_from_list(data: &Value) -> Vec<RemoteEmoji> {
    let mut out: Vec<RemoteEmoji> = data["emoji"]
        .as_object()
        .map(|all| {
            all.iter()
                .filter_map(|(name, value)| {
                    let value = value.as_str()?;
                    Some(match value.strip_prefix("alias:") {
                        Some(target) => RemoteEmoji { name: name.clone(), url: None, alias_for: Some(target.to_string()) },
                        None => RemoteEmoji { name: name.clone(), url: Some(value.to_string()), alias_for: None },
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn profile_user(user: &Value) -> ResponseUser {
    let profile = &user["profile"];
    ResponseUser {
        id: user["id"].as_str().unwrap_or("").to_string(),
        email: profile["email"].as_str().map(String::from),
        phone: profile["phone"].as_str().filter(|s| !s.is_empty()).map(String::from),
        display_name: profile["display_name"].as_str().filter(|s| !s.is_empty()).or_else(|| profile["real_name"].as_str()).or_else(|| user["name"].as_str()).map(String::from),
        first_name: profile["first_name"].as_str().filter(|s| !s.is_empty()).map(String::from),
        last_name: profile["last_name"].as_str().filter(|s| !s.is_empty()).map(String::from),
        avatar_url: profile["image_192"].as_str().or_else(|| profile["image_72"].as_str()).map(String::from),
        job_title: profile["title"].as_str().filter(|s| !s.is_empty()).map(String::from),
        department: None,
        company: None,
        office_location: None,
    }
}

pub fn status_profile(text: &str, emoji: Option<&str>, until: Option<DateTime<Utc>>) -> Value {
    let emoji = emoji.map(crate::content::emoji::canonical_reaction).filter(|e| e.starts_with(':')).unwrap_or_default();
    serde_json::json!({ "profile": { "status_text": text.trim(), "status_emoji": emoji, "status_expiration": until.map(|u| u.timestamp()).unwrap_or(0) } })
}

pub fn channel_name(name: &str) -> String {
    let lowered: String = name.trim().to_lowercase().chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    lowered.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-").chars().take(80).collect()
}

pub struct SlackProvider {
    provider: Provider,
    client: Client,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    api: String,
    users: Mutex<HashMap<String, ResponseUser>>,
    app_token: String,
}

impl SlackProvider {
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        let provider = Provider {
            id: "slack".to_string(),
            profile_id: String::new(),
            provider_type: "slack".to_string(),
            name: "Slack".to_string(),
            external_id: None,
            logo_id: None,
            logo: None,
            capabilities: ProviderCapabilities {
                can_send_messages: true,
                can_edit_messages: true,
                can_delete_messages: true,
                can_create_conversations: true,
                can_add_members: true,
                supports_reactions: true,
                supports_threads: true,
                supports_attachments: true,
                supports_presence: true,
                supports_typing_indicators: true,
                supports_read_receipts: true,
                supports_reply_quote: true,
                supports_forward: true,
                reply_quote: Delivery::Native,
                forward: Delivery::Native,
                supports_calendar_responses: false,
                live: true,
                supports_unread_sync: true,
                thread_broadcast: Delivery::Native,
                supports_channel_create: true,
                supports_channel_join: true,
                supports_channel_archive: true,
                supports_topic: true,
                supports_purpose: true,
                supports_search: true,
                supports_user_groups: true,
                ..Default::default()
            },
            is_enabled: true,
            is_connected: false,
            auth: None,
            sync_interval: 300,
            namespace: None,
            delta_token: None,
            last_sync_at: None,
            config: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            theme: None,
            missing_scopes: Vec::new(),
            account: None,
            last_used_at: None,
        };
        Self {
            provider,
            client: Client::new(),
            client_id,
            client_secret,
            redirect_uri,
            api: std::env::var("SLACK_API_ENDPOINT").unwrap_or_else(|_| API.to_string()),
            users: Mutex::new(HashMap::new()),
            app_token: String::new(),
        }
    }

    pub fn with_api(mut self, api: &str) -> Self {
        self.api = api.trim_end_matches('/').to_string();
        self
    }

    pub fn with_app_token(mut self, app_token: &str) -> Self {
        self.app_token = app_token.trim().to_string();
        self
    }

    fn permalink_url(&self, channel: &str, ts: &str) -> String {
        format!("{}/chat.getPermalink?channel={}&message_ts={}", self.api, urlencoding::encode(channel), urlencoding::encode(ts))
    }

    fn quoted_text(text: &str, permalinks: &[String]) -> String {
        let text = text.trim_end();
        match (text.is_empty(), permalinks.is_empty()) {
            (_, true) => text.to_string(),
            (true, false) => permalinks.join("\n"),
            (false, false) => format!("{}\n{}", text, permalinks.join("\n")),
        }
    }

    pub async fn ensure_valid_auth(
        &self,
        pool: &sqlx::Pool<sqlx::Sqlite>,
        provider_id: &str,
    ) -> Result<ProviderAuth, String> {
        let mut provider = Provider::load_from_db(pool, provider_id).await?;
        let auth = provider.auth.as_ref().ok_or_else(|| "No auth tokens found".to_string())?;
        match (&auth.refresh_token, auth.expires_at < Utc::now() + Duration::minutes(5)) {
            (Some(refresh_token), true) => {
                let new_auth = self.refresh_token(refresh_token).await?;
                provider.update_auth(pool, new_auth.clone()).await?;
                Ok(new_auth)
            }
            _ => Ok(auth.clone()),
        }
    }

    pub async fn fetch_current_user(&self, auth: &ProviderAuth) -> Result<ResponseUser, String> {
        let data = self.post(auth, "auth.test", &serde_json::json!({})).await?;
        let user_id = data["user_id"].as_str().unwrap_or("").to_string();
        self.fetch_user_details(auth, &user_id).await
    }

    async fn fetch_user_details(&self, auth: &ProviderAuth, user_id: &str) -> Result<ResponseUser, String> {
        let data = self.call(auth, &format!("{}/users.info?user={}", self.api, user_id)).await?;
        Ok(ResponseUser { id: data["user"]["id"].as_str().unwrap_or(user_id).to_string(), ..profile_user(&data["user"]) })
    }

    fn map_conversation_type(channel_type: &str, is_im: bool, is_mpim: bool) -> ConversationType {
        if is_im {
            ConversationType::DirectMessage
        } else if is_mpim {
            ConversationType::GroupMessage
        } else {
            match channel_type {
                "im" => ConversationType::DirectMessage,
                "mpim" => ConversationType::GroupMessage,
                _ => ConversationType::Channel,
            }
        }
    }

    async fn resolve_im_name(&self, auth: &ProviderAuth, user_id: &str) -> String {
        self.get_user(auth, user_id, None).await.display_name.unwrap_or_else(|| "Direct Message".to_string())
    }

    async fn fetch_conversation_members(&self, auth: &ProviderAuth, channel_id: &str) -> Vec<ResponseUser> {
        let mut members = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut url = format!("{}/conversations.members?channel={}&limit={}", self.api, channel_id, PAGE);
            if let Some(c) = &cursor {
                url.push_str(&format!("&cursor={}", urlencoding::encode(c)));
            }
            let Ok(data) = self.call(auth, &url).await else { break };
            for uid in data["members"].as_array().cloned().unwrap_or_default().iter().filter_map(|m| m.as_str()) {
                members.push(self.get_user(auth, uid, None).await);
            }
            cursor = Self::next_cursor(&data);
            if cursor.is_none() {
                break;
            }
        }
        members
    }

    fn next_cursor(data: &Value) -> Option<String> {
        data["response_metadata"]["next_cursor"].as_str().filter(|c| !c.is_empty()).map(String::from)
    }

    fn pinned(data: &Value) -> Vec<String> {
        data["items"].as_array().cloned().unwrap_or_default().iter().filter_map(|item| item["message"]["ts"].as_str().map(String::from)).collect()
    }
}

#[async_trait]
impl ProviderTrait for SlackProvider {
    fn get_provider(&self) -> &Provider {
        &self.provider
    }

    fn connects_with_token(&self) -> bool {
        true
    }

    async fn get_auth_url(&self, state: &str, code_challenge: Option<&str>) -> Result<String, String> {
        if self.client_id.is_empty() {
            return Err("Add the Slack client id under Settings, Connections, or paste a user token".to_string());
        }
        Ok(auth_url(&self.client_id, &self.redirect_uri, state, code_challenge))
    }

    async fn exchange_code(&self, code: &str, code_verifier: Option<&str>) -> Result<ProviderAuth, String> {
        let mut params = vec![("client_id", self.client_id.as_str()), ("code", code), ("redirect_uri", self.redirect_uri.as_str())];
        if let Some(verifier) = code_verifier {
            params.push(("code_verifier", verifier));
        }
        if !self.client_secret.is_empty() {
            params.push(("client_secret", self.client_secret.as_str()));
        }
        self.token_call(&params).await
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<ProviderAuth, String> {
        let mut params = vec![("client_id", self.client_id.as_str()), ("grant_type", "refresh_token"), ("refresh_token", refresh_token)];
        if !self.client_secret.is_empty() {
            params.push(("client_secret", self.client_secret.as_str()));
        }
        self.token_call(&params).await
    }

    async fn identity(&self, auth: &ProviderAuth) -> Result<ConnectionIdentity, String> {
        let data = self.post(auth, "auth.test", &serde_json::json!({})).await.map_err(|e| format!("Slack refused the token: {}", e))?;
        let team_id = data["team_id"].as_str().unwrap_or("").to_string();
        let team_name = data["team"].as_str().unwrap_or("Slack").to_string();
        let user_id = data["user_id"].as_str().unwrap_or("").to_string();
        let user = self.fetch_user_details(auth, &user_id).await.ok();
        Ok(ConnectionIdentity {
            namespace: team_id.clone(),
            name: team_name,
            external_id: Some(team_id),
            user,
            logo_url: None,
        })
    }

    async fn fetch_logo(&self, auth: &ProviderAuth, _identity: &ConnectionIdentity) -> Result<Option<(Vec<u8>, String)>, String> {
        let data = self.call(auth, &format!("{}/team.info", self.api)).await?;
        let Some(icon_url) = data["team"]["icon"]["image_132"].as_str() else { return Ok(None) };
        let response = crate::providers::http::send(self.client.get(icon_url)).await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("image/png").to_string();
        let bytes = response.bytes().await.map_err(|e| format!("Failed to read the team icon: {}", e))?;
        Ok(Some((bytes.to_vec(), content_type)))
    }

    async fn validate_tokens(&self, auth: &ProviderAuth) -> Result<bool, String> {
        if auth.expires_at < Utc::now() + Duration::minutes(5) {
            return Ok(false);
        }
        Ok(self.post(auth, "auth.test", &serde_json::json!({})).await.is_ok())
    }

    async fn sync_conversations(
        &self,
        auth: &ProviderAuth,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<ResponseConversation>, String> {
        let floor = since.unwrap_or_else(|| Utc::now() - Duration::days(30));
        let team = self.call(auth, &format!("{}/team.info", self.api)).await.ok();
        let domain = team.as_ref().and_then(|t| t["team"]["domain"].as_str()).map(String::from);
        let mut conversations = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut url = format!(
                "{}/conversations.list?types=public_channel,private_channel,mpim,im&exclude_archived=true&limit={}",
                self.api, PAGE
            );
            if let Some(c) = &cursor {
                url.push_str(&format!("&cursor={}", urlencoding::encode(c)));
            }
            let data = self.call(auth, &url).await?;
            for channel in data["channels"].as_array().cloned().unwrap_or_default() {
                let external_id = channel["id"].as_str().unwrap_or("").to_string();
                let is_im = channel["is_im"].as_bool().unwrap_or(false);
                let is_mpim = channel["is_mpim"].as_bool().unwrap_or(false);
                let is_member = channel["is_member"].as_bool().unwrap_or(is_im || is_mpim);
                if !is_member {
                    continue;
                }
                let latest = channel["latest"]["ts"].as_str().map(Self::ts_time);
                let updated = channel["updated"].as_f64().map(|ms| Self::ts_time(&format!("{}", ms / 1000.0)));
                let last_activity = latest.or(updated).unwrap_or(floor);
                if !is_im && latest.is_some_and(|at| at < floor) {
                    continue;
                }
                let name = if is_im {
                    let user_id = channel["user"].as_str().unwrap_or("");
                    self.resolve_im_name(auth, user_id).await
                } else {
                    channel["name"].as_str()
                        .or_else(|| channel["name_normalized"].as_str())
                        .unwrap_or("Unnamed Channel")
                        .to_string()
                };
                let conversation_type = Self::map_conversation_type(channel["channel_type"].as_str().unwrap_or(""), is_im, is_mpim);
                let members = if is_im || is_mpim { self.fetch_conversation_members(auth, &external_id).await } else { Vec::new() };
                let mut metadata = HashMap::new();
                if let Some(domain) = &domain {
                    metadata.insert("web_url".to_string(), Value::String(format!("https://{}.slack.com/archives/{}", domain, external_id)));
                }
                if let Some(topic) = channel["topic"]["value"].as_str().filter(|t| !t.is_empty()) {
                    metadata.insert("topic".to_string(), Value::String(topic.to_string()));
                }
                conversations.push(ResponseConversation {
                    external_id,
                    name,
                    conversation_type,
                    members,
                    last_activity,
                    metadata,
                });
            }
            cursor = Self::next_cursor(&data);
            if cursor.is_none() {
                break;
            }
        }
        Ok(conversations)
    }

    async fn sync_messages(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        since: Option<DateTime<Utc>>,
        limit: Option<u32>,
    ) -> Result<Vec<ResponseMessage>, String> {
        let floor = since.unwrap_or_else(|| Utc::now() - Duration::days(30));
        let cap = limit.unwrap_or(500) as usize;
        let last_read = self.last_read(auth, conversation_external_id).await;
        let mut messages = Vec::new();
        let mut cursor: Option<String> = None;
        'pages: loop {
            let mut url = format!(
                "{}/conversations.history?channel={}&limit={}&oldest={}",
                self.api,
                conversation_external_id,
                PAGE,
                floor.timestamp()
            );
            if let Some(c) = &cursor {
                url.push_str(&format!("&cursor={}", urlencoding::encode(c)));
            }
            let data = self.call(auth, &url).await?;
            for msg in data["messages"].as_array().cloned().unwrap_or_default() {
                if let Some(message) = self.message(auth, &msg, conversation_external_id).await {
                    let has_replies = msg["reply_count"].as_u64().unwrap_or(0) > 0;
                    let ts = message.external_id.clone();
                    messages.push(message);
                    if has_replies {
                        messages.extend(self.replies(auth, conversation_external_id, &ts, floor).await);
                    }
                }
                if messages.len() >= cap {
                    break 'pages;
                }
            }
            cursor = Self::next_cursor(&data);
            if cursor.is_none() || data["has_more"].as_bool() != Some(true) {
                break;
            }
        }
        if let Some(mark) = &last_read {
            for message in messages.iter_mut() {
                message.is_read = Self::ts_time(&message.external_id) <= Self::ts_time(mark);
            }
        }
        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(messages)
    }

    async fn send_message(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        draft: &OutgoingMessage,
    ) -> Result<ResponseMessage, String> {
        let mut permalinks = Vec::new();
        for reference in draft.quote.iter().chain(draft.forwarded.iter()).filter(|r| r.native) {
            let channel = reference.conversation_external_id.as_deref().unwrap_or(conversation_external_id);
            let link = self.call(auth, &self.permalink_url(channel, &reference.external_id)).await?;
            if let Some(permalink) = link["permalink"].as_str() {
                permalinks.push(permalink.to_string());
            }
        }
        let text = Self::quoted_text(&if draft.content_type == "markdown" { crate::content::markdown::slack(&draft.content) } else { draft.content.clone() }, &permalinks);
        let stored_type = if draft.content_type == "markdown" { "markdown" } else { "text" };
        let mut body = serde_json::json!({
            "channel": conversation_external_id,
            "text": text,
        });
        if !permalinks.is_empty() {
            body["unfurl_links"] = Value::Bool(true);
        }
        if let Some(thread) = draft.thread_external_id.as_ref() {
            body["thread_ts"] = Value::String(thread.clone());
            if draft.broadcast {
                body["reply_broadcast"] = Value::Bool(true);
            }
        }
        if !draft.attachments.is_empty() {
            let mut files = Vec::new();
            for attachment in &draft.attachments {
                let ticket = self
                    .call(auth, &format!("{}/files.getUploadURLExternal?filename={}&length={}", self.api, urlencoding::encode(&attachment.name), attachment.bytes.len()))
                    .await?;
                let upload_url = ticket["upload_url"].as_str().ok_or_else(|| "Slack gave no upload URL".to_string())?;
                let file_id = ticket["file_id"].as_str().ok_or_else(|| "Slack gave no file id".to_string())?;
                let response = crate::providers::http::send(self.client.post(upload_url).header("Content-Type", "application/octet-stream").body(attachment.bytes.clone())).await?;
                if !response.status().is_success() {
                    return Err(format!("Slack refused the upload of {}: {}", attachment.name, response.status()));
                }
                files.push(serde_json::json!({ "id": file_id, "title": attachment.name }));
            }
            let mut complete = serde_json::json!({ "files": files, "channel_id": conversation_external_id, "initial_comment": text });
            if let Some(thread) = body["thread_ts"].as_str() {
                complete["thread_ts"] = Value::String(thread.to_string());
            }
            let data = self.post(auth, "files.completeUploadExternal", &complete).await?;
            let shared = &data["files"][0];
            let ts = shared["shares"]["public"][conversation_external_id][0]["ts"]
                .as_str()
                .or(shared["shares"]["private"][conversation_external_id][0]["ts"].as_str())
                .map(String::from)
                .unwrap_or_else(|| format!("file:{}", shared["id"].as_str().unwrap_or("")));
            let sender = self.fetch_current_user(auth).await.unwrap_or_default();
            return Ok(ResponseMessage {
                external_id: ts.clone(),
                conversation_external_id: conversation_external_id.to_string(),
                sender,
                content: draft.content.clone(),
                content_type: stored_type.to_string(),
                timestamp: if ts.starts_with("file:") { Utc::now() } else { Self::ts_time(&ts) },
                r#type: MessageType::Chat,
                thread_id: draft.thread_external_id.clone(),
                reply_to_id: draft.in_reply_to.clone(),
                attachments: data["files"]
                    .as_array()
                    .map(|list| {
                        list.iter()
                            .zip(draft.attachments.iter())
                            .map(|(file, a)| ResponseAttachment {
                                external_id: file["id"].as_str().unwrap_or("").to_string(),
                                name: a.name.clone(),
                                content_type: a.mime_type.clone(),
                                size: a.bytes.len() as u64,
                                url: file["url_private"].as_str().map(String::from),
                                content_id: None,
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                ..Default::default()
            });
        }
        let data = self.post(auth, "chat.postMessage", &body).await?;
        let message = &data["message"];
        let ts = message["ts"].as_str().unwrap_or("").to_string();
        let user_id = message["user"].as_str().unwrap_or("").to_string();
        let sender = self.get_user(auth, &user_id, None).await;
        Ok(ResponseMessage {
            external_id: ts.clone(),
            conversation_external_id: conversation_external_id.to_string(),
            sender,
            content: draft.content.clone(),
            content_type: stored_type.to_string(),
            timestamp: Self::ts_time(&ts),
            r#type: MessageType::Chat,
            thread_id: draft.thread_external_id.clone(),
            reply_to_id: draft.in_reply_to.clone(),
            ..Default::default()
        })
    }

    async fn send_reaction(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        message_external_id: &str,
        reaction: &str,
        active: bool,
    ) -> Result<(), String> {
        let name = crate::content::emoji::slack_reaction(reaction);
        let body = serde_json::json!({ "channel": conversation_external_id, "timestamp": message_external_id, "name": name });
        let method = if active { "reactions.add" } else { "reactions.remove" };
        match self.post(auth, method, &body).await {
            Ok(_) => Ok(()),
            Err(e) if e.ends_with("already_reacted") || e.ends_with("no_reaction") => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn delete_message(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        message_external_id: &str,
    ) -> Result<(), String> {
        let body = serde_json::json!({ "channel": conversation_external_id, "ts": message_external_id });
        self.post(auth, "chat.delete", &body).await.map(|_| ())
    }

    async fn mark_read(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        message_external_id: &str,
        read: bool,
    ) -> Result<(), String> {
        let ts = if read { message_external_id.to_string() } else { Self::before(message_external_id) };
        let body = serde_json::json!({ "channel": conversation_external_id, "ts": ts });
        self.post(auth, "conversations.mark", &body).await.map(|_| ())
    }

    async fn edit_message(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        message_external_id: &str,
        content: &str,
    ) -> Result<(), String> {
        let body = serde_json::json!({ "channel": conversation_external_id, "ts": message_external_id, "text": content });
        self.post(auth, "chat.update", &body).await.map(|_| ())
    }

    async fn get_presence(
        &self,
        auth: &ProviderAuth,
        user_id: &str,
    ) -> Result<String, String> {
        let data = self.call(auth, &format!("{}/users.getPresence?user={}", self.api, user_id)).await?;
        Ok(data["presence"].as_str().unwrap_or("unknown").to_string())
    }

    async fn put_member(&self, auth: &ProviderAuth, conversation_external_id: &str, user_external_id: &str, member: bool) -> Result<bool, String> {
        if conversation_external_id.starts_with('D') {
            return Ok(false);
        }
        let (method, body) = if member {
            ("conversations.invite", serde_json::json!({ "channel": conversation_external_id, "users": user_external_id }))
        } else {
            ("conversations.kick", serde_json::json!({ "channel": conversation_external_id, "user": user_external_id }))
        };
        match self.post(auth, method, &body).await {
            Ok(_) => Ok(true),
            Err(e) if e.ends_with("already_in_channel") || e.ends_with("not_in_channel") => Ok(true),
            Err(e) => Err(e),
        }
    }

    async fn put_conversation_name(&self, auth: &ProviderAuth, conversation_external_id: &str, name: &str) -> Result<bool, String> {
        if conversation_external_id.starts_with('D') {
            return Ok(false);
        }
        self.post(auth, "conversations.rename", &serde_json::json!({ "channel": conversation_external_id, "name": channel_name(name) })).await?;
        Ok(true)
    }

    async fn put_pin(&self, auth: &ProviderAuth, conversation_external_id: &str, message_external_id: &str, pinned: bool) -> Result<bool, String> {
        let method = if pinned { "pins.add" } else { "pins.remove" };
        match self.post(auth, method, &serde_json::json!({ "channel": conversation_external_id, "timestamp": message_external_id })).await {
            Ok(_) => Ok(true),
            Err(e) if e.ends_with("already_pinned") || e.ends_with("no_pin") => Ok(true),
            Err(e) => Err(e),
        }
    }

    async fn get_pins(&self, auth: &ProviderAuth, conversation_external_id: &str) -> Result<Option<Vec<String>>, String> {
        let data = self.call(auth, &format!("{}/pins.list?channel={}", self.api, conversation_external_id)).await?;
        Ok(Some(Self::pinned(&data)))
    }

    async fn get_custom_emoji(&self, auth: &ProviderAuth) -> Result<Vec<RemoteEmoji>, String> {
        let data = self.call(auth, &format!("{}/emoji.list", self.api)).await?;
        Ok(emoji_from_list(&data))
    }

    async fn get_people(&self, auth: &ProviderAuth) -> Result<Vec<ResponseUser>, String> {
        let mut people = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut url = format!("{}/users.list?limit={}", self.api, PAGE);
            if let Some(c) = &cursor {
                url.push_str(&format!("&cursor={}", urlencoding::encode(c)));
            }
            let data = self.call(auth, &url).await?;
            for user in data["members"].as_array().cloned().unwrap_or_default().iter().filter(|u| u["deleted"].as_bool() != Some(true)) {
                people.push(profile_user(user));
            }
            cursor = Self::next_cursor(&data);
            if cursor.is_none() {
                break;
            }
        }
        let mut cache = self.users.lock().await;
        for person in &people {
            cache.insert(person.id.clone(), person.clone());
        }
        Ok(people)
    }

    async fn get_attachment(&self, auth: &ProviderAuth, _conversation_external_id: &str, _message_external_id: &str, attachment: &ResponseAttachment) -> Result<Option<Vec<u8>>, String> {
        let Some(url) = attachment.url.as_deref().filter(|u| u.starts_with("https://") || u.starts_with("http://")) else { return Ok(None) };
        let response = crate::providers::http::send(self.client.get(url).bearer_auth(&auth.access_token)).await?;
        if !response.status().is_success() {
            return Err(format!("Slack refused the file {}: {}", attachment.name, response.status()));
        }
        Ok(Some(response.bytes().await.map_err(|e| format!("Slack sent an unreadable file: {}", e))?.to_vec()))
    }

    async fn get_message_link(&self, auth: &ProviderAuth, conversation_external_id: &str, message_external_id: &str) -> Result<Option<String>, String> {
        let data = self.call(auth, &self.permalink_url(conversation_external_id, message_external_id)).await?;
        Ok(data["permalink"].as_str().map(String::from))
    }

    async fn put_status(&self, auth: &ProviderAuth, text: &str, emoji: Option<&str>, until: Option<DateTime<Utc>>) -> Result<bool, String> {
        self.post(auth, "users.profile.set", &status_profile(text, emoji, until)).await?;
        Ok(true)
    }

    fn is_live(&self) -> bool {
        !self.app_token.is_empty()
    }

    async fn get_live(&self, auth: &ProviderAuth, sink: LiveSink) -> Result<(), String> {
        use futures::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message as Frame;
        if self.app_token.is_empty() {
            return Err("Add the Slack app token under Settings, Connections to get live updates".to_string());
        }
        let response = crate::providers::http::send(self.client.post(format!("{}/apps.connections.open", self.api)).bearer_auth(&self.app_token)).await?;
        let opened = Self::read(response).await?;
        let url = opened["url"].as_str().ok_or_else(|| "Slack gave no Socket Mode address".to_string())?;
        let (mut socket, _) = tokio_tungstenite::connect_async(url).await.map_err(|e| format!("Slack Socket Mode did not connect: {}", e))?;
        while let Some(frame) = socket.next().await {
            let frame = frame.map_err(|e| format!("Slack Socket Mode dropped: {}", e))?;
            let text = match frame {
                Frame::Text(text) => text.to_string(),
                Frame::Ping(data) => {
                    socket.send(Frame::Pong(data)).await.map_err(|e| e.to_string())?;
                    continue;
                }
                Frame::Close(_) => return Ok(()),
                _ => continue,
            };
            let Ok(envelope) = serde_json::from_str::<Value>(&text) else { continue };
            if let Some(id) = envelope["envelope_id"].as_str() {
                socket.send(Frame::text(serde_json::json!({ "envelope_id": id }).to_string())).await.map_err(|e| format!("Slack Socket Mode refused the acknowledgement: {}", e))?;
            }
            match envelope["type"].as_str().unwrap_or("") {
                "disconnect" => return Ok(()),
                "events_api" => {
                    for event in self.live_events(auth, &envelope["payload"]["event"]).await {
                        if sink.send(event).await.is_err() {
                            return Ok(());
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn get_user_groups(&self, auth: &ProviderAuth) -> Result<Vec<RemoteUserGroup>, String> {
        let data = self.call(auth, &format!("{}/usergroups.list?include_users=true", self.api)).await?;
        Ok(data["usergroups"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|g| RemoteUserGroup {
                external_id: g["id"].as_str().unwrap_or("").to_string(),
                handle: g["handle"].as_str().unwrap_or("").to_string(),
                name: g["name"].as_str().unwrap_or("").to_string(),
                members: g["users"].as_array().map(|u| u.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
            })
            .collect())
    }

    async fn get_search(&self, auth: &ProviderAuth, search: &RemoteSearch) -> Result<Vec<ResponseMessage>, String> {
        let mut query = search.text.trim().to_string();
        if let Some(from) = &search.from {
            query.push_str(&format!(" from:<@{}>", from));
        }
        if let Some(channel) = &search.conversation_external_id {
            query.push_str(&format!(" in:<#{}>", channel));
        }
        if let Some(before) = search.before {
            query.push_str(&format!(" before:{}", before.format("%Y-%m-%d")));
        }
        if let Some(after) = search.after {
            query.push_str(&format!(" after:{}", after.format("%Y-%m-%d")));
        }
        let data = self.call(auth, &format!("{}/search.messages?query={}&count={}&sort=timestamp", self.api, urlencoding::encode(query.trim()), search.limit.clamp(1, 100))).await?;
        let mut out = Vec::new();
        for hit in data["messages"]["matches"].as_array().cloned().unwrap_or_default() {
            let channel = hit["channel"]["id"].as_str().unwrap_or("").to_string();
            if let Some(message) = self.message(auth, &hit, &channel).await {
                out.push(message);
            }
        }
        Ok(out)
    }

    async fn post_conversation(&self, auth: &ProviderAuth, conversation: &NewRemoteConversation) -> Result<Option<ResponseConversation>, String> {
        let data = match conversation.conversation_type {
            ConversationType::Channel => self.post(auth, "conversations.create", &serde_json::json!({ "name": channel_name(&conversation.name), "is_private": conversation.private })).await?,
            _ => self.post(auth, "conversations.open", &serde_json::json!({ "users": conversation.members.join(","), "return_im": true })).await?,
        };
        let channel = &data["channel"];
        let external_id = channel["id"].as_str().ok_or_else(|| "Slack returned no conversation".to_string())?.to_string();
        if conversation.conversation_type == ConversationType::Channel && !conversation.members.is_empty() {
            self.post(auth, "conversations.invite", &serde_json::json!({ "channel": external_id, "users": conversation.members.join(",") })).await?;
        }
        Ok(Some(ResponseConversation {
            name: channel["name"].as_str().map(String::from).unwrap_or_else(|| conversation.name.clone()),
            conversation_type: Self::map_conversation_type(channel["channel_type"].as_str().unwrap_or(""), channel["is_im"].as_bool().unwrap_or(false), channel["is_mpim"].as_bool().unwrap_or(false)),
            external_id,
            members: Vec::new(),
            last_activity: Utc::now(),
            metadata: HashMap::new(),
        }))
    }

    async fn put_membership(&self, auth: &ProviderAuth, conversation_external_id: &str, joined: bool) -> Result<bool, String> {
        let method = if joined { "conversations.join" } else { "conversations.leave" };
        match self.post(auth, method, &serde_json::json!({ "channel": conversation_external_id })).await {
            Ok(_) => Ok(true),
            Err(e) if e.ends_with("already_in_channel") || e.ends_with("not_in_channel") => Ok(true),
            Err(e) => Err(e),
        }
    }

    async fn put_conversation_state(&self, auth: &ProviderAuth, conversation_external_id: &str, archived: bool) -> Result<bool, String> {
        let method = if archived { "conversations.archive" } else { "conversations.unarchive" };
        match self.post(auth, method, &serde_json::json!({ "channel": conversation_external_id })).await {
            Ok(_) => Ok(true),
            Err(e) if e.ends_with("already_archived") || e.ends_with("not_archived") => Ok(true),
            Err(e) => Err(e),
        }
    }

    async fn put_conversation_topic(&self, auth: &ProviderAuth, conversation_external_id: &str, topic: Option<&str>, purpose: Option<&str>) -> Result<bool, String> {
        if let Some(topic) = topic {
            self.post(auth, "conversations.setTopic", &serde_json::json!({ "channel": conversation_external_id, "topic": topic })).await?;
        }
        if let Some(purpose) = purpose {
            self.post(auth, "conversations.setPurpose", &serde_json::json!({ "channel": conversation_external_id, "purpose": purpose })).await?;
        }
        Ok(topic.is_some() || purpose.is_some())
    }

    async fn get_joinable_conversations(&self, auth: &ProviderAuth) -> Result<Vec<ResponseConversation>, String> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut url = format!("{}/conversations.list?types=public_channel&exclude_archived=true&limit={}", self.api, PAGE);
            if let Some(c) = &cursor {
                url.push_str(&format!("&cursor={}", urlencoding::encode(c)));
            }
            let data = self.call(auth, &url).await?;
            for channel in data["channels"].as_array().cloned().unwrap_or_default().iter().filter(|c| c["is_member"].as_bool() != Some(true)) {
                let mut metadata = HashMap::new();
                for (key, field) in [("topic", "topic"), ("purpose", "purpose")] {
                    if let Some(value) = channel[field]["value"].as_str().filter(|v| !v.is_empty()) {
                        metadata.insert(key.to_string(), Value::String(value.to_string()));
                    }
                }
                if let Some(count) = channel["num_members"].as_u64() {
                    metadata.insert("member_count".to_string(), Value::from(count));
                }
                out.push(ResponseConversation {
                    external_id: channel["id"].as_str().unwrap_or("").to_string(),
                    name: channel["name"].as_str().unwrap_or("").to_string(),
                    conversation_type: ConversationType::Channel,
                    members: Vec::new(),
                    last_activity: channel["updated"].as_f64().map(|ms| Self::ts_time(&format!("{}", ms / 1000.0))).unwrap_or_else(Utc::now),
                    metadata,
                });
            }
            cursor = Self::next_cursor(&data);
            if cursor.is_none() {
                break;
            }
        }
        Ok(out)
    }
}
impl SlackProvider {
    fn before(ts: &str) -> String {
        let at = Self::ts_time(ts) - Duration::microseconds(1);
        format!("{}.{:06}", at.timestamp(), at.timestamp_subsec_micros())
    }

    async fn last_read(&self, auth: &ProviderAuth, channel: &str) -> Option<String> {
        let data = self.call(auth, &format!("{}/conversations.info?channel={}", self.api, urlencoding::encode(channel))).await.ok()?;
        data["channel"]["last_read"].as_str().filter(|ts| !ts.is_empty() && *ts != "0000000000.000000").map(String::from)
    }

    async fn single(&self, auth: &ProviderAuth, channel: &str, ts: &str) -> Option<ResponseMessage> {
        let data = self.call(auth, &format!("{}/conversations.history?channel={}&latest={}&inclusive=true&limit=1", self.api, urlencoding::encode(channel), urlencoding::encode(ts))).await.ok()?;
        let found = data["messages"].as_array()?.iter().find(|m| m["ts"].as_str() == Some(ts)).cloned();
        match found {
            Some(msg) => self.message(auth, &msg, channel).await,
            None => {
                let replies = self.call(auth, &format!("{}/conversations.replies?channel={}&ts={}&latest={}&inclusive=true&limit=1", self.api, urlencoding::encode(channel), urlencoding::encode(ts), urlencoding::encode(ts))).await.ok()?;
                let reply = replies["messages"].as_array()?.iter().find(|m| m["ts"].as_str() == Some(ts)).cloned()?;
                self.message(auth, &reply, channel).await
            }
        }
    }

    pub async fn live_events(&self, auth: &ProviderAuth, event: &Value) -> Vec<LiveEvent> {
        let channel = event["channel"].as_str().or_else(|| event["item"]["channel"].as_str()).or_else(|| event["channel"]["id"].as_str()).unwrap_or("").to_string();
        match event["type"].as_str().unwrap_or("") {
            "message" => match event["subtype"].as_str().unwrap_or("") {
                "message_deleted" => event["deleted_ts"].as_str().map(|ts| LiveEvent::Change { change: RemoteChange { conversation_external_id: Some(channel.clone()), message_external_id: ts.to_string(), deleted: true, ..Default::default() } }).into_iter().collect(),
                "message_changed" => {
                    let mut message = event["message"].clone();
                    if message["thread_ts"].is_null() {
                        message["thread_ts"] = event["previous_message"]["thread_ts"].clone();
                    }
                    self.message(auth, &message, &channel).await.map(|m| LiveEvent::Message { message: Self::threaded(m) }).into_iter().collect()
                }
                "message_replied" => vec![LiveEvent::Touched { conversation_external_id: channel }],
                _ => self.message(auth, event, &channel).await.map(|m| LiveEvent::Message { message: Self::threaded(m) }).into_iter().collect(),
            },
            "reaction_added" | "reaction_removed" => match event["item"]["ts"].as_str() {
                Some(ts) => self.single(auth, &channel, ts).await.map(|m| LiveEvent::Message { message: Self::threaded(m) }).into_iter().collect(),
                None => Vec::new(),
            },
            kind @ ("pin_added" | "pin_removed") => event["item"]["message"]["ts"]
                .as_str()
                .map(|ts| LiveEvent::Change { change: RemoteChange { conversation_external_id: Some(channel.clone()), message_external_id: ts.to_string(), pinned: Some(kind == "pin_added"), ..Default::default() } })
                .into_iter()
                .collect(),
            "channel_created" | "channel_joined" | "group_joined" | "im_created" | "channel_rename" | "group_rename" | "channel_archive" | "group_archive" | "channel_unarchive" | "member_joined_channel" | "member_left_channel" => vec![LiveEvent::Conversation {
                conversation: ResponseConversation {
                    external_id: channel,
                    name: event["channel"]["name"].as_str().unwrap_or("").to_string(),
                    conversation_type: ConversationType::Channel,
                    members: Vec::new(),
                    last_activity: Utc::now(),
                    metadata: HashMap::new(),
                },
            }],
            "user_typing" => vec![LiveEvent::Typing { conversation_external_id: channel, user: self.get_user(auth, event["user"].as_str().unwrap_or(""), None).await }],
            "presence_change" => event["user"]
                .as_str()
                .map(|u| LiveEvent::Presence { user_external_id: u.to_string(), status: event["presence"].as_str().unwrap_or("away").to_string(), text: None, emoji: None })
                .into_iter()
                .collect(),
            "user_change" | "user_status_changed" => {
                let user = &event["user"];
                user["id"]
                    .as_str()
                    .map(|u| LiveEvent::Presence {
                        user_external_id: u.to_string(),
                        status: if user["presence"].is_string() { user["presence"].as_str().unwrap_or("away").to_string() } else { "active".to_string() },
                        text: user["profile"]["status_text"].as_str().filter(|t| !t.is_empty()).map(String::from),
                        emoji: user["profile"]["status_emoji"].as_str().filter(|t| !t.is_empty()).map(String::from),
                    })
                    .into_iter()
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn threaded(mut message: ResponseMessage) -> ResponseMessage {
        if let Some(thread) = message.thread_id.clone().filter(|t| *t != message.external_id) {
            message.reply_to_id.get_or_insert(thread);
        }
        message
    }

    async fn token_call(&self, params: &[(&str, &str)]) -> Result<ProviderAuth, String> {
        let response = crate::providers::http::send(self.client.post(format!("{}/oauth.v2.access", self.api)).form(params)).await?;
        let data: Value = response.json().await.map_err(|e| format!("Failed to read the Slack token response: {}", e))?;
        token_from_response(&data)
    }

    fn ts_time(ts: &str) -> DateTime<Utc> {
        let mut parts = ts.split('.');
        let secs = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
        let micros = parts.next().and_then(|s| format!("{:0<6}", s).parse::<u32>().ok()).unwrap_or(0);
        DateTime::from_timestamp(secs, micros * 1000).unwrap_or_else(Utc::now)
    }
    async fn read(response: reqwest::Response) -> Result<Value, String> {
        let status = response.status();
        let data: Value = response.json().await.map_err(|e| format!("Slack returned unreadable JSON ({}): {}", status, e))?;
        if data["ok"].as_bool() != Some(true) {
            return Err(format!("Slack: {}", data["error"].as_str().unwrap_or("unknown error")));
        }
        Ok(data)
    }
    async fn call(&self, auth: &ProviderAuth, url: &str) -> Result<Value, String> {
        Self::read(crate::providers::http::send(self.client.get(url).bearer_auth(&auth.access_token)).await?).await
    }
    async fn post(&self, auth: &ProviderAuth, method: &str, body: &Value) -> Result<Value, String> {
        let url = format!("{}/{}", self.api, method);
        Self::read(crate::providers::http::send(self.client.post(&url).bearer_auth(&auth.access_token).json(body)).await?).await
    }
    async fn replies(&self, auth: &ProviderAuth, channel: &str, ts: &str, floor: DateTime<Utc>) -> Vec<ResponseMessage> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut url = format!("{}/conversations.replies?channel={}&ts={}&limit={}&oldest={}", self.api, channel, ts, PAGE, floor.timestamp());
            if let Some(c) = &cursor {
                url.push_str(&format!("&cursor={}", urlencoding::encode(c)));
            }
            let Ok(replies) = self.call(auth, &url).await else { break };
            for reply in replies["messages"].as_array().cloned().unwrap_or_default() {
                if reply["ts"].as_str() == Some(ts) {
                    continue;
                }
                if let Some(mut message) = self.message(auth, &reply, channel).await {
                    message.reply_to_id = Some(ts.to_string());
                    message.thread_id = Some(ts.to_string());
                    out.push(message);
                }
            }
            cursor = Self::next_cursor(&replies);
            if cursor.is_none() || replies["has_more"].as_bool() != Some(true) {
                break;
            }
        }
        out
    }
    async fn message(
        &self,
        auth: &ProviderAuth,
        msg: &Value,
        conversation_external_id: &str,
    ) -> Option<ResponseMessage> {
        let ts = msg["ts"].as_str()?.to_string();
        let subtype = msg["subtype"].as_str().unwrap_or("");
        if subtype == "tombstone" {
            return None;
        }
        if let Some(mut event) = Self::system_event(msg) {
            for person in event.people_mut() {
                let user = self.get_user(auth, &person.external_id, None).await;
                person.name = user.display_name.or(user.email).unwrap_or_default();
            }
            let mut message = ResponseMessage::system(ts.clone(), conversation_external_id, Self::ts_time(&ts), event);
            message.sender = self.get_user(auth, &message.sender.id.clone(), None).await;
            message.thread_id = msg["thread_ts"].as_str().filter(|t| *t != ts).map(String::from);
            return Some(message);
        }
        if matches!(subtype, "channel_join" | "channel_leave" | "group_join" | "group_leave" | "channel_topic" | "channel_purpose" | "channel_name" | "pinned_item" | "unpinned_item") {
            return None;
        }
        let user_id = msg["user"].as_str().or_else(|| msg["bot_id"].as_str()).unwrap_or("").to_string();
        if user_id.is_empty() {
            return None;
        }
        let sender = self.get_user(auth, &user_id, msg["username"].as_str().or_else(|| msg["bot_profile"]["name"].as_str())).await;
        let r#type = match subtype {
            "bot_message" => MessageType::Bot,
            _ if msg["bot_id"].is_string() && msg["user"].is_null() => MessageType::Bot,
            _ => MessageType::Chat,
        };
        let mut metadata = HashMap::new();
        if let Some(permalink) = msg["permalink"].as_str() {
            metadata.insert("web_url".to_string(), Value::String(permalink.to_string()));
        }
        let content = crate::content::chat::slack(msg);
        let mut message = ResponseMessage {
            external_id: ts.clone(),
            conversation_external_id: conversation_external_id.to_string(),
            sender,
            content: content.text.clone(),
            content_type: "text".to_string(),
            timestamp: Self::ts_time(&ts),
            r#type,
            metadata,
            ..Default::default()
        };
        content.apply(&mut message);
        Some(message)
    }
    async fn get_user(&self, auth: &ProviderAuth, user_id: &str, username: Option<&str>) -> ResponseUser {
        if let Some(cached) = self.users.lock().await.get(user_id) {
            return cached.clone();
        }
        let user = if user_id.starts_with('B') {
            match self.call(auth, &format!("{}/bots.info?bot={}", self.api, user_id)).await {
                Ok(data) => ResponseUser {
                    id: user_id.to_string(),
                    display_name: data["bot"]["name"].as_str().map(String::from).or(username.map(String::from)).or(Some("Bot".to_string())),
                    avatar_url: data["bot"]["icons"]["image_72"].as_str().or_else(|| data["bot"]["icons"]["image_48"].as_str()).map(String::from),
                    ..Default::default()
                },
                Err(_) => ResponseUser { id: user_id.to_string(), display_name: username.map(String::from).or(Some("Bot".to_string())), ..Default::default() },
            }
        } else {
            self.fetch_user_details(auth, user_id).await.unwrap_or_else(|_| ResponseUser { id: user_id.to_string(), display_name: Some(user_id.to_string()), ..Default::default() })
        };
        self.users.lock().await.insert(user_id.to_string(), user.clone());
        user
    }
    fn system_event(msg: &Value) -> Option<SystemEvent> {
        let user = msg["user"].as_str().filter(|u| !u.is_empty()).map(|u| SystemPerson::new(u, ""))?;
        let inviter = msg["inviter"].as_str().filter(|u| !u.is_empty() && Some(*u) != msg["user"].as_str()).map(|u| SystemPerson::new(u, ""));
        let text = |key: &str| msg[key].as_str().map(String::from);
        Some(match msg["subtype"].as_str().unwrap_or("") {
            "channel_join" | "group_join" => match inviter {
                Some(inviter) => SystemEvent::new(SystemEventKind::MemberAdded, vec![inviter], vec![user], None),
                None => SystemEvent::new(SystemEventKind::MemberJoined, vec![user], Vec::new(), None),
            },
            "channel_leave" | "group_leave" => SystemEvent::new(SystemEventKind::MemberLeft, vec![user], Vec::new(), None),
            "channel_topic" | "group_topic" => SystemEvent::new(SystemEventKind::TopicChanged, vec![user], Vec::new(), text("topic")),
            "channel_purpose" | "group_purpose" => SystemEvent::new(SystemEventKind::TopicChanged, vec![user], Vec::new(), text("purpose")),
            "channel_name" | "group_name" => SystemEvent::new(SystemEventKind::Renamed, vec![user], Vec::new(), text("name")),
            "pinned_item" => SystemEvent::new(SystemEventKind::Pinned, vec![user], Vec::new(), None),
            "unpinned_item" => SystemEvent::new(SystemEventKind::Unpinned, vec![user], Vec::new(), None),
            "huddle_thread" => SystemEvent::new(SystemEventKind::CallStarted, vec![user], Vec::new(), None),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_install_link_asks_for_user_scopes_with_pkce() {
        let url = auth_url("123.456", "http://localhost:48731/callback/slack", "s1", Some("ch"));
        assert!(url.starts_with("https://slack.com/oauth/v2/authorize?client_id=123.456&user_scope="));
        assert!(url.contains("channels%3Ahistory%2Cchannels%3Aread") && !url.contains("&scope="));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A48731%2Fcallback%2Fslack") && url.ends_with("&code_challenge=ch&code_challenge_method=S256"));
    }

    #[test]
    fn user_tokens_are_read_from_the_install_and_the_refresh_responses() {
        let install = token_from_response(&json!({ "ok": true, "authed_user": { "id": "U1", "access_token": "xoxp-1", "refresh_token": "xoxe-1", "expires_in": 43200, "scope": "chat:write" }, "team": { "id": "T1" } })).unwrap();
        assert_eq!(install.access_token, "xoxp-1");
        assert_eq!(install.refresh_token.as_deref(), Some("xoxe-1"));
        assert!(install.expires_at < Utc::now() + Duration::hours(13));
        let refreshed = token_from_response(&json!({ "ok": true, "access_token": "xoxe.xoxp-2", "refresh_token": "xoxe-2", "expires_in": 43200, "token_type": "user" })).unwrap();
        assert_eq!(refreshed.access_token, "xoxe.xoxp-2");
        let lasting = token_from_response(&json!({ "ok": true, "authed_user": { "access_token": "xoxp-3" } })).unwrap();
        assert!(lasting.refresh_token.is_none() && lasting.expires_at > Utc::now() + Duration::days(3650));
    }

    #[test]
    fn slack_refusals_and_bot_only_installs_are_errors() {
        assert!(token_from_response(&json!({ "ok": false, "error": "invalid_code" })).unwrap_err().contains("invalid_code"));
        assert!(token_from_response(&json!({ "ok": true, "access_token": null, "authed_user": { "id": "U1" } })).unwrap_err().contains("user scopes"));
    }

    #[test]
    fn the_manifest_asks_for_exactly_the_scopes_the_app_uses() {
        let manifest: Value = serde_json::from_str(include_str!("../../../../docs/slack-manifest.json")).unwrap();
        let listed: Vec<&str> = manifest["oauth_config"]["scopes"]["user"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect();
        assert_eq!(listed, USER_SCOPES.to_vec());
        assert_eq!(manifest["oauth_config"]["redirect_urls"][0], "http://localhost:48731/callback/slack");
        assert_eq!(manifest["oauth_config"]["pkce_enabled"], true);
        assert!(manifest["oauth_config"]["scopes"]["bot"].is_null());
    }

    #[tokio::test]
    async fn without_a_client_id_slack_falls_back_to_the_pasted_token() {
        let provider = SlackProvider::new(String::new(), String::new(), "http://localhost:48731/callback/slack".into());
        assert!(provider.get_auth_url("s", None).await.is_err());
        assert!(provider.connects_with_token());
    }

    #[test]
    fn a_quote_is_the_permalink_under_the_text() {
        assert_eq!(SlackProvider::quoted_text("Agreed ", &[]), "Agreed");
        assert_eq!(SlackProvider::quoted_text("Agreed", &["https://northwind.slack.com/archives/C1/p1728470000000100".to_string()]), "Agreed\nhttps://northwind.slack.com/archives/C1/p1728470000000100");
        assert_eq!(SlackProvider::quoted_text("", &["a".to_string(), "b".to_string()]), "a\nb");
        assert!(SlackProvider::new(String::new(), String::new(), String::new()).permalink_url("C1", "1728470000.000100").ends_with("/chat.getPermalink?channel=C1&message_ts=1728470000.000100"));
    }

    #[tokio::test]
    async fn a_quoted_reply_posts_the_permalink_outside_any_thread() {
        use crate::providers::traits::OutgoingReference;
        use wiremock::matchers::{method, path, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/chat.getPermalink")).and(query_param("message_ts", "1728470000.000100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true, "permalink": "https://northwind.slack.com/archives/C1/p1728470000000100" })))
            .mount(&server).await;
        Mock::given(method("POST")).and(path("/chat.postMessage"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true, "message": { "ts": "1728470100.000200", "user": "U0ME" } })))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/users.info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": false, "error": "user_not_found" })))
            .mount(&server).await;
        let provider = SlackProvider::new(String::new(), String::new(), String::new()).with_api(&server.uri());
        let auth = ProviderAuth { access_token: "xoxp".into(), refresh_token: None, expires_at: Utc::now(), token_type: "Bearer".into(), scope: None };
        let quote = OutgoingReference { external_id: "1728470000.000100".into(), conversation_external_id: Some("C1".into()), sender_name: "Sam".into(), native: true, ..Default::default() };
        let draft = OutgoingMessage { content: "Agreed".into(), content_type: "text".into(), in_reply_to: Some("1728470000.000100".into()), quote: Some(quote), ..Default::default() };
        let sent = provider.send_message(&auth, "C1", &draft).await.unwrap();
        assert_eq!(sent.reply_to_id.as_deref(), Some("1728470000.000100"));
        assert!(sent.thread_id.is_none());
        let posted = server.received_requests().await.unwrap().into_iter().find(|r| r.url.path() == "/chat.postMessage").unwrap();
        let body: Value = serde_json::from_slice(&posted.body).unwrap();
        assert_eq!(body["text"], "Agreed\nhttps://northwind.slack.com/archives/C1/p1728470000000100");
        assert!(body.get("thread_ts").is_none());
        assert_eq!(body["unfurl_links"], true);
    }

    #[test]
    fn slack_declares_native_quotes_and_forwards() {
        let caps = SlackProvider::new(String::new(), String::new(), String::new()).get_provider().capabilities.clone();
        assert!(caps.supports_reply_quote && caps.supports_forward);
        assert_eq!((caps.reply_quote, caps.forward), (Delivery::Native, Delivery::Native));
    }

    fn slack_parsed(msg: Value, names: &[(&str, &str)]) -> (SystemEvent, String) {
        let mut event = SlackProvider::system_event(&msg).unwrap();
        for person in event.people_mut() {
            person.name = names.iter().find(|(id, _)| *id == person.external_id).map(|(_, name)| name.to_string()).unwrap_or_default();
        }
        let sentence = event.sentence();
        (event, sentence)
    }

    #[test]
    fn slack_joins_and_leaves_become_member_events() {
        let names = [("U0MIA", "Mia Sund"), ("U0HUGO", "Hugo Brandt"), ("U0ANNA", "Anna Berg")];
        let (joined, sentence) = slack_parsed(json!({ "type": "message", "subtype": "channel_join", "ts": "1728470000.000100", "user": "U0MIA", "text": "<@U0MIA> has joined the channel" }), &names);
        assert_eq!((joined.event, joined.actors[0].external_id.as_str(), joined.targets.len()), (SystemEventKind::MemberJoined, "U0MIA", 0));
        assert_eq!(sentence, "Mia Sund joined the conversation");
        let (added, sentence) = slack_parsed(json!({ "subtype": "channel_join", "ts": "1", "user": "U0MIA", "inviter": "U0ANNA" }), &names);
        assert_eq!((added.event, added.actors[0].external_id.as_str(), added.targets[0].external_id.as_str()), (SystemEventKind::MemberAdded, "U0ANNA", "U0MIA"));
        assert_eq!(sentence, "Anna Berg added Mia Sund");
        let (_, sentence) = slack_parsed(json!({ "subtype": "group_join", "ts": "1", "user": "U0MIA", "inviter": "U0MIA" }), &names);
        assert_eq!(sentence, "Mia Sund joined the conversation");
        let (left, sentence) = slack_parsed(json!({ "subtype": "channel_leave", "ts": "1", "user": "U0HUGO", "text": "<@U0HUGO> has left the channel" }), &names);
        assert_eq!(left.event, SystemEventKind::MemberLeft);
        assert_eq!(sentence, "Hugo Brandt left the conversation");
        let (_, sentence) = slack_parsed(json!({ "subtype": "group_leave", "ts": "1", "user": "U0HUGO" }), &names);
        assert_eq!(sentence, "Hugo Brandt left the conversation");
    }

    #[test]
    fn slack_topic_purpose_name_pins_and_huddles_carry_their_detail() {
        let names = [("U0ANNA", "Anna Berg")];
        let cases = [
            (json!({ "subtype": "channel_topic", "ts": "1", "user": "U0ANNA", "topic": "Release 0.5" }), SystemEventKind::TopicChanged, Some("Release 0.5"), "Anna Berg changed the topic to “Release 0.5”"),
            (json!({ "subtype": "channel_topic", "ts": "1", "user": "U0ANNA", "topic": "" }), SystemEventKind::TopicChanged, None, "Anna Berg cleared the topic"),
            (json!({ "subtype": "channel_purpose", "ts": "1", "user": "U0ANNA", "purpose": "Ship it" }), SystemEventKind::TopicChanged, Some("Ship it"), "Anna Berg changed the topic to “Ship it”"),
            (json!({ "subtype": "channel_name", "ts": "1", "user": "U0ANNA", "old_name": "dev", "name": "dev-team" }), SystemEventKind::Renamed, Some("dev-team"), "Anna Berg renamed the conversation to “dev-team”"),
            (json!({ "subtype": "pinned_item", "ts": "1", "user": "U0ANNA", "item_type": "C" }), SystemEventKind::Pinned, None, "Anna Berg pinned a message"),
            (json!({ "subtype": "unpinned_item", "ts": "1", "user": "U0ANNA" }), SystemEventKind::Unpinned, None, "Anna Berg unpinned a message"),
            (json!({ "subtype": "huddle_thread", "ts": "1", "user": "U0ANNA" }), SystemEventKind::CallStarted, None, "Anna Berg started a call"),
        ];
        for (msg, kind, detail, sentence) in cases {
            let (event, built) = slack_parsed(msg, &names);
            assert_eq!((event.event, event.detail.as_deref(), built.as_str()), (kind, detail, sentence));
        }
    }

    #[test]
    fn plain_slack_messages_and_events_without_a_user_are_not_system_events() {
        assert!(SlackProvider::system_event(&json!({ "ts": "1", "user": "U0ANNA", "text": "Hei" })).is_none());
        assert!(SlackProvider::system_event(&json!({ "subtype": "bot_message", "ts": "1", "bot_id": "B1" })).is_none());
        assert!(SlackProvider::system_event(&json!({ "subtype": "channel_join", "ts": "1" })).is_none());
    }

    #[test]
    fn a_slack_system_message_keeps_the_sentence_as_content() {
        let (event, _) = slack_parsed(json!({ "subtype": "channel_join", "ts": "1", "user": "U0MIA" }), &[("U0MIA", "Mia Sund")]);
        let message = ResponseMessage::system("1728470000.000100", "C1", SlackProvider::ts_time("1728470000.000100"), event);
        assert_eq!((message.r#type, message.content.as_str(), message.sender.id.as_str()), (MessageType::System, "Mia Sund joined the conversation", "U0MIA"));
        assert_eq!(message.metadata["system"]["actors"][0]["name"], "Mia Sund");
    }

    mod served {
        use super::super::*;
        use serde_json::json;
        use wiremock::matchers::{method, path, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        fn fixture(name: &str) -> Value {
            serde_json::from_str(&std::fs::read_to_string(format!("{}/tests/fixtures/slack/{}", env!("CARGO_MANIFEST_DIR"), name)).unwrap()).unwrap()
        }

        fn auth() -> ProviderAuth {
            ProviderAuth { access_token: "xoxp-test".into(), refresh_token: None, expires_at: Utc::now() + Duration::days(1), token_type: "Bearer".into(), scope: None }
        }

        fn slack(server: &MockServer) -> SlackProvider {
            SlackProvider::new(String::new(), String::new(), String::new()).with_api(&server.uri())
        }

        async fn get(server: &MockServer, at: &str, body: Value) {
            Mock::given(method("GET")).and(path(at)).respond_with(ResponseTemplate::new(200).set_body_json(body)).mount(server).await;
        }

        async fn bodies(server: &MockServer, at: &str) -> Vec<Value> {
            server.received_requests().await.unwrap().into_iter().filter(|r| r.url.path() == at).map(|r| serde_json::from_slice(&r.body).unwrap_or(Value::Null)).collect()
        }

        async fn people(server: &MockServer) {
            Mock::given(method("GET")).and(path("/users.info")).and(query_param("user", "U0ANNA")).respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true, "user": fixture("users-page-1.json")["members"][0].clone() }))).mount(server).await;
            Mock::given(method("GET")).and(path("/users.info")).and(query_param("user", "U0ME")).respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true, "user": { "id": "U0ME", "profile": { "real_name": "Milo Oakes", "email": "milo@northwind.example" } } }))).mount(server).await;
        }

        #[tokio::test]
        async fn every_conversation_type_pages_by_cursor_after_a_429_and_carries_archive_links() {
            let server = MockServer::start().await;
            people(&server).await;
            get(&server, "/team.info", json!({ "ok": true, "team": { "id": "T0", "domain": "northwind" } })).await;
            Mock::given(method("GET")).and(path("/conversations.list")).respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0")).up_to_n_times(1).with_priority(1).mount(&server).await;
            Mock::given(method("GET")).and(path("/conversations.list")).and(query_param("cursor", "dXNlcjpVMEc5V0ZYTlo=")).respond_with(ResponseTemplate::new(200).set_body_json(fixture("conversations-page-2.json"))).with_priority(2).mount(&server).await;
            get(&server, "/conversations.list", fixture("conversations-page-1.json")).await;
            get(&server, "/conversations.members", json!({ "ok": true, "members": ["U0ANNA", "U0ME"], "response_metadata": { "next_cursor": "" } })).await;
            let since = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z").unwrap().with_timezone(&Utc);
            let conversations = slack(&server).sync_conversations(&auth(), Some(since)).await.unwrap();
            assert_eq!(conversations.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["dev", "Anna Berg"]);
            assert_eq!(conversations[0].metadata["web_url"], "https://northwind.slack.com/archives/C0DEV");
            assert_eq!(conversations[0].metadata["topic"], "Releases :rocket:");
            assert_eq!(conversations[1].conversation_type, ConversationType::DirectMessage);
            assert_eq!(conversations[1].members.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["U0ANNA", "U0ME"]);
            let listed = server.received_requests().await.unwrap().into_iter().filter(|r| r.url.path() == "/conversations.list").collect::<Vec<_>>();
            assert_eq!(listed.len(), 3);
            let query: std::collections::HashMap<String, String> = listed[1].url.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            assert_eq!(query.get("types").map(String::as_str), Some("public_channel,private_channel,mpim,im"));
            assert_eq!(query.get("limit").map(String::as_str), Some("200"));
        }

        #[tokio::test]
        async fn history_threads_page_bots_resolve_and_shortcodes_stay_canonical() {
            let server = MockServer::start().await;
            people(&server).await;
            get(&server, "/conversations.history", fixture("history.json")).await;
            Mock::given(method("GET")).and(path("/conversations.replies")).and(query_param("cursor", "replies-2")).respond_with(ResponseTemplate::new(200).set_body_json(fixture("replies-page-2.json"))).with_priority(1).mount(&server).await;
            get(&server, "/conversations.replies", fixture("replies-page-1.json")).await;
            get(&server, "/bots.info", json!({ "ok": true, "bot": { "id": "B0CI", "name": "CI", "icons": { "image_72": "https://avatars.slack-edge.com/ci_72.png" } } })).await;
            let since = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z").unwrap().with_timezone(&Utc);
            let messages = slack(&server).sync_messages(&auth(), "C0DEV", Some(since), None).await.unwrap();
            assert_eq!(messages.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(), vec!["Build passed :white_check_mark:", "Andre svar :+1::skin-tone-3:", "Første svar", "Ny release :tada:"]);
            assert_eq!(messages[0].r#type, MessageType::Bot);
            assert_eq!((messages[0].sender.display_name.as_deref(), messages[0].sender.avatar_url.as_deref()), (Some("CI"), Some("https://avatars.slack-edge.com/ci_72.png")));
            assert_eq!(messages[1].thread_id.as_deref(), Some("1760515200.000100"));
            assert_eq!(messages[3].sender.display_name.as_deref(), Some("Anna Berg"));
            assert_eq!(server.received_requests().await.unwrap().iter().filter(|r| r.url.path() == "/users.info").count(), 1);
        }

        #[tokio::test]
        async fn the_directory_custom_emoji_and_pins_are_read_with_pagination() {
            let server = MockServer::start().await;
            Mock::given(method("GET")).and(path("/users.list")).and(query_param("cursor", "page-2")).respond_with(ResponseTemplate::new(200).set_body_json(fixture("users-page-2.json"))).with_priority(1).mount(&server).await;
            get(&server, "/users.list", fixture("users-page-1.json")).await;
            get(&server, "/emoji.list", fixture("emoji-list.json")).await;
            get(&server, "/pins.list", fixture("pins-list.json")).await;
            let provider = slack(&server);
            let people = provider.get_people(&auth()).await.unwrap();
            assert_eq!(people.iter().map(|p| (p.id.as_str(), p.display_name.as_deref())).collect::<Vec<_>>(), vec![("U0ANNA", Some("Anna Berg")), ("U0BOT", Some("ci-bot"))]);
            assert_eq!(people[0].avatar_url.as_deref(), Some("https://avatars.slack-edge.com/anna_192.png"));
            assert_eq!(people[0].job_title.as_deref(), Some("Innkjøp"));
            assert_eq!(provider.get_user(&auth(), "U0ANNA", None).await.email.as_deref(), Some("anna.berg@client.example"));
            assert!(!server.received_requests().await.unwrap().iter().any(|r| r.url.path() == "/users.info"));
            assert_eq!(provider.get_custom_emoji(&auth()).await.unwrap(), vec![
                RemoteEmoji { name: "party_parrot".into(), url: Some("https://emoji.slack-edge.com/T0/party_parrot/2f0f.gif".into()), alias_for: None },
                RemoteEmoji { name: "partyparrot".into(), url: None, alias_for: Some("party_parrot".into()) },
                RemoteEmoji { name: "shipit".into(), url: Some("https://emoji.slack-edge.com/T0/shipit/7d3a.png".into()), alias_for: None },
            ]);
            assert_eq!(provider.get_pins(&auth(), "C0DEV").await.unwrap(), Some(vec!["1728470000.000100".to_string()]));
        }

        #[tokio::test]
        async fn writes_use_the_slack_methods_for_reactions_pins_members_names_status_and_files() {
            let server = MockServer::start().await;
            for method_name in ["reactions.add", "pins.add", "pins.remove", "conversations.invite", "conversations.kick", "conversations.rename", "users.profile.set", "conversations.mark"] {
                Mock::given(method("POST")).and(path(format!("/{}", method_name))).respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true }))).mount(&server).await;
            }
            Mock::given(method("POST")).and(path("/reactions.remove")).respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": false, "error": "no_reaction" }))).mount(&server).await;
            Mock::given(method("GET")).and(path("/files-pri/T0/F0LOG/sync.log")).respond_with(ResponseTemplate::new(200).set_body_string("log")).mount(&server).await;
            let provider = slack(&server);
            let auth = auth();
            provider.send_reaction(&auth, "C0DEV", "1.2", ":+1::skin-tone-3:", true).await.unwrap();
            provider.send_reaction(&auth, "C0DEV", "1.2", "👍🏼", false).await.unwrap();
            assert_eq!(bodies(&server, "/reactions.add").await, vec![json!({ "channel": "C0DEV", "timestamp": "1.2", "name": "+1::skin-tone-3" })]);
            assert_eq!(bodies(&server, "/reactions.remove").await[0]["name"], "+1::skin-tone-3");
            assert!(provider.put_pin(&auth, "C0DEV", "1.2", true).await.unwrap() && provider.put_pin(&auth, "C0DEV", "1.2", false).await.unwrap());
            assert_eq!(bodies(&server, "/pins.add").await, vec![json!({ "channel": "C0DEV", "timestamp": "1.2" })]);
            assert!(provider.put_member(&auth, "C0DEV", "U0ANNA", true).await.unwrap());
            assert!(provider.put_member(&auth, "C0DEV", "U0ANNA", false).await.unwrap());
            assert!(!provider.put_member(&auth, "D0ANNA", "U0ANNA", true).await.unwrap());
            assert_eq!(bodies(&server, "/conversations.invite").await, vec![json!({ "channel": "C0DEV", "users": "U0ANNA" })]);
            assert_eq!(bodies(&server, "/conversations.kick").await, vec![json!({ "channel": "C0DEV", "user": "U0ANNA" })]);
            assert!(provider.put_conversation_name(&auth, "C0DEV", "Release Plan 2026!").await.unwrap());
            assert_eq!(bodies(&server, "/conversations.rename").await[0]["name"], "release-plan-2026");
            let until = DateTime::parse_from_rfc3339("2026-10-20T16:00:00Z").unwrap().with_timezone(&Utc);
            assert!(provider.put_status(&auth, " Ferie ", Some("🌴"), Some(until)).await.unwrap());
            assert_eq!(bodies(&server, "/users.profile.set").await, vec![json!({ "profile": { "status_text": "Ferie", "status_emoji": ":palm_tree:", "status_expiration": until.timestamp() } })]);
            provider.mark_read(&auth, "C0DEV", "1.2", false).await.unwrap();
            assert_eq!(bodies(&server, "/conversations.mark").await, vec![json!({ "channel": "C0DEV", "ts": "1.199999" })]);
            provider.mark_read(&auth, "C0DEV", "1.2", true).await.unwrap();
            assert_eq!(bodies(&server, "/conversations.mark").await[1], json!({ "channel": "C0DEV", "ts": "1.2" }));
            let file = ResponseAttachment { external_id: "F0LOG".into(), name: "sync.log".into(), content_type: "text/plain".into(), size: 3, url: Some(format!("{}/files-pri/T0/F0LOG/sync.log", server.uri())), content_id: None };
            assert_eq!(provider.get_attachment(&auth, "C0DEV", "1.2", &file).await.unwrap(), Some(b"log".to_vec()));
            let download = server.received_requests().await.unwrap().into_iter().find(|r| r.url.path() == "/files-pri/T0/F0LOG/sync.log").unwrap();
            assert_eq!(download.headers.get("authorization").unwrap(), "Bearer xoxp-test");
        }

        #[test]
        fn channel_names_and_status_follow_slack_rules() {
            assert_eq!(channel_name("  Q4 Board / Økonomi "), "q4-board-økonomi");
            assert_eq!(status_profile("Lunsj", None, None), json!({ "profile": { "status_text": "Lunsj", "status_emoji": "", "status_expiration": 0 } }));
            assert_eq!(status_profile("Lunsj", Some(":party_parrot:"), None)["profile"]["status_emoji"], ":party_parrot:");
        }
    }
}
