// Neural Field Benchmark - Memory-mapped operations performance
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use demand_paged_cognition::*;
use tempfile::NamedTempFile;

fn bench_hash_address(c: &mut Criterion) {
    let temp = NamedTempFile::new().unwrap();
    let field = MmapNeuralField::new(
        temp.path(),
        1024 * 1024 * 1024,    // 1 GB
        Some(4 * 1024 * 1024), // 4 MB pages
    )
    .unwrap();

    let mut group = c.benchmark_group("hash_address");

    for size in [4, 16, 64, 256, 1024].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let concept = vec![0.1f32; size];
            b.iter(|| field.hash_address(black_box(&concept)));
        });
    }
    group.finish();
}

fn bench_read_write(c: &mut Criterion) {
    let temp = NamedTempFile::new().unwrap();
    let field = MmapNeuralField::new(
        temp.path(),
        1024 * 1024 * 1024, // 1 GB
        Some(4 * 1024 * 1024),
    )
    .unwrap();

    let mut group = c.benchmark_group("read_write");

    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes((*size * 4) as u64)); // f32 = 4 bytes

        // Write benchmark
        group.bench_with_input(BenchmarkId::new("write", size), size, |b, &size| {
            let data = vec![1.0f32; size];
            b.iter(|| field.write(black_box(0), black_box(&data)).unwrap());
        });

        // Read benchmark
        field.write(0, &vec![1.0f32; *size]).unwrap();
        group.bench_with_input(BenchmarkId::new("read", size), size, |b, &size| {
            b.iter(|| field.read(black_box(0), black_box(size)).unwrap());
        });
    }
    group.finish();
}

fn bench_lazy_layer_forward(c: &mut Criterion) {
    let temp = NamedTempFile::new().unwrap();
    let storage = std::sync::Arc::new(
        MmapNeuralField::new(temp.path(), 1024 * 1024 * 1024, Some(4096)).unwrap(),
    );

    let mut group = c.benchmark_group("lazy_layer");

    for (input_dim, output_dim) in [(10, 10), (100, 100), (256, 256), (512, 512)].iter() {
        // Initialize weights
        let weights = vec![0.1f32; input_dim * output_dim];
        let bias = vec![0.01f32; *output_dim];
        storage.write(0, &weights).unwrap();
        storage.write((weights.len() * 4) as u64, &bias).unwrap();

        let mut layer = LazyLayer::new(
            0,
            (weights.len() * 4) as u64,
            *input_dim,
            *output_dim,
            storage.clone(),
        );

        group.throughput(Throughput::Elements((*input_dim * *output_dim) as u64));
        group.bench_with_input(
            BenchmarkId::new("forward", format!("{}x{}", input_dim, output_dim)),
            &(*input_dim, *output_dim),
            |b, &(input_dim, _)| {
                let input = vec![1.0f32; input_dim];
                b.iter(|| layer.forward(black_box(&input)).unwrap());
            },
        );
    }
    group.finish();
}

fn bench_tiered_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("tiered_memory");

    // Promotion benchmark
    group.bench_function("promote_l4_to_l1", |b| {
        b.iter_with_setup(
            || {
                let mut memory = TieredMemory::new();
                let page = Page::new(1, vec![1.0; 1024], Tier::L4Hdd);
                memory.insert(page).unwrap();
                memory
            },
            |mut memory| memory.promote(1, Tier::L1Dram, "bench").unwrap(),
        );
    });

    // Load benchmark (includes promotion)
    group.bench_function("load_page", |b| {
        b.iter_with_setup(
            || {
                let mut memory = TieredMemory::new();
                let page = Page::new(1, vec![1.0; 1024], Tier::L4Hdd);
                memory.insert(page).unwrap();
                memory
            },
            |mut memory| memory.load(1).unwrap(),
        );
    });

    group.finish();
}

fn bench_prefetch_prediction(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch");

    // Hoeffding Tree prediction
    group.bench_function("hoeffding_predict", |b| {
        let predictor = HoeffdingTreePredictor::new();

        // Train with some data
        for i in 0..100 {
            let page = (i % 10) as u64;
            let features = AccessFeatures::new(page);
            predictor.update(page, &features);
        }

        let features = AccessFeatures::new(5);
        b.iter(|| predictor.predict(black_box(&features), black_box(10)));
    });

    // Markov prediction
    group.bench_function("markov_predict", |b| {
        let predictor = MarkovPredictor::new();

        // Build transition pattern
        for _ in 0..10 {
            predictor.update(1, 2);
            predictor.update(2, 3);
            predictor.update(3, 1);
        }

        b.iter(|| predictor.predict(black_box(1), black_box(10)));
    });

    // Coordinator
    group.bench_function("coordinator_predict", |b| {
        let coordinator = PrefetchCoordinator::new();
        let context = vec![0.1, 0.2, 0.3];

        // Record some history
        for i in 0..50 {
            coordinator.record_access(i, &context);
        }

        b.iter(|| coordinator.predict_and_queue(black_box(50), black_box(&context), black_box(5)));
    });

    group.finish();
}

fn bench_dpnc_system(c: &mut Criterion) {
    let mut group = c.benchmark_group("dpnc_system");
    group.sample_size(50); // Reduce sample size for expensive operations

    group.bench_function("full_query", |b| {
        b.iter_with_setup(
            || {
                let temp = NamedTempFile::new().unwrap();
                let config = DPNCConfig::default();
                DPNC::new(temp.path(), config).unwrap()
            },
            |mut dpnc| {
                let concept = vec![0.1, 0.2, 0.3, 0.4];
                dpnc.query(black_box(&concept)).unwrap()
            },
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_hash_address,
    bench_read_write,
    bench_lazy_layer_forward,
    bench_tiered_memory,
    bench_prefetch_prediction,
    bench_dpnc_system
);
criterion_main!(benches);
