use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use nonzero_ext::nonzero;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::{debug, warn};

use super::{responses::ErrorResponse, state::AppState};

/// Authentication middleware
/// Validates app_id and app_key from headers or query parameters
pub async fn auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, ErrorResponse> {
    // Check if authentication is enabled
    if !state.auth_enabled {
        debug!("Authentication disabled, allowing request");
        return Ok(next.run(request).await);
    }

    // Extract credentials from headers
    let app_id = headers
        .get("app_id")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            // Fallback to query parameters
            request
                .uri()
                .query()
                .and_then(|q| extract_query_param(q, "app_id"))
        });

    let app_key = headers
        .get("app_key")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            request
                .uri()
                .query()
                .and_then(|q| extract_query_param(q, "app_key"))
        });

    // Validate credentials
    match (app_id, app_key) {
        (Some(id), Some(key)) => {
            if validate_credentials(&state, id, key).await {
                debug!("Authentication successful for app_id: {}", id);
                Ok(next.run(request).await)
            } else {
                warn!("Invalid credentials for app_id: {}", id);
                Err(ErrorResponse::unauthorized("Invalid credentials"))
            }
        }
        _ => {
            warn!("Missing authentication credentials");
            Err(ErrorResponse::unauthorized("Missing app_id or app_key"))
        }
    }
}

/// Rate limiting middleware using token bucket algorithm
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ErrorResponse> {
    // Check rate limit
    match state.rate_limiter.check() {
        Ok(_) => {
            debug!("Rate limit check passed");
            Ok(next.run(request).await)
        }
        Err(_) => {
            warn!("Rate limit exceeded");
            Err(ErrorResponse::rate_limited(
                "Rate limit exceeded. Please try again later.",
            ))
        }
    }
}

/// Validate app credentials using secure comparison
///
/// SECURITY: This implementation:
/// 1. Requires credentials to be pre-configured in AppState
/// 2. Uses constant-time comparison to prevent timing attacks
/// 3. Hashes the key before comparison
async fn validate_credentials(state: &AppState, app_id: &str, app_key: &str) -> bool {
    // Reject empty credentials
    if app_id.is_empty() || app_key.is_empty() {
        return false;
    }

    // Get configured credentials from state
    let Some(expected_key_hash) = state.api_keys.get(app_id) else {
        warn!("Unknown app_id attempted authentication: {}", app_id);
        return false;
    };

    // Hash the provided key
    let provided_key_hash = hash_api_key(app_key);

    // Constant-time comparison to prevent timing attacks
    constant_time_compare(&provided_key_hash, expected_key_hash.as_str())
}

/// Hash an API key using SHA-256
fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Constant-time string comparison to prevent timing attacks
fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}

/// Extract query parameter from query string
fn extract_query_param<'a>(query: &'a str, param: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let mut parts = pair.split('=');
        match (parts.next(), parts.next()) {
            (Some(k), Some(v)) if k == param => Some(v),
            _ => None,
        }
    })
}

/// Create a rate limiter with token bucket algorithm
pub fn create_rate_limiter() -> Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>> {
    // Allow 100 requests per minute
    let quota = Quota::per_minute(nonzero!(100u32));
    Arc::new(RateLimiter::direct(quota))
}

/// Type alias for rate limiter
pub type AppRateLimiter = Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_query_param() {
        let query = "app_id=123&app_key=secret&foo=bar";
        assert_eq!(extract_query_param(query, "app_id"), Some("123"));
        assert_eq!(extract_query_param(query, "app_key"), Some("secret"));
        assert_eq!(extract_query_param(query, "foo"), Some("bar"));
        assert_eq!(extract_query_param(query, "missing"), None);
    }

    #[test]
    fn test_hash_api_key() {
        let key = "test_key_123";
        let hash1 = hash_api_key(key);
        let hash2 = hash_api_key(key);
        assert_eq!(hash1, hash2);
        assert_ne!(hash_api_key("different"), hash1);
    }

    #[test]
    fn test_constant_time_compare() {
        assert!(constant_time_compare("abc", "abc"));
        assert!(!constant_time_compare("abc", "abd"));
        assert!(!constant_time_compare("abc", "ab"));
        assert!(!constant_time_compare("", "a"));
    }

    #[tokio::test]
    async fn test_validate_credentials_rejects_empty() {
        let state = AppState::new();
        assert!(!validate_credentials(&state, "", "key").await);
        assert!(!validate_credentials(&state, "test", "").await);
        assert!(!validate_credentials(&state, "", "").await);
    }
}
