//! Bridge between sensing-server frame data and signal crate FieldModel
//! for eigenvalue-based person counting.
//!
//! The FieldModel decomposes CSI observations into environmental drift and
//! body perturbation via SVD eigenmodes. When calibrated, perturbation energy
//! provides a physics-grounded occupancy estimate that supplements the
//! score-based heuristic in `score_to_person_count`.

use std::collections::VecDeque;
use wifi_densepose_signal::ruvsense::field_model::{CalibrationStatus, FieldModel, FieldModelConfig};

use super::score_to_person_count;

/// Number of recent frames to feed into perturbation extraction.
const OCCUPANCY_WINDOW: usize = 50;

/// Perturbation energy threshold for detecting a second person.
const ENERGY_THRESH_2: f64 = 12.0;
/// Perturbation energy threshold for detecting a third person.
const ENERGY_THRESH_3: f64 = 25.0;

/// Create a FieldModelConfig for single-link mode (one ESP32 node = one link).
/// n_subcarriers=64 matches the raw ESP32 CSI frame amplitude vector length.
pub fn single_link_config() -> FieldModelConfig {
    FieldModelConfig {
        n_links: 1,
        n_subcarriers: 64,
        ..FieldModelConfig::default()
    }
}

/// Estimate occupancy using the FieldModel when calibrated, falling back
/// to the score-based heuristic otherwise.
///
/// Prefers `estimate_occupancy()` (eigenvalue-based) when the model is
/// calibrated and enough frames are available. Falls back to perturbation
/// energy thresholds, then to the score heuristic.
pub fn occupancy_or_fallback(
    field: &FieldModel,
    frame_history: &VecDeque<Vec<f64>>,
    smoothed_score: f64,
    prev_count: usize,
) -> usize {
    match field.status() {
        CalibrationStatus::Fresh | CalibrationStatus::Stale => {
            let frames: Vec<Vec<f64>> = frame_history
                .iter()
                .rev()
                .take(OCCUPANCY_WINDOW)
                .cloned()
                .collect();

            if frames.is_empty() {
                return score_to_person_count(smoothed_score, prev_count);
            }

            // Try eigenvalue-based occupancy first (best accuracy).
            match field.estimate_occupancy(&frames) {
                Ok(count) => return count,
                Err(_) => {} // fall through to perturbation energy
            }

            // Fallback: perturbation energy thresholds.
            // FieldModel expects [n_links][n_subcarriers] — we use n_links=1.
            let observation = vec![frames[0].clone()];
            match field.extract_perturbation(&observation) {
                Ok(perturbation) => {
                    if perturbation.total_energy > ENERGY_THRESH_3 {
                        3
                    } else if perturbation.total_energy > ENERGY_THRESH_2 {
                        2
                    } else if perturbation.total_energy > 1.0 {
                        1
                    } else {
                        0
                    }
                }
                Err(_) => score_to_person_count(smoothed_score, prev_count),
            }
        }
        _ => score_to_person_count(smoothed_score, prev_count),
    }
}

/// Feed the latest frame to the FieldModel during calibration collection.
///
/// Acts when status is `Uncalibrated` or `Collecting` — the first feed call
/// transitions `Uncalibrated` → `Collecting` inside `feed_calibration` itself.
pub fn maybe_feed_calibration(field: &mut FieldModel, frame_history: &VecDeque<Vec<f64>>) {
    if !matches!(field.status(), CalibrationStatus::Uncalibrated | CalibrationStatus::Collecting) {
        return;
    }
    if let Some(latest) = frame_history.back() {
        // Single-link observation: [1][n_subcarriers]
        let observations = vec![latest.clone()];
        if let Err(e) = field.feed_calibration(&observations) {
            tracing::debug!("FieldModel calibration feed: {e}");
        }
    }
}

/// Parse node positions from a semicolon-delimited string.
///
/// Format: `"x,y,z;x,y,z;..."` where each coordinate is an `f32`.
/// Malformed entries are skipped with a warning log.
pub fn parse_node_positions(input: &str) -> Vec<[f32; 3]> {
    if input.is_empty() {
        return Vec::new();
    }
    input
        .split(';')
        .enumerate()
        .filter_map(|(idx, triplet)| {
            let parts: Vec<&str> = triplet.split(',').collect();
            if parts.len() != 3 {
                tracing::warn!("Skipping malformed node position entry {idx}: '{triplet}' (expected x,y,z)");
                return None;
            }
            match (parts[0].parse::<f32>(), parts[1].parse::<f32>(), parts[2].parse::<f32>()) {
                (Ok(x), Ok(y), Ok(z)) => Some([x, y, z]),
                _ => {
                    tracing::warn!("Skipping unparseable node position entry {idx}: '{triplet}'");
                    None
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use wifi_densepose_signal::ruvsense::field_model::{FieldModel, FieldModelConfig};

    // ── calibration guard fix (issue #496) ───────────────────────────────────

    /// Before the fix, `maybe_feed_calibration` bailed when status==Uncalibrated.
    /// A freshly created FieldModel starts Uncalibrated, so frame_count stayed 0.
    #[test]
    fn test_maybe_feed_calibration_starts_from_uncalibrated() {
        let mut fm = FieldModel::new(single_link_config()).unwrap();
        assert_eq!(fm.status(), CalibrationStatus::Uncalibrated);

        let frame = vec![1.0f64; 64];
        let mut history: VecDeque<Vec<f64>> = VecDeque::new();
        history.push_back(frame);

        maybe_feed_calibration(&mut fm, &history);

        assert_eq!(fm.status(), CalibrationStatus::Collecting,
            "status must transition Uncalibrated→Collecting on first feed");
        assert_eq!(fm.calibration_frame_count(), 1,
            "frame_count must be 1 after one feed");
    }

    /// Feeding multiple frames increments the counter correctly.
    #[test]
    fn test_maybe_feed_calibration_accumulates_frames() {
        let mut fm = FieldModel::new(single_link_config()).unwrap();
        let mut history: VecDeque<Vec<f64>> = VecDeque::new();

        for i in 0..10u32 {
            let frame: Vec<f64> = (0..64).map(|j| (i * 64 + j) as f64).collect();
            history.push_back(frame);
            maybe_feed_calibration(&mut fm, &history);
        }

        assert_eq!(fm.calibration_frame_count(), 10);
    }

    /// `maybe_feed_calibration` must NOT run when status is Fresh (calibrated).
    #[test]
    fn test_maybe_feed_calibration_skips_when_fresh() {
        // Build a model that can finalize after just 5 frames.
        let cfg = FieldModelConfig {
            n_links: 1,
            n_subcarriers: 64,
            min_calibration_frames: 5,
            ..FieldModelConfig::default()
        };
        let mut fm = FieldModel::new(cfg).unwrap();

        // Feed 5 frames directly to reach Collecting state.
        for _ in 0..5 {
            fm.feed_calibration(&[vec![0.5f64; 64]]).unwrap();
        }
        fm.finalize_calibration(1_000_000, 0xDEAD).unwrap();
        assert_eq!(fm.status(), CalibrationStatus::Fresh);

        let count_before = fm.calibration_frame_count();

        // Now call maybe_feed_calibration — it must be a no-op when Fresh.
        let mut history: VecDeque<Vec<f64>> = VecDeque::new();
        history.push_back(vec![1.0f64; 64]);
        maybe_feed_calibration(&mut fm, &history);

        assert_eq!(fm.calibration_frame_count(), count_before,
            "maybe_feed_calibration must not increment frame_count when status is Fresh");
    }

    // ── subcarrier count fix (issue #496) ────────────────────────────────────

    /// single_link_config must use 64 subcarriers to match ESP32 CSI frame size.
    /// Before the fix it used the default (56), causing DimensionMismatch on every feed.
    #[test]
    fn test_single_link_config_has_64_subcarriers() {
        let cfg = single_link_config();
        assert_eq!(cfg.n_subcarriers, 64,
            "n_subcarriers must be 64 to match ESP32 CSI amplitude vector length");
        assert_eq!(cfg.n_links, 1);
    }

    /// Feeding a 64-element frame must succeed (no DimensionMismatch).
    #[test]
    fn test_feed_64_subcarrier_frame_succeeds() {
        let mut fm = FieldModel::new(single_link_config()).unwrap();
        let frame = vec![0.5f64; 64];
        let mut history = VecDeque::new();
        history.push_back(frame);

        maybe_feed_calibration(&mut fm, &history);

        assert_eq!(fm.calibration_frame_count(), 1,
            "a 64-subcarrier frame must be accepted without DimensionMismatch");
    }

    /// Feeding a 56-element frame (old default) must NOT increment frame_count.
    #[test]
    fn test_feed_56_subcarrier_frame_rejected() {
        let mut fm = FieldModel::new(single_link_config()).unwrap();
        // Status starts Uncalibrated — force it to Collecting first with a good frame
        let good_frame = vec![0.5f64; 64];
        let mut history = VecDeque::new();
        history.push_back(good_frame);
        maybe_feed_calibration(&mut fm, &history);
        assert_eq!(fm.calibration_frame_count(), 1);

        // Now push a short (56-elem) frame — must be silently dropped, not crash
        history.clear();
        history.push_back(vec![0.5f64; 56]);
        maybe_feed_calibration(&mut fm, &history);
        assert_eq!(fm.calibration_frame_count(), 1,
            "56-subcarrier frame must be rejected — count must not increase");
    }

    #[test]
    fn test_parse_node_positions() {
        let positions = parse_node_positions("0,0,1.5;3,0,1.5;1.5,3,1.5");
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], [0.0, 0.0, 1.5]);
        assert_eq!(positions[1], [3.0, 0.0, 1.5]);
        assert_eq!(positions[2], [1.5, 3.0, 1.5]);
    }

    #[test]
    fn test_parse_node_positions_empty() {
        let positions = parse_node_positions("");
        assert!(positions.is_empty());
    }

    #[test]
    fn test_parse_node_positions_invalid() {
        let positions = parse_node_positions("abc;1,2,3");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_parse_node_positions_partial_triplet() {
        let positions = parse_node_positions("1,2;3,4,5");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], [3.0, 4.0, 5.0]);
    }
}
