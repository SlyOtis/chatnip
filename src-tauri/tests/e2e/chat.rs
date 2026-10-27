use super::support::{js_string, tid, App};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

async fn db(app: &App, sql: &str) -> Value {
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::{ConnectOptions, Row};
    let path = app.home.join("Library/Application Support/com.slyotis.chatnip/chatnip.db");
    let mut connection = SqliteConnectOptions::new().filename(&path).read_only(true).connect().await.unwrap();
    let Some(row) = sqlx::query(sql).fetch_optional(&mut connection).await.unwrap_or_else(|e| panic!("query failed: {}\n{}", e, sql)) else {
        return Value::Null;
    };
    let text: Option<String> = row.try_get(0).unwrap();
    serde_json::from_str(&text.unwrap_or_else(|| "null".into())).unwrap()
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

async fn eventually<F, Fut>(what: &str, probe: F) -> Value
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Option<Value>>,
{
    let started = Instant::now();
    loop {
        if let Some(found) = probe().await {
            return found;
        }
        assert!(started.elapsed() < Duration::from_secs(30), "timed out waiting for {}", what);
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn conversation(app: &App, kind: &str, name: &str) -> Value {
    let provider = app.mock_provider(kind);
    let mut best: Option<(i64, Value)> = None;
    for c in app.conversations().await {
        let named = c["provider_name"].as_str() == Some(name) || c["name"].as_str() == Some(name);
        let here = c["providers"].as_array().is_some_and(|p| p.iter().any(|p| p["id"].as_str() == Some(provider.as_str())));
        if !named || !here {
            continue;
        }
        let count = db(app, &format!("SELECT json(COUNT(*)) FROM message WHERE conversation_id = {}", quote(c["id"].as_str().unwrap_or_default()))).await.as_i64().unwrap_or(0);
        if best.as_ref().map(|(n, _)| count > *n).unwrap_or(true) {
            best = Some((count, c));
        }
    }
    best.map(|(_, c)| c).unwrap_or_else(|| panic!("no {} conversation called {}", kind, name))
}

async fn local(app: &App, name: &str) -> Value {
    let chats = app.engine("get_chats", json!({})).await.unwrap();
    let chat = chats.as_array().unwrap().iter().find(|c| c["name"].as_str() == Some(name)).unwrap_or_else(|| panic!("no local chat called {}", name)).clone();
    json!({ "id": chat["conversation_id"], "chat": chat })
}

async fn fetch(app: &App, id: &str) -> Value {
    app.conversations().await.into_iter().find(|c| c["id"].as_str() == Some(id)).unwrap_or(Value::Null)
}

async fn until_conversation<F: Fn(&Value) -> bool>(app: &App, id: &str, what: &str, test: F) -> Value {
    let started = Instant::now();
    loop {
        let found = fetch(app, id).await;
        if test(&found) {
            return found;
        }
        assert!(started.elapsed() < Duration::from_secs(20), "timed out waiting for {}: name {} provider_name {} overrides {} mode {} view {} archived {} level {}", what, found["name"], found["provider_name"], json!(found["overrides"].as_object().map(|o| o.keys().cloned().collect::<Vec<_>>())), found["mode"], found["view"], found["is_archived"], found["notification_level"]);
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn message(app: &App, id: &str) -> Value {
    db(app, &format!("SELECT json_object('id', id, 'content', content, 'sender_id', sender_id, 'deleted', deleted_datetime, 'edited', last_modified_datetime, 'metadata', metadata, 'reply_to_id', reply_to_id, 'conversation_id', conversation_id) FROM message WHERE id = {}", quote(id))).await
}

async fn open(app: &App, id: &str) {
    app.goto(&format!("/main/conversation/{}", id)).await;
    app.wait_url("/chat").await;
    app.see(&tid("chat-header")).await;
}

async fn open_with_messages(app: &App, id: &str) {
    open(app, id).await;
    app.see("[data-message-id]").await;
}

async fn details(app: &App) {
    if !app.exists(&tid("details-panel")).await {
        app.click(&tid("chat-info")).await;
    }
    app.see(&tid("details-panel")).await;
}

fn within(scope: &str, selector: &str) -> String {
    format!("{} {}", scope, selector)
}

fn line(id: &str) -> String {
    format!("{}[data-id=\"{}\"]", tid("message"), id)
}

async fn message_ids(app: &App, mine: bool) -> Vec<String> {
    app.js(&format!("return [...document.querySelectorAll('{}[data-mine=\"{}\"]')].filter((el) => el.querySelector('.selectable')).map((el) => el.dataset.id)", tid("message"), mine))
        .await
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

async fn menu_for(app: &App, id: &str) {
    app.context_click(&line(id)).await;
    app.see(&format!("{}[data-id=\"{}\"]", tid("message-menu"), id)).await;
}

async fn press_enabled(app: &App, selector: &str) {
    app.wait(&format!("{} to be enabled", selector), &format!("document.querySelector({}) && !document.querySelector({}).disabled", js_string(selector), js_string(selector))).await;
    app.click(selector).await;
}

async fn layout_setting(app: &App, prefix: &str) -> Option<String> {
    let settings = app.engine("get_app_settings", json!({})).await.unwrap();
    settings.as_object().and_then(|map| map.iter().find(|(k, _)| k.starts_with(prefix)).and_then(|(_, v)| v.as_str().map(String::from)))
}

#[tokio::test]
async fn header_actions_mark_done_snooze_tag_delete_and_notify() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Start of Week Sync").await;
    let id = chat["id"].as_str().unwrap().to_string();
    open(&app, &id).await;
    assert!(!app.text(&tid("chat-title")).await.is_empty());

    app.click(&tid("chat-done")).await;
    until_conversation(&app, &id, "the chat to be done", |c| c["is_archived"] == true).await;
    app.click(&tid("chat-done")).await;
    until_conversation(&app, &id, "the chat back in the inbox", |c| c["is_archived"] == false).await;

    let sheets = app.count("[role=\"dialog\"]").await;
    app.click(&tid("chat-snooze")).await;
    app.wait("the snooze sheet", &format!("document.querySelectorAll('[role=\"dialog\"]').length > {}", sheets)).await;
    app.press(None, "Escape", &[]).await;
    app.wait("the snooze sheet to close", &format!("document.querySelectorAll('[role=\"dialog\"]').length === {}", sheets)).await;
    app.click(&tid("chat-tag")).await;
    app.wait("the tag sheet", &format!("document.querySelectorAll('[role=\"dialog\"]').length > {}", sheets)).await;
    app.press(None, "Escape", &[]).await;
    app.wait("the tag sheet to close", &format!("document.querySelectorAll('[role=\"dialog\"]').length === {}", sheets)).await;

    app.click(&tid("chat-notify")).await;
    app.click(&tid("chat-notify-mentions")).await;
    until_conversation(&app, &id, "mentions only", |c| c["notification_level"] == "mentions").await;
    app.click(&tid("chat-notify")).await;
    app.click(&tid("chat-notify-all")).await;
    until_conversation(&app, &id, "all notifications", |c| c["notification_level"] == "all").await;

    app.click(&tid("chat-delete")).await;
    until_conversation(&app, &id, "the chat to be deleted", |c| !c["deleted_at"].is_null()).await;
    app.click(&tid("chat-delete")).await;
    until_conversation(&app, &id, "the chat to be restored", |c| c["deleted_at"].is_null()).await;
}

#[tokio::test]
async fn the_info_button_toggles_the_details_panel() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Start of Week Sync").await;
    open(&app, chat["id"].as_str().unwrap()).await;
    app.click(&tid("chat-info")).await;
    app.wait("the details panel to open", &details_open(true)).await;
    app.wait("the members", &format!("document.querySelectorAll('{}').length > 1", tid("member"))).await;
    app.click(&tid("details-close")).await;
    app.wait("the details panel to close", &details_open(false)).await;
    app.press(None, "D", &["meta", "shift"]).await;
    app.wait("the details panel to open again", &details_open(true)).await;
}

#[tokio::test]
async fn details_members_open_the_person_and_take_a_nickname_for_this_chat() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Start of Week Sync").await;
    let id = chat["id"].as_str().unwrap().to_string();
    open(&app, &id).await;
    details(&app).await;
    let member = app.js(&format!("const el = [...document.querySelectorAll('{}')].find((m) => m.querySelector('{}')); return el ? el.dataset.id : null", tid("member"), tid("member-nickname-edit"))).await;
    let member = member.as_str().expect("a member that is a known person").to_string();
    let row = format!("{}[data-id=\"{}\"]", tid("member"), member);
    app.click(&within(&row, &tid("member-nickname-edit"))).await;
    app.fill(&within(&row, &tid("member-nickname-field")), "Captain E2E").await;
    app.click(&within(&row, &tid("member-nickname-save"))).await;
    until_conversation(&app, &id, "the nickname", |c| c["nicknames"][&member] == "Captain E2E").await;
    app.wait("the nickname mark", &format!("document.querySelector('{}')?.dataset.nickname === 'Captain E2E'", row.replace('\'', "\\'"))).await;
    app.click(&within(&row, &tid("member-nickname-reset"))).await;
    until_conversation(&app, &id, "the nickname to reset", |c| c["nicknames"].get(&member).is_none()).await;

    app.click(&within(&row, &tid("member-open"))).await;
    app.wait_url("/main/person/").await;
}

#[tokio::test]
async fn details_topic_round_trips_into_the_conversation_metadata() {
    let app = App::fresh().await;
    let chat = conversation(&app, "slack", "#dev").await;
    let id = chat["id"].as_str().unwrap().to_string();
    open(&app, &id).await;
    details(&app).await;
    app.fill(&tid("chat-topic"), "Ship the E2E suite").await;
    app.press(Some(&tid("chat-topic")), "Enter", &[]).await;
    until_conversation(&app, &id, "the topic", |c| c["metadata"].as_str().is_some_and(|m| m.contains("Ship the E2E suite"))).await;
}

#[tokio::test]
async fn details_mode_and_view_switch_the_chat_between_write_read_and_grid() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "#dev").await;
    let id = chat["id"].as_str().unwrap().to_string();
    open(&app, &id).await;
    details(&app).await;
    assert!(app.exists(&tid("composer")).await);
    app.click(&format!("{} [data-value=\"read\"]", tid("chat-mode"))).await;
    until_conversation(&app, &id, "read mode", |c| c["mode"] == "read").await;
    app.gone(&tid("composer")).await;
    app.click(&format!("{} [data-value=\"grid\"]", tid("chat-view"))).await;
    until_conversation(&app, &id, "the grid view", |c| c["view"] == "grid").await;
    app.see(&tid("chat-grid")).await;
    app.click(&format!("{} [data-value=\"chat\"]", tid("chat-view"))).await;
    until_conversation(&app, &id, "the chat view", |c| c["view"] == "chat").await;
    app.gone(&tid("chat-grid")).await;
    app.click(&format!("{} [data-value=\"write\"]", tid("chat-mode"))).await;
    until_conversation(&app, &id, "write mode", |c| c["mode"] == "write").await;
    app.see(&tid("composer")).await;
}

async fn source_count(app: &App, id: &str) -> usize {
    let chats = app.engine("get_chats", json!({})).await.unwrap();
    chats.as_array().unwrap().iter().find(|c| c["conversation_id"].as_str() == Some(id)).map(|c| c["sources"].as_array().map(|s| s.len()).unwrap_or(0)).unwrap_or(0)
}

async fn attached_count(app: &App, id: &str) -> usize {
    let actions = app.engine("get_actions", json!({})).await.unwrap();
    actions.as_array().unwrap().iter().filter(|a| a["chats"].as_array().is_some_and(|c| c.iter().any(|c| c.as_str() == Some(id)))).count()
}

#[tokio::test]
async fn details_sources_of_a_local_chat_add_and_remove_a_conversation() {
    let app = App::fresh().await;
    let chat = local(&app, "Release notes").await;
    let id = chat["id"].as_str().unwrap().to_string();
    open(&app, &id).await;
    details(&app).await;
    let before = source_count(&app, &id).await;
    app.click(&tid("chat-sources-edit")).await;
    app.click(&format!("{}[data-value=\"conversation\"]", tid("source-kind"))).await;
    let pick = format!("{}[aria-pressed=\"false\"]", tid("source-choice"));
    let added = app.attr(&pick, "data-id").await.unwrap();
    app.click(&pick).await;
    let started = Instant::now();
    while source_count(&app, &id).await != before + 1 {
        assert!(started.elapsed() < Duration::from_secs(20), "the source was not added; error: {:?}", app.js(&format!("return document.querySelector('{}')?.textContent ?? null", tid("chat-settings-error"))).await);
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let chosen = format!("{}[data-id=\"{}\"]", tid("source-chosen"), added);
    app.see(&chosen).await;
    app.click(&within(&chosen, &tid("source-remove"))).await;
    let started = Instant::now();
    while source_count(&app, &id).await != before {
        assert!(started.elapsed() < Duration::from_secs(20), "the source was not removed");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test]
async fn details_pipe_attaches_an_action_shows_its_step_and_detaches_it() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "#dev").await;
    let id = chat["id"].as_str().unwrap().to_string();
    open(&app, &id).await;
    details(&app).await;
    let before = attached_count(&app, &id).await;
    app.click(&tid("chat-actions-attach")).await;
    let choice = app.attr(&tid("action-choice"), "data-id").await.unwrap();
    app.click(&format!("{}[data-id=\"{}\"]", tid("action-choice"), choice)).await;
    let started = Instant::now();
    while attached_count(&app, &id).await != before + 1 {
        assert!(started.elapsed() < Duration::from_secs(20), "the action was not attached");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let step = format!("{}[data-id=\"{}\"]", tid("pipe-step"), choice);
    app.see(&step).await;
    app.click(&within(&step, &tid("pipe-detach"))).await;
    app.gone(&step).await;
    assert_eq!(attached_count(&app, &id).await, before);
}

#[tokio::test]
async fn the_chat_image_opens_the_dialog_that_renames_locally_and_resets() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Sundial Design").await;
    let id = chat["id"].as_str().unwrap().to_string();
    let original = chat["provider_name"].as_str().unwrap().to_string();
    open(&app, &id).await;
    app.click(&tid("chat-avatar")).await;
    app.see(&tid("chat-dialog")).await;
    app.fill(&tid("chat-dialog-name"), "Design crew").await;
    app.js(&format!("document.querySelector('{}').requestSubmit()", tid("chat-dialog-name-form"))).await;
    let renamed = until_conversation(&app, &id, "the local name", |c| c["name"] == "Design crew").await;
    assert_eq!(renamed["provider_name"].as_str(), Some(original.as_str()));
    assert_eq!(renamed["overrides"]["name"], "Design crew");
    app.wait("the title to follow", &format!("document.querySelector('{}').textContent.includes('Design crew')", tid("chat-title"))).await;
    press_enabled(&app, &tid("chat-dialog-reset-name")).await;
    let reset = until_conversation(&app, &id, "the name to reset", |c| c["name"].as_str() == Some(original.as_str())).await;
    assert!(reset["overrides"].get("name").is_none());
    app.click(&tid("chat-dialog-close")).await;
    app.gone(&tid("chat-dialog")).await;
}

#[tokio::test]
async fn the_chat_dialog_renames_on_the_provider_too() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Sundial Design").await;
    let id = chat["id"].as_str().unwrap().to_string();
    open(&app, &id).await;
    app.click(&tid("chat-avatar")).await;
    app.see(&tid("chat-dialog")).await;
    app.click(&tid("chat-dialog-also-provider")).await;
    app.fill(&tid("chat-dialog-name"), "Sundial Design Team").await;
    app.click(&tid("chat-dialog-rename")).await;
    let renamed = until_conversation(&app, &id, "the provider name", |c| c["provider_name"] == "Sundial Design Team").await;
    assert_eq!(renamed["name"], "Sundial Design Team");
    assert!(renamed["overrides"].get("name").is_none());
    app.see(&format!("{}[data-noted=\"true\"]", tid("chat-dialog-note"))).await;
}

#[tokio::test]
async fn the_chat_dialog_sets_colour_description_picture_notifications_and_mode() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Sundial Design").await;
    let id = chat["id"].as_str().unwrap().to_string();
    open(&app, &id).await;
    app.click(&tid("chat-avatar")).await;
    app.see(&tid("chat-dialog")).await;

    app.click(&within(&tid("chat-dialog"), &tid("colour-swatch"))).await;
    let colour = app.attr(&tid("colour-option"), "data-value").await.unwrap();
    app.click(&tid("colour-option")).await;
    until_conversation(&app, &id, "the local colour", |c| c["overrides"]["color"].as_str() == Some(colour.as_str())).await;
    app.click(&within(&tid("chat-dialog"), &tid("colour-swatch"))).await;
    app.click(&tid("colour-clear")).await;
    until_conversation(&app, &id, "the colour to reset", |c| c["overrides"].get("color").is_none()).await;

    app.fill(&tid("chat-dialog-description"), "Where the design talk lives").await;
    app.press(Some(&tid("chat-dialog-description")), "Enter", &[]).await;
    until_conversation(&app, &id, "the description", |c| c["overrides"]["description"] == "Where the design talk lives").await;
    press_enabled(&app, &tid("chat-dialog-reset-description")).await;
    until_conversation(&app, &id, "the description to reset", |c| c["overrides"].get("description").is_none()).await;

    app.click(&tid("chat-dialog-picture")).await;
    app.see(&tid("image-library-item")).await;
    app.click(&tid("image-library-item")).await;
    app.wait("the picture to be taken or refused", &format!("!document.querySelector('{}') || document.querySelector('{}')", tid("image-library-item"), tid("image-upload-error"))).await;
    if app.exists(&tid("image-upload-error")).await {
        panic!("picking a library image as the chat picture failed: {}", app.text(&tid("image-upload-error")).await);
    }
    until_conversation(&app, &id, "the local picture", |c| c["overrides"]["avatar_data"].as_str().is_some_and(|s| !s.is_empty())).await;
    press_enabled(&app, &tid("chat-dialog-reset-avatar_data")).await;
    until_conversation(&app, &id, "the picture to reset", |c| c["overrides"].get("avatar_data").is_none()).await;

    app.click(&format!("{} [data-value=\"mentions\"]", tid("chat-dialog-notify"))).await;
    until_conversation(&app, &id, "mentions only", |c| c["notification_level"] == "mentions").await;
    app.click(&format!("{} {} [data-value=\"read\"]", tid("chat-dialog"), tid("chat-mode"))).await;
    until_conversation(&app, &id, "read mode", |c| c["mode"] == "read").await;

    app.click(&tid("chat-dialog-panel")).await;
    app.gone(&tid("chat-dialog")).await;
    app.see(&tid("details-panel")).await;
}

#[tokio::test]
async fn sections_collapse_and_expand_one_and_all() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Start of Week Sync").await;
    let id = chat["id"].as_str().unwrap().to_string();
    open_with_messages(&app, &id).await;
    app.see(&tid("section")).await;
    let total = app.count(&tid("section")).await;
    assert!(total > 1, "the seeded chat should span several sections, found {}", total);
    let first = app.attr(&tid("section"), "data-id").await.unwrap();
    let one = format!("{}[data-id=\"{}\"]", tid("section"), first);
    app.click(&within(&one, &tid("section-toggle"))).await;
    app.see(&format!("{}[data-collapsed=\"true\"]", one)).await;
    assert!(layout_setting(&app, "messages.sections.").await.is_some_and(|v| v.contains(&first)));
    app.click(&within(&one, &tid("section-toggle"))).await;
    app.see(&format!("{}[data-collapsed=\"false\"]", one)).await;

    details(&app).await;
    app.click(&tid("sections-toggle-all")).await;
    app.wait("every section collapsed", &format!("document.querySelectorAll('{}[data-collapsed=\"true\"]').length === {}", tid("section"), total)).await;
    app.click(&tid("sections-toggle-all")).await;
    app.wait("every section open", &format!("document.querySelectorAll('{}[data-collapsed=\"false\"]').length === {}", tid("section"), total)).await;
    app.press(Some("body"), "z", &[]).await;
    app.wait("the Z shortcut to collapse all", &format!("document.querySelectorAll('{}[data-collapsed=\"true\"]').length === {}", tid("section"), total)).await;
}

#[tokio::test]
async fn summarize_without_ai_shows_what_settings_are_needed() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Start of Week Sync").await;
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    assert!(!app.exists(&tid("section-needs")).await, "nothing warns before the first click");
    details(&app).await;
    app.click(&tid("sections-summarize")).await;
    app.see(&tid("sections-needs")).await;
    assert!(!app.exists(&tid("summarize-popover")).await, "AI is off, so the first click warns instead of opening the menu");
    app.click(&tid("sections-needs")).await;
    app.wait_url("/main/profile").await;
}

#[tokio::test]
async fn a_section_needs_button_deep_links_into_settings() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Start of Week Sync").await;
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    app.js(&format!("document.querySelector('{}').click()", tid("section-summarize"))).await;
    app.see(&tid("section-needs")).await;
    app.click(&tid("section-needs")).await;
    app.wait_url("/main/profile").await;
}

#[tokio::test]
async fn system_messages_link_people_to_their_page() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Start of Week Sync").await;
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    app.see(&tid("person-link")).await;
    let person = app.attr(&tid("person-link"), "data-person").await.unwrap();
    app.click(&tid("person-link")).await;
    app.wait_url(&format!("/main/person/{}", person)).await;
}

#[tokio::test]
async fn a_reaction_is_added_from_the_menu_and_removed_from_its_chip() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Eva Ortega").await;
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    let ids = message_ids(&app, false).await;
    let id = ids.last().expect("a message from the other side").clone();
    menu_for(&app, &id).await;
    let emoji = app.attr(&tid("quick-reaction"), "data-emoji").await.unwrap();
    app.click(&tid("quick-reaction")).await;
    let chip = within(&line(&id), &tid("reaction"));
    app.see(&chip).await;
    eventually("the reaction in the message metadata", || async { Some(message(&app, &id).await).filter(|m| m["metadata"].as_str().is_some_and(|meta| meta.contains("reactions") && !meta.contains("\"reactions\":[]"))) }).await;
    app.click(&chip).await;
    app.gone(&chip).await;
    let _ = emoji;
}

#[tokio::test]
async fn a_tagged_message_shows_its_tag_and_links_back_from_the_tag_list() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Eva Ortega").await;
    let conversation_id = chat["id"].as_str().unwrap().to_string();
    open_with_messages(&app, &conversation_id).await;
    let id = message_ids(&app, false).await.first().cloned().expect("a message");
    menu_for(&app, &id).await;
    app.click(&tid("menu-tags")).await;
    app.see(&tid("tag-picker")).await;
    let tag = app.attr(&tid("tag-picker-item"), "data-id").await.unwrap();
    app.click(&format!("{}[data-id=\"{}\"]", tid("tag-picker-item"), tag)).await;
    app.see(&format!("[data-message-id=\"{}\"] [data-message-tags]", id)).await;
    eventually("the tag on the message", || async {
        let tags = app.engine("get_object_tags", json!({ "objectType": "message", "objectId": id })).await.unwrap();
        tags.as_array().unwrap().iter().any(|t| t["tag_id"].as_str() == Some(tag.as_str())).then(|| json!(true))
    })
    .await;
    app.press(None, "Escape", &[]).await;

    app.goto("/main/conversation").await;
    app.click(&format!("{}[data-id=\"{}\"] button", tid("rail-tag"), tag)).await;
    let tagged = format!("[data-tagged-object=\"{}\"]", id);
    app.click(&tagged).await;
    app.wait_url(&format!("#message-{}", id)).await;
    app.wait_url(&conversation_id).await;
    app.see(&format!("[data-message-id=\"{}\"]", id)).await;
}

#[tokio::test]
async fn pinning_a_message_lists_it_in_the_details_and_unpinning_removes_it() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Eva Ortega").await;
    let conversation_id = chat["id"].as_str().unwrap().to_string();
    open_with_messages(&app, &conversation_id).await;
    let id = message_ids(&app, false).await.last().cloned().expect("a message");
    menu_for(&app, &id).await;
    app.click(&tid("menu-pin")).await;
    eventually("the pin", || async {
        let pins = app.engine("get_pinned_messages", json!({ "input": { "conversation_id": conversation_id } })).await.unwrap();
        pins.as_array().unwrap().iter().any(|p| p["message_id"].as_str() == Some(id.as_str())).then(|| json!(true))
    })
    .await;
    details(&app).await;
    let pinned = format!("{}[data-id=\"{}\"]", tid("pinned-message"), id);
    app.see(&pinned).await;
    app.click(&within(&pinned, &tid("pinned-unpin"))).await;
    app.gone(&pinned).await;
    let pins = app.engine("get_pinned_messages", json!({ "input": { "conversation_id": conversation_id } })).await.unwrap();
    assert!(!pins.as_array().unwrap().iter().any(|p| p["message_id"].as_str() == Some(id.as_str())));
}

#[tokio::test]
async fn bookmarking_a_message_saves_it() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Eva Ortega").await;
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    let id = message_ids(&app, false).await.last().cloned().expect("a message");
    menu_for(&app, &id).await;
    app.click(&tid("menu-bookmark")).await;
    eventually("the bookmark", || async {
        let saved = app.engine("get_bookmarks", json!({})).await.unwrap();
        saved.as_array().unwrap().iter().any(|b| b["message_id"].as_str() == Some(id.as_str())).then(|| json!(true))
    })
    .await;
}

#[tokio::test]
async fn reply_with_quote_sends_a_reply_that_quotes_the_original() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Eva Ortega").await;
    let conversation_id = chat["id"].as_str().unwrap().to_string();
    open_with_messages(&app, &conversation_id).await;
    let id = message_ids(&app, false).await.last().cloned().expect("a message");
    menu_for(&app, &id).await;
    app.click(&tid("menu-reply-quote")).await;
    app.see(&tid("composer-reply-bar")).await;
    app.type_into(&tid("composer-input"), "Quoting you on this").await;
    app.press(Some(&tid("composer-input")), "Enter", &[]).await;
    let sent = eventually("the reply to be stored", || async {
        let found = db(&app, &format!("SELECT json_object('id', id, 'reply_to_id', reply_to_id) FROM message WHERE conversation_id = {} AND content LIKE '%Quoting you on this%' ORDER BY timestamp DESC LIMIT 1", quote(&conversation_id))).await;
        (!found.is_null()).then_some(found)
    })
    .await;
    assert!(sent["reply_to_id"].as_str().is_some(), "the reply carries the original: {}", sent);
    app.gone(&tid("composer-reply-bar")).await;
    app.see(&format!("{}[data-reply=\"{}\"]", tid("reply-quote"), sent["id"].as_str().unwrap())).await;
}

#[tokio::test]
async fn the_forward_dialog_previews_and_forwards_to_another_chat() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Eva Ortega").await;
    let target = conversation(&app, "teams", "Nora Gale").await;
    let target_id = target["id"].as_str().unwrap().to_string();
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    let id = message_ids(&app, false).await.last().cloned().expect("a message");
    let before = db(&app, &format!("SELECT json(COUNT(*)) FROM message WHERE conversation_id = {}", quote(&target_id))).await.as_i64().unwrap();
    menu_for(&app, &id).await;
    app.click(&tid("menu-forward")).await;
    app.see(&tid("forward-dialog")).await;
    app.see(&format!("{}[data-id=\"{}\"]", tid("forward-preview"), id)).await;
    assert_eq!(app.attr(&tid("forward-send"), "disabled").await.is_some(), true);
    app.fill(&tid("forward-comment"), "Have a look").await;
    app.fill(&tid("source-find"), "Nora").await;
    app.click(&format!("{}[data-id=\"{}\"]", tid("source-merge-choice"), target_id)).await;
    app.see(&format!("{}[data-id=\"{}\"]", tid("forward-target"), target_id)).await;
    app.click(&tid("forward-send")).await;
    app.gone(&tid("forward-dialog")).await;
    eventually("the forward to land in the target", || async {
        let now = db(&app, &format!("SELECT json(COUNT(*)) FROM message WHERE conversation_id = {}", quote(&target_id))).await.as_i64().unwrap();
        (now > before).then(|| json!(now))
    })
    .await;
}

#[tokio::test]
async fn the_forward_button_on_a_message_opens_the_dialog() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Eva Ortega").await;
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    app.click(&tid("message-forward")).await;
    app.see(&tid("forward-dialog")).await;
    app.press(None, "Escape", &[]).await;
    app.gone(&tid("forward-dialog")).await;
}

#[tokio::test]
async fn an_own_message_is_edited_and_deleted() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Eva Ortega").await;
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    let mine = message_ids(&app, true).await;
    let id = mine.last().cloned().expect("a message of mine");
    menu_for(&app, &id).await;
    app.click(&tid("menu-edit")).await;
    app.fill(&tid("message-edit-field"), "Edited by the E2E suite").await;
    app.click(&tid("message-edit-save")).await;
    eventually("the edit to be stored", || async { Some(message(&app, &id).await).filter(|m| m["content"] == "Edited by the E2E suite") }).await;
    app.gone(&tid("message-edit-field")).await;
    app.wait("the edited text", &format!("document.querySelector({}).textContent.includes('Edited by the E2E suite')", js_string(&line(&id)))).await;

    menu_for(&app, &id).await;
    app.click(&tid("menu-delete")).await;
    app.click(&tid("message-delete-confirm")).await;
    eventually("the delete to be stored", || async { Some(message(&app, &id).await).filter(|m| !m["deleted"].is_null()) }).await;
    app.wait("the line to show it was deleted", &format!("document.querySelector({}) || !document.querySelector({})", js_string(&format!("{}[data-id=\"{}\"]", tid("message-deleted"), id)), js_string(&line(&id)))).await;
}

#[tokio::test]
async fn the_group_more_menu_offers_no_dead_edit_or_delete() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Eva Ortega").await;
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    let mine = message_ids(&app, true).await;
    let id = mine.last().cloned().expect("a message of mine");
    app.js(&format!("document.querySelector({}).closest('article').querySelector({}).click()", js_string(&line(&id)), js_string(&tid("message-more")))).await;
    app.see(&tid("message-menu")).await;
    assert!(app.exists(&tid("menu-pin")).await);
    assert!(!app.exists(&tid("menu-delete")).await);
    if app.exists(&tid("menu-edit")).await {
        app.click(&tid("menu-edit")).await;
        app.see(&tid("message-edit-form")).await;
    }
}

#[tokio::test]
async fn a_file_chip_opens_quick_look() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Eva Ortega").await;
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    app.intercept(&["post_attachment_open"]).await;
    app.see(&tid("file-chip")).await;
    let attachment = app.attr(&tid("file-chip"), "data-id").await.unwrap();
    let owner = app.js(&format!("return document.querySelector({}).closest('[data-message-id]').dataset.messageId", js_string(&tid("file-chip")))).await;
    app.click(&tid("file-chip")).await;
    let started = Instant::now();
    while app.intercepted("post_attachment_open").await.is_empty() {
        assert!(started.elapsed() < Duration::from_secs(10), "Quick Look was never asked for");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let call = app.intercepted("post_attachment_open").await[0].clone();
    assert_eq!(call["input"]["attachment_id"].as_str(), Some(attachment.as_str()));
    assert_eq!(call["input"]["message_id"], owner);
    assert_eq!(call["input"]["quick_look"], true);
    assert!(!app.exists(&tid("attachment-error")).await);
}

async fn grid_settings(app: &App, id: &str) -> Value {
    db(app, &format!("SELECT json_extract(view_settings, '$.grid') FROM conversation WHERE id = {}", quote(id))).await
}

async fn until_grid<F: Fn(&Value) -> bool>(app: &App, id: &str, what: &str, test: F) -> Value {
    let started = Instant::now();
    loop {
        let grid = grid_settings(app, id).await;
        if test(&grid) {
            return grid;
        }
        assert!(started.elapsed() < Duration::from_secs(20), "timed out waiting for {}: {}", what, grid);
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test]
async fn the_grid_view_show_group_sort_layout_and_tags_controls_persist() {
    let app = App::fresh().await;
    let chat = local(&app, "Receipts").await;
    let id = chat["id"].as_str().unwrap().to_string();
    open(&app, &id).await;
    app.see(&tid("chat-grid")).await;
    app.see(&tid("grid-tile")).await;

    app.click(&tid("grid-show")).await;
    app.see(&format!("{}[data-menu=\"show\"]", tid("grid-menu"))).await;
    app.click(&tid("grid-show-all")).await;
    until_grid(&app, &id, "show everything", |g| g["show"].as_array().is_some_and(|s| s.is_empty())).await;
    app.click(&tid("grid-show-files")).await;
    until_grid(&app, &id, "show files only", |g| g["show"] == json!(["files"])).await;
    app.see(&format!("{}[data-show=\"files\"]", tid("chat-grid"))).await;
    app.click(&tid("grid-show-all")).await;
    until_grid(&app, &id, "show everything again", |g| g["show"].as_array().is_some_and(|s| s.is_empty())).await;
    app.press(None, "Escape", &[]).await;

    for group in ["kind", "sender", "none", "source"] {
        app.click(&tid("grid-group")).await;
        app.click(&tid(&format!("grid-group-{}", group))).await;
        until_grid(&app, &id, &format!("group by {}", group), |g| g["group"] == group).await;
        app.see(&format!("{}[data-group=\"{}\"]", tid("chat-grid"), group)).await;
    }

    app.click(&tid("grid-sort")).await;
    app.click(&tid("grid-sort-name")).await;
    until_grid(&app, &id, "sort by name", |g| g["sort"] == "name").await;
    app.press(None, "Escape", &[]).await;
    let direction = app.attr(&tid("grid-direction"), "data-value").await.unwrap();
    app.click(&tid("grid-direction")).await;
    until_grid(&app, &id, "the other direction", |g| g["direction"].as_str() != Some(direction.as_str())).await;

    app.click(&tid("grid-rows-scroll")).await;
    until_grid(&app, &id, "scrolling rows", |g| g["rows"] == "scroll").await;
    app.click(&tid("grid-rows-wrap")).await;
    until_grid(&app, &id, "wrapping rows", |g| g["rows"] == "wrap").await;
    let tags = grid_settings(&app, &id).await["tags"].as_bool().unwrap_or(true);
    app.click(&tid("grid-tags")).await;
    until_grid(&app, &id, "tags toggled", |g| g["tags"].as_bool() == Some(!tags)).await;

    app.click(&tid("grid-more")).await;
    app.see(&format!("{}[data-menu=\"more\"]", tid("grid-menu"))).await;
    app.click(&tid("grid-group-kind")).await;
    until_grid(&app, &id, "group by kind from the more menu", |g| g["group"] == "kind").await;
}

#[tokio::test]
async fn grid_search_filters_tiles_and_selection_quick_looks() {
    let app = App::fresh().await;
    let chat = local(&app, "Receipts").await;
    open(&app, chat["id"].as_str().unwrap()).await;
    app.see(&tid("grid-tile")).await;
    let all = app.attr(&tid("grid-count"), "data-count").await.unwrap().parse::<usize>().unwrap();
    app.fill(&tid("grid-search"), "zzzz-nothing-matches").await;
    app.wait("no tiles", &format!("document.querySelector('{}').dataset.count === '0'", tid("grid-count"))).await;
    app.fill(&tid("grid-search"), "").await;
    app.wait("every tile back", &format!("document.querySelector('{}').dataset.count === '{}'", tid("grid-count"), all)).await;

    app.intercept(&["post_attachment_open"]).await;
    let file = app.js(&format!("const tiles = [...document.querySelectorAll('{}')]; const file = tiles.find((t) => ['file', 'image', 'record'].includes(t.dataset.kind)); return file ? file.dataset.id : null", tid("grid-tile"))).await;
    let file = file.as_str().expect("a file tile").to_string();
    let tile = format!("{}[data-id=\"{}\"]", tid("grid-tile"), file);
    app.click(&within(&tile, &tid("grid-tile-select"))).await;
    app.see(&format!("{}[data-selected=\"true\"]", tile)).await;
    app.click(&tid("grid-quick-look")).await;
    let started = Instant::now();
    while app.intercepted("post_attachment_open").await.is_empty() {
        assert!(started.elapsed() < Duration::from_secs(10), "Quick Look was never asked for");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(app.intercepted("post_attachment_open").await[0]["input"]["quick_look"], true);
    app.click(&tid("grid-clear")).await;
    app.see(&format!("{}[data-selected=\"false\"]", tile)).await;
}



fn details_open(open: bool) -> String {
    let panel = format!("document.querySelector({})", js_string(&tid("details-panel")));
    if open {
        format!("!!{panel} && !{panel}.closest('[inert]')")
    } else {
        format!("!{panel} || !!{panel}.closest('[inert]')")
    }
}

#[tokio::test]
async fn command_a_selects_every_message_or_the_text_of_the_one_you_are_in() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Start of Week Sync").await;
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    app.press(Some("body"), "a", &["meta"]).await;
    app.see(&tid("composer-selection")).await;
    app.press(Some("body"), "Escape", &[]).await;
    let picked = app.js(&format!("const el = document.querySelector('{} [data-testid=\"message-text\"], {}'); const range = document.createRange(); range.setStart(el.firstChild || el, 0); range.collapse(true); getSelection().removeAllRanges(); getSelection().addRange(range); return el.closest('[data-testid=\"message\"], [data-message-id]').innerText.trim()", tid("message"), tid("message"))).await;
    app.press(Some("body"), "a", &["meta"]).await;
    let selected = app.js("return getSelection().toString().trim()").await;
    assert_eq!(selected, picked);
}

#[tokio::test]
async fn command_a_inside_a_thread_selects_the_thread() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Start of Week Sync").await;
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    app.js(&format!("document.querySelector('{}').click(); return true", tid("message-thread"))).await;
    app.see(&tid("thread-panel")).await;
    app.hover(&tid("thread-panel")).await;
    app.press(Some("body"), "a", &["meta"]).await;
    app.see(&tid("composer-selection")).await;
    app.wait("the thread messages to be selected", &format!("[...document.querySelectorAll('{} [data-message-id]')].every((el) => el.classList.contains('bg-accent-soft'))", tid("thread-panel"))).await;
}

#[tokio::test]
async fn selected_messages_are_checked_copied_and_exported_from_the_header() {
    let app = App::fresh().await;
    let chat = conversation(&app, "teams", "Start of Week Sync").await;
    open_with_messages(&app, chat["id"].as_str().unwrap()).await;
    let header_export = format!("{} {}", tid("chat-header"), tid("chat-export"));
    assert!(!app.exists(&header_export).await);
    app.press(Some("body"), "a", &["meta"]).await;
    app.wait("every message to be checked", &format!("[...document.querySelectorAll('{}')].length > 0 && [...document.querySelectorAll('{}')].every((el) => el.getAttribute('aria-pressed') === 'true')", tid("message-check"), tid("message-check"))).await;
    app.js("window.__copied = null; navigator.clipboard.write = async (items) => { window.__copied = await (await items[0].getType('text/plain')).text() }; return true").await;
    app.click(&tid("composer-selection-copy")).await;
    app.wait("the selection to be copied", "typeof window.__copied === 'string' && window.__copied.split('---').length > 1").await;
    app.intercept(&["post_export"]).await;
    app.click(&header_export).await;
    app.click(&tid("chat-export-markdown")).await;
    app.wait("the export to be written", "(window.__e2eCalls || []).some((c) => c.cmd === 'post_export')").await;
    let calls = app.intercepted("post_export").await;
    let content = calls[0]["content"].as_str().unwrap_or_default().to_string();
    assert!(content.starts_with("# "), "{content}");
    assert!(content.matches("\n---\n").count() >= 1, "{content}");
    app.press(Some("body"), "Escape", &[]).await;
    app.gone(&tid("composer-selection")).await;
    app.gone(&header_export).await;
}
