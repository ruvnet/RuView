use crate::benchmarks::{BenchDevice, BenchDeviceHandler};
use candle_core::{DType, Device, Tensor};
use criterion::{criterion_group, Criterion, Throughput};
use std::hint::black_box;
use std::time::Instant;

/// Matmul benchmark shapes covering common GEMM scenarios
const MATMUL_SHAPES: &[(&str, &[usize], &[usize])] = &[
    // Original GEMV test
    ("gemv", &[1, 1, 2048], &[1, 2048, 2048]),
    // 4D Attention scenarios (multi-head attention)
    ("attn_4d_small", &[484, 6, 144, 32], &[484, 6, 32, 144]),
    ("attn_4d_large", &[121, 24, 144, 32], &[121, 24, 32, 144]),
    // Square matrix tests
    ("square_512", &[512, 512], &[512, 512]),
    ("square_1024", &[1024, 1024], &[1024, 1024]),
    // 3D Batch matmul (attention patterns)
    ("batch_1000", &[1000, 144, 32], &[1000, 32, 144]),
    // 2D Linear layer scenarios (transformer FFN)
    ("linear_large", &[17424, 768], &[768, 3072]),
];

fn run(a: &Tensor, b: &Tensor) {
    a.broadcast_matmul(b).unwrap();
}

fn calculate_flops(shape_a: &[usize], shape_b: &[usize]) -> usize {
    let batch: usize = shape_a
        .iter()
        .take(shape_a.len().saturating_sub(2))
        .product();
    let batch = if batch == 0 { 1 } else { batch };
    let m = shape_a[shape_a.len() - 2];
    let k = shape_a[shape_a.len() - 1];
    let n = shape_b[shape_b.len() - 1];
    2 * batch * m * k * n
}

fn run_bench(c: &mut Criterion, device: &Device, name: &str, shape_a: &[usize], shape_b: &[usize]) {
    let dtype = DType::F32;
    let lhs = Tensor::zeros(shape_a, dtype, device).unwrap();
    let rhs = Tensor::zeros(shape_b, dtype, device).unwrap();

    let flops = calculate_flops(shape_a, shape_b);

    let mut group = c.benchmark_group(device.bench_name(format!("matmul_{name}")));
    group.throughput(Throughput::Bytes(flops as u64));
    group.bench_function("iter", move |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _i in 0..iters {
                run(black_box(&lhs), black_box(&rhs));
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
        for (name, shape_a, shape_b) in MATMUL_SHAPES {
            run_bench(c, &device, name, shape_a, shape_b);
        }
    }
}

criterion_group!(benches, criterion_benchmark);
