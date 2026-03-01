use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_dither::{
    channel::ChannelDither, quantize_dithered, quantize_slice_dithered, GoldenRatioDither, PiDither,
};

fn bench_single_quantize(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantize_dithered_single");
    for bits in [5u32, 7, 8] {
        group.bench_with_input(BenchmarkId::from_parameter(bits), &bits, |b, &bits| {
            let mut d = GoldenRatioDither::new(0.0);
            b.iter(|| quantize_dithered(black_box(0.314_f32), bits, 0.5, &mut d));
        });
    }
    group.finish();
}

fn bench_slice_quantize(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantize_slice");
    for n in [64usize, 256, 1024] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let input: Vec<f32> = (0..n).map(|i| (i as f32 / n as f32) * 2.0 - 1.0).collect();
            b.iter(|| {
                let mut buf = input.clone();
                let mut d = GoldenRatioDither::new(0.0);
                quantize_slice_dithered(black_box(&mut buf), 8, 0.5, &mut d);
                black_box(buf)
            });
        });
    }
    group.finish();
}

fn bench_pi_dither(c: &mut Criterion) {
    c.bench_function("pi_dither_1k", |b| {
        let mut d = PiDither::new(0);
        let mut buf: Vec<f32> = vec![0.5; 1024];
        b.iter(|| {
            quantize_slice_dithered(black_box(&mut buf), 7, 0.5, &mut d);
        });
    });
}

fn bench_channel_dither(c: &mut Criterion) {
    c.bench_function("channel_dither_256activations_32ch", |b| {
        let mut cd = ChannelDither::new(0, 32, 8, 0.5);
        let mut acts: Vec<f32> = vec![0.314; 256];
        b.iter(|| {
            cd.quantize_batch(black_box(&mut acts));
        });
    });
}

criterion_group!(
    benches,
    bench_single_quantize,
    bench_slice_quantize,
    bench_pi_dither,
    bench_channel_dither
);
criterion_main!(benches);
