use super::support::{js_string, tid, App};
use serde_json::{json, Value};

async fn person(app: &App, name: &str, human: bool) -> Value {
    let people = app.engine("get_people", json!({})).await.unwrap();
    people.as_array().unwrap().iter().filter(|p| p["display_name"] == name && p["is_human"] == human).max_by_key(|p| p["identities"].as_array().map(|i| i.len()).unwrap_or(0)).cloned().unwrap_or_else(|| panic!("no seeded person called {}", name))
}

async fn profile(app: &App, id: &str) -> Value {
    app.engine("get_person_profile", json!({ "personId": id })).await.unwrap()
}

async fn overrides(app: &App, id: &str) -> Vec<Value> {
    app.engine("get_object_overrides", json!({})).await.unwrap().as_array().unwrap().iter().filter(|o| o["object_type"] == "person" && o["object_id"] == id).cloned().collect()
}

async fn open_profile(app: &App, id: &str) {
    app.goto(&format!("/main/person/{}/profile", id)).await;
    app.see(&tid("person-heading")).await;
}

fn q(name: &str) -> String {
    js_string(&tid(name))
}

async fn press_enabled(app: &App, selector: &str) {
    app.wait(&format!("{} to be enabled", selector), &format!("document.querySelector({}) && !document.querySelector({}).disabled", js_string(selector), js_string(selector))).await;
    app.click(selector).await;
}

fn open_identity(key: &str) -> String {
    format!("{} {}", sel("person-identity-card", "key", key), tid("person-open-identity"))
}

fn sel(name: &str, attr: &str, value: &str) -> String {
    format!("{}[data-{}=\"{}\"]", tid(name), attr, value)
}

#[tokio::test]
async fn the_profile_lists_every_thread_and_a_thread_opens_its_chat() {
    let app = App::fresh().await;
    let kari = person(&app, "Kari Nordmann", true).await;
    let id = kari["id"].as_str().unwrap();
    open_profile(&app, id).await;
    assert_eq!(app.text(&tid("person-heading")).await, "Kari Nordmann");
    let threads = profile(&app, id).await["threads"].as_array().unwrap().clone();
    assert!(!threads.is_empty(), "the seeded person has no threads");
    app.wait("every thread in the sidebar list", &format!("document.querySelectorAll({}).length === {}", q("person-thread"), threads.len())).await;
    let first = threads[0]["id"].as_str().unwrap();
    app.click(&sel("person-thread", "id", first)).await;
    app.wait_url(&format!("/main/conversation/{}", first)).await;
}

#[tokio::test]
async fn the_person_thread_shows_messages_from_every_channel() {
    let app = App::fresh().await;
    let kari = person(&app, "Kari Nordmann", true).await;
    let id = kari["id"].as_str().unwrap();
    app.goto(&format!("/main/person/{}", id)).await;
    app.see(&sel("person-thread-view", "person", id)).await;
    app.wait("person messages loaded", &format!("Number(document.querySelector({}).dataset.count) > 0", q("person-thread-view"))).await;
    assert!(app.count(&tid("message-origin")).await >= 1);
    app.goto(&format!("/main/person/{}/profile", id)).await;
    app.click(&tid("person-open-chat")).await;
    app.wait("the chat of the person opens", "!location.pathname.endsWith('/profile')").await;
}

#[tokio::test]
async fn a_local_job_title_and_company_are_kept_and_reset() {
    let app = App::fresh().await;
    let mia = person(&app, "Mia Sund", true).await;
    let id = mia["id"].as_str().unwrap().to_string();
    open_profile(&app, &id).await;
    for (field, value) in [("job_title", "Chief Tester"), ("company", "E2E Industries"), ("notes", "met at the summit")] {
        let input = tid(&format!("person-{}", field));
        app.fill(&input, value).await;
        app.press(Some(&input), "Enter", &[]).await;
        app.see(&sel("person-local", "field", field)).await;
        let kept = overrides(&app, &id).await;
        assert!(kept.iter().any(|o| o["field"] == field && o["value"] == value), "{} was not stored: {:?}", field, kept);
    }
    app.wait("the subtitle shows the local job title", &format!("document.querySelector({}).parentElement.textContent.includes('Chief Tester')", q("person-heading"))).await;
    for field in ["job_title", "company", "notes"] {
        press_enabled(&app, &tid(&format!("person-reset-{}", field))).await;
        app.gone(&sel("person-local", "field", field)).await;
    }
    assert!(overrides(&app, &id).await.is_empty(), "the reset left overrides behind");
    assert_eq!(app.value(&tid("person-job_title")).await, "");
}

#[tokio::test]
async fn a_local_name_renames_the_person_and_the_reset_restores_the_synced_name() {
    let app = App::fresh().await;
    let jonas = person(&app, "Jonas Lie", true).await;
    let id = jonas["id"].as_str().unwrap().to_string();
    open_profile(&app, &id).await;
    app.fill(&tid("person-name"), "Jonas the Tester").await;
    app.js(&format!("document.querySelector({}).blur()", q("person-name"))).await;
    app.wait("the heading shows the local name", &format!("document.querySelector({}).textContent.trim() === 'Jonas the Tester'", q("person-heading"))).await;
    let renamed = app.engine("get_person", json!({ "personId": id })).await.unwrap();
    assert_eq!(renamed["display_name"], "Jonas the Tester");
    assert_eq!(renamed["provider_name"], "Jonas Lie");
    press_enabled(&app, &tid("person-reset-display_name")).await;
    app.wait("the heading shows the synced name", &format!("document.querySelector({}).textContent.trim() === 'Jonas Lie'", q("person-heading"))).await;
    let restored = app.engine("get_person", json!({ "personId": id })).await.unwrap();
    assert_eq!(restored["display_name"], "Jonas Lie");
}

#[tokio::test]
async fn an_identity_opens_alone_and_an_added_address_can_be_split_out() {
    let app = App::fresh().await;
    let anna = person(&app, "Anna Berg", true).await;
    let id = anna["id"].as_str().unwrap().to_string();
    open_profile(&app, &id).await;
    let identities = profile(&app, &id).await["identities"].as_array().unwrap().clone();
    assert_eq!(app.count(&tid("person-identity-card")).await, identities.len());
    let key = identities[0]["key"].as_str().unwrap();
    app.click(&open_identity(key)).await;
    app.wait_url("identity=").await;
    app.see(&format!("{}[data-full=\"true\"]", tid("person-identity-card"))).await;
    assert_eq!(app.count(&tid("person-identity-card")).await, 1);
    app.click(&tid("person-identities-all")).await;
    app.wait("the overview is back", "!location.search.includes('identity=')").await;

    app.invoke("post_person_identity", json!({ "input": { "person_id": id, "provider_type": "email", "email": "anna.e2e@example.com", "external_id": null, "display_name": null } })).await.unwrap();
    open_profile(&app, &id).await;
    app.wait("the new address is listed", &format!("document.querySelectorAll({}).length === {}", q("person-identity-card"), identities.len() + 1)).await;
    let added = profile(&app, &id).await["identities"].as_array().unwrap().iter().find(|i| i["identity"]["email"] == "anna.e2e@example.com").cloned().expect("the address was not stored");
    app.click(&open_identity(added["key"].as_str().unwrap())).await;
    press_enabled(&app, &tid("person-split")).await;
    app.wait("the address is split out", &format!("document.querySelectorAll({}).length === {}", q("person-identity-card"), identities.len())).await;
    let after = profile(&app, &id).await["identities"].as_array().unwrap().len();
    assert_eq!(after, identities.len());
}
