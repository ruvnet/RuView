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
//! Metal GPU acceleration benchmarks
//!
//! Benchmarks Metal compute shaders for LLM operations.
//! Only runs on macOS with `metal-compute` feature enabled.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(all(target_os = "macos", feature = "metal-compute"))]
use ruvllm::kernels::AttentionConfig;
#[cfg(all(target_os = "macos", feature = "metal-compute"))]
use ruvllm::metal::{MetalConfig, MetalContext};

#[cfg(all(target_os = "macos", feature = "metal-compute"))]
fn bench_flash_attention_metal(c: &mut Criterion) {
    let ctx = match MetalContext::new(MetalConfig::default()) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to create Metal context: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("metal_flash_attention");

    for (seq_len, kv_len) in [(1, 512), (1, 2048), (1, 4096), (4, 512), (4, 2048)] {
        let config = AttentionConfig {
            num_heads: 32,
            num_kv_heads: 8,
            head_dim: 128,
            max_seq_len: seq_len,
            causal: true,
            scale: 0.0,
        };

        let query: Vec<f32> = (0..seq_len * config.num_heads * config.head_dim)
            .map(|i| (i as f32) * 0.001)
            .collect();
        let key: Vec<f32> = (0..kv_len * config.num_kv_heads * config.head_dim)
            .map(|i| (i as f32) * 0.001)
            .collect();
        let value: Vec<f32> = (0..kv_len * config.num_kv_heads * config.head_dim)
            .map(|i| (i as f32) * 0.001)
            .collect();

        group.bench_with_input(
            BenchmarkId::new("metal", format!("seq{}_kv{}", seq_len, kv_len)),
            &(&query, &key, &value, &config),
            |b, (q, k, v, cfg)| {
                b.iter(|| {
                    ctx.flash_attention(
                        black_box(*q),
                        black_box(*k),
                        black_box(*v),
                        black_box(*cfg),
                    )
                })
            },
        );
    }

    group.finish();
}

#[cfg(all(target_os = "macos", feature = "metal-compute"))]
fn bench_gemm_metal(c: &mut Criterion) {
    let ctx = match MetalContext::new(MetalConfig::default()) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to create Metal context: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("metal_gemm");

    for size in [128, 256, 512, 1024, 2048] {
        let m = size;
        let n = size;
        let k = size;

        let a: Vec<f32> = (0..m * k).map(|i| (i as f32) * 0.001).collect();
        let b: Vec<f32> = (0..k * n).map(|i| (i as f32) * 0.001).collect();

        group.bench_with_input(
            BenchmarkId::new("metal_f32", format!("{}x{}", size, size)),
            &(&a, &b, m, n, k),
            |bench, (a, b, m, n, k)| {
                bench.iter(|| ctx.gemm_f32(black_box(*a), black_box(*b), *m, *n, *k))
            },
        );
    }

    group.finish();
}

#[cfg(all(target_os = "macos", feature = "metal-compute"))]
fn bench_rms_norm_metal(c: &mut Criterion) {
    let ctx = match MetalContext::new(MetalConfig::default()) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to create Metal context: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("metal_rms_norm");

    for hidden_size in [1024, 2048, 4096, 8192] {
        let batch_size = 4;
        let mut x: Vec<f32> = (0..batch_size * hidden_size)
            .map(|i| (i as f32) * 0.001)
            .collect();
        let weight: Vec<f32> = vec![1.0; hidden_size];

        group.bench_with_input(
            BenchmarkId::new("metal", format!("hidden{}", hidden_size)),
            &(hidden_size, batch_size),
            |bench, _| {
                bench.iter(|| {
                    let mut x_clone = x.clone();
                    ctx.rms_norm(black_box(&mut x_clone), black_box(&weight), 1e-6)
                })
            },
        );
    }

    group.finish();
}

#[cfg(all(target_os = "macos", feature = "metal-compute"))]
fn bench_rope_metal(c: &mut Criterion) {
    let ctx = match MetalContext::new(MetalConfig::default()) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to create Metal context: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("metal_rope");

    for num_heads in [8, 16, 32] {
        let head_dim = 128;
        let batch_size = 4;
        let mut x: Vec<f32> = (0..batch_size * num_heads * head_dim)
            .map(|i| (i as f32) * 0.001)
            .collect();

        group.bench_with_input(
            BenchmarkId::new("metal", format!("heads{}", num_heads)),
            &(num_heads, head_dim, batch_size),
            |bench, &(nh, hd, bs)| {
                bench.iter(|| {
                    let mut x_clone = x.clone();
                    ctx.apply_rope(black_box(&mut x_clone), 0, nh, hd, 10000.0)
                })
            },
        );
    }

    group.finish();
}

// ============ M4 Pro Optimized Benchmarks ============

#[cfg(all(target_os = "macos", feature = "metal-compute"))]
fn bench_optimized_gemm_metal(c: &mut Criterion) {
    let ctx = match MetalContext::new(MetalConfig::default()) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to create Metal context: {}", e);
            return;
        }
    };

    if !ctx.has_m4_pro_optimizations() {
        eprintln!("M4 Pro optimizations not available, skipping optimized GEMM benchmark");
        return;
    }

    println!(
        "Available optimizations: {:?}",
        ctx.available_optimizations()
    );

    let mut group = c.benchmark_group("metal_gemm_optimized");

    for size in [128, 256, 512, 1024, 2048, 4096] {
        let m = size;
        let n = size;
        let k = size;

        let a: Vec<half::f16> = (0..m * k)
            .map(|i| half::f16::from_f32((i as f32) * 0.001))
            .collect();
        let b: Vec<half::f16> = (0..k * n)
            .map(|i| half::f16::from_f32((i as f32) * 0.001))
            .collect();

        // Benchmark standard GEMM
        group.bench_with_input(
            BenchmarkId::new("standard_f16", format!("{}x{}", size, size)),
            &(&a, &b, m, n, k),
            |bench, (a, b, m, n, k)| {
                bench.iter(|| ctx.gemm_f16(black_box(*a), black_box(*b), *m, *n, *k))
            },
        );

        // Benchmark M4 Pro optimized GEMM (BM=128, BN=128, BK=32)
        group.bench_with_input(
            BenchmarkId::new("m4_optimized", format!("{}x{}", size, size)),
            &(&a, &b, m, n, k),
            |bench, (a, b, m, n, k)| {
                bench.iter(|| ctx.gemm_optimized(black_box(*a), black_box(*b), *m, *n, *k))
            },
        );
    }

    group.finish();
}

#[cfg(all(target_os = "macos", feature = "metal-compute"))]
fn bench_fused_attention_metal(c: &mut Criterion) {
    let ctx = match MetalContext::new(MetalConfig::default()) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to create Metal context: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("metal_fused_attention");

    for (seq_len, kv_len) in [
        (1, 512),
        (1, 2048),
        (1, 4096),
        (4, 512),
        (4, 2048),
        (16, 2048),
    ] {
        let num_heads = 32;
        let num_kv_heads = 8;
        let head_dim = 128;

        let query: Vec<f32> = (0..seq_len * num_heads * head_dim)
            .map(|i| (i as f32) * 0.001)
            .collect();
        let key: Vec<f32> = (0..kv_len * num_kv_heads * head_dim)
            .map(|i| (i as f32) * 0.001)
            .collect();
        let value: Vec<f32> = (0..kv_len * num_kv_heads * head_dim)
            .map(|i| (i as f32) * 0.001)
            .collect();

        // Standard attention (legacy)
        let config = AttentionConfig {
            num_heads,
            num_kv_heads,
            head_dim,
            max_seq_len: seq_len,
            causal: true,
            scale: 0.0,
        };

        group.bench_with_input(
            BenchmarkId::new("standard", format!("seq{}_kv{}", seq_len, kv_len)),
            &(&query, &key, &value, &config),
            |b, (q, k, v, cfg)| {
                b.iter(|| {
                    ctx.flash_attention(
                        black_box(*q),
                        black_box(*k),
                        black_box(*v),
                        black_box(*cfg),
                    )
                })
            },
        );

        // Fused Flash Attention 2
        group.bench_with_input(
            BenchmarkId::new("fused_fa2", format!("seq{}_kv{}", seq_len, kv_len)),
            &(&query, &key, &value, num_heads, num_kv_heads, head_dim),
            |b, (q, k, v, nh, nkv, hd)| {
                b.iter(|| {
                    ctx.fused_attention(
                        black_box(*q),
                        black_box(*k),
                        black_box(*v),
                        *nh,
                        *nkv,
                        *hd,
                        true,
                    )
                })
            },
        );
    }

    group.finish();
}

#[cfg(all(target_os = "macos", feature = "metal-compute"))]
fn bench_fused_norm_residual_metal(c: &mut Criterion) {
    let ctx = match MetalContext::new(MetalConfig::default()) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to create Metal context: {}", e);
            return;
        }
    };

    if ctx
        .available_optimizations()
        .iter()
        .find(|&&s| s == "fused_layernorm_residual")
        .is_none()
    {
        eprintln!("Fused LayerNorm+Residual not available, skipping benchmark");
        return;
    }

    let mut group = c.benchmark_group("metal_fused_norm");

    for hidden_size in [1024, 2048, 4096, 8192] {
        let batch_size = 4;

        let x: Vec<f32> = (0..batch_size * hidden_size)
            .map(|i| (i as f32) * 0.001)
            .collect();
        let residual: Vec<f32> = (0..batch_size * hidden_size)
            .map(|i| (i as f32) * 0.0005)
            .collect();
        let weight: Vec<f32> = vec![1.0; hidden_size];
        let bias: Vec<f32> = vec![0.0; hidden_size];

        // Separate RMSNorm
        group.bench_with_input(
            BenchmarkId::new("separate_rmsnorm", format!("hidden{}", hidden_size)),
            &(hidden_size, batch_size),
            |bench, _| {
                bench.iter(|| {
                    let mut x_clone = x.clone();
                    // Add residual manually then normalize
                    for i in 0..x_clone.len() {
                        x_clone[i] += residual[i];
                    }
                    ctx.rms_norm(black_box(&mut x_clone), black_box(&weight), 1e-6)
                })
            },
        );

        // Fused RMSNorm + Residual
        group.bench_with_input(
            BenchmarkId::new("fused_rmsnorm_residual", format!("hidden{}", hidden_size)),
            &(hidden_size, batch_size),
            |bench, _| {
                bench.iter(|| {
                    let mut x_clone = x.clone();
                    ctx.fused_rmsnorm_residual(
                        black_box(&mut x_clone),
                        black_box(&residual),
                        black_box(&weight),
                        1e-6,
                    )
                })
            },
        );

        // Fused LayerNorm + Residual
        group.bench_with_input(
            BenchmarkId::new("fused_layernorm_residual", format!("hidden{}", hidden_size)),
            &(hidden_size, batch_size),
            |bench, _| {
                bench.iter(|| {
                    let mut x_clone = x.clone();
                    ctx.fused_layernorm_residual(
                        black_box(&mut x_clone),
                        black_box(&residual),
                        black_box(&weight),
                        black_box(&bias),
                        1e-6,
                    )
                })
            },
        );
    }

    group.finish();
}

#[cfg(all(target_os = "macos", feature = "metal-compute"))]
fn bench_rope_attention_fusion_metal(c: &mut Criterion) {
    let ctx = match MetalContext::new(MetalConfig::default()) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to create Metal context: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("metal_rope_attention_fusion");

    for (seq_len, kv_len) in [(1, 512), (1, 2048), (4, 2048)] {
        let num_heads = 32;
        let num_kv_heads = 8;
        let head_dim = 128;
        let rope_theta = 10000.0;

        let query: Vec<f32> = (0..seq_len * num_heads * head_dim)
            .map(|i| (i as f32) * 0.001)
            .collect();
        let key: Vec<f32> = (0..kv_len * num_kv_heads * head_dim)
            .map(|i| (i as f32) * 0.001)
            .collect();
        let value: Vec<f32> = (0..kv_len * num_kv_heads * head_dim)
            .map(|i| (i as f32) * 0.001)
            .collect();

        // Separate RoPE + Attention (baseline)
        group.bench_with_input(
            BenchmarkId::new("separate", format!("seq{}_kv{}", seq_len, kv_len)),
            &(&query, &key, &value, num_heads, num_kv_heads, head_dim),
            |b, (q, k, v, nh, nkv, hd)| {
                b.iter(|| {
                    let mut q_clone = (*q).clone();
                    let mut k_clone = (*k).clone();
                    let _ = ctx.apply_rope(&mut q_clone, 0, *nh, *hd, rope_theta);
                    let _ = ctx.apply_rope(&mut k_clone, 0, *nkv, *hd, rope_theta);
                    ctx.fused_attention(
                        black_box(&q_clone),
                        black_box(&k_clone),
                        black_box(*v),
                        *nh,
                        *nkv,
                        *hd,
                        true,
                    )
                })
            },
        );

        // Fused RoPE + Attention
        group.bench_with_input(
            BenchmarkId::new("fused", format!("seq{}_kv{}", seq_len, kv_len)),
            &(&query, &key, &value, num_heads, num_kv_heads, head_dim),
            |b, (q, k, v, nh, nkv, hd)| {
                b.iter(|| {
                    ctx.rope_then_attention(
                        black_box(*q),
                        black_box(*k),
                        black_box(*v),
                        *nh,
                        *nkv,
                        *hd,
                        0,
                        rope_theta,
                        true,
                    )
                })
            },
        );
    }

    group.finish();
}

#[cfg(all(target_os = "macos", feature = "metal-compute"))]
fn bench_swiglu_metal(c: &mut Criterion) {
    let ctx = match MetalContext::new(MetalConfig::default()) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to create Metal context: {}", e);
            return;
        }
    };

    if ctx
        .available_optimizations()
        .iter()
        .find(|&&s| s == "fused_swiglu")
        .is_none()
    {
        eprintln!("Fused SwiGLU not available, skipping benchmark");
        return;
    }

    let mut group = c.benchmark_group("metal_swiglu");

    for size in [1024, 4096, 11008, 14336] {
        let gate: Vec<f32> = (0..size).map(|i| (i as f32) * 0.001 - 0.5).collect();
        let up: Vec<f32> = (0..size).map(|i| (i as f32) * 0.001).collect();

        // Fused SwiGLU
        group.bench_with_input(
            BenchmarkId::new("fused", format!("size{}", size)),
            &(&gate, &up),
            |b, (g, u)| b.iter(|| ctx.fused_swiglu(black_box(*g), black_box(*u))),
        );

        // CPU baseline for comparison
        group.bench_with_input(
            BenchmarkId::new("cpu_baseline", format!("size{}", size)),
            &(&gate, &up),
            |b, (g, u)| {
                b.iter(|| {
                    let result: Vec<f32> = g
                        .iter()
                        .zip(u.iter())
                        .map(|(&g_val, &u_val)| {
                            // SwiGLU: swish(gate) * up
                            let swish = g_val / (1.0 + (-g_val).exp());
                            swish * u_val
                        })
                        .collect();
                    black_box(result)
                })
            },
        );
    }

    group.finish();
}

// CPU baseline comparison
fn bench_cpu_gemm(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_gemm");

    for size in [128, 256, 512] {
        let m = size;
        let n = size;
        let k = size;

        let a: Vec<f32> = (0..m * k).map(|i| (i as f32) * 0.001).collect();
        let b: Vec<f32> = (0..k * n).map(|i| (i as f32) * 0.001).collect();

        group.bench_with_input(
            BenchmarkId::new("naive", format!("{}x{}", size, size)),
            &(&a, &b, m, n, k),
            |bench, (a, b, m, n, k)| {
                bench.iter(|| {
                    let mut c = vec![0.0f32; *m * *n];
                    for i in 0..*m {
                        for j in 0..*n {
                            let mut sum = 0.0f32;
                            for l in 0..*k {
                                sum += a[i * *k + l] * b[l * *n + j];
                            }
                            c[i * *n + j] = sum;
                        }
                    }
                    black_box(c)
                })
            },
        );
    }

    group.finish();
}

#[cfg(all(target_os = "macos", feature = "metal-compute"))]
criterion_group!(
    metal_benches,
    // Legacy benchmarks
    bench_flash_attention_metal,
    bench_gemm_metal,
    bench_rms_norm_metal,
    bench_rope_metal,
    // M4 Pro optimized benchmarks
    bench_optimized_gemm_metal,
    bench_fused_attention_metal,
    bench_fused_norm_residual_metal,
    bench_rope_attention_fusion_metal,
    bench_swiglu_metal,
    // CPU baseline
    bench_cpu_gemm,
);

#[cfg(not(all(target_os = "macos", feature = "metal-compute")))]
criterion_group!(metal_benches, bench_cpu_gemm,);

criterion_main!(metal_benches);
