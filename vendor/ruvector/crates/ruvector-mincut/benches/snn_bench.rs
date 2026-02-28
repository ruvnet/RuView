//! Benchmarks for SNN-MinCut Integration
//!
//! Measures:
//! - LIF neuron step performance
//! - STDP weight update throughput
//! - Network propagation latency
//! - Karger-Stein mincut performance
//! - Full cognitive engine throughput
//! - Synchrony computation efficiency

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ruvector_mincut::graph::DynamicGraph;
use ruvector_mincut::snn::{
    compute_synchrony, AttractorConfig, AttractorDynamics, CognitiveMinCutEngine, EngineConfig,
    LIFNeuron, LayerConfig, NetworkConfig, NeuronConfig, Spike, SpikeTrain, SpikingNetwork,
    SynapseMatrix,
};

/// Generate a random graph for benchmarking
fn create_test_graph(n: usize) -> DynamicGraph {
    let graph = DynamicGraph::new();

    // Add vertices
    for i in 0..n {
        graph.add_vertex(i as u64);
    }

    // Add random edges (sparse graph ~3 edges per vertex)
    let mut seed: u64 = 12345;
    for i in 0..n {
        for _ in 0..3 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = (seed % n as u64) as usize;
            if i != j {
                let weight = ((seed % 100) as f64) / 100.0 + 0.5;
                let _ = graph.insert_edge(i as u64, j as u64, weight);
            }
        }
    }

    graph
}

/// Benchmark LIF neuron step
fn bench_lif_neuron(c: &mut Criterion) {
    let mut group = c.benchmark_group("lif_neuron");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("step", size), size, |b, &size| {
            let config = NeuronConfig::default();
            let mut neurons: Vec<LIFNeuron> = (0..size)
                .map(|i| LIFNeuron::with_config(i, config.clone()))
                .collect();
            let currents: Vec<f64> = (0..size).map(|i| (i % 10) as f64 * 0.1).collect();

            b.iter(|| {
                let mut spikes = 0;
                for (neuron, &current) in neurons.iter_mut().zip(currents.iter()) {
                    if neuron.step(black_box(current), 1.0, 0.0) {
                        spikes += 1;
                    }
                }
                black_box(spikes)
            });
        });
    }

    group.finish();
}

/// Benchmark STDP weight updates
fn bench_stdp(c: &mut Criterion) {
    let mut group = c.benchmark_group("stdp");

    for size in [100, 500, 1000].iter() {
        let n_synapses = size * size / 10; // ~10% connectivity
        group.throughput(Throughput::Elements(n_synapses as u64));

        group.bench_with_input(BenchmarkId::new("update", size), size, |b, &size| {
            let mut matrix = SynapseMatrix::new(size, size);

            // Create sparse connections
            let mut seed: u64 = 42;
            for i in 0..size {
                for _ in 0..size / 10 {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let j = (seed as usize) % size;
                    matrix.add_synapse(i, j, 0.5);
                }
            }

            b.iter(|| {
                // Simulate spike events
                for i in 0..size / 10 {
                    matrix.on_pre_spike(i, black_box(i as f64));
                    matrix.on_post_spike(i + 1, black_box(i as f64 + 5.0));
                }
            });
        });
    }

    group.finish();
}

/// Benchmark spiking network propagation
fn bench_network(c: &mut Criterion) {
    let mut group = c.benchmark_group("spiking_network");

    for &(input, hidden, output) in [(100, 50, 10), (500, 200, 50), (1000, 500, 100)].iter() {
        let total = input + hidden + output;
        group.throughput(Throughput::Elements(total as u64));

        let name = format!("{}-{}-{}", input, hidden, output);
        group.bench_with_input(
            BenchmarkId::new("step", &name),
            &(input, hidden, output),
            |b, &(i, h, o)| {
                let config = NetworkConfig {
                    layers: vec![
                        LayerConfig::new(i),
                        LayerConfig::new(h),
                        LayerConfig::new(o),
                    ],
                    ..NetworkConfig::default()
                };
                let mut network = SpikingNetwork::new(config);

                b.iter(|| black_box(network.step()));
            },
        );
    }

    group.finish();
}

/// Benchmark attractor dynamics (includes Karger-Stein)
fn bench_attractor(c: &mut Criterion) {
    let mut group = c.benchmark_group("attractor_dynamics");
    group.sample_size(50); // Fewer samples for slower benchmarks

    for size in [50, 100, 200].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("step", size), size, |b, &size| {
            let graph = create_test_graph(size);
            let config = AttractorConfig::default();
            let mut attractor = AttractorDynamics::new(graph, config);

            b.iter(|| black_box(attractor.step()));
        });
    }

    group.finish();
}

/// Benchmark synchrony computation
fn bench_synchrony(c: &mut Criterion) {
    let mut group = c.benchmark_group("synchrony");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("compute", size), size, |b, &size| {
            // Generate random spikes
            let mut seed: u64 = 999;
            let spikes: Vec<Spike> = (0..size)
                .map(|i| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    Spike {
                        neuron_id: (seed as usize) % 100,
                        time: (i as f64) + ((seed % 100) as f64) / 100.0,
                    }
                })
                .collect();

            b.iter(|| black_box(compute_synchrony(&spikes, 10.0)));
        });
    }

    group.finish();
}

/// Benchmark full cognitive engine
fn bench_cognitive_engine(c: &mut Criterion) {
    let mut group = c.benchmark_group("cognitive_engine");
    group.sample_size(30); // Fewer samples for complex benchmark

    for size in [50, 100].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("step", size), size, |b, &size| {
            let graph = create_test_graph(size);
            let config = EngineConfig::default();
            let mut engine = CognitiveMinCutEngine::new(graph, config);

            b.iter(|| black_box(engine.step()));
        });

        group.bench_with_input(BenchmarkId::new("run_10", size), size, |b, &size| {
            let graph = create_test_graph(size);
            let config = EngineConfig::default();
            let mut engine = CognitiveMinCutEngine::new(graph, config);

            b.iter(|| black_box(engine.run(10)));
        });
    }

    group.finish();
}

/// Benchmark spike train operations
fn bench_spike_train(c: &mut Criterion) {
    let mut group = c.benchmark_group("spike_train");

    for size in [100, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("to_pattern", size), size, |b, &size| {
            let mut train = SpikeTrain::new(0);
            for i in 0..size {
                train.record_spike(i as f64 * 0.5);
            }

            b.iter(|| black_box(train.to_pattern(0.0, 1.0, 100)));
        });

        group.bench_with_input(
            BenchmarkId::new("cross_correlation", size),
            size,
            |b, &size| {
                let mut train1 = SpikeTrain::new(0);
                let mut train2 = SpikeTrain::new(1);
                for i in 0..size {
                    train1.record_spike(i as f64 * 0.5);
                    train2.record_spike(i as f64 * 0.5 + 2.0);
                }

                b.iter(|| black_box(train1.cross_correlation(&train2, 50.0, 1.0)));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_lif_neuron,
    bench_stdp,
    bench_network,
    bench_attractor,
    bench_synchrony,
    bench_cognitive_engine,
    bench_spike_train,
);

criterion_main!(benches);
