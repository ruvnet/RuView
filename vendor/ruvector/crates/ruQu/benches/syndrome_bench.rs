//! Benchmarks for syndrome processing performance.
//!
//! Run with: `cargo bench -p ruqu`

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use ruqu::syndrome::{DetectorBitmap, SyndromeBuffer, SyndromeDelta, SyndromeRound};

/// Benchmark DetectorBitmap operations
fn bench_bitmap_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("DetectorBitmap");

    // Benchmark set operation
    group.throughput(Throughput::Elements(1024));
    group.bench_function("set_all_1024", |b| {
        let mut bitmap = DetectorBitmap::new(1024);
        b.iter(|| {
            for i in 0..1024 {
                bitmap.set(i, true);
            }
            black_box(&bitmap);
        });
    });

    // Benchmark get operation
    group.bench_function("get_all_1024", |b| {
        let mut bitmap = DetectorBitmap::new(1024);
        for i in (0..1024).step_by(3) {
            bitmap.set(i, true);
        }
        b.iter(|| {
            let mut count = 0usize;
            for i in 0..1024 {
                if bitmap.get(i) {
                    count += 1;
                }
            }
            black_box(count);
        });
    });

    // Benchmark popcount
    group.bench_function("popcount_sparse", |b| {
        let mut bitmap = DetectorBitmap::new(1024);
        for i in (0..1024).step_by(100) {
            bitmap.set(i, true);
        }
        b.iter(|| black_box(bitmap.popcount()));
    });

    group.bench_function("popcount_dense", |b| {
        let mut bitmap = DetectorBitmap::new(1024);
        for i in 0..512 {
            bitmap.set(i, true);
        }
        b.iter(|| black_box(bitmap.popcount()));
    });

    // Benchmark XOR
    group.bench_function("xor_1024", |b| {
        let mut a = DetectorBitmap::new(1024);
        let mut bb = DetectorBitmap::new(1024);
        for i in (0..512).step_by(2) {
            a.set(i, true);
        }
        for i in (256..768).step_by(2) {
            bb.set(i, true);
        }
        b.iter(|| black_box(a.xor(&bb)));
    });

    // Benchmark iter_fired
    group.bench_function("iter_fired_sparse", |b| {
        let mut bitmap = DetectorBitmap::new(1024);
        for i in (0..1024).step_by(100) {
            bitmap.set(i, true);
        }
        b.iter(|| {
            let count: usize = bitmap.iter_fired().count();
            black_box(count);
        });
    });

    group.bench_function("iter_fired_dense", |b| {
        let mut bitmap = DetectorBitmap::new(1024);
        for i in 0..100 {
            bitmap.set(i, true);
        }
        b.iter(|| {
            let count: usize = bitmap.iter_fired().count();
            black_box(count);
        });
    });

    group.finish();
}

/// Benchmark SyndromeBuffer operations
fn bench_buffer_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("SyndromeBuffer");

    // Benchmark push (main hot path)
    group.throughput(Throughput::Elements(1));
    group.bench_function("push", |b| {
        let mut buffer = SyndromeBuffer::new(1024);
        let mut round_id = 0u64;
        b.iter(|| {
            let mut detectors = DetectorBitmap::new(64);
            detectors.set((round_id % 64) as usize, true);
            let round = SyndromeRound::new(round_id, round_id, round_id * 1000, detectors, 0);
            buffer.push(round);
            round_id = round_id.wrapping_add(1);
            black_box(&buffer);
        });
    });

    // Benchmark window extraction
    group.bench_function("window_10", |b| {
        let mut buffer = SyndromeBuffer::new(1024);
        for i in 0..1000 {
            let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }
        b.iter(|| black_box(buffer.window(10)));
    });

    group.bench_function("window_100", |b| {
        let mut buffer = SyndromeBuffer::new(1024);
        for i in 0..1000 {
            let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }
        b.iter(|| black_box(buffer.window(100)));
    });

    // Benchmark get by round_id
    group.bench_function("get_recent", |b| {
        let mut buffer = SyndromeBuffer::new(1024);
        for i in 0..1000 {
            let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }
        b.iter(|| black_box(buffer.get(995)));
    });

    group.bench_function("get_old", |b| {
        let mut buffer = SyndromeBuffer::new(1024);
        for i in 0..1000 {
            let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }
        b.iter(|| black_box(buffer.get(100)));
    });

    group.finish();
}

/// Benchmark SyndromeDelta computation
fn bench_delta_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("SyndromeDelta");

    // Create test rounds
    let mut d1 = DetectorBitmap::new(1024);
    let mut d2 = DetectorBitmap::new(1024);
    for i in (0..512).step_by(2) {
        d1.set(i, true);
    }
    for i in (256..768).step_by(2) {
        d2.set(i, true);
    }
    let round1 = SyndromeRound::new(1, 100, 1000, d1, 0);
    let round2 = SyndromeRound::new(2, 101, 2000, d2, 0);

    // Benchmark delta computation
    group.bench_function("compute", |b| {
        b.iter(|| black_box(SyndromeDelta::compute(&round1, &round2)));
    });

    // Benchmark activity level
    let delta = SyndromeDelta::compute(&round1, &round2);
    group.bench_function("activity_level", |b| {
        b.iter(|| black_box(delta.activity_level()));
    });

    // Benchmark is_quiet
    group.bench_function("is_quiet", |b| {
        b.iter(|| black_box(delta.is_quiet()));
    });

    group.finish();
}

/// Benchmark full pipeline throughput
fn bench_pipeline_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("Pipeline");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("ingest_1000_rounds", |b| {
        b.iter(|| {
            let mut buffer = SyndromeBuffer::new(1024);
            for i in 0..1000u64 {
                let mut detectors = DetectorBitmap::new(64);
                // Simulate sparse detector firings
                if i % 10 == 0 {
                    detectors.set((i % 64) as usize, true);
                }
                let round = SyndromeRound::new(i, i, i * 1000, detectors, 0);
                buffer.push(round);
            }
            black_box(&buffer);
        });
    });

    group.bench_function("ingest_and_delta_1000", |b| {
        b.iter(|| {
            let mut buffer = SyndromeBuffer::new(1024);
            let mut prev_round: Option<SyndromeRound> = None;
            let mut delta_count = 0usize;

            for i in 0..1000u64 {
                let mut detectors = DetectorBitmap::new(64);
                if i % 10 == 0 {
                    detectors.set((i % 64) as usize, true);
                }
                let round = SyndromeRound::new(i, i, i * 1000, detectors, 0);

                if let Some(prev) = &prev_round {
                    let delta = SyndromeDelta::compute(prev, &round);
                    if !delta.is_quiet() {
                        delta_count += 1;
                    }
                }

                prev_round = Some(round.clone());
                buffer.push(round);
            }
            black_box(delta_count);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_bitmap_operations,
    bench_buffer_operations,
    bench_delta_operations,
    bench_pipeline_throughput,
);

criterion_main!(benches);
