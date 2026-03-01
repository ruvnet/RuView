use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

/// Benchmark single image OCR at various sizes
fn bench_single_image(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_image_ocr");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    // Test various image sizes
    let sizes = [
        (224, 224),   // Small
        (384, 384),   // Medium
        (512, 512),   // Large
        (768, 768),   // Extra large
        (1024, 1024), // Very large
    ];

    for (w, h) in sizes {
        group.bench_with_input(
            BenchmarkId::new("resolution", format!("{}x{}", w, h)),
            &(w, h),
            |b, &(width, height)| {
                // Create synthetic image data
                let image_data = vec![128u8; (width * height * 3) as usize];

                b.iter(|| {
                    // Simulate OCR processing pipeline
                    // In production, this would call actual OCR functions
                    let preprocessed = preprocess_image(black_box(&image_data), width, height);
                    let features = extract_features(black_box(&preprocessed));
                    let text = recognize_text(black_box(&features));
                    black_box(text)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark batch processing with various batch sizes
fn bench_batch_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_processing");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(30);

    let batch_sizes = [1, 4, 8, 16, 32];
    let image_size = (384, 384);

    for batch_size in batch_sizes {
        group.bench_with_input(
            BenchmarkId::new("batch_size", batch_size),
            &batch_size,
            |b, &size| {
                // Create batch of synthetic images
                let images: Vec<Vec<u8>> = (0..size)
                    .map(|_| vec![128u8; (image_size.0 * image_size.1 * 3) as usize])
                    .collect();

                b.iter(|| {
                    // Process entire batch
                    let results: Vec<_> = images
                        .iter()
                        .map(|img| {
                            let preprocessed =
                                preprocess_image(black_box(img), image_size.0, image_size.1);
                            let features = extract_features(black_box(&preprocessed));
                            recognize_text(black_box(&features))
                        })
                        .collect();
                    black_box(results)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark cold start vs warm model performance
fn bench_cold_vs_warm(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_vs_warm");
    group.measurement_time(Duration::from_secs(10));

    let image_data = vec![128u8; (384 * 384 * 3) as usize];

    // Cold start benchmark - model initialization included
    group.bench_function("cold_start", |b| {
        b.iter_with_large_drop(|| {
            // Simulate model initialization + inference
            let _model = initialize_model();
            let preprocessed = preprocess_image(black_box(&image_data), 384, 384);
            let features = extract_features(black_box(&preprocessed));
            let text = recognize_text(black_box(&features));
            black_box(text)
        });
    });

    // Warm model benchmark - model already initialized
    group.bench_function("warm_inference", |b| {
        let _model = initialize_model(); // Initialize once outside benchmark

        b.iter(|| {
            let preprocessed = preprocess_image(black_box(&image_data), 384, 384);
            let features = extract_features(black_box(&preprocessed));
            let text = recognize_text(black_box(&features));
            black_box(text)
        });
    });

    group.finish();
}

/// Benchmark P95 and P99 latency targets
fn bench_latency_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_percentiles");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(100); // More samples for better percentile accuracy

    let image_data = vec![128u8; (384 * 384 * 3) as usize];

    group.bench_function("p95_target_100ms", |b| {
        b.iter(|| {
            let preprocessed = preprocess_image(black_box(&image_data), 384, 384);
            let features = extract_features(black_box(&preprocessed));
            let text = recognize_text(black_box(&features));
            black_box(text)
        });
    });

    group.finish();
}

/// Benchmark throughput (images per second)
fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    group.measurement_time(Duration::from_secs(15));
    group.throughput(criterion::Throughput::Elements(1));

    let image_data = vec![128u8; (384 * 384 * 3) as usize];

    group.bench_function("images_per_second", |b| {
        b.iter(|| {
            let preprocessed = preprocess_image(black_box(&image_data), 384, 384);
            let features = extract_features(black_box(&preprocessed));
            let text = recognize_text(black_box(&features));
            black_box(text)
        });
    });

    group.finish();
}

// Mock implementations for benchmarking
// In production, these would be actual OCR pipeline functions

fn initialize_model() -> Vec<u8> {
    // Simulate model loading
    std::thread::sleep(Duration::from_millis(50));
    vec![0u8; 1024]
}

fn preprocess_image(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    // Simulate preprocessing: resize, normalize, grayscale
    let mut processed = Vec::with_capacity((width * height) as usize);
    for chunk in data.chunks(3) {
        // Convert to grayscale
        let gray = (chunk[0] as u32 + chunk[1] as u32 + chunk[2] as u32) / 3;
        processed.push(gray as u8);
    }
    processed
}

fn extract_features(data: &[u8]) -> Vec<f32> {
    // Simulate feature extraction
    data.iter().map(|&x| x as f32 / 255.0).collect()
}

fn recognize_text(features: &[f32]) -> String {
    // Simulate text recognition
    let sum: f32 = features.iter().take(100).sum();
    format!("recognized_text_{:.2}", sum)
}

criterion_group!(
    benches,
    bench_single_image,
    bench_batch_processing,
    bench_cold_vs_warm,
    bench_latency_percentiles,
    bench_throughput
);
criterion_main!(benches);
