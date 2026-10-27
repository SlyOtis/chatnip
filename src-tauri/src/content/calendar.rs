use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Invite {
    pub uid: Option<String>,
    pub method: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub organizer_name: Option<String>,
    pub organizer_email: Option<String>,
    pub attendees: Vec<String>,
    pub start: DateTime<Utc>,
    pub end: Option<DateTime<Utc>>,
    pub all_day: bool,
    pub tzid: Option<String>,
    pub status: Option<String>,
}

fn unfold(ics: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in ics.split('\n').map(|l| l.trim_end_matches('\r')) {
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            out.last_mut().unwrap().push_str(&line[1..]);
        } else if !line.is_empty() {
            out.push(line.to_string());
        }
    }
    out
}

fn unescape(value: &str) -> String {
    value.replace("\\n", "\n").replace("\\N", "\n").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

fn param<'a>(params: &'a [&'a str], name: &str) -> Option<&'a str> {
    params.iter().find_map(|p| p.split_once('=').filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim_matches('"')))
}

fn parse_time(value: &str, params: &[&str]) -> Option<(DateTime<Utc>, bool)> {
    if param(params, "VALUE") == Some("DATE") || (value.len() == 8 && value.chars().all(|c| c.is_ascii_digit())) {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        return Some((Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0)?), true));
    }
    let naive = NaiveDateTime::parse_from_str(value.trim_end_matches('Z'), "%Y%m%dT%H%M%S").ok()?;
    Some((Utc.from_utc_datetime(&naive), false))
}

pub fn parse_ics(ics: &str) -> Vec<Invite> {
    let mut invites = Vec::new();
    let mut method = None;
    let mut current: Option<Vec<(String, Vec<String>, String)>> = None;
    for line in unfold(ics) {
        let Some((left, value)) = line.split_once(':') else { continue };
        let mut parts = left.split(';');
        let name = parts.next().unwrap_or("").to_uppercase();
        let params: Vec<String> = parts.map(String::from).collect();
        match (name.as_str(), value) {
            ("METHOD", v) => method = Some(v.to_string()),
            ("BEGIN", "VEVENT") => current = Some(Vec::new()),
            ("END", "VEVENT") => {
                if let Some(props) = current.take() {
                    let get = |key: &str| props.iter().find(|(n, _, _)| n == key);
                    let Some((_, start_params, start_value)) = get("DTSTART") else { continue };
                    let start_params: Vec<&str> = start_params.iter().map(String::as_str).collect();
                    let Some((start, all_day)) = parse_time(start_value, &start_params) else { continue };
                    let end = get("DTEND").and_then(|(_, p, v)| parse_time(v, &p.iter().map(String::as_str).collect::<Vec<_>>())).map(|(t, _)| t);
                    let organizer = get("ORGANIZER");
                    invites.push(Invite {
                        uid: get("UID").map(|(_, _, v)| v.clone()),
                        method: method.clone(),
                        title: get("SUMMARY").map(|(_, _, v)| unescape(v)).unwrap_or_default(),
                        description: get("DESCRIPTION").map(|(_, _, v)| unescape(v)),
                        location: get("LOCATION").map(|(_, _, v)| unescape(v)).filter(|l| !l.is_empty()),
                        organizer_name: organizer.and_then(|(_, p, _)| param(&p.iter().map(String::as_str).collect::<Vec<_>>(), "CN").map(String::from)),
                        organizer_email: organizer.map(|(_, _, v)| v.trim_start_matches("mailto:").trim_start_matches("MAILTO:").to_string()),
                        attendees: props.iter().filter(|(n, _, _)| n == "ATTENDEE").map(|(_, _, v)| v.trim_start_matches("mailto:").trim_start_matches("MAILTO:").to_string()).collect(),
                        start,
                        end,
                        all_day,
                        tzid: param(&start_params, "TZID").map(String::from),
                        status: get("STATUS").map(|(_, _, v)| v.clone()),
                    });
                }
            }
            (_, v) => {
                if let Some(props) = current.as_mut() {
                    props.push((name.clone(), params, v.to_string()));
                }
            }
        }
    }
    invites
}


#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InviteGuess {
    pub confidence: f64,
    pub title: String,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub location: String,
    pub organizer: String,
    pub join_url: Option<String>,
    pub provider: Option<String>,
}

fn join_link() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r#"(?i)https://(teams\.microsoft\.com/l/meetup-join/[^\s"'<>]+|teams\.live\.com/meet/[^\s"'<>]+|meet\.google\.com/[a-z]{3}-[a-z]{4}-[a-z]{3}|[a-z0-9.-]*zoom\.us/j/\d+[^\s"'<>]*|[a-z0-9.-]*webex\.com/[^\s"'<>]+|whereby\.com/[^\s"'<>]+)"#).expect("join link regex"))
}

fn invite_subject() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)^\s*((updated\s+)?invitation(\s+updated)?|invite|møteinnkalling|møteinvitasjon|invitasjon|oppdatert\s+invitasjon|kalenderinvitasjon|einladung|inbjudan|mødeindkaldelse)\s*[:\-]").expect("invite subject regex"))
}

fn meeting_body() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)(microsoft teams[- ](meeting|møte)|join the meeting now|bli med i møtet( nå)?|join with google meet|bli med med google meet|join zoom meeting|meeting id|møte-id|passcode|passord:)").expect("meeting body regex"))
}

fn provider_of(url: &str) -> &'static str {
    let lower = url.to_lowercase();
    if lower.contains("teams.") {
        "teams"
    } else if lower.contains("meet.google.com") {
        "meet"
    } else if lower.contains("zoom.us") {
        "zoom"
    } else if lower.contains("webex.com") {
        "webex"
    } else {
        "link"
    }
}

pub fn recognise_invite(subject: &str, text: &str, event: Option<&serde_json::Value>) -> InviteGuess {
    let field = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let link = join_link().find(text).map(|m| m.as_str().trim_end_matches(['.', ',', ')']).to_string());
    if let Some(event) = event.filter(|e| e.get("starts_at").and_then(|v| v.as_str()).is_some()) {
        return InviteGuess {
            confidence: 1.0,
            title: field(event, "title"),
            starts_at: Some(field(event, "starts_at")),
            ends_at: event.get("ends_at").and_then(|v| v.as_str()).map(String::from),
            location: field(event, "location"),
            organizer: field(event, "organizer"),
            provider: link.as_deref().map(|l| provider_of(l).to_string()),
            join_url: link,
        };
    }
    let subject_hit = invite_subject().is_match(subject);
    let body_hit = meeting_body().is_match(text);
    let timed = crate::services::calendar_service::looks_like_it_has_a_time(&format!("{} {}", subject, text));
    let confidence: f64 = match (link.is_some(), subject_hit, body_hit, timed) {
        (true, true, _, _) => 0.95,
        (true, false, true, true) => 0.9,
        (true, false, _, true) => 0.75,
        (true, false, _, false) => 0.6,
        (false, true, _, true) => 0.7,
        (false, true, _, false) => 0.55,
        (false, false, true, true) => 0.5,
        _ => 0.0,
    };
    let title = invite_subject().replace(subject, "").trim().to_string();
    InviteGuess {
        confidence,
        title: if title.is_empty() { subject.trim().to_string() } else { title },
        provider: link.as_deref().map(|l| provider_of(l).to_string()),
        join_url: link,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_invites_with_and_without_a_calendar_file() {
        let event = serde_json::json!({ "title": "Designreview", "starts_at": "2026-10-08T11:00:00+00:00", "ends_at": "2026-10-08T12:00:00+00:00", "location": "Møterom Fjell" });
        let ics = recognise_invite("Designreview", "", Some(&event));
        assert_eq!(ics.confidence, 1.0);
        assert_eq!(ics.location, "Møterom Fjell");
        let teams = recognise_invite("Sprintdemo", "Microsoft Teams-møte\nBli med i møtet nå https://teams.microsoft.com/l/meetup-join/19%3ameeting_abc/0?context=x\nMøte-ID: 382 991 004 117\nFre 16. okt 14:00", None);
        assert!(teams.confidence >= 0.85, "{:?}", teams);
        assert_eq!(teams.provider.as_deref(), Some("teams"));
        assert!(teams.join_url.unwrap().starts_with("https://teams.microsoft.com/l/meetup-join/"));
        let english = recognise_invite("Invitation: Quarterly review @ Thu Oct 22, 2026 2pm", "Join with Google Meet https://meet.google.com/abc-defg-hij", None);
        assert!(english.confidence >= 0.85);
        assert_eq!(english.provider.as_deref(), Some("meet"));
        assert!(english.title.starts_with("Quarterly review"));
        let norwegian = recognise_invite("Møteinnkalling: Budsjett 2027", "Vi møtes i rom 4 på torsdag kl 10:00.", None);
        assert!(norwegian.confidence >= 0.5 && norwegian.confidence < 0.85, "{:?}", norwegian);
        assert_eq!(norwegian.title, "Budsjett 2027");
        let plain = recognise_invite("Faktura september", "Vedlagt faktura, forfall 30 dager.", None);
        assert_eq!(plain.confidence, 0.0);
    }

    #[test]
    fn parses_a_google_invite() {
        let raw = crate::content::fixture("invite.eml");
        let ics = &raw[raw.find("BEGIN:VCALENDAR").unwrap()..raw.find("END:VCALENDAR").unwrap() + 13];
        let invites = parse_ics(ics);
        assert_eq!(invites.len(), 1);
        let invite = &invites[0];
        assert_eq!(invite.title, "Designreview");
        assert_eq!(invite.method.as_deref(), Some("REQUEST"));
        assert_eq!(invite.location.as_deref(), Some("Møterom Fjell, Oslo"));
        assert_eq!(invite.organizer_name.as_deref(), Some("Mia Sund"));
        assert_eq!(invite.organizer_email.as_deref(), Some("mia.sund@design.no"));
        assert_eq!(invite.attendees, vec!["milo@northwind.example"]);
        assert_eq!(invite.start.to_rfc3339(), "2026-10-08T11:00:00+00:00");
        assert_eq!(invite.end.unwrap().to_rfc3339(), "2026-10-08T12:00:00+00:00");
        assert!(!invite.all_day);
    }

    #[test]
    fn handles_folding_all_day_tzid_and_several_events() {
        let ics = "BEGIN:VCALENDAR\r\nMETHOD:PUBLISH\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261224\r\nDTEND;VALUE=DATE:20261225\r\nSUMMARY:Julaften\\; fri\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;TZID=Europe/Oslo:20261009T090000\r\nSUMMARY:Standup med en veldig lang tittel som \r\n brettes over to linjer\r\nDESCRIPTION:Linje 1\\nLinje 2\r\nSTATUS:CANCELLED\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:No start\r\nEND:VEVENT\r\nEND:VCALENDAR";
        let invites = parse_ics(ics);
        assert_eq!(invites.len(), 2);
        assert!(invites[0].all_day);
        assert_eq!(invites[0].title, "Julaften; fri");
        assert_eq!(invites[1].title, "Standup med en veldig lang tittel som brettes over to linjer");
        assert_eq!(invites[1].tzid.as_deref(), Some("Europe/Oslo"));
        assert_eq!(invites[1].description.as_deref(), Some("Linje 1\nLinje 2"));
        assert_eq!(invites[1].status.as_deref(), Some("CANCELLED"));
        assert!(invites[1].end.is_none());
    }
}
