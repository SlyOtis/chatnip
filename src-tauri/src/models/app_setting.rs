use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::collections::HashMap;

pub const MICROSOFT_CLIENT_ID: &str = "microsoft.client_id";
pub const MICROSOFT_CLIENT_SECRET: &str = "microsoft.client_secret";
pub const GOOGLE_CLIENT_ID: &str = "google.client_id";
pub const MICROSOFT_TENANT: &str = "microsoft.tenant";
pub const MICROSOFT_ADMIN_SCOPES: &str = "microsoft.admin_scopes";
pub const GOOGLE_CLIENT_SECRET: &str = "google.client_secret";
pub const SLACK_CLIENT_ID: &str = "slack.client_id";
pub const SLACK_CLIENT_SECRET: &str = "slack.client_secret";
pub const SLACK_APP_TOKEN: &str = "slack.app_token";
pub const SYNC_INTERVAL_SECONDS: &str = "sync.interval_seconds";
pub const SYNC_LIVE_INTERVAL_SECONDS: &str = "sync.live_interval_seconds";
pub const SYNC_HISTORY_DAYS: &str = "sync.history_days";
pub const EMOJI_SYNC_INTERVAL_SECONDS: &str = "emoji.sync_interval_seconds";
pub const DIRECTORY_SYNC_INTERVAL_SECONDS: &str = "directory.sync_interval_seconds";
pub const PROVIDER_RETRY_ATTEMPTS: &str = "providers.retry_attempts";
pub const PROVIDER_RETRY_MAX_SECONDS: &str = "providers.retry_max_seconds";
pub const ASSISTANT_ENABLED: &str = "assistant.enabled";
pub const ASSISTANT_MODEL: &str = "assistant.model";
pub const ASSISTANT_BINARY: &str = "assistant.binary";
pub const ASSISTANT_PROMPT: &str = "assistant.prompt";
pub const MOCK_ENABLED: &str = "mock.enabled";
pub const MOCK_REPLY_SECONDS: &str = "mock.reply_seconds";
pub const MOCK_NEWSLETTER_MINUTES: &str = "mock.newsletter_minutes";
pub const MOCK_HOME: &str = "mock.home";
pub const PLUGIN_HOME: &str = "plugin.home";
pub const PLUGIN_HOME_PATH: &str = "plugin.home_path";
pub const PLUGIN_MOCK: &str = "plugin.mock";
pub const GIF_TENOR_KEY: &str = "gif.tenor_key";
pub const GIF_GIPHY_KEY: &str = "gif.giphy_key";
pub const GIF_RATING: &str = "gif.rating";
pub const GIF_LOCALE: &str = "gif.locale";
pub const GIF_DEFAULT_TAB: &str = "gif.default_tab";
pub const GIF_MAX_SIZE: &str = "gif.max_size";
pub const GIF_STICKERS: &str = "gif.stickers";
pub const GIF_COLLECTIONS: &str = "gif.collections";
pub const LINKS_PREVIEWS: &str = "links.previews";
pub const INTELLIGENCE_TIMEOUT_SECONDS: &str = "intelligence.timeout_seconds";
pub const GEO_MMDB_PATH: &str = "geo.mmdb_path";
pub const APP_LOCALE: &str = "app.locale";
pub const APP_LAST_ROUTE: &str = "app.last-route";
pub const APP_STARTUP_PAGE: &str = "app.startup-page";

pub const SECURITY_LOCK: &str = "security.lock";
pub const SECURITY_IDLE_MINUTES: &str = "security.idle_minutes";
pub const ACTIONS_ENABLED: &str = "actions.enabled";
pub const OUTBOUND_WAIT_FOR_NETWORK: &str = "outbound.wait_for_network";
pub const VOICE_ENABLED: &str = "voice.enabled";
pub const VOICE_MODEL: &str = "voice.model";
pub const VOICE_VAD: &str = "voice.vad";
pub const VOICE_LANGUAGE: &str = "voice.language";
pub const VOICE_CLEANUP: &str = "voice.cleanup";
pub const VOICE_HOTKEY: &str = "voice.hotkey";
pub const VOICE_MODEL_URL: &str = "voice.model_url";
pub const VOICE_VAD_URL: &str = "voice.vad_url";
pub const VOICE_DEBOUNCE_MS: &str = "voice.debounce_ms";
pub const OLLAMA_HOST: &str = "ollama.host";
pub const SUMMARIES_CONCURRENCY: &str = "summaries.concurrency";
pub const SUMMARIES_LENGTH: &str = "summaries.length";
pub const SUMMARIES_FORM: &str = "summaries.form";
pub const SUMMARIES_LANGUAGE: &str = "summaries.language";
pub const SUMMARIES_ACTIONS: &str = "summaries.actions";
pub const SUMMARIES_INSTRUCTIONS: &str = "summaries.instructions";
pub const ACTIONS_MAX_SENDS_PER_HOUR: &str = "actions.max_sends_per_hour";
pub const RECEIPTS_BATCH: &str = "receipts.batch";
pub const RECEIPTS_THRESHOLD: &str = "receipts.threshold";
pub const WINDOW_CARD: &str = "window.card";
pub const MESSAGES_ENTER_SENDS: &str = "messages.enter_sends";
pub const MAIL_SEND_DELAY_SECONDS: &str = "mail.send_delay_seconds";
pub const MAIL_SEND_NOTICE: &str = "mail.send_notice";
pub const MAIL_DRAFT_NOTICE: &str = "mail.draft_notice";
pub const MAIL_DELAY_ACTIONS: &str = "mail.delay_actions";
pub const MAIL_QUIT_POLICY: &str = "mail.quit_policy";
pub const FEATURE_PREFIX: &str = "ai.feature.";
pub const FEATURE_CONNECTION: &str = "connection";
pub const FEATURE_MODEL: &str = "model";
pub const THREADS_FROM: &str = "summaries";

pub const APPEARANCE_PREFIX: &str = "appearance.";
pub const LAYOUT_PREFIXES: [&str; 4] = ["sidebar.", "list.", "messages.", "shortcuts."];
pub const ACTION_MOCK_PREFIX: &str = "actions.mock.";
pub const MAIL_IMAGES_PREFIX: &str = "mail.images.";
pub const KEYS: [&str; 70] = [
    APP_LOCALE,
    OUTBOUND_WAIT_FOR_NETWORK,
    APP_LAST_ROUTE,
    APP_STARTUP_PAGE,
    ACTIONS_ENABLED,
    SECURITY_LOCK,
    SECURITY_IDLE_MINUTES,
    MICROSOFT_TENANT,
    MICROSOFT_ADMIN_SCOPES,
    SLACK_CLIENT_ID,
    SLACK_CLIENT_SECRET,
    SLACK_APP_TOKEN,
    INTELLIGENCE_TIMEOUT_SECONDS,
    MICROSOFT_CLIENT_ID,
    MICROSOFT_CLIENT_SECRET,
    GOOGLE_CLIENT_ID,
    GOOGLE_CLIENT_SECRET,
    SYNC_INTERVAL_SECONDS,
    SYNC_LIVE_INTERVAL_SECONDS,
    SYNC_HISTORY_DAYS,
    EMOJI_SYNC_INTERVAL_SECONDS,
    DIRECTORY_SYNC_INTERVAL_SECONDS,
    PROVIDER_RETRY_ATTEMPTS,
    PROVIDER_RETRY_MAX_SECONDS,
    ASSISTANT_ENABLED,
    ASSISTANT_MODEL,
    ASSISTANT_BINARY,
    ASSISTANT_PROMPT,
    MOCK_ENABLED,
    MOCK_REPLY_SECONDS,
    MOCK_NEWSLETTER_MINUTES,
    MOCK_HOME,
    PLUGIN_HOME,
    PLUGIN_HOME_PATH,
    PLUGIN_MOCK,
    GIF_TENOR_KEY,
    GIF_GIPHY_KEY,
    GIF_RATING,
    GIF_LOCALE,
    GIF_DEFAULT_TAB,
    GIF_MAX_SIZE,
    GIF_STICKERS,
    GIF_COLLECTIONS,
    LINKS_PREVIEWS,
    GEO_MMDB_PATH,
    VOICE_ENABLED,
    VOICE_MODEL,
    VOICE_VAD,
    VOICE_LANGUAGE,
    VOICE_CLEANUP,
    VOICE_HOTKEY,
    VOICE_MODEL_URL,
    VOICE_VAD_URL,
    VOICE_DEBOUNCE_MS,
    OLLAMA_HOST,
    SUMMARIES_CONCURRENCY,
    SUMMARIES_LENGTH,
    SUMMARIES_FORM,
    SUMMARIES_LANGUAGE,
    SUMMARIES_ACTIONS,
    SUMMARIES_INSTRUCTIONS,
    ACTIONS_MAX_SENDS_PER_HOUR,
    RECEIPTS_BATCH,
    RECEIPTS_THRESHOLD,
    WINDOW_CARD,
    MAIL_SEND_DELAY_SECONDS,
    MAIL_SEND_NOTICE,
    MAIL_DRAFT_NOTICE,
    MAIL_DELAY_ACTIONS,
    MAIL_QUIT_POLICY,
];

const SEED_FROM_ENV: [(&str, &str); 9] = [
    (SLACK_APP_TOKEN, "SLACK_APP_TOKEN"),
    (MICROSOFT_TENANT, "MICROSOFT_TENANT_ID"),
    (SLACK_CLIENT_ID, "SLACK_CLIENT_ID"),
    (SLACK_CLIENT_SECRET, "SLACK_CLIENT_SECRET"),
    (MICROSOFT_CLIENT_ID, "MICROSOFT_CLIENT_ID"),
    (MICROSOFT_CLIENT_SECRET, "MICROSOFT_CLIENT_SECRET"),
    (GOOGLE_CLIENT_ID, "GOOGLE_CLIENT_ID"),
    (GOOGLE_CLIENT_SECRET, "GOOGLE_CLIENT_SECRET"),
    (MOCK_ENABLED, "CHATNIP_USE_DEBUG"),
];

pub const DEFAULTS: [(&str, &str); 31] = [
    (ASSISTANT_BINARY, "claude"),
    (INTELLIGENCE_TIMEOUT_SECONDS, "120"),
    (OLLAMA_HOST, "http://localhost:11434"),
    (SUMMARIES_CONCURRENCY, "3"),
    (SYNC_INTERVAL_SECONDS, "120"),
    (SYNC_LIVE_INTERVAL_SECONDS, "900"),
    (SYNC_HISTORY_DAYS, "30"),
    (EMOJI_SYNC_INTERVAL_SECONDS, "21600"),
    (DIRECTORY_SYNC_INTERVAL_SECONDS, "86400"),
    (PROVIDER_RETRY_ATTEMPTS, "4"),
    (PROVIDER_RETRY_MAX_SECONDS, "60"),
    (SECURITY_IDLE_MINUTES, "10"),
    (GIF_MAX_SIZE, "8"),
    (VOICE_MODEL_URL, "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin"),
    (VOICE_VAD_URL, "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin"),
    (VOICE_DEBOUNCE_MS, "1200"),
    (ACTIONS_MAX_SENDS_PER_HOUR, "10"),
    (RECEIPTS_BATCH, "5"),
    (RECEIPTS_THRESHOLD, "0.5"),
    (MOCK_REPLY_SECONDS, "8"),
    (MOCK_NEWSLETTER_MINUTES, "30"),
    (MOCK_HOME, "~/.chatnip-mock"),
    (PLUGIN_HOME, "app"),
    (PLUGIN_MOCK, "false"),
    (WINDOW_CARD, "360x227"),
    (MESSAGES_ENTER_SENDS, "enter"),
    (MAIL_SEND_DELAY_SECONDS, "10"),
    (MAIL_SEND_NOTICE, "Sending email {subject} in {seconds}"),
    (MAIL_DRAFT_NOTICE, "Kept email {subject} as a draft"),
    (MAIL_DELAY_ACTIONS, "off"),
    (MAIL_QUIT_POLICY, "draft"),
];

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ModelFeature {
    pub id: &'static str,
    pub prompts: &'static [&'static str],
    pub models: &'static [(&'static str, &'static str)],
}

pub const MODEL_FEATURES: [ModelFeature; 3] = [
    ModelFeature { id: "threads", prompts: &["summarise_thread"], models: &[] },
    ModelFeature { id: "assistant", prompts: &["author_action", "author_template"], models: &[] },
    ModelFeature { id: "voice", prompts: &["voice_cleanup"], models: &[(crate::models::intelligence::KIND_CLAUDE_CLI, "haiku")] },
];

pub const MOVED_FEATURES: [(&str, &[&str]); 6] = [
    ("sorting", &["prompt(\"sort\"", "prompt(\"classify_sender\""]),
    (THREADS_FROM, &["prompt(\"summarise_mail\"", "prompt(\"summarise_thread\""]),
    ("events", &["prompt(\"parse_events\""]),
    ("drafting", &["prompt(\"reply_draft\""]),
    ("receipts", &["receipt_confirm("]),
    ("scripts", &["ai(", "ai_json("]),
];

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct OllamaSuggestion {
    pub name: &'static str,
    pub size: &'static str,
    pub good_for: &'static str,
}

pub const OLLAMA_SUGGESTIONS: [OllamaSuggestion; 6] = [
    OllamaSuggestion { name: "qwen3:8b", size: "5.2 GB", good_for: "The best all-rounder on a laptop: summaries, sorting and replies in many languages, Norwegian included" },
    OllamaSuggestion { name: "qwen3:4b", size: "2.5 GB", good_for: "Fast summaries and sorting on machines with 8 GB of memory" },
    OllamaSuggestion { name: "gemma3:4b", size: "3.3 GB", good_for: "Short, clean summaries and reply drafts" },
    OllamaSuggestion { name: "llama3.2:3b", size: "2.0 GB", good_for: "The smallest useful model, for quick sorting" },
    OllamaSuggestion { name: "phi4-mini:3.8b", size: "2.5 GB", good_for: "Structured answers such as appointments and rules" },
    OllamaSuggestion { name: "mistral-small:24b", size: "14 GB", good_for: "Near hosted quality for drafts on a machine with 32 GB or more" },
];

#[derive(Debug, Clone, Serialize)]
pub struct SettingRegistry {
    pub defaults: HashMap<String, String>,
    pub values: HashMap<String, String>,
    pub features: Vec<ModelFeature>,
}

pub fn get_default(key: &str) -> Option<&'static str> {
    DEFAULTS.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

pub fn get_default_u64(key: &str) -> u64 {
    get_default(key).and_then(|v| v.parse().ok()).unwrap_or(0)
}

pub fn get_default_f64(key: &str) -> f64 {
    get_default(key).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

pub fn get_model_feature(prompt: &str) -> Option<&'static ModelFeature> {
    MODEL_FEATURES.iter().find(|f| f.prompts.contains(&prompt))
}

pub fn feature_key(feature: &str, part: &str) -> String {
    format!("{}{}.{}", FEATURE_PREFIX, feature, part)
}

impl ModelFeature {
    pub fn get_default_model(&self, kind: &str) -> Option<&'static str> {
        self.models.iter().find(|(k, _)| *k == kind).map(|(_, m)| *m)
    }
}

fn is_feature_key(key: &str) -> bool {
    key.strip_prefix(FEATURE_PREFIX)
        .and_then(|rest| rest.rsplit_once('.'))
        .is_some_and(|(feature, part)| MODEL_FEATURES.iter().any(|f| f.id == feature) && (part == FEATURE_CONNECTION || part == FEATURE_MODEL))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSetting {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

impl AppSetting {
    pub fn is_secret(key: &str) -> bool {
        key.ends_with(".client_secret") || key == SLACK_APP_TOKEN
    }

    pub async fn get(pool: &Pool<Sqlite>, key: &str) -> Result<Option<String>, String> {
        let row = sqlx::query("SELECT value FROM app_setting WHERE key = ?1")
            .bind(key)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to read setting {}: {}", key, e))?;
        let value = row.map(|r| r.get::<String, _>("value"));
        Ok(if Self::is_secret(key) { crate::services::security_service::reveal(crate::services::security_service::APP_VAULT, key, value) } else { value })
    }

    pub async fn get_or(pool: &Pool<Sqlite>, key: &str, default: &str) -> Result<String, String> {
        Ok(Self::get(pool, key).await?.unwrap_or_else(|| default.to_string()))
    }

    pub async fn get_value(pool: &Pool<Sqlite>, key: &str) -> Result<String, String> {
        Ok(Self::get(pool, key).await?.unwrap_or_else(|| get_default(key).unwrap_or_default().to_string()))
    }

    pub async fn get_u64(pool: &Pool<Sqlite>, key: &str) -> Result<u64, String> {
        Ok(Self::get(pool, key)
            .await?
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or_else(|| get_default_u64(key)))
    }

    pub async fn get_f64(pool: &Pool<Sqlite>, key: &str) -> Result<f64, String> {
        Ok(Self::get(pool, key)
            .await?
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or_else(|| get_default_f64(key)))
    }

    pub async fn get_registry(pool: &Pool<Sqlite>) -> Result<SettingRegistry, String> {
        let defaults: HashMap<String, String> = DEFAULTS.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let mut values = defaults.clone();
        values.extend(Self::all(pool).await?);
        Ok(SettingRegistry { defaults, values, features: MODEL_FEATURES.to_vec() })
    }

    pub fn allowed(key: &str) -> bool {
        KEYS.contains(&key) || is_feature_key(key) || key.starts_with(APPEARANCE_PREFIX) || [ACTION_MOCK_PREFIX, MAIL_IMAGES_PREFIX].iter().chain(LAYOUT_PREFIXES.iter()).any(|prefix| key.starts_with(prefix) && key.len() > prefix.len())
    }

    pub fn switches_mock_off(key: &str, value: &str) -> bool {
        key == MOCK_ENABLED && value.trim() != "true"
    }

    pub async fn put(pool: &Pool<Sqlite>, key: &str, value: &str) -> Result<(), String> {
        if !Self::allowed(key) {
            return Err(format!("Unknown setting {}", key));
        }
        let now = Utc::now().to_rfc3339();
        let sealed = if Self::is_secret(key) { crate::services::security_service::seal(crate::services::security_service::APP_VAULT, key, Some(value))?.unwrap_or_default() } else { value.trim().to_string() };
        let value = sealed.as_str();
        if value.is_empty() {
            sqlx::query("DELETE FROM app_setting WHERE key = ?1")
                .bind(key)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to clear setting {}: {}", key, e))?;
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO app_setting (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value.trim())
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to save setting {}: {}", key, e))?;
        Ok(())
    }

    pub async fn all(pool: &Pool<Sqlite>) -> Result<HashMap<String, String>, String> {
        let rows = sqlx::query("SELECT key, value FROM app_setting")
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read settings: {}", e))?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let key = r.get::<String, _>("key");
                let value = if Self::is_secret(&key) { crate::services::security_service::SECRET_MARKER.to_string() } else { r.get::<String, _>("value") };
                (key, value)
            })
            .collect())
    }

    pub async fn seal_existing(pool: &Pool<Sqlite>) -> Result<usize, String> {
        let rows = sqlx::query("SELECT key, value FROM app_setting WHERE value != ?1")
            .bind(crate::services::security_service::SECRET_MARKER)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to read settings: {}", e))?;
        let mut moved = 0;
        for row in rows {
            let key: String = row.get("key");
            if Self::is_secret(&key) {
                Self::put(pool, &key, &row.get::<String, _>("value")).await?;
                moved += 1;
            }
        }
        Ok(moved)
    }

    pub async fn seed_from_env(pool: &Pool<Sqlite>) -> Result<(), String> {
        for (key, env_name) in SEED_FROM_ENV {
            if Self::get(pool, key).await?.is_some() {
                continue;
            }
            if let Ok(value) = std::env::var(env_name) {
                if !value.trim().is_empty() {
                    Self::put(pool, key, &value).await?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::test_pool;

    #[test]
    fn layout_keys_are_allowed_and_mock_off_is_detected() {
        assert!(AppSetting::allowed("sidebar.icon.inbox"));
        assert!(AppSetting::allowed("list.section.people.collapsed"));
        assert!(!AppSetting::allowed("sidebar."));
        assert!(AppSetting::allowed("actions.mock.a1.node"));
        assert!(AppSetting::allowed("mail.images.nyhetsbrev@nordhus.no"));
        assert!(!AppSetting::allowed("mail.images."));
        assert!(!AppSetting::allowed("actions.mock."));
        assert!(!AppSetting::allowed("unknown.key"));
        assert!(AppSetting::switches_mock_off(MOCK_ENABLED, "false"));
        assert!(AppSetting::switches_mock_off(MOCK_ENABLED, ""));
        assert!(!AppSetting::switches_mock_off(MOCK_ENABLED, "true"));
        assert!(!AppSetting::switches_mock_off("links.previews", "false"));
    }

    #[tokio::test]
    async fn put_get_and_clear_round_trip() {
        let pool = test_pool().await;
        AppSetting::put(&pool, MICROSOFT_CLIENT_ID, "  abc  ").await.unwrap();
        assert_eq!(AppSetting::get(&pool, MICROSOFT_CLIENT_ID).await.unwrap(), Some("abc".to_string()));
        AppSetting::put(&pool, MICROSOFT_CLIENT_ID, "xyz").await.unwrap();
        assert_eq!(AppSetting::get(&pool, MICROSOFT_CLIENT_ID).await.unwrap(), Some("xyz".to_string()));
        AppSetting::put(&pool, MICROSOFT_CLIENT_ID, "").await.unwrap();
        assert_eq!(AppSetting::get(&pool, MICROSOFT_CLIENT_ID).await.unwrap(), None);
    }

    #[tokio::test]
    async fn unknown_keys_are_refused() {
        let pool = test_pool().await;
        assert!(AppSetting::put(&pool, "nope", "x").await.is_err());
    }

    #[tokio::test]
    async fn numeric_settings_fall_back_to_default() {
        let pool = test_pool().await;
        assert_eq!(AppSetting::get_u64(&pool, SYNC_INTERVAL_SECONDS).await.unwrap(), 120);
        AppSetting::put(&pool, SYNC_INTERVAL_SECONDS, "90").await.unwrap();
        assert_eq!(AppSetting::get_u64(&pool, SYNC_INTERVAL_SECONDS).await.unwrap(), 90);
        AppSetting::put(&pool, SYNC_INTERVAL_SECONDS, "soon").await.unwrap();
        assert_eq!(AppSetting::get_u64(&pool, SYNC_INTERVAL_SECONDS).await.unwrap(), 120);
        assert_eq!(AppSetting::get_f64(&pool, RECEIPTS_THRESHOLD).await.unwrap(), 0.5);
        AppSetting::put(&pool, RECEIPTS_THRESHOLD, "0.7").await.unwrap();
        assert_eq!(AppSetting::get_f64(&pool, RECEIPTS_THRESHOLD).await.unwrap(), 0.7);
    }

    #[test]
    fn every_default_belongs_to_a_known_key_and_parses() {
        for (key, value) in DEFAULTS {
            assert!(AppSetting::allowed(key), "{} has a default but is not a setting", key);
            assert!(!value.trim().is_empty(), "{} has an empty default", key);
        }
        for key in [INTELLIGENCE_TIMEOUT_SECONDS, SUMMARIES_CONCURRENCY, SYNC_INTERVAL_SECONDS, SYNC_HISTORY_DAYS, EMOJI_SYNC_INTERVAL_SECONDS, DIRECTORY_SYNC_INTERVAL_SECONDS, PROVIDER_RETRY_ATTEMPTS, PROVIDER_RETRY_MAX_SECONDS, SECURITY_IDLE_MINUTES, GIF_MAX_SIZE, VOICE_DEBOUNCE_MS, ACTIONS_MAX_SENDS_PER_HOUR, RECEIPTS_BATCH, MAIL_SEND_DELAY_SECONDS] {
            assert!(get_default_u64(key) > 0, "{} needs a positive numeric default", key);
        }
        assert!(get_default_f64(RECEIPTS_THRESHOLD) > 0.0);
        assert_eq!(get_default(ASSISTANT_MODEL), None);
        assert!(!AppSetting::allowed("assistant.models"));
    }

    #[test]
    fn feature_keys_are_allowed_only_for_known_features_and_parts() {
        assert!(AppSetting::allowed(&feature_key("voice", FEATURE_MODEL)));
        assert!(AppSetting::allowed(&feature_key("threads", FEATURE_CONNECTION)));
        assert!(!AppSetting::allowed(&feature_key("summaries", FEATURE_CONNECTION)));
        assert!(!AppSetting::allowed(&feature_key("sorting", FEATURE_MODEL)));
        assert!(!AppSetting::allowed(&feature_key("nowhere", FEATURE_MODEL)));
        assert!(!AppSetting::allowed(&feature_key("voice", "colour")));
        assert!(!AppSetting::allowed(FEATURE_PREFIX));
        assert_eq!(get_model_feature("summarise_thread").map(|f| f.id), Some("threads"));
        assert_eq!(get_model_feature("summarise_mail"), None);
        assert_eq!(get_model_feature("nothing"), None);
    }

    #[tokio::test]
    async fn the_registry_overlays_stored_values_on_defaults() {
        let pool = test_pool().await;
        AppSetting::put(&pool, ASSISTANT_MODEL, "picked").await.unwrap();
        let registry = AppSetting::get_registry(&pool).await.unwrap();
        assert_eq!(registry.values.get(ASSISTANT_MODEL).map(String::as_str), Some("picked"));
        assert_eq!(registry.defaults.get(ASSISTANT_MODEL).map(String::as_str), get_default(ASSISTANT_MODEL));
        assert_eq!(registry.values.get(OLLAMA_HOST), registry.defaults.get(OLLAMA_HOST));
        assert_eq!(registry.features.len(), MODEL_FEATURES.len());
        assert_eq!(AppSetting::get_value(&pool, ASSISTANT_MODEL).await.unwrap(), "picked");
        assert_eq!(AppSetting::get_value(&pool, ASSISTANT_BINARY).await.unwrap(), "claude");
    }

    #[tokio::test]
    async fn all_returns_every_stored_key() {
        let pool = test_pool().await;
        AppSetting::put(&pool, GOOGLE_CLIENT_ID, "g").await.unwrap();
        AppSetting::put(&pool, SYNC_HISTORY_DAYS, "30").await.unwrap();
        let all = AppSetting::all(&pool).await.unwrap();
        assert_eq!(all.get(GOOGLE_CLIENT_ID).map(String::as_str), Some("g"));
        assert_eq!(all.get(SYNC_HISTORY_DAYS).map(String::as_str), Some("30"));
        assert_eq!(all.len(), 2);
    }

    fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn no_source_outside_the_registry_names_a_model() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let registry = root.join("models").join("app_setting.rs");
        let model = regex::Regex::new(r#"(?i)\b(claude-(fable|opus|sonnet|haiku)[\w.-]*|(qwen|llama|gemma|phi)\d[\w.:-]*|mistral-[\w.:-]+|gpt-\d[\w.-]*|ggml-[\w.-]+)|"(haiku|sonnet|opus|fable)""#).unwrap();
        let mut files = Vec::new();
        sources(&root, &mut files);
        assert!(files.len() > 20);
        let mut found = Vec::new();
        for path in files.iter().filter(|p| **p != registry && !p.ends_with("test_helpers.rs")) {
            let text = std::fs::read_to_string(path).unwrap();
            let code = text.split("#[cfg(test)]").next().unwrap_or_default();
            for (n, line) in code.lines().enumerate() {
                if let Some(hit) = model.find(line) {
                    found.push(format!("{}:{}: {}", path.strip_prefix(&root).unwrap().display(), n + 1, hit.as_str()));
                }
            }
        }
        assert!(found.is_empty(), "model ids belong in the settings registry:\n{}", found.join("\n"));
        assert!(OLLAMA_SUGGESTIONS.iter().all(|s| model.is_match(s.name)));
        assert!(model.is_match(get_default(VOICE_MODEL_URL).unwrap()) && model.is_match(get_default(VOICE_VAD_URL).unwrap()));
    }
}
