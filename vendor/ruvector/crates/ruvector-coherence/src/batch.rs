//! Batched evaluation over multiple samples.

use serde::{Deserialize, Serialize};

use crate::metrics::delta_behavior;
use crate::quality::quality_check;

/// Aggregated results from evaluating a batch of baseline/gated output pairs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub mean_coherence_delta: f64,
    pub std_coherence_delta: f64,
    pub ci_95_lower: f64,
    pub ci_95_upper: f64,
    pub n_samples: usize,
    pub pass_rate: f64,
}

/// Evaluates a batch of output pairs, producing mean/std/CI for coherence delta and pass rate.
pub fn evaluate_batch(
    baseline_outputs: &[Vec<f32>],
    gated_outputs: &[Vec<f32>],
    threshold: f64,
) -> BatchResult {
    let n = baseline_outputs.len().min(gated_outputs.len());
    if n == 0 {
        return BatchResult {
            mean_coherence_delta: 0.0,
            std_coherence_delta: 0.0,
            ci_95_lower: 0.0,
            ci_95_upper: 0.0,
            n_samples: 0,
            pass_rate: 0.0,
        };
    }

    let mut deltas = Vec::with_capacity(n);
    let mut passes = 0usize;
    for i in 0..n {
        deltas.push(delta_behavior(&baseline_outputs[i], &gated_outputs[i]).coherence_delta);
        if quality_check(&baseline_outputs[i], &gated_outputs[i], threshold).passes_threshold {
            passes += 1;
        }
    }

    let mean = deltas.iter().sum::<f64>() / n as f64;
    let var = if n > 1 {
        deltas.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n - 1) as f64
    } else {
        0.0
    };
    let std_dev = var.sqrt();
    let margin = 1.96 * std_dev / (n as f64).sqrt();

    BatchResult {
        mean_coherence_delta: mean,
        std_coherence_delta: std_dev,
        ci_95_lower: mean - margin,
        ci_95_upper: mean + margin,
        n_samples: n,
        pass_rate: passes as f64 / n as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_empty() {
        let r = evaluate_batch(&[], &[], 0.9);
        assert_eq!(r.n_samples, 0);
    }

    #[test]
    fn batch_identical() {
        let bl = vec![vec![1.0, 2.0, 3.0]; 10];
        let r = evaluate_batch(&bl, &bl.clone(), 0.9);
        assert_eq!(r.n_samples, 10);
        assert!(r.mean_coherence_delta.abs() < 1e-10);
        assert!((r.pass_rate - 1.0).abs() < 1e-10);
    }

    #[test]
    fn batch_ci_contains_mean() {
        let bl = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
            vec![2.0, 3.0],
        ];
        let gt = vec![
            vec![1.1, 0.1],
            vec![0.1, 1.1],
            vec![1.2, 0.9],
            vec![2.1, 2.9],
        ];
        let r = evaluate_batch(&bl, &gt, 0.9);
        assert!(r.ci_95_lower <= r.mean_coherence_delta);
        assert!(r.ci_95_upper >= r.mean_coherence_delta);
    }

    #[test]
    fn batch_pass_rate_partial() {
        let bl = vec![vec![1.0, 0.0], vec![1.0, 0.0]];
        let gt = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let r = evaluate_batch(&bl, &gt, 0.5);
        assert!((r.pass_rate - 0.5).abs() < 1e-10);
    }

    #[test]
    fn batch_result_serializable() {
        let r = BatchResult {
            mean_coherence_delta: -0.05,
            std_coherence_delta: 0.02,
            ci_95_lower: -0.07,
            ci_95_upper: -0.03,
            n_samples: 100,
            pass_rate: 0.95,
        };
        let d: BatchResult = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(d.n_samples, 100);
    }
}
