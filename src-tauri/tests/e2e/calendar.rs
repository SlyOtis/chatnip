use super::support::{js_string, tid, App};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn q(name: &str) -> String {
    js_string(&tid(name))
}

fn sel(name: &str, attr: &str, value: &str) -> String {
    format!("{}[data-{}=\"{}\"]", tid(name), attr, value)
}

fn view(value: &str) -> String {
    format!("{} [data-value=\"{}\"]", tid("calendar-view"), value)
}

async fn events_between(app: &App, days_back: i64, days_ahead: i64) -> Vec<Value> {
    let start = (chrono::Utc::now() - chrono::Duration::days(days_back)).to_rfc3339();
    let end = (chrono::Utc::now() + chrono::Duration::days(days_ahead)).to_rfc3339();
    app.engine("get_events", json!({ "input": { "start": start, "end": end } })).await.unwrap().as_array().cloned().unwrap_or_default()
}

async fn week_events(app: &App) -> Vec<Value> {
    let range = app.js("const start = new Date(); start.setDate(start.getDate() - ((start.getDay() + 6) % 7)); start.setHours(0, 0, 0, 0); const end = new Date(start); end.setDate(start.getDate() + 6); end.setHours(23, 59, 59, 999); return { start: start.toISOString(), end: end.toISOString() }").await;
    app.engine("get_events", json!({ "input": range })).await.unwrap().as_array().cloned().unwrap_or_default()
}

async fn event_titled(app: &App, title: &str) -> Option<Value> {
    events_between(app, 30, 60).await.into_iter().find(|e| e["title"] == title)
}

async fn open_calendar(app: &App) {
    app.goto("/main/calendar").await;
    app.see(&sel("calendar-grid", "view", "week")).await;
}

#[tokio::test]
async fn the_week_grid_shows_every_seeded_event_of_the_week() {
    let app = App::fresh().await;
    open_calendar(&app).await;
    let expected: Vec<String> = week_events(&app).await.iter().filter(|e| e["status"] != "dismissed").map(|e| e["id"].as_str().unwrap().to_string()).collect();
    assert!(!expected.is_empty(), "the mock calendars seeded nothing this week");
    app.wait("every event of the week is drawn", &format!("new Set([...document.querySelectorAll({})].map((el) => el.dataset.id)).size === {}", q("calendar-event"), expected.len())).await;
    for id in &expected {
        assert!(app.exists(&sel("calendar-event", "id", id)).await, "event {} is missing from the grid", id);
    }
    assert!(app.exists(&format!("{}[data-status=\"proposed\"]", tid("calendar-event"))).await, "parsed proposals are not drawn");
}

#[tokio::test]
async fn navigation_moves_by_the_view_and_today_comes_back() {
    let app = App::fresh().await;
    open_calendar(&app).await;
    app.click(&view("month")).await;
    app.see(&sel("calendar-grid", "view", "month")).await;
    let now = app.text(&tid("calendar-title")).await;
    assert!(app.count(&tid("calendar-day")).await >= 28);
    app.click(&tid("calendar-next")).await;
    app.wait("the month moves on", &format!("document.querySelector({}).textContent.trim() !== {}", q("calendar-title"), js_string(&now))).await;
    let next = app.text(&tid("calendar-title")).await;
    app.click(&tid("calendar-prev")).await;
    app.click(&tid("calendar-prev")).await;
    app.wait("the month moves back", &format!("![{}, {}].includes(document.querySelector({}).textContent.trim())", js_string(&now), js_string(&next), q("calendar-title"))).await;
    app.click(&tid("calendar-today")).await;
    app.wait("today is back", &format!("document.querySelector({}).textContent.trim() === {}", q("calendar-title"), js_string(&now))).await;
    app.click(&view("day")).await;
    app.see(&sel("calendar-grid", "view", "day")).await;
    assert_eq!(app.count(&tid("calendar-slot")).await, 24);
    let today = app.text(&tid("calendar-title")).await;
    app.click(&tid("calendar-next")).await;
    app.wait("the day moves on", &format!("document.querySelector({}).textContent.trim() !== {}", q("calendar-title"), js_string(&today))).await;
    app.click(&view("week")).await;
    app.see(&sel("calendar-grid", "view", "week")).await;
    assert_eq!(app.count(&tid("calendar-slot")).await, 24 * 7);
}

#[tokio::test]
async fn an_event_in_the_month_view_opens_its_detail_and_not_a_new_event() {
    let app = App::fresh().await;
    open_calendar(&app).await;
    app.click(&view("month")).await;
    app.see(&sel("calendar-grid", "view", "month")).await;
    app.see(&tid("calendar-event")).await;
    let id = app.attr(&tid("calendar-event"), "data-id").await.unwrap();
    app.click(&sel("calendar-event", "id", &id)).await;
    app.see(&tid("calendar-detail")).await;
    assert!(!app.exists(&tid("calendar-create-sheet")).await, "clicking an event also opened the new event sheet");
    app.press(None, "Escape", &[]).await;
    app.gone(&tid("calendar-detail")).await;
    app.click(&tid("calendar-day")).await;
    app.see(&tid("calendar-create-sheet")).await;
}

#[tokio::test]
async fn a_local_event_is_created_shown_opened_and_deleted() {
    let app = App::fresh().await;
    open_calendar(&app).await;
    app.click(&format!("{}[data-day=\"2\"][data-hour=\"14\"]", tid("calendar-slot"))).await;
    app.see(&tid("calendar-create-sheet")).await;
    assert_eq!(app.value(&tid("calendar-create-start-time")).await, "14:00");
    assert_eq!(app.value(&tid("calendar-create-end-time")).await, "15:00");
    let defaults: Vec<String> = app.js(&format!("return [...document.querySelectorAll({})].filter((el) => el.getAttribute('aria-checked') === 'true').map((el) => el.dataset.id)", q("calendar-create-target"))).await.as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert!(!defaults.is_empty(), "no calendar is picked by default");
    let local = "local";
    app.click(&sel("calendar-create-target", "id", local)).await;
    app.see(&format!("{}[aria-checked=\"true\"]", sel("calendar-create-target", "id", local))).await;
    for id in &defaults {
        app.click(&sel("calendar-create-target", "id", id)).await;
        app.see(&format!("{}[aria-checked=\"false\"]", sel("calendar-create-target", "id", id))).await;
        assert!(app.exists(&format!("{}[aria-checked=\"true\"]", sel("calendar-create-target", "id", local))).await, "unpicking one calendar unpicked every calendar");
    }
    app.fill(&tid("calendar-create-title"), "E2E planning").await;
    app.fill(&tid("calendar-create-location"), "Room 42").await;
    app.fill(&tid("calendar-create-attendee-input"), "guest@example.com").await;
    app.press(Some(&tid("calendar-create-attendee-input")), "Enter", &[]).await;
    app.see(&sel("calendar-create-attendee", "email", "guest@example.com")).await;
    app.fill(&tid("calendar-create-attendee-input"), "other@example.com").await;
    app.press(Some(&tid("calendar-create-attendee-input")), "Enter", &[]).await;
    app.click(&sel("calendar-create-attendee", "email", "other@example.com")).await;
    app.gone(&sel("calendar-create-attendee", "email", "other@example.com")).await;
    app.fill(&tid("calendar-create-notes"), "bring the plan").await;
    app.click(&tid("calendar-create-submit")).await;
    app.gone(&tid("calendar-create-sheet")).await;

    let started = Instant::now();
    let created = loop {
        if let Some(found) = event_titled(&app, "E2E planning").await {
            break found;
        }
        assert!(started.elapsed() < Duration::from_secs(20), "the event was never stored");
        tokio::time::sleep(Duration::from_millis(150)).await;
    };
    assert_eq!(created["location"], "Room 42");
    assert_eq!(created["notes"], "bring the plan");
    assert_eq!(created["attendees"], json!(["guest@example.com"]));
    assert!(created["sources"].as_array().map(|s| s.iter().all(|s| s["calendar_id"].is_null())).unwrap_or(true), "a local only event landed in a provider calendar");
    let id = created["id"].as_str().unwrap().to_string();
    app.see(&sel("calendar-event", "id", &id)).await;

    app.click(&sel("calendar-event", "id", &id)).await;
    app.see(&sel("calendar-detail-title", "id", &id)).await;
    assert_eq!(app.text(&tid("calendar-detail-title")).await, "E2E planning");
    app.click(&tid("calendar-detail-delete")).await;
    app.gone(&sel("calendar-event", "id", &id)).await;
    assert!(event_titled(&app, "E2E planning").await.is_none());
}

#[tokio::test]
async fn an_invitation_is_answered_through_the_mock_calendar() {
    let app = App::fresh().await;
    open_calendar(&app).await;
    let event = event_titled(&app, "Arkitekturgjennomgang Tidewater").await.expect("the seeded invitation is missing");
    let id = event["id"].as_str().unwrap().to_string();
    let source = event["sources"].as_array().unwrap().iter().find(|s| s["response_status"] == "notResponded").expect("the invitation has no open answer").clone();
    let source_id = source["event_id"].as_str().unwrap().to_string();
    let starts = chrono::DateTime::parse_from_rfc3339(event["starts_at"].as_str().unwrap()).unwrap().with_timezone(&chrono::Local).date_naive();
    let today = chrono::Local::now().date_naive();
    let monday = today - chrono::Duration::days(i64::from(chrono::Datelike::weekday(&today).num_days_from_monday()));
    if starts >= monday + chrono::Duration::days(7) {
        app.click(&tid("calendar-next")).await;
    }
    app.click(&sel("calendar-event", "id", &id)).await;
    app.see(&tid("calendar-detail")).await;
    app.see(&sel("calendar-detail-response", "source", &source_id)).await;
    app.click(&tid("calendar-answer-accepted")).await;
    app.see(&format!("{}[data-status=\"accepted\"]", sel("calendar-detail-response", "source", &source_id))).await;
    assert!(!app.exists(&tid("calendar-answer-failure")).await);
    let started = Instant::now();
    while !event_titled(&app, "Arkitekturgjennomgang Tidewater").await.map(|e| e["sources"].as_array().unwrap().iter().any(|s| s["event_id"] == source_id.as_str() && s["response_status"] == "accepted")).unwrap_or(false) {
        assert!(started.elapsed() < Duration::from_secs(20), "the answer never reached the mock calendar");
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

#[tokio::test]
async fn proposals_are_confirmed_and_dismissed_from_the_side_list() {
    let app = App::fresh().await;
    open_calendar(&app).await;
    let proposed = app.engine("get_proposed_events", json!({})).await.unwrap().as_array().cloned().unwrap();
    assert!(proposed.len() >= 2, "the seed has fewer than two proposals");
    app.wait("every proposal is listed", &format!("document.querySelectorAll({}).length === {}", q("calendar-proposed"), proposed.len())).await;
    let first = proposed[0]["id"].as_str().unwrap().to_string();
    let second = proposed[1]["id"].as_str().unwrap().to_string();
    app.click(&format!("{} {}", sel("calendar-proposed", "id", &first), tid("calendar-proposed-confirm"))).await;
    app.gone(&sel("calendar-proposed", "id", &first)).await;
    app.click(&format!("{} {}", sel("calendar-proposed", "id", &second), tid("calendar-proposed-detail"))).await;
    app.see(&sel("calendar-detail-title", "id", &second)).await;
    app.click(&tid("calendar-detail-dismiss")).await;
    app.gone(&sel("calendar-proposed", "id", &second)).await;
    let all = app.engine("get_events", json!({ "input": { "start": (chrono::Utc::now() - chrono::Duration::days(60)).to_rfc3339(), "end": (chrono::Utc::now() + chrono::Duration::days(60)).to_rfc3339(), "include_dismissed": true } })).await.unwrap();
    let status = |id: &str| all.as_array().unwrap().iter().find(|e| e["id"] == id).map(|e| e["status"].clone()).unwrap_or(Value::Null);
    assert_eq!(status(&first), "confirmed");
    assert_eq!(status(&second), "dismissed");
    let left = app.engine("get_proposed_events", json!({})).await.unwrap().as_array().unwrap().len();
    assert_eq!(left, proposed.len() - 2);
}

#[tokio::test]
async fn a_calendar_switched_off_hides_its_events() {
    let app = App::fresh().await;
    open_calendar(&app).await;
    let calendars = app.engine("get_calendars", json!({})).await.unwrap().as_array().cloned().unwrap();
    assert_eq!(app.count(&tid("calendar-calendar")).await, calendars.len());
    let week = week_events(&app).await;
    let calendar = calendars.iter().find(|c| week.iter().any(|e| e["sources"].as_array().map(|s| s.len() == 1 && s[0]["calendar_id"] == c["id"]).unwrap_or(false))).expect("no calendar owns an event this week").clone();
    let calendar_id = calendar["id"].as_str().unwrap().to_string();
    let owned: Vec<String> = week.iter().filter(|e| e["sources"].as_array().map(|s| s.len() == 1 && s[0]["calendar_id"] == calendar_id.as_str()).unwrap_or(false)).map(|e| e["id"].as_str().unwrap().to_string()).collect();
    let row = sel("calendar-calendar", "id", &calendar_id);
    app.click(&format!("{} {}", row, tid("calendar-toggle"))).await;
    app.see(&format!("{}[data-enabled=\"false\"]", row)).await;
    for id in &owned {
        app.gone(&sel("calendar-event", "id", id)).await;
    }
    let stored = app.engine("get_calendars", json!({})).await.unwrap().as_array().unwrap().iter().find(|c| c["id"] == calendar_id.as_str()).unwrap().clone();
    assert_eq!(stored["enabled"], false);
    app.click(&format!("{} {}", row, tid("calendar-toggle"))).await;
    app.see(&format!("{}[data-enabled=\"true\"]", row)).await;
    for id in &owned {
        app.see(&sel("calendar-event", "id", id)).await;
    }
}

#[tokio::test]
async fn the_calendar_exports_an_ics_file_and_the_scan_reports_back() {
    let app = App::fresh().await;
    open_calendar(&app).await;
    app.click(&tid("calendar-export")).await;
    app.see(&sel("calendar-banner", "kind", "exported")).await;
    let path = app.text(&tid("calendar-exported")).await;
    let ics = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("the export at {} is unreadable: {}", path, e));
    assert!(ics.contains("BEGIN:VCALENDAR") && ics.contains("Standup Kunde AS"), "the export lacks the seeded events");
    assert!(path.starts_with(app.home.to_str().unwrap()) || path.starts_with("/private/tmp") || path.starts_with("/tmp"), "the export left the test home: {}", path);
    app.click(&tid("calendar-scan")).await;
    app.wait("the scan finishes with a count or an error", &format!("document.querySelector({}) || document.querySelector({})", q("calendar-scanned"), js_string(&sel("calendar-banner", "kind", "failure")))).await;
}
