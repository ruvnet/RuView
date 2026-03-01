//! Federated aggregation: FedAvg, FedProx, Byzantine-tolerant weighted averaging.

use crate::error::FederationError;
use crate::types::AggregateWeights;

/// Aggregation strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregationStrategy {
    /// Federated Averaging (McMahan et al., 2017).
    FedAvg,
    /// Federated Proximal (Li et al., 2020).
    FedProx { mu: u32 },
    /// Simple weighted average.
    WeightedAverage,
}

impl Default for AggregationStrategy {
    fn default() -> Self {
        Self::FedAvg
    }
}

/// A single contribution to a federated averaging round.
#[derive(Clone, Debug)]
pub struct Contribution {
    /// Contributor pseudonym.
    pub contributor: String,
    /// Weight vector (LoRA deltas).
    pub weights: Vec<f64>,
    /// Quality/reputation weight for this contributor.
    pub quality_weight: f64,
    /// Number of training trajectories behind this contribution.
    pub trajectory_count: u64,
}

/// Federated aggregation server.
pub struct FederatedAggregator {
    /// Aggregation strategy.
    strategy: AggregationStrategy,
    /// Domain identifier.
    domain_id: String,
    /// Current round number.
    round: u64,
    /// Minimum contributions required for a round.
    min_contributions: usize,
    /// Standard deviation threshold for Byzantine outlier detection.
    byzantine_std_threshold: f64,
    /// Collected contributions for the current round.
    contributions: Vec<Contribution>,
}

impl FederatedAggregator {
    /// Create a new aggregator.
    pub fn new(domain_id: String, strategy: AggregationStrategy) -> Self {
        Self {
            strategy,
            domain_id,
            round: 0,
            min_contributions: 2,
            byzantine_std_threshold: 2.0,
            contributions: Vec::new(),
        }
    }

    /// Set minimum contributions required.
    pub fn with_min_contributions(mut self, min: usize) -> Self {
        self.min_contributions = min;
        self
    }

    /// Set Byzantine outlier threshold (in standard deviations).
    pub fn with_byzantine_threshold(mut self, threshold: f64) -> Self {
        self.byzantine_std_threshold = threshold;
        self
    }

    /// Add a contribution for the current round.
    pub fn add_contribution(&mut self, contribution: Contribution) {
        self.contributions.push(contribution);
    }

    /// Number of contributions collected so far.
    pub fn contribution_count(&self) -> usize {
        self.contributions.len()
    }

    /// Current round number.
    pub fn round(&self) -> u64 {
        self.round
    }

    /// Check if we have enough contributions to aggregate.
    pub fn ready(&self) -> bool {
        self.contributions.len() >= self.min_contributions
    }

    /// Detect and remove Byzantine outliers.
    ///
    /// Returns the number of outliers removed.
    fn remove_byzantine_outliers(&mut self) -> u32 {
        if self.contributions.len() < 3 {
            return 0; // Need at least 3 for meaningful outlier detection
        }

        let dim = self.contributions[0].weights.len();
        if dim == 0 || !self.contributions.iter().all(|c| c.weights.len() == dim) {
            return 0;
        }

        // Compute mean and std of L2 norms
        let norms: Vec<f64> = self.contributions.iter()
            .map(|c| c.weights.iter().map(|w| w * w).sum::<f64>().sqrt())
            .collect();

        let mean_norm = norms.iter().sum::<f64>() / norms.len() as f64;
        let variance = norms.iter().map(|n| (n - mean_norm).powi(2)).sum::<f64>() / norms.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev < 1e-10 {
            return 0;
        }

        let original_count = self.contributions.len();
        let threshold = self.byzantine_std_threshold;

        self.contributions.retain(|c| {
            let norm = c.weights.iter().map(|w| w * w).sum::<f64>().sqrt();
            ((norm - mean_norm) / std_dev).abs() <= threshold
        });

        (original_count - self.contributions.len()) as u32
    }

    /// Aggregate contributions and produce an `AggregateWeights` segment.
    pub fn aggregate(&mut self) -> Result<AggregateWeights, FederationError> {
        if self.contributions.len() < self.min_contributions {
            return Err(FederationError::InsufficientContributions {
                min: self.min_contributions,
                got: self.contributions.len(),
            });
        }

        // Byzantine outlier removal
        let outliers_removed = self.remove_byzantine_outliers();

        if self.contributions.is_empty() {
            return Err(FederationError::InsufficientContributions {
                min: self.min_contributions,
                got: 0,
            });
        }

        let dim = self.contributions[0].weights.len();

        let result = match self.strategy {
            AggregationStrategy::FedAvg => self.fedavg(dim),
            AggregationStrategy::FedProx { mu } => self.fedprox(dim, mu as f64 / 100.0),
            AggregationStrategy::WeightedAverage => self.weighted_avg(dim),
        };

        self.round += 1;
        let participation_count = self.contributions.len() as u32;

        // Compute loss stats
        let losses: Vec<f64> = self.contributions.iter()
            .map(|c| {
                // Use inverse quality as a proxy for loss
                1.0 - c.quality_weight.clamp(0.0, 1.0)
            })
            .collect();
        let mean_loss = losses.iter().sum::<f64>() / losses.len() as f64;
        let loss_variance = losses.iter().map(|l| (l - mean_loss).powi(2)).sum::<f64>() / losses.len() as f64;

        self.contributions.clear();

        Ok(AggregateWeights {
            round: self.round,
            participation_count,
            lora_deltas: result.0,
            confidences: result.1,
            mean_loss,
            loss_variance,
            domain_id: self.domain_id.clone(),
            byzantine_filtered: outliers_removed > 0,
            outliers_removed,
        })
    }

    /// FedAvg: weighted average by trajectory count.
    fn fedavg(&self, dim: usize) -> (Vec<f64>, Vec<f64>) {
        let total_trajectories: f64 = self.contributions.iter()
            .map(|c| c.trajectory_count as f64)
            .sum();

        let mut avg = vec![0.0f64; dim];
        let mut confidences = vec![0.0f64; dim];

        if total_trajectories <= 0.0 {
            return (avg, confidences);
        }

        for c in &self.contributions {
            let w = c.trajectory_count as f64 / total_trajectories;
            for (i, val) in c.weights.iter().enumerate() {
                if i < dim {
                    avg[i] += w * val;
                }
            }
        }

        // Confidence = inverse of variance across contributions per dimension
        for i in 0..dim {
            let mean = avg[i];
            let var: f64 = self.contributions.iter()
                .map(|c| {
                    let v = if i < c.weights.len() { c.weights[i] } else { 0.0 };
                    (v - mean).powi(2)
                })
                .sum::<f64>() / self.contributions.len() as f64;
            confidences[i] = 1.0 / (1.0 + var);
        }

        (avg, confidences)
    }

    /// FedProx: weighted average with proximal term.
    fn fedprox(&self, dim: usize, mu: f64) -> (Vec<f64>, Vec<f64>) {
        let (mut avg, confidences) = self.fedavg(dim);
        // Apply proximal regularization: pull toward zero (global model)
        for val in &mut avg {
            *val *= 1.0 / (1.0 + mu);
        }
        (avg, confidences)
    }

    /// Weighted average by quality_weight.
    fn weighted_avg(&self, dim: usize) -> (Vec<f64>, Vec<f64>) {
        let total_weight: f64 = self.contributions.iter().map(|c| c.quality_weight).sum();

        let mut avg = vec![0.0f64; dim];
        let mut confidences = vec![0.0f64; dim];

        if total_weight <= 0.0 {
            return (avg, confidences);
        }

        for c in &self.contributions {
            let w = c.quality_weight / total_weight;
            for (i, val) in c.weights.iter().enumerate() {
                if i < dim {
                    avg[i] += w * val;
                }
            }
        }

        for i in 0..dim {
            let mean = avg[i];
            let var: f64 = self.contributions.iter()
                .map(|c| {
                    let v = if i < c.weights.len() { c.weights[i] } else { 0.0 };
                    (v - mean).powi(2)
                })
                .sum::<f64>() / self.contributions.len() as f64;
            confidences[i] = 1.0 / (1.0 + var);
        }

        (avg, confidences)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_contribution(name: &str, weights: Vec<f64>, quality: f64, trajectories: u64) -> Contribution {
        Contribution {
            contributor: name.to_string(),
            weights,
            quality_weight: quality,
            trajectory_count: trajectories,
        }
    }

    #[test]
    fn fedavg_two_equal_contributions() {
        let mut agg = FederatedAggregator::new("test".into(), AggregationStrategy::FedAvg)
            .with_min_contributions(2);

        agg.add_contribution(make_contribution("a", vec![1.0, 2.0, 3.0], 1.0, 100));
        agg.add_contribution(make_contribution("b", vec![3.0, 4.0, 5.0], 1.0, 100));

        let result = agg.aggregate().unwrap();
        assert_eq!(result.round, 1);
        assert_eq!(result.participation_count, 2);
        assert!((result.lora_deltas[0] - 2.0).abs() < 1e-10);
        assert!((result.lora_deltas[1] - 3.0).abs() < 1e-10);
        assert!((result.lora_deltas[2] - 4.0).abs() < 1e-10);
    }

    #[test]
    fn fedavg_weighted_by_trajectories() {
        let mut agg = FederatedAggregator::new("test".into(), AggregationStrategy::FedAvg)
            .with_min_contributions(2);

        // A has 3x more trajectories, so A's values should dominate
        agg.add_contribution(make_contribution("a", vec![10.0], 1.0, 300));
        agg.add_contribution(make_contribution("b", vec![0.0], 1.0, 100));

        let result = agg.aggregate().unwrap();
        // (300*10 + 100*0) / 400 = 7.5
        assert!((result.lora_deltas[0] - 7.5).abs() < 1e-10);
    }

    #[test]
    fn fedprox_shrinks_toward_zero() {
        let mut agg_avg = FederatedAggregator::new("test".into(), AggregationStrategy::FedAvg)
            .with_min_contributions(2);
        agg_avg.add_contribution(make_contribution("a", vec![10.0], 1.0, 100));
        agg_avg.add_contribution(make_contribution("b", vec![10.0], 1.0, 100));
        let avg_result = agg_avg.aggregate().unwrap();

        let mut agg_prox = FederatedAggregator::new("test".into(), AggregationStrategy::FedProx { mu: 50 })
            .with_min_contributions(2);
        agg_prox.add_contribution(make_contribution("a", vec![10.0], 1.0, 100));
        agg_prox.add_contribution(make_contribution("b", vec![10.0], 1.0, 100));
        let prox_result = agg_prox.aggregate().unwrap();

        // FedProx should produce smaller values due to proximal regularization
        assert!(prox_result.lora_deltas[0] < avg_result.lora_deltas[0]);
    }

    #[test]
    fn byzantine_outlier_removal() {
        let mut agg = FederatedAggregator::new("test".into(), AggregationStrategy::FedAvg)
            .with_min_contributions(2)
            .with_byzantine_threshold(2.0);

        // Need enough good contributions so the outlier's z-score exceeds 2.0.
        // With k good + 1 evil, the evil z-score grows with sqrt(k).
        agg.add_contribution(make_contribution("good1", vec![1.0, 1.0], 1.0, 100));
        agg.add_contribution(make_contribution("good2", vec![1.1, 0.9], 1.0, 100));
        agg.add_contribution(make_contribution("good3", vec![0.9, 1.1], 1.0, 100));
        agg.add_contribution(make_contribution("good4", vec![1.0, 1.0], 1.0, 100));
        agg.add_contribution(make_contribution("good5", vec![1.0, 1.0], 1.0, 100));
        agg.add_contribution(make_contribution("good6", vec![1.0, 1.0], 1.0, 100));
        agg.add_contribution(make_contribution("evil", vec![100.0, 100.0], 1.0, 100)); // outlier

        let result = agg.aggregate().unwrap();
        assert!(result.byzantine_filtered);
        assert!(result.outliers_removed >= 1);
        // Result should be close to 1.0, not pulled toward 100
        assert!(result.lora_deltas[0] < 5.0);
    }

    #[test]
    fn insufficient_contributions_error() {
        let mut agg = FederatedAggregator::new("test".into(), AggregationStrategy::FedAvg)
            .with_min_contributions(3);

        agg.add_contribution(make_contribution("a", vec![1.0], 1.0, 100));

        let result = agg.aggregate();
        assert!(result.is_err());
    }

    #[test]
    fn weighted_average_strategy() {
        let mut agg = FederatedAggregator::new("test".into(), AggregationStrategy::WeightedAverage)
            .with_min_contributions(2);

        agg.add_contribution(make_contribution("a", vec![10.0], 0.9, 10));
        agg.add_contribution(make_contribution("b", vec![0.0], 0.1, 10));

        let result = agg.aggregate().unwrap();
        // (0.9*10 + 0.1*0) / 1.0 = 9.0
        assert!((result.lora_deltas[0] - 9.0).abs() < 1e-10);
    }

    #[test]
    fn round_increments() {
        let mut agg = FederatedAggregator::new("test".into(), AggregationStrategy::FedAvg)
            .with_min_contributions(2);

        agg.add_contribution(make_contribution("a", vec![1.0], 1.0, 100));
        agg.add_contribution(make_contribution("b", vec![2.0], 1.0, 100));
        let r1 = agg.aggregate().unwrap();
        assert_eq!(r1.round, 1);

        agg.add_contribution(make_contribution("a", vec![3.0], 1.0, 100));
        agg.add_contribution(make_contribution("b", vec![4.0], 1.0, 100));
        let r2 = agg.aggregate().unwrap();
        assert_eq!(r2.round, 2);
    }

    #[test]
    fn confidences_high_when_agreement() {
        let mut agg = FederatedAggregator::new("test".into(), AggregationStrategy::FedAvg)
            .with_min_contributions(2);

        agg.add_contribution(make_contribution("a", vec![1.0], 1.0, 100));
        agg.add_contribution(make_contribution("b", vec![1.0], 1.0, 100));

        let result = agg.aggregate().unwrap();
        // When all agree, variance = 0, confidence = 1/(1+0) = 1.0
        assert!((result.confidences[0] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn confidences_lower_when_disagreement() {
        let mut agg = FederatedAggregator::new("test".into(), AggregationStrategy::FedAvg)
            .with_min_contributions(2);

        agg.add_contribution(make_contribution("a", vec![0.0], 1.0, 100));
        agg.add_contribution(make_contribution("b", vec![10.0], 1.0, 100));

        let result = agg.aggregate().unwrap();
        // When disagreement, confidence < 1.0
        assert!(result.confidences[0] < 1.0);
    }
}
