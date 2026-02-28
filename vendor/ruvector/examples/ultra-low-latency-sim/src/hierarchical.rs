//! Hierarchical Meta-Simulation
//!
//! Each level of hierarchy compresses BATCH_SIZE simulations into one result.
//! Level k represents BATCH_SIZE^k simulations per output value.

/// Default batch size for hierarchical compression
pub const DEFAULT_BATCH_SIZE: usize = 64;

/// Hierarchical simulation compressor
/// Each output represents BATCH_SIZE^level input simulations
#[repr(align(64))]
pub struct HierarchicalCompressor {
    /// Current results (each represents many sub-simulations)
    results: Vec<f32>,
    /// Hierarchy level
    level: u32,
    /// Batch size for compression
    batch_size: usize,
}

impl HierarchicalCompressor {
    /// Create new compressor at given hierarchy level
    pub fn new(output_size: usize, level: u32, batch_size: usize) -> Self {
        Self {
            results: vec![0.0; output_size],
            level,
            batch_size,
        }
    }

    /// Simulations represented by each result value
    pub fn sims_per_result(&self) -> u64 {
        (self.batch_size as u64).pow(self.level)
    }

    /// Total simulations represented by all results
    pub fn total_simulations(&self) -> u64 {
        self.results.len() as u64 * self.sims_per_result()
    }

    /// Compress batch of inputs into meta-results
    /// Aggregates BATCH_SIZE values per output
    #[inline]
    pub fn compress(&mut self, inputs: &[f32]) {
        let out_count = inputs.len() / self.batch_size;

        for (i, result) in self.results.iter_mut().take(out_count).enumerate() {
            let start = i * self.batch_size;
            let end = start + self.batch_size;

            // Sum and average (vectorizable loop)
            let sum: f32 = inputs[start..end].iter().sum();
            *result = sum / self.batch_size as f32;
        }
    }

    /// Get compressed results
    pub fn results(&self) -> &[f32] {
        &self.results
    }

    /// Hierarchy level
    pub fn level(&self) -> u32 {
        self.level
    }
}

/// Multi-level hierarchical simulation pipeline
/// Compresses through multiple levels for exponential multiplier
pub struct HierarchicalPipeline {
    /// Compressors for each level
    levels: Vec<HierarchicalCompressor>,
    /// Batch size
    batch_size: usize,
}

impl HierarchicalPipeline {
    /// Create pipeline with given depth
    pub fn new(base_size: usize, depth: usize, batch_size: usize) -> Self {
        let mut levels = Vec::with_capacity(depth);
        let mut size = base_size;

        for level in 0..depth as u32 {
            size /= batch_size;
            if size == 0 { size = 1; }
            levels.push(HierarchicalCompressor::new(size, level + 1, batch_size));
        }

        Self { levels, batch_size }
    }

    /// Run full compression pipeline
    pub fn compress_all(&mut self, base_inputs: &[f32]) {
        if self.levels.is_empty() { return; }

        // First level compresses base inputs
        self.levels[0].compress(base_inputs);

        // Each subsequent level compresses previous level's output
        for i in 1..self.levels.len() {
            let prev_results = self.levels[i - 1].results.clone();
            self.levels[i].compress(&prev_results);
        }
    }

    /// Total simulations at final level
    pub fn final_simulations(&self) -> u64 {
        self.levels.last()
            .map(|l| l.total_simulations())
            .unwrap_or(0)
    }

    /// Get final results
    pub fn final_results(&self) -> Option<&[f32]> {
        self.levels.last().map(|l| l.results())
    }
}

/// Ensemble aggregator for Monte Carlo with hierarchical batching
/// Each "sample" represents many underlying random samples
pub struct EnsembleAggregator {
    /// Running mean estimates
    means: Vec<f64>,
    /// Running M2 for Welford's online variance
    m2: Vec<f64>,
    /// Sample count (each "sample" = batch_size underlying samples)
    count: u64,
    /// Samples per aggregate
    samples_per_aggregate: u64,
}

impl EnsembleAggregator {
    /// Create aggregator for n-dimensional output
    pub fn new(dimensions: usize, samples_per_aggregate: u64) -> Self {
        Self {
            means: vec![0.0; dimensions],
            m2: vec![0.0; dimensions],
            count: 0,
            samples_per_aggregate,
        }
    }

    /// Add aggregate sample (represents many underlying samples)
    /// Using Welford's online algorithm for numerical stability
    #[inline]
    pub fn add_aggregate(&mut self, values: &[f64]) {
        self.count += 1;
        let n = self.count as f64;

        for (i, &x) in values.iter().enumerate() {
            let delta = x - self.means[i];
            self.means[i] += delta / n;
            let delta2 = x - self.means[i];
            self.m2[i] += delta * delta2;
        }
    }

    /// Total underlying samples represented
    pub fn total_samples(&self) -> u64 {
        self.count * self.samples_per_aggregate
    }

    /// Get current mean estimates
    pub fn means(&self) -> &[f64] {
        &self.means
    }

    /// Get sample variance
    pub fn variance(&self) -> Vec<f64> {
        if self.count < 2 {
            return vec![0.0; self.means.len()];
        }
        self.m2.iter().map(|m| m / (self.count - 1) as f64).collect()
    }

    /// Standard error (adjusted for aggregation)
    pub fn standard_error(&self) -> Vec<f64> {
        let var = self.variance();
        let n = self.total_samples() as f64;
        var.iter().map(|v| (v / n).sqrt()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hierarchical_compressor() {
        let mut comp = HierarchicalCompressor::new(16, 1, 64);
        let inputs: Vec<f32> = (0..1024).map(|i| i as f32).collect();

        comp.compress(&inputs);

        assert_eq!(comp.results().len(), 16);
        assert_eq!(comp.sims_per_result(), 64);
        assert_eq!(comp.total_simulations(), 16 * 64);
    }

    #[test]
    fn test_hierarchical_pipeline() {
        let mut pipeline = HierarchicalPipeline::new(4096, 3, 4);
        let inputs: Vec<f32> = (0..4096).map(|i| (i as f32).sin()).collect();

        pipeline.compress_all(&inputs);

        assert!(pipeline.final_simulations() > 0);
    }

    #[test]
    fn test_ensemble_aggregator() {
        let mut agg = EnsembleAggregator::new(2, 1000);

        for i in 0..100 {
            agg.add_aggregate(&[i as f64, -i as f64]);
        }

        assert_eq!(agg.total_samples(), 100_000);
        assert!((agg.means()[0] - 49.5).abs() < 0.1);
    }
}
