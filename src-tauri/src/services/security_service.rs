use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use sqlx::{Pool, Row, Sqlite};

pub const SECRET_MARKER: &str = "keychain";
pub const SERVICE: &str = "com.slyotis.chatnip";
pub const APP_VAULT: &str = "app";

pub fn profile_vault(profile_id: &str) -> String {
    format!("profile:{}", profile_id)
}

pub trait SecretStore: Send + Sync {
    fn get(&self, vault: &str, key: &str) -> Result<Option<String>, String>;
    fn put(&self, vault: &str, key: &str, value: &str) -> Result<(), String>;
    fn delete(&self, vault: &str, key: &str) -> Result<(), String>;
}

pub struct KeychainSecretStore {
    cache: Mutex<HashMap<String, BTreeMap<String, String>>>,
}

impl KeychainSecretStore {
    pub fn new() -> Self {
        Self { cache: Mutex::new(HashMap::new()) }
    }

    fn entry(vault: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, vault).map_err(|e| format!("The keychain could not be opened: {}", e))
    }

    fn load(&self, vault: &str) -> Result<BTreeMap<String, String>, String> {
        if let Some(found) = self.cache.lock().map_err(|_| "The keychain cache is poisoned".to_string())?.get(vault) {
            return Ok(found.clone());
        }
        let values = match Self::entry(vault)?.get_password() {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(keyring::Error::NoEntry) => BTreeMap::new(),
            Err(e) => return Err(format!("The keychain could not be read: {}", e)),
        };
        self.cache.lock().map_err(|_| "The keychain cache is poisoned".to_string())?.insert(vault.to_string(), values.clone());
        Ok(values)
    }

    fn save(&self, vault: &str, values: BTreeMap<String, String>) -> Result<(), String> {
        let entry = Self::entry(vault)?;
        if values.is_empty() {
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(e) => return Err(format!("The keychain could not be cleared: {}", e)),
            }
        } else {
            let raw = serde_json::to_string(&values).map_err(|e| format!("The secrets could not be encoded: {}", e))?;
            entry.set_password(&raw).map_err(|e| format!("The keychain could not be written: {}", e))?;
        }
        self.cache.lock().map_err(|_| "The keychain cache is poisoned".to_string())?.insert(vault.to_string(), values);
        Ok(())
    }
}

impl SecretStore for KeychainSecretStore {
    fn get(&self, vault: &str, key: &str) -> Result<Option<String>, String> {
        Ok(self.load(vault)?.get(key).cloned())
    }

    fn put(&self, vault: &str, key: &str, value: &str) -> Result<(), String> {
        let mut values = self.load(vault)?;
        if values.get(key).map(String::as_str) == Some(value) {
            return Ok(());
        }
        values.insert(key.to_string(), value.to_string());
        self.save(vault, values)
    }

    fn delete(&self, vault: &str, key: &str) -> Result<(), String> {
        let mut values = self.load(vault)?;
        if values.remove(key).is_none() {
            return Ok(());
        }
        self.save(vault, values)
    }
}

#[derive(Default)]
pub struct MemorySecretStore {
    values: Mutex<HashMap<(String, String), String>>,
}

impl SecretStore for MemorySecretStore {
    fn get(&self, vault: &str, key: &str) -> Result<Option<String>, String> {
        Ok(self.values.lock().map_err(|_| "poisoned".to_string())?.get(&(vault.to_string(), key.to_string())).cloned())
    }

    fn put(&self, vault: &str, key: &str, value: &str) -> Result<(), String> {
        self.values.lock().map_err(|_| "poisoned".to_string())?.insert((vault.to_string(), key.to_string()), value.to_string());
        Ok(())
    }

    fn delete(&self, vault: &str, key: &str) -> Result<(), String> {
        self.values.lock().map_err(|_| "poisoned".to_string())?.remove(&(vault.to_string(), key.to_string()));
        Ok(())
    }
}

pub struct FileSecretStore {
    path: std::path::PathBuf,
    keychain: KeychainSecretStore,
    unreadable: Mutex<HashSet<String>>,
    lock: Mutex<()>,
}

impl FileSecretStore {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path, keychain: KeychainSecretStore::new(), unreadable: Mutex::new(HashSet::new()), lock: Mutex::new(()) }
    }

    fn read(&self) -> BTreeMap<String, BTreeMap<String, String>> {
        std::fs::read_to_string(&self.path).ok().and_then(|raw| serde_json::from_str(&raw).ok()).unwrap_or_default()
    }

    fn write(&self, values: &BTreeMap<String, BTreeMap<String, String>>) -> Result<(), String> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("The secrets folder could not be created: {}", e))?;
        }
        let raw = serde_json::to_string_pretty(values).map_err(|e| format!("The secrets could not be encoded: {}", e))?;
        let fresh = self.path.with_extension(format!("{}.part", std::process::id()));
        let _ = std::fs::remove_file(&fresh);
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&fresh).map_err(|e| format!("The secrets file could not be opened: {}", e))?;
        file.write_all(raw.as_bytes()).map_err(|e| format!("The secrets file could not be written: {}", e))?;
        std::fs::rename(&fresh, &self.path).map_err(|e| format!("The secrets file could not be saved: {}", e))
    }

    fn vault(&self, all: &mut BTreeMap<String, BTreeMap<String, String>>, vault: &str) -> bool {
        if all.contains_key(vault) {
            return false;
        }
        if self.unreadable.lock().map(|skipped| skipped.contains(vault)).unwrap_or(true) {
            return false;
        }
        let imported = if cfg!(test) { Ok(BTreeMap::new()) } else { self.keychain.load(vault) };
        match imported {
            Ok(values) => {
                all.insert(vault.to_string(), values);
                true
            }
            Err(_) => {
                if let Ok(mut skipped) = self.unreadable.lock() {
                    skipped.insert(vault.to_string());
                }
                false
            }
        }
    }
}

impl SecretStore for FileSecretStore {
    fn get(&self, vault: &str, key: &str) -> Result<Option<String>, String> {
        let _guard = self.lock.lock().map_err(|_| "The secrets file lock is poisoned".to_string())?;
        let mut all = self.read();
        if self.vault(&mut all, vault) {
            self.write(&all)?;
        }
        Ok(all.get(vault).and_then(|values| values.get(key)).cloned())
    }

    fn put(&self, vault: &str, key: &str, value: &str) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|_| "The secrets file lock is poisoned".to_string())?;
        let mut all = self.read();
        self.vault(&mut all, vault);
        all.entry(vault.to_string()).or_default().insert(key.to_string(), value.to_string());
        self.write(&all)
    }

    fn delete(&self, vault: &str, key: &str) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|_| "The secrets file lock is poisoned".to_string())?;
        let mut all = self.read();
        self.vault(&mut all, vault);
        if let Some(values) = all.get_mut(vault) {
            values.remove(key);
        }
        self.write(&all)
    }
}

pub fn dev_secrets_path() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Library/Application Support").join(SERVICE).join("secrets.dev.json")
}

pub fn secrets() -> &'static dyn SecretStore {
    static STORE: OnceLock<Box<dyn SecretStore>> = OnceLock::new();
    STORE
        .get_or_init(|| {
            let choice = std::env::var("CHATNIP_SECRETS").unwrap_or_default();
            if cfg!(test) || choice == "memory" {
                Box::new(MemorySecretStore::default())
            } else if choice == "file" || (cfg!(debug_assertions) && choice != "keychain") {
                Box::new(FileSecretStore::new(dev_secrets_path()))
            } else {
                Box::new(KeychainSecretStore::new())
            }
        })
        .as_ref()
}

pub fn reveal(vault: &str, key: &str, stored: Option<String>) -> Option<String> {
    match stored {
        Some(value) if value == SECRET_MARKER => secrets().get(vault, key).ok().flatten(),
        other => other,
    }
}

pub fn seal(vault: &str, key: &str, value: Option<&str>) -> Result<Option<String>, String> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(value) if value == SECRET_MARKER => Ok(Some(SECRET_MARKER.to_string())),
        Some(value) => {
            secrets().put(vault, key, value)?;
            Ok(Some(SECRET_MARKER.to_string()))
        }
        None => {
            secrets().delete(vault, key)?;
            Ok(None)
        }
    }
}

pub async fn migrate_secrets(pool: &Pool<Sqlite>) -> Result<usize, String> {
    let mut moved = 0;
    let rows = sqlx::query("SELECT id, profile_id, access_token, refresh_token FROM provider WHERE (access_token IS NOT NULL AND access_token != ?1) OR (refresh_token IS NOT NULL AND refresh_token != ?1)")
        .bind(SECRET_MARKER)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to read connections for the keychain move: {}", e))?;
    for row in rows {
        let id: String = row.get("id");
        let profile_id: String = row.get("profile_id");
        let auth = crate::providers::models::ProviderAuth::from_columns(&profile_id, &id, row.get("access_token"), row.get("refresh_token"), chrono::Utc::now(), "Bearer".into(), None);
        if let Some(auth) = auth {
            let (access, refresh) = auth.store(&profile_id, &id)?;
            sqlx::query("UPDATE provider SET access_token = ?1, refresh_token = ?2 WHERE id = ?3")
                .bind(access)
                .bind(refresh)
                .bind(&id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to mark the connection as moved: {}", e))?;
            moved += 1;
        }
    }
    moved += crate::models::app_setting::AppSetting::seal_existing(pool).await?;
    moved += crate::models::intelligence::IntelligenceConnection::seal_existing(pool).await?;
    Ok(moved)
}

static LOCKED: AtomicBool = AtomicBool::new(false);

pub const UNLOCKED_COMMANDS: [&str; 5] = ["get_lock_state", "delete_lock", "post_lock", "put_window_frame", "put_window_position"];

pub fn is_locked() -> bool {
    LOCKED.load(Ordering::SeqCst)
}

pub fn set_locked(locked: bool) {
    LOCKED.store(locked, Ordering::SeqCst);
}

pub fn allows(command: &str) -> bool {
    !is_locked() || UNLOCKED_COMMANDS.contains(&command)
}

#[cfg(target_os = "macos")]
pub async fn authenticate(reason: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        use objc2_foundation::{NSError, NSString};
        use objc2_local_authentication::{LAContext, LAPolicy};
        let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let context = unsafe { LAContext::new() };
        let policy = LAPolicy::DeviceOwnerAuthentication;
        if let Err(error) = unsafe { context.canEvaluatePolicy_error(policy) } {
            return Err(format!("This Mac cannot confirm it is you: {}", error.localizedDescription()));
        }
        let reason = NSString::from_str(&reason);
        let reply = block2::RcBlock::new(move |success: objc2::runtime::Bool, error: *mut NSError| {
            let result = if success.as_bool() {
                Ok(())
            } else {
                Err(unsafe { error.as_ref() }.map(|e| e.localizedDescription().to_string()).unwrap_or_else(|| "Not confirmed".to_string()))
            };
            let _ = tx.send(result);
        });
        unsafe { context.evaluatePolicy_localizedReason_reply(policy, &reason, &reply) };
        rx.recv().map_err(|_| "The confirmation was interrupted".to_string())?
    })
    .await
    .map_err(|e| format!("The confirmation could not run: {}", e))?
}

#[cfg(not(target_os = "macos"))]
pub async fn authenticate(_reason: String) -> Result<(), String> {
    Err("Device confirmation is only available on macOS".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dev_file_vault_keeps_secrets_private_and_per_vault() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("chatnip-secrets-{}.json", uuid::Uuid::new_v4()));
        let store = FileSecretStore::new(path.clone());
        store.put("profile:a", "token", "one").unwrap();
        store.put("profile:b", "token", "two").unwrap();
        assert_eq!(store.get("profile:a", "token").unwrap().as_deref(), Some("one"));
        assert_eq!(store.get("profile:b", "token").unwrap().as_deref(), Some("two"));
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        store.delete("profile:a", "token").unwrap();
        assert_eq!(FileSecretStore::new(path.clone()).get("profile:a", "token").unwrap(), None);
        std::fs::remove_file(path).unwrap();
    }

    async fn owned_conversation(pool: &sqlx::Pool<sqlx::Sqlite>, profile_id: &str, name: &str) -> (String, String) {
        let provider_id = crate::test_helpers::test_provider(pool, profile_id, "mock").await;
        let conversation_id = crate::test_helpers::test_conversation(pool, name, "direct_message").await;
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id) VALUES (?1, ?2, ?3, ?4)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&conversation_id)
            .bind(&provider_id)
            .bind(format!("ext-{}", name))
            .execute(pool)
            .await
            .unwrap();
        (provider_id, conversation_id)
    }

    #[tokio::test]
    async fn two_profiles_never_see_each_others_conversations_or_tokens() {
        use crate::models::Conversation;
        use crate::providers::models::ProviderAuth;
        let pool = crate::test_helpers::test_pool().await;
        let first = crate::test_helpers::test_profile(&pool).await;
        let second = crate::test_helpers::test_profile(&pool).await;
        let (first_provider, first_conversation) = owned_conversation(&pool, &first, "first").await;
        let (_, second_conversation) = owned_conversation(&pool, &second, "second").await;

        let ids = |rows: Vec<sqlx::sqlite::SqliteRow>| rows.into_iter().map(|r| r.get::<String, _>("id")).collect::<Vec<_>>();
        assert_eq!(ids(Conversation::get_rows_for_profile(&pool, &first).await.unwrap()), vec![first_conversation.clone()]);
        assert_eq!(ids(Conversation::get_rows_for_profile(&pool, &second).await.unwrap()), vec![second_conversation.clone()]);
        assert!(Conversation::is_owned_by(&pool, &first_conversation, &first).await.unwrap());
        assert!(!Conversation::is_owned_by(&pool, &first_conversation, &second).await.unwrap());

        let auth = ProviderAuth { access_token: "first-token".into(), refresh_token: Some("first-refresh".into()), expires_at: chrono::Utc::now(), token_type: "Bearer".into(), scope: None };
        let (access, refresh) = auth.store(&first, &first_provider).unwrap();
        assert_eq!(access, SECRET_MARKER);
        assert_eq!(refresh.as_deref(), Some(SECRET_MARKER));
        let mine = ProviderAuth::from_columns(&first, &first_provider, Some(access.clone()), refresh.clone(), chrono::Utc::now(), "Bearer".into(), None).unwrap();
        assert_eq!(mine.access_token, "first-token");
        assert!(ProviderAuth::from_columns(&second, &first_provider, Some(access), refresh, chrono::Utc::now(), "Bearer".into(), None).is_none());
        ProviderAuth::delete(&first, &first_provider).unwrap();
        assert_eq!(secrets().get(&profile_vault(&first), &format!("provider:{}:access", first_provider)).unwrap(), None);
    }

    #[tokio::test]
    async fn plaintext_secrets_move_into_the_store_on_startup() {
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        let provider = crate::test_helpers::test_provider(&pool, &profile, "slack").await;
        sqlx::query("UPDATE provider SET access_token = 'xoxp-plain', refresh_token = NULL, expires_at = ?2 WHERE id = ?1").bind(&provider).bind(chrono::Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO app_setting (key, value, updated_at) VALUES ('microsoft.client_secret', 'plain-secret', '')").execute(&pool).await.unwrap();
        assert!(migrate_secrets(&pool).await.unwrap() >= 2);
        let stored: String = sqlx::query_scalar("SELECT access_token FROM provider WHERE id = ?1").bind(&provider).fetch_one(&pool).await.unwrap();
        assert_eq!(stored, SECRET_MARKER);
        let loaded = crate::providers::models::Provider::load_from_db(&pool, &provider).await.unwrap();
        assert_eq!(loaded.auth.unwrap().access_token, "xoxp-plain");
        let raw: String = sqlx::query_scalar("SELECT value FROM app_setting WHERE key = 'microsoft.client_secret'").fetch_one(&pool).await.unwrap();
        assert_eq!(raw, SECRET_MARKER);
        assert_eq!(crate::models::AppSetting::get(&pool, "microsoft.client_secret").await.unwrap().as_deref(), Some("plain-secret"));
        assert_eq!(crate::models::AppSetting::all(&pool).await.unwrap().get("microsoft.client_secret").map(String::as_str), Some(SECRET_MARKER));
    }

    #[test]
    fn seal_and_reveal_round_trip_through_the_store() {
        let vault = profile_vault("p-seal");
        assert_eq!(seal(&vault, "k", Some(" s3cret ")).unwrap().as_deref(), Some(SECRET_MARKER));
        assert_eq!(reveal(&vault, "k", Some(SECRET_MARKER.to_string())).as_deref(), Some("s3cret"));
        assert_eq!(reveal(&vault, "k", Some("legacy".to_string())).as_deref(), Some("legacy"));
        assert_eq!(seal(&vault, "k", Some(SECRET_MARKER)).unwrap().as_deref(), Some(SECRET_MARKER));
        assert_eq!(reveal(&vault, "k", Some(SECRET_MARKER.to_string())).as_deref(), Some("s3cret"));
        assert_eq!(seal(&vault, "k", Some("")).unwrap(), None);
        assert_eq!(secrets().get(&vault, "k").unwrap(), None);
    }

    #[test]
    fn vaults_keep_profiles_apart() {
        secrets().put(&profile_vault("a"), "provider:x:access", "token-a").unwrap();
        assert_eq!(secrets().get(&profile_vault("b"), "provider:x:access").unwrap(), None);
    }

    #[test]
    fn the_lock_only_lets_unlock_commands_through() {
        set_locked(true);
        assert!(!allows("get_conversations"));
        assert!(allows("delete_lock"));
        set_locked(false);
        assert!(allows("get_conversations"));
    }
}
