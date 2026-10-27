# Tauri Architecture

## Database

**IMPORTANT**: This application uses ONE SQLite database with the following tables ONLY. Do NOT add additional tables without explicit approval.

### Database Tables

#### `profile`
| Column | Type | Constraints |
|--------|------|-------------|
| id | TEXT | PRIMARY KEY |
| name | TEXT | NOT NULL |
| avatar_id | TEXT | |
| theme_id | TEXT | |
| setup_step | INTEGER | DEFAULT 0 |
| created_at | TEXT | NOT NULL DEFAULT CURRENT_TIMESTAMP |
| | | FOREIGN KEY (avatar_id) REFERENCES assets (id) |
| | | FOREIGN KEY (theme_id) REFERENCES profile_theme (id) |

#### `provider`
| Column | Type | Constraints |
|--------|------|-------------|
| id | TEXT | PRIMARY KEY |
| provider_type | TEXT | NOT NULL |
| display_name | TEXT | NOT NULL |
| capabilities | TEXT | NOT NULL |
| auth_config | TEXT | NOT NULL |
| is_enabled | BOOLEAN | DEFAULT 1 |
| access_token | TEXT | |
| refresh_token | TEXT | |
| expires_at | TEXT | |
| token_type | TEXT | DEFAULT 'Bearer' |
| scope | TEXT | |
| created_at | TEXT | NOT NULL |
| updated_at | TEXT | NOT NULL |

#### `conversation`
| Column | Type | Constraints |
|--------|------|-------------|
| id | TEXT | PRIMARY KEY |
| name | TEXT | NOT NULL |
| conversation_type | TEXT | NOT NULL |
| participants | TEXT | NOT NULL |
| last_message_id | TEXT | |
| last_activity | TEXT | NOT NULL |
| providers | TEXT | NOT NULL |
| metadata | TEXT | |
| is_cached | BOOLEAN | DEFAULT 0 |
| unread_count | INTEGER | DEFAULT 0 |
| last_message_preview | TEXT | |
| participants_count | INTEGER | DEFAULT 0 |
| is_current | BOOLEAN | DEFAULT 0 |
| summary | TEXT | |
| created_at | TEXT | NOT NULL DEFAULT CURRENT_TIMESTAMP |
| updated_at | TEXT | NOT NULL DEFAULT CURRENT_TIMESTAMP |

#### `message`
| Column | Type | Constraints |
|--------|------|-------------|
| id | TEXT | PRIMARY KEY |
| conversation_id | TEXT | NOT NULL |
| provider_id | TEXT | NOT NULL |
| provider_name | TEXT | NOT NULL |
| sender_id | TEXT | NOT NULL |
| sender_name | TEXT | |
| content | TEXT | NOT NULL |
| content_type | TEXT | NOT NULL |
| timestamp | TEXT | NOT NULL |
| message_type | TEXT | NOT NULL |
| metadata | TEXT | |
| cached_at | TEXT | |
| created_at | TEXT | NOT NULL |
| updated_at | TEXT | NOT NULL |
| | | FOREIGN KEY (conversation_id) REFERENCES conversation (id) |
| | | FOREIGN KEY (provider_id) REFERENCES provider (id) |

#### `user`
| Column | Type | Constraints |
|--------|------|-------------|
| id | TEXT | PRIMARY KEY |
| provider_user_id | TEXT | NOT NULL |
| provider_id | TEXT | NOT NULL |
| provider_name | TEXT | NOT NULL |
| display_name | TEXT | |
| email | TEXT | |
| avatar_url | TEXT | |
| avatar_data | TEXT | |
| job_title | TEXT | |
| office_location | TEXT | |
| business_phones | TEXT | |
| mobile_phone | TEXT | |
| first_seen | TEXT | NOT NULL |
| last_updated | TEXT | NOT NULL |
| | | UNIQUE(provider_user_id, provider_id) |
| | | FOREIGN KEY (provider_id) REFERENCES provider (id) |

#### `assets`
| Column | Type | Constraints |
|--------|------|-------------|
| id | TEXT | PRIMARY KEY |
| profile_id | TEXT | NOT NULL |
| provider_id | TEXT | NOT NULL |
| provider_name | TEXT | NOT NULL |
| asset_id | TEXT | NOT NULL |
| asset_name | TEXT | NOT NULL |
| asset_url | TEXT | NOT NULL |
| asset_type | TEXT | NOT NULL |
| mime_type | TEXT | NOT NULL |
| discovered_at | TEXT | NOT NULL |
| last_seen | TEXT | NOT NULL |
| usage_count | INTEGER | DEFAULT 1 |
| | | UNIQUE(profile_id, provider_id, asset_id) |
| | | FOREIGN KEY (profile_id) REFERENCES profile (id) |
| | | FOREIGN KEY (provider_id) REFERENCES provider (id) |

#### `message_template`
| Column | Type | Constraints |
|--------|------|-------------|
| id | TEXT | PRIMARY KEY |
| profile_id | TEXT | NOT NULL |
| name | TEXT | NOT NULL |
| content | TEXT | NOT NULL |
| is_html | BOOLEAN | DEFAULT 0 |
| created_at | TEXT | NOT NULL |
| updated_at | TEXT | NOT NULL |
| usage_count | INTEGER | DEFAULT 0 |
| | | FOREIGN KEY (profile_id) REFERENCES profile (id) |

#### `profile_setting`
| Column | Type | Constraints |
|--------|------|-------------|
| id | TEXT | PRIMARY KEY |
| profile_id | TEXT | NOT NULL |
| email_content_threshold | INTEGER | DEFAULT 500 |
| navigation_width | INTEGER | DEFAULT 280 |
| navigation_collapsed | BOOLEAN | DEFAULT 0 |
| details_width | INTEGER | DEFAULT 320 |
| window_x | INTEGER | |
| window_y | INTEGER | |
| window_width | INTEGER | |
| window_height | INTEGER | |
| window_maximized | BOOLEAN | DEFAULT 0 |
| message_timestamp_interval | INTEGER | DEFAULT 5 |
| time_format | TEXT | DEFAULT '24h' |
| window_controls_style | TEXT | DEFAULT 'macos' |
| created_at | TEXT | NOT NULL DEFAULT CURRENT_TIMESTAMP |
| updated_at | TEXT | NOT NULL DEFAULT CURRENT_TIMESTAMP |
| | | FOREIGN KEY (profile_id) REFERENCES profile (id) |

#### `profile_theme`
| Column | Type | Constraints |
|--------|------|-------------|
| id | TEXT | PRIMARY KEY |
| profile_id | TEXT | NOT NULL |
| theme_name | TEXT | NOT NULL |
| is_active | BOOLEAN | DEFAULT 0 |
| primary_50 | TEXT | NOT NULL |
| primary_100 | TEXT | NOT NULL |
| primary_200 | TEXT | NOT NULL |
| primary_300 | TEXT | NOT NULL |
| primary_400 | TEXT | NOT NULL |
| primary_500 | TEXT | NOT NULL |
| primary_600 | TEXT | NOT NULL |
| primary_700 | TEXT | NOT NULL |
| primary_800 | TEXT | NOT NULL |
| primary_900 | TEXT | NOT NULL |
| primary_950 | TEXT | NOT NULL |
| secondary_50 | TEXT | NOT NULL |
| secondary_100 | TEXT | NOT NULL |
| secondary_200 | TEXT | NOT NULL |
| secondary_300 | TEXT | NOT NULL |
| secondary_400 | TEXT | NOT NULL |
| secondary_500 | TEXT | NOT NULL |
| secondary_600 | TEXT | NOT NULL |
| secondary_700 | TEXT | NOT NULL |
| secondary_800 | TEXT | NOT NULL |
| secondary_900 | TEXT | NOT NULL |
| secondary_950 | TEXT | NOT NULL |
| accent_50 | TEXT | NOT NULL |
| accent_100 | TEXT | NOT NULL |
| accent_200 | TEXT | NOT NULL |
| accent_300 | TEXT | NOT NULL |
| accent_400 | TEXT | NOT NULL |
| accent_500 | TEXT | NOT NULL |
| accent_600 | TEXT | NOT NULL |
| accent_700 | TEXT | NOT NULL |
| accent_800 | TEXT | NOT NULL |
| accent_900 | TEXT | NOT NULL |
| accent_950 | TEXT | NOT NULL |
| background_50 | TEXT | NOT NULL |
| background_100 | TEXT | NOT NULL |
| background_200 | TEXT | NOT NULL |
| background_300 | TEXT | NOT NULL |
| background_400 | TEXT | NOT NULL |
| background_500 | TEXT | NOT NULL |
| background_600 | TEXT | NOT NULL |
| background_700 | TEXT | NOT NULL |
| background_800 | TEXT | NOT NULL |
| background_900 | TEXT | NOT NULL |
| background_950 | TEXT | NOT NULL |
| content_50 | TEXT | NOT NULL |
| content_100 | TEXT | NOT NULL |
| content_200 | TEXT | NOT NULL |
| content_300 | TEXT | NOT NULL |
| content_400 | TEXT | NOT NULL |
| content_500 | TEXT | NOT NULL |
| content_600 | TEXT | NOT NULL |
| content_700 | TEXT | NOT NULL |
| content_800 | TEXT | NOT NULL |
| content_900 | TEXT | NOT NULL |
| content_950 | TEXT | NOT NULL |
| control_50 | TEXT | NOT NULL |
| control_100 | TEXT | NOT NULL |
| control_200 | TEXT | NOT NULL |
| control_300 | TEXT | NOT NULL |
| control_400 | TEXT | NOT NULL |
| control_500 | TEXT | NOT NULL |
| control_600 | TEXT | NOT NULL |
| control_700 | TEXT | NOT NULL |
| control_800 | TEXT | NOT NULL |
| control_900 | TEXT | NOT NULL |
| control_950 | TEXT | NOT NULL |
| error_50 | TEXT | NOT NULL |
| error_100 | TEXT | NOT NULL |
| error_200 | TEXT | NOT NULL |
| error_300 | TEXT | NOT NULL |
| error_400 | TEXT | NOT NULL |
| error_500 | TEXT | NOT NULL |
| error_600 | TEXT | NOT NULL |
| error_700 | TEXT | NOT NULL |
| error_800 | TEXT | NOT NULL |
| error_900 | TEXT | NOT NULL |
| error_950 | TEXT | NOT NULL |
| warn_50 | TEXT | NOT NULL |
| warn_100 | TEXT | NOT NULL |
| warn_200 | TEXT | NOT NULL |
| warn_300 | TEXT | NOT NULL |
| warn_400 | TEXT | NOT NULL |
| warn_500 | TEXT | NOT NULL |
| warn_600 | TEXT | NOT NULL |
| warn_700 | TEXT | NOT NULL |
| warn_800 | TEXT | NOT NULL |
| warn_900 | TEXT | NOT NULL |
| warn_950 | TEXT | NOT NULL |
| success_50 | TEXT | NOT NULL |
| success_100 | TEXT | NOT NULL |
| success_200 | TEXT | NOT NULL |
| success_300 | TEXT | NOT NULL |
| success_400 | TEXT | NOT NULL |
| success_500 | TEXT | NOT NULL |
| success_600 | TEXT | NOT NULL |
| success_700 | TEXT | NOT NULL |
| success_800 | TEXT | NOT NULL |
| success_900 | TEXT | NOT NULL |
| success_950 | TEXT | NOT NULL |
| created_at | TEXT | NOT NULL DEFAULT CURRENT_TIMESTAMP |
| updated_at | TEXT | NOT NULL DEFAULT CURRENT_TIMESTAMP |
| | | FOREIGN KEY (profile_id) REFERENCES profile (id) |

## IPC Commands

**IMPORTANT**: The following IPC commands are the ONLY commands exposed to the frontend. Do NOT add additional IPC commands without explicit approval.

### Allowed IPC Commands

1. `initialize_app` - Initialize application state
2. `post_profile` - Create a new user profile
3. `get_profile` - Get user profile with settings and theme
4. `put_profile` - Update user profile
5. `put_profile_settings` - Update profile settings
6. `put_profile_theme` - Update profile theme
7. `set_profile_theme` - Set active theme for profile
8. `get_conversations` - Get conversation list
9. `get_conversation` - Get specific conversation with messages
10. `send_message` - Send a message
11. `send_message_reaction` - Send reaction to message
12. `sync_conversations` - Sync conversations from providers
13. `get_current_conversation` - Get currently active conversation ID
14. `set_current_conversation` - Set current conversation
15. `clear_current_conversation` - Clear current conversation
16. `start_microsoft_oauth` - Start Microsoft OAuth flow
17. `complete_microsoft_oauth` - Complete Microsoft OAuth
18. `log` - Log messages from frontend

### Command Details

#### 1. Application State Commands

##### `initialize_app`
- **Parameters**: None
- **Returns**: `ProfileState | null` - Returns profile data if exists, null if needs setup
- **Description**: Initializes application state and checks if profile exists
- **Events Emitted**: `profile-updated` (if profile exists)

##### `log`
- **Parameters**: 
  - `level`: string - Log level ("debug", "info", "warn", "error")
  - `message`: string - Log message
  - `source?`: string - Optional source identifier
- **Returns**: void
- **Description**: Logs messages from frontend to Rust backend

#### 2. Profile Management Commands

##### `post_profile`
- **Parameters**: 
  - `name`: string - Profile name
- **Returns**: `ProfileState` - Complete profile data with settings and theme
- **Description**: Creates new profile with default settings and theme
- **Events Emitted**: `profile-updated`

##### `get_profile`
- **Parameters**: None
- **Returns**: `ProfileState | null` - Profile data or null if none exists
- **Description**: Gets complete profile data including settings, theme, and providers
- **Structure**: 
  ```
  ProfileState {
    profile: Profile,
    settings: ProfileSetting,
    theme: ProfileTheme | null,
    providers: ProfileProvider[]
  }
  ```

##### `put_profile`
- **Parameters**: 
  - `id`: string - Profile ID
  - `name?`: string - Updated name
  - `avatar_id?`: string - Avatar asset ID
  - `theme_id?`: string - Theme ID
  - `setup_step?`: number - Setup progress
- **Returns**: `ProfileState` - Updated complete profile data
- **Description**: Updates profile information
- **Events Emitted**: `profile-updated`

##### `put_profile_settings`
- **Parameters**: 
  - `id`: string - Settings ID
  - Various optional UI and window settings
- **Returns**: `ProfileSetting` - Updated settings
- **Description**: Updates profile settings (UI preferences, window state)
- **Events Emitted**: `profile-updated`

##### `put_profile_theme`
- **Parameters**: 
  - `id`: string - Theme ID
  - Color palette properties (primary_*, secondary_*, etc.)
- **Returns**: `ProfileTheme` - Updated theme
- **Description**: Updates theme color values
- **Events Emitted**: `profile-updated`

##### `set_profile_theme`
- **Parameters**: 
  - `theme`: ProfileTheme - Complete theme object
- **Returns**: `ProfileState` - Updated profile data
- **Description**: Sets active theme for profile and generates full color palette
- **Events Emitted**: `profile-updated`

#### 3. Conversation Commands

##### `get_conversations`
- **Parameters**: None
- **Returns**: `Conversation[]` - Array of conversations
- **Description**: Gets list of all conversations
- **Events Emitted**: None (use `conversations-updated` event for updates)

##### `get_conversation`
- **Parameters**: 
  - `conversation_id`: string - Conversation ID
- **Returns**: `Conversation` - Conversation with messages
- **Description**: Gets specific conversation including all messages
- **Events Emitted**: None (use `conversation-updated` event for updates)

##### `send_message`
- **Parameters**: 
  - `conversation_id`: string - Target conversation
  - `content`: string - Message content
  - `content_type?`: string - Content type (default: "text/plain")
  - `metadata?`: object - Optional metadata
- **Returns**: `Message` - Created message
- **Description**: Sends message to conversation
- **Events Emitted**: `message-received`, `conversation-updated`

##### `send_message_reaction`
- **Parameters**: 
  - `message_id`: string - Target message ID
  - `reaction`: string - Reaction emoji/text
- **Returns**: void
- **Description**: Adds reaction to message
- **Events Emitted**: `conversation-updated`

##### `sync_conversations`
- **Parameters**: None
- **Returns**: `number` - Count of synced conversations
- **Description**: Syncs conversations from connected providers
- **Events Emitted**: `conversations-updated`, `conversation-updated`

##### `get_current_conversation`
- **Parameters**: None
- **Returns**: `string | null` - Current conversation ID or null
- **Description**: Gets currently active conversation ID

##### `set_current_conversation`
- **Parameters**: 
  - `conversation_id`: string - Conversation ID to set as current
- **Returns**: void
- **Description**: Sets active conversation
- **Events Emitted**: `current-conversation-changed`

##### `clear_current_conversation`
- **Parameters**: None
- **Returns**: void
- **Description**: Clears current conversation selection
- **Events Emitted**: `current-conversation-changed`

#### 4. OAuth Commands

##### `start_microsoft_oauth`
- **Parameters**: None
- **Returns**: `string` - OAuth authorization URL
- **Description**: Initiates Microsoft OAuth flow, returns URL to navigate to

##### `complete_microsoft_oauth`
- **Parameters**: 
  - `code`: string - OAuth authorization code from callback
- **Returns**: void
- **Description**: Completes Microsoft OAuth flow with authorization code
- **Events Emitted**: `profile-updated` (updates providers)

#### Event-Driven Architecture

The backend emits Tauri events for real-time updates:

- `profile-updated` - When profile, settings, theme, or providers change
- `conversations-updated` - When conversation list changes  
- `conversation-updated` - When specific conversation or its messages change
- `message-received` - When new message arrives
- `current-conversation-changed` - When current conversation selection changes

#### Frontend Store Integration

Stores should use this pattern:
1. Call `get_*` commands for initial data
2. Subscribe to corresponding events for real-time updates
3. Use `put_*` commands to update data
4. Let events automatically update the UI