//! Fast sampling techniques for sublinear algorithms
//!
//! Implements advanced sampling methods needed for true sublinear complexity,
//! including importance sampling, reservoir sampling, and sketching techniques.

use crate::types::Precision;
use crate::error::{SolverError, Result};
use alloc::{vec::Vec, string::String};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

/// Configuration for sampling algorithms
#[derive(Debug, Clone)]
pub struct SamplingConfig {
    /// Sampling probability
    pub sampling_prob: Precision,
    /// Reservoir size for reservoir sampling
    pub reservoir_size: usize,
    /// Sketch dimension for matrix sketching
    pub sketch_dimension: usize,
    /// Random seed
    pub seed: Option<u64>,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            sampling_prob: 0.01,
            reservoir_size: 1000,
            sketch_dimension: 64,
            seed: None,
        }
    }
}

/// Importance sampling engine
#[derive(Debug)]
pub struct ImportanceSampler {
    config: SamplingConfig,
    rng: StdRng,
}

impl ImportanceSampler {
    /// Create new importance sampler
    pub fn new(config: SamplingConfig) -> Self {
        let rng = match config.seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::from_entropy(),
        };

        Self { config, rng }
    }

    /// Sample matrix entries with importance weighting
    pub fn sample_matrix_entries(
        &mut self,
        entries: &[(usize, usize, Precision)],
    ) -> Result<Vec<(usize, usize, Precision)>> {
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // Compute importance weights (based on magnitude)
        let mut weights = Vec::with_capacity(entries.len());
        let mut total_weight = 0.0;

        for &(_, _, value) in entries {
            let weight = value.abs();
            weights.push(weight);
            total_weight += weight;
        }

        if total_weight == 0.0 {
            return Ok(Vec::new());
        }

        // Normalize weights to probabilities
        for weight in &mut weights {
            *weight /= total_weight;
        }

        // Sample entries based on importance
        let target_samples = (entries.len() as f64 * self.config.sampling_prob).ceil() as usize;
        let mut sampled_entries = Vec::new();

        for _ in 0..target_samples {
            let sample_index = self.weighted_sample(&weights)?;
            let (i, j, value) = entries[sample_index];

            // Reweight to maintain expectation
            let reweighted_value = value / weights[sample_index];
            sampled_entries.push((i, j, reweighted_value));
        }

        Ok(sampled_entries)
    }

    /// Sample a single index based on weights
    fn weighted_sample(&mut self, weights: &[Precision]) -> Result<usize> {
        let random_val = self.rng.gen::<f64>();
        let mut cumulative = 0.0;

        for (i, &weight) in weights.iter().enumerate() {
            cumulative += weight;
            if random_val <= cumulative {
                return Ok(i);
            }
        }

        // Fallback to last index
        Ok(weights.len() - 1)
    }

    /// Sample vector entries with importance weights
    pub fn sample_vector_entries(
        &mut self,
        vector: &[Precision],
    ) -> Result<Vec<(usize, Precision)>> {
        if vector.is_empty() {
            return Ok(Vec::new());
        }

        // Compute importance weights
        let total_magnitude: Precision = vector.iter().map(|x| x.abs()).sum();
        if total_magnitude == 0.0 {
            return Ok(Vec::new());
        }

        let target_samples = (vector.len() as f64 * self.config.sampling_prob).ceil() as usize;
        let mut sampled_entries = Vec::new();

        for i in 0..target_samples.min(vector.len()) {
            let importance_weight = vector[i].abs() / total_magnitude;

            if self.rng.gen::<f64>() < importance_weight / self.config.sampling_prob {
                let reweighted_value = vector[i] / importance_weight;
                sampled_entries.push((i, reweighted_value));
            }
        }

        Ok(sampled_entries)
    }
}

/// Reservoir sampling for streaming data
#[derive(Debug)]
pub struct ReservoirSampler {
    reservoir: Vec<(usize, usize, Precision)>,
    reservoir_size: usize,
    items_seen: usize,
    rng: StdRng,
}

impl ReservoirSampler {
    /// Create new reservoir sampler
    pub fn new(reservoir_size: usize, seed: Option<u64>) -> Self {
        let rng = match seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_entropy(),
        };

        Self {
            reservoir: Vec::with_capacity(reservoir_size),
            reservoir_size,
            items_seen: 0,
            rng,
        }
    }

    /// Add new item to reservoir (maintains uniform sample)
    pub fn add_item(&mut self, i: usize, j: usize, value: Precision) {
        self.items_seen += 1;

        if self.reservoir.len() < self.reservoir_size {
            // Fill reservoir first
            self.reservoir.push((i, j, value));
        } else {
            // Randomly replace existing item
            let replace_index = self.rng.gen_range(0..self.items_seen);
            if replace_index < self.reservoir_size {
                self.reservoir[replace_index] = (i, j, value);
            }
        }
    }

    /// Get current reservoir contents
    pub fn get_sample(&self) -> Vec<(usize, usize, Precision)> {
        self.reservoir.clone()
    }

    /// Get number of items processed
    pub fn items_seen(&self) -> usize {
        self.items_seen
    }

    /// Clear reservoir and reset counters
    pub fn reset(&mut self) {
        self.reservoir.clear();
        self.items_seen = 0;
    }
}

/// Matrix sketching for dimension reduction
#[derive(Debug)]
pub struct MatrixSketcher {
    sketch_dimension: usize,
    sketch_matrix: Vec<Vec<Precision>>,
    original_dimension: usize,
    rng: StdRng,
}

impl MatrixSketcher {
    /// Create new matrix sketcher
    pub fn new(
        original_dimension: usize,
        sketch_dimension: usize,
        seed: Option<u64>,
    ) -> Result<Self> {
        if sketch_dimension > original_dimension {
            return Err(SolverError::InvalidInput {
                message: "Sketch dimension must be <= original dimension".to_string(),
                parameter: Some("sketch_dimension".to_string()),
            });
        }

        let mut rng = match seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_entropy(),
        };

        // Generate random sketch matrix
        let mut sketch_matrix = vec![vec![0.0; original_dimension]; sketch_dimension];
        let scale = (1.0 / sketch_dimension as f64).sqrt();

        for i in 0..sketch_dimension {
            for j in 0..original_dimension {
                // Random sign matrix (Rademacher distribution)
                sketch_matrix[i][j] = if rng.gen::<bool>() { scale } else { -scale };
            }
        }

        Ok(Self {
            sketch_dimension,
            sketch_matrix,
            original_dimension,
            rng,
        })
    }

    /// Sketch a vector (reduce dimension)
    pub fn sketch_vector(&self, vector: &[Precision]) -> Result<Vec<Precision>> {
        if vector.len() != self.original_dimension {
            return Err(SolverError::DimensionMismatch {
                expected: self.original_dimension,
                actual: vector.len(),
                operation: "sketch_vector".to_string(),
            });
        }

        let mut sketched = vec![0.0; self.sketch_dimension];

        for i in 0..self.sketch_dimension {
            for j in 0..self.original_dimension {
                sketched[i] += self.sketch_matrix[i][j] * vector[j];
            }
        }

        Ok(sketched)
    }

    /// Sketch a matrix (reduce both dimensions)
    pub fn sketch_matrix(
        &self,
        matrix_rows: &[Vec<Precision>],
    ) -> Result<Vec<Vec<Precision>>> {
        if matrix_rows.is_empty() {
            return Ok(Vec::new());
        }

        let mut sketched_rows = Vec::new();

        for row in matrix_rows {
            sketched_rows.push(self.sketch_vector(row)?);
        }

        Ok(sketched_rows)
    }

    /// Get compression ratio
    pub fn compression_ratio(&self) -> Precision {
        self.sketch_dimension as Precision / self.original_dimension as Precision
    }

    /// Reconstruct approximate vector (simplified)
    pub fn reconstruct_vector(&self, sketched: &[Precision]) -> Result<Vec<Precision>> {
        if sketched.len() != self.sketch_dimension {
            return Err(SolverError::DimensionMismatch {
                expected: self.sketch_dimension,
                actual: sketched.len(),
                operation: "reconstruct_vector".to_string(),
            });
        }

        // Simple reconstruction using transpose
        let mut reconstructed = vec![0.0; self.original_dimension];

        for j in 0..self.original_dimension {
            for i in 0..self.sketch_dimension {
                reconstructed[j] += self.sketch_matrix[i][j] * sketched[i];
            }
        }

        Ok(reconstructed)
    }
}

/// Adaptive sampling that adjusts parameters based on observed error
#[derive(Debug)]
pub struct AdaptiveSampler {
    importance_sampler: ImportanceSampler,
    reservoir_sampler: ReservoirSampler,
    matrix_sketcher: Option<MatrixSketcher>,
    adaptive_threshold: Precision,
    current_error: Precision,
}

impl AdaptiveSampler {
    /// Create new adaptive sampler
    pub fn new(
        config: SamplingConfig,
        original_dimension: Option<usize>,
    ) -> Result<Self> {
        let importance_sampler = ImportanceSampler::new(config.clone());
        let reservoir_sampler = ReservoirSampler::new(config.reservoir_size, config.seed);

        let matrix_sketcher = if let Some(dim) = original_dimension {
            Some(MatrixSketcher::new(dim, config.sketch_dimension, config.seed)?)
        } else {
            None
        };

        Ok(Self {
            importance_sampler,
            reservoir_sampler,
            matrix_sketcher,
            adaptive_threshold: 0.1,
            current_error: 0.0,
        })
    }

    /// Adapt sampling parameters based on error
    pub fn adapt_parameters(&mut self, observed_error: Precision) {
        self.current_error = observed_error;

        if observed_error > self.adaptive_threshold * 2.0 {
            // Increase sampling probability
            self.importance_sampler.config.sampling_prob =
                (self.importance_sampler.config.sampling_prob * 1.5).min(1.0);
        } else if observed_error < self.adaptive_threshold * 0.5 {
            // Decrease sampling probability
            self.importance_sampler.config.sampling_prob =
                (self.importance_sampler.config.sampling_prob * 0.8).max(0.001);
        }
    }

    /// Get current sampling statistics
    pub fn get_statistics(&self) -> SamplingStatistics {
        SamplingStatistics {
            current_sampling_prob: self.importance_sampler.config.sampling_prob,
            reservoir_items_seen: self.reservoir_sampler.items_seen(),
            current_error: self.current_error,
            compression_ratio: self.matrix_sketcher
                .as_ref()
                .map(|s| s.compression_ratio())
                .unwrap_or(1.0),
        }
    }
}

/// Sampling performance statistics
#[derive(Debug, Clone)]
pub struct SamplingStatistics {
    pub current_sampling_prob: Precision,
    pub reservoir_items_seen: usize,
    pub current_error: Precision,
    pub compression_ratio: Precision,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_importance_sampler() {
        let config = SamplingConfig {
            sampling_prob: 0.5,
            ..Default::default()
        };
        let mut sampler = ImportanceSampler::new(config);

        let entries = vec![
            (0, 0, 1.0),
            (0, 1, 10.0),  // High importance
            (1, 0, 0.1),
            (1, 1, 2.0),
        ];

        let sampled = sampler.sample_matrix_entries(&entries).unwrap();
        assert!(!sampled.is_empty());
    }

    #[test]
    fn test_reservoir_sampler() {
        let mut sampler = ReservoirSampler::new(3, Some(42));

        // Add more items than reservoir size
        for i in 0..10 {
            sampler.add_item(i, i, i as f64);
        }

        let sample = sampler.get_sample();
        assert_eq!(sample.len(), 3);
        assert_eq!(sampler.items_seen(), 10);
    }

    #[test]
    fn test_matrix_sketcher() {
        let sketcher = MatrixSketcher::new(10, 5, Some(123)).unwrap();
        let vector = vec![1.0; 10];

        let sketched = sketcher.sketch_vector(&vector).unwrap();
        assert_eq!(sketched.len(), 5);

        let reconstructed = sketcher.reconstruct_vector(&sketched).unwrap();
        assert_eq!(reconstructed.len(), 10);
    }

    #[test]
    fn test_adaptive_sampler() {
        let config = SamplingConfig::default();
        let mut adaptive = AdaptiveSampler::new(config, Some(20)).unwrap();

        let initial_prob = adaptive.importance_sampler.config.sampling_prob;

        // High error should increase sampling
        adaptive.adapt_parameters(1.0);
        assert!(adaptive.importance_sampler.config.sampling_prob >= initial_prob);

        // Low error should decrease sampling
        adaptive.adapt_parameters(0.001);
        assert!(adaptive.importance_sampler.config.sampling_prob <= initial_prob);
    }
}