use super::support::{js_string, tid, App};
use serde_json::{json, Value};

const PANELS: [&str; 17] = ["people", "heatmap", "distribution", "timeline", "weekday-replies", "habits", "mix", "backlog", "kinds", "files", "channels", "tags", "conversations", "words", "rankings", "groups", "globe"];

fn q(name: &str) -> String {
    js_string(&tid(name))
}

fn sel(name: &str, attr: &str, value: &str) -> String {
    format!("{}[data-{}=\"{}\"]", tid(name), attr, value)
}

fn segment(name: &str, value: &str) -> String {
    format!("{} [data-value=\"{}\"]", tid(name), value)
}

async fn open_stats(app: &App) {
    app.goto("/main/stats").await;
    app.see(&format!("{}[data-loading=\"false\"]", tid("stats-sheet"))).await;
}

async fn settled(app: &App, mode: &str) {
    app.see(&format!("{}[data-loading=\"false\"][data-mode=\"{}\"]", tid("stats-sheet"), mode)).await;
}

async fn every_panel_has_data(app: &App, mode: &str) {
    for key in PANELS {
        let panel = sel("stats-panel", "key", key);
        app.see(&panel).await;
        app.wait(&format!("the {} panel to render data in {} mode", key, mode), &format!("(() => {{ const el = document.querySelector({}); return !el.querySelector({}) && el.querySelectorAll('*').length > 12 && el.textContent.trim().length > 60 }})()", js_string(&panel), q("stats-empty"))).await;
    }
    assert!(!app.exists(&tid("stats-synthetic")).await, "the page fell back to synthetic numbers");
}

async fn backend_stats(app: &App, start: &str, end: &str, filters: Value) -> Value {
    let mut input = json!({ "start": start, "end": end });
    if let (Some(input), Some(extra)) = (input.as_object_mut(), filters.as_object()) {
        input.extend(extra.clone());
    }
    app.engine("get_stats", json!({ "input": input })).await.unwrap()
}

async fn shown_range(app: &App) -> (String, String) {
    (app.attr(&tid("stats-range"), "data-start").await.unwrap(), app.attr(&tid("stats-range"), "data-end").await.unwrap())
}

fn days(start: &str, end: &str) -> i64 {
    let parse = |d: &str| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap();
    (parse(end) - parse(start)).num_days() + 1
}

async fn pick(app: &App, selector: &str, value: &str) {
    app.see(selector).await;
    app.js_with("const el = document.querySelector(arguments[0]); el.value = arguments[1]; el.dispatchEvent(new Event('change', { bubbles: true }))", vec![json!(selector), json!(value)]).await;
}

#[tokio::test]
async fn every_panel_renders_seeded_data_without_comparison() {
    let app = App::fresh().await;
    open_stats(&app).await;
    every_panel_has_data(&app, "off").await;
    let (start, end) = shown_range(&app).await;
    assert_eq!(days(&start, &end), 30);
    let stats = backend_stats(&app, &start, &end, json!({})).await;
    let total = stats["totals"]["messages"].as_i64().unwrap();
    assert!(total > 0, "the seeded traffic counts no messages");
    let people = stats["by_person"].as_array().unwrap().len();
    assert_eq!(app.count(&tid("stats-person")).await, people);
    let busiest = stats["by_conversation"].as_array().unwrap().iter().take(8).count();
    assert_eq!(app.count(&tid("stats-conversation")).await, busiest);
    assert!(!app.exists(&tid("stats-sides")).await);
}

#[tokio::test]
async fn comparing_two_periods_draws_both_sides_and_every_panel() {
    let app = App::fresh().await;
    open_stats(&app).await;
    app.click(&segment("stats-compare", "periods")).await;
    settled(&app, "periods").await;
    app.wait("two sides", &format!("document.querySelectorAll({}).length === 2", q("stats-side"))).await;
    every_panel_has_data(&app, "periods").await;
    assert!(!app.exists(&tid("stats-compare-left")).await);
    app.click(&segment("stats-compare", "off")).await;
    settled(&app, "off").await;
    app.gone(&tid("stats-sides")).await;
}

#[tokio::test]
async fn comparing_two_people_highlights_them_and_draws_every_panel() {
    let app = App::fresh().await;
    open_stats(&app).await;
    let (start, end) = shown_range(&app).await;
    let stats = backend_stats(&app, &start, &end, json!({})).await;
    let people: Vec<Value> = stats["by_person"].as_array().unwrap().iter().filter(|p| p["person_id"].is_string()).cloned().collect();
    assert!(people.len() >= 2, "fewer than two people to compare");
    let left = people[0]["person_id"].as_str().unwrap().to_string();
    let right = people[1]["person_id"].as_str().unwrap().to_string();
    app.click(&segment("stats-compare", "people")).await;
    settled(&app, "people").await;
    pick(&app, &tid("stats-compare-left"), &left).await;
    pick(&app, &tid("stats-compare-right"), &right).await;
    app.wait("two people sides", &format!("document.querySelectorAll({}).length === 2", q("stats-side"))).await;
    let headings = app.texts(&format!("{} h2", tid("stats-side"))).await;
    assert_eq!(headings, vec![people[0]["name"].as_str().unwrap().to_string(), people[1]["name"].as_str().unwrap().to_string()]);
    let only_left = backend_stats(&app, &start, &end, json!({ "person_ids": [left] })).await;
    assert!(only_left["totals"]["messages"].as_i64().unwrap() <= stats["totals"]["messages"].as_i64().unwrap());
    app.wait("only the compared people stay highlighted", &format!("[...document.querySelectorAll({})].filter((el) => el.dataset.highlight === 'true').map((el) => el.dataset.id).sort().join() === {}", q("stats-person"), js_string(&{
        let mut pair = vec![left.clone(), right.clone()];
        pair.sort();
        pair.join(",")
    }))).await;
    every_panel_has_data(&app, "people").await;
}

#[tokio::test]
async fn range_presets_and_a_custom_range_reload_the_page() {
    let app = App::fresh().await;
    open_stats(&app).await;
    for (preset, length) in [("7d", 7), ("90d", 90), ("30d", 30)] {
        app.click(&segment("stats-range-preset", preset)).await;
        app.wait(&format!("the {} range", preset), &format!("(() => {{ const el = document.querySelector({}); const d = (s) => new Date(s + 'T00:00:00'); return Math.round((d(el.dataset.end) - d(el.dataset.start)) / 86400000) + 1 === {} }})()", q("stats-range"), length)).await;
        settled(&app, "off").await;
    }
    app.click(&segment("stats-range-preset", "year")).await;
    app.wait("the year range starts on the first of January", &format!("document.querySelector({}).dataset.start.endsWith('-01-01')", q("stats-range"))).await;
    settled(&app, "off").await;
    let (_, end) = shown_range(&app).await;
    let start = format!("{}-01", &end[..7]);
    app.fill(&tid("stats-range-start"), &start).await;
    app.js(&format!("document.querySelector({}).blur()", q("stats-range-start"))).await;
    app.wait("the custom start", &format!("document.querySelector({}).dataset.start === {}", q("stats-range"), js_string(&start))).await;
    app.fill(&tid("stats-range-start"), "2026-13-45").await;
    app.js(&format!("document.querySelector({}).blur()", q("stats-range-start"))).await;
    app.wait("an invalid date is refused", &format!("document.querySelector({}).value === {} && document.querySelector({}).dataset.start === {}", q("stats-range-start"), js_string(&start), q("stats-range"), js_string(&start))).await;
    settled(&app, "off").await;
    assert!(!app.exists(&tid("stats-synthetic")).await);
}

#[tokio::test]
async fn the_people_table_sorts_and_rows_open_people_conversations_and_senders() {
    let app = App::fresh().await;
    open_stats(&app).await;
    let sort = sel("stats-sort", "key", "characters");
    app.click(&sort).await;
    app.wait("the table sorts by characters", &format!("document.querySelector({}).closest('th').getAttribute('aria-sort') !== 'none'", js_string(&sort))).await;
    let first = app.attr(&tid("stats-person"), "data-id").await;
    app.click(&sort).await;
    app.wait("the order flips", &format!("document.querySelector({}).dataset.id !== {}", q("stats-person"), js_string(&first.clone().unwrap_or_default()))).await;

    app.click(&tid("stats-ranking-person")).await;
    app.see(&tid("stats-sender")).await;
    app.press(None, "Escape", &[]).await;
    app.gone(&tid("stats-sender")).await;

    let conversation = app.attr(&tid("stats-conversation"), "data-id").await.unwrap();
    app.click(&tid("stats-conversation")).await;
    app.wait_url(&format!("/main/conversation/{}", conversation)).await;

    open_stats(&app).await;
    let person = app.js(&format!("return [...document.querySelectorAll({})].find((el) => el.dataset.id)?.dataset.id", q("stats-person"))).await.as_str().unwrap().to_string();
    app.click(&format!("{} {}", sel("stats-person", "id", &person), tid("stats-person-open"))).await;
    app.wait_url(&format!("/main/person/{}/profile", person)).await;
}

