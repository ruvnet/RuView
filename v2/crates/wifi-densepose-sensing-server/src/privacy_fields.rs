//! `--privacy-mode` classification table (#2165).
//!
//! Privacy mode is deny-by-default: on a sensing surface a JSON key reaches a
//! client only if it is classified [`FieldClass::Public`] here. A field added
//! to a payload without an entry is stripped until someone classifies it, so
//! a new biometric field can't leak by omission. `privacy_filter` applies the
//! table; this module only says what each key, frame kind and route is.
//!
//! "Biometric" covers vital signs, anything derived from them, body pose, and
//! raw signal from which vital signs can be recomputed. Keys are matched by
//! name at any depth, so a key name must mean the same thing everywhere it is
//! used; a name that is public in one payload and biometric in another needs
//! a new, distinct name.

/// What `--privacy-mode` does with a JSON key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldClass {
    /// Not derived from vital signs or pose: kept.
    Public,
    /// Kept. When the value is an object its keys are identifiers (node id,
    /// device id, zone name), not field names; each entry must itself be an
    /// object or array and is filtered like any other value.
    IdMap,
    /// Removed in privacy mode on every surface.
    Biometric,
}

use FieldClass::{Biometric, IdMap, Public};

/// Every JSON key the sensing surfaces emit, with its privacy class. Keep
/// entries grouped by the payload that introduced them.
pub const FIELD_CLASSES: &[(&str, FieldClass)] = &[
    // Envelope, status and error keys shared by frames and REST bodies.
    ("type", Public),
    ("event_type", Public),
    ("timestamp", Public),
    ("timestamp_ms", Public),
    ("timestamp_ns", Public),
    ("source", Public),
    ("tick", Public),
    ("status", Public),
    ("message", Public),
    ("error", Public),
    ("code", Public),
    ("detail", Public),
    ("hint", Public),
    ("age_ms", Public),
    ("total", Public),
    ("active", Public),
    ("clients", Public),
    ("fps", Public),
    // sensing_update
    ("nodes", IdMap),
    ("features", Public),
    ("classification", Public),
    ("signal_field", Public),
    ("calibrated_presence_evidence", Public),
    ("enhanced_motion", Public),
    ("posture", Public),
    ("signal_quality_score", Public),
    ("quality_verdict", Public),
    ("bssid_count", Public),
    ("model_status", Public),
    ("persons", Public),
    ("estimated_persons", Public),
    ("node_features", Public),
    ("room_inference", Public),
    ("classifier", Public),
    ("activity", Public),
    ("vital_signs", Biometric),
    ("enhanced_breathing", Biometric),
    ("pose_keypoints", Biometric),
    // sensing_update.nodes[]
    ("node_id", Public),
    ("rssi_dbm", Public),
    ("position", Public),
    ("subcarrier_count", Public),
    ("device_id", Public),
    ("sync", Public),
    ("node_inference", Public),
    ("mediatek_diagnostics", Public),
    // Per-subcarrier CSI amplitude, re-sent every tick: breathing and heart
    // rate can be recomputed from it.
    ("amplitude", Biometric),
    ("presence_state", Public),
    ("coefficient_of_variation", Public),
    ("baseline_deviation", Public),
    ("sample_count", Public),
    ("span_ms", Public),
    ("offset_us", Public),
    ("is_leader", Public),
    ("is_valid", Public),
    ("smoothed", Public),
    ("sequence", Public),
    ("csi_fps_ema", Public),
    ("csi_fps_samples", Public),
    ("staleness_ms", Public),
    // features (also node_features[].features)
    ("mean_rssi", Public),
    ("variance", Public),
    ("motion_band_power", Public),
    ("change_points", Public),
    ("spectral_power", Public),
    // Power in the breathing band, and the dominant temporal frequency (the
    // breathing frequency when someone is still).
    ("breathing_band_power", Biometric),
    ("dominant_freq_hz", Biometric),
    // classification, node_inference, room_inference
    ("motion_level", Public),
    ("presence", Public),
    ("confidence", Public),
    ("contributing_nodes", Public),
    // signal_field
    ("grid_size", Public),
    ("values", Public),
    // calibrated_presence_evidence
    ("schema", Public),
    ("boot_epoch", Public),
    ("session_id", Public),
    ("model_id", Public),
    ("binding_digest", Public),
    ("source_node_ids", Public),
    ("model_completed_at_unix_ms", Public),
    ("inference_node_id", Public),
    ("source_tick", Public),
    ("observed_at_unix_ms", Public),
    ("inference_method", Public),
    ("person_count", Public),
    // enhanced_motion, model_status
    ("score", Public),
    ("level", Public),
    ("contributing_bssids", Public),
    ("loaded", Public),
    ("layers", Public),
    ("sona_profile", Public),
    // persons[]
    ("id", Public),
    ("bbox", Public),
    ("x", Public),
    ("y", Public),
    ("width", Public),
    ("height", Public),
    ("zone", Public),
    ("motion_score", Public),
    // Coarse posture label ("standing", "lying"), kept as before.
    ("pose", Public),
    ("keypoints", Biometric),
    ("joints_m", Biometric),
    // node_features[]
    ("last_seen_ms", Public),
    ("frame_rate_hz", Public),
    ("stale", Public),
    ("novelty_score", Public),
    // activity (MediaTek fluctuation index; not a vital sign)
    ("room", Public),
    ("devices", IdMap),
    ("index", Public),
    ("peak_30s", Public),
    ("abs_level", Public),
    ("abs_peak_30s", Public),
    ("is_occupancy_estimate", Public),
    ("fast", Public),
    ("slow", Public),
    ("frames_2s", Public),
    ("floor", Public),
    // edge_vitals, edge_fused_vitals, /api/v1/edge-vitals
    ("edge_vitals", Public),
    ("fall_detected", Public),
    ("motion", Public),
    ("n_persons", Public),
    ("person_count_valid", Public),
    ("motion_energy", Public),
    ("presence_score", Public),
    ("rssi", Public),
    ("calibrated_evidence_authorized", Public),
    ("mmwave", Public),
    ("distance_cm", Public),
    ("targets", Public),
    ("breathing_rate_bpm", Biometric),
    ("heart_rate_bpm", Biometric),
    ("heartrate_bpm", Biometric),
    ("hr_bpm", Biometric),
    ("br_bpm", Biometric),
    ("fusion_confidence", Biometric),
    // State of the vitals pipeline: whether rates were published, and why not.
    ("numeric_vitals_authorized", Biometric),
    ("abstention_reason", Biometric),
    // /api/v1/vital-signs
    ("authority", Biometric),
    ("buffer_status", Biometric),
    ("breathing_confidence", Biometric),
    ("heartbeat_confidence", Biometric),
    ("signal_quality", Biometric),
    // pose_data, connection_established
    ("zone_id", Public),
    ("payload", Public),
    ("pose_source", Public),
    ("metadata", Public),
    ("frame_id", Public),
    ("processing_time_ms", Public),
    ("signal_strength", Public),
    ("backend", Public),
    // Pose physics assessment of the skeleton.
    ("physics", Biometric),
    // /api/v1/pose/*
    ("total_persons", Public),
    ("average_confidence", Public),
    ("frames_processed", Public),
    ("total_detections", Public),
    ("zones", IdMap),
    ("activities", Public),
    ("persisted", Public),
    // /api/v1/nodes, /api/v1/stream/status
    ("csi_status", Public),
    ("csi_last_seen_ms", Public),
    ("csi_sequence", Public),
    ("position_configured", Public),
    ("csi_fps", Public),
    ("csi_fps_total", Public),
    // /api/v1/introspection/snapshot and /ws/introspection
    ("frame_count", Public),
    ("regime", Public),
    ("lyapunov_exponent", Public),
    ("attractor_dim", Public),
    ("attractor_confidence", Public),
    ("regime_changed", Public),
    ("top_k_similarity", Public),
    ("signature_id", Public),
    ("above_threshold", Public),
];

/// Frame kinds (`type` or `event_type`) that may leave the server in privacy
/// mode. Any other kind is dropped whole: raw CSI, radar and vendor RF
/// snapshots, and edge WASM events (module-defined values that can't be
/// classified).
pub const PUBLIC_FRAME_KINDS: &[&str] = &[
    "sensing_update",
    "edge_vitals",
    "edge_fused_vitals",
    "pose_data",
    "connection_established",
];

/// What the REST middleware does with a route in privacy mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteClass {
    /// Control plane (health, config, models, training, auth): no sensing
    /// payload. Only [`FieldClass::Biometric`] keys are removed.
    Control,
    /// Raw signal: refused with `403 privacy_mode`.
    Withheld,
    /// Everything else, including any route not listed here: filtered with
    /// [`FIELD_CLASSES`], unclassified keys removed.
    Sensing,
}

/// Route patterns. `*` matches one path segment, a trailing `**` matches the
/// rest of the path (and the bare prefix). Routes not listed are
/// [`RouteClass::Sensing`].
pub const ROUTE_CLASSES: &[(&str, RouteClass)] = &[
    ("/health", RouteClass::Control),
    ("/health/**", RouteClass::Control),
    ("/api/v1/info", RouteClass::Control),
    ("/api/v1/status", RouteClass::Control),
    ("/api/v1/metrics", RouteClass::Control),
    ("/api/v1/rf/vendors", RouteClass::Control),
    ("/api/v1/rf/vendors/*/events", RouteClass::Control),
    ("/api/v1/edge/registry", RouteClass::Control),
    ("/api/v1/model/**", RouteClass::Control),
    ("/api/v1/models/**", RouteClass::Control),
    ("/api/v1/recording/**", RouteClass::Control),
    ("/api/v1/train/**", RouteClass::Control),
    ("/api/v1/adaptive/**", RouteClass::Control),
    ("/api/v1/calibration/**", RouteClass::Control),
    ("/api/v1/pose/calibrate", RouteClass::Control),
    ("/api/v1/pose/calibration/**", RouteClass::Control),
    ("/api/v1/config/**", RouteClass::Control),
    ("/api/v1/ws-ticket", RouteClass::Control),
    ("/oauth/**", RouteClass::Control),
    // RuField events are signed and carry their own governed privacy class
    // (ADR-262); rewriting them would break the signature.
    ("/api/field", RouteClass::Control),
    ("/api/v1/csi/**", RouteClass::Withheld),
    ("/api/v1/radar/**", RouteClass::Withheld),
    ("/api/v1/rf/vendors/latest", RouteClass::Withheld),
    ("/api/v1/rf/vendors/*/latest", RouteClass::Withheld),
    ("/api/v1/wasm-events", RouteClass::Withheld),
];

/// The class of `key`, or `None` when it is not classified (which privacy
/// mode treats as biometric).
pub fn field_class(key: &str) -> Option<FieldClass> {
    FIELD_CLASSES
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, class)| *class)
}

/// Whether a frame of this kind may leave the server in privacy mode.
pub fn is_public_frame_kind(kind: &str) -> bool {
    PUBLIC_FRAME_KINDS.contains(&kind)
}

/// The class of a request path. Unlisted paths are [`RouteClass::Sensing`].
pub fn route_class(path: &str) -> RouteClass {
    ROUTE_CLASSES
        .iter()
        .find(|(pattern, _)| route_matches(pattern, path))
        .map_or(RouteClass::Sensing, |(_, class)| *class)
}

/// Paths (`a.b[].c`) of every key in `value` that [`FIELD_CLASSES`] doesn't
/// classify. Keys under a biometric key are not visited, since the whole
/// subtree is removed; keys of an [`FieldClass::IdMap`] object are ids, not
/// field names. An empty result means every key is classified. Tests use
/// this to fail when a payload grows an unclassified field.
pub fn unclassified_keys(value: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    collect_unclassified(value, "", &mut out);
    out
}

fn collect_unclassified(value: &serde_json::Value, path: &str, out: &mut Vec<String>) {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                match field_class(key) {
                    None => out.push(child_path),
                    Some(Biometric) => {}
                    Some(Public) => collect_unclassified(child, &child_path, out),
                    Some(IdMap) => match child {
                        Value::Object(entries) => {
                            for (id, entry) in entries {
                                collect_unclassified(entry, &format!("{child_path}.{id}"), out);
                            }
                        }
                        other => collect_unclassified(other, &child_path, out),
                    },
                }
            }
        }
        Value::Array(items) => {
            let item_path = format!("{path}[]");
            for item in items {
                collect_unclassified(item, &item_path, out);
            }
        }
        _ => {}
    }
}

fn route_matches(pattern: &str, path: &str) -> bool {
    let mut want = pattern.trim_end_matches('/').split('/');
    let mut have = path.trim_end_matches('/').split('/');
    loop {
        match (want.next(), have.next()) {
            (Some("**"), _) => return true,
            (Some("*"), Some(seg)) if !seg.is_empty() => {}
            (Some(w), Some(h)) if w == h => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_classified_once() {
        for (i, (key, _)) in FIELD_CLASSES.iter().enumerate() {
            assert!(
                !FIELD_CLASSES[i + 1..].iter().any(|(k, _)| k == key),
                "{key} is classified twice"
            );
        }
    }

    #[test]
    fn unclassified_keys_have_no_class() {
        assert_eq!(field_class("presence"), Some(Public));
        assert_eq!(field_class("heart_rate_bpm"), Some(Biometric));
        assert_eq!(field_class("sleep_stage"), None);
    }

    #[test]
    fn unclassified_keys_reports_paths_and_skips_removed_subtrees() {
        let v = serde_json::json!({
            "presence": true,
            "vital_signs": { "anything": 1 },
            "persons": [{ "zone": "a", "gait": 1 }],
            "nodes": { "7": { "offset_us": 1, "drift": 2 } },
            "sleep_stage": "rem"
        });
        let mut found = unclassified_keys(&v);
        found.sort();
        assert_eq!(found, [".nodes.7.drift", ".persons[].gait", ".sleep_stage"]);
    }

    #[test]
    fn routes_default_to_sensing() {
        assert_eq!(route_class("/api/v1/sensing/latest"), RouteClass::Sensing);
        assert_eq!(route_class("/api/v1/some/new/route"), RouteClass::Sensing);
        assert_eq!(route_class("/api/v1/vital-signs"), RouteClass::Sensing);
        assert_eq!(route_class("/health"), RouteClass::Control);
        assert_eq!(route_class("/health/ready"), RouteClass::Control);
        assert_eq!(route_class("/api/v1/models"), RouteClass::Control);
        assert_eq!(route_class("/api/v1/models/abc"), RouteClass::Control);
        assert_eq!(route_class("/api/v1/rf/vendors"), RouteClass::Control);
        assert_eq!(route_class("/api/v1/rf/vendors/plume/events"), RouteClass::Control);
        assert_eq!(route_class("/api/v1/rf/vendors/plume/latest"), RouteClass::Withheld);
        assert_eq!(route_class("/api/v1/rf/vendors/latest"), RouteClass::Withheld);
        assert_eq!(route_class("/api/v1/csi/mediatek/devices/ab/frames"), RouteClass::Withheld);
        assert_eq!(route_class("/api/v1/wasm-events"), RouteClass::Withheld);
        // A look-alike prefix is not a match.
        assert_eq!(route_class("/healthz"), RouteClass::Sensing);
        assert_eq!(route_class("/api/v1/infox"), RouteClass::Sensing);
    }
}
