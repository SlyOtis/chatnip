use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Ltr,
    Rtl,
    Mixed,
}

fn is_combining(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

pub fn is_emoji(c: char) -> bool {
    matches!(c as u32,
        0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2B00..=0x2BFF | 0x2300..=0x23FF | 0x3030 | 0x303D | 0x3297 | 0x3299 | 0x00A9 | 0x00AE | 0x2122 | 0x2190..=0x21FF | 0x2934 | 0x2935)
}

fn is_emoji_joiner(c: char) -> bool {
    matches!(c as u32, 0x200D | 0xFE0F | 0xFE0E | 0x20E3 | 0x1F3FB..=0x1F3FF | 0xE0020..=0xE007F)
}

pub fn is_emoji_only(text: &str) -> bool {
    let named = super::emoji::shortcode_pattern().is_match(text);
    let rest = super::emoji::shortcode_pattern().replace_all(text, "");
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        return named;
    }
    (named || trimmed.chars().any(is_emoji))
        && trimmed.chars().all(|c| c.is_whitespace() || is_emoji(c) || is_emoji_joiner(c) || (c.is_ascii_digit() && trimmed.contains('\u{20E3}')))
}

pub fn fold_name(name: &str) -> String {
    let mut folded = String::with_capacity(name.len());
    for c in name.to_lowercase().chars() {
        match c {
            'æ' => folded.push_str("ae"),
            'ø' | 'ö' | 'ó' | 'ò' | 'ô' | 'õ' => folded.push('o'),
            'å' | 'ä' | 'á' | 'à' | 'â' | 'ã' => folded.push('a'),
            'é' | 'è' | 'ê' | 'ë' => folded.push('e'),
            'ú' | 'ù' | 'û' | 'ü' => folded.push('u'),
            'í' | 'ì' | 'î' | 'ï' => folded.push('i'),
            'ñ' => folded.push('n'),
            'ç' => folded.push('c'),
            'ß' => folded.push_str("ss"),
            'ð' => folded.push('d'),
            'þ' => folded.push_str("th"),
            c if is_combining(c) || is_emoji(c) || is_emoji_joiner(c) => {}
            c if c.is_alphanumeric() || c == '-' || c == '\'' => folded.push(c),
            _ => folded.push(' '),
        }
    }
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_rtl(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
}

pub fn direction(text: &str) -> Direction {
    let rtl = text.chars().filter(|c| is_rtl(*c)).count();
    let ltr = text.chars().filter(|c| c.is_alphabetic() && !is_rtl(*c)).count();
    match (rtl, ltr) {
        (0, _) => Direction::Ltr,
        (_, 0) => Direction::Rtl,
        _ => Direction::Mixed,
    }
}

pub fn scripts(text: &str) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    for c in text.chars() {
        let name = match c as u32 {
            0x0041..=0x005A | 0x0061..=0x007A | 0x00C0..=0x024F => "latin",
            0x0370..=0x03FF => "greek",
            0x0400..=0x04FF => "cyrillic",
            0x0590..=0x05FF | 0xFB1D..=0xFB4F => "hebrew",
            0x0600..=0x06FF | 0x0750..=0x077F | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => "arabic",
            0x3040..=0x30FF => "kana",
            0x4E00..=0x9FFF => "han",
            0xAC00..=0xD7AF => "hangul",
            _ if is_emoji(c) => "emoji",
            _ => continue,
        };
        if !found.contains(&name) {
            found.push(name);
        }
    }
    found.sort_unstable();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_norwegian_and_drops_emoji() {
        assert_eq!(fold_name("Oscar Strand"), "oscar strand");
        assert_eq!(fold_name("SAM  HALE"), "sam hale");
        assert_eq!(fold_name("Mia Sund 🎨"), "mia sund");
        assert_eq!(fold_name("Hugo Brandt 🇳🇴"), "hugo brandt");
        assert_eq!(fold_name("Ha\u{030A}le, Sam"), "hale sam");
        assert_eq!(fold_name("Anne-Lise O'Brien\u{00A0}"), "anne-lise o'brien");
        assert_eq!(fold_name("👩🏽‍💻 Dev Team"), "dev team");
    }

    #[test]
    fn the_people_matcher_uses_the_same_folding() {
        assert_eq!(crate::services::people_service::normalise_name("Mia Sund 🎨"), fold_name("mia  sund"));
        assert_eq!(crate::services::people_service::normalise_name("Oscar Strand"), crate::services::people_service::normalise_name("oscar strand"));
    }

    #[test]
    fn detects_emoji_only() {
        assert!(is_emoji_only("🎉"));
        assert!(is_emoji_only(" 👍🏽 🚀 "));
        assert!(is_emoji_only("👩‍👩‍👧‍👦"));
        assert!(is_emoji_only("❤️"));
        assert!(!is_emoji_only("ok 👍"));
        assert!(!is_emoji_only(""));
        assert!(!is_emoji_only("123"));
        assert!(is_emoji_only(":tada: :party_parrot:"));
        assert!(is_emoji_only(":+1::skin-tone-3: 🎉"));
        assert!(!is_emoji_only("ok :tada:"));
    }

    #[test]
    fn detects_direction_and_scripts() {
        assert_eq!(direction("Hei, alle sammen"), Direction::Ltr);
        assert_eq!(direction("שלום עולם"), Direction::Rtl);
        assert_eq!(direction("مرحبا بالعالم"), Direction::Rtl);
        assert_eq!(direction("Thanks! شكراً — תודה"), Direction::Mixed);
        assert_eq!(scripts("Thanks! شكراً — תודה 🎉"), vec!["arabic", "emoji", "hebrew", "latin"]);
        assert_eq!(scripts("Привет 你好 こんにちは 안녕"), vec!["cyrillic", "han", "hangul", "kana"]);
    }
}
