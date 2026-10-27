use crate::models::tag::KIND_SPLIT;
use crate::models::rule::Subject;
use sqlx::{Pool, Row, Sqlite};

pub const SOURCE_MANUAL: &str = "manual";

pub async fn subject(pool: &Pool<Sqlite>, conversation_id: &str) -> Result<Subject, String> {
    let row = sqlx::query(
        "SELECT c.name, c.type, c.namespace,
            (SELECT COUNT(*) FROM conversation_member WHERE conversation_id = c.id) AS members,
            p.provider_type, p.name AS connection_name
         FROM conversation c
         LEFT JOIN conversation_provider cp ON cp.conversation_id = c.id AND cp.is_primary = 1
         LEFT JOIN provider p ON p.id = cp.provider_id
         WHERE c.id = ?1",
    )
    .bind(conversation_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to load the conversation: {}", e))?
    .ok_or_else(|| "Conversation not found".to_string())?;
    let latest = sqlx::query(
        "SELECT sender_email, sender_name, subject, content, content_type FROM message
         WHERE conversation_id = ?1 AND deleted_datetime IS NULL ORDER BY timestamp DESC LIMIT 1",
    )
    .bind(conversation_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to load the latest message: {}", e))?;
    let (sender_email, sender_name, subject, body) = match latest {
        Some(m) => {
            let content: Option<String> = m.get("content");
            let content_type: Option<String> = m.get("content_type");
            let text = crate::utils::text_extraction::extract_text_from_content(content.as_deref().unwrap_or(""), content_type.as_deref().unwrap_or("text"));
            (
                m.try_get::<Option<String>, _>("sender_email").ok().flatten().unwrap_or_default().to_lowercase(),
                m.try_get::<Option<String>, _>("sender_name").ok().flatten().unwrap_or_default(),
                m.try_get::<Option<String>, _>("subject").ok().flatten().unwrap_or_default(),
                text.chars().take(2000).collect(),
            )
        }
        None => (String::new(), String::new(), String::new(), String::new()),
    };
    Ok(Subject {
        provider_type: row.try_get::<Option<String>, _>("provider_type").ok().flatten().unwrap_or_default(),
        connection_name: row.try_get::<Option<String>, _>("connection_name").ok().flatten().unwrap_or_default(),
        conversation_type: row.get("type"),
        conversation_name: row.get("name"),
        sender_email,
        sender_name,
        subject,
        body,
        member_count: row.try_get::<i64, _>("members").unwrap_or(0),
        namespace: row.try_get::<Option<String>, _>("namespace").ok().flatten().unwrap_or_default(),
    })
}

pub async fn has_split(pool: &Pool<Sqlite>, conversation_id: &str) -> Result<bool, String> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM object_tag ot JOIN tag l ON l.id = ot.tag_id WHERE ot.object_type = 'conversation' AND ot.object_id = ?1 AND l.kind = ?2",
    )
    .bind(conversation_id)
    .bind(KIND_SPLIT)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to check the split: {}", e))?;
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::tag::{ObjectTag, Tag, OBJECT_CONVERSATION};
    use crate::test_helpers::{test_conversation, test_message, test_pool, test_profile, test_provider};

    #[tokio::test]
    async fn the_subject_reads_the_latest_message_and_splits_are_seen() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        Tag::ensure_defaults(&pool, &profile).await.unwrap();
        let provider = test_provider(&pool, &profile, "mock").await;
        let conversation = test_conversation(&pool, "Vercel", "email").await;
        let message = test_message(&pool, &conversation, &provider, "Your deploy finished").await;
        sqlx::query("UPDATE message SET sender_email = 'Bot@Vercel.com', subject = 'Deploy' WHERE id = ?1").bind(&message).execute(&pool).await.unwrap();
        let read = subject(&pool, &conversation).await.unwrap();
        assert_eq!(read.sender_email, "bot@vercel.com");
        assert_eq!(read.subject, "Deploy");
        assert!(read.body.contains("deploy finished"));
        assert!(!has_split(&pool, &conversation).await.unwrap());
        let team = Tag::all(&pool, &profile).await.unwrap().into_iter().find(|l| l.name == "Team").unwrap();
        ObjectTag::put(&pool, OBJECT_CONVERSATION, &conversation, &team.id, SOURCE_MANUAL, None, None).await.unwrap();
        assert!(has_split(&pool, &conversation).await.unwrap());
    }
}
