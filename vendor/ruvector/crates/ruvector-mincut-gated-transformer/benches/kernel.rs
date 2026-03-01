//! Kernel benchmarks for low-level operations.
//!
//! Tests GEMM, INT4 quantization, arena allocation, and SIMD operations.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ruvector_mincut_gated_transformer::arena::{calculate_arena_size, WeightArena};
use ruvector_mincut_gated_transformer::kernel::{
    compute_gflops, dequantize_int4_to_f32, int4_gemm, int4_gemv, layer_norm, pack_int4, qgemm_i8,
    qgemm_i8_simd, quantize_f32_to_int4, rms_norm, unpack_int4, BenchStats, Int4Weights, Timer,
};

// ============================================================================
// INT8 GEMM Benchmarks
// ============================================================================

fn bench_qgemm_i8_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("qgemm_i8");

    for size in [64, 128, 256].iter() {
        let m = *size;
        let n = *size;
        let k = *size;

        let a: Vec<i8> = (0..m * k).map(|i| ((i as i16 % 256 - 128) as i8)).collect();
        let b: Vec<i8> = (0..n * k).map(|i| ((i as i16 % 256 - 128) as i8)).collect();
        let b_scales: Vec<f32> = vec![1.0 / 128.0; n];

        let ops = 2 * m * n * k;
        group.throughput(Throughput::Elements(ops as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |bench, _| {
            let mut c_out = vec![0i32; m * n];
            bench.iter(|| {
                qgemm_i8(m, n, k, &a, 1.0 / 128.0, &b, &b_scales, None, &mut c_out);
                black_box(c_out[0])
            })
        });

        group.bench_with_input(BenchmarkId::new("simd", size), size, |bench, _| {
            let mut c_out = vec![0i32; m * n];
            bench.iter(|| {
                qgemm_i8_simd(m, n, k, &a, 1.0 / 128.0, &b, &b_scales, None, &mut c_out);
                black_box(c_out[0])
            })
        });
    }

    group.finish();
}

fn bench_qgemv(c: &mut Criterion) {
    let mut group = c.benchmark_group("qgemv");

    for size in [128, 256, 512].iter() {
        let n = *size;
        let k = *size;

        let a: Vec<i8> = (0..k).map(|i| ((i as i16 % 256 - 128) as i8)).collect();
        let b: Vec<i8> = (0..n * k).map(|i| ((i as i16 % 256 - 128) as i8)).collect();
        let b_scales: Vec<f32> = vec![1.0 / 128.0; n];

        group.throughput(Throughput::Elements((2 * n * k) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, _| {
            let mut c_out = vec![0i32; n];
            bench.iter(|| {
                qgemm_i8_simd(1, n, k, &a, 1.0 / 128.0, &b, &b_scales, None, &mut c_out);
                black_box(c_out[0])
            })
        });
    }

    group.finish();
}

// ============================================================================
// INT4 Quantization Benchmarks
// ============================================================================

fn bench_int4_pack_unpack(c: &mut Criterion) {
    let mut group = c.benchmark_group("int4_pack_unpack");

    group.bench_function("pack_single", |b| {
        b.iter(|| {
            let packed = pack_int4(black_box(5), black_box(-3));
            black_box(packed)
        })
    });

    group.bench_function("unpack_single", |b| {
        let packed = pack_int4(5, -3);
        b.iter(|| {
            let (v0, v1) = unpack_int4(black_box(packed));
            black_box((v0, v1))
        })
    });

    // Bulk operations
    for count in [256, 1024, 4096].iter() {
        let values: Vec<f32> = (0..*count)
            .map(|i| (i as f32 - *count as f32 / 2.0) / 100.0)
            .collect();
        group.throughput(Throughput::Elements(*count as u64));

        group.bench_with_input(BenchmarkId::new("quantize", count), count, |bench, cnt| {
            let mut packed = vec![0u8; (*cnt + 1) / 2];
            bench.iter(|| {
                let scale = quantize_f32_to_int4(&values, &mut packed);
                black_box(scale)
            })
        });

        group.bench_with_input(
            BenchmarkId::new("dequantize", count),
            count,
            |bench, cnt| {
                let mut packed = vec![0u8; (*cnt + 1) / 2];
                let scale = quantize_f32_to_int4(&values, &mut packed);
                let mut output = vec![0.0f32; *cnt];
                bench.iter(|| {
                    dequantize_int4_to_f32(&packed, scale, *cnt, &mut output);
                    black_box(output[0])
                })
            },
        );
    }

    group.finish();
}

fn bench_int4_weights(c: &mut Criterion) {
    let mut group = c.benchmark_group("int4_weights");

    for (rows, cols) in [(256, 256), (512, 512), (768, 768)].iter() {
        let weights: Vec<f32> = (0..rows * cols)
            .map(|i| ((i % 200) as f32 - 100.0) / 100.0)
            .collect();

        group.throughput(Throughput::Bytes((*rows * *cols * 4) as u64));

        group.bench_with_input(
            BenchmarkId::new("from_f32", format!("{}x{}", rows, cols)),
            &(*rows, *cols),
            |bench, (r, c)| {
                bench.iter(|| {
                    let int4_w = Int4Weights::from_f32(&weights, *r, *c);
                    black_box(int4_w.memory_bytes())
                })
            },
        );
    }

    group.finish();
}

fn bench_int4_gemv(c: &mut Criterion) {
    let mut group = c.benchmark_group("int4_gemv");

    for size in [256, 512, 768].iter() {
        let n = *size;
        let k = *size;

        let weights: Vec<f32> = (0..n * k)
            .map(|i| ((i % 200) as f32 - 100.0) / 100.0)
            .collect();
        let int4_w = Int4Weights::from_f32(&weights, n, k);
        let x: Vec<f32> = (0..k).map(|i| (i as f32) / k as f32).collect();

        let ops = 2 * n * k;
        group.throughput(Throughput::Elements(ops as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, sz| {
            let mut y = vec![0.0f32; *sz];
            bench.iter(|| {
                int4_gemv(&int4_w, &x, 1.0, &mut y);
                black_box(y[0])
            })
        });
    }

    group.finish();
}

fn bench_int4_gemm(c: &mut Criterion) {
    let mut group = c.benchmark_group("int4_gemm");

    for (m, n, k) in [(32, 256, 256), (64, 512, 512)].iter() {
        let weights: Vec<f32> = (0..n * k)
            .map(|i| ((i % 200) as f32 - 100.0) / 100.0)
            .collect();
        let int4_w = Int4Weights::from_f32(&weights, *n, *k);
        let a: Vec<f32> = (0..m * k).map(|i| (i as f32) / (m * k) as f32).collect();

        let ops = 2 * m * n * k;
        group.throughput(Throughput::Elements(ops as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x{}x{}", m, n, k)),
            &(*m, *n, *k),
            |bench, (batch, nn, _)| {
                let mut c_out = vec![0.0f32; *batch * *nn];
                bench.iter(|| {
                    int4_gemm(&int4_w, &a, 1.0, *batch, &mut c_out);
                    black_box(c_out[0])
                })
            },
        );
    }

    group.finish();
}

fn bench_int4_memory_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("int4_vs_int8_memory");

    for size in [256, 512].iter() {
        let n = *size;
        let k = *size;
        let total_weights = n * k;

        // INT8 baseline
        let weights_i8: Vec<i8> = (0..total_weights)
            .map(|i| (i as i16 % 256 - 128) as i8)
            .collect();
        let b_scales: Vec<f32> = vec![1.0 / 128.0; n];
        let x_i8: Vec<i8> = (0..k).map(|i| (i as i16 % 256 - 128) as i8).collect();

        // INT4
        let weights_f32: Vec<f32> = (0..total_weights)
            .map(|i| ((i % 200) as f32 - 100.0) / 100.0)
            .collect();
        let int4_w = Int4Weights::from_f32(&weights_f32, n, k);
        let x_f32: Vec<f32> = (0..k).map(|i| i as f32 / k as f32).collect();

        group.bench_with_input(BenchmarkId::new("int8_gemv", size), size, |bench, sz| {
            let mut y_i8 = vec![0i32; *sz];
            bench.iter(|| {
                qgemm_i8_simd(
                    1,
                    n,
                    k,
                    &x_i8,
                    1.0 / 128.0,
                    &weights_i8,
                    &b_scales,
                    None,
                    &mut y_i8,
                );
                black_box(y_i8[0])
            })
        });

        group.bench_with_input(BenchmarkId::new("int4_gemv", size), size, |bench, sz| {
            let mut y_f32 = vec![0.0f32; *sz];
            bench.iter(|| {
                int4_gemv(&int4_w, &x_f32, 1.0, &mut y_f32);
                black_box(y_f32[0])
            })
        });
    }

    group.finish();
}

// ============================================================================
// Normalization Benchmarks
// ============================================================================

fn bench_layer_norm(c: &mut Criterion) {
    let mut group = c.benchmark_group("layer_norm");

    for size in [128, 256, 512, 768].iter() {
        let input: Vec<f32> = (0..*size)
            .map(|i| (i as f32 - *size as f32 / 2.0) / 100.0)
            .collect();
        let gamma: Vec<f32> = vec![1.0f32; *size];
        let beta: Vec<f32> = vec![0.0f32; *size];

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("layer_norm", size), size, |bench, sz| {
            let mut output = vec![0.0f32; *sz];
            bench.iter(|| {
                layer_norm(&input, &gamma, &beta, 1e-5, &mut output);
                black_box(output[0])
            })
        });

        group.bench_with_input(BenchmarkId::new("rms_norm", size), size, |bench, sz| {
            let mut output = vec![0.0f32; *sz];
            bench.iter(|| {
                rms_norm(&input, &gamma, 1e-5, &mut output);
                black_box(output[0])
            })
        });
    }

    group.finish();
}

// ============================================================================
// Arena Allocator Benchmarks
// ============================================================================

fn bench_arena_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_alloc");

    for size_kb in [64, 256, 1024].iter() {
        let size = size_kb * 1024;

        group.bench_with_input(
            BenchmarkId::new("create", format!("{}KB", size_kb)),
            &size,
            |bench, sz| {
                bench.iter(|| {
                    let arena = WeightArena::new(black_box(*sz));
                    black_box(arena.capacity())
                })
            },
        );
    }

    // Allocation patterns
    let arena_size = 1024 * 1024; // 1MB

    group.bench_function("alloc_i8_1024", |b| {
        b.iter(|| {
            let mut arena = WeightArena::new(arena_size);
            for _ in 0..100 {
                let _ = arena.alloc_i8(black_box(1024));
            }
            black_box(arena.offset())
        })
    });

    group.bench_function("alloc_f32_256", |b| {
        b.iter(|| {
            let mut arena = WeightArena::new(arena_size);
            for _ in 0..100 {
                let _ = arena.alloc_f32(black_box(256));
            }
            black_box(arena.offset())
        })
    });

    group.bench_function("alloc_mixed", |b| {
        b.iter(|| {
            let mut arena = WeightArena::new(arena_size);
            for i in 0..50 {
                if i % 2 == 0 {
                    let _ = arena.alloc_i8(black_box(1024));
                } else {
                    let _ = arena.alloc_f32(black_box(256));
                }
            }
            black_box(arena.offset())
        })
    });

    group.bench_function("reset_reuse", |b| {
        let mut arena = WeightArena::new(arena_size);
        b.iter(|| {
            arena.reset();
            for _ in 0..100 {
                let _ = arena.alloc_i8(black_box(1024));
            }
            black_box(arena.offset())
        })
    });

    group.finish();
}

fn bench_arena_size_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_size_calc");

    for (layers, hidden) in [(4, 256), (12, 768), (24, 1024)].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}L_{}H", layers, hidden)),
            &(*layers, *hidden),
            |bench, (l, h)| {
                bench.iter(|| {
                    let size = calculate_arena_size(black_box(*l), black_box(*h), 4, 8);
                    black_box(size)
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Timer/Stats Utilities Benchmarks
// ============================================================================

fn bench_timer_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("timer_overhead");

    group.bench_function("timer_start_stop", |b| {
        b.iter(|| {
            let mut timer = Timer::new();
            timer.start();
            timer.stop();
            black_box(timer.elapsed_ns())
        })
    });

    group.bench_function("bench_stats_record", |b| {
        let mut stats = BenchStats::new(1000);
        b.iter(|| {
            stats.add_sample(black_box(100));
            black_box(stats.min_ns())
        })
    });

    group.bench_function("compute_gflops", |b| {
        b.iter(|| {
            let gflops = compute_gflops(black_box(2_000_000_000), black_box(1_000_000));
            black_box(gflops)
        })
    });

    group.finish();
}

// ============================================================================
// Combined Workload Benchmarks
// ============================================================================

fn bench_transformer_layer_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("layer_simulation");

    let hidden = 256;
    let ffn_hidden = hidden * 4;

    let q_weights: Vec<i8> = (0..hidden * hidden)
        .map(|i| ((i as i16 % 256 - 128) as i8))
        .collect();
    let k_weights: Vec<i8> = (0..hidden * hidden)
        .map(|i| ((i as i16 % 256 - 128) as i8))
        .collect();
    let v_weights: Vec<i8> = (0..hidden * hidden)
        .map(|i| ((i as i16 % 256 - 128) as i8))
        .collect();
    let ffn_up: Vec<i8> = (0..hidden * ffn_hidden)
        .map(|i| ((i as i16 % 256 - 128) as i8))
        .collect();
    let ffn_down: Vec<i8> = (0..ffn_hidden * hidden)
        .map(|i| ((i as i16 % 256 - 128) as i8))
        .collect();

    let q_scales: Vec<f32> = vec![1.0 / 128.0; hidden];
    let k_scales: Vec<f32> = vec![1.0 / 128.0; hidden];
    let v_scales: Vec<f32> = vec![1.0 / 128.0; hidden];
    let ffn_up_scales: Vec<f32> = vec![1.0 / 128.0; ffn_hidden];
    let ffn_down_scales: Vec<f32> = vec![1.0 / 128.0; hidden];

    let input: Vec<i8> = (0..hidden)
        .map(|i| ((i as i16 % 256 - 128) as i8))
        .collect();

    group.bench_function("qkv_projection", |b| {
        let mut q_out = vec![0i32; hidden];
        let mut k_out = vec![0i32; hidden];
        let mut v_out = vec![0i32; hidden];
        b.iter(|| {
            qgemm_i8_simd(
                1,
                hidden,
                hidden,
                &input,
                1.0 / 128.0,
                &q_weights,
                &q_scales,
                None,
                &mut q_out,
            );
            qgemm_i8_simd(
                1,
                hidden,
                hidden,
                &input,
                1.0 / 128.0,
                &k_weights,
                &k_scales,
                None,
                &mut k_out,
            );
            qgemm_i8_simd(
                1,
                hidden,
                hidden,
                &input,
                1.0 / 128.0,
                &v_weights,
                &v_scales,
                None,
                &mut v_out,
            );
            black_box((q_out[0], k_out[0], v_out[0]))
        })
    });

    group.bench_function("ffn_forward", |b| {
        let mut ffn_mid = vec![0i32; ffn_hidden];
        let mut out = vec![0i32; hidden];
        b.iter(|| {
            qgemm_i8_simd(
                1,
                ffn_hidden,
                hidden,
                &input,
                1.0 / 128.0,
                &ffn_up,
                &ffn_up_scales,
                None,
                &mut ffn_mid,
            );
            let ffn_mid_i8: Vec<i8> = ffn_mid
                .iter()
                .map(|&x| (x >> 8).clamp(-128, 127) as i8)
                .collect();
            qgemm_i8_simd(
                1,
                hidden,
                ffn_hidden,
                &ffn_mid_i8,
                1.0 / 128.0,
                &ffn_down,
                &ffn_down_scales,
                None,
                &mut out,
            );
            black_box(out[0])
        })
    });

    group.bench_function("layer_norm_overhead", |b| {
        let input_f32: Vec<f32> = input.iter().map(|&x| x as f32 / 128.0).collect();
        let gamma_f32: Vec<f32> = vec![1.0f32; hidden];
        let beta_f32: Vec<f32> = vec![0.0f32; hidden];
        let mut output = vec![0.0f32; hidden];
        b.iter(|| {
            layer_norm(&input_f32, &gamma_f32, &beta_f32, 1e-5, &mut output);
            black_box(output[0])
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_qgemm_i8_sizes,
    bench_qgemv,
    bench_int4_pack_unpack,
    bench_int4_weights,
    bench_int4_gemv,
    bench_int4_gemm,
    bench_int4_memory_comparison,
    bench_layer_norm,
    bench_arena_allocation,
    bench_arena_size_calculation,
    bench_timer_overhead,
    bench_transformer_layer_simulation,
);

criterion_main!(benches);
