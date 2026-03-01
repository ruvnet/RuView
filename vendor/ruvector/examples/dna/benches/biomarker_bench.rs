//! Criterion benchmarks for Biomarker Analysis Engine
//!
//! Performance benchmarks covering ADR-014 targets:
//! - Risk scoring (<50 μs)
//! - Profile vector encoding (<100 μs)
//! - Population generation (<500ms for 10k)
//! - Streaming throughput (>100k readings/sec)
//! - Z-score and classification (<5 μs)

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rvdna::biomarker::*;
use rvdna::biomarker_stream::*;
use std::collections::HashMap;

// ============================================================================
// Helpers
// ============================================================================

fn sample_genotypes() -> HashMap<String, String> {
    let mut gts = HashMap::new();
    gts.insert("rs429358".into(), "TT".into());
    gts.insert("rs7412".into(), "CC".into());
    gts.insert("rs4680".into(), "AG".into());
    gts.insert("rs1799971".into(), "AA".into());
    gts.insert("rs762551".into(), "AA".into());
    gts.insert("rs1801133".into(), "AG".into());
    gts.insert("rs1801131".into(), "TT".into());
    gts.insert("rs1042522".into(), "CG".into());
    gts.insert("rs80357906".into(), "DD".into());
    gts.insert("rs4363657".into(), "TT".into());
    gts
}

fn full_panel_genotypes() -> HashMap<String, String> {
    // All 17 SNPs from health.rs
    let mut gts = sample_genotypes();
    gts.insert("rs28897696".into(), "GG".into());
    gts.insert("rs11571833".into(), "AA".into());
    gts.insert("rs4988235".into(), "AG".into());
    gts.insert("rs53576".into(), "GG".into());
    gts.insert("rs6311".into(), "CT".into());
    gts.insert("rs1800497".into(), "AG".into());
    gts.insert("rs1800566".into(), "CC".into());
    gts
}

// ============================================================================
// Risk Scoring Benchmarks (target: <50 μs)
// ============================================================================

fn risk_scoring_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("biomarker_scoring");

    // Setup: create a representative genotype map
    let gts = sample_genotypes();

    group.bench_function("compute_risk_scores", |b| {
        b.iter(|| black_box(compute_risk_scores(&gts)));
    });

    group.bench_function("compute_risk_scores_full_panel", |b| {
        let full_gts = full_panel_genotypes();
        b.iter(|| black_box(compute_risk_scores(&full_gts)));
    });

    group.finish();
}

// ============================================================================
// Profile Vector Benchmarks (target: <100 μs)
// ============================================================================

fn vector_encoding_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("biomarker_vector");

    let gts = sample_genotypes();
    let profile = compute_risk_scores(&gts);

    group.bench_function("encode_profile_vector", |b| {
        b.iter(|| black_box(encode_profile_vector(&profile)));
    });

    group.finish();
}

// ============================================================================
// Population Generation Benchmarks (target: <500ms for 10k)
// ============================================================================

fn population_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("biomarker_population");

    group.bench_function("generate_100", |b| {
        b.iter(|| black_box(generate_synthetic_population(100, 42)));
    });

    group.bench_function("generate_1000", |b| {
        b.iter(|| black_box(generate_synthetic_population(1000, 42)));
    });

    group.finish();
}

// ============================================================================
// Streaming Benchmarks (target: >100k readings/sec)
// ============================================================================

fn streaming_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("biomarker_streaming");

    group.bench_function("generate_1000_readings", |b| {
        let config = StreamConfig::default();
        b.iter(|| black_box(generate_readings(&config, 1000, 42)));
    });

    group.bench_function("process_1000_readings", |b| {
        let config = StreamConfig::default();
        let readings = generate_readings(&config, 1000, 42);
        b.iter(|| {
            let mut processor = StreamProcessor::new(config.clone());
            for reading in &readings {
                black_box(processor.process_reading(reading));
            }
        });
    });

    group.bench_function("ring_buffer_1000_push", |b| {
        b.iter(|| {
            let mut rb: RingBuffer<f64> = RingBuffer::new(100);
            for i in 0..1000 {
                rb.push(black_box(i as f64));
            }
        });
    });

    group.finish();
}

// ============================================================================
// Z-Score and Classification Benchmarks (target: <5 μs)
// ============================================================================

fn classification_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("biomarker_classification");
    let refs = biomarker_references();

    group.bench_function("z_score_single", |b| {
        let r = &refs[0];
        b.iter(|| black_box(z_score(180.0, r)));
    });

    group.bench_function("classify_single", |b| {
        let r = &refs[0];
        b.iter(|| black_box(classify_biomarker(180.0, r)));
    });

    group.bench_function("z_score_all_biomarkers", |b| {
        b.iter(|| {
            for r in refs {
                let mid = (r.normal_low + r.normal_high) / 2.0;
                black_box(z_score(mid, r));
            }
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    risk_scoring_benchmarks,
    vector_encoding_benchmarks,
    population_benchmarks,
    streaming_benchmarks,
    classification_benchmarks,
);
criterion_main!(benches);
