//! Numerical utility functions

use super::{EPS, LOG_MAX, LOG_MIN};

/// Stable log-sum-exp: log(sum(exp(x_i)))
///
/// Uses the max-trick for numerical stability:
/// log(sum(exp(x_i))) = max_x + log(sum(exp(x_i - max_x)))
#[inline]
pub fn log_sum_exp(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NEG_INFINITY;
    }

    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    if max_val.is_infinite() {
        return max_val;
    }

    let sum: f64 = values.iter().map(|&x| (x - max_val).exp()).sum();
    max_val + sum.ln()
}

/// Stable softmax in log domain
///
/// Returns log(softmax(x)) for numerical stability
#[inline]
pub fn log_softmax(values: &[f64]) -> Vec<f64> {
    let lse = log_sum_exp(values);
    values.iter().map(|&x| x - lse).collect()
}

/// Standard softmax with numerical stability
#[inline]
pub fn softmax(values: &[f64]) -> Vec<f64> {
    if values.is_empty() {
        return vec![];
    }

    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exp_vals: Vec<f64> = values.iter().map(|&x| (x - max_val).exp()).collect();
    let sum: f64 = exp_vals.iter().sum();

    if sum < EPS {
        vec![1.0 / values.len() as f64; values.len()]
    } else {
        exp_vals.iter().map(|&e| e / sum).collect()
    }
}

/// Clamp a log value to prevent overflow/underflow
#[inline]
pub fn clamp_log(x: f64) -> f64 {
    x.clamp(LOG_MIN, LOG_MAX)
}

/// Safe log that returns LOG_MIN for non-positive values
#[inline]
pub fn safe_ln(x: f64) -> f64 {
    if x <= 0.0 {
        LOG_MIN
    } else {
        x.ln().max(LOG_MIN)
    }
}

/// Safe exp that clamps input to prevent overflow
#[inline]
pub fn safe_exp(x: f64) -> f64 {
    clamp_log(x).exp()
}

/// Euclidean norm of a vector
#[inline]
pub fn norm(x: &[f64]) -> f64 {
    x.iter().map(|&v| v * v).sum::<f64>().sqrt()
}

/// Dot product of two vectors
#[inline]
pub fn dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y.iter()).map(|(&a, &b)| a * b).sum()
}

/// Squared Euclidean distance
#[inline]
pub fn squared_euclidean(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y.iter()).map(|(&a, &b)| (a - b).powi(2)).sum()
}

/// Euclidean distance
#[inline]
pub fn euclidean_distance(x: &[f64], y: &[f64]) -> f64 {
    squared_euclidean(x, y).sqrt()
}

/// Normalize a vector to unit length
pub fn normalize(x: &[f64]) -> Vec<f64> {
    let n = norm(x);
    if n < EPS {
        x.to_vec()
    } else {
        x.iter().map(|&v| v / n).collect()
    }
}

/// Normalize vector in place
pub fn normalize_mut(x: &mut [f64]) {
    let n = norm(x);
    if n >= EPS {
        for v in x.iter_mut() {
            *v /= n;
        }
    }
}

/// Cosine similarity between two vectors
#[inline]
pub fn cosine_similarity(x: &[f64], y: &[f64]) -> f64 {
    let dot_prod = dot(x, y);
    let norm_x = norm(x);
    let norm_y = norm(y);

    if norm_x < EPS || norm_y < EPS {
        0.0
    } else {
        (dot_prod / (norm_x * norm_y)).clamp(-1.0, 1.0)
    }
}

/// KL divergence: D_KL(P || Q) = sum(P * log(P/Q))
///
/// Both P and Q must be probability distributions (sum to 1)
pub fn kl_divergence(p: &[f64], q: &[f64]) -> f64 {
    debug_assert_eq!(p.len(), q.len());

    p.iter()
        .zip(q.iter())
        .map(|(&pi, &qi)| {
            if pi < EPS {
                0.0
            } else if qi < EPS {
                f64::INFINITY
            } else {
                pi * (pi / qi).ln()
            }
        })
        .sum()
}

/// Symmetric KL divergence: (D_KL(P||Q) + D_KL(Q||P)) / 2
pub fn symmetric_kl(p: &[f64], q: &[f64]) -> f64 {
    (kl_divergence(p, q) + kl_divergence(q, p)) / 2.0
}

/// Jensen-Shannon divergence
pub fn jensen_shannon(p: &[f64], q: &[f64]) -> f64 {
    let m: Vec<f64> = p
        .iter()
        .zip(q.iter())
        .map(|(&pi, &qi)| (pi + qi) / 2.0)
        .collect();
    (kl_divergence(p, &m) + kl_divergence(q, &m)) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_sum_exp() {
        let values = vec![1.0, 2.0, 3.0];
        let result = log_sum_exp(&values);

        // Manual calculation: log(e^1 + e^2 + e^3)
        let expected = (1.0_f64.exp() + 2.0_f64.exp() + 3.0_f64.exp()).ln();
        assert!((result - expected).abs() < 1e-10);
    }

    #[test]
    fn test_softmax() {
        let values = vec![1.0, 2.0, 3.0];
        let result = softmax(&values);

        // Should sum to 1
        let sum: f64 = result.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);

        // Larger values should have higher probability
        assert!(result[2] > result[1]);
        assert!(result[1] > result[0]);
    }

    #[test]
    fn test_normalize() {
        let x = vec![3.0, 4.0];
        let n = normalize(&x);

        assert!((n[0] - 0.6).abs() < 1e-10);
        assert!((n[1] - 0.8).abs() < 1e-10);

        let norm_result = norm(&n);
        assert!((norm_result - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_kl_divergence() {
        let p = vec![0.25, 0.25, 0.25, 0.25];
        let q = vec![0.25, 0.25, 0.25, 0.25];

        // KL divergence of identical distributions is 0
        assert!(kl_divergence(&p, &q).abs() < 1e-10);
    }
}
