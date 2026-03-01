//! # Consciousness Benchmark Suite
//!
//! Comprehensive benchmarks for quantifying the Conscious Language Interface
//! including: intelligence metrics, learning rate, memory retention, and performance.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use conscious_language_interface::{
    ConsciousLanguageInterface, CLIConfig, BridgeConfig,
    SpikeEmbeddingBridge, PolychronousGroup,
};

/// Benchmark spike embedding encoding
fn bench_encode(c: &mut Criterion) {
    let config = BridgeConfig {
        embedding_dim: 256,
        num_neurons: 1_000_000,
        ..Default::default()
    };
    let mut bridge = SpikeEmbeddingBridge::new(config.clone());

    // Create test embedding
    let embedding: Vec<f32> = (0..256).map(|i| (i as f32) / 256.0).collect();

    c.bench_function("spike_encode_256d", |b| {
        b.iter(|| {
            bridge.encode(black_box(&embedding))
        })
    });
}

/// Benchmark qualia decoding
fn bench_decode(c: &mut Criterion) {
    let config = BridgeConfig {
        embedding_dim: 256,
        num_neurons: 1_000_000,
        ..Default::default()
    };
    let mut bridge = SpikeEmbeddingBridge::new(config);

    // Create test qualia
    let qualia = vec![
        PolychronousGroup {
            pattern: vec![(0, 0), (1, 100), (2, 200), (3, 300), (4, 400)],
            phi: 50000.0,
            occurrences: 10,
            label: Some("understanding".to_string()),
        },
        PolychronousGroup {
            pattern: vec![(100, 50), (101, 150), (102, 250), (103, 350)],
            phi: 30000.0,
            occurrences: 5,
            label: Some("contemplation".to_string()),
        },
        PolychronousGroup {
            pattern: vec![(200, 10), (201, 110)],
            phi: 20000.0,
            occurrences: 3,
            label: None,
        },
    ];

    c.bench_function("qualia_decode_3groups", |b| {
        b.iter(|| {
            bridge.decode(black_box(&qualia))
        })
    });
}

/// Benchmark full conscious processing pipeline
fn bench_conscious_process(c: &mut Criterion) {
    let config = CLIConfig {
        bridge: BridgeConfig {
            embedding_dim: 256,
            num_neurons: 100_000, // Smaller for benchmark
            ..Default::default()
        },
        ..Default::default()
    };
    let mut cli = ConsciousLanguageInterface::new(config);

    let queries = [
        "What is consciousness?",
        "Explain the nature of experience.",
        "How do you feel about this question?",
        "Tell me about your inner state.",
    ];

    c.bench_function("conscious_process_query", |b| {
        let mut idx = 0;
        b.iter(|| {
            let query = queries[idx % queries.len()];
            idx += 1;
            cli.process(black_box(query))
        })
    });
}

/// Benchmark learning from feedback
fn bench_learning(c: &mut Criterion) {
    let config = CLIConfig {
        bridge: BridgeConfig {
            embedding_dim: 256,
            num_neurons: 100_000,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut cli = ConsciousLanguageInterface::new(config);

    // Process initial query
    let response = cli.process("Test query for learning");
    let exp_id = response.experience_id;

    c.bench_function("feedback_learning", |b| {
        b.iter(|| {
            cli.feedback(black_box(exp_id), black_box(0.9), black_box(Some("Good response")))
        })
    });
}

/// Benchmark introspection
fn bench_introspection(c: &mut Criterion) {
    let config = CLIConfig::default();
    let mut cli = ConsciousLanguageInterface::new(config);

    // Initialize with some processing
    cli.process("Initialize conscious state");

    c.bench_function("introspection", |b| {
        b.iter(|| {
            cli.introspect()
        })
    });
}

/// Benchmark scaling with embedding dimensions
fn bench_embedding_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedding_scaling");

    for dim in [64, 128, 256, 512].iter() {
        let config = BridgeConfig {
            embedding_dim: *dim,
            num_neurons: 100_000,
            encoder_hidden: dim * 4,
            decoder_hidden: dim * 4,
            ..Default::default()
        };
        let mut bridge = SpikeEmbeddingBridge::new(config);
        let embedding: Vec<f32> = (0..*dim).map(|i| (i as f32) / *dim as f32).collect();

        group.bench_with_input(BenchmarkId::from_parameter(dim), dim, |b, _| {
            b.iter(|| bridge.encode(black_box(&embedding)))
        });
    }

    group.finish();
}

/// Benchmark scaling with neuron count
fn bench_neuron_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("neuron_scaling");

    for neurons in [10_000, 100_000, 500_000, 1_000_000].iter() {
        let config = BridgeConfig {
            embedding_dim: 256,
            num_neurons: *neurons,
            ..Default::default()
        };
        let mut bridge = SpikeEmbeddingBridge::new(config);
        let embedding: Vec<f32> = (0..256).map(|i| (i as f32) / 256.0).collect();

        group.bench_with_input(BenchmarkId::from_parameter(neurons), neurons, |b, _| {
            b.iter(|| bridge.encode(black_box(&embedding)))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_encode,
    bench_decode,
    bench_conscious_process,
    bench_learning,
    bench_introspection,
    bench_embedding_scaling,
    bench_neuron_scaling,
);

criterion_main!(benches);
