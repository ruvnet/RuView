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
    let mut samples: Vec<Sample> = Vec::new();
    for (path, fname, class_name) in &file_classes {
        let class_idx = class_map[class_name];
        let loaded = load_recording(path, class_idx);
        eprintln!("  Loaded {}: {} frames → class '{}'",
                 fname, loaded.len(), class_name);
        samples.extend(loaded);
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
        eprintln!("  {}: {}/{} ({:.0}%)", class_names[c], class_correct[c], tot,
                 class_correct[c] as f64 / tot as f64 * 100.0);
    }

    Ok(AdaptiveModel {
        class_stats,
        weights,
        global_mean,
        global_std,
        trained_frames: n,
        training_accuracy: accuracy,
        version: 1,
        class_names,
    })
}

/// Default path for the saved adaptive model.
pub fn model_path() -> PathBuf {
    PathBuf::from("data/adaptive_model.json")
}
