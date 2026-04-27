//! CSI frame parsing, signal field generation, feature extraction,
//! classification, vital signs smoothing, and multi-person estimation.

use std::collections::{HashMap, VecDeque};
use ruvector_mincut::{DynamicMinCut, MinCutBuilder};

use crate::adaptive_classifier;
use crate::types::*;
use crate::vital_signs::VitalSigns;

// ── ESP32 UDP frame parsers ─────────────────────────────────────────────────

/// Parse a 32-byte edge vitals packet (magic 0xC511_0002).
pub fn parse_esp32_vitals(buf: &[u8]) -> Option<Esp32VitalsPacket> {
    if buf.len() < 32 { return None; }
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0002 { return None; }

    let node_id = buf[4];
    let flags = buf[5];
    let breathing_raw = u16::from_le_bytes([buf[6], buf[7]]);
    let heartrate_raw = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let rssi = buf[12] as i8;
    let n_persons = buf[13];
    let motion_energy = f32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
    let presence_score = f32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
    let timestamp_ms = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);

    Some(Esp32VitalsPacket {
        node_id,
        presence: (flags & 0x01) != 0,
        fall_detected: (flags & 0x02) != 0,
        motion: (flags & 0x04) != 0,
        breathing_rate_bpm: breathing_raw as f64 / 100.0,
        heartrate_bpm: heartrate_raw as f64 / 10000.0,
        rssi, n_persons, motion_energy, presence_score, timestamp_ms,
    })
}

/// Parse a WASM output packet (magic 0xC511_0004).
pub fn parse_wasm_output(buf: &[u8]) -> Option<WasmOutputPacket> {
    if buf.len() < 8 { return None; }
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0004 { return None; }

    let node_id = buf[4];
    let module_id = buf[5];
    let event_count = u16::from_le_bytes([buf[6], buf[7]]) as usize;

    let mut events = Vec::with_capacity(event_count);
    let mut offset = 8;
    for _ in 0..event_count {
        if offset + 5 > buf.len() { break; }
        let event_type = buf[offset];
        let value = f32::from_le_bytes([
            buf[offset + 1], buf[offset + 2], buf[offset + 3], buf[offset + 4],
        ]);
        events.push(WasmEvent { event_type, value });
        offset += 5;
    }

    Some(WasmOutputPacket { node_id, module_id, events })
}

pub fn parse_esp32_frame(buf: &[u8]) -> Option<Esp32Frame> {
    if buf.len() < 20 { return None; }
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0001 { return None; }

    let node_id = buf[4];
    let n_antennas = buf[5];
    let n_subcarriers = buf[6];
    let freq_mhz = u16::from_le_bytes([buf[8], buf[9]]);
    let sequence = u32::from_le_bytes([buf[10], buf[11], buf[12], buf[13]]);
    let rssi_raw = buf[14] as i8;
    let rssi = if rssi_raw > 0 { rssi_raw.saturating_neg() } else { rssi_raw };
    let noise_floor = buf[15] as i8;

    let iq_start = 20;
    let n_pairs = n_antennas as usize * n_subcarriers as usize;
    let expected_len = iq_start + n_pairs * 2;
    if buf.len() < expected_len { return None; }

    let mut amplitudes = Vec::with_capacity(n_pairs);
    let mut phases = Vec::with_capacity(n_pairs);
    for k in 0..n_pairs {
        let i_val = buf[iq_start + k * 2] as i8 as f64;
        let q_val = buf[iq_start + k * 2 + 1] as i8 as f64;
        amplitudes.push((i_val * i_val + q_val * q_val).sqrt());
        phases.push(q_val.atan2(i_val));
    }

    Some(Esp32Frame {
        magic, node_id, n_antennas, n_subcarriers, freq_mhz, sequence,
        rssi, noise_floor, amplitudes, phases,
    })
}

// ── Signal field generation ─────────────────────────────────────────────────

pub fn generate_signal_field(
    _mean_rssi: f64, motion_score: f64, breathing_rate_hz: f64,
    signal_quality: f64, subcarrier_variances: &[f64],
) -> SignalField {
    let grid = 20usize;
    let mut values = vec![0.0f64; grid * grid];
    let center = (grid as f64 - 1.0) / 2.0;

    let max_var = subcarrier_variances.iter().cloned().fold(0.0f64, f64::max);
    let norm_factor = if max_var > 1e-9 { max_var } else { 1.0 };
    let n_sub = subcarrier_variances.len().max(1);

    for (k, &var) in subcarrier_variances.iter().enumerate() {
        let weight = (var / norm_factor) * motion_score;
        if weight < 1e-6 { continue; }
        let angle = (k as f64 / n_sub as f64) * 2.0 * std::f64::consts::PI;
        let radius = center * 0.8 * weight.sqrt();
        let hx = center + radius * angle.cos();
        let hz = center + radius * angle.sin();
        for z in 0..grid {
            for x in 0..grid {
                let dx = x as f64 - hx;
                let dz = z as f64 - hz;
                let dist2 = dx * dx + dz * dz;
                let spread = (0.5 + weight * 2.0).max(0.5);
                values[z * grid + x] += weight * (-dist2 / (2.0 * spread * spread)).exp();
            }
        }
    }

    for z in 0..grid {
        for x in 0..grid {
            let dx = x as f64 - center;
            let dz = z as f64 - center;
            let dist = (dx * dx + dz * dz).sqrt();
            let base = signal_quality * (-dist * 0.12).exp();
            values[z * grid + x] += base * 0.3;
        }
    }

    if breathing_rate_hz > 0.05 {
        let ring_r = center * 0.55;
        let ring_width = 1.8f64;
        for z in 0..grid {
            for x in 0..grid {
                let dx = x as f64 - center;
                let dz = z as f64 - center;
                let dist = (dx * dx + dz * dz).sqrt();
                let ring_val = 0.08 * (-(dist - ring_r).powi(2) / (2.0 * ring_width * ring_width)).exp();
                values[z * grid + x] += ring_val;
            }
        }
    }

    let field_max = values.iter().cloned().fold(0.0f64, f64::max);
    let scale = if field_max > 1e-9 { 1.0 / field_max } else { 1.0 };
    for v in &mut values { *v = (*v * scale).clamp(0.0, 1.0); }

    SignalField { grid_size: [grid, 1, grid], values }
}

// ── Feature extraction ──────────────────────────────────────────────────────

pub fn estimate_breathing_rate_hz(frame_history: &VecDeque<Vec<f64>>, sample_rate_hz: f64) -> f64 {
    let n = frame_history.len();
    if n < 6 { return 0.0; }

    let series: Vec<f64> = frame_history.iter()
        .map(|amps| if amps.is_empty() { 0.0 } else { amps.iter().sum::<f64>() / amps.len() as f64 })
        .collect();
    let mean_s = series.iter().sum::<f64>() / n as f64;
    let detrended: Vec<f64> = series.iter().map(|x| x - mean_s).collect();

    let n_candidates = 9usize;
    let f_low = 0.1f64;
    let f_high = 0.5f64;
    let mut best_freq = 0.0f64;
    let mut best_power = 0.0f64;

    for i in 0..n_candidates {
        let freq = f_low + (f_high - f_low) * i as f64 / (n_candidates - 1).max(1) as f64;
        let omega = 2.0 * std::f64::consts::PI * freq / sample_rate_hz;
        let coeff = 2.0 * omega.cos();
        let (mut s_prev2, mut s_prev1) = (0.0f64, 0.0f64);
        for &x in &detrended {
            let s = x + coeff * s_prev1 - s_prev2;
            s_prev2 = s_prev1;
            s_prev1 = s;
        }
        let power = s_prev2 * s_prev2 + s_prev1 * s_prev1 - coeff * s_prev1 * s_prev2;
        if power > best_power { best_power = power; best_freq = freq; }
    }

    let avg_power = {
        let mut total = 0.0f64;
        for i in 0..n_candidates {
            let freq = f_low + (f_high - f_low) * i as f64 / (n_candidates - 1).max(1) as f64;
            let omega = 2.0 * std::f64::consts::PI * freq / sample_rate_hz;
            let coeff = 2.0 * omega.cos();
            let (mut s_prev2, mut s_prev1) = (0.0f64, 0.0f64);
            for &x in &detrended {
                let s = x + coeff * s_prev1 - s_prev2;
                s_prev2 = s_prev1;
                s_prev1 = s;
            }
            total += s_prev2 * s_prev2 + s_prev1 * s_prev1 - coeff * s_prev1 * s_prev2;
        }
        total / n_candidates as f64
    };

    if best_power > avg_power * 3.0 { best_freq.clamp(f_low, f_high) } else { 0.0 }
}

pub fn compute_subcarrier_importance_weights(sensitivity: &[f64]) -> Vec<f64> {
    let n = sensitivity.len();
    if n == 0 { return vec![]; }
    let max_sens = sensitivity.iter().cloned().fold(f64::NEG_INFINITY, f64::max).max(1e-9);
    let mut sorted = sensitivity.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if n % 2 == 0 { (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0 } else { sorted[n / 2] };
    sensitivity.iter()
        .map(|&s| if s >= median { 1.0 + (s / max_sens).min(1.0) } else { 0.5 })
        .collect()
}

pub fn compute_subcarrier_variances(frame_history: &VecDeque<Vec<f64>>, n_sub: usize) -> Vec<f64> {
    if frame_history.is_empty() || n_sub == 0 { return vec![0.0; n_sub]; }
    let n_frames = frame_history.len() as f64;
    let mut means = vec![0.0f64; n_sub];
    let mut sq_means = vec![0.0f64; n_sub];
    for frame in frame_history.iter() {
        for k in 0..n_sub {
            let a = if k < frame.len() { frame[k] } else { 0.0 };
            means[k] += a;
            sq_means[k] += a * a;
        }
    }
    (0..n_sub).map(|k| {
        let mean = means[k] / n_frames;
        let sq_mean = sq_means[k] / n_frames;
        (sq_mean - mean * mean).max(0.0)
    }).collect()
}

pub fn extract_features_from_frame(
    frame: &Esp32Frame, frame_history: &VecDeque<Vec<f64>>, sample_rate_hz: f64,
) -> (FeatureInfo, ClassificationInfo, f64, Vec<f64>, f64) {
    let n_sub = frame.amplitudes.len().max(1);
    let n = n_sub as f64;
    let mean_rssi = frame.rssi as f64;

    let sub_sensitivity: Vec<f64> = frame.amplitudes.iter().map(|a| a.abs()).collect();
    let importance_weights = compute_subcarrier_importance_weights(&sub_sensitivity);
    let weight_sum: f64 = importance_weights.iter().sum::<f64>();

    let mean_amp: f64 = if weight_sum > 0.0 {
        frame.amplitudes.iter().zip(importance_weights.iter())
            .map(|(a, w)| a * w).sum::<f64>() / weight_sum
    } else {
        frame.amplitudes.iter().sum::<f64>() / n
    };

    let intra_variance: f64 = if weight_sum > 0.0 {
        frame.amplitudes.iter().zip(importance_weights.iter())
            .map(|(a, w)| w * (a - mean_amp).powi(2)).sum::<f64>() / weight_sum
    } else {
        frame.amplitudes.iter().map(|a| (a - mean_amp).powi(2)).sum::<f64>() / n
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
        frame.amplitudes[half..].iter().map(|a| (a - mean_amp).powi(2)).sum::<f64>()
            / (frame.amplitudes.len() - half) as f64
    } else { 0.0 };
    let breathing_band_power = if half > 0 {
        frame.amplitudes[..half].iter().map(|a| (a - mean_amp).powi(2)).sum::<f64>() / half as f64
    } else { 0.0 };

    let peak_idx = frame.amplitudes.iter().enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i).unwrap_or(0);
    let dominant_freq_hz = peak_idx as f64 * 0.05;

    let threshold = mean_amp * 1.2;
    let change_points = frame.amplitudes.windows(2)
        .filter(|w| (w[0] < threshold) != (w[1] < threshold)).count();

    let temporal_motion_score = if let Some(prev_frame) = frame_history.back() {
        let n_cmp = n_sub.min(prev_frame.len());
        if n_cmp > 0 {
            let diff_energy: f64 = (0..n_cmp)
                .map(|k| (frame.amplitudes[k] - prev_frame[k]).powi(2)).sum::<f64>() / n_cmp as f64;
            let ref_energy = mean_amp * mean_amp + 1e-9;
            (diff_energy / ref_energy).sqrt().clamp(0.0, 1.0)
        } else { 0.0 }
    } else {
        (intra_variance / (mean_amp * mean_amp + 1e-9)).sqrt().clamp(0.0, 1.0)
    };

    let variance_motion = (temporal_variance / 10.0).clamp(0.0, 1.0);
    let mbp_motion = (motion_band_power / 25.0).clamp(0.0, 1.0);
    let cp_motion = (change_points as f64 / 15.0).clamp(0.0, 1.0);
    let motion_score = (temporal_motion_score * 0.4 + variance_motion * 0.2
        + mbp_motion * 0.25 + cp_motion * 0.15).clamp(0.0, 1.0);

    let snr_db = (frame.rssi as f64 - frame.noise_floor as f64).max(0.0);
    let snr_quality = (snr_db / 40.0).clamp(0.0, 1.0);
    let stability = (1.0 - (temporal_variance / (mean_amp * mean_amp + 1e-9)).clamp(0.0, 1.0)).max(0.0);
    let signal_quality = (snr_quality * 0.6 + stability * 0.4).clamp(0.0, 1.0);

    let breathing_rate_hz = estimate_breathing_rate_hz(frame_history, sample_rate_hz);

    let features = FeatureInfo {
        mean_rssi, variance, motion_band_power, breathing_band_power,
        dominant_freq_hz, change_points, spectral_power,
    };

    let raw_classification = ClassificationInfo {
        motion_level: raw_classify(motion_score),
        presence: motion_score > 0.04,
        confidence: (0.4 + signal_quality * 0.3 + motion_score * 0.3).clamp(0.0, 1.0),
    };

    (features, raw_classification, breathing_rate_hz, sub_variances, motion_score)
}

// ── Classification ──────────────────────────────────────────────────────────

pub fn raw_classify(score: f64) -> String {
    if score > 0.25 { "active".into() }
    else if score > 0.12 { "present_moving".into() }
    else if score > 0.04 { "present_still".into() }
    else { "absent".into() }
}

pub fn smooth_and_classify(state: &mut AppStateInner, raw: &mut ClassificationInfo, raw_motion: f64) {
    state.baseline_frames += 1;
    if state.baseline_frames < BASELINE_WARMUP {
        state.baseline_motion = state.baseline_motion * 0.9 + raw_motion * 0.1;
    } else if raw_motion < state.smoothed_motion + 0.05 {
        state.baseline_motion = state.baseline_motion * (1.0 - BASELINE_EMA_ALPHA)
                              + raw_motion * BASELINE_EMA_ALPHA;
    }
    let adjusted = (raw_motion - state.baseline_motion * 0.7).max(0.0);
    state.smoothed_motion = state.smoothed_motion * (1.0 - MOTION_EMA_ALPHA) + adjusted * MOTION_EMA_ALPHA;
    let sm = state.smoothed_motion;
    let candidate = raw_classify(sm);
    if candidate == state.current_motion_level {
        state.debounce_counter = 0;
        state.debounce_candidate = candidate;
    } else if candidate == state.debounce_candidate {
        state.debounce_counter += 1;
        if state.debounce_counter >= DEBOUNCE_FRAMES {
            state.current_motion_level = candidate;
            state.debounce_counter = 0;
        }
    } else {
        state.debounce_candidate = candidate;
        state.debounce_counter = 1;
    }
    raw.motion_level = state.current_motion_level.clone();
    raw.presence = sm > 0.03;
    raw.confidence = (0.4 + sm * 0.6).clamp(0.0, 1.0);
}

pub fn smooth_and_classify_node(ns: &mut NodeState, raw: &mut ClassificationInfo, raw_motion: f64) {
    ns.baseline_frames += 1;
    if ns.baseline_frames < BASELINE_WARMUP {
        ns.baseline_motion = ns.baseline_motion * 0.9 + raw_motion * 0.1;
    } else if raw_motion < ns.smoothed_motion + 0.05 {
        ns.baseline_motion = ns.baseline_motion * (1.0 - BASELINE_EMA_ALPHA) + raw_motion * BASELINE_EMA_ALPHA;
    }
    let adjusted = (raw_motion - ns.baseline_motion * 0.7).max(0.0);
    ns.smoothed_motion = ns.smoothed_motion * (1.0 - MOTION_EMA_ALPHA) + adjusted * MOTION_EMA_ALPHA;
    let sm = ns.smoothed_motion;
    let candidate = raw_classify(sm);
    if candidate == ns.current_motion_level {
        ns.debounce_counter = 0;
        ns.debounce_candidate = candidate;
    } else if candidate == ns.debounce_candidate {
        ns.debounce_counter += 1;
        if ns.debounce_counter >= DEBOUNCE_FRAMES {
            ns.current_motion_level = candidate;
            ns.debounce_counter = 0;
        }
    } else {
        ns.debounce_candidate = candidate;
        ns.debounce_counter = 1;
    }
    raw.motion_level = ns.current_motion_level.clone();
    raw.presence = sm > 0.03;
    raw.confidence = (0.4 + sm * 0.6).clamp(0.0, 1.0);
}

pub fn adaptive_override(state: &AppStateInner, features: &FeatureInfo, classification: &mut ClassificationInfo) {
    if let Some(ref model) = state.adaptive_model {
        let amps = state.frame_history.back().map(|v| v.as_slice()).unwrap_or(&[]);
        let feat_arr = adaptive_classifier::features_from_runtime(
            &serde_json::json!({
                "variance": features.variance,
                "motion_band_power": features.motion_band_power,
                "breathing_band_power": features.breathing_band_power,
                "spectral_power": features.spectral_power,
                "dominant_freq_hz": features.dominant_freq_hz,
                "change_points": features.change_points,
                "mean_rssi": features.mean_rssi,
            }),
            amps,
        );
        let (label, conf) = model.classify(&feat_arr);
        classification.motion_level = label.to_string();
        classification.presence = label != "absent";
        classification.confidence = (conf * 0.7 + classification.confidence * 0.3).clamp(0.0, 1.0);
    }
}

// ── Vital signs smoothing ───────────────────────────────────────────────────

fn trimmed_mean(buf: &VecDeque<f64>) -> f64 {
    if buf.is_empty() { return 0.0; }
    let mut sorted: Vec<f64> = buf.iter().copied().collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    let trim = n / 4;
    let middle = &sorted[trim..n - trim.max(0)];
    if middle.is_empty() { sorted[n / 2] } else { middle.iter().sum::<f64>() / middle.len() as f64 }
}

pub fn smooth_vitals(state: &mut AppStateInner, raw: &VitalSigns) -> VitalSigns {
    let raw_hr = raw.heart_rate_bpm.unwrap_or(0.0);
    let raw_br = raw.breathing_rate_bpm.unwrap_or(0.0);
    let hr_ok = state.smoothed_hr < 1.0 || (raw_hr - state.smoothed_hr).abs() < HR_MAX_JUMP;
    let br_ok = state.smoothed_br < 1.0 || (raw_br - state.smoothed_br).abs() < BR_MAX_JUMP;
    if hr_ok && raw_hr > 0.0 {
        state.hr_buffer.push_back(raw_hr);
        if state.hr_buffer.len() > VITAL_MEDIAN_WINDOW { state.hr_buffer.pop_front(); }
    }
    if br_ok && raw_br > 0.0 {
        state.br_buffer.push_back(raw_br);
        if state.br_buffer.len() > VITAL_MEDIAN_WINDOW { state.br_buffer.pop_front(); }
    }
    let trimmed_hr = trimmed_mean(&state.hr_buffer);
    let trimmed_br = trimmed_mean(&state.br_buffer);
    if trimmed_hr > 0.0 {
        if state.smoothed_hr < 1.0 { state.smoothed_hr = trimmed_hr; }
        else if (trimmed_hr - state.smoothed_hr).abs() > HR_DEAD_BAND {
            state.smoothed_hr = state.smoothed_hr * (1.0 - VITAL_EMA_ALPHA) + trimmed_hr * VITAL_EMA_ALPHA;
        }
    }
    if trimmed_br > 0.0 {
        if state.smoothed_br < 1.0 { state.smoothed_br = trimmed_br; }
        else if (trimmed_br - state.smoothed_br).abs() > BR_DEAD_BAND {
            state.smoothed_br = state.smoothed_br * (1.0 - VITAL_EMA_ALPHA) + trimmed_br * VITAL_EMA_ALPHA;
        }
    }
    state.smoothed_hr_conf = state.smoothed_hr_conf * 0.92 + raw.heartbeat_confidence * 0.08;
    state.smoothed_br_conf = state.smoothed_br_conf * 0.92 + raw.breathing_confidence * 0.08;
    VitalSigns {
        breathing_rate_bpm: if state.smoothed_br > 1.0 { Some(state.smoothed_br) } else { None },
        heart_rate_bpm: if state.smoothed_hr > 1.0 { Some(state.smoothed_hr) } else { None },
        breathing_confidence: state.smoothed_br_conf,
        heartbeat_confidence: state.smoothed_hr_conf,
        signal_quality: raw.signal_quality,
    }
}

pub fn smooth_vitals_node(ns: &mut NodeState, raw: &VitalSigns) -> VitalSigns {
    let raw_hr = raw.heart_rate_bpm.unwrap_or(0.0);
    let raw_br = raw.breathing_rate_bpm.unwrap_or(0.0);
    let hr_ok = ns.smoothed_hr < 1.0 || (raw_hr - ns.smoothed_hr).abs() < HR_MAX_JUMP;
    let br_ok = ns.smoothed_br < 1.0 || (raw_br - ns.smoothed_br).abs() < BR_MAX_JUMP;
    if hr_ok && raw_hr > 0.0 {
        ns.hr_buffer.push_back(raw_hr);
        if ns.hr_buffer.len() > VITAL_MEDIAN_WINDOW { ns.hr_buffer.pop_front(); }
    }
    if br_ok && raw_br > 0.0 {
        ns.br_buffer.push_back(raw_br);
        if ns.br_buffer.len() > VITAL_MEDIAN_WINDOW { ns.br_buffer.pop_front(); }
    }
    let trimmed_hr = trimmed_mean(&ns.hr_buffer);
    let trimmed_br = trimmed_mean(&ns.br_buffer);
    if trimmed_hr > 0.0 {
        if ns.smoothed_hr < 1.0 { ns.smoothed_hr = trimmed_hr; }
        else if (trimmed_hr - ns.smoothed_hr).abs() > HR_DEAD_BAND {
            ns.smoothed_hr = ns.smoothed_hr * (1.0 - VITAL_EMA_ALPHA) + trimmed_hr * VITAL_EMA_ALPHA;
        }
    }
    if trimmed_br > 0.0 {
        if ns.smoothed_br < 1.0 { ns.smoothed_br = trimmed_br; }
        else if (trimmed_br - ns.smoothed_br).abs() > BR_DEAD_BAND {
            ns.smoothed_br = ns.smoothed_br * (1.0 - VITAL_EMA_ALPHA) + trimmed_br * VITAL_EMA_ALPHA;
        }
    }
    ns.smoothed_hr_conf = ns.smoothed_hr_conf * 0.92 + raw.heartbeat_confidence * 0.08;
    ns.smoothed_br_conf = ns.smoothed_br_conf * 0.92 + raw.breathing_confidence * 0.08;
    VitalSigns {
        breathing_rate_bpm: if ns.smoothed_br > 1.0 { Some(ns.smoothed_br) } else { None },
        heart_rate_bpm: if ns.smoothed_hr > 1.0 { Some(ns.smoothed_hr) } else { None },
        breathing_confidence: ns.smoothed_br_conf,
        heartbeat_confidence: ns.smoothed_hr_conf,
        signal_quality: raw.signal_quality,
    }
}

// ── Multi-person estimation ─────────────────────────────────────────────────

pub fn fuse_multi_node_features(
    current_features: &FeatureInfo, node_states: &HashMap<u8, NodeState>,
) -> FeatureInfo {
    let now = std::time::Instant::now();
    let active: Vec<(&FeatureInfo, f64)> = node_states.values()
        .filter(|ns| ns.last_frame_time.map_or(false, |t| now.duration_since(t).as_secs() < 10))
        .filter_map(|ns| {
            let feat = ns.latest_features.as_ref()?;
            let rssi = ns.rssi_history.back().copied().unwrap_or(-80.0);
            Some((feat, rssi))
        })
        .collect();

    if active.len() <= 1 { return current_features.clone(); }

    let max_rssi = active.iter().map(|(_, r)| *r).fold(f64::NEG_INFINITY, f64::max);
    let weights: Vec<f64> = active.iter()
        .map(|(_, r)| (1.0 + (r - max_rssi + 20.0) / 20.0).clamp(0.1, 1.0)).collect();
    let w_sum: f64 = weights.iter().sum::<f64>().max(1e-9);

    FeatureInfo {
        variance: active.iter().zip(&weights).map(|((f, _), w)| f.variance * w).sum::<f64>() / w_sum,
        motion_band_power: active.iter().zip(&weights).map(|((f, _), w)| f.motion_band_power * w).sum::<f64>() / w_sum,
        breathing_band_power: active.iter().zip(&weights).map(|((f, _), w)| f.breathing_band_power * w).sum::<f64>() / w_sum,
        spectral_power: active.iter().zip(&weights).map(|((f, _), w)| f.spectral_power * w).sum::<f64>() / w_sum,
        dominant_freq_hz: active.iter().zip(&weights).map(|((f, _), w)| f.dominant_freq_hz * w).sum::<f64>() / w_sum,
        change_points: current_features.change_points,
        mean_rssi: active.iter().map(|(f, _)| f.mean_rssi).fold(f64::NEG_INFINITY, f64::max),
    }
}

pub fn compute_person_score(feat: &FeatureInfo) -> f64 {
    let var_norm = (feat.variance / 300.0).clamp(0.0, 1.0);
    let cp_norm = (feat.change_points as f64 / 30.0).clamp(0.0, 1.0);
    let motion_norm = (feat.motion_band_power / 250.0).clamp(0.0, 1.0);
    let sp_norm = (feat.spectral_power / 500.0).clamp(0.0, 1.0);
    var_norm * 0.40 + cp_norm * 0.20 + motion_norm * 0.25 + sp_norm * 0.15
}

pub fn estimate_persons_from_correlation(frame_history: &VecDeque<Vec<f64>>) -> usize {
    let n_frames = frame_history.len();
    if n_frames < 10 { return 1; }

    let window: Vec<&Vec<f64>> = frame_history.iter().rev().take(20).collect();
    let n_sub = window[0].len().min(56);
    if n_sub < 4 { return 1; }
    let k = window.len() as f64;

    let mut means = vec![0.0f64; n_sub];
    let mut variances = vec![0.0f64; n_sub];
    for frame in &window {
        for sc in 0..n_sub.min(frame.len()) { means[sc] += frame[sc] / k; }
    }
    for frame in &window {
        for sc in 0..n_sub.min(frame.len()) { variances[sc] += (frame[sc] - means[sc]).powi(2) / k; }
    }

    let noise_floor = 1.0;
    let active: Vec<usize> = (0..n_sub).filter(|&sc| variances[sc] > noise_floor).collect();
    let m = active.len();
    if m < 3 { return if m == 0 { 0 } else { 1 }; }

    let mut edges: Vec<(u64, u64, f64)> = Vec::new();
    let source = m as u64;
    let sink = (m + 1) as u64;
    let stds: Vec<f64> = active.iter().map(|&sc| variances[sc].sqrt().max(1e-9)).collect();

    for i in 0..m {
        for j in (i + 1)..m {
            let mut cov = 0.0f64;
            for frame in &window {
                let (si, sj) = (active[i], active[j]);
                if si < frame.len() && sj < frame.len() {
                    cov += (frame[si] - means[si]) * (frame[sj] - means[sj]) / k;
                }
            }
            let corr = (cov / (stds[i] * stds[j])).abs();
            if corr > 0.1 {
                let weight = corr * 10.0;
                edges.push((i as u64, j as u64, weight));
                edges.push((j as u64, i as u64, weight));
            }
        }
    }

    let (max_var_idx, _) = active.iter().enumerate()
        .max_by(|(_, &a), (_, &b)| variances[a].partial_cmp(&variances[b]).unwrap())
        .unwrap_or((0, &0));
    let (min_var_idx, _) = active.iter().enumerate()
        .min_by(|(_, &a), (_, &b)| variances[a].partial_cmp(&variances[b]).unwrap())
        .unwrap_or((0, &0));
    if max_var_idx == min_var_idx { return 1; }

    edges.push((source, max_var_idx as u64, 100.0));
    edges.push((min_var_idx as u64, sink, 100.0));

    let mc: DynamicMinCut = match MinCutBuilder::new().exact().with_edges(edges.clone()).build() {
        Ok(mc) => mc,
        Err(_) => return 1,
    };

    let cut_value = mc.min_cut_value();
    let total_edge_weight: f64 = edges.iter()
        .filter(|(s, t, _)| *s != source && *s != sink && *t != source && *t != sink)
        .map(|(_, _, w)| w).sum::<f64>() / 2.0;
    if total_edge_weight < 1e-9 { return 1; }

    let cut_ratio = cut_value / total_edge_weight;
    if cut_ratio > 0.4 { 1 }
    else if cut_ratio > 0.15 { 2 }
    else { 3 }
}

pub fn score_to_person_count(smoothed_score: f64, prev_count: usize) -> usize {
    match prev_count {
        0 | 1 => {
            if smoothed_score > 0.85 { 3 }
            else if smoothed_score > 0.70 { 2 }
            else { 1 }
        }
        2 => {
            if smoothed_score > 0.92 { 3 }
            else if smoothed_score < 0.55 { 1 }
            else { 2 }
        }
        _ => {
            if smoothed_score < 0.55 { 1 }
            else if smoothed_score < 0.78 { 2 }
            else { 3 }
        }
    }
}

/// Generate a simulated ESP32 frame for testing/demo mode.
pub fn generate_simulated_frame(tick: u64) -> Esp32Frame {
    let t = tick as f64 * 0.1;
    let n_sub = 56usize;
    let mut amplitudes = Vec::with_capacity(n_sub);
    let mut phases = Vec::with_capacity(n_sub);
    for i in 0..n_sub {
        let base = 15.0 + 5.0 * (i as f64 * 0.1 + t * 0.3).sin();
        let noise = (i as f64 * 7.3 + t * 13.7).sin() * 2.0;
        amplitudes.push((base + noise).max(0.1));
        phases.push((i as f64 * 0.2 + t * 0.5).sin() * std::f64::consts::PI);
    }
    Esp32Frame {
        magic: 0xC511_0001, node_id: 1, n_antennas: 1, n_subcarriers: n_sub as u8,
        freq_mhz: 2437, sequence: tick as u32,
        rssi: (-40.0 + 5.0 * (t * 0.2).sin()) as i8, noise_floor: -90,
        amplitudes, phases,
    }
}

/// Generate a simple timestamp (epoch seconds) for recording IDs.
pub fn chrono_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
