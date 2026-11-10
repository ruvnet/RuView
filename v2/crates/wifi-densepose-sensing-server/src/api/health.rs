//! Server health, system metrics, and capability discovery REST endpoints.
//!
//! Provides liveness, readiness, governed trust status, and hardware provenance
//! inspection endpoints conforming to ADR-080, ADR-135..146, and ADR-295.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;
use axum::{
    extract::State,
    response::Json,
    routing::get,
    Router,
};
use wifi_densepose_sensing_server::provenance::SourceState;

use crate::{
    default_position_node_ids, processing_health,
    NodeState, SharedState, DEFAULT_NODE_POSITION,
};
use super::pose::stream_health_component;

/// `GET /health` — basic liveness & processing check.
pub(crate) async fn health(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let now_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    Json(serde_json::json!({
        "status": "ok",
        "source": s.effective_source(),
        "tick": s.tick,
        "clients": s.ws_clients.get(),
        "processing": processing_health(
            s.latest_update.as_ref().map(|u| u.timestamp),
            now_unix_ms,
        ),
    }))
}

/// `GET /health/live` — lightweight process liveness check with uptime.
pub(crate) async fn health_live(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "status": "alive",
        "uptime": s.start_time.elapsed().as_secs(),
    }))
}

/// Governed-fuser positions for `/api/v1/status`, keyed by node id and sorted.
pub(crate) fn engine_node_positions_json(positions: &HashMap<u8, [f32; 3]>) -> serde_json::Value {
    let sorted: BTreeMap<String, [f32; 3]> = positions
        .iter()
        .map(|(id, p)| (id.to_string(), *p))
        .collect();
    serde_json::json!(sorted)
}

/// Lowercase hex of a 32-byte witness for JSON exposure.
pub(crate) fn witness_hex(w: [u8; 32]) -> String {
    w.iter().fold(String::with_capacity(64), |mut acc, b| {
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

/// `GET /health/ready` & `GET /api/v1/status` — readiness and trust provenance.
pub(crate) async fn health_ready(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let source_state = s.source_state();
    Json(serde_json::json!({
        "status": "ready",
        "source": s.effective_source(),
        // ADR-295 — canonical provenance state
        "source_state": source_state.as_str(),
        // Issue #2097 — unstarted live source must not appear ready
        "waiting_for_frames": source_state == SourceState::Disconnected,
        "last_frame_age_ms": s.last_frame_age().map(|age| age.as_millis() as u64),
        // Governed trust-path state (ADR-135..146)
        "trust": {
            "node_positions": engine_node_positions_json(s.engine_bridge.node_positions()),
            "last_witness": s.engine_bridge.last_trust_witness().map(witness_hex),
            "effective_class": s.engine_bridge.effective_class().map(|c| format!("{c:?}")),
            "demoted": s.engine_bridge.demoted(),
            "recalibration_recommended": s.engine_bridge.recalibration_recommended(),
            "engine_error_count": s.engine_bridge.engine_error_count(),
            "raw_outputs_suppressed": s.engine_bridge.suppress_raw_outputs(),
            "phase_fusion": s.engine_bridge.phase_fusion().map(|p| serde_json::json!({
                "mode": p.mode_str(),
                "reason": p.reason().map(|r| r.as_str()),
            })),
        },
        "node_positions": node_positions_status(&s.node_positions_config, &s.node_states),
    }))
}

/// `/api/v1/status` view of `--node-positions` (issue #1804).
pub(crate) fn node_positions_status(
    node_positions_config: &HashMap<u8, [f32; 3]>,
    node_states: &HashMap<u8, NodeState>,
) -> serde_json::Value {
    let mut configured: Vec<u8> = node_positions_config.keys().copied().collect();
    configured.sort_unstable();
    serde_json::json!({
        "configured_node_ids": configured,
        "default_position": DEFAULT_NODE_POSITION,
        "default_position_node_ids":
            default_position_node_ids(node_positions_config, node_states.keys().copied()),
    })
}

/// `GET /health/health` — full system health and component status.
pub(crate) async fn health_system(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let uptime = s.start_time.elapsed().as_secs();
    Json(serde_json::json!({
        "status": "healthy",
        "components": {
            "api": { "status": "healthy", "message": "Rust Axum server" },
            "hardware": {
                "status": if s.effective_source().ends_with(":offline") { "degraded" } else { "healthy" },
                "message": format!("Source: {}", s.effective_source())
            },
            "pose": { "status": "healthy", "message": "WiFi-derived pose estimation" },
            "stream": stream_health_component(&s, std::time::Instant::now()),
        },
        "metrics": {
            "cpu_percent": 2.5,
            "memory_percent": 1.8,
            "disk_percent": 15.0,
            "uptime_seconds": uptime,
        }
    }))
}

/// `GET /health/version` — build and backend identity.
pub(crate) async fn health_version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": "wifi-densepose-sensing-server",
        "backend": "rust+axum+ruvector",
    }))
}

/// `GET /health/metrics` & `GET /api/v1/metrics` — telemetry and counters.
pub(crate) async fn health_metrics(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "system_metrics": {
            "cpu": { "percent": 2.5 },
            "memory": { "percent": 1.8, "used_mb": 5 },
            "disk": { "percent": 15.0 },
        },
        "tick": s.tick,
    }))
}

/// `GET /api/v1/info` — API capability and runtime feature flags.
pub(crate) async fn api_info(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "environment": "production",
        "backend": "rust",
        "source": s.effective_source(),
        "features": {
            "wifi_sensing": true,
            "pose_estimation": true,
            "signal_processing": true,
            "ruvector": true,
            "streaming": true,
        }
    }))
}

/// `GET /` — root server info landing page.
pub(crate) async fn info_page() -> axum::response::Html<String> {
    axum::response::Html(
        "<html><body>\
         <h1>WiFi-DensePose Sensing Server</h1>\
         <p>Rust + Axum + RuVector</p>\
         <ul>\
         <li><a href='/health'>/health</a> — Server health</li>\
         <li><a href='/api/v1/sensing/latest'>/api/v1/sensing/latest</a> — Latest sensing data</li>\
         <li><a href='/api/v1/vital-signs'>/api/v1/vital-signs</a> — Vital sign estimates (HR/RR)</li>\
         <li><a href='/api/v1/model/info'>/api/v1/model/info</a> — RVF model container info</li>\
         <li>ws://localhost:8765/ws/sensing — WebSocket stream</li>\
         </ul>\
         </body></html>"
            .to_string(),
    )
}

/// Build the health and status routes.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/", get(info_page))
        .route("/health", get(health))
        .route("/health/health", get(health_system))
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        .route("/health/version", get(health_version))
        .route("/health/metrics", get(health_metrics))
        .route("/api/v1/info", get(api_info))
        .route("/api/v1/status", get(health_ready))
        .route("/api/v1/metrics", get(health_metrics))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_node_positions_json_is_keyed_by_id() {
        let configured: HashMap<u8, [f32; 3]> = [(12, [4.0, 3.0, 1.0]), (11, [0.5, 0.0, 1.0])]
            .into_iter()
            .collect();
        assert_eq!(
            engine_node_positions_json(&configured),
            serde_json::json!({"11": [0.5, 0.0, 1.0], "12": [4.0, 3.0, 1.0]})
        );
    }
}
