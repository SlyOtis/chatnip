use super::calendar::{parse_ics, Invite};
use super::headers;
use super::html::{self, Image, QuoteStyle};
use super::links::{self, Link};
use super::rx;
use crate::providers::traits::ResponseMessage;
use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use chrono::{DateTime, Utc};
use mail_parser::{MessageParser, MimeHeaders};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MailStructure {
    pub text: String,
    pub quoted: Option<String>,
    pub quote_style: Option<QuoteStyle>,
    pub signature: Option<String>,
    pub preheader: Option<String>,
    pub images: Vec<Image>,
    pub links: Vec<Link>,
    pub previews: Vec<String>,
    pub unsubscribe: Vec<String>,
    pub is_bulk: bool,
    pub columns: usize,
    pub table_depth: usize,
    pub invites: Vec<Invite>,
}

impl MailStructure {
    pub fn apply(&self, message: &mut ResponseMessage) {
        if let Ok(value) = serde_json::to_value(self) {
            message.metadata.insert("structure".to_string(), value);
        }
        if let Some(invite) = self.invites.first() {
            message.metadata.insert(
                "event".to_string(),
                serde_json::json!({
                    "title": invite.title,
                    "starts_at": invite.start.to_rfc3339(),
                    "ends_at": invite.end.map(|e| e.to_rfc3339()),
                    "location": invite.location.clone().unwrap_or_default(),
                    "organizer": invite.organizer_email.clone().unwrap_or_default(),
                }),
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Address {
    pub name: Option<String>,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub inline: bool,
    pub content_id: Option<String>,
    pub data: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MailContent {
    pub subject: Option<String>,
    pub from: Option<Address>,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub date: Option<DateTime<Utc>>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub html: Option<String>,
    pub plain: Option<String>,
    pub parts: Vec<Part>,
    pub headers: String,
    pub origin_ip: Option<String>,
    pub structure: MailStructure,
}

pub fn split_quoted_text(text: &str) -> (String, Option<String>) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let header = rx(&RE, r"(?i)^(?:on .+wrote:|den .+ skrev .+:|.+ skrev .+:$|-----\s*original message\s*-----|-{2,}\s*forwarded message\s*-{2,}|from: .+)$");
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().enumerate().position(|(i, line)| {
        let t = line.trim();
        i > 0 && (header.is_match(t) || (t.starts_with('>') && lines[..i].iter().any(|l| !l.trim().is_empty() && !l.trim().starts_with('>'))))
    });
    match start {
        Some(i) => (lines[..i].join("\n").trim_end().to_string(), Some(lines[i..].join("\n"))),
        None => (text.to_string(), None),
    }
}

pub fn split_signature_text(text: &str) -> (String, Option<String>) {
    let lines: Vec<&str> = text.lines().collect();
    match lines.iter().rposition(|l| *l == "-- " || *l == "--" || *l == "—") {
        Some(i) if i > 0 => (lines[..i].join("\n").trim_end().to_string(), Some(lines[i + 1..].join("\n").trim().to_string()).filter(|s| !s.is_empty())),
        _ => (text.to_string(), None),
    }
}

fn invites_in(text: &str) -> Vec<Invite> {
    match (text.find("BEGIN:VCALENDAR"), text.rfind("END:VCALENDAR")) {
        (Some(start), Some(end)) if end > start => parse_ics(&text[start..end + 13]),
        _ => Vec::new(),
    }
}

pub fn analyse(html: Option<&str>, plain: Option<&str>, raw_headers: Option<&str>, calendar: Option<&str>) -> MailStructure {
    let mut structure = MailStructure::default();
    if let Some(h) = html.filter(|h| !h.trim().is_empty()) {
        let (body, quoted) = html::split_quoted(h);
        let (body, signature) = html::split_signature(&body);
        structure.text = html::visible_text(&body);
        structure.quoted = quoted.as_ref().map(|(q, _)| html::visible_text(q));
        structure.quote_style = quoted.map(|(_, s)| s);
        structure.signature = signature;
        structure.preheader = html::hidden_text(h);
        structure.images = html::images(h);
        structure.links = links::extract("", Some(h));
        structure.columns = html::columns(h);
        structure.table_depth = html::table_depth(h);
    } else if let Some(p) = plain {
        let (body, quoted) = split_quoted_text(p);
        let (body, signature) = split_signature_text(&body);
        structure.text = body.trim().to_string();
        structure.quote_style = quoted.as_ref().map(|_| QuoteStyle::Text);
        structure.quoted = quoted;
        structure.signature = signature;
        structure.links = links::extract(p, None);
    }
    if let Some(h) = raw_headers {
        structure.unsubscribe = headers::unsubscribe(h);
        structure.is_bulk = headers::is_bulk(h);
    }
    if structure.unsubscribe.is_empty() {
        structure.unsubscribe = structure.links.iter().filter(|l| l.kind == links::LinkKind::Unsubscribe).map(|l| l.url.clone()).collect();
    }
    structure.is_bulk = structure.is_bulk || !structure.unsubscribe.is_empty() || structure.images.iter().any(|i| i.kind == html::ImageKind::Tracking);
    structure.previews = links::preview_candidates(&structure.links, 3);
    structure.invites = calendar.map(invites_in).unwrap_or_default();
    structure
}

fn address(addr: Option<&mail_parser::Address<'_>>) -> Vec<Address> {
    addr.map(|a| {
        a.iter()
            .filter_map(|x| x.address().map(|e| Address { name: x.name().map(String::from), email: e.to_lowercase() }))
            .collect()
    })
    .unwrap_or_default()
}

pub fn parse(raw: &[u8]) -> Option<MailContent> {
    let parsed = MessageParser::default().parse(raw)?;
    let headers = parsed.headers_raw().map(|(name, value)| format!("{}:{}", name, value.trim_end())).collect::<Vec<_>>().join("\r\n");
    let html = parsed.parts.iter().any(|p| p.is_content_type("text", "html")).then(|| parsed.body_html(0).map(|h| h.to_string())).flatten();
    let plain = parsed.parts.iter().any(|p| p.is_content_type("text", "plain")).then(|| parsed.body_text(0).map(|t| t.to_string())).flatten();
    let mut calendar = parsed
        .parts
        .iter()
        .find(|p| p.is_content_type("text", "calendar"))
        .and_then(|p| p.text_contents().map(String::from).or_else(|| std::str::from_utf8(p.contents()).ok().map(String::from)));
    let mut parts = Vec::new();
    for part in parsed.attachments() {
        let mime = part.content_type().map(|c| format!("{}/{}", c.c_type, c.c_subtype.as_deref().unwrap_or("octet-stream")).to_lowercase()).unwrap_or_else(|| "application/octet-stream".to_string());
        let content_id = part.content_id().map(|c| c.trim_matches(|ch| ch == '<' || ch == '>').to_string());
        let disposition_inline = part.content_disposition().map(|d| d.c_type.eq_ignore_ascii_case("inline")).unwrap_or(false);
        if mime == "text/calendar" && part.attachment_name().is_none() {
            continue;
        }
        let name = part.attachment_name().map(String::from).unwrap_or_else(|| content_id.clone().unwrap_or_else(|| "attachment".to_string()));
        if calendar.is_none() && (mime.contains("calendar") || mime.contains("ics") || name.ends_with(".ics")) {
            calendar = std::str::from_utf8(part.contents()).ok().map(String::from);
        }
        let inline = disposition_inline || content_id.is_some();
        let data = (inline && mime.starts_with("image/") && part.contents().len() <= 512 * 1024).then(|| format!("data:{};base64,{}", mime, STANDARD.encode(part.contents())));
        parts.push(Part { name, mime, size: part.contents().len() as u64, inline, content_id, data });
    }
    let structure = analyse(html.as_deref(), plain.as_deref(), Some(&headers), calendar.as_deref());
    Some(MailContent {
        subject: parsed.subject().map(String::from),
        from: address(parsed.from()).into_iter().next(),
        to: address(parsed.to()),
        cc: address(parsed.cc()),
        date: parsed.date().and_then(|d| DateTime::from_timestamp(d.to_timestamp(), 0)),
        message_id: parsed.message_id().map(String::from),
        in_reply_to: parsed.in_reply_to().as_text().map(String::from),
        html,
        plain,
        parts,
        origin_ip: headers::origin_ip(&headers).map(|ip| ip.to_string()),
        headers,
        structure,
    })
}

fn decode_gmail(data: &str) -> Option<String> {
    URL_SAFE_NO_PAD
        .decode(data.trim_end_matches('='))
        .or_else(|_| URL_SAFE.decode(data))
        .or_else(|_| STANDARD.decode(data))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

pub fn gmail_bodies(payload: &Value) -> (Option<String>, Option<String>, Option<String>) {
    let mut html = None;
    let mut plain = None;
    let mut calendar = None;
    let mut stack = vec![payload];
    while let Some(part) = stack.pop() {
        let mime = part["mimeType"].as_str().unwrap_or("").to_lowercase();
        let has_name = part["filename"].as_str().map(|n| !n.is_empty()).unwrap_or(false);
        if let Some(text) = part["body"]["data"].as_str().and_then(decode_gmail) {
            match mime.as_str() {
                "text/html" if !has_name && html.is_none() => html = Some(text),
                "text/plain" if !has_name && plain.is_none() => plain = Some(text),
                "text/calendar" if calendar.is_none() => calendar = Some(text),
                _ => {}
            }
        }
        if let Some(children) = part["parts"].as_array() {
            stack.extend(children.iter().rev());
        }
    }
    (html, plain, calendar)
}

pub fn gmail_parts(payload: &Value) -> Vec<(String, Part)> {
    let mut out = Vec::new();
    let mut stack = vec![payload];
    while let Some(part) = stack.pop() {
        if let Some(name) = part["filename"].as_str().filter(|n| !n.is_empty()) {
            let header = |key: &str| part["headers"].as_array().and_then(|hs| hs.iter().find(|h| h["name"].as_str().map(|n| n.eq_ignore_ascii_case(key)).unwrap_or(false))).and_then(|h| h["value"].as_str());
            let content_id = header("Content-ID").map(|c| c.trim_matches(|ch| ch == '<' || ch == '>').to_string());
            let inline = header("Content-Disposition").map(|d| d.to_lowercase().starts_with("inline")).unwrap_or(false) || content_id.is_some();
            let mime = part["mimeType"].as_str().unwrap_or("application/octet-stream").to_string();
            let data = part["body"]["data"].as_str().filter(|_| inline && mime.starts_with("image/")).and_then(|d| URL_SAFE_NO_PAD.decode(d.trim_end_matches('=')).ok()).map(|bytes| format!("data:{};base64,{}", mime, STANDARD.encode(bytes)));
            out.push((
                part["body"]["attachmentId"].as_str().unwrap_or("").to_string(),
                Part { name: name.to_string(), mime, size: part["body"]["size"].as_u64().unwrap_or(0), inline, content_id, data },
            ));
        }
        if let Some(children) = part["parts"].as_array() {
            stack.extend(children.iter().rev());
        }
    }
    out
}

pub fn gmail_headers(payload: &Value) -> String {
    payload["headers"]
        .as_array()
        .map(|hs| hs.iter().filter_map(|h| Some(format!("{}: {}", h["name"].as_str()?, h["value"].as_str()?))).collect::<Vec<_>>().join("\r\n"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::fixture;
    use crate::content::html::ImageKind;
    use crate::content::links::LinkKind;

    fn mail(name: &str) -> MailContent {
        parse(fixture(name).as_bytes()).unwrap()
    }

    #[test]
    fn plain_text_mail_has_body_signature_and_link() {
        let m = mail("plain.eml");
        assert!(m.html.is_none());
        assert_eq!(m.from.as_ref().unwrap().email, "eva.ortega@northwind.example");
        assert_eq!(m.from.as_ref().unwrap().name.as_deref(), Some("Eva Ortega"));
        assert!(m.structure.text.starts_with("Hei Milo,"));
        assert!(m.structure.text.ends_with("Mvh\nEva"));
        assert_eq!(m.structure.signature.as_deref(), Some("Eva Ortega\nDesigner, Northwind\n+47 900 00 000"));
        assert!(m.structure.quoted.is_none());
        assert_eq!(m.structure.previews, vec!["https://northwind.example/skisser/3"]);
        assert!(!m.structure.is_bulk);
        assert_eq!(m.origin_ip.as_deref(), Some("185.35.187.100"));
        assert!(m.parts.is_empty());
    }

    #[test]
    fn gmail_reply_splits_signature_and_quote() {
        let m = mail("gmail-quote.eml");
        assert!(m.html.is_some() && m.plain.is_some());
        assert_eq!(m.structure.quote_style, Some(QuoteStyle::Gmail));
        assert_eq!(m.structure.text, "Takk Milo, vi tar tilbudet. Kan vi signere torsdag?\n\nAnna");
        assert_eq!(m.structure.signature.as_deref(), Some("Anna Berg CTO, Kunde AS client.example"));
        let quoted = m.structure.quoted.unwrap();
        assert!(quoted.starts_with("On Mon, 5 Oct 2026 at 15:40, Milo Oakes <milo@northwind.example> wrote:"));
        assert!(quoted.contains("Vedlagt ligger tilbudet"));
        assert_eq!(m.in_reply_to.as_deref(), Some("plain-9@northwind.example"));
    }

    #[test]
    fn gmail_plain_alternative_splits_the_same_way() {
        let m = mail("gmail-quote.eml");
        let (body, quoted) = split_quoted_text(m.plain.as_deref().unwrap());
        assert_eq!(body, "Takk Milo, vi tar tilbudet. Kan vi signere torsdag?\n\nAnna");
        assert!(quoted.unwrap().lines().nth(1).unwrap().starts_with("> Hei Anna"));
    }

    #[test]
    fn outlook_reply_is_quoted_printable_with_a_reply_header() {
        let m = mail("outlook-quote.eml");
        assert_eq!(m.structure.quote_style, Some(QuoteStyle::Outlook));
        assert!(m.structure.text.starts_with("Hi, your access request is approved."));
        assert!(m.structure.text.contains("Kari Nordmann | Platform lead | Tidewater"));
        let quoted = m.structure.quoted.unwrap();
        assert!(quoted.contains("From: Milo Oakes <milo@northwind.example>"));
        assert!(quoted.contains("could I get access to the staging cluster?"));
        assert!(!m.structure.text.contains("MsoNormal"));
        assert_eq!(m.origin_ip.as_deref(), Some("85.166.12.9"));
    }

    #[test]
    fn apple_mail_reply_folds_the_cite_blockquote() {
        let m = mail("apple-quote.eml");
        assert_eq!(m.structure.quote_style, Some(QuoteStyle::Apple));
        assert_eq!(m.structure.text, "Kommer! Skal jeg ta med noe? 🥗\n\nJonas");
        assert!(m.structure.quoted.unwrap().contains("Lunsj fredag kl 12?"));
        assert_eq!(m.origin_ip.as_deref(), Some("17.57.154.21"));
    }

    #[test]
    fn newsletter_is_bulk_with_columns_preheader_and_tracking() {
        let m = mail("newsletter.eml");
        let s = &m.structure;
        assert!(s.is_bulk);
        assert_eq!(s.preheader.as_deref(), Some("Rust 2.0, Svelte 6, and the end of cookies. Preview text you should never see."));
        assert!(!s.text.contains("Preview text you should never see"));
        assert!(s.text.starts_with("TLDR"));
        assert_eq!(s.columns, 2);
        assert_eq!(s.table_depth, 2);
        assert_eq!(s.images.iter().filter(|i| i.kind == ImageKind::Tracking).count(), 1);
        assert_eq!(s.unsubscribe[0], "https://tldr.tech/unsubscribe?u=abc123");
        assert!(s.links.iter().any(|l| l.kind == LinkKind::Preferences));
        assert_eq!(s.previews, vec!["https://tldr.tech/l/1", "https://tldr.tech/l/2"]);
        assert!(crate::services::people_service::is_service(m.from.as_ref().map(|a| a.email.as_str()), Some("TLDR Newsletter")) || s.is_bulk);
    }

    #[test]
    fn calendar_invite_becomes_an_event_candidate() {
        let m = mail("invite.eml");
        assert_eq!(m.structure.invites.len(), 1);
        let invite = &m.structure.invites[0];
        assert_eq!(invite.title, "Designreview");
        assert_eq!(invite.location.as_deref(), Some("Møterom Fjell, Oslo"));
        assert_eq!(invite.organizer_email.as_deref(), Some("mia.sund@design.no"));
        let mut message = ResponseMessage::default();
        m.structure.apply(&mut message);
        assert_eq!(message.metadata["event"]["title"], "Designreview");
        assert_eq!(message.metadata["event"]["ends_at"], "2026-10-08T12:00:00+00:00");
        assert_eq!(message.metadata["structure"]["invites"][0]["location"], "Møterom Fjell, Oslo");
        assert_eq!(m.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["invite.ics"]);
        assert!(crate::services::people_service::is_service(m.from.as_ref().map(|a| a.email.as_str()), m.from.as_ref().and_then(|a| a.name.as_deref())));
    }

    #[test]
    fn attachments_keep_mime_size_and_inline_cid() {
        let m = mail("attachments.eml");
        let summary: Vec<(String, String, u64, bool)> = m.parts.iter().map(|p| (p.name.clone(), p.mime.clone(), p.size, p.inline)).collect();
        assert_eq!(
            summary,
            vec![
                ("logo.png".to_string(), "image/png".to_string(), 67, true),
                ("tilbud.pdf".to_string(), "application/pdf".to_string(), 36, false),
                ("budsjett.xlsx".to_string(), "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(), 22, false),
                ("skisser.zip".to_string(), "application/zip".to_string(), 22, false),
            ]
        );
        assert_eq!(m.parts[0].content_id.as_deref(), Some("logo@client.example"));
        assert_eq!(m.structure.images.iter().map(|i| i.kind).collect::<Vec<_>>(), vec![ImageKind::Cid]);
    }

    #[test]
    fn gmail_api_payload_decodes_nested_parts() {
        let message: Value = serde_json::from_str(&fixture("gmail-message.json")).unwrap();
        let (html, plain, calendar) = gmail_bodies(&message["payload"]);
        assert!(html.unwrap().contains("gmail_quote"));
        assert_eq!(plain.unwrap().lines().next(), Some("Takk, vi ses torsdag. 👍"));
        assert!(calendar.unwrap().contains("SUMMARY:Signering"));
        let parts = gmail_parts(&message["payload"]);
        assert_eq!(parts.iter().map(|(_, p)| (p.name.as_str(), p.inline)).collect::<Vec<_>>(), vec![("kontrakt.pdf", false), ("sign.png", true)]);
        let headers = gmail_headers(&message["payload"]);
        assert_eq!(crate::content::headers::value(&headers, "Subject").as_deref(), Some("Re: Spørsmål om kontrakten"));
        let (html, plain, calendar) = gmail_bodies(&message["payload"]);
        let s = analyse(html.as_deref(), plain.as_deref(), Some(&headers), calendar.as_deref());
        assert_eq!(s.quote_style, Some(QuoteStyle::Gmail));
        assert_eq!(s.text, "Takk, vi ses torsdag. 👍");
        assert_eq!(s.invites[0].title, "Signering");
    }

    #[test]
    fn plain_text_quote_variants() {
        for (text, expected) in [
            ("Ja.\n\nDen 5. okt. 2026 kl. 10:00 skrev Anna Berg <anna@client.example>:\n> Hei", "Ja."),
            ("Ja.\n\n-----Original Message-----\nFrom: Kari", "Ja."),
            ("Ja.\n> sitat\n> mer", "Ja."),
            ("Ja.\n\n---------- Forwarded message ---------\nFrom: x", "Ja."),
        ] {
            let (body, quoted) = split_quoted_text(text);
            assert_eq!(body, expected, "{}", text);
            assert!(quoted.is_some(), "{}", text);
        }
        let (body, quoted) = split_quoted_text("> starts with a quote\nand continues");
        assert!(quoted.is_none());
        assert_eq!(body, "> starts with a quote\nand continues");
    }
}
