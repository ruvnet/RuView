# Testing Strategy for Sublinear-Time Solver

## Overview

This document outlines a comprehensive testing strategy for the sublinear-time solver project, focusing on correctness, performance, and reliability across all components including Rust core algorithms, WASM bindings, and integration layers.

## 1. Unit Testing (Rust)

### 1.1 Module-Level Test Organization

```rust
// src/algorithms/tests/mod.rs
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    mod condition_number_tests;
    mod spectral_analysis_tests;
    mod graph_algorithms_tests;
    mod matrix_operations_tests;
    mod streaming_tests;
}

// Example: src/algorithms/tests/condition_number_tests.rs
use crate::algorithms::condition_number::*;
use approx::assert_relative_eq;

#[test]
fn test_well_conditioned_matrix() {
    let matrix = create_identity_matrix(5);
    let cond = estimate_condition_number(&matrix, 1e-10);
    assert_relative_eq!(cond, 1.0, epsilon = 1e-6);
}

#[test]
fn test_singular_matrix_detection() {
    let mut matrix = DMatrix::zeros(3, 3);
    matrix[(0, 0)] = 1.0;
    matrix[(1, 1)] = 1.0;
    // Third row/column remains zero

    let result = estimate_condition_number(&matrix, 1e-10);
    assert!(result.is_infinite() || result > 1e12);
}
```

### 1.2 Property-Based Testing with Proptest

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_condition_number_invariants(
        size in 2usize..20,
        scale in 1e-3f64..1e3
    ) {
        let matrix = generate_spd_matrix(size, scale);
        let cond = estimate_condition_number(&matrix, 1e-12);

        // Property: condition number >= 1
        prop_assert!(cond >= 1.0);

        // Property: scaling preserves relative condition
        let scaled = &matrix * 2.0;
        let scaled_cond = estimate_condition_number(&scaled, 1e-12);
        prop_assert_relative_eq!(cond, scaled_cond, epsilon = 1e-6);
    }

    #[test]
    fn test_spectral_radius_bounds(
        size in 3usize..15,
        density in 0.1f64..0.9
    ) {
        let graph = generate_random_graph(size, density);
        let matrix = graph_to_adjacency_matrix(&graph);
        let radius = compute_spectral_radius(&matrix);

        // Property: spectral radius <= max degree
        let max_degree = graph.vertices().map(|v| graph.degree(v)).max().unwrap();
        prop_assert!(radius <= max_degree as f64 + 1e-10);
    }
}

// Custom strategies for test data generation
fn generate_spd_matrix(size: usize, scale: f64) -> impl Strategy<Value = DMatrix<f64>> {
    (0..size*size)
        .prop_map(move |_| {
            let mut rng = thread_rng();
            let a = DMatrix::from_fn(size, size, |_, _| rng.gen_range(-1.0..1.0));
            let spd = &a * a.transpose() + DMatrix::identity(size, size) * scale;
            spd
        })
}
```

### 1.3 Numerical Accuracy Tests

```rust
#[test]
fn test_numerical_stability() {
    let test_cases = vec![
        ("hilbert_5", generate_hilbert_matrix(5), 1e6),
        ("vandermonde_8", generate_vandermonde_matrix(8), 1e10),
        ("toeplitz_10", generate_toeplitz_matrix(10), 1e3),
    ];

    for (name, matrix, expected_cond) in test_cases {
        let computed_cond = estimate_condition_number(&matrix, 1e-14);
        let relative_error = (computed_cond - expected_cond).abs() / expected_cond;

        assert!(
            relative_error < 0.1,
            "Test {} failed: expected {}, got {}, error: {}",
            name, expected_cond, computed_cond, relative_error
        );
    }
}

#[test]
fn test_precision_comparison() {
    let matrix = generate_ill_conditioned_matrix(10, 1e8);

    let cond_f32 = estimate_condition_number_f32(&matrix.cast::<f32>(), 1e-6);
    let cond_f64 = estimate_condition_number(&matrix, 1e-12);

    // f64 should be more accurate for ill-conditioned matrices
    assert!(cond_f64 > cond_f32 * 0.9);
    assert!(cond_f64 < cond_f32 * 1.1 || cond_f32 == f32::INFINITY);
}
```

### 1.4 Edge Case Coverage

```rust
#[test]
fn test_edge_cases() {
    // Empty/minimal matrices
    assert!(estimate_condition_number(&DMatrix::zeros(1, 1), 1e-10).is_infinite());

    // Very large matrices (memory limits)
    let result = std::panic::catch_unwind(|| {
        let large_matrix = DMatrix::identity(100_000, 100_000);
        estimate_condition_number(&large_matrix, 1e-10)
    });
    assert!(result.is_ok()); // Should handle gracefully

    // NaN/Infinity inputs
    let mut nan_matrix = DMatrix::identity(3, 3);
    nan_matrix[(1, 1)] = f64::NAN;
    let result = estimate_condition_number(&nan_matrix, 1e-10);
    assert!(result.is_nan() || result.is_infinite());
}

#[test]
fn test_memory_constraints() {
    // Test with limited memory budget
    let matrix = generate_large_sparse_matrix(1000, 0.01);

    let start_memory = get_memory_usage();
    let cond = estimate_condition_number_bounded(&matrix, 1e-10, 100_000_000); // 100MB limit
    let end_memory = get_memory_usage();

    assert!(end_memory - start_memory < 150_000_000); // Allow some overhead
    assert!(cond > 0.0);
}
```

### 1.5 Mock Strategies for Modules

```rust
// src/test_utils/mocks.rs
use mockall::mock;

mock! {
    pub GraphProvider {
        fn load_graph(&self, path: &str) -> Result<Graph, GraphError>;
        fn validate_graph(&self, graph: &Graph) -> bool;
    }
}

mock! {
    pub StreamingDataSource {
        fn next_batch(&mut self) -> Option<Vec<Edge>>;
        fn has_more(&self) -> bool;
        fn reset(&mut self);
    }
}

// Test using mocks
#[test]
fn test_streaming_algorithm_with_mock() {
    let mut mock_source = MockStreamingDataSource::new();
    mock_source
        .expect_next_batch()
        .times(3)
        .returning(|| Some(vec![Edge::new(0, 1, 1.0), Edge::new(1, 2, 1.0)]));

    mock_source
        .expect_has_more()
        .returning(|| false);

    let mut processor = StreamingProcessor::new(Box::new(mock_source));
    let result = processor.process_until_convergence(1e-6);

    assert!(result.is_ok());
    assert!(result.unwrap().iterations > 0);
}
```

## 2. Integration Testing

### 2.1 Algorithm Comparison Tests

```rust
// tests/integration/algorithm_comparison.rs
use sublinear_time_solver::*;

#[test]
fn test_power_iteration_vs_lanczos() {
    let test_matrices = load_benchmark_matrices();

    for (name, matrix) in test_matrices {
        let power_result = power_iteration_condition_number(&matrix, 1e-10, 1000);
        let lanczos_result = lanczos_condition_number(&matrix, 1e-10, 100);

        let relative_diff = (power_result - lanczos_result).abs() / power_result;

        assert!(
            relative_diff < 0.05,
            "Algorithm mismatch for {}: power={}, lanczos={}, diff={}",
            name, power_result, lanczos_result, relative_diff
        );
    }
}

#[test]
fn test_streaming_vs_batch() {
    let graph = generate_test_graph(1000, 0.1);

    // Batch processing
    let batch_result = compute_spectral_radius_batch(&graph);

    // Streaming processing
    let edges: Vec<_> = graph.edges().collect();
    let streaming_result = compute_spectral_radius_streaming(edges.into_iter(), 1e-8);

    assert_relative_eq!(batch_result, streaming_result, epsilon = 1e-6);
}
```

### 2.2 WASM Binding Tests

```rust
// tests/integration/wasm_tests.rs
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_wasm_condition_number() {
    let matrix = js_sys::Array::new();
    // Create 3x3 identity matrix
    for i in 0..9 {
        matrix.push(&JsValue::from_f64(if i % 4 == 0 { 1.0 } else { 0.0 }));
    }

    let result = estimate_condition_number_wasm(&matrix, 3, 3, 1e-10);
    assert!((result - 1.0).abs() < 1e-6);
}

#[wasm_bindgen_test]
fn test_wasm_memory_management() {
    let large_size = 500;
    let matrix = create_large_matrix_js(large_size);

    let initial_memory = web_sys::window()
        .unwrap()
        .performance()
        .unwrap()
        .memory()
        .unwrap()
        .used_js_heap_size();

    let _result = estimate_condition_number_wasm(&matrix, large_size, large_size, 1e-8);

    // Force garbage collection (if available)
    if let Ok(gc) = js_sys::Reflect::get(&js_sys::global(), &"gc".into()) {
        if !gc.is_undefined() {
            let _ = js_sys::Function::from(gc).call0(&js_sys::global());
        }
    }

    let final_memory = web_sys::window()
        .unwrap()
        .performance()
        .unwrap()
        .memory()
        .unwrap()
        .used_js_heap_size();

    // Memory should not have grown significantly
    assert!(final_memory < initial_memory + 10_000_000); // 10MB threshold
}
```

### 2.3 HTTP Streaming Tests

```rust
// tests/integration/http_streaming.rs
use tokio_test;
use reqwest;

#[tokio::test]
async fn test_streaming_api_endpoint() {
    let server = start_test_server().await;
    let client = reqwest::Client::new();

    let graph_data = generate_test_graph_json(1000);

    let response = client
        .post(&format!("{}/api/v1/streaming/spectral-radius", server.url()))
        .json(&graph_data)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let mut stream = response.bytes_stream();
    let mut results = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        let line = String::from_utf8(chunk.to_vec()).unwrap();
        if let Ok(result) = serde_json::from_str::<StreamingResult>(&line) {
            results.push(result);
        }
    }

    assert!(!results.is_empty());
    assert!(results.last().unwrap().converged);
}

#[tokio::test]
async fn test_concurrent_streaming_requests() {
    let server = start_test_server().await;
    let client = reqwest::Client::new();

    let futures: Vec<_> = (0..10)
        .map(|i| {
            let client = client.clone();
            let url = server.url().clone();
            tokio::spawn(async move {
                let graph = generate_test_graph_json(100 + i * 10);
                client
                    .post(&format!("{}/api/v1/streaming/condition-number", url))
                    .json(&graph)
                    .send()
                    .await
            })
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    for result in results {
        assert!(result.unwrap().unwrap().status().is_success());
    }
}
```

### 2.4 CLI Command Tests

```rust
// tests/integration/cli_tests.rs
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;

#[test]
fn test_cli_condition_number_file() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "3 3").unwrap();
    writeln!(temp_file, "1.0 0.0 0.0").unwrap();
    writeln!(temp_file, "0.0 1.0 0.0").unwrap();
    writeln!(temp_file, "0.0 0.0 1.0").unwrap();
    temp_file.flush().unwrap();

    let mut cmd = Command::cargo_bin("sublinear-solver").unwrap();
    cmd.arg("condition-number")
       .arg("--input")
       .arg(temp_file.path())
       .arg("--tolerance")
       .arg("1e-10");

    cmd.assert()
       .success()
       .stdout(predicate::str::contains("1.0"));
}

#[test]
fn test_cli_streaming_mode() {
    let mut cmd = Command::cargo_bin("sublinear-solver").unwrap();
    cmd.arg("spectral-radius")
       .arg("--streaming")
       .arg("--input")
       .arg("-") // stdin
       .write_stdin("0 1 1.0\n1 2 1.0\n2 0 1.0\n");

    cmd.assert()
       .success()
       .stdout(predicate::str::contains("2.0"));
}

#[test]
fn test_cli_error_handling() {
    let mut cmd = Command::cargo_bin("sublinear-solver").unwrap();
    cmd.arg("condition-number")
       .arg("--input")
       .arg("nonexistent_file.txt");

    cmd.assert()
       .failure()
       .stderr(predicate::str::contains("File not found"));
}
```

### 2.5 Cross-Platform Validation

```rust
// tests/integration/cross_platform.rs
#[cfg(target_os = "linux")]
#[test]
fn test_linux_specific_optimizations() {
    // Test SIMD optimizations on Linux
    let matrix = generate_large_matrix(1000);
    let result = estimate_condition_number_simd(&matrix, 1e-10);
    assert!(result > 0.0);
}

#[cfg(target_os = "windows")]
#[test]
fn test_windows_file_handling() {
    // Test Windows-specific file path handling
    let path = r"C:\temp\matrix.txt";
    let result = load_matrix_from_file(path);
    // Should handle Windows paths correctly
}

#[cfg(target_arch = "wasm32")]
#[test]
fn test_wasm_performance() {
    use web_time::Instant;

    let start = Instant::now();
    let matrix = generate_test_matrix(100);
    let _result = estimate_condition_number(&matrix, 1e-8);
    let duration = start.elapsed();

    // WASM should complete within reasonable time
    assert!(duration.as_millis() < 5000);
}
```

## 3. Performance Testing

### 3.1 Benchmark Suite Design

```rust
// benches/condition_number_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use sublinear_time_solver::*;

fn benchmark_condition_number_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("condition_number_scaling");

    for size in [100, 500, 1000, 2000, 5000].iter() {
        let matrix = generate_spd_matrix(*size, 1.0);

        group.bench_with_input(
            BenchmarkId::new("power_iteration", size),
            size,
            |b, &size| {
                b.iter(|| {
                    estimate_condition_number_power(
                        black_box(&matrix),
                        black_box(1e-10),
                        black_box(1000)
                    )
                })
            }
        );

        group.bench_with_input(
            BenchmarkId::new("lanczos", size),
            size,
            |b, &size| {
                b.iter(|| {
                    estimate_condition_number_lanczos(
                        black_box(&matrix),
                        black_box(1e-10),
                        black_box(100)
                    )
                })
            }
        );
    }

    group.finish();
}

fn benchmark_memory_usage(c: &mut Criterion) {
    c.benchmark_function("memory_efficient_large_matrix", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            for _ in 0..iters {
                let matrix = generate_sparse_matrix(10000, 0.001);
                let _result = black_box(estimate_condition_number_sparse(&matrix, 1e-8));
            }
            start.elapsed()
        })
    });
}

criterion_group!(benches, benchmark_condition_number_scaling, benchmark_memory_usage);
criterion_main!(benches);
```

### 3.2 Scaling Tests (10^3 to 10^7 nodes)

```rust
// tests/performance/scaling_tests.rs
use std::time::Instant;

#[test]
fn test_scaling_performance() {
    let sizes = vec![1_000, 10_000, 100_000, 1_000_000];
    let mut results = Vec::new();

    for size in sizes {
        println!("Testing size: {}", size);

        let graph = generate_scale_free_graph(size, 3); // Average degree 3
        let start = Instant::now();

        let spectral_radius = compute_spectral_radius_streaming(
            graph.edges(),
            1e-6
        );

        let duration = start.elapsed();
        results.push((size, duration, spectral_radius));

        // Verify sublinear scaling
        if results.len() > 1 {
            let prev = &results[results.len() - 2];
            let ratio = duration.as_secs_f64() / prev.1.as_secs_f64();
            let size_ratio = size as f64 / prev.0 as f64;

            // Should be better than linear scaling
            assert!(
                ratio < size_ratio * 1.1,
                "Scaling worse than linear: time ratio {} vs size ratio {}",
                ratio, size_ratio
            );
        }
    }

    // Log results for analysis
    for (size, duration, result) in results {
        println!("Size: {}, Time: {:?}, Result: {}", size, duration, result);
    }
}

#[test]
#[ignore] // Run only when specifically requested
fn test_extreme_scaling() {
    // Test with very large graphs (requires substantial memory)
    let size = 10_000_000;

    let memory_before = get_memory_usage();
    let graph = generate_sparse_graph(size, 0.0001); // Very sparse
    let memory_after_gen = get_memory_usage();

    assert!(
        memory_after_gen - memory_before < 2_000_000_000, // 2GB limit
        "Graph generation used too much memory"
    );

    let start = Instant::now();
    let result = compute_spectral_radius_approximate(&graph, 1e-4);
    let duration = start.elapsed();

    assert!(duration.as_secs() < 300); // 5 minute limit
    assert!(result > 0.0);

    println!("Extreme scale test: {} nodes in {:?}", size, duration);
}
```

### 3.3 Memory Usage Profiling

```rust
// tests/performance/memory_profiling.rs
use jemalloc_ctl::{epoch, stats};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn get_memory_stats() -> (usize, usize) {
    epoch::advance().unwrap();
    let allocated = stats::allocated::read().unwrap();
    let resident = stats::resident::read().unwrap();
    (allocated, resident)
}

#[test]
fn test_memory_usage_patterns() {
    let (initial_alloc, initial_resident) = get_memory_stats();

    {
        let matrix = generate_large_matrix(5000);
        let (after_alloc, after_resident) = get_memory_stats();

        let alloc_diff = after_alloc - initial_alloc;
        let expected_size = 5000 * 5000 * 8; // f64 matrix

        assert!(
            alloc_diff < expected_size * 2,
            "Memory usage much higher than expected: {} vs {}",
            alloc_diff, expected_size
        );

        let _result = estimate_condition_number(&matrix, 1e-10);
        let (compute_alloc, compute_resident) = get_memory_stats();

        // Algorithm shouldn't allocate much additional memory
        assert!(
            compute_alloc - after_alloc < expected_size / 2,
            "Algorithm used excessive memory"
        );
    }

    // Force garbage collection and check for leaks
    for _ in 0..10 {
        std::alloc::System.alloc(std::alloc::Layout::new::<u8>());
    }

    let (final_alloc, final_resident) = get_memory_stats();
    let leaked = final_alloc - initial_alloc;

    assert!(
        leaked < 1_000_000, // 1MB leak tolerance
        "Memory leak detected: {} bytes",
        leaked
    );
}
```

### 3.4 WASM vs Native Comparison

```rust
// tests/performance/wasm_comparison.rs
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn benchmark_native_vs_wasm() {
    use std::process::Command;
    use std::time::Instant;

    let matrix = generate_test_matrix(500);

    // Native performance
    let native_start = Instant::now();
    let native_result = estimate_condition_number(&matrix, 1e-10);
    let native_duration = native_start.elapsed();

    // Save matrix to file for WASM test
    save_matrix_to_file(&matrix, "test_matrix.txt").unwrap();

    // Run WASM version via Node.js
    let wasm_start = Instant::now();
    let output = Command::new("node")
        .arg("wasm_test_runner.js")
        .arg("test_matrix.txt")
        .output()
        .expect("Failed to run WASM test");
    let wasm_duration = wasm_start.elapsed();

    let wasm_result: f64 = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();

    // Results should be similar
    assert_relative_eq!(native_result, wasm_result, epsilon = 1e-6);

    // WASM should be at most 10x slower
    let slowdown = wasm_duration.as_secs_f64() / native_duration.as_secs_f64();
    assert!(
        slowdown < 10.0,
        "WASM too slow: {}x slowdown",
        slowdown
    );

    println!(
        "Native: {:?}, WASM: {:?}, Slowdown: {:.2}x",
        native_duration, wasm_duration, slowdown
    );
}
```

### 3.5 Streaming Latency Measurements

```rust
// tests/performance/streaming_latency.rs
use tokio::time::{Duration, Instant};

#[tokio::test]
async fn test_streaming_latency() {
    let mut stream = create_edge_stream(1000, Duration::from_millis(10));
    let mut processor = StreamingSpectralProcessor::new();

    let mut latencies = Vec::new();
    let mut total_processed = 0;

    while let Some(batch) = stream.next().await {
        let batch_start = Instant::now();
        processor.process_batch(&batch).await;
        let batch_latency = batch_start.elapsed();

        latencies.push(batch_latency);
        total_processed += batch.len();

        // Check for convergence
        if let Some(result) = processor.check_convergence(1e-6) {
            println!("Converged after {} edges: {}", total_processed, result);
            break;
        }
    }

    // Analyze latency distribution
    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];

    println!("Latency percentiles: p50={:?}, p95={:?}, p99={:?}", p50, p95, p99);

    // Assert reasonable latency bounds
    assert!(p50 < Duration::from_millis(50));
    assert!(p95 < Duration::from_millis(200));
    assert!(p99 < Duration::from_millis(500));
}
```

## 4. Numerical Testing

### 4.1 Condition Number Stress Tests

```rust
// tests/numerical/condition_number_stress.rs
#[test]
fn test_extreme_condition_numbers() {
    let test_cases = vec![
        ("well_conditioned", 1e0, 1e1),
        ("moderate", 1e3, 1e5),
        ("ill_conditioned", 1e6, 1e12),
        ("near_singular", 1e12, 1e16),
    ];

    for (name, min_cond, max_cond) in test_cases {
        for target_cond in [min_cond, min_cond * 10.0, max_cond] {
            let matrix = generate_matrix_with_condition_number(10, target_cond);
            let estimated_cond = estimate_condition_number(&matrix, 1e-14);

            let relative_error = (estimated_cond - target_cond).abs() / target_cond;

            assert!(
                relative_error < 0.5, // 50% tolerance for extreme cases
                "Test {} failed: target={}, estimated={}, error={}",
                name, target_cond, estimated_cond, relative_error
            );
        }
    }
}

#[test]
fn test_iterative_refinement() {
    let matrix = generate_hilbert_matrix(12); // Notoriously ill-conditioned

    let tolerances = [1e-6, 1e-8, 1e-10, 1e-12];
    let mut previous_estimate = f64::INFINITY;

    for tolerance in tolerances {
        let estimate = estimate_condition_number(&matrix, tolerance);

        // Tighter tolerance should give more accurate result
        assert!(
            estimate <= previous_estimate * 1.1,
            "Tighter tolerance gave worse result: {} vs {}",
            estimate, previous_estimate
        );

        previous_estimate = estimate;
    }
}
```

### 4.2 Convergence Validation

```rust
// tests/numerical/convergence_tests.rs
#[test]
fn test_power_iteration_convergence() {
    let matrix = generate_test_matrix_with_known_eigenvalues(
        vec![10.0, 5.0, 1.0, 0.1, 0.01]
    );

    let max_iterations = [10, 50, 100, 500, 1000];
    let mut convergence_history = Vec::new();

    for max_iter in max_iterations {
        let (result, iterations) = power_iteration_with_history(
            &matrix,
            1e-10,
            max_iter
        );

        convergence_history.push((max_iter, iterations, result));

        if iterations < max_iter {
            // Should converge to the same value regardless of max_iter
            if let Some((_, _, prev_result)) = convergence_history.get(convergence_history.len() - 2) {
                assert_relative_eq!(result, *prev_result, epsilon = 1e-8);
            }
        }
    }

    // Verify convergence rate is geometric
    let final_result = convergence_history.last().unwrap().2;
    let expected_ratio = 5.0 / 10.0; // Second/first eigenvalue ratio

    // Test geometric convergence rate
    for window in convergence_history.windows(2) {
        if window[1].1 < window[1].0 { // Converged
            let improvement = (window[0].2 - final_result).abs() / (window[1].2 - final_result).abs();
            assert!(
                improvement >= expected_ratio * 0.8,
                "Convergence rate slower than expected: {} vs {}",
                improvement, expected_ratio
            );
        }
    }
}

#[test]
fn test_lanczos_convergence() {
    let matrix = generate_sparse_symmetric_matrix(1000, 0.01);

    let subspace_sizes = [10, 20, 50, 100];
    let mut estimates = Vec::new();

    for k in subspace_sizes {
        let estimate = lanczos_condition_number(&matrix, 1e-10, k);
        estimates.push(estimate);

        // Larger subspace should give better estimate
        if estimates.len() > 1 {
            let improvement = (estimates[estimates.len()-1] - estimates[estimates.len()-2]).abs();
            assert!(
                improvement < estimates[estimates.len()-1] * 0.1,
                "Lanczos estimate not stabilizing"
            );
        }
    }
}
```

### 4.3 Error Bound Verification

```rust
// tests/numerical/error_bounds.rs
#[test]
fn test_theoretical_error_bounds() {
    let test_matrices = [
        ("identity", DMatrix::identity(5, 5), 1.0),
        ("scaled_identity", DMatrix::identity(5, 5) * 100.0, 1.0),
        ("diagonal", DMatrix::from_diagonal(&DVector::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0])), 5.0),
    ];

    for (name, matrix, true_cond) in test_matrices {
        let tolerance = 1e-12;
        let estimate = estimate_condition_number(&matrix, tolerance);

        // Theoretical error bound for power iteration
        let spectral_gap = estimate_spectral_gap(&matrix);
        let expected_error_bound = tolerance / spectral_gap;

        let actual_error = (estimate - true_cond).abs();

        assert!(
            actual_error <= expected_error_bound * 10.0, // Allow some factor of safety
            "Error bound violation for {}: actual={}, bound={}",
            name, actual_error, expected_error_bound
        );
    }
}

#[test]
fn test_perturbation_analysis() {
    let base_matrix = generate_spd_matrix(8, 1.0);
    let base_cond = estimate_condition_number(&base_matrix, 1e-12);

    let perturbation_sizes = [1e-10, 1e-8, 1e-6, 1e-4];

    for eps in perturbation_sizes {
        let perturbation = DMatrix::from_fn(8, 8, |_, _| {
            (rand::random::<f64>() - 0.5) * eps
        });

        let perturbed_matrix = &base_matrix + &perturbation;
        let perturbed_cond = estimate_condition_number(&perturbed_matrix, 1e-12);

        // Condition number perturbation bound
        let relative_change = (perturbed_cond - base_cond).abs() / base_cond;
        let perturbation_norm = perturbation.norm();
        let matrix_norm = base_matrix.norm();

        let theoretical_bound = base_cond * base_cond * perturbation_norm / matrix_norm;

        assert!(
            relative_change <= theoretical_bound * 2.0,
            "Perturbation bound violated: change={}, bound={}",
            relative_change, theoretical_bound
        );
    }
}
```

### 4.4 Precision Comparison (f32/f64)

```rust
// tests/numerical/precision_tests.rs
#[test]
fn test_precision_effects() {
    let test_matrices = generate_precision_test_suite();

    for (name, matrix_f64) in test_matrices {
        let matrix_f32 = matrix_f64.cast::<f32>();

        let result_f64 = estimate_condition_number(&matrix_f64, 1e-12);
        let result_f32 = estimate_condition_number_f32(&matrix_f32, 1e-6) as f64;

        let condition_number = result_f64;

        if condition_number < 1e6 {
            // For well-conditioned matrices, f32 should be close
            let relative_error = (result_f64 - result_f32).abs() / result_f64;
            assert!(
                relative_error < 0.01,
                "Test {}: f32/f64 mismatch for well-conditioned matrix: {} vs {}",
                name, result_f32, result_f64
            );
        } else {
            // For ill-conditioned matrices, f32 may saturate
            assert!(
                result_f32 >= result_f64 * 0.1,
                "Test {}: f32 result unreasonably small: {} vs {}",
                name, result_f32, result_f64
            );
        }

        println!("Test {}: f64={:.2e}, f32={:.2e}, ratio={:.2}",
                 name, result_f64, result_f32, result_f64 / result_f32);
    }
}

#[test]
fn test_mixed_precision_algorithms() {
    let matrix = generate_matrix_with_condition_number(20, 1e8);

    // Start with f32 for speed, refine with f64
    let rough_estimate_f32 = estimate_condition_number_f32(
        &matrix.cast::<f32>(), 1e-4
    ) as f64;

    let refined_estimate_f64 = refine_condition_number_estimate(
        &matrix,
        rough_estimate_f32,
        1e-10
    );

    let full_f64_estimate = estimate_condition_number(&matrix, 1e-10);

    // Mixed precision should be close to full f64
    let relative_error = (refined_estimate_f64 - full_f64_estimate).abs() / full_f64_estimate;
    assert!(
        relative_error < 0.05,
        "Mixed precision refinement failed: {} vs {}",
        refined_estimate_f64, full_f64_estimate
    );
}
```

### 4.5 Ill-Conditioned System Handling

```rust
// tests/numerical/ill_conditioned_tests.rs
#[test]
fn test_singular_and_near_singular_matrices() {
    // Exactly singular matrix
    let mut singular = DMatrix::zeros(5, 5);
    singular.set_diagonal(&DVector::from_vec(vec![1.0, 1.0, 1.0, 1.0, 0.0]));

    let result = estimate_condition_number(&singular, 1e-14);
    assert!(result.is_infinite() || result > 1e15);

    // Near-singular matrices
    let epsilons = [1e-15, 1e-12, 1e-9, 1e-6];

    for eps in epsilons {
        let mut near_singular = singular.clone();
        near_singular[(4, 4)] = eps;

        let cond = estimate_condition_number(&near_singular, 1e-14);
        let expected_cond = 1.0 / eps;

        let relative_error = (cond - expected_cond).abs() / expected_cond;
        assert!(
            relative_error < 0.1,
            "Near-singular test failed for eps={}: got {}, expected {}",
            eps, cond, expected_cond
        );
    }
}

#[test]
fn test_rank_deficient_handling() {
    // Create rank-deficient matrix
    let rank = 3;
    let size = 8;
    let a = DMatrix::from_fn(size, rank, |_, _| rand::random::<f64>());
    let rank_deficient = &a * a.transpose();

    let result = estimate_condition_number_with_rank_detection(&rank_deficient, 1e-12);

    match result {
        ConditionNumberResult::FullRank(cond) => {
            // Should detect as rank deficient, not full rank
            panic!("Failed to detect rank deficiency, got condition number: {}", cond);
        }
        ConditionNumberResult::RankDeficient { rank: detected_rank, .. } => {
            assert_eq!(detected_rank, rank);
        }
        ConditionNumberResult::Singular => {
            // Acceptable if numerical rank detection is conservative
        }
    }
}
```

## 5. Test Data Generation

### 5.1 Random Matrix Generators

```rust
// src/test_utils/matrix_generators.rs
use rand::prelude::*;
use nalgebra::*;

pub fn generate_spd_matrix(size: usize, condition_number: f64) -> DMatrix<f64> {
    let mut rng = thread_rng();

    // Generate random orthogonal matrix via QR decomposition
    let random_matrix = DMatrix::from_fn(size, size, |_, _| rng.gen_range(-1.0..1.0));
    let qr = random_matrix.qr();
    let q = qr.q();

    // Create diagonal matrix with specified condition number
    let eigenvalues: Vec<f64> = (0..size)
        .map(|i| {
            let t = i as f64 / (size - 1) as f64;
            1.0 + (condition_number - 1.0) * t
        })
        .collect();

    let d = DMatrix::from_diagonal(&DVector::from_vec(eigenvalues));

    // SPD matrix: Q * D * Q^T
    &q * &d * q.transpose()
}

pub fn generate_hilbert_matrix(size: usize) -> DMatrix<f64> {
    DMatrix::from_fn(size, size, |i, j| {
        1.0 / ((i + j + 1) as f64)
    })
}

pub fn generate_vandermonde_matrix(size: usize) -> DMatrix<f64> {
    let points: Vec<f64> = (0..size).map(|i| i as f64 / size as f64).collect();

    DMatrix::from_fn(size, size, |i, j| {
        points[i].powi(j as i32)
    })
}

pub fn generate_toeplitz_matrix(size: usize) -> DMatrix<f64> {
    let mut rng = thread_rng();
    let coeffs: Vec<f64> = (0..2*size-1).map(|_| rng.gen_range(-1.0..1.0)).collect();

    DMatrix::from_fn(size, size, |i, j| {
        coeffs[size - 1 + i - j]
    })
}

pub fn generate_sparse_matrix(size: usize, density: f64) -> CsMatrix<f64> {
    let mut rng = thread_rng();
    let mut triplets = Vec::new();

    for i in 0..size {
        for j in 0..size {
            if rng.gen::<f64>() < density {
                let value = rng.gen_range(-1.0..1.0);
                triplets.push((i, j, value));
            }
        }
    }

    CsMatrix::try_from_triplets(size, size, triplets).unwrap()
}

pub fn generate_graph_laplacian(vertices: usize, edges: &[(usize, usize, f64)]) -> DMatrix<f64> {
    let mut laplacian = DMatrix::zeros(vertices, vertices);

    for &(i, j, weight) in edges {
        if i != j {
            laplacian[(i, j)] -= weight;
            laplacian[(j, i)] -= weight;
            laplacian[(i, i)] += weight;
            laplacian[(j, j)] += weight;
        }
    }

    laplacian
}
```

### 5.2 Real-World Graph Datasets

```rust
// src/test_utils/graph_datasets.rs
pub struct GraphDataset {
    pub name: String,
    pub vertices: usize,
    pub edges: Vec<(usize, usize, f64)>,
    pub properties: GraphProperties,
}

pub struct GraphProperties {
    pub is_connected: bool,
    pub is_bipartite: bool,
    pub average_degree: f64,
    pub clustering_coefficient: f64,
    pub diameter: Option<usize>,
}

pub fn load_benchmark_graphs() -> Vec<GraphDataset> {
    vec![
        load_karate_club_graph(),
        load_erdos_renyi_graph(1000, 0.01),
        load_barabasi_albert_graph(1000, 5),
        load_watts_strogatz_graph(1000, 6, 0.1),
        load_grid_graph(32, 32),
        load_complete_graph(50),
        load_star_graph(1000),
        load_path_graph(1000),
    ]
}

fn load_karate_club_graph() -> GraphDataset {
    // Zachary's Karate Club - famous small social network
    let edges = vec![
        (0, 1, 1.0), (0, 2, 1.0), (0, 3, 1.0), // ... complete edge list
        // 78 edges total in the karate club graph
    ];

    GraphDataset {
        name: "zachary_karate".to_string(),
        vertices: 34,
        edges,
        properties: GraphProperties {
            is_connected: true,
            is_bipartite: false,
            average_degree: 4.59,
            clustering_coefficient: 0.571,
            diameter: Some(5),
        }
    }
}

fn load_erdos_renyi_graph(n: usize, p: f64) -> GraphDataset {
    let mut rng = thread_rng();
    let mut edges = Vec::new();

    for i in 0..n {
        for j in i+1..n {
            if rng.gen::<f64>() < p {
                edges.push((i, j, 1.0));
            }
        }
    }

    GraphDataset {
        name: format!("erdos_renyi_{}_{}", n, p),
        vertices: n,
        edges,
        properties: estimate_graph_properties(n, &edges),
    }
}

pub fn load_matrix_market_graphs() -> Result<Vec<GraphDataset>, Box<dyn std::error::Error>> {
    // Load standard test matrices from Matrix Market
    let matrices = [
        "bcsstk01.mtx",
        "can_24.mtx",
        "cavity01.mtx",
        "dwt_59.mtx",
    ];

    let mut datasets = Vec::new();

    for matrix_name in matrices {
        let path = format!("test_data/matrix_market/{}", matrix_name);
        if std::path::Path::new(&path).exists() {
            let graph = load_matrix_market_file(&path)?;
            datasets.push(graph);
        }
    }

    Ok(datasets)
}
```

### 5.3 Pathological Cases

```rust
// src/test_utils/pathological_cases.rs
pub fn generate_pathological_test_suite() -> Vec<(&'static str, DMatrix<f64>)> {
    vec![
        ("grcar", generate_grcar_matrix(100)),
        ("wilkinson", generate_wilkinson_matrix(21)),
        ("clement", generate_clement_matrix(50)),
        ("kahan", generate_kahan_matrix(20, 0.1)),
        ("prolate", generate_prolate_matrix(30, 8.0)),
        ("rosser", generate_rosser_matrix()),
        ("frank", generate_frank_matrix(50)),
        ("lotkin", generate_lotkin_matrix(30)),
    ]
}

fn generate_grcar_matrix(n: usize) -> DMatrix<f64> {
    // Grcar matrix - Toeplitz matrix with specific structure
    let mut matrix = DMatrix::zeros(n, n);

    for i in 0..n {
        for j in 0..n {
            if j == i {
                matrix[(i, j)] = 1.0;
            } else if j == i + 1 && j < n {
                matrix[(i, j)] = 1.0;
            } else if j == i + 2 && j < n {
                matrix[(i, j)] = 1.0;
            } else if j == i + 3 && j < n {
                matrix[(i, j)] = 1.0;
            } else if i == j + 1 {
                matrix[(i, j)] = -1.0;
            }
        }
    }

    matrix
}

fn generate_wilkinson_matrix(n: usize) -> DMatrix<f64> {
    // Wilkinson matrix - symmetric tridiagonal
    let mut matrix = DMatrix::zeros(n, n);
    let center = n / 2;

    for i in 0..n {
        matrix[(i, i)] = (i as i32 - center as i32).abs() as f64;
        if i > 0 {
            matrix[(i, i-1)] = 1.0;
            matrix[(i-1, i)] = 1.0;
        }
    }

    matrix
}

fn generate_kahan_matrix(n: usize, theta: f64) -> DMatrix<f64> {
    // Kahan matrix - upper triangular with specific structure
    let s = theta.sin();
    let c = theta.cos();

    let mut matrix = DMatrix::zeros(n, n);

    for i in 0..n {
        for j in i..n {
            if i == j {
                matrix[(i, j)] = s.powi((i + 1) as i32);
            } else {
                matrix[(i, j)] = -c * s.powi(i as i32);
            }
        }
    }

    matrix
}

pub fn generate_adversarial_streaming_cases() -> Vec<Vec<(usize, usize, f64)>> {
    vec![
        generate_star_bursts(1000, 10),
        generate_delayed_connections(1000),
        generate_oscillating_weights(500),
        generate_heavy_tailed_degrees(1000),
    ]
}

fn generate_star_bursts(n: usize, num_bursts: usize) -> Vec<(usize, usize, f64)> {
    // Create multiple star subgraphs that connect late
    let mut edges = Vec::new();
    let nodes_per_star = n / num_bursts;

    for burst in 0..num_bursts {
        let center = burst * nodes_per_star;
        for i in 1..nodes_per_star {
            edges.push((center, center + i, 1.0));
        }
    }

    // Connect stars at the end
    for burst in 1..num_bursts {
        edges.push((0, burst * nodes_per_star, 1.0));
    }

    edges
}
```

### 5.4 Benchmark Problems from Literature

```rust
// src/test_utils/literature_benchmarks.rs
pub struct BenchmarkProblem {
    pub name: String,
    pub matrix: DMatrix<f64>,
    pub known_condition_number: Option<f64>,
    pub known_eigenvalues: Option<Vec<f64>>,
    pub source: String,
    pub notes: String,
}

pub fn load_literature_benchmarks() -> Vec<BenchmarkProblem> {
    vec![
        create_higham_test_matrices(),
        create_stewart_test_matrices(),
        create_golub_test_matrices(),
        create_moler_test_matrices(),
    ].into_iter().flatten().collect()
}

fn create_higham_test_matrices() -> Vec<BenchmarkProblem> {
    // From Higham's "Accuracy and Stability of Numerical Algorithms"
    vec![
        BenchmarkProblem {
            name: "higham_2_1".to_string(),
            matrix: {
                let mut m = DMatrix::identity(3, 3);
                m[(0, 1)] = -1.0;
                m[(1, 2)] = -1.0;
                m
            },
            known_condition_number: Some(3.0 + 2.0 * 2.0_f64.sqrt()),
            known_eigenvalues: None,
            source: "Higham, ASNA, 2nd ed., Example 2.1".to_string(),
            notes: "Simple example with known condition number".to_string(),
        },

        BenchmarkProblem {
            name: "higham_gallery_prolate".to_string(),
            matrix: generate_prolate_matrix(16, 4.0),
            known_condition_number: None,
            known_eigenvalues: None,
            source: "Higham, Test Matrix Toolbox, prolate".to_string(),
            notes: "Prolate matrix with parameter W=4".to_string(),
        },
    ]
}

fn create_golub_test_matrices() -> Vec<BenchmarkProblem> {
    // From Golub & Van Loan, "Matrix Computations"
    vec![
        BenchmarkProblem {
            name: "golub_discrete_laplacian_1d".to_string(),
            matrix: generate_1d_discrete_laplacian(50),
            known_condition_number: Some(4.0 * (50.0 + 1.0).powi(2) / std::f64::consts::PI.powi(2)),
            known_eigenvalues: Some(
                (1..=50).map(|k| {
                    4.0 * (k as f64 * std::f64::consts::PI / 51.0 / 2.0).sin().powi(2)
                }).collect()
            ),
            source: "Golub & Van Loan, Matrix Computations, Ch. 4".to_string(),
            notes: "1D discrete Laplacian, condition number grows as O(n^2)".to_string(),
        }
    ]
}

fn generate_1d_discrete_laplacian(n: usize) -> DMatrix<f64> {
    let mut matrix = DMatrix::zeros(n, n);
    let h = 1.0 / (n + 1) as f64;

    for i in 0..n {
        matrix[(i, i)] = 2.0 / h.powi(2);
        if i > 0 {
            matrix[(i, i-1)] = -1.0 / h.powi(2);
        }
        if i < n - 1 {
            matrix[(i, i+1)] = -1.0 / h.powi(2);
        }
    }

    matrix
}
```

### 5.5 Incremental Update Scenarios

```rust
// src/test_utils/incremental_scenarios.rs
pub struct IncrementalScenario {
    pub name: String,
    pub initial_graph: Vec<(usize, usize, f64)>,
    pub updates: Vec<GraphUpdate>,
    pub expected_spectral_changes: Vec<f64>,
}

pub enum GraphUpdate {
    AddEdge(usize, usize, f64),
    RemoveEdge(usize, usize),
    ModifyWeight(usize, usize, f64),
    AddVertex(Vec<(usize, f64)>), // Connections to new vertex
    RemoveVertex(usize),
}

pub fn generate_incremental_test_scenarios() -> Vec<IncrementalScenario> {
    vec![
        path_to_cycle_scenario(),
        star_to_clique_scenario(),
        weight_perturbation_scenario(),
        vertex_insertion_scenario(),
        bridge_addition_scenario(),
    ]
}

fn path_to_cycle_scenario() -> IncrementalScenario {
    let n = 100;
    let initial_graph: Vec<_> = (0..n-1)
        .map(|i| (i, i+1, 1.0))
        .collect();

    IncrementalScenario {
        name: "path_to_cycle".to_string(),
        initial_graph,
        updates: vec![
            GraphUpdate::AddEdge(n-1, 0, 1.0), // Close the cycle
        ],
        expected_spectral_changes: vec![
            2.0, // Spectral radius should jump from ~2 to 2
        ],
    }
}

fn weight_perturbation_scenario() -> IncrementalScenario {
    let base_graph = generate_grid_graph(10, 10);
    let perturbations = vec![0.1, 0.5, 1.0, 2.0, 10.0];

    let updates: Vec<_> = perturbations
        .into_iter()
        .map(|weight| GraphUpdate::ModifyWeight(0, 1, weight))
        .collect();

    IncrementalScenario {
        name: "weight_perturbation".to_string(),
        initial_graph: base_graph,
        updates,
        expected_spectral_changes: vec![4.1, 4.5, 5.0, 6.0, 14.0],
    }
}

pub fn test_incremental_accuracy() {
    for scenario in generate_incremental_test_scenarios() {
        let mut current_graph = scenario.initial_graph.clone();
        let mut current_spectral_radius = compute_spectral_radius_batch(&edges_to_graph(&current_graph));

        for (update, expected_change) in scenario.updates.iter().zip(scenario.expected_spectral_changes.iter()) {
            apply_graph_update(&mut current_graph, update);

            // Incremental update
            let incremental_radius = update_spectral_radius_incremental(
                current_spectral_radius,
                update
            );

            // Batch recomputation
            let batch_radius = compute_spectral_radius_batch(&edges_to_graph(&current_graph));

            // Verify incremental vs batch accuracy
            let error = (incremental_radius - batch_radius).abs() / batch_radius;
            assert!(
                error < 0.01,
                "Incremental update error too large: {} vs {} ({})",
                incremental_radius, batch_radius, error
            );

            current_spectral_radius = batch_radius;
        }
    }
}
```

## 6. CI/CD Testing Pipeline

### 6.1 GitHub Actions Workflow

```yaml
# .github/workflows/test.yml
name: Comprehensive Testing

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  rust-tests:
    name: Rust Tests
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
        rust: [stable, beta, nightly]
        exclude:
          - os: windows-latest
            rust: nightly
          - os: macos-latest
            rust: beta

    steps:
    - uses: actions/checkout@v3

    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: ${{ matrix.rust }}
        override: true
        components: rustfmt, clippy

    - name: Cache dependencies
      uses: actions/cache@v3
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          target
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

    - name: Check formatting
      run: cargo fmt --all -- --check

    - name: Clippy lints
      run: cargo clippy --all-targets --all-features -- -D warnings

    - name: Unit tests
      run: cargo test --lib --bins

    - name: Integration tests
      run: cargo test --test '*'

    - name: Doc tests
      run: cargo test --doc

  wasm-tests:
    name: WASM Tests
    runs-on: ubuntu-latest

    steps:
    - uses: actions/checkout@v3

    - name: Install Rust and wasm-pack
      run: |
        curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
        rustup target add wasm32-unknown-unknown

    - name: Build WASM package
      run: wasm-pack build --target web --out-dir pkg

    - name: Setup Node.js
      uses: actions/setup-node@v3
      with:
        node-version: '18'

    - name: Install npm dependencies
      run: npm install

    - name: WASM tests
      run: |
        npm run test:wasm
        npm run test:wasm:node

    - name: Bundle size check
      run: |
        npm run build:wasm
        ls -la pkg/
        # Fail if bundle > 2MB
        [ $(wc -c < pkg/sublinear_time_solver_bg.wasm) -lt 2097152 ]

  performance-tests:
    name: Performance Benchmarks
    runs-on: ubuntu-latest

    steps:
    - uses: actions/checkout@v3

    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true

    - name: Run benchmarks
      run: |
        cargo bench --bench condition_number_benchmarks -- --output-format bencher | tee benchmark_output.txt
        cargo bench --bench streaming_benchmarks -- --output-format bencher | tee -a benchmark_output.txt

    - name: Performance regression check
      run: |
        python scripts/check_performance_regression.py benchmark_output.txt

    - name: Archive benchmark results
      uses: actions/upload-artifact@v3
      with:
        name: benchmark-results
        path: benchmark_output.txt

  numerical-accuracy:
    name: Numerical Accuracy Tests
    runs-on: ubuntu-latest

    steps:
    - uses: actions/checkout@v3

    - name: Install dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y libopenblas-dev liblapack-dev

    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true

    - name: Run numerical accuracy tests
      run: cargo test --release numerical_accuracy -- --nocapture

    - name: Run condition number stress tests
      run: cargo test --release condition_number_stress --features stress-tests

    - name: Generate accuracy report
      run: |
        cargo run --bin accuracy_analyzer > accuracy_report.txt

    - name: Upload accuracy report
      uses: actions/upload-artifact@v3
      with:
        name: accuracy-report
        path: accuracy_report.txt

  coverage:
    name: Code Coverage
    runs-on: ubuntu-latest

    steps:
    - uses: actions/checkout@v3

    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true
        components: llvm-tools-preview

    - name: Install grcov
      run: cargo install grcov

    - name: Run tests with coverage
      env:
        RUSTFLAGS: '-Cinstrument-coverage'
        LLVM_PROFILE_FILE: 'sublinear-solver-%p-%m.profraw'
      run: |
        cargo test --all-features
        cargo test --release --all-features

    - name: Generate coverage report
      run: |
        grcov . --binary-path ./target/debug/ -s . -t lcov --branch --ignore-not-existing --ignore "/*" -o lcov.info

    - name: Upload coverage to Codecov
      uses: codecov/codecov-action@v3
      with:
        files: lcov.info
        fail_ci_if_error: true

  integration-matrix:
    name: Integration Test Matrix
    runs-on: ubuntu-latest
    strategy:
      matrix:
        test-suite:
          - algorithm_comparison
          - streaming_integration
          - memory_management
          - cross_platform

    steps:
    - uses: actions/checkout@v3

    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true

    - name: Run integration test suite
      run: cargo test --test ${{ matrix.test-suite }} --release -- --nocapture

    - name: Collect test artifacts
      if: failure()
      run: |
        mkdir -p test-artifacts/${{ matrix.test-suite }}
        cp -r target/debug/deps/test_* test-artifacts/${{ matrix.test-suite }}/ || true

    - name: Upload test artifacts
      if: failure()
      uses: actions/upload-artifact@v3
      with:
        name: test-artifacts-${{ matrix.test-suite }}
        path: test-artifacts/

  documentation:
    name: Documentation Build
    runs-on: ubuntu-latest

    steps:
    - uses: actions/checkout@v3

    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true

    - name: Build documentation
      run: |
        cargo doc --no-deps --all-features
        cargo test --doc --all-features

    - name: Check for broken links
      run: |
        cargo install cargo-deadlinks
        cargo deadlinks --check-http

    - name: Deploy documentation
      if: github.ref == 'refs/heads/main'
      run: |
        echo '<meta http-equiv="refresh" content="0; url=sublinear_time_solver">' > target/doc/index.html
        # Deploy to GitHub Pages or documentation hosting
```

### 6.2 Performance Regression Detection

```python
# scripts/check_performance_regression.py
import sys
import re
import json
from typing import Dict, List, Tuple

def parse_benchmark_output(filename: str) -> Dict[str, float]:
    """Parse criterion benchmark output."""
    results = {}

    with open(filename, 'r') as f:
        content = f.read()

    # Parse criterion output format
    pattern = r'(\w+(?:_\w+)*)\s+time:\s+\[([0-9.]+)\s+([a-z]+)\s+([0-9.]+)\s+([a-z]+)\s+([0-9.]+)\s+([a-z]+)\]'

    for match in re.finditer(pattern, content):
        test_name = match.group(1)
        # Use the middle estimate (group 4, 5)
        time_value = float(match.group(4))
        time_unit = match.group(5)

        # Convert to nanoseconds
        multipliers = {
            'ns': 1,
            'us': 1000,
            'ms': 1000000,
            's': 1000000000
        }

        time_ns = time_value * multipliers.get(time_unit, 1)
        results[test_name] = time_ns

    return results

def load_baseline_performance() -> Dict[str, float]:
    """Load baseline performance from previous runs."""
    try:
        with open('baseline_performance.json', 'r') as f:
            return json.load(f)
    except FileNotFoundError:
        return {}

def check_regressions(current: Dict[str, float], baseline: Dict[str, float]) -> List[Tuple[str, float, float, float]]:
    """Check for performance regressions."""
    regressions = []

    for test_name, current_time in current.items():
        if test_name in baseline:
            baseline_time = baseline[test_name]
            regression_ratio = current_time / baseline_time

            # Flag if >10% slower
            if regression_ratio > 1.1:
                regressions.append((test_name, baseline_time, current_time, regression_ratio))

    return regressions

def main():
    if len(sys.argv) != 2:
        print("Usage: python check_performance_regression.py <benchmark_output.txt>")
        sys.exit(1)

    current_results = parse_benchmark_output(sys.argv[1])
    baseline_results = load_baseline_performance()

    regressions = check_regressions(current_results, baseline_results)

    if regressions:
        print("⚠️  Performance regressions detected:")
        for test_name, baseline, current, ratio in regressions:
            print(f"  {test_name}: {baseline:.0f}ns → {current:.0f}ns ({ratio:.2f}x slower)")

        # Fail the CI if regressions are too severe
        max_regression = max(ratio for _, _, _, ratio in regressions)
        if max_regression > 2.0:  # 2x slower
            print("❌ Severe performance regression detected!")
            sys.exit(1)
        else:
            print("⚠️  Minor performance regression - review recommended")
    else:
        print("✅ No performance regressions detected")

    # Update baseline with current results
    with open('baseline_performance.json', 'w') as f:
        json.dump(current_results, f, indent=2)

if __name__ == "__main__":
    main()
```

### 6.3 Test Data Validation Pipeline

```rust
// src/bin/test_data_validator.rs
use std::path::Path;
use sublinear_time_solver::test_utils::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Validating test data integrity...");

    validate_matrix_market_files()?;
    validate_benchmark_datasets()?;
    validate_pathological_cases()?;
    validate_streaming_scenarios()?;

    println!("✅ All test data validation passed!");
    Ok(())
}

fn validate_matrix_market_files() -> Result<(), Box<dyn std::error::Error>> {
    let test_data_dir = Path::new("test_data/matrix_market");

    if !test_data_dir.exists() {
        println!("⚠️  Matrix Market test data not found - skipping");
        return Ok(());
    }

    for entry in std::fs::read_dir(test_data_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "mtx") {
            println!("Validating {:?}...", path.file_name().unwrap());

            let matrix = load_matrix_market_file(&path)?;

            // Basic validation
            assert!(matrix.nrows() > 0, "Matrix has zero rows");
            assert!(matrix.ncols() > 0, "Matrix has zero columns");

            // Check for NaN/infinity
            for &value in matrix.iter() {
                assert!(value.is_finite(), "Matrix contains non-finite values");
            }

            // Verify symmetry for symmetric matrices
            if path.file_name().unwrap().to_str().unwrap().contains("sym") {
                assert!(is_symmetric(&matrix, 1e-12), "Matrix marked as symmetric but isn't");
            }
        }
    }

    Ok(())
}

fn validate_benchmark_datasets() -> Result<(), Box<dyn std::error::Error>> {
    let benchmarks = load_literature_benchmarks();

    for benchmark in benchmarks {
        println!("Validating benchmark: {}", benchmark.name);

        // Verify matrix properties
        let matrix = &benchmark.matrix;
        assert!(matrix.nrows() == matrix.ncols(), "Matrix not square");

        // Check condition number bounds if known
        if let Some(known_cond) = benchmark.known_condition_number {
            let estimated_cond = estimate_condition_number(matrix, 1e-12);
            let relative_error = (estimated_cond - known_cond).abs() / known_cond;

            assert!(
                relative_error < 0.1,
                "Condition number estimate too far from known value: {} vs {}",
                estimated_cond, known_cond
            );
        }

        // Verify eigenvalues if known
        if let Some(ref eigenvalues) = benchmark.known_eigenvalues {
            let computed_eigenvalues = compute_eigenvalues_symmetric(matrix);

            assert_eq!(
                eigenvalues.len(),
                computed_eigenvalues.len(),
                "Eigenvalue count mismatch"
            );

            for (known, computed) in eigenvalues.iter().zip(computed_eigenvalues.iter()) {
                let error = (known - computed).abs();
                assert!(
                    error < 1e-6,
                    "Eigenvalue mismatch: {} vs {}",
                    known, computed
                );
            }
        }
    }

    Ok(())
}

fn validate_pathological_cases() -> Result<(), Box<dyn std::error::Error>> {
    let pathological_matrices = generate_pathological_test_suite();

    for (name, matrix) in pathological_matrices {
        println!("Validating pathological case: {}", name);

        // These should not crash or produce NaN
        let result = std::panic::catch_unwind(|| {
            estimate_condition_number(&matrix, 1e-10)
        });

        match result {
            Ok(cond) => {
                assert!(
                    cond >= 1.0 || cond.is_infinite(),
                    "Invalid condition number: {}",
                    cond
                );
            }
            Err(_) => {
                eprintln!("⚠️  Pathological case {} caused panic", name);
            }
        }
    }

    Ok(())
}

fn validate_streaming_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    let scenarios = generate_incremental_test_scenarios();

    for scenario in scenarios {
        println!("Validating streaming scenario: {}", scenario.name);

        // Verify initial graph is valid
        let initial_graph = edges_to_adjacency_matrix(&scenario.initial_graph);
        assert!(initial_graph.nrows() > 0, "Empty initial graph");

        // Verify updates are valid
        for update in &scenario.updates {
            match update {
                GraphUpdate::AddEdge(i, j, weight) => {
                    assert!(weight.is_finite(), "Invalid edge weight");
                    assert!(i != j, "Self-loops not supported");
                }
                GraphUpdate::ModifyWeight(i, j, weight) => {
                    assert!(weight.is_finite(), "Invalid weight modification");
                }
                _ => {} // Other updates are structural
            }
        }
    }

    Ok(())
}
```

## 7. Verification & Validation

### 7.1 Cross-Validation Framework

```rust
// src/validation/cross_validation.rs
use std::collections::HashMap;

pub struct ValidationSuite {
    reference_solvers: HashMap<String, Box<dyn ConditionNumberSolver>>,
    test_matrices: Vec<TestMatrix>,
    tolerance_levels: Vec<f64>,
}

pub trait ConditionNumberSolver {
    fn estimate_condition_number(&self, matrix: &DMatrix<f64>, tolerance: f64) -> f64;
    fn name(&self) -> &str;
}

pub struct PowerIterationSolver;
pub struct LanczosSolver;
pub struct DirectSVDSolver;
pub struct ExternalSolver { command: String }

impl ConditionNumberSolver for DirectSVDSolver {
    fn estimate_condition_number(&self, matrix: &DMatrix<f64>, _tolerance: f64) -> f64 {
        let svd = matrix.svd(true, true);
        let max_sv = svd.singular_values[0];
        let min_sv = svd.singular_values[svd.singular_values.len() - 1];
        max_sv / min_sv
    }

    fn name(&self) -> &str { "DirectSVD" }
}

impl ConditionNumberSolver for ExternalSolver {
    fn estimate_condition_number(&self, matrix: &DMatrix<f64>, tolerance: f64) -> f64 {
        // Call external solver (MATLAB, Octave, etc.)
        let temp_file = format!("/tmp/matrix_{}.txt", rand::random::<u32>());
        save_matrix_to_file(matrix, &temp_file).unwrap();

        let output = std::process::Command::new(&self.command)
            .arg(&temp_file)
            .arg(&tolerance.to_string())
            .output()
            .expect("Failed to run external solver");

        let result: f64 = String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();

        std::fs::remove_file(temp_file).ok();
        result
    }

    fn name(&self) -> &str { &self.command }
}

impl ValidationSuite {
    pub fn new() -> Self {
        let mut reference_solvers: HashMap<String, Box<dyn ConditionNumberSolver>> = HashMap::new();

        reference_solvers.insert("power_iteration".to_string(), Box::new(PowerIterationSolver));
        reference_solvers.insert("lanczos".to_string(), Box::new(LanczosSolver));
        reference_solvers.insert("direct_svd".to_string(), Box::new(DirectSVDSolver));

        // Add external solvers if available
        if which::which("octave").is_ok() {
            reference_solvers.insert(
                "octave".to_string(),
                Box::new(ExternalSolver {
                    command: "octave --eval 'cond(load(\"-ascii\", argv(){1}))'".to_string()
                })
            );
        }

        Self {
            reference_solvers,
            test_matrices: load_validation_test_matrices(),
            tolerance_levels: vec![1e-6, 1e-8, 1e-10, 1e-12],
        }
    }

    pub fn run_cross_validation(&self) -> ValidationReport {
        let mut results = HashMap::new();

        for test_matrix in &self.test_matrices {
            let mut matrix_results = HashMap::new();

            for tolerance in &self.tolerance_levels {
                let mut solver_results = HashMap::new();

                for (solver_name, solver) in &self.reference_solvers {
                    let start_time = std::time::Instant::now();

                    let result = std::panic::catch_unwind(|| {
                        solver.estimate_condition_number(&test_matrix.matrix, *tolerance)
                    });

                    let duration = start_time.elapsed();

                    match result {
                        Ok(cond_num) => {
                            solver_results.insert(solver_name.clone(), SolverResult {
                                condition_number: cond_num,
                                duration,
                                success: true,
                                error: None,
                            });
                        }
                        Err(panic_info) => {
                            solver_results.insert(solver_name.clone(), SolverResult {
                                condition_number: f64::NAN,
                                duration,
                                success: false,
                                error: Some(format!("{:?}", panic_info)),
                            });
                        }
                    }
                }

                matrix_results.insert(*tolerance, solver_results);
            }

            results.insert(test_matrix.name.clone(), matrix_results);
        }

        ValidationReport { results }
    }
}

pub struct ValidationReport {
    results: HashMap<String, HashMap<f64, HashMap<String, SolverResult>>>,
}

impl ValidationReport {
    pub fn analyze_consistency(&self) -> ConsistencyAnalysis {
        let mut inconsistencies = Vec::new();
        let mut performance_comparison = HashMap::new();

        for (matrix_name, tolerance_results) in &self.results {
            for (tolerance, solver_results) in tolerance_results {
                let successful_results: Vec<_> = solver_results
                    .iter()
                    .filter(|(_, result)| result.success)
                    .collect();

                if successful_results.len() < 2 {
                    continue; // Need at least 2 successful results to compare
                }

                // Check pairwise consistency
                for i in 0..successful_results.len() {
                    for j in i+1..successful_results.len() {
                        let (name1, result1) = successful_results[i];
                        let (name2, result2) = successful_results[j];

                        let relative_diff = (result1.condition_number - result2.condition_number).abs()
                            / result1.condition_number.min(result2.condition_number);

                        if relative_diff > 0.1 { // 10% threshold
                            inconsistencies.push(Inconsistency {
                                matrix: matrix_name.clone(),
                                tolerance: *tolerance,
                                solver1: name1.clone(),
                                solver2: name2.clone(),
                                value1: result1.condition_number,
                                value2: result2.condition_number,
                                relative_difference: relative_diff,
                            });
                        }
                    }
                }

                // Track performance
                for (solver_name, result) in successful_results {
                    performance_comparison
                        .entry(solver_name.clone())
                        .or_insert_with(Vec::new)
                        .push(result.duration);
                }
            }
        }

        ConsistencyAnalysis {
            inconsistencies,
            performance_comparison,
        }
    }

    pub fn generate_report(&self) -> String {
        let analysis = self.analyze_consistency();

        let mut report = String::new();
        report.push_str("# Cross-Validation Report\n\n");

        report.push_str(&format!("## Summary\n"));
        report.push_str(&format!("- Test matrices: {}\n", self.results.len()));
        report.push_str(&format!("- Inconsistencies found: {}\n", analysis.inconsistencies.len()));

        if !analysis.inconsistencies.is_empty() {
            report.push_str("\n## Inconsistencies\n\n");
            for inconsistency in &analysis.inconsistencies {
                report.push_str(&format!(
                    "- **{}** (tol={:.0e}): {} = {:.2e}, {} = {:.2e} (diff: {:.1%})\n",
                    inconsistency.matrix,
                    inconsistency.tolerance,
                    inconsistency.solver1,
                    inconsistency.value1,
                    inconsistency.solver2,
                    inconsistency.value2,
                    inconsistency.relative_difference
                ));
            }
        }

        report.push_str("\n## Performance Comparison\n\n");
        for (solver, durations) in &analysis.performance_comparison {
            let avg_duration = durations.iter().sum::<std::time::Duration>() / durations.len() as u32;
            report.push_str(&format!("- {}: {:.2}ms average\n", solver, avg_duration.as_secs_f64() * 1000.0));
        }

        report
    }
}
```

### 7.2 Stability Analysis Over Long Runs

```rust
// src/validation/stability_analysis.rs
pub struct StabilityTest {
    pub name: String,
    pub matrix: DMatrix<f64>,
    pub num_runs: usize,
    pub duration: std::time::Duration,
}

pub fn run_stability_analysis() -> StabilityReport {
    let tests = vec![
        StabilityTest {
            name: "moderate_condition".to_string(),
            matrix: generate_spd_matrix(50, 1e6),
            num_runs: 1000,
            duration: std::time::Duration::from_secs(60),
        },
        StabilityTest {
            name: "ill_conditioned".to_string(),
            matrix: generate_hilbert_matrix(10),
            num_runs: 500,
            duration: std::time::Duration::from_secs(120),
        },
        StabilityTest {
            name: "streaming_convergence".to_string(),
            matrix: generate_large_sparse_matrix(1000, 0.01),
            num_runs: 100,
            duration: std::time::Duration::from_secs(300),
        },
    ];

    let mut results = HashMap::new();

    for test in tests {
        println!("Running stability test: {}", test.name);
        let test_result = run_single_stability_test(&test);
        results.insert(test.name.clone(), test_result);
    }

    StabilityReport { results }
}

fn run_single_stability_test(test: &StabilityTest) -> StabilityResult {
    let mut estimates = Vec::new();
    let mut durations = Vec::new();
    let mut memory_usage = Vec::new();

    let start_time = std::time::Instant::now();
    let mut run_count = 0;

    while start_time.elapsed() < test.duration && run_count < test.num_runs {
        let memory_before = get_memory_usage();
        let run_start = std::time::Instant::now();

        let estimate = estimate_condition_number(&test.matrix, 1e-10);

        let run_duration = run_start.elapsed();
        let memory_after = get_memory_usage();

        estimates.push(estimate);
        durations.push(run_duration);
        memory_usage.push(memory_after - memory_before);

        run_count += 1;

        // Periodic garbage collection hint
        if run_count % 100 == 0 {
            std::hint::black_box(&test.matrix);
        }
    }

    let mean_estimate = estimates.iter().sum::<f64>() / estimates.len() as f64;
    let variance = estimates
        .iter()
        .map(|x| (x - mean_estimate).powi(2))
        .sum::<f64>() / estimates.len() as f64;
    let std_dev = variance.sqrt();

    let mean_duration = durations.iter().sum::<std::time::Duration>() / durations.len() as u32;
    let max_duration = *durations.iter().max().unwrap();
    let min_duration = *durations.iter().min().unwrap();

    let mean_memory = memory_usage.iter().sum::<i64>() / memory_usage.len() as i64;
    let max_memory = *memory_usage.iter().max().unwrap();

    // Check for trends over time
    let trend_analysis = analyze_trends(&estimates, &durations, &memory_usage);

    StabilityResult {
        num_runs: run_count,
        mean_estimate,
        std_dev_estimate: std_dev,
        min_estimate: estimates.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
        max_estimate: estimates.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
        mean_duration,
        min_duration,
        max_duration,
        mean_memory_usage: mean_memory,
        max_memory_usage: max_memory,
        trend_analysis,
    }
}

fn analyze_trends(
    estimates: &[f64],
    durations: &[std::time::Duration],
    memory_usage: &[i64]
) -> TrendAnalysis {
    // Simple linear regression to detect trends
    let n = estimates.len() as f64;
    let x_mean = (n - 1.0) / 2.0; // Time points 0, 1, 2, ...

    // Trend in estimates
    let y_mean = estimates.iter().sum::<f64>() / n;
    let estimate_slope = estimates
        .iter()
        .enumerate()
        .map(|(i, &y)| (i as f64 - x_mean) * (y - y_mean))
        .sum::<f64>() / estimates
        .iter()
        .enumerate()
        .map(|(i, _)| (i as f64 - x_mean).powi(2))
        .sum::<f64>();

    // Trend in performance
    let duration_values: Vec<f64> = durations.iter().map(|d| d.as_secs_f64()).collect();
    let duration_mean = duration_values.iter().sum::<f64>() / n;
    let duration_slope = duration_values
        .iter()
        .enumerate()
        .map(|(i, &y)| (i as f64 - x_mean) * (y - duration_mean))
        .sum::<f64>() / duration_values
        .iter()
        .enumerate()
        .map(|(i, _)| (i as f64 - x_mean).powi(2))
        .sum::<f64>();

    // Trend in memory usage
    let memory_mean = memory_usage.iter().sum::<i64>() as f64 / n;
    let memory_slope = memory_usage
        .iter()
        .enumerate()
        .map(|(i, &y)| (i as f64 - x_mean) * (y as f64 - memory_mean))
        .sum::<f64>() / memory_usage
        .iter()
        .enumerate()
        .map(|(i, _)| (i as f64 - x_mean).powi(2))
        .sum::<f64>();

    TrendAnalysis {
        estimate_drift: estimate_slope,
        performance_degradation: duration_slope,
        memory_leak_rate: memory_slope,
    }
}
```

### 7.3 Memory Leak Detection

```rust
// src/validation/memory_leak_detection.rs
use jemalloc_ctl::{epoch, stats};

pub fn run_memory_leak_detection() -> MemoryLeakReport {
    let test_scenarios = vec![
        ("repeated_condition_number", test_repeated_condition_number),
        ("streaming_processing", test_streaming_memory_leaks),
        ("matrix_generation", test_matrix_generation_leaks),
        ("wasm_integration", test_wasm_memory_leaks),
    ];

    let mut results = HashMap::new();

    for (name, test_fn) in test_scenarios {
        println!("Running memory leak test: {}", name);
        let result = run_memory_test(test_fn);
        results.insert(name.to_string(), result);
    }

    MemoryLeakReport { results }
}

fn run_memory_test<F>(test_fn: F) -> MemoryTestResult
where
    F: Fn() -> ()
{
    // Force epoch advance and get initial memory stats
    epoch::advance().unwrap();
    let initial_allocated = stats::allocated::read().unwrap();
    let initial_resident = stats::resident::read().unwrap();

    // Warm up
    for _ in 0..10 {
        test_fn();
    }

    // Force garbage collection
    epoch::advance().unwrap();
    let warmup_allocated = stats::allocated::read().unwrap();
    let warmup_resident = stats::resident::read().unwrap();

    // Main test run
    let num_iterations = 1000;
    let checkpoint_interval = 100;
    let mut memory_samples = Vec::new();

    for i in 0..num_iterations {
        test_fn();

        if i % checkpoint_interval == 0 {
            epoch::advance().unwrap();
            let allocated = stats::allocated::read().unwrap();
            let resident = stats::resident::read().unwrap();

            memory_samples.push(MemorySample {
                iteration: i,
                allocated_bytes: allocated,
                resident_bytes: resident,
            });
        }
    }

    // Final measurement
    epoch::advance().unwrap();
    let final_allocated = stats::allocated::read().unwrap();
    let final_resident = stats::resident::read().unwrap();

    // Analysis
    let net_allocated_growth = final_allocated as i64 - warmup_allocated as i64;
    let net_resident_growth = final_resident as i64 - warmup_resident as i64;

    // Check for linear growth (indicating leaks)
    let leak_detected = detect_memory_leak_pattern(&memory_samples);

    MemoryTestResult {
        initial_allocated,
        final_allocated,
        net_allocated_growth,
        net_resident_growth,
        memory_samples,
        leak_detected,
        max_memory_usage: memory_samples.iter().map(|s| s.resident_bytes).max().unwrap_or(0),
    }
}

fn test_repeated_condition_number() {
    let matrix = generate_spd_matrix(100, 1e6);
    let _result = estimate_condition_number(&matrix, 1e-10);
}

fn test_streaming_memory_leaks() {
    let edges = generate_random_edges(1000, 0.01);
    let mut processor = StreamingSpectralProcessor::new();

    for edge in edges {
        processor.add_edge(edge);
    }

    let _result = processor.get_current_estimate();
}

fn test_matrix_generation_leaks() {
    let _matrix = generate_sparse_matrix(500, 0.05);
}

#[cfg(target_arch = "wasm32")]
fn test_wasm_memory_leaks() {
    use wasm_bindgen_test::*;

    let matrix_data = vec![1.0; 10000]; // 100x100 matrix
    let _result = estimate_condition_number_wasm(&matrix_data, 100, 100, 1e-8);
}

#[cfg(not(target_arch = "wasm32"))]
fn test_wasm_memory_leaks() {
    // Skip on non-WASM platforms
}

fn detect_memory_leak_pattern(samples: &[MemorySample]) -> bool {
    if samples.len() < 3 {
        return false;
    }

    // Linear regression on memory usage vs iteration
    let n = samples.len() as f64;
    let x_mean = samples.iter().map(|s| s.iteration as f64).sum::<f64>() / n;
    let y_mean = samples.iter().map(|s| s.allocated_bytes as f64).sum::<f64>() / n;

    let slope = samples
        .iter()
        .map(|s| (s.iteration as f64 - x_mean) * (s.allocated_bytes as f64 - y_mean))
        .sum::<f64>() / samples
        .iter()
        .map(|s| (s.iteration as f64 - x_mean).powi(2))
        .sum::<f64>();

    // Significant positive slope indicates potential leak
    // Threshold: >1KB per 100 iterations
    slope > 1024.0 / 100.0
}

pub struct MemoryLeakReport {
    results: HashMap<String, MemoryTestResult>,
}

impl MemoryLeakReport {
    pub fn has_leaks(&self) -> bool {
        self.results.values().any(|result| result.leak_detected)
    }

    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("# Memory Leak Detection Report\n\n");

        let total_tests = self.results.len();
        let failed_tests = self.results.values().filter(|r| r.leak_detected).count();

        report.push_str(&format!("## Summary\n"));
        report.push_str(&format!("- Total tests: {}\n", total_tests));
        report.push_str(&format!("- Tests with potential leaks: {}\n", failed_tests));

        if failed_tests > 0 {
            report.push_str("\n## ⚠️ Potential Memory Leaks Detected\n\n");

            for (test_name, result) in &self.results {
                if result.leak_detected {
                    report.push_str(&format!(
                        "### {}\n- Net growth: {} bytes\n- Max usage: {} MB\n\n",
                        test_name,
                        result.net_allocated_growth,
                        result.max_memory_usage / 1_048_576
                    ));
                }
            }
        } else {
            report.push_str("\n## ✅ No Memory Leaks Detected\n\n");
        }

        report
    }
}
```

## Coverage Targets

### Minimum Coverage Requirements

- **Overall Test Coverage**: 85%
- **Unit Test Coverage**: 90%
- **Integration Test Coverage**: 75%
- **Performance Test Coverage**: 100% of critical paths
- **Edge Case Coverage**: 95% of identified edge cases

### Coverage by Component

| Component | Unit Tests | Integration | Performance | Edge Cases |
|-----------|------------|-------------|-------------|------------|
| Core Algorithms | 95% | 80% | 100% | 95% |
| WASM Bindings | 85% | 90% | 100% | 85% |
| Streaming API | 90% | 95% | 100% | 90% |
| CLI Interface | 80% | 100% | 75% | 85% |
| Numerical Methods | 95% | 85% | 100% | 100% |

### Quality Gates

1. **All tests must pass** before merge
2. **No performance regressions** >10%
3. **Memory usage** within acceptable bounds
4. **Cross-platform compatibility** verified
5. **Documentation** matches implementation

This comprehensive testing strategy ensures robust, reliable, and performant software that meets the high standards required for numerical computing applications.