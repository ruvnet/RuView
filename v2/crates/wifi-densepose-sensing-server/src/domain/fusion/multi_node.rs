//! Multi-node spatial feature fusion across active receivers.
//!
//! Complies with ELMAS.md: <= 350 lines, functional transformation.

use std::collections::HashMap;
use std::time::Instant;

use crate::domain::node::NodeState;
use crate::domain::types::FeatureInfo;

/// Fuse CSI features across all active ESP32 nodes for improved SNR.
///
/// Multi-node weighting strategy:
/// - RSSI-based weighted average for motion, breathing, and spectral power.
/// - Weighted average for variance (preventing person count inflation from single-node outliers).
/// - Weighted average for dominant frequency.
/// - Best RSSI across nodes.
pub fn fuse_multi_node_features(
    current_features: &FeatureInfo,
    node_states: &HashMap<u8, NodeState>,
) -> FeatureInfo {
    let now = Instant::now();
    let active: Vec<(&FeatureInfo, f64)> = node_states
        .values()
        .filter(|ns| {
            ns.last_frame_time
                .is_some_and(|t| now.duration_since(t).as_secs() < 10)
        })
        .filter_map(|ns| {
            let feat = ns.latest_features.as_ref()?;
            let rssi = ns.rssi_history.back().copied().unwrap_or(-80.0);
            Some((feat, rssi))
        })
        .collect();

    if active.len() <= 1 {
        return current_features.clone();
    }

    let max_rssi = active
        .iter()
        .map(|(_, r)| *r)
        .fold(f64::NEG_INFINITY, f64::max);

    let weights: Vec<f64> = active
        .iter()
        .map(|(_, r)| (1.0 + (r - max_rssi + 20.0) / 20.0).clamp(0.1, 1.0))
        .collect();

    let w_sum: f64 = weights.iter().sum::<f64>().max(1e-9);

    FeatureInfo {
        variance: active
            .iter()
            .zip(&weights)
            .map(|((f, _), w)| f.variance * w)
            .sum::<f64>()
            / w_sum,
        motion_band_power: active
            .iter()
            .zip(&weights)
            .map(|((f, _), w)| f.motion_band_power * w)
            .sum::<f64>()
            / w_sum,
        breathing_band_power: active
            .iter()
            .zip(&weights)
            .map(|((f, _), w)| f.breathing_band_power * w)
            .sum::<f64>()
            / w_sum,
        spectral_power: active
            .iter()
            .zip(&weights)
            .map(|((f, _), w)| f.spectral_power * w)
            .sum::<f64>()
            / w_sum,
        dominant_freq_hz: active
            .iter()
            .zip(&weights)
            .map(|((f, _), w)| f.dominant_freq_hz * w)
            .sum::<f64>()
            / w_sum,
        change_points: current_features.change_points,
        mean_rssi: active
            .iter()
            .map(|(f, _)| f.mean_rssi)
            .fold(f64::NEG_INFINITY, f64::max),
    }
}

