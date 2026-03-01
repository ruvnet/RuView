//! Quantum-inspired search acceleration.
//!
//! Provides [`QuantumSearch`], a collection of quantum-inspired algorithms
//! that accelerate and diversify search results.
//!
//! On native targets the implementation delegates to the `ruqu-algorithms`
//! crate (Grover's amplitude amplification, QAOA for MaxCut). On WASM
//! targets an equivalent classical fallback is provided so that the same
//! API is available everywhere.

/// Quantum-inspired search operations.
///
/// All methods are deterministic and require no quantum hardware; they
/// use classical simulations of quantum algorithms (on native) or
/// purely classical heuristics (on WASM) to improve search result
/// quality.
pub struct QuantumSearch {
    _private: (),
}

impl QuantumSearch {
    /// Create a new `QuantumSearch` instance.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Compute the theoretically optimal number of Grover iterations for
    /// a search space of `search_space_size` items (with a single target).
    ///
    /// Returns `floor(pi/4 * sqrt(N))`, which is at least 1.
    pub fn optimal_iterations(&self, search_space_size: u32) -> u32 {
        if search_space_size <= 1 {
            return 1;
        }
        let n = search_space_size as f64;
        let iters = (std::f64::consts::FRAC_PI_4 * n.sqrt()).floor() as u32;
        iters.max(1)
    }

    /// Select `k` diverse results from a scored set using QAOA-inspired
    /// MaxCut partitioning.
    ///
    /// A similarity graph is built between all result pairs and a
    /// partition is found that maximizes the "cut" between selected and
    /// unselected items. For small `k` (<=8) on native targets the
    /// quantum QAOA solver is used; otherwise a greedy heuristic selects
    /// the next-highest-scoring item that is most different from those
    /// already selected.
    ///
    /// Returns up to `k` items from `scores`, preserving their original
    /// `(id, score)` tuples.
    pub fn diversity_select(&self, scores: &[(String, f32)], k: usize) -> Vec<(String, f32)> {
        if scores.is_empty() || k == 0 {
            return Vec::new();
        }
        let k = k.min(scores.len());

        // Try QAOA path on native for small k.
        #[cfg(not(target_arch = "wasm32"))]
        {
            if k <= 8 {
                if let Some(result) = self.qaoa_diversity_select(scores, k) {
                    return result;
                }
            }
        }

        // Classical greedy fallback (also used on WASM).
        self.greedy_diversity_select(scores, k)
    }

    /// Amplify scores above `target_threshold` and dampen scores below
    /// it, inspired by Grover amplitude amplification.
    ///
    /// Scores above the threshold are boosted by `sqrt(boost_factor)`
    /// and scores below are dampened by `1/sqrt(boost_factor)`. All
    /// scores are then re-normalized to the [0, 1] range.
    ///
    /// The boost factor is derived from the ratio of items above vs
    /// below the threshold, clamped so that results stay meaningful.
    pub fn amplitude_boost(&self, scores: &mut [(String, f32)], target_threshold: f32) {
        if scores.is_empty() {
            return;
        }

        let above_count = scores
            .iter()
            .filter(|(_, s)| *s >= target_threshold)
            .count();
        let below_count = scores.len() - above_count;

        if above_count == 0 || below_count == 0 {
            // All on one side -- nothing useful to amplify.
            return;
        }

        // Boost factor: ratio of total to above (analogous to Grover's
        // N/M amplification), clamped to [1.5, 4.0] to avoid extremes.
        let boost_factor = (scores.len() as f64 / above_count as f64).clamp(1.5, 4.0);
        let sqrt_boost = (boost_factor).sqrt() as f32;
        let inv_sqrt_boost = 1.0 / sqrt_boost;

        for (_id, score) in scores.iter_mut() {
            if *score >= target_threshold {
                *score *= sqrt_boost;
            } else {
                *score *= inv_sqrt_boost;
            }
        }

        // Re-normalize to [0, 1].
        let max_score = scores
            .iter()
            .map(|(_, s)| *s)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_score = scores.iter().map(|(_, s)| *s).fold(f32::INFINITY, f32::min);

        let range = max_score - min_score;
        if range > f32::EPSILON {
            for (_id, score) in scores.iter_mut() {
                *score = (*score - min_score) / range;
            }
        } else {
            // All scores are identical after boost; set to 1.0.
            for (_id, score) in scores.iter_mut() {
                *score = 1.0;
            }
        }
    }

    // ------------------------------------------------------------------
    // Native-only: QAOA diversity selection
    // ------------------------------------------------------------------

    #[cfg(not(target_arch = "wasm32"))]
    fn qaoa_diversity_select(
        &self,
        scores: &[(String, f32)],
        k: usize,
    ) -> Option<Vec<(String, f32)>> {
        use ruqu_algorithms::{run_qaoa, Graph, QaoaConfig};

        let n = scores.len();
        if n < 2 {
            return Some(scores.to_vec());
        }

        // Build a similarity graph: edge weight encodes how *similar*
        // two items are (based on score proximity). QAOA MaxCut will
        // then prefer to *separate* similar items across the partition,
        // giving us diversity.
        let mut graph = Graph::new(n as u32);
        for i in 0..n {
            for j in (i + 1)..n {
                // Similarity = 1 - |score_i - score_j| (higher when scores
                // are close, promoting diversity in the selected set).
                let similarity = 1.0 - (scores[i].1 - scores[j].1).abs();
                graph.add_edge(i as u32, j as u32, similarity as f64);
            }
        }

        let config = QaoaConfig {
            graph,
            p: 2,
            max_iterations: 50,
            learning_rate: 0.1,
            seed: Some(42),
        };

        let result = run_qaoa(&config).ok()?;

        // Collect indices for the partition with the most members near k.
        let partition_true: Vec<usize> = result
            .best_bitstring
            .iter()
            .enumerate()
            .filter(|(_, &b)| b)
            .map(|(i, _)| i)
            .collect();
        let partition_false: Vec<usize> = result
            .best_bitstring
            .iter()
            .enumerate()
            .filter(|(_, &b)| !b)
            .map(|(i, _)| i)
            .collect();

        // Pick the partition closer to size k, then sort by score
        // descending and take the top k.
        let chosen = if (partition_true.len() as isize - k as isize).unsigned_abs()
            <= (partition_false.len() as isize - k as isize).unsigned_abs()
        {
            partition_true
        } else {
            partition_false
        };

        // If neither partition has at least k items, fall back to greedy.
        if chosen.len() < k {
            return None;
        }

        let mut selected: Vec<(String, f32)> = chosen.iter().map(|&i| scores[i].clone()).collect();
        selected.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        selected.truncate(k);

        Some(selected)
    }

    // ------------------------------------------------------------------
    // Classical greedy diversity selection (WASM + large-k fallback)
    // ------------------------------------------------------------------

    fn greedy_diversity_select(&self, scores: &[(String, f32)], k: usize) -> Vec<(String, f32)> {
        let mut remaining: Vec<(usize, &(String, f32))> = scores.iter().enumerate().collect();

        // Sort by score descending to seed with the best item.
        remaining.sort_by(|a, b| {
            b.1 .1
                .partial_cmp(&a.1 .1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut selected: Vec<(String, f32)> = Vec::with_capacity(k);

        // Pick the highest-scoring item first.
        if let Some((_, first)) = remaining.first() {
            selected.push((*first).clone());
        }
        let first_idx = remaining.first().map(|(i, _)| *i);
        remaining.retain(|(i, _)| Some(*i) != first_idx);

        // Greedily pick the next item that maximizes (score * diversity).
        // Diversity is measured as the minimum score-distance from any
        // already-selected item.
        while selected.len() < k && !remaining.is_empty() {
            let mut best_idx_in_remaining = 0;
            let mut best_value = f64::NEG_INFINITY;

            for (ri, (_, candidate)) in remaining.iter().enumerate() {
                let min_dist: f32 = selected
                    .iter()
                    .map(|(_, sel_score)| (candidate.1 - sel_score).abs())
                    .fold(f32::INFINITY, f32::min);

                // Combined objective: high score + high diversity.
                let value = candidate.1 as f64 + min_dist as f64;
                if value > best_value {
                    best_value = value;
                    best_idx_in_remaining = ri;
                }
            }

            let (_, picked) = remaining.remove(best_idx_in_remaining);
            selected.push(picked.clone());
        }

        selected
    }
}

impl Default for QuantumSearch {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimal_iterations_basic() {
        let qs = QuantumSearch::new();
        assert_eq!(qs.optimal_iterations(1), 1);
        assert_eq!(qs.optimal_iterations(4), 1); // pi/4 * 2 = 1.57 -> floor = 1
    }

    #[test]
    fn test_optimal_iterations_larger() {
        let qs = QuantumSearch::new();
        // pi/4 * sqrt(100) = pi/4 * 10 = 7.85 -> floor = 7
        assert_eq!(qs.optimal_iterations(100), 7);
    }

    #[test]
    fn test_diversity_select_empty() {
        let qs = QuantumSearch::new();
        let result = qs.diversity_select(&[], 3);
        assert!(result.is_empty());
    }

    #[test]
    fn test_diversity_select_k_zero() {
        let qs = QuantumSearch::new();
        let scores = vec![("a".to_string(), 0.5)];
        let result = qs.diversity_select(&scores, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_amplitude_boost_empty() {
        let qs = QuantumSearch::new();
        let mut scores: Vec<(String, f32)> = Vec::new();
        qs.amplitude_boost(&mut scores, 0.5);
        assert!(scores.is_empty());
    }

    #[test]
    fn test_amplitude_boost_all_above() {
        let qs = QuantumSearch::new();
        let mut scores = vec![("a".to_string(), 0.8), ("b".to_string(), 0.9)];
        let orig = scores.clone();
        qs.amplitude_boost(&mut scores, 0.5);
        // All above threshold -> no change in relative ordering,
        // but scores remain unchanged since boost is a no-op.
        assert_eq!(scores[0].0, orig[0].0);
        assert_eq!(scores[1].0, orig[1].0);
    }
}
