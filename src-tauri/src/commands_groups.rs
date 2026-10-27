use crate::models::group::{Group, GroupItemsInput, GroupQuery, PostGroupInput, PutGroupInput};
use crate::models::chat::{self, AttachmentOpenInput, Chat, ChatActionInput, ChatItem, GridItem, ChatNodeSpec, ChatPage, ChatRoutesInput, PostChatInput, PostChatMessageInput, PutChatInput};
use crate::services::{attachment_service, chat_service, group_service};
use crate::ChatnipService;
use tauri::{AppHandle, Emitter, State, Window};

async fn emit_groups(service: &ChatnipService, window: &Window) -> Result<Vec<Group>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let groups = group_service::all(service.db(), &profile).await?;
    window.emit("groups-updated", &groups).map_err(|e| format!("Failed to emit groups: {}", e))?;
    Ok(groups)
}

#[tauri::command]
pub async fn get_groups(service: State<'_, ChatnipService>) -> Result<Vec<Group>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    group_service::all(service.db(), &profile).await
}

#[tauri::command]
pub async fn post_group(service: State<'_, ChatnipService>, window: Window, input: PostGroupInput) -> Result<Group, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let created = Group::post(service.db(), &profile, &input).await?;
    let groups = emit_groups(&service, &window).await?;
    Ok(groups.into_iter().find(|g| g.id == created.id).unwrap_or(created))
}

#[tauri::command]
pub async fn put_group(service: State<'_, ChatnipService>, window: Window, input: PutGroupInput) -> Result<Group, String> {
    let updated = Group::put(service.db(), &input).await?;
    let groups = emit_groups(&service, &window).await?;
    Ok(groups.into_iter().find(|g| g.id == updated.id).unwrap_or(updated))
}

#[tauri::command]
pub async fn delete_group(service: State<'_, ChatnipService>, window: Window, group_id: String) -> Result<(), String> {
    Group::delete(service.db(), &group_id).await?;
    emit_groups(&service, &window).await?;
    Ok(())
}

#[tauri::command]
pub async fn put_group_items(service: State<'_, ChatnipService>, window: Window, input: GroupItemsInput) -> Result<Group, String> {
    Group::add_items(service.db(), &input.group_id, &input.conversation_ids).await?;
    let groups = emit_groups(&service, &window).await?;
    groups.into_iter().find(|g| g.id == input.group_id).ok_or_else(|| "Group not found".to_string())
}

#[tauri::command]
pub async fn delete_group_items(service: State<'_, ChatnipService>, window: Window, input: GroupItemsInput) -> Result<Group, String> {
    Group::remove_items(service.db(), &input.group_id, &input.conversation_ids).await?;
    let groups = emit_groups(&service, &window).await?;
    groups.into_iter().find(|g| g.id == input.group_id).ok_or_else(|| "Group not found".to_string())
}

#[tauri::command]
pub async fn get_query_conversations(service: State<'_, ChatnipService>, query: GroupQuery) -> Result<Vec<String>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    group_service::evaluate(service.db(), &profile, &query).await
}


#[tauri::command]
pub async fn get_read_conversation_ids(service: State<'_, ChatnipService>) -> Result<Vec<String>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    group_service::read_conversation_ids(service.db(), &profile).await
}

async fn emit_chats(window: &Window) -> Result<(), String> {
    use tauri::Manager;
    let conversations = crate::commands::get_conversations(window.state::<ChatnipService>()).await?;
    window.emit("conversations-updated", &conversations).map_err(|e| format!("Failed to emit conversations: {}", e))?;
    Ok(())
}

fn spawn_backfill(app: AppHandle, conversation_id: String) {
    tauri::async_runtime::spawn(async move {
        use tauri::Manager;
        let pool = app.state::<ChatnipService>().db().clone();
        let outcome = match crate::services::action_service::runner_for(&pool, &app, false).await {
            Ok(runner) => chat_service::backfill(&runner, &conversation_id, chat_service::BACKFILL_LIMIT).await.map(|_| ()),
            Err(e) => Err(e),
        };
        if let Err(e) = outcome {
            eprintln!("The chat {} was not filled: {}", conversation_id, e);
        }
    });
}

#[tauri::command]
pub async fn get_chats(service: State<'_, ChatnipService>) -> Result<Vec<Chat>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    Chat::all(service.db(), &profile).await
}

#[tauri::command]
pub async fn post_chat(service: State<'_, ChatnipService>, app: AppHandle, window: Window, input: PostChatInput) -> Result<Chat, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let chat = Chat::post(service.db(), &profile, &input).await?;
    for action_id in &input.action_ids {
        chat_service::attach(service.db(), &profile, &chat.conversation_id, action_id).await?;
    }
    emit_chats(&window).await?;
    if !input.action_ids.is_empty() {
        spawn_backfill(app, chat.conversation_id.clone());
    }
    Chat::get(service.db(), &profile, &chat.id).await
}

#[tauri::command]
pub async fn put_chat(service: State<'_, ChatnipService>, window: Window, input: PutChatInput) -> Result<Chat, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let chat = Chat::put(service.db(), &profile, &input).await?;
    emit_chats(&window).await?;
    window.emit("chat-items-updated", &chat.conversation_id).ok();
    Ok(chat)
}

#[tauri::command]
pub async fn delete_chat(service: State<'_, ChatnipService>, window: Window, chat_id: String) -> Result<(), String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    Chat::delete(service.db(), &profile, &chat_id).await?;
    emit_chats(&window).await
}

#[tauri::command]
pub async fn get_chat_items(service: State<'_, ChatnipService>, app: AppHandle, conversation_id: String, before: Option<String>, limit: Option<u32>) -> Result<ChatPage, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    if before.is_none() && !chat::action_ids(service.db(), &conversation_id).await?.is_empty() && Chat::find(service.db(), &profile, &conversation_id).await?.is_some() && !Chat::backfilled(service.db(), &conversation_id).await? {
        Chat::mark_backfilled(service.db(), &conversation_id).await?;
        spawn_backfill(app, conversation_id.clone());
    }
    chat_service::page(service.db(), &profile, &conversation_id, before.as_deref(), limit.map(|l| l as usize)).await
}

#[tauri::command]
pub async fn get_chat_grid(service: State<'_, ChatnipService>, conversation_id: String) -> Result<Vec<GridItem>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    chat_service::grid(service.db(), &profile, &conversation_id).await
}

#[tauri::command]
pub async fn get_chat_node() -> Result<ChatNodeSpec, String> {
    Ok(chat::node_spec().clone())
}

#[tauri::command]
pub async fn put_chat_action(service: State<'_, ChatnipService>, app: AppHandle, window: Window, input: ChatActionInput) -> Result<Vec<String>, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    chat_service::attach(service.db(), &profile, &input.conversation_id, &input.action_id).await?;
    app.emit("actions-updated", &input.action_id).ok();
    emit_chats(&window).await?;
    spawn_backfill(app, input.conversation_id.clone());
    chat::action_ids(service.db(), &input.conversation_id).await
}

#[tauri::command]
pub async fn delete_chat_action(service: State<'_, ChatnipService>, app: AppHandle, input: ChatActionInput) -> Result<Vec<String>, String> {
    chat::delete_action(service.db(), &input.conversation_id, &input.action_id).await?;
    app.emit("actions-updated", &input.action_id).ok();
    app.emit("chat-items-updated", &input.conversation_id).ok();
    chat::action_ids(service.db(), &input.conversation_id).await
}

#[tauri::command]
pub async fn post_chat_message(service: State<'_, ChatnipService>, app: AppHandle, input: PostChatMessageInput) -> Result<ChatItem, String> {
    let runner = crate::services::action_service::runner_for(service.db(), &app, false).await?;
    chat_service::post_message(&runner, &input.conversation_id, &input.text, input.target_id.as_deref(), input.reply_to_id.as_deref()).await
}

#[tauri::command]
pub async fn get_chat_pipe(service: State<'_, ChatnipService>, conversation_id: String) -> Result<chat_service::ChatPipe, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    chat_service::pipe(service.db(), &profile, &conversation_id).await
}

#[tauri::command]
pub async fn put_chat_routes(service: State<'_, ChatnipService>, app: AppHandle, input: ChatRoutesInput) -> Result<chat_service::ChatPipe, String> {
    let profile = crate::models::Profile::get_active_id(service.db()).await?;
    let pipe = chat_service::put_routes(service.db(), &profile, &input.conversation_id, &input.action_id, &input.routes).await?;
    app.emit("chat-items-updated", &input.conversation_id).ok();
    Ok(pipe)
}

#[tauri::command]
pub async fn post_chat_backfill(service: State<'_, ChatnipService>, app: AppHandle, conversation_id: String, limit: Option<i64>) -> Result<usize, String> {
    let runner = crate::services::action_service::runner_for(service.db(), &app, false).await?;
    chat_service::backfill(&runner, &conversation_id, limit.unwrap_or(chat_service::BACKFILL_LIMIT)).await
}

#[tauri::command]
pub async fn post_attachment_open(service: State<'_, ChatnipService>, app: AppHandle, input: AttachmentOpenInput) -> Result<String, String> {
    use tauri::Manager;
    use tauri_plugin_opener::OpenerExt;
    let app_data = app.path().app_data_dir().map_err(|e| format!("Failed to find the app data folder: {}", e))?;
    let cache = app.path().app_cache_dir().map_err(|e| format!("Failed to find the app cache folder: {}", e))?;
    let path = match (&input.message_id, &input.attachment_id, &input.path) {
        (Some(message_id), Some(attachment_id), _) => attachment_service::cached_by_id(service.db(), &app_data, &cache, message_id, attachment_id).await?,
        (_, _, Some(path)) => {
            let path = std::path::PathBuf::from(path);
            if !attachment_service::inside_cache(&cache, &path) {
                return Err("Chatnip only opens files it downloaded to its cache".to_string());
            }
            path
        }
        _ => return Err("Pass a message and attachment, or a cached path".to_string()),
    };
    let shown = path.to_string_lossy().to_string();
    if input.quick_look.unwrap_or(false) {
        std::process::Command::new("qlmanage").arg("-p").arg(&path).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map_err(|e| format!("Quick Look did not open: {}", e))?;
    } else {
        app.opener().open_path(&shown, None::<&str>).map_err(|e| format!("The file did not open: {}", e))?;
    }
    Ok(shown)
}
