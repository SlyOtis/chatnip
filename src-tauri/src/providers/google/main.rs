use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use reqwest::Client;
use serde_json::Value;
use base64::{Engine as _, engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD}};
use crate::providers::traits::{
    ProviderTrait, ConversationType, MessageType,
    ResponseConversation, ResponseMessage, ResponseUser, ConnectionIdentity, ResponseAttachment, OutgoingMessage, OutgoingAttachment,
    RemoteCalendar, RemoteEvent, NewRemoteEvent, Delta, RemoteChange,
    RemoteAlias, RemoteAutoReply, RemoteSearch
};
use crate::providers::models::{Delivery, Provider, ProviderCapabilities, ProviderAuth};
use std::collections::HashMap;

pub const SCOPES: [&str; 7] = [
    "https://www.googleapis.com/auth/gmail.modify",
    "https://www.googleapis.com/auth/calendar",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
    "https://www.googleapis.com/auth/contacts.readonly",
    "https://www.googleapis.com/auth/contacts.other.readonly",
    "https://www.googleapis.com/auth/gmail.settings.basic",
];
pub const GMAIL: &str = "https://gmail.googleapis.com/gmail/v1";
pub const CALENDAR: &str = "https://www.googleapis.com/calendar/v3";
pub const PEOPLE: &str = "https://people.googleapis.com/v1";
pub const USERINFO: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
pub const TOKEN: &str = "https://oauth2.googleapis.com/token";
pub const HISTORY_CURSOR: &str = "gmail:history";
const PEOPLE_FIELDS: &str = "names,emailAddresses,photos,organizations,phoneNumbers";

pub struct GoogleProvider {
    provider: Provider,
    client: Client,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    gmail: String,
    calendar: String,
    people: String,
    userinfo: String,
    token: String,
    labels: tokio::sync::OnceCell<HashMap<String, String>>,
    account: tokio::sync::OnceCell<String>,
}

impl GoogleProvider {
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        let provider = Provider {
            id: "google".to_string(),
            profile_id: String::new(),
            provider_type: "google".to_string(),
            name: "Google".to_string(),
            external_id: None,
            logo_id: None,
            logo: None,
            capabilities: ProviderCapabilities {
                can_send_messages: true,
                can_edit_messages: false,
                can_delete_messages: true,
                can_create_conversations: true,
                can_add_members: false,
                supports_reactions: false,
                supports_threads: true,
                supports_attachments: true,
                supports_presence: false,
                supports_typing_indicators: false,
                supports_read_receipts: false,
                supports_reply_quote: true,
                supports_forward: true,
                reply_quote: Delivery::Mail,
                forward: Delivery::Mail,
                supports_calendar_responses: true,
                supports_unread_sync: true,
                drafts: Delivery::Native,
                archive: Delivery::Native,
                supports_flags: true,
                supports_search: true,
                supports_aliases: true,
                signature_sync: true,
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
            gmail: GMAIL.to_string(),
            calendar: std::env::var("GOOGLE_CALENDAR_ENDPOINT").unwrap_or_else(|_| CALENDAR.to_string()),
            people: PEOPLE.to_string(),
            userinfo: USERINFO.to_string(),
            token: TOKEN.to_string(),
            labels: tokio::sync::OnceCell::new(),
            account: tokio::sync::OnceCell::new(),
        }
    }

    pub fn with_api(mut self, base: &str) -> Self {
        let base = base.trim_end_matches('/');
        self.gmail = format!("{}/gmail/v1", base);
        self.calendar = format!("{}/calendar/v3", base);
        self.people = format!("{}/people/v1", base);
        self.userinfo = format!("{}/oauth2/v2/userinfo", base);
        self.token = format!("{}/token", base);
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

    pub async fn fetch_current_user(&self, auth: &ProviderAuth) -> Result<ResponseUser, String> {
        let data = self.get_json(auth, &self.userinfo).await?;
        Ok(ResponseUser {
            id: data["id"].as_str().unwrap_or("").to_string(),
            email: data["email"].as_str().map(String::from),
            phone: None,
            display_name: data["name"].as_str().map(String::from),
            first_name: data["given_name"].as_str().map(String::from),
            last_name: data["family_name"].as_str().map(String::from),
            avatar_url: data["picture"].as_str().map(String::from),
            job_title: None,
            department: None,
            company: None,
            office_location: None,
        })
    }

    fn extract_gmail_body(payload: &Value) -> (String, &'static str) {
        match crate::content::mail::gmail_bodies(payload) {
            (Some(html), _, _) => (html, "html"),
            (None, Some(plain), _) => (plain, "text"),
            _ => (String::new(), "text"),
        }
    }

    fn get_header_value(headers: &Value, name: &str) -> Option<String> {
        headers.as_array()
            .and_then(|arr| {
                arr.iter()
                    .find(|h| h["name"].as_str().map(|n| n.eq_ignore_ascii_case(name)).unwrap_or(false))
                    .and_then(|h| h["value"].as_str().map(String::from))
            })
    }

    fn parse_email_sender(from: &str) -> ResponseUser {
        let (display_name, email) = if let Some(angle_start) = from.find('<') {
            let name = from[..angle_start].trim().trim_matches('"').to_string();
            let email = from[angle_start + 1..].trim_end_matches('>').to_string();
            (if name.is_empty() { None } else { Some(name) }, Some(email))
        } else {
            (None, Some(from.to_string()))
        };
        ResponseUser {
            id: email.clone().unwrap_or_default(),
            email,
            display_name,
            ..Default::default()
        }
    }

    fn with_reply_headers(raw: &str, references: Option<&(String, String)>) -> String {
        match references {
            Some((id, chain)) => raw.replacen("\r\n\r\n", &format!("\r\nIn-Reply-To: {}\r\nReferences: {}\r\n\r\n", id, chain), 1),
            None => raw.to_string(),
        }
    }
    fn build_rfc2822_message(to: &str, subject: &str, content: &str, content_type: &str, html: Option<&str>) -> String {
        if let Some(html) = html {
            let boundary = format!("chatnip-alt-{}", Utc::now().timestamp_nanos_opt().unwrap_or_default());
            return format!(
                "To: {}\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"{}\"\r\n\r\n--{}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}\r\n--{}\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{}\r\n--{}--\r\n",
                to, subject, boundary, boundary, content, boundary, html, boundary
            );
        }
        let mime_type = if content_type == "html" { "text/html" } else { "text/plain" };
        format!(
            "To: {}\r\nContent-Type: {}; charset=utf-8\r\nSubject: {}\r\n\r\n{}",
            to, mime_type, subject, content
        )
    }

    fn with_attachments(raw: &str, attachments: &[OutgoingAttachment]) -> String {
        let (headers, body) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
        let boundary = format!("chatnip-{}", Utc::now().timestamp_nanos_opt().unwrap_or_default());
        let mut text_type = "text/plain; charset=utf-8".to_string();
        let mut kept = Vec::new();
        for line in headers.split("\r\n") {
            if let Some(value) = line.strip_prefix("Content-Type: ") {
                text_type = value.to_string();
            } else if !line.starts_with("MIME-Version:") {
                kept.push(line);
            }
        }
        let mut out = format!("{}\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"{}\"\r\n\r\n", kept.join("\r\n"), boundary);
        out.push_str(&format!("--{}\r\nContent-Type: {}\r\n\r\n{}\r\n", boundary, text_type, body));
        for attachment in attachments {
            let encoded = STANDARD.encode(&attachment.bytes);
            let wrapped: Vec<&str> = encoded.as_bytes().chunks(76).map(|c| std::str::from_utf8(c).unwrap_or("")).collect();
            out.push_str(&format!(
                "--{}\r\nContent-Type: {}; name=\"{}\"\r\nContent-Disposition: attachment; filename=\"{}\"\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n",
                boundary, attachment.mime_type, attachment.name, attachment.name, wrapped.join("\r\n")
            ));
        }
        out.push_str(&format!("--{}--\r\n", boundary));
        out
    }

    pub fn google_calendar(item: &Value) -> RemoteCalendar {
        RemoteCalendar {
            external_id: item["id"].as_str().unwrap_or("").to_string(),
            name: item["summaryOverride"].as_str().or_else(|| item["summary"].as_str()).unwrap_or("Calendar").to_string(),
            colour: item["backgroundColor"].as_str().map(String::from),
            is_primary: item["primary"].as_bool().unwrap_or(false),
            can_write: matches!(item["accessRole"].as_str(), Some("owner") | Some("writer")),
        }
    }

    pub fn google_event(item: &Value) -> Option<RemoteEvent> {
        let all_day = item["start"]["date"].is_string();
        let starts_at = Self::parse_calendar_datetime(&item["start"])?;
        let ends_at = Self::parse_calendar_datetime(&item["end"]).unwrap_or(starts_at);
        let attendees = item["attendees"].as_array().cloned().unwrap_or_default();
        let join = item["hangoutLink"].as_str().map(String::from).or_else(|| {
            item["conferenceData"]["entryPoints"].as_array()?.iter().find(|e| e["entryPointType"] == "video").and_then(|e| e["uri"].as_str()).map(String::from)
        });
        Some(RemoteEvent {
            external_id: item["id"].as_str()?.to_string(),
            ical_uid: item["iCalUID"].as_str().filter(|u| !u.is_empty()).map(String::from),
            title: item["summary"].as_str().unwrap_or("").to_string(),
            notes: item["description"].as_str().unwrap_or("").to_string(),
            location: item["location"].as_str().unwrap_or("").to_string(),
            starts_at,
            ends_at,
            all_day,
            attendees: attendees.iter().filter_map(|a| a["email"].as_str().map(|e| e.to_lowercase())).collect(),
            organizer: item["organizer"]["email"].as_str().map(|e| e.to_lowercase()),
            response_status: attendees.iter().find(|a| a["self"].as_bool() == Some(true)).and_then(|a| a["responseStatus"].as_str()).map(String::from),
            online_meeting_url: join,
            cancelled: item["status"].as_str() == Some("cancelled"),
            updated_at: item["updated"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc)).unwrap_or(starts_at),
        })
    }

    pub fn google_event_body(event: &NewRemoteEvent, request_id: &str) -> Value {
        let time = |at: DateTime<Utc>, end: bool| {
            if event.all_day {
                let day = if end && at.time() == chrono::NaiveTime::MIN { at.date_naive() } else if end { at.date_naive() + Duration::days(1) } else { at.date_naive() };
                serde_json::json!({ "date": day.format("%Y-%m-%d").to_string() })
            } else {
                serde_json::json!({ "dateTime": at.to_rfc3339(), "timeZone": "UTC" })
            }
        };
        let mut body = serde_json::json!({
            "summary": event.title,
            "description": event.notes,
            "location": event.location,
            "start": time(event.starts_at, false),
            "end": time(event.ends_at, true),
            "attendees": event.attendees.iter().map(|a| serde_json::json!({ "email": a })).collect::<Vec<_>>(),
        });
        if event.online_meeting {
            body["conferenceData"] = serde_json::json!({ "createRequest": { "requestId": request_id, "conferenceSolutionKey": { "type": "hangoutsMeet" } } });
        }
        body
    }

    pub fn response_status(response: &str) -> Result<&'static str, String> {
        match response {
            "accepted" | "accept" => Ok("accepted"),
            "declined" | "decline" => Ok("declined"),
            "tentativelyAccepted" | "tentative" => Ok("tentative"),
            other => Err(format!("{} is not an answer to an invitation; use accepted, declined or tentativelyAccepted", other)),
        }
    }

    pub fn answered_attendees(event: &Value, status: &str) -> Option<Value> {
        let mut attendees = event["attendees"].as_array().cloned()?;
        let me = attendees.iter_mut().find(|a| a["self"].as_bool() == Some(true))?;
        me["responseStatus"] = Value::String(status.to_string());
        Some(Value::Array(attendees))
    }

    async fn calendar_json(response: reqwest::Response) -> Result<Value, String> {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            let detail = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["error"]["message"].as_str().map(String::from)).unwrap_or(text);
            return Err(format!("Google {}: {}", status, detail));
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|e| format!("Google returned unreadable JSON: {}", e))
    }

    async fn get_json(&self, auth: &ProviderAuth, url: &str) -> Result<Value, String> {
        Self::calendar_json(crate::providers::http::send(self.client.get(url).bearer_auth(&auth.access_token)).await?).await
    }

    async fn post_json(&self, auth: &ProviderAuth, url: &str, body: &Value) -> Result<Value, String> {
        Self::calendar_json(crate::providers::http::send(self.client.post(url).bearer_auth(&auth.access_token).json(body)).await?).await
    }

    async fn send_json(&self, auth: &ProviderAuth, method: reqwest::Method, url: &str, body: &Value) -> Result<Value, String> {
        let request = self.client.request(method, url).bearer_auth(&auth.access_token);
        let request = if body.is_null() { request } else { request.json(body) };
        Self::calendar_json(crate::providers::http::send(request).await?).await
    }

    fn parse_calendar_datetime(dt_obj: &Value) -> Option<DateTime<Utc>> {
        dt_obj["dateTime"].as_str()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|| {
                dt_obj["date"].as_str()
                    .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                    .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
            })
    }

    pub fn thread_url(account: &str, thread_id: &str) -> String {
        format!("https://mail.google.com/mail/?authuser={}#all/{}", urlencoding::encode(account), thread_id)
    }

    pub fn gmail_message(msg: &Value, conversation_external_id: &str, labels: &HashMap<String, String>, account: Option<&str>) -> ResponseMessage {
        let headers = &msg["payload"]["headers"];
        let people = |name: &str| -> Vec<ResponseUser> {
            Self::get_header_value(headers, name)
                .map(|v| v.split(',').map(|p| Self::parse_email_sender(p.trim())).filter(|u| u.email.is_some()).collect())
                .unwrap_or_default()
        };
        let ids: Vec<String> = msg["labelIds"].as_array().cloned().unwrap_or_default().iter().filter_map(|l| l.as_str().map(String::from)).collect();
        let (content, content_type) = Self::extract_gmail_body(&msg["payload"]);
        let raw_headers = crate::content::mail::gmail_headers(&msg["payload"]);
        let (html, plain, calendar) = crate::content::mail::gmail_bodies(&msg["payload"]);
        let structure = crate::content::mail::analyse(html.as_deref(), plain.as_deref(), Some(&raw_headers), calendar.as_deref());
        let mut metadata = HashMap::new();
        metadata.insert("labels".to_string(), serde_json::json!(ids));
        let names: Vec<String> = ids.iter().filter_map(|id| labels.get(id).cloned()).collect();
        if !names.is_empty() {
            metadata.insert("label_names".to_string(), serde_json::json!(names));
        }
        if let Some(id) = Self::get_header_value(headers, "Message-ID") {
            metadata.insert("internet_message_id".to_string(), Value::String(id));
        }
        if let (Some(account), Some(thread)) = (account, msg["threadId"].as_str()) {
            metadata.insert("web_url".to_string(), Value::String(Self::thread_url(account, thread)));
        }
        let origin = crate::content::headers::origin_ip(&raw_headers).map(|ip| crate::providers::traits::MessageOrigin { ip: Some(ip.to_string()), ..Default::default() });
        let mut message = ResponseMessage {
            external_id: msg["id"].as_str().unwrap_or("").to_string(),
            conversation_external_id: conversation_external_id.to_string(),
            sender: Self::parse_email_sender(&Self::get_header_value(headers, "From").unwrap_or_default()),
            content,
            content_type: content_type.to_string(),
            timestamp: msg["internalDate"].as_str().and_then(|ts| ts.parse::<i64>().ok()).and_then(|ms| DateTime::from_timestamp(ms / 1000, 0)).unwrap_or_else(Utc::now),
            r#type: MessageType::Email,
            metadata,
            subject: Self::get_header_value(headers, "Subject"),
            recipients: people("To"),
            cc: people("Cc"),
            is_read: !ids.iter().any(|l| l == "UNREAD"),
            reply_to_id: Self::get_header_value(headers, "In-Reply-To"),
            attachments: Self::collect_attachments(&msg["payload"]),
            headers: Some(raw_headers),
            origin,
            ..Default::default()
        };
        structure.apply(&mut message);
        message
    }

    pub fn history_threads(data: &Value) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for entry in data["history"].as_array().cloned().unwrap_or_default() {
            for key in ["messagesAdded", "messagesDeleted", "labelsAdded", "labelsRemoved"] {
                for item in entry[key].as_array().cloned().unwrap_or_default() {
                    if let Some(thread) = item["message"]["threadId"].as_str().map(|t| format!("gmail:{}", t)).filter(|t| !out.contains(t)) {
                        out.push(thread);
                    }
                }
            }
        }
        out
    }

    pub fn history_changes(data: &Value) -> Vec<RemoteChange> {
        let mut out: Vec<RemoteChange> = Vec::new();
        for entry in data["history"].as_array().cloned().unwrap_or_default() {
            for removed in entry["messagesDeleted"].as_array().cloned().unwrap_or_default() {
                if let Some(id) = removed["message"]["id"].as_str() {
                    out.push(RemoteChange { message_external_id: id.to_string(), deleted: true, ..Default::default() });
                }
            }
            for key in ["labelsAdded", "labelsRemoved"] {
                for item in entry[key].as_array().cloned().unwrap_or_default() {
                    let message = &item["message"];
                    let Some(id) = message["id"].as_str() else { continue };
                    let labels: Vec<String> = message["labelIds"].as_array().cloned().unwrap_or_default().iter().filter_map(|l| l.as_str().map(String::from)).collect();
                    let unread_touched = item["labelIds"].as_array().is_some_and(|l| l.iter().any(|v| v == "UNREAD"));
                    let trashed = labels.iter().any(|l| l == "TRASH");
                    out.retain(|c| c.message_external_id != id || c.deleted);
                    out.push(RemoteChange {
                        conversation_external_id: message["threadId"].as_str().map(|t| format!("gmail:{}", t)),
                        message_external_id: id.to_string(),
                        is_read: unread_touched.then(|| !labels.iter().any(|l| l == "UNREAD")),
                        deleted: trashed,
                        flagged: Some(labels.iter().any(|l| l == "STARRED")),
                        labels: Some(labels),
                        ..Default::default()
                    });
                }
            }
        }
        out
    }

    pub fn people_users(data: &Value, key: &str) -> Vec<ResponseUser> {
        data[key]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|person| {
                let email = person["emailAddresses"].as_array()?.first()?["value"].as_str()?.to_lowercase();
                let name = person["names"].as_array().and_then(|n| n.first()).cloned().unwrap_or(Value::Null);
                Some(ResponseUser {
                    id: email.clone(),
                    email: Some(email),
                    phone: person["phoneNumbers"].as_array().and_then(|p| p.first()).and_then(|p| p["value"].as_str()).map(String::from),
                    display_name: name["displayName"].as_str().map(String::from),
                    first_name: name["givenName"].as_str().map(String::from),
                    last_name: name["familyName"].as_str().map(String::from),
                    avatar_url: person["photos"].as_array().and_then(|p| p.iter().find(|ph| ph["default"].as_bool() != Some(true))).and_then(|p| p["url"].as_str()).map(String::from),
                    job_title: person["organizations"].as_array().and_then(|o| o.first()).and_then(|o| o["title"].as_str()).map(String::from),
                    company: person["organizations"].as_array().and_then(|o| o.first()).and_then(|o| o["name"].as_str()).map(String::from),
                    ..Default::default()
                })
            })
            .collect()
    }

    async fn label_names(&self, auth: &ProviderAuth) -> HashMap<String, String> {
        self.labels
            .get_or_init(|| async {
                match self.get_json(auth, &format!("{}/users/me/labels", self.gmail)).await {
                    Ok(data) => data["labels"].as_array().cloned().unwrap_or_default().iter().filter_map(|l| Some((l["id"].as_str()?.to_string(), l["name"].as_str()?.to_string()))).collect(),
                    Err(_) => HashMap::new(),
                }
            })
            .await
            .clone()
    }

    async fn account_email(&self, auth: &ProviderAuth) -> Option<String> {
        self.account
            .get_or_try_init(|| async { self.get_json(auth, &format!("{}/users/me/profile", self.gmail)).await.and_then(|p| p["emailAddress"].as_str().map(String::from).ok_or_else(|| "no address".to_string())) })
            .await
            .ok()
            .cloned()
    }
}

#[async_trait]
impl ProviderTrait for GoogleProvider {
    fn get_provider(&self) -> &Provider {
        &self.provider
    }

    async fn get_auth_url(&self, state: &str, code_challenge: Option<&str>) -> Result<String, String> {
        let mut auth_url = format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&access_type=offline&prompt=consent",
            self.client_id,
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(&SCOPES.join(" ")),
            state
        );
        if let Some(challenge) = code_challenge {
            auth_url.push_str(&format!("&code_challenge={}&code_challenge_method=S256", challenge));
        }
        Ok(auth_url)
    }

    async fn identity(&self, auth: &ProviderAuth) -> Result<ConnectionIdentity, String> {
        let me = self.fetch_current_user(auth).await?;
        let email = me.email.clone().unwrap_or_else(|| me.id.clone());
        Ok(ConnectionIdentity {
            namespace: email.clone(),
            name: email,
            external_id: Some(me.id.clone()),
            logo_url: me.avatar_url.clone(),
            user: Some(me),
        })
    }

    async fn fetch_logo(&self, _auth: &ProviderAuth, identity: &ConnectionIdentity) -> Result<Option<(Vec<u8>, String)>, String> {
        let Some(url) = &identity.logo_url else { return Ok(None) };
        let response = crate::providers::http::send(self.client.get(url)).await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("image/png").to_string();
        let bytes = response.bytes().await.map_err(|e| format!("Failed to read the account picture: {}", e))?;
        Ok(Some((bytes.to_vec(), content_type)))
    }

    async fn exchange_code(&self, code: &str, code_verifier: Option<&str>) -> Result<ProviderAuth, String> {
        let mut params = vec![
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ];
        if let Some(verifier) = code_verifier {
            params.push(("code_verifier", verifier));
        }
        self.token_call(&params, None).await
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<ProviderAuth, String> {
        let params = [
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];
        self.token_call(&params, Some(refresh_token)).await
    }

    async fn validate_tokens(&self, auth: &ProviderAuth) -> Result<bool, String> {
        if auth.expires_at < Utc::now() + Duration::minutes(5) {
            return Ok(false);
        }
        let response = crate::providers::http::send(self.client.get(&self.userinfo).bearer_auth(&auth.access_token)).await?;
        Ok(response.status().is_success())
    }

    async fn sync_conversations(
        &self,
        auth: &ProviderAuth,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<ResponseConversation>, String> {
        let floor = since.unwrap_or_else(|| Utc::now() - Duration::days(30));
        let account = self.account_email(auth).await;
        let mut conversations = Vec::new();
        let mut page: Option<String> = None;
        loop {
            let mut url = format!("{}/users/me/threads?maxResults=100&q=after:{}", self.gmail, floor.timestamp());
            if let Some(token) = &page {
                url.push_str(&format!("&pageToken={}", urlencoding::encode(token)));
            }
            let data = self.get_json(auth, &url).await?;
            for thread in data["threads"].as_array().cloned().unwrap_or_default() {
                let thread_id = thread["id"].as_str().unwrap_or("").to_string();
                let snippet = thread["snippet"].as_str().unwrap_or("").to_string();
                let detail = self
                    .get_json(auth, &format!("{}/users/me/threads/{}?format=metadata&metadataHeaders=Subject&metadataHeaders=From&metadataHeaders=To", self.gmail, thread_id))
                    .await
                    .unwrap_or(Value::Null);
                let messages = detail["messages"].as_array().cloned().unwrap_or_default();
                let subject = messages
                    .first()
                    .and_then(|m| Self::get_header_value(&m["payload"]["headers"], "Subject"))
                    .unwrap_or_else(|| snippet.clone());
                let last_activity = messages
                    .last()
                    .and_then(|m| m["internalDate"].as_str())
                    .and_then(|ts| ts.parse::<i64>().ok())
                    .and_then(|ms| DateTime::from_timestamp(ms / 1000, 0))
                    .unwrap_or_else(Utc::now);
                let mut members: HashMap<String, ResponseUser> = HashMap::new();
                for m in &messages {
                    for header in ["From", "To"] {
                        if let Some(value) = Self::get_header_value(&m["payload"]["headers"], header) {
                            for part in value.split(',') {
                                let user = Self::parse_email_sender(part.trim());
                                if let Some(email) = &user.email {
                                    members.entry(email.clone()).or_insert(user);
                                }
                            }
                        }
                    }
                }
                let mut metadata = HashMap::new();
                if let Some(account) = &account {
                    metadata.insert("web_url".to_string(), Value::String(Self::thread_url(account, &thread_id)));
                }
                conversations.push(ResponseConversation {
                    external_id: format!("gmail:{}", thread_id),
                    name: subject,
                    conversation_type: ConversationType::Email,
                    members: members.into_values().collect(),
                    last_activity,
                    metadata,
                });
            }
            page = data["nextPageToken"].as_str().map(String::from);
            if page.is_none() {
                break;
            }
        }
        Ok(conversations)
    }

    async fn sync_messages(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        _since: Option<DateTime<Utc>>,
        limit: Option<u32>,
    ) -> Result<Vec<ResponseMessage>, String> {
        let limit = limit.unwrap_or(500) as usize;
        let thread_id = conversation_external_id.strip_prefix("gmail:")
            .ok_or_else(|| format!("Invalid Gmail conversation id: {}", conversation_external_id))?;
        let data = self.get_json(auth, &format!("{}/users/me/threads/{}?format=full", self.gmail, thread_id)).await?;
        let labels = self.label_names(auth).await;
        let account = self.account_email(auth).await;
        let mut messages: Vec<ResponseMessage> = data["messages"].as_array().cloned().unwrap_or_default().iter().take(limit).map(|msg| Self::gmail_message(msg, conversation_external_id, &labels, account.as_deref())).collect();
        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(messages)
    }

    async fn send_message(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        draft: &OutgoingMessage,
    ) -> Result<ResponseMessage, String> {
        let current_user = self.fetch_current_user(auth).await?;
        let forwarding = !draft.forwarded.is_empty();
        let thread_id = conversation_external_id.strip_prefix("gmail:").filter(|_| !forwarding);
        let mut to = draft.to.clone();
        let mut subject = draft.subject.clone().unwrap_or_default();
        let mut references: Option<(String, String)> = None;
        if let Some(quote) = draft.quote.as_ref().filter(|_| !forwarding) {
            references = quote.reply_headers();
            if to.is_empty() {
                to.extend(quote.sender_email.clone());
            }
            if subject.is_empty() {
                subject = crate::providers::traits::prefixed_subject("Re", quote.subject.as_deref().unwrap_or_default());
            }
        }
        if let (Some(thread_id), true) = (thread_id, to.is_empty() || subject.is_empty() || references.is_none()) {
            let detail = self
                .get_json(auth, &format!("{}/users/me/threads/{}?format=metadata&metadataHeaders=To&metadataHeaders=From&metadataHeaders=Subject&metadataHeaders=Message-ID", self.gmail, thread_id))
                .await?;
            let last = detail["messages"].as_array().and_then(|m| m.last()).cloned().unwrap_or(Value::Null);
            if to.is_empty() {
                if let Some(from) = Self::get_header_value(&last["payload"]["headers"], "From") {
                    to.push(from);
                }
            }
            if subject.is_empty() {
                if let Some(s) = Self::get_header_value(&last["payload"]["headers"], "Subject") {
                    subject = if s.starts_with("Re:") { s } else { format!("Re: {}", s) };
                }
            }
            if references.is_none() {
                references = Self::get_header_value(&last["payload"]["headers"], "Message-ID").map(|id| (id.clone(), id));
            }
        }
        if to.is_empty() {
            return Err("A new mail needs at least one recipient".to_string());
        }
        let mail_body = draft.mail_text();
        let (ct, content, html) = match draft.content_type.as_str() {
            "markdown" => ("markdown", crate::content::markdown::plain(&mail_body), Some(draft.mail_html().unwrap_or_else(|| crate::content::markdown::html(&mail_body)))),
            "html" => ("html", mail_body.clone(), None),
            _ => ("text", mail_body.clone(), None),
        };
        let mut raw = Self::build_rfc2822_message(&to.join(", "), &subject, &content, ct, html.as_deref());
        if !draft.cc.is_empty() {
            raw = raw.replacen("\r\n\r\n", &format!("\r\nCc: {}\r\n\r\n", draft.cc.join(", ")), 1);
        }
        if let Some(from) = draft.from.as_ref() {
            raw = format!("From: {}\r\n{}", from, raw);
        }
        raw = Self::with_reply_headers(&raw, references.as_ref());
        if !draft.attachments.is_empty() {
            raw = Self::with_attachments(&raw, &draft.attachments);
        }
        let mut body = serde_json::json!({ "raw": URL_SAFE_NO_PAD.encode(raw.as_bytes()) });
        if let Some(thread_id) = thread_id {
            body["threadId"] = Value::String(thread_id.to_string());
        }
        let data = self.post_json(auth, &format!("{}/users/me/messages/send", self.gmail), &body).await?;
        Ok(ResponseMessage {
            external_id: data["id"].as_str().unwrap_or("").to_string(),
            conversation_external_id: conversation_external_id.to_string(),
            sender: current_user,
            content: mail_body,
            content_type: ct.to_string(),
            timestamp: Utc::now(),
            r#type: MessageType::Email,
            subject: Some(subject),
            recipients: to.iter().map(|a| ResponseUser { id: a.clone(), email: Some(a.clone()), ..Default::default() }).collect(),
            cc: draft.cc.iter().map(|a| ResponseUser { id: a.clone(), email: Some(a.clone()), ..Default::default() }).collect(),
            attachments: draft
                .attachments
                .iter()
                .enumerate()
                .map(|(i, a)| ResponseAttachment { external_id: format!("{}", i + 1), name: a.name.clone(), content_type: a.mime_type.clone(), size: a.bytes.len() as u64, url: None, content_id: None })
                .collect(),
            ..Default::default()
        })
    }

    async fn mark_read(
        &self,
        auth: &ProviderAuth,
        _conversation_external_id: &str,
        message_external_id: &str,
        read: bool,
    ) -> Result<(), String> {
        let change = if read { serde_json::json!({ "removeLabelIds": ["UNREAD"] }) } else { serde_json::json!({ "addLabelIds": ["UNREAD"] }) };
        self.post_json(auth, &format!("{}/users/me/messages/{}/modify", self.gmail, message_external_id), &change).await.map(|_| ())
    }

    async fn delete_message(
        &self,
        auth: &ProviderAuth,
        _conversation_external_id: &str,
        message_external_id: &str,
    ) -> Result<(), String> {
        self.post_json(auth, &format!("{}/users/me/messages/{}/trash", self.gmail, message_external_id), &serde_json::json!({})).await.map(|_| ())
    }

    async fn get_calendars(&self, auth: &ProviderAuth) -> Result<Vec<RemoteCalendar>, String> {
        let data = self.get_json(auth, &format!("{}/users/me/calendarList", self.calendar)).await?;
        Ok(data["items"].as_array().cloned().unwrap_or_default().iter().map(Self::google_calendar).collect())
    }

    async fn get_calendar_events(&self, auth: &ProviderAuth, calendar_external_id: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<RemoteEvent>, String> {
        let base = format!(
            "{}/calendars/{}/events?timeMin={}&timeMax={}&singleEvents=true&orderBy=startTime&maxResults=250",
            self.calendar,
            urlencoding::encode(calendar_external_id),
            urlencoding::encode(&from.to_rfc3339()),
            urlencoding::encode(&to.to_rfc3339())
        );
        let mut events = Vec::new();
        let mut token: Option<String> = None;
        for _ in 0..20 {
            let url = match &token {
                Some(t) => format!("{}&pageToken={}", base, urlencoding::encode(t)),
                None => base.clone(),
            };
            let data = self.get_json(auth, &url).await?;
            events.extend(data["items"].as_array().cloned().unwrap_or_default().iter().filter_map(Self::google_event));
            token = data["nextPageToken"].as_str().map(String::from);
            if token.is_none() {
                break;
            }
        }
        Ok(events)
    }

    async fn post_calendar_event(&self, auth: &ProviderAuth, calendar_external_id: &str, event: &NewRemoteEvent) -> Result<RemoteEvent, String> {
        let url = format!("{}/calendars/{}/events?conferenceDataVersion=1&sendUpdates=all", self.calendar, urlencoding::encode(calendar_external_id));
        let body = Self::google_event_body(event, &uuid::Uuid::new_v4().to_string());
        let created = self.post_json(auth, &url, &body).await?;
        Self::google_event(&created).ok_or_else(|| "Google returned an event it could not describe".to_string())
    }

    async fn put_calendar_response(&self, auth: &ProviderAuth, calendar_external_id: &str, event_external_id: &str, response: &str) -> Result<RemoteEvent, String> {
        let status = Self::response_status(response)?;
        let url = format!("{}/calendars/{}/events/{}", self.calendar, urlencoding::encode(calendar_external_id), urlencoding::encode(event_external_id));
        let event = self.get_json(auth, &url).await?;
        let attendees = Self::answered_attendees(&event, status).ok_or_else(|| "You are not invited to this event; there is no invitation to answer".to_string())?;
        let response = crate::providers::http::send(self.client.patch(format!("{}?sendUpdates=all", url)).bearer_auth(&auth.access_token).json(&serde_json::json!({ "attendees": attendees }))).await?;
        let answered = Self::calendar_json(response).await?;
        Self::google_event(&answered).ok_or_else(|| "Google returned an event it could not describe".to_string())
    }

    async fn get_people(&self, auth: &ProviderAuth) -> Result<Vec<ResponseUser>, String> {
        let mut people = Vec::new();
        for (path, key, fields) in [("people/me/connections", "connections", "personFields"), ("otherContacts", "otherContacts", "readMask")] {
            let mut page: Option<String> = None;
            loop {
                let mut url = format!("{}/{}?{}={}&pageSize=1000", self.people, path, fields, if key == "connections" { PEOPLE_FIELDS } else { "names,emailAddresses,photos,phoneNumbers" });
                if let Some(token) = &page {
                    url.push_str(&format!("&pageToken={}", urlencoding::encode(token)));
                }
                let data = self.get_json(auth, &url).await?;
                people.extend(Self::people_users(&data, key));
                page = data["nextPageToken"].as_str().map(String::from);
                if page.is_none() {
                    break;
                }
            }
        }
        Ok(people)
    }

    async fn get_delta(&self, auth: &ProviderAuth, cursors: &HashMap<String, String>, _since: DateTime<Utc>) -> Result<Delta, String> {
        let mut out = Delta { cursors: cursors.clone(), ..Default::default() };
        let Some(start) = cursors.get(HISTORY_CURSOR).cloned() else {
            let profile = self.get_json(auth, &format!("{}/users/me/profile", self.gmail)).await?;
            if let Some(history) = profile["historyId"].as_str().map(String::from).or_else(|| profile["historyId"].as_u64().map(|n| n.to_string())) {
                out.cursors.insert(HISTORY_CURSOR.to_string(), history);
            }
            return Ok(out);
        };
        let mut page: Option<String> = None;
        loop {
            let mut url = format!("{}/users/me/history?startHistoryId={}&historyTypes=messageAdded&historyTypes=labelAdded&historyTypes=labelRemoved&historyTypes=messageDeleted&maxResults=500", self.gmail, start);
            if let Some(token) = &page {
                url.push_str(&format!("&pageToken={}", urlencoding::encode(token)));
            }
            let data = match self.get_json(auth, &url).await {
                Ok(data) => data,
                Err(e) if e.contains("404") => {
                    out.cursors.remove(HISTORY_CURSOR);
                    return Ok(out);
                }
                Err(e) => return Err(e),
            };
            out.changes.extend(Self::history_changes(&data));
            for thread in Self::history_threads(&data) {
                if !out.conversations.contains(&thread) {
                    out.conversations.push(thread);
                }
            }
            if let Some(history) = data["historyId"].as_str() {
                out.cursors.insert(HISTORY_CURSOR.to_string(), history.to_string());
            }
            page = data["nextPageToken"].as_str().map(String::from);
            if page.is_none() {
                break;
            }
        }
        out.complete = true;
        Ok(out)
    }

    async fn get_attachment(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str, attachment: &ResponseAttachment) -> Result<Option<Vec<u8>>, String> {
        if attachment.external_id.is_empty() {
            return Ok(None);
        }
        let data = self.get_json(auth, &format!("{}/users/me/messages/{}/attachments/{}", self.gmail, message_external_id, attachment.external_id)).await?;
        let encoded = data["data"].as_str().ok_or_else(|| format!("Gmail sent no data for {}", attachment.name))?;
        let padded = match encoded.len() % 4 {
            0 => encoded.to_string(),
            n => format!("{}{}", encoded, "=".repeat(4 - n)),
        };
        URL_SAFE.decode(padded.as_bytes()).map(Some).map_err(|e| format!("Gmail sent unreadable data for {}: {}", attachment.name, e))
    }

    async fn get_message_link(&self, auth: &ProviderAuth, conversation_external_id: &str, _message_external_id: &str) -> Result<Option<String>, String> {
        let Some(thread) = conversation_external_id.strip_prefix("gmail:") else { return Ok(None) };
        Ok(self.account_email(auth).await.map(|account| Self::thread_url(&account, thread)))
    }

    async fn get_search(&self, auth: &ProviderAuth, search: &RemoteSearch) -> Result<Vec<ResponseMessage>, String> {
        let mut terms = vec![search.text.trim().to_string()];
        if let Some(from) = &search.from {
            terms.push(format!("from:{}", from));
        }
        if let Some(after) = search.after {
            terms.push(format!("after:{}", after.timestamp()));
        }
        if let Some(before) = search.before {
            terms.push(format!("before:{}", before.timestamp()));
        }
        let query = terms.into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
        let data = self.get_json(auth, &format!("{}/users/me/messages?q={}&maxResults={}", self.gmail, urlencoding::encode(&query), search.limit.clamp(1, 100))).await?;
        let labels = self.label_names(auth).await;
        let account = self.account_email(auth).await;
        let mut out = Vec::new();
        for hit in data["messages"].as_array().cloned().unwrap_or_default() {
            let thread = format!("gmail:{}", hit["threadId"].as_str().unwrap_or(""));
            if search.conversation_external_id.as_deref().is_some_and(|c| c != thread) {
                continue;
            }
            let Some(id) = hit["id"].as_str() else { continue };
            let message = self.get_json(auth, &format!("{}/users/me/messages/{}?format=full", self.gmail, id)).await?;
            out.push(Self::gmail_message(&message, &thread, &labels, account.as_deref()));
        }
        Ok(out)
    }

    async fn put_remote_draft(&self, auth: &ProviderAuth, conversation_external_id: &str, draft: &OutgoingMessage, existing_external_id: Option<&str>) -> Result<Option<String>, String> {
        let body = draft.mail_text();
        let html = draft.mail_html().unwrap_or_else(|| crate::content::markdown::html(&body));
        let mut raw = Self::build_rfc2822_message(&draft.to.join(", "), draft.subject.as_deref().unwrap_or_default(), &crate::content::markdown::plain(&body), "markdown", Some(&html));
        if !draft.cc.is_empty() {
            raw = raw.replacen("\r\n\r\n", &format!("\r\nCc: {}\r\n\r\n", draft.cc.join(", ")), 1);
        }
        if let Some(from) = draft.from.as_ref() {
            raw = format!("From: {}\r\n{}", from, raw);
        }
        let mut message = serde_json::json!({ "raw": URL_SAFE_NO_PAD.encode(raw.as_bytes()) });
        if let Some(thread) = conversation_external_id.strip_prefix("gmail:") {
            message["threadId"] = Value::String(thread.to_string());
        }
        let saved = match existing_external_id {
            Some(id) => self.send_json(auth, reqwest::Method::PUT, &format!("{}/users/me/drafts/{}", self.gmail, id), &serde_json::json!({ "id": id, "message": message })).await?,
            None => self.post_json(auth, &format!("{}/users/me/drafts", self.gmail), &serde_json::json!({ "message": message })).await?,
        };
        Ok(saved["id"].as_str().map(String::from).or(existing_external_id.map(String::from)))
    }

    async fn delete_remote_draft(&self, auth: &ProviderAuth, draft_external_id: &str) -> Result<bool, String> {
        self.send_json(auth, reqwest::Method::DELETE, &format!("{}/users/me/drafts/{}", self.gmail, draft_external_id), &Value::Null).await?;
        Ok(true)
    }

    async fn put_flag(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str, flagged: bool) -> Result<bool, String> {
        let change = if flagged { serde_json::json!({ "addLabelIds": ["STARRED"] }) } else { serde_json::json!({ "removeLabelIds": ["STARRED"] }) };
        self.post_json(auth, &format!("{}/users/me/messages/{}/modify", self.gmail, message_external_id), &change).await?;
        Ok(true)
    }

    async fn put_archive(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str, archived: bool) -> Result<bool, String> {
        let change = if archived { serde_json::json!({ "removeLabelIds": ["INBOX"] }) } else { serde_json::json!({ "addLabelIds": ["INBOX"] }) };
        self.post_json(auth, &format!("{}/users/me/messages/{}/modify", self.gmail, message_external_id), &change).await?;
        Ok(true)
    }

    async fn get_aliases(&self, auth: &ProviderAuth) -> Result<Vec<RemoteAlias>, String> {
        let data = self.get_json(auth, &format!("{}/users/me/settings/sendAs", self.gmail)).await?;
        Ok(data["sendAs"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter(|a| a["verificationStatus"].as_str().is_none_or(|s| s == "accepted"))
            .map(|a| RemoteAlias {
                email: a["sendAsEmail"].as_str().unwrap_or("").to_lowercase(),
                name: a["displayName"].as_str().filter(|n| !n.is_empty()).map(String::from),
                is_default: a["isDefault"].as_bool().unwrap_or(false),
                signature: a["signature"].as_str().filter(|s| !s.trim().is_empty()).map(String::from),
            })
            .collect())
    }

    async fn put_signature(&self, auth: &ProviderAuth, alias: &str, html: &str) -> Result<bool, String> {
        self.send_json(auth, reqwest::Method::PATCH, &format!("{}/users/me/settings/sendAs/{}", self.gmail, urlencoding::encode(alias)), &serde_json::json!({ "signature": html })).await?;
        Ok(true)
    }

    async fn get_auto_reply(&self, auth: &ProviderAuth) -> Result<Option<RemoteAutoReply>, String> {
        let data = self.get_json(auth, &format!("{}/users/me/settings/vacation", self.gmail)).await?;
        let at = |key: &str| data[key].as_str().and_then(|ms| ms.parse::<i64>().ok()).or_else(|| data[key].as_i64()).and_then(DateTime::from_timestamp_millis);
        Ok(Some(RemoteAutoReply {
            enabled: data["enableAutoReply"].as_bool().unwrap_or(false),
            subject: data["responseSubject"].as_str().filter(|s| !s.is_empty()).map(String::from),
            body: data["responseBodyPlainText"].as_str().or_else(|| data["responseBodyHtml"].as_str()).unwrap_or("").to_string(),
            starts_at: at("startTime"),
            ends_at: at("endTime"),
            external_only: data["restrictToDomain"].as_bool().unwrap_or(false),
        }))
    }

    async fn put_auto_reply(&self, auth: &ProviderAuth, reply: &RemoteAutoReply) -> Result<bool, String> {
        let mut body = serde_json::json!({
            "enableAutoReply": reply.enabled,
            "responseSubject": reply.subject.clone().unwrap_or_default(),
            "responseBodyHtml": crate::content::markdown::html(&reply.body),
            "responseBodyPlainText": crate::content::markdown::plain(&reply.body),
            "restrictToDomain": reply.external_only,
        });
        if let Some(start) = reply.starts_at {
            body["startTime"] = Value::String(start.timestamp_millis().to_string());
        }
        if let Some(end) = reply.ends_at {
            body["endTime"] = Value::String(end.timestamp_millis().to_string());
        }
        self.send_json(auth, reqwest::Method::PUT, &format!("{}/users/me/settings/vacation", self.gmail), &body).await?;
        Ok(true)
    }
}

impl GoogleProvider {
    fn collect_attachments(payload: &Value) -> Vec<ResponseAttachment> {
        crate::content::mail::gmail_parts(payload)
            .into_iter()
            .map(|(id, part)| ResponseAttachment {
                external_id: id,
                name: part.name,
                content_type: part.mime,
                size: part.size,
                url: part.data,
                content_id: part.content_id,
            })
            .collect()
    }

    async fn token_call(&self, params: &[(&str, &str)], previous_refresh: Option<&str>) -> Result<ProviderAuth, String> {
        let response = crate::providers::http::send(self.client.post(&self.token).form(params)).await?;
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Token request failed: {} {}", status, error_text));
        }
        let token_data: Value = response.json().await.map_err(|e| format!("Failed to parse token response: {}", e))?;
        let access_token = token_data["access_token"].as_str().ok_or("No access token in response")?;
        let expires_in = token_data["expires_in"].as_u64().unwrap_or(3600);
        Ok(ProviderAuth {
            access_token: access_token.to_string(),
            refresh_token: token_data["refresh_token"].as_str().or(previous_refresh).map(String::from),
            expires_at: Utc::now() + Duration::seconds(expires_in as i64),
            token_type: "Bearer".to_string(),
            scope: token_data["scope"].as_str().map(String::from),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calendar_fixture(name: &str) -> Value {
        serde_json::from_str(&std::fs::read_to_string(format!("{}/tests/fixtures/calendar/{}", env!("CARGO_MANIFEST_DIR"), name)).unwrap()).unwrap()
    }

    #[test]
    fn google_calendars_map_access_and_names() {
        let list = calendar_fixture("google-calendar-list.json");
        let calendars: Vec<RemoteCalendar> = list["items"].as_array().unwrap().iter().map(GoogleProvider::google_calendar).collect();
        assert_eq!(calendars[0], RemoteCalendar { external_id: "milo@sundial.example".into(), name: "milo@sundial.example".into(), colour: Some("#0b8043".into()), is_primary: true, can_write: true });
        assert_eq!(calendars[1].name, "Sundial team");
        assert!(calendars[1].can_write);
        assert!(!calendars[2].can_write);
    }

    #[test]
    fn google_events_map_times_people_meet_links_and_cancellations() {
        let data = calendar_fixture("google-events.json");
        let items = data["items"].as_array().unwrap();
        let signing = GoogleProvider::google_event(&items[0]).unwrap();
        assert_eq!(signing.starts_at.to_rfc3339(), "2026-10-16T11:00:00+00:00");
        assert_eq!(signing.ical_uid.as_deref(), Some("040000008200E00074C5B7101A82E008000000001"));
        assert_eq!(signing.attendees, vec!["anna.berg@client.example", "milo@sundial.example"]);
        assert_eq!(signing.organizer.as_deref(), Some("anna.berg@client.example"));
        assert_eq!(signing.response_status.as_deref(), Some("tentative"));
        assert_eq!(signing.online_meeting_url.as_deref(), Some("https://meet.google.com/abc-defg-hij"));
        let cabin = GoogleProvider::google_event(&items[1]).unwrap();
        assert!(cabin.all_day);
        assert_eq!(cabin.ends_at.to_rfc3339(), "2026-10-25T00:00:00+00:00");
        assert!(GoogleProvider::google_event(&items[2]).unwrap().cancelled);
        assert_eq!(GoogleProvider::google_event(&items[3]).unwrap().online_meeting_url.as_deref(), Some("https://meet.google.com/xyz-board"));
    }

    #[test]
    fn google_create_body_asks_for_meet_and_uses_exclusive_all_day_ends() {
        let at = |s: &str| DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc);
        let timed = NewRemoteEvent { title: "Kickoff".into(), starts_at: at("2026-10-20T08:00:00Z"), ends_at: at("2026-10-20T09:00:00Z"), attendees: vec!["anna.berg@client.example".into()], online_meeting: true, ..Default::default() };
        let body = GoogleProvider::google_event_body(&timed, "req-1");
        assert_eq!(body["start"]["dateTime"], "2026-10-20T08:00:00+00:00");
        assert_eq!(body["attendees"][0]["email"], "anna.berg@client.example");
        assert_eq!(body["conferenceData"]["createRequest"]["requestId"], "req-1");
        assert_eq!(body["conferenceData"]["createRequest"]["conferenceSolutionKey"]["type"], "hangoutsMeet");
        let day = NewRemoteEvent { title: "Off".into(), starts_at: at("2026-10-23T00:00:00Z"), ends_at: at("2026-10-24T00:00:00Z"), all_day: true, ..Default::default() };
        let body = GoogleProvider::google_event_body(&day, "req-2");
        assert_eq!(body["start"]["date"], "2026-10-23");
        assert_eq!(body["end"]["date"], "2026-10-24");
        assert!(body.get("conferenceData").is_none());
    }

    #[tokio::test]
    async fn the_consent_url_asks_for_mail_changes_and_full_calendar() {
        let provider = GoogleProvider::new("g".into(), "s".into(), "http://localhost:48731/callback/google".into());
        let url = provider.get_auth_url("s1", Some("ch")).await.unwrap();
        assert!(url.contains("gmail.modify") && url.contains("auth%2Fcalendar%20") && !url.contains("calendar.readonly") && !url.contains("gmail.readonly"));
        assert!(url.contains("access_type=offline&prompt=consent") && url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn markdown_mail_is_multipart_alternative_and_survives_attachments() {
        let markdown = "**Hei** og [agenda](https://northwind.example/agenda)";
        let raw = GoogleProvider::build_rfc2822_message("anna@client.example", "Møte", &crate::content::markdown::plain(markdown), "markdown", Some(&crate::content::markdown::html(markdown)));
        let (headers, body) = raw.split_once("\r\n\r\n").unwrap();
        assert!(headers.contains("Content-Type: multipart/alternative; boundary="));
        assert!(body.contains("Content-Type: text/plain; charset=utf-8\r\n\r\nHei og agenda (https://northwind.example/agenda)\r\n"));
        assert!(body.contains("Content-Type: text/html; charset=utf-8\r\n\r\n<p><strong>Hei</strong> og <a href=\"https://northwind.example/agenda\">agenda</a></p>\r\n"));
        let mixed = GoogleProvider::with_attachments(&raw, &[OutgoingAttachment { name: "a.txt".to_string(), mime_type: "text/plain".to_string(), bytes: b"x".to_vec() }]);
        assert_eq!(mixed.matches("MIME-Version").count(), 1);
        assert!(mixed.contains("Content-Type: multipart/mixed"));
        assert!(mixed.contains("\r\nContent-Type: multipart/alternative; boundary="));
    }

    #[test]
    fn a_gmail_quoted_reply_carries_in_reply_to_and_the_reference_chain() {
        use crate::providers::traits::{prefixed_subject, OutgoingReference};
        let quote = OutgoingReference { internet_message_id: Some("<b@client.example>".into()), references: Some("<a@client.example>".into()), subject: Some("Tilbud".into()), ..Default::default() };
        let headers = quote.reply_headers();
        assert_eq!(headers, Some(("<b@client.example>".to_string(), "<a@client.example> <b@client.example>".to_string())));
        let raw = GoogleProvider::build_rfc2822_message("anna.berg@client.example", &prefixed_subject("Re", "Tilbud"), "Takk", "text", None);
        let replied = GoogleProvider::with_reply_headers(&raw, headers.as_ref());
        assert!(replied.contains("Subject: Re: Tilbud"));
        assert!(replied.contains("\r\nIn-Reply-To: <b@client.example>\r\nReferences: <a@client.example> <b@client.example>\r\n\r\n"));
        assert_eq!(GoogleProvider::with_reply_headers(&raw, None), raw);
        assert_eq!(prefixed_subject("Re", "RE: Tilbud"), "RE: Tilbud");
        assert_eq!(prefixed_subject("Fwd", "Tilbud"), "Fwd: Tilbud");
        let chained = OutgoingReference { internet_message_id: Some("<b@client.example>".into()), references: Some("<a@client.example> <b@client.example>".into()), ..Default::default() };
        assert_eq!(chained.reply_headers().unwrap().1, "<a@client.example> <b@client.example>");
        assert!(OutgoingReference::default().reply_headers().is_none());
    }

    #[test]
    fn gmail_replies_and_forwards_through_mail() {
        let caps = GoogleProvider::new(String::new(), String::new(), String::new()).get_provider().capabilities.clone();
        assert!(caps.supports_reply_quote && caps.supports_forward);
        assert_eq!((caps.reply_quote, caps.forward), (Delivery::Mail, Delivery::Mail));
    }

    mod served {
        use super::super::*;
        use serde_json::json;
        use wiremock::matchers::{method, path, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        fn fixture(name: &str) -> Value {
            serde_json::from_str(&std::fs::read_to_string(format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)).unwrap()).unwrap()
        }

        fn auth() -> ProviderAuth {
            ProviderAuth { access_token: "ya29.test".into(), refresh_token: None, expires_at: Utc::now() + Duration::hours(1), token_type: "Bearer".into(), scope: None }
        }

        fn google(server: &MockServer) -> GoogleProvider {
            GoogleProvider::new("g".into(), "s".into(), String::new()).with_api(&server.uri())
        }

        async fn bodies(server: &MockServer, verb: &str, at: &str) -> Vec<Value> {
            server.received_requests().await.unwrap().into_iter().filter(|r| r.method.as_str() == verb && r.url.path() == at).map(|r| serde_json::from_slice(&r.body).unwrap_or(Value::Null)).collect()
        }

        #[test]
        fn history_turns_label_moves_and_deletions_into_read_state_labels_and_removals() {
            let changes = GoogleProvider::history_changes(&fixture("google/history-page-1.json"));
            assert_eq!(changes[0], RemoteChange { conversation_external_id: Some("gmail:t-1".into()), message_external_id: "m-1".into(), is_read: Some(true), deleted: false, flagged: Some(false), labels: Some(vec!["INBOX".into(), "CATEGORY_UPDATES".into()]), ..Default::default() });
            assert_eq!((changes[1].is_read, changes[1].labels.as_ref().map(|l| l.len())), (None, Some(3)));
            let later = GoogleProvider::history_changes(&fixture("google/history-page-2.json"));
            assert!(later[0].deleted && later[0].message_external_id == "m-3");
            assert!(later[1].deleted && later[1].labels == Some(vec!["TRASH".to_string()]));
        }

        #[tokio::test]
        async fn the_history_id_starts_from_the_profile_then_pages_and_restarts_when_gmail_forgets_it() {
            let server = MockServer::start().await;
            Mock::given(method("GET")).and(path("/gmail/v1/users/me/profile")).respond_with(ResponseTemplate::new(200).set_body_json(json!({ "emailAddress": "milo@sundial.example", "historyId": "8000" }))).mount(&server).await;
            Mock::given(method("GET")).and(path("/gmail/v1/users/me/history")).and(query_param("startHistoryId", "1")).respond_with(ResponseTemplate::new(404).set_body_json(json!({ "error": { "message": "Requested entity was not found." } }))).with_priority(1).mount(&server).await;
            Mock::given(method("GET")).and(path("/gmail/v1/users/me/history")).and(query_param("pageToken", "history-2")).respond_with(ResponseTemplate::new(200).set_body_json(fixture("google/history-page-2.json"))).with_priority(2).mount(&server).await;
            Mock::given(method("GET")).and(path("/gmail/v1/users/me/history")).respond_with(ResponseTemplate::new(200).set_body_json(fixture("google/history-page-1.json"))).mount(&server).await;
            let provider = google(&server);
            let now = Utc::now();
            let first = provider.get_delta(&auth(), &HashMap::new(), now).await.unwrap();
            assert!(first.changes.is_empty());
            assert_eq!(first.cursors.get(HISTORY_CURSOR).map(String::as_str), Some("8000"));
            let second = provider.get_delta(&auth(), &first.cursors, now).await.unwrap();
            assert_eq!(second.changes.len(), 4);
            assert_eq!(second.cursors.get(HISTORY_CURSOR).map(String::as_str), Some("9004"));
            let asked = server.received_requests().await.unwrap().into_iter().find(|r| r.url.path() == "/gmail/v1/users/me/history").unwrap();
            let query: HashMap<String, String> = asked.url.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            assert_eq!(query.get("startHistoryId").map(String::as_str), Some("8000"));
            let gone = provider.get_delta(&auth(), &HashMap::from([(HISTORY_CURSOR.to_string(), "1".to_string())]), now).await.unwrap();
            assert!(!gone.cursors.contains_key(HISTORY_CURSOR));
        }

        #[tokio::test]
        async fn contacts_and_other_contacts_become_people_with_photos() {
            let server = MockServer::start().await;
            Mock::given(method("GET")).and(path("/people/v1/people/me/connections")).respond_with(ResponseTemplate::new(200).set_body_json(fixture("google/connections.json"))).mount(&server).await;
            Mock::given(method("GET")).and(path("/people/v1/otherContacts")).respond_with(ResponseTemplate::new(200).set_body_json(fixture("google/other-contacts.json"))).mount(&server).await;
            let people = google(&server).get_people(&auth()).await.unwrap();
            assert_eq!(people.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["anna.berg@client.example", "ola@mail.example"]);
            assert_eq!((people[0].avatar_url.as_deref(), people[0].company.as_deref(), people[0].phone.as_deref()), (Some("https://lh3.googleusercontent.com/anna"), Some("Kunde AS"), Some("+47 900 00 000")));
            assert_eq!(people[1].avatar_url, None);
            let asked = server.received_requests().await.unwrap();
            let connections: HashMap<String, String> = asked[0].url.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            assert_eq!(connections.get("personFields").map(String::as_str), Some("names,emailAddresses,photos,organizations,phoneNumbers"));
        }

        #[tokio::test]
        async fn answering_an_invitation_patches_only_my_attendee_status() {
            let server = MockServer::start().await;
            let at = "/calendar/v3/calendars/primary/events/ev-9";
            Mock::given(method("GET")).and(path(at)).respond_with(ResponseTemplate::new(200).set_body_json(fixture("google/event-invited.json"))).mount(&server).await;
            let mut answered = fixture("google/event-invited.json");
            answered["attendees"][1]["responseStatus"] = json!("tentative");
            Mock::given(method("PATCH")).and(path(at)).and(query_param("sendUpdates", "all")).respond_with(ResponseTemplate::new(200).set_body_json(answered)).mount(&server).await;
            let event = google(&server).put_calendar_response(&auth(), "primary", "ev-9", "tentativelyAccepted").await.unwrap();
            assert_eq!(event.response_status.as_deref(), Some("tentative"));
            let patched = &bodies(&server, "PATCH", at).await[0];
            assert_eq!(patched["attendees"][0]["responseStatus"], "accepted");
            assert_eq!(patched["attendees"][1], json!({ "email": "milo@sundial.example", "self": true, "responseStatus": "tentative" }));
            assert!(GoogleProvider::response_status("maybe").is_err());
            assert!(GoogleProvider::answered_attendees(&json!({ "attendees": [{ "email": "x" }] }), "accepted").is_none());
        }

        #[tokio::test]
        async fn attachments_read_state_and_threads_use_gmail_and_survive_a_429() {
            let server = MockServer::start().await;
            Mock::given(method("GET")).and(path("/gmail/v1/users/me/messages/m-1/attachments/att-1")).respond_with(ResponseTemplate::new(200).set_body_json(json!({ "size": 3, "data": "aGVp" }))).mount(&server).await;
            Mock::given(method("POST")).and(path("/gmail/v1/users/me/messages/m-1/modify")).respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "m-1" }))).mount(&server).await;
            Mock::given(method("GET")).and(path("/gmail/v1/users/me/profile")).respond_with(ResponseTemplate::new(200).set_body_json(json!({ "emailAddress": "milo@sundial.example", "historyId": "1" }))).mount(&server).await;
            Mock::given(method("GET")).and(path("/gmail/v1/users/me/threads")).respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0")).up_to_n_times(1).with_priority(1).mount(&server).await;
            Mock::given(method("GET")).and(path("/gmail/v1/users/me/threads")).respond_with(ResponseTemplate::new(200).set_body_json(json!({ "threads": [{ "id": "18f2a90000aa", "snippet": "Hei" }] }))).mount(&server).await;
            Mock::given(method("GET")).and(path("/gmail/v1/users/me/threads/18f2a90000aa")).respond_with(ResponseTemplate::new(200).set_body_json(json!({ "messages": [fixture("gmail-message.json")] }))).mount(&server).await;
            let provider = google(&server);
            let file = ResponseAttachment { external_id: "att-1".into(), name: "a.txt".into(), content_type: "text/plain".into(), size: 3, url: None, content_id: None };
            assert_eq!(provider.get_attachment(&auth(), "gmail:t-1", "m-1", &file).await.unwrap(), Some(b"hei".to_vec()));
            provider.mark_read(&auth(), "gmail:t-1", "m-1", false).await.unwrap();
            provider.mark_read(&auth(), "gmail:t-1", "m-1", true).await.unwrap();
            assert_eq!(bodies(&server, "POST", "/gmail/v1/users/me/messages/m-1/modify").await, vec![json!({ "addLabelIds": ["UNREAD"] }), json!({ "removeLabelIds": ["UNREAD"] })]);
            let conversations = provider.sync_conversations(&auth(), Some(Utc::now() - Duration::days(1))).await.unwrap();
            assert_eq!(conversations.len(), 1);
            assert_eq!(conversations[0].metadata["web_url"], "https://mail.google.com/mail/?authuser=milo%40sundial.example#all/18f2a90000aa");
            assert_eq!(server.received_requests().await.unwrap().iter().filter(|r| r.url.path() == "/gmail/v1/users/me/threads").count(), 2);
            assert_eq!(provider.get_message_link(&auth(), "gmail:18f2a90000aa", "x").await.unwrap().as_deref(), Some("https://mail.google.com/mail/?authuser=milo%40sundial.example#all/18f2a90000aa"));
        }

        #[test]
        fn gmail_messages_carry_label_names_read_state_and_the_thread_link() {
            let labels = HashMap::from([("INBOX".to_string(), "Inbox".to_string())]);
            let message = GoogleProvider::gmail_message(&fixture("gmail-message.json"), "gmail:18f2a90000aa", &labels, Some("milo@sundial.example"));
            assert_eq!(message.metadata["labels"], json!(["INBOX", "UNREAD"]));
            assert_eq!(message.metadata["label_names"], json!(["Inbox"]));
            assert!(!message.is_read);
            assert_eq!(message.metadata["web_url"], "https://mail.google.com/mail/?authuser=milo%40sundial.example#all/18f2a90000aa");
        }
    }
}
