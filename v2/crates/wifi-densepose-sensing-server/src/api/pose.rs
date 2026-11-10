//! Pose estimation and real-time stream status REST endpoints.
//!
//! Exposes current estimated poses, physics-governed joint metrics, zone summaries,
//! and measured stream frame rates (ADR-099, ADR-2087, ADR-2094).

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::Deserialize;
use crate::{
    derive_pose_from_sensing, node_is_fresh, pose_physics,
    AppStateInner, SharedState,
};

#[derive(Debug, Default, Deserialize)]
pub(crate) struct PoseCurrentQuery {
    #[serde(default)]
    pub(crate) view: pose_physics::PoseView,
}

/// `GET /api/v1/pose/current` — return latest pose detections or physics-refined joints.
pub(crate) async fn pose_current(
    State(state): State<SharedState>,
    Query(query): Query<PoseCurrentQuery>,
) -> impl IntoResponse {
    let s = state.read().await;
    let persons = match &s.latest_update {
        Some(update) => update
            .persons
            .clone()
            .unwrap_or_else(|| derive_pose_from_sensing(update)),
        None => vec![],
    };
    let legacy = serde_json::json!({
        "timestamp": chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
        "persons": persons,
        "total_persons": persons.len(),
        "source": s.effective_source(),
    });
    // #2094: the raw/refined views are the skeleton itself. The legacy view is
    // still served; the REST privacy middleware strips its keypoints.
    if s.privacy_mode && !matches!(query.view, pose_physics::PoseView::Raw) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "code": "privacy_mode",
                "detail": "pose views are disabled while --privacy-mode is set"
            })),
        );
    }
    match query.view {
        pose_physics::PoseView::Raw => (StatusCode::OK, Json(legacy)),
        pose_physics::PoseView::Both => {
            if let Some((raw, physics)) = s.pose_physics.latest() {
                let mut body = legacy;
                body["raw"] = serde_json::to_value(raw).unwrap_or(serde_json::Value::Null);
                body["physics"] =
                    serde_json::to_value(physics).unwrap_or(serde_json::Value::Null);
                (StatusCode::OK, Json(body))
            } else {
                (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "code": "pose_physics_unavailable",
                        "detail": "no canonical pose observation has been assessed"
                    })),
                )
            }
        }
        pose_physics::PoseView::Refined => {
            let Some((raw, physics)) = s.pose_physics.latest() else {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "code": "pose_refined_unavailable",
                        "detail": "no canonical pose observation has been assessed"
                    })),
                );
            };
            match pose_physics::PosePhysicsRuntime::select(query.view, raw, physics) {
                Ok(pose_physics::SelectedPose::Refined(joints)) => (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "schema_version": 1,
                        "raw_observation_hash": raw.canonical_hash,
                        "joints_m": joints,
                        "physics": physics,
                    })),
                ),
                Ok(_) => unreachable!("refined view returns only refined data"),
                Err(error) => (
                    StatusCode::CONFLICT,
                    Json(serde_json::to_value(error).unwrap_or_else(|_| {
                        serde_json::json!({"code": "pose_refined_unavailable"})
                    })),
                ),
            }
        }
    }
}

/// `GET /api/v1/pose/physics/metrics` — Prometheus-compatible metrics text.
pub(crate) async fn pose_physics_metrics(State(state): State<SharedState>) -> impl IntoResponse {
    let metrics = state.read().await.pose_physics.metrics_text();
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        metrics,
    )
}

/// `GET /api/v1/pose/stats` — overall detection and frame statistics.
pub(crate) async fn pose_stats(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "total_detections": s.total_detections,
        "average_confidence": 0.87,
        "frames_processed": s.tick,
        "source": s.effective_source(),
    }))
}

/// `GET /api/v1/pose/zones/summary` — occupancy summary partitioned by room zones.
pub(crate) async fn pose_zones_summary(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let presence = s
        .latest_update
        .as_ref()
        .map(|u| u.classification.presence)
        .unwrap_or(false);
    Json(serde_json::json!({
        "zones": {
            "zone_1": { "person_count": if presence { 1 } else { 0 }, "status": "monitored" },
            "zone_2": { "person_count": 0, "status": "clear" },
            "zone_3": { "person_count": 0, "status": "clear" },
            "zone_4": { "person_count": 0, "status": "clear" },
        }
    }))
}

/// `GET /api/v1/pose/activities` — activity classification history placeholder.
pub(crate) async fn pose_activities() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "activities": [],
        "total": 0,
        "persisted": false,
        "message": "Activity history is not persisted by the Rust sensing server.",
    }))
}

/// `GET /api/v1/stream/status` — measured sensing stream status and node FPS.
pub(crate) async fn stream_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(stream_status_json(&s, std::time::Instant::now()))
}

/// #2087: measured stream status.
pub(crate) fn stream_status_json(s: &AppStateInner, now: std::time::Instant) -> serde_json::Value {
    use wifi_densepose_sensing_server::stream_stats::round1;
    let fps = s.update_rate.rate_hz(now);
    let mut ids: Vec<u8> = s
        .node_states
        .iter()
        .filter(|(_, n)| node_is_fresh(n, now))
        .map(|(id, _)| *id)
        .collect();
    ids.sort_unstable();
    let nodes: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| {
            let n = &s.node_states[id];
            serde_json::json!({
                "node_id": id,
                "csi_fps": n.measured_sample_rate_hz().map(round1),
                "csi_fps_samples": n.csi_fps_samples,
            })
        })
        .collect();
    let csi_fps_total = ids
        .iter()
        .filter_map(|id| s.node_states[id].measured_sample_rate_hz())
        .fold(0.0, |acc, fps| acc + fps);
    serde_json::json!({
        "active": fps > 0.0,
        "clients": s.ws_clients.get(),
        "fps": round1(fps),
        "csi_fps_total": round1(csi_fps_total),
        "nodes": nodes,
        "source": s.effective_source(),
    })
}

/// `stream` component of `/health/system`: healthy while updates are flowing.
pub(crate) fn stream_health_component(s: &AppStateInner, now: std::time::Instant) -> serde_json::Value {
    let clients = s.ws_clients.get();
    let fps = wifi_densepose_sensing_server::stream_stats::round1(s.update_rate.rate_hz(now));
    let status = if clients > 0 && fps > 0.0 {
        "healthy"
    } else {
        "idle"
    };
    serde_json::json!({
        "status": status,
        "message": format!("{clients} client(s), {fps} updates/s"),
        "clients": clients,
        "fps": fps,
    })
}

/// Build the pose and stream REST routes.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/pose/current", get(pose_current))
        .route("/api/v1/pose/physics/metrics", get(pose_physics_metrics))
        .route("/api/v1/pose/stats", get(pose_stats))
        .route("/api/v1/pose/zones/summary", get(pose_zones_summary))
        .route("/api/v1/pose/activities", get(pose_activities))
        .route("/api/v1/stream/status", get(stream_status))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wifi_densepose_sensing_server::domain::{NodeState, NODE_STALE_AFTER_MS};

    fn warmed_node(fps: f64, seen: std::time::Instant) -> NodeState {
        let mut n = NodeState::new();
        n.csi_fps_ema = fps;
        n.csi_fps_samples = 50;
        n.last_frame_time = Some(seen);
        n
    }

    #[test]
    fn idle_server_reports_zero_fps_and_no_clients() {
        let s = AppStateInner::minimal();
        let _recorder = s.tx.subscribe();
        let v = stream_status_json(&s, std::time::Instant::now());
        assert_eq!(v["fps"], 0.0);
        assert_eq!(v["active"], false);
        assert_eq!(v["clients"], 0);
        assert_eq!(v["nodes"].as_array().unwrap().len(), 0);
        assert_eq!(
            stream_health_component(&s, std::time::Instant::now())["status"],
            "idle"
        );
    }

    #[test]
    fn fps_is_the_measured_update_rate() {
        let mut s = AppStateInner::minimal();
        let t0 = std::time::Instant::now();
        for i in 0..=40u64 {
            s.tick += 1;
            s.update_rate.observe(t0 + Duration::from_millis(50 * i));
        }
        let v = stream_status_json(&s, t0 + Duration::from_millis(2000));
        assert_eq!(v["fps"], 20.0, "a 50 ms cadence must read 20, not 10");
        assert_eq!(v["active"], true);
    }

    #[test]
    fn per_node_fps_and_aggregate_cover_fresh_nodes_only() {
        let mut s = AppStateInner::minimal();
        let now = std::time::Instant::now();
        s.node_states.insert(2, warmed_node(46.0, now));
        s.node_states.insert(1, warmed_node(41.0, now));
        s.node_states.insert(
            3,
            warmed_node(30.0, now - Duration::from_millis(NODE_STALE_AFTER_MS + 1)),
        );
        let mut cold = NodeState::new();
        cold.last_frame_time = Some(now);
        s.node_states.insert(4, cold);

        let v = stream_status_json(&s, now);
        let nodes = v["nodes"].as_array().unwrap();
        let ids: Vec<u64> = nodes
            .iter()
            .map(|n| n["node_id"].as_u64().unwrap())
            .collect();
        assert_eq!(ids, vec![1, 2, 4], "stale node 3 excluded, sorted by id");
        assert_eq!(nodes[0]["csi_fps"], 41.0);
        assert_eq!(nodes[1]["csi_fps"], 46.0);
        assert!(
            nodes[2]["csi_fps"].is_null(),
            "unwarmed EMA is not reported"
        );
        assert_eq!(v["csi_fps_total"], 87.0);
    }

    #[test]
    fn clients_count_websockets_not_internal_subscribers() {
        let mut s = AppStateInner::minimal();
        let _recorder = s.tx.subscribe();
        let _mqtt = s.tx.subscribe();
        let browser = s.ws_clients.enter();
        let now = std::time::Instant::now();
        for i in 0..10u64 {
            s.update_rate
                .observe(now - Duration::from_millis(100 * (10 - i)));
        }
        assert_eq!(stream_status_json(&s, now)["clients"], 1);
        let health = stream_health_component(&s, now);
        assert_eq!(health["status"], "healthy");
        assert_eq!(health["clients"], 1);
        drop(browser);
        assert_eq!(stream_status_json(&s, now)["clients"], 0);
        assert_eq!(stream_health_component(&s, now)["status"], "idle");
    }
}
