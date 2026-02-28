//! Quality guardrails for attention mechanism output comparison.

use serde::{Deserialize, Serialize};

/// Result of a quality check comparing baseline and gated outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityResult {
    pub cosine_sim: f64,
    pub l2_dist: f64,
    pub passes_threshold: bool,
}

/// Cosine similarity between two vectors. Returns `0.0` for zero-magnitude inputs.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len().min(b.len());
    let (mut dot, mut na, mut nb) = (0.0_f64, 0.0_f64, 0.0_f64);
    for i in 0..n {
        let (ai, bi) = (a[i] as f64, b[i] as f64);
        dot += ai * bi;
        na += ai * ai;
        nb += bi * bi;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom < f64::EPSILON {
        0.0
    } else {
        dot / denom
    }
}

/// Euclidean (L2) distance between two vectors.
pub fn l2_distance(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len().min(b.len());
    let mut s = 0.0_f64;
    for i in 0..n {
        let d = a[i] as f64 - b[i] as f64;
        s += d * d;
    }
    if a.len() > n {
        s += a[n..].iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
    }
    if b.len() > n {
        s += b[n..].iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
    }
    s.sqrt()
}

/// Quality gate: passes when `cosine_similarity >= threshold`.
pub fn quality_check(
    baseline_output: &[f32],
    gated_output: &[f32],
    threshold: f64,
) -> QualityResult {
    let cosine_sim = cosine_similarity(baseline_output, gated_output);
    let l2_dist = l2_distance(baseline_output, gated_output);
    QualityResult {
        cosine_sim,
        l2_dist,
        passes_threshold: cosine_sim >= threshold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_cases() {
        assert!((cosine_similarity(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]) - 1.0).abs() < 1e-10);
        assert!((cosine_similarity(&[1.0, 0.0], &[-1.0, 0.0]) + 1.0).abs() < 1e-10);
        assert!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-10);
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 2.0]), 0.0);
    }

    #[test]
    fn l2_cases() {
        assert!(l2_distance(&[1.0, 2.0], &[1.0, 2.0]) < 1e-10);
        assert!((l2_distance(&[0.0, 0.0], &[3.0, 4.0]) - 5.0).abs() < 1e-10);
        assert!((l2_distance(&[1.0], &[1.0, 3.0]) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn quality_check_pass_and_fail() {
        let r = quality_check(&[1.0, 2.0, 3.0], &[1.1, 2.1, 3.1], 0.99);
        assert!(r.passes_threshold);
        let r2 = quality_check(&[1.0, 0.0], &[0.0, 1.0], 0.5);
        assert!(!r2.passes_threshold);
    }

    #[test]
    fn quality_result_serializable() {
        let r = QualityResult {
            cosine_sim: 0.95,
            l2_dist: 0.32,
            passes_threshold: true,
        };
        let j = serde_json::to_string(&r).unwrap();
        let d: QualityResult = serde_json::from_str(&j).unwrap();
        assert!((d.cosine_sim - 0.95).abs() < 1e-10);
    }
}
