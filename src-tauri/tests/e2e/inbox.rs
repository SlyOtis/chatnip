use super::support::{js_string, tid, App};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn row(id: &str) -> String {
    format!("[data-testid=\"inbox-row\"][data-id=\"{}\"]", id)
}

fn in_row(id: &str, testid: &str) -> String {
    format!("{} {}", row(id), tid(testid))
}

async fn row_ids(app: &App, kind: &str) -> Vec<String> {
    app.see(&tid("inbox-row")).await;
    app.js_with("return [...document.querySelectorAll('[data-testid=\"inbox-row\"]')].filter((r) => r.dataset.kind === arguments[0]).map((r) => r.dataset.id)", vec![json!(kind)])
        .await
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

async fn group_names(app: &App, key: &str) -> Vec<String> {
    app.texts(&format!("[data-testid=\"inbox-group\"][data-key=\"{}\"] [data-testid=\"inbox-row-name\"]", key)).await
}

async fn conversation(app: &App, id: &str) -> Value {
    app.conversations().await.into_iter().find(|c| c["id"].as_str() == Some(id)).unwrap_or(Value::Null)
}

async fn wait_conversation(app: &App, id: &str, what: &str, test: impl Fn(&Value) -> bool) -> Value {
    let started = Instant::now();
    loop {
        let found = conversation(app, id).await;
        if test(&found) {
            return found;
        }
        assert!(started.elapsed() < Duration::from_secs(20), "conversation {} never reached: {}; last {}", id, what, found);
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn focus_row(app: &App, id: &str) {
    app.hover(&row(id)).await;
    app.wait("the row to take focus", &format!("document.querySelector({}).dataset.focused === 'true'", js_string(&row(id)))).await;
}

async fn meta_click(app: &App, selector: &str) {
    app.see(selector).await;
    app.js_with("document.querySelector(arguments[0]).dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, metaKey: true }))", vec![json!(selector)]).await;
}

async fn open_view(app: &App, key: &str) {
    let entry = format!("[data-testid=\"rail-entry\"][data-key=\"{}\"]", key);
    app.see(&entry).await;
    app.js_with("const entry = document.querySelector(arguments[0]); (entry.querySelector('button') || entry).click()", vec![json!(entry)]).await;
}

async fn first_unread_conversation(app: &App) -> String {
    app.see(&tid("inbox-row")).await;
    app.js("return [...document.querySelectorAll('[data-testid=\"inbox-row\"][data-kind=\"conversation\"][data-unread=\"true\"]')].map((r) => r.dataset.id)[0] || null").await.as_str().map(String::from).expect("an unread conversation row")
}

#[tokio::test]
async fn rows_show_people_and_chats_each_with_an_avatar() {
    let app = App::fresh().await;
    let people = row_ids(&app, "person").await;
    let chats = row_ids(&app, "conversation").await;
    assert!(!people.is_empty(), "no person rows");
    assert!(!chats.is_empty(), "no chat rows");
    assert!(app.exists("[data-testid=\"inbox-group\"][data-key=\"people\"]").await);
    let rows = app.count(&tid("inbox-row")).await;
    assert_eq!(app.count(&format!("{} {}", tid("inbox-row"), tid("inbox-row-avatar"))).await, rows, "every row carries an avatar");
    assert!(app.count(&format!("{}[data-kind=\"person\"] {} img", tid("inbox-row"), tid("inbox-row-avatar"))).await > 0, "some people show a picture");
    let names = app.texts(&tid("inbox-row-name")).await;
    assert!(names.iter().all(|name| !name.is_empty()), "a row has no name: {:?}", names);
}

#[tokio::test]
async fn clicking_the_name_of_a_chat_opens_it() {
    let app = App::fresh().await;
    let id = row_ids(&app, "conversation").await[0].clone();
    app.click(&in_row(&id, "inbox-row-name")).await;
    app.wait_url(&format!("/main/conversation/{}", id)).await;
    app.wait("the row to show as open", &format!("document.querySelector({}).getAttribute('aria-selected') === 'true'", js_string(&row(&id)))).await;
    let current = app.engine("get_current_conversation", json!({})).await.unwrap();
    assert_eq!(current["data"]["id"].as_str(), Some(id.as_str()), "the backend current conversation is the opened one: {}", current);
}

#[tokio::test]
async fn clicking_a_person_row_opens_their_thread() {
    let app = App::fresh().await;
    let id = row_ids(&app, "person").await[0].clone();
    app.click(&row(&id)).await;
    app.wait_url(&format!("/main/person/{}", id)).await;
    assert!(!app.url().await.ends_with("/profile"));
}

#[tokio::test]
async fn clicking_a_person_avatar_opens_the_profile_page() {
    let app = App::fresh().await;
    let id = row_ids(&app, "person").await[0].clone();
    app.click(&in_row(&id, "inbox-row-avatar")).await;
    app.wait_url(&format!("/main/person/{}/profile", id)).await;
}

#[tokio::test]
async fn clicking_a_chat_avatar_opens_its_details_without_leaving_the_list() {
    let app = App::fresh().await;
    app.see("[data-testid=\"inbox-group\"][data-key=\"teams\"]").await;
    let id = app.js("return document.querySelector('[data-testid=\"inbox-group\"][data-key=\"teams\"] [data-testid=\"inbox-row\"]').dataset.id").await.as_str().unwrap().to_string();
    let before = app.url().await;
    app.click(&in_row(&id, "inbox-row-avatar")).await;
    app.see("[role=\"dialog\"]").await;
    assert_eq!(app.url().await, before);
}

#[tokio::test]
async fn the_unread_filter_narrows_the_list_and_toggles_back() {
    let app = App::fresh().await;
    app.see(&tid("inbox-row")).await;
    let all = app.count(&tid("inbox-row")).await;
    app.click(&tid("inbox-filter")).await;
    app.see(&tid("inbox-filter-menu")).await;
    assert_eq!(app.attr(&tid("inbox-filter-unread"), "data-checked").await.as_deref(), Some("false"));
    app.click(&tid("inbox-filter-unread")).await;
    app.gone(&tid("inbox-filter-menu")).await;
    app.wait("only unread rows", "[...document.querySelectorAll('[data-testid=\"inbox-row\"]')].every((r) => r.dataset.unread === 'true')").await;
    let unread = app.count(&tid("inbox-row")).await;
    assert!(unread > 0 && unread < all, "unread filter showed {} of {}", unread, all);
    assert_eq!(app.attr(&tid("inbox-filter"), "aria-pressed").await.as_deref(), Some("true"));
    app.click(&tid("inbox-filter")).await;
    assert_eq!(app.attr(&tid("inbox-filter-unread"), "data-checked").await.as_deref(), Some("true"));
    app.click(&tid("inbox-filter-unread")).await;
    app.wait("all rows back", &format!("document.querySelectorAll('[data-testid=\"inbox-row\"]').length === {}", all)).await;
}

#[tokio::test]
async fn sorting_changes_the_order_and_survives_navigation() {
    let app = App::fresh().await;
    app.see(&tid("inbox-row")).await;
    let recent = group_names(&app, "people").await;
    app.click(&tid("inbox-filter")).await;
    assert_eq!(app.attr(&tid("inbox-sort-recent"), "data-checked").await.as_deref(), Some("true"));
    app.click(&tid("inbox-sort-alphabetical")).await;
    let names = "[...document.querySelectorAll('[data-testid=\"inbox-group\"][data-key=\"people\"] [data-testid=\"inbox-row-name\"]')].map((e) => e.textContent.trim())";
    app.wait("people sorted by name", &format!("JSON.stringify({names}.sort((a, b) => a.localeCompare(b))) === JSON.stringify({names})")).await;
    let alphabetical = group_names(&app, "people").await;
    assert_ne!(alphabetical, recent, "the seed gives the same order for recent and alphabetical");
    open_view(&app, "calendar").await;
    app.wait_url("/main/calendar").await;
    app.client.back().await.unwrap();
    app.wait_url("/main/conversation").await;
    app.see(&tid("inbox-row")).await;
    assert_eq!(group_names(&app, "people").await, alphabetical, "the sort did not survive navigation");
    app.click(&tid("inbox-filter")).await;
    assert_eq!(app.attr(&tid("inbox-sort-alphabetical"), "data-checked").await.as_deref(), Some("true"));
    app.click(&tid("inbox-sort-recent")).await;
    app.wait("the recent order back", &format!("JSON.stringify({names}) === {}", js_string(&serde_json::to_string(&recent).unwrap()))).await;
}

#[tokio::test]
async fn a_group_collapses_and_expands_and_the_state_is_saved() {
    let app = App::fresh().await;
    let group = "[data-testid=\"inbox-group\"][data-key=\"teams\"]";
    app.see(group).await;
    assert!(app.count(&format!("{} {}", group, tid("inbox-row"))).await > 0);
    app.click(&format!("{} {}", group, tid("inbox-group-toggle"))).await;
    app.wait("the group to collapse", &format!("document.querySelector({}).dataset.collapsed === 'true'", js_string(group))).await;
    assert_eq!(app.count(&format!("{} {}", group, tid("inbox-row"))).await, 0);
    assert_eq!(app.attr(&format!("{} {}", group, tid("inbox-group-toggle")), "aria-expanded").await.as_deref(), Some("false"));
    let started = Instant::now();
    while app.setting("list.section.teams.collapsed").await.as_deref() != Some("true") {
        assert!(started.elapsed() < Duration::from_secs(10), "the collapsed state was not saved");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    app.goto("/main/conversation").await;
    app.see(group).await;
    assert_eq!(app.attr(group, "data-collapsed").await.as_deref(), Some("true"), "the collapse did not survive a reload");
    app.click(&format!("{} {}", group, tid("inbox-group-toggle"))).await;
    app.wait("the group to expand", &format!("document.querySelector({}).dataset.collapsed === 'false'", js_string(group))).await;
    assert!(app.count(&format!("{} {}", group, tid("inbox-row"))).await > 0);
}

#[tokio::test]
async fn group_headers_stay_sticky_while_the_list_scrolls() {
    let app = App::fresh().await;
    app.see(&tid("inbox-group-header")).await;
    let position = app.js("return getComputedStyle(document.querySelector('[data-testid=\"inbox-group-header\"]')).position").await;
    assert_eq!(position.as_str(), Some("sticky"));
}

#[tokio::test]
async fn keyboard_j_k_and_enter_walk_and_open_rows() {
    let app = App::fresh().await;
    app.see(&tid("inbox-row")).await;
    let ids: Vec<String> = app.js("return [...document.querySelectorAll('[data-testid=\"inbox-row\"]')].map((r) => r.dataset.id)").await.as_array().unwrap().iter().filter_map(|v| v.as_str().map(String::from)).collect();
    let focused = "[...document.querySelectorAll('[data-testid=\"inbox-row\"]')].findIndex((r) => r.dataset.focused === 'true')";
    app.js("document.activeElement && document.activeElement.blur()").await;
    let start = app.js(&format!("return {}", focused)).await.as_i64().unwrap().max(0);
    app.press(None, "j", &[]).await;
    app.press(None, "j", &[]).await;
    app.wait("two rows down", &format!("{} === {}", focused, start + 2)).await;
    app.press(None, "k", &[]).await;
    app.wait("one row up", &format!("{} === {}", focused, start + 1)).await;
    let target = ids[(start + 1) as usize].clone();
    let kind = app.attr(&row(&target), "data-kind").await.unwrap();
    app.press(None, "Enter", &[]).await;
    app.wait_url(&if kind == "person" { format!("/main/person/{}", target) } else { format!("/main/conversation/{}", target) }).await;
}

#[tokio::test]
async fn e_marks_the_focused_chat_done_and_the_done_view_returns_it() {
    let app = App::fresh().await;
    let id = row_ids(&app, "conversation").await[0].clone();
    focus_row(&app, &id).await;
    app.js("document.activeElement && document.activeElement.blur()").await;
    app.press(None, "e", &[]).await;
    wait_conversation(&app, &id, "archived", |c| c["is_archived"] == json!(true)).await;
    app.gone(&row(&id)).await;
    open_view(&app, "done").await;
    app.see(&row(&id)).await;
    focus_row(&app, &id).await;
    app.click(&in_row(&id, "inbox-row-done")).await;
    wait_conversation(&app, &id, "back in the inbox", |c| c["is_archived"] == json!(false)).await;
    app.gone(&row(&id)).await;
}

#[tokio::test]
async fn u_toggles_read_on_the_focused_chat() {
    let app = App::fresh().await;
    let id = first_unread_conversation(&app).await;
    assert!(conversation(&app, &id).await["unread_count"].as_i64().unwrap_or(0) > 0);
    focus_row(&app, &id).await;
    app.js("document.activeElement && document.activeElement.blur()").await;
    app.press(None, "u", &[]).await;
    wait_conversation(&app, &id, "read", |c| c["unread_count"] == json!(0)).await;
    app.wait("the unread mark to clear", &format!("document.querySelector({}).dataset.unread === 'false'", js_string(&row(&id)))).await;
    focus_row(&app, &id).await;
    app.press(None, "u", &[]).await;
    wait_conversation(&app, &id, "unread again", |c| c["unread_count"].as_i64().unwrap_or(0) > 0).await;
    app.wait("the unread mark to return", &format!("document.querySelector({}).dataset.unread === 'true'", js_string(&row(&id)))).await;
}

#[tokio::test]
async fn the_snooze_sheet_hides_a_chat_until_later() {
    let app = App::fresh().await;
    let id = row_ids(&app, "conversation").await[0].clone();
    focus_row(&app, &id).await;
    app.click(&in_row(&id, "inbox-row-snooze")).await;
    app.see(&format!("{}[data-kind=\"snooze\"]", tid("inbox-sheet"))).await;
    app.click(&tid("inbox-snooze-tomorrow")).await;
    let snoozed = wait_conversation(&app, &id, "snoozed", |c| c["snoozed_until"].is_string()).await;
    let until = chrono::DateTime::parse_from_rfc3339(snoozed["snoozed_until"].as_str().unwrap()).unwrap();
    assert!(until > chrono::Utc::now(), "snoozed into the past: {}", until);
    app.gone(&row(&id)).await;
    open_view(&app, "snoozed").await;
    app.see(&row(&id)).await;
}

async fn open_row_sheet(app: &App, id: &str) {
    focus_row(app, id).await;
    app.click(&in_row(id, "inbox-row-more")).await;
    app.see(&format!("{}[data-kind=\"tag\"]", tid("inbox-sheet"))).await;
}

#[tokio::test]
async fn the_row_sheet_toggles_read_mode_tags_and_delete() {
    let app = App::fresh().await;
    let id = row_ids(&app, "conversation").await[0].clone();
    let before = conversation(&app, &id).await;
    let mode = before["mode"].as_str().unwrap_or("write").to_string();
    open_row_sheet(&app, &id).await;
    app.click(&tid("inbox-sheet-mode")).await;
    wait_conversation(&app, &id, "the mode flipped", |c| c["mode"].as_str().map(|m| m != mode).unwrap_or(false)).await;

    open_row_sheet(&app, &id).await;
    app.click(&tid("inbox-sheet-delete")).await;
    wait_conversation(&app, &id, "deleted", |c| c["deleted_at"].is_string()).await;
    app.gone(&row(&id)).await;
    open_view(&app, "deleted").await;
    app.see(&row(&id)).await;
    focus_row(&app, &id).await;
    app.click(&in_row(&id, "inbox-row-restore")).await;
    wait_conversation(&app, &id, "restored", |c| c["deleted_at"].is_null()).await;
    open_view(&app, "inbox").await;
    app.see(&row(&id)).await;
    open_row_sheet(&app, &id).await;
    let tag = app.attr("[data-testid^=\"inbox-sheet-tag-\"]:not([data-testid=\"inbox-sheet-tag-search\"])", "data-testid").await.unwrap();
    let tag_id = tag.trim_start_matches("inbox-sheet-tag-").to_string();
    let tagged = move |c: &Value| c["tags"].as_array().map(|t| t.iter().any(|l| l["tag_id"].as_str() == Some(tag_id.as_str()))).unwrap_or(false);
    let had = tagged(&before);
    assert_eq!(app.attr(&tid(&tag), "data-checked").await.as_deref(), Some(if had { "true" } else { "false" }));
    app.click(&tid(&tag)).await;
    app.gone(&tid("inbox-sheet")).await;
    wait_conversation(&app, &id, "the tag toggled", |c| tagged(c) != had).await;
}

#[tokio::test]
async fn selecting_two_chats_marks_both_done_and_clear_drops_the_selection() {
    let app = App::fresh().await;
    let ids = row_ids(&app, "conversation").await;
    meta_click(&app, &row(&ids[0])).await;
    meta_click(&app, &row(&ids[1])).await;
    app.see(&tid("inbox-selection")).await;
    app.see(&tid("inbox-selection-merge")).await;
    app.click(&tid("inbox-selection-clear")).await;
    app.gone(&tid("inbox-selection")).await;
    meta_click(&app, &row(&ids[0])).await;
    meta_click(&app, &row(&ids[1])).await;
    app.click(&tid("inbox-selection-done")).await;
    wait_conversation(&app, &ids[0], "archived", |c| c["is_archived"] == json!(true)).await;
    wait_conversation(&app, &ids[1], "archived", |c| c["is_archived"] == json!(true)).await;
    app.gone(&tid("inbox-selection")).await;
}

fn merged(groups: &Value, a: &str, b: &str) -> bool {
    groups.as_array().map(|all| all.iter().any(|g| { let ids = &g["conversation_ids"]; ids.as_array().map(|ids| ids.iter().any(|i| i == a) && ids.iter().any(|i| i == b)).unwrap_or(false) })).unwrap_or(false)
}

#[tokio::test]
async fn merging_two_chats_from_the_selection_groups_them_and_undo_splits_them() {
    let app = App::fresh().await;
    let ids = row_ids(&app, "conversation").await;
    meta_click(&app, &row(&ids[0])).await;
    meta_click(&app, &row(&ids[1])).await;
    app.click(&tid("inbox-selection-merge")).await;
    app.see(&tid("inbox-toast")).await;
    app.see(&format!("{}[data-kind=\"merged\"]", tid("inbox-row"))).await;
    let groups = app.engine("get_merge_groups", json!({})).await.unwrap();
    assert!(merged(&groups, &ids[0], &ids[1]), "no merge group: {}", groups);
    app.click(&tid("inbox-toast-undo")).await;
    let started = Instant::now();
    while merged(&app.engine("get_merge_groups", json!({})).await.unwrap(), &ids[0], &ids[1]) {
        assert!(started.elapsed() < Duration::from_secs(20), "undo kept the merge group");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    app.see(&row(&ids[0])).await;
    app.see(&row(&ids[1])).await;
}

#[tokio::test]
async fn the_new_chat_dialog_needs_a_name_and_cancel_closes_it() {
    let app = App::fresh().await;
    app.click(&tid("inbox-new-chat")).await;
    app.see(&tid("new-chat")).await;
    assert!(app.exists(&format!("{}:disabled", tid("new-chat-create"))).await);
    app.type_into(&tid("new-chat-name"), "Draft").await;
    app.wait("create to enable", &format!("!document.querySelector({}).disabled", js_string(&tid("new-chat-create")))).await;
    let chats = app.engine("get_chats", json!({})).await.unwrap().as_array().unwrap().len();
    app.click(&tid("new-chat-cancel")).await;
    app.gone(&tid("new-chat")).await;
    assert_eq!(app.engine("get_chats", json!({})).await.unwrap().as_array().unwrap().len(), chats);
}

#[tokio::test]
async fn a_read_chat_gathers_sources_with_a_view_and_an_attached_action() {
    let app = App::fresh().await;
    app.click(&tid("inbox-new-chat")).await;
    app.see(&tid("new-chat")).await;
    app.type_into(&tid("new-chat-name"), "E2E reader").await;
    assert_eq!(app.attr(&format!("{} [data-value=\"read\"]", tid("new-chat-mode")), "aria-checked").await.as_deref(), Some("true"));
    for kind in ["conversation", "group", "tag", "person"] {
        let tab = format!("{}[data-value=\"{}\"]", tid("source-kind"), kind);
        app.click(&tab).await;
        app.wait(&format!("{} choices", kind), &format!("document.querySelector({}).getAttribute('aria-selected') === 'true' && document.querySelector('[data-testid=\"source-choice\"][data-kind=\"{}\"]')", js_string(&tab), kind)).await;
        app.click(&format!("{}[data-kind=\"{}\"][aria-pressed=\"false\"]", tid("source-choice"), kind)).await;
    }
    app.wait("four chosen sources", &format!("document.querySelectorAll({}).length === 4", js_string(&tid("source-chosen")))).await;
    app.click(&format!("{}[data-kind=\"tag\"] {}", tid("source-chosen"), tid("source-remove"))).await;
    app.wait("three chosen sources", &format!("document.querySelectorAll({}).length === 3", js_string(&tid("source-chosen")))).await;
    let sources: Vec<(String, String)> = app.js("return [...document.querySelectorAll('[data-testid=\"source-chosen\"]')].map((s) => [s.dataset.kind, s.dataset.id])").await.as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_str().unwrap().to_string())).collect();
    app.click(&format!("{} [data-value=\"grid\"]", tid("new-chat-view"))).await;
    let action = app.attr(&tid("action-choice"), "data-id").await.expect("an attachable action");
    app.click(&format!("{}[data-id=\"{}\"]", tid("action-choice"), action)).await;
    app.wait("the action to be chosen", &format!("document.querySelector('[data-testid=\"action-choice\"][data-id=\"{}\"]').getAttribute('aria-pressed') === 'true'", action)).await;
    app.click(&tid("new-chat-create")).await;
    app.gone(&tid("new-chat")).await;
    let chats = app.engine("get_chats", json!({})).await.unwrap();
    let chat = chats.as_array().unwrap().iter().find(|c| c["name"] == json!("E2E reader")).cloned().expect("the chat was not created");
    assert_eq!(chat["mode"], json!("read"));
    let stored: Vec<(String, String)> = chat["sources"].as_array().unwrap().iter().map(|s| (s["kind"].as_str().unwrap().to_string(), s["id"].as_str().unwrap_or_default().to_string())).collect();
    assert_eq!(stored, sources);
    assert!(chat["attachments"].as_array().unwrap().iter().any(|a| a["action_id"].as_str() == Some(action.as_str())), "{}", chat);
    let conversation_id = chat["conversation_id"].as_str().unwrap().to_string();
    app.wait_url(&format!("/main/conversation/{}", conversation_id)).await;
    let found = app.engine("get_search", json!({ "input": { "query": "E2E reader" } })).await.unwrap_or_else(|e| panic!("search failed: {}", e));
    let stored = found["conversations"].as_array().unwrap().iter().find(|c| c["id"].as_str() == Some(conversation_id.as_str())).cloned().expect("the chat conversation is searchable");
    assert_eq!(stored["view"], json!("grid"));
    assert_eq!(stored["mode"], json!("read"));
}

#[tokio::test]
async fn a_write_chat_merges_picked_chats_and_writes_through_them() {
    let app = App::fresh().await;
    app.click(&tid("inbox-new-chat")).await;
    app.see(&tid("new-chat")).await;
    app.type_into(&tid("new-chat-name"), "E2E writer").await;
    app.click(&format!("{} [data-value=\"write\"]", tid("new-chat-mode"))).await;
    app.see(&tid("source-find")).await;
    assert!(!app.exists(&tid("new-chat-view")).await, "a write chat offers no view choice");
    app.see(&tid("source-merge-choice")).await;
    let picks: Vec<String> = app.js("return [...document.querySelectorAll('[data-testid=\"source-merge-choice\"]')].slice(0, 2).map((b) => b.dataset.id)").await.as_array().unwrap().iter().filter_map(|v| v.as_str().map(String::from)).collect();
    assert_eq!(picks.len(), 2);
    let name = conversation(&app, &picks[0]).await["name"].as_str().unwrap_or_default().to_string();
    app.type_into(&tid("source-find"), &name).await;
    app.wait("the merge picker to filter", &format!("document.querySelector('[data-testid=\"source-merge-choice\"][data-id=\"{}\"]')", picks[0])).await;
    app.click(&format!("{}[data-id=\"{}\"]", tid("source-merge-choice"), picks[0])).await;
    app.fill(&tid("source-find"), "").await;
    app.click(&format!("{}[data-id=\"{}\"]", tid("source-merge-choice"), picks[1])).await;
    app.wait("two picked", "document.querySelectorAll('[data-testid=\"source-merge-choice\"][aria-pressed=\"true\"]').length === 2").await;
    app.click(&format!("{}[data-id=\"{}\"]", tid("source-merge-choice"), picks[1])).await;
    app.wait("one picked", "document.querySelectorAll('[data-testid=\"source-merge-choice\"][aria-pressed=\"true\"]').length === 1").await;
    app.click(&format!("{}[data-id=\"{}\"]", tid("source-merge-choice"), picks[1])).await;
    app.wait("two picked again", "document.querySelectorAll('[data-testid=\"source-merge-choice\"][aria-pressed=\"true\"]').length === 2").await;
    app.click(&tid("new-chat-create")).await;
    app.gone(&tid("new-chat")).await;
    let chats = app.engine("get_chats", json!({})).await.unwrap();
    let chat = chats.as_array().unwrap().iter().find(|c| c["name"] == json!("E2E writer")).cloned().expect("the chat was not created");
    assert_eq!(chat["mode"], json!("write"));
    let mut stored: Vec<String> = chat["sources"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap().to_string()).collect();
    let mut expected = picks.clone();
    stored.sort();
    expected.sort();
    assert_eq!(stored, expected);
    assert!(chat["sources"].as_array().unwrap().iter().all(|s| s["kind"] == json!("conversation")));
    app.wait_url(&format!("/main/conversation/{}", chat["conversation_id"].as_str().unwrap())).await;
}

#[tokio::test]
async fn cmd_k_searches_people_and_chats_opens_a_result_and_escape_closes() {
    let app = App::fresh().await;
    let person = row_ids(&app, "person").await[0].clone();
    let name = app.text(&in_row(&person, "inbox-row-name")).await;
    let chat_name = plain_chat_name(&app).await;
    app.js("document.activeElement && document.activeElement.blur()").await;
    app.press(None, "k", &["meta"]).await;
    app.see(&tid("search-input")).await;
    app.press(Some(&tid("search-input")), "Escape", &[]).await;
    app.gone(&tid("search")).await;
    app.press(None, "k", &["meta"]).await;
    app.type_into(&tid("search-input"), &name).await;
    let hit = format!("{}[data-kind=\"person\"]", tid("search-result"));
    wait_hit(&app, "person", &name).await;
    app.fill(&tid("search-input"), &chat_name).await;
    wait_hit(&app, "conversation", &chat_name).await;
    app.fill(&tid("search-input"), &name).await;
    wait_hit(&app, "person", &name).await;
    app.js_with("[...document.querySelectorAll(arguments[0])].find((r) => r.textContent.includes(arguments[1])).click()", vec![json!(hit), json!(name)]).await;
    app.gone(&tid("search")).await;
    app.wait("the person to open", &format!("location.pathname.includes({})", js_string(&person))).await;
}

#[tokio::test]
async fn the_search_button_opens_the_palette_and_arrows_with_enter_open_the_selection() {
    let app = App::fresh().await;
    let name = plain_chat_name(&app).await;
    app.click(&tid("inbox-search")).await;
    app.see(&tid("search-input")).await;
    app.type_into(&tid("search-input"), &name).await;
    wait_hit(&app, "conversation", &name).await;
    let selected = "[...document.querySelectorAll('[data-testid=\"search-result\"]')].findIndex((r) => r.dataset.selected === 'true')";
    let wanted = app.js("return [...document.querySelectorAll('[data-testid=\"search-result\"]')].findIndex((r) => r.dataset.kind === 'conversation')").await.as_i64().unwrap();
    for _ in 0..wanted {
        let before = app.js(&format!("return {}", selected)).await.as_i64().unwrap();
        app.press(Some(&tid("search-input")), "ArrowDown", &[]).await;
        app.wait("the selection to move", &format!("{} === {}", selected, before + 1)).await;
    }
    let opens = app.js(&format!("return [...document.querySelectorAll('[data-testid=\"search-result\"]')][{}].dataset.id", wanted)).await.as_str().unwrap().to_string();
    app.press(Some(&tid("search-input")), "Enter", &[]).await;
    app.gone(&tid("search")).await;
    let opened = opens.trim_start_matches("conversation-").to_string();
    app.wait("the chosen chat to open", &format!("location.pathname.includes({})", js_string(&opened))).await;
}


async fn plain_chat_name(app: &App) -> String {
    for id in row_ids(app, "conversation").await {
        let name = conversation(app, &id).await["name"].as_str().unwrap_or_default().to_string();
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == ' ') {
            return name;
        }
    }
    panic!("no chat with a plain name");
}

async fn wait_hit(app: &App, kind: &str, text: &str) {
    let selector = format!("{}[data-kind=\"{}\"]", tid("search-result"), kind);
    let expression = format!("[...document.querySelectorAll({})].some((r) => r.textContent.includes({}))", js_string(&selector), js_string(text));
    let started = Instant::now();
    while !app.check(&expression).await {
        if started.elapsed() > Duration::from_secs(20) {
            let shown = app.js("return [...document.querySelectorAll('[data-testid=\"search-result\"]')].map((r) => r.dataset.kind + ':' + r.textContent.trim().slice(0, 40))").await;
            panic!("no {} result containing {:?}; query {:?}; shown {}", kind, text, app.value(&tid("search-input")).await, shown);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn the_section_menu_opens_on_its_more_button_and_names_the_section_inline() {
    let app = App::fresh().await;
    let header = "[data-testid=\"inbox-group\"][data-key=\"people\"] [data-testid=\"inbox-group-header\"]";
    app.hover(header).await;
    app.click(&format!("{} {}", header, tid("inbox-group-more"))).await;
    app.see(&tid("inbox-group-menu")).await;
    app.see(&tid("inbox-group-menu-name")).await;
}

#[tokio::test]
async fn the_section_menu_opens_on_a_section_with_an_image_background() {
    let app = App::fresh().await;
    let image = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0naHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmcnIHdpZHRoPSc0JyBoZWlnaHQ9JzQnPjxyZWN0IHdpZHRoPSc0JyBoZWlnaHQ9JzQnIGZpbGw9JyMzMzY2OTknLz48L3N2Zz4=";
    app.put_setting("list.section.people.background", image).await;
    app.js("location.reload(); return true").await;
    let section = "[data-testid=\"inbox-group\"][data-key=\"people\"]";
    app.wait("the section to take the image", &format!("!!document.querySelector({})?.dataset.background", js_string(section))).await;
    let header = format!("{} [data-testid=\"inbox-group-header\"]", section);
    app.hover(&header).await;
    app.click(&format!("{} {}", header, tid("inbox-group-more"))).await;
    app.see(&tid("inbox-group-menu")).await;
    app.see(&tid("inbox-group-menu-fit")).await;
    app.see(&tid("inbox-group-menu-blend")).await;
}

#[tokio::test]
async fn a_merged_member_opens_its_menu_and_scrolling_the_list_closes_it() {
    let app = App::fresh().await;
    let ids = row_ids(&app, "conversation").await;
    meta_click(&app, &row(&ids[0])).await;
    meta_click(&app, &row(&ids[1])).await;
    app.click(&tid("inbox-selection-merge")).await;
    let merged = format!("{}[data-kind=\"merged\"]", tid("inbox-row"));
    app.see(&merged).await;
    app.click(&format!("{} {}", merged, tid("inbox-row-expand"))).await;
    let member = format!("{} {}", tid("inbox-merged-member"), tid("inbox-row"));
    app.see(&member).await;
    app.hover(&member).await;
    app.click(&format!("{} {}", member, tid("inbox-row-more"))).await;
    app.see(&tid("inbox-sheet")).await;
    app.js(&format!("document.querySelector({}).dispatchEvent(new WheelEvent('wheel', {{ bubbles: true, deltaY: 120 }})); return true", js_string(&tid("inbox-list")))).await;
    app.gone(&tid("inbox-sheet")).await;
}

#[tokio::test]
async fn a_row_in_a_coloured_section_opens_its_menu() {
    let app = App::fresh().await;
    app.put_setting("list.section.people.background", "#5a8fb8").await;
    app.js("location.reload(); return true").await;
    let section = "[data-testid=\"inbox-group\"][data-key=\"people\"]";
    app.wait("the section to take the colour", &format!("document.querySelector({})?.dataset.background === '#5a8fb8'", js_string(section))).await;
    let first = format!("{} {}", section, tid("inbox-row"));
    app.hover(&first).await;
    app.click(&format!("{} {}", first, tid("inbox-row-more"))).await;
    app.see(&tid("inbox-sheet")).await;
}

#[tokio::test]
async fn a_row_in_a_section_with_an_image_opens_its_menu() {
    let app = App::fresh().await;
    let image = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0naHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmcnIHdpZHRoPSc0JyBoZWlnaHQ9JzQnPjxyZWN0IHdpZHRoPSc0JyBoZWlnaHQ9JzQnIGZpbGw9JyMzMzY2OTknLz48L3N2Zz4=";
    app.put_setting("list.section.people.background", image).await;
    app.js("location.reload(); return true").await;
    let section = "[data-testid=\"inbox-group\"][data-key=\"people\"]";
    app.wait("the section to take the image", &format!("!!document.querySelector({})?.dataset.background", js_string(section))).await;
    let first = format!("{} {}", section, tid("inbox-row"));
    app.hover(&first).await;
    app.click(&format!("{} {}", first, tid("inbox-row-more"))).await;
    app.see(&tid("inbox-sheet")).await;
}
