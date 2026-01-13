//! REST API endpoints for WiFi-DensePose MAT disaster response monitoring.
//!
//! This module provides a complete REST API and WebSocket interface for
//! managing disaster events, zones, survivors, and alerts in real-time.
//!
//! ## Endpoints
//!
//! ### Disaster Events
//! - `GET /api/v1/mat/events` - List all disaster events
//! - `POST /api/v1/mat/events` - Create new disaster event
//! - `GET /api/v1/mat/events/{id}` - Get event details
//!
//! ### Zones
//! - `GET /api/v1/mat/events/{id}/zones` - List zones for event
//! - `POST /api/v1/mat/events/{id}/zones` - Add zone to event
//!
//! ### Survivors
//! - `GET /api/v1/mat/events/{id}/survivors` - List survivors in event
//!
//! ### Alerts
//! - `GET /api/v1/mat/events/{id}/alerts` - List alerts for event
//! - `POST /api/v1/mat/alerts/{id}/acknowledge` - Acknowledge alert
//!
//! ### WebSocket
//! - `WS /ws/mat/stream` - Real-time survivor and alert stream

pub mod dto;
pub mod handlers;
pub mod error;
pub mod state;
pub mod websocket;

use axum::{
    Router,
    routing::{get, post},
};

pub use dto::*;
pub use error::ApiError;
pub use state::AppState;

/// Create the MAT API router with all endpoints.
///
/// # Example
///
/// ```rust,no_run
/// use wifi_densepose_mat::api::{create_router, AppState};
///
/// #[tokio::main]
/// async fn main() {
///     let state = AppState::new();
///     let app = create_router(state);
///     // ... serve with axum
/// }
/// ```
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Event endpoints
        .route("/api/v1/mat/events", get(handlers::list_events).post(handlers::create_event))
        .route("/api/v1/mat/events/:event_id", get(handlers::get_event))
        // Zone endpoints
        .route("/api/v1/mat/events/:event_id/zones", get(handlers::list_zones).post(handlers::add_zone))
        // Survivor endpoints
        .route("/api/v1/mat/events/:event_id/survivors", get(handlers::list_survivors))
        // Alert endpoints
        .route("/api/v1/mat/events/:event_id/alerts", get(handlers::list_alerts))
        .route("/api/v1/mat/alerts/:alert_id/acknowledge", post(handlers::acknowledge_alert))
        // WebSocket endpoint
        .route("/ws/mat/stream", get(websocket::ws_handler))
        .with_state(state)
}
