use axum::extract::State;
use axum::Json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::api::health::health_ready;
use crate::state::{AppStateInner, SharedState, ESP32_OFFLINE_TIMEOUT};

async fn status_with(
    source: &str,
    last_esp32_frame: Option<std::time::Instant>,
) -> serde_json::Value {
    let mut inner = AppStateInner::minimal();
    inner.source = source.to_string();
    inner.last_esp32_frame = last_esp32_frame;
    let state: SharedState = Arc::new(RwLock::new(inner));
    let Json(value) = health_ready(State(state)).await;
    value
}

#[tokio::test]
async fn esp32_with_no_frame_yet_is_waiting_not_live() {
    let status = status_with("esp32", None).await;
    assert_eq!(status["source_state"], "disconnected");
    assert_eq!(status["waiting_for_frames"], true);
    assert!(status["last_frame_age_ms"].is_null());
}

#[tokio::test]
async fn esp32_with_fresh_frame_is_live() {
    let status = status_with("esp32", Some(std::time::Instant::now())).await;
    assert_eq!(status["source_state"], "live_unverified");
    assert_eq!(status["waiting_for_frames"], false);
    assert!(status["last_frame_age_ms"].as_u64().unwrap() < 1000);
}

#[tokio::test]
async fn esp32_whose_frames_stopped_is_stale() {
    let old = std::time::Instant::now()
        .checked_sub(ESP32_OFFLINE_TIMEOUT + Duration::from_secs(1))
        .expect("monotonic clock is past the offline timeout");
    let status = status_with("esp32", Some(old)).await;
    assert_eq!(status["source_state"], "stale");
    assert_eq!(status["waiting_for_frames"], false);
    assert_eq!(status["source"], "esp32:offline");
}

#[tokio::test]
async fn vendor_event_source_uses_its_receive_clock() {
    let mut inner = AppStateInner::minimal();
    inner.source = "vendor:linksys:live".to_string();
    inner.last_vendor_rf_frame = Some(std::time::Instant::now());
    let state: SharedState = Arc::new(RwLock::new(inner));
    let Json(status) = health_ready(State(state)).await;
    assert_eq!(status["source_state"], "live_unverified");
}

#[tokio::test]
async fn wifi_source_before_first_scan_is_waiting() {
    let status = status_with("wifi", None).await;
    assert_eq!(status["source_state"], "disconnected");
    assert_eq!(status["waiting_for_frames"], true);
}

#[tokio::test]
async fn simulated_source_stays_synthetic() {
    let status = status_with("simulated", None).await;
    assert_eq!(status["source_state"], "synthetic");
    assert_eq!(status["waiting_for_frames"], false);
}

