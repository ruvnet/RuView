#![allow(
    clippy::all,
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_assignments,
    non_camel_case_types,
    clippy::approx_constant,
    unexpected_cfgs,
    unused_must_use,
    unused_parens
)]
//! Test utilities for ANE/Core ML testing
//!
//! This module provides shared test utilities, fixtures, and helper functions
//! for testing Apple Neural Engine and Core ML functionality.
//!
//! ## Features
//!
//! - Random tensor generators with various distributions
//! - Comparison utilities with configurable tolerance
//! - Small test model generators for quick testing
//! - Platform detection helpers
//! - Benchmark utilities

use std::time::{Duration, Instant};

// ============================================================================
// Platform Detection
// ============================================================================

/// Check if running on Apple Silicon
pub fn is_apple_silicon() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
}

/// Check if the coreml feature is enabled
pub fn is_coreml_enabled() -> bool {
    cfg!(feature = "coreml")
}

/// Check if both Apple Silicon and coreml feature are available
pub fn is_ane_test_enabled() -> bool {
    is_apple_silicon() && is_coreml_enabled()
}

/// Skip message for non-Apple Silicon platforms
pub fn skip_non_apple_silicon() -> Option<&'static str> {
    if !is_apple_silicon() {
        Some("Test skipped: requires Apple Silicon")
    } else {
        None
    }
}

/// Skip message for non-coreml builds
pub fn skip_non_coreml() -> Option<&'static str> {
    if !is_coreml_enabled() {
        Some("Test skipped: requires coreml feature")
    } else {
        None
    }
}

// ============================================================================
// Random Tensor Generators
// ============================================================================

/// Simple linear congruential generator for reproducible random numbers
pub struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    /// Create a new RNG with the given seed
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Generate the next random u64
    pub fn next_u64(&mut self) -> u64 {
        // LCG parameters (same as glibc)
        self.state = self.state.wrapping_mul(1103515245).wrapping_add(12345);
        self.state
    }

    /// Generate a random f32 in [0, 1)
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() as f64 / u64::MAX as f64) as f32
    }

    /// Generate a random f32 in [min, max)
    pub fn next_f32_range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

/// Generate a random tensor with uniform distribution
pub fn random_tensor_uniform(size: usize, min: f32, max: f32, seed: u64) -> Vec<f32> {
    let mut rng = SimpleRng::new(seed);
    (0..size).map(|_| rng.next_f32_range(min, max)).collect()
}

/// Generate a random tensor with approximate normal distribution
/// Uses Box-Muller transform for simplicity
pub fn random_tensor_normal(size: usize, mean: f32, std: f32, seed: u64) -> Vec<f32> {
    let mut rng = SimpleRng::new(seed);
    let mut result = Vec::with_capacity(size);

    while result.len() < size {
        let u1 = rng.next_f32().max(1e-10); // Avoid log(0)
        let u2 = rng.next_f32();

        let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
        let z1 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).sin();

        result.push(mean + z0 * std);
        if result.len() < size {
            result.push(mean + z1 * std);
        }
    }

    result
}

/// Generate a tensor with sequential values
pub fn sequential_tensor(size: usize, start: f32, step: f32) -> Vec<f32> {
    (0..size).map(|i| start + (i as f32) * step).collect()
}

/// Generate a tensor filled with a constant value
pub fn constant_tensor(size: usize, value: f32) -> Vec<f32> {
    vec![value; size]
}

/// Generate an identity matrix
pub fn identity_matrix(size: usize) -> Vec<f32> {
    let mut result = vec![0.0; size * size];
    for i in 0..size {
        result[i * size + i] = 1.0;
    }
    result
}

/// Generate a zero matrix
pub fn zero_matrix(rows: usize, cols: usize) -> Vec<f32> {
    vec![0.0; rows * cols]
}

// ============================================================================
// Comparison Utilities
// ============================================================================

/// Configuration for tensor comparison
#[derive(Debug, Clone)]
pub struct CompareConfig {
    /// Absolute tolerance
    pub atol: f32,
    /// Relative tolerance
    pub rtol: f32,
    /// Whether to print differences
    pub verbose: bool,
    /// Maximum number of differences to report
    pub max_diffs: usize,
}

impl Default for CompareConfig {
    fn default() -> Self {
        Self {
            atol: 1e-5,
            rtol: 1e-4,
            verbose: false,
            max_diffs: 10,
        }
    }
}

impl CompareConfig {
    /// Create a loose tolerance config (for ANE vs CPU comparison)
    pub fn loose() -> Self {
        Self {
            atol: 1e-3,
            rtol: 1e-2,
            verbose: true,
            max_diffs: 5,
        }
    }

    /// Create a strict tolerance config
    pub fn strict() -> Self {
        Self {
            atol: 1e-6,
            rtol: 1e-5,
            verbose: true,
            max_diffs: 10,
        }
    }
}

/// Result of tensor comparison
#[derive(Debug)]
pub struct CompareResult {
    /// Whether the tensors are approximately equal
    pub equal: bool,
    /// Maximum absolute difference
    pub max_abs_diff: f32,
    /// Maximum relative difference
    pub max_rel_diff: f32,
    /// Index of maximum absolute difference
    pub max_abs_diff_idx: usize,
    /// Number of elements that differ
    pub num_diffs: usize,
    /// Total number of elements compared
    pub num_elements: usize,
    /// List of (index, expected, actual, abs_diff) for differences
    pub differences: Vec<(usize, f32, f32, f32)>,
}

/// Compare two tensors element-wise with configurable tolerance
pub fn compare_tensors(expected: &[f32], actual: &[f32], config: &CompareConfig) -> CompareResult {
    assert_eq!(expected.len(), actual.len(), "Tensor sizes must match");

    let mut max_abs_diff = 0.0f32;
    let mut max_rel_diff = 0.0f32;
    let mut max_abs_diff_idx = 0;
    let mut differences = Vec::new();

    for (i, (&e, &a)) in expected.iter().zip(actual.iter()).enumerate() {
        let abs_diff = (e - a).abs();
        let rel_diff = if e.abs() > 1e-10 {
            abs_diff / e.abs()
        } else {
            abs_diff
        };

        if abs_diff > max_abs_diff {
            max_abs_diff = abs_diff;
            max_abs_diff_idx = i;
        }
        if rel_diff > max_rel_diff {
            max_rel_diff = rel_diff;
        }

        // Check if this element differs beyond tolerance
        let within_tol = abs_diff <= config.atol + config.rtol * e.abs();
        if !within_tol && differences.len() < config.max_diffs {
            differences.push((i, e, a, abs_diff));
        }
    }

    let equal =
        max_abs_diff <= config.atol || max_rel_diff <= config.rtol || differences.is_empty();

    if config.verbose && !equal {
        eprintln!("Tensor comparison failed:");
        eprintln!(
            "  Max abs diff: {} at index {}",
            max_abs_diff, max_abs_diff_idx
        );
        eprintln!("  Max rel diff: {}", max_rel_diff);
        eprintln!("  Differences ({}/{}):", differences.len(), expected.len());
        for (idx, exp, act, diff) in &differences {
            eprintln!(
                "    [{}]: expected={}, actual={}, diff={}",
                idx, exp, act, diff
            );
        }
    }

    CompareResult {
        equal,
        max_abs_diff,
        max_rel_diff,
        max_abs_diff_idx,
        num_diffs: differences.len(),
        num_elements: expected.len(),
        differences,
    }
}

/// Simple approximate equality check
pub fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() < eps
}

/// Check if all elements in a tensor are finite
pub fn all_finite(tensor: &[f32]) -> bool {
    tensor.iter().all(|v| v.is_finite())
}

/// Check if a tensor sums to approximately 1.0 (for softmax output)
pub fn sums_to_one(tensor: &[f32], eps: f32) -> bool {
    let sum: f32 = tensor.iter().sum();
    approx_eq(sum, 1.0, eps)
}

/// Check if all elements are in range [min, max]
pub fn all_in_range(tensor: &[f32], min: f32, max: f32) -> bool {
    tensor.iter().all(|&v| v >= min && v <= max)
}

// ============================================================================
// Small Test Model Generators
// ============================================================================

/// Configuration for a small test model
#[derive(Debug, Clone)]
pub struct TestModelConfig {
    /// Hidden dimension
    pub hidden_dim: usize,
    /// Number of attention heads
    pub num_heads: usize,
    /// Intermediate (FFN) dimension
    pub intermediate_dim: usize,
    /// Vocabulary size
    pub vocab_size: usize,
    /// Maximum sequence length
    pub max_seq_len: usize,
    /// Number of layers
    pub num_layers: usize,
}

impl Default for TestModelConfig {
    fn default() -> Self {
        Self {
            hidden_dim: 64,
            num_heads: 4,
            intermediate_dim: 128,
            vocab_size: 1000,
            max_seq_len: 128,
            num_layers: 2,
        }
    }
}

impl TestModelConfig {
    /// Create a tiny model config for quick tests
    pub fn tiny() -> Self {
        Self {
            hidden_dim: 32,
            num_heads: 2,
            intermediate_dim: 64,
            vocab_size: 256,
            max_seq_len: 32,
            num_layers: 1,
        }
    }

    /// Create a small model config
    pub fn small() -> Self {
        Self::default()
    }

    /// Create a medium model config for more thorough testing
    pub fn medium() -> Self {
        Self {
            hidden_dim: 256,
            num_heads: 8,
            intermediate_dim: 512,
            vocab_size: 4096,
            max_seq_len: 256,
            num_layers: 4,
        }
    }

    /// Head dimension
    pub fn head_dim(&self) -> usize {
        self.hidden_dim / self.num_heads
    }
}

/// Generate random weights for a layer
pub struct TestWeights {
    seed: u64,
}

impl TestWeights {
    /// Create a new weight generator with the given seed
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Generate weights for a linear layer
    pub fn linear(&mut self, in_features: usize, out_features: usize) -> Vec<f32> {
        // Xavier initialization scale
        let scale = (2.0 / (in_features + out_features) as f32).sqrt();
        let weights = random_tensor_uniform(in_features * out_features, -scale, scale, self.seed);
        self.seed += 1;
        weights
    }

    /// Generate bias for a linear layer
    pub fn bias(&mut self, features: usize) -> Vec<f32> {
        let bias = random_tensor_uniform(features, -0.01, 0.01, self.seed);
        self.seed += 1;
        bias
    }

    /// Generate layer norm weights (initialized to 1.0)
    pub fn layer_norm_weight(&self, features: usize) -> Vec<f32> {
        vec![1.0; features]
    }

    /// Generate layer norm bias (initialized to 0.0)
    pub fn layer_norm_bias(&self, features: usize) -> Vec<f32> {
        vec![0.0; features]
    }

    /// Generate embedding table
    pub fn embedding(&mut self, vocab_size: usize, hidden_dim: usize) -> Vec<f32> {
        let scale = 0.02;
        let weights = random_tensor_normal(vocab_size * hidden_dim, 0.0, scale, self.seed);
        self.seed += 1;
        weights
    }
}

// ============================================================================
// Benchmark Utilities
// ============================================================================

/// Result of a benchmark run
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Name of the benchmark
    pub name: String,
    /// Total time for all iterations
    pub total_time: Duration,
    /// Number of iterations
    pub iterations: usize,
    /// Average time per iteration
    pub avg_time: Duration,
    /// Minimum time per iteration
    pub min_time: Duration,
    /// Maximum time per iteration
    pub max_time: Duration,
}

impl BenchmarkResult {
    /// Print the benchmark result
    pub fn print(&self) {
        println!(
            "{}: avg={:?}, min={:?}, max={:?} ({} iterations)",
            self.name, self.avg_time, self.min_time, self.max_time, self.iterations
        );
    }
}

/// Run a simple benchmark
pub fn benchmark<F>(name: &str, iterations: usize, mut f: F) -> BenchmarkResult
where
    F: FnMut(),
{
    // Warmup
    for _ in 0..3 {
        f();
    }

    let mut times = Vec::with_capacity(iterations);
    let total_start = Instant::now();

    for _ in 0..iterations {
        let start = Instant::now();
        f();
        times.push(start.elapsed());
    }

    let total_time = total_start.elapsed();
    let avg_time = total_time / iterations as u32;
    let min_time = times.iter().min().cloned().unwrap_or(Duration::ZERO);
    let max_time = times.iter().max().cloned().unwrap_or(Duration::ZERO);

    BenchmarkResult {
        name: name.to_string(),
        total_time,
        iterations,
        avg_time,
        min_time,
        max_time,
    }
}

/// Compare two benchmark results
pub fn compare_benchmarks(baseline: &BenchmarkResult, optimized: &BenchmarkResult) -> f64 {
    baseline.avg_time.as_secs_f64() / optimized.avg_time.as_secs_f64()
}

// ============================================================================
// Test Data Fixtures
// ============================================================================

/// Common test data for activation function tests
pub struct ActivationTestData {
    /// Input values covering various ranges
    pub inputs: Vec<f32>,
    /// Expected GELU outputs (approximate)
    pub expected_gelu: Vec<f32>,
    /// Expected SiLU outputs (approximate)
    pub expected_silu: Vec<f32>,
}

impl Default for ActivationTestData {
    fn default() -> Self {
        let inputs: Vec<f32> = vec![-3.0, -2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0, 3.0];

        // Pre-computed expected values (approximate)
        let expected_gelu: Vec<f32> = vec![
            -0.004, // GELU(-3)
            -0.045, // GELU(-2)
            -0.159, // GELU(-1)
            -0.154, // GELU(-0.5)
            0.0,    // GELU(0)
            0.346,  // GELU(0.5)
            0.841,  // GELU(1)
            1.955,  // GELU(2)
            2.996,  // GELU(3)
        ];

        let expected_silu: Vec<f32> = inputs
            .iter()
            .map(|&x: &f32| x / (1.0_f32 + (-x).exp()))
            .collect();

        Self {
            inputs,
            expected_gelu,
            expected_silu,
        }
    }
}

/// Common test data for matrix multiplication tests
pub struct MatmulTestData {
    /// 2x2 matrix A
    pub a_2x2: Vec<f32>,
    /// 2x2 matrix B
    pub b_2x2: Vec<f32>,
    /// Expected C = A * B (2x2)
    pub c_2x2: Vec<f32>,
    /// Identity matrix 2x2
    pub identity_2x2: Vec<f32>,
}

impl Default for MatmulTestData {
    fn default() -> Self {
        Self {
            a_2x2: vec![1.0, 2.0, 3.0, 4.0],
            b_2x2: vec![5.0, 6.0, 7.0, 8.0],
            c_2x2: vec![19.0, 22.0, 43.0, 50.0], // A * B
            identity_2x2: vec![1.0, 0.0, 0.0, 1.0],
        }
    }
}

// ============================================================================
// Tests for the test utilities
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_rng() {
        let mut rng1 = SimpleRng::new(42);
        let mut rng2 = SimpleRng::new(42);

        // Same seed should produce same sequence
        for _ in 0..10 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn test_random_tensor_uniform() {
        let tensor = random_tensor_uniform(100, 0.0, 1.0, 42);
        assert_eq!(tensor.len(), 100);
        assert!(tensor.iter().all(|&v| v >= 0.0 && v < 1.0));
    }

    #[test]
    fn test_random_tensor_normal() {
        let tensor = random_tensor_normal(1000, 0.0, 1.0, 42);
        assert_eq!(tensor.len(), 1000);

        // Check approximate mean (should be close to 0)
        let mean: f32 = tensor.iter().sum::<f32>() / tensor.len() as f32;
        assert!(mean.abs() < 0.2, "Mean should be close to 0, got {}", mean);
    }

    #[test]
    fn test_sequential_tensor() {
        let tensor = sequential_tensor(5, 0.0, 1.0);
        assert_eq!(tensor, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_identity_matrix() {
        let identity = identity_matrix(3);
        assert_eq!(identity, vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0,]);
    }

    #[test]
    fn test_compare_tensors_equal() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let result = compare_tensors(&a, &b, &CompareConfig::default());
        assert!(result.equal);
        assert_eq!(result.num_diffs, 0);
    }

    #[test]
    fn test_compare_tensors_within_tolerance() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.00001, 2.00001, 3.00001];
        let result = compare_tensors(&a, &b, &CompareConfig::default());
        assert!(result.equal);
    }

    #[test]
    fn test_compare_tensors_different() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.5, 3.0]; // Middle element differs
        let config = CompareConfig::strict();
        let result = compare_tensors(&a, &b, &config);
        assert!(!result.equal);
        assert!(result.num_diffs > 0);
    }

    #[test]
    fn test_all_finite() {
        assert!(all_finite(&[1.0, 2.0, 3.0]));
        assert!(!all_finite(&[1.0, f32::NAN, 3.0]));
        assert!(!all_finite(&[1.0, f32::INFINITY, 3.0]));
    }

    #[test]
    fn test_sums_to_one() {
        assert!(sums_to_one(&[0.25, 0.25, 0.25, 0.25], 1e-5));
        assert!(sums_to_one(&[0.1, 0.2, 0.3, 0.4], 1e-5));
        assert!(!sums_to_one(&[0.1, 0.2, 0.3], 1e-5));
    }

    #[test]
    fn test_benchmark() {
        let result = benchmark("test_add", 10, || {
            let _sum: i32 = (0..1000).sum();
        });
        assert_eq!(result.iterations, 10);
        assert!(result.avg_time > Duration::ZERO);
    }

    #[test]
    fn test_model_config() {
        let config = TestModelConfig::tiny();
        assert_eq!(config.head_dim(), 16); // 32 / 2

        let config = TestModelConfig::default();
        assert_eq!(config.head_dim(), 16); // 64 / 4
    }

    #[test]
    fn test_weight_generator() {
        let mut gen = TestWeights::new(42);

        let linear = gen.linear(64, 128);
        assert_eq!(linear.len(), 64 * 128);

        let bias = gen.bias(128);
        assert_eq!(bias.len(), 128);

        let ln_weight = gen.layer_norm_weight(64);
        assert!(ln_weight.iter().all(|&v| v == 1.0));
    }

    #[test]
    fn test_matmul_test_data() {
        let data = MatmulTestData::default();
        assert_eq!(data.a_2x2.len(), 4);
        assert_eq!(data.c_2x2, vec![19.0, 22.0, 43.0, 50.0]);
    }
}
