use sqlx::{Pool, Row, Sqlite};

use crate::models::case::{CaseExpectation, CaseFile, CaseItem, CaseRule};
use crate::models::Message;
use crate::services::{MessageLoadOptions, MessageService};

#[derive(Debug, Clone, Default)]
pub struct Incoming {
    pub id: String,
    pub conversation_id: String,
    pub sender_name: String,
    pub sender_email: String,
    pub subject: String,
    pub conversation_name: String,
    pub attachments: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arrival {
    pub expectation_id: String,
    pub case_id: String,
    pub message_id: String,
    pub attachment: Option<String>,
}

pub fn glob_to_regex(pattern: &str) -> Option<regex::Regex> {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(body) = trimmed.strip_prefix('/').and_then(|p| p.strip_suffix('/')) {
        return regex::RegexBuilder::new(body).case_insensitive(true).build().ok();
    }
    let mut out = String::from("^");
    for ch in trimmed.chars() {
        match ch {
            '*' => out.push_str(".*"),
            '?' => out.push('.'),
            c => out.push_str(&regex::escape(&c.to_string())),
        }
    }
    out.push('$');
    regex::RegexBuilder::new(&out).case_insensitive(true).build().ok()
}

fn contains_folded(haystack: &str, needle: &str) -> bool {
    let needle = needle.trim().to_lowercase();
    needle.is_empty() || haystack.to_lowercase().contains(&needle)
}

pub fn matches(expectation: &CaseExpectation, message: &Incoming) -> Option<Option<String>> {
    if let Some(from) = &expectation.from_match {
        let haystack = format!("{} {}", message.sender_name, message.sender_email);
        if !contains_folded(&haystack, from) {
            return None;
        }
    }
    if let Some(subject) = &expectation.subject_match {
        let haystack = format!("{} {}", message.subject, message.conversation_name);
        if !contains_folded(&haystack, subject) {
            return None;
        }
    }
    let wants_file = expectation.kind == "file" || expectation.filename_match.is_some() || expectation.mime_match.is_some();
    if !wants_file {
        return Some(None);
    }
    let pattern = expectation.filename_match.as_deref().and_then(glob_to_regex);
    let hit = message.attachments.iter().find(|(name, mime)| {
        let name_ok = match (&pattern, &expectation.filename_match) {
            (Some(re), _) => re.is_match(name),
            (None, Some(raw)) if !raw.trim().is_empty() => false,
            _ => true,
        };
        let mime_ok = expectation.mime_match.as_deref().map(|m| contains_folded(mime, m)).unwrap_or(true);
        name_ok && mime_ok
    })?;
    Some(Some(hit.0.clone()))
}

pub fn arrivals(expectations: &[CaseExpectation], message: &Incoming) -> Vec<Arrival> {
    expectations
        .iter()
        .filter(|e| e.status == crate::models::case::EXPECT_PENDING)
        .filter_map(|e| {
            matches(e, message).map(|attachment| Arrival {
                expectation_id: e.id.clone(),
                case_id: e.case_id.clone(),
                message_id: message.id.clone(),
                attachment,
            })
        })
        .collect()
}

fn attachments_of(raw: Option<String>) -> Vec<(String, String)> {
    let parsed: serde_json::Value = raw.as_deref().and_then(|r| serde_json::from_str(r).ok()).unwrap_or(serde_json::Value::Null);
    parsed
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    let name = a.get("name").and_then(|v| v.as_str())?.to_string();
                    let mime = a
                        .get("content_type")
                        .or_else(|| a.get("mime_type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some((name, mime))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub async fn incoming_for_conversation(pool: &Pool<Sqlite>, conversation_id: &str) -> Result<Vec<Incoming>, String> {
    let rows = sqlx::query(
        "SELECT m.id, m.conversation_id, m.sender_name, m.sender_email, m.subject, m.attachments, c.name AS conversation_name
         FROM message m JOIN conversation c ON c.id = m.conversation_id
         WHERE m.conversation_id = ?1 AND m.deleted_datetime IS NULL ORDER BY m.timestamp",
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to load messages for case matching: {}", e))?;
    Ok(rows
        .into_iter()
        .map(|r| Incoming {
            id: r.get("id"),
            conversation_id: r.get("conversation_id"),
            sender_name: r.try_get::<Option<String>, _>("sender_name").ok().flatten().unwrap_or_default(),
            sender_email: r.try_get::<Option<String>, _>("sender_email").ok().flatten().unwrap_or_default(),
            subject: r.try_get::<Option<String>, _>("subject").ok().flatten().unwrap_or_default(),
            conversation_name: r.try_get::<Option<String>, _>("conversation_name").ok().flatten().unwrap_or_default(),
            attachments: attachments_of(r.try_get("attachments").ok().flatten()),
        })
        .collect())
}

pub async fn match_incoming(pool: &Pool<Sqlite>, profile_id: &str, message: &Incoming) -> Result<Vec<Arrival>, String> {
    let pending = CaseExpectation::pending(pool, profile_id).await?;
    let found = arrivals(&pending, message);
    for arrival in &found {
        CaseExpectation::mark_arrived(pool, &arrival.expectation_id, &arrival.message_id, arrival.attachment.as_deref()).await?;
        CaseItem::add(pool, &arrival.case_id, Some(&message.conversation_id), None).await?;
        crate::models::case::touch(pool, &arrival.case_id).await?;
    }
    Ok(found)
}

pub async fn apply_rules(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str) -> Result<Vec<String>, String> {
    let rules = CaseRule::for_profile(pool, profile_id).await?;
    if rules.is_empty() {
        return Ok(Vec::new());
    }
    let subject = crate::services::sorting_service::subject(pool, conversation_id).await?;
    let mut added = Vec::new();
    for rule in rules.iter().filter(|r| r.matches(&subject)) {
        if CaseItem::add(pool, &rule.case_id, Some(conversation_id), None).await? {
            crate::models::case::touch(pool, &rule.case_id).await?;
            added.push(rule.case_id.clone());
        }
    }
    Ok(added)
}

pub async fn scan_conversation(pool: &Pool<Sqlite>, profile_id: &str, conversation_id: &str) -> Result<bool, String> {
    let mut changed = !apply_rules(pool, profile_id, conversation_id).await?.is_empty();
    for message in incoming_for_conversation(pool, conversation_id).await? {
        changed |= !match_incoming(pool, profile_id, &message).await?.is_empty();
    }
    Ok(changed)
}

pub async fn scan_all(pool: &Pool<Sqlite>, profile_id: &str) -> Result<bool, String> {
    let rows = sqlx::query("SELECT id FROM conversation WHERE deleted_at IS NULL")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list conversations: {}", e))?;
    let mut changed = false;
    for row in rows {
        let id: String = row.get("id");
        changed |= scan_conversation(pool, profile_id, &id).await?;
    }
    Ok(changed)
}

pub async fn timeline(pool: &Pool<Sqlite>, case: &CaseFile, limit: Option<u32>) -> Result<Vec<Message>, String> {
    let loader = MessageService::new(pool.clone());
    let mut messages: Vec<Message> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let whole: std::collections::HashSet<&str> = case.items.iter().filter_map(|i| i.conversation_id.as_deref()).collect();
    for conversation_id in &whole {
        let loaded = loader
            .load_conversation_messages(conversation_id, MessageLoadOptions { load_reactions: true, load_assets: true, load_sender_details: true, limit: None, offset: None })
            .await?;
        for m in loaded {
            if seen.insert(m.message.id.clone()) {
                messages.push(m.message);
            }
        }
    }
    let singles: Vec<&str> = case.items.iter().filter_map(|i| i.message_id.as_deref()).collect();
    for message_id in singles {
        let conversation = crate::models::Message::get_conversation_id(pool, message_id).await?;
        let Some(conversation) = conversation else { continue };
        if whole.contains(conversation.as_str()) {
            continue;
        }
        let loaded = loader
            .load_conversation_messages(&conversation, MessageLoadOptions { load_reactions: true, load_assets: true, load_sender_details: true, limit: None, offset: None })
            .await?;
        for m in loaded.into_iter().filter(|m| m.message.id == message_id) {
            if seen.insert(m.message.id.clone()) {
                messages.push(m.message);
            }
        }
    }
    messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    if let Some(limit) = limit {
        let keep = messages.len().saturating_sub(limit as usize);
        messages.drain(..keep);
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::case::{CaseItemsInput, PostCaseInput, PostCaseRuleInput, PostExpectationInput, PutCaseInput, EXPECT_ARRIVED, EXPECT_OVERDUE, EXPECT_PENDING};
    use crate::models::rule::{Condition, Field, Operator};
    use crate::test_helpers::*;
    use chrono::{Duration, Utc};

    fn expectation(kind: &str, from: Option<&str>, subject: Option<&str>, file: Option<&str>) -> CaseExpectation {
        CaseExpectation {
            id: "e".into(),
            case_id: "c".into(),
            title: "t".into(),
            kind: kind.into(),
            from_match: from.map(Into::into),
            subject_match: subject.map(Into::into),
            filename_match: file.map(Into::into),
            mime_match: None,
            due_at: None,
            status: EXPECT_PENDING.into(),
            arrived_message_id: None,
            arrived_attachment_name: None,
            arrived_at: None,
            arrived_from: None,
            arrived_conversation_id: None,
            created_at: Utc::now().to_rfc3339(),
        }
    }

    fn mail(from: &str, subject: &str, files: &[(&str, &str)]) -> Incoming {
        Incoming {
            id: "m".into(),
            conversation_id: "conv".into(),
            sender_name: from.split('@').next().unwrap_or("").into(),
            sender_email: from.into(),
            subject: subject.into(),
            conversation_name: subject.into(),
            attachments: files.iter().map(|(n, m)| (n.to_string(), m.to_string())).collect(),
        }
    }

    #[test]
    fn glob_patterns_and_slash_regex() {
        assert!(glob_to_regex("contract*.pdf").unwrap().is_match("Contract_signed.PDF"));
        assert!(!glob_to_regex("contract*.pdf").unwrap().is_match("invoice.pdf"));
        assert!(glob_to_regex("report-??.xlsx").unwrap().is_match("report-09.xlsx"));
        assert!(glob_to_regex("/^inv(oice)?-\\d+\\.pdf$/").unwrap().is_match("INV-2041.pdf"));
        assert!(glob_to_regex("  ").is_none());
    }

    #[test]
    fn expectation_matching_on_sender_subject_and_file() {
        let want = expectation("file", Some("client.example"), Some("kontrakt"), Some("contract*.pdf"));
        assert_eq!(matches(&want, &mail("anna.berg@client.example", "Kontrakt Kunde AS", &[("contract.pdf", "application/pdf")])), Some(Some("contract.pdf".into())));
        assert_eq!(matches(&want, &mail("anna.berg@client.example", "Kontrakt Kunde AS", &[("notes.txt", "text/plain")])), None);
        assert_eq!(matches(&want, &mail("ola@other.no", "Kontrakt Kunde AS", &[("contract.pdf", "application/pdf")])), None);
        assert_eq!(matches(&want, &mail("anna.berg@client.example", "Lunsj", &[("contract.pdf", "application/pdf")])), None);
        let reply = expectation("reply", Some("Anna"), None, None);
        assert_eq!(matches(&reply, &mail("anna.berg@client.example", "Re: noe", &[])), Some(None));
        let any_pdf = CaseExpectation { mime_match: Some("pdf".into()), ..expectation("file", None, None, None) };
        assert_eq!(matches(&any_pdf, &mail("x@y.z", "s", &[("a.png", "image/png"), ("b.pdf", "application/pdf")])), Some(Some("b.pdf".into())));
    }

    #[test]
    fn overdue_is_derived_from_due_date() {
        let past = CaseExpectation { due_at: Some((Utc::now() - Duration::days(1)).to_rfc3339()), ..expectation("file", None, None, None) };
        let future = CaseExpectation { due_at: Some((Utc::now() + Duration::days(1)).to_rfc3339()), ..expectation("file", None, None, None) };
        let arrived = CaseExpectation { status: EXPECT_ARRIVED.into(), ..past.clone() };
        let now = Utc::now().to_rfc3339();
        assert_eq!(past.effective_status(&now), EXPECT_OVERDUE);
        assert_eq!(future.effective_status(&now), EXPECT_PENDING);
        assert_eq!(arrived.effective_status(&now), EXPECT_ARRIVED);
    }

    async fn message_with(pool: &Pool<Sqlite>, conversation: &str, provider: &str, sender: &str, subject: &str, attachments: &str, minutes_ago: i64) -> String {
        let id = test_message(pool, conversation, provider, subject).await;
        sqlx::query("UPDATE message SET sender_email = ?1, sender_name = ?2, subject = ?3, attachments = ?4, has_attachments = ?5, timestamp = ?6 WHERE id = ?7")
            .bind(sender)
            .bind(sender.split('@').next().unwrap_or(""))
            .bind(subject)
            .bind(attachments)
            .bind(attachments != "[]")
            .bind((Utc::now() - Duration::minutes(minutes_ago)).to_rfc3339())
            .bind(&id)
            .execute(pool)
            .await
            .unwrap();
        id
    }

    #[tokio::test]
    async fn many_to_many_membership_and_timeline_order() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "imap").await;
        let a = test_conversation(&pool, "Kontrakt Kunde AS", "email").await;
        let b = test_conversation(&pool, "Lunsj", "direct_message").await;
        let m1 = message_with(&pool, &a, &provider, "anna@client.example", "Kontrakt", "[]", 30).await;
        let m2 = message_with(&pool, &b, &provider, "ola@northwind.example", "Lunsj", "[]", 20).await;
        let m3 = message_with(&pool, &a, &provider, "anna@client.example", "Re: Kontrakt", "[]", 10).await;
        let one = CaseFile::post(&pool, &profile, &PostCaseInput { name: "Kunde AS".into(), description: None, color: None, due_at: None, conversation_ids: vec![a.clone()], message_ids: vec![m2.clone()] }).await.unwrap();
        let two = CaseFile::post(&pool, &profile, &PostCaseInput { name: "Q4".into(), description: None, color: None, due_at: None, conversation_ids: vec![a.clone()], message_ids: vec![] }).await.unwrap();
        assert!(!CaseItem::add(&pool, &one, Some(&a), None).await.unwrap());
        let mut cases = CaseFile::for_conversation(&pool, &a).await.unwrap();
        cases.sort();
        let mut both = vec![one.clone(), two.clone()];
        both.sort();
        assert_eq!(cases, both);
        assert_eq!(CaseFile::for_conversation(&pool, &b).await.unwrap(), vec![one.clone()]);
        let case = CaseFile::get(&pool, &one).await.unwrap();
        assert_eq!((case.conversation_count, case.message_count), (1, 1));
        let ids: Vec<String> = timeline(&pool, &case, None).await.unwrap().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec![m1.clone(), m2.clone(), m3.clone()]);
        let last_two: Vec<String> = timeline(&pool, &case, Some(2)).await.unwrap().into_iter().map(|m| m.id).collect();
        assert_eq!(last_two, vec![m2.clone(), m3.clone()]);
        let input = CaseItemsInput { case_id: one.clone(), conversation_ids: vec![], message_ids: vec![m2.clone()] };
        for m in &input.message_ids {
            CaseItem::remove(&pool, &input.case_id, None, Some(m)).await.unwrap();
        }
        assert_eq!(CaseFile::get(&pool, &one).await.unwrap().message_count, 0);
        CaseFile::put(&pool, &PutCaseInput { case_id: one.clone(), status: Some("waiting".into()), ..Default::default() }).await.unwrap();
        assert_eq!(CaseFile::get(&pool, &one).await.unwrap().status, "waiting");
        assert!(CaseFile::put(&pool, &PutCaseInput { case_id: one.clone(), status: Some("lost".into()), ..Default::default() }).await.is_err());
        CaseFile::delete(&pool, &two).await.unwrap();
        assert_eq!(CaseFile::for_conversation(&pool, &a).await.unwrap(), vec![one]);
    }

    #[tokio::test]
    async fn incoming_file_arrives_and_joins_the_case() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "imap").await;
        let thread = test_conversation(&pool, "Kontrakt Kunde AS", "email").await;
        let case = CaseFile::post(&pool, &profile, &PostCaseInput { name: "Kunde AS".into(), description: None, color: None, due_at: None, conversation_ids: vec![], message_ids: vec![] }).await.unwrap();
        CaseExpectation::post(&pool, &PostExpectationInput { case_id: case.clone(), title: "Signed contract".into(), kind: Some("file".into()), from_match: Some("client.example".into()), filename_match: Some("contract*.pdf".into()), ..Default::default() }).await.unwrap();
        message_with(&pool, &thread, &provider, "anna@client.example", "Kontrakt", "[]", 20).await;
        assert!(!scan_conversation(&pool, &profile, &thread).await.unwrap());
        let file = message_with(&pool, &thread, &provider, "anna@client.example", "Re: Kontrakt", r#"[{"id":"1","name":"Contract-signed.pdf","content_type":"application/pdf","size":120000,"is_inline":false,"content_id":null}]"#, 5).await;
        assert!(scan_conversation(&pool, &profile, &thread).await.unwrap());
        let loaded = CaseFile::get(&pool, &case).await.unwrap();
        let e = &loaded.expectations[0];
        assert_eq!(e.status, EXPECT_ARRIVED);
        assert_eq!(e.arrived_message_id.as_deref(), Some(file.as_str()));
        assert_eq!(e.arrived_attachment_name.as_deref(), Some("Contract-signed.pdf"));
        assert_eq!(e.arrived_conversation_id.as_deref(), Some(thread.as_str()));
        assert_eq!(loaded.items.len(), 1);
        assert!(!scan_conversation(&pool, &profile, &thread).await.unwrap());
    }

    #[tokio::test]
    async fn auto_add_rule_pulls_matching_conversations() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "imap").await;
        let hit = test_conversation(&pool, "Kontrakt Kunde AS", "email").await;
        let miss = test_conversation(&pool, "Lunsj", "email").await;
        message_with(&pool, &hit, &provider, "anna@client.example", "Kontrakt", "[]", 5).await;
        message_with(&pool, &miss, &provider, "ola@northwind.example", "Lunsj", "[]", 5).await;
        let case = CaseFile::post(&pool, &profile, &PostCaseInput { name: "Kunde AS".into(), description: None, color: None, due_at: None, conversation_ids: vec![], message_ids: vec![] }).await.unwrap();
        CaseRule::post(&pool, &PostCaseRuleInput { case_id: case.clone(), r#match: None, conditions: vec![Condition { field: Field::SenderDomain, operator: Operator::Is, value: "client.example".into() }] }).await.unwrap();
        assert!(scan_all(&pool, &profile).await.unwrap());
        let loaded = CaseFile::get(&pool, &case).await.unwrap();
        assert_eq!(loaded.items.iter().filter_map(|i| i.conversation_id.clone()).collect::<Vec<_>>(), vec![hit]);
        assert!(!scan_all(&pool, &profile).await.unwrap());
    }
}
