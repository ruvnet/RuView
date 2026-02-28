use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

/// Benchmark individual preprocessing transforms
fn bench_individual_transforms(c: &mut Criterion) {
    let mut group = c.benchmark_group("individual_transforms");
    group.measurement_time(Duration::from_secs(8));

    let sizes = [(224, 224), (384, 384), (512, 512)];

    for (w, h) in sizes {
        let image_data = generate_test_image(w, h);

        // Grayscale conversion
        group.bench_with_input(
            BenchmarkId::new("grayscale", format!("{}x{}", w, h)),
            &image_data,
            |b, img| {
                b.iter(|| black_box(convert_to_grayscale(black_box(img), w, h)));
            },
        );

        // Gaussian blur
        group.bench_with_input(
            BenchmarkId::new("gaussian_blur", format!("{}x{}", w, h)),
            &image_data,
            |b, img| {
                b.iter(|| black_box(apply_gaussian_blur(black_box(img), w, h, 5)));
            },
        );

        // Adaptive threshold
        group.bench_with_input(
            BenchmarkId::new("threshold", format!("{}x{}", w, h)),
            &image_data,
            |b, img| {
                b.iter(|| black_box(apply_adaptive_threshold(black_box(img), w, h)));
            },
        );

        // Edge detection
        group.bench_with_input(
            BenchmarkId::new("edge_detection", format!("{}x{}", w, h)),
            &image_data,
            |b, img| {
                b.iter(|| black_box(detect_edges(black_box(img), w, h)));
            },
        );

        // Normalization
        group.bench_with_input(
            BenchmarkId::new("normalize", format!("{}x{}", w, h)),
            &image_data,
            |b, img| {
                b.iter(|| black_box(normalize_image(black_box(img))));
            },
        );
    }

    group.finish();
}

/// Benchmark full preprocessing pipeline
fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pipeline");
    group.measurement_time(Duration::from_secs(10));

    let sizes = [(224, 224), (384, 384), (512, 512)];

    for (w, h) in sizes {
        let image_data = generate_test_image(w, h);

        group.bench_with_input(
            BenchmarkId::new("sequential", format!("{}x{}", w, h)),
            &(image_data.clone(), w, h),
            |b, (img, width, height)| {
                b.iter(|| {
                    let gray = convert_to_grayscale(black_box(img), *width, *height);
                    let blurred = apply_gaussian_blur(&gray, *width, *height, 5);
                    let threshold = apply_adaptive_threshold(&blurred, *width, *height);
                    let edges = detect_edges(&threshold, *width, *height);
                    let normalized = normalize_image(&edges);
                    black_box(normalized)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark parallel vs sequential preprocessing
fn bench_parallel_vs_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_vs_sequential");
    group.measurement_time(Duration::from_secs(10));

    // Create batch of images
    let batch_size = 8;
    let size = (384, 384);
    let images: Vec<Vec<u8>> = (0..batch_size)
        .map(|_| generate_test_image(size.0, size.1))
        .collect();

    // Sequential processing
    group.bench_function("sequential_batch", |b| {
        b.iter(|| {
            let results: Vec<_> = images
                .iter()
                .map(|img| {
                    let gray = convert_to_grayscale(black_box(img), size.0, size.1);
                    let blurred = apply_gaussian_blur(&gray, size.0, size.1, 5);
                    apply_adaptive_threshold(&blurred, size.0, size.1)
                })
                .collect();
            black_box(results)
        });
    });

    // Parallel processing (simulated with rayon-like chunking)
    group.bench_function("parallel_batch", |b| {
        b.iter(|| {
            // In production, this would use rayon::par_iter()
            let results: Vec<_> = images
                .chunks(2)
                .flat_map(|chunk| {
                    chunk.iter().map(|img| {
                        let gray = convert_to_grayscale(black_box(img), size.0, size.1);
                        let blurred = apply_gaussian_blur(&gray, size.0, size.1, 5);
                        apply_adaptive_threshold(&blurred, size.0, size.1)
                    })
                })
                .collect();
            black_box(results)
        });
    });

    group.finish();
}

/// Benchmark resize operations
fn bench_resize_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("resize_operations");
    group.measurement_time(Duration::from_secs(8));

    let source_image = generate_test_image(1024, 1024);
    let target_sizes = [(224, 224), (384, 384), (512, 512)];

    for (target_w, target_h) in target_sizes {
        group.bench_with_input(
            BenchmarkId::new("nearest_neighbor", format!("{}x{}", target_w, target_h)),
            &(target_w, target_h),
            |b, &(tw, th)| {
                b.iter(|| black_box(resize_nearest(&source_image, 1024, 1024, tw, th)));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("bilinear", format!("{}x{}", target_w, target_h)),
            &(target_w, target_h),
            |b, &(tw, th)| {
                b.iter(|| black_box(resize_bilinear(&source_image, 1024, 1024, tw, th)));
            },
        );
    }

    group.finish();
}

/// Benchmark target: preprocessing should complete in <20ms
fn bench_latency_target(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_target_20ms");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    let image_data = generate_test_image(384, 384);

    group.bench_function("full_pipeline_384x384", |b| {
        b.iter(|| {
            let gray = convert_to_grayscale(black_box(&image_data), 384, 384);
            let blurred = apply_gaussian_blur(&gray, 384, 384, 5);
            let threshold = apply_adaptive_threshold(&blurred, 384, 384);
            let normalized = normalize_image(&threshold);
            black_box(normalized)
        });
    });

    group.finish();
}

// Mock implementations

fn generate_test_image(width: u32, height: u32) -> Vec<u8> {
    let size = (width * height * 3) as usize;
    (0..size).map(|i| ((i * 123 + 456) % 256) as u8).collect()
}

fn convert_to_grayscale(rgb_data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut gray = Vec::with_capacity((width * height) as usize);
    for chunk in rgb_data.chunks(3) {
        let r = chunk[0] as u32;
        let g = chunk[1] as u32;
        let b = chunk[2] as u32;
        let gray_value = ((r * 299 + g * 587 + b * 114) / 1000) as u8;
        gray.push(gray_value);
    }
    gray
}

fn apply_gaussian_blur(data: &[u8], width: u32, height: u32, kernel_size: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let radius = kernel_size / 2;

    for y in 0..height {
        for x in 0..width {
            let mut sum = 0u32;
            let mut count = 0u32;

            for ky in 0..kernel_size {
                for kx in 0..kernel_size {
                    let nx = x as i32 + kx as i32 - radius as i32;
                    let ny = y as i32 + ky as i32 - radius as i32;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let idx = (ny as u32 * width + nx as u32) as usize;
                        sum += data[idx] as u32;
                        count += 1;
                    }
                }
            }

            result.push((sum / count) as u8);
        }
    }

    result
}

fn apply_adaptive_threshold(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let block_size = 11;
    let c = 2;

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let pixel = data[idx];

            // Calculate local mean
            let mut sum = 0u32;
            let mut count = 0u32;
            let radius = block_size / 2;

            for by in y.saturating_sub(radius)..=(y + radius).min(height - 1) {
                for bx in x.saturating_sub(radius)..=(x + radius).min(width - 1) {
                    let bidx = (by * width + bx) as usize;
                    sum += data[bidx] as u32;
                    count += 1;
                }
            }

            let threshold = (sum / count) as i32 - c;
            result.push(if pixel as i32 > threshold { 255 } else { 0 });
        }
    }

    result
}

fn detect_edges(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());

    // Simple Sobel edge detection
    for y in 0..height {
        for x in 0..width {
            if x == 0 || x == width - 1 || y == 0 || y == height - 1 {
                result.push(0);
                continue;
            }

            let idx = (y * width + x) as usize;
            let gx = (data[idx + 1] as i32 - data[idx - 1] as i32).abs();
            let gy = (data[idx + width as usize] as i32 - data[idx - width as usize] as i32).abs();
            let magnitude = ((gx * gx + gy * gy) as f32).sqrt().min(255.0);

            result.push(magnitude as u8);
        }
    }

    result
}

fn normalize_image(data: &[u8]) -> Vec<f32> {
    data.iter().map(|&x| (x as f32 - 128.0) / 128.0).collect()
}

fn resize_nearest(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut result = Vec::with_capacity((dst_w * dst_h) as usize);
    let x_ratio = src_w as f32 / dst_w as f32;
    let y_ratio = src_h as f32 / dst_h as f32;

    for y in 0..dst_h {
        for x in 0..dst_w {
            let src_x = (x as f32 * x_ratio) as u32;
            let src_y = (y as f32 * y_ratio) as u32;
            let idx = (src_y * src_w + src_x) as usize;
            result.push(src[idx]);
        }
    }

    result
}

fn resize_bilinear(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut result = Vec::with_capacity((dst_w * dst_h) as usize);
    let x_ratio = (src_w - 1) as f32 / dst_w as f32;
    let y_ratio = (src_h - 1) as f32 / dst_h as f32;

    for y in 0..dst_h {
        for x in 0..dst_w {
            let src_x = x as f32 * x_ratio;
            let src_y = y as f32 * y_ratio;

            let x1 = src_x.floor() as u32;
            let y1 = src_y.floor() as u32;
            let x2 = (x1 + 1).min(src_w - 1);
            let y2 = (y1 + 1).min(src_h - 1);

            let q11 = src[(y1 * src_w + x1) as usize] as f32;
            let q21 = src[(y1 * src_w + x2) as usize] as f32;
            let q12 = src[(y2 * src_w + x1) as usize] as f32;
            let q22 = src[(y2 * src_w + x2) as usize] as f32;

            let wx = src_x - x1 as f32;
            let wy = src_y - y1 as f32;

            let value = q11 * (1.0 - wx) * (1.0 - wy)
                + q21 * wx * (1.0 - wy)
                + q12 * (1.0 - wx) * wy
                + q22 * wx * wy;

            result.push(value as u8);
        }
    }

    result
}

criterion_group!(
    benches,
    bench_individual_transforms,
    bench_full_pipeline,
    bench_parallel_vs_sequential,
    bench_resize_operations,
    bench_latency_target
);
criterion_main!(benches);
