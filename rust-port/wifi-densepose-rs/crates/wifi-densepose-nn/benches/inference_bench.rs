//! Benchmarks for neural network inference.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use wifi_densepose_nn::{
    densepose::{DensePoseConfig, DensePoseHead},
    inference::{EngineBuilder, InferenceOptions, MockBackend, Backend},
    tensor::{Tensor, TensorShape},
    translator::{ModalityTranslator, TranslatorConfig},
};

fn bench_tensor_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("tensor_ops");

    for size in [32, 64, 128].iter() {
        let tensor = Tensor::zeros_4d([1, 256, *size, *size]);

        group.throughput(Throughput::Elements((size * size * 256) as u64));

        group.bench_with_input(BenchmarkId::new("relu", size), size, |b, _| {
            b.iter(|| black_box(tensor.relu().unwrap()))
        });

        group.bench_with_input(BenchmarkId::new("sigmoid", size), size, |b, _| {
            b.iter(|| black_box(tensor.sigmoid().unwrap()))
        });

        group.bench_with_input(BenchmarkId::new("tanh", size), size, |b, _| {
            b.iter(|| black_box(tensor.tanh().unwrap()))
        });
    }

    group.finish();
}

fn bench_densepose_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("densepose_forward");

    let config = DensePoseConfig::new(256, 24, 2);
    let head = DensePoseHead::new(config).unwrap();

    for size in [32, 64].iter() {
        let input = Tensor::zeros_4d([1, 256, *size, *size]);

        group.throughput(Throughput::Elements((size * size * 256) as u64));

        group.bench_with_input(BenchmarkId::new("mock_forward", size), size, |b, _| {
            b.iter(|| black_box(head.forward(&input).unwrap()))
        });
    }

    group.finish();
}

fn bench_translator_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("translator_forward");

    let config = TranslatorConfig::new(128, vec![256, 512, 256], 256);
    let translator = ModalityTranslator::new(config).unwrap();

    for size in [32, 64].iter() {
        let input = Tensor::zeros_4d([1, 128, *size, *size]);

        group.throughput(Throughput::Elements((size * size * 128) as u64));

        group.bench_with_input(BenchmarkId::new("mock_forward", size), size, |b, _| {
            b.iter(|| black_box(translator.forward(&input).unwrap()))
        });
    }

    group.finish();
}

fn bench_mock_inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("mock_inference");

    let engine = EngineBuilder::new().build_mock();
    let input = Tensor::zeros_4d([1, 256, 64, 64]);

    group.throughput(Throughput::Elements(1));

    group.bench_function("single_inference", |b| {
        b.iter(|| black_box(engine.infer(&input).unwrap()))
    });

    group.finish();
}

fn bench_batch_inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_inference");

    let engine = EngineBuilder::new().build_mock();

    for batch_size in [1, 2, 4, 8].iter() {
        let inputs: Vec<Tensor> = (0..*batch_size)
            .map(|_| Tensor::zeros_4d([1, 256, 64, 64]))
            .collect();

        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            batch_size,
            |b, _| {
                b.iter(|| black_box(engine.infer_batch(&inputs).unwrap()))
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_tensor_operations,
    bench_densepose_forward,
    bench_translator_forward,
    bench_mock_inference,
    bench_batch_inference,
);

criterion_main!(benches);
