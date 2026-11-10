//! Fleet node status and mesh synchronization REST endpoints.
//!
//! Provides per-node telemetry, mesh sync offsets, and Prometheus mesh metrics
//! according to ADR-110 (A0.12, iter 29, iter 36, iter 37).

use std::fmt::Write;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};

use crate::{NodeSyncSnapshot, SharedState, ESP32_OFFLINE_TIMEOUT};

/// `GET /api/v1/nodes/:id/sync` — single-node mesh sync snapshot.
pub(crate) async fn node_sync_endpoint(
    State(state): State<SharedState>,
    Path(id): Path<u8>,
) -> Result<Json<NodeSyncSnapshot>, (StatusCode, Json<serde_json::Value>)> {
    let s = state.read().await;
    let ns = s.node_states.get(&id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "unknown_node",
                "node_id": id,
            })),
        )
    })?;
    ns.sync_snapshot().map(Json).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "no_sync",
                "node_id": id,
                "hint": "node hasn't emitted a sync packet yet (no mesh peer or not v0.6.9+)",
            })),
        )
    })
}

/// `GET /api/v1/mesh/metrics` — Prometheus exposition format for fleet mesh state.
pub(crate) async fn mesh_metrics_endpoint(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.read().await;
    let mut body = String::with_capacity(1024);

    let metrics: &[(&str, &str, &str)] = &[
        (
            "wifi_densepose_mesh_offset_us",
            "Cross-board mesh-aligned offset, microseconds (signed)",
            "gauge",
        ),
        (
            "wifi_densepose_mesh_is_leader",
            "1 if this node is the elected mesh leader, else 0",
            "gauge",
        ),
        (
            "wifi_densepose_mesh_is_valid",
            "1 if this node has heard a fresh leader beacon, else 0",
            "gauge",
        ),
        (
            "wifi_densepose_mesh_smoothed",
            "1 once the firmware-side EMA filter has seeded, else 0",
            "gauge",
        ),
        (
            "wifi_densepose_mesh_sequence",
            "High-water CSI sequence at sync emit time",
            "gauge",
        ),
        (
            "wifi_densepose_mesh_csi_fps",
            "Per-node measured CSI frame rate (Hz)",
            "gauge",
        ),
        (
            "wifi_densepose_mesh_csi_fps_samples",
            "How many inter-frame deltas the fps EMA has seen",
            "gauge",
        ),
        (
            "wifi_densepose_mesh_staleness_ms",
            "Milliseconds since the host last received this node's sync packet",
            "gauge",
        ),
    ];

    let snaps: Vec<(u8, NodeSyncSnapshot)> = s
        .node_states
        .iter()
        .filter_map(|(&id, ns)| ns.sync_snapshot().map(|snap| (id, snap)))
        .collect();

    let (leaders, followers) = fleet_role_counts(&snaps);
    let no_sync = s.node_states.len().saturating_sub(snaps.len()) as u64;
    let _ = writeln!(
        body,
        "# HELP wifi_densepose_mesh_node_total Per-state node count across the fleet"
    );
    let _ = writeln!(body, "# TYPE wifi_densepose_mesh_node_total gauge");
    let _ = writeln!(
        body,
        "wifi_densepose_mesh_node_total{{state=\"leader\"}} {leaders}"
    );
    let _ = writeln!(
        body,
        "wifi_densepose_mesh_node_total{{state=\"follower\"}} {followers}"
    );
    let _ = writeln!(
        body,
        "wifi_densepose_mesh_node_total{{state=\"no_sync\"}} {no_sync}"
    );

    for (name, help, kind) in metrics {
        let _ = writeln!(body, "# HELP {name} {help}");
        let _ = writeln!(body, "# TYPE {name} {kind}");
        for (id, snap) in &snaps {
            let value = match *name {
                "wifi_densepose_mesh_offset_us" => snap.offset_us.to_string(),
                "wifi_densepose_mesh_is_leader" => bool_metric(snap.is_leader),
                "wifi_densepose_mesh_is_valid" => bool_metric(snap.is_valid),
                "wifi_densepose_mesh_smoothed" => bool_metric(snap.smoothed),
                "wifi_densepose_mesh_sequence" => snap.sequence.to_string(),
                "wifi_densepose_mesh_csi_fps" => format!("{:.3}", snap.csi_fps_ema),
                "wifi_densepose_mesh_csi_fps_samples" => snap.csi_fps_samples.to_string(),
                "wifi_densepose_mesh_staleness_ms" => snap
                    .staleness_ms
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "0".into()),
                _ => continue,
            };
            let _ = writeln!(body, "{name}{{node=\"{id}\"}} {value}");
        }
    }
    ([(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")], body)
}

pub(crate) fn bool_metric(b: bool) -> String {
    (if b { 1 } else { 0 }).to_string()
}

/// Count (leaders, followers) in a populated snapshot set.
pub(crate) fn fleet_role_counts(snaps: &[(u8, NodeSyncSnapshot)]) -> (u64, u64) {
    let leaders = snaps.iter().filter(|(_, s)| s.is_leader).count() as u64;
    let followers = (snaps.len() as u64).saturating_sub(leaders);
    (leaders, followers)
}

/// `GET /api/v1/mesh` — fleet-wide mesh state overview.
pub(crate) async fn mesh_endpoint(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let mut nodes = serde_json::Map::new();
    for (&id, ns) in s.node_states.iter() {
        if let Some(snap) = ns.sync_snapshot() {
            nodes.insert(id.to_string(), serde_json::to_value(snap).unwrap());
        }
    }
    let total = nodes.len();
    Json(serde_json::json!({
        "nodes": serde_json::Value::Object(nodes),
        "total": total,
    }))
}

/// `GET /api/v1/nodes` — per-node telemetry and freshness status.
pub(crate) async fn nodes_endpoint(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let now = std::time::Instant::now();
    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let nodes: Vec<serde_json::Value> = s
        .node_states
        .iter()
        .map(|(&id, ns)| {
            let elapsed_ms = ns
                .last_frame_time
                .map(|t| now.duration_since(t).as_millis() as u64)
                .unwrap_or(999999);
            let csi_elapsed_ms = ns
                .latest_accepted_csi_at
                .map(|t| now.saturating_duration_since(t).as_millis() as u64);
            let stale = elapsed_ms > 5000;
            let status = if stale { "stale" } else { "active" };
            let csi_status = if csi_elapsed_ms.is_some_and(|age| {
                age < ESP32_OFFLINE_TIMEOUT.as_millis() as u64
            }) {
                "active"
            } else {
                "stale"
            };
            let rssi = ns.rssi_history.back().copied().unwrap_or(-90.0);
            let bootstrap_empty = s.bootstrap_baseline_active
                && s.bootstrap_baseline.as_ref().is_some_and(|metadata| {
                    metadata.source_node_ids.as_slice() == [id]
                })
                && s.bootstrap_background_match(observed_at_unix_ms)
                    .is_some_and(|result| result.matches_empty);
            serde_json::json!({
                "node_id": id,
                "status": status,
                "last_seen_ms": elapsed_ms,
                "csi_status": csi_status,
                "csi_last_seen_ms": csi_elapsed_ms,
                "csi_sequence": ns.latest_csi_sequence,
                "rssi_dbm": rssi,
                "motion_level": if bootstrap_empty { "absent" } else { &ns.current_motion_level },
                "person_count": if bootstrap_empty { 0 } else { ns.prev_person_count },
                "person_count_valid": ns.edge_vitals
                    .as_ref()
                    .map(|vitals| vitals.person_count_valid),
                "position_configured": s.node_positions_config.contains_key(&id),
            })
        })
        .collect();
    Json(serde_json::json!({
        "nodes": nodes,
        "total": nodes.len(),
    }))
}

/// Build the nodes and mesh routes.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/nodes", get(nodes_endpoint))
        .route("/api/v1/nodes/:id/sync", get(node_sync_endpoint))
        .route("/api/v1/mesh", get(mesh_endpoint))
        .route("/api/v1/mesh/metrics", get(mesh_metrics_endpoint))
}
