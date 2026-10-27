#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use tauri::Manager;
use sqlx::{Pool, Sqlite};
pub mod providers;
pub mod commands;
pub mod commands_people;
pub mod commands_cases;
pub mod commands_content;
pub mod commands_links;
pub mod commands_security;
pub mod commands_groups;
pub mod commands_inboxes;
pub mod commands_stats;
pub mod commands_intelligence;
pub mod commands_actions;
pub mod commands_voice;
pub mod commands_compose;
pub mod commands_conversations;
pub mod database;
pub mod models;
pub mod loading_state;
pub mod utils;
pub mod content;
pub mod services;
pub mod cli;
#[cfg(test)]
pub mod test_helpers;
use database::create_database_pool;
use commands::{
    post_startup, post_profile, get_profile, put_profile, put_profile_settings, put_profile_theme,
    post_profile_theme, post_profile_avatar, put_profile_avatar,
    get_conversations, get_conversation, post_message, post_message_reaction, post_message_reply, post_message_forward, get_message_quote,
    post_sync,
    post_provider_sync, put_provider_logo,
    get_current_conversation, put_current_conversation, delete_current_conversation, get_current_messages, get_older_messages,
    post_provider_auth, post_slack_connection, get_connection_setup, post_admin_consent, post_imap_connection, delete_provider, get_app_settings, put_app_setting,
    get_tags, post_tag, put_tag, put_tag_favourites, delete_tag, get_object_tags, get_tagged_objects, put_object_tag, delete_object_tag,
    get_assistant_runs,
    get_events, get_proposed_events, post_event, put_event, delete_event, get_calendar_export, post_export, get_calendars, put_calendar, put_event_merge, post_log, get_platform, put_window_position, put_window_frame,
    post_provider_connection, get_providers, get_users, get_provider_theme, put_provider_theme, get_asset,
    post_conversation, post_conversation_merge, post_conversation_link, get_conversation_links, delete_conversation_link, get_merge_groups, post_merge_group, delete_merge_group_items,
    post_thread, post_thread_reply, get_thread_messages,
    put_conversation, post_conversation_member, delete_conversation_member,
    get_search,
    put_message, delete_message,
    post_message_pin, delete_message_pin, get_pinned_messages,
    post_bookmark, delete_bookmark, get_bookmarks,
    put_message_unread, put_conversation_read,
    put_conversation_notification,
    post_sidebar_section, put_sidebar_section, delete_sidebar_section,
    post_sidebar_section_item, delete_sidebar_section_item, get_sidebar_sections,
    put_user_status, get_user_status,
};
use loading_state::LoadingStateManager;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub static STARTUP: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone)]
pub struct ChatnipService {
    db: Pool<Sqlite>,
    loading_state: LoadingStateManager,
    write_semaphore: Arc<Semaphore>,
    pending_auth: services::connection_service::PendingAuths,
}
impl ChatnipService {
    pub fn new(db: Pool<Sqlite>) -> Self {
        Self { 
            db,
            loading_state: LoadingStateManager::new(),
            write_semaphore: Arc::new(Semaphore::new(1)),
            pending_auth: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }
    
    pub fn db(&self) -> &Pool<Sqlite> {
        &self.db
    }

    pub fn pending_auth(&self) -> &services::connection_service::PendingAuths {
        &self.pending_auth
    }
    
    pub fn loading_state(&self) -> &LoadingStateManager {
        &self.loading_state
    }
    
    pub async fn with_write_lock<F, Fut, R>(&self, operation: F) -> Result<R, String>
    where
        F: FnOnce(Pool<Sqlite>) -> Fut,
        Fut: std::future::Future<Output = Result<R, String>>,
    {
        use tokio::time::{timeout, Duration};
        
        println!("🔒 Attempting to acquire write lock...");
        let permit = timeout(Duration::from_secs(30), self.write_semaphore.acquire())
            .await
            .map_err(|_| "Timeout waiting for write lock".to_string())?
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        
        println!("✅ Write lock acquired");
        let result = operation(self.db.clone()).await;
        drop(permit);
        println!("🔓 Write lock released");
        result
    }
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Err((code, e)) = cli::configure(&args) {
        cli::style::fail(&e);
        std::process::exit(code);
    }
    if let Some(home) = std::env::var_os("CHATNIP_HOME") {
        std::env::set_var("HOME", &home);
        std::env::set_var("CFFIXED_USER_HOME", &home);
    }
    dotenv::dotenv().ok();
    if cli::wants_cli(&args) {
        std::process::exit(cli::run(args));
    }
    let app = app_builder()
        .setup(|app| {
            commands_security::watch_screen_lock(app.handle());
            services::notification_service::listen(app.handle());
            start_blocking(app.handle().clone(), true)?;
            if let Some(window) = app.get_webview_window("main").map(|window| window.as_ref().window()) {
                services::window_service::put_start(&window)?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                services::window_service::put_closing(window);
            }
        })
        .build(app_context())
        .expect("error while building tauri application");
    app.run(|handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } => {
            if let Some(window) = handle.get_webview_window("main").map(|window| window.as_ref().window()) {
                services::window_service::put_closing(&window);
            }
            services::outbound_service::quit(handle);
        }
        tauri::RunEvent::Exit => cli::engine::remove_socket(handle),
        _ => {}
    });
}

pub fn app_context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

pub fn app_builder() -> tauri::Builder<tauri::Wry> {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init());
    #[cfg(feature = "webdriver")]
    {
        builder = builder.plugin(tauri_plugin_webdriver::init());
    }
    builder
        .register_asynchronous_uri_scheme_protocol("asset", |ctx, request, responder| {
            let app_handle = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                let uri_str = request.uri().to_string();
                let path = uri_str.strip_prefix("asset://localhost/").unwrap_or("");
                
                let decoded_path = percent_encoding::percent_decode_str(path)
                    .decode_utf8()
                    .unwrap_or_else(|_| std::borrow::Cow::from(path))
                    .to_string();
                
                let app_data_dir = match app_handle.path().app_data_dir() {
                    Ok(dir) => dir,
                    Err(_) => {
                        responder.respond(
                            tauri::http::Response::builder()
                                .status(404)
                                .body(Vec::new())
                                .unwrap()
                        );
                        return;
                    }
                };
                
                let file_path = app_data_dir.join(&decoded_path);
                
                if !file_path.exists() || !file_path.is_file() {
                    responder.respond(
                        tauri::http::Response::builder()
                            .status(404)
                            .body(Vec::new())
                            .unwrap()
                    );
                    return;
                }
                
                match std::fs::read(&file_path) {
                    Ok(content) => {
                        let mime_type = if decoded_path.ends_with(".png") {
                            "image/png"
                        } else if decoded_path.ends_with(".jpg") || decoded_path.ends_with(".jpeg") {
                            "image/jpeg"
                        } else if decoded_path.ends_with(".gif") {
                            "image/gif"
                        } else if decoded_path.ends_with(".svg") {
                            "image/svg+xml"
                        } else if decoded_path.ends_with(".webp") {
                            "image/webp"
                        } else if decoded_path.ends_with(".ico") {
                            "image/x-icon"
                        } else {
                            "application/octet-stream"
                        };
                        
                        responder.respond(
                            tauri::http::Response::builder()
                                .status(200)
                                .header("Content-Type", mime_type)
                                .header("Access-Control-Allow-Origin", "*")
                                .body(content)
                                .unwrap()
                        );
                    }
                    Err(_) => {
                        responder.respond(
                            tauri::http::Response::builder()
                                .status(500)
                                .body(Vec::new())
                                .unwrap()
                        );
                    }
                }
            });
        })
        .invoke_handler({
            let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
            commands_security::get_lock_state,
            commands_security::post_lock,
            commands_security::delete_lock,
            commands_security::put_lock,
            post_startup,
            post_profile,
            get_profile,
            put_profile,
            put_profile_settings,
            put_profile_theme,
            post_profile_theme,
            post_profile_avatar,
            put_profile_avatar,
            get_conversations,
            get_conversation,
            post_message,
            commands::get_outbound_holds,
            commands::get_history,
            commands::post_history,
            commands::put_history,
            commands::delete_outbound_hold,
            commands::get_outbound_opened,
            post_message_reaction,
            post_message_reply,
            post_message_forward,
            get_message_quote,
            post_sync,
            post_provider_sync,
            put_provider_logo,
            get_current_conversation,
            put_current_conversation,
            delete_current_conversation,
            get_current_messages,
            get_older_messages,
            post_provider_auth,
            post_slack_connection,
            get_connection_setup,
            post_admin_consent,
            post_imap_connection,
            delete_provider,
            get_tags,
            post_tag,
            put_tag,
            put_tag_favourites,
            delete_tag,
            get_object_tags,
            get_tagged_objects,
            put_object_tag,
            delete_object_tag,
            get_assistant_runs,
            get_events,
            get_proposed_events,
            post_event,
            get_calendars,
            put_calendar,
            put_event_merge,
            put_event,
            commands::put_event_response,
            delete_event,
            get_calendar_export,
            post_export,
            commands_people::get_people,
            commands_people::get_my_user_ids,
            commands_people::get_person_profile,
            commands_cases::get_cases,
            commands_cases::get_case,
            commands_cases::post_case,
            commands_cases::put_case,
            commands_cases::delete_case,
            commands_cases::put_case_items,
            commands_cases::delete_case_items,
            commands_cases::get_case_timeline,
            commands_cases::post_expectation,
            commands_cases::put_expectation,
            commands_cases::delete_expectation,
            commands_cases::post_case_rule,
            commands_cases::delete_case_rule,
            commands_cases::get_conversation_cases,
            commands_people::get_person,
            commands_people::get_person_messages,
            commands_people::get_conversation_messages,
            commands_people::get_merge_group_messages,
            commands_people::get_shared_files,
            commands_people::get_shared_links,
            commands_people::put_person,
            commands_people::get_object_overrides,
            commands_people::put_object_override,
            commands_people::delete_object_override,
            commands_people::post_person_merge,
            commands_people::delete_person_identity,
            commands_people::post_person_identity,
            commands_people::put_people_match,
            commands_people::put_favourites,
            commands_people::post_mock_connection,
            commands_people::get_mock_mode,
            commands_people::post_mock_seed,
            commands_people::put_mock_state,
            get_app_settings,
            put_app_setting,
            post_log,
            get_platform,
            put_window_position,
            put_window_frame,
            post_provider_connection,
            get_providers,
            get_users,
            get_provider_theme,
            put_provider_theme,
            get_asset,
            post_conversation,
            post_conversation_merge,
            post_conversation_link,
            get_conversation_links,
            delete_conversation_link,
            get_merge_groups,
            post_merge_group,
            delete_merge_group_items,
            post_thread,
            post_thread_reply,
            get_thread_messages,
            put_conversation,
            post_conversation_member,
            delete_conversation_member,
            get_search,
            put_message,
            delete_message,
            post_message_pin,
            delete_message_pin,
            get_pinned_messages,
            post_bookmark,
            delete_bookmark,
            get_bookmarks,
            put_message_unread,
            put_conversation_read,
            put_conversation_notification,
            post_sidebar_section,
            put_sidebar_section,
            delete_sidebar_section,
            post_sidebar_section_item,
            delete_sidebar_section_item,
            get_sidebar_sections,
            put_user_status,
            get_user_status,
            commands_content::get_gifs,
            commands_content::get_content,
            commands_content::post_content,
            commands_content::put_content,
            commands_content::put_content_order,
            commands_content::delete_content,
            commands_content::get_content_tags,
            commands_content::put_content_bulk,
            commands_content::post_content_from_message,
            commands_content::post_content_from_url,
            commands_content::post_content_send,
            commands_content::post_content_render,
            commands_content::get_content_uses,
            commands_content::get_content_groups,
            commands_content::post_content_group,
            commands_content::put_content_group,
            commands_content::delete_content_group,
            commands_content::put_content_group_items,
            commands_content::delete_content_group_items,
            commands_content::get_reaction_uses,
            commands_content::get_workspace_emoji,
            commands_links::get_link_preview,
            commands_links::get_link_previews,
            commands_links::get_message_link,
            commands_links::get_link_target,
            commands_inboxes::get_inboxes,
            commands_inboxes::post_inbox,
            commands_inboxes::put_inbox,
            commands_inboxes::put_inbox_order,
            commands_inboxes::delete_inbox,
            commands_inboxes::put_inbox_members,
            commands_inboxes::get_sidebar_layout,
            commands_inboxes::put_sidebar_layout,
            commands_groups::get_groups,
            commands_groups::post_group,
            commands_groups::put_group,
            commands_groups::get_read_conversation_ids,
            commands_groups::get_chats,
            commands_groups::post_chat,
            commands_groups::put_chat,
            commands_groups::delete_chat,
            commands_groups::get_chat_items,
            commands_groups::get_chat_grid,
            commands_groups::get_chat_node,
            commands_groups::put_chat_action,
            commands_groups::delete_chat_action,
            commands_groups::post_chat_message,
            commands_groups::get_chat_pipe,
            commands_groups::put_chat_routes,
            commands_groups::post_chat_backfill,
            commands_groups::post_attachment_open,
            commands_groups::delete_group,
            commands_groups::put_group_items,
            commands_groups::delete_group_items,
            commands_groups::get_query_conversations,
            commands_stats::get_stats,
            commands_intelligence::get_intelligence_connections,
            commands_intelligence::post_intelligence_connection,
            commands_intelligence::put_intelligence_connection,
            commands_intelligence::delete_intelligence_connection,
            commands_intelligence::post_intelligence_connection_test,
            commands_intelligence::get_intelligence_models,
            commands_intelligence::get_prompts,
            commands_intelligence::get_section_summaries,
            commands_intelligence::post_section_summary,
            commands_compose::get_drafts,
            commands_compose::put_draft,
            commands_compose::delete_draft,
            commands_compose::get_reminders,
            commands_compose::post_reminder,
            commands_compose::delete_reminder,
            commands_compose::get_signature,
            commands_compose::get_signatures,
            commands_compose::put_signature,
            commands_compose::delete_signature,
            commands_compose::post_signature_import,
            commands_compose::put_signature_sync,
            commands_compose::get_aliases,
            commands_compose::get_auto_reply,
            commands_compose::put_auto_reply,
            commands_conversations::get_joinable_conversations,
            commands_conversations::post_conversation_membership,
            commands_conversations::delete_conversation_membership,
            commands_conversations::put_conversation_state,
            commands_conversations::put_conversation_topic,
            commands_conversations::post_provider_conversation,
            commands_conversations::get_presences,
            commands_conversations::get_user_groups,
            commands_conversations::get_activity,
            commands_conversations::get_provider_messages,
            commands_actions::get_action_groups,
            commands_actions::post_action_group,
            commands_actions::put_action_group,
            commands_actions::delete_action_group,
            commands_actions::get_actions,
            commands_actions::get_action,
            commands_actions::get_action_code,
            commands_actions::post_action,
            commands_actions::put_action,
            commands_actions::delete_action,
            commands_actions::post_action_check,
            commands_actions::post_action_render,
            commands_actions::post_action_run,
            commands_actions::post_action_tests,
            commands_actions::get_action_runs,
            commands_actions::get_action_sample,
            commands_actions::get_action_catalog,
            commands_actions::get_message_presentation,
            commands_actions::post_action_node_run,
            commands_actions::get_presentation_template, commands_actions::get_action_hook,
            commands_actions::post_presentation_template_error,
            commands_actions::post_template_assist,
            commands_actions::put_message_presentation,
            commands_actions::put_conversation_presentation,
            commands_actions::post_action_backfill,
            commands_actions::post_action_copy,
            commands_actions::put_actions,
            commands_actions::post_action_assist,
            commands_actions::post_message_schedule,
            commands_actions::get_message_schedules,
            commands_actions::delete_message_schedule,
            commands_intelligence::get_ai_setup,
            commands_intelligence::get_setting_registry,
            commands_intelligence::post_ollama_pull,
            commands_intelligence::post_ollama_start,
            commands_intelligence::get_cli_setup,
            commands_intelligence::post_claude_plugin,
            commands_intelligence::post_cli_path,
            commands_voice::get_voice_setup,
            commands_voice::post_voice_setup,
            commands_voice::post_voice,
            commands_voice::delete_voice,
        ];
            move |invoke: tauri::ipc::Invoke<tauri::Wry>| {
                if !services::security_service::allows(invoke.message.command()) {
                    invoke.resolver.reject("Chatnip is locked");
                    return true;
                }
                handler(invoke)
            }
        })
}

pub fn start_blocking(handle: tauri::AppHandle, gui: bool) -> Result<(), Box<dyn std::error::Error>> {
            let server = handle.clone();
            
            // Use a channel to signal when the service is ready
            let (tx, rx) = std::sync::mpsc::channel();
            
            tauri::async_runtime::spawn(async move {
                match create_database_pool(&handle).await {
                    Ok(pool) => {
                        let fresh = gui && commands_people::mock_enabled(&pool).await && matches!(models::Profile::get_active(&pool).await, Ok(None));
                        let startup = if fresh { Some(STARTUP.lock().await) } else { None };
                        if commands_security::lock_enabled(&pool).await {
                            services::security_service::set_locked(true);
                        }
                        handle.manage(ChatnipService::new(pool.clone()));
                        println!("✅ Service initialized successfully");
                        let _ = tx.send(Ok(()));
                        if let Err(e) = models::AppSetting::seed_from_env(&pool).await {
                            eprintln!("Settings were not seeded from the environment: {}", e);
                        }
                        match services::security_service::migrate_secrets(&pool).await {
                            Ok(0) => {}
                            Ok(moved) => println!("🔐 Moved {} secrets into the keychain", moved),
                            Err(e) => eprintln!("⚠️ Secrets were not moved into the keychain: {}", e),
                        }
                        if fresh {
                            for kind in providers::mock::MockKind::ALL {
                                match commands_people::post_mock_connection(handle.clone(), handle.state::<ChatnipService>(), kind.key().to_string()).await {
                                    Ok(provider) => {
                                        if let Err(e) = services::connection_service::sync_provider(&handle, &pool, &provider.id).await {
                                            eprintln!("The {} test client did not sync: {}", kind.key(), e);
                                        }
                                    }
                                    Err(e) => eprintln!("The {} test client was not connected: {}", kind.key(), e),
                                }
                            }
                        }
                        // Do profile validation first
                        if gui { match models::Profile::get_active(&pool).await {
                            Ok(Some(profile)) => {
                                println!("🔑 Found active profile: {}", profile.name);
                                if let Err(e) = profile.validate_providers(&pool).await {
                                    eprintln!("⚠️ Failed to validate providers for profile {}: {}", profile.name, e);
                                }
                            }
                            Ok(None) => {
                                println!("ℹ️ No active profile found, skipping provider validation");
                            }
                            Err(e) => {
                                eprintln!("⚠️ Failed to get active profile: {}", e);
                            }
                        } }
                        
                        if let (true, Ok(Some(profile))) = (gui, models::Profile::get_active(&pool).await) {
                            if let Err(e) = models::Tag::ensure_defaults(&pool, &profile.id).await {
                                eprintln!("Inbox splits were not seeded: {}", e);
                            }
                            if let Err(e) = models::Tag::migrate_favourites(&pool, &profile.id).await {
                                eprintln!("Stars and favourites were not moved to tags: {}", e);
                            }
                            if let Err(e) = models::inbox::Inbox::ensure_defaults(&pool, &profile.id).await {
                                eprintln!("Inboxes were not created: {}", e);
                            }
                            if let Err(e) = services::intelligence_service::ensure_defaults(&pool, &profile.id).await {
                                eprintln!("Intelligence defaults were not seeded: {}", e);
                            }
                            if let Err(e) = services::action_service::ensure_defaults(&pool, &profile.id).await {
                                eprintln!("Default actions were not seeded: {}", e);
                            }
                            if let Err(e) = services::people_service::rematch(&pool, &profile.id).await {
                                eprintln!("People were not matched: {}", e);
                            }
                            if commands_people::mock_enabled(&pool).await {
                                let app_data = handle.path().app_data_dir().ok();
                                if let Err(e) = providers::mock::seed::seed(&pool, &profile.id, app_data.as_deref()).await {
                                    eprintln!("Mock data was not seeded: {}", e);
                                }
                            }
                        }
                        drop(startup);
                        if gui {
                            services::connection_service::spawn_listener(handle.clone());
                            services::connection_service::spawn_sync_loop(handle.clone());
                            services::live_service::spawn_live(handle.clone());
                            services::action_service::spawn_scheduler(handle.clone());
                            services::outbound_service::spawn_resume(handle.clone());
                            services::history_service::spawn_retry(handle.clone());
                            cli::engine::serve_socket(server);
                        }
                        println!("✅ Startup tasks finished");
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to create database pool: {}", e);
                        let _ = tx.send(Err(e));
                    }
                }
            });
            
            // Wait for initialization to complete
            match rx.recv_timeout(std::time::Duration::from_secs(30)) {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => {
                    eprintln!("❌ Failed to initialize: {}", e);
                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to initialize: {}", e)
                    )))
                }
                Err(_) => {
                    eprintln!("❌ Service initialization timed out");
                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Service initialization timed out"
                    )))
                }
            }
}
