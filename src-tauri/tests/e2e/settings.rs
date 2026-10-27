use super::support::{js_string, tid, App, MOCK_KINDS};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const SETTINGS: &str = "/main/profile";
const ONLY_WHEN_LOCKED: [&str; 1] = ["privacy.lock-now"];

struct Anchor {
    id: String,
    section: String,
    fallback: Option<String>,
}

fn anchors() -> Vec<Anchor> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/settings.ts");
    let source = std::fs::read_to_string(&path).unwrap();
    let block = source.split("export const SETTING_SECTIONS = [").nth(1).unwrap().split("] as const").next().unwrap().to_string();
    let section_line = regex::Regex::new(r"^    id: '([^']+)'").unwrap();
    let setting_line = regex::Regex::new(r"^      \{ id: '([^']+)'").unwrap();
    let fallback = regex::Regex::new(r"fallback: '([^']+)'").unwrap();
    let mut out = Vec::new();
    let mut section = String::new();
    for line in block.lines() {
        if let Some(found) = section_line.captures(line) {
            section = found[1].to_string();
            out.push(Anchor { id: section.clone(), section: section.clone(), fallback: None });
        } else if let Some(found) = setting_line.captures(line) {
            let back = fallback.captures(line).map(|f| format!("{}.{}", section, &f[1]));
            out.push(Anchor { id: format!("{}.{}", section, &found[1]), section: section.clone(), fallback: back });
        }
    }
    assert!(out.len() > 60, "the settings anchors were not parsed from {}", path.display());
    out
}

fn sections() -> Vec<String> {
    anchors().into_iter().filter(|a| a.id == a.section).map(|a| a.id).collect()
}

fn row(id: &str) -> String {
    format!("[id={}]", js_string(id))
}

fn href(anchor: &str) -> String {
    let section = anchor.split('.').next().unwrap_or(anchor);
    if section == anchor {
        format!("{}/{}", SETTINGS, section)
    } else {
        format!("{}/{}#{}", SETTINGS, section, anchor)
    }
}

fn trigger(row_id: &str) -> String {
    format!("{} [aria-haspopup=dialog]", row(row_id))
}

async fn visit(app: &App, section: &str) {
    app.goto(&href(section)).await;
    app.see(&row(&format!("{}-title", section))).await;
    settle(app, section).await;
}

async fn pick(app: &App, row_id: &str, value: &str) {
    let script = "const row = document.getElementById(arguments[0]); if (!row) return 'no row'; const buttons = [...row.querySelectorAll('[role=radio]')].filter((b) => b.dataset.value === arguments[1]); const live = buttons.find((b) => !b.closest('[inert]')) ?? buttons[0]; if (!live) return 'no option'; live.scrollIntoView({ block: 'center' }); live.click(); return 'ok'";
    let answer = app.js_with(script, vec![json!(row_id), json!(value)]).await;
    assert_eq!(answer, json!("ok"), "{} has no option {}", row_id, value);
}

async fn picked(app: &App, row_id: &str) -> Option<String> {
    let script = "const row = document.getElementById(arguments[0]); const on = row && [...row.querySelectorAll('[role=radio][aria-checked=true]')].find((b) => !b.closest('[inert]')); return on ? on.dataset.value : null";
    app.js_with(script, vec![json!(row_id)]).await.as_str().map(String::from)
}

async fn chosen(app: &App, row_id: &str) -> Option<String> {
    app.attr(&trigger(row_id), "data-value").await
}

async fn switch_in(app: &App, row_id: &str) -> bool {
    app.see(&format!("{} [role=switch]", row(row_id))).await;
    app.js_with("return document.getElementById(arguments[0]).querySelector('[role=switch]').getAttribute('aria-checked') === 'true'", vec![json!(row_id)]).await.as_bool().unwrap_or(false)
}

async fn toggle(app: &App, row_id: &str) {
    app.wait("the switch to accept a click", &format!("document.getElementById({})?.querySelector('[role=switch]')?.disabled === false", js_string(row_id))).await;
    app.click(&format!("{} [role=switch]", row(row_id))).await;
}

async fn settled(app: &App, key: &str, expected: &str) {
    let started = Instant::now();
    loop {
        let value = app.setting(key).await.unwrap_or_default();
        if value == expected {
            return;
        }
        assert!(started.elapsed() < Duration::from_secs(10), "{} stayed {:?}, expected {:?}", key, value, expected);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn appearance(app: &App) -> Value {
    serde_json::from_str(&app.setting("appearance.global").await.unwrap_or_else(|| "{}".into())).unwrap_or(Value::Null)
}

#[tokio::test]
async fn every_section_is_its_own_page_with_a_title_content_and_a_nav_entry() {
    let app = App::fresh().await;
    app.goto(SETTINGS).await;
    app.wait_url(&href("general")).await;
    assert_eq!(app.count(&tid("settings-nav")).await, sections().len());
    for section in sections() {
        let nav = format!("{}[data-section={}]", tid("settings-nav"), js_string(&section));
        app.click(&nav).await;
        app.wait_url(&href(&section)).await;
        app.see(&row(&format!("{}-title", section))).await;
        assert_eq!(app.attr(&nav, "aria-current").await.as_deref(), Some("location"), "{} is not marked current", section);
        let filled = app.js_with("const s = document.getElementById(arguments[0]); return s.querySelector('h1') !== null && s.children.length > 1 && s.textContent.trim().length > 20", vec![json!(section)]).await;
        assert_eq!(filled, json!(true), "{} renders empty", section);
        let others = app.js_with("return [...document.querySelectorAll('h1[id$=\"-title\"]')].map((h) => h.id).filter((id) => id !== arguments[0] + '-title')", vec![json!(section)]).await;
        assert_eq!(others, json!([]), "{} mounts other sections too", section);
    }
    assert!(!app.exists(&tid("settings-failure")).await, "the settings page shows a failure");
    app.goto(SETTINGS).await;
    app.wait_url(&href("developer")).await;
}

#[tokio::test]
async fn every_anchor_deep_link_opens_its_page_scrolls_to_and_highlights_its_row() {
    let app = App::fresh().await;
    app.goto(SETTINGS).await;
    let mut missing = Vec::new();
    for anchor in anchors() {
        follow(&app, &anchor.id).await;
        app.wait_url(&href(&anchor.id)).await;
        app.see(&row(&format!("{}-title", anchor.section))).await;
        settle(&app, &anchor.section).await;
        let exact = app.js_with("return !!document.getElementById(arguments[0])", vec![json!(anchor.id)]).await.as_bool().unwrap_or(false);
        let target = if exact { anchor.id.clone() } else { anchor.fallback.clone().unwrap_or_else(|| anchor.section.clone()) };
        if !exact && anchor.fallback.is_none() && !ONLY_WHEN_LOCKED.contains(&anchor.id.as_str()) {
            missing.push(anchor.id.clone());
            continue;
        }
        if anchor.id != anchor.section {
            follow(&app, &anchor.section).await;
            app.wait("the page to leave the anchor", "!location.hash").await;
            follow(&app, &anchor.id).await;
        }
        let pulses = target != anchor.section;
        let condition = format!(
            "(() => {{ const el = document.getElementById({}); if (!el) return false; const box = el.getBoundingClientRect(); const seen = box.bottom > 0 && box.top < innerHeight; const nav = document.querySelector('[data-testid=\"settings-nav\"][data-section={}]'); return seen && nav && nav.getAttribute('aria-current') === 'location' && ({} ? el.classList.contains('reveal-pulse') : true) }})()",
            js_string(&target),
            js_string(&anchor.section),
            pulses
        );
        let explain = format!("(() => {{ const el = document.getElementById({}); const box = el && el.getBoundingClientRect(); return {{ pulse: el && el.className, top: box && box.top, bottom: box && box.bottom, height: innerHeight, current: document.querySelector('[aria-current=location]')?.dataset.section, path: location.pathname, hash: location.hash }} }})()", js_string(&target));
        wait_or_explain(&app, &format!("{} to scroll into view and highlight", anchor.id), &condition, &explain, Duration::from_secs(10)).await;
        still(&app).await;
    }
    assert!(missing.is_empty(), "these setting anchors have no row on their page: {:?}", missing);
}

#[tokio::test]
async fn a_cold_deep_link_lands_on_its_row() {
    let app = App::fresh().await;
    app.goto(&href("messages.mail-delay")).await;
    let condition = "(() => { const el = document.getElementById('messages.mail-delay'); if (!el) return false; const box = el.getBoundingClientRect(); return box.top >= 0 && box.bottom <= innerHeight && document.querySelector('[aria-current=location]')?.dataset.section === 'messages' })()";
    let explain = "(() => { const el = document.getElementById('messages.mail-delay'); const box = el && el.getBoundingClientRect(); return { top: box && box.top, bottom: box && box.bottom, height: innerHeight, current: document.querySelector('[aria-current=location]')?.dataset.section } })()";
    wait_or_explain(&app, "the mail delay row to be in view with Messages marked", condition, explain, Duration::from_secs(10)).await;
    settle(&app, "messages").await;
    assert!(app.check(condition).await, "the mail delay row moved out of view after the page settled: {}", app.js(&format!("return JSON.stringify({})", explain)).await);
}

#[tokio::test]
async fn an_old_hash_link_moves_to_the_page_of_its_anchor() {
    let app = App::fresh().await;
    app.goto(SETTINGS).await;
    follow_raw(&app, &format!("{}#voice.hotkey", SETTINGS)).await;
    app.wait_url(&href("voice.hotkey")).await;
    app.wait("the hotkey row to pulse", "document.getElementById('voice.hotkey')?.classList.contains('reveal-pulse')").await;
}

#[tokio::test]
async fn search_finds_settings_opens_their_page_and_clears() {
    let app = App::fresh().await;
    visit(&app, "general").await;
    app.type_into(&tid("settings-search"), "mock").await;
    let hit = format!("{}[data-anchor=\"accounts.mock-state\"]", tid("settings-hit"));
    app.see(&hit).await;
    assert!(!app.exists(&tid("settings-nav")).await, "the section list stays while searching");
    app.click(&hit).await;
    app.wait_url(&href("accounts.mock-state")).await;
    app.wait("the mock state row to pulse", "document.getElementById('accounts.mock-state')?.classList.contains('reveal-pulse')").await;
    app.fill(&tid("settings-search"), "zzzz nothing matches").await;
    app.see(&tid("settings-search-empty")).await;
    app.fill(&tid("settings-search"), "undo delay").await;
    app.see(&format!("{}[data-anchor=\"messages.mail-delay\"]", tid("settings-hit"))).await;
    app.press(Some(&tid("settings-search")), "Enter", &[]).await;
    app.wait_url(&href("messages.mail-delay")).await;
    app.wait("the mail delay row to pulse", "document.getElementById('messages.mail-delay')?.classList.contains('reveal-pulse')").await;
    app.press(Some(&tid("settings-search")), "Escape", &[]).await;
    app.see(&tid("settings-nav")).await;
    assert_eq!(app.value(&tid("settings-search")).await, "");
}

fn draft_for(key: &str, current: &str) -> String {
    if key == "window.card" {
        return "420x264".into();
    }
    if key == "ollama.host" {
        return "http://127.0.0.1:8".into();
    }
    if key.ends_with("_url") {
        return format!("https://example.invalid/{}.bin", key.replace('.', "-"));
    }
    if key.contains("notice") {
        return "E2E {subject} {seconds}".into();
    }
    if key == "assistant.model" {
        return "haiku".into();
    }
    match current.parse::<f64>() {
        Ok(number) if number.fract() == 0.0 => format!("{}", number as i64 + 1),
        Ok(number) => format!("{}", (number * 100.0 + 5.0).round() / 100.0),
        Err(_) => format!("e2e-{}", key.replace('.', "-")),
    }
}

#[tokio::test]
async fn every_registry_text_field_round_trips_after_reload() {
    let app = App::fresh().await;
    let registry = app.engine("get_setting_registry", json!({})).await.unwrap();
    let mut wanted = Vec::new();
    for section in sections() {
        visit(&app, &section).await;
        let keys: Vec<String> = app.js("return [...document.querySelectorAll('input[id*=\".\"]')].map((el) => el.id)").await.as_array().unwrap().iter().filter_map(|v| v.as_str().map(String::from)).collect();
        for key in keys {
            let secret = key.ends_with("client_secret");
            let current = registry["values"][key.as_str()].as_str().or_else(|| registry["defaults"][key.as_str()].as_str()).unwrap_or("").to_string();
            let next = if secret { "e2e-secret".to_string() } else { draft_for(&key, &current) };
            app.fill(&row(&key), &next).await;
            app.press(Some(&row(&key)), "Enter", &[]).await;
            wanted.push((section.clone(), key, next, secret));
        }
    }
    for expected in ["mail.send_delay_seconds", "mail.send_notice", "voice.model_url", "intelligence.timeout_seconds", "window.card", "sync.interval_seconds", "mock.reply_seconds", "assistant.model", "ollama.host", "receipts.threshold", "actions.max_sends_per_hour", "microsoft.client_id"] {
        assert!(wanted.iter().any(|(_, k, _, _)| k == expected), "{} has no field on the settings pages", expected);
    }
    for (_, key, value, secret) in &wanted {
        if *secret {
            let started = Instant::now();
            while app.setting(key).await.unwrap_or_default().is_empty() {
                assert!(started.elapsed() < Duration::from_secs(10), "{} was not stored", key);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        } else {
            settled(&app, key, value).await;
        }
    }
    for section in sections() {
        visit(&app, &section).await;
        for (_, key, value, _) in wanted.iter().filter(|(s, _, _, secret)| *s == section && !*secret) {
            app.wait(&format!("{} to show {} after reload", key, value), &format!("document.getElementById({})?.value === {}", js_string(key), js_string(value))).await;
        }
    }
}

#[tokio::test]
async fn every_choice_round_trips_after_reload() {
    let app = App::fresh().await;
    let segmented: [(&str, &str, &str, &str); 12] = [
        ("messages", "messages.save-eyes", "on", "messages.save_eyes"),
        ("messages", "messages.sending", "mod", "messages.enter_sends"),
        ("messages", "messages.collapse", "30", "messages.collapse_lines"),
        ("messages", "messages.emoji-size", "small", "messages.emoji_size"),
        ("messages", "messages.edited", "off", "messages.show_edited"),
        ("messages", "messages.link-previews", "known-only", "links.previews"),
        ("messages", "messages.folded-mail", "preview", "messages.mail_summaries"),
        ("messages", "messages.default-reply", "all", "messages.default_reply"),
        ("messages", "messages.mail-delay-actions", "on", "mail.delay_actions"),
        ("messages", "messages.mail-quit", "send", "mail.quit_policy"),
        ("voice", "voice.language", "no", "voice.language"),
        ("voice", "voice.cleanup", "off", "voice.cleanup"),
    ];
    let dialogs: [(&str, &str, &str); 3] = [("messages", "messages.breaks", "180"), ("messages", "messages.timestamp-interval", "30"), ("privacy", "privacy.idle", "5")];
    for section in ["messages", "voice", "content", "privacy"] {
        visit(&app, section).await;
        for (_, row_id, value, _) in segmented.iter().filter(|c| c.0 == section) {
            pick(&app, row_id, value).await;
        }
        for (_, row_id, value) in dialogs.iter().filter(|c| c.0 == section) {
            app.select(&trigger(row_id), value).await;
        }
        if section == "messages" {
            pick(&app, "messages.time-format", "12h").await;
        }
        if section == "voice" {
            pick(&app, "voice.hotkey", "off").await;
        }
        if section == "content" {
            pick(&app, "content.rating", "g").await;
        }
    }
    for (_, _, value, key) in segmented {
        settled(&app, key, value).await;
    }
    settled(&app, "messages.section_gap", "180").await;
    settled(&app, "voice.hotkey", "off").await;
    settled(&app, "gif.rating", "g").await;
    settled(&app, "security.idle_minutes", "5").await;
    let started = Instant::now();
    loop {
        let profile = app.engine("get_profile", json!({})).await.unwrap();
        let settings = &profile["data"]["settings"];
        if settings["time_format"] == "12h" && settings["message_timestamp_interval"] == 30 {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(10), "the profile settings stayed {}", settings);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(app.engine("get_lock_state", json!({})).await.unwrap()["enabled"], json!(false));
    for section in ["messages", "voice", "content", "privacy"] {
        visit(&app, section).await;
        for (_, row_id, value, _) in segmented.iter().filter(|c| c.0 == section) {
            app.wait(&format!("{} to keep {}", row_id, value), &format!("(() => {{ const row = document.getElementById({}); const on = row && [...row.querySelectorAll('[role=radio][aria-checked=true]')].find((b) => !b.closest('[inert]')); return on && on.dataset.value === {} }})()", js_string(row_id), js_string(value))).await;
        }
        for (_, row_id, value) in dialogs.iter().filter(|c| c.0 == section) {
            app.wait(&format!("{} to keep {}", row_id, value), &format!("document.querySelector({})?.dataset.value === {}", js_string(&trigger(row_id)), js_string(value))).await;
        }
        if section == "messages" {
            assert_eq!(picked(&app, "messages.time-format").await.as_deref(), Some("12h"));
            assert_eq!(picked(&app, "messages.sending").await.as_deref(), Some("mod"));
        }
        if section == "voice" {
            assert_eq!(picked(&app, "voice.hotkey").await.as_deref(), Some("off"));
        }
        if section == "content" {
            assert_eq!(picked(&app, "content.rating").await.as_deref(), Some("g"));
        }
    }
}

#[tokio::test]
async fn a_selection_dialog_searches_and_picks_with_the_keyboard() {
    let app = App::fresh().await;
    visit(&app, "messages").await;
    app.click(&trigger("messages.breaks")).await;
    app.see(&tid("select-dialog")).await;
    app.type_into(&tid("select-search"), "day").await;
    app.wait("the list to filter", &format!("document.querySelectorAll({}).length === 1", js_string(&tid("select-option")))).await;
    app.press(Some(&tid("select-search")), "Enter", &[]).await;
    app.gone(&tid("select-dialog")).await;
    settled(&app, "messages.section_gap", "day").await;
    app.click(&trigger("messages.breaks")).await;
    app.see(&tid("select-dialog")).await;
    app.press(Some(&tid("select-search")), "Escape", &[]).await;
    app.gone(&tid("select-dialog")).await;
    assert_eq!(chosen(&app, "messages.breaks").await.as_deref(), Some("day"));
}

#[tokio::test]
async fn switches_and_selects_round_trip_after_reload() {
    let app = App::fresh().await;
    visit(&app, "ai").await;
    assert!(!switch_in(&app, "ai.enabled").await, "intelligence starts switched off in the test profile");
    visit(&app, "actions").await;
    assert!(switch_in(&app, "actions.enabled").await);
    toggle(&app, "actions.enabled").await;
    settled(&app, "actions.enabled", "false").await;
    visit(&app, "content").await;
    assert!(!switch_in(&app, "content.stickers").await);
    toggle(&app, "content.stickers").await;
    app.select(&trigger("content.opens-on"), "trending").await;
    settled(&app, "gif.stickers", "true").await;
    settled(&app, "gif.default_tab", "trending").await;
    visit(&app, "content").await;
    assert!(switch_in(&app, "content.stickers").await);
    assert_eq!(chosen(&app, "content.opens-on").await.as_deref(), Some("trending"));
    visit(&app, "actions").await;
    assert!(!switch_in(&app, "actions.enabled").await);
    toggle(&app, "actions.enabled").await;
    settled(&app, "actions.enabled", "true").await;
    visit(&app, "ai").await;
    toggle(&app, "ai.enabled").await;
    settled(&app, "assistant.enabled", "true").await;
    toggle(&app, "ai.enabled").await;
    settled(&app, "assistant.enabled", "false").await;
    visit(&app, "ai").await;
    assert!(!switch_in(&app, "ai.enabled").await, "intelligence did not stay off");
}

#[tokio::test]
async fn the_language_switches_between_english_and_norwegian_and_persists() {
    let app = App::fresh().await;
    visit(&app, "general").await;
    assert_eq!(app.js("return document.documentElement.lang").await, json!("en"));
    let english = app.text("[id=\"general-title\"]").await;
    pick(&app, "general.language", "nb").await;
    app.wait("the page to be Norwegian", "document.documentElement.lang === 'nb'").await;
    settled(&app, "app.locale", "nb").await;
    assert_ne!(app.text("[id=\"general-title\"]").await, english, "the section titles did not change language");
    visit(&app, "general").await;
    app.wait("Norwegian to survive a reload", "document.documentElement.lang === 'nb'").await;
    assert_eq!(picked(&app, "general.language").await.as_deref(), Some("nb"));
    pick(&app, "general.language", "en").await;
    app.wait("the page to be English again", "document.documentElement.lang === 'en'").await;
    settled(&app, "app.locale", "en").await;
    assert_eq!(app.text("[id=\"general-title\"]").await, english);
}

#[tokio::test]
async fn appearance_presets_modifiers_and_frost_apply_and_persist() {
    let app = App::fresh().await;
    visit(&app, "appearance").await;
    pick(&app, "appearance.theme", "night").await;
    app.wait("the night theme", "document.documentElement.dataset.theme === 'night' && document.documentElement.classList.contains('dark')").await;
    let gradient = format!("{}[data-key=\"gradient:aurora\"]", tid("appearance-background"));
    app.click(&gradient).await;
    let started = Instant::now();
    while appearance(&app).await["background"]["value"] != "aurora" {
        assert!(started.elapsed() < Duration::from_secs(10), "the gradient was not stored: {}", appearance(&app).await);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    app.click(&format!("{}[data-key=\"art:meadow-flow\"]", tid("appearance-background"))).await;
    pick(&app, "appearance.background", "vivid").await;
    pick(&app, "appearance.background", "fast").await;
    pick(&app, "appearance.surface", "glass").await;
    pick(&app, "appearance.frost", "dark").await;
    app.js("const el = document.getElementById('appearance-blur'); el.value = '12'; el.dispatchEvent(new Event('change', { bubbles: true }))").await;
    app.click(&format!("{}[data-key=\"amber\"]", tid("appearance-accent"))).await;
    let started = Instant::now();
    loop {
        let stored = appearance(&app).await;
        if stored["background"]["kind"] == "art" && stored["background"]["value"] == "meadow-flow" && stored["intensity"] == "vivid" && stored["motion"] == "fast" && stored["surface"] == "glass" && stored["tint"] == "dark" && stored["blur"] == 12 && stored["accent"] == "#d8892a" {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(10), "the appearance was not stored: {}", stored);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    app.see(".bg-glass").await;
    visit(&app, "appearance").await;
    app.see(".bg-glass").await;
    assert_eq!(app.js("return document.documentElement.dataset.theme").await, json!("night"));
    assert_eq!(picked(&app, "appearance.surface").await.as_deref(), Some("glass"));
    assert_eq!(picked(&app, "appearance.frost").await.as_deref(), Some("dark"));
    assert_eq!(app.js("return document.getElementById('appearance-blur').value").await, json!("12"));
    app.click(&format!("{}[data-key=\"none\"]", tid("appearance-background"))).await;
    app.click(&format!("{}[data-key=\"default\"]", tid("appearance-accent"))).await;
    pick(&app, "appearance.surface", "solid").await;
    let started = Instant::now();
    loop {
        let stored = appearance(&app).await;
        if stored["background"]["kind"] == "none" && stored["accent"].is_null() && stored["surface"] == "solid" {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(10), "the appearance did not return to plain: {}", stored);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    app.gone(".bg-glass").await;
    pick(&app, "appearance.theme", "day").await;
    app.wait("the day theme", "document.documentElement.dataset.theme === 'day'").await;
}

#[tokio::test]
async fn the_voice_section_shows_switched_off_then_needs_setup_without_downloading() {
    let app = App::fresh().await;
    visit(&app, "voice").await;
    app.intercept(&["post_voice_setup"]).await;
    assert!(!switch_in(&app, "voice.enabled").await);
    let state = format!("{}[data-present=\"false\"]", tid("voice-models-state"));
    app.see(&state).await;
    let setup = app.engine("get_voice_setup", json!({})).await.unwrap();
    assert_eq!(setup["ready"], json!(false));
    toggle(&app, "voice.enabled").await;
    settled(&app, "voice.enabled", "true").await;
    app.wait("the switch to show on", "document.getElementById('voice.enabled').querySelector('[role=switch]').getAttribute('aria-checked') === 'true'").await;
    let setup = app.engine("get_voice_setup", json!({})).await.unwrap();
    assert_eq!(setup["ready"], json!(false), "voice claims ready without models");
    let unmet: Vec<String> = setup["unmet"].as_array().unwrap().iter().filter_map(|r| r["key"].as_str().map(String::from)).collect();
    assert!(unmet.iter().any(|k| k == "voice.models") && !unmet.iter().any(|k| k == "voice.enabled"), "unmet after switching on: {:?}", unmet);
    app.wait("the set up button to be enabled", "document.querySelector('[data-testid=\"voice-setup\"]')?.disabled === false").await;
    app.click(&tid("voice-setup")).await;
    let started = Instant::now();
    while app.intercepted("post_voice_setup").await.is_empty() {
        assert!(started.elapsed() < Duration::from_secs(10), "set up did not ask the backend to download");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    app.see(&state).await;
    toggle(&app, "voice.enabled").await;
    settled(&app, "voice.enabled", "false").await;
}

#[tokio::test]
async fn the_composer_voice_button_leads_to_the_missing_voice_setting() {
    let app = App::fresh().await;
    let conversation = app.conversations().await.into_iter().find(|c| c["deleted_at"].is_null() && c["mode"].as_str() != Some("read")).expect("a writable conversation");
    let id = conversation["id"].as_str().unwrap().to_string();
    app.goto(&format!("/main/conversation/{}", id)).await;
    let mic = format!("{}[data-ready=\"false\"][data-requirement=\"voice.enabled\"]", tid("mic-button"));
    app.see(&mic).await;
    app.click(&format!("{} button", mic)).await;
    app.wait_url(&href("voice.enabled")).await;
    app.wait("the voice switch row to pulse", "document.getElementById('voice.enabled')?.classList.contains('reveal-pulse')").await;
    app.put_setting("voice.enabled", "true").await;
    app.goto(&format!("/main/conversation/{}", id)).await;
    let missing = format!("{}[data-ready=\"false\"][data-requirement=\"voice.models\"]", tid("mic-button"));
    app.see(&missing).await;
    app.click(&format!("{} button", missing)).await;
    app.wait_url(&href("voice.models")).await;
}

#[tokio::test]
async fn ai_model_pickers_sit_next_to_their_own_settings_and_not_on_the_ai_page() {
    let app = App::fresh().await;
    visit(&app, "ai").await;
    assert!(!app.exists(&row("ai.models")).await, "the AI page still lists a model per feature");
    let registry = app.engine("get_setting_registry", json!({})).await.unwrap();
    let features: Vec<String> = registry["features"].as_array().unwrap().iter().filter_map(|f| f["id"].as_str().map(String::from)).collect();
    assert_eq!(features, ["threads", "assistant", "voice"]);
    for (feature, section) in [("threads", "messages"), ("assistant", "actions"), ("voice", "voice")] {
        visit(&app, section).await;
        app.see(&row(&format!("{}.model-{}", section, feature))).await;
        app.see(&tid(&format!("model-choice-{}", feature))).await;
    }
    visit(&app, "ai").await;
    app.js("location.hash = '#ai.model-voice'").await;
    app.wait_url(&href("voice.model-voice")).await;
    let connections = app.engine("get_intelligence_connections", json!({})).await.unwrap();
    let connections = connections.as_array().cloned().unwrap_or_default();
    if connections.is_empty() {
        return;
    }
    visit(&app, "voice").await;
    let connection = connections[0]["id"].as_str().unwrap().to_string();
    let choice = format!("{}|", connection);
    app.select(&trigger("voice.model-voice"), &choice).await;
    settled(&app, "ai.feature.voice.connection", &connection).await;
    app.see(&tid("model-reset-voice")).await;
    visit(&app, "voice").await;
    assert_eq!(chosen(&app, "voice.model-voice").await, Some(choice));
    app.click(&tid("model-reset-voice")).await;
    settled(&app, "ai.feature.voice.connection", "").await;
    assert_eq!(app.setting("assistant.enabled").await.as_deref(), Some("false"));
    assert!(app.engine("get_assistant_runs", json!({ "limit": 20 })).await.unwrap().as_array().map(|r| r.is_empty()).unwrap_or(true), "a model ran");
}

#[tokio::test]
async fn the_developer_section_shows_the_plugin_and_cli_and_keeps_its_values() {
    let app = App::fresh().await;
    visit(&app, "developer").await;
    for id in ["developer.plugin", "developer.cli", "developer.ai-timeout", "developer.summary-concurrency", "developer.launch-card"] {
        app.see(&row(id)).await;
    }
    app.fill(&row("window.card"), "480x302").await;
    app.press(Some(&row("window.card")), "Enter", &[]).await;
    settled(&app, "window.card", "480x302").await;
    app.fill(&row("summaries.concurrency"), "2").await;
    app.press(Some(&row("summaries.concurrency")), "Enter", &[]).await;
    settled(&app, "summaries.concurrency", "2").await;
    visit(&app, "developer").await;
    assert_eq!(app.value(&row("window.card")).await, "480x302");
    assert_eq!(app.value(&row("summaries.concurrency")).await, "2");
}

async fn wait_route(app: &App, path: &str) {
    app.wait(&format!("the app to land on {}", path), &format!("location.pathname === {}", js_string(path))).await;
}

#[tokio::test]
async fn a_specific_startup_page_opens_after_a_relaunch() {
    let mut app = App::fresh().await;
    visit(&app, "general").await;
    pick(&app, "general.startup-page", "page").await;
    app.see(&tid("select-dialog")).await;
    app.click(&format!("{}[data-value=\"/main/calendar\"]", tid("select-option"))).await;
    app.gone(&tid("select-dialog")).await;
    settled(&app, "app.startup-page", "/main/calendar").await;
    app.wait("the calendar choice to be shown", &format!("document.querySelector({})?.dataset.value === '/main/calendar'", js_string(&tid("startup-choice")))).await;
    app.goto("/main/stats").await;
    app.relaunch_ready().await;
    wait_route(&app, "/main/calendar").await;
}

#[tokio::test]
async fn the_last_page_opens_after_a_relaunch() {
    let mut app = App::fresh().await;
    visit(&app, "general").await;
    assert_eq!(picked(&app, "general.startup-page").await.as_deref(), Some("last"));
    assert!(!app.exists(&tid("startup-choice")).await, "the page picker shows while the last page is chosen");
    app.goto("/main/stats").await;
    settled(&app, "app.last-route", "/main/stats").await;
    app.relaunch_ready().await;
    wait_route(&app, "/main/stats").await;
}

#[tokio::test]
async fn mock_connection_error_states_surface_in_connections() {
    let app = App::fresh().await;
    app.put_setting("providers.retry_attempts", "1").await;
    app.put_setting("providers.retry_max_seconds", "1").await;
    app.goto(&href("accounts.mock-state")).await;
    for kind in MOCK_KINDS {
        app.see(&tid(&format!("mock-state-{}", app.mock_provider(kind)))).await;
    }
    let cases: [(&str, &str, &str); 4] = [("teams", "expired", "401"), ("slack", "rate_limited", "ratelimited"), ("gmail", "offline", "error sending request"), ("imap", "expired", "AUTHENTICATIONFAILED")];
    for (kind, state, needle) in cases {
        let provider = app.mock_provider(kind);
        app.select(&tid(&format!("mock-state-{}", provider)), state).await;
        let started = Instant::now();
        while app.engine("get_mock_mode", json!({})).await.unwrap()["states"][provider.as_str()] != state {
            assert!(started.elapsed() < Duration::from_secs(10), "{} never went {}", kind, state);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let sync = tid(&format!("provider-sync-{}", provider));
        app.wait("the sync button to be free", &format!("document.querySelector({})?.disabled === false", js_string(&sync))).await;
        app.click(&sync).await;
        wait_or_explain(&app, &format!("{} {} to show its error", kind, state), &format!("document.querySelector('[data-testid=\"connections-failure\"]')?.textContent.includes({})", js_string(needle)), "({ failure: document.querySelector('[data-testid=\"connections-failure\"]')?.textContent, busy: [...document.querySelectorAll('[data-testid^=provider-sync]')].map((b) => b.disabled) })", Duration::from_secs(60)).await;
        app.click(&format!("{} button", tid("connections-failure"))).await;
        app.gone(&tid("connections-failure")).await;
    }
    let teams = app.mock_provider("teams");
    for kind in ["teams", "slack", "gmail", "imap"] {
        app.select(&tid(&format!("mock-state-{}", app.mock_provider(kind))), "ok").await;
    }
    let started = Instant::now();
    loop {
        let mode = app.engine("get_mock_mode", json!({})).await.unwrap();
        if MOCK_KINDS.iter().all(|kind| mode["states"][app.mock_provider(kind).as_str()] == "ok") {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(10), "the mock clients did not all return to ok: {}", mode);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let sync = tid(&format!("provider-sync-{}", teams));
    app.wait("the teams sync button to be free", &format!("document.querySelector({})?.disabled === false", js_string(&sync))).await;
    app.click(&sync).await;
    app.wait_for("teams to sync again", &format!("document.querySelector('[data-testid=\"provider-row\"][data-provider={}]')?.dataset.connected === 'true'", js_string(&teams)), Duration::from_secs(30)).await;
    assert!(!app.exists(&tid("connections-failure")).await, "a healthy sync still shows a failure");
}

#[tokio::test]
async fn long_lists_show_ten_and_expand_to_all() {
    let app = App::fresh().await;
    for index in 0..12 {
        app.engine("post_tag", json!({ "input": { "name": format!("E2E tag {:02}", index), "kind": "tag", "color": "#1f9d55" } })).await.unwrap();
    }
    visit(&app, "organise").await;
    let tags = format!("{} ul > li", row("organise.tags"));
    app.wait("ten tags to show", &format!("document.querySelectorAll({}).length === 10", js_string(&tags))).await;
    let more = format!("{} {}", row("organise.tags"), tid("section-more"));
    let total = app.engine("get_tags", json!({})).await.unwrap().as_array().map(|all| all.iter().filter(|t| t["kind"] == "tag").count()).unwrap_or(0);
    assert!(app.text(&more).await.contains(&total.to_string()), "the toggle does not count all {} tags", total);
    app.click(&more).await;
    app.wait("every tag to show", &format!("document.querySelectorAll({}).length === {}", js_string(&tags), total)).await;
    app.click(&more).await;
    app.wait("the tags to fold back to ten", &format!("document.querySelectorAll({}).length === 10", js_string(&tags))).await;
}

#[tokio::test]
async fn the_sidebar_layout_is_one_list_with_a_footer_boundary() {
    let app = App::fresh().await;
    visit(&app, "organise").await;
    app.see(&tid("sidebar-boundary")).await;
    assert_eq!(app.count(&format!("{} {}", row("organise.sidebar"), tid("sidebar-layout"))).await, 1);
    let key = app.js(&format!("return [...document.querySelectorAll({})].filter((el) => el.dataset.zone === 'top' && !el.parentElement.closest('.ml-6')).pop()?.dataset.key ?? null", js_string(&tid("sidebar-entry")))).await.as_str().map(String::from).expect("an entry above the boundary");
    let keyed = |layout: &Value, zone: &str| layout[zone].as_array().cloned().unwrap_or_default().iter().any(|e| e["key"].as_str() == Some(key.as_str()) || e["id"].as_str() == Some(key.as_str()));
    let down = format!("{}[data-key={}] button[aria-label={}]", tid("sidebar-entry"), js_string(&key), js_string("Move down"));
    let up = format!("{}[data-key={}] button[aria-label={}]", tid("sidebar-entry"), js_string(&key), js_string("Move up"));
    for _ in 0..3 {
        app.click(&down).await;
        tokio::time::sleep(Duration::from_millis(400)).await;
        if keyed(&app.engine("get_sidebar_layout", json!({})).await.unwrap(), "footer") {
            break;
        }
    }
    let layout = app.engine("get_sidebar_layout", json!({})).await.unwrap();
    assert!(keyed(&layout, "footer") && !keyed(&layout, "top"), "moving down past the boundary did not move {} into the footer: {}", key, layout);
    app.wait("the entry to sit below the boundary", &format!("document.querySelector({})?.closest('[data-zone]')?.dataset.zone === 'footer'", js_string(&down))).await;
    app.click(&up).await;
    let started = Instant::now();
    while !keyed(&app.engine("get_sidebar_layout", json!({})).await.unwrap(), "top") {
        assert!(started.elapsed() < Duration::from_secs(10), "moving up past the boundary did not move {} back", key);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn app_registrations_show_their_fields_under_accounts_and_save() {
    let app = App::fresh().await;
    app.goto(&href("accounts.registrations")).await;
    for key in ["microsoft.client_id", "microsoft.client_secret", "microsoft.tenant", "google.client_id", "google.client_secret", "slack.client_id", "slack.client_secret"] {
        app.see(&format!("{} {}", row("accounts.registrations"), row(key))).await;
    }
    assert!(app.visible(&row("microsoft.client_id")).await, "the Microsoft client id field is not on screen");
    app.fill(&row("google.client_id"), "e2e-client.apps.googleusercontent.com").await;
    app.press(Some(&row("google.client_id")), "Enter", &[]).await;
    settled(&app, "google.client_id", "e2e-client.apps.googleusercontent.com").await;
}

async fn wait_or_explain(app: &App, what: &str, condition: &str, explain: &str, timeout: Duration) {
    let started = Instant::now();
    while !app.check(condition).await {
        if started.elapsed() > timeout {
            panic!("timed out waiting for {}: {}", what, app.js(&format!("return JSON.stringify({})", explain)).await);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn settle(app: &App, section: &str) {
    let fields = "[...document.querySelectorAll('[data-testid=\"setting-field\"]')].every((el) => el.dataset.ready === 'true')";
    let late = match section {
        "accounts" => "document.querySelector('[data-testid=\"provider-row\"]')",
        "voice" => "document.getElementById('voice.enabled')?.querySelector('[role=switch]:not([disabled])')",
        _ => "true",
    };
    app.wait(&format!("the late loading {} settings to arrive", section), &format!("{} && {}", fields, late)).await;
}

async fn follow(app: &App, anchor: &str) {
    follow_raw(app, &href(anchor)).await;
}

async fn follow_raw(app: &App, link: &str) {
    app.js_with("const link = document.createElement('a'); link.href = arguments[0]; document.body.appendChild(link); link.click(); link.remove()", vec![json!(link)]).await;
}

async fn still(app: &App) {
    let started = Instant::now();
    let mut last = Value::Null;
    let mut same = 0;
    while same < 3 {
        let now = app.js("return [...document.querySelectorAll('.overflow-y-auto')].map((el) => el.scrollTop)").await;
        same = if now == last { same + 1 } else { 0 };
        last = now;
        assert!(started.elapsed() < Duration::from_secs(10), "the settings page kept scrolling");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
