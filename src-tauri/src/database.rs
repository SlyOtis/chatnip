use tauri::{AppHandle, Manager};
use sqlx::{Pool, Sqlite};
pub async fn create_database_pool(app: &AppHandle) -> Result<Pool<Sqlite>, Box<dyn std::error::Error + Send + Sync>> {
    let app_data_dir = app.path().app_data_dir()?;
    println!("=� App data directory: {:?}", app_data_dir);
    std::fs::create_dir_all(&app_data_dir)?;
    let db_path = app_data_dir.join("chatnip.db");
    println!("=� Database path: {:?}", db_path);
    if !db_path.exists() {
        println!("<� Creating new database file...");
        std::fs::File::create(&db_path)?;
    } else {
        println!(" Database file already exists");
        use std::os::unix::fs::MetadataExt;
        if let Ok(metadata) = std::fs::metadata(&db_path) {
            let mode = metadata.mode();
            println!("= Database file permissions: {:o}", mode & 0o777);
            if metadata.permissions().readonly() {
                println!("� WARNING: Database file is readonly!");
                let mut perms = metadata.permissions();
                perms.set_readonly(false);
                if let Err(e) = std::fs::set_permissions(&db_path, perms) {
                    println!("L Failed to set write permissions: {}", e);
                } else {
                    println!(" Successfully set write permissions");
                }
            }
        }
    }
    let database_url = format!("sqlite:{}?mode=rwc", db_path.display());
    println!("= Database URL: {}", database_url);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(60))
        .idle_timeout(std::time::Duration::from_secs(300))
        .max_lifetime(std::time::Duration::from_secs(1800))
        .connect(&database_url).await?;
    println!(" Successfully connected to database");
    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("L Failed to set journal_mode: {}", e);
            e
        })?;
    sqlx::query("PRAGMA synchronous = NORMAL")
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("L Failed to set synchronous: {}", e);
            e
        })?;
    sqlx::query("PRAGMA busy_timeout = 60000")
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("L Failed to set busy_timeout: {}", e);
            e
        })?;
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("L Failed to checkpoint WAL: {}", e);
            e
        })?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("L Failed to enable foreign_keys: {}", e);
            e
        })?;
    init_database(&pool).await?;
    Ok(pool)
}
const MESSAGE_COLUMNS: [(&str, &str); 43] = [
    ("presentation", "TEXT"),
    ("type", "TEXT NOT NULL DEFAULT 'chat'"),
    ("subject", "TEXT"),
    ("content", "TEXT"),
    ("content_type", "TEXT DEFAULT 'html'"),
    ("summary", "TEXT"),
    ("sender_id", "TEXT"),
    ("sender_name", "TEXT"),
    ("sender_email", "TEXT"),
    ("recipients", "TEXT"),
    ("cc_recipients", "TEXT"),
    ("bcc_recipients", "TEXT"),
    ("conversation_id", "TEXT"),
    ("chat_id", "TEXT"),
    ("team_id", "TEXT"),
    ("channel_id", "TEXT"),
    ("importance", "TEXT DEFAULT 'normal'"),
    ("is_read", "BOOLEAN DEFAULT 0"),
    ("is_draft", "BOOLEAN DEFAULT 0"),
    ("has_attachments", "BOOLEAN DEFAULT 0"),
    ("attachments", "TEXT"),
    ("thread_id", "TEXT"),
    ("reply_to_id", "TEXT"),
    ("conversation_index", "TEXT"),
    ("created_datetime", "TEXT NOT NULL DEFAULT ''"),
    ("last_modified_datetime", "TEXT NOT NULL DEFAULT ''"),
    ("received_datetime", "TEXT"),
    ("sent_datetime", "TEXT"),
    ("deleted_datetime", "TEXT"),
    ("timestamp", "TEXT NOT NULL DEFAULT ''"),
    ("cached_at", "TEXT"),
    ("created_at", "TEXT NOT NULL DEFAULT ''"),
    ("updated_at", "TEXT NOT NULL DEFAULT ''"),
    ("provider_name", "TEXT"),
    ("metadata", "TEXT"),
    ("scanned_for_events", "BOOLEAN NOT NULL DEFAULT 0"),
    ("headers", "TEXT"),
    ("origin_ip", "TEXT"),
    ("origin_lat", "REAL"),
    ("origin_lon", "REAL"),
    ("origin_city", "TEXT"),
    ("origin_country", "TEXT"),
    ("origin_resolved", "BOOLEAN NOT NULL DEFAULT 0"),
];

const EVENT_COLUMNS: [(&str, &str); 10] = [
    ("provider_id", "TEXT"),
    ("calendar_id", "TEXT"),
    ("external_id", "TEXT"),
    ("ical_uid", "TEXT"),
    ("online_meeting_url", "TEXT"),
    ("organizer", "TEXT"),
    ("response_status", "TEXT"),
    ("link_id", "TEXT"),
    ("unmerged", "BOOLEAN NOT NULL DEFAULT 0"),
    ("remote_updated_at", "TEXT"),
];

const CONVERSATION_COLUMNS: [(&str, &str); 21] = [
    ("mode", "TEXT NOT NULL DEFAULT 'write'"),
    ("view", "TEXT NOT NULL DEFAULT 'chat'"),
    ("view_settings", "TEXT NOT NULL DEFAULT '{}'"),
    ("presentation", "TEXT"),
    ("namespace", "TEXT"),
    ("group_type", "TEXT NOT NULL DEFAULT 'auto'"),
    ("last_id", "TEXT"),
    ("metadata", "TEXT"),
    ("unread_count", "INTEGER DEFAULT 0"),
    ("is_current", "BOOLEAN DEFAULT 0"),
    ("is_pinned", "BOOLEAN DEFAULT 0"),
    ("is_muted", "BOOLEAN DEFAULT 0"),
    ("summary", "TEXT"),
    ("notification_level", "TEXT DEFAULT 'all'"),
    ("is_archived", "BOOLEAN NOT NULL DEFAULT 0"),
    ("snoozed_until", "TEXT"),
    ("deleted_at", "TEXT"),
    ("person_id", "TEXT"),
    ("favourite_position", "INTEGER"),
    ("created_at", "TEXT NOT NULL DEFAULT ''"),
    ("updated_at", "TEXT NOT NULL DEFAULT ''"),
];

pub async fn ensure_columns(pool: &Pool<Sqlite>, table: &str, columns: &[(&str, &str)]) {
    let present: Vec<(String,)> = sqlx::query_as(&format!("SELECT name FROM pragma_table_info('{}')", table))
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    for (column, definition) in columns {
        if present.iter().any(|(name,)| name == column) {
            continue;
        }
        if let Err(e) = sqlx::query(&format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, definition)).execute(pool).await {
            eprintln!("Could not add {}.{}: {}", table, column, e);
        }
    }
}

pub async fn init_database(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    println!("=� Initializing database tables...");
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS profile (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            user_id TEXT,
            avatar_id TEXT,
            theme_id TEXT,
            setup_step INTEGER DEFAULT 0,
            active BOOLEAN DEFAULT 0,
            last_id TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user (id),
            FOREIGN KEY (avatar_id) REFERENCES assets (id),
            FOREIGN KEY (theme_id) REFERENCES profile_theme (id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS provider (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            provider_type TEXT NOT NULL,
            name TEXT NOT NULL,
            external_id TEXT,
            logo_id TEXT,
            capabilities TEXT NOT NULL,
            is_enabled BOOLEAN DEFAULT 1,
            access_token TEXT,
            refresh_token TEXT,
            expires_at TEXT,
            token_type TEXT DEFAULT 'Bearer',
            scope TEXT,
            sync_interval INTEGER DEFAULT 300,
            namespace TEXT,
            delta_token TEXT,
            last_sync_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(profile_id, provider_type, namespace),
            FOREIGN KEY (profile_id) REFERENCES profile (id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS conversation (
            id TEXT PRIMARY KEY,
            namespace TEXT,
            name TEXT NOT NULL,
            type TEXT NOT NULL,
            group_type TEXT NOT NULL DEFAULT 'auto',
            last_id TEXT,
            last_activity TEXT NOT NULL,
            metadata TEXT,
            unread_count INTEGER DEFAULT 0,
            is_current BOOLEAN DEFAULT 0,
            is_pinned BOOLEAN DEFAULT 0,
            is_muted BOOLEAN DEFAULT 0,
            summary TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS message (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            external_id TEXT NOT NULL,

            -- Message Type: email, chat, channel, other
            type TEXT NOT NULL DEFAULT 'chat',

            -- Common fields
            subject TEXT,
            content TEXT,                   -- Main body or message content
            content_type TEXT DEFAULT 'html',  -- html, text, etc.
            summary TEXT,                  -- Auto-generated summary
            
            -- Sender info
            sender_id TEXT,
            sender_name TEXT,
            sender_email TEXT,
            
            -- Recipients info (mainly for emails; empty or null for chats)
            recipients TEXT,              -- JSON array of recipient objects (name, email)
            cc_recipients TEXT,
            bcc_recipients TEXT,
            
            -- Chat/Teams specific fields
            conversation_id TEXT,          -- Chat/conversation id for chat messages
            chat_id TEXT,                  -- Alternative chat id field
            team_id TEXT,                  -- Team id for channel messages
            channel_id TEXT,               -- Channel id for channel messages

            -- Metadata
            importance TEXT DEFAULT 'normal',

            -- Flags
            is_read BOOLEAN DEFAULT 0,
            is_draft BOOLEAN DEFAULT 0,
            has_attachments BOOLEAN DEFAULT 0,
            attachments TEXT,              -- JSON array of attachment objects

            -- Threading
            thread_id TEXT,
            reply_to_id TEXT,
            conversation_index TEXT,
            
            -- Timestamps
            created_datetime TEXT NOT NULL,
            last_modified_datetime TEXT NOT NULL,
            received_datetime TEXT,        -- For emails
            sent_datetime TEXT,            -- For emails
            deleted_datetime TEXT,
            timestamp TEXT NOT NULL,       -- Backwards compatibility

            -- Sync metadata
            cached_at TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

            -- Legacy fields for backwards compatibility
            provider_name TEXT,
            metadata TEXT,

            UNIQUE(provider_id, external_id),
            FOREIGN KEY (conversation_id) REFERENCES conversation (id),
            FOREIGN KEY (provider_id) REFERENCES provider (id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user (
            id TEXT PRIMARY KEY,
            
            -- Unified user info (merged from all providers)
            email TEXT,                        -- Primary email (unique identifier)
            phone TEXT,                        -- Primary phone (can also be unique identifier)
            display_name TEXT,
            first_name TEXT,
            last_name TEXT,
            avatar_url TEXT,
            avatar_data TEXT,
            job_title TEXT,
            department TEXT,
            company TEXT,
            office_location TEXT,
            
            -- Metadata
            first_seen TEXT NOT NULL,
            last_updated TEXT NOT NULL,
            
            -- Ensure we can lookup by email or phone
            UNIQUE(email),
            UNIQUE(phone)
        )
        "#,
    ).execute(pool).await?;
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS provider_user (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,              -- References the unified user record
            provider_id TEXT NOT NULL,
            external_id TEXT NOT NULL,          -- The ID from the provider's system
            
            -- User info fields (mirrors user table structure)
            email TEXT,
            phone TEXT,
            display_name TEXT,
            first_name TEXT,
            last_name TEXT,
            avatar_url TEXT,
            avatar_data TEXT,
            
            -- Additional fields
            job_title TEXT,
            department TEXT,
            company TEXT,
            office_location TEXT,
            theme_color TEXT,
            
            -- Custom provider data
            details TEXT,                      -- JSON with any provider-specific fields
            
            -- Timestamps
            first_seen TEXT NOT NULL,          -- When this user was first encountered from this provider
            last_synced TEXT NOT NULL,         -- Last time data was synced from the provider
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            
            FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE,
            FOREIGN KEY (provider_id) REFERENCES provider (id) ON DELETE CASCADE,
            UNIQUE(provider_id, external_id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS assets (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            message_id TEXT,
            name TEXT NOT NULL,
            url TEXT,
            type TEXT NOT NULL,
            mime_type TEXT NOT NULL,
            path TEXT,
            size INTEGER,
            is_inline BOOLEAN DEFAULT 0,
            content_id TEXT,
            external_id TEXT,
            provider_id TEXT,
            download_url TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (profile_id) REFERENCES profile (id),
            FOREIGN KEY (message_id) REFERENCES message (id),
            FOREIGN KEY (provider_id) REFERENCES provider (id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS message_template (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            name TEXT NOT NULL,
            content TEXT NOT NULL,
            is_html BOOLEAN DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            usage_count INTEGER DEFAULT 0,
            FOREIGN KEY (profile_id) REFERENCES profile (id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS profile_setting (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            email_content_threshold INTEGER DEFAULT 500,
            navigation_width INTEGER DEFAULT 280,
            navigation_collapsed BOOLEAN DEFAULT 0,
            details_width INTEGER DEFAULT 320,
            details_panel_open BOOLEAN DEFAULT 0,
            window_x REAL,
            window_y REAL,
            window_width REAL,
            window_height REAL,
            window_monitor TEXT,
            window_scale REAL,
            window_maximized BOOLEAN DEFAULT 0,
            window_fullscreen BOOLEAN NOT NULL DEFAULT 0,
            message_timestamp_interval INTEGER DEFAULT 5,
            time_format TEXT DEFAULT '24h',
            window_controls_style TEXT DEFAULT 'macos',
            max_input_lines INTEGER DEFAULT 10,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (profile_id) REFERENCES profile (id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS profile_theme (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            theme_name TEXT NOT NULL,
            is_active BOOLEAN DEFAULT 0,
            primary_50 TEXT NOT NULL,
            primary_100 TEXT NOT NULL,
            primary_200 TEXT NOT NULL,
            primary_300 TEXT NOT NULL,
            primary_400 TEXT NOT NULL,
            primary_500 TEXT NOT NULL,
            primary_600 TEXT NOT NULL,
            primary_700 TEXT NOT NULL,
            primary_800 TEXT NOT NULL,
            primary_900 TEXT NOT NULL,
            primary_950 TEXT NOT NULL,
            secondary_50 TEXT NOT NULL,
            secondary_100 TEXT NOT NULL,
            secondary_200 TEXT NOT NULL,
            secondary_300 TEXT NOT NULL,
            secondary_400 TEXT NOT NULL,
            secondary_500 TEXT NOT NULL,
            secondary_600 TEXT NOT NULL,
            secondary_700 TEXT NOT NULL,
            secondary_800 TEXT NOT NULL,
            secondary_900 TEXT NOT NULL,
            secondary_950 TEXT NOT NULL,
            accent_50 TEXT NOT NULL,
            accent_100 TEXT NOT NULL,
            accent_200 TEXT NOT NULL,
            accent_300 TEXT NOT NULL,
            accent_400 TEXT NOT NULL,
            accent_500 TEXT NOT NULL,
            accent_600 TEXT NOT NULL,
            accent_700 TEXT NOT NULL,
            accent_800 TEXT NOT NULL,
            accent_900 TEXT NOT NULL,
            accent_950 TEXT NOT NULL,
            background_50 TEXT NOT NULL,
            background_100 TEXT NOT NULL,
            background_200 TEXT NOT NULL,
            background_300 TEXT NOT NULL,
            background_400 TEXT NOT NULL,
            background_500 TEXT NOT NULL,
            background_600 TEXT NOT NULL,
            background_700 TEXT NOT NULL,
            background_800 TEXT NOT NULL,
            background_900 TEXT NOT NULL,
            background_950 TEXT NOT NULL,
            content_50 TEXT NOT NULL,
            content_100 TEXT NOT NULL,
            content_200 TEXT NOT NULL,
            content_300 TEXT NOT NULL,
            content_400 TEXT NOT NULL,
            content_500 TEXT NOT NULL,
            content_600 TEXT NOT NULL,
            content_700 TEXT NOT NULL,
            content_800 TEXT NOT NULL,
            content_900 TEXT NOT NULL,
            content_950 TEXT NOT NULL,
            control_50 TEXT NOT NULL,
            control_100 TEXT NOT NULL,
            control_200 TEXT NOT NULL,
            control_300 TEXT NOT NULL,
            control_400 TEXT NOT NULL,
            control_500 TEXT NOT NULL,
            control_600 TEXT NOT NULL,
            control_700 TEXT NOT NULL,
            control_800 TEXT NOT NULL,
            control_900 TEXT NOT NULL,
            control_950 TEXT NOT NULL,
            error_50 TEXT NOT NULL,
            error_100 TEXT NOT NULL,
            error_200 TEXT NOT NULL,
            error_300 TEXT NOT NULL,
            error_400 TEXT NOT NULL,
            error_500 TEXT NOT NULL,
            error_600 TEXT NOT NULL,
            error_700 TEXT NOT NULL,
            error_800 TEXT NOT NULL,
            error_900 TEXT NOT NULL,
            error_950 TEXT NOT NULL,
            warn_50 TEXT NOT NULL,
            warn_100 TEXT NOT NULL,
            warn_200 TEXT NOT NULL,
            warn_300 TEXT NOT NULL,
            warn_400 TEXT NOT NULL,
            warn_500 TEXT NOT NULL,
            warn_600 TEXT NOT NULL,
            warn_700 TEXT NOT NULL,
            warn_800 TEXT NOT NULL,
            warn_900 TEXT NOT NULL,
            warn_950 TEXT NOT NULL,
            success_50 TEXT NOT NULL,
            success_100 TEXT NOT NULL,
            success_200 TEXT NOT NULL,
            success_300 TEXT NOT NULL,
            success_400 TEXT NOT NULL,
            success_500 TEXT NOT NULL,
            success_600 TEXT NOT NULL,
            success_700 TEXT NOT NULL,
            success_800 TEXT NOT NULL,
            success_900 TEXT NOT NULL,
            success_950 TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (profile_id) REFERENCES profile (id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS message_reactions (
            id TEXT PRIMARY KEY,
            message_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            external_id TEXT NOT NULL,
            reaction_type TEXT NOT NULL,
            reaction_value TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(message_id, user_id, reaction_type, reaction_value),
            FOREIGN KEY (message_id) REFERENCES message (id) ON DELETE CASCADE
        )
        "#,
    ).execute(pool).await?;
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS provider_theme (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            theme_name TEXT NOT NULL,
            primary_50 TEXT NOT NULL,
            primary_100 TEXT NOT NULL,
            primary_200 TEXT NOT NULL,
            primary_300 TEXT NOT NULL,
            primary_400 TEXT NOT NULL,
            primary_500 TEXT NOT NULL,
            primary_600 TEXT NOT NULL,
            primary_700 TEXT NOT NULL,
            primary_800 TEXT NOT NULL,
            primary_900 TEXT NOT NULL,
            primary_950 TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (provider_id) REFERENCES provider (id) ON DELETE CASCADE,
            UNIQUE(provider_id)
        )
        "#,
    ).execute(pool).await?;
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS calendar_event (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            external_id TEXT NOT NULL,
            
            -- Event details
            subject TEXT NOT NULL,
            body_content TEXT,
            body_content_type TEXT DEFAULT 'html',
            location TEXT,
            location_display_name TEXT,
            online_meeting_url TEXT,
            
            -- Time and recurrence
            start_datetime TEXT NOT NULL,
            start_timezone TEXT,
            end_datetime TEXT NOT NULL,
            end_timezone TEXT,
            is_all_day BOOLEAN DEFAULT 0,
            recurrence_pattern TEXT,  -- JSON for complex recurrence rules
            series_master_id TEXT,    -- Reference to the master event for recurring events
            
            -- Status and type
            status TEXT DEFAULT 'confirmed',  -- confirmed, tentative, cancelled
            event_type TEXT DEFAULT 'single_instance',  -- single_instance, occurrence, exception, series_master
            importance TEXT DEFAULT 'normal',  -- low, normal, high
            sensitivity TEXT DEFAULT 'normal', -- normal, personal, private, confidential
            show_as TEXT DEFAULT 'busy',      -- free, tentative, busy, oof, working_elsewhere, unknown
            
            -- Organizer
            organizer_id TEXT,
            organizer_name TEXT,
            organizer_email TEXT,
            
            -- Response tracking
            response_status TEXT,              -- none, organizer, tentativelyAccepted, accepted, declined, notResponded
            response_time TEXT,
            
            -- Attendees (JSON array)
            attendees TEXT,                    -- JSON array of attendee objects
            required_attendees_count INTEGER DEFAULT 0,
            optional_attendees_count INTEGER DEFAULT 0,
            
            -- Categories and metadata
            categories TEXT,                   -- JSON array of category strings
            reminder_minutes_before_start INTEGER,
            is_reminder_on BOOLEAN DEFAULT 0,
            
            -- Flags
            is_cancelled BOOLEAN DEFAULT 0,
            is_draft BOOLEAN DEFAULT 0,
            has_attachments BOOLEAN DEFAULT 0,
            
            -- Sync metadata
            last_modified_datetime TEXT NOT NULL,
            created_datetime TEXT NOT NULL,
            cached_at TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            
            UNIQUE(provider_id, external_id),
            FOREIGN KEY (provider_id) REFERENCES provider (id),
            FOREIGN KEY (series_master_id) REFERENCES calendar_event (id)
        )
        "#,
    ).execute(pool).await?;
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS meeting (
            id TEXT PRIMARY KEY,
            calendar_event_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            conversation_id TEXT,              -- Link to related conversation if exists
            
            -- Meeting specific details
            join_url TEXT,
            join_phone TEXT,
            join_meeting_id TEXT,
            join_passcode TEXT,
            
            -- Recording info
            recording_url TEXT,
            recording_available BOOLEAN DEFAULT 0,
            
            -- Meeting type
            meeting_type TEXT,                 -- teams, zoom, webex, etc.
            meeting_provider TEXT,             -- microsoft, zoom, cisco, etc.
            
            -- Additional metadata
            notes TEXT,
            agenda TEXT,
            
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            
            FOREIGN KEY (calendar_event_id) REFERENCES calendar_event (id) ON DELETE CASCADE,
            FOREIGN KEY (provider_id) REFERENCES provider (id),
            FOREIGN KEY (conversation_id) REFERENCES conversation (id)
        )
        "#,
    ).execute(pool).await?;
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS calendar_attendee (
            id TEXT PRIMARY KEY,
            event_id TEXT NOT NULL,
            user_id TEXT NOT NULL,             -- Always reference a user
            
            -- Attendance type
            type TEXT NOT NULL,                -- required, optional, resource
            
            -- Response
            response_status TEXT,              -- none, organizer, tentativelyAccepted, accepted, declined, notResponded
            response_time TEXT,
            response_message TEXT,
            
            -- Additional info
            proposed_new_time TEXT,            -- JSON for proposed time changes
            
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            
            FOREIGN KEY (event_id) REFERENCES calendar_event (id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES user (id),
            UNIQUE(event_id, user_id)
        )
        "#,
    ).execute(pool).await?;
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS conversation_member (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'member',
            added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (conversation_id) REFERENCES conversation(id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES user(id) ON DELETE CASCADE,
            UNIQUE(conversation_id, user_id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS thread (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            root_message_id TEXT NOT NULL,
            name TEXT,
            last_activity TEXT NOT NULL,
            message_count INTEGER DEFAULT 0,
            is_resolved BOOLEAN DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (conversation_id) REFERENCES conversation(id) ON DELETE CASCADE,
            FOREIGN KEY (root_message_id) REFERENCES message(id) ON DELETE CASCADE,
            UNIQUE(root_message_id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS thread_message (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL,
            message_id TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (thread_id) REFERENCES thread(id) ON DELETE CASCADE,
            FOREIGN KEY (message_id) REFERENCES message(id) ON DELETE CASCADE,
            UNIQUE(thread_id, message_id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS conversation_link (
            id TEXT PRIMARY KEY,
            source_conversation_id TEXT NOT NULL,
            target_conversation_id TEXT NOT NULL,
            link_type TEXT NOT NULL DEFAULT 'related',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (source_conversation_id) REFERENCES conversation(id) ON DELETE CASCADE,
            FOREIGN KEY (target_conversation_id) REFERENCES conversation(id) ON DELETE CASCADE,
            UNIQUE(source_conversation_id, target_conversation_id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS conversation_provider (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            external_id TEXT NOT NULL,
            theme_id TEXT,
            is_primary BOOLEAN DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (conversation_id) REFERENCES conversation(id) ON DELETE CASCADE,
            FOREIGN KEY (provider_id) REFERENCES provider(id) ON DELETE CASCADE,
            FOREIGN KEY (theme_id) REFERENCES profile_theme(id),
            UNIQUE(conversation_id, provider_id),
            UNIQUE(provider_id, external_id)
        )
        "#,
    ).execute(pool).await?;
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pinned_message (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            message_id TEXT NOT NULL,
            pinned_by TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (conversation_id) REFERENCES conversation(id) ON DELETE CASCADE,
            FOREIGN KEY (message_id) REFERENCES message(id) ON DELETE CASCADE,
            UNIQUE(conversation_id, message_id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS bookmarked_message (
            id TEXT PRIMARY KEY,
            message_id TEXT NOT NULL,
            profile_id TEXT NOT NULL,
            note TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (message_id) REFERENCES message(id) ON DELETE CASCADE,
            FOREIGN KEY (profile_id) REFERENCES profile(id) ON DELETE CASCADE,
            UNIQUE(message_id, profile_id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sidebar_section (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            name TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            is_collapsed BOOLEAN DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (profile_id) REFERENCES profile(id) ON DELETE CASCADE
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sidebar_section_item (
            id TEXT PRIMARY KEY,
            section_id TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (section_id) REFERENCES sidebar_section(id) ON DELETE CASCADE,
            FOREIGN KEY (conversation_id) REFERENCES conversation(id) ON DELETE CASCADE,
            UNIQUE(section_id, conversation_id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user_status (
            user_id TEXT PRIMARY KEY,
            status TEXT NOT NULL DEFAULT 'online',
            status_text TEXT NOT NULL DEFAULT '',
            status_emoji TEXT NOT NULL DEFAULT '',
            status_expiration TEXT,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user(id) ON DELETE CASCADE
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS app_setting (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        "#,
    ).execute(pool).await?;
    rename_label_tables(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tag (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            name TEXT NOT NULL,
            color TEXT NOT NULL DEFAULT '#8c93a3',
            kind TEXT NOT NULL DEFAULT 'tag',
            position INTEGER NOT NULL DEFAULT 0,
            description TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(profile_id, name),
            FOREIGN KEY (profile_id) REFERENCES profile (id)
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS assistant_run (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            model TEXT NOT NULL,
            prompt TEXT NOT NULL,
            output TEXT NOT NULL,
            cost_usd REAL NOT NULL DEFAULT 0,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        )
        "#,
    ).execute(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS event (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            title TEXT NOT NULL,
            notes TEXT NOT NULL DEFAULT '',
            location TEXT NOT NULL DEFAULT '',
            starts_at TEXT NOT NULL,
            ends_at TEXT NOT NULL,
            all_day BOOLEAN NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'confirmed',
            origin TEXT NOT NULL DEFAULT 'manual',
            confidence REAL NOT NULL DEFAULT 1,
            source_message_id TEXT,
            conversation_id TEXT,
            attendees TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profile (id)
        )
        "#,
    ).execute(pool).await?;
    ensure_columns(pool, "message", &[("scanned_for_events", "BOOLEAN NOT NULL DEFAULT 0")]).await;
    ensure_columns(pool, "conversation", &[("is_archived", "BOOLEAN NOT NULL DEFAULT 0"), ("snoozed_until", "TEXT"), ("deleted_at", "TEXT"), ("person_id", "TEXT"), ("favourite_position", "INTEGER"), ("notification_level", "TEXT DEFAULT 'all'")]).await;
    ensure_columns(pool, "provider", &[("config", "TEXT")]).await;
    ensure_columns(pool, "message", &MESSAGE_COLUMNS).await;
    ensure_columns(pool, "conversation", &CONVERSATION_COLUMNS).await;
    ensure_columns(pool, "event", &EVENT_COLUMNS).await;
    ensure_columns(pool, "tag", &[("icon", "TEXT"), ("favourite", "INTEGER NOT NULL DEFAULT 0"), ("favourite_position", "INTEGER")]).await;
    ensure_columns(pool, "provider", &[("last_used_at", "TEXT"), ("user_external_id", "TEXT")]).await;
    ratio_window_frames(pool).await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS calendar (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            external_id TEXT NOT NULL,
            name TEXT NOT NULL,
            colour TEXT,
            is_primary BOOLEAN NOT NULL DEFAULT 0,
            can_write BOOLEAN NOT NULL DEFAULT 0,
            enabled BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE (provider_id, external_id),
            FOREIGN KEY (profile_id) REFERENCES profile (id),
            FOREIGN KEY (provider_id) REFERENCES provider (id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS event_remote ON event (calendar_id, external_id) WHERE external_id IS NOT NULL").execute(pool).await?;
    rename_media_tables(pool).await?;
    crate::models::content::schema(pool).await?;
    crate::models::action::schema(pool).await?;
    crate::services::link_service::schema(pool).await?;
    crate::services::message_service::search_schema(pool).await?;
    crate::models::person::schema(pool).await?;
    crate::models::history::schema(pool).await?;
    crate::models::intelligence::schema(pool).await?;
    crate::models::group::schema(pool).await?;
    crate::models::inbox::schema(pool).await?;
    crate::models::sidebar::schema(pool).await?;
    crate::models::tag::schema(pool).await?;
    crate::models::object_override::schema(pool).await?;
    crate::models::case::schema(pool).await?;
    crate::models::chat::schema(pool).await?;
    crate::models::outbound_hold::schema(pool).await?;
    crate::models::draft::schema(pool).await?;
    crate::models::reminder::schema(pool).await?;
    crate::models::MergeGroup::schema(pool).await?;
    crate::providers::mock::server::schema(pool).await?;
    println!(" All database tables created successfully");
    let tables: Vec<(String,)> = sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .fetch_all(pool)
        .await?;
    println!("=� Database tables:");
    for (table_name,) in &tables {
        println!("   - {}", table_name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::*;

    #[tokio::test]
    async fn media_tables_become_content_tables_and_collections_become_groups() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        let old = concat!("media", "_item");
        sqlx::query("CREATE TABLE profile (id TEXT PRIMARY KEY, name TEXT)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO profile VALUES ('p1', 'Milo')").execute(&pool).await.unwrap();
        sqlx::query(&format!(
            "CREATE TABLE {} (id TEXT PRIMARY KEY, profile_id TEXT NOT NULL, kind TEXT NOT NULL, name TEXT NOT NULL, source_url TEXT, path TEXT NOT NULL, mime_type TEXT NOT NULL, width INTEGER, height INTEGER, tags TEXT NOT NULL DEFAULT '[]', is_favourite BOOLEAN NOT NULL DEFAULT 0, used_count INTEGER NOT NULL DEFAULT 0, last_used_at TEXT, created_at TEXT NOT NULL, favourite_position INTEGER, collections TEXT NOT NULL DEFAULT '[]', query TEXT)",
            old
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(&format!("CREATE INDEX {} ON {} (profile_id, kind)", concat!("idx_", "media", "_item_profile_kind"), old)).execute(&pool).await.unwrap();
        for (id, collections, created) in [("a", r#"["Reactions","Team"]"#, "2026-01-01"), ("b", r#"["Team"]"#, "2026-01-02"), ("c", "[]", "2026-01-03")] {
            sqlx::query(&format!("INSERT INTO {} (id, profile_id, kind, name, path, mime_type, tags, created_at, collections) VALUES (?1, 'p1', 'gif', ?1, 'media/x.gif', 'image/gif', '[\"party\"]', ?2, ?3)", old))
                .bind(id)
                .bind(created)
                .bind(collections)
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::query("CREATE TABLE app_setting (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at TEXT NOT NULL)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO app_setting VALUES (?1, '[\"Team\",\"Empty\"]', '')").bind(concat!("gif.", "collections")).execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE mock_seed (profile_id TEXT, key TEXT, kind TEXT, item_id TEXT, created_at TEXT, PRIMARY KEY (profile_id, key))").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO mock_seed VALUES ('p1', ?1, ?2, 'a', '')").bind(concat!("me", "dia:Party parrot")).bind(concat!("me", "dia")).execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE sidebar_layout (profile_id TEXT PRIMARY KEY, layout TEXT NOT NULL, updated_at TEXT NOT NULL)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO sidebar_layout VALUES ('p1', ?1, '')").bind(format!("{{\"top\":[],\"footer\":[{{\"kind\":\"item\",\"key\":\"actions\"}},{{\"kind\":\"item\",\"key\":\"{}\"}}]}}", concat!("me", "mes"))).execute(&pool).await.unwrap();

        super::rename_media_tables(&pool).await.unwrap();
        crate::models::content::schema(&pool).await.unwrap();
        super::rename_media_tables(&pool).await.unwrap();
        crate::models::content::schema(&pool).await.unwrap();

        assert!(!super::table_exists(&pool, old).await.unwrap());
        let items: Vec<String> = sqlx::query_scalar("SELECT id FROM content_item ORDER BY id").fetch_all(&pool).await.unwrap();
        assert_eq!(items, vec!["a", "b", "c"]);
        let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('content_item')").fetch_all(&pool).await.unwrap();
        assert!(!columns.contains(&"collections".to_string()) && columns.contains(&"body".to_string()) && columns.contains(&"source_person_id".to_string()));
        let groups: Vec<(String, String)> = sqlx::query_as("SELECT id, name FROM content_group WHERE profile_id = 'p1' ORDER BY position").fetch_all(&pool).await.unwrap();
        assert_eq!(groups.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>(), vec!["Team", "Empty", "Reactions"]);
        let team: Vec<String> = sqlx::query_scalar("SELECT content_id FROM content_group_item WHERE group_id = ?1 ORDER BY position").bind(&groups[0].0).fetch_all(&pool).await.unwrap();
        assert_eq!(team, vec!["a", "b"]);
        let in_groups: Vec<String> = sqlx::query_scalar("SELECT group_id FROM content_group_item WHERE content_id = 'a'").fetch_all(&pool).await.unwrap();
        assert_eq!(in_groups.len(), 2);
        let setting: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_setting").fetch_one(&pool).await.unwrap();
        assert_eq!(setting, 0);
        let (kind, key): (String, String) = sqlx::query_as("SELECT kind, key FROM mock_seed").fetch_one(&pool).await.unwrap();
        assert_eq!((kind.as_str(), key.as_str()), ("content", "content:Party parrot"));
        let layout: String = sqlx::query_scalar("SELECT layout FROM sidebar_layout").fetch_one(&pool).await.unwrap();
        assert!(layout.contains("\"key\":\"content\"") && !layout.contains(concat!("me", "mes")));
        let indexes: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'content_item' AND name LIKE 'idx_%'").fetch_all(&pool).await.unwrap();
        assert!(indexes.iter().all(|i| i.starts_with("idx_content_item")), "{:?}", indexes);
    }

    #[tokio::test]
    async fn label_tables_become_tag_tables_and_keep_their_data() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        let old = "label";
        let link = concat!("conversation_", "label");
        let column = concat!("label", "_id");
        sqlx::query(&format!("CREATE TABLE {} (id TEXT PRIMARY KEY, name TEXT, kind TEXT)", old)).execute(&pool).await.unwrap();
        sqlx::query(&format!("CREATE TABLE {} (conversation_id TEXT, {} TEXT)", link, column)).execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE inbox_member (inbox_id TEXT, kind TEXT, member_id TEXT, PRIMARY KEY (inbox_id, kind, member_id))").execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE action (id TEXT, graph TEXT, inputs TEXT, outputs TEXT)").execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE conversation_group (id TEXT, query TEXT)").execute(&pool).await.unwrap();
        sqlx::query(&format!("INSERT INTO {} VALUES ('t1', 'Kunde', ?1), ('s1', 'Team', 'split')", old)).bind(old).execute(&pool).await.unwrap();
        sqlx::query(&format!("INSERT INTO {} VALUES ('c1', 't1')", link)).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO inbox_member VALUES ('i1', ?1, 't1')").bind(old).execute(&pool).await.unwrap();
        let graph = format!("{{\"nodes\":[{{\"kind\":\"builtin:{a}\",\"config\":{{\"values\":{{\"{o}\":\"Kunde\"}}}}}}]}}", a = concat!("label", "_add"), o = old);
        sqlx::query("INSERT INTO action VALUES ('a1', ?1, ?2, '[]')").bind(&graph).bind(format!("[{{\"name\":\"x\",\"type\":\"{}\"}}]", old)).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO conversation_group VALUES ('g1', ?1)").bind(format!("{{{}:[\"t1\"]}}", concat!("\"label", "_ids\""))).execute(&pool).await.unwrap();

        super::rename_label_tables(&pool).await.unwrap();
        super::rename_label_tables(&pool).await.unwrap();

        assert!(!super::table_exists(&pool, old).await.unwrap());
        let kinds: Vec<String> = sqlx::query_scalar("SELECT kind FROM tag ORDER BY id").fetch_all(&pool).await.unwrap();
        assert_eq!(kinds, vec!["split", "tag"]);
        let linked: String = sqlx::query_scalar("SELECT tag_id FROM conversation_tag WHERE conversation_id = 'c1'").fetch_one(&pool).await.unwrap();
        assert_eq!(linked, "t1");
        let member: String = sqlx::query_scalar("SELECT kind FROM inbox_member").fetch_one(&pool).await.unwrap();
        assert_eq!(member, "tag");
        let (graph, inputs): (String, String) = sqlx::query_as("SELECT graph, inputs FROM action").fetch_one(&pool).await.unwrap();
        assert!(graph.contains("builtin:tag_add") && graph.contains("\"values\":{\"tag\":\"Kunde\"}"));
        assert!(inputs.contains("\"type\":\"tag\""));
        let query: String = sqlx::query_scalar("SELECT query FROM conversation_group").fetch_one(&pool).await.unwrap();
        assert!(query.contains("\"tag_ids\""));
    }

    #[tokio::test]
    async fn pixel_window_frames_give_way_to_ratios() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE profile_setting (id TEXT PRIMARY KEY, profile_id TEXT NOT NULL, window_x INTEGER, window_y INTEGER, window_width INTEGER, window_height INTEGER, window_maximized BOOLEAN DEFAULT 0, updated_at TEXT)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO profile_setting VALUES ('s1', 'p1', 100, 100, 1200, 800, 1, ''), ('s2', 'p2', NULL, NULL, NULL, NULL, 0, '')").execute(&pool).await.unwrap();

        super::ratio_window_frames(&pool).await.unwrap();
        super::ratio_window_frames(&pool).await.unwrap();

        let (x, width, maximized): (Option<f64>, Option<f64>, bool) = sqlx::query_as("SELECT window_x, window_width, window_maximized FROM profile_setting WHERE id = 's1'").fetch_one(&pool).await.unwrap();
        assert_eq!((x, width, maximized), (None, None, true));
        assert_eq!(crate::models::WindowPosition::get(&pool, "p1").await.unwrap(), None);

        let position = crate::models::WindowPosition { x: 0.0, y: 0.125, width: 1.0, height: 0.75, monitor: "Studio".to_string(), scale: 2.0, maximized: false, fullscreen: true };
        position.put(&pool, "p1").await.unwrap();
        super::ratio_window_frames(&pool).await.unwrap();
        assert_eq!(crate::models::WindowPosition::get(&pool, "p1").await.unwrap(), Some(position));
        assert_eq!(crate::models::WindowPosition::get(&pool, "p2").await.unwrap(), None);
    }

    #[tokio::test]
    async fn init_database_creates_all_tables() {
        let pool = test_pool().await;

        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let table_names: Vec<&str> = tables.iter().map(|(n,)| n.as_str()).collect();

        let expected = vec![
            "app_setting",
            "assets",
            "assistant_run",
            "bookmarked_message",
            "calendar_attendee",
            "case_expectation",
            "chat",
            "chat_action",
            "chat_item",
            "case_file",
            "case_item",
            "case_rule",
            "calendar_event",
            "conversation",
            "conversation_link",
            "conversation_member",
            "conversation_provider",
            "event",
            "intelligence_connection",
            "tag",
            "object_tag",
            "content_item",
            "content_group",
            "content_group_item",
            "meeting",
            "merge_group",
            "merge_group_conversation",
            "merge_group_member",
            "message",
            "message_reactions",
            "message_template",
            "pinned_message",
            "profile",
            "profile_setting",
            "profile_theme",
            "provider",
            "provider_theme",
            "provider_user",
            "sidebar_section",
            "sidebar_section_item",
            "thread",
            "thread_message",
            "user",
            "user_status",
        ];

        for table in &expected {
            assert!(
                table_names.contains(table),
                "Missing table: {}",
                table
            );
        }
    }

    #[tokio::test]
    async fn message_table_has_expected_columns() {
        let pool = test_pool().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('message')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        let expected_columns = vec![
            "id", "provider_id", "external_id", "type", "content",
            "content_type", "sender_id", "sender_name", "conversation_id",
            "importance", "is_read", "is_draft", "has_attachments",
            "timestamp", "created_at", "updated_at",
        ];

        for col in &expected_columns {
            assert!(
                column_names.contains(col),
                "Missing column in message table: {}",
                col
            );
        }
    }

    #[tokio::test]
    async fn conversation_table_has_expected_columns() {
        let pool = test_pool().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('conversation')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        let expected_columns = vec![
            "id", "name", "type", "group_type", "last_activity",
            "is_pinned", "is_muted", "is_current", "unread_count",
            "notification_level", "created_at", "updated_at",
        ];

        for col in &expected_columns {
            assert!(
                column_names.contains(col),
                "Missing column in conversation table: {}",
                col
            );
        }
    }

    #[tokio::test]
    async fn unique_constraint_on_provider() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let now = chrono::Utc::now().to_rfc3339();
        let capabilities = "{}";

        sqlx::query(
            "INSERT INTO provider (id, profile_id, provider_type, name, capabilities, namespace, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)"
        )
        .bind("prov-1")
        .bind(&profile_id)
        .bind("microsoft")
        .bind("Test")
        .bind(capabilities)
        .bind("default")
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let result = sqlx::query(
            "INSERT INTO provider (id, profile_id, provider_type, name, capabilities, namespace, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)"
        )
        .bind("prov-2")
        .bind(&profile_id)
        .bind("microsoft")
        .bind("Test Dupe")
        .bind(capabilities)
        .bind("default")
        .bind(&now)
        .execute(&pool)
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn unique_constraint_on_message_provider_external_id() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let conv_id = test_conversation(&pool, "Unique Test", "chat").await;
        let provider_id = test_provider(&pool, &profile_id, "microsoft").await;
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"INSERT INTO message (id, provider_id, external_id, type, content_type, conversation_id, importance, is_read, is_draft, has_attachments, created_datetime, last_modified_datetime, timestamp, created_at, updated_at)
            VALUES (?1, ?2, ?3, 'chat', 'text', ?4, 'normal', 0, 0, 0, ?5, ?5, ?5, ?5, ?5)"#
        )
        .bind("msg-1")
        .bind(&provider_id)
        .bind("ext-unique-1")
        .bind(&conv_id)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let result = sqlx::query(
            r#"INSERT INTO message (id, provider_id, external_id, type, content_type, conversation_id, importance, is_read, is_draft, has_attachments, created_datetime, last_modified_datetime, timestamp, created_at, updated_at)
            VALUES (?1, ?2, ?3, 'chat', 'text', ?4, 'normal', 0, 0, 0, ?5, ?5, ?5, ?5, ?5)"#
        )
        .bind("msg-2")
        .bind(&provider_id)
        .bind("ext-unique-1")
        .bind(&conv_id)
        .bind(&now)
        .execute(&pool)
        .await;

        assert!(result.is_err());
    }
}

pub async fn table_exists(pool: &Pool<Sqlite>, name: &str) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1").bind(name).fetch_one(pool).await?;
    Ok(count > 0)
}

pub async fn column_exists(pool: &Pool<Sqlite>, table: &str, column: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(&format!("SELECT COUNT(*) > 0 FROM pragma_table_info('{}') WHERE name = ?1", table)).bind(column).fetch_one(pool).await
}

pub async fn rename_label_tables(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    const OLD_TABLE: &str = "label";
    const OLD_LINK: &str = concat!("conversation_", "label");
    const OLD_COLUMN: &str = concat!("label", "_id");
    const OLD_IDS: &str = concat!("\"label", "_ids\"");
    const OLD_ADD: &str = concat!("label", "_add");
    const OLD_REMOVE: &str = concat!("label", "_remove");
    if table_exists(pool, OLD_TABLE).await? && !table_exists(pool, "tag").await? {
        sqlx::query(&format!("ALTER TABLE {} RENAME TO tag", OLD_TABLE)).execute(pool).await?;
    }
    if table_exists(pool, OLD_LINK).await? && !table_exists(pool, "conversation_tag").await? {
        sqlx::query(&format!("ALTER TABLE {} RENAME TO conversation_tag", OLD_LINK)).execute(pool).await?;
        sqlx::query(&format!("ALTER TABLE conversation_tag RENAME COLUMN {} TO tag_id", OLD_COLUMN)).execute(pool).await?;
    }
    if table_exists(pool, "tag").await? {
        sqlx::query("UPDATE tag SET kind = 'tag' WHERE kind = ?1").bind(OLD_TABLE).execute(pool).await?;
    }
    if table_exists(pool, "inbox_member").await? {
        sqlx::query("UPDATE OR IGNORE inbox_member SET kind = 'tag' WHERE kind = ?1").bind(OLD_TABLE).execute(pool).await?;
    }
    if table_exists(pool, "conversation_group").await? {
        sqlx::query("UPDATE conversation_group SET query = REPLACE(query, ?1, '\"tag_ids\"') WHERE instr(query, ?1) > 0").bind(OLD_IDS).execute(pool).await?;
    }
    if table_exists(pool, "action").await? {
        let old_type = format!("\"type\":\"{}\"", OLD_TABLE);
        let old_value = format!("\"values\":{{\"{}\":", OLD_TABLE);
        for column in ["graph", "inputs"] {
            sqlx::query(&format!("UPDATE action SET {c} = REPLACE(REPLACE(REPLACE(REPLACE({c}, ?1, 'tag_add'), ?2, 'tag_remove'), ?3, '\"type\":\"tag\"'), ?4, '\"values\":{{\"tag\":') WHERE instr({c}, ?5) > 0", c = column))
                .bind(OLD_ADD)
                .bind(OLD_REMOVE)
                .bind(&old_type)
                .bind(&old_value)
                .bind(OLD_TABLE)
                .execute(pool)
                .await?;
        }
    }
    if table_exists(pool, "app_setting").await? {
        sqlx::query("UPDATE OR IGNORE app_setting SET key = REPLACE(key, '.labels', '.tags') WHERE key LIKE 'sidebar.%.labels%'").execute(pool).await?;
    }
    if table_exists(pool, "mock_seed").await? {
        sqlx::query("UPDATE mock_seed SET kind = 'tag' WHERE kind = ?1").bind(OLD_TABLE).execute(pool).await?;
        sqlx::query("UPDATE OR IGNORE mock_seed SET key = 'tag:' || substr(key, length(?1) + 2) WHERE key LIKE ?1 || ':%'").bind(OLD_TABLE).execute(pool).await?;
        sqlx::query("UPDATE OR IGNORE mock_seed SET key = 'tagged' WHERE key = ?1").bind(concat!("label", "led")).execute(pool).await?;
    }
    Ok(())
}

pub async fn rename_media_tables(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    const OLD_TABLE: &str = concat!("media", "_item");
    const OLD_KIND: &str = concat!("me", "dia");
    const OLD_PAGE: &str = concat!("\"key\":\"", "me", "mes\"");
    if table_exists(pool, OLD_TABLE).await? && !table_exists(pool, "content_item").await? {
        sqlx::query(&format!("ALTER TABLE {} RENAME TO content_item", OLD_TABLE)).execute(pool).await?;
        for index in [concat!("idx_", "media", "_item_hash"), concat!("idx_", "media", "_item_profile_kind")] {
            sqlx::query(&format!("DROP INDEX IF EXISTS {}", index)).execute(pool).await?;
        }
    }
    if table_exists(pool, "mock_seed").await? {
        sqlx::query("UPDATE mock_seed SET kind = 'content' WHERE kind = ?1").bind(OLD_KIND).execute(pool).await?;
        sqlx::query("UPDATE OR IGNORE mock_seed SET key = 'content:' || substr(key, length(?1) + 2) WHERE key LIKE ?1 || ':%'").bind(OLD_KIND).execute(pool).await?;
    }
    if table_exists(pool, "sidebar_layout").await? {
        sqlx::query("UPDATE sidebar_layout SET layout = REPLACE(layout, ?1, '\"key\":\"content\"') WHERE instr(layout, ?1) > 0").bind(OLD_PAGE).execute(pool).await?;
    }
    Ok(())
}

pub async fn ratio_window_frames(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    if !table_exists(pool, "profile_setting").await? {
        return Ok(());
    }
    ensure_columns(pool, "profile_setting", &[("window_monitor", "TEXT"), ("window_scale", "REAL"), ("window_fullscreen", "BOOLEAN NOT NULL DEFAULT 0")]).await;
    sqlx::query("UPDATE profile_setting SET window_x = NULL, window_y = NULL, window_width = NULL, window_height = NULL WHERE window_monitor IS NULL AND (window_x IS NOT NULL OR window_y IS NOT NULL OR window_width IS NOT NULL OR window_height IS NOT NULL)")
        .execute(pool)
        .await?;
    Ok(())
}
