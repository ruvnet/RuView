//! Vital signs and edge runtime telemetry REST endpoints.
//!
//! Exposes breathing/heart rate estimates with strict calibration checks (ADR-039),
//! Edge Module Registry catalog (ADR-102), and edge WASM execution events (ADR-040).

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Extension, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use wifi_densepose_sensing_server::error_response;

use crate::{
    calibrated_vitals_for_publication, SharedState,
};

/// `GET /api/v1/vital-signs` — published breathing & heart rate estimates.
pub(crate) async fn vital_signs_endpoint(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let person_count = s.person_count_at(observed_at_unix_ms);
    let published = calibrated_vitals_for_publication(
        &s,
        &s.latest_vitals,
        person_count,
        observed_at_unix_ms,
    );
    let (br_len, br_cap, hb_len, hb_cap) = s.vital_detector.buffer_status();
    Json(serde_json::json!({
        "vital_signs": {
            "breathing_rate_bpm": published.as_ref().and_then(|value| value.breathing_rate_bpm),
            "heart_rate_bpm": published.as_ref().and_then(|value| value.heart_rate_bpm),
            "breathing_confidence": published.as_ref().map(|value| value.breathing_confidence),
            "heartbeat_confidence": published.as_ref().map(|value| value.heartbeat_confidence),
            "signal_quality": published.as_ref().map(|value| value.signal_quality),
        },
        "authority": if published.is_some() { "explicit_calibration" } else { "abstained" },
        "abstention_reason": if published.is_some() {
            serde_json::Value::Null
        } else {
            serde_json::json!("fresh explicit calibration with exactly one occupant and qualified evidence required")
        },
        "buffer_status": {
            "breathing_samples": br_len,
            "breathing_capacity": br_cap,
            "heartbeat_samples": hb_len,
            "heartbeat_capacity": hb_cap,
        },
        "source": s.effective_source(),
        "tick": s.tick,
    }))
}

/// Query params for `GET /api/v1/edge/registry`.
#[derive(Debug, Deserialize)]
pub(crate) struct EdgeRegistryParams {
    /// `?refresh=1` bypasses the in-process cache.
    #[serde(default)]
    pub(crate) refresh: Option<String>,
}

/// `GET /api/v1/edge/registry` — surfaces the canonical Cognitum cog catalog (ADR-102).
pub(crate) async fn edge_registry_endpoint(
    Extension(reg): Extension<
        Option<Arc<wifi_densepose_sensing_server::edge_registry::EdgeRegistry>>,
    >,
    Query(params): Query<EdgeRegistryParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let Some(reg) = reg else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "edge_registry_disabled",
                "detail": "This sensing-server was started with --no-edge-registry."
            })),
        ));
    };
    let force_refresh = matches!(params.refresh.as_deref(), Some("1") | Some("true"));
    if force_refresh {
        tracing::debug!(
            event = "edge_registry.refresh_requested",
            "?refresh=1 bypassed the cache; verify this isn't being abused"
        );
    }
    match tokio::task::spawn_blocking(move || reg.get(force_refresh)).await {
        Ok(Ok(resp)) => Ok(Json(
            serde_json::to_value(resp).unwrap_or(serde_json::json!({})),
        )),
        Ok(Err(err)) => Err(error_response::upstream_unavailable("edge_registry", err)),
        Err(join_err) => Err(error_response::internal_error("edge_registry", join_err)),
    }
}

/// `GET /api/v1/edge-vitals` — latest edge vitals from ESP32 (ADR-039).
pub(crate) async fn edge_vitals_endpoint(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let explicit_single_occupant = s.explicit_calibration_fresh_at(observed_at_unix_ms)
        && s.person_count_at(observed_at_unix_ms) == 1;
    if !explicit_single_occupant {
        return Json(serde_json::json!({
            "status": "abstained",
            "edge_vitals": null,
            "message": "Fresh explicit calibration with exactly one occupant is required.",
        }));
    }
    match &s.edge_vitals {
        Some(v) => Json(serde_json::json!({
            "status": "ok",
            "edge_vitals": v,
        })),
        None => Json(serde_json::json!({
            "status": "no_data",
            "edge_vitals": null,
            "message": "No edge vitals packet received yet. Ensure ESP32 edge_tier >= 1.",
        })),
    }
}

/// `GET /api/v1/wasm-events` — latest WASM events from ESP32 (ADR-040).
pub(crate) async fn wasm_events_endpoint(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.latest_wasm_events {
        Some(w) => Json(serde_json::json!({
            "status": "ok",
            "wasm_events": w,
        })),
        None => Json(serde_json::json!({
            "status": "no_data",
            "wasm_events": null,
            "message": "No WASM output packet received yet. Upload and start a .wasm module on the ESP32.",
        })),
    }
}

/// Build the vitals and edge runtime routes.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/vital-signs", get(vital_signs_endpoint))
        .route("/api/v1/edge-vitals", get(edge_vitals_endpoint))
        .route("/api/v1/edge/registry", get(edge_registry_endpoint))
        .route("/api/v1/wasm-events", get(wasm_events_endpoint))
}
