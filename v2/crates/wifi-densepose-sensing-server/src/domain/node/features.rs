//! Aggregation of per-node features for telemetry broadcasts.
//!
//! Complies with ELMAS.md: <= 350 lines, functional transform.

use std::collections::HashMap;
use std::time::Instant;

use crate::domain::types::{ClassificationInfo, FeatureInfo, PerNodeFeatureInfo};
use super::constants::ESP32_OFFLINE_TIMEOUT;
use super::state::NodeState;

/// Build a per-node feature snapshot for the WebSocket envelope.
///
/// Returns `None` when the node map is empty (cold start).
pub fn build_node_features(
    node_states: &HashMap<u8, NodeState>,
    now: Instant,
) -> Option<Vec<PerNodeFeatureInfo>> {
    if node_states.is_empty() {
        return None;
    }

    let entries: Vec<PerNodeFeatureInfo> = node_states
        .iter()
        .map(|(&node_id, ns)| {
            let last_seen_ms = ns
                .last_frame_time
                .map(|t| now.saturating_duration_since(t).as_millis() as u64)
                .unwrap_or(u64::MAX);

            let stale = ns
                .last_frame_time
                .map(|t| now.saturating_duration_since(t) > ESP32_OFFLINE_TIMEOUT)
                .unwrap_or(true);

            let features = ns.latest_features.clone().unwrap_or(FeatureInfo {
                mean_rssi: 0.0,
                variance: 0.0,
                motion_band_power: 0.0,
                breathing_band_power: 0.0,
                dominant_freq_hz: 0.0,
                change_points: 0,
                spectral_power: 0.0,
            });

            PerNodeFeatureInfo {
                node_id,
                features,
                classification: ClassificationInfo {
                    motion_level: ns.current_motion_level.clone(),
                    presence: !matches!(ns.current_motion_level.as_str(), "absent"),
                    confidence: ns.smoothed_person_score.clamp(0.0, 1.0),
                },
                rssi_dbm: ns.rssi_history.back().copied().unwrap_or(0.0),
                last_seen_ms,
                frame_rate_hz: ns.measured_sample_rate_hz().unwrap_or(0.0),
                stale,
                novelty_score: ns.last_novelty_score,
            }
        })
        .collect();

    Some(entries)
}

