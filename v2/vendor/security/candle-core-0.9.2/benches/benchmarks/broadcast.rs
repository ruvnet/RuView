use crate::benchmarks::{BenchDevice, BenchDeviceHandler};
use candle_core::{DType, Device, Tensor};
use criterion::{criterion_group, Criterion, Throughput};
use std::hint::black_box;
use std::time::Instant;

fn run(w: &Tensor, bias: &Tensor) {
    w.broadcast_add(bias).unwrap();
}

fn run_benchmark(c: &mut Criterion, device: &Device, dtype: DType, name: &str) {
    // We simulate a candle-nn style conv2d + bias forward pass.
    let batch_size = 1;
    let ch = 1;
    let m = 126;
    let bias_size = 128;

    let x = Tensor::ones((batch_size, ch, m, m), dtype, device).unwrap();
    let bias = Tensor::ones((1, bias_size, 1, 1), dtype, device).unwrap();

    let flops = batch_size * ch * m * bias_size * dtype.size_in_bytes();

    let mut group = c.benchmark_group(device.bench_name(name));
    group.throughput(Throughput::Bytes(flops as u64));
    group.bench_function("iter", move |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _i in 0..iters {
                run(black_box(&x), black_box(&bias));
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
        run_benchmark(c, &device, DType::F32, "broadcast_add_f32");
        run_benchmark(c, &device, DType::F16, "broadcast_add_f16");
        run_benchmark(c, &device, DType::BF16, "broadcast_add_bf16");
    }
}

criterion_group!(benches, criterion_benchmark);
