use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite, Row};
use uuid::Uuid;
use tauri::{WebviewWindow, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub display_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub avatar_url: Option<String>,
    #[serde(skip_serializing)]
    pub avatar_data: Option<String>,
    pub job_title: Option<String>,
    pub department: Option<String>,
    pub company: Option<String>,
    pub office_location: Option<String>,
    #[serde(skip_serializing)]
    pub first_seen: String,
    #[serde(skip_serializing)]
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUser {
    pub id: String,
    pub user_id: String,
    pub provider_id: String,
    pub external_id: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub display_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub avatar_url: Option<String>,
    pub avatar_data: Option<String>,
    pub job_title: Option<String>,
    pub department: Option<String>,
    pub company: Option<String>,
    pub office_location: Option<String>,
    pub theme_color: Option<String>,
    pub details: Option<serde_json::Value>,
    pub first_seen: String,
    pub last_synced: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub user_id: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub avatar_data: Option<String>,
}

impl Account {
    pub async fn get(pool: &Pool<Sqlite>, provider_id: &str) -> Result<Option<Self>, String> {
        let row = sqlx::query(
            "SELECT u.id AS user_id,
                COALESCE(NULLIF(pu.display_name, ''), u.display_name) AS display_name,
                COALESCE(NULLIF(pu.email, ''), u.email) AS email,
                COALESCE(NULLIF(pu.avatar_url, ''), u.avatar_url) AS avatar_url,
                COALESCE(NULLIF(pu.avatar_data, ''), u.avatar_data) AS avatar_data
             FROM provider p
             JOIN provider_user pu ON pu.provider_id = p.id
             JOIN user u ON u.id = pu.user_id
             WHERE p.id = ?1 AND (pu.external_id = COALESCE(p.user_external_id, p.external_id)
                OR (p.user_external_id IS NULL AND pu.user_id = (SELECT user_id FROM profile WHERE id = p.profile_id)))
             ORDER BY pu.external_id = COALESCE(p.user_external_id, p.external_id) DESC
             LIMIT 1",
        )
        .bind(provider_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to load the account: {}", e))?;
        Ok(row.map(|row| Self {
            user_id: row.get("user_id"),
            display_name: row.try_get("display_name").ok().flatten(),
            email: row.try_get("email").ok().flatten(),
            avatar_url: row.try_get("avatar_url").ok().flatten(),
            avatar_data: row.try_get("avatar_data").ok().flatten(),
        }))
    }

    pub async fn get_conversation(pool: &Pool<Sqlite>, conversation_id: &str, provider_id: Option<&str>) -> Result<Option<Self>, String> {
        let found: Option<String> = sqlx::query_scalar(
            "SELECT provider_id FROM conversation_provider WHERE conversation_id = ?1 AND (?2 IS NULL OR provider_id = ?2) ORDER BY is_primary DESC, created_at ASC LIMIT 1",
        )
        .bind(conversation_id)
        .bind(provider_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to find where the conversation lives: {}", e))?;
        match found {
            Some(provider_id) => Self::get(pool, &provider_id).await,
            None => Ok(None),
        }
    }

    pub fn name(&self) -> Option<String> {
        [&self.display_name, &self.email].into_iter().flatten().map(|v| v.trim()).find(|v| !v.is_empty()).map(String::from)
    }
}

impl User {
    pub async fn find_by_id(
        pool: &Pool<Sqlite>,
        id: &str,
    ) -> Result<Option<Self>, String> {
        let row = sqlx::query("SELECT * FROM user WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to find user by id: {}", e))?;
        
        if let Some(row) = row {
            Ok(Some(Self::from_row(row)?))
        } else {
            Ok(None)
        }
    }
    
    pub async fn find_by_email_or_phone(
        pool: &Pool<Sqlite>,
        email: Option<&str>,
        phone: Option<&str>,
    ) -> Result<Option<Self>, String> {
        if email.is_none() && phone.is_none() {
            return Ok(None);
        }

        let mut query = String::from("SELECT * FROM user WHERE ");
        let mut conditions = Vec::new();
        
        if let Some(email) = email {
            conditions.push(format!("email = '{}'", email.replace("'", "''")));
        }
        
        if let Some(phone) = phone {
            conditions.push(format!("phone = '{}'", phone.replace("'", "''")));
        }
        
        query.push_str(&conditions.join(" OR "));
        
        if let Some(row) = sqlx::query(&query)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to query user: {}", e))?
        {
            Ok(Some(Self::from_row(row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn upsert_from_response(
        pool: &Pool<Sqlite>,
        person: &crate::providers::traits::ResponseUser,
    ) -> Result<String, String> {
        if let Some(existing) = Self::find_by_email_or_phone(pool, person.email.as_deref(), person.phone.as_deref()).await? {
            sqlx::query(
                r#"UPDATE user SET
                    display_name = CASE WHEN ?1 IS NULL THEN display_name WHEN display_name LIKE '% %' AND ?1 NOT LIKE '% %' THEN display_name ELSE ?1 END,
                    first_name = COALESCE(?2, first_name),
                    last_name = COALESCE(?3, last_name),
                    avatar_url = COALESCE(?4, avatar_url),
                    job_title = COALESCE(?5, job_title),
                    department = COALESCE(?6, department),
                    company = COALESCE(?7, company),
                    office_location = COALESCE(?8, office_location),
                    last_updated = ?9
                WHERE id = ?10"#,
            )
            .bind(&person.display_name)
            .bind(&person.first_name)
            .bind(&person.last_name)
            .bind(&person.avatar_url)
            .bind(&person.job_title)
            .bind(&person.department)
            .bind(&person.company)
            .bind(&person.office_location)
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(&existing.id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update user: {}", e))?;
            return Ok(existing.id);
        }
        let created = Self::create(
            pool,
            person.email.clone(),
            person.phone.clone(),
            person.display_name.clone(),
            person.first_name.clone(),
            person.last_name.clone(),
            person.avatar_url.clone(),
            None,
            person.job_title.clone(),
            person.department.clone(),
            person.company.clone(),
            person.office_location.clone(),
        )
        .await?;
        Ok(created.id)
    }

    pub async fn create(
        pool: &Pool<Sqlite>,
        email: Option<String>,
        phone: Option<String>,
        display_name: Option<String>,
        first_name: Option<String>,
        last_name: Option<String>,
        avatar_url: Option<String>,
        avatar_data: Option<String>,
        job_title: Option<String>,
        department: Option<String>,
        company: Option<String>,
        office_location: Option<String>,
    ) -> Result<Self, String> {
        let now = chrono::Utc::now().to_rfc3339();
        let user = Self {
            id: Uuid::new_v4().to_string(),
            email,
            phone,
            display_name,
            first_name,
            last_name,
            avatar_url,
            avatar_data,
            job_title,
            department,
            company,
            office_location,
            first_seen: now.clone(),
            last_updated: now.clone(),
        };
        
        user.save_to_db(pool).await?;
        Ok(user)
    }

    pub async fn update_with_new_data(
        &mut self,
        pool: &Pool<Sqlite>,
        email: Option<String>,
        phone: Option<String>,
        display_name: Option<String>,
        first_name: Option<String>,
        last_name: Option<String>,
        avatar_url: Option<String>,
        job_title: Option<String>,
        department: Option<String>,
        company: Option<String>,
        office_location: Option<String>,
    ) -> Result<(), String> {
        // Update fields only if new data is provided and current field is None
        if email.is_some() && self.email.is_none() {
            self.email = email;
        }
        if phone.is_some() && self.phone.is_none() {
            self.phone = phone;
        }
        if display_name.is_some() && self.display_name.is_none() {
            self.display_name = display_name;
        }
        if first_name.is_some() && self.first_name.is_none() {
            self.first_name = first_name;
        }
        if last_name.is_some() && self.last_name.is_none() {
            self.last_name = last_name;
        }
        if avatar_url.is_some() && self.avatar_url.is_none() {
            self.avatar_url = avatar_url;
        }
        if job_title.is_some() && self.job_title.is_none() {
            self.job_title = job_title;
        }
        if department.is_some() && self.department.is_none() {
            self.department = department;
        }
        if company.is_some() && self.company.is_none() {
            self.company = company;
        }
        if office_location.is_some() && self.office_location.is_none() {
            self.office_location = office_location;
        }
        
        self.last_updated = chrono::Utc::now().to_rfc3339();
        self.save_to_db(pool).await
    }

    pub async fn save_to_db(&self, pool: &Pool<Sqlite>) -> Result<(), String> {
        let is_new = sqlx::query("SELECT id FROM user WHERE id = ?1")
            .bind(&self.id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to check user existence: {}", e))?
            .is_none();

        let action = if is_new { "Inserting" } else { "Updating" };
        println!("👤 {} user: {} ({} - {})", 
            action, 
            self.id, 
            self.display_name.as_deref().unwrap_or("No name"), 
            self.email.as_deref().unwrap_or("No email")
        );

        sqlx::query(
            "INSERT OR REPLACE INTO user (
                id, email, phone, display_name, first_name, last_name,
                avatar_url, avatar_data, job_title, department, company,
                office_location, first_seen, last_updated
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
        )
        .bind(&self.id)
        .bind(&self.email)
        .bind(&self.phone)
        .bind(&self.display_name)
        .bind(&self.first_name)
        .bind(&self.last_name)
        .bind(&self.avatar_url)
        .bind(&self.avatar_data)
        .bind(&self.job_title)
        .bind(&self.department)
        .bind(&self.company)
        .bind(&self.office_location)
        .bind(&self.first_seen)
        .bind(&self.last_updated)
        .execute(pool)
        .await
        .map_err(|e| {
            println!("  ❌ Failed to save user {}: {}", self.id, e);
            format!("Failed to save user: {}", e)
        })?;
        
        println!("  ✅ User saved successfully: {}", self.id);
        Ok(())
    }

    pub async fn save_to_db_and_emit(&self, pool: &Pool<Sqlite>, window: Option<&WebviewWindow>) -> Result<(), String> {
        let is_new = sqlx::query("SELECT id FROM user WHERE id = ?1")
            .bind(&self.id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to check user existence: {}", e))?
            .is_none();

        self.save_to_db(pool).await?;

        if let Some(window) = window {
            let event_name = if is_new { "user-created" } else { "user-updated" };
            println!("  📤 Emitting {} event for user {}", event_name, self.id);
            if let Err(e) = window.emit(event_name, &self) {
                eprintln!("  ❌ Failed to emit {} event: {}", event_name, e);
            } else {
                println!("  ✅ {} event emitted successfully", event_name);
            }
        }
        
        Ok(())
    }

    pub async fn get_all_for_profile(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Self>, String> {
        let rows = sqlx::query(
            "SELECT DISTINCT u.* FROM user u
             INNER JOIN provider_user pu ON u.id = pu.user_id
             INNER JOIN provider p ON pu.provider_id = p.id
             WHERE p.profile_id = ?1
             ORDER BY u.last_updated DESC"
        )
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to get users for profile: {}", e))?;

        let mut users = Vec::new();
        for row in rows {
            users.push(Self::from_row(row)?);
        }
        Ok(users)
    }

    pub fn from_row(row: sqlx::sqlite::SqliteRow) -> Result<Self, String> {
        Ok(Self {
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
    }
}

impl ProviderUser {
    pub async fn create_or_update(
        pool: &Pool<Sqlite>,
        user_id: String,
        provider_id: String,
        external_id: String,
    ) -> Result<Self, String> {
        // Check if provider_user already exists
        if let Some(existing) = Self::find_by_external_id(pool, &provider_id, &external_id).await? {
            Ok(existing)
        } else {
            Self::create(pool, user_id, provider_id, external_id).await
        }
    }

    pub async fn find_by_external_id(
        pool: &Pool<Sqlite>,
        provider_id: &str,
        external_id: &str,
    ) -> Result<Option<Self>, String> {
        let row = sqlx::query(
            "SELECT * FROM provider_user WHERE provider_id = ?1 AND external_id = ?2"
        )
        .bind(provider_id)
        .bind(external_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to query provider_user: {}", e))?;

        if let Some(row) = row {
            Ok(Some(Self::from_row(row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn create(
        pool: &Pool<Sqlite>,
        user_id: String,
        provider_id: String,
        external_id: String,
    ) -> Result<Self, String> {
        let now = chrono::Utc::now().to_rfc3339();
        let provider_user = Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            provider_id,
            external_id,
            email: None,
            phone: None,
            display_name: None,
            first_name: None,
            last_name: None,
            avatar_url: None,
            avatar_data: None,
            job_title: None,
            department: None,
            company: None,
            office_location: None,
            theme_color: None,
            details: None,
            first_seen: now.clone(),
            last_synced: now.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        
        provider_user.save_to_db(pool).await?;
        Ok(provider_user)
    }

    pub async fn save_to_db(&self, pool: &Pool<Sqlite>) -> Result<(), String> {
        println!("🆔 Saving provider_user: {} for provider {} (user: {}, name: {})", 
            self.id, 
            self.provider_id, 
            &self.user_id,
            self.display_name.as_deref().unwrap_or("No name")
        );
        
        sqlx::query(
            "INSERT OR REPLACE INTO provider_user (
                id, user_id, provider_id, external_id,
                email, phone, display_name,
                first_name, last_name, avatar_url, avatar_data,
                job_title, department, company, office_location,
                theme_color, details, first_seen, last_synced,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)"
        )
        .bind(&self.id)
        .bind(&self.user_id)
        .bind(&self.provider_id)
        .bind(&self.external_id)
        .bind(&self.email)
        .bind(&self.phone)
        .bind(&self.display_name)
        .bind(&self.first_name)
        .bind(&self.last_name)
        .bind(&self.avatar_url)
        .bind(&self.avatar_data)
        .bind(&self.job_title)
        .bind(&self.department)
        .bind(&self.company)
        .bind(&self.office_location)
        .bind(&self.theme_color)
        .bind(&self.details.as_ref().map(|d| d.to_string()))
        .bind(&self.first_seen)
        .bind(&self.last_synced)
        .bind(&self.created_at)
        .bind(&self.updated_at)
        .execute(pool)
        .await
        .map_err(|e| {
            println!("  ❌ Failed to save provider_user {}: {}", self.id, e);
            format!("Failed to save provider_user: {}", e)
        })?;
        
        println!("  ✅ Provider_user saved successfully: {}", self.id);
        Ok(())
    }

    pub async fn save_to_db_and_emit(&self, pool: &Pool<Sqlite>, window: Option<&WebviewWindow>) -> Result<(), String> {
        self.save_to_db(pool).await?;

        if let Some(window) = window {
            println!("  📤 Emitting user_provider_updated event for provider_user {}", self.id);
            if let Err(e) = window.emit("user_provider_updated", &self) {
                eprintln!("  ❌ Failed to emit user-provider-updated event: {}", e);
            } else {
                println!("  ✅ Provider user event emitted successfully");
            }
        }
        
        Ok(())
    }

    fn from_row(row: sqlx::sqlite::SqliteRow) -> Result<Self, String> {
        Ok(Self {
            id: row.get("id"),
            user_id: row.get("user_id"),
            provider_id: row.get("provider_id"),
            external_id: row.get("external_id"),
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
            theme_color: row.try_get("theme_color").ok(),
            details: row.try_get::<Option<String>, _>("details").ok().flatten()
                .and_then(|s| serde_json::from_str(&s).ok()),
            first_seen: row.get("first_seen"),
            last_synced: row.get("last_synced"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    async fn own(pool: &Pool<Sqlite>, provider_id: &str, user_id: &str, external: &str, column: &str) {
        ProviderUser::create_or_update(pool, user_id.to_string(), provider_id.to_string(), external.to_string()).await.unwrap();
        sqlx::query(&format!("UPDATE provider SET {} = ?1 WHERE id = ?2", column)).bind(external).bind(provider_id).execute(pool).await.unwrap();
    }

    async fn link(pool: &Pool<Sqlite>, conversation_id: &str, provider_id: &str, primary: bool) {
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id, is_primary) VALUES (?1, ?2, ?3, ?1, ?4)")
            .bind(Uuid::new_v4().to_string())
            .bind(conversation_id)
            .bind(provider_id)
            .bind(primary)
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn each_conversation_speaks_as_the_account_it_goes_through() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let teams = test_provider(&pool, &profile, "microsoft").await;
        let slack = test_provider(&pool, &profile, "slack").await;
        let work = test_user(&pool, "milo@northwind.example", "Milo Oakes").await;
        let chat = test_user(&pool, "milo@acme.io", "milo.o").await;
        own(&pool, &teams, &work, "aad-milo", "user_external_id").await;
        own(&pool, &slack, &chat, "U123", "user_external_id").await;
        let colleague = test_user(&pool, "ola@northwind.example", "Ola").await;
        ProviderUser::create_or_update(&pool, colleague.clone(), teams.clone(), "aad-ola".to_string()).await.unwrap();

        let on_teams = Account::get(&pool, &teams).await.unwrap().unwrap();
        assert_eq!(on_teams.user_id, work);
        assert_eq!(on_teams.name().as_deref(), Some("Milo Oakes"));
        assert_eq!(Account::get(&pool, &slack).await.unwrap().unwrap().user_id, chat);

        let spanning = test_conversation(&pool, "Deploy", "group_message").await;
        link(&pool, &spanning, &slack, true).await;
        link(&pool, &spanning, &teams, false).await;
        assert_eq!(Account::get_conversation(&pool, &spanning, None).await.unwrap().unwrap().user_id, chat);
        assert_eq!(Account::get_conversation(&pool, &spanning, Some(&teams)).await.unwrap().unwrap().user_id, work);
        let local = test_conversation(&pool, "Notes", "direct_message").await;
        assert!(Account::get_conversation(&pool, &local, None).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn a_connection_from_before_accounts_falls_back_to_its_own_id_or_the_profile_user() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let mock = test_provider(&pool, &profile, "mock").await;
        let tenant = test_provider(&pool, &profile, "microsoft").await;
        let me = test_user(&pool, "milo@northwind.example", "Milo").await;
        own(&pool, &mock, &me, "mock-teams-me", "external_id").await;
        ProviderUser::create_or_update(&pool, me.clone(), tenant.clone(), "aad-milo".to_string()).await.unwrap();
        sqlx::query("UPDATE provider SET external_id = 'tenant-id' WHERE id = ?1").bind(&tenant).execute(&pool).await.unwrap();
        assert_eq!(Account::get(&pool, &mock).await.unwrap().unwrap().user_id, me);
        assert!(Account::get(&pool, &tenant).await.unwrap().is_none());
        sqlx::query("UPDATE profile SET user_id = ?1 WHERE id = ?2").bind(&me).bind(&profile).execute(&pool).await.unwrap();
        assert_eq!(Account::get(&pool, &tenant).await.unwrap().unwrap().user_id, me);
    }
}
