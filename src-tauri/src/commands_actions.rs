use crate::models::action::{
    Action, ActionGroup, ActionRun, MessageSchedule, PostActionGroupInput, PostActionInput, PostMessageScheduleInput, PutActionGroupInput, PutActionInput,
};
use crate::services::action_script::ActionDraft;
use crate::services::action_service::{self, ActionsOutcome, AssistInput, AssistOutcome, Catalog, CheckInput, CheckOutcome, PutActionsInput, Start};
use crate::ChatnipService;
use serde_json::{Map, Value};
use tauri::{AppHandle, Emitter, State};

async fn emit_actions(service: &ChatnipService, app: &AppHandle) -> Result<Vec<Action>, String> {
    let actions = Action::all(service.db(), &crate::models::Profile::get_active_id(service.db()).await?).await?;
    app.emit("actions-updated", &actions).map_err(|e| format!("Failed to emit actions: {}", e))?;
    Ok(actions)
}

pub(crate) async fn runner(service: &ChatnipService, app: &AppHandle, dry_run: bool) -> Result<action_service::Runner, String> {
    action_service::runner_for(service.db(), app, dry_run).await
}

#[tauri::command]
pub async fn get_action_groups(service: State<'_, ChatnipService>) -> Result<Vec<ActionGroup>, String> {
    ActionGroup::all(service.db(), &crate::models::Profile::get_active_id(service.db()).await?).await
}

#[tauri::command]
pub async fn post_action_group(service: State<'_, ChatnipService>, app: AppHandle, input: PostActionGroupInput) -> Result<ActionGroup, String> {
    let group = ActionGroup::post(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, &input).await?;
    emit_actions(&service, &app).await?;
    Ok(group)
}

#[tauri::command]
pub async fn put_action_group(service: State<'_, ChatnipService>, app: AppHandle, input: PutActionGroupInput) -> Result<ActionGroup, String> {
    let group = ActionGroup::put(service.db(), &input).await?;
    emit_actions(&service, &app).await?;
    Ok(group)
}

#[tauri::command]
pub async fn delete_action_group(service: State<'_, ChatnipService>, app: AppHandle, group_id: String) -> Result<(), String> {
    ActionGroup::delete(service.db(), &group_id).await?;
    emit_actions(&service, &app).await?;
    Ok(())
}

#[tauri::command]
pub async fn get_actions(service: State<'_, ChatnipService>) -> Result<Vec<Action>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    action_service::ensure_defaults(service.db(), &profile).await?;
    Action::all(service.db(), &profile).await
}

#[tauri::command]
pub async fn get_action(service: State<'_, ChatnipService>, action_id: String) -> Result<Action, String> {
    Action::find(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, &action_id).await
}

#[tauri::command]
pub async fn get_action_code(service: State<'_, ChatnipService>, action_id: String) -> Result<String, String> {
    Ok(action_service::code_of(&Action::find(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, &action_id).await?))
}

#[tauri::command]
pub async fn post_action(service: State<'_, ChatnipService>, app: AppHandle, input: PostActionInput) -> Result<Action, String> {
    let action = action_service::post_action(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, input).await?;
    emit_actions(&service, &app).await?;
    Ok(action)
}

#[tauri::command]
pub async fn put_action(service: State<'_, ChatnipService>, app: AppHandle, input: PutActionInput) -> Result<Action, String> {
    let action = action_service::put_action(service.db(), input).await?;
    emit_actions(&service, &app).await?;
    Ok(action)
}

#[tauri::command]
pub async fn post_action_copy(service: State<'_, ChatnipService>, app: AppHandle, action_id: String, without_trigger: Option<bool>, name: Option<String>) -> Result<Action, String> {
    let action = action_service::post_action_copy(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, &action_id, without_trigger.unwrap_or(false), name.as_deref()).await?;
    emit_actions(&service, &app).await?;
    Ok(action)
}

#[tauri::command]
pub async fn put_actions(service: State<'_, ChatnipService>, app: AppHandle, input: PutActionsInput) -> Result<ActionsOutcome, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let outcome = service.with_write_lock(move |pool| async move { action_service::put_actions(&pool, &profile, input.operations).await }).await?;
    emit_actions(&service, &app).await?;
    Ok(outcome)
}

#[tauri::command]
pub async fn delete_action(service: State<'_, ChatnipService>, app: AppHandle, action_id: String) -> Result<(), String> {
    Action::delete(service.db(), &action_id).await?;
    emit_actions(&service, &app).await?;
    Ok(())
}

#[tauri::command]
pub async fn post_action_check(service: State<'_, ChatnipService>, input: CheckInput) -> Result<CheckOutcome, String> {
    action_service::check_input(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, input).await
}

#[tauri::command]
pub async fn post_action_render(draft: ActionDraft) -> Result<String, String> {
    Ok(crate::services::action_script::render(&draft))
}

#[tauri::command]
pub async fn post_action_run(service: State<'_, ChatnipService>, app: AppHandle, action_id: String, input: Option<Value>, dry_run: Option<bool>, message_id: Option<String>) -> Result<ActionRun, String> {
    let runner = runner(&service, &app, dry_run.unwrap_or(false)).await?;
    let action = Action::find(service.db(), &runner.profile_id, &action_id).await?;
    let payload = match &message_id {
        Some(id) => action_service::payload_for_message(service.db(), &runner.profile_id, id).await?,
        None => Value::Null,
    };
    let inputs: Map<String, Value> = input.and_then(|v| v.as_object().cloned()).unwrap_or_default();
    let start = if runner.dry_run { action_service::dry_start(&action, inputs, payload) } else { Start::Invoke { inputs, payload } };
    let run = action_service::execute(&runner, &action, start, 0, vec![action.id.clone()]).await?;
    action_service::record(&runner, &run).await?;
    Ok(run)
}

#[tauri::command]
pub async fn get_action_sample(service: State<'_, ChatnipService>, message_id: Option<String>, person_id: Option<String>) -> Result<Value, String> {
    action_service::sample(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, message_id.as_deref(), person_id.as_deref()).await
}

#[tauri::command]
pub async fn get_action_runs(service: State<'_, ChatnipService>, action_id: Option<String>, limit: Option<i64>) -> Result<Vec<ActionRun>, String> {
    ActionRun::list(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, action_id.as_deref(), limit.unwrap_or(50)).await
}

#[tauri::command]
pub async fn get_action_catalog(service: State<'_, ChatnipService>) -> Result<Catalog, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    action_service::ensure_defaults(service.db(), &profile).await?;
    action_service::catalog(service.db(), &profile).await
}

#[tauri::command]
pub async fn post_action_backfill(service: State<'_, ChatnipService>, app: AppHandle, action_id: Option<String>, limit: Option<i64>) -> Result<action_service::BackfillReport, String> {
    let runner = runner(&service, &app, false).await?;
    action_service::backfill(&runner, action_id.as_deref(), limit.unwrap_or(50)).await
}

#[tauri::command]
pub async fn post_action_assist(service: State<'_, ChatnipService>, input: AssistInput) -> Result<AssistOutcome, String> {
    action_service::assist(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, input, None).await
}

#[tauri::command]
pub async fn post_message_schedule(service: State<'_, ChatnipService>, app: AppHandle, input: PostMessageScheduleInput) -> Result<MessageSchedule, String> {
    let schedule = MessageSchedule::post(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, &input).await?;
    app.emit("schedules-updated", &schedule.conversation_id).ok();
    Ok(schedule)
}

#[tauri::command]
pub async fn get_message_schedules(service: State<'_, ChatnipService>, conversation_id: Option<String>) -> Result<Vec<MessageSchedule>, String> {
    MessageSchedule::list(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, conversation_id.as_deref()).await
}

#[tauri::command]
pub async fn delete_message_schedule(service: State<'_, ChatnipService>, app: AppHandle, schedule_id: String) -> Result<(), String> {
    let schedule = MessageSchedule::get(service.db(), &schedule_id).await?;
    MessageSchedule::finish(service.db(), &schedule_id, crate::models::action::STATUS_CANCELLED, None).await?;
    app.emit("schedules-updated", &schedule.conversation_id).ok();
    Ok(())
}

#[tauri::command]
pub async fn get_message_presentation(service: State<'_, ChatnipService>, message_id: String) -> Result<crate::models::presentation::MessagePresentation, String> {
    crate::models::presentation::get_message(service.db(), &message_id).await
}

#[tauri::command]
pub async fn put_message_presentation(service: State<'_, ChatnipService>, app: AppHandle, input: crate::models::presentation::PutMessagePresentationInput) -> Result<crate::models::presentation::MessagePresentation, String> {
    let shown = crate::models::presentation::put_message(service.db(), &input.message_id, &input.presentation).await?;
    app.emit("presentation-updated", &shown).map_err(|e| format!("Failed to emit the presentation: {}", e))?;
    Ok(shown)
}

#[tauri::command]
pub async fn put_conversation_presentation(service: State<'_, ChatnipService>, app: AppHandle, input: crate::models::presentation::PutConversationPresentationInput) -> Result<Value, String> {
    let defaults = crate::models::presentation::put_conversation(service.db(), &input.conversation_id, &input.presentation).await?;
    app.emit("presentation-updated", &serde_json::json!({ "conversation_id": input.conversation_id, "conversation": defaults })).map_err(|e| format!("Failed to emit the presentation: {}", e))?;
    Ok(defaults)
}

#[tauri::command]
pub async fn post_action_node_run(service: State<'_, ChatnipService>, app: AppHandle, action_id: String, node_id: String, input: Option<Value>, dry_run: Option<bool>, message_id: Option<String>) -> Result<ActionRun, String> {
    let runner = runner(&service, &app, dry_run.unwrap_or(true)).await?;
    let action = Action::find(service.db(), &runner.profile_id, &action_id).await?;
    let single = action_service::isolate_node(service.db(), &runner.profile_id, &action, &node_id).await?;
    let payload = match &message_id {
        Some(id) => action_service::payload_for_message(service.db(), &runner.profile_id, id).await?,
        None => Value::Null,
    };
    let inputs: Map<String, Value> = input.and_then(|v| v.as_object().cloned()).unwrap_or_default();
    let run = action_service::execute(&runner, &single, Start::Invoke { inputs, payload }, 0, vec![action.id.clone()]).await?;
    action_service::record(&runner, &run).await?;
    Ok(run)
}

#[derive(serde::Serialize)]
pub struct PresentationTemplate {
    pub action_id: String,
    pub node_id: String,
    pub kind: String,
    pub source: String,
}

#[tauri::command]
pub async fn get_presentation_template(service: State<'_, ChatnipService>, action_id: String, node_id: String) -> Result<PresentationTemplate, String> {
    let action = Action::find(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, &action_id).await?;
    let (kind, source) = action_service::template_source(&action, &node_id)?;
    Ok(PresentationTemplate { action_id: action.id, node_id, kind, source })
}

#[tauri::command]
pub async fn get_action_hook(action_id: String, node_id: String) -> Result<String, String> {
    Ok(format!("http://127.0.0.1:{}/hook/{}/{}", crate::services::connection_service::get_oauth_port(), action_id, node_id))
}

#[tauri::command]
pub async fn post_presentation_template_error(service: State<'_, ChatnipService>, app: AppHandle, action_id: String, node_id: String, message_id: String, error: String) -> Result<ActionRun, String> {
    let runner = runner(&service, &app, false).await?;
    let action = Action::find(service.db(), &runner.profile_id, &action_id).await?;
    let run = action_service::template_failure(&action, &node_id, &message_id, &error);
    action_service::record(&runner, &run).await?;
    Ok(run)
}

#[tauri::command]
pub async fn post_template_assist(service: State<'_, ChatnipService>, input: action_service::TemplateAssistInput) -> Result<action_service::TemplateAssistOutcome, String> {
    action_service::assist_template(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, input, None).await
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActionTest {
    pub action_id: String,
    pub name: String,
    pub group: Option<String>,
    pub builtin: bool,
    pub status: String,
    pub done: usize,
    pub would: usize,
    pub skipped: usize,
    pub failed: usize,
    pub effects: usize,
    pub error: Option<String>,
    pub run_id: String,
    pub kind: String,
    pub message: String,
    pub result: Value,
}

#[tauri::command]
pub async fn post_action_tests(service: State<'_, ChatnipService>, app: AppHandle, action_id: Option<String>) -> Result<Vec<ActionTest>, String> {
    let mut runner = runner(&service, &app, true).await?;
    runner.test_delay = Some(std::time::Duration::from_secs(4));
    let sample = action_service::sample(service.db(), &runner.profile_id, None, None).await?;
    let mut inputs: Map<String, Value> = Map::new();
    for key in ["message", "conversation", "person"] {
        if let Some(value) = sample.get(key).filter(|v| !v.is_null()) {
            inputs.insert(key.into(), value.clone());
        }
    }
    let text = sample.get("text").and_then(Value::as_str).or_else(|| sample.pointer("/message/text").and_then(Value::as_str)).unwrap_or("").to_string();
    inputs.insert("text".into(), Value::String(text));
    let groups: std::collections::HashMap<String, String> = ActionGroup::all(service.db(), &runner.profile_id).await?.into_iter().map(|g| (g.id, g.name)).collect();
    let mut out = Vec::new();
    for action in Action::all(service.db(), &runner.profile_id).await? {
        let picked = match &action_id {
            Some(id) => &action.id == id,
            None => action.usage.standalone || action.usage.chat,
        };
        if !picked {
            continue;
        }
        let start = action_service::dry_start(&action, inputs.clone(), Value::Null);
        let run = action_service::execute(&runner, &action, start, 0, vec![action.id.clone()]).await?;
        action_service::record(&runner, &run).await?;
        let steps = |status: &str| run.log.iter().filter(|l| l.kind != "effect" && l.kind != "log" && l.status == status).count();
        out.push(ActionTest {
            action_id: action.id.clone(),
            name: action.name.clone(),
            group: action.group_id.as_ref().and_then(|id| groups.get(id).cloned()),
            builtin: action.builtin,
            status: run.status.clone(),
            done: steps("done"),
            would: steps("would"),
            skipped: steps("skipped"),
            failed: steps("failed"),
            effects: run.log.iter().filter(|l| l.kind == "effect").count(),
            error: run.error.clone(),
            run_id: run.id.clone(),
            kind: run.result.get("kind").and_then(Value::as_str).unwrap_or("none").to_string(),
            message: run.result.get("message").and_then(Value::as_str).unwrap_or("").to_string(),
            result: run.result.clone(),
        });
    }
    Ok(out)
}
