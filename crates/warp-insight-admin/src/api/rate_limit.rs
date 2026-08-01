use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::{
    extract::connect_info::ConnectInfo,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

use super::ApiState;

const MAX_FAILURES_PER_WINDOW: u32 = 5;
const FAILURE_WINDOW: Duration = Duration::from_secs(60);
const BLOCK_DURATION: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub struct RateLimitState {
    buckets: HashMap<String, RateLimitBucket>,
}

#[derive(Debug)]
struct RateLimitBucket {
    failures: u32,
    first_failure_at: Instant,
    blocked_until: Option<Instant>,
}

/// Derive a stable per-client bucket key from the peer socket address injected by
/// [`axum::extract::connect_info::ConnectInfo`]. Client-supplied headers such as
/// `x-real-ip` / `x-forwarded-for` are intentionally ignored (they are spoofable).
/// Falls back to a single shared bucket when no connection info is available (e.g. tests).
pub fn client_key(client: Option<ConnectInfo<SocketAddr>>) -> String {
    client
        .map(|ConnectInfo(addr)| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn check_rate_limit(state: &ApiState, client_key: &str, scope: &str) -> Option<Response> {
    let key = rate_limit_key(client_key, scope);
    let now = Instant::now();
    let mut limits = state.rate_limits.lock().ok()?;
    let bucket = limits.buckets.get_mut(&key)?;
    if now.duration_since(bucket.first_failure_at) > FAILURE_WINDOW {
        limits.buckets.remove(&key);
        return None;
    }
    if let Some(blocked_until) = bucket.blocked_until {
        if blocked_until > now {
            let retry_after = blocked_until
                .duration_since(now)
                .as_secs()
                .max(1)
                .to_string();
            return Some(
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    [
                        (header::RETRY_AFTER, retry_after),
                        (header::CACHE_CONTROL, "no-store".to_string()),
                    ],
                    "too many failed authentication attempts",
                )
                    .into_response(),
            );
        }
        limits.buckets.remove(&key);
    }
    None
}

pub fn record_auth_failure(state: &ApiState, client_key: &str, scope: &str) {
    let key = rate_limit_key(client_key, scope);
    let now = Instant::now();
    let Ok(mut limits) = state.rate_limits.lock() else {
        return;
    };
    let bucket = limits.buckets.entry(key).or_insert(RateLimitBucket {
        failures: 0,
        first_failure_at: now,
        blocked_until: None,
    });
    if now.duration_since(bucket.first_failure_at) > FAILURE_WINDOW {
        bucket.failures = 0;
        bucket.first_failure_at = now;
        bucket.blocked_until = None;
    }
    bucket.failures = bucket.failures.saturating_add(1);
    if bucket.failures >= MAX_FAILURES_PER_WINDOW {
        bucket.blocked_until = Some(now + BLOCK_DURATION);
    }
}

pub fn clear_auth_failures(state: &ApiState, client_key: &str, scope: &str) {
    let key = rate_limit_key(client_key, scope);
    let Ok(mut limits) = state.rate_limits.lock() else {
        return;
    };
    limits.buckets.remove(&key);
}

fn rate_limit_key(client_key: &str, scope: &str) -> String {
    format!("{scope}:{client_key}")
}
