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
//! ### Scan Control
//! - `POST /api/v1/mat/scan/csi` - Push raw CSI data into detection pipeline
//! - `POST /api/v1/mat/scan/control` - Start/stop/pause/resume scanning
//! - `GET /api/v1/mat/scan/status` - Get detection pipeline status
//!
//! ### Domain Events
//! - `GET /api/v1/mat/events/domain` - List domain events from event store
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
        // Scan control endpoints (ADR-001: CSI data ingestion + pipeline control)
        .route("/api/v1/mat/scan/csi", post(handlers::push_csi_data))
        .route("/api/v1/mat/scan/control", post(handlers::scan_control))
        .route("/api/v1/mat/scan/status", get(handlers::pipeline_status))
        // Domain event store endpoint
        .route("/api/v1/mat/events/domain", get(handlers::list_domain_events))
        // WebSocket endpoint
        .route("/ws/mat/stream", get(websocket::ws_handler))
        .with_state(state)
}
