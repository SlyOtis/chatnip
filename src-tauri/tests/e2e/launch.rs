use super::support::{tid, App, MOCK_KINDS};
use serde_json::json;

#[tokio::test]
async fn the_app_opens_on_the_inbox_with_navigation() {
    let app = App::fresh().await;
    app.wait_url("/main/conversation").await;
    assert!(app.exists("#shell").await);
}

#[tokio::test]
async fn the_calendar_and_threads_pages_load() {
    let app = App::fresh().await;
    app.goto("/main/calendar").await;
    app.goto("/main/threads").await;
    assert!(app.exists(&tid("app-root")).await);
}

#[tokio::test]
async fn the_window_grows_from_the_card_to_the_app_frame() {
    let app = App::fresh().await;
    let (_, _, width, height) = frame(&app).await;
    assert!(width > 400.0 && height > 300.0, "the window stayed at the card size: {}x{}", width, height);
}

#[tokio::test]
async fn the_harness_reaches_every_mock_client_and_intercepts_commands() {
    let app = App::fresh().await;
    for kind in MOCK_KINDS {
        let id = app.mock_provider(kind);
        assert!(app.providers().await.iter().any(|p| p["id"].as_str() == Some(id.as_str())), "{} is not connected", kind);
    }
    app.intercept(&["post_attachment_open"]).await;
    let _ = app.invoke("post_attachment_open", json!({ "path": "x" })).await;
    assert_eq!(app.intercepted("post_attachment_open").await.len(), 1);
}


async fn scale(app: &App) -> f64 {
    app.client.execute("return window.devicePixelRatio || 1", vec![]).await.ok().and_then(|v| v.as_f64()).unwrap_or(2.0)
}

async fn frame(app: &App) -> (f64, f64, f64, f64) {
    let ratio = scale(app).await;
    let (x, y, width, height) = app.client.get_window_rect().await.unwrap();
    (x as f64 / ratio, y as f64 / ratio, width as f64 / ratio, height as f64 / ratio)
}

async fn saved_frame(app: &App) -> serde_json::Value {
    app.engine("get_profile", json!({})).await.unwrap()["data"]["settings"]["window_position"].clone()
}

#[tokio::test]
async fn the_launch_card_shows_then_grows_into_the_app_frame() {
    let app = App::fresh_unready().await;
    let mut card = false;
    let mut splash = false;
    let started = std::time::Instant::now();
    let sample = format!("return [innerWidth, innerHeight, !!document.querySelector('{}'), !!document.querySelector('{}[data-ready=\"true\"]')]", tid("launch-splash"), tid("app-root"));
    loop {
        let state = app.client.execute(&sample, vec![]).await.unwrap_or_default();
        let width = state[0].as_f64().unwrap_or(0.0);
        let height = state[1].as_f64().unwrap_or(0.0);
        if (width - 360.0).abs() < 2.0 && (height - 227.0).abs() < 2.0 {
            card = true;
            splash |= state[2].as_bool().unwrap_or(false);
        }
        if state[3].as_bool().unwrap_or(false) && width > 600.0 {
            break;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(30), "the window never grew past the card");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(card, "the window never showed at the 360x227 card size");
    assert!(splash, "the launch splash was not on screen during the card");
    app.wait("the splash to fade out", &format!("!document.querySelector('{}') || document.querySelector('{}').classList.contains('launch-out')", tid("launch-splash"), tid("launch-splash"))).await;
    let (_, _, width, height) = frame(&app).await;
    assert!(width >= 300.0 && height >= 500.0, "the grown window is below the app minimum: {}x{}", width, height);
}

#[tokio::test]
async fn the_window_frame_is_restored_after_relaunch() {
    let mut app = App::fresh().await;
    let ratio = scale(&app).await;
    app.wait("the launch to settle", &format!("!document.querySelector('{}')", tid("launch-splash"))).await;
    let before = saved_frame(&app).await;
    let target = (140.0, 120.0, 1000.0, 700.0);
    app.client.set_window_rect((target.0 * ratio) as u32, (target.1 * ratio) as u32, (target.2 * ratio) as u32, (target.3 * ratio) as u32).await.unwrap();
    let started = std::time::Instant::now();
    while saved_frame(&app).await == before {
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "the moved window was never saved");
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    let placed = frame(&app).await;
    app.relaunch_ready().await;
    let started = std::time::Instant::now();
    loop {
        let now = frame(&app).await;
        if (now.2 - placed.2).abs() < 3.0 && (now.3 - placed.3).abs() < 3.0 && (now.0 - placed.0).abs() < 3.0 && (now.1 - placed.1).abs() < 3.0 {
            break;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "the window came back at {:?}, not {:?}", now, placed);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

async fn seeded_actions(app: &App) -> usize {
    app.engine("get_actions", json!({})).await.unwrap().as_array().map(|a| a.len()).unwrap_or(0)
}

#[tokio::test]
async fn a_debug_start_on_a_fresh_home_lands_in_main_with_every_mock_client_seeded() {
    let mut app = App::blank(Some("true")).await;
    app.ready().await;
    app.wait_url("/main").await;
    assert!(!app.url().await.contains("/setup"), "the debug start went through setup: {}", app.url().await);
    let profile = app.engine("get_profile", json!({})).await.unwrap();
    assert_eq!(profile["data"]["setup_step"], json!(3), "{}", profile);
    let providers = app.providers().await;
    let mut kinds: Vec<String> = providers.iter().filter(|p| p["provider_type"].as_str() == Some("mock")).filter_map(|p| p["namespace"].as_str().map(String::from)).collect();
    kinds.sort();
    let mut expected: Vec<String> = MOCK_KINDS.iter().map(|k| k.to_string()).collect();
    expected.sort();
    assert_eq!(kinds, expected, "{:?}", providers);
    assert!(providers.iter().all(|p| !p["last_sync_at"].is_null()), "a mock client was not synced: {:?}", providers);
    let actions = seeded_actions(&app).await;
    assert!(actions > 20, "the mock seed did not run: {} actions", actions);
    app.relaunch_ready().await;
    app.wait_url("/main").await;
    assert!(!app.url().await.contains("/setup"), "the second debug start went through setup: {}", app.url().await);
    let again = app.engine("get_profile", json!({})).await.unwrap();
    assert_eq!(again["data"]["id"], profile["data"]["id"], "the second start made another profile");
    assert_eq!(app.providers().await.len(), MOCK_KINDS.len(), "the second start connected the mock clients again");
    assert_eq!(seeded_actions(&app).await, actions, "the second start seeded twice");
}

#[tokio::test]
async fn a_non_debug_start_on_a_fresh_home_is_left_to_onboarding() {
    let app = App::blank(Some("false")).await;
    app.wait_url("/setup/welcome").await;
    let profile = app.engine("get_profile", json!({})).await.unwrap();
    assert!(profile["data"].is_null(), "{}", profile);
    assert!(app.providers().await.is_empty());
    app.click(&tid("setup-start")).await;
    app.wait_url("/setup/providers").await;
    app.see(&tid("setup-providers-continue")).await;
    assert!(!app.exists(&tid("setup-test-clients")).await, "test clients show with debug off");
}

#[tokio::test]
async fn onboarding_connects_a_test_client_and_lands_on_the_inbox_after_relaunch() {
    let mut app = App::blank(None).await;
    app.wait_url("/setup/welcome").await;
    assert!(app.providers().await.is_empty());
    app.put_setting("mock.enabled", "true").await;
    app.click(&tid("setup-start")).await;
    app.wait_url("/setup/providers").await;
    app.see(&tid("setup-test-clients")).await;
    app.wait("continue to need an account", &format!("document.querySelector('{}').disabled", tid("setup-providers-continue"))).await;
    app.click(&tid("setup-mock-teams")).await;
    app.wait("continue to unlock", &format!("!document.querySelector('{}').disabled", tid("setup-providers-continue"))).await;
    assert_eq!(app.providers().await.len(), 1);
    app.click(&tid("setup-providers-continue")).await;
    app.wait_url("/setup/intelligence").await;
    app.click(&tid("setup-finish")).await;
    app.wait_url("/main/conversation").await;
    app.ready().await;
    let profile = app.engine("get_profile", json!({})).await.unwrap();
    assert_eq!(profile["data"]["setup_step"], json!(3), "{}", profile);
    app.relaunch_ready().await;
    app.wait_url("/main/conversation").await;
}

#[tokio::test]
async fn onboarding_cannot_finish_without_an_account() {
    let app = App::blank(Some("false")).await;
    app.wait_url("/setup/welcome").await;
    app.click(&tid("setup-start")).await;
    app.wait_url("/setup/providers").await;
    app.wait("continue to be disabled", &format!("document.querySelector('{}').disabled", tid("setup-providers-continue"))).await;
    app.client.goto("tauri://localhost/setup/intelligence").await.unwrap();
    app.wait_url("/setup/intelligence").await;
    app.see(&tid("setup-finish")).await;
    app.click(&tid("setup-finish")).await;
    app.wait_url("/setup/providers").await;
    assert!(app.providers().await.is_empty());
}

#[tokio::test]
async fn every_connection_in_the_e2e_home_is_a_mock_client() {
    let app = App::fresh().await;
    let providers = app.providers().await;
    assert_eq!(providers.len(), MOCK_KINDS.len(), "{:?}", providers);
    for provider in &providers {
        assert_eq!(provider["provider_type"].as_str(), Some("mock"), "{}", provider);
    }
    assert!(app.socket().starts_with(&app.home), "the engine socket {} is outside the E2E home", app.socket().display());
    let mode = app.engine("get_mock_mode", serde_json::json!({})).await.unwrap();
    assert_eq!(mode["forced"].as_bool(), Some(true), "{}", mode);
}

#[tokio::test]
async fn the_ai_connection_is_the_stand_in_and_intelligence_stays_off() {
    let app = App::fresh().await;
    assert_eq!(app.setting("assistant.enabled").await.as_deref(), Some("false"));
    let connections = app.engine("get_intelligence_connections", json!({})).await.unwrap();
    let binaries: Vec<String> = connections.as_array().cloned().unwrap_or_default().iter().filter_map(|c| c["config"]["binary"].as_str().map(String::from)).collect();
    assert!(!binaries.is_empty(), "the stand in claude was not picked up: {}", connections);
    for binary in binaries {
        assert!(binary.ends_with("tests/fixtures/bin/claude"), "the E2E instance found a real model binary: {}", binary);
    }
}

#[tokio::test]
async fn the_seeded_template_holds_each_seeded_action_once() {
    let app = App::fresh().await;
    let actions = app.engine("get_actions", json!({})).await.unwrap();
    let mut names: Vec<String> = actions.as_array().cloned().unwrap_or_default().iter().filter_map(|a| a["name"].as_str().map(|name| format!("{} in {}", name, a["group_id"]))).collect();
    assert!(names.len() > 20, "the seed made too few actions: {}", names.len());
    let total = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), total, "the seed made some actions twice");
    let groups = app.engine("get_action_groups", json!({})).await.unwrap();
    let mut keys: Vec<String> = groups.as_array().cloned().unwrap_or_default().iter().map(|g| format!("{} in {}", g["name"], g["parent_id"])).collect();
    let count = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), count, "the seed made some action groups twice");
}
