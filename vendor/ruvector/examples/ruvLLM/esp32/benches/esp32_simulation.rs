//! ESP32 Simulation Benchmarks
//!
//! Simulates ESP32 performance constraints to validate the implementation
//! will work on actual hardware.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;

// Import the ESP32 crate (compiled for host for simulation)
#[path = "../src/lib.rs"]
mod ruvllm_esp32;

use ruvllm_esp32::prelude::*;
use ruvllm_esp32::model::ModelConfig;
use ruvllm_esp32::quantized::{QuantizationType, matmul_int8, QuantParams};
use ruvllm_esp32::attention::MicroAttention;

/// ESP32 clock speed in MHz
const ESP32_CLOCK_MHZ: u64 = 240;

/// Estimated cycles per INT8 multiply-accumulate on ESP32
const CYCLES_PER_MAC: u64 = 4;

/// Estimate ESP32 execution time from x86 measurement
fn estimate_esp32_time(x86_duration: Duration, mac_ops: u64) -> Duration {
    // ESP32 is roughly 10-20x slower than modern x86 for pure compute
    // But INT8 operations are more efficient
    let estimated_cycles = mac_ops * CYCLES_PER_MAC;
    let esp32_seconds = estimated_cycles as f64 / (ESP32_CLOCK_MHZ as f64 * 1_000_000.0);
    Duration::from_secs_f64(esp32_seconds.max(x86_duration.as_secs_f64() * 15.0))
}

fn benchmark_matmul_int8(c: &mut Criterion) {
    let mut group = c.benchmark_group("INT8 MatMul");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));

    // Test different sizes typical for ESP32 models
    for (out_dim, in_dim) in [(32, 32), (64, 64), (128, 64), (64, 128)] {
        let weights: Vec<i8> = (0..out_dim * in_dim)
            .map(|i| ((i * 17) % 256) as i8 - 128)
            .collect();
        let input: Vec<i8> = (0..in_dim)
            .map(|i| ((i * 13) % 256) as i8 - 128)
            .collect();
        let mut output = vec![0i32; out_dim];

        let params = QuantParams::default();

        let mac_ops = (out_dim * in_dim) as u64;

        group.bench_with_input(
            BenchmarkId::new("size", format!("{}x{}", out_dim, in_dim)),
            &(out_dim, in_dim),
            |b, _| {
                b.iter(|| {
                    matmul_int8(
                        black_box(&weights),
                        black_box(&params),
                        black_box(&input),
                        black_box(&params),
                        black_box(&mut output),
                        out_dim,
                        in_dim,
                    )
                })
            },
        );

        // Print ESP32 estimate
        println!(
            "  {}x{}: {} MAC ops, estimated ESP32 time: {:.1} us",
            out_dim, in_dim, mac_ops,
            mac_ops as f64 * CYCLES_PER_MAC as f64 / ESP32_CLOCK_MHZ as f64
        );
    }

    group.finish();
}

fn benchmark_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("Micro Attention");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));

    for (embed_dim, num_heads, seq_len) in [(64, 4, 16), (64, 4, 32), (32, 2, 16)] {
        let head_dim = embed_dim / num_heads;
        let attn = MicroAttention::new(embed_dim, num_heads);

        let query: Vec<i8> = (0..head_dim).map(|i| (i * 7 % 128) as i8).collect();
        let keys: Vec<Vec<i8>> = (0..seq_len)
            .map(|s| (0..head_dim).map(|i| ((i + s) * 11 % 128) as i8).collect())
            .collect();
        let key_refs: Vec<&[i8]> = keys.iter().map(|k| k.as_slice()).collect();
        let mut scores = vec![0i32; seq_len];

        group.bench_with_input(
            BenchmarkId::new("config", format!("d{}_h{}_s{}", embed_dim, num_heads, seq_len)),
            &seq_len,
            |b, _| {
                b.iter(|| {
                    attn.compute_scores(
                        black_box(&query),
                        black_box(&key_refs),
                        black_box(&mut scores),
                    )
                })
            },
        );
    }

    group.finish();
}

fn benchmark_full_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("Full Forward Pass");
    group.warm_up_time(Duration::from_millis(1000));
    group.measurement_time(Duration::from_secs(5));

    // Test configurations for different ESP32 variants
    let configs = [
        ("ESP32", ModelConfig {
            vocab_size: 256,
            embed_dim: 64,
            hidden_dim: 128,
            num_layers: 2,
            num_heads: 4,
            max_seq_len: 32,
            quant_type: QuantizationType::Int8,
        }),
        ("ESP32-S2", ModelConfig {
            vocab_size: 128,
            embed_dim: 32,
            hidden_dim: 64,
            num_layers: 1,
            num_heads: 2,
            max_seq_len: 16,
            quant_type: QuantizationType::Int8,
        }),
        ("ESP32-S3", ModelConfig {
            vocab_size: 512,
            embed_dim: 64,
            hidden_dim: 128,
            num_layers: 2,
            num_heads: 4,
            max_seq_len: 32,
            quant_type: QuantizationType::Int8,
        }),
    ];

    for (variant, config) in configs {
        let model = TinyModel::new(config.clone()).unwrap();
        let mut engine = MicroEngine::new(model).unwrap();

        let model_size = config.estimate_size();

        group.bench_with_input(
            BenchmarkId::new("variant", variant),
            &variant,
            |b, _| {
                b.iter(|| {
                    engine.reset();
                    black_box(engine.forward_one(black_box(42)).unwrap())
                })
            },
        );

        println!(
            "  {}: model size {} KB, embed_dim {}, layers {}",
            variant, model_size / 1024, config.embed_dim, config.num_layers
        );
    }

    group.finish();
}

fn benchmark_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Token Generation");
    group.warm_up_time(Duration::from_millis(1000));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(20); // Fewer samples for slower operation

    let config = ModelConfig::for_variant(Esp32Variant::Esp32);
    let model = TinyModel::new(config).unwrap();
    let mut engine = MicroEngine::new(model).unwrap();

    let prompt = [1u16, 2, 3, 4, 5];
    let gen_config = InferenceConfig {
        max_tokens: 10,
        greedy: true,
        ..Default::default()
    };

    group.bench_function("generate_10_tokens", |b| {
        b.iter(|| {
            engine.reset();
            black_box(engine.generate(black_box(&prompt), black_box(&gen_config)).unwrap())
        })
    });

    group.finish();
}

fn benchmark_memory_constraints(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory Validation");

    // Validate that models fit within ESP32 memory constraints
    for variant in [
        Esp32Variant::Esp32,
        Esp32Variant::Esp32S2,
        Esp32Variant::Esp32S3,
        Esp32Variant::Esp32C3,
        Esp32Variant::Esp32C6,
    ] {
        let config = ModelConfig::for_variant(variant);
        let model = TinyModel::new(config.clone()).unwrap();
        let engine = MicroEngine::new(model).unwrap();

        let usage = engine.memory_usage();
        let available = variant.max_model_ram();

        println!("  {:?}:", variant);
        println!("    Available RAM: {} KB", available / 1024);
        println!("    Model weights: {} KB", usage.model_weights / 1024);
        println!("    Activations: {} KB", usage.activation_buffers / 1024);
        println!("    KV cache: {} KB", usage.kv_cache / 1024);
        println!("    Total used: {} KB", usage.total / 1024);
        println!("    Headroom: {} KB", (available - usage.total) / 1024);
        println!();

        assert!(
            usage.total <= available,
            "{:?} exceeds memory: {} > {}",
            variant, usage.total, available
        );
    }

    // Dummy benchmark to satisfy criterion
    group.bench_function("memory_check", |b| {
        b.iter(|| black_box(Esp32Variant::Esp32.max_model_ram()))
    });

    group.finish();
}

fn benchmark_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("Quantization");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));

    use ruvllm_esp32::quantized::QuantizedTensor;

    // Test quantization of different sized tensors
    for size in [256, 1024, 4096] {
        let data: Vec<f32> = (0..size)
            .map(|i| (i as f32 / size as f32) * 2.0 - 1.0)
            .collect();

        group.bench_with_input(
            BenchmarkId::new("int8", size),
            &size,
            |b, _| {
                b.iter(|| {
                    QuantizedTensor::<16384>::from_f32(
                        black_box(&data),
                        &[size],
                        QuantizationType::Int8,
                    ).unwrap()
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("int4", size),
            &size,
            |b, _| {
                b.iter(|| {
                    QuantizedTensor::<16384>::from_f32(
                        black_box(&data),
                        &[size],
                        QuantizationType::Int4,
                    ).unwrap()
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("binary", size),
            &size,
            |b, _| {
                b.iter(|| {
                    QuantizedTensor::<16384>::from_f32(
                        black_box(&data),
                        &[size],
                        QuantizationType::Binary,
                    ).unwrap()
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_matmul_int8,
    benchmark_attention,
    benchmark_full_forward,
    benchmark_generation,
    benchmark_memory_constraints,
    benchmark_quantization,
);

criterion_main!(benches);
