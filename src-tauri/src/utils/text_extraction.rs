use regex::Regex;

pub fn extract_text_from_content(content: &str, content_type: &str) -> String {
    match content_type {
        "html" => extract_text_from_html(content),
        "markdown" => extract_text_from_markdown(content),
        _ => content.to_string(),
    }
}

fn extract_text_from_html(html: &str) -> String {
    crate::content::html::flat_text(html)
}

fn extract_text_from_markdown(markdown: &str) -> String {
    let code_block_regex = Regex::new(r"```[\s\S]*?```").unwrap();
    let inline_code_regex = Regex::new(r"`[^`]+`").unwrap();
    let link_regex = Regex::new(r"\[([^\]]+)\]\([^\)]+\)").unwrap();
    let image_regex = Regex::new(r"!\[([^\]]*)\]\([^\)]+\)").unwrap();
    let heading_regex = Regex::new(r"^#+\s*").unwrap();
    let list_regex = Regex::new(r"^[\*\-\+]\s+").unwrap();
    let numbered_list_regex = Regex::new(r"^\d+\.\s+").unwrap();
    let emphasis_regex = Regex::new(r"[\*_]{1,3}").unwrap();
    
    let text = code_block_regex.replace_all(markdown, " [code] ");
    let text = inline_code_regex.replace_all(&text, " $1 ");
    let text = link_regex.replace_all(&text, "$1");
    let text = image_regex.replace_all(&text, "[image: $1]");
    let text = heading_regex.replace_all(&text, "");
    let text = list_regex.replace_all(&text, "");
    let text = numbered_list_regex.replace_all(&text, "");
    let text = emphasis_regex.replace_all(&text, "");
    
    text.trim().to_string()
}

pub fn generate_summary(text: &str, max_length: usize) -> String {
    let clean_text = text.trim();
    if clean_text.len() <= max_length {
        clean_text.to_string()
    } else {
        // Find a safe UTF-8 boundary
        let mut boundary = max_length;
        while !clean_text.is_char_boundary(boundary) && boundary > 0 {
            boundary -= 1;
        }
        
        let truncated = &clean_text[..boundary];
        if let Some(last_space) = truncated.rfind(' ') {
            format!("{}...", &truncated[..last_space])
        } else {
            format!("{}...", truncated)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_html_strips_tags() {
        let result = extract_text_from_content("<p>Hello <b>world</b></p>", "html");
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn extract_html_decodes_entities() {
        let result = extract_text_from_content("5 &lt; 10 &amp; 10 &gt; 5", "html");
        assert_eq!(result, "5 < 10 & 10 > 5");
    }

    #[test]
    fn extract_html_decodes_quot_and_apos() {
        let result = extract_text_from_content("&quot;hello&quot; &#39;world&#39;", "html");
        assert_eq!(result, "\"hello\" 'world'");
    }

    #[test]
    fn extract_html_nested_tags() {
        let result = extract_text_from_content(
            "<div><p><span><b><i>deep</i></b></span></p></div>",
            "html",
        );
        assert_eq!(result, "deep");
    }

    #[test]
    fn extract_html_empty_string() {
        let result = extract_text_from_content("", "html");
        assert_eq!(result, "");
    }

    #[test]
    fn extract_html_malformed() {
        let result = extract_text_from_content("<p>unclosed <b>tags", "html");
        assert_eq!(result, "unclosed tags");
    }

    #[test]
    fn extract_markdown_strips_formatting() {
        let result = extract_text_from_content("**bold** and *italic*", "markdown");
        assert_eq!(result, "bold and italic");
    }

    #[test]
    fn extract_markdown_strips_links() {
        let result = extract_text_from_content("[click here](https://example.com)", "markdown");
        assert_eq!(result, "click here");
    }

    #[test]
    fn extract_text_passthrough() {
        let result = extract_text_from_content("plain text <b>not stripped</b>", "text");
        assert_eq!(result, "plain text <b>not stripped</b>");
    }

    #[test]
    fn extract_unknown_type_passthrough() {
        let result = extract_text_from_content("hello", "unknown");
        assert_eq!(result, "hello");
    }

    #[test]
    fn generate_summary_short_text() {
        let result = generate_summary("short", 100);
        assert_eq!(result, "short");
    }

    #[test]
    fn generate_summary_truncates_at_word_boundary() {
        let result = generate_summary("hello world this is a long message", 15);
        assert_eq!(result, "hello world...");
    }

    #[test]
    fn generate_summary_exact_length() {
        let text = "exactly";
        let result = generate_summary(text, text.len());
        assert_eq!(result, "exactly");
    }

    #[test]
    fn generate_summary_empty_string() {
        let result = generate_summary("", 100);
        assert_eq!(result, "");
    }

    #[test]
    fn generate_summary_trims_whitespace() {
        let result = generate_summary("  hello  ", 100);
        assert_eq!(result, "hello");
    }

    #[test]
    fn long_message_summary_is_fast_and_bounded() {
        let line = "Linje med æøå, emoji 🚀 og en lenke https://northwind.example/x ";
        let body: String = (0..200).map(|i| format!("<p>{} {}</p>\n", i, line.repeat(1))).collect::<String>();
        let body = format!("{}{}", body, "x".repeat(10_000usize.saturating_sub(body.chars().count())));
        assert!(body.chars().count() >= 10_000);
        assert_eq!(body.matches('\n').count(), 200);
        let started = std::time::Instant::now();
        let text = extract_text_from_content(&body, "html");
        let summary = generate_summary(&text, 200);
        assert!(started.elapsed() < std::time::Duration::from_millis(50), "{:?}", started.elapsed());
        assert!(summary.len() <= 203);
        assert!(summary.ends_with("..."));
        assert!(summary.starts_with("0 Linje med æøå"));
        assert!(!summary.contains('<'));
    }
}
pub fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
