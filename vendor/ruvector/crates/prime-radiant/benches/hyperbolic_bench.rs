//! Benchmarks for Poincare distance computation
//!
//! ADR-014 Performance Target: < 500ns per Poincare distance
//!
//! Hyperbolic geometry enables hierarchy-aware coherence where
//! deeper nodes (further from origin) have different energy weights.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ============================================================================
// Hyperbolic Geometry Functions
// ============================================================================

/// Compute squared Euclidean norm
#[inline]
fn squared_norm(x: &[f32]) -> f32 {
    x.iter().map(|v| v * v).sum()
}

/// Compute Euclidean norm
#[inline]
fn norm(x: &[f32]) -> f32 {
    squared_norm(x).sqrt()
}

/// Compute squared Euclidean distance
#[inline]
fn squared_distance(x: &[f32], y: &[f32]) -> f32 {
    x.iter().zip(y.iter()).map(|(a, b)| (a - b).powi(2)).sum()
}

/// Poincare distance in the Poincare ball model
///
/// d(x, y) = arcosh(1 + 2 * ||x - y||^2 / ((1 - ||x||^2) * (1 - ||y||^2)))
///
/// where arcosh(z) = ln(z + sqrt(z^2 - 1))
#[inline]
pub fn poincare_distance(x: &[f32], y: &[f32], curvature: f32) -> f32 {
    let sq_norm_x = squared_norm(x);
    let sq_norm_y = squared_norm(y);
    let sq_dist = squared_distance(x, y);

    // Clamp to valid range for numerical stability
    let denom = (1.0 - sq_norm_x).max(1e-10) * (1.0 - sq_norm_y).max(1e-10);
    let arg = 1.0 + 2.0 * sq_dist / denom;

    // arcosh(arg) = ln(arg + sqrt(arg^2 - 1))
    let arcosh = (arg + (arg * arg - 1.0).max(0.0).sqrt()).ln();

    // Scale by curvature
    arcosh / (-curvature).sqrt()
}

/// Optimized Poincare distance with fused operations
#[inline]
pub fn poincare_distance_optimized(x: &[f32], y: &[f32], curvature: f32) -> f32 {
    let mut sq_norm_x = 0.0f32;
    let mut sq_norm_y = 0.0f32;
    let mut sq_dist = 0.0f32;

    for i in 0..x.len() {
        sq_norm_x += x[i] * x[i];
        sq_norm_y += y[i] * y[i];
        let d = x[i] - y[i];
        sq_dist += d * d;
    }

    let denom = (1.0 - sq_norm_x).max(1e-10) * (1.0 - sq_norm_y).max(1e-10);
    let arg = 1.0 + 2.0 * sq_dist / denom;
    let arcosh = (arg + (arg * arg - 1.0).max(0.0).sqrt()).ln();

    arcosh / (-curvature).sqrt()
}

/// SIMD-friendly Poincare distance (chunked)
#[inline]
pub fn poincare_distance_simd_friendly(x: &[f32], y: &[f32], curvature: f32) -> f32 {
    // Process in chunks of 4 for potential auto-vectorization
    let mut sq_norm_x = [0.0f32; 4];
    let mut sq_norm_y = [0.0f32; 4];
    let mut sq_dist = [0.0f32; 4];

    let chunks = x.len() / 4;
    for c in 0..chunks {
        let base = c * 4;
        for i in 0..4 {
            let xi = x[base + i];
            let yi = y[base + i];
            sq_norm_x[i] += xi * xi;
            sq_norm_y[i] += yi * yi;
            let d = xi - yi;
            sq_dist[i] += d * d;
        }
    }

    // Handle remainder
    let remainder = x.len() % 4;
    let base = chunks * 4;
    for i in 0..remainder {
        let xi = x[base + i];
        let yi = y[base + i];
        sq_norm_x[0] += xi * xi;
        sq_norm_y[0] += yi * yi;
        let d = xi - yi;
        sq_dist[0] += d * d;
    }

    // Reduce
    let total_sq_norm_x: f32 = sq_norm_x.iter().sum();
    let total_sq_norm_y: f32 = sq_norm_y.iter().sum();
    let total_sq_dist: f32 = sq_dist.iter().sum();

    let denom = (1.0 - total_sq_norm_x).max(1e-10) * (1.0 - total_sq_norm_y).max(1e-10);
    let arg = 1.0 + 2.0 * total_sq_dist / denom;
    let arcosh = (arg + (arg * arg - 1.0).max(0.0).sqrt()).ln();

    arcosh / (-curvature).sqrt()
}

/// Mobius addition in the Poincare ball
///
/// x + y = ((1 + 2<x,y> + ||y||^2)x + (1 - ||x||^2)y) / (1 + 2<x,y> + ||x||^2||y||^2)
pub fn mobius_add(x: &[f32], y: &[f32], curvature: f32) -> Vec<f32> {
    let c = -curvature;
    let sq_norm_x = squared_norm(x);
    let sq_norm_y = squared_norm(y);
    let xy_dot: f32 = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum();

    let num_factor_x = 1.0 + 2.0 * c * xy_dot + c * sq_norm_y;
    let num_factor_y = 1.0 - c * sq_norm_x;
    let denom = 1.0 + 2.0 * c * xy_dot + c * c * sq_norm_x * sq_norm_y;

    x.iter()
        .zip(y.iter())
        .map(|(xi, yi)| (num_factor_x * xi + num_factor_y * yi) / denom)
        .collect()
}

/// Exponential map at point p with tangent vector v
pub fn exp_map(v: &[f32], p: &[f32], curvature: f32) -> Vec<f32> {
    let c = -curvature;
    let v_norm = norm(v);

    if v_norm < 1e-10 {
        return p.to_vec();
    }

    let lambda_p = 2.0 / (1.0 - c * squared_norm(p)).max(1e-10);
    let t = (c.sqrt() * lambda_p * v_norm / 2.0).tanh();
    let factor = t / (c.sqrt() * v_norm);

    let v_scaled: Vec<f32> = v.iter().map(|vi| factor * vi).collect();
    mobius_add(p, &v_scaled, curvature)
}

/// Logarithmic map from point p to point q
pub fn log_map(q: &[f32], p: &[f32], curvature: f32) -> Vec<f32> {
    let c = -curvature;

    // Compute -p + q
    let neg_p: Vec<f32> = p.iter().map(|x| -x).collect();
    let diff = mobius_add(&neg_p, q, curvature);

    let diff_norm = norm(&diff);
    if diff_norm < 1e-10 {
        return vec![0.0; p.len()];
    }

    let lambda_p = 2.0 / (1.0 - c * squared_norm(p)).max(1e-10);
    let factor = 2.0 / (c.sqrt() * lambda_p) * (c.sqrt() * diff_norm).atanh() / diff_norm;

    diff.iter().map(|d| factor * d).collect()
}

/// Project vector to Poincare ball (ensure ||x|| < 1/sqrt(c))
pub fn project_to_ball(x: &[f32], curvature: f32) -> Vec<f32> {
    let max_norm = 1.0 / (-curvature).sqrt() - 1e-5;
    let current_norm = norm(x);

    if current_norm >= max_norm {
        let scale = max_norm / current_norm;
        x.iter().map(|v| v * scale).collect()
    } else {
        x.to_vec()
    }
}

/// Compute depth (distance from origin) in Poincare ball
#[inline]
pub fn poincare_depth(x: &[f32], curvature: f32) -> f32 {
    let origin = vec![0.0f32; x.len()];
    poincare_distance(x, &origin, curvature)
}

// ============================================================================
// Test Data Generation
// ============================================================================

fn generate_point(dim: usize, seed: u64, max_norm: f32) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let raw: Vec<f32> = (0..dim)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            (seed, i).hash(&mut hasher);
            (hasher.finish() % 1000) as f32 / 1000.0 - 0.5
        })
        .collect();

    // Scale to be within ball
    let n = norm(&raw);
    if n > 0.0 {
        let scale = max_norm / n * 0.9; // 90% of max
        raw.iter().map(|v| v * scale).collect()
    } else {
        raw
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

/// Benchmark Poincare distance at various dimensions
fn bench_poincare_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_poincare_distance");
    group.throughput(Throughput::Elements(1));

    let curvature = -1.0;

    for dim in [8, 32, 64, 128, 256, 512] {
        let x = generate_point(dim, 42, 0.9);
        let y = generate_point(dim, 123, 0.9);

        // Standard implementation
        group.bench_with_input(BenchmarkId::new("standard", dim), &dim, |b, _| {
            b.iter(|| poincare_distance(black_box(&x), black_box(&y), black_box(curvature)))
        });

        // Optimized implementation
        group.bench_with_input(BenchmarkId::new("optimized", dim), &dim, |b, _| {
            b.iter(|| {
                poincare_distance_optimized(black_box(&x), black_box(&y), black_box(curvature))
            })
        });

        // SIMD-friendly implementation
        group.bench_with_input(BenchmarkId::new("simd_friendly", dim), &dim, |b, _| {
            b.iter(|| {
                poincare_distance_simd_friendly(black_box(&x), black_box(&y), black_box(curvature))
            })
        });
    }

    group.finish();
}

/// Benchmark Mobius addition
fn bench_mobius_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_mobius_add");
    group.throughput(Throughput::Elements(1));

    let curvature = -1.0;

    for dim in [8, 32, 64, 128] {
        let x = generate_point(dim, 42, 0.5);
        let y = generate_point(dim, 123, 0.5);

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, _| {
            b.iter(|| mobius_add(black_box(&x), black_box(&y), black_box(curvature)))
        });
    }

    group.finish();
}

/// Benchmark exp/log maps
fn bench_exp_log_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_exp_log");

    let dim = 32;
    let curvature = -1.0;

    let p = generate_point(dim, 42, 0.3);
    let v: Vec<f32> = (0..dim).map(|i| ((i as f32 * 0.1).sin() * 0.2)).collect();
    let q = generate_point(dim, 123, 0.4);

    group.bench_function("exp_map", |b| {
        b.iter(|| exp_map(black_box(&v), black_box(&p), black_box(curvature)))
    });

    group.bench_function("log_map", |b| {
        b.iter(|| log_map(black_box(&q), black_box(&p), black_box(curvature)))
    });

    group.finish();
}

/// Benchmark projection to ball
fn bench_projection(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_projection");
    group.throughput(Throughput::Elements(1));

    let curvature = -1.0;

    for dim in [8, 32, 64, 128, 256] {
        // Point that needs projection (outside ball)
        let x: Vec<f32> = (0..dim).map(|i| ((i as f32 * 0.1).sin())).collect();

        group.bench_with_input(BenchmarkId::new("project", dim), &dim, |b, _| {
            b.iter(|| project_to_ball(black_box(&x), black_box(curvature)))
        });
    }

    group.finish();
}

/// Benchmark depth computation
fn bench_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_depth");
    group.throughput(Throughput::Elements(1));

    let curvature = -1.0;

    for dim in [8, 32, 64, 128, 256] {
        let x = generate_point(dim, 42, 0.9);

        group.bench_with_input(BenchmarkId::new("depth", dim), &dim, |b, _| {
            b.iter(|| poincare_depth(black_box(&x), black_box(curvature)))
        });
    }

    group.finish();
}

/// Benchmark batch distance computation
fn bench_batch_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_batch_distance");

    let dim = 64;
    let curvature = -1.0;

    for batch_size in [10, 100, 1000] {
        let points: Vec<Vec<f32>> = (0..batch_size)
            .map(|i| generate_point(dim, i as u64, 0.9))
            .collect();
        let query = generate_point(dim, 999, 0.9);

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_size,
            |b, _| {
                b.iter(|| {
                    let distances: Vec<f32> = points
                        .iter()
                        .map(|p| poincare_distance(&query, p, curvature))
                        .collect();
                    black_box(distances)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark k-nearest in hyperbolic space
fn bench_knn_hyperbolic(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_knn");
    group.sample_size(50);

    let dim = 64;
    let curvature = -1.0;

    let points: Vec<Vec<f32>> = (0..1000)
        .map(|i| generate_point(dim, i as u64, 0.9))
        .collect();
    let query = generate_point(dim, 999, 0.9);

    for k in [1, 5, 10, 50] {
        group.bench_with_input(BenchmarkId::new("k", k), &k, |b, &k| {
            b.iter(|| {
                // Compute all distances
                let mut distances: Vec<(usize, f32)> = points
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (i, poincare_distance(&query, p, curvature)))
                    .collect();

                // Partial sort for k-nearest
                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                let result = distances[..k]
                    .iter()
                    .map(|(i, d)| (*i, *d))
                    .collect::<Vec<_>>();
                black_box(result)
            })
        });
    }

    group.finish();
}

/// Benchmark hierarchy-weighted energy computation
fn bench_hierarchy_weighted_energy(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_hierarchy_energy");

    let dim = 64;
    let curvature = -1.0;

    // Create hierarchy: shallow and deep nodes
    let shallow_nodes: Vec<Vec<f32>> = (0..100)
        .map(|i| generate_point(dim, i as u64, 0.3)) // Near origin
        .collect();
    let deep_nodes: Vec<Vec<f32>> = (0..100)
        .map(|i| generate_point(dim, (i + 100) as u64, 0.9)) // Far from origin
        .collect();

    group.bench_function("shallow_energy", |b| {
        b.iter(|| {
            let mut total_energy = 0.0f32;
            for i in 0..shallow_nodes.len() - 1 {
                let depth_a = poincare_depth(&shallow_nodes[i], curvature);
                let depth_b = poincare_depth(&shallow_nodes[i + 1], curvature);
                let avg_depth = (depth_a + depth_b) / 2.0;
                let weight = 1.0 + avg_depth.ln().max(0.0);

                let dist = poincare_distance(&shallow_nodes[i], &shallow_nodes[i + 1], curvature);
                total_energy += weight * dist * dist;
            }
            black_box(total_energy)
        })
    });

    group.bench_function("deep_energy", |b| {
        b.iter(|| {
            let mut total_energy = 0.0f32;
            for i in 0..deep_nodes.len() - 1 {
                let depth_a = poincare_depth(&deep_nodes[i], curvature);
                let depth_b = poincare_depth(&deep_nodes[i + 1], curvature);
                let avg_depth = (depth_a + depth_b) / 2.0;
                let weight = 1.0 + avg_depth.ln().max(0.0);

                let dist = poincare_distance(&deep_nodes[i], &deep_nodes[i + 1], curvature);
                total_energy += weight * dist * dist;
            }
            black_box(total_energy)
        })
    });

    group.finish();
}

/// Benchmark curvature impact
fn bench_curvature_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_curvature");

    let dim = 64;
    let x = generate_point(dim, 42, 0.5);
    let y = generate_point(dim, 123, 0.5);

    for curvature in [-0.1, -0.5, -1.0, -2.0, -5.0] {
        group.bench_with_input(
            BenchmarkId::new("curvature", format!("{:.1}", curvature)),
            &curvature,
            |b, &c| b.iter(|| poincare_distance(black_box(&x), black_box(&y), black_box(c))),
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_poincare_distance,
    bench_mobius_add,
    bench_exp_log_map,
    bench_projection,
    bench_depth,
    bench_batch_distance,
    bench_knn_hyperbolic,
    bench_hierarchy_weighted_energy,
    bench_curvature_impact,
);

criterion_main!(benches);
