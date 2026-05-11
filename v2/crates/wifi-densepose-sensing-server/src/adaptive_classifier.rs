//! Adaptive CSI Activity Classifier
//!
//! Learns environment-specific classification thresholds from labeled JSONL
//! recordings.  Uses a lightweight approach:
//!
//! 1. **Feature statistics**: per-class mean/stddev for each of 7 CSI features
//! 2. **Mahalanobis-like distance**: weighted distance to each class centroid
//! 3. **Logistic regression weights**: learned via gradient descent on the
//!    labeled data for fine-grained boundary tuning
//!
//! The trained model is serialised as JSON and hot-loaded at runtime so that
//! the classification thresholds adapt to the specific room and ESP32 placement.
//!
//! Classes are discovered dynamically from training data filenames instead of
//! being hardcoded, so new activity classes can be added just by recording data
//! with the appropriate filename convention.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

// ── Feature vector ───────────────────────────────────────────────────────────

/// Extended feature vector: 7 server features + 8 subcarrier-derived features = 15.
pub const N_FEATURES: usize = 15;

/// Temporal classifier window size. The calibration bursts showed that roughly
/// 4-second windows separate absent/moving much better than single frames.
pub const TEMPORAL_WINDOW_SIZE: usize = 512;
pub const N_TEMPORAL_FEATURES: usize = N_FEATURES * 6;

/// Default class names for backward compatibility with old saved models.
const DEFAULT_CLASSES: &[&str] = &["absent", "present_still", "present_moving", "active"];

/// Extract extended feature vector from a JSONL frame (features + raw amplitudes).
pub fn features_from_frame(frame: &serde_json::Value) -> [f64; N_FEATURES] {
    let feat = frame
        .get("features")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let nodes = frame.get("nodes").and_then(|n| n.as_array());
    let amps: Vec<f64> = nodes
        .and_then(|ns| ns.first())
        .and_then(|n| n.get("amplitude"))
        .and_then(|a| a.as_array())
        .map(|arr| values_to_f64_vec(arr))
        .unwrap_or_default();

    features_from_json_and_amplitudes(&feat, &amps)
}

fn features_from_node_frame(
    frame: &serde_json::Value,
    node_feature: &serde_json::Value,
) -> [f64; N_FEATURES] {
    let node_id = node_feature.get("node_id").and_then(|v| v.as_u64());
    let feat = node_feature
        .get("features")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let amps = frame
        .get("nodes")
        .and_then(|n| n.as_array())
        .and_then(|nodes| {
            nodes.iter().find(|node| {
                node_id.is_some_and(|id| node.get("node_id").and_then(|v| v.as_u64()) == Some(id))
            })
        })
        .and_then(|node| node.get("amplitude"))
        .and_then(|a| a.as_array())
        .map(|arr| values_to_f64_vec(arr))
        .unwrap_or_default();

    features_from_json_and_amplitudes(&feat, &amps)
}

fn values_to_f64_vec(values: &[serde_json::Value]) -> Vec<f64> {
    values.iter().filter_map(|v| v.as_f64()).collect()
}

fn features_from_json_and_amplitudes(feat: &serde_json::Value, amps: &[f64]) -> [f64; N_FEATURES] {
    // Server-computed features (0-6).
    let variance = feat.get("variance").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let mbp = feat
        .get("motion_band_power")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let bbp = feat
        .get("breathing_band_power")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let sp = feat
        .get("spectral_power")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let df = feat
        .get("dominant_freq_hz")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let cp = feat
        .get("change_points")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let rssi = feat
        .get("mean_rssi")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    // Subcarrier-derived features (7-14).
    let (amp_mean, amp_std, amp_skew, amp_kurt, amp_iqr, amp_entropy, amp_max, amp_range) =
        subcarrier_stats(amps);

    [
        variance,
        mbp,
        bbp,
        sp,
        df,
        cp,
        rssi,
        amp_mean,
        amp_std,
        amp_skew,
        amp_kurt,
        amp_iqr,
        amp_entropy,
        amp_max,
        amp_range,
    ]
}

/// Also keep a simpler version for runtime (no JSONL, just FeatureInfo + amps).
pub fn features_from_runtime(feat: &serde_json::Value, amps: &[f64]) -> [f64; N_FEATURES] {
    let variance = feat.get("variance").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let mbp = feat
        .get("motion_band_power")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let bbp = feat
        .get("breathing_band_power")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let sp = feat
        .get("spectral_power")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let df = feat
        .get("dominant_freq_hz")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let cp = feat
        .get("change_points")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let rssi = feat
        .get("mean_rssi")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let (amp_mean, amp_std, amp_skew, amp_kurt, amp_iqr, amp_entropy, amp_max, amp_range) =
        subcarrier_stats(amps);
    [
        variance,
        mbp,
        bbp,
        sp,
        df,
        cp,
        rssi,
        amp_mean,
        amp_std,
        amp_skew,
        amp_kurt,
        amp_iqr,
        amp_entropy,
        amp_max,
        amp_range,
    ]
}

/// Compute statistical features from raw subcarrier amplitudes.
fn subcarrier_stats(amps: &[f64]) -> (f64, f64, f64, f64, f64, f64, f64, f64) {
    if amps.is_empty() {
        return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    }
    let n = amps.len() as f64;
    let mean = amps.iter().sum::<f64>() / n;
    let var = amps.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / n;
    let std = var.sqrt().max(1e-9);

    // Skewness (asymmetry).
    let skew = amps.iter().map(|a| ((a - mean) / std).powi(3)).sum::<f64>() / n;
    // Kurtosis (peakedness).
    let kurt = amps.iter().map(|a| ((a - mean) / std).powi(4)).sum::<f64>() / n - 3.0;

    // IQR (inter-quartile range).
    let mut sorted = amps.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q1 = sorted[sorted.len() / 4];
    let q3 = sorted[3 * sorted.len() / 4];
    let iqr = q3 - q1;

    // Spectral entropy (normalised).
    let total_power: f64 = amps.iter().map(|a| a * a).sum::<f64>().max(1e-9);
    let entropy: f64 = amps
        .iter()
        .map(|a| {
            let p = (a * a) / total_power;
            if p > 1e-12 {
                -p * p.ln()
            } else {
                0.0
            }
        })
        .sum::<f64>()
        / n.ln().max(1e-9); // normalise to [0,1]

    let max_val = sorted.last().copied().unwrap_or(0.0);
    let range = max_val - sorted.first().copied().unwrap_or(0.0);

    (mean, std, skew, kurt, iqr, entropy, max_val, range)
}

// ── Per-class statistics ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassStats {
    pub label: String,
    pub count: usize,
    pub mean: [f64; N_FEATURES],
    pub stddev: [f64; N_FEATURES],
}

// ── Temporal window model ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalModel {
    /// Number of per-frame feature vectors summarised for training/runtime.
    pub window_size: usize,
    /// Class names in centroid order.
    pub class_names: Vec<String>,
    /// Standardised temporal centroids: [n_classes x N_TEMPORAL_FEATURES].
    pub class_centroids: Vec<Vec<f64>>,
    /// Global temporal feature normalisation.
    pub global_mean: Vec<f64>,
    pub global_std: Vec<f64>,
}

impl TemporalModel {
    #[cfg(test)]
    fn from_centroids_for_tests(
        window_size: usize,
        class_names: Vec<String>,
        class_centroids: Vec<Vec<f64>>,
        global_mean: Vec<f64>,
        global_std: Vec<f64>,
    ) -> Self {
        Self {
            window_size,
            class_names,
            class_centroids,
            global_mean,
            global_std,
        }
    }

    pub fn classify(&self, history: &VecDeque<[f64; N_FEATURES]>) -> Option<(String, f64)> {
        let temporal = temporal_features_from_history(history, self.window_size)?;
        if self.class_centroids.is_empty() {
            return None;
        }
        let mut x = vec![0.0; temporal.len()];
        for i in 0..temporal.len() {
            let mu = self.global_mean.get(i).copied().unwrap_or(0.0);
            let sd = self.global_std.get(i).copied().unwrap_or(1.0).max(1e-9);
            x[i] = (temporal[i] - mu) / sd;
        }

        let distances: Vec<f64> = self
            .class_centroids
            .iter()
            .map(|centroid| {
                let d = x.len().min(centroid.len()).max(1);
                x.iter()
                    .zip(centroid)
                    .take(d)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    / d as f64
            })
            .collect();
        let (best_idx, best_dist) = distances
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))?;

        // Softmax over negative average squared distance. The scale factor keeps
        // the confidence useful for nearest-centroid distances in standardised space.
        let scale = 0.25;
        let max_score = distances
            .iter()
            .map(|d| -d / scale)
            .fold(f64::NEG_INFINITY, f64::max);
        let exp_sum: f64 = distances
            .iter()
            .map(|d| ((-d / scale) - max_score).exp())
            .sum();
        let confidence = (((-*best_dist / scale) - max_score).exp() / exp_sum).clamp(0.0, 1.0);
        let label = self
            .class_names
            .get(best_idx)
            .cloned()
            .unwrap_or_else(|| "present_still".to_string());
        Some((label, confidence))
    }
}

pub fn temporal_features_from_history(
    history: &VecDeque<[f64; N_FEATURES]>,
    window_size: usize,
) -> Option<Vec<f64>> {
    if history.len() < window_size || window_size == 0 {
        return None;
    }
    let start = history.len() - window_size;
    let window: Vec<&[f64; N_FEATURES]> = history.iter().skip(start).collect();
    Some(summarize_temporal_window(&window))
}

fn summarize_temporal_window(window: &[&[f64; N_FEATURES]]) -> Vec<f64> {
    let n = window.len().max(1) as f64;
    let mut mean = vec![0.0; N_FEATURES];
    let mut min_v = vec![f64::INFINITY; N_FEATURES];
    let mut max_v = vec![f64::NEG_INFINITY; N_FEATURES];
    for frame in window {
        for i in 0..N_FEATURES {
            mean[i] += frame[i];
            min_v[i] = min_v[i].min(frame[i]);
            max_v[i] = max_v[i].max(frame[i]);
        }
    }
    for v in &mut mean {
        *v /= n;
    }

    let mut std = vec![0.0; N_FEATURES];
    for frame in window {
        for i in 0..N_FEATURES {
            std[i] += (frame[i] - mean[i]).powi(2);
        }
    }
    for v in &mut std {
        *v = (*v / n).sqrt();
    }

    let first = window.first().copied().unwrap();
    let last = window.last().copied().unwrap();
    let mut delta = vec![0.0; N_FEATURES];
    for i in 0..N_FEATURES {
        delta[i] = last[i] - first[i];
    }

    let mut mad = vec![0.0; N_FEATURES];
    if window.len() > 1 {
        for pair in window.windows(2) {
            for i in 0..N_FEATURES {
                mad[i] += (pair[1][i] - pair[0][i]).abs();
            }
        }
        let denom = (window.len() - 1) as f64;
        for v in &mut mad {
            *v /= denom;
        }
    }

    [mean, std, min_v, max_v, delta, mad].concat()
}

fn train_temporal_model(
    samples: &[Sample],
    n_classes: usize,
    class_names: &[String],
) -> Option<TemporalModel> {
    let mut temporal_samples: Vec<(Vec<f64>, usize)> = Vec::new();
    let mut by_sequence: HashMap<(usize, String), Vec<[f64; N_FEATURES]>> = HashMap::new();
    for s in samples {
        if s.class_idx < n_classes {
            by_sequence
                .entry((s.class_idx, s.sequence_key.clone()))
                .or_default()
                .push(s.features);
        }
    }

    for ((class_idx, _sequence_key), frames) in by_sequence.iter() {
        if frames.len() < TEMPORAL_WINDOW_SIZE {
            continue;
        }
        for start in (0..=frames.len() - TEMPORAL_WINDOW_SIZE).step_by(TEMPORAL_WINDOW_SIZE) {
            let window: Vec<&[f64; N_FEATURES]> =
                frames[start..start + TEMPORAL_WINDOW_SIZE].iter().collect();
            temporal_samples.push((summarize_temporal_window(&window), *class_idx));
        }
    }
    if temporal_samples.len() < n_classes {
        return None;
    }

    let n = temporal_samples.len();
    let d = N_TEMPORAL_FEATURES;
    let mut global_mean = vec![0.0; d];
    for (x, _) in &temporal_samples {
        for i in 0..d {
            global_mean[i] += x[i];
        }
    }
    for v in &mut global_mean {
        *v /= n as f64;
    }
    let mut global_std = vec![0.0; d];
    for (x, _) in &temporal_samples {
        for i in 0..d {
            global_std[i] += (x[i] - global_mean[i]).powi(2);
        }
    }
    for v in &mut global_std {
        *v = (*v / n as f64).sqrt().max(1e-9);
    }

    let mut sums = vec![vec![0.0; d]; n_classes];
    let mut counts = vec![0usize; n_classes];
    for (x, c) in &temporal_samples {
        counts[*c] += 1;
        for i in 0..d {
            sums[*c][i] += (x[i] - global_mean[i]) / global_std[i];
        }
    }
    let mut class_centroids = vec![vec![0.0; d]; n_classes];
    for c in 0..n_classes {
        if counts[c] == 0 {
            continue;
        }
        for i in 0..d {
            class_centroids[c][i] = sums[c][i] / counts[c] as f64;
        }
    }
    eprintln!(
        "Temporal model: {} windows across {} classes",
        temporal_samples.len(),
        n_classes
    );
    Some(TemporalModel {
        window_size: TEMPORAL_WINDOW_SIZE,
        class_names: class_names.to_vec(),
        class_centroids,
        global_mean,
        global_std,
    })
}

// ── Trained model ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveModel {
    /// Per-class feature statistics (centroid + spread).
    pub class_stats: Vec<ClassStats>,
    /// Logistic regression weights: [n_classes x (N_FEATURES + 1)] (last = bias).
    /// Dynamic: the outer Vec length equals the number of discovered classes.
    pub weights: Vec<Vec<f64>>,
    /// Global feature normalisation: mean and stddev across all training data.
    pub global_mean: [f64; N_FEATURES],
    pub global_std: [f64; N_FEATURES],
    /// Training metadata.
    pub trained_frames: usize,
    pub training_accuracy: f64,
    pub version: u32,
    /// Dynamically discovered class names (in index order).
    #[serde(default = "default_class_names")]
    pub class_names: Vec<String>,
    /// Optional nearest-centroid temporal model over rolling feature windows.
    #[serde(default)]
    pub temporal_model: Option<TemporalModel>,
}

/// Backward-compatible fallback for models saved without class_names.
fn default_class_names() -> Vec<String> {
    DEFAULT_CLASSES.iter().map(|s| s.to_string()).collect()
}

impl Default for AdaptiveModel {
    fn default() -> Self {
        let n_classes = DEFAULT_CLASSES.len();
        Self {
            class_stats: Vec::new(),
            weights: vec![vec![0.0; N_FEATURES + 1]; n_classes],
            global_mean: [0.0; N_FEATURES],
            global_std: [1.0; N_FEATURES],
            trained_frames: 0,
            training_accuracy: 0.0,
            version: 1,
            class_names: default_class_names(),
            temporal_model: None,
        }
    }
}

impl AdaptiveModel {
    /// Classify a raw feature vector.  Returns (class_label, confidence).
    pub fn classify(&self, raw_features: &[f64; N_FEATURES]) -> (String, f64) {
        let n_classes = self.weights.len();
        if n_classes == 0 || self.class_stats.is_empty() {
            return ("present_still".to_string(), 0.5);
        }

        // Normalise features.
        let mut x = [0.0f64; N_FEATURES];
        for i in 0..N_FEATURES {
            x[i] = (raw_features[i] - self.global_mean[i]) / (self.global_std[i] + 1e-9);
        }

        // Compute logits: w·x + b for each class.
        let mut logits: Vec<f64> = vec![0.0; n_classes];
        for c in 0..n_classes {
            let w = &self.weights[c];
            let mut z = w[N_FEATURES]; // bias
            for i in 0..N_FEATURES {
                z += w[i] * x[i];
            }
            logits[c] = z;
        }

        // Softmax.
        let max_logit = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp_sum: f64 = logits.iter().map(|z| (z - max_logit).exp()).sum();
        let mut probs: Vec<f64> = vec![0.0; n_classes];
        for c in 0..n_classes {
            probs[c] = ((logits[c] - max_logit).exp()) / exp_sum;
        }

        // Pick argmax.
        let (best_c, best_p) = probs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();
        let label = if best_c < self.class_names.len() {
            self.class_names[best_c].clone()
        } else {
            "present_still".to_string()
        };
        (label, *best_p)
    }

    /// Classify a rolling temporal history if this model has temporal centroids.
    pub fn classify_temporal(
        &self,
        history: &VecDeque<[f64; N_FEATURES]>,
    ) -> Option<(String, f64)> {
        self.temporal_model.as_ref()?.classify(history)
    }

    /// Save model to a JSON file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    /// Load model from a JSON file.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

// ── Training ─────────────────────────────────────────────────────────────────

/// A labeled training sample.
struct Sample {
    features: [f64; N_FEATURES],
    class_idx: usize,
    /// Stable sequence key used for temporal training windows. RuView multi-node
    /// JSONL lines are expanded to one sample per node; temporal windows must
    /// not mix nodes or recordings.
    sequence_key: String,
}

/// Load JSONL recording frames and assign a class label based on filename.
fn load_recording(path: &Path, class_idx: usize) -> Vec<Sample> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let recording_key = path.to_string_lossy().to_string();
    content
        .lines()
        .flat_map(|line| {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                return Vec::new();
            };

            // RuView live recordings include a fused/global `features` object plus
            // per-node `node_features`.  Training on only the fused update collapses
            // six ESP32 viewpoints into one noisy sample and makes the adaptive
            // model over-report movement.  Prefer one sample per fresh node so the
            // runtime per-node temporal classifier is trained on the same schema it
            // sees live.
            if let Some(node_features) = v.get("node_features").and_then(|n| n.as_array()) {
                let samples: Vec<Sample> = node_features
                    .iter()
                    .filter(|node_feature| {
                        !node_feature
                            .get("stale")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    })
                    .filter(|node_feature| node_feature.get("features").is_some())
                    .map(|node_feature| {
                        let node_id = node_feature
                            .get("node_id")
                            .and_then(|v| v.as_u64())
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        Sample {
                            features: features_from_node_frame(&v, node_feature),
                            class_idx,
                            sequence_key: format!("{recording_key}::node_{node_id}"),
                        }
                    })
                    .collect();
                if !samples.is_empty() {
                    return samples;
                }
            }

            // Backward-compatible fallback for older single-node/fused JSONL.
            vec![Sample {
                features: features_from_frame(&v),
                class_idx,
                sequence_key: format!("{recording_key}::fused"),
            }]
        })
        .collect()
}

/// Map a recording filename to a class name (String).
/// Returns the discovered class name for the file, or None if it cannot be determined.
fn classify_recording_name(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    // Strip "train_" prefix and ".jsonl" suffix, then extract the class label.
    // Convention: train_<class>_<description>.jsonl
    // The class is the first segment after "train_" that matches a known pattern,
    // or the entire middle portion if no pattern matches.

    // Check common patterns first for backward compat
    if lower.contains("empty") || lower.contains("absent") {
        return Some("absent".into());
    }
    if lower.contains("still") || lower.contains("sitting") || lower.contains("standing") {
        return Some("present_still".into());
    }
    if lower.contains("walking") || lower.contains("moving") {
        return Some("present_moving".into());
    }
    if lower.contains("active") || lower.contains("exercise") || lower.contains("running") {
        return Some("active".into());
    }

    // Fallback: extract class from filename structure train_<class>_*.jsonl
    let stem = lower
        .trim_start_matches("train_")
        .trim_end_matches(".jsonl");
    let class_name = stem.split('_').next().unwrap_or(stem);
    if !class_name.is_empty() {
        Some(class_name.to_string())
    } else {
        None
    }
}

/// Train a model from labeled JSONL recordings in a directory.
///
/// Recordings are matched to classes by filename pattern. Classes are discovered
/// dynamically from the training data filenames:
/// - `*empty*` / `*absent*`   → absent
/// - `*still*` / `*sitting*`  → present_still
/// - `*walking*` / `*moving*` → present_moving
/// - `*active*` / `*exercise*`→ active
/// - Any other `train_<class>_*.jsonl` → <class>
pub fn train_from_recordings(recordings_dir: &Path) -> Result<AdaptiveModel, String> {
    // First pass: scan filenames to discover all unique class names.
    let entries: Vec<_> = std::fs::read_dir(recordings_dir)
        .map_err(|e| format!("Cannot read {}: {}", recordings_dir.display(), e))?
        .flatten()
        .collect();

    let mut class_map: HashMap<String, usize> = HashMap::new();
    let mut class_names: Vec<String> = Vec::new();

    // Collect (entry, class_name) pairs for files that match.
    let mut file_classes: Vec<(PathBuf, String, String)> = Vec::new(); // (path, fname, class_name)
    for entry in &entries {
        let fname = entry.file_name().to_string_lossy().to_string();
        if !fname.starts_with("train_") || !fname.ends_with(".jsonl") {
            continue;
        }
        if let Some(class_name) = classify_recording_name(&fname) {
            if !class_map.contains_key(&class_name) {
                let idx = class_names.len();
                class_map.insert(class_name.clone(), idx);
                class_names.push(class_name.clone());
            }
            file_classes.push((entry.path(), fname, class_name));
        }
    }

    let n_classes = class_names.len();
    if n_classes == 0 {
        return Err("No training samples found. Record data with train_* prefix.".into());
    }

    // Second pass: load recordings with the discovered class indices.
    let mut samples: Vec<Sample> = Vec::new();
    for (path, fname, class_name) in &file_classes {
        let class_idx = class_map[class_name];
        let loaded = load_recording(path, class_idx);
        eprintln!(
            "  Loaded {}: {} frames → class '{}'",
            fname,
            loaded.len(),
            class_name
        );
        samples.extend(loaded);
    }

    if samples.is_empty() {
        return Err("No training samples found. Record data with train_* prefix.".into());
    }

    let n = samples.len();
    eprintln!(
        "Total training samples: {n} across {n_classes} classes: {:?}",
        class_names
    );

    // ── Compute global normalisation stats ──
    let mut global_mean = [0.0f64; N_FEATURES];
    let mut global_var = [0.0f64; N_FEATURES];
    for s in &samples {
        for i in 0..N_FEATURES {
            global_mean[i] += s.features[i];
        }
    }
    for i in 0..N_FEATURES {
        global_mean[i] /= n as f64;
    }
    for s in &samples {
        for i in 0..N_FEATURES {
            global_var[i] += (s.features[i] - global_mean[i]).powi(2);
        }
    }
    let mut global_std = [0.0f64; N_FEATURES];
    for i in 0..N_FEATURES {
        global_std[i] = (global_var[i] / n as f64).sqrt().max(1e-9);
    }

    // ── Compute per-class statistics ──
    let mut class_sums = vec![[0.0f64; N_FEATURES]; n_classes];
    let mut class_sq = vec![[0.0f64; N_FEATURES]; n_classes];
    let mut class_counts = vec![0usize; n_classes];
    for s in &samples {
        let c = s.class_idx;
        class_counts[c] += 1;
        for i in 0..N_FEATURES {
            class_sums[c][i] += s.features[i];
            class_sq[c][i] += s.features[i] * s.features[i];
        }
    }

    let mut class_stats = Vec::new();
    for c in 0..n_classes {
        let cnt = class_counts[c].max(1) as f64;
        let mut mean = [0.0; N_FEATURES];
        let mut stddev = [0.0; N_FEATURES];
        for i in 0..N_FEATURES {
            mean[i] = class_sums[c][i] / cnt;
            stddev[i] = ((class_sq[c][i] / cnt) - mean[i] * mean[i]).max(0.0).sqrt();
        }
        class_stats.push(ClassStats {
            label: class_names[c].clone(),
            count: class_counts[c],
            mean,
            stddev,
        });
    }

    // ── Normalise all samples ──
    let mut norm_samples: Vec<([f64; N_FEATURES], usize)> = samples
        .iter()
        .map(|s| {
            let mut x = [0.0; N_FEATURES];
            for i in 0..N_FEATURES {
                x[i] = (s.features[i] - global_mean[i]) / (global_std[i] + 1e-9);
            }
            (x, s.class_idx)
        })
        .collect();

    // ── Train logistic regression via mini-batch SGD ──
    let mut weights: Vec<Vec<f64>> = vec![vec![0.0f64; N_FEATURES + 1]; n_classes];
    let lr = 0.1;
    let epochs = 200;
    let batch_size = 32;

    // Shuffle helper (simple LCG for determinism).
    let mut rng_state: u64 = 42;
    let mut rng_next = move || -> u64 {
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        rng_state >> 33
    };

    for epoch in 0..epochs {
        // Shuffle samples.
        for i in (1..norm_samples.len()).rev() {
            let j = (rng_next() as usize) % (i + 1);
            norm_samples.swap(i, j);
        }

        let mut epoch_loss = 0.0f64;
        let mut _batch_count = 0;

        for batch_start in (0..norm_samples.len()).step_by(batch_size) {
            let batch_end = (batch_start + batch_size).min(norm_samples.len());
            let batch = &norm_samples[batch_start..batch_end];

            // Accumulate gradients.
            let mut grad: Vec<Vec<f64>> = vec![vec![0.0f64; N_FEATURES + 1]; n_classes];

            for (x, target) in batch {
                // Forward: softmax.
                let mut logits: Vec<f64> = vec![0.0; n_classes];
                for c in 0..n_classes {
                    logits[c] = weights[c][N_FEATURES]; // bias
                    for i in 0..N_FEATURES {
                        logits[c] += weights[c][i] * x[i];
                    }
                }
                let max_l = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let exp_sum: f64 = logits.iter().map(|z| (z - max_l).exp()).sum();
                let mut probs: Vec<f64> = vec![0.0; n_classes];
                for c in 0..n_classes {
                    probs[c] = ((logits[c] - max_l).exp()) / exp_sum;
                }

                // Cross-entropy loss.
                epoch_loss += -(probs[*target].max(1e-15)).ln();

                // Gradient: prob - one_hot(target).
                for c in 0..n_classes {
                    let delta = probs[c] - if c == *target { 1.0 } else { 0.0 };
                    for i in 0..N_FEATURES {
                        grad[c][i] += delta * x[i];
                    }
                    grad[c][N_FEATURES] += delta; // bias grad
                }
            }

            // Update weights.
            let bs = batch.len() as f64;
            let current_lr = lr * (1.0 - epoch as f64 / epochs as f64); // linear decay
            for c in 0..n_classes {
                for i in 0..=N_FEATURES {
                    weights[c][i] -= current_lr * grad[c][i] / bs;
                }
            }
            _batch_count += 1;
        }

        if epoch % 50 == 0 || epoch == epochs - 1 {
            let avg_loss = epoch_loss / n as f64;
            eprintln!("  Epoch {epoch:3}: loss = {avg_loss:.4}");
        }
    }

    // ── Evaluate accuracy ──
    let mut correct = 0;
    for (x, target) in &norm_samples {
        let mut logits: Vec<f64> = vec![0.0; n_classes];
        for c in 0..n_classes {
            logits[c] = weights[c][N_FEATURES];
            for i in 0..N_FEATURES {
                logits[c] += weights[c][i] * x[i];
            }
        }
        let pred = logits
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        if pred == *target {
            correct += 1;
        }
    }
    let accuracy = correct as f64 / n as f64;
    eprintln!("Training accuracy: {correct}/{n} = {accuracy:.1}%");

    // ── Per-class accuracy ──
    let mut class_correct = vec![0usize; n_classes];
    let mut class_total = vec![0usize; n_classes];
    for (x, target) in &norm_samples {
        class_total[*target] += 1;
        let mut logits: Vec<f64> = vec![0.0; n_classes];
        for c in 0..n_classes {
            logits[c] = weights[c][N_FEATURES];
            for i in 0..N_FEATURES {
                logits[c] += weights[c][i] * x[i];
            }
        }
        let pred = logits
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        if pred == *target {
            class_correct[*target] += 1;
        }
    }
    for c in 0..n_classes {
        let tot = class_total[c].max(1);
        eprintln!(
            "  {}: {}/{} ({:.0}%)",
            class_names[c],
            class_correct[c],
            tot,
            class_correct[c] as f64 / tot as f64 * 100.0
        );
    }

    let temporal_model = train_temporal_model(&samples, n_classes, &class_names);

    Ok(AdaptiveModel {
        class_stats,
        weights,
        global_mean,
        global_std,
        trained_frames: n,
        training_accuracy: accuracy,
        version: 1,
        class_names,
        temporal_model,
    })
}

/// Default path for the saved adaptive model.
pub fn model_path() -> PathBuf {
    PathBuf::from("data/adaptive_model.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn temporal_features_summarise_mean_volatility_extremes_drift_and_change() {
        let mut history = VecDeque::new();
        let mut a = [0.0; N_FEATURES];
        let mut b = [0.0; N_FEATURES];
        let mut c = [0.0; N_FEATURES];
        a[0] = 1.0;
        b[0] = 3.0;
        c[0] = 5.0;
        history.push_back(a);
        history.push_back(b);
        history.push_back(c);

        let temporal = temporal_features_from_history(&history, 3).expect("full window");

        assert_eq!(temporal.len(), N_FEATURES * 6);
        assert!((temporal[0] - 3.0).abs() < 1e-9, "mean feature");
        assert!(
            (temporal[N_FEATURES] - (8.0_f64 / 3.0).sqrt()).abs() < 1e-9,
            "std feature"
        );
        assert!((temporal[N_FEATURES * 2] - 1.0).abs() < 1e-9, "min feature");
        assert!((temporal[N_FEATURES * 3] - 5.0).abs() < 1e-9, "max feature");
        assert!(
            (temporal[N_FEATURES * 4] - 4.0).abs() < 1e-9,
            "delta feature"
        );
        assert!(
            (temporal[N_FEATURES * 5] - 2.0).abs() < 1e-9,
            "mean absolute diff feature"
        );
    }

    #[test]
    fn temporal_model_classifies_by_nearest_standardized_centroid() {
        let mut model = AdaptiveModel::default();
        model.class_names = vec!["absent".into(), "present_moving".into()];
        let mut present = vec![0.0; N_FEATURES * 6];
        for idx in [0, N_FEATURES * 2, N_FEATURES * 3] {
            for feature in 0..N_FEATURES {
                present[idx + feature] = 10.0;
            }
        }
        model.temporal_model = Some(TemporalModel::from_centroids_for_tests(
            3,
            vec!["absent".into(), "present_moving".into()],
            vec![vec![0.0; N_FEATURES * 6], present],
            vec![0.0; N_FEATURES * 6],
            vec![1.0; N_FEATURES * 6],
        ));

        let mut history = VecDeque::new();
        for _ in 0..3 {
            history.push_back([10.0; N_FEATURES]);
        }

        let (label, confidence) = model
            .classify_temporal(&history)
            .expect("temporal prediction");

        assert_eq!(label, "present_moving");
        assert!(
            confidence > 0.90,
            "expected high confidence, got {confidence}"
        );
    }

    #[test]
    fn temporal_model_waits_until_minimum_window_is_available() {
        let model = AdaptiveModel {
            temporal_model: Some(TemporalModel::from_centroids_for_tests(
                4,
                vec!["absent".into()],
                vec![vec![0.0; N_FEATURES * 6]],
                vec![0.0; N_FEATURES * 6],
                vec![1.0; N_FEATURES * 6],
            )),
            ..AdaptiveModel::default()
        };
        let mut history = VecDeque::new();
        history.push_back([0.0; N_FEATURES]);

        assert!(model.classify_temporal(&history).is_none());
    }

    #[test]
    fn load_recording_expands_ruview_updates_into_per_node_samples() {
        let path = std::env::temp_dir().join(format!(
            "ruview_multi_node_recording_{}_{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let jsonl = serde_json::json!({
            "type": "sensing_update",
            "nodes": [
                {"node_id": 2, "amplitude": [1.0, 3.0, 5.0]},
                {"node_id": 4, "amplitude": [10.0, 14.0, 18.0]}
            ],
            "node_features": [
                {"node_id": 2, "features": {
                    "mean_rssi": -54.0,
                    "variance": 2.0,
                    "motion_band_power": 3.0,
                    "breathing_band_power": 4.0,
                    "dominant_freq_hz": 0.2,
                    "change_points": 1,
                    "spectral_power": 5.0
                }},
                {"node_id": 4, "features": {
                    "mean_rssi": -20.0,
                    "variance": 20.0,
                    "motion_band_power": 30.0,
                    "breathing_band_power": 40.0,
                    "dominant_freq_hz": 0.4,
                    "change_points": 2,
                    "spectral_power": 50.0
                }}
            ],
            "features": {
                "mean_rssi": -999.0,
                "variance": 999.0,
                "motion_band_power": 999.0,
                "breathing_band_power": 999.0,
                "dominant_freq_hz": 9.99,
                "change_points": 99,
                "spectral_power": 999.0
            }
        })
        .to_string();
        std::fs::write(&path, format!("{}\n", jsonl)).unwrap();

        let samples = load_recording(&path, 7);
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            samples.len(),
            2,
            "one training sample per node, not one global update"
        );
        assert_eq!(samples[0].class_idx, 7);
        assert_eq!(samples[1].class_idx, 7);
        assert_eq!(samples[0].features[0], 2.0);
        assert_eq!(samples[0].features[6], -54.0);
        assert_eq!(samples[1].features[0], 20.0);
        assert_eq!(samples[1].features[6], -20.0);
        assert!(
            (samples[0].features[7] - 3.0).abs() < 1e-9,
            "node 2 amplitude mean"
        );
        assert!(
            (samples[1].features[7] - 14.0).abs() < 1e-9,
            "node 4 amplitude mean"
        );
    }
}
