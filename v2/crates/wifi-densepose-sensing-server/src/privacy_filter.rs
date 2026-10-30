//! Server-wide `--privacy-mode` output filter (#2094, #2165).
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
//! The filter is deny-by-default (#2165). A key survives only if
//! [`crate::privacy_fields::FIELD_CLASSES`] classifies it as public; biometric
//! and unclassified keys are removed, at any depth. A frame whose kind isn't
//! in [`crate::privacy_fields::PUBLIC_FRAME_KINDS`] is dropped whole. REST
//! routes are classified too: control-plane routes lose only biometric keys,
//! raw-signal routes are refused, and every other route, including one added
//! later, gets the full filter. Keys are removed, not nulled, so a client
//! can't tell a suppressed value from one the server never had.
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
use serde_json::Value;

use crate::privacy_fields::{field_class, is_public_frame_kind, route_class, FieldClass, RouteClass};

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

/// Keep only keys classified public, recursively. Biometric and unclassified
/// keys are removed.
pub fn redact_value(value: &mut Value) {
    match value {
        Value::Object(map) => map.retain(|key, child| match field_class(key) {
            Some(FieldClass::Public) => {
                redact_value(child);
                true
            }
            Some(FieldClass::IdMap) => {
                redact_id_map(child);
                true
            }
            Some(FieldClass::Biometric) | None => false,
        }),
        Value::Array(items) => items.iter_mut().for_each(redact_value),
        _ => {}
    }
}

/// An id-keyed object: the keys are identifiers, and each entry must be an
/// object or array, which is filtered as usual. Anything else is filtered
/// as an ordinary value.
fn redact_id_map(value: &mut Value) {
    match value {
        Value::Object(map) => map.retain(|key, entry| {
            let structured = matches!(entry, Value::Object(_) | Value::Array(_));
            if !structured || field_class(key) == Some(FieldClass::Biometric) {
                return false;
            }
            redact_value(entry);
            true
        }),
        other => redact_value(other),
    }
}

/// Remove only keys classified biometric, recursively. Used for control-plane
/// routes, which carry no sensing payload.
pub fn redact_biometric_keys(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, _| field_class(key) != Some(FieldClass::Biometric));
            map.values_mut().for_each(redact_biometric_keys);
        }
        Value::Array(items) => items.iter_mut().for_each(redact_biometric_keys),
        _ => {}
    }
}

/// Whether `value` is a frame whose kind (`type` or `event_type`) may not
/// leave the server in privacy mode. A non-string kind counts as withheld.
pub fn is_withheld_frame(value: &Value) -> bool {
    ["type", "event_type"].iter().any(|k| match value.get(k) {
        None => false,
        Some(Value::String(kind)) => !is_public_frame_kind(kind),
        Some(_) => true,
    })
}

/// Redact one serialized JSON frame. `None` when the frame isn't valid JSON
/// or its kind is withheld; either way the caller must drop it.
pub fn redact_json_str(json: &str) -> Option<String> {
    let mut value: Value = serde_json::from_str(json).ok()?;
    if is_withheld_frame(&value) {
        return None;
    }
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

fn withheld_response() -> Response {
    (
        StatusCode::FORBIDDEN,
        axum::Json(serde_json::json!({
            "code": "privacy_mode",
            "detail": "this endpoint serves raw signal and is disabled while --privacy-mode is set",
        })),
    )
        .into_response()
}

/// How a JSON body on a given route was filtered.
enum Filtered {
    Body(Vec<u8>),
    Withheld,
    Failed,
}

fn filter_body(route: RouteClass, bytes: &[u8]) -> Filtered {
    let Ok(mut value) = serde_json::from_slice::<Value>(bytes) else {
        return Filtered::Failed;
    };
    match route {
        RouteClass::Control => redact_biometric_keys(&mut value),
        RouteClass::Withheld => return Filtered::Withheld,
        RouteClass::Sensing if is_withheld_frame(&value) => return Filtered::Withheld,
        RouteClass::Sensing => redact_value(&mut value),
    }
    serde_json::to_vec(&value).map_or(Filtered::Failed, Filtered::Body)
}

/// Axum middleware: in privacy mode, filter every JSON response body by its
/// route's [`RouteClass`]. A no-op when privacy mode is off.
pub async fn redact_json_responses(
    State(filter): State<PrivacyFilter>,
    request: Request,
    next: Next,
) -> Response {
    if !filter.is_enabled() {
        return next.run(request).await;
    }
    let route = route_class(request.uri().path());
    if route == RouteClass::Withheld {
        return withheld_response();
    }
    let response = next.run(request).await;
    if !is_json(&response) {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let filtered = match axum::body::to_bytes(body, MAX_FILTERED_BODY_BYTES).await {
        Ok(bytes) => filter_body(route, &bytes),
        Err(_) => Filtered::Failed,
    };
    match filtered {
        Filtered::Body(bytes) => {
            parts.headers.remove(CONTENT_LENGTH);
            Response::from_parts(parts, Body::from(bytes))
        }
        Filtered::Withheld => withheld_response(),
        Filtered::Failed => {
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
            "presence_score": 0.8,
            "breathing_rate_bpm": 15.0,
            "heartrate_bpm": 72.0,
            "fusion_confidence": 0.9,
            "mmwave": { "hr_bpm": 71.0, "br_bpm": 14.0, "targets": 1 }
        });
        redact_value(&mut v);
        let text = v.to_string();
        for key in ["breathing_rate_bpm", "heartrate_bpm", "hr_bpm", "br_bpm", "fusion_confidence"] {
            assert!(!text.contains(key), "{key} leaked: {text}");
        }
        assert_eq!(v["presence_score"], 0.8);
        assert_eq!(v["mmwave"]["targets"], 1);
    }

    #[test]
    fn redact_strips_unclassified_fields_at_any_depth() {
        let mut v = sensing_update();
        v["sleep_stage"] = json!("rem");
        v["classification"]["respiration_index"] = json!(0.4);
        v["persons"][0]["new_field"] = json!({ "presence": true });
        redact_value(&mut v);
        assert!(v.get("sleep_stage").is_none(), "{v}");
        assert!(v["classification"].get("respiration_index").is_none(), "{v}");
        assert!(v["persons"][0].get("new_field").is_none(), "{v}");
        assert_eq!(v["classification"]["presence"], true);
    }

    #[test]
    fn redact_strips_vitals_derived_fields() {
        let mut v = json!({
            "type": "sensing_update",
            "features": { "mean_rssi": -50.0, "breathing_band_power": 0.2, "dominant_freq_hz": 0.25 },
            "nodes": [{ "node_id": 1, "amplitude": [1.0, 2.0] }],
            "abstention_reason": "vital_quality_gate_failed",
            "numeric_vitals_authorized": false,
        });
        redact_value(&mut v);
        assert_eq!(
            v,
            json!({ "type": "sensing_update", "features": { "mean_rssi": -50.0 }, "nodes": [{ "node_id": 1 }] })
        );
    }

    #[test]
    fn id_map_entries_must_be_structured_and_are_filtered() {
        let mut v = json!({
            "nodes": {
                "1": { "offset_us": 5, "heart_rate_bpm": 60.0, "unknown": 1 },
                "2": 72.0,
                "heart_rate_bpm": { "offset_us": 1 }
            }
        });
        redact_value(&mut v);
        assert_eq!(v, json!({ "nodes": { "1": { "offset_us": 5 } } }));
    }

    #[test]
    fn redact_json_str_fails_closed_and_drops_withheld_kinds() {
        assert!(redact_json_str("not json").is_none());
        let out = redact_json_str(&sensing_update().to_string()).unwrap();
        assert!(!out.contains("heart_rate_bpm"));
        assert!(!out.contains("keypoints"));
        for frame in [
            json!({ "event_type": "mediatek_csi", "device_id": "ab" }),
            json!({ "type": "wasm_event", "node_id": 1, "events": [] }),
            json!({ "type": "sleep_report", "presence": true }),
            json!({ "type": 7, "presence": true }),
        ] {
            assert!(redact_json_str(&frame.to_string()).is_none(), "{frame}");
        }
    }

    #[test]
    fn control_plane_keeps_unclassified_keys_but_not_biometrics() {
        let mut v = json!({ "status": "ok", "uptime": 5, "nested": { "heart_rate_bpm": 60.0 } });
        redact_biometric_keys(&mut v);
        assert_eq!(v, json!({ "status": "ok", "uptime": 5, "nested": {} }));
    }

    fn app(enabled: bool) -> Router {
        Router::new()
            .route("/json", get(|| async { axum::Json(sensing_update()) }))
            .route("/text", get(|| async { "heart_rate_bpm keypoints" }))
            .route("/health", get(|| async { axum::Json(json!({ "uptime": 1, "heart_rate_bpm": 60.0 })) }))
            .route("/api/v1/csi/mediatek/latest", get(|| async { axum::Json(json!({ "status": "no_data" })) }))
            .route("/raw", get(|| async { axum::Json(json!({ "event_type": "realtek_radar", "sequence": 1 })) }))
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
        // Byte-for-byte what the handler wrote.
        let golden = serde_json::to_string(&sensing_update()).unwrap();
        let (_, text) = body(app(false), "/json").await;
        assert_eq!(text, golden);
        for path in ["/health", "/api/v1/csi/mediatek/latest", "/raw"] {
            let (status, _) = body(app(false), path).await;
            assert_eq!(status, StatusCode::OK, "{path}");
        }
    }

    #[tokio::test]
    async fn middleware_filters_by_route_class() {
        // Control plane: unclassified keys stay, biometric keys go.
        let (status, text) = body(app(true), "/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(text, r#"{"uptime":1}"#);
        // Raw-signal routes are refused, with a reason.
        let (status, text) = body(app(true), "/api/v1/csi/mediatek/latest").await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(text.contains(r#""code":"privacy_mode""#), "{text}");
        // A raw frame served from an unlisted route is refused by its kind.
        let (status, text) = body(app(true), "/raw").await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(text.contains(r#""code":"privacy_mode""#), "{text}");
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
