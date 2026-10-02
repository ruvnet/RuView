//! Sensing-broadcast JSON → [`VitalsSnapshot`] bridge (issues #872, #898,
//! #1541, #2085).
//!
//! `sensing-server` broadcasts several JSON message types on one channel.
//! Only two of them carry data the MQTT publisher can use:
//!
//! - `sensing_update` — one snapshot per physical node (`nodes[]`), or a
//!   single aggregate snapshot when the source has no per-node data.
//! - `edge_vitals` — the ESP32 edge tier's per-node frame. Its
//!   `fall_detected` flag is the only fall source the server has; the bridge
//!   turns its rising edge into a one-shot fall on that node's next snapshot.
//!
//! Every other message type (`edge_fused_vitals`, `wasm_event`, …) is
//! ignored. Before #2085 every message was read as a `sensing_update`, so an
//! `edge_vitals` frame produced a phantom aggregate device whose presence
//! flickered with each frame.
//!
//! Absent fields stay absent (`None`) so the publisher can mark the matching
//! entity unavailable instead of publishing a default value.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;

use super::state::VitalsSnapshot;

/// A node whose last `edge_vitals` frame is older than this no longer counts
/// as having a fall detector; its fall entity goes unavailable.
pub const EDGE_FALL_STALE_AFTER: Duration = Duration::from_secs(30);

#[derive(Debug, Default)]
struct EdgeFall {
    last_seen: Duration,
    prev: bool,
    pending: bool,
}

/// Stateful bridge. Holds the per-node fall edge detector, so one instance
/// lives for the lifetime of the publisher.
#[derive(Debug)]
pub struct SensingBridge {
    base_id: String,
    edge: HashMap<u64, EdgeFall>,
}

impl SensingBridge {
    /// `base_id` prefixes every node id (`<base_id>-node<N>`); it is the MQTT
    /// client id, so it must be stable across restarts (#2093).
    pub fn new(base_id: impl Into<String>) -> Self {
        Self {
            base_id: base_id.into(),
            edge: HashMap::new(),
        }
    }

    /// Map one broadcast message into zero or more snapshots. `now` is a
    /// monotonic time since the bridge started.
    pub fn ingest(&mut self, v: &Value, now: Duration) -> Vec<VitalsSnapshot> {
        match v["type"].as_str() {
            Some("sensing_update") => self.sensing_update(v, now),
            Some("edge_vitals") => {
                self.edge_vitals(v, now);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn edge_vitals(&mut self, v: &Value, now: Duration) {
        let Some(n) = v["node_id"].as_u64() else {
            return;
        };
        let fall = v["fall_detected"].as_bool().unwrap_or(false);
        let e = self.edge.entry(n).or_default();
        e.last_seen = now;
        if fall && !e.prev {
            e.pending = true;
        }
        e.prev = fall;
    }

    /// `Some(true)` once per fall rising edge, `Some(false)` while the node's
    /// edge tier is reporting, `None` when it has no fall source.
    fn take_fall(&mut self, node: u64, now: Duration) -> Option<bool> {
        let e = self.edge.get_mut(&node)?;
        if now.saturating_sub(e.last_seen) >= EDGE_FALL_STALE_AFTER {
            return None;
        }
        Some(std::mem::take(&mut e.pending))
    }

    fn sensing_update(&mut self, v: &Value, now: Duration) -> Vec<VitalsSnapshot> {
        let ts = (v["timestamp"].as_f64().unwrap_or(0.0) * 1000.0) as i64;
        let vit = &v["vital_signs"];
        let breathing = vit["breathing_rate_bpm"].as_f64();
        let hr = vit["heart_rate_bpm"].as_f64();
        // Confidence of the vitals actually present, not the presence
        // classifier's confidence.
        let vital_confidence = [
            breathing.and(vit["breathing_confidence"].as_f64()),
            hr.and(vit["heartbeat_confidence"].as_f64()),
        ]
        .into_iter()
        .flatten()
        .fold(None, |acc: Option<f64>, c| {
            Some(acc.map_or(c, |a| a.min(c)))
        })
        .unwrap_or(0.0);
        let n_persons = v["persons"]
            .as_array()
            .map(|a| a.len() as u32)
            .or_else(|| v["estimated_persons"].as_u64().map(|x| x as u32))
            .unwrap_or(0);

        // Room-level aggregate: the no-nodes fallback, and the per-node default
        // for any field a node omits.
        let acls = &v["classification"];
        let agg_presence = acls["presence"].as_bool().unwrap_or(false);
        let agg_motion = motion_of(acls["motion_level"].as_str(), 0.0);
        let agg_conf = acls["confidence"].as_f64().unwrap_or(0.0);
        let agg_energy = v["features"]["motion_band_power"].as_f64();

        let mk = |node_id: String,
                  presence: bool,
                  motion: f64,
                  conf: f64,
                  rssi: Option<f64>,
                  motion_energy: Option<f64>,
                  fall_detected: Option<bool>| VitalsSnapshot {
            node_id,
            timestamp_ms: ts,
            presence,
            fall_detected,
            motion,
            motion_energy,
            presence_score: if presence { conf.max(0.0) } else { 0.0 },
            breathing_rate_bpm: breathing,
            heartrate_bpm: hr,
            n_persons,
            rssi_dbm: rssi,
            vital_confidence,
        };

        let Some(arr) = v["nodes"].as_array().filter(|a| !a.is_empty()) else {
            return vec![mk(
                self.base_id.clone(),
                agg_presence,
                agg_motion,
                agg_conf,
                None,
                agg_energy,
                None,
            )];
        };
        let node_features = v["node_features"].as_array();
        let mut out = Vec::with_capacity(arr.len());
        for node in arr {
            let n = node["node_id"].as_u64().unwrap_or(0);
            // Each node carries its OWN classification under `node_inference`
            // (ADR-297, issue #1541); defer to the room aggregate only for
            // fields the node omits.
            let ninf = &node["node_inference"];
            let presence = ninf["classification"]
                .as_str()
                .map(|c| c != "absent")
                .unwrap_or(agg_presence);
            let motion = motion_of(ninf["classification"].as_str(), agg_motion);
            let conf = ninf["confidence"].as_f64().unwrap_or(agg_conf);
            let energy = node_features
                .and_then(|nf| nf.iter().find(|f| f["node_id"].as_u64() == Some(n)))
                .and_then(|f| f["features"]["motion_band_power"].as_f64())
                .or(agg_energy);
            let fall = self.take_fall(n, now);
            out.push(mk(
                format!("{}-node{n}", self.base_id),
                presence,
                motion,
                conf,
                node["rssi_dbm"].as_f64(),
                energy,
                fall,
            ));
        }
        out
    }
}

/// motion_level string → motion scalar. `absent` / `present_still` and the
/// legacy `none`/`still`/`idle`/`""` are non-moving; anything else (e.g.
/// `present_moving`, `walking`) is motion. `fallback` is used when the field
/// is absent so a partial per-node payload defers to the room aggregate.
fn motion_of(level: Option<&str>, fallback: f64) -> f64 {
    match level {
        Some("none")
        | Some("still")
        | Some("idle")
        | Some("absent")
        | Some("present_still")
        | Some("") => 0.0,
        Some(_) => 1.0,
        None => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const T0: Duration = Duration::from_secs(100);

    fn snaps(v: Value, base: &str) -> Vec<VitalsSnapshot> {
        SensingBridge::new(base).ingest(&v, T0)
    }

    /// Regression for #872/#898/#1541: each node surfaces its OWN
    /// classification from `nodes[].node_inference` (the shape `NodeInfo`
    /// actually serializes), not the room aggregate.
    #[test]
    fn per_node_presence_uses_each_nodes_own_classification() {
        let v = json!({
            "type": "sensing_update",
            "timestamp": 1.0,
            "classification": { "presence": true, "motion_level": "present_moving", "confidence": 0.9 },
            "vital_signs": { "breathing_rate_bpm": 14.0, "heart_rate_bpm": 60.0,
                             "breathing_confidence": 0.8, "heartbeat_confidence": 0.7 },
            "persons": [{}, {}],
            "nodes": [
                { "node_id": 1, "rssi_dbm": -40.0,
                  "node_inference": { "classification": "present_moving", "confidence": 0.8 } },
                { "node_id": 2, "rssi_dbm": -70.0,
                  "node_inference": { "classification": "absent", "confidence": 0.1 } }
            ]
        });
        let s = snaps(v, "ruview");
        assert_eq!(s.len(), 2, "one snapshot per node");
        let n1 = s.iter().find(|s| s.node_id == "ruview-node1").unwrap();
        let n2 = s.iter().find(|s| s.node_id == "ruview-node2").unwrap();
        assert!(n1.presence && n1.motion > 0.0);
        assert!(
            !n2.presence && n2.motion == 0.0,
            "node2 must not inherit the aggregate"
        );
        assert_eq!(n1.rssi_dbm, Some(-40.0));
        assert_eq!(n2.rssi_dbm, Some(-70.0));
        // Vitals + person count are room-level, shared across node devices.
        assert_eq!((n1.n_persons, n2.n_persons), (2, 2));
        assert_eq!(n1.breathing_rate_bpm, Some(14.0));
        assert_eq!(n2.heartrate_bpm, Some(60.0));
        // Vital confidence is the weakest present vital, not the classifier's.
        assert_eq!(n1.vital_confidence, 0.7);
        assert!(n1.presence_score > 0.0);
        assert_eq!(n2.presence_score, 0.0);
    }

    #[test]
    fn present_still_is_presence_without_motion() {
        let v = json!({
            "type": "sensing_update", "timestamp": 1.0,
            "classification": { "presence": true, "motion_level": "present_moving", "confidence": 0.9 },
            "nodes": [ { "node_id": 4, "rssi_dbm": -50.0,
                         "node_inference": { "classification": "present_still", "confidence": 0.6 } } ]
        });
        let s = snaps(v, "b");
        assert!(s[0].presence);
        assert_eq!(s[0].motion, 0.0);
    }

    #[test]
    fn per_node_missing_fields_fall_back_to_aggregate() {
        let v = json!({
            "type": "sensing_update", "timestamp": 1.0,
            "classification": { "presence": true, "motion_level": "still", "confidence": 0.7 },
            "vital_signs": {},
            "nodes": [ { "node_id": 3, "rssi_dbm": -55.0 } ]
        });
        let s = snaps(v, "n");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].node_id, "n-node3");
        assert!(s[0].presence, "defers to aggregate presence");
        assert_eq!(s[0].motion, 0.0);
    }

    #[test]
    fn falls_back_to_single_aggregate_when_no_nodes() {
        let v = json!({
            "type": "sensing_update", "timestamp": 2.0,
            "classification": { "presence": true, "motion_level": "idle", "confidence": 0.6 },
            "vital_signs": { "breathing_rate_bpm": 12.0 },
            "persons": [{}]
        });
        let s = snaps(v, "ruview");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].node_id, "ruview");
        assert!(s[0].presence);
        assert_eq!(s[0].n_persons, 1);
        assert_eq!(
            s[0].rssi_dbm, None,
            "no per-node RSSI on an aggregate frame"
        );
        assert_eq!(
            s[0].fall_detected, None,
            "no fall source on an aggregate frame"
        );
    }

    #[test]
    fn absent_motion_level_is_zero_motion() {
        let v = json!({
            "type": "sensing_update", "timestamp": 0.0,
            "classification": { "presence": false, "motion_level": "absent", "confidence": 0.0 }
        });
        let s = snaps(v, "x");
        assert_eq!(s[0].motion, 0.0);
        assert!(!s[0].presence);
    }

    /// #2085: `edge_vitals` and other message types must not be read as a
    /// `sensing_update` (that produced a phantom aggregate device).
    #[test]
    fn non_sensing_messages_produce_no_snapshot() {
        let mut b = SensingBridge::new("x");
        for t in [
            "edge_vitals",
            "edge_fused_vitals",
            "wasm_event",
            "pose_data",
        ] {
            let v = json!({ "type": t, "node_id": 1, "presence": true });
            assert!(
                b.ingest(&v, T0).is_empty(),
                "{t} must not become a snapshot"
            );
        }
        assert!(
            b.ingest(&json!({ "presence": true }), T0).is_empty(),
            "untyped"
        );
    }

    #[test]
    fn motion_energy_prefers_per_node_band_power() {
        let v = json!({
            "type": "sensing_update", "timestamp": 1.0,
            "classification": { "presence": true, "motion_level": "present_moving", "confidence": 0.9 },
            "features": { "motion_band_power": 9.0 },
            "nodes": [ { "node_id": 1 }, { "node_id": 2 } ],
            "node_features": [ { "node_id": 1, "features": { "motion_band_power": 3.5 } } ]
        });
        let s = snaps(v, "e");
        assert_eq!(
            s[0].motion_energy,
            Some(3.5),
            "node 1 uses its own band power"
        );
        assert_eq!(
            s[1].motion_energy,
            Some(9.0),
            "node 2 falls back to the aggregate"
        );

        let none = json!({ "type": "sensing_update", "timestamp": 1.0, "classification": {} });
        assert_eq!(snaps(none, "e")[0].motion_energy, None, "never a default 0");
    }

    #[test]
    fn edge_fall_rising_edge_fires_once_on_that_node() {
        let mut b = SensingBridge::new("f");
        let upd = json!({
            "type": "sensing_update", "timestamp": 1.0, "classification": {},
            "nodes": [ { "node_id": 1 }, { "node_id": 2 } ]
        });
        let edge =
            |fall: bool| json!({ "type": "edge_vitals", "node_id": 1, "fall_detected": fall });

        // No edge tier yet: no fall source on either node.
        let s = b.ingest(&upd, T0);
        assert_eq!((s[0].fall_detected, s[1].fall_detected), (None, None));

        b.ingest(&edge(false), T0);
        assert_eq!(b.ingest(&upd, T0)[0].fall_detected, Some(false));

        // Rising edge → exactly one Some(true), on node 1 only.
        b.ingest(&edge(true), T0);
        let s = b.ingest(&upd, T0);
        assert_eq!(s[0].fall_detected, Some(true));
        assert_eq!(s[1].fall_detected, None);
        assert_eq!(b.ingest(&upd, T0)[0].fall_detected, Some(false), "consumed");

        // A held flag does not re-fire; a new edge after it clears does.
        b.ingest(&edge(true), T0);
        assert_eq!(b.ingest(&upd, T0)[0].fall_detected, Some(false));
        b.ingest(&edge(false), T0);
        b.ingest(&edge(true), T0);
        assert_eq!(b.ingest(&upd, T0)[0].fall_detected, Some(true));
    }

    #[test]
    fn edge_fall_source_goes_stale() {
        let mut b = SensingBridge::new("f");
        b.ingest(
            &json!({ "type": "edge_vitals", "node_id": 1, "fall_detected": false }),
            T0,
        );
        let upd = json!({ "type": "sensing_update", "timestamp": 1.0, "classification": {},
                          "nodes": [ { "node_id": 1 } ] });
        assert_eq!(
            b.ingest(&upd, T0 + Duration::from_secs(29))[0].fall_detected,
            Some(false)
        );
        assert_eq!(
            b.ingest(&upd, T0 + EDGE_FALL_STALE_AFTER)[0].fall_detected,
            None
        );
    }
}
