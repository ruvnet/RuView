//! Criterion bench for `Pipeline::run` throughput.
//!
//! Plan §5 acceptance: ≥ 1 kHz simulated samples per second of wall-clock
//! on a Cortex-A53-class CPU. This bench measures wall-clock on whatever
//! the developer is running on; the user evaluates it against the
//! Cortex-A53 budget by applying their own scaling factor (typically
//! ~4-6× slower than x86_64 dev hardware).
//!
//! Run with:
//! ```bash
//! cargo bench -p nvsim --bench pipeline_throughput
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint;

use nvsim::pipeline::{Pipeline, PipelineConfig};
use nvsim::scene::{DipoleSource, Scene};

fn fixture_scene(n_dipoles: usize) -> Scene {
    let mut s = Scene::new();
    for i in 0..n_dipoles {
        let z = 0.3 + (i as f64) * 0.05;
        s.add_dipole(DipoleSource::new([0.0, 0.0, z], [0.0, 0.0, 1.0e-3]));
    }
    s.add_sensor([0.0, 0.0, 0.0]);
    s
}

fn bench_pipeline_throughput(c: &mut Criterion) {
    let scene_sizes = [1, 4, 16];
    let sample_counts = [256, 1024];

    let mut group = c.benchmark_group("pipeline_run");
    for &n_dipoles in &scene_sizes {
        for &n_samples in &sample_counts {
            let scene = fixture_scene(n_dipoles);
            let cfg = PipelineConfig::default();
            let pipeline = Pipeline::new(scene, cfg, 42);

            group.throughput(Throughput::Elements(n_samples as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("d{}", n_dipoles), n_samples),
                &n_samples,
                |bencher, &n| {
                    bencher.iter(|| {
                        let frames = black_box(&pipeline).run(black_box(n));
                        hint::black_box(frames)
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_witness_overhead(c: &mut Criterion) {
    let scene = fixture_scene(4);
    let cfg = PipelineConfig::default();
    let pipeline = Pipeline::new(scene, cfg, 42);
    let n = 1024;

    let mut group = c.benchmark_group("witness");
    group.throughput(Throughput::Elements(n as u64));

    group.bench_function("run", |bencher| {
        bencher.iter(|| {
            let r = black_box(&pipeline).run(n);
            hint::black_box(r)
        });
    });

    group.bench_function("run_with_witness", |bencher| {
        bencher.iter(|| {
            let r = black_box(&pipeline).run_with_witness(n);
            hint::black_box(r)
        });
    });

    group.finish();
}

criterion_group!(benches, bench_pipeline_throughput, bench_witness_overhead);
criterion_main!(benches);
