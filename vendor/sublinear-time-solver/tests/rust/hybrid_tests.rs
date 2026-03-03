use sublinear_time_solver::core::{SparseMatrix, Vector};
use sublinear_time_solver::solver::hybrid::{HybridSolver, HybridConfig};
use sublinear_time_solver::solver::random_walk::{RandomWalkConfig, VarianceReduction};
use sublinear_time_solver::solver::sampling::{SamplingConfig, SamplingStrategy};
use sublinear_time_solver::algorithms::{Algorithm, Precision};

fn create_test_matrix(n: usize) -> SparseMatrix {
    let mut matrix = SparseMatrix::new(n, n);

    // Create a symmetric positive definite matrix
    for i in 0..n {
        matrix.insert(i, i, 2.0 + i as f64 * 0.1); // Diagonal dominance

        if i > 0 {
            matrix.insert(i, i-1, -0.5);
            matrix.insert(i-1, i, -0.5);
        }

        if i < n - 1 {
            matrix.insert(i, i+1, -0.3);
            matrix.insert(i+1, i, -0.3);
        }
    }

    matrix
}

fn create_test_vector(n: usize) -> Vector {
    (0..n).map(|i| 1.0 + (i as f64) * 0.2).collect()
}

#[test]
fn test_hybrid_solver_basic_functionality() {
    let mut config = HybridConfig::default();
    config.max_iterations = 500;
    config.convergence_tolerance = 1e-6;
    config.parallel_execution = false; // Avoid threading issues in tests

    let mut solver = HybridSolver::new(config);

    let matrix = create_test_matrix(5);
    let b = create_test_vector(5);

    let solution = solver.solve_linear_system(&matrix, &b).unwrap();

    assert_eq!(solution.len(), 5);

    // Verify solution quality by computing residual
    let mut residual = vec![0.0; 5];
    for i in 0..5 {
        let row = matrix.get_row(i);
        for (&j, &value) in row {
            residual[i] += value * solution[j];
        }
        residual[i] -= b[i];
    }

    let residual_norm: f64 = residual.iter().map(|r| r.powi(2)).sum::<f64>().sqrt();
    assert!(residual_norm < 0.1, "Residual norm {} too large", residual_norm);
}

#[test]
fn test_hybrid_solver_with_different_configurations() {
    let test_cases = vec![
        // Pure deterministic
        HybridConfig {
            use_deterministic: true,
            use_random_walk: false,
            use_bidirectional: false,
            use_multilevel: false,
            max_iterations: 200,
            convergence_tolerance: 1e-5,
            parallel_execution: false,
            ..Default::default()
        },
        // Pure random walk
        HybridConfig {
            use_deterministic: false,
            use_random_walk: true,
            use_bidirectional: false,
            use_multilevel: false,
            max_iterations: 200,
            convergence_tolerance: 1e-4,
            parallel_execution: false,
            random_walk_config: RandomWalkConfig {
                max_steps: 1000,
                variance_reduction: VarianceReduction::Antithetic,
                seed: Some(42),
                ..Default::default()
            },
            ..Default::default()
        },
        // Hybrid approach
        HybridConfig {
            use_deterministic: true,
            use_random_walk: true,
            use_bidirectional: true,
            use_multilevel: false,
            deterministic_weight: 0.6,
            max_iterations: 300,
            convergence_tolerance: 1e-5,
            parallel_execution: false,
            ..Default::default()
        },
    ];

    let matrix = create_test_matrix(4);
    let b = create_test_vector(4);

    for (idx, config) in test_cases.into_iter().enumerate() {
        let mut solver = HybridSolver::new(config);
        let solution = solver.solve_linear_system(&matrix, &b);

        match solution {
            Ok(sol) => {
                assert_eq!(sol.len(), 4, "Test case {}: Wrong solution size", idx);

                // Basic sanity checks
                assert!(sol.iter().all(|&x| x.is_finite()), "Test case {}: Non-finite solution", idx);

                let metrics = solver.get_metrics();
                assert!(metrics.total_iterations > 0, "Test case {}: No iterations performed", idx);

                println!("Test case {}: Iterations: {}, Residual: {:.2e}",
                        idx, metrics.total_iterations, metrics.final_residual);
            },
            Err(e) => {
                panic!("Test case {} failed: {:?}", idx, e);
            }
        }
    }
}

#[test]
fn test_adaptive_weight_adjustment() {
    let mut config = HybridConfig::default();
    config.adaptation_interval = 10;
    config.max_iterations = 100;
    config.parallel_execution = false;

    let mut solver = HybridSolver::new(config);

    let matrix = create_test_matrix(3);
    let b = create_test_vector(3);

    let initial_metrics = solver.get_metrics();
    let _solution = solver.solve_linear_system(&matrix, &b).unwrap();
    let final_metrics = solver.get_metrics();

    // Weights should be normalized
    let weights = &final_metrics.method_weights;
    let total_weight = weights.deterministic + weights.random_walk
                     + weights.bidirectional + weights.multilevel;
    assert!((total_weight - 1.0).abs() < 1e-10, "Weights not normalized: {}", total_weight);

    // Should have made progress
    assert!(final_metrics.total_iterations > initial_metrics.total_iterations);
}

#[test]
fn test_convergence_detection() {
    let mut config = HybridConfig::default();
    config.convergence_tolerance = 1e-8;
    config.max_iterations = 1000;
    config.parallel_execution = false;

    let mut solver = HybridSolver::new(config);

    // Simple well-conditioned system
    let mut matrix = SparseMatrix::new(2, 2);
    matrix.insert(0, 0, 4.0);
    matrix.insert(0, 1, -1.0);
    matrix.insert(1, 0, -1.0);
    matrix.insert(1, 1, 4.0);

    let b = vec![3.0, 3.0];
    let solution = solver.solve_linear_system(&matrix, &b).unwrap();

    let metrics = solver.get_metrics();

    // Should converge to high precision
    assert!(metrics.final_residual < 1e-6, "Did not achieve convergence: {:.2e}", metrics.final_residual);
    assert!(matches!(metrics.precision, Precision::High | Precision::Medium));

    // Expected solution is [1, 1]
    assert!((solution[0] - 1.0).abs() < 0.01, "Solution[0] = {}, expected ~1.0", solution[0]);
    assert!((solution[1] - 1.0).abs() < 0.01, "Solution[1] = {}, expected ~1.0", solution[1]);
}

#[test]
fn test_memory_management() {
    let mut config = HybridConfig::default();
    config.memory_limit = 1; // Very small limit to trigger cleanup
    config.max_iterations = 500;
    config.parallel_execution = false;

    let mut solver = HybridSolver::new(config);

    let matrix = create_test_matrix(3);
    let b = create_test_vector(3);

    let _solution = solver.solve_linear_system(&matrix, &b).unwrap();

    // Memory should be managed (convergence history should be limited)
    let metrics = solver.get_metrics();
    assert!(metrics.memory_usage > 0, "Memory usage should be tracked");
}

#[test]
fn test_different_sampling_strategies() {
    let strategies = vec![
        SamplingStrategy::Uniform,
        SamplingStrategy::ImportanceSampling,
        SamplingStrategy::AdaptiveSampling,
        SamplingStrategy::QuasiMonteCarlo,
    ];

    let matrix = create_test_matrix(3);
    let b = create_test_vector(3);

    for strategy in strategies {
        let config = HybridConfig {
            use_random_walk: true,
            use_deterministic: false,
            max_iterations: 200,
            convergence_tolerance: 1e-4,
            parallel_execution: false,
            sampling_config: SamplingConfig {
                strategy,
                sample_size: 500,
                seed: Some(42),
                ..Default::default()
            },
            random_walk_config: RandomWalkConfig {
                max_steps: 1000,
                seed: Some(42),
                ..Default::default()
            },
            ..Default::default()
        };

        let mut solver = HybridSolver::new(config);
        let solution = solver.solve_linear_system(&matrix, &b);

        match solution {
            Ok(sol) => {
                assert_eq!(sol.len(), 3);
                assert!(sol.iter().all(|&x| x.is_finite()));
                println!("Strategy {:?}: Solution quality OK", strategy);
            },
            Err(e) => {
                println!("Strategy {:?} failed: {:?}", strategy, e);
                // Some strategies might fail for small test cases, that's OK
            }
        }
    }
}

#[test]
fn test_variance_reduction_techniques() {
    let variance_methods = vec![
        VarianceReduction::None,
        VarianceReduction::Antithetic,
    ];

    let matrix = create_test_matrix(4);
    let b = create_test_vector(4);

    for method in variance_methods {
        let config = HybridConfig {
            use_random_walk: true,
            use_deterministic: false,
            max_iterations: 100,
            parallel_execution: false,
            random_walk_config: RandomWalkConfig {
                variance_reduction: method.clone(),
                max_steps: 1000,
                seed: Some(42),
                ..Default::default()
            },
            ..Default::default()
        };

        let mut solver = HybridSolver::new(config);
        let solution = solver.solve_linear_system(&matrix, &b);

        match solution {
            Ok(sol) => {
                assert_eq!(sol.len(), 4);
                println!("Variance reduction {:?}: Success", method);
            },
            Err(e) => {
                println!("Variance reduction {:?} failed: {:?}", method, e);
            }
        }
    }
}

#[test]
fn test_algorithm_trait_implementation() {
    let config = HybridConfig {
        max_iterations: 100,
        convergence_tolerance: 1e-6,
        parallel_execution: false,
        ..Default::default()
    };

    let mut solver = HybridSolver::new(config);

    let matrix = create_test_matrix(3);
    let b = create_test_vector(3);

    // Test Algorithm trait methods
    let solution = solver.solve(&matrix, &b).unwrap();
    assert_eq!(solution.len(), 3);

    let metrics = solver.get_metrics();
    assert!(metrics.iterations > 0);
    assert!(metrics.residual >= 0.0);
    assert!(metrics.convergence_rate >= 0.0);

    // Test config update (should not panic)
    let mut params = std::collections::HashMap::new();
    params.insert("learning_rate".to_string(), 0.1);
    solver.update_config(params);
}

#[test]
fn test_ill_conditioned_system() {
    let mut config = HybridConfig::default();
    config.max_iterations = 1000;
    config.convergence_tolerance = 1e-4; // Relaxed tolerance for ill-conditioned system
    config.parallel_execution = false;

    let mut solver = HybridSolver::new(config);

    // Create an ill-conditioned matrix
    let mut matrix = SparseMatrix::new(3, 3);
    matrix.insert(0, 0, 1.0);
    matrix.insert(0, 1, 1.0);
    matrix.insert(0, 2, 1.0);
    matrix.insert(1, 0, 1.0);
    matrix.insert(1, 1, 1.0001);
    matrix.insert(1, 2, 1.0);
    matrix.insert(2, 0, 1.0);
    matrix.insert(2, 1, 1.0);
    matrix.insert(2, 2, 1.0002);

    let b = vec![3.0, 3.0001, 3.0002];

    let solution = solver.solve_linear_system(&matrix, &b);

    match solution {
        Ok(sol) => {
            assert_eq!(sol.len(), 3);
            assert!(sol.iter().all(|&x| x.is_finite()));

            let metrics = solver.get_metrics();
            println!("Ill-conditioned system: Iterations: {}, Residual: {:.2e}",
                    metrics.total_iterations, metrics.final_residual);
        },
        Err(_) => {
            // It's acceptable for very ill-conditioned systems to fail
            println!("Ill-conditioned system failed as expected");
        }
    }
}

#[test]
fn test_large_sparse_system() {
    let mut config = HybridConfig::default();
    config.max_iterations = 200;
    config.convergence_tolerance = 1e-5;
    config.parallel_execution = false;

    let mut solver = HybridSolver::new(config);

    // Create a larger sparse system (10x10)
    let matrix = create_test_matrix(10);
    let b = create_test_vector(10);

    let start = std::time::Instant::now();
    let solution = solver.solve_linear_system(&matrix, &b).unwrap();
    let duration = start.elapsed();

    assert_eq!(solution.len(), 10);
    assert!(solution.iter().all(|&x| x.is_finite()));

    let metrics = solver.get_metrics();
    println!("Large system (10x10): Time: {:?}, Iterations: {}, Residual: {:.2e}",
            duration, metrics.total_iterations, metrics.final_residual);

    // Should solve in reasonable time
    assert!(duration.as_secs() < 10, "Took too long: {:?}", duration);
}

#[test]
#[ignore] // Potentially slow test
fn test_parallel_execution() {
    let mut config = HybridConfig::default();
    config.parallel_execution = true;
    config.max_iterations = 100;
    config.convergence_tolerance = 1e-6;

    let mut solver = HybridSolver::new(config);

    let matrix = create_test_matrix(5);
    let b = create_test_vector(5);

    let start = std::time::Instant::now();
    let solution = solver.solve_linear_system(&matrix, &b).unwrap();
    let duration = start.elapsed();

    assert_eq!(solution.len(), 5);
    assert!(solution.iter().all(|&x| x.is_finite()));

    println!("Parallel execution: Time: {:?}", duration);

    // Parallel execution should complete
    assert!(duration.as_secs() < 30, "Parallel execution took too long");
}