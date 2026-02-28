//! Benchmark: Lorentz Cascade Attention vs Poincaré Attention
//!
//! Run with: cargo bench -p ruvector-attention --bench hyperbolic_bench

use std::time::Instant;

// Import both attention mechanisms
use ruvector_attention::hyperbolic::{
    busemann_score,
    einstein_midpoint,
    frechet_mean,
    lorentz_distance,
    // Poincaré (baseline)
    poincare_distance,
    project_hyperboloid,
    HyperbolicAttention,
    HyperbolicAttentionConfig,
    LCAConfig,
    // Lorentz Cascade (novel)
    LorentzCascadeAttention,
};

fn generate_test_data(n: usize, dim: usize) -> (Vec<f32>, Vec<Vec<f32>>) {
    let query: Vec<f32> = (0..dim)
        .map(|i| ((i as f32 * 0.1).sin() * 0.3).clamp(-0.9, 0.9))
        .collect();

    let keys: Vec<Vec<f32>> = (0..n)
        .map(|j| {
            (0..dim)
                .map(|i| (((i + j) as f32 * 0.07).cos() * 0.3).clamp(-0.9, 0.9))
                .collect()
        })
        .collect();

    (query, keys)
}

fn bench_poincare_distance(iterations: usize, n_keys: usize, dim: usize) -> std::time::Duration {
    let (query, keys) = generate_test_data(n_keys, dim);
    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

    let start = Instant::now();
    for _ in 0..iterations {
        for key in &keys_refs {
            let _d = poincare_distance(&query, key, 1.0);
        }
    }
    start.elapsed()
}

fn bench_lorentz_distance(iterations: usize, n_keys: usize, dim: usize) -> std::time::Duration {
    let (query, keys) = generate_test_data(n_keys, dim + 1); // +1 for time dimension
    let query_h = project_hyperboloid(&query, 1.0);
    let keys_h: Vec<Vec<f32>> = keys.iter().map(|k| project_hyperboloid(k, 1.0)).collect();

    let start = Instant::now();
    for _ in 0..iterations {
        for key in &keys_h {
            let _d = lorentz_distance(&query_h, key, 1.0);
        }
    }
    start.elapsed()
}

fn bench_busemann_scoring(iterations: usize, n_keys: usize, dim: usize) -> std::time::Duration {
    let (query, keys) = generate_test_data(n_keys, dim + 1);
    let focal: Vec<f32> = {
        let mut f = vec![1.0];
        f.extend(vec![0.0; dim]);
        f[1] = 1.0; // Light-like
        f
    };
    let query_h = project_hyperboloid(&query, 1.0);
    let keys_h: Vec<Vec<f32>> = keys.iter().map(|k| project_hyperboloid(k, 1.0)).collect();

    let start = Instant::now();
    for _ in 0..iterations {
        for key in &keys_h {
            let _score = busemann_score(key, &focal) - busemann_score(&query_h, &focal);
        }
    }
    start.elapsed()
}

fn bench_frechet_mean(iterations: usize, n_points: usize, dim: usize) -> std::time::Duration {
    let (_, points) = generate_test_data(n_points, dim);
    let points_refs: Vec<&[f32]> = points.iter().map(|p| p.as_slice()).collect();

    let start = Instant::now();
    for _ in 0..iterations {
        let _mean = frechet_mean(&points_refs, None, 1.0, 50, 1e-5);
    }
    start.elapsed()
}

fn bench_einstein_midpoint(iterations: usize, n_points: usize, dim: usize) -> std::time::Duration {
    let (_, points) = generate_test_data(n_points, dim + 1);
    let points_h: Vec<Vec<f32>> = points.iter().map(|p| project_hyperboloid(p, 1.0)).collect();
    let points_refs: Vec<&[f32]> = points_h.iter().map(|p| p.as_slice()).collect();
    let weights: Vec<f32> = vec![1.0 / n_points as f32; n_points];

    let start = Instant::now();
    for _ in 0..iterations {
        let _mid = einstein_midpoint(&points_refs, &weights, 1.0);
    }
    start.elapsed()
}

fn bench_full_poincare_attention(
    iterations: usize,
    n_keys: usize,
    dim: usize,
) -> std::time::Duration {
    let (query, keys) = generate_test_data(n_keys, dim);
    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

    let config = HyperbolicAttentionConfig {
        dim,
        curvature: -1.0,
        adaptive_curvature: false,
        temperature: 1.0,
        frechet_max_iter: 50,
        frechet_tol: 1e-5,
    };
    let attention = HyperbolicAttention::new(config);

    let start = Instant::now();
    for _ in 0..iterations {
        let _result = attention.compute_weights(&query, &keys_refs);
    }
    start.elapsed()
}

fn bench_full_lca_attention(iterations: usize, n_keys: usize, dim: usize) -> std::time::Duration {
    let (query, keys) = generate_test_data(n_keys, dim);
    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

    let config = LCAConfig {
        dim,
        num_heads: 4,
        curvature_range: (0.1, 2.0),
        temperature: 1.0,
    };
    let attention = LorentzCascadeAttention::new(config);

    let start = Instant::now();
    for _ in 0..iterations {
        let _result = attention.attend(&query, &keys_refs, &keys_refs);
    }
    start.elapsed()
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║     Lorentz Cascade Attention (LCA) vs Poincaré Benchmark        ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    let iterations = 1000;
    let n_keys = 100;
    let dim = 64;

    println!(
        "Configuration: {} iterations, {} keys, {} dimensions\n",
        iterations, n_keys, dim
    );

    // Distance computation benchmarks
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ 1. DISTANCE COMPUTATION                                         │");
    println!("├─────────────────────────────────────────────────────────────────┤");

    let poincare_dist_time = bench_poincare_distance(iterations, n_keys, dim);
    let lorentz_dist_time = bench_lorentz_distance(iterations, n_keys, dim);
    let busemann_time = bench_busemann_scoring(iterations, n_keys, dim);

    let poincare_per_op = poincare_dist_time.as_nanos() as f64 / (iterations * n_keys) as f64;
    let lorentz_per_op = lorentz_dist_time.as_nanos() as f64 / (iterations * n_keys) as f64;
    let busemann_per_op = busemann_time.as_nanos() as f64 / (iterations * n_keys) as f64;

    println!(
        "│ Poincaré distance:     {:>8.1} ns/op                          │",
        poincare_per_op
    );
    println!(
        "│ Lorentz distance:      {:>8.1} ns/op  ({:.1}x vs Poincaré)      │",
        lorentz_per_op,
        poincare_per_op / lorentz_per_op
    );
    println!(
        "│ Busemann scoring:      {:>8.1} ns/op  ({:.1}x vs Poincaré)      │",
        busemann_per_op,
        poincare_per_op / busemann_per_op
    );
    println!("└─────────────────────────────────────────────────────────────────┘\n");

    // Aggregation benchmarks
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ 2. AGGREGATION (CENTROID)                                       │");
    println!("├─────────────────────────────────────────────────────────────────┤");

    let frechet_time = bench_frechet_mean(iterations / 10, n_keys, dim); // Fewer iterations (slow)
    let einstein_time = bench_einstein_midpoint(iterations, n_keys, dim);

    let frechet_per_op = frechet_time.as_nanos() as f64 / (iterations / 10) as f64;
    let einstein_per_op = einstein_time.as_nanos() as f64 / iterations as f64;

    println!(
        "│ Fréchet mean (50 iter): {:>10.1} ns/op                       │",
        frechet_per_op
    );
    println!(
        "│ Einstein midpoint:      {:>10.1} ns/op  ({:.1}x faster!)      │",
        einstein_per_op,
        frechet_per_op / einstein_per_op
    );
    println!("└─────────────────────────────────────────────────────────────────┘\n");

    // Full attention benchmarks
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ 3. FULL ATTENTION (END-TO-END)                                  │");
    println!("├─────────────────────────────────────────────────────────────────┤");

    let poincare_full_time = bench_full_poincare_attention(iterations / 10, n_keys, dim);
    let lca_full_time = bench_full_lca_attention(iterations / 10, n_keys, dim);

    let poincare_full_per_op = poincare_full_time.as_nanos() as f64 / (iterations / 10) as f64;
    let lca_full_per_op = lca_full_time.as_nanos() as f64 / (iterations / 10) as f64;

    println!(
        "│ Poincaré Attention:     {:>10.1} ns/op                       │",
        poincare_full_per_op
    );
    println!(
        "│ Lorentz Cascade (4 heads): {:>7.1} ns/op  ({:.1}x speedup)   │",
        lca_full_per_op,
        poincare_full_per_op / lca_full_per_op
    );
    println!("└─────────────────────────────────────────────────────────────────┘\n");

    // Summary
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║ SUMMARY: Lorentz Cascade Attention Improvements                  ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!(
        "║ • Busemann scoring:    {:.1}x faster than Poincaré distance       ║",
        poincare_per_op / busemann_per_op
    );
    println!(
        "║ • Einstein midpoint:   {:.1}x faster than Fréchet mean           ║",
        frechet_per_op / einstein_per_op
    );
    println!(
        "║ • End-to-end:          {:.1}x overall speedup                     ║",
        poincare_full_per_op / lca_full_per_op
    );
    println!("║                                                                  ║");
    println!("║ Additional benefits:                                             ║");
    println!("║ • No boundary instability (Lorentz vs Poincaré ball)            ║");
    println!("║ • Multi-scale hierarchy (4 curvature heads)                     ║");
    println!("║ • Sparse attention via hierarchical pruning                     ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
}
