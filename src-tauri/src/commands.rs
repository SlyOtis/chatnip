use tauri::{State, Window, WebviewWindow, Emitter, Manager, AppHandle};
use sqlx::Row;
use chrono::Utc;
use uuid::Uuid;
use crate::ChatnipService;
use crate::models::{Profile, ProfileSetting, ProfileTheme,
    Conversation, Message, WindowPosition, Asset, PostProfileInput, PostProfileThemeInput,
    PutProfileInput, PutProfileSettingsInput, PutProfileThemeInput,
    PostProfileAvatarInput, PutProfileAvatarInput, SendMessageInput,
    SendMessageReactionInput, ConnectProviderInput, ConnectImapInput,
    GetConversationInput, SetCurrentConversationInput, StartProviderAuthInput,
    ConversationsResponse, ConversationResponse, UsersResponse, User, ProvidersResponse, MessagesResponse, ProfileResponse,
    Thread, CreateConversationInput, MergeConversationsInput, LinkConversationsInput,
    CreateThreadInput, ReplyInThreadInput, GetThreadInput, UpdateConversationInput,
    ConversationMemberInput, SearchInput, SearchResults,
    EditMessageInput, DeleteMessageInput, PinMessageInput, GetPinnedInput,
    BookmarkMessageInput, UnbookmarkMessageInput, MarkMessageUnreadInput, MarkConversationReadInput,
    PinnedMessage, BookmarkedMessage,
    SetNotificationInput, CreateSidebarSectionInput, UpdateSidebarSectionInput,
    DeleteSidebarSectionInput, SidebarSectionItemInput, RemoveSidebarSectionItemInput,
    SidebarSection,
    SetUserStatusInput, GetUserStatusInput, UserStatus};
use crate::providers::models::Provider;
use crate::services::{MessageService, MessageLoadOptions};

macro_rules! log_command {
    ($name:expr) => {
        println!("📍 Command: {}", $name);
    };
    ($name:expr, $($arg:tt)*) => {
        println!("📍 Command: {} - {}", $name, format!($($arg)*));
    };
}

async fn get_current_profile_id(pool: &sqlx::Pool<sqlx::Sqlite>) -> Result<String, String> {
    let profile = Profile::get_active(pool).await?
        .ok_or_else(|| "No active profile found. Please create a profile first.".to_string())?;
    
    Ok(profile.id)
}

async fn get_current_conversation_id(pool: &sqlx::Pool<sqlx::Sqlite>) -> Result<Option<String>, String> {
    let profile = Profile::get_active(pool).await?
        .ok_or_else(|| "No active profile found. Please create a profile first.".to_string())?;
    
    Ok(profile.last_id)
}


#[tauri::command]
pub async fn post_startup(
    service: State<'_, ChatnipService>,
    window: Window,
) -> Result<Option<Profile>, String> {
    log_command!("post_startup");
    drop(crate::STARTUP.lock().await);
    if let Err(e) = crate::services::connection_service::put_capabilities(service.db()).await {
        eprintln!("Capabilities were not refreshed: {}", e);
    }
    
    // Get active profile with all details
    let profile_state = load_profile_with_settings_and_theme(&service).await?;
    
    if let Some(ref state) = profile_state {
        let providers_response = get_providers(service.clone()).await?;
        let missing: Vec<String> = providers_response.data.into_iter()
            .filter(|provider| provider.logo_id.is_none() && provider.provider_type == "microsoft")
            .map(|provider| provider.id)
            .collect();
        if !missing.is_empty() {
            let handle = window.app_handle().clone();
            let logo_window = window.clone();
            tauri::async_runtime::spawn(async move {
                for provider_id in missing {
                    if let Err(e) = put_provider_logo(handle.state::<ChatnipService>(), logo_window.clone(), provider_id).await {
                        println!("   ⚠️ Failed to fetch logo: {}", e);
                    }
                }
            });
        }

        // Emit profile state update
        window.emit("profile-updated", &ProfileResponse {
            data: Some(state.clone()),
            is_loading: false,
        })
            .map_err(|e| format!("Failed to emit profile event: {}", e))?;
        
        // If there's a last conversation id, notify about current conversation
        if let Some(ref last_id) = state.last_id {
            // Get the conversation and emit it
            let conversation_row = sqlx::query("SELECT * FROM conversation WHERE id = ?1")
                .bind(last_id)
                .fetch_optional(service.db())
                .await
                .map_err(|e| format!("Failed to get conversation: {}", e))?;
                
            if let Some(row) = conversation_row {
                let mut conversation = Conversation::from_row(row)?;
                conversation.is_current = true;
                conversation.update_summary(service.db()).await.ok();
                conversation.load_display(service.db()).await.ok();
                
                window.emit("conversation-updated", &ConversationResponse { data: Some(conversation.clone()), is_loading: false })
                    .map_err(|e| format!("Failed to emit conversation event: {}", e))?;
            }
        }
    }
    
    Ok(profile_state)
}
#[tauri::command]
pub async fn post_profile(
    service: State<'_, ChatnipService>,
    input: PostProfileInput,
) -> Result<ProfileResponse, String> {
    let name = input.name;
    service.with_write_lock(move |pool| async move {
        let profile = Profile::post(&pool, &name).await?;
        Ok(ProfileResponse {
            data: Some(profile),
            is_loading: false,
        })
    }).await
}
#[tauri::command]
pub async fn get_profile(
    service: State<'_, ChatnipService>,
) -> Result<ProfileResponse, String> {
    let mut profile = match Profile::get_active(service.db()).await? {
        Some(p) => p,
        None => return Ok(ProfileResponse {
            data: None,
            is_loading: false,
        }),
    };
    
    // Load settings
    let settings_row = sqlx::query(
        "SELECT * FROM profile_setting WHERE profile_id = ?1"
    )
    .bind(&profile.id)
    .fetch_optional(service.db())
    .await
    .map_err(|e| format!("Failed to get settings: {}", e))?;
    
    profile.settings = settings_row.map(|row| ProfileSetting {
        id: row.get("id"),
        profile_id: row.get("profile_id"),
        email_content_threshold: row.get("email_content_threshold"),
        navigation_width: row.get("navigation_width"),
        navigation_collapsed: row.get("navigation_collapsed"),
        details_width: row.get("details_width"),
        details_panel_open: row.get("details_panel_open"),
        window_position: WindowPosition::from_row(&row),
        message_timestamp_interval: row.get("message_timestamp_interval"),
        time_format: row.get("time_format"),
        window_controls_style: row.get("window_controls_style"),
        max_input_lines: row.get::<Option<i32>, _>("max_input_lines").unwrap_or(10),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    });
    
    // Load theme
    let theme_row = sqlx::query("SELECT * FROM profile_theme WHERE profile_id = ?1 AND is_active = true")
        .bind(&profile.id)
        .fetch_optional(service.db())
        .await
        .map_err(|e| format!("Failed to get theme: {}", e))?;
    
    profile.theme = theme_row.map(|row| ProfileTheme {
        id: row.get("id"),
        profile_id: row.get("profile_id"),
        theme_name: row.get("theme_name"),
        is_active: row.get("is_active"),
        primary_50: row.get("primary_50"),
        primary_100: row.get("primary_100"),
        primary_200: row.get("primary_200"),
        primary_300: row.get("primary_300"),
        primary_400: row.get("primary_400"),
        primary_500: row.get("primary_500"),
        primary_600: row.get("primary_600"),
        primary_700: row.get("primary_700"),
        primary_800: row.get("primary_800"),
        primary_900: row.get("primary_900"),
        primary_950: row.get("primary_950"),
        secondary_50: row.get("secondary_50"),
        secondary_100: row.get("secondary_100"),
        secondary_200: row.get("secondary_200"),
        secondary_300: row.get("secondary_300"),
        secondary_400: row.get("secondary_400"),
        secondary_500: row.get("secondary_500"),
        secondary_600: row.get("secondary_600"),
        secondary_700: row.get("secondary_700"),
        secondary_800: row.get("secondary_800"),
        secondary_900: row.get("secondary_900"),
        secondary_950: row.get("secondary_950"),
        accent_50: row.get("accent_50"),
        accent_100: row.get("accent_100"),
        accent_200: row.get("accent_200"),
        accent_300: row.get("accent_300"),
        accent_400: row.get("accent_400"),
        accent_500: row.get("accent_500"),
        accent_600: row.get("accent_600"),
        accent_700: row.get("accent_700"),
        accent_800: row.get("accent_800"),
        accent_900: row.get("accent_900"),
        accent_950: row.get("accent_950"),
        background_50: row.get("background_50"),
        background_100: row.get("background_100"),
        background_200: row.get("background_200"),
        background_300: row.get("background_300"),
        background_400: row.get("background_400"),
        background_500: row.get("background_500"),
        background_600: row.get("background_600"),
        background_700: row.get("background_700"),
        background_800: row.get("background_800"),
        background_900: row.get("background_900"),
        background_950: row.get("background_950"),
        content_50: row.get("content_50"),
        content_100: row.get("content_100"),
        content_200: row.get("content_200"),
        content_300: row.get("content_300"),
        content_400: row.get("content_400"),
        content_500: row.get("content_500"),
        content_600: row.get("content_600"),
        content_700: row.get("content_700"),
        content_800: row.get("content_800"),
        content_900: row.get("content_900"),
        content_950: row.get("content_950"),
        control_50: row.get("control_50"),
        control_100: row.get("control_100"),
        control_200: row.get("control_200"),
        control_300: row.get("control_300"),
        control_400: row.get("control_400"),
        control_500: row.get("control_500"),
        control_600: row.get("control_600"),
        control_700: row.get("control_700"),
        control_800: row.get("control_800"),
        control_900: row.get("control_900"),
        control_950: row.get("control_950"),
        error_50: row.get("error_50"),
        error_100: row.get("error_100"),
        error_200: row.get("error_200"),
        error_300: row.get("error_300"),
        error_400: row.get("error_400"),
        error_500: row.get("error_500"),
        error_600: row.get("error_600"),
        error_700: row.get("error_700"),
        error_800: row.get("error_800"),
        error_900: row.get("error_900"),
        error_950: row.get("error_950"),
        warn_50: row.get("warn_50"),
        warn_100: row.get("warn_100"),
        warn_200: row.get("warn_200"),
        warn_300: row.get("warn_300"),
        warn_400: row.get("warn_400"),
        warn_500: row.get("warn_500"),
        warn_600: row.get("warn_600"),
        warn_700: row.get("warn_700"),
        warn_800: row.get("warn_800"),
        warn_900: row.get("warn_900"),
        warn_950: row.get("warn_950"),
        success_50: row.get("success_50"),
        success_100: row.get("success_100"),
        success_200: row.get("success_200"),
        success_300: row.get("success_300"),
        success_400: row.get("success_400"),
        success_500: row.get("success_500"),
        success_600: row.get("success_600"),
        success_700: row.get("success_700"),
        success_800: row.get("success_800"),
        success_900: row.get("success_900"),
        success_950: row.get("success_950"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    });
    
    Ok(ProfileResponse {
        data: Some(profile),
        is_loading: false,
    })
}

async fn load_profile_with_settings_and_theme(
    service: &State<'_, ChatnipService>,
) -> Result<Option<Profile>, String> {
    let mut profile = match Profile::get_active(service.db()).await? {
        Some(p) => p,
        None => return Ok(None),
    };
    
    // Load settings
    let settings_row = sqlx::query(
        "SELECT * FROM profile_setting WHERE profile_id = ?1"
    )
    .bind(&profile.id)
    .fetch_optional(service.db())
    .await
    .map_err(|e| format!("Failed to get settings: {}", e))?;
    
    profile.settings = settings_row.map(|row| ProfileSetting {
        id: row.get("id"),
        profile_id: row.get("profile_id"),
        email_content_threshold: row.get("email_content_threshold"),
        navigation_width: row.get("navigation_width"),
        navigation_collapsed: row.get("navigation_collapsed"),
        details_width: row.get("details_width"),
        details_panel_open: row.get("details_panel_open"),
        window_position: WindowPosition::from_row(&row),
        message_timestamp_interval: row.get("message_timestamp_interval"),
        time_format: row.get("time_format"),
        window_controls_style: row.get("window_controls_style"),
        max_input_lines: row.get::<Option<i32>, _>("max_input_lines").unwrap_or(10),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    });
    
    // Load theme
    let theme_row = sqlx::query("SELECT * FROM profile_theme WHERE profile_id = ?1 AND is_active = true")
        .bind(&profile.id)
        .fetch_optional(service.db())
        .await
        .map_err(|e| format!("Failed to get theme: {}", e))?;
    
    profile.theme = theme_row.map(|row| ProfileTheme {
        id: row.get("id"),
        profile_id: row.get("profile_id"),
        theme_name: row.get("theme_name"),
        is_active: row.get("is_active"),
        primary_50: row.get("primary_50"),
        primary_100: row.get("primary_100"),
        primary_200: row.get("primary_200"),
        primary_300: row.get("primary_300"),
        primary_400: row.get("primary_400"),
        primary_500: row.get("primary_500"),
        primary_600: row.get("primary_600"),
        primary_700: row.get("primary_700"),
        primary_800: row.get("primary_800"),
        primary_900: row.get("primary_900"),
        primary_950: row.get("primary_950"),
        secondary_50: row.get("secondary_50"),
        secondary_100: row.get("secondary_100"),
        secondary_200: row.get("secondary_200"),
        secondary_300: row.get("secondary_300"),
        secondary_400: row.get("secondary_400"),
        secondary_500: row.get("secondary_500"),
        secondary_600: row.get("secondary_600"),
        secondary_700: row.get("secondary_700"),
        secondary_800: row.get("secondary_800"),
        secondary_900: row.get("secondary_900"),
        secondary_950: row.get("secondary_950"),
        accent_50: row.get("accent_50"),
        accent_100: row.get("accent_100"),
        accent_200: row.get("accent_200"),
        accent_300: row.get("accent_300"),
        accent_400: row.get("accent_400"),
        accent_500: row.get("accent_500"),
        accent_600: row.get("accent_600"),
        accent_700: row.get("accent_700"),
        accent_800: row.get("accent_800"),
        accent_900: row.get("accent_900"),
        accent_950: row.get("accent_950"),
        background_50: row.get("background_50"),
        background_100: row.get("background_100"),
        background_200: row.get("background_200"),
        background_300: row.get("background_300"),
        background_400: row.get("background_400"),
        background_500: row.get("background_500"),
        background_600: row.get("background_600"),
        background_700: row.get("background_700"),
        background_800: row.get("background_800"),
        background_900: row.get("background_900"),
        background_950: row.get("background_950"),
        content_50: row.get("content_50"),
        content_100: row.get("content_100"),
        content_200: row.get("content_200"),
        content_300: row.get("content_300"),
        content_400: row.get("content_400"),
        content_500: row.get("content_500"),
        content_600: row.get("content_600"),
        content_700: row.get("content_700"),
        content_800: row.get("content_800"),
        content_900: row.get("content_900"),
        content_950: row.get("content_950"),
        control_50: row.get("control_50"),
        control_100: row.get("control_100"),
        control_200: row.get("control_200"),
        control_300: row.get("control_300"),
        control_400: row.get("control_400"),
        control_500: row.get("control_500"),
        control_600: row.get("control_600"),
        control_700: row.get("control_700"),
        control_800: row.get("control_800"),
        control_900: row.get("control_900"),
        control_950: row.get("control_950"),
        error_50: row.get("error_50"),
        error_100: row.get("error_100"),
        error_200: row.get("error_200"),
        error_300: row.get("error_300"),
        error_400: row.get("error_400"),
        error_500: row.get("error_500"),
        error_600: row.get("error_600"),
        error_700: row.get("error_700"),
        error_800: row.get("error_800"),
        error_900: row.get("error_900"),
        error_950: row.get("error_950"),
        warn_50: row.get("warn_50"),
        warn_100: row.get("warn_100"),
        warn_200: row.get("warn_200"),
        warn_300: row.get("warn_300"),
        warn_400: row.get("warn_400"),
        warn_500: row.get("warn_500"),
        warn_600: row.get("warn_600"),
        warn_700: row.get("warn_700"),
        warn_800: row.get("warn_800"),
        warn_900: row.get("warn_900"),
        warn_950: row.get("warn_950"),
        success_50: row.get("success_50"),
        success_100: row.get("success_100"),
        success_200: row.get("success_200"),
        success_300: row.get("success_300"),
        success_400: row.get("success_400"),
        success_500: row.get("success_500"),
        success_600: row.get("success_600"),
        success_700: row.get("success_700"),
        success_800: row.get("success_800"),
        success_900: row.get("success_900"),
        success_950: row.get("success_950"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    });
    
    Ok(Some(profile))
}

#[tauri::command]
pub async fn put_profile(
    service: State<'_, ChatnipService>,
    window: Window,
    input: PutProfileInput,
) -> Result<ProfileResponse, String> {
    let profile = Profile::get_active(service.db()).await?
        .ok_or_else(|| "No active profile found".to_string())?;
    let id = profile.id.clone();
    let name = input.name;
    let avatar_id = input.avatar_id;
    let theme_id = input.theme_id;
    let setup_step = input.setup_step;
    let current = sqlx::query(
        "SELECT * FROM profile WHERE id = ?1"
    )
    .bind(&id)
    .fetch_optional(service.db())
    .await
    .map_err(|e| format!("Failed to get current profile: {}", e))?;
    if current.is_none() {
        return Err("Profile not found".to_string());
    }
    let row = current.unwrap();
    let final_name = name.unwrap_or_else(|| row.get("name"));
    let final_avatar_id: Option<String> = avatar_id.or_else(|| row.get("avatar_id"));
    let final_theme_id: Option<String> = theme_id.or_else(|| row.get("theme_id"));
    let final_setup_step: i32 = setup_step.unwrap_or_else(|| row.get("setup_step"));
    sqlx::query(
        "UPDATE profile SET name = ?1, avatar_id = ?2, theme_id = ?3, setup_step = ?4 WHERE id = ?5"
    )
    .bind(&final_name)
    .bind(&final_avatar_id)
    .bind(&final_theme_id)
    .bind(&final_setup_step)
    .bind(&id)
    .execute(service.db())
    .await
    .map_err(|e| format!("Failed to update profile: {}", e))?;
    
    let profile_row = sqlx::query("SELECT * FROM profile WHERE id = ?1")
    .bind(&id)
    .fetch_one(service.db())
    .await
    .map_err(|e| format!("Failed to get updated profile: {}", e))?;
    
    let profile = Profile {
        id: profile_row.get("id"),
        name: profile_row.get("name"),
        user_id: profile_row.get("user_id"),
        avatar_id: profile_row.get("avatar_id"),
        avatar: None,
        theme_id: profile_row.get("theme_id"),
        setup_step: profile_row.get("setup_step"),
        active: profile_row.get("active"),
        last_id: profile_row.get("last_id"),
        created_at: profile_row.get("created_at"),
        settings: None,
        theme: None,
    };
    
    // Still emit the full profile state for the event
    profile.update_setup_step(service.db()).await?;
    
    if let Ok(Some(profile_state)) = load_profile_with_settings_and_theme(&service).await {
        window.emit("profile-updated", &ProfileResponse {
            data: Some(profile_state),
            is_loading: false,
        })
            .map_err(|e| format!("Failed to emit event: {}", e))?;
    }
    
    Ok(ProfileResponse {
        data: Some(profile),
        is_loading: false,
    })
}
#[tauri::command]
pub async fn put_profile_settings(
    service: State<'_, ChatnipService>,
    window: Window,
    input: PutProfileSettingsInput,
) -> Result<ProfileResponse, String> {
    let profile = Profile::get_active(service.db()).await?
        .ok_or_else(|| "No active profile found".to_string())?;
    let profile_id = profile.id;
    let settings = input.settings;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"UPDATE profile_setting SET 
            email_content_threshold = ?1,
            navigation_width = ?2,
            navigation_collapsed = ?3,
            details_width = ?4,
            details_panel_open = ?5,
            message_timestamp_interval = ?6,
            time_format = ?7,
            window_controls_style = ?8,
            max_input_lines = ?9,
            updated_at = ?10
        WHERE profile_id = ?11"#
    )
    .bind(settings.email_content_threshold)
    .bind(settings.navigation_width)
    .bind(settings.navigation_collapsed)
    .bind(settings.details_width)
    .bind(settings.details_panel_open)
    .bind(settings.message_timestamp_interval)
    .bind(&settings.time_format)
    .bind(&settings.window_controls_style)
    .bind(settings.max_input_lines)
    .bind(&now)
    .bind(&profile_id)
    .execute(service.db())
    .await
    .map_err(|e| format!("Failed to update settings: {}", e))?;
    let profile_data = load_profile_with_settings_and_theme(&service).await?
        .ok_or_else(|| "Profile not found after update".to_string())?;
    window.emit("profile-updated", &ProfileResponse {
        data: Some(profile_data.clone()),
        is_loading: false,
    })
        .map_err(|e| format!("Failed to emit event: {}", e))?;
    Ok(ProfileResponse {
        data: Some(profile_data),
        is_loading: false,
    })
}
#[tauri::command]
pub async fn put_profile_theme(
    service: State<'_, ChatnipService>,
    window: Window,
    input: PutProfileThemeInput,
) -> Result<ProfileResponse, String> {
    let profile = Profile::get_active(service.db()).await?
        .ok_or_else(|| "No active profile found".to_string())?;
    let profile_id = profile.id;
    let theme_id = input.theme_id;
    let theme = input.theme;
    let now = Utc::now().to_rfc3339();
    
    if theme.is_active {
        sqlx::query("UPDATE profile_theme SET is_active = 0 WHERE profile_id = ?1")
            .bind(&profile_id)
            .execute(service.db())
            .await
            .map_err(|e| format!("Failed to deactivate themes: {}", e))?;
    }
    sqlx::query(
        r#"INSERT OR REPLACE INTO profile_theme (
            id, profile_id, theme_name, is_active,
            primary_50, primary_100, primary_200, primary_300, primary_400, 
            primary_500, primary_600, primary_700, primary_800, primary_900, primary_950,
            secondary_50, secondary_100, secondary_200, secondary_300, secondary_400,
            secondary_500, secondary_600, secondary_700, secondary_800, secondary_900, secondary_950,
            accent_50, accent_100, accent_200, accent_300, accent_400,
            accent_500, accent_600, accent_700, accent_800, accent_900, accent_950,
            background_50, background_100, background_200, background_300, background_400,
            background_500, background_600, background_700, background_800, background_900, background_950,
            content_50, content_100, content_200, content_300, content_400,
            content_500, content_600, content_700, content_800, content_900, content_950,
            control_50, control_100, control_200, control_300, control_400,
            control_500, control_600, control_700, control_800, control_900, control_950,
            error_50, error_100, error_200, error_300, error_400,
            error_500, error_600, error_700, error_800, error_900, error_950,
            warn_50, warn_100, warn_200, warn_300, warn_400,
            warn_500, warn_600, warn_700, warn_800, warn_900, warn_950,
            success_50, success_100, success_200, success_300, success_400,
            success_500, success_600, success_700, success_800, success_900, success_950,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49, ?50, ?51, ?52, ?53, ?54, ?55, ?56, ?57, ?58, ?59, ?60, ?61, ?62, ?63, ?64, ?65, ?66, ?67, ?68, ?69, ?70, ?71, ?72, ?73, ?74, ?75, ?76, ?77, ?78, ?79, ?80, ?81, ?82, ?83, ?84, ?85, ?86, ?87, ?88, ?89, ?90, ?91, ?92, ?93, ?94, ?95, ?96, ?97, ?98, ?99, ?100, ?101, ?102, ?103, ?104, ?105)"#
    )
    .bind(&theme_id)
    .bind(&profile_id)
    .bind(&theme.theme_name)
    .bind(&theme.is_active)
    .bind(&theme.primary_50)
    .bind(&theme.primary_100)
    .bind(&theme.primary_200)
    .bind(&theme.primary_300)
    .bind(&theme.primary_400)
    .bind(&theme.primary_500)
    .bind(&theme.primary_600)
    .bind(&theme.primary_700)
    .bind(&theme.primary_800)
    .bind(&theme.primary_900)
    .bind(&theme.primary_950)
    .bind(&theme.secondary_50)
    .bind(&theme.secondary_100)
    .bind(&theme.secondary_200)
    .bind(&theme.secondary_300)
    .bind(&theme.secondary_400)
    .bind(&theme.secondary_500)
    .bind(&theme.secondary_600)
    .bind(&theme.secondary_700)
    .bind(&theme.secondary_800)
    .bind(&theme.secondary_900)
    .bind(&theme.secondary_950)
    .bind(&theme.accent_50)
    .bind(&theme.accent_100)
    .bind(&theme.accent_200)
    .bind(&theme.accent_300)
    .bind(&theme.accent_400)
    .bind(&theme.accent_500)
    .bind(&theme.accent_600)
    .bind(&theme.accent_700)
    .bind(&theme.accent_800)
    .bind(&theme.accent_900)
    .bind(&theme.accent_950)
    .bind(&theme.background_50)
    .bind(&theme.background_100)
    .bind(&theme.background_200)
    .bind(&theme.background_300)
    .bind(&theme.background_400)
    .bind(&theme.background_500)
    .bind(&theme.background_600)
    .bind(&theme.background_700)
    .bind(&theme.background_800)
    .bind(&theme.background_900)
    .bind(&theme.background_950)
    .bind(&theme.content_50)
    .bind(&theme.content_100)
    .bind(&theme.content_200)
    .bind(&theme.content_300)
    .bind(&theme.content_400)
    .bind(&theme.content_500)
    .bind(&theme.content_600)
    .bind(&theme.content_700)
    .bind(&theme.content_800)
    .bind(&theme.content_900)
    .bind(&theme.content_950)
    .bind(&theme.control_50)
    .bind(&theme.control_100)
    .bind(&theme.control_200)
    .bind(&theme.control_300)
    .bind(&theme.control_400)
    .bind(&theme.control_500)
    .bind(&theme.control_600)
    .bind(&theme.control_700)
    .bind(&theme.control_800)
    .bind(&theme.control_900)
    .bind(&theme.control_950)
    .bind(&theme.error_50)
    .bind(&theme.error_100)
    .bind(&theme.error_200)
    .bind(&theme.error_300)
    .bind(&theme.error_400)
    .bind(&theme.error_500)
    .bind(&theme.error_600)
    .bind(&theme.error_700)
    .bind(&theme.error_800)
    .bind(&theme.error_900)
    .bind(&theme.error_950)
    .bind(&theme.warn_50)
    .bind(&theme.warn_100)
    .bind(&theme.warn_200)
    .bind(&theme.warn_300)
    .bind(&theme.warn_400)
    .bind(&theme.warn_500)
    .bind(&theme.warn_600)
    .bind(&theme.warn_700)
    .bind(&theme.warn_800)
    .bind(&theme.warn_900)
    .bind(&theme.warn_950)
    .bind(&theme.success_50)
    .bind(&theme.success_100)
    .bind(&theme.success_200)
    .bind(&theme.success_300)
    .bind(&theme.success_400)
    .bind(&theme.success_500)
    .bind(&theme.success_600)
    .bind(&theme.success_700)
    .bind(&theme.success_800)
    .bind(&theme.success_900)
    .bind(&theme.success_950)
    .bind(&now)
    .bind(&now)
    .execute(service.db())
    .await
    .map_err(|e| format!("Failed to update theme: {}", e))?;
    sqlx::query("UPDATE profile SET theme_id = ?1 WHERE id = ?2")
        .bind(&theme_id)
        .bind(&profile_id)
        .execute(service.db())
        .await
        .map_err(|e| format!("Failed to update profile theme reference: {}", e))?;
    let profile_data = load_profile_with_settings_and_theme(&service).await?
        .ok_or_else(|| "Profile not found after theme update".to_string())?;
    window.emit("profile-updated", &profile_data)
        .map_err(|e| format!("Failed to emit event: {}", e))?;
    
    Ok(ProfileResponse {
        data: Some(profile_data),
        is_loading: false,
    })
}
#[tauri::command]
pub async fn post_profile_theme(
    service: State<'_, ChatnipService>,
    window: Window,
    input: PostProfileThemeInput,
) -> Result<ProfileResponse, String> {
    let profile = Profile::get_active(service.db()).await?
        .ok_or_else(|| "No active profile found".to_string())?;
    let profile_id = profile.id;
    let theme = input.theme;
    let now = Utc::now().to_rfc3339();
    let theme_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"INSERT INTO profile_theme (
            id, profile_id, theme_name, is_active,
            primary_50, primary_100, primary_200, primary_300, primary_400, 
            primary_500, primary_600, primary_700, primary_800, primary_900, primary_950,
            secondary_50, secondary_100, secondary_200, secondary_300, secondary_400,
            secondary_500, secondary_600, secondary_700, secondary_800, secondary_900, secondary_950,
            accent_50, accent_100, accent_200, accent_300, accent_400,
            accent_500, accent_600, accent_700, accent_800, accent_900, accent_950,
            background_50, background_100, background_200, background_300, background_400,
            background_500, background_600, background_700, background_800, background_900, background_950,
            content_50, content_100, content_200, content_300, content_400,
            content_500, content_600, content_700, content_800, content_900, content_950,
            control_50, control_100, control_200, control_300, control_400,
            control_500, control_600, control_700, control_800, control_900, control_950,
            error_50, error_100, error_200, error_300, error_400,
            error_500, error_600, error_700, error_800, error_900, error_950,
            warn_50, warn_100, warn_200, warn_300, warn_400,
            warn_500, warn_600, warn_700, warn_800, warn_900, warn_950,
            success_50, success_100, success_200, success_300, success_400,
            success_500, success_600, success_700, success_800, success_900, success_950,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49, ?50, ?51, ?52, ?53, ?54, ?55, ?56, ?57, ?58, ?59, ?60, ?61, ?62, ?63, ?64, ?65, ?66, ?67, ?68, ?69, ?70, ?71, ?72, ?73, ?74, ?75, ?76, ?77, ?78, ?79, ?80, ?81, ?82, ?83, ?84, ?85, ?86, ?87, ?88, ?89, ?90, ?91, ?92, ?93, ?94, ?95, ?96, ?97, ?98, ?99, ?100, ?101, ?102, ?103, ?104, ?105)"#
    )
    .bind(&theme_id)
    .bind(&profile_id)
    .bind(&theme.theme_name)
    .bind(&theme.is_active)
    .bind(&theme.primary_50)
    .bind(&theme.primary_100)
    .bind(&theme.primary_200)
    .bind(&theme.primary_300)
    .bind(&theme.primary_400)
    .bind(&theme.primary_500)
    .bind(&theme.primary_600)
    .bind(&theme.primary_700)
    .bind(&theme.primary_800)
    .bind(&theme.primary_900)
    .bind(&theme.primary_950)
    .bind(&theme.secondary_50)
    .bind(&theme.secondary_100)
    .bind(&theme.secondary_200)
    .bind(&theme.secondary_300)
    .bind(&theme.secondary_400)
    .bind(&theme.secondary_500)
    .bind(&theme.secondary_600)
    .bind(&theme.secondary_700)
    .bind(&theme.secondary_800)
    .bind(&theme.secondary_900)
    .bind(&theme.secondary_950)
    .bind(&theme.accent_50)
    .bind(&theme.accent_100)
    .bind(&theme.accent_200)
    .bind(&theme.accent_300)
    .bind(&theme.accent_400)
    .bind(&theme.accent_500)
    .bind(&theme.accent_600)
    .bind(&theme.accent_700)
    .bind(&theme.accent_800)
    .bind(&theme.accent_900)
    .bind(&theme.accent_950)
    .bind(&theme.background_50)
    .bind(&theme.background_100)
    .bind(&theme.background_200)
    .bind(&theme.background_300)
    .bind(&theme.background_400)
    .bind(&theme.background_500)
    .bind(&theme.background_600)
    .bind(&theme.background_700)
    .bind(&theme.background_800)
    .bind(&theme.background_900)
    .bind(&theme.background_950)
    .bind(&theme.content_50)
    .bind(&theme.content_100)
    .bind(&theme.content_200)
    .bind(&theme.content_300)
    .bind(&theme.content_400)
    .bind(&theme.content_500)
    .bind(&theme.content_600)
    .bind(&theme.content_700)
    .bind(&theme.content_800)
    .bind(&theme.content_900)
    .bind(&theme.content_950)
    .bind(&theme.control_50)
    .bind(&theme.control_100)
    .bind(&theme.control_200)
    .bind(&theme.control_300)
    .bind(&theme.control_400)
    .bind(&theme.control_500)
    .bind(&theme.control_600)
    .bind(&theme.control_700)
    .bind(&theme.control_800)
    .bind(&theme.control_900)
    .bind(&theme.control_950)
    .bind(&theme.error_50)
    .bind(&theme.error_100)
    .bind(&theme.error_200)
    .bind(&theme.error_300)
    .bind(&theme.error_400)
    .bind(&theme.error_500)
    .bind(&theme.error_600)
    .bind(&theme.error_700)
    .bind(&theme.error_800)
    .bind(&theme.error_900)
    .bind(&theme.error_950)
    .bind(&theme.warn_50)
    .bind(&theme.warn_100)
    .bind(&theme.warn_200)
    .bind(&theme.warn_300)
    .bind(&theme.warn_400)
    .bind(&theme.warn_500)
    .bind(&theme.warn_600)
    .bind(&theme.warn_700)
    .bind(&theme.warn_800)
    .bind(&theme.warn_900)
    .bind(&theme.warn_950)
    .bind(&theme.success_50)
    .bind(&theme.success_100)
    .bind(&theme.success_200)
    .bind(&theme.success_300)
    .bind(&theme.success_400)
    .bind(&theme.success_500)
    .bind(&theme.success_600)
    .bind(&theme.success_700)
    .bind(&theme.success_800)
    .bind(&theme.success_900)
    .bind(&theme.success_950)
    .bind(&now)
    .bind(&now)
    .execute(service.db())
    .await
    .map_err(|e| format!("Failed to create theme: {}", e))?;
    let _created_theme = ProfileTheme {
        id: theme_id.clone(),
        profile_id: profile_id.clone(),
        theme_name: theme.theme_name,
        is_active: false,
        primary_50: theme.primary_50,
        primary_100: theme.primary_100,
        primary_200: theme.primary_200,
        primary_300: theme.primary_300,
        primary_400: theme.primary_400,
        primary_500: theme.primary_500,
        primary_600: theme.primary_600,
        primary_700: theme.primary_700,
        primary_800: theme.primary_800,
        primary_900: theme.primary_900,
        primary_950: theme.primary_950,
        secondary_50: theme.secondary_50,
        secondary_100: theme.secondary_100,
        secondary_200: theme.secondary_200,
        secondary_300: theme.secondary_300,
        secondary_400: theme.secondary_400,
        secondary_500: theme.secondary_500,
        secondary_600: theme.secondary_600,
        secondary_700: theme.secondary_700,
        secondary_800: theme.secondary_800,
        secondary_900: theme.secondary_900,
        secondary_950: theme.secondary_950,
        accent_50: theme.accent_50,
        accent_100: theme.accent_100,
        accent_200: theme.accent_200,
        accent_300: theme.accent_300,
        accent_400: theme.accent_400,
        accent_500: theme.accent_500,
        accent_600: theme.accent_600,
        accent_700: theme.accent_700,
        accent_800: theme.accent_800,
        accent_900: theme.accent_900,
        accent_950: theme.accent_950,
        background_50: theme.background_50,
        background_100: theme.background_100,
        background_200: theme.background_200,
        background_300: theme.background_300,
        background_400: theme.background_400,
        background_500: theme.background_500,
        background_600: theme.background_600,
        background_700: theme.background_700,
        background_800: theme.background_800,
        background_900: theme.background_900,
        background_950: theme.background_950,
        content_50: theme.content_50,
        content_100: theme.content_100,
        content_200: theme.content_200,
        content_300: theme.content_300,
        content_400: theme.content_400,
        content_500: theme.content_500,
        content_600: theme.content_600,
        content_700: theme.content_700,
        content_800: theme.content_800,
        content_900: theme.content_900,
        content_950: theme.content_950,
        control_50: theme.control_50,
        control_100: theme.control_100,
        control_200: theme.control_200,
        control_300: theme.control_300,
        control_400: theme.control_400,
        control_500: theme.control_500,
        control_600: theme.control_600,
        control_700: theme.control_700,
        control_800: theme.control_800,
        control_900: theme.control_900,
        control_950: theme.control_950,
        error_50: theme.error_50,
        error_100: theme.error_100,
        error_200: theme.error_200,
        error_300: theme.error_300,
        error_400: theme.error_400,
        error_500: theme.error_500,
        error_600: theme.error_600,
        error_700: theme.error_700,
        error_800: theme.error_800,
        error_900: theme.error_900,
        error_950: theme.error_950,
        warn_50: theme.warn_50,
        warn_100: theme.warn_100,
        warn_200: theme.warn_200,
        warn_300: theme.warn_300,
        warn_400: theme.warn_400,
        warn_500: theme.warn_500,
        warn_600: theme.warn_600,
        warn_700: theme.warn_700,
        warn_800: theme.warn_800,
        warn_900: theme.warn_900,
        warn_950: theme.warn_950,
        success_50: theme.success_50,
        success_100: theme.success_100,
        success_200: theme.success_200,
        success_300: theme.success_300,
        success_400: theme.success_400,
        success_500: theme.success_500,
        success_600: theme.success_600,
        success_700: theme.success_700,
        success_800: theme.success_800,
        success_900: theme.success_900,
        success_950: theme.success_950,
        created_at: now.clone(),
        updated_at: now,
    };
    
    if theme.is_active {
        sqlx::query("UPDATE profile_theme SET is_active = false WHERE profile_id = ?1 AND id != ?2")
            .bind(&profile_id)
            .bind(&theme_id)
            .execute(service.db())
            .await
            .map_err(|e| format!("Failed to deactivate other themes: {}", e))?;
    }
    
    sqlx::query("UPDATE profile SET theme_id = ?1 WHERE id = ?2")
        .bind(&theme_id)
        .bind(&profile_id)
        .execute(service.db())
        .await
        .map_err(|e| format!("Failed to update profile theme_id: {}", e))?;
    
    if let Some(profile) = Profile::get_active(service.db()).await? {
        profile.update_setup_step(service.db()).await?;
    }
    
    let profile_data = load_profile_with_settings_and_theme(&service).await?
        .ok_or_else(|| "Profile not found after theme creation".to_string())?;
    window.emit("profile-updated", &profile_data)
        .map_err(|e| format!("Failed to emit event: {}", e))?;
    Ok(ProfileResponse {
        data: Some(profile_data),
        is_loading: false,
    })
}
#[tauri::command]
pub async fn post_profile_avatar(
    service: State<'_, ChatnipService>,
    window: Window,
    app_handle: AppHandle,
    input: PostProfileAvatarInput,
) -> Result<String, String> {
    let profile = Profile::get_active(service.db()).await?
        .ok_or_else(|| "No active profile found".to_string())?;
    let profile_id = profile.id;
    let avatar_data = input.avatar_data;
    
    let app_dir = app_handle.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let assets_dir = app_dir.join("assets").join("avatars");
    std::fs::create_dir_all(&assets_dir)
        .map_err(|e| format!("Failed to create assets directory: {}", e))?;
    
    let avatar_id = Uuid::new_v4().to_string();
    
    let (mime_type, file_extension, image_data) = if avatar_data.starts_with("data:") {
        let parts: Vec<&str> = avatar_data.split(',').collect();
        if parts.len() != 2 {
            return Err("Invalid data URL format".to_string());
        }
        
        let metadata = parts[0];
        let mime_type = metadata
            .trim_start_matches("data:")
            .trim_end_matches(";base64")
            .to_string();
        
        let extension = match mime_type.as_str() {
            "image/png" => "png",
            "image/jpeg" | "image/jpg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "image/svg+xml" => "svg",
            _ => "png",
        };
        
        use base64::{Engine as _, engine::general_purpose};
        let decoded = general_purpose::STANDARD.decode(parts[1])
            .map_err(|e| format!("Failed to decode base64 image: {}", e))?;
        
        (mime_type, extension.to_string(), decoded)
    } else if std::path::Path::new(&avatar_data).exists() {
        let path = std::path::Path::new(&avatar_data);
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("png")
            .to_lowercase();
        
        let mime_type = match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            _ => "image/png",
        }.to_string();
        
        let data = std::fs::read(&avatar_data)
            .map_err(|e| format!("Failed to read image file: {}", e))?;
        
        (mime_type, extension, data)
    } else {
        return Err("Invalid avatar data: neither base64 nor valid file path".to_string());
    };
    
    let filename = format!("{}.{}", avatar_id, file_extension);
    let file_path = assets_dir.join(&filename);
    
    std::fs::write(&file_path, image_data)
        .map_err(|e| format!("Failed to save avatar image: {}", e))?;
    
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO assets (id, profile_id, name, url, type, mime_type, path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
    )
    .bind(&avatar_id)
    .bind(&profile_id)
    .bind(&filename)
    .bind(&filename)
    .bind("avatar")
    .bind(&mime_type)
    .bind(format!("assets/avatars/{}", filename))
    .bind(&now)
    .bind(&now)
    .execute(service.db())
    .await
    .map_err(|e| format!("Failed to create asset record: {}", e))?;
    
    sqlx::query("UPDATE profile SET avatar_id = ?1 WHERE id = ?2")
        .bind(&avatar_id)
        .bind(&profile_id)
        .execute(service.db())
        .await
        .map_err(|e| format!("Failed to update profile avatar: {}", e))?;
    
    let profile_data = load_profile_with_settings_and_theme(&service).await?
        .ok_or_else(|| "Profile not found after avatar update".to_string())?;
    window.emit("profile-updated", &profile_data)
        .map_err(|e| format!("Failed to emit event: {}", e))?;
    Ok(avatar_id)
}
#[tauri::command]
pub async fn put_profile_avatar(
    service: State<'_, ChatnipService>,
    window: Window,
    app_handle: AppHandle,
    input: PutProfileAvatarInput,
) -> Result<ProfileResponse, String> {
    let profile = Profile::get_active(service.db()).await?
        .ok_or_else(|| "No active profile found".to_string())?;
    let profile_id = profile.id;
    let avatar_data = input.avatar_data;
    let db = service.db();
    let app_dir = app_handle.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let assets_dir = app_dir.join("assets").join("avatars");
    std::fs::create_dir_all(&assets_dir)
        .map_err(|e| format!("Failed to create assets directory: {}", e))?;
    let asset_id = Uuid::new_v4().to_string();
    
    let (mime_type, file_extension, image_data) = if avatar_data.starts_with("data:") {
        let parts: Vec<&str> = avatar_data.split(',').collect();
        if parts.len() != 2 {
            return Err("Invalid data URL format".to_string());
        }
        
        let metadata = parts[0];
        let mime_type = metadata
            .trim_start_matches("data:")
            .trim_end_matches(";base64")
            .to_string();
        
        let extension = match mime_type.as_str() {
            "image/png" => "png",
            "image/jpeg" | "image/jpg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "image/svg+xml" => "svg",
            _ => "png",
        };
        
        use base64::{Engine as _, engine::general_purpose};
        let decoded = general_purpose::STANDARD.decode(parts[1])
            .map_err(|e| format!("Failed to decode base64 image: {}", e))?;
        
        (mime_type, extension.to_string(), decoded)
    } else if std::path::Path::new(&avatar_data).exists() {
        let path = std::path::Path::new(&avatar_data);
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("png")
            .to_lowercase();
        
        let mime_type = match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            _ => "image/png",
        }.to_string();
        
        let data = std::fs::read(&avatar_data)
            .map_err(|e| format!("Failed to read image file: {}", e))?;
        
        (mime_type, extension, data)
    } else {
        return Err("Invalid avatar data: neither base64 nor valid file path".to_string());
    };
    
    let filename = format!("{}.{}", asset_id, file_extension);
    let file_path = assets_dir.join(&filename);
    
    std::fs::write(&file_path, image_data)
        .map_err(|e| format!("Failed to save avatar image: {}", e))?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO assets (id, profile_id, name, url, type, mime_type, path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
    )
    .bind(&asset_id)
    .bind(&profile_id)
    .bind(&asset_id)
    .bind(&filename)
    .bind(&filename)
    .bind("avatar")
    .bind(&mime_type)
    .bind(format!("assets/avatars/{}", filename))
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to create asset record: {}", e))?;
    sqlx::query("UPDATE profile SET avatar_id = ?1 WHERE id = ?2")
        .bind(&asset_id)
        .bind(&profile_id)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to update profile avatar: {}", e))?;
    let profile_data = load_profile_with_settings_and_theme(&service).await?
        .ok_or_else(|| "Profile not found after avatar update".to_string())?;
    window.emit("profile-updated", &profile_data)
        .map_err(|e| format!("Failed to emit event: {}", e))?;
    Ok(ProfileResponse {
        data: Some(profile_data),
        is_loading: false,
    })
}
#[tauri::command]
pub async fn get_conversations(
    service: State<'_, ChatnipService>,
) -> Result<ConversationsResponse, String> {
    use crate::loading_state::loading_key_conversations;
    
    log_command!("get_conversations");
    
    let is_loading = service.loading_state().is_loading(loading_key_conversations());
    
    // Get active profile
    let profile = match Profile::get_active(service.db()).await? {
        Some(p) => {
            println!("  ✅ Found active profile: {}", p.id);
            p
        },
        None => {
            println!("  ⚠️ No active profile found, returning empty conversations");
            return Ok(ConversationsResponse {
                data: vec![],
                is_loading: false,
            });
        }
    };
    
    // Get all providers for this profile
    let provider_rows = sqlx::query("SELECT id FROM provider WHERE profile_id = ?1")
        .bind(&profile.id)
        .fetch_all(service.db())
        .await
        .map_err(|e| format!("Failed to get providers: {}", e))?;
    
    if provider_rows.is_empty() {
        // No providers, return empty conversations
        return Ok(ConversationsResponse {
            data: vec![],
            is_loading,
        });
    }
    
    // Get all conversations for these providers through the junction table
    let provider_ids: Vec<String> = provider_rows.iter().map(|row| row.get("id")).collect();
    let placeholders = provider_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query = format!(
        "SELECT DISTINCT c.* FROM conversation c 
         JOIN conversation_provider cp ON c.id = cp.conversation_id 
         WHERE cp.provider_id IN ({}) 
         ORDER BY c.last_activity DESC", 
        placeholders
    );
    
    let mut query_builder = sqlx::query(&query);
    for provider_id in &provider_ids {
        query_builder = query_builder.bind(provider_id);
    }
    
    let rows = query_builder
        .fetch_all(service.db())
        .await
        .map_err(|e| format!("Failed to get conversations: {}", e))?;
    
    let mut conversations: Vec<Conversation> = Vec::new();
    for row in rows {
        let mut conversation = Conversation::from_row(row)?;
        
        // Update summary
        conversation.update_summary(service.db()).await.ok();
        
        conversation.load_providers(service.db()).await?;
        
        conversation.tags = crate::models::ObjectTag::get_conversation(service.db(), &conversation.id).await.unwrap_or_default();
        
        // Load provider themes
        for provider in &mut conversation.providers {
            if let Ok(Some(theme)) = crate::utils::theme_generator::get_provider_theme(service.db(), &provider.id).await {
                provider.theme = Some(theme);
            }
        }
        
        if let Ok(Some(theme_row)) = sqlx::query(
            r#"SELECT pt.* FROM profile_theme pt
               JOIN conversation_provider cp ON pt.id = cp.theme_id
               WHERE cp.conversation_id = ?1 LIMIT 1"#
        )
        .bind(&conversation.id)
        .fetch_optional(service.db())
        .await {
            conversation.theme = Some(ProfileTheme {
                id: theme_row.get("id"),
                profile_id: theme_row.get("profile_id"),
                theme_name: theme_row.get("theme_name"),
                is_active: theme_row.get("is_active"),
                primary_50: theme_row.get("primary_50"),
                primary_100: theme_row.get("primary_100"),
                primary_200: theme_row.get("primary_200"),
                primary_300: theme_row.get("primary_300"),
                primary_400: theme_row.get("primary_400"),
                primary_500: theme_row.get("primary_500"),
                primary_600: theme_row.get("primary_600"),
                primary_700: theme_row.get("primary_700"),
                primary_800: theme_row.get("primary_800"),
                primary_900: theme_row.get("primary_900"),
                primary_950: theme_row.get("primary_950"),
                secondary_50: theme_row.get("secondary_50"),
                secondary_100: theme_row.get("secondary_100"),
                secondary_200: theme_row.get("secondary_200"),
                secondary_300: theme_row.get("secondary_300"),
                secondary_400: theme_row.get("secondary_400"),
                secondary_500: theme_row.get("secondary_500"),
                secondary_600: theme_row.get("secondary_600"),
                secondary_700: theme_row.get("secondary_700"),
                secondary_800: theme_row.get("secondary_800"),
                secondary_900: theme_row.get("secondary_900"),
                secondary_950: theme_row.get("secondary_950"),
                accent_50: theme_row.get("accent_50"),
                accent_100: theme_row.get("accent_100"),
                accent_200: theme_row.get("accent_200"),
                accent_300: theme_row.get("accent_300"),
                accent_400: theme_row.get("accent_400"),
                accent_500: theme_row.get("accent_500"),
                accent_600: theme_row.get("accent_600"),
                accent_700: theme_row.get("accent_700"),
                accent_800: theme_row.get("accent_800"),
                accent_900: theme_row.get("accent_900"),
                accent_950: theme_row.get("accent_950"),
                background_50: theme_row.get("background_50"),
                background_100: theme_row.get("background_100"),
                background_200: theme_row.get("background_200"),
                background_300: theme_row.get("background_300"),
                background_400: theme_row.get("background_400"),
                background_500: theme_row.get("background_500"),
                background_600: theme_row.get("background_600"),
                background_700: theme_row.get("background_700"),
                background_800: theme_row.get("background_800"),
                background_900: theme_row.get("background_900"),
                background_950: theme_row.get("background_950"),
                content_50: theme_row.get("content_50"),
                content_100: theme_row.get("content_100"),
                content_200: theme_row.get("content_200"),
                content_300: theme_row.get("content_300"),
                content_400: theme_row.get("content_400"),
                content_500: theme_row.get("content_500"),
                content_600: theme_row.get("content_600"),
                content_700: theme_row.get("content_700"),
                content_800: theme_row.get("content_800"),
                content_900: theme_row.get("content_900"),
                content_950: theme_row.get("content_950"),
                control_50: theme_row.get("control_50"),
                control_100: theme_row.get("control_100"),
                control_200: theme_row.get("control_200"),
                control_300: theme_row.get("control_300"),
                control_400: theme_row.get("control_400"),
                control_500: theme_row.get("control_500"),
                control_600: theme_row.get("control_600"),
                control_700: theme_row.get("control_700"),
                control_800: theme_row.get("control_800"),
                control_900: theme_row.get("control_900"),
                control_950: theme_row.get("control_950"),
                error_50: theme_row.get("error_50"),
                error_100: theme_row.get("error_100"),
                error_200: theme_row.get("error_200"),
                error_300: theme_row.get("error_300"),
                error_400: theme_row.get("error_400"),
                error_500: theme_row.get("error_500"),
                error_600: theme_row.get("error_600"),
                error_700: theme_row.get("error_700"),
                error_800: theme_row.get("error_800"),
                error_900: theme_row.get("error_900"),
                error_950: theme_row.get("error_950"),
                warn_50: theme_row.get("warn_50"),
                warn_100: theme_row.get("warn_100"),
                warn_200: theme_row.get("warn_200"),
                warn_300: theme_row.get("warn_300"),
                warn_400: theme_row.get("warn_400"),
                warn_500: theme_row.get("warn_500"),
                warn_600: theme_row.get("warn_600"),
                warn_700: theme_row.get("warn_700"),
                warn_800: theme_row.get("warn_800"),
                warn_900: theme_row.get("warn_900"),
                warn_950: theme_row.get("warn_950"),
                success_50: theme_row.get("success_50"),
                success_100: theme_row.get("success_100"),
                success_200: theme_row.get("success_200"),
                success_300: theme_row.get("success_300"),
                success_400: theme_row.get("success_400"),
                success_500: theme_row.get("success_500"),
                success_600: theme_row.get("success_600"),
                success_700: theme_row.get("success_700"),
                success_800: theme_row.get("success_800"),
                success_900: theme_row.get("success_900"),
                success_950: theme_row.get("success_950"),
                created_at: theme_row.get("created_at"),
                updated_at: theme_row.get("updated_at"),
            });
        }
        
        // Generate name from members if name is empty or "Unnamed Chat"
        if conversation.name.is_empty() || conversation.name == "null" || conversation.name == "Unnamed Chat" {
            let members = conversation.get_members(service.db()).await.unwrap_or_default();
            
            let member_names: Vec<String> = members
                .iter()
                .filter(|m| profile.user_id.as_ref().map_or(true, |uid| m.id != *uid))
                .filter_map(|m| m.display_name.clone().or_else(|| m.email.clone()))
                .collect();
            
            if !member_names.is_empty() {
                conversation.name = member_names.join(", ");
            }
        }
        
        conversation.load_display(service.db()).await.ok();
        conversations.push(conversation);
    }
    
    Ok(ConversationsResponse {
        data: conversations,
        is_loading,
    })
}
#[tauri::command]
pub async fn get_conversation(
    service: State<'_, ChatnipService>,
    window: Window,
    input: GetConversationInput,
) -> Result<(), String> {
    use crate::services::{MessageService, MessageLoadOptions};
    
    let conversation_id = input.conversation_id;
    
    // Load basic conversation info
    let row = sqlx::query(
        "SELECT * FROM conversation WHERE id = ?1"
    )
    .bind(&conversation_id)
    .fetch_optional(service.db())
    .await
    .map_err(|e| format!("Failed to get conversation: {}", e))?
    .ok_or_else(|| "Conversation not found".to_string())?;
    
    let mut conversation = Conversation::from_row(row)?;
    conversation.update_summary(service.db()).await.ok();
    
    // Emit initial conversation data
    window.emit("conversation-loaded", &conversation)
        .map_err(|e| format!("Failed to emit conversation-loaded event: {}", e))?;
    
    // Load messages with all associated data asynchronously
    let message_service = MessageService::new(service.db().clone());
    let load_options = MessageLoadOptions {
        load_reactions: true,
        load_assets: true,
        load_sender_details: true,
        limit: Some(50),
        offset: None,
    };
    
    let enhanced_messages = message_service
        .load_conversation_messages(&conversation_id, load_options)
        .await
        .map_err(|e| format!("Failed to load enhanced messages: {}", e))?;
    
    // Emit messages data
    window.emit("conversation-messages-loaded", &enhanced_messages)
        .map_err(|e| format!("Failed to emit conversation-messages-loaded event: {}", e))?;
    
    // Optionally sync fresh data from Microsoft Graph if provider is connected
    if let Ok(providers) = sqlx::query("SELECT * FROM provider WHERE is_enabled = 1")
        .fetch_all(service.db())
        .await
    {
        for provider_row in providers {
            let provider_type: String = provider_row.get("provider_type");
            if provider_type == "microsoft" {
                let message_service_clone = MessageService::new(service.db().clone());
                let window_clone = window.clone();
                let conversation_id_clone = conversation_id.clone();
                
                if let Err(e) = sync_microsoft_conversation_assets(
                    message_service_clone,
                    window_clone,
                    conversation_id_clone
                ).await {
                    eprintln!("❌ Failed to sync Microsoft conversation assets: {}", e);
                }
            }
        }
    }
    
    Ok(())
}
#[tauri::command]
pub async fn post_message_reply(
    service: State<'_, ChatnipService>,
    window: Window,
    input: crate::services::outbound_service::PostMessageReplyInput,
    hold: Option<crate::services::outbound_service::HoldRequest>,
) -> Result<crate::services::outbound_service::Sent, String> {
    log_command!("post_message_reply", "message_id={}", input.message_id);
    let app_data = window.app_handle().path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let sent = crate::services::outbound_service::put_outbound(service.db(), &app_data, crate::services::outbound_service::Outbound::Reply(input), hold, Utc::now()).await?;
    crate::services::outbound_service::started(window.app_handle(), &sent);
    let message = sent.into_iter().next().ok_or_else(|| "Nothing was sent".to_string())?;
    let messages_response = get_current_messages(service.clone(), window.clone()).await?;
    window.emit("messages-updated", &messages_response).map_err(|e| format!("Failed to emit messages update: {}", e))?;
    let conversations = get_conversations(service.clone()).await?;
    window.emit("conversations-updated", &conversations).map_err(|e| format!("Failed to emit conversations update: {}", e))?;
    Ok(message)
}

#[tauri::command]
pub async fn post_message_forward(
    service: State<'_, ChatnipService>,
    window: Window,
    input: crate::services::outbound_service::PostMessageForwardInput,
    hold: Option<crate::services::outbound_service::HoldRequest>,
) -> Result<Vec<crate::services::outbound_service::Sent>, String> {
    log_command!("post_message_forward", "messages={} targets={}", input.message_ids.len(), input.targets.len());
    let app_data = window.app_handle().path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let messages = crate::services::outbound_service::put_outbound(service.db(), &app_data, crate::services::outbound_service::Outbound::Forward(input), hold, Utc::now()).await?;
    crate::services::outbound_service::started(window.app_handle(), &messages);
    let messages_response = get_current_messages(service.clone(), window.clone()).await?;
    window.emit("messages-updated", &messages_response).map_err(|e| format!("Failed to emit messages update: {}", e))?;
    let conversations = get_conversations(service.clone()).await?;
    window.emit("conversations-updated", &conversations).map_err(|e| format!("Failed to emit conversations update: {}", e))?;
    Ok(messages)
}

#[tauri::command]
pub async fn get_message_quote(service: State<'_, ChatnipService>, message_id: String) -> Result<Option<Message>, String> {
    crate::services::outbound_service::quote_of(service.db(), &message_id).await
}

#[tauri::command]
pub async fn post_message(
    service: State<'_, ChatnipService>,
    window: Window,
    input: SendMessageInput,
    hold: Option<crate::services::outbound_service::HoldRequest>,
    delivery_mode: Option<String>,
) -> Result<crate::services::outbound_service::Sent, String> {
    log_command!("post_message", "conversation_id={}", input.conversation_id);
    let app_data = window.app_handle().path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
    let inputs = if delivery_mode.as_deref() == Some("all") {
        let providers: Vec<String> = sqlx::query_scalar("SELECT provider_id FROM conversation_provider WHERE conversation_id = ?1 ORDER BY is_primary DESC, created_at ASC")
            .bind(&input.conversation_id)
            .fetch_all(service.db())
            .await
            .map_err(|e| format!("Failed to read the accounts of the conversation: {}", e))?;
        providers.into_iter().map(|provider_id| SendMessageInput { provider_id: Some(provider_id), ..input.clone() }).collect()
    } else {
        vec![input]
    };
    let mut sent = Vec::new();
    for each in inputs {
        let delivered = match crate::services::outbound_service::put_outbound(service.db(), &app_data, crate::services::outbound_service::Outbound::Send(each.clone()), hold.clone(), Utc::now()).await {
            Ok(delivered) => delivered,
            Err(e) => {
                crate::services::history_service::record_failed_send(service.db(), Some(window.app_handle()), &each, &e).await.ok();
                return Err(e);
            }
        };
        for item in &delivered {
            if let crate::services::outbound_service::Sent::Message(message) = item {
                crate::services::history_service::record_sent(service.db(), Some(window.app_handle()), &each.conversation_id, Some(&message.id), &crate::services::history_service::snippet(&each.content), "").await.ok();
            }
        }
        crate::services::outbound_service::started(window.app_handle(), &delivered);
        sent.extend(delivered);
    }
    let message = sent.into_iter().next().ok_or_else(|| "Nothing was sent".to_string())?;
    let messages_response = get_current_messages(service.clone(), window.clone()).await?;
    window.emit("messages-updated", &messages_response)
        .map_err(|e| format!("Failed to emit messages update: {}", e))?;
    let conversations = get_conversations(service.clone()).await?;
    window.emit("conversations-updated", &conversations)
        .map_err(|e| format!("Failed to emit conversations update: {}", e))?;
    Ok(message)
}

#[tauri::command]
pub async fn get_outbound_holds(service: State<'_, ChatnipService>, conversation_id: Option<String>) -> Result<Vec<crate::models::outbound_hold::OutboundHold>, String> {
    use crate::models::outbound_hold::{OutboundHold, STATUS_DRAFT, STATUS_FAILED, STATUS_PENDING};
    OutboundHold::get_holds(service.db(), &[STATUS_PENDING, STATUS_DRAFT, STATUS_FAILED], conversation_id.as_deref()).await
}

#[tauri::command]
pub async fn delete_outbound_hold(service: State<'_, ChatnipService>, app: AppHandle, hold_id: String) -> Result<crate::models::outbound_hold::OutboundHold, String> {
    let held = crate::services::outbound_service::delete_hold(service.db(), &hold_id).await?;
    crate::services::notification_service::delete(&hold_id);
    app.emit(crate::services::outbound_service::HOLD_EVENT, &held).map_err(|e| format!("Failed to announce the stopped email: {}", e))?;
    Ok(held)
}

#[tauri::command]
pub async fn get_outbound_opened(service: State<'_, ChatnipService>) -> Result<Option<crate::models::outbound_hold::OutboundHold>, String> {
    crate::services::outbound_service::get_opened(service.db()).await
}

#[tauri::command]
pub async fn post_imap_connection(
    service: State<'_, ChatnipService>,
    window: Window,
    input: ConnectImapInput,
) -> Result<Provider, String> {
    log_command!("post_imap_connection", "email: {}", input.email);
    let config = crate::providers::imap::ImapConfig {
        email: input.email.trim().to_string(),
        display_name: input.display_name.unwrap_or_default().trim().to_string(),
        username: input.username.unwrap_or_default().trim().to_string(),
        imap_host: input.imap_host.trim().to_string(),
        imap_port: input.imap_port.unwrap_or(993),
        smtp_host: input.smtp_host.trim().to_string(),
        smtp_port: input.smtp_port.unwrap_or(587),
        allow_invalid_certs: false,
        aliases: input.aliases.unwrap_or_default().iter().map(|a| a.trim().to_lowercase()).filter(|a| a.contains('@')).collect(),
    };
    crate::services::connection_service::connect_imap(window.app_handle(), service.db(), config, &input.password).await
}

#[tauri::command]
pub async fn post_message_reaction(
    service: State<'_, ChatnipService>,
    window: Window,
    input: SendMessageReactionInput,
) -> Result<(), String> {
    let message_id = input.message_id;
    let reacted = crate::services::outbound_service::react(service.db(), &message_id, &input.reaction).await?;
    if let (true, Some(profile)) = (reacted.active, crate::models::Profile::get_active(service.db()).await?) {
        crate::models::content::ReactionUse::put(service.db(), &profile.id, &reacted.reaction).await?;
    }
    crate::services::action_service::on_reaction(window.app_handle().clone(), message_id.clone());
    // Emit messages-updated with fresh data
    let messages_response = get_current_messages(service.clone(), window.clone()).await?;
    window.emit("messages-updated", &messages_response)
        .map_err(|e| format!("Failed to emit messages update: {}", e))?;
    Ok(())
}
#[tauri::command]
pub async fn post_sync(
    service: State<'_, ChatnipService>,
    window: WebviewWindow,
) -> Result<ConversationsResponse, String> {
    use crate::providers::traits::ProviderTrait;
    use crate::providers::models::Provider;
    use crate::loading_state::loading_key_conversations;
    use std::sync::Arc;

    service.loading_state().start_loading(loading_key_conversations());

    let conversations_response = get_conversations(service.clone()).await?;

    let response = ConversationsResponse {
        data: conversations_response.data,
        is_loading: true,
    };

    let rows = sqlx::query("SELECT * FROM provider WHERE is_enabled = true")
        .fetch_all(service.db())
        .await
        .map_err(|e| format!("Failed to get enabled providers: {}", e))?;

    println!("🔍 Found {} enabled providers to sync", rows.len());

    let mut sync_count = 0;
    let mut errors = vec![];

    for row in rows {
        let provider = match Provider::from_row(row) {
            Ok(p) => p,
            Err(e) => {
                errors.push(format!("Failed to parse provider: {}", e));
                continue;
            }
        };

        let provider_impl: Arc<dyn ProviderTrait> = match crate::services::connection_service::provider_impl(service.db(), &provider.provider_type, provider.config.as_deref()).await {
            Ok(p) => p,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        let mut provider = provider;
        if let Err(e) = crate::services::connection_service::ensure_fresh(service.db(), &mut provider, provider_impl.clone()).await {
            errors.push(format!("Failed to refresh {}: {}", provider.id, e));
            continue;
        }
        
        let provider_id = provider.id.clone();
        let window_clone = window.clone();
        
        // Use enhanced async pattern with timeout
        use tokio::time::timeout;
        use std::time::Duration;
        
        let sync_result = timeout(
            Duration::from_secs(60),
            service.with_write_lock(move |pool| {
                let provider = provider;
                let window = window_clone;
                let provider_impl = provider_impl;
                async move {
                    provider.sync(&pool, &window, provider_impl).await
                }
            })
        ).await;
        
        match sync_result {
            Ok(Ok(_)) => {
                sync_count += 1;
                println!("✅ Successfully synced provider {}", provider_id);
            }
            Ok(Err(e)) => {
                let error_msg = format!("Failed to sync provider {}: {}", provider_id, e);
                errors.push(error_msg.clone());
                println!("⚠️ {}", error_msg);
            }
            Err(_) => {
                let error_msg = format!("Provider {} sync timed out after 60 seconds", provider_id);
                eprintln!("❌ {}", error_msg);
                errors.push(error_msg);
            }
        }
    }
    
    // Get updated conversations after sync
    let updated_conversations_response = get_conversations(service.clone()).await?;
    
    // Get updated providers after sync (includes token updates, sync status, etc.)
    let updated_providers_response = get_providers(service.clone()).await?;
    
    // Get updated profile after sync (includes user associations)
    let updated_profile = load_profile_with_settings_and_theme(&service).await?;
    
    // Get updated users after sync
    let updated_users_response = get_users(service.clone()).await?;
    
    // Emit updated conversations
    window.emit("conversations-updated", &ConversationsResponse {
        data: updated_conversations_response.data,
        is_loading: false,
    }).map_err(|e| format!("Failed to emit conversations update: {}", e))?;
    
    // Emit updated providers (includes auth status, sync timestamps, etc.)
    window.emit("providers-updated", &ProvidersResponse {
        data: updated_providers_response.data,
        is_loading: false,
    }).map_err(|e| format!("Failed to emit providers update: {}", e))?;
    
    // Emit updated profile (includes user associations from sync)
    if let Some(profile) = updated_profile {
        window.emit("profile-updated", &ProfileResponse {
            data: Some(profile),
            is_loading: false,
        }).map_err(|e| format!("Failed to emit profile update: {}", e))?;
    }
    
    // Emit updated users (new users may have been created during sync)
    window.emit("users-updated", &UsersResponse {
        data: updated_users_response.data,
        is_loading: false,
    }).map_err(|e| format!("Failed to emit users update: {}", e))?;
    
    // Emit sync complete event
    window.emit("sync-complete", serde_json::json!({
        "sync_count": sync_count,
        "errors": errors,
    })).map_err(|e| format!("Failed to emit sync complete: {}", e))?;
    
    // Finish loading state
    service.loading_state().finish_loading(loading_key_conversations());
    
    Ok(response)
}
#[tauri::command]
pub async fn get_current_conversation(
    service: State<'_, ChatnipService>,
    window: Window,
) -> Result<ConversationResponse, String> {
    use crate::services::{MessageService, MessageLoadOptions};
    
    // Get current conversation ID from profile
    let conversation_id = match get_current_conversation_id(service.db()).await? {
        Some(id) => id,
        None => {
            // No current conversation
            return Ok(ConversationResponse {
                data: None,
                is_loading: false,
            });
        }
    };
    
    // Load conversation details
    let row = sqlx::query("SELECT * FROM conversation WHERE id = ?1")
        .bind(&conversation_id)
        .fetch_optional(service.db())
        .await
        .map_err(|e| format!("Failed to get current conversation: {}", e))?;
    
    let conversation = match row {
        Some(row) => {
            let mut conv = Conversation::from_row(row)?;
            conv.is_current = true;
            conv.update_summary(service.db()).await.ok();
            
            // Load providers for the conversation
            conv.load_providers(service.db()).await?;
            conv.tags = crate::models::ObjectTag::get_conversation(service.db(), &conv.id).await.unwrap_or_default();
            conv.load_display(service.db()).await.ok();
            
            // Also emit the conversation-updated event for other listeners
            window.emit("conversation-updated", &ConversationResponse { data: Some(conv.clone()), is_loading: false })
                .map_err(|e| format!("Failed to emit conversation event: {}", e))?;
            
            // Load and emit messages separately
            let message_service = MessageService::new(service.db().clone());
            let load_options = MessageLoadOptions {
                load_reactions: true,
                load_assets: true,
                load_sender_details: true,
                limit: Some(50),
                offset: None,
            };
            
            if let Ok(enhanced_messages) = message_service.load_conversation_messages(&conversation_id, load_options).await {
                let messages: Vec<Message> = enhanced_messages.into_iter()
                    .map(|em| em.message)
                    .collect();
                println!("  📤 Emitting messages-updated event with {} messages", messages.len());
                window.emit("messages-updated", &MessagesResponse {
                    data: messages,
                    is_loading: false,
                })
                    .map_err(|e| format!("Failed to emit messages event: {}", e))?;
                println!("  ✅ Messages emitted for conversation {}", conversation_id);
            } else {
                println!("  ⚠️ Failed to load messages for conversation {}", conversation_id);
            }
            
            Some(conv)
        },
        None => None
    };
    
    Ok(ConversationResponse {
        data: conversation,
        is_loading: false,
    })
}
#[tauri::command]
pub async fn put_current_conversation(
    service: State<'_, ChatnipService>,
    window: Window,
    input: SetCurrentConversationInput,
) -> Result<Conversation, String> {
    use crate::services::{MessageService, MessageLoadOptions};
    
    let conversation_id = input.conversation_id.clone();
    log_command!("put_current_conversation", "conversation_id: {}", conversation_id);
    
    let profile = Profile::get_active(service.db()).await?.ok_or_else(|| "No active profile found".to_string())?;
    if !Conversation::is_owned_by(service.db(), &conversation_id, &profile.id).await? {
        return Err("Conversation not found".to_string());
    }
    sqlx::query("UPDATE profile SET last_id = ?1 WHERE id = ?2")
        .bind(&conversation_id)
        .bind(&profile.id)
        .execute(service.db())
        .await
        .map_err(|e| format!("Failed to set current conversation: {}", e))?;
    
    // Load conversation details
    let row = sqlx::query("SELECT * FROM conversation WHERE id = ?1")
        .bind(&conversation_id)
        .fetch_optional(service.db())
        .await
        .map_err(|e| format!("Failed to get conversation: {}", e))?;
    
    let mut conversation = match row {
        Some(row) => {
            let mut conv = Conversation::from_row(row)?;
            conv.is_current = true;
            conv.update_summary(service.db()).await.ok();
            conv
        },
        None => {
            println!("  ❌ Conversation not found: {}", conversation_id);
            return Err("Conversation not found".to_string());
        }
    };
    
    // Load providers for the conversation
    conversation.load_providers(service.db()).await?;
    conversation.tags = crate::models::ObjectTag::get_conversation(service.db(), &conversation.id).await.unwrap_or_default();
    conversation.load_display(service.db()).await.ok();
    
    println!("  ✅ Found conversation: {} ({} providers)", conversation.name, conversation.providers.len());
    
    // Load messages
    println!("  🔄 Loading messages for conversation...");
    let message_service = MessageService::new(service.db().clone());
    let load_options = MessageLoadOptions {
        load_reactions: true,
        load_assets: true,
        load_sender_details: true,
        limit: Some(50),
        offset: None,
    };
    
    let messages = message_service.load_conversation_messages(&conversation_id, load_options).await
        .map_err(|e| format!("Failed to load messages: {}", e))?;
    
    // Emit conversation-updated for the new current conversation
    window.emit("conversation-updated", &ConversationResponse {
        data: Some(conversation.clone()),
        is_loading: false,
    }).map_err(|e| format!("Failed to emit conversation-updated event: {}", e))?;
    
    // Emit messages-updated for the conversation's messages
    let message_list: Vec<Message> = messages.iter()
        .map(|em| em.message.clone())
        .collect();
    window.emit("messages-updated", &MessagesResponse {
        data: message_list,
        is_loading: false,
    }).map_err(|e| format!("Failed to emit messages-updated event: {}", e))?;
    
    // Emit profile-updated since we updated the last_id
    if let Ok(Some(profile_state)) = load_profile_with_settings_and_theme(&service).await {
        window.emit("profile-updated", &ProfileResponse {
            data: Some(profile_state),
            is_loading: false,
        }).map_err(|e| format!("Failed to emit profile event: {}", e))?;
    }
    
    // Start async sync for this conversation with timeout and progress reporting
    use serde_json::json;
    
    let db_pool = service.db().clone();
    let window_clone = window.clone();
    let conversation_clone = conversation.clone();
    let conversation_id = conversation.id.clone();
    let window_for_events = window.clone();
    
    tauri::async_runtime::spawn(async move {
        use tokio::time::timeout;
        use std::time::Duration;
        
        window_for_events.emit("conversation:sync:started", &json!({ "conversation_id": &conversation_id })).ok();
        
        match timeout(
            Duration::from_secs(30),
            conversation_clone.sync(&db_pool, &window_clone)
        ).await {
            Ok(Ok(())) => {
                window_for_events.emit("conversation:sync:completed", &json!({ "conversation_id": &conversation_id })).ok();
            }
            Ok(Err(e)) => {
                window_for_events.emit("conversation:sync:failed", &json!({ 
                    "conversation_id": &conversation_id,
                    "error": format!("Sync failed: {}", e)
                })).ok();
            }
            Err(_) => {
                window_for_events.emit("conversation:sync:failed", &json!({ 
                    "conversation_id": &conversation_id,
                    "error": "Sync timed out after 30 seconds"
                })).ok();
            }
        }
    });
    
    Ok(conversation)
}
#[tauri::command]
pub async fn delete_current_conversation(
    service: State<'_, ChatnipService>,
    window: Window,
) -> Result<(), String> {
    // Clear current conversation ID from profile
    sqlx::query("UPDATE profile SET last_id = NULL")
        .execute(service.db())
        .await
        .map_err(|e| format!("Failed to clear current conversation: {}", e))?;
    
    // Emit conversation-updated with null to clear the current conversation
    window.emit("conversation-updated", &ConversationResponse {
        data: None,
        is_loading: false,
    }).map_err(|e| format!("Failed to emit conversation-updated event: {}", e))?;
    
    // Emit messages-updated with empty array to clear messages
    window.emit("messages-updated", &MessagesResponse {
        data: vec![],
        is_loading: false,
    }).map_err(|e| format!("Failed to emit messages-updated event: {}", e))?;
    
    // Emit profile-updated since we cleared the last_id
    if let Ok(Some(profile_state)) = load_profile_with_settings_and_theme(&service).await {
        window.emit("profile-updated", &ProfileResponse {
            data: Some(profile_state),
            is_loading: false,
        }).map_err(|e| format!("Failed to emit profile event: {}", e))?;
    }
    
    Ok(())
}
#[tauri::command]
pub async fn get_current_messages(
    service: State<'_, ChatnipService>,
    window: Window,
) -> Result<MessagesResponse, String> {
    use crate::services::{MessageService, MessageLoadOptions};
    use crate::loading_state::loading_key_current_conversation;
    
    log_command!("get_current_messages");
    
    // Check loading state
    let is_loading = service.loading_state().is_loading(loading_key_current_conversation());
    
    // Get current conversation ID from profile
    let conversation_id = match get_current_conversation_id(service.db()).await? {
        Some(id) => {
            println!("  ✅ Current conversation: {}", id);
            id
        },
        None => {
            println!("  ⚠️ No current conversation set, returning empty messages");
            let response = MessagesResponse {
                data: vec![],
                is_loading,
            };
            window.emit("messages-updated", &response)
                .map_err(|e| format!("Failed to emit messages event: {}", e))?;
            return Ok(response);
        }
    };
    
    // Load messages for the current conversation
    let message_service = MessageService::new(service.db().clone());
    let enhanced_messages = message_service.load_conversation_messages(
        &conversation_id,
        MessageLoadOptions {
            limit: Some(50),
            offset: None,
            load_reactions: true,
            load_assets: true,
            load_sender_details: true,
        }
    ).await?;
    
    // Extract just the Message structs
    let messages: Vec<Message> = enhanced_messages.into_iter()
        .map(|em| em.message)
        .collect();
    
    println!("  📊 Loaded {} messages for conversation", messages.len());
    
    // Emit messages
    println!("  📤 Emitting messages-updated event with {} messages", messages.len());
    window.emit("messages-updated", &MessagesResponse {
        data: messages.clone(),
        is_loading,
    })
        .map_err(|e| format!("Failed to emit messages event: {}", e))?;
    println!("  ✅ Messages event emitted successfully");
    
    Ok(MessagesResponse {
        data: messages,
        is_loading,
    })
}
#[tauri::command]
pub async fn get_older_messages(
    service: State<'_, ChatnipService>,
    window: Window,
    input: GetConversationInput,
) -> Result<(), String> {
    use crate::loading_state::loading_key_conversations;
    
    let conversation_id = input.conversation_id;
    
    // Set loading state
    service.loading_state().start_loading(loading_key_conversations());
    
    // Get all providers for this conversation
    let provider_rows = sqlx::query(
        "SELECT DISTINCT provider_id FROM message WHERE conversation_id = ?1"
    )
    .bind(&conversation_id)
    .fetch_all(service.db())
    .await
    .map_err(|e| format!("Failed to get providers for conversation: {}", e))?;
    
    let mut message_count = 0;
    
    for provider_row in provider_rows {
        let provider_id: String = provider_row.get("provider_id");
        
        // Get the provider details
        let provider_detail_row = sqlx::query(
            "SELECT * FROM provider WHERE id = ?1"
        )
        .bind(&provider_id)
        .fetch_optional(service.db())
        .await
        .map_err(|e| format!("Failed to get provider details: {}", e))?;
        
        if let Some(row) = provider_detail_row {
            let mut provider = crate::providers::models::Provider::from_row(row)?;
            
            // Call more_messages on the provider
            match provider.more_messages(service.db(), &conversation_id).await {
                Ok(count) => {
                    message_count += count;
                    println!("📥 Loaded {} more messages from provider {}", count, provider.id);
                }
                Err(e) => {
                    eprintln!("⚠️ Failed to load more messages from provider {}: {}", provider.id, e);
                }
            }
        }
    }
    
    // Finish loading state
    service.loading_state().finish_loading(loading_key_conversations());
    
    // Emit event to notify frontend
    window.emit("more-messages-loaded", serde_json::json!({
        "conversation_id": conversation_id,
        "message_count": message_count
    })).map_err(|e| format!("Failed to emit more-messages-loaded event: {}", e))?;
    
    Ok(())
}
#[tauri::command]
pub async fn post_provider_auth(
    service: State<'_, ChatnipService>,
    input: StartProviderAuthInput,
) -> Result<String, String> {
    log_command!("post_provider_auth", "provider: {}", input.provider_name);
    crate::services::connection_service::start_auth(service.db(), service.pending_auth(), &input.provider_name).await
}

#[tauri::command]
pub async fn post_slack_connection(
    service: State<'_, ChatnipService>,
    window: Window,
    token: String,
) -> Result<Provider, String> {
    log_command!("post_slack_connection");
    crate::services::connection_service::connect_with_token(window.app_handle(), service.db(), "slack", &token).await
}

#[tauri::command]
pub async fn get_connection_setup(service: State<'_, ChatnipService>) -> Result<Vec<crate::services::connection_service::ProviderSetup>, String> {
    crate::services::connection_service::connection_setup(service.db()).await
}

#[tauri::command]
pub async fn post_admin_consent(service: State<'_, ChatnipService>) -> Result<String, String> {
    let url = crate::services::connection_service::admin_consent_link(service.db()).await?;
    open::that(&url).map_err(|e| format!("Could not open the browser: {}", e))?;
    Ok(url)
}

#[tauri::command]
pub async fn delete_provider(
    service: State<'_, ChatnipService>,
    window: Window,
    provider_id: String,
) -> Result<(), String> {
    log_command!("delete_provider", "provider: {}", provider_id);
    crate::services::connection_service::delete_provider(window.app_handle(), service.db(), &provider_id).await
}

#[tauri::command]
pub async fn get_app_settings(
    service: State<'_, ChatnipService>,
) -> Result<std::collections::HashMap<String, String>, String> {
    crate::models::AppSetting::all(service.db()).await
}

#[tauri::command]
pub async fn put_app_setting(
    service: State<'_, ChatnipService>,
    app: AppHandle,
    key: String,
    value: String,
) -> Result<std::collections::HashMap<String, String>, String> {
    log_command!("put_app_setting", "key: {}", key);
    crate::models::AppSetting::put(service.db(), &key, &value).await?;
    if crate::models::AppSetting::switches_mock_off(&key, &value) && crate::commands_people::mock_forced().is_none() {
        if let Some(profile) = crate::models::Profile::get_active(service.db()).await? {
            let mocks: Vec<String> = sqlx::query_scalar("SELECT id FROM provider WHERE profile_id = ?1 AND provider_type = 'mock'")
                .bind(&profile.id)
                .fetch_all(service.db())
                .await
                .map_err(|e| format!("Failed to list test clients: {}", e))?;
            for provider_id in mocks {
                crate::services::connection_service::delete_provider(&app, service.db(), &provider_id).await?;
            }
        }
    }
    crate::models::AppSetting::all(service.db()).await
}
#[tauri::command]
pub async fn post_log(
    level: String,
    message: String,
    source: Option<String>,
) -> Result<(), String> {
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let source_str = source.unwrap_or_else(|| "frontend".to_string());
    match level.as_str() {
        "debug" => println!("🐛 [{} {}] {}", timestamp, source_str, message),
        "info" => println!("ℹ️  [{} {}] {}", timestamp, source_str, message),
        "warn" => println!("⚠️  [{} {}] {}", timestamp, source_str, message),
        "error" => println!("❌ [{} {}] {}", timestamp, source_str, message),
        _ => println!("📝 [{} {}] {}", timestamp, source_str, message),
    }
    Ok(())
}

#[tauri::command]
pub async fn load_message_assets(
    service: State<'_, ChatnipService>,
    window: Window,
    message_id: String,
) -> Result<(), String> {
    use crate::services::{MessageService, MessageLoadOptions};
    
    let message_service = MessageService::new(service.db().clone());
    
    // Load the message first
    let enhanced_message = message_service
        .load_message(&message_id, MessageLoadOptions::default())
        .await
        .map_err(|e| format!("Failed to load message: {}", e))?;
    
    // Check if we have a Microsoft provider to sync from
    if let Ok(providers) = sqlx::query("SELECT * FROM provider WHERE provider_type = 'microsoft' AND is_enabled = 1")
        .fetch_all(service.db())
        .await
    {
        for provider_row in providers {
            let provider_id: String = provider_row.get("id");
            
            // Create provider client
            let provider = crate::services::connection_service::microsoft_provider(service.db()).await?;
            
            // Ensure we have valid auth tokens (will refresh if needed)
            if let Ok(auth) = provider.ensure_valid_auth(service.db(), &provider_id).await {
                // Sync assets from Graph API
                match message_service.sync_message_assets_from_graph(&enhanced_message.message, &auth, &provider).await {
                        Ok(assets) => {
                            println!("✅ Synced {} assets for message {}", assets.len(), message_id);
                            
                            // Reload the message with updated assets
                            if let Ok(updated_message) = message_service
                                .load_message(&message_id, MessageLoadOptions::default())
                                .await
                            {
                                window.emit("message-assets-updated", &updated_message)
                                    .map_err(|e| format!("Failed to emit message-assets-updated event: {}", e))?;
                            }
                        }
                        Err(e) => {
                            println!("⚠️ Failed to sync assets for message {}: {}", message_id, e);
                        }
                    }
                    
                    break; // Only use the first available provider
            }
        }
    }
    
    Ok(())
}

#[tauri::command]
pub async fn get_providers(
    service: State<'_, ChatnipService>,
) -> Result<ProvidersResponse, String> {
    let db = service.db();
    let profile_id = match get_current_profile_id(db).await {
        Ok(id) => id,
        Err(_) => {
            // No active profile, return empty providers
            return Ok(ProvidersResponse {
                data: vec![],
                is_loading: false,
            });
        }
    };
    
    let hide_mock = !crate::commands_people::mock_enabled(service.db()).await;
    let rows = sqlx::query(if hide_mock { "SELECT * FROM provider WHERE profile_id = ?1 AND provider_type != 'mock'" } else { "SELECT * FROM provider WHERE profile_id = ?1" })
        .bind(&profile_id)
        .fetch_all(db)
        .await
        .map_err(|e| format!("Failed to get providers: {}", e))?;
    
    let mut providers = Vec::new();
    for row in rows {
        match crate::providers::Provider::load_from_db(db, row.get("id")).await {
            Ok(mut provider) => {
                provider.theme = crate::utils::theme_generator::get_provider_theme(db, &provider.id).await?;
                provider.load_logo(db).await.ok();
                provider.load_account(db).await.ok();
                providers.push(provider);
            },
            Err(e) => eprintln!("Failed to load provider: {}", e),
        }
    }
    
    Ok(ProvidersResponse {
        data: providers,
        is_loading: false,
    })
}

#[tauri::command]
pub async fn get_users(
    service: State<'_, ChatnipService>,
) -> Result<UsersResponse, String> {
    log_command!("get_users");
    
    let profile_id = match get_current_profile_id(service.db()).await {
        Ok(id) => {
            println!("  ✅ Found profile: {}", id);
            id
        },
        Err(e) => {
            println!("  ⚠️ No profile found: {}", e);
            return Ok(UsersResponse {
                data: vec![],
                is_loading: false,
            });
        }
    };
    
    let users = User::get_all_for_profile(service.db(), &profile_id).await?;
    println!("  📊 Loaded {} users for profile", users.len());
    
    Ok(UsersResponse {
        data: users,
        is_loading: false,
    })
}

#[tauri::command]
pub async fn get_provider_theme(
    service: State<'_, ChatnipService>,
    provider_id: String,
) -> Result<Option<crate::utils::theme_generator::ProviderTheme>, String> {
    crate::utils::theme_generator::get_provider_theme(service.db(), &provider_id).await
}

#[tauri::command]
pub async fn put_provider_theme(
    service: State<'_, ChatnipService>,
    window: Window,
    provider_id: String,
    theme: crate::utils::theme_generator::ProviderTheme,
) -> Result<crate::utils::theme_generator::ProviderTheme, String> {
    let db = service.db();
    
    sqlx::query(
        r#"UPDATE provider_theme SET
            theme_name = ?2,
            primary_50 = ?3, primary_100 = ?4, primary_200 = ?5, primary_300 = ?6,
            primary_400 = ?7, primary_500 = ?8, primary_600 = ?9, primary_700 = ?10,
            primary_800 = ?11, primary_900 = ?12, primary_950 = ?13,
            updated_at = ?14
        WHERE provider_id = ?1"#
    )
    .bind(&provider_id)
    .bind(&theme.theme_name)
    .bind(&theme.primary_50)
    .bind(&theme.primary_100)
    .bind(&theme.primary_200)
    .bind(&theme.primary_300)
    .bind(&theme.primary_400)
    .bind(&theme.primary_500)
    .bind(&theme.primary_600)
    .bind(&theme.primary_700)
    .bind(&theme.primary_800)
    .bind(&theme.primary_900)
    .bind(&theme.primary_950)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(db)
    .await
    .map_err(|e| format!("Failed to update provider theme: {}", e))?;
    
    window.emit("provider-theme-updated", &theme)
        .map_err(|e| format!("Failed to emit event: {}", e))?;
    
    Ok(theme)
}

#[tauri::command]
pub fn get_platform() -> String {
    std::env::consts::OS.to_string()
}
#[tauri::command]
pub async fn put_window_position(
    service: State<'_, ChatnipService>,
    window: Window,
) -> Result<(), String> {
    crate::services::window_service::put_position(&service, &window).await
}
#[tauri::command]
pub async fn put_window_frame(
    service: State<'_, ChatnipService>,
    window: Window,
    duration: u64,
) -> Result<(), String> {
    crate::services::window_service::put_frame(&service, &window, duration).await
}
#[tauri::command]
pub async fn post_provider_connection(
    service: State<'_, ChatnipService>,
    _window: tauri::Window,
    input: ConnectProviderInput,
) -> Result<String, String> {
    post_provider_auth(service, StartProviderAuthInput { provider_name: input.provider_name }).await
}

#[tauri::command]
pub async fn post_provider_sync(
    service: State<'_, ChatnipService>,
    window: WebviewWindow,
    provider_id: String,
) -> Result<(), String> {
    log_command!("post_provider_sync", "provider: {}", provider_id);
    crate::services::connection_service::sync_provider(window.app_handle(), service.db(), &provider_id).await
}


#[tauri::command]
pub async fn put_provider_logo(
    service: State<'_, ChatnipService>,
    window: Window,
    provider_id: String,
) -> Result<(), String> {
    log_command!("put_provider_logo", "provider: {}", provider_id);
    crate::services::connection_service::refresh_logo(window.app_handle(), service.db(), &provider_id).await
}

#[tauri::command]
pub async fn get_asset(
    service: State<'_, ChatnipService>,
    app_handle: AppHandle,
    asset_id: String,
) -> Result<Option<Asset>, String> {
    let row = sqlx::query("SELECT * FROM assets WHERE id = ?1")
        .bind(&asset_id)
        .fetch_optional(service.db())
        .await
        .map_err(|e| format!("Failed to fetch asset: {}", e))?;
    
    if let Some(row) = row {
        let path: Option<String> = row.get("path");
        let url: Option<String> = row.get("url");
        
        let final_url = if let Some(path_str) = &path {
            if std::path::Path::new(path_str).exists() {
                let app_dir = app_handle.path().app_data_dir()
                    .map_err(|e| format!("Failed to get app data dir: {}", e))?;
                let relative_path = std::path::Path::new(path_str)
                    .strip_prefix(&app_dir)
                    .unwrap_or(std::path::Path::new(path_str));
                Some(format!("asset://localhost/{}", relative_path.to_string_lossy().replace('\\', "/")))
            } else {
                url
            }
        } else {
            url
        };
        
        Ok(Some(Asset {
            id: row.get("id"),
            profile_id: row.get("profile_id"),
            name: row.get("name"),
            url: final_url,
            asset_type: row.get("type"),
            mime_type: row.get("mime_type"),
            path,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    } else {
        Ok(None)
    }
}

async fn sync_microsoft_conversation_assets(
    message_service: MessageService,
    window: Window,
    conversation_id: String
) -> Result<(), String> {
    println!("🔄 Background sync of assets for conversation {} from Microsoft Graph", conversation_id);

    let updated_messages = message_service
        .load_conversation_messages(&conversation_id, MessageLoadOptions::default())
        .await
        .map_err(|e| format!("Failed to load conversation messages: {}", e))?;

    window.emit("conversation-messages-updated", &updated_messages)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn post_conversation(
    service: State<'_, ChatnipService>,
    input: CreateConversationInput,
) -> Result<Conversation, String> {
    log_command!("post_conversation", "name={}", input.name);

    let pool = service.db();
    let conversation_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"INSERT INTO conversation (id, name, type, group_type, last_activity, is_current, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)"#
    )
    .bind(&conversation_id)
    .bind(&input.name)
    .bind(&input.conversation_type)
    .bind(&input.group_type)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create conversation: {}", e))?;

    for member_id in &input.member_ids {
        let member_row_id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO conversation_member (id, conversation_id, user_id, added_at)
               VALUES (?1, ?2, ?3, ?4)"#
        )
        .bind(&member_row_id)
        .bind(&conversation_id)
        .bind(member_id)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to add member: {}", e))?;
    }

    let mut members = Vec::new();
    for member_id in &input.member_ids {
        if let Some(user) = User::find_by_id(pool, member_id).await? {
            members.push(user);
        }
    }

    Ok(Conversation {
        id: conversation_id,
        provider_name: input.name.clone(),
        image: None,
        overrides: std::collections::BTreeMap::new(),
        nicknames: std::collections::BTreeMap::new(),
        name: input.name,
        conversation_type: input.conversation_type,
        group_type: input.group_type,
        last_id: None,
        last_activity: now.clone(),
        providers: vec![],
        theme: None,
        namespace: None,
        metadata: None,
        unread_count: 0,
        members,
        is_current: false,
        is_pinned: false,
        is_muted: false,
        mode: crate::models::chat::ChatMode::Write,
        view: crate::models::chat::ChatView::Chat,
        view_settings: crate::models::chat::ViewSettings::default(),
        is_archived: false,
        snoozed_until: None,
        deleted_at: None,
        person_id: None,
        favourite_position: None,
        tags: Vec::new(),
        notification_level: "all".to_string(),
        summary: None,
        presentation: None,
        threads: vec![],
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
pub async fn post_conversation_merge(
    service: State<'_, ChatnipService>,
    input: MergeConversationsInput,
) -> Result<Conversation, String> {
    log_command!("post_conversation_merge", "source={} target={}", input.source_conversation_id, input.target_conversation_id);

    let pool = service.db();
    let now = Utc::now().to_rfc3339();

    sqlx::query("UPDATE message SET conversation_id = ?1 WHERE conversation_id = ?2")
        .bind(&input.target_conversation_id)
        .bind(&input.source_conversation_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to move messages: {}", e))?;

    sqlx::query("UPDATE OR IGNORE conversation_provider SET conversation_id = ?1, updated_at = ?2 WHERE conversation_id = ?3")
    .bind(&input.target_conversation_id)
    .bind(&now)
    .bind(&input.source_conversation_id)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to merge providers: {}", e))?;

    sqlx::query(
        r#"INSERT OR IGNORE INTO conversation_member (id, conversation_id, user_id, added_at)
           SELECT ?1 || '-' || id, ?2, user_id, ?3
           FROM conversation_member WHERE conversation_id = ?4"#
    )
    .bind(&Uuid::new_v4().to_string())
    .bind(&input.target_conversation_id)
    .bind(&now)
    .bind(&input.source_conversation_id)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to merge members: {}", e))?;

    sqlx::query("UPDATE conversation SET is_current = 0, updated_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(&input.source_conversation_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to deactivate source conversation: {}", e))?;

    Conversation::load_with_providers(pool, &input.target_conversation_id)
        .await?
        .ok_or_else(|| "Target conversation not found".to_string())
}

#[tauri::command]
pub async fn post_conversation_link(
    service: State<'_, ChatnipService>,
    input: LinkConversationsInput,
) -> Result<(), String> {
    log_command!("post_conversation_link", "source={} target={} type={}", input.source_conversation_id, input.target_conversation_id, input.link_type);

    crate::models::ConversationLink::post(service.db(), &input.source_conversation_id, &input.target_conversation_id, &input.link_type).await
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConversationLinkRow {
    pub source_conversation_id: String,
    pub target_conversation_id: String,
    pub link_type: String,
}

#[tauri::command]
pub async fn get_conversation_links(service: State<'_, ChatnipService>, link_type: Option<String>) -> Result<Vec<ConversationLinkRow>, String> {
    let profile = crate::models::Profile::get_active(service.db()).await?.ok_or_else(|| "No active profile".to_string())?;
    let rows = sqlx::query(
        "SELECT DISTINCT l.source_conversation_id, l.target_conversation_id, l.link_type FROM conversation_link l
         JOIN conversation_provider cp ON cp.conversation_id = l.target_conversation_id
         JOIN provider p ON p.id = cp.provider_id
         WHERE p.profile_id = ?1 AND (?2 IS NULL OR l.link_type = ?2)",
    )
    .bind(&profile.id)
    .bind(link_type.as_deref())
    .fetch_all(service.db())
    .await
    .map_err(|e| format!("Failed to load conversation links: {}", e))?;
    Ok(rows
        .into_iter()
        .map(|row| ConversationLinkRow { source_conversation_id: row.get("source_conversation_id"), target_conversation_id: row.get("target_conversation_id"), link_type: row.get("link_type") })
        .collect())
}

#[tauri::command]
pub async fn delete_conversation_link(service: State<'_, ChatnipService>, source_conversation_id: String, target_conversation_id: String) -> Result<(), String> {
    sqlx::query("DELETE FROM conversation_link WHERE (source_conversation_id = ?1 AND target_conversation_id = ?2) OR (source_conversation_id = ?2 AND target_conversation_id = ?1)")
        .bind(&source_conversation_id)
        .bind(&target_conversation_id)
        .execute(service.db())
        .await
        .map_err(|e| format!("Failed to unlink conversations: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn get_merge_groups(service: State<'_, ChatnipService>) -> Result<Vec<crate::models::MergeGroup>, String> {
    let profile = Profile::get_active(service.db()).await?.ok_or_else(|| "No active profile".to_string())?;
    crate::models::MergeGroup::get_all(service.db(), &profile.id).await
}

#[tauri::command]
pub async fn post_merge_group(service: State<'_, ChatnipService>, input: crate::models::MergeGroupInput) -> Result<Vec<crate::models::MergeGroup>, String> {
    log_command!("post_merge_group", "lead={} conversations={} groups={}", input.lead_conversation_id, input.conversation_ids.len(), input.group_lead_ids.len());
    let profile = Profile::get_active(service.db()).await?.ok_or_else(|| "No active profile".to_string())?;
    service
        .with_write_lock(move |pool| async move {
            crate::models::MergeGroup::post(&pool, &profile.id, &input).await?;
            crate::models::MergeGroup::get_all(&pool, &profile.id).await
        })
        .await
}

#[tauri::command]
pub async fn delete_merge_group_items(service: State<'_, ChatnipService>, input: crate::models::MergeGroupInput) -> Result<Vec<crate::models::MergeGroup>, String> {
    log_command!("delete_merge_group_items", "lead={} conversations={} groups={}", input.lead_conversation_id, input.conversation_ids.len(), input.group_lead_ids.len());
    let profile = Profile::get_active(service.db()).await?.ok_or_else(|| "No active profile".to_string())?;
    service
        .with_write_lock(move |pool| async move {
            crate::models::MergeGroup::delete_items(&pool, &profile.id, &input).await?;
            crate::models::MergeGroup::get_all(&pool, &profile.id).await
        })
        .await
}

#[tauri::command]
pub async fn post_thread(
    service: State<'_, ChatnipService>,
    input: CreateThreadInput,
) -> Result<Thread, String> {
    log_command!("post_thread", "message_id={}", input.message_id);

    let pool = service.db();
    let thread_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let conversation_id = crate::models::Message::get_conversation_id(pool, &input.message_id).await?.ok_or_else(|| "Message not found".to_string())?;

    sqlx::query(
        r#"INSERT INTO thread (id, conversation_id, root_message_id, name, last_activity, message_count, is_resolved, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6, ?7)"#
    )
    .bind(&thread_id)
    .bind(&conversation_id)
    .bind(&input.message_id)
    .bind(&input.name)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create thread: {}", e))?;

    sqlx::query("UPDATE message SET thread_id = ?1 WHERE id = ?2")
        .bind(&thread_id)
        .bind(&input.message_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update message thread_id: {}", e))?;

    Ok(Thread {
        id: thread_id,
        conversation_id,
        root_message_id: input.message_id,
        name: input.name,
        last_activity: now.clone(),
        message_count: 0,
        is_resolved: false,
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
pub async fn post_thread_reply(
    service: State<'_, ChatnipService>,
    window: Window,
    input: ReplyInThreadInput,
) -> Result<Message, String> {
    log_command!("post_thread_reply", "thread_id={}", input.thread_id);

    let pool = service.db();
    let now = Utc::now().to_rfc3339();
    let content_type = input.content_type.clone().unwrap_or_else(|| "text".to_string());

    let thread_row = sqlx::query("SELECT conversation_id, root_message_id FROM thread WHERE id = ?1")
        .bind(&input.thread_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to find thread: {}", e))?
        .ok_or_else(|| "Thread not found".to_string())?;

    let conversation_id: String = thread_row.get("conversation_id");
    let root_message_id: String = thread_row.get("root_message_id");
    let root_provider: Option<String> = sqlx::query_scalar("SELECT provider_id FROM message WHERE id = ?1 AND provider_id != 'local'")
        .bind(&root_message_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to read the thread root: {}", e))?;

    let message = match root_provider {
        Some(provider_id) => {
            let app_data = window.app_handle().path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
            let provider = crate::providers::models::Provider::load_from_db(pool, &provider_id).await?;
            let broadcast = input.broadcast.unwrap_or(false);
            let native = broadcast && provider.capabilities.thread_broadcast == crate::providers::models::Delivery::Native;
            let send = |metadata: serde_json::Value| SendMessageInput {
                conversation_id: conversation_id.clone(),
                content: input.content.clone(),
                content_type: Some(content_type.clone()),
                provider_id: Some(provider_id.clone()),
                metadata: Some(metadata),
                attachments: input.attachments.clone(),
            };
            let sent = crate::services::outbound_service::send(pool, &app_data, send(serde_json::json!({ "thread_message_id": root_message_id, "broadcast": native }))).await?;
            if broadcast && provider.capabilities.thread_broadcast == crate::providers::models::Delivery::Repost {
                crate::services::outbound_service::send(pool, &app_data, send(serde_json::json!({ "in_reply_to": root_message_id }))).await?;
            }
            sent
        }
        None => {
            let message_id = Uuid::new_v4().to_string();
            sqlx::query(
                r#"INSERT INTO message (
                    id, external_id, provider_id, type,
                    content, content_type, conversation_id,
                    thread_id, reply_to_id,
                    sender_id, sender_name, importance, is_read, is_draft, has_attachments,
                    created_datetime, last_modified_datetime, sent_datetime,
                    timestamp, cached_at, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)"#
            )
            .bind(&message_id)
            .bind(&message_id)
            .bind("local")
            .bind("chat")
            .bind(&input.content)
            .bind(&content_type)
            .bind(&conversation_id)
            .bind(&input.thread_id)
            .bind(&root_message_id)
            .bind("current_user")
            .bind("User")
            .bind("normal")
            .bind(true)
            .bind(false)
            .bind(false)
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to create thread reply: {}", e))?;
            let mut message = Message::new(message_id.clone(), "local".to_string(), message_id, "chat".to_string(), Some(input.content.clone()), now.clone());
            message.content_type = content_type.clone();
            message.conversation_id = Some(conversation_id.clone());
            message.thread_id = Some(input.thread_id.clone());
            message.reply_to_id = Some(root_message_id.clone());
            message.sender_id = Some("current_user".to_string());
            message.sender_name = Some("User".to_string());
            message.is_read = true;
            message.sent_datetime = Some(now.clone());
            message
        }
    };

    let position_row = sqlx::query("SELECT COALESCE(MAX(position), 0) + 1 as next_pos FROM thread_message WHERE thread_id = ?1")
        .bind(&input.thread_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to get thread position: {}", e))?;
    let next_position: i32 = position_row.get("next_pos");

    sqlx::query("INSERT INTO thread_message (id, thread_id, message_id, position, created_at) VALUES (?1, ?2, ?3, ?4, ?5)")
        .bind(Uuid::new_v4().to_string())
        .bind(&input.thread_id)
        .bind(&message.id)
        .bind(next_position)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to insert thread_message: {}", e))?;

    sqlx::query("UPDATE thread SET message_count = message_count + 1, last_activity = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(&now)
        .bind(&now)
        .bind(&input.thread_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update thread: {}", e))?;

    Ok(message)
}

#[tauri::command]
pub async fn get_thread_messages(
    service: State<'_, ChatnipService>,
    input: GetThreadInput,
) -> Result<Vec<Message>, String> {
    log_command!("get_thread_messages", "thread_id={}", input.thread_id);

    let pool = service.db();

    let rows = sqlx::query(
        r#"SELECT m.* FROM message m
           JOIN thread_message tm ON m.id = tm.message_id
           WHERE tm.thread_id = ?1
           ORDER BY tm.position ASC"#
    )
    .bind(&input.thread_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to get thread messages: {}", e))?;

    let message_service = MessageService::new(pool.clone());
    let messages: Vec<Message> = rows.into_iter()
        .map(|row| message_service.row_to_message(row))
        .collect();

    Ok(messages)
}

#[tauri::command]
pub async fn put_conversation(
    service: State<'_, ChatnipService>,
    window: Window,
    input: UpdateConversationInput,
) -> Result<Conversation, String> {
    log_command!("put_conversation", "id={}", input.conversation_id);

    let pool = service.db();
    let now = Utc::now().to_rfc3339();

    if let Some(name) = &input.name {
        Conversation::put_name(pool, &input.conversation_id, name, input.rename_on_provider.unwrap_or(false)).await?;
    }

    if let Some(group_type) = &input.group_type {
        sqlx::query("UPDATE conversation SET group_type = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(group_type)
            .bind(&now)
            .bind(&input.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update conversation group_type: {}", e))?;
    }

    if let Some(is_pinned) = input.is_pinned {
        sqlx::query("UPDATE conversation SET is_pinned = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(is_pinned)
            .bind(&now)
            .bind(&input.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update conversation is_pinned: {}", e))?;
    }

    if let Some(is_archived) = input.is_archived {
        sqlx::query("UPDATE conversation SET is_archived = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(is_archived)
            .bind(&now)
            .bind(&input.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update conversation archive state: {}", e))?;
        let pool = pool.clone();
        let conversation_id = input.conversation_id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::services::outbound_service::put_archive(&pool, &conversation_id, is_archived).await {
                eprintln!("Done was not written to the provider: {}", e);
            }
        });
    }
    if let Some(snoozed_until) = &input.snoozed_until {
        sqlx::query("UPDATE conversation SET snoozed_until = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(snoozed_until)
            .bind(&now)
            .bind(&input.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update conversation snooze: {}", e))?;
    }
    if let Some(deleted_at) = &input.deleted_at {
        sqlx::query("UPDATE conversation SET deleted_at = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(deleted_at)
            .bind(&now)
            .bind(&input.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update conversation: {}", e))?;
    }
    if let Some(favourite_position) = &input.favourite_position {
        sqlx::query("UPDATE conversation SET favourite_position = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(favourite_position)
            .bind(&now)
            .bind(&input.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update conversation: {}", e))?;
    }
    if let Some(is_muted) = input.is_muted {
        sqlx::query("UPDATE conversation SET is_muted = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(is_muted)
            .bind(&now)
            .bind(&input.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update conversation is_muted: {}", e))?;
    }
    if let Some(mode) = input.mode {
        sqlx::query("UPDATE conversation SET mode = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(mode.name())
            .bind(&now)
            .bind(&input.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update the conversation mode: {}", e))?;
    }
    if let Some(view) = input.view {
        sqlx::query("UPDATE conversation SET view = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(view.name())
            .bind(&now)
            .bind(&input.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update the conversation view: {}", e))?;
    }
    if let Some(settings) = &input.view_settings {
        sqlx::query("UPDATE conversation SET view_settings = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(serde_json::to_string(settings).map_err(|e| e.to_string())?)
            .bind(&now)
            .bind(&input.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update the view settings: {}", e))?;
    }

    if let Some(metadata) = &input.metadata {
        sqlx::query("UPDATE conversation SET metadata = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(metadata)
            .bind(&now)
            .bind(&input.conversation_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update conversation metadata: {}", e))?;
    }

    let mut conversation = Conversation::load_with_providers(pool, &input.conversation_id)
        .await?
        .ok_or_else(|| "Conversation not found".to_string())?;
    conversation.tags = crate::models::ObjectTag::get_conversation(pool, &conversation.id).await.unwrap_or_default();
    conversation.load_display(pool).await.ok();
    if get_current_conversation_id(pool).await?.as_deref() == Some(conversation.id.as_str()) {
        let mut current = conversation.clone();
        current.is_current = true;
        window.emit("conversation-updated", &ConversationResponse { data: Some(current), is_loading: false })
            .map_err(|e| format!("Failed to emit conversation event: {}", e))?;
    }
    let conversations = get_conversations(service.clone()).await?;
    window.emit("conversations-updated", &conversations)
        .map_err(|e| format!("Failed to emit conversations event: {}", e))?;
    Ok(conversation)
}

#[tauri::command]
pub async fn post_conversation_member(
    service: State<'_, ChatnipService>,
    input: ConversationMemberInput,
) -> Result<(), String> {
    log_command!("post_conversation_member", "conversation={} user={}", input.conversation_id, input.user_id);
    crate::services::outbound_service::put_member(service.db(), &input.conversation_id, &input.user_id, true).await?;

    let pool = service.db();
    let member_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"INSERT OR IGNORE INTO conversation_member (id, conversation_id, user_id, added_at)
           VALUES (?1, ?2, ?3, ?4)"#
    )
    .bind(&member_id)
    .bind(&input.conversation_id)
    .bind(&input.user_id)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to add conversation member: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_conversation_member(
    service: State<'_, ChatnipService>,
    input: ConversationMemberInput,
) -> Result<(), String> {
    log_command!("delete_conversation_member", "conversation={} user={}", input.conversation_id, input.user_id);
    crate::services::outbound_service::put_member(service.db(), &input.conversation_id, &input.user_id, false).await?;

    let pool = service.db();

    sqlx::query("DELETE FROM conversation_member WHERE conversation_id = ?1 AND user_id = ?2")
        .bind(&input.conversation_id)
        .bind(&input.user_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to remove conversation member: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn get_search(
    service: State<'_, ChatnipService>,
    input: SearchInput,
) -> Result<SearchResults, String> {
    log_command!("get_search", "query={}", input.query);

    let pool = service.db();
    let limit = input.limit.unwrap_or(20).clamp(1, 200) as i64;
    let profile_id = Profile::get_active(pool).await?.map(|p| p.id).unwrap_or_default();
    let scope = search_scope(pool, input.conversation_ids.as_deref(), input.tag_ids.as_deref()).await?;
    let parsed = crate::services::message_service::SearchFilter::parse(input.query.trim());
    let text = parsed.text.as_str();

    let conversations: Vec<Conversation> = if text.is_empty() && scope.is_none() {
        Vec::new()
    } else {
        let pattern = format!("%{}%", text);
        let filter = match &scope {
            Some(ids) if ids.is_empty() => " AND 0".to_string(),
            Some(ids) => format!(" AND c.id IN ({})", ids.iter().map(|id| format!("'{}'", id.replace('\'', "''"))).collect::<Vec<_>>().join(",")),
            None => String::new(),
        };
        let rows = sqlx::query(&format!(
            "SELECT c.* FROM conversation c WHERE {} AND (c.name LIKE ?2 OR c.summary LIKE ?2 OR EXISTS (SELECT 1 FROM object_override o WHERE o.object_type = '{}' AND o.object_id = c.id AND o.scope_type = '' AND o.field = '{}' AND o.value LIKE ?2)){} ORDER BY (c.name LIKE ?4) DESC, c.last_activity DESC LIMIT ?3",
            crate::models::OWNED_BY_PROFILE,
            crate::models::tag::OBJECT_CONVERSATION,
            crate::models::object_override::FIELD_NAME,
            filter
        ))
        .bind(&profile_id)
        .bind(&pattern)
        .bind(limit)
        .bind(format!("{}%", text))
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to search conversations: {}", e))?;
        let mut found = rows.into_iter().map(Conversation::from_row).collect::<Result<Vec<_>, _>>()?;
        for conversation in &mut found {
            conversation.load_display(pool).await?;
        }
        found
    };

    let owner = profile_id.clone();
    let hits = service.with_write_lock(move |pool| async move { crate::services::message_service::search_messages(&pool, &owner, &parsed, scope.as_deref(), limit).await }).await?;
    let snippets = hits.iter().map(|(message, snippet)| crate::services::message_service::MessageSnippet { message_id: message.id.clone(), snippet: snippet.clone() }).collect();
    let messages = hits.into_iter().map(|(message, _)| message).collect();

    Ok(SearchResults { conversations, messages, snippets })
}

async fn search_scope(pool: &sqlx::Pool<sqlx::Sqlite>, conversation_ids: Option<&[String]>, tag_ids: Option<&[String]>) -> Result<Option<Vec<String>>, String> {
    let mut scope: Option<Vec<String>> = conversation_ids.filter(|ids| !ids.is_empty()).map(|ids| ids.to_vec());
    for tag_id in tag_ids.unwrap_or_default() {
        let tagged = crate::models::ObjectTag::get_conversation_ids(pool, tag_id).await?;
        scope = Some(match scope {
            Some(current) => current.into_iter().filter(|id| tagged.contains(id)).collect(),
            None => tagged,
        });
    }
    Ok(scope)
}

#[tauri::command]
pub async fn put_message(
    service: State<'_, ChatnipService>,
    window: Window,
    input: EditMessageInput,
) -> Result<Message, String> {
    log_command!("put_message", "message_id={}", input.message_id);

    let pool = service.db();
    let now = Utc::now().to_rfc3339();

    let existing = sqlx::query("SELECT sender_id FROM message WHERE id = ?1")
        .bind(&input.message_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to find message: {}", e))?
        .ok_or_else(|| "Message not found".to_string())?;

    let sender_id: Option<String> = existing.get("sender_id");
    let me = Profile::get_active(pool).await?.and_then(|p| p.user_id);
    if sender_id.as_deref() != Some("current_user") && sender_id != me {
        return Err("You can only edit your own messages".to_string());
    }
    crate::services::outbound_service::edit(pool, &input.message_id, &input.content).await?;

    sqlx::query("UPDATE message SET content = ?1, last_modified_datetime = ?2, updated_at = ?3 WHERE id = ?4")
        .bind(&input.content)
        .bind(&now)
        .bind(&now)
        .bind(&input.message_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to edit message: {}", e))?;

    crate::services::action_service::on_edited(window.app_handle().clone(), input.message_id.clone());
    let message_service = MessageService::new(pool.clone());
    let row = sqlx::query("SELECT * FROM message WHERE id = ?1")
        .bind(&input.message_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to reload message: {}", e))?;
    let message = message_service.row_to_message(row);

    let messages_response = get_current_messages(service.clone(), window.clone()).await?;
    window.emit("messages-updated", &messages_response)
        .map_err(|e| format!("Failed to emit messages update: {}", e))?;

    Ok(message)
}

#[tauri::command]
pub async fn delete_message(
    service: State<'_, ChatnipService>,
    window: Window,
    input: DeleteMessageInput,
) -> Result<(), String> {
    log_command!("delete_message", "message_id={}", input.message_id);

    let pool = service.db();
    let now = Utc::now().to_rfc3339();

    let existing = sqlx::query("SELECT sender_id FROM message WHERE id = ?1")
        .bind(&input.message_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to find message: {}", e))?
        .ok_or_else(|| "Message not found".to_string())?;

    let sender_id: Option<String> = existing.get("sender_id");
    let me = Profile::get_active(pool).await?.and_then(|p| p.user_id);
    if sender_id.as_deref() != Some("current_user") && sender_id != me {
        return Err("You can only delete your own messages".to_string());
    }
    crate::services::outbound_service::delete(pool, &input.message_id).await?;

    sqlx::query("UPDATE message SET deleted_datetime = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(&now)
        .bind(&now)
        .bind(&input.message_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to delete message: {}", e))?;

    crate::services::action_service::on_deleted(window.app_handle().clone(), input.message_id.clone());
    let messages_response = get_current_messages(service.clone(), window.clone()).await?;
    window.emit("messages-updated", &messages_response)
        .map_err(|e| format!("Failed to emit messages update: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn post_message_pin(
    service: State<'_, ChatnipService>,
    input: PinMessageInput,
) -> Result<(), String> {
    log_command!("post_message_pin", "message_id={}", input.message_id);
    crate::services::outbound_service::put_pin(service.db(), &input.message_id, true).await?;

    let pool = service.db();
    let pin_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT OR IGNORE INTO pinned_message (id, conversation_id, message_id, pinned_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5)"
    )
    .bind(&pin_id)
    .bind(&input.conversation_id)
    .bind(&input.message_id)
    .bind("current_user")
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to pin message: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_message_pin(
    service: State<'_, ChatnipService>,
    input: PinMessageInput,
) -> Result<(), String> {
    log_command!("delete_message_pin", "message_id={}", input.message_id);
    crate::services::outbound_service::put_pin(service.db(), &input.message_id, false).await?;

    let pool = service.db();

    sqlx::query("DELETE FROM pinned_message WHERE conversation_id = ?1 AND message_id = ?2")
        .bind(&input.conversation_id)
        .bind(&input.message_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to unpin message: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn get_pinned_messages(
    service: State<'_, ChatnipService>,
    input: GetPinnedInput,
) -> Result<Vec<PinnedMessage>, String> {
    log_command!("get_pinned_messages", "conversation_id={}", input.conversation_id);

    let pool = service.db();
    let message_service = MessageService::new(pool.clone());

    let pin_rows = sqlx::query(
        "SELECT id, conversation_id, message_id, pinned_by, created_at FROM pinned_message WHERE conversation_id = ?1 ORDER BY created_at DESC"
    )
    .bind(&input.conversation_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to get pinned messages: {}", e))?;

    let mut pinned: Vec<PinnedMessage> = Vec::new();
    for pin_row in pin_rows {
        let pm_id: String = pin_row.get("id");
        let pm_conversation_id: String = pin_row.get("conversation_id");
        let pm_message_id: String = pin_row.get("message_id");
        let pm_pinned_by: Option<String> = pin_row.get("pinned_by");
        let pm_created_at: String = pin_row.get("created_at");

        let msg_row = sqlx::query("SELECT * FROM message WHERE id = ?1")
            .bind(&pm_message_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to get pinned message content: {}", e))?;

        let message = msg_row.map(|r| message_service.row_to_message(r));

        pinned.push(PinnedMessage {
            id: pm_id,
            conversation_id: pm_conversation_id,
            message_id: pm_message_id,
            pinned_by: pm_pinned_by,
            created_at: pm_created_at,
            message,
        });
    }

    Ok(pinned)
}

#[tauri::command]
pub async fn post_bookmark(
    service: State<'_, ChatnipService>,
    input: BookmarkMessageInput,
) -> Result<(), String> {
    log_command!("post_bookmark", "message_id={}", input.message_id);

    let pool = service.db();
    let bookmark_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let profile_id = get_current_profile_id(pool).await?;

    sqlx::query(
        "INSERT OR IGNORE INTO bookmarked_message (id, message_id, profile_id, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5)"
    )
    .bind(&bookmark_id)
    .bind(&input.message_id)
    .bind(&profile_id)
    .bind(&input.note)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to bookmark message: {}", e))?;
    if let Err(e) = crate::services::outbound_service::put_flag(pool, &input.message_id, true).await {
        eprintln!("The flag was not set on the provider: {}", e);
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_bookmark(
    service: State<'_, ChatnipService>,
    input: UnbookmarkMessageInput,
) -> Result<(), String> {
    log_command!("delete_bookmark", "message_id={}", input.message_id);

    let pool = service.db();
    let profile_id = get_current_profile_id(pool).await?;

    sqlx::query("DELETE FROM bookmarked_message WHERE message_id = ?1 AND profile_id = ?2")
        .bind(&input.message_id)
        .bind(&profile_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to unbookmark message: {}", e))?;
    if let Err(e) = crate::services::outbound_service::put_flag(pool, &input.message_id, false).await {
        eprintln!("The flag was not cleared on the provider: {}", e);
    }

    Ok(())
}

#[tauri::command]
pub async fn get_bookmarks(
    service: State<'_, ChatnipService>,
) -> Result<Vec<BookmarkedMessage>, String> {
    log_command!("get_bookmarks");

    let pool = service.db();
    let profile_id = get_current_profile_id(pool).await?;
    let message_service = MessageService::new(pool.clone());

    let bm_rows = sqlx::query(
        "SELECT id, message_id, profile_id, note, created_at FROM bookmarked_message WHERE profile_id = ?1 ORDER BY created_at DESC"
    )
    .bind(&profile_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to get bookmarked messages: {}", e))?;

    let mut bookmarked: Vec<BookmarkedMessage> = Vec::new();
    for bm_row in bm_rows {
        let bm_id: String = bm_row.get("id");
        let bm_message_id: String = bm_row.get("message_id");
        let bm_profile_id: String = bm_row.get("profile_id");
        let bm_note: Option<String> = bm_row.get("note");
        let bm_created_at: String = bm_row.get("created_at");

        let msg_row = sqlx::query("SELECT * FROM message WHERE id = ?1")
            .bind(&bm_message_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to get bookmarked message content: {}", e))?;

        let message = msg_row.map(|r| message_service.row_to_message(r));

        bookmarked.push(BookmarkedMessage {
            id: bm_id,
            message_id: bm_message_id,
            profile_id: bm_profile_id,
            note: bm_note,
            created_at: bm_created_at,
            message,
        });
    }

    Ok(bookmarked)
}

#[tauri::command]
pub async fn put_message_unread(
    service: State<'_, ChatnipService>,
    window: Window,
    input: MarkMessageUnreadInput,
) -> Result<(), String> {
    log_command!("put_message_unread", "message_id={}", input.message_id);

    let pool = service.db();
    let now = Utc::now().to_rfc3339();

    let msg_row = sqlx::query("SELECT conversation_id, timestamp FROM message WHERE id = ?1")
        .bind(&input.message_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to find message: {}", e))?
        .ok_or_else(|| "Message not found".to_string())?;

    let conversation_id: String = msg_row.get("conversation_id");
    let timestamp: String = msg_row.get("timestamp");

    let marked: Vec<String> = sqlx::query_scalar("SELECT id FROM message WHERE conversation_id = ?1 AND timestamp >= ?2 AND deleted_datetime IS NULL")
        .bind(&conversation_id)
        .bind(&timestamp)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list the messages: {}", e))?;
    sqlx::query("UPDATE message SET is_read = 0, updated_at = ?1 WHERE conversation_id = ?2 AND timestamp >= ?3")
        .bind(&now)
        .bind(&conversation_id)
        .bind(&timestamp)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to mark messages unread: {}", e))?;
    if let Err(e) = crate::services::outbound_service::mark_read(pool, &marked, false).await {
        eprintln!("Unread state not sent to the provider: {}", e);
    }

    let unread_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM message WHERE conversation_id = ?1 AND is_read = 0")
        .bind(&conversation_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to count unread: {}", e))?
        .get("count");

    sqlx::query("UPDATE conversation SET unread_count = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(unread_count)
        .bind(&now)
        .bind(&conversation_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update unread count: {}", e))?;

    let messages_response = get_current_messages(service.clone(), window.clone()).await?;
    window.emit("messages-updated", &messages_response)
        .map_err(|e| format!("Failed to emit messages update: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn put_conversation_read(
    service: State<'_, ChatnipService>,
    window: Window,
    input: MarkConversationReadInput,
) -> Result<(), String> {
    log_command!("put_conversation_read", "conversation_id={}", input.conversation_id);

    let pool = service.db();
    let now = Utc::now().to_rfc3339();

    let unread: Vec<String> = sqlx::query_scalar("SELECT id FROM message WHERE conversation_id = ?1 AND is_read = 0 AND deleted_datetime IS NULL")
        .bind(&input.conversation_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list unread messages: {}", e))?;

    sqlx::query("UPDATE message SET is_read = 1, updated_at = ?1 WHERE conversation_id = ?2 AND is_read = 0")
        .bind(&now)
        .bind(&input.conversation_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to mark messages read: {}", e))?;

    sqlx::query("UPDATE conversation SET unread_count = 0, updated_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(&input.conversation_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update unread count: {}", e))?;
    if let Err(e) = crate::services::outbound_service::mark_read(pool, &unread, true).await {
        eprintln!("Read state not sent to the provider: {}", e);
    }

    let messages_response = get_current_messages(service.clone(), window.clone()).await?;
    window.emit("messages-updated", &messages_response)
        .map_err(|e| format!("Failed to emit messages update: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn put_conversation_notification(
    service: State<'_, ChatnipService>,
    input: SetNotificationInput,
) -> Result<(), String> {
    log_command!("put_conversation_notification", "conversation_id={} level={}", input.conversation_id, input.notification_level);

    let pool = service.db();
    let now = Utc::now().to_rfc3339();

    sqlx::query("UPDATE conversation SET notification_level = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(&input.notification_level)
        .bind(&now)
        .bind(&input.conversation_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to set notification level: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn post_sidebar_section(
    service: State<'_, ChatnipService>,
    input: CreateSidebarSectionInput,
) -> Result<SidebarSection, String> {
    log_command!("post_sidebar_section", "name={}", input.name);

    let pool = service.db();
    let profile_id = get_current_profile_id(pool).await?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let position = input.position.unwrap_or(0);

    sqlx::query(
        "INSERT INTO sidebar_section (id, profile_id, name, position, created_at) VALUES (?1, ?2, ?3, ?4, ?5)"
    )
    .bind(&id)
    .bind(&profile_id)
    .bind(&input.name)
    .bind(position)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create sidebar section: {}", e))?;

    Ok(SidebarSection {
        id,
        profile_id,
        name: input.name,
        position,
        is_collapsed: false,
        conversations: vec![],
        created_at: now,
    })
}

#[tauri::command]
pub async fn put_sidebar_section(
    service: State<'_, ChatnipService>,
    input: UpdateSidebarSectionInput,
) -> Result<(), String> {
    log_command!("put_sidebar_section", "id={}", input.id);

    let pool = service.db();

    if let Some(name) = &input.name {
        sqlx::query("UPDATE sidebar_section SET name = ?1 WHERE id = ?2")
            .bind(name)
            .bind(&input.id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update sidebar section name: {}", e))?;
    }

    if let Some(position) = input.position {
        sqlx::query("UPDATE sidebar_section SET position = ?1 WHERE id = ?2")
            .bind(position)
            .bind(&input.id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update sidebar section position: {}", e))?;
    }

    if let Some(is_collapsed) = input.is_collapsed {
        sqlx::query("UPDATE sidebar_section SET is_collapsed = ?1 WHERE id = ?2")
            .bind(is_collapsed)
            .bind(&input.id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update sidebar section collapsed: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_sidebar_section(
    service: State<'_, ChatnipService>,
    input: DeleteSidebarSectionInput,
) -> Result<(), String> {
    log_command!("delete_sidebar_section", "id={}", input.id);

    let pool = service.db();

    sqlx::query("DELETE FROM sidebar_section WHERE id = ?1")
        .bind(&input.id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to delete sidebar section: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn post_sidebar_section_item(
    service: State<'_, ChatnipService>,
    input: SidebarSectionItemInput,
) -> Result<(), String> {
    log_command!("post_sidebar_section_item", "section={} conversation={}", input.section_id, input.conversation_id);

    let pool = service.db();
    let id = Uuid::new_v4().to_string();
    let position = input.position.unwrap_or(0);

    sqlx::query(
        "INSERT OR IGNORE INTO sidebar_section_item (id, section_id, conversation_id, position) VALUES (?1, ?2, ?3, ?4)"
    )
    .bind(&id)
    .bind(&input.section_id)
    .bind(&input.conversation_id)
    .bind(position)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to add to sidebar section: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_sidebar_section_item(
    service: State<'_, ChatnipService>,
    input: RemoveSidebarSectionItemInput,
) -> Result<(), String> {
    log_command!("delete_sidebar_section_item", "section={} conversation={}", input.section_id, input.conversation_id);

    let pool = service.db();

    sqlx::query("DELETE FROM sidebar_section_item WHERE section_id = ?1 AND conversation_id = ?2")
        .bind(&input.section_id)
        .bind(&input.conversation_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to remove from sidebar section: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn get_sidebar_sections(
    service: State<'_, ChatnipService>,
) -> Result<Vec<SidebarSection>, String> {
    log_command!("get_sidebar_sections");

    let pool = service.db();
    let profile_id = get_current_profile_id(pool).await?;

    let rows = sqlx::query(
        "SELECT * FROM sidebar_section WHERE profile_id = ?1 ORDER BY position ASC"
    )
    .bind(&profile_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to get sidebar sections: {}", e))?;

    let mut sections = Vec::new();
    for row in rows {
        let section_id: String = row.get("id");

        let conv_rows = sqlx::query(
            r#"SELECT c.* FROM conversation c
               INNER JOIN sidebar_section_item si ON si.conversation_id = c.id
               WHERE si.section_id = ?1
               ORDER BY si.position ASC"#
        )
        .bind(&section_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to get section conversations: {}", e))?;

        let mut conversations = Vec::new();
        for conv_row in conv_rows {
            let mut conversation = Conversation::from_row(conv_row)?;
            if let Some(loaded) = Conversation::load_with_providers(pool, &conversation.id).await? {
                conversation = loaded;
            }
            conversations.push(conversation);
        }

        sections.push(SidebarSection {
            id: section_id,
            profile_id: row.get("profile_id"),
            name: row.get("name"),
            position: row.get("position"),
            is_collapsed: row.try_get("is_collapsed").unwrap_or(false),
            conversations,
            created_at: row.get("created_at"),
        });
    }

    Ok(sections)
}

#[tauri::command]
pub async fn put_user_status(
    service: State<'_, ChatnipService>,
    input: SetUserStatusInput,
) -> Result<(), String> {
    log_command!("put_user_status");

    let pool = service.db();
    let profile = Profile::get_active(pool).await?
        .ok_or_else(|| "No active profile found".to_string())?;
    let account = match input.provider_id.as_deref() {
        Some(provider_id) => crate::models::Account::get(pool, provider_id).await?.map(|a| a.user_id),
        None => None,
    };
    let user_id = account.or(profile.user_id.clone())
        .ok_or_else(|| "No user associated with profile".to_string())?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO user_status (user_id, status, status_text, status_emoji, status_expiration, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(user_id) DO UPDATE SET status = ?2, status_text = ?3, status_emoji = ?4, status_expiration = ?5, updated_at = ?6"
    )
    .bind(&user_id)
    .bind(&input.status)
    .bind(&input.status_text)
    .bind(&input.status_emoji)
    .bind(&input.status_expiration)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to set user status: {}", e))?;
    let until = input.status_expiration.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc));
    let emoji = Some(input.status_emoji.as_str()).filter(|e| !e.trim().is_empty());
    let providers: Vec<String> = sqlx::query_scalar("SELECT id FROM provider WHERE profile_id = ?1 AND is_enabled = 1 AND access_token IS NOT NULL AND (?2 IS NULL OR id = ?2)")
        .bind(&profile.id)
        .bind(&input.provider_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list the connections: {}", e))?;
    for provider_id in providers {
        match crate::services::connection_service::connected(pool, &provider_id).await {
            Ok((provider, client, auth)) => {
                match client.put_status(&auth, &input.status_text, emoji, until).await {
                    Ok(true) if input.provider_id.is_some() => Provider::put_last_used(pool, &provider.id).await?,
                    Ok(_) => {}
                    Err(e) if input.provider_id.is_some() => return Err(format!("Status not set on {}: {}", provider.name, e)),
                    Err(e) => eprintln!("Status not set on {}: {}", provider.name, e),
                }
            }
            Err(e) if input.provider_id.is_some() => return Err(e),
            Err(e) => eprintln!("Status not set on {}: {}", provider_id, e),
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_user_status(
    service: State<'_, ChatnipService>,
    input: GetUserStatusInput,
) -> Result<UserStatus, String> {
    log_command!("get_user_status", "user_id={}", input.user_id);

    let pool = service.db();

    let row = sqlx::query("SELECT * FROM user_status WHERE user_id = ?1")
        .bind(&input.user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to get user status: {}", e))?;

    match row {
        Some(row) => Ok(UserStatus {
            user_id: row.get("user_id"),
            status: row.get("status"),
            status_text: row.get("status_text"),
            status_emoji: row.get("status_emoji"),
            status_expiration: row.get("status_expiration"),
            updated_at: row.get("updated_at"),
        }),
        None => Ok(UserStatus {
            user_id: input.user_id,
            status: "offline".to_string(),
            status_text: String::new(),
            status_emoji: String::new(),
            status_expiration: None,
            updated_at: Utc::now().to_rfc3339(),
        }),
    }
}

#[tauri::command]
pub async fn get_tags(service: State<'_, ChatnipService>) -> Result<Vec<crate::models::Tag>, String> {
    let profile_id = get_current_profile_id(service.db()).await?;
    crate::models::Tag::ensure_defaults(service.db(), &profile_id).await?;
    crate::models::Tag::all(service.db(), &profile_id).await
}

#[tauri::command]
pub async fn post_tag(service: State<'_, ChatnipService>, input: crate::models::PostTagInput) -> Result<crate::models::Tag, String> {
    let profile_id = get_current_profile_id(service.db()).await?;
    let kind = input.kind.unwrap_or_else(|| crate::models::tag::KIND_TAG.to_string());
    crate::models::Tag::post(service.db(), &profile_id, &input.name, input.color.as_deref().unwrap_or("#8c93a3"), &kind, input.position.unwrap_or(99), input.description.as_deref().unwrap_or("")).await
}

#[tauri::command]
pub async fn put_tag(service: State<'_, ChatnipService>, input: crate::models::PutTagInput) -> Result<crate::models::Tag, String> {
    let label = crate::models::Tag::put(service.db(), &input.tag_id, input.name.as_deref(), input.color.as_deref(), input.position, input.description.as_deref(), input.icon.as_deref()).await?;
    match input.favourite {
        Some(favourite) if favourite != label.favourite => crate::models::Tag::put_favourite(service.db(), &label.id, favourite).await,
        _ => Ok(label),
    }
}

#[tauri::command]
pub async fn put_tag_favourites(service: State<'_, ChatnipService>, order: Vec<String>) -> Result<Vec<crate::models::Tag>, String> {
    let profile = crate::models::Profile::get_active(service.db()).await?.ok_or_else(|| "No active profile".to_string())?;
    crate::models::Tag::put_favourite_order(service.db(), &profile.id, &order).await?;
    crate::models::Tag::all(service.db(), &profile.id).await
}

#[tauri::command]
pub async fn delete_tag(service: State<'_, ChatnipService>, tag_id: String) -> Result<(), String> {
    crate::models::Tag::delete(service.db(), &tag_id).await
}

async fn emit_object_tags(service: &State<'_, ChatnipService>, window: &Window, object_type: &str) -> Result<(), String> {
    window.emit("object-tags-updated", object_type).map_err(|e| format!("Failed to emit tags update: {}", e))?;
    if [crate::models::tag::OBJECT_CONVERSATION, crate::models::tag::OBJECT_PERSON, crate::models::tag::OBJECT_IDENTITY].contains(&object_type) {
        let conversations = get_conversations(service.clone()).await?;
        window.emit("conversations-updated", &conversations).map_err(|e| format!("Failed to emit conversations update: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_object_tags(service: State<'_, ChatnipService>, object_type: Option<String>, object_id: Option<String>) -> Result<Vec<crate::models::ObjectTag>, String> {
    match (object_type.as_deref(), object_id.as_deref()) {
        (Some(crate::models::tag::OBJECT_CONVERSATION), Some(id)) => crate::models::ObjectTag::get_conversation(service.db(), id).await,
        (Some(kind), Some(id)) => crate::models::ObjectTag::get(service.db(), kind, id).await,
        (kind, _) => {
            let profile_id = get_current_profile_id(service.db()).await?;
            crate::models::ObjectTag::get_all(service.db(), &profile_id, kind).await
        }
    }
}

#[tauri::command]
pub async fn get_tagged_objects(service: State<'_, ChatnipService>, tag_id: Option<String>) -> Result<Vec<crate::models::TaggedObject>, String> {
    let profile_id = get_current_profile_id(service.db()).await?;
    crate::models::ObjectTag::get_tagged(service.db(), &profile_id, tag_id.as_deref()).await
}

#[tauri::command]
pub async fn put_object_tag(service: State<'_, ChatnipService>, window: Window, input: crate::models::ObjectTagInput) -> Result<Vec<crate::models::ObjectTag>, String> {
    crate::models::ObjectTag::put(service.db(), &input.object_type, &input.object_id, &input.tag_id, crate::services::sorting_service::SOURCE_MANUAL, None, None).await?;
    emit_object_tags(&service, &window, &input.object_type).await?;
    crate::models::ObjectTag::get(service.db(), &input.object_type, &input.object_id).await
}

#[tauri::command]
pub async fn delete_object_tag(service: State<'_, ChatnipService>, window: Window, input: crate::models::ObjectTagInput) -> Result<Vec<crate::models::ObjectTag>, String> {
    crate::models::ObjectTag::delete(service.db(), &input.object_type, &input.object_id, &input.tag_id).await?;
    emit_object_tags(&service, &window, &input.object_type).await?;
    crate::models::ObjectTag::get(service.db(), &input.object_type, &input.object_id).await
}

#[tauri::command]
pub async fn get_assistant_runs(service: State<'_, ChatnipService>, limit: Option<i64>) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query("SELECT id, kind, model, output, cost_usd, duration_ms, created_at FROM assistant_run ORDER BY created_at DESC LIMIT ?1")
        .bind(limit.unwrap_or(50))
        .fetch_all(service.db())
        .await
        .map_err(|e| format!("Failed to list assistant runs: {}", e))?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let output: String = r.get("output");
            serde_json::json!({
                "id": r.get::<String, _>("id"),
                "kind": r.get::<String, _>("kind"),
                "model": r.get::<String, _>("model"),
                "output": serde_json::from_str::<serde_json::Value>(&output).unwrap_or(serde_json::Value::Null),
                "cost_usd": r.get::<f64, _>("cost_usd"),
                "duration_ms": r.get::<i64, _>("duration_ms"),
                "created_at": r.get::<String, _>("created_at"),
            })
        })
        .collect())
}

#[tauri::command]
pub async fn get_events(service: State<'_, ChatnipService>, input: crate::models::EventRangeInput) -> Result<Vec<crate::models::Event>, String> {
    let profile_id = get_current_profile_id(service.db()).await?;
    crate::services::calendar_service::merged_between(service.db(), &profile_id, &input.start, &input.end, input.include_dismissed.unwrap_or(false)).await
}

#[tauri::command]
pub async fn get_proposed_events(service: State<'_, ChatnipService>) -> Result<Vec<crate::models::Event>, String> {
    let profile_id = get_current_profile_id(service.db()).await?;
    crate::models::Event::proposed(service.db(), &profile_id).await
}

#[tauri::command]
pub async fn post_event(service: State<'_, ChatnipService>, window: Window, input: crate::models::PostEventInput) -> Result<crate::models::PostEventResult, String> {
    let profile_id = get_current_profile_id(service.db()).await?;
    let pool = service.db().clone();
    let connect = move |provider_id: String| -> futures::future::BoxFuture<'static, Result<(crate::providers::models::Provider, std::sync::Arc<dyn crate::providers::traits::ProviderTrait>, crate::providers::models::ProviderAuth), String>> {
        let pool = pool.clone();
        Box::pin(async move { crate::services::connection_service::connected(&pool, &provider_id).await })
    };
    let (events, failures) = crate::services::calendar_service::post_to_targets(service.db(), &profile_id, &input.draft, &input.targets, input.online_meeting, connect).await?;
    window.emit("events-updated", events.len()).map_err(|e| format!("Failed to emit events update: {}", e))?;
    Ok(crate::models::PostEventResult { events, failures })
}

#[tauri::command]
pub async fn get_calendars(service: State<'_, ChatnipService>) -> Result<Vec<crate::models::calendar::Calendar>, String> {
    let profile_id = get_current_profile_id(service.db()).await?;
    crate::models::calendar::Calendar::for_profile(service.db(), &profile_id).await
}

#[tauri::command]
pub async fn put_calendar(service: State<'_, ChatnipService>, window: Window, calendar_id: String, enabled: bool) -> Result<crate::models::calendar::Calendar, String> {
    let profile_id = get_current_profile_id(service.db()).await?;
    let calendar = crate::models::calendar::Calendar::get(service.db(), &calendar_id).await?;
    if calendar.profile_id != profile_id {
        return Err("That calendar belongs to another profile".to_string());
    }
    let calendar = crate::models::calendar::Calendar::put_enabled(service.db(), &calendar_id, enabled).await?;
    window.emit("events-updated", &calendar.id).map_err(|e| format!("Failed to emit events update: {}", e))?;
    Ok(calendar)
}

#[tauri::command]
pub async fn put_event_merge(service: State<'_, ChatnipService>, window: Window, event_ids: Vec<String>, merged: bool) -> Result<(), String> {
    if merged {
        crate::models::Event::put_link(service.db(), &event_ids, Some(&uuid::Uuid::new_v4().to_string()), false).await?;
    } else {
        crate::services::calendar_service::unmerge(service.db(), &event_ids).await?;
    }
    window.emit("events-updated", event_ids.len()).map_err(|e| format!("Failed to emit events update: {}", e))
}

#[tauri::command]
pub async fn put_event(service: State<'_, ChatnipService>, window: Window, input: crate::models::PutEventInput) -> Result<crate::models::Event, String> {
    let event = crate::models::Event::put(service.db(), &input.event_id, input.draft.as_ref(), input.status.as_deref()).await?;
    window.emit("events-updated", &event).map_err(|e| format!("Failed to emit events update: {}", e))?;
    Ok(event)
}

#[tauri::command]
pub async fn put_event_response(service: State<'_, ChatnipService>, window: Window, event_id: String, response: String) -> Result<crate::models::Event, String> {
    let event = crate::services::calendar_service::put_response(service.db(), &event_id, &response).await?;
    window.emit("events-updated", &event).map_err(|e| format!("Failed to emit events update: {}", e))?;
    Ok(event)
}

#[tauri::command]
pub async fn delete_event(service: State<'_, ChatnipService>, window: Window, event_id: String) -> Result<(), String> {
    crate::models::Event::delete(service.db(), &event_id).await?;
    window.emit("events-updated", &event_id).map_err(|e| format!("Failed to emit events update: {}", e))
}

#[tauri::command]
pub async fn get_calendar_export(service: State<'_, ChatnipService>, window: Window) -> Result<String, String> {
    let profile_id = get_current_profile_id(service.db()).await?;
    let feed = crate::services::calendar_service::ics(service.db(), &profile_id).await?;
    let dir = window.app_handle().path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create app data dir: {}", e))?;
    let path = dir.join("chatnip.ics");
    std::fs::write(&path, feed).map_err(|e| format!("Failed to write the calendar feed: {}", e))?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn post_export(window: Window, name: String, content: String) -> Result<String, String> {
    let dir = window.app_handle().path().download_dir().map_err(|e| format!("Failed to find the downloads folder: {}", e))?;
    let clean: String = name.chars().map(|c| if c.is_alphanumeric() || " .-_".contains(c) { c } else { '_' }).collect();
    let path = std::path::Path::new(clean.trim());
    let stem = path.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()).unwrap_or("chatnip");
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("md");
    let mut target = dir.join(format!("{}.{}", stem, extension));
    let mut copy = 2;
    while target.exists() {
        target = dir.join(format!("{} {}.{}", stem, copy, extension));
        copy += 1;
    }
    std::fs::write(&target, content).map_err(|e| format!("Failed to write the export: {}", e))?;
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn get_history(service: State<'_, ChatnipService>, limit: Option<i64>) -> Result<Vec<crate::models::HistoryEntry>, String> {
    crate::models::HistoryEntry::get(service.db(), &crate::models::Profile::get_active_id(service.db()).await?, limit.unwrap_or(200)).await
}

#[tauri::command]
pub async fn post_history(service: State<'_, ChatnipService>, app: AppHandle, input: crate::models::PostHistoryInput) -> Result<crate::models::HistoryEntry, String> {
    crate::services::history_service::record(service.db(), Some(&app), input).await
}

#[tauri::command]
pub async fn put_history(service: State<'_, ChatnipService>, app: AppHandle, history_id: String, option: String) -> Result<crate::models::HistoryEntry, String> {
    crate::services::history_service::put_option(service.db(), &app, &history_id, &option).await
}
