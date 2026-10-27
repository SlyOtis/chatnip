use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::providers::models::{Provider, ProviderAuth};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResponseUser {
    pub id: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub display_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub avatar_url: Option<String>,
    pub job_title: Option<String>,
    pub department: Option<String>,
    pub company: Option<String>,
    pub office_location: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConversationType {
    DirectMessage,
    GroupMessage,
    Channel,
    Email,
    #[default]
    Unknown,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Chat,
    Email,
    System,
    Bot,
    #[default]
    Unknown,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResponseAttachment {
    pub external_id: String,
    pub name: String,
    pub content_type: String,
    pub size: u64,
    pub url: Option<String>,
    #[serde(default)]
    pub content_id: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutgoingAttachment {
    pub name: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OutgoingReference {
    pub external_id: String,
    pub conversation_external_id: Option<String>,
    pub conversation_name: Option<String>,
    pub sender_id: Option<String>,
    pub sender_name: String,
    pub sender_email: Option<String>,
    pub text: String,
    pub subject: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub internet_message_id: Option<String>,
    pub references: Option<String>,
    pub to: Vec<String>,
    pub attachments: Vec<String>,
    pub native: bool,
}
impl OutgoingReference {
    pub fn reply_headers(&self) -> Option<(String, String)> {
        let id = self.internet_message_id.clone().filter(|id| !id.trim().is_empty())?;
        let references = match self.references.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
            Some(chain) if chain.split_whitespace().any(|r| r == id) => chain.to_string(),
            Some(chain) => format!("{} {}", chain, id),
            None => id.clone(),
        };
        Some((id, references))
    }

    pub fn forward_block(&self) -> String {
        let from = match &self.sender_email {
            Some(email) if *email != self.sender_name => format!("{} <{}>", self.sender_name, email),
            _ => self.sender_name.clone(),
        };
        let mut out = format!("---------- Forwarded message ---------\nFrom: {}\nDate: {}\n", from, self.timestamp.to_rfc2822());
        if let Some(subject) = self.subject.as_deref().filter(|s| !s.trim().is_empty()) {
            out.push_str(&format!("Subject: {}\n", subject));
        }
        if !self.to.is_empty() {
            out.push_str(&format!("To: {}\n", self.to.join(", ")));
        }
        if !self.attachments.is_empty() {
            out.push_str(&format!("Attachments: {}\n", self.attachments.join(", ")));
        }
        out.push('\n');
        out.push_str(&self.text);
        out
    }
}
pub fn prefixed_subject(prefix: &str, subject: &str) -> String {
    let subject = subject.trim();
    if subject.to_lowercase().starts_with(&format!("{}:", prefix.to_lowercase())) {
        subject.to_string()
    } else {
        format!("{}: {}", prefix, subject)
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutgoingMessage {
    pub content: String,
    pub content_type: String,
    pub html: Option<String>,
    pub subject: Option<String>,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub in_reply_to: Option<String>,
    pub thread_external_id: Option<String>,
    pub attachments: Vec<OutgoingAttachment>,
    #[serde(default)]
    pub quote: Option<OutgoingReference>,
    #[serde(default)]
    pub forwarded: Vec<OutgoingReference>,
    #[serde(default)]
    pub broadcast: bool,
    #[serde(default)]
    pub from: Option<String>,
}
impl OutgoingMessage {
    pub fn mail_text(&self) -> String {
        if self.forwarded.is_empty() {
            return self.content.clone();
        }
        let blocks = self.forwarded.iter().map(|f| f.forward_block()).collect::<Vec<_>>().join("\n\n");
        match (self.content.trim().is_empty(), self.content_type.as_str()) {
            (true, "html") => crate::utils::text_extraction::html_escape(&blocks).replace('\n', "<br>"),
            (true, _) => blocks,
            (false, "html") => format!("{}<br><br>{}", self.content, crate::utils::text_extraction::html_escape(&blocks).replace('\n', "<br>")),
            (false, _) => format!("{}\n\n{}", self.content.trim_end(), blocks),
        }
    }
    pub fn mail_html(&self) -> Option<String> {
        self.html.clone().filter(|_| self.forwarded.is_empty())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseConversation {
    pub external_id: String,
    pub name: String,
    pub conversation_type: ConversationType,
    pub members: Vec<ResponseUser>,
    pub last_activity: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageOrigin {
    pub ip: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub city: Option<String>,
    pub country: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    pub external_id: String,
    pub conversation_external_id: String,
    pub sender: ResponseUser,
    pub content: String,
    pub content_type: String,
    pub timestamp: DateTime<Utc>,
    pub r#type: MessageType,
    pub metadata: HashMap<String, serde_json::Value>,
    pub subject: Option<String>,
    pub recipients: Vec<ResponseUser>,
    pub cc: Vec<ResponseUser>,
    pub is_read: bool,
    pub thread_id: Option<String>,
    pub reply_to_id: Option<String>,
    pub attachments: Vec<ResponseAttachment>,
    pub headers: Option<String>,
    pub origin: Option<MessageOrigin>,
}
impl Default for ResponseMessage {
    fn default() -> Self {
        Self {
            external_id: String::new(),
            conversation_external_id: String::new(),
            sender: ResponseUser::default(),
            content: String::new(),
            content_type: "text".to_string(),
            timestamp: Utc::now(),
            r#type: MessageType::Unknown,
            metadata: HashMap::new(),
            subject: None,
            recipients: Vec::new(),
            cc: Vec::new(),
            is_read: true,
            thread_id: None,
            reply_to_id: None,
            attachments: Vec::new(),
            headers: None,
            origin: None,
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SystemPerson {
    pub user_id: Option<String>,
    pub external_id: String,
    pub name: String,
}
impl SystemPerson {
    pub fn new(external_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self { user_id: None, external_id: external_id.into(), name: name.into() }
    }
}
impl From<&ResponseUser> for SystemPerson {
    fn from(user: &ResponseUser) -> Self {
        Self::new(user.id.clone(), user.display_name.clone().or_else(|| user.email.clone()).unwrap_or_default())
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SystemEventKind {
    MemberJoined,
    MemberLeft,
    MemberAdded,
    MemberRemoved,
    Renamed,
    TopicChanged,
    CallStarted,
    CallEnded,
    Pinned,
    Unpinned,
    ChatCreated,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemEvent {
    pub event: SystemEventKind,
    pub actors: Vec<SystemPerson>,
    pub targets: Vec<SystemPerson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}
impl SystemEvent {
    pub fn new(event: SystemEventKind, actors: Vec<SystemPerson>, targets: Vec<SystemPerson>, detail: Option<String>) -> Self {
        Self { event, actors, targets, detail: detail.map(|d| d.trim().to_string()).filter(|d| !d.is_empty()) }
    }

    pub fn people_mut(&mut self) -> impl Iterator<Item = &mut SystemPerson> {
        self.actors.iter_mut().chain(self.targets.iter_mut())
    }

    pub fn names(people: &[SystemPerson]) -> String {
        let names: Vec<&str> = people.iter().map(|p| if p.name.trim().is_empty() { "Someone" } else { p.name.trim() }).collect();
        match names.as_slice() {
            [] => "Someone".to_string(),
            [one] => one.to_string(),
            [rest @ .., last] => format!("{} and {}", rest.join(", "), last),
        }
    }

    pub fn sentence(&self) -> String {
        let actor = Self::names(&self.actors);
        let targets = Self::names(&self.targets);
        let alone = self.actors.is_empty();
        match (self.event, self.detail.as_deref()) {
            (SystemEventKind::MemberJoined, _) => format!("{} joined the conversation", actor),
            (SystemEventKind::MemberLeft, _) => format!("{} left the conversation", actor),
            (SystemEventKind::MemberAdded, _) if alone => format!("{} joined the conversation", targets),
            (SystemEventKind::MemberAdded, _) => format!("{} added {}", actor, targets),
            (SystemEventKind::MemberRemoved, _) if alone => format!("{} left the conversation", targets),
            (SystemEventKind::MemberRemoved, _) => format!("{} removed {}", actor, targets),
            (SystemEventKind::Renamed, Some(name)) => format!("{} renamed the conversation to “{}”", actor, name),
            (SystemEventKind::Renamed, None) => format!("{} renamed the conversation", actor),
            (SystemEventKind::TopicChanged, Some(topic)) => format!("{} changed the topic to “{}”", actor, topic),
            (SystemEventKind::TopicChanged, None) => format!("{} cleared the topic", actor),
            (SystemEventKind::CallStarted, _) => format!("{} started a call", actor),
            (SystemEventKind::CallEnded, _) if alone => "The call ended".to_string(),
            (SystemEventKind::CallEnded, _) => format!("{} ended the call", actor),
            (SystemEventKind::Pinned, _) => format!("{} pinned a message", actor),
            (SystemEventKind::Unpinned, _) => format!("{} unpinned a message", actor),
            (SystemEventKind::ChatCreated, _) => format!("{} created the conversation", actor),
        }
    }
}
impl ResponseMessage {
    pub fn system(external_id: impl Into<String>, conversation_external_id: impl Into<String>, timestamp: DateTime<Utc>, event: SystemEvent) -> Self {
        let sender = event.actors.first().or(event.targets.first()).map(|p| ResponseUser { id: p.external_id.clone(), display_name: (!p.name.is_empty()).then(|| p.name.clone()), ..Default::default() }).unwrap_or_default();
        let mut metadata = HashMap::new();
        metadata.insert("system".to_string(), serde_json::to_value(&event).unwrap_or(serde_json::Value::Null));
        Self {
            external_id: external_id.into(),
            conversation_external_id: conversation_external_id.into(),
            sender,
            content: event.sentence(),
            content_type: "text".to_string(),
            timestamp,
            r#type: MessageType::System,
            metadata,
            ..Default::default()
        }
    }

    pub fn system_event(&self) -> Option<SystemEvent> {
        self.metadata.get("system").and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncResult {
    pub conversations_synced: u32,
    pub messages_synced: u32,
    pub errors: Vec<SyncError>,
    pub last_sync: DateTime<Utc>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncError {
    pub error_type: String,
    pub message: String,
    pub conversation_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemoteCalendar {
    pub external_id: String,
    pub name: String,
    pub colour: Option<String>,
    pub is_primary: bool,
    pub can_write: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteEvent {
    pub external_id: String,
    pub ical_uid: Option<String>,
    pub title: String,
    pub notes: String,
    pub location: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub all_day: bool,
    pub attendees: Vec<String>,
    pub organizer: Option<String>,
    pub response_status: Option<String>,
    pub online_meeting_url: Option<String>,
    pub cancelled: bool,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NewRemoteEvent {
    pub title: String,
    pub notes: String,
    pub location: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub all_day: bool,
    pub attendees: Vec<String>,
    pub online_meeting: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemoteEmoji {
    pub name: String,
    pub url: Option<String>,
    pub alias_for: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemoteChange {
    pub conversation_external_id: Option<String>,
    pub message_external_id: String,
    pub is_read: Option<bool>,
    pub deleted: bool,
    pub labels: Option<Vec<String>>,
    #[serde(default)]
    pub flagged: Option<bool>,
    #[serde(default)]
    pub reactions: Option<Vec<(String, String)>>,
    #[serde(default)]
    pub edited_at: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Delta {
    pub cursors: HashMap<String, String>,
    pub changes: Vec<RemoteChange>,
    #[serde(default)]
    pub conversations: Vec<String>,
    #[serde(default)]
    pub complete: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LiveEvent {
    Message { message: ResponseMessage },
    Change { change: RemoteChange },
    Touched { conversation_external_id: String },
    Conversation { conversation: ResponseConversation },
    Typing { conversation_external_id: String, user: ResponseUser },
    Presence { user_external_id: String, status: String, text: Option<String>, emoji: Option<String> },
}
pub type LiveSink = tokio::sync::mpsc::Sender<LiveEvent>;
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NewRemoteConversation {
    pub conversation_type: ConversationType,
    pub name: String,
    pub members: Vec<String>,
    pub private: bool,
    pub parent_external_id: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemoteAlias {
    pub email: String,
    pub name: Option<String>,
    pub is_default: bool,
    pub signature: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemoteAutoReply {
    pub enabled: bool,
    pub subject: Option<String>,
    pub body: String,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub external_only: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemoteUserGroup {
    pub external_id: String,
    pub handle: String,
    pub name: String,
    pub members: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemoteSearch {
    pub text: String,
    pub from: Option<String>,
    pub conversation_external_id: Option<String>,
    pub before: Option<DateTime<Utc>>,
    pub after: Option<DateTime<Utc>>,
    pub limit: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionIdentity {
    pub namespace: String,
    pub name: String,
    pub external_id: Option<String>,
    pub user: Option<ResponseUser>,
    pub logo_url: Option<String>,
}

impl ConnectionIdentity {
    pub fn display_name(&self) -> String {
        let user = self.user.as_ref();
        [user.and_then(|u| u.display_name.as_deref()), user.and_then(|u| u.email.as_deref()), Some(self.name.as_str())]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|v| !v.is_empty())
            .unwrap_or_default()
            .to_string()
    }
}

#[async_trait]
pub trait ProviderTrait: Send + Sync {
    fn get_provider(&self) -> &Provider;
    fn connects_with_token(&self) -> bool {
        false
    }
    async fn get_auth_url(&self, state: &str, code_challenge: Option<&str>) -> Result<String, String>;
    async fn exchange_code(&self, code: &str, code_verifier: Option<&str>) -> Result<ProviderAuth, String>;
    async fn identity(&self, auth: &ProviderAuth) -> Result<ConnectionIdentity, String>;
    async fn fetch_logo(&self, _auth: &ProviderAuth, _identity: &ConnectionIdentity) -> Result<Option<(Vec<u8>, String)>, String> {
        Ok(None)
    }
    async fn refresh_token(&self, refresh_token: &str) -> Result<ProviderAuth, String>;
    async fn validate_tokens(&self, auth: &ProviderAuth) -> Result<bool, String>;
    async fn sync_conversations(
        &self,
        auth: &ProviderAuth,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<ResponseConversation>, String>;
    async fn sync_messages(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        since: Option<DateTime<Utc>>,
        limit: Option<u32>,
    ) -> Result<Vec<ResponseMessage>, String>;
    async fn send_message(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        draft: &OutgoingMessage,
    ) -> Result<ResponseMessage, String>;
    async fn mark_read(
        &self,
        _auth: &ProviderAuth,
        _conversation_external_id: &str,
        _message_external_id: &str,
        _read: bool,
    ) -> Result<(), String> {
        Ok(())
    }
    async fn edit_message(
        &self,
        _auth: &ProviderAuth,
        _conversation_external_id: &str,
        _message_external_id: &str,
        _content: &str,
    ) -> Result<(), String> {
        Err("Editing is not supported by this provider".to_string())
    }
    async fn send_reaction(
        &self,
        _auth: &ProviderAuth,
        _conversation_external_id: &str,
        _message_external_id: &str,
        _reaction: &str,
        _active: bool,
    ) -> Result<(), String> {
        Err("Reactions not supported by this provider".to_string())
    }
    async fn delete_message(
        &self,
        _auth: &ProviderAuth,
        _conversation_external_id: &str,
        _message_external_id: &str,
    ) -> Result<(), String> {
        Err("Message deletion not supported by this provider".to_string())
    }
    async fn get_presence(
        &self,
        _auth: &ProviderAuth,
        _user_id: &str,
    ) -> Result<String, String> {
        Err("Presence not supported by this provider".to_string())
    }
    async fn get_calendars(&self, _auth: &ProviderAuth) -> Result<Vec<RemoteCalendar>, String> {
        Ok(Vec::new())
    }
    async fn get_calendar_events(
        &self,
        _auth: &ProviderAuth,
        _calendar_external_id: &str,
        _from: DateTime<Utc>,
        _to: DateTime<Utc>,
    ) -> Result<Vec<RemoteEvent>, String> {
        Ok(Vec::new())
    }
    async fn post_calendar_event(
        &self,
        _auth: &ProviderAuth,
        _calendar_external_id: &str,
        _event: &NewRemoteEvent,
    ) -> Result<RemoteEvent, String> {
        Err("This connection cannot create calendar events".to_string())
    }
    async fn put_calendar_response(
        &self,
        _auth: &ProviderAuth,
        _calendar_external_id: &str,
        _event_external_id: &str,
        _response: &str,
    ) -> Result<RemoteEvent, String> {
        Err("This connection cannot answer invitations".to_string())
    }
    async fn put_member(
        &self,
        _auth: &ProviderAuth,
        _conversation_external_id: &str,
        _user_external_id: &str,
        _member: bool,
    ) -> Result<bool, String> {
        Ok(false)
    }
    async fn put_conversation_name(
        &self,
        _auth: &ProviderAuth,
        _conversation_external_id: &str,
        _name: &str,
    ) -> Result<bool, String> {
        Ok(false)
    }
    async fn put_pin(
        &self,
        _auth: &ProviderAuth,
        _conversation_external_id: &str,
        _message_external_id: &str,
        _pinned: bool,
    ) -> Result<bool, String> {
        Ok(false)
    }
    async fn get_pins(&self, _auth: &ProviderAuth, _conversation_external_id: &str) -> Result<Option<Vec<String>>, String> {
        Ok(None)
    }
    async fn get_custom_emoji(&self, _auth: &ProviderAuth) -> Result<Vec<RemoteEmoji>, String> {
        Ok(Vec::new())
    }
    async fn get_people(&self, _auth: &ProviderAuth) -> Result<Vec<ResponseUser>, String> {
        Ok(Vec::new())
    }
    async fn get_delta(&self, _auth: &ProviderAuth, cursors: &HashMap<String, String>, _since: DateTime<Utc>) -> Result<Delta, String> {
        Ok(Delta { cursors: cursors.clone(), ..Default::default() })
    }
    async fn get_attachment(&self, _auth: &ProviderAuth, _conversation_external_id: &str, _message_external_id: &str, _attachment: &ResponseAttachment) -> Result<Option<Vec<u8>>, String> {
        Ok(None)
    }
    async fn get_message_link(&self, _auth: &ProviderAuth, _conversation_external_id: &str, _message_external_id: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
    async fn put_status(&self, _auth: &ProviderAuth, _text: &str, _emoji: Option<&str>, _until: Option<DateTime<Utc>>) -> Result<bool, String> {
        Ok(false)
    }
    fn is_live(&self) -> bool {
        self.get_provider().capabilities.live
    }
    async fn get_live(&self, _auth: &ProviderAuth, _sink: LiveSink) -> Result<(), String> {
        Err("This connection does not push changes".to_string())
    }
    async fn get_presences(&self, auth: &ProviderAuth, user_ids: &[String]) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        for id in user_ids {
            out.push((id.clone(), self.get_presence(auth, id).await?));
        }
        Ok(out)
    }
    async fn get_user_groups(&self, _auth: &ProviderAuth) -> Result<Vec<RemoteUserGroup>, String> {
        Ok(Vec::new())
    }
    async fn get_search(&self, _auth: &ProviderAuth, _search: &RemoteSearch) -> Result<Vec<ResponseMessage>, String> {
        Ok(Vec::new())
    }
    async fn put_remote_draft(&self, _auth: &ProviderAuth, _conversation_external_id: &str, _draft: &OutgoingMessage, _existing_external_id: Option<&str>) -> Result<Option<String>, String> {
        Ok(None)
    }
    async fn delete_remote_draft(&self, _auth: &ProviderAuth, _draft_external_id: &str) -> Result<bool, String> {
        Ok(false)
    }
    async fn put_flag(&self, _auth: &ProviderAuth, _conversation_external_id: &str, _message_external_id: &str, _flagged: bool) -> Result<bool, String> {
        Ok(false)
    }
    async fn put_archive(&self, _auth: &ProviderAuth, _conversation_external_id: &str, _message_external_id: &str, _archived: bool) -> Result<bool, String> {
        Ok(false)
    }
    async fn post_conversation(&self, _auth: &ProviderAuth, _conversation: &NewRemoteConversation) -> Result<Option<ResponseConversation>, String> {
        Ok(None)
    }
    async fn put_membership(&self, _auth: &ProviderAuth, _conversation_external_id: &str, _joined: bool) -> Result<bool, String> {
        Ok(false)
    }
    async fn put_conversation_state(&self, _auth: &ProviderAuth, _conversation_external_id: &str, _archived: bool) -> Result<bool, String> {
        Ok(false)
    }
    async fn put_conversation_topic(&self, _auth: &ProviderAuth, _conversation_external_id: &str, _topic: Option<&str>, _purpose: Option<&str>) -> Result<bool, String> {
        Ok(false)
    }
    async fn get_joinable_conversations(&self, _auth: &ProviderAuth) -> Result<Vec<ResponseConversation>, String> {
        Ok(Vec::new())
    }
    async fn get_aliases(&self, _auth: &ProviderAuth) -> Result<Vec<RemoteAlias>, String> {
        Ok(Vec::new())
    }
    async fn put_signature(&self, _auth: &ProviderAuth, _alias: &str, _html: &str) -> Result<bool, String> {
        Ok(false)
    }
    async fn get_auto_reply(&self, _auth: &ProviderAuth) -> Result<Option<RemoteAutoReply>, String> {
        Ok(None)
    }
    async fn put_auto_reply(&self, _auth: &ProviderAuth, _reply: &RemoteAutoReply) -> Result<bool, String> {
        Ok(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseProviderUser {
    pub external_id: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub display_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub job_title: Option<String>,
    pub department: Option<String>,
    pub company: Option<String>,
    pub office_location: Option<String>,
    pub theme_color: Option<String>,
    pub details: Option<serde_json::Value>,
}

