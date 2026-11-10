//! Sensing and RF Vendor REST endpoints (ADR-080, ADR-166, ADR-295).
//!
//! Provides latest sensing updates, radar/CSI snapshots across hardware vendors
//! (MediaTek, Qualcomm, Realtek), and pluggable vendor RF event ingestion.

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use std::time::Duration;

use crate::{SharedState, ESP32_OFFLINE_TIMEOUT};

/// How old `latest_update` may be before `/api/v1/sensing/latest` stops
/// serving it as current.
pub(crate) const LATEST_UPDATE_STALE_AFTER: Duration = Duration::from_secs(10);

/// `GET /api/v1/sensing/latest` — return the most recent sensing update or stale status.
pub(crate) async fn latest(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.latest_update {
        Some(update) => {
            let now_ms = chrono::Utc::now().timestamp_millis() as f64;
            let age_ms = (now_ms - update.timestamp * 1000.0).max(0.0);
            if age_ms > LATEST_UPDATE_STALE_AFTER.as_millis() as f64 {
                return Json(serde_json::json!({
                    "status": "stale",
                    "age_ms": age_ms,
                    "source": s.effective_source(),
                }));
            }
            let mut value = serde_json::to_value(update).unwrap_or_default();
            if let Some(obj) = value.as_object_mut() {
                obj.insert("age_ms".to_string(), serde_json::json!(age_ms));
            }
            Json(value)
        }
        None => Json(serde_json::json!({"status": "no data yet"})),
    }
}

/// `GET /api/v1/radar/latest` — latest Realtek radar snapshot.
pub(crate) async fn latest_realtek_radar(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.latest_realtek_radar {
        Some(snapshot) => Json(serde_json::to_value(snapshot).unwrap_or_default()),
        None => Json(serde_json::json!({"status": "no Realtek radar data yet"})),
    }
}

/// `GET /api/v1/csi/mediatek/latest` — latest MediaTek CSI snapshot.
pub(crate) async fn latest_mediatek_csi(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.latest_mediatek_csi {
        Some(snapshot) => Json(serde_json::to_value(snapshot).unwrap_or_default()),
        None => Json(serde_json::json!({"status": "no MediaTek CSI data yet"})),
    }
}

/// `GET /api/v1/csi/qualcomm/latest` — latest Qualcomm CSI snapshot.
pub(crate) async fn latest_qualcomm_csi(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.latest_qualcomm_csi {
        Some(snapshot) => Json(serde_json::to_value(snapshot).unwrap_or_default()),
        None => Json(serde_json::json!({"status": "no Qualcomm CSI data yet"})),
    }
}

/// `GET /api/v1/csi/realtek/latest` — latest Realtek RTL8721Dx CSI snapshot.
pub(crate) async fn latest_realtek_csi(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.latest_realtek_csi {
        Some(snapshot) => Json(serde_json::to_value(snapshot).unwrap_or_default()),
        None => Json(serde_json::json!({"status": "no Realtek RTL8721Dx CSI data yet"})),
    }
}

/// Determine primary room source, prioritizing fresh ESP32 frames over Realtek CSI.
pub(crate) fn primary_source_with_realtek(
    last_esp32_frame: Option<std::time::Instant>,
    realtek_source: &str,
) -> String {
    if last_esp32_frame.is_some_and(|seen| seen.elapsed() < ESP32_OFFLINE_TIMEOUT) {
        "esp32".to_string()
    } else {
        realtek_source.to_string()
    }
}

/// `GET /api/v1/rf/vendors` — list supported RF vendor capabilities.
pub(crate) async fn vendor_descriptors() -> Json<serde_json::Value> {
    Json(
        serde_json::to_value(wifi_densepose_sensing_server::vendor_rf::descriptors())
            .unwrap_or_default(),
    )
}

/// `GET /api/v1/rf/vendors/latest` — latest RF events across all vendors.
pub(crate) async fn latest_vendor_events(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let state = state.read().await;
    Json(serde_json::to_value(&state.latest_vendor_rf).unwrap_or_default())
}

/// `GET /api/v1/rf/vendors/:vendor/latest` — latest event snapshot for a given vendor.
pub(crate) async fn latest_vendor_event(
    State(state): State<SharedState>,
    Path(vendor): Path<String>,
) -> impl IntoResponse {
    let Some(vendor_id) = wifi_densepose_sensing_server::vendor_rf::vendor_from_str(&vendor) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "unknown vendor"})));
    };
    let state = state.read().await;
    let canonical_vendor = vendor_id.as_str();
    match state.latest_vendor_rf.get(canonical_vendor) {
        Some(snapshot) => (
            StatusCode::OK,
            Json(serde_json::to_value(snapshot).unwrap_or_default()),
        ),
        None => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "no vendor RF data yet",
                "vendor": canonical_vendor
            })),
        ),
    }
}

/// `POST /api/v1/rf/vendors/:vendor/events` — ingest vendor RF events.
pub(crate) async fn ingest_vendor_events(
    State(state): State<SharedState>,
    Path(vendor): Path<String>,
    payload: Bytes,
) -> impl IntoResponse {
    let Some(vendor_id) = wifi_densepose_sensing_server::vendor_rf::vendor_from_str(&vendor) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "unknown vendor"})));
    };
    let events = match wifi_densepose_sensing_server::vendor_rf::decode_provider(vendor_id, &payload) {
        Ok(events) => events,
        Err(error) => {
            let status = match error {
                wifi_densepose_hardware::vendor_rf::VendorEventError::Unsupported => StatusCode::NOT_IMPLEMENTED,
                wifi_densepose_hardware::vendor_rf::VendorEventError::ContractRequired
                | wifi_densepose_hardware::vendor_rf::VendorEventError::CredentialsRequired => StatusCode::FORBIDDEN,
                _ => StatusCode::BAD_REQUEST,
            };
            return (status, Json(serde_json::json!({"error": error.to_string(), "vendor": vendor})));
        }
    };
    let mut accepted = 0usize;
    let mut state = state.write().await;
    let canonical_vendor = vendor_id.as_str().to_string();
    for event in events {
        match wifi_densepose_sensing_server::vendor_rf::VendorEventSnapshot::from_event(event) {
            Ok(snapshot) => {
                let json = serde_json::to_string(&snapshot).ok();
                state.source = snapshot.source.clone();
                state.last_vendor_rf_frame = Some(std::time::Instant::now());
                state
                    .latest_vendor_rf
                    .insert(canonical_vendor.clone(), snapshot);
                if let Some(json) = json {
                    let _ = state.tx.send(json);
                }
                accepted += 1;
            }
            Err(error) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": error.to_string(), "vendor": canonical_vendor})),
                )
            }
        }
    }
    (StatusCode::ACCEPTED, Json(serde_json::json!({"vendor": vendor, "accepted": accepted})))
}

/// Build the sensing and RF routes.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/v1/sensing/latest", get(latest))
        .route("/api/v1/radar/latest", get(latest_realtek_radar))
        .route("/api/v1/csi/mediatek/latest", get(latest_mediatek_csi))
        .route("/api/v1/csi/qualcomm/latest", get(latest_qualcomm_csi))
        .route("/api/v1/csi/realtek/latest", get(latest_realtek_csi))
        .route("/api/v1/rf/vendors", get(vendor_descriptors))
        .route("/api/v1/rf/vendors/latest", get(latest_vendor_events))
        .route("/api/v1/rf/vendors/:vendor/latest", get(latest_vendor_event))
        .route("/api/v1/rf/vendors/:vendor/events", post(ingest_vendor_events))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use wifi_densepose_hardware::realtek_csi::simulator::{RealtekCsiSimulator, SimulatorConfig};
    use wifi_densepose_sensing_server::domain::ClassificationInfo;
    use crate::{AppStateInner, FeatureInfo, SensingUpdate, SignalField, realtek_csi};

    fn update_at(timestamp: f64) -> SensingUpdate {
        SensingUpdate {
            msg_type: "sensing_update".to_string(),
            timestamp,
            source: "esp32".to_string(),
            tick: 1,
            nodes: vec![],
            features: FeatureInfo {
                mean_rssi: 0.0,
                variance: 0.0,
                motion_band_power: 0.0,
                breathing_band_power: 0.0,
                dominant_freq_hz: 0.0,
                change_points: 0,
                spectral_power: 0.0,
            },
            classification: ClassificationInfo {
                motion_level: "absent".to_string(),
                presence: false,
                confidence: 0.97,
            },
            signal_field: SignalField {
                grid_size: [0, 0, 0],
                values: vec![],
            },
            vital_signs: None,
            calibrated_presence_evidence: None,
            detector_diagnostics: None,
            enhanced_motion: None,
            enhanced_breathing: None,
            posture: None,
            signal_quality_score: None,
            quality_verdict: None,
            bssid_count: None,
            pose_keypoints: None,
            model_status: None,
            persons: None,
            estimated_persons: None,
            node_features: None,
            room_inference: None,
            classifier: None,
            activity: None,
        }
    }

    fn now_s() -> f64 {
        chrono::Utc::now().timestamp_millis() as f64 / 1000.0
    }

    #[tokio::test]
    async fn fresh_update_is_served_with_age_ms() {
        let state: SharedState = Arc::new(RwLock::new(AppStateInner::minimal()));
        state.write().await.latest_update = Some(update_at(now_s()));
        let Json(value) = latest(State(state)).await;
        assert_eq!(value["classification"]["motion_level"], "absent");
        let age = value["age_ms"]
            .as_f64()
            .expect("fresh payload carries age_ms");
        assert!(age < 1000.0, "age_ms was {age}");
    }

    #[tokio::test]
    async fn update_older_than_cutoff_is_reported_stale_not_served() {
        let state: SharedState = Arc::new(RwLock::new(AppStateInner::minimal()));
        let old = now_s() - (LATEST_UPDATE_STALE_AFTER.as_secs_f64() + 60.0);
        state.write().await.latest_update = Some(update_at(old));
        let Json(value) = latest(State(state)).await;
        assert_eq!(value["status"], "stale");
        assert!(value["age_ms"].as_f64().unwrap() > LATEST_UPDATE_STALE_AFTER.as_millis() as f64);
        assert!(
            value.get("classification").is_none(),
            "a frozen classification must not be served as current"
        );
    }

    #[tokio::test]
    async fn no_update_yet_is_unchanged() {
        let state: SharedState = Arc::new(RwLock::new(AppStateInner::minimal()));
        let Json(value) = latest(State(state)).await;
        assert_eq!(value["status"], "no data yet");
    }

    #[test]
    fn secondary_realtek_does_not_displace_fresh_esp32_room_source() {
        assert_eq!(
            primary_source_with_realtek(Some(std::time::Instant::now()), "realtek_csi"),
            "esp32"
        );
        assert_eq!(primary_source_with_realtek(None, "realtek_csi"), "realtek_csi");
    }

    #[tokio::test]
    async fn latest_route_keeps_synthetic_provenance_and_node_identity() {
        let state: SharedState = Arc::new(RwLock::new(AppStateInner::minimal()));
        let mut simulator = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let snapshot = realtek_csi::RealtekCsiSnapshot::from_frame(&simulator.next_frame());
        let expected_node_id = snapshot.node_id;
        state.write().await.latest_realtek_csi = Some(snapshot);
        let Json(value) = latest_realtek_csi(State(state)).await;
        assert_eq!(value["node_id"], expected_node_id);
        assert_eq!(value["source"], "realtek_csi:simulated");
        assert_eq!(value["synthetic"], true);
    }
}
