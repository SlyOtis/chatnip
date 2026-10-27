use crate::models::app_setting::{self, AppSetting};
use crate::models::Profile;
use crate::services::intelligence_service;
use serde_json::Value;
use sqlx::{Pool, Sqlite};
use std::sync::Arc;
use tokio::sync::Semaphore;

static SLOT: Semaphore = Semaphore::const_new(1);

pub struct AssistantRun {
    pub kind: String,
    pub output: Value,
    pub cost_usd: f64,
    pub duration_ms: u64,
}

pub async fn enabled(pool: &Pool<Sqlite>) -> bool {
    AppSetting::get_or(pool, app_setting::ASSISTANT_ENABLED, "true").await.map(|v| v != "false").unwrap_or(true)
}

pub async fn ask(pool: &Pool<Sqlite>, kind: &str, system: &str, prompt: &str, schema: &Value) -> Result<AssistantRun, String> {
    if !enabled(pool).await {
        return Err(crate::services::action_script::REQUIREMENT_AI_ENABLED.error());
    }
    let profile = Profile::get_active(pool).await?.ok_or_else(|| "No active profile".to_string())?;
    intelligence_service::ensure_defaults(pool, &profile.id).await?;
    let (connection, model) = intelligence_service::get_connection(pool, &profile.id, kind, None, None, None).await?;
    let run = intelligence_service::execute(pool, &connection, kind, system, prompt, model.as_deref(), Some(schema)).await?;
    Ok(AssistantRun { kind: run.key, output: run.output, cost_usd: run.cost_usd, duration_ms: run.duration_ms })
}

pub fn slot() -> Arc<&'static Semaphore> {
    Arc::new(&SLOT)
}

#[cfg(test)]
mod live {
    use super::*;
    use crate::test_helpers::test_pool;
    use serde_json::json;

    #[tokio::test]
    async fn the_cli_answers_with_the_requested_structure() {
        if std::env::var("CHATNIP_ASSISTANT_TEST").is_err() {
            return;
        }
        let pool = test_pool().await;
        crate::test_helpers::test_profile(&pool).await;
        AppSetting::put(&pool, app_setting::ASSISTANT_MODEL, "claude-haiku-4-5-20251001").await.unwrap();
        let schema = json!({ "type": "object", "properties": { "label": { "type": "string", "enum": ["Important", "Newsletters", "Other"] } }, "required": ["label"] });
        let run = ask(&pool, "test", "Sort the conversation into one label.", "Subject: Your weekly digest. Sender: newsletter@vercel.com.", &schema).await.unwrap();
        assert_eq!(run.output["label"], "Newsletters");
        let logged: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM assistant_run").fetch_one(&pool).await.unwrap();
        assert_eq!(logged, 1);
    }
}
