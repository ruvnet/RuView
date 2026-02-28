use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use scipix_ocr::optimize::*;

fn bench_grayscale(c: &mut Criterion) {
    let mut group = c.benchmark_group("grayscale");

    for size in [256, 512, 1024, 2048].iter() {
        let pixels = size * size;
        let rgba: Vec<u8> = (0..pixels * 4).map(|i| (i % 256) as u8).collect();
        let mut gray = vec![0u8; pixels];

        group.throughput(Throughput::Elements(pixels as u64));

        // Benchmark SIMD version
        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, _| {
            b.iter(|| {
                simd::simd_grayscale(black_box(&rgba), black_box(&mut gray));
            });
        });

        // Benchmark scalar version
        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            b.iter(|| {
                for (i, chunk) in rgba.chunks_exact(4).enumerate() {
                    let r = chunk[0] as u32;
                    let g = chunk[1] as u32;
                    let b = chunk[2] as u32;
                    gray[i] = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
                }
            });
        });
    }

    group.finish();
}

fn bench_threshold(c: &mut Criterion) {
    let mut group = c.benchmark_group("threshold");

    for size in [1024, 4096, 16384, 65536].iter() {
        let gray: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();
        let mut out = vec![0u8; *size];

        group.throughput(Throughput::Elements(*size as u64));

        // SIMD version
        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, _| {
            b.iter(|| {
                simd::simd_threshold(black_box(&gray), black_box(128), black_box(&mut out));
            });
        });

        // Scalar version
        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            b.iter(|| {
                for (g, o) in gray.iter().zip(out.iter_mut()) {
                    *o = if *g >= 128 { 255 } else { 0 };
                }
            });
        });
    }

    group.finish();
}

fn bench_normalize(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalize");

    for size in [128, 512, 2048, 8192].iter() {
        let mut data: Vec<f32> = (0..*size).map(|i| i as f32).collect();

        group.throughput(Throughput::Elements(*size as u64));

        // SIMD version
        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, _| {
            let mut data_copy = data.clone();
            b.iter(|| {
                simd::simd_normalize(black_box(&mut data_copy));
            });
        });

        // Scalar version
        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let mut data_copy = data.clone();
            b.iter(|| {
                let sum: f32 = data_copy.iter().sum();
                let mean = sum / data_copy.len() as f32;
                let variance: f32 = data_copy.iter().map(|x| (x - mean).powi(2)).sum::<f32>()
                    / data_copy.len() as f32;
                let std_dev = variance.sqrt() + 1e-8;
                for x in data_copy.iter_mut() {
                    *x = (*x - mean) / std_dev;
                }
            });
        });
    }

    group.finish();
}

fn bench_parallel_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_map");

    for size in [100, 1000, 10000].iter() {
        let data: Vec<i32> = (0..*size).collect();

        group.throughput(Throughput::Elements(*size as u64));

        // Parallel version
        group.bench_with_input(BenchmarkId::new("parallel", size), size, |b, _| {
            b.iter(|| {
                parallel::parallel_map_chunked(black_box(data.clone()), 100, |x| x * x + x * 2 + 1)
            });
        });

        // Sequential version
        group.bench_with_input(BenchmarkId::new("sequential", size), size, |b, _| {
            b.iter(|| data.iter().map(|&x| x * x + x * 2 + 1).collect::<Vec<_>>());
        });
    }

    group.finish();
}

fn bench_buffer_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool");

    let pool = memory::BufferPool::new(|| Vec::with_capacity(1024), 10, 100);

    // Benchmark pooled allocation
    group.bench_function("pooled", |b| {
        b.iter(|| {
            let mut buf = pool.acquire();
            buf.extend_from_slice(&[0u8; 512]);
            black_box(&buf);
        });
    });

    // Benchmark direct allocation
    group.bench_function("direct", |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(1024);
            buf.extend_from_slice(&[0u8; 512]);
            black_box(&buf);
        });
    });

    group.finish();
}

fn bench_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantization");

    for size in [1024, 4096, 16384].iter() {
        let weights: Vec<f32> = (0..*size)
            .map(|i| (i as f32 / *size as f32) * 2.0 - 1.0)
            .collect();

        group.throughput(Throughput::Elements(*size as u64));

        // Quantize
        group.bench_with_input(BenchmarkId::new("quantize", size), size, |b, _| {
            b.iter(|| quantize::quantize_weights(black_box(&weights)));
        });

        // Dequantize
        let (quantized, params) = quantize::quantize_weights(&weights);
        group.bench_with_input(BenchmarkId::new("dequantize", size), size, |b, _| {
            b.iter(|| quantize::dequantize(black_box(&quantized), black_box(params)));
        });

        // Per-channel quantization
        let shape = vec![*size / 64, 64];
        group.bench_with_input(BenchmarkId::new("per_channel", size), size, |b, _| {
            b.iter(|| {
                quantize::PerChannelQuant::from_f32(black_box(&weights), black_box(shape.clone()))
            });
        });
    }

    group.finish();
}

fn bench_memory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_ops");

    // Arena allocation
    let mut arena = memory::Arena::with_capacity(1024 * 1024);
    group.bench_function("arena_alloc", |b| {
        b.iter(|| {
            arena.reset();
            for _ in 0..100 {
                let slice = arena.alloc(1024, 8);
                black_box(slice);
            }
        });
    });

    // Vector allocation
    group.bench_function("vec_alloc", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let mut vec = Vec::with_capacity(1024);
                vec.resize(1024, 0u8);
                black_box(&vec);
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_grayscale,
    bench_threshold,
    bench_normalize,
    bench_parallel_map,
    bench_buffer_pool,
    bench_quantization,
    bench_memory_operations
);

criterion_main!(benches);
