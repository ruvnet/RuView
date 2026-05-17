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
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ── Feature vector ───────────────────────────────────────────────────────────

/// ADR-118: feature vector redesigned for multi-node use + multicollinearity
/// reduction. Audit on 7-class training set showed:
///   * 17-21 multicollinear pairs (|r|>0.85) — energy features and amplitude
///     scalars were highly redundant.
///   * `amp_min` constant 0.0 across all frames (null subcarrier of HT20),
///     making `amp_range = amp_max - 0` fully redundant with `amp_max`.
///   * On 6-node data F-stat 10× higher than 2-node, but classifier accuracy
///     barely budged (40→44%) because the prior 15-feature pipeline used only
///     `nodes.first()` — 5 of 6 sensors carried zero weight.
///
/// New 22-feature layout:
///   [0..4]  global signal features:
///       variance, mean_rssi, dominant_freq_hz, change_points
///   [4..22] per-node features (6 nodes × 3 features each):
///       per node id N∈{1..6}, base = 4 + (N-1)*3:
///       base+0: amp_std       — motion / multipath spread
///       base+1: amp_skew      — distribution asymmetry (where strong scatterers are)
///       base+2: amp_entropy   — spectral diversity (normalised)
///   Total: 22 features.
const N_GLOBAL_FEATURES: usize = 4;
const N_PER_NODE_FEATURES: usize = 3;
const MAX_NODES: usize = 6;
const N_FEATURES: usize = N_GLOBAL_FEATURES + MAX_NODES * N_PER_NODE_FEATURES;

/// ADR-120: exported feature count so external crates (e.g. the main
/// crate's AppStateInner) can size their rolling buffers correctly.
pub const N_FEATURES_PUB: usize = N_FEATURES;

/// Default class names for backward compatibility with old saved models.
const DEFAULT_CLASSES: &[&str] = &["absent", "present_still", "present_moving", "active"];

/// Extract extended feature vector from a JSONL frame (features + per-node amplitudes).
/// Missing-node features are zero-padded; z-score normalisation later treats
/// them consistently.
pub fn features_from_frame(frame: &serde_json::Value) -> [f64; N_FEATURES] {
    let feat = frame.get("features").cloned().unwrap_or(serde_json::Value::Null);
    let mut out = [0.0f64; N_FEATURES];

    // ── Global signal features (0..4) ──
    out[0] = feat.get("variance").and_then(|v| v.as_f64()).unwrap_or(0.0);
    out[1] = feat.get("mean_rssi").and_then(|v| v.as_f64()).unwrap_or(0.0);
    out[2] = feat.get("dominant_freq_hz").and_then(|v| v.as_f64()).unwrap_or(0.0);
    out[3] = feat.get("change_points").and_then(|v| v.as_f64()).unwrap_or(0.0);

    // ── Per-node features (4..22) ──
    if let Some(nodes) = frame.get("nodes").and_then(|n| n.as_array()) {
        for node_obj in nodes {
            let nid = node_obj.get("node_id").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if nid == 0 || nid > MAX_NODES { continue; }
            let amps: Vec<f64> = node_obj.get("amplitude")
                .or_else(|| node_obj.get("amplitudes"))
                .and_then(|a| a.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
                .unwrap_or_default();
            let (std_a, skew_a, entropy_a) = per_node_stats(&amps);
            let base = N_GLOBAL_FEATURES + (nid - 1) * N_PER_NODE_FEATURES;
            out[base] = std_a;
            out[base + 1] = skew_a;
            out[base + 2] = entropy_a;
        }
    }
    out
}

/// Runtime variant: callers pass the already-aggregated feature struct and a
/// slice of (node_id, &amplitudes) pairs. Compatible with the broadcast tick
/// task which has access to all live nodes simultaneously.
pub fn features_from_runtime(
    feat: &serde_json::Value,
    per_node_amps: &[(u8, &[f64])],
) -> [f64; N_FEATURES] {
    let mut out = [0.0f64; N_FEATURES];

    out[0] = feat.get("variance").and_then(|v| v.as_f64()).unwrap_or(0.0);
    out[1] = feat.get("mean_rssi").and_then(|v| v.as_f64()).unwrap_or(0.0);
    out[2] = feat.get("dominant_freq_hz").and_then(|v| v.as_f64()).unwrap_or(0.0);
    out[3] = feat.get("change_points").and_then(|v| v.as_f64()).unwrap_or(0.0);

    for (nid, amps) in per_node_amps {
        let nid = *nid as usize;
        if nid == 0 || nid > MAX_NODES { continue; }
        let (std_a, skew_a, entropy_a) = per_node_stats(amps);
        let base = N_GLOBAL_FEATURES + (nid - 1) * N_PER_NODE_FEATURES;
        out[base] = std_a;
        out[base + 1] = skew_a;
        out[base + 2] = entropy_a;
    }
    out
}

/// Compute the 3 per-node statistics used in the new feature vector:
/// std (motion / multipath spread), skew (distribution asymmetry),
/// entropy (spectral diversity, normalised to [0, 1]).
fn per_node_stats(amps: &[f64]) -> (f64, f64, f64) {
    if amps.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let n = amps.len() as f64;
    let mean = amps.iter().sum::<f64>() / n;
    let var = amps.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / n;
    let std = var.sqrt().max(1e-9);
    let skew = amps.iter().map(|a| ((a - mean) / std).powi(3)).sum::<f64>() / n;
    let total_power: f64 = amps.iter().map(|a| a * a).sum::<f64>().max(1e-9);
    let entropy: f64 = amps.iter()
        .map(|a| {
            let p = (a * a) / total_power;
            if p > 1e-12 { -p * p.ln() } else { 0.0 }
        })
        .sum::<f64>() / n.ln().max(1e-9);
    (std, skew, entropy)
}

// ── Per-class statistics ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassStats {
    pub label: String,
    pub count: usize,
    pub mean: [f64; N_FEATURES],
    pub stddev: [f64; N_FEATURES],
}

/// ADR-119: MLP (multi-layer perceptron) hidden-layer width.
/// 32 units is enough capacity for our 22-feature × 6-class problem
/// (~3k weights) while staying small enough to train in <60s on the
/// 151k-frame dataset and load instantly at runtime.
const MLP_HIDDEN: usize = 32;

/// ADR-120: temporal window size (number of consecutive frames stacked
/// into the windowed-MLP input). At the broadcast tick rate (~10 fps),
/// 20 frames = 2 seconds of context — enough to capture walking step
/// cadence (2 Hz), sit-stand transition cycles (0.5 Hz), and breathing
/// modulation. Chosen to match WiFlow's training-time window so amplitude
/// history buffers can be reused.
pub const WINDOW_FRAMES: usize = 20;

/// ADR-120: windowed-MLP input dimensionality = WINDOW_FRAMES × N_FEATURES.
const WINDOWED_INPUT: usize = WINDOW_FRAMES * N_FEATURES;

/// ADR-120: windowed-MLP hidden width. Larger than MLP_HIDDEN because
/// input is 20× wider (440 vs 22). 64 keeps params under 30k.
const WINDOWED_HIDDEN: usize = 64;

/// ADR-119: trained MLP classifier. Single hidden layer, ReLU activation,
/// softmax output. Stored alongside the LogReg weights — when `is_trained()`
/// returns true, `AdaptiveModel::classify` uses the MLP; otherwise it falls
/// back to logistic regression (the legacy path from before ADR-119).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MlpModel {
    /// Layer 1 weights, row-major `[N_FEATURES × MLP_HIDDEN]`.
    #[serde(default)]
    pub w1: Vec<f64>,
    /// Layer 1 bias, `[MLP_HIDDEN]`.
    #[serde(default)]
    pub b1: Vec<f64>,
    /// Layer 2 weights, row-major `[MLP_HIDDEN × n_classes]`.
    #[serde(default)]
    pub w2: Vec<f64>,
    /// Layer 2 bias, `[n_classes]`.
    #[serde(default)]
    pub b2: Vec<f64>,
    /// Number of output classes (== len(b2) when trained).
    #[serde(default)]
    pub n_classes: usize,
}

impl MlpModel {
    pub fn is_trained(&self) -> bool {
        !self.w1.is_empty() && self.n_classes > 0 && self.b2.len() == self.n_classes
    }

    /// Forward pass. Input is already z-score normalised by the caller.
    /// Returns softmax probabilities of length `n_classes`.
    pub fn forward(&self, x: &[f64; N_FEATURES]) -> Vec<f64> {
        // Layer 1: h = ReLU(x · W1 + b1)
        let mut h = vec![0.0f64; MLP_HIDDEN];
        for j in 0..MLP_HIDDEN {
            let mut s = self.b1[j];
            for i in 0..N_FEATURES {
                s += x[i] * self.w1[i * MLP_HIDDEN + j];
            }
            h[j] = s.max(0.0);
        }
        // Layer 2: logits = h · W2 + b2
        let mut logits = vec![0.0f64; self.n_classes];
        for c in 0..self.n_classes {
            let mut s = self.b2[c];
            for j in 0..MLP_HIDDEN {
                s += h[j] * self.w2[j * self.n_classes + c];
            }
            logits[c] = s;
        }
        // Softmax.
        let m = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp_sum: f64 = logits.iter().map(|z| (z - m).exp()).sum();
        logits.iter().map(|z| (z - m).exp() / exp_sum).collect()
    }
}

/// ADR-120: Windowed MLP — same architecture as MlpModel but takes a
/// 20-frame × 22-feature stack (440-d input) instead of a single frame.
/// Captures temporal patterns (walking step cadence, sit-stand cycles,
/// breathing modulation) that frame-level classifiers miss.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowedMlpModel {
    /// Layer 1 weights, row-major `[WINDOWED_INPUT × WINDOWED_HIDDEN]`.
    #[serde(default)]
    pub w1: Vec<f64>,
    /// Layer 1 bias, `[WINDOWED_HIDDEN]`.
    #[serde(default)]
    pub b1: Vec<f64>,
    /// Layer 2 weights, row-major `[WINDOWED_HIDDEN × n_classes]`.
    #[serde(default)]
    pub w2: Vec<f64>,
    /// Layer 2 bias, `[n_classes]`.
    #[serde(default)]
    pub b2: Vec<f64>,
    /// Number of output classes (== len(b2) when trained).
    #[serde(default)]
    pub n_classes: usize,
}

impl WindowedMlpModel {
    pub fn is_trained(&self) -> bool {
        !self.w1.is_empty()
            && self.n_classes > 0
            && self.b2.len() == self.n_classes
            && self.w1.len() == WINDOWED_INPUT * WINDOWED_HIDDEN
    }

    /// Forward pass. `window` is `WINDOW_FRAMES × N_FEATURES` flat,
    /// row-major (oldest-frame-first), already z-score normalised.
    /// Returns softmax probabilities of length `n_classes`.
    pub fn forward(&self, window: &[f64]) -> Vec<f64> {
        debug_assert_eq!(window.len(), WINDOWED_INPUT);
        // Layer 1: h = ReLU(window · W1 + b1)
        let mut h = vec![0.0f64; WINDOWED_HIDDEN];
        for j in 0..WINDOWED_HIDDEN {
            let mut s = self.b1[j];
            for i in 0..WINDOWED_INPUT {
                s += window[i] * self.w1[i * WINDOWED_HIDDEN + j];
            }
            h[j] = s.max(0.0);
        }
        // Layer 2: logits = h · W2 + b2
        let mut logits = vec![0.0f64; self.n_classes];
        for c in 0..self.n_classes {
            let mut s = self.b2[c];
            for j in 0..WINDOWED_HIDDEN {
                s += h[j] * self.w2[j * self.n_classes + c];
            }
            logits[c] = s;
        }
        let m = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp_sum: f64 = logits.iter().map(|z| (z - m).exp()).sum();
        logits.iter().map(|z| (z - m).exp() / exp_sum).collect()
    }
}

// ── Trained model ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveModel {
    /// Per-class feature statistics (centroid + spread).
    pub class_stats: Vec<ClassStats>,
    /// ADR-119: legacy logistic regression weights, kept as fallback.
    /// Shape: `[n_classes × (N_FEATURES + 1)]` (last column = bias).
    /// When `mlp.is_trained()` returns true, MLP wins and these are unused
    /// at classify time but still updated by `train_from_recordings` so
    /// rollback is one-line.
    pub weights: Vec<Vec<f64>>,
    /// ADR-119: trained MLP (frame-level fallback, used when WindowedMlp
    /// has no data yet — e.g. cold start before 20 frames accumulated).
    #[serde(default)]
    pub mlp: MlpModel,
    /// ADR-120: trained Windowed MLP (preferred classifier when trained
    /// AND a 20-frame window of fresh features is available at classify
    /// time). Captures temporal patterns the frame-level MLP can't see.
    #[serde(default)]
    pub windowed_mlp: WindowedMlpModel,
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
            mlp: MlpModel::default(),
            windowed_mlp: WindowedMlpModel::default(),
            global_mean: [0.0; N_FEATURES],
            global_std: [1.0; N_FEATURES],
            trained_frames: 0,
            training_accuracy: 0.0,
            version: 1,
            class_names: default_class_names(),
        }
    }
}

impl AdaptiveModel {
    /// ADR-120: classify using a temporal window of recent frames.
    /// `window` is `WINDOW_FRAMES × N_FEATURES` flat row-major (oldest first),
    /// in raw (un-normalised) units — this fn applies z-score normalisation
    /// internally using the model's `global_mean`/`global_std`.
    /// Falls back to frame-level `classify()` on the most recent frame when
    /// the windowed MLP isn't trained.
    pub fn classify_window(&self, window: &[f64]) -> (String, f64) {
        if self.windowed_mlp.is_trained() && window.len() == WINDOWED_INPUT {
            let mut norm = vec![0.0f64; WINDOWED_INPUT];
            for f in 0..WINDOW_FRAMES {
                for i in 0..N_FEATURES {
                    let idx = f * N_FEATURES + i;
                    norm[idx] = (window[idx] - self.global_mean[i]) / (self.global_std[i] + 1e-9);
                }
            }
            let probs = self.windowed_mlp.forward(&norm);
            let (best_c, best_p) = probs.iter().enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap();
            let label = if best_c < self.class_names.len() {
                self.class_names[best_c].clone()
            } else {
                "present_still".to_string()
            };
            return (label, *best_p);
        }
        // Cold-start fallback: most recent frame via frame-level classifier.
        let mut last_frame = [0.0f64; N_FEATURES];
        if window.len() >= N_FEATURES {
            let off = window.len() - N_FEATURES;
            last_frame.copy_from_slice(&window[off..off + N_FEATURES]);
        }
        self.classify(&last_frame)
    }

    /// Classify a raw feature vector. Returns (class_label, confidence).
    /// ADR-119: prefers MLP when trained; falls back to logistic regression
    /// otherwise. ADR-120: temporal-context API is `classify_window` —
    /// prefer it when callers have a recent feature buffer.
    pub fn classify(&self, raw_features: &[f64; N_FEATURES]) -> (String, f64) {
        // Normalise features once (shared by MLP and LogReg).
        let mut x = [0.0f64; N_FEATURES];
        for i in 0..N_FEATURES {
            x[i] = (raw_features[i] - self.global_mean[i]) / (self.global_std[i] + 1e-9);
        }

        // ADR-119: MLP path (preferred when trained).
        if self.mlp.is_trained() {
            let probs = self.mlp.forward(&x);
            let (best_c, best_p) = probs.iter().enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap();
            let label = if best_c < self.class_names.len() {
                self.class_names[best_c].clone()
            } else {
                "present_still".to_string()
            };
            return (label, *best_p);
        }

        // Legacy logistic regression fallback.
        let n_classes = self.weights.len();
        if n_classes == 0 || self.class_stats.is_empty() {
            return ("present_still".to_string(), 0.5);
        }
        let mut logits: Vec<f64> = vec![0.0; n_classes];
        for c in 0..n_classes {
            let w = &self.weights[c];
            let mut z = w[N_FEATURES];
            for i in 0..N_FEATURES {
                z += w[i] * x[i];
            }
            logits[c] = z;
        }
        let max_logit = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp_sum: f64 = logits.iter().map(|z| (z - max_logit).exp()).sum();
        let mut probs: Vec<f64> = vec![0.0; n_classes];
        for c in 0..n_classes {
            probs[c] = ((logits[c] - max_logit).exp()) / exp_sum;
        }
        let (best_c, best_p) = probs.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();
        let label = if best_c < self.class_names.len() {
            self.class_names[best_c].clone()
        } else {
            "present_still".to_string()
        };
        (label, *best_p)
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
        serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

// ── Training ─────────────────────────────────────────────────────────────────

/// A labeled training sample.
#[derive(Clone)]
struct Sample {
    features: [f64; N_FEATURES],
    class_idx: usize,
}

/// Load JSONL recording frames and assign a class label based on filename.
fn load_recording(path: &Path, class_idx: usize) -> Vec<Sample> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content.lines().filter_map(|line| {
        let v: serde_json::Value = serde_json::from_str(line).ok()?;
        // Use extended features (server features + subcarrier stats).
        Some(Sample {
            features: features_from_frame(&v),
            class_idx,
        })
    }).collect()
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
    if lower.contains("empty") || lower.contains("absent") { return Some("absent".into()); }
    if lower.contains("still") || lower.contains("sitting") || lower.contains("standing") { return Some("present_still".into()); }
    if lower.contains("walking") || lower.contains("moving") { return Some("present_moving".into()); }
    if lower.contains("active") || lower.contains("exercise") || lower.contains("running") { return Some("active".into()); }

    // Fallback: extract class from filename structure train_<class>_*.jsonl
    let stem = lower.trim_start_matches("train_").trim_end_matches(".jsonl");
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
    // ADR-120: keep recordings grouped so windowed-MLP training can slide
    // a temporal window WITHIN each recording (not across recording
    // boundaries — would mix classes).
    let mut samples: Vec<Sample> = Vec::new();
    let mut recording_groups: Vec<Vec<Sample>> = Vec::new();
    for (path, fname, class_name) in &file_classes {
        let class_idx = class_map[class_name];
        let loaded = load_recording(path, class_idx);
        eprintln!("  Loaded {}: {} frames → class '{}'",
                 fname, loaded.len(), class_name);
        samples.extend(loaded.clone());
        recording_groups.push(loaded);
    }

    if samples.is_empty() {
        return Err("No training samples found. Record data with train_* prefix.".into());
    }

    let n = samples.len();
    eprintln!("Total training samples: {n} across {n_classes} classes: {:?}", class_names);

    // ── Compute global normalisation stats ──
    let mut global_mean = [0.0f64; N_FEATURES];
    let mut global_var = [0.0f64; N_FEATURES];
    for s in &samples {
        for i in 0..N_FEATURES { global_mean[i] += s.features[i]; }
    }
    for i in 0..N_FEATURES { global_mean[i] /= n as f64; }
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
    let mut norm_samples: Vec<([f64; N_FEATURES], usize)> = samples.iter().map(|s| {
        let mut x = [0.0; N_FEATURES];
        for i in 0..N_FEATURES {
            x[i] = (s.features[i] - global_mean[i]) / (global_std[i] + 1e-9);
        }
        (x, s.class_idx)
    }).collect();

    // ── Train logistic regression via mini-batch SGD ──
    let mut weights: Vec<Vec<f64>> = vec![vec![0.0f64; N_FEATURES + 1]; n_classes];
    let lr = 0.1;
    let epochs = 200;
    let batch_size = 32;

    // Shuffle helper (simple LCG for determinism).
    let mut rng_state: u64 = 42;
    let mut rng_next = move || -> u64 {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
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
        let pred = logits.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap().0;
        if pred == *target { correct += 1; }
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
        let pred = logits.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap().0;
        if pred == *target { class_correct[*target] += 1; }
    }
    for c in 0..n_classes {
        let tot = class_total[c].max(1);
        eprintln!("  LogReg {}: {}/{} ({:.0}%)", class_names[c], class_correct[c], tot,
                 class_correct[c] as f64 / tot as f64 * 100.0);
    }

    // ── ADR-119: train MLP on the same normalised samples ──
    eprintln!("Training MLP (22 → {} → {}) ...", MLP_HIDDEN, n_classes);
    let mlp = train_mlp_classifier(&norm_samples, n_classes);
    let (mlp_acc, mlp_per_class) = eval_mlp(&mlp, &norm_samples, n_classes);
    eprintln!("MLP accuracy: {:.2}% (LogReg was {:.2}%)",
              mlp_acc * 100.0, accuracy * 100.0);
    for c in 0..n_classes {
        let tot = class_total[c].max(1);
        let corr = mlp_per_class[c];
        eprintln!("  MLP    {}: {}/{} ({:.0}%)",
                 class_names[c], corr, tot, corr as f64 / tot as f64 * 100.0);
    }

    // ── ADR-120: Windowed MLP training ──
    // Build temporal-window samples within each recording (no cross-recording
    // mixing). Slide window of WINDOW_FRAMES with stride to balance class
    // count vs sample count.
    eprintln!("Building temporal windows ({} frames × {} features → {} dims)...",
              WINDOW_FRAMES, N_FEATURES, WINDOWED_INPUT);
    let window_stride = 5usize; // 4× overlap; ~28k windows total on 151k frames
    let mut win_samples: Vec<(Vec<f64>, usize)> = Vec::new();
    for group in &recording_groups {
        if group.len() < WINDOW_FRAMES { continue; }
        let class_idx = group[0].class_idx;
        let mut start = 0usize;
        while start + WINDOW_FRAMES <= group.len() {
            let mut flat: Vec<f64> = Vec::with_capacity(WINDOWED_INPUT);
            for f in 0..WINDOW_FRAMES {
                let frame = &group[start + f];
                for i in 0..N_FEATURES {
                    let z = (frame.features[i] - global_mean[i]) / (global_std[i] + 1e-9);
                    flat.push(z);
                }
            }
            win_samples.push((flat, class_idx));
            start += window_stride;
        }
    }
    eprintln!("Total windowed samples: {}", win_samples.len());

    // Count per-class windowed samples.
    let mut win_class_total = vec![0usize; n_classes];
    for (_, c) in &win_samples { win_class_total[*c] += 1; }

    eprintln!("Training Windowed MLP ({} → {} → {}) ...", WINDOWED_INPUT, WINDOWED_HIDDEN, n_classes);
    let windowed_mlp = train_windowed_mlp_classifier(&win_samples, n_classes);
    let (win_acc, win_per_class) = eval_windowed_mlp(&windowed_mlp, &win_samples, n_classes);
    eprintln!("Windowed MLP accuracy: {:.2}% (frame-level MLP was {:.2}%)",
              win_acc * 100.0, mlp_acc * 100.0);
    for c in 0..n_classes {
        let tot = win_class_total[c].max(1);
        let corr = win_per_class[c];
        eprintln!("  W-MLP  {}: {}/{} ({:.0}%)",
                 class_names[c], corr, tot, corr as f64 / tot as f64 * 100.0);
    }

    // Pick the best classifier as final accuracy number.
    let final_accuracy = win_acc.max(mlp_acc).max(accuracy);

    Ok(AdaptiveModel {
        class_stats,
        weights,
        mlp,
        windowed_mlp,
        global_mean,
        global_std,
        trained_frames: n,
        training_accuracy: final_accuracy,
        version: 1,
        class_names,
    })
}

// ── ADR-119: MLP training (manual backprop, no external ML crate) ────────────

/// Train a single-hidden-layer MLP on already-z-score-normalised samples.
/// Architecture: N_FEATURES → MLP_HIDDEN → n_classes (ReLU + softmax).
/// Optimiser: SGD + momentum 0.9 + weight decay 1e-4 + cosine LR decay.
fn train_mlp_classifier(samples: &[([f64; N_FEATURES], usize)], n_classes: usize) -> MlpModel {
    let n_w1 = N_FEATURES * MLP_HIDDEN;
    let n_w2 = MLP_HIDDEN * n_classes;

    // He initialisation: w ~ N(0, sqrt(2/fan_in))
    let mut rng_state: u64 = 1337;
    let mut rng_u01 = move || -> f64 {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((rng_state >> 33) as f64) / ((u64::MAX >> 33) as f64)
    };
    let mut he_init = |n: usize, fan_in: usize| -> Vec<f64> {
        let s = (2.0 / fan_in as f64).sqrt();
        let mut v = Vec::with_capacity(n);
        let mut k = 0;
        while k < n {
            let u1 = rng_u01().max(1e-12);
            let u2 = rng_u01();
            let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos() * s;
            let z1 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).sin() * s;
            v.push(z0);
            k += 1;
            if k < n { v.push(z1); k += 1; }
        }
        v
    };

    let mut w1 = he_init(n_w1, N_FEATURES);
    let mut b1 = vec![0.0f64; MLP_HIDDEN];
    let mut w2 = he_init(n_w2, MLP_HIDDEN);
    let mut b2 = vec![0.0f64; n_classes];

    let mut mw1 = vec![0.0f64; n_w1];
    let mut mb1 = vec![0.0f64; MLP_HIDDEN];
    let mut mw2 = vec![0.0f64; n_w2];
    let mut mb2 = vec![0.0f64; n_classes];

    let momentum = 0.9f64;
    let weight_decay = 1e-4f64;
    let base_lr = 0.05f64;
    let batch_size = 64usize;
    let epochs = 30usize;
    let n = samples.len();

    // Shuffle index buffer (avoid cloning sample arrays).
    let mut idx: Vec<usize> = (0..n).collect();
    let mut shuf_state: u64 = 7;
    let mut shuf_next = move || -> u64 {
        shuf_state = shuf_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        shuf_state >> 33
    };

    for epoch in 0..epochs {
        for i in (1..idx.len()).rev() {
            let j = (shuf_next() as usize) % (i + 1);
            idx.swap(i, j);
        }

        let lr = base_lr * 0.5 * (1.0 + (std::f64::consts::PI * epoch as f64 / epochs as f64).cos());
        let mut epoch_loss = 0.0f64;
        let mut h_pre = vec![0.0f64; MLP_HIDDEN];
        let mut h = vec![0.0f64; MLP_HIDDEN];
        let mut logits = vec![0.0f64; n_classes];

        let mut k = 0usize;
        while k < n {
            let bend = (k + batch_size).min(n);
            let mut gw1 = vec![0.0f64; n_w1];
            let mut gb1 = vec![0.0f64; MLP_HIDDEN];
            let mut gw2 = vec![0.0f64; n_w2];
            let mut gb2 = vec![0.0f64; n_classes];
            let bs = (bend - k) as f64;

            for &si in &idx[k..bend] {
                let (x, target) = &samples[si];

                // Forward.
                for j in 0..MLP_HIDDEN {
                    let mut s = b1[j];
                    for i in 0..N_FEATURES { s += x[i] * w1[i * MLP_HIDDEN + j]; }
                    h_pre[j] = s;
                    h[j] = s.max(0.0);
                }
                for c in 0..n_classes {
                    let mut s = b2[c];
                    for j in 0..MLP_HIDDEN { s += h[j] * w2[j * n_classes + c]; }
                    logits[c] = s;
                }
                let mx = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let ex_sum: f64 = logits.iter().map(|z| (z - mx).exp()).sum();
                // d_logits = softmax - one_hot
                let mut d_logits = vec![0.0f64; n_classes];
                for c in 0..n_classes {
                    let p = (logits[c] - mx).exp() / ex_sum;
                    d_logits[c] = p - if c == *target { 1.0 } else { 0.0 };
                    if c == *target { epoch_loss += -(p.max(1e-15)).ln(); }
                }

                // Gradients.
                for c in 0..n_classes {
                    gb2[c] += d_logits[c];
                    for j in 0..MLP_HIDDEN {
                        gw2[j * n_classes + c] += h[j] * d_logits[c];
                    }
                }
                // Backprop through Layer-2 to hidden.
                let mut d_h = [0.0f64; MLP_HIDDEN];
                for j in 0..MLP_HIDDEN {
                    if h_pre[j] <= 0.0 { continue; }
                    let mut s = 0.0;
                    for c in 0..n_classes { s += w2[j * n_classes + c] * d_logits[c]; }
                    d_h[j] = s;
                }
                for j in 0..MLP_HIDDEN {
                    gb1[j] += d_h[j];
                    for i in 0..N_FEATURES { gw1[i * MLP_HIDDEN + j] += x[i] * d_h[j]; }
                }
            }

            // SGD + momentum + weight decay.
            for q in 0..n_w1 {
                let g = gw1[q] / bs + weight_decay * w1[q];
                mw1[q] = momentum * mw1[q] + g;
                w1[q] -= lr * mw1[q];
            }
            for q in 0..MLP_HIDDEN {
                let g = gb1[q] / bs;
                mb1[q] = momentum * mb1[q] + g;
                b1[q] -= lr * mb1[q];
            }
            for q in 0..n_w2 {
                let g = gw2[q] / bs + weight_decay * w2[q];
                mw2[q] = momentum * mw2[q] + g;
                w2[q] -= lr * mw2[q];
            }
            for q in 0..n_classes {
                let g = gb2[q] / bs;
                mb2[q] = momentum * mb2[q] + g;
                b2[q] -= lr * mb2[q];
            }

            k = bend;
        }
        if epoch % 5 == 0 || epoch == epochs - 1 {
            eprintln!("  MLP epoch {epoch:2}/{}: loss = {:.4}, lr = {:.4}",
                      epochs, epoch_loss / n as f64, lr);
        }
    }

    MlpModel { w1, b1, w2, b2, n_classes }
}

/// Evaluate MLP accuracy and per-class correct counts on normalised samples.
fn eval_mlp(mlp: &MlpModel, samples: &[([f64; N_FEATURES], usize)], n_classes: usize)
    -> (f64, Vec<usize>)
{
    let mut correct = 0usize;
    let mut per_class = vec![0usize; n_classes];
    for (x, target) in samples {
        let probs = mlp.forward(x);
        let pred = probs.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap().0;
        if pred == *target { correct += 1; per_class[*target] += 1; }
    }
    (correct as f64 / samples.len() as f64, per_class)
}

// ── ADR-120: Windowed MLP training ──────────────────────────────────────────

/// Train a windowed MLP on temporal-window samples.
/// Each sample is a 440-d flat vector (20 frames × 22 features) labeled
/// with a class index. Architecture: 440 → 64 ReLU → n_classes softmax.
/// Same SGD + momentum + cosine-decay recipe as MLP, fewer epochs because
/// each window is a richer training signal than a single frame.
fn train_windowed_mlp_classifier(
    samples: &[(Vec<f64>, usize)],
    n_classes: usize,
) -> WindowedMlpModel {
    let n_w1 = WINDOWED_INPUT * WINDOWED_HIDDEN;
    let n_w2 = WINDOWED_HIDDEN * n_classes;

    let mut rng_state: u64 = 24601;
    let mut rng_u01 = move || -> f64 {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((rng_state >> 33) as f64) / ((u64::MAX >> 33) as f64)
    };
    let mut he_init = |n: usize, fan_in: usize| -> Vec<f64> {
        let s = (2.0 / fan_in as f64).sqrt();
        let mut v = Vec::with_capacity(n);
        let mut k = 0;
        while k < n {
            let u1 = rng_u01().max(1e-12);
            let u2 = rng_u01();
            let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos() * s;
            let z1 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).sin() * s;
            v.push(z0); k += 1;
            if k < n { v.push(z1); k += 1; }
        }
        v
    };

    let mut w1 = he_init(n_w1, WINDOWED_INPUT);
    let mut b1 = vec![0.0f64; WINDOWED_HIDDEN];
    let mut w2 = he_init(n_w2, WINDOWED_HIDDEN);
    let mut b2 = vec![0.0f64; n_classes];

    let mut mw1 = vec![0.0f64; n_w1];
    let mut mb1 = vec![0.0f64; WINDOWED_HIDDEN];
    let mut mw2 = vec![0.0f64; n_w2];
    let mut mb2 = vec![0.0f64; n_classes];

    let momentum = 0.9f64;
    let weight_decay = 1e-4f64;
    let base_lr = 0.03f64; // smaller LR for larger network (vs MLP's 0.05)
    let batch_size = 32usize;
    let epochs = 25usize;
    let n = samples.len();

    let mut idx: Vec<usize> = (0..n).collect();
    let mut shuf_state: u64 = 11;
    let mut shuf_next = move || -> u64 {
        shuf_state = shuf_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        shuf_state >> 33
    };

    let mut h_pre = vec![0.0f64; WINDOWED_HIDDEN];
    let mut h = vec![0.0f64; WINDOWED_HIDDEN];
    let mut logits = vec![0.0f64; n_classes];

    for epoch in 0..epochs {
        for i in (1..idx.len()).rev() {
            let j = (shuf_next() as usize) % (i + 1);
            idx.swap(i, j);
        }
        let lr = base_lr * 0.5 * (1.0 + (std::f64::consts::PI * epoch as f64 / epochs as f64).cos());
        let mut epoch_loss = 0.0f64;

        let mut k = 0usize;
        while k < n {
            let bend = (k + batch_size).min(n);
            let mut gw1 = vec![0.0f64; n_w1];
            let mut gb1 = vec![0.0f64; WINDOWED_HIDDEN];
            let mut gw2 = vec![0.0f64; n_w2];
            let mut gb2 = vec![0.0f64; n_classes];
            let bs = (bend - k) as f64;

            for &si in &idx[k..bend] {
                let (x, target) = &samples[si];
                debug_assert_eq!(x.len(), WINDOWED_INPUT);

                // Forward.
                for j in 0..WINDOWED_HIDDEN {
                    let mut s = b1[j];
                    for i in 0..WINDOWED_INPUT { s += x[i] * w1[i * WINDOWED_HIDDEN + j]; }
                    h_pre[j] = s;
                    h[j] = s.max(0.0);
                }
                for c in 0..n_classes {
                    let mut s = b2[c];
                    for j in 0..WINDOWED_HIDDEN { s += h[j] * w2[j * n_classes + c]; }
                    logits[c] = s;
                }
                let mx = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let ex_sum: f64 = logits.iter().map(|z| (z - mx).exp()).sum();
                let mut d_logits = vec![0.0f64; n_classes];
                for c in 0..n_classes {
                    let p = (logits[c] - mx).exp() / ex_sum;
                    d_logits[c] = p - if c == *target { 1.0 } else { 0.0 };
                    if c == *target { epoch_loss += -(p.max(1e-15)).ln(); }
                }

                for c in 0..n_classes {
                    gb2[c] += d_logits[c];
                    for j in 0..WINDOWED_HIDDEN {
                        gw2[j * n_classes + c] += h[j] * d_logits[c];
                    }
                }
                let mut d_h = vec![0.0f64; WINDOWED_HIDDEN];
                for j in 0..WINDOWED_HIDDEN {
                    if h_pre[j] <= 0.0 { continue; }
                    let mut s = 0.0;
                    for c in 0..n_classes { s += w2[j * n_classes + c] * d_logits[c]; }
                    d_h[j] = s;
                }
                for j in 0..WINDOWED_HIDDEN {
                    gb1[j] += d_h[j];
                    for i in 0..WINDOWED_INPUT { gw1[i * WINDOWED_HIDDEN + j] += x[i] * d_h[j]; }
                }
            }

            for q in 0..n_w1 {
                let g = gw1[q] / bs + weight_decay * w1[q];
                mw1[q] = momentum * mw1[q] + g;
                w1[q] -= lr * mw1[q];
            }
            for q in 0..WINDOWED_HIDDEN {
                let g = gb1[q] / bs;
                mb1[q] = momentum * mb1[q] + g;
                b1[q] -= lr * mb1[q];
            }
            for q in 0..n_w2 {
                let g = gw2[q] / bs + weight_decay * w2[q];
                mw2[q] = momentum * mw2[q] + g;
                w2[q] -= lr * mw2[q];
            }
            for q in 0..n_classes {
                let g = gb2[q] / bs;
                mb2[q] = momentum * mb2[q] + g;
                b2[q] -= lr * mb2[q];
            }

            k = bend;
        }
        if epoch % 3 == 0 || epoch == epochs - 1 {
            eprintln!("  W-MLP epoch {epoch:2}/{}: loss = {:.4}, lr = {:.4}",
                      epochs, epoch_loss / n as f64, lr);
        }
    }

    WindowedMlpModel { w1, b1, w2, b2, n_classes }
}

/// Evaluate Windowed MLP accuracy + per-class correct counts.
fn eval_windowed_mlp(
    mlp: &WindowedMlpModel,
    samples: &[(Vec<f64>, usize)],
    n_classes: usize,
) -> (f64, Vec<usize>) {
    let mut correct = 0usize;
    let mut per_class = vec![0usize; n_classes];
    for (x, target) in samples {
        let probs = mlp.forward(x);
        let pred = probs.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap().0;
        if pred == *target { correct += 1; per_class[*target] += 1; }
    }
    (correct as f64 / samples.len() as f64, per_class)
}

/// Default path for the saved adaptive model.
pub fn model_path() -> PathBuf {
    PathBuf::from("data/adaptive_model.json")
}
