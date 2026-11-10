use crate::benchmarks::{BenchDevice, BenchDeviceHandler};
use candle_core::{DType, Device, Tensor};
use criterion::{criterion_group, Criterion, Throughput};
use std::hint::black_box;
use std::time::Instant;

fn run(a: &Tensor) {
    a.affine(12.34, 56.78).unwrap();
}

fn run_affine_benchmark(c: &mut Criterion, device: &Device, dtype: DType, name: &str) {
    let b = 1;
    let m = 1024;
    let k = 1024;

    let tensor = Tensor::zeros((b, m, k), dtype, device).unwrap();

    let flops = b * m * k * dtype.size_in_bytes();

    let mut group = c.benchmark_group(device.bench_name(name));
    group.throughput(Throughput::Bytes(flops as u64));
    group.bench_function("iter", move |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _i in 0..iters {
                run(black_box(&tensor));
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
        run_affine_benchmark(c, &device, DType::F32, "affine_f32");
        run_affine_benchmark(c, &device, DType::F16, "affine_f16");
        run_affine_benchmark(c, &device, DType::BF16, "affine_bf16");
        #[cfg(not(feature = "metal"))]
        run_affine_benchmark(c, &device, DType::F8E4M3, "affine_fp8");
    }
}

criterion_group!(benches, criterion_benchmark);
