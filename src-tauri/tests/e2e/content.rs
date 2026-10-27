use super::support::{js_string, tid, App};
use serde_json::{json, Value};

fn q(name: &str) -> String {
    js_string(&tid(name))
}

fn sel(name: &str, attr: &str, value: &str) -> String {
    format!("{}[data-{}=\"{}\"]", tid(name), attr, value)
}

async fn library(app: &App) -> Vec<Value> {
    app.engine("get_content", json!({})).await.unwrap().as_array().cloned().unwrap_or_default()
}

async fn item(app: &App, name: &str) -> Value {
    library(app).await.into_iter().find(|i| i["name"] == name).unwrap_or_else(|| panic!("no seeded content called {}", name))
}

async fn groups(app: &App) -> Vec<Value> {
    app.engine("get_content_groups", json!({})).await.unwrap().as_array().cloned().unwrap_or_default()
}

async fn open_content(app: &App) {
    app.goto("/main/content").await;
    app.see(&tid("content-section")).await;
}

async fn open_item(app: &App, id: &str) {
    app.click(&sel("content-tile", "id", id)).await;
    app.see(&sel("content-detail", "id", id)).await;
}

#[tokio::test]
async fn every_group_shows_as_a_section_and_collapses_and_expands() {
    let app = App::fresh().await;
    open_content(&app).await;
    let seeded = groups(&app).await;
    assert!(!seeded.is_empty());
    for group in seeded.iter().filter(|g| g["kind"] == "manual") {
        app.see(&sel("content-section", "group", group["id"].as_str().unwrap())).await;
    }
    assert_eq!(app.text(&tid("content-count")).await, library(&app).await.iter().filter(|i| i["kind"] != "emoji").count().to_string());
    let reactions = seeded.iter().find(|g| g["name"] == "Celebrations").unwrap()["id"].as_str().unwrap().to_string();
    let section = sel("content-section", "group", &reactions);
    app.wait("the section has tiles", &format!("document.querySelector({}).querySelectorAll({}).length > 0", js_string(&section), q("content-tile"))).await;
    app.click(&format!("{} {}", section, tid("content-section-toggle"))).await;
    app.wait("the section collapses", &format!("document.querySelector({}).querySelectorAll({}).length === 0", js_string(&section), q("content-tile"))).await;
    let collapsed = groups(&app).await.into_iter().find(|g| g["id"] == reactions.as_str()).unwrap();
    assert_eq!(collapsed["is_collapsed"], true);
    app.click(&format!("{} {}", section, tid("content-section-toggle"))).await;
    app.wait("the section expands", &format!("document.querySelector({}).querySelectorAll({}).length > 0", js_string(&section), q("content-tile"))).await;
    assert_eq!(groups(&app).await.into_iter().find(|g| g["id"] == reactions.as_str()).unwrap()["is_collapsed"], false);
}

#[tokio::test]
async fn a_new_group_is_created_filled_from_the_detail_and_deleted() {
    let app = App::fresh().await;
    open_content(&app).await;
    app.click(&tid("content-new-group")).await;
    app.see(&tid("group-editor")).await;
    app.fill(&tid("group-editor-name"), "E2E Shelf").await;
    app.click(&tid("group-editor-save")).await;
    app.gone(&tid("group-editor")).await;
    let shelf = groups(&app).await.into_iter().find(|g| g["name"] == "E2E Shelf").expect("the group was not stored");
    let id = shelf["id"].as_str().unwrap().to_string();
    let section = sel("content-section", "group", &id);
    app.see(&format!("{}[data-count=\"0\"]", section)).await;

    let fjord = item(&app, "Fjord").await;
    open_item(&app, fjord["id"].as_str().unwrap()).await;
    app.click(&sel("content-group-toggle", "id", &id)).await;
    app.see(&format!("{}[data-count=\"1\"]", section)).await;
    assert!(item(&app, "Fjord").await["group_ids"].as_array().unwrap().iter().any(|g| g == id.as_str()));

    app.click(&format!("{} {}", section, tid("content-section-more"))).await;
    app.see(&tid("group-editor")).await;
    app.click(&tid("group-editor-delete")).await;
    app.click(&tid("group-editor-delete-confirm")).await;
    app.gone(&section).await;
    assert!(!groups(&app).await.iter().any(|g| g["id"] == id.as_str()));
}

#[tokio::test]
async fn a_template_lists_its_variables_and_previews_with_sample_values() {
    let app = App::fresh().await;
    open_content(&app).await;
    let late = item(&app, "Running late").await;
    let id = late["id"].as_str().unwrap().to_string();
    open_item(&app, &id).await;
    app.see(&sel("content-variable", "name", "person.first_name")).await;
    app.wait("the preview renders the template", &format!("document.querySelector({}).textContent.includes('Running about ten minutes late')", q("content-preview"))).await;
    app.fill(&sel("content-sample", "name", "person.first_name"), "Zelda").await;
    app.wait("the preview uses the sample", &format!("document.querySelector({}).textContent.includes('Zelda')", q("content-preview"))).await;

    app.fill(&tid("content-body"), "Hello {{ .person.first_name }}, see {{ .conversation.name }}").await;
    app.js(&format!("document.querySelector({}).blur()", q("content-body"))).await;
    app.wait("the preview follows the new body", &format!("document.querySelector({}).textContent.includes('Hello Zelda')", q("content-preview"))).await;
    app.see(&sel("content-variable", "name", "conversation.name")).await;
    let mut stored = Value::Null;
    for _ in 0..50 {
        stored = item(&app, "Running late").await;
        if stored["body"].as_str().unwrap_or_default().starts_with("Hello") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(stored["body"], "Hello {{ .person.first_name }}, see {{ .conversation.name }}");

    app.fill(&tid("content-body"), "Broken {{ nosuchfunction .person }}").await;
    app.see(&tid("content-render-error")).await;
}

#[tokio::test]
async fn a_new_snippet_is_created_from_the_upload_menu_and_changes_kind() {
    let app = App::fresh().await;
    open_content(&app).await;
    let before = library(&app).await.len();
    app.click(&tid("content-upload-menu")).await;
    app.see(&tid("content-upload-list")).await;
    app.click(&tid("content-create-snippet")).await;
    app.see(&tid("content-detail")).await;
    let id = app.attr(&tid("content-detail"), "data-id").await.unwrap();
    let created = library(&app).await;
    assert_eq!(created.len(), before + 1);
    assert_eq!(created.iter().find(|i| i["id"] == id.as_str()).unwrap()["kind"], "snippet");
    app.fill(&tid("content-name"), "E2E snippet").await;
    app.press(Some(&tid("content-name")), "Enter", &[]).await;
    app.wait("the title is saved", &format!("document.querySelector({}).textContent.trim() === 'E2E snippet'", q("content-detail-name"))).await;
    app.click(&format!("{} div:not([inert]) > [data-value=\"email\"]", tid("content-kind"))).await;
    app.see(&sel("content-tile", "kind", "email")).await;
    let mut kind = Value::Null;
    for _ in 0..50 {
        kind = library(&app).await.into_iter().find(|i| i["id"] == id.as_str()).unwrap()["kind"].clone();
        if kind == "email" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(kind, "email");
    assert_eq!(library(&app).await.into_iter().find(|i| i["id"] == id.as_str()).unwrap()["name"], "E2E snippet");

    app.confirm_next(true).await;
    app.click(&tid("content-delete")).await;
    app.gone(&sel("content-tile", "id", &id)).await;
    assert_eq!(library(&app).await.len(), before);
}

#[tokio::test]
async fn tags_and_keywords_are_added_and_removed_in_the_detail() {
    let app = App::fresh().await;
    open_content(&app).await;
    let rocket = item(&app, "Rocket").await;
    let id = rocket["id"].as_str().unwrap().to_string();
    open_item(&app, &id).await;
    app.fill(&tid("content-tag-input"), "launches").await;
    app.press(Some(&tid("content-tag-input")), "Enter", &[]).await;
    app.see(&sel("content-tag", "tag", "launches")).await;
    app.fill(&tid("content-keyword-input"), "liftoff").await;
    app.press(Some(&tid("content-keyword-input")), "Enter", &[]).await;
    app.see(&sel("content-keyword", "word", "liftoff")).await;
    let stored = item(&app, "Rocket").await;
    assert!(stored["tags"].as_array().unwrap().iter().any(|t| t == "launches"));
    assert!(stored["keywords"].as_array().unwrap().iter().any(|k| k == "liftoff"));
    app.click(&sel("content-tag", "tag", "launches")).await;
    app.gone(&sel("content-tag", "tag", "launches")).await;
    app.click(&sel("content-keyword", "word", "liftoff")).await;
    app.gone(&sel("content-keyword", "word", "liftoff")).await;
    let cleared = item(&app, "Rocket").await;
    assert!(!cleared["tags"].as_array().unwrap().iter().any(|t| t == "launches"));
    assert!(!cleared["keywords"].as_array().unwrap().iter().any(|k| k == "liftoff"));
}

#[tokio::test]
async fn the_provenance_link_opens_the_source_conversation_person_action_and_url() {
    let app = App::fresh().await;
    for (name, path) in [("Invoice received", "/main/conversation/"), ("Thanks, on it", "/main/person/"), ("Action items", "/main/actions")] {
        open_content(&app).await;
        let entry = item(&app, name).await;
        open_item(&app, entry["id"].as_str().unwrap()).await;
        app.click(&tid("content-source-link")).await;
        app.wait_url(path).await;
    }
    open_content(&app).await;
    app.intercept(&["plugin:opener|open_url"]).await;
    let link = item(&app, "Meeting link").await;
    open_item(&app, link["id"].as_str().unwrap()).await;
    app.click(&tid("content-source-link")).await;
    wait_opened(&app).await;
}

async fn wait_opened(app: &App) {
    for _ in 0..100 {
        let calls = app.intercepted("plugin:opener|open_url").await;
        if let Some(call) = calls.first() {
            assert!(call["url"].as_str().unwrap_or_default().starts_with("http"), "opened {:?}", call);
            assert!(app.url().await.contains("/main/content"), "an external link navigated the app");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("the external source was never handed to the opener");
}

#[tokio::test]
async fn upload_picks_files_without_a_native_dialog_and_stores_them() {
    let app = App::fresh().await;
    open_content(&app).await;
    let before = library(&app).await.len();
    app.js("window.__e2ePicks = 0; HTMLInputElement.prototype.click = function () { if (this.type === 'file') { window.__e2ePicks += 1; return } return HTMLElement.prototype.click.call(this) }").await;
    app.click(&tid("content-upload-main")).await;
    app.wait("the upload button asks for files", "window.__e2ePicks === 1").await;
    app.js(&format!("const input = document.querySelector({}); const data = new DataTransfer(); data.items.add(new File(['hello from the e2e suite'], 'e2e-notes.txt', {{ type: 'text/plain' }})); input.files = data.files; input.dispatchEvent(new Event('change', {{ bubbles: true }}))", q("content-file"))).await;
    app.wait("the upload is stored", &format!("[...document.querySelectorAll({})].some((el) => el.getAttribute('aria-label') === 'e2e-notes.txt')", q("content-tile"))).await;
    let stored = library(&app).await;
    assert_eq!(stored.len(), before + 1);
    let notes = stored.iter().find(|i| i["name"] == "e2e-notes.txt").unwrap();
    assert_eq!(notes["kind"], "file");
    assert_eq!(notes["source_kind"], "upload");

    app.click(&tid("content-upload-menu")).await;
    app.click(&tid("content-import-url")).await;
    app.see(&tid("content-import")).await;
    app.fill(&tid("content-import-field"), "not a url").await;
    app.wait("the import refuses a non url", &format!("document.querySelector({}).disabled", q("content-import-submit"))).await;
    app.fill(&tid("content-import-field"), "https://example.com/cat.gif").await;
    app.wait("the import accepts a url", &format!("!document.querySelector({}).disabled", q("content-import-submit"))).await;
    app.press(Some(&tid("content-import-field")), "Escape", &[]).await;
}

#[tokio::test]
async fn search_ranks_matches_and_bulk_actions_favourite_and_delete() {
    let app = App::fresh().await;
    open_content(&app).await;
    app.fill(&tid("content-search"), "parrot").await;
    app.see(&tid("content-matches")).await;
    let parrot = item(&app, "Party parrot").await;
    app.see(&sel("content-tile", "id", parrot["id"].as_str().unwrap())).await;
    app.see(&tid("content-web")).await;
    app.fill(&tid("content-search"), "").await;
    app.gone(&tid("content-matches")).await;

    let ship = item(&app, "Ship it").await["id"].as_str().unwrap().to_string();
    let face = item(&app, "Facepalm").await["id"].as_str().unwrap().to_string();
    for id in [&ship, &face] {
        app.click(&format!("{} {}", sel("content-tile", "id", id), tid("content-tile-select"))).await;
    }
    app.see(&tid("content-bulk")).await;
    app.click(&tid("content-bulk-favourite")).await;
    app.wait("both are favourites", &format!("document.querySelector({}).getAttribute('aria-pressed') === 'true'", js_string(&sel("content-tile", "id", &ship)))).await;
    for _ in 0..50 {
        if item(&app, "Ship it").await["is_favourite"] == true && item(&app, "Facepalm").await["is_favourite"] == true {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(item(&app, "Ship it").await["is_favourite"], true);
    assert_eq!(item(&app, "Facepalm").await["is_favourite"], true);
    app.click(&tid("content-bulk-clear")).await;
    app.gone(&tid("content-bulk")).await;

    app.click(&tid("content-favourites")).await;
    let favourites = library(&app).await.iter().filter(|i| i["is_favourite"] == true).count();
    app.wait("the favourite count matches", &format!("new Set([...document.querySelectorAll({})].map((el) => el.dataset.id)).size === {}", q("content-tile"), favourites)).await;
    app.click(&tid("content-favourites")).await;

    let before = library(&app).await.len();
    for id in [&ship, &face] {
        app.click(&format!("{} {}", sel("content-tile", "id", id), tid("content-tile-select"))).await;
    }
    app.confirm_next(true).await;
    app.click(&tid("content-bulk-delete")).await;
    app.gone(&sel("content-tile", "id", &ship)).await;
    app.gone(&sel("content-tile", "id", &face)).await;
    assert_eq!(library(&app).await.len(), before - 2);
}

#[tokio::test]
async fn a_template_is_inserted_into_the_composer_from_the_content_picker() {
    let app = App::fresh().await;
    let chat = app.conversations().await.into_iter().find(|c| c["type"] == "direct_message" && c["is_archived"] != true && c["deleted_at"].is_null()).expect("no direct chat seeded");
    let id = chat["id"].as_str().unwrap().to_string();
    let used = item(&app, "Running late").await["used_count"].as_i64().unwrap_or(0);
    app.goto(&format!("/main/conversation/{}", id)).await;
    app.see(&tid("composer-input")).await;
    app.click(&tid("composer-more")).await;
    app.click(&tid("composer-more-content")).await;
    app.see(&tid("content-picker")).await;
    app.fill(&tid("content-picker-search"), "Running late").await;
    let late = item(&app, "Running late").await["id"].as_str().unwrap().to_string();
    app.click(&sel("content-picker-item", "id", &late)).await;
    app.gone(&tid("content-picker")).await;
    app.wait("the composer holds the rendered template", &format!("document.querySelector({}).innerText.includes('Running about ten minutes late')", q("composer-input"))).await;
    assert!(app.value(&tid("composer-input")).await.contains("Running about ten minutes late"));
    assert!(!app.value(&tid("composer-input")).await.contains("{{"), "the template was inserted unrendered");
    assert_eq!(item(&app, "Running late").await["used_count"].as_i64().unwrap_or(0), used + 1);

    app.click(&tid("composer-more")).await;
    app.click(&tid("composer-more-content")).await;
    app.click(&tid("content-picker-manage")).await;
    app.wait_url("/main/content").await;
}
