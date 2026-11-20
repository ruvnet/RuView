//! Single-link RSSI motion heuristic (#2557), independent of CSI spatial bins.
//!
//! Uses a five-second window in dB, so a fixed RSSI offset cannot change the
//! score. The 0.25 dB noise floor and 1.5 dB score scale are heuristic defaults,
//! not calibrated occupancy thresholds. Stillness cannot prove an empty room.
//! Validation fixtures are SYNTHETIC; no real-device accuracy is claimed.

use std::collections::VecDeque;

/// History includes the current frame. Amplitudes use the host WiFi mapping
/// `10^((rssi_dbm + 100) / 20)`; logarithmic offsets cancel in the variance.
pub(crate) fn motion_score(history: &VecDeque<Vec<f64>>, sample_rate_hz: f64) -> f64 {
    if !sample_rate_hz.is_finite() || sample_rate_hz <= 0.0 {
        return 0.0;
    }
    let window = (sample_rate_hz * 5.0).ceil().clamp(4.0, 100.0) as usize;
    let mut count = 0;
    let mut mean = 0.0;
    let mut sum_squared_deviations = 0.0;
    for frame in history.iter().rev().take(window) {
        // Stop at a modality/cardinality change; do not mix AP vectors or CSI
        // with a single RSSI link. Invalid samples must never imply motion.
        if frame.len() != 1 {
            break;
        }
        let amplitude = frame[0];
        if !amplitude.is_finite() || amplitude <= 0.0 {
            return 0.0;
        }
        let db = 20.0 * amplitude.log10();
        count += 1;
        let delta = db - mean;
        mean += delta / count as f64;
        sum_squared_deviations += delta * (db - mean);
    }
    if count < 4 {
        return 0.0;
    }
    let std_db = (sum_squared_deviations / count as f64).max(0.0).sqrt();
    ((std_db - 0.25) / 1.5).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(rssi: impl IntoIterator<Item = f64>) -> VecDeque<Vec<f64>> {
        rssi.into_iter()
            .map(|db| vec![10.0_f64.powf((db + 100.0) / 20.0)])
            .collect()
    }

    #[test]
    fn constant_and_quantized_quiet_links_do_not_have_a_presence_floor() {
        for base in [-80.0, -56.0, -39.0] {
            assert_eq!(motion_score(&history([base; 100]), 2.0), 0.0);
            let quiet = history((0..100).map(|i| base + if i % 50 == 0 { 1.0 } else { 0.0 }));
            assert!(motion_score(&quiet, 2.0) < 0.04);
        }
    }

    #[test]
    fn one_db_std_motion_survives_the_legacy_baseline_subtraction() {
        let walking = history((0..100).map(|i| -56.0 + if i % 2 == 0 { -1.0 } else { 1.0 }));
        let score = motion_score(&walking, 2.0);
        assert!((score - 0.5).abs() < 1e-10);
        assert!(
            score * (1.0 - 0.7) > 0.12,
            "must remain moving after baseline convergence"
        );
    }

    #[test]
    fn score_is_invariant_to_rssi_level_and_tick_rate() {
        for rate in [1.0, 2.0, 10.0, 20.0] {
            for base in [-80.0, -56.0, -39.0] {
                let samples = history((0..100).map(|i| base + if i % 2 == 0 { -1.0 } else { 1.0 }));
                assert!((motion_score(&samples, rate) - 0.5).abs() < 0.02);
            }
        }
    }

    #[test]
    fn a_level_change_ages_out_in_five_seconds_instead_of_fifty() {
        let mut samples = history([-70.0; 90]);
        samples.extend(history([-40.0; 10]));
        assert_eq!(motion_score(&samples, 2.0), 0.0);
    }

    #[test]
    fn warmup_missing_and_invalid_samples_fail_closed() {
        assert_eq!(motion_score(&history([-40.0; 3]), 2.0), 0.0);
        for bad in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
            let mut samples = history([-40.0; 10]);
            samples.push_back(vec![bad]);
            assert_eq!(motion_score(&samples, 2.0), 0.0);
        }
        for bad_rate in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
            assert_eq!(motion_score(&history([-40.0; 10]), bad_rate), 0.0);
        }
        let mut mixed = history([-60.0; 10]);
        mixed.push_back(vec![10.0, 100.0]);
        mixed.extend(history([-40.0; 3]));
        assert_eq!(motion_score(&mixed, 2.0), 0.0);
    }
}
