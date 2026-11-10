//! Runtime configuration endpoints (ADR-044).

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use crate::config::runtime_config::{save_runtime_config, RuntimeConfig};
use crate::SharedState;

/// Create sub-router for runtime configuration routes.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/config/dedup-factor",
            get(config_get_dedup_factor).post(config_set_dedup_factor),
        )
        .route("/api/v1/config/ground-truth", post(config_set_ground_truth))
}

/// `GET /api/v1/config/dedup-factor` — read the current dedup factor.
pub(crate) async fn config_get_dedup_factor(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "dedup_factor": s.dedup_factor,
        "description": "Divisor for multi-node person count deduplication (sum / factor). Range: 1.0–10.0."
    }))
}

/// `POST /api/v1/config/dedup-factor` — set the dedup factor (clamped 1.0–10.0).
pub(crate) async fn config_set_dedup_factor(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let value = body.get("value").and_then(|v| v.as_f64()).unwrap_or(3.0);
    let clamped = value.clamp(1.0, 10.0);
    let mut s = state.write().await;
    s.dedup_factor = clamped;
    let data_dir = s.data_dir.clone();
    drop(s);
    save_runtime_config(
        &data_dir,
        &RuntimeConfig {
            dedup_factor: clamped,
        },
    );
    Json(serde_json::json!({
        "status": "ok",
        "dedup_factor": clamped,
    }))
}

/// `POST /api/v1/config/ground-truth` — auto-tune dedup factor from a known person count.
pub(crate) async fn config_set_ground_truth(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let ground_truth = match body.get("count").and_then(|v| v.as_u64()) {
        Some(n) if n > 0 => n as usize,
        _ => return Json(serde_json::json!({"error": "count must be a positive integer"})),
    };
    let mut s = state.write().await;
    let raw_sum: usize = s
        .node_states
        .values()
        .filter(|ns| {
            ns.last_frame_time
                .map(|t| t.elapsed() < std::time::Duration::from_secs(10))
                .unwrap_or(false)
        })
        .map(|ns| ns.prev_person_count)
        .sum();
    let optimal = if raw_sum > 0 {
        (raw_sum as f64) / (ground_truth as f64)
    } else {
        3.0
    };
    let clamped = optimal.clamp(1.0, 10.0);
    s.dedup_factor = clamped;
    let data_dir = s.data_dir.clone();
    drop(s);
    save_runtime_config(
        &data_dir,
        &RuntimeConfig {
            dedup_factor: clamped,
        },
    );
    Json(serde_json::json!({
        "status": "ok",
        "ground_truth": ground_truth,
        "raw_sum": raw_sum,
        "computed_dedup_factor": clamped,
    }))
}
