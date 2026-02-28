//! Benchmark utilities for measuring optimization performance.
//!
//! Provides lightweight timing and throughput measurement without
//! external dependencies, suitable for embedded/no_std environments.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use ruvector_mincut_gated_transformer::kernel::bench_utils::*;
//!
//! let mut timer = Timer::new();
//!
//! // Warm-up run
//! timer.start();
//! // ... operation ...
//! timer.stop();
//!
//! // Measured run
//! timer.reset();
//! timer.start();
//! // ... operation ...
//! let elapsed_ns = timer.elapsed_ns();
//!
//! // Compute throughput
//! let ops = 1024 * 1024; // Number of operations
//! let gflops = compute_gflops(ops, elapsed_ns);
//! ```

extern crate alloc;
use alloc::vec::Vec;

/// Lightweight timer for benchmarking.
///
/// Uses CPU cycle counter when available, falls back to iteration counting.
#[derive(Clone, Debug)]
pub struct Timer {
    /// Start timestamp
    start_cycles: u64,
    /// End timestamp
    end_cycles: u64,
    /// Whether timer is running
    running: bool,
}

impl Timer {
    /// Create a new timer.
    pub fn new() -> Self {
        Self {
            start_cycles: 0,
            end_cycles: 0,
            running: false,
        }
    }

    /// Start the timer.
    #[inline]
    pub fn start(&mut self) {
        self.start_cycles = read_timestamp();
        self.running = true;
    }

    /// Stop the timer.
    #[inline]
    pub fn stop(&mut self) {
        if self.running {
            self.end_cycles = read_timestamp();
            self.running = false;
        }
    }

    /// Reset the timer.
    #[inline]
    pub fn reset(&mut self) {
        self.start_cycles = 0;
        self.end_cycles = 0;
        self.running = false;
    }

    /// Get elapsed cycles.
    #[inline]
    pub fn elapsed_cycles(&self) -> u64 {
        if self.running {
            read_timestamp().saturating_sub(self.start_cycles)
        } else {
            self.end_cycles.saturating_sub(self.start_cycles)
        }
    }

    /// Get elapsed nanoseconds (estimated).
    ///
    /// Assumes ~3 GHz CPU frequency. For accurate timing, use std::time
    /// or criterion benchmarks.
    #[inline]
    pub fn elapsed_ns(&self) -> u64 {
        // Assume ~3 GHz CPU frequency
        self.elapsed_cycles() / 3
    }

    /// Get elapsed microseconds (estimated).
    #[inline]
    pub fn elapsed_us(&self) -> u64 {
        self.elapsed_ns() / 1000
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

/// Read CPU timestamp counter.
#[inline]
fn read_timestamp() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        // Use RDTSC instruction
        #[cfg(target_feature = "sse2")]
        unsafe {
            core::arch::x86_64::_rdtsc()
        }
        #[cfg(not(target_feature = "sse2"))]
        {
            // Fallback: use a simple counter
            static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
            COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // Use CNTVCT_EL0 (virtual timer count)
        let mut count: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntvct_el0", out(reg) count);
        }
        count
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        // Fallback: use a simple counter
        static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
        COUNTER.fetch_add(1000, core::sync::atomic::Ordering::Relaxed)
    }
}

/// Compute GFLOPS from operation count and elapsed nanoseconds.
#[inline]
pub fn compute_gflops(operations: u64, elapsed_ns: u64) -> f64 {
    if elapsed_ns == 0 {
        return 0.0;
    }
    (operations as f64) / (elapsed_ns as f64)
}

/// Compute throughput in GB/s from bytes and elapsed nanoseconds.
#[inline]
pub fn compute_bandwidth_gbps(bytes: u64, elapsed_ns: u64) -> f64 {
    if elapsed_ns == 0 {
        return 0.0;
    }
    (bytes as f64) / (elapsed_ns as f64)
}

/// Benchmark statistics collector.
#[derive(Clone, Debug)]
pub struct BenchStats {
    /// All measured times in nanoseconds
    samples: Vec<u64>,
    /// Operation count per sample
    ops_per_sample: u64,
}

impl BenchStats {
    /// Create a new stats collector.
    pub fn new(ops_per_sample: u64) -> Self {
        Self {
            samples: Vec::with_capacity(100),
            ops_per_sample,
        }
    }

    /// Add a timing sample.
    pub fn add_sample(&mut self, elapsed_ns: u64) {
        self.samples.push(elapsed_ns);
    }

    /// Get sample count.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Get minimum time in nanoseconds.
    pub fn min_ns(&self) -> u64 {
        self.samples.iter().copied().min().unwrap_or(0)
    }

    /// Get maximum time in nanoseconds.
    pub fn max_ns(&self) -> u64 {
        self.samples.iter().copied().max().unwrap_or(0)
    }

    /// Get mean time in nanoseconds.
    pub fn mean_ns(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.samples.iter().sum();
        sum as f64 / self.samples.len() as f64
    }

    /// Get median time in nanoseconds.
    pub fn median_ns(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        sorted[sorted.len() / 2]
    }

    /// Get standard deviation in nanoseconds.
    pub fn std_dev_ns(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        let mean = self.mean_ns();
        let variance: f64 = self
            .samples
            .iter()
            .map(|&s| {
                let diff = s as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / (self.samples.len() - 1) as f64;
        variance.sqrt()
    }

    /// Get peak GFLOPS (from minimum time).
    pub fn peak_gflops(&self) -> f64 {
        compute_gflops(self.ops_per_sample, self.min_ns())
    }

    /// Get mean GFLOPS.
    pub fn mean_gflops(&self) -> f64 {
        compute_gflops(self.ops_per_sample, self.mean_ns() as u64)
    }
}

/// Operation count for GEMM (2 * M * N * K).
#[inline]
pub fn gemm_ops(m: usize, n: usize, k: usize) -> u64 {
    (2 * m * n * k) as u64
}

/// Operation count for sparse matrix-vector multiply (2 * nnz).
#[inline]
pub fn spmv_ops(nnz: usize) -> u64 {
    (2 * nnz) as u64
}

/// Operation count for dot product (2 * length).
#[inline]
pub fn dot_ops(length: usize) -> u64 {
    (2 * length) as u64
}

/// Memory bytes for GEMM (A + B + C).
#[inline]
pub fn gemm_bytes(m: usize, n: usize, k: usize, elem_size: usize) -> u64 {
    ((m * k + k * n + m * n) * elem_size) as u64
}

/// Benchmark configuration for performance testing.
#[derive(Clone, Debug)]
pub struct BenchConfig {
    /// Number of warmup iterations
    pub warmup_iters: u32,
    /// Number of measurement iterations
    pub measure_iters: u32,
    /// Minimum time per measurement (nanoseconds)
    pub min_time_ns: u64,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            warmup_iters: 10,
            measure_iters: 100,
            min_time_ns: 1_000_000, // 1ms
        }
    }
}

impl BenchConfig {
    /// Quick configuration for fast testing.
    pub fn quick() -> Self {
        Self {
            warmup_iters: 3,
            measure_iters: 20,
            min_time_ns: 100_000, // 100μs
        }
    }

    /// Thorough configuration for accurate measurements.
    pub fn thorough() -> Self {
        Self {
            warmup_iters: 50,
            measure_iters: 500,
            min_time_ns: 10_000_000, // 10ms
        }
    }
}

/// Run a benchmark with the given configuration.
///
/// # Arguments
///
/// * `config` - Benchmark configuration
/// * `ops_per_iter` - Number of operations per iteration
/// * `f` - Function to benchmark
///
/// # Returns
///
/// BenchStats with timing information
pub fn run_benchmark<F>(config: &BenchConfig, ops_per_iter: u64, mut f: F) -> BenchStats
where
    F: FnMut(),
{
    let mut stats = BenchStats::new(ops_per_iter);
    let mut timer = Timer::new();

    // Warmup
    for _ in 0..config.warmup_iters {
        f();
    }

    // Measurement
    for _ in 0..config.measure_iters {
        timer.reset();
        timer.start();
        f();
        timer.stop();
        stats.add_sample(timer.elapsed_ns());
    }

    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_basic() {
        let mut timer = Timer::new();

        timer.start();
        // Small busy work
        let mut sum = 0u64;
        for i in 0..1000 {
            sum += i;
        }
        timer.stop();

        let elapsed = timer.elapsed_cycles();
        assert!(elapsed > 0, "Timer should measure some cycles");
        // Use sum to prevent optimization
        assert!(sum > 0);
    }

    #[test]
    fn test_timer_reset() {
        let mut timer = Timer::new();

        timer.start();
        timer.stop();
        let first = timer.elapsed_cycles();

        timer.reset();
        assert_eq!(timer.elapsed_cycles(), 0);

        timer.start();
        timer.stop();
        let second = timer.elapsed_cycles();

        assert!(first > 0);
        assert!(second > 0);
    }

    #[test]
    fn test_bench_stats() {
        let mut stats = BenchStats::new(1000);

        stats.add_sample(100);
        stats.add_sample(200);
        stats.add_sample(150);

        assert_eq!(stats.sample_count(), 3);
        assert_eq!(stats.min_ns(), 100);
        assert_eq!(stats.max_ns(), 200);
        assert!((stats.mean_ns() - 150.0).abs() < 0.1);
        assert_eq!(stats.median_ns(), 150);
    }

    #[test]
    fn test_compute_gflops() {
        let ops = 1_000_000_000; // 1 billion ops
        let elapsed_ns = 1_000_000_000; // 1 second

        let gflops = compute_gflops(ops, elapsed_ns);
        assert!((gflops - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_gemm_ops() {
        let m = 128;
        let n = 256;
        let k = 64;

        let ops = gemm_ops(m, n, k);
        assert_eq!(ops, 2 * 128 * 256 * 64);
    }

    #[test]
    fn test_run_benchmark() {
        let config = BenchConfig::quick();

        let stats = run_benchmark(&config, 1000, || {
            let mut sum = 0u64;
            for i in 0..100 {
                sum += i;
            }
            // Use black_box-like trick
            let _ = sum;
        });

        assert_eq!(stats.sample_count(), config.measure_iters as usize);
        assert!(stats.min_ns() > 0);
    }
}
