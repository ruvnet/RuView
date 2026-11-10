use axum::{body::Body, http::{Request, StatusCode}, routing::get, Router};
use clap::Parser;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower::ServiceExt;

use crate::api::pose::pose_current;
use crate::api::sensing::latest;
use crate::api::stream::{ws_pose_handler, ws_sensing_handler};
use crate::api::vitals::vital_signs_endpoint;
use crate::cli::Args;
use crate::domain::SensingUpdate;
use crate::privacy_filter;
use crate::state::{AppStateInner, SharedState};

fn update_with_biometrics() -> SensingUpdate {
    serde_json::from_value(serde_json::json!({
        "type": "sensing_update",
        "timestamp": 1.0,
        "source": "simulated",
        "tick": 7,
        "nodes": [],
        "features": { "mean_rssi": -50.0, "variance": 1.0, "motion_band_power": 0.5,
                      "breathing_band_power": 0.2, "dominant_freq_hz": 0.25,
                      "change_points": 0, "spectral_power": 1.0 },
        "classification": { "motion_level": "present_still", "presence": true, "confidence": 0.9 },
        "signal_field": { "grid_size": [1, 1, 1], "values": [0.0] },
        "vital_signs": { "breathing_rate_bpm": 14.0, "heart_rate_bpm": 62.0,
                         "breathing_confidence": 0.8, "heartbeat_confidence": 0.7,
                         "signal_quality": 0.9 },
        "pose_keypoints": [[0.1, 0.2, 0.0, 0.9]],
        "persons": [{ "id": 1, "confidence": 0.9, "zone": "zone_1",
                      "bbox": { "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0 },
                      "keypoints": [{ "name": "nose", "x": 1.0, "y": 2.0, "z": 0.0, "confidence": 0.9 }] }]
    }))
    .expect("test SensingUpdate")
}

fn state(privacy_mode: bool) -> SharedState {
    let mut inner = AppStateInner::minimal();
    inner.privacy_mode = privacy_mode;
    let mut update = update_with_biometrics();
    update.timestamp = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
    inner.latest_update = Some(update);
    Arc::new(RwLock::new(inner))
}

fn rest_app(privacy_mode: bool) -> Router {
    Router::new()
        .route("/api/v1/sensing/latest", get(latest))
        .route("/api/v1/vital-signs", get(vital_signs_endpoint))
        .route("/api/v1/pose/current", get(pose_current))
        .with_state(state(privacy_mode))
        .layer(axum::middleware::from_fn_with_state(
            privacy_filter::PrivacyFilter::new(privacy_mode),
            privacy_filter::redact_json_responses,
        ))
}

async fn get_json(app: Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[test]
fn privacy_flag_is_top_level_and_defaults_off() {
    assert!(!Args::parse_from(["sensing-server"]).privacy_mode);
    let args = Args::parse_from(["sensing-server", "--privacy-mode"]);
    assert!(args.privacy_mode);
    assert!(!args.mqtt_opts.mqtt, "--privacy-mode must not imply --mqtt");
}

#[tokio::test]
async fn sensing_latest_keeps_biometrics_when_privacy_off() {
    let (_, body) = get_json(rest_app(false), "/api/v1/sensing/latest").await;
    assert!(body.get("vital_signs").is_some());
    assert!(body.get("pose_keypoints").is_some());
    assert!(body["persons"][0].get("keypoints").is_some());
}

#[tokio::test]
async fn sensing_latest_strips_biometrics_in_privacy_mode() {
    let (status, body) = get_json(rest_app(true), "/api/v1/sensing/latest").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("vital_signs").is_none(), "{body}");
    assert!(body.get("pose_keypoints").is_none(), "{body}");
    assert!(body["persons"][0].get("keypoints").is_none(), "{body}");
    assert_eq!(body["classification"]["presence"], true);
    assert_eq!(body["persons"][0]["zone"], "zone_1");
}

#[tokio::test]
async fn vital_signs_endpoint_strips_rates_in_privacy_mode() {
    let (_, body) = get_json(rest_app(true), "/api/v1/vital-signs").await;
    assert!(body.get("vital_signs").is_none(), "{body}");
    let (_, body) = get_json(rest_app(false), "/api/v1/vital-signs").await;
    assert!(body["vital_signs"].get("heart_rate_bpm").is_some());
}

#[tokio::test]
async fn pose_current_strips_keypoints_and_refuses_skeletal_views() {
    let (status, body) = get_json(rest_app(true), "/api/v1/pose/current").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total_persons"], 1);
    assert!(body["persons"][0].get("keypoints").is_none(), "{body}");
    for view in ["both", "refined"] {
        let (status, body) =
            get_json(rest_app(true), &format!("/api/v1/pose/current?view={view}")).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "view={view}");
        assert_eq!(body["code"], "privacy_mode");
    }
}

async fn first_ws_frame(app: Router, path: &str, tx: broadcast::Sender<String>) -> String {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message as TMsg;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}{path}"))
        .await
        .expect("WebSocket handshake");
    let frame = serde_json::to_string(&update_with_biometrics()).unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        let _ = tx.send(frame.clone());
        match tokio::time::timeout(std::time::Duration::from_millis(200), ws.next()).await {
            Ok(Some(Ok(TMsg::Text(text)))) => {
                let v: serde_json::Value = serde_json::from_str(&text).unwrap();
                if v["type"] == "sensing_update" || v["type"] == "pose_data" {
                    return text;
                }
            }
            Ok(Some(Ok(_))) | Err(_) => {}
            Ok(Some(Err(e))) => panic!("ws error: {e}"),
            Ok(None) => panic!("ws closed"),
        }
    }
    panic!("no frame on {path}");
}

async fn ws_frames(privacy_mode: bool) -> (String, String) {
    let shared = state(privacy_mode);
    let tx = shared.read().await.tx.clone();
    let app = Router::new()
        .route("/ws/sensing", get(ws_sensing_handler))
        .route("/api/v1/stream/pose", get(ws_pose_handler))
        .with_state(shared);
    let sensing = first_ws_frame(app.clone(), "/ws/sensing", tx.clone()).await;
    let pose = first_ws_frame(app, "/api/v1/stream/pose", tx).await;
    (sensing, pose)
}

#[tokio::test]
async fn websocket_streams_strip_biometrics_in_privacy_mode() {
    let (sensing, pose) = ws_frames(true).await;
    for text in [&sensing, &pose] {
        assert!(!text.contains("heart_rate_bpm"), "{text}");
        assert!(!text.contains("breathing_rate_bpm"), "{text}");
        assert!(!text.contains("keypoints"), "{text}");
    }
    assert!(sensing.contains("\"presence\":true"), "{sensing}");
    assert!(pose.contains("\"persons\""), "{pose}");
}

#[tokio::test]
async fn websocket_streams_unchanged_when_privacy_off() {
    let (sensing, pose) = ws_frames(false).await;
    assert!(sensing.contains("heart_rate_bpm"), "{sensing}");
    assert!(sensing.contains("keypoints"), "{sensing}");
    assert!(pose.contains("keypoints"), "{pose}");
}

