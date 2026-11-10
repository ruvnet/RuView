use crate::benchmarks::{BenchDevice, BenchDeviceHandler};
use candle_core::{DType, Device, Tensor};
use criterion::{criterion_group, Criterion, Throughput};
use std::hint::black_box;
use std::time::Instant;

fn run(a: &Tensor, b: &Tensor, c: &Tensor) {
    a.where_cond(b, c).unwrap();
}

const fn create_cond_arr<const N: usize>() -> [u8; N] {
    let mut arr = [0u8; N];
    let mut i = 0;
    while i < N {
        arr[i] = (i % 2) as u8;
        i += 1;
    }
    arr
}

const B: usize = 1;
const M: usize = 1024;
const K: usize = 1024;
const SIZE: usize = B * M * K;

static DATA: [u8; SIZE] = create_cond_arr::<SIZE>();

fn run_where_cond_benchmark(c: &mut Criterion, device: &Device, dtype: DType, name: &str) {
    let tensor = Tensor::from_slice(DATA.as_slice(), (B, M, K), device).unwrap();
    let on_true = Tensor::ones((B, M, K), dtype, device).unwrap();
    let on_false = Tensor::zeros((B, M, K), dtype, device).unwrap();

    let elements = B * M * K;
    // E.g. 2 f32 tensors + 1 u8 tensor
    let flops = (2 * elements * dtype.size_in_bytes()) + elements;

    let mut group = c.benchmark_group(device.bench_name(name));
    group.throughput(Throughput::Bytes(flops as u64));
    group.bench_function("iter", move |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _i in 0..iters {
                run(
                    black_box(&tensor),
                    black_box(&on_true),
                    black_box(&on_false),
                );
            }
            device.sync().unwrap();
            start.elapsed()
        })
    });
    group.finish();
}

fn criterion_benchmark(c: &mut Criterion) {
    let device = BenchDeviceHandler::new().unwrap();
    for d in device.devices {
        run_where_cond_benchmark(c, &d, DType::F32, "where_cond_f32");
        run_where_cond_benchmark(c, &d, DType::BF16, "where_cond_bf16");
        run_where_cond_benchmark(c, &d, DType::F16, "where_cond_f16");
    }
}

criterion_group!(benches, criterion_benchmark);
