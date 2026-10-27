use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::providers::models::{Delivery, Provider, ProviderAuth, ProviderCapabilities};
use super::server::{self, Server};
use crate::providers::traits::{
    ConnectionIdentity, ConversationType, MessageOrigin, MessageType, OutgoingMessage, ProviderTrait,
    ResponseAttachment, ResponseConversation, ResponseMessage, ResponseUser, RemoteCalendar, RemoteEvent, NewRemoteEvent, SystemEvent, SystemEventKind, SystemPerson,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MockKind {
    Teams,
    Outlook,
    Slack,
    Gmail,
    GoogleChat,
    Imap,
}

impl MockKind {
    pub const ALL: [MockKind; 6] = [MockKind::Teams, MockKind::Outlook, MockKind::Slack, MockKind::Gmail, MockKind::GoogleChat, MockKind::Imap];

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_lowercase().as_str() {
            "teams" => Ok(Self::Teams),
            "outlook" => Ok(Self::Outlook),
            "slack" => Ok(Self::Slack),
            "gmail" => Ok(Self::Gmail),
            "google_chat" => Ok(Self::GoogleChat),
            "imap" => Ok(Self::Imap),
            other => Err(format!("Unknown mock client: {}", other)),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Teams => "teams",
            Self::Outlook => "outlook",
            Self::Slack => "slack",
            Self::Gmail => "gmail",
            Self::GoogleChat => "google_chat",
            Self::Imap => "imap",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Teams => "Mock Teams",
            Self::Outlook => "Mock Outlook",
            Self::Slack => "Mock Slack",
            Self::Gmail => "Mock Gmail",
            Self::GoogleChat => "Mock Google Chat",
            Self::Imap => "Mock Mail",
        }
    }

    pub fn is_mail(self) -> bool {
        matches!(self, Self::Outlook | Self::Gmail | Self::Imap)
    }

    fn mail_slice(self) -> std::ops::Range<usize> {
        match self {
            Self::Outlook => 0..3,
            Self::Gmail => 3..6,
            _ => 6..9,
        }
    }

    fn seed(self) -> u64 {
        match self {
            Self::Teams => 11,
            Self::Outlook => 23,
            Self::Slack => 37,
            Self::Gmail => 41,
            Self::GoogleChat => 53,
            Self::Imap => 67,
        }
    }

    fn supports_reactions(self) -> bool {
        !self.is_mail()
    }

    pub fn reply_quote(self) -> Delivery {
        match self {
            Self::Teams | Self::Slack => Delivery::Native,
            Self::GoogleChat => Delivery::Repost,
            _ => Delivery::Mail,
        }
    }

    pub fn forward(self) -> Delivery {
        match self {
            Self::Slack => Delivery::Native,
            Self::Teams | Self::GoogleChat => Delivery::Repost,
            _ => Delivery::Mail,
        }
    }

    pub fn thread_broadcast(self) -> Delivery {
        match self {
            Self::Slack => Delivery::Native,
            Self::Teams | Self::GoogleChat => Delivery::Repost,
            _ => Delivery::Unsupported,
        }
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

const CAST: [(&str, &str, &str, &str); 12] = [
    ("eva.ortega@northwind.example", "Eva", "Ortega", "Designer"),
    ("nora.gale@northwind.example", "Nora", "Gale", "CTO"),
    ("lea.lindqvist@northwind.example", "Lea", "Lindqvist", "Project lead"),
    ("sam.hale@northwind.example", "Sam", "Hale", "Developer"),
    ("mike.vance@northwind.example", "Mike", "Vance", "Developer"),
    ("oscar.strand@sundial.example", "Oscar", "Strand", "Founder"),
    ("hugo.brandt@sundial.example", "Hugo", "Brandt", "Advisor"),
    ("anna.berg@client.example", "Anna", "Berg", "Procurement"),
    ("jonas.lie@client.example", "Jonas", "Lie", "IT manager"),
    ("kari.nordmann@tidewater.example", "Kari", "Nordmann", "Platform engineer"),
    ("ola.hansen@tidewater.example", "Ola", "Hansen", "Security"),
    ("mia.sund@freelance.example", "Mia", "Sund", "Illustrator"),
];

const SERVICES: [(&str, &str); 3] = [
    ("newsletter@tldrnewsletter.com", "TLDR Newsletter"),
    ("notifications@github.com", "GitHub"),
    ("calendar-notification@google.com", "Google Calendar"),
];

const CHAT_LINES: [&str; 24] = [
    "Hei! Har du tid til en kjapp prat om designet før lunsj?",
    "Ja, jeg er ledig fra 11. Teams eller her?",
    "Her er fint. Jeg sender over skissene nå.",
    "Ser bra ut, men marginene på høyre side ser litt tette ut.",
    "Enig, jeg justerer til 16px og sender ny versjon.",
    "Can you review the PR when you get a minute? It touches the sync loop.",
    "On it. Running the tests locally first.",
    "Tests pass here. One nit about naming, otherwise good to go.",
    "Merged. Thanks for the quick turnaround!",
    "Vi må bestemme oss for leverandør innen fredag.",
    "Jeg heller mot alternativ B, billigere og enklere å drifte.",
    "Fint, jeg lager et kort notat til styret.",
    "Lunch & Learn på torsdag, kommer du?",
    "Kommer! Skal jeg ta med noe?",
    "Nei, det er dekket. Bare møt opp 11:30.",
    "Deployet til staging nå, kan dere teste innlogging?",
    "Funker for meg. Rask som bare det.",
    "Reminder: standup moves to 09:15 tomorrow.",
    "Noted, see you then.",
    "Fakturaen for september er sendt, si fra om noe mangler.",
    "Mottatt, alt ser riktig ut. Takk!",
    "Kan du dele dokumentet med Anna også?",
    "Done, she has edit access now.",
    "Perfekt, da er vi i mål for denne uka 🎉",
];

const GROUPS: [(&str, [usize; 3]); 4] = [
    ("Start of Week Sync", [0, 1, 2]),
    ("Sundial Design", [5, 6, 11]),
    ("Kunde: Tidewater platform", [9, 10, 8]),
    ("C-level", [1, 5, 2]),
];

const CHANNELS: [(&str, [usize; 4]); 3] = [
    ("#general", [0, 1, 2, 3]),
    ("#dev", [3, 4, 9, 1]),
    ("#design", [0, 11, 5, 2]),
];

const HUMAN_MAIL: [(&str, usize, &str); 9] = [
    ("Tilbud på plattformdrift", 7, "<p>Hei Milo,</p><p>Vedlagt ligger tilbudet vi snakket om. Gi beskjed om noe er uklart, så tar vi et møte neste uke.</p><p>Mvh<br>Anna</p>"),
    ("Møte om Tidewater-plattformen torsdag", 9, "<p>Hei,</p><p>Passer torsdag 13:00 for en gjennomgang av clusteret? Jeg booker rommet.</p><p>Kari</p>"),
    ("Styremøte: agenda og tall", 5, "<p>Hei Milo,</p><p>Agenda for styremøtet ligger vedlagt. Si fra om du vil ha med et punkt om leverandørvalget.</p><p>Oscar</p>"),
    ("Re: Illustrasjoner til forsiden", 11, "<p>Hei!</p><p>Her er de tre første utkastene. Jeg kan levere resten innen onsdag hvis retningen stemmer.</p><p>Mia</p>"),
    ("Oppfølging: tilgang til Tidewater cluster", 9, "<p>Hi,</p><p>Your access request is approved. The kubeconfig is attached and expires in 30 days.</p><p>Kari</p>"),
    ("Follow-up from our call", 8, "<p>Hi Milo,</p><p>Thanks for the call today. I will send the revised numbers once procurement has signed off.</p><p>Jonas</p>"),
    ("Faktura september", 6, "<p>Hei,</p><p>Vedlagt faktura for rådgivning i september. Forfall 14 dager.</p><p>Hugo</p>"),
    ("Spørsmål om kontrakten", 7, "<p>Hei Milo,</p><p>Ett spørsmål om punkt 4.2 i kontrakten før vi signerer. Kan du ringe når det passer?</p><p>Anna</p>"),
    ("Bilder fra workshopen", 11, "<p>Hei!</p><p>Her er bildene fra workshopen i forrige uke, fritt frem for bruk på nettsiden.</p><p>Mia</p>"),
];

const SERVICE_MAIL: [(&str, usize, &str); 9] = [
    ("Invitation: Lunch & Learn @ Thu 11:30 (CEST)", 2, "<p>You have been invited to <b>Lunch & Learn: OpenShift</b> on Thursday at 11:30.</p><p>Where: Kantina, 3. etasje</p>"),
    ("Updated invitation: Styremøte @ Mon 09:00 (CEST)", 2, "<p>The event <b>Styremøte</b> has been moved to Monday at 09:00.</p>"),
    ("[SlyOtis/chatnip] Release v0.4.0 published", 1, "<p><b>milo</b> published a release.</p><p>Superhuman inbox, people merge and the mock clients.</p></p>"),
    ("TLDR: Tauri 2.5, WebKit tracing, and more", 0, "<h2>Tech</h2><p>Tauri 2.5 ships with better tracing support (4 minute read).</p><p>WebKit adds stale style fixes for transitions (2 minute read).</p>"),
    ("[SlyOtis/chatnip] Fix sync loop (PR #42)", 1, "<p><b>sam</b> opened a pull request.</p><p>Fixes the double refresh on the first sync. Reviewers: @milo</p>"),
    ("TLDR: Rust 1.92, SQLite 3.50, Svelte 6 preview", 0, "<h2>Dev</h2><p>Rust 1.92 lands async closures (3 minute read).</p><p>Svelte 6 preview drops the compiler flags (2 minute read).</p>"),
    ("Reminder: Lunch & Learn starts in 1 hour", 2, "<p>Your event <b>Lunch & Learn: OpenShift</b> starts at 11:30.</p>"),
    ("[SlyOtis/chatnip] 3 new issues assigned to you", 1, "<p>Issues #51, #52 and #53 were assigned to <b>milo</b>.</p>"),
    ("TLDR Weekly: the best of this week", 0, "<h2>Weekly</h2><p>The ten most read stories from the week (8 minute read).</p>"),
];

const CITIES: [(&str, &str, f64, f64, &str); 12] = [
    ("Oslo", "Norway", 59.9139, 10.7522, "185.12.34.56"),
    ("Bergen", "Norway", 60.3913, 5.3221, "185.12.35.17"),
    ("Trondheim", "Norway", 63.4305, 10.3951, "185.12.36.88"),
    ("Stockholm", "Sweden", 59.3293, 18.0686, "193.10.14.22"),
    ("Copenhagen", "Denmark", 55.6761, 12.5683, "130.225.11.9"),
    ("London", "United Kingdom", 51.5074, -0.1278, "151.101.1.69"),
    ("Berlin", "Germany", 52.5200, 13.4050, "88.198.16.40"),
    ("Amsterdam", "Netherlands", 52.3676, 4.9041, "145.100.7.3"),
    ("New York", "United States", 40.7128, -74.0060, "104.16.132.229"),
    ("San Francisco", "United States", 37.7749, -122.4194, "64.233.160.0"),
    ("Singapore", "Singapore", 1.3521, 103.8198, "203.117.136.10"),
    ("Sydney", "Australia", -33.8688, 151.2093, "203.2.218.17"),
];

const PLAIN_EML: &str = include_str!("../../../tests/fixtures/plain.eml");
const GMAIL_EML: &str = include_str!("../../../tests/fixtures/gmail-quote.eml");
const OUTLOOK_EML: &str = include_str!("../../../tests/fixtures/outlook-quote.eml");
const APPLE_EML: &str = include_str!("../../../tests/fixtures/apple-quote.eml");
const NEWSLETTER_EML: &str = include_str!("../../../tests/fixtures/newsletter.eml");
const INVITE_EML: &str = include_str!("../../../tests/fixtures/invite.eml");
const ATTACHMENTS_EML: &str = include_str!("../../../tests/fixtures/attachments.eml");
const TEAMS_JSON: &str = include_str!("../../../tests/fixtures/teams-message.json");
const SLACK_JSON: &str = include_str!("../../../tests/fixtures/slack-messages.json");
const GCHAT_JSON: &str = include_str!("../../../tests/fixtures/gchat-message.json");
const MOCK_TLDR: &str = include_str!("../../../tests/fixtures/mock/tldr.eml");
const MOCK_RETAILER: &str = include_str!("../../../tests/fixtures/mock/retailer.eml");
const MOCK_RECEIPT: &str = include_str!("../../../tests/fixtures/mock/receipt.eml");
const MOCK_GITHUB: &str = include_str!("../../../tests/fixtures/mock/github.eml");
const MOCK_LINEAR: &str = include_str!("../../../tests/fixtures/mock/linear.eml");
const MOCK_INVITE: &str = include_str!("../../../tests/fixtures/mock/invite.eml");
const MOCK_TRACKING: &str = include_str!("../../../tests/fixtures/mock/tracking.eml");
const MOCK_DARK: &str = include_str!("../../../tests/fixtures/mock/dark-hostile.eml");
const MOCK_APPLE: &str = include_str!("../../../tests/fixtures/mock/apple-reply.eml");
const MOCK_INLINE: &str = include_str!("../../../tests/fixtures/mock/inline.eml");
const MOCK_PLAIN: &str = include_str!("../../../tests/fixtures/mock/plain-wrapped.eml");
const MOCK_LONGFORM: &str = include_str!("../../../tests/fixtures/mock/longform.eml");
const OUTLOOK_REPLY_HTML: &str = include_str!("../../../tests/fixtures/mock/outlook-reply.html");
const OUTLOOK_QUOTE_HTML: &str = include_str!("../../../tests/fixtures/mock/outlook-quote.html");
const GMAIL_REPLY_HTML: &str = include_str!("../../../tests/fixtures/mock/gmail-reply.html");
const GMAIL_QUOTE_HTML: &str = include_str!("../../../tests/fixtures/mock/gmail-quote.html");
const TEAMS_RICH: &str = include_str!("../../../tests/fixtures/mock/teams.json");
const SLACK_RICH: &str = include_str!("../../../tests/fixtures/mock/slack.json");
const CHART_PNG: &[u8] = include_bytes!("../../../tests/fixtures/mock/chart.png");
const LINKS_SAMPLE: &str = "Tre lenker til helgen: https://github.com/tauri-apps/tauri https://www.youtube.com/watch?v=jfKfPfyJRdk https://svelte.dev/blog/runes";

const OUTLOOK_THREAD: [(Option<usize>, &str); 8] = [
    (Some(7), "<p class=\"MsoNormal\">Hei Milo,</p><p class=\"MsoNormal\">Som avtalt sender jeg utkastet til avtalen om plattformdrift for 2027. Vi ønsker å gå fra timepris til fast månedspris med en ramme for endringer.</p>"),
    (None, "<p class=\"MsoNormal\">Hei Anna,</p><p class=\"MsoNormal\">Takk! Fast månedspris passer oss godt. Hvor stor ramme for endringer ser dere for dere?</p>"),
    (Some(8), "<p class=\"MsoNormal\">Hei begge,</p><p class=\"MsoNormal\">Jeg kobler meg på fra IT. Vi tenker 40 timer i måneden, med mulighet for å overføre ubrukte timer ett kvartal.</p>"),
    (None, "<p class=\"MsoNormal\">Hei Jonas,</p><p class=\"MsoNormal\">40 timer går fint. Overføring ett kvartal er greit, men ikke lenger enn det.</p>"),
    (Some(7), "<p class=\"MsoNormal\">Enig. Da oppdaterer jeg punkt 3.2 og sender til juristene.</p>"),
    (Some(8), "<p class=\"MsoNormal\">En ting til: vi trenger beredskap i påsken. Kan det legges inn som et tillegg?</p>"),
    (None, "<p class=\"MsoNormal\">Ja, beredskap i påsken legges inn som tillegg med fast pris. Jeg sender tallene i morgen.</p>"),
    (Some(7), "<p class=\"MsoNormal\">Perfekt, takk begge. Signert versjon kommer så snart juristene er ferdige.</p>"),
];

const GMAIL_THREAD: [(Option<usize>, &str); 4] = [
    (Some(11), "Hei Milo,<div><br></div><div>Her er skissene til onboarding-illustrasjonene. Tre varianter av velkomstskjermen, alle i samme strek som ikonene.</div>"),
    (None, "Hei Mia,<div><br></div><div>Variant to er favoritten. Kan du prøve den med varmere farger? Eva, hva tenker du?</div>"),
    (Some(0), "Enig i variant to. Jeg leverer fargepaletten i dag, så Mia har den å jobbe ut fra."),
    (Some(11), "Supert, da kjører jeg på med variant to og Evas palett. Ferdig fredag."),
];

fn chart_uri() -> String {
    use base64::Engine;
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(CHART_PNG))
}

fn slack_person(id: &str) -> usize {
    match id {
        "U0MIKE" => 4,
        "U0SAM" => 3,
        "U0HUGO" => 6,
        "U0ANNA" => 7,
        "U0JONAS" => 8,
        _ => 0,
    }
}

const MARKDOWN_SAMPLE: &str = "**Agenda** for _fredag_, ~torsdag~ :tada:\n> Husk å oppdatere `Cargo.lock` først\n- gå gjennom PR #42\n- bestem leverandør\n1. demo\n2. retro\n```rust\nfn main() {\n    println!(\"hei\");\n}\n```\nNotater: https://northwind.example/agenda @Eva";

fn mail_origin(index: usize, at: DateTime<Utc>, sender: &str) -> (String, MessageOrigin) {
    let (city, country, lat, lon, ip) = CITIES[index % CITIES.len()];
    let stamp = at.format("%a, %d %b %Y %H:%M:%S +0000");
    let domain = sender.split('@').nth(1).unwrap_or("example.org");
    let headers = format!(
        "Received: from mx.chatnip.local (mx.chatnip.local [185.12.34.1]) by inbox.chatnip.local with ESMTPS; {stamp}\r\nReceived: from smtp.{domain} (smtp.{domain} [{ip}]) by mx.chatnip.local with ESMTPS; {stamp}\r\nReceived: from [192.168.10.24] by smtp.{domain} with ESMTPA; {stamp}\r\nFrom: {sender}\r\nX-Mailer: Chatnip mock"
    );
    (
        headers,
        MessageOrigin { ip: Some(ip.to_string()), lat: Some(lat), lon: Some(lon), city: Some(city.to_string()), country: Some(country.to_string()) },
    )
}

fn local_file(name: &str, mime: &str) -> (String, String, u64) {
    use base64::Engine;
    let (mime, bytes) = if mime.starts_with("image/gif") {
        (mime.to_string(), crate::providers::mock::seed::gif(48, 48, &[[224, 97, 47], [31, 127, 174], [106, 168, 58]]))
    } else if mime.starts_with("image/") {
        ("image/png".to_string(), CHART_PNG.to_vec())
    } else if mime == "application/pdf" {
        (mime.to_string(), crate::providers::mock::seed::pdf(&[name, "Mock file served by the Chatnip mock server"]))
    } else {
        (mime.to_string(), format!("{}\nMock file served by the Chatnip mock server.\n", name).into_bytes())
    };
    let size = bytes.len() as u64;
    (format!("data:{};base64,{}", mime, base64::engine::general_purpose::STANDARD.encode(bytes)), mime, size)
}

pub struct MockProvider {
    provider: Provider,
    kind: MockKind,
    pool: sqlx::Pool<sqlx::Sqlite>,
}

impl MockKind {
    pub fn provider(self) -> Provider {
        let kind = self;
        let now = Utc::now();
        Provider {
            id: format!("mock-{}", kind.key()),
            profile_id: String::new(),
            provider_type: "mock".to_string(),
            name: kind.label().to_string(),
            external_id: None,
            logo_id: None,
            logo: None,
            capabilities: ProviderCapabilities {
                can_send_messages: true,
                can_edit_messages: !kind.is_mail(),
                can_delete_messages: !kind.is_mail(),
                can_create_conversations: true,
                can_add_members: true,
                supports_reactions: kind.supports_reactions(),
                supports_threads: !kind.is_mail(),
                supports_attachments: true,
                supports_presence: !kind.is_mail(),
                supports_typing_indicators: !kind.is_mail(),
                supports_read_receipts: !kind.is_mail(),
                supports_reply_quote: true,
                supports_forward: true,
                reply_quote: kind.reply_quote(),
                forward: kind.forward(),
                supports_calendar_responses: !matches!(kind, MockKind::Slack | MockKind::Imap),
                live: true,
                supports_unread_sync: true,
                thread_broadcast: kind.thread_broadcast(),
                drafts: if kind.is_mail() { Delivery::Native } else { Delivery::Unsupported },
                archive: if kind.is_mail() { Delivery::Native } else { Delivery::Unsupported },
                supports_flags: kind.is_mail(),
                supports_channel_create: !kind.is_mail(),
                supports_channel_join: matches!(kind, MockKind::Slack | MockKind::GoogleChat),
                supports_channel_archive: matches!(kind, MockKind::Slack),
                supports_topic: matches!(kind, MockKind::Slack),
                supports_purpose: matches!(kind, MockKind::Slack | MockKind::Teams),
                supports_search: true,
                supports_user_groups: matches!(kind, MockKind::Slack),
                supports_aliases: kind.is_mail(),
                signature_sync: matches!(kind, MockKind::Gmail),
                supports_auto_reply: matches!(kind, MockKind::Outlook | MockKind::Gmail),
            },
            is_enabled: true,
            is_connected: true,
            auth: None,
            sync_interval: 300,
            namespace: Some(kind.key().to_string()),
            delta_token: None,
            last_sync_at: None,
            config: Some(kind.key().to_string()),
            created_at: now,
            updated_at: now,
            theme: None,
            missing_scopes: Vec::new(),
            account: None,
            last_used_at: None,
        }
    }
}

impl MockProvider {
    pub fn new(kind: MockKind, pool: sqlx::Pool<sqlx::Sqlite>) -> Self {
        Self { provider: kind.provider(), kind, pool }
    }

    pub fn kind(&self) -> MockKind {
        self.kind
    }

    pub fn pool(&self) -> &sqlx::Pool<sqlx::Sqlite> {
        &self.pool
    }

    pub(super) fn me(&self) -> ResponseUser {
        ResponseUser {
            id: format!("mock-{}-me", self.kind.key()),
            email: Some("milo@northwind.example".to_string()),
            display_name: Some("Milo Oakes".to_string()),
            first_name: Some("Milo".to_string()),
            last_name: Some("Oakes".to_string()),
            job_title: Some("Founder".to_string()),
            company: Some("Northwind".to_string()),
            ..Default::default()
        }
    }

    pub(super) fn person(&self, index: usize) -> ResponseUser {
        let (email, first, last, title) = CAST[index % CAST.len()];
        let display = if self.kind == MockKind::Slack {
            format!("{}.{}", first.to_lowercase(), last.to_lowercase())
        } else {
            format!("{} {}", first, last)
        };
        ResponseUser {
            id: format!("mock-{}-u{}", self.kind.key(), index),
            email: Some(email.to_string()),
            display_name: Some(display),
            first_name: Some(first.to_string()),
            last_name: Some(last.to_string()),
            job_title: Some(title.to_string()),
            company: Some(email.split('@').nth(1).unwrap_or("").split('.').next().unwrap_or("").to_string()),
            ..Default::default()
        }
    }

    fn service(&self, index: usize) -> ResponseUser {
        let (email, name) = SERVICES[index % SERVICES.len()];
        ResponseUser {
            id: format!("mock-{}-s{}", self.kind.key(), index),
            email: Some(email.to_string()),
            display_name: Some(name.to_string()),
            ..Default::default()
        }
    }

    fn base_time(&self) -> DateTime<Utc> {
        let now = Utc::now();
        let day = now.date_naive().and_hms_opt(12, 0, 0).unwrap();
        DateTime::<Utc>::from_naive_utc_and_offset(day, Utc)
    }

    pub(super) fn meeting_url(&self, id: &str) -> String {
        match self.kind {
            MockKind::Gmail | MockKind::GoogleChat => format!("https://meet.google.com/mock-{}", &id[..8.min(id.len())]),
            _ => format!("https://teams.microsoft.com/l/meetup-join/mock-{}", id),
        }
    }

    pub fn calendars(&self) -> Vec<RemoteCalendar> {
        let calendar = |id: &str, name: &str, colour: &str, primary: bool, write: bool| RemoteCalendar { external_id: format!("mock-{}-{}", self.kind.key(), id), name: name.to_string(), colour: Some(colour.to_string()), is_primary: primary, can_write: write };
        match self.kind {
            MockKind::Outlook => vec![calendar("calendar", "Northwind", "#0f6cbd", true, true), calendar("holidays", "Helligdager i Norge", "#8a8886", false, false)],
            MockKind::Teams => vec![calendar("calendar", "Kunde AS", "#5b5fc7", true, true)],
            MockKind::Gmail => vec![calendar("calendar", "Privat", "#d50000", true, true)],
            MockKind::GoogleChat => vec![calendar("calendar", "Sundial Workspace", "#0b8043", true, true)],
            MockKind::Slack | MockKind::Imap => Vec::new(),
        }
    }

    pub fn calendar_events(&self, calendar_external_id: &str) -> Vec<RemoteEvent> {
        let day = self.base_time().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let at = |days: i64, hour: u32, minute: u32| day + Duration::days(days) + Duration::hours(hour as i64) + Duration::minutes(minute as i64);
        let me = self.me().email.unwrap_or_default();
        let who = |indices: &[usize]| -> Vec<String> { indices.iter().map(|i| CAST[*i].0.to_string()).chain(std::iter::once(me.clone())).collect() };
        let key = self.kind.key();
        let event = |id: &str, uid: Option<&str>, title: &str, start: DateTime<Utc>, minutes: i64, attendees: Vec<String>, organizer: &str, online: bool, location: &str| RemoteEvent {
            external_id: format!("mock-{}-ev-{}", key, id),
            ical_uid: uid.map(String::from),
            title: title.to_string(),
            notes: String::new(),
            location: location.to_string(),
            starts_at: start,
            ends_at: start + Duration::minutes(minutes),
            all_day: false,
            attendees,
            organizer: Some(organizer.to_string()),
            response_status: Some(if organizer == me { "organizer" } else { "accepted" }.to_string()),
            online_meeting_url: online.then(|| self.meeting_url(id)),
            cancelled: false,
            updated_at: day,
        };
        let all_day = |id: &str, title: &str, days: i64, length: i64| RemoteEvent { all_day: true, ends_at: day + Duration::days(days + length), response_status: None, organizer: None, ..event(id, None, title, day + Duration::days(days), 0, Vec::new(), "", false, "") };
        let signing = |id: &str| event(id, Some("040000008200E00074C5B7101A82E008-signering-2027"), "Signering: plattformdrift 2027", at(2, 13, 0), 60, who(&[7, 8]), CAST[7].0, true, "Teams");
        let lunch = |id: &str| event(id, None, "Lunsj med Eva", at(1, 11, 30), 60, who(&[0]), &me, false, "Fyrlykta, Bjørvika");
        let primary = format!("mock-{}-calendar", key);
        match (self.kind, calendar_external_id) {
            (MockKind::Outlook, id) if id == primary => {
                let mut events = vec![
                    signing("signing"),
                    lunch("lunch"),
                    event("sprint", Some("sprintdemo-northwind"), "Sprintdemo", at(3, 14, 0), 45, who(&[1, 2, 3, 4]), CAST[2].0, true, ""),
                    event("one-on-one", None, "1:1 Nora", at(0, 15, 0), 30, who(&[1]), &me, true, ""),
                    event("review", None, "Designgjennomgang", at(-2, 10, 0), 60, who(&[0, 11]), CAST[0].0, true, ""),
                ];
                let mut cancelled = event("cancelled", None, "Avlyst: arkitekturmøte", at(4, 9, 0), 60, who(&[1]), CAST[1].0, true, "");
                cancelled.cancelled = true;
                events.push(cancelled);
                let mut invite = event("invite", Some("arkitektur-tidewater-2026"), "Arkitekturgjennomgang Tidewater", at(1, 9, 0), 60, who(&[9, 10]), CAST[9].0, true, "Rom Fjord");
                invite.response_status = Some("notResponded".to_string());
                events.push(invite);
                events
            }
            (MockKind::Outlook, id) if id.ends_with("-holidays") => vec![all_day("autumn", "Høstferie", 7, 5)],
            (MockKind::Teams, id) if id == primary => {
                let mut events = vec![signing("signing")];
                events.extend((0..5).map(|d| event(&format!("standup-{}", d), Some(&format!("standup-kunde-{}", d)), "Standup Kunde AS", at(d, 9, 15), 15, who(&[8, 9]), CAST[8].0, true, "")));
                events
            }
            (MockKind::Gmail, id) if id == primary => {
                let mut coffee = event("coffee", None, "Kaffe og skisser", at(2, 15, 30), 30, who(&[11]), CAST[11].0, true, "Google Meet");
                coffee.response_status = Some("needsAction".to_string());
                vec![lunch("lunch"), event("dentist", None, "Tannlege", at(5, 8, 0), 60, vec![me.clone()], &me, false, "Tannhelse Grünerløkka"), all_day("cabin", "Hytta", 12, 2), coffee]
            }
            (MockKind::GoogleChat, id) if id == primary => vec![event("board", Some("sundial-styremote-q4"), "Styremøte Sundial", at(6, 16, 0), 90, who(&[5, 6]), CAST[5].0, true, "")],
            _ => Vec::new(),
        }
    }

    fn chat_people(&self) -> Vec<usize> {
        match self.kind {
            MockKind::Teams => vec![0, 1, 2, 3, 4, 5],
            MockKind::Slack => vec![3, 4, 5, 6, 7, 8],
            _ => vec![6, 7, 8, 9, 10, 11],
        }
    }

    fn chat_conversations(&self) -> Vec<(ResponseConversation, Vec<ResponseMessage>)> {
        let mut rng = Lcg(self.kind.seed());
        let base = self.base_time();
        let me = self.me();
        let mut out = Vec::new();
        for (slot, index) in self.chat_people().into_iter().enumerate() {
            let other = self.person(index);
            let external = format!("mock-{}-dm-{}", self.kind.key(), index);
            let count = 4 + rng.below(6) as usize;
            let mut messages = Vec::new();
            let mut at = base - Duration::days(1 + (slot as i64 * 3) + rng.below(4) as i64) - Duration::hours(rng.below(8) as i64);
            for m in 0..count {
                let from_me = m % 2 == 1;
                let line = CHAT_LINES[(rng.below(CHAT_LINES.len() as u64)) as usize];
                let mut message = ResponseMessage {
                    external_id: format!("{}-m{}", external, m),
                    conversation_external_id: external.clone(),
                    sender: if from_me { me.clone() } else { other.clone() },
                    content: line.to_string(),
                    content_type: "text".to_string(),
                    timestamp: at,
                    r#type: MessageType::Chat,
                    is_read: from_me || m < count - 1 || slot % 2 == 1,
                    ..Default::default()
                };
                if m == 1 && slot == 0 {
                    message.attachments.push(ResponseAttachment {
                        external_id: format!("{}-a1", external),
                        name: "skisser-v2.pdf".to_string(),
                        content_type: "application/pdf".to_string(),
                        size: 184_320,
                        url: None,
                        content_id: None,
                    });
                }
                if self.kind.supports_reactions() && m == count - 2 {
                    message.metadata.insert("reactions".to_string(), serde_json::json!([{ "emoji": "👍", "user": other.display_name.clone().unwrap_or_default() }]));
                }
                messages.push(message);
                at = at + Duration::minutes(3 + rng.below(40) as i64);
            }
            let last = messages.last().map(|m| m.timestamp).unwrap_or(base);
            out.push((
                ResponseConversation {
                    external_id: external,
                    name: other.display_name.clone().unwrap_or_default(),
                    conversation_type: ConversationType::DirectMessage,
                    members: vec![me.clone(), other],
                    last_activity: last,
                    metadata: HashMap::new(),
                },
                messages,
            ));
        }
        for (g, (name, people)) in GROUPS.iter().enumerate() {
            let external = format!("mock-{}-group-{}", self.kind.key(), g);
            let members: Vec<ResponseUser> = people.iter().map(|p| self.person(*p)).collect();
            let mut messages = Vec::new();
            let mut at = base - Duration::days(2 + g as i64 * 4) - Duration::hours(rng.below(6) as i64);
            for m in 0..(6 + rng.below(5) as usize) {
                let sender = if m % 4 == 3 { me.clone() } else { members[m % members.len()].clone() };
                let line = CHAT_LINES[(rng.below(CHAT_LINES.len() as u64)) as usize];
                let mut message = ResponseMessage {
                    external_id: format!("{}-m{}", external, m),
                    conversation_external_id: external.clone(),
                    sender,
                    content: line.to_string(),
                    content_type: "text".to_string(),
                    timestamp: at,
                    r#type: MessageType::Chat,
                    is_read: g != 0 || m < 4,
                    ..Default::default()
                };
                if g == 0 && m == 2 {
                    message.thread_id = Some(format!("{}-m1", external));
                    message.reply_to_id = Some(format!("{}-m1", external));
                }
                messages.push(message);
                at = at + Duration::minutes(5 + rng.below(90) as i64);
            }
            let last = messages.last().map(|m| m.timestamp).unwrap_or(base);
            let mut all_members = vec![me.clone()];
            all_members.extend(members);
            out.push((
                ResponseConversation {
                    external_id: external,
                    name: name.to_string(),
                    conversation_type: ConversationType::GroupMessage,
                    members: all_members,
                    last_activity: last,
                    metadata: HashMap::new(),
                },
                messages,
            ));
        }
        if !self.kind.is_mail() {
            for (c, (name, people)) in CHANNELS.iter().enumerate() {
                let external = format!("mock-{}-channel-{}", self.kind.key(), c);
                let members: Vec<ResponseUser> = people.iter().map(|p| self.person(*p)).collect();
                let mut messages = Vec::new();
                let mut at = base - Duration::days(1 + c as i64 * 2) - Duration::hours(rng.below(10) as i64);
                for m in 0..(8 + rng.below(6) as usize) {
                    let sender = members[(m + c) % members.len()].clone();
                    let line = CHAT_LINES[(rng.below(CHAT_LINES.len() as u64)) as usize];
                    messages.push(ResponseMessage {
                        external_id: format!("{}-m{}", external, m),
                        conversation_external_id: external.clone(),
                        sender,
                        content: line.to_string(),
                        content_type: "text".to_string(),
                        timestamp: at,
                        r#type: MessageType::Chat,
                        is_read: true,
                        ..Default::default()
                    });
                    at = at + Duration::minutes(10 + rng.below(240) as i64);
                }
                let last = messages.last().map(|m| m.timestamp).unwrap_or(base);
                let mut all_members = vec![me.clone()];
                all_members.extend(members);
                let mut metadata = HashMap::new();
                metadata.insert("team".to_string(), serde_json::json!("Northwind"));
                out.push((
                    ResponseConversation {
                        external_id: external,
                        name: name.to_string(),
                        conversation_type: ConversationType::Channel,
                        members: all_members,
                        last_activity: last,
                        metadata,
                    },
                    messages,
                ));
            }
        }
        out
    }

    fn mail_conversations(&self) -> Vec<(ResponseConversation, Vec<ResponseMessage>)> {
        let mut rng = Lcg(self.kind.seed());
        let base = self.base_time();
        let me = self.me();
        let mut out = Vec::new();
        for (i, (subject, person, body)) in HUMAN_MAIL[self.kind.mail_slice()].iter().enumerate() {
            let other = self.person(*person);
            let external = format!("mock-{}-mail-h{}", self.kind.key(), i);
            let first_at = base - Duration::days(1 + i as i64 * 5) - Duration::hours(rng.below(9) as i64);
            let (headers, origin) = mail_origin(*person, first_at, other.email.as_deref().unwrap_or("someone@example.org"));
            let mut messages = vec![ResponseMessage {
                external_id: format!("{}-m0", external),
                conversation_external_id: external.clone(),
                sender: other.clone(),
                content: body.to_string(),
                content_type: "html".to_string(),
                timestamp: first_at,
                r#type: MessageType::Email,
                subject: Some(subject.to_string()),
                recipients: vec![me.clone()],
                headers: Some(headers),
                origin: Some(origin),
                is_read: i != 0,
                attachments: if i == 1 {
                    vec![ResponseAttachment { external_id: format!("{}-a0", external), name: "kubeconfig.yaml".to_string(), content_type: "application/x-yaml".to_string(), size: 2_048, url: None, content_id: None }]
                } else {
                    Vec::new()
                },
                ..Default::default()
            }];
            if i == 2 {
                messages.push(ResponseMessage {
                    external_id: format!("{}-m1", external),
                    conversation_external_id: external.clone(),
                    sender: me.clone(),
                    content: "<p>Takk Mia! Retningen stemmer, kjør på med resten.</p>".to_string(),
                    content_type: "html".to_string(),
                    timestamp: first_at + Duration::hours(3),
                    r#type: MessageType::Email,
                    subject: Some(format!("Re: {}", subject)),
                    recipients: vec![other.clone()],
                    is_read: true,
                    reply_to_id: Some(format!("{}-m0", external)),
                    ..Default::default()
                });
            }
            let last = messages.last().map(|m| m.timestamp).unwrap_or(base);
            out.push((
                ResponseConversation {
                    external_id: external,
                    name: subject.to_string(),
                    conversation_type: ConversationType::Email,
                    members: vec![me.clone(), other],
                    last_activity: last,
                    metadata: HashMap::new(),
                },
                messages,
            ));
        }
        if matches!(self.kind, MockKind::Outlook | MockKind::Gmail) {
            let anna = self.person(7);
            let external = format!("mock-{}-mail-contract", self.kind.key());
            let subject = "Kontrakt Kunde AS";
            let first_at = base - Duration::days(2) - Duration::hours(3);
            let (headers, origin) = mail_origin(7, first_at, anna.email.as_deref().unwrap_or("anna.berg@client.example"));
            let messages = if self.kind == MockKind::Outlook {
                vec![
                    ResponseMessage {
                        external_id: format!("{}-m0", external),
                        conversation_external_id: external.clone(),
                        sender: anna.clone(),
                        content: "<p>Hei! Vi er enige om rammene. Juristene våre ferdigstiller kontrakten og jeg sender den så snart den er signert.</p><p>Mvh Anna</p>".to_string(),
                        content_type: "html".to_string(),
                        timestamp: first_at,
                        r#type: MessageType::Email,
                        subject: Some(subject.to_string()),
                        recipients: vec![me.clone()],
                        headers: Some(headers),
                        origin: Some(origin),
                        is_read: true,
                        ..Default::default()
                    },
                    ResponseMessage {
                        external_id: format!("{}-m1", external),
                        conversation_external_id: external.clone(),
                        sender: me.clone(),
                        content: "<p>Supert, takk Anna. Vi venter på den signerte versjonen.</p>".to_string(),
                        content_type: "html".to_string(),
                        timestamp: first_at + Duration::hours(1),
                        r#type: MessageType::Email,
                        subject: Some(format!("Re: {}", subject)),
                        recipients: vec![anna.clone()],
                        is_read: true,
                        reply_to_id: Some(format!("{}-m0", external)),
                        ..Default::default()
                    },
                ]
            } else {
                let at = base - Duration::hours(2);
                let (headers, origin) = mail_origin(7, at, anna.email.as_deref().unwrap_or("anna.berg@client.example"));
                vec![ResponseMessage {
                    external_id: format!("{}-m0", external),
                    conversation_external_id: external.clone(),
                    sender: anna.clone(),
                    content: "<p>Hei igjen! Her er den signerte kontrakten, sendt fra privat konto siden Outlook hos oss er nede.</p><p>Mvh Anna</p>".to_string(),
                    content_type: "html".to_string(),
                    timestamp: at,
                    r#type: MessageType::Email,
                    subject: Some(format!("Re: {}", subject)),
                    recipients: vec![me.clone()],
                    headers: Some(headers),
                    origin: Some(origin),
                    is_read: false,
                    attachments: vec![ResponseAttachment { external_id: format!("{}-a0", external), name: "contract.pdf".to_string(), content_type: "application/pdf".to_string(), size: 184_320, url: None, content_id: None }],
                    ..Default::default()
                }]
            };
            let last = messages.last().map(|m| m.timestamp).unwrap_or(base);
            out.push((
                ResponseConversation {
                    external_id: external,
                    name: subject.to_string(),
                    conversation_type: ConversationType::Email,
                    members: vec![me.clone(), anna],
                    last_activity: last,
                    metadata: HashMap::new(),
                },
                messages,
            ));
        }
        for (i, (subject, service, body)) in SERVICE_MAIL[self.kind.mail_slice()].iter().enumerate() {
            let sender = self.service(*service);
            let external = format!("mock-{}-mail-s{}", self.kind.key(), i);
            let at = base - Duration::days(i as i64 * 2) - Duration::hours(2 + rng.below(6) as i64);
            let (headers, origin) = mail_origin(8 + *service, at, sender.email.as_deref().unwrap_or("noreply@example.org"));
            let messages = vec![ResponseMessage {
                external_id: format!("{}-m0", external),
                conversation_external_id: external.clone(),
                sender: sender.clone(),
                content: body.to_string(),
                content_type: "html".to_string(),
                timestamp: at,
                r#type: MessageType::Email,
                subject: Some(subject.to_string()),
                recipients: vec![me.clone()],
                headers: Some(headers),
                origin: Some(origin),
                is_read: i == 0,
                ..Default::default()
            }];
            out.push((
                ResponseConversation {
                    external_id: external,
                    name: subject.to_string(),
                    conversation_type: ConversationType::Email,
                    members: vec![me.clone(), sender],
                    last_activity: at,
                    metadata: HashMap::new(),
                },
                messages,
            ));
        }
        out
    }

    fn message_mut<'a>(data: &'a mut [(ResponseConversation, Vec<ResponseMessage>)], external_id: &str) -> Option<&'a mut ResponseMessage> {
        data.iter_mut().flat_map(|(_, messages)| messages.iter_mut()).find(|m| m.external_id == external_id)
    }

    fn mail_fixture(message: &mut ResponseMessage, raw: &str) {
        let Some(mail) = crate::content::mail::parse(raw.as_bytes()) else { return };
        let (content, content_type) = match (&mail.html, &mail.plain) {
            (Some(html), _) => (html.clone(), "html"),
            (None, plain) => (plain.clone().unwrap_or_default(), "text"),
        };
        message.content = content;
        message.content_type = content_type.to_string();
        message.attachments = mail
            .parts
            .iter()
            .enumerate()
            .map(|(i, part)| ResponseAttachment { external_id: format!("{}-p{}", message.external_id, i), name: part.name.clone(), content_type: part.mime.clone(), size: part.size, url: part.data.clone(), content_id: part.content_id.clone() })
            .collect();
        mail.structure.apply(message);
    }

    fn chat_fixture(message: &mut ResponseMessage, kind: MockKind, payload: &serde_json::Value) {
        let content = match kind {
            MockKind::Teams => crate::content::chat::teams(payload),
            MockKind::Slack => crate::content::chat::slack(payload),
            _ => crate::content::chat::google_chat(payload),
        };
        match (kind, &content.html) {
            (MockKind::Teams, Some(html)) => {
                message.content = html.clone();
                message.content_type = "html".to_string();
            }
            _ => {
                message.content = content.text.clone();
                message.content_type = "text".to_string();
            }
        }
        if content.is_bot {
            message.r#type = MessageType::Bot;
        }
        let (thread_id, reply_to_id) = (message.thread_id.clone(), message.reply_to_id.clone());
        content.apply(message);
        message.thread_id = thread_id;
        message.reply_to_id = reply_to_id.or_else(|| content.reply_to.as_ref().and_then(|_| message.external_id.strip_suffix("-m2").map(|base| format!("{}-m1", base)))).or_else(|| content.reply_to.clone());
    }

    fn vary(&self, data: &mut [(ResponseConversation, Vec<ResponseMessage>)]) {
        let key = self.kind.key();
        if self.kind.is_mail() {
            let fixtures: &[(&str, &str)] = match self.kind {
                MockKind::Outlook => &[("h0", ATTACHMENTS_EML), ("s0", INVITE_EML)],
                MockKind::Gmail => &[("h1", OUTLOOK_EML), ("h2", APPLE_EML), ("s0", NEWSLETTER_EML)],
                _ => &[("h0", PLAIN_EML), ("h1", GMAIL_EML)],
            };
            for (_, messages) in data.iter_mut() {
                for message in messages.iter_mut() {
                    crate::content::mail::analyse(Some(&message.content), None, message.headers.as_deref(), None).apply(message);
                }
            }
            for (slot, raw) in fixtures {
                if let Some(message) = Self::message_mut(data, &format!("mock-{}-mail-{}-m0", key, slot)) {
                    Self::mail_fixture(message, raw);
                }
            }
            if let Some(message) = Self::message_mut(data, &format!("mock-{}-mail-h2-m1", key)).filter(|_| self.kind == MockKind::Imap) {
                message.content = "Takk Mia! Retningen stemmer, kjør på med resten.\n\n-- \nMilo Oakes\nNorthwind\n\nDen 8. okt. 2026 kl. 10:00 skrev Mia Sund <mia.sund@freelance.example>:\n> Her er bildene fra workshopen i forrige uke.\n> Fritt frem for bruk på nettsiden.".to_string();
                message.content_type = "text".to_string();
                crate::content::mail::analyse(None, Some(&message.content), message.headers.as_deref(), None).apply(message);
            }
            return;
        }
        for (_, messages) in data.iter_mut() {
            for message in messages.iter_mut() {
                crate::content::chat::markdown(&message.content).apply(message);
            }
        }
        let first_dm = format!("mock-{}-dm-{}", key, self.chat_people()[0]);
        let second_dm = format!("mock-{}-dm-{}", key, self.chat_people()[1]);
        let third_dm = format!("mock-{}-dm-{}", key, self.chat_people()[2]);
        let slack: serde_json::Value = serde_json::from_str(SLACK_JSON).unwrap_or_default();
        let fixtures: Vec<(String, serde_json::Value)> = match self.kind {
            MockKind::Teams => vec![(format!("{}-m2", first_dm), serde_json::from_str(TEAMS_JSON).unwrap_or_default())],
            MockKind::Slack => vec![
                (format!("{}-m2", first_dm), slack["messages"][0].clone()),
                (format!("mock-{}-group-0-m2", key), slack["messages"][1].clone()),
                (format!("mock-{}-channel-1-m0", key), slack["messages"][2].clone()),
                (format!("mock-{}-channel-2-m0", key), slack["messages"][3].clone()),
            ],
            _ => vec![(format!("{}-m2", first_dm), serde_json::from_str(GCHAT_JSON).unwrap_or_default())],
        };
        for (id, payload) in fixtures {
            if let Some(message) = Self::message_mut(data, &id) {
                Self::chat_fixture(message, self.kind, &payload);
            }
        }
        let long: String = (1..=200).map(|i| format!("{:03} Logglinje fra sync-løkka med æøå og 🚀, se https://northwind.example/logg/{} for detaljer om kjøringen.\n", i, i)).collect();
        let texts = [
            (format!("{}-m1", second_dm), "🎉🎉🎉".to_string()),
            (format!("{}-m0", third_dm), "مرحبا Milo! שלום 👋 Привет, møtet er kl 10 ✅".to_string()),
            (format!("mock-{}-group-0-m3", key), MARKDOWN_SAMPLE.to_string()),
            (format!("mock-{}-channel-0-m1", key), long),
            (format!("{}-m0", second_dm), LINKS_SAMPLE.to_string()),
        ];
        for (id, text) in texts {
            if let Some(message) = Self::message_mut(data, &id) {
                message.content = text;
                message.content_type = "text".to_string();
                message.metadata.remove("structure");
                crate::content::chat::markdown(&message.content).apply(message);
            }
        }
        self.rich_chat(data);
    }

    fn mail_fixtures(&self) -> &'static [(&'static str, &'static str)] {
        match self.kind {
            MockKind::Outlook => &[("retailer", MOCK_RETAILER), ("linear", MOCK_LINEAR), ("sprintdemo", MOCK_INVITE), ("payslip", MOCK_DARK)],
            MockKind::Gmail => &[("tldr", MOCK_TLDR), ("github", MOCK_GITHUB), ("strategy", MOCK_LONGFORM), ("summit", MOCK_TRACKING)],
            MockKind::Imap => &[("receipt", MOCK_RECEIPT), ("icons", MOCK_APPLE), ("status", MOCK_INLINE), ("pricing", MOCK_PLAIN)],
            _ => &[],
        }
    }

    pub(super) fn addressee(&self, address: &crate::content::mail::Address) -> ResponseUser {
        if address.email == "milo@northwind.example" {
            return self.me();
        }
        match CAST.iter().position(|(email, ..)| *email == address.email) {
            Some(index) => self.person(index),
            None => ResponseUser {
                id: format!("mock-{}-x-{}", self.kind.key(), address.email),
                email: Some(address.email.clone()),
                display_name: Some(address.name.clone().unwrap_or_else(|| address.email.clone())),
                ..Default::default()
            },
        }
    }

    fn fixture_mail(&self) -> Vec<(ResponseConversation, Vec<ResponseMessage>)> {
        let base = self.base_time();
        let me = self.me();
        self.mail_fixtures()
            .iter()
            .enumerate()
            .filter_map(|(i, (slug, raw))| {
                let mail = crate::content::mail::parse(raw.as_bytes())?;
                let sender = self.addressee(mail.from.as_ref()?);
                let external = format!("mock-{}-mail-{}", self.kind.key(), slug);
                let at = base - Duration::hours(5 + i as i64 * 31);
                let (headers, origin) = mail_origin(i + 2, at, sender.email.as_deref().unwrap_or("someone@example.org"));
                let subject = mail.subject.clone().unwrap_or_default();
                let mut message = ResponseMessage {
                    external_id: format!("{}-m0", external),
                    conversation_external_id: external.clone(),
                    sender: sender.clone(),
                    timestamp: at,
                    r#type: MessageType::Email,
                    subject: Some(subject.clone()),
                    recipients: mail.to.iter().map(|a| self.addressee(a)).collect(),
                    cc: mail.cc.iter().map(|a| self.addressee(a)).collect(),
                    headers: Some(headers),
                    origin: Some(origin),
                    is_read: i % 2 == 1,
                    ..Default::default()
                };
                Self::mail_fixture(&mut message, raw);
                Some((
                    ResponseConversation { external_id: external, name: subject, conversation_type: ConversationType::Email, members: vec![me.clone(), sender], last_activity: at, metadata: HashMap::new() },
                    vec![message],
                ))
            })
            .collect()
    }

    fn mail_thread(&self, slug: &str, subject: &str, start: DateTime<Utc>, steps: &[(Option<usize>, &str)], render: &dyn Fn(&ResponseUser, bool, &str, Option<(&ResponseUser, DateTime<Utc>, &str)>) -> String) -> (ResponseConversation, Vec<ResponseMessage>) {
        let me = self.me();
        let external = format!("mock-{}-mail-{}", self.kind.key(), slug);
        let mut messages: Vec<ResponseMessage> = Vec::new();
        let mut previous: Option<(ResponseUser, DateTime<Utc>, String)> = None;
        let people: Vec<ResponseUser> = steps.iter().filter_map(|(who, _)| who.map(|p| self.person(p))).fold(Vec::new(), |mut acc, p| {
            if !acc.iter().any(|x: &ResponseUser| x.id == p.id) {
                acc.push(p);
            }
            acc
        });
        for (i, (who, body)) in steps.iter().enumerate() {
            let sender = who.map(|p| self.person(p)).unwrap_or_else(|| me.clone());
            let at = start + Duration::hours(i as i64 * 7) + Duration::minutes(i as i64 * 13);
            let html = render(&sender, who.is_none(), body, previous.as_ref().map(|(p, sent, html)| (p, *sent, html.as_str())));
            let (headers, origin) = match who {
                Some(p) => {
                    let (h, o) = mail_origin(*p, at, sender.email.as_deref().unwrap_or("someone@example.org"));
                    (Some(h), Some(o))
                }
                None => (None, None),
            };
            let mut message = ResponseMessage {
                external_id: format!("{}-m{}", external, i),
                conversation_external_id: external.clone(),
                sender: sender.clone(),
                content: html.clone(),
                content_type: "html".to_string(),
                timestamp: at,
                r#type: MessageType::Email,
                subject: Some(if i == 0 { subject.to_string() } else { format!("Re: {}", subject) }),
                recipients: std::iter::once(me.clone()).chain(people.iter().cloned()).filter(|p| p.id != sender.id).collect(),
                headers,
                origin,
                is_read: i + 1 < steps.len(),
                reply_to_id: (i > 0).then(|| format!("{}-m{}", external, i - 1)),
                ..Default::default()
            };
            crate::content::mail::analyse(Some(&html), None, message.headers.as_deref(), None).apply(&mut message);
            messages.push(message);
            previous = Some((sender, at, html));
        }
        let last = messages.last().map(|m| m.timestamp).unwrap_or(start);
        let mut members = vec![me];
        members.extend(people);
        (ResponseConversation { external_id: external, name: subject.to_string(), conversation_type: ConversationType::Email, members, last_activity: last, metadata: HashMap::new() }, messages)
    }

    fn mail_threads(&self) -> Vec<(ResponseConversation, Vec<ResponseMessage>)> {
        let base = self.base_time();
        let name = |u: &ResponseUser| u.display_name.clone().unwrap_or_default();
        let email = |u: &ResponseUser| u.email.clone().unwrap_or_default();
        match self.kind {
            MockKind::Outlook => {
                let subject = "Avtale om plattformdrift 2027";
                let to = "Milo Oakes; Anna Berg; Jonas Lie";
                let outlook = |sender: &ResponseUser, mine: bool, body: &str, previous: Option<(&ResponseUser, DateTime<Utc>, &str)>| -> String {
                    let (company, domain, logo) = if mine { ("Northwind", "northwind.example", "https://placehold.co/192x64/2f4fd8/ffffff/png?text=Northwind") } else { ("Kunde AS", "client.example", "https://placehold.co/192x64/c8102e/ffffff/png?text=Kunde+AS") };
                    let quote = previous
                        .map(|(p, sent, html)| {
                            let inner = html.split("<div class=\"WordSection1\">").nth(1).and_then(|s| s.rsplit_once("</div>")).map(|(s, _)| s.to_string()).unwrap_or_default();
                            OUTLOOK_QUOTE_HTML
                                .replace("{name}", &name(p))
                                .replace("{email}", &email(p))
                                .replace("{sent}", &sent.format("%A, %-d %B %Y %H:%M").to_string())
                                .replace("{to}", to)
                                .replace("{subject}", subject)
                                .replace("{previous}", &inner)
                        })
                        .unwrap_or_default();
                    OUTLOOK_REPLY_HTML
                        .replace("{name}", &name(sender))
                        .replace("{title}", sender.job_title.as_deref().unwrap_or(""))
                        .replace("{company}", company)
                        .replace("{domain}", domain)
                        .replace("{logo}", logo)
                        .replace("{body}", body)
                        .replace("{quote}", &quote)
                };
                vec![self.mail_thread("agreement", subject, base - Duration::days(4), &OUTLOOK_THREAD, &outlook)]
            }
            MockKind::Gmail => {
                let gmail = |sender: &ResponseUser, _mine: bool, body: &str, previous: Option<(&ResponseUser, DateTime<Utc>, &str)>| -> String {
                    let quote = previous
                        .map(|(p, sent, html)| GMAIL_QUOTE_HTML.trim_end().replace("{sent}", &sent.format("%a, %-d %b %Y at %H:%M").to_string()).replace("{name}", &name(p)).replace("{email}", &email(p)).replace("{previous}", html))
                        .unwrap_or_default();
                    GMAIL_REPLY_HTML.trim_end().replace("{name}", &name(sender)).replace("{title}", sender.job_title.as_deref().unwrap_or("")).replace("{body}", body).replace("{quote}", &quote)
                };
                vec![self.mail_thread("onboarding", "Illustrasjoner til onboarding", base - Duration::days(3), &GMAIL_THREAD, &gmail)]
            }
            _ => Vec::new(),
        }
    }

    fn chat_message(&self, external_id: String, sender: ResponseUser, content: &str) -> ResponseMessage {
        let mut message = ResponseMessage { external_id, sender, content: content.to_string(), content_type: "text".to_string(), r#type: MessageType::Chat, is_read: true, ..Default::default() };
        crate::content::chat::markdown(&message.content).apply(&mut message);
        message
    }

    fn append(data: &mut [(ResponseConversation, Vec<ResponseMessage>)], external: &str, extra: Vec<ResponseMessage>) {
        let Some((conversation, messages)) = data.iter_mut().find(|(c, _)| c.external_id == external) else { return };
        let mut at = messages.last().map(|m| m.timestamp).unwrap_or(conversation.last_activity);
        for mut message in extra {
            at += Duration::minutes(2);
            message.timestamp = at;
            message.conversation_external_id = external.to_string();
            if message.metadata.contains_key("edited_at") {
                message.metadata.insert("edited_at".to_string(), serde_json::json!((at + Duration::minutes(8)).to_rfc3339()));
            }
            if message.metadata.contains_key("deleted_at") {
                message.metadata.insert("deleted_at".to_string(), serde_json::json!((at + Duration::minutes(1)).to_rfc3339()));
            }
            messages.push(message);
        }
        conversation.last_activity = conversation.last_activity.max(at);
    }

    fn rich_chat(&self, data: &mut [(ResponseConversation, Vec<ResponseMessage>)]) {
        let key = self.kind.key();
        let group = format!("mock-{}-group-0", key);
        let general = format!("mock-{}-channel-0", key);
        let dev = format!("mock-{}-channel-1", key);
        let second_dm = format!("mock-{}-dm-{}", key, self.chat_people()[1]);
        let third_dm = format!("mock-{}-dm-{}", key, self.chat_people()[2]);
        let media = [(format!("{}-m2", second_dm), "celebrate.gif", "image/gif"), (format!("{}-m2", third_dm), "whiteboard.png", "image/png")];
        for (id, name, mime) in media {
            if let Some(message) = Self::message_mut(data, &id) {
                message.content = String::new();
                message.metadata.remove("structure");
                message.attachments = vec![ResponseAttachment { external_id: format!("{}-media", id), name: name.to_string(), content_type: mime.to_string(), size: 0, url: None, content_id: None }];
                crate::content::chat::markdown("").apply(message);
            }
        }
        let run: Vec<ResponseMessage> = (0..22).map(|i| self.chat_message(format!("{}-run{}", general, i), self.person(CHANNELS[0].1[(i / 3) % 4]), CHAT_LINES[(i * 5) % CHAT_LINES.len()])).collect();
        Self::append(data, &general, run);
        let mut extra: Vec<ResponseMessage> = Vec::new();
        if self.kind == MockKind::Teams {
            let teams: serde_json::Value = serde_json::from_str(TEAMS_RICH).unwrap_or_default();
            let chart = chart_uri();
            let ids: Vec<String> = ["table", "image", "mention", "reply", "edited", "deleted"].iter().filter_map(|k| teams[*k]["id"].as_str().map(String::from)).collect();
            for slot in ["table", "image", "mention", "reply", "edited", "deleted"] {
                let payload = &teams[slot];
                let display = payload["from"]["user"]["displayName"].as_str().unwrap_or("");
                let person = CAST.iter().position(|(_, first, last, _)| format!("{} {}", first, last) == display).unwrap_or(0);
                let mut message = ResponseMessage { external_id: format!("{}-x-{}", group, payload["id"].as_str().unwrap_or(slot)), sender: self.person(person), r#type: MessageType::Chat, is_read: true, ..Default::default() };
                Self::chat_fixture(&mut message, self.kind, payload);
                let hosted: Vec<String> = message.metadata.get("structure").and_then(|s| s["images"].as_array()).map(|a| a.iter().filter_map(|i| i.as_str()).filter(|s| s.contains("hostedContents")).map(String::from).collect()).unwrap_or_default();
                for src in hosted {
                    message.content = message.content.replace(&src, &chart);
                }
                if let Some(reply) = message.reply_to_id.clone().filter(|r| ids.contains(r)) {
                    message.reply_to_id = Some(format!("{}-x-{}", group, reply));
                }
                extra.push(message);
            }
        }
        let system = |slug: &str, conversation: &str, sender: usize, kind: SystemEventKind, actors: &[usize], targets: &[usize], detail: Option<&str>| {
            let people = |list: &[usize]| list.iter().map(|i| SystemPerson::from(&self.person(*i))).collect::<Vec<_>>();
            let mut message = ResponseMessage::system(format!("{}-{}", conversation, slug), conversation.to_string(), Utc::now(), SystemEvent::new(kind, people(actors), people(targets), detail.map(String::from)));
            message.sender = self.person(sender);
            message.is_read = true;
            let text = message.content.clone();
            crate::content::chat::markdown(&text).apply(&mut message);
            message
        };
        extra.push(system("joined", &group, 11, SystemEventKind::MemberJoined, &[11], &[], None));
        extra.push(system("left", &group, 6, SystemEventKind::MemberLeft, &[6], &[], None));
        extra.push(system("added", &group, 0, SystemEventKind::MemberAdded, &[0], &[3, 7], None));
        extra.push(system("removed", &group, 1, SystemEventKind::MemberRemoved, &[1], &[10], None));
        extra.push(system("renamed", &group, 2, SystemEventKind::Renamed, &[2], &[], Some(GROUPS[0].0)));
        Self::append(data, &group, extra);
        let lively = vec![
            system("topic", &dev, 2, SystemEventKind::TopicChanged, &[2], &[], Some("Release 0.5 og mock serveren")),
            system("call-start", &dev, 1, SystemEventKind::CallStarted, &[1], &[], None),
            system("call-end", &dev, 1, SystemEventKind::CallEnded, &[], &[], None),
            system("pinned", &dev, 3, SystemEventKind::Pinned, &[3], &[], None),
            system("unpinned", &dev, 3, SystemEventKind::Unpinned, &[3], &[], None),
        ];
        Self::append(data, &dev, lively);
        let board = format!("mock-{}-group-3", key);
        let created = system("created", &board, 1, SystemEventKind::ChatCreated, &[1], &[], None);
        if let Some((_, messages)) = data.iter_mut().find(|(c, _)| c.external_id == board) {
            let mut created = created;
            created.timestamp = messages.iter().map(|m| m.timestamp).min().unwrap_or_else(Utc::now) - Duration::minutes(5);
            messages.insert(0, created);
        }
        if self.kind == MockKind::Slack {
            let slack: serde_json::Value = serde_json::from_str(SLACK_RICH).unwrap_or_default();
            let build = |payload: &serde_json::Value| {
                let mut message = ResponseMessage {
                    external_id: format!("{}-x-{}", dev, payload["ts"].as_str().unwrap_or("")),
                    sender: self.person(slack_person(payload["user"].as_str().unwrap_or(""))),
                    r#type: MessageType::Chat,
                    is_read: true,
                    ..Default::default()
                };
                Self::chat_fixture(&mut message, MockKind::Slack, payload);
                message
            };
            let parent = build(&slack["parent"]);
            let parent_id = parent.external_id.clone();
            let mut thread = vec![parent];
            for reply in slack["replies"].as_array().cloned().unwrap_or_default() {
                let mut message = build(&reply);
                message.thread_id = Some(parent_id.clone());
                message.reply_to_id = Some(parent_id.clone());
                thread.push(message);
            }
            let mut huddle = build(&slack["huddle"]);
            huddle.r#type = MessageType::System;
            thread.push(huddle);
            thread.push(build(&slack["files"]));
            Self::append(data, &dev, thread);
        }
    }

    fn extras(&self, data: &mut Vec<(ResponseConversation, Vec<ResponseMessage>)>) {
        let key = self.kind.key();
        let base = self.base_time();
        let me = self.me();
        let day = (base + Duration::days(1)).format("%A %-d. %B").to_string();
        if self.kind.is_mail() {
            let it = ResponseUser { id: format!("mock-{}-x-it", key), email: Some("no-reply@it.client.example".to_string()), display_name: Some("Kunde AS IT".to_string()), ..Default::default() };
            let alias = ResponseUser { id: format!("mock-{}-x-anna-privat", key), email: Some("anna.berg.private@mail.example".to_string()), display_name: Some("Anna Berg".to_string()), first_name: Some("Anna".to_string()), last_name: Some("Berg".to_string()), ..Default::default() };
            let mails: Vec<(&str, ResponseUser, usize, String, String)> = match self.kind {
                MockKind::Gmail => vec![
                    ("zoom", self.person(8), 3, "Zoom: Pricing review with Kunde AS".to_string(), format!("<p>Hi Milo,</p><p>Jonas Lie is inviting you to a scheduled Zoom meeting.</p><p><b>Topic:</b> Pricing review<br><b>Time:</b> {} 14:00 Europe/Oslo</p><p>Join Zoom Meeting<br><a href=\"https://zoom.us/j/93018477261?pwd=mock\">https://zoom.us/j/93018477261?pwd=mock</a></p><p>Meeting ID: 930 1847 7261<br>Passcode: 4471</p>", day)),
                    ("meet", self.person(11), 6, "Invitasjon: Skissegjennomgang".to_string(), format!("<p>Hei Milo!</p><p>Mia Sund har invitert deg til en gjennomgang av skissene {} kl. 10:00–10:30.</p><p>Bli med i Google Meet: <a href=\"https://meet.google.com/abc-defg-hij\">meet.google.com/abc-defg-hij</a></p><p>Eller ring inn: +47 21 00 00 00, PIN 552 118#</p>", day)),
                    ("alias", alias, 9, "Signert kopi fra privat konto".to_string(), "<p>Hei Milo,</p><p>Sender den signerte kopien fra min private adresse, jobbmailen er nede igjen.</p><p>Mvh<br>Anna</p>".to_string()),
                ],
                MockKind::Outlook => vec![
                    ("teams-invite", self.person(9), 4, "Teams-møte: Gjennomgang av clusteret".to_string(), format!("<p>Hei,</p><p>Kari Nordmann inviterer deg til et Microsoft Teams-møte {} kl. 13:00–14:00.</p><p><a href=\"https://teams.microsoft.com/l/meetup-join/19%3ameeting_mock_cluster%40thread.v2/0\">Bli med i møtet nå</a></p><p>Møte-ID: 341 223 998 1<br>Passord: xY7kq2</p>", day)),
                    ("password", it, 7, "Passordet ditt utløper om 3 dager".to_string(), "<p>Hei Milo Oakes,</p><p>Passordet til kontoen din hos Kunde AS utløper om 3 dager. Bytt det i portalen før det går ut.</p><p>Dette er en automatisk melding. Ikke svar på denne e-posten.</p>".to_string()),
                ],
                _ => vec![
                    ("webinar", self.person(5), 5, "Webinar invite: Local first apps, Thursday 16:00".to_string(), "<p>Hi Milo,</p><p>You are invited to our webinar on local first apps on Thursday at 16:00 CET.</p><p>Join on Zoom: <a href=\"https://zoom.us/j/81122334455\">https://zoom.us/j/81122334455</a></p>".to_string()),
                ],
            };
            for (slug, sender, hours, subject, html) in mails {
                let external = format!("mock-{}-mail-{}", key, slug);
                let at = base - Duration::hours(hours as i64);
                let (headers, origin) = mail_origin(hours, at, sender.email.as_deref().unwrap_or("someone@example.org"));
                let mut message = ResponseMessage {
                    external_id: format!("{}-m0", external),
                    conversation_external_id: external.clone(),
                    sender: sender.clone(),
                    content: html.clone(),
                    content_type: "html".to_string(),
                    timestamp: at,
                    r#type: MessageType::Email,
                    subject: Some(subject.clone()),
                    recipients: vec![me.clone()],
                    headers: Some(headers),
                    origin: Some(origin),
                    is_read: false,
                    ..Default::default()
                };
                crate::content::mail::analyse(Some(&html), None, message.headers.as_deref(), None).apply(&mut message);
                data.push((ResponseConversation { external_id: external, name: subject, conversation_type: ConversationType::Email, members: vec![me.clone(), sender], last_activity: at, metadata: HashMap::new() }, vec![message]));
            }
            return;
        }
        let people = self.chat_people();
        let first_dm = format!("mock-{}-dm-{}", key, people[0]);
        let third_dm = format!("mock-{}-dm-{}", key, people[2]);
        let group = format!("mock-{}-group-0", key);
        let design = format!("mock-{}-group-1", key);
        let general = format!("mock-{}-channel-0", key);
        let dev = format!("mock-{}-channel-1", key);
        let other = self.person(people[0]);
        let previous = data.iter().find(|(c, _)| c.external_id == first_dm).and_then(|(_, m)| m.iter().rev().find(|m| m.sender.id == me.id).cloned());
        if let Some(previous) = previous {
            let mut quoted = self.chat_message(format!("{}-quote", first_dm), other.clone(), &format!("> **{}**: {}\n\nFlott, da sier vi det!", me.display_name.clone().unwrap_or_default(), previous.content.lines().next().unwrap_or_default()));
            quoted.reply_to_id = Some(previous.external_id.clone());
            quoted.metadata.insert("quote".to_string(), serde_json::json!({ "id": previous.external_id, "sender": me.display_name, "text": previous.content, "native": self.kind.reply_quote() == Delivery::Native }));
            Self::append(data, &first_dm, vec![quoted]);
        }
        let at = (base - Duration::days(2)).format("%Y-%m-%d %H:%M");
        let mut forwarded = self.chat_message(format!("{}-forward", group), self.person(2), &format!("FYI fra kunden\n\n**Forwarded from Anna Berg in Kontrakt Kunde AS** · {}\n> Vi er enige om rammene. Juristene ferdigstiller kontrakten.\n", at));
        forwarded.metadata.insert("forwarded".to_string(), serde_json::json!([{ "id": "mock-outlook-mail-contract-m0", "sender": "Anna Berg", "native": false }]));
        let plan = "# Sprintplan uke 43\n## Mål\n- [x] Mock server med rundtur\n- [ ] Skrivevarsel\n- [ ] Svar på kalenderinvitasjoner\n\n## Fordeling\n| Oppgave | Eier | Status |\n|---|---|---|\n| Seed | Milo | Ferdig |\n| Statistikk | Eva | Pågår |\n| Kalender | Sam | Ikke startet |\n\n```ts\nconst ready = tasks.every((t) => t.done)\n```\n\n> Demo fredag kl. 14";
        let mut mention = self.chat_message(format!("{}-mention", general), self.person(CHANNELS[0].1[1]), &format!("@{} kan du se på deploy-loggen før lunsj? Den feiler på steg 3.", me.first_name.clone().unwrap_or_default()));
        mention.metadata.insert("mentions_me".to_string(), serde_json::json!(true));
        Self::append(data, &group, vec![forwarded]);
        Self::append(data, &design, vec![self.chat_message(format!("{}-plan", design), self.person(5), plan)]);
        Self::append(data, &general, vec![mention]);
        let mut files = self.chat_message(format!("{}-files", third_dm), self.person(people[2]), "Her er filene fra workshopen: opptaket, referatet, pitchen og pakken med ikoner.");
        files.attachments = [("workshop-opptak.mp4", "video/mp4"), ("referat-workshop.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document"), ("pitch-q4.pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation"), ("ikoner.zip", "application/zip"), ("budsjett.xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")]
            .iter()
            .enumerate()
            .map(|(i, (name, mime))| ResponseAttachment { external_id: format!("{}-files-a{}", third_dm, i), name: name.to_string(), content_type: mime.to_string(), size: 0, url: None, content_id: None })
            .collect();
        Self::append(data, &third_dm, vec![files]);
        let bot = ResponseUser { id: format!("mock-{}-bot-workflows", key), display_name: Some("Workflows".to_string()), ..Default::default() };
        let mut digest = self.chat_message(format!("{}-digest", dev), bot, "📋 Daglig sammendrag for #dev: 3 nye saker, 1 lukket, 2 PR-er venter på review.");
        digest.r#type = MessageType::Bot;
        Self::append(data, &dev, vec![digest]);
    }

    fn history(&self, data: &mut [(ResponseConversation, Vec<ResponseMessage>)]) {
        const LINES: [&str; 16] = [
            "Kan vi ta dette på standupen i morgen?",
            "Sender over utkastet nå.",
            "Takk, ser fint ut!",
            "Jeg sjekker med kunden og kommer tilbake.",
            "Har du tid til en rask prat etter lunsj?",
            "Bygget feilet igjen på main, ser på det.",
            "Notatene fra møtet ligger i mappa.",
            "Flott, da kjører vi på.",
            "Hvem tar demoen på fredag?",
            "Oppdatert estimatet til tre dager.",
            "Fikk du svar fra leverandøren?",
            "Jeg er på hjemmekontor i dag.",
            "Lagt inn i kalenderen.",
            "Kan du se over PR-en min?",
            "Godt jobbet med releasen!",
            "Skal vi flytte møtet til torsdag?",
        ];
        let mut rng = Lcg(self.kind.seed() * 7919);
        let base = self.base_time();
        let me = self.me();
        for (conversation, messages) in data.iter_mut() {
            let template = messages.first().cloned();
            let count = 6 + rng.below(14) as usize;
            let mut older = Vec::with_capacity(count);
            for i in 0..count {
                let days = 12 + rng.below(77) as i64;
                let at = base - Duration::days(days) + Duration::hours(rng.below(10) as i64 - 4) + Duration::minutes(rng.below(60) as i64);
                let sender = if rng.below(4) == 0 || conversation.members.is_empty() { me.clone() } else { conversation.members[rng.below(conversation.members.len() as u64) as usize].clone() };
                let mut message = self.chat_message(format!("{}-h{}", conversation.external_id, i), sender, LINES[rng.below(LINES.len() as u64) as usize]);
                message.conversation_external_id = conversation.external_id.clone();
                message.timestamp = at;
                if let Some(template) = &template {
                    message.r#type = template.r#type.clone();
                    if template.subject.is_some() {
                        message.subject = template.subject.clone().map(|s| if s.starts_with("Re: ") { s } else { format!("Re: {}", s) });
                    }
                    if template.headers.is_some() {
                        let (headers, origin) = mail_origin(i, at, message.sender.email.as_deref().unwrap_or("someone@example.org"));
                        message.headers = Some(headers);
                        message.origin = Some(origin);
                    }
                }
                older.push(message);
            }
            older.sort_by_key(|m| m.timestamp);
            older.append(messages);
            *messages = older;
        }
    }

    pub(super) fn dataset(&self) -> Vec<(ResponseConversation, Vec<ResponseMessage>)> {
        let mut data = if self.kind.is_mail() { self.mail_conversations() } else { self.chat_conversations() };
        self.vary(&mut data);
        self.history(&mut data);
        if self.kind.is_mail() {
            data.extend(self.fixture_mail());
            data.extend(self.mail_threads());
        }
        self.extras(&mut data);
        let ceiling = Utc::now() - Duration::minutes(3);
        for (conversation, messages) in data.iter_mut() {
            let Some(latest) = messages.iter().map(|m| m.timestamp).max() else { continue };
            if latest > ceiling {
                let shift = latest - ceiling;
                for message in messages.iter_mut() {
                    message.timestamp -= shift;
                    for key in ["edited_at", "deleted_at"] {
                        if let Some(at) = message.metadata.get(key).and_then(|v| v.as_str()).and_then(|v| DateTime::parse_from_rfc3339(v).ok()) {
                            message.metadata.insert(key.to_string(), serde_json::json!((at.with_timezone(&Utc) - shift).to_rfc3339()));
                        }
                    }
                }
            }
            conversation.last_activity = messages.iter().map(|m| m.timestamp).max().unwrap_or(conversation.last_activity);
        }
        for (_, messages) in data.iter_mut() {
            for message in messages.iter_mut() {
                for attachment in message.attachments.iter_mut().filter(|a| a.url.as_deref().is_none_or(|u| u.starts_with("http"))) {
                    let (url, mime, size) = local_file(&attachment.name, &attachment.content_type);
                    attachment.url = Some(url);
                    attachment.content_type = mime;
                    attachment.size = size;
                }
                if message.r#type == MessageType::Email && !message.metadata.contains_key("internet_message_id") {
                    let id = format!("<{}@mock.chatnip>", message.external_id);
                    message.headers = Some(match message.headers.take() {
                        Some(headers) => format!("{}\r\nMessage-ID: {}", headers, id),
                        None => format!("Message-ID: {}", id),
                    });
                    message.metadata.insert("internet_message_id".to_string(), serde_json::json!(id));
                }
            }
        }
        data
    }
}

#[async_trait]
impl ProviderTrait for MockProvider {
    fn get_provider(&self) -> &Provider {
        &self.provider
    }

    fn connects_with_token(&self) -> bool {
        true
    }

    async fn get_auth_url(&self, _state: &str, _code_challenge: Option<&str>) -> Result<String, String> {
        Err("Mock clients connect from settings".to_string())
    }

    async fn exchange_code(&self, _code: &str, _code_verifier: Option<&str>) -> Result<ProviderAuth, String> {
        Err("Mock clients connect from settings".to_string())
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<ProviderAuth, String> {
        let access_token = refresh_token.strip_suffix("-refresh").unwrap_or(refresh_token).to_string();
        let auth = ProviderAuth { access_token, refresh_token: Some(refresh_token.to_string()), expires_at: Utc::now() + Duration::days(36500), token_type: "Mock".to_string(), scope: None };
        if Server::open(self, &auth).await?.state().await? == server::STATE_EXPIRED {
            return Err("Token refresh failed: 400 Bad Request - {\"error\":\"invalid_grant\",\"error_description\":\"The refresh token has expired.\"}".to_string());
        }
        Ok(auth)
    }

    async fn validate_tokens(&self, auth: &ProviderAuth) -> Result<bool, String> {
        Ok(Server::open(self, auth).await?.state().await? != server::STATE_EXPIRED)
    }

    async fn identity(&self, _auth: &ProviderAuth) -> Result<ConnectionIdentity, String> {
        Ok(ConnectionIdentity {
            namespace: self.kind.key().to_string(),
            name: self.kind.label().to_string(),
            external_id: Some(self.me().id),
            user: Some(self.me()),
            logo_url: None,
        })
    }

    async fn sync_conversations(&self, auth: &ProviderAuth, since: Option<DateTime<Utc>>) -> Result<Vec<ResponseConversation>, String> {
        Server::open(self, auth).await?.get_conversations(since).await
    }

    async fn sync_messages(&self, auth: &ProviderAuth, conversation_external_id: &str, since: Option<DateTime<Utc>>, limit: Option<u32>) -> Result<Vec<ResponseMessage>, String> {
        Server::open(self, auth).await?.get_messages(conversation_external_id, since, limit).await
    }

    async fn send_message(&self, auth: &ProviderAuth, conversation_external_id: &str, draft: &OutgoingMessage) -> Result<ResponseMessage, String> {
        Server::open(self, auth).await?.post_message(conversation_external_id, draft).await
    }

    async fn mark_read(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str, read: bool) -> Result<(), String> {
        Server::open(self, auth).await?.put_read(message_external_id, read).await
    }

    async fn edit_message(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str, content: &str) -> Result<(), String> {
        Server::open(self, auth).await?.put_content(message_external_id, content).await
    }

    async fn send_reaction(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str, reaction: &str, active: bool) -> Result<(), String> {
        Server::open(self, auth).await?.put_reaction(message_external_id, reaction, active).await
    }

    async fn delete_message(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str) -> Result<(), String> {
        Server::open(self, auth).await?.delete_message(message_external_id).await
    }

    async fn get_custom_emoji(&self, auth: &ProviderAuth) -> Result<Vec<crate::providers::traits::RemoteEmoji>, String> {
        Server::open(self, auth).await?.get_emoji().await
    }

    async fn get_presence(&self, auth: &ProviderAuth, user_id: &str) -> Result<String, String> {
        Server::open(self, auth).await?.get_presence(user_id).await
    }

    async fn get_calendars(&self, auth: &ProviderAuth) -> Result<Vec<RemoteCalendar>, String> {
        Server::open(self, auth).await?.get_calendars().await
    }

    async fn get_calendar_events(&self, auth: &ProviderAuth, calendar_external_id: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<RemoteEvent>, String> {
        Server::open(self, auth).await?.get_events(calendar_external_id, from, to).await
    }

    async fn post_calendar_event(&self, auth: &ProviderAuth, calendar_external_id: &str, event: &NewRemoteEvent) -> Result<RemoteEvent, String> {
        Server::open(self, auth).await?.post_event(calendar_external_id, event).await
    }

    async fn put_calendar_response(&self, auth: &ProviderAuth, _calendar_external_id: &str, event_external_id: &str, response: &str) -> Result<RemoteEvent, String> {
        Server::open(self, auth).await?.put_response(event_external_id, response).await
    }

    async fn put_member(&self, auth: &ProviderAuth, conversation_external_id: &str, user_external_id: &str, member: bool) -> Result<bool, String> {
        Server::open(self, auth).await?.put_member(conversation_external_id, user_external_id, member).await
    }

    async fn put_conversation_name(&self, auth: &ProviderAuth, conversation_external_id: &str, name: &str) -> Result<bool, String> {
        Server::open(self, auth).await?.put_name(conversation_external_id, name).await
    }

    async fn put_pin(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str, pinned: bool) -> Result<bool, String> {
        Server::open(self, auth).await?.put_pin(message_external_id, pinned).await
    }

    async fn get_live(&self, auth: &ProviderAuth, sink: crate::providers::traits::LiveSink) -> Result<(), String> {
        super::server::get_live(self, auth, sink).await
    }

    async fn get_search(&self, auth: &ProviderAuth, search: &crate::providers::traits::RemoteSearch) -> Result<Vec<ResponseMessage>, String> {
        Server::open(self, auth).await?.get_search(search).await
    }

    async fn put_remote_draft(&self, auth: &ProviderAuth, _conversation_external_id: &str, draft: &OutgoingMessage, existing_external_id: Option<&str>) -> Result<Option<String>, String> {
        if !self.kind.is_mail() {
            return Ok(None);
        }
        Server::open(self, auth).await?.put_draft(draft, existing_external_id).await
    }

    async fn delete_remote_draft(&self, auth: &ProviderAuth, draft_external_id: &str) -> Result<bool, String> {
        Server::open(self, auth).await?.delete_draft(draft_external_id).await
    }

    async fn put_flag(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str, flagged: bool) -> Result<bool, String> {
        if !self.kind.is_mail() {
            return Ok(false);
        }
        Server::open(self, auth).await?.put_message_flag(message_external_id, "flagged", flagged).await
    }

    async fn put_archive(&self, auth: &ProviderAuth, _conversation_external_id: &str, message_external_id: &str, archived: bool) -> Result<bool, String> {
        if !self.kind.is_mail() {
            return Ok(false);
        }
        Server::open(self, auth).await?.put_message_flag(message_external_id, "archived", archived).await
    }

    async fn post_conversation(&self, auth: &ProviderAuth, conversation: &crate::providers::traits::NewRemoteConversation) -> Result<Option<ResponseConversation>, String> {
        if self.kind.is_mail() {
            return Ok(None);
        }
        Server::open(self, auth).await?.post_conversation(conversation).await
    }

    async fn put_membership(&self, auth: &ProviderAuth, conversation_external_id: &str, joined: bool) -> Result<bool, String> {
        if !self.provider.capabilities.supports_channel_join && joined {
            return Ok(false);
        }
        Server::open(self, auth).await?.put_membership(conversation_external_id, joined).await
    }

    async fn put_conversation_state(&self, auth: &ProviderAuth, conversation_external_id: &str, archived: bool) -> Result<bool, String> {
        if !self.provider.capabilities.supports_channel_archive {
            return Ok(false);
        }
        Server::open(self, auth).await?.put_state(conversation_external_id, archived).await
    }

    async fn put_conversation_topic(&self, auth: &ProviderAuth, conversation_external_id: &str, topic: Option<&str>, purpose: Option<&str>) -> Result<bool, String> {
        let capabilities = &self.provider.capabilities;
        let topic = topic.filter(|_| capabilities.supports_topic);
        let purpose = purpose.filter(|_| capabilities.supports_purpose);
        if topic.is_none() && purpose.is_none() {
            return Ok(false);
        }
        Server::open(self, auth).await?.put_topic(conversation_external_id, topic, purpose).await
    }

    async fn get_joinable_conversations(&self, auth: &ProviderAuth) -> Result<Vec<ResponseConversation>, String> {
        if !self.provider.capabilities.supports_channel_join {
            return Ok(Vec::new());
        }
        Server::open(self, auth).await?.get_joinable().await
    }

    async fn get_user_groups(&self, _auth: &ProviderAuth) -> Result<Vec<crate::providers::traits::RemoteUserGroup>, String> {
        if !self.provider.capabilities.supports_user_groups {
            return Ok(Vec::new());
        }
        Ok(vec![crate::providers::traits::RemoteUserGroup { external_id: "S0DESIGN".to_string(), handle: "design".to_string(), name: "Design".to_string(), members: Vec::new() }])
    }

    async fn get_aliases(&self, auth: &ProviderAuth) -> Result<Vec<crate::providers::traits::RemoteAlias>, String> {
        if !self.kind.is_mail() {
            return Ok(Vec::new());
        }
        Server::open(self, auth).await?.get_aliases().await
    }

    async fn put_signature(&self, auth: &ProviderAuth, _alias: &str, html: &str) -> Result<bool, String> {
        if !self.provider.capabilities.signature_sync {
            return Ok(false);
        }
        Server::open(self, auth).await?.put_signature(html).await
    }

    async fn get_auto_reply(&self, auth: &ProviderAuth) -> Result<Option<crate::providers::traits::RemoteAutoReply>, String> {
        if !self.provider.capabilities.supports_auto_reply {
            return Ok(None);
        }
        Server::open(self, auth).await?.get_auto_reply().await
    }

    async fn put_auto_reply(&self, auth: &ProviderAuth, reply: &crate::providers::traits::RemoteAutoReply) -> Result<bool, String> {
        if !self.provider.capabilities.supports_auto_reply {
            return Ok(false);
        }
        Server::open(self, auth).await?.put_auto_reply(reply).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth() -> ProviderAuth {
        ProviderAuth { access_token: "mock".to_string(), refresh_token: None, expires_at: Utc::now(), token_type: "Mock".to_string(), scope: None }
    }

    #[tokio::test]
    async fn chat_kinds_have_dms_groups_and_channels() {
        for kind in [MockKind::Teams, MockKind::Slack, MockKind::GoogleChat] {
            let provider = MockProvider::new(kind, crate::test_helpers::test_pool().await);
            let conversations = provider.sync_conversations(&auth(), None).await.unwrap();
            assert_eq!(conversations.iter().filter(|c| c.conversation_type == ConversationType::DirectMessage).count(), 6, "{:?}", kind);
            assert_eq!(conversations.iter().filter(|c| c.conversation_type == ConversationType::GroupMessage).count(), 4, "{:?}", kind);
            assert_eq!(conversations.iter().filter(|c| c.conversation_type == ConversationType::Channel).count(), 3, "{:?}", kind);
            let first = &conversations[0];
            let messages = provider.sync_messages(&auth(), &first.external_id, None, None).await.unwrap();
            assert!(messages.len() >= 4);
            assert!(messages.windows(2).all(|w| w[0].timestamp >= w[1].timestamp));
        }
    }

    #[tokio::test]
    async fn mail_kinds_carry_generated_and_fixture_threads() {
        for kind in [MockKind::Outlook, MockKind::Gmail, MockKind::Imap] {
            let provider = MockProvider::new(kind, crate::test_helpers::test_pool().await);
            let conversations = provider.sync_conversations(&auth(), None).await.unwrap();
            assert_eq!(conversations.len(), match kind { MockKind::Imap => 11, MockKind::Outlook => 14, _ => 15 }, "{:?}", kind);
            assert!(conversations.iter().all(|c| c.conversation_type == ConversationType::Email));
            let services = conversations.iter().filter(|c| c.members.iter().any(|m| m.email.as_deref().map(|e| e.contains("github") || e.contains("newsletter") || e.contains("calendar-notification")).unwrap_or(false))).count();
            assert_eq!(services, if kind == MockKind::Gmail { 5 } else { 3 }, "{:?}", kind);
        }
    }

    #[tokio::test]
    async fn mail_carries_headers_and_an_origin() {
        let provider = MockProvider::new(MockKind::Outlook, crate::test_helpers::test_pool().await);
        let conversations = provider.sync_conversations(&auth(), None).await.unwrap();
        for conversation in conversations {
            let messages = provider.sync_messages(&auth(), &conversation.external_id, None, None).await.unwrap();
            for message in messages.iter().filter(|m| m.sender.id != provider.me().id) {
                let headers = message.headers.as_deref().unwrap();
                assert!(headers.contains("Received:") && headers.contains("Message-ID: <"), "{}", message.external_id);
                let origin = message.origin.as_ref().unwrap();
                assert!(origin.lat.is_some() && origin.city.is_some());
                assert_eq!(crate::services::analytics_service::origin_ip_from_headers(headers).map(|ip| ip.to_string()), origin.ip);
                assert!(message.metadata["internet_message_id"].as_str().unwrap().ends_with("@mock.chatnip>"));
            }
        }
    }

    async fn connect(pool: &sqlx::Pool<sqlx::Sqlite>, profile: &str, kind: MockKind) -> Provider {
        let id = crate::test_helpers::test_provider(pool, profile, "mock").await;
        sqlx::query("UPDATE provider SET external_id = ?1, namespace = ?2, config = ?2, access_token = 'mock', expires_at = ?3 WHERE id = ?4")
            .bind(format!("mock-{}-me", kind.key()))
            .bind(kind.key())
            .bind((Utc::now() + Duration::days(365)).to_rfc3339())
            .bind(&id)
            .execute(pool)
            .await
            .unwrap();
        Provider::load_from_db(pool, &id).await.unwrap()
    }

    #[tokio::test]
    async fn ingest_through_the_sync_path_lands_messages_members_and_people() {
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        for kind in [MockKind::Teams, MockKind::Outlook, MockKind::Slack, MockKind::Gmail] {
            let provider = connect(&pool, &profile, kind).await;
            provider.ingest(&pool, None, std::sync::Arc::new(MockProvider::new(kind, pool.clone()))).await.unwrap();
        }
        let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message").fetch_one(&pool).await.unwrap();
        let members: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM conversation_member").fetch_one(&pool).await.unwrap();
        let origins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE origin_lat IS NOT NULL").fetch_one(&pool).await.unwrap();
        assert!(messages > 100, "messages {}", messages);
        assert!(members > 40, "members {}", members);
        assert!(origins >= 12, "origins {}", origins);
        let persons = crate::services::people_service::rematch(&pool, &profile).await.unwrap();
        assert!(persons > 0);
        let people = crate::models::Person::all(&pool, &profile).await.unwrap();
        let eva = people.iter().find(|p| p.primary_email.as_deref() == Some("eva.ortega@northwind.example")).unwrap();
        assert!(eva.identities.len() >= 2, "{:?}", eva.identities);
        assert!(eva.is_human);
        let github = people.iter().find(|p| p.primary_email.as_deref() == Some("notifications@github.com")).unwrap();
        assert!(!github.is_human);
        let linked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM conversation WHERE person_id IS NOT NULL").fetch_one(&pool).await.unwrap();
        assert!(linked > 0);
    }

    #[tokio::test]
    async fn each_mail_kind_has_its_own_threads() {
        let outlook = MockProvider::new(MockKind::Outlook, crate::test_helpers::test_pool().await).sync_conversations(&auth(), None).await.unwrap();
        let gmail = MockProvider::new(MockKind::Gmail, crate::test_helpers::test_pool().await).sync_conversations(&auth(), None).await.unwrap();
        let imap = MockProvider::new(MockKind::Imap, crate::test_helpers::test_pool().await).sync_conversations(&auth(), None).await.unwrap();
        let names = |list: &Vec<ResponseConversation>| list.iter().map(|c| c.name.clone()).filter(|n| n != "Kontrakt Kunde AS").collect::<std::collections::HashSet<_>>();
        assert!(names(&outlook).is_disjoint(&names(&gmail)));
        assert!(names(&gmail).is_disjoint(&names(&imap)));
        assert!(names(&outlook).is_disjoint(&names(&imap)));
    }

    #[tokio::test]
    async fn contract_thread_spans_outlook_and_gmail_with_the_pdf_arriving_later() {
        let mut first = None;
        let mut pdf = None;
        for kind in [MockKind::Outlook, MockKind::Gmail] {
            let provider = MockProvider::new(kind, crate::test_helpers::test_pool().await);
            let conversations = provider.sync_conversations(&auth(), None).await.unwrap();
            let thread = conversations.iter().find(|c| c.name == "Kontrakt Kunde AS").unwrap();
            let messages = provider.sync_messages(&auth(), &thread.external_id, None, None).await.unwrap();
            if kind == MockKind::Outlook {
                assert!(messages.iter().all(|m| m.attachments.is_empty()));
                first = messages.last().map(|m| m.timestamp);
            } else {
                let carrier = messages.iter().find(|m| m.attachments.iter().any(|a| a.name == "contract.pdf" && a.content_type == "application/pdf")).unwrap();
                assert_eq!(carrier.sender.email.as_deref(), Some("anna.berg@client.example"));
                pdf = Some(carrier.timestamp);
            }
        }
        assert!(pdf.unwrap() > first.unwrap());
    }

    #[tokio::test]
    async fn the_cast_is_shared_across_kinds() {
        let teams = MockProvider::new(MockKind::Teams, crate::test_helpers::test_pool().await);
        let slack = MockProvider::new(MockKind::Slack, crate::test_helpers::test_pool().await);
        let a = teams.sync_conversations(&auth(), None).await.unwrap();
        let b = slack.sync_conversations(&auth(), None).await.unwrap();
        let emails = |list: &Vec<ResponseConversation>| -> Vec<String> {
            list.iter().flat_map(|c| c.members.iter().filter_map(|m| m.email.clone())).collect()
        };
        let shared: Vec<String> = emails(&a).into_iter().filter(|e| emails(&b).contains(e) && e != "milo@northwind.example").collect();
        assert!(!shared.is_empty());
    }


    #[tokio::test]
    async fn every_content_kind_is_emitted_across_the_clients() {
        let mut all: Vec<(MockKind, ResponseMessage)> = Vec::new();
        for kind in MockKind::ALL {
            let provider = MockProvider::new(kind, crate::test_helpers::test_pool().await);
            for (_, messages) in provider.dataset() {
                all.extend(messages.into_iter().map(|m| (kind, m)));
            }
        }
        let structure = |m: &ResponseMessage| m.metadata.get("structure").cloned().unwrap_or_default();
        let any = |f: &dyn Fn(&MockKind, &ResponseMessage, &serde_json::Value) -> bool| all.iter().any(|(k, m)| f(k, m, &structure(m)));
        let missing: Vec<&str> = all.iter().filter(|(_, m)| !m.metadata.contains_key("structure")).map(|(_, m)| m.external_id.as_str()).collect();
        assert!(missing.is_empty(), "{:?}", missing);
        for style in ["gmail", "outlook", "apple", "text"] {
            assert!(any(&|_, _, s| s["quote_style"] == style), "{}", style);
        }
        assert!(any(&|_, m, s| m.content_type == "text" && s["signature"].as_str().map(|x| x.contains("Designer")).unwrap_or(false)));
        assert!(any(&|_, _, s| s["is_bulk"] == true && s["columns"] == 2 && s["preheader"].is_string() && s["images"].as_array().unwrap().iter().any(|i| i["kind"] == "tracking")));
        assert!(any(&|_, m, s| s["invites"][0]["title"] == "Designreview" && m.metadata.get("event").map(|e| e["starts_at"] == "2026-10-08T11:00:00+00:00").unwrap_or(false)));
        assert!(any(&|_, m, _| m.attachments.len() >= 4 && m.attachments.iter().map(|a| a.content_type.as_str()).collect::<std::collections::HashSet<_>>().len() >= 4 && m.attachments.iter().any(|a| a.content_id.as_deref() == Some("logo@client.example") && a.url.as_deref().map(|u| u.starts_with("data:image/png;base64,")).unwrap_or(false))));
        assert!(any(&|k, m, s| *k == MockKind::Teams && m.content_type == "html" && s["cards"][0]["kind"] == "adaptive" && s["mentions"].as_array().unwrap().len() == 2 && s["code"][0]["language"] == "Bash" && m.reply_to_id.is_some()));
        assert!(any(&|k, m, _| *k == MockKind::Teams && m.metadata.get("reactions").and_then(|r| r.as_array()).map(|r| r.len() == 3 && r[0]["emoji"] == ":+1:").unwrap_or(false)));
        assert!(any(&|k, _, s| *k == MockKind::Slack && s["lists"].as_array().unwrap().len() == 2 && s["quotes"].as_array().unwrap().len() == 1));
        assert!(any(&|k, m, s| *k == MockKind::Slack && m.r#type == MessageType::Bot && s["is_bot"] == true && s["cards"].as_array().unwrap().iter().any(|c| c["kind"] == "attachment")));
        assert!(any(&|k, m, _| *k == MockKind::Slack && m.attachments.iter().any(|a| a.name == "sync.log")));
        assert!(any(&|k, m, s| *k == MockKind::Slack && m.thread_id.is_some() && s["mentions"].is_array() && m.content == "On it :+1:"));
        assert!(any(&|k, _, s| *k == MockKind::GoogleChat && s["commands"][0] == "/standup" && s["cards"][0]["kind"] == "card_v2"));
        for kind in [MockKind::Teams, MockKind::Slack, MockKind::GoogleChat] {
            assert!(any(&|k, _, s| *k == kind && s["emoji_only"] == true), "{:?}", kind);
            assert!(any(&|k, m, _| *k == kind && crate::content::text::direction(&m.content) == crate::content::text::Direction::Mixed), "{:?}", kind);
            assert!(any(&|k, _, s| *k == kind && s["code"][0]["language"] == "rust" && s["lists"].as_array().unwrap().len() == 2 && s["mentions"][0]["name"] == "Eva"), "{:?}", kind);
            assert!(any(&|k, m, _| *k == kind && m.content.chars().count() >= 10_000 && m.content.lines().count() == 200), "{:?}", kind);
        }
    }
    #[tokio::test]
    async fn every_mock_mail_and_chat_kind_exists() {
        let pool = crate::test_helpers::test_pool().await;
        let data = |kind: MockKind| -> HashMap<String, Vec<ResponseMessage>> {
            MockProvider::new(kind, pool.clone())
                .dataset()
                .into_iter()
                .map(|(c, mut m)| {
                    m.sort_by_key(|x| x.timestamp);
                    (c.external_id, m)
                })
                .collect()
        };
        let outlook = data(MockKind::Outlook);
        let gmail = data(MockKind::Gmail);
        let imap = data(MockKind::Imap);
        let first = |set: &HashMap<String, Vec<ResponseMessage>>, id: &str| set.get(id).and_then(|m| m.first().cloned()).unwrap_or_else(|| panic!("missing {}", id));
        let structure = |m: &ResponseMessage| m.metadata.get("structure").cloned().unwrap_or_default();
        let images = |m: &ResponseMessage, kind: &str| structure(m)["images"].as_array().map(|a| a.iter().filter(|i| i["kind"] == kind).count()).unwrap_or(0);

        let tldr = first(&gmail, "mock-gmail-mail-tldr");
        assert!(structure(&tldr)["is_bulk"] == true && structure(&tldr)["preheader"].is_string() && images(&tldr, "tracking") >= 1);
        assert!(!structure(&tldr)["unsubscribe"].as_array().unwrap().is_empty());

        let retailer = first(&outlook, "mock-outlook-mail-retailer");
        assert!(structure(&retailer)["table_depth"].as_u64().unwrap() >= 3 && structure(&retailer)["columns"].as_u64().unwrap() >= 2);
        assert!(images(&retailer, "remote") >= 4 && retailer.content.contains("Meld deg av") && retailer.content.contains("bgcolor=\"#2b241c\""));

        let receipt = first(&imap, "mock-imap-mail-receipt");
        assert!(receipt.content.contains("Totalt") && receipt.content.contains("<tfoot>") && structure(&receipt)["columns"].as_u64().unwrap() >= 4);

        let github = first(&gmail, "mock-gmail-mail-github");
        assert_eq!(github.sender.email.as_deref(), Some("notifications@github.com"));
        assert!(github.content.contains("@@ -41,9 +41,14 @@") && github.content.contains("#dafbe1") && github.content.contains("language-diff"));

        let linear = first(&outlook, "mock-outlook-mail-linear");
        assert!(linear.subject.as_deref().unwrap().starts_with("[NW-142]") && linear.content.contains("In Progress"));

        let invite = first(&outlook, "mock-outlook-mail-sprintdemo");
        assert_eq!(invite.content_type, "html");
        assert!(invite.content.contains("Teams-møte") && invite.metadata["event"]["title"] == "Sprintdemo" && invite.metadata["event"]["starts_at"] == "2026-10-16T12:00:00+00:00");

        let agreement = outlook.get("mock-outlook-mail-agreement").unwrap();
        assert_eq!(agreement.len(), 8);
        let last = agreement.last().unwrap();
        assert_eq!(last.content.matches("id=\"divRplyFwdMsg\"").count(), 7);
        assert_eq!(structure(last)["quote_style"], "outlook");
        assert!(structure(last)["signature"].as_str().unwrap().contains("Kunde AS"));
        assert!(agreement.iter().any(|m| m.sender.email.as_deref() == Some("milo@northwind.example")) && agreement.windows(2).all(|w| w[1].reply_to_id.as_deref() == Some(w[0].external_id.as_str())));

        let onboarding = gmail.get("mock-gmail-mail-onboarding").unwrap();
        assert_eq!(onboarding.len(), 4);
        let deepest = onboarding.last().unwrap();
        assert_eq!(deepest.content.matches("<blockquote class=\"gmail_quote\"").count(), 3);
        assert_eq!(structure(deepest)["quote_style"], "gmail");
        assert!(deepest.content.contains("wrote:"));

        let apple = first(&imap, "mock-imap-mail-icons");
        assert_eq!(structure(&apple)["quote_style"], "apple");
        assert!(apple.content.contains("<blockquote type=\"cite\">"));

        let status = first(&imap, "mock-imap-mail-status");
        let inline: Vec<&ResponseAttachment> = status.attachments.iter().filter(|a| a.content_id.is_some()).collect();
        assert_eq!(inline.len(), 2);
        assert!(inline.iter().all(|a| a.url.as_deref().unwrap().starts_with("data:image/png;base64,")) && status.content.contains("cid:chart-uke41@tidewater.example"));
        let files: std::collections::HashSet<&str> = status.attachments.iter().filter(|a| a.content_id.is_none()).map(|a| a.content_type.as_str()).collect();
        assert_eq!(files, ["application/pdf", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", "image/png", "application/zip"].into_iter().collect());

        let pricing = first(&imap, "mock-imap-mail-pricing");
        assert_eq!(pricing.content_type, "text");
        assert_eq!(structure(&pricing)["quote_style"], "text");
        assert!(structure(&pricing)["signature"].as_str().unwrap().contains("Hugo"));
        let quoted: Vec<&str> = pricing.content.lines().filter(|l| l.starts_with('>')).collect();
        assert!(quoted.len() >= 8 && quoted.iter().all(|l| l.chars().count() <= 80));

        let strategy = first(&gmail, "mock-gmail-mail-strategy");
        assert!(structure(&strategy)["text"].as_str().unwrap().split_whitespace().count() >= 3000);
        assert!(strategy.content.matches("<h2>").count() >= 8 && strategy.content.contains("<ul>") && strategy.content.contains("<ol>") && strategy.content.matches("<table").count() >= 3);

        let summit = first(&gmail, "mock-gmail-mail-summit");
        assert!(images(&summit, "remote") >= 4 && images(&summit, "tracking") >= 2);

        let payslip = first(&outlook, "mock-outlook-mail-payslip");
        assert!(payslip.content.contains("light only") && payslip.content.contains("bgcolor=\"#ffffff\"") && payslip.content.contains("<font color=\"#000000\""));

        for kind in [MockKind::Teams, MockKind::Slack, MockKind::GoogleChat] {
            let chats = data(kind);
            let key = kind.key();
            let me = format!("mock-{}-me", key);
            let general = chats.get(&format!("mock-{}-channel-0", key)).unwrap();
            let longest = general.iter().fold((0, 0), |(best, run), m| if m.sender.id != me && m.r#type == MessageType::Chat { (best.max(run + 1), run + 1) } else { (best, 0) }).0;
            assert!(longest >= 20, "{:?} run {}", kind, longest);
            let all: Vec<&ResponseMessage> = chats.values().flatten().collect();
            assert!(all.iter().any(|m| structure(m)["links"].as_array().map(|l| l.len() >= 3).unwrap_or(false)), "{:?} links", kind);
            assert!(all.iter().any(|m| m.content.is_empty() && m.attachments.iter().any(|a| a.content_type == "image/gif" && a.url.is_some())), "{:?} gif", kind);
            assert!(all.iter().any(|m| m.content.is_empty() && m.attachments.iter().any(|a| a.content_type == "image/png")), "{:?} image", kind);
            assert!(all.iter().any(|m| structure(m)["emoji_only"] == true), "{:?} emoji", kind);
            let group = chats.get(&format!("mock-{}-group-0", key)).unwrap();
            assert!(group.iter().any(|m| m.r#type == MessageType::System && m.content.ends_with("joined the conversation")), "{:?} joined", kind);
            assert!(group.iter().any(|m| m.r#type == MessageType::System && m.content.ends_with("left the conversation")), "{:?} left", kind);
        }

        let teams = data(MockKind::Teams);
        let group = teams.get("mock-teams-group-0").unwrap();
        let ids: Vec<&str> = group.iter().map(|m| m.external_id.as_str()).collect();
        assert!(group.iter().any(|m| m.content.contains("copy-paste-table") && m.content.contains("<th>Status</th>")));
        assert!(group.iter().any(|m| m.content.contains("data:image/png;base64,") && structure(m)["images"][0].as_str().map(|s| s.contains("hostedContents")).unwrap_or(false)));
        assert!(group.iter().any(|m| structure(m)["mentions"].as_array().map(|a| a.len() == 2).unwrap_or(false) && m.content.contains("<at id=\"0\">")));
        assert!(group.iter().any(|m| m.content.contains("class=\"reply-quote\"") && m.reply_to_id.as_deref().map(|r| ids.contains(&r)).unwrap_or(false)));
        assert!(group.iter().any(|m| m.metadata.get("edited_at").and_then(|v| v.as_str()).and_then(|v| DateTime::parse_from_rfc3339(v).ok()).map(|v| v.with_timezone(&Utc) > m.timestamp).unwrap_or(false)));
        assert!(group.iter().any(|m| m.metadata.contains_key("deleted_at") && m.content.is_empty()));

        let slack = data(MockKind::Slack);
        let dev = slack.get("mock-slack-channel-1").unwrap();
        let parent = dev.iter().find(|m| m.content.starts_with("Release 0.5 is cut")).unwrap();
        assert!(structure(parent)["lists"][0].as_array().unwrap().len() == 3 && structure(parent)["quotes"].as_array().unwrap().len() == 1 && !structure(parent)["code"].as_array().unwrap().is_empty());
        assert_eq!(dev.iter().filter(|m| m.thread_id.as_deref() == Some(parent.external_id.as_str())).count(), 6);
        assert!(dev.iter().any(|m| m.r#type == MessageType::System && m.content.contains("huddle")));
        assert!(dev.iter().any(|m| m.attachments.len() == 3 && m.attachments.iter().any(|a| a.content_type == "application/pdf")));
        assert!(dev.iter().any(|m| m.r#type == MessageType::Bot) || slack.values().flatten().any(|m| m.r#type == MessageType::Bot));

        let gchat: Vec<ResponseMessage> = data(MockKind::GoogleChat).into_values().flatten().collect();
        assert!(gchat.iter().any(|m| structure(m)["commands"][0] == "/standup" && structure(m)["cards"][0]["kind"] == "card_v2"));
    }

    #[tokio::test]
    async fn sent_messages_show_up_on_the_next_sync() {
        let provider = MockProvider::new(MockKind::GoogleChat, crate::test_helpers::test_pool().await);
        let conversations = provider.sync_conversations(&auth(), None).await.unwrap();
        let target = conversations[0].external_id.clone();
        let before = provider.sync_messages(&auth(), &target, None, None).await.unwrap().len();
        let seeded = Utc::now();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let draft = OutgoingMessage { content: "Hei fra testen".to_string(), content_type: "text".to_string(), ..Default::default() };
        let sent = provider.send_message(&auth(), &target, &draft).await.unwrap();
        assert!(!sent.external_id.starts_with("mock-"));
        let after = provider.sync_messages(&auth(), &target, None, None).await.unwrap();
        assert_eq!(after.len(), before + 1);
        assert_eq!(after.first().unwrap().content, "Hei fra testen");
        assert_eq!(after.first().unwrap().external_id, sent.external_id);
        let incremental = provider.sync_messages(&auth(), &target, Some(seeded), None).await.unwrap();
        assert_eq!(incremental.len(), 1);
        let again = MockProvider::new(MockKind::GoogleChat, provider.pool().clone());
        assert_eq!(again.sync_messages(&auth(), &target, Some(seeded), None).await.unwrap()[0].external_id, sent.external_id);
        let limited = provider.sync_messages(&auth(), &target, None, Some(2)).await.unwrap();
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].external_id, sent.external_id);
    }

    #[tokio::test]
    async fn every_mock_kind_declares_how_it_quotes_and_forwards_and_records_both() {
        use crate::providers::traits::OutgoingReference;
        for kind in MockKind::ALL {
            let caps = kind.provider().capabilities;
            assert!(caps.supports_reply_quote && caps.supports_forward);
            let expected = match kind {
                MockKind::Teams => (Delivery::Native, Delivery::Repost),
                MockKind::Slack => (Delivery::Native, Delivery::Native),
                MockKind::GoogleChat => (Delivery::Repost, Delivery::Repost),
                _ => (Delivery::Mail, Delivery::Mail),
            };
            assert_eq!((caps.reply_quote, caps.forward), expected, "{:?}", kind);
        }
        let provider = MockProvider::new(MockKind::Slack, crate::test_helpers::test_pool().await);
        let auth = ProviderAuth { access_token: "mock".into(), refresh_token: None, expires_at: Utc::now(), token_type: "Bearer".into(), scope: None };
        let quote = OutgoingReference { external_id: "q1".into(), sender_name: "Sam".into(), text: "Hm?".into(), native: true, ..Default::default() };
        let draft = OutgoingMessage { content: "Ja".into(), content_type: "text".into(), in_reply_to: Some("q1".into()), quote: Some(quote.clone()), forwarded: vec![quote], ..Default::default() };
        let sent = provider.send_message(&auth, "mock-slack-quote-test", &draft).await.unwrap();
        assert_eq!(sent.reply_to_id.as_deref(), Some("q1"));
        assert_eq!(sent.metadata["quote"]["sender"], "Sam");
        assert_eq!(sent.metadata["forwarded"][0]["id"], "q1");
        assert!(sent.thread_id.is_none());
    }

    async fn connected(kind: MockKind) -> (sqlx::Pool<sqlx::Sqlite>, String, Provider, MockProvider) {
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        crate::models::AppSetting::put(&pool, crate::models::app_setting::MOCK_ENABLED, "true").await.unwrap();
        let provider = connect(&pool, &profile, kind).await;
        provider.ingest(&pool, None, std::sync::Arc::new(MockProvider::new(kind, pool.clone()))).await.unwrap();
        (pool.clone(), profile, provider, MockProvider::new(kind, pool))
    }

    async fn conversation_of(pool: &sqlx::Pool<sqlx::Sqlite>, provider: &Provider, kind: &str) -> (String, String) {
        sqlx::query_as("SELECT c.id, cp.external_id FROM conversation c JOIN conversation_provider cp ON cp.conversation_id = c.id WHERE cp.provider_id = ?1 AND c.type = ?2 ORDER BY c.last_activity DESC, cp.external_id LIMIT 1")
            .bind(&provider.id)
            .bind(kind)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn local(pool: &sqlx::Pool<sqlx::Sqlite>, provider: &Provider, external_id: &str) -> Option<(String, String, Option<String>, String)> {
        sqlx::query_as("SELECT id, COALESCE(content, ''), deleted_datetime, COALESCE(metadata, '{}') FROM message WHERE provider_id = ?1 AND external_id = ?2")
            .bind(&provider.id)
            .bind(external_id)
            .fetch_optional(pool)
            .await
            .unwrap()
    }

    async fn resync(pool: &sqlx::Pool<sqlx::Sqlite>, provider: &Provider, kind: MockKind) {
        provider.ingest(pool, None, std::sync::Arc::new(MockProvider::new(kind, pool.clone()))).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn every_kind_round_trips_a_send_through_the_server_and_back_through_sync() {
        for kind in MockKind::ALL {
            let (pool, _, provider, mock) = connected(kind).await;
            let auth = provider.auth.clone().unwrap();
            let (conversation, external) = conversation_of(&pool, &provider, if kind.is_mail() { "email" } else { "direct_message" }).await;
            let draft = OutgoingMessage { content: format!("Rundtur via {}", kind.label()), content_type: "text".into(), subject: kind.is_mail().then(|| "Rundtur".to_string()), ..Default::default() };
            let sent = mock.send_message(&auth, &external, &draft).await.unwrap();
            assert!(local(&pool, &provider, &sent.external_id).await.is_none(), "{:?} wrote locally before the sync", kind);
            let stored = Server::open(&mock, &auth).await.unwrap().get_message(&sent.external_id).await.unwrap().unwrap();
            assert_eq!(stored.conversation_external_id, external);
            resync(&pool, &provider, kind).await;
            let (id, content, _, metadata) = local(&pool, &provider, &sent.external_id).await.unwrap_or_else(|| panic!("{:?} did not sync the send back", kind));
            assert!(content.contains("Rundtur"), "{:?}: {}", kind, content);
            assert_eq!(serde_json::from_str::<serde_json::Value>(&metadata).unwrap()["delivery"], "delivered");
            let owner: String = sqlx::query_scalar("SELECT conversation_id FROM message WHERE id = ?1").bind(&id).fetch_one(&pool).await.unwrap();
            assert_eq!(owner, conversation);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn edits_reactions_and_deletes_land_on_the_server_and_sync_back() {
        for kind in [MockKind::Teams, MockKind::Slack, MockKind::GoogleChat] {
            let (pool, _, provider, mock) = connected(kind).await;
            let auth = provider.auth.clone().unwrap();
            let (_, external) = conversation_of(&pool, &provider, "direct_message").await;
            let draft = OutgoingMessage { content: "Første utkast".into(), content_type: "text".into(), ..Default::default() };
            let sent = mock.send_message(&auth, &external, &draft).await.unwrap();
            mock.edit_message(&auth, &external, &sent.external_id, "Rettet tekst").await.unwrap();
            mock.send_reaction(&auth, &external, &sent.external_id, "🎉", true).await.unwrap();
            resync(&pool, &provider, kind).await;
            let (_, content, deleted, metadata) = local(&pool, &provider, &sent.external_id).await.unwrap();
            let metadata: serde_json::Value = serde_json::from_str(&metadata).unwrap();
            assert_eq!(content, "Rettet tekst");
            assert!(deleted.is_none());
            assert!(metadata["edited_at"].is_string(), "{:?}", metadata);
            assert!(metadata["reactions"].as_array().unwrap().iter().any(|r| r["emoji"] == ":tada:" && r["original"] == "🎉"), "{:?}", metadata);
            mock.send_reaction(&auth, &external, &sent.external_id, "🎉", false).await.unwrap();
            mock.delete_message(&auth, &external, &sent.external_id).await.unwrap();
            resync(&pool, &provider, kind).await;
            let (_, content, deleted, metadata) = local(&pool, &provider, &sent.external_id).await.unwrap();
            assert!(content.is_empty() && deleted.is_some(), "{:?}", kind);
            assert!(serde_json::from_str::<serde_json::Value>(&metadata).unwrap()["reactions"].as_array().unwrap().is_empty());
            let theirs: String = sqlx::query_scalar("SELECT external_id FROM message WHERE provider_id = ?1 AND sender_email != 'milo@northwind.example' AND external_id LIKE ?2 LIMIT 1").bind(&provider.id).bind(format!("{}-%", external)).fetch_one(&pool).await.unwrap();
            assert!(mock.edit_message(&auth, &external, &theirs, "x").await.unwrap_err().starts_with("403"));
            assert!(mock.edit_message(&auth, &external, "nothing-here", "x").await.unwrap_err().starts_with("404"));
        }
        let (_, _, provider, mock) = connected(MockKind::Outlook).await;
        let auth = provider.auth.clone().unwrap();
        assert_eq!(mock.send_reaction(&auth, "x", "y", ":+1:", true).await.unwrap_err(), "Mail has no reactions");
        assert!(mock.edit_message(&auth, "x", "y", "z").await.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn uploads_are_served_back_as_downloads_through_the_attachment_cache() {
        let (pool, _, provider, mock) = connected(MockKind::Slack).await;
        let auth = provider.auth.clone().unwrap();
        let (_, external) = conversation_of(&pool, &provider, "direct_message").await;
        let bytes = b"Plan for uke 43\n- seed\n- server\n".to_vec();
        let draft = OutgoingMessage { content: "Planen".into(), content_type: "text".into(), attachments: vec![crate::providers::traits::OutgoingAttachment { name: "plan.txt".into(), mime_type: "text/plain".into(), bytes: bytes.clone() }], ..Default::default() };
        let sent = mock.send_message(&auth, &external, &draft).await.unwrap();
        resync(&pool, &provider, MockKind::Slack).await;
        let (id, ..) = local(&pool, &provider, &sent.external_id).await.unwrap();
        let files = crate::services::attachment_service::attachments(&pool, &id).await.unwrap();
        assert_eq!(files.len(), 1);
        let dir = std::env::temp_dir().join(format!("chatnip-mock-files-{}", uuid::Uuid::new_v4()));
        let path = crate::services::attachment_service::cached(&pool, &dir, &dir.join("cache"), &id, &files[0]).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let seeded: Vec<(String, String)> = sqlx::query_as("SELECT id, attachments FROM message WHERE provider_id = ?1 AND has_attachments = 1").bind(&provider.id).fetch_all(&pool).await.unwrap();
        assert!(!seeded.is_empty());
        for (id, raw) in seeded {
            for file in crate::models::Message::parse_attachments(Some(&raw)) {
                assert!(file.url.as_deref().is_some_and(|u| u.starts_with("data:")), "{} would reach the network", file.name);
                assert!(crate::services::attachment_service::cached(&pool, &dir, &dir.join("cache"), &id, &file).await.is_ok(), "{}", file.name);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_reply_all_carries_the_threading_headers_and_drops_me_from_the_copy() {
        let (pool, _, provider, mock) = connected(MockKind::Outlook).await;
        let auth = provider.auth.clone().unwrap();
        let quoted: String = sqlx::query_scalar("SELECT m.id FROM message m JOIN conversation_provider cp ON cp.conversation_id = m.conversation_id WHERE cp.external_id = 'mock-outlook-mail-agreement' AND m.external_id = 'mock-outlook-mail-agreement-m7'").fetch_one(&pool).await.unwrap();
        let input = crate::services::outbound_service::PostMessageReplyInput { message_id: quoted, content: "Takk, da signerer vi.".into(), reply_all: true, ..Default::default() };
        let replied = crate::services::outbound_service::reply(&pool, &std::env::temp_dir(), input).await.unwrap();
        let stored = Server::open(&mock, &auth).await.unwrap().get_message(&replied.external_id).await.unwrap().unwrap();
        let headers = stored.headers.unwrap();
        assert!(headers.contains("In-Reply-To: <mock-outlook-mail-agreement-m7@mock.chatnip>"), "{}", headers);
        assert!(headers.contains("References: <mock-outlook-mail-agreement-m7@mock.chatnip>"));
        assert!(headers.contains("To: Anna Berg <anna.berg@client.example>"), "{}", headers);
        assert!(headers.contains("Cc: Jonas Lie <jonas.lie@client.example>") && !headers.lines().any(|l| l.starts_with("Cc:") && l.contains("milo@northwind.example")), "{}", headers);
        assert!(stored.subject.unwrap().starts_with("Re: "));
        assert!(stored.metadata["internet_message_id"].as_str().unwrap().contains(&stored.external_id));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn accepting_an_invitation_answers_the_server_and_the_organiser_writes_back() {
        let (pool, _, provider, mock) = connected(MockKind::Outlook).await;
        crate::models::AppSetting::put(&pool, crate::models::app_setting::MOCK_REPLY_SECONDS, "1").await.unwrap();
        let auth = provider.auth.clone().unwrap();
        crate::services::calendar_service::sync_provider_calendars(&pool, &provider, std::sync::Arc::new(MockProvider::new(MockKind::Outlook, pool.clone())), &auth).await.unwrap();
        let (event_id, status): (String, Option<String>) = sqlx::query_as("SELECT id, response_status FROM event WHERE title = 'Arkitekturgjennomgang Tidewater'").fetch_one(&pool).await.unwrap();
        assert_eq!(status.as_deref(), Some("notResponded"));
        assert!(crate::services::calendar_service::put_response(&pool, &event_id, "maybe").await.is_err());
        let answered = crate::services::calendar_service::put_response(&pool, &event_id, "accepted").await.unwrap();
        assert_eq!(answered.response_status.as_deref(), Some("accepted"));
        let own: String = sqlx::query_scalar("SELECT id FROM event WHERE title = '1:1 Nora'").fetch_one(&pool).await.unwrap();
        assert!(crate::services::calendar_service::put_response(&pool, &own, "accepted").await.unwrap_err().contains("organise"));
        let server = Server::open(&mock, &auth).await.unwrap();
        let later = Utc::now() + Duration::seconds(3);
        assert!(server::due(&pool, &server.account, later).await.unwrap());
        assert_eq!(server.deliver(later).await.unwrap(), 1);
        let conversations = server.get_conversations(None).await.unwrap();
        assert!(conversations.iter().any(|c| c.name == "Re: Arkitekturgjennomgang Tidewater"));
        let made = mock.post_calendar_event(&auth, "mock-outlook-calendar", &NewRemoteEvent { title: "Planlegging".into(), starts_at: Utc::now() + Duration::days(1), ends_at: Utc::now() + Duration::days(1) + Duration::hours(1), attendees: vec!["kari.nordmann@tidewater.example".into()], online_meeting: true, ..Default::default() }).await.unwrap();
        assert!(made.online_meeting_url.is_some());
        assert!(mock.post_calendar_event(&auth, "mock-outlook-holidays", &NewRemoteEvent { title: "x".into(), ..Default::default() }).await.unwrap_err().contains("read only"));
        assert_eq!(server.deliver(Utc::now() + Duration::seconds(3)).await.unwrap(), 1);
        assert!(server.get_conversations(None).await.unwrap().iter().any(|c| c.name == "Accepted: Planlegging"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_simulated_reply_arrives_through_sync_unread_and_fires_an_arrival_action() {
        let (pool, profile, provider, mock) = connected(MockKind::Teams).await;
        crate::models::AppSetting::put(&pool, crate::models::app_setting::MOCK_REPLY_SECONDS, "1").await.unwrap();
        let auth = provider.auth.clone().unwrap();
        let external = "mock-teams-dm-0";
        let conversation: String = sqlx::query_scalar("SELECT conversation_id FROM conversation_provider WHERE provider_id = ?1 AND external_id = ?2").bind(&provider.id).bind(external).fetch_one(&pool).await.unwrap();
        mock.send_message(&auth, external, &OutgoingMessage { content: "Hei Eva, har du et minutt?".into(), content_type: "text".into(), ..Default::default() }).await.unwrap();
        let server = Server::open(&mock, &auth).await.unwrap();
        assert_eq!(server.deliver(Utc::now()).await.unwrap(), 0);
        assert_eq!(server.deliver(Utc::now() + Duration::seconds(2)).await.unwrap(), 1);
        resync(&pool, &provider, MockKind::Teams).await;
        let (last, unread): (String, i64) = sqlx::query_as("SELECT last_id, unread_count FROM conversation WHERE id = ?1").bind(&conversation).fetch_one(&pool).await.unwrap();
        let (sender, read): (String, bool) = sqlx::query_as("SELECT sender_email, is_read FROM message WHERE id = ?1").bind(&last).fetch_one(&pool).await.unwrap();
        assert_eq!(sender, "eva.ortega@northwind.example");
        assert!(!read && unread > 0);
        use crate::models::action::{Graph, PostActionInput, TriggerOn};
        use crate::services::action_service::{trigger_node, wire};
        let notify = crate::models::action::Node { id: "tell".into(), kind: format!("{}notify", crate::models::action::BUILTIN_PREFIX), label: "Tell me".into(), config: serde_json::json!({ "values": { "title": "Svar" } }), body: String::new(), x: 0.0, y: 0.0 };
        let graph = Graph { nodes: vec![trigger_node("arrival", "Reply arrives", serde_json::json!({ "on": "arrival" }), "", 0.0), notify], edges: vec![wire("arrival", "text", "tell", "body")] };
        crate::services::action_service::post_action(&pool, &profile, PostActionInput { name: "Tell me about replies".into(), graph: Some(graph), ..Default::default() }).await.unwrap();
        let runner = crate::services::action_service::Runner { pool: pool.clone(), app: None, app_data: std::env::temp_dir(), cache: std::env::temp_dir(), profile_id: profile.clone(), dry_run: false, ai: None, test_delay: None };
        let runs = crate::services::action_service::dispatch(&runner, &[TriggerOn::Arrival], &last).await.unwrap();
        assert_eq!(runs.len(), 1, "{:?}", runs);
        assert_eq!(runs[0].status, "done", "{:?}", runs[0]);
    }

    #[tokio::test]
    async fn every_error_state_surfaces_the_connectors_own_error() {
        for kind in MockKind::ALL {
            let pool = crate::test_helpers::test_pool().await;
            let mock = MockProvider::new(kind, pool.clone());
            let auth = auth();
            let first = mock.sync_conversations(&auth, None).await.unwrap();
            let account = server::account(kind, &auth);
            let draft = OutgoingMessage { content: "Hei".into(), content_type: "text".into(), ..Default::default() };
            for state in [server::STATE_EXPIRED, server::STATE_RATE_LIMITED, server::STATE_OFFLINE] {
                server::put_state(&pool, &account, kind, state).await.unwrap();
                let error = mock.sync_conversations(&auth, None).await.unwrap_err();
                assert_eq!(Some(error.clone()), server::refusal(kind, state, false), "{:?} {}", kind, state);
                assert!(mock.send_message(&auth, &first[0].external_id, &draft).await.is_err());
                assert!(!server::due(&pool, &account, Utc::now() + Duration::days(1)).await.unwrap());
                if matches!(kind, MockKind::Teams | MockKind::Outlook) {
                    assert!(error.starts_with("Graph ") || error.starts_with("error sending request"), "{}", error);
                }
            }
            server::put_state(&pool, &account, kind, server::STATE_EXPIRED).await.unwrap();
            assert!(!mock.validate_tokens(&auth).await.unwrap());
            assert!(mock.refresh_token("mock").await.unwrap_err().contains("invalid_grant"));
            server::put_state(&pool, &account, kind, server::STATE_DENIED).await.unwrap();
            assert!(mock.sync_conversations(&auth, None).await.is_ok());
            assert_eq!(Some(mock.send_message(&auth, &first[0].external_id, &draft).await.unwrap_err()), server::refusal(kind, server::STATE_DENIED, true));
            server::put_state(&pool, &account, kind, server::STATE_OK).await.unwrap();
            assert!(mock.validate_tokens(&auth).await.unwrap());
            assert!(mock.send_message(&auth, &first[0].external_id, &draft).await.is_ok());
            assert!(server::put_state(&pool, &account, kind, "melted").await.is_err());
        }
    }

    #[tokio::test]
    async fn newsletters_and_bots_arrive_on_their_cadence() {
        for kind in [MockKind::Gmail, MockKind::Slack] {
            let pool = crate::test_helpers::test_pool().await;
            let mock = MockProvider::new(kind, pool.clone());
            mock.sync_conversations(&auth(), None).await.unwrap();
            let account = server::account(kind, &auth());
            assert!(!server::due(&pool, &account, Utc::now()).await.unwrap());
            crate::models::AppSetting::put(&pool, crate::models::app_setting::MOCK_NEWSLETTER_MINUTES, "1").await.unwrap();
            sqlx::query("UPDATE mock_server_account SET newsletter_at = ?1 WHERE id = ?2").bind((Utc::now() - Duration::minutes(2)).to_rfc3339_opts(chrono::SecondsFormat::Micros, true)).bind(&account).execute(&pool).await.unwrap();
            assert!(server::due(&pool, &account, Utc::now()).await.unwrap());
            let since = Utc::now();
            let fresh = mock.sync_conversations(&auth(), Some(since)).await.unwrap();
            assert_eq!(fresh.len(), 1, "{:?}", kind);
            let messages = mock.sync_messages(&auth(), &fresh[0].external_id, Some(since), None).await.unwrap();
            if kind == MockKind::Gmail {
                assert!(fresh[0].name.starts_with("TLDR"));
                assert!(!messages[0].is_read && messages[0].sender.email.as_deref() == Some("newsletter@tldrnewsletter.com"));
            } else {
                assert_eq!(messages[0].r#type, MessageType::Bot);
            }
            assert!(!server::due(&pool, &account, Utc::now()).await.unwrap());
            crate::models::AppSetting::put(&pool, crate::models::app_setting::MOCK_NEWSLETTER_MINUTES, "0").await.unwrap();
            sqlx::query("UPDATE mock_server_account SET newsletter_at = ?1 WHERE id = ?2").bind((Utc::now() - Duration::days(2)).to_rfc3339_opts(chrono::SecondsFormat::Micros, true)).bind(&account).execute(&pool).await.unwrap();
            assert!(!server::due(&pool, &account, Utc::now()).await.unwrap());
        }
    }

    #[tokio::test]
    async fn presence_comes_from_the_server_and_mail_has_none() {
        let pool = crate::test_helpers::test_pool().await;
        let teams = MockProvider::new(MockKind::Teams, pool.clone());
        let presence = teams.get_presence(&auth(), "mock-teams-u0").await.unwrap();
        assert!(["online", "away", "busy", "offline"].contains(&presence.as_str()));
        assert!(MockProvider::new(MockKind::Imap, pool).get_presence(&auth(), "x").await.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn members_names_and_pins_round_trip_as_system_events() {
        use crate::services::outbound_service::{put_member, put_name, put_pin};
        let (pool, _, provider, _) = connected(MockKind::Teams).await;
        let group: String = sqlx::query_scalar("SELECT conversation_id FROM conversation_provider WHERE provider_id = ?1 AND external_id = 'mock-teams-group-0'").bind(&provider.id).fetch_one(&pool).await.unwrap();
        let mia: String = sqlx::query_scalar("SELECT user_id FROM provider_user WHERE provider_id = ?1 AND external_id = 'mock-teams-u11'").bind(&provider.id).fetch_one(&pool).await.unwrap();
        assert!(put_member(&pool, &group, &mia, true).await.unwrap());
        assert!(put_name(&pool, &group, "Mandagsmøtet").await.unwrap());
        let message: String = sqlx::query_scalar("SELECT id FROM message WHERE conversation_id = ?1 AND type = 'chat' ORDER BY timestamp DESC LIMIT 1").bind(&group).fetch_one(&pool).await.unwrap();
        assert!(put_pin(&pool, &message, true).await.unwrap());
        assert!(put_member(&pool, &group, &mia, false).await.unwrap());
        resync(&pool, &provider, MockKind::Teams).await;
        let rows: Vec<String> = sqlx::query_scalar("SELECT metadata FROM message WHERE conversation_id = ?1 AND type = 'system' AND external_id NOT LIKE 'mock-%'").bind(&group).fetch_all(&pool).await.unwrap();
        let events: Vec<serde_json::Value> = rows.iter().map(|m| serde_json::from_str::<serde_json::Value>(m).unwrap()["system"].clone()).collect();
        let kinds: Vec<&str> = events.iter().filter_map(|e| e["event"].as_str()).collect();
        for wanted in ["member_added", "renamed", "pinned", "member_removed"] {
            assert!(kinds.contains(&wanted), "{} missing from {:?}", wanted, kinds);
        }
        assert!(events.iter().any(|e| e["event"] == "renamed" && e["detail"] == "Mandagsmøtet"));
        assert!(events.iter().any(|e| e["event"] == "member_added" && e["targets"][0]["name"] == "Mia Sund"));
        let pinned: String = sqlx::query_scalar("SELECT metadata FROM message WHERE id = ?1").bind(&message).fetch_one(&pool).await.unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&pinned).unwrap()["pinned"], true);
        let direct: String = sqlx::query_scalar("SELECT conversation_id FROM conversation_provider WHERE provider_id = ?1 AND external_id = 'mock-teams-dm-0'").bind(&provider.id).fetch_one(&pool).await.unwrap();
        assert!(put_member(&pool, &direct, &mia, true).await.unwrap_err().starts_with("400"));
        let (outlook_pool, _, outlook, _) = connected(MockKind::Outlook).await;
        let (thread, _) = conversation_of(&outlook_pool, &outlook, "email").await;
        assert!(!put_name(&outlook_pool, &thread, "Ny tittel").await.unwrap(), "mail keeps renames in Chatnip");
    }
}
