//! Real-time WebSocket streaming handlers for sensing and pose.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use tracing::{debug, info};
use crate::field_localize;
use crate::privacy_filter;
use crate::{
    derive_pose_from_sensing, BoundingBox, PersonDetection, PoseKeypoint, SensingUpdate,
    SharedState,
};

/// Create sub-router for WebSocket streaming routes.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/ws/sensing", get(ws_sensing_handler))
        .route("/api/v1/stream/pose", get(ws_pose_handler))
}

pub(crate) async fn ws_sensing_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_client(socket, state))
}

async fn handle_ws_client(mut socket: WebSocket, state: SharedState) {
    let (mut rx, privacy_mode, _client) = {
        let s = state.read().await;
        (s.tx.subscribe(), s.privacy_mode, s.ws_clients.enter())
    };

    info!("WebSocket client connected (sensing)");

    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(json) => {
                        let json = if privacy_mode {
                            match privacy_filter::redact_json_str(&json) {
                                Some(json) => json,
                                None => continue,
                            }
                        } else {
                            json
                        };
                        if socket.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!("WS client lagged by {n} frames, skipping");
                        continue;
                    }
                    Err(_) => break,
                }
            }
            _ = ping_interval.tick() => {
                if socket.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Pong(_))) => {}
                    _ => {}
                }
            }
        }
    }

    info!("WebSocket client disconnected (sensing)");
}

pub(crate) async fn ws_pose_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_pose_client(socket, state))
}

async fn handle_ws_pose_client(mut socket: WebSocket, state: SharedState) {
    let (mut rx, privacy_mode, _client) = {
        let s = state.read().await;
        (s.tx.subscribe(), s.privacy_mode, s.ws_clients.enter())
    };

    info!("WebSocket client connected (pose)");

    let conn_msg = serde_json::json!({
        "type": "connection_established",
        "payload": { "status": "connected", "backend": "rust+ruvector" }
    });
    let _ = socket.send(Message::Text(conn_msg.to_string())).await;

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(json) => {
                        if let Ok(sensing) = serde_json::from_str::<SensingUpdate>(&json) {
                            if sensing.msg_type == "sensing_update" {
                                let (model_loaded, physics_assessment) = {
                                    let s = state.read().await;
                                    let latest = s.pose_physics.latest().filter(|_| !privacy_mode);
                                    let physics = latest.and_then(|(raw, result)| {
                                        (raw.sequence == sensing.tick).then(|| {
                                            serde_json::json!({
                                                "schema": "pose-refinement-v1",
                                                "raw_observation_hash": raw.canonical_hash,
                                                "assessment": result,
                                            })
                                        })
                                    });
                                    (s.model_loaded, physics)
                                };
                                let pose_source = if model_loaded {
                                    "model_inference"
                                } else {
                                    "signal_derived"
                                };

                                let persons = if model_loaded {
                                    sensing.pose_keypoints.as_ref().map(|kps| {
                                        let kp_names = [
                                            "nose","left_eye","right_eye","left_ear","right_ear",
                                            "left_shoulder","right_shoulder","left_elbow","right_elbow",
                                            "left_wrist","right_wrist","left_hip","right_hip",
                                            "left_knee","right_knee","left_ankle","right_ankle",
                                        ];
                                        let keypoints: Vec<PoseKeypoint> = kps.iter()
                                            .enumerate()
                                            .map(|(i, kp)| PoseKeypoint {
                                                name: kp_names.get(i).unwrap_or(&"unknown").to_string(),
                                                x: kp[0], y: kp[1], z: kp[2], confidence: kp[3],
                                            })
                                            .collect();
                                        let [nx, _ny, nz] = sensing.signal_field.grid_size;
                                        let peak = field_localize::extract_peaks(
                                            &sensing.signal_field.values, nx, nz, 1, 3.0,
                                        ).into_iter().next();
                                        vec![PersonDetection {
                                            id: 1,
                                            confidence: sensing.classification.confidence,
                                            bbox: BoundingBox { x: 260.0, y: 150.0, width: 120.0, height: 220.0 },
                                            keypoints,
                                            zone: "zone_1".into(),
                                            position: peak.map_or([0.0, 0.0, 0.0], |p| p.position),
                                            motion_score: field_localize::motion_score_from_power(
                                                sensing.features.motion_band_power,
                                            ),
                                            pose: sensing.posture.clone(),
                                        }]
                                    }).unwrap_or_else(|| {
                                        sensing.persons.clone().unwrap_or_else(|| derive_pose_from_sensing(&sensing))
                                    })
                                } else {
                                    sensing.persons.clone().unwrap_or_else(|| derive_pose_from_sensing(&sensing))
                                };

                                let mut pose_msg = serde_json::json!({
                                    "type": "pose_data",
                                    "zone_id": "zone_1",
                                    "timestamp": sensing.timestamp,
                                    "physics": physics_assessment,
                                    "payload": {
                                        "pose": {
                                            "persons": persons,
                                        },
                                        "confidence": if sensing.classification.presence { sensing.classification.confidence } else { 0.0 },
                                        "activity": sensing.classification.motion_level,
                                        "pose_source": pose_source,
                                        "metadata": {
                                            "frame_id": format!("rust_frame_{}", sensing.tick),
                                            "processing_time_ms": 1,
                                            "source": sensing.source,
                                            "tick": sensing.tick,
                                            "signal_strength": sensing.features.mean_rssi,
                                            "motion_band_power": sensing.features.motion_band_power,
                                            "breathing_band_power": sensing.features.breathing_band_power,
                                            "estimated_persons": persons.len(),
                                        }
                                    }
                                });
                                if privacy_mode {
                                    privacy_filter::redact_value(&mut pose_msg);
                                }
                                if socket.send(Message::Text(pose_msg.to_string())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!("WS pose client lagged by {n} frames, skipping");
                        continue;
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                            if v.get("type").and_then(|t| t.as_str()) == Some("ping") {
                                let pong = serde_json::json!({"type": "pong"});
                                let _ = socket.send(Message::Text(pong.to_string())).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Pong(_))) => {}
                    _ => {}
                }
            }
        }
    }
}
