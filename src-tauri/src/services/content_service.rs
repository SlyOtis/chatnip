use crate::models::app_setting;
use crate::utils::content_hash;
use crate::models::content::{dimensions, extension_for, is_template, kind_for_mime, normalize_words, sniff_mime, ContentGroup, ContentItem, ContentOrigin, PostContentInput, PutContentInput, KIND_FILE, KIND_GIF, KIND_PROMPT, KIND_STICKER, KIND_VIDEO, SOURCES, SOURCE_ACTION, SOURCE_CONVERSATION, SOURCE_GIPHY, SOURCE_MANUAL, SOURCE_MESSAGE, SOURCE_PERSON, SOURCE_TENOR, SOURCE_UPLOAD, SOURCE_URL};
use crate::services::intelligence_service;
use crate::models::AppSetting;
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Pool, Row, Sqlite};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

pub const NO_KEY: &str = "No GIF API key in settings";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GifResult {
    pub id: String,
    pub source: String,
    pub kind: String,
    pub title: String,
    pub url: String,
    pub preview_url: String,
    pub width: u32,
    pub height: u32,
    pub content_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GifPage {
    pub results: Vec<GifResult>,
    pub next: Option<BTreeMap<String, String>>,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Feed {
    Tenor,
    Giphy,
    TenorStickers,
    GiphyStickers,
}

impl Feed {
    pub fn name(self) -> &'static str {
        match self {
            Feed::Tenor => "tenor",
            Feed::Giphy => "giphy",
            Feed::TenorStickers => "tenor_stickers",
            Feed::GiphyStickers => "giphy_stickers",
        }
    }

    fn sticker(self) -> bool {
        matches!(self, Feed::TenorStickers | Feed::GiphyStickers)
    }

    fn tenor(self) -> bool {
        matches!(self, Feed::Tenor | Feed::TenorStickers)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GifOptions {
    pub tenor_key: String,
    pub giphy_key: String,
    pub rating: String,
    pub locale: String,
    pub stickers: bool,
}

impl GifOptions {
    pub async fn get(pool: &Pool<Sqlite>) -> Result<GifOptions, String> {
        Ok(GifOptions {
            tenor_key: AppSetting::get_or(pool, app_setting::GIF_TENOR_KEY, "").await?,
            giphy_key: AppSetting::get_or(pool, app_setting::GIF_GIPHY_KEY, "").await?,
            rating: AppSetting::get_or(pool, app_setting::GIF_RATING, "pg-13").await?,
            locale: AppSetting::get_or(pool, app_setting::GIF_LOCALE, "en_US").await?,
            stickers: AppSetting::get_or(pool, app_setting::GIF_STICKERS, "false").await? == "true",
        })
    }

    pub fn feeds(&self) -> Vec<Feed> {
        let mut feeds = Vec::new();
        let tenor = !self.tenor_key.trim().is_empty();
        let giphy = !self.giphy_key.trim().is_empty();
        if tenor {
            feeds.push(Feed::Tenor);
        }
        if giphy {
            feeds.push(Feed::Giphy);
        }
        if self.stickers && tenor {
            feeds.push(Feed::TenorStickers);
        }
        if self.stickers && giphy {
            feeds.push(Feed::GiphyStickers);
        }
        feeds
    }
}

fn tenor_filter(rating: &str) -> &'static str {
    match rating {
        "g" => "high",
        "pg" => "medium",
        "r" => "off",
        _ => "low",
    }
}

pub fn feed_request(feed: Feed, options: &GifOptions, query: &str, limit: u32, pos: Option<&str>) -> (String, Vec<(&'static str, String)>) {
    let query = query.trim();
    let mut params: Vec<(&'static str, String)> = vec![("limit", limit.to_string())];
    let url = if feed.tenor() {
        params.push(("key", options.tenor_key.trim().to_string()));
        params.push(("client_key", "chatnip".to_string()));
        params.push(("contentfilter", tenor_filter(&options.rating).to_string()));
        params.push(("locale", options.locale.clone()));
        params.push(("media_filter", if feed.sticker() { "gif_transparent,tinygif_transparent" } else { "gif,tinygif" }.to_string()));
        if feed.sticker() {
            params.push(("searchfilter", "sticker".to_string()));
        }
        if let Some(pos) = pos {
            params.push(("pos", pos.to_string()));
        }
        if query.is_empty() {
            "https://tenor.googleapis.com/v2/featured".to_string()
        } else {
            params.push(("q", query.to_string()));
            "https://tenor.googleapis.com/v2/search".to_string()
        }
    } else {
        params.push(("api_key", options.giphy_key.trim().to_string()));
        params.push(("rating", options.rating.clone()));
        if let Some(pos) = pos {
            params.push(("offset", pos.to_string()));
        }
        let kind = if feed.sticker() { "stickers" } else { "gifs" };
        if query.is_empty() {
            format!("https://api.giphy.com/v1/{}/trending", kind)
        } else {
            params.push(("q", query.to_string()));
            params.push(("lang", options.locale.split(['_', '-']).next().unwrap_or("en").to_lowercase()));
            format!("https://api.giphy.com/v1/{}/search", kind)
        }
    };
    (url, params)
}

async fn fetch_feed(client: &reqwest::Client, feed: Feed, options: &GifOptions, query: &str, limit: u32, pos: Option<&str>) -> Result<(Vec<GifResult>, Option<String>), String> {
    let (url, params) = feed_request(feed, options, query, limit, pos);
    let label = if feed.tenor() { "Tenor" } else { "GIPHY" };
    let response = client.get(&url).query(&params).send().await.map_err(|e| format!("{} search failed: {}", label, e))?;
    let status = response.status();
    let body: Value = response.json().await.map_err(|e| format!("{} returned unreadable JSON: {}", label, e))?;
    if !status.is_success() {
        let detail = body["error"]["message"].as_str().or(body["meta"]["msg"].as_str()).or(body["message"].as_str()).unwrap_or("unknown error");
        return Err(format!("{} {}: {}", label, status, detail));
    }
    Ok(if feed.tenor() { parse_tenor(&body, feed.sticker()) } else { parse_giphy(&body, feed.sticker()) })
}

pub fn library_result(item: &ContentItem) -> GifResult {
    GifResult {
        id: item.id.clone(),
        source: "library".to_string(),
        kind: item.kind.clone(),
        title: item.name.clone(),
        url: item.path.clone(),
        preview_url: item.path.clone(),
        width: item.width.unwrap_or(0) as u32,
        height: item.height.unwrap_or(0) as u32,
        content_id: Some(item.id.clone()),
    }
}

pub fn merge(lists: Vec<Vec<GifResult>>, saved: &HashSet<String>) -> Vec<GifResult> {
    let mut seen_ids = HashSet::new();
    let mut seen_urls: HashSet<String> = saved.clone();
    let mut merged = Vec::new();
    let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
    for index in 0..longest {
        for list in &lists {
            let Some(result) = list.get(index) else { continue };
            if !seen_ids.insert(format!("{}:{}", result.source, result.id)) {
                continue;
            }
            if result.source != "library" && !seen_urls.insert(result.url.clone()) {
                continue;
            }
            merged.push(result.clone());
        }
    }
    merged
}

pub async fn search_gifs(pool: &Pool<Sqlite>, profile_id: Option<&str>, query: &str, limit: u32, cursor: Option<BTreeMap<String, String>>) -> Result<GifPage, String> {
    let options = GifOptions::get(pool).await?;
    let limit = limit.clamp(1, 50);
    let first_page = cursor.is_none();
    let feeds: Vec<Feed> = options
        .feeds()
        .into_iter()
        .filter(|feed| cursor.as_ref().is_none_or(|c| c.contains_key(feed.name())))
        .collect();
    let library: Vec<ContentItem> = match (profile_id, first_page && !query.trim().is_empty()) {
        (Some(profile_id), true) => ContentItem::library(pool, profile_id, None, Some(query)).await?,
        _ => Vec::new(),
    };
    if options.feeds().is_empty() && library.is_empty() {
        return Err(NO_KEY.to_string());
    }
    let client = reqwest::Client::builder().timeout(Duration::from_secs(8)).build().map_err(|e| format!("GIF search failed: {}", e))?;
    let pages = futures::future::join_all(feeds.iter().map(|feed| {
        let pos = cursor.as_ref().and_then(|c| c.get(feed.name())).map(String::as_str);
        fetch_feed(&client, *feed, &options, query, limit, pos)
    }))
    .await;
    let mut lists = vec![library.iter().map(library_result).collect::<Vec<_>>()];
    let mut next = BTreeMap::new();
    let mut errors = Vec::new();
    for (feed, page) in feeds.iter().zip(pages) {
        match page {
            Ok((results, pos)) => {
                if let Some(pos) = pos {
                    next.insert(feed.name().to_string(), pos);
                }
                lists.push(results);
            }
            Err(e) => errors.push(e),
        }
    }
    if !errors.is_empty() && errors.len() == feeds.len() && library.is_empty() {
        return Err(errors.join("; "));
    }
    let saved: HashSet<String> = library.iter().filter_map(|m| m.source_url.clone()).collect();
    let notice = if options.feeds().is_empty() { Some(NO_KEY.to_string()) } else if errors.is_empty() { None } else { Some(errors.join("; ")) };
    Ok(GifPage { results: merge(lists, &saved), next: if next.is_empty() { None } else { Some(next) }, notice })
}

fn tenor_dims(format: &Value) -> (u32, u32) {
    format["dims"]
        .as_array()
        .map(|d| (d.first().and_then(|v| v.as_u64()).unwrap_or(0) as u32, d.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as u32))
        .unwrap_or((0, 0))
}

pub fn parse_tenor(body: &Value, sticker: bool) -> (Vec<GifResult>, Option<String>) {
    let (full, small) = if sticker { ("gif_transparent", "tinygif_transparent") } else { ("gif", "tinygif") };
    let results: Vec<GifResult> = body["results"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let formats = &item["media_formats"];
                    let gif = &formats[full];
                    let preview = if formats[small].is_object() { &formats[small] } else { gif };
                    let url = gif["url"].as_str()?;
                    let dims = tenor_dims(gif);
                    Some(GifResult {
                        id: item["id"].as_str().unwrap_or_default().to_string(),
                        source: "tenor".to_string(),
                        kind: if sticker { "sticker" } else { KIND_GIF }.to_string(),
                        title: item["title"].as_str().filter(|t| !t.is_empty()).or(item["content_description"].as_str()).unwrap_or_default().to_string(),
                        url: url.to_string(),
                        preview_url: preview["url"].as_str().unwrap_or(url).to_string(),
                        width: dims.0,
                        height: dims.1,
                        content_id: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let next = body["next"].as_str().filter(|n| !n.is_empty() && *n != "0" && !results.is_empty()).map(String::from);
    (results, next)
}

pub fn parse_giphy(body: &Value, sticker: bool) -> (Vec<GifResult>, Option<String>) {
    let results: Vec<GifResult> = body["data"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let images = &item["images"];
                    let original = &images["original"];
                    let url = original["url"].as_str()?;
                    let preview = &images["fixed_width_small"];
                    Some(GifResult {
                        id: item["id"].as_str().unwrap_or_default().to_string(),
                        source: "giphy".to_string(),
                        kind: if sticker { "sticker" } else { KIND_GIF }.to_string(),
                        title: item["title"].as_str().unwrap_or_default().to_string(),
                        url: url.to_string(),
                        preview_url: preview["url"].as_str().unwrap_or(url).to_string(),
                        width: original["width"].as_str().and_then(|w| w.parse().ok()).unwrap_or(0),
                        height: original["height"].as_str().and_then(|h| h.parse().ok()).unwrap_or(0),
                        content_id: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let page = &body["pagination"];
    let offset = page["offset"].as_u64().unwrap_or(0);
    let count = page["count"].as_u64().unwrap_or(results.len() as u64);
    let next = match page["total_count"].as_u64() {
        Some(total) if count > 0 && offset + count < total => Some((offset + count).to_string()),
        _ => None,
    };
    (results, next)
}

pub async fn fetch_bytes(input: &PostContentInput) -> Result<Vec<u8>, String> {
    if let Some(data) = input.data.as_deref().filter(|d| !d.trim().is_empty()) {
        let raw = data.split_once(";base64,").map(|(_, rest)| rest).unwrap_or(data);
        return STANDARD.decode(raw.trim()).map_err(|e| format!("The media data is not valid base64: {}", e));
    }
    let url = input.source_url.as_deref().filter(|u| !u.trim().is_empty()).ok_or_else(|| "Content needs a source_url or data".to_string())?;
    if url.starts_with("file://") {
        let path = url::Url::parse(url).ok().and_then(|u| u.to_file_path().ok()).ok_or_else(|| format!("Not a file path: {}", url))?;
        return std::fs::read(&path).map_err(|e| format!("Could not read {}: {}", path.display(), e));
    }
    let response = reqwest::Client::new().get(url).send().await.map_err(|e| format!("Could not download the media: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("Could not download the media: {}", response.status()));
    }
    response.bytes().await.map(|b| b.to_vec()).map_err(|e| format!("Could not read the media: {}", e))
}

fn tags_with_query(tags: &[String], query: Option<&str>) -> Vec<String> {
    let mut merged: Vec<String> = Vec::new();
    for tag in tags.iter().map(String::as_str).chain(query) {
        let tag = tag.trim().to_lowercase();
        if !tag.is_empty() && !merged.contains(&tag) {
            merged.push(tag);
        }
    }
    merged
}

fn favourite_update(item: &ContentItem, input: &PostContentInput) -> Option<PutContentInput> {
    let tags = tags_with_query(&[item.tags.clone(), input.tags.clone().unwrap_or_default()].concat(), input.query.as_deref());
    let keywords = normalize_words(&[item.keywords.clone(), input.keywords.clone().unwrap_or_default()].concat());
    let favourite = input.is_favourite == Some(true) && !item.is_favourite;
    if !favourite && tags == item.tags && keywords == item.keywords {
        return None;
    }
    Some(PutContentInput { content_id: item.id.clone(), is_favourite: input.is_favourite.filter(|f| *f), tags: Some(tags), keywords: Some(keywords), ..PutContentInput::default() })
}

pub const VIDEO_MAX_MB: u64 = 64;

pub fn source_kind_for(input: &PostContentInput) -> &'static str {
    if let Some(kind) = SOURCES.iter().copied().find(|s| Some(*s) == input.source_kind.as_deref()) {
        return kind;
    }
    if input.source_message_id.is_some() {
        return SOURCE_MESSAGE;
    }
    if input.source_conversation_id.is_some() {
        return SOURCE_CONVERSATION;
    }
    if input.source_person_id.is_some() {
        return SOURCE_PERSON;
    }
    if input.source_action_id.is_some() {
        return SOURCE_ACTION;
    }
    match input.source_url.as_deref() {
        Some(url) if url.contains("tenor.com") || url.contains("tenor.googleapis") => SOURCE_TENOR,
        Some(url) if url.starts_with("file://") => SOURCE_UPLOAD,
        Some(url) if url.contains("giphy.com") => SOURCE_GIPHY,
        Some(url) if !url.trim().is_empty() => SOURCE_URL,
        _ if is_template(&input.kind) => SOURCE_MANUAL,
        _ => SOURCE_UPLOAD,
    }
}

fn origin_of(input: &PostContentInput) -> ContentOrigin {
    ContentOrigin {
        source_message_id: input.source_message_id.clone(),
        source_conversation_id: input.source_conversation_id.clone(),
        source_person_id: input.source_person_id.clone(),
        source_action_id: input.source_action_id.clone(),
        ..ContentOrigin::default()
    }
}

async fn merge_existing(pool: &Pool<Sqlite>, profile_id: &str, existing: ContentItem, input: &PostContentInput) -> Result<ContentItem, String> {
    let item = match favourite_update(&existing, input) {
        Some(update) => ContentItem::update(pool, &update).await?,
        None => existing,
    };
    let joining: Vec<&String> = input.group_ids.iter().flatten().filter(|g| !item.group_ids.contains(g)).collect();
    if joining.is_empty() {
        return Ok(item);
    }
    for group_id in joining {
        ContentGroup::put_items(pool, profile_id, group_id, std::slice::from_ref(&item.id), None).await?;
    }
    ContentItem::get(pool, &item.id).await?.ok_or_else(|| "Content not found".to_string())
}

pub async fn store(pool: &Pool<Sqlite>, app_data: &Path, profile_id: &str, input: PostContentInput) -> Result<ContentItem, String> {
    let origin = origin_of(&input);
    if is_template(&input.kind) {
        return store_text(pool, profile_id, input, origin).await;
    }
    if let Some(url) = input.source_url.as_deref().filter(|u| !u.trim().is_empty() && !u.starts_with("data:") && !u.starts_with("file://")) {
        if let Some(existing) = ContentItem::get_source(pool, profile_id, url).await? {
            return merge_existing(pool, profile_id, existing, &input).await;
        }
    }
    let bytes = fetch_bytes(&input).await?;
    store_bytes(pool, app_data, profile_id, input, origin, bytes).await
}

async fn store_text(pool: &Pool<Sqlite>, profile_id: &str, input: PostContentInput, origin: ContentOrigin) -> Result<ContentItem, String> {
    let body = input.body.clone().unwrap_or_default();
    intelligence_service::check_template(&body)?;
    let name = match input.name.trim() {
        "" => body.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| l.chars().take(48).collect::<String>()).unwrap_or_else(|| "Untitled".to_string()),
        name => name.to_string(),
    };
    let tags = tags_with_query(&input.tags.clone().unwrap_or_default(), None);
    let item = ContentItem::insert(pool, profile_id, &input.kind, &name, input.source_url.as_deref().filter(|url| !url.trim().is_empty()), "", "text/plain", None, &tags, None, Some(&body)).await?;
    let origin = ContentOrigin { source_kind: origin.source_kind.clone().or_else(|| Some(source_kind_for(&input).to_string())), keywords: normalize_words(&input.keywords.clone().unwrap_or_default()), ..origin };
    let item = ContentItem::set_origin(pool, &item.id, &origin).await?;
    merge_existing(pool, profile_id, item, &input).await
}

pub async fn store_bytes(pool: &Pool<Sqlite>, app_data: &Path, profile_id: &str, input: PostContentInput, origin: ContentOrigin, bytes: Vec<u8>) -> Result<ContentItem, String> {
    if bytes.is_empty() {
        return Err("The file is empty".to_string());
    }
    let hash = content_hash(&bytes);
    if let Some(existing) = ContentItem::get_hash(pool, profile_id, &hash).await? {
        return merge_existing(pool, profile_id, existing, &input).await;
    }
    let auto = input.kind.trim().is_empty() || input.kind == "auto";
    let visual = sniff_mime(&bytes).map(String::from).or_else(|| input.mime_type.clone().filter(|m| m.starts_with("image/") || m.starts_with("video/")));
    let mime_type = match (visual, auto || input.kind == KIND_FILE) {
        (Some(mime), _) => mime,
        (None, true) => input.mime_type.clone().filter(|m| !m.trim().is_empty()).unwrap_or_else(|| "application/octet-stream".to_string()),
        (None, false) => return Err("Only gif, png, jpeg, webp, mp4, mov and webm files can be a GIF, image, video or sticker".to_string()),
    };
    let kind = if auto {
        kind_for_mime(&mime_type).to_string()
    } else if mime_type.starts_with("video/") && input.kind != KIND_STICKER && input.kind != KIND_FILE {
        KIND_VIDEO.to_string()
    } else {
        input.kind.clone()
    };
    let max_mb = AppSetting::get_u64(pool, app_setting::GIF_MAX_SIZE).await?;
    if kind == KIND_GIF && bytes.len() as u64 > max_mb * 1024 * 1024 {
        return Err(format!("The GIF is larger than the {} MB limit in Settings, Media", max_mb));
    }
    if (kind == KIND_VIDEO || kind == KIND_FILE) && bytes.len() as u64 > VIDEO_MAX_MB * 1024 * 1024 {
        return Err(format!("The {} is larger than {} MB", if kind == KIND_FILE { "file" } else { "video" }, VIDEO_MAX_MB));
    }
    let size = dimensions(&bytes);
    let id = Uuid::new_v4().to_string();
    let relative = format!("media/{}/{}.{}", profile_id, id, extension_for(&mime_type));
    let dir = app_data.join("media").join(profile_id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create the media folder: {}", e))?;
    std::fs::write(app_data.join(&relative), &bytes).map_err(|e| format!("Failed to write the media file: {}", e))?;
    let name = if input.name.trim().is_empty() { id.clone() } else { input.name.trim().to_string() };
    let tags = tags_with_query(&input.tags.clone().unwrap_or_default(), input.query.as_deref());
    let source_url = input.source_url.as_deref().filter(|u| !u.starts_with("data:") && !u.starts_with("file://"));
    let item = ContentItem::insert(pool, profile_id, &kind, &name, source_url, &relative, &mime_type, size, &tags, input.query.as_deref(), None).await?;
    let origin = ContentOrigin {
        source_kind: origin.source_kind.clone().or_else(|| Some(source_kind_for(&input).to_string())),
        content_hash: Some(hash),
        size_bytes: Some(bytes.len() as i64),
        keywords: normalize_words(&[origin.keywords.clone(), input.keywords.clone().unwrap_or_default()].concat()),
        ..origin
    };
    let item = ContentItem::set_origin(pool, &item.id, &origin).await?;
    merge_existing(pool, profile_id, item, &input).await
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub name: String,
    pub url: String,
    pub mime_type: Option<String>,
}

fn is_media_type(mime: &str) -> bool {
    mime.starts_with("image/") || mime.starts_with("video/")
}

pub fn media_candidates(attachments: &[crate::models::Attachment], content: &str) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = attachments
        .iter()
        .filter(|a| is_media_type(&a.content_type))
        .filter_map(|a| a.url.clone().filter(|u| !u.is_empty()).map(|url| Candidate { name: a.name.clone(), url, mime_type: Some(a.content_type.clone()) }))
        .collect();
    static IMG: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let img = IMG.get_or_init(|| regex::Regex::new(r#"(?i)<(?:img|video|source)\b[^>]*\bsrc\s*=\s*["']([^"']+)["']"#).expect("img pattern"));
    for capture in img.captures_iter(content) {
        let url = crate::content::html::decode_entities(&capture[1]);
        if (url.starts_with("http") || url.starts_with("data:")) && !out.iter().any(|c| c.url == url) {
            let name = url.rsplit('/').next().unwrap_or("image").split('?').next().unwrap_or("image").to_string();
            out.push(Candidate { name, url, mime_type: None });
        }
    }
    out
}

pub async fn asset_bytes(pool: &Pool<Sqlite>, app_data: &Path, message_id: &str, name: &str) -> Option<Vec<u8>> {
    let path: Option<String> = sqlx::query_scalar("SELECT path FROM assets WHERE message_id = ?1 AND name = ?2 AND path IS NOT NULL LIMIT 1")
        .bind(message_id)
        .bind(name)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    let path = path?;
    let full = if Path::new(&path).is_absolute() { std::path::PathBuf::from(&path) } else { app_data.join(&path) };
    std::fs::read(full).ok()
}

pub async fn save_from_message(pool: &Pool<Sqlite>, app_data: &Path, profile_id: &str, message_id: &str, index: Option<usize>) -> Result<ContentItem, String> {
    let row = sqlx::query("SELECT m.content, m.attachments, m.conversation_id, m.provider_id, m.sender_name, c.person_id FROM message m JOIN provider p ON p.id = m.provider_id LEFT JOIN conversation c ON c.id = m.conversation_id WHERE m.id = ?1 AND p.profile_id = ?2")
        .bind(message_id)
        .bind(profile_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to load the message: {}", e))?
        .ok_or_else(|| "Message not found".to_string())?;
    let attachments: Vec<crate::models::Attachment> = row.get::<Option<String>, _>("attachments").and_then(|a| serde_json::from_str(&a).ok()).unwrap_or_default();
    let content: String = row.get::<Option<String>, _>("content").unwrap_or_default();
    let candidates = media_candidates(&attachments, &content);
    let candidate = candidates.get(index.unwrap_or(0)).cloned().ok_or_else(|| "The message has no image, GIF or video to save".to_string())?;
    let bytes = match asset_bytes(pool, app_data, message_id, &candidate.name).await {
        Some(bytes) => bytes,
        None => fetch_bytes(&PostContentInput { source_url: Some(candidate.url.clone()), data: candidate.url.starts_with("data:").then(|| candidate.url.clone()), mime_type: candidate.mime_type.clone(), ..PostContentInput::default() }).await?,
    };
    let sender: Option<String> = row.get("sender_name");
    let name = std::path::Path::new(&candidate.name).file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| sender.clone().map(|s| format!("From {}", s)).unwrap_or_else(|| "Saved".to_string()));
    let input = PostContentInput { kind: "auto".into(), name, source_url: Some(candidate.url).filter(|u| !u.starts_with("data:")), mime_type: candidate.mime_type, source_kind: Some(SOURCE_MESSAGE.into()), ..PostContentInput::default() };
    let origin = ContentOrigin {
        source_kind: Some(SOURCE_MESSAGE.into()),
        source_message_id: Some(message_id.to_string()),
        source_conversation_id: row.get("conversation_id"),
        source_provider_id: row.get("provider_id"),
        source_person_id: row.try_get("person_id").ok().flatten(),
        ..ContentOrigin::default()
    };
    store_bytes(pool, app_data, profile_id, input, origin, bytes).await
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostContentRenderInput {
    pub content_id: Option<String>,
    pub body: Option<String>,
    pub values: Option<Value>,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    #[serde(default)]
    pub used: Option<bool>,
    #[serde(default)]
    pub sample: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RenderedContent {
    pub text: String,
    pub variables: Vec<intelligence_service::TemplateVariable>,
    pub missing: Vec<String>,
}

pub async fn owned(pool: &Pool<Sqlite>, profile_id: &str, content_id: &str) -> Result<ContentItem, String> {
    ContentItem::get(pool, content_id).await?.filter(|item| item.profile_id == profile_id).ok_or_else(|| format!("{} is not in your content", content_id))
}

pub async fn template_context(pool: &Pool<Sqlite>, conversation_id: Option<&str>, message_id: Option<&str>, sample: bool) -> Result<Value, String> {
    if sample && conversation_id.is_none() {
        return Ok(intelligence_service::sample_context());
    }
    let profile = crate::models::Profile::get_active(pool).await?.ok_or_else(|| "No active profile".to_string())?;
    let mut context = intelligence_service::base_context(pool, &profile, conversation_id).await?;
    let message = match (message_id, conversation_id) {
        (Some(id), _) => Some(id.to_string()),
        (None, None) => None,
        (None, Some(conversation_id)) => sqlx::query_scalar("SELECT id FROM message WHERE conversation_id = ?1 AND deleted_datetime IS NULL ORDER BY timestamp DESC LIMIT 1").bind(conversation_id).fetch_optional(pool).await.map_err(|e| format!("Failed to load the message: {}", e))?,
    };
    if let (Some(id), Value::Object(map)) = (message, &mut context) {
        map.insert("message".into(), crate::services::action_service::prompt_message(pool, &id).await?);
    }
    Ok(context)
}

pub async fn render(pool: &Pool<Sqlite>, profile_id: &str, input: &PostContentRenderInput) -> Result<RenderedContent, String> {
    let item = match input.content_id.as_deref() {
        Some(id) => Some(owned(pool, profile_id, id).await?),
        None => None,
    };
    let body = input.body.clone().or_else(|| item.as_ref().and_then(|i| i.body.clone())).ok_or_else(|| "Pass a content_id of a reply, email, prompt or snippet, or a body".to_string())?;
    let mut context = template_context(pool, input.conversation_id.as_deref(), input.message_id.as_deref(), input.sample == Some(true)).await?;
    if let Some(values) = input.values.clone() {
        intelligence_service::merge(&mut context, values);
    }
    let missing = intelligence_service::missing_variables(&body, &context);
    let text = intelligence_service::render(&body, &context)?;
    if let (Some(item), Some(true)) = (&item, input.used) {
        ContentItem::touch(pool, &item.id).await?;
    }
    Ok(RenderedContent { text, variables: intelligence_service::variables(&body), missing })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RenderedSignature {
    pub content_id: String,
    pub name: String,
    pub text: String,
    pub html: Option<String>,
}

pub const SIGNATURE_DELIMITER: &str = "-- ";

fn is_html(body: &str) -> bool {
    let trimmed = body.trim_start();
    trimmed.starts_with('<') && trimmed.contains('>')
}

pub async fn render_signature(pool: &Pool<Sqlite>, profile_id: &str, content_id: &str, provider_id: &str, alias: &str, conversation_id: Option<&str>) -> Result<RenderedSignature, String> {
    let item = owned(pool, profile_id, content_id).await?;
    if item.kind != crate::models::content::KIND_SIGNATURE {
        return Err(format!("{} is not a signature", item.name));
    }
    let provider = crate::providers::models::Provider::load_from_db(pool, provider_id).await?;
    let values = serde_json::json!({ "account": { "email": alias, "alias": alias, "provider": provider.name } });
    let rendered = render(pool, profile_id, &PostContentRenderInput { content_id: Some(item.id.clone()), conversation_id: conversation_id.map(String::from), values: Some(values), ..PostContentRenderInput::default() }).await?;
    let html = if is_html(&rendered.text) { Some(rendered.text.clone()) } else { None };
    let text = match &html {
        Some(html) => crate::content::html::flat_text(html),
        None => rendered.text.clone(),
    };
    Ok(RenderedSignature { content_id: item.id, name: item.name, text, html })
}

pub async fn get_signature(pool: &Pool<Sqlite>, profile_id: &str, provider_id: &str, alias: &str, mode: &str, conversation_id: Option<&str>) -> Result<Option<RenderedSignature>, String> {
    let Some(bound) = crate::models::content::ContentSignature::get_bound(pool, profile_id, provider_id, alias, mode).await? else { return Ok(None) };
    render_signature(pool, profile_id, &bound.content_id, provider_id, alias, conversation_id).await.map(Some)
}

pub fn put_signed(draft: &mut crate::providers::traits::OutgoingMessage, signature: &RenderedSignature) {
    let body = draft.content.trim_end().to_string();
    match &signature.html {
        Some(html) => {
            let current = draft.html.clone().unwrap_or_else(|| match draft.content_type.as_str() {
                "html" => body.clone(),
                "markdown" => crate::content::markdown::html(&body),
                _ => crate::utils::text_extraction::html_escape(&body).replace('\n', "<br>"),
            });
            draft.html = Some(format!("{}<br><br><div class=\"chatnip-signature\">{}<br>{}</div>", current, SIGNATURE_DELIMITER, html));
            draft.content = format!("{}\n\n{}\n{}", body, SIGNATURE_DELIMITER, signature.text.trim());
        }
        None => {
            draft.content = format!("{}\n\n{}\n{}", body, SIGNATURE_DELIMITER, signature.text.trim());
            draft.html = draft.html.take().map(|html| format!("{}<br><br><div class=\"chatnip-signature\">{}<br>{}</div>", html, SIGNATURE_DELIMITER, crate::content::markdown::html(signature.text.trim())));
        }
    }
}

pub async fn post_signature_import(pool: &Pool<Sqlite>, profile_id: &str, provider_id: &str) -> Result<Vec<ContentItem>, String> {
    let mut provider = crate::providers::models::Provider::load_from_db(pool, provider_id).await?;
    let client = crate::services::connection_service::provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = crate::services::connection_service::ensure_fresh(pool, &mut provider, client.clone()).await?;
    let bound = crate::models::content::ContentSignature::get_all(pool, profile_id, None).await?;
    let mut out = Vec::new();
    for alias in client.get_aliases(&auth).await? {
        let Some(html) = alias.signature.clone() else { continue };
        let mut known = None;
        for binding in bound.iter().filter(|b| b.provider_id == provider.id && b.alias == alias.email) {
            let item = owned(pool, profile_id, &binding.content_id).await?;
            if item.body.as_deref().map(str::trim) == Some(html.trim()) {
                known = Some(item);
            }
        }
        let item = match known {
            Some(item) => item,
            None => {
                let input = PostContentInput { kind: crate::models::content::KIND_SIGNATURE.to_string(), name: format!("{} ({})", alias.name.clone().unwrap_or_else(|| alias.email.clone()), provider.name), body: Some(html), source_kind: Some(SOURCE_MANUAL.to_string()), ..PostContentInput::default() };
                let origin = origin_of(&input);
                store_text(pool, profile_id, input, origin).await?
            }
        };
        crate::models::content::ContentSignature::put(pool, &crate::models::content::ContentSignature { content_id: item.id.clone(), provider_id: provider.id.clone(), alias: alias.email.clone(), use_new: true, use_reply: true, use_forward: true }).await?;
        out.push(item);
    }
    Ok(out)
}

pub async fn put_signature_sync(pool: &Pool<Sqlite>, profile_id: &str, content_id: &str) -> Result<usize, String> {
    let mut written = 0;
    for binding in crate::models::content::ContentSignature::get_all(pool, profile_id, Some(content_id)).await? {
        let mut provider = crate::providers::models::Provider::load_from_db(pool, &binding.provider_id).await?;
        if !provider.capabilities.signature_sync || binding.alias.is_empty() {
            continue;
        }
        let client = crate::services::connection_service::provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
        let auth = crate::services::connection_service::ensure_fresh(pool, &mut provider, client.clone()).await?;
        let rendered = render_signature(pool, profile_id, content_id, &provider.id, &binding.alias, None).await?;
        let html = rendered.html.clone().unwrap_or_else(|| crate::content::markdown::html(&rendered.text));
        if client.put_signature(&auth, &binding.alias, &html).await? {
            written += 1;
        }
    }
    Ok(written)
}

pub async fn send(pool: &Pool<Sqlite>, app_data: &Path, profile_id: &str, content_id: &str, conversation_id: &str, provider_id: Option<&str>) -> Result<crate::models::Message, String> {
    let item = owned(pool, profile_id, content_id).await?;
    if item.kind == KIND_PROMPT {
        return Err(format!("{} is an AI prompt; use it in an action instead of sending it", item.name));
    }
    let input = if is_template(&item.kind) {
        let rendered = render(pool, profile_id, &PostContentRenderInput { content_id: Some(item.id.clone()), conversation_id: Some(conversation_id.to_string()), ..PostContentRenderInput::default() }).await?;
        crate::models::SendMessageInput { conversation_id: conversation_id.to_string(), content: rendered.text, content_type: Some("markdown".into()), provider_id: provider_id.map(String::from), metadata: None, attachments: None }
    } else {
        crate::models::SendMessageInput {
            conversation_id: conversation_id.to_string(),
            content: String::new(),
            content_type: Some("text".into()),
            provider_id: provider_id.map(String::from),
            metadata: None,
            attachments: Some(vec![crate::models::OutgoingAttachmentInput { content_id: Some(item.id.clone()), name: None, mime_type: None, data: None }]),
        }
    };
    let message = crate::services::outbound_service::send(pool, app_data, input).await?;
    if is_template(&item.kind) {
        ContentItem::touch(pool, &item.id).await?;
    }
    Ok(message)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ContentUse {
    pub action_id: String,
    pub action_name: String,
    pub node_id: String,
    pub node_label: String,
    pub via: String,
}

pub fn references(item: &ContentItem, node: &crate::models::action::Node) -> Option<&'static str> {
    let prompts = node.config.get("prompts").and_then(Value::as_object);
    if prompts.is_some_and(|p| p.values().any(|o| o.get("content_id").and_then(Value::as_str) == Some(item.id.as_str()))) {
        return Some("prompt");
    }
    let body = node.body.to_lowercase();
    let named = [item.id.as_str(), item.name.as_str()].into_iter().filter(|n| !n.trim().is_empty()).any(|name| ["prompt", "content"].iter().any(|f| body.contains(&format!("{}(\"{}\"", f, name.to_lowercase()))));
    named.then_some("script")
}

pub async fn uses(pool: &Pool<Sqlite>, profile_id: &str, item: &ContentItem) -> Result<Vec<ContentUse>, String> {
    let mut out = Vec::new();
    for action in crate::models::action::Action::all(pool, profile_id).await? {
        for node in &action.graph.nodes {
            if let Some(via) = references(item, node) {
                out.push(ContentUse { action_id: action.id.clone(), action_name: action.name.clone(), node_id: node.id.clone(), node_label: node.label.clone(), via: via.to_string() });
            }
        }
    }
    Ok(out)
}

pub fn read(app_data: &Path, item: &ContentItem) -> Result<Vec<u8>, String> {
    std::fs::read(app_data.join(&item.path)).map_err(|e| format!("The file for {} is missing: {}", item.name, e))
}

pub fn remove_file(app_data: &Path, item: &ContentItem) {
    if item.path.is_empty() {
        return;
    }
    std::fs::remove_file(app_data.join(&item.path)).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signature_goes_below_the_delimiter_and_comes_off_again_for_summaries() {
        let plain = RenderedSignature { content_id: "s".into(), name: "Me".into(), text: "Milo\nNorthwind".into(), html: None };
        let mut draft = crate::providers::traits::OutgoingMessage { content: "Hei Anna".into(), content_type: "markdown".into(), ..Default::default() };
        put_signed(&mut draft, &plain);
        assert_eq!(draft.content, "Hei Anna\n\n-- \nMilo\nNorthwind");
        assert_eq!(crate::content::mail::split_signature_text(&draft.content).0, "Hei Anna");
        let html = RenderedSignature { content_id: "s".into(), name: "Me".into(), text: "Milo".into(), html: Some("<b>Milo</b>".into()) };
        let mut mail = crate::providers::traits::OutgoingMessage { content: "Hei".into(), content_type: "markdown".into(), ..Default::default() };
        put_signed(&mut mail, &html);
        let body = mail.html.clone().unwrap();
        assert!(body.contains("chatnip-signature") && body.contains("<b>Milo</b>"));
        assert!(!crate::content::html::split_signature(&body).0.contains("Milo"));
    }
    use crate::models::content::KIND_REPLY;
    use crate::test_helpers::{test_pool, test_profile};

    fn hit(source: &str, id: &str, url: &str) -> GifResult {
        GifResult { id: id.into(), source: source.into(), kind: KIND_GIF.into(), title: id.into(), url: url.into(), preview_url: url.into(), width: 1, height: 1, content_id: None }
    }

    fn tiny_gif_data() -> (Vec<u8>, String) {
        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&[0x02, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x3b]);
        let data = format!("data:image/gif;base64,{}", STANDARD.encode(&gif));
        (gif, data)
    }

    fn post(data: &str, source_url: Option<&str>) -> PostContentInput {
        PostContentInput { kind: KIND_GIF.to_string(), name: "Wave".into(), source_url: source_url.map(String::from), data: Some(data.to_string()), ..PostContentInput::default() }
    }

    fn options(tenor: &str, giphy: &str, stickers: bool) -> GifOptions {
        GifOptions { tenor_key: tenor.into(), giphy_key: giphy.into(), rating: "pg".into(), locale: "nb_NO".into(), stickers }
    }

    #[test]
    fn parses_tenor_results_and_cursor() {
        let body: Value = serde_json::from_str(
            r#"{"results":[{"id":"abc","title":"","content_description":"Slow clap","media_formats":{"gif":{"url":"https://t/abc.gif","dims":[498,280]},"tinygif":{"url":"https://t/abc-tiny.gif","dims":[220,124]}}},{"id":"nogif","media_formats":{}}],"next":"CAgQ"}"#,
        )
        .unwrap();
        let (results, next) = parse_tenor(&body, false);
        assert_eq!(
            results,
            vec![GifResult { id: "abc".into(), source: "tenor".into(), kind: "gif".into(), title: "Slow clap".into(), url: "https://t/abc.gif".into(), preview_url: "https://t/abc-tiny.gif".into(), width: 498, height: 280, content_id: None }]
        );
        assert_eq!(next.as_deref(), Some("CAgQ"));
        let sticker: Value = serde_json::from_str(r#"{"results":[{"id":"s","media_formats":{"gif_transparent":{"url":"https://t/s.gif","dims":[10,10]}}}],"next":""}"#).unwrap();
        let (results, next) = parse_tenor(&sticker, true);
        assert_eq!((results[0].kind.as_str(), results[0].preview_url.as_str()), ("sticker", "https://t/s.gif"));
        assert_eq!(next, None);
    }

    #[test]
    fn parses_giphy_results_and_offset() {
        let body: Value = serde_json::from_str(
            r#"{"data":[{"id":"g1","title":"Party","images":{"original":{"url":"https://g/o.gif","width":"480","height":"270"},"fixed_width_small":{"url":"https://g/s.gif"}}}],"pagination":{"offset":25,"count":1,"total_count":40}}"#,
        )
        .unwrap();
        let (results, next) = parse_giphy(&body, false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "giphy");
        assert_eq!(results[0].preview_url, "https://g/s.gif");
        assert_eq!((results[0].width, results[0].height), (480, 270));
        assert_eq!(next.as_deref(), Some("26"));
        let last: Value = serde_json::from_str(r#"{"data":[],"pagination":{"offset":40,"count":0,"total_count":40}}"#).unwrap();
        assert_eq!(parse_giphy(&last, true).1, None);
    }

    #[test]
    fn merge_interleaves_sources_and_drops_duplicates() {
        let library = vec![hit("library", "m1", "media/m1.gif")];
        let tenor = vec![hit("tenor", "t1", "https://t/1.gif"), hit("tenor", "t2", "https://t/2.gif"), hit("tenor", "t1", "https://t/1.gif"), hit("tenor", "t3", "https://saved.gif")];
        let giphy = vec![hit("giphy", "g1", "https://g/1.gif"), hit("giphy", "g2", "https://t/2.gif")];
        let saved: HashSet<String> = ["https://saved.gif".to_string()].into();
        let merged = merge(vec![library, tenor, giphy], &saved);
        let keys: Vec<String> = merged.iter().map(|r| format!("{}:{}", r.source, r.id)).collect();
        assert_eq!(keys, vec!["library:m1", "tenor:t1", "giphy:g1", "tenor:t2"]);
        assert!(merge(vec![], &HashSet::new()).is_empty());
    }

    #[test]
    fn feeds_follow_keys_and_stickers() {
        assert!(options("", "", true).feeds().is_empty());
        assert_eq!(options("t", "", false).feeds(), vec![Feed::Tenor]);
        assert_eq!(options("t", "g", true).feeds(), vec![Feed::Tenor, Feed::Giphy, Feed::TenorStickers, Feed::GiphyStickers]);
    }

    #[test]
    fn requests_carry_rating_locale_paging_and_trending() {
        let opts = options("tk", "gk", false);
        let (url, params) = feed_request(Feed::Tenor, &opts, " cats ", 20, Some("CAgQ"));
        assert_eq!(url, "https://tenor.googleapis.com/v2/search");
        for pair in [("q", "cats"), ("contentfilter", "medium"), ("locale", "nb_NO"), ("pos", "CAgQ"), ("key", "tk")] {
            assert!(params.contains(&(pair.0, pair.1.to_string())), "{:?}", pair);
        }
        let (url, params) = feed_request(Feed::TenorStickers, &opts, "", 20, None);
        assert_eq!(url, "https://tenor.googleapis.com/v2/featured");
        assert!(params.contains(&("searchfilter", "sticker".to_string())));
        let (url, params) = feed_request(Feed::Giphy, &opts, "cats", 20, Some("40"));
        assert_eq!(url, "https://api.giphy.com/v1/gifs/search");
        for pair in [("rating", "pg"), ("lang", "nb"), ("offset", "40"), ("api_key", "gk")] {
            assert!(params.contains(&(pair.0, pair.1.to_string())), "{:?}", pair);
        }
        assert_eq!(feed_request(Feed::GiphyStickers, &opts, "", 20, None).0, "https://api.giphy.com/v1/stickers/trending");
    }

    #[tokio::test]
    async fn search_without_a_key_names_the_setting_and_still_finds_the_library() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        assert_eq!(search_gifs(&pool, Some(&profile), "cats", 5, None).await.unwrap_err(), NO_KEY);
        let dir = std::env::temp_dir().join(format!("chatnip-media-{}", Uuid::new_v4()));
        let (_, data) = tiny_gif_data();
        let mut input = post(&data, None);
        input.query = Some("Cats".into());
        store(&pool, &dir, &profile, input).await.unwrap();
        let page = search_gifs(&pool, Some(&profile), "cats", 5, None).await.unwrap();
        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].source, "library");
        assert_eq!(page.notice.as_deref(), Some(NO_KEY));
        assert_eq!(page.next, None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn stores_base64_data_and_reads_it_back() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let dir = std::env::temp_dir().join(format!("chatnip-media-{}", Uuid::new_v4()));
        let (gif, data) = tiny_gif_data();
        let mut input = post(&data, None);
        input.name = "  Wave ".into();
        input.tags = Some(vec!["hello".to_string()]);
        let item = store(&pool, &dir, &profile, input).await.unwrap();
        assert_eq!(item.name, "Wave");
        assert_eq!(item.mime_type, "image/gif");
        assert_eq!((item.width, item.height), (Some(2), Some(3)));
        assert!(item.path.starts_with("media/") && item.path.ends_with(".gif"));
        assert!(!item.is_favourite);
        assert_eq!(read(&dir, &item).unwrap(), gif);
        remove_file(&dir, &item);
        assert!(read(&dir, &item).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn starring_saves_once_tags_the_query_and_joins_groups() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let dir = std::env::temp_dir().join(format!("chatnip-media-{}", Uuid::new_v4()));
        let (_, data) = tiny_gif_data();
        let mut star = post(&data, Some("https://t/clap.gif"));
        star.query = Some("Slow Clap".into());
        star.is_favourite = Some(true);
        let first = store(&pool, &dir, &profile, star.clone()).await.unwrap();
        assert!(first.is_favourite);
        assert_eq!(first.tags, vec!["slow clap".to_string()]);
        assert_eq!(first.query.as_deref(), Some("Slow Clap"));
        star.query = Some("bravo".into());
        let team = ContentGroup::post(&pool, &profile, &crate::models::content::PostContentGroupInput { name: "Team".into(), ..Default::default() }).await.unwrap();
        star.group_ids = Some(vec![team.id.clone()]);
        let again = store(&pool, &dir, &profile, star).await.unwrap();
        assert_eq!(again.id, first.id);
        assert_eq!(again.tags, vec!["slow clap".to_string(), "bravo".to_string()]);
        assert_eq!(again.group_ids, vec![team.id.clone()]);
        assert_eq!(ContentItem::library(&pool, &profile, None, None).await.unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn refuses_media_without_a_source_unknown_bytes_or_over_the_limit() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let dir = std::env::temp_dir().join(format!("chatnip-media-{}", Uuid::new_v4()));
        let mut none = post("", None);
        none.data = None;
        assert!(store(&pool, &dir, &profile, none).await.unwrap_err().contains("source_url or data"));
        let mut text = post(&STANDARD.encode(b"not an image"), None);
        text.mime_type = Some("text/plain".into());
        assert!(store(&pool, &dir, &profile, text).await.unwrap_err().contains("Only gif"));
        AppSetting::put(&pool, app_setting::GIF_MAX_SIZE, "0").await.unwrap();
        let (_, data) = tiny_gif_data();
        assert!(store(&pool, &dir, &profile, post(&data, None)).await.unwrap_err().contains("0 MB limit"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn identical_bytes_are_stored_once_and_merge_their_words() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let dir = std::env::temp_dir().join(format!("chatnip-media-{}", Uuid::new_v4()));
        let (_, data) = tiny_gif_data();
        let mut first = post(&data, None);
        first.keywords = Some(vec!["Hello".into()]);
        let first = store(&pool, &dir, &profile, first).await.unwrap();
        assert_eq!(first.source_kind.as_deref(), Some(SOURCE_UPLOAD));
        assert_eq!(first.keywords, vec!["hello".to_string()]);
        assert!(first.content_hash.as_deref().is_some_and(|h| h.len() == 64));
        let mut again = post(&data, None);
        again.name = "Other".into();
        again.tags = Some(vec!["wave".into()]);
        again.keywords = Some(vec!["hi".into()]);
        let again = store(&pool, &dir, &profile, again).await.unwrap();
        assert_eq!(again.id, first.id);
        assert_eq!(again.name, "Wave");
        assert_eq!(again.tags, vec!["wave".to_string()]);
        assert_eq!(again.keywords, vec!["hello".to_string(), "hi".to_string()]);
        assert_eq!(ContentItem::library(&pool, &profile, None, None).await.unwrap().len(), 1);
        let other = test_profile(&pool).await;
        let theirs = store(&pool, &dir, &other, post(&data, None)).await.unwrap();
        assert_ne!(theirs.id, first.id);
        assert!(theirs.path.contains(&other));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn video_bytes_are_kept_as_video() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let dir = std::env::temp_dir().join(format!("chatnip-media-{}", Uuid::new_v4()));
        let mut mp4 = vec![0, 0, 0, 0x18];
        mp4.extend_from_slice(b"ftypisom");
        mp4.extend_from_slice(&[0; 12]);
        let mut input = post(&format!("data:video/mp4;base64,{}", STANDARD.encode(&mp4)), None);
        input.kind = "auto".into();
        let item = store(&pool, &dir, &profile, input).await.unwrap();
        assert_eq!(item.kind, KIND_VIDEO);
        assert_eq!(item.mime_type, "video/mp4");
        assert!(item.path.ends_with(".mp4"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn candidates_come_from_media_attachments_then_inline_images() {
        let attachment = |name: &str, kind: &str, url: Option<&str>| crate::models::Attachment { id: name.into(), name: name.into(), content_type: kind.into(), size: 1, is_inline: false, content_id: None, url: url.map(String::from) };
        let found = media_candidates(
            &[attachment("doc.pdf", "application/pdf", Some("https://x/doc.pdf")), attachment("party.gif", "image/gif", Some("https://x/party.gif")), attachment("clip.mp4", "video/mp4", None)],
            r#"<p>look</p><img src="https://x/inline.png?a=1&amp;b=2"><img src="cid:logo">"#,
        );
        assert_eq!(found.iter().map(|c| c.url.as_str()).collect::<Vec<_>>(), vec!["https://x/party.gif", "https://x/inline.png?a=1&b=2"]);
        assert_eq!(found[0].mime_type.as_deref(), Some("image/gif"));
        assert_eq!(found[1].name, "inline.png");
        assert!(media_candidates(&[], "plain text").is_empty());
    }

    #[tokio::test]
    async fn saves_media_from_a_received_message_once() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let dir = std::env::temp_dir().join(format!("chatnip-media-{}", Uuid::new_v4()));
        let provider = crate::test_helpers::test_provider(&pool, &profile, "mock").await;
        let conversation = crate::test_helpers::test_conversation(&pool, "Saved things", "direct_message").await;
        let message = crate::test_helpers::test_message(&pool, &conversation, &provider, "haha").await;
        let (_, data) = tiny_gif_data();
        let attachments = serde_json::json!([{ "id": "a1", "name": "dance.gif", "content_type": "image/gif", "size": 14, "is_inline": false, "content_id": null, "url": data }]);
        sqlx::query("UPDATE message SET attachments = ?1, sender_name = 'Kari' WHERE id = ?2").bind(attachments.to_string()).bind(&message).execute(&pool).await.unwrap();
        let saved = save_from_message(&pool, &dir, &profile, &message, None).await.unwrap();
        assert_eq!(saved.kind, KIND_GIF);
        assert_eq!(saved.name, "dance");
        assert_eq!(saved.source_kind.as_deref(), Some(SOURCE_MESSAGE));
        assert_eq!(saved.source_message_id.as_deref(), Some(message.as_str()));
        assert_eq!(saved.source_conversation_id.as_deref(), Some(conversation.as_str()));
        assert_eq!(saved.source_provider_id.as_deref(), Some(provider.as_str()));
        assert_eq!(saved.source_url, None);
        let again = save_from_message(&pool, &dir, &profile, &message, Some(0)).await.unwrap();
        assert_eq!(again.id, saved.id);
        assert!(save_from_message(&pool, &dir, &profile, &message, Some(3)).await.unwrap_err().contains("no image"));
        let stranger = test_profile(&pool).await;
        assert!(save_from_message(&pool, &dir, &stranger, &message, None).await.unwrap_err().contains("not found"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn reads_dropped_files_without_keeping_their_path() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let dir = std::env::temp_dir().join(format!("chatnip-media-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let (gif, _) = tiny_gif_data();
        let file = dir.join("my party.gif");
        std::fs::write(&file, &gif).unwrap();
        let url = url::Url::from_file_path(&file).unwrap().to_string();
        let mut input = post("", Some(&url));
        input.data = None;
        input.kind = "auto".into();
        let item = store(&pool, &dir, &profile, input).await.unwrap();
        assert_eq!(item.kind, KIND_GIF);
        assert_eq!(item.source_url, None);
        assert_eq!(item.source_kind.as_deref(), Some(SOURCE_UPLOAD));
        let mut text = post("", Some(&url::Url::from_file_path(dir.join("missing.gif")).unwrap().to_string()));
        text.data = None;
        assert!(store(&pool, &dir, &profile, text).await.unwrap_err().contains("Could not read"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn text_templates_are_stored_with_where_they_came_from() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let dir = std::env::temp_dir().join(format!("chatnip-content-{}", Uuid::new_v4()));
        let by_hand = PostContentInput { kind: KIND_REPLY.into(), body: Some("\n  Takk for sist, {{ .person.first_name | default \"du\" }}!\nHilsen".into()), ..PostContentInput::default() };
        let item = store(&pool, &dir, &profile, by_hand).await.unwrap();
        assert_eq!(item.name, "Takk for sist, {{ .person.first_name | default \"du\" }}!".chars().take(48).collect::<String>());
        assert_eq!((item.source_kind.as_deref(), item.path.as_str(), item.mime_type.as_str()), (Some(SOURCE_MANUAL), "", "text/plain"));
        let from_person = PostContentInput { kind: KIND_PROMPT.into(), name: "Decline politely".into(), body: Some("Write a polite no to {{ .person.name }}".into()), source_person_id: Some("p1".into()), tags: Some(vec!["No".into()]), ..PostContentInput::default() };
        let prompt = store(&pool, &dir, &profile, from_person).await.unwrap();
        assert_eq!((prompt.source_kind.as_deref(), prompt.source_person_id.as_deref()), (Some(SOURCE_PERSON), Some("p1")));
        assert_eq!(prompt.tags, vec!["no".to_string()]);
        let from_action = PostContentInput { kind: "snippet".into(), name: "Sig".into(), body: Some("Milo".into()), source_action_id: Some("a1".into()), ..PostContentInput::default() };
        assert_eq!(store(&pool, &dir, &profile, from_action).await.unwrap().source_kind.as_deref(), Some(SOURCE_ACTION));
        let broken = PostContentInput { kind: "snippet".into(), name: "Bad".into(), body: Some("{{ if .x }}".into()), ..PostContentInput::default() };
        assert!(store(&pool, &dir, &profile, broken).await.is_err());
        let explicit = PostContentInput { kind: "snippet".into(), name: "Link".into(), body: Some("x".into()), source_kind: Some(SOURCE_URL.into()), ..PostContentInput::default() };
        assert_eq!(store(&pool, &dir, &profile, explicit).await.unwrap().source_kind.as_deref(), Some(SOURCE_URL));
    }

    #[tokio::test]
    async fn rendering_fills_the_conversation_and_counts_use_on_request() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let dir = std::env::temp_dir().join(format!("chatnip-content-{}", Uuid::new_v4()));
        let conversation = crate::test_helpers::test_conversation(&pool, "Kari Nordmann", "direct_message").await;
        let body = "Hei {{ .person.first_name | default \"der\" }}, takk for {{ .conversation.name | lower }}.{{ if .weather }} {{ .weather }}{{ end }} {{ .me.first_name }}";
        let item = store(&pool, &dir, &profile, PostContentInput { kind: KIND_REPLY.into(), name: "Thanks".into(), body: Some(body.into()), ..PostContentInput::default() }).await.unwrap();
        let input = PostContentRenderInput { content_id: Some(item.id.clone()), conversation_id: Some(conversation.clone()), values: Some(serde_json::json!({ "me": { "first_name": "Milo" } })), used: Some(true), ..PostContentRenderInput::default() };
        let rendered = render(&pool, &profile, &input).await.unwrap();
        assert_eq!(rendered.text, "Hei der, takk for kari nordmann. Milo");
        assert!(rendered.missing.contains(&"weather".to_string()) && rendered.missing.contains(&"person.first_name".to_string()));
        assert!(rendered.variables.iter().any(|v| v.name == "conversation.name"));
        assert_eq!(ContentItem::get(&pool, &item.id).await.unwrap().unwrap().used_count, 1);
        let preview = render(&pool, &profile, &PostContentRenderInput { body: Some("{{ .person.first_name | upper }}".into()), sample: Some(true), ..PostContentRenderInput::default() }).await.unwrap();
        assert_eq!(preview.text, "KARI");
        let stranger = test_profile(&pool).await;
        assert!(render(&pool, &stranger, &PostContentRenderInput { content_id: Some(item.id.clone()), ..PostContentRenderInput::default() }).await.is_err());
        assert!(render(&pool, &profile, &PostContentRenderInput::default()).await.is_err());
    }

    #[tokio::test]
    async fn nodes_reference_content_by_prompt_override_or_by_script() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let dir = std::env::temp_dir().join(format!("chatnip-content-{}", Uuid::new_v4()));
        let item = store(&pool, &dir, &profile, PostContentInput { kind: KIND_PROMPT.into(), name: "Polite no".into(), body: Some("Say no".into()), ..PostContentInput::default() }).await.unwrap();
        let id = item.id.clone();
        let node = |config: Value, body: &str| crate::models::action::Node { id: "n".into(), kind: "script".into(), label: "Ask".into(), config, body: body.into(), x: 0.0, y: 0.0 };
        assert_eq!(references(&item, &node(serde_json::json!({ "prompts": { "script_ai": { "content_id": id } } }), "")), Some("prompt"));
        assert_eq!(references(&item, &node(Value::Null, "let a = prompt(\"Polite no\", #{});")), Some("script"));
        assert_eq!(references(&item, &node(Value::Null, &format!("content(\"{}\", #{{}})", id))), Some("script"));
        assert_eq!(references(&item, &node(serde_json::json!({ "prompts": { "script_ai": { "content_id": "c2" } } }), "prompt(\"sort\")")), None);
    }
}
