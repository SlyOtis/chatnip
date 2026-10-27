use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::{Pool, Row, Sqlite};

pub const KINDS: [&str; 10] = ["auto", "chat", "mail", "invite", "notification", "bot", "digest", "system", "media", "hidden"];
const OPTIONS: [&str; 8] = ["collapsed", "summary_only", "highlight", "pinned", "badge", "icon", "invite", "template"];

#[derive(Debug, Clone, Deserialize)]
pub struct PutMessagePresentationInput {
    pub message_id: String,
    pub presentation: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PutConversationPresentationInput {
    pub conversation_id: String,
    pub presentation: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessagePresentation {
    pub message_id: String,
    pub own: Value,
    pub conversation: Value,
    pub effective: Value,
}

pub fn clean(input: &Value) -> Result<Value, String> {
    let Some(object) = input.as_object() else { return Err("A presentation is an object".to_string()) };
    let mut out = Map::new();
    if let Some(kind) = object.get("kind") {
        if kind.is_null() {
            out.insert("kind".into(), Value::Null);
            return Ok(options(object, out));
        }
        let kind = kind.as_str().ok_or("kind is a text")?;
        if !KINDS.contains(&kind) {
            return Err(format!("kind must be one of {}", KINDS.join(", ")));
        }
        out.insert("kind".into(), Value::String(kind.to_string()));
    }
    Ok(options(object, out))
}

fn options(object: &Map<String, Value>, mut out: Map<String, Value>) -> Value {
    for key in OPTIONS {
        if let Some(value) = object.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(out)
}

pub fn merge(base: &Value, over: &Value) -> Value {
    let mut out = base.as_object().cloned().unwrap_or_default();
    if let Some(over) = over.as_object() {
        for (key, value) in over {
            if value.is_null() {
                out.remove(key);
            } else {
                out.insert(key.clone(), value.clone());
            }
        }
    }
    Value::Object(out)
}

fn parse(raw: Option<String>) -> Value {
    raw.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_else(|| json!({}))
}

async fn read(pool: &Pool<Sqlite>, table: &str, id: &str) -> Result<Value, String> {
    let raw: Option<Option<String>> = sqlx::query_scalar(&format!("SELECT presentation FROM {} WHERE id = ?1", table)).bind(id).fetch_optional(pool).await.map_err(|e| format!("Failed to read the presentation: {}", e))?;
    match raw {
        Some(raw) => Ok(parse(raw)),
        None => Err(format!("No {} with id {}", table, id)),
    }
}

async fn write(pool: &Pool<Sqlite>, table: &str, id: &str, value: &Value) -> Result<(), String> {
    let stored = if value.as_object().is_none_or(|o| o.is_empty()) { None } else { Some(value.to_string()) };
    sqlx::query(&format!("UPDATE {} SET presentation = ?1 WHERE id = ?2", table)).bind(stored).bind(id).execute(pool).await.map_err(|e| format!("Failed to keep the presentation: {}", e))?;
    Ok(())
}

pub async fn get_message(pool: &Pool<Sqlite>, message_id: &str) -> Result<MessagePresentation, String> {
    let row = sqlx::query("SELECT m.presentation AS own, c.presentation AS conversation FROM message m LEFT JOIN conversation c ON c.id = m.conversation_id WHERE m.id = ?1")
        .bind(message_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to read the presentation: {}", e))?
        .ok_or_else(|| format!("No message with id {}", message_id))?;
    let own = parse(row.try_get("own").ok().flatten());
    let conversation = parse(row.try_get("conversation").ok().flatten());
    Ok(MessagePresentation { message_id: message_id.to_string(), effective: merge(&conversation, &own), own, conversation })
}

pub async fn put_message(pool: &Pool<Sqlite>, message_id: &str, patch: &Value) -> Result<MessagePresentation, String> {
    let current = read(pool, "message", message_id).await?;
    write(pool, "message", message_id, &merge(&current, &clean(patch)?)).await?;
    get_message(pool, message_id).await
}

pub async fn put_conversation(pool: &Pool<Sqlite>, conversation_id: &str, patch: &Value) -> Result<Value, String> {
    let current = read(pool, "conversation", conversation_id).await?;
    let next = merge(&current, &clean(patch)?);
    write(pool, "conversation", conversation_id, &next).await?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_conversation, test_message, test_pool, test_profile, test_provider};

    #[test]
    fn cleaning_keeps_known_kinds_and_options_only() {
        assert_eq!(clean(&json!({ "kind": "invite", "badge": "RSVP", "colour": "red" })).unwrap(), json!({ "kind": "invite", "badge": "RSVP" }));
        assert!(clean(&json!({ "kind": "poster" })).is_err());
        assert!(clean(&json!("invite")).is_err());
        assert_eq!(merge(&json!({ "kind": "mail", "collapsed": true }), &json!({ "collapsed": null, "highlight": "#e0612f" })), json!({ "kind": "mail", "highlight": "#e0612f" }));
    }

    #[tokio::test]
    async fn message_presentation_layers_over_conversation_defaults() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "mock").await;
        let conversation = test_conversation(&pool, "Plattform", "email").await;
        let message = test_message(&pool, &conversation, &provider, "Hei").await;
        put_conversation(&pool, &conversation, &json!({ "kind": "digest", "highlight": "#1f9d55" })).await.unwrap();
        let shown = put_message(&pool, &message, &json!({ "kind": "invite", "invite": { "title": "Sprintdemo" } })).await.unwrap();
        assert_eq!(shown.effective["kind"], "invite");
        assert_eq!(shown.effective["highlight"], "#1f9d55");
        assert_eq!(shown.own["invite"]["title"], "Sprintdemo");
        let cleared = put_message(&pool, &message, &json!({ "kind": null, "invite": null })).await.unwrap();
        assert_eq!(cleared.effective["kind"], "digest");
    }
}
