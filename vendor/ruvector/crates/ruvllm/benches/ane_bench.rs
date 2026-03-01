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
//! ANE vs NEON Benchmark Suite
//!
//! Compares Apple Neural Engine (via BNNS) operations against
//! hand-optimized NEON implementations.
//!
//! ## Running Benchmarks
//!
//! ANE benchmarks (requires macOS with coreml feature):
//! ```bash
//! cargo bench -p ruvllm --features coreml --bench ane_bench
//! ```
//!
//! Compare ANE vs Accelerate:
//! ```bash
//! cargo bench -p ruvllm --features coreml,accelerate --bench ane_bench
//! ```
//!
//! ## Performance Targets (M4 Pro)
//!
//! | Operation | Size | ANE Target | NEON Baseline | Expected Speedup |
//! |-----------|------|------------|---------------|------------------|
//! | GEMM | 1x4096x4096 | <500us | <800us | 1.5-2x |
//! | GELU | 64x4096 | <100us | <150us | 1.3-1.5x |
//! | SiLU | 64x4096 | <100us | <150us | 1.3-1.5x |
//! | Softmax | 64x4096 | <150us | <200us | 1.2-1.4x |
//! | LayerNorm | 64x4096 | <200us | <250us | 1.2-1.3x |
//!
//! ## Power Efficiency
//!
//! ANE typically provides 3-4x better performance per watt compared to
//! GPU or CPU for supported operations. This benchmark suite measures
//! wall-clock time, not power consumption.
//!
//! To measure power consumption on macOS, use:
//! ```bash
//! sudo powermetrics --samplers tasks -i 100 | grep ruvllm
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate random tensor data
fn random_tensor(size: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

/// Generate random positive tensor (for softmax stability testing)
fn random_positive_tensor(size: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen_range(0.0..10.0)).collect()
}

// ============================================================================
// Matrix Multiplication Benchmarks
// ============================================================================

/// Compare GEMM implementations: ANE vs Accelerate vs NEON
fn bench_gemm_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("gemm_ane_vs_neon");
    group.sample_size(30);

    // Test various matrix sizes relevant to LLM inference
    // Format: (m, k, n) - m=batch, k=input_dim, n=output_dim
    //
    // Size categories:
    // - Small (128x128, 256x256): ANE should dominate (~30-50% faster)
    // - Medium (512x512, 1024x1024): Transition zone, ANE slight edge
    // - Large (2048x2048, 4096x4096): GPU crossover zone
    // - Very Large (8192x8192): GPU clear winner
    let sizes = [
        // Small matrices - ANE advantage zone
        (1, 128, 128), // Tiny matmul - ANE wins
        (1, 256, 256), // Small matmul - ANE wins
        (1, 512, 512), // Medium-small - ANE edge
        // Medium matrices - Transition zone
        (1, 1024, 1024), // ANE/GPU crossover starts
        (1, 2048, 2048), // Crossover zone
        // Large matrices - GPU advantage
        (1, 4096, 4096),  // Single token, typical projection - GPU starts winning
        (1, 4096, 11008), // Llama MLP up-projection
        (1, 11008, 4096), // Llama MLP down-projection
        // Batch inference - ANE optimal for small batches
        (8, 4096, 4096),   // Small batch
        (32, 4096, 4096),  // Medium batch
        (64, 4096, 4096),  // Optimal ANE batch size
        (128, 4096, 4096), // Beyond ANE optimal - GPU wins
    ];

    for (m, k, n) in sizes {
        let a = random_tensor(m * k);
        let b = random_tensor(k * n);
        let mut c_out = vec![0.0f32; m * n];

        let flops = 2 * m * k * n;
        let id_suffix = format!("{}x{}x{}", m, k, n);

        group.throughput(Throughput::Elements(flops as u64));

        // NEON baseline (always available on aarch64)
        #[cfg(target_arch = "aarch64")]
        {
            let id = BenchmarkId::new("neon", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    // Use local GEMM implementation to avoid module dependency issues
                    gemm_neon_local(black_box(&a), black_box(&b), black_box(&mut c_out), m, k, n);
                })
            });
        }

        // Accelerate (uses AMX coprocessor)
        #[cfg(all(target_os = "macos", feature = "accelerate"))]
        {
            let id = BenchmarkId::new("accelerate", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    ruvllm::kernels::accelerate::gemm_accelerate(
                        black_box(&a),
                        black_box(&b),
                        black_box(&mut c_out),
                        m,
                        k,
                        n,
                    );
                })
            });
        }

        // ANE via BNNS/Accelerate
        #[cfg(all(target_os = "macos", feature = "coreml"))]
        {
            let id = BenchmarkId::new("ane", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    ruvllm::kernels::ane_ops::matmul_ane(
                        black_box(&a),
                        black_box(&b),
                        black_box(&mut c_out),
                        m,
                        k,
                        n,
                    );
                })
            });
        }
    }

    group.finish();
}

/// Benchmark batched matrix multiplication
fn bench_batched_gemm_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("batched_gemm_ane_vs_neon");
    group.sample_size(30);

    // Typical attention shapes: batch of Q*K^T or attention*V
    let configs = [
        (8, 128, 128, 128),  // 8 heads, seq=128
        (32, 128, 128, 128), // 32 heads, seq=128
        (32, 256, 128, 256), // 32 heads, seq=256, head_dim=128
        (8, 512, 128, 512),  // 8 heads, seq=512
    ];

    for (batch_size, m, k, n) in configs {
        let a = random_tensor(batch_size * m * k);
        let b = random_tensor(batch_size * k * n);
        let mut c_out = vec![0.0f32; batch_size * m * n];

        let flops = 2 * batch_size * m * k * n;
        let id_suffix = format!("batch{}_{}x{}x{}", batch_size, m, k, n);

        group.throughput(Throughput::Elements(flops as u64));

        // NEON batched
        #[cfg(target_arch = "aarch64")]
        {
            let id = BenchmarkId::new("neon", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    for batch in 0..batch_size {
                        let a_off = batch * m * k;
                        let b_off = batch * k * n;
                        let c_off = batch * m * n;
                        gemm_neon_local(
                            black_box(&a[a_off..a_off + m * k]),
                            black_box(&b[b_off..b_off + k * n]),
                            black_box(&mut c_out[c_off..c_off + m * n]),
                            m,
                            k,
                            n,
                        );
                    }
                })
            });
        }

        // ANE batched
        #[cfg(all(target_os = "macos", feature = "coreml"))]
        {
            let id = BenchmarkId::new("ane", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    ruvllm::kernels::ane_ops::batched_matmul_ane(
                        black_box(&a),
                        black_box(&b),
                        black_box(&mut c_out),
                        batch_size,
                        m,
                        k,
                        n,
                    );
                })
            });
        }
    }

    group.finish();
}

// ============================================================================
// Activation Function Benchmarks
// ============================================================================

/// Compare GELU implementations
fn bench_gelu_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("gelu_ane_vs_neon");
    group.sample_size(50);

    // Various batch and dimension sizes
    let configs = [
        (1, 4096),
        (8, 4096),
        (32, 4096),
        (64, 4096),
        (1, 11008), // Llama MLP intermediate
        (32, 11008),
    ];

    for (batch_size, dim) in configs {
        let size = batch_size * dim;
        let x_orig = random_tensor(size);

        let ops = size; // One GELU per element
        let id_suffix = format!("{}x{}", batch_size, dim);

        group.throughput(Throughput::Elements(ops as u64));

        // NEON
        #[cfg(target_arch = "aarch64")]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("neon", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::activations::batch_gelu(black_box(&mut x), dim);
                })
            });
        }

        // ANE
        #[cfg(all(target_os = "macos", feature = "coreml"))]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("ane", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::ane_ops::gelu_ane(black_box(&mut x), batch_size, dim);
                })
            });
        }
    }

    group.finish();
}

/// Compare SiLU implementations
fn bench_silu_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("silu_ane_vs_neon");
    group.sample_size(50);

    let configs = [
        (1, 4096),
        (8, 4096),
        (32, 4096),
        (64, 4096),
        (1, 11008),
        (32, 11008),
    ];

    for (batch_size, dim) in configs {
        let size = batch_size * dim;
        let x_orig = random_tensor(size);

        let ops = size;
        let id_suffix = format!("{}x{}", batch_size, dim);

        group.throughput(Throughput::Elements(ops as u64));

        // NEON
        #[cfg(target_arch = "aarch64")]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("neon", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::activations::batch_silu(black_box(&mut x), dim);
                })
            });
        }

        // ANE
        #[cfg(all(target_os = "macos", feature = "coreml"))]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("ane", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::ane_ops::silu_ane(black_box(&mut x), batch_size, dim);
                })
            });
        }
    }

    group.finish();
}

/// Compare Softmax implementations
fn bench_softmax_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("softmax_ane_vs_neon");
    group.sample_size(50);

    // Softmax is typically applied to attention scores
    let configs = [
        (1, 128),   // Single head, short seq
        (32, 128),  // 32 heads, short seq
        (32, 512),  // 32 heads, medium seq
        (32, 2048), // 32 heads, long seq
        (1, 4096),  // Single head, very long
    ];

    for (batch_size, dim) in configs {
        let size = batch_size * dim;
        let x_orig = random_positive_tensor(size);

        let ops = size;
        let id_suffix = format!("{}x{}", batch_size, dim);

        group.throughput(Throughput::Elements(ops as u64));

        // NEON
        #[cfg(target_arch = "aarch64")]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("neon", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::activations::batch_softmax(black_box(&mut x), dim);
                })
            });
        }

        // ANE
        #[cfg(all(target_os = "macos", feature = "coreml"))]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("ane", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::ane_ops::softmax_ane(black_box(&mut x), batch_size, dim);
                })
            });
        }
    }

    group.finish();
}

// ============================================================================
// Normalization Benchmarks
// ============================================================================

/// Compare LayerNorm implementations
fn bench_layer_norm_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("layernorm_ane_vs_neon");
    group.sample_size(50);

    let configs = [(1, 4096), (8, 4096), (32, 4096), (64, 4096), (128, 4096)];

    for (batch_size, dim) in configs {
        let size = batch_size * dim;
        let x_orig = random_tensor(size);
        let weight = vec![1.0f32; dim];
        let bias = vec![0.0f32; dim];

        let ops = size * 4; // Approximate: mean, var, normalize, scale
        let id_suffix = format!("{}x{}", batch_size, dim);

        group.throughput(Throughput::Elements(ops as u64));

        // NEON
        #[cfg(target_arch = "aarch64")]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("neon", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::norm::batched_layer_norm_neon(
                        black_box(&mut x),
                        black_box(&weight),
                        black_box(&bias),
                        batch_size,
                        dim,
                        1e-6,
                    );
                })
            });
        }

        // ANE
        #[cfg(all(target_os = "macos", feature = "coreml"))]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("ane", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::ane_ops::layer_norm_ane(
                        black_box(&mut x),
                        black_box(&weight),
                        black_box(&bias),
                        batch_size,
                        dim,
                        1e-6,
                    );
                })
            });
        }
    }

    group.finish();
}

/// Compare RMSNorm implementations
fn bench_rms_norm_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("rmsnorm_ane_vs_neon");
    group.sample_size(50);

    let configs = [(1, 4096), (8, 4096), (32, 4096), (64, 4096), (128, 4096)];

    for (batch_size, dim) in configs {
        let size = batch_size * dim;
        let x_orig = random_tensor(size);
        let weight = vec![1.0f32; dim];

        let ops = size * 3; // Approximate: sum_sq, normalize, scale
        let id_suffix = format!("{}x{}", batch_size, dim);

        group.throughput(Throughput::Elements(ops as u64));

        // NEON
        #[cfg(target_arch = "aarch64")]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("neon", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::norm::batched_rms_norm_neon(
                        black_box(&mut x),
                        black_box(&weight),
                        batch_size,
                        dim,
                        1e-6,
                    );
                })
            });
        }

        // ANE
        #[cfg(all(target_os = "macos", feature = "coreml"))]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("ane", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::ane_ops::rms_norm_ane(
                        black_box(&mut x),
                        black_box(&weight),
                        batch_size,
                        dim,
                        1e-6,
                    );
                })
            });
        }
    }

    group.finish();
}

// ============================================================================
// Auto-Dispatch Benchmarks
// ============================================================================

/// Test the auto-dispatch functions that select best backend
fn bench_auto_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("auto_dispatch");
    group.sample_size(50);

    let batch_size = 32;
    let dim = 4096;
    let size = batch_size * dim;

    let x_orig = random_tensor(size);
    let weight = vec![1.0f32; dim];
    let bias = vec![0.0f32; dim];

    // Auto-dispatch GELU
    {
        let mut x = x_orig.clone();
        group.bench_function("gelu_auto", |bencher| {
            bencher.iter(|| {
                x.copy_from_slice(&x_orig);
                #[cfg(all(target_os = "macos", feature = "coreml"))]
                ruvllm::kernels::ane_ops::gelu_auto(black_box(&mut x), batch_size, dim);
                #[cfg(not(all(target_os = "macos", feature = "coreml")))]
                ruvllm::kernels::activations::batch_gelu(black_box(&mut x), dim);
            })
        });
    }

    // Auto-dispatch SiLU
    {
        let mut x = x_orig.clone();
        group.bench_function("silu_auto", |bencher| {
            bencher.iter(|| {
                x.copy_from_slice(&x_orig);
                #[cfg(all(target_os = "macos", feature = "coreml"))]
                ruvllm::kernels::ane_ops::silu_auto(black_box(&mut x), batch_size, dim);
                #[cfg(not(all(target_os = "macos", feature = "coreml")))]
                ruvllm::kernels::activations::batch_silu(black_box(&mut x), dim);
            })
        });
    }

    // Auto-dispatch LayerNorm
    {
        let mut x = x_orig.clone();
        group.bench_function("layernorm_auto", |bencher| {
            bencher.iter(|| {
                x.copy_from_slice(&x_orig);
                #[cfg(all(target_os = "macos", feature = "coreml"))]
                ruvllm::kernels::ane_ops::layer_norm_auto(
                    black_box(&mut x),
                    black_box(&weight),
                    black_box(&bias),
                    batch_size,
                    dim,
                    1e-6,
                );
                #[cfg(not(all(target_os = "macos", feature = "coreml")))]
                ruvllm::kernels::norm::batched_layer_norm_neon(
                    black_box(&mut x),
                    black_box(&weight),
                    black_box(&bias),
                    batch_size,
                    dim,
                    1e-6,
                );
            })
        });
    }

    group.finish();
}

// ============================================================================
// LLM Workload Benchmarks (Realistic Scenarios)
// ============================================================================

/// Benchmark typical MLP block operations
fn bench_mlp_block(c: &mut Criterion) {
    let mut group = c.benchmark_group("mlp_block");
    group.sample_size(20);

    // Llama2-7B MLP: hidden_dim=4096, intermediate=11008
    let batch_size = 1;
    let hidden_dim = 4096;
    let intermediate_dim = 11008;

    // Up projection weights
    let w_up = random_tensor(hidden_dim * intermediate_dim);
    // Down projection weights
    let w_down = random_tensor(intermediate_dim * hidden_dim);

    let input = random_tensor(batch_size * hidden_dim);
    let mut intermediate = vec![0.0f32; batch_size * intermediate_dim];
    let mut output = vec![0.0f32; batch_size * hidden_dim];

    let total_flops = 2 * batch_size * hidden_dim * intermediate_dim  // Up
                    + batch_size * intermediate_dim                    // Activation
                    + 2 * batch_size * intermediate_dim * hidden_dim; // Down

    group.throughput(Throughput::Elements(total_flops as u64));

    // NEON path
    #[cfg(target_arch = "aarch64")]
    {
        group.bench_function("neon", |bencher| {
            bencher.iter(|| {
                // Up projection
                gemm_neon_local(
                    black_box(&input),
                    black_box(&w_up),
                    black_box(&mut intermediate),
                    batch_size,
                    hidden_dim,
                    intermediate_dim,
                );
                // SiLU activation
                ruvllm::kernels::activations::batch_silu(
                    black_box(&mut intermediate),
                    intermediate_dim,
                );
                // Down projection
                gemm_neon_local(
                    black_box(&intermediate),
                    black_box(&w_down),
                    black_box(&mut output),
                    batch_size,
                    intermediate_dim,
                    hidden_dim,
                );
            })
        });
    }

    // ANE path
    #[cfg(all(target_os = "macos", feature = "coreml"))]
    {
        group.bench_function("ane", |bencher| {
            bencher.iter(|| {
                // Up projection
                ruvllm::kernels::ane_ops::matmul_ane(
                    black_box(&input),
                    black_box(&w_up),
                    black_box(&mut intermediate),
                    batch_size,
                    hidden_dim,
                    intermediate_dim,
                );
                // SiLU activation
                ruvllm::kernels::ane_ops::silu_ane(
                    black_box(&mut intermediate),
                    batch_size,
                    intermediate_dim,
                );
                // Down projection
                ruvllm::kernels::ane_ops::matmul_ane(
                    black_box(&intermediate),
                    black_box(&w_down),
                    black_box(&mut output),
                    batch_size,
                    intermediate_dim,
                    hidden_dim,
                );
            })
        });
    }

    group.finish();
}

// ============================================================================
// Local NEON GEMM Implementation (to avoid module dependency issues)
// ============================================================================

#[cfg(target_arch = "aarch64")]
fn gemm_neon_local(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    c.fill(0.0);

    unsafe {
        use std::arch::aarch64::*;

        let a_ptr = a.as_ptr();
        let b_ptr = b.as_ptr();
        let c_ptr = c.as_mut_ptr();

        for i in 0..m {
            let mut j = 0usize;
            while j + 4 <= n {
                let mut acc = vdupq_n_f32(0.0);

                for kk in 0..k {
                    let a_val = vdupq_n_f32(*a_ptr.add(i * k + kk));
                    let b_v = vld1q_f32(b_ptr.add(kk * n + j));
                    acc = vfmaq_f32(acc, a_val, b_v);
                }

                vst1q_f32(c_ptr.add(i * n + j), acc);
                j += 4;
            }

            // Handle remaining columns
            while j < n {
                let mut sum = 0.0f32;
                for kk in 0..k {
                    sum += *a_ptr.add(i * k + kk) * *b_ptr.add(kk * n + j);
                }
                *c_ptr.add(i * n + j) = sum;
                j += 1;
            }
        }
    }
}

#[cfg(not(target_arch = "aarch64"))]
fn gemm_neon_local(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    c.fill(0.0);
    for i in 0..m {
        for j in 0..n {
            let mut sum = 0.0f32;
            for kk in 0..k {
                sum += a[i * k + kk] * b[kk * n + j];
            }
            c[i * n + j] = sum;
        }
    }
}

// ============================================================================
// Crossover Point Detection Benchmark
// ============================================================================

/// Benchmark to identify the exact crossover point where GPU beats ANE
///
/// This benchmark tests matrix sizes in increments to find where:
/// 1. ANE is clearly faster (small matrices)
/// 2. Performance is similar (crossover zone)
/// 3. GPU is clearly faster (large matrices)
///
/// Expected M4 Pro results:
/// - ANE wins: dim < 1024
/// - Crossover: 1024 <= dim <= 2048
/// - GPU wins: dim > 2048
fn bench_crossover_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("crossover_detection");
    group.sample_size(20);

    // Test dimensions in powers of 2 to find crossover
    let dimensions = [64, 128, 256, 512, 768, 1024, 1536, 2048, 3072, 4096];

    for dim in dimensions {
        let a = random_tensor(dim * dim);
        let b = random_tensor(dim * dim);
        let mut c_out = vec![0.0f32; dim * dim];

        let flops = 2 * dim * dim * dim;
        let id_suffix = format!("{}x{}", dim, dim);

        group.throughput(Throughput::Elements(flops as u64));

        // NEON baseline
        #[cfg(target_arch = "aarch64")]
        {
            let id = BenchmarkId::new("neon", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    gemm_neon_local(
                        black_box(&a),
                        black_box(&b),
                        black_box(&mut c_out),
                        dim,
                        dim,
                        dim,
                    );
                })
            });
        }

        // ANE via BNNS
        #[cfg(all(target_os = "macos", feature = "coreml"))]
        {
            let id = BenchmarkId::new("ane", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    ruvllm::kernels::ane_ops::matmul_ane(
                        black_box(&a),
                        black_box(&b),
                        black_box(&mut c_out),
                        dim,
                        dim,
                        dim,
                    );
                })
            });
        }

        // Accelerate (AMX)
        #[cfg(all(target_os = "macos", feature = "accelerate"))]
        {
            let id = BenchmarkId::new("accelerate", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    ruvllm::kernels::accelerate::gemm_accelerate(
                        black_box(&a),
                        black_box(&b),
                        black_box(&mut c_out),
                        dim,
                        dim,
                        dim,
                    );
                })
            });
        }
    }

    group.finish();
}

// ============================================================================
// Hybrid Pipeline Benchmarks (ANE for MLP, GPU for Attention)
// ============================================================================

/// Benchmark hybrid ANE+GPU pipeline for transformer inference
///
/// Real transformer layers have different compute patterns:
/// - Attention: memory-bound, GPU-friendly (high parallelism)
/// - MLP: compute-bound, ANE-friendly (batch operations)
///
/// This benchmark simulates a hybrid pipeline where:
/// 1. ANE handles MLP layers (activations, small projections)
/// 2. GPU/NEON handles attention (Q*K^T, softmax*V)
#[cfg(all(target_os = "macos", feature = "coreml"))]
fn bench_hybrid_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid_pipeline");
    group.sample_size(15);

    // Transformer configuration (Llama-7B like)
    let configs = [
        // (batch, seq_len, hidden, heads, head_dim, intermediate)
        (1, 128, 4096, 32, 128, 11008),  // Short context
        (1, 512, 4096, 32, 128, 11008),  // Medium context
        (1, 2048, 4096, 32, 128, 11008), // Long context
    ];

    for (batch, seq_len, hidden_dim, num_heads, head_dim, intermediate_dim) in configs {
        let id_suffix = format!("batch{}_seq{}", batch, seq_len);

        // Pre-allocate tensors
        let hidden = random_tensor(batch * seq_len * hidden_dim);
        let w_q = random_tensor(hidden_dim * hidden_dim);
        let w_k = random_tensor(hidden_dim * hidden_dim);
        let w_v = random_tensor(hidden_dim * hidden_dim);
        let w_o = random_tensor(hidden_dim * hidden_dim);
        let w_up = random_tensor(hidden_dim * intermediate_dim);
        let w_down = random_tensor(intermediate_dim * hidden_dim);

        let mut q = vec![0.0f32; batch * seq_len * hidden_dim];
        let mut k = vec![0.0f32; batch * seq_len * hidden_dim];
        let mut v = vec![0.0f32; batch * seq_len * hidden_dim];
        let mut attn_output = vec![0.0f32; batch * seq_len * hidden_dim];
        let mut intermediate = vec![0.0f32; batch * seq_len * intermediate_dim];
        let mut mlp_output = vec![0.0f32; batch * seq_len * hidden_dim];

        let total_ops =
            // Q, K, V projections
            3 * 2 * batch * seq_len * hidden_dim * hidden_dim +
            // Attention (Q*K^T + softmax + attn*V)
            2 * batch * num_heads * seq_len * seq_len * head_dim * 2 +
            // O projection
            2 * batch * seq_len * hidden_dim * hidden_dim +
            // MLP up + down
            2 * batch * seq_len * hidden_dim * intermediate_dim * 2 +
            // Activations
            batch * seq_len * intermediate_dim;

        group.throughput(Throughput::Elements(total_ops as u64));

        // Pure NEON path
        #[cfg(target_arch = "aarch64")]
        {
            let id = BenchmarkId::new("pure_neon", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    // Q, K, V projections
                    gemm_neon_local(
                        &hidden,
                        &w_q,
                        &mut q,
                        batch * seq_len,
                        hidden_dim,
                        hidden_dim,
                    );
                    gemm_neon_local(
                        &hidden,
                        &w_k,
                        &mut k,
                        batch * seq_len,
                        hidden_dim,
                        hidden_dim,
                    );
                    gemm_neon_local(
                        &hidden,
                        &w_v,
                        &mut v,
                        batch * seq_len,
                        hidden_dim,
                        hidden_dim,
                    );

                    // O projection
                    gemm_neon_local(
                        &v,
                        &w_o,
                        &mut attn_output,
                        batch * seq_len,
                        hidden_dim,
                        hidden_dim,
                    );

                    // MLP: up projection
                    gemm_neon_local(
                        &attn_output,
                        &w_up,
                        &mut intermediate,
                        batch * seq_len,
                        hidden_dim,
                        intermediate_dim,
                    );

                    // MLP: SiLU activation (in-place)
                    ruvllm::kernels::activations::batch_silu(
                        black_box(&mut intermediate),
                        intermediate_dim,
                    );

                    // MLP: down projection
                    gemm_neon_local(
                        &intermediate,
                        &w_down,
                        &mut mlp_output,
                        batch * seq_len,
                        intermediate_dim,
                        hidden_dim,
                    );
                })
            });
        }

        // Pure ANE path
        let id = BenchmarkId::new("pure_ane", &id_suffix);
        group.bench_function(id, |bencher| {
            bencher.iter(|| {
                // Q, K, V projections
                ruvllm::kernels::ane_ops::matmul_ane(
                    &hidden,
                    &w_q,
                    &mut q,
                    batch * seq_len,
                    hidden_dim,
                    hidden_dim,
                );
                ruvllm::kernels::ane_ops::matmul_ane(
                    &hidden,
                    &w_k,
                    &mut k,
                    batch * seq_len,
                    hidden_dim,
                    hidden_dim,
                );
                ruvllm::kernels::ane_ops::matmul_ane(
                    &hidden,
                    &w_v,
                    &mut v,
                    batch * seq_len,
                    hidden_dim,
                    hidden_dim,
                );

                // O projection
                ruvllm::kernels::ane_ops::matmul_ane(
                    &v,
                    &w_o,
                    &mut attn_output,
                    batch * seq_len,
                    hidden_dim,
                    hidden_dim,
                );

                // MLP: up projection
                ruvllm::kernels::ane_ops::matmul_ane(
                    &attn_output,
                    &w_up,
                    &mut intermediate,
                    batch * seq_len,
                    hidden_dim,
                    intermediate_dim,
                );

                // MLP: SiLU activation (ANE)
                ruvllm::kernels::ane_ops::silu_ane(
                    black_box(&mut intermediate),
                    batch * seq_len,
                    intermediate_dim,
                );

                // MLP: down projection
                ruvllm::kernels::ane_ops::matmul_ane(
                    &intermediate,
                    &w_down,
                    &mut mlp_output,
                    batch * seq_len,
                    intermediate_dim,
                    hidden_dim,
                );
            })
        });

        // Hybrid path: ANE for MLP activations, auto-dispatch for matmul
        let id = BenchmarkId::new("hybrid", &id_suffix);
        group.bench_function(id, |bencher| {
            bencher.iter(|| {
                // Q, K, V projections (auto-dispatch based on size)
                ruvllm::kernels::ane_ops::matmul_auto(
                    &hidden,
                    &w_q,
                    &mut q,
                    batch * seq_len,
                    hidden_dim,
                    hidden_dim,
                );
                ruvllm::kernels::ane_ops::matmul_auto(
                    &hidden,
                    &w_k,
                    &mut k,
                    batch * seq_len,
                    hidden_dim,
                    hidden_dim,
                );
                ruvllm::kernels::ane_ops::matmul_auto(
                    &hidden,
                    &w_v,
                    &mut v,
                    batch * seq_len,
                    hidden_dim,
                    hidden_dim,
                );

                // O projection (auto-dispatch)
                ruvllm::kernels::ane_ops::matmul_auto(
                    &v,
                    &w_o,
                    &mut attn_output,
                    batch * seq_len,
                    hidden_dim,
                    hidden_dim,
                );

                // MLP: up projection (auto-dispatch)
                ruvllm::kernels::ane_ops::matmul_auto(
                    &attn_output,
                    &w_up,
                    &mut intermediate,
                    batch * seq_len,
                    hidden_dim,
                    intermediate_dim,
                );

                // MLP: SiLU activation (auto-dispatch - typically ANE)
                ruvllm::kernels::ane_ops::silu_auto(
                    black_box(&mut intermediate),
                    batch * seq_len,
                    intermediate_dim,
                );

                // MLP: down projection (auto-dispatch)
                ruvllm::kernels::ane_ops::matmul_auto(
                    &intermediate,
                    &w_down,
                    &mut mlp_output,
                    batch * seq_len,
                    intermediate_dim,
                    hidden_dim,
                );
            })
        });
    }

    group.finish();
}

// ============================================================================
// Activation Crossover Benchmark
// ============================================================================

/// Benchmark activation functions to find ANE vs NEON crossover
fn bench_activation_crossover(c: &mut Criterion) {
    let mut group = c.benchmark_group("activation_crossover");
    group.sample_size(50);

    // Test various sizes to find where ANE beats NEON
    let sizes = [
        (1, 128),    // Tiny
        (1, 512),    // Small
        (1, 2048),   // Medium
        (1, 4096),   // Llama hidden
        (1, 11008),  // Llama intermediate
        (32, 4096),  // Batch
        (64, 4096),  // Larger batch
        (128, 4096), // Big batch
    ];

    for (batch_size, dim) in sizes {
        let size = batch_size * dim;
        let x_orig = random_tensor(size);

        let id_suffix = format!("{}x{}", batch_size, dim);
        group.throughput(Throughput::Elements(size as u64));

        // NEON SiLU
        #[cfg(target_arch = "aarch64")]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("silu_neon", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::activations::batch_silu(black_box(&mut x), dim);
                })
            });
        }

        // ANE SiLU
        #[cfg(all(target_os = "macos", feature = "coreml"))]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("silu_ane", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::ane_ops::silu_ane(black_box(&mut x), batch_size, dim);
                })
            });
        }

        // Auto-dispatch SiLU
        #[cfg(all(target_os = "macos", feature = "coreml"))]
        {
            let mut x = x_orig.clone();
            let id = BenchmarkId::new("silu_auto", &id_suffix);
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    x.copy_from_slice(&x_orig);
                    ruvllm::kernels::ane_ops::silu_auto(black_box(&mut x), batch_size, dim);
                })
            });
        }
    }

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

// Full benchmark group for macOS with both features
#[cfg(all(target_os = "macos", feature = "coreml"))]
criterion_group!(
    benches,
    bench_gemm_comparison,
    bench_batched_gemm_comparison,
    bench_gelu_comparison,
    bench_silu_comparison,
    bench_softmax_comparison,
    bench_layer_norm_comparison,
    bench_rms_norm_comparison,
    bench_auto_dispatch,
    bench_mlp_block,
    bench_crossover_detection,
    bench_hybrid_pipeline,
    bench_activation_crossover,
);

// Reduced benchmark group for non-coreml builds
#[cfg(not(all(target_os = "macos", feature = "coreml")))]
criterion_group!(
    benches,
    bench_gemm_comparison,
    bench_batched_gemm_comparison,
    bench_gelu_comparison,
    bench_silu_comparison,
    bench_softmax_comparison,
    bench_layer_norm_comparison,
    bench_rms_norm_comparison,
    bench_mlp_block,
    bench_crossover_detection,
    bench_activation_crossover,
);

criterion_main!(benches);
