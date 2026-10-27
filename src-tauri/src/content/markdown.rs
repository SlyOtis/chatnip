use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

use super::chat::Mention;
use super::rx;
use super::emoji::unicode_text;
use crate::utils::text_extraction::html_escape;

#[derive(Debug, Clone, PartialEq)]
enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Underline(Vec<Inline>),
    Strike(Vec<Inline>),
    Code(String),
    Link(String, Vec<Inline>),
    Image(String, String),
    Emoji(String, String),
    Mention(String, String),
    Channel(String, String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Bullet,
    Ordered(u32),
    Task(bool),
}

#[derive(Debug, Clone, PartialEq)]
struct Item {
    depth: usize,
    kind: Kind,
    inlines: Vec<Inline>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Align {
    Default,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
enum Block {
    Blank,
    Text(Vec<Vec<Inline>>),
    Heading(usize, Vec<Inline>),
    Quote(Vec<Vec<Inline>>),
    List(Vec<Item>),
    Code(Option<String>, String),
    Table(Vec<Align>, Vec<Vec<Inline>>, Vec<Vec<Vec<Inline>>>),
    Rule,
}

#[derive(Clone, Copy, PartialEq)]
enum Target {
    Mail,
    Teams,
    Slack,
    GoogleChat,
    Plain,
}

struct Support {
    headings: usize,
    tables: bool,
    images: bool,
    mentions: bool,
    underline: bool,
    shortcodes: bool,
}

impl Target {
    fn support(self) -> Support {
        match self {
            Target::Mail => Support { headings: 6, tables: true, images: true, mentions: false, underline: true, shortcodes: true },
            Target::Teams => Support { headings: 3, tables: true, images: false, mentions: true, underline: true, shortcodes: true },
            Target::Slack => Support { headings: 0, tables: false, images: false, mentions: true, underline: false, shortcodes: false },
            Target::GoogleChat => Support { headings: 0, tables: false, images: false, mentions: false, underline: false, shortcodes: true },
            Target::Plain => Support { headings: 0, tables: false, images: false, mentions: false, underline: false, shortcodes: true },
        }
    }
}

pub fn html(source: &str) -> String {
    html_document(source, Target::Mail).0
}

pub fn teams(source: &str) -> (String, Vec<Mention>) {
    html_document(source, Target::Teams)
}

pub fn slack(source: &str) -> String {
    lines(source, Target::Slack)
}

pub fn google_chat(source: &str) -> String {
    lines(source, Target::GoogleChat)
}

pub fn plain(source: &str) -> String {
    lines(source, Target::Plain)
}

fn mention_pattern() -> &'static Regex {
    static MENTION: OnceLock<Regex> = OnceLock::new();
    rx(&MENTION, r"(\[@[^\]\n]+\])\(mention:([^)\s]*)\)")
}

pub fn is_broadcast(id: &str) -> bool {
    matches!(id, "here" | "channel" | "everyone") || id.starts_with("group:")
}

pub fn mention_ids(source: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in mention_pattern().captures_iter(source) {
        let id = c[2].to_string();
        if !id.is_empty() && !out.contains(&id) && !is_broadcast(&id) {
            out.push(id);
        }
    }
    out
}

fn channel_pattern() -> &'static Regex {
    static CHANNEL: OnceLock<Regex> = OnceLock::new();
    rx(&CHANNEL, r"(\[#[^\]\n]+\])\(channel:([^)\s]*)\)")
}

pub fn channel_ids(source: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in channel_pattern().captures_iter(source) {
        let id = c[2].to_string();
        if !id.is_empty() && !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

pub fn with_channel_ids(source: &str, ids: &HashMap<String, String>) -> String {
    channel_pattern().replace_all(source, |c: &regex::Captures| format!("{}(channel:{})", &c[1], ids.get(&c[2]).cloned().unwrap_or_default())).into_owned()
}

pub fn with_mention_ids(source: &str, ids: &HashMap<String, String>) -> String {
    mention_pattern()
        .replace_all(source, |c: &regex::Captures| {
            let id = if is_broadcast(&c[2]) { c[2].to_string() } else { ids.get(&c[2]).cloned().unwrap_or_default() };
            format!("{}(mention:{})", &c[1], id)
        })
        .into_owned()
}

fn blocks(source: &str) -> Vec<Block> {
    static ITEM: OnceLock<Regex> = OnceLock::new();
    static TASK: OnceLock<Regex> = OnceLock::new();
    static HEADING: OnceLock<Regex> = OnceLock::new();
    static RULE: OnceLock<Regex> = OnceLock::new();
    static DELIMITER: OnceLock<Regex> = OnceLock::new();
    static FENCE: OnceLock<Regex> = OnceLock::new();
    let item = rx(&ITEM, r"^([ \t]*)([-*+•]|\d{1,9}[.)])[ \t]+(.*)$");
    let task = rx(&TASK, r"^\[([ xX])\](?:[ \t]+(.*))?$");
    let heading = rx(&HEADING, r"^ {0,3}(#{1,6})[ \t]+(.*?)(?:[ \t]+#+)?[ \t]*$");
    let rule = rx(&RULE, r"^ {0,3}(?:(?:-[ \t]*){3,}|(?:\*[ \t]*){3,}|(?:_[ \t]*){3,})$");
    let delimiter = rx(&DELIMITER, r"^[ \t]*\|?[ \t]*:?-+:?[ \t]*(?:\|[ \t]*:?-+:?[ \t]*)*\|?[ \t]*$");
    let fence = rx(&FENCE, r"^[ \t]*(`{3,}|~{3,})[ \t]*([^`\s]*)[ \t]*$");
    let text = source.replace("\r\n", "\n");
    let rows: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<Block> = Vec::new();
    let mut indents: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        let row = rows[i];
        if let Some(rest) = row.trim_start().strip_prefix("```") {
            if let Some(code) = rest.strip_suffix("```").filter(|c| !c.trim().is_empty() && !c.contains('`')) {
                out.push(Block::Code(None, code.to_string()));
                i += 1;
                continue;
            }
        }
        if let Some(c) = fence.captures(row) {
            let mark = c[1].to_string();
            let closing = |line: &str| {
                let line = line.trim();
                line.len() >= mark.len() && line.chars().all(|ch| Some(ch) == mark.chars().next())
            };
            if let Some(end) = (i + 1..rows.len()).find(|&j| closing(rows[j])) {
                let lang = c[2].trim().to_string();
                out.push(Block::Code(Some(lang).filter(|l| !l.is_empty()), rows[i + 1..end].join("\n")));
                indents.clear();
                i = end + 1;
                continue;
            }
        }
        if row.contains('|') && rows.get(i + 1).map(|next| next.contains('|') && delimiter.is_match(next)).unwrap_or(false) {
            let header = cells(row);
            let aligns: Vec<Align> = cells(rows[i + 1]).iter().map(|cell| align(cell)).collect();
            if header.len() == aligns.len() {
                let mut body = Vec::new();
                let mut j = i + 2;
                while j < rows.len() && rows[j].contains('|') && !rows[j].trim().is_empty() {
                    let mut row_cells: Vec<Vec<Inline>> = cells(rows[j]).iter().map(|cell| inlines(cell)).collect();
                    row_cells.resize(header.len(), Vec::new());
                    body.push(row_cells);
                    j += 1;
                }
                out.push(Block::Table(aligns, header.iter().map(|cell| inlines(cell)).collect(), body));
                indents.clear();
                i = j;
                continue;
            }
        }
        i += 1;
        if row.trim().is_empty() {
            indents.clear();
            out.push(Block::Blank);
            continue;
        }
        if rule.is_match(row) {
            indents.clear();
            out.push(Block::Rule);
            continue;
        }
        if let Some(c) = heading.captures(row) {
            indents.clear();
            out.push(Block::Heading(c[1].len(), inlines(&c[2])));
            continue;
        }
        if let Some(c) = item.captures(row) {
            let width: usize = c[1].chars().map(|ch| if ch == '\t' { 4 } else { 1 }).sum();
            let marker = &c[2];
            let rest = &c[3];
            let continuing = matches!(out.last(), Some(Block::List(_)));
            if !continuing || indents.is_empty() {
                indents = vec![width];
            } else {
                while indents.len() > 1 && width < *indents.last().unwrap_or(&0) {
                    indents.pop();
                }
                if width > *indents.last().unwrap_or(&0) {
                    indents.push(width);
                }
            }
            let depth = indents.len() - 1;
            let (kind, content) = match task.captures(rest) {
                Some(t) => (Kind::Task(&t[1] != " "), t.get(2).map(|m| m.as_str()).unwrap_or("").to_string()),
                None if marker.starts_with(|ch: char| ch.is_ascii_digit()) => (Kind::Ordered(marker.trim_end_matches(['.', ')']).parse().unwrap_or(1)), rest.to_string()),
                None => (Kind::Bullet, rest.to_string()),
            };
            let entry = Item { depth, kind, inlines: inlines(&content) };
            match out.last_mut() {
                Some(Block::List(items)) => items.push(entry),
                _ => out.push(Block::List(vec![entry])),
            }
            continue;
        }
        indents.clear();
        if let Some(quote) = row.trim_start().strip_prefix('>') {
            let line = inlines(&line_text(quote.strip_prefix(' ').unwrap_or(quote)));
            match out.last_mut() {
                Some(Block::Quote(lines)) => lines.push(line),
                _ => out.push(Block::Quote(vec![line])),
            }
            continue;
        }
        let line = inlines(&line_text(row));
        match out.last_mut() {
            Some(Block::Text(lines)) => lines.push(line),
            _ => out.push(Block::Text(vec![line])),
        }
    }
    while out.last() == Some(&Block::Blank) {
        out.pop();
    }
    out
}

fn line_text(row: &str) -> String {
    if row.ends_with("  ") {
        return row.trim_end().to_string();
    }
    match row.strip_suffix('\\') {
        Some(rest) if !rest.ends_with('\\') => rest.to_string(),
        _ => row.to_string(),
    }
}

fn cells(row: &str) -> Vec<String> {
    let trimmed = row.trim();
    let trimmed = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let trimmed = if trimmed.ends_with('|') && !trimmed.ends_with("\\|") { &trimmed[..trimmed.len() - 1] } else { trimmed };
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut chars = trimmed.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cell).trim().to_string()),
            _ => cell.push(c),
        }
    }
    out.push(cell.trim().to_string());
    out
}

fn align(cell: &str) -> Align {
    match (cell.starts_with(':'), cell.ends_with(':')) {
        (true, true) => Align::Center,
        (true, false) => Align::Left,
        (false, true) => Align::Right,
        _ => Align::Default,
    }
}

fn inlines(text: &str) -> Vec<Inline> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let prev = if i == 0 { None } else { Some(chars[i - 1]) };
        if c == '\\' && chars.get(i + 1).map(|n| n.is_ascii_punctuation()).unwrap_or(false) {
            plain.push(chars[i + 1]);
            i += 2;
            continue;
        }
        let token = match c {
            '`' => code_span(&chars, i),
            '!' if chars.get(i + 1) == Some(&'[') => image(&chars, i),
            '[' => link(&chars, i),
            '<' => autolink(&chars, i).or_else(|| underline(&chars, i)),
            'h' if !prev.map(is_word).unwrap_or(false) && (starts(&chars, i, "https://") || starts(&chars, i, "http://")) => {
                let end = (i..chars.len()).find(|&j| chars[j].is_whitespace() || matches!(chars[j], '<' | '>' | '"')).unwrap_or(chars.len());
                let url = trim_url(&collect(&chars, i, end));
                let next = i + url.chars().count();
                Some((Inline::Link(url.clone(), vec![Inline::Text(url)]), next))
            }
            '*' if starts(&chars, i, "**") => span(&chars, i, "**").map(|(inner, next)| (Inline::Bold(inlines(&inner)), next)),
            '*' => span(&chars, i, "*").map(|(inner, next)| (Inline::Italic(inlines(&inner)), next)),
            '_' if starts(&chars, i, "__") => bounded(&chars, i, prev, "__").map(|(inner, next)| (Inline::Bold(inlines(&inner)), next)),
            '_' => bounded(&chars, i, prev, "_").map(|(inner, next)| (Inline::Italic(inlines(&inner)), next)),
            '~' if starts(&chars, i, "~~") => span(&chars, i, "~~").map(|(inner, next)| (Inline::Strike(inlines(&inner)), next)),
            '~' => bounded(&chars, i, prev, "~").map(|(inner, next)| (Inline::Strike(inlines(&inner)), next)),
            _ => None,
        };
        match token {
            Some((inline, next)) => {
                if !plain.is_empty() {
                    out.push(Inline::Text(std::mem::take(&mut plain)));
                }
                out.push(inline);
                i = next;
            }
            None => {
                plain.push(c);
                i += 1;
            }
        }
    }
    if !plain.is_empty() {
        out.push(Inline::Text(plain));
    }
    out
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '/' || c == '('
}

fn starts(chars: &[char], at: usize, pattern: &str) -> bool {
    pattern.chars().enumerate().all(|(k, p)| chars.get(at + k) == Some(&p))
}

fn find(chars: &[char], from: usize, pattern: &str) -> Option<usize> {
    (from..chars.len()).find(|&j| starts(chars, j, pattern))
}

fn collect(chars: &[char], from: usize, to: usize) -> String {
    chars[from..to].iter().collect()
}

fn span(chars: &[char], at: usize, delimiter: &str) -> Option<(String, usize)> {
    let width = delimiter.chars().count();
    let mark = delimiter.chars().next()?;
    let open = at + width;
    if chars.get(open).map(|c| c.is_whitespace()).unwrap_or(true) {
        return None;
    }
    let mut from = open + 1;
    while let Some(found) = find(chars, from, delimiter) {
        let mut close = found;
        if width > 1 {
            while chars.get(close + width) == Some(&mark) {
                close += 1;
            }
        }
        let after = chars.get(close + width).copied();
        let doubled = width == 1 && after == Some(mark);
        if close > open && !chars[close - 1].is_whitespace() && !doubled {
            return Some((collect(chars, open, close), close + width));
        }
        from = close + width;
    }
    None
}

fn bounded(chars: &[char], at: usize, prev: Option<char>, delimiter: &str) -> Option<(String, usize)> {
    if prev.map(char::is_alphanumeric).unwrap_or(false) {
        return None;
    }
    span(chars, at, delimiter).filter(|(_, next)| !chars.get(*next).map(|c| c.is_alphanumeric()).unwrap_or(false))
}

fn code_span(chars: &[char], at: usize) -> Option<(Inline, usize)> {
    let width = (at..chars.len()).take_while(|&j| chars[j] == '`').count();
    let mut from = at + width;
    while from < chars.len() {
        let start = (from..chars.len()).find(|&j| chars[j] == '`')?;
        let run = (start..chars.len()).take_while(|&j| chars[j] == '`').count();
        if run == width {
            let code = collect(chars, at + width, start);
            if code.is_empty() {
                return None;
            }
            let code = match code.strip_prefix(' ').and_then(|c| c.strip_suffix(' ')) {
                Some(inner) if !inner.trim().is_empty() => inner.to_string(),
                _ => code,
            };
            return Some((Inline::Code(code), start + run));
        }
        from = start + run;
    }
    None
}

fn destination(chars: &[char], open: usize) -> Option<(String, usize)> {
    let mut depth = 0;
    let mut end = None;
    for (j, &c) in chars.iter().enumerate().skip(open) {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => {
                end = Some(j);
                break;
            }
            ')' => depth -= 1,
            _ => {}
        }
    }
    let end = end?;
    let inside = collect(chars, open, end);
    let href = inside.trim().split_whitespace().next().unwrap_or("").trim_start_matches('<').trim_end_matches('>').to_string();
    Some((href, end + 1))
}

fn label_end(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0;
    let mut j = open;
    while j < chars.len() {
        match chars[j] {
            '\\' => j += 1,
            '[' => depth += 1,
            ']' if depth == 0 => return Some(j),
            ']' => depth -= 1,
            _ => {}
        }
        j += 1;
    }
    None
}

fn link(chars: &[char], at: usize) -> Option<(Inline, usize)> {
    let close = label_end(chars, at + 1)?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let label = collect(chars, at + 1, close);
    let (href, next) = destination(chars, close + 2)?;
    if label.is_empty() {
        return None;
    }
    if let Some(id) = href.strip_prefix("mention:") {
        let name = label.strip_prefix('@')?;
        return Some((Inline::Mention(id.to_string(), name.to_string()), next));
    }
    if let Some(id) = href.strip_prefix("channel:") {
        let name = label.strip_prefix('#')?;
        return Some((Inline::Channel(id.to_string(), name.to_string()), next));
    }
    if !(href.starts_with("https://") || href.starts_with("http://") || href.starts_with("mailto:")) {
        return None;
    }
    Some((Inline::Link(href, unlink(inlines(&label))), next))
}

fn image(chars: &[char], at: usize) -> Option<(Inline, usize)> {
    let close = label_end(chars, at + 2)?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let alt = collect(chars, at + 2, close);
    let (src, next) = destination(chars, close + 2)?;
    if !(src.starts_with("https://") || src.starts_with("http://")) {
        return None;
    }
    if let Some(name) = alt.strip_prefix(':').and_then(|a| a.strip_suffix(':')).filter(|n| !n.is_empty() && !n.contains(char::is_whitespace)) {
        return Some((Inline::Emoji(name.to_string(), src), next));
    }
    Some((Inline::Image(src, alt), next))
}

fn autolink(chars: &[char], at: usize) -> Option<(Inline, usize)> {
    let end = (at + 1..chars.len()).find(|&j| chars[j] == '>' || chars[j] == '<' || chars[j].is_whitespace())?;
    if chars[end] != '>' {
        return None;
    }
    let inner = collect(chars, at + 1, end);
    if inner.starts_with("https://") || inner.starts_with("http://") || inner.starts_with("mailto:") {
        let label = inner.strip_prefix("mailto:").unwrap_or(&inner).to_string();
        return Some((Inline::Link(inner.clone(), vec![Inline::Text(label)]), end + 1));
    }
    let (user, host) = inner.split_once('@')?;
    if user.is_empty() || !host.contains('.') || host.starts_with('.') || host.ends_with('.') {
        return None;
    }
    Some((Inline::Link(format!("mailto:{}", inner), vec![Inline::Text(inner)]), end + 1))
}

fn underline(chars: &[char], at: usize) -> Option<(Inline, usize)> {
    if !starts(chars, at, "<u>") {
        return None;
    }
    let close = find(chars, at + 3, "</u>")?;
    if close == at + 3 {
        return None;
    }
    Some((Inline::Underline(inlines(&collect(chars, at + 3, close))), close + 4))
}

fn unlink(parts: Vec<Inline>) -> Vec<Inline> {
    parts
        .into_iter()
        .flat_map(|part| match part {
            Inline::Link(_, label) => unlink(label),
            Inline::Bold(inner) => vec![Inline::Bold(unlink(inner))],
            Inline::Italic(inner) => vec![Inline::Italic(unlink(inner))],
            Inline::Underline(inner) => vec![Inline::Underline(unlink(inner))],
            Inline::Strike(inner) => vec![Inline::Strike(unlink(inner))],
            other => vec![other],
        })
        .collect()
}

fn unbold(parts: Vec<Inline>) -> Vec<Inline> {
    parts.into_iter().flat_map(|part| match part {
        Inline::Bold(inner) => unbold(inner),
        other => vec![other],
    }).collect()
}

fn trim_url(url: &str) -> String {
    let mut out = url.to_string();
    loop {
        let last = match out.chars().last() {
            Some(c) => c,
            None => return out,
        };
        let unbalanced = |open: char, close: char| out.matches(open).count() < out.matches(close).count();
        if matches!(last, '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"') || (last == ')' && unbalanced('(', ')')) || (last == ']' && unbalanced('[', ']')) {
            out.pop();
        } else {
            return out;
        }
    }
}

fn html_document(source: &str, target: Target) -> (String, Vec<Mention>) {
    let support = target.support();
    let mut mentions = Vec::new();
    let mut out = String::new();
    for block in blocks(source) {
        match block {
            Block::Blank => {}
            Block::Text(lines) => out.push_str(&format!("<p>{}</p>", html_lines(&lines, target, &mut mentions))),
            Block::Heading(level, parts) if level <= support.headings => out.push_str(&format!("<h{}>{}</h{}>", level, html_inline(&parts, target, &mut mentions), level)),
            Block::Heading(_, parts) => out.push_str(&format!("<p><strong>{}</strong></p>", html_inline(&parts, target, &mut mentions))),
            Block::Quote(lines) => out.push_str(&format!("<blockquote>{}</blockquote>", html_lines(&lines, target, &mut mentions))),
            Block::List(items) => out.push_str(&html_list(&items, target, &mut mentions)),
            Block::Code(Some(lang), code) if target == Target::Teams => out.push_str(&format!("<codeblock class=\"{}\"><code>{}</code></codeblock>", html_escape(&lang), html_escape(&code))),
            Block::Code(_, code) => out.push_str(&format!("<pre><code>{}</code></pre>", html_escape(&code))),
            Block::Table(aligns, header, rows) if support.tables => out.push_str(&html_table(&aligns, &header, &rows, target, &mut mentions)),
            Block::Table(aligns, header, rows) => out.push_str(&format!("<pre>{}</pre>", html_escape(&text_table(&aligns, &header, &rows)))),
            Block::Rule => out.push_str("<hr>"),
        }
    }
    (out, mentions)
}

fn html_lines(lines: &[Vec<Inline>], target: Target, mentions: &mut Vec<Mention>) -> String {
    lines.iter().map(|line| html_inline(line, target, mentions)).collect::<Vec<_>>().join("<br>")
}

fn html_list(items: &[Item], target: Target, mentions: &mut Vec<Mention>) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < items.len() {
        let depth = items[i].depth;
        let ordered = matches!(items[i].kind, Kind::Ordered(_));
        match items[i].kind {
            Kind::Ordered(start) if start != 1 => out.push_str(&format!("<ol start=\"{}\">", start)),
            Kind::Ordered(_) => out.push_str("<ol>"),
            _ => out.push_str("<ul>"),
        }
        while i < items.len() && items[i].depth == depth && matches!(items[i].kind, Kind::Ordered(_)) == ordered {
            let box_mark = match items[i].kind {
                Kind::Task(true) => "☑ ",
                Kind::Task(false) => "☐ ",
                _ => "",
            };
            out.push_str(&format!("<li>{}{}", box_mark, html_inline(&items[i].inlines, target, mentions)));
            i += 1;
            let start = i;
            while i < items.len() && items[i].depth > depth {
                i += 1;
            }
            if start < i {
                out.push_str(&html_list(&items[start..i], target, mentions));
            }
            out.push_str("</li>");
        }
        out.push_str(if ordered { "</ol>" } else { "</ul>" });
    }
    out
}

fn html_table(aligns: &[Align], header: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>], target: Target, mentions: &mut Vec<Mention>) -> String {
    let cell = |tag: &str, align: Align, content: String| {
        let side = match align {
            Align::Left => "text-align:left",
            Align::Center => "text-align:center",
            Align::Right => "text-align:right",
            Align::Default => "",
        };
        let border = if target == Target::Mail { "border:1px solid #d0d7de;padding:4px 8px" } else { "" };
        let style = [border, side].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(";");
        if style.is_empty() {
            format!("<{}>{}</{}>", tag, content, tag)
        } else {
            format!("<{} style=\"{}\">{}</{}>", tag, style, content, tag)
        }
    };
    let mut out = String::from(if target == Target::Mail { "<table style=\"border-collapse:collapse\">" } else { "<table>" });
    out.push_str("<thead><tr>");
    for (k, head) in header.iter().enumerate() {
        let content = html_inline(head, target, mentions);
        out.push_str(&cell("th", aligns.get(k).copied().unwrap_or(Align::Default), content));
    }
    out.push_str("</tr></thead><tbody>");
    for row in rows {
        out.push_str("<tr>");
        for (k, value) in row.iter().enumerate() {
            let content = html_inline(value, target, mentions);
            out.push_str(&cell("td", aligns.get(k).copied().unwrap_or(Align::Default), content));
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table>");
    out
}

fn html_inline(parts: &[Inline], target: Target, mentions: &mut Vec<Mention>) -> String {
    let support = target.support();
    let mut out = String::new();
    for part in parts {
        match part {
            Inline::Text(text) if support.shortcodes => out.push_str(&html_escape(&unicode_text(text))),
            Inline::Text(text) => out.push_str(&html_escape(text)),
            Inline::Bold(inner) => out.push_str(&format!("<strong>{}</strong>", html_inline(inner, target, mentions))),
            Inline::Italic(inner) => out.push_str(&format!("<em>{}</em>", html_inline(inner, target, mentions))),
            Inline::Underline(inner) if support.underline => out.push_str(&format!("<u>{}</u>", html_inline(inner, target, mentions))),
            Inline::Underline(inner) => out.push_str(&html_inline(inner, target, mentions)),
            Inline::Strike(inner) => out.push_str(&format!("<s>{}</s>", html_inline(inner, target, mentions))),
            Inline::Code(code) => out.push_str(&format!("<code>{}</code>", html_escape(code))),
            Inline::Link(href, label) => out.push_str(&format!("<a href=\"{}\">{}</a>", html_escape(href), html_inline(label, target, mentions))),
            Inline::Emoji(name, src) if support.images => out.push_str(&format!("<img src=\"{}\" alt=\":{}:\" title=\":{}:\" style=\"height:1.25em;vertical-align:middle\">", html_escape(src), html_escape(name), html_escape(name))),
            Inline::Emoji(name, src) => out.push_str(&format!("<a href=\"{}\">:{}:</a>", html_escape(src), html_escape(name))),
            Inline::Image(src, alt) if support.images => out.push_str(&format!("<img src=\"{}\" alt=\"{}\">", html_escape(src), html_escape(alt))),
            Inline::Image(src, alt) => out.push_str(&format!("<a href=\"{}\">{}</a>", html_escape(src), html_escape(if alt.is_empty() { src } else { alt }))),
            Inline::Mention(id, name) if is_broadcast(id) => out.push_str(&format!("@{}", html_escape(name.trim_start_matches('@')))),
            Inline::Mention(id, name) if support.mentions && !id.is_empty() => {
                out.push_str(&format!("<at id=\"{}\">{}</at>", mentions.len(), html_escape(name)));
                mentions.push(Mention { id: id.clone(), name: name.clone() });
            }
            Inline::Mention(_, name) => out.push_str(&format!("@{}", html_escape(name))),
            Inline::Channel(_, name) => out.push_str(&format!("#{}", html_escape(name))),
        }
    }
    out
}

fn lines(source: &str, target: Target) -> String {
    let mut out: Vec<String> = Vec::new();
    for block in blocks(source) {
        match block {
            Block::Blank => out.push(String::new()),
            Block::Text(lines) => out.extend(lines.iter().map(|line| inline(line, target))),
            Block::Heading(_, parts) if target == Target::Plain => out.push(inline(&parts, target)),
            Block::Heading(_, parts) => out.push(format!("*{}*", inline(&unbold(parts), target))),
            Block::Quote(lines) => out.extend(lines.iter().map(|line| format!("> {}", inline(line, target)))),
            Block::List(items) => out.extend(text_list(&items, target)),
            Block::Code(_, code) => match target {
                Target::Slack => out.push(format!("```\n{}\n```", slack_escape(&code))),
                Target::GoogleChat => out.push(format!("```\n{}\n```", code)),
                _ => out.push(code),
            },
            Block::Table(aligns, header, rows) => {
                let table = text_table(&aligns, &header, &rows);
                match target {
                    Target::Slack => out.push(format!("```\n{}\n```", slack_escape(&table))),
                    Target::GoogleChat => out.push(format!("```\n{}\n```", table)),
                    _ => out.push(table),
                }
            }
            Block::Rule if target == Target::Plain => out.push("---".to_string()),
            Block::Rule => out.push("──────────".to_string()),
        }
    }
    out.join("\n")
}

fn text_list(items: &[Item], target: Target) -> Vec<String> {
    let mut counters: Vec<u32> = Vec::new();
    let unit = if target == Target::Slack { "    " } else { "  " };
    items
        .iter()
        .map(|item| {
            counters.resize(item.depth + 1, 0);
            let lead = unit.repeat(item.depth);
            let text = inline(&item.inlines, target);
            match item.kind {
                Kind::Ordered(start) => {
                    counters[item.depth] = if counters[item.depth] == 0 { start } else { counters[item.depth] + 1 };
                    format!("{}{}. {}", lead, counters[item.depth], text)
                }
                kind => {
                    counters[item.depth] = 0;
                    let bullet = if target == Target::Slack { ["•", "◦", "▪"][item.depth % 3] } else { "-" };
                    match (kind, target) {
                        (Kind::Task(done), Target::Slack) => format!("{}{} {}", lead, if done { "☑" } else { "☐" }, text),
                        (Kind::Task(done), _) => format!("{}{} [{}] {}", lead, bullet, if done { "x" } else { " " }, text),
                        _ => format!("{}{} {}", lead, bullet, text),
                    }
                }
            }
        })
        .collect()
}

fn text_table(aligns: &[Align], header: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) -> String {
    let grid: Vec<Vec<String>> = std::iter::once(header).chain(rows.iter().map(|r| r.as_slice())).map(|row| row.iter().map(|cell| inline(cell, Target::Plain)).collect()).collect();
    let columns = header.len();
    let widths: Vec<usize> = (0..columns).map(|k| grid.iter().map(|row| row.get(k).map(|c| c.chars().count()).unwrap_or(0)).max().unwrap_or(0).max(3)).collect();
    let pad = |text: &str, k: usize| {
        let room = widths[k].saturating_sub(text.chars().count());
        match aligns.get(k).copied().unwrap_or(Align::Default) {
            Align::Right => format!("{}{}", " ".repeat(room), text),
            Align::Center => format!("{}{}{}", " ".repeat(room / 2), text, " ".repeat(room - room / 2)),
            _ => format!("{}{}", text, " ".repeat(room)),
        }
    };
    let render = |row: &Vec<String>| (0..columns).map(|k| pad(row.get(k).map(String::as_str).unwrap_or(""), k)).collect::<Vec<_>>().join(" | ").trim_end().to_string();
    let mut out = vec![render(&grid[0]), widths.iter().map(|w| "-".repeat(*w)).collect::<Vec<_>>().join("-+-")];
    out.extend(grid[1..].iter().map(render));
    out.join("\n")
}

fn inline(parts: &[Inline], target: Target) -> String {
    let support = target.support();
    let wrap = |mark: &str, inner: &[Inline]| if target == Target::Plain { inline(inner, target) } else { format!("{}{}{}", mark, inline(inner, target), mark) };
    parts
        .iter()
        .map(|part| match part {
            Inline::Text(text) if target == Target::Slack => slack_escape(text),
            Inline::Text(text) if support.shortcodes => unicode_text(text),
            Inline::Text(text) => text.clone(),
            Inline::Bold(inner) => wrap("*", inner),
            Inline::Italic(inner) => wrap("_", inner),
            Inline::Underline(inner) => inline(inner, target),
            Inline::Strike(inner) => wrap("~", inner),
            Inline::Code(code) if target == Target::Slack => format!("`{}`", slack_escape(code)),
            Inline::Code(code) if target == Target::GoogleChat => format!("`{}`", code),
            Inline::Code(code) => code.clone(),
            Inline::Link(href, label) => {
                let text = inline(label, Target::Plain);
                let bare = text == *href || href.strip_prefix("mailto:") == Some(text.as_str());
                match target {
                    Target::Plain if bare => text,
                    Target::Plain => format!("{} ({})", text, href),
                    _ if bare => format!("<{}>", href),
                    Target::Slack => format!("<{}|{}>", href, slack_escape(&text.replace('|', "/"))),
                    _ => format!("<{}|{}>", href, text.replace('|', "/")),
                }
            }
            Inline::Image(src, alt) => match target {
                Target::Plain if alt.is_empty() => src.clone(),
                Target::Plain => format!("{} ({})", alt, src),
                _ if alt.is_empty() || support.images => format!("<{}>", src),
                Target::Slack => format!("<{}|{}>", src, slack_escape(&alt.replace('|', "/"))),
                _ => format!("<{}|{}>", src, alt.replace('|', "/")),
            },
            Inline::Emoji(name, _) if target == Target::Plain => format!(":{}:", name),
            Inline::Emoji(name, src) => format!("<{}|:{}:>", src, name),
            Inline::Mention(id, _) if target == Target::Slack && matches!(id.as_str(), "here" | "channel" | "everyone") => format!("<!{}>", id),
            Inline::Mention(id, _) if target == Target::Slack && id.starts_with("group:") => format!("<!subteam^{}>", id.trim_start_matches("group:")),
            Inline::Mention(id, _) if support.mentions && target == Target::Slack && !id.is_empty() => format!("<@{}>", id),
            Inline::Mention(_, name) if target == Target::Slack => format!("@{}", slack_escape(name)),
            Inline::Mention(_, name) => format!("@{}", name),
            Inline::Channel(id, _) if target == Target::Slack && !id.is_empty() => format!("<#{}>", id),
            Inline::Channel(_, name) => format!("#{}", name),
        })
        .collect()
}

fn slack_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORMATTED: &str = "**Agenda** for _fredag_, ~onsdag~ er avlyst & `Cargo.lock` <ok>";
    const QUOTE: &str = "> Husk å oppdatere\n> før fredag";
    const LISTS: &str = "- gå gjennom PR\n- bestem leverandør\n\n1. demo\n2. retro";
    const FENCE: &str = "Før\n```rust\nfn main() {\n    println!(\"<hei>\");\n}\n```\nEtter";
    const LINKS: &str = "Se [agendaen](https://northwind.example/agenda) og https://northwind.example/x.";
    const SNAKE: &str = "some_variable_name og 2 * 3 * 4";
    const LINES: &str = "linje en\nlinje to\n\nny avsnitt";
    const HEADINGS: &str = "# Plan\n## Uke **42**\n###### Detalj\n#hashtag";
    const NESTED: &str = "- topp\n  - under\n    - dypest\n- tilbake\n1. en\n   1. en.en\n2. to";
    const TASKS: &str = "- [ ] skriv\n- [x] test";
    const TABLE: &str = "| Navn | Antall |\n| :--- | ---: |\n| Kari | 3 |\n| Ola & co | **12** |";
    const RULE: &str = "over\n\n---\nunder";
    const IMAGE: &str = "Se ![diagram](https://northwind.example/d.png) her";
    const AUTOLINK: &str = "<https://northwind.example> og <kari@northwind.example>";
    const MENTION: &str = "Hei [@Kari Nordmann](mention:U1) og [@Ola](mention:)";
    const BREAKS: &str = "linje en\\\nlinje to  \nlinje tre";
    const ESCAPES: &str = "\\*ikke fet\\* og \\# ikke tittel";
    const STYLES: &str = "***begge*** og <u>under</u> og __fet__ og ``kode med ` inni``";

    #[test]
    fn html_formats_inline_marks() {
        assert_eq!(html(FORMATTED), "<p><strong>Agenda</strong> for <em>fredag</em>, <s>onsdag</s> er avlyst &amp; <code>Cargo.lock</code> &lt;ok&gt;</p>");
        assert_eq!(html(STYLES), "<p><strong><em>begge</em></strong> og <u>under</u> og <strong>fet</strong> og <code>kode med ` inni</code></p>");
        assert_eq!(html(ESCAPES), "<p>*ikke fet* og # ikke tittel</p>");
    }

    #[test]
    fn html_formats_quote_lists_and_code() {
        assert_eq!(html(QUOTE), "<blockquote>Husk å oppdatere<br>før fredag</blockquote>");
        assert_eq!(html(LISTS), "<ul><li>gå gjennom PR</li><li>bestem leverandør</li></ul><ol><li>demo</li><li>retro</li></ol>");
        assert_eq!(html(FENCE), "<p>Før</p><pre><code>fn main() {\n    println!(&quot;&lt;hei&gt;&quot;);\n}</code></pre><p>Etter</p>");
        assert_eq!(html("~~~\nraw\n~~~"), "<pre><code>raw</code></pre>");
    }

    #[test]
    fn html_formats_links_and_keeps_plain_words() {
        assert_eq!(html(LINKS), "<p>Se <a href=\"https://northwind.example/agenda\">agendaen</a> og <a href=\"https://northwind.example/x\">https://northwind.example/x</a>.</p>");
        assert_eq!(html(SNAKE), "<p>some_variable_name og 2 * 3 * 4</p>");
        assert_eq!(html(LINES), "<p>linje en<br>linje to</p><p>ny avsnitt</p>");
        assert_eq!(html(AUTOLINK), "<p><a href=\"https://northwind.example\">https://northwind.example</a> og <a href=\"mailto:kari@northwind.example\">kari@northwind.example</a></p>");
        assert_eq!(html("[x](javascript:alert(1))"), "<p>[x](javascript:alert(1))</p>");
        assert_eq!(html("[**fet lenke**](https://x.no)"), "<p><a href=\"https://x.no\"><strong>fet lenke</strong></a></p>");
    }

    #[test]
    fn html_formats_headings_tables_tasks_rules_and_images() {
        assert_eq!(html(HEADINGS), "<h1>Plan</h1><h2>Uke <strong>42</strong></h2><h6>Detalj</h6><p>#hashtag</p>");
        assert_eq!(html(NESTED), "<ul><li>topp<ul><li>under<ul><li>dypest</li></ul></li></ul></li><li>tilbake</li></ul><ol><li>en<ol><li>en.en</li></ol></li><li>to</li></ol>");
        assert_eq!(html(TASKS), "<ul><li>☐ skriv</li><li>☑ test</li></ul>");
        assert_eq!(
            html(TABLE),
            "<table style=\"border-collapse:collapse\"><thead><tr><th style=\"border:1px solid #d0d7de;padding:4px 8px;text-align:left\">Navn</th><th style=\"border:1px solid #d0d7de;padding:4px 8px;text-align:right\">Antall</th></tr></thead><tbody><tr><td style=\"border:1px solid #d0d7de;padding:4px 8px;text-align:left\">Kari</td><td style=\"border:1px solid #d0d7de;padding:4px 8px;text-align:right\">3</td></tr><tr><td style=\"border:1px solid #d0d7de;padding:4px 8px;text-align:left\">Ola &amp; co</td><td style=\"border:1px solid #d0d7de;padding:4px 8px;text-align:right\"><strong>12</strong></td></tr></tbody></table>"
        );
        assert_eq!(html(RULE), "<p>over</p><hr><p>under</p>");
        assert_eq!(html(IMAGE), "<p>Se <img src=\"https://northwind.example/d.png\" alt=\"diagram\"> her</p>");
        assert_eq!(html(BREAKS), "<p>linje en<br>linje to<br>linje tre</p>");
        assert_eq!(html("Takk :tada:"), "<p>Takk 🎉</p>");
    }

    #[test]
    fn html_mail_writes_mentions_as_names() {
        assert_eq!(html(MENTION), "<p>Hei @Kari Nordmann og @Ola</p>");
    }

    #[test]
    fn teams_writes_mention_entities_and_supported_tags() {
        let (body, mentions) = teams(MENTION);
        assert_eq!(body, "<p>Hei <at id=\"0\">Kari Nordmann</at> og @Ola</p>");
        assert_eq!(mentions, vec![Mention { id: "U1".to_string(), name: "Kari Nordmann".to_string() }]);
        assert_eq!(teams(HEADINGS).0, "<h1>Plan</h1><h2>Uke <strong>42</strong></h2><p><strong>Detalj</strong></p><p>#hashtag</p>");
        assert_eq!(teams(IMAGE).0, "<p>Se <a href=\"https://northwind.example/d.png\">diagram</a> her</p>");
        assert_eq!(teams(FENCE).0, "<p>Før</p><codeblock class=\"rust\"><code>fn main() {\n    println!(&quot;&lt;hei&gt;&quot;);\n}</code></codeblock><p>Etter</p>");
        assert_eq!(teams(TABLE).0, "<table><thead><tr><th style=\"text-align:left\">Navn</th><th style=\"text-align:right\">Antall</th></tr></thead><tbody><tr><td style=\"text-align:left\">Kari</td><td style=\"text-align:right\">3</td></tr><tr><td style=\"text-align:left\">Ola &amp; co</td><td style=\"text-align:right\"><strong>12</strong></td></tr></tbody></table>");
        assert_eq!(teams(FORMATTED).0, html(FORMATTED));
        assert_eq!(teams("<script>x</script> <u>u</u>").0, "<p>&lt;script&gt;x&lt;/script&gt; <u>u</u></p>");
        let (_, many) = teams("[@A](mention:1) [@A](mention:1)");
        assert_eq!(many.len(), 2);
    }

    #[test]
    fn slack_formats_mrkdwn() {
        assert_eq!(slack(FORMATTED), "*Agenda* for _fredag_, ~onsdag~ er avlyst &amp; `Cargo.lock` &lt;ok&gt;");
        assert_eq!(slack(QUOTE), "> Husk å oppdatere\n> før fredag");
        assert_eq!(slack(LISTS), "• gå gjennom PR\n• bestem leverandør\n\n1. demo\n2. retro");
        assert_eq!(slack(FENCE), "Før\n```\nfn main() {\n    println!(\"&lt;hei&gt;\");\n}\n```\nEtter");
        assert_eq!(slack(LINKS), "Se <https://northwind.example/agenda|agendaen> og <https://northwind.example/x>.");
        assert_eq!(slack(SNAKE), SNAKE);
        assert_eq!(slack(LINES), LINES);
    }

    #[test]
    fn slack_degrades_what_mrkdwn_lacks() {
        assert_eq!(slack(HEADINGS), "*Plan*\n*Uke 42*\n*Detalj*\n#hashtag");
        assert_eq!(slack(NESTED), "• topp\n    ◦ under\n        ▪ dypest\n• tilbake\n1. en\n    1. en.en\n2. to");
        assert_eq!(slack(TASKS), "☐ skriv\n☑ test");
        assert_eq!(slack(TABLE), "```\nNavn     | Antall\n---------+-------\nKari     |      3\nOla &amp; co |     12\n```");
        assert_eq!(slack(RULE), "over\n\n──────────\nunder");
        assert_eq!(slack(IMAGE), "Se <https://northwind.example/d.png|diagram> her");
        assert_eq!(slack(AUTOLINK), "<https://northwind.example> og <mailto:kari@northwind.example>");
        assert_eq!(slack(MENTION), "Hei <@U1> og @Ola");
        assert_eq!(slack(STYLES), "*_begge_* og under og *fet* og `kode med ` inni`");
        assert_eq!(slack("Takk :tada: <@U2>"), "Takk :tada: &lt;@U2&gt;");
        assert_eq!(slack("[a|b](https://x.no)"), "<https://x.no|a/b>");
    }

    #[test]
    fn google_chat_formats_its_subset() {
        assert_eq!(google_chat(FORMATTED), "*Agenda* for _fredag_, ~onsdag~ er avlyst & `Cargo.lock` <ok>");
        assert_eq!(google_chat(QUOTE), QUOTE);
        assert_eq!(google_chat(LISTS), LISTS);
        assert_eq!(google_chat(FENCE), "Før\n```\nfn main() {\n    println!(\"<hei>\");\n}\n```\nEtter");
        assert_eq!(google_chat(LINKS), "Se <https://northwind.example/agenda|agendaen> og <https://northwind.example/x>.");
        assert_eq!(google_chat(SNAKE), SNAKE);
        assert_eq!(google_chat(LINES), LINES);
        assert_eq!(google_chat(MENTION), "Hei @Kari Nordmann og @Ola");
    }

    #[test]
    fn plain_strips_marks_for_the_text_part() {
        assert_eq!(plain(FORMATTED), "Agenda for fredag, onsdag er avlyst & Cargo.lock <ok>");
        assert_eq!(plain(QUOTE), QUOTE);
        assert_eq!(plain(LISTS), LISTS);
        assert_eq!(plain(FENCE), "Før\nfn main() {\n    println!(\"<hei>\");\n}\nEtter");
        assert_eq!(plain(LINKS), "Se agendaen (https://northwind.example/agenda) og https://northwind.example/x.");
        assert_eq!(plain(SNAKE), SNAKE);
        assert_eq!(plain(LINES), LINES);
    }

    #[test]
    fn plain_writes_every_block_as_readable_text() {
        assert_eq!(plain(HEADINGS), "Plan\nUke 42\nDetalj\n#hashtag");
        assert_eq!(plain(NESTED), "- topp\n  - under\n    - dypest\n- tilbake\n1. en\n  1. en.en\n2. to");
        assert_eq!(plain(TASKS), "- [ ] skriv\n- [x] test");
        assert_eq!(plain(TABLE), "Navn     | Antall\n---------+-------\nKari     |      3\nOla & co |     12");
        assert_eq!(plain(RULE), "over\n\n---\nunder");
        assert_eq!(plain(IMAGE), "Se diagram (https://northwind.example/d.png) her");
        assert_eq!(plain(AUTOLINK), "https://northwind.example og kari@northwind.example");
        assert_eq!(plain(MENTION), "Hei @Kari Nordmann og @Ola");
        assert_eq!(plain(BREAKS), "linje en\nlinje to\nlinje tre");
        assert_eq!(plain(ESCAPES), "*ikke fet* og # ikke tittel");
        assert_eq!(plain("\\- ikke liste\n1\\. ikke tall\nsnake\\_case \\[x\\]"), "- ikke liste\n1. ikke tall\nsnake_case [x]");
        assert_eq!(slack("\\*\\*ikke fet\\*\\*"), "**ikke fet**");
        assert_eq!(plain("Takk :tada:"), "Takk 🎉");
    }

    #[test]
    fn mention_ids_resolve_per_provider() {
        assert_eq!(mention_ids(MENTION), vec!["U1".to_string()]);
        let ids = HashMap::from([("U1".to_string(), "slack-U1".to_string())]);
        assert_eq!(with_mention_ids("[@Kari](mention:U1) [@Ola](mention:U9)", &ids), "[@Kari](mention:slack-U1) [@Ola](mention:)");
        assert_eq!(slack(&with_mention_ids("[@Kari](mention:U1) [@Ola](mention:U9)", &ids)), "<@slack-U1> @Ola");
    }

    #[test]
    fn emoji_shortcodes_skin_tones_and_custom_images_follow_each_target() {
        let source = "Takk :+1::skin-tone-3: :tada: ![:party_parrot:](https://emoji.slack-edge.com/T1/party_parrot/a.gif) :ukjent:";
        assert_eq!(html(source), "<p>Takk 👍🏼 🎉 <img src=\"https://emoji.slack-edge.com/T1/party_parrot/a.gif\" alt=\":party_parrot:\" title=\":party_parrot:\" style=\"height:1.25em;vertical-align:middle\"> :ukjent:</p>");
        assert_eq!(teams(source).0, "<p>Takk 👍🏼 🎉 <a href=\"https://emoji.slack-edge.com/T1/party_parrot/a.gif\">:party_parrot:</a> :ukjent:</p>");
        assert_eq!(slack(source), "Takk :+1::skin-tone-3: :tada: <https://emoji.slack-edge.com/T1/party_parrot/a.gif|:party_parrot:> :ukjent:");
        assert_eq!(google_chat(source), "Takk 👍🏼 🎉 <https://emoji.slack-edge.com/T1/party_parrot/a.gif|:party_parrot:> :ukjent:");
        assert_eq!(plain(source), "Takk 👍🏼 🎉 :party_parrot: :ukjent:");
        assert_eq!(html("![diagram](https://x.no/d.png)"), "<p><img src=\"https://x.no/d.png\" alt=\"diagram\"></p>");
    }

    #[test]
    fn urls_keep_balanced_parentheses() {
        assert_eq!(slack("(se https://en.wikipedia.org/wiki/Rust_(language))"), "(se <https://en.wikipedia.org/wiki/Rust_(language)>)");
        assert_eq!(html("[wiki](https://en.wikipedia.org/wiki/Rust_(language))"), "<p><a href=\"https://en.wikipedia.org/wiki/Rust_(language)\">wiki</a></p>");
    }
}
