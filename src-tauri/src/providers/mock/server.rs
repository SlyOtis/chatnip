use chrono::{DateTime, Duration, SecondsFormat, Timelike, Utc};
use serde_json::{json, Value};
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

use crate::content::mail::Address;
use crate::models::app_setting::{self, AppSetting};
use crate::providers::models::ProviderAuth;
use crate::providers::traits::{ConversationType, MessageType, NewRemoteEvent, RemoteEmoji, OutgoingMessage, RemoteCalendar, RemoteEvent, ResponseAttachment, ResponseConversation, ResponseMessage, ResponseUser, SystemEvent, SystemEventKind, SystemPerson};

use super::main::{MockKind, MockProvider};

pub const STATE_OK: &str = "ok";
pub const STATE_EXPIRED: &str = "expired";
pub const STATE_RATE_LIMITED: &str = "rate_limited";
pub const STATE_OFFLINE: &str = "offline";
pub const STATE_DENIED: &str = "denied";
pub const STATES: [&str; 5] = [STATE_OK, STATE_EXPIRED, STATE_RATE_LIMITED, STATE_OFFLINE, STATE_DENIED];
pub const SIMULATION_TICK_SECONDS: u64 = 5;
pub const RESPONSES: [&str; 3] = ["accepted", "declined", "tentativelyAccepted"];

const CHAT_REPLIES: [&str; 12] = [
    "Takk! Jeg ser på det nå.",
    "👍",
    "Supert, da gjør vi det sånn.",
    "Kan vi ta det på standupen i morgen?",
    "Got it, I will get back to you after lunch.",
    "Enig. Jeg oppdaterer saken.",
    "Haha, ja 😄",
    "Sounds good to me.",
    "Jeg sjekker med Anna og svarer i ettermiddag.",
    "Perfekt, takk for raskt svar!",
    "Hm, kan du utdype litt?",
    "Done ✅",
];

const MAIL_REPLIES: [&str; 6] = [
    "Takk for e-posten. Jeg ser på dette i løpet av dagen og kommer tilbake til deg.",
    "Thanks Milo, that works for us. I will confirm with procurement and send the signed copy.",
    "Hei! Mottatt. Kan vi ta en kort prat torsdag for å gå gjennom detaljene?",
    "Takk, dette ser bra ut. Jeg videresender til juristene.",
    "Thanks! I have added it to the agenda for Monday.",
    "Hei Milo, flott. Jeg sender tallene i morgen tidlig.",
];

const DIGESTS: [(&str, &str); 4] = [
    ("TLDR: Tauri 2.6, SQLite JSONB and a faster WebKit", "<h2>Tech</h2><p>Tauri 2.6 lands smaller bundles (3 minute read).</p><p>SQLite adds JSONB indexes (4 minute read).</p>"),
    ("TLDR: Rust 1.93 and the end of async traits boxing", "<h2>Dev</h2><p>Rust 1.93 stabilises return position impl trait in traits everywhere (5 minute read).</p>"),
    ("TLDR: Svelte 6 beta, Bun 2 and local first apps", "<h2>Web</h2><p>Svelte 6 beta drops stores for good (2 minute read).</p><p>Local first apps go mainstream (6 minute read).</p>"),
    ("TLDR Weekly: the ten most read stories", "<h2>Weekly</h2><p>The ten stories readers opened most this week (8 minute read).</p>"),
];

const BOT_LINES: [&str; 4] = [
    "✅ Build #482 on main passed in 4m 12s",
    "🔀 PR #57 \"Mock server round trip\" was merged by sam",
    "⚠️ Deploy to staging is waiting for approval",
    "🐛 Issue #61 \"Typing indicator flickers\" was opened by mike",
];

pub const CUSTOM_EMOJI: [(&str, &[[u8; 3]]); 3] = [
    ("party_parrot", &[[255, 90, 90], [255, 200, 60], [120, 255, 120], [90, 200, 255], [190, 120, 255]]),
    ("shipit", &[[31, 157, 85], [76, 199, 126]]),
    ("chatnip", &[[224, 97, 47]]),
];
pub const CUSTOM_ALIASES: [(&str, &str); 2] = [("partyparrot", "party_parrot"), ("party-parrot", "party_parrot")];

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS mock_server_account (id TEXT PRIMARY KEY, kind TEXT NOT NULL, state TEXT NOT NULL DEFAULT 'ok', newsletter_at TEXT, created_at TEXT NOT NULL)",
        "CREATE TABLE IF NOT EXISTS mock_server_conversation (id TEXT PRIMARY KEY, account TEXT NOT NULL, external_id TEXT NOT NULL, body TEXT NOT NULL, modified_at TEXT NOT NULL, UNIQUE (account, external_id))",
        "CREATE TABLE IF NOT EXISTS mock_server_message (id TEXT PRIMARY KEY, account TEXT NOT NULL, external_id TEXT NOT NULL, conversation_external_id TEXT NOT NULL, body TEXT NOT NULL, timestamp TEXT NOT NULL, modified_at TEXT NOT NULL, UNIQUE (account, external_id))",
        "CREATE INDEX IF NOT EXISTS idx_mock_server_message_conversation ON mock_server_message (account, conversation_external_id, modified_at)",
        "CREATE TABLE IF NOT EXISTS mock_server_event (id TEXT PRIMARY KEY, account TEXT NOT NULL, calendar_external_id TEXT NOT NULL, external_id TEXT NOT NULL, body TEXT NOT NULL, modified_at TEXT NOT NULL, UNIQUE (account, external_id))",
        "CREATE TABLE IF NOT EXISTS mock_server_pending (id TEXT PRIMARY KEY, account TEXT NOT NULL, due_at TEXT NOT NULL, body TEXT NOT NULL)",
        "CREATE TABLE IF NOT EXISTS mock_server_setting (account TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL, PRIMARY KEY (account, key))",
    ] {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}

pub fn account(kind: MockKind, auth: &ProviderAuth) -> String {
    format!("{}:{}", kind.key(), auth.access_token)
}

fn stamp(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Micros, true)
}

fn read(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw).ok().map(|at| at.with_timezone(&Utc))
}

fn spread(text: &str) -> usize {
    text.bytes().fold(7usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize))
}

pub fn refusal(kind: MockKind, state: &str, write: bool) -> Option<String> {
    let graph = matches!(kind, MockKind::Teams | MockKind::Outlook);
    let google = matches!(kind, MockKind::Gmail | MockKind::GoogleChat);
    let text = match state {
        STATE_EXPIRED => match kind {
            _ if graph => "Graph 401 Unauthorized: InvalidAuthenticationToken: Lifetime validation failed, the token is expired.",
            MockKind::Slack => "Slack: token_expired",
            _ if google => "Google 401 Unauthorized: Request had invalid authentication credentials. Expected OAuth 2 access token.",
            _ => "IMAP login failed: [AUTHENTICATIONFAILED] Invalid credentials (Failure)",
        },
        STATE_RATE_LIMITED => match kind {
            _ if graph => "Graph 429 Too Many Requests: TooManyRequests: Too many requests, retry after 30 seconds.",
            MockKind::Slack => "Slack: ratelimited",
            _ if google => "Google 429 Too Many Requests: Quota exceeded for quota metric 'Queries' and limit 'Queries per minute per user'.",
            _ => "IMAP: [LIMIT] Too many simultaneous connections. (Failure)",
        },
        STATE_OFFLINE => match kind {
            _ if graph => "error sending request for url (https://graph.microsoft.com/v1.0/me/chats): client error (Connect)",
            MockKind::Slack => "Slack request failed: error sending request for url (https://slack.com/api/conversations.list): client error (Connect)",
            _ if google => "error sending request for url (https://gmail.googleapis.com/gmail/v1/users/me/threads): client error (Connect)",
            _ => "IMAP connection to imap.mock.chatnip:993 failed: Connection refused (os error 61)",
        },
        STATE_DENIED if write => match kind {
            _ if graph => "Graph 403 Forbidden: Forbidden: Missing scope permissions on the request. API requires one of 'ChatMessage.Send, Mail.Send'.",
            MockKind::Slack => "Slack: missing_scope",
            _ if google => "Google 403 Forbidden: Request had insufficient authentication scopes.",
            _ => "SMTP: 550 5.7.1 Sender address rejected: not owned by the authenticated user",
        },
        _ => return None,
    };
    Some(text.to_string())
}

pub async fn get_state(pool: &Pool<Sqlite>, account: &str) -> Result<String, String> {
    schema(pool).await.map_err(|e| format!("The mock server could not start: {}", e))?;
    let state: Option<String> = sqlx::query_scalar("SELECT state FROM mock_server_account WHERE id = ?1").bind(account).fetch_optional(pool).await.map_err(|e| format!("The mock server could not read its state: {}", e))?;
    Ok(state.unwrap_or_else(|| STATE_OK.to_string()))
}

pub async fn put_state(pool: &Pool<Sqlite>, account: &str, kind: MockKind, state: &str) -> Result<(), String> {
    if !STATES.contains(&state) {
        return Err(format!("There is no mock connection state called {}", state));
    }
    schema(pool).await.map_err(|e| format!("The mock server could not start: {}", e))?;
    sqlx::query("INSERT INTO mock_server_account (id, kind, state, newsletter_at, created_at) VALUES (?1, ?2, ?3, ?4, ?4) ON CONFLICT(id) DO UPDATE SET state = excluded.state")
        .bind(account)
        .bind(kind.key())
        .bind(state)
        .bind(stamp(Utc::now()))
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| format!("The mock server could not change its state: {}", e))
}

pub async fn delete_account(pool: &Pool<Sqlite>, account: &str) -> Result<(), String> {
    schema(pool).await.map_err(|e| format!("The mock server could not start: {}", e))?;
    for table in ["mock_server_pending", "mock_server_event", "mock_server_message", "mock_server_conversation", "mock_server_setting"] {
        sqlx::query(&format!("DELETE FROM {} WHERE account = ?1", table)).bind(account).execute(pool).await.map_err(|e| format!("The mock server could not forget {}: {}", account, e))?;
    }
    sqlx::query("DELETE FROM mock_server_account WHERE id = ?1").bind(account).execute(pool).await.map(|_| ()).map_err(|e| format!("The mock server could not forget {}: {}", account, e))
}

pub async fn due(pool: &Pool<Sqlite>, account: &str, now: DateTime<Utc>) -> Result<bool, String> {
    if get_state(pool, account).await? != STATE_OK {
        return Ok(false);
    }
    let pending: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM mock_server_pending WHERE account = ?1 AND due_at <= ?2").bind(account).bind(stamp(now)).fetch_one(pool).await.map_err(|e| e.to_string())?;
    if pending {
        return Ok(true);
    }
    let minutes = AppSetting::get_u64(pool, app_setting::MOCK_NEWSLETTER_MINUTES).await?;
    if minutes == 0 {
        return Ok(false);
    }
    let last: Option<Option<String>> = sqlx::query_scalar("SELECT newsletter_at FROM mock_server_account WHERE id = ?1").bind(account).fetch_optional(pool).await.map_err(|e| e.to_string())?;
    Ok(match last {
        Some(Some(at)) => read(&at).is_none_or(|at| now - at >= Duration::minutes(minutes as i64)),
        _ => false,
    })
}

pub async fn accounts(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<(String, String)>, String> {
    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM provider WHERE profile_id = ?1 AND provider_type = 'mock'").bind(profile_id).fetch_all(pool).await.map_err(|e| format!("Failed to list the mock connections: {}", e))?;
    let mut out = Vec::new();
    for id in ids {
        let provider = crate::providers::models::Provider::load_from_db(pool, &id).await?;
        let (Some(auth), Some(kind)) = (provider.auth.as_ref(), provider.config.as_deref().and_then(|c| MockKind::parse(c).ok())) else { continue };
        out.push((id, account(kind, auth)));
    }
    Ok(out)
}

pub async fn get_live(mock: &MockProvider, auth: &ProviderAuth, sink: crate::providers::traits::LiveSink) -> Result<(), String> {
    use crate::providers::traits::LiveEvent;
    let account = account(mock.kind(), auth);
    let mut typed: Vec<String> = Vec::new();
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(SIMULATION_TICK_SECONDS)).await;
        let now = Utc::now();
        let server = Server::open(mock, auth).await?;
        if !mock.kind().is_mail() {
            for (id, conversation, from) in server.get_typing(now + Duration::seconds(SIMULATION_TICK_SECONDS as i64)).await? {
                if typed.contains(&id) {
                    continue;
                }
                server.put_pending_due(&id, now + Duration::seconds(SIMULATION_TICK_SECONDS as i64)).await?;
                typed.push(id);
                if sink.send(LiveEvent::Typing { conversation_external_id: conversation, user: from }).await.is_err() {
                    return Ok(());
                }
            }
        }
        if !due(mock.pool(), &account, now).await? {
            continue;
        }
        let event = LiveEvent::Conversation { conversation: ResponseConversation { external_id: String::new(), name: String::new(), conversation_type: ConversationType::Unknown, members: Vec::new(), last_activity: now, metadata: Default::default() } };
        if sink.send(event).await.is_err() {
            return Ok(());
        }
        typed.clear();
    }
}

pub struct Server<'a> {
    pool: &'a Pool<Sqlite>,
    mock: &'a MockProvider,
    pub account: String,
}

impl<'a> Server<'a> {
    pub async fn open(mock: &'a MockProvider, auth: &ProviderAuth) -> Result<Server<'a>, String> {
        let pool = mock.pool();
        schema(pool).await.map_err(|e| format!("The mock server could not start: {}", e))?;
        let server = Server { pool, mock, account: account(mock.kind(), auth) };
        server.seed().await?;
        Ok(server)
    }

    async fn seed(&self) -> Result<(), String> {
        let now = Utc::now();
        let modified = stamp(now);
        let mut tx = self.pool.begin().await.map_err(|e| format!("The mock server could not start: {}", e))?;
        let claimed = sqlx::query("INSERT OR IGNORE INTO mock_server_account (id, kind, state, newsletter_at, created_at) VALUES (?1, ?2, 'ok', ?3, ?3)")
            .bind(&self.account)
            .bind(self.mock.kind().key())
            .bind(&modified)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("The mock server could not open the account: {}", e))?
            .rows_affected();
        let seeded: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM mock_server_conversation WHERE account = ?1").bind(&self.account).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
        if claimed == 0 && seeded {
            return tx.commit().await.map_err(|e| e.to_string());
        }
        for (conversation, messages) in self.mock.dataset() {
            sqlx::query("INSERT OR IGNORE INTO mock_server_conversation (id, account, external_id, body, modified_at) VALUES (?1, ?2, ?3, ?4, ?5)")
                .bind(Uuid::new_v4().to_string())
                .bind(&self.account)
                .bind(&conversation.external_id)
                .bind(serde_json::to_string(&conversation).map_err(|e| e.to_string())?)
                .bind(&modified)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("The mock server could not hold {}: {}", conversation.name, e))?;
            for message in messages {
                sqlx::query("INSERT OR IGNORE INTO mock_server_message (id, account, external_id, conversation_external_id, body, timestamp, modified_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")
                    .bind(Uuid::new_v4().to_string())
                    .bind(&self.account)
                    .bind(&message.external_id)
                    .bind(&conversation.external_id)
                    .bind(serde_json::to_string(&message).map_err(|e| e.to_string())?)
                    .bind(stamp(message.timestamp))
                    .bind(&modified)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("The mock server could not hold a message: {}", e))?;
            }
        }
        for calendar in self.mock.calendars() {
            for event in self.mock.calendar_events(&calendar.external_id) {
                sqlx::query("INSERT OR IGNORE INTO mock_server_event (id, account, calendar_external_id, external_id, body, modified_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
                    .bind(Uuid::new_v4().to_string())
                    .bind(&self.account)
                    .bind(&calendar.external_id)
                    .bind(&event.external_id)
                    .bind(serde_json::to_string(&event).map_err(|e| e.to_string())?)
                    .bind(&modified)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("The mock server could not hold an event: {}", e))?;
            }
        }
        tx.commit().await.map_err(|e| format!("The mock server could not finish starting: {}", e))
    }

    pub async fn state(&self) -> Result<String, String> {
        get_state(self.pool, &self.account).await
    }

    pub async fn refuse(&self, write: bool) -> Result<(), String> {
        match refusal(self.mock.kind(), &self.state().await?, write) {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    async fn get_conversation(&self, external_id: &str) -> Result<Option<ResponseConversation>, String> {
        let body: Option<String> = sqlx::query_scalar("SELECT body FROM mock_server_conversation WHERE account = ?1 AND external_id = ?2").bind(&self.account).bind(external_id).fetch_optional(self.pool).await.map_err(|e| e.to_string())?;
        Ok(body.and_then(|b| serde_json::from_str(&b).ok()))
    }

    pub async fn get_message(&self, external_id: &str) -> Result<Option<ResponseMessage>, String> {
        let body: Option<String> = sqlx::query_scalar("SELECT body FROM mock_server_message WHERE account = ?1 AND external_id = ?2").bind(&self.account).bind(external_id).fetch_optional(self.pool).await.map_err(|e| e.to_string())?;
        Ok(body.and_then(|b| serde_json::from_str(&b).ok()))
    }

    async fn put_conversation(&self, conversation: &ResponseConversation, at: DateTime<Utc>) -> Result<(), String> {
        sqlx::query("INSERT INTO mock_server_conversation (id, account, external_id, body, modified_at) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(account, external_id) DO UPDATE SET body = excluded.body, modified_at = excluded.modified_at")
            .bind(Uuid::new_v4().to_string())
            .bind(&self.account)
            .bind(&conversation.external_id)
            .bind(serde_json::to_string(conversation).map_err(|e| e.to_string())?)
            .bind(stamp(at))
            .execute(self.pool)
            .await
            .map(|_| ())
            .map_err(|e| format!("The mock server could not keep {}: {}", conversation.name, e))
    }

    async fn put_message(&self, message: &ResponseMessage, at: DateTime<Utc>) -> Result<(), String> {
        sqlx::query("INSERT INTO mock_server_message (id, account, external_id, conversation_external_id, body, timestamp, modified_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(account, external_id) DO UPDATE SET body = excluded.body, modified_at = excluded.modified_at")
            .bind(Uuid::new_v4().to_string())
            .bind(&self.account)
            .bind(&message.external_id)
            .bind(&message.conversation_external_id)
            .bind(serde_json::to_string(message).map_err(|e| e.to_string())?)
            .bind(stamp(message.timestamp))
            .bind(stamp(at))
            .execute(self.pool)
            .await
            .map_err(|e| format!("The mock server could not keep the message: {}", e))?;
        if let Some(mut conversation) = self.get_conversation(&message.conversation_external_id).await? {
            conversation.last_activity = conversation.last_activity.max(message.timestamp);
            self.put_conversation(&conversation, at).await?;
        }
        Ok(())
    }

    pub async fn get_conversations(&self, since: Option<DateTime<Utc>>) -> Result<Vec<ResponseConversation>, String> {
        self.refuse(false).await?;
        self.deliver(Utc::now()).await?;
        let rows: Vec<String> = sqlx::query_scalar("SELECT body FROM mock_server_conversation WHERE account = ?1 AND modified_at >= ?2 ORDER BY modified_at DESC")
            .bind(&self.account)
            .bind(since.map(stamp).unwrap_or_default())
            .fetch_all(self.pool)
            .await
            .map_err(|e| format!("The mock server could not list conversations: {}", e))?;
        Ok(rows.iter().filter_map(|b| serde_json::from_str(b).ok()).collect())
    }

    pub async fn get_messages(&self, conversation_external_id: &str, since: Option<DateTime<Utc>>, limit: Option<u32>) -> Result<Vec<ResponseMessage>, String> {
        self.refuse(false).await?;
        let rows: Vec<String> = sqlx::query_scalar("SELECT body FROM mock_server_message WHERE account = ?1 AND conversation_external_id = ?2 AND modified_at > ?3 ORDER BY timestamp DESC, external_id DESC LIMIT ?4")
            .bind(&self.account)
            .bind(conversation_external_id)
            .bind(since.map(stamp).unwrap_or_default())
            .bind(limit.map(|l| l as i64).unwrap_or(-1))
            .fetch_all(self.pool)
            .await
            .map_err(|e| format!("The mock server could not list messages: {}", e))?;
        Ok(rows.iter().filter_map(|b| serde_json::from_str(b).ok()).collect())
    }

    fn user(&self, email: &str) -> ResponseUser {
        self.mock.addressee(&Address { name: None, email: email.trim().to_lowercase() })
    }

    fn server_id(&self, at: DateTime<Utc>) -> String {
        let unique = Uuid::new_v4().simple().to_string();
        match self.mock.kind() {
            MockKind::Teams | MockKind::Outlook => format!("AAMkAD{}", &unique[..24]),
            MockKind::Slack => format!("{}.{:06}", at.timestamp(), at.timestamp_subsec_micros()),
            MockKind::Gmail | MockKind::GoogleChat => format!("18{}", &unique[..14]),
            MockKind::Imap => format!("{}{}", at.timestamp_millis(), &unique[..4]),
        }
    }

    fn mail_headers(&self, id: &str, from: &ResponseUser, to: &[ResponseUser], cc: &[ResponseUser], subject: &str, at: DateTime<Utc>, reply: Option<(String, String)>) -> String {
        let address = |u: &ResponseUser| match (&u.display_name, &u.email) {
            (Some(name), Some(email)) => format!("{} <{}>", name, email),
            (_, Some(email)) => email.clone(),
            (Some(name), None) => name.clone(),
            _ => String::new(),
        };
        let mut lines = vec![
            format!("Received: from mx.mock.chatnip (mx.mock.chatnip [127.0.0.1]) by inbox.mock.chatnip with ESMTPS; {}", at.to_rfc2822()),
            format!("Message-ID: <{}@mock.chatnip>", id),
            format!("Date: {}", at.to_rfc2822()),
            format!("From: {}", address(from)),
        ];
        if !to.is_empty() {
            lines.push(format!("To: {}", to.iter().map(address).collect::<Vec<_>>().join(", ")));
        }
        if !cc.is_empty() {
            lines.push(format!("Cc: {}", cc.iter().map(address).collect::<Vec<_>>().join(", ")));
        }
        lines.push(format!("Subject: {}", subject));
        if let Some((in_reply_to, references)) = reply {
            lines.push(format!("In-Reply-To: {}", in_reply_to));
            lines.push(format!("References: {}", references));
        }
        lines.push("X-Mailer: Chatnip mock server".to_string());
        lines.join("\r\n")
    }

    pub async fn post_message(&self, conversation_external_id: &str, draft: &OutgoingMessage) -> Result<ResponseMessage, String> {
        self.refuse(true).await?;
        let kind = self.mock.kind();
        let now = Utc::now();
        let id = self.server_id(now);
        let me = self.mock.me();
        let mine = |u: &ResponseUser| u.id == me.id || u.email.as_deref().is_some_and(|e| Some(e) == me.email.as_deref());
        let conversation = self.get_conversation(conversation_external_id).await?;
        let mut message = ResponseMessage {
            external_id: id.clone(),
            conversation_external_id: conversation_external_id.to_string(),
            sender: me.clone(),
            content: if kind.is_mail() { draft.mail_html().unwrap_or_else(|| draft.mail_text()) } else { draft.content.clone() },
            content_type: if kind.is_mail() && draft.mail_html().is_some() { "html".to_string() } else { draft.content_type.clone() },
            timestamp: now,
            r#type: if kind.is_mail() { MessageType::Email } else { MessageType::Chat },
            is_read: true,
            reply_to_id: draft.in_reply_to.clone(),
            thread_id: draft.thread_external_id.clone(),
            ..Default::default()
        };
        if kind.is_mail() {
            let others: Vec<ResponseUser> = conversation.as_ref().map(|c| c.members.iter().filter(|m| !mine(m)).cloned().collect()).unwrap_or_default();
            let to: Vec<ResponseUser> = if draft.to.is_empty() { others } else { draft.to.iter().map(|a| self.user(a)).filter(|u| !mine(u)).collect() };
            let cc: Vec<ResponseUser> = draft.cc.iter().map(|a| self.user(a)).filter(|u| !mine(u) && !to.iter().any(|t| t.email == u.email)).collect();
            let subject = draft.subject.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| crate::providers::traits::prefixed_subject("Re", &conversation.as_ref().map(|c| c.name.clone()).unwrap_or_default()));
            let reply = draft.quote.as_ref().and_then(|q| q.reply_headers());
            message.headers = Some(self.mail_headers(&id, &me, &to, &cc, &subject, now, reply));
            message.metadata.insert("internet_message_id".to_string(), json!(format!("<{}@mock.chatnip>", id)));
            message.subject = Some(subject);
            message.recipients = to;
            message.cc = cc;
        } else {
            message.subject = draft.subject.clone();
            message.recipients = draft.to.iter().map(|a| self.user(a)).collect();
        }
        use base64::Engine as _;
        message.attachments = draft
            .attachments
            .iter()
            .enumerate()
            .map(|(i, file)| ResponseAttachment { external_id: format!("{}-file{}", id, i), name: file.name.clone(), content_type: file.mime_type.clone(), size: file.bytes.len() as u64, url: Some(format!("data:{};base64,{}", file.mime_type, base64::engine::general_purpose::STANDARD.encode(&file.bytes))), content_id: None })
            .collect();
        if let Some(quote) = &draft.quote {
            message.reply_to_id = Some(quote.external_id.clone());
            message.metadata.insert("quote".to_string(), json!({ "id": quote.external_id, "sender": quote.sender_name, "text": quote.text, "native": quote.native }));
        }
        if !draft.forwarded.is_empty() {
            message.metadata.insert("forwarded".to_string(), json!(draft.forwarded.iter().map(|f| json!({ "id": f.external_id, "sender": f.sender_name, "native": f.native })).collect::<Vec<_>>()));
        }
        message.metadata.insert("delivery".to_string(), json!("delivered"));
        if kind.is_mail() {
            let html = message.content_type == "html";
            let text = message.content.clone();
            crate::content::mail::analyse(html.then_some(text.as_str()), (!html).then_some(text.as_str()), message.headers.as_deref(), None).apply(&mut message);
        } else {
            crate::content::chat::markdown(&draft.content).apply(&mut message);
        }
        if conversation.is_none() {
            let mut members = vec![me.clone()];
            members.extend(message.recipients.iter().cloned());
            let name = message.subject.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| message.recipients.iter().filter_map(|r| r.display_name.clone()).collect::<Vec<_>>().join(", "));
            let created = ResponseConversation { external_id: conversation_external_id.to_string(), name, conversation_type: if kind.is_mail() { ConversationType::Email } else if members.len() > 2 { ConversationType::GroupMessage } else { ConversationType::DirectMessage }, members, last_activity: now, metadata: Default::default() };
            self.put_conversation(&created, now).await?;
        }
        self.put_message(&message, now).await?;
        let seconds = AppSetting::get_u64(self.pool, app_setting::MOCK_REPLY_SECONDS).await?;
        if seconds > 0 {
            let conversation = self.get_conversation(conversation_external_id).await?;
            if let Some(from) = self.replier(conversation.as_ref(), &message) {
                self.post_pending(now + Duration::seconds(seconds as i64), json!({ "kind": "reply", "message": message.external_id, "from": from })).await?;
            }
        }
        Ok(message)
    }

    fn replier(&self, conversation: Option<&ResponseConversation>, message: &ResponseMessage) -> Option<ResponseUser> {
        let me = self.mock.me();
        let real = |u: &ResponseUser| u.id != me.id && u.email.as_deref() != me.email.as_deref() && !crate::services::people_service::is_service(u.email.as_deref(), u.display_name.as_deref());
        let mut others: Vec<ResponseUser> = message.recipients.iter().filter(|u| real(u)).cloned().collect();
        if others.is_empty() {
            others = conversation.map(|c| c.members.iter().filter(|u| real(u)).cloned().collect()).unwrap_or_default();
        }
        if others.is_empty() {
            return None;
        }
        let index = spread(&message.external_id) % others.len();
        Some(others.swap_remove(index))
    }

    async fn post_pending(&self, at: DateTime<Utc>, body: Value) -> Result<(), String> {
        sqlx::query("INSERT INTO mock_server_pending (id, account, due_at, body) VALUES (?1, ?2, ?3, ?4)")
            .bind(Uuid::new_v4().to_string())
            .bind(&self.account)
            .bind(stamp(at))
            .bind(body.to_string())
            .execute(self.pool)
            .await
            .map(|_| ())
            .map_err(|e| format!("The mock server could not plan an answer: {}", e))
    }

    async fn own(&self, external_id: &str, verb: &str) -> Result<ResponseMessage, String> {
        let message = self.get_message(external_id).await?.ok_or_else(|| format!("404 Not Found: the message {} does not exist on the server", external_id))?;
        if message.sender.id != self.mock.me().id {
            return Err(format!("403 Forbidden: you can only {} your own messages", verb));
        }
        Ok(message)
    }

    pub async fn put_content(&self, external_id: &str, content: &str) -> Result<(), String> {
        self.refuse(true).await?;
        if self.mock.kind().is_mail() {
            return Err("Editing is not supported by this provider".to_string());
        }
        let mut message = self.own(external_id, "edit").await?;
        let now = Utc::now();
        message.content = content.to_string();
        message.content_type = "text".to_string();
        message.metadata.remove("structure");
        crate::content::chat::markdown(content).apply(&mut message);
        message.metadata.insert("edited_at".to_string(), json!(now.to_rfc3339()));
        self.put_message(&message, now).await
    }

    pub async fn delete_message(&self, external_id: &str) -> Result<(), String> {
        self.refuse(true).await?;
        if self.mock.kind().is_mail() {
            return Err("Message deletion not supported by this provider".to_string());
        }
        let mut message = self.own(external_id, "delete").await?;
        let now = Utc::now();
        message.content = String::new();
        message.attachments.clear();
        message.metadata.insert("deleted_at".to_string(), json!(now.to_rfc3339()));
        self.put_message(&message, now).await
    }

    pub async fn put_reaction(&self, external_id: &str, reaction: &str, active: bool) -> Result<(), String> {
        self.refuse(true).await?;
        if self.mock.kind().is_mail() {
            return Err("Mail has no reactions".to_string());
        }
        let mut message = self.get_message(external_id).await?.ok_or_else(|| format!("404 Not Found: the message {} does not exist on the server", external_id))?;
        let me = self.mock.me().display_name.unwrap_or_default();
        let mut reactions: Vec<Value> = message.metadata.get("reactions").and_then(|r| r.as_array()).cloned().unwrap_or_default();
        reactions.retain(|r| !(r["emoji"] == reaction && r["user"] == me.as_str()));
        if active {
            reactions.push(json!({ "emoji": reaction, "user": me, "at": Utc::now().to_rfc3339() }));
        }
        message.metadata.insert("reactions".to_string(), Value::Array(reactions));
        self.put_message(&message, Utc::now()).await
    }

    async fn post_system(&self, conversation: &ResponseConversation, event: SystemEvent) -> Result<(), String> {
        let now = Utc::now();
        let mut message = ResponseMessage::system(self.server_id(now), conversation.external_id.clone(), now, event);
        message.sender = self.mock.me();
        message.is_read = true;
        let text = message.content.clone();
        crate::content::chat::markdown(&text).apply(&mut message);
        self.put_message(&message, now).await
    }

    fn member(&self, external_id: &str) -> ResponseUser {
        let prefix = format!("mock-{}-u", self.mock.kind().key());
        match external_id.strip_prefix(&prefix).and_then(|i| i.parse::<usize>().ok()) {
            Some(index) => self.mock.person(index),
            None => ResponseUser { id: external_id.to_string(), display_name: Some(external_id.to_string()), ..Default::default() },
        }
    }

    pub async fn put_member(&self, conversation_external_id: &str, user_external_id: &str, member: bool) -> Result<bool, String> {
        if self.mock.kind().is_mail() {
            return Ok(false);
        }
        self.refuse(true).await?;
        let mut conversation = self.get_conversation(conversation_external_id).await?.ok_or_else(|| format!("404 Not Found: the chat {} does not exist on the server", conversation_external_id))?;
        if conversation.conversation_type == ConversationType::DirectMessage {
            return Err("400 Bad Request: people cannot be added to or removed from a one to one chat".to_string());
        }
        let user = self.member(user_external_id);
        let present = conversation.members.iter().any(|m| m.id == user.id);
        if present == member {
            return Ok(true);
        }
        if member {
            conversation.members.push(user.clone());
        } else {
            conversation.members.retain(|m| m.id != user.id);
        }
        self.put_conversation(&conversation, Utc::now()).await?;
        let kind = if member { SystemEventKind::MemberAdded } else { SystemEventKind::MemberRemoved };
        self.post_system(&conversation, SystemEvent::new(kind, vec![SystemPerson::from(&self.mock.me())], vec![SystemPerson::from(&user)], None)).await?;
        Ok(true)
    }

    pub async fn put_name(&self, conversation_external_id: &str, name: &str) -> Result<bool, String> {
        if self.mock.kind().is_mail() {
            return Ok(false);
        }
        self.refuse(true).await?;
        let mut conversation = self.get_conversation(conversation_external_id).await?.ok_or_else(|| format!("404 Not Found: the chat {} does not exist on the server", conversation_external_id))?;
        if conversation.conversation_type == ConversationType::DirectMessage {
            return Err("400 Bad Request: a one to one chat cannot be renamed".to_string());
        }
        conversation.name = name.trim().to_string();
        self.put_conversation(&conversation, Utc::now()).await?;
        self.post_system(&conversation, SystemEvent::new(SystemEventKind::Renamed, vec![SystemPerson::from(&self.mock.me())], Vec::new(), Some(conversation.name.clone()))).await?;
        Ok(true)
    }

    pub async fn put_pin(&self, external_id: &str, pinned: bool) -> Result<bool, String> {
        if self.mock.kind().is_mail() {
            return Ok(false);
        }
        self.refuse(true).await?;
        let mut message = self.get_message(external_id).await?.ok_or_else(|| format!("404 Not Found: the message {} does not exist on the server", external_id))?;
        message.metadata.insert("pinned".to_string(), json!(pinned));
        self.put_message(&message, Utc::now()).await?;
        let Some(conversation) = self.get_conversation(&message.conversation_external_id).await? else { return Ok(true) };
        let kind = if pinned { SystemEventKind::Pinned } else { SystemEventKind::Unpinned };
        self.post_system(&conversation, SystemEvent::new(kind, vec![SystemPerson::from(&self.mock.me())], Vec::new(), None)).await?;
        Ok(true)
    }

    pub async fn put_read(&self, external_id: &str, read: bool) -> Result<(), String> {
        self.refuse(false).await?;
        let Some(mut message) = self.get_message(external_id).await? else { return Ok(()) };
        if message.is_read == read {
            return Ok(());
        }
        message.is_read = read;
        self.put_message(&message, Utc::now()).await
    }

    pub async fn get_emoji(&self) -> Result<Vec<RemoteEmoji>, String> {
        self.refuse(false).await?;
        if self.mock.kind() != MockKind::Slack {
            return Ok(Vec::new());
        }
        let mut out: Vec<RemoteEmoji> = CUSTOM_EMOJI
            .iter()
            .map(|(name, frames)| RemoteEmoji { name: name.to_string(), url: Some(format!("data:image/gif;base64,{}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, super::seed::gif(32, 32, frames)))), alias_for: None })
            .collect();
        out.extend(CUSTOM_ALIASES.iter().map(|(name, target)| RemoteEmoji { name: name.to_string(), url: None, alias_for: Some(target.to_string()) }));
        Ok(out)
    }

    pub async fn get_presence(&self, user_id: &str) -> Result<String, String> {
        self.refuse(false).await?;
        if self.mock.kind().is_mail() {
            return Err("Presence not supported by this provider".to_string());
        }
        let slot = (Utc::now().minute() / 10) as usize;
        Ok(["online", "away", "busy", "online", "offline", "online"][(spread(user_id) + slot) % 6].to_string())
    }

    pub async fn get_calendars(&self) -> Result<Vec<RemoteCalendar>, String> {
        self.refuse(false).await?;
        Ok(self.mock.calendars())
    }

    async fn get_event(&self, external_id: &str) -> Result<Option<(String, RemoteEvent)>, String> {
        let row = sqlx::query("SELECT calendar_external_id, body FROM mock_server_event WHERE account = ?1 AND external_id = ?2").bind(&self.account).bind(external_id).fetch_optional(self.pool).await.map_err(|e| e.to_string())?;
        Ok(row.and_then(|r| serde_json::from_str(&r.get::<String, _>("body")).ok().map(|e| (r.get("calendar_external_id"), e))))
    }

    async fn put_event(&self, calendar_external_id: &str, event: &RemoteEvent) -> Result<(), String> {
        sqlx::query("INSERT INTO mock_server_event (id, account, calendar_external_id, external_id, body, modified_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(account, external_id) DO UPDATE SET body = excluded.body, modified_at = excluded.modified_at")
            .bind(Uuid::new_v4().to_string())
            .bind(&self.account)
            .bind(calendar_external_id)
            .bind(&event.external_id)
            .bind(serde_json::to_string(event).map_err(|e| e.to_string())?)
            .bind(stamp(Utc::now()))
            .execute(self.pool)
            .await
            .map(|_| ())
            .map_err(|e| format!("The mock server could not keep the event: {}", e))
    }

    pub async fn get_events(&self, calendar_external_id: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<RemoteEvent>, String> {
        self.refuse(false).await?;
        let rows: Vec<String> = sqlx::query_scalar("SELECT body FROM mock_server_event WHERE account = ?1 AND calendar_external_id = ?2").bind(&self.account).bind(calendar_external_id).fetch_all(self.pool).await.map_err(|e| e.to_string())?;
        let mut events: Vec<RemoteEvent> = rows.iter().filter_map(|b| serde_json::from_str::<RemoteEvent>(b).ok()).filter(|e| e.ends_at >= from && e.starts_at <= to).collect();
        events.sort_by_key(|e| e.starts_at);
        Ok(events)
    }

    pub async fn post_event(&self, calendar_external_id: &str, event: &NewRemoteEvent) -> Result<RemoteEvent, String> {
        self.refuse(true).await?;
        let calendar = self.mock.calendars().into_iter().find(|c| c.external_id == calendar_external_id).ok_or_else(|| "No such mock calendar".to_string())?;
        if !calendar.can_write {
            return Err(format!("{} is read only", calendar.name));
        }
        let id = Uuid::new_v4().to_string();
        let me = self.mock.me();
        let made = RemoteEvent {
            external_id: format!("mock-{}-new-{}", self.mock.kind().key(), id),
            ical_uid: Some(format!("{}@mock.chatnip", id)),
            title: event.title.clone(),
            notes: event.notes.clone(),
            location: event.location.clone(),
            starts_at: event.starts_at,
            ends_at: event.ends_at,
            all_day: event.all_day,
            attendees: event.attendees.clone(),
            organizer: me.email.clone(),
            response_status: Some("organizer".to_string()),
            online_meeting_url: event.online_meeting.then(|| self.mock.meeting_url(&id)),
            cancelled: false,
            updated_at: Utc::now(),
        };
        self.put_event(calendar_external_id, &made).await?;
        let seconds = AppSetting::get_u64(self.pool, app_setting::MOCK_REPLY_SECONDS).await?;
        if seconds > 0 && self.mock.kind().is_mail() {
            for attendee in made.attendees.iter().filter(|a| Some(a.as_str()) != me.email.as_deref()) {
                self.post_pending(Utc::now() + Duration::seconds(seconds as i64), json!({ "kind": "accepted", "event": made.external_id, "from": self.user(attendee) })).await?;
            }
        }
        Ok(made)
    }

    pub async fn put_response(&self, event_external_id: &str, response: &str) -> Result<RemoteEvent, String> {
        self.refuse(true).await?;
        if !RESPONSES.contains(&response) {
            return Err(format!("{} is not an answer to an invitation; use accepted, declined or tentativelyAccepted", response));
        }
        let (calendar, mut event) = self.get_event(event_external_id).await?.ok_or_else(|| format!("404 Not Found: the event {} does not exist on the server", event_external_id))?;
        let me = self.mock.me();
        if event.organizer.is_none() || event.organizer.as_deref() == me.email.as_deref() {
            return Err("You organise this event; there is no invitation to answer".to_string());
        }
        event.response_status = Some(response.to_string());
        event.updated_at = Utc::now();
        self.put_event(&calendar, &event).await?;
        let seconds = AppSetting::get_u64(self.pool, app_setting::MOCK_REPLY_SECONDS).await?;
        if seconds > 0 {
            let from = self.user(event.organizer.as_deref().unwrap_or_default());
            self.post_pending(Utc::now() + Duration::seconds(seconds as i64), json!({ "kind": "rsvp", "event": event.external_id, "response": response, "from": from })).await?;
        }
        Ok(event)
    }

    async fn get_setting(&self, key: &str) -> Result<Option<Value>, String> {
        let value: Option<String> = sqlx::query_scalar("SELECT value FROM mock_server_setting WHERE account = ?1 AND key = ?2").bind(&self.account).bind(key).fetch_optional(self.pool).await.map_err(|e| e.to_string())?;
        Ok(value.and_then(|v| serde_json::from_str(&v).ok()))
    }

    async fn put_setting(&self, key: &str, value: &Value) -> Result<(), String> {
        sqlx::query("INSERT INTO mock_server_setting (account, key, value) VALUES (?1, ?2, ?3) ON CONFLICT(account, key) DO UPDATE SET value = excluded.value")
            .bind(&self.account)
            .bind(key)
            .bind(value.to_string())
            .execute(self.pool)
            .await
            .map(|_| ())
            .map_err(|e| format!("The mock server could not keep {}: {}", key, e))
    }

    pub async fn put_topic(&self, conversation_external_id: &str, topic: Option<&str>, purpose: Option<&str>) -> Result<bool, String> {
        self.refuse(true).await?;
        let mut conversation = self.get_conversation(conversation_external_id).await?.ok_or_else(|| format!("404 Not Found: the chat {} does not exist on the server", conversation_external_id))?;
        for (key, value) in [("topic", topic), ("purpose", purpose)] {
            if let Some(value) = value {
                conversation.metadata.insert(key.to_string(), json!(value.trim()));
            }
        }
        self.put_conversation(&conversation, Utc::now()).await?;
        if let Some(text) = topic.or(purpose) {
            self.post_system(&conversation, SystemEvent::new(SystemEventKind::TopicChanged, vec![SystemPerson::from(&self.mock.me())], Vec::new(), Some(text.trim().to_string()))).await?;
        }
        Ok(true)
    }

    pub async fn put_message_flag(&self, external_id: &str, key: &str, on: bool) -> Result<bool, String> {
        self.refuse(true).await?;
        let Some(mut message) = self.get_message(external_id).await? else { return Ok(false) };
        message.metadata.insert(key.to_string(), json!(on));
        self.put_message(&message, Utc::now()).await?;
        Ok(true)
    }

    pub async fn post_conversation(&self, conversation: &crate::providers::traits::NewRemoteConversation) -> Result<Option<ResponseConversation>, String> {
        self.refuse(true).await?;
        let now = Utc::now();
        let mut members = vec![self.mock.me()];
        members.extend(conversation.members.iter().map(|id| self.user(id)));
        let created = ResponseConversation {
            external_id: format!("mock-{}-new-{}", self.mock.kind().key(), Uuid::new_v4().simple()),
            name: conversation.name.trim().to_string(),
            conversation_type: conversation.conversation_type.clone(),
            members,
            last_activity: now,
            metadata: Default::default(),
        };
        self.put_conversation(&created, now).await?;
        self.post_system(&created, SystemEvent::new(SystemEventKind::ChatCreated, vec![SystemPerson::from(&self.mock.me())], Vec::new(), Some(created.name.clone()))).await?;
        Ok(Some(created))
    }

    pub async fn get_joinable(&self) -> Result<Vec<ResponseConversation>, String> {
        self.refuse(false).await?;
        let joined = self.get_setting("joined").await?.and_then(|v| serde_json::from_value::<Vec<String>>(v).ok()).unwrap_or_default();
        Ok(["design", "random", "releases"]
            .iter()
            .map(|name| format!("mock-{}-open-{}", self.mock.kind().key(), name))
            .filter(|id| !joined.contains(id))
            .map(|id| {
                let name = id.rsplit('-').next().unwrap_or_default().to_string();
                let mut metadata = std::collections::HashMap::new();
                metadata.insert("purpose".to_string(), json!(format!("Everything about {}", name)));
                metadata.insert("member_count".to_string(), json!(12));
                ResponseConversation { external_id: id, name, conversation_type: ConversationType::Channel, members: Vec::new(), last_activity: Utc::now(), metadata }
            })
            .collect())
    }

    pub async fn put_membership(&self, conversation_external_id: &str, joined: bool) -> Result<bool, String> {
        self.refuse(true).await?;
        let now = Utc::now();
        let mut list = self.get_setting("joined").await?.and_then(|v| serde_json::from_value::<Vec<String>>(v).ok()).unwrap_or_default();
        if joined {
            let opened = self.get_joinable().await?.into_iter().find(|c| c.external_id == conversation_external_id);
            let mut conversation = match opened {
                Some(found) => found,
                None => self.get_conversation(conversation_external_id).await?.ok_or_else(|| format!("404 Not Found: the channel {} does not exist on the server", conversation_external_id))?,
            };
            if !conversation.members.iter().any(|m| m.id == self.mock.me().id) {
                conversation.members.push(self.mock.me());
            }
            self.put_conversation(&conversation, now).await?;
            if !list.contains(&conversation_external_id.to_string()) {
                list.push(conversation_external_id.to_string());
            }
            self.post_system(&conversation, SystemEvent::new(SystemEventKind::MemberJoined, vec![SystemPerson::from(&self.mock.me())], Vec::new(), None)).await?;
        } else {
            let Some(conversation) = self.get_conversation(conversation_external_id).await? else { return Ok(false) };
            self.post_system(&conversation, SystemEvent::new(SystemEventKind::MemberLeft, vec![SystemPerson::from(&self.mock.me())], Vec::new(), None)).await?;
            sqlx::query("DELETE FROM mock_server_message WHERE account = ?1 AND conversation_external_id = ?2").bind(&self.account).bind(conversation_external_id).execute(self.pool).await.map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM mock_server_conversation WHERE account = ?1 AND external_id = ?2").bind(&self.account).bind(conversation_external_id).execute(self.pool).await.map_err(|e| e.to_string())?;
            list.retain(|c| c != conversation_external_id);
        }
        self.put_setting("joined", &json!(list)).await?;
        Ok(true)
    }

    pub async fn put_state(&self, conversation_external_id: &str, archived: bool) -> Result<bool, String> {
        self.refuse(true).await?;
        let Some(mut conversation) = self.get_conversation(conversation_external_id).await? else { return Ok(false) };
        conversation.metadata.insert("archived".to_string(), json!(archived));
        self.put_conversation(&conversation, Utc::now()).await?;
        Ok(true)
    }

    pub async fn get_search(&self, search: &crate::providers::traits::RemoteSearch) -> Result<Vec<ResponseMessage>, String> {
        self.refuse(false).await?;
        let rows: Vec<String> = sqlx::query_scalar("SELECT body FROM mock_server_message WHERE account = ?1 AND (?2 IS NULL OR conversation_external_id = ?2) AND body LIKE ?3 ORDER BY timestamp DESC LIMIT ?4")
            .bind(&self.account)
            .bind(&search.conversation_external_id)
            .bind(format!("%{}%", search.text.trim()))
            .bind(search.limit.clamp(1, 200) as i64)
            .fetch_all(self.pool)
            .await
            .map_err(|e| format!("The mock server could not search: {}", e))?;
        Ok(rows
            .iter()
            .filter_map(|b| serde_json::from_str::<ResponseMessage>(b).ok())
            .filter(|m| search.from.as_deref().is_none_or(|f| m.sender.id == f || m.sender.email.as_deref() == Some(f)))
            .filter(|m| search.after.is_none_or(|a| m.timestamp >= a) && search.before.is_none_or(|b| m.timestamp <= b))
            .collect())
    }

    pub async fn put_draft(&self, draft: &OutgoingMessage, existing: Option<&str>) -> Result<Option<String>, String> {
        self.refuse(true).await?;
        let id = existing.map(String::from).unwrap_or_else(|| format!("mock-{}-draft-{}", self.mock.kind().key(), Uuid::new_v4().simple()));
        self.put_setting(&format!("draft:{}", id), &json!({ "subject": draft.subject, "to": draft.to, "content": draft.content })).await?;
        Ok(Some(id))
    }

    pub async fn delete_draft(&self, id: &str) -> Result<bool, String> {
        sqlx::query("DELETE FROM mock_server_setting WHERE account = ?1 AND key = ?2").bind(&self.account).bind(format!("draft:{}", id)).execute(self.pool).await.map_err(|e| e.to_string())?;
        Ok(true)
    }

    pub async fn get_aliases(&self) -> Result<Vec<crate::providers::traits::RemoteAlias>, String> {
        let me = self.mock.me();
        let email = me.email.clone().unwrap_or_default();
        let signature = self.get_setting("signature").await?.and_then(|v| v.as_str().map(String::from));
        let mut out = vec![crate::providers::traits::RemoteAlias { email: email.clone(), name: me.display_name.clone(), is_default: true, signature }];
        out.push(crate::providers::traits::RemoteAlias { email: email.replace("milo@", "post@"), name: Some("Northwind".to_string()), is_default: false, signature: None });
        Ok(out)
    }

    pub async fn put_signature(&self, html: &str) -> Result<bool, String> {
        self.put_setting("signature", &json!(html)).await?;
        Ok(true)
    }

    pub async fn get_auto_reply(&self) -> Result<Option<crate::providers::traits::RemoteAutoReply>, String> {
        Ok(Some(self.get_setting("auto_reply").await?.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()))
    }

    pub async fn put_auto_reply(&self, reply: &crate::providers::traits::RemoteAutoReply) -> Result<bool, String> {
        self.put_setting("auto_reply", &serde_json::to_value(reply).map_err(|e| e.to_string())?).await?;
        Ok(true)
    }

    pub async fn put_pending_due(&self, id: &str, at: DateTime<Utc>) -> Result<(), String> {
        sqlx::query("UPDATE mock_server_pending SET due_at = ?1 WHERE id = ?2 AND account = ?3").bind(stamp(at)).bind(id).bind(&self.account).execute(self.pool).await.map(|_| ()).map_err(|e| e.to_string())
    }

    pub async fn get_typing(&self, before: DateTime<Utc>) -> Result<Vec<(String, String, ResponseUser)>, String> {
        let rows = sqlx::query("SELECT id, body FROM mock_server_pending WHERE account = ?1 AND due_at <= ?2").bind(&self.account).bind(stamp(before)).fetch_all(self.pool).await.map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            let body: Value = serde_json::from_str(&row.get::<String, _>("body")).unwrap_or(Value::Null);
            if body["kind"].as_str() != Some("reply") {
                continue;
            }
            let Ok(from) = serde_json::from_value::<ResponseUser>(body["from"].clone()) else { continue };
            let Some(mine) = self.get_message(body["message"].as_str().unwrap_or_default()).await? else { continue };
            out.push((row.get::<String, _>("id"), mine.conversation_external_id, from));
        }
        Ok(out)
    }

    pub async fn deliver(&self, now: DateTime<Utc>) -> Result<usize, String> {
        if self.state().await? != STATE_OK {
            return Ok(0);
        }
        let rows = sqlx::query("SELECT id, body FROM mock_server_pending WHERE account = ?1 AND due_at <= ?2 ORDER BY due_at").bind(&self.account).bind(stamp(now)).fetch_all(self.pool).await.map_err(|e| e.to_string())?;
        let mut delivered = 0;
        for row in rows {
            let id: String = row.get("id");
            let claimed = sqlx::query("DELETE FROM mock_server_pending WHERE id = ?1").bind(&id).execute(self.pool).await.map_err(|e| e.to_string())?.rows_affected();
            if claimed == 0 {
                continue;
            }
            let body: Value = serde_json::from_str(&row.get::<String, _>("body")).unwrap_or(Value::Null);
            if self.answer(&body, now).await? {
                delivered += 1;
            }
        }
        let minutes = AppSetting::get_u64(self.pool, app_setting::MOCK_NEWSLETTER_MINUTES).await?;
        if minutes > 0 {
            let last: Option<String> = sqlx::query_scalar("SELECT newsletter_at FROM mock_server_account WHERE id = ?1").bind(&self.account).fetch_optional(self.pool).await.map_err(|e| e.to_string())?.flatten();
            let ready = last.as_deref().and_then(read).is_none_or(|at| now - at >= Duration::minutes(minutes as i64));
            if ready {
                let claimed = sqlx::query("UPDATE mock_server_account SET newsletter_at = ?1 WHERE id = ?2 AND COALESCE(newsletter_at, '') = COALESCE(?3, '')").bind(stamp(now)).bind(&self.account).bind(&last).execute(self.pool).await.map_err(|e| e.to_string())?.rows_affected();
                if claimed > 0 {
                    self.newsletter(now).await?;
                    delivered += 1;
                }
            }
        }
        Ok(delivered)
    }

    fn mail_message(&self, external: &str, conversation: &str, from: &ResponseUser, subject: &str, html: &str, at: DateTime<Utc>, reply: Option<&ResponseMessage>) -> ResponseMessage {
        let me = self.mock.me();
        let references = reply.and_then(|r| r.metadata.get("internet_message_id").and_then(|v| v.as_str()).map(String::from));
        let headers = self.mail_headers(external, from, std::slice::from_ref(&me), &[], subject, at, references.map(|r| (r.clone(), r)));
        let mut message = ResponseMessage {
            external_id: external.to_string(),
            conversation_external_id: conversation.to_string(),
            sender: from.clone(),
            content: html.to_string(),
            content_type: "html".to_string(),
            timestamp: at,
            r#type: MessageType::Email,
            subject: Some(subject.to_string()),
            recipients: vec![me],
            headers: Some(headers),
            is_read: false,
            reply_to_id: reply.map(|r| r.external_id.clone()),
            ..Default::default()
        };
        message.metadata.insert("internet_message_id".to_string(), json!(format!("<{}@mock.chatnip>", external)));
        crate::content::mail::analyse(Some(html), None, message.headers.as_deref(), None).apply(&mut message);
        message
    }

    async fn new_mail(&self, slug: &str, from: &ResponseUser, subject: &str, html: &str, at: DateTime<Utc>) -> Result<(), String> {
        let external = format!("mock-{}-mail-{}", self.mock.kind().key(), slug);
        let conversation = ResponseConversation { external_id: external.clone(), name: subject.to_string(), conversation_type: ConversationType::Email, members: vec![self.mock.me(), from.clone()], last_activity: at, metadata: Default::default() };
        self.put_conversation(&conversation, at).await?;
        let message = self.mail_message(&format!("{}-m0", external), &external, from, subject, html, at, None);
        self.put_message(&message, at).await
    }

    async fn answer(&self, body: &Value, now: DateTime<Utc>) -> Result<bool, String> {
        let Ok(from) = serde_json::from_value::<ResponseUser>(body["from"].clone()) else { return Ok(false) };
        let first = from.first_name.clone().or_else(|| from.display_name.clone()).unwrap_or_default();
        match body["kind"].as_str().unwrap_or_default() {
            "reply" => {
                let Some(mut mine) = self.get_message(body["message"].as_str().unwrap_or_default()).await? else { return Ok(false) };
                let id = self.server_id(now);
                let reply = if self.mock.kind().is_mail() {
                    let line = MAIL_REPLIES[spread(&mine.external_id) % MAIL_REPLIES.len()];
                    let subject = crate::providers::traits::prefixed_subject("Re", mine.subject.as_deref().unwrap_or_default());
                    let quoted = format!("<div class=\"gmail_quote\"><div dir=\"ltr\" class=\"gmail_attr\">{} wrote:</div><blockquote class=\"gmail_quote\" style=\"margin:0 0 0 .8ex;border-left:1px #ccc solid;padding-left:1ex\">{}</blockquote></div>", mine.timestamp.format("%a, %-d %b %Y at %H:%M"), mine.content);
                    let html = format!("<p>Hei Milo,</p><p>{}</p><p>Mvh<br>{}</p>{}", line, first, quoted);
                    self.mail_message(&id, &mine.conversation_external_id, &from, &subject, &html, now, Some(&mine))
                } else {
                    let line = CHAT_REPLIES[spread(&mine.external_id) % CHAT_REPLIES.len()];
                    let mut reply = ResponseMessage { external_id: id, conversation_external_id: mine.conversation_external_id.clone(), sender: from.clone(), content: line.to_string(), content_type: "text".to_string(), timestamp: now, r#type: MessageType::Chat, is_read: false, thread_id: mine.thread_id.clone(), ..Default::default() };
                    crate::content::chat::markdown(line).apply(&mut reply);
                    reply
                };
                self.put_message(&reply, now).await?;
                mine.metadata.insert("delivery".to_string(), json!("read"));
                self.put_message(&mine, now).await?;
                Ok(true)
            }
            "accepted" => {
                let Some((_, event)) = self.get_event(body["event"].as_str().unwrap_or_default()).await? else { return Ok(false) };
                let name = from.display_name.clone().unwrap_or_else(|| from.email.clone().unwrap_or_default());
                self.new_mail(&format!("accepted-{}", Uuid::new_v4().simple()), &from, &format!("Accepted: {}", event.title), &format!("<p>{} has accepted this meeting.</p><p><b>{}</b><br>{}</p>", name, event.title, event.starts_at.format("%A %-d %B %Y %H:%M")), now).await?;
                Ok(true)
            }
            "rsvp" => {
                let Some((_, event)) = self.get_event(body["event"].as_str().unwrap_or_default()).await? else { return Ok(false) };
                let line = match body["response"].as_str().unwrap_or_default() {
                    "accepted" => "Så bra at du kommer, vi sees!",
                    "declined" => "Synd du ikke kan. Jeg sender referatet etterpå.",
                    _ => "Takk for svaret. Si fra når du vet sikkert.",
                };
                if self.mock.kind().is_mail() {
                    self.new_mail(&format!("rsvp-{}", Uuid::new_v4().simple()), &from, &format!("Re: {}", event.title), &format!("<p>Hei Milo,</p><p>{}</p><p>{}</p>", line, first), now).await?;
                    return Ok(true);
                }
                let rows: Vec<String> = sqlx::query_scalar("SELECT body FROM mock_server_conversation WHERE account = ?1").bind(&self.account).fetch_all(self.pool).await.map_err(|e| e.to_string())?;
                let direct = rows.iter().filter_map(|b| serde_json::from_str::<ResponseConversation>(b).ok()).find(|c| c.conversation_type == ConversationType::DirectMessage && c.members.iter().any(|m| m.email == from.email));
                let Some(direct) = direct else { return Ok(false) };
                let text = format!("{} ({})", line, event.title);
                let mut reply = ResponseMessage { external_id: self.server_id(now), conversation_external_id: direct.external_id.clone(), sender: from.clone(), content: text.clone(), content_type: "text".to_string(), timestamp: now, r#type: MessageType::Chat, is_read: false, ..Default::default() };
                crate::content::chat::markdown(&text).apply(&mut reply);
                self.put_message(&reply, now).await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn newsletter(&self, now: DateTime<Utc>) -> Result<(), String> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mock_server_message WHERE account = ?1 AND external_id LIKE '%-news-%'").bind(&self.account).fetch_one(self.pool).await.map_err(|e| e.to_string())?;
        let n = count as usize;
        if self.mock.kind().is_mail() {
            let (subject, body) = DIGESTS[n % DIGESTS.len()];
            let html = format!("<div style=\"font-family:Arial,sans-serif\">{}<p><a href=\"https://tldr.tech/unsubscribe\">Unsubscribe</a></p></div>", body);
            let from = ResponseUser { id: format!("mock-{}-s0", self.mock.kind().key()), email: Some("newsletter@tldrnewsletter.com".to_string()), display_name: Some("TLDR Newsletter".to_string()), ..Default::default() };
            return self.new_mail(&format!("news-{}", now.timestamp()), &from, subject, &html, now).await;
        }
        let channel = format!("mock-{}-channel-1", self.mock.kind().key());
        if self.get_conversation(&channel).await?.is_none() {
            return Ok(());
        }
        let bot = ResponseUser { id: format!("mock-{}-bot-github", self.mock.kind().key()), display_name: Some("GitHub".to_string()), email: Some("notifications@github.com".to_string()), ..Default::default() };
        let line = BOT_LINES[n % BOT_LINES.len()];
        let mut message = ResponseMessage { external_id: format!("{}-news-{}", channel, now.timestamp()), conversation_external_id: channel, sender: bot, content: line.to_string(), content_type: "text".to_string(), timestamp: now, r#type: MessageType::Bot, is_read: false, ..Default::default() };
        crate::content::chat::markdown(line).apply(&mut message);
        self.put_message(&message, now).await
    }
}
