//! Core coherence metrics for attention mechanism evaluation.

use serde::{Deserialize, Serialize};

/// Result of comparing baseline vs. gated attention outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaMetric {
    pub coherence_delta: f64,
    pub decision_flips: usize,
    pub path_length_change: f64,
}

/// Measures the rate of contradictory outputs (negative dot product) between pairs.
pub fn contradiction_rate(predictions: &[Vec<f32>], references: &[Vec<f32>]) -> f64 {
    if predictions.is_empty() || references.is_empty() {
        return 0.0;
    }
    let n = predictions.len().min(references.len());
    let contradictions = predictions[..n]
        .iter()
        .zip(&references[..n])
        .filter(|(p, r)| {
            p.iter()
                .zip(r.iter())
                .map(|(a, b)| *a as f64 * *b as f64)
                .sum::<f64>()
                < 0.0
        })
        .count();
    contradictions as f64 / n as f64
}

/// Mean pairwise cosine similarity between consecutive output vectors.
pub fn entailment_consistency(outputs: &[Vec<f32>]) -> f64 {
    if outputs.len() < 2 {
        return 1.0;
    }
    let pairs = outputs.len() - 1;
    let total: f64 = (0..pairs)
        .map(|i| cosine(&outputs[i], &outputs[i + 1]))
        .sum();
    total / pairs as f64
}

/// Computes the behavioral delta between baseline and gated attention outputs.
pub fn delta_behavior(baseline_outputs: &[f32], gated_outputs: &[f32]) -> DeltaMetric {
    let n = baseline_outputs.len().min(gated_outputs.len());
    if n == 0 {
        return DeltaMetric {
            coherence_delta: 0.0,
            decision_flips: 0,
            path_length_change: 0.0,
        };
    }
    let (bl, gl) = (&baseline_outputs[..n], &gated_outputs[..n]);
    let coherence_delta = cosine(bl, gl) - 1.0;
    let decision_flips = bl
        .iter()
        .zip(gl)
        .filter(|(b, g)| b.is_sign_positive() != g.is_sign_positive())
        .count();
    let bn = l2_norm(bl);
    let path_length_change = if bn > f64::EPSILON {
        l2_norm(gl) / bn - 1.0
    } else {
        0.0
    };
    DeltaMetric {
        coherence_delta,
        decision_flips,
        path_length_change,
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum();
    let denom = l2_norm(a) * l2_norm(b);
    if denom < f64::EPSILON {
        0.0
    } else {
        dot / denom
    }
}

fn l2_norm(v: &[f32]) -> f64 {
    v.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contradiction_rate_boundaries() {
        let preds = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        assert_eq!(
            contradiction_rate(&preds, &[vec![1.0, 1.0], vec![1.0, 1.0]]),
            0.0
        );
        assert_eq!(
            contradiction_rate(&preds, &[vec![-1.0, -1.0], vec![-1.0, -1.0]]),
            1.0
        );
        assert_eq!(contradiction_rate(&[], &[]), 0.0);
    }

    #[test]
    fn entailment_consistency_cases() {
        let identical = vec![vec![1.0, 0.0]; 3];
        assert!((entailment_consistency(&identical) - 1.0).abs() < 1e-10);
        assert_eq!(entailment_consistency(&[vec![1.0]]), 1.0);
        let ortho = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        assert!(entailment_consistency(&ortho).abs() < 1e-10);
    }

    #[test]
    fn delta_behavior_cases() {
        let v = vec![1.0, 2.0, 3.0];
        let d = delta_behavior(&v, &v);
        assert!(d.coherence_delta.abs() < 1e-10);
        assert_eq!(d.decision_flips, 0);

        let d2 = delta_behavior(&[1.0, -1.0, 1.0], &[-1.0, 1.0, 1.0]);
        assert_eq!(d2.decision_flips, 2);

        let d3 = delta_behavior(&[], &[]);
        assert_eq!(d3.decision_flips, 0);
    }
}
