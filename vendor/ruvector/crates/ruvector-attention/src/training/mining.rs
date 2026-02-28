//! Hard negative mining strategies
//!
//! Provides various methods for selecting informative negative samples.

/// Mining strategy enumeration
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum MiningStrategy {
    #[default]
    Random,
    HardNegative,
    SemiHard,
    DistanceWeighted,
}

/// Trait for negative sample mining
pub trait NegativeMiner: Send + Sync {
    /// Mine negatives for an anchor from a candidate pool
    fn mine(
        &self,
        anchor: &[f32],
        positive: &[f32],
        candidates: &[&[f32]],
        num_negatives: usize,
    ) -> Vec<usize>;

    /// Get mining strategy
    fn strategy(&self) -> MiningStrategy;
}

/// Hard negative miner that selects closest negatives
pub struct HardNegativeMiner {
    strategy: MiningStrategy,
    margin: f32,
    temperature: f32,
}

impl HardNegativeMiner {
    pub fn new(strategy: MiningStrategy) -> Self {
        Self {
            strategy,
            margin: 0.1,
            temperature: 1.0,
        }
    }

    pub fn with_margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp;
        self
    }

    fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        dot / (norm_a * norm_b)
    }

    /// Select random indices
    fn random_selection(num_candidates: usize, num_select: usize, seed: u64) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..num_candidates).collect();
        let mut current_seed = seed;

        // Fisher-Yates shuffle
        for i in (1..indices.len()).rev() {
            current_seed = current_seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1);
            let j = (current_seed as usize) % (i + 1);
            indices.swap(i, j);
        }

        indices.truncate(num_select.min(num_candidates));
        indices
    }

    /// Select hardest negatives (closest to anchor)
    fn hard_negative_selection(
        &self,
        anchor: &[f32],
        candidates: &[&[f32]],
        num_select: usize,
    ) -> Vec<usize> {
        let mut indexed_sims: Vec<(usize, f32)> = candidates
            .iter()
            .enumerate()
            .map(|(i, c)| (i, Self::cosine_similarity(anchor, c)))
            .collect();

        // Sort by similarity descending (higher sim = harder negative)
        indexed_sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        indexed_sims
            .into_iter()
            .take(num_select.min(candidates.len()))
            .map(|(i, _)| i)
            .collect()
    }

    /// Select semi-hard negatives (within margin of positive)
    fn semi_hard_selection(
        &self,
        anchor: &[f32],
        positive: &[f32],
        candidates: &[&[f32]],
        num_select: usize,
    ) -> Vec<usize> {
        let d_pos = Self::euclidean_distance(anchor, positive);

        let mut semi_hard: Vec<(usize, f32)> = candidates
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let d_neg = Self::euclidean_distance(anchor, c);
                // Semi-hard: d_pos < d_neg < d_pos + margin
                if d_neg > d_pos && d_neg < d_pos + self.margin {
                    Some((i, d_neg))
                } else {
                    None
                }
            })
            .collect();

        // Sort by distance (prefer harder ones)
        semi_hard.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut result: Vec<usize> = semi_hard.into_iter().map(|(i, _)| i).collect();

        // If not enough semi-hard, fill with hard negatives
        if result.len() < num_select {
            let hard = self.hard_negative_selection(anchor, candidates, num_select - result.len());
            for idx in hard {
                if !result.contains(&idx) {
                    result.push(idx);
                }
            }
        }

        result.truncate(num_select);
        result
    }

    /// Distance-weighted sampling
    fn distance_weighted_selection(
        &self,
        anchor: &[f32],
        candidates: &[&[f32]],
        num_select: usize,
    ) -> Vec<usize> {
        if candidates.is_empty() {
            return vec![];
        }

        // Compute weights based on similarity (closer = higher weight)
        let sims: Vec<f32> = candidates
            .iter()
            .map(|c| Self::cosine_similarity(anchor, c) / self.temperature)
            .collect();

        // Softmax weights
        let max_sim = sims.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let exp_sims: Vec<f32> = sims.iter().map(|s| (s - max_sim).exp()).collect();
        let sum_exp: f32 = exp_sims.iter().sum();
        let probs: Vec<f32> = exp_sims.iter().map(|e| e / sum_exp).collect();

        // Sample without replacement using the probabilities
        let mut remaining: Vec<(usize, f32)> = probs.into_iter().enumerate().collect();
        let mut selected = Vec::with_capacity(num_select);
        let mut seed = 42u64;

        while selected.len() < num_select && !remaining.is_empty() {
            // Random value
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let r = (seed as f32) / (u64::MAX as f32);

            // Select based on cumulative probability
            let total: f32 = remaining.iter().map(|(_, p)| p).sum();
            let mut cumsum = 0.0;
            let mut select_idx = 0;

            for (i, (_, p)) in remaining.iter().enumerate() {
                cumsum += p / total;
                if r < cumsum {
                    select_idx = i;
                    break;
                }
            }

            let (orig_idx, _) = remaining.remove(select_idx);
            selected.push(orig_idx);
        }

        selected
    }
}

impl NegativeMiner for HardNegativeMiner {
    fn mine(
        &self,
        anchor: &[f32],
        positive: &[f32],
        candidates: &[&[f32]],
        num_negatives: usize,
    ) -> Vec<usize> {
        match self.strategy {
            MiningStrategy::Random => Self::random_selection(candidates.len(), num_negatives, 42),
            MiningStrategy::HardNegative => {
                self.hard_negative_selection(anchor, candidates, num_negatives)
            }
            MiningStrategy::SemiHard => {
                self.semi_hard_selection(anchor, positive, candidates, num_negatives)
            }
            MiningStrategy::DistanceWeighted => {
                self.distance_weighted_selection(anchor, candidates, num_negatives)
            }
        }
    }

    fn strategy(&self) -> MiningStrategy {
        self.strategy
    }
}

/// In-batch negative mining (uses other batch items as negatives)
pub struct InBatchMiner {
    exclude_positive: bool,
}

impl InBatchMiner {
    pub fn new() -> Self {
        Self {
            exclude_positive: true,
        }
    }

    pub fn include_positive(mut self) -> Self {
        self.exclude_positive = false;
        self
    }

    /// Get negative indices from a batch for a given anchor index
    pub fn get_negatives(
        &self,
        anchor_idx: usize,
        positive_idx: usize,
        batch_size: usize,
    ) -> Vec<usize> {
        (0..batch_size)
            .filter(|&i| i != anchor_idx && (!self.exclude_positive || i != positive_idx))
            .collect()
    }
}

impl Default for InBatchMiner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_mining() {
        let miner = HardNegativeMiner::new(MiningStrategy::Random);

        let anchor = vec![1.0, 0.0, 0.0];
        let positive = vec![0.9, 0.1, 0.0];
        let candidates: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32 * 0.05; 3]).collect();
        let cand_refs: Vec<&[f32]> = candidates.iter().map(|c| c.as_slice()).collect();

        let selected = miner.mine(&anchor, &positive, &cand_refs, 5);
        assert_eq!(selected.len(), 5);
    }

    #[test]
    fn test_hard_negative_mining() {
        let miner = HardNegativeMiner::new(MiningStrategy::HardNegative);

        let anchor = vec![1.0, 0.0, 0.0];
        let positive = vec![0.9, 0.1, 0.0];
        // Create candidates with varying similarity to anchor
        let candidates: Vec<Vec<f32>> = vec![
            vec![0.9, 0.1, 0.0], // Similar to anchor
            vec![0.5, 0.5, 0.0], // Medium
            vec![0.0, 1.0, 0.0], // Different
            vec![0.0, 0.0, 1.0], // Different
        ];
        let cand_refs: Vec<&[f32]> = candidates.iter().map(|c| c.as_slice()).collect();

        let selected = miner.mine(&anchor, &positive, &cand_refs, 2);

        // Should select the most similar ones first
        assert!(selected.contains(&0)); // Most similar
    }

    #[test]
    fn test_semi_hard_mining() {
        let miner = HardNegativeMiner::new(MiningStrategy::SemiHard).with_margin(1.0);

        let anchor = vec![0.0, 0.0];
        let positive = vec![0.5, 0.0]; // Distance 0.5
        let candidates: Vec<Vec<f32>> = vec![
            vec![0.3, 0.0], // Too easy (d = 0.3 < 0.5)
            vec![0.7, 0.0], // Semi-hard (0.5 < 0.7 < 1.5)
            vec![1.0, 0.0], // Semi-hard
            vec![3.0, 0.0], // Too hard (d = 3.0 > 1.5)
        ];
        let cand_refs: Vec<&[f32]> = candidates.iter().map(|c| c.as_slice()).collect();

        let selected = miner.mine(&anchor, &positive, &cand_refs, 2);
        assert!(!selected.is_empty());
    }

    #[test]
    fn test_distance_weighted() {
        let miner = HardNegativeMiner::new(MiningStrategy::DistanceWeighted).with_temperature(0.5);

        let anchor = vec![1.0, 0.0];
        let positive = vec![0.9, 0.1];
        let candidates: Vec<Vec<f32>> = (0..10).map(|i| vec![0.1 * i as f32; 2]).collect();
        let cand_refs: Vec<&[f32]> = candidates.iter().map(|c| c.as_slice()).collect();

        let selected = miner.mine(&anchor, &positive, &cand_refs, 3);
        assert_eq!(selected.len(), 3);
    }

    #[test]
    fn test_in_batch_miner() {
        let miner = InBatchMiner::new();

        let negatives = miner.get_negatives(2, 5, 10);

        assert!(!negatives.contains(&2)); // Exclude anchor
        assert!(!negatives.contains(&5)); // Exclude positive
        assert_eq!(negatives.len(), 8);
    }
}
