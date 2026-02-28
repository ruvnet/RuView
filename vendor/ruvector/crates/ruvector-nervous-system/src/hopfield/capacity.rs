//! Capacity calculations and β parameter tuning
//!
//! This module provides utilities for calculating theoretical storage
//! capacity and determining optimal β values for Modern Hopfield networks.

/// Calculate theoretical storage capacity
///
/// Returns 2^(d/2) where d is the dimension.
///
/// For Modern Hopfield Networks, the theoretical storage capacity
/// is exponential in the dimension, specifically 2^(d/2) patterns.
///
/// # Arguments
///
/// * `dimension` - Vector dimensionality
///
/// # Examples
///
/// ```rust
/// use ruvector_nervous_system::hopfield::theoretical_capacity;
///
/// assert_eq!(theoretical_capacity(64), 2_u64.pow(32));  // 4 billion patterns
/// assert_eq!(theoretical_capacity(128), u64::MAX);      // saturates for d >= 128
/// ```
pub fn theoretical_capacity(dimension: usize) -> u64 {
    let exponent = dimension / 2;
    if exponent >= 64 {
        u64::MAX
    } else {
        2_u64.pow(exponent as u32)
    }
}

/// Calculate optimal beta for given number of patterns
///
/// The β parameter controls the sharpness of the softmax attention.
/// Higher β values make the attention more concentrated on the best
/// match, while lower values distribute attention more evenly.
///
/// Rule of thumb:
/// - β ≈ 1/√d for random patterns
/// - β ≈ ln(N) for N patterns (information-theoretic optimum)
/// - β ∈ [0.5, 10.0] for practical applications
///
/// # Arguments
///
/// * `num_patterns` - Number of stored patterns
/// * `dimension` - Vector dimensionality
///
/// # Returns
///
/// Recommended β value
///
/// # Examples
///
/// ```rust
/// use ruvector_nervous_system::hopfield::optimal_beta;
///
/// let beta = optimal_beta(100, 128);
/// assert!(beta > 0.0 && beta < 20.0);
/// ```
pub fn optimal_beta(num_patterns: usize, dimension: usize) -> f32 {
    if num_patterns == 0 {
        return 1.0;
    }

    // Use information-theoretic optimum: β ≈ ln(N)
    let ln_n = (num_patterns as f32).ln();

    // Scale by dimension
    let dim_factor = 1.0 / (dimension as f32).sqrt();

    // Combine and clamp to reasonable range
    let beta = ln_n * (1.0 + dim_factor);
    beta.clamp(0.5, 10.0)
}

/// Calculate separation ratio between patterns
///
/// Measures how well-separated patterns are in the network.
/// Higher values indicate better separation and more reliable retrieval.
///
/// # Arguments
///
/// * `patterns` - Stored patterns
///
/// # Returns
///
/// Separation ratio (average minimum distance / average distance)
pub fn separation_ratio(patterns: &[Vec<f32>]) -> f32 {
    if patterns.len() < 2 {
        return 0.0;
    }

    let n = patterns.len();
    let mut min_distances = vec![f32::MAX; n];
    let mut total_distance = 0.0;
    let mut count = 0;

    for i in 0..n {
        for j in (i + 1)..n {
            let dist = euclidean_distance(&patterns[i], &patterns[j]);
            total_distance += dist;
            count += 1;

            min_distances[i] = min_distances[i].min(dist);
            min_distances[j] = min_distances[j].min(dist);
        }
    }

    if count == 0 {
        return 0.0;
    }

    let avg_distance = total_distance / (count as f32);
    let avg_min_distance = min_distances.iter().sum::<f32>() / (n as f32);

    if avg_distance == 0.0 {
        0.0
    } else {
        avg_min_distance / avg_distance
    }
}

/// Calculate Euclidean distance between two vectors
fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Estimate retrieval accuracy for given β
///
/// Uses empirical formula based on pattern separation and temperature.
/// Returns estimated probability of correct retrieval.
///
/// # Arguments
///
/// * `beta` - Temperature parameter
/// * `patterns` - Stored patterns
///
/// # Returns
///
/// Estimated accuracy in [0, 1]
pub fn estimate_accuracy(beta: f32, patterns: &[Vec<f32>]) -> f32 {
    if patterns.is_empty() {
        return 0.0;
    }

    let sep = separation_ratio(patterns);

    // Empirical formula: accuracy ≈ sigmoid(β * separation - threshold)
    let threshold = 1.0;
    let x = beta * sep - threshold;

    // Sigmoid function
    1.0 / (1.0 + (-x).exp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_theoretical_capacity() {
        assert_eq!(theoretical_capacity(2), 2);
        assert_eq!(theoretical_capacity(4), 4);
        assert_eq!(theoretical_capacity(8), 16);
        assert_eq!(theoretical_capacity(64), 2_u64.pow(32));
        // d=128 has exponent=64 which saturates
        assert_eq!(theoretical_capacity(128), u64::MAX);
    }

    #[test]
    fn test_theoretical_capacity_large() {
        // Should saturate to u64::MAX for very large dimensions
        assert_eq!(theoretical_capacity(256), u64::MAX);
        assert_eq!(theoretical_capacity(512), u64::MAX);
    }

    #[test]
    fn test_optimal_beta_zero_patterns() {
        let beta = optimal_beta(0, 128);
        assert_relative_eq!(beta, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn test_optimal_beta_range() {
        for num_patterns in [10, 100, 1000, 10000] {
            let beta = optimal_beta(num_patterns, 128);
            assert!(beta >= 0.5 && beta <= 10.0, "Beta {} out of range", beta);
        }
    }

    #[test]
    fn test_optimal_beta_increases_with_patterns() {
        let beta_10 = optimal_beta(10, 128);
        let beta_100 = optimal_beta(100, 128);
        let beta_1000 = optimal_beta(1000, 128);

        // Beta should generally increase with more patterns
        assert!(beta_100 >= beta_10);
        assert!(beta_1000 >= beta_100);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        let dist = euclidean_distance(&a, &b);
        assert_relative_eq!(dist, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn test_euclidean_distance_diagonal() {
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 1.0];

        let dist = euclidean_distance(&a, &b);
        assert_relative_eq!(dist, 2.0_f32.sqrt(), epsilon = 1e-6);
    }

    #[test]
    fn test_separation_ratio_empty() {
        let patterns: Vec<Vec<f32>> = Vec::new();
        let ratio = separation_ratio(&patterns);
        assert_relative_eq!(ratio, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn test_separation_ratio_single() {
        let patterns = vec![vec![1.0, 2.0, 3.0]];
        let ratio = separation_ratio(&patterns);
        assert_relative_eq!(ratio, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn test_separation_ratio_orthogonal() {
        // Orthogonal patterns are well-separated
        let patterns = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];

        let ratio = separation_ratio(&patterns);

        // For orthogonal patterns, all distances are equal
        // So ratio should be close to 1.0
        assert!(ratio > 0.9 && ratio <= 1.0);
    }

    #[test]
    fn test_separation_ratio_close_patterns() {
        // Two patterns very close together
        let patterns = vec![vec![1.0, 0.0], vec![1.01, 0.0]];

        let ratio = separation_ratio(&patterns);

        // Minimum distance equals average distance for 2 patterns
        assert_relative_eq!(ratio, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn test_estimate_accuracy_empty() {
        let patterns: Vec<Vec<f32>> = Vec::new();
        let accuracy = estimate_accuracy(1.0, &patterns);
        assert_relative_eq!(accuracy, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn test_estimate_accuracy_range() {
        let patterns = vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]];

        for beta in [0.1, 0.5, 1.0, 2.0, 5.0, 10.0] {
            let accuracy = estimate_accuracy(beta, &patterns);
            assert!(accuracy >= 0.0 && accuracy <= 1.0);
        }
    }

    #[test]
    fn test_estimate_accuracy_increases_with_beta() {
        let patterns = vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]];

        let acc_low = estimate_accuracy(0.5, &patterns);
        let acc_high = estimate_accuracy(5.0, &patterns);

        // Higher beta should give better accuracy for well-separated patterns
        assert!(acc_high >= acc_low);
    }
}
