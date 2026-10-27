use crate::models::app_setting;
use reqwest::header::RETRY_AFTER;
use reqwest::{RequestBuilder, Response, StatusCode};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static MAX_SECONDS: AtomicU64 = AtomicU64::new(0);

const BACKOFF_BASE_MS: u64 = 500;

pub fn put_policy(attempts: u64, max_seconds: u64) {
    ATTEMPTS.store(attempts.max(1), Ordering::Relaxed);
    MAX_SECONDS.store(max_seconds, Ordering::Relaxed);
}

fn attempts() -> u64 {
    match ATTEMPTS.load(Ordering::Relaxed) {
        0 => app_setting::get_default_u64(app_setting::PROVIDER_RETRY_ATTEMPTS).max(1),
        n => n,
    }
}

fn max_wait() -> Duration {
    Duration::from_secs(match MAX_SECONDS.load(Ordering::Relaxed) {
        0 => app_setting::get_default_u64(app_setting::PROVIDER_RETRY_MAX_SECONDS),
        n => n,
    })
}

pub fn retry_wait(status: StatusCode, retry_after: Option<&str>, attempt: u32, max: Duration) -> Option<Duration> {
    if !matches!(status, StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT) {
        return None;
    }
    let told = retry_after.and_then(|v| v.trim().parse::<u64>().ok()).map(Duration::from_secs);
    let wait = told.unwrap_or_else(|| Duration::from_millis(BACKOFF_BASE_MS.saturating_mul(1u64 << attempt.min(10))));
    Some(wait.min(max))
}

pub async fn send(request: RequestBuilder) -> Result<Response, String> {
    let limit = attempts();
    let max = max_wait();
    let mut attempt: u32 = 0;
    loop {
        let builder = request.try_clone().ok_or_else(|| "The request cannot be repeated".to_string())?;
        let response = builder.send().await.map_err(|e| format!("Request failed: {}", e))?;
        let header = response.headers().get(RETRY_AFTER).and_then(|v| v.to_str().ok()).map(String::from);
        match retry_wait(response.status(), header.as_deref(), attempt, max) {
            Some(wait) if u64::from(attempt) + 1 < limit => {
                tokio::time::sleep(wait).await;
                attempt += 1;
            }
            _ => return Ok(response),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn waits_follow_retry_after_then_back_off_and_stop_at_the_cap() {
        let max = Duration::from_secs(60);
        assert_eq!(retry_wait(StatusCode::TOO_MANY_REQUESTS, Some("7"), 0, max), Some(Duration::from_secs(7)));
        assert_eq!(retry_wait(StatusCode::TOO_MANY_REQUESTS, Some("600"), 0, max), Some(max));
        assert_eq!(retry_wait(StatusCode::SERVICE_UNAVAILABLE, None, 0, max), Some(Duration::from_millis(500)));
        assert_eq!(retry_wait(StatusCode::SERVICE_UNAVAILABLE, None, 3, max), Some(Duration::from_millis(4000)));
        assert_eq!(retry_wait(StatusCode::OK, Some("1"), 0, max), None);
        assert_eq!(retry_wait(StatusCode::FORBIDDEN, None, 0, max), None);
    }

    #[tokio::test]
    async fn a_429_is_retried_after_the_server_says_and_then_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/limited"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
            .up_to_n_times(2)
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/limited"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server).await;
        let response = send(reqwest::Client::new().get(format!("{}/limited", server.uri()))).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(server.received_requests().await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn retries_give_up_after_the_attempts_and_hand_back_the_429() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/always"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
            .mount(&server).await;
        let response = send(reqwest::Client::new().get(format!("{}/always", server.uri()))).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(server.received_requests().await.unwrap().len() as u64, attempts());
    }
}
