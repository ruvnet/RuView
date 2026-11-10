use crate::benchmarks::{BenchDevice, BenchDeviceHandler};
use candle_core::{DType, Device, Tensor};
use criterion::{criterion_group, Criterion, Throughput};
use std::hint::black_box;
use std::time::Instant;

fn run_sqrt(a: &Tensor) {
    a.sqrt().unwrap();
}

fn run_unary_benchmark(c: &mut Criterion, device: &Device, dtype: DType, name: &str) {
    let b = 1;
    let m = 1024;
    let k = 1024;

    let tensor = Tensor::arange(0.0f32, (b * m * k) as f32, device)
        .unwrap()
        .to_dtype(dtype)
        .unwrap()
        .reshape((b, m, k))
        .unwrap();

    let flops = b * m * k * dtype.size_in_bytes();

    let mut group = c.benchmark_group(device.bench_name(name));
    group.throughput(Throughput::Bytes(flops as u64));
    group.bench_function("iter", move |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _i in 0..iters {
                run_sqrt(black_box(&tensor));
            }
            device.sync().unwrap();
            start.elapsed()
        })
    });
    group.finish();
}

fn run_cast(a: &Tensor, dtype: DType) {
    a.to_dtype(dtype).unwrap();
}

fn run_cast_benchmark(
    c: &mut Criterion,
    device: &Device,
    dtype: DType,
    to_dtype: DType,
    name: &str,
) {
    let b = 1;
    let m = 1024;
    let k = 1024;

    let tensor = Tensor::arange(0.0f32, (b * m * k) as f32, device)
        .unwrap()
        .to_dtype(dtype)
        .unwrap()
        .reshape((b, m, k))
        .unwrap();

    let flops = b * m * k * dtype.size_in_bytes();

    let mut group = c.benchmark_group(device.bench_name(name));
    group.throughput(Throughput::Bytes(flops as u64));
    group.bench_function("iter", move |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _i in 0..iters {
                run_cast(black_box(&tensor), black_box(to_dtype));
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
        for dtype in [DType::F32, DType::BF16, DType::F16] {
            let to_dtype = if matches!(dtype, DType::F32) {
                DType::F16
            } else {
                DType::F32
            };
            let name = format!("cast_{}_{}", dtype.as_str(), to_dtype.as_str());
            run_cast_benchmark(c, &device, dtype, to_dtype, &name);
        }
        for dtype in [DType::F32, DType::BF16, DType::F16] {
            let name = format!("sqrt_{dtype:?}");
            run_unary_benchmark(c, &device, dtype, &name);
        }
    }
}

criterion_group!(benches, criterion_benchmark);
