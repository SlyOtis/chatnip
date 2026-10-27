use super::rx;
use regex::Regex;
use std::net::IpAddr;
use std::sync::OnceLock;

pub fn unfold(headers: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in headers.split('\n').map(|l| l.trim_end_matches('\r')) {
        if line.is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(last) = out.last_mut() {
                last.1.push(' ');
                last.1.push_str(line.trim());
            }
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            out.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    out
}

pub fn values(headers: &str, name: &str) -> Vec<String> {
    unfold(headers).into_iter().filter(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v).collect()
}

pub fn value(headers: &str, name: &str) -> Option<String> {
    values(headers, name).into_iter().next()
}

pub fn unsubscribe(headers: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    value(headers, "List-Unsubscribe")
        .map(|v| rx(&RE, r"<([^>]+)>").captures_iter(&v).map(|c| c[1].trim().to_string()).collect())
        .unwrap_or_default()
}

pub fn is_bulk(headers: &str) -> bool {
    let precedence = value(headers, "Precedence").map(|v| v.to_lowercase()).unwrap_or_default();
    let auto = value(headers, "Auto-Submitted").map(|v| v.to_lowercase()).unwrap_or_default();
    matches!(precedence.as_str(), "bulk" | "list" | "junk")
        || (!auto.is_empty() && auto != "no")
        || value(headers, "List-Unsubscribe").is_some()
        || value(headers, "List-Id").is_some()
        || value(headers, "X-Campaign").is_some()
}

pub fn is_public(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => !(v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_broadcast() || v4.is_unspecified() || v4.is_documentation() || (v4.octets()[0] == 100 && (64..=127).contains(&v4.octets()[1]))),
        IpAddr::V6(v6) => !(v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80 || v6.segments()[0] == 0x2001 && v6.segments()[1] == 0x0db8),
    }
}

fn ips_in(text: &str) -> Vec<IpAddr> {
    static RE: OnceLock<Regex> = OnceLock::new();
    rx(&RE, r"(?i)[\[(](?:IPv6:)?([0-9a-f:.]{3,45})[\])]")
        .captures_iter(text)
        .filter_map(|c| c[1].trim_end_matches('.').parse::<IpAddr>().ok())
        .collect()
}

pub fn received_hops(headers: &str) -> Vec<IpAddr> {
    static BY: OnceLock<Regex> = OnceLock::new();
    let by = rx(&BY, r"(?i)\sby\s");
    values(headers, "Received")
        .into_iter()
        .rev()
        .filter_map(|v| {
            let from = v.trim_start().strip_prefix("from").or_else(|| v.trim_start().strip_prefix("FROM"))?;
            let clause = by.split(from).next().unwrap_or(from).to_string();
            ips_in(&clause).into_iter().next()
        })
        .collect()
}

pub fn origin_ip(headers: &str) -> Option<IpAddr> {
    for name in ["X-Originating-IP", "X-Sender-IP", "X-Client-IP"] {
        if let Some(ip) = value(headers, name).and_then(|v| v.trim_matches(|c| c == '[' || c == ']' || c == ' ').parse::<IpAddr>().ok()) {
            if is_public(&ip) {
                return Some(ip);
            }
        }
    }
    received_hops(headers).into_iter().find(is_public)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(raw: &str) -> String {
        raw.split("\n\n").next().unwrap_or(raw).replace("\r\n\r\n", "").to_string()
    }

    #[test]
    fn unfolds_continuation_lines() {
        let h = "Subject: a\r\n b\r\nTo: x@y.no\r\n";
        assert_eq!(unfold(h), vec![("Subject".to_string(), "a b".to_string()), ("To".to_string(), "x@y.no".to_string())]);
    }

    #[test]
    fn origin_prefers_x_originating_ip() {
        let h = head(&crate::content::fixture("outlook-quote.eml"));
        assert_eq!(origin_ip(&h).unwrap().to_string(), "85.166.12.9");
        assert_eq!(received_hops(&h).iter().map(|ip| ip.to_string()).collect::<Vec<_>>(), vec!["40.107.21.75", "2603:10a6:208:12::21"]);
    }

    #[test]
    fn origin_skips_private_ipv4_and_ipv6_hops() {
        let plain = head(&crate::content::fixture("plain.eml"));
        assert_eq!(received_hops(&plain).iter().map(|ip| ip.to_string()).collect::<Vec<_>>(), vec!["192.168.1.44", "185.35.187.100"]);
        assert_eq!(origin_ip(&plain).unwrap().to_string(), "185.35.187.100");
        let apple = head(&crate::content::fixture("apple-quote.eml"));
        assert_eq!(received_hops(&apple)[0].to_string(), "fd00::1");
        assert_eq!(origin_ip(&apple).unwrap().to_string(), "17.57.154.21");
    }

    #[test]
    fn origin_handles_hops_without_from_and_trailing_dots() {
        let gmail = head(&crate::content::fixture("gmail-quote.eml"));
        assert_eq!(origin_ip(&gmail).unwrap().to_string(), "209.85.218.47");
    }

    #[test]
    fn origin_reads_bracketed_ipv6() {
        let h = "Received: from mail.example (mail.example [IPv6:2a01:4f8:c0c:1::2]) by mx.example; Mon, 5 Oct 2026 10:00:00 +0000\r\nReceived: from [10.0.0.4] by mail.example; Mon, 5 Oct 2026 09:59:00 +0000";
        assert_eq!(origin_ip(h).unwrap().to_string(), "2a01:4f8:c0c:1::2");
    }

    #[test]
    fn origin_is_none_when_every_hop_is_private() {
        let h = "X-Originating-IP: [10.1.1.1]\r\nReceived: from [192.168.0.2] by a; x\r\nReceived: from b (b [127.0.0.1]) by c; y";
        assert!(origin_ip(h).is_none());
    }

    #[test]
    fn detects_bulk_mail_and_unsubscribe_targets() {
        let news = head(&crate::content::fixture("newsletter.eml"));
        assert!(is_bulk(&news));
        assert_eq!(unsubscribe(&news), vec!["https://tldr.tech/unsubscribe?u=abc123", "mailto:unsub@tldrnewsletter.com?subject=unsubscribe"]);
        let plain = head(&crate::content::fixture("plain.eml"));
        assert!(!is_bulk(&plain));
        assert!(unsubscribe(&plain).is_empty());
    }

    #[test]
    fn analytics_uses_the_same_origin_function() {
        let h = head(&crate::content::fixture("outlook-quote.eml"));
        assert_eq!(crate::services::analytics_service::origin_ip_from_headers(&h), origin_ip(&h));
    }
}
