use crate::models::group::{Group, GroupQuery, GroupSort, SortDirection, KIND_QUERY, KIND_TAG};
use regex::Regex;
use sqlx::{Pool, Row, Sqlite};

pub struct Built {
    pub sql: String,
    pub binds: Vec<String>,
}

pub fn build(query: &GroupQuery, profile_id: &str) -> Built {
    let mut clauses: Vec<String> = vec!["c.deleted_at IS NULL".to_string()];
    let mut binds: Vec<String> = Vec::new();
    let mut joins: Vec<String> = Vec::new();
    let bind = |value: &str, binds: &mut Vec<String>| -> String {
        binds.push(value.to_string());
        format!("?{}", binds.len() + 1)
    };
    if !query.providers.is_empty() {
        let marks: Vec<String> = query.providers.iter().map(|p| bind(p, &mut binds)).collect();
        clauses.push(format!(
            "EXISTS (SELECT 1 FROM conversation_provider cp JOIN provider p ON p.id = cp.provider_id WHERE cp.conversation_id = c.id AND p.provider_type IN ({0}))",
            marks.join(", ")
        ));
    }
    if !query.connection_ids.is_empty() {
        let marks: Vec<String> = query.connection_ids.iter().map(|p| bind(p, &mut binds)).collect();
        clauses.push(format!("EXISTS (SELECT 1 FROM conversation_provider cp WHERE cp.conversation_id = c.id AND cp.provider_id IN ({}))", marks.join(", ")));
    }
    if !query.types.is_empty() {
        let marks: Vec<String> = query.types.iter().map(|t| bind(t, &mut binds)).collect();
        clauses.push(format!("c.type IN ({})", marks.join(", ")));
    }
    if !query.tag_ids.is_empty() {
        let marks: Vec<String> = query.tag_ids.iter().map(|l| bind(l, &mut binds)).collect();
        let list = marks.join(", ");
        clauses.push(crate::models::tag::get_conversation_sql(&list));
    }
    if !query.person_ids.is_empty() {
        let marks: Vec<String> = query.person_ids.iter().map(|p| bind(p, &mut binds)).collect();
        clauses.push(format!("c.person_id IN ({})", marks.join(", ")));
    }
    if let Some(unread) = query.unread {
        clauses.push(if unread { "c.unread_count > 0".to_string() } else { "c.unread_count = 0".to_string() });
    }
    if let Some(has) = query.has_attachments {
        let exists = "EXISTS (SELECT 1 FROM message m WHERE m.conversation_id = c.id AND m.has_attachments = 1)";
        clauses.push(if has { exists.to_string() } else { format!("NOT {}", exists) });
    }
    if query.humans_only == Some(true) {
        clauses.push("EXISTS (SELECT 1 FROM person pe WHERE pe.id = c.person_id AND pe.is_human = 1)".to_string());
    }
    if let Some(from) = query.from.as_deref().filter(|s| !s.trim().is_empty()) {
        let mark = bind(from, &mut binds);
        clauses.push(format!("c.last_activity >= {}", mark));
    }
    if let Some(to) = query.to.as_deref().filter(|s| !s.trim().is_empty()) {
        let mark = bind(to, &mut binds);
        clauses.push(format!("c.last_activity <= {}", mark));
    }
    if let Some(sender) = query.sender.as_deref().filter(|s| !s.trim().is_empty()) {
        let mark = bind(&format!("%{}%", sender.trim()), &mut binds);
        clauses.push(format!(
            "(EXISTS (SELECT 1 FROM message m WHERE m.conversation_id = c.id AND (m.sender_email LIKE {0} OR m.sender_name LIKE {0})) OR EXISTS (SELECT 1 FROM conversation_member cm JOIN user u ON u.id = cm.user_id WHERE cm.conversation_id = c.id AND (u.email LIKE {0} OR u.display_name LIKE {0})))",
            mark
        ));
    }
    if let Some(text) = query.text.as_deref().filter(|s| !s.trim().is_empty()) {
        let mark = bind(&format!("%{}%", text.trim()), &mut binds);
        clauses.push(format!("(c.name LIKE {0} OR c.summary LIKE {0})", mark));
    }
    let order = match query.sort {
        GroupSort::Activity => "c.last_activity".to_string(),
        GroupSort::Name => "c.name COLLATE NOCASE".to_string(),
        GroupSort::Unread => "c.unread_count, c.last_activity".to_string(),
        GroupSort::Messages => {
            joins.push("LEFT JOIN (SELECT conversation_id, COUNT(*) AS n FROM message WHERE deleted_datetime IS NULL GROUP BY conversation_id) mc ON mc.conversation_id = c.id".to_string());
            "COALESCE(mc.n, 0), c.last_activity".to_string()
        }
        GroupSort::Tags => {
            joins.push("LEFT JOIN (SELECT cl.conversation_id, GROUP_CONCAT(l.name, ' ') AS tags FROM (SELECT object_id AS conversation_id, tag_id FROM object_tag WHERE object_type = 'conversation') cl JOIN tag l ON l.id = cl.tag_id GROUP BY cl.conversation_id) tg ON tg.conversation_id = c.id".to_string());
            "COALESCE(tg.tags, '') COLLATE NOCASE, c.last_activity".to_string()
        }
        GroupSort::Created => "c.created_at".to_string(),
    };
    let direction = match query.direction {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };
    let order = order
        .split(", ")
        .map(|column| format!("{} {}", column, direction))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT c.id, c.name, c.summary, (SELECT content FROM message m WHERE m.conversation_id = c.id ORDER BY timestamp DESC LIMIT 1) AS last_content FROM conversation c {} WHERE EXISTS (SELECT 1 FROM conversation_provider cp JOIN provider p ON p.id = cp.provider_id WHERE cp.conversation_id = c.id AND p.profile_id = ?1) AND {} ORDER BY {}",
        joins.join(" "),
        clauses.join(" AND "),
        order
    );
    let _ = profile_id;
    Built { sql, binds }
}

pub async fn evaluate(pool: &Pool<Sqlite>, profile_id: &str, query: &GroupQuery) -> Result<Vec<String>, String> {
    let built = build(query, profile_id);
    let mut statement = sqlx::query(&built.sql).bind(profile_id);
    for value in &built.binds {
        statement = statement.bind(value);
    }
    let rows = statement.fetch_all(pool).await.map_err(|e| format!("Failed to run the group query: {}", e))?;
    let regex = match query.regex.as_deref().filter(|r| !r.trim().is_empty()) {
        Some(pattern) => Some(Regex::new(&format!("(?i){}", pattern)).map_err(|e| format!("The regex is not valid: {}", e))?),
        None => None,
    };
    let mut ids: Vec<String> = Vec::new();
    for row in rows {
        if let Some(regex) = &regex {
            let haystack = format!(
                "{}\n{}\n{}",
                row.get::<String, _>("name"),
                row.try_get::<Option<String>, _>("summary").ok().flatten().unwrap_or_default(),
                row.try_get::<Option<String>, _>("last_content").ok().flatten().unwrap_or_default()
            );
            if !regex.is_match(&haystack) {
                continue;
            }
        }
        ids.push(row.get("id"));
        if let Some(limit) = query.limit {
            if limit > 0 && ids.len() as i64 >= limit {
                break;
            }
        }
    }
    Ok(ids)
}

pub async fn resolve(pool: &Pool<Sqlite>, profile_id: &str, group: &mut Group) -> Result<(), String> {
    if group.kind == KIND_TAG {
        if let Some(tag_id) = &group.tag_id {
            group.conversation_ids = crate::models::ObjectTag::get_conversation_ids(pool, tag_id).await?;
        }
    } else if group.kind == KIND_QUERY {
        if let Some(query) = &group.query {
            group.conversation_ids = evaluate(pool, profile_id, query).await?;
        }
    }
    Ok(())
}

pub async fn all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Group>, String> {
    Group::migrate_sections(pool, profile_id).await?;
    let mut groups = Group::all(pool, profile_id).await?;
    for group in &mut groups {
        resolve(pool, profile_id, group).await?;
    }
    Ok(groups)
}

pub fn read_ids(groups: &[Group], marked: impl IntoIterator<Item = String>) -> std::collections::BTreeSet<String> {
    let by_id: std::collections::HashMap<&str, &Group> = groups.iter().map(|g| (g.id.as_str(), g)).collect();
    let reads = |group: &Group| {
        let mut current = Some(group);
        let mut hops = 0;
        while let Some(g) = current {
            if g.mode == crate::models::chat::ChatMode::Read {
                return true;
            }
            hops += 1;
            if hops > groups.len() {
                return false;
            }
            current = g.parent_id.as_deref().and_then(|p| by_id.get(p).copied());
        }
        false
    };
    let mut ids: std::collections::BTreeSet<String> = marked.into_iter().collect();
    for group in groups.iter().filter(|g| reads(g)) {
        ids.extend(group.conversation_ids.iter().cloned());
    }
    ids
}

pub async fn read_conversation_ids(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<String>, String> {
    let marked: Vec<String> = sqlx::query_scalar(&format!("SELECT c.id FROM conversation c WHERE c.mode = 'read' AND {}", crate::models::OWNED_BY_PROFILE))
        .bind(profile_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to read the read chats: {}", e))?;
    let groups = all(pool, profile_id).await?;
    Ok(read_ids(&groups, marked).into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::group::{PostGroupInput, PutGroupInput};
    use crate::models::Tag;
    use crate::test_helpers::*;
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    #[tokio::test]
    async fn read_mode_flows_from_chats_and_from_groups_to_their_nested_groups() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = crate::test_helpers::test_provider(&pool, &profile, "mock").await;
        let mut chats = Vec::new();
        for name in ["marked", "in parent", "in child", "free"] {
            let id = crate::test_helpers::test_conversation(&pool, name, "channel").await;
            sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id) VALUES (?1, ?2, ?3, ?1)")
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(&id)
                .bind(&provider)
                .execute(&pool)
                .await
                .unwrap();
            chats.push(id);
        }
        sqlx::query("UPDATE conversation SET mode = 'read' WHERE id = ?1").bind(&chats[0]).execute(&pool).await.unwrap();
        let parent = Group::post(&pool, &profile, &PostGroupInput { name: "Reading".into(), kind: None, parent_id: None, tag_id: None, query: None, icon: None, color: None }).await.unwrap();
        let child = Group::post(&pool, &profile, &PostGroupInput { name: "Vendors".into(), kind: None, parent_id: Some(parent.id.clone()), tag_id: None, query: None, icon: None, color: None }).await.unwrap();
        Group::add_items(&pool, &parent.id, &[chats[1].clone()]).await.unwrap();
        Group::add_items(&pool, &child.id, &[chats[2].clone()]).await.unwrap();

        let before = read_conversation_ids(&pool, &profile).await.unwrap();
        assert_eq!(before, vec![chats[0].clone()]);

        Group::put(&pool, &PutGroupInput { group_id: parent.id.clone(), name: None, parent_id: None, tag_id: None, query: None, position: None, is_collapsed: None, icon: None, color: None, mode: Some(crate::models::chat::ChatMode::Read) }).await.unwrap();
        let mut after = read_conversation_ids(&pool, &profile).await.unwrap();
        after.sort();
        let mut expected = vec![chats[0].clone(), chats[1].clone(), chats[2].clone()];
        expected.sort();
        assert_eq!(after, expected);
        assert!(!after.contains(&chats[3]));
    }

    async fn attach(pool: &Pool<Sqlite>, conversation: &str, provider: &str, external: &str) {
        sqlx::query("INSERT INTO conversation_provider (id, conversation_id, provider_id, external_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)")
            .bind(Uuid::new_v4().to_string())
            .bind(conversation)
            .bind(provider)
            .bind(external)
            .bind(Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .unwrap();
    }

    async fn seed(pool: &Pool<Sqlite>) -> (String, String, String, Vec<String>) {
        let profile = test_profile(pool).await;
        let microsoft = test_provider(pool, &profile, "microsoft").await;
        let slack = test_provider(pool, &profile, "slack").await;
        let a = test_conversation(pool, "Alpha design", "direct_message").await;
        let b = test_conversation(pool, "Beta invoices", "email").await;
        let c = test_conversation(pool, "Gamma channel", "channel").await;
        attach(pool, &a, &microsoft, "a").await;
        attach(pool, &b, &microsoft, "b").await;
        attach(pool, &c, &slack, "c").await;
        for (id, days, unread) in [(&a, 1, 2), (&b, 5, 0), (&c, 10, 1)] {
            sqlx::query("UPDATE conversation SET last_activity = ?1, unread_count = ?2 WHERE id = ?3")
                .bind((Utc::now() - Duration::days(days)).to_rfc3339())
                .bind(unread)
                .bind(id)
                .execute(pool)
                .await
                .unwrap();
        }
        test_message(pool, &a, &microsoft, "Invoice 2024-09 attached").await;
        test_message(pool, &b, &microsoft, "Plain words").await;
        (profile, microsoft, slack, vec![a, b, c])
    }

    #[tokio::test]
    async fn query_filters_sort_and_limit() {
        let pool = test_pool().await;
        let (profile, microsoft, _slack, ids) = seed(&pool).await;
        let all = evaluate(&pool, &profile, &GroupQuery::default()).await.unwrap();
        assert_eq!(all, vec![ids[0].clone(), ids[1].clone(), ids[2].clone()]);
        let by_name = evaluate(&pool, &profile, &GroupQuery { sort: GroupSort::Name, direction: SortDirection::Asc, ..Default::default() }).await.unwrap();
        assert_eq!(by_name[0], ids[0]);
        assert_eq!(by_name[2], ids[2]);
        let mail = evaluate(&pool, &profile, &GroupQuery { types: vec!["email".to_string()], ..Default::default() }).await.unwrap();
        assert_eq!(mail, vec![ids[1].clone()]);
        let provider = evaluate(&pool, &profile, &GroupQuery { connection_ids: vec![microsoft.clone()], ..Default::default() }).await.unwrap();
        assert_eq!(provider.len(), 2);
        let unread = evaluate(&pool, &profile, &GroupQuery { unread: Some(true), ..Default::default() }).await.unwrap();
        assert_eq!(unread.len(), 2);
        let limited = evaluate(&pool, &profile, &GroupQuery { limit: Some(1), ..Default::default() }).await.unwrap();
        assert_eq!(limited, vec![ids[0].clone()]);
        let text = evaluate(&pool, &profile, &GroupQuery { text: Some("beta".to_string()), ..Default::default() }).await.unwrap();
        assert_eq!(text, vec![ids[1].clone()]);
        let regex = evaluate(&pool, &profile, &GroupQuery { regex: Some(r"invoice \d{4}".to_string()), ..Default::default() }).await.unwrap();
        assert_eq!(regex, vec![ids[0].clone()]);
        let recent = evaluate(&pool, &profile, &GroupQuery { from: Some((Utc::now() - Duration::days(3)).to_rfc3339()), ..Default::default() }).await.unwrap();
        assert_eq!(recent, vec![ids[0].clone()]);
        assert!(evaluate(&pool, &profile, &GroupQuery { regex: Some("(".to_string()), ..Default::default() }).await.is_err());
    }

    #[tokio::test]
    async fn groups_nest_resolve_and_reject_cycles() {
        let pool = test_pool().await;
        let (profile, _microsoft, _slack, ids) = seed(&pool).await;
        let root = Group::post(&pool, &profile, &PostGroupInput { name: "Clients".to_string(), kind: None, parent_id: None, tag_id: None, query: None, icon: None, color: None }).await.unwrap();
        let child = Group::post(&pool, &profile, &PostGroupInput { name: "Tidewater".to_string(), kind: None, parent_id: Some(root.id.clone()), tag_id: None, query: None, icon: None, color: None }).await.unwrap();
        assert_eq!(child.parent_id.as_deref(), Some(root.id.as_str()));
        let moved = Group::put(&pool, &PutGroupInput { group_id: root.id.clone(), name: None, parent_id: Some(Some(child.id.clone())), tag_id: None, query: None, position: None, is_collapsed: None, icon: None, color: None, mode: None }).await;
        assert!(moved.is_err());
        Group::add_items(&pool, &child.id, &[ids[0].clone(), ids[1].clone()]).await.unwrap();
        let label = Tag::post(&pool, &profile, "Urgent", "#d14a33", "tag", 0, "").await.unwrap();
        crate::models::ObjectTag::put(&pool, crate::models::tag::OBJECT_CONVERSATION, &ids[2], &label.id, "manual", None, None).await.unwrap();
        let tag = Group::post(&pool, &profile, &PostGroupInput { name: "Urgent".to_string(), kind: Some("tag".to_string()), parent_id: None, tag_id: Some(label.id.clone()), query: None, icon: None, color: None }).await.unwrap();
        let query = Group::post(&pool, &profile, &PostGroupInput { name: "Unread".to_string(), kind: Some("query".to_string()), parent_id: None, tag_id: None, query: Some(GroupQuery { unread: Some(true), ..Default::default() }), icon: None, color: None }).await.unwrap();
        let groups = all(&pool, &profile).await.unwrap();
        let find = |id: &str| groups.iter().find(|g| g.id == id).unwrap().clone();
        assert_eq!(find(&child.id).conversation_ids.len(), 2);
        assert_eq!(find(&tag.id).conversation_ids, vec![ids[2].clone()]);
        assert_eq!(find(&query.id).conversation_ids.len(), 2);
        Group::delete(&pool, &root.id).await.unwrap();
        let after = Group::get(&pool, &child.id).await.unwrap();
        assert_eq!(after.parent_id, None);
    }

    #[tokio::test]
    async fn sections_migrate_once() {
        let pool = test_pool().await;
        let (profile, _m, _s, ids) = seed(&pool).await;
        sqlx::query("INSERT INTO sidebar_section (id, profile_id, name, position, created_at) VALUES ('s1', ?1, 'Old section', 0, ?2)")
            .bind(&profile)
            .bind(Utc::now().to_rfc3339())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sidebar_section_item (id, section_id, conversation_id, position) VALUES ('i1', 's1', ?1, 0)")
            .bind(&ids[0])
            .execute(&pool)
            .await
            .unwrap();
        let groups = all(&pool, &profile).await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "Old section");
        assert_eq!(groups[0].conversation_ids, vec![ids[0].clone()]);
        assert_eq!(all(&pool, &profile).await.unwrap().len(), 1);
    }
}
