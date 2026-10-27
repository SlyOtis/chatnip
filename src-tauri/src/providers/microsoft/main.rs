use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use reqwest::Client;
use serde_json::Value;
use base64::{Engine as _, engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD}};
use crate::utils::text_extraction::html_escape;
use crate::providers::traits::{
    ProviderTrait, ConversationType, MessageType,
    ResponseConversation, ResponseMessage, ResponseUser, ConnectionIdentity, ResponseAttachment, OutgoingAttachment, OutgoingMessage, OutgoingReference,
    RemoteCalendar, RemoteEvent, NewRemoteEvent, SystemEvent, SystemEventKind, SystemPerson, Delta, RemoteChange,
    NewRemoteConversation, RemoteAlias, RemoteAutoReply, RemoteSearch
};
use crate::providers::models::{Delivery, Provider, ProviderCapabilities, ProviderAuth};
use crate::models::user::User;
use std::collections::HashMap;

pub const DEFAULT_TENANT: &str = "organizations";
pub const LOGIN_HOST: &str = "https://login.microsoftonline.com";
pub const GRAPH: &str = "https://graph.microsoft.com/v1.0";
pub const BASE_SCOPES: [&str; 17] = [
    "offline_access",
    "User.Read",
    "User.ReadBasic.All",
    "People.Read",
    "Chat.ReadWrite",
    "ChannelMessage.Send",
    "Team.ReadBasic.All",
    "Channel.ReadBasic.All",
    "Mail.ReadWrite",
    "Mail.Send",
    "Calendars.ReadWrite",
    "OnlineMeetings.ReadWrite",
    "Presence.Read.All",
    "Presence.ReadWrite",
    "Files.ReadWrite",
    "Files.Read.All",
    "MailboxSettings.ReadWrite",
];
pub const ADMIN_SCOPES: [&str; 2] = ["ChannelMessage.ReadWrite", "ChannelMessage.Read.All"];
pub const ADMIN_CONSENT_STATE: &str = "admin-consent";
pub const MAIL_FOLDERS: [&str; 3] = ["inbox", "sentitems", "archive"];
pub const CHATS_CURSOR: &str = "graph:chats";
pub const CHAT_FILES_FOLDER: &str = "Microsoft Teams Chat Files";
pub const TEAMS_WEB: &str = "https://teams.microsoft.com";

pub fn scopes(admin: bool) -> String {
    let mut list: Vec<&str> = BASE_SCOPES.to_vec();
    if admin {
        list.extend(ADMIN_SCOPES);
    }
    list.join(" ")
}

pub fn normalize_tenant(raw: &str) -> Result<String, String> {
    let tenant = raw.trim().to_lowercase();
    if tenant.is_empty() {
        return Ok(DEFAULT_TENANT.to_string());
    }
    if matches!(tenant.as_str(), "organizations" | "common" | "consumers") {
        return Ok(tenant);
    }
    let guid = tenant.len() == 36 && tenant.chars().enumerate().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { c == '-' } else { c.is_ascii_hexdigit() });
    let domain = tenant.contains('.') && !tenant.starts_with('.') && !tenant.ends_with('.') && tenant.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if guid || domain {
        return Ok(tenant);
    }
    Err(format!("{} is not a tenant. Use organizations, common, a tenant id or a domain such as contoso.onmicrosoft.com", raw.trim()))
}

pub fn authority(tenant: &str) -> String {
    format!("{}/{}/oauth2/v2.0", LOGIN_HOST, tenant)
}

pub fn admin_consent_url(tenant: &str, client_id: &str, redirect_uri: &str) -> String {
    let graph: Vec<String> = BASE_SCOPES.iter().chain(ADMIN_SCOPES.iter()).filter(|s| **s != "offline_access").map(|s| format!("https://graph.microsoft.com/{}", s)).collect();
    format!(
        "{}/{}/v2.0/adminconsent?client_id={}&scope={}&redirect_uri={}&state={}",
        LOGIN_HOST,
        tenant,
        urlencoding::encode(client_id),
        urlencoding::encode(&graph.join(" ")),
        urlencoding::encode(redirect_uri),
        ADMIN_CONSENT_STATE
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentIssue {
    NeedsAdmin,
    Declined,
    RedirectMismatch,
    UnknownApp,
    WrongAccount,
}

pub fn consent_issue(error: &str, description: &str) -> Option<ConsentIssue> {
    let code = description.split(|c: char| !c.is_ascii_alphanumeric()).find(|w| w.starts_with("AADSTS")).unwrap_or("");
    match code {
        "AADSTS65001" | "AADSTS90094" | "AADSTS90095" | "AADSTS90099" => return Some(ConsentIssue::NeedsAdmin),
        "AADSTS65004" => return Some(ConsentIssue::Declined),
        "AADSTS50011" | "AADSTS500113" => return Some(ConsentIssue::RedirectMismatch),
        "AADSTS700016" => return Some(ConsentIssue::UnknownApp),
        "AADSTS50020" | "AADSTS50194" | "AADSTS9002331" => return Some(ConsentIssue::WrongAccount),
        _ => {}
    }
    match error {
        "consent_required" | "interaction_required" => Some(ConsentIssue::NeedsAdmin),
        "access_denied" => Some(ConsentIssue::Declined),
        _ => None,
    }
}

pub fn consent_text(issue: ConsentIssue) -> (&'static str, &'static str) {
    match issue {
        ConsentIssue::NeedsAdmin => ("Your organisation has to approve Chatnip first.", "Microsoft 365 asks an administrator to approve this app for the whole organisation before anyone can sign in. Send the approval link below to your IT administrator, or open it yourself if you are one. Every person still signs in with their own account afterwards."),
        ConsentIssue::Declined => ("Access was not granted.", "The consent screen was closed or declined. Nothing was stored. Start again from Chatnip when you are ready."),
        ConsentIssue::RedirectMismatch => ("The app registration is missing the Chatnip redirect address.", "Add http://localhost:48731/callback/microsoft as a Mobile and desktop redirect URI on the app registration, then try again."),
        ConsentIssue::UnknownApp => ("This organisation does not know the Chatnip app yet.", "The app registration is not multi-tenant, or it has not been approved here. An administrator can approve it with the link below."),
        ConsentIssue::WrongAccount => ("That account cannot sign in to this app.", "Use a work or school account from an organisation the app accepts, or change the tenant under Settings, Connections."),
    }
}
pub struct MicrosoftProvider {
    provider: Provider,
    client: Client,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    tenant: String,
    admin: bool,
    api: String,
    me: tokio::sync::OnceCell<(String, Option<String>)>,
}
impl MicrosoftProvider {
    pub fn new(client_id: String, client_secret: String, redirect_uri: String, tenant: String, admin: bool) -> Self {
        let provider = Provider {
            id: "microsoft".to_string(),
            profile_id: String::new(),
            provider_type: "microsoft".to_string(),
            name: "Microsoft 365".to_string(),
            external_id: None,
            logo_id: None,
            logo: None,
            capabilities: ProviderCapabilities {
                can_send_messages: true,
                can_edit_messages: true,
                can_delete_messages: true,
                can_create_conversations: false,
                can_add_members: true,
                supports_reactions: true,
                supports_threads: true,
                supports_attachments: true,
                supports_presence: true,
                supports_typing_indicators: false,
                supports_read_receipts: false,
                supports_reply_quote: true,
                supports_forward: true,
                reply_quote: Delivery::Native,
                forward: Delivery::Repost,
                supports_calendar_responses: true,
                supports_unread_sync: true,
                thread_broadcast: Delivery::Repost,
                drafts: Delivery::Native,
                archive: Delivery::Native,
                supports_flags: true,
                supports_channel_create: true,
                supports_purpose: true,
                supports_search: true,
                supports_aliases: true,
                supports_auto_reply: true,
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
            tenant,
            admin,
            api: std::env::var("MICROSOFT_API_ENDPOINT").unwrap_or_else(|_| GRAPH.to_string()),
            me: tokio::sync::OnceCell::new(),
        }
    }

    pub fn with_api(mut self, api: &str) -> Self {
        self.api = api.trim_end_matches('/').to_string();
        self
    }

    pub async fn ensure_valid_auth(
        &self,
        pool: &sqlx::Pool<sqlx::Sqlite>,
        provider_id: &str,
    ) -> Result<ProviderAuth, String> {
        let mut provider = Provider::load_from_db(pool, provider_id).await?;
        let auth = provider.auth.as_ref().ok_or_else(|| "No auth tokens found".to_string())?;
        if auth.expires_at < Utc::now() + Duration::minutes(5) {
            let refresh_token = auth.refresh_token.as_ref().ok_or_else(|| "No refresh token available".to_string())?;
            let new_auth = self.refresh_token(refresh_token).await?;
            provider.update_auth(pool, new_auth.clone()).await?;
            Ok(new_auth)
        } else {
            Ok(auth.clone())
        }
    }

    pub async fn fetch_current_user(&self, auth: &ProviderAuth) -> Result<User, String> {
        let data = self.get_json(auth, &format!("{}/me", self.api)).await?;
        Ok(User {
            id: data["id"].as_str().unwrap_or("").to_string(),
            email: data["mail"].as_str().or_else(|| data["userPrincipalName"].as_str()).map(String::from),
            phone: data["mobilePhone"].as_str().map(String::from),
            display_name: data["displayName"].as_str().map(String::from),
            first_name: data["givenName"].as_str().map(String::from),
            last_name: data["surname"].as_str().map(String::from),
            avatar_url: None,
            avatar_data: None,
            job_title: data["jobTitle"].as_str().map(String::from),
            department: data["department"].as_str().map(String::from),
            company: data["companyName"].as_str().map(String::from),
            office_location: data["officeLocation"].as_str().map(String::from),
            first_seen: Utc::now().to_rfc3339(),
            last_updated: Utc::now().to_rfc3339(),
        })
    }

    pub async fn fetch_members(&self, auth: &ProviderAuth, chat_id: &str) -> Result<Vec<ResponseUser>, String> {
        let mut members = Vec::new();
        let mut next = Some(format!("{}/chats/{}/members", self.api, chat_id));
        while let Some(url) = next.take() {
            let data = self.get_json(auth, &url).await?;
            members.extend(data["value"].as_array().cloned().unwrap_or_default().iter().filter(|m| m["userId"].is_string()).map(Self::member_user));
            next = data["@odata.nextLink"].as_str().map(String::from);
        }
        Ok(members)
    }
}
#[async_trait]
impl ProviderTrait for MicrosoftProvider {
    fn get_provider(&self) -> &Provider {
        &self.provider
    }
    async fn get_auth_url(&self, state: &str, code_challenge: Option<&str>) -> Result<String, String> {
        let mut auth_url = format!(
            "{}/authorize?client_id={}&redirect_uri={}&response_type=code&response_mode=query&scope={}&state={}&prompt=select_account",
            authority(&self.tenant),
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(&scopes(self.admin)),
            state
        );
        if let Some(challenge) = code_challenge {
            auth_url.push_str(&format!("&code_challenge={}&code_challenge_method=S256", challenge));
        }
        Ok(auth_url)
    }
    async fn exchange_code(&self, code: &str, code_verifier: Option<&str>) -> Result<ProviderAuth, String> {
        let scope = scopes(self.admin);
        let mut params = vec![
            ("client_id", self.client_id.as_str()),
            ("code", code),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("scope", scope.as_str()),
        ];
        if !self.client_secret.is_empty() {
            params.push(("client_secret", self.client_secret.as_str()));
        }
        if let Some(verifier) = code_verifier {
            params.push(("code_verifier", verifier));
        }
        self.token_call(&params, None).await
    }
    async fn identity(&self, auth: &ProviderAuth) -> Result<ConnectionIdentity, String> {
        let organization = self.get_json(auth, &format!("{}/organization", self.api)).await?;
        let org = organization["value"].as_array().and_then(|v| v.first()).cloned().unwrap_or(Value::Null);
        let tenant_id = org["id"].as_str().map(String::from);
        let me = self.get_current_user(auth).await?;
        let name = org["displayName"]
            .as_str()
            .map(String::from)
            .or_else(|| me.email.clone())
            .unwrap_or_else(|| "Microsoft 365".to_string());
        let namespace = tenant_id.clone().unwrap_or_else(|| me.email.clone().unwrap_or_else(|| me.id.clone()));
        Ok(ConnectionIdentity {
            namespace,
            name,
            external_id: tenant_id,
            user: Some(me),
            logo_url: None,
        })
    }
    async fn fetch_logo(&self, auth: &ProviderAuth, identity: &ConnectionIdentity) -> Result<Option<(Vec<u8>, String)>, String> {
        let Some(tenant_id) = &identity.external_id else { return Ok(None) };
        for path in [
            "branding/localizations/default/squareLogo",
            "branding/localizations/0/squareLogo",
            "branding/squareLogo",
            "branding/localizations/default/bannerLogo",
            "branding/localizations/0/bannerLogo",
            "branding/bannerLogo",
        ] {
            if let Ok(Some(found)) = self.get_bytes(auth, &format!("{}/organization/{}/{}", self.api, tenant_id, path)).await {
                if !found.0.is_empty() {
                    return Ok(Some(found));
                }
            }
        }
        Ok(None)
    }
    async fn refresh_token(&self, refresh_token: &str) -> Result<ProviderAuth, String> {
        let scope = scopes(self.admin);
        let mut params = vec![
            ("client_id", self.client_id.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
            ("scope", scope.as_str()),
        ];
        if !self.client_secret.is_empty() {
            params.push(("client_secret", self.client_secret.as_str()));
        }
        self.token_call(&params, Some(refresh_token)).await
    }
    async fn validate_tokens(&self, auth: &ProviderAuth) -> Result<bool, String> {
        if auth.expires_at < Utc::now() + Duration::minutes(5) {
            return Ok(false);
        }
        let response = crate::providers::http::send(self.client.get(format!("{}/me", self.api)).bearer_auth(&auth.access_token)).await?;
        Ok(response.status().is_success())
    }
    async fn sync_conversations(
        &self,
        auth: &ProviderAuth,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<ResponseConversation>, String> {
        let api = self.api.clone();
        let me = self.get_current_user(auth).await.ok();
        let me_id = me.as_ref().map(|u| u.id.clone());
        let floor = since.unwrap_or_else(|| Utc::now() - Duration::days(30));
        let mut conversations = Vec::new();

        let mut next = Some(format!("{}/chats?$top=50&$expand=members", api));
        while let Some(url) = next.take() {
            let data = self.get_json(auth, &url).await?;
            for chat in data["value"].as_array().cloned().unwrap_or_default() {
                if let Some(conversation) = Self::chat_conversation(&chat, me_id.as_deref(), floor) {
                    conversations.push(conversation);
                }
            }
            next = data["@odata.nextLink"].as_str().map(String::from);
        }

        let teams = if self.admin { self.get_json(auth, &format!("{}/me/joinedTeams", api)).await? } else { Value::Null };
        for team in teams["value"].as_array().cloned().unwrap_or_default() {
            let team_id = team["id"].as_str().unwrap_or("").to_string();
            let team_name = team["displayName"].as_str().unwrap_or("Team").to_string();
            let mut next = Some(format!("{}/teams/{}/channels", api, team_id));
            while let Some(url) = next.take() {
                let Ok(channels) = self.get_json(auth, &url).await else { break };
                for channel in channels["value"].as_array().cloned().unwrap_or_default() {
                    conversations.push(Self::channel_conversation(&team_id, &team_name, &channel, floor));
                }
                next = channels["@odata.nextLink"].as_str().map(String::from);
            }
        }

        let mut threads: HashMap<String, ResponseConversation> = HashMap::new();
        for folder in MAIL_FOLDERS {
            let mut next = Some(format!(
                "{}/me/mailFolders/{}/messages?$top=50&$orderby=receivedDateTime desc&$filter=receivedDateTime ge {}&$select=id,conversationId,subject,from,toRecipients,ccRecipients,receivedDateTime,isRead",
                api,
                folder,
                floor.format("%Y-%m-%dT%H:%M:%SZ")
            ));
            while let Some(url) = next.take() {
                let data = match self.get_json(auth, &url).await {
                    Ok(data) => data,
                    Err(e) if folder != "inbox" && e.contains("404") => break,
                    Err(e) => return Err(e),
                };
                for mail in data["value"].as_array().cloned().unwrap_or_default() {
                    let Some(conversation_id) = mail["conversationId"].as_str() else { continue };
                    let received = Self::parse_time(mail["receivedDateTime"].as_str());
                    let subject = mail["subject"].as_str().unwrap_or("(no subject)").to_string();
                    let mut people = vec![Self::email_user(&mail["from"]["emailAddress"])];
                    people.extend(mail["toRecipients"].as_array().cloned().unwrap_or_default().iter().map(|r| Self::email_user(&r["emailAddress"])));
                    people.extend(mail["ccRecipients"].as_array().cloned().unwrap_or_default().iter().map(|r| Self::email_user(&r["emailAddress"])));
                    let entry = threads.entry(conversation_id.to_string()).or_insert_with(|| ResponseConversation {
                        external_id: format!("mail:{}", conversation_id),
                        name: subject.clone(),
                        conversation_type: ConversationType::Email,
                        members: Vec::new(),
                        last_activity: received,
                        metadata: HashMap::new(),
                    });
                    let folders = entry.metadata.entry("folders".to_string()).or_insert_with(|| Value::Array(Vec::new()));
                    if let Some(list) = folders.as_array_mut() {
                        if !list.iter().any(|f| f == folder) {
                            list.push(Value::String(folder.to_string()));
                        }
                    }
                    if received > entry.last_activity {
                        entry.last_activity = received;
                        entry.name = subject;
                    }
                    for person in people {
                        let known = entry.members.iter().any(|m| m.email.is_some() && m.email == person.email);
                        if !known && person.email.is_some() {
                            entry.members.push(person);
                        }
                    }
                }
                next = data["@odata.nextLink"].as_str().map(String::from);
            }
        }
        conversations.extend(threads.into_values());
        Ok(conversations)
    }
    async fn sync_messages(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        since: Option<DateTime<Utc>>,
        limit: Option<u32>,
    ) -> Result<Vec<ResponseMessage>, String> {
        let api = self.api.clone();
        let floor = since.unwrap_or_else(|| Utc::now() - Duration::days(30));
        let cap = limit.unwrap_or(500) as usize;
        let mut messages = Vec::new();
        let mut cache: HashMap<String, ResponseUser> = HashMap::new();

        if let Some(conversation_id) = conversation_external_id.strip_prefix("mail:") {
            let mut next = Some(format!(
                "{}/me/messages?$top=50&$orderby=receivedDateTime desc&$filter=conversationId eq '{}'&$select=id,subject,from,toRecipients,ccRecipients,receivedDateTime,isRead,hasAttachments,body,conversationIndex,internetMessageId,webLink,parentFolderId,categories&$expand=attachments($select=id,name,contentType,size,isInline)",
                api,
                conversation_id.replace('\'', "''")
            ));
            while let Some(url) = next.take() {
                let data = self.get_json(auth, &url).await?;
                for mail in data["value"].as_array().cloned().unwrap_or_default() {
                    messages.push(Self::mail_message(&mail, conversation_external_id));
                    if messages.len() >= cap {
                        return Ok(messages);
                    }
                }
                next = data["@odata.nextLink"].as_str().map(String::from);
            }
            return Ok(messages);
        }

        let base = Self::messages_url(&api, conversation_external_id, since);
        let mut next = Some(base);
        'pages: while let Some(url) = next.take() {
            let data = self.get_json(auth, &url).await?;
            for msg in data["value"].as_array().cloned().unwrap_or_default() {
                let changed = Self::parse_time(msg["lastModifiedDateTime"].as_str().or_else(|| msg["createdDateTime"].as_str()));
                if changed < floor {
                    break 'pages;
                }
                if let Some(message) = self.chat_message(auth, &msg, conversation_external_id, None, &mut cache).await {
                    messages.push(message);
                }
                let parent = msg["id"].as_str().map(String::from);
                let mut replies = msg["replies"].as_array().cloned().unwrap_or_default();
                let mut more = msg["replies@odata.nextLink"].as_str().map(String::from);
                while let Some(link) = more.take() {
                    let page = self.get_json(auth, &link).await?;
                    replies.extend(page["value"].as_array().cloned().unwrap_or_default());
                    more = page["@odata.nextLink"].as_str().map(String::from);
                }
                for reply in replies {
                    if let Some(message) = self.chat_message(auth, &reply, conversation_external_id, parent.clone(), &mut cache).await {
                        messages.push(message);
                    }
                }
                if messages.len() >= cap {
                    break 'pages;
                }
            }
            next = data["@odata.nextLink"].as_str().map(String::from);
        }
        if Self::split_channel(conversation_external_id).is_none() {
            if let Ok(chat) = self.get_json(auth, &format!("{}/chats/{}?$select=viewpoint", api, conversation_external_id)).await {
                if let Some(read_at) = chat["viewpoint"]["lastMessageReadDateTime"].as_str().map(|at| Self::parse_time(Some(at))) {
                    for message in messages.iter_mut() {
                        message.is_read = message.timestamp <= read_at;
                    }
                }
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
        let api = self.api.clone();
        let me = self.get_current_user(auth).await?;
        let is_mail = conversation_external_id.starts_with("mail:");
        let (content_type, content, mentions) = match draft.content_type.as_str() {
            "html" => ("html", draft.content.clone(), Vec::new()),
            "markdown" if is_mail => ("html", draft.html.clone().unwrap_or_else(|| crate::content::markdown::html(&draft.content)), Vec::new()),
            "markdown" => {
                let (html, mentions) = crate::content::markdown::teams(&draft.content);
                ("html", html, mentions)
            }
            _ => ("text", draft.content.clone(), Vec::new()),
        };
        let stored_type = if draft.content_type == "markdown" { "markdown" } else { content_type };
        let body = serde_json::json!({ "contentType": content_type, "content": content });
        let sent_attachments: Vec<ResponseAttachment> = draft
            .attachments
            .iter()
            .enumerate()
            .map(|(i, a)| ResponseAttachment { external_id: format!("{}", i + 1), name: a.name.clone(), content_type: a.mime_type.clone(), size: a.bytes.len() as u64, url: None, content_id: None })
            .collect();

        if is_mail {
            let recipients = |addresses: &Vec<String>| -> Vec<Value> {
                addresses.iter().map(|a| serde_json::json!({ "emailAddress": { "address": a } })).collect()
            };
            let native_forward = Self::mail_forward_of(draft);
            let sent_text = if native_forward.is_some() { draft.content.clone() } else { draft.mail_text() };
            let body = if native_forward.is_some() || draft.forwarded.is_empty() {
                body
            } else {
                match draft.content_type.as_str() {
                    "html" => serde_json::json!({ "contentType": "html", "content": sent_text }),
                    "markdown" => serde_json::json!({ "contentType": "html", "content": crate::content::markdown::html(&sent_text) }),
                    _ => serde_json::json!({ "contentType": "text", "content": sent_text }),
                }
            };
            let draft_id = match (&draft.in_reply_to, native_forward) {
                (None, Some(original)) => {
                    if draft.to.is_empty() {
                        return Err("A forward needs at least one recipient".to_string());
                    }
                    let forward = self.post_json(auth, &format!("{}/me/messages/{}/createForward", api, original.external_id), &Self::forward_request(&draft.to, &content)).await?;
                    forward["id"].as_str().ok_or_else(|| "Outlook did not return the forward draft".to_string())?.to_string()
                }
                (Some(original), _) => {
                    let reply = self.post_json(auth, &format!("{}/me/messages/{}/createReply", api, original), &serde_json::json!({})).await?;
                    let id = reply["id"].as_str().ok_or_else(|| "Outlook did not return the reply draft".to_string())?.to_string();
                    let mut patch = serde_json::json!({ "body": body });
                    if !draft.to.is_empty() {
                        patch["toRecipients"] = Value::Array(recipients(&draft.to));
                    }
                    if !draft.cc.is_empty() {
                        patch["ccRecipients"] = Value::Array(recipients(&draft.cc));
                    }
                    if let Some(subject) = &draft.subject {
                        patch["subject"] = Value::String(subject.clone());
                    }
                    self.patch_json(auth, &format!("{}/me/messages/{}", api, id), &patch).await?;
                    id
                }
                (None, None) => {
                    if draft.to.is_empty() {
                        return Err("A new mail needs at least one recipient".to_string());
                    }
                    let message = serde_json::json!({
                        "subject": draft.subject.clone().unwrap_or_default(),
                        "body": body,
                        "toRecipients": recipients(&draft.to),
                        "ccRecipients": recipients(&draft.cc),
                        "bccRecipients": recipients(&draft.bcc),
                    });
                    let created = self.post_json(auth, &format!("{}/me/messages", api), &message).await?;
                    created["id"].as_str().ok_or_else(|| "Outlook did not return the draft".to_string())?.to_string()
                }
            };
            for attachment in draft.attachments.iter().filter(|a| !native_forward.is_some_and(|f| f.attachments.contains(&a.name))) {
                let file = serde_json::json!({
                    "@odata.type": "#microsoft.graph.fileAttachment",
                    "name": attachment.name,
                    "contentType": attachment.mime_type,
                    "contentBytes": STANDARD.encode(&attachment.bytes),
                });
                self.post_json(auth, &format!("{}/me/messages/{}/attachments", api, draft_id), &file).await?;
            }
            self.post_json(auth, &format!("{}/me/messages/{}/send", api, draft_id), &serde_json::json!({})).await?;
            return Ok(ResponseMessage {
                external_id: draft_id,
                conversation_external_id: conversation_external_id.to_string(),
                sender: me,
                content: sent_text,
                content_type: stored_type.to_string(),
                timestamp: Utc::now(),
                r#type: MessageType::Email,
                subject: draft.subject.clone(),
                recipients: draft.to.iter().map(|a| ResponseUser { id: a.clone(), email: Some(a.clone()), ..Default::default() }).collect(),
                cc: draft.cc.iter().map(|a| ResponseUser { id: a.clone(), email: Some(a.clone()), ..Default::default() }).collect(),
                reply_to_id: draft.in_reply_to.clone(),
                attachments: sent_attachments,
                ..Default::default()
            });
        }

        let url = match (Self::split_channel(conversation_external_id), &draft.thread_external_id) {
            (Some((team_id, channel_id)), Some(parent)) => format!("{}/teams/{}/channels/{}/messages/{}/replies", api, team_id, channel_id, parent),
            (Some((team_id, channel_id)), None) => format!("{}/teams/{}/channels/{}/messages", api, team_id, channel_id),
            (None, _) => format!("{}/chats/{}/messages", api, conversation_external_id),
        };
        let mut hosted = Vec::new();
        let mut images = String::new();
        let mut files = Vec::new();
        for (i, attachment) in draft.attachments.iter().enumerate() {
            if !attachment.mime_type.starts_with("image/") {
                files.push(self.post_reference(auth, attachment).await?);
                continue;
            }
            let temporary_id = format!("{}", i + 1);
            hosted.push(serde_json::json!({
                "@microsoft.graph.temporaryId": temporary_id,
                "contentBytes": STANDARD.encode(&attachment.bytes),
                "contentType": attachment.mime_type,
            }));
            images.push_str(&format!("<div><img src=\"../hostedContents/{}/$value\" alt=\"{}\"></div>", temporary_id, html_escape(&attachment.name)));
        }
        let mut payload = Self::chat_payload(content_type, &content, draft.quote.as_ref(), &images, hosted, files);
        if !mentions.is_empty() {
            payload["mentions"] = Value::Array(mentions.iter().enumerate().map(|(i, m)| serde_json::json!({ "id": i, "mentionText": m.name, "mentioned": { "user": { "id": m.id, "displayName": m.name, "userIdentityType": "aadUser" } } })).collect());
        }
        let data = self.post_json(auth, &url, &payload).await?;
        let mut metadata = HashMap::new();
        if let Some(link) = data["webUrl"].as_str().map(String::from).or_else(|| Self::chat_web_url(conversation_external_id, data["id"].as_str().unwrap_or(""))) {
            metadata.insert("web_url".to_string(), Value::String(link));
        }
        Ok(ResponseMessage {
            external_id: data["id"].as_str().unwrap_or("").to_string(),
            conversation_external_id: conversation_external_id.to_string(),
            sender: me,
            content: draft.content.clone(),
            content_type: stored_type.to_string(),
            timestamp: Self::parse_time(data["createdDateTime"].as_str()),
            r#type: MessageType::Chat,
            metadata,
            thread_id: draft.thread_external_id.clone(),
            reply_to_id: draft.in_reply_to.clone(),
            attachments: sent_attachments,
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
        if conversation_external_id.starts_with("mail:") {
            return Err("Mail has no reactions".to_string());
        }
        let reaction_type = crate::content::emoji::teams_reaction(reaction)?;
        let verb = if active { "setReaction" } else { "unsetReaction" };
        let url = format!("{}/{}", self.message_url(conversation_external_id, message_external_id), verb);
        self.post_json(auth, &url, &serde_json::json!({ "reactionType": reaction_type })).await.map(|_| ())
    }
    async fn delete_message(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        message_external_id: &str,
    ) -> Result<(), String> {
        if conversation_external_id.starts_with("mail:") {
            return self.delete(auth, &format!("{}/me/messages/{}", self.api, message_external_id)).await;
        }
        let url = format!("{}/softDelete", self.message_url(conversation_external_id, message_external_id));
        self.post_json(auth, &url, &serde_json::json!({})).await.map(|_| ())
    }
    async fn mark_read(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        message_external_id: &str,
        read: bool,
    ) -> Result<(), String> {
        if conversation_external_id.starts_with("mail:") {
            return self.patch_json(auth, &format!("{}/me/messages/{}", self.api, message_external_id), &serde_json::json!({ "isRead": read })).await.map(|_| ());
        }
        if Self::split_channel(conversation_external_id).is_some() {
            return Ok(());
        }
        let (me, tenant) = self.me_and_tenant(auth).await?;
        let user = serde_json::json!({ "id": me, "tenantId": tenant });
        if read {
            return self.post_json(auth, &format!("{}/chats/{}/markChatReadForUser", self.api, conversation_external_id), &serde_json::json!({ "user": user })).await.map(|_| ());
        }
        let message = self.get_json(auth, &format!("{}/chats/{}/messages/{}", self.api, conversation_external_id, message_external_id)).await?;
        let before = Self::parse_time(message["createdDateTime"].as_str()) - Duration::seconds(1);
        self.post_json(auth, &format!("{}/chats/{}/markChatUnreadForUser", self.api, conversation_external_id), &serde_json::json!({ "user": user, "lastMessageReadDateTime": before.to_rfc3339() })).await.map(|_| ())
    }
    async fn edit_message(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        message_external_id: &str,
        content: &str,
    ) -> Result<(), String> {
        if conversation_external_id.starts_with("mail:") {
            return Err("A sent mail cannot be edited".to_string());
        }
        let url = self.message_url(conversation_external_id, message_external_id);
        let html = crate::content::emoji::unicode_html(content);
        self.patch_json(auth, &url, &serde_json::json!({ "body": { "contentType": "html", "content": html } })).await.map(|_| ())
    }
    async fn get_presence(
        &self,
        auth: &ProviderAuth,
        user_id: &str,
    ) -> Result<String, String> {
        let data = self.get_json(auth, &format!("{}/users/{}/presence", self.api, user_id)).await?;
        Ok(data["availability"].as_str().unwrap_or("Unknown").to_string())
    }
    async fn get_calendars(&self, auth: &ProviderAuth) -> Result<Vec<RemoteCalendar>, String> {
        let data = self.get_json(auth, &format!("{}/me/calendars?$top=100", self.api)).await?;
        Ok(data["value"].as_array().cloned().unwrap_or_default().iter().map(Self::graph_calendar).collect())
    }
    async fn get_calendar_events(&self, auth: &ProviderAuth, calendar_external_id: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<RemoteEvent>, String> {
        let mut url = Some(format!(
            "{}/me/calendars/{}/calendarView?startDateTime={}&endDateTime={}&$top=200",
            self.api,
            urlencoding::encode(calendar_external_id),
            urlencoding::encode(&from.to_rfc3339()),
            urlencoding::encode(&to.to_rfc3339())
        ));
        let mut events = Vec::new();
        let mut pages = 0;
        while let Some(next) = url.take() {
            let response = crate::providers::http::send(self.client.get(&next).bearer_auth(&auth.access_token).header("Prefer", "outlook.timezone=\"UTC\"")).await?;
            let data = Self::read_json(response).await?;
            events.extend(data["value"].as_array().cloned().unwrap_or_default().iter().filter_map(Self::graph_event));
            pages += 1;
            url = if pages < 20 { data["@odata.nextLink"].as_str().map(String::from) } else { None };
        }
        Ok(events)
    }
    async fn post_calendar_event(&self, auth: &ProviderAuth, calendar_external_id: &str, event: &NewRemoteEvent) -> Result<RemoteEvent, String> {
        let url = format!("{}/me/calendars/{}/events", self.api, urlencoding::encode(calendar_external_id));
        let created = self.post_json(auth, &url, &Self::graph_event_body(event)).await?;
        Self::graph_event(&created).ok_or_else(|| "Graph returned an event it could not describe".to_string())
    }
    async fn put_calendar_response(&self, auth: &ProviderAuth, _calendar_external_id: &str, event_external_id: &str, response: &str) -> Result<RemoteEvent, String> {
        let verb = Self::response_verb(response)?;
        let event = format!("{}/me/events/{}", self.api, urlencoding::encode(event_external_id));
        self.post_json(auth, &format!("{}/{}", event, verb), &serde_json::json!({ "sendResponse": true })).await?;
        let response = crate::providers::http::send(self.client.get(&event).bearer_auth(&auth.access_token).header("Prefer", "outlook.timezone=\"UTC\"")).await?;
        let answered = Self::read_json(response).await?;
        Self::graph_event(&answered).ok_or_else(|| "Graph returned an event it could not describe".to_string())
    }
    async fn put_member(&self, auth: &ProviderAuth, conversation_external_id: &str, user_external_id: &str, member: bool) -> Result<bool, String> {
        if conversation_external_id.starts_with("mail:") || Self::split_channel(conversation_external_id).is_some() {
            return Ok(false);
        }
        if member {
            self.post_json(auth, &format!("{}/chats/{}/members", self.api, conversation_external_id), &Self::member_body(&self.api, user_external_id)).await?;
            return Ok(true);
        }
        let data = self.get_json(auth, &format!("{}/chats/{}/members", self.api, conversation_external_id)).await?;
        let Some(membership) = data["value"].as_array().and_then(|all| all.iter().find(|m| m["userId"].as_str() == Some(user_external_id))).and_then(|m| m["id"].as_str()) else { return Ok(false) };
        self.delete(auth, &format!("{}/chats/{}/members/{}", self.api, conversation_external_id, membership)).await?;
        Ok(true)
    }
    async fn put_conversation_name(&self, auth: &ProviderAuth, conversation_external_id: &str, name: &str) -> Result<bool, String> {
        if conversation_external_id.starts_with("mail:") || Self::split_channel(conversation_external_id).is_some() {
            return Ok(false);
        }
        self.patch_json(auth, &format!("{}/chats/{}", self.api, conversation_external_id), &serde_json::json!({ "topic": name })).await?;
        Ok(true)
    }
    async fn put_pin(&self, auth: &ProviderAuth, conversation_external_id: &str, message_external_id: &str, pinned: bool) -> Result<bool, String> {
        if conversation_external_id.starts_with("mail:") || Self::split_channel(conversation_external_id).is_some() {
            return Ok(false);
        }
        let pins = format!("{}/chats/{}/pinnedMessages", self.api, conversation_external_id);
        if pinned {
            let bind = format!("{}/chats/{}/messages/{}", GRAPH, conversation_external_id, message_external_id);
            self.post_json(auth, &pins, &serde_json::json!({ "message@odata.bind": bind })).await?;
            return Ok(true);
        }
        let data = self.get_json(auth, &format!("{}?$expand=message", pins)).await?;
        let Some(pin) = Self::pinned(&data).into_iter().find(|(_, message)| message == message_external_id).map(|(pin, _)| pin) else { return Ok(false) };
        self.delete(auth, &format!("{}/{}", pins, pin)).await?;
        Ok(true)
    }
    async fn get_pins(&self, auth: &ProviderAuth, conversation_external_id: &str) -> Result<Option<Vec<String>>, String> {
        if conversation_external_id.starts_with("mail:") || Self::split_channel(conversation_external_id).is_some() {
            return Ok(None);
        }
        let data = self.get_json(auth, &format!("{}/chats/{}/pinnedMessages?$expand=message", self.api, conversation_external_id)).await?;
        Ok(Some(Self::pinned(&data).into_iter().map(|(_, message)| message).collect()))
    }
    async fn get_people(&self, auth: &ProviderAuth) -> Result<Vec<ResponseUser>, String> {
        let data = self.get_json(auth, &format!("{}/me/people?$top=100", self.api)).await?;
        let mut people: Vec<ResponseUser> = data["value"].as_array().cloned().unwrap_or_default().iter().filter_map(Self::people_user).collect();
        for person in people.iter_mut().filter(|p| !p.id.contains('@')) {
            if let Ok(Some((bytes, mime))) = self.get_bytes(auth, &format!("{}/users/{}/photos/64x64/$value", self.api, person.id)).await {
                person.avatar_url = Some(format!("data:{};base64,{}", mime, STANDARD.encode(bytes)));
            }
        }
        Ok(people)
    }
    async fn get_delta(&self, auth: &ProviderAuth, cursors: &HashMap<String, String>, since: DateTime<Utc>) -> Result<Delta, String> {
        let mut out = Delta { cursors: cursors.clone(), complete: true, ..Default::default() };
        for folder in MAIL_FOLDERS {
            let key = format!("graph:mail:{}", folder);
            if !cursors.contains_key(&key) {
                out.complete = false;
            }
            let mut next = Some(cursors.get(&key).cloned().unwrap_or_else(|| format!(
                "{}/me/mailFolders/{}/messages/delta?$select=isRead,conversationId,flag&$filter=receivedDateTime ge {}",
                self.api,
                folder,
                since.format("%Y-%m-%dT%H:%M:%SZ")
            )));
            while let Some(url) = next.take() {
                let data = match self.get_json(auth, &url).await {
                    Ok(data) => data,
                    Err(e) if e.contains("410") || e.contains("404") => {
                        out.cursors.remove(&key);
                        out.complete = false;
                        break;
                    }
                    Err(e) => return Err(e),
                };
                out.changes.extend(Self::mail_changes(&data));
                next = data["@odata.nextLink"].as_str().map(String::from);
                if let Some(link) = data["@odata.deltaLink"].as_str() {
                    out.cursors.insert(key.clone(), link.to_string());
                }
            }
        }
        for change in &out.changes {
            if let Some(conversation) = change.conversation_external_id.clone().filter(|c| !out.conversations.contains(c)) {
                out.conversations.push(conversation);
            }
        }
        let seen = cursors.get(CHATS_CURSOR).and_then(|at| DateTime::parse_from_rfc3339(at).ok()).map(|at| at.with_timezone(&Utc));
        if seen.is_none() {
            out.complete = false;
        }
        let mut newest = seen.unwrap_or(since);
        let mut next = Some(format!("{}/chats?$top=50&$expand=lastMessagePreview&$orderby=lastMessagePreview/createdDateTime desc", self.api));
        'chats: while let Some(url) = next.take() {
            let data = match self.get_json(auth, &url).await {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Teams chats were not checked for changes: {}", e);
                    out.complete = false;
                    break;
                }
            };
            for chat in data["value"].as_array().cloned().unwrap_or_default() {
                let at = Self::parse_time(chat["lastMessagePreview"]["createdDateTime"].as_str().or_else(|| chat["lastUpdatedDateTime"].as_str()));
                if seen.is_some_and(|s| at <= s) {
                    break 'chats;
                }
                if let Some(id) = chat["id"].as_str().filter(|id| !out.conversations.iter().any(|c| c == id)) {
                    out.conversations.push(id.to_string());
                }
                newest = newest.max(at);
            }
            next = if seen.is_some() { data["@odata.nextLink"].as_str().map(String::from) } else { None };
        }
        out.cursors.insert(CHATS_CURSOR.to_string(), newest.to_rfc3339());
        if self.admin {
            let teams = self.get_json(auth, &format!("{}/me/joinedTeams", self.api)).await?;
            for team in teams["value"].as_array().cloned().unwrap_or_default() {
                let team_id = team["id"].as_str().unwrap_or("").to_string();
                let channels = self.get_json(auth, &format!("{}/teams/{}/channels", self.api, team_id)).await?;
                for channel in channels["value"].as_array().cloned().unwrap_or_default() {
                    let channel_id = channel["id"].as_str().unwrap_or("").to_string();
                    let conversation = format!("channel:{}:{}", team_id, channel_id);
                    let key = format!("graph:{}", conversation);
                    if !cursors.contains_key(&key) {
                        out.complete = false;
                    }
                    let mut next = Some(cursors.get(&key).cloned().unwrap_or_else(|| format!(
                        "{}/teams/{}/channels/{}/messages/delta?$filter=lastModifiedDateTime gt {}",
                        self.api,
                        team_id,
                        channel_id,
                        since.format("%Y-%m-%dT%H:%M:%S%.3fZ")
                    )));
                    while let Some(url) = next.take() {
                        let data = match self.get_json(auth, &url).await {
                            Ok(data) => data,
                            Err(e) if e.contains("410") || e.contains("404") || e.contains("403") => {
                                out.cursors.remove(&key);
                                out.complete = false;
                                break;
                            }
                            Err(e) => return Err(e),
                        };
                        if data["value"].as_array().is_some_and(|v| !v.is_empty()) && !out.conversations.contains(&conversation) {
                            out.conversations.push(conversation.clone());
                        }
                        next = data["@odata.nextLink"].as_str().map(String::from);
                        if let Some(link) = data["@odata.deltaLink"].as_str() {
                            out.cursors.insert(key.clone(), link.to_string());
                        }
                    }
                }
            }
        }
        Ok(out)
    }
    async fn get_attachment(&self, auth: &ProviderAuth, conversation_external_id: &str, message_external_id: &str, attachment: &ResponseAttachment) -> Result<Option<Vec<u8>>, String> {
        let url = match attachment.url.as_deref() {
            Some(url) if url.contains("/hostedContents/") => url.to_string(),
            _ if conversation_external_id.starts_with("mail:") => format!("{}/me/messages/{}/attachments/{}/$value", self.api, message_external_id, attachment.external_id),
            Some(url) if url.starts_with("https://") => format!("{}/shares/u!{}/driveItem/content", self.api, URL_SAFE_NO_PAD.encode(url)),
            _ => return Ok(None),
        };
        Ok(self.get_bytes(auth, &url).await?.map(|(bytes, _)| bytes))
    }
    async fn get_message_link(&self, auth: &ProviderAuth, conversation_external_id: &str, message_external_id: &str) -> Result<Option<String>, String> {
        if conversation_external_id.starts_with("mail:") {
            let data = self.get_json(auth, &format!("{}/me/messages/{}?$select=webLink", self.api, message_external_id)).await?;
            return Ok(data["webLink"].as_str().map(String::from));
        }
        if Self::split_channel(conversation_external_id).is_some() {
            let data = self.get_json(auth, &self.message_url(conversation_external_id, message_external_id)).await?;
            return Ok(data["webUrl"].as_str().map(String::from));
        }
        Ok(Self::chat_web_url(conversation_external_id, message_external_id))
    }
    async fn put_status(&self, auth: &ProviderAuth, text: &str, emoji: Option<&str>, until: Option<DateTime<Utc>>) -> Result<bool, String> {
        let glyph = emoji.map(|e| crate::content::emoji::unicode_text(e)).filter(|e| !e.trim().is_empty());
        let message = match glyph {
            Some(glyph) => format!("{} {}", glyph, text).trim().to_string(),
            None => text.trim().to_string(),
        };
        let mut status = serde_json::json!({ "message": { "content": message, "contentType": "text" } });
        if let Some(until) = until {
            status["expiryDateTime"] = serde_json::json!({ "dateTime": until.format("%Y-%m-%dT%H:%M:%S").to_string(), "timeZone": "UTC" });
        }
        self.post_json(auth, &format!("{}/me/presence/setStatusMessage", self.api), &serde_json::json!({ "statusMessage": status })).await?;
        Ok(true)
    }
    async fn get_presences(&self, auth: &ProviderAuth, user_ids: &[String]) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        for batch in user_ids.iter().filter(|id| !id.contains('@')).collect::<Vec<_>>().chunks(650) {
            let data = self.post_json(auth, &format!("{}/communications/getPresencesByUserId", self.api), &serde_json::json!({ "ids": batch })).await?;
            out.extend(data["value"].as_array().cloned().unwrap_or_default().iter().filter_map(|p| Some((p["id"].as_str()?.to_string(), p["availability"].as_str().unwrap_or("Unknown").to_string()))));
        }
        Ok(out)
    }
    async fn get_search(&self, auth: &ProviderAuth, search: &RemoteSearch) -> Result<Vec<ResponseMessage>, String> {
        let mut terms = vec![search.text.trim().replace('"', "")];
        if let Some(from) = &search.from {
            terms.push(format!("from:{}", from));
        }
        if let Some(after) = search.after {
            terms.push(format!("received>={}", after.format("%Y-%m-%d")));
        }
        if let Some(before) = search.before {
            terms.push(format!("received<={}", before.format("%Y-%m-%d")));
        }
        let query = terms.into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
        let data = self
            .get_json(auth, &format!("{}/me/messages?$search=\"{}\"&$top={}&$select=id,conversationId,subject,from,toRecipients,ccRecipients,receivedDateTime,isRead,hasAttachments,body,internetMessageId,webLink,categories", self.api, urlencoding::encode(&query), search.limit.clamp(1, 250)))
            .await?;
        Ok(data["value"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter(|mail| search.conversation_external_id.as_deref().is_none_or(|c| Some(c) == mail["conversationId"].as_str().map(|id| format!("mail:{}", id)).as_deref()))
            .map(|mail| Self::mail_message(mail, &format!("mail:{}", mail["conversationId"].as_str().unwrap_or(""))))
            .collect())
    }
    async fn put_remote_draft(&self, auth: &ProviderAuth, conversation_external_id: &str, draft: &OutgoingMessage, existing_external_id: Option<&str>) -> Result<Option<String>, String> {
        if !conversation_external_id.starts_with("mail:") && draft.to.is_empty() {
            return Ok(None);
        }
        let recipients = |list: &[String]| -> Vec<Value> { list.iter().map(|a| serde_json::json!({ "emailAddress": { "address": a } })).collect() };
        let html = draft.mail_html().unwrap_or_else(|| crate::content::markdown::html(&draft.mail_text()));
        let body = serde_json::json!({
            "subject": draft.subject.clone().unwrap_or_default(),
            "body": { "contentType": "html", "content": html },
            "toRecipients": recipients(&draft.to),
            "ccRecipients": recipients(&draft.cc),
            "bccRecipients": recipients(&draft.bcc),
        });
        let saved = match existing_external_id {
            Some(id) => self.patch_json(auth, &format!("{}/me/messages/{}", self.api, id), &body).await?,
            None => self.post_json(auth, &format!("{}/me/messages", self.api), &body).await?,
        };
        Ok(saved["id"].as_str().map(String::from).or(existing_external_id.map(String::from)))
    }
    async fn delete_remote_draft(&self, auth: &ProviderAuth, draft_external_id: &str) -> Result<bool, String> {
        self.delete(auth, &format!("{}/me/messages/{}", self.api, draft_external_id)).await?;
        Ok(true)
    }
    async fn put_flag(&self, auth: &ProviderAuth, conversation_external_id: &str, message_external_id: &str, flagged: bool) -> Result<bool, String> {
        if !conversation_external_id.starts_with("mail:") {
            return Ok(false);
        }
        self.patch_json(auth, &format!("{}/me/messages/{}", self.api, message_external_id), &serde_json::json!({ "flag": { "flagStatus": if flagged { "flagged" } else { "notFlagged" } } })).await?;
        Ok(true)
    }
    async fn put_archive(&self, auth: &ProviderAuth, conversation_external_id: &str, message_external_id: &str, archived: bool) -> Result<bool, String> {
        if !conversation_external_id.starts_with("mail:") {
            return Ok(false);
        }
        self.post_json(auth, &format!("{}/me/messages/{}/move", self.api, message_external_id), &serde_json::json!({ "destinationId": if archived { "archive" } else { "inbox" } })).await?;
        Ok(true)
    }
    async fn post_conversation(&self, auth: &ProviderAuth, conversation: &NewRemoteConversation) -> Result<Option<ResponseConversation>, String> {
        if conversation.conversation_type == ConversationType::Channel {
            let Some(team_id) = conversation.parent_external_id.as_deref() else { return Err("Choose the team the channel belongs to".to_string()) };
            let created = self
                .post_json(auth, &format!("{}/teams/{}/channels", self.api, team_id), &serde_json::json!({ "displayName": conversation.name, "membershipType": if conversation.private { "private" } else { "standard" } }))
                .await?;
            return Ok(Some(Self::channel_conversation(team_id, "", &created, Utc::now())));
        }
        let (me, _) = self.me_and_tenant(auth).await?;
        let mut members = vec![Self::member_body(&self.api, &me)];
        members.extend(conversation.members.iter().filter(|m| **m != me).map(|m| Self::member_body(&self.api, m)));
        let mut body = serde_json::json!({ "chatType": if members.len() > 2 { "group" } else { "oneOnOne" }, "members": members });
        if members.len() > 2 && !conversation.name.trim().is_empty() {
            body["topic"] = Value::String(conversation.name.clone());
        }
        let created = self.post_json(auth, &format!("{}/chats", self.api), &body).await?;
        Ok(Self::chat_conversation(&created, Some(&me), Utc::now() - Duration::days(1)).or_else(|| {
            created["id"].as_str().map(|id| ResponseConversation { external_id: id.to_string(), name: conversation.name.clone(), conversation_type: ConversationType::GroupMessage, members: Vec::new(), last_activity: Utc::now(), metadata: HashMap::new() })
        }))
    }
    async fn put_membership(&self, auth: &ProviderAuth, conversation_external_id: &str, joined: bool) -> Result<bool, String> {
        if joined || conversation_external_id.starts_with("mail:") || Self::split_channel(conversation_external_id).is_some() {
            return Ok(false);
        }
        let (me, _) = self.me_and_tenant(auth).await?;
        self.put_member(auth, conversation_external_id, &me, false).await
    }
    async fn put_conversation_topic(&self, auth: &ProviderAuth, conversation_external_id: &str, _topic: Option<&str>, purpose: Option<&str>) -> Result<bool, String> {
        let (Some((team_id, channel_id)), Some(purpose)) = (Self::split_channel(conversation_external_id), purpose) else { return Ok(false) };
        self.patch_json(auth, &format!("{}/teams/{}/channels/{}", self.api, team_id, channel_id), &serde_json::json!({ "description": purpose })).await?;
        Ok(true)
    }
    async fn get_aliases(&self, auth: &ProviderAuth) -> Result<Vec<RemoteAlias>, String> {
        let me = self.get_json(auth, &format!("{}/me?$select=mail,displayName,proxyAddresses,userPrincipalName", self.api)).await?;
        let primary = me["mail"].as_str().or_else(|| me["userPrincipalName"].as_str()).unwrap_or("").to_lowercase();
        let name = me["displayName"].as_str().map(String::from);
        let mut out = vec![RemoteAlias { email: primary.clone(), name: name.clone(), is_default: true, signature: None }];
        for address in me["proxyAddresses"].as_array().cloned().unwrap_or_default().iter().filter_map(|a| a.as_str()) {
            let Some(email) = address.strip_prefix("smtp:").or_else(|| address.strip_prefix("SMTP:")).map(str::to_lowercase) else { continue };
            if !out.iter().any(|a| a.email == email) {
                out.push(RemoteAlias { email, name: name.clone(), is_default: false, signature: None });
            }
        }
        Ok(out.into_iter().filter(|a| !a.email.is_empty()).collect())
    }
    async fn get_auto_reply(&self, auth: &ProviderAuth) -> Result<Option<RemoteAutoReply>, String> {
        let data = self.get_json(auth, &format!("{}/me/mailboxSettings/automaticRepliesSetting", self.api)).await?;
        let status = data["status"].as_str().unwrap_or("disabled");
        let at = |key: &str| data[key]["dateTime"].as_str().map(|t| Self::parse_time(Some(&format!("{}Z", t.trim_end_matches('Z')))));
        Ok(Some(RemoteAutoReply {
            enabled: status != "disabled",
            subject: None,
            body: data["externalReplyMessage"].as_str().or_else(|| data["internalReplyMessage"].as_str()).unwrap_or("").to_string(),
            starts_at: (status == "scheduled").then(|| at("scheduledStartDateTime")).flatten(),
            ends_at: (status == "scheduled").then(|| at("scheduledEndDateTime")).flatten(),
            external_only: data["externalAudience"].as_str() == Some("none"),
        }))
    }
    async fn put_auto_reply(&self, auth: &ProviderAuth, reply: &RemoteAutoReply) -> Result<bool, String> {
        let html = crate::content::markdown::html(&reply.body);
        let mut setting = serde_json::json!({
            "status": match (reply.enabled, reply.starts_at.is_some() || reply.ends_at.is_some()) { (false, _) => "disabled", (true, true) => "scheduled", (true, false) => "alwaysEnabled" },
            "externalAudience": if reply.external_only { "none" } else { "all" },
            "internalReplyMessage": html,
            "externalReplyMessage": html,
        });
        if let Some(start) = reply.starts_at {
            setting["scheduledStartDateTime"] = serde_json::json!({ "dateTime": start.format("%Y-%m-%dT%H:%M:%S").to_string(), "timeZone": "UTC" });
        }
        if let Some(end) = reply.ends_at {
            setting["scheduledEndDateTime"] = serde_json::json!({ "dateTime": end.format("%Y-%m-%dT%H:%M:%S").to_string(), "timeZone": "UTC" });
        }
        self.patch_json(auth, &format!("{}/me/mailboxSettings", self.api), &serde_json::json!({ "automaticRepliesSetting": setting })).await?;
        Ok(true)
    }
}
impl MicrosoftProvider {
    fn map_conversation_type(provider_type: &str) -> ConversationType {
        match provider_type {
            "oneOnOne" => ConversationType::DirectMessage,
            "group" => ConversationType::GroupMessage,
            "meeting" => ConversationType::GroupMessage,
            _ => ConversationType::Unknown,
        }
    }

    fn chat_conversation(chat: &Value, me_id: Option<&str>, floor: DateTime<Utc>) -> Option<ResponseConversation> {
        let external_id = chat["id"].as_str().unwrap_or("").to_string();
        let last_updated = Self::parse_time(chat["lastUpdatedDateTime"].as_str());
        if last_updated < floor || external_id.is_empty() {
            return None;
        }
        let members: Vec<ResponseUser> = chat["members"].as_array().cloned().unwrap_or_default().iter().map(Self::member_user).collect();
        let mut topic = chat["topic"].as_str().unwrap_or("").to_string();
        if topic.is_empty() {
            let names: Vec<String> = members.iter().filter(|m| me_id.map_or(true, |id| m.id != id)).filter_map(|m| m.display_name.clone()).collect();
            topic = if names.is_empty() { "Chat".to_string() } else { names.join(", ") };
        }
        let mut metadata = HashMap::new();
        if let Some(link) = chat["webUrl"].as_str() {
            metadata.insert("web_url".to_string(), Value::String(link.to_string()));
        }
        Some(ResponseConversation {
            external_id,
            name: topic,
            conversation_type: Self::map_conversation_type(chat["chatType"].as_str().unwrap_or("unknown")),
            members,
            last_activity: last_updated,
            metadata,
        })
    }

    fn channel_conversation(team_id: &str, team_name: &str, channel: &Value, floor: DateTime<Utc>) -> ResponseConversation {
        let channel_id = channel["id"].as_str().unwrap_or("").to_string();
        let channel_name = channel["displayName"].as_str().unwrap_or("General").to_string();
        let mut metadata = HashMap::new();
        metadata.insert("team_id".to_string(), Value::String(team_id.to_string()));
        metadata.insert("channel_id".to_string(), Value::String(channel_id.clone()));
        metadata.insert("team_name".to_string(), Value::String(team_name.to_string()));
        if let Some(link) = channel["webUrl"].as_str() {
            metadata.insert("web_url".to_string(), Value::String(link.to_string()));
        }
        ResponseConversation {
            external_id: format!("channel:{}:{}", team_id, channel_id),
            name: format!("{} / {}", team_name, channel_name),
            conversation_type: ConversationType::Channel,
            members: Vec::new(),
            last_activity: Self::parse_time(channel["createdDateTime"].as_str()).max(floor),
            metadata,
        }
    }

    fn messages_url(api: &str, conversation_external_id: &str, since: Option<DateTime<Utc>>) -> String {
        match (Self::split_channel(conversation_external_id), since) {
            (Some((team_id, channel_id)), _) => format!("{}/teams/{}/channels/{}/messages?$top=50&$expand=replies", api, team_id, channel_id),
            (None, Some(since)) => format!(
                "{}/chats/{}/messages?$top=50&$orderby=lastModifiedDateTime desc&$filter=lastModifiedDateTime gt {}",
                api,
                conversation_external_id,
                since.format("%Y-%m-%dT%H:%M:%S%.3fZ")
            ),
            (None, None) => format!("{}/chats/{}/messages?$top=50", api, conversation_external_id),
        }
    }

    fn message_url(&self, conversation_external_id: &str, message_external_id: &str) -> String {
        match Self::split_channel(conversation_external_id) {
            Some((team_id, channel_id)) => format!("{}/teams/{}/channels/{}/messages/{}", self.api, team_id, channel_id, message_external_id),
            None => format!("{}/chats/{}/messages/{}", self.api, conversation_external_id, message_external_id),
        }
    }

    pub fn chat_web_url(conversation_external_id: &str, message_external_id: &str) -> Option<String> {
        if conversation_external_id.starts_with("mail:") || Self::split_channel(conversation_external_id).is_some() || message_external_id.is_empty() {
            return None;
        }
        Some(format!("{}/l/message/{}/{}?context=%7B%22contextType%22%3A%22chat%22%7D", TEAMS_WEB, urlencoding::encode(conversation_external_id), message_external_id))
    }

    fn response_verb(response: &str) -> Result<&'static str, String> {
        match response {
            "accepted" | "accept" => Ok("accept"),
            "declined" | "decline" => Ok("decline"),
            "tentativelyAccepted" | "tentative" | "tentativelyAccept" => Ok("tentativelyAccept"),
            other => Err(format!("{} is not an answer to an invitation; use accepted, declined or tentativelyAccepted", other)),
        }
    }

    fn member_body(api: &str, user_external_id: &str) -> Value {
        let base = if api.starts_with(GRAPH) { GRAPH } else { api };
        serde_json::json!({
            "@odata.type": "#microsoft.graph.aadUserConversationMember",
            "roles": ["owner"],
            "user@odata.bind": format!("{}/users('{}')", base, user_external_id),
            "visibleHistoryStartDateTime": "0001-01-01T00:00:00Z",
        })
    }

    fn pinned(data: &Value) -> Vec<(String, String)> {
        data["value"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|p| Some((p["id"].as_str()?.to_string(), p["message"]["id"].as_str()?.to_string())))
            .collect()
    }

    fn mail_changes(data: &Value) -> Vec<RemoteChange> {
        data["value"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|item| {
                let id = item["id"].as_str()?.to_string();
                if item.get("@removed").is_some() {
                    return Some(RemoteChange { message_external_id: id, deleted: true, ..Default::default() });
                }
                Some(RemoteChange {
                    conversation_external_id: item["conversationId"].as_str().map(|c| format!("mail:{}", c)),
                    message_external_id: id,
                    is_read: item["isRead"].as_bool(),
                    flagged: item["flag"]["flagStatus"].as_str().map(|f| f == "flagged"),
                    ..Default::default()
                })
            })
            .collect()
    }

    fn people_user(item: &Value) -> Option<ResponseUser> {
        let email = item["scoredEmailAddresses"].as_array().and_then(|a| a.first()).and_then(|e| e["address"].as_str()).or_else(|| item["userPrincipalName"].as_str()).map(|e| e.to_lowercase());
        let organisation = item["personType"]["subclass"].as_str() == Some("OrganizationUser");
        let id = if organisation { item["id"].as_str()?.to_string() } else { email.clone()? };
        Some(ResponseUser {
            id,
            email,
            display_name: item["displayName"].as_str().map(String::from),
            first_name: item["givenName"].as_str().map(String::from),
            last_name: item["surname"].as_str().map(String::from),
            job_title: item["jobTitle"].as_str().filter(|t| !t.is_empty()).map(String::from),
            department: item["department"].as_str().filter(|t| !t.is_empty()).map(String::from),
            company: item["companyName"].as_str().filter(|t| !t.is_empty()).map(String::from),
            office_location: item["officeLocation"].as_str().filter(|t| !t.is_empty()).map(String::from),
            ..Default::default()
        })
    }

    async fn post_reference(&self, auth: &ProviderAuth, attachment: &OutgoingAttachment) -> Result<Value, String> {
        let path = format!("{}/{}", CHAT_FILES_FOLDER, attachment.name);
        let url = format!("{}/me/drive/root:/{}:/content", self.api, urlencoding::encode(&path).replace("%2F", "/"));
        let response = crate::providers::http::send(self.client.put(&url).bearer_auth(&auth.access_token).header("Content-Type", attachment.mime_type.as_str()).body(attachment.bytes.clone())).await?;
        let item = Self::read_json(response).await?;
        Self::reference_attachment(&item).ok_or_else(|| format!("OneDrive did not describe the uploaded {}", attachment.name))
    }

    fn reference_attachment(item: &Value) -> Option<Value> {
        let tag = item["eTag"].as_str()?;
        let guid = tag.trim_matches('"').trim_start_matches('{').split(['}', ',']).next()?.to_lowercase();
        Some(serde_json::json!({ "id": guid, "contentType": "reference", "contentUrl": item["webUrl"].as_str()?, "name": item["name"].as_str()? }))
    }

    async fn fetch_user_details(&self, auth: &ProviderAuth, user_id: &str) -> Result<ResponseUser, String> {
        let data = self.get_json(auth, &format!("{}/users/{}", self.api, user_id)).await?;
        Ok(ResponseUser {
            id: data["id"].as_str().unwrap_or(user_id).to_string(),
            email: data["mail"].as_str().or_else(|| data["userPrincipalName"].as_str()).map(String::from),
            phone: data["mobilePhone"].as_str().or_else(|| data["businessPhones"].as_array().and_then(|phones| phones.first()).and_then(|p| p.as_str())).map(String::from),
            display_name: data["displayName"].as_str().map(String::from),
            first_name: data["givenName"].as_str().map(String::from),
            last_name: data["surname"].as_str().map(String::from),
            avatar_url: None,
            job_title: data["jobTitle"].as_str().map(String::from),
            department: data["department"].as_str().map(String::from),
            company: data["companyName"].as_str().map(String::from),
            office_location: data["officeLocation"].as_str().map(String::from),
        })
    }

    async fn get_current_user(&self, auth: &ProviderAuth) -> Result<ResponseUser, String> {
        let data = self.get_json(auth, &format!("{}/me", self.api)).await?;
        Ok(ResponseUser {
            id: data["id"].as_str().unwrap_or("").to_string(),
            email: data["mail"].as_str().or(data["userPrincipalName"].as_str()).map(String::from),
            phone: data["mobilePhone"].as_str().map(String::from),
            display_name: data["displayName"].as_str().map(String::from),
            first_name: data["givenName"].as_str().map(String::from),
            last_name: data["surname"].as_str().map(String::from),
            avatar_url: None,
            job_title: data["jobTitle"].as_str().map(String::from),
            department: data["department"].as_str().map(String::from),
            company: data["companyName"].as_str().map(String::from),
            office_location: data["officeLocation"].as_str().map(String::from),
        })
    }

    async fn me_and_tenant(&self, auth: &ProviderAuth) -> Result<(String, Option<String>), String> {
        self.me
            .get_or_try_init(|| async {
                let me = self.get_json(auth, &format!("{}/me?$select=id", self.api)).await?;
                let organization = self.get_json(auth, &format!("{}/organization?$select=id", self.api)).await.unwrap_or(Value::Null);
                let tenant = organization["value"][0]["id"].as_str().map(String::from);
                Ok::<_, String>((me["id"].as_str().unwrap_or("").to_string(), tenant))
            })
            .await
            .cloned()
    }

    async fn token_call(&self, params: &[(&str, &str)], previous_refresh: Option<&str>) -> Result<ProviderAuth, String> {
        let response = crate::providers::http::send(self.client.post(format!("{}/token", authority(&self.tenant))).form(params)).await?;
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Token request failed: {} - {}", status, error_text));
        }
        let token_data: Value = response.json().await.map_err(|e| format!("Failed to parse token response: {}", e))?;
        let access_token = token_data["access_token"].as_str().ok_or("No access token in response")?;
        let refresh_token = token_data["refresh_token"].as_str().or(previous_refresh);
        let expires_in = token_data["expires_in"].as_u64().unwrap_or(3600);
        Ok(ProviderAuth {
            access_token: access_token.to_string(),
            refresh_token: refresh_token.map(String::from),
            expires_at: Utc::now() + Duration::seconds(expires_in as i64),
            token_type: "Bearer".to_string(),
            scope: token_data["scope"].as_str().map(String::from),
        })
    }
}
impl MicrosoftProvider {
    pub fn graph_calendar(item: &Value) -> RemoteCalendar {
        let hex = item["hexColor"].as_str().filter(|c| !c.is_empty()).map(String::from);
        RemoteCalendar {
            external_id: item["id"].as_str().unwrap_or("").to_string(),
            name: item["name"].as_str().unwrap_or("Calendar").to_string(),
            colour: hex,
            is_primary: item["isDefaultCalendar"].as_bool().unwrap_or(false),
            can_write: item["canEdit"].as_bool().unwrap_or(false),
        }
    }
    fn graph_time(value: &Value) -> Option<DateTime<Utc>> {
        let text = value["dateTime"].as_str()?;
        if let Ok(at) = DateTime::parse_from_rfc3339(text) {
            return Some(at.with_timezone(&Utc));
        }
        let naive = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f").ok()?;
        Some(naive.and_utc())
    }
    pub fn graph_event(item: &Value) -> Option<RemoteEvent> {
        let starts_at = Self::graph_time(&item["start"])?;
        let ends_at = Self::graph_time(&item["end"]).unwrap_or(starts_at);
        let address = |v: &Value| v["emailAddress"]["address"].as_str().map(|s| s.to_lowercase());
        let notes = item["bodyPreview"].as_str().unwrap_or("").trim().to_string();
        let join = item["onlineMeeting"]["joinUrl"].as_str().or_else(|| item["onlineMeetingUrl"].as_str()).filter(|u| !u.is_empty()).map(String::from);
        Some(RemoteEvent {
            external_id: item["id"].as_str()?.to_string(),
            ical_uid: item["iCalUId"].as_str().filter(|u| !u.is_empty()).map(String::from),
            title: item["subject"].as_str().unwrap_or("").to_string(),
            notes,
            location: item["location"]["displayName"].as_str().unwrap_or("").to_string(),
            starts_at,
            ends_at,
            all_day: item["isAllDay"].as_bool().unwrap_or(false),
            attendees: item["attendees"].as_array().cloned().unwrap_or_default().iter().filter_map(address).collect(),
            organizer: address(&item["organizer"]),
            response_status: item["responseStatus"]["response"].as_str().filter(|r| *r != "none").map(String::from),
            online_meeting_url: join,
            cancelled: item["isCancelled"].as_bool().unwrap_or(false),
            updated_at: item["lastModifiedDateTime"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc)).unwrap_or(starts_at),
        })
    }
    pub fn graph_event_body(event: &NewRemoteEvent) -> Value {
        let time = |at: DateTime<Utc>| serde_json::json!({ "dateTime": at.format("%Y-%m-%dT%H:%M:%S").to_string(), "timeZone": "UTC" });
        let mut body = serde_json::json!({
            "subject": event.title,
            "body": { "contentType": "text", "content": event.notes },
            "start": time(event.starts_at),
            "end": time(event.ends_at),
            "isAllDay": event.all_day,
            "attendees": event.attendees.iter().map(|a| serde_json::json!({ "emailAddress": { "address": a }, "type": "required" })).collect::<Vec<_>>(),
        });
        if !event.location.trim().is_empty() {
            body["location"] = serde_json::json!({ "displayName": event.location });
        }
        if event.online_meeting {
            body["isOnlineMeeting"] = Value::Bool(true);
            body["onlineMeetingProvider"] = Value::String("teamsForBusiness".to_string());
        }
        body
    }
    fn parse_time(value: Option<&str>) -> DateTime<Utc> {
        value
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now)
    }
    fn mail_forward_of(draft: &OutgoingMessage) -> Option<&OutgoingReference> {
        match draft.forwarded.as_slice() {
            [only] if only.native => Some(only),
            _ => None,
        }
    }
    fn forward_request(to: &[String], comment: &str) -> Value {
        serde_json::json!({
            "comment": comment,
            "toRecipients": to.iter().map(|a| serde_json::json!({ "emailAddress": { "address": a } })).collect::<Vec<_>>(),
        })
    }
    fn quote_attachment(quote: &OutgoingReference) -> Value {
        let reference = serde_json::json!({
            "messageId": quote.external_id,
            "messagePreview": quote.text.chars().take(200).collect::<String>(),
            "messageSender": { "user": { "id": quote.sender_id, "displayName": quote.sender_name, "userIdentityType": "aadUser" } },
        });
        serde_json::json!({ "id": quote.external_id, "contentType": "messageReference", "content": reference.to_string() })
    }
    fn chat_payload(content_type: &str, content: &str, quote: Option<&OutgoingReference>, images: &str, hosted: Vec<Value>, files: Vec<Value>) -> Value {
        if quote.is_none() && hosted.is_empty() && files.is_empty() {
            return serde_json::json!({ "body": { "contentType": content_type, "content": content } });
        }
        let html = if content_type == "html" { content.to_string() } else { html_escape(content).replace('\n', "<br>") };
        let lead = quote.map(|q| format!("<attachment id=\"{}\"></attachment>", q.external_id)).unwrap_or_default();
        let tail: String = files.iter().filter_map(|f| f["id"].as_str()).map(|id| format!("<attachment id=\"{}\"></attachment>", id)).collect();
        let mut payload = serde_json::json!({ "body": { "contentType": "html", "content": format!("{}{}{}{}", lead, html, images, tail) } });
        let mut attachments: Vec<Value> = quote.map(Self::quote_attachment).into_iter().collect();
        attachments.extend(files);
        if !attachments.is_empty() {
            payload["attachments"] = Value::Array(attachments);
        }
        if !hosted.is_empty() {
            payload["hostedContents"] = Value::Array(hosted);
        }
        payload
    }
    fn split_channel(conversation_external_id: &str) -> Option<(String, String)> {
        let rest = conversation_external_id.strip_prefix("channel:")?;
        let (team, channel) = rest.split_once(':')?;
        Some((team.to_string(), channel.to_string()))
    }
    fn member_user(member: &Value) -> ResponseUser {
        let identity = &member["userIdentity"];
        ResponseUser {
            id: member["userId"].as_str().or_else(|| member["id"].as_str()).unwrap_or("").to_string(),
            email: member["email"].as_str().or_else(|| member["userPrincipalName"].as_str()).or_else(|| identity["signInName"].as_str()).or_else(|| identity["userPrincipalName"].as_str()).filter(|e| !e.is_empty()).map(String::from),
            display_name: member["displayName"].as_str().map(String::from),
            ..Default::default()
        }
    }
    fn email_user(address: &Value) -> ResponseUser {
        let email = address["address"].as_str().map(|s| s.to_lowercase());
        ResponseUser {
            id: email.clone().unwrap_or_default(),
            email,
            display_name: address["name"].as_str().map(String::from),
            ..Default::default()
        }
    }
    fn mail_message(mail: &Value, conversation_external_id: &str) -> ResponseMessage {
        let people = |key: &str| -> Vec<ResponseUser> {
            mail[key].as_array().cloned().unwrap_or_default().iter().map(|r| Self::email_user(&r["emailAddress"])).collect()
        };
        let attachments = mail["attachments"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|a| ResponseAttachment {
                external_id: a["id"].as_str().unwrap_or("").to_string(),
                name: a["name"].as_str().unwrap_or("attachment").to_string(),
                content_type: a["contentType"].as_str().unwrap_or("application/octet-stream").to_string(),
                size: a["size"].as_u64().unwrap_or(0),
                url: None,
                content_id: a["isInline"].as_bool().filter(|i| *i).and_then(|_| a["contentId"].as_str().or_else(|| a["name"].as_str())).map(String::from),
            })
            .collect();
        let mut metadata = HashMap::new();
        if let Some(id) = mail["internetMessageId"].as_str() {
            metadata.insert("internet_message_id".to_string(), Value::String(id.to_string()));
        }
        if let Some(link) = mail["webLink"].as_str() {
            metadata.insert("web_url".to_string(), Value::String(link.to_string()));
        }
        if let Some(categories) = mail["categories"].as_array().filter(|c| !c.is_empty()) {
            metadata.insert("labels".to_string(), Value::Array(categories.clone()));
        }
        let body = mail["body"]["content"].as_str().unwrap_or("");
        let is_html = mail["body"]["contentType"].as_str().map(|c| c.eq_ignore_ascii_case("html")).unwrap_or(true);
        let structure = crate::content::mail::analyse(is_html.then_some(body), (!is_html).then_some(body), None, None);
        let mut message = ResponseMessage {
            external_id: mail["id"].as_str().unwrap_or("").to_string(),
            conversation_external_id: conversation_external_id.to_string(),
            sender: Self::email_user(&mail["from"]["emailAddress"]),
            content: mail["body"]["content"].as_str().unwrap_or("").to_string(),
            content_type: mail["body"]["contentType"].as_str().unwrap_or("html").to_lowercase(),
            timestamp: Self::parse_time(mail["receivedDateTime"].as_str()),
            r#type: MessageType::Email,
            metadata,
            subject: mail["subject"].as_str().map(String::from),
            recipients: people("toRecipients"),
            cc: people("ccRecipients"),
            is_read: mail["isRead"].as_bool().unwrap_or(true),
            attachments,
            ..Default::default()
        };
        structure.apply(&mut message);
        message
    }
    fn hosted_attachments(images: &[String]) -> Vec<ResponseAttachment> {
        images
            .iter()
            .filter(|src| src.contains("/hostedContents/"))
            .enumerate()
            .map(|(i, src)| {
                let id = src.split("/hostedContents/").nth(1).and_then(|rest| rest.split('/').next()).unwrap_or("").to_string();
                ResponseAttachment { external_id: format!("hosted:{}", id), name: format!("image-{}.png", i + 1), content_type: "image/png".to_string(), size: 0, url: Some(src.clone()), content_id: Some(id) }
            })
            .collect()
    }
    async fn chat_message(
        &self,
        auth: &ProviderAuth,
        msg: &Value,
        conversation_external_id: &str,
        reply_to: Option<String>,
        cache: &mut HashMap<String, ResponseUser>,
    ) -> Option<ResponseMessage> {
        if msg["deletedDateTime"].is_string() {
            return None;
        }
        match msg["messageType"].as_str().unwrap_or("message") {
            "message" => {}
            "systemEventMessage" => return self.system_message(auth, msg, conversation_external_id, cache).await,
            _ => return None,
        }
        let user = msg["from"]["user"].as_object()?;
        let user_id = user.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if user_id.is_empty() {
            return None;
        }
        let sender = self.get_user(auth, &user_id, user.get("displayName").and_then(|v| v.as_str()), cache).await;
        let id = msg["id"].as_str().unwrap_or("").to_string();
        let mut metadata = HashMap::new();
        if let Some(link) = msg["webUrl"].as_str().map(String::from).or_else(|| Self::chat_web_url(conversation_external_id, &id)) {
            metadata.insert("web_url".to_string(), Value::String(link));
        }
        let mut message = ResponseMessage {
            external_id: id,
            conversation_external_id: conversation_external_id.to_string(),
            sender,
            content: msg["body"]["content"].as_str().unwrap_or("").to_string(),
            content_type: msg["body"]["contentType"].as_str().unwrap_or("text").to_lowercase(),
            timestamp: Self::parse_time(msg["createdDateTime"].as_str()),
            r#type: MessageType::Chat,
            metadata,
            subject: msg["subject"].as_str().map(String::from),
            thread_id: reply_to.clone(),
            reply_to_id: reply_to.or_else(|| msg["replyToId"].as_str().map(String::from)),
            ..Default::default()
        };
        let content = crate::content::chat::teams(msg);
        content.apply(&mut message);
        message.attachments.extend(Self::hosted_attachments(&content.images));
        Some(message)
    }
    async fn get_user(&self, auth: &ProviderAuth, user_id: &str, name: Option<&str>, cache: &mut HashMap<String, ResponseUser>) -> ResponseUser {
        if let Some(cached) = cache.get(user_id) {
            return cached.clone();
        }
        let full = self.fetch_user_details(auth, user_id).await.unwrap_or_else(|_| ResponseUser {
            id: user_id.to_string(),
            display_name: name.filter(|n| !n.is_empty()).map(String::from),
            ..Default::default()
        });
        cache.insert(user_id.to_string(), full.clone());
        full
    }
    fn system_person(identity: &Value) -> Option<SystemPerson> {
        let user = if identity["user"].is_object() { &identity["user"] } else { identity };
        let id = user["id"].as_str().filter(|id| !id.is_empty())?;
        Some(SystemPerson::new(id, user["displayName"].as_str().unwrap_or("")))
    }
    fn system_event(msg: &Value) -> Option<SystemEvent> {
        let detail = &msg["eventDetail"];
        let kind = detail["@odata.type"].as_str()?.trim_start_matches("#microsoft.graph.");
        let initiator: Vec<SystemPerson> = Self::system_person(&detail["initiator"]).into_iter().collect();
        let members: Vec<SystemPerson> = detail["members"].as_array().map(|all| all.iter().filter_map(Self::system_person).collect()).unwrap_or_default();
        let text = |key: &str| detail[key].as_str().map(String::from);
        let themselves = !initiator.is_empty() && members.len() == 1 && members[0].external_id == initiator[0].external_id;
        Some(match kind {
            "membersAddedEventMessageDetail" if themselves => SystemEvent::new(SystemEventKind::MemberJoined, members, Vec::new(), None),
            "membersAddedEventMessageDetail" => SystemEvent::new(SystemEventKind::MemberAdded, initiator, members, None),
            "membersDeletedEventMessageDetail" if themselves => SystemEvent::new(SystemEventKind::MemberLeft, members, Vec::new(), None),
            "membersDeletedEventMessageDetail" => SystemEvent::new(SystemEventKind::MemberRemoved, initiator, members, None),
            "membersJoinedEventMessageDetail" => SystemEvent::new(SystemEventKind::MemberJoined, members, Vec::new(), None),
            "membersLeftEventMessageDetail" => SystemEvent::new(SystemEventKind::MemberLeft, members, Vec::new(), None),
            "chatRenamedEventMessageDetail" => SystemEvent::new(SystemEventKind::Renamed, initiator, Vec::new(), text("chatDisplayName")),
            "channelRenamedEventMessageDetail" => SystemEvent::new(SystemEventKind::Renamed, initiator, Vec::new(), text("channelDisplayName")),
            "teamRenamedEventMessageDetail" => SystemEvent::new(SystemEventKind::Renamed, initiator, Vec::new(), text("teamDisplayName")),
            "channelDescriptionUpdatedEventMessageDetail" => SystemEvent::new(SystemEventKind::TopicChanged, initiator, Vec::new(), text("channelDescription")),
            "teamDescriptionUpdatedEventMessageDetail" => SystemEvent::new(SystemEventKind::TopicChanged, initiator, Vec::new(), text("teamDescription")),
            "callStartedEventMessageDetail" => SystemEvent::new(SystemEventKind::CallStarted, initiator, Vec::new(), None),
            "callEndedEventMessageDetail" => SystemEvent::new(SystemEventKind::CallEnded, initiator, Vec::new(), None),
            "messagePinnedEventMessageDetail" => SystemEvent::new(SystemEventKind::Pinned, initiator, Vec::new(), None),
            "messageUnpinnedEventMessageDetail" => SystemEvent::new(SystemEventKind::Unpinned, initiator, Vec::new(), None),
            "channelAddedEventMessageDetail" | "teamCreatedEventMessageDetail" => SystemEvent::new(SystemEventKind::ChatCreated, initiator, Vec::new(), None),
            _ => return None,
        })
    }
    async fn system_message(
        &self,
        auth: &ProviderAuth,
        msg: &Value,
        conversation_external_id: &str,
        cache: &mut HashMap<String, ResponseUser>,
    ) -> Option<ResponseMessage> {
        let mut event = Self::system_event(msg)?;
        for person in event.people_mut() {
            if person.name.is_empty() {
                let user = self.get_user(auth, &person.external_id, None, cache).await;
                person.name = user.display_name.or(user.email).unwrap_or_default();
            }
        }
        let mut message = ResponseMessage::system(msg["id"].as_str().unwrap_or(""), conversation_external_id, Self::parse_time(msg["createdDateTime"].as_str()), event);
        if message.sender.id.is_empty() {
            return None;
        }
        message.sender = self.get_user(auth, &message.sender.id.clone(), message.sender.display_name.clone().as_deref(), cache).await;
        message.reply_to_id = msg["replyToId"].as_str().map(String::from);
        Some(message)
    }
    async fn get_json(&self, auth: &ProviderAuth, url: &str) -> Result<Value, String> {
        let response = crate::providers::http::send(self.client.get(url).bearer_auth(&auth.access_token)).await?;
        Self::read_json(response).await
    }
    async fn get_bytes(&self, auth: &ProviderAuth, url: &str) -> Result<Option<(Vec<u8>, String)>, String> {
        let response = crate::providers::http::send(self.client.get(url).bearer_auth(&auth.access_token)).await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!("Graph {}", response.status()));
        }
        let mime = response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("application/octet-stream").to_string();
        let bytes = response.bytes().await.map_err(|e| format!("Graph sent an unreadable file: {}", e))?;
        Ok(Some((bytes.to_vec(), mime)))
    }
    async fn post_json(&self, auth: &ProviderAuth, url: &str, body: &Value) -> Result<Value, String> {
        let response = crate::providers::http::send(self.client.post(url).bearer_auth(&auth.access_token).json(body)).await?;
        Self::read_json(response).await
    }
    async fn patch_json(&self, auth: &ProviderAuth, url: &str, body: &Value) -> Result<Value, String> {
        let response = crate::providers::http::send(self.client.patch(url).bearer_auth(&auth.access_token).json(body)).await?;
        Self::read_json(response).await
    }
    async fn delete(&self, auth: &ProviderAuth, url: &str) -> Result<(), String> {
        let response = crate::providers::http::send(self.client.delete(url).bearer_auth(&auth.access_token)).await?;
        Self::read_json(response).await.map(|_| ())
    }
    async fn read_json(response: reqwest::Response) -> Result<Value, String> {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            let detail = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(String::from))
                .unwrap_or(text);
            return Err(format!("Graph {}: {}", status, detail));
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|e| format!("Graph returned unreadable JSON: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calendar_fixture(name: &str) -> Value {
        serde_json::from_str(&std::fs::read_to_string(format!("{}/tests/fixtures/calendar/{}", env!("CARGO_MANIFEST_DIR"), name)).unwrap()).unwrap()
    }

    #[test]
    fn graph_calendars_map_default_and_edit_rights() {
        let list = calendar_fixture("graph-calendars.json");
        let calendars: Vec<RemoteCalendar> = list["value"].as_array().unwrap().iter().map(MicrosoftProvider::graph_calendar).collect();
        assert_eq!(calendars[0], RemoteCalendar { external_id: "AAMkAGI2-cal-1".into(), name: "Calendar".into(), colour: Some("#0f6cbd".into()), is_primary: true, can_write: true });
        assert_eq!(calendars[1].colour, None);
        assert!(!calendars[1].can_write);
    }

    #[test]
    fn graph_events_map_utc_times_people_teams_links_and_cancellations() {
        let view = calendar_fixture("graph-calendar-view.json");
        let items = view["value"].as_array().unwrap();
        let signing = MicrosoftProvider::graph_event(&items[0]).unwrap();
        assert_eq!(signing.starts_at.to_rfc3339(), "2026-10-16T11:00:00+00:00");
        assert_eq!(signing.notes, "Vi går gjennom avtalen og signerer.");
        assert_eq!(signing.attendees, vec!["anna.berg@client.example", "milo@northwind.example"]);
        assert_eq!(signing.organizer.as_deref(), Some("anna.berg@client.example"));
        assert_eq!(signing.response_status.as_deref(), Some("accepted"));
        assert_eq!(signing.online_meeting_url.as_deref(), Some("https://teams.microsoft.com/l/meetup-join/19%3ameeting_abc"));
        assert_eq!(signing.location, "Microsoft Teams Meeting");
        let holiday = MicrosoftProvider::graph_event(&items[1]).unwrap();
        assert!(holiday.all_day);
        assert_eq!(holiday.online_meeting_url, None);
        assert_eq!(holiday.response_status.as_deref(), Some("organizer"));
        assert!(MicrosoftProvider::graph_event(&items[2]).unwrap().cancelled);
        assert!(MicrosoftProvider::graph_event(&items[3]).is_none());
    }

    #[test]
    fn graph_create_body_requests_a_teams_meeting_in_utc() {
        let at = |s: &str| DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc);
        let event = NewRemoteEvent { title: "Kickoff".into(), notes: "Agenda".into(), location: "Rom 4".into(), starts_at: at("2026-10-20T08:00:00Z"), ends_at: at("2026-10-20T09:00:00Z"), attendees: vec!["anna.berg@client.example".into()], online_meeting: true, ..Default::default() };
        let body = MicrosoftProvider::graph_event_body(&event);
        assert_eq!(body["subject"], "Kickoff");
        assert_eq!(body["start"], serde_json::json!({ "dateTime": "2026-10-20T08:00:00", "timeZone": "UTC" }));
        assert_eq!(body["attendees"][0]["emailAddress"]["address"], "anna.berg@client.example");
        assert_eq!(body["location"]["displayName"], "Rom 4");
        assert_eq!(body["isOnlineMeeting"], true);
        assert_eq!(body["onlineMeetingProvider"], "teamsForBusiness");
        let plain = MicrosoftProvider::graph_event_body(&NewRemoteEvent { online_meeting: false, location: String::new(), ..event });
        assert!(plain.get("isOnlineMeeting").is_none());
        assert!(plain.get("location").is_none());
    }

    #[test]
    fn admin_scopes_are_requested_only_when_switched_on() {
        let base = scopes(false);
        assert!(base.contains("Calendars.ReadWrite") && base.contains("OnlineMeetings.ReadWrite") && base.contains("offline_access"));
        assert!(!base.contains("ChannelMessage.Read.All") && !base.contains("Calendars.Read ") && !base.contains("Organization.Read.All"));
        assert!(scopes(true).ends_with("ChannelMessage.ReadWrite ChannelMessage.Read.All"));
        assert!(base.contains("User.ReadBasic.All") && base.contains("People.Read") && base.contains("Files.ReadWrite") && base.contains("Presence.ReadWrite"));
    }

    #[test]
    fn tenants_are_normalised_or_refused() {
        assert_eq!(normalize_tenant("").unwrap(), "organizations");
        assert_eq!(normalize_tenant(" Common ").unwrap(), "common");
        assert_eq!(normalize_tenant("Contoso.onmicrosoft.com").unwrap(), "contoso.onmicrosoft.com");
        assert_eq!(normalize_tenant("72f988bf-86f1-41af-91ab-2d7cd011db47").unwrap(), "72f988bf-86f1-41af-91ab-2d7cd011db47");
        assert!(normalize_tenant("not a tenant").is_err());
        assert!(normalize_tenant("72f988bf86f141af91ab2d7cd011db47").is_err());
        assert_eq!(authority("organizations"), "https://login.microsoftonline.com/organizations/oauth2/v2.0");
    }

    #[test]
    fn the_admin_consent_link_names_every_graph_permission() {
        let url = admin_consent_url("contoso.com", "abc", "http://localhost:48731/callback/microsoft");
        assert!(url.starts_with("https://login.microsoftonline.com/contoso.com/v2.0/adminconsent?client_id=abc&scope="));
        let scope = url::Url::parse(&url).unwrap().query_pairs().find(|(k, _)| k == "scope").unwrap().1.to_string();
        assert!(scope.contains("https://graph.microsoft.com/ChannelMessage.Read.All") && scope.contains("https://graph.microsoft.com/Mail.ReadWrite"));
        assert!(!scope.contains("offline_access"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A48731%2Fcallback%2Fmicrosoft") && url.ends_with("state=admin-consent"));
    }

    #[test]
    fn consent_errors_map_to_plain_problems() {
        assert_eq!(consent_issue("invalid_client", "AADSTS65001: The user or administrator has not consented"), Some(ConsentIssue::NeedsAdmin));
        assert_eq!(consent_issue("access_denied", "AADSTS90094: Admin consent is required"), Some(ConsentIssue::NeedsAdmin));
        assert_eq!(consent_issue("consent_required", ""), Some(ConsentIssue::NeedsAdmin));
        assert_eq!(consent_issue("access_denied", "AADSTS65004: User declined to consent"), Some(ConsentIssue::Declined));
        assert_eq!(consent_issue("invalid_request", "AADSTS50011: The redirect URI does not match"), Some(ConsentIssue::RedirectMismatch));
        assert_eq!(consent_issue("unauthorized_client", "AADSTS700016: Application not found in the directory"), Some(ConsentIssue::UnknownApp));
        assert_eq!(consent_issue("invalid_request", "AADSTS50020: User account from identity provider does not exist in tenant"), Some(ConsentIssue::WrongAccount));
        assert_eq!(consent_issue("server_error", "something else"), None);
        assert!(consent_text(ConsentIssue::RedirectMismatch).1.contains("http://localhost:48731/callback/microsoft"));
    }

    #[tokio::test]
    async fn the_sign_in_url_uses_the_tenant_and_the_scope_set() {
        let provider = MicrosoftProvider::new("abc".into(), String::new(), "http://localhost:48731/callback/microsoft".into(), "contoso.com".into(), false);
        let url = provider.get_auth_url("s1", Some("ch")).await.unwrap();
        assert!(url.starts_with("https://login.microsoftonline.com/contoso.com/oauth2/v2.0/authorize?client_id=abc&"));
        assert!(url.contains("code_challenge=ch&code_challenge_method=S256") && url.contains("state=s1"));
        assert!(!url.contains("ChannelMessage.Read.All"));
    }

    fn reference(id: &str) -> OutgoingReference {
        OutgoingReference { external_id: id.into(), sender_id: Some("u-nora".into()), sender_name: "Nora Gale".into(), text: "Is staging ready?".into(), native: true, ..Default::default() }
    }

    #[test]
    fn a_quoted_teams_reply_carries_a_message_reference_the_sync_reads_back() {
        let quote = reference("1699");
        let payload = MicrosoftProvider::chat_payload("text", "Yes, green", Some(&quote), "", Vec::new(), Vec::new());
        assert_eq!(payload["body"]["contentType"], "html");
        assert_eq!(payload["body"]["content"], "<attachment id=\"1699\"></attachment>Yes, green");
        let attachment = &payload["attachments"][0];
        assert_eq!(attachment["contentType"], "messageReference");
        assert_eq!(attachment["id"], "1699");
        let content: Value = serde_json::from_str(attachment["content"].as_str().unwrap()).unwrap();
        assert_eq!(content["messageId"], "1699");
        assert_eq!(content["messageSender"]["user"]["displayName"], "Nora Gale");
        let echoed = serde_json::json!({ "body": payload["body"].clone(), "attachments": payload["attachments"].clone(), "from": { "user": { "id": "me" } } });
        let parsed = crate::content::chat::teams(&echoed);
        assert_eq!(parsed.reply_to.as_deref(), Some("1699"));
        assert!(parsed.html.unwrap().contains("class=\"reply-quote\""));
    }

    #[test]
    fn a_plain_teams_message_has_no_reference_and_images_join_the_quote() {
        assert_eq!(MicrosoftProvider::chat_payload("text", "Hei", None, "", Vec::new(), Vec::new()), serde_json::json!({ "body": { "contentType": "text", "content": "Hei" } }));
        let quote = reference("7");
        let payload = MicrosoftProvider::chat_payload("text", "a\nb", Some(&quote), "<div><img></div>", vec![serde_json::json!({ "@microsoft.graph.temporaryId": "1" })], Vec::new());
        assert_eq!(payload["body"]["content"], "<attachment id=\"7\"></attachment>a<br>b<div><img></div>");
        assert_eq!(payload["hostedContents"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn outlook_forwards_one_message_of_its_own_natively_and_others_by_repost() {
        let mut draft = OutgoingMessage { forwarded: vec![reference("AAMk-1")], ..Default::default() };
        assert_eq!(MicrosoftProvider::mail_forward_of(&draft).map(|r| r.external_id.as_str()), Some("AAMk-1"));
        draft.forwarded[0].native = false;
        assert!(MicrosoftProvider::mail_forward_of(&draft).is_none());
        draft.forwarded = vec![reference("a"), reference("b")];
        assert!(MicrosoftProvider::mail_forward_of(&draft).is_none());
        let request = MicrosoftProvider::forward_request(&["anna.berg@client.example".to_string()], "<p>FYI</p>");
        assert_eq!(request["comment"], "<p>FYI</p>");
        assert_eq!(request["toRecipients"][0]["emailAddress"]["address"], "anna.berg@client.example");
    }

    #[test]
    fn microsoft_declares_native_quotes_and_reposted_forwards() {
        let caps = MicrosoftProvider::new(String::new(), String::new(), String::new(), "common".into(), false).get_provider().capabilities.clone();
        assert!(caps.supports_reply_quote && caps.supports_forward);
        assert_eq!(caps.reply_quote, Delivery::Native);
        assert_eq!(caps.forward, Delivery::Repost);
    }

    fn graph_event(detail: Value) -> Value {
        serde_json::json!({ "id": "1700", "messageType": "systemEventMessage", "createdDateTime": "2026-10-12T09:00:00Z", "from": null, "body": { "contentType": "html", "content": "<systemEventMessage/>" }, "eventDetail": detail })
    }

    fn graph_user(id: &str, name: Option<&str>) -> Value {
        serde_json::json!({ "id": id, "displayName": name, "userIdentityType": "aadUser" })
    }

    fn graph_parsed(detail: Value) -> (SystemEvent, String) {
        let event = MicrosoftProvider::system_event(&graph_event(detail)).unwrap();
        let sentence = event.sentence();
        (event, sentence)
    }

    #[test]
    fn graph_members_added_names_the_initiator_and_every_member() {
        let (event, sentence) = graph_parsed(serde_json::json!({ "@odata.type": "#microsoft.graph.membersAddedEventMessageDetail", "initiator": { "application": null, "device": null, "user": graph_user("u-anna", Some("Anna Berg")) }, "members": [graph_user("u-mia", Some("Mia Sund")), graph_user("u-sam", Some("Sam Lie")), graph_user("u-nora", Some("Nora Gale"))] }));
        assert_eq!(event.event, SystemEventKind::MemberAdded);
        assert_eq!(event.actors, vec![SystemPerson::new("u-anna", "Anna Berg")]);
        assert_eq!(event.targets.iter().map(|p| p.external_id.as_str()).collect::<Vec<_>>(), vec!["u-mia", "u-sam", "u-nora"]);
        assert_eq!(sentence, "Anna Berg added Mia Sund, Sam Lie and Nora Gale");
    }

    #[test]
    fn graph_members_added_by_themselves_is_a_join_and_deleted_by_themselves_is_a_leave() {
        let mia = graph_user("u-mia", Some("Mia Sund"));
        let (joined, sentence) = graph_parsed(serde_json::json!({ "@odata.type": "#microsoft.graph.membersAddedEventMessageDetail", "initiator": { "user": mia }, "members": [mia] }));
        assert_eq!((joined.event, joined.targets.len()), (SystemEventKind::MemberJoined, 0));
        assert_eq!(sentence, "Mia Sund joined the conversation");
        let hugo = graph_user("u-hugo", Some("Hugo Brandt"));
        let (left, sentence) = graph_parsed(serde_json::json!({ "@odata.type": "#microsoft.graph.membersDeletedEventMessageDetail", "initiator": { "user": hugo }, "members": [hugo] }));
        assert_eq!(left.event, SystemEventKind::MemberLeft);
        assert_eq!(sentence, "Hugo Brandt left the conversation");
    }

    #[test]
    fn graph_members_deleted_by_someone_else_is_a_removal() {
        let (event, sentence) = graph_parsed(serde_json::json!({ "@odata.type": "#microsoft.graph.membersDeletedEventMessageDetail", "initiator": { "user": graph_user("u-anna", Some("Anna Berg")) }, "members": [graph_user("u-hugo", Some("Hugo Brandt")), graph_user("u-mia", Some("Mia Sund"))] }));
        assert_eq!(event.event, SystemEventKind::MemberRemoved);
        assert_eq!(sentence, "Anna Berg removed Hugo Brandt and Mia Sund");
    }

    #[test]
    fn graph_joined_and_left_details_make_the_members_the_actors() {
        let (joined, sentence) = graph_parsed(serde_json::json!({ "@odata.type": "#microsoft.graph.membersJoinedEventMessageDetail", "initiator": null, "members": [graph_user("u-mia", Some("Mia Sund"))] }));
        assert_eq!((joined.event, joined.actors[0].external_id.as_str()), (SystemEventKind::MemberJoined, "u-mia"));
        assert_eq!(sentence, "Mia Sund joined the conversation");
        let (_, sentence) = graph_parsed(serde_json::json!({ "@odata.type": "#microsoft.graph.membersLeftEventMessageDetail", "members": [graph_user("u-hugo", Some("Hugo Brandt"))] }));
        assert_eq!(sentence, "Hugo Brandt left the conversation");
    }

    #[test]
    fn graph_renames_topics_calls_and_pins_carry_their_initiator_and_detail() {
        let anna = serde_json::json!({ "user": graph_user("u-anna", Some("Anna Berg")) });
        let cases = [
            (serde_json::json!({ "@odata.type": "#microsoft.graph.chatRenamedEventMessageDetail", "initiator": anna, "chatDisplayName": "Q4 board", "chatId": "19:x" }), SystemEventKind::Renamed, Some("Q4 board"), "Anna Berg renamed the conversation to “Q4 board”"),
            (serde_json::json!({ "@odata.type": "#microsoft.graph.chatRenamedEventMessageDetail", "initiator": anna, "chatDisplayName": null }), SystemEventKind::Renamed, None, "Anna Berg renamed the conversation"),
            (serde_json::json!({ "@odata.type": "#microsoft.graph.channelRenamedEventMessageDetail", "initiator": anna, "channelDisplayName": "dev" }), SystemEventKind::Renamed, Some("dev"), "Anna Berg renamed the conversation to “dev”"),
            (serde_json::json!({ "@odata.type": "#microsoft.graph.channelDescriptionUpdatedEventMessageDetail", "initiator": anna, "channelDescription": "Releases only" }), SystemEventKind::TopicChanged, Some("Releases only"), "Anna Berg changed the topic to “Releases only”"),
            (serde_json::json!({ "@odata.type": "#microsoft.graph.callStartedEventMessageDetail", "initiator": anna, "callEventType": "call", "callId": "c1" }), SystemEventKind::CallStarted, None, "Anna Berg started a call"),
            (serde_json::json!({ "@odata.type": "#microsoft.graph.callEndedEventMessageDetail", "initiator": anna, "callDuration": "PT5M" }), SystemEventKind::CallEnded, None, "Anna Berg ended the call"),
            (serde_json::json!({ "@odata.type": "#microsoft.graph.callEndedEventMessageDetail", "initiator": null }), SystemEventKind::CallEnded, None, "The call ended"),
            (serde_json::json!({ "@odata.type": "#microsoft.graph.messagePinnedEventMessageDetail", "initiator": anna }), SystemEventKind::Pinned, None, "Anna Berg pinned a message"),
            (serde_json::json!({ "@odata.type": "#microsoft.graph.messageUnpinnedEventMessageDetail", "initiator": anna }), SystemEventKind::Unpinned, None, "Anna Berg unpinned a message"),
            (serde_json::json!({ "@odata.type": "#microsoft.graph.teamCreatedEventMessageDetail", "initiator": anna }), SystemEventKind::ChatCreated, None, "Anna Berg created the conversation"),
        ];
        for (detail, kind, text, sentence) in cases {
            let (event, built) = graph_parsed(detail);
            assert_eq!((event.event, event.detail.as_deref(), built.as_str()), (kind, text, sentence));
        }
    }

    #[test]
    fn graph_unknown_details_and_missing_names_are_handled() {
        assert!(MicrosoftProvider::system_event(&graph_event(serde_json::json!({ "@odata.type": "#microsoft.graph.tabUpdatedEventMessageDetail" }))).is_none());
        assert!(MicrosoftProvider::system_event(&serde_json::json!({ "messageType": "systemEventMessage" })).is_none());
        let (event, sentence) = graph_parsed(serde_json::json!({ "@odata.type": "#microsoft.graph.membersAddedEventMessageDetail", "initiator": null, "members": [graph_user("u-mia", None)] }));
        assert_eq!(event.targets, vec![SystemPerson::new("u-mia", "")]);
        assert_eq!(sentence, "Someone joined the conversation");
    }

    #[test]
    fn a_graph_system_message_stores_the_event_and_the_fallback_sentence() {
        let (event, _) = graph_parsed(serde_json::json!({ "@odata.type": "#microsoft.graph.membersAddedEventMessageDetail", "initiator": { "user": graph_user("u-anna", Some("Anna Berg")) }, "members": [graph_user("u-mia", Some("Mia Sund"))] }));
        let message = ResponseMessage::system("1700", "19:chat", Utc::now(), event.clone());
        assert_eq!(message.r#type, MessageType::System);
        assert_eq!(message.content, "Anna Berg added Mia Sund");
        assert_eq!((message.sender.id.as_str(), message.sender.display_name.as_deref()), ("u-anna", Some("Anna Berg")));
        assert_eq!(message.system_event(), Some(event));
        assert_eq!(message.metadata["system"]["event"], "member_added");
        assert_eq!(message.metadata["system"]["targets"][0], serde_json::json!({ "user_id": null, "external_id": "u-mia", "name": "Mia Sund" }));
        assert!(message.metadata["system"].get("detail").is_none());
    }

    mod served {
        use super::super::*;
        use wiremock::matchers::{body_partial_json, method, path, query_param};
        use wiremock::{Mock, MockServer, Request, ResponseTemplate};

        fn fixture(name: &str, base: &str) -> Value {
            let text = std::fs::read_to_string(format!("{}/tests/fixtures/graph/{}", env!("CARGO_MANIFEST_DIR"), name)).unwrap();
            serde_json::from_str(&text.replace("{{base}}", base)).unwrap()
        }

        fn auth() -> ProviderAuth {
            ProviderAuth { access_token: "graph-token".into(), refresh_token: None, expires_at: Utc::now() + Duration::hours(1), token_type: "Bearer".into(), scope: None }
        }

        fn graph(server: &MockServer, admin: bool) -> MicrosoftProvider {
            MicrosoftProvider::new("abc".into(), String::new(), String::new(), "organizations".into(), admin).with_api(&server.uri())
        }

        async fn bodies(server: &MockServer, verb: &str, at: &str) -> Vec<Value> {
            server.received_requests().await.unwrap().into_iter().filter(|r| r.method.as_str() == verb && r.url.path() == at).map(|r| serde_json::from_slice(&r.body).unwrap_or(Value::Null)).collect()
        }

        async fn me(server: &MockServer) {
            Mock::given(method("GET")).and(path("/me")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": "u-me", "displayName": "Milo Oakes", "mail": "milo@northwind.example" }))).mount(server).await;
        }

        #[tokio::test]
        async fn chats_page_through_next_links_skip_old_ones_and_carry_members_and_web_links() {
            let server = MockServer::start().await;
            me(&server).await;
            Mock::given(method("GET")).and(path("/chats")).and(query_param("$skiptoken", "page2")).respond_with(ResponseTemplate::new(200).set_body_json(fixture("chats-page-2.json", &server.uri()))).with_priority(1).mount(&server).await;
            Mock::given(method("GET")).and(path("/chats")).respond_with(ResponseTemplate::new(200).set_body_json(fixture("chats-page-1.json", &server.uri()))).mount(&server).await;
            for folder in ["inbox", "sentitems"] {
                Mock::given(method("GET")).and(path(format!("/me/mailFolders/{}/messages", folder))).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "value": [] }))).mount(&server).await;
            }
            Mock::given(method("GET")).and(path("/me/mailFolders/archive/messages")).respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({ "error": { "message": "The specified folder could not be found" } }))).mount(&server).await;
            let floor = DateTime::parse_from_rfc3339("2026-10-01T00:00:00Z").unwrap().with_timezone(&Utc);
            let conversations = graph(&server, false).sync_conversations(&auth(), Some(floor)).await.unwrap();
            assert_eq!(conversations.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["Nora Gale", "Planning"]);
            assert_eq!(conversations[0].conversation_type, ConversationType::GroupMessage);
            assert_eq!(conversations[0].members[1].email.as_deref(), Some("nora@northwind.example"));
            assert_eq!(conversations[0].metadata["web_url"], "https://teams.microsoft.com/l/chat/19%3Aabc%40thread.v2/0?tenantId=t1");
            let requests = server.received_requests().await.unwrap();
            assert!(requests.iter().all(|r| r.headers.get("authorization").map(|h| h.to_str().unwrap() == "Bearer graph-token").unwrap_or(false)));
            assert!(!requests.iter().any(|r| r.url.path() == "/me/joinedTeams"));
        }

        #[tokio::test]
        async fn chat_messages_sync_incrementally_retry_a_429_and_keep_reactions_hosted_images_and_links() {
            let server = MockServer::start().await;
            let at = "/chats/19:abc@thread.v2/messages";
            Mock::given(method("GET")).and(path(at)).respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0")).up_to_n_times(1).with_priority(1).mount(&server).await;
            Mock::given(method("GET")).and(path(at)).respond_with(ResponseTemplate::new(200).set_body_json(fixture("chat-messages.json", &server.uri()))).mount(&server).await;
            Mock::given(method("GET")).and(path("/users/u-nora")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": "u-nora", "displayName": "Nora Gale", "mail": "nora@northwind.example" }))).mount(&server).await;
            let since = DateTime::parse_from_rfc3339("2026-10-15T07:00:00Z").unwrap().with_timezone(&Utc);
            let messages = graph(&server, false).sync_messages(&auth(), "19:abc@thread.v2", Some(since), None).await.unwrap();
            let listed: Vec<Request> = server.received_requests().await.unwrap().into_iter().filter(|r| r.url.path() == at).collect();
            assert_eq!(listed.len(), 2);
            let query: HashMap<String, String> = listed[1].url.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            assert_eq!(query.get("$filter").map(String::as_str), Some("lastModifiedDateTime gt 2026-10-15T07:00:00.000Z"));
            assert_eq!(query.get("$orderby").map(String::as_str), Some("lastModifiedDateTime desc"));
            let mut message = messages.into_iter().next().unwrap();
            assert_eq!(message.sender.email.as_deref(), Some("nora@northwind.example"));
            assert_eq!(message.metadata["web_url"], "https://teams.microsoft.com/l/message/19%3Aabc%40thread.v2/1700?context=%7B%22contextType%22%3A%22chat%22%7D");
            assert_eq!(message.metadata["reactions"], serde_json::json!([{ "emoji": ":+1:", "user": "u-me", "original": "like" }, { "emoji": ":rocket:", "user": "u-nora", "original": "🚀" }]));
            assert_eq!(message.attachments.len(), 1);
            assert_eq!(message.attachments[0].content_id.as_deref(), Some("aWQ9MQ"));
            crate::content::emoji::canonical_message(&mut message);
            assert!(message.content.starts_with("<p>Klar :+1:</p>"), "{}", message.content);
            assert!(message.metadata["original_content"].as_str().unwrap().contains("<emoji id=\"like\""));
        }

        #[tokio::test]
        async fn mail_delta_walks_pages_keeps_the_delta_link_and_restarts_when_it_expires() {
            let server = MockServer::start().await;
            let inbox = "/me/mailFolders/inbox/messages/delta";
            Mock::given(method("GET")).and(path(inbox)).and(query_param("$skiptoken", "next")).respond_with(ResponseTemplate::new(200).set_body_json(fixture("mail-delta-page-2.json", &server.uri()))).with_priority(1).mount(&server).await;
            Mock::given(method("GET")).and(path(inbox)).and(query_param("$deltatoken", "dt-inbox")).respond_with(ResponseTemplate::new(410).set_body_json(serde_json::json!({ "error": { "code": "SyncStateNotFound", "message": "gone" } }))).with_priority(1).mount(&server).await;
            Mock::given(method("GET")).and(path(inbox)).respond_with(ResponseTemplate::new(200).set_body_json(fixture("mail-delta-page-1.json", &server.uri()))).mount(&server).await;
            Mock::given(method("GET")).and(path("/me/mailFolders/sentitems/messages/delta")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "@odata.deltaLink": format!("{}/me/mailFolders/sentitems/messages/delta?$deltatoken=dt-sent", server.uri()), "value": [] }))).mount(&server).await;
            Mock::given(method("GET")).and(path("/me/mailFolders/archive/messages/delta")).respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({ "error": { "message": "missing" } }))).mount(&server).await;
            let provider = graph(&server, false);
            let since = DateTime::parse_from_rfc3339("2026-10-01T00:00:00Z").unwrap().with_timezone(&Utc);
            let delta = provider.get_delta(&auth(), &HashMap::new(), since).await.unwrap();
            assert_eq!(delta.changes, vec![
                RemoteChange { conversation_external_id: Some("mail:conv-1".into()), message_external_id: "AAMk-1".into(), is_read: Some(true), ..Default::default() },
                RemoteChange { conversation_external_id: Some("mail:conv-1".into()), message_external_id: "AAMk-2".into(), is_read: Some(false), ..Default::default() },
                RemoteChange { message_external_id: "AAMk-3".into(), deleted: true, ..Default::default() },
            ]);
            assert_eq!(delta.cursors.get("graph:mail:inbox").map(String::as_str), Some(format!("{}/me/mailFolders/inbox/messages/delta?$deltatoken=dt-inbox", server.uri()).as_str()));
            assert!(delta.cursors.contains_key("graph:mail:sentitems") && !delta.cursors.contains_key("graph:mail:archive"));
            let first: HashMap<String, String> = server.received_requests().await.unwrap().iter().find(|r| r.url.path() == inbox).unwrap().url.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            assert_eq!(first.get("$filter").map(String::as_str), Some("receivedDateTime ge 2026-10-01T00:00:00Z"));
            let again = provider.get_delta(&auth(), &delta.cursors, since).await.unwrap();
            assert!(!again.cursors.contains_key("graph:mail:inbox"));
            assert!(again.cursors.contains_key("graph:mail:sentitems"));
        }

        #[tokio::test]
        async fn writes_follow_the_graph_mechanisms_for_reactions_pins_members_names_read_state_and_invitations() {
            let server = MockServer::start().await;
            me(&server).await;
            let chat = "/chats/19:abc@thread.v2";
            for (verb, at, status) in [
                ("POST", format!("{}/messages/1700/setReaction", chat), 204),
                ("POST", format!("{}/messages/1700/unsetReaction", chat), 204),
                ("POST", format!("{}/pinnedMessages", chat), 201),
                ("DELETE", format!("{}/pinnedMessages/pin-1", chat), 204),
                ("POST", format!("{}/members", chat), 201),
                ("DELETE", format!("{}/members/mem-9", chat), 204),
                ("PATCH", chat.to_string(), 204),
                ("POST", format!("{}/markChatReadForUser", chat), 204),
                ("PATCH", "/me/messages/AAMk-1".to_string(), 200),
                ("POST", "/me/events/AAMkAGI2-ev-1/accept".to_string(), 202),
            ] {
                Mock::given(method(verb)).and(path(at)).respond_with(ResponseTemplate::new(status)).mount(&server).await;
            }
            Mock::given(method("GET")).and(path(format!("{}/pinnedMessages", chat))).respond_with(ResponseTemplate::new(200).set_body_json(fixture("pinned-messages.json", &server.uri()))).mount(&server).await;
            Mock::given(method("GET")).and(path(format!("{}/members", chat))).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "value": [{ "id": "mem-9", "userId": "u-anna" }] }))).mount(&server).await;
            Mock::given(method("GET")).and(path("/organization")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "value": [{ "id": "t1" }] }))).mount(&server).await;
            let view: Value = serde_json::from_str(&std::fs::read_to_string(format!("{}/tests/fixtures/calendar/graph-calendar-view.json", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap();
            Mock::given(method("GET")).and(path("/me/events/AAMkAGI2-ev-1")).respond_with(ResponseTemplate::new(200).set_body_json(view["value"][0].clone())).mount(&server).await;
            let provider = graph(&server, false);
            let auth = auth();
            provider.send_reaction(&auth, "19:abc@thread.v2", "1700", ":+1:", true).await.unwrap();
            provider.send_reaction(&auth, "19:abc@thread.v2", "1700", ":rocket:", false).await.unwrap();
            assert!(provider.send_reaction(&auth, "19:abc@thread.v2", "1700", ":party_parrot:", true).await.unwrap_err().contains("party_parrot"));
            assert_eq!(bodies(&server, "POST", &format!("{}/messages/1700/setReaction", chat)).await, vec![serde_json::json!({ "reactionType": "like" })]);
            assert_eq!(bodies(&server, "POST", &format!("{}/messages/1700/unsetReaction", chat)).await, vec![serde_json::json!({ "reactionType": "🚀" })]);
            assert!(provider.put_pin(&auth, "19:abc@thread.v2", "1700", true).await.unwrap());
            assert!(provider.put_pin(&auth, "19:abc@thread.v2", "1700", false).await.unwrap());
            assert_eq!(bodies(&server, "POST", &format!("{}/pinnedMessages", chat)).await[0]["message@odata.bind"], "https://graph.microsoft.com/v1.0/chats/19:abc@thread.v2/messages/1700");
            assert_eq!(provider.get_pins(&auth, "19:abc@thread.v2").await.unwrap(), Some(vec!["1700".to_string(), "1699".to_string()]));
            assert_eq!(provider.get_pins(&auth, "mail:conv-1").await.unwrap(), None);
            assert!(provider.put_member(&auth, "19:abc@thread.v2", "u-anna", true).await.unwrap());
            assert!(provider.put_member(&auth, "19:abc@thread.v2", "u-anna", false).await.unwrap());
            let added = &bodies(&server, "POST", &format!("{}/members", chat)).await[0];
            assert_eq!(added["@odata.type"], "#microsoft.graph.aadUserConversationMember");
            assert_eq!(added["user@odata.bind"], format!("{}/users('u-anna')", server.uri()));
            assert!(provider.put_conversation_name(&auth, "19:abc@thread.v2", "Kaffeklubben").await.unwrap());
            assert_eq!(bodies(&server, "PATCH", chat).await, vec![serde_json::json!({ "topic": "Kaffeklubben" })]);
            assert!(!provider.put_conversation_name(&auth, "channel:t:c", "x").await.unwrap());
            provider.mark_read(&auth, "19:abc@thread.v2", "1700", true).await.unwrap();
            assert_eq!(bodies(&server, "POST", &format!("{}/markChatReadForUser", chat)).await, vec![serde_json::json!({ "user": { "id": "u-me", "tenantId": "t1" } })]);
            provider.mark_read(&auth, "mail:conv-1", "AAMk-1", false).await.unwrap();
            assert_eq!(bodies(&server, "PATCH", "/me/messages/AAMk-1").await, vec![serde_json::json!({ "isRead": false })]);
            let answered = provider.put_calendar_response(&auth, "cal", "AAMkAGI2-ev-1", "accepted").await.unwrap();
            assert_eq!(answered.response_status.as_deref(), Some("accepted"));
            assert_eq!(bodies(&server, "POST", "/me/events/AAMkAGI2-ev-1/accept").await, vec![serde_json::json!({ "sendResponse": true })]);
            assert!(provider.put_calendar_response(&auth, "cal", "AAMkAGI2-ev-1", "maybe").await.is_err());
        }

        #[tokio::test]
        async fn files_upload_to_onedrive_and_post_as_reference_attachments_while_images_stay_hosted() {
            let server = MockServer::start().await;
            me(&server).await;
            Mock::given(method("PUT")).and(path("/me/drive/root:/Microsoft%20Teams%20Chat%20Files/rapport.pdf:/content")).respond_with(ResponseTemplate::new(201).set_body_json(fixture("drive-item.json", &server.uri()))).mount(&server).await;
            Mock::given(method("POST")).and(path("/chats/19:abc@thread.v2/messages")).respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({ "id": "1801", "createdDateTime": "2026-10-15T09:00:00Z" }))).mount(&server).await;
            let draft = OutgoingMessage {
                content: "Se vedlegg".into(),
                content_type: "text".into(),
                attachments: vec![
                    OutgoingAttachment { name: "rapport.pdf".into(), mime_type: "application/pdf".into(), bytes: b"%PDF".to_vec() },
                    OutgoingAttachment { name: "skjerm.png".into(), mime_type: "image/png".into(), bytes: vec![0x89, b'P', b'N', b'G'] },
                ],
                ..Default::default()
            };
            let sent = graph(&server, false).send_message(&auth(), "19:abc@thread.v2", &draft).await.unwrap();
            assert_eq!(sent.external_id, "1801");
            let posted = &bodies(&server, "POST", "/chats/19:abc@thread.v2/messages").await[0];
            assert_eq!(posted["attachments"][0], serde_json::json!({ "id": "153d8d3a-d6e8-4a21-8e8b-2ae7a8c1b5c9", "contentType": "reference", "contentUrl": "https://northwind-my.sharepoint.com/personal/milo/Documents/Microsoft%20Teams%20Chat%20Files/rapport.pdf", "name": "rapport.pdf" }));
            assert!(posted["body"]["content"].as_str().unwrap().ends_with("<attachment id=\"153d8d3a-d6e8-4a21-8e8b-2ae7a8c1b5c9\"></attachment>"));
            assert_eq!(posted["hostedContents"][0]["@microsoft.graph.temporaryId"], "2");
            let uploaded = server.received_requests().await.unwrap().into_iter().find(|r| r.method.as_str() == "PUT").unwrap();
            assert_eq!(uploaded.body, b"%PDF".to_vec());
            assert_eq!(uploaded.headers.get("content-type").unwrap(), "application/pdf");
        }

        #[tokio::test]
        async fn attachments_download_through_the_matching_graph_resource() {
            let server = MockServer::start().await;
            let reference = "https://northwind-my.sharepoint.com/personal/milo/Documents/rapport.pdf";
            for (at, body) in [
                ("/me/messages/AAMk-1/attachments/att-1/$value".to_string(), "mail"),
                ("/chats/19:abc@thread.v2/messages/1700/hostedContents/aWQ9MQ/$value".to_string(), "hosted"),
                (format!("/shares/u!{}/driveItem/content", URL_SAFE_NO_PAD.encode(reference)), "shared"),
            ] {
                Mock::given(method("GET")).and(path(at)).respond_with(ResponseTemplate::new(200).set_body_string(body)).mount(&server).await;
            }
            let provider = graph(&server, false);
            let file = |id: &str, url: Option<String>| ResponseAttachment { external_id: id.into(), name: "f".into(), content_type: "application/octet-stream".into(), size: 1, url, content_id: None };
            assert_eq!(provider.get_attachment(&auth(), "mail:conv-1", "AAMk-1", &file("att-1", None)).await.unwrap(), Some(b"mail".to_vec()));
            let hosted = format!("{}/chats/19:abc@thread.v2/messages/1700/hostedContents/aWQ9MQ/$value", server.uri());
            assert_eq!(provider.get_attachment(&auth(), "19:abc@thread.v2", "1700", &file("hosted:aWQ9MQ", Some(hosted))).await.unwrap(), Some(b"hosted".to_vec()));
            assert_eq!(provider.get_attachment(&auth(), "19:abc@thread.v2", "1700", &file("ref", Some(reference.to_string()))).await.unwrap(), Some(b"shared".to_vec()));
            assert_eq!(provider.get_attachment(&auth(), "19:abc@thread.v2", "1700", &file("x", None)).await.unwrap(), None);
        }

        #[tokio::test]
        async fn the_people_directory_brings_organisation_users_with_photos_and_contacts_by_address() {
            let server = MockServer::start().await;
            Mock::given(method("GET")).and(path("/me/people")).respond_with(ResponseTemplate::new(200).set_body_json(fixture("people.json", &server.uri()))).mount(&server).await;
            Mock::given(method("GET")).and(path("/users/u-anna/photos/64x64/$value")).respond_with(ResponseTemplate::new(200).insert_header("content-type", "image/jpeg").set_body_bytes(vec![0xff, 0xd8, 0xff])).mount(&server).await;
            let people = graph(&server, false).get_people(&auth()).await.unwrap();
            assert_eq!(people.len(), 2);
            assert_eq!((people[0].id.as_str(), people[0].email.as_deref(), people[0].job_title.as_deref(), people[0].department.as_deref()), ("u-anna", Some("anna.berg@client.example"), Some("Innkjøp"), None));
            assert_eq!(people[0].avatar_url.as_deref(), Some("data:image/jpeg;base64,/9j/"));
            assert_eq!((people[1].id.as_str(), people[1].avatar_url.as_deref()), ("ola@mail.example", None));
        }

        #[tokio::test]
        async fn status_messages_and_links_use_presence_and_web_links() {
            let server = MockServer::start().await;
            Mock::given(method("POST")).and(path("/me/presence/setStatusMessage")).and(body_partial_json(serde_json::json!({ "statusMessage": { "message": { "content": "🌴 Ferie", "contentType": "text" } } }))).respond_with(ResponseTemplate::new(200)).mount(&server).await;
            Mock::given(method("GET")).and(path("/me/messages/AAMk-1")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "webLink": "https://outlook.office365.com/owa/?ItemID=AAMk-1" }))).mount(&server).await;
            let provider = graph(&server, false);
            let until = DateTime::parse_from_rfc3339("2026-10-20T16:00:00Z").unwrap().with_timezone(&Utc);
            assert!(provider.put_status(&auth(), "Ferie", Some(":palm_tree:"), Some(until)).await.unwrap());
            let set = &bodies(&server, "POST", "/me/presence/setStatusMessage").await[0];
            assert_eq!(set["statusMessage"]["expiryDateTime"], serde_json::json!({ "dateTime": "2026-10-20T16:00:00", "timeZone": "UTC" }));
            assert_eq!(provider.get_message_link(&auth(), "mail:conv-1", "AAMk-1").await.unwrap().as_deref(), Some("https://outlook.office365.com/owa/?ItemID=AAMk-1"));
            assert_eq!(provider.get_message_link(&auth(), "19:abc@thread.v2", "1700").await.unwrap(), MicrosoftProvider::chat_web_url("19:abc@thread.v2", "1700"));
        }
    }
}
