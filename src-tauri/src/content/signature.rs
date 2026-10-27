const SIGN_OFFS: [&str; 24] = [
    "med vennlig hilsen",
    "vennlig hilsen",
    "med hilsen",
    "beste hilsen",
    "hilsen",
    "mvh",
    "vh",
    "takk",
    "best regards",
    "kind regards",
    "warm regards",
    "regards",
    "best",
    "all the best",
    "thanks",
    "thank you",
    "many thanks",
    "cheers",
    "sincerely",
    "yours sincerely",
    "br",
    "rgds",
    "ha en fin dag",
    "god helg",
];

const QUOTE_STARTS: [&str; 6] = [">", "fra:", "from:", "-----original", "________", "sendt fra min"];

fn is_quote_start(line: &str) -> bool {
    let lower = line.trim().to_lowercase();
    if QUOTE_STARTS.iter().any(|start| lower.starts_with(start)) {
        return true;
    }
    (lower.starts_with("on ") && lower.ends_with("wrote:")) || (lower.starts_with("den ") && lower.ends_with("skrev:"))
}

fn is_sign_off(line: &str) -> bool {
    let lower = line.trim().trim_start_matches("--").trim().to_lowercase();
    let bare = lower.trim_end_matches(|c| c == ',' || c == '.' || c == '!' || c == ':').trim();
    !bare.is_empty() && SIGN_OFFS.contains(&bare)
}

fn is_delimiter(line: &str) -> bool {
    let trimmed = line.trim_end();
    trimmed == "--" || trimmed == "-- " || trimmed == "__" || trimmed.chars().count() >= 3 && trimmed.chars().all(|c| c == '-' || c == '_' || c == '—')
}

pub fn signature_of(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    let end = lines.iter().position(|line| is_quote_start(line)).unwrap_or(lines.len());
    let mut end = end;
    while end > 0 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    let from = end.saturating_sub(15);
    let start = (from..end).rev().find(|&i| is_delimiter(lines[i]) || is_sign_off(lines[i]))?;
    let block: Vec<&str> = lines[start..end].iter().map(|line| line.trim()).filter(|line| !line.is_empty() && !is_delimiter(line)).collect();
    if block.len() < 2 || block.len() > 14 {
        return None;
    }
    Some(block.join("\n"))
}

pub fn signature_hash(signature: &str) -> String {
    let folded: String = signature.lines().map(|line| line.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()).collect::<Vec<_>>().join("\n");
    crate::utils::short_hash(folded.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_norwegian_sign_off_starts_the_signature_and_the_quote_is_left_out() {
        let text = "Hei,\n\nJeg sjekker med kunden.\n\nMed vennlig hilsen\nEva Ortega\nNorthwind AS\n+47 900 00 000\n\nFra: Kari\nSendt: i går\n> gammel tekst";
        assert_eq!(signature_of(text).as_deref(), Some("Med vennlig hilsen\nEva Ortega\nNorthwind AS\n+47 900 00 000"));
    }

    #[test]
    fn a_dash_delimiter_starts_the_signature() {
        assert_eq!(signature_of("Ok, sees.\n\n-- \nJonas Lie\nclient.example").as_deref(), Some("Jonas Lie\nclient.example"));
    }

    #[test]
    fn a_mail_without_a_sign_off_has_no_signature() {
        assert_eq!(signature_of("Hei\n\nKommer kl 12."), None);
        assert_eq!(signature_of("Takk"), None);
    }

    #[test]
    fn the_hash_ignores_spacing_and_case() {
        assert_eq!(signature_hash("Best regards\nEva  Ortega"), signature_hash("best regards\neva ortega"));
        assert_ne!(signature_hash("Best regards\nEva Ortega"), signature_hash("Best regards\nJonas Lie"));
    }
}
