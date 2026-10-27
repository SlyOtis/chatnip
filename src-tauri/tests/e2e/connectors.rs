use super::composer::{enter, messages, open, query, quoted, reply_after, see_message, sent_message, server_messages, wait_value, write};
use super::support::{tid, App};
use serde_json::{json, Value};
use std::time::Duration;

const MAIL: [&str; 3] = ["outlook", "gmail", "imap"];

fn is_mail(kind: &str) -> bool {
    MAIL.contains(&kind)
}

fn refusal(kind: &str, state: &str) -> &'static str {
    let graph = kind == "teams" || kind == "outlook";
    let google = kind == "gmail" || kind == "google_chat";
    match state {
        "expired" if graph => "InvalidAuthenticationToken",
        "expired" if kind == "slack" => "token_expired",
        "expired" if google => "invalid authentication credentials",
        "expired" => "AUTHENTICATIONFAILED",
        "rate_limited" if graph => "429",
        "rate_limited" if kind == "slack" => "ratelimited",
        "rate_limited" if google => "429",
        "rate_limited" => "Too many simultaneous connections",
        "offline" if kind == "imap" => "Connection refused",
        "offline" => "client error (Connect)",
        "denied" if graph => "403",
        "denied" if kind == "slack" => "missing_scope",
        "denied" if google => "403",
        _ => "550 5.7.1",
    }
}

async fn human_conversation(app: &App, kind: &str) -> String {
    let provider = app.mock_provider(kind);
    let read: Vec<Value> = app.engine("get_read_conversation_ids", json!({})).await.unwrap().as_array().cloned().unwrap_or_default();
    let people = app.engine("get_people", json!({})).await.unwrap();
    let people: Vec<Value> = people.as_array().cloned().or_else(|| people["data"].as_array().cloned()).unwrap_or_default();
    let human: std::collections::HashSet<String> = people.iter().filter(|p| p["is_human"] == true).flat_map(|p| p["conversation_ids"].as_array().cloned().unwrap_or_default()).filter_map(|id| id.as_str().map(String::from)).collect();
    let wanted = if is_mail(kind) { "email" } else { "direct_message" };
    app.conversations()
        .await
        .into_iter()
        .filter(|c| c["type"] == wanted && c["deleted_at"].is_null() && !read.contains(&c["id"]))
        .filter(|c| c["providers"].as_array().map(|p| p.len() == 1 && p[0]["id"].as_str() == Some(provider.as_str())).unwrap_or(false))
        .find(|c| human.contains(c["id"].as_str().unwrap_or_default()))
        .and_then(|c| c["id"].as_str().map(String::from))
        .unwrap_or_else(|| panic!("no {} conversation with a person on the mock {}", wanted, kind))
}

async fn server_message(app: &App, provider: &str, external_id: &str) -> Value {
    server_messages(app, provider).await.into_iter().find(|m| m["external_id"].as_str() == Some(external_id)).unwrap_or(Value::Null)
}

async fn arrival_runs(app: &App, message_id: &str) -> Vec<Value> {
    let sql = format!("SELECT json_group_array(json_object('action', a.name, 'status', r.status)) FROM action_run r JOIN action a ON a.id = r.action_id WHERE r.trigger LIKE {}", quoted(&format!("%{}%", message_id)));
    query(app, &sql).await.as_array().cloned().unwrap_or_default()
}

async fn round_trip(kind: &str) {
    let app = App::fresh().await;
    app.put_setting("mail.send_delay_seconds", "0").await;
    let provider = app.mock_provider(kind);
    let id = human_conversation(&app, kind).await;
    open(&app, &id).await;
    let text = format!("Round trip through the mock {}", kind);
    write(&app, &text).await;
    enter(&app, &["meta"]).await;
    let mine = sent_message(&app, &id, &text).await;
    assert_eq!(mine["provider_id"].as_str(), Some(provider.as_str()));
    see_message(&app, mine["id"].as_str().unwrap()).await;
    let on_server = server_message(&app, &provider, mine["external_id"].as_str().unwrap()).await;
    assert_eq!(on_server["metadata"]["delivery"].as_str().map(|d| d == "delivered" || d == "read"), Some(true), "{}", on_server);
    let reply = reply_after(&app, &id, &mine).await;
    see_message(&app, reply["id"].as_str().unwrap()).await;
    let reply_id = reply["id"].as_str().unwrap().to_string();
    let runs = wait_value("an arrival action run for the reply", Duration::from_secs(30), || async {
        let runs = arrival_runs(&app, &reply_id).await;
        (!runs.is_empty()).then(|| Value::Array(runs))
    })
    .await;
    assert!(runs.as_array().unwrap().iter().all(|r| r["status"] != "running"), "{}", runs);
    assert!(runs.as_array().unwrap().iter().any(|r| r["status"] == "done"), "{}", runs);
    let read_back = wait_value("the server to mark the sent message read", Duration::from_secs(10), || async {
        let body = server_message(&app, &provider, mine["external_id"].as_str().unwrap()).await;
        (body["metadata"]["delivery"] == "read").then_some(body)
    })
    .await;
    assert_eq!(read_back["metadata"]["delivery"], "read");
    if is_mail(kind) {
        let mine_id = mine["id"].as_str().unwrap();
        assert!(app.engine("put_message", json!({ "input": { "message_id": mine_id, "content": "changed" } })).await.is_err(), "mail cannot be edited");
        assert!(app.engine("post_message_reaction", json!({ "input": { "conversation_id": id, "message_id": reply_id, "reaction": "👍" } })).await.is_err(), "mail has no reactions");
        return;
    }
    let mine_id = mine["id"].as_str().unwrap().to_string();
    let external = mine["external_id"].as_str().unwrap().to_string();
    app.engine("post_message_reaction", json!({ "input": { "conversation_id": id, "message_id": reply_id, "reaction": "👍" } })).await.unwrap();
    let reaction = format!("{}[data-id=\"{}\"] {}", tid("message"), reply_id, tid("reaction"));
    app.see(&reaction).await;
    let emoji = app.attr(&reaction, "data-emoji").await.unwrap_or_default();
    assert!(["👍", ":+1:", ":thumbsup:"].contains(&emoji.as_str()), "{}", emoji);
    let reacted = server_message(&app, &provider, reply["external_id"].as_str().unwrap()).await;
    assert!(reacted["metadata"]["reactions"].as_array().is_some_and(|r| r.iter().any(|x| ["👍", ":+1:", ":thumbsup:"].contains(&x["emoji"].as_str().unwrap_or_default()))), "{}", reacted);
    let edited = format!("{} (edited)", text);
    app.engine("put_message", json!({ "input": { "message_id": mine_id, "content": edited } })).await.unwrap();
    app.wait("the edit in the chat", &format!("(document.querySelector({}) || {{ textContent: '' }}).textContent.includes({})", serde_json::to_string(&format!("{}[data-id=\"{}\"]", tid("message"), mine_id)).unwrap(), serde_json::to_string(&edited).unwrap())).await;
    let body = server_message(&app, &provider, &external).await;
    assert_eq!(body["content"].as_str(), Some(edited.as_str()));
    assert!(body["metadata"]["edited_at"].is_string());
    app.engine("delete_message", json!({ "input": { "message_id": mine_id } })).await.unwrap();
    app.wait("the deleted message to leave the chat", &format!("!document.querySelector('{}[data-id=\"{}\"]') || document.querySelector('{}[data-id=\"{}\"]')", tid("message").replace('"', "\\\""), mine_id, tid("message-deleted").replace('"', "\\\""), mine_id)).await;
    let gone = server_message(&app, &provider, &external).await;
    assert!(gone["metadata"]["deleted_at"].is_string(), "{}", gone);
    assert_eq!(gone["content"].as_str(), Some(""));
    let local = messages(&app, &id).await.into_iter().find(|m| m["id"].as_str() == Some(mine_id.as_str())).unwrap_or(Value::Null);
    assert!(local.is_null() || !local["deleted"].is_null(), "{}", local);
}

async fn error_states(kind: &str) {
    let app = App::fresh().await;
    let provider = app.mock_provider(kind);
    app.goto("/main/profile/accounts#accounts.connections").await;
    let select = tid(&format!("mock-state-{}", provider));
    for state in ["expired", "rate_limited", "offline"] {
        app.select(&select, state).await;
        app.wait_until_state(&provider, state).await;
        let sync = tid(&format!("provider-sync-{}", provider));
        app.wait("the sync button to settle", &format!("document.querySelector({}) && !document.querySelector({}).disabled", serde_json::to_string(&sync).unwrap(), serde_json::to_string(&sync).unwrap())).await;
        app.click(&sync).await;
        let fragment = refusal(kind, state);
        app.wait(&format!("{} to surface {:?}", state, fragment), &format!("(document.querySelector({}) || {{ textContent: '' }}).textContent.includes({})", serde_json::to_string(&tid("connections-failure")).unwrap(), serde_json::to_string(fragment).unwrap())).await;
    }
    app.select(&select, "denied").await;
    app.wait_until_state(&provider, "denied").await;
    let id = human_conversation(&app, kind).await;
    app.put_setting("mail.send_delay_seconds", "0").await;
    open(&app, &id).await;
    write(&app, "Refused by the server").await;
    enter(&app, &["meta"]).await;
    app.see(&tid("composer-error")).await;
    assert!(app.text(&tid("composer-error")).await.contains(refusal(kind, "denied")), "{}", app.text(&tid("composer-error")).await);
    for state in ["expired", "rate_limited", "offline"] {
        app.engine("put_mock_state", json!({ "providerId": provider, "state": state })).await.unwrap();
        let error = app.engine("post_message", json!({ "input": { "conversation_id": id, "content": "engine send", "content_type": "markdown", "provider_id": provider } })).await.unwrap_err();
        assert!(error.as_str().unwrap_or_default().contains(refusal(kind, state)), "{} gave {}", state, error);
    }
    app.engine("put_mock_state", json!({ "providerId": provider, "state": "ok" })).await.unwrap();
    app.engine("post_provider_sync", json!({ "providerId": provider })).await.unwrap();
}

trait MockState {
    async fn wait_until_state(&self, provider: &str, state: &str);
}

impl MockState for App {
    async fn wait_until_state(&self, provider: &str, state: &str) {
        wait_value(&format!("{} to be {}", provider, state), Duration::from_secs(10), || async {
            let mode = self.engine("get_mock_mode", json!({})).await.unwrap();
            (mode["states"][provider] == state).then_some(mode)
        })
        .await;
    }
}

#[tokio::test]
async fn teams_round_trip_edit_delete_and_react() {
    round_trip("teams").await
}

#[tokio::test]
async fn slack_round_trip_edit_delete_and_react() {
    round_trip("slack").await
}

#[tokio::test]
async fn google_chat_round_trip_edit_delete_and_react() {
    round_trip("google_chat").await
}

#[tokio::test]
async fn outlook_mail_round_trip() {
    round_trip("outlook").await
}

#[tokio::test]
async fn gmail_mail_round_trip() {
    round_trip("gmail").await
}

#[tokio::test]
async fn imap_mail_round_trip() {
    round_trip("imap").await
}

#[tokio::test]
async fn teams_error_states_surface() {
    error_states("teams").await
}

#[tokio::test]
async fn slack_error_states_surface() {
    error_states("slack").await
}

#[tokio::test]
async fn google_chat_error_states_surface() {
    error_states("google_chat").await
}

#[tokio::test]
async fn outlook_error_states_surface() {
    error_states("outlook").await
}

#[tokio::test]
async fn gmail_error_states_surface() {
    error_states("gmail").await
}

#[tokio::test]
async fn imap_error_states_surface() {
    error_states("imap").await
}
