//! REST API middleware for cross-cutting concerns.
//!
//! This module provides:
//! - CORS configuration
//! - Rate limiting
//! - API key authentication
//! - Request logging

use std::{
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use tower_http::cors::{Any, CorsLayer};

use crate::{error::ErrorResponse, AppContext, Config};

/// Create CORS layer based on configuration.
pub fn cors_layer(config: &Config) -> CorsLayer {
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .max_age(Duration::from_secs(3600));

    if config.cors_origins.contains(&"*".to_string()) {
        cors.allow_origin(Any)
    } else {
        // Parse origins - in production, validate these
        cors.allow_origin(Any) // Simplified for now
    }
}

/// Rate limiter type alias.
pub type SharedRateLimiter = Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>;

/// Create a rate limiter with the configured limit.
pub fn create_rate_limiter(rps: u32) -> SharedRateLimiter {
    let quota = Quota::per_second(std::num::NonZeroU32::new(rps).unwrap());
    Arc::new(RateLimiter::direct(quota))
}

/// Rate limiting middleware.
pub async fn rate_limit_middleware(
    State(limiter): State<SharedRateLimiter>,
    request: Request<Body>,
    next: Next,
) -> Response {
    match limiter.check() {
        Ok(_) => next.run(request).await,
        Err(_) => {
            let body = ErrorResponse {
                error: "rate_limit_exceeded".into(),
                message: "Too many requests. Please slow down.".into(),
                details: None,
                request_id: None,
            };
            (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response()
        }
    }
}

/// API key authentication middleware.
pub async fn auth_middleware(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    request: Request<Body>,
    next: Next,
) -> Response {
    // If no API key configured, allow all requests
    let Some(expected_key) = &ctx.config.api_key else {
        return next.run(request).await;
    };

    // Check Authorization header
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(auth) if auth.starts_with("Bearer ") => {
            let provided_key = auth.trim_start_matches("Bearer ").trim();
            if provided_key == expected_key {
                next.run(request).await
            } else {
                unauthorized_response("Invalid API key")
            }
        }
        Some(_) => unauthorized_response("Invalid authorization format. Use 'Bearer <api_key>'"),
        None => unauthorized_response("Missing Authorization header"),
    }
}

fn unauthorized_response(message: &str) -> Response {
    let body = ErrorResponse {
        error: "unauthorized".into(),
        message: message.into(),
        details: None,
        request_id: None,
    };
    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}

/// Request logging middleware that adds structured logging.
pub async fn logging_middleware(
    headers: HeaderMap,
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let request_id = headers
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .map(String::from);

    let start = std::time::Instant::now();

    let response = next.run(request).await;

    let latency = start.elapsed();
    let status = response.status();

    tracing::info!(
        method = %method,
        uri = %uri,
        status = %status.as_u16(),
        latency_ms = %latency.as_millis(),
        request_id = ?request_id,
        "HTTP request"
    );

    response
}

/// Content type validation middleware for JSON endpoints.
pub async fn json_content_type_middleware(
    headers: HeaderMap,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Only check POST/PUT/PATCH requests
    if matches!(
        request.method().as_str(),
        "POST" | "PUT" | "PATCH"
    ) {
        // Skip multipart endpoints
        let path = request.uri().path();
        if path.contains("/recordings") {
            return next.run(request).await;
        }

        // Check content type
        let content_type = headers
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok());

        match content_type {
            Some(ct) if ct.contains("application/json") => next.run(request).await,
            Some(ct) => {
                let body = ErrorResponse {
                    error: "unsupported_media_type".into(),
                    message: format!("Expected application/json, got {}", ct),
                    details: None,
                    request_id: None,
                };
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, Json(body)).into_response()
            }
            None => {
                let body = ErrorResponse {
                    error: "unsupported_media_type".into(),
                    message: "Missing Content-Type header".into(),
                    details: None,
                    request_id: None,
                };
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, Json(body)).into_response()
            }
        }
    } else {
        next.run(request).await
    }
}

/// Request body size limit middleware.
pub struct BodyLimitMiddleware {
    max_size: usize,
}

impl BodyLimitMiddleware {
    pub fn new(max_size: usize) -> Self {
        Self { max_size }
    }
}

/// Extract client IP from request.
pub fn extract_client_ip(headers: &HeaderMap, connect_info: Option<&ConnectInfo<SocketAddr>>) -> Option<String> {
    // Try X-Forwarded-For first (for proxied requests)
    if let Some(forwarded) = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
    {
        // Take the first IP in the chain
        if let Some(ip) = forwarded.split(',').next() {
            return Some(ip.trim().to_string());
        }
    }

    // Try X-Real-IP
    if let Some(real_ip) = headers
        .get("x-real-ip")
        .and_then(|h| h.to_str().ok())
    {
        return Some(real_ip.to_string());
    }

    // Fall back to connection info
    connect_info.map(|ci| ci.0.ip().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cors_layer_creation() {
        let config = Config::default();
        let _layer = cors_layer(&config);
    }

    #[test]
    fn test_rate_limiter_creation() {
        let limiter = create_rate_limiter(100);
        assert!(limiter.check().is_ok());
    }

    #[test]
    fn test_extract_client_ip_x_forwarded() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4, 5.6.7.8".parse().unwrap());

        let ip = extract_client_ip(&headers, None);
        assert_eq!(ip, Some("1.2.3.4".to_string()));
    }

    #[test]
    fn test_extract_client_ip_x_real() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "10.0.0.1".parse().unwrap());

        let ip = extract_client_ip(&headers, None);
        assert_eq!(ip, Some("10.0.0.1".to_string()));
    }
}
