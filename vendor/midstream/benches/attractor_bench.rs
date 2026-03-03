//! Comprehensive benchmarks for temporal-attractor-studio crate
//!
//! Benchmarks cover:
//! - Phase space embedding (target: <20ms)
//! - Lyapunov exponent calculation (target: <500ms)
//! - Attractor detection (target: <100ms)
//! - Trajectory analysis
//! - Dimension estimation
//! - Chaos detection
//!
//! Performance targets:
//! - Phase space: <20ms for n=1000
//! - Lyapunov: <500ms
//! - Detection: <100ms

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use midstreamer_attractor::{
    AttractorStudio, PhaseSpace, Trajectory, AttractorType,
    lyapunov::calculate_lyapunov_exponent,
    embedding::reconstruct_phase_space,
    detection::detect_attractor_type,
    dimension::estimate_correlation_dimension,
};

// ============================================================================
// Test Signal Generators
// ============================================================================

fn generate_lorenz_attractor(n: usize) -> Vec<(f64, f64, f64)> {
    let dt = 0.01;
    let sigma = 10.0;
    let rho = 28.0;
    let beta = 8.0 / 3.0;

    let mut points = Vec::with_capacity(n);
    let (mut x, mut y, mut z) = (1.0, 1.0, 1.0);

    for _ in 0..n {
        let dx = sigma * (y - x);
        let dy = x * (rho - z) - y;
        let dz = x * y - beta * z;

        x += dx * dt;
        y += dy * dt;
        z += dz * dt;

        points.push((x, y, z));
    }

    points
}

fn generate_rossler_attractor(n: usize) -> Vec<(f64, f64, f64)> {
    let dt = 0.01;
    let a = 0.2;
    let b = 0.2;
    let c = 5.7;

    let mut points = Vec::with_capacity(n);
    let (mut x, mut y, mut z) = (1.0, 1.0, 1.0);

    for _ in 0..n {
        let dx = -y - z;
        let dy = x + a * y;
        let dz = b + z * (x - c);

        x += dx * dt;
        y += dy * dt;
        z += dz * dt;

        points.push((x, y, z));
    }

    points
}

fn generate_henon_map(n: usize) -> Vec<(f64, f64)> {
    let a = 1.4;
    let b = 0.3;

    let mut points = Vec::with_capacity(n);
    let (mut x, mut y) = (0.1, 0.1);

    for _ in 0..n {
        let x_new = 1.0 - a * x * x + y;
        let y_new = b * x;

        points.push((x, y));
        x = x_new;
        y = y_new;
    }

    points
}

fn generate_time_series(n: usize, pattern: &str) -> Vec<f64> {
    match pattern {
        "sine" => (0..n).map(|i| (i as f64 * 0.1).sin()).collect(),
        "chaotic" => {
            let lorenz = generate_lorenz_attractor(n);
            lorenz.iter().map(|(x, _, _)| *x).collect()
        }
        "random" => (0..n).map(|i| {
            ((i as f64 * 7919.0).sin() * 10000.0) % 100.0
        }).collect(),
        "periodic" => (0..n).map(|i| {
            (i as f64 * 0.1).sin() + 0.5 * (i as f64 * 0.3).sin()
        }).collect(),
        _ => vec![0.0; n],
    }
}

// ============================================================================
// Phase Space Embedding Benchmarks
// ============================================================================

fn bench_phase_space_embedding(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase_space_embedding");

    for size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // Dimension 2
        group.bench_with_input(
            BenchmarkId::new("dim2", size),
            size,
            |b, &n| {
                let data = generate_time_series(n, "chaotic");
                b.iter(|| {
                    black_box(reconstruct_phase_space(
                        black_box(&data),
                        black_box(2),
                        black_box(1)
                    ))
                });
            }
        );

        // Dimension 3
        group.bench_with_input(
            BenchmarkId::new("dim3", size),
            size,
            |b, &n| {
                let data = generate_time_series(n, "chaotic");
                b.iter(|| {
                    black_box(reconstruct_phase_space(
                        black_box(&data),
                        black_box(3),
                        black_box(1)
                    ))
                });
            }
        );

        // Dimension 5
        group.bench_with_input(
            BenchmarkId::new("dim5", size),
            size,
            |b, &n| {
                let data = generate_time_series(n, "chaotic");
                b.iter(|| {
                    black_box(reconstruct_phase_space(
                        black_box(&data),
                        black_box(5),
                        black_box(1)
                    ))
                });
            }
        );
    }

    group.finish();
}

fn bench_embedding_delays(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedding_delays");

    let data = generate_time_series(1000, "chaotic");

    for delay in [1, 5, 10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::new("delay", delay),
            delay,
            |b, &d| {
                b.iter(|| {
                    black_box(reconstruct_phase_space(
                        black_box(&data),
                        black_box(3),
                        black_box(d)
                    ))
                });
            }
        );
    }

    group.finish();
}

// ============================================================================
// Lyapunov Exponent Benchmarks
// ============================================================================

fn bench_lyapunov_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("lyapunov_exponent");

    // Different attractor types
    group.bench_function("lorenz", |b| {
        let points = generate_lorenz_attractor(1000);
        let trajectory: Vec<f64> = points.iter().map(|(x, _, _)| *x).collect();

        b.iter(|| {
            black_box(calculate_lyapunov_exponent(
                black_box(&trajectory),
                black_box(3),
                black_box(10)
            ))
        });
    });

    group.bench_function("rossler", |b| {
        let points = generate_rossler_attractor(1000);
        let trajectory: Vec<f64> = points.iter().map(|(x, _, _)| *x).collect();

        b.iter(|| {
            black_box(calculate_lyapunov_exponent(
                black_box(&trajectory),
                black_box(3),
                black_box(10)
            ))
        });
    });

    group.bench_function("periodic", |b| {
        let trajectory = generate_time_series(1000, "periodic");

        b.iter(|| {
            black_box(calculate_lyapunov_exponent(
                black_box(&trajectory),
                black_box(3),
                black_box(10)
            ))
        });
    });

    // Varying data sizes
    for size in [500, 1000, 2000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::new("size", size),
            size,
            |b, &n| {
                let trajectory = generate_time_series(n, "chaotic");
                b.iter(|| {
                    black_box(calculate_lyapunov_exponent(
                        black_box(&trajectory),
                        black_box(3),
                        black_box(10)
                    ))
                });
            }
        );
    }

    group.finish();
}

// ============================================================================
// Attractor Detection Benchmarks
// ============================================================================

fn bench_attractor_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("attractor_detection");

    // Known attractors
    group.bench_function("lorenz_detection", |b| {
        let points = generate_lorenz_attractor(1000);

        b.iter(|| {
            black_box(detect_attractor_type(black_box(&points)))
        });
    });

    group.bench_function("rossler_detection", |b| {
        let points = generate_rossler_attractor(1000);

        b.iter(|| {
            black_box(detect_attractor_type(black_box(&points)))
        });
    });

    // Different data sizes
    for size in [100, 500, 1000, 2000].iter() {
        group.bench_with_input(
            BenchmarkId::new("size", size),
            size,
            |b, &n| {
                let points = generate_lorenz_attractor(n);
                b.iter(|| {
                    black_box(detect_attractor_type(black_box(&points)))
                });
            }
        );
    }

    group.finish();
}

// ============================================================================
// Trajectory Analysis Benchmarks
// ============================================================================

fn bench_trajectory_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("trajectory_analysis");

    // Trajectory reconstruction
    group.bench_function("reconstruction", |b| {
        let data = generate_time_series(1000, "chaotic");

        b.iter(|| {
            let phase_space = reconstruct_phase_space(&data, 3, 10);
            black_box(Trajectory::from_phase_space(black_box(phase_space)))
        });
    });

    // Distance calculations
    group.bench_function("distances", |b| {
        let points = generate_lorenz_attractor(1000);
        let trajectory = Trajectory::from_points(points);

        b.iter(|| {
            black_box(trajectory.calculate_distances())
        });
    });

    // Nearest neighbors
    group.bench_function("nearest_neighbors", |b| {
        let points = generate_lorenz_attractor(1000);
        let trajectory = Trajectory::from_points(points);

        b.iter(|| {
            black_box(trajectory.find_nearest_neighbors(black_box(10)))
        });
    });

    group.finish();
}

// ============================================================================
// Dimension Estimation Benchmarks
// ============================================================================

fn bench_dimension_estimation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dimension_estimation");

    // Correlation dimension
    group.bench_function("correlation_dim_lorenz", |b| {
        let points = generate_lorenz_attractor(1000);

        b.iter(|| {
            black_box(estimate_correlation_dimension(
                black_box(&points),
                black_box(20)
            ))
        });
    });

    group.bench_function("correlation_dim_henon", |b| {
        let points = generate_henon_map(1000);
        let points_3d: Vec<(f64, f64, f64)> = points
            .iter()
            .map(|(x, y)| (*x, *y, 0.0))
            .collect();

        b.iter(|| {
            black_box(estimate_correlation_dimension(
                black_box(&points_3d),
                black_box(20)
            ))
        });
    });

    // Varying sample sizes
    for size in [500, 1000, 2000].iter() {
        group.bench_with_input(
            BenchmarkId::new("size", size),
            size,
            |b, &n| {
                let points = generate_lorenz_attractor(n);
                b.iter(|| {
                    black_box(estimate_correlation_dimension(
                        black_box(&points),
                        black_box(20)
                    ))
                });
            }
        );
    }

    group.finish();
}

// ============================================================================
// Chaos Detection Benchmarks
// ============================================================================

fn bench_chaos_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("chaos_detection");

    // Chaotic signals
    group.bench_function("chaotic_lorenz", |b| {
        let points = generate_lorenz_attractor(1000);
        let signal: Vec<f64> = points.iter().map(|(x, _, _)| *x).collect();

        b.iter(|| {
            let lyapunov = calculate_lyapunov_exponent(&signal, 3, 10);
            black_box(lyapunov > 0.0)
        });
    });

    // Periodic signals
    group.bench_function("periodic", |b| {
        let signal = generate_time_series(1000, "periodic");

        b.iter(|| {
            let lyapunov = calculate_lyapunov_exponent(&signal, 3, 10);
            black_box(lyapunov > 0.0)
        });
    });

    // Random signals
    group.bench_function("random", |b| {
        let signal = generate_time_series(1000, "random");

        b.iter(|| {
            let lyapunov = calculate_lyapunov_exponent(&signal, 3, 10);
            black_box(lyapunov > 0.0)
        });
    });

    group.finish();
}

// ============================================================================
// Complete Pipeline Benchmarks
// ============================================================================

fn bench_complete_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("complete_analysis");

    group.bench_function("full_pipeline", |b| {
        let data = generate_time_series(1000, "chaotic");

        b.iter(|| {
            // Phase space reconstruction
            let phase_space = reconstruct_phase_space(&data, 3, 10);

            // Convert to 3D points for analysis
            let points: Vec<(f64, f64, f64)> = phase_space
                .iter()
                .map(|p| (p[0], p[1], p[2]))
                .collect();

            // Attractor detection
            let attractor_type = detect_attractor_type(&points);

            // Lyapunov calculation
            let lyapunov = calculate_lyapunov_exponent(&data, 3, 10);

            // Dimension estimation
            let dimension = estimate_correlation_dimension(&points, 20);

            black_box((attractor_type, lyapunov, dimension))
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = embedding_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(10))
        .warm_up_time(std::time::Duration::from_secs(3));
    targets = bench_phase_space_embedding, bench_embedding_delays
}

criterion_group! {
    name = lyapunov_benches;
    config = Criterion::default()
        .sample_size(50)
        .measurement_time(std::time::Duration::from_secs(15));
    targets = bench_lyapunov_calculation
}

criterion_group! {
    name = detection_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(10));
    targets = bench_attractor_detection
}

criterion_group! {
    name = trajectory_benches;
    config = Criterion::default()
        .sample_size(100);
    targets = bench_trajectory_analysis
}

criterion_group! {
    name = dimension_benches;
    config = Criterion::default()
        .sample_size(50)
        .measurement_time(std::time::Duration::from_secs(12));
    targets = bench_dimension_estimation
}

criterion_group! {
    name = chaos_benches;
    config = Criterion::default()
        .sample_size(50);
    targets = bench_chaos_detection
}

criterion_group! {
    name = pipeline_benches;
    config = Criterion::default()
        .sample_size(30)
        .measurement_time(std::time::Duration::from_secs(15));
    targets = bench_complete_analysis
}

criterion_main!(
    embedding_benches,
    lyapunov_benches,
    detection_benches,
    trajectory_benches,
    dimension_benches,
    chaos_benches,
    pipeline_benches
);
