use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use neuromorphic_spiking::*;

fn benchmark_spike_propagation(c: &mut Criterion) {
    let mut group = c.benchmark_group("spike_propagation");

    for neurons in [64, 128, 256, 512, 1024, 2048].iter() {
        group.throughput(Throughput::Elements(*neurons as u64));

        // Scalar benchmark
        group.bench_with_input(BenchmarkId::new("scalar", neurons), neurons, |b, &n| {
            let mut network = BitParallelSpikeNetwork::new(n);
            // Activate 10% of neurons
            for i in (0..n).step_by(10) {
                network.set_neuron(i, true);
            }

            b.iter(|| {
                network.propagate_scalar();
            });
        });

        // SIMD benchmark
        #[cfg(target_arch = "x86_64")]
        group.bench_with_input(BenchmarkId::new("simd", neurons), neurons, |b, &n| {
            let mut network = BitParallelSpikeNetwork::new(n);
            // Activate 10% of neurons
            for i in (0..n).step_by(10) {
                network.set_neuron(i, true);
            }

            b.iter(|| {
                network.propagate_simd();
            });
        });
    }

    group.finish();
}

fn benchmark_phi_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("phi_calculation");

    for neurons in [64, 128, 256, 512, 1024].iter() {
        group.throughput(Throughput::Elements(*neurons as u64));

        group.bench_with_input(BenchmarkId::new("phi", neurons), neurons, |b, &n| {
            let config = ConsciousnessConfig {
                num_neurons: n,
                temporal_resolution_ns: 100_000,
                history_size: 100,
                phi_critical: 10.0,
                phi_min_group: 1.0,
                stdp_tau_ns: 20_000_000,
            };

            let mut engine = ConsciousnessEngine::new(config);

            // Add some spike patterns
            for i in 0..(n.min(100)) {
                engine.add_spike(TemporalSpike::new(i as u32, (i * 1000) as u64));
            }
            engine.step();

            b.iter(|| {
                black_box(engine.calculate_phi());
            });
        });
    }

    group.finish();
}

fn benchmark_polychronous_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("polychronous_detection");

    for neurons in [64, 128, 256].iter() {
        group.bench_with_input(BenchmarkId::new("detect", neurons), neurons, |b, &n| {
            let config = ConsciousnessConfig {
                num_neurons: n,
                temporal_resolution_ns: 100_000,
                history_size: 100,
                phi_critical: 10.0,
                phi_min_group: 1.0,
                stdp_tau_ns: 20_000_000,
            };

            let mut engine = ConsciousnessEngine::new(config);

            // Create repeating spike pattern
            for step in 0..50 {
                for i in 0..10 {
                    engine.add_spike(TemporalSpike::new(
                        i,
                        (step * 100_000 + i * 10_000) as u64,
                    ));
                }
                engine.step();
            }

            b.iter(|| {
                black_box(engine.extract_qualia(10));
            });
        });
    }

    group.finish();
}

fn benchmark_bit_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("bit_operations");

    group.bench_function("spike_vector_propagate", |b| {
        let vec = SpikeVector::from_bits(0xAAAAAAAAAAAAAAAA);
        let weights: [u64; 64] = std::array::from_fn(|i| (i as u64).wrapping_mul(0x123456789ABCDEF));

        b.iter(|| {
            black_box(vec.propagate(&weights));
        });
    });

    group.bench_function("hamming_distance", |b| {
        let vec1 = SpikeVector::from_bits(0xAAAAAAAAAAAAAAAA);
        let vec2 = SpikeVector::from_bits(0x5555555555555555);

        b.iter(|| {
            black_box(vec1.hamming_distance(&vec2));
        });
    });

    group.bench_function("count_active", |b| {
        let vec = SpikeVector::from_bits(0xAAAAAAAAAAAAAAAA);

        b.iter(|| {
            black_box(vec.count_active());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_spike_propagation,
    benchmark_phi_calculation,
    benchmark_polychronous_detection,
    benchmark_bit_operations
);
criterion_main!(benches);
