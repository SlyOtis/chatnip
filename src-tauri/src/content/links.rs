use super::html::{attr, flat_text};
use super::rx;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    Web,
    Mailto,
    Tel,
    Unsubscribe,
    Preferences,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub url: String,
    pub label: Option<String>,
    pub kind: LinkKind,
}

fn trim_url(url: &str) -> String {
    let mut url = url.trim_end_matches(['.', ',', ';', ':', '!', '?', '\'', '"']).to_string();
    while url.ends_with(')') && url.matches('(').count() < url.matches(')').count() {
        url.pop();
    }
    url
}

pub fn clean_url(url: &str) -> String {
    match url.split_once('?') {
        Some((base, query)) => {
            let kept: Vec<&str> = query.split('&').filter(|p| !p.starts_with("utm_") && !p.starts_with("mc_") && !p.is_empty()).collect();
            if kept.is_empty() {
                base.to_string()
            } else {
                format!("{}?{}", base, kept.join("&"))
            }
        }
        None => url.to_string(),
    }
}

fn kind_of(url: &str, label: Option<&str>) -> LinkKind {
    let probe = format!("{} {}", url, label.unwrap_or("")).to_lowercase();
    if url.starts_with("mailto:") && !probe.contains("unsub") {
        return LinkKind::Mailto;
    }
    if url.starts_with("tel:") {
        return LinkKind::Tel;
    }
    if ["unsubscribe", "unsub", "avmeld", "opt-out", "optout", "meld deg av"].iter().any(|w| probe.contains(w)) {
        return LinkKind::Unsubscribe;
    }
    if ["preferences", "innstillinger", "manage-subscription", "email-settings"].iter().any(|w| probe.contains(w)) {
        return LinkKind::Preferences;
    }
    LinkKind::Web
}

pub fn extract(text: &str, html: Option<&str>) -> Vec<Link> {
    static ANCHOR: OnceLock<Regex> = OnceLock::new();
    static URL: OnceLock<Regex> = OnceLock::new();
    static SLACK: OnceLock<Regex> = OnceLock::new();
    let mut out: Vec<Link> = Vec::new();
    let mut push = |url: String, label: Option<String>| {
        let url = trim_url(&url);
        if url.is_empty() || out.iter().any(|l| l.url == url) {
            return;
        }
        let kind = kind_of(&url, label.as_deref());
        out.push(Link { url, label, kind });
    };
    if let Some(html) = html {
        for c in rx(&ANCHOR, r"(?is)<a\b([^>]*)>(.*?)</a>").captures_iter(html) {
            if let Some(href) = attr(&format!("<a {}>", &c[1]), "href") {
                let label = flat_text(&c[2]);
                push(href, Some(label).filter(|l| !l.is_empty()));
            }
        }
    }
    for c in rx(&SLACK, r"<((?:https?|mailto):[^|>\s]+)(?:\|([^>]+))?>").captures_iter(text) {
        push(c[1].to_string(), c.get(2).map(|m| m.as_str().to_string()));
    }
    for m in rx(&URL, r#"(?i)\b(?:https?://|www\.)[^\s<>"'\]|]+"#).find_iter(text) {
        let raw = m.as_str();
        let url = if raw.to_lowercase().starts_with("www.") { format!("https://{}", raw) } else { raw.to_string() };
        push(url, None);
    }
    out
}

pub fn preview_candidates(links: &[Link], limit: usize) -> Vec<String> {
    links
        .iter()
        .filter(|l| l.kind == LinkKind::Web && l.url.starts_with("http"))
        .map(|l| clean_url(&l.url))
        .fold(Vec::new(), |mut acc, url| {
            if !acc.contains(&url) {
                acc.push(url);
            }
            acc
        })
        .into_iter()
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_urls_in_plain_text_and_trims_punctuation() {
        let links = extract("Se https://northwind.example/skisser/3 først. Og (https://en.wikipedia.org/wiki/Rust_(programming_language)), www.tidewater.example!", None);
        let urls: Vec<&str> = links.iter().map(|l| l.url.as_str()).collect();
        assert_eq!(urls, vec!["https://northwind.example/skisser/3", "https://en.wikipedia.org/wiki/Rust_(programming_language)", "https://www.tidewater.example"]);
    }

    #[test]
    fn reads_anchors_with_labels_and_kinds() {
        let html = r#"<a href="https://tldr.tech/l/1?utm_source=tldr&amp;id=7">Apple ships M6</a> <a href="mailto:anna@client.example">Anna</a> <a href="tel:+4790000000">Ring</a> <a href="https://tldr.tech/unsubscribe?u=1">Unsubscribe</a> <a href="https://tldr.tech/p">Manage preferences</a>"#;
        let links = extract("", Some(html));
        let kinds: Vec<LinkKind> = links.iter().map(|l| l.kind).collect();
        assert_eq!(kinds, vec![LinkKind::Web, LinkKind::Mailto, LinkKind::Tel, LinkKind::Unsubscribe, LinkKind::Preferences]);
        assert_eq!(links[0].label.as_deref(), Some("Apple ships M6"));
        assert_eq!(clean_url(&links[0].url), "https://tldr.tech/l/1?id=7");
    }

    #[test]
    fn reads_slack_link_syntax() {
        let links = extract("Se <https://github.com/SlyOtis/chatnip/pull/42|PR #42> og <https://docs.rs>", None);
        assert_eq!(links[0].url, "https://github.com/SlyOtis/chatnip/pull/42");
        assert_eq!(links[0].label.as_deref(), Some("PR #42"));
        assert_eq!(links[1].url, "https://docs.rs");
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn previews_skip_unsubscribe_and_duplicates() {
        let links = extract("https://a.no/x?utm_source=1 https://a.no/x https://b.no/unsubscribe mailto:x@y.no https://c.no", None);
        assert_eq!(preview_candidates(&links, 3), vec!["https://a.no/x", "https://c.no"]);
    }
}
