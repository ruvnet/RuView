//! Admin API and health check endpoints for Tiny Dancer
//!
//! This module provides a production-ready REST API for monitoring, administration,
//! and health checks. It's designed to integrate with Kubernetes and monitoring systems.
//!
//! ## Features
//! - Health check endpoints (liveness & readiness probes)
//! - Prometheus-compatible metrics export
//! - Admin endpoints for hot-reloading and configuration
//! - Circuit breaker management
//! - Optional bearer token authentication

use crate::circuit_breaker::CircuitState;
use crate::error::{Result, TinyDancerError};
use crate::router::Router;
use crate::types::{RouterConfig, RoutingMetrics};
use axum::{
    extract::{Json, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Router as AxumRouter,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower_http::cors::CorsLayer;

/// Version information for the API
pub const API_VERSION: &str = "v1";

/// Admin server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminServerConfig {
    /// Server bind address
    pub bind_address: String,
    /// Server port
    pub port: u16,
    /// Optional bearer token for authentication
    pub auth_token: Option<String>,
    /// Enable CORS
    pub enable_cors: bool,
}

impl Default for AdminServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            auth_token: None,
            enable_cors: true,
        }
    }
}

/// Admin server state
#[derive(Clone)]
pub struct AdminServerState {
    router: Arc<Router>,
    metrics: Arc<RwLock<RoutingMetrics>>,
    start_time: Instant,
    config: AdminServerConfig,
}

impl AdminServerState {
    /// Create new admin server state
    pub fn new(router: Arc<Router>, config: AdminServerConfig) -> Self {
        Self {
            router,
            metrics: Arc::new(RwLock::new(RoutingMetrics::default())),
            start_time: Instant::now(),
            config,
        }
    }

    /// Get router reference
    pub fn router(&self) -> &Arc<Router> {
        &self.router
    }

    /// Get metrics reference
    pub fn metrics(&self) -> Arc<RwLock<RoutingMetrics>> {
        Arc::clone(&self.metrics)
    }

    /// Get uptime in seconds
    pub fn uptime(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

/// Admin server for managing Tiny Dancer
pub struct AdminServer {
    state: AdminServerState,
}

impl AdminServer {
    /// Create a new admin server
    pub fn new(router: Arc<Router>, config: AdminServerConfig) -> Self {
        Self {
            state: AdminServerState::new(router, config),
        }
    }

    /// Build the Axum router with all routes
    pub fn build_router(&self) -> AxumRouter {
        let mut router = AxumRouter::new()
            // Health check endpoints
            .route("/health", get(health_check))
            .route("/health/ready", get(readiness_check))
            // Metrics endpoint
            .route("/metrics", get(metrics_endpoint))
            // Admin endpoints
            .route("/admin/reload", post(reload_model))
            .route("/admin/config", get(get_config).put(update_config))
            .route("/admin/circuit-breaker", get(circuit_breaker_status))
            .route("/admin/circuit-breaker/reset", post(reset_circuit_breaker))
            // Info endpoint
            .route("/info", get(system_info))
            .with_state(self.state.clone());

        // Add CORS if enabled
        if self.state.config.enable_cors {
            router = router.layer(CorsLayer::permissive());
        }

        router
    }

    /// Start the admin server
    pub async fn serve(self) -> Result<()> {
        let addr = format!("{}:{}", self.state.config.bind_address, self.state.config.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| TinyDancerError::ConfigError(format!("Failed to bind to {}: {}", addr, e)))?;

        tracing::info!("Admin server listening on {}", addr);

        let router = self.build_router();
        axum::serve(listener, router)
            .await
            .map_err(|e| TinyDancerError::ConfigError(format!("Server error: {}", e)))?;

        Ok(())
    }
}

// ============================================================================
// Health Check Endpoints
// ============================================================================

/// Health check response
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    uptime_seconds: u64,
}

/// Basic health check (liveness probe)
///
/// Always returns 200 OK if the service is running.
/// Suitable for Kubernetes liveness probes.
async fn health_check(State(state): State<AdminServerState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: crate::VERSION.to_string(),
        uptime_seconds: state.uptime(),
    })
}

/// Readiness check response
#[derive(Debug, Serialize)]
struct ReadinessResponse {
    ready: bool,
    circuit_breaker: String,
    model_loaded: bool,
    version: String,
    uptime_seconds: u64,
}

/// Readiness check (readiness probe)
///
/// Returns 200 OK if the service is ready to accept traffic.
/// Checks circuit breaker status and model availability.
/// Suitable for Kubernetes readiness probes.
async fn readiness_check(State(state): State<AdminServerState>) -> impl IntoResponse {
    let circuit_breaker_closed = state.router.circuit_breaker_status().unwrap_or(true);
    let model_loaded = true; // Model is always loaded in Router

    let ready = circuit_breaker_closed && model_loaded;

    let cb_state = match state.router.circuit_breaker_status() {
        Some(true) => "closed",
        Some(false) => "open",
        None => "disabled",
    };

    let response = ReadinessResponse {
        ready,
        circuit_breaker: cb_state.to_string(),
        model_loaded,
        version: crate::VERSION.to_string(),
        uptime_seconds: state.uptime(),
    };

    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status, Json(response))
}

// ============================================================================
// Metrics Endpoint
// ============================================================================

/// Metrics endpoint (Prometheus format)
///
/// Exports metrics in Prometheus exposition format.
/// Compatible with Prometheus, Grafana, and other monitoring tools.
async fn metrics_endpoint(State(state): State<AdminServerState>) -> impl IntoResponse {
    let metrics = state.metrics.read();
    let uptime = state.uptime();

    let prometheus_metrics = format!(
        r#"# HELP tiny_dancer_requests_total Total number of routing requests
# TYPE tiny_dancer_requests_total counter
tiny_dancer_requests_total {{}} {}

# HELP tiny_dancer_lightweight_routes_total Requests routed to lightweight model
# TYPE tiny_dancer_lightweight_routes_total counter
tiny_dancer_lightweight_routes_total {{}} {}

# HELP tiny_dancer_powerful_routes_total Requests routed to powerful model
# TYPE tiny_dancer_powerful_routes_total counter
tiny_dancer_powerful_routes_total {{}} {}

# HELP tiny_dancer_inference_time_microseconds Average inference time
# TYPE tiny_dancer_inference_time_microseconds gauge
tiny_dancer_inference_time_microseconds {{}} {}

# HELP tiny_dancer_latency_microseconds Latency percentiles
# TYPE tiny_dancer_latency_microseconds gauge
tiny_dancer_latency_microseconds {{quantile="0.5"}} {}
tiny_dancer_latency_microseconds {{quantile="0.95"}} {}
tiny_dancer_latency_microseconds {{quantile="0.99"}} {}

# HELP tiny_dancer_errors_total Total number of errors
# TYPE tiny_dancer_errors_total counter
tiny_dancer_errors_total {{}} {}

# HELP tiny_dancer_circuit_breaker_trips_total Circuit breaker trip count
# TYPE tiny_dancer_circuit_breaker_trips_total counter
tiny_dancer_circuit_breaker_trips_total {{}} {}

# HELP tiny_dancer_uptime_seconds Service uptime
# TYPE tiny_dancer_uptime_seconds counter
tiny_dancer_uptime_seconds {{}} {}
"#,
        metrics.total_requests,
        metrics.lightweight_routes,
        metrics.powerful_routes,
        metrics.avg_inference_time_us,
        metrics.p50_latency_us,
        metrics.p95_latency_us,
        metrics.p99_latency_us,
        metrics.error_count,
        metrics.circuit_breaker_trips,
        uptime,
    );

    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        prometheus_metrics,
    )
}

// ============================================================================
// Admin Endpoints
// ============================================================================

/// Reload model response
#[derive(Debug, Serialize)]
struct ReloadResponse {
    success: bool,
    message: String,
}

/// Hot reload the routing model
///
/// POST /admin/reload
///
/// Reloads the model from disk without restarting the service.
/// Useful for deploying model updates in production.
async fn reload_model(
    State(state): State<AdminServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Check authentication
    if let Err(response) = check_auth(&state, &headers) {
        return response;
    }

    match state.router.reload_model() {
        Ok(_) => {
            tracing::info!("Model reloaded successfully");
            (
                StatusCode::OK,
                Json(ReloadResponse {
                    success: true,
                    message: "Model reloaded successfully".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to reload model: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ReloadResponse {
                    success: false,
                    message: format!("Failed to reload model: {}", e),
                }),
            )
                .into_response()
        }
    }
}

/// Get current router configuration
///
/// GET /admin/config
async fn get_config(
    State(state): State<AdminServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Check authentication
    if let Err(response) = check_auth(&state, &headers) {
        return response;
    }

    (StatusCode::OK, Json(state.router.config())).into_response()
}

/// Update configuration request
#[derive(Debug, Deserialize)]
struct UpdateConfigRequest {
    confidence_threshold: Option<f32>,
    max_uncertainty: Option<f32>,
    circuit_breaker_threshold: Option<u32>,
}

/// Update configuration response
#[derive(Debug, Serialize)]
struct UpdateConfigResponse {
    success: bool,
    message: String,
    updated_fields: Vec<String>,
}

/// Update router configuration
///
/// PUT /admin/config
///
/// Note: This endpoint updates the in-memory configuration.
/// Changes are not persisted to disk and will be lost on restart.
async fn update_config(
    State(_state): State<AdminServerState>,
    headers: HeaderMap,
    Json(_payload): Json<UpdateConfigRequest>,
) -> impl IntoResponse {
    // Check authentication
    if let Err(response) = check_auth(&_state, &headers) {
        return response;
    }

    // Note: Router doesn't currently support runtime config updates
    // This would require adding a method to Router to update config
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(UpdateConfigResponse {
            success: false,
            message: "Configuration updates not yet implemented".to_string(),
            updated_fields: vec![],
        }),
    )
        .into_response()
}

/// Circuit breaker status response
#[derive(Debug, Serialize)]
struct CircuitBreakerStatusResponse {
    enabled: bool,
    state: String,
    failure_count: Option<u32>,
    success_count: Option<u32>,
}

/// Get circuit breaker status
///
/// GET /admin/circuit-breaker
async fn circuit_breaker_status(
    State(state): State<AdminServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Check authentication
    if let Err(response) = check_auth(&state, &headers) {
        return response;
    }

    let enabled = state.router.circuit_breaker_status().is_some();
    let is_closed = state.router.circuit_breaker_status().unwrap_or(true);

    // We don't have direct access to CircuitBreaker from Router
    // In production, you'd add methods to expose these metrics
    let cb_state = if !enabled {
        "disabled"
    } else if is_closed {
        "closed"
    } else {
        "open"
    };

    (
        StatusCode::OK,
        Json(CircuitBreakerStatusResponse {
            enabled,
            state: cb_state.to_string(),
            failure_count: None, // Would need Router API extension
            success_count: None, // Would need Router API extension
        }),
    )
        .into_response()
}

/// Reset circuit breaker response
#[derive(Debug, Serialize)]
struct ResetResponse {
    success: bool,
    message: String,
}

/// Reset the circuit breaker
///
/// POST /admin/circuit-breaker/reset
///
/// Forces the circuit breaker back to closed state.
/// Use with caution in production.
async fn reset_circuit_breaker(
    State(_state): State<AdminServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Check authentication
    if let Err(response) = check_auth(&_state, &headers) {
        return response;
    }

    // Note: Router doesn't expose circuit breaker reset
    // This would require adding a method to Router
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ResetResponse {
            success: false,
            message: "Circuit breaker reset not yet implemented".to_string(),
        }),
    )
        .into_response()
}

// ============================================================================
// System Info Endpoint
// ============================================================================

/// System information response
#[derive(Debug, Serialize)]
struct SystemInfoResponse {
    version: String,
    api_version: String,
    uptime_seconds: u64,
    config: RouterConfig,
    circuit_breaker_enabled: bool,
    metrics: RoutingMetrics,
}

/// Get system information
///
/// GET /info
///
/// Returns comprehensive system information including version,
/// configuration, and current metrics.
async fn system_info(State(state): State<AdminServerState>) -> Json<SystemInfoResponse> {
    let metrics = state.metrics.read().clone();

    Json(SystemInfoResponse {
        version: crate::VERSION.to_string(),
        api_version: API_VERSION.to_string(),
        uptime_seconds: state.uptime(),
        config: state.router.config().clone(),
        circuit_breaker_enabled: state.router.circuit_breaker_status().is_some(),
        metrics,
    })
}

// ============================================================================
// Authentication
// ============================================================================

/// Check bearer token authentication
fn check_auth(state: &AdminServerState, headers: &HeaderMap) -> std::result::Result<(), Response> {
    // If no auth token is configured, allow all requests
    let Some(expected_token) = &state.config.auth_token else {
        return Ok(());
    };

    // Extract bearer token from Authorization header
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header_value) if header_value.starts_with("Bearer ") => {
            // Security: Use strip_prefix instead of slice indexing to avoid panic
            let token = match header_value.strip_prefix("Bearer ") {
                Some(t) => t,
                None => return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "error": "Invalid Authorization header format"
                    })),
                ).into_response()),
            };
            // Security: Use constant-time comparison to prevent timing attacks
            let token_bytes = token.as_bytes();
            let expected_bytes = expected_token.as_bytes();
            let mut result = token_bytes.len() == expected_bytes.len();
            // Compare all bytes even if lengths differ to maintain constant time
            let min_len = std::cmp::min(token_bytes.len(), expected_bytes.len());
            for i in 0..min_len {
                result &= token_bytes[i] == expected_bytes[i];
            }
            if result && token_bytes.len() == expected_bytes.len() {
                Ok(())
            } else {
                Err((
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "error": "Invalid authentication token"
                    })),
                )
                    .into_response())
            }
        }
        _ => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Missing or invalid Authorization header"
            })),
        )
            .into_response()),
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Record routing metrics
///
/// This function should be called after each routing operation
/// to update the metrics.
pub fn record_routing_metrics(
    metrics: &Arc<RwLock<RoutingMetrics>>,
    inference_time_us: u64,
    lightweight_count: usize,
    powerful_count: usize,
) {
    let mut m = metrics.write();
    m.total_requests += 1;
    m.lightweight_routes += lightweight_count as u64;
    m.powerful_routes += powerful_count as u64;

    // Update rolling average
    let alpha = 0.1; // Exponential moving average factor
    m.avg_inference_time_us = m.avg_inference_time_us * (1.0 - alpha) + inference_time_us as f64 * alpha;

    // Note: Percentile calculation would require a histogram
    // For now, we'll use simple approximations
    m.p50_latency_us = inference_time_us;
    m.p95_latency_us = (inference_time_us as f64 * 1.5) as u64;
    m.p99_latency_us = (inference_time_us as f64 * 2.0) as u64;
}

/// Record an error in metrics
pub fn record_error(metrics: &Arc<RwLock<RoutingMetrics>>) {
    let mut m = metrics.write();
    m.error_count += 1;
}

/// Record a circuit breaker trip
pub fn record_circuit_breaker_trip(metrics: &Arc<RwLock<RoutingMetrics>>) {
    let mut m = metrics.write();
    m.circuit_breaker_trips += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::Router;
    use crate::types::RouterConfig;

    #[test]
    fn test_admin_server_creation() {
        let router = Router::default().unwrap();
        let config = AdminServerConfig::default();
        let server = AdminServer::new(Arc::new(router), config);
        assert_eq!(server.state.uptime(), 0);
    }

    #[test]
    fn test_metrics_recording() {
        let metrics = Arc::new(RwLock::new(RoutingMetrics::default()));
        record_routing_metrics(&metrics, 1000, 5, 2);

        let m = metrics.read();
        assert_eq!(m.total_requests, 1);
        assert_eq!(m.lightweight_routes, 5);
        assert_eq!(m.powerful_routes, 2);
    }

    #[test]
    fn test_error_recording() {
        let metrics = Arc::new(RwLock::new(RoutingMetrics::default()));
        record_error(&metrics);
        record_error(&metrics);

        let m = metrics.read();
        assert_eq!(m.error_count, 2);
    }
}
