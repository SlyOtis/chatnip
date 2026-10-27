use super::support::{tid, App};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) async fn query(app: &App, sql: &str) -> Value {
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::{ConnectOptions, Row};
    let path = app.home.join("Library/Application Support/com.slyotis.chatnip/chatnip.db");
    let mut connection = SqliteConnectOptions::new().filename(&path).read_only(true).connect().await.unwrap();
    let row = sqlx::query(sql).fetch_one(&mut connection).await.unwrap_or_else(|e| panic!("query failed: {}\n{}", e, sql));
    let text: Option<String> = row.try_get(0).unwrap();
    serde_json::from_str(&text.unwrap_or_else(|| "null".into())).unwrap()
}

pub(super) fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub(super) async fn messages(app: &App, conversation_id: &str) -> Vec<Value> {
    let sql = format!(
        "SELECT json_group_array(json_object('id', id, 'external_id', external_id, 'content', content, 'sender_id', sender_id, 'provider_id', provider_id, 'deleted', deleted_datetime, 'metadata', metadata, 'reply_to_id', reply_to_id, 'subject', subject)) FROM (SELECT * FROM message WHERE conversation_id = {} ORDER BY timestamp)",
        quoted(conversation_id)
    );
    query(app, &sql).await.as_array().cloned().unwrap_or_default()
}

pub(super) async fn server_messages(app: &App, provider_id: &str) -> Vec<Value> {
    let sql = format!("SELECT json_group_array(json(body)) FROM (SELECT body FROM mock_server_message WHERE account LIKE {} ORDER BY timestamp)", quoted(&format!("%:{}", provider_id)));
    query(app, &sql).await.as_array().cloned().unwrap_or_default()
}

pub(super) async fn conversation_of(app: &App, kind: &str, types: &[&str]) -> Value {
    let provider = app.mock_provider(kind);
    let read: Vec<Value> = app.engine("get_read_conversation_ids", json!({})).await.unwrap().as_array().cloned().unwrap_or_default();
    app.conversations()
        .await
        .into_iter()
        .filter(|c| !read.contains(&c["id"]) && c["deleted_at"].is_null())
        .filter(|c| c["providers"].as_array().map(|p| p.len() == 1 && p[0]["id"].as_str() == Some(provider.as_str())).unwrap_or(false))
        .find(|c| types.contains(&c["type"].as_str().unwrap_or_default()))
        .unwrap_or_else(|| panic!("no {:?} conversation on the mock {}", types, kind))
}

pub(super) async fn open(app: &App, conversation_id: &str) {
    app.goto(&format!("/main/conversation/{}", conversation_id)).await;
    app.wait_url("/chat").await;
    app.see(&tid("composer-input")).await;
}

pub(super) async fn write(app: &App, text: &str) {
    app.type_into(&tid("composer-input"), text).await;
}

pub(super) async fn enter(app: &App, modifiers: &[&str]) {
    app.press(Some(&tid("composer-input")), "Enter", modifiers).await;
}

pub(super) async fn blocks(app: &App) -> Vec<String> {
    app.js(&format!("return [...document.querySelector({}).children].map((el) => el.getAttribute('data-type'))", serde_json::to_string(&tid("composer-input")).unwrap()))
        .await
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

pub(super) async fn composer_text(app: &App) -> String {
    app.value(&tid("composer-input")).await
}

pub(super) async fn wait_value<F, Fut>(what: &str, timeout: Duration, probe: F) -> Value
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Option<Value>>,
{
    let started = Instant::now();
    loop {
        if let Some(found) = probe().await {
            return found;
        }
        assert!(started.elapsed() < timeout, "timed out waiting for {}", what);
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

pub(super) async fn sent_message(app: &App, conversation_id: &str, text: &str) -> Value {
    wait_value(&format!("{:?} to be stored with a server id", text), Duration::from_secs(30), || async {
        let local = messages(app, conversation_id).await.into_iter().find(|m| m["content"].as_str().is_some_and(|c| c.contains(text)))?;
        let provider = local["provider_id"].as_str()?.to_string();
        let external = local["external_id"].as_str()?.to_string();
        server_messages(app, &provider).await.iter().any(|s| s["external_id"].as_str() == Some(external.as_str())).then_some(local)
    })
    .await
}

pub(super) async fn reply_after(app: &App, conversation_id: &str, mine: &Value) -> Value {
    let me = mine["sender_id"].as_str().unwrap_or_default().to_string();
    let count = messages(app, conversation_id).await.iter().position(|m| m["id"] == mine["id"]).unwrap_or(0);
    wait_value("the simulated reply", Duration::from_secs(45), || async {
        messages(app, conversation_id).await.into_iter().skip(count + 1).find(|m| m["sender_id"].as_str() != Some(me.as_str()) && m["deleted"].is_null())
    })
    .await
}

pub(super) async fn see_message(app: &App, id: &str) {
    app.see(&format!("[data-message-id=\"{}\"]", id)).await;
}

pub(super) fn sel(name: &str) -> String {
    serde_json::to_string(&tid(name)).unwrap()
}

pub(super) async fn choose(app: &App, selector: &str, value: &str) {
    app.see(selector).await;
    app.js_with("const el = document.querySelector(arguments[0]); el.value = arguments[1]; el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true }))", vec![json!(selector), json!(value)]).await;
}

pub(super) async fn open_kind(app: &App, kind: &str, types: &[&str]) -> String {
    let conversation = conversation_of(app, kind, types).await;
    let id = conversation["id"].as_str().unwrap().to_string();
    open(app, &id).await;
    id
}

async fn no_message(app: &App, conversation_id: &str, text: &str) -> bool {
    !messages(app, conversation_id).await.iter().any(|m| m["content"].as_str().is_some_and(|c| c.contains(text)))
}

async fn option(app: &App, menu: &str, value: &str) -> String {
    let selector = format!("{} [data-value={}]", tid(menu), serde_json::to_string(value).unwrap());
    app.see(&selector).await;
    selector
}

#[tokio::test]
async fn enter_sends_and_the_mock_reply_arrives_through_sync() {
    let app = App::fresh().await;
    let id = open_kind(&app, "teams", &["direct_message"]).await;
    write(&app, "Round trip from the composer").await;
    enter(&app, &[]).await;
    app.wait("the composer to clear", &format!("document.querySelector({}).innerText.trim() === ''", sel("composer-input"))).await;
    let mine = sent_message(&app, &id, "Round trip from the composer").await;
    see_message(&app, mine["id"].as_str().unwrap()).await;
    let reply = reply_after(&app, &id, &mine).await;
    see_message(&app, reply["id"].as_str().unwrap()).await;
    assert!(!reply["content"].as_str().unwrap_or_default().is_empty());
}

#[tokio::test]
async fn shift_enter_breaks_the_line_without_sending() {
    let app = App::fresh().await;
    let id = open_kind(&app, "teams", &["direct_message"]).await;
    write(&app, "first line").await;
    enter(&app, &["shift"]).await;
    write(&app, "second line").await;
    assert_eq!(blocks(&app).await, vec!["paragraph", "paragraph"]);
    let text = composer_text(&app).await;
    assert!(text.contains("first line") && text.contains("second line"), "{:?}", text);
    assert!(no_message(&app, &id, "first line").await);
    enter(&app, &["meta"]).await;
    let mine = sent_message(&app, &id, "first line").await;
    assert!(mine["content"].as_str().unwrap().contains("second line"));
}

#[tokio::test]
async fn enter_inside_lists_code_and_tables_edits_instead_of_sending() {
    let app = App::fresh().await;
    let id = open_kind(&app, "teams", &["direct_message"]).await;
    write(&app, "- ").await;
    write(&app, "milk").await;
    assert_eq!(blocks(&app).await, vec!["bullet"]);
    enter(&app, &[]).await;
    write(&app, "bread").await;
    assert_eq!(blocks(&app).await, vec!["bullet", "bullet"]);
    enter(&app, &[]).await;
    enter(&app, &[]).await;
    assert_eq!(blocks(&app).await.last().map(String::as_str), Some("paragraph"), "an empty item leaves the list");
    write(&app, "```").await;
    enter(&app, &[]).await;
    assert_eq!(blocks(&app).await.last().map(String::as_str), Some("code"));
    write(&app, "let a = 1;").await;
    enter(&app, &[]).await;
    write(&app, "let b = 2;").await;
    assert_eq!(blocks(&app).await.last().map(String::as_str), Some("code"), "enter stays inside the code block");
    enter(&app, &[]).await;
    write(&app, "```").await;
    enter(&app, &[]).await;
    assert_eq!(blocks(&app).await.last().map(String::as_str), Some("paragraph"), "a closing fence leaves the code block");
    write(&app, "| name | qty |").await;
    enter(&app, &[]).await;
    assert_eq!(blocks(&app).await.last().map(String::as_str), Some("table"));
    write(&app, "eggs | 12 |").await;
    enter(&app, &[]).await;
    let html = app.js(&format!("return document.querySelector({}).innerHTML", sel("composer-input"))).await;
    assert_eq!(blocks(&app).await.last().map(String::as_str), Some("table"), "enter adds a row: {}", html);
    enter(&app, &[]).await;
    assert_eq!(blocks(&app).await.last().map(String::as_str), Some("paragraph"), "an empty row leaves the table");
    assert!(no_message(&app, &id, "milk").await, "nothing was sent while editing");
    write(&app, "done").await;
    enter(&app, &["meta"]).await;
    let mine = sent_message(&app, &id, "milk").await;
    let content = mine["content"].as_str().unwrap();
    assert!(content.contains("bread") && content.contains("let b = 2;") && content.contains("eggs"), "{}", content);
}

#[tokio::test]
async fn the_formatting_toolbar_and_the_markdown_source_switch() {
    let app = App::fresh().await;
    open_kind(&app, "teams", &["direct_message"]).await;
    app.gone(&tid("composer-toolbar")).await;
    app.click(&tid("composer-formatting")).await;
    app.see(&tid("composer-toolbar")).await;
    app.click(&tid("composer-format-bold")).await;
    write(&app, "strong").await;
    app.see(&format!("{} strong, {} b", tid("composer-input"), tid("composer-input"))).await;
    app.click(&tid("composer-source-toggle")).await;
    app.see(&tid("composer-source")).await;
    assert!(app.value(&tid("composer-source")).await.contains("**strong**"));
    app.type_into(&tid("composer-source"), " and *slanted*").await;
    app.click(&tid("composer-source-toggle")).await;
    app.gone(&tid("composer-source")).await;
    app.see(&format!("{} em, {} i", tid("composer-input"), tid("composer-input"))).await;
    app.click(&tid("composer-block-bullet")).await;
    assert_eq!(blocks(&app).await, vec!["bullet"]);
    app.click(&tid("composer-format-heading")).await;
    app.click(&format!("{}[data-value=\"2\"]", tid("composer-heading-level"))).await;
    app.see(&format!("{} [data-type=heading][data-level=\"2\"]", tid("composer-input"))).await;
    app.click(&tid("composer-format-heading")).await;
    app.click(&tid("composer-heading-normal")).await;
    assert_eq!(blocks(&app).await, vec!["paragraph"]);
    for (button, block) in [("composer-block-quote", "quote"), ("composer-block-task", "task"), ("composer-block-ordered", "ordered")] {
        app.click(&tid(button)).await;
        assert!(blocks(&app).await.contains(&block.to_string()), "{} made no {}", button, block);
    }
    app.click(&tid("composer-formatting")).await;
    app.gone(&tid("composer-toolbar")).await;
}

#[tokio::test]
async fn slash_commands_filter_and_insert_blocks() {
    let app = App::fresh().await;
    let id = open_kind(&app, "teams", &["direct_message"]).await;
    write(&app, "/").await;
    app.see(&tid("composer-commands")).await;
    assert!(app.exists(&format!("{}[data-value=table]", tid("composer-command"))).await);
    assert!(!app.exists(&format!("{}[data-value=draft]", tid("composer-command"))).await, "drafting with AI is hidden while intelligence is off");
    write(&app, "tab").await;
    app.wait("only the table command", &format!("document.querySelectorAll({}).length === 1", sel("composer-command"))).await;
    enter(&app, &[]).await;
    app.gone(&tid("composer-commands")).await;
    assert_eq!(blocks(&app).await, vec!["table"]);
    assert!(no_message(&app, &id, "/tab").await);
    app.click(&tid("composer-commands-button")).await;
    app.see(&tid("composer-commands")).await;
    assert!(app.count(&tid("composer-command")).await >= 7);
    app.click(&format!("{}[data-value=chat]", tid("composer-command"))).await;
    app.gone(&tid("composer-commands")).await;
}

#[tokio::test]
async fn at_mentions_pick_a_member() {
    let app = App::fresh().await;
    let id = open_kind(&app, "teams", &["group_message"]).await;
    write(&app, "Hei @e").await;
    app.see(&tid("composer-mentions")).await;
    let first = app.js(&format!("return document.querySelector({}).textContent.trim()", sel("composer-mention-option"))).await.as_str().unwrap_or_default().to_string();
    assert!(!first.is_empty());
    enter(&app, &[]).await;
    app.gone(&tid("composer-mentions")).await;
    let text = composer_text(&app).await;
    assert!(text.contains(first.split_whitespace().next().unwrap()), "{:?} lacks {:?}", text, first);
    enter(&app, &["meta"]).await;
    let mine = sent_message(&app, &id, "Hei").await;
    assert!(mine["content"].as_str().unwrap().contains(first.split_whitespace().next().unwrap()));
}

#[tokio::test]
async fn colon_shortcodes_and_custom_workspace_emoji() {
    let app = App::fresh().await;
    let id = open_kind(&app, "slack", &["direct_message"]).await;
    write(&app, "nice :thumbsu").await;
    app.see(&tid("composer-shortcode-option")).await;
    let picked = app.attr(&tid("composer-shortcode-option"), "data-value").await.unwrap();
    assert!(picked.contains("thumbs") || picked == "+1", "{}", picked);
    app.click(&tid("composer-shortcode-option")).await;
    app.gone(&tid("composer-shortcodes")).await;
    app.see(&format!("{} [data-emoji]", tid("composer-input"))).await;
    write(&app, " :party_p").await;
    app.see(&tid("composer-shortcodes")).await;
    app.click(&option(&app, "composer-shortcodes", "party_parrot").await).await;
    app.gone(&tid("composer-shortcodes")).await;
    enter(&app, &[]).await;
    let mine = sent_message(&app, &id, "nice").await;
    let content = mine["content"].as_str().unwrap();
    assert!(content.contains("party_parrot"), "{}", content);
    assert!(content.contains("👍") || content.contains(&format!(":{}:", picked)), "{}", content);
}

#[tokio::test]
async fn send_as_lists_every_route_and_remembers_the_choice() {
    let app = App::fresh().await;
    let id = open_kind(&app, "teams", &["direct_message"]).await;
    let default = app.attr(&tid("composer-send-as"), "data-value").await.unwrap();
    assert!(default.starts_with(&format!("conversation:{}|", id)), "the default sends through this chat: {}", default);
    app.click(&tid("composer-send-as")).await;
    app.see(&tid("composer-send-as-menu")).await;
    let options: Vec<String> = app.js(&format!("return [...document.querySelectorAll({})].map((el) => el.dataset.value)", sel("composer-send-as-option"))).await.as_array().unwrap().iter().filter_map(|v| v.as_str().map(String::from)).collect();
    assert!(options.len() >= 2, "a person with several channels offers several routes: {:?}", options);
    assert_eq!(app.attr(&format!("{}[data-checked=\"true\"]", tid("composer-send-as-option")), "data-value").await.as_deref(), Some(default.as_str()));
    let other = options.iter().find(|o| !o.starts_with(&format!("conversation:{}|", id))).unwrap().clone();
    app.click(&format!("{}[data-value={}]", tid("composer-send-as-option"), serde_json::to_string(&other).unwrap())).await;
    app.see(&tid("composer-send-as-more")).await;
    app.click(&format!("{}[data-value={}]", tid("composer-send-as-option"), serde_json::to_string(&default).unwrap())).await;
    app.gone(&tid("composer-send-as-more")).await;
    app.press(None, "Escape", &[]).await;
    app.gone(&tid("composer-send-as-menu")).await;
    app.wait("the new route", &format!("document.querySelector({}).dataset.value === {}", sel("composer-send-as"), serde_json::to_string(&other).unwrap())).await;
    app.goto(&format!("/main/conversation/{}", id)).await;
    app.see(&tid("composer-input")).await;
    app.wait("the route survives a reload", &format!("document.querySelector({}).dataset.value === {}", sel("composer-send-as"), serde_json::to_string(&other).unwrap())).await;
    if let Some(target) = other.strip_prefix("conversation:").and_then(|rest| rest.split('|').next()) {
        let target = target.to_string();
        write(&app, "Through another route").await;
        enter(&app, &["meta"]).await;
        sent_message(&app, &target, "Through another route").await;
        app.wait_url(&target).await;
    }
}

#[tokio::test]
async fn send_from_every_account_on_a_merged_chat() {
    let app = App::fresh().await;
    let teams = conversation_of(&app, "teams", &["direct_message"]).await;
    let slack = conversation_of(&app, "slack", &["direct_message"]).await;
    let id = teams["id"].as_str().unwrap().to_string();
    app.engine("post_conversation_merge", json!({ "input": { "source_conversation_id": slack["id"], "target_conversation_id": id } })).await.unwrap();
    let merged = app.conversations().await.into_iter().find(|c| c["id"].as_str() == Some(id.as_str())).unwrap();
    assert_eq!(merged["providers"].as_array().map(|p| p.len()), Some(2), "{}", merged["providers"]);
    open(&app, &id).await;
    app.click(&tid("composer-send-as")).await;
    app.click(&tid("composer-send-every")).await;
    app.wait("every account", &format!("document.querySelector({}).dataset.mode === 'all'", sel("composer-send-as"))).await;
    write(&app, "To every account").await;
    enter(&app, &["meta"]).await;
    let started = Instant::now();
    loop {
        let providers: std::collections::HashSet<String> = messages(&app, &id).await.into_iter().filter(|m| m["content"].as_str().is_some_and(|c| c.contains("To every account"))).filter_map(|m| m["provider_id"].as_str().map(String::from)).collect();
        if providers.len() == merged["providers"].as_array().unwrap().len() {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(30), "sent through {:?} only", providers);
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[tokio::test]
async fn send_and_done_archives_the_chat() {
    let app = App::fresh().await;
    let id = open_kind(&app, "slack", &["direct_message"]).await;
    write(&app, "Sending and closing").await;
    app.click(&tid("composer-send-menu")).await;
    app.see(&tid("composer-send-list")).await;
    app.click(&tid("composer-send-done")).await;
    sent_message(&app, &id, "Sending and closing").await;
    app.wait("the inbox", "location.pathname === '/main/conversation'").await;
    app.wait_until_archived(&id).await;
}

trait Archived {
    async fn wait_until_archived(&self, id: &str);
}

impl Archived for App {
    async fn wait_until_archived(&self, id: &str) {
        let started = Instant::now();
        while !self.conversations().await.iter().any(|c| c["id"].as_str() == Some(id) && c["is_archived"].as_bool() == Some(true)) {
            assert!(started.elapsed() < Duration::from_secs(20), "{} was not archived", id);
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
}

#[tokio::test]
async fn scheduling_from_the_send_menu_and_cancelling() {
    let app = App::fresh().await;
    let id = open_kind(&app, "teams", &["direct_message"]).await;
    let before = app.count(&tid("composer-schedule")).await;
    let pending_before = app.engine("get_message_schedules", json!({ "conversationId": id })).await.unwrap().as_array().unwrap().iter().filter(|r| r["status"] == "pending").count();
    for item in ["composer-send-later", "composer-send-tomorrow", "composer-send-monday"] {
        write(&app, &format!("Scheduled by {}", item)).await;
        app.click(&tid("composer-send-menu")).await;
        app.click(&tid(item)).await;
        app.wait("the composer to clear", &format!("document.querySelector({}).innerText.trim() === ''", sel("composer-input"))).await;
    }
    write(&app, "Scheduled at a chosen time").await;
    app.click(&tid("composer-send-menu")).await;
    app.fill(&tid("composer-send-at"), "2030-01-02T09:30").await;
    app.click(&tid("composer-send-at-set")).await;
    app.wait("four more scheduled rows", &format!("document.querySelectorAll({}).length === {}", sel("composer-schedule"), before + 4)).await;
    let pending = app.engine("get_message_schedules", json!({ "conversationId": id })).await.unwrap();
    let rows: Vec<&Value> = pending.as_array().unwrap().iter().filter(|r| r["status"] == "pending").collect();
    assert_eq!(rows.len(), pending_before + 4);
    assert!(rows.iter().any(|r| r["at"].as_str().is_some_and(|at| at.starts_with("2030-01-02"))));
    assert!(no_message(&app, &id, "Scheduled by").await, "scheduled messages wait");
    app.js(&format!("[...document.querySelectorAll({})].find((row) => row.textContent.includes('Scheduled at a chosen time')).querySelector({}).click()", sel("composer-schedule"), sel("composer-schedule-cancel"))).await;
    app.wait("one row fewer", &format!("document.querySelectorAll({}).length === {}", sel("composer-schedule"), before + 3)).await;
    let left = app.engine("get_message_schedules", json!({ "conversationId": id })).await.unwrap();
    assert_eq!(left.as_array().unwrap().iter().filter(|r| r["status"] == "pending").count(), pending_before + 3);
    assert!(!left.as_array().unwrap().iter().any(|r| r["status"] == "pending" && r["content"].as_str() == Some("Scheduled at a chosen time")));
}

#[tokio::test]
async fn send_when_an_action_fires_schedules_on_the_action() {
    let app = App::fresh().await;
    let id = open_kind(&app, "teams", &["direct_message"]).await;
    write(&app, "Send when the action fires").await;
    app.click(&tid("composer-send-menu")).await;
    let action = app.attr(&tid("composer-send-when"), "data-value").await.unwrap();
    app.click(&tid("composer-send-when")).await;
    app.wait("the action schedule row", &format!("[...document.querySelectorAll({})].some((row) => row.textContent.includes('Send when the action fires'))", sel("composer-schedule"))).await;
    let rows = app.engine("get_message_schedules", json!({ "conversationId": id })).await.unwrap();
    assert!(rows.as_array().unwrap().iter().any(|r| r["action_id"].as_str() == Some(action.as_str()) && r["status"] == "pending"));
    write(&app, "a draft keeps the menu open").await;
    app.click(&tid("composer-send-menu")).await;
    app.click(&tid("composer-send-new-action")).await;
    app.wait_url("/main/actions").await;
}

#[tokio::test]
async fn mail_waits_in_a_countdown_and_undo_restores_the_draft() {
    let app = App::fresh().await;
    let id = open_kind(&app, "gmail", &["email"]).await;
    app.see(&tid("composer-mail-to")).await;
    write(&app, "Undo me before it leaves").await;
    enter(&app, &[]).await;
    assert!(composer_text(&app).await.contains("Undo me"), "enter does not send mail");
    enter(&app, &["meta"]).await;
    app.see(&format!("{}[data-status=pending]", tid("composer-hold"))).await;
    app.wait("the composer to clear", &format!("document.querySelector({}).innerText.trim() === ''", sel("composer-input"))).await;
    let holds = app.engine("get_outbound_holds", json!({ "conversationId": id })).await.unwrap();
    assert_eq!(holds.as_array().unwrap().iter().filter(|h| h["status"] == "pending").count(), 1);
    app.click(&tid("composer-hold-undo")).await;
    app.gone(&tid("composer-hold")).await;
    app.wait("the draft to come back", &format!("document.querySelector({}).innerText.includes('Undo me before it leaves')", sel("composer-input"))).await;
    let holds = app.engine("get_outbound_holds", json!({ "conversationId": id })).await.unwrap();
    assert_eq!(holds.as_array().unwrap().iter().filter(|h| h["status"] == "pending").count(), 0);
    assert!(no_message(&app, &id, "Undo me before it leaves").await, "the undone mail never left");
    enter(&app, &["meta"]).await;
    app.see(&format!("{}[data-status=pending]", tid("composer-hold"))).await;
    sent_message(&app, &id, "Undo me before it leaves").await;
    app.gone(&tid("composer-hold")).await;
}

#[tokio::test]
async fn reply_bar_quotes_the_pointed_message() {
    let app = App::fresh().await;
    let id = open_kind(&app, "teams", &["direct_message"]).await;
    let me: Vec<Value> = app.engine("get_my_user_ids", json!({})).await.unwrap().as_array().cloned().unwrap_or_default();
    let target = messages(&app, &id).await.into_iter().rev().find(|m| !me.contains(&m["sender_id"]) && m["deleted"].is_null()).unwrap();
    let target_id = target["id"].as_str().unwrap().to_string();
    see_message(&app, &target_id).await;
    app.js_with("document.querySelector(`[data-message-id=\"${arguments[0]}\"]`).closest('article').dispatchEvent(new MouseEvent('mouseenter'))", vec![json!(target_id)]).await;
    app.press(None, "r", &[]).await;
    app.see(&tid("composer-reply-bar")).await;
    app.click(&tid("composer-reply-cancel")).await;
    app.gone(&tid("composer-reply-bar")).await;
    app.js_with("document.querySelector(`[data-message-id=\"${arguments[0]}\"]`).closest('article').dispatchEvent(new MouseEvent('mouseenter'))", vec![json!(target_id)]).await;
    app.press(None, "r", &[]).await;
    app.see(&tid("composer-reply-bar")).await;
    write(&app, "Answering that one").await;
    assert!(composer_text(&app).await.contains("Answering that one"), "typed into the composer: {:?}", composer_text(&app).await);
    enter(&app, &[]).await;
    app.wait("the reply to send", &format!("!document.querySelector({}) || document.querySelector({})", sel("composer-reply-bar"), sel("composer-error"))).await;
    assert!(!app.exists(&tid("composer-error")).await, "{}", app.text(&tid("composer-error")).await);
    let mine = sent_message(&app, &id, "Answering that one").await;
    let metadata = mine["metadata"].as_str().unwrap_or_default();
    assert!(mine["reply_to_id"].as_str().is_some() || metadata.contains("quote"), "the reply carries its quote: {}", mine);
}

#[tokio::test]
async fn a_refused_send_shows_the_error_and_keeps_the_draft() {
    let app = App::fresh().await;
    let id = open_kind(&app, "teams", &["direct_message"]).await;
    app.engine("put_mock_state", json!({ "providerId": app.mock_provider("teams"), "state": "denied" })).await.unwrap();
    write(&app, "This one is refused").await;
    enter(&app, &[]).await;
    app.see(&tid("composer-error")).await;
    assert!(app.text(&tid("composer-error")).await.contains("403"));
    assert!(composer_text(&app).await.contains("This one is refused"));
    assert!(no_message(&app, &id, "This one is refused").await);
    app.engine("put_mock_state", json!({ "providerId": app.mock_provider("teams"), "state": "ok" })).await.unwrap();
    enter(&app, &[]).await;
    sent_message(&app, &id, "This one is refused").await;
    app.gone(&tid("composer-error")).await;
}

#[tokio::test]
async fn the_more_menu_and_the_emoji_picker() {
    let app = App::fresh().await;
    open_kind(&app, "teams", &["direct_message"]).await;
    app.click(&tid("composer-more")).await;
    app.see(&tid("composer-more-files")).await;
    assert_eq!(app.attr(&tid("composer-more-draft"), "data-value").await.as_deref(), Some("off"));
    app.click(&tid("composer-more-draft")).await;
    app.gone(&tid("composer-more-files")).await;
    assert!(!app.exists(&tid("composer-error")).await, "drafting is not attempted with intelligence off");
    app.see(&tid("composer-needs")).await;
    app.click(&tid("composer-mention")).await;
    assert!(composer_text(&app).await.contains('@'));
}


#[tokio::test]
async fn replying_to_mail_opens_the_compose_sheet_and_shows_refusals() {
    let app = App::fresh().await;
    app.put_setting("mail.send_delay_seconds", "0").await;
    let id = open_kind(&app, "gmail", &["email"]).await;
    let me: Vec<Value> = app.engine("get_my_user_ids", json!({})).await.unwrap().as_array().cloned().unwrap_or_default();
    let target = messages(&app, &id).await.into_iter().rev().find(|m| !me.contains(&m["sender_id"]) && m["deleted"].is_null()).unwrap();
    let target_id = target["id"].as_str().unwrap().to_string();
    see_message(&app, &target_id).await;
    app.js("document.activeElement && document.activeElement.blur()").await;
    app.js_with("document.querySelector(`[data-message-id=\"${arguments[0]}\"]`).closest('article').dispatchEvent(new MouseEvent('mouseenter'))", vec![json!(target_id)]).await;
    app.press(None, "r", &[]).await;
    app.see(&tid("email-compose")).await;
    app.wait("the reply subject", &format!("/^(re|sv):/i.test(document.querySelector({}).value)", sel("email-compose-subject"))).await;
    app.engine("put_mock_state", json!({ "providerId": app.mock_provider("gmail"), "state": "denied" })).await.unwrap();
    app.type_into(&tid("email-compose-body"), "Replying from the compose sheet").await;
    app.click(&tid("email-compose-send")).await;
    app.see(&tid("email-compose-error")).await;
    assert!(app.text(&tid("email-compose-error")).await.contains("403"));
    app.engine("put_mock_state", json!({ "providerId": app.mock_provider("gmail"), "state": "ok" })).await.unwrap();
    app.click(&tid("email-compose-send")).await;
    app.gone(&tid("email-compose")).await;
    let mine = sent_message(&app, &id, "Replying from the compose sheet").await;
    let subject = mine["subject"].as_str().unwrap_or_default().to_lowercase();
    assert!(subject.starts_with("re") || subject.starts_with("sv"), "{}", mine);
}
