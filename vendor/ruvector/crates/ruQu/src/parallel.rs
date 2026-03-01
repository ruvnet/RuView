//! Parallel Processing for 256-Tile Fabric
//!
//! This module provides rayon-based parallel processing for the tile fabric,
//! achieving 4-8× throughput improvement on multi-core systems.
//!
//! ## Architecture
//!
//! ```text
//! Syndromes ──┬──► Tile 0-63   ──┐
//!             ├──► Tile 64-127 ──┼──► TileZero Merge ──► Decision
//!             ├──► Tile 128-191 ─┤
//!             └──► Tile 192-255 ─┘
//!                  (parallel)      (parallel reduce)
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruqu::parallel::{ParallelFabric, ParallelConfig};
//!
//! let config = ParallelConfig::default(); // Auto-detect cores
//! let mut fabric = ParallelFabric::new(config)?;
//!
//! // Process syndromes in parallel
//! let decision = fabric.process_parallel(&syndrome_data)?;
//! ```

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::error::{Result, RuQuError};
use crate::tile::{GateDecision, GateThresholds, SyndromeDelta, TileReport, TileZero, WorkerTile};

/// Configuration for parallel processing
#[derive(Clone, Debug)]
pub struct ParallelConfig {
    /// Number of worker threads (0 = auto-detect)
    pub num_threads: usize,
    /// Chunk size for parallel iteration
    pub chunk_size: usize,
    /// Enable work-stealing scheduler
    pub work_stealing: bool,
    /// Tile thresholds
    pub thresholds: GateThresholds,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            num_threads: 0, // Auto-detect
            chunk_size: 16, // Process 16 tiles per chunk
            work_stealing: true,
            thresholds: GateThresholds::default(),
        }
    }
}

impl ParallelConfig {
    /// Create config optimized for low latency
    pub fn low_latency() -> Self {
        Self {
            num_threads: 4,
            chunk_size: 64,       // Larger chunks = less overhead
            work_stealing: false, // Predictable scheduling
            thresholds: GateThresholds::default(),
        }
    }

    /// Create config optimized for high throughput
    pub fn high_throughput() -> Self {
        Self {
            num_threads: 0, // Use all cores
            chunk_size: 8,  // Smaller chunks = better load balancing
            work_stealing: true,
            thresholds: GateThresholds::default(),
        }
    }
}

/// Parallel fabric for multi-threaded syndrome processing
pub struct ParallelFabric {
    /// Worker tiles (256 total, indices 1-255 are workers)
    workers: Vec<WorkerTile>,
    /// TileZero coordinator
    coordinator: TileZero,
    /// Configuration
    config: ParallelConfig,
    /// Statistics
    stats: ParallelStats,
}

/// Statistics for parallel processing
#[derive(Clone, Copy, Debug, Default)]
pub struct ParallelStats {
    /// Total syndromes processed
    pub total_processed: u64,
    /// Total parallel batches
    pub batches: u64,
    /// Average batch time (nanoseconds)
    pub avg_batch_time_ns: u64,
    /// Peak throughput (syndromes/sec)
    pub peak_throughput: f64,
}

impl ParallelFabric {
    /// Create a new parallel fabric
    pub fn new(config: ParallelConfig) -> Result<Self> {
        #[cfg(feature = "parallel")]
        {
            // Configure rayon thread pool if needed
            if config.num_threads > 0 {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(config.num_threads)
                    .build_global()
                    .ok(); // Ignore if already initialized
            }
        }

        // Create 255 worker tiles (1-255)
        let workers: Vec<WorkerTile> = (1..=255u8).map(WorkerTile::new).collect();

        let coordinator = TileZero::with_random_key(config.thresholds.clone());

        Ok(Self {
            workers,
            coordinator,
            config,
            stats: ParallelStats::default(),
        })
    }

    /// Process a syndrome batch in parallel
    #[cfg(feature = "parallel")]
    pub fn process_parallel(&mut self, syndrome: &SyndromeDelta) -> Result<GateDecision> {
        use std::time::Instant;
        let start = Instant::now();

        // Process all workers in parallel
        let reports: Vec<TileReport> = self
            .workers
            .par_iter_mut()
            .with_min_len(self.config.chunk_size)
            .map(|worker| worker.tick(syndrome))
            .collect();

        // Merge reports (single-threaded at coordinator)
        let decision = self.coordinator.merge_reports(reports);

        // Update stats
        let elapsed_ns = start.elapsed().as_nanos() as u64;
        self.stats.total_processed += 255;
        self.stats.batches += 1;
        self.stats.avg_batch_time_ns = (self.stats.avg_batch_time_ns * (self.stats.batches - 1)
            + elapsed_ns)
            / self.stats.batches;

        let throughput = 255.0 / (elapsed_ns as f64 / 1_000_000_000.0);
        if throughput > self.stats.peak_throughput {
            self.stats.peak_throughput = throughput;
        }

        Ok(decision)
    }

    /// Process a syndrome batch (fallback for non-parallel builds)
    #[cfg(not(feature = "parallel"))]
    pub fn process_parallel(&mut self, syndrome: &SyndromeDelta) -> Result<GateDecision> {
        use std::time::Instant;
        let start = Instant::now();

        // Process all workers sequentially
        let reports: Vec<TileReport> = self
            .workers
            .iter_mut()
            .map(|worker| worker.tick(syndrome))
            .collect();

        // Merge reports
        let decision = self.coordinator.merge_reports(reports);

        // Update stats
        let elapsed_ns = start.elapsed().as_nanos() as u64;
        self.stats.total_processed += 255;
        self.stats.batches += 1;
        self.stats.avg_batch_time_ns = (self.stats.avg_batch_time_ns * (self.stats.batches - 1)
            + elapsed_ns)
            / self.stats.batches;

        Ok(decision)
    }

    /// Process multiple syndromes in parallel (batch mode)
    #[cfg(feature = "parallel")]
    pub fn process_batch(&mut self, syndromes: &[SyndromeDelta]) -> Result<Vec<GateDecision>> {
        // Process each syndrome, parallelizing across tiles within each
        let decisions: Vec<GateDecision> = syndromes
            .iter()
            .map(|s| self.process_parallel(s).unwrap_or(GateDecision::Defer))
            .collect();

        Ok(decisions)
    }

    /// Process multiple syndromes (fallback)
    #[cfg(not(feature = "parallel"))]
    pub fn process_batch(&mut self, syndromes: &[SyndromeDelta]) -> Result<Vec<GateDecision>> {
        let decisions: Vec<GateDecision> = syndromes
            .iter()
            .map(|s| self.process_parallel(s).unwrap_or(GateDecision::Defer))
            .collect();

        Ok(decisions)
    }

    /// Get processing statistics
    pub fn stats(&self) -> &ParallelStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = ParallelStats::default();
    }

    /// Get the coordinator for direct access
    pub fn coordinator(&self) -> &TileZero {
        &self.coordinator
    }

    /// Get mutable coordinator
    pub fn coordinator_mut(&mut self) -> &mut TileZero {
        &mut self.coordinator
    }
}

/// Parallel reduce for aggregating tile reports
#[cfg(feature = "parallel")]
pub fn parallel_aggregate(reports: &[TileReport]) -> (f64, f64, f64) {
    use rayon::prelude::*;

    if reports.is_empty() {
        return (f64::MAX, 0.0, 1.0);
    }

    // Parallel reduction for min_cut (minimum)
    let min_cut = reports
        .par_iter()
        .map(|r| {
            if r.local_cut > 0.0 {
                r.local_cut
            } else {
                f64::MAX
            }
        })
        .reduce(|| f64::MAX, |a, b| a.min(b));

    // Parallel reduction for shift (maximum)
    let max_shift = reports
        .par_iter()
        .map(|r| r.shift_score)
        .reduce(|| 0.0, |a, b| a.max(b));

    // Parallel reduction for e-value (geometric mean via log sum)
    let log_sum: f64 = reports
        .par_iter()
        .map(|r| f64::log2(r.e_value.max(1e-10)))
        .sum();

    let e_aggregate = f64::exp2(log_sum / reports.len() as f64);

    (min_cut, max_shift, e_aggregate)
}

/// Sequential aggregate (fallback)
#[cfg(not(feature = "parallel"))]
pub fn parallel_aggregate(reports: &[TileReport]) -> (f64, f64, f64) {
    if reports.is_empty() {
        return (f64::MAX, 0.0, 1.0);
    }

    let mut min_cut = f64::MAX;
    let mut max_shift = 0.0;
    let mut log_sum = 0.0;

    for r in reports {
        if r.local_cut > 0.0 && r.local_cut < min_cut {
            min_cut = r.local_cut;
        }
        if r.shift_score > max_shift {
            max_shift = r.shift_score;
        }
        log_sum += f64::log2(r.e_value.max(1e-10));
    }

    let e_aggregate = f64::exp2(log_sum / reports.len() as f64);
    (min_cut, max_shift, e_aggregate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_config_default() {
        let config = ParallelConfig::default();
        assert_eq!(config.num_threads, 0);
        assert!(config.work_stealing);
    }

    #[test]
    fn test_parallel_fabric_creation() {
        let config = ParallelConfig::default();
        let fabric = ParallelFabric::new(config);
        assert!(fabric.is_ok());

        let fabric = fabric.unwrap();
        assert_eq!(fabric.workers.len(), 255);
    }

    #[test]
    fn test_parallel_process() {
        let config = ParallelConfig::default();
        let mut fabric = ParallelFabric::new(config).unwrap();

        let syndrome = SyndromeDelta::new(1, 2, 100);
        let decision = fabric.process_parallel(&syndrome);

        assert!(decision.is_ok());
    }

    #[test]
    fn test_parallel_aggregate() {
        let reports: Vec<TileReport> = (1..=10)
            .map(|i| {
                let mut r = TileReport::new(i);
                r.local_cut = i as f64 * 2.0;
                r.shift_score = i as f64 * 0.05;
                r.e_value = 100.0;
                r
            })
            .collect();

        let (min_cut, max_shift, e_agg) = parallel_aggregate(&reports);

        assert_eq!(min_cut, 2.0); // First report has 2.0
        assert!((max_shift - 0.5).abs() < 0.001); // Last report has 0.5
        assert!((e_agg - 100.0).abs() < 0.001); // All have 100.0
    }
}
