use std::collections::HashMap;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::{Normal, Uniform, Distribution};
use crate::core::{SparseMatrix, Vector, Result, SublinearError};

/// Adaptive sampling strategies for Monte Carlo methods
#[derive(Debug, Clone, PartialEq)]
pub enum SamplingStrategy {
    Uniform,
    ImportanceSampling,
    StratifiedSampling,
    AdaptiveSampling,
    QuasiMonteCarlo,
}

/// Configuration for adaptive sampling
#[derive(Debug, Clone)]
pub struct SamplingConfig {
    pub strategy: SamplingStrategy,
    pub sample_size: usize,
    pub adaptation_rate: f64,
    pub variance_threshold: f64,
    pub seed: Option<u64>,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            strategy: SamplingStrategy::AdaptiveSampling,
            sample_size: 1000,
            adaptation_rate: 0.1,
            variance_threshold: 1e-4,
            seed: None,
        }
    }
}

/// Adaptive sampling engine for variance reduction
pub struct AdaptiveSampler {
    config: SamplingConfig,
    rng: ChaCha8Rng,
    importance_weights: HashMap<usize, f64>,
    strata_boundaries: Vec<f64>,
    sample_history: Vec<f64>,
    variance_history: Vec<f64>,
}

impl AdaptiveSampler {
    pub fn new(config: SamplingConfig) -> Self {
        let rng = match config.seed {
            Some(seed) => ChaCha8Rng::seed_from_u64(seed),
            None => ChaCha8Rng::from_entropy(),
        };

        Self {
            config,
            rng,
            importance_weights: HashMap::new(),
            strata_boundaries: Vec::new(),
            sample_history: Vec::new(),
            variance_history: Vec::new(),
        }
    }

    /// Generate adaptive samples for Monte Carlo estimation
    pub fn generate_samples(&mut self, domain_size: usize, target_function: &dyn Fn(usize) -> f64) -> Result<Vec<(usize, f64)>> {
        match self.config.strategy {
            SamplingStrategy::Uniform => self.uniform_sampling(domain_size),
            SamplingStrategy::ImportanceSampling => self.importance_sampling(domain_size, target_function),
            SamplingStrategy::StratifiedSampling => self.stratified_sampling(domain_size, target_function),
            SamplingStrategy::AdaptiveSampling => self.adaptive_sampling(domain_size, target_function),
            SamplingStrategy::QuasiMonteCarlo => self.quasi_monte_carlo_sampling(domain_size),
        }
    }

    /// Uniform random sampling
    fn uniform_sampling(&mut self, domain_size: usize) -> Result<Vec<(usize, f64)>> {
        let mut samples = Vec::with_capacity(self.config.sample_size);
        let uniform_weight = 1.0 / domain_size as f64;

        for _ in 0..self.config.sample_size {
            let index = self.rng.gen_range(0..domain_size);
            samples.push((index, uniform_weight));
        }

        Ok(samples)
    }

    /// Importance sampling based on learned weights
    fn importance_sampling(&mut self, domain_size: usize, target_function: &dyn Fn(usize) -> f64) -> Result<Vec<(usize, f64)>> {
        // Initialize or update importance weights
        if self.importance_weights.is_empty() {
            self.initialize_importance_weights(domain_size, target_function);
        } else {
            self.update_importance_weights(domain_size, target_function);
        }

        let mut samples = Vec::with_capacity(self.config.sample_size);
        let total_weight: f64 = self.importance_weights.values().sum();

        // Create cumulative distribution
        let mut cumulative_weights = Vec::new();
        let mut cumulative = 0.0;
        for i in 0..domain_size {
            cumulative += self.importance_weights.get(&i).unwrap_or(&1.0) / total_weight;
            cumulative_weights.push((i, cumulative));
        }

        // Sample according to importance weights
        for _ in 0..self.config.sample_size {
            let u = self.rng.gen::<f64>();
            for (index, cum_weight) in &cumulative_weights {
                if u <= *cum_weight {
                    let importance_weight = self.importance_weights.get(index).unwrap_or(&1.0);
                    let sample_weight = 1.0 / (importance_weight / total_weight);
                    samples.push((*index, sample_weight));
                    break;
                }
            }
        }

        Ok(samples)
    }

    /// Stratified sampling for variance reduction
    fn stratified_sampling(&mut self, domain_size: usize, target_function: &dyn Fn(usize) -> f64) -> Result<Vec<(usize, f64)>> {
        let num_strata = (self.config.sample_size as f64).sqrt() as usize;
        let samples_per_stratum = self.config.sample_size / num_strata;

        // Update strata boundaries if needed
        if self.strata_boundaries.is_empty() || self.strata_boundaries.len() != num_strata + 1 {
            self.update_strata_boundaries(domain_size, num_strata, target_function);
        }

        let mut samples = Vec::with_capacity(self.config.sample_size);

        for stratum in 0..num_strata {
            let start_idx = (stratum as f64 / num_strata as f64 * domain_size as f64) as usize;
            let end_idx = ((stratum + 1) as f64 / num_strata as f64 * domain_size as f64) as usize;
            let stratum_size = end_idx - start_idx;

            if stratum_size == 0 {
                continue;
            }

            // Sample uniformly within stratum
            for _ in 0..samples_per_stratum {
                let index = start_idx + self.rng.gen_range(0..stratum_size);
                let weight = domain_size as f64 / self.config.sample_size as f64;
                samples.push((index, weight));
            }
        }

        Ok(samples)
    }

    /// Adaptive sampling that learns from variance patterns
    fn adaptive_sampling(&mut self, domain_size: usize, target_function: &dyn Fn(usize) -> f64) -> Result<Vec<(usize, f64)>> {
        // Start with importance sampling and adapt based on variance
        let mut samples = self.importance_sampling(domain_size, target_function)?;

        // Compute current variance estimate
        let values: Vec<f64> = samples.iter().map(|(idx, _)| target_function(*idx)).collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;

        self.variance_history.push(variance);

        // Adapt sampling strategy if variance is too high
        if variance > self.config.variance_threshold && self.variance_history.len() > 10 {
            // Switch to stratified sampling for high-variance regions
            let stratified_samples = self.stratified_sampling(domain_size, target_function)?;

            // Combine samples based on adaptive weighting
            let alpha = self.compute_adaptive_weight();
            samples = self.combine_samples(&samples, &stratified_samples, alpha);
        }

        Ok(samples)
    }

    /// Quasi-Monte Carlo sampling using low-discrepancy sequences
    fn quasi_monte_carlo_sampling(&mut self, domain_size: usize) -> Result<Vec<(usize, f64)>> {
        let mut samples = Vec::with_capacity(self.config.sample_size);
        let uniform_weight = 1.0 / domain_size as f64;

        // Use Halton sequence for low-discrepancy sampling
        for i in 0..self.config.sample_size {
            let halton_value = self.halton_sequence(i + 1, 2); // Base 2 Halton sequence
            let index = (halton_value * domain_size as f64) as usize;
            let clamped_index = index.min(domain_size - 1);
            samples.push((clamped_index, uniform_weight));
        }

        Ok(samples)
    }

    /// Initialize importance weights based on function evaluation
    fn initialize_importance_weights(&mut self, domain_size: usize, target_function: &dyn Fn(usize) -> f64) {
        let sample_size = (domain_size as f64).sqrt() as usize;
        let mut function_values = Vec::new();

        // Sample function at regular intervals
        for i in 0..sample_size {
            let index = (i * domain_size) / sample_size;
            let value = target_function(index).abs() + 1e-10; // Avoid zero weights
            function_values.push((index, value));
        }

        // Interpolate weights for all indices
        for i in 0..domain_size {
            let weight = self.interpolate_weight(i, &function_values, domain_size);
            self.importance_weights.insert(i, weight);
        }
    }

    /// Update importance weights based on recent samples
    fn update_importance_weights(&mut self, domain_size: usize, target_function: &dyn Fn(usize) -> f64) {
        let learning_rate = self.config.adaptation_rate;

        // Sample a subset for weight updates
        let update_size = domain_size / 100; // Update 1% of weights
        for _ in 0..update_size {
            let index = self.rng.gen_range(0..domain_size);
            let function_value = target_function(index).abs() + 1e-10;

            let current_weight = self.importance_weights.get(&index).unwrap_or(&1.0);
            let new_weight = (1.0 - learning_rate) * current_weight + learning_rate * function_value;
            self.importance_weights.insert(index, new_weight);
        }
    }

    /// Update strata boundaries based on function characteristics
    fn update_strata_boundaries(&mut self, domain_size: usize, num_strata: usize, target_function: &dyn Fn(usize) -> f64) {
        let mut function_samples = Vec::new();
        let sample_size = domain_size / 10; // Sample 10% of domain

        for _ in 0..sample_size {
            let index = self.rng.gen_range(0..domain_size);
            let value = target_function(index);
            function_samples.push(value);
        }

        // Sort and create quantile-based boundaries
        function_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        self.strata_boundaries.clear();

        for i in 0..=num_strata {
            let quantile_idx = (i * sample_size) / num_strata;
            let boundary = if quantile_idx < function_samples.len() {
                function_samples[quantile_idx]
            } else {
                function_samples.last().copied().unwrap_or(0.0)
            };
            self.strata_boundaries.push(boundary);
        }
    }

    /// Interpolate weight for given index
    fn interpolate_weight(&self, index: usize, function_values: &[(usize, f64)], domain_size: usize) -> f64 {
        if function_values.is_empty() {
            return 1.0;
        }

        // Find nearest samples for interpolation
        let mut best_value = function_values[0].1;
        let mut min_distance = (index as i32 - function_values[0].0 as i32).abs() as f64;

        for &(sample_idx, value) in function_values {
            let distance = (index as i32 - sample_idx as i32).abs() as f64;
            if distance < min_distance {
                min_distance = distance;
                best_value = value;
            }
        }

        best_value
    }

    /// Compute adaptive weight for combining sampling strategies
    fn compute_adaptive_weight(&self) -> f64 {
        if self.variance_history.len() < 2 {
            return 0.5;
        }

        let recent_variance = self.variance_history[self.variance_history.len() - 1];
        let previous_variance = self.variance_history[self.variance_history.len() - 2];

        // Increase stratified sampling weight if variance is increasing
        if recent_variance > previous_variance {
            0.3 // More weight to stratified sampling
        } else {
            0.7 // More weight to importance sampling
        }
    }

    /// Combine two sets of samples with given weight
    fn combine_samples(&self, samples1: &[(usize, f64)], samples2: &[(usize, f64)], alpha: f64) -> Vec<(usize, f64)> {
        let size1 = (alpha * self.config.sample_size as f64) as usize;
        let size2 = self.config.sample_size - size1;

        let mut combined = Vec::new();
        combined.extend_from_slice(&samples1[..size1.min(samples1.len())]);
        combined.extend_from_slice(&samples2[..size2.min(samples2.len())]);
        combined
    }

    /// Generate Halton sequence value
    fn halton_sequence(&self, index: usize, base: usize) -> f64 {
        let mut result = 0.0;
        let mut f = 1.0 / base as f64;
        let mut i = index;

        while i > 0 {
            result += f * (i % base) as f64;
            i /= base;
            f /= base as f64;
        }

        result
    }

    /// Get sampling statistics
    pub fn get_statistics(&self) -> SamplingStatistics {
        SamplingStatistics {
            total_samples: self.sample_history.len(),
            mean_variance: self.variance_history.iter().sum::<f64>() / self.variance_history.len().max(1) as f64,
            convergence_rate: self.compute_convergence_rate(),
            efficiency_score: self.compute_efficiency_score(),
        }
    }

    fn compute_convergence_rate(&self) -> f64 {
        if self.variance_history.len() < 2 {
            return 0.0;
        }

        let initial_variance = self.variance_history[0];
        let final_variance = self.variance_history.last().unwrap();

        if initial_variance > 0.0 {
            (initial_variance - final_variance) / initial_variance
        } else {
            0.0
        }
    }

    fn compute_efficiency_score(&self) -> f64 {
        // Higher score for lower variance with fewer samples
        let mean_variance = self.variance_history.iter().sum::<f64>() / self.variance_history.len().max(1) as f64;
        let sample_efficiency = 1.0 / (1.0 + self.sample_history.len() as f64 / 1000.0);
        let variance_efficiency = 1.0 / (1.0 + mean_variance);

        (sample_efficiency + variance_efficiency) / 2.0
    }
}

/// Statistics for sampling performance
#[derive(Debug, Clone)]
pub struct SamplingStatistics {
    pub total_samples: usize,
    pub mean_variance: f64,
    pub convergence_rate: f64,
    pub efficiency_score: f64,
}

/// Multi-level Monte Carlo sampler for hierarchical problems
pub struct MultiLevelSampler {
    levels: Vec<AdaptiveSampler>,
    level_costs: Vec<f64>,
    level_variances: Vec<f64>,
}

impl MultiLevelSampler {
    pub fn new(num_levels: usize, base_config: SamplingConfig) -> Self {
        let mut levels = Vec::new();

        for level in 0..num_levels {
            let mut config = base_config.clone();
            config.sample_size = base_config.sample_size / (2_usize.pow(level as u32));
            config.seed = base_config.seed.map(|s| s.wrapping_add(level as u64));
            levels.push(AdaptiveSampler::new(config));
        }

        Self {
            levels,
            level_costs: vec![1.0; num_levels],
            level_variances: vec![1.0; num_levels],
        }
    }

    /// Generate multi-level samples with optimal allocation
    pub fn generate_multilevel_samples(&mut self, domain_sizes: &[usize], target_functions: &[&dyn Fn(usize) -> f64]) -> Result<Vec<Vec<(usize, f64)>>> {
        if domain_sizes.len() != self.levels.len() || target_functions.len() != self.levels.len() {
            return Err(SublinearError::InvalidDimensions);
        }

        let mut all_samples = Vec::new();

        // Generate samples for each level
        for (level, (domain_size, target_function)) in domain_sizes.iter().zip(target_functions.iter()).enumerate() {
            let samples = self.levels[level].generate_samples(*domain_size, *target_function)?;

            // Update level statistics
            let values: Vec<f64> = samples.iter().map(|(idx, _)| target_function(*idx)).collect();
            let variance = self.compute_level_variance(&values);
            self.level_variances[level] = variance;

            all_samples.push(samples);
        }

        // Optimize sample allocation for next iteration
        self.optimize_sample_allocation();

        Ok(all_samples)
    }

    fn compute_level_variance(&self, values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 1.0;
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64
    }

    fn optimize_sample_allocation(&mut self) {
        // Optimal allocation based on variance and cost
        let total_budget = self.levels.iter().map(|l| l.config.sample_size).sum::<usize>() as f64;

        for (level, sampler) in self.levels.iter_mut().enumerate() {
            let variance = self.level_variances[level];
            let cost = self.level_costs[level];

            // Optimal allocation: proportional to sqrt(variance/cost)
            let allocation_factor = (variance / cost).sqrt();
            let new_sample_size = (allocation_factor * total_budget / self.levels.len() as f64) as usize;

            sampler.config.sample_size = new_sample_size.max(10); // Minimum samples
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_sampler_creation() {
        let config = SamplingConfig::default();
        let sampler = AdaptiveSampler::new(config);
        assert!(sampler.importance_weights.is_empty());
    }

    #[test]
    fn test_uniform_sampling() {
        let mut config = SamplingConfig::default();
        config.sample_size = 100;
        config.seed = Some(42);

        let mut sampler = AdaptiveSampler::new(config);
        let samples = sampler.uniform_sampling(1000).unwrap();

        assert_eq!(samples.len(), 100);
        for (index, weight) in samples {
            assert!(index < 1000);
            assert!((weight - 0.001).abs() < 1e-10); // 1/1000
        }
    }

    #[test]
    fn test_importance_sampling() {
        let mut config = SamplingConfig::default();
        config.sample_size = 50;
        config.seed = Some(123);

        let mut sampler = AdaptiveSampler::new(config);
        let target_fn = |x: usize| (x as f64 / 10.0).sin().abs() + 1.0;

        let samples = sampler.importance_sampling(100, &target_fn).unwrap();
        assert_eq!(samples.len(), 50);

        // Weights should be reasonable
        for (_, weight) in samples {
            assert!(weight > 0.0);
            assert!(weight < 100.0); // Reasonable upper bound
        }
    }

    #[test]
    fn test_halton_sequence() {
        let config = SamplingConfig::default();
        let sampler = AdaptiveSampler::new(config);

        // Test first few Halton sequence values
        assert!((sampler.halton_sequence(1, 2) - 0.5).abs() < 1e-10);
        assert!((sampler.halton_sequence(2, 2) - 0.25).abs() < 1e-10);
        assert!((sampler.halton_sequence(3, 2) - 0.75).abs() < 1e-10);
    }

    #[test]
    fn test_multilevel_sampler() {
        let base_config = SamplingConfig {
            sample_size: 100,
            seed: Some(42),
            ..Default::default()
        };

        let mut ml_sampler = MultiLevelSampler::new(3, base_config);

        let domain_sizes = vec![100, 50, 25];
        let fn1 = |x: usize| x as f64;
        let fn2 = |x: usize| (x as f64).sqrt();
        let fn3 = |x: usize| (x as f64).log2();
        let target_functions: Vec<&dyn Fn(usize) -> f64> = vec![&fn1, &fn2, &fn3];

        let samples = ml_sampler.generate_multilevel_samples(&domain_sizes, &target_functions).unwrap();

        assert_eq!(samples.len(), 3);
        assert!(!samples[0].is_empty());
        assert!(!samples[1].is_empty());
        assert!(!samples[2].is_empty());
    }
}