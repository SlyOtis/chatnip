use crate::content::html::{attr, decode_entities, flat_text};
use crate::content::links::{clean_url, LinkKind};
use crate::content::rx;
use crate::models::app_setting;
use crate::models::content::{dimensions, extension_for, sniff_mime};
use crate::models::AppSetting;
use crate::services::message_service::{GetSharedInput, SHARED_SCOPE};
use futures::TryStreamExt;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Pool, Row, Sqlite};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;
use url::Url;

pub const KNOWN_HOSTS: [&str; 9] = [
    "github.com",
    "youtube.com",
    "youtu.be",
    "vimeo.com",
    "spotify.com",
    "figma.com",
    "notion.so",
    "linear.app",
    "docs.google.com",
];
pub const MODE_ON: &str = "on";
pub const MODE_OFF: &str = "off";
pub const MODE_KNOWN_ONLY: &str = "known-only";

const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36";
const TIMEOUT_SECONDS: u64 = 6;
const MAX_PAGE_BYTES: usize = 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;
const FRESH_OK_SECONDS: i64 = 7 * 24 * 3600;
const FRESH_ERROR_SECONDS: i64 = 3600;
const STATUS_OK: &str = "ok";
const STATUS_ERROR: &str = "error";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LinkPreview {
    pub url: String,
    pub kind: String,
    pub site_name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub image: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub favicon_url: Option<String>,
    pub favicon: Option<String>,
    pub theme_color: Option<String>,
    pub author: Option<String>,
    pub duration: Option<u64>,
    pub state: Option<String>,
    pub repo: Option<String>,
    pub number: Option<u64>,
    pub file_name: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GithubTarget {
    pub api: String,
    pub kind: &'static str,
    pub repo: String,
    pub number: Option<u64>,
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS link_preview (
            url TEXT PRIMARY KEY,
            json TEXT NOT NULL,
            fetched_at TEXT NOT NULL,
            status TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub fn host(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    parsed.host_str().map(|h| h.trim_start_matches("www.").to_lowercase())
}

pub fn allows(mode: &str, url: &str) -> bool {
    let Some(host) = host(url) else { return false };
    match mode {
        MODE_OFF => false,
        MODE_KNOWN_ONLY => KNOWN_HOSTS.iter().any(|known| host == *known || host.ends_with(&format!(".{}", known))),
        _ => true,
    }
}

fn resolve(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with("data:") {
        return None;
    }
    Url::parse(base).ok()?.join(href).ok().map(String::from)
}

fn clip(text: &str, max: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max {
        return text;
    }
    format!("{}…", text.chars().take(max - 1).collect::<String>().trim_end())
}

fn present(value: Option<&String>) -> Option<String> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

pub fn iso_duration(text: &str) -> Option<u64> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let caps = rx(&RE, r"(?i)^P(?:(\d+)D)?T?(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?$").captures(text.trim())?;
    let part = |i: usize| caps.get(i).and_then(|m| m.as_str().parse::<u64>().ok()).unwrap_or(0);
    let total = part(1) * 86400 + part(2) * 3600 + part(3) * 60 + part(4);
    (total > 0).then_some(total)
}

pub fn file_name(url: &str, disposition: Option<&str>) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    if let Some(name) = disposition.and_then(|d| rx(&RE, r#"(?i)filename\*?=(?:UTF-8'')?"?([^";]+)"?"#).captures(d)).map(|c| c[1].to_string()) {
        return Some(urlencoding::decode(&name).map(|n| n.into_owned()).unwrap_or(name));
    }
    let parsed = Url::parse(url).ok()?;
    let last = parsed.path_segments()?.filter(|s| !s.is_empty()).last()?.to_string();
    Some(urlencoding::decode(&last).map(|n| n.into_owned()).unwrap_or(last))
}

pub fn parse_html(url: &str, html: &str) -> LinkPreview {
    static META: OnceLock<Regex> = OnceLock::new();
    static LINK: OnceLock<Regex> = OnceLock::new();
    static TITLE: OnceLock<Regex> = OnceLock::new();
    let mut meta: HashMap<String, String> = HashMap::new();
    for tag in rx(&META, r"(?is)<meta\b[^>]*>").find_iter(html) {
        let tag = tag.as_str();
        let Some(key) = attr(tag, "property").or_else(|| attr(tag, "name")).or_else(|| attr(tag, "itemprop")) else { continue };
        let Some(content) = attr(tag, "content") else { continue };
        meta.entry(key.trim().to_lowercase()).or_insert(content);
    }
    let pick = |keys: &[&str]| keys.iter().find_map(|k| present(meta.get(*k)));
    let title_tag = rx(&TITLE, r"(?is)<title\b[^>]*>(.*?)</title>").captures(html).map(|c| decode_entities(&c[1]));
    let mut icon = None;
    let mut touch_icon = None;
    for tag in rx(&LINK, r"(?is)<link\b[^>]*>").find_iter(html) {
        let tag = tag.as_str();
        let rel = attr(tag, "rel").unwrap_or_default().to_lowercase();
        let Some(href) = attr(tag, "href") else { continue };
        if rel.split_whitespace().any(|r| r == "icon") && icon.is_none() {
            icon = resolve(url, &href);
        } else if rel.contains("apple-touch-icon") && touch_icon.is_none() {
            touch_icon = resolve(url, &href);
        }
    }
    let og_type = pick(&["og:type"]).unwrap_or_default().to_lowercase();
    let kind = if og_type.starts_with("video") {
        "video"
    } else if og_type.starts_with("music") {
        "audio"
    } else {
        "article"
    };
    LinkPreview {
        url: url.to_string(),
        kind: kind.to_string(),
        site_name: pick(&["og:site_name", "application-name"]).or_else(|| host(url)),
        title: pick(&["og:title", "twitter:title"]).or(title_tag).map(|t| clip(&t, 200)).filter(|t| !t.is_empty()),
        description: pick(&["og:description", "twitter:description", "description"]).map(|d| clip(&d, 300)),
        image_url: pick(&["og:image:secure_url", "og:image", "og:image:url", "twitter:image", "twitter:image:src"]).and_then(|i| resolve(url, &i)),
        image_width: pick(&["og:image:width"]).and_then(|w| w.parse().ok()),
        image_height: pick(&["og:image:height"]).and_then(|h| h.parse().ok()),
        favicon_url: icon.or(touch_icon).or_else(|| resolve(url, "/favicon.ico")),
        theme_color: pick(&["theme-color"]),
        author: pick(&["author", "article:author", "twitter:creator"]),
        duration: pick(&["video:duration"]).and_then(|d| d.parse().ok()).or_else(|| pick(&["duration"]).and_then(|d| iso_duration(&d))),
        ..LinkPreview::default()
    }
}

pub fn youtube_id(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?.trim_start_matches("www.").trim_start_matches("m.").to_string();
    let id = match host.as_str() {
        "youtu.be" => parsed.path_segments()?.next().map(String::from),
        "youtube.com" | "music.youtube.com" => {
            let mut segments = parsed.path_segments()?;
            match segments.next() {
                Some("watch") => parsed.query_pairs().find(|(k, _)| k == "v").map(|(_, v)| v.into_owned()),
                Some("shorts") | Some("embed") | Some("live") => segments.next().map(String::from),
                _ => None,
            }
        }
        _ => None,
    }?;
    let valid = id.len() >= 6 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    valid.then_some(id)
}

pub fn youtube_duration(html: &str) -> Option<u64> {
    static RE: OnceLock<Regex> = OnceLock::new();
    rx(&RE, r#""lengthSeconds"\s*:\s*"(\d+)""#).captures(html).and_then(|c| c[1].parse().ok())
}

pub fn oembed_endpoint(url: &str) -> Option<String> {
    let host = host(url)?;
    let base = match host.as_str() {
        "youtube.com" | "m.youtube.com" | "youtu.be" | "music.youtube.com" => "https://www.youtube.com/oembed?format=json&url=",
        "vimeo.com" | "player.vimeo.com" => "https://vimeo.com/api/oembed.json?url=",
        "open.spotify.com" | "spotify.com" => "https://open.spotify.com/oembed?url=",
        "twitter.com" | "x.com" | "mobile.twitter.com" => "https://publish.twitter.com/oembed?omit_script=true&url=",
        _ => return None,
    };
    Some(format!("{}{}", base, urlencoding::encode(url)))
}

pub fn apply_oembed(preview: &mut LinkPreview, body: &Value) {
    static P: OnceLock<Regex> = OnceLock::new();
    let text = |key: &str| body[key].as_str().map(str::trim).filter(|v| !v.is_empty()).map(String::from);
    if let Some(title) = text("title") {
        preview.title = Some(clip(&title, 200));
    }
    if let Some(author) = text("author_name") {
        preview.author = Some(author);
    }
    if let Some(provider) = text("provider_name") {
        preview.site_name = Some(provider);
    }
    if let Some(thumbnail) = text("thumbnail_url") {
        preview.image_url = Some(thumbnail);
        preview.image_width = body["thumbnail_width"].as_u64().map(|w| w as u32);
        preview.image_height = body["thumbnail_height"].as_u64().map(|h| h as u32);
    }
    match body["type"].as_str() {
        Some("video") => preview.kind = "video".to_string(),
        Some("photo") => {
            preview.kind = "image".to_string();
            if let Some(url) = text("url") {
                preview.image_url = Some(url);
            }
        }
        Some("rich") if text("title").is_none() => {
            if let Some(html) = text("html") {
                preview.kind = "post".to_string();
                let quote = rx(&P, r"(?is)<p\b[^>]*>(.*?)</p>").captures(&html).map(|c| flat_text(&c[1])).unwrap_or_else(|| flat_text(&html));
                preview.description = Some(clip(&quote, 300)).filter(|d| !d.is_empty());
                preview.title = preview.author.clone();
            }
        }
        _ => {}
    }
    if let Some(duration) = body["duration"].as_u64() {
        preview.duration = Some(duration);
    }
}

pub fn github_target(url: &str) -> Option<GithubTarget> {
    if host(url)? != "github.com" {
        return None;
    }
    let parsed = Url::parse(url).ok()?;
    let segments: Vec<&str> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    let (owner, name) = (segments.first()?, segments.get(1)?);
    let repo = format!("{}/{}", owner, name.trim_end_matches(".git"));
    match (segments.get(2).copied(), segments.get(3).and_then(|n| n.parse::<u64>().ok())) {
        (None, _) => Some(GithubTarget { api: format!("https://api.github.com/repos/{}", repo), kind: "repo", repo, number: None }),
        (Some("pull"), Some(number)) => Some(GithubTarget { api: format!("https://api.github.com/repos/{}/pulls/{}", repo, number), kind: "pull", repo, number: Some(number) }),
        (Some("issues"), Some(number)) => Some(GithubTarget { api: format!("https://api.github.com/repos/{}/issues/{}", repo, number), kind: "issue", repo, number: Some(number) }),
        _ => None,
    }
}

pub fn apply_github(preview: &mut LinkPreview, target: &GithubTarget, body: &Value) {
    preview.kind = target.kind.to_string();
    preview.repo = Some(target.repo.clone());
    preview.number = target.number;
    preview.site_name = Some("GitHub".to_string());
    if target.kind == "repo" {
        preview.title = body["full_name"].as_str().map(String::from).or(preview.title.take());
        if let Some(description) = body["description"].as_str().filter(|d| !d.trim().is_empty()) {
            preview.description = Some(clip(description, 300));
        }
        preview.state = body["archived"].as_bool().filter(|a| *a).map(|_| "archived".to_string());
        return;
    }
    if let Some(title) = body["title"].as_str() {
        preview.title = Some(clip(title, 200));
    }
    if let Some(text) = body["body"].as_str().filter(|b| !b.trim().is_empty()) {
        preview.description = Some(clip(text, 300));
    }
    preview.author = body["user"]["login"].as_str().map(String::from).or(preview.author.take());
    let merged = body["merged"].as_bool().unwrap_or(false) || body["merged_at"].is_string();
    let draft = body["draft"].as_bool().unwrap_or(false);
    let state = body["state"].as_str().unwrap_or("open");
    preview.state = Some(
        match (target.kind, merged, draft, state, body["state_reason"].as_str()) {
            ("pull", true, _, _, _) => "merged",
            ("pull", false, true, "open", _) => "draft",
            ("issue", _, _, "closed", Some("not_planned")) => "not_planned",
            (_, _, _, "closed", _) => "closed",
            _ => "open",
        }
        .to_string(),
    );
}

pub fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    if let Some(mime) = sniff_mime(bytes) {
        return Some(extension_for(mime));
    }
    if bytes.starts_with(&[0, 0, 1, 0]) {
        return Some("ico");
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_lowercase();
    head.contains("<svg").then_some("svg")
}

async fn read_capped(mut response: reqwest::Response, cap: usize) -> Result<(Vec<u8>, bool), String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| format!("Could not read the page: {}", e))? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > cap {
            bytes.resize(cap, 0);
            return Ok((bytes, true));
        }
    }
    Ok((bytes, false))
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECONDS))
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()
        .map_err(|e| format!("Could not start the preview client: {}", e))
}

async fn get_json(client: &reqwest::Client, url: &str) -> Option<Value> {
    let response = client.get(url).header("Accept", "application/json, application/vnd.github+json").send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let (bytes, cut) = read_capped(response, MAX_PAGE_BYTES).await.ok()?;
    if cut {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

pub fn local_name(url: &str, ext: &str) -> String {
    format!("media/links/{}.{}", crate::utils::short_hash(url.as_bytes()), ext)
}

fn save(app_data: &Path, url: &str, bytes: &[u8]) -> Option<(String, Option<(u32, u32)>)> {
    let ext = image_extension(bytes)?;
    let relative = local_name(url, ext);
    let path = app_data.join(&relative);
    if !path.exists() {
        std::fs::create_dir_all(path.parent()?).ok()?;
        std::fs::write(&path, bytes).ok()?;
    }
    Some((relative, dimensions(bytes)))
}

async fn download(client: &reqwest::Client, app_data: &Path, url: &str) -> Option<(String, Option<(u32, u32)>)> {
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let (bytes, cut) = read_capped(response, MAX_IMAGE_BYTES).await.ok()?;
    if cut || bytes.is_empty() {
        return None;
    }
    save(app_data, url, &bytes)
}

async fn fetch(app_data: &Path, url: &str) -> Result<LinkPreview, String> {
    let client = client()?;
    let github = github_target(url);
    let oembed = oembed_endpoint(url);
    let page = client.get(url).header("Accept", "text/html,application/xhtml+xml,*/*;q=0.8").send();
    let api = async {
        match &github {
            Some(target) => get_json(&client, &target.api).await,
            None => None,
        }
    };
    let embed = async {
        match &oembed {
            Some(endpoint) => get_json(&client, endpoint).await,
            None => None,
        }
    };
    let (page, api, embed) = tokio::join!(page, api, embed);
    let response = page.map_err(|e| if e.is_timeout() { "The page took too long to answer".to_string() } else { format!("Could not open the link: {}", e) })?;
    if !response.status().is_success() && api.is_none() && embed.is_none() {
        return Err(format!("The page answered {}", response.status()));
    }
    let final_url = response.url().to_string();
    let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).map(String::from);
    let content_type = header("content-type").unwrap_or_default().to_lowercase();
    let disposition = header("content-disposition");
    let length = response.content_length();
    let mut preview = if content_type.starts_with("application/pdf") {
        LinkPreview { url: url.to_string(), kind: "pdf".to_string(), site_name: host(&final_url), file_name: file_name(&final_url, disposition.as_deref()), size: length, ..LinkPreview::default() }
    } else if content_type.starts_with("image/") {
        let (bytes, cut) = read_capped(response, MAX_IMAGE_BYTES).await?;
        let size = dimensions(&bytes);
        let local = if cut { None } else { save(app_data, url, &bytes).map(|s| s.0) };
        LinkPreview {
            url: url.to_string(),
            kind: "image".to_string(),
            site_name: host(&final_url),
            file_name: file_name(&final_url, disposition.as_deref()),
            image_url: Some(final_url.clone()),
            image: local,
            image_width: size.map(|s| s.0),
            image_height: size.map(|s| s.1),
            size: length.or(Some(bytes.len() as u64).filter(|_| !cut)),
            ..LinkPreview::default()
        }
    } else if content_type.starts_with("video/") || content_type.starts_with("audio/") {
        LinkPreview {
            url: url.to_string(),
            kind: content_type.split('/').next().unwrap_or("video").to_string(),
            site_name: host(&final_url),
            file_name: file_name(&final_url, disposition.as_deref()),
            size: length,
            ..LinkPreview::default()
        }
    } else {
        let (bytes, _) = read_capped(response, MAX_PAGE_BYTES).await?;
        let html = String::from_utf8_lossy(&bytes);
        let mut preview = parse_html(&final_url, &html);
        preview.url = url.to_string();
        if youtube_id(url).is_some() {
            preview.duration = preview.duration.or_else(|| youtube_duration(&html));
        }
        preview
    };
    if let Some(body) = embed {
        apply_oembed(&mut preview, &body);
    }
    if let Some(id) = youtube_id(url) {
        preview.kind = "video".to_string();
        if preview.image_url.is_none() {
            preview.image_url = Some(format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", id));
        }
    }
    if let (Some(target), Some(body)) = (&github, &api) {
        apply_github(&mut preview, target, body);
    }
    if preview.image.is_none() {
        if let Some(image_url) = preview.image_url.clone() {
            if let Some((path, size)) = download(&client, app_data, &image_url).await {
                preview.image = Some(path);
                if let Some((w, h)) = size {
                    preview.image_width = Some(w);
                    preview.image_height = Some(h);
                }
            }
        }
    }
    let favicon_url = preview.favicon_url.clone().or_else(|| resolve(&final_url, "/favicon.ico"));
    if let Some(favicon_url) = favicon_url {
        preview.favicon = download(&client, app_data, &favicon_url).await.map(|s| s.0);
        preview.favicon_url = Some(favicon_url);
    }
    Ok(preview)
}

pub async fn get_cached(pool: &Pool<Sqlite>, url: &str, now: DateTime<Utc>) -> Result<Option<Result<LinkPreview, String>>, String> {
    let row = sqlx::query("SELECT json, fetched_at, status FROM link_preview WHERE url = ?1")
        .bind(url)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to read the link preview cache: {}", e))?;
    let Some(row) = row else { return Ok(None) };
    let status: String = row.get("status");
    let fetched_at: String = row.get("fetched_at");
    let json: String = row.get("json");
    let age = DateTime::parse_from_rfc3339(&fetched_at).map(|t| (now - t.with_timezone(&Utc)).num_seconds()).unwrap_or(i64::MAX);
    let fresh = if status == STATUS_OK { FRESH_OK_SECONDS } else { FRESH_ERROR_SECONDS };
    if age > fresh {
        return Ok(None);
    }
    Ok(Some(if status == STATUS_OK {
        serde_json::from_str(&json).map_err(|e| format!("The cached preview is unreadable: {}", e))
    } else {
        Err(json)
    }))
}

pub async fn put_cached(pool: &Pool<Sqlite>, url: &str, result: &Result<LinkPreview, String>, now: DateTime<Utc>) -> Result<(), String> {
    let (json, status) = match result {
        Ok(preview) => (serde_json::to_string(preview).map_err(|e| format!("Failed to encode the preview: {}", e))?, STATUS_OK),
        Err(message) => (message.clone(), STATUS_ERROR),
    };
    sqlx::query(
        "INSERT INTO link_preview (url, json, fetched_at, status) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(url) DO UPDATE SET json = excluded.json, fetched_at = excluded.fetched_at, status = excluded.status",
    )
    .bind(url)
    .bind(json)
    .bind(now.to_rfc3339())
    .bind(status)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to cache the link preview: {}", e))?;
    Ok(())
}

pub async fn preview(pool: &Pool<Sqlite>, app_data: &Path, url: &str) -> Result<Option<LinkPreview>, String> {
    let mode = AppSetting::get_or(pool, app_setting::LINKS_PREVIEWS, MODE_ON).await?;
    let url = clean_url(url.trim());
    if !allows(&mode, &url) {
        return Ok(None);
    }
    if let Some(cached) = get_cached(pool, &url, Utc::now()).await? {
        return cached.map(Some);
    }
    let result = fetch(app_data, &url).await;
    put_cached(pool, &url, &result, Utc::now()).await?;
    result.map(Some)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderLink {
    pub provider_type: String,
    pub conversation_external_id: Option<String>,
    pub message_external_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkTarget {
    pub conversation_id: String,
    pub message_id: Option<String>,
}

fn slack_ts(raw: &str) -> Option<String> {
    let digits = raw.strip_prefix('p')?;
    if digits.len() <= 6 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (secs, micros) = digits.split_at(digits.len() - 6);
    Some(format!("{}.{}", secs, micros))
}

pub fn parse_provider_link(url: &str) -> Option<ProviderLink> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    let segments: Vec<String> = parsed.path_segments().map(|s| s.map(|p| urlencoding::decode(p).map(|d| d.into_owned()).unwrap_or_else(|_| p.to_string())).collect()).unwrap_or_default();
    let query: HashMap<String, String> = parsed.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    if host.ends_with(".slack.com") && segments.first().map(String::as_str) == Some("archives") {
        let channel = segments.get(1)?.clone();
        let message = segments.get(2).and_then(|p| slack_ts(p));
        return Some(ProviderLink { provider_type: "slack".into(), conversation_external_id: Some(channel), message_external_id: message });
    }
    if host == "teams.microsoft.com" && segments.first().map(String::as_str) == Some("l") {
        let kind = segments.get(1)?.as_str();
        let id = segments.get(2)?.clone();
        let conversation = match query.get("groupId") {
            Some(team) if kind != "chat" => format!("channel:{}:{}", team, id),
            _ => id,
        };
        let message = (kind == "message").then(|| segments.get(3).cloned()).flatten();
        return Some(ProviderLink { provider_type: "microsoft".into(), conversation_external_id: Some(conversation), message_external_id: message });
    }
    if host == "mail.google.com" {
        let thread = parsed.fragment()?.rsplit('/').next()?.to_string();
        if thread.is_empty() || !thread.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        return Some(ProviderLink { provider_type: "google".into(), conversation_external_id: Some(format!("gmail:{}", thread)), message_external_id: None });
    }
    None
}

pub async fn get_link_target(pool: &Pool<Sqlite>, url: &str) -> Result<Option<LinkTarget>, String> {
    let by_link: Option<(Option<String>, String)> = sqlx::query_as(
        "SELECT m.conversation_id, m.id FROM message m JOIN provider p ON p.id = m.provider_id
         WHERE p.profile_id = (SELECT id FROM profile WHERE active = 1 LIMIT 1) AND m.deleted_datetime IS NULL
           AND CASE WHEN json_valid(m.metadata) THEN json_extract(m.metadata, '$.web_url') END = ?1 LIMIT 1",
    )
    .bind(url)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to look up the link: {}", e))?;
    if let Some((Some(conversation_id), message_id)) = by_link {
        return Ok(Some(LinkTarget { conversation_id, message_id: Some(message_id) }));
    }
    let Some(link) = parse_provider_link(url) else { return Ok(None) };
    let Some(conversation) = link.conversation_external_id.as_deref() else { return Ok(None) };
    let found: Option<(String, String)> = sqlx::query_as(
        "SELECT cp.conversation_id, cp.provider_id FROM conversation_provider cp JOIN provider p ON p.id = cp.provider_id
         WHERE p.profile_id = (SELECT id FROM profile WHERE active = 1 LIMIT 1) AND (p.provider_type = ?1 OR p.provider_type = 'mock') AND cp.external_id = ?2 LIMIT 1",
    )
    .bind(&link.provider_type)
    .bind(conversation)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to look up the linked conversation: {}", e))?;
    let Some((conversation_id, provider_id)) = found else { return Ok(None) };
    let message_id = match link.message_external_id.as_deref() {
        Some(external) => sqlx::query_scalar::<_, String>("SELECT id FROM message WHERE provider_id = ?1 AND external_id = ?2 LIMIT 1")
            .bind(&provider_id)
            .bind(external)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to look up the linked message: {}", e))?,
        None => None,
    };
    Ok(Some(LinkTarget { conversation_id, message_id }))
}

pub async fn get_message_link(pool: &Pool<Sqlite>, message_id: &str) -> Result<Option<String>, String> {
    let metadata: Option<String> = sqlx::query_scalar("SELECT metadata FROM message WHERE id = ?1")
        .bind(message_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to load the message: {}", e))?
        .flatten();
    let mut parsed: Value = metadata.as_deref().and_then(|m| serde_json::from_str(m).ok()).unwrap_or_else(|| serde_json::json!({}));
    if let Some(link) = parsed["web_url"].as_str().filter(|l| !l.is_empty()) {
        return Ok(Some(link.to_string()));
    }
    let Ok((route, external_id)) = crate::services::outbound_service::route_for_message(pool, message_id).await else { return Ok(None) };
    let mut provider = route.provider;
    let client = crate::services::connection_service::provider_impl(pool, &provider.provider_type, provider.config.as_deref()).await?;
    let auth = crate::services::connection_service::ensure_fresh(pool, &mut provider, client.clone()).await?;
    let Some(link) = client.get_message_link(&auth, &route.conversation_external_id, &external_id).await? else { return Ok(None) };
    if !parsed.is_object() {
        parsed = serde_json::json!({});
    }
    parsed["web_url"] = Value::String(link.clone());
    sqlx::query("UPDATE message SET metadata = ?1 WHERE id = ?2")
        .bind(parsed.to_string())
        .bind(message_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to keep the link: {}", e))?;
    Ok(Some(link))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SharedLink {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    pub site_name: Option<String>,
    pub domain: Option<String>,
    pub favicon: Option<String>,
    pub message_id: String,
    pub conversation_id: String,
    pub conversation_name: String,
    pub sender: String,
    pub at: String,
}

pub async fn get_shared_links(pool: &Pool<Sqlite>, profile_id: &str, input: &GetSharedInput) -> Result<Vec<SharedLink>, String> {
    let limit = input.limit() as usize;
    let sql = format!(
        "SELECT m.id AS message_id, m.conversation_id, COALESCE(c.name, '') AS conversation_name, COALESCE(m.sender_name, m.sender_email, '') AS sender, m.timestamp AS at, m.content, m.content_type
         FROM message m
         JOIN provider p ON p.id = m.provider_id
         LEFT JOIN conversation c ON c.id = m.conversation_id
         WHERE p.profile_id = ?2 AND {scope} AND m.deleted_datetime IS NULL AND (m.content LIKE '%http%' OR m.content LIKE '%www.%')
         ORDER BY m.timestamp DESC",
        scope = SHARED_SCOPE
    );
    let mut rows = sqlx::query(&sql).bind(input.ids()).bind(profile_id).bind(&input.lead_conversation_id).fetch(pool);
    let mut out: Vec<SharedLink> = Vec::new();
    while out.len() < limit {
        let Some(row) = rows.try_next().await.map_err(|e| format!("Failed to load the shared links: {}", e))? else { break };
        let content: String = row.get::<Option<String>, _>("content").unwrap_or_default();
        let html = matches!(row.get::<Option<String>, _>("content_type").as_deref(), Some("html") | Some("text/html"));
        let text = if html { crate::content::html::visible_text(&content) } else { content.clone() };
        let message_id: String = row.get("message_id");
        for link in crate::content::links::extract(&text, html.then_some(content.as_str())) {
            if link.kind != LinkKind::Web || !link.url.starts_with("http") {
                continue;
            }
            let url = clean_url(&link.url);
            if out.len() >= limit || out.iter().any(|l| l.url == url) {
                continue;
            }
            out.push(SharedLink {
                id: format!("{}:{}", message_id, url),
                title: link.label.filter(|l| !l.trim().is_empty() && l.trim() != link.url && l.trim() != url),
                site_name: None,
                domain: host(&url),
                favicon: None,
                message_id: message_id.clone(),
                conversation_id: row.get::<Option<String>, _>("conversation_id").unwrap_or_default(),
                conversation_name: row.get("conversation_name"),
                sender: row.get("sender"),
                at: row.get("at"),
                url,
            });
        }
    }
    drop(rows);
    let urls = serde_json::to_string(&out.iter().map(|l| l.url.as_str()).collect::<Vec<_>>()).map_err(|e| format!("Failed to encode the links: {}", e))?;
    let cached: Vec<(String, String)> = sqlx::query_as("SELECT url, json FROM link_preview WHERE status = ?1 AND url IN (SELECT value FROM json_each(?2))")
        .bind(STATUS_OK)
        .bind(urls)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to read the link preview cache: {}", e))?;
    let previews: HashMap<String, LinkPreview> = cached.into_iter().filter_map(|(url, json)| serde_json::from_str(&json).ok().map(|p| (url, p))).collect();
    for link in &mut out {
        let Some(preview) = previews.get(&link.url) else { continue };
        link.title = preview.title.clone().filter(|t| !t.trim().is_empty()).or(link.title.take());
        link.site_name = preview.site_name.clone();
        link.favicon = preview.favicon.clone();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::fixture;
    use crate::test_helpers::test_pool;

    #[test]
    fn modes_gate_hosts() {
        assert!(allows(MODE_ON, "https://example.com/a"));
        assert!(!allows(MODE_OFF, "https://github.com/a/b"));
        assert!(allows(MODE_KNOWN_ONLY, "https://www.github.com/a/b"));
        assert!(allows(MODE_KNOWN_ONLY, "https://open.spotify.com/track/1"));
        assert!(!allows(MODE_KNOWN_ONLY, "https://example.com/a"));
        assert!(!allows(MODE_KNOWN_ONLY, "https://notgithub.com/a"));
        assert!(!allows(MODE_ON, "ftp://example.com/a"));
        assert!(!allows(MODE_ON, "not a url"));
    }

    #[test]
    fn parses_open_graph_twitter_and_icons() {
        let preview = parse_html("https://www.example.com/news/story", &fixture("link-article.html"));
        assert_eq!(preview.kind, "article");
        assert_eq!(preview.site_name.as_deref(), Some("Example News"));
        assert_eq!(preview.title.as_deref(), Some("Fjords & glaciers: a field guide"));
        assert_eq!(preview.description.as_deref(), Some("Everything you need to know before you go north."));
        assert_eq!(preview.image_url.as_deref(), Some("https://www.example.com/img/fjord.jpg"));
        assert_eq!((preview.image_width, preview.image_height), (Some(1200), Some(630)));
        assert_eq!(preview.favicon_url.as_deref(), Some("https://www.example.com/static/icon-32.png"));
        assert_eq!(preview.theme_color.as_deref(), Some("#0b5fff"));
        assert_eq!(preview.author.as_deref(), Some("Kari Nordmann"));
    }

    #[test]
    fn falls_back_to_title_description_and_favicon_ico() {
        let preview = parse_html("https://plain.example.org/page", &fixture("link-plain.html"));
        assert_eq!(preview.title.as_deref(), Some("Plain page"));
        assert_eq!(preview.description.as_deref(), Some("A page with only the basics."));
        assert_eq!(preview.site_name.as_deref(), Some("plain.example.org"));
        assert_eq!(preview.favicon_url.as_deref(), Some("https://plain.example.org/favicon.ico"));
        assert_eq!(preview.image_url, None);
    }

    #[test]
    fn reads_youtube_ids_duration_and_oembed() {
        assert_eq!(youtube_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1").as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(youtube_id("https://youtu.be/dQw4w9WgXcQ").as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(youtube_id("https://youtube.com/shorts/abcDEF_123").as_deref(), Some("abcDEF_123"));
        assert_eq!(youtube_id("https://youtube.com/@channel"), None);
        let html = fixture("link-youtube.html");
        let mut preview = parse_html("https://www.youtube.com/watch?v=dQw4w9WgXcQ", &html);
        assert_eq!(preview.kind, "video");
        assert_eq!(preview.duration, Some(213));
        assert_eq!(youtube_duration(&html), Some(212));
        let body: Value = serde_json::from_str(&fixture("link-oembed-youtube.json")).unwrap();
        apply_oembed(&mut preview, &body);
        assert_eq!(preview.title.as_deref(), Some("Never Gonna Give You Up"));
        assert_eq!(preview.author.as_deref(), Some("Rick Astley"));
        assert_eq!(preview.site_name.as_deref(), Some("YouTube"));
        assert_eq!(preview.image_url.as_deref(), Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg"));
        assert_eq!(iso_duration("PT1H2M3S"), Some(3723));
        assert_eq!(iso_duration("nonsense"), None);
    }

    #[test]
    fn oembed_endpoints_cover_known_providers_and_x_posts() {
        assert!(oembed_endpoint("https://vimeo.com/76979871").unwrap().starts_with("https://vimeo.com/api/oembed.json?url=https%3A%2F%2Fvimeo.com"));
        assert!(oembed_endpoint("https://open.spotify.com/track/1").unwrap().starts_with("https://open.spotify.com/oembed"));
        assert!(oembed_endpoint("https://x.com/jack/status/20").unwrap().starts_with("https://publish.twitter.com/oembed"));
        assert_eq!(oembed_endpoint("https://example.com/"), None);
        let mut preview = LinkPreview { url: "https://x.com/jack/status/20".into(), kind: "article".into(), ..LinkPreview::default() };
        apply_oembed(&mut preview, &serde_json::from_str(&fixture("link-oembed-x.json")).unwrap());
        assert_eq!(preview.kind, "post");
        assert_eq!(preview.title.as_deref(), Some("jack"));
        assert_eq!(preview.description.as_deref(), Some("just setting up my twttr"));
    }

    #[test]
    fn github_targets_and_states() {
        assert_eq!(github_target("https://github.com/tauri-apps/tauri").map(|t| (t.kind, t.api)), Some(("repo", "https://api.github.com/repos/tauri-apps/tauri".to_string())));
        assert_eq!(github_target("https://github.com/tauri-apps/tauri/pull/42/files").map(|t| (t.kind, t.number)), Some(("pull", Some(42))));
        assert_eq!(github_target("https://github.com/tauri-apps/tauri/issues/7").map(|t| t.kind), Some("issue"));
        assert_eq!(github_target("https://github.com/tauri-apps/tauri/blob/dev/README.md"), None);
        assert_eq!(github_target("https://gitlab.com/a/b"), None);
        let target = github_target("https://github.com/tauri-apps/tauri/pull/42").unwrap();
        let mut preview = LinkPreview::default();
        apply_github(&mut preview, &target, &serde_json::from_str(&fixture("link-github-pull.json")).unwrap());
        assert_eq!(preview.kind, "pull");
        assert_eq!(preview.title.as_deref(), Some("Add link previews"));
        assert_eq!(preview.state.as_deref(), Some("merged"));
        assert_eq!(preview.repo.as_deref(), Some("tauri-apps/tauri"));
        assert_eq!(preview.author.as_deref(), Some("octocat"));
        let target = github_target("https://github.com/tauri-apps/tauri").unwrap();
        let mut preview = LinkPreview::default();
        apply_github(&mut preview, &target, &serde_json::from_str(&fixture("link-github-repo.json")).unwrap());
        assert_eq!(preview.title.as_deref(), Some("tauri-apps/tauri"));
        assert_eq!(preview.description.as_deref(), Some("Build smaller, faster, and more secure desktop applications."));
        assert_eq!(preview.state, None);
        let issue = github_target("https://github.com/a/b/issues/1").unwrap();
        let mut preview = LinkPreview::default();
        apply_github(&mut preview, &issue, &serde_json::json!({"title": "Bug", "state": "closed", "state_reason": "not_planned"}));
        assert_eq!(preview.state.as_deref(), Some("not_planned"));
    }

    #[test]
    fn names_files_and_images() {
        assert_eq!(file_name("https://x.org/docs/Annual%20Report.pdf?dl=1", None).as_deref(), Some("Annual Report.pdf"));
        assert_eq!(file_name("https://x.org/download", Some("attachment; filename=\"plan.pdf\"")).as_deref(), Some("plan.pdf"));
        assert_eq!(image_extension(b"GIF89a\x01\x00\x01\x00"), Some("gif"));
        assert_eq!(image_extension(&[0, 0, 1, 0, 1, 0]), Some("ico"));
        assert_eq!(image_extension(b"<?xml version=\"1.0\"?><svg xmlns=\"\"></svg>"), Some("svg"));
        assert_eq!(image_extension(b"<html>"), None);
        let a = local_name("https://a/x.png", "png");
        assert!(a.starts_with("media/links/") && a.ends_with(".png"));
        assert_eq!(a, local_name("https://a/x.png", "png"));
        assert_ne!(a, local_name("https://a/y.png", "png"));
    }

    #[tokio::test]
    async fn cache_keeps_previews_for_a_week_and_errors_for_an_hour() {
        let pool = test_pool().await;
        let now = Utc::now();
        let preview = LinkPreview { url: "https://a.com".into(), kind: "article".into(), title: Some("A".into()), ..LinkPreview::default() };
        put_cached(&pool, "https://a.com", &Ok(preview.clone()), now).await.unwrap();
        assert_eq!(get_cached(&pool, "https://a.com", now + chrono::Duration::days(6)).await.unwrap(), Some(Ok(preview)));
        assert_eq!(get_cached(&pool, "https://a.com", now + chrono::Duration::days(8)).await.unwrap(), None);
        put_cached(&pool, "https://b.com", &Err("The page answered 404".into()), now).await.unwrap();
        assert_eq!(get_cached(&pool, "https://b.com", now).await.unwrap(), Some(Err("The page answered 404".to_string())));
        assert_eq!(get_cached(&pool, "https://b.com", now + chrono::Duration::hours(2)).await.unwrap(), None);
        assert_eq!(get_cached(&pool, "https://c.com", now).await.unwrap(), None);
    }

    #[tokio::test]
    async fn previews_off_or_unknown_hosts_return_nothing_without_fetching() {
        let pool = test_pool().await;
        let dir = std::env::temp_dir();
        AppSetting::put(&pool, app_setting::LINKS_PREVIEWS, MODE_OFF).await.unwrap();
        assert_eq!(preview(&pool, &dir, "https://github.com/a/b").await.unwrap(), None);
        AppSetting::put(&pool, app_setting::LINKS_PREVIEWS, MODE_KNOWN_ONLY).await.unwrap();
        assert_eq!(preview(&pool, &dir, "https://example.com/").await.unwrap(), None);
        let cached = LinkPreview { url: "https://github.com/a/b".into(), kind: "repo".into(), ..LinkPreview::default() };
        put_cached(&pool, "https://github.com/a/b", &Ok(cached.clone()), Utc::now()).await.unwrap();
        assert_eq!(preview(&pool, &dir, "https://github.com/a/b?utm_source=x").await.unwrap(), Some(cached));
    }

    #[test]
    fn provider_links_parse_into_conversations_and_messages() {
        assert_eq!(parse_provider_link("https://northwind.slack.com/archives/C0DEV/p1728470000000100?thread_ts=1728469000.000200"), Some(ProviderLink { provider_type: "slack".into(), conversation_external_id: Some("C0DEV".into()), message_external_id: Some("1728470000.000100".into()) }));
        assert_eq!(parse_provider_link("https://northwind.slack.com/archives/C0DEV").unwrap().message_external_id, None);
        let chat = parse_provider_link("https://teams.microsoft.com/l/message/19%3Aabc%40thread.v2/1700?context=%7B%22contextType%22%3A%22chat%22%7D").unwrap();
        assert_eq!((chat.conversation_external_id.as_deref(), chat.message_external_id.as_deref()), (Some("19:abc@thread.v2"), Some("1700")));
        let channel = parse_provider_link("https://teams.microsoft.com/l/message/19%3Adev%40thread.tacv2/1701?groupId=team-1&tenantId=t").unwrap();
        assert_eq!(channel.conversation_external_id.as_deref(), Some("channel:team-1:19:dev@thread.tacv2"));
        assert_eq!(parse_provider_link("https://teams.microsoft.com/l/channel/19%3Adev%40thread.tacv2/General?groupId=team-1").unwrap().conversation_external_id.as_deref(), Some("channel:team-1:19:dev@thread.tacv2"));
        assert_eq!(parse_provider_link("https://mail.google.com/mail/?authuser=milo%40sundial.example#all/18c2f0a1b2c3d4e5").unwrap().conversation_external_id.as_deref(), Some("gmail:18c2f0a1b2c3d4e5"));
        assert_eq!(parse_provider_link("https://example.com/x"), None);
        assert_eq!(parse_provider_link("https://mail.google.com/mail/u/0/#inbox/FMfcgzQZ"), None);
    }

    #[tokio::test]
    async fn links_resolve_to_synced_conversations_and_messages_and_cached_web_urls() {
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        let slack = crate::test_helpers::test_provider(&pool, &profile, "slack").await;
        let conversation = crate::test_helpers::test_conversation(&pool, "dev", "channel").await;
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id, is_primary, created_at, updated_at) VALUES ('cp1', ?1, ?2, 'C0DEV', 1, '2026-10-15', '2026-10-15')").bind(&conversation).bind(&slack).execute(&pool).await.unwrap();
        let message = crate::test_helpers::test_message(&pool, &conversation, &slack, "Hei").await;
        sqlx::query("UPDATE message SET external_id = '1728470000.000100', metadata = '{\"web_url\":\"https://x.no/m\"}' WHERE id = ?1").bind(&message).execute(&pool).await.unwrap();
        let target = get_link_target(&pool, "https://northwind.slack.com/archives/C0DEV/p1728470000000100").await.unwrap().unwrap();
        assert_eq!((target.conversation_id.as_str(), target.message_id.as_deref()), (conversation.as_str(), Some(message.as_str())));
        assert_eq!(get_link_target(&pool, "https://x.no/m").await.unwrap().unwrap().message_id.as_deref(), Some(message.as_str()));
        assert_eq!(get_link_target(&pool, "https://northwind.slack.com/archives/C0OTHER").await.unwrap(), None);
        assert_eq!(get_message_link(&pool, &message).await.unwrap().as_deref(), Some("https://x.no/m"));
    }

    #[tokio::test]
    async fn shared_links_are_unique_newest_first_with_cached_titles_across_a_merge_group() {
        let pool = test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        let provider = crate::test_helpers::test_provider(&pool, &profile, "mock").await;
        let a = crate::test_helpers::test_conversation(&pool, "a", "direct_message").await;
        let b = crate::test_helpers::test_conversation(&pool, "b", "direct_message").await;
        let c = crate::test_helpers::test_conversation(&pool, "c", "direct_message").await;
        crate::models::MergeGroup::post(&pool, &profile, &crate::models::MergeGroupInput { lead_conversation_id: a.clone(), conversation_ids: vec![b.clone()], group_lead_ids: vec![] }).await.unwrap();
        let first = crate::test_helpers::test_message(&pool, &a, &provider, "See https://github.com/a/b?utm_source=x and mailto:kari@x.no").await;
        let second = crate::test_helpers::test_message(&pool, &b, &provider, "<p>Read <a href=\"https://www.example.com/story\">the story</a> or https://github.com/a/b</p>").await;
        let other = crate::test_helpers::test_message(&pool, &c, &provider, "https://elsewhere.org").await;
        crate::test_helpers::test_message(&pool, &a, &provider, "no links here").await;
        for (id, at) in [(&first, "2026-10-01T10:00:00Z"), (&second, "2026-10-02T10:00:00Z"), (&other, "2026-10-03T10:00:00Z")] {
            sqlx::query("UPDATE message SET timestamp = ?1 WHERE id = ?2").bind(at).bind(id).execute(&pool).await.unwrap();
        }
        sqlx::query("UPDATE message SET content_type = 'html' WHERE id = ?1").bind(&second).execute(&pool).await.unwrap();
        let cached = LinkPreview { url: "https://github.com/a/b".into(), kind: "repo".into(), title: Some("a/b".into()), site_name: Some("GitHub".into()), favicon: Some("media/links/gh.png".into()), ..LinkPreview::default() };
        put_cached(&pool, "https://github.com/a/b", &Ok(cached), Utc::now()).await.unwrap();

        let links = get_shared_links(&pool, &profile, &GetSharedInput { lead_conversation_id: Some(a.clone()), ..Default::default() }).await.unwrap();
        assert_eq!(links.iter().map(|l| l.url.as_str()).collect::<Vec<_>>(), vec!["https://www.example.com/story", "https://github.com/a/b"]);
        assert_eq!((links[0].title.as_deref(), links[0].domain.as_deref(), links[0].conversation_id.as_str()), (Some("the story"), Some("example.com"), b.as_str()));
        assert_eq!((links[1].title.as_deref(), links[1].site_name.as_deref(), links[1].favicon.as_deref()), (Some("a/b"), Some("GitHub"), Some("media/links/gh.png")));
        assert_eq!((links[1].message_id.as_str(), links[1].conversation_name.as_str()), (second.as_str(), "b"));

        let listed = get_shared_links(&pool, &profile, &GetSharedInput { conversation_ids: vec![a.clone(), c.clone()], ..Default::default() }).await.unwrap();
        assert_eq!(listed.iter().map(|l| (l.url.as_str(), l.message_id.as_str())).collect::<Vec<_>>(), vec![("https://elsewhere.org", other.as_str()), ("https://github.com/a/b", first.as_str())]);
        assert_eq!(get_shared_links(&pool, &profile, &GetSharedInput { lead_conversation_id: Some(a.clone()), limit: Some(1), ..Default::default() }).await.unwrap().len(), 1);
        assert!(get_shared_links(&pool, "someone-else", &GetSharedInput { conversation_ids: vec![a], ..Default::default() }).await.unwrap().is_empty());
    }
}
