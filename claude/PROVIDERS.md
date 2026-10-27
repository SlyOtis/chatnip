# Provider Implementation Guide for Claude Code

This document provides instructions for Claude Code to implement new providers in the Chatnip application. Follow these patterns and guidelines when adding support for new messaging platforms.

## Overview

Providers in Chatnip are messaging platforms (Microsoft Teams, Slack, Google Chat, etc.) that sync conversations and messages to a local cache. Each provider runs independently with its own sync interval and authentication.

The system uses a generalized `Provider` trait that all specific providers must implement, ensuring consistency and making it easier to add new providers.

## Architecture

### Key Components

1. **Provider Trait** (`src-tauri/src/providers/traits.rs`)
   - Defines the interface all providers must implement
   - Includes methods for OAuth, syncing, sending messages, etc.
   - Provides consistent data types across all providers

2. **Provider Registry** (`src-tauri/src/providers/traits.rs`)
   - Manages all registered provider implementations
   - Allows runtime provider lookup by type
   - Automatically registers providers based on environment variables

3. **Provider Sync Module** (`src-tauri/src/provider_sync.rs`)
   - Manages background sync tasks for each provider
   - Uses the Provider trait for all operations
   - Handles token refresh and error recovery

4. **Database Schema**
   - `provider` table: Stores OAuth tokens and provider configuration
   - `conversation` table: Caches conversations with `provider_id` and `external_id`
   - `message` table: Caches messages with provider references

5. **OAuth Flow**
   - Start: `start_{provider}_oauth` command (calls Provider::get_auth_url)
   - Complete: `complete_{provider}_oauth` command (calls Provider::exchange_code)
   - Tokens stored in `provider` table

## Implementation Steps for New Providers

### 1. Environment Variables

Add to `.env.example`:
```env
# {PROVIDER} OAuth Configuration
{PROVIDER}_CLIENT_ID=your_client_id_here
{PROVIDER}_CLIENT_SECRET=your_client_secret_here
{PROVIDER}_SYNC_INTERVAL=300
```

### 2. Create Provider Implementation

Create a new provider struct in `src-tauri/src/providers/{provider}_provider.rs`:

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use reqwest::Client;

use super::traits::{
    Provider, ProviderConfig, ProviderCapabilities, ProviderConversation, 
    ProviderMessage, AuthTokens, Participant, ConversationType, MessageType
};

pub struct {Provider}Provider {
    config: ProviderConfig,
    client: Client,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

impl {Provider}Provider {
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        let config = ProviderConfig {
            id: "{provider}".to_string(),
            provider_type: "{provider}".to_string(),
            display_name: "{Provider} Workspace".to_string(),
            sync_interval: std::env::var("{PROVIDER}_SYNC_INTERVAL")
                .unwrap_or_else(|_| "300".to_string())
                .parse::<u64>()
                .unwrap_or(300),
            capabilities: ProviderCapabilities {
                can_send_messages: true,
                can_edit_messages: false,
                can_delete_messages: false,
                // ... set capabilities based on provider features
            },
        };

        Self { config, client: Client::new(), client_id, client_secret, redirect_uri }
    }
}

#[async_trait]
impl Provider for {Provider}Provider {
    fn get_config(&self) -> &ProviderConfig {
        &self.config
    }

    async fn get_auth_url(&self, state: &str) -> Result<String, String> {
        // Build OAuth authorization URL
        // Include appropriate scopes for the provider
    }

    async fn exchange_code(&self, code: &str) -> Result<AuthTokens, String> {
        // Exchange authorization code for access/refresh tokens
        // Parse response and return AuthTokens struct
    }

    async fn sync_conversations(&self, tokens: &AuthTokens, _since: Option<DateTime<Utc>>) -> Result<Vec<ProviderConversation>, String> {
        // Fetch conversations from provider API
        // Map to ProviderConversation structs
    }

    async fn sync_messages(&self, tokens: &AuthTokens, conversation_external_id: &str, _since: Option<DateTime<Utc>>, limit: Option<u32>) -> Result<Vec<ProviderMessage>, String> {
        // Fetch messages for a conversation
        // Map to ProviderMessage structs
    }

    // Implement other required methods...
}
```

### 3. Register Provider

Add the provider to the registry in `src-tauri/src/provider_sync.rs` constructor:

```rust
// In ProviderSync::new()
if let (Ok(client_id), Ok(client_secret)) = (
    std::env::var("{PROVIDER}_CLIENT_ID"),
    std::env::var("{PROVIDER}_CLIENT_SECRET")
) {
    let redirect_uri = "http://localhost:1420/provider/{provider}/callback".to_string();
    let provider = {Provider}Provider::new(client_id, client_secret, redirect_uri);
    registry.register(Box::new(provider));
}
```

### 4. OAuth Commands

Create new commands in `src-tauri/src/commands.rs` that use the Provider trait:

```rust
#[tauri::command]
pub async fn start_{provider}_oauth(
    provider_sync: State<'_, crate::provider_sync::ProviderSync>,
) -> Result<String, String> {
    let registry = provider_sync.registry.read().await;
    let provider = registry.get("{provider}")
        .ok_or("Provider not registered")?;
    
    let state = uuid::Uuid::new_v4().to_string();
    provider.get_auth_url(&state).await
}

#[tauri::command]
pub async fn complete_{provider}_oauth(
    service: State<'_, ChatnipService>,
    window: Window,
    code: String,
    provider_sync: State<'_, crate::provider_sync::ProviderSync>,
) -> Result<(), String> {
    let registry = provider_sync.registry.read().await;
    let provider = registry.get("{provider}")
        .ok_or("Provider not registered")?;
    
    // Exchange code for tokens
    let tokens = provider.exchange_code(&code).await?;
    
    // Store tokens in database
    // Emit "provider-connected" event
    // Start provider sync
}
```

Remember to:
- Add the new commands to the `invoke_handler` in `lib.rs`
- Update the providers module to export your new provider

## Provider-Specific Implementation Notes

### Slack
- Use Slack Web API (conversations.list, conversations.history)
- Handle both public channels and direct messages
- Map Slack user IDs to display names
- Consider workspace isolation

### Google Chat
- Use Google Chat API (spaces.list, spaces.messages.list)
- Handle both spaces and direct messages
- Requires Google Workspace domain setup
- Consider using service account for background sync

### Discord
- Use Discord API v10 (guilds, channels, messages endpoints)
- Handle servers (guilds) and DM channels differently
- Respect rate limits (50 requests per second)
- Store server/channel hierarchy in metadata

## Data Mapping Guidelines

### Conversation Fields
```rust
conversation_id: Uuid::new_v4().to_string(),  // Local UUID
provider_id: provider_id,                      // e.g., "slack"
external_id: chat["id"],                       // Provider's ID
name: chat["name"] or generate from participants,
conversation_type: map_provider_type_to_standard(),
participants: JSON array of participant info,
last_activity: parse provider timestamp,
providers: provider_id,                        // Single provider per conversation
is_cached: true,
cached_at: Utc::now().to_rfc3339(),
```

### Message Fields
```rust
id: Uuid::new_v4().to_string(),               // Local UUID
conversation_id: local_conversation_id,
provider_id: provider_id,
provider_name: "Slack" | "Google Chat" | etc,
external_id: message["id"],                    // Provider's message ID
sender_id: message["user"]["id"],
sender_name: message["user"]["name"],
content: message["text"] or message["content"],
content_type: "text/plain" or "text/html",
timestamp: parse provider timestamp,
message_type: "chat" | "system" | "bot",
cached_at: Utc::now().to_rfc3339(),
```

## Testing Checklist

When implementing a new provider, ensure:

1. [ ] OAuth flow completes successfully
2. [ ] Tokens are stored and retrieved correctly
3. [ ] Provider syncs on app startup if enabled
4. [ ] Manual sync via sync_conversations works
5. [ ] Conversations appear in frontend
6. [ ] Messages load when selecting conversation
7. [ ] Sync respects the interval from .env
8. [ ] Expired token handling works
9. [ ] Events are emitted correctly
10. [ ] No duplicate conversations/messages created

## Error Handling

Always implement proper error handling:
- Log errors with context: `eprintln!("Failed to sync {provider}: {}", e);`
- Continue syncing other providers if one fails
- Handle rate limits with exponential backoff
- Validate API responses before processing
- Use transactions for database operations when possible

## Security Considerations

- Never log full access tokens (use first 20 chars max)
- Always use HTTPS for API calls
- Validate OAuth state parameter
- Store tokens encrypted if possible
- Implement token refresh before expiration
- Use minimal required OAuth scopes

## Adding Frontend Support

After implementing the provider backend:

1. Add provider to setup flow in `/setup/providers`
2. Create OAuth callback page at `/provider/{provider}/callback`
3. Add provider icon and branding
4. Update types in TypeScript stores if needed

## Example: Implementing Slack Provider

As a concrete example, here's how you would implement Slack support:

1. **Environment Setup**
   ```env
   SLACK_CLIENT_ID=your_slack_client_id
   SLACK_CLIENT_SECRET=your_slack_client_secret
   SLACK_SYNC_INTERVAL=60
   ```

2. **OAuth Scopes Needed**
   - `channels:history` - Read public channel messages
   - `channels:read` - List public channels
   - `groups:history` - Read private channel messages
   - `groups:read` - List private channels
   - `im:history` - Read direct messages
   - `im:read` - List direct message conversations
   - `users:read` - Get user information

3. **API Endpoints**
   - List conversations: `GET https://slack.com/api/conversations.list`
   - Get messages: `GET https://slack.com/api/conversations.history`
   - Get user info: `GET https://slack.com/api/users.info`

4. **Special Considerations**
   - Slack uses cursor-based pagination
   - Message timestamps are unique IDs in Slack
   - Handle message threads separately
   - Map Slack markdown to standard format

Remember: Always refer to the official API documentation for each provider as APIs can change. This guide provides the pattern, but specific implementation details may vary.