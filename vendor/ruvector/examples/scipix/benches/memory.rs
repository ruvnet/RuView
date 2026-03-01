use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

/// Benchmark peak memory during inference
fn bench_peak_memory_inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("peak_memory_inference");
    group.measurement_time(Duration::from_secs(10));

    let sizes = [(224, 224), (384, 384), (512, 512)];

    for (w, h) in sizes {
        group.bench_with_input(
            BenchmarkId::new("single_inference", format!("{}x{}", w, h)),
            &(w, h),
            |b, &(width, height)| {
                b.iter_with_large_drop(|| {
                    let memory_tracker = MemoryTracker::new();

                    // Simulate model loading
                    let model = load_model();

                    // Create input
                    let image = create_image(width, height);

                    // Preprocessing
                    let preprocessed = preprocess(image);

                    // Inference
                    let output = run_inference(&model, preprocessed);

                    // Postprocessing
                    let result = postprocess(output);

                    let peak_memory = memory_tracker.peak_usage();
                    black_box((result, peak_memory))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory per image in batch
fn bench_memory_per_batch_image(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_per_batch_image");
    group.measurement_time(Duration::from_secs(15));

    let batch_sizes = [1, 4, 8, 16, 32];
    let size = (384, 384);

    for batch_size in batch_sizes {
        group.bench_with_input(
            BenchmarkId::new("batch_inference", batch_size),
            &batch_size,
            |b, &size| {
                b.iter_with_large_drop(|| {
                    let memory_tracker = MemoryTracker::new();

                    let model = load_model();
                    let batch = create_batch(size, 384, 384);
                    let output = run_batch_inference(&model, batch);

                    let total_memory = memory_tracker.peak_usage();
                    let per_image = total_memory / size;

                    black_box((output, per_image))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark model loading memory
fn bench_model_loading_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("model_loading_memory");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("detection_model", |b| {
        b.iter_with_large_drop(|| {
            let tracker = MemoryTracker::new();
            let model = load_detection_model();
            let memory = tracker.peak_usage();
            black_box((model, memory))
        });
    });

    group.bench_function("recognition_model", |b| {
        b.iter_with_large_drop(|| {
            let tracker = MemoryTracker::new();
            let model = load_recognition_model();
            let memory = tracker.peak_usage();
            black_box((model, memory))
        });
    });

    group.bench_function("math_model", |b| {
        b.iter_with_large_drop(|| {
            let tracker = MemoryTracker::new();
            let model = load_math_model();
            let memory = tracker.peak_usage();
            black_box((model, memory))
        });
    });

    group.bench_function("all_models", |b| {
        b.iter_with_large_drop(|| {
            let tracker = MemoryTracker::new();
            let detection = load_detection_model();
            let recognition = load_recognition_model();
            let math = load_math_model();
            let total_memory = tracker.peak_usage();
            black_box((detection, recognition, math, total_memory))
        });
    });

    group.finish();
}

/// Benchmark memory growth over time
fn bench_memory_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_growth");
    group.measurement_time(Duration::from_secs(20));

    group.bench_function("sequential_inferences", |b| {
        b.iter_with_large_drop(|| {
            let tracker = MemoryTracker::new();
            let model = load_model();
            let mut memory_samples = Vec::new();

            for i in 0..100 {
                let image = create_image(384, 384);
                let preprocessed = preprocess(image);
                let _output = run_inference(&model, preprocessed);

                if i % 10 == 0 {
                    memory_samples.push(tracker.current_usage());
                }
            }

            let growth = calculate_memory_growth(&memory_samples);
            black_box((memory_samples, growth))
        });
    });

    group.finish();
}

/// Benchmark memory fragmentation
fn bench_memory_fragmentation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_fragmentation");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("allocate_deallocate_pattern", |b| {
        b.iter(|| {
            let mut allocations = Vec::new();

            // Allocate various sizes
            for i in 0..100 {
                let size = (i % 10 + 1) * 1024;
                allocations.push(vec![0u8; size]);
            }

            // Deallocate every other allocation
            allocations = allocations
                .into_iter()
                .enumerate()
                .filter_map(|(i, v)| if i % 2 == 0 { Some(v) } else { None })
                .collect();

            // Allocate more
            for i in 0..50 {
                let size = (i % 5 + 1) * 2048;
                allocations.push(vec![0u8; size]);
            }

            black_box(allocations)
        });
    });

    group.finish();
}

/// Benchmark cache memory overhead
fn bench_cache_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_memory");
    group.measurement_time(Duration::from_secs(10));

    let cache_sizes = [100, 1000, 10000];

    for cache_size in cache_sizes {
        group.bench_with_input(
            BenchmarkId::new("embedding_cache", cache_size),
            &cache_size,
            |b, &size| {
                b.iter_with_large_drop(|| {
                    let tracker = MemoryTracker::new();
                    let cache = create_embedding_cache(size);
                    let memory = tracker.peak_usage();
                    black_box((cache, memory))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory pool efficiency
fn bench_memory_pools(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pools");
    group.measurement_time(Duration::from_secs(8));

    group.bench_function("without_pool", |b| {
        b.iter(|| {
            let mut allocations = Vec::new();
            for _ in 0..100 {
                let buffer = vec![0u8; 1024 * 1024];
                allocations.push(buffer);
            }
            black_box(allocations)
        });
    });

    group.bench_function("with_pool", |b| {
        let mut pool = MemoryPool::new(1024 * 1024, 100);
        b.iter(|| {
            let mut handles = Vec::new();
            for _ in 0..100 {
                let handle = pool.allocate();
                handles.push(handle);
            }
            black_box(handles)
        });
    });

    group.finish();
}

/// Benchmark tensor memory layouts
fn bench_tensor_layouts(c: &mut Criterion) {
    let mut group = c.benchmark_group("tensor_layouts");
    group.measurement_time(Duration::from_secs(8));

    let size = (384, 384, 3);

    group.bench_function("hwc_layout", |b| {
        b.iter(|| {
            let tracker = MemoryTracker::new();
            let tensor = create_hwc_tensor(size.0, size.1, size.2);
            let memory = tracker.peak_usage();
            black_box((tensor, memory))
        });
    });

    group.bench_function("chw_layout", |b| {
        b.iter(|| {
            let tracker = MemoryTracker::new();
            let tensor = create_chw_tensor(size.0, size.1, size.2);
            let memory = tracker.peak_usage();
            black_box((tensor, memory))
        });
    });

    group.bench_function("layout_conversion", |b| {
        let hwc = create_hwc_tensor(size.0, size.1, size.2);
        b.iter(|| {
            let tracker = MemoryTracker::new();
            let chw = convert_hwc_to_chw(&hwc, size.0, size.1, size.2);
            let memory = tracker.peak_usage();
            black_box((chw, memory))
        });
    });

    group.finish();
}

// Mock implementations

struct MemoryTracker {
    initial_usage: usize,
    peak: usize,
}

impl MemoryTracker {
    fn new() -> Self {
        Self {
            initial_usage: get_current_memory_usage(),
            peak: 0,
        }
    }

    fn current_usage(&self) -> usize {
        get_current_memory_usage() - self.initial_usage
    }

    fn peak_usage(&mut self) -> usize {
        let current = self.current_usage();
        self.peak = self.peak.max(current);
        self.peak
    }
}

fn get_current_memory_usage() -> usize {
    // In production, this would query actual memory usage
    // For benchmarking, we'll estimate based on allocations
    0
}

type Model = Vec<u8>;
type Image = Vec<u8>;
type Tensor = Vec<f32>;
type Output = Vec<f32>;

fn load_model() -> Model {
    vec![0u8; 100 * 1024 * 1024] // 100 MB model
}

fn load_detection_model() -> Model {
    vec![0u8; 150 * 1024 * 1024] // 150 MB
}

fn load_recognition_model() -> Model {
    vec![0u8; 80 * 1024 * 1024] // 80 MB
}

fn load_math_model() -> Model {
    vec![0u8; 120 * 1024 * 1024] // 120 MB
}

fn create_image(width: u32, height: u32) -> Image {
    vec![128u8; (width * height * 3) as usize]
}

fn create_batch(batch_size: usize, width: u32, height: u32) -> Vec<Image> {
    (0..batch_size)
        .map(|_| create_image(width, height))
        .collect()
}

fn preprocess(image: Image) -> Tensor {
    image.iter().map(|&x| x as f32 / 255.0).collect()
}

fn run_inference(_model: &Model, input: Tensor) -> Output {
    input.iter().map(|&x| x * 2.0).collect()
}

fn run_batch_inference(_model: &Model, batch: Vec<Image>) -> Vec<Output> {
    batch
        .into_iter()
        .map(|img| {
            let tensor = preprocess(img);
            tensor.iter().map(|&x| x * 2.0).collect()
        })
        .collect()
}

fn postprocess(output: Output) -> String {
    format!("result_{:.2}", output[0])
}

fn calculate_memory_growth(samples: &[usize]) -> f64 {
    if samples.len() < 2 {
        return 0.0;
    }

    let first = samples[0] as f64;
    let last = samples[samples.len() - 1] as f64;

    (last - first) / first
}

fn create_embedding_cache(size: usize) -> Vec<Vec<f32>> {
    (0..size).map(|_| vec![0.5f32; 512]).collect()
}

struct MemoryPool {
    block_size: usize,
    blocks: Vec<Vec<u8>>,
    available: Vec<usize>,
}

impl MemoryPool {
    fn new(block_size: usize, count: usize) -> Self {
        let blocks = (0..count).map(|_| vec![0u8; block_size]).collect();
        let available = (0..count).collect();

        Self {
            block_size,
            blocks,
            available,
        }
    }

    fn allocate(&mut self) -> Option<usize> {
        self.available.pop()
    }
}

fn create_hwc_tensor(height: u32, width: u32, channels: u32) -> Vec<f32> {
    vec![0.5f32; (height * width * channels) as usize]
}

fn create_chw_tensor(height: u32, width: u32, channels: u32) -> Vec<f32> {
    vec![0.5f32; (channels * height * width) as usize]
}

fn convert_hwc_to_chw(hwc: &[f32], height: u32, width: u32, channels: u32) -> Vec<f32> {
    let mut chw = Vec::with_capacity(hwc.len());

    for c in 0..channels {
        for h in 0..height {
            for w in 0..width {
                let hwc_idx = ((h * width + w) * channels + c) as usize;
                chw.push(hwc[hwc_idx]);
            }
        }
    }

    chw
}

criterion_group!(
    benches,
    bench_peak_memory_inference,
    bench_memory_per_batch_image,
    bench_model_loading_memory,
    bench_memory_growth,
    bench_memory_fragmentation,
    bench_cache_memory,
    bench_memory_pools,
    bench_tensor_layouts
);
criterion_main!(benches);
