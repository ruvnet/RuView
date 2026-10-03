//! Server-wide `--privacy-mode` output filter (#2094).
//!
//! `--privacy-mode` used to be read only by the MQTT publisher, so REST, the
//! WebSocket streams and recordings kept serving heart rate, breathing rate
//! and pose keypoints. This module is the one filter every other output
//! surface applies when the flag is set:
//!
//! - REST: [`redact_json_responses`] rewrites every `application/json`
//!   response body.
//! - WebSocket and recordings: the server calls [`redact_json_str`] on each
//!   broadcast frame before it is sent or written.
//!
//! The suppressed set matches the MQTT filter (`mqtt::privacy`): heart rate,
//! breathing rate and pose keypoints. Presence, motion, person count, zone
//! and the coarse posture label are kept, as they are on MQTT. Keys are
//! removed, not nulled, so a client can't tell a suppressed value from one
//! the server never had.
//!
//! The filter fails closed: a frame or JSON body that can't be parsed is
//! dropped (WebSocket, recording) or replaced with a 500 (REST) rather than
//! passed through unfiltered.

use axum::{
    body::Body,
    extract::{Request, State},
    http::{
        header::{CONTENT_LENGTH, CONTENT_TYPE},
        StatusCode,
    },
    middleware::Next,
    response::{IntoResponse, Response},
};

/// JSON object keys removed, at any depth, from every output in privacy mode.
pub const BIOMETRIC_KEYS: &[&str] = &[
    // Vital signs: the `vital_signs` object carries HR/BR and their
    // confidences; the leaf keys cover edge vitals, fused vitals and the
    // mmWave block.
    "vital_signs",
    "breathing_rate_bpm",
    "heart_rate_bpm",
    "heartrate_bpm",
    "hr_bpm",
    "br_bpm",
    "enhanced_breathing",
    // Pose: per-person skeletons, model keypoints and refined joints.
    "keypoints",
    "pose_keypoints",
    "joints_m",
];

/// Upper bound on a REST body the filter will buffer.
const MAX_FILTERED_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Whether privacy mode is on. Cheap to clone into router state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PrivacyFilter {
    enabled: bool,
}

impl PrivacyFilter {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Remove every [`BIOMETRIC_KEYS`] entry from `value`, recursively.
pub fn redact_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|key, _| !BIOMETRIC_KEYS.contains(&key.as_str()));
            for child in map.values_mut() {
                redact_value(child);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_value(item);
            }
        }
        _ => {}
    }
}

/// Redact one serialized JSON frame. `None` when the frame isn't valid JSON,
/// in which case the caller must drop it.
pub fn redact_json_str(json: &str) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_str(json).ok()?;
    redact_value(&mut value);
    serde_json::to_string(&value).ok()
}

fn is_json(response: &Response) -> bool {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.trim_start().starts_with("application/json"))
}

/// Axum middleware: in privacy mode, strip [`BIOMETRIC_KEYS`] from every JSON
/// response body. A no-op when privacy mode is off.
pub async fn redact_json_responses(
    State(filter): State<PrivacyFilter>,
    request: Request,
    next: Next,
) -> Response {
    let response = next.run(request).await;
    if !filter.is_enabled() || !is_json(&response) {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let redacted = axum::body::to_bytes(body, MAX_FILTERED_BODY_BYTES)
        .await
        .ok()
        .and_then(|bytes| {
            let mut value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
            redact_value(&mut value);
            serde_json::to_vec(&value).ok()
        });
    match redacted {
        Some(bytes) => {
            parts.headers.remove(CONTENT_LENGTH);
            Response::from_parts(parts, Body::from(bytes))
        }
        None => {
            tracing::warn!("privacy mode: could not filter a JSON response; withholding it");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({
                    "code": "privacy_filter_failed",
                    "detail": "response withheld because privacy mode could not filter it",
                })),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use serde_json::json;
    use tower::ServiceExt;

    fn sensing_update() -> serde_json::Value {
        json!({
            "type": "sensing_update",
            "classification": { "presence": true, "motion_level": "active", "confidence": 0.9 },
            "vital_signs": { "breathing_rate_bpm": 14.0, "heart_rate_bpm": 62.0,
                             "breathing_confidence": 0.8, "heartbeat_confidence": 0.7,
                             "signal_quality": 0.9 },
            "enhanced_breathing": { "rate_bpm": 14.1 },
            "pose_keypoints": [[0.1, 0.2, 0.0, 0.9]],
            "posture": "standing",
            "estimated_persons": 1,
            "persons": [{ "id": 1, "confidence": 0.9, "zone": "zone_1", "pose": "standing",
                          "keypoints": [{ "name": "nose", "x": 1.0, "y": 2.0, "z": 0.0, "confidence": 0.9 }] }]
        })
    }

    #[test]
    fn redact_removes_vitals_and_pose_at_any_depth() {
        let mut v = sensing_update();
        redact_value(&mut v);
        assert!(v.get("vital_signs").is_none());
        assert!(v.get("enhanced_breathing").is_none());
        assert!(v.get("pose_keypoints").is_none());
        assert!(v["persons"][0].get("keypoints").is_none());
        // Non-biometric signals survive, as they do on MQTT.
        assert_eq!(v["classification"]["presence"], true);
        assert_eq!(v["estimated_persons"], 1);
        assert_eq!(v["posture"], "standing");
        assert_eq!(v["persons"][0]["zone"], "zone_1");
    }

    #[test]
    fn redact_strips_edge_and_fused_vitals_leaves() {
        let mut v = json!({
            "type": "edge_fused_vitals",
            "presence": true,
            "breathing_rate_bpm": 15.0,
            "heartrate_bpm": 72.0,
            "mmwave": { "hr_bpm": 71.0, "br_bpm": 14.0, "present": true }
        });
        redact_value(&mut v);
        let text = v.to_string();
        for key in ["breathing_rate_bpm", "heartrate_bpm", "hr_bpm", "br_bpm"] {
            assert!(!text.contains(key), "{key} leaked: {text}");
        }
        assert_eq!(v["presence"], true);
        assert_eq!(v["mmwave"]["present"], true);
    }

    #[test]
    fn redact_json_str_fails_closed_on_invalid_json() {
        assert!(redact_json_str("not json").is_none());
        let out = redact_json_str(&sensing_update().to_string()).unwrap();
        assert!(!out.contains("heart_rate_bpm"));
        assert!(!out.contains("keypoints"));
    }

    fn app(enabled: bool) -> Router {
        Router::new()
            .route("/json", get(|| async { axum::Json(sensing_update()) }))
            .route("/text", get(|| async { "heart_rate_bpm keypoints" }))
            .route(
                "/bad",
                get(|| async { ([(CONTENT_TYPE, "application/json")], "{not json") }),
            )
            .layer(axum::middleware::from_fn_with_state(
                PrivacyFilter::new(enabled),
                redact_json_responses,
            ))
    }

    async fn body(app: Router, path: &str) -> (StatusCode, String) {
        let resp = app
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn middleware_strips_json_when_enabled() {
        let (status, text) = body(app(true), "/json").await;
        assert_eq!(status, StatusCode::OK);
        assert!(!text.contains("heart_rate_bpm"), "{text}");
        assert!(!text.contains("keypoints"), "{text}");
        assert!(text.contains("\"presence\":true"), "{text}");
    }

    #[tokio::test]
    async fn middleware_is_a_no_op_when_disabled() {
        let (_, text) = body(app(false), "/json").await;
        assert!(text.contains("heart_rate_bpm"));
        assert!(text.contains("keypoints"));
    }

    #[tokio::test]
    async fn middleware_leaves_non_json_alone_and_withholds_unparseable_json() {
        let (status, text) = body(app(true), "/text").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(text, "heart_rate_bpm keypoints");
        let (status, text) = body(app(true), "/bad").await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(text.contains("privacy_filter_failed"));
    }
}
