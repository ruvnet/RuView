//! ADR-084 Pass 2 acceptance bench — EmbeddingHistory::search_prefilter
//! vs the brute-force EmbeddingHistory::search baseline.
//!
//! Measures the second ADR-084 acceptance number — **end-to-end query
//! cost reduction** at the AETHER re-ID site, with the empirically
//! validated `prefilter_factor=8` from
//! `test_search_prefilter_topk_coverage_meets_adr_084`.
//!
//! Run with:
//! ```bash
//! cargo bench -p wifi-densepose-signal --bench aether_prefilter_bench
//! ```
//!
//! Pass criterion: prefilter ≥ 4× faster than brute-force at n=1024;
//! ideally trends toward 8× as n grows. The 90%-coverage criterion is
//! exercised in the unit-test suite, not the bench (the bench measures
//! cost only).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint;
use wifi_densepose_signal::ruvsense::longitudinal::{EmbeddingEntry, EmbeddingHistory};

const SKETCH_VERSION: u16 = 1;
const PREFILTER_FACTOR: usize = 8;

/// Deterministic LCG so bench fixtures are reproducible across runs.
fn lcg_embedding(dim: usize, seed: u32) -> Vec<f32> {
    let mut s = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    (0..dim)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let u = (s >> 8) as f32 / (1u32 << 24) as f32;
            u * 2.0 - 1.0
        })
        .collect()
}

fn bench_search_vs_prefilter(c: &mut Criterion) {
    const DIM: usize = 128; // AETHER embedding dimension (ADR-024)
    const K: usize = 8;

    for &n in &[256usize, 1024, 4096] {
        // Build two parallel histories — one with sketches (prefilter
        // path) and one without (brute-force path). They contain the
        // same embeddings.
        let mut bf = EmbeddingHistory::new(DIM, n);
        let mut pf = EmbeddingHistory::with_sketch(DIM, n, SKETCH_VERSION);
        for i in 0..n {
            let v = lcg_embedding(DIM, i as u32 + 1);
            let entry = EmbeddingEntry {
                person_id: i as u64,
                day_us: i as u64,
                embedding: v,
            };
            bf.push(entry.clone()).expect("bf push");
            pf.push(entry).expect("pf push");
        }

        let query = lcg_embedding(DIM, 0xCAFE_BABE);

        let mut group = c.benchmark_group(format!("aether_search_d{DIM}_n{n}_k{K}"));
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(
            BenchmarkId::new("brute_force_cosine", n),
            &n,
            |bencher, _| {
                bencher.iter(|| {
                    let r = black_box(&bf).search(black_box(&query), K);
                    hint::black_box(r)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("sketch_prefilter_factor8", n),
            &n,
            |bencher, _| {
                bencher.iter(|| {
                    let r = black_box(&pf).search_prefilter(
                        black_box(&query),
                        K,
                        PREFILTER_FACTOR,
                    );
                    hint::black_box(r)
                });
            },
        );

        group.finish();
    }
}

criterion_group!(benches, bench_search_vs_prefilter);
criterion_main!(benches);
