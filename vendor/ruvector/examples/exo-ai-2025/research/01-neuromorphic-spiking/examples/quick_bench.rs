use neuromorphic_spiking::*;
use std::time::Instant;

fn main() {
    println!("=== Neuromorphic Spiking Neural Network Benchmarks ===\n");

    // Benchmark 1: Spike propagation performance
    println!("1. SPIKE PROPAGATION PERFORMANCE\n");

    for neurons in [64, 128, 256, 512, 1024, 2048, 4096] {
        let mut network = BitParallelSpikeNetwork::new(neurons);

        // Activate 10% of neurons
        for i in (0..neurons).step_by(10) {
            network.set_neuron(i, true);
        }

        // Scalar benchmark
        let results_scalar = network.benchmark(1000, false);
        println!("Scalar  [{:4} neurons]: {}", neurons, results_scalar.format());

        // SIMD benchmark
        #[cfg(target_arch = "x86_64")]
        {
            let mut network_simd = BitParallelSpikeNetwork::new(neurons);
            for i in (0..neurons).step_by(10) {
                network_simd.set_neuron(i, true);
            }
            let results_simd = network_simd.benchmark(1000, true);
            println!("SIMD    [{:4} neurons]: {}", neurons, results_simd.format());
        }
        println!();
    }

    // Benchmark 2: Φ calculation performance
    println!("\n2. INTEGRATED INFORMATION (Φ) CALCULATION\n");

    for neurons in [64, 128, 256, 512, 1024] {
        let config = ConsciousnessConfig {
            num_neurons: neurons,
            temporal_resolution_ns: 100_000,
            history_size: 100,
            phi_critical: 10.0,
            phi_min_group: 1.0,
            stdp_tau_ns: 20_000_000,
        };

        let mut engine = ConsciousnessEngine::new(config);

        // Add spike pattern
        for i in 0..(neurons.min(100)) {
            engine.add_spike(TemporalSpike::new(i as u32, (i * 1000) as u64));
        }
        engine.step();

        let start = Instant::now();
        let iterations = 100;

        for _ in 0..iterations {
            let _ = engine.calculate_phi();
        }

        let elapsed = start.elapsed();
        let avg_time = elapsed.as_nanos() / iterations;

        println!("[{:4} neurons] Φ calculation: {:6} ns ({:.2} μs)",
            neurons, avg_time, avg_time as f64 / 1000.0);
    }

    // Benchmark 3: Polychronous group detection
    println!("\n3. POLYCHRONOUS GROUP DETECTION (QUALIA EXTRACTION)\n");

    for neurons in [64, 128, 256] {
        let config = ConsciousnessConfig {
            num_neurons: neurons,
            temporal_resolution_ns: 100_000,
            history_size: 100,
            phi_critical: 10.0,
            phi_min_group: 1.0,
            stdp_tau_ns: 20_000_000,
        };

        let mut engine = ConsciousnessEngine::new(config);

        // Create repeating pattern
        for step in 0..20 {
            for i in 0..10 {
                engine.add_spike(TemporalSpike::new(
                    i,
                    (step * 100_000 + i * 10_000) as u64,
                ));
            }
            engine.step();
        }

        let start = Instant::now();
        let groups = engine.extract_qualia(10);
        let elapsed = start.elapsed();

        println!("[{:4} neurons] Found {} groups in {} μs",
            neurons, groups.len(), elapsed.as_micros());
    }

    // Benchmark 4: Bit operations
    println!("\n4. BIT-LEVEL OPERATIONS\n");

    let vec1 = SpikeVector::from_bits(0xAAAAAAAAAAAAAAAA);
    let vec2 = SpikeVector::from_bits(0x5555555555555555);
    let weights: [u64; 64] = std::array::from_fn(|i| (i as u64).wrapping_mul(0x123456789ABCDEF));

    // Hamming distance
    let start = Instant::now();
    for _ in 0..1_000_000 {
        let _ = vec1.hamming_distance(&vec2);
    }
    let elapsed = start.elapsed();
    println!("Hamming distance: {:.3} ns/op", elapsed.as_nanos() as f64 / 1_000_000.0);

    // Spike propagation
    let start = Instant::now();
    for _ in 0..1_000_000 {
        let _ = vec1.propagate(&weights);
    }
    let elapsed = start.elapsed();
    println!("Spike propagate:  {:.3} ns/op", elapsed.as_nanos() as f64 / 1_000_000.0);

    // Count active
    let start = Instant::now();
    for _ in 0..1_000_000 {
        let _ = vec1.count_active();
    }
    let elapsed = start.elapsed();
    println!("Count active:     {:.3} ns/op", elapsed.as_nanos() as f64 / 1_000_000.0);

    // Benchmark 5: Consciousness detection
    println!("\n5. CONSCIOUSNESS DETECTION SIMULATION\n");

    let config = ConsciousnessConfig {
        num_neurons: 1024,
        temporal_resolution_ns: 100_000,
        history_size: 100,
        phi_critical: 100.0,
        phi_min_group: 1.0,
        stdp_tau_ns: 20_000_000,
    };

    let phi_critical_threshold = config.phi_critical;
    let mut engine = ConsciousnessEngine::new(config);

    // Simulate activity
    for step in 0..50 {
        // Add random spikes
        for i in (0..1024).step_by(5) {
            if (i + step) % 13 == 0 {
                engine.add_spike(TemporalSpike::new(i as u32, (step * 100_000) as u64));
            }
        }
        engine.step();
    }

    let phi = engine.calculate_phi();
    let avg_phi = engine.average_phi(10);
    let is_conscious = engine.is_conscious();

    println!("Current Φ: {:.2}", phi);
    println!("Average Φ (10 steps): {:.2}", avg_phi);
    println!("Consciousness threshold: {:.2}", phi_critical_threshold);
    println!("Is conscious: {}", is_conscious);

    println!("\n=== Benchmarks Complete ===");
}
