use sqlx::{Pool, Sqlite};
use sqlx::sqlite::SqlitePoolOptions;
use crate::database::init_database;
use uuid::Uuid;
use chrono::Utc;

pub async fn test_pool() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create test pool");
    init_database(&pool).await.expect("Failed to init schema");
    pool
}

pub async fn test_profile(pool: &Pool<Sqlite>) -> String {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO profile (id, name, active, setup_step, created_at) VALUES (?1, ?2, 1, 0, ?3)"
    )
    .bind(&id)
    .bind("Test Profile")
    .bind(&now)
    .execute(pool)
    .await
    .expect("Failed to insert test profile");
    id
}

pub async fn test_conversation(pool: &Pool<Sqlite>, name: &str, conversation_type: &str) -> String {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO conversation (id, name, type, group_type, last_activity, created_at, updated_at) VALUES (?1, ?2, ?3, 'auto', ?4, ?4, ?4)"
    )
    .bind(&id)
    .bind(name)
    .bind(conversation_type)
    .bind(&now)
    .execute(pool)
    .await
    .expect("Failed to insert test conversation");
    id
}

pub async fn test_provider(pool: &Pool<Sqlite>, profile_id: &str, provider_type: &str) -> String {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let capabilities = serde_json::json!({
        "can_send_messages": true,
        "can_edit_messages": false,
        "can_delete_messages": false,
        "can_create_conversations": false,
        "can_add_members": false,
        "supports_reactions": true,
        "supports_threads": false,
        "supports_attachments": false,
        "supports_presence": false,
        "supports_typing_indicators": false,
        "supports_read_receipts": false,
    });
    sqlx::query(
        "INSERT INTO provider (id, profile_id, provider_type, name, capabilities, is_enabled, sync_interval, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 1, 300, ?6, ?6)"
    )
    .bind(&id)
    .bind(profile_id)
    .bind(provider_type)
    .bind(format!("Test {}", provider_type))
    .bind(capabilities.to_string())
    .bind(&now)
    .execute(pool)
    .await
    .expect("Failed to insert test provider");
    id
}

pub async fn test_message(pool: &Pool<Sqlite>, conversation_id: &str, provider_id: &str, content: &str) -> String {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT INTO message (id, provider_id, external_id, type, content, content_type, conversation_id, sender_name, importance, is_read, is_draft, has_attachments, created_datetime, last_modified_datetime, timestamp, created_at, updated_at)
        VALUES (?1, ?2, ?1, 'chat', ?3, 'text', ?4, 'Test User', 'normal', 0, 0, 0, ?5, ?5, ?5, ?5, ?5)"#
    )
    .bind(&id)
    .bind(provider_id)
    .bind(content)
    .bind(conversation_id)
    .bind(&now)
    .execute(pool)
    .await
    .expect("Failed to insert test message");
    id
}

pub async fn test_user(pool: &Pool<Sqlite>, email: &str, display_name: &str) -> String {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO user (id, email, display_name, first_seen, last_updated) VALUES (?1, ?2, ?3, ?4, ?4)"
    )
    .bind(&id)
    .bind(email)
    .bind(display_name)
    .bind(&now)
    .execute(pool)
    .await
    .expect("Failed to insert test user");
    id
}
