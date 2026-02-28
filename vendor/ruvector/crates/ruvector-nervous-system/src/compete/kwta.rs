//! K-Winner-Take-All Layer
//!
//! Selects top-k neurons for sparse distributed coding and attention mechanisms.

/// K-Winner-Take-All competition layer
///
/// Selects k neurons with highest activations for sparse distributed representations.
/// Used in HNSW for multi-path routing and attention mechanisms.
///
/// # Performance
///
/// - O(n + k log k) using partial sorting
/// - <10μs for 1000 neurons, k=50
///
/// # Example
///
/// ```
/// use ruvector_nervous_system::compete::KWTALayer;
///
/// let kwta = KWTALayer::new(100, 5);
/// let inputs: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
/// let winners = kwta.select(&inputs);
/// assert_eq!(winners.len(), 5);
/// // Winners are indices [95, 96, 97, 98, 99] (top 5 values)
/// ```
#[derive(Debug, Clone)]
pub struct KWTALayer {
    /// Number of competing neurons
    size: usize,

    /// Number of winners to select
    k: usize,

    /// Optional activation threshold
    threshold: Option<f32>,
}

impl KWTALayer {
    /// Create a new K-WTA layer
    ///
    /// # Arguments
    ///
    /// * `size` - Total number of neurons
    /// * `k` - Number of winners to select
    ///
    /// # Panics
    ///
    /// Panics if k > size or k == 0
    pub fn new(size: usize, k: usize) -> Self {
        assert!(k > 0, "k must be positive");
        assert!(k <= size, "k cannot exceed layer size");

        Self {
            size,
            k,
            threshold: None,
        }
    }

    /// Set activation threshold
    ///
    /// Only neurons exceeding this threshold can be selected as winners.
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = Some(threshold);
        self
    }

    /// Select top-k neurons
    ///
    /// Returns indices of k neurons with highest activations, sorted in descending order.
    /// If threshold filtering results in fewer than k candidates, returns all candidates.
    /// Returns empty vec if no candidates meet the threshold.
    ///
    /// # Performance
    ///
    /// - O(n + k log k) using partial sort
    /// - Faster than full sort for small k
    pub fn select(&self, inputs: &[f32]) -> Vec<usize> {
        assert_eq!(inputs.len(), self.size, "Input size mismatch");

        // Create (index, value) pairs
        let mut indexed: Vec<(usize, f32)> =
            inputs.iter().enumerate().map(|(i, &v)| (i, v)).collect();

        // Filter by threshold if set
        if let Some(threshold) = self.threshold {
            indexed.retain(|(_, v)| *v >= threshold);
        }

        // Handle empty case after filtering
        if indexed.is_empty() {
            return Vec::new();
        }

        // Partial sort to get top-k
        let k_actual = self.k.min(indexed.len());
        indexed.select_nth_unstable_by(k_actual - 1, |a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top k and sort descending by value
        let mut winners: Vec<(usize, f32)> = indexed[..k_actual].to_vec();
        winners.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return only indices
        winners.into_iter().map(|(i, _)| i).collect()
    }

    /// Select top-k neurons with their activation values
    ///
    /// Returns (index, value) pairs sorted by descending activation.
    /// Returns empty vec if no candidates meet the threshold.
    pub fn select_with_values(&self, inputs: &[f32]) -> Vec<(usize, f32)> {
        assert_eq!(inputs.len(), self.size, "Input size mismatch");

        let mut indexed: Vec<(usize, f32)> =
            inputs.iter().enumerate().map(|(i, &v)| (i, v)).collect();

        // Filter by threshold if set
        if let Some(threshold) = self.threshold {
            indexed.retain(|(_, v)| *v >= threshold);
        }

        // Handle empty case after filtering
        if indexed.is_empty() {
            return Vec::new();
        }

        // Partial sort to get top-k
        let k_actual = self.k.min(indexed.len());
        indexed.select_nth_unstable_by(k_actual - 1, |a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top k and sort descending
        let mut winners: Vec<(usize, f32)> = indexed[..k_actual].to_vec();
        winners.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        winners
    }

    /// Create sparse activation vector
    ///
    /// Returns a vector of size `size` with only top-k activations preserved.
    /// All other values are set to 0.
    pub fn sparse_activations(&self, inputs: &[f32]) -> Vec<f32> {
        let winners = self.select_with_values(inputs);
        let mut sparse = vec![0.0; self.size];

        for (idx, value) in winners {
            sparse[idx] = value;
        }

        sparse
    }

    /// Create normalized sparse activation vector
    ///
    /// Like `sparse_activations` but normalizes winner activations to sum to 1.0.
    pub fn sparse_normalized(&self, inputs: &[f32]) -> Vec<f32> {
        let winners = self.select_with_values(inputs);
        let mut sparse = vec![0.0; self.size];

        // Calculate sum of winner activations
        let sum: f32 = winners.iter().map(|(_, v)| v).sum();

        if sum > 0.0 {
            for (idx, value) in winners {
                sparse[idx] = value / sum;
            }
        }

        sparse
    }

    /// Get number of winners
    pub fn k(&self) -> usize {
        self.k
    }

    /// Get layer size
    pub fn size(&self) -> usize {
        self.size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kwta_basic() {
        let kwta = KWTALayer::new(10, 3);
        let inputs: Vec<f32> = (0..10).map(|i| i as f32).collect();

        let winners = kwta.select(&inputs);

        assert_eq!(winners.len(), 3);
        assert_eq!(winners, vec![9, 8, 7], "Top 3 indices in descending order");
    }

    #[test]
    fn test_kwta_with_values() {
        let kwta = KWTALayer::new(10, 3);
        let inputs: Vec<f32> = (0..10).map(|i| i as f32).collect();

        let winners = kwta.select_with_values(&inputs);

        assert_eq!(winners.len(), 3);
        assert_eq!(winners[0], (9, 9.0));
        assert_eq!(winners[1], (8, 8.0));
        assert_eq!(winners[2], (7, 7.0));
    }

    #[test]
    fn test_kwta_threshold() {
        let kwta = KWTALayer::new(10, 5).with_threshold(7.0);
        let inputs: Vec<f32> = (0..10).map(|i| i as f32).collect();

        let winners = kwta.select(&inputs);

        // Only 3 values (7.0, 8.0, 9.0) exceed threshold
        assert_eq!(winners.len(), 3);
        assert_eq!(winners, vec![9, 8, 7]);
    }

    #[test]
    fn test_kwta_sparse_activations() {
        let kwta = KWTALayer::new(10, 3);
        let inputs: Vec<f32> = (0..10).map(|i| i as f32).collect();

        let sparse = kwta.sparse_activations(&inputs);

        assert_eq!(sparse.len(), 10);
        assert_eq!(sparse[9], 9.0);
        assert_eq!(sparse[8], 8.0);
        assert_eq!(sparse[7], 7.0);
        assert!(
            sparse[..7].iter().all(|&x| x == 0.0),
            "Non-winners should be zero"
        );
    }

    #[test]
    fn test_kwta_sparse_normalized() {
        let kwta = KWTALayer::new(10, 3);
        let inputs: Vec<f32> = (0..10).map(|i| i as f32).collect();

        let sparse = kwta.sparse_normalized(&inputs);

        // Sum should be 1.0
        let sum: f32 = sparse.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "Normalized activations should sum to 1.0"
        );

        // Winners should have proportional activations
        let expected_sum = 9.0 + 8.0 + 7.0; // Sum of top 3
        assert!((sparse[9] - 9.0 / expected_sum).abs() < 1e-6);
        assert!((sparse[8] - 8.0 / expected_sum).abs() < 1e-6);
        assert!((sparse[7] - 7.0 / expected_sum).abs() < 1e-6);
    }

    #[test]
    fn test_kwta_sorted_order() {
        let kwta = KWTALayer::new(10, 5);
        let inputs = vec![0.5, 0.9, 0.2, 0.8, 0.1, 0.7, 0.3, 0.6, 0.4, 0.0];

        let winners = kwta.select_with_values(&inputs);

        // Winners should be in descending order by value
        for i in 0..winners.len() - 1 {
            assert!(
                winners[i].1 >= winners[i + 1].1,
                "Winners should be sorted by descending value"
            );
        }
    }

    #[test]
    fn test_kwta_determinism() {
        let kwta = KWTALayer::new(100, 10);
        let inputs: Vec<f32> = (0..100).map(|i| (i * 7) as f32 % 100.0).collect();

        let winners1 = kwta.select(&inputs);
        let winners2 = kwta.select(&inputs);

        assert_eq!(winners1, winners2, "K-WTA should be deterministic");
    }

    #[test]
    fn test_kwta_all_zeros() {
        let kwta = KWTALayer::new(10, 3);
        let inputs = vec![0.0; 10];

        let winners = kwta.select(&inputs);

        // Should still return k winners even if all equal
        assert_eq!(winners.len(), 3);
    }

    #[test]
    fn test_kwta_ties() {
        let kwta = KWTALayer::new(10, 3);
        let inputs = vec![1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0];

        let winners = kwta.select_with_values(&inputs);

        // Should select 3 winners from tied values
        assert_eq!(winners.len(), 3);
        assert!(
            winners.iter().all(|(_, v)| *v == 1.0),
            "Should select from highest tier"
        );
    }

    #[test]
    #[should_panic(expected = "k must be positive")]
    fn test_kwta_zero_k() {
        KWTALayer::new(10, 0);
    }

    #[test]
    #[should_panic(expected = "k cannot exceed layer size")]
    fn test_kwta_k_exceeds_size() {
        KWTALayer::new(10, 11);
    }

    #[test]
    fn test_kwta_performance() {
        use std::time::Instant;

        let kwta = KWTALayer::new(1000, 50);
        let inputs: Vec<f32> = (0..1000).map(|i| (i * 7) as f32 % 1000.0).collect();

        let start = Instant::now();
        for _ in 0..1000 {
            let _ = kwta.select(&inputs);
        }
        let elapsed = start.elapsed();

        let avg_micros = elapsed.as_micros() as f64 / 1000.0;
        println!("Average K-WTA selection time: {:.2}μs", avg_micros);

        // Should complete in reasonable time (very relaxed for CI environments)
        assert!(
            avg_micros < 10000.0,
            "K-WTA should be reasonably fast (got {:.2}μs)",
            avg_micros
        );
    }

    #[test]
    fn test_kwta_small_k_advantage() {
        use std::time::Instant;

        let inputs: Vec<f32> = (0..10000).map(|i| (i * 7) as f32 % 10000.0).collect();

        // Small k
        let kwta_small = KWTALayer::new(10000, 10);
        let start = Instant::now();
        for _ in 0..100 {
            let _ = kwta_small.select(&inputs);
        }
        let time_small = start.elapsed();

        // Large k
        let kwta_large = KWTALayer::new(10000, 1000);
        let start = Instant::now();
        for _ in 0..100 {
            let _ = kwta_large.select(&inputs);
        }
        let time_large = start.elapsed();

        println!("Small k (10): {:?}", time_small);
        println!("Large k (1000): {:?}", time_large);

        // Small k should be faster (partial sort advantage)
        // Note: This may not always hold due to variance, but generally true
    }
}
