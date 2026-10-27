use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use futures::TryStreamExt;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use mail_parser::{Address, MessageParser};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;
use crate::providers::models::{Delivery, Provider, ProviderAuth, ProviderCapabilities};
use crate::providers::traits::{
    ConnectionIdentity, ConversationType, Delta, LiveEvent, LiveSink, MessageType, OutgoingMessage, ProviderTrait, RemoteAlias, RemoteChange, RemoteSearch,
    ResponseAttachment, ResponseConversation, ResponseMessage, ResponseUser,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImapConfig {
    pub email: String,
    pub display_name: String,
    pub username: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    #[serde(default)]
    pub allow_invalid_certs: bool,
    #[serde(default)]
    pub aliases: Vec<String>,
}

type Session = async_imap::Session<TlsStream<TcpStream>>;

pub struct ImapProvider {
    provider: Provider,
    config: ImapConfig,
    cache: Mutex<HashMap<String, Vec<ResponseMessage>>>,
}

impl ImapProvider {
    pub fn new(config: ImapConfig) -> Self {
        let provider = Provider {
            id: "imap".to_string(),
            profile_id: String::new(),
            provider_type: "imap".to_string(),
            name: config.email.clone(),
            external_id: None,
            logo_id: None,
            logo: None,
            capabilities: ProviderCapabilities {
                can_send_messages: true,
                supports_attachments: true,
                supports_reply_quote: true,
                supports_forward: true,
                reply_quote: Delivery::Mail,
                forward: Delivery::Mail,
                supports_calendar_responses: false,
                live: true,
                supports_unread_sync: true,
                drafts: Delivery::Native,
                archive: Delivery::Native,
                supports_flags: true,
                supports_search: true,
                supports_aliases: true,
                ..Default::default()
            },
            is_enabled: true,
            is_connected: false,
            auth: None,
            sync_interval: 300,
            namespace: Some(config.email.clone()),
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
        Self { provider, config, cache: Mutex::new(HashMap::new()) }
    }

    fn addressing(draft: &OutgoingMessage, last: Option<&ResponseMessage>) -> (Vec<String>, String, Option<(String, String)>) {
        let mut to = draft.to.clone();
        let mut subject = draft.subject.clone().unwrap_or_default();
        if !draft.forwarded.is_empty() {
            return (to, subject, None);
        }
        let mut reference = draft.quote.as_ref().and_then(|q| q.reply_headers());
        if let Some(quote) = &draft.quote {
            if to.is_empty() {
                to.extend(quote.sender_email.clone());
            }
            if subject.is_empty() {
                subject = crate::providers::traits::prefixed_subject("Re", quote.subject.as_deref().unwrap_or_default());
            }
        }
        if let Some(last) = last {
            if to.is_empty() {
                to.extend(last.sender.email.clone());
            }
            if subject.is_empty() {
                subject = crate::providers::traits::prefixed_subject("Re", last.subject.as_deref().unwrap_or_default());
            }
            if reference.is_none() {
                reference = last.metadata.get("internet_message_id").and_then(|v| v.as_str()).map(|id| (id.to_string(), id.to_string()));
            }
        }
        (to, subject, reference)
    }

    pub fn config_from_json(raw: &str) -> Result<ImapConfig, String> {
        serde_json::from_str(raw).map_err(|e| format!("The mail connection settings are unreadable: {}", e))
    }

    async fn session(&self, password: &str) -> Result<Session, String> {
        static CRYPTO: std::sync::Once = std::sync::Once::new();
        CRYPTO.call_once(|| {
            let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
        });
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let mut config = ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
        if self.config.allow_invalid_certs {
            config.dangerous().set_certificate_verifier(Arc::new(AcceptAnyCertificate));
        }
        let connector = TlsConnector::from(Arc::new(config));
        let tcp = TcpStream::connect((self.config.imap_host.as_str(), self.config.imap_port))
            .await
            .map_err(|e| format!("Could not reach {}:{}: {}", self.config.imap_host, self.config.imap_port, e))?;
        let server = ServerName::try_from(self.config.imap_host.clone()).map_err(|_| "The IMAP host name is not valid".to_string())?;
        let tls = connector.connect(server, tcp).await.map_err(|e| format!("TLS to {} failed: {}", self.config.imap_host, e))?;
        let client = async_imap::Client::new(tls);
        let session = client
            .login(&self.config.username, password)
            .await
            .map_err(|(e, _)| format!("{} refused the login: {}", self.config.imap_host, e))?;
        Ok(session)
    }

    async fn special_folder(session: &mut Session, kind: &str) -> Option<String> {
        let mut names = session.list(None, Some("*")).await.ok()?;
        let mut found = None;
        while let Ok(Some(name)) = names.try_next().await {
            let lower = name.name().to_lowercase();
            let marked = name.attributes().iter().any(|a| format!("{:?}", a).to_lowercase().contains(kind));
            if marked || lower.contains(kind) {
                found = Some(name.name().to_string());
                if marked {
                    break;
                }
            }
        }
        found
    }

    fn address_users(address: Option<&Address<'_>>) -> Vec<ResponseUser> {
        address
            .map(|a| {
                a.iter()
                    .filter_map(|addr| {
                        let email = addr.address.as_ref().map(|e| e.to_lowercase())?;
                        Some(ResponseUser {
                            id: email.clone(),
                            email: Some(email),
                            display_name: addr.name.as_ref().map(|n| n.to_string()),
                            ..Default::default()
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn thread_key(parsed: &mail_parser::Message<'_>) -> String {
        if let Some(list) = parsed.references().as_text_list() {
            if let Some(first) = list.first() {
                return first.to_string();
            }
        }
        if let Some(reply) = parsed.in_reply_to().as_text() {
            return reply.to_string();
        }
        if let Some(id) = parsed.message_id() {
            return id.to_string();
        }
        let subject = parsed.subject().unwrap_or("").to_lowercase();
        format!("subject:{}", Self::strip_prefixes(&subject))
    }

    fn strip_prefixes(subject: &str) -> String {
        let mut s = subject.trim();
        loop {
            let lower = s.to_lowercase();
            let stripped = ["re:", "fwd:", "fw:", "sv:", "vs:", "aw:"].iter().find_map(|p| lower.strip_prefix(p).map(|_| s[p.len()..].trim()));
            match stripped {
                Some(rest) if rest != s => s = rest,
                _ => break,
            }
        }
        s.to_string()
    }

    fn parse(folder: &str, uid: u32, seen: bool, raw: &[u8]) -> Option<(String, ResponseMessage)> {
        let parsed = MessageParser::default().parse(raw)?;
        let mail = crate::content::mail::parse(raw)?;
        let key = Self::thread_key(&parsed);
        let sender = Self::address_users(parsed.from()).into_iter().next().unwrap_or_default();
        let timestamp = mail.date.unwrap_or_else(Utc::now);
        let (content, content_type) = match (&mail.html, &mail.plain) {
            (Some(html), _) => (html.clone(), "html"),
            (None, plain) => (plain.clone().unwrap_or_default(), "text"),
        };
        let attachments: Vec<ResponseAttachment> = mail
            .parts
            .iter()
            .map(|part| ResponseAttachment {
                external_id: format!("{}/{}", folder, uid),
                name: part.name.clone(),
                content_type: part.mime.clone(),
                size: part.size,
                url: part.data.clone(),
                content_id: part.content_id.clone(),
            })
            .collect();
        let mut metadata = HashMap::new();
        if let Some(id) = &mail.message_id {
            metadata.insert("internet_message_id".to_string(), Value::String(id.clone()));
        }
        metadata.insert("folder".to_string(), Value::String(folder.to_string()));
        let origin = mail.origin_ip.clone().map(|ip| crate::providers::traits::MessageOrigin { ip: Some(ip), ..Default::default() });
        let mut message = ResponseMessage {
            external_id: format!("{}/{}", folder, uid),
            conversation_external_id: String::new(),
            sender,
            content,
            content_type: content_type.to_string(),
            timestamp,
            r#type: MessageType::Email,
            metadata,
            subject: mail.subject.clone(),
            recipients: Self::address_users(parsed.to()),
            cc: Self::address_users(parsed.cc()),
            is_read: seen,
            reply_to_id: mail.in_reply_to.clone(),
            attachments,
            headers: Some(mail.headers.clone()),
            origin,
            ..Default::default()
        };
        mail.structure.apply(&mut message);
        Some((key, message))
    }

    async fn fetch_folder(session: &mut Session, folder: &str, since: DateTime<Utc>) -> Result<Vec<(String, ResponseMessage)>, String> {
        session.select(folder).await.map_err(|e| format!("Could not open {}: {}", folder, e))?;
        let query = format!("SINCE {}", since.format("%d-%b-%Y"));
        let uids = session.uid_search(query).await.map_err(|e| format!("Search in {} failed: {}", folder, e))?;
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        let mut sorted: Vec<u32> = uids.into_iter().collect();
        sorted.sort_unstable();
        let set = sorted.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
        let mut out = Vec::new();
        let mut fetches = session.uid_fetch(set, "(UID FLAGS BODY.PEEK[])").await.map_err(|e| format!("Fetch in {} failed: {}", folder, e))?;
        while let Some(fetch) = fetches.try_next().await.map_err(|e| format!("Fetch in {} failed: {}", folder, e))? {
            let Some(uid) = fetch.uid else { continue };
            let seen = fetch.flags().any(|f| matches!(f, async_imap::types::Flag::Seen));
            if let Some(body) = fetch.body() {
                if let Some(parsed) = Self::parse(folder, uid, seen, body) {
                    out.push(parsed);
                }
            }
        }
        drop(fetches);
        Ok(out)
    }

    fn modseq_cursor(uid_validity: Option<u32>, modseq: Option<u64>) -> Option<String> {
        Some(format!("{}:{}", uid_validity?, modseq?))
    }

    fn changed_since(cursor: &str, uid_validity: Option<u32>) -> Option<u64> {
        let (validity, modseq) = cursor.split_once(':')?;
        (validity.parse::<u32>().ok()? == uid_validity?).then(|| modseq.parse::<u64>().ok()).flatten()
    }

    fn split_external(id: &str) -> Result<(String, u32), String> {
        let (folder, uid) = id.rsplit_once('/').ok_or_else(|| "The message id does not name a folder".to_string())?;
        Ok((folder.to_string(), uid.parse::<u32>().map_err(|_| "The message id is not an IMAP uid".to_string())?))
    }

    async fn store_flag(&self, auth: &ProviderAuth, message_external_id: &str, flag: &str, add: bool) -> Result<(), String> {
        let (folder, uid) = Self::split_external(message_external_id)?;
        let mut session = self.session(&auth.access_token).await?;
        session.select(&folder).await.map_err(|e| format!("Could not open {}: {}", folder, e))?;
        let mut updates = session.uid_store(uid.to_string(), format!("{}FLAGS ({})", if add { "+" } else { "-" }, flag)).await.map_err(|e| format!("Flag update failed: {}", e))?;
        while let Ok(Some(_)) = updates.try_next().await {}
        drop(updates);
        if flag == "\\Deleted" {
            let expunged = session.expunge().await.map_err(|e| format!("Expunge failed: {}", e))?;
            let mut expunged = std::pin::pin!(expunged);
            while let Ok(Some(_)) = expunged.try_next().await {}
            drop(expunged);
        }
        let _ = session.logout().await;
        Ok(())
    }
}

#[async_trait]
impl ProviderTrait for ImapProvider {
    fn get_provider(&self) -> &Provider {
        &self.provider
    }

    fn connects_with_token(&self) -> bool {
        true
    }

    async fn get_auth_url(&self, _state: &str, _code_challenge: Option<&str>) -> Result<String, String> {
        Err("Mail accounts connect with a password in settings".to_string())
    }

    async fn exchange_code(&self, _code: &str, _code_verifier: Option<&str>) -> Result<ProviderAuth, String> {
        Err("Mail accounts connect with a password in settings".to_string())
    }

    async fn refresh_token(&self, _refresh_token: &str) -> Result<ProviderAuth, String> {
        Err("Mail passwords do not refresh".to_string())
    }

    async fn validate_tokens(&self, auth: &ProviderAuth) -> Result<bool, String> {
        let mut session = self.session(&auth.access_token).await?;
        let _ = session.logout().await;
        Ok(true)
    }

    async fn identity(&self, auth: &ProviderAuth) -> Result<ConnectionIdentity, String> {
        let mut session = self.session(&auth.access_token).await?;
        session.select("INBOX").await.map_err(|e| format!("Could not open INBOX: {}", e))?;
        let _ = session.logout().await;
        let display = if self.config.display_name.trim().is_empty() { self.config.email.clone() } else { self.config.display_name.clone() };
        Ok(ConnectionIdentity {
            namespace: self.config.email.to_lowercase(),
            name: self.config.email.clone(),
            external_id: Some(self.config.email.to_lowercase()),
            user: Some(ResponseUser {
                id: self.config.email.to_lowercase(),
                email: Some(self.config.email.to_lowercase()),
                display_name: Some(display),
                ..Default::default()
            }),
            logo_url: None,
        })
    }

    async fn sync_conversations(
        &self,
        auth: &ProviderAuth,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<ResponseConversation>, String> {
        let floor = since.unwrap_or_else(|| Utc::now() - Duration::days(30));
        let mut session = self.session(&auth.access_token).await?;
        let mut all = Self::fetch_folder(&mut session, "INBOX", floor).await?;
        if let Some(sent) = Self::special_folder(&mut session, "sent").await {
            if let Ok(more) = Self::fetch_folder(&mut session, &sent, floor).await {
                all.extend(more);
            }
        }
        let _ = session.logout().await;

        let mut threads: HashMap<String, ResponseConversation> = HashMap::new();
        let mut cache = self.cache.lock().await;
        cache.clear();
        for (key, mut message) in all {
            let external_id = format!("thread:{}", key);
            message.conversation_external_id = external_id.clone();
            let subject = message.subject.clone().map(|s| Self::strip_prefixes(&s)).filter(|s| !s.is_empty()).unwrap_or_else(|| "(no subject)".to_string());
            let entry = threads.entry(key.clone()).or_insert_with(|| ResponseConversation {
                external_id: external_id.clone(),
                name: subject.clone(),
                conversation_type: ConversationType::Email,
                members: Vec::new(),
                last_activity: message.timestamp,
                metadata: HashMap::new(),
            });
            if message.timestamp > entry.last_activity {
                entry.last_activity = message.timestamp;
            }
            for person in std::iter::once(message.sender.clone()).chain(message.recipients.iter().cloned()).chain(message.cc.iter().cloned()) {
                if person.email.is_some() && !entry.members.iter().any(|m| m.email == person.email) {
                    entry.members.push(person);
                }
            }
            cache.entry(external_id).or_default().push(message);
        }
        Ok(threads.into_values().collect())
    }

    async fn sync_messages(
        &self,
        _auth: &ProviderAuth,
        conversation_external_id: &str,
        _since: Option<DateTime<Utc>>,
        _limit: Option<u32>,
    ) -> Result<Vec<ResponseMessage>, String> {
        let cache = self.cache.lock().await;
        let mut messages = cache.get(conversation_external_id).cloned().unwrap_or_default();
        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(messages)
    }

    async fn send_message(
        &self,
        auth: &ProviderAuth,
        conversation_external_id: &str,
        draft: &OutgoingMessage,
    ) -> Result<ResponseMessage, String> {
        let address = draft.from.as_deref().map(str::to_lowercase).filter(|f| self.config.aliases.contains(f)).unwrap_or_else(|| self.config.email.clone());
        let from: Mailbox = format!("{} <{}>", self.config.display_name.replace(['<', '>'], ""), address)
            .parse()
            .map_err(|e| format!("The sender address is not valid: {}", e))?;
        let last = self.cache.lock().await.get(conversation_external_id).and_then(|list| list.iter().max_by_key(|m| m.timestamp).cloned());
        let (to, subject, reference) = Self::addressing(draft, last.as_ref());
        if to.is_empty() {
            return Err("A new mail needs at least one recipient".to_string());
        }
        let mut builder = Message::builder().from(from).subject(subject.clone());
        for address in &to {
            builder = builder.to(address.parse::<Mailbox>().map_err(|e| format!("{} is not a valid address: {}", address, e))?);
        }
        for address in &draft.cc {
            builder = builder.cc(address.parse::<Mailbox>().map_err(|e| format!("{} is not a valid address: {}", address, e))?);
        }
        for address in &draft.bcc {
            builder = builder.bcc(address.parse::<Mailbox>().map_err(|e| format!("{} is not a valid address: {}", address, e))?);
        }
        if let Some((id, chain)) = &reference {
            builder = builder.in_reply_to(id.clone()).references(chain.clone());
        }
        let body = draft.mail_text();
        let (plain, html) = match draft.content_type.as_str() {
            "html" => (crate::utils::text_extraction::extract_text_from_content(&body, "html"), Some(body.clone())),
            "markdown" => (crate::content::markdown::plain(&body), Some(draft.mail_html().unwrap_or_else(|| crate::content::markdown::html(&body)))),
            _ => (body.clone(), None),
        };
        let text = match html {
            Some(html) => MultiPart::alternative()
                .singlepart(SinglePart::builder().header(ContentType::TEXT_PLAIN).body(plain))
                .singlepart(SinglePart::builder().header(ContentType::TEXT_HTML).body(html)),
            None => MultiPart::alternative().singlepart(SinglePart::builder().header(ContentType::TEXT_PLAIN).body(plain)),
        };
        let message = if draft.attachments.is_empty() {
            builder.multipart(text).map_err(|e| format!("The mail could not be built: {}", e))?
        } else {
            let mut mixed = MultiPart::mixed().multipart(text);
            for attachment in &draft.attachments {
                let content_type = ContentType::parse(&attachment.mime_type).map_err(|e| format!("{} has an unusable type {}: {}", attachment.name, attachment.mime_type, e))?;
                mixed = mixed.singlepart(Attachment::new(attachment.name.clone()).body(attachment.bytes.clone(), content_type));
            }
            builder.multipart(mixed).map_err(|e| format!("The mail could not be built: {}", e))?
        };
        let transport = if self.config.allow_invalid_certs {
            Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.smtp_host))
        } else if self.config.smtp_port == 465 {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.smtp_host)
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.smtp_host)
        }
        .map_err(|e| format!("SMTP setup failed: {}", e))?
        .port(self.config.smtp_port)
        .credentials(Credentials::new(self.config.username.clone(), auth.access_token.clone()))
        .build();
        transport.send(message).await.map_err(|e| format!("{} refused the mail: {}", self.config.smtp_host, e))?;
        Ok(ResponseMessage {
            external_id: format!("sent/{}", Utc::now().timestamp_millis()),
            conversation_external_id: conversation_external_id.to_string(),
            sender: ResponseUser { id: self.config.email.clone(), email: Some(self.config.email.clone()), display_name: Some(self.config.display_name.clone()), ..Default::default() },
            content: body,
            content_type: draft.content_type.clone(),
            timestamp: Utc::now(),
            r#type: MessageType::Email,
            subject: Some(subject),
            recipients: to.iter().map(|a| ResponseUser { id: a.clone(), email: Some(a.clone()), ..Default::default() }).collect(),
            cc: draft.cc.iter().map(|a| ResponseUser { id: a.clone(), email: Some(a.clone()), ..Default::default() }).collect(),
            reply_to_id: draft.in_reply_to.clone().or(reference.map(|(id, _)| id)),
            attachments: draft
                .attachments
                .iter()
                .enumerate()
                .map(|(i, a)| ResponseAttachment { external_id: format!("{}", i + 1), name: a.name.clone(), content_type: a.mime_type.clone(), size: a.bytes.len() as u64, url: None, content_id: None })
                .collect(),
            ..Default::default()
        })
    }

    async fn mark_read(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str, read: bool) -> Result<(), String> {
        if message_external_id.starts_with("sent/") {
            return Ok(());
        }
        self.store_flag(auth, message_external_id, "\\Seen", read).await
    }

    async fn delete_message(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str) -> Result<(), String> {
        self.store_flag(auth, message_external_id, "\\Deleted", true).await
    }

    async fn get_live(&self, auth: &ProviderAuth, sink: LiveSink) -> Result<(), String> {
        let mut session = self.session(&auth.access_token).await?;
        loop {
            session.select("INBOX").await.map_err(|e| format!("Could not open INBOX: {}", e))?;
            let mut idle = session.idle();
            idle.init().await.map_err(|e| format!("{} does not hold IDLE: {}", self.config.imap_host, e))?;
            let (wait, _stop) = idle.wait_with_timeout(std::time::Duration::from_secs(IDLE_SECONDS));
            let woke = wait.await.map_err(|e| format!("IDLE on {} dropped: {}", self.config.imap_host, e))?;
            session = idle.done().await.map_err(|e| format!("IDLE on {} did not end: {}", self.config.imap_host, e))?;
            if let async_imap::extensions::idle::IdleResponse::NewData(_) = woke {
                let changed = LiveEvent::Conversation { conversation: ResponseConversation { external_id: String::new(), name: String::new(), conversation_type: ConversationType::Email, members: Vec::new(), last_activity: Utc::now(), metadata: HashMap::new() } };
                if sink.send(changed).await.is_err() {
                    let _ = session.logout().await;
                    return Ok(());
                }
            }
        }
    }

    async fn get_delta(&self, auth: &ProviderAuth, cursors: &HashMap<String, String>, since: DateTime<Utc>) -> Result<Delta, String> {
        let mut out = Delta { cursors: cursors.clone(), ..Default::default() };
        let mut session = self.session(&auth.access_token).await?;
        let condstore = session.capabilities().await.map(|c| c.has_str("CONDSTORE")).unwrap_or(false);
        let (set, query) = if condstore {
            let mailbox = session.select_condstore("INBOX").await.map_err(|e| format!("Could not open INBOX: {}", e))?;
            let now = Self::modseq_cursor(mailbox.uid_validity, mailbox.highest_modseq);
            let earlier = cursors.get(MODSEQ_CURSOR).and_then(|c| Self::changed_since(c, mailbox.uid_validity));
            if let Some(now) = now {
                out.cursors.insert(MODSEQ_CURSOR.to_string(), now);
            }
            match earlier {
                Some(modseq) => (Some("1:*".to_string()), format!("(UID FLAGS) (CHANGEDSINCE {})", modseq)),
                None => (None, "(UID FLAGS)".to_string()),
            }
        } else {
            session.select("INBOX").await.map_err(|e| format!("Could not open INBOX: {}", e))?;
            (None, "(UID FLAGS)".to_string())
        };
        let set = match set {
            Some(set) => Some(set),
            None => {
                let uids = session.uid_search(format!("SINCE {}", since.format("%d-%b-%Y"))).await.map_err(|e| format!("Search in INBOX failed: {}", e))?;
                (!uids.is_empty()).then(|| uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(","))
            }
        };
        if let Some(set) = set {
            let mut fetches = session.uid_fetch(set, query).await.map_err(|e| format!("Flags in INBOX were not read: {}", e))?;
            while let Some(fetch) = fetches.try_next().await.map_err(|e| format!("Flags in INBOX were not read: {}", e))? {
                let Some(uid) = fetch.uid else { continue };
                let flags: Vec<async_imap::types::Flag<'_>> = fetch.flags().collect();
                out.changes.push(RemoteChange {
                    message_external_id: format!("INBOX/{}", uid),
                    is_read: Some(flags.iter().any(|f| matches!(f, async_imap::types::Flag::Seen))),
                    flagged: Some(flags.iter().any(|f| matches!(f, async_imap::types::Flag::Flagged))),
                    deleted: flags.iter().any(|f| matches!(f, async_imap::types::Flag::Deleted)),
                    ..Default::default()
                });
            }
        }
        let _ = session.logout().await;
        Ok(out)
    }

    async fn get_search(&self, auth: &ProviderAuth, search: &RemoteSearch) -> Result<Vec<ResponseMessage>, String> {
        let quoted = |s: &str| format!("\"{}\"", s.replace(['"', '\\'], ""));
        let mut terms: Vec<String> = Vec::new();
        if !search.text.trim().is_empty() {
            terms.push(format!("TEXT {}", quoted(search.text.trim())));
        }
        if let Some(from) = &search.from {
            terms.push(format!("FROM {}", quoted(from)));
        }
        if let Some(after) = search.after {
            terms.push(format!("SINCE {}", after.format("%d-%b-%Y")));
        }
        if let Some(before) = search.before {
            terms.push(format!("BEFORE {}", before.format("%d-%b-%Y")));
        }
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let mut session = self.session(&auth.access_token).await?;
        session.select("INBOX").await.map_err(|e| format!("Could not open INBOX: {}", e))?;
        let uids = session.uid_search(terms.join(" ")).await.map_err(|e| format!("Search in INBOX failed: {}", e))?;
        let mut sorted: Vec<u32> = uids.into_iter().collect();
        sorted.sort_unstable_by(|a, b| b.cmp(a));
        sorted.truncate(search.limit.clamp(1, 200) as usize);
        let mut out = Vec::new();
        if !sorted.is_empty() {
            let set = sorted.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
            let mut fetches = session.uid_fetch(set, "(UID FLAGS BODY.PEEK[])").await.map_err(|e| format!("Fetch in INBOX failed: {}", e))?;
            while let Some(fetch) = fetches.try_next().await.map_err(|e| format!("Fetch in INBOX failed: {}", e))? {
                let Some(uid) = fetch.uid else { continue };
                let seen = fetch.flags().any(|f| matches!(f, async_imap::types::Flag::Seen));
                if let Some((key, mut message)) = fetch.body().and_then(|body| Self::parse("INBOX", uid, seen, body)) {
                    message.conversation_external_id = format!("thread:{}", key);
                    if search.conversation_external_id.as_deref().is_none_or(|c| c == message.conversation_external_id) {
                        out.push(message);
                    }
                }
            }
        }
        let _ = session.logout().await;
        Ok(out)
    }

    async fn put_remote_draft(&self, auth: &ProviderAuth, _conversation_external_id: &str, draft: &OutgoingMessage, existing_external_id: Option<&str>) -> Result<Option<String>, String> {
        let body = draft.mail_text();
        let html = draft.mail_html().unwrap_or_else(|| crate::content::markdown::html(&body));
        let boundary = format!("chatnip-draft-{}", Utc::now().timestamp_nanos_opt().unwrap_or_default());
        let mut raw = format!("From: {} <{}>\r\n", self.config.display_name.replace(['<', '>'], ""), draft.from.clone().unwrap_or_else(|| self.config.email.clone()));
        if !draft.to.is_empty() {
            raw.push_str(&format!("To: {}\r\n", draft.to.join(", ")));
        }
        if !draft.cc.is_empty() {
            raw.push_str(&format!("Cc: {}\r\n", draft.cc.join(", ")));
        }
        raw.push_str(&format!(
            "Subject: {}\r\nDate: {}\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"{}\"\r\n\r\n--{}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}\r\n--{}\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{}\r\n--{}--\r\n",
            draft.subject.clone().unwrap_or_default(),
            Utc::now().to_rfc2822(),
            boundary,
            boundary,
            crate::content::markdown::plain(&body),
            boundary,
            html,
            boundary
        ));
        let mut session = self.session(&auth.access_token).await?;
        let folder = Self::special_folder(&mut session, "drafts").await.ok_or_else(|| format!("{} has no drafts folder", self.config.imap_host))?;
        if let Some(existing) = existing_external_id {
            let (old_folder, uid) = Self::split_external(existing)?;
            session.select(&old_folder).await.map_err(|e| format!("Could not open {}: {}", old_folder, e))?;
            let mut updates = session.uid_store(uid.to_string(), "+FLAGS (\\Deleted)").await.map_err(|e| format!("The old draft was not replaced: {}", e))?;
            while let Ok(Some(_)) = updates.try_next().await {}
            drop(updates);
            let expunged = session.expunge().await.map_err(|e| format!("Expunge failed: {}", e))?;
            let mut expunged = std::pin::pin!(expunged);
            while let Ok(Some(_)) = expunged.try_next().await {}
        }
        session.append(&folder, Some("(\\Draft \\Seen)"), None, raw.as_bytes()).await.map_err(|e| format!("The draft was not saved: {}", e))?;
        session.select(&folder).await.map_err(|e| format!("Could not open {}: {}", folder, e))?;
        let newest = session.uid_search("ALL").await.map_err(|e| format!("Search in {} failed: {}", folder, e))?.into_iter().max();
        let _ = session.logout().await;
        Ok(newest.map(|uid| format!("{}/{}", folder, uid)))
    }

    async fn delete_remote_draft(&self, auth: &ProviderAuth, draft_external_id: &str) -> Result<bool, String> {
        self.store_flag(auth, draft_external_id, "\\Deleted", true).await?;
        Ok(true)
    }

    async fn put_flag(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str, flagged: bool) -> Result<bool, String> {
        if message_external_id.starts_with("sent/") {
            return Ok(false);
        }
        self.store_flag(auth, message_external_id, "\\Flagged", flagged).await?;
        Ok(true)
    }

    async fn put_archive(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str, archived: bool) -> Result<bool, String> {
        if message_external_id.starts_with("sent/") {
            return Ok(false);
        }
        let (folder, uid) = Self::split_external(message_external_id)?;
        let mut session = self.session(&auth.access_token).await?;
        let target = if archived { Self::special_folder(&mut session, "archive").await.ok_or_else(|| format!("{} has no archive folder", self.config.imap_host))? } else { "INBOX".to_string() };
        if target == folder {
            let _ = session.logout().await;
            return Ok(true);
        }
        session.select(&folder).await.map_err(|e| format!("Could not open {}: {}", folder, e))?;
        session.uid_mv(uid.to_string(), &target).await.map_err(|e| format!("The mail was not moved to {}: {}", target, e))?;
        let _ = session.logout().await;
        Ok(true)
    }

    async fn get_aliases(&self, _auth: &ProviderAuth) -> Result<Vec<RemoteAlias>, String> {
        let name = Some(self.config.display_name.clone()).filter(|n| !n.trim().is_empty());
        let mut out = vec![RemoteAlias { email: self.config.email.to_lowercase(), name: name.clone(), is_default: true, signature: None }];
        out.extend(self.config.aliases.iter().map(|email| RemoteAlias { email: email.clone(), name: name.clone(), is_default: false, signature: None }));
        Ok(out)
    }
}

const IDLE_SECONDS: u64 = 25 * 60;
const MODSEQ_CURSOR: &str = "imap:inbox:modseq";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imap_replies_to_the_quoted_mail_not_the_latest_and_forwards_start_fresh() {
        use crate::providers::traits::OutgoingReference;
        let mut last = ResponseMessage { subject: Some("Lunch".into()), sender: ResponseUser { email: Some("last@x.no".into()), ..Default::default() }, ..Default::default() };
        last.metadata.insert("internet_message_id".into(), Value::String("<last@x.no>".into()));
        let plain = OutgoingMessage::default();
        assert_eq!(ImapProvider::addressing(&plain, Some(&last)), (vec!["last@x.no".to_string()], "Re: Lunch".to_string(), Some(("<last@x.no>".to_string(), "<last@x.no>".to_string()))));
        let quote = OutgoingReference { sender_email: Some("anna@client.example".into()), subject: Some("Tilbud".into()), internet_message_id: Some("<q@client.example>".into()), references: Some("<p@client.example>".into()), ..Default::default() };
        let quoted = OutgoingMessage { quote: Some(quote.clone()), ..Default::default() };
        assert_eq!(ImapProvider::addressing(&quoted, Some(&last)), (vec!["anna@client.example".to_string()], "Re: Tilbud".to_string(), Some(("<q@client.example>".to_string(), "<p@client.example> <q@client.example>".to_string()))));
        let forward = OutgoingMessage { to: vec!["jonas@client.example".into()], subject: Some("Fwd: Tilbud".into()), forwarded: vec![quote], ..Default::default() };
        assert_eq!(ImapProvider::addressing(&forward, Some(&last)), (vec!["jonas@client.example".to_string()], "Fwd: Tilbud".to_string(), None));
    }

    #[test]
    fn subject_prefixes_are_stripped_repeatedly() {
        assert_eq!(ImapProvider::strip_prefixes("Re: Fwd: SV: Lunch"), "Lunch");
        assert_eq!(ImapProvider::strip_prefixes("Lunch"), "Lunch");
    }

    #[test]
    fn the_modseq_cursor_holds_only_while_the_uid_validity_holds() {
        assert_eq!(ImapProvider::modseq_cursor(Some(7), Some(900)).as_deref(), Some("7:900"));
        assert_eq!(ImapProvider::modseq_cursor(None, Some(900)), None);
        assert_eq!(ImapProvider::changed_since("7:900", Some(7)), Some(900));
        assert_eq!(ImapProvider::changed_since("7:900", Some(8)), None);
        assert_eq!(ImapProvider::changed_since("junk", Some(7)), None);
    }

    #[test]
    fn external_ids_split_into_folder_and_uid() {
        assert_eq!(ImapProvider::split_external("INBOX/42").unwrap(), ("INBOX".to_string(), 42));
        assert_eq!(ImapProvider::split_external("Sent Items/7").unwrap(), ("Sent Items".to_string(), 7));
        assert!(ImapProvider::split_external("nope").is_err());
    }

    #[test]
    fn a_raw_mail_parses_into_a_threaded_message() {
        let raw = b"Message-ID: <a@x>\r\nIn-Reply-To: <root@x>\r\nReferences: <root@x>\r\nFrom: Anna <anna@chatnip.local>\r\nTo: Milo <milo@chatnip.local>\r\nSubject: Re: Lunch\r\nDate: Thu, 9 Oct 2026 10:00:00 +0000\r\nContent-Type: text/plain\r\n\r\nSee you at noon";
        let (key, message) = ImapProvider::parse("INBOX", 5, false, raw).unwrap();
        assert_eq!(key, "root@x");
        assert_eq!(message.external_id, "INBOX/5");
        assert_eq!(message.subject.as_deref(), Some("Re: Lunch"));
        assert_eq!(message.sender.email.as_deref(), Some("anna@chatnip.local"));
        assert_eq!(message.recipients.len(), 1);
        assert!(!message.is_read);
        assert_eq!(message.content.trim(), "See you at noon");
    }
}

#[derive(Debug)]
struct AcceptAnyCertificate;

impl tokio_rustls::rustls::client::danger::ServerCertVerifier for AcceptAnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[tokio_rustls::rustls::pki_types::CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: tokio_rustls::rustls::pki_types::UnixTime,
    ) -> Result<tokio_rustls::rustls::client::danger::ServerCertVerified, tokio_rustls::rustls::Error> {
        Ok(tokio_rustls::rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<tokio_rustls::rustls::client::danger::HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<tokio_rustls::rustls::client::danger::HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<tokio_rustls::rustls::SignatureScheme> {
        tokio_rustls::rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod live {
    use super::*;

    fn config() -> Option<ImapConfig> {
        let host = std::env::var("CHATNIP_IMAP_TEST_HOST").ok()?;
        Some(ImapConfig {
            email: "milo@chatnip.local".to_string(),
            display_name: "Milo".to_string(),
            username: "milo".to_string(),
            imap_host: host.clone(),
            imap_port: 3993,
            smtp_host: host,
            smtp_port: 3025,
            allow_invalid_certs: true,
            aliases: Vec::new(),
        })
    }

    #[tokio::test]
    async fn sends_then_syncs_a_thread_against_a_local_server() {
        let Some(config) = config() else { return };
        let provider = ImapProvider::new(config);
        let auth = ProviderAuth {
            access_token: "secret".to_string(),
            refresh_token: None,
            expires_at: Utc::now() + Duration::days(1),
            token_type: "Password".to_string(),
            scope: None,
        };
        let identity = provider.identity(&auth).await.unwrap();
        assert_eq!(identity.namespace, "milo@chatnip.local");
        let draft = OutgoingMessage {
            content: "Lunch at noon?".to_string(),
            content_type: "text".to_string(),
            subject: Some("Lunch".to_string()),
            to: vec!["milo@chatnip.local".to_string()],
            ..Default::default()
        };
        provider.send_message(&auth, "thread:none", &draft).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let conversations = provider.sync_conversations(&auth, Some(Utc::now() - Duration::days(1))).await.unwrap();
        let lunch = conversations.iter().find(|c| c.name == "Lunch").expect("the sent mail arrives in INBOX");
        let messages = provider.sync_messages(&auth, &lunch.external_id, None, None).await.unwrap();
        assert!(!messages.is_empty());
        assert_eq!(messages[0].subject.as_deref(), Some("Lunch"));
        assert!(messages[0].content.contains("Lunch at noon?"));
        provider.mark_read(&auth, &lunch.external_id, &messages[0].external_id, true).await.unwrap();
        let again = provider.sync_conversations(&auth, Some(Utc::now() - Duration::days(1))).await.unwrap();
        let lunch_again = again.iter().find(|c| c.name == "Lunch").unwrap();
        let read = provider.sync_messages(&auth, &lunch_again.external_id, None, None).await.unwrap();
        assert!(read.iter().any(|m| m.is_read));
    }
}
