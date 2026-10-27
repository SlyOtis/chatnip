use super::rx;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/emoji.rs"));

pub const TEAMS_REACTIONS: [(&str, &str); 6] = [("like", "👍"), ("heart", "❤️"), ("laugh", "😆"), ("surprised", "😮"), ("sad", "😢"), ("angry", "😡")];

const VARIATION: char = '\u{FE0F}';
const TEXT_VARIATION: char = '\u{FE0E}';
const KEYCAP: char = '\u{20E3}';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Slack,
    Teams,
    Mail,
    GoogleChat,
}

impl Platform {
    pub fn of(provider_type: &str, conversation_external_id: &str, is_mail: bool) -> Platform {
        match provider_type {
            "slack" => Platform::Slack,
            "microsoft" if is_mail || conversation_external_id.starts_with("mail:") => Platform::Mail,
            "microsoft" => Platform::Teams,
            _ if is_mail => Platform::Mail,
            _ => Platform::GoogleChat,
        }
    }
}

struct Table {
    by_name: HashMap<&'static str, &'static str>,
    by_unicode: HashMap<String, (&'static str, bool)>,
    starts: HashSet<char>,
    longest: usize,
}

fn key(text: &str) -> String {
    text.chars().filter(|c| *c != VARIATION && *c != TEXT_VARIATION).collect()
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut by_name = HashMap::new();
        let mut by_unicode = HashMap::new();
        let mut starts = HashSet::new();
        let mut longest = 1;
        for (name, unicode, primary, text) in EMOJI {
            by_name.entry(*name).or_insert(*unicode);
            if *primary {
                by_unicode.entry(key(unicode)).or_insert((*name, *text));
            }
            if let Some(first) = unicode.chars().next() {
                starts.insert(first);
            }
            longest = longest.max(unicode.chars().count() + 1);
        }
        Table { by_name, by_unicode, starts, longest }
    })
}

pub fn shortcode_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    rx(&PATTERN, r":([a-z0-9_+'\-]+):(?::(skin-tone-[2-6](?:-[2-6])?):)?")
}

fn joined(name: &str, tone: Option<&str>) -> String {
    match tone {
        Some(tone) => format!("{}::{}", name, tone),
        None => name.to_string(),
    }
}

pub fn unicode(name: &str) -> Option<&'static str> {
    let name = name.trim().trim_start_matches(':').trim_end_matches(':');
    table().by_name.get(name).copied()
}

pub fn shortcode(emoji: &str) -> Option<&'static str> {
    table().by_unicode.get(&key(emoji.trim())).map(|(name, _)| *name)
}

pub fn is_standard(name: &str) -> bool {
    unicode(name).is_some()
}

fn longest_at(chars: &[char], at: usize) -> Option<(usize, &'static str)> {
    let t = table();
    let first = chars[at];
    if !t.starts.contains(&first) {
        return None;
    }
    if first.is_ascii() && !matches!(chars.get(at + 1), Some(&VARIATION) | Some(&KEYCAP)) {
        return None;
    }
    let room = (chars.len() - at).min(t.longest);
    for len in (1..=room).rev() {
        let slice: String = chars[at..at + len].iter().collect();
        let Some((name, text_default)) = t.by_unicode.get(&key(&slice)) else { continue };
        let mut used = len;
        let mut presented = slice.contains(VARIATION);
        while matches!(chars.get(at + used), Some(&VARIATION) | Some(&TEXT_VARIATION)) {
            presented |= chars[at + used] == VARIATION;
            used += 1;
        }
        if *text_default && !presented {
            continue;
        }
        return Some((used, name));
    }
    None
}

pub fn canonical_text(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        match longest_at(&chars, i) {
            Some((used, name)) => {
                out.push(':');
                out.push_str(name);
                out.push(':');
                i += used;
            }
            None => {
                out.push(chars[i]);
                i += 1;
            }
        }
    }
    out
}

fn decode(text: &str) -> String {
    text.replace("&quot;", "\"").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

fn emoji_tag(tag: &str) -> Option<String> {
    static ALT: OnceLock<Regex> = OnceLock::new();
    static TAG: OnceLock<Regex> = OnceLock::new();
    let lower = tag.to_lowercase();
    let teams = lower.starts_with("<emoji") || (lower.starts_with("<img") && lower.contains("schema.skype.com/emoji"));
    if !teams || !rx(&TAG, r"^<\w").is_match(tag) {
        return None;
    }
    let alt = rx(&ALT, r#"(?i)\balt\s*=\s*(?:"([^"]*)"|'([^']*)')"#).captures(tag)?;
    let text = decode(alt.get(1).or_else(|| alt.get(2))?.as_str());
    let canonical = canonical_text(&text);
    (canonical != text || shortcode_pattern().is_match(&canonical)).then_some(canonical)
}

fn segments(html: &str, text: impl Fn(&str) -> String, tag: impl Fn(&str) -> Option<String>) -> String {
    static TAGS: OnceLock<Regex> = OnceLock::new();
    let tags = rx(&TAGS, r"(?s)<!--.*?-->|<[^>]*>");
    let mut out = String::with_capacity(html.len());
    let mut last = 0;
    let mut skip_close = false;
    for found in tags.find_iter(html) {
        let between = &html[last..found.start()];
        out.push_str(&text(between));
        let raw = found.as_str();
        if skip_close && raw.eq_ignore_ascii_case("</emoji>") {
            skip_close = false;
        } else if let Some(replacement) = tag(raw) {
            out.push_str(&replacement);
            skip_close = raw.to_lowercase().starts_with("<emoji") && !raw.ends_with("/>");
        } else {
            out.push_str(raw);
        }
        last = found.end();
    }
    out.push_str(&text(&html[last..]));
    out
}

pub fn canonical_html(html: &str) -> String {
    segments(html, canonical_text, emoji_tag)
}

pub fn canonical(content: &str, content_type: &str) -> String {
    if content_type.eq_ignore_ascii_case("html") || content_type.to_lowercase().contains("text/html") {
        canonical_html(content)
    } else {
        canonical_text(content)
    }
}

pub fn unicode_text(text: &str) -> String {
    shortcode_pattern()
        .replace_all(text, |c: &regex::Captures| unicode(&joined(&c[1], c.get(2).map(|m| m.as_str()))).map(String::from).unwrap_or_else(|| c[0].to_string()))
        .into_owned()
}

pub fn unicode_html(html: &str) -> String {
    segments(html, unicode_text, |_| None)
}

pub fn custom_names(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in shortcode_pattern().captures_iter(text) {
        let name = c[1].to_string();
        if !is_standard(&name) && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

pub fn canonical_reaction(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    let bare = raw.trim_start_matches(':').trim_end_matches(':');
    if let Some(name) = unicode(bare).and_then(shortcode) {
        return format!(":{}:", name);
    }
    if raw.len() > 2 && raw.starts_with(':') && raw.ends_with(':') {
        return raw.to_string();
    }
    if let Some(name) = shortcode(raw) {
        return format!(":{}:", name);
    }
    if raw.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '\'' | ':')) {
        return format!(":{}:", raw.trim_matches(':'));
    }
    raw.to_string()
}

pub fn teams_canonical(kind: &str) -> String {
    match TEAMS_REACTIONS.iter().find(|(legacy, _)| kind.trim().eq_ignore_ascii_case(legacy)) {
        Some((_, glyph)) => canonical_reaction(glyph),
        None => canonical_reaction(kind),
    }
}

pub fn reaction_name(canonical: &str) -> String {
    canonical.trim().trim_start_matches(':').trim_end_matches(':').to_string()
}

pub fn teams_reaction(canonical: &str) -> Result<String, String> {
    let name = reaction_name(&canonical_reaction(canonical));
    let glyph = unicode(&name).ok_or_else(|| format!("Teams has no :{}: emoji; pick a standard one", name))?;
    let legacy = TEAMS_REACTIONS.iter().find(|(_, g)| key(g) == key(glyph)).map(|(kind, _)| kind.to_string());
    Ok(legacy.unwrap_or_else(|| glyph.to_string()))
}

pub fn slack_reaction(canonical: &str) -> String {
    reaction_name(&canonical_reaction(canonical))
}

pub fn slack_name(name: &str) -> String {
    let (base, tone) = name.split_once("::").map(|(b, t)| (b, Some(t))).unwrap_or((name, None));
    let base = base.trim_matches(':');
    joined(base, tone.map(|t| t.trim_matches(':')))
}

pub fn outbound(content: &str, content_type: &str, platform: Platform, own: &HashSet<String>, images: &HashMap<String, String>) -> String {
    let markdown = content_type == "markdown";
    let html = content_type == "html";
    let text = |part: &str| -> String {
        shortcode_pattern()
        .replace_all(part, |c: &regex::Captures| {
            let name = &c[1];
            let tone = c.get(2).map(|m| m.as_str());
            if is_standard(&joined(name, tone)) || is_standard(name) {
                return match (platform, markdown) {
                    (Platform::Slack, _) | (_, true) => c[0].to_string(),
                    _ => unicode(&joined(name, tone)).or_else(|| unicode(name)).map(String::from).unwrap_or_else(|| c[0].to_string()),
                };
            }
            if platform == Platform::Slack && own.contains(name) {
                return c[0].to_string();
            }
            match (images.get(name), markdown, html, platform) {
                (Some(url), true, _, _) => format!("![:{}:]({})", name, url),
                (Some(url), _, true, Platform::Mail) => format!("<img src=\"{}\" alt=\":{}:\" title=\":{}:\" style=\"height:1.25em;vertical-align:middle\">", crate::utils::text_extraction::html_escape(url), name, name),
                (Some(url), _, true, _) => format!("<a href=\"{}\">:{}:</a>", crate::utils::text_extraction::html_escape(url), name),
                (Some(url), false, false, Platform::Slack) => format!("<{}|:{}:>", url, name),
                _ => c[0].to_string(),
            }
        })
        .into_owned()
    };
    if html {
        segments(content, text, |_| None)
    } else {
        text(content)
    }
}

pub fn canonical_message(message: &mut crate::providers::traits::ResponseMessage) {
    let content = canonical(&message.content, &message.content_type);
    if content != message.content {
        if message.r#type != crate::providers::traits::MessageType::Email {
            message.metadata.insert("original_content".to_string(), serde_json::Value::String(std::mem::take(&mut message.content)));
        }
        message.content = content;
    }
    if let Some(serde_json::Value::Array(reactions)) = message.metadata.get_mut("reactions") {
        for reaction in reactions.iter_mut() {
            let Some(raw) = reaction["emoji"].as_str().map(String::from) else { continue };
            let canonical = canonical_reaction(&raw);
            if canonical != raw {
                if reaction.get("original").is_none() {
                    reaction["original"] = serde_json::Value::String(raw);
                }
                reaction["emoji"] = serde_json::Value::String(canonical);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_comes_from_the_dataset_with_slack_names_first() {
        assert!(EMOJI.len() > 5000);
        assert_eq!(unicode("+1"), Some("👍"));
        assert_eq!(unicode(":thumbsup:"), Some("👍"));
        assert_eq!(unicode("tada"), Some("🎉"));
        assert_eq!(unicode("+1::skin-tone-3"), Some("👍🏼"));
        assert_eq!(shortcode("👍"), Some("+1"));
        assert_eq!(shortcode("👍🏼"), Some("+1::skin-tone-3"));
        assert_eq!(shortcode("❤"), Some("heart"));
        assert_eq!(shortcode("❤️"), Some("heart"));
        assert_eq!(unicode("party_parrot"), None);
    }

    #[test]
    fn unicode_in_text_becomes_shortcodes_and_text_symbols_stay() {
        assert_eq!(canonical_text("Shipped 🚀 🎉!"), "Shipped :rocket: :tada:!");
        assert_eq!(canonical_text("Bra 👍🏽"), "Bra :+1::skin-tone-4:");
        assert_eq!(canonical_text("👩‍👩‍👧‍👦"), format!(":{}:", shortcode("👩‍👩‍👧‍👦").unwrap()));
        assert_eq!(canonical_text("Acme™ 10:30 #1 © 2026"), "Acme™ 10:30 #1 © 2026");
        assert_eq!(canonical_text("Merke™️"), format!("Merke:{}:", shortcode("™️").unwrap()));
        assert_eq!(canonical_text("#️⃣"), format!(":{}:", shortcode("#️⃣").unwrap()));
        assert_eq!(canonical_text("❤️❤"), ":heart:❤");
    }

    #[test]
    fn html_changes_only_text_and_teams_emoji_tags() {
        let teams = "<p>Takk <emoji id=\"like\" alt=\"👍\" title=\"Like\"></emoji> og <img itemtype=\"http://schema.skype.com/Emoji\" itemid=\"smile\" alt=\"🙂\" src=\"https://statics.teams.cdn.office.net/smile.png\"> <a title=\"🎉\" href=\"https://x.no/🎉\">🎉</a></p>";
        assert_eq!(canonical_html(teams), "<p>Takk :+1: og :slightly_smiling_face: <a title=\"🎉\" href=\"https://x.no/🎉\">:tada:</a></p>");
        assert_eq!(canonical("Hei 😀", "text"), "Hei :grinning:");
        assert_eq!(canonical("<b>😀</b>", "html"), "<b>:grinning:</b>");
        assert_eq!(unicode_html("<a href=\"x:tada:y\">:tada: :+1::skin-tone-2:</a>"), "<a href=\"x:tada:y\">🎉 👍🏻</a>");
        assert_eq!(unicode_text("Takk :tada: og :party_parrot: 10:30:00"), "Takk 🎉 og :party_parrot: 10:30:00");
    }

    #[test]
    fn reactions_normalise_from_every_platform_and_keep_custom_names() {
        assert_eq!(teams_canonical("like"), ":+1:");
        assert_eq!(teams_canonical("heart"), ":heart:");
        assert_eq!(teams_canonical("laugh"), ":laughing:");
        assert_eq!(teams_canonical("surprised"), ":open_mouth:");
        assert_eq!(teams_canonical("sad"), ":cry:");
        assert_eq!(teams_canonical("angry"), ":rage:");
        assert_eq!(teams_canonical("🚀"), ":rocket:");
        assert_eq!(canonical_reaction("angry"), ":angry:");
        assert_eq!(canonical_reaction(":thumbsup:"), ":+1:");
        assert_eq!(canonical_reaction("🙏"), ":pray:");
        assert_eq!(canonical_reaction("+1::skin-tone-3"), ":+1::skin-tone-3:");
        assert_eq!(canonical_reaction("party-parrot"), ":party-parrot:");
        assert_eq!(canonical_reaction(":eyes:"), ":eyes:");
        assert_eq!(teams_reaction(":+1:").unwrap(), "like");
        assert_eq!(teams_reaction(":thumbsup:").unwrap(), "like");
        assert_eq!(teams_reaction("❤️").unwrap(), "heart");
        assert_eq!(teams_reaction(":rage:").unwrap(), "angry");
        assert_eq!(teams_reaction(":rocket:").unwrap(), "🚀");
        assert_eq!(teams_reaction(":+1::skin-tone-3:").unwrap(), "👍🏼");
        assert!(teams_reaction(":party_parrot:").unwrap_err().contains("party_parrot"));
        assert_eq!(slack_reaction(":+1::skin-tone-3:"), "+1::skin-tone-3");
        assert_eq!(slack_reaction("👍🏼"), "+1::skin-tone-3");
        assert_eq!(slack_name(":+1::skin-tone-3:"), "+1::skin-tone-3");
    }

    #[test]
    fn outbound_maps_standard_and_custom_emoji_per_platform() {
        let own: HashSet<String> = ["party_parrot".to_string()].into_iter().collect();
        let images: HashMap<String, String> = [("party_parrot".to_string(), "https://emoji.slack-edge.com/T1/party_parrot/a.gif".to_string()), ("shipit".to_string(), "https://emoji.slack-edge.com/T1/shipit/b.png".to_string())].into_iter().collect();
        let none = HashSet::new();
        let text = "Klar :tada: :party_parrot: :shipit: :ukjent:";
        assert_eq!(outbound(text, "text", Platform::Slack, &own, &images), "Klar :tada: :party_parrot: <https://emoji.slack-edge.com/T1/shipit/b.png|:shipit:> :ukjent:");
        assert_eq!(outbound(text, "markdown", Platform::Slack, &own, &images), "Klar :tada: :party_parrot: ![:shipit:](https://emoji.slack-edge.com/T1/shipit/b.png) :ukjent:");
        assert_eq!(outbound(text, "text", Platform::Teams, &none, &images), "Klar 🎉 :party_parrot: :shipit: :ukjent:");
        assert_eq!(outbound(text, "markdown", Platform::Teams, &none, &images), "Klar :tada: ![:party_parrot:](https://emoji.slack-edge.com/T1/party_parrot/a.gif) ![:shipit:](https://emoji.slack-edge.com/T1/shipit/b.png) :ukjent:");
        assert_eq!(outbound("<p>:+1::skin-tone-5: :shipit:</p>", "html", Platform::Mail, &none, &images), "<p>👍🏾 <img src=\"https://emoji.slack-edge.com/T1/shipit/b.png\" alt=\":shipit:\" title=\":shipit:\" style=\"height:1.25em;vertical-align:middle\"></p>");
        assert_eq!(custom_names(text), vec!["party_parrot", "shipit", "ukjent"]);
        assert_eq!(Platform::of("microsoft", "mail:AAQ", false), Platform::Mail);
        assert_eq!(Platform::of("microsoft", "19:abc@thread.v2", false), Platform::Teams);
        assert_eq!(Platform::of("google", "gmail:1", true), Platform::Mail);
    }
}
