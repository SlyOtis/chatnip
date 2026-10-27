use crate::models::case::{
    CaseExpectation, CaseFile, CaseItem, CaseItemsInput, CaseRule, PostCaseInput, PostCaseRuleInput, PostExpectationInput, PutCaseInput, PutExpectationInput,
};
use crate::models::Message;
use crate::services::case_service;
use crate::ChatnipService;
use tauri::{Emitter, State, Window};

async fn emit_cases(service: &ChatnipService, window: &Window) -> Result<Vec<CaseFile>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let cases = CaseFile::all(service.db(), &profile).await?;
    window.emit("cases-updated", &cases).map_err(|e| format!("Failed to emit cases: {}", e))?;
    Ok(cases)
}

#[tauri::command]
pub async fn get_cases(service: State<'_, ChatnipService>) -> Result<Vec<CaseFile>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    CaseFile::all(service.db(), &profile).await
}

#[tauri::command]
pub async fn get_case(service: State<'_, ChatnipService>, case_id: String) -> Result<CaseFile, String> {
    CaseFile::get(service.db(), &case_id).await
}

#[tauri::command]
pub async fn post_case(service: State<'_, ChatnipService>, window: Window, input: PostCaseInput) -> Result<CaseFile, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let id = CaseFile::post(service.db(), &profile, &input).await?;
    emit_cases(&service, &window).await?;
    CaseFile::get(service.db(), &id).await
}

#[tauri::command]
pub async fn put_case(service: State<'_, ChatnipService>, window: Window, input: PutCaseInput) -> Result<CaseFile, String> {
    CaseFile::put(service.db(), &input).await?;
    emit_cases(&service, &window).await?;
    CaseFile::get(service.db(), &input.case_id).await
}

#[tauri::command]
pub async fn delete_case(service: State<'_, ChatnipService>, window: Window, case_id: String) -> Result<(), String> {
    CaseFile::delete(service.db(), &case_id).await?;
    emit_cases(&service, &window).await?;
    Ok(())
}

#[tauri::command]
pub async fn put_case_items(service: State<'_, ChatnipService>, window: Window, input: CaseItemsInput) -> Result<CaseFile, String> {
    for c in &input.conversation_ids {
        CaseItem::add(service.db(), &input.case_id, Some(c), None).await?;
    }
    for m in &input.message_ids {
        CaseItem::add(service.db(), &input.case_id, None, Some(m)).await?;
    }
    crate::models::case::touch(service.db(), &input.case_id).await?;
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    for c in &input.conversation_ids {
        case_service::scan_conversation(service.db(), &profile, c).await?;
    }
    emit_cases(&service, &window).await?;
    CaseFile::get(service.db(), &input.case_id).await
}

#[tauri::command]
pub async fn delete_case_items(service: State<'_, ChatnipService>, window: Window, input: CaseItemsInput) -> Result<CaseFile, String> {
    for c in &input.conversation_ids {
        CaseItem::remove(service.db(), &input.case_id, Some(c), None).await?;
    }
    for m in &input.message_ids {
        CaseItem::remove(service.db(), &input.case_id, None, Some(m)).await?;
    }
    crate::models::case::touch(service.db(), &input.case_id).await?;
    emit_cases(&service, &window).await?;
    CaseFile::get(service.db(), &input.case_id).await
}

#[tauri::command]
pub async fn get_case_timeline(service: State<'_, ChatnipService>, case_id: String, limit: Option<u32>) -> Result<Vec<Message>, String> {
    let case = CaseFile::get(service.db(), &case_id).await?;
    case_service::timeline(service.db(), &case, limit).await
}

#[tauri::command]
pub async fn post_expectation(service: State<'_, ChatnipService>, window: Window, input: PostExpectationInput) -> Result<CaseFile, String> {
    if input.title.trim().is_empty() {
        return Err("An expectation needs a title".to_string());
    }
    CaseExpectation::post(service.db(), &input).await?;
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    case_service::scan_all(service.db(), &profile).await?;
    emit_cases(&service, &window).await?;
    CaseFile::get(service.db(), &input.case_id).await
}

#[tauri::command]
pub async fn put_expectation(service: State<'_, ChatnipService>, window: Window, input: PutExpectationInput) -> Result<CaseFile, String> {
    let case_id = CaseExpectation::put(service.db(), &input).await?;
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    case_service::scan_all(service.db(), &profile).await?;
    emit_cases(&service, &window).await?;
    CaseFile::get(service.db(), &case_id).await
}

#[tauri::command]
pub async fn delete_expectation(service: State<'_, ChatnipService>, window: Window, expectation_id: String) -> Result<(), String> {
    CaseExpectation::delete(service.db(), &expectation_id).await?;
    emit_cases(&service, &window).await?;
    Ok(())
}

#[tauri::command]
pub async fn post_case_rule(service: State<'_, ChatnipService>, window: Window, input: PostCaseRuleInput) -> Result<CaseFile, String> {
    CaseRule::post(service.db(), &input).await?;
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    case_service::scan_all(service.db(), &profile).await?;
    emit_cases(&service, &window).await?;
    CaseFile::get(service.db(), &input.case_id).await
}

#[tauri::command]
pub async fn delete_case_rule(service: State<'_, ChatnipService>, window: Window, rule_id: String) -> Result<(), String> {
    CaseRule::delete(service.db(), &rule_id).await?;
    emit_cases(&service, &window).await?;
    Ok(())
}

#[tauri::command]
pub async fn get_conversation_cases(service: State<'_, ChatnipService>, conversation_id: String) -> Result<Vec<String>, String> {
    CaseFile::for_conversation(service.db(), &conversation_id).await
}
