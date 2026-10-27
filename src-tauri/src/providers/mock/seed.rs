use std::collections::HashMap;
use std::path::Path;

use chrono::{Duration, Local, TimeZone, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{Pool, Row, Sqlite};

use crate::models::action::{Action, ActionGroup, ActionRun, Graph, LogEntry, MessageSchedule, Node, Port, PortType, PostActionGroupInput, PostActionInput, PostMessageScheduleInput, PutActionInput, ACTION_PREFIX, BUILTIN_PREFIX, NODE_SCRIPT, NODE_TRIGGER};
use crate::models::case::{CaseExpectation, CaseFile, PostCaseInput, PostExpectationInput, PutCaseInput, STATUS_DONE};
use crate::models::event::{ORIGIN_MANUAL, ORIGIN_PARSED, STATUS_CONFIRMED, STATUS_PROPOSED};
use crate::models::chat::{Chat, ChatMode, ChatSource, ChatView, PostChatInput, SOURCE_QUERY, SOURCE_TAG};
use crate::models::group::{Group, GroupQuery, PostGroupInput, KIND_MANUAL, KIND_QUERY, KIND_TAG};
use crate::models::inbox::{Inbox, InboxMember, PostInboxInput, KIND_CONVERSATION, KIND_GROUP, KIND_TAG as MEMBER_TAG, KIND_PERSON, KIND_PROVIDER};
use crate::models::tag::{KIND_TAG as TAG_KIND, OBJECT_CONVERSATION, OBJECT_IDENTITY, OBJECT_PERSON};
use crate::models::object_override::{DeleteObjectOverrideInput, PutObjectOverrideInput, FIELD_AVATAR, FIELD_COLOR, FIELD_COMPANY, FIELD_DISPLAY_NAME, FIELD_JOB_TITLE, FIELD_NAME, FIELD_NOTES, SCOPE_CONVERSATION};
use crate::models::content::{ContentGroup, ContentItem, ContentOrigin, PostContentGroupInput, PostContentInput, KIND_EMAIL, KIND_FILE, KIND_PROMPT, KIND_REPLY, KIND_SNIPPET, SOURCE_ACTION, SOURCE_CONVERSATION, SOURCE_MANUAL, SOURCE_MESSAGE, SOURCE_PERSON, SOURCE_UPLOAD, SOURCE_URL};
use crate::models::{Event, EventDraft, ObjectTag, Tag};
use crate::services::action_service::{script_node, trigger_node, wire};

#[derive(Debug, Clone, Default, Serialize)]
pub struct SeedReport {
    pub added: usize,
    pub kept: usize,
    pub synced: usize,
    pub edited: Vec<String>,
}

#[derive(Debug, Clone)]
struct Conversation {
    id: String,
    name: String,
    kind: String,
    conversation_type: String,
    provider_id: String,
    person_id: Option<String>,
}

struct Seed<'a> {
    pool: &'a Pool<Sqlite>,
    profile_id: &'a str,
    providers: Vec<(String, String, String)>,
    conversations: Vec<Conversation>,
    people: Vec<(String, String)>,
    report: SeedReport,
}

pub async fn ledger(pool: &Pool<Sqlite>) -> Result<(), String> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS mock_seed (
            profile_id TEXT NOT NULL,
            key TEXT NOT NULL,
            kind TEXT NOT NULL,
            item_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (profile_id, key)
        )",
    )
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| format!("Failed to create the mock seed ledger: {}", e))?;
    crate::database::ensure_columns(pool, "mock_seed", &[("hash", "TEXT")]).await;
    Ok(())
}

pub fn seed_hash(name: &str, description: &str, icon: Option<&str>, inputs: &[Port], graph: &Graph) -> String {
    let nodes: Vec<Value> = graph.nodes.iter().map(|n| json!({ "id": n.id, "type": n.kind, "label": n.label, "config": n.config, "body": if n.kind == NODE_SCRIPT || n.kind == NODE_TRIGGER { n.body.as_str() } else { "" } })).collect();
    let shape = json!({ "name": name.trim(), "description": description, "icon": icon, "inputs": inputs, "nodes": nodes, "edges": graph.edges });
    crate::utils::content_hash(shape.to_string().as_bytes())
}

static SEEDING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn seed(pool: &Pool<Sqlite>, profile_id: &str, app_data: Option<&Path>) -> Result<SeedReport, String> {
    let _seeding = SEEDING.lock().await;
    ledger(pool).await?;
    let mut s = Seed { pool, profile_id, providers: providers(pool, profile_id).await?, conversations: Vec::new(), people: Vec::new(), report: SeedReport::default() };
    if s.providers.is_empty() {
        return Ok(s.report);
    }
    Tag::ensure_defaults(pool, profile_id).await?;
    Inbox::ensure_defaults(pool, profile_id).await?;
    s.conversations = conversations(pool, profile_id).await?;
    s.people = people(pool, profile_id).await?;
    s.alias().await?;
    s.people = people(pool, profile_id).await?;
    s.history().await?;
    let tags = s.tags().await?;
    s.tag_groups(&tags).await?;
    s.person_tags(&tags).await?;
    let groups = s.inbox_groups().await?;
    s.inboxes(&tags, &groups).await?;
    s.states().await?;
    s.chats(&tags).await?;
    s.presentations().await?;
    s.merge().await?;
    s.threads().await?;
    s.cases().await?;
    s.events().await?;
    if let Some(app_data) = app_data {
        s.content(app_data).await?;
    }
    s.actions(&tags).await?;
    s.receipts().await?;
    s.views().await?;
    s.provenance().await?;
    s.marks().await?;
    s.previews().await?;
    s.layout().await?;
    s.summaries().await?;
    s.object_tags(&tags).await?;
    s.faces().await?;
    Ok(s.report)
}

pub async fn unseed(pool: &Pool<Sqlite>, profile_id: &str, app_data: Option<&Path>) -> Result<usize, String> {
    let _seeding = SEEDING.lock().await;
    ledger(pool).await?;
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT kind, item_id FROM mock_seed WHERE profile_id = ?1")
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to read the mock seed ledger: {}", e))?;
    let order = ["switch", "setting", "sidebar", "alias", "mode", "view", "chat", "receipt_mail", "schedule", "run", "action", "action_group", "inbox", "group", "tag", "case", "event", "content_group", "content", "merge_group", "link", "summary", "pin", "bookmark", "preview", "thread", "override", "picture", "face"];
    let mut removed = 0;
    for kind in order {
        for (_, id) in rows.iter().filter(|(k, _)| k == kind) {
            let outcome = match kind {
                "chat" => Chat::delete(pool, profile_id, id).await,
                "switch" => match id.split_once('|') {
                    Some((action, before)) => sqlx::query("UPDATE action SET enabled = ?1 WHERE id = ?2").bind(before == "true").bind(action).execute(pool).await.map(|_| ()).map_err(|e| e.to_string()),
                    None => Ok(()),
                },
                "setting" => exec(pool, "DELETE FROM app_setting WHERE key = ?1", id).await,
                "sidebar" => match serde_json::from_str::<crate::models::sidebar::SidebarLayout>(id) {
                    Ok(layout) => crate::models::sidebar::SidebarLayout::put(pool, profile_id, &layout).await,
                    Err(_) => exec(pool, "DELETE FROM sidebar_layout WHERE profile_id = ?1", profile_id).await,
                },
                "alias" => async {
                    let alias: Value = serde_json::from_str(id).unwrap_or(Value::Null);
                    sqlx::query("DELETE FROM person_identity WHERE LOWER(email) = ?1 AND manual = 1").bind(alias["email"].as_str().unwrap_or_default()).execute(pool).await.map_err(|e| e.to_string())?;
                    for identity in alias["flipped"].as_array().into_iter().flatten().filter_map(|v| v.as_str()) {
                        exec(pool, "UPDATE person_identity SET manual = 0 WHERE id = ?1", identity).await?;
                    }
                    Ok::<(), String>(())
                }
                .await,
                "mode" => exec(pool, "UPDATE conversation SET mode = 'write' WHERE id = ?1", id).await,
                "view" => exec(pool, "UPDATE conversation SET view = 'chat', view_settings = '{}' WHERE id = ?1", id).await,
                "summary" => exec(pool, "DELETE FROM section_summary WHERE id = ?1", id).await,
                "pin" => exec(pool, "DELETE FROM pinned_message WHERE id = ?1", id).await,
                "bookmark" => exec(pool, "DELETE FROM bookmarked_message WHERE id = ?1", id).await,
                "preview" => exec(pool, "DELETE FROM link_preview WHERE url = ?1", id).await,
                "receipt_mail" => async {
                    for statement in ["DELETE FROM message WHERE conversation_id = ?1", "DELETE FROM conversation_provider WHERE conversation_id = ?1", "DELETE FROM conversation WHERE id = ?1"] {
                        exec(pool, statement, id).await?;
                    }
                    Ok::<(), String>(())
                }
                .await,
                "schedule" => exec(pool, "DELETE FROM message_schedule WHERE id = ?1", id).await,
                "run" => exec(pool, "DELETE FROM action_run WHERE id = ?1", id).await,
                "action" => Action::delete(pool, id).await,
                "action_group" => ActionGroup::delete(pool, id).await,
                "inbox" => Inbox::delete(pool, id).await,
                "group" => Group::delete(pool, id).await,
                "tag" => Tag::delete(pool, id).await,
                "case" => CaseFile::delete(pool, id).await,
                "event" => Event::delete(pool, id).await,
                "content_group" => ContentGroup::delete(pool, profile_id, id).await,
                "content" => match ContentItem::delete(pool, id).await {
                    Ok(Some(item)) => {
                        if let Some(app_data) = app_data {
                            crate::services::content_service::remove_file(app_data, &item);
                        }
                        Ok(())
                    }
                    Ok(None) => Ok(()),
                    Err(e) => Err(e),
                },
                "merge_group" => match id.split_once('|') {
                    Some((lead, source)) => crate::models::MergeGroup::delete_items(pool, profile_id, &crate::models::MergeGroupInput { lead_conversation_id: lead.to_string(), conversation_ids: vec![source.to_string()], group_lead_ids: vec![] }).await,
                    None => Ok(()),
                },
                "link" => {
                    let (source, target) = id.split_once('|').unwrap_or((id.as_str(), ""));
                    sqlx::query("DELETE FROM conversation_link WHERE source_conversation_id = ?1 AND target_conversation_id = ?2")
                        .bind(source)
                        .bind(target)
                        .execute(pool)
                        .await
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                        .and(crate::models::MergeGroup::delete_items(pool, profile_id, &crate::models::MergeGroupInput { lead_conversation_id: target.to_string(), conversation_ids: vec![source.to_string()], group_lead_ids: vec![] }).await)
                }
                "thread" => exec(pool, "DELETE FROM thread WHERE id = ?1", id).await,
                "override" => {
                    let parts: Vec<&str> = id.split('|').collect();
                    match parts.as_slice() {
                        [object_type, object_id, scope_type, scope_id, field] => crate::models::ObjectOverride::delete(pool, &DeleteObjectOverrideInput { object_type: object_type.to_string(), object_id: object_id.to_string(), scope_type: Some(scope_type.to_string()), scope_id: Some(scope_id.to_string()), field: Some(field.to_string()) }).await,
                        _ => Ok(()),
                    }
                }
                "picture" => exec(pool, "UPDATE conversation SET metadata = json_remove(metadata, '$.image') WHERE id = ?1 AND json_valid(metadata)", id).await,
                "face" => async {
                    exec(pool, "UPDATE provider_user SET avatar_data = NULL WHERE user_id = ?1", id).await?;
                    exec(pool, "UPDATE person SET avatar_data = NULL WHERE id IN (SELECT person_id FROM person_identity WHERE user_id = ?1)", id).await
                }
                .await,
                _ => Ok(()),
            };
            if outcome.is_ok() {
                removed += 1;
            }
        }
    }
    sqlx::query("DELETE FROM mock_seed WHERE profile_id = ?1")
        .bind(profile_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to clear the mock seed ledger: {}", e))?;
    if rows.iter().any(|(kind, _)| kind == "alias") {
        crate::services::people_service::rematch(pool, profile_id).await?;
    }
    Ok(removed)
}

async fn exec(pool: &Pool<Sqlite>, sql: &str, id: &str) -> Result<(), String> {
    sqlx::query(sql).bind(id).execute(pool).await.map(|_| ()).map_err(|e| e.to_string())
}

async fn providers(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<(String, String, String)>, String> {
    let rows = sqlx::query("SELECT id, name, COALESCE(config, '') AS kind FROM provider WHERE profile_id = ?1 AND provider_type = 'mock' ORDER BY name")
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list the mock connections: {}", e))?;
    Ok(rows.into_iter().map(|r| (r.get("id"), r.get("name"), r.get("kind"))).collect())
}

async fn conversations(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Conversation>, String> {
    let rows = sqlx::query(
        "SELECT c.id, c.name, c.type, c.person_id, p.id AS provider_id, COALESCE(p.config, '') AS kind FROM conversation c
         JOIN conversation_provider cp ON cp.conversation_id = c.id
         JOIN provider p ON p.id = cp.provider_id
         WHERE p.profile_id = ?1 AND p.provider_type = 'mock'
         ORDER BY c.last_activity DESC",
    )
    .bind(profile_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to list the mock conversations: {}", e))?;
    Ok(rows
        .into_iter()
        .map(|r| Conversation { id: r.get("id"), name: r.get("name"), kind: r.get("kind"), conversation_type: r.get("type"), provider_id: r.get("provider_id"), person_id: r.get("person_id") })
        .collect())
}

async fn people(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<(String, String)>, String> {
    let rows = sqlx::query("SELECT id, display_name FROM person WHERE profile_id = ?1 AND is_human = 1 ORDER BY display_name, created_at, id")
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list people: {}", e))?;
    Ok(rows.into_iter().map(|r| (r.get("id"), r.get("display_name"))).collect())
}

fn port(name: &str, kind: PortType) -> Port {
    Port::new(name, kind)
}

fn builtin(id: &str, key: &str, label: &str, values: Value, x: f64, y: f64) -> Node {
    Node { id: id.into(), kind: format!("{}{}", BUILTIN_PREFIX, key), label: label.into(), config: json!({ "values": values }), body: String::new(), x, y }
}

fn spot(mut node: Node, x: f64, y: f64) -> Node {
    node.x = x;
    node.y = y;
    node
}

pub fn gif(width: u16, height: u16, frames: &[[u8; 3]]) -> Vec<u8> {
    let mut out = b"GIF89a".to_vec();
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&[0x00, 0x00, 0x00]);
    if frames.len() > 1 {
        out.extend_from_slice(&[0x21, 0xFF, 0x0B]);
        out.extend_from_slice(b"NETSCAPE2.0");
        out.extend_from_slice(&[0x03, 0x01, 0x00, 0x00, 0x00]);
    }
    let pixels = width as usize * height as usize;
    for colour in frames {
        out.extend_from_slice(&[0x21, 0xF9, 0x04, 0x04, 0x28, 0x00, 0x00, 0x00]);
        out.push(0x2C);
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.push(0x80);
        out.extend_from_slice(colour);
        out.extend_from_slice(&[0, 0, 0]);
        out.push(0x02);
        let mut bits: Vec<bool> = Vec::new();
        let mut push = |code: u16| {
            for i in 0..3 {
                bits.push(code >> i & 1 == 1);
            }
        };
        let mut left = pixels;
        while left > 0 {
            push(4);
            push(0);
            left -= 1;
            if left > 0 {
                push(0);
                left -= 1;
            }
        }
        push(5);
        let bytes: Vec<u8> = bits.chunks(8).map(|chunk| chunk.iter().enumerate().fold(0u8, |acc, (i, bit)| acc | ((*bit as u8) << i))).collect();
        for block in bytes.chunks(255) {
            out.push(block.len() as u8);
            out.extend_from_slice(block);
        }
        out.push(0x00);
    }
    out.push(0x3B);
    out
}

fn svg_data(svg: &str) -> String {
    use base64::Engine as _;
    format!("data:image/svg+xml;base64,{}", base64::engine::general_purpose::STANDARD.encode(svg))
}

pub fn portrait(index: usize) -> String {
    const SKIN: [&str; 6] = ["#f1c7a5", "#e0ac86", "#c68863", "#a86b4a", "#8a5236", "#f6d5bd"];
    const HAIR: [&str; 6] = ["#2b211c", "#5a3b25", "#a0612f", "#d7b06a", "#1d1d1d", "#7a4b2a"];
    const BACK: [&str; 6] = ["#cfe7d6", "#f6dcc2", "#d5e4ee", "#efe1b8", "#e7d0c9", "#d9e8c4"];
    const SHIRT: [&str; 6] = ["#1f9d55", "#e07b39", "#3b6e8f", "#8c5a3c", "#4a4a4a", "#c9a227"];
    let pick = |list: &[&'static str; 6], step: usize| list[(index * step + step) % list.len()];
    svg_data(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" fill="{}"/><path d="M8 64c2-14 12-20 24-20s22 6 24 20z" fill="{}"/><circle cx="32" cy="28" r="12.5" fill="{}"/><path d="M19.5 27c0-9 5.6-14.5 12.5-14.5S44.5 18 44.5 27c-2.8-4.6-7.4-6.8-12.5-6.8S22.3 22.4 19.5 27z" fill="{}"/></svg>"#,
        pick(&BACK, 1),
        pick(&SHIRT, 5),
        pick(&SKIN, 1),
        pick(&HAIR, 5)
    ))
}

pub fn scene(index: usize) -> String {
    const SKY: [(&str, &str); 4] = [("#f6c27a", "#f08a4b"), ("#bfe3c7", "#4cc77e"), ("#cfe0ea", "#7fb0c9"), ("#f3d7a6", "#d98b5f")];
    const LAND: [(&str, &str); 4] = [("#1f9d55", "#16703d"), ("#e07b39", "#a9561f"), ("#3b6e8f", "#264a61"), ("#6b8f3b", "#476226")];
    let (top, bottom) = SKY[index % SKY.len()];
    let (near, far) = LAND[index % LAND.len()];
    svg_data(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient></defs><rect width="64" height="64" fill="url(#s)"/><circle cx="44" cy="20" r="7" fill="#fff7e6" opacity="0.9"/><path d="M0 46l14-12 12 9 14-15 24 20v16H0z" fill="{}"/><path d="M0 54c10-6 22-8 34-4s22 4 30-2v16H0z" fill="{}"/></svg>"##,
        top, bottom, far, near
    ))
}

pub fn pdf(lines: &[&str]) -> Vec<u8> {
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream, StringFormat};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
    let resources_id = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font_id } });
    let mut operations = vec![Operation::new("BT", vec![]), Operation::new("Tf", vec!["F1".into(), 12.into()]), Operation::new("TL", vec![18.into()]), Operation::new("Td", vec![56.into(), 780.into()])];
    for line in lines {
        let latin: Vec<u8> = line.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
        operations.push(Operation::new("Tj", vec![Object::String(latin, StringFormat::Literal)]));
        operations.push(Operation::new("T*", vec![]));
    }
    operations.push(Operation::new("ET", vec![]));
    let content = Content { operations };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap_or_default()));
    let page_id = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => content_id });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1, "Resources" => resources_id, "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()] }));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut out = Vec::new();
    doc.save_to(&mut out).ok();
    out
}

fn preview_for(url: &str) -> crate::services::link_service::LinkPreview {
    use crate::services::link_service::{host, LinkPreview};
    let site = host(url).unwrap_or_default();
    let path: Vec<&str> = url.split('/').skip(3).map(|p| p.split(['?', '#']).next().unwrap_or_default()).filter(|p| !p.is_empty()).collect();
    let title = path.last().map(|p| p.replace(['-', '_'], " ")).filter(|p| !p.is_empty()).unwrap_or_else(|| site.clone());
    let mut preview = LinkPreview { url: url.to_string(), kind: "article".into(), site_name: Some(site.clone()), title: Some(title), description: Some("Preview from the Chatnip mock server. Nothing was fetched.".into()), theme_color: Some("#e0612f".into()), ..LinkPreview::default() };
    if site.contains("youtube") || site == "youtu.be" {
        preview.kind = "video".into();
        preview.title = Some("Mock video".into());
        preview.duration = Some(213);
    } else if site == "github.com" && path.len() >= 2 {
        preview.repo = Some(format!("{}/{}", path[0], path[1]));
        match (path.get(2).copied(), path.get(3).and_then(|n| n.parse::<u64>().ok())) {
            (Some("pull"), Some(number)) => {
                preview.kind = "pull".into();
                preview.number = Some(number);
                preview.state = Some("open".into());
            }
            (Some("issues"), Some(number)) => {
                preview.kind = "issue".into();
                preview.number = Some(number);
                preview.state = Some("open".into());
            }
            _ => preview.kind = "repo".into(),
        }
    }
    preview
}

impl<'a> Seed<'a> {
    async fn have(&self, key: &str) -> Result<bool, String> {
        let found: Option<String> = sqlx::query_scalar("SELECT item_id FROM mock_seed WHERE profile_id = ?1 AND key = ?2")
            .bind(self.profile_id)
            .bind(key)
            .fetch_optional(self.pool)
            .await
            .map_err(|e| format!("Failed to read the mock seed ledger: {}", e))?;
        Ok(found.is_some())
    }

    async fn item(&self, key: &str) -> Result<Option<String>, String> {
        sqlx::query_scalar("SELECT item_id FROM mock_seed WHERE profile_id = ?1 AND key = ?2")
            .bind(self.profile_id)
            .bind(key)
            .fetch_optional(self.pool)
            .await
            .map_err(|e| format!("Failed to read the mock seed ledger: {}", e))
    }

    async fn note(&mut self, key: &str, kind: &str, id: &str) -> Result<(), String> {
        sqlx::query("INSERT OR IGNORE INTO mock_seed (profile_id, key, kind, item_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)")
            .bind(self.profile_id)
            .bind(key)
            .bind(kind)
            .bind(id)
            .bind(Utc::now().to_rfc3339())
            .execute(self.pool)
            .await
            .map_err(|e| format!("Failed to write the mock seed ledger: {}", e))?;
        self.report.added += 1;
        Ok(())
    }

    async fn skip(&mut self, key: &str) -> Result<bool, String> {
        let have = self.have(key).await?;
        if have {
            self.report.kept += 1;
        }
        Ok(have)
    }

    fn of(&self, kind: &str, types: &[&str]) -> Vec<Conversation> {
        self.conversations.iter().filter(|c| (kind.is_empty() || c.kind == kind) && (types.is_empty() || types.contains(&c.conversation_type.as_str()))).cloned().collect()
    }

    fn provider(&self, kind: &str) -> Option<(String, String)> {
        self.providers.iter().find(|(_, _, k)| k == kind).map(|(id, name, _)| (id.clone(), name.clone()))
    }

    fn person(&self, names: &[&str], fallback: usize) -> Option<String> {
        names
            .iter()
            .find_map(|name| self.people.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)).map(|(id, _)| id.clone()))
            .or_else(|| self.people.get(fallback).map(|(id, _)| id.clone()))
    }

    async fn messages(&self, conversation_id: &str) -> Result<Vec<String>, String> {
        sqlx::query_scalar("SELECT id FROM message WHERE conversation_id = ?1 ORDER BY timestamp")
            .bind(conversation_id)
            .fetch_all(self.pool)
            .await
            .map_err(|e| format!("Failed to list messages: {}", e))
    }

    async fn history(&mut self) -> Result<(), String> {
        if self.skip("history").await? {
            return Ok(());
        }
        sqlx::query("UPDATE provider SET last_sync_at = NULL WHERE profile_id = ?1 AND provider_type = 'mock'")
            .bind(self.profile_id)
            .execute(self.pool)
            .await
            .map_err(|e| format!("Failed to reset the mock sync: {}", e))?;
        self.note("history", "marker", "resync").await
    }

    async fn tags(&mut self) -> Result<HashMap<String, String>, String> {
        let wanted = [
            ("Kunde AS", "#1f9d55", true),
            ("Tidewater", "#e0612f", false),
            ("Faktura", "#d8892a", false),
            ("Kontrakt", "#13857a", false),
            ("Rekruttering", "#6aa83a", false),
            ("Reise", "#1f7fae", false),
            ("Lunsj", "#d0456b", false),
            ("Release", "#e0612f", true),
            ("Design", "#6aa83a", false),
            ("Support", "#4a4642", false),
            ("Privat", "#d0456b", false),
            ("Les senere", "#d8892a", true),
        ];
        let mut ids = HashMap::new();
        let existing = Tag::all(self.pool, self.profile_id).await?;
        for (position, (name, colour, favourite)) in wanted.iter().enumerate() {
            let key = format!("tag:{}", name);
            let id = match self.item(&key).await? {
                Some(id) if existing.iter().any(|l| l.id == id) => {
                    self.report.kept += 1;
                    id
                }
                _ => {
                    let tag = Tag::post(self.pool, self.profile_id, name, colour, TAG_KIND, 100 + position as i64, "").await?;
                    if *favourite {
                        Tag::put_favourite(self.pool, &tag.id, true).await?;
                    }
                    self.note(&key, "tag", &tag.id).await?;
                    tag.id
                }
            };
            ids.insert(name.to_string(), id);
        }
        if !self.skip("tagged").await? {
            let all = self.conversations.clone();
            for (i, conversation) in all.iter().enumerate() {
                let picks = [wanted[i % wanted.len()].0, wanted[(i * 5 + 3) % wanted.len()].0];
                for name in picks.iter().take(1 + i % 2) {
                    if let Some(tag) = ids.get(*name) {
                        ObjectTag::put(self.pool, OBJECT_CONVERSATION, &conversation.id, tag, "manual", None, None).await?;
                    }
                }
            }
            if let Some(starred) = existing.iter().find(|l| l.name == "Starred") {
                for conversation in all.iter().step_by(7).take(4) {
                    ObjectTag::put(self.pool, OBJECT_CONVERSATION, &conversation.id, &starred.id, "manual", None, None).await?;
                }
            }
            self.note("tagged", "marker", "tags").await?;
        }
        Ok(ids)
    }

    async fn group(&mut self, key: &str, name: &str, kind: &str, parent: Option<&str>, tag: Option<&str>, icon: &str) -> Result<String, String> {
        if let Some(id) = self.item(key).await? {
            self.report.kept += 1;
            return Ok(id);
        }
        let input: PostGroupInput = serde_json::from_value(json!({ "name": name, "kind": kind, "parent_id": parent, "tag_id": tag, "icon": icon })).map_err(|e| e.to_string())?;
        let group = Group::post(self.pool, self.profile_id, &input).await?;
        self.note(key, "group", &group.id).await?;
        Ok(group.id)
    }

    async fn person_tags(&mut self, tags: &HashMap<String, String>) -> Result<(), String> {
        if self.skip("person_tags").await? {
            return Ok(());
        }
        let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT pi.person_id, pi.provider_type, pi.external_id, pi.id FROM person_identity pi JOIN person p ON p.id = pi.person_id WHERE p.profile_id = ?1 AND p.is_human = 1 ORDER BY p.display_name, pi.provider_type",
        )
        .bind(self.profile_id)
        .fetch_all(self.pool)
        .await
        .map_err(|e| format!("Failed to list identities: {}", e))?;
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for (person, ..) in &rows {
            *counts.entry(person.as_str()).or_default() += 1;
        }
        let mut people: Vec<(&str, usize)> = counts.into_iter().collect();
        people.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let mut picks = Vec::new();
        if let (Some((person, _)), Some(tag)) = (people.first(), tags.get("Kunde AS")) {
            picks.push((OBJECT_PERSON, person.to_string(), tag.clone()));
            if let (Some((_, _, _, Some(identity))), Some(tag)) = (rows.iter().find(|(p, ..)| p == person), tags.get("Support")) {
                picks.push((OBJECT_IDENTITY, identity.clone(), tag.clone()));
            }
        }
        if let (Some((person, _)), Some(tag)) = (people.get(1), tags.get("Rekruttering")) {
            picks.push((OBJECT_PERSON, person.to_string(), tag.clone()));
        }
        for (object_type, object_id, tag_id) in &picks {
            ObjectTag::put(self.pool, object_type, object_id, tag_id, "manual", None, None).await?;
        }
        self.note("person_tags", "marker", "person_tags").await
    }

    async fn tag_groups(&mut self, tags: &HashMap<String, String>) -> Result<(), String> {
        let clients = self.group("lgroup:clients", "Clients", KIND_MANUAL, None, None, "lucide:building-2").await?;
        let topics = self.group("lgroup:topics", "Topics", KIND_MANUAL, None, None, "lucide:hash").await?;
        for (parent, names) in [(&clients, ["Kunde AS", "Tidewater", "Kontrakt"]), (&topics, ["Release", "Design", "Support"])] {
            for name in names {
                if let Some(tag) = tags.get(name) {
                    self.group(&format!("lgroup:{}", name), name, KIND_TAG, Some(parent), Some(tag), "lucide:tag").await?;
                }
            }
        }
        Ok(())
    }

    async fn inbox_groups(&mut self) -> Result<HashMap<&'static str, String>, String> {
        let mut out = HashMap::new();
        let projects = self.group("igroup:projects", "Projects", KIND_MANUAL, None, None, "lucide:folder-kanban").await?;
        let platform = self.group("igroup:platform", "Tidewater platform", KIND_MANUAL, Some(&projects), None, "lucide:server").await?;
        let family = self.group("igroup:family", "Family", KIND_MANUAL, None, None, "lucide:heart").await?;
        let weekend = self.group("igroup:weekend", "Weekend", KIND_MANUAL, Some(&family), None, "lucide:sun").await?;
        let kunde = self.group("igroup:kunde", "Kunde AS", KIND_MANUAL, Some(&projects), None, "lucide:building-2").await?;
        if self.item("igroup:newsletters").await?.is_none() {
            let query = GroupQuery { regex: Some("unsubscribe|avmeld|nyhetsbrev|newsletter|tldr".into()), humans_only: Some(false), ..GroupQuery::default() };
            let input: PostGroupInput = serde_json::from_value(json!({ "name": "Newsletters and digests", "kind": KIND_QUERY, "parent_id": null, "tag_id": null, "query": query, "icon": "lucide:newspaper" })).map_err(|e| e.to_string())?;
            let group = Group::post(self.pool, self.profile_id, &input).await?;
            self.note("igroup:newsletters", "group", &group.id).await?;
        } else {
            self.report.kept += 1;
        }
        if self.item("igroup:files").await?.is_none() {
            let query = GroupQuery { has_attachments: Some(true), types: vec!["email".into()], ..GroupQuery::default() };
            let input: PostGroupInput = serde_json::from_value(json!({ "name": "Mail with files", "kind": KIND_QUERY, "parent_id": projects, "tag_id": null, "query": query, "icon": "lucide:paperclip" })).map_err(|e| e.to_string())?;
            let group = Group::post(self.pool, self.profile_id, &input).await?;
            self.note("igroup:files", "group", &group.id).await?;
        } else {
            self.report.kept += 1;
        }
        if !self.skip("igroup:items").await? {
            let channels: Vec<String> = self.of("", &["channel", "group_message"]).into_iter().take(3).map(|c| c.id).collect();
            let gmail: Vec<String> = self.of("gmail", &[]).into_iter().take(2).map(|c| c.id).collect();
            Group::add_items(self.pool, &platform, &channels).await?;
            Group::add_items(self.pool, &weekend, &gmail).await?;
            self.note("igroup:items", "marker", "items").await?;
        }
        if !self.skip("igroup:shared").await? {
            let mut shared: Vec<String> = self.of("outlook", &["email"]).into_iter().take(3).map(|c| c.id).collect();
            shared.extend(self.of("", &["channel", "group_message"]).into_iter().take(1).map(|c| c.id));
            shared.extend(self.of("gmail", &[]).into_iter().take(1).map(|c| c.id));
            Group::add_items(self.pool, &kunde, &shared).await?;
            self.note("igroup:shared", "marker", "shared").await?;
        }
        out.insert("projects", projects);
        out.insert("family", family);
        Ok(out)
    }

    async fn inboxes(&mut self, tags: &HashMap<String, String>, groups: &HashMap<&'static str, String>) -> Result<(), String> {
        let member = |kind: &str, id: &str| InboxMember { kind: kind.to_string(), member_id: id.to_string() };
        if !self.skip("inbox:private").await? {
            let mut members = Vec::new();
            if let Some((id, _)) = self.provider("gmail") {
                members.push(member(KIND_PROVIDER, &id));
            }
            for (names, fallback) in [(&["Mia Sund"][..], 0), (&["Eva Ortega"][..], 1)] {
                if let Some(id) = self.person(names, fallback) {
                    members.push(member(KIND_PERSON, &id));
                }
            }
            if let Some(id) = groups.get("family") {
                members.push(member(KIND_GROUP, id));
            }
            let inbox = Inbox::post(self.pool, self.profile_id, &PostInboxInput { name: "Private".into(), icon: Some("lucide:house".into()), members: Some(members) }).await?;
            self.note("inbox:private", "inbox", &inbox.id).await?;
        }
        if !self.skip("inbox:kunde").await? {
            let mut members = Vec::new();
            if let Some((id, _)) = self.provider("outlook") {
                members.push(member(KIND_PROVIDER, &id));
            }
            if let Some(id) = tags.get("Kunde AS") {
                members.push(member(MEMBER_TAG, id));
            }
            if let Some(id) = groups.get("projects") {
                members.push(member(KIND_GROUP, id));
            }
            if let Some(shared) = self.of("gmail", &[]).first() {
                members.push(member(KIND_CONVERSATION, &shared.id));
            }
            let inbox = Inbox::post(self.pool, self.profile_id, &PostInboxInput { name: "Kunde AS".into(), icon: Some("lucide:building-2".into()), members: Some(members) }).await?;
            self.note("inbox:kunde", "inbox", &inbox.id).await?;
        }
        Ok(())
    }

    async fn states(&mut self) -> Result<(), String> {
        if self.conversations.len() < 12 || self.skip("states").await? {
            return Ok(());
        }
        let now = Utc::now();
        let ids: Vec<String> = self.conversations.iter().map(|c| c.id.clone()).collect();
        let set = |sql: &'static str| sql;
        let updates: Vec<(&str, String, Option<String>)> = vec![
            (set("UPDATE conversation SET is_pinned = 1 WHERE id = ?1"), ids[0].clone(), None),
            (set("UPDATE conversation SET is_pinned = 1 WHERE id = ?1"), ids[3].clone(), None),
            (set("UPDATE conversation SET is_muted = 1 WHERE id = ?1"), ids[5].clone(), None),
            (set("UPDATE conversation SET snoozed_until = ?2 WHERE id = ?1"), ids[6].clone(), Some((now + Duration::minutes(20)).to_rfc3339())),
            (set("UPDATE conversation SET snoozed_until = ?2 WHERE id = ?1"), ids[7].clone(), Some((now + Duration::hours(3)).to_rfc3339())),
            (set("UPDATE conversation SET snoozed_until = ?2 WHERE id = ?1"), ids[8].clone(), Some((now + Duration::days(2)).to_rfc3339())),
            (set("UPDATE conversation SET is_archived = 1 WHERE id = ?1"), ids[9].clone(), None),
            (set("UPDATE conversation SET is_archived = 1 WHERE id = ?1"), ids[10].clone(), None),
            (set("UPDATE conversation SET deleted_at = ?2 WHERE id = ?1"), ids[11].clone(), Some(now.to_rfc3339())),
            (set("UPDATE conversation SET unread_count = 3 WHERE id = ?1"), ids[1].clone(), None),
            (set("UPDATE conversation SET unread_count = 7 WHERE id = ?1"), ids[2].clone(), None),
            (set("UPDATE conversation SET unread_count = 1 WHERE id = ?1"), ids[4].clone(), None),
        ];
        for (sql, id, value) in updates {
            let mut query = sqlx::query(sql).bind(id);
            if let Some(value) = value {
                query = query.bind(value);
            }
            query.execute(self.pool).await.map_err(|e| format!("Failed to set a conversation state: {}", e))?;
        }
        self.note("states", "marker", "states").await
    }

    async fn chats(&mut self, tags: &HashMap<String, String>) -> Result<(), String> {
        if !self.have("read_mode").await? {
            if let Some(channel) = self.of("", &["channel"]).first().cloned() {
                sqlx::query("UPDATE conversation SET mode = 'read' WHERE id = ?1")
                    .bind(&channel.id)
                    .execute(self.pool)
                    .await
                    .map_err(|e| format!("Failed to make a channel a read chat: {}", e))?;
                self.note("read_mode", "mode", &channel.id).await?;
            }
        } else {
            self.report.kept += 1;
        }
        let release = GroupQuery { regex: Some("release|deploy|version|changelog|merged|shipped".into()), ..GroupQuery::default() };
        let mut wanted: Vec<(&str, &str, Vec<ChatSource>)> = vec![("chat:release", "Release notes", vec![ChatSource { kind: SOURCE_QUERY.into(), id: None, query: Some(release) }])];
        if let Some(tag) = tags.get("Kunde AS") {
            wanted.push(("chat:kunde", "Kunde AS digest", vec![ChatSource { kind: SOURCE_TAG.into(), id: Some(tag.clone()), query: None }]));
        }
        for (key, name, sources) in wanted {
            if self.skip(key).await? {
                continue;
            }
            let icon = if key == "chat:release" { "lucide:rocket" } else { "lucide:newspaper" };
            let chat = Chat::post(self.pool, self.profile_id, &PostChatInput { name: name.into(), icon: Some(icon.into()), mode: ChatMode::Read, sources, ..Default::default() }).await?;
            self.note(key, "chat", &chat.id).await?;
        }
        if let Some(tag) = tags.get("Kunde AS") {
            if !self.skip("chat:cards").await? {
                let chat = Chat::post(self.pool, self.profile_id, &PostChatInput { name: "Kunde AS cards".into(), icon: Some("lucide:id-card".into()), mode: ChatMode::Read, sources: vec![ChatSource { kind: SOURCE_TAG.into(), id: Some(tag.clone()), query: None }], ..Default::default() }).await?;
                self.note("chat:cards", "chat", &chat.id).await?;
            }
        }
        if !self.skip("chat:week").await? {
            let mut sources = Vec::new();
            for key in ["chat:release", "chat:kunde"] {
                if let Some(chat) = self.chat_conversation(key).await? {
                    sources.push(ChatSource { kind: crate::models::chat::SOURCE_CONVERSATION.into(), id: Some(chat), query: None });
                }
            }
            let chat = Chat::post(self.pool, self.profile_id, &PostChatInput { name: "Week at a glance".into(), icon: Some("lucide:calendar-range".into()), mode: ChatMode::Read, sources, ..Default::default() }).await?;
            self.note("chat:week", "chat", &chat.id).await?;
        }
        let sam: Vec<String> = self.conversations.iter().filter(|c| c.conversation_type == "direct_message" && c.name.to_lowercase().starts_with("sam")).map(|c| c.id.clone()).collect();
        if sam.len() >= 2 && !self.skip("chat:sam").await? {
            let sources = sam.iter().map(|id| ChatSource { kind: crate::models::chat::SOURCE_CONVERSATION.into(), id: Some(id.clone()), query: None }).collect();
            let chat = Chat::post(self.pool, self.profile_id, &PostChatInput { name: "Sam everywhere".into(), icon: Some("lucide:user-round".into()), mode: ChatMode::Write, sources, ..Default::default() }).await?;
            self.note("chat:sam", "chat", &chat.id).await?;
        }
        if !self.skip("chat:notes").await? {
            let chat = Chat::post(self.pool, self.profile_id, &PostChatInput { name: "Notes to self".into(), icon: Some("lucide:sticky-note".into()), mode: ChatMode::Write, ..Default::default() }).await?;
            let notes = [
                (3, "Huskeliste fredag", "- [ ] Send tallene til Anna\n- [x] Book rom til sprintdemo\n- [ ] Svar Mia om illustrasjonene"),
                (2, "Ideer til Q4", "Felles kalender for alle kunder, og en lesechat per prosjekt.\n\n> Spør Nora om budsjett"),
                (0, "Passord til wifi på kontoret", "Står på lappen ved kaffemaskinen, ikke her 🙂"),
            ];
            for (days, title, body) in notes {
                let at = (Utc::now() - Duration::days(days) - Duration::hours(2)).to_rfc3339();
                Chat::put_item(self.pool, &chat.conversation_id, &uuid::Uuid::new_v4().to_string(), crate::models::chat::ITEM_NOTE, title, Some(body), &json!({ "text": body }), &at, (None, None, None)).await?;
            }
            self.note("chat:notes", "chat", &chat.id).await?;
        }
        Ok(())
    }

    async fn receipt_mail(&mut self, kind: &str, slug: &str, subject: &str, from: (&str, &str), body: &str, files: Vec<(&str, &str, Vec<u8>)>, days_ago: i64) -> Result<(), String> {
        let key = format!("receipt_mail:{}:{}", kind, slug);
        if self.skip(&key).await? {
            return Ok(());
        }
        let Some((provider_id, _)) = self.provider(kind) else { return Ok(()) };
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let message_id = uuid::Uuid::new_v4().to_string();
        let external = format!("mock-{}-mail-receipt-{}", kind, slug);
        let at = (Utc::now() - Duration::days(days_ago) - Duration::hours(2 + days_ago % 5)).to_rfc3339();
        use base64::Engine as _;
        let attachments: Vec<Value> = files
            .iter()
            .enumerate()
            .map(|(i, (name, mime, bytes))| json!({ "id": format!("{}-a{}", external, i), "name": name, "content_type": mime, "size": bytes.len(), "is_inline": false, "content_id": null, "url": format!("data:{};base64,{}", mime, base64::engine::general_purpose::STANDARD.encode(bytes)) }))
            .collect();
        sqlx::query("INSERT INTO conversation (id, name, type, group_type, last_activity, unread_count, created_at, updated_at) VALUES (?1, ?2, 'email', 'auto', ?3, 1, ?3, ?3)")
            .bind(&conversation_id)
            .bind(subject)
            .bind(&at)
            .execute(self.pool)
            .await
            .map_err(|e| format!("Failed to add a receipt mail: {}", e))?;
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id, is_primary, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&conversation_id)
            .bind(&provider_id)
            .bind(&external)
            .bind(&at)
            .execute(self.pool)
            .await
            .map_err(|e| format!("Failed to link a receipt mail: {}", e))?;
        sqlx::query(
            "INSERT INTO message (id, provider_id, external_id, type, subject, content, content_type, sender_id, sender_name, sender_email, conversation_id, importance, is_read, is_draft, has_attachments, attachments, created_datetime, last_modified_datetime, received_datetime, timestamp, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'email', ?4, ?5, 'html', ?6, ?7, ?8, ?9, 'normal', 0, 0, ?10, ?11, ?12, ?12, ?12, ?12, ?12, ?12)",
        )
        .bind(&message_id)
        .bind(&provider_id)
        .bind(format!("{}-m0", external))
        .bind(subject)
        .bind(body)
        .bind(format!("mock-{}-x-{}", kind, from.1))
        .bind(from.0)
        .bind(from.1)
        .bind(&conversation_id)
        .bind(!attachments.is_empty())
        .bind((!attachments.is_empty()).then(|| Value::Array(attachments).to_string()))
        .bind(&at)
        .execute(self.pool)
        .await
        .map_err(|e| format!("Failed to add a receipt message: {}", e))?;
        sqlx::query("UPDATE conversation SET last_id = ?1 WHERE id = ?2").bind(&message_id).bind(&conversation_id).execute(self.pool).await.map_err(|e| e.to_string())?;
        self.note(&key, "receipt_mail", &conversation_id).await
    }

    async fn receipts(&mut self) -> Result<(), String> {
        if !self.providers.iter().any(|(_, _, k)| ["outlook", "gmail", "imap"].contains(&k.as_str())) {
            return Ok(());
        }
        let html = |inner: &str| format!("<html><body style=\"font-family:Arial,sans-serif;color:#1f2933\">{}</body></html>", inner);
        self.receipt_mail("outlook", "elkjop", "Kvittering fra Elkjøp – ordre 5521093", ("Elkjøp", "kvittering@elkjop.no"), &html("<p>Hei Milo!</p><p>Takk for at du handlet hos oss. Kvitteringen ligger vedlagt.</p><p>Hilsen Elkjøp</p>"), vec![("Kvittering-Elkjop-5521093.pdf", "application/pdf", pdf(&["Elkjøp Norge AS", "Kvittering", "Org.nr. 974 312 345 MVA", "Dato 03.10.2026", "Ordrenummer 5521093", "AirPods Pro (2. gen) 2 990,00", "USB-C-lader 30 W 299,00", "Totalt 3 289,00 kr", "Herav mva 25 % 657,80", "Betalt med Visa **** 4821"]))], 2).await?;
        self.receipt_mail("gmail", "apple", "Your receipt from Apple", ("Apple", "no_reply@email.apple.com"), &html("<h2>Receipt</h2><p>Order ID: MQ7XK2LP9 · Document no. 192837465 · October 8, 2026</p><table><tr><td>Apple One Individual (Monthly)</td><td>$19.95</td></tr><tr><td>Subtotal</td><td>$18.13</td></tr><tr><td>Tax</td><td>$1.82</td></tr><tr><td><b>Total</b></td><td><b>$19.95</b></td></tr></table><p>Billed to Visa •••• 1234</p>"), vec![], 6).await?;
        self.receipt_mail("imap", "fjord", "Faktura 2026-118 er betalt", ("Fjord Regnskap AS", "faktura@fjordregnskap.no"), &html("<p>Hei!</p><p>Takk, vi har mottatt betalingen. Fakturaen er betalt, og kvitteringen ligger vedlagt.</p>"), vec![("Faktura-2026-118.html", "text/html", html("<h1>Faktura 2026-118 – betalt</h1><p>Fjord Regnskap AS · Org.nr. 998 877 665 MVA</p><p>Betalt 07.10.2026</p><table><tr><td>Regnskapstjenester september</td><td>3 500,00</td></tr><tr><td>MVA 25 %</td><td>875,00</td></tr><tr><td><b>Totalt betalt</b></td><td><b>4 375,00 NOK</b></td></tr></table>").into_bytes())], 8).await?;
        self.receipt_mail("outlook", "amazon", "Your Amazon.de order #305-1234567-7654321", ("Amazon.de", "auto-confirm@amazon.de"), &html("<p>Thank you for your order.</p><p>Order #305-1234567-7654321 placed on 1 October 2026.</p><p>Order total: EUR 42.98 (VAT included 6.86)</p><p>Paid with Mastercard ending in 7731</p>"), vec![], 14).await?;
        self.receipt_mail("imap", "ruter", "Kvittering for kjøp av billett", ("Ruter", "noreply@ruter.no"), &html("<p>Takk for kjøpet! Kvitteringen ligger vedlagt.</p>"), vec![("Ruter-kvittering-30-dager.pdf", "application/pdf", pdf(&["Ruter AS", "Kvittering", "30-dagersbillett voksen, sone 1", "Kjøpt 07.10.2026 kl 08:12", "Transaksjonsnr. 77812345", "Sum 919,00 kr", "Herav mva 12 % 98,46 kr", "Betalt med Vipps"]))], 9).await?;
        self.receipt_mail("gmail", "hosting", "Invoice INV-2026-0419 paid", ("Fjord Hosting", "billing@fjordhosting.com"), &html("<p>Thanks! Your payment was received. The paid invoice is attached.</p>"), vec![("INV-2026-0419.pdf", "application/pdf", pdf(&["Fjord Hosting Ltd", "Invoice INV-2026-0419 - PAID", "Date 2026-10-02", "VPS medium, October 40.00", "VAT 25% 10.00", "Total paid EUR 50.00", "Card ending in 5512"]))], 13).await?;
        self.receipt_mail("outlook", "telenor", "Ny faktura fra Telenor", ("Telenor", "faktura@telenor.no"), &html("<p>Hei! Du har fått en ny faktura. Den ligger vedlagt.</p>"), vec![("Faktura-99812.pdf", "application/pdf", pdf(&["Telenor Norge AS", "Faktura 99812", "Å betale 649,00 kr", "Forfallsdato 30.10.2026", "KID-nummer 0099812001234"]))], 1).await?;
        self.receipt_mail("gmail", "kiwi", "Ukens tilbud hos Kiwi", ("Kiwi", "nyhetsbrev@kiwi.no"), &html("<h1>Ukens tilbud!</h1><p>Kjøp tre, betal for to. Totalt sparer du 199 kr denne uken.</p><p><a href=\"https://kiwi.no/avmeld\">Meld deg av nyhetsbrevet</a></p>"), vec![], 3).await?;
        self.receipt_mail("gmail", "application", "Receipt of your application", ("Acme Careers", "careers@acme.com"), &html("<p>Hello Milo,</p><p>We confirm receipt of your application for the role of product designer. We will get back to you within two weeks.</p>"), vec![], 5).await?;
        self.receipt_mail("imap", "shipped", "Your order has shipped", ("Amazon.de", "shipment-tracking@amazon.de"), &html("<p>Your package is on its way and will arrive on Friday.</p><p>Track your parcel with the link below.</p>"), vec![], 4).await?;

        if self.skip("chat:receipts").await? {
            return Ok(());
        }
        crate::services::action_service::ensure_sources(self.pool, self.profile_id).await?;
        let mail = GroupQuery { types: vec!["email".into()], ..GroupQuery::default() };
        let chat = Chat::post(self.pool, self.profile_id, &PostChatInput { name: "Receipts".into(), icon: Some("lucide:receipt".into()), mode: ChatMode::Read, view: Some(ChatView::Grid), sources: vec![ChatSource { kind: SOURCE_QUERY.into(), id: None, query: Some(mail) }], ..Default::default() }).await?;
        let settings = crate::models::chat::ViewSettings { grid: crate::models::chat::GridSettings { show: vec![crate::models::chat::GridShow::Records, crate::models::chat::GridShow::Files], ..Default::default() }, ..Default::default() };
        sqlx::query("UPDATE conversation SET view_settings = ?1 WHERE id = ?2").bind(serde_json::to_string(&settings).map_err(|e| e.to_string())?).bind(&chat.conversation_id).execute(self.pool).await.map_err(|e| format!("Failed to set the grid of the Receipts chat: {}", e))?;
        let copy = crate::services::action_service::post_action_copy(self.pool, self.profile_id, crate::services::action_service::RECEIPTS_KEY, true, Some("Receipts in a chat")).await?;
        crate::services::chat_service::attach(self.pool, self.profile_id, &chat.conversation_id, &copy.id).await?;
        self.note("action:receipts_chat", "action", &copy.id).await?;
        self.note("chat:receipts", "chat", &chat.id).await
    }

    async fn presentations(&mut self) -> Result<(), String> {
        if self.skip("presentations").await? {
            return Ok(());
        }
        use crate::models::presentation::{put_conversation, put_message};
        if let Some(mail) = self.of("outlook", &["email"]).first() {
            put_conversation(self.pool, &mail.id, &serde_json::json!({ "badge": "Kunde", "highlight": "#e0612f" })).await?;
        }
        if let Some(service) = self.conversations.iter().find(|c| c.name.to_lowercase().contains("newsletter") || c.name.to_lowercase().contains("tldr")) {
            put_conversation(self.pool, &service.id, &serde_json::json!({ "kind": "digest", "summary_only": true })).await?;
        }
        if let Some(channel) = self.of("teams", &["channel"]).first() {
            let message: Option<String> = sqlx::query_scalar("SELECT id FROM message WHERE conversation_id = ?1 ORDER BY timestamp DESC LIMIT 1").bind(&channel.id).fetch_optional(self.pool).await.map_err(|e| e.to_string())?;
            if let Some(message) = message {
                put_message(self.pool, &message, &serde_json::json!({ "highlight": "#1f9d55", "pinned": true, "badge": "Decision" })).await?;
            }
        }
        crate::services::action_service::recognise_invites(self.pool, self.profile_id).await?;
        self.note("presentations", "marker", "presentations").await
    }

    async fn merge(&mut self) -> Result<(), String> {
        if self.skip("merge").await? {
            return Ok(());
        }
        let teams = self.of("teams", &["direct_message"]);
        let slack = self.of("slack", &["direct_message"]);
        let pair = teams.iter().find_map(|t| slack.iter().find(|s| t.person_id.is_some() && s.person_id == t.person_id).map(|s| (t.clone(), s.clone())));
        let Some((target, source)) = pair else { return Ok(()) };
        crate::models::MergeGroup::post(self.pool, self.profile_id, &crate::models::MergeGroupInput { lead_conversation_id: target.id.clone(), conversation_ids: vec![source.id.clone()], group_lead_ids: vec![] }).await?;
        self.note("merge", "merge_group", &format!("{}|{}", target.id, source.id)).await
    }

    async fn threads(&mut self) -> Result<(), String> {
        if self.skip("thread").await? {
            return Ok(());
        }
        let channels = self.of("", &["channel"]);
        for channel in channels {
            let messages = self.messages(&channel.id).await?;
            if messages.len() < 6 {
                continue;
            }
            let root = &messages[messages.len() - 5];
            let id = uuid::Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            sqlx::query("INSERT INTO thread (id, conversation_id, root_message_id, name, last_activity, message_count, is_resolved, created_at, updated_at) VALUES (?1, ?2, ?3, 'Release plan', ?4, 2, 0, ?4, ?4)")
                .bind(&id)
                .bind(&channel.id)
                .bind(root)
                .bind(&now)
                .execute(self.pool)
                .await
                .map_err(|e| format!("Failed to create a thread: {}", e))?;
            for message in [root, &messages[messages.len() - 4], &messages[messages.len() - 3]] {
                sqlx::query("UPDATE message SET thread_id = ?1 WHERE id = ?2").bind(&id).bind(message).execute(self.pool).await.map_err(|e| e.to_string())?;
            }
            return self.note("thread", "thread", &id).await;
        }
        Ok(())
    }

    async fn cases(&mut self) -> Result<(), String> {
        if !self.skip("case:agreement").await? {
            let outlook = self.of("outlook", &["email"]);
            let Some(agreement) = outlook.iter().find(|c| c.name.contains("plattformdrift")).or(outlook.first()).cloned() else { return Ok(()) };
            let messages = self.messages(&agreement.id).await?;
            let input: PostCaseInput = serde_json::from_value(json!({
                "name": "Avtale plattformdrift 2027",
                "description": "Signert avtale og tallene for påskeberedskap.",
                "color": "#1f9d55",
                "due_at": (Utc::now() + Duration::days(5)).to_rfc3339(),
                "conversation_ids": [agreement.id],
                "message_ids": messages.iter().rev().take(2).collect::<Vec<_>>(),
            }))
            .map_err(|e| e.to_string())?;
            let case_id = CaseFile::post(self.pool, self.profile_id, &input).await?;
            for (title, kind, from, filename) in [("Signert kontrakt", "attachment", None, Some("kontrakt")), ("Tall for påskeberedskap", "message", Some("anna"), None)] {
                let expectation = PostExpectationInput { case_id: case_id.clone(), title: title.into(), kind: Some(kind.into()), from_match: from.map(String::from), filename_match: filename.map(String::from), due_at: Some((Utc::now() + Duration::days(3)).to_rfc3339()), ..Default::default() };
                CaseExpectation::post(self.pool, &expectation).await?;
            }
            self.note("case:agreement", "case", &case_id).await?;
        }
        if !self.skip("case:recruiting").await? {
            let ids: Vec<String> = self.of("gmail", &[]).into_iter().take(2).map(|c| c.id).collect();
            if ids.is_empty() {
                return Ok(());
            }
            let input: PostCaseInput = serde_json::from_value(json!({ "name": "Rekruttering designer", "description": "Kandidater og illustrasjoner.", "color": "#e0612f", "conversation_ids": ids })).map_err(|e| e.to_string())?;
            let case_id = CaseFile::post(self.pool, self.profile_id, &input).await?;
            self.note("case:recruiting", "case", &case_id).await?;
        }
        if !self.skip("case:closed").await? {
            let ids: Vec<String> = self.of("outlook", &["email"]).into_iter().rev().take(1).map(|c| c.id).collect();
            let input: PostCaseInput = serde_json::from_value(json!({ "name": "Leverandørbytte kantine", "description": "Avsluttet etter signert avtale med ny leverandør.", "color": "#4a4642", "conversation_ids": ids })).map_err(|e| e.to_string())?;
            let case_id = CaseFile::post(self.pool, self.profile_id, &input).await?;
            CaseFile::put(self.pool, &PutCaseInput { case_id: case_id.clone(), name: None, description: None, status: Some(STATUS_DONE.into()), color: None, due_at: None }).await?;
            self.note("case:closed", "case", &case_id).await?;
        }
        Ok(())
    }

    async fn events(&mut self) -> Result<(), String> {
        let tomorrow = Local::now().date_naive() + Duration::days(1);
        let at = |day: chrono::NaiveDate, hour: u32| Local.from_local_datetime(&day.and_hms_opt(hour, 0, 0).unwrap()).single().unwrap().to_rfc3339();
        if !self.skip("event:proposed").await? {
            let source = self.of("outlook", &["email"]).first().cloned();
            let message = match &source {
                Some(c) => self.messages(&c.id).await?.last().cloned(),
                None => None,
            };
            let draft = EventDraft { title: "Gjennomgang av clusteret".into(), location: "Rom Fjord".into(), starts_at: at(tomorrow, 13), ends_at: Some(at(tomorrow, 14)), attendees: vec!["kari.nordmann@tidewater.example".into()], ..Default::default() };
            let event = Event::post(self.pool, self.profile_id, &draft, ORIGIN_PARSED, STATUS_PROPOSED, 0.62, message.as_deref(), source.as_ref().map(|c| c.id.as_str())).await?;
            self.note("event:proposed", "event", &event.id).await?;
        }
        if !self.skip("event:allday").await? {
            let day = tomorrow + Duration::days(3);
            let draft = EventDraft { title: "Kunde AS offsite".into(), location: "Holmenkollen".into(), starts_at: at(day, 0), ends_at: Some(at(day + Duration::days(1), 0)), all_day: true, ..Default::default() };
            let event = Event::post(self.pool, self.profile_id, &draft, ORIGIN_MANUAL, STATUS_CONFIRMED, 1.0, None, None).await?;
            self.note("event:allday", "event", &event.id).await?;
        }
        for (key, external, title, location, hour, minutes, confidence) in [("event:zoom", "mock-gmail-mail-zoom", "Pricing review with Kunde AS", "Zoom", 14, 60, 0.74), ("event:meet", "mock-gmail-mail-meet", "Skissegjennomgang", "Google Meet", 10, 30, 0.81), ("event:teams", "mock-outlook-mail-teams-invite", "Gjennomgang av clusteret", "Teams", 13, 60, 0.9)] {
            if self.skip(key).await? {
                continue;
            }
            let found: Option<(String, String)> = sqlx::query_as("SELECT m.id, m.conversation_id FROM message m JOIN conversation_provider cp ON cp.conversation_id = m.conversation_id JOIN provider p ON p.id = cp.provider_id WHERE p.profile_id = ?1 AND cp.external_id = ?2 LIMIT 1")
                .bind(self.profile_id)
                .bind(external)
                .fetch_optional(self.pool)
                .await
                .map_err(|e| e.to_string())?;
            let Some((message, conversation)) = found else { continue };
            let starts = Local.from_local_datetime(&tomorrow.and_hms_opt(hour, 0, 0).unwrap()).single().unwrap();
            let draft = EventDraft { title: title.into(), location: location.into(), starts_at: starts.to_rfc3339(), ends_at: Some((starts + Duration::minutes(minutes)).to_rfc3339()), attendees: vec!["milo@northwind.example".into()], ..Default::default() };
            let event = Event::post(self.pool, self.profile_id, &draft, ORIGIN_PARSED, STATUS_PROPOSED, confidence, Some(&message), Some(&conversation)).await?;
            self.note(key, "event", &event.id).await?;
        }
        if !self.skip("event:demo").await? {
            let friday = (0..7).map(|d| Local::now().date_naive() + Duration::days(d)).find(|d| chrono::Datelike::weekday(d) == chrono::Weekday::Fri).unwrap_or(tomorrow);
            let draft = EventDraft { title: "Release demo".into(), location: "Rom Fjord og Teams".into(), notes: "Vis mock serveren, handlinger og statistikk.".into(), starts_at: at(friday, 14), ends_at: Some(at(friday, 15)), attendees: vec!["eva.ortega@northwind.example".into(), "sam.hale@northwind.example".into(), "nora.gale@northwind.example".into()], ..Default::default() };
            let event = Event::post(self.pool, self.profile_id, &draft, ORIGIN_MANUAL, STATUS_CONFIRMED, 1.0, None, None).await?;
            self.note("event:demo", "event", &event.id).await?;
        }
        Ok(())
    }

    async fn hash(&self, key: &str) -> Result<Option<String>, String> {
        sqlx::query_scalar::<_, Option<String>>("SELECT hash FROM mock_seed WHERE profile_id = ?1 AND key = ?2")
            .bind(self.profile_id)
            .bind(key)
            .fetch_optional(self.pool)
            .await
            .map(Option::flatten)
            .map_err(|e| format!("Failed to read the mock seed ledger: {}", e))
    }

    async fn put_hash(&self, key: &str, action: &Action) -> Result<(), String> {
        sqlx::query("UPDATE mock_seed SET hash = ?1 WHERE profile_id = ?2 AND key = ?3")
            .bind(seed_hash(&action.name, &action.description, action.icon.as_deref(), &action.inputs, &action.graph))
            .bind(self.profile_id)
            .bind(key)
            .execute(self.pool)
            .await
            .map(|_| ())
            .map_err(|e| format!("Failed to write the mock seed ledger: {}", e))
    }

    async fn action(&mut self, key: &str, input: PostActionInput) -> Result<Action, String> {
        if let Some(id) = self.item(key).await? {
            if let Ok(action) = Action::get(self.pool, &id).await {
                let current = seed_hash(&action.name, &action.description, action.icon.as_deref(), &action.inputs, &action.graph);
                let edited = match self.hash(key).await? {
                    Some(stored) => stored != current,
                    None => action.updated_at != action.created_at,
                };
                let wanted = seed_hash(&input.name, input.description.as_deref().unwrap_or_default(), input.icon.as_deref(), input.inputs.as_deref().unwrap_or_default(), &input.graph.clone().unwrap_or_default());
                if edited || current == wanted {
                    if edited {
                        self.report.edited.push(action.name.clone());
                    } else {
                        self.put_hash(key, &action).await?;
                    }
                    self.report.kept += 1;
                    return Ok(action);
                }
                let put = PutActionInput { action_id: action.id.clone(), name: Some(input.name), description: input.description, icon: Some(input.icon), inputs: input.inputs, graph: input.graph, ..Default::default() };
                let synced = crate::services::action_service::put_action(self.pool, put).await?;
                self.put_hash(key, &synced).await?;
                self.report.synced += 1;
                return Ok(synced);
            }
        }
        let action = crate::services::action_service::post_action(self.pool, self.profile_id, input).await?;
        self.note(key, "action", &action.id).await?;
        self.put_hash(key, &action).await?;
        Ok(action)
    }

    async fn action_group(&mut self, key: &str, name: &str, parent: Option<&str>, description: &str, icon: &str) -> Result<String, String> {
        if let Some(id) = self.item(key).await? {
            if ActionGroup::get(self.pool, &id).await.is_ok() {
                self.report.kept += 1;
                return Ok(id);
            }
        }
        let input = PostActionGroupInput { name: name.into(), parent_id: parent.map(String::from), description: Some(description.into()), icon: Some(icon.into()) };
        let group = ActionGroup::post(self.pool, self.profile_id, &input).await?;
        self.note(key, "action_group", &group.id).await?;
        Ok(group.id)
    }

    async fn made(&mut self, key: &str, name: &str, description: &str, icon: &str, group: &str, inputs: Vec<Port>, nodes: Vec<Node>, edges: Vec<crate::models::action::Edge>, enabled: bool) -> Result<Action, String> {
        let input = PostActionInput { name: name.into(), description: Some(description.into()), icon: Some(icon.into()), group_id: Some(group.into()), enabled: Some(enabled), inputs: Some(inputs), graph: Some(Graph { nodes, edges }), ..Default::default() };
        self.action(key, input).await
    }

    async fn chat_conversation(&self, key: &str) -> Result<Option<String>, String> {
        match self.item(key).await? {
            Some(id) => Ok(Chat::find(self.pool, self.profile_id, &id).await?.map(|c| c.conversation_id)),
            None => Ok(None),
        }
    }

    async fn actions(&mut self, tags: &HashMap<String, String>) -> Result<(), String> {
        let mine = self.action_group("agroup:mine", "Mine", None, "Actions you made, built from other actions.", "lucide:user").await?;
        let timing = self.action_group("agroup:timing", "Timing", Some(&mine), "Answer, remind and follow up at the right time.", "lucide:clock").await?;
        let behaviours = self.action_group("agroup:behaviours", "Behaviours", Some(&mine), "What happens on its own when you read, answer or receive.", "lucide:toggle-right").await?;
        let routine = self.action_group("agroup:routine", "Routine", Some(&mine), "Briefs and digests on a schedule.", "lucide:calendar-clock").await?;
        let calendar = self.action_group("agroup:calendar", "Calendar and invites", Some(&mine), "Invites, dates and meetings, without AI.", "lucide:calendar").await?;
        let chats = self.action_group("agroup:chats", "Read and write chats", Some(&mine), "Actions that start in a chat or deliver to one.", "lucide:messages-square").await?;
        let blocks = self.action_group("agroup:blocks", "Building blocks", Some(&mine), "Small actions the others use as nodes. Open one inside another to see its frame.", "lucide:blocks").await?;
        let ai = self.action_group("agroup:ai", "Smart", Some(&mine), "These need Intelligence switched on, an AI connection and a default model in Settings, AI.", "lucide:sparkles").await?;
        let conversation = || vec![port("conversation", PortType::Conversation)];
        let called = || trigger_node("called", "Called by another action", json!({ "on": "called" }), "", 0.0);
        let gate = |id: &str, label: &str, inputs: Value, outputs: Value, body: &str| spot(script_node(id, label, inputs, outputs, body, 0.0), 320.0, 160.0);
        let invite_gate = "if invite.confidence >= 0.5 && invite.starts_at != () {\n    #{ event: invite, message: message, options: #{ invite: invite } }\n} else {\n    #{}\n}";

        let polite = self
            .action(
                "action:polite",
                PostActionInput {
                    name: "Polite reply draft".into(),
                    description: Some("Drafts a short, friendly reply. Needs AI in Settings, AI.".into()),
                    icon: Some("lucide:pen-line".into()),
                    group_id: Some(ai.clone()),
                    inputs: Some(conversation()),
                    graph: Some(Graph {
                        nodes: vec![builtin("write", "ai_reply", "Write the reply", json!({ "instruction": "Svar kort og vennlig på norsk. Ingen signatur." }), 320.0, 0.0), builtin("draft", "draft", "Keep as draft", json!({}), 640.0, 0.0)],
                        edges: vec![wire("write", "text", "draft", "text")],
                    }),
                    ..Default::default()
                },
            )
            .await?;
        let important = tags.get("Important").cloned().unwrap_or_default();
        self.action(
            "action:vip",
            PostActionInput {
                name: "VIP triage".into(),
                description: Some("A message in a chat tagged Important gets a reply from Polite reply draft. Needs AI in Settings, AI.".into()),
                icon: Some("lucide:crown".into()),
                group_id: Some(ai.clone()),
                graph: Some(Graph {
                    nodes: vec![
                        trigger_node("arrival", "Message in an Important chat", json!({ "on": "arrival", "scope": { "tag_ids": [important] } }), "", 0.0),
                        Node { id: "reply".into(), kind: format!("{}{}", ACTION_PREFIX, polite.id), label: "Polite reply draft".into(), config: json!({}), body: String::new(), x: 320.0, y: 0.0 },
                    ],
                    edges: vec![wire("arrival", "conversation", "reply", "conversation")],
                }),
                ..Default::default()
            },
        )
        .await?;
        self.action(
            "action:digest",
            PostActionInput {
                name: "Monday digest".into(),
                description: Some("Every Monday at 08:00 the AI sums up last week's unread threads. Needs AI in Settings, AI.".into()),
                icon: Some("lucide:newspaper".into()),
                group_id: Some(routine.clone()),
                graph: Some(Graph {
                    nodes: vec![
                        trigger_node("monday", "Mondays at 08:00", json!({ "on": "schedule", "schedule": { "at": "08:00", "days": [1] } }), "", 0.0),
                        script_node("collect", "Collect unread", json!([]), json!([{ "name": "material", "type": "list" }]), "let found = search(\"is:unread after:-7d\");\n#{ material: found }", 0.0),
                        Node { id: "ask".into(), kind: format!("{}ai", BUILTIN_PREFIX), label: "Summarise the week".into(), config: json!({ "values": { "prompt": "Summarise these threads in five short Norwegian lines, most urgent first." } }), body: String::new(), x: 640.0, y: 0.0 },
                        builtin("tell", "notify", "Notify me", json!({ "title": "Monday digest" }), 960.0, 0.0),
                    ],
                    edges: vec![wire("collect", "material", "ask", "material"), wire("ask", "answer", "tell", "body")],
                }),
                ..Default::default()
            },
        )
        .await?;
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for (i, (provider, name, _)) in self.providers.iter().enumerate() {
            let id = format!("t{}", i);
            nodes.push(trigger_node(&id, &format!("Message arrives in {}", name), json!({ "on": "arrival", "scope": { "connection_ids": [provider] } }), "", i as f64 * 120.0));
            edges.push(wire(&id, "text", "log", "text"));
        }
        nodes.push(script_node("log", "Log it", json!([{ "name": "text", "type": "text" }]), json!([]), "log(text);\n#{}", 0.0));
        self.action("action:connections", PostActionInput { name: "Arrivals per connection".into(), description: Some("One trigger per connected account, each writing the text to the run log.".into()), icon: Some("lucide:plug".into()), group_id: Some(behaviours.clone()), graph: Some(Graph { nodes, edges }), ..Default::default() }).await?;
        self.action(
            "action:archive",
            PostActionInput {
                name: "Archive newsletters".into(),
                description: Some("Switched off: marks newsletters done on arrival.".into()),
                icon: Some("lucide:archive".into()),
                group_id: Some(behaviours.clone()),
                enabled: Some(false),
                graph: Some(Graph {
                    nodes: vec![trigger_node("arrival", "Newsletter arrives", json!({ "on": "arrival" }), "regex_match(text, \"unsubscribe|avmeld\")", 0.0), builtin("done", "done", "Mark done", json!({}), 320.0, 0.0)],
                    edges: vec![wire("arrival", "conversation", "done", "conversation")],
                }),
                ..Default::default()
            },
        )
        .await?;

        let tidy_block = self
            .made(
                "action:block-tidy",
                "Tidy a newsletter",
                "Marks the chat read, folds the message and tags the chat Les senere. Newsletter desk uses it as a node.",
                "lucide:mail-minus",
                &blocks,
                vec![port("conversation", PortType::Conversation), port("message", PortType::Message)],
                vec![called(), builtin("read", "mark_read", "Mark read", json!({}), 320.0, 0.0), builtin("fold", "collapse", "Fold it", json!({}), 320.0, 160.0), builtin("later", "tag_add", "Tag Les senere", json!({ "tag": "Les senere" }), 320.0, 320.0)],
                vec![wire("called", "conversation", "read", "conversation"), wire("called", "message", "fold", "message"), wire("called", "conversation", "later", "conversation")],
                true,
            )
            .await?;
        let lookup_block = self
            .made(
                "action:block-lookup",
                "Look up a chat by id",
                "Finds a chat by its id and gives its five newest messages and who takes part. Who is in this chat uses it as a node; play it alone with the stored mock id.",
                "lucide:search",
                &blocks,
                vec![port("id", PortType::Text)],
                vec![called(), builtin("get", "get_conversation", "Get the chat", json!({}), 320.0, 0.0), builtin("recent", "list_messages", "Five newest", json!({ "limit": 5 }), 640.0, 0.0), builtin("people", "people_of", "Who takes part", json!({}), 640.0, 160.0)],
                vec![wire("called", "id", "get", "id"), wire("get", "conversation", "recent", "conversation"), wire("get", "conversation", "people", "conversation")],
                true,
            )
            .await?;
        if !self.have(&format!("mock:{}", lookup_block.id)).await? {
            if let Some(first) = self.conversations.first().map(|c| c.id.clone()) {
                self.setting(&format!("mock:{}", lookup_block.id), &format!("{}{}", crate::models::app_setting::ACTION_MOCK_PREFIX, lookup_block.id), &json!({ "/id": first }).to_string()).await?;
            }
        }

        if let Some(newsletters) = self.item("igroup:newsletters").await? {
            self.made(
                "action:quiet-newsletters",
                "Quiet newsletters",
                "Whatever the Newsletters and digests filter lets in shows as a digest, is marked read and snoozed until tomorrow 07:00.",
                "lucide:moon",
                &behaviours,
                Vec::new(),
                vec![
                    trigger_node("filter", "Newsletters and digests lets it in", json!({ "on": "filter", "group_ids": [newsletters] }), "", 0.0),
                    builtin("digest", "present_digest", "Show as digest", json!({ "options": { "summary_only": true } }), 320.0, 0.0),
                    builtin("read", "mark_read", "Mark read", json!({}), 320.0, 160.0),
                    builtin("snooze", "snooze", "Snooze till morning", json!({ "wake_at": "tomorrow 07:00" }), 320.0, 320.0),
                ],
                vec![wire("filter", "message", "digest", "message"), wire("filter", "conversation", "read", "conversation"), wire("filter", "conversation", "snooze", "conversation")],
                true,
            )
            .await?;
        }
        self.made(
            "action:newsletter-desk",
            "Newsletter desk",
            "A newsletter arrives: Tidy a newsletter runs on it, and the chat shows every message as a digest from then on.",
            "lucide:newspaper",
            &routine,
            Vec::new(),
            vec![
                trigger_node("arrival", "Looks like a newsletter", json!({ "on": "arrival" }), "regex_match(text, \"unsubscribe|avmeld|nyhetsbrev|newsletter\")", 0.0),
                Node { id: "tidy".into(), kind: format!("{}{}", ACTION_PREFIX, tidy_block.id), label: "Tidy a newsletter".into(), config: json!({}), body: String::new(), x: 320.0, y: 0.0 },
                builtin("look", "present_conversation", "Digest from now on", json!({ "defaults": { "kind": "digest", "summary_only": true } }), 320.0, 200.0),
            ],
            vec![wire("arrival", "conversation", "tidy", "conversation"), wire("arrival", "message", "tidy", "message"), wire("arrival", "conversation", "look", "conversation")],
            true,
        )
        .await?;
        let follow = self
            .made(
                "action:follow-up",
                "Follow up on what I send",
                "Two days after you send something, looks at the chat and reminds you to follow up.",
                "lucide:hourglass",
                &behaviours,
                Vec::new(),
                vec![
                    trigger_node("sent", "You send a message", json!({ "on": "sent" }), "", 0.0),
                    builtin("wait", "wait", "Wait two days", json!({ "minutes": 2880 }), 320.0, 0.0),
                    builtin("recent", "list_messages", "Newest three", json!({ "limit": 3 }), 640.0, 0.0),
                    spot(script_node("still", "Still waiting?", json!([{ "name": "messages", "type": "list" }, { "name": "conversation", "type": "conversation" }]), json!([{ "name": "title", "type": "text" }]), "if messages.len() > 0 {\n    #{ title: \"Follow up: \" + conversation.name }\n} else {\n    #{}\n}", 0.0), 960.0, 0.0),
                    builtin("tell", "notify", "Remind me", json!({ "body": "Ingen har svart ennå." }), 1280.0, 0.0),
                ],
                vec![wire("sent", "conversation", "wait", "value"), wire("wait", "value", "recent", "conversation"), wire("recent", "messages", "still", "messages"), wire("wait", "value", "still", "conversation"), wire("still", "title", "tell", "title")],
                true,
            )
            .await?;
        if tags.contains_key("Les senere") {
            self.made(
                "action:clear-later",
                "Clear Read later when I answer",
                "When you write in a chat tagged Les senere, the tag comes off.",
                "lucide:tag",
                &behaviours,
                Vec::new(),
                vec![trigger_node("sent", "You send a message", json!({ "on": "sent" }), "", 0.0), builtin("untag", "tag_remove", "Remove Les senere", json!({ "tag": "Les senere" }), 320.0, 0.0)],
                vec![wire("sent", "conversation", "untag", "conversation")],
                true,
            )
            .await?;
        }
        self.made(
            "action:show-kinds",
            "Show each kind the way it reads",
            "Six triggers, one look each: bots as bots, joins as system lines, bare files as media, short mail as chat bubbles, long channel posts as mail and no reply mail as quiet notifications.",
            "lucide:shapes",
            &behaviours,
            Vec::new(),
            vec![
                trigger_node("bot", "A bot posts", json!({ "on": "arrival", "message_kinds": ["bot"] }), "", 0.0),
                trigger_node("joined", "Someone joins", json!({ "on": "arrival", "message_kinds": ["system"] }), "regex_match(text, \"joined|ble med\")", 140.0),
                trigger_node("media", "A bare file arrives", json!({ "on": "arrival", "scope": { "types": ["direct_message", "group_message", "channel"], "has_attachments": true } }), "text.len() == 0", 280.0),
                trigger_node("short", "A short mail", json!({ "on": "arrival", "scope": { "types": ["email"] } }), "text.len() < 160", 420.0),
                trigger_node("long", "A long channel post", json!({ "on": "arrival", "scope": { "types": ["channel"] } }), "text.len() > 1500", 560.0),
                trigger_node("notice", "Mail from a no reply address", json!({ "on": "arrival", "scope": { "types": ["email"] } }), "message != () && regex_match(message.sender_email, \"no-?reply|notifications\")", 700.0),
                builtin("as_bot", "present_bot", "Show as bot", json!({}), 320.0, 0.0),
                builtin("as_system", "present_system", "Show as system line", json!({}), 320.0, 140.0),
                builtin("as_media", "present_media", "Show as media", json!({}), 320.0, 280.0),
                builtin("as_chat", "present_chat", "Show as chat", json!({}), 320.0, 420.0),
                builtin("as_mail", "present_mail", "Show as mail", json!({ "options": { "collapsed": false } }), 320.0, 560.0),
                builtin("as_notice", "present_notification", "Show as notification", json!({}), 320.0, 700.0),
            ],
            vec![
                wire("bot", "message", "as_bot", "message"),
                wire("joined", "message", "as_system", "message"),
                wire("media", "message", "as_media", "message"),
                wire("short", "message", "as_chat", "message"),
                wire("long", "message", "as_mail", "message"),
                wire("notice", "message", "as_notice", "message"),
            ],
            true,
        )
        .await?;
        self.made(
            "action:dates",
            "Remind me of dates",
            "When a chat message mentions a day or a time that can be read, you get a notification with it. No AI.",
            "lucide:calendar-days",
            &calendar,
            Vec::new(),
            vec![
                trigger_node("arrival", "Chat message arrives", json!({ "on": "arrival", "scope": { "types": ["direct_message", "group_message"] } }), "", 0.0),
                builtin("time", "has_time", "Mentions a time?", json!({}), 320.0, 0.0),
                spot(script_node("only", "Only with a time", json!([{ "name": "mentions", "type": "bool" }, { "name": "text", "type": "text" }]), json!([{ "name": "text", "type": "text" }]), "if mentions {\n    #{ text: text }\n} else {\n    #{}\n}", 0.0), 640.0, 0.0),
                builtin("read", "parse_date", "Read the date", json!({}), 960.0, 0.0),
                spot(script_node("found", "Could it be read?", json!([{ "name": "value", "type": "any" }]), json!([{ "name": "true", "type": "any" }, { "name": "false", "type": "any" }]), "if value != () {\n    #{ \"true\": value }\n} else {\n    #{ \"false\": value }\n}", 0.0), 1280.0, 0.0),
                builtin("tell", "notify", "Tell me", json!({ "title": "A date came up" }), 1600.0, 0.0),
            ],
            vec![wire("arrival", "text", "time", "text"), wire("time", "mentions", "only", "mentions"), wire("arrival", "text", "only", "text"), wire("only", "text", "read", "text"), wire("read", "at", "found", "value"), wire("found", "true", "tell", "body")],
            true,
        )
        .await?;
        self.made(
            "action:spot-invites",
            "Spot invites without AI",
            "Mail that reads like an invitation, from a calendar file or a Teams, Meet or Zoom link with a time, is proposed in the calendar and shown as an invite card.",
            "lucide:calendar-check",
            &calendar,
            Vec::new(),
            vec![
                trigger_node("arrival", "Mail arrives", json!({ "on": "arrival", "scope": { "types": ["email"] } }), "", 0.0),
                builtin("read", "invite_details", "Read invite details", json!({}), 320.0, 0.0),
                gate("sure", "Sure enough?", json!([{ "name": "invite", "type": "json" }, { "name": "message", "type": "message" }]), json!([{ "name": "event", "type": "json" }, { "name": "message", "type": "message" }, { "name": "options", "type": "json" }]), invite_gate),
                builtin("propose", "propose_event", "Propose it", json!({}), 640.0, 0.0),
                builtin("card", "present_invite", "Show as invite", json!({}), 640.0, 160.0),
            ],
            vec![wire("arrival", "message", "read", "message"), wire("read", "invite", "sure", "invite"), wire("arrival", "message", "sure", "message"), wire("sure", "event", "propose", "event"), wire("sure", "message", "propose", "message"), wire("sure", "message", "card", "message"), wire("sure", "options", "card", "options")],
            true,
        )
        .await?;
        self.made(
            "action:book-invite",
            "Book this invite",
            "Run it on a mail by hand: reads the invite and puts it straight in your calendar, linked to the mail.",
            "lucide:calendar-plus",
            &calendar,
            vec![port("message", PortType::Message)],
            vec![
                trigger_node("manual", "Run on a mail", json!({ "on": "manual" }), "", 0.0),
                builtin("read", "invite_details", "Read invite details", json!({}), 320.0, 0.0),
                gate("sure", "Sure enough?", json!([{ "name": "invite", "type": "json" }, { "name": "message", "type": "message" }]), json!([{ "name": "event", "type": "json" }, { "name": "message", "type": "message" }, { "name": "options", "type": "json" }]), invite_gate),
                builtin("book", "create_event", "Add to calendar", json!({}), 640.0, 0.0),
            ],
            vec![wire("manual", "message", "read", "message"), wire("read", "invite", "sure", "invite"), wire("manual", "message", "sure", "message"), wire("sure", "event", "book", "event"), wire("sure", "message", "book", "message")],
            true,
        )
        .await?;
        self.made(
            "action:meeting-soon",
            "Meeting in ten minutes",
            "Ten minutes before an event in your calendar starts, you get a notification with its title.",
            "lucide:alarm-clock",
            &calendar,
            Vec::new(),
            vec![trigger_node("soon", "An event starts in ten minutes", json!({ "on": "event_soon", "minutes_before": 10 }), "", 0.0), builtin("tell", "notify", "Tell me", json!({ "body": "Starter om ti minutter." }), 320.0, 0.0)],
            vec![wire("soon", "text", "tell", "title")],
            true,
        )
        .await?;
        self.made(
            "action:morning",
            "Morning brief",
            "Weekdays at 07:30, counts the chats that mention today and tells you. No AI.",
            "lucide:sunrise",
            &routine,
            Vec::new(),
            vec![
                trigger_node("morning", "Weekdays at 07:30", json!({ "on": "schedule", "schedule": { "at": "07:30", "days": [1, 2, 3, 4, 5] } }), "", 0.0),
                spot(script_node("count", "Count chats about today", json!([]), json!([{ "name": "title", "type": "text" }, { "name": "body", "type": "text" }]), "let found = search(\"i dag\") + search(\"today\");\n#{ title: \"Morgenoversikt\", body: found.len().to_string() + \" samtaler nevner Kunde AS\" }", 0.0), 320.0, 0.0),
                builtin("tell", "notify", "Tell me", json!({}), 640.0, 0.0),
            ],
            vec![wire("count", "title", "tell", "title"), wire("count", "body", "tell", "body")],
            true,
        )
        .await?;
        self.made(
            "action:who",
            "Who is in this chat",
            "Run it on a chat: Look up a chat by id finds the chat, its newest messages and who takes part.",
            "lucide:users",
            &chats,
            conversation(),
            vec![
                trigger_node("manual", "Run on a chat", json!({ "on": "manual" }), "", 0.0),
                spot(script_node("id", "Its id", json!([{ "name": "conversation", "type": "conversation" }]), json!([{ "name": "id", "type": "text" }]), "#{ id: conversation.id }", 0.0), 320.0, 0.0),
                Node { id: "lookup".into(), kind: format!("{}{}", ACTION_PREFIX, lookup_block.id), label: "Look up a chat by id".into(), config: json!({}), body: String::new(), x: 640.0, y: 0.0 },
            ],
            vec![wire("manual", "conversation", "id", "conversation"), wire("id", "id", "lookup", "id")],
            true,
        )
        .await?;
        self.made(
            "action:reply-later",
            "Answer tomorrow",
            "A direct message about tonight or the weekend gets a polite answer sent tomorrow at 08:00.",
            "lucide:calendar-clock",
            &timing,
            Vec::new(),
            vec![trigger_node("arrival", "About tonight or the weekend", json!({ "on": "arrival", "scope": { "types": ["direct_message"] } }), "regex_match(text, \"i kveld|tonight|helg|weekend\")", 0.0), builtin("later", "schedule_send", "Send tomorrow", json!({ "text": "Takk! Jeg svarer deg i morgen tidlig.", "at": "tomorrow 08:00" }), 320.0, 0.0)],
            vec![wire("arrival", "conversation", "later", "conversation")],
            true,
        )
        .await?;


        self.made(
            "action:ai-ask",
            "Needs an answer?",
            "The AI reads each message that arrives and decides whether it asks you to do something; if so the chat is tagged Les senere and you are told. Needs AI in Settings, AI.",
            "lucide:sparkles",
            &ai,
            Vec::new(),
            vec![
                trigger_node("ai", "AI decides", json!({ "on": "arrival" }), "let answer = ai_json(\"Does this message ask me to do something?\", #{ type: \"object\", properties: #{ yes: #{ type: \"boolean\" } }, required: [\"yes\"] });\nanswer.yes", 0.0),
                builtin("later", "tag_add", "Tag Les senere", json!({ "tag": "Les senere" }), 320.0, 0.0),
                builtin("tell", "notify", "Tell me", json!({ "title": "Needs an answer" }), 320.0, 160.0),
            ],
            vec![wire("ai", "conversation", "later", "conversation"), wire("ai", "text", "tell", "body")],
            true,
        )
        .await?;
        self.made(
            "action:ai-sort",
            "Sort chats by topic",
            "The AI picks Kunde AS, Release, Lunsj or Rekruttering for each chat message and tags the chat. Needs AI in Settings, AI.",
            "lucide:shapes",
            &ai,
            Vec::new(),
            vec![
                trigger_node("arrival", "Chat message arrives", json!({ "on": "arrival", "scope": { "types": ["direct_message", "group_message"] } }), "", 0.0),
                builtin("pick", "classify", "Pick a topic", json!({ "labels": ["Kunde AS", "Release", "Lunsj", "Rekruttering"] }), 320.0, 0.0),
                builtin("tag", "tag_add", "Tag it", json!({}), 640.0, 0.0),
            ],
            vec![wire("arrival", "text", "pick", "text"), wire("pick", "label", "tag", "tag"), wire("arrival", "conversation", "tag", "conversation")],
            true,
        )
        .await?;
        self.made(
            "action:ai-summary",
            "One line on long mail",
            "Long mail gets a one line summary from the AI, which folded mail shows. Needs AI in Settings, AI.",
            "lucide:align-left",
            &ai,
            Vec::new(),
            vec![
                trigger_node("arrival", "A long mail", json!({ "on": "arrival", "scope": { "types": ["email"] } }), "text.len() > 600", 0.0),
                builtin("sum", "summarize", "Sum it up", json!({}), 320.0, 0.0),
                builtin("keep", "store_summary", "Keep the summary", json!({}), 640.0, 0.0),
            ],
            vec![wire("arrival", "text", "sum", "text"), wire("sum", "summary", "keep", "text"), wire("arrival", "message", "keep", "message")],
            true,
        )
        .await?;
        self.made(
            "action:ai-meetings",
            "Propose what the AI finds",
            "The AI reads each mail for meetings, calls and deadlines, and every one it finds is proposed in the calendar. Needs AI in Settings, AI.",
            "lucide:calendar-search",
            &ai,
            Vec::new(),
            vec![
                trigger_node("arrival", "Mail arrives", json!({ "on": "arrival", "scope": { "types": ["email"] } }), "", 0.0),
                builtin("find", "find_events", "Find appointments", json!({}), 320.0, 0.0),
                spot(script_node("each", "Propose each", json!([{ "name": "events", "type": "list" }, { "name": "message", "type": "message" }]), json!([{ "name": "count", "type": "number" }]), "let n = 0;\nfor event in events {\n    propose_event(event, message);\n    n += 1;\n}\n#{ count: n }", 0.0), 640.0, 0.0),
            ],
            vec![wire("arrival", "message", "find", "message"), wire("find", "events", "each", "events"), wire("arrival", "message", "each", "message")],
            true,
        )
        .await?;
        self.made(
            "action:ai-senders",
            "Human or service, checked by AI",
            "The AI decides whether the sender of each message is a person or a service and marks the person. Needs AI in Settings, AI.",
            "lucide:user-search",
            &ai,
            Vec::new(),
            vec![
                trigger_node("arrival", "Message from someone", json!({ "on": "arrival" }), "person != ()", 0.0),
                builtin("decide", "sender_kind", "Human or service", json!({}), 320.0, 0.0),
                builtin("mark", "mark_person", "Mark the person", json!({}), 640.0, 0.0),
            ],
            vec![wire("arrival", "message", "decide", "message"), wire("arrival", "conversation", "decide", "conversation"), wire("arrival", "person", "mark", "person"), wire("decide", "human", "mark", "human")],
            true,
        )
        .await?;
        if self.item("content:Summarise for the board").await?.is_some() {
            self.made(
                "action:ai-board",
                "Summarize",
                "Run it on a chat: the Summarise for the board prompt from Content sums it up, and you get it as a notification. Needs AI in Settings, AI.",
                "lucide:presentation",
                &ai,
                conversation(),
                vec![
                    trigger_node("manual", "Run on a chat", json!({ "on": "manual" }), "", 0.0),
                    builtin("prompt", "prompt", "Summarise for the board", json!({ "key": "Summarise for the board" }), 320.0, 0.0),
                    spot(script_node("line", "Answer line", json!([{ "name": "answer", "type": "json" }]), json!([{ "name": "line", "type": "text" }]), "#{ line: answer.answer }", 0.0), 640.0, 0.0),
                    builtin("tell", "notify", "Tell me", json!({ "title": "For the board" }), 960.0, 0.0),
                ],
                vec![wire("manual", "conversation", "prompt", "material"), wire("prompt", "answer", "line", "answer"), wire("line", "line", "tell", "body")],
                true,
            )
            .await?;
        }
        if self.item("content:Polite decline").await?.is_some() && self.item("content:Signature").await?.is_some() {
            self.made(
                "action:ai-decline",
                "Decline politely, signed",
                "Run it on a chat: the Polite decline prompt writes the answer, your Signature from Content is added, and it waits as a draft. Needs AI in Settings, AI.",
                "lucide:signature",
                &ai,
                conversation(),
                vec![
                    trigger_node("manual", "Run on a chat", json!({ "on": "manual" }), "", 0.0),
                    spot(script_node("write", "Write and sign", json!([{ "name": "conversation", "type": "conversation" }]), json!([{ "name": "text", "type": "text" }]), "let answer = prompt(\"Polite decline\", #{ conversation: conversation });\nlet signature = content(\"Signature\", #{ conversation: conversation });\n#{ text: answer.answer + \"\\n\\n\" + signature }", 0.0), 320.0, 0.0),
                    builtin("keep", "draft", "Keep as draft", json!({}), 640.0, 0.0),
                ],
                vec![wire("manual", "conversation", "write", "conversation"), wire("write", "text", "keep", "text"), wire("manual", "conversation", "keep", "conversation")],
                true,
            )
            .await?;
        }
        self.switches().await?;
        self.runs(&follow).await?;
        self.pipeline().await
    }

    async fn setting(&mut self, key: &str, setting: &str, value: &str) -> Result<(), String> {
        if crate::models::AppSetting::get(self.pool, setting).await?.is_some() {
            return Ok(());
        }
        crate::models::AppSetting::put(self.pool, setting, value).await?;
        self.note(key, "setting", setting).await
    }

    async fn switches(&mut self) -> Result<(), String> {
        if self.skip("switch:classify_senders").await? {
            return Ok(());
        }
        let Ok(mut action) = Action::find(self.pool, self.profile_id, crate::services::action_service::CLASSIFY_SENDERS_KEY).await else { return Ok(()) };
        let before = action.enabled;
        action.enabled = false;
        Action::save(self.pool, &action).await?;
        self.note("switch:classify_senders", "switch", &format!("{}|{}", action.id, before)).await
    }

    async fn runs(&mut self, follow: &Action) -> Result<(), String> {
        let started = Utc::now();
        let cases: [(&str, &str, bool, Option<&str>, i64); 4] = [
            ("sort_splits", "done", false, None, 40),
            ("sort_splits", "done", false, None, 180),
            ("recognise_invites", "failed", false, Some("The AI did not answer within 20 seconds"), 95),
            ("summarise_mail", "done", true, None, 12),
        ];
        for (i, (key, status, dry_run, error, minutes)) in cases.iter().enumerate() {
            let ledger = format!("run:{}", i);
            if self.skip(&ledger).await? {
                continue;
            }
            let Ok(action) = Action::find(self.pool, self.profile_id, key).await else { continue };
            let at = started - Duration::minutes(*minutes);
            let node = action.graph.nodes.last().map(|n| n.id.clone()).unwrap_or_default();
            let log = vec![
                LogEntry { node: action.graph.nodes.first().map(|n| n.id.clone()).unwrap_or_default(), kind: "trigger".into(), status: "done".into(), detail: "Matched".into() },
                LogEntry { node, kind: "script".into(), status: if error.is_some() { "failed".into() } else { "done".into() }, detail: error.map(String::from).unwrap_or_else(|| if *dry_run { "Would store a one line summary".into() } else { "Moved to Important".into() }) },
            ];
            self.run(&ledger, &action, status, *dry_run, *error, at, log, None, Value::Null).await?;
        }
        let entry = |node: &str, kind: &str, status: &str, detail: &str| LogEntry { node: node.into(), kind: kind.into(), status: status.into(), detail: detail.into() };
        let seeded: [(&str, &str, bool, Option<&str>, i64, Vec<LogEntry>); 1] = [
            ("action:ai-summary", "failed", false, Some("Sum it up: No AI connection is set up. Add one under Settings, AI."), 140, vec![entry("arrival", "trigger", "done", "A long mail fired"), entry("sum", "builtin:summarize", "failed", "No AI connection is set up. Add one under Settings, AI.")]),
        ];
        for (key, status, dry_run, error, minutes, log) in seeded {
            let ledger = format!("run:{}", key);
            if self.skip(&ledger).await? {
                continue;
            }
            let Some(id) = self.item(key).await? else { continue };
            let Ok(action) = Action::get(self.pool, &id).await else { continue };
            self.run(&ledger, &action, status, dry_run, error, started - Duration::minutes(minutes), log, None, Value::Null).await?;
        }
        if !self.skip("run:waiting").await? {
            let order = crate::services::action_service::topo_order(&follow.graph)?;
            let cursor = order.iter().position(|n| n == "wait").map(|i| i + 1).unwrap_or(order.len());
            let state = json!({ "order": order, "cursor": cursor, "produced": {}, "dead": [], "payload": {}, "inputs": {}, "trigger": "sent", "chat": null });
            let log = vec![entry("sent", "trigger", "done", "You send a message fired"), entry("wait", "builtin:wait", "waiting", "Paused for 2880 minutes")];
            self.run("run:waiting", follow, "waiting", false, None, started - Duration::hours(3), log, Some((started + Duration::days(2) - Duration::hours(3)).to_rfc3339()), state).await?;
        }
        Ok(())
    }

    async fn run(&mut self, ledger: &str, action: &Action, status: &str, dry_run: bool, error: Option<&str>, at: chrono::DateTime<Utc>, log: Vec<LogEntry>, resume_at: Option<String>, state: Value) -> Result<(), String> {
        let waiting = resume_at.is_some();
        let run = ActionRun {
            id: uuid::Uuid::new_v4().to_string(),
            action_id: action.id.clone(),
            action_name: action.name.clone(),
            status: status.to_string(),
            dry_run,
            trigger: json!({ "on": "arrival" }),
            log,
            outputs: json!({}),
            state,
            error: error.map(String::from),
            resume_at,
            depth: 0,
            started_at: at.to_rfc3339(),
            finished_at: (!waiting).then(|| (at + Duration::seconds(3)).to_rfc3339()),
            result: Value::Null,
            requirement: None,
        };
        ActionRun::save(self.pool, self.profile_id, &run).await?;
        self.note(ledger, "run", &run.id).await
    }

    async fn pipeline(&mut self) -> Result<(), String> {
        let Some(vip) = self.item("action:vip").await? else { return Ok(()) };
        let targets = self.of("", &["direct_message"]);
        let Some(first) = targets.first().cloned() else { return Ok(()) };
        let second = targets.get(1).cloned().unwrap_or_else(|| first.clone());
        if !self.skip("schedule:at").await? {
            let input = PostMessageScheduleInput { conversation_id: second.id.clone(), provider_id: Some(second.provider_id.clone()), content: "Påminnelse: demo kl. 14 i dag.".into(), content_type: Some("text".into()), at: Some((Utc::now() + Duration::hours(3)).to_rfc3339()), action_id: None, metadata: None, attachments: None };
            let schedule = MessageSchedule::post(self.pool, self.profile_id, &input).await?;
            self.note("schedule:at", "schedule", &schedule.id).await?;
        }
        if !self.skip("schedule:action").await? {
            let input = PostMessageScheduleInput { conversation_id: first.id.clone(), provider_id: Some(first.provider_id.clone()), content: "Takk, da sender jeg tallene når du har bekreftet.".into(), content_type: Some("text".into()), at: None, action_id: Some(vip), metadata: None, attachments: None };
            let schedule = MessageSchedule::post(self.pool, self.profile_id, &input).await?;
            self.note("schedule:action", "schedule", &schedule.id).await?;
        }
        Ok(())
    }

    async fn alias(&mut self) -> Result<(), String> {
        const ALIAS: &str = "anna.berg.private@mail.example";
        if self.skip("alias:anna").await? {
            return Ok(());
        }
        let owner = |email: &'static str| {
            sqlx::query_scalar::<_, String>("SELECT pi.person_id FROM person_identity pi JOIN person p ON p.id = pi.person_id WHERE p.profile_id = ?1 AND LOWER(pi.email) = ?2 LIMIT 1").bind(self.profile_id).bind(email).fetch_optional(self.pool)
        };
        let (Some(anna), Some(alias)) = (owner("anna.berg@client.example").await.map_err(|e| e.to_string())?, owner(ALIAS).await.map_err(|e| e.to_string())?) else { return Ok(()) };
        if anna == alias {
            return Ok(());
        }
        let flipped: Vec<String> = sqlx::query_scalar("SELECT id FROM person_identity WHERE manual = 0 AND person_id IN (?1, ?2)").bind(&anna).bind(&alias).fetch_all(self.pool).await.map_err(|e| e.to_string())?;
        let input = crate::models::PostIdentityInput { person_id: anna.clone(), provider_type: "mock".into(), email: Some(ALIAS.into()), external_id: None, display_name: Some("Anna Berg".into()) };
        crate::services::people_service::post_identity(self.pool, self.profile_id, &input).await?;
        self.note("alias:anna", "alias", &json!({ "email": ALIAS, "flipped": flipped }).to_string()).await
    }

    async fn marks(&mut self) -> Result<(), String> {
        if self.skip("marks").await? {
            return Ok(());
        }
        let (pool, profile) = (self.pool, self.profile_id);
        let pick = move |pattern: &'static str| {
            sqlx::query_as::<_, (String, String)>("SELECT m.id, m.conversation_id FROM message m JOIN provider p ON p.id = m.provider_id WHERE p.profile_id = ?1 AND p.provider_type = 'mock' AND m.external_id LIKE ?2 ORDER BY m.timestamp DESC LIMIT 1").bind(profile).bind(pattern).fetch_optional(pool)
        };
        let now = Utc::now().to_rfc3339();
        for pattern in ["%-group-1-plan", "%-mail-contract-m0", "%-channel-0-mention"] {
            if let Some((message, conversation)) = pick(pattern).await.map_err(|e| e.to_string())? {
                let id = uuid::Uuid::new_v4().to_string();
                let added = sqlx::query("INSERT OR IGNORE INTO pinned_message (id, conversation_id, message_id, pinned_by, created_at) VALUES (?1, ?2, ?3, 'current_user', ?4)").bind(&id).bind(&conversation).bind(&message).bind(&now).execute(self.pool).await.map_err(|e| e.to_string())?.rows_affected();
                if added > 0 {
                    self.note(&format!("pin:{}", message), "pin", &id).await?;
                }
            }
        }
        for (pattern, note) in [("%-mail-alias-m0", "Signert kopi, lagre i saken"), ("%-dm-%-files", "Filene fra workshopen"), ("%-mail-zoom-m0", "Prisgjennomgang med Jonas")] {
            if let Some((message, _)) = pick(pattern).await.map_err(|e| e.to_string())? {
                let id = uuid::Uuid::new_v4().to_string();
                let added = sqlx::query("INSERT OR IGNORE INTO bookmarked_message (id, message_id, profile_id, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5)").bind(&id).bind(&message).bind(self.profile_id).bind(note).bind(&now).execute(self.pool).await.map_err(|e| e.to_string())?.rows_affected();
                if added > 0 {
                    self.note(&format!("bookmark:{}", message), "bookmark", &id).await?;
                }
            }
        }
        self.note("marks", "marker", "marks").await
    }

    async fn previews(&mut self) -> Result<(), String> {
        use crate::services::link_service;
        if self.skip("previews").await? {
            return Ok(());
        }
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT COALESCE(m.content, ''), COALESCE(m.content_type, '') FROM message m JOIN provider p ON p.id = m.provider_id WHERE p.profile_id = ?1 AND p.provider_type = 'mock'")
            .bind(self.profile_id)
            .fetch_all(self.pool)
            .await
            .map_err(|e| format!("Failed to read the mock messages: {}", e))?;
        let mut urls: Vec<String> = Vec::new();
        for (content, kind) in rows {
            let links = crate::content::links::extract(&content, (kind == "html").then_some(content.as_str()));
            for url in crate::content::links::preview_candidates(&links, 3) {
                if urls.len() < 150 && !urls.contains(&url) {
                    urls.push(url);
                }
            }
        }
        let forever = Utc::now() + Duration::days(3650);
        for url in urls {
            if link_service::get_cached(self.pool, &url, Utc::now()).await?.is_some() {
                continue;
            }
            link_service::put_cached(self.pool, &url, &Ok(preview_for(&url)), forever).await?;
            self.note(&format!("preview:{}", url), "preview", &url).await?;
        }
        self.note("previews", "marker", "previews").await
    }

    async fn layout(&mut self) -> Result<(), String> {
        use crate::models::sidebar::{SidebarEntry, SidebarLayout, INBOX_PREFIX};
        if self.skip("sidebar").await? {
            return Ok(());
        }
        let before = SidebarLayout::get(self.pool, self.profile_id).await?;
        let inboxes = Inbox::all(self.pool, self.profile_id).await?;
        let inbox = |name: &str| inboxes.iter().find(|i| !i.is_default && i.name.eq_ignore_ascii_case(name)).map(|i| SidebarEntry::Item { key: format!("{}{}", INBOX_PREFIX, i.id) });
        let item = |key: &str| SidebarEntry::Item { key: key.to_string() };
        let divider = || SidebarEntry::Divider { id: uuid::Uuid::new_v4().to_string() };
        let mut top = vec![item("notifications")];
        top.extend(inbox("Important"));
        top.push(divider());
        top.push(SidebarEntry::Folder { id: uuid::Uuid::new_v4().to_string(), name: "Work".into(), icon: Some("lucide:briefcase".into()), collapsed: false, items: inbox("Kunde AS").into_iter().chain(["cases", "groups", "pipeline"].iter().map(|k| item(k))).collect() });
        top.push(SidebarEntry::Folder { id: uuid::Uuid::new_v4().to_string(), name: "Home".into(), icon: Some("lucide:house".into()), collapsed: true, items: inbox("Private").into_iter().collect() });
        top.push(divider());
        top.extend(["inbox", "tags", "done", "snoozed", "deleted"].iter().map(|k| item(k)));
        let footer = ["actions", "content", "calendar", "statistics"].iter().map(|k| item(k)).collect();
        let layout = SidebarLayout { top, footer }.reconcile(&inboxes);
        SidebarLayout::put(self.pool, self.profile_id, &layout).await?;
        self.note("sidebar", "sidebar", &serde_json::to_string(&before).map_err(|e| e.to_string())?).await
    }

    async fn summaries(&mut self) -> Result<(), String> {
        use crate::models::intelligence::{PutSectionSummary, SectionSummary, SECTION_KIND_ALL, SECTION_KIND_SECTION};
        use crate::services::intelligence_service::{section_hash, section_lines};
        if self.skip("summaries").await? {
            return Ok(());
        }
        let picks: [(&str, &[&str]); 4] = [
            ("mock-outlook-mail-agreement", &["Oppdater punkt 3.2 i avtalen", "Send tallene for påskeberedskap"]),
            ("mock-gmail-mail-onboarding", &["Lever fargepaletten til Mia"]),
            ("mock-teams-dm-0", &[]),
            ("mock-slack-dm-3", &["Se over PR-en før fredag"]),
        ];
        for (external, items) in picks {
            let conversation: Option<String> = sqlx::query_scalar("SELECT cp.conversation_id FROM conversation_provider cp JOIN provider p ON p.id = cp.provider_id WHERE p.profile_id = ?1 AND cp.external_id = ?2 LIMIT 1").bind(self.profile_id).bind(external).fetch_optional(self.pool).await.map_err(|e| e.to_string())?;
            let Some(conversation) = conversation else { continue };
            let rows: Vec<(String, String)> = sqlx::query_as("SELECT id, timestamp FROM message WHERE conversation_id = ?1 AND deleted_datetime IS NULL ORDER BY timestamp ASC, id ASC").bind(&conversation).fetch_all(self.pool).await.map_err(|e| e.to_string())?;
            let mut sections: Vec<Vec<String>> = Vec::new();
            let mut previous: Option<chrono::DateTime<Local>> = None;
            for (id, at) in &rows {
                let Ok(at) = chrono::DateTime::parse_from_rfc3339(at).map(|a| a.with_timezone(&Local)) else { continue };
                let split = previous.is_none_or(|p| p.date_naive() != at.date_naive() || at - p >= Duration::minutes(60));
                if split {
                    sections.push(Vec::new());
                }
                if let Some(last) = sections.last_mut() {
                    last.push(id.clone());
                }
                previous = Some(at);
            }
            let all: Vec<String> = rows.iter().map(|(id, _)| id.clone()).collect();
            let mut wanted: Vec<(&str, Vec<String>)> = sections.iter().rev().take(2).map(|s| (SECTION_KIND_SECTION, s.clone())).collect();
            if external.contains("agreement") {
                wanted.push((SECTION_KIND_ALL, all));
            }
            for (kind, ids) in wanted {
                let lines = section_lines(self.pool, &ids).await?;
                let (Some(first), Some(last)) = (lines.first(), lines.last()) else { continue };
                let mut names: Vec<String> = Vec::new();
                for line in &lines {
                    let name = line.sender.split([' ', '.']).next().unwrap_or_default().to_string();
                    if !name.is_empty() && !names.contains(&name) {
                        names.push(name);
                    }
                }
                let opening: String = first.text.split_whitespace().take(12).collect::<Vec<_>>().join(" ");
                let summary = if kind == SECTION_KIND_ALL {
                    "Anna og Jonas ble enige med Milo om fast månedspris, 40 timer i måneden og beredskap i påsken som tillegg. Signert versjon kommer fra juristene.".to_string()
                } else {
                    format!("{} skrev {} meldinger. Det startet med «{}».", names.join(" og "), lines.len(), opening)
                };
                let action_items: Vec<String> = items.iter().map(|s| s.to_string()).collect();
                let hash = section_hash(&lines);
                let saved = SectionSummary::put(self.pool, self.profile_id, PutSectionSummary { scope: &conversation, kind, start_message_id: &first.id, end_message_id: &last.id, content_hash: &hash, message_count: lines.len() as i64, summary: &summary, action_items: &action_items, connection: "Mock", model: None }).await?;
                self.note(&format!("summary:{}:{}:{}", conversation, kind, first.id), "summary", &saved.id).await?;
            }
        }
        self.note("summaries", "marker", "summaries").await
    }

    async fn local(&mut self, object_type: &str, object_id: &str, scope: Option<&str>, field: &str, value: &str) -> Result<(), String> {
        let scope_id = scope.unwrap_or_default();
        let scope_type = if scope.is_some() { SCOPE_CONVERSATION } else { "" };
        let key = format!("{}|{}|{}|{}|{}", object_type, object_id, scope_type, scope_id, field);
        if self.skip(&format!("override:{}", key)).await? {
            return Ok(());
        }
        let input = PutObjectOverrideInput { object_type: object_type.into(), object_id: object_id.into(), scope_type: scope.map(|_| SCOPE_CONVERSATION.to_string()), scope_id: scope.map(String::from), field: field.into(), value: Some(value.into()) };
        crate::models::ObjectOverride::put(self.pool, &input).await?;
        self.note(&format!("override:{}", key), "override", &key).await
    }

    async fn faces(&mut self) -> Result<(), String> {
        let photographed = ["Eva Ortega", "Nora Gale", "Sam Hale", "Anna Berg", "Kari Nordmann", "Mia Sund"];
        for (index, name) in photographed.iter().enumerate() {
            let Some(person) = self.people.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)).map(|(id, _)| id.clone()) else { continue };
            let users: Vec<String> = sqlx::query_scalar("SELECT user_id FROM person_identity WHERE person_id = ?1 AND user_id IS NOT NULL").bind(&person).fetch_all(self.pool).await.map_err(|e| e.to_string())?;
            let photo = portrait(index);
            for user in users {
                if self.skip(&format!("face:{}", user)).await? {
                    continue;
                }
                let changed = sqlx::query("UPDATE provider_user SET avatar_data = ?1 WHERE user_id = ?2 AND avatar_data IS NULL").bind(&photo).bind(&user).execute(self.pool).await.map_err(|e| e.to_string())?.rows_affected();
                if changed > 0 {
                    sqlx::query("UPDATE person SET avatar_data = ?1 WHERE id = ?2 AND avatar_data IS NULL").bind(&photo).bind(&person).execute(self.pool).await.map_err(|e| e.to_string())?;
                    self.note(&format!("face:{}", user), "face", &user).await?;
                }
            }
        }
        let channels = self.of("", &["channel"]);
        let groups = self.of("", &["group_message"]);
        for (index, conversation) in channels.iter().take(1).chain(groups.iter().skip(1).take(1)).enumerate() {
            if self.skip(&format!("picture:{}", conversation.id)).await? {
                continue;
            }
            let changed = sqlx::query("UPDATE conversation SET metadata = json_set(CASE WHEN json_valid(metadata) THEN metadata ELSE '{}' END, '$.image', ?1) WHERE id = ?2 AND (metadata IS NULL OR NOT json_valid(metadata) OR json_extract(metadata, '$.image') IS NULL)")
                .bind(scene(index))
                .bind(&conversation.id)
                .execute(self.pool)
                .await
                .map_err(|e| e.to_string())?
                .rows_affected();
            if changed > 0 {
                self.note(&format!("picture:{}", conversation.id), "picture", &conversation.id).await?;
            }
        }
        if let Some(lea) = self.person(&["Lea Lindqvist"], 2) {
            for (field, value) in [(FIELD_DISPLAY_NAME, "Lea (PL)"), (FIELD_JOB_TITLE, "Prosjektleder, Chatnip"), (FIELD_COMPANY, "Northwind"), (FIELD_NOTES, "Foretrekker Teams før lunsj. Vil ha statusmøtet på mandager."), (FIELD_COLOR, "#e07b39")] {
                self.local(OBJECT_PERSON, &lea, None, field, value).await?;
            }
        }
        if let Some(group) = groups.first() {
            self.local(OBJECT_CONVERSATION, &group.id, None, FIELD_NAME, "Kaffeklubben").await?;
            self.local(OBJECT_CONVERSATION, &group.id, None, FIELD_AVATAR, &scene(2)).await?;
            let member: Option<String> = sqlx::query_scalar(
                "SELECT pi.person_id FROM conversation_member cm JOIN person_identity pi ON pi.user_id = cm.user_id JOIN person p ON p.id = pi.person_id
                 WHERE cm.conversation_id = ?1 AND p.profile_id = ?2 AND p.is_human = 1 ORDER BY p.display_name LIMIT 1",
            )
            .bind(&group.id)
            .bind(self.profile_id)
            .fetch_optional(self.pool)
            .await
            .map_err(|e| e.to_string())?;
            if let Some(member) = member {
                self.local(OBJECT_PERSON, &member, Some(&group.id), FIELD_DISPLAY_NAME, "Kapteinen").await?;
            }
        }
        Ok(())
    }

    async fn object_tags(&mut self, tags: &HashMap<String, String>) -> Result<(), String> {
        use crate::models::tag::{ObjectTag, OBJECT_ACTION_GROUP, OBJECT_CONTENT, OBJECT_EVENT, OBJECT_GROUP, OBJECT_IDENTITY, OBJECT_INBOX, OBJECT_MESSAGE};
        if self.skip("object_tags").await? {
            return Ok(());
        }
        let message = |pattern: &'static str| {
            sqlx::query_scalar::<_, String>("SELECT m.id FROM message m JOIN provider p ON p.id = m.provider_id WHERE p.profile_id = ?1 AND p.provider_type = 'mock' AND m.external_id LIKE ?2 ORDER BY m.timestamp DESC LIMIT 1").bind(self.profile_id).bind(pattern).fetch_optional(self.pool)
        };
        let mut picks: Vec<(&str, String, &str)> = Vec::new();
        for (pattern, tag) in [("%-mail-contract-m0", "Kontrakt"), ("%-group-1-plan", "Release"), ("%-dm-%-files", "Design"), ("%-mail-alias-m0", "Kunde AS"), ("%-channel-0-mention", "Support")] {
            if let Some(id) = message(pattern).await.map_err(|e| e.to_string())? {
                picks.push((OBJECT_MESSAGE, id, tag));
            }
        }
        for (key, object, tag) in [
            ("event:allday", OBJECT_EVENT, "Kunde AS"),
            ("event:demo", OBJECT_EVENT, "Release"),
            ("igroup:platform", OBJECT_GROUP, "Tidewater"),
            ("igroup:kunde", OBJECT_GROUP, "Kunde AS"),
            ("inbox:kunde", OBJECT_INBOX, "Kunde AS"),
            ("agroup:behaviours", OBJECT_ACTION_GROUP, "Support"),
            ("content:Price list.pdf", OBJECT_CONTENT, "Kunde AS"),
            ("content:Referat workshop", OBJECT_CONTENT, "Design"),
        ] {
            if let Some(id) = self.item(key).await? {
                picks.push((object, id, tag));
            }
        }
        for key in ["chat:week", "chat:notes"] {
            if let Some(chat) = self.chat_conversation(key).await? {
                picks.push((crate::models::tag::OBJECT_CONVERSATION, chat, "Les senere"));
            }
        }
        let identity: Option<String> = sqlx::query_scalar("SELECT pi.id FROM person_identity pi JOIN person p ON p.id = pi.person_id WHERE p.profile_id = ?1 AND LOWER(pi.email) = 'jonas.lie@client.example' LIMIT 1").bind(self.profile_id).fetch_optional(self.pool).await.map_err(|e| e.to_string())?;
        if let Some(identity) = identity {
            picks.push((OBJECT_IDENTITY, identity, "Kunde AS"));
        }
        for (object, id, tag) in picks {
            if let Some(tag) = tags.get(tag) {
                ObjectTag::put(self.pool, object, &id, tag, "manual", None, None).await?;
            }
        }
        self.note("object_tags", "marker", "object_tags").await
    }

    async fn views(&mut self) -> Result<(), String> {
        if !self.have("view:receipts").await? {
            if let Some(receipts) = self.chat_conversation("chat:receipts").await? {
                let settings = json!({ "chat": { "sources": true }, "grid": { "group": "source", "sort": "size", "direction": "asc", "rows": "wrap", "tags": true, "show": ["records", "files"] } });
                sqlx::query("UPDATE conversation SET view_settings = ?1 WHERE id = ?2").bind(settings.to_string()).bind(&receipts).execute(self.pool).await.map_err(|e| e.to_string())?;
                self.note("view:receipts", "marker", &receipts).await?;
            }
        } else {
            self.report.kept += 1;
        }
        if !self.have("view:files").await? {
            let files: Option<String> = sqlx::query_scalar("SELECT m.conversation_id FROM message m JOIN provider p ON p.id = m.provider_id WHERE p.profile_id = ?1 AND p.provider_type = 'mock' AND m.external_id LIKE '%-dm-%-files' LIMIT 1").bind(self.profile_id).fetch_optional(self.pool).await.map_err(|e| e.to_string())?;
            if let Some(files) = files {
                let settings = json!({ "grid": { "group": "kind", "sort": "date", "direction": "desc", "rows": "scroll", "tags": true, "show": ["files", "media", "links"] } });
                sqlx::query("UPDATE conversation SET view = 'grid', view_settings = ?1 WHERE id = ?2").bind(settings.to_string()).bind(&files).execute(self.pool).await.map_err(|e| e.to_string())?;
                self.note("view:files", "view", &files).await?;
            }
        } else {
            self.report.kept += 1;
        }
        Ok(())
    }

    async fn provenance(&mut self) -> Result<(), String> {
        if self.skip("provenance").await? {
            return Ok(());
        }
        for (content, action) in [("content:Action items", "action:vip"), ("content:Summarise for the board", "action:ai-board")] {
            if let (Some(content), Some(action)) = (self.item(content).await?, self.item(action).await?) {
                sqlx::query("UPDATE content_item SET source_kind = ?1, source_action_id = ?2 WHERE id = ?3 AND source_action_id IS NULL").bind(SOURCE_ACTION).bind(&action).bind(&content).execute(self.pool).await.map_err(|e| e.to_string())?;
            }
        }
        self.note("provenance", "marker", "provenance").await
    }

    async fn content_group(&mut self, key: &str, name: &str, parent: Option<&str>, icon: &str) -> Result<Option<String>, String> {
        if let Some(id) = self.item(key).await? {
            self.report.kept += 1;
            return Ok(ContentGroup::get(self.pool, self.profile_id, &id).await.ok().map(|g| g.id));
        }
        let input = PostContentGroupInput { name: name.into(), parent_id: parent.map(String::from), icon: Some(icon.into()), ..PostContentGroupInput::default() };
        let group = ContentGroup::post(self.pool, self.profile_id, &input).await?;
        self.note(key, "content_group", &group.id).await?;
        Ok(Some(group.id))
    }

    async fn content(&mut self, app_data: &Path) -> Result<(), String> {
        let own: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content_item WHERE profile_id = ?1 AND id NOT IN (SELECT item_id FROM mock_seed WHERE profile_id = ?1 AND kind = 'content')")
            .bind(self.profile_id)
            .fetch_one(self.pool)
            .await
            .map_err(|e| format!("Failed to count the content library: {}", e))?;
        if own > 0 {
            return Ok(());
        }
        let known: Vec<String> = Tag::all(self.pool, self.profile_id).await?.into_iter().map(|t| t.id).collect();
        let reactions = self.content_group("content_group:reactions", "Reactions", None, "lucide:smile-plus").await?;
        let celebrations = self.content_group("content_group:celebrations", "Celebrations", reactions.as_deref(), "lucide:sparkles").await?;
        let work = self.content_group("content_group:work", "Work", None, "lucide:briefcase").await?;
        let customers = self.content_group("content_group:customers", "Customer replies", work.as_deref(), "lucide:users").await?;
        let office = self.content_group("content_group:office", "Office", None, "lucide:building-2").await?;
        let photos = self.content_group("content_group:photos", "Photos", None, "lucide:camera").await?;
        let items: [(&str, &str, &[&str], Vec<Option<String>>, [u16; 2], &[[u8; 3]], u32, bool); 20] = [
            ("Party parrot", "gif", &["party", "yay"], vec![reactions.clone(), celebrations.clone()], [64, 64], &[[255, 90, 90], [90, 200, 255], [120, 255, 120]], 31, true),
            ("Ship it", "gif", &["release", "ship"], vec![work.clone()], [96, 54], &[[31, 157, 85], [224, 97, 47]], 18, true),
            ("Facepalm", "gif", &["oops", "facepalm"], vec![reactions.clone()], [80, 80], &[[216, 137, 42], [74, 70, 66]], 12, false),
            ("Thumbs up", "gif", &["yes", "ok"], vec![reactions.clone()], [72, 72], &[[106, 168, 58], [31, 157, 85]], 44, true),
            ("Mind blown", "gif", &["wow", "mind"], vec![reactions.clone()], [96, 64], &[[208, 69, 107], [224, 97, 47], [255, 210, 80]], 7, false),
            ("Coffee time", "gif", &["coffee", "pause"], vec![office.clone()], [64, 96], &[[110, 70, 40], [190, 140, 90]], 9, false),
            ("Deploy friday", "gif", &["deploy", "friday"], vec![work.clone(), celebrations.clone()], [96, 64], &[[220, 60, 50], [255, 160, 40]], 5, false),
            ("Celebrate", "gif", &["party", "win"], vec![celebrations.clone()], [80, 60], &[[255, 210, 80], [31, 127, 174]], 16, false),
            ("Typing fast", "gif", &["busy", "typing"], vec![office.clone()], [96, 60], &[[19, 133, 122], [60, 60, 60]], 3, false),
            ("Rocket", "gif", &["launch", "release"], vec![work.clone()], [60, 96], &[[31, 127, 174], [224, 97, 47]], 11, false),
            ("Fjord", "image", &["norway", "nature"], vec![photos.clone()], [120, 80], &[[31, 127, 174]], 2, false),
            ("Meadow", "image", &["green", "summer"], vec![photos.clone()], [120, 80], &[[106, 168, 58]], 1, false),
            ("Sunset", "image", &["orange", "evening"], vec![photos.clone()], [120, 70], &[[224, 97, 47]], 4, false),
            ("Whiteboard", "image", &["plan", "sketch"], vec![work.clone()], [100, 100], &[[245, 243, 238]], 6, false),
            ("Architecture", "image", &["diagram", "cluster"], vec![work.clone()], [140, 90], &[[60, 70, 90]], 2, false),
            ("Team photo", "image", &["team", "people"], vec![photos.clone(), office.clone()], [120, 90], &[[200, 170, 140]], 0, false),
            ("Cat", "sticker", &["cat", "cute"], vec![reactions.clone()], [64, 64], &[[230, 150, 60]], 21, true),
            ("Dog", "sticker", &["dog", "cute"], vec![], [64, 64], &[[170, 120, 80]], 8, false),
            ("Heart", "sticker", &["love", "heart"], vec![reactions.clone()], [64, 64], &[[208, 69, 107]], 14, false),
            ("Check", "sticker", &["done", "ok"], vec![], [64, 64], &[[31, 157, 85]], 10, false),
        ];
        let sources: Vec<(String, String, String)> = {
            let mut out = Vec::new();
            for conversation in self.of("", &["channel", "group_message"]).into_iter().take(3) {
                if let Some(message) = self.messages(&conversation.id).await?.last() {
                    out.push((message.clone(), conversation.id.clone(), conversation.provider_id.clone()));
                }
            }
            out
        };
        for (i, (name, kind, tags, groups, size, frames, uses, favourite)) in items.iter().enumerate() {
            let key = format!("content:{}", name);
            if self.skip(&key).await? {
                continue;
            }
            let input = PostContentInput { kind: kind.to_string(), name: name.to_string(), tags: Some(tags.iter().map(|t| t.to_string()).collect()), keywords: Some(tags.iter().map(|t| t.to_string()).collect()), is_favourite: Some(*favourite), group_ids: Some(groups.iter().flatten().cloned().collect()), ..PostContentInput::default() };
            let origin = match sources.get(i) {
                Some((message, conversation, provider)) if i < 3 => ContentOrigin {
                    source_kind: Some(SOURCE_MESSAGE.into()),
                    source_message_id: Some(message.clone()),
                    source_conversation_id: Some(conversation.clone()),
                    source_provider_id: Some(provider.clone()),
                    ..Default::default()
                },
                _ => ContentOrigin::default(),
            };
            let item = crate::services::content_service::store_bytes(self.pool, app_data, self.profile_id, input, origin, gif(size[0], size[1], frames)).await?;
            for _ in 0..*uses {
                ContentItem::touch(self.pool, &item.id).await?;
            }
            self.note(&key, "content", &item.id).await?;
        }
        let person = self.people.first().map(|(id, _)| id.clone());
        let mail = self.of("", &["email"]).first().map(|c| c.id.clone());
        let action = self.item("action:vip").await?;
        let templates: [(&str, &str, &str, &[&str], Vec<Option<String>>, ContentOrigin, u32); 9] = [
            ("Thanks, on it", KIND_REPLY, "Thanks {{ .person.first_name | default \"there\" }}, I am on it and will get back to you today.", &["thanks"], vec![customers.clone()], ContentOrigin { source_kind: Some(SOURCE_PERSON.into()), source_person_id: person.clone(), ..Default::default() }, 12),
            ("Running late", KIND_REPLY, "Running about ten minutes late, {{ .person.first_name | default \"everyone\" }}. Start without me.", &["late"], vec![office.clone()], ContentOrigin { source_kind: Some(SOURCE_MANUAL.into()), ..Default::default() }, 4),
            ("Invoice received", KIND_EMAIL, "Hi {{ .person.first_name | default \"there\" }},\n\nThanks, the invoice in {{ .message.subject | default \"your last email\" }} is received and goes to payment this week.\n\nBest regards,\n{{ .me.name }}", &["invoice", "faktura"], vec![customers.clone(), work.clone()], ContentOrigin { source_kind: Some(SOURCE_CONVERSATION.into()), source_conversation_id: mail.clone(), ..Default::default() }, 6),
            ("Out of office", KIND_EMAIL, "Hi {{ .person.first_name | default \"there\" }},\n\nI am away until {{ .today | date \"Monday 2 January\" }}{{ if .instruction }} and {{ .instruction }}{{ end }}. I will answer when I am back.\n\n{{ .me.name }}", &["away"], vec![], ContentOrigin { source_kind: Some(SOURCE_MANUAL.into()), ..Default::default() }, 1),
            ("Polite decline", KIND_PROMPT, "Write a short, friendly reply to {{ .person.name | default \"the sender\" }} that declines what the latest message asks for, in the language the thread uses.{{ if .instruction }} Keep this in mind: {{ .instruction }}{{ end }}\n\n{{ range .messages }}[{{ .time }}] {{ .sender }}: {{ .content }}\n{{ end }}", &["decline"], vec![customers.clone()], ContentOrigin { source_kind: Some(SOURCE_MANUAL.into()), ..Default::default() }, 0),
            ("Action items", KIND_PROMPT, "List the concrete action items for {{ .me.first_name }} in {{ .conversation.name }}, one per line, newest first.\n\n{{ range .messages }}{{ .sender }}: {{ .content }}\n{{ end }}", &["todo"], vec![work.clone()], ContentOrigin { source_kind: Some(SOURCE_ACTION.into()), source_action_id: action.clone(), ..Default::default() }, 0),
            ("Signature", KIND_SNIPPET, "{{ .me.name }}\nNorthwind AS{{ if .me.email }} · {{ .me.email }}{{ end }}", &["signature"], vec![work.clone()], ContentOrigin { source_kind: Some(SOURCE_MANUAL.into()), ..Default::default() }, 9),
            ("Meeting link", KIND_SNIPPET, "Join here: https://meet.example.com/milo ({{ .now | date \"15:04\" }} works for me)", &["meeting"], vec![office.clone()], ContentOrigin { source_kind: Some(SOURCE_URL.into()), ..Default::default() }, 3),
            ("Price list link", KIND_SNIPPET, "{{ .person.first_name | default \"Hi\" }}, the current price list is at https://northwind.example/priser", &["prices"], vec![customers.clone()], ContentOrigin { source_kind: Some(SOURCE_MANUAL.into()), ..Default::default() }, 2),
        ];
        for (name, kind, body, tags, groups, origin, uses) in templates {
            let key = format!("content:{}", name);
            let pending = match origin.source_kind.as_deref() {
                Some(SOURCE_CONVERSATION) => origin.source_conversation_id.is_none(),
                Some(SOURCE_PERSON) => origin.source_person_id.is_none(),
                _ => false,
            };
            if pending {
                continue;
            }
            if self.skip(&key).await? {
                continue;
            }
            let input = PostContentInput {
                kind: kind.into(),
                name: name.into(),
                body: Some(body.into()),
                tags: Some(tags.iter().map(|t| t.to_string()).collect()),
                group_ids: Some(groups.into_iter().flatten().collect()),
                source_kind: origin.source_kind.clone(),
                source_person_id: origin.source_person_id.clone(),
                source_conversation_id: origin.source_conversation_id.clone(),
                source_url: if origin.source_kind.as_deref() == Some(SOURCE_URL) { body.split_whitespace().find(|word| word.starts_with("https://")).map(String::from) } else { None },
                source_action_id: origin.source_action_id.clone(),
                ..PostContentInput::default()
            };
            let item = crate::services::content_service::store(self.pool, app_data, self.profile_id, input).await?;
            for _ in 0..uses {
                ContentItem::touch(self.pool, &item.id).await?;
            }
            self.note(&key, "content", &item.id).await?;
        }
        let memes = self.content_group("content_group:memes", "Memes", reactions.as_deref(), "lucide:laugh").await?;
        let shared = self.content_group("content_group:shared", "Shared files", work.as_deref(), "lucide:folder-open").await?;
        let more: [(&str, &str, &[&str], Vec<Option<String>>, [u16; 2], &[[u8; 3]], u32, bool); 4] = [
            ("This is fine", "image", &["meme", "fire"], vec![memes.clone(), reactions.clone()], [96, 72], &[[240, 120, 40]], 9, true),
            ("Works on my machine", "image", &["meme", "dev"], vec![memes.clone(), work.clone()], [96, 72], &[[60, 90, 160]], 6, false),
            ("Monday again", "gif", &["meme", "monday"], vec![memes.clone()], [80, 60], &[[90, 90, 90], [140, 140, 140], [200, 200, 200]], 3, false),
            ("Nailed it", "sticker", &["meme", "win"], vec![memes.clone(), celebrations.clone()], [64, 64], &[[31, 157, 85], [250, 250, 250]], 5, false),
        ];
        for (name, kind, tags, groups, size, frames, uses, favourite) in more {
            let key = format!("content:{}", name);
            if self.skip(&key).await? {
                continue;
            }
            let input = PostContentInput { kind: kind.to_string(), name: name.to_string(), tags: Some(tags.iter().map(|t| t.to_string()).collect()), keywords: Some(tags.iter().map(|t| t.to_string()).collect()), is_favourite: Some(favourite), group_ids: Some(groups.into_iter().flatten().collect()), ..PostContentInput::default() };
            let item = crate::services::content_service::store_bytes(self.pool, app_data, self.profile_id, input, ContentOrigin { source_kind: Some(SOURCE_UPLOAD.into()), ..Default::default() }, gif(size[0], size[1], frames)).await?;
            for _ in 0..uses {
                ContentItem::touch(self.pool, &item.id).await?;
            }
            self.note(&key, "content", &item.id).await?;
        }
        let files: Option<(String, String, String)> = sqlx::query_as("SELECT m.id, m.conversation_id, m.provider_id FROM message m WHERE m.external_id LIKE '%-files' AND m.provider_id IN (SELECT id FROM provider WHERE profile_id = ?1 AND provider_type = 'mock') LIMIT 1")
            .bind(self.profile_id)
            .fetch_optional(self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let from_message = |files: &Option<(String, String, String)>| match files {
            Some((message, conversation, provider)) => ContentOrigin { source_kind: Some(SOURCE_MESSAGE.into()), source_message_id: Some(message.clone()), source_conversation_id: Some(conversation.clone()), source_provider_id: Some(provider.clone()), ..Default::default() },
            None => ContentOrigin { source_kind: Some(SOURCE_UPLOAD.into()), ..Default::default() },
        };
        let documents: [(&str, &str, &[&str], Vec<u8>); 2] = [
            ("Referat workshop", "application/pdf", &["workshop", "referat"], pdf(&["Referat fra workshopen", "Deltakere: Eva, Sam, Mia, Milo", "Beslutning: variant to av forsiden", "Neste steg: palett innen fredag"])),
            ("Budsjett Q4", "text/csv", &["budsjett", "q4"], "Post;Beløp\nLisenser;12000\nKonsulent;48000\nReise;6500\n".as_bytes().to_vec()),
        ];
        for (name, mime, tags, bytes) in documents {
            let key = format!("content:{}", name);
            if self.skip(&key).await? {
                continue;
            }
            let input = PostContentInput { kind: KIND_FILE.into(), name: name.into(), mime_type: Some(mime.into()), tags: Some(tags.iter().map(|t| t.to_string()).collect()), group_ids: Some(shared.iter().cloned().collect()), ..PostContentInput::default() };
            let item = crate::services::content_service::store_bytes(self.pool, app_data, self.profile_id, input, from_message(&files), bytes).await?;
            self.note(&key, "content", &item.id).await?;
        }
        let invoice_mail: Option<String> = self.conversations.iter().find(|c| c.kind == "outlook" && c.conversation_type == "email").map(|c| c.id.clone());
        let templates: [(&str, &str, &str, &[&str], Vec<Option<String>>, ContentOrigin, u32); 4] = [
            ("Meeting confirmation", KIND_EMAIL, "Hi {{ .person.first_name | default \"there\" }},\n\nConfirming {{ .instruction | default \"our meeting\" }}. The invite is in your calendar, with the join link.\n\nBest regards,\n{{ .me.name }}", &["meeting", "invite"], vec![office.clone()], ContentOrigin { source_kind: Some(SOURCE_PERSON.into()), source_person_id: person.clone(), ..Default::default() }, 2),
            ("Received, thanks", KIND_REPLY, "Mottatt, takk {{ .person.first_name | default \"\" }}! Jeg ser på det i dag.", &["thanks", "mottatt"], vec![customers.clone()], ContentOrigin { source_kind: Some(SOURCE_CONVERSATION.into()), source_conversation_id: invoice_mail.clone(), ..Default::default() }, 5),
            ("Summarise for the board", KIND_PROMPT, "Sum up {{ .conversation.name }} for the board in three short Norwegian lines: what was decided, what it costs and what is still open.\n\n{{ range .messages }}{{ .sender }}: {{ .content }}\n{{ end }}", &["board", "styret"], vec![work.clone()], ContentOrigin { source_kind: Some(SOURCE_ACTION.into()), ..Default::default() }, 0),
            ("Release note", KIND_SNIPPET, "🚀 {{ .instruction | default \"En ny versjon\" }} er ute. Se {{ .conversation.name | default \"#dev\" }} for detaljer.", &["release"], vec![work.clone()], ContentOrigin { source_kind: Some(SOURCE_MANUAL.into()), ..Default::default() }, 1),
        ];
        for (name, kind, body, tags, groups, origin, uses) in templates {
            let key = format!("content:{}", name);
            if self.skip(&key).await? {
                continue;
            }
            let input = PostContentInput {
                kind: kind.into(),
                name: name.into(),
                body: Some(body.into()),
                tags: Some(tags.iter().map(|t| t.to_string()).collect()),
                group_ids: Some(groups.into_iter().flatten().collect()),
                source_kind: origin.source_kind.clone(),
                source_person_id: origin.source_person_id.clone(),
                source_conversation_id: origin.source_conversation_id.clone(),
                source_action_id: origin.source_action_id.clone(),
                ..PostContentInput::default()
            };
            let item = crate::services::content_service::store(self.pool, app_data, self.profile_id, input).await?;
            for _ in 0..uses {
                ContentItem::touch(self.pool, &item.id).await?;
            }
            self.note(&key, "content", &item.id).await?;
        }
        if !self.skip("content:Price list.pdf").await? {
            let input = PostContentInput { kind: KIND_FILE.into(), name: "Price list".into(), mime_type: Some("application/pdf".into()), tags: Some(vec!["prices".into()]), group_ids: Some(customers.into_iter().collect()), ..PostContentInput::default() };
            let pdf = b"%PDF-1.4\n1 0 obj << /Type /Catalog >> endobj\ntrailer << /Root 1 0 R >>\n%%EOF\n".to_vec();
            let item = crate::services::content_service::store_bytes(self.pool, app_data, self.profile_id, input, ContentOrigin { source_kind: Some(SOURCE_UPLOAD.into()), ..Default::default() }, pdf).await?;
            self.note("content:Price list.pdf", "content", &item.id).await?;
        }
        for tag in Tag::all(self.pool, self.profile_id).await?.into_iter().filter(|t| !known.contains(&t.id)) {
            self.note(&format!("tag:content:{}", tag.id), "tag", &tag.id).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn world() -> (sqlx::Pool<sqlx::Sqlite>, String, std::path::PathBuf) {
        use crate::providers::mock::{MockKind, MockProvider};
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        for kind in [MockKind::Teams, MockKind::Outlook, MockKind::Slack, MockKind::Gmail] {
            let id = crate::test_helpers::test_provider(&pool, &profile, "mock").await;
            sqlx::query("UPDATE provider SET external_id = ?1, namespace = ?2, config = ?2, name = ?3, access_token = 'mock', expires_at = ?4 WHERE id = ?5")
                .bind(format!("mock-{}-me", kind.key()))
                .bind(kind.key())
                .bind(kind.label())
                .bind((Utc::now() + Duration::days(365)).to_rfc3339())
                .bind(&id)
                .execute(&pool)
                .await
                .unwrap();
            let provider = crate::providers::models::Provider::load_from_db(&pool, &id).await.unwrap();
            provider.ingest(&pool, None, std::sync::Arc::new(MockProvider::new(kind, pool.clone()))).await.unwrap();
        }
        crate::services::people_service::rematch(&pool, &profile).await.unwrap();
        Tag::ensure_defaults(&pool, &profile).await.unwrap();
        Inbox::ensure_defaults(&pool, &profile).await.unwrap();
        crate::services::action_service::ensure_defaults(&pool, &profile).await.unwrap();
        let dir = std::env::temp_dir().join(format!("chatnip-seed-{}", uuid::Uuid::new_v4()));
        (pool, profile, dir)
    }

    async fn counts(pool: &Pool<Sqlite>) -> Vec<(String, i64)> {
        let mut out = Vec::new();
        for table in ["inbox", "inbox_member", "tag", "object_tag", "conversation_group", "conversation_group_item", "action_group", "action", "action_run", "message_schedule", "case_file", "case_expectation", "event", "content_item", "content_group", "content_group_item", "thread", "conversation_link", "merge_group", "merge_group_member", "merge_group_conversation", "chat", "chat_action", "chat_item", "section_summary", "pinned_message", "bookmarked_message", "link_preview", "app_setting", "object_override", "mock_seed"] {
            let n: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {}", table)).fetch_one(pool).await.unwrap_or(-1);
            out.push((table.to_string(), n));
        }
        out
    }

    #[tokio::test]
    async fn seeding_twice_changes_nothing() {
        let (pool, profile, dir) = world().await;
        let first = seed(&pool, &profile, Some(&dir)).await.unwrap();
        assert!(first.added > 40, "added {}", first.added);
        let after_first = counts(&pool).await;
        let second = seed(&pool, &profile, Some(&dir)).await.unwrap();
        assert_eq!(second.added, 0);
        assert_eq!(counts(&pool).await, after_first);
        let content: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content_item").fetch_one(&pool).await.unwrap();
        assert_eq!(content, 40);
        let templates: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content_item WHERE body IS NOT NULL").fetch_one(&pool).await.unwrap();
        assert_eq!(templates, 13);
        let shared: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content_group_item gi JOIN content_item ci ON ci.id = gi.content_id WHERE ci.name = 'Party parrot'").fetch_one(&pool).await.unwrap();
        assert_eq!(shared, 2);
        let inboxes: Vec<String> = Inbox::all(&pool, &profile).await.unwrap().into_iter().map(|i| i.name).collect();
        assert!(inboxes.contains(&"Private".to_string()) && inboxes.contains(&"Kunde AS".to_string()), "{:?}", inboxes);
        let actions = Action::all(&pool, &profile).await.unwrap();
        let vip = actions.iter().find(|a| a.name == "VIP triage").unwrap();
        assert!(vip.enabled, "VIP triage should pass its checks");
        assert!(!actions.iter().find(|a| a.name == "Archive newsletters").unwrap().enabled);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn reseeding_syncs_unedited_seeded_actions_and_leaves_edited_ones() {
        let (pool, profile, dir) = world().await;
        seed(&pool, &profile, Some(&dir)).await.unwrap();
        let item = |key: &'static str| {
            let pool = pool.clone();
            let profile = profile.clone();
            async move { sqlx::query_scalar::<_, String>("SELECT item_id FROM mock_seed WHERE profile_id = ?1 AND key = ?2").bind(&profile).bind(key).fetch_one(&pool).await.unwrap() }
        };
        let polite = item("action:polite").await;
        let digest = item("action:digest").await;
        let stale = json!({
            "nodes": [
                { "id": "write", "type": "builtin:ai_reply", "label": "Write the reply", "config": { "values": {} }, "body": "", "x": 640.0, "y": 0.0 },
                { "id": "queue", "type": "builtin:draft", "label": "Queue for approval", "config": { "values": {} }, "body": "", "x": 640.0, "y": 0.0 }
            ],
            "edges": [
                { "id": "write.text-queue.text", "source": "write", "source_port": "text", "target": "queue", "target_port": "text" },
                { "id": "queue.draft-queue.text", "source": "queue", "source_port": "draft", "target": "queue", "target_port": "text" }
            ]
        });
        sqlx::query("UPDATE action SET graph = ?1, description = 'Drafts a short, friendly reply for approval.', updated_at = created_at WHERE id = ?2").bind(stale.to_string()).bind(&polite).execute(&pool).await.unwrap();
        sqlx::query("UPDATE mock_seed SET hash = NULL WHERE key = 'action:polite'").execute(&pool).await.unwrap();
        let before = crate::services::action_service::check_input(&pool, &profile, crate::services::action_service::CheckInput { action_id: Some(polite.clone()), ..Default::default() }).await.unwrap();
        assert!(before.diagnostics.iter().any(|d| d.edge.as_deref() == Some("queue.draft-queue.text") && d.message == "A node cannot connect to itself"), "{:?}", before.diagnostics);
        crate::services::action_service::put_action(&pool, PutActionInput { action_id: digest.clone(), name: Some("My digest".into()), ..Default::default() }).await.unwrap();

        let report = seed(&pool, &profile, Some(&dir)).await.unwrap();
        assert_eq!(report.synced, 1, "{:?}", report);
        assert_eq!(report.edited, vec!["My digest".to_string()]);
        let synced = Action::get(&pool, &polite).await.unwrap();
        assert_eq!(synced.graph.nodes.iter().map(|n| n.label.as_str()).collect::<Vec<_>>(), vec!["Write the reply", "Keep as draft"]);
        assert_eq!(synced.graph.edges.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["write.text-draft.text"]);
        assert!(!synced.description.contains("approval"));
        assert_eq!(Action::get(&pool, &digest).await.unwrap().name, "My digest");

        let again = seed(&pool, &profile, Some(&dir)).await.unwrap();
        assert_eq!((again.synced, again.added), (0, 0));
        assert_eq!(again.edited, vec!["My digest".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn unseeding_removes_what_the_seed_added() {
        let (pool, profile, dir) = world().await;
        let before = counts(&pool).await;
        seed(&pool, &profile, Some(&dir)).await.unwrap();
        let removed = unseed(&pool, &profile, Some(&dir)).await.unwrap();
        assert!(removed > 20, "removed {}", removed);
        let after: HashMap<String, i64> = counts(&pool).await.into_iter().collect();
        for (table, n) in before {
            if ["object_tag", "inbox_member", "mock_seed"].contains(&table.as_str()) {
                continue;
            }
            assert_eq!(after[&table], n, "{} keeps seeded rows", table);
        }
        assert_eq!(after["mock_seed"], 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_receipts_chat_is_wired_to_an_attachable_copy_and_finds_only_receipts() {
        let (pool, profile, dir) = world().await;
        seed(&pool, &profile, Some(&dir)).await.unwrap();
        let chats = Chat::all(&pool, &profile).await.unwrap();
        let receipts = chats.iter().find(|c| c.name == "Receipts").unwrap();
        assert_eq!(receipts.mode, ChatMode::Read);
        assert_eq!(receipts.attachments.len(), 1);
        let copy = Action::get(&pool, &receipts.attachments[0].action_id).await.unwrap();
        assert!(copy.usage.chat && copy.enabled && !copy.builtin, "{:?}", copy);
        let conversation = crate::models::Conversation::load_with_providers(&pool, &receipts.conversation_id).await.unwrap().unwrap();
        assert_eq!(conversation.view, ChatView::Grid);
        assert_eq!(conversation.view_settings.grid.show, vec![crate::models::chat::GridShow::Records, crate::models::chat::GridShow::Files]);
        let mails: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mock_seed WHERE kind = 'receipt_mail'").fetch_one(&pool).await.unwrap();
        assert_eq!(mails, 7);
        let runner = crate::services::action_service::Runner {
            pool: pool.clone(),
            test_delay: None,
            app: None,
            app_data: dir.clone(),
            cache: dir.join("cache"),
            profile_id: profile.clone(),
            dry_run: false,
            ai: Some(std::sync::Arc::new(|key: &str, extra: &Value| match key {
                "receipt_confirm" => Ok(json!({ "verdicts": extra["candidates"].as_array().cloned().unwrap_or_default().into_iter().map(|id| json!({ "id": id, "receipt": true, "confidence": 0.9 })).collect::<Vec<_>>() })),
                other => Err(format!("no AI for {} in this test", other)),
            })),
        };
        crate::services::chat_service::backfill(&runner, &receipts.conversation_id, 500).await.unwrap();
        let page = crate::services::chat_service::page(&pool, &profile, &receipts.conversation_id, None, Some(100)).await.unwrap();
        let mut vendors: Vec<String> = page.items.iter().filter_map(|i| i.record.as_ref()).map(|r| r["vendor"].as_str().unwrap_or("").to_string()).collect();
        vendors.sort();
        assert!(vendors.contains(&"Elkjøp".to_string()) && vendors.contains(&"Apple".to_string()) && vendors.contains(&"Amazon".to_string()) && vendors.contains(&"Fjord Hosting".to_string()), "{:?}", vendors);
        for look_alike in ["Telenor", "Kiwi", "Acme Careers"] {
            assert!(!vendors.iter().any(|v| v == look_alike), "{} is not a receipt: {:?}", look_alike, vendors);
        }
        let shipped = page.items.iter().filter_map(|i| i.record.as_ref()).any(|r| r["subject"] == "Your order has shipped");
        assert!(!shipped);
        let elkjop = page.items.iter().filter_map(|i| i.record.as_ref()).find(|r| r["vendor"] == "Elkjøp").unwrap();
        assert_eq!(elkjop["total"], 3289.0);
        assert_eq!(elkjop["attachment"]["name"], "Kvittering-Elkjop-5521093.pdf");
        assert!(std::path::Path::new(elkjop["attachment"]["path"].as_str().unwrap()).is_file());
        let tiles = crate::services::chat_service::grid(&pool, &profile, &receipts.conversation_id).await.unwrap();
        assert!(tiles.iter().any(|t| t.kind == "file" && t.name == "Kvittering-Elkjop-5521093.pdf"));
        assert!(tiles.iter().any(|t| t.kind == "record" && t.tags.contains(&"Elkjøp".to_string()) && t.attachment_id.is_some()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn generated_gifs_are_valid_and_distinct() {
        let a = gif(4, 3, &[[1, 2, 3]]);
        let b = gif(4, 3, &[[1, 2, 4]]);
        assert!(a.starts_with(b"GIF89a"));
        assert_eq!(a.last(), Some(&0x3B));
        assert_eq!(u16::from_le_bytes([a[6], a[7]]), 4);
        assert_ne!(a, b);
        let animated = gif(2, 2, &[[0, 0, 0], [255, 255, 255]]);
        assert!(animated.windows(11).any(|w| w == b"NETSCAPE2.0"));
    }

    async fn seeded_actions(pool: &Pool<Sqlite>, profile: &str) -> Vec<Action> {
        let ids: Vec<String> = sqlx::query_scalar("SELECT item_id FROM mock_seed WHERE profile_id = ?1 AND kind = 'action'").bind(profile).fetch_all(pool).await.unwrap();
        let mut out = Vec::new();
        for id in ids {
            out.push(Action::get(pool, &id).await.unwrap());
        }
        out
    }

    fn needs_ai(action: &Action, known: &HashMap<String, Action>) -> bool {
        use crate::services::action_script::{REQUIRES_AI_CONNECTION, REQUIRES_AI_ENABLED, REQUIRES_AI_MODEL};
        action.graph.nodes.iter().any(|n| crate::services::action_service::node_requirements(n, known).iter().any(|k| [REQUIRES_AI_ENABLED, REQUIRES_AI_CONNECTION, REQUIRES_AI_MODEL].contains(k)))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn every_seeded_action_passes_its_check_with_at_most_requirements() {
        let (pool, profile, dir) = world().await;
        seed(&pool, &profile, Some(&dir)).await.unwrap();
        let actions = seeded_actions(&pool, &profile).await;
        assert!(actions.len() >= 25, "{}", actions.len());
        for action in &actions {
            let outcome = crate::services::action_service::check_input(&pool, &profile, crate::services::action_service::CheckInput { action_id: Some(action.id.clone()), ..Default::default() }).await.unwrap();
            let problems: Vec<String> = outcome.diagnostics.iter().filter(|d| d.severity != "requirement").map(|d| format!("{:?}: {}", d.node, d.message)).collect();
            assert!(problems.is_empty(), "{} has problems: {:?}", action.name, problems);
            let off = ["Archive newsletters"];
            assert_eq!(action.enabled, !off.contains(&action.name.as_str()), "{} is switched {}", action.name, if action.enabled { "on" } else { "off" });
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn every_built_in_trigger_step_and_landing_is_used_by_a_seeded_action() {
        use crate::models::action::{BUILTIN_KEY_PREFIX, NODE_CHAT, NODE_SCRIPT, NODE_TRIGGER};
        let (pool, profile, dir) = world().await;
        seed(&pool, &profile, Some(&dir)).await.unwrap();
        let actions = seeded_actions(&pool, &profile).await;
        let nodes: Vec<(&Action, &Node)> = actions.iter().flat_map(|a| a.graph.nodes.iter().map(move |n| (a, n))).collect();
        let wired = |action: &Action, node: &Node| action.graph.edges.iter().any(|e| e.target == node.id);
        let mut missing = Vec::new();
        for item in crate::services::action_service::picker_defaults() {
            let used = match item.key.strip_prefix(BUILTIN_KEY_PREFIX) {
                Some("pattern") => nodes.iter().any(|(_, n)| n.kind == NODE_TRIGGER && n.trigger().is_some_and(|t| t.on == crate::models::action::TriggerOn::Arrival) && n.body.contains("regex_match")),
                Some("ai") => nodes.iter().any(|(_, n)| n.kind == NODE_TRIGGER && (n.body.contains("ai_json(") || n.body.contains("ai("))),
                Some("chat_read") => nodes.iter().any(|(a, n)| n.chat().is_some_and(|c| c.mode == ChatMode::Read) && !wired(a, n)),
                Some(on) => nodes.iter().any(|(_, n)| n.kind == NODE_TRIGGER && n.config.get("on").and_then(|v| v.as_str()) == Some(on)),
                None => match item.key.strip_prefix(BUILTIN_PREFIX).unwrap_or_default() {
                    "condition" => nodes.iter().any(|(_, n)| n.kind == NODE_SCRIPT && n.declared("outputs").iter().any(|p| p.name == "true") && n.declared("outputs").iter().any(|p| p.name == "false")),
                    "script" => nodes.iter().any(|(_, n)| n.kind == NODE_SCRIPT),
                    "chat_read" => nodes.iter().any(|(a, n)| n.kind == NODE_CHAT && n.chat().is_some_and(|c| c.mode == ChatMode::Read) && wired(a, n)),
                    "chat_write" => nodes.iter().any(|(a, n)| n.kind == NODE_CHAT && n.chat().is_some_and(|c| c.mode == ChatMode::Write) && wired(a, n)),
                    _ => nodes.iter().any(|(_, n)| n.kind == item.key),
                },
            };
            if !used {
                missing.push(item.key.clone());
            }
        }
        let undemonstrated = ["builtin:hide", "builtin:mute", "builtin:mark_person", "builtin:regex_find", "builtin:pin", "builtin:notify", "trigger:reaction", "trigger:edited", "trigger:deleted", "trigger:mention", "trigger:pattern", "trigger:chat_read", "builtin:move", "builtin:reply", "builtin:send", "builtin:forward", "builtin:post_status", "builtin:group_add", "builtin:receipt_extract", "builtin:receipt_match", "builtin:receipt_confirm", "builtin:receipt_organise", "builtin:ai_json", "builtin:content", "builtin:regex_match", "builtin:regex_replace", "builtin:highlight", "builtin:style_conversation", "builtin:chat_read", "builtin:chat_write"];
        missing.retain(|key| !undemonstrated.contains(&key.as_str()));
        assert!(missing.is_empty(), "no seeded action uses {:?}", missing);
        let nested = actions.iter().filter(|a| a.graph.nodes.iter().any(|n| n.kind.starts_with(ACTION_PREFIX))).count();
        assert!(nested >= 3, "{} actions nest another", nested);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn every_seeded_ai_action_reports_what_to_set_up_when_ai_is_missing() {
        use crate::services::action_script::{REQUIRES_AI_CONNECTION, REQUIRES_AI_MODEL};
        let (pool, profile, dir) = world().await;
        seed(&pool, &profile, Some(&dir)).await.unwrap();
        let state = crate::services::action_service::requirement_state(&pool, &profile).await.unwrap();
        assert!(!state.connection && !state.model && state.accounts && state.actions);
        let all = Action::all(&pool, &profile).await.unwrap();
        let known: HashMap<String, Action> = all.iter().map(|a| (a.id.clone(), a.clone())).collect();
        let unmet = crate::services::action_service::unmet_across(&all, &state);
        let mut ai = 0;
        for action in seeded_actions(&pool, &profile).await {
            let mine: Vec<&String> = unmet.iter().filter(|u| u.action_id == action.id).map(|u| &u.setting).collect();
            if needs_ai(&action, &known) {
                ai += 1;
                assert!(mine.iter().any(|s| s.as_str() == REQUIRES_AI_CONNECTION) && mine.iter().any(|s| s.as_str() == REQUIRES_AI_MODEL), "{} hides what it needs: {:?}", action.name, mine);
                assert!(crate::services::action_script::requirement(REQUIRES_AI_CONNECTION).is_some());
            } else {
                assert!(mine.is_empty(), "{} claims it needs {:?}", action.name, mine);
            }
        }
        assert!(ai >= 10, "{} AI actions", ai);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_dry_run_of_every_seeded_action_without_ai_succeeds_on_the_mock_data() {
        use crate::services::action_service::{dry_start, execute, sample, Runner};
        let (pool, profile, dir) = world().await;
        seed(&pool, &profile, Some(&dir)).await.unwrap();
        let all = Action::all(&pool, &profile).await.unwrap();
        let known: HashMap<String, Action> = all.iter().map(|a| (a.id.clone(), a.clone())).collect();
        let runner = Runner { pool: pool.clone(), app: None, app_data: dir.clone(), cache: dir.join("cache"), profile_id: profile.clone(), dry_run: true, ai: None, test_delay: None };
        let payload = sample(&pool, &profile, None, None).await.unwrap();
        assert!(payload["message"]["id"].is_string());
        let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM object_tag").fetch_one(&pool).await.unwrap();
        let mut ran = 0;
        for action in seeded_actions(&pool, &profile).await.into_iter().filter(|a| !needs_ai(a, &known)) {
            let mut inputs = serde_json::Map::new();
            let stored = crate::models::AppSetting::get(&pool, &format!("{}{}", crate::models::app_setting::ACTION_MOCK_PREFIX, action.id)).await.unwrap();
            if let Some(Value::Object(mock)) = stored.and_then(|raw| serde_json::from_str(&raw).ok()) {
                for (key, value) in mock {
                    inputs.insert(key.trim_start_matches('/').to_string(), value);
                }
            }
            for port in &action.inputs {
                if !inputs.contains_key(&port.name) {
                    inputs.insert(port.name.clone(), payload.get(&port.name).cloned().unwrap_or(Value::Null));
                }
            }
            let run = execute(&runner, &action, dry_start(&action, inputs, payload.clone()), 0, vec![action.id.clone()]).await.unwrap();
            assert_eq!(run.status, "done", "{} failed its dry run: {:?}\n{:?}", action.name, run.error, run.log);
            ran += 1;
        }
        assert!(ran >= 15, "{} dry runs", ran);
        let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM object_tag").fetch_one(&pool).await.unwrap();
        assert_eq!(before, after, "a dry run changed tags");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_seed_covers_marks_previews_summaries_layout_aliases_and_chats() {
        use crate::models::sidebar::{SidebarEntry, SidebarLayout};
        let (pool, profile, dir) = world().await;
        let layout_before = SidebarLayout::get(&pool, &profile).await.unwrap();
        seed(&pool, &profile, Some(&dir)).await.unwrap();
        let count = |sql: &'static str| sqlx::query_scalar::<_, i64>(sql).fetch_one(&pool);
        assert!(count("SELECT COUNT(*) FROM pinned_message").await.unwrap() >= 2);
        assert!(count("SELECT COUNT(*) FROM bookmarked_message").await.unwrap() >= 2);
        assert!(count("SELECT COUNT(*) FROM link_preview").await.unwrap() >= 5);
        assert!(count("SELECT COUNT(*) FROM section_summary").await.unwrap() >= 4);
        assert!(count("SELECT COUNT(*) FROM section_summary WHERE kind = 'all'").await.unwrap() >= 1);
        assert!(count("SELECT COUNT(*) FROM object_tag WHERE object_type = 'message'").await.unwrap() >= 3);
        assert!(count("SELECT COUNT(DISTINCT object_type) FROM object_tag").await.unwrap() >= 7);
        assert!(count("SELECT COUNT(*) FROM event WHERE status = 'proposed'").await.unwrap() >= 3);
        assert!(count("SELECT COUNT(*) FROM chat_item WHERE kind = 'note'").await.unwrap() >= 3);
        assert!(count("SELECT COUNT(*) FROM object_override WHERE object_type = 'person' AND scope_type = ''").await.unwrap() >= 5, "a person carries local details");
        assert_eq!(count("SELECT COUNT(*) FROM object_override WHERE scope_type = 'conversation' AND field = 'display_name'").await.unwrap(), 1, "one chat nickname");
        assert_eq!(count("SELECT COUNT(*) FROM object_override WHERE object_type = 'conversation' AND field IN ('name', 'avatar_data')").await.unwrap(), 2, "a renamed chat with its own picture");
        assert!(count("SELECT COUNT(*) FROM person WHERE avatar_data LIKE 'data:image/svg+xml%'").await.unwrap() >= 4, "some people have photos");
        assert!(count("SELECT COUNT(*) FROM person WHERE avatar_data IS NULL AND is_human = 1").await.unwrap() >= 2, "some people keep generated avatars");
        assert!(count("SELECT COUNT(*) FROM conversation WHERE json_valid(metadata) AND json_extract(metadata, '$.image') IS NOT NULL").await.unwrap() >= 1, "a chat has a provider picture");
        let preview = crate::services::link_service::get_cached(&pool, "https://github.com/tauri-apps/tauri", Utc::now() + Duration::days(30)).await.unwrap().unwrap().unwrap();
        assert_eq!(preview.kind, "repo");
        let anna: Vec<String> = sqlx::query_scalar("SELECT DISTINCT person_id FROM person_identity WHERE LOWER(email) IN ('anna.berg@client.example', 'anna.berg.private@mail.example')").fetch_all(&pool).await.unwrap();
        assert_eq!(anna.len(), 1, "the private address is an alias of Anna");
        let grouped: Vec<Option<String>> = sqlx::query_scalar("SELECT c.person_id FROM merge_group_member m JOIN conversation c ON c.id = m.conversation_id").fetch_all(&pool).await.unwrap();
        assert!(grouped.len() == 2 && grouped[0].is_some() && grouped[0] == grouped[1], "the seeded group is one person's chats across providers");
        let layout = SidebarLayout::get(&pool, &profile).await.unwrap();
        assert!(layout.top.iter().filter(|e| matches!(e, SidebarEntry::Folder { .. })).count() >= 2);
        assert!(layout.top.iter().filter(|e| matches!(e, SidebarEntry::Divider { .. })).count() >= 2);
        let chats = Chat::all(&pool, &profile).await.unwrap();
        let week = chats.iter().find(|c| c.name == "Week at a glance").unwrap();
        assert!(week.sources.iter().all(|s| chats.iter().any(|c| Some(&c.conversation_id) == s.id.as_ref())) && week.sources.len() == 2, "Week at a glance reads other read chats");
        let sam = chats.iter().find(|c| c.name == "Sam everywhere").unwrap();
        assert_eq!(sam.mode, ChatMode::Write);
        assert_eq!(sam.links().len(), 2);
        let classify = Action::find(&pool, &profile, crate::services::action_service::CLASSIFY_SENDERS_KEY).await.unwrap();
        assert!(!classify.enabled);
        let runs: Vec<(String, bool)> = sqlx::query_as("SELECT status, dry_run FROM action_run").fetch_all(&pool).await.unwrap();
        for wanted in ["done", "failed", "waiting"] {
            assert!(runs.iter().any(|(s, _)| s == wanted), "no {} run", wanted);
        }
        assert!(runs.iter().any(|(_, dry)| *dry));
        unseed(&pool, &profile, Some(&dir)).await.unwrap();
        assert!(Action::find(&pool, &profile, crate::services::action_service::CLASSIFY_SENDERS_KEY).await.unwrap().enabled);
        assert_eq!(SidebarLayout::get(&pool, &profile).await.unwrap().keys(), layout_before.keys());
        let anna: Vec<String> = sqlx::query_scalar("SELECT DISTINCT person_id FROM person_identity WHERE LOWER(email) IN ('anna.berg@client.example', 'anna.berg.private@mail.example')").fetch_all(&pool).await.unwrap();
        assert_eq!(anna.len(), 2, "the alias is gone after unseeding");
        assert_eq!(count("SELECT COUNT(*) FROM object_override").await.unwrap(), 0);
        assert_eq!(count("SELECT COUNT(*) FROM provider_user WHERE avatar_data LIKE 'data:image/svg+xml%'").await.unwrap(), 0);
        assert_eq!(count("SELECT COUNT(*) FROM conversation WHERE json_valid(metadata) AND json_extract(metadata, '$.image') IS NOT NULL").await.unwrap(), 0);
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_setting WHERE key LIKE 'actions.mock.%'").fetch_one(&pool).await.unwrap();
        assert_eq!(left, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_seed_gives_faces_pictures_and_local_details_and_takes_them_back() {
        let (pool, profile, dir) = world().await;
        ledger(&pool).await.unwrap();
        let mut s = Seed { pool: &pool, profile_id: &profile, providers: providers(&pool, &profile).await.unwrap(), conversations: conversations(&pool, &profile).await.unwrap(), people: people(&pool, &profile).await.unwrap(), report: SeedReport::default() };
        s.faces().await.unwrap();
        let added = s.report.added;
        assert!(added >= 10, "added {}", added);
        s.faces().await.unwrap();
        assert_eq!(s.report.added, added, "a second run adds nothing");
        let count = |sql: &'static str| sqlx::query_scalar::<_, i64>(sql).fetch_one(&pool);
        assert!(count("SELECT COUNT(*) FROM person WHERE avatar_data LIKE 'data:image/svg+xml%'").await.unwrap() >= 4, "some people have photos");
        assert!(count("SELECT COUNT(*) FROM person WHERE avatar_data IS NULL AND is_human = 1").await.unwrap() >= 2, "some people keep generated avatars");
        assert_eq!(count("SELECT COUNT(*) FROM object_override WHERE object_type = 'person' AND scope_type = ''").await.unwrap(), 5);
        assert_eq!(count("SELECT COUNT(*) FROM object_override WHERE scope_type = 'conversation' AND field = 'display_name'").await.unwrap(), 1);
        assert_eq!(count("SELECT COUNT(*) FROM object_override WHERE object_type = 'conversation' AND field IN ('name', 'avatar_data')").await.unwrap(), 2);
        assert!(count("SELECT COUNT(*) FROM conversation WHERE json_valid(metadata) AND json_extract(metadata, '$.image') IS NOT NULL").await.unwrap() >= 1);
        let renamed: String = sqlx::query_scalar("SELECT object_id FROM object_override WHERE object_type = 'conversation' AND field = 'name'").fetch_one(&pool).await.unwrap();
        let mut shown = crate::models::Conversation::load_with_providers(&pool, &renamed).await.unwrap().unwrap();
        shown.load_display(&pool).await.unwrap();
        assert_eq!(shown.name, "Kaffeklubben");
        assert!(shown.image.as_deref().unwrap_or_default().starts_with("data:image/svg+xml;base64,"));
        assert!(shown.members.iter().any(|m| m.display_name.as_deref() == Some("Kapteinen")), "the nickname shows on the member");
        let lea = crate::models::Person::all(&pool, &profile).await.unwrap().into_iter().find(|p| p.provider_name == "Lea Lindqvist").unwrap();
        assert_eq!(lea.display_name, "Lea (PL)");
        assert_eq!(lea.overrides.get("color").map(String::as_str), Some("#e07b39"));
        unseed(&pool, &profile, Some(&dir)).await.unwrap();
        assert_eq!(count("SELECT COUNT(*) FROM object_override").await.unwrap(), 0);
        assert_eq!(count("SELECT COUNT(*) FROM person WHERE avatar_data LIKE 'data:image/svg+xml%'").await.unwrap(), 0);
        assert_eq!(count("SELECT COUNT(*) FROM provider_user WHERE avatar_data LIKE 'data:image/svg+xml%'").await.unwrap(), 0);
        assert_eq!(count("SELECT COUNT(*) FROM conversation WHERE json_valid(metadata) AND json_extract(metadata, '$.image') IS NOT NULL").await.unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_built_in_body_compiles_so_seeded_nodes_can_run() {
        for builtin in crate::services::action_service::builtins() {
            let problems = crate::services::action_script::check_script(builtin.body);
            assert!(problems.is_empty(), "{} does not compile: {:?}", builtin.key, problems);
        }
    }
}
