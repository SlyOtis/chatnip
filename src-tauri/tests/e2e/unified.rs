use super::composer::{conversation_of, messages, query, quoted, reply_after, sent_message, wait_value};
use super::support::{tid, App};
use serde_json::{json, Value};
use std::time::Duration;

async fn send(app: &App, conversation_id: &str, provider_id: &str, text: &str, metadata: Value) -> Value {
    app.engine("post_message", json!({ "input": { "conversation_id": conversation_id, "content": text, "content_type": "markdown", "provider_id": provider_id, "metadata": metadata } })).await.unwrap_or_else(|e| panic!("send failed: {}", e));
    sent_message(app, conversation_id, text).await
}

#[tokio::test]
async fn a_slack_reply_arrives_through_the_live_connection_and_typing_shows_before_it() {
    let app = App::fresh().await;
    let conversation = conversation_of(&app, "slack", &["direct_message"]).await;
    let id = conversation["id"].as_str().unwrap().to_string();
    let provider = app.mock_provider("slack");
    app.goto(&format!("/main/conversation/{}", id)).await;
    app.see(&tid("composer-input")).await;
    let mine = send(&app, &id, &provider, "Live from the e2e test", json!({})).await;
    app.wait_for("the typing indicator", &format!("!!document.querySelector({})", serde_json::to_string(&tid("typing-indicator")).unwrap()), Duration::from_secs(30)).await;
    let reply = reply_after(&app, &id, &mine).await;
    assert!(reply["content"].as_str().is_some_and(|c| !c.is_empty()));
    let synced: Option<String> = app.setting("sync.interval_seconds").await;
    assert!(synced.as_deref().unwrap_or("120").parse::<u64>().unwrap() >= 60, "the reply came in without a short poll");
}

#[tokio::test]
async fn a_draft_is_kept_per_conversation_and_thread_and_dropped_after_send() {
    let app = App::fresh().await;
    let conversation = conversation_of(&app, "teams", &["direct_message", "group_message"]).await;
    let id = conversation["id"].as_str().unwrap().to_string();
    app.engine("put_draft", json!({ "input": { "conversation_id": id, "markdown": "Half a thought", "doc": null } })).await.unwrap();
    app.engine("put_draft", json!({ "input": { "conversation_id": id, "thread_id": "t1", "markdown": "In the thread", "doc": null } })).await.unwrap();
    let drafts = app.engine("get_drafts", json!({ "conversationId": id })).await.unwrap();
    let texts: Vec<&str> = drafts.as_array().unwrap().iter().filter_map(|d| d["markdown"].as_str()).collect();
    assert!(texts.contains(&"Half a thought") && texts.contains(&"In the thread"), "{:?}", texts);
    app.engine("put_draft", json!({ "input": { "conversation_id": id, "markdown": "A whole thought", "doc": null } })).await.unwrap();
    app.engine("delete_draft", json!({ "conversationId": id, "threadId": "t1" })).await.unwrap();
    let after = app.engine("get_drafts", json!({ "conversationId": id })).await.unwrap();
    assert_eq!(after.as_array().unwrap().iter().filter_map(|d| d["markdown"].as_str()).collect::<Vec<_>>(), vec!["A whole thought"]);
}

#[tokio::test]
async fn a_mail_draft_is_mirrored_to_the_mail_server() {
    let app = App::fresh().await;
    let conversation = conversation_of(&app, "gmail", &["email"]).await;
    let id = conversation["id"].as_str().unwrap().to_string();
    app.engine("put_draft", json!({ "input": { "conversation_id": id, "markdown": "Draft for Gmail", "doc": null, "metadata": { "subject": "Re: plan", "to": ["anna@client.example"] } } })).await.unwrap();
    let draft = wait_value("the draft to get a server id", Duration::from_secs(15), || async {
        let drafts = app.engine("get_drafts", json!({ "conversationId": id })).await.ok()?;
        drafts.as_array()?.iter().find(|d| d["remote_id"].is_string()).cloned()
    })
    .await;
    let remote = draft["remote_id"].as_str().unwrap();
    let kept = query(&app, &format!("SELECT json_object('value', value) FROM mock_server_setting WHERE key = {}", quoted(&format!("draft:{}", remote)))).await;
    assert!(kept["value"].as_str().unwrap_or_default().contains("Draft for Gmail"), "{}", kept);
}

#[tokio::test]
async fn joining_a_slack_channel_brings_it_in_and_its_topic_writes_back() {
    let app = App::fresh().await;
    let provider = app.mock_provider("slack");
    let joinable = app.engine("get_joinable_conversations", json!({ "providerId": provider })).await.unwrap();
    let design = joinable.as_array().unwrap().iter().find(|j| j["conversation"]["name"] == "design").cloned().expect("design is joinable");
    let external = design["conversation"]["external_id"].as_str().unwrap().to_string();
    assert!(app.engine("post_conversation_membership", json!({ "input": { "provider_id": provider, "external_id": external } })).await.unwrap().as_bool().unwrap());
    let joined = wait_value("the design channel to sync in", Duration::from_secs(30), || async {
        app.conversations().await.into_iter().find(|c| c["name"] == "design" && c["providers"].as_array().is_some_and(|p| p.iter().any(|x| x["id"].as_str() == Some(provider.as_str()))))
    })
    .await;
    let id = joined["id"].as_str().unwrap().to_string();
    let left = app.engine("get_joinable_conversations", json!({ "providerId": provider })).await.unwrap();
    assert!(!left.as_array().unwrap().iter().any(|j| j["conversation"]["external_id"] == external.as_str()));
    assert!(app.engine("put_conversation_topic", json!({ "input": { "conversation_id": id, "topic": "Ship the inbox", "purpose": "Design reviews" } })).await.unwrap().as_bool().unwrap());
    let server = query(&app, &format!("SELECT body FROM mock_server_conversation WHERE external_id = {}", quoted(&external))).await;
    assert_eq!((server["metadata"]["topic"].as_str(), server["metadata"]["purpose"].as_str()), (Some("Ship the inbox"), Some("Design reviews")));
    assert!(app.engine("delete_conversation_membership", json!({ "conversationId": id })).await.unwrap().as_bool().unwrap());
}

#[tokio::test]
async fn a_bound_signature_signs_mail_and_can_be_left_out() {
    let app = App::fresh().await;
    app.put_setting("mail.send_delay_seconds", "0").await;
    let provider = app.mock_provider("gmail");
    let conversation = conversation_of(&app, "gmail", &["email"]).await;
    let id = conversation["id"].as_str().unwrap().to_string();
    let item = app.engine("post_content", json!({ "input": { "kind": "signature", "name": "Work", "body": "Milo Oakes\n{{ .account.email }}" } })).await.unwrap();
    let content_id = item["id"].as_str().unwrap().to_string();
    app.engine("put_signature", json!({ "input": { "content_id": content_id, "provider_id": provider, "alias": "", "use_new": true, "use_reply": true, "use_forward": true } })).await.unwrap();
    let preview = app.engine("get_signature", json!({ "input": { "provider_id": provider, "alias": "milo@northwind.example", "mode": "reply", "conversation_id": id } })).await.unwrap();
    assert!(preview["text"].as_str().unwrap().contains("milo@northwind.example"), "{}", preview);
    let signed = send(&app, &id, &provider, "Signed reply", json!({})).await;
    assert!(signed["content"].as_str().unwrap().contains("\n-- \nMilo Oakes"), "{}", signed);
    let plain = send(&app, &id, &provider, "Unsigned reply", json!({ "signature": false })).await;
    assert!(!plain["content"].as_str().unwrap().contains("-- "), "{}", plain);
    let synced = app.engine("put_signature_sync", json!({ "contentId": content_id })).await;
    assert!(synced.is_ok(), "{:?}", synced);
}

#[tokio::test]
async fn aliases_and_automatic_replies_go_through_the_mail_account() {
    let app = App::fresh().await;
    let provider = app.mock_provider("gmail");
    let aliases = app.engine("get_aliases", json!({ "providerId": provider })).await.unwrap();
    assert!(aliases.as_array().unwrap().len() >= 2 && aliases[0]["is_default"] == true, "{}", aliases);
    let reply = json!({ "enabled": true, "subject": "Away", "body": "Back on Monday", "starts_at": null, "ends_at": null, "external_only": false });
    assert!(app.engine("put_auto_reply", json!({ "providerId": provider, "reply": reply })).await.unwrap().as_bool().unwrap());
    let stored = app.engine("get_auto_reply", json!({ "providerId": provider })).await.unwrap();
    assert_eq!((stored["enabled"].as_bool(), stored["body"].as_str()), (Some(true), Some("Back on Monday")));
    let slack = app.mock_provider("slack");
    assert!(app.engine("get_auto_reply", json!({ "providerId": slack })).await.unwrap().is_null());
}

#[tokio::test]
async fn search_filters_narrow_and_saved_items_and_reminders_are_listed() {
    let app = App::fresh().await;
    let conversation = conversation_of(&app, "teams", &["direct_message", "group_message"]).await;
    let id = conversation["id"].as_str().unwrap().to_string();
    let first = messages(&app, &id).await.into_iter().find(|m| m["content"].as_str().is_some_and(|c| c.split_whitespace().count() > 2)).expect("a message with words");
    let word = first["content"].as_str().unwrap().split_whitespace().find(|w| w.chars().all(char::is_alphanumeric) && w.len() > 3).unwrap_or("the").to_string();
    let all = app.engine("get_search", json!({ "input": { "query": word } })).await.unwrap();
    let saved_only = app.engine("get_search", json!({ "input": { "query": format!("{} is:saved", word) } })).await.unwrap();
    assert!(all["messages"].as_array().unwrap().len() >= saved_only["messages"].as_array().unwrap().len());
    let message_id = first["id"].as_str().unwrap().to_string();
    app.engine("post_bookmark", json!({ "input": { "message_id": message_id } })).await.unwrap();
    let saved = app.engine("get_search", json!({ "input": { "query": "is:saved" } })).await.unwrap();
    assert!(saved["messages"].as_array().unwrap().iter().any(|m| m["id"] == message_id.as_str()), "{}", saved);
    let bookmarks = app.engine("get_bookmarks", json!({})).await.unwrap();
    assert!(bookmarks.to_string().contains(&message_id));
    let at = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    let reminder = app.engine("post_reminder", json!({ "input": { "message_id": message_id, "at": at } })).await.unwrap();
    let pending = app.engine("get_reminders", json!({ "pending": true })).await.unwrap();
    assert!(pending.as_array().unwrap().iter().any(|r| r["id"] == reminder["id"]));
    assert!(app.engine("delete_reminder", json!({ "reminderId": reminder["id"] })).await.unwrap().as_bool().unwrap());
    let activity = app.engine("get_activity", json!({})).await.unwrap();
    assert!(activity["items"].is_array());
}
