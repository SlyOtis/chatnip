use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Pool, Row, Sqlite};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use uuid::Uuid;

use crate::models::PostIdentityInput;

#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    pub user_id: String,
    pub provider_type: String,
    pub external_id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub avatar_data: Option<String>,
    pub bot: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pin {
    pub person_id: String,
    pub provider_type: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Counterparts {
    pub conversation_id: String,
    pub conversation_type: String,
    pub user_ids: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MatchInput {
    pub own_user_ids: Vec<String>,
    pub identities: Vec<Identity>,
    pub conversations: Vec<Counterparts>,
    pub bot_only_senders: Vec<String>,
    pub pins: Vec<Pin>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchedPerson {
    pub display_name: String,
    pub primary_email: Option<String>,
    pub avatar_url: Option<String>,
    pub avatar_data: Option<String>,
    pub is_human: bool,
    pub identities: Vec<Identity>,
    pub conversation_ids: Vec<String>,
    pub pinned: Option<String>,
}

#[async_trait]
pub trait PeopleStore: Send + Sync {
    async fn load(&self, profile_id: &str) -> Result<MatchInput, String>;
    async fn save(&self, profile_id: &str, people: Vec<MatchedPerson>) -> Result<usize, String>;
    async fn post_identity(&self, profile_id: &str, input: &PostIdentityInput) -> Result<(), String>;
}

pub fn pin_matches(pin: &Pin, identity: &Identity) -> bool {
    if pin.key.contains('@') && identity.email.as_deref().map(normalise_email) == Some(normalise_email(&pin.key)) {
        return true;
    }
    if pin.provider_type != identity.provider_type {
        return false;
    }
    identity.external_id == pin.key || identity.display_name.as_deref().map(normalise_name).filter(|n| !n.is_empty()) == Some(normalise_name(&pin.key))
}

const SERVICE_PATTERNS: [&str; 18] = [
    "noreply",
    "no-reply",
    "no_reply",
    "do-not-reply",
    "donotreply",
    "notifications",
    "notification",
    "alerts",
    "alert",
    "mailer",
    "bounce",
    "newsletter",
    "digest",
    "bot",
    "automated",
    "system",
    "postmaster",
    "calendar-notification",
];

pub fn presence_status(raw: &str) -> String {
    match raw.trim().to_lowercase().replace(['_', ' '], "").as_str() {
        "active" | "online" | "available" | "availableidle" => "online",
        "busy" | "busyidle" | "donotdisturb" | "dnd" | "presenting" | "inacall" | "inameeting" => "dnd",
        "away" | "berightback" | "idle" => "away",
        _ => "offline",
    }
    .to_string()
}

pub fn normalise_email(email: &str) -> String {
    let trimmed = email.trim().to_lowercase();
    match trimmed.split_once('@') {
        Some((local, domain)) => {
            let local = local.split('+').next().unwrap_or(local);
            format!("{}@{}", local, domain)
        }
        None => trimmed,
    }
}

pub fn best_display_name(names: &[&str], email: Option<&str>) -> String {
    let looks_like_a_name = |n: &str| n.contains(' ') && n.chars().any(|c| c.is_uppercase());
    let spaced = |n: &str| n.contains(' ');
    let capitals = |n: &str| n.chars().any(|c| c.is_uppercase());
    let pick = names
        .iter()
        .filter(|n| !n.trim().is_empty() && !n.contains('@'))
        .max_by_key(|n| {
            let n = n.trim();
            (looks_like_a_name(n) as u8, spaced(n) as u8, capitals(n) as u8, n.len())
        })
        .map(|n| n.trim().to_string());
    match pick {
        Some(name) if spaced(&name) || capitals(&name) => name,
        other => {
            let local = other.or_else(|| email.and_then(|e| e.split('@').next()).map(|l| l.to_string())).unwrap_or_default();
            let titled = local
                .split(|c: char| c == '.' || c == '_' || c == '-' || c == ' ')
                .filter(|part| !part.is_empty())
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            if titled.is_empty() {
                "Unknown".to_string()
            } else {
                titled
            }
        }
    }
}

pub fn normalise_name(name: &str) -> String {
    crate::content::text::fold_name(name)
}

fn matches_service(text: &str) -> bool {
    let lower = text.to_lowercase();
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|t| !t.is_empty())
        .collect();
    SERVICE_PATTERNS.iter().any(|pattern| {
        if pattern.contains('-') {
            lower.contains(pattern)
        } else {
            tokens.iter().any(|t| t == pattern || t.split('-').any(|part| part == *pattern))
        }
    })
}

pub fn is_service(email: Option<&str>, display_name: Option<&str>) -> bool {
    if let Some(email) = email {
        let local = email.split('@').next().unwrap_or(email);
        if matches_service(local) {
            return true;
        }
        if local.eq_ignore_ascii_case("support") && display_name.map(|n| n.split_whitespace().count() < 2).unwrap_or(true) {
            return true;
        }
    }
    display_name.map(matches_service).unwrap_or(false)
}

struct Union {
    parent: Vec<usize>,
}

impl Union {
    fn new(size: usize) -> Self {
        Self { parent: (0..size).collect() }
    }

    fn find(&mut self, index: usize) -> usize {
        if self.parent[index] != index {
            let root = self.find(self.parent[index]);
            self.parent[index] = root;
        }
        self.parent[index]
    }

    fn join(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[rb] = ra;
        }
    }
}

pub fn match_people(input: &MatchInput) -> Vec<MatchedPerson> {
    let own: HashSet<&str> = input.own_user_ids.iter().map(|s| s.as_str()).collect();
    let identities: Vec<&Identity> = input.identities.iter().filter(|i| !own.contains(i.user_id.as_str())).collect();
    let mut dm_counterparts: HashSet<&str> = HashSet::new();
    for conversation in &input.conversations {
        if conversation.conversation_type == "direct_message" {
            let others: Vec<&String> = conversation.user_ids.iter().filter(|u| !own.contains(u.as_str())).collect();
            if others.len() == 1 {
                dm_counterparts.insert(others[0].as_str());
            }
        }
    }
    let mut union = Union::new(identities.len());
    let mut by_user: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut by_email: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, identity) in identities.iter().enumerate() {
        by_user.entry(identity.user_id.as_str()).or_default().push(index);
        if let Some(email) = identity.email.as_deref().filter(|e| e.contains('@')) {
            by_email.entry(normalise_email(email)).or_default().push(index);
        }
    }
    for group in by_user.values().chain(by_email.values()) {
        for pair in group.windows(2) {
            union.join(pair[0], pair[1]);
        }
    }
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, identity) in identities.iter().enumerate() {
        if !dm_counterparts.contains(identity.user_id.as_str()) {
            continue;
        }
        if let Some(name) = identity.display_name.as_deref().map(normalise_name).filter(|n| !n.is_empty()) {
            by_name.entry(name).or_default().push(index);
        }
    }
    for group in by_name.values() {
        for a in group {
            for b in group {
                if a >= b {
                    continue;
                }
                let email_a = identities[*a].email.as_deref().map(normalise_email);
                let email_b = identities[*b].email.as_deref().map(normalise_email);
                let conflict = matches!((&email_a, &email_b), (Some(x), Some(y)) if x != y);
                if !conflict {
                    union.join(*a, *b);
                }
            }
        }
    }
    let pinned: Vec<Option<&str>> = identities.iter().map(|identity| input.pins.iter().find(|pin| pin_matches(pin, identity)).map(|pin| pin.person_id.as_str())).collect();
    let mut by_pin: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, pin) in pinned.iter().enumerate() {
        if let Some(pin) = pin {
            by_pin.entry(pin).or_default().push(index);
        }
    }
    for group in by_pin.values() {
        for pair in group.windows(2) {
            union.join(pair[0], pair[1]);
        }
    }
    let bot_only: HashSet<&str> = input.bot_only_senders.iter().map(|s| s.as_str()).collect();
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for index in 0..identities.len() {
        let root = union.find(index);
        groups.entry(root).or_default().push(index);
    }
    let mut buckets: Vec<(Option<String>, Vec<usize>)> = Vec::new();
    for members in groups.into_values() {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for member in &members {
            if let Some(pin) = pinned[*member] {
                *counts.entry(pin).or_default() += 1;
            }
        }
        let majority = counts.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0))).map(|(pin, _)| pin);
        let mut split: HashMap<Option<&str>, Vec<usize>> = HashMap::new();
        for member in members {
            split.entry(pinned[member].or(majority)).or_default().push(member);
        }
        buckets.extend(split.into_iter().map(|(pin, list)| (pin.map(|p| p.to_string()), list)));
    }
    let mut people: Vec<MatchedPerson> = buckets
        .into_iter()
        .map(|(pin, members)| {
            let ids: Vec<Identity> = members.iter().map(|i| identities[*i].clone()).collect();
            let user_ids: HashSet<&str> = ids.iter().map(|i| i.user_id.as_str()).collect();
            let primary_email = ids.iter().filter_map(|i| i.email.as_deref()).map(normalise_email).next();
            let names: Vec<&str> = ids.iter().filter_map(|i| i.display_name.as_deref()).collect();
            let display_name = best_display_name(&names, primary_email.as_deref());
            let bot = ids.iter().any(|i| i.bot)
                || ids.iter().all(|i| bot_only.contains(i.user_id.as_str()))
                || ids.iter().any(|i| is_service(i.email.as_deref(), i.display_name.as_deref()));
            let mut conversation_ids: Vec<String> = Vec::new();
            for conversation in &input.conversations {
                let others: Vec<&String> = conversation.user_ids.iter().filter(|u| !own.contains(u.as_str())).collect();
                let mine = others.iter().any(|u| user_ids.contains(u.as_str()));
                if !mine {
                    continue;
                }
                let only_me = others.iter().all(|u| user_ids.contains(u.as_str()));
                if conversation.conversation_type == "direct_message" && only_me {
                    conversation_ids.push(conversation.conversation_id.clone());
                }
            }
            MatchedPerson {
                display_name,
                primary_email,
                avatar_url: ids.iter().filter_map(|i| i.avatar_url.clone()).next(),
                avatar_data: ids.iter().filter_map(|i| i.avatar_data.clone()).next(),
                is_human: !bot,
                identities: ids,
                conversation_ids,
                pinned: pin,
            }
        })
        .collect();
    let human_users: HashSet<String> = people
        .iter()
        .filter(|p| p.is_human)
        .flat_map(|p| p.identities.iter().map(|i| i.user_id.clone()))
        .collect();
    for conversation in &input.conversations {
        if conversation.conversation_type != "email" {
            continue;
        }
        let humans: Vec<&String> = conversation
            .user_ids
            .iter()
            .filter(|u| !own.contains(u.as_str()) && human_users.contains(*u))
            .collect();
        let distinct: HashSet<usize> = humans
            .iter()
            .filter_map(|u| people.iter().position(|p| p.identities.iter().any(|i| &i.user_id == *u)))
            .collect();
        if distinct.len() == 1 {
            let index = *distinct.iter().next().unwrap();
            people[index].conversation_ids.push(conversation.conversation_id.clone());
        }
    }
    for person in &mut people {
        person.identities.sort_by(|a, b| (&a.provider_type, &a.external_id).cmp(&(&b.provider_type, &b.external_id)));
        person.conversation_ids.sort();
        person.conversation_ids.dedup();
    }
    people.sort_by(|a, b| a.display_name.cmp(&b.display_name).then(a.primary_email.cmp(&b.primary_email)));
    people
}

pub struct SqlitePeopleStore {
    pool: Pool<Sqlite>,
}

impl SqlitePeopleStore {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PeopleStore for SqlitePeopleStore {
    async fn load(&self, profile_id: &str) -> Result<MatchInput, String> {
        let pool = &self.pool;
        let own_user_ids = crate::models::Profile::get_own_user_ids(pool, profile_id).await?;
        let rows = sqlx::query(
            r#"SELECT pu.user_id, p.provider_type, pu.external_id,
                      COALESCE(pu.email, u.email) AS email,
                      COALESCE(pu.display_name, u.display_name) AS display_name,
                      COALESCE(pu.avatar_url, u.avatar_url) AS avatar_url,
                      COALESCE(pu.avatar_data, u.avatar_data) AS avatar_data,
                      pu.details
               FROM provider_user pu
               JOIN provider p ON p.id = pu.provider_id
               JOIN user u ON u.id = pu.user_id
               WHERE p.profile_id = ?1"#,
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load identities: {}", e))?;
        let identities = rows
            .into_iter()
            .map(|row| {
                let details: Option<String> = row.try_get("details").ok().flatten();
                let bot = details
                    .and_then(|d| serde_json::from_str::<serde_json::Value>(&d).ok())
                    .map(|v| v.get("is_bot").and_then(|b| b.as_bool()).unwrap_or(false))
                    .unwrap_or(false);
                Identity {
                    user_id: row.get("user_id"),
                    provider_type: row.get("provider_type"),
                    external_id: row.get("external_id"),
                    email: row.try_get("email").ok().flatten(),
                    display_name: row.try_get("display_name").ok().flatten(),
                    avatar_url: row.try_get("avatar_url").ok().flatten(),
                    avatar_data: row.try_get("avatar_data").ok().flatten(),
                    bot,
                }
            })
            .collect();
        let member_rows = sqlx::query(
            r#"SELECT c.id, c.type, cm.user_id FROM conversation c
               JOIN conversation_member cm ON cm.conversation_id = c.id
               WHERE c.id IN (
                   SELECT cp.conversation_id FROM conversation_provider cp
                   JOIN provider p ON p.id = cp.provider_id WHERE p.profile_id = ?1
               )
               ORDER BY c.id"#,
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load conversation members: {}", e))?;
        let mut conversations: Vec<Counterparts> = Vec::new();
        for row in member_rows {
            let id: String = row.get("id");
            let user_id: String = row.get("user_id");
            match conversations.last_mut() {
                Some(last) if last.conversation_id == id => last.user_ids.push(user_id),
                _ => conversations.push(Counterparts { conversation_id: id, conversation_type: row.get("type"), user_ids: vec![user_id] }),
            }
        }
        let bot_only_senders: Vec<String> = sqlx::query_scalar(
            "SELECT sender_id FROM message WHERE sender_id IS NOT NULL GROUP BY sender_id HAVING SUM(CASE WHEN type = 'bot' THEN 0 ELSE 1 END) = 0",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load bot senders: {}", e))?;
        let pins = sqlx::query(
            r#"SELECT i.person_id, i.provider_type, COALESCE(i.external_id, i.email) AS pin_key FROM person_identity i
               JOIN person p ON p.id = i.person_id WHERE p.profile_id = ?1 AND i.manual = 1"#,
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load pins: {}", e))?
        .into_iter()
        .filter_map(|row| row.try_get::<Option<String>, _>("pin_key").ok().flatten().map(|key| Pin { person_id: row.get("person_id"), provider_type: row.get("provider_type"), key }))
        .collect();
        Ok(MatchInput { own_user_ids, identities, conversations, bot_only_senders, pins })
    }

    async fn save(&self, profile_id: &str, people: Vec<MatchedPerson>) -> Result<usize, String> {
        let pool = &self.pool;
        let now = Utc::now().to_rfc3339();
        let existing = sqlx::query(
            r#"SELECT i.provider_type, i.external_id, i.person_id, p.human_override, p.favourite_position, p.display_name
               FROM person_identity i JOIN person p ON p.id = i.person_id WHERE p.profile_id = ?1"#,
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load existing people: {}", e))?;
        let mut existing_person: HashMap<(String, String), String> = HashMap::new();
        let mut overrides: HashMap<String, Option<bool>> = HashMap::new();
        for row in existing {
            let person_id: String = row.get("person_id");
            existing_person.insert((row.get("provider_type"), row.get("external_id")), person_id.clone());
            overrides.insert(person_id, row.try_get("human_override").ok().flatten());
        }
        let mut tx = pool.begin().await.map_err(|e| format!("Failed to start transaction: {}", e))?;
        sqlx::query("UPDATE conversation SET person_id = NULL WHERE person_id IN (SELECT id FROM person WHERE profile_id = ?1)")
            .bind(profile_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to reset conversation links: {}", e))?;
        sqlx::query("DELETE FROM person_identity WHERE manual = 0 AND person_id IN (SELECT id FROM person WHERE profile_id = ?1)")
            .bind(profile_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to reset identities: {}", e))?;
        let mut kept: Vec<String> = Vec::new();
        let count = people.len();
        for person in people {
            let mut votes: HashMap<&String, usize> = HashMap::new();
            for identity in &person.identities {
                if let Some(id) = existing_person.get(&(identity.provider_type.clone(), identity.external_id.clone())) {
                    *votes.entry(id).or_default() += 1;
                }
            }
            let person_id = person
                .pinned
                .clone()
                .filter(|id| !kept.contains(id))
                .or_else(|| votes
                .into_iter()
                .filter(|(id, _)| !kept.contains(id))
                .max_by_key(|(id, n)| (*n, (*id).clone()))
                .map(|(id, _)| id.clone()))
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let is_human = overrides.get(&person_id).copied().flatten().unwrap_or(person.is_human);
            sqlx::query(
                r#"INSERT INTO person (id, profile_id, display_name, primary_email, avatar_url, avatar_data, is_human, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                   ON CONFLICT(id) DO UPDATE SET
                       display_name = CASE WHEN person.human_override IS NULL THEN excluded.display_name ELSE person.display_name END,
                       primary_email = excluded.primary_email,
                       avatar_url = COALESCE(excluded.avatar_url, person.avatar_url),
                       avatar_data = COALESCE(excluded.avatar_data, person.avatar_data),
                       is_human = ?7,
                       updated_at = excluded.updated_at"#,
            )
            .bind(&person_id)
            .bind(profile_id)
            .bind(&person.display_name)
            .bind(&person.primary_email)
            .bind(&person.avatar_url)
            .bind(&person.avatar_data)
            .bind(is_human)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to save person: {}", e))?;
            for identity in &person.identities {
                sqlx::query(
                    "INSERT INTO person_identity (id, person_id, user_id, provider_type, external_id, email, display_name) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(provider_type, external_id) DO NOTHING",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&person_id)
                .bind(&identity.user_id)
                .bind(&identity.provider_type)
                .bind(&identity.external_id)
                .bind(&identity.email)
                .bind(&identity.display_name)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to save identity: {}", e))?;
            }
            for conversation_id in &person.conversation_ids {
                sqlx::query("UPDATE conversation SET person_id = ?1 WHERE id = ?2")
                    .bind(&person_id)
                    .bind(conversation_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to link conversation: {}", e))?;
            }
            kept.push(person_id);
        }
        sqlx::query("DELETE FROM person WHERE profile_id = ?1 AND id NOT IN (SELECT DISTINCT person_id FROM person_identity)")
            .bind(profile_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to prune people: {}", e))?;
        tx.commit().await.map_err(|e| format!("Failed to commit people: {}", e))?;
        Ok(count)
    }

    async fn post_identity(&self, profile_id: &str, input: &PostIdentityInput) -> Result<(), String> {
        let pool = &self.pool;
        let email = input.email.as_deref().map(str::trim).filter(|e| !e.is_empty()).map(normalise_email);
        let key = input
            .external_id
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(str::to_string)
            .or_else(|| email.clone())
            .ok_or_else(|| "An email address or alias is required".to_string())?;
        let owner: Option<String> = sqlx::query_scalar("SELECT id FROM person WHERE id = ?1 AND profile_id = ?2")
            .bind(&input.person_id)
            .bind(profile_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to load person: {}", e))?;
        owner.ok_or_else(|| "Person not found".to_string())?;
        let pin = Pin { person_id: input.person_id.clone(), provider_type: input.provider_type.clone(), key: key.clone() };
        let loaded = self.load(profile_id).await?;
        let own: HashSet<&str> = loaded.own_user_ids.iter().map(|s| s.as_str()).collect();
        let matches: Vec<&Identity> = loaded.identities.iter().filter(|i| !own.contains(i.user_id.as_str()) && pin_matches(&pin, i)).collect();
        let mut tx = pool.begin().await.map_err(|e| format!("Failed to start transaction: {}", e))?;
        let losers: Vec<String> = {
            let mut found: Vec<String> = Vec::new();
            for identity in &matches {
                let current: Option<String> = sqlx::query_scalar("SELECT person_id FROM person_identity WHERE provider_type = ?1 AND external_id = ?2")
                    .bind(&identity.provider_type)
                    .bind(&identity.external_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to load identity owner: {}", e))?;
                if let Some(current) = current.filter(|c| c != &input.person_id) {
                    found.push(current);
                }
            }
            found.sort();
            found.dedup();
            found
        };
        for person_id in losers.iter().chain(std::iter::once(&input.person_id)) {
            sqlx::query("UPDATE person_identity SET manual = 1 WHERE person_id = ?1")
                .bind(person_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to pin identities: {}", e))?;
        }
        let rows: Vec<(Option<String>, String, String, Option<String>, Option<String>)> = if matches.is_empty() {
            vec![(None, input.provider_type.clone(), key.clone(), email.clone(), input.display_name.clone())]
        } else {
            matches.iter().map(|i| (Some(i.user_id.clone()), i.provider_type.clone(), i.external_id.clone(), i.email.clone(), i.display_name.clone())).collect()
        };
        for (user_id, provider_type, external_id, address, name) in rows {
            sqlx::query(
                r#"INSERT INTO person_identity (id, person_id, user_id, provider_type, external_id, email, display_name, manual) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)
                   ON CONFLICT(provider_type, external_id) DO UPDATE SET person_id = excluded.person_id, manual = 1"#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&input.person_id)
            .bind(&user_id)
            .bind(&provider_type)
            .bind(&external_id)
            .bind(&address)
            .bind(&name)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to save identity: {}", e))?;
        }
        sqlx::query("DELETE FROM person WHERE profile_id = ?1 AND id != ?2 AND id NOT IN (SELECT DISTINCT person_id FROM person_identity)")
            .bind(profile_id)
            .bind(&input.person_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to prune people: {}", e))?;
        tx.commit().await.map_err(|e| format!("Failed to commit identity: {}", e))?;
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryPeopleStore {

    pub input: MatchInput,
    pub saved: Mutex<Vec<MatchedPerson>>,
    pub pins: Mutex<Vec<Pin>>,
}

impl MemoryPeopleStore {
    pub fn new(input: MatchInput) -> Self {
        Self { input, saved: Mutex::new(Vec::new()), pins: Mutex::new(Vec::new()) }
    }
}

#[async_trait]
impl PeopleStore for MemoryPeopleStore {
    async fn load(&self, _profile_id: &str) -> Result<MatchInput, String> {
        let mut input = self.input.clone();
        input.pins.extend(self.pins.lock().map_err(|_| "People store poisoned".to_string())?.iter().cloned());
        Ok(input)
    }

    async fn save(&self, _profile_id: &str, people: Vec<MatchedPerson>) -> Result<usize, String> {
        let count = people.len();
        *self.saved.lock().map_err(|_| "People store poisoned".to_string())? = people;
        Ok(count)
    }

    async fn post_identity(&self, _profile_id: &str, input: &PostIdentityInput) -> Result<(), String> {
        let key = input
            .external_id
            .clone()
            .or_else(|| input.email.as_deref().map(normalise_email))
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| "An email address or alias is required".to_string())?;
        self.pins.lock().map_err(|_| "People store poisoned".to_string())?.push(Pin { person_id: input.person_id.clone(), provider_type: input.provider_type.clone(), key });
        Ok(())
    }
}

pub async fn rematch_with(store: &dyn PeopleStore, profile_id: &str) -> Result<usize, String> {
    let input = store.load(profile_id).await?;
    let people = match_people(&input);
    store.save(profile_id, people).await
}

pub async fn rematch(pool: &Pool<Sqlite>, profile_id: &str) -> Result<usize, String> {
    rematch_with(&SqlitePeopleStore::new(pool.clone()), profile_id).await
}

pub async fn post_identity_with(store: &dyn PeopleStore, profile_id: &str, input: &PostIdentityInput) -> Result<usize, String> {
    store.post_identity(profile_id, input).await?;
    rematch_with(store, profile_id).await
}

pub async fn post_identity(pool: &Pool<Sqlite>, profile_id: &str, input: &PostIdentityInput) -> Result<usize, String> {
    post_identity_with(&SqlitePeopleStore::new(pool.clone()), profile_id, input).await
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SharedConversation {
    pub id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IdentityProfile {
    pub identity: crate::models::PersonIdentity,
    pub key: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub department: Option<String>,
    pub company: Option<String>,
    pub office_location: Option<String>,
    pub avatar_url: Option<String>,
    pub avatar_data: Option<String>,
    pub presence: Option<String>,
    pub status_text: Option<String>,
    pub first_seen: Option<String>,
    pub last_seen: Option<String>,
    pub message_count: i64,
    pub conversations: Vec<SharedConversation>,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct PersonThread {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub mode: String,
    pub local: bool,
    pub direct: bool,
    pub provider_types: Vec<String>,
    pub provider_names: Vec<String>,
    pub message_count: i64,
    pub unread_count: i64,
    pub last_activity: String,
}

pub async fn get_threads(pool: &Pool<Sqlite>, profile_id: &str, person_id: &str) -> Result<Vec<PersonThread>, String> {
    let sql = format!(
        r#"WITH mine AS (
               SELECT c.id FROM conversation c WHERE c.person_id = ?2
               UNION SELECT cm.conversation_id FROM conversation_member cm JOIN person_identity pi ON pi.user_id = cm.user_id WHERE pi.person_id = ?2
               UNION SELECT m.conversation_id FROM message m JOIN person_identity pi ON pi.user_id = m.sender_id WHERE pi.person_id = ?2
           ),
           chats AS (
               SELECT ch.conversation_id AS id FROM chat ch, json_each(ch.sources) s
               WHERE ch.profile_id = ?1 AND ((json_extract(s.value, '$.kind') = 'person' AND json_extract(s.value, '$.id') = ?2)
                  OR (json_extract(s.value, '$.kind') = 'conversation' AND json_extract(s.value, '$.id') IN (SELECT id FROM mine)))
           )
           SELECT c.id, COALESCE(o.value, c.name) AS name, c.type, c.mode, c.last_activity, COALESCE(c.unread_count, 0) AS unread_count, c.person_id,
                  (SELECT COUNT(*) FROM message m WHERE m.conversation_id = c.id AND m.deleted_datetime IS NULL) AS message_count,
                  EXISTS (SELECT 1 FROM chat ch WHERE ch.conversation_id = c.id) AS local,
                  (SELECT GROUP_CONCAT(p.provider_type || char(31) || p.name, char(30)) FROM conversation_provider cp JOIN provider p ON p.id = cp.provider_id WHERE cp.conversation_id = c.id) AS providers
           FROM conversation c
           LEFT JOIN object_override o ON o.object_type = 'conversation' AND o.object_id = c.id AND o.scope_type = '' AND o.field = 'name'
           WHERE c.deleted_at IS NULL AND {} AND (c.id IN (SELECT id FROM mine) OR c.id IN (SELECT id FROM chats))
           ORDER BY c.last_activity DESC"#,
        crate::models::OWNED_BY_PROFILE
    );
    let rows = sqlx::query(&sql)
        .bind(profile_id)
        .bind(person_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list the person's chats: {}", e))?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let providers: Vec<(String, String)> = row
                .try_get::<Option<String>, _>("providers")
                .ok()
                .flatten()
                .unwrap_or_default()
                .split('\u{1e}')
                .filter(|part| !part.is_empty())
                .filter_map(|part| part.split_once('\u{1f}').map(|(kind, name)| (kind.to_string(), name.to_string())))
                .collect();
            PersonThread {
                id: row.get("id"),
                name: row.get("name"),
                kind: row.get("type"),
                mode: row.try_get::<String, _>("mode").unwrap_or_else(|_| "write".to_string()),
                local: row.get("local"),
                direct: row.try_get::<Option<String>, _>("person_id").ok().flatten().as_deref() == Some(person_id),
                provider_types: providers.iter().map(|(kind, _)| kind.clone()).collect(),
                provider_names: providers.into_iter().map(|(_, name)| name).collect(),
                message_count: row.get("message_count"),
                unread_count: row.get("unread_count"),
                last_activity: row.get("last_activity"),
            }
        })
        .collect())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PersonProfile {
    pub person: crate::models::Person,
    pub tags: Vec<crate::models::ObjectTag>,
    pub threads: Vec<PersonThread>,
    pub identities: Vec<IdentityProfile>,
    pub message_count: i64,
    pub conversation_count: i64,
    pub first_seen: Option<String>,
    pub last_seen: Option<String>,
    pub signatures: Vec<crate::models::PersonSignature>,
}

pub async fn refresh_signatures(pool: &Pool<Sqlite>, profile_id: &str, person: &crate::models::Person) -> Result<Vec<crate::models::PersonSignature>, String> {
    use crate::content::{html::visible_text, signature::{signature_hash, signature_of}};
    let mut found: std::collections::BTreeMap<String, crate::models::PersonSignature> = std::collections::BTreeMap::new();
    for identity in &person.identities {
        let Some(user_id) = &identity.user_id else { continue };
        let rows: Vec<(String, String, Option<String>, String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT m.id, m.provider_id, m.conversation_id, m.content, m.content_type, m.sender_email, m.timestamp FROM message m JOIN conversation c ON c.id = m.conversation_id WHERE m.sender_id = ?1 AND c.type = 'email' AND m.deleted_datetime IS NULL AND m.content IS NOT NULL ORDER BY m.timestamp DESC LIMIT 300",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to read the emails: {}", e))?;
        for (message_id, provider_id, conversation_id, content, content_type, sender_email, timestamp) in rows {
            let text = if content_type.to_lowercase().contains("html") { visible_text(&content) } else { content };
            let Some(signature) = signature_of(&text) else { continue };
            let hash = signature_hash(&signature);
            let entry = found.entry(hash.clone()).or_insert_with(|| crate::models::PersonSignature {
                id: uuid::Uuid::new_v4().to_string(),
                person_id: person.id.clone(),
                hash,
                text: signature.clone(),
                sender_email: sender_email.clone(),
                provider_id: Some(provider_id.clone()),
                provider_name: None,
                conversation_id: conversation_id.clone(),
                message_id: Some(message_id.clone()),
                first_seen: timestamp.clone(),
                last_seen: timestamp.clone(),
                use_count: 0,
            });
            entry.use_count += 1;
            if timestamp < entry.first_seen {
                entry.first_seen = timestamp.clone();
            }
            if timestamp > entry.last_seen {
                entry.last_seen = timestamp.clone();
                entry.text = signature;
                entry.sender_email = sender_email;
                entry.provider_id = Some(provider_id);
                entry.conversation_id = conversation_id;
                entry.message_id = Some(message_id);
            }
        }
    }
    for signature in found.values() {
        crate::models::PersonSignature::put(pool, profile_id, signature).await?;
    }
    crate::models::PersonSignature::get(pool, profile_id, &person.id).await
}

pub async fn person_profile(pool: &Pool<Sqlite>, profile_id: &str, person_id: &str) -> Result<PersonProfile, String> {
    let person = crate::models::Person::get(pool, profile_id, person_id).await?;
    let mut tags = crate::models::ObjectTag::get(pool, crate::models::tag::OBJECT_PERSON, person_id).await?;
    for identity in &person.identities {
        tags.extend(crate::models::ObjectTag::get(pool, crate::models::tag::OBJECT_IDENTITY, &identity.id).await?);
    }
    let mut identities = Vec::new();
    let mut shared: HashSet<String> = HashSet::new();
    for identity in &person.identities {
        let key = crate::models::tag::identity_key(&identity.provider_type, identity.external_id.as_deref(), identity.email.as_deref());
        let mut card = IdentityProfile {
            identity: identity.clone(),
            key,
            display_name: identity.display_name.clone(),
            email: identity.email.clone(),
            phone: None,
            job_title: None,
            department: None,
            company: None,
            office_location: None,
            avatar_url: None,
            avatar_data: None,
            presence: None,
            status_text: None,
            first_seen: None,
            last_seen: None,
            message_count: 0,
            conversations: Vec::new(),
        };
        if let Some(user_id) = &identity.user_id {
            if let Some(row) = sqlx::query("SELECT u.*, s.status, s.status_text FROM user u LEFT JOIN user_status s ON s.user_id = u.id WHERE u.id = ?1")
                .bind(user_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("Failed to read the account: {}", e))?
            {
                let text = |column: &str| row.try_get::<Option<String>, _>(column).ok().flatten().filter(|v| !v.trim().is_empty());
                card.display_name = card.display_name.or_else(|| text("display_name"));
                card.email = card.email.or_else(|| text("email"));
                card.phone = text("phone");
                card.job_title = text("job_title");
                card.department = text("department");
                card.company = text("company");
                card.office_location = text("office_location");
                card.avatar_url = text("avatar_url");
                card.avatar_data = text("avatar_data");
                card.presence = text("status");
                card.status_text = text("status_text");
                card.first_seen = text("first_seen");
            }
            let (count, first, last): (i64, Option<String>, Option<String>) = sqlx::query_as("SELECT COUNT(*), MIN(timestamp), MAX(timestamp) FROM message WHERE sender_id = ?1 AND deleted_datetime IS NULL")
                .bind(user_id)
                .fetch_one(pool)
                .await
                .map_err(|e| format!("Failed to count messages: {}", e))?;
            card.message_count = count;
            card.first_seen = first.or(card.first_seen);
            card.last_seen = last;
            let rows: Vec<(String, String, String)> = sqlx::query_as("SELECT c.id, c.name, c.type FROM conversation_member cm JOIN conversation c ON c.id = cm.conversation_id WHERE cm.user_id = ?1 AND c.deleted_at IS NULL ORDER BY c.last_activity DESC")
                .bind(user_id)
                .fetch_all(pool)
                .await
                .map_err(|e| format!("Failed to list shared conversations: {}", e))?;
            for (id, name, kind) in rows {
                shared.insert(id.clone());
                card.conversations.push(SharedConversation { id, name, kind });
            }
        }
        identities.push(card);
    }
    let message_count = identities.iter().map(|card| card.message_count).sum();
    let first_seen = identities.iter().filter_map(|card| card.first_seen.clone()).min();
    let last_seen = identities.iter().filter_map(|card| card.last_seen.clone()).max();
    let threads = get_threads(pool, profile_id, person_id).await?;
    let signatures = refresh_signatures(pool, profile_id, &person).await?;
    Ok(PersonProfile { person, tags, threads, identities, message_count, conversation_count: shared.len() as i64, first_seen, last_seen, signatures })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_conversation, test_pool, test_profile, test_provider};

    fn identity(user: &str, provider: &str, email: Option<&str>, name: Option<&str>) -> Identity {
        Identity {
            user_id: user.to_string(),
            provider_type: provider.to_string(),
            external_id: format!("{}-{}", provider, user),
            email: email.map(|e| e.to_string()),
            display_name: name.map(|n| n.to_string()),
            avatar_url: None,
            avatar_data: None,
            bot: false,
        }
    }

    fn dm(id: &str, user: &str) -> Counterparts {
        Counterparts { conversation_id: id.to_string(), conversation_type: "direct_message".to_string(), user_ids: vec!["me".to_string(), user.to_string()] }
    }

    #[test]
    fn same_email_joins_across_providers() {
        let input = MatchInput {
            own_user_ids: vec!["me".to_string()],
            identities: vec![
                identity("u1", "microsoft", Some("Eva.ortega@Northwind.example"), Some("Eva Ortega")),
                identity("u2", "slack", Some("eva.ortega+slack@northwind.example"), Some("eva")),
                identity("u3", "google", Some("other@northwind.example"), Some("Other Person")),
            ],
            conversations: vec![dm("c1", "u1"), dm("c2", "u2"), dm("c3", "u3")],
            bot_only_senders: vec![],
            pins: vec![],
        };
        let people = match_people(&input);
        assert_eq!(people.len(), 2);
        let eva = people.iter().find(|p| p.display_name == "Eva Ortega").unwrap();
        assert_eq!(eva.identities.len(), 2);
        assert_eq!(eva.conversation_ids, vec!["c1".to_string(), "c2".to_string()]);
        assert_eq!(eva.primary_email.as_deref(), Some("eva.ortega@northwind.example"));
        assert!(eva.is_human);
    }

    #[test]
    fn same_name_joins_only_for_direct_message_counterparts() {
        let input = MatchInput {
            own_user_ids: vec!["me".to_string()],
            identities: vec![
                identity("u1", "microsoft", None, Some("Oscar Strand")),
                identity("u2", "slack", Some("oscar@northwind.example"), Some("Oscar  Strand")),
                identity("u3", "slack", None, Some("Oscar Strand")),
            ],
            conversations: vec![
                dm("c1", "u1"),
                dm("c2", "u2"),
                Counterparts { conversation_id: "c3".to_string(), conversation_type: "channel".to_string(), user_ids: vec!["me".to_string(), "u3".to_string(), "u1".to_string()] },
            ],
            bot_only_senders: vec![],
            pins: vec![],
        };
        let people = match_people(&input);
        assert_eq!(people.len(), 2);
        let joined = people.iter().find(|p| p.identities.len() == 2).unwrap();
        assert!(joined.identities.iter().any(|i| i.user_id == "u1"));
        assert!(joined.identities.iter().any(|i| i.user_id == "u2"));
    }

    #[test]
    fn different_emails_never_merge_on_name() {
        let input = MatchInput {
            own_user_ids: vec!["me".to_string()],
            identities: vec![
                identity("u1", "microsoft", Some("milo@a.no"), Some("Max Sutton")),
                identity("u2", "slack", Some("milo@b.no"), Some("Max Sutton")),
            ],
            conversations: vec![dm("c1", "u1"), dm("c2", "u2")],
            bot_only_senders: vec![],
            pins: vec![],
        };
        assert_eq!(match_people(&input).len(), 2);
    }

    #[test]
    fn services_are_not_human_and_own_user_is_skipped() {
        let input = MatchInput {
            own_user_ids: vec!["me".to_string()],
            identities: vec![
                identity("me", "microsoft", Some("milo@northwind.example"), Some("Milo Oakes")),
                identity("u1", "google", Some("noreply@github.com"), Some("GitHub")),
                identity("u2", "google", Some("news@substack.com"), Some("Weekly Newsletter")),
                identity("u3", "slack", Some("drone@northwind.example"), Some("Drone")),
                identity("u4", "google", Some("anna@client.example"), Some("Anna Berg")),
            ],
            conversations: vec![
                Counterparts { conversation_id: "e1".to_string(), conversation_type: "email".to_string(), user_ids: vec!["me".to_string(), "u4".to_string()] },
                Counterparts { conversation_id: "e2".to_string(), conversation_type: "email".to_string(), user_ids: vec!["me".to_string(), "u1".to_string()] },
            ],
            bot_only_senders: vec!["u3".to_string()],
            pins: vec![],
        };
        let people = match_people(&input);
        assert!(people.iter().all(|p| p.identities.iter().all(|i| i.user_id != "me")));
        let by_user = |u: &str| people.iter().find(|p| p.identities.iter().any(|i| i.user_id == u)).unwrap();
        assert!(!by_user("u1").is_human);
        assert!(!by_user("u2").is_human);
        assert!(!by_user("u3").is_human);
        assert!(by_user("u4").is_human);
        assert_eq!(by_user("u4").conversation_ids, vec!["e1".to_string()]);
        assert!(by_user("u1").conversation_ids.is_empty());
    }

    #[test]
    fn the_best_display_name_prefers_real_names_and_titles_local_parts() {
        assert_eq!(best_display_name(&["eva.ortega", "Eva Ortega"], Some("eva.ortega@northwind.example")), "Eva Ortega");
        assert_eq!(best_display_name(&["eva.ortega"], Some("eva.ortega@northwind.example")), "Eva Ortega");
        assert_eq!(best_display_name(&["oscar.strand"], Some("oscar.strand@sundial.example")), "Oscar Strand");
        assert_eq!(best_display_name(&[], Some("kari_nordmann@tidewater.example")), "Kari Nordmann");
        assert_eq!(best_display_name(&["GitHub"], Some("notifications@github.com")), "GitHub");
        assert_eq!(best_display_name(&[], None), "Unknown");
    }

    #[test]
    fn support_without_a_name_is_a_service() {
        assert!(is_service(Some("support@acme.com"), Some("Support")));
        assert!(!is_service(Some("support@acme.com"), Some("Kari Nordmann")));
        assert!(is_service(Some("calendar-notification@google.com"), None));
        assert!(!is_service(Some("robotics@acme.com"), Some("Rob Bott")));
    }

    #[tokio::test]
    async fn memory_store_round_trip() {
        let store = MemoryPeopleStore::new(MatchInput {
            own_user_ids: vec!["me".to_string()],
            identities: vec![identity("u1", "microsoft", Some("a@b.no"), Some("A B"))],
            conversations: vec![dm("c1", "u1")],
            bot_only_senders: vec![],
            pins: vec![],
        });
        assert_eq!(rematch_with(&store, "p").await.unwrap(), 1);
        assert_eq!(store.saved.lock().unwrap()[0].conversation_ids, vec!["c1".to_string()]);
    }

    async fn seed_user(pool: &Pool<Sqlite>, provider_id: &str, external: &str, email: &str, name: &str) -> String {
        let user = crate::providers::traits::ResponseUser {
            id: external.to_string(),
            email: Some(email.to_string()),
            display_name: Some(name.to_string()),
            ..Default::default()
        };
        let user_id = crate::models::User::upsert_from_response(pool, &user).await.unwrap();
        crate::models::ProviderUser::create_or_update(pool, user_id.clone(), provider_id.to_string(), external.to_string()).await.unwrap();
        user_id
    }

    #[tokio::test]
    async fn sqlite_rematch_is_idempotent_and_keeps_person_ids() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let microsoft = test_provider(&pool, &profile, "microsoft").await;
        let slack = test_provider(&pool, &profile, "slack").await;
        let me = seed_user(&pool, &microsoft, "me-ms", "milo@northwind.example", "Milo Oakes").await;
        sqlx::query("UPDATE profile SET user_id = ?1 WHERE id = ?2").bind(&me).bind(&profile).execute(&pool).await.unwrap();
        let eva_ms = seed_user(&pool, &microsoft, "eva-ms", "eva@northwind.example", "Eva Ortega").await;
        let eva_slack = seed_user(&pool, &slack, "U123", "eva@northwind.example", "eva").await;
        let conversation = test_conversation(&pool, "Eva Ortega", "direct_message").await;
        for (provider, member) in [(&microsoft, &eva_ms), (&slack, &eva_slack)] {
            sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)")
                .bind(Uuid::new_v4().to_string())
                .bind(&conversation)
                .bind(provider)
                .bind(format!("ext-{}", provider))
                .bind(Utc::now().to_rfc3339())
                .execute(&pool)
                .await
                .unwrap();
            for user in [&me, member] {
                sqlx::query("INSERT OR IGNORE INTO conversation_member (id, conversation_id, user_id, role) VALUES (?1, ?2, ?3, 'member')")
                    .bind(Uuid::new_v4().to_string())
                    .bind(&conversation)
                    .bind(user)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        assert_eq!(rematch(&pool, &profile).await.unwrap(), 1);
        let first = crate::models::Person::all(&pool, &profile).await.unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].identities.len(), 2);
        assert_eq!(first[0].conversation_ids, vec![conversation.clone()]);
        crate::models::Person::put(&pool, &crate::models::PutPersonInput { person_id: first[0].id.clone(), display_name: Some("Eva".to_string()), is_human: None, favourite_position: Some(Some(0)) }).await.unwrap();
        assert_eq!(rematch(&pool, &profile).await.unwrap(), 1);
        let second = crate::models::Person::all(&pool, &profile).await.unwrap();
        assert_eq!(second[0].id, first[0].id);
        assert_eq!(second[0].favourite_position, Some(0));
        let linked: Option<String> = sqlx::query_scalar("SELECT person_id FROM conversation WHERE id = ?1").bind(&conversation).fetch_one(&pool).await.unwrap();
        assert_eq!(linked.as_deref(), Some(first[0].id.as_str()));
    }
    #[test]
    fn pins_override_email_joins_and_pull_identities_together() {
        let pin = |person: &str, provider: &str, key: &str| Pin { person_id: person.to_string(), provider_type: provider.to_string(), key: key.to_string() };
        let input = MatchInput {
            own_user_ids: vec!["me".to_string()],
            identities: vec![
                identity("u1", "microsoft", Some("kari@tidewater.example"), Some("Kari Nordmann")),
                identity("u2", "slack", Some("kari@tidewater.example"), Some("kari")),
                identity("u3", "google", Some("anna@client.example"), Some("Anna Berg")),
            ],
            conversations: vec![dm("c1", "u1"), dm("c2", "u2"), dm("c3", "u3")],
            bot_only_senders: vec![],
            pins: vec![pin("kari", "microsoft", "microsoft-u1"), pin("anna", "slack", "slack-u2"), pin("anna", "google", "anna@client.example")],
        };
        let people = match_people(&input);
        assert_eq!(people.len(), 2);
        let anna = people.iter().find(|p| p.pinned.as_deref() == Some("anna")).unwrap();
        assert_eq!(anna.identities.iter().map(|i| i.user_id.as_str()).collect::<Vec<_>>(), vec!["u3", "u2"]);
        assert_eq!(anna.conversation_ids, vec!["c2".to_string(), "c3".to_string()]);
        let kari = people.iter().find(|p| p.pinned.as_deref() == Some("kari")).unwrap();
        assert_eq!(kari.conversation_ids, vec!["c1".to_string()]);
    }

    #[test]
    fn an_alias_pin_matches_by_handle_within_its_provider_only() {
        let pin = Pin { person_id: "p".to_string(), provider_type: "slack".to_string(), key: "Kari N".to_string() };
        assert!(pin_matches(&pin, &identity("u1", "slack", None, Some("kari  n"))));
        assert!(!pin_matches(&pin, &identity("u1", "teams", None, Some("Kari N"))));
        let email = Pin { person_id: "p".to_string(), provider_type: "email".to_string(), key: "Kari@Tidewater.example".to_string() };
        assert!(pin_matches(&email, &identity("u2", "google", Some("kari@tidewater.example"), None)));
    }

    #[tokio::test]
    async fn memory_store_posts_identities_and_rematches() {
        let store = MemoryPeopleStore::new(MatchInput {
            own_user_ids: vec!["me".to_string()],
            identities: vec![identity("u1", "microsoft", Some("kari@tidewater.example"), Some("Kari Nordmann")), identity("u2", "slack", None, Some("kari.n"))],
            conversations: vec![dm("c1", "u1"), dm("c2", "u2")],
            bot_only_senders: vec![],
            pins: vec![],
        });
        assert_eq!(rematch_with(&store, "p").await.unwrap(), 2);
        let post = |provider: &str, email: Option<&str>, external: Option<&str>| PostIdentityInput {
            person_id: "kari".to_string(),
            provider_type: provider.to_string(),
            email: email.map(str::to_string),
            external_id: external.map(str::to_string),
            display_name: None,
        };
        post_identity_with(&store, "p", &post("email", Some("Kari@Tidewater.example"), None)).await.unwrap();
        assert_eq!(post_identity_with(&store, "p", &post("slack", None, Some("kari.n"))).await.unwrap(), 1);
        let saved = store.saved.lock().unwrap();
        assert_eq!(saved[0].pinned.as_deref(), Some("kari"));
        assert_eq!(saved[0].conversation_ids, vec!["c1".to_string(), "c2".to_string()]);
        assert!(store.post_identity("p", &post("slack", None, None)).await.is_err());
    }

    async fn dm_with(pool: &Pool<Sqlite>, provider: &str, me: &str, user: &str, name: &str) -> String {
        let conversation = test_conversation(pool, name, "direct_message").await;
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)")
            .bind(Uuid::new_v4().to_string())
            .bind(&conversation)
            .bind(provider)
            .bind(format!("ext-{}", conversation))
            .bind(Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .unwrap();
        for member in [me, user] {
            sqlx::query("INSERT OR IGNORE INTO conversation_member (id, conversation_id, user_id, role) VALUES (?1, ?2, ?3, 'member')")
                .bind(Uuid::new_v4().to_string())
                .bind(&conversation)
                .bind(member)
                .execute(pool)
                .await
                .unwrap();
        }
        conversation
    }

    #[tokio::test]
    async fn sqlite_post_identity_moves_aliases_and_survives_rematch() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let microsoft = test_provider(&pool, &profile, "microsoft").await;
        let slack = test_provider(&pool, &profile, "slack").await;
        let me = seed_user(&pool, &microsoft, "me-ms", "milo@northwind.example", "Milo Oakes").await;
        sqlx::query("UPDATE profile SET user_id = ?1 WHERE id = ?2").bind(&me).bind(&profile).execute(&pool).await.unwrap();
        let eva = seed_user(&pool, &microsoft, "eva-ms", "eva@northwind.example", "Eva Ortega").await;
        let alias = seed_user(&pool, &slack, "U9", "e.ortega@mail.example", "eo").await;
        let mail = dm_with(&pool, &microsoft, &me, &eva, "Eva Ortega").await;
        let chat = dm_with(&pool, &slack, &me, &alias, "eo").await;
        rematch(&pool, &profile).await.unwrap();
        let people = crate::models::Person::all(&pool, &profile).await.unwrap();
        assert_eq!(people.len(), 2);
        let target = people.iter().find(|p| p.display_name == "Eva Ortega").unwrap().id.clone();
        let input = PostIdentityInput { person_id: target.clone(), provider_type: "slack".to_string(), email: None, external_id: Some("U9".to_string()), display_name: None };
        post_identity(&pool, &profile, &input).await.unwrap();
        let pending = PostIdentityInput { person_id: target.clone(), provider_type: "email".to_string(), email: Some("Eva@Private.no".to_string()), external_id: None, display_name: Some("Eva privat".to_string()) };
        post_identity(&pool, &profile, &pending).await.unwrap();
        rematch(&pool, &profile).await.unwrap();
        let people = crate::models::Person::all(&pool, &profile).await.unwrap();
        assert_eq!(people.len(), 1);
        let person = &people[0];
        assert_eq!(person.id, target);
        let mut conversations = person.conversation_ids.clone();
        conversations.sort();
        let mut expected = vec![mail, chat];
        expected.sort();
        assert_eq!(conversations, expected);
        assert_eq!(person.identities.len(), 3);
        assert!(person.identities.iter().all(|i| i.manual));
        let waiting = person.identities.iter().find(|i| i.user_id.is_none()).unwrap();
        assert_eq!(waiting.email.as_deref(), Some("eva@private.no"));
        assert_eq!(waiting.display_name.as_deref(), Some("Eva privat"));
    }

    #[tokio::test]
    async fn sqlite_merges_survive_rematch() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let microsoft = test_provider(&pool, &profile, "microsoft").await;
        let slack = test_provider(&pool, &profile, "slack").await;
        let me = seed_user(&pool, &microsoft, "me-ms", "milo@northwind.example", "Milo Oakes").await;
        sqlx::query("UPDATE profile SET user_id = ?1 WHERE id = ?2").bind(&me).bind(&profile).execute(&pool).await.unwrap();
        let kari = seed_user(&pool, &microsoft, "kari-ms", "kari@tidewater.example", "Kari Nordmann").await;
        let other = seed_user(&pool, &slack, "U7", "knordmann@mail.example", "kn").await;
        dm_with(&pool, &microsoft, &me, &kari, "Kari").await;
        dm_with(&pool, &slack, &me, &other, "kn").await;
        rematch(&pool, &profile).await.unwrap();
        let people = crate::models::Person::all(&pool, &profile).await.unwrap();
        let target = people.iter().find(|p| p.display_name == "Kari Nordmann").unwrap().id.clone();
        let source = people.iter().find(|p| p.id != target).unwrap().id.clone();
        crate::models::Person::merge(&pool, &source, &target).await.unwrap();
        rematch(&pool, &profile).await.unwrap();
        let people = crate::models::Person::all(&pool, &profile).await.unwrap();
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].id, target);
        assert_eq!(people[0].conversation_ids.len(), 2);
    }

    #[tokio::test]
    async fn a_person_lists_every_chat_they_are_part_of_with_local_names_and_counts() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let microsoft = test_provider(&pool, &profile, "microsoft").await;
        let slack = test_provider(&pool, &profile, "slack").await;
        let me = seed_user(&pool, &microsoft, "me-ms", "milo@northwind.example", "Milo Oakes").await;
        sqlx::query("UPDATE profile SET user_id = ?1 WHERE id = ?2").bind(&me).bind(&profile).execute(&pool).await.unwrap();
        let eva = seed_user(&pool, &microsoft, "eva-ms", "eva@northwind.example", "Eva Ortega").await;
        let kari = seed_user(&pool, &slack, "U2", "kari@tidewater.example", "Kari Nordmann").await;
        let dm = dm_with(&pool, &microsoft, &me, &eva, "Eva Ortega").await;
        let group = dm_with(&pool, &slack, &me, &kari, "Design").await;
        sqlx::query("UPDATE conversation SET type = 'group_message' WHERE id = ?1").bind(&group).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO conversation_member (id, conversation_id, user_id, role) VALUES (?1, ?2, ?3, 'member')").bind(Uuid::new_v4().to_string()).bind(&group).bind(&eva).execute(&pool).await.unwrap();
        let elsewhere = dm_with(&pool, &slack, &me, &kari, "Kari Nordmann").await;
        crate::test_helpers::test_message(&pool, &dm, &microsoft, "Hei").await;
        crate::test_helpers::test_message(&pool, &dm, &microsoft, "Ses").await;
        rematch(&pool, &profile).await.unwrap();
        let person = crate::models::Person::all(&pool, &profile).await.unwrap().into_iter().find(|p| p.display_name == "Eva Ortega").unwrap();
        let notes = test_conversation(&pool, "Eva notes", "local").await;
        sqlx::query("INSERT INTO chat (id, profile_id, conversation_id, name, sources, created_at) VALUES (?1, ?2, ?3, 'Eva notes', ?4, ?5)")
            .bind(Uuid::new_v4().to_string())
            .bind(&profile)
            .bind(&notes)
            .bind(serde_json::json!([{ "kind": "person", "id": person.id }]).to_string())
            .bind(Utc::now().to_rfc3339())
            .execute(&pool)
            .await
            .unwrap();
        let local = crate::models::object_override::PutObjectOverrideInput { object_type: "conversation".into(), object_id: group.clone(), scope_type: None, scope_id: None, field: "name".into(), value: Some("Designgjengen".into()) };
        crate::models::ObjectOverride::put(&pool, &local).await.unwrap();

        let threads = get_threads(&pool, &profile, &person.id).await.unwrap();
        let ids: HashSet<&str> = threads.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, HashSet::from([dm.as_str(), group.as_str(), notes.as_str()]));
        assert!(!ids.contains(elsewhere.as_str()));
        let direct = threads.iter().find(|t| t.id == dm).unwrap();
        assert!(direct.direct && !direct.local);
        assert_eq!(direct.message_count, 2);
        assert_eq!(direct.provider_types, vec!["microsoft".to_string()]);
        let shared = threads.iter().find(|t| t.id == group).unwrap();
        assert_eq!((shared.name.as_str(), shared.kind.as_str(), shared.direct), ("Designgjengen", "group_message", false));
        assert!(threads.iter().find(|t| t.id == notes).unwrap().local);
        let profile_view = person_profile(&pool, &profile, &person.id).await.unwrap();
        assert_eq!(profile_view.threads, threads);
    }

    #[tokio::test]
    async fn local_details_survive_rematch_and_follow_a_merge() {
        use crate::models::object_override::{PutObjectOverrideInput, FIELD_COMPANY, FIELD_DISPLAY_NAME};
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let microsoft = test_provider(&pool, &profile, "microsoft").await;
        let slack = test_provider(&pool, &profile, "slack").await;
        let me = seed_user(&pool, &microsoft, "me-ms", "milo@northwind.example", "Milo Oakes").await;
        sqlx::query("UPDATE profile SET user_id = ?1 WHERE id = ?2").bind(&me).bind(&profile).execute(&pool).await.unwrap();
        let eva = seed_user(&pool, &microsoft, "eva-ms", "eva@northwind.example", "Eva Ortega").await;
        let alias = seed_user(&pool, &slack, "U9", "e.ortega@mail.example", "eo").await;
        let dm = dm_with(&pool, &microsoft, &me, &eva, "Eva Ortega").await;
        dm_with(&pool, &slack, &me, &alias, "eo").await;
        rematch(&pool, &profile).await.unwrap();
        let people = crate::models::Person::all(&pool, &profile).await.unwrap();
        let target = people.iter().find(|p| p.identities.iter().any(|i| i.external_id.as_deref() == Some("eva-ms"))).unwrap().id.clone();
        let source = people.iter().find(|p| p.identities.iter().any(|i| i.external_id.as_deref() == Some("U9"))).unwrap().id.clone();
        crate::models::Person::put(&pool, &crate::models::PutPersonInput { person_id: target.clone(), display_name: Some("Eva".into()), is_human: None, favourite_position: None }).await.unwrap();
        crate::models::ObjectOverride::put(&pool, &PutObjectOverrideInput { object_type: "person".into(), object_id: source.clone(), scope_type: None, scope_id: None, field: FIELD_COMPANY.into(), value: Some("Northwind".into()) }).await.unwrap();
        crate::models::ObjectOverride::put(&pool, &PutObjectOverrideInput { object_type: "person".into(), object_id: target.clone(), scope_type: Some("conversation".into()), scope_id: Some(dm.clone()), field: FIELD_DISPLAY_NAME.into(), value: Some("E".into()) }).await.unwrap();
        rematch(&pool, &profile).await.unwrap();
        let person = crate::models::Person::get(&pool, &profile, &target).await.unwrap();
        assert_eq!((person.display_name.as_str(), person.provider_name.as_str()), ("Eva", "Eva Ortega"));
        assert_eq!(person.nicknames.get(&dm).map(String::as_str), Some("E"));
        crate::models::Person::merge(&pool, &source, &target).await.unwrap();
        let merged = crate::models::Person::get(&pool, &profile, &target).await.unwrap();
        assert_eq!(merged.overrides.get(FIELD_COMPANY).map(String::as_str), Some("Northwind"));
        assert_eq!(merged.overrides.get(FIELD_DISPLAY_NAME).map(String::as_str), Some("Eva"));
        let stored: String = sqlx::query_scalar("SELECT display_name FROM person WHERE id = ?1").bind(&target).fetch_one(&pool).await.unwrap();
        assert_eq!(stored, "Eva Ortega", "the local name never replaces the matched one");
    }
}
