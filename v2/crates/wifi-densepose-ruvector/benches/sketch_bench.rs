//! ADR-084 acceptance criterion benchmark: sketch-vs-float compare cost.
//!
//! Acceptance threshold from `docs/adr/ADR-084-rabitq-similarity-sensor.md`:
//! > Sketch compare cost reduction: **8×–30×** vs full-float compare.
//!
//! This bench measures the per-pair compare cost at the embedding sizes
//! actually used in RuView:
//!
//! - 128-d (AETHER re-ID embeddings, ADR-024)
//! - 256-d (CSI spectrogram embeddings, ADR-076)
//! - 512-d (forward-looking, in case of post-rotation projection)
//!
//! For each dimension, three benches compare:
//!
//! 1. **`float_l2`** — squared-euclidean over `&[f32]` (the baseline; what
//!    AETHER actually computes today via the centroid path in
//!    `tracker_bridge.rs`).
//! 2. **`float_cosine`** — cosine distance over `&[f32]` (alternative
//!    baseline; what some pipeline sites prefer).
//! 3. **`sketch_hamming`** — hamming distance over the 1-bit sketch.
//!
//! Run with:
//! ```bash
//! cargo bench -p wifi-densepose-ruvector --bench sketch_bench
//! ```
//!
//! Pass criterion: `sketch_hamming` is at least **8×** faster than the
//! cheaper of `float_l2` / `float_cosine` at every measured dimension.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint;
use wifi_densepose_ruvector::Sketch;

const SKETCH_VERSION: u16 = 1;

/// Squared-euclidean over `&[f32]` — baseline AETHER path.
#[inline]
fn float_l2_squared(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let d = x - y;
            d * d
        })
        .sum()
}

/// Cosine distance (1.0 - cosine similarity) over `&[f32]`.
/// Alternative baseline — used by some pipeline sites that need
/// magnitude-invariant similarity.
#[inline]
fn float_cosine(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (&x, &y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = (na * nb).sqrt();
    if denom < f32::EPSILON {
        1.0
    } else {
        1.0 - dot / denom
    }
}

/// Generate a deterministic pseudo-random embedding of the given dimension.
/// Uses a simple LCG so benches are repeatable across runs and machines
/// without pulling in a `rand` dev-dep just for fixture generation.
fn make_embedding(dim: usize, seed: u32) -> Vec<f32> {
    let mut state = seed.wrapping_mul(2654435761).wrapping_add(1);
    (0..dim)
        .map(|_| {
            // Iterate LCG (Numerical Recipes constants — for fixture only,
            // not for cryptographic use).
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            // Map to [-1.0, 1.0] approximately.
            let u = (state >> 8) as f32 / (1u32 << 24) as f32;
            u * 2.0 - 1.0
        })
        .collect()
}

fn bench_compare_cost(c: &mut Criterion) {
    for &dim in &[128usize, 256, 512] {
        let a_vec = make_embedding(dim, 0xAAAA_AAAA);
        let b_vec = make_embedding(dim, 0xBBBB_BBBB);
        let a_sketch = Sketch::from_embedding(&a_vec, SKETCH_VERSION);
        let b_sketch = Sketch::from_embedding(&b_vec, SKETCH_VERSION);

        let mut group = c.benchmark_group(format!("compare_d{dim}"));
        group.throughput(Throughput::Elements(1));

        group.bench_with_input(BenchmarkId::new("float_l2", dim), &dim, |bencher, _| {
            bencher.iter(|| {
                let d = float_l2_squared(black_box(&a_vec), black_box(&b_vec));
                hint::black_box(d)
            });
        });

        group.bench_with_input(BenchmarkId::new("float_cosine", dim), &dim, |bencher, _| {
            bencher.iter(|| {
                let d = float_cosine(black_box(&a_vec), black_box(&b_vec));
                hint::black_box(d)
            });
        });

        group.bench_with_input(BenchmarkId::new("sketch_hamming", dim), &dim, |bencher, _| {
            bencher.iter(|| {
                let d = black_box(&a_sketch).distance_unchecked(black_box(&b_sketch));
                hint::black_box(d)
            });
        });

        group.finish();
    }
}

/// Top-K @ K=8 over a 1024-sketch bank — the realistic AETHER use case
/// (a few thousand re-ID candidates, K small).
fn bench_topk(c: &mut Criterion) {
    use wifi_densepose_ruvector::SketchBank;

    let dim = 128usize;
    let bank_size = 1024usize;
    let k = 8usize;

    let mut bank = SketchBank::new();
    for i in 0..bank_size {
        let v = make_embedding(dim, i as u32);
        bank.insert(i as u32, Sketch::from_embedding(&v, SKETCH_VERSION))
            .expect("schema-locked insert");
    }

    let query_vec = make_embedding(dim, 0xCAFE_BABE);
    let query_sketch = Sketch::from_embedding(&query_vec, SKETCH_VERSION);

    // Build a parallel float bank for the baseline.
    let float_bank: Vec<Vec<f32>> = (0..bank_size).map(|i| make_embedding(dim, i as u32)).collect();

    let mut group = c.benchmark_group(format!("topk_d{dim}_n{bank_size}_k{k}"));
    group.throughput(Throughput::Elements(bank_size as u64));

    group.bench_function("float_l2_topk", |bencher| {
        bencher.iter(|| {
            let mut scored: Vec<(u32, f32)> = float_bank
                .iter()
                .enumerate()
                .map(|(i, v)| (i as u32, float_l2_squared(black_box(&query_vec), v)))
                .collect();
            scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(k);
            hint::black_box(scored)
        });
    });

    group.bench_function("sketch_hamming_topk", |bencher| {
        bencher.iter(|| {
            let result = black_box(&bank).topk(black_box(&query_sketch), k).expect("schema match");
            hint::black_box(result)
        });
    });

    group.finish();
}

criterion_group!(benches, bench_compare_cost, bench_topk);
criterion_main!(benches);
