use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

use crate::models::inbox::Inbox;

pub const INBOX_PREFIX: &str = "inbox:";
pub const BUILT_IN: [&str; 17] = ["history", "inbox", "tags", "done", "snoozed", "notifications", "deleted", "pipeline", "groups", "cases", "actions", "content", "calendar", "statistics", "activity", "saved", "reminders"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SidebarEntry {
    Item { key: String },
    Divider { id: String },
    Heading { id: String, name: String },
    Folder {
        id: String,
        name: String,
        icon: Option<String>,
        #[serde(default)]
        collapsed: bool,
        #[serde(default)]
        items: Vec<SidebarEntry>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SidebarLayout {
    pub top: Vec<SidebarEntry>,
    pub footer: Vec<SidebarEntry>,
}

pub async fn schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sidebar_layout (
            profile_id TEXT PRIMARY KEY,
            layout TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profile(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

fn item(key: impl Into<String>) -> SidebarEntry {
    SidebarEntry::Item { key: key.into() }
}

fn divider() -> SidebarEntry {
    SidebarEntry::Divider { id: Uuid::new_v4().to_string() }
}

const PAGES: [&str; 6] = ["actions", "content", "calendar", "statistics", "cases", "history"];

fn known(key: &str) -> bool {
    BUILT_IN.contains(&key) || key.starts_with(INBOX_PREFIX)
}

impl SidebarLayout {
    pub fn default_for(inboxes: &[Inbox]) -> Self {
        let mut user: Vec<&Inbox> = inboxes.iter().filter(|inbox| !inbox.is_default).collect();
        user.sort_by_key(|inbox| inbox.position);
        let important = user.iter().find(|inbox| inbox.name.eq_ignore_ascii_case("important")).map(|inbox| inbox.id.clone());
        let mut top = vec![item("notifications")];
        if let Some(id) = &important {
            top.push(item(format!("{}{}", INBOX_PREFIX, id)));
        }
        top.push(divider());
        for inbox in user.iter().filter(|inbox| Some(&inbox.id) != important.as_ref()) {
            top.push(item(format!("{}{}", INBOX_PREFIX, inbox.id)));
        }
        for key in ["inbox", "activity", "saved", "reminders", "tags", "done", "snoozed", "deleted", "pipeline", "groups"] {
            top.push(item(key));
        }
        let footer = PAGES.into_iter().map(item).collect();
        Self { top, footer }
    }

    pub fn keys(&self) -> Vec<String> {
        fn walk(entries: &[SidebarEntry], out: &mut Vec<String>) {
            for entry in entries {
                match entry {
                    SidebarEntry::Item { key } => out.push(key.clone()),
                    SidebarEntry::Folder { items, .. } => walk(items, out),
                    SidebarEntry::Divider { .. } | SidebarEntry::Heading { .. } => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.top, &mut out);
        walk(&self.footer, &mut out);
        out
    }

    pub fn reconcile(mut self, inboxes: &[Inbox]) -> Self {
        let live: Vec<String> = inboxes.iter().filter(|inbox| !inbox.is_default).map(|inbox| format!("{}{}", INBOX_PREFIX, inbox.id)).collect();
        fn keep(entries: Vec<SidebarEntry>, live: &[String], seen: &mut Vec<String>) -> Vec<SidebarEntry> {
            entries
                .into_iter()
                .filter_map(|entry| match entry {
                    SidebarEntry::Item { key } => {
                        let alive = if key.starts_with(INBOX_PREFIX) { live.contains(&key) } else { known(&key) };
                        if !alive || seen.contains(&key) {
                            return None;
                        }
                        seen.push(key.clone());
                        Some(SidebarEntry::Item { key })
                    }
                    SidebarEntry::Folder { id, name, icon, collapsed, items } => Some(SidebarEntry::Folder { id, name, icon, collapsed, items: keep(items, live, seen) }),
                    divider => Some(divider),
                })
                .collect()
        }
        let mut seen = Vec::new();
        self.top = keep(self.top, &live, &mut seen);
        self.footer = keep(self.footer, &live, &mut seen);
        let missing: Vec<SidebarEntry> = live.iter().filter(|key| !seen.contains(key)).map(|key| item(key.clone())).collect();
        if !missing.is_empty() {
            let holds_inbox = |entry: &SidebarEntry| match entry {
                SidebarEntry::Item { key } => key.starts_with(INBOX_PREFIX),
                SidebarEntry::Folder { items, .. } => items.iter().any(|inner| matches!(inner, SidebarEntry::Item { key } if key.starts_with(INBOX_PREFIX))),
                SidebarEntry::Divider { .. } | SidebarEntry::Heading { .. } => false,
            };
            let at = self.top.iter().rposition(holds_inbox).map(|i| i + 1).unwrap_or(0);
            self.top.splice(at..at, missing);
        }
        for key in BUILT_IN {
            if !seen.iter().any(|seen| seen == key) {
                if PAGES.contains(&key) {
                    self.footer.push(item(key));
                } else {
                    self.top.push(item(key));
                }
            }
        }
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        fn check(entries: &[SidebarEntry], nested: bool) -> Result<(), String> {
            for entry in entries {
                if let SidebarEntry::Folder { name, items, .. } = entry {
                    if nested {
                        return Err("Folders cannot sit inside folders".to_string());
                    }
                    if name.trim().is_empty() {
                        return Err("A folder needs a name".to_string());
                    }
                    check(items, true)?;
                }
            }
            Ok(())
        }
        check(&self.top, false)?;
        check(&self.footer, false)
    }

    pub async fn get(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Self, String> {
        let inboxes = Inbox::all(pool, profile_id).await?;
        let stored: Option<String> = sqlx::query_scalar("SELECT layout FROM sidebar_layout WHERE profile_id = ?1")
            .bind(profile_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to read the sidebar layout: {}", e))?;
        match stored.and_then(|raw| serde_json::from_str::<SidebarLayout>(&raw).ok()) {
            Some(layout) => Ok(layout.reconcile(&inboxes)),
            None => {
                let layout = Self::default_for(&inboxes);
                Self::put(pool, profile_id, &layout).await?;
                Ok(layout)
            }
        }
    }

    pub async fn put(pool: &Pool<Sqlite>, profile_id: &str, layout: &SidebarLayout) -> Result<(), String> {
        layout.validate()?;
        let raw = serde_json::to_string(layout).map_err(|e| format!("Failed to encode the sidebar layout: {}", e))?;
        sqlx::query("INSERT INTO sidebar_layout (profile_id, layout, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(profile_id) DO UPDATE SET layout = excluded.layout, updated_at = excluded.updated_at")
            .bind(profile_id)
            .bind(raw)
            .bind(Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to save the sidebar layout: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::inbox::PostInboxInput;
    use crate::test_helpers::{test_pool, test_profile};

    async fn inbox(pool: &Pool<Sqlite>, profile: &str, name: &str) -> Inbox {
        Inbox::post(pool, profile, &PostInboxInput { name: name.into(), icon: None, members: None }).await.unwrap()
    }

    #[tokio::test]
    async fn the_default_puts_notifications_and_important_first_then_a_divider() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        Inbox::ensure_defaults(&pool, &profile).await.unwrap();
        let team = inbox(&pool, &profile, "Team").await;
        let important = inbox(&pool, &profile, "Important").await;
        let layout = SidebarLayout::get(&pool, &profile).await.unwrap();
        assert_eq!(layout.top[0], item("notifications"));
        assert_eq!(layout.top[1], item(format!("inbox:{}", important.id)));
        assert!(matches!(layout.top[2], SidebarEntry::Divider { .. }));
        assert_eq!(layout.top[3], item(format!("inbox:{}", team.id)));
        let footer: Vec<String> = layout.footer.iter().map(|e| match e { SidebarEntry::Item { key } => key.clone(), _ => String::new() }).collect();
        assert_eq!(footer, vec!["actions", "content", "calendar", "statistics", "cases", "history"]);
        let again = SidebarLayout::get(&pool, &profile).await.unwrap();
        assert_eq!(again.keys(), layout.keys());
    }

    #[tokio::test]
    async fn a_saved_layout_keeps_folders_and_dividers_and_picks_up_new_inboxes() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let work = inbox(&pool, &profile, "Work").await;
        let layout = SidebarLayout {
            top: vec![
                SidebarEntry::Folder { id: "f".into(), name: "Mail".into(), icon: Some("lucide:mail".into()), collapsed: true, items: vec![item(format!("inbox:{}", work.id)), divider(), item("done")] },
                item("ghost"),
                item("done"),
            ],
            footer: vec![item("calendar")],
        };
        SidebarLayout::put(&pool, &profile, &layout).await.unwrap();
        let private = inbox(&pool, &profile, "Private").await;
        let read = SidebarLayout::get(&pool, &profile).await.unwrap();
        let SidebarEntry::Folder { items, collapsed, .. } = &read.top[0] else { panic!("folder lost") };
        assert!(collapsed);
        assert_eq!(items.len(), 3);
        let keys = read.keys();
        assert!(!keys.contains(&"ghost".to_string()));
        assert_eq!(keys.iter().filter(|k| *k == "done").count(), 1);
        assert!(keys.contains(&format!("inbox:{}", private.id)));
        assert!(keys.contains(&"statistics".to_string()));
    }

    #[tokio::test]
    async fn folders_cannot_nest() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let nested = SidebarLayout {
            top: vec![SidebarEntry::Folder { id: "a".into(), name: "A".into(), icon: None, collapsed: false, items: vec![SidebarEntry::Folder { id: "b".into(), name: "B".into(), icon: None, collapsed: false, items: vec![] }] }],
            footer: vec![],
        };
        assert!(SidebarLayout::put(&pool, &profile, &nested).await.is_err());
    }
}
