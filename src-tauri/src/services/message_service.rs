use sqlx::{Pool, Sqlite, Row};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::Utc;
use futures::future::try_join_all;

use crate::models::{Message, Asset, User};
use crate::providers::models::ProviderAuth;
use crate::providers::microsoft::MicrosoftProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReaction {
    pub id: String,
    pub message_id: String,
    pub user_id: String,
    pub external_id: String,
    pub reaction_type: String,
    pub reaction_value: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedMessage {
    #[serde(flatten)]
    pub message: Message,
    pub reactions: Vec<MessageReaction>,
    pub assets: Vec<Asset>,
    pub sender_details: Option<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageLoadOptions {
    pub load_reactions: bool,
    pub load_assets: bool,
    pub load_sender_details: bool,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl Default for MessageLoadOptions {
    fn default() -> Self {
        Self {
            load_reactions: true,
            load_assets: true,
            load_sender_details: true,
            limit: Some(50),
            offset: None,
        }
    }
}

pub struct MessageService {
    pool: Pool<Sqlite>,
}

impl MessageService {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Load messages for a conversation with all associated data asynchronously
    pub async fn load_conversation_messages(
        &self,
        conversation_id: &str,
        options: MessageLoadOptions,
    ) -> Result<Vec<EnhancedMessage>, String> {
        // Build the base query with optional LIMIT and OFFSET
        let mut query = "SELECT * FROM message WHERE conversation_id = ?1 ORDER BY timestamp ASC".to_string();
        
        if let Some(limit) = options.limit {
            query.push_str(&format!(" LIMIT {}", limit));
            if let Some(offset) = options.offset {
                query.push_str(&format!(" OFFSET {}", offset));
            }
        }

        let message_rows = sqlx::query(&query)
            .bind(conversation_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to get messages: {}", e))?;

        let messages: Vec<Message> = message_rows
            .into_iter()
            .map(|row| self.row_to_message(row))
            .collect();

        // Load all associated data concurrently for each message
        let enhanced_messages = try_join_all(
            messages
                .into_iter()
                .map(|msg| self.enhance_message(msg, &options))
        )
        .await?;

        Ok(enhanced_messages)
    }

    pub async fn get_merge_group_messages(&self, profile_id: &str, lead_conversation_id: &str, limit: u32, offset: u32) -> Result<Vec<Message>, String> {
        let rows = sqlx::query(
            "SELECT m.* FROM message m
             WHERE m.conversation_id IN (
                SELECT gc.conversation_id FROM merge_group g JOIN merge_group_conversation gc ON gc.merge_group_id = g.id
                WHERE g.profile_id = ?1 AND g.lead_conversation_id = ?2
             )
             ORDER BY m.timestamp DESC LIMIT ?3 OFFSET ?4",
        )
        .bind(profile_id)
        .bind(lead_conversation_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to get group messages: {}", e))?;
        let mut messages: Vec<Message> = rows.into_iter().map(|row| self.row_to_message(row)).collect();
        messages.reverse();
        Ok(messages)
    }

    /// Load a single message with all associated data
    pub async fn load_message(
        &self,
        message_id: &str,
        options: MessageLoadOptions,
    ) -> Result<EnhancedMessage, String> {
        let row = sqlx::query("SELECT * FROM message WHERE id = ?1")
            .bind(message_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| format!("Failed to get message: {}", e))?
            .ok_or_else(|| "Message not found".to_string())?;

        let message = self.row_to_message(row);
        self.enhance_message(message, &options).await
    }

    /// Enhance a message with reactions, assets, and sender details
    async fn enhance_message(
        &self,
        message: Message,
        options: &MessageLoadOptions,
    ) -> Result<EnhancedMessage, String> {
        let message_id = &message.id;

        // Load all associated data sequentially
        let reactions = if options.load_reactions {
            self.load_message_reactions(message_id).await?
        } else {
            Vec::new()
        };
        
        let assets = if options.load_assets {
            self.load_message_assets(message_id).await?
        } else {
            Vec::new()
        };
        
        let sender_details = if options.load_sender_details {
            if let Some(sender_id) = &message.sender_id {
                self.load_user_details(sender_id).await.map(Some)?
            } else {
                None
            }
        } else {
            None
        };

        Ok(EnhancedMessage {
            message,
            reactions,
            assets,
            sender_details,
        })
    }

    /// Load reactions for a specific message
    async fn load_message_reactions(&self, message_id: &str) -> Result<Vec<MessageReaction>, String> {
        let rows = sqlx::query("SELECT * FROM message_reactions WHERE message_id = ?1 ORDER BY created_at")
            .bind(message_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to load reactions: {}", e))?;

        Ok(rows
            .into_iter()
            .map(|row| MessageReaction {
                id: row.get("id"),
                message_id: row.get("message_id"),
                user_id: row.get("user_id"),
                external_id: row.get("external_id"),
                reaction_type: row.get("reaction_type"),
                reaction_value: row.get("reaction_value"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    /// Load assets (attachments, images, files) for a specific message
    async fn load_message_assets(&self, message_id: &str) -> Result<Vec<Asset>, String> {
        let rows = sqlx::query("SELECT * FROM assets WHERE message_id = ?1 ORDER BY created_at")
            .bind(message_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to load assets: {}", e))?;

        Ok(rows
            .into_iter()
            .map(|row| Asset {
                id: row.get("id"),
                profile_id: row.get("profile_id"),
                name: row.get("name"),
                url: row.get("url"),
                asset_type: row.get("type"),
                mime_type: row.get("mime_type"),
                path: row.get("path"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    /// Load detailed user information
    async fn load_user_details(&self, user_id: &str) -> Result<User, String> {
        let row = sqlx::query("SELECT * FROM user WHERE id = ?1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| format!("Failed to load user: {}", e))?;

        if let Some(row) = row {
            Ok(User {
                id: row.get("id"),
                email: row.try_get("email").ok(),
                phone: row.try_get("phone").ok(),
                display_name: row.try_get("display_name").ok(),
                first_name: row.try_get("first_name").ok(),
                last_name: row.try_get("last_name").ok(),
                avatar_url: row.try_get("avatar_url").ok(),
                avatar_data: row.try_get("avatar_data").ok(),
                job_title: row.try_get("job_title").ok(),
                department: row.try_get("department").ok(),
                company: row.try_get("company").ok(),
                office_location: row.try_get("office_location").ok(),
                first_seen: row.get("first_seen"),
                last_updated: row.get("last_updated"),
            })
        } else {
            // Return a basic user if not found in database
            let now = chrono::Utc::now().to_rfc3339();
            Ok(User {
                id: user_id.to_string(),
                email: None,
                phone: None,
                display_name: Some("Unknown User".to_string()),
                first_name: None,
                last_name: None,
                avatar_url: None,
                avatar_data: None,
                job_title: None,
                department: None,
                company: None,
                office_location: None,
                first_seen: now.clone(),
                last_updated: now,
            })
        }
    }

    /// Sync message assets from Microsoft Graph API
    pub async fn sync_message_assets_from_graph(
        &self,
        message: &Message,
        auth: &ProviderAuth,
        _provider: &MicrosoftProvider,
    ) -> Result<Vec<Asset>, String> {
        // This would integrate with Microsoft Graph API to fetch:
        // 1. Attachments
        // 2. Hosted content (images, files)
        // 3. Reactions
        
        let api_endpoint = std::env::var("MICROSOFT_API_ENDPOINT")
            .unwrap_or_else(|_| "https://graph.microsoft.com/v1.0".to_string());
        
        let mut assets = Vec::new();
        
        if let Some(conversation_id) = &message.conversation_id {
            // Fetch message attachments
            let attachments_url = format!(
                "{}/chats/{}/messages/{}/attachments",
                api_endpoint, conversation_id, message.external_id
            );
            
            if let Ok(response) = reqwest::Client::new()
                .get(&attachments_url)
                .bearer_auth(&auth.access_token)
                .send()
                .await
            {
                if response.status().is_success() {
                    if let Ok(data) = response.json::<serde_json::Value>().await {
                        if let Some(attachments) = data["value"].as_array() {
                            for attachment in attachments {
                                let asset_id = Uuid::new_v4().to_string();
                                let now = Utc::now().to_rfc3339();
                                
                                let asset = Asset {
                                    id: asset_id.clone(),
                                    profile_id: "default".to_string(), // TODO: Get from context
                                    name: attachment["name"].as_str().unwrap_or("Unknown").to_string(),
                                    url: attachment["contentUrl"].as_str().map(|s| s.to_string()),
                                    asset_type: "attachment".to_string(),
                                    mime_type: attachment["contentType"].as_str().unwrap_or("application/octet-stream").to_string(),
                                    path: None,
                                    created_at: now.clone(),
                                    updated_at: now,
                                };
                                
                                // Save to database
                                let _ = self.save_asset(&asset, &message.id).await;
                                assets.push(asset);
                            }
                        }
                    }
                }
            }
            
            // Fetch reactions
            let reactions_url = format!(
                "{}/chats/{}/messages/{}",
                api_endpoint, conversation_id, message.external_id
            );
            
            if let Ok(response) = reqwest::Client::new()
                .get(&reactions_url)
                .bearer_auth(&auth.access_token)
                .send()
                .await
            {
                if response.status().is_success() {
                    if let Ok(data) = response.json::<serde_json::Value>().await {
                        if let Some(reactions) = data["reactions"].as_array() {
                            for reaction in reactions {
                                let _ = self.save_reaction(&MessageReaction {
                                    id: Uuid::new_v4().to_string(),
                                    message_id: message.id.clone(),
                                    user_id: reaction["user"]["user"]["id"].as_str().unwrap_or("unknown").to_string(),
                                    external_id: reaction["user"]["user"]["id"].as_str().unwrap_or("unknown").to_string(),
                                    reaction_type: "emoji".to_string(),
                                    reaction_value: reaction["reactionType"].as_str().unwrap_or("👍").to_string(),
                                    created_at: Utc::now().to_rfc3339(),
                                    updated_at: Utc::now().to_rfc3339(),
                                }).await;
                            }
                        }
                    }
                }
            }
        }
        
        Ok(assets)
    }

    /// Save an asset to the database
    async fn save_asset(&self, asset: &Asset, message_id: &str) -> Result<(), String> {
        sqlx::query(
            r#"INSERT OR REPLACE INTO assets (
                id, profile_id, message_id, name, url, type, mime_type, path, 
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#
        )
        .bind(&asset.id)
        .bind(&asset.profile_id)
        .bind(message_id)
        .bind(&asset.name)
        .bind(&asset.url)
        .bind(&asset.asset_type)
        .bind(&asset.mime_type)
        .bind(&asset.path)
        .bind(&asset.created_at)
        .bind(&asset.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to save asset: {}", e))?;

        Ok(())
    }

    /// Save a reaction to the database
    async fn save_reaction(&self, reaction: &MessageReaction) -> Result<(), String> {
        sqlx::query(
            r#"INSERT OR REPLACE INTO message_reactions (
                id, message_id, user_id, external_id, reaction_type, reaction_value,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#
        )
        .bind(&reaction.id)
        .bind(&reaction.message_id)
        .bind(&reaction.user_id)
        .bind(&reaction.external_id)
        .bind(&reaction.reaction_type)
        .bind(&reaction.reaction_value)
        .bind(&reaction.created_at)
        .bind(&reaction.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to save reaction: {}", e))?;

        Ok(())
    }

    pub fn row_to_message(&self, row: sqlx::sqlite::SqliteRow) -> Message {
        let recipients_json: Option<String> = row.get("recipients");
        let cc_recipients_json: Option<String> = row.get("cc_recipients");
        let bcc_recipients_json: Option<String> = row.get("bcc_recipients");
        let attachments_json: Option<String> = row.get("attachments");

        Message {
            id: row.get("id"),
            provider_id: row.get("provider_id"),
            external_id: row.get("external_id"),
            r#type: row.get("type"),
            subject: row.get("subject"),
            content: row.get("content"),
            content_type: row.get("content_type"),
            summary: row.get("summary"),
            presentation: row.try_get::<Option<String>, _>("presentation").ok().flatten().and_then(|p| serde_json::from_str(&p).ok()),
            sender_id: row.get("sender_id"),
            sender_name: row.get("sender_name"),
            sender_email: row.get("sender_email"),
            recipients: Message::parse_recipients(recipients_json.as_deref()),
            cc_recipients: Message::parse_recipients(cc_recipients_json.as_deref()),
            bcc_recipients: Message::parse_recipients(bcc_recipients_json.as_deref()),
            conversation_id: row.get("conversation_id"),
            chat_id: row.get("chat_id"),
            team_id: row.get("team_id"),
            channel_id: row.get("channel_id"),
            importance: row.get("importance"),
            is_read: row.get("is_read"),
            is_draft: row.get("is_draft"),
            has_attachments: row.get("has_attachments"),
            attachments: Message::parse_attachments(attachments_json.as_deref()),
            thread_id: row.try_get("thread_id").ok().flatten(),
            reply_to_id: row.get("reply_to_id"),
            conversation_index: row.get("conversation_index"),
            created_datetime: row.get("created_datetime"),
            last_modified_datetime: row.get("last_modified_datetime"),
            received_datetime: row.get("received_datetime"),
            sent_datetime: row.get("sent_datetime"),
            deleted_datetime: row.get("deleted_datetime"),
            timestamp: row.get("timestamp"),
            cached_at: row.get("cached_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            provider_name: row.get("provider_name"),
            metadata: row.get("metadata"),
        }
    }
}


pub const SNIPPET_OPEN: &str = "\u{2}";
pub const SNIPPET_CLOSE: &str = "\u{3}";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageSnippet {
    pub message_id: String,
    pub snippet: String,
}

pub async fn search_schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query("CREATE VIRTUAL TABLE IF NOT EXISTS message_fts USING fts5(message_id UNINDEXED, subject, body, tokenize = 'unicode61 remove_diacritics 2')")
        .execute(pool)
        .await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS message_fts_state (message_id TEXT PRIMARY KEY, stamp TEXT NOT NULL)")
        .execute(pool)
        .await?;
    Ok(())
}

pub fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-' && c != '_' && c != '.' && c != '@')
        .map(|term| term.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{}\"*", term.replace('"', "")))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

pub async fn refresh_search_index(pool: &Pool<Sqlite>) -> Result<usize, String> {
    let stale = sqlx::query(
        "SELECT m.id, m.subject, m.content, m.content_type, COALESCE(m.updated_at, m.timestamp) AS stamp FROM message m
         LEFT JOIN message_fts_state s ON s.message_id = m.id
         WHERE s.message_id IS NULL OR s.stamp != COALESCE(m.updated_at, m.timestamp)
         LIMIT 5000",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to read messages to index: {}", e))?;
    let mut tx = pool.begin().await.map_err(|e| format!("Failed to start indexing: {}", e))?;
    for row in &stale {
        let id: String = row.get("id");
        let subject: Option<String> = row.get("subject");
        let content: Option<String> = row.get("content");
        let content_type: Option<String> = row.get("content_type");
        let stamp: String = row.get("stamp");
        let body = crate::utils::text_extraction::extract_text_from_content(content.as_deref().unwrap_or(""), content_type.as_deref().unwrap_or("text"));
        sqlx::query("DELETE FROM message_fts WHERE message_id = ?1").bind(&id).execute(&mut *tx).await.map_err(|e| format!("Failed to reindex a message: {}", e))?;
        sqlx::query("INSERT INTO message_fts (message_id, subject, body) VALUES (?1, ?2, ?3)")
            .bind(&id)
            .bind(subject.unwrap_or_default())
            .bind(body)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to index a message: {}", e))?;
        sqlx::query("INSERT INTO message_fts_state (message_id, stamp) VALUES (?1, ?2) ON CONFLICT(message_id) DO UPDATE SET stamp = excluded.stamp")
            .bind(&id)
            .bind(&stamp)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to record the index state: {}", e))?;
    }
    sqlx::query("DELETE FROM message_fts WHERE message_id IN (SELECT s.message_id FROM message_fts_state s LEFT JOIN message m ON m.id = s.message_id WHERE m.id IS NULL)")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to drop removed messages from the index: {}", e))?;
    sqlx::query("DELETE FROM message_fts_state WHERE message_id NOT IN (SELECT id FROM message)")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to drop removed messages from the index: {}", e))?;
    tx.commit().await.map_err(|e| format!("Failed to finish indexing: {}", e))?;
    Ok(stale.len())
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchFilter {
    pub text: String,
    pub from: Vec<String>,
    pub within: Vec<String>,
    pub has: Vec<String>,
    pub is: Vec<String>,
    pub before: Option<String>,
    pub after: Option<String>,
}

impl SearchFilter {
    pub fn parse(input: &str) -> SearchFilter {
        let mut filter = SearchFilter::default();
        let mut words: Vec<&str> = Vec::new();
        for token in input.split_whitespace() {
            match token.split_once(':') {
                Some(("from", value)) if !value.is_empty() => filter.from.push(value.trim_start_matches('@').to_lowercase()),
                Some(("in", value)) if !value.is_empty() => filter.within.push(value.trim_start_matches('#').to_lowercase()),
                Some(("has", value)) if !value.is_empty() => filter.has.push(value.to_lowercase()),
                Some(("is", value)) if !value.is_empty() => filter.is.push(value.to_lowercase()),
                Some(("before", value)) if chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok() => filter.before = Some(value.to_string()),
                Some(("after", value)) if chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok() => filter.after = Some(value.to_string()),
                _ => words.push(token),
            }
        }
        filter.text = words.join(" ");
        filter
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.from.is_empty() && self.within.is_empty() && self.has.is_empty() && self.is.is_empty() && self.before.is_none() && self.after.is_none()
    }

    fn clauses(&self, profile_id: &str) -> (String, Vec<String>) {
        let mut sql = String::new();
        let mut binds: Vec<String> = Vec::new();
        for from in &self.from {
            binds.push(format!("%{}%", from));
            sql.push_str(&format!(" AND (lower(COALESCE(m.sender_name, '')) LIKE ?{n} OR lower(COALESCE(m.sender_email, '')) LIKE ?{n})", n = binds.len() + 3));
        }
        for within in &self.within {
            binds.push(format!("%{}%", within));
            sql.push_str(&format!(" AND m.conversation_id IN (SELECT id FROM conversation WHERE lower(name) LIKE ?{})", binds.len() + 3));
        }
        for has in &self.has {
            sql.push_str(match has.as_str() {
                "link" => " AND json_array_length(json_extract(CASE WHEN json_valid(m.metadata) THEN m.metadata ELSE '{}' END, '$.structure.links')) > 0",
                "file" | "attachment" => " AND m.has_attachments = 1",
                "reaction" => " AND json_array_length(json_extract(CASE WHEN json_valid(m.metadata) THEN m.metadata ELSE '{}' END, '$.reactions')) > 0",
                "mention" => " AND json_array_length(json_extract(CASE WHEN json_valid(m.metadata) THEN m.metadata ELSE '{}' END, '$.structure.mentions')) > 0",
                _ => "",
            });
        }
        for is in &self.is {
            match is.as_str() {
                "unread" => sql.push_str(" AND m.is_read = 0"),
                "pinned" => sql.push_str(" AND m.id IN (SELECT message_id FROM pinned_message)"),
                "saved" => {
                    binds.push(profile_id.to_string());
                    sql.push_str(&format!(" AND m.id IN (SELECT message_id FROM bookmarked_message WHERE profile_id = ?{})", binds.len() + 3));
                }
                "thread" => sql.push_str(" AND m.thread_id IS NOT NULL"),
                _ => {}
            }
        }
        if let Some(before) = &self.before {
            binds.push(before.clone());
            sql.push_str(&format!(" AND m.timestamp < ?{}", binds.len() + 3));
        }
        if let Some(after) = &self.after {
            binds.push(after.clone());
            sql.push_str(&format!(" AND m.timestamp >= ?{}", binds.len() + 3));
        }
        (sql, binds)
    }
}

pub async fn search_messages(pool: &Pool<Sqlite>, profile_id: &str, filter: &SearchFilter, conversation_ids: Option<&[String]>, limit: i64) -> Result<Vec<(Message, String)>, String> {
    if filter.is_empty() {
        return Ok(Vec::new());
    }
    let query = fts_query(&filter.text);
    if query.is_some() {
        refresh_search_index(pool).await?;
    }
    let scoped = conversation_ids.map(|ids| ids.iter().map(|id| format!("'{}'", id.replace('\'', "''"))).collect::<Vec<_>>().join(","));
    let scope = match &scoped {
        Some(list) if list.is_empty() => return Ok(Vec::new()),
        Some(list) => format!(" AND m.conversation_id IN ({})", list),
        None => String::new(),
    };
    let (clauses, binds) = filter.clauses(profile_id);
    let sql = match query {
        Some(_) => format!(
            "SELECT m.*, snippet(message_fts, 2, '{open}', '{close}', '…', 14) AS hit, snippet(message_fts, 1, '{open}', '{close}', '…', 14) AS subject_hit
             FROM message_fts f JOIN message m ON m.id = f.message_id JOIN provider p ON p.id = m.provider_id
             WHERE message_fts MATCH ?1 AND p.profile_id = ?2 AND m.deleted_datetime IS NULL{scope}{clauses}
             ORDER BY bm25(message_fts, 0.0, 4.0, 1.0) + (julianday('now') - julianday(m.timestamp)) / 90.0 ASC
             LIMIT ?3",
            open = SNIPPET_OPEN,
            close = SNIPPET_CLOSE,
            scope = scope,
            clauses = clauses
        ),
        None => format!(
            "SELECT m.*, substr(COALESCE(m.summary, m.content, ''), 1, 160) AS hit, '' AS subject_hit
             FROM message m JOIN provider p ON p.id = m.provider_id
             WHERE ?1 IS NULL AND p.profile_id = ?2 AND m.deleted_datetime IS NULL{scope}{clauses}
             ORDER BY m.timestamp DESC
             LIMIT ?3",
            scope = scope,
            clauses = clauses
        ),
    };
    let mut statement = sqlx::query(&sql).bind(&query).bind(profile_id).bind(limit);
    for bind in &binds {
        statement = statement.bind(bind);
    }
    let rows = statement.fetch_all(pool).await.map_err(|e| format!("Failed to search messages: {}", e))?;
    let service = MessageService::new(pool.clone());
    Ok(rows
        .into_iter()
        .map(|row| {
            let hit: String = row.try_get("hit").unwrap_or_default();
            let subject_hit: String = row.try_get("subject_hit").unwrap_or_default();
            let snippet = if hit.contains(SNIPPET_OPEN) || subject_hit.is_empty() { hit } else { subject_hit };
            (service.row_to_message(row), snippet)
        })
        .collect())
}

pub const SHARED_SCOPE: &str = "m.conversation_id IN (SELECT value FROM json_each(?1) UNION SELECT gc.conversation_id FROM merge_group g JOIN merge_group_conversation gc ON gc.merge_group_id = g.id WHERE g.profile_id = ?2 AND g.lead_conversation_id = ?3)";
pub const SHARED_LIMIT: u32 = 200;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GetSharedInput {
    #[serde(default)]
    pub conversation_ids: Vec<String>,
    pub lead_conversation_id: Option<String>,
    pub limit: Option<u32>,
}

impl GetSharedInput {
    pub fn ids(&self) -> String {
        serde_json::to_string(&self.conversation_ids).unwrap_or_else(|_| "[]".to_string())
    }

    pub fn limit(&self) -> u32 {
        self.limit.unwrap_or(SHARED_LIMIT).clamp(1, 1000)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SharedFile {
    pub id: String,
    pub attachment_id: String,
    pub name: String,
    pub content_type: String,
    pub size: i64,
    pub url: Option<String>,
    pub message_id: String,
    pub conversation_id: String,
    pub conversation_name: String,
    pub sender: String,
    pub at: String,
}

pub async fn get_shared_files(pool: &Pool<Sqlite>, profile_id: &str, input: &GetSharedInput) -> Result<Vec<SharedFile>, String> {
    let rows = sqlx::query(&format!(
        "SELECT m.id AS message_id, m.conversation_id, COALESCE(c.name, '') AS conversation_name, COALESCE(m.sender_name, m.sender_email, '') AS sender, m.timestamp AS at, a.value AS attachment
         FROM message m
         JOIN provider p ON p.id = m.provider_id
         LEFT JOIN conversation c ON c.id = m.conversation_id
         JOIN json_each(CASE WHEN json_valid(m.attachments) THEN m.attachments ELSE '[]' END) a
         WHERE p.profile_id = ?2 AND {scope} AND m.deleted_datetime IS NULL
           AND json_type(a.value) = 'object' AND COALESCE(json_extract(a.value, '$.is_inline'), 0) = 0
         ORDER BY m.timestamp DESC, a.key
         LIMIT ?4",
        scope = SHARED_SCOPE
    ))
    .bind(input.ids())
    .bind(profile_id)
    .bind(&input.lead_conversation_id)
    .bind(input.limit())
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to load the shared files: {}", e))?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let attachment: serde_json::Value = serde_json::from_str(&row.get::<String, _>("attachment")).ok()?;
            let attachment_id = attachment["id"].as_str().filter(|id| !id.is_empty())?.to_string();
            let message_id: String = row.get("message_id");
            Some(SharedFile {
                id: format!("{}:{}", message_id, attachment_id),
                name: attachment["name"].as_str().unwrap_or_default().to_string(),
                content_type: attachment["content_type"].as_str().unwrap_or_default().to_string(),
                size: attachment["size"].as_i64().unwrap_or(0),
                url: attachment["url"].as_str().filter(|u| !u.starts_with("data:")).map(String::from),
                attachment_id,
                message_id,
                conversation_id: row.get::<Option<String>, _>("conversation_id").unwrap_or_default(),
                conversation_name: row.get("conversation_name"),
                sender: row.get("sender"),
                at: row.get("at"),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn fts_query_quotes_every_term_as_a_prefix() {
        assert_eq!(fts_query("fast pris").as_deref(), Some("\"fast\"* \"pris\"*"));
        assert_eq!(fts_query("  \"quoted\" (x) ").as_deref(), Some("\"quoted\"* \"x\"*"));
        assert_eq!(fts_query("  ,, "), None);
    }

    #[tokio::test]
    async fn full_text_search_finds_words_anywhere_with_a_centred_snippet_and_scope() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let provider_id = test_provider(&pool, &profile_id, "mock").await;
        let first = test_conversation(&pool, "Avtale", "email").await;
        let second = test_conversation(&pool, "Lunsj", "direct_message").await;
        let long = format!("{} vi går for fast månedspris fra januar {}", "innledning ".repeat(40), "avslutning ".repeat(40));
        let hit = test_message(&pool, &first, &provider_id, &long).await;
        test_message(&pool, &second, &provider_id, "Lunsj kl 11:30 i morgen, fast bord").await;
        test_message(&pool, &second, &provider_id, "Ingenting relevant her").await;

        let found = search_messages(&pool, &profile_id, &SearchFilter::parse("månedspris"), None, 10).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0.id, hit);
        let snippet = &found[0].1;
        assert!(snippet.contains(&format!("{}månedspris{}", SNIPPET_OPEN, SNIPPET_CLOSE)));
        assert!(!snippet.starts_with("innledning innledning innledning innledning innledning innledning innledning innledning innledning innledning innledning innledning innledning innledning"));

        assert_eq!(search_messages(&pool, &profile_id, &SearchFilter::parse("fast"), None, 10).await.unwrap().len(), 2);
        assert_eq!(search_messages(&pool, &profile_id, &SearchFilter::parse("fast"), Some(&[second.clone()]), 10).await.unwrap().len(), 1);
        assert!(search_messages(&pool, &profile_id, &SearchFilter::parse("fast"), Some(&[]), 10).await.unwrap().is_empty());
        assert!(search_messages(&pool, "someone-else", &SearchFilter::parse("fast"), None, 10).await.unwrap().is_empty());

        sqlx::query("UPDATE message SET content = 'nå heter det fastpris', updated_at = '2099-01-01T00:00:00Z' WHERE id = ?1").bind(&hit).execute(&pool).await.unwrap();
        assert!(search_messages(&pool, &profile_id, &SearchFilter::parse("månedspris"), None, 10).await.unwrap().is_empty());
        sqlx::query("DELETE FROM message WHERE id = ?1").bind(&hit).execute(&pool).await.unwrap();
        refresh_search_index(&pool).await.unwrap();
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message_fts").fetch_one(&pool).await.unwrap();
        assert_eq!(left, 2);
    }

    #[tokio::test]
    async fn load_conversation_messages_returns_messages() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let conv_id = test_conversation(&pool, "Msg Test", "chat").await;
        let provider_id = test_provider(&pool, &profile_id, "microsoft").await;

        test_message(&pool, &conv_id, &provider_id, "First message").await;
        test_message(&pool, &conv_id, &provider_id, "Second message").await;

        let service = MessageService::new(pool);
        let options = MessageLoadOptions {
            load_reactions: false,
            load_assets: false,
            load_sender_details: false,
            limit: None,
            offset: None,
        };

        let messages = service.load_conversation_messages(&conv_id, options).await.unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn load_conversation_messages_empty_conversation() {
        let pool = test_pool().await;
        let conv_id = test_conversation(&pool, "Empty Conv", "chat").await;

        let service = MessageService::new(pool);
        let options = MessageLoadOptions {
            load_reactions: false,
            load_assets: false,
            load_sender_details: false,
            limit: None,
            offset: None,
        };

        let messages = service.load_conversation_messages(&conv_id, options).await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn load_conversation_messages_respects_limit() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let conv_id = test_conversation(&pool, "Limit Test", "chat").await;
        let provider_id = test_provider(&pool, &profile_id, "microsoft").await;

        for i in 0..5 {
            test_message(&pool, &conv_id, &provider_id, &format!("Message {}", i)).await;
        }

        let service = MessageService::new(pool);
        let options = MessageLoadOptions {
            load_reactions: false,
            load_assets: false,
            load_sender_details: false,
            limit: Some(2),
            offset: None,
        };

        let messages = service.load_conversation_messages(&conv_id, options).await.unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn row_to_message_produces_correct_struct() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let conv_id = test_conversation(&pool, "Row Test", "chat").await;
        let provider_id = test_provider(&pool, &profile_id, "microsoft").await;
        let msg_id = test_message(&pool, &conv_id, &provider_id, "Test content").await;

        let row = sqlx::query("SELECT * FROM message WHERE id = ?1")
            .bind(&msg_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        let service = MessageService::new(pool);
        let message = service.row_to_message(row);

        assert_eq!(message.id, msg_id);
        assert_eq!(message.provider_id, provider_id);
        assert_eq!(message.content, Some("Test content".to_string()));
        assert_eq!(message.r#type, "chat");
        assert_eq!(message.sender_name, Some("Test User".to_string()));
        assert_eq!(message.importance, "normal");
        assert!(!message.is_read);
        assert!(!message.has_attachments);
    }

    #[tokio::test]
    async fn shared_files_come_newest_first_from_ids_or_a_merge_group_without_inline_or_deleted() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let provider_id = test_provider(&pool, &profile_id, "mock").await;
        let a = test_conversation(&pool, "a", "direct_message").await;
        let b = test_conversation(&pool, "b", "direct_message").await;
        let c = test_conversation(&pool, "c", "direct_message").await;
        crate::models::MergeGroup::post(&pool, &profile_id, &crate::models::MergeGroupInput { lead_conversation_id: a.clone(), conversation_ids: vec![b.clone()], group_lead_ids: vec![] }).await.unwrap();
        let attach = |id: &str, name: &str, inline: bool| serde_json::json!({"id": id, "name": name, "content_type": "application/pdf", "size": 2048, "is_inline": inline, "content_id": null, "url": null});
        let old = test_message(&pool, &a, &provider_id, "plan").await;
        let new = test_message(&pool, &b, &provider_id, "budget").await;
        let gone = test_message(&pool, &a, &provider_id, "removed").await;
        let other = test_message(&pool, &c, &provider_id, "elsewhere").await;
        test_message(&pool, &a, &provider_id, "no files").await;
        for (id, at, files) in [
            (&old, "2026-10-01T10:00:00Z", serde_json::json!([attach("f1", "plan.pdf", false), attach("f2", "logo.png", true)])),
            (&new, "2026-10-02T10:00:00Z", serde_json::json!([attach("f3", "budget.xlsx", false)])),
            (&gone, "2026-10-03T10:00:00Z", serde_json::json!([attach("f4", "gone.pdf", false)])),
            (&other, "2026-10-04T10:00:00Z", serde_json::json!([attach("f5", "other.pdf", false)])),
        ] {
            sqlx::query("UPDATE message SET attachments = ?1, has_attachments = 1, timestamp = ?2 WHERE id = ?3").bind(files.to_string()).bind(at).bind(id).execute(&pool).await.unwrap();
        }
        sqlx::query("UPDATE message SET attachments = 'not json' WHERE content = 'no files'").execute(&pool).await.unwrap();
        sqlx::query("UPDATE message SET deleted_datetime = '2026-10-05T10:00:00Z' WHERE id = ?1").bind(&gone).execute(&pool).await.unwrap();

        let grouped = get_shared_files(&pool, &profile_id, &GetSharedInput { lead_conversation_id: Some(a.clone()), ..Default::default() }).await.unwrap();
        assert_eq!(grouped.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), vec!["budget.xlsx", "plan.pdf"]);
        assert_eq!(grouped[0].id, format!("{}:f3", new));
        assert_eq!((grouped[0].conversation_id.as_str(), grouped[0].conversation_name.as_str(), grouped[0].sender.as_str(), grouped[0].size), (b.as_str(), "b", "Test User", 2048));
        assert_eq!((grouped[1].message_id.as_str(), grouped[1].attachment_id.as_str()), (old.as_str(), "f1"));

        let listed = get_shared_files(&pool, &profile_id, &GetSharedInput { conversation_ids: vec![a.clone(), c.clone()], ..Default::default() }).await.unwrap();
        assert_eq!(listed.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), vec!["other.pdf", "plan.pdf"]);
        assert_eq!(get_shared_files(&pool, &profile_id, &GetSharedInput { lead_conversation_id: Some(a.clone()), limit: Some(1), ..Default::default() }).await.unwrap().len(), 1);
        assert!(get_shared_files(&pool, &profile_id, &GetSharedInput::default()).await.unwrap().is_empty());
        assert!(get_shared_files(&pool, "someone-else", &GetSharedInput { conversation_ids: vec![a.clone()], ..Default::default() }).await.unwrap().is_empty());
    }
}
