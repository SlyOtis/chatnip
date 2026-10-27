use crate::models::intelligence::{IntelligenceConnection, PostIntelligenceConnectionInput, PutIntelligenceConnectionInput};
use crate::services::action_script;
use crate::models::app_setting::{self, AppSetting, SettingRegistry};
use crate::services::intelligence_service::{self, GetSectionSummariesInput, PostSectionSummaryInput, Prompt, SectionSummaries};
use crate::ChatnipService;
use tauri::State;

#[tauri::command]
pub async fn get_intelligence_connections(service: State<'_, ChatnipService>) -> Result<Vec<IntelligenceConnection>, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    intelligence_service::ensure_defaults(service.db(), &profile_id).await?;
    let mut described = Vec::new();
    for connection in IntelligenceConnection::all(service.db(), &profile_id).await? {
        described.push(intelligence_service::describe(service.db(), connection).await);
    }
    Ok(described)
}

#[tauri::command]
pub async fn post_intelligence_connection(service: State<'_, ChatnipService>, input: PostIntelligenceConnectionInput) -> Result<IntelligenceConnection, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    let created = IntelligenceConnection::post(service.db(), &profile_id, input).await?;
    Ok(intelligence_service::describe(service.db(), created).await)
}

#[tauri::command]
pub async fn put_intelligence_connection(service: State<'_, ChatnipService>, input: PutIntelligenceConnectionInput) -> Result<IntelligenceConnection, String> {
    let updated = IntelligenceConnection::put(service.db(), input).await?;
    Ok(intelligence_service::describe(service.db(), updated).await)
}

#[tauri::command]
pub async fn delete_intelligence_connection(service: State<'_, ChatnipService>, connection_id: String) -> Result<(), String> {
    IntelligenceConnection::delete(service.db(), &connection_id).await
}

#[tauri::command]
pub async fn post_intelligence_connection_test(service: State<'_, ChatnipService>, connection_id: String) -> Result<String, String> {
    intelligence_service::test_connection(service.db(), &connection_id).await
}

#[tauri::command]
pub async fn get_intelligence_models(service: State<'_, ChatnipService>, connection_id: String) -> Result<Vec<String>, String> {
    intelligence_service::models(service.db(), &connection_id).await
}

#[tauri::command]
pub async fn get_prompts() -> Result<Vec<Prompt>, String> {
    Ok(intelligence_service::prompts())
}

#[tauri::command]
pub async fn get_section_summaries(service: State<'_, ChatnipService>, input: GetSectionSummariesInput) -> Result<SectionSummaries, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    intelligence_service::get_section_summaries(service.db(), &profile_id, &input).await
}

#[tauri::command]
pub async fn post_section_summary(service: State<'_, ChatnipService>, input: PostSectionSummaryInput) -> Result<SectionSummaries, String> {
    let profile_id = crate::models::Profile::get_active_id(service.db()).await?;
    match intelligence_service::post_section_summary(service.db(), &profile_id, &input).await {
        Ok(summary) => Ok(SectionSummaries { summaries: vec![summary], requirements: Vec::new() }),
        Err(e) => match action_script::unmet_of(&e) {
            Some(needed) => Ok(SectionSummaries { summaries: Vec::new(), requirements: vec![needed] }),
            None => Err(e),
        },
    }
}


#[derive(serde::Serialize)]
pub struct AiSetup {
    pub claude: intelligence_service::ClaudeStatus,
    pub ollama: intelligence_service::OllamaStatus,
    pub suggestions: Vec<intelligence_service::ModelSuggestion>,
    pub claude_model: String,
}

#[tauri::command]
pub async fn get_ai_setup(service: State<'_, ChatnipService>, refresh: Option<bool>) -> Result<AiSetup, String> {
    let refresh = refresh.unwrap_or(false);
    let host = AppSetting::get_value(service.db(), app_setting::OLLAMA_HOST).await?;
    let (claude, ollama) = tokio::join!(intelligence_service::claude_status(refresh), intelligence_service::ollama_status(&host, refresh));
    let suggestions = intelligence_service::suggestions(&ollama);
    let claude_model = AppSetting::get_value(service.db(), app_setting::ASSISTANT_MODEL).await?;
    Ok(AiSetup { claude, ollama, suggestions, claude_model })
}

#[tauri::command]
pub async fn get_setting_registry(service: State<'_, ChatnipService>) -> Result<SettingRegistry, String> {
    AppSetting::get_registry(service.db()).await
}

#[tauri::command]
pub async fn post_ollama_pull(service: State<'_, ChatnipService>, app: tauri::AppHandle, model: String) -> Result<(), String> {
    use tauri::Emitter;
    let host = AppSetting::get_value(service.db(), app_setting::OLLAMA_HOST).await?;
    intelligence_service::pull_ollama_model(&host, &model, |progress| {
        let _ = app.emit("ollama-pull", &progress);
    })
    .await
}

#[tauri::command]
pub async fn post_ollama_start(service: State<'_, ChatnipService>) -> Result<intelligence_service::OllamaStatus, String> {
    let host = AppSetting::get_value(service.db(), app_setting::OLLAMA_HOST).await?;
    intelligence_service::start_ollama(&host).await
}

#[tauri::command]
pub async fn get_cli_setup() -> Result<crate::cli::plugin::CliSetup, String> {
    tokio::task::spawn_blocking(crate::cli::plugin::setup).await.map_err(|e| format!("The command line status could not be read: {}", e))
}

#[tauri::command]
pub async fn post_claude_plugin(options: Option<crate::cli::plugin::PluginOptions>) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || crate::cli::plugin::install_plugin(false, &options.unwrap_or_default())).await.map_err(|e| format!("The plugin install stopped: {}", e))?
}

#[tauri::command]
pub async fn post_cli_path() -> Result<String, String> {
    crate::cli::plugin::link(None).map(|path| path.to_string_lossy().to_string())
}
