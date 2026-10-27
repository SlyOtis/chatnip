use crate::models::calendar::Calendar as ProviderCalendar;
use crate::models::event::{Event, EventDraft, EventSource, ORIGIN_MANUAL, ORIGIN_PARSED, STATUS_CONFIRMED, STATUS_PROPOSED};
use crate::providers::models::{Provider, ProviderAuth};
use crate::providers::traits::{NewRemoteEvent, ProviderTrait};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use icalendar::{Calendar, Component, EventLike};
use regex::Regex;
use sqlx::{Pool, Sqlite};
use std::sync::OnceLock;

fn time_hint() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(\d{1,2}[:.]\d{2}|\d{1,2}\s?(am|pm)|tomorrow|tonight|today|next week|next (mon|tues|wednes|thurs|fri|satur|sun)day|(mon|tues|wednes|thurs|fri|satur|sun)day|(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.? \d{1,2}|\d{1,2}\. ?(jan|feb|mar|apr|mai|jun|jul|aug|sep|okt|nov|des)|i morgen|i dag|på (man|tirs|ons|tors|fre|lør|søn)dag|kl\.? ?\d{1,2}|møte|meeting|call|lunch|lunsj|deadline|frist)\b",
        )
        .expect("time hint regex")
    })
}

pub fn looks_like_it_has_a_time(text: &str) -> bool {
    time_hint().is_match(text)
}

pub fn draft_from_value(item: &serde_json::Value) -> EventDraft {
    EventDraft {
        title: item["title"].as_str().unwrap_or("").to_string(),
        notes: item["notes"].as_str().unwrap_or("").to_string(),
        location: item["location"].as_str().unwrap_or("").to_string(),
        starts_at: item["starts_at"].as_str().unwrap_or("").to_string(),
        ends_at: item["ends_at"].as_str().map(String::from),
        all_day: item["all_day"].as_bool().unwrap_or(false),
        attendees: item["attendees"].as_array().cloned().unwrap_or_default().iter().filter_map(|a| a.as_str().map(String::from)).collect(),
    }
}

pub async fn post_found_event(pool: &Pool<Sqlite>, profile_id: &str, item: &serde_json::Value, confirmed: bool, message_id: Option<&str>, conversation_id: Option<&str>) -> Result<Option<Event>, String> {
    let draft = draft_from_value(item);
    let (starts, _) = Event::ends_at_or_default(&draft)?;
    if let Some(message_id) = message_id {
        if Event::exists_for_message(pool, message_id, &starts.to_rfc3339()).await? {
            return Ok(None);
        }
        sqlx::query("UPDATE message SET scanned_for_events = 1 WHERE id = ?1").bind(message_id).execute(pool).await.map_err(|e| format!("Failed to mark the message as scanned: {}", e))?;
    }
    let confidence = item["confidence"].as_f64().unwrap_or(1.0);
    let origin = if message_id.is_some() { ORIGIN_PARSED } else { ORIGIN_MANUAL };
    let status = if confirmed { STATUS_CONFIRMED } else { STATUS_PROPOSED };
    Event::post(pool, profile_id, &draft, origin, status, confidence, message_id, conversation_id).await.map(Some)
}

pub const SYNC_DAYS_BACK: i64 = 30;
pub const SYNC_DAYS_AHEAD: i64 = 90;
pub const LOCAL_TARGET: &str = "local";
const SAME_START_SECONDS: i64 = 60;

fn normalised_title(title: &str) -> String {
    let lower = title.to_lowercase();
    let mut text = lower.trim();
    for prefix in ["re:", "fw:", "fwd:", "sv:", "vs:", "oppdatert:", "updated:", "invitation:", "invitasjon:"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.trim();
        }
    }
    text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

fn same_meeting(a: &Event, b: &Event) -> bool {
    if a.unmerged || b.unmerged {
        return false;
    }
    if a.calendar_id.is_some() && a.calendar_id == b.calendar_id {
        return false;
    }
    if let (Some(x), Some(y)) = (&a.ical_uid, &b.ical_uid) {
        if x == y {
            return true;
        }
    }
    if a.all_day != b.all_day || normalised_title(&a.title).is_empty() || normalised_title(&a.title) != normalised_title(&b.title) {
        return false;
    }
    let (Ok(start_a), Ok(start_b)) = (DateTime::parse_from_rfc3339(&a.starts_at), DateTime::parse_from_rfc3339(&b.starts_at)) else { return false };
    if (start_a - start_b).num_seconds().abs() > SAME_START_SECONDS {
        return false;
    }
    let people = |e: &Event| e.attendees.iter().map(|x| x.to_lowercase()).collect::<std::collections::HashSet<_>>();
    let (pa, pb) = (people(a), people(b));
    if pa.is_empty() || pb.is_empty() {
        return a.ends_at == b.ends_at;
    }
    pa.intersection(&pb).next().is_some()
}

pub fn merge_groups(events: &[Event]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..events.len()).collect();
    fn root(parent: &mut Vec<usize>, i: usize) -> usize {
        let mut r = i;
        while parent[r] != r {
            r = parent[r];
        }
        let mut c = i;
        while parent[c] != r {
            let next = parent[c];
            parent[c] = r;
            c = next;
        }
        r
    }
    let mut by_link: HashMap<&str, usize> = HashMap::new();
    for (i, event) in events.iter().enumerate() {
        if let Some(link) = event.link_id.as_deref() {
            if let Some(&j) = by_link.get(link) {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                parent[a] = b;
            } else {
                by_link.insert(link, i);
            }
        }
    }
    for i in 0..events.len() {
        for j in (i + 1)..events.len() {
            if same_meeting(&events[i], &events[j]) {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                if a != b {
                    parent[a] = b;
                }
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut slot: HashMap<usize, usize> = HashMap::new();
    for i in 0..events.len() {
        let r = root(&mut parent, i);
        match slot.get(&r) {
            Some(&g) => groups[g].push(i),
            None => {
                slot.insert(r, groups.len());
                groups.push(vec![i]);
            }
        }
    }
    groups
}

fn source_of(event: &Event, calendars: &HashMap<String, ProviderCalendar>) -> EventSource {
    let calendar = event.calendar_id.as_ref().and_then(|id| calendars.get(id));
    EventSource {
        event_id: event.id.clone(),
        calendar_id: event.calendar_id.clone(),
        calendar_name: calendar.map(|c| c.name.clone()).unwrap_or_else(|| "Chatnip".to_string()),
        colour: calendar.and_then(|c| c.colour.clone()),
        provider_id: event.provider_id.clone(),
        provider_type: calendar.map(|c| c.provider_type.clone()),
        provider_external_id: calendar.and_then(|c| c.provider_external_id.clone()),
        response_status: event.response_status.clone(),
        online_meeting_url: event.online_meeting_url.clone(),
    }
}

pub fn merge_events(events: Vec<Event>, calendars: &HashMap<String, ProviderCalendar>) -> Vec<Event> {
    let groups = merge_groups(&events);
    let mut merged: Vec<Event> = groups
        .into_iter()
        .map(|group| {
            let members: Vec<&Event> = group.iter().map(|&i| &events[i]).collect();
            let lead = members
                .iter()
                .max_by_key(|e| (e.online_meeting_url.is_some(), e.response_status.as_deref() == Some("organizer"), e.calendar_id.is_some(), std::cmp::Reverse(e.created_at.clone())))
                .copied()
                .unwrap_or(members[0]);
            let mut out = lead.clone();
            out.sources = members.iter().map(|e| source_of(e, calendars)).collect();
            if out.online_meeting_url.is_none() {
                out.online_meeting_url = members.iter().find_map(|e| e.online_meeting_url.clone());
            }
            if out.location.is_empty() {
                out.location = members.iter().map(|e| e.location.clone()).find(|l| !l.is_empty()).unwrap_or_default();
            }
            let mut attendees: Vec<String> = Vec::new();
            for person in members.iter().flat_map(|e| e.attendees.iter()) {
                if !attendees.iter().any(|a| a.eq_ignore_ascii_case(person)) {
                    attendees.push(person.clone());
                }
            }
            out.attendees = attendees;
            out
        })
        .collect();
    merged.sort_by(|a, b| a.starts_at.cmp(&b.starts_at));
    merged
}

pub async fn merged_between(pool: &Pool<Sqlite>, profile_id: &str, start: &str, end: &str, include_dismissed: bool) -> Result<Vec<Event>, String> {
    let events = Event::between(pool, profile_id, start, end, include_dismissed).await?;
    let calendars: HashMap<String, ProviderCalendar> = ProviderCalendar::for_profile(pool, profile_id).await?.into_iter().map(|c| (c.id.clone(), c)).collect();
    Ok(merge_events(events, &calendars))
}

pub async fn unmerge(pool: &Pool<Sqlite>, event_ids: &[String]) -> Result<(), String> {
    Event::put_link(pool, event_ids, None, true).await
}

pub async fn sync_provider_calendars(pool: &Pool<Sqlite>, provider: &Provider, provider_impl: Arc<dyn ProviderTrait>, auth: &ProviderAuth) -> Result<usize, String> {
    let remote_calendars = provider_impl.get_calendars(auth).await?;
    if remote_calendars.is_empty() {
        return Ok(0);
    }
    let from = Utc::now() - chrono::Duration::days(SYNC_DAYS_BACK);
    let to = Utc::now() + chrono::Duration::days(SYNC_DAYS_AHEAD);
    let mut seen = 0;
    for remote in &remote_calendars {
        let calendar = ProviderCalendar::put_remote(pool, &provider.profile_id, &provider.id, remote).await?;
        if !calendar.enabled {
            continue;
        }
        let events = match provider_impl.get_calendar_events(auth, &remote.external_id, from, to).await {
            Ok(events) => events,
            Err(e) => {
                eprintln!("Calendar {} on {} did not sync: {}", calendar.name, provider.name, e);
                continue;
            }
        };
        let mut keep = Vec::new();
        for event in &events {
            if Event::put_remote(pool, &provider.profile_id, &provider.id, &calendar.id, event).await?.is_some() {
                keep.push(event.external_id.clone());
            }
        }
        Event::prune_remote(pool, &calendar.id, &from.to_rfc3339(), &to.to_rfc3339(), &keep).await?;
        seen += keep.len();
    }
    ProviderCalendar::prune(pool, &provider.id, &remote_calendars.iter().map(|c| c.external_id.clone()).collect::<Vec<_>>()).await?;
    Ok(seen)
}

pub fn new_remote_event(draft: &EventDraft, online_meeting: bool) -> Result<NewRemoteEvent, String> {
    if draft.title.trim().is_empty() {
        return Err("An event needs a title".to_string());
    }
    let (starts_at, ends_at) = Event::ends_at_or_default(draft)?;
    Ok(NewRemoteEvent {
        title: draft.title.trim().to_string(),
        notes: draft.notes.clone(),
        location: draft.location.clone(),
        starts_at,
        ends_at,
        all_day: draft.all_day,
        attendees: draft.attendees.iter().map(|a| a.trim().to_lowercase()).filter(|a| !a.is_empty()).collect(),
        online_meeting,
    })
}

pub async fn post_to_targets(
    pool: &Pool<Sqlite>,
    profile_id: &str,
    draft: &EventDraft,
    targets: &[String],
    online_meeting: bool,
    connect: impl Fn(String) -> futures::future::BoxFuture<'static, Result<(Provider, Arc<dyn ProviderTrait>, ProviderAuth), String>>,
) -> Result<(Vec<Event>, Vec<String>), String> {
    let wanted: Vec<String> = if targets.is_empty() { vec![LOCAL_TARGET.to_string()] } else { targets.to_vec() };
    let remote = new_remote_event(draft, online_meeting)?;
    let mut created_ids = Vec::new();
    let mut failures = Vec::new();
    for target in &wanted {
        if target == LOCAL_TARGET {
            created_ids.push(Event::post(pool, profile_id, draft, ORIGIN_MANUAL, STATUS_CONFIRMED, 1.0, None, None).await?.id);
            continue;
        }
        let calendar = ProviderCalendar::get(pool, target).await?;
        if calendar.profile_id != profile_id {
            return Err("That calendar belongs to another profile".to_string());
        }
        if !calendar.can_write {
            failures.push(format!("{} is read only", calendar.name));
            continue;
        }
        let attempt = async {
            let (provider, provider_impl, auth) = connect(calendar.provider_id.clone()).await?;
            let made = provider_impl.post_calendar_event(&auth, &calendar.external_id, &remote).await?;
            Event::put_remote(pool, profile_id, &provider.id, &calendar.id, &made).await
        };
        match attempt.await {
            Ok(Some(id)) => created_ids.push(id),
            Ok(None) => {}
            Err(e) => failures.push(format!("{}: {}", calendar.name, e)),
        }
    }
    if created_ids.is_empty() {
        return Err(failures.join("; "));
    }
    if created_ids.len() > 1 {
        Event::put_link(pool, &created_ids, Some(&Uuid::new_v4().to_string()), false).await?;
    }
    let mut created = Vec::new();
    for id in &created_ids {
        created.push(Event::get(pool, id).await?);
    }
    Ok((created, failures))
}

pub async fn put_response(pool: &Pool<Sqlite>, event_id: &str, response: &str) -> Result<Event, String> {
    let event = Event::get(pool, event_id).await?;
    let (Some(calendar_id), Some(external_id), Some(provider_id)) = (event.calendar_id.clone(), event.external_id.clone(), event.provider_id.clone()) else {
        return Err(format!("{} lives only in Chatnip; there is no invitation to answer", event.title));
    };
    let calendar = ProviderCalendar::get(pool, &calendar_id).await?;
    let (provider, provider_impl, auth) = crate::services::connection_service::connected(pool, &provider_id).await?;
    let answered = provider_impl.put_calendar_response(&auth, &calendar.external_id, &external_id, response).await?;
    Event::put_remote(pool, &event.profile_id, &provider.id, &calendar.id, &answered).await?;
    Event::get(pool, event_id).await
}

pub async fn ics(pool: &Pool<Sqlite>, profile_id: &str) -> Result<String, String> {
    let events = Event::between(pool, profile_id, "0000", "9999", false).await?;
    let mut calendar = Calendar::new();
    calendar.name("Chatnip");
    for event in events.iter().filter(|e| e.status == STATUS_CONFIRMED) {
        let starts = DateTime::parse_from_rfc3339(&event.starts_at).map_err(|_| "bad start".to_string())?.with_timezone(&Utc);
        let ends = DateTime::parse_from_rfc3339(&event.ends_at).map_err(|_| "bad end".to_string())?.with_timezone(&Utc);
        let mut item = icalendar::Event::new();
        item.uid(&format!("{}@chatnip", event.id)).summary(&event.title).description(&event.notes);
        if !event.location.is_empty() {
            item.location(&event.location);
        }
        if event.all_day {
            item.all_day(starts.date_naive());
        } else {
            item.starts(starts).ends(ends);
        }
        calendar.push(item.done());
    }
    Ok(calendar.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: &str, calendar: Option<&str>, title: &str, start: &str, attendees: &[&str]) -> Event {
        Event {
            id: id.into(),
            profile_id: "p".into(),
            title: title.into(),
            notes: String::new(),
            location: String::new(),
            starts_at: start.into(),
            ends_at: "2026-10-16T12:00:00+00:00".into(),
            all_day: false,
            status: STATUS_CONFIRMED.into(),
            origin: "synced".into(),
            confidence: 1.0,
            source_message_id: None,
            conversation_id: None,
            attendees: attendees.iter().map(|a| a.to_string()).collect(),
            created_at: "2026-10-01T00:00:00Z".into(),
            updated_at: "2026-10-01T00:00:00Z".into(),
            provider_id: calendar.map(|c| format!("prov-{}", c)),
            calendar_id: calendar.map(String::from),
            external_id: calendar.map(|c| format!("{}-{}", c, id)),
            ical_uid: None,
            online_meeting_url: None,
            organizer: None,
            response_status: None,
            link_id: None,
            unmerged: false,
            sources: Vec::new(),
        }
    }

    fn sizes(groups: &[Vec<usize>]) -> Vec<usize> {
        groups.iter().map(|g| g.len()).collect()
    }

    #[test]
    fn the_same_ical_uid_in_two_calendars_is_one_meeting() {
        let mut a = event("a", Some("work"), "Signering", "2026-10-16T11:00:00+00:00", &[]);
        let mut b = event("b", Some("client"), "Signing (moved)", "2026-10-16T15:00:00+00:00", &[]);
        a.ical_uid = Some("uid-1".into());
        b.ical_uid = Some("uid-1".into());
        assert_eq!(sizes(&merge_groups(&[a, b])), vec![2]);
    }

    #[test]
    fn title_start_and_shared_people_merge_without_a_uid() {
        let a = event("a", Some("work"), "Lunsj med Eva", "2026-10-16T11:00:00+00:00", &["eva.ortega@northwind.example", "milo@northwind.example"]);
        let b = event("b", Some("private"), "  lunsj MED eva ", "2026-10-16T11:00:30Z", &["Eva.ortega@northwind.example"]);
        let c = event("c", Some("private2"), "Re: Lunsj med Eva", "2026-10-16T11:00:00+00:00", &["milo@northwind.example"]);
        assert_eq!(sizes(&merge_groups(&[a, b, c])), vec![3]);
    }

    #[test]
    fn different_people_times_or_the_same_calendar_stay_apart() {
        let base = event("a", Some("work"), "Standup", "2026-10-16T09:00:00+00:00", &["x@a.no"]);
        let strangers = event("b", Some("other"), "Standup", "2026-10-16T09:00:00+00:00", &["y@b.no"]);
        let later = event("c", Some("third"), "Standup", "2026-10-16T09:05:00+00:00", &["x@a.no"]);
        let same_calendar = event("d", Some("work"), "Standup", "2026-10-16T09:00:00+00:00", &["x@a.no"]);
        assert_eq!(sizes(&merge_groups(&[base, strangers, later, same_calendar])), vec![1, 1, 1, 1]);
    }

    #[test]
    fn links_join_and_unmerged_events_never_rejoin() {
        let mut a = event("a", Some("work"), "Kickoff", "2026-10-16T09:00:00+00:00", &[]);
        let mut b = event("b", None, "Kickoff local", "2026-10-17T09:00:00+00:00", &[]);
        a.link_id = Some("l1".into());
        b.link_id = Some("l1".into());
        assert_eq!(sizes(&merge_groups(&[a.clone(), b.clone()])), vec![2]);
        let mut x = event("x", Some("work"), "Sync", "2026-10-16T10:00:00+00:00", &["p@a.no"]);
        let y = event("y", Some("other"), "Sync", "2026-10-16T10:00:00+00:00", &["p@a.no"]);
        x.unmerged = true;
        assert_eq!(sizes(&merge_groups(&[x, y])), vec![1, 1]);
    }

    #[test]
    fn the_merged_event_lists_every_source_and_prefers_the_one_with_a_meeting_link() {
        let mut a = event("a", Some("work"), "Signering", "2026-10-16T11:00:00+00:00", &["anna@client.example"]);
        let mut b = event("b", Some("client"), "Signering", "2026-10-16T11:00:00+00:00", &["anna@client.example", "jonas@client.example"]);
        b.online_meeting_url = Some("https://teams.microsoft.com/l/x".into());
        a.location = "Rom 4".into();
        let merged = merge_events(vec![a, b], &HashMap::new());
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "b");
        assert_eq!(merged[0].sources.iter().map(|s| s.event_id.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
        assert_eq!(merged[0].location, "Rom 4");
        assert_eq!(merged[0].attendees, vec!["anna@client.example", "jonas@client.example"]);
    }

    async fn mock_connection(pool: &Pool<Sqlite>, profile: &str, kind: crate::providers::mock::MockKind) -> (Provider, Arc<dyn ProviderTrait>) {
        let id = crate::test_helpers::test_provider(pool, profile, "mock").await;
        let implementation = crate::providers::mock::MockProvider::new(kind, pool.clone());
        let mut provider = implementation.get_provider().clone();
        provider.id = id;
        provider.profile_id = profile.to_string();
        (provider, Arc::new(implementation))
    }

    fn mock_auth() -> ProviderAuth {
        ProviderAuth { access_token: "mock".into(), refresh_token: None, expires_at: Utc::now(), token_type: "Mock".into(), scope: None }
    }

    #[tokio::test]
    async fn synced_mock_tenants_merge_shared_meetings_and_can_be_split() {
        use crate::providers::mock::MockKind;
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        for kind in [MockKind::Outlook, MockKind::Teams, MockKind::Gmail] {
            let (provider, implementation) = mock_connection(&pool, &profile, kind).await;
            assert!(sync_provider_calendars(&pool, &provider, implementation, &mock_auth()).await.unwrap() > 0);
        }
        assert_eq!(ProviderCalendar::for_profile(&pool, &profile).await.unwrap().len(), 4);
        let from = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        let to = (Utc::now() + chrono::Duration::days(90)).to_rfc3339();
        let merged = merged_between(&pool, &profile, &from, &to, false).await.unwrap();
        let signing = merged.iter().find(|e| e.title == "Signering: plattformdrift 2027").unwrap();
        assert_eq!(signing.sources.len(), 2);
        assert!(signing.online_meeting_url.is_some());
        let lunch = merged.iter().find(|e| e.title == "Lunsj med Eva").unwrap();
        assert_eq!(lunch.sources.len(), 2);
        assert!(!merged.iter().any(|e| e.title.starts_with("Avlyst")));
        unmerge(&pool, &lunch.sources.iter().map(|s| s.event_id.clone()).collect::<Vec<_>>()).await.unwrap();
        let split = merged_between(&pool, &profile, &from, &to, false).await.unwrap();
        assert_eq!(split.iter().filter(|e| e.title == "Lunsj med Eva").count(), 2);
        let holidays = ProviderCalendar::for_profile(&pool, &profile).await.unwrap().into_iter().find(|c| !c.can_write).unwrap();
        ProviderCalendar::put_enabled(&pool, &holidays.id, false).await.unwrap();
        assert!(!merged_between(&pool, &profile, &from, &to, false).await.unwrap().iter().any(|e| e.title == "Høstferie"));
    }

    #[tokio::test]
    async fn one_event_lands_in_several_calendars_as_a_linked_invite() {
        use crate::providers::mock::MockKind;
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        let mut connections: HashMap<String, (Provider, Arc<dyn ProviderTrait>)> = HashMap::new();
        for kind in [MockKind::Outlook, MockKind::Gmail] {
            let (provider, implementation) = mock_connection(&pool, &profile, kind).await;
            sync_provider_calendars(&pool, &provider, implementation.clone(), &mock_auth()).await.unwrap();
            connections.insert(provider.id.clone(), (provider, implementation));
        }
        let calendars = ProviderCalendar::for_profile(&pool, &profile).await.unwrap();
        let writable: Vec<String> = calendars.iter().filter(|c| c.can_write).map(|c| c.id.clone()).collect();
        let read_only = calendars.iter().find(|c| !c.can_write).unwrap().id.clone();
        let mut targets = writable.clone();
        targets.push(LOCAL_TARGET.to_string());
        targets.push(read_only);
        let draft = EventDraft { title: "Kickoff".into(), starts_at: (Utc::now() + chrono::Duration::days(3)).to_rfc3339(), attendees: vec![" Anna.Berg@client.example ".into()], ..Default::default() };
        let connections = Arc::new(connections);
        let connect = move |id: String| -> futures::future::BoxFuture<'static, Result<(Provider, Arc<dyn ProviderTrait>, ProviderAuth), String>> {
            let found = connections.get(&id).cloned();
            Box::pin(async move { found.map(|(p, i)| (p, i, mock_auth())).ok_or_else(|| "unknown".to_string()) })
        };
        let (created, failures) = post_to_targets(&pool, &profile, &draft, &targets, true, connect).await.unwrap();
        assert_eq!(created.len(), 3);
        assert_eq!(failures.len(), 1);
        assert!(created.iter().filter(|e| e.calendar_id.is_some()).all(|e| e.online_meeting_url.is_some() && e.attendees == vec!["anna.berg@client.example"]));
        let link = created[0].link_id.clone().unwrap();
        assert!(created.iter().all(|e| e.link_id.as_deref() == Some(link.as_str())));
        let from = Utc::now().to_rfc3339();
        let to = (Utc::now() + chrono::Duration::days(10)).to_rfc3339();
        let kickoff: Vec<Event> = merged_between(&pool, &profile, &from, &to, false).await.unwrap().into_iter().filter(|e| e.title == "Kickoff").collect();
        assert_eq!(kickoff.len(), 1);
        assert_eq!(kickoff[0].sources.len(), 3);
    }

    #[test]
    fn a_new_remote_event_needs_a_title_and_cleans_attendees() {
        assert!(new_remote_event(&EventDraft { title: " ".into(), starts_at: "2026-10-16T09:00:00Z".into(), ..Default::default() }, false).is_err());
        let made = new_remote_event(&EventDraft { title: " Plan ".into(), starts_at: "2026-10-16T09:00:00Z".into(), attendees: vec!["".into(), " A@B.no".into()], ..Default::default() }, true).unwrap();
        assert_eq!(made.title, "Plan");
        assert_eq!(made.attendees, vec!["a@b.no"]);
        assert_eq!(made.ends_at.to_rfc3339(), "2026-10-16T10:00:00+00:00");
        assert!(made.online_meeting);
    }

    #[test]
    fn the_prefilter_catches_times_and_skips_noise() {
        assert!(looks_like_it_has_a_time("Can we meet tomorrow at 14:30?"));
        assert!(looks_like_it_has_a_time("Lunch on Friday"));
        assert!(looks_like_it_has_a_time("Møte i morgen kl 9"));
        assert!(looks_like_it_has_a_time("Deadline is Oct 12"));
        assert!(!looks_like_it_has_a_time("Thanks for the files, all good here."));
    }

    #[tokio::test]
    async fn the_feed_lists_confirmed_events_only() {
        let pool = crate::test_helpers::test_pool().await;
        let profile = crate::test_helpers::test_profile(&pool).await;
        let confirmed = EventDraft { title: "Standup".into(), starts_at: "2026-10-12T08:00:00Z".into(), ..Default::default() };
        let proposed = EventDraft { title: "Maybe".into(), starts_at: "2026-10-13T08:00:00Z".into(), ..Default::default() };
        Event::post(&pool, &profile, &confirmed, "manual", STATUS_CONFIRMED, 1.0, None, None).await.unwrap();
        Event::post(&pool, &profile, &proposed, "parsed", STATUS_PROPOSED, 0.5, None, None).await.unwrap();
        let feed = ics(&pool, &profile).await.unwrap();
        assert!(feed.contains("SUMMARY:Standup"));
        assert!(!feed.contains("SUMMARY:Maybe"));
        assert!(feed.contains("BEGIN:VCALENDAR"));
    }
}
