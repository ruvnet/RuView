//! Neuromorphic Component Benchmarks
//!
//! Benchmarks for bio-inspired neural components:
//! - HDC (Hyperdimensional Computing)
//! - BTSP (Behavioral Time-Scale Plasticity)
//! - Spiking Neural Networks
//!
//! Run with: cargo bench --bench neuromorphic_benchmarks

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};

/// Generate random f32 vector
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    (0..dim)
        .map(|i| {
            let x = ((seed.wrapping_mul(i as u64 + 1).wrapping_mul(0x5DEECE66D)) % 1000) as f32;
            (x / 500.0) - 1.0
        })
        .collect()
}

/// Generate binary hypervector
fn random_binary_hv(dim: usize, seed: u64) -> Vec<i8> {
    (0..dim)
        .map(|i| {
            if ((seed.wrapping_mul(i as u64 + 1).wrapping_mul(0x5DEECE66D)) % 2) == 0 {
                1
            } else {
                -1
            }
        })
        .collect()
}

fn bench_hdc_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("hdc");

    // HDC typically uses high dimensions for orthogonality
    for dim in [1000, 4000, 10000].iter() {
        let hv_a = random_binary_hv(*dim, 42);
        let hv_b = random_binary_hv(*dim, 123);
        let hv_c = random_binary_hv(*dim, 456);

        group.throughput(Throughput::Elements(*dim as u64));

        // Bundling (element-wise majority/addition)
        group.bench_with_input(
            BenchmarkId::new("bundle_3", dim),
            &(&hv_a, &hv_b, &hv_c),
            |b, (a, b_hv, c)| {
                b.iter(|| {
                    let bundled: Vec<i8> = (0..a.len())
                        .map(|i| {
                            let sum = a[i] as i32 + b_hv[i] as i32 + c[i] as i32;
                            if sum > 0 { 1 } else { -1 }
                        })
                        .collect();
                    bundled
                });
            },
        );

        // Binding (element-wise XOR / multiplication)
        group.bench_with_input(
            BenchmarkId::new("bind", dim),
            &(&hv_a, &hv_b),
            |b, (a, b_hv)| {
                b.iter(|| {
                    let bound: Vec<i8> = a.iter()
                        .zip(b_hv.iter())
                        .map(|(&ai, &bi)| ai * bi)
                        .collect();
                    bound
                });
            },
        );

        // Permutation (cyclic shift)
        group.bench_with_input(
            BenchmarkId::new("permute", dim),
            &(&hv_a,),
            |b, (a,)| {
                b.iter(|| {
                    let shift = 7;
                    let mut permuted = vec![0i8; a.len()];
                    for i in 0..a.len() {
                        permuted[(i + shift) % a.len()] = a[i];
                    }
                    permuted
                });
            },
        );

        // Similarity (Hamming distance / cosine)
        group.bench_with_input(
            BenchmarkId::new("similarity", dim),
            &(&hv_a, &hv_b),
            |b, (a, b_hv)| {
                b.iter(|| {
                    let matching: i32 = a.iter()
                        .zip(b_hv.iter())
                        .map(|(&ai, &bi)| (ai * bi) as i32)
                        .sum();
                    matching as f32 / a.len() as f32
                });
            },
        );
    }

    group.finish();
}

fn bench_hdc_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("hdc_encoding");

    let hd_dim = 10000;
    let input_dim = 64;

    let input = random_vector(input_dim, 42);

    // Level hypervectors for encoding continuous values
    let num_levels = 100;
    let level_hvs: Vec<Vec<i8>> = (0..num_levels)
        .map(|i| random_binary_hv(hd_dim, i as u64))
        .collect();

    // Position hypervectors for encoding positions
    let pos_hvs: Vec<Vec<i8>> = (0..input_dim)
        .map(|i| random_binary_hv(hd_dim, (i + 1000) as u64))
        .collect();

    group.bench_function("encode_vector", |b| {
        b.iter(|| {
            // Encode each dimension and bundle
            let mut result = vec![0i32; hd_dim];

            for (i, &val) in input.iter().enumerate() {
                // Quantize to level
                let level = ((val + 1.0) / 2.0 * (num_levels - 1) as f32) as usize;
                let level = level.min(num_levels - 1);

                // Bind position with level
                for j in 0..hd_dim {
                    result[j] += (pos_hvs[i][j] * level_hvs[level][j]) as i32;
                }
            }

            // Threshold to binary
            let encoded: Vec<i8> = result.iter()
                .map(|&v| if v > 0 { 1 } else { -1 })
                .collect();
            encoded
        });
    });

    group.finish();
}

fn bench_btsp(c: &mut Criterion) {
    let mut group = c.benchmark_group("btsp");

    let num_inputs = 100;
    let num_outputs = 10;

    let input = random_vector(num_inputs, 42);
    let weights: Vec<Vec<f32>> = (0..num_outputs)
        .map(|i| random_vector(num_inputs, i as u64))
        .collect();

    group.bench_function("forward", |b| {
        b.iter(|| {
            // Forward pass with dendritic compartments
            let mut outputs = vec![0.0f32; num_outputs];
            for i in 0..num_outputs {
                for j in 0..num_inputs {
                    outputs[i] += weights[i][j] * input[j];
                }
                // Dendritic nonlinearity
                outputs[i] = outputs[i].tanh();
            }
            outputs
        });
    });

    group.bench_function("eligibility_update", |b| {
        let tau_e = 100.0; // Eligibility trace time constant
        let mut eligibility = vec![vec![0.0f32; num_inputs]; num_outputs];

        b.iter(|| {
            // Update eligibility traces
            for i in 0..num_outputs {
                for j in 0..num_inputs {
                    eligibility[i][j] *= (-1.0 / tau_e).exp();
                    eligibility[i][j] += input[j];
                }
            }
            &eligibility
        });
    });

    group.bench_function("behavioral_update", |b| {
        let eligibility = vec![vec![0.5f32; num_inputs]; num_outputs];
        let mut weights = weights.clone();
        let learning_rate = 0.01;

        b.iter(|| {
            // Apply behavioral signal to modulate learning
            let behavioral_signal = 1.0; // Reward/plateau potential

            for i in 0..num_outputs {
                for j in 0..num_inputs {
                    weights[i][j] += learning_rate * behavioral_signal * eligibility[i][j];
                }
            }
            &weights
        });
    });

    group.finish();
}

fn bench_spiking_neurons(c: &mut Criterion) {
    let mut group = c.benchmark_group("spiking");

    // LIF neuron simulation
    group.bench_function("lif_neuron_1000steps", |b| {
        let threshold = 1.0;
        let tau_m = 10.0;
        let dt = 1.0;
        let input_current = 0.15;

        b.iter(|| {
            let mut voltage = 0.0f32;
            let mut spike_count = 0u32;

            for _ in 0..1000 {
                // Leaky integration
                voltage += (-voltage / tau_m + input_current) * dt;

                // Spike and reset
                if voltage >= threshold {
                    spike_count += 1;
                    voltage = 0.0;
                }
            }

            spike_count
        });
    });

    // Spiking network simulation
    for num_neurons in [100, 500, 1000].iter() {
        let connectivity = 0.1; // 10% connectivity
        let num_connections = (*num_neurons as f32 * *num_neurons as f32 * connectivity) as usize;

        // Pre-generate connections
        let connections: Vec<(usize, usize, f32)> = (0..num_connections)
            .map(|i| {
                let pre = i % *num_neurons;
                let post = (i * 7 + 3) % *num_neurons;
                let weight = 0.1;
                (pre, post, weight)
            })
            .collect();

        group.throughput(Throughput::Elements(*num_neurons as u64));

        group.bench_with_input(
            BenchmarkId::new("network_step", num_neurons),
            &(&connections, num_neurons),
            |b, (conns, n)| {
                b.iter(|| {
                    let mut voltages = vec![0.0f32; **n];
                    let mut spikes = vec![false; **n];
                    let threshold = 1.0;
                    let tau_m = 10.0;

                    // Input current
                    let input: Vec<f32> = (0..**n).map(|i| 0.1 + 0.01 * (i as f32)).collect();

                    // Integrate
                    for i in 0..**n {
                        voltages[i] += (-voltages[i] / tau_m + input[i]);
                    }

                    // Propagate spikes from previous step
                    for (pre, post, weight) in conns.iter() {
                        if spikes[*pre] {
                            voltages[*post] += weight;
                        }
                    }

                    // Generate spikes
                    for i in 0..**n {
                        spikes[i] = voltages[i] >= threshold;
                        if spikes[i] {
                            voltages[i] = 0.0;
                        }
                    }

                    (voltages, spikes)
                });
            },
        );
    }

    group.finish();
}

fn bench_stdp(c: &mut Criterion) {
    let mut group = c.benchmark_group("stdp");

    let num_synapses = 10000;
    let a_plus = 0.01;
    let a_minus = 0.012;
    let tau_plus = 20.0;
    let tau_minus = 20.0;

    // Spike times
    let pre_spike_times: Vec<f32> = (0..num_synapses)
        .map(|i| (i % 100) as f32)
        .collect();
    let post_spike_times: Vec<f32> = (0..num_synapses)
        .map(|i| ((i + 10) % 100) as f32)
        .collect();

    let mut weights = vec![0.5f32; num_synapses];

    group.throughput(Throughput::Elements(num_synapses as u64));

    group.bench_function("weight_update", |b| {
        b.iter(|| {
            for i in 0..num_synapses {
                let dt = post_spike_times[i] - pre_spike_times[i];

                let delta_w = if dt > 0.0 {
                    // Potentiation
                    a_plus * (-dt / tau_plus).exp()
                } else {
                    // Depression
                    -a_minus * (dt / tau_minus).exp()
                };

                weights[i] = (weights[i] + delta_w).max(0.0).min(1.0);
            }
            weights.clone()
        });
    });

    group.finish();
}

fn bench_reservoir_computing(c: &mut Criterion) {
    let mut group = c.benchmark_group("reservoir");

    let input_dim = 10;
    let reservoir_size = 500;
    let output_dim = 5;
    let seq_len = 100;

    // Generate reservoir weights (sparse)
    let sparsity = 0.1;
    let num_connections = (reservoir_size as f32 * reservoir_size as f32 * sparsity) as usize;
    let reservoir_weights: Vec<(usize, usize, f32)> = (0..num_connections)
        .map(|i| {
            let pre = i % reservoir_size;
            let post = (i * 17 + 5) % reservoir_size;
            let weight = 0.1 * (((i * 7) % 100) as f32 / 50.0 - 1.0);
            (pre, post, weight)
        })
        .collect();

    // Input weights
    let input_weights: Vec<Vec<f32>> = (0..reservoir_size)
        .map(|i| random_vector(input_dim, i as u64))
        .collect();

    // Input sequence
    let input_sequence: Vec<Vec<f32>> = (0..seq_len)
        .map(|i| random_vector(input_dim, i as u64))
        .collect();

    group.throughput(Throughput::Elements(seq_len as u64));

    group.bench_function("run_sequence", |b| {
        b.iter(|| {
            let mut state = vec![0.0f32; reservoir_size];
            let mut states = Vec::with_capacity(seq_len);

            for input in &input_sequence {
                // Input contribution
                let mut new_state = vec![0.0f32; reservoir_size];
                for i in 0..reservoir_size {
                    for j in 0..input_dim {
                        new_state[i] += input_weights[i][j] * input[j];
                    }
                }

                // Recurrent contribution
                for (pre, post, weight) in &reservoir_weights {
                    new_state[*post] += weight * state[*pre];
                }

                // Nonlinearity
                for s in &mut new_state {
                    *s = s.tanh();
                }

                state = new_state;
                states.push(state.clone());
            }

            states
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_hdc_operations,
    bench_hdc_encoding,
    bench_btsp,
    bench_spiking_neurons,
    bench_stdp,
    bench_reservoir_computing
);

criterion_main!(benches);
