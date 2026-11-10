use crate::benchmarks::{BenchDevice, BenchDeviceHandler};
use candle_core::{DType, Device, Tensor};
use criterion::{criterion_group, Criterion, Throughput};
use std::hint::black_box;
use std::time::Instant;

fn run(
    x: &Tensor,
    k: &Tensor,
    padding: usize,
    output_padding: usize,
    stride: usize,
    dilation: usize,
) {
    x.conv_transpose2d(k, padding, output_padding, stride, dilation)
        .unwrap();
}

fn run_benchmark(c: &mut Criterion, device: &Device, dtype: DType, name: &str) {
    let t = Tensor::arange(0.0f32, 10000.0, device)
        .unwrap()
        .reshape((1, 4, 50, 50))
        .unwrap()
        .to_dtype(dtype)
        .unwrap();

    let kernel = Tensor::arange(0.0f32, 100.0, device)
        .unwrap()
        .reshape((4, 1, 5, 5))
        .unwrap()
        .to_dtype(dtype)
        .unwrap();

    let flops = t.dims().iter().product::<usize>() * dtype.size_in_bytes();

    let mut group = c.benchmark_group(device.bench_name(name));
    group.throughput(Throughput::Bytes(flops as u64));
    group.bench_function("iter", move |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _i in 0..iters {
                run(black_box(&t), black_box(&kernel), 1, 0, 1, 2);
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
        run_benchmark(c, &device, DType::F32, "conv_transpose2d_f32");
        run_benchmark(c, &device, DType::F16, "conv_transpose2d_f16");
        run_benchmark(c, &device, DType::BF16, "conv_transpose2d_bf16");
    }
}

criterion_group!(benches, criterion_benchmark);
