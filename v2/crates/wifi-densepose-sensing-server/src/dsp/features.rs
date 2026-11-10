use std::collections::VecDeque;

use crate::domain::types::{ClassificationInfo, FeatureInfo};
use crate::ingest::esp32_frame::Esp32Frame;
use super::breathing::estimate_breathing_rate_hz;
use super::dominant_freq::estimate_temporal_dominant_frequency_hz;
use super::subcarrier::{compute_subcarrier_importance_weights, compute_subcarrier_variances};

/// Simple threshold classification without temporal smoothing.
pub fn raw_classify(score: f64) -> String {
    if score > 0.25 {
        "active".into()
    } else if score > 0.12 {
        "present_moving".into()
    } else if score > 0.04 {
        "present_still".into()
    } else {
        "absent".into()
    }
}

/// Extract physical features from an ESP32 frame and historical amplitude window.
///
/// Returns:
/// `(FeatureInfo, raw ClassificationInfo, breathing_rate_hz, subcarrier_variances, raw_motion_score)`
pub fn extract_features_from_frame(
    frame: &Esp32Frame,
    frame_history: &VecDeque<Vec<f64>>,
    sample_rate_hz: f64,
) -> (FeatureInfo, ClassificationInfo, f64, Vec<f64>, f64) {
    let n_sub = frame.amplitudes.len().max(1);
    let n = n_sub as f64;
    let mean_rssi = frame.rssi as f64;

    let sub_sensitivity: Vec<f64> = frame.amplitudes.iter().map(|a| a.abs()).collect();
    let importance_weights = compute_subcarrier_importance_weights(&sub_sensitivity);

    let weight_sum: f64 = importance_weights.iter().sum::<f64>();
    let mean_amp: f64 = if weight_sum > 0.0 {
        frame
            .amplitudes
            .iter()
            .zip(importance_weights.iter())
            .map(|(a, w)| a * w)
            .sum::<f64>()
            / weight_sum
    } else {
        frame.amplitudes.iter().sum::<f64>() / n
    };

    let intra_variance: f64 = if weight_sum > 0.0 {
        frame
            .amplitudes
            .iter()
            .zip(importance_weights.iter())
            .map(|(a, w)| w * (a - mean_amp).powi(2))
            .sum::<f64>()
            / weight_sum
    } else {
        frame
            .amplitudes
            .iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>()
            / n
    };

    let sub_variances = compute_subcarrier_variances(frame_history, n_sub);
    let temporal_variance: f64 = if sub_variances.is_empty() {
        intra_variance
    } else {
        sub_variances.iter().sum::<f64>() / sub_variances.len() as f64
    };

    let variance = intra_variance.max(temporal_variance);
    let spectral_power: f64 = frame.amplitudes.iter().map(|a| a * a).sum::<f64>() / n;

    let half = frame.amplitudes.len() / 2;
    let motion_band_power = if half > 0 {
        frame.amplitudes[half..]
            .iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>()
            / (frame.amplitudes.len() - half) as f64
    } else {
        0.0
    };

    let breathing_band_power = if half > 0 {
        frame.amplitudes[..half]
            .iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>()
            / half as f64
    } else {
        0.0
    };

    let dominant_freq_hz =
        estimate_temporal_dominant_frequency_hz(frame_history, sample_rate_hz);

    let threshold = mean_amp * 1.2;
    let change_points = frame
        .amplitudes
        .windows(2)
        .filter(|w| (w[0] < threshold) != (w[1] < threshold))
        .count();

    let temporal_motion_score = if let Some(prev_frame) = frame_history.iter().rev().nth(1) {
        let n_cmp = n_sub.min(prev_frame.len());
        if n_cmp > 0 {
            let diff_energy: f64 = (0..n_cmp)
                .map(|k| (frame.amplitudes[k] - prev_frame[k]).powi(2))
                .sum::<f64>()
                / n_cmp as f64;
            let ref_energy = mean_amp * mean_amp + 1e-9;
            (diff_energy / ref_energy).sqrt().clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else {
        (intra_variance / (mean_amp * mean_amp + 1e-9))
            .sqrt()
            .clamp(0.0, 1.0)
    };

    let variance_motion = (temporal_variance / 10.0).clamp(0.0, 1.0);
    let mbp_motion = (motion_band_power / 25.0).clamp(0.0, 1.0);
    let cp_motion = (change_points as f64 / 15.0).clamp(0.0, 1.0);
    let motion_score = (temporal_motion_score * 0.4
        + variance_motion * 0.2
        + mbp_motion * 0.25
        + cp_motion * 0.15)
        .clamp(0.0, 1.0);

    let snr_db = (frame.rssi as f64 - frame.noise_floor as f64).max(0.0);
    let snr_quality = (snr_db / 40.0).clamp(0.0, 1.0);
    let stability =
        (1.0_f64 - (temporal_variance / (mean_amp * mean_amp + 1e-9)).clamp(0.0_f64, 1.0_f64)).max(0.0_f64);
    let signal_quality = (snr_quality * 0.6 + stability * 0.4).clamp(0.0_f64, 1.0_f64);

    let breathing_rate_hz = estimate_breathing_rate_hz(frame_history, sample_rate_hz);

    let features = FeatureInfo {
        mean_rssi,
        variance,
        motion_band_power,
        breathing_band_power,
        dominant_freq_hz,
        change_points,
        spectral_power,
    };

    let raw_classification = ClassificationInfo {
        motion_level: raw_classify(motion_score),
        presence: motion_score > 0.04,
        confidence: (0.4_f64 + signal_quality * 0.3 + motion_score * 0.3).clamp(0.0_f64, 1.0_f64),
    };

    (
        features,
        raw_classification,
        breathing_rate_hz,
        sub_variances,
        motion_score,
    )
}
