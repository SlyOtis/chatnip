use crate::models::{app_setting, AppSetting};
use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::IpAddr;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct StatsRangeInput {
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub person_ids: Vec<String>,
    #[serde(default)]
    pub provider_ids: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PersonStat {
    pub person_id: Option<String>,
    pub name: String,
    pub messages: i64,
    pub characters: i64,
    pub avg_length: f64,
    pub to_me: i64,
    pub from_me: i64,
    pub median_response_minutes: Option<f64>,
    pub my_median_response_minutes: Option<f64>,
    pub first_seen: Option<String>,
    pub last_seen: Option<String>,
    pub provider_types: Vec<String>,
    pub busiest_hour: Option<i64>,
    pub busiest_weekday: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderStat {
    pub provider_id: String,
    pub provider_type: String,
    pub name: String,
    pub messages: i64,
    pub conversations: i64,
    pub people: i64,
    pub unread: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversationStat {
    pub conversation_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub conversation_type: String,
    pub messages: i64,
    pub people: i64,
    pub characters: i64,
    pub last_activity: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagStat {
    pub tag_id: String,
    pub name: String,
    pub color: String,
    pub conversations: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelinePoint {
    pub date: String,
    pub sent: i64,
    pub received: i64,
    pub unread: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponseBucket {
    pub label: String,
    pub minutes: i64,
    pub count: i64,
    pub theirs: i64,
    pub mine: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct KindStat {
    pub kind: String,
    pub messages: i64,
    pub sent: i64,
    pub received: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileTypeStat {
    pub kind: String,
    pub files: i64,
    pub bytes: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeekdayReply {
    pub weekday: i64,
    pub theirs: Option<f64>,
    pub mine: Option<f64>,
    pub replies: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WordStat {
    pub word: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Origin {
    pub ip: String,
    pub lat: f64,
    pub lon: f64,
    pub city: Option<String>,
    pub country: Option<String>,
    pub count: i64,
    pub person_name: Option<String>,
    pub provider_type: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Range {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Totals {
    pub messages: i64,
    pub sent: i64,
    pub received: i64,
    pub conversations: i64,
    pub people: i64,
    pub services: i64,
    pub attachments: i64,
    pub characters: i64,
    pub human_messages: i64,
    pub service_messages: i64,
    pub median_response_minutes: Option<f64>,
    pub my_median_response_minutes: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RankingEntry {
    pub person_id: Option<String>,
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ranking {
    pub key: String,
    pub title: String,
    pub description: String,
    pub entries: Vec<RankingEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupStat {
    pub key: String,
    pub kind: String,
    pub name: String,
    pub color: Option<String>,
    pub messages: i64,
    pub conversations: i64,
    pub people: i64,
    pub characters: i64,
    pub median_response_minutes: Option<f64>,
    pub busiest_hour: Option<i64>,
    pub busiest_weekday: Option<i64>,
    pub top: Vec<RankingEntry>,
    pub by_hour: Vec<i64>,
    pub by_weekday: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub range: Range,
    pub totals: Totals,
    pub by_person: Vec<PersonStat>,
    pub by_provider: Vec<ProviderStat>,
    pub by_conversation: Vec<ConversationStat>,
    pub by_label: Vec<TagStat>,
    pub by_hour: Vec<i64>,
    pub by_weekday: Vec<i64>,
    pub heatmap: Vec<Vec<i64>>,
    pub timeline: Vec<TimelinePoint>,
    pub response_times: Vec<ResponseBucket>,
    pub words: Vec<WordStat>,
    pub origins: Vec<Origin>,
    pub rankings: Vec<Ranking>,
    pub groups: Vec<GroupStat>,
    pub by_kind: Vec<KindStat>,
    pub by_file_type: Vec<FileTypeStat>,
    pub reply_by_weekday: Vec<WeekdayReply>,
}

struct Row_ {
    id: String,
    conversation_id: String,
    conversation_name: String,
    conversation_type: String,
    person_id: Option<String>,
    person_name: Option<String>,
    sender_person_name: Option<String>,
    sender_person_id: Option<String>,
    sender_is_human: Option<bool>,
    sender_email: Option<String>,
    sender_id: Option<String>,
    sender_name: String,
    provider_id: String,
    provider_type: String,
    timestamp: DateTime<Utc>,
    characters: i64,
    has_attachments: bool,
    attachments: Option<String>,
    is_read: bool,
    message_type: String,
    content_type: String,
    presented_kind: Option<String>,
    has_event: bool,
    content: String,
    origin_ip: Option<String>,
    origin_lat: Option<f64>,
    origin_lon: Option<f64>,
    origin_city: Option<String>,
    origin_country: Option<String>,
}

const STOPWORDS: [&str; 98] = [
    "the", "and", "for", "that", "this", "with", "you", "your", "are", "was", "have", "has", "not", "but", "can", "will", "from", "they", "them", "their", "what", "when", "where", "which", "there", "here", "about", "would", "could", "should", "just", "like", "into", "than", "then", "also", "been", "were", "some", "more", "very", "much", "over", "only", "https", "http", "www", "com", "det", "som", "har", "ikke", "til", "med", "jeg", "deg", "den", "der", "det", "for", "fra", "kan", "skal", "vil", "var", "men", "eller", "også", "noe", "noen", "dette", "denne", "disse", "være", "blir", "ble", "oss", "dere", "hvis", "når", "hvor", "hva", "hvordan", "mye", "mer", "bare", "takk", "hei", "alle", "etter", "over", "under", "ved", "sin", "sitt", "sine", "her", "nei",
];

fn median(values: &mut Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    Some(if values.len() % 2 == 0 { (values[mid - 1] + values[mid]) / 2.0 } else { values[mid] })
}

pub fn origin_ip_from_headers(headers: &str) -> Option<IpAddr> {
    crate::content::headers::origin_ip(headers)
}

pub struct Geo {
    pub lat: f64,
    pub lon: f64,
    pub city: Option<String>,
    pub country: Option<String>,
}

pub fn lookup(reader: &maxminddb::Reader<Vec<u8>>, ip: IpAddr) -> Option<Geo> {
    let city: maxminddb::geoip2::City = reader.lookup(ip).ok()?;
    let location = city.location?;
    let name = |names: Option<std::collections::BTreeMap<&str, &str>>| names.and_then(|n| n.get("en").map(|s| s.to_string()));
    Some(Geo {
        lat: location.latitude?,
        lon: location.longitude?,
        city: city.city.and_then(|c| name(c.names)),
        country: city.country.and_then(|c| name(c.names)),
    })
}

pub async fn resolve_origins(pool: &Pool<Sqlite>) -> Result<usize, String> {
    let path = AppSetting::get_or(pool, app_setting::GEO_MMDB_PATH, "").await?;
    if path.trim().is_empty() {
        return Ok(0);
    }
    let reader = maxminddb::Reader::open_readfile(path.trim()).map_err(|e| format!("Could not open the GeoLite2 database: {}", e))?;
    let rows = sqlx::query("SELECT id, headers, origin_ip FROM message WHERE origin_resolved = 0 AND (headers IS NOT NULL OR origin_ip IS NOT NULL) LIMIT 500")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load unresolved origins: {}", e))?;
    let mut resolved = 0;
    for row in rows {
        let id: String = row.get("id");
        let headers: Option<String> = row.try_get("headers").ok().flatten();
        let stored: Option<String> = row.try_get("origin_ip").ok().flatten();
        let ip = stored.and_then(|s| s.parse::<IpAddr>().ok()).or_else(|| headers.as_deref().and_then(origin_ip_from_headers));
        let geo = ip.and_then(|ip| lookup(&reader, ip));
        sqlx::query("UPDATE message SET origin_ip = COALESCE(?1, origin_ip), origin_lat = COALESCE(?2, origin_lat), origin_lon = COALESCE(?3, origin_lon), origin_city = COALESCE(?4, origin_city), origin_country = COALESCE(?5, origin_country), origin_resolved = 1 WHERE id = ?6")
            .bind(ip.map(|i| i.to_string()))
            .bind(geo.as_ref().map(|g| g.lat))
            .bind(geo.as_ref().map(|g| g.lon))
            .bind(geo.as_ref().and_then(|g| g.city.clone()))
            .bind(geo.as_ref().and_then(|g| g.country.clone()))
            .bind(&id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to store an origin: {}", e))?;
        if geo.is_some() {
            resolved += 1;
        }
    }
    Ok(resolved)
}

async fn load(pool: &Pool<Sqlite>, profile_id: &str, input: &StatsRangeInput) -> Result<Vec<Row_>, String> {
    let rows = sqlx::query(
        r#"SELECT m.id, m.conversation_id, c.name AS conversation_name, c.type AS conversation_type,
                  c.person_id, pe.display_name AS person_name,
                  sp.id AS sender_person_id, sp.is_human AS sender_is_human, sp.display_name AS sender_person_name, m.sender_email,
                  m.sender_id, COALESCE(m.sender_name, m.sender_email, 'Unknown') AS sender_name,
                  m.provider_id, p.provider_type, m.timestamp,
                  LENGTH(COALESCE(m.content, '')) AS characters, m.has_attachments, m.attachments, COALESCE(m.is_read, 0) AS is_read,
                  COALESCE(m.type, 'chat') AS message_type, COALESCE(m.content_type, '') AS content_type,
                  COALESCE(CASE WHEN json_valid(m.presentation) THEN json_extract(m.presentation, '$.kind') END, CASE WHEN json_valid(c.presentation) THEN json_extract(c.presentation, '$.kind') END) AS presented_kind,
                  CASE WHEN json_valid(m.metadata) AND json_extract(m.metadata, '$.event') IS NOT NULL THEN 1 ELSE 0 END AS has_event,
                  COALESCE(m.content, '') AS content,
                  m.origin_ip, m.origin_lat, m.origin_lon, m.origin_city, m.origin_country
           FROM message m
           JOIN conversation c ON c.id = m.conversation_id
           JOIN provider p ON p.id = m.provider_id
           LEFT JOIN person pe ON pe.id = c.person_id
           LEFT JOIN person_identity spi ON spi.user_id = m.sender_id
           LEFT JOIN person sp ON sp.id = spi.person_id AND sp.profile_id = ?1
           WHERE p.profile_id = ?1 AND m.deleted_datetime IS NULL AND c.deleted_at IS NULL
             AND m.timestamp >= ?2 AND m.timestamp <= ?3
           ORDER BY m.conversation_id, m.timestamp"#,
    )
    .bind(profile_id)
    .bind(&input.start)
    .bind(&input.end)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to load messages for statistics: {}", e))?;
    let labelled: HashSet<String> = if input.tag_ids.is_empty() {
        HashSet::new()
    } else {
        let marks = input.tag_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect::<Vec<_>>().join(", ");
        let sql = format!("SELECT DISTINCT object_id FROM object_tag WHERE object_type = 'conversation' AND tag_id IN ({})", marks);
        let mut statement = sqlx::query_scalar::<_, String>(&sql);
        for id in &input.tag_ids {
            statement = statement.bind(id);
        }
        statement.fetch_all(pool).await.map_err(|e| format!("Failed to filter by label: {}", e))?.into_iter().collect()
    };
    let mut out = Vec::new();
    for row in rows {
        let conversation_id: String = row.get("conversation_id");
        let person_id: Option<String> = row.try_get("person_id").ok().flatten();
        let sender_person_id: Option<String> = row.try_get("sender_person_id").ok().flatten();
        let provider_id: String = row.get("provider_id");
        if !input.person_ids.is_empty() {
            let in_conversation = person_id.as_ref().map(|p| input.person_ids.contains(p)).unwrap_or(false);
            let by_sender = sender_person_id.as_ref().map(|p| input.person_ids.contains(p)).unwrap_or(false);
            if !in_conversation && !by_sender {
                continue;
            }
        }
        if !input.provider_ids.is_empty() && !input.provider_ids.contains(&provider_id) {
            continue;
        }
        if !input.tag_ids.is_empty() && !labelled.contains(&conversation_id) {
            continue;
        }
        let timestamp: String = row.get("timestamp");
        let Ok(timestamp) = DateTime::parse_from_rfc3339(&timestamp).map(|t| t.with_timezone(&Utc)) else { continue };
        out.push(Row_ {
            id: row.get("id"),
            conversation_id,
            conversation_name: row.get("conversation_name"),
            conversation_type: row.get("conversation_type"),
            person_id,
            person_name: row.try_get("person_name").ok().flatten(),
            sender_person_name: row.try_get("sender_person_name").ok().flatten(),
            sender_person_id,
            sender_is_human: row.try_get("sender_is_human").ok().flatten(),
            sender_email: row.try_get("sender_email").ok().flatten(),
            sender_id: row.try_get("sender_id").ok().flatten(),
            sender_name: row.get("sender_name"),
            provider_id,
            provider_type: row.get("provider_type"),
            timestamp,
            characters: row.get("characters"),
            has_attachments: row.try_get("has_attachments").unwrap_or(false),
            attachments: row.try_get("attachments").ok().flatten(),
            is_read: row.try_get("is_read").unwrap_or(true),
            message_type: row.get("message_type"),
            content_type: row.get("content_type"),
            presented_kind: row.try_get("presented_kind").ok().flatten(),
            has_event: row.try_get("has_event").unwrap_or(false),
            content: row.get("content"),
            origin_ip: row.try_get("origin_ip").ok().flatten(),
            origin_lat: row.try_get("origin_lat").ok().flatten(),
            origin_lon: row.try_get("origin_lon").ok().flatten(),
            origin_city: row.try_get("origin_city").ok().flatten(),
            origin_country: row.try_get("origin_country").ok().flatten(),
        });
    }
    Ok(out)
}

fn sender_key(row: &Row_) -> String {
    row.sender_person_id.clone().or_else(|| row.sender_id.clone()).unwrap_or_else(|| row.sender_name.clone())
}

fn sender_label(row: &Row_) -> String {
    row.sender_person_name.clone().or_else(|| row.person_name.clone()).unwrap_or_else(|| row.sender_name.clone())
}

fn busiest(values: &[i64]) -> Option<i64> {
    values.iter().enumerate().max_by_key(|(_, v)| **v).filter(|(_, v)| **v > 0).map(|(i, _)| i as i64)
}

fn is_human_sender(row: &Row_) -> bool {
    matches!((&row.sender_person_id, row.sender_is_human), (Some(_), Some(true)))
        || (row.sender_person_id.is_none() && !crate::services::people_service::is_service(row.sender_email.as_deref(), Some(&row.sender_name)))
}

pub const KINDS: [&str; 6] = ["chat", "channel", "mail", "invite", "notification", "bot"];

fn message_kind(row: &Row_, mine: bool) -> &'static str {
    match row.presented_kind.as_deref() {
        Some("chat") => return "chat",
        Some("mail") | Some("digest") => return "mail",
        Some("invite") => return "invite",
        Some("notification") | Some("system") => return "notification",
        Some("bot") => return "bot",
        _ => {}
    }
    if row.content_type.to_lowercase().contains("text/calendar") || row.has_event || row.content.contains("BEGIN:VEVENT") {
        return "invite";
    }
    let mail = row.message_type == "email" || row.conversation_type == "email";
    if row.message_type == "system" {
        return "notification";
    }
    if row.message_type == "bot" || (!mine && !is_human_sender(row)) {
        return if mail { "notification" } else { "bot" };
    }
    if mail {
        return "mail";
    }
    if row.conversation_type == "channel" || row.conversation_type == "group_message" {
        return "channel";
    }
    "chat"
}

pub const FILE_KINDS: [&str; 10] = ["image", "pdf", "document", "spreadsheet", "presentation", "archive", "calendar", "audio", "video", "other"];

fn file_kind(name: &str, content_type: &str) -> &'static str {
    let mime = content_type.to_lowercase();
    let extension = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    let is = |mimes: &[&str], extensions: &[&str]| mimes.iter().any(|m| mime.contains(m)) || extensions.contains(&extension.as_str());
    if is(&["image/"], &["png", "jpg", "jpeg", "gif", "webp", "heic", "svg"]) {
        "image"
    } else if is(&["pdf"], &["pdf"]) {
        "pdf"
    } else if is(&["spreadsheet", "excel", "csv"], &["xls", "xlsx", "csv", "numbers", "ods"]) {
        "spreadsheet"
    } else if is(&["presentation", "powerpoint"], &["ppt", "pptx", "key", "odp"]) {
        "presentation"
    } else if is(&["calendar"], &["ics"]) {
        "calendar"
    } else if is(&["zip", "compressed", "tar", "x-7z"], &["zip", "gz", "tar", "7z", "rar"]) {
        "archive"
    } else if is(&["audio/"], &["mp3", "m4a", "wav", "ogg"]) {
        "audio"
    } else if is(&["video/"], &["mp4", "mov", "webm", "mkv"]) {
        "video"
    } else if is(&["word", "text/", "document", "rtf"], &["doc", "docx", "txt", "md", "rtf", "odt", "pages"]) {
        "document"
    } else {
        "other"
    }
}

fn file_types(rows: &[Row_]) -> Vec<FileTypeStat> {
    let mut out: Vec<FileTypeStat> = FILE_KINDS.iter().map(|kind| FileTypeStat { kind: kind.to_string(), files: 0, bytes: 0 }).collect();
    for row in rows {
        let Some(raw) = row.attachments.as_deref() else { continue };
        let Ok(list) = serde_json::from_str::<Vec<serde_json::Value>>(raw) else { continue };
        for item in list {
            if item.get("is_inline").and_then(|v| v.as_bool()).unwrap_or(false) {
                continue;
            }
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let mime = item.get("content_type").and_then(|v| v.as_str()).unwrap_or("");
            let kind = file_kind(name, mime);
            if let Some(stat) = out.iter_mut().find(|s| s.kind == kind) {
                stat.files += 1;
                stat.bytes += item.get("size").and_then(|v| v.as_i64()).unwrap_or(0);
            }
        }
    }
    out.retain(|s| s.files > 0);
    out.sort_by(|a, b| b.files.cmp(&a.files).then(a.kind.cmp(&b.kind)));
    out
}

fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x1F300..=0x1FAFF | 0x2600..=0x27BF | 0x1F1E6..=0x1F1FF)
}

struct Gaps {
    mine: Vec<f64>,
    theirs: Vec<f64>,
    my_by_person: HashMap<String, Vec<f64>>,
    their_by_person: HashMap<String, Vec<f64>>,
    my_by_weekday: Vec<Vec<f64>>,
    their_by_weekday: Vec<Vec<f64>>,
}

fn gaps<'a>(rows: &[&'a Row_], mine: &dyn Fn(&Row_) -> bool) -> Gaps {
    let mut out = Gaps { mine: Vec::new(), theirs: Vec::new(), my_by_person: HashMap::new(), their_by_person: HashMap::new(), my_by_weekday: vec![Vec::new(); 7], their_by_weekday: vec![Vec::new(); 7] };
    let mut index = 0;
    while index < rows.len() {
        let conversation = &rows[index].conversation_id;
        let mut end = index;
        while end < rows.len() && &rows[end].conversation_id == conversation {
            end += 1;
        }
        for pair in rows[index..end].windows(2) {
            let (previous, next) = (pair[0], pair[1]);
            let gap = (next.timestamp - previous.timestamp).num_seconds() as f64 / 60.0;
            if gap < 0.0 || gap > 60.0 * 24.0 * 14.0 {
                continue;
            }
            let weekday = next.timestamp.with_timezone(&chrono::Local).weekday().num_days_from_monday() as usize;
            if !mine(previous) && mine(next) {
                out.mine.push(gap);
                out.my_by_person.entry(sender_key(previous)).or_default().push(gap);
                out.my_by_weekday[weekday].push(gap);
            } else if mine(previous) && !mine(next) {
                out.theirs.push(gap);
                out.their_by_person.entry(sender_key(next)).or_default().push(gap);
                out.their_by_weekday[weekday].push(gap);
            }
        }
        index = end;
    }
    out
}

fn top_senders(rows: &[&Row_], mine: &dyn Fn(&Row_) -> bool, limit: usize) -> Vec<RankingEntry> {
    let mut counts: HashMap<String, (i64, &Row_)> = HashMap::new();
    for row in rows {
        if mine(row) {
            continue;
        }
        let entry = counts.entry(sender_key(row)).or_insert((0, row));
        entry.0 += 1;
    }
    let mut entries: Vec<RankingEntry> = counts
        .into_values()
        .map(|(count, row)| RankingEntry { person_id: row.sender_person_id.clone(), name: sender_label(row), value: count as f64, unit: "messages".to_string(), detail: None })
        .collect();
    entries.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal).then(a.name.cmp(&b.name)));
    entries.truncate(limit);
    entries
}

fn group_stat(key: &str, kind: &str, name: &str, color: Option<String>, rows: &[&Row_], mine: &dyn Fn(&Row_) -> bool) -> GroupStat {
    let mut by_hour = vec![0i64; 24];
    let mut by_weekday = vec![0i64; 7];
    let mut conversations: HashSet<&str> = HashSet::new();
    let mut people: HashSet<String> = HashSet::new();
    let mut characters = 0;
    for row in rows {
        let local = row.timestamp.with_timezone(&chrono::Local);
        by_hour[local.hour() as usize] += 1;
        by_weekday[local.weekday().num_days_from_monday() as usize] += 1;
        conversations.insert(row.conversation_id.as_str());
        characters += row.characters;
        if !mine(row) {
            people.insert(sender_key(row));
        }
    }
    let mut g = gaps(rows, mine);
    GroupStat {
        key: key.to_string(),
        kind: kind.to_string(),
        name: name.to_string(),
        color,
        messages: rows.len() as i64,
        conversations: conversations.len() as i64,
        people: people.len() as i64,
        characters,
        median_response_minutes: median(&mut g.theirs),
        busiest_hour: busiest(&by_hour),
        busiest_weekday: busiest(&by_weekday),
        top: top_senders(rows, mine, 5),
        by_hour,
        by_weekday,
    }
}

struct SenderAgg<'a> {
    first: &'a Row_,
    messages: i64,
    characters: i64,
    questions: i64,
    attachments: i64,
    links: i64,
    emoji: i64,
    night: i64,
    early: i64,
    weekend: i64,
    conversations: HashSet<String>,
    last_seen: DateTime<Utc>,
    reactions: i64,
}

fn ranking(key: &str, title: &str, description: &str, unit: &str, mut entries: Vec<RankingEntry>, ascending: bool) -> Ranking {
    entries.sort_by(|a, b| {
        let order = a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal);
        (if ascending { order } else { order.reverse() }).then(a.name.cmp(&b.name))
    });
    entries.truncate(10);
    for entry in &mut entries {
        entry.unit = unit.to_string();
    }
    Ranking { key: key.to_string(), title: title.to_string(), description: description.to_string(), entries }
}

fn rankings(rows: &[&Row_], mine: &dyn Fn(&Row_) -> bool, g: &Gaps, reactions: &HashMap<String, i64>, now: DateTime<Utc>) -> Vec<Ranking> {
    let mut agg: HashMap<String, SenderAgg> = HashMap::new();
    for row in rows {
        if mine(row) {
            continue;
        }
        let local = row.timestamp.with_timezone(&chrono::Local);
        let hour = local.hour();
        let weekday = local.weekday().num_days_from_monday();
        let entry = agg.entry(sender_key(row)).or_insert_with(|| SenderAgg {
            first: row,
            messages: 0,
            characters: 0,
            questions: 0,
            attachments: 0,
            links: 0,
            emoji: 0,
            night: 0,
            early: 0,
            weekend: 0,
            conversations: HashSet::new(),
            last_seen: row.timestamp,
            reactions: 0,
        });
        entry.messages += 1;
        entry.characters += row.characters;
        entry.questions += row.content.contains('?') as i64;
        entry.attachments += row.has_attachments as i64;
        entry.links += row.content.matches("http").count() as i64;
        entry.emoji += row.content.chars().filter(|c| is_emoji(*c)).count() as i64;
        entry.night += (hour >= 22 || hour < 6) as i64;
        entry.early += (5..9).contains(&hour) as i64;
        entry.weekend += (weekday >= 5) as i64;
        entry.conversations.insert(row.conversation_id.clone());
        entry.reactions += reactions.get(&row.id).copied().unwrap_or(0);
        if row.timestamp > entry.last_seen {
            entry.last_seen = row.timestamp;
        }
    }
    let entry = |a: &SenderAgg, value: f64, detail: Option<String>| RankingEntry { person_id: a.first.sender_person_id.clone(), name: sender_label(a.first), value, unit: String::new(), detail };
    let count = |pick: &dyn Fn(&SenderAgg) -> i64| agg.values().filter(|a| pick(a) > 0).map(|a| entry(a, pick(a) as f64, None)).collect::<Vec<_>>();
    let share = |pick: &dyn Fn(&SenderAgg) -> i64| {
        agg.values()
            .filter(|a| a.messages >= 5)
            .map(|a| entry(a, 100.0 * pick(a) as f64 / a.messages as f64, Some(format!("{} of {} messages", pick(a), a.messages))))
            .filter(|e| e.value > 0.0)
            .collect::<Vec<_>>()
    };
    let reply = |map: &HashMap<String, Vec<f64>>| {
        agg.iter()
            .filter_map(|(key, a)| map.get(key).cloned().as_mut().and_then(median).map(|m| entry(a, m, Some(if map[key].len() == 1 { "1 reply".to_string() } else { format!("{} replies", map[key].len()) }))))
            .collect::<Vec<_>>()
    };
    let mut out = vec![
        ranking("most_messages", "Writes most often", "Messages sent to you in the period.", "messages", count(&|a| a.messages), false),
        ranking("most_characters", "Writes the most text", "Characters across every message.", "characters", count(&|a| a.characters), false),
        ranking(
            "longest_average",
            "Longest messages",
            "Average characters per message, three messages or more.",
            "characters",
            agg.values().filter(|a| a.messages >= 3).map(|a| entry(a, a.characters as f64 / a.messages as f64, Some(format!("{} messages", a.messages)))).collect(),
            false,
        ),
        ranking("fastest_reply", "Answers you fastest", "Median minutes from your message to their reply.", "minutes", reply(&g.their_by_person), true),
        ranking("slowest_reply", "Answers you slowest", "Median minutes from your message to their reply.", "minutes", reply(&g.their_by_person), false),
        ranking("fastest_i_reply", "You answer fastest", "Median minutes from their message to your reply.", "minutes", reply(&g.my_by_person), true),
        ranking("most_questions", "Asks the most", "Messages with a question mark.", "count", count(&|a| a.questions), false),
        ranking("most_attachments", "Sends the most files", "Messages carrying an attachment.", "count", count(&|a| a.attachments), false),
        ranking("most_links", "Shares the most links", "Links found in messages.", "count", count(&|a| a.links), false),
        ranking("most_emoji", "Uses the most emoji", "Emoji characters across messages.", "count", count(&|a| a.emoji), false),
        ranking("night_owl", "Night owls", "Share of messages between 22:00 and 06:00, five messages or more.", "percent", share(&|a| a.night), false),
        ranking("early_bird", "Early birds", "Share of messages between 05:00 and 09:00, five messages or more.", "percent", share(&|a| a.early), false),
        ranking("weekend_share", "Weekend writers", "Share of messages on Saturday and Sunday, five messages or more.", "percent", share(&|a| a.weekend), false),
        ranking("most_conversations", "In the most conversations", "Distinct conversations they wrote in.", "count", agg.values().map(|a| entry(a, a.conversations.len() as f64, None)).collect(), false),
        ranking(
            "longest_silence",
            "Quiet the longest",
            "Days since their last message.",
            "count",
            agg.values().map(|a| entry(a, (now - a.last_seen).num_hours() as f64 / 24.0, Some(format!("last {}", a.last_seen.with_timezone(&chrono::Local).format("%-d %b"))))).collect(),
            false,
        ),
    ];
    if !reactions.is_empty() {
        out.push(ranking("most_reactions_received", "Gets the most reactions", "Reactions received on their messages.", "count", count(&|a| a.reactions), false));
    }
    out
}

async fn reactions_by_message(pool: &Pool<Sqlite>, profile_id: &str, input: &StatsRangeInput) -> HashMap<String, i64> {
    let rows = sqlx::query(
        r#"SELECT r.message_id, COUNT(*) AS reactions FROM message_reactions r
           JOIN message m ON m.id = r.message_id JOIN provider p ON p.id = m.provider_id
           WHERE p.profile_id = ?1 AND m.timestamp >= ?2 AND m.timestamp <= ?3 GROUP BY r.message_id"#,
    )
    .bind(profile_id)
    .bind(&input.start)
    .bind(&input.end)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.into_iter().map(|r| (r.get::<String, _>("message_id"), r.get::<i64, _>("reactions"))).collect()
}

pub async fn stats(pool: &Pool<Sqlite>, profile_id: &str, input: &StatsRangeInput) -> Result<Stats, String> {
    resolve_origins(pool).await.ok();
    let rows = load(pool, profile_id, input).await?;
    let me: HashSet<String> = crate::models::Profile::get_own_user_ids(pool, profile_id).await?.into_iter().collect();
    let mine = |row: &Row_| row.sender_id.as_ref().map(|s| me.contains(s)).unwrap_or(false);

    let mut by_hour = vec![0i64; 24];
    let mut by_weekday = vec![0i64; 7];
    let mut heatmap = vec![vec![0i64; 24]; 7];
    let mut timeline: BTreeMap<NaiveDate, (i64, i64, i64)> = BTreeMap::new();
    let mut kinds: Vec<KindStat> = KINDS.iter().map(|kind| KindStat { kind: kind.to_string(), messages: 0, sent: 0, received: 0 }).collect();
    let mut human_messages = 0;
    let mut service_messages = 0;
    let mut words: HashMap<String, i64> = HashMap::new();
    let mut conversations: HashSet<String> = HashSet::new();
    let mut people: HashSet<String> = HashSet::new();
    let mut services: HashSet<String> = HashSet::new();
    let mut sent = 0;
    let mut received = 0;
    let mut attachments = 0;
    let mut characters = 0;
    for row in &rows {
        let local = row.timestamp.with_timezone(&chrono::Local);
        let hour = local.hour() as usize;
        let weekday = local.weekday().num_days_from_monday() as usize;
        by_hour[hour] += 1;
        by_weekday[weekday] += 1;
        heatmap[weekday][hour] += 1;
        let day = timeline.entry(local.date_naive()).or_insert((0, 0, 0));
        let own = mine(row);
        if own {
            sent += 1;
            day.0 += 1;
        } else {
            received += 1;
            day.1 += 1;
            if !row.is_read {
                day.2 += 1;
            }
        }
        let kind = message_kind(row, own);
        if let Some(stat) = kinds.iter_mut().find(|k| k.kind == kind) {
            stat.messages += 1;
            if own {
                stat.sent += 1;
            } else {
                stat.received += 1;
            }
        }
        if row.has_attachments {
            attachments += 1;
        }
        characters += row.characters;
        conversations.insert(row.conversation_id.clone());
        if !mine(row) {
            match (&row.sender_person_id, row.sender_is_human) {
                (Some(person), Some(true)) => {
                    people.insert(person.clone());
                    human_messages += 1;
                }
                (Some(person), _) => {
                    services.insert(person.clone());
                    service_messages += 1;
                }
                (None, _) => {
                    let key = row.sender_id.clone().unwrap_or_else(|| row.sender_name.clone());
                    if crate::services::people_service::is_service(row.sender_email.as_deref(), Some(&row.sender_name)) {
                        services.insert(key);
                        service_messages += 1;
                    } else {
                        people.insert(key);
                        human_messages += 1;
                    }
                }
            }
        }
        for word in row.content.split(|c: char| !c.is_alphanumeric()) {
            let word = word.to_lowercase();
            if word.chars().count() > 3 && !STOPWORDS.contains(&word.as_str()) && !word.chars().all(|c| c.is_ascii_digit()) {
                *words.entry(word).or_insert(0) += 1;
            }
        }
    }

    let refs: Vec<&Row_> = rows.iter().collect();
    let all_gaps = gaps(&refs, &mine);
    let mut my_gaps = all_gaps.mine.clone();
    let mut their_gaps = all_gaps.theirs.clone();
    let person_my_gaps = &all_gaps.my_by_person;
    let person_their_gaps = &all_gaps.their_by_person;
    let mut person_rows: HashMap<String, Vec<&Row_>> = HashMap::new();
    for row in &rows {
        if mine(row) {
            continue;
        }
        person_rows.entry(sender_key(row)).or_default().push(row);
    }
    let mut by_person: Vec<PersonStat> = person_rows
        .into_iter()
        .map(|(key, list)| {
            let characters: i64 = list.iter().map(|r| r.characters).sum();
            let mut hours = vec![0i64; 24];
            let mut days = vec![0i64; 7];
            let mut providers: Vec<String> = Vec::new();
            for r in &list {
                let local = r.timestamp.with_timezone(&chrono::Local);
                hours[local.hour() as usize] += 1;
                days[local.weekday().num_days_from_monday() as usize] += 1;
                if !providers.contains(&r.provider_type) {
                    providers.push(r.provider_type.clone());
                }
            }
            let from_me = rows.iter().filter(|r| mine(r) && list.iter().any(|l| l.conversation_id == r.conversation_id)).count() as i64;
            PersonStat {
                person_id: list[0].sender_person_id.clone().or_else(|| list[0].person_id.clone()),
                name: list[0].sender_person_name.clone().or_else(|| list[0].person_name.clone()).unwrap_or_else(|| list[0].sender_name.clone()),
                messages: list.len() as i64,
                characters,
                avg_length: characters as f64 / list.len() as f64,
                to_me: list.len() as i64,
                from_me,
                median_response_minutes: person_their_gaps.get(&key).cloned().as_mut().and_then(median),
                my_median_response_minutes: person_my_gaps.get(&key).cloned().as_mut().and_then(median),
                first_seen: list.iter().map(|r| r.timestamp).min().map(|t| t.to_rfc3339()),
                last_seen: list.iter().map(|r| r.timestamp).max().map(|t| t.to_rfc3339()),
                provider_types: providers,
                busiest_hour: busiest(&hours),
                busiest_weekday: busiest(&days),
            }
        })
        .collect();
    by_person.sort_by(|a, b| b.messages.cmp(&a.messages).then(a.name.cmp(&b.name)));

    let mut provider_map: BTreeMap<String, ProviderStat> = BTreeMap::new();
    let provider_rows = sqlx::query("SELECT id, provider_type, name FROM provider WHERE profile_id = ?1")
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load providers: {}", e))?;
    for row in provider_rows {
        let id: String = row.get("id");
        provider_map.insert(id.clone(), ProviderStat { provider_id: id, provider_type: row.get("provider_type"), name: row.get("name"), messages: 0, conversations: 0, people: 0, unread: 0 });
    }
    let mut provider_conversations: HashMap<String, HashSet<String>> = HashMap::new();
    let mut provider_people: HashMap<String, HashSet<String>> = HashMap::new();
    for row in &rows {
        if let Some(stat) = provider_map.get_mut(&row.provider_id) {
            stat.messages += 1;
        }
        provider_conversations.entry(row.provider_id.clone()).or_default().insert(row.conversation_id.clone());
        if let Some(person) = &row.person_id {
            provider_people.entry(row.provider_id.clone()).or_default().insert(person.clone());
        }
    }
    for (id, stat) in provider_map.iter_mut() {
        stat.conversations = provider_conversations.get(id).map(|s| s.len() as i64).unwrap_or(0);
        stat.people = provider_people.get(id).map(|s| s.len() as i64).unwrap_or(0);
        stat.unread = sqlx::query_scalar("SELECT COUNT(*) FROM message m JOIN conversation c ON c.id = m.conversation_id WHERE m.provider_id = ?1 AND m.is_read = 0 AND m.deleted_datetime IS NULL AND c.deleted_at IS NULL")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);
    }
    let mut by_provider: Vec<ProviderStat> = provider_map.into_values().filter(|p| p.messages > 0).collect();
    by_provider.sort_by(|a, b| b.messages.cmp(&a.messages));

    let mut conversation_map: HashMap<String, ConversationStat> = HashMap::new();
    let mut conversation_people: HashMap<String, HashSet<String>> = HashMap::new();
    for row in &rows {
        let entry = conversation_map.entry(row.conversation_id.clone()).or_insert_with(|| ConversationStat {
            conversation_id: row.conversation_id.clone(),
            name: row.conversation_name.clone(),
            conversation_type: row.conversation_type.clone(),
            messages: 0,
            people: 0,
            characters: 0,
            last_activity: row.timestamp.to_rfc3339(),
        });
        entry.messages += 1;
        entry.characters += row.characters;
        if row.timestamp.to_rfc3339() > entry.last_activity {
            entry.last_activity = row.timestamp.to_rfc3339();
        }
        if let Some(sender) = &row.sender_id {
            conversation_people.entry(row.conversation_id.clone()).or_default().insert(sender.clone());
        }
    }
    let mut by_conversation: Vec<ConversationStat> = conversation_map
        .into_iter()
        .map(|(id, mut stat)| {
            stat.people = conversation_people.get(&id).map(|s| s.len() as i64).unwrap_or(0);
            stat
        })
        .collect();
    by_conversation.sort_by(|a, b| b.messages.cmp(&a.messages));
    by_conversation.truncate(20);

    let label_rows = sqlx::query(
        r#"SELECT l.id, l.name, l.color, COUNT(DISTINCT cl.object_id) AS conversations
           FROM tag l LEFT JOIN object_tag cl ON cl.tag_id = l.id AND cl.object_type = 'conversation'
           WHERE l.profile_id = ?1 GROUP BY l.id ORDER BY l.position"#,
    )
    .bind(profile_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to load labels: {}", e))?;
    let mut by_label = Vec::new();
    for row in label_rows {
        let id: String = row.get("id");
        let members: Vec<String> = sqlx::query_scalar("SELECT object_id FROM object_tag WHERE object_type = 'conversation' AND tag_id = ?1")
            .bind(&id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        let members: HashSet<String> = members.into_iter().collect();
        let messages = rows.iter().filter(|r| members.contains(&r.conversation_id)).count() as i64;
        by_label.push(TagStat { tag_id: id, name: row.get("name"), color: row.get("color"), conversations: row.get("conversations"), messages });
    }

    let buckets = [("under 5m", 5), ("15m", 15), ("1h", 60), ("4h", 240), ("1d", 1440), ("3d", 4320), ("longer", i64::MAX)];
    let mut response_times: Vec<ResponseBucket> = buckets.iter().map(|(label, minutes)| ResponseBucket { label: label.to_string(), minutes: *minutes, count: 0, theirs: 0, mine: 0 }).collect();
    for (gap, own) in my_gaps.iter().map(|g| (g, true)).chain(their_gaps.iter().map(|g| (g, false))) {
        let slot = buckets.iter().position(|(_, minutes)| (*gap as i64) < *minutes).unwrap_or(buckets.len() - 1);
        response_times[slot].count += 1;
        if own {
            response_times[slot].mine += 1;
        } else {
            response_times[slot].theirs += 1;
        }
    }
    let reply_by_weekday: Vec<WeekdayReply> = (0..7)
        .map(|day| WeekdayReply {
            weekday: day as i64,
            theirs: median(&mut all_gaps.their_by_weekday[day].clone()),
            mine: median(&mut all_gaps.my_by_weekday[day].clone()),
            replies: (all_gaps.their_by_weekday[day].len() + all_gaps.my_by_weekday[day].len()) as i64,
        })
        .collect();

    let mut word_list: Vec<WordStat> = words.into_iter().map(|(word, count)| WordStat { word, count }).collect();
    word_list.sort_by(|a, b| b.count.cmp(&a.count).then(a.word.cmp(&b.word)));
    word_list.truncate(40);

    let mut origin_map: BTreeMap<String, Origin> = BTreeMap::new();
    for row in &rows {
        let (Some(ip), Some(lat), Some(lon)) = (&row.origin_ip, row.origin_lat, row.origin_lon) else { continue };
        let entry = origin_map.entry(ip.clone()).or_insert_with(|| Origin {
            ip: ip.clone(),
            lat,
            lon,
            city: row.origin_city.clone(),
            country: row.origin_country.clone(),
            count: 0,
            person_name: row.sender_person_name.clone().or_else(|| row.person_name.clone()).or_else(|| Some(row.sender_name.clone())),
            provider_type: row.provider_type.clone(),
            last_seen: row.timestamp.to_rfc3339(),
        });
        entry.count += 1;
        if row.timestamp.to_rfc3339() > entry.last_seen {
            entry.last_seen = row.timestamp.to_rfc3339();
        }
    }
    let mut origins: Vec<Origin> = origin_map.into_values().collect();
    origins.sort_by(|a, b| b.count.cmp(&a.count));

    let timeline = timeline.into_iter().map(|(date, (sent, received, unread))| TimelinePoint { date: date.to_string(), sent, received, unread }).collect();

    let reactions = reactions_by_message(pool, profile_id, input).await;
    let rankings = rankings(&refs, &mine, &all_gaps, &reactions, Utc::now());

    let mut groups: Vec<GroupStat> = Vec::new();
    let bucket = |key: &str, name: &str, pick: &dyn Fn(&Row_) -> bool| {
        let subset: Vec<&Row_> = refs.iter().copied().filter(|r| pick(r)).collect();
        group_stat(key, "bucket", name, None, &subset, &mine)
    };
    groups.push(bucket("people", "People", &|r| (r.conversation_type == "direct_message" || r.conversation_type == "email") && (mine(r) || is_human_sender(r))));
    groups.push(bucket("teams", "Teams and channels", &|r| r.conversation_type == "channel" || r.conversation_type == "group_message"));
    groups.push(bucket("services", "Services", &|r| !mine(r) && !is_human_sender(r)));
    groups.push(bucket("mail", "Mail", &|r| r.conversation_type == "email"));
    let label_members = sqlx::query("SELECT l.id, l.name, l.color, l.kind, cl.object_id AS conversation_id FROM tag l JOIN object_tag cl ON cl.tag_id = l.id AND cl.object_type = 'conversation' WHERE l.profile_id = ?1 ORDER BY l.position")
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load label members: {}", e))?;
    let mut label_sets: Vec<(String, String, String, String, HashSet<String>)> = Vec::new();
    for row in label_members {
        let id: String = row.get("id");
        let conversation: String = row.get("conversation_id");
        match label_sets.iter_mut().find(|(lid, ..)| lid == &id) {
            Some(set) => {
                set.4.insert(conversation);
            }
            None => label_sets.push((id, row.get("name"), row.get("color"), row.get("kind"), HashSet::from([conversation]))),
        }
    }
    for (id, name, color, kind, members) in &label_sets {
        let subset: Vec<&Row_> = refs.iter().copied().filter(|r| members.contains(&r.conversation_id)).collect();
        if subset.is_empty() {
            continue;
        }
        groups.push(group_stat(id, if kind == "split" { "split" } else { "tag" }, name, Some(color.clone()), &subset, &mine));
    }
    for group in crate::services::group_service::all(pool, profile_id).await.unwrap_or_default() {
        let members: HashSet<&str> = group.conversation_ids.iter().map(|s| s.as_str()).collect();
        let subset: Vec<&Row_> = refs.iter().copied().filter(|r| members.contains(r.conversation_id.as_str())).collect();
        if subset.is_empty() {
            continue;
        }
        groups.push(group_stat(&group.id, "group", &group.name, group.color.clone(), &subset, &mine));
    }

    Ok(Stats {
        range: Range { start: input.start.clone(), end: input.end.clone() },
        totals: Totals {
            messages: rows.len() as i64,
            sent,
            received,
            conversations: conversations.len() as i64,
            people: people.len() as i64,
            services: services.len() as i64,
            attachments,
            characters,
            human_messages,
            service_messages,
            median_response_minutes: median(&mut their_gaps),
            my_median_response_minutes: median(&mut my_gaps),
        },
        by_person,
        by_provider,
        by_conversation,
        by_label,
        by_hour,
        by_weekday,
        heatmap,
        timeline,
        response_times,
        words: word_list,
        origins,
        rankings,
        groups,
        by_kind: kinds.into_iter().filter(|k| k.messages > 0).collect(),
        by_file_type: file_types(&rows),
        reply_by_weekday,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use chrono::{Duration, TimeZone};
    use uuid::Uuid;

    #[test]
    fn origin_ip_takes_the_bottom_most_public_hop() {
        let headers = "Received: from mx.northwind.example (mx.northwind.example [185.12.34.56]) by mail.chatnip.local with ESMTPS; Thu, 9 Oct 2026 10:00:00 +0200\r\nReceived: from [10.0.0.4] (helo=laptop) by mx.northwind.example with ESMTP; Thu, 9 Oct 2026 09:59:00 +0200\r\nReceived: from sender.example.org (sender.example.org [91.189.89.17]) by relay.example.org; Thu, 9 Oct 2026 09:58:00 +0200\r\nReceived: from [192.168.1.20] by sender.example.org; Thu, 9 Oct 2026 09:57:00 +0200\r\nFrom: a@example.org\r\nSubject: x";
        assert_eq!(origin_ip_from_headers(headers), Some("91.189.89.17".parse().unwrap()));
        assert_eq!(origin_ip_from_headers("Received: from [10.1.1.1] by x; Thu, 9 Oct 2026 09:57:00 +0200\r\nSubject: y"), None);
    }

    async fn seed_message(pool: &Pool<Sqlite>, conversation: &str, provider: &str, sender: &str, name: &str, at: DateTime<Utc>, content: &str) {
        sqlx::query(
            r#"INSERT INTO message (id, provider_id, external_id, type, content, content_type, conversation_id, sender_id, sender_name, importance, is_read, is_draft, has_attachments, created_datetime, last_modified_datetime, timestamp, created_at, updated_at)
               VALUES (?1, ?2, ?1, 'chat', ?3, 'text', ?4, ?5, ?6, 'normal', 1, 0, 0, ?7, ?7, ?7, ?7, ?7)"#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(provider)
        .bind(content)
        .bind(conversation)
        .bind(sender)
        .bind(name)
        .bind(at.to_rfc3339())
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn stats_count_medians_heatmap_and_timeline() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "microsoft").await;
        let me = test_user(&pool, "milo@northwind.example", "Milo").await;
        let eva = test_user(&pool, "eva@northwind.example", "Eva").await;
        sqlx::query("UPDATE profile SET user_id = ?1 WHERE id = ?2").bind(&me).bind(&profile).execute(&pool).await.unwrap();
        let conversation = test_conversation(&pool, "Eva", "direct_message").await;
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id, created_at) VALUES (?1, ?2, ?3, 'x', ?4)")
            .bind(Uuid::new_v4().to_string())
            .bind(&conversation)
            .bind(&provider)
            .bind(Utc::now().to_rfc3339())
            .execute(&pool)
            .await
            .unwrap();
        let base = Utc::now() - Duration::days(2);
        seed_message(&pool, &conversation, &provider, &eva, "Eva", base, "Kan du se på designet i dag?").await;
        seed_message(&pool, &conversation, &provider, &me, "Milo", base + Duration::minutes(10), "Ja, straks").await;
        seed_message(&pool, &conversation, &provider, &eva, "Eva", base + Duration::minutes(40), "Supert, designet er klart").await;
        seed_message(&pool, &conversation, &provider, &me, "Milo", base + Duration::minutes(70), "Takk").await;
        let input = StatsRangeInput { start: (Utc::now() - Duration::days(30)).to_rfc3339(), end: Utc::now().to_rfc3339(), person_ids: vec![], provider_ids: vec![], tag_ids: vec![] };
        seed_message(&pool, &conversation, &provider, "svc-github", "GitHub", base + Duration::minutes(80), "[SlyOtis/chatnip] CI passed").await;
        sqlx::query("UPDATE message SET sender_email = 'notifications@github.com' WHERE sender_id = 'svc-github'").execute(&pool).await.unwrap();
        let with_service = super::stats(&pool, &profile, &input).await.unwrap();
        assert_eq!(with_service.totals.services, 1);
        assert_eq!(with_service.totals.people, 1);
        sqlx::query("DELETE FROM message WHERE sender_id = 'svc-github'").execute(&pool).await.unwrap();
        let stats = super::stats(&pool, &profile, &input).await.unwrap();
        assert_eq!(stats.totals.messages, 4);
        assert_eq!(stats.totals.sent, 2);
        assert_eq!(stats.totals.received, 2);
        assert_eq!(stats.totals.my_median_response_minutes, Some(20.0));
        assert_eq!(stats.totals.median_response_minutes, Some(30.0));
        assert_eq!(stats.by_person.len(), 1);
        assert_eq!(stats.by_person[0].name, "Eva");
        assert_eq!(stats.by_person[0].messages, 2);
        assert_eq!(stats.by_person[0].from_me, 2);
        assert_eq!(stats.heatmap.iter().flatten().sum::<i64>(), 4);
        assert_eq!(stats.by_hour.iter().sum::<i64>(), 4);
        assert_eq!(stats.timeline.iter().map(|t| t.sent + t.received).sum::<i64>(), 4);
        assert_eq!(stats.by_provider[0].messages, 4);
        assert_eq!(stats.by_conversation[0].messages, 4);
        assert!(stats.words.iter().any(|w| w.word == "designet" && w.count == 2));
        assert_eq!(stats.response_times.iter().map(|b| b.count).sum::<i64>(), 3);
    }

    async fn seed_person(pool: &Pool<Sqlite>, profile: &str, user: &str, name: &str, human: bool) -> String {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO person (id, profile_id, display_name, is_human, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)")
            .bind(&id)
            .bind(profile)
            .bind(name)
            .bind(human)
            .bind(&now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO person_identity (id, person_id, user_id, provider_type, external_id, email, display_name) VALUES (?1, ?2, ?3, 'microsoft', ?3, NULL, ?4)")
            .bind(Uuid::new_v4().to_string())
            .bind(&id)
            .bind(user)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        id
    }

    async fn link(pool: &Pool<Sqlite>, conversation: &str, provider: &str, person: Option<&str>) {
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id, created_at) VALUES (?1, ?2, ?3, ?1, ?4)")
            .bind(Uuid::new_v4().to_string())
            .bind(conversation)
            .bind(provider)
            .bind(Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .unwrap();
        if let Some(person) = person {
            sqlx::query("UPDATE conversation SET person_id = ?1 WHERE id = ?2").bind(person).bind(conversation).execute(pool).await.unwrap();
        }
    }

    fn at(day: i64, hour: u32) -> DateTime<Utc> {
        let local = chrono::Local::now().date_naive() - Duration::days(day);
        let naive = local.and_hms_opt(hour, 0, 0).unwrap();
        chrono::Local.from_local_datetime(&naive).unwrap().with_timezone(&Utc)
    }

    #[tokio::test]
    async fn rankings_groups_and_person_filter() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "microsoft").await;
        let me = test_user(&pool, "milo@northwind.example", "Milo").await;
        let eva = test_user(&pool, "eva@northwind.example", "Eva").await;
        let nora = test_user(&pool, "nora@northwind.example", "Nora").await;
        let bot = test_user(&pool, "notifications@github.com", "GitHub").await;
        sqlx::query("UPDATE profile SET user_id = ?1 WHERE id = ?2").bind(&me).bind(&profile).execute(&pool).await.unwrap();
        let eva_person = seed_person(&pool, &profile, &eva, "Eva Ortega", true).await;
        let nora_person = seed_person(&pool, &profile, &nora, "Nora Gale", true).await;
        let bot_person = seed_person(&pool, &profile, &bot, "GitHub", false).await;
        let dm = test_conversation(&pool, "Eva", "direct_message").await;
        link(&pool, &dm, &provider, Some(&eva_person)).await;
        let team = test_conversation(&pool, "Platform", "channel").await;
        link(&pool, &team, &provider, None).await;
        let mail = test_conversation(&pool, "Build failed", "email").await;
        link(&pool, &mail, &provider, Some(&bot_person)).await;
        for i in 0..6 {
            seed_message(&pool, &dm, &provider, &eva, "Eva", at(5, 23) + Duration::minutes(i), "Har du sett dette? 🎉").await;
        }
        seed_message(&pool, &dm, &provider, &me, "Milo", at(5, 23) + Duration::minutes(20), "Ja").await;
        seed_message(&pool, &dm, &provider, &eva, "Eva", at(5, 23) + Duration::minutes(30), "Bra, se https://northwind.example").await;
        for i in 0..3 {
            seed_message(&pool, &team, &provider, &nora, "Nora", at(3, 10) + Duration::minutes(i), "Deployet til staging, alt grønt og klart for test av hele teamet").await;
        }
        seed_message(&pool, &team, &provider, &me, "Milo", at(3, 11), "Takk").await;
        seed_message(&pool, &mail, &provider, &bot, "GitHub", at(2, 12), "[SlyOtis/chatnip] CI failed").await;
        let label = crate::models::Tag::post(&pool, &profile, "Team", "#4f8fd6", "split", 0, "").await.unwrap();
        sqlx::query("INSERT INTO object_tag (object_id, tag_id, object_type, source, created_at) VALUES (?1, ?2, 'conversation', 'manual', ?3)")
            .bind(&team)
            .bind(&label.id)
            .bind(Utc::now().to_rfc3339())
            .execute(&pool)
            .await
            .unwrap();
        let input = StatsRangeInput { start: (Utc::now() - Duration::days(30)).to_rfc3339(), end: Utc::now().to_rfc3339(), person_ids: vec![], provider_ids: vec![], tag_ids: vec![] };
        let stats = super::stats(&pool, &profile, &input).await.unwrap();

        let find = |key: &str| stats.rankings.iter().find(|r| r.key == key).unwrap();
        let most = find("most_messages");
        assert_eq!(most.entries[0].name, "Eva Ortega");
        assert_eq!(most.entries[0].value, 7.0);
        assert_eq!(most.entries[0].unit, "messages");
        assert_eq!(most.entries[1].name, "Nora Gale");
        assert_eq!(find("most_characters").entries[0].name, "Nora Gale");
        assert_eq!(find("most_characters").entries[0].unit, "characters");
        assert_eq!(find("longest_average").entries[0].name, "Nora Gale");
        assert_eq!(find("most_questions").entries[0].value, 6.0);
        assert_eq!(find("most_links").entries[0].name, "Eva Ortega");
        assert_eq!(find("most_emoji").entries[0].value, 6.0);
        assert_eq!(find("fastest_reply").entries[0].unit, "minutes");
        assert_eq!(find("fastest_reply").entries[0].value, 10.0);
        let night = find("night_owl");
        assert_eq!(night.entries.len(), 1);
        assert_eq!(night.entries[0].name, "Eva Ortega");
        assert_eq!(night.entries[0].unit, "percent");
        assert!((night.entries[0].value - 100.0).abs() < 0.01);
        assert!(find("early_bird").entries.is_empty());
        assert!(find("longest_silence").entries.iter().all(|e| e.value >= 0.0));
        assert!(stats.rankings.iter().all(|r| r.entries.len() <= 10 && !r.title.is_empty() && !r.description.is_empty()));

        let group = |key: &str| stats.groups.iter().find(|g| g.key == key).unwrap();
        assert_eq!(group("people").kind, "bucket");
        assert_eq!(group("people").messages, 8);
        assert_eq!(group("people").top[0].name, "Eva Ortega");
        assert_eq!(group("teams").messages, 4);
        assert_eq!(group("teams").top[0].name, "Nora Gale");
        assert_eq!(group("teams").people, 1);
        assert_eq!(group("services").messages, 1);
        assert_eq!(group("services").top[0].name, "GitHub");
        assert_eq!(group("mail").messages, 1);
        let split = group(&label.id);
        assert_eq!(split.kind, "split");
        assert_eq!(split.name, "Team");
        assert_eq!(split.messages, 4);
        assert_eq!(split.by_hour.iter().sum::<i64>(), 4);
        assert_eq!(split.busiest_hour, Some(10));

        let narrowed = StatsRangeInput { person_ids: vec![nora_person.clone()], ..input.clone() };
        let narrowed = super::stats(&pool, &profile, &narrowed).await.unwrap();
        assert_eq!(narrowed.totals.messages, 3);
        assert_eq!(narrowed.by_person.len(), 1);
        assert_eq!(narrowed.by_person[0].name, "Nora Gale");
        assert_eq!(narrowed.rankings.iter().find(|r| r.key == "most_messages").unwrap().entries.len(), 1);
        assert_eq!(narrowed.heatmap.iter().flatten().sum::<i64>(), 3);
        let by_conversation = StatsRangeInput { person_ids: vec![eva_person.clone()], ..input.clone() };
        let by_conversation = super::stats(&pool, &profile, &by_conversation).await.unwrap();
        assert_eq!(by_conversation.totals.messages, 8);
        assert_eq!(by_conversation.totals.sent, 1);
    }

    #[tokio::test]
    async fn kinds_files_backlog_split_and_weekday_replies() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "microsoft").await;
        let me = test_user(&pool, "milo@northwind.example", "Milo").await;
        let eva = test_user(&pool, "eva@northwind.example", "Eva").await;
        let bot = test_user(&pool, "notifications@github.com", "GitHub").await;
        sqlx::query("UPDATE profile SET user_id = ?1 WHERE id = ?2").bind(&me).bind(&profile).execute(&pool).await.unwrap();
        let eva_person = seed_person(&pool, &profile, &eva, "Eva Ortega", true).await;
        seed_person(&pool, &profile, &bot, "GitHub", false).await;
        let dm = test_conversation(&pool, "Eva", "direct_message").await;
        link(&pool, &dm, &provider, Some(&eva_person)).await;
        let team = test_conversation(&pool, "Platform", "channel").await;
        link(&pool, &team, &provider, None).await;
        let mail = test_conversation(&pool, "Builds", "email").await;
        link(&pool, &mail, &provider, None).await;

        let monday = {
            let today = chrono::Local::now().date_naive();
            let back = today.weekday().num_days_from_monday() as i64 + 7;
            let naive = (today - Duration::days(back)).and_hms_opt(9, 0, 0).unwrap();
            chrono::Local.from_local_datetime(&naive).unwrap().with_timezone(&Utc)
        };
        seed_message(&pool, &dm, &provider, &eva, "Eva", monday, "Har du sett rapporten?").await;
        seed_message(&pool, &dm, &provider, &me, "Milo", monday + Duration::minutes(4), "Ja").await;
        seed_message(&pool, &dm, &provider, &eva, "Eva", monday + Duration::minutes(10), "Her er filene").await;
        seed_message(&pool, &dm, &provider, &me, "Milo", monday + Duration::days(1), "Takk").await;
        seed_message(&pool, &dm, &provider, &eva, "Eva", monday + Duration::days(1) + Duration::minutes(90), "Bra").await;
        seed_message(&pool, &team, &provider, &eva, "Eva", monday + Duration::hours(2), "Standup flyttes").await;
        seed_message(&pool, &team, &provider, &bot, "GitHub", monday + Duration::hours(3), "Deploy ferdig").await;
        seed_message(&pool, &mail, &provider, &bot, "GitHub", monday + Duration::hours(4), "[SlyOtis/chatnip] CI failed").await;
        seed_message(&pool, &mail, &provider, &eva, "Eva", monday + Duration::hours(5), "Sprintdemo").await;
        sqlx::query("UPDATE message SET type = 'email' WHERE conversation_id = ?1").bind(&mail).execute(&pool).await.unwrap();
        sqlx::query("UPDATE message SET content_type = 'text/calendar' WHERE content = 'Sprintdemo'").execute(&pool).await.unwrap();
        sqlx::query("UPDATE message SET is_read = 0 WHERE content IN ('Bra', 'Deploy ferdig')").execute(&pool).await.unwrap();
        let files = serde_json::json!([
            { "id": "a", "name": "rapport.pdf", "content_type": "application/pdf", "size": 1000, "is_inline": false },
            { "id": "b", "name": "skisse.png", "content_type": "image/png", "size": 500, "is_inline": false },
            { "id": "c", "name": "budsjett.xlsx", "content_type": "application/octet-stream", "size": 200, "is_inline": false },
            { "id": "d", "name": "logo.png", "content_type": "image/png", "size": 50, "is_inline": true }
        ]);
        sqlx::query("UPDATE message SET has_attachments = 1, attachments = ?1 WHERE content = 'Her er filene'").bind(files.to_string()).execute(&pool).await.unwrap();

        let input = StatsRangeInput { start: (Utc::now() - Duration::days(30)).to_rfc3339(), end: Utc::now().to_rfc3339(), person_ids: vec![], provider_ids: vec![], tag_ids: vec![] };
        let stats = super::stats(&pool, &profile, &input).await.unwrap();

        let kind = |k: &str| stats.by_kind.iter().find(|s| s.kind == k).map(|s| (s.messages, s.sent, s.received)).unwrap_or((0, 0, 0));
        assert_eq!(kind("chat"), (5, 2, 3));
        assert_eq!(kind("channel"), (1, 0, 1));
        assert_eq!(kind("bot"), (1, 0, 1));
        assert_eq!(kind("notification"), (1, 0, 1));
        assert_eq!(kind("invite"), (1, 0, 1));
        assert_eq!(kind("mail"), (0, 0, 0));
        assert_eq!(stats.by_kind.iter().map(|k| k.messages).sum::<i64>(), stats.totals.messages);

        assert_eq!(stats.by_file_type.len(), 3);
        assert!(stats.by_file_type.iter().all(|f| f.files == 1));
        assert_eq!(stats.by_file_type.iter().find(|f| f.kind == "pdf").unwrap().bytes, 1000);
        assert_eq!(stats.by_file_type.iter().find(|f| f.kind == "image").unwrap().bytes, 500);
        assert!(stats.by_file_type.iter().any(|f| f.kind == "spreadsheet"));

        assert_eq!(stats.timeline.iter().map(|t| t.unread).sum::<i64>(), 2);
        assert_eq!(stats.totals.human_messages, 5);
        assert_eq!(stats.totals.service_messages, 2);
        assert_eq!(stats.totals.human_messages + stats.totals.service_messages + stats.totals.sent, stats.totals.messages);

        assert_eq!(stats.response_times.iter().map(|b| b.mine).sum::<i64>(), 2);
        assert_eq!(stats.response_times.iter().map(|b| b.theirs).sum::<i64>(), 2);
        assert!(stats.response_times.iter().all(|b| b.mine + b.theirs == b.count));
        assert_eq!(stats.response_times[0].mine, 1);
        assert_eq!(stats.response_times[1].theirs, 1);

        assert_eq!(stats.reply_by_weekday.len(), 7);
        assert_eq!(stats.reply_by_weekday[0].mine, Some(4.0));
        assert_eq!(stats.reply_by_weekday[0].theirs, Some(6.0));
        assert_eq!(stats.reply_by_weekday[1].theirs, Some(90.0));
        assert_eq!(stats.reply_by_weekday[1].mine, Some(1430.0));
        assert_eq!(stats.reply_by_weekday[1].replies, 2);
        assert!(stats.reply_by_weekday[2..].iter().all(|d| d.replies == 0 && d.mine.is_none()));
    }
}
