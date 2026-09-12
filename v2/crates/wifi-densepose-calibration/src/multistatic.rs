//! Multistatic fusion (ADR-029 / ADR-151) — combine several *co-located* nodes
//! observing one room.
//!
//! More links = more geometric diversity, so a person hidden from one node's
//! line of sight is caught by another. Each node carries its own room-calibrated
//! [`SpecialistBank`] (its own baseline + anchors); this fuses their per-window
//! readings into a single [`RoomState`]:
//!
//! - **presence** — OR across nodes (any node seeing a person wins);
//! - **posture / breathing / heartbeat** — the highest-*confidence* node (best
//!   viewpoint for that signal that window);
//! - **restlessness** — max (any node detecting movement);
//! - **anomaly / veto** — max / any (a single implausible node vetoes the room);
//! - **stale** — any node's bank stale flags the fused result.
//!
//! This is *same-room* multistatic. Nodes in *different* rooms are a federation
//! concern (ADR-105), not fusion — see ADR-151 §3.3.

use std::collections::BTreeMap;

use crate::bank::SpecialistBank;
use crate::extract::Features;
use crate::geometry::NodeGeometry;
use crate::runtime::{MixtureOfSpecialists, RoomState};
use crate::specialist::SpecialistReading;

/// A bank plus the node's current baseline id (for per-node staleness).
struct NodeEntry {
    mixture: MixtureOfSpecialists,
    baseline_id: String,
}

/// Fuses co-located nodes' specialist banks into one room state.
#[derive(Default)]
pub struct MultiNodeMixture {
    nodes: BTreeMap<u8, NodeEntry>,
}

impl MultiNodeMixture {
    /// Empty fusion set.
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
        }
    }

    /// Register a node's bank. `current_baseline_id` is the baseline the node is
    /// observing now (drift vs the bank's training baseline → STALE).
    pub fn add_node(
        &mut self,
        node_id: u8,
        bank: SpecialistBank,
        current_baseline_id: impl Into<String>,
    ) {
        self.nodes.insert(
            node_id,
            NodeEntry {
                mixture: MixtureOfSpecialists::new(bank),
                baseline_id: current_baseline_id.into(),
            },
        );
    }

    /// Number of registered nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// One node's mixture, so a caller can print the per-node readings next to
    /// the fused state instead of only the fused result. Diagnostics only —
    /// fusion still goes through [`Self::infer`].
    pub fn node_mixture(&self, node_id: u8) -> Option<&MixtureOfSpecialists> {
        self.nodes.get(&node_id).map(|e| &e.mixture)
    }

    /// The transceiver-geometry snapshot a node's bank was trained under
    /// (ADR-152 §2.1.1), if its enrollment recorded one. Threaded through for
    /// the fusion logic; **not used algorithmically yet** — geometry-aware
    /// fusion is the §2.1.2 learned-embedding work (ADR-151 P6).
    pub fn node_geometry(&self, node_id: u8) -> Option<&[NodeGeometry]> {
        self.nodes
            .get(&node_id)
            .map(|e| e.mixture.bank().geometry.as_slice())
            .filter(|g| !g.is_empty())
    }

    /// All registered nodes' geometry snapshots, keyed by node id. Nodes whose
    /// banks carry no geometry are omitted.
    pub fn geometries(&self) -> BTreeMap<u8, &[NodeGeometry]> {
        self.nodes
            .keys()
            .filter_map(|&id| self.node_geometry(id).map(|g| (id, g)))
            .collect()
    }

    /// Fuse per-node feature windows into one room state. Nodes without a feature
    /// entry this window are skipped.
    pub fn infer(&self, per_node: &BTreeMap<u8, Features>) -> RoomState {
        let states: Vec<RoomState> = per_node
            .iter()
            .filter_map(|(id, f)| {
                self.nodes
                    .get(id)
                    .map(|e| e.mixture.infer(f, &e.baseline_id))
            })
            .collect();

        if states.is_empty() {
            return RoomState::default();
        }

        let presence = fuse_presence(&states);
        let anomaly = max_value(states.iter().map(|s| &s.anomaly));
        // Conservative: a single node seeing a physically-implausible signal
        // vetoes the room (anti-hallucination, same as the single-node runtime).
        let vetoed = states.iter().any(|s| s.vetoed);
        let present = presence.as_ref().map(|r| r.value > 0.5).unwrap_or(true);

        // Vitals/posture only when present and not vetoed.
        let (posture, breathing, heartbeat) = if present && !vetoed {
            (
                best_confidence(states.iter().map(|s| &s.posture)),
                best_confidence(states.iter().map(|s| &s.breathing)),
                best_confidence(states.iter().map(|s| &s.heartbeat)),
            )
        } else {
            (None, None, None)
        };

        RoomState {
            presence,
            posture,
            breathing,
            heartbeat,
            restlessness: max_value(states.iter().map(|s| &s.restlessness)),
            anomaly,
            vetoed,
            stale: states.iter().any(|s| s.stale),
        }
    }
}

/// Presence: a person is present if ANY node sees one; confidence = max.
fn fuse_presence(states: &[RoomState]) -> Option<SpecialistReading> {
    let readings: Vec<&SpecialistReading> =
        states.iter().filter_map(|s| s.presence.as_ref()).collect();
    if readings.is_empty() {
        return None;
    }
    let any_present = readings.iter().any(|r| r.value > 0.5);
    let confidence = readings.iter().map(|r| r.confidence).fold(0.0f32, f32::max);
    Some(SpecialistReading {
        kind: readings[0].kind,
        value: if any_present { 1.0 } else { 0.0 },
        confidence,
        label: Some(if any_present { "present" } else { "absent" }.into()),
    })
}

/// Pick the highest-confidence reading across nodes.
fn best_confidence<'a>(
    readings: impl Iterator<Item = &'a Option<SpecialistReading>>,
) -> Option<SpecialistReading> {
    readings
        .flatten()
        .fold(None::<&SpecialistReading>, |best, r| match best {
            Some(b) if b.confidence >= r.confidence => Some(b),
            _ => Some(r),
        })
        .cloned()
}

/// Pick the reading with the maximum value across nodes (movement / anomaly).
fn max_value<'a>(
    readings: impl Iterator<Item = &'a Option<SpecialistReading>>,
) -> Option<SpecialistReading> {
    readings
        .flatten()
        .fold(None::<&SpecialistReading>, |best, r| match best {
            Some(b) if b.value >= r.value => Some(b),
            _ => Some(r),
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::AnchorLabel;
    use crate::extract::AnchorFeature;

    fn af(label: AnchorLabel, variance: f32, motion: f32) -> AnchorFeature {
        AnchorFeature {
            room_id: "r".into(),
            label,
            features: Features {
                mean: 1.0,
                variance,
                motion,
                breathing_score: 0.0,
                breathing_hz: 0.0,
                heart_score: 0.0,
                heart_hz: 0.0,
            },
        }
    }

    fn bank(baseline: &str) -> SpecialistBank {
        let anchors = vec![
            af(AnchorLabel::Empty, 1.0, 0.1),
            af(AnchorLabel::StandStill, 10.0, 0.2),
            af(AnchorLabel::Sit, 6.0, 0.2),
            af(AnchorLabel::SmallMove, 4.0, 1.2),
            af(AnchorLabel::SleepPosture, 3.0, 0.1),
        ];
        SpecialistBank::train("r", baseline, &anchors, 1).unwrap()
    }

    fn live(variance: f32, motion: f32, br_hz: f32, br_score: f32) -> Features {
        Features {
            mean: 1.0,
            variance,
            motion,
            breathing_score: br_score,
            breathing_hz: br_hz,
            heart_score: 0.0,
            heart_hz: 0.0,
        }
    }

    #[test]
    fn two_nodes_register() {
        let mut m = MultiNodeMixture::new();
        m.add_node(1, bank("b1"), "b1");
        m.add_node(2, bank("b2"), "b2");
        assert_eq!(m.node_count(), 2);
    }

    #[test]
    fn geometry_threads_through_to_fusion() {
        let geo1 = vec![NodeGeometry::new(1, "tape-measure")
            .with_position(0.0, 0.0, 1.0)
            .with_distance(2, 3.0)];
        let mut m = MultiNodeMixture::new();
        m.add_node(1, bank("b1").with_geometry(geo1.clone()), "b1");
        m.add_node(2, bank("b1"), "b1"); // no geometry recorded for node 2
        assert_eq!(m.node_geometry(1), Some(geo1.as_slice()));
        assert_eq!(m.node_geometry(2), None, "geometry-free bank reads None");
        assert_eq!(m.node_geometry(9), None, "unknown node reads None");
        let all = m.geometries();
        assert_eq!(all.len(), 1);
        assert_eq!(all.get(&1), Some(&geo1.as_slice()));
    }

    #[test]
    fn presence_or_across_nodes() {
        let mut m = MultiNodeMixture::new();
        m.add_node(1, bank("b1"), "b1");
        m.add_node(2, bank("b1"), "b1");
        // Node 1 sees nobody (low variance), node 2 sees a person (high variance).
        let mut per = BTreeMap::new();
        per.insert(1u8, live(1.0, 0.1, 0.0, 0.0));
        per.insert(2u8, live(12.0, 0.2, 0.3, 0.9));
        let s = m.infer(&per);
        assert_eq!(s.presence.unwrap().value, 1.0, "any node present → present");
        assert!(s.breathing.is_some());
    }

    #[test]
    fn breathing_picks_best_confidence_node() {
        let mut m = MultiNodeMixture::new();
        m.add_node(1, bank("b1"), "b1");
        m.add_node(2, bank("b1"), "b1");
        let mut per = BTreeMap::new();
        // Both present; node 2 has the stronger breathing periodicity.
        per.insert(1u8, live(12.0, 0.2, 0.2, 0.4));
        per.insert(2u8, live(12.0, 0.2, 0.3, 0.95));
        let s = m.infer(&per);
        let br = s.breathing.unwrap();
        assert!((br.value - 18.0).abs() < 0.3, "picked 0.3 Hz node");
        assert!(br.confidence > 0.9);
    }

    #[test]
    fn anomaly_in_one_node_vetoes_room() {
        let mut m = MultiNodeMixture::new();
        m.add_node(1, bank("b1"), "b1");
        m.add_node(2, bank("b1"), "b1");
        let mut per = BTreeMap::new();
        per.insert(1u8, live(12.0, 0.2, 0.3, 0.9));
        per.insert(2u8, live(9000.0, 500.0, 0.0, 0.0)); // wild outlier
        let s = m.infer(&per);
        assert!(s.vetoed);
        assert!(s.breathing.is_none());
    }

    #[test]
    fn stale_node_flags_room() {
        let mut m = MultiNodeMixture::new();
        m.add_node(1, bank("b1"), "b2"); // trained on b1, now observing b2 → stale
        let mut per = BTreeMap::new();
        per.insert(1u8, live(12.0, 0.2, 0.3, 0.9));
        assert!(m.infer(&per).stale);
    }

    #[test]
    fn empty_window_safe() {
        let m = MultiNodeMixture::new();
        let s = m.infer(&BTreeMap::new());
        assert!(s.presence.is_none());
    }
}
