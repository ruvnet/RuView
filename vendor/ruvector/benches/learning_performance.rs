//! Learning Mechanism Performance Benchmarks
//!
//! Benchmarks for MicroLoRA, SONA, and adaptive learning.
//! Focus on parameter efficiency and training speed.
//!
//! Run with: cargo bench --bench learning_performance

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

fn bench_micro_lora(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_lora");

    // Test different ranks and dimensions
    for (dim, rank) in [(64, 4), (128, 8), (256, 16), (512, 32)].iter() {
        let input = random_vector(*dim, 42);
        let gradients = random_vector(*dim, 123);

        group.bench_with_input(
            BenchmarkId::new("forward", format!("d{}_r{}", dim, rank)),
            &(&input, dim, rank),
            |b, (inp, d, r)| {
                // TODO: When MicroLoRA is implemented:
                // let lora = MicroLoRA::new(*d, *r);

                b.iter(|| {
                    // Placeholder: simulate LoRA forward pass
                    // output = input + B @ A @ input

                    // A: dim x rank projection
                    let mut projected = vec![0.0f32; **r];
                    for i in 0..**r {
                        for j in 0..**d {
                            projected[i] += inp[j] * 0.01;
                        }
                    }

                    // B: rank x dim projection
                    let mut output = vec![0.0f32; **d];
                    for i in 0..**d {
                        for j in 0..**r {
                            output[i] += projected[j] * 0.01;
                        }
                    }

                    // Add residual
                    for (o, i) in output.iter_mut().zip(inp.iter()) {
                        *o += i;
                    }

                    output
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("backward", format!("d{}_r{}", dim, rank)),
            &(&gradients, dim, rank),
            |b, (grad, d, r)| {
                b.iter(|| {
                    // Placeholder: simulate LoRA backward pass
                    // Compute gradients for A and B

                    let grad_a = vec![vec![0.01f32; **r]; **d];
                    let grad_b = vec![vec![0.01f32; **d]; **r];

                    (grad_a, grad_b)
                });
            },
        );
    }

    group.finish();
}

fn bench_sona_adaptation(c: &mut Criterion) {
    let mut group = c.benchmark_group("sona");

    let input_dim = 64;
    let hidden_dim = 128;
    let output_dim = 32;

    let input = random_vector(input_dim, 42);
    let target = random_vector(output_dim, 123);

    group.bench_function("forward", |b| {
        // TODO: When SONA is implemented:
        // let sona = SONA::new(input_dim, hidden_dim, output_dim);

        b.iter(|| {
            // Placeholder: simulate SONA forward
            let mut hidden = vec![0.0f32; hidden_dim];
            for i in 0..hidden_dim {
                for j in 0..input_dim {
                    hidden[i] += input[j] * 0.01;
                }
                hidden[i] = hidden[i].tanh();
            }

            let mut output = vec![0.0f32; output_dim];
            for i in 0..output_dim {
                for j in 0..hidden_dim {
                    output[i] += hidden[j] * 0.01;
                }
            }
            output
        });
    });

    group.bench_function("adapt_architecture", |b| {
        b.iter(|| {
            // Placeholder: simulate architecture adaptation
            // Analyze activation statistics and prune/grow neurons

            let neuron_activities = random_vector(hidden_dim, 456);
            let threshold = 0.1;

            let active_count: usize = neuron_activities.iter()
                .filter(|&a| a.abs() > threshold)
                .count();

            active_count
        });
    });

    group.bench_function("prune_neurons", |b| {
        b.iter(|| {
            // Placeholder: simulate neuron pruning
            let neuron_importance = random_vector(hidden_dim, 789);
            let prune_threshold = 0.05;

            let kept_indices: Vec<usize> = neuron_importance.iter()
                .enumerate()
                .filter(|(_, &imp)| imp.abs() > prune_threshold)
                .map(|(i, _)| i)
                .collect();

            kept_indices
        });
    });

    group.finish();
}

fn bench_online_learning(c: &mut Criterion) {
    let mut group = c.benchmark_group("online_learning");

    let dim = 64;
    let num_samples = 100;

    // Generate training samples
    let samples: Vec<(Vec<f32>, Vec<f32>)> = (0..num_samples)
        .map(|i| {
            let input = random_vector(dim, i as u64);
            let target = random_vector(dim, (i + 1000) as u64);
            (input, target)
        })
        .collect();

    group.throughput(Throughput::Elements(num_samples as u64));

    group.bench_function("single_sample_update", |b| {
        b.iter(|| {
            // Placeholder: simulate single-sample SGD update
            let (input, target) = &samples[0];
            let learning_rate = 0.01;

            // Forward
            let mut output = vec![0.0f32; dim];
            for i in 0..dim {
                for j in 0..dim {
                    output[i] += input[j] * 0.01;
                }
            }

            // Compute gradients
            let mut gradients = vec![0.0f32; dim];
            for i in 0..dim {
                gradients[i] = 2.0 * (output[i] - target[i]);
            }

            // Update (simulated)
            let weight_update: f32 = gradients.iter().map(|g| g * learning_rate).sum();

            weight_update
        });
    });

    group.bench_function("batch_update_100", |b| {
        b.iter(|| {
            // Placeholder: simulate batch update
            let mut total_gradient = vec![0.0f32; dim];

            for (input, target) in &samples {
                // Forward
                let mut output = vec![0.0f32; dim];
                for i in 0..dim {
                    for j in 0..dim {
                        output[i] += input[j] * 0.01;
                    }
                }

                // Accumulate gradients
                for i in 0..dim {
                    total_gradient[i] += 2.0 * (output[i] - target[i]) / (num_samples as f32);
                }
            }

            total_gradient
        });
    });

    group.finish();
}

fn bench_experience_replay(c: &mut Criterion) {
    let mut group = c.benchmark_group("experience_replay");

    let dim = 64;
    let buffer_size = 10000;
    let batch_size = 32;

    // Pre-fill buffer
    let buffer: Vec<(Vec<f32>, Vec<f32>)> = (0..buffer_size)
        .map(|i| {
            (random_vector(dim, i as u64), random_vector(dim, (i + 10000) as u64))
        })
        .collect();

    group.bench_function("uniform_sampling", |b| {
        b.iter(|| {
            // Uniform random sampling
            let mut batch = Vec::with_capacity(batch_size);
            for i in 0..batch_size {
                let idx = (i * 313 + 7) % buffer_size; // Pseudo-random
                batch.push(&buffer[idx]);
            }
            batch
        });
    });

    group.bench_function("prioritized_sampling", |b| {
        // Priorities (simulated)
        let priorities: Vec<f32> = (0..buffer_size)
            .map(|i| 1.0 + (i as f32 / buffer_size as f32))
            .collect();
        let total_priority: f32 = priorities.iter().sum();

        b.iter(|| {
            // Prioritized sampling (simplified)
            let mut batch = Vec::with_capacity(batch_size);
            let segment = total_priority / batch_size as f32;

            for i in 0..batch_size {
                let target = (i as f32 + 0.5) * segment;
                let mut cumsum = 0.0;
                for (idx, &p) in priorities.iter().enumerate() {
                    cumsum += p;
                    if cumsum >= target {
                        batch.push(&buffer[idx]);
                        break;
                    }
                }
            }
            batch
        });
    });

    group.finish();
}

fn bench_meta_learning(c: &mut Criterion) {
    let mut group = c.benchmark_group("meta_learning");

    let dim = 32;
    let num_tasks = 5;
    let shots_per_task = 5;

    // Generate task data
    let tasks: Vec<Vec<(Vec<f32>, Vec<f32>)>> = (0..num_tasks)
        .map(|t| {
            (0..shots_per_task)
                .map(|s| {
                    let input = random_vector(dim, (t * 100 + s) as u64);
                    let target = random_vector(dim, (t * 100 + s + 50) as u64);
                    (input, target)
                })
                .collect()
        })
        .collect();

    group.bench_function("inner_loop_adaptation", |b| {
        b.iter(|| {
            // MAML-style inner loop (simplified)
            let task = &tasks[0];
            let inner_lr = 0.01;
            let inner_steps = 5;

            let mut adapted_weights = vec![0.0f32; dim * dim];

            for _ in 0..inner_steps {
                let mut gradient = vec![0.0f32; dim * dim];

                for (input, target) in task {
                    // Forward
                    let mut output = vec![0.0f32; dim];
                    for i in 0..dim {
                        for j in 0..dim {
                            output[i] += input[j] * adapted_weights[i * dim + j];
                        }
                    }

                    // Backward
                    for i in 0..dim {
                        for j in 0..dim {
                            gradient[i * dim + j] += 2.0 * (output[i] - target[i]) * input[j];
                        }
                    }
                }

                // Update
                for (w, g) in adapted_weights.iter_mut().zip(gradient.iter()) {
                    *w -= inner_lr * g / (shots_per_task as f32);
                }
            }

            adapted_weights
        });
    });

    group.bench_function("outer_loop_update", |b| {
        b.iter(|| {
            // Meta-gradient computation (simplified)
            let outer_lr = 0.001;
            let mut meta_gradient = vec![0.0f32; dim * dim];

            for task in &tasks {
                // Simulate adapted performance gradient
                for (input, target) in task {
                    for i in 0..dim {
                        for j in 0..dim {
                            meta_gradient[i * dim + j] += input[j] * target[i] * 0.001;
                        }
                    }
                }
            }

            // Scale by learning rate
            for g in &mut meta_gradient {
                *g *= outer_lr / (num_tasks as f32);
            }

            meta_gradient
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_micro_lora,
    bench_sona_adaptation,
    bench_online_learning,
    bench_experience_replay,
    bench_meta_learning
);

criterion_main!(benches);
