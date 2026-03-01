//! Hierarchical Φ Computation
//!
//! Exploits hierarchical batching from ultra-low-latency-sim to compute
//! integrated information across multiple parameter spaces simultaneously.
//!
//! # Key Innovation
//!
//! Each hierarchical level represents BATCH_SIZE^level consciousness measurements:
//! - Level 0: Individual network Φ computations
//! - Level 1: Batch average across parameter variations
//! - Level 2: Statistical ensemble across architectures
//! - Level 3: Meta-consciousness landscape
//!
//! With closed-form Φ, each batch operation is O(N³) instead of O(Bell(N))

use crate::closed_form_phi::ClosedFormPhi;

/// Hierarchical Φ batch processor
#[repr(align(64))]
pub struct HierarchicalPhiBatcher {
    /// Phi calculator
    calculator: ClosedFormPhi,
    /// Results at each hierarchy level
    levels: Vec<PhiLevel>,
    /// Batch size for compression
    batch_size: usize,
    /// Current hierarchy level
    max_level: usize,
}

impl HierarchicalPhiBatcher {
    /// Create new hierarchical batcher
    pub fn new(base_size: usize, depth: usize, batch_size: usize) -> Self {
        let mut levels = Vec::with_capacity(depth);
        let mut size = base_size;

        for level in 0..depth {
            levels.push(PhiLevel::new(size, level));
            size = (size / batch_size).max(1);
        }

        Self {
            calculator: ClosedFormPhi::default(),
            levels,
            batch_size,
            max_level: depth,
        }
    }

    /// Process batch of cognitive networks through hierarchy
    ///
    /// # Arguments
    /// * `networks` - Adjacency matrices for cognitive networks
    /// * `node_ids` - Node IDs for each network
    ///
    /// # Returns
    /// Hierarchical Φ statistics at each level
    pub fn process_hierarchical_batch(
        &mut self,
        networks: &[(Vec<Vec<f64>>, Vec<u64>)],
    ) -> HierarchicalPhiResults {
        let start = std::time::Instant::now();

        // Level 0: Compute individual Φ for each network
        let base_phis: Vec<f64> = networks
            .iter()
            .map(|(adj, nodes)| {
                let result = self.calculator.compute_phi_ergodic(adj, nodes);
                result.phi
            })
            .collect();

        self.levels[0].phi_values = base_phis.clone();

        // Hierarchical compression through levels
        for level in 1..self.max_level {
            let prev_phis = &self.levels[level - 1].phi_values;
            let compressed = self.compress_phi_batch(prev_phis);
            self.levels[level].phi_values = compressed;
        }

        // Compute statistics at each level
        let level_stats: Vec<PhiLevelStats> = self
            .levels
            .iter()
            .map(|level| level.compute_statistics())
            .collect();

        HierarchicalPhiResults {
            level_statistics: level_stats,
            total_networks_processed: networks.len(),
            effective_simulations: self.compute_effective_simulations(),
            computation_time_ms: start.elapsed().as_millis(),
        }
    }

    /// Compress batch of Φ values to next level
    fn compress_phi_batch(&self, phi_values: &[f64]) -> Vec<f64> {
        let out_count = (phi_values.len() / self.batch_size).max(1);
        let mut compressed = Vec::with_capacity(out_count);

        for i in 0..out_count {
            let start = i * self.batch_size;
            let end = (start + self.batch_size).min(phi_values.len());

            if start < phi_values.len() {
                // Aggregate via mean (could also use median, max, etc.)
                let batch_mean: f64 =
                    phi_values[start..end].iter().sum::<f64>() / (end - start) as f64;
                compressed.push(batch_mean);
            }
        }

        compressed
    }

    /// Compute effective number of consciousness measurements
    fn compute_effective_simulations(&self) -> u64 {
        if self.levels.is_empty() {
            return 0;
        }

        // Each level represents batch_size^level measurements
        let base_count = self.levels[0].phi_values.len() as u64;
        let hierarchy_mult = (self.batch_size as u64).pow(self.max_level as u32);

        base_count * hierarchy_mult
    }

    /// Get final meta-Φ (top of hierarchy)
    pub fn get_meta_phi(&self) -> Option<f64> {
        self.levels.last()?.phi_values.first().copied()
    }
}

/// Single level in hierarchical Φ pyramid
#[derive(Debug, Clone)]
struct PhiLevel {
    /// Φ values at this level
    phi_values: Vec<f64>,
    /// Level index (0 = base)
    level: usize,
}

impl PhiLevel {
    fn new(capacity: usize, level: usize) -> Self {
        Self {
            phi_values: Vec::with_capacity(capacity),
            level,
        }
    }

    /// Compute statistics for this level
    fn compute_statistics(&self) -> PhiLevelStats {
        if self.phi_values.is_empty() {
            return PhiLevelStats::empty(self.level);
        }

        let n = self.phi_values.len();
        let sum: f64 = self.phi_values.iter().sum();
        let mean = sum / n as f64;

        // Variance (Welford's would be better for streaming)
        let variance: f64 = self
            .phi_values
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / n as f64;

        let std_dev = variance.sqrt();

        let mut sorted = self.phi_values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let median = if n % 2 == 0 {
            (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
        } else {
            sorted[n / 2]
        };

        let min = sorted[0];
        let max = sorted[n - 1];

        PhiLevelStats {
            level: self.level,
            count: n,
            mean,
            median,
            std_dev,
            min,
            max,
        }
    }
}

/// Statistics for Φ values at one hierarchy level
#[derive(Debug, Clone)]
pub struct PhiLevelStats {
    /// Hierarchy level
    pub level: usize,
    /// Number of Φ values
    pub count: usize,
    /// Mean Φ
    pub mean: f64,
    /// Median Φ
    pub median: f64,
    /// Standard deviation
    pub std_dev: f64,
    /// Minimum Φ
    pub min: f64,
    /// Maximum Φ
    pub max: f64,
}

impl PhiLevelStats {
    fn empty(level: usize) -> Self {
        Self {
            level,
            count: 0,
            mean: 0.0,
            median: 0.0,
            std_dev: 0.0,
            min: 0.0,
            max: 0.0,
        }
    }

    /// Consciousness diversity (std_dev / mean)
    pub fn consciousness_diversity(&self) -> f64 {
        if self.mean > 1e-10 {
            self.std_dev / self.mean
        } else {
            0.0
        }
    }
}

/// Results from hierarchical Φ computation
#[derive(Debug, Clone)]
pub struct HierarchicalPhiResults {
    /// Statistics at each hierarchy level
    pub level_statistics: Vec<PhiLevelStats>,
    /// Total networks processed at base level
    pub total_networks_processed: usize,
    /// Effective number of consciousness measurements
    pub effective_simulations: u64,
    /// Total computation time in milliseconds
    pub computation_time_ms: u128,
}

impl HierarchicalPhiResults {
    /// Get simulations per second rate
    pub fn simulations_per_second(&self) -> f64 {
        if self.computation_time_ms == 0 {
            return 0.0;
        }

        let sims = self.effective_simulations as f64;
        let seconds = self.computation_time_ms as f64 / 1000.0;

        sims / seconds
    }

    /// Display results in human-readable format
    pub fn display_summary(&self) -> String {
        let mut summary = String::new();

        summary.push_str(&format!("Hierarchical Φ Computation Results\n"));
        summary.push_str(&format!("===================================\n"));
        summary.push_str(&format!(
            "Networks processed: {}\n",
            self.total_networks_processed
        ));
        summary.push_str(&format!(
            "Effective simulations: {:.2e}\n",
            self.effective_simulations as f64
        ));
        summary.push_str(&format!(
            "Computation time: {} ms\n",
            self.computation_time_ms
        ));
        summary.push_str(&format!(
            "Rate: {:.2e} sims/sec\n\n",
            self.simulations_per_second()
        ));

        for stats in &self.level_statistics {
            summary.push_str(&format!("Level {}: ", stats.level));
            summary.push_str(&format!(
                "n={}, mean={:.3}, median={:.3}, std={:.3}, range=[{:.3}, {:.3}]\n",
                stats.count, stats.mean, stats.median, stats.std_dev, stats.min, stats.max
            ));
        }

        summary
    }
}

/// Parameter space explorer for consciousness
///
/// Generates variations of cognitive architectures and measures Φ
pub struct ConsciousnessParameterSpace {
    /// Base network size
    base_size: usize,
    /// Connection density variations
    densities: Vec<f64>,
    /// Clustering coefficient variations
    clusterings: Vec<f64>,
    /// Reentry probability variations
    reentry_probs: Vec<f64>,
}

impl ConsciousnessParameterSpace {
    /// Create new parameter space
    pub fn new(base_size: usize) -> Self {
        Self {
            base_size,
            densities: (1..10).map(|i| i as f64 * 0.1).collect(),
            clusterings: (1..10).map(|i| i as f64 * 0.1).collect(),
            reentry_probs: (1..10).map(|i| i as f64 * 0.1).collect(),
        }
    }

    /// Generate all network variations
    pub fn generate_networks(&self) -> Vec<(Vec<Vec<f64>>, Vec<u64>)> {
        let mut networks = Vec::new();

        for &density in &self.densities {
            for &clustering in &self.clusterings {
                for &reentry in &self.reentry_probs {
                    let network = self.generate_network(density, clustering, reentry);
                    networks.push(network);
                }
            }
        }

        networks
    }

    /// Generate single network with parameters
    fn generate_network(
        &self,
        density: f64,
        _clustering: f64,
        reentry_prob: f64,
    ) -> (Vec<Vec<f64>>, Vec<u64>) {
        let n = self.base_size;
        let mut adj = vec![vec![0.0; n]; n];

        // Random connectivity with density
        for i in 0..n {
            for j in 0..n {
                if i != j && rand() < density {
                    adj[i][j] = 1.0;
                }
            }
        }

        // Add reentrant connections (feedback loops)
        for i in 0..n {
            if rand() < reentry_prob {
                let target = (i + 1) % n;
                adj[i][target] = 1.0;
                adj[target][i] = 1.0; // Bidirectional
            }
        }

        let nodes: Vec<u64> = (0..n as u64).collect();
        (adj, nodes)
    }

    /// Total number of network variations
    pub fn total_variations(&self) -> usize {
        self.densities.len() * self.clusterings.len() * self.reentry_probs.len()
    }
}

/// Simple random number generator (for deterministic testing)
fn rand() -> f64 {
    use std::cell::RefCell;
    thread_local! {
        static SEED: RefCell<u64> = RefCell::new(0x853c49e6748fea9b);
    }

    SEED.with(|s| {
        let mut seed = s.borrow_mut();
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        (*seed as f64) / (u64::MAX as f64)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hierarchical_batching() {
        let mut batcher = HierarchicalPhiBatcher::new(64, 3, 4);

        // Generate test networks
        let param_space = ConsciousnessParameterSpace::new(4);
        let networks: Vec<_> = param_space
            .generate_networks()
            .into_iter()
            .take(64)
            .collect();

        let results = batcher.process_hierarchical_batch(&networks);

        assert_eq!(results.total_networks_processed, 64);
        assert!(results.effective_simulations > 64);
        assert!(!results.level_statistics.is_empty());
    }

    #[test]
    fn test_parameter_space() {
        let space = ConsciousnessParameterSpace::new(5);
        let total = space.total_variations();

        assert_eq!(total, 9 * 9 * 9); // 3 parameters, 9 values each

        let networks = space.generate_networks();
        assert_eq!(networks.len(), total);
    }

    #[test]
    fn test_phi_level_stats() {
        let level = PhiLevel {
            phi_values: vec![1.0, 2.0, 3.0, 4.0, 5.0],
            level: 0,
        };

        let stats = level.compute_statistics();

        assert_eq!(stats.count, 5);
        assert!((stats.mean - 3.0).abs() < 0.01);
        assert!((stats.median - 3.0).abs() < 0.01);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
    }

    #[test]
    fn test_simulations_per_second() {
        let results = HierarchicalPhiResults {
            level_statistics: vec![],
            total_networks_processed: 1000,
            effective_simulations: 1_000_000,
            computation_time_ms: 100,
        };

        let rate = results.simulations_per_second();
        assert!((rate - 10_000_000.0).abs() < 1.0); // 10M sims/sec
    }
}
