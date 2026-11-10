use crate::benchmarks::{BenchDevice, BenchDeviceHandler};
use candle_core::{DType, Device, Tensor};
use criterion::{criterion_group, Criterion, Throughput};
use std::hint::black_box;
use std::time::Instant;

fn rand_uniform(a: &Tensor) {
    a.rand_like(-1.0, 123.0).unwrap();
}

fn rand_normal(a: &Tensor) {
    a.randn_like(100.0, 15.0).unwrap();
}

fn run_random_bench(c: &mut Criterion, device: &Device) {
    let b = 1;

    let rows = 2048;
    let cols = 2048;

    let dtype = DType::F32;
    let tensor = Tensor::zeros((b, rows, cols), dtype, device).unwrap();

    let flops = b * rows * cols * dtype.size_in_bytes();

    let mut group = c.benchmark_group(device.bench_name("random_uniform"));
    group.throughput(Throughput::Bytes(flops as u64));
    group.bench_function("iter", move |benches| {
        benches.iter_custom(|iters| {
            let start = Instant::now();
            for _i in 0..iters {
                rand_uniform(black_box(&tensor));
            }
            device.sync().unwrap();
            start.elapsed()
        })
    });
    group.finish();

    let tensor = Tensor::zeros((b, rows, cols), dtype, device).unwrap();

    let mut group = c.benchmark_group(device.bench_name("random_normal"));
    group.throughput(Throughput::Bytes(flops as u64));
    group.bench_function("iter", move |benches| {
        benches.iter_custom(|iters| {
            let start = Instant::now();
            for _i in 0..iters {
                rand_normal(black_box(&tensor));
            }
            device.sync().unwrap();
            start.elapsed()
        })
    });
    group.finish();
}

fn criterion_benchmark(c: &mut Criterion) {
    let handler = BenchDeviceHandler::new().unwrap();
    for device in handler.devices {
        run_random_bench(c, &device);
    }
}

criterion_group!(benches, criterion_benchmark);
