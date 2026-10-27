pub mod calendar;
pub mod chat;
pub mod emoji;
pub mod headers;
pub mod html;
pub mod links;
pub mod mail;
pub mod markdown;
pub mod signature;
pub mod text;

use regex::Regex;
use std::sync::OnceLock;

pub(crate) fn rx(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("content pattern"))
}

#[cfg(test)]
pub(crate) fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {}: {}", name, e))
}
