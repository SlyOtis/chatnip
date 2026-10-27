use super::html;
use super::links::{self, Link};
use super::rx;
use super::emoji;
use crate::providers::traits::{ResponseAttachment, ResponseMessage};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Mention {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Reaction {
    pub emoji: String,
    pub count: u32,
    pub users: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub kind: String,
    pub title: Option<String>,
    pub text: Vec<String>,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FileRef {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CodeBlock {
    pub language: Option<String>,
    pub code: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatContent {
    pub text: String,
    pub html: Option<String>,
    pub mentions: Vec<Mention>,
    pub channels: Vec<Mention>,
    pub reactions: Vec<Reaction>,
    pub cards: Vec<Card>,
    pub files: Vec<FileRef>,
    pub images: Vec<String>,
    pub links: Vec<Link>,
    pub code: Vec<CodeBlock>,
    pub lists: Vec<Vec<String>>,
    pub quotes: Vec<String>,
    pub commands: Vec<String>,
    pub reply_to: Option<String>,
    pub thread_id: Option<String>,
    pub is_bot: bool,
    pub edited_at: Option<String>,
    pub deleted_at: Option<String>,
}

impl ChatContent {
    pub fn apply(&self, message: &mut ResponseMessage) {
        if let Ok(value) = serde_json::to_value(StructureView::from(self)) {
            message.metadata.insert("structure".to_string(), value);
        }
        if !self.reactions.is_empty() {
            let flat: Vec<Value> = self.reactions.iter().flat_map(|r| { let users: Vec<String> = if r.users.is_empty() { vec![String::new(); r.count as usize] } else { r.users.clone() }; users.into_iter().map(move |u| match r.original.as_deref().filter(|o| *o != r.emoji) { Some(original) => serde_json::json!({ "emoji": r.emoji, "user": u, "original": original }), None => serde_json::json!({ "emoji": r.emoji, "user": u }) }) }).collect();
            message.metadata.insert("reactions".to_string(), Value::Array(flat));
        }
        if let Some(at) = &self.edited_at {
            message.metadata.insert("edited_at".to_string(), Value::String(at.clone()));
        }
        if let Some(at) = &self.deleted_at {
            message.metadata.insert("deleted_at".to_string(), Value::String(at.clone()));
        }
        if message.reply_to_id.is_none() {
            message.reply_to_id = self.reply_to.clone();
        }
        if message.thread_id.is_none() {
            message.thread_id = self.thread_id.clone();
        }
        if message.attachments.is_empty() {
            message.attachments = self.files.iter().map(|f| ResponseAttachment { external_id: f.id.clone(), name: f.name.clone(), content_type: f.mime.clone(), size: f.size, url: f.url.clone(), content_id: None }).collect();
        }
    }
}

#[derive(Serialize)]
struct StructureView<'a> {
    mentions: &'a [Mention],
    channels: &'a [Mention],
    cards: &'a [Card],
    images: &'a [String],
    links: &'a [Link],
    previews: Vec<String>,
    code: &'a [CodeBlock],
    lists: &'a [Vec<String>],
    quotes: &'a [String],
    commands: &'a [String],
    is_bot: bool,
    emoji_only: bool,
}

impl<'a> From<&'a ChatContent> for StructureView<'a> {
    fn from(c: &'a ChatContent) -> Self {
        Self {
            mentions: &c.mentions,
            channels: &c.channels,
            cards: &c.cards,
            images: &c.images,
            links: &c.links,
            previews: links::preview_candidates(&c.links, 3),
            code: &c.code,
            lists: &c.lists,
            quotes: &c.quotes,
            commands: &c.commands,
            is_bot: c.is_bot,
            emoji_only: super::text::is_emoji_only(&c.text),
        }
    }
}

fn group_reactions(items: impl Iterator<Item = (String, String)>) -> Vec<Reaction> {
    let mut out: Vec<Reaction> = Vec::new();
    for (original, user) in items {
        let emoji = emoji::teams_canonical(&original);
        match out.iter_mut().find(|r| r.emoji == emoji) {
            Some(r) => {
                r.count += 1;
                r.users.push(user);
            }
            None => out.push(Reaction { original: (original != emoji).then_some(original), emoji, count: 1, users: vec![user] }),
        }
    }
    out
}

fn adaptive_card(content: &Value) -> Card {
    let mut texts = Vec::new();
    let mut title = None;
    let mut stack: Vec<&Value> = content["body"].as_array().map(|a| a.iter().rev().collect()).unwrap_or_default();
    while let Some(item) = stack.pop() {
        match item["type"].as_str().unwrap_or("") {
            "TextBlock" => {
                let text = item["text"].as_str().unwrap_or("").to_string();
                let heading = item["weight"].as_str() == Some("Bolder") || matches!(item["size"].as_str(), Some("Large" | "ExtraLarge" | "Medium"));
                if title.is_none() && heading {
                    title = Some(text);
                } else if !text.is_empty() {
                    texts.push(text);
                }
            }
            "FactSet" => {
                for fact in item["facts"].as_array().cloned().unwrap_or_default() {
                    texts.push(format!("{}: {}", fact["title"].as_str().unwrap_or(""), fact["value"].as_str().unwrap_or("")));
                }
            }
            _ => {
                for key in ["items", "columns"] {
                    if let Some(children) = item[key].as_array() {
                        stack.extend(children.iter().rev());
                    }
                }
            }
        }
    }
    Card {
        kind: "adaptive".to_string(),
        title,
        text: texts,
        actions: content["actions"].as_array().map(|a| a.iter().filter_map(|x| x["title"].as_str().map(String::from)).collect()).unwrap_or_default(),
    }
}

pub fn teams(msg: &Value) -> ChatContent {
    let body = msg["body"]["content"].as_str().unwrap_or("");
    let is_html = msg["body"]["contentType"].as_str().map(|c| c.eq_ignore_ascii_case("html")).unwrap_or(false);
    let mut content = ChatContent {
        is_bot: msg["from"]["application"].is_object(),
        edited_at: msg["lastEditedDateTime"].as_str().map(String::from),
        deleted_at: msg["deletedDateTime"].as_str().map(String::from),
        ..Default::default()
    };
    if is_html {
        content.html = Some(body.to_string());
        content.text = html::visible_text(body);
        content.images = html::images(body).into_iter().map(|i| i.src).collect();
        content.links = links::extract("", Some(body));
        content.code = html::code_blocks(body).into_iter().map(|(language, code)| CodeBlock { language, code }).collect();
    } else {
        content.text = body.to_string();
        content.links = links::extract(body, None);
    }
    content.mentions = msg["mentions"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|m| Mention {
                    id: m["mentioned"]["user"]["id"].as_str().or_else(|| m["mentioned"]["conversation"]["id"].as_str()).unwrap_or("").to_string(),
                    name: m["mentioned"]["user"]["displayName"].as_str().or_else(|| m["mentionText"].as_str()).unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    content.reactions = group_reactions(msg["reactions"].as_array().cloned().unwrap_or_default().into_iter().map(|r| (r["reactionType"].as_str().unwrap_or("").to_string(), r["user"]["user"]["id"].as_str().unwrap_or("").to_string())));
    for attachment in msg["attachments"].as_array().cloned().unwrap_or_default() {
        match attachment["contentType"].as_str().unwrap_or("") {
            "application/vnd.microsoft.card.adaptive" => {
                let parsed = attachment["content"].as_str().and_then(|s| serde_json::from_str::<Value>(s).ok()).unwrap_or_else(|| attachment["content"].clone());
                content.cards.push(adaptive_card(&parsed));
            }
            "messageReference" => {
                content.reply_to = attachment["id"].as_str().map(String::from);
                let reference = attachment["content"].as_str().and_then(|s| serde_json::from_str::<Value>(s).ok()).unwrap_or_default();
                let preview = reference["messagePreview"].as_str().unwrap_or("");
                let sender = reference["messageSender"]["user"]["displayName"].as_str().unwrap_or("");
                if let (Some(html), Some(id)) = (content.html.as_mut(), attachment["id"].as_str()) {
                    let quote = format!("<blockquote class=\"reply-quote\"><strong>{}</strong><p>{}</p></blockquote>", html::escape(sender), html::escape(preview));
                    *html = html.replace(&format!("<attachment id=\"{}\"></attachment>", id), &quote);
                }
                if !preview.is_empty() {
                    content.quotes.push(preview.to_string());
                }
            }
            "reference" => content.files.push(FileRef {
                id: attachment["id"].as_str().unwrap_or("").to_string(),
                name: attachment["name"].as_str().unwrap_or("file").to_string(),
                mime: mime_from_name(attachment["name"].as_str().unwrap_or("")),
                size: 0,
                url: attachment["contentUrl"].as_str().map(String::from),
            }),
            _ => {}
        }
    }
    if content.reply_to.is_none() {
        content.reply_to = msg["replyToId"].as_str().map(String::from);
    }
    content
}

fn mime_from_name(name: &str) -> String {
    match name.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "zip" => "application/zip",
        "txt" | "log" => "text/plain",
        "csv" => "text/csv",
        _ => "application/octet-stream",
    }
    .to_string()
}

pub fn slack_text(text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let rendered = rx(&RE, r"<([@#!])([^|>]+)(?:\|([^>]+))?>|<((?:https?|mailto):[^|>]+)(?:\|([^>]+))?>").replace_all(text, |c: &regex::Captures| {
        if let Some(url) = c.get(4) {
            return match c.get(5) {
                Some(label) => format!("{} ({})", label.as_str(), url.as_str()),
                None => url.as_str().to_string(),
            };
        }
        let sigil = &c[1];
        let label = c.get(3).map(|m| m.as_str()).unwrap_or(&c[2]);
        match sigil {
            "@" => format!("@{}", label),
            "#" => format!("#{}", label),
            _ => format!("@{}", label.trim_start_matches("subteam^")),
        }
    });
    rendered.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

pub fn slack_time(ts: &str) -> String {
    let mut parts = ts.split('.');
    let secs = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let micros = parts.next().and_then(|s| format!("{:0<6}", s).get(..6).and_then(|s| s.parse::<u32>().ok())).unwrap_or(0);
    chrono::DateTime::from_timestamp(secs, micros * 1000).unwrap_or_else(chrono::Utc::now).to_rfc3339()
}

fn slack_emoji(el: &Value) -> String {
    let name = el["name"].as_str().unwrap_or("");
    let tone = el["skin_tone"].as_u64().filter(|t| (2..=6).contains(t));
    match (name.is_empty(), tone) {
        (true, _) => el["unicode"].as_str().and_then(|u| u.split('-').map(|h| u32::from_str_radix(h, 16).ok().and_then(char::from_u32)).collect::<Option<String>>()).map(|u| emoji::canonical_text(&u)).unwrap_or_default(),
        (false, Some(tone)) => format!(":{}::skin-tone-{}:", name, tone),
        (false, None) => format!(":{}:", name),
    }
}

fn slack_inline(elements: &Value, mentions: &mut Vec<Mention>, channels: &mut Vec<Mention>, links: &mut Vec<Link>) -> String {
    let mut out = String::new();
    for el in elements.as_array().cloned().unwrap_or_default() {
        match el["type"].as_str().unwrap_or("") {
            "text" => {
                let text = el["text"].as_str().unwrap_or("");
                let style = &el["style"];
                let wrap = if style["code"].as_bool() == Some(true) { "`" } else if style["bold"].as_bool() == Some(true) { "**" } else if style["italic"].as_bool() == Some(true) { "_" } else if style["strike"].as_bool() == Some(true) { "~" } else { "" };
                out.push_str(&format!("{wrap}{text}{wrap}"));
            }
            "user" => {
                let id = el["user_id"].as_str().unwrap_or("").to_string();
                out.push_str(&format!("@{}", id));
                if !mentions.iter().any(|m| m.id == id) {
                    mentions.push(Mention { name: id.clone(), id });
                }
            }
            "usergroup" => {
                let id = el["usergroup_id"].as_str().unwrap_or("").to_string();
                out.push_str(&format!("@{}", id));
                let key = format!("group:{}", id);
                if !mentions.iter().any(|m| m.id == key) {
                    mentions.push(Mention { name: id, id: key });
                }
            }
            "channel" => {
                let id = el["channel_id"].as_str().unwrap_or("").to_string();
                out.push_str(&format!("#{}", id));
                if !channels.iter().any(|c| c.id == id) {
                    channels.push(Mention { name: id.clone(), id });
                }
            }
            "link" => {
                let url = el["url"].as_str().unwrap_or("").to_string();
                let label = el["text"].as_str().map(String::from);
                out.push_str(label.as_deref().unwrap_or(&url));
                if !links.iter().any(|l| l.url == url) {
                    links.extend(links::extract(&url, None).into_iter().map(|mut l| {
                        l.label = label.clone();
                        l
                    }));
                }
            }
            "emoji" => out.push_str(&slack_emoji(&el)),
            "broadcast" => {
                let range = el["range"].as_str().unwrap_or("here").to_string();
                out.push_str(&format!("@{}", range));
                if !mentions.iter().any(|m| m.id == range) {
                    mentions.push(Mention { name: format!("@{}", range), id: range });
                }
            }
            _ => {}
        }
    }
    out
}

pub fn slack(msg: &Value) -> ChatContent {
    static MENTION: OnceLock<Regex> = OnceLock::new();
    let raw = msg["text"].as_str().unwrap_or("");
    let ts = msg["ts"].as_str().unwrap_or("");
    let mut content = ChatContent {
        text: slack_text(raw),
        is_bot: msg["subtype"].as_str() == Some("bot_message") || msg["bot_id"].is_string(),
        thread_id: msg["thread_ts"].as_str().filter(|t| *t != ts).map(String::from),
        ..Default::default()
    };
    content.reply_to = content.thread_id.clone();
    for c in rx(&MENTION, r"<@([UW][A-Z0-9]+)(?:\|([^>]+))?>").captures_iter(raw) {
        content.mentions.push(Mention { id: c[1].to_string(), name: c.get(2).map(|m| m.as_str().to_string()).unwrap_or_else(|| c[1].to_string()) });
    }
    static SPECIAL: OnceLock<Regex> = OnceLock::new();
    static CHANNEL: OnceLock<Regex> = OnceLock::new();
    for c in rx(&SPECIAL, r"<!(here|channel|everyone|subteam\^([A-Z0-9]+))(?:\|([^>]+))?>").captures_iter(raw) {
        let mention = match c.get(2) {
            Some(group) => Mention { id: format!("group:{}", group.as_str()), name: c.get(3).map(|m| m.as_str().to_string()).unwrap_or_else(|| group.as_str().to_string()) },
            None => Mention { id: c[1].to_string(), name: format!("@{}", &c[1]) },
        };
        if !content.mentions.iter().any(|m| m.id == mention.id) {
            content.mentions.push(mention);
        }
    }
    let mut channels: Vec<Mention> = Vec::new();
    for c in rx(&CHANNEL, r"<#([CG][A-Z0-9]+)(?:\|([^>]*))?>").captures_iter(raw) {
        if !channels.iter().any(|m| m.id == c[1]) {
            channels.push(Mention { id: c[1].to_string(), name: c.get(2).map(|m| m.as_str()).filter(|n| !n.is_empty()).unwrap_or(&c[1]).to_string() });
        }
    }
    content.edited_at = msg["edited"]["ts"].as_str().map(slack_time);
    content.links = links::extract(raw, None);
    let mut mentions = std::mem::take(&mut content.mentions);
    let mut found_links = std::mem::take(&mut content.links);
    for block in msg["blocks"].as_array().cloned().unwrap_or_default() {
        match block["type"].as_str().unwrap_or("") {
            "rich_text" => {
                for el in block["elements"].as_array().cloned().unwrap_or_default() {
                    match el["type"].as_str().unwrap_or("") {
                        "rich_text_list" => content.lists.push(el["elements"].as_array().cloned().unwrap_or_default().iter().map(|item| slack_inline(&item["elements"], &mut mentions, &mut channels, &mut found_links)).collect()),
                        "rich_text_preformatted" => content.code.push(CodeBlock { language: el["language"].as_str().map(String::from), code: slack_inline(&el["elements"], &mut mentions, &mut channels, &mut found_links) }),
                        "rich_text_quote" => content.quotes.push(slack_inline(&el["elements"], &mut mentions, &mut channels, &mut found_links)),
                        _ => {
                            slack_inline(&el["elements"], &mut mentions, &mut channels, &mut found_links);
                        }
                    }
                }
            }
            kind @ ("section" | "header" | "context") => {
                let mut card = Card { kind: kind.to_string(), ..Default::default() };
                let text = block["text"]["text"].as_str().map(slack_text);
                if kind == "header" {
                    card.title = text;
                } else if let Some(t) = text {
                    card.text.push(t);
                }
                for field in block["fields"].as_array().cloned().unwrap_or_default().iter().chain(block["elements"].as_array().cloned().unwrap_or_default().iter()) {
                    if let Some(t) = field["text"].as_str() {
                        card.text.push(slack_text(t));
                    }
                }
                content.cards.push(card);
            }
            "actions" => content.cards.push(Card {
                kind: "actions".to_string(),
                actions: block["elements"].as_array().cloned().unwrap_or_default().iter().filter_map(|e| e["text"]["text"].as_str().map(String::from)).collect(),
                ..Default::default()
            }),
            "image" => content.images.push(block["image_url"].as_str().unwrap_or("").to_string()),
            _ => {}
        }
    }
    for attachment in msg["attachments"].as_array().cloned().unwrap_or_default() {
        let mut text: Vec<String> = attachment["text"].as_str().map(slack_text).into_iter().collect();
        for field in attachment["fields"].as_array().cloned().unwrap_or_default() {
            text.push(format!("{}: {}", field["title"].as_str().unwrap_or(""), field["value"].as_str().unwrap_or("")));
        }
        content.cards.push(Card {
            kind: "attachment".to_string(),
            title: attachment["title"].as_str().or_else(|| attachment["fallback"].as_str()).map(String::from),
            text,
            actions: attachment["actions"].as_array().map(|a| a.iter().filter_map(|x| x["text"].as_str().map(String::from)).collect()).unwrap_or_default(),
        });
        if let Some(url) = attachment["title_link"].as_str() {
            found_links.extend(links::extract(url, None));
        }
    }
    content.mentions = mentions;
    content.channels = channels;
    content.links = found_links;
    content.reactions = msg["reactions"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|r| {
            let name = r["name"].as_str().unwrap_or("");
            let emoji = emoji::canonical_reaction(name);
            Reaction {
                original: (format!(":{}:", name) != emoji).then(|| name.to_string()),
                emoji,
                count: r["count"].as_u64().unwrap_or(0) as u32,
                users: r["users"].as_array().map(|u| u.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
            }
        })
        .collect();
    content.files = msg["files"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|f| FileRef {
            id: f["id"].as_str().unwrap_or("").to_string(),
            name: f["name"].as_str().unwrap_or("file").to_string(),
            mime: f["mimetype"].as_str().unwrap_or("application/octet-stream").to_string(),
            size: f["size"].as_u64().unwrap_or(0),
            url: f["url_private"].as_str().map(String::from),
        })
        .collect();
    content
}

pub fn google_chat(msg: &Value) -> ChatContent {
    let text = msg["text"].as_str().unwrap_or("").to_string();
    let chars: Vec<char> = text.chars().collect();
    let mut content = ChatContent {
        links: links::extract(&text, None),
        is_bot: msg["sender"]["type"].as_str() == Some("BOT"),
        thread_id: msg["thread"]["name"].as_str().map(String::from),
        reply_to: msg["quotedMessageMetadata"]["name"].as_str().map(String::from),
        ..Default::default()
    };
    for annotation in msg["annotations"].as_array().cloned().unwrap_or_default() {
        let start = annotation["startIndex"].as_u64().unwrap_or(0) as usize;
        let length = annotation["length"].as_u64().unwrap_or(0) as usize;
        let span: String = chars.iter().skip(start).take(length).collect();
        match annotation["type"].as_str().unwrap_or("") {
            "USER_MENTION" => content.mentions.push(Mention {
                id: annotation["userMention"]["user"]["name"].as_str().unwrap_or("").to_string(),
                name: annotation["userMention"]["user"]["displayName"].as_str().map(String::from).unwrap_or_else(|| span.trim_start_matches('@').to_string()),
            }),
            "SLASH_COMMAND" => content.commands.push(annotation["slashCommand"]["commandName"].as_str().map(String::from).unwrap_or(span)),
            _ => {}
        }
    }
    for wrapper in msg["cardsV2"].as_array().cloned().unwrap_or_default() {
        let card = &wrapper["card"];
        let mut out = Card { kind: "card_v2".to_string(), title: card["header"]["title"].as_str().map(String::from), ..Default::default() };
        for section in card["sections"].as_array().cloned().unwrap_or_default() {
            for widget in section["widgets"].as_array().cloned().unwrap_or_default() {
                if let Some(t) = widget["textParagraph"]["text"].as_str().or_else(|| widget["decoratedText"]["text"].as_str()) {
                    out.text.push(html::flat_text(t));
                }
                for button in widget["buttonList"]["buttons"].as_array().cloned().unwrap_or_default() {
                    if let Some(t) = button["text"].as_str() {
                        out.actions.push(t.to_string());
                    }
                }
            }
        }
        content.cards.push(out);
    }
    content.files = msg["attachment"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|a| FileRef {
            id: a["name"].as_str().unwrap_or("").to_string(),
            name: a["contentName"].as_str().unwrap_or("file").to_string(),
            mime: a["contentType"].as_str().unwrap_or("application/octet-stream").to_string(),
            size: 0,
            url: a["downloadUri"].as_str().map(String::from),
        })
        .collect();
    content.reactions = msg["emojiReactionSummaries"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|r| { let original = r["emoji"]["unicode"].as_str().unwrap_or("").to_string(); Reaction { emoji: emoji::canonical_reaction(&original), original: Some(original), count: r["reactionCount"].as_u64().unwrap_or(0) as u32, users: Vec::new() } })
        .collect();
    content.text = text;
    content
}

pub fn markdown(source: &str) -> ChatContent {
    static FENCE: OnceLock<Regex> = OnceLock::new();
    static MENTION: OnceLock<Regex> = OnceLock::new();
    static ITEM: OnceLock<Regex> = OnceLock::new();
    let text = source.to_string();
    let mut content = ChatContent { links: links::extract(&text, None), ..Default::default() };
    for c in rx(&FENCE, r"(?s)```([\w+#-]*)[ \t]*\n(.*?)\n?```").captures_iter(&text) {
        content.code.push(CodeBlock { language: Some(c[1].to_string()).filter(|l| !l.is_empty()), code: c[2].to_string() });
    }
    let prose = rx(&FENCE, r"(?s)```([\w+#-]*)[ \t]*\n(.*?)\n?```").replace_all(&text, "").to_string();
    let item = rx(&ITEM, r"^\s*([-*+]|\d+[.)])\s+(.+)$");
    let mut list: Vec<String> = Vec::new();
    let mut ordered = false;
    for line in prose.lines().chain(std::iter::once("")) {
        if let Some(c) = item.captures(line) {
            let numbered = c[1].starts_with(|ch: char| ch.is_ascii_digit());
            if numbered != ordered && !list.is_empty() {
                content.lists.push(std::mem::take(&mut list));
            }
            ordered = numbered;
            list.push(c[2].trim().to_string());
            continue;
        }
        if !list.is_empty() {
            content.lists.push(std::mem::take(&mut list));
        }
        if let Some(quote) = line.strip_prefix('>') {
            content.quotes.push(quote.trim().to_string());
        }
    }
    for c in rx(&MENTION, r"(?:^|[\s(])@([\p{L}][\p{L}\p{N}._-]*)").captures_iter(&prose) {
        let name = c[1].trim_end_matches(['.', '_', '-']).to_string();
        if !content.mentions.iter().any(|m| m.name == name) {
            content.mentions.push(Mention { id: name.to_lowercase(), name });
        }
    }
    content.text = text;
    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::fixture;

    fn json(name: &str) -> Value {
        serde_json::from_str(&fixture(name)).unwrap()
    }

    #[test]
    fn teams_message_has_mentions_reactions_card_image_reply_and_code() {
        let msg = json("teams-message.json");
        let c = teams(&msg);
        assert_eq!(c.text.lines().next(), Some("@Nora Gale @Sam Hale can you check the deploy before lunch?"));
        assert_eq!(c.mentions, vec![Mention { id: "u-nora".to_string(), name: "Nora Gale".to_string() }, Mention { id: "u-sam".to_string(), name: "Sam Hale".to_string() }]);
        assert_eq!(c.reactions, vec![Reaction { emoji: ":+1:".to_string(), count: 2, users: vec!["u-nora".to_string(), "u-sam".to_string()], original: Some("like".to_string()) }, Reaction { emoji: ":heart:".to_string(), count: 1, users: vec!["u-eva".to_string()], original: Some("heart".to_string()) }]);
        assert_eq!(c.cards.len(), 1);
        assert_eq!(c.cards[0].title.as_deref(), Some("Deploy approved"));
        assert_eq!(c.cards[0].text, vec!["Staging is green.", "Environment: staging", "Version: 0.4.1"]);
        assert_eq!(c.cards[0].actions, vec!["Open pipeline"]);
        assert_eq!(c.images, vec!["https://graph.microsoft.com/v1.0/chats/19:abc/messages/1700/hostedContents/aWQ9/$value"]);
        assert_eq!(c.reply_to.as_deref(), Some("1699"));
        assert_eq!(c.code, vec![CodeBlock { language: Some("Bash".to_string()), code: "kubectl rollout status deploy/api".to_string() }]);
        assert_eq!(c.files.len(), 1);
        assert_eq!(c.files[0].mime, "application/pdf");
        assert_eq!(links::preview_candidates(&c.links, 3), vec!["https://dev.azure.com/northwind/pipelines/12"]);
    }

    #[test]
    fn teams_plain_text_and_bot_sender() {
        let msg = serde_json::json!({ "body": { "contentType": "text", "content": "Build #12 passed https://ci.example/12" }, "from": { "application": { "id": "app", "displayName": "CI" } } });
        let c = teams(&msg);
        assert!(c.is_bot);
        assert!(c.html.is_none());
        assert_eq!(c.links[0].url, "https://ci.example/12");
    }

    #[test]
    fn slack_message_with_blocks_lists_code_quote_and_reactions() {
        let messages = json("slack-messages.json");
        let c = slack(&messages["messages"][0]);
        assert_eq!(c.text, "@Sam can you look at Fix sync loop (https://github.com/SlyOtis/chatnip/pull/42) in #dev :rocket:");
        assert_eq!(c.mentions.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["U0SAM", "U0MIKE"]);
        assert_eq!(c.lists, vec![vec!["**double refresh** on first sync".to_string(), "`since` is ignored".to_string()], vec!["write the test".to_string(), "merge @U0MIKE".to_string()]]);
        assert_eq!(c.code, vec![CodeBlock { language: None, code: "cargo test --bin chatnip".to_string() }]);
        assert_eq!(c.quotes, vec!["works on my machine :shrug:".to_string()]);
        assert_eq!(c.reactions, vec![Reaction { emoji: ":eyes:".to_string(), count: 2, users: vec!["U0MIKE".to_string(), "U0NORA".to_string()], original: None }, Reaction { emoji: ":party-parrot:".to_string(), count: 1, users: vec!["U0NORA".to_string()], original: None }, Reaction { emoji: ":+1::skin-tone-3:".to_string(), count: 1, users: vec!["U0SAM".to_string()], original: None }]);
        assert!(c.links.iter().any(|l| l.url == "https://github.com/SlyOtis/chatnip/pull/42" && l.label.as_deref() == Some("Fix sync loop")));
        assert!(c.thread_id.is_none());
    }

    #[test]
    fn slack_thread_reply_file_share_and_bot_attachment() {
        let messages = json("slack-messages.json");
        let reply = slack(&messages["messages"][1]);
        assert_eq!(reply.thread_id.as_deref(), Some("1728470000.000100"));
        assert_eq!(reply.reply_to, reply.thread_id);
        let file = slack(&messages["messages"][2]);
        assert_eq!(file.files, vec![FileRef { id: "F0LOG".to_string(), name: "sync.log".to_string(), mime: "text/plain".to_string(), size: 48211, url: Some("https://files.slack.com/files-pri/T0/F0LOG/sync.log".to_string()) }]);
        let bot = slack(&messages["messages"][3]);
        assert!(bot.is_bot);
        assert_eq!(bot.cards.iter().map(|c| c.kind.as_str()).collect::<Vec<_>>(), vec!["header", "section", "context", "actions", "attachment"]);
        assert_eq!(bot.cards[0].title.as_deref(), Some("Deploy finished"));
        assert_eq!(bot.cards[1].text, vec!["*api* is live on staging", "*Version:* 0.4.1", "*Took:* 3m 12s"]);
        assert_eq!(bot.cards[3].actions, vec!["Open", "Roll back"]);
        let legacy = &bot.cards[4];
        assert_eq!(legacy.title.as_deref(), Some("Coverage report"));
        assert_eq!(legacy.text, vec!["Coverage went up", "Lines: 82%"]);
    }

    #[test]
    fn slack_text_renders_entities_links_and_shortcodes() {
        assert_eq!(slack_text("<!here> <@U1|anna> see <#C1|general> and <https://a.no|A> or <mailto:x@y.no> &lt;3 :tada:"), "@here @anna see #general and A (https://a.no) or mailto:x@y.no <3 :tada:");
    }

    #[test]
    fn google_chat_message_with_mention_command_card_and_thread() {
        let msg = json("gchat-message.json");
        let c = google_chat(&msg);
        assert_eq!(c.mentions, vec![Mention { id: "users/110".to_string(), name: "Mia Sund".to_string() }]);
        assert_eq!(c.commands, vec!["/standup"]);
        assert_eq!(c.cards.len(), 1);
        assert_eq!(c.cards[0].title.as_deref(), Some("Standup 09:15"));
        assert_eq!(c.cards[0].text, vec!["3 updates, 1 blocker", "Blocker: staging DNS"]);
        assert_eq!(c.cards[0].actions, vec!["Open notes"]);
        assert_eq!(c.thread_id.as_deref(), Some("spaces/AAA/threads/T1"));
        assert_eq!(c.files[0].name, "notes.pdf");
        assert_eq!(c.reactions[0].emoji, ":pray:");
        assert_eq!(c.reactions[0].original.as_deref(), Some("🙏"));
        assert!(!c.is_bot);
    }

    #[test]
    fn apply_fills_metadata_without_overwriting_provider_fields() {
        let c = slack(&json("slack-messages.json")["messages"][2]);
        let mut message = ResponseMessage { reply_to_id: Some("keep".to_string()), ..Default::default() };
        c.apply(&mut message);
        assert_eq!(message.reply_to_id.as_deref(), Some("keep"));
        assert_eq!(message.attachments.len(), 1);
        assert!(message.metadata["structure"]["links"].is_array());
        assert_eq!(message.metadata["structure"]["emoji_only"], Value::Bool(false));
    }


    #[test]
    fn our_markdown_has_code_lists_quotes_mentions_links_and_shortcodes() {
        let source = "**Plan** for _fredag_ ~not~ `cargo` :tada:\n> sitat fra @Anna\n- første\n- andre\n1. en\n2. to\n```rust\nfn main() {}\n```\nSe https://northwind.example/plan, @Sam.";
        let c = markdown(source);
        assert!(c.text.starts_with("**Plan** for _fredag_ ~not~ `cargo` :tada:"));
        assert_eq!(c.code, vec![CodeBlock { language: Some("rust".to_string()), code: "fn main() {}".to_string() }]);
        assert_eq!(c.lists, vec![vec!["første".to_string(), "andre".to_string()], vec!["en".to_string(), "to".to_string()]]);
        assert_eq!(c.quotes, vec!["sitat fra @Anna".to_string()]);
        assert_eq!(c.mentions.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["Anna", "Sam"]);
        assert_eq!(c.links.iter().map(|l| l.url.as_str()).collect::<Vec<_>>(), vec!["https://northwind.example/plan"]);
        let emoji = markdown(":tada: :rocket:");
        let mut message = ResponseMessage::default();
        emoji.apply(&mut message);
        assert_eq!(message.metadata["structure"]["emoji_only"], Value::Bool(true));
        assert_eq!(markdown("mail me at a@b.no").mentions, vec![]);
    }
    #[test]
    fn apply_writes_one_reaction_entry_per_user_for_the_frontend() {
        let c = slack(&json("slack-messages.json")["messages"][0]);
        let mut message = ResponseMessage::default();
        c.apply(&mut message);
        assert_eq!(message.metadata["reactions"], serde_json::json!([{ "emoji": ":eyes:", "user": "U0MIKE" }, { "emoji": ":eyes:", "user": "U0NORA" }, { "emoji": ":party-parrot:", "user": "U0NORA" }, { "emoji": ":+1::skin-tone-3:", "user": "U0SAM" }]));
        let t = teams(&json("teams-message.json"));
        let mut message = ResponseMessage::default();
        t.apply(&mut message);
        assert_eq!(message.metadata["reactions"][0], serde_json::json!({ "emoji": ":+1:", "user": "u-nora", "original": "like" }));
        let g = google_chat(&json("gchat-message.json"));
        let mut message = ResponseMessage::default();
        g.apply(&mut message);
        assert_eq!(message.metadata["reactions"].as_array().unwrap().len(), 2);
        assert_eq!(message.thread_id.as_deref(), Some("spaces/AAA/threads/T1"));
    }
}
