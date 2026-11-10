//! HTTP and WebSocket router builders.
//!
//! Complies with ELMAS.md: <= 350 lines per file, strictly modular.

use std::path::Path;
use std::sync::Arc;
use axum::http::HeaderValue;
use axum::routing::get;
use axum::{Extension, Router};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

use wifi_densepose_sensing_server::bearer_auth::{require_bearer, AuthState};
use wifi_densepose_sensing_server::edge_registry::EdgeRegistry;
use wifi_densepose_sensing_server::host_validation::{require_allowed_host, HostAllowlist};
use wifi_densepose_sensing_server::privacy_filter::{redact_json_responses, PrivacyFilter};
use wifi_densepose_sensing_server::rufield_surface::{self, FieldState};

use crate::api::stream::ws_sensing_handler;
use crate::api;
use crate::training_api;
use crate::SharedState;

/// Build the dedicated WebSocket router on port 8765.
pub fn build_ws_router(
    state: SharedState,
    field_surface: FieldState,
    privacy_mode: bool,
    bearer_auth: AuthState,
    host_allowlist: HostAllowlist,
) -> Router {
    Router::new()
        .route("/ws/sensing", get(ws_sensing_handler))
        .route("/health", get(api::health::health))
        .with_state(state)
        .merge(rufield_surface::router(field_surface))
        .layer(axum::middleware::from_fn_with_state(
            PrivacyFilter::new(privacy_mode),
            redact_json_responses,
        ))
        .layer(axum::middleware::from_fn_with_state(
            bearer_auth,
            require_bearer,
        ))
        .layer(axum::middleware::from_fn_with_state(
            host_allowlist,
            require_allowed_host,
        ))
}

/// Build the primary HTTP router serving UI, REST APIs, and WebSockets.
pub fn build_http_router(
    state: SharedState,
    field_surface: FieldState,
    ui_path: &Path,
    edge_registry: Option<Arc<EdgeRegistry>>,
    privacy_mode: bool,
    bearer_auth: AuthState,
    host_allowlist: HostAllowlist,
) -> Router {
    Router::new()
        .merge(api::routes())
        .merge(training_api::routes())
        .nest_service("/ui", ServeDir::new(ui_path))
        .layer(Extension(edge_registry))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache, no-store, must-revalidate"),
        ))
        .layer(Extension(bearer_auth.clone()))
        .with_state(state)
        .merge(rufield_surface::router(field_surface))
        .layer(axum::middleware::from_fn_with_state(
            PrivacyFilter::new(privacy_mode),
            redact_json_responses,
        ))
        .layer(axum::middleware::from_fn_with_state(
            bearer_auth,
            require_bearer,
        ))
        .layer(axum::middleware::from_fn_with_state(
            host_allowlist,
            require_allowed_host,
        ))
}
