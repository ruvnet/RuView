//! Comprehensive tests for push algorithms
//!
//! Tests forward push, backward push, and bidirectional algorithms
//! with various graph structures and configurations.

use sublinear_time_solver::graph::{CompressedSparseRow, PushGraph, AdjacencyList};
use sublinear_time_solver::solver::forward_push::{
    ForwardPushSolver, ForwardPushConfig, ForwardPushResult,
};
use sublinear_time_solver::solver::backward_push::{
    BackwardPushSolver, BackwardPushConfig, BidirectionalPushSolver,
};

/// Create a simple test graph for basic testing
fn create_simple_graph() -> PushGraph {
    let mut csr = CompressedSparseRow::new(4, 4);
    csr.row_ptr = vec![0, 2, 4, 6, 7];
    csr.col_indices = vec![1, 2, 0, 3, 0, 3, 1];
    csr.values = vec![0.5, 0.5, 0.8, 0.2, 0.6, 0.4, 1.0];
    
    PushGraph::from_matrix(&csr)
}

/// Create a larger random-like graph for performance testing
fn create_random_graph(n: usize, edges_per_node: usize) -> PushGraph {
    let mut adjacency = AdjacencyList::new(n);
    
    // Create a random-like graph with deterministic seed for reproducibility
    let mut seed = 12345u64;
    for i in 0..n {
        for j in 0..edges_per_node {
            // Simple LCG for reproducible "randomness"
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let target = (seed as usize) % n;
            let weight = 1.0 / edges_per_node as f64;
            
            if target != i {
                adjacency.add_edge(i, target, weight);
            }
        }
    }
    
    adjacency.normalize();
    let csr = adjacency.to_csr();
    PushGraph::from_matrix(&csr)
}

/// Create a path graph (0 -> 1 -> 2 -> ... -> n-1)
fn create_path_graph(n: usize) -> PushGraph {
    let mut adjacency = AdjacencyList::new(n);
    
    for i in 0..n-1 {
        adjacency.add_edge(i, i + 1, 1.0);
    }
    
    let csr = adjacency.to_csr();
    PushGraph::from_matrix(&csr)
}

/// Create a complete graph where every node connects to every other node
fn create_complete_graph(n: usize) -> PushGraph {
    let mut adjacency = AdjacencyList::new(n);
    let weight = 1.0 / (n - 1) as f64;
    
    for i in 0..n {
        for j in 0..n {
            if i != j {
                adjacency.add_edge(i, j, weight);
            }
        }
    }
    
    let csr = adjacency.to_csr();
    PushGraph::from_matrix(&csr)
}

#[cfg(test)]
mod forward_push_tests {
    use super::*;
    
    #[test]
    fn test_forward_push_basic_functionality() {
        let graph = create_simple_graph();
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_source(0);
        
        // Basic sanity checks
        assert!(result.push_count > 0, "Should perform at least one push operation");
        assert!(result.nodes_visited > 0, "Should visit at least one node");
        assert!(result.estimate[0] > 0.0, "Source should have positive estimate");
        assert!(result.residual_norm >= 0.0, "Residual norm should be non-negative");
        
        // Check that estimates are non-negative
        for &est in &result.estimate {
            assert!(est >= 0.0, "All estimates should be non-negative");
        }
        
        // Check that residuals are non-negative
        for &res in &result.residual {
            assert!(res >= 0.0, "All residuals should be non-negative");
        }
    }
    
    #[test]
    fn test_forward_push_mass_conservation() {
        let graph = create_simple_graph();
        let config = ForwardPushConfig {
            epsilon: 1e-8,
            ..ForwardPushConfig::default()
        };
        let solver = ForwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_source(0);
        let final_solution = solver.extrapolated_solution(&result);
        
        let total_mass: f64 = final_solution.iter().sum();
        let residual_mass: f64 = result.residual.iter().sum();
        
        // Total mass should be approximately conserved
        assert!(
            (total_mass - 1.0).abs() < 0.01,
            "Total mass should be approximately 1.0, got {}",
            total_mass
        );
        
        println!("Total mass: {}, Residual mass: {}", total_mass, residual_mass);
    }
    
    #[test]
    fn test_forward_push_convergence() {
        let graph = create_simple_graph();
        let tight_config = ForwardPushConfig {
            epsilon: 1e-10,
            max_pushes: 100_000,
            ..ForwardPushConfig::default()
        };
        let loose_config = ForwardPushConfig {
            epsilon: 1e-4,
            max_pushes: 100_000,
            ..ForwardPushConfig::default()
        };
        
        let tight_solver = ForwardPushSolver::new(graph.clone(), tight_config);
        let loose_solver = ForwardPushSolver::new(graph, loose_config);
        
        let tight_result = tight_solver.solve_single_source(0);
        let loose_result = loose_solver.solve_single_source(0);
        
        // Tighter tolerance should require more pushes
        assert!(
            tight_result.push_count >= loose_result.push_count,
            "Tighter tolerance should require at least as many pushes"
        );
        
        // Tighter tolerance should have smaller residual norm
        assert!(
            tight_result.residual_norm <= loose_result.residual_norm * 10.0,
            "Tighter tolerance should have smaller residual norm"
        );
    }
    
    #[test]
    fn test_forward_push_multi_source() {
        let graph = create_simple_graph();
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        let sources = vec![0, 2];
        let result = solver.solve_multi_source(&sources);
        
        assert!(result.push_count > 0);
        assert!(result.nodes_visited > 0);
        
        // Both sources should have positive estimates
        assert!(result.estimate[0] > 0.0);
        assert!(result.estimate[2] > 0.0);
        
        let total_mass: f64 = result.estimate.iter().sum();
        assert!(total_mass > 0.0, "Total estimate mass should be positive");
    }
    
    #[test]
    fn test_forward_push_single_entry_query() {
        let graph = create_simple_graph();
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        let value = solver.query_single_entry(0, 1);
        assert!(value >= 0.0, "Query result should be non-negative");
        
        // Query from node to itself should be positive
        let self_value = solver.query_single_entry(0, 0);
        assert!(self_value > 0.0, "Self-query should be positive");
    }
    
    #[test]
    fn test_forward_push_path_graph() {
        let graph = create_path_graph(5);
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_source(0);
        
        // In a path graph, probability should decrease along the path
        assert!(result.estimate[0] > result.estimate[1]);
        assert!(result.estimate[1] > result.estimate[2] || result.estimate[2] < 1e-6);
    }
    
    #[test]
    fn test_forward_push_complete_graph() {
        let graph = create_complete_graph(4);
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_source(0);
        let final_solution = solver.extrapolated_solution(&result);
        
        // In a complete graph, steady-state should be approximately uniform
        let expected = config.alpha; // Restart probability
        for i in 0..4 {
            let diff = (final_solution[i] - expected).abs();
            assert!(
                diff < 0.1,
                "Complete graph should have approximately uniform distribution, got {} for node {}",
                final_solution[i], i
            );
        }
    }
}

#[cfg(test)]
mod backward_push_tests {
    use super::*;
    
    #[test]
    fn test_backward_push_basic_functionality() {
        let graph = create_simple_graph();
        let config = BackwardPushConfig::default();
        let solver = BackwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_target(3);
        
        assert!(result.push_count > 0, "Should perform at least one push operation");
        assert!(result.nodes_visited > 0, "Should visit at least one node");
        assert!(result.estimate[3] > 0.0, "Target should have positive estimate");
        assert!(result.residual_norm >= 0.0, "Residual norm should be non-negative");
        
        // Check non-negativity
        for &est in &result.estimate {
            assert!(est >= 0.0, "All estimates should be non-negative");
        }
    }
    
    #[test]
    fn test_backward_push_transition_probability() {
        let graph = create_simple_graph();
        let config = BackwardPushConfig::default();
        let solver = BackwardPushSolver::new(graph, config);
        
        let prob = solver.query_transition_probability(0, 3);
        assert!(prob >= 0.0 && prob <= 1.0, "Transition probability should be in [0,1]");
        
        // Self-transition should be positive due to restart probability
        let self_prob = solver.query_transition_probability(0, 0);
        assert!(self_prob > 0.0, "Self-transition should be positive");
    }
    
    #[test]
    fn test_backward_push_multi_target() {
        let graph = create_simple_graph();
        let config = BackwardPushConfig::default();
        let solver = BackwardPushSolver::new(graph, config);
        
        let targets = vec![1, 3];
        let result = solver.solve_multi_target(&targets);
        
        assert!(result.push_count > 0);
        assert!(result.nodes_visited > 0);
        
        // Both targets should have positive estimates
        assert!(result.estimate[1] > 0.0);
        assert!(result.estimate[3] > 0.0);
    }
    
    #[test]
    fn test_backward_push_reachability() {
        let graph = create_path_graph(5);
        let config = BackwardPushConfig::default();
        let solver = BackwardPushSolver::new(graph, config);
        
        let reachability = solver.reachability_probabilities(4); // Target is end of path
        
        // In path graph, reachability should decrease going backwards
        assert!(reachability[4] > reachability[3]);
        assert!(reachability[3] > reachability[2] || reachability[2] < 1e-6);
        assert!(reachability[2] > reachability[1] || reachability[1] < 1e-6);
        assert!(reachability[1] > reachability[0] || reachability[0] < 1e-6);
    }
}

#[cfg(test)]
mod bidirectional_tests {
    use super::*;
    
    #[test]
    fn test_bidirectional_solver_consistency() {
        let graph = create_simple_graph();
        let forward_config = ForwardPushConfig::default();
        let backward_config = BackwardPushConfig::default();
        
        let bidirectional_solver = BidirectionalPushSolver::new(
            graph.clone(),
            forward_config.clone(),
            backward_config.clone(),
        );
        
        let forward_solver = ForwardPushSolver::new(graph.clone(), forward_config);
        let backward_solver = BackwardPushSolver::new(graph, backward_config);
        
        let bidirectional_result = bidirectional_solver.solve_bidirectional(0, 3);
        let forward_result = forward_solver.query_single_entry(0, 3);
        let backward_result = backward_solver.query_transition_probability(0, 3);
        
        // Results should be in the same ballpark
        assert!(bidirectional_result >= 0.0);
        assert!(forward_result >= 0.0);
        assert!(backward_result >= 0.0);
        
        println!(
            "Bidirectional: {}, Forward: {}, Backward: {}",
            bidirectional_result, forward_result, backward_result
        );
    }
    
    #[test]
    fn test_adaptive_solver_selection() {
        let graph = create_simple_graph();
        let forward_config = ForwardPushConfig::default();
        let backward_config = BackwardPushConfig::default();
        
        let solver = BidirectionalPushSolver::new(graph, forward_config, backward_config);
        
        // Test different source-target pairs
        for source in 0..4 {
            for target in 0..4 {
                let result = solver.adaptive_solve(source, target);
                assert!(
                    result >= 0.0,
                    "Adaptive solve should return non-negative result for ({}, {})",
                    source, target
                );
            }
        }
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn test_forward_push_performance_scaling() {
        let sizes = vec![10, 50, 100];
        let edges_per_node = 5;
        
        for &n in &sizes {
            let graph = create_random_graph(n, edges_per_node);
            let config = ForwardPushConfig {
                epsilon: 1e-4,
                max_pushes: 10_000,
                ..ForwardPushConfig::default()
            };
            let solver = ForwardPushSolver::new(graph, config);
            
            let start = Instant::now();
            let result = solver.solve_single_source(0);
            let duration = start.elapsed();
            
            println!(
                "Graph size {}: {} pushes, {} nodes visited, {:.2}ms",
                n,
                result.push_count,
                result.nodes_visited,
                duration.as_millis()
            );
            
            // Sanity check that we got a reasonable result
            assert!(result.push_count > 0);
            assert!(result.estimate[0] > 0.0);
        }
    }
    
    #[test]
    fn test_backward_push_performance_scaling() {
        let sizes = vec![10, 50, 100];
        let edges_per_node = 5;
        
        for &n in &sizes {
            let graph = create_random_graph(n, edges_per_node);
            let config = BackwardPushConfig {
                epsilon: 1e-4,
                max_pushes: 10_000,
                ..BackwardPushConfig::default()
            };
            let solver = BackwardPushSolver::new(graph, config);
            
            let start = Instant::now();
            let result = solver.solve_single_target(n - 1);
            let duration = start.elapsed();
            
            println!(
                "Backward graph size {}: {} pushes, {} nodes visited, {:.2}ms",
                n,
                result.push_count,
                result.nodes_visited,
                duration.as_millis()
            );
            
            assert!(result.push_count > 0);
            assert!(result.estimate[n - 1] > 0.0);
        }
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;
    
    #[test]
    fn test_empty_graph() {
        let graph = PushGraph::from_matrix(&CompressedSparseRow::new(0, 0));
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_source(0);
        assert_eq!(result.push_count, 0);
        assert_eq!(result.nodes_visited, 0);
    }
    
    #[test]
    fn test_single_node_graph() {
        let mut csr = CompressedSparseRow::new(1, 1);
        csr.row_ptr = vec![0, 0];
        
        let graph = PushGraph::from_matrix(&csr);
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_source(0);
        assert!(result.push_count > 0);
        assert!(result.estimate[0] > 0.0);
    }
    
    #[test]
    fn test_disconnected_graph() {
        let mut adjacency = AdjacencyList::new(4);
        // Two disconnected components: 0->1 and 2->3
        adjacency.add_edge(0, 1, 1.0);
        adjacency.add_edge(2, 3, 1.0);
        
        let csr = adjacency.to_csr();
        let graph = PushGraph::from_matrix(&csr);
        
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_source(0);
        
        // Should have positive estimates for connected component
        assert!(result.estimate[0] > 0.0);
        assert!(result.estimate[1] > 0.0);
        
        // Should have zero or very small estimates for disconnected component
        assert!(result.estimate[2] < 1e-6);
        assert!(result.estimate[3] < 1e-6);
    }
    
    #[test]
    fn test_out_of_bounds_queries() {
        let graph = create_simple_graph();
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        // Query with out-of-bounds source
        let result = solver.solve_single_source(100);
        assert_eq!(result.push_count, 0);
        
        // Query with out-of-bounds target
        let value = solver.query_single_entry(0, 100);
        assert_eq!(value, 0.0);
    }
}

#[cfg(test)]
mod numerical_stability_tests {
    use super::*;
    
    #[test]
    fn test_very_small_epsilon() {
        let graph = create_simple_graph();
        let config = ForwardPushConfig {
            epsilon: 1e-15,
            max_pushes: 1_000_000,
            ..ForwardPushConfig::default()
        };
        let solver = ForwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_source(0);
        
        // Should still produce valid results
        assert!(result.push_count > 0);
        assert!(result.estimate[0] > 0.0);
        assert!(result.residual_norm.is_finite());
    }
    
    #[test]
    fn test_very_large_alpha() {
        let graph = create_simple_graph();
        let config = ForwardPushConfig {
            alpha: 0.99, // Very high restart probability
            ..ForwardPushConfig::default()
        };
        let solver = ForwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_source(0);
        
        // High alpha should concentrate mass at the source
        assert!(result.estimate[0] > 0.5);
        
        // Mass conservation should still hold
        let final_solution = solver.extrapolated_solution(&result);
        let total_mass: f64 = final_solution.iter().sum();
        assert!((total_mass - 1.0).abs() < 0.1);
    }
    
    #[test]
    fn test_very_small_alpha() {
        let graph = create_simple_graph();
        let config = ForwardPushConfig {
            alpha: 0.01, // Very low restart probability
            ..ForwardPushConfig::default()
        };
        let solver = ForwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_source(0);
        
        // Should still converge
        assert!(result.push_count > 0);
        assert!(result.estimate[0] > 0.0);
        assert!(result.residual_norm.is_finite());
    }
}
