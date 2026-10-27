use super::rx;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageKind {
    Cid,
    Data,
    Hosted,
    Tracking,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Image {
    pub src: String,
    pub kind: ImageKind,
    pub alt: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteStyle {
    Gmail,
    Outlook,
    Apple,
    Thunderbird,
    Yahoo,
    Proton,
    Blockquote,
    Text,
}

pub fn attr(tag: &str, name: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = rx(&RE, r#"(?is)\s([a-z:_-]+)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#);
    re.captures_iter(tag)
        .find(|c| c[1].eq_ignore_ascii_case(name))
        .and_then(|c| c.get(2).or(c.get(3)).or(c.get(4)).map(|m| decode_entities(m.as_str())))
}

pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn decode_entities(text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    rx(&RE, r"&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z][a-zA-Z0-9]*);")
        .replace_all(text, |caps: &regex::Captures| {
            let name = &caps[1];
            if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32).map(String::from).unwrap_or_default();
            }
            if let Some(dec) = name.strip_prefix('#') {
                return dec.parse::<u32>().ok().and_then(char::from_u32).map(String::from).unwrap_or_default();
            }
            match name {
                "nbsp" => " ",
                "lt" => "<",
                "gt" => ">",
                "amp" => "&",
                "quot" => "\"",
                "apos" => "'",
                "ndash" => "–",
                "mdash" => "—",
                "hellip" => "…",
                "rsquo" | "lsquo" => "'",
                "ldquo" | "rdquo" => "\"",
                "laquo" => "«",
                "raquo" => "»",
                "copy" => "©",
                "euro" => "€",
                "aring" => "å",
                "Aring" => "Å",
                "oslash" => "ø",
                "Oslash" => "Ø",
                "aelig" => "æ",
                "AElig" => "Æ",
                _ => "",
            }
            .to_string()
        })
        .into_owned()
}

fn strip_hidden(html: &str) -> String {
    static NOISE: OnceLock<Regex> = OnceLock::new();
    static HIDDEN: OnceLock<Vec<Regex>> = OnceLock::new();
    let mut out = rx(&NOISE, r"(?is)<!--.*?-->|<head\b.*?</head>|<style\b.*?</style>|<script\b.*?</script>|<title\b.*?</title>").replace_all(html, "").into_owned();
    let hidden = HIDDEN.get_or_init(|| {
        ["div", "span", "p", "td", "table"]
            .iter()
            .map(|tag| Regex::new(&format!(r#"(?is)<{tag}\b[^>]*(?:display\s*:\s*none|class\s*=\s*["'][^"']*preheader[^"']*["']|mso-hide\s*:\s*all)[^>]*>.*?</{tag}>"#)).unwrap())
            .collect()
    });
    for re in hidden {
        out = re.replace_all(&out, "").into_owned();
    }
    out
}

pub fn hidden_text(html: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = rx(&RE, r#"(?is)<(div|span|p|td)\b[^>]*(?:display\s*:\s*none|class\s*=\s*["'][^"']*preheader[^"']*["'])[^>]*>(.*?)</(?:div|span|p|td)>"#);
    re.captures(html).map(|c| collapse(&decode_entities(&strip_tags(&c[2])))).filter(|t| !t.is_empty())
}

fn strip_tags(html: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    rx(&RE, r"(?s)<[^>]*>").replace_all(html, "").into_owned()
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn visible_text(html: &str) -> String {
    static AT: OnceLock<Regex> = OnceLock::new();
    static IMG: OnceLock<Regex> = OnceLock::new();
    static BLOCK: OnceLock<Regex> = OnceLock::new();
    static CELL: OnceLock<Regex> = OnceLock::new();
    static LINES: OnceLock<Regex> = OnceLock::new();
    let body = strip_hidden(html);
    let body = rx(&AT, r"(?is)<at\b[^>]*>(.*?)</at>").replace_all(&body, "@$1");
    let body = rx(&IMG, r"(?is)<img\b[^>]*>").replace_all(&body, "");
    let body = rx(&BLOCK, r"(?i)</?(?:p|div|br|tr|li|h[1-6]|table|blockquote|pre|ul|ol|hr|section|article|header|footer)\b[^>]*>").replace_all(&body, "\n");
    let body = rx(&CELL, r"(?i)</?(?:td|th)\b[^>]*>").replace_all(&body, " ");
    let text = decode_entities(&strip_tags(&body));
    let lines: Vec<String> = text.lines().map(collapse).collect();
    let joined = lines.join("\n");
    rx(&LINES, r"\n{3,}").replace_all(joined.trim(), "\n\n").into_owned()
}

pub fn flat_text(html: &str) -> String {
    collapse(&visible_text(html))
}

const QUOTE_MARKERS: [(&str, QuoteStyle); 9] = [
    (r#"(?i)<div\b[^>]*class\s*=\s*["'][^"']*\bgmail_quote\b"#, QuoteStyle::Gmail),
    (r#"(?i)<div\b[^>]*id\s*=\s*["'](?:x_)?divRplyFwdMsg["']"#, QuoteStyle::Outlook),
    (r#"(?i)<div\b[^>]*id\s*=\s*["']appendonsend["']"#, QuoteStyle::Outlook),
    (r#"(?i)<blockquote\b[^>]*type\s*=\s*["']cite["']"#, QuoteStyle::Apple),
    (r#"(?i)<div\b[^>]*class\s*=\s*["'][^"']*\bmoz-cite-prefix\b"#, QuoteStyle::Thunderbird),
    (r#"(?i)<div\b[^>]*class\s*=\s*["'][^"']*\byahoo_quoted\b"#, QuoteStyle::Yahoo),
    (r#"(?i)<div\b[^>]*class\s*=\s*["'][^"']*\bprotonmail_quote\b"#, QuoteStyle::Proton),
    (r"(?i)-----\s*Original Message\s*-----", QuoteStyle::Outlook),
    (r"(?i)<blockquote\b", QuoteStyle::Blockquote),
];

pub fn split_quoted(html: &str) -> (String, Option<(String, QuoteStyle)>) {
    static MARKERS: OnceLock<Vec<(Regex, QuoteStyle)>> = OnceLock::new();
    let markers = MARKERS.get_or_init(|| QUOTE_MARKERS.iter().map(|(p, s)| (Regex::new(p).unwrap(), *s)).collect());
    let mut best: Option<(usize, QuoteStyle)> = None;
    for (re, style) in markers {
        if let Some(m) = re.find(html) {
            let start = if *style == QuoteStyle::Apple {
                html[..m.start()].rfind("<div").filter(|i| html[*i..m.start()].len() < 40).unwrap_or(m.start())
            } else {
                m.start()
            };
            if best.map(|(b, _)| start < b).unwrap_or(true) {
                best = Some((start, *style));
            }
        }
    }
    match best {
        Some((start, style)) if !flat_text(&html[..start]).is_empty() => (html[..start].to_string(), Some((html[start..].to_string(), style))),
        _ => (html.to_string(), None),
    }
}

pub fn split_signature(html: &str) -> (String, Option<String>) {
    static RE: OnceLock<Regex> = OnceLock::new();
    match rx(&RE, r#"(?i)<div\b[^>]*(?:class|id)\s*=\s*["'][^"']*\b(?:gmail_signature|signature|moz-signature)\b"#).find(html) {
        Some(m) => {
            let signature = flat_text(&html[m.start()..]);
            (html[..m.start()].to_string(), Some(signature).filter(|s| !s.is_empty()))
        }
        None => (html.to_string(), None),
    }
}

pub fn images(html: &str) -> Vec<Image> {
    static RE: OnceLock<Regex> = OnceLock::new();
    rx(&RE, r"(?is)<img\b[^>]*>")
        .find_iter(html)
        .filter_map(|m| {
            let tag = m.as_str();
            let src = attr(tag, "src")?;
            let style = attr(tag, "style").unwrap_or_default().replace(' ', "").to_lowercase();
            let tiny = |name: &str| attr(tag, name).map(|v| v.trim_end_matches("px") == "1" || v == "0").unwrap_or(false);
            let kind = if src.starts_with("cid:") {
                ImageKind::Cid
            } else if src.starts_with("data:") {
                ImageKind::Data
            } else if src.contains("hostedContents") {
                ImageKind::Hosted
            } else if (tiny("width") && tiny("height")) || style.contains("display:none") || style.contains("width:1px") {
                ImageKind::Tracking
            } else {
                ImageKind::Remote
            };
            Some(Image { src, kind, alt: attr(tag, "alt").filter(|a| !a.is_empty()) })
        })
        .collect()
}

pub fn columns(html: &str) -> usize {
    static TD: OnceLock<Regex> = OnceLock::new();
    let td = rx(&TD, r"(?i)<t[dh]\b");
    html.split("<tr")
        .skip(1)
        .map(|segment| td.find_iter(segment.split("</tr>").next().unwrap_or(segment)).count())
        .max()
        .unwrap_or(0)
}

pub fn table_depth(html: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let mut depth = 0usize;
    let mut max = 0usize;
    for m in rx(&RE, r"(?i)</?table\b").find_iter(html) {
        if m.as_str().starts_with("</") {
            depth = depth.saturating_sub(1);
        } else {
            depth += 1;
            max = max.max(depth);
        }
    }
    max
}

pub fn code_blocks(html: &str) -> Vec<(Option<String>, String)> {
    static RE: OnceLock<Regex> = OnceLock::new();
    rx(&RE, r"(?is)<(codeblock|pre)\b([^>]*)>(.*?)</(?:codeblock|pre)>")
        .captures_iter(html)
        .map(|c| {
            let open = format!("<x {}>", &c[2]);
            let language = attr(&open, "class").or_else(|| attr(&open, "lang")).filter(|l| !l.is_empty());
            (language, decode_entities(&strip_tags(&c[3])).trim().to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_text_keeps_blocks_and_drops_noise() {
        let html = "<html><head><style>p{color:red}</style><title>x</title></head><body><!-- c --><div style=\"display:none\">preview</div><p>Hei&nbsp;Milo,</p><p>Linje &amp; to</p><script>alert(1)</script><table><tr><td>A</td><td>B</td></tr></table></body></html>";
        assert_eq!(visible_text(html), "Hei Milo,\n\nLinje & to\n\nA B");
        assert_eq!(flat_text(html), "Hei Milo, Linje & to A B");
        assert_eq!(hidden_text(html).as_deref(), Some("preview"));
    }

    #[test]
    fn decodes_numeric_and_named_entities() {
        assert_eq!(decode_entities("&#248;&#xE5; &AElig; &hellip; &bogus;"), "øå Æ … ");
    }

    #[test]
    fn reads_attributes_in_any_quote_style() {
        assert_eq!(attr(r#"<img src='a.png' width=1 alt="x &amp; y">"#, "alt").as_deref(), Some("x & y"));
        assert_eq!(attr(r#"<img src='a.png' width=1>"#, "width").as_deref(), Some("1"));
    }

    #[test]
    fn classifies_images() {
        let html = r#"<img src="cid:logo@client.example" alt="Kunde"><img src="https://t.example/o.gif" width="1" height="1"><img src="../hostedContents/1/$value"><img src="data:image/png;base64,AAA"><img src="https://cdn.example/hero.jpg" width="600">"#;
        let kinds: Vec<ImageKind> = images(html).into_iter().map(|i| i.kind).collect();
        assert_eq!(kinds, vec![ImageKind::Cid, ImageKind::Tracking, ImageKind::Hosted, ImageKind::Data, ImageKind::Remote]);
    }

    #[test]
    fn counts_columns_and_nesting() {
        let html = "<table><tr><td><table><tr><td colspan=2>h</td></tr><tr><td>a</td><td>b</td><td>c</td></tr></table></td></tr></table>";
        assert_eq!(columns(html), 3);
        assert_eq!(table_depth(html), 2);
    }

    #[test]
    fn splits_each_quote_style() {
        let cases = [
            ("<div>Ja</div><div class=\"gmail_quote\">On Mon wrote:<blockquote class=\"gmail_quote\">x</blockquote></div>", QuoteStyle::Gmail),
            ("<p>Ja</p><div id=\"divRplyFwdMsg\"><b>From:</b> x</div><p>old</p>", QuoteStyle::Outlook),
            ("<p>Ja</p><div id=\"x_divRplyFwdMsg\">x</div>", QuoteStyle::Outlook),
            ("<div>Ja</div><div><br><blockquote type=\"cite\"><div>old</div></blockquote></div>", QuoteStyle::Apple),
            ("<p>Ja</p><div class=\"moz-cite-prefix\">On x wrote:</div><blockquote>old</blockquote>", QuoteStyle::Thunderbird),
            ("<p>Ja</p><div class=\"yahoo_quoted\">old</div>", QuoteStyle::Yahoo),
            ("<p>Ja</p><div class=\"protonmail_quote\">old</div>", QuoteStyle::Proton),
            ("<p>Ja</p><p>-----Original Message-----<br>From: x</p>", QuoteStyle::Outlook),
            ("<p>Ja</p><blockquote>old</blockquote>", QuoteStyle::Blockquote),
        ];
        for (html, style) in cases {
            let (body, quoted) = split_quoted(html);
            assert_eq!(flat_text(&body), "Ja", "{}", html);
            assert_eq!(quoted.map(|q| q.1), Some(style), "{}", html);
        }
    }

    #[test]
    fn a_quote_alone_is_not_split_away() {
        let (body, quoted) = split_quoted("<blockquote>Only a quote</blockquote>");
        assert!(quoted.is_none());
        assert_eq!(flat_text(&body), "Only a quote");
    }

    #[test]
    fn finds_code_blocks() {
        let blocks = code_blocks(r#"<p>See</p><codeblock class="Rust"><code>fn main() {}<br></code></codeblock><pre>ls -la &amp;&amp; pwd</pre>"#);
        assert_eq!(blocks, vec![(Some("Rust".to_string()), "fn main() {}".to_string()), (None, "ls -la && pwd".to_string())]);
    }
}
