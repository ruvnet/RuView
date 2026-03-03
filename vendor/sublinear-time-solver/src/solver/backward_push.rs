//! Backward push algorithm implementation
//!
//! Implements the backward push method for computing reverse personalized PageRank
//! and solving linear systems from the target perspective.

use crate::graph::{PushGraph, WorkQueue, VisitedTracker};
use std::collections::HashMap;

/// Result of a backward push operation
#[derive(Debug, Clone)]
pub struct BackwardPushResult {
    /// The computed estimate vector (backward probabilities)
    pub estimate: Vec<f64>,
    /// The residual vector after push operations
    pub residual: Vec<f64>,
    /// Number of push operations performed
    pub push_count: usize,
    /// Number of nodes visited
    pub nodes_visited: usize,
    /// Final residual norm
    pub residual_norm: f64,
}

/// Backward push solver configuration
#[derive(Debug, Clone)]
pub struct BackwardPushConfig {
    /// Restart probability (alpha)
    pub alpha: f64,
    /// Precision threshold (epsilon)
    pub epsilon: f64,
    /// Maximum number of push operations
    pub max_pushes: usize,
    /// Work queue threshold
    pub queue_threshold: f64,
    /// Whether to use adaptive thresholding
    pub adaptive_threshold: bool,
}

impl Default for BackwardPushConfig {
    fn default() -> Self {
        Self {
            alpha: 0.15,
            epsilon: 1e-6,
            max_pushes: 1_000_000,
            queue_threshold: 1e-8,
            adaptive_threshold: true,
        }
    }
}

/// Backward push algorithm implementation
#[derive(Debug)]
pub struct BackwardPushSolver {
    /// Graph representation optimized for push operations
    graph: PushGraph,
    /// Configuration parameters
    config: BackwardPushConfig,
}

impl BackwardPushSolver {
    /// Create a new backward push solver
    pub fn new(graph: PushGraph, config: BackwardPushConfig) -> Self {
        Self { graph, config }
    }
    
    /// Solve backward from a single target node
    pub fn solve_single_target(&self, target: usize) -> BackwardPushResult {
        let n = self.graph.num_nodes();
        let mut estimate = vec![0.0; n];
        let mut residual = vec![0.0; n];
        
        // Initialize with unit mass at target
        if target < n {
            residual[target] = 1.0;
        } else {
            return BackwardPushResult {
                estimate,
                residual,
                push_count: 0,
                nodes_visited: 0,
                residual_norm: 0.0,
            };
        }
        
        let mut work_queue = WorkQueue::new(self.config.queue_threshold);
        let mut visited = VisitedTracker::new(n);
        let mut push_count = 0;
        
        // Add target to work queue if it meets threshold
        let target_in_degree = self.graph.in_degree(target).max(1.0);
        work_queue.push_if_threshold(target, residual[target], target_in_degree);
        
        while !work_queue.is_empty() && push_count < self.config.max_pushes {
            if let Some((node, _priority)) = work_queue.pop() {
                // Check if node still meets threshold
                let node_in_degree = self.graph.in_degree(node).max(1.0);
                if residual[node] < self.config.epsilon * node_in_degree {
                    continue;
                }
                
                // Perform backward push operation
                self.backward_push_node(node, &mut estimate, &mut residual, &mut work_queue);
                visited.mark_visited(node);
                push_count += 1;
                
                // Adaptive threshold adjustment
                if self.config.adaptive_threshold && push_count % 1000 == 0 {
                    work_queue.adaptive_threshold(10000, 100);
                }
            }
        }
        
        let residual_norm = self.compute_residual_norm(&residual);
        
        BackwardPushResult {
            estimate,
            residual,
            push_count,
            nodes_visited: visited.num_visited(),
            residual_norm,
        }
    }
    
    /// Solve backward from multiple target nodes
    pub fn solve_multi_target(&self, targets: &[usize]) -> BackwardPushResult {
        let n = self.graph.num_nodes();
        let mut estimate = vec![0.0; n];
        let mut residual = vec![0.0; n];
        
        // Initialize with uniform mass at targets
        let mass_per_target = 1.0 / targets.len() as f64;
        for &target in targets {
            if target < n {
                residual[target] += mass_per_target;
            }
        }
        
        let mut work_queue = WorkQueue::new(self.config.queue_threshold);
        let mut visited = VisitedTracker::new(n);
        let mut push_count = 0;
        
        // Add all targets to work queue
        for &target in targets {
            if target < n {
                let target_in_degree = self.graph.in_degree(target).max(1.0);
                work_queue.push_if_threshold(target, residual[target], target_in_degree);
            }
        }
        
        while !work_queue.is_empty() && push_count < self.config.max_pushes {
            if let Some((node, _priority)) = work_queue.pop() {
                let node_in_degree = self.graph.in_degree(node).max(1.0);
                if residual[node] < self.config.epsilon * node_in_degree {
                    continue;
                }
                
                self.backward_push_node(node, &mut estimate, &mut residual, &mut work_queue);
                visited.mark_visited(node);
                push_count += 1;
                
                if self.config.adaptive_threshold && push_count % 1000 == 0 {
                    work_queue.adaptive_threshold(10000, 100);
                }
            }
        }
        
        let residual_norm = self.compute_residual_norm(&residual);
        
        BackwardPushResult {
            estimate,
            residual,
            push_count,
            nodes_visited: visited.num_visited(),
            residual_norm,
        }
    }
    
    /// Backward push operation on a single node
    fn backward_push_node(
        &self,
        node: usize,
        estimate: &mut [f64],
        residual: &mut [f64],
        work_queue: &mut WorkQueue,
    ) {
        if residual[node] <= 0.0 {
            return;
        }
        
        // Add alpha fraction to estimate (probability of starting from this node)
        let push_amount = self.config.alpha * residual[node];
        estimate[node] += push_amount;
        
        // Distribute remaining mass to incoming neighbors
        let remaining_mass = (1.0 - self.config.alpha) * residual[node];
        residual[node] = 0.0;
        
        let node_in_degree = self.graph.in_degree(node);
        
        if node_in_degree > 0.0 {
            // Distribute to incoming neighbors based on their transition probabilities
            for (predecessor, weight) in self.graph.backward_neighbors(node) {
                // The weight here represents the transition probability from predecessor to node
                let predecessor_out_degree = self.graph.out_degree(predecessor).max(1.0);
                let transition_prob = weight / predecessor_out_degree;
                let mass_to_transfer = remaining_mass * transition_prob;
                
                residual[predecessor] += mass_to_transfer;
                
                // Add to work queue if threshold is met
                let predecessor_in_degree = self.graph.in_degree(predecessor).max(1.0);
                work_queue.push_if_threshold(predecessor, residual[predecessor], predecessor_in_degree);
            }
        } else {
            // Self-loop for nodes with no incoming edges
            residual[node] += remaining_mass;
            let node_degree_effective = 1.0;
            work_queue.push_if_threshold(node, residual[node], node_degree_effective);
        }
    }
    
    /// Compute the L2 norm of the residual vector
    fn compute_residual_norm(&self, residual: &[f64]) -> f64 {
        residual.iter().map(|&x| x * x).sum::<f64>().sqrt()
    }
    
    /// Query transition probability from source to target
    pub fn query_transition_probability(&self, source: usize, target: usize) -> f64 {
        let result = self.solve_single_target(target);
        if source < result.estimate.len() {
            result.estimate[source]
        } else {
            0.0
        }
    }
    
    /// Solve with early termination when source reaches desired precision
    pub fn solve_with_source(&self, source: usize, target: usize, source_precision: f64) -> BackwardPushResult {
        let n = self.graph.num_nodes();
        let mut estimate = vec![0.0; n];
        let mut residual = vec![0.0; n];
        
        if source >= n || target >= n {
            return BackwardPushResult {
                estimate,
                residual,
                push_count: 0,
                nodes_visited: 0,
                residual_norm: 0.0,
            };
        }
        
        residual[target] = 1.0;
        
        let mut work_queue = WorkQueue::new(self.config.queue_threshold);
        let mut visited = VisitedTracker::new(n);
        let mut push_count = 0;
        
        let target_in_degree = self.graph.in_degree(target).max(1.0);
        work_queue.push_if_threshold(target, residual[target], target_in_degree);
        
        while !work_queue.is_empty() && push_count < self.config.max_pushes {
            // Check if source has reached desired precision
            if estimate[source] > source_precision && residual[source] < source_precision * 0.1 {
                break;
            }
            
            if let Some((node, _priority)) = work_queue.pop() {
                let node_in_degree = self.graph.in_degree(node).max(1.0);
                if residual[node] < self.config.epsilon * node_in_degree {
                    continue;
                }
                
                self.backward_push_node(node, &mut estimate, &mut residual, &mut work_queue);
                visited.mark_visited(node);
                push_count += 1;
                
                if self.config.adaptive_threshold && push_count % 1000 == 0 {
                    work_queue.adaptive_threshold(10000, 100);
                }
            }
        }
        
        let residual_norm = self.compute_residual_norm(&residual);
        
        BackwardPushResult {
            estimate,
            residual,
            push_count,
            nodes_visited: visited.num_visited(),
            residual_norm,
        }
    }
    
    /// Estimate reachability probabilities for all nodes to the target
    pub fn reachability_probabilities(&self, target: usize) -> Vec<f64> {
        let result = self.solve_single_target(target);
        self.extrapolated_solution(&result)
    }
    
    /// Estimate the final solution by extrapolating from current residual
    pub fn extrapolated_solution(&self, result: &BackwardPushResult) -> Vec<f64> {
        let mut solution = result.estimate.clone();
        
        // Add residual contribution (assumes uniform distribution)
        for (i, &res) in result.residual.iter().enumerate() {
            solution[i] += self.config.alpha * res;
        }
        
        solution
    }
    
    /// Combine with forward push result for bidirectional estimation
    pub fn combine_with_forward(
        &self,
        backward_result: &BackwardPushResult,
        forward_estimate: &[f64],
        forward_residual: &[f64],
    ) -> f64 {
        let mut total_probability = 0.0;
        
        // Combine estimates where both algorithms have computed values
        for i in 0..backward_result.estimate.len().min(forward_estimate.len()) {
            // Direct estimate combination
            total_probability += backward_result.estimate[i] * forward_estimate[i];
            
            // Add residual cross-terms
            total_probability += backward_result.residual[i] * forward_estimate[i] * self.config.alpha;
            total_probability += backward_result.estimate[i] * forward_residual[i] * self.config.alpha;
        }
        
        total_probability
    }
}

/// Bidirectional push solver that combines forward and backward push
#[derive(Debug)]
pub struct BidirectionalPushSolver {
    graph: PushGraph,
    forward_config: crate::solver::forward_push::ForwardPushConfig,
    backward_config: BackwardPushConfig,
}

impl BidirectionalPushSolver {
    /// Create a new bidirectional push solver
    pub fn new(
        graph: PushGraph,
        forward_config: crate::solver::forward_push::ForwardPushConfig,
        backward_config: BackwardPushConfig,
    ) -> Self {
        Self {
            graph,
            forward_config,
            backward_config,
        }
    }
    
    /// Solve using both forward and backward push for improved accuracy
    pub fn solve_bidirectional(&self, source: usize, target: usize) -> f64 {
        let forward_solver = crate::solver::forward_push::ForwardPushSolver::new(
            self.graph.clone(),
            self.forward_config.clone(),
        );
        let backward_solver = BackwardPushSolver::new(
            self.graph.clone(),
            self.backward_config.clone(),
        );
        
        let forward_result = forward_solver.solve_single_source(source);
        let backward_result = backward_solver.solve_single_target(target);
        
        backward_solver.combine_with_forward(
            &backward_result,
            &forward_result.estimate,
            &forward_result.residual,
        )
    }
    
    /// Decide whether to use forward, backward, or bidirectional approach
    pub fn adaptive_solve(&self, source: usize, target: usize) -> f64 {
        let n = self.graph.num_nodes();
        
        if source >= n || target >= n {
            return 0.0;
        }
        
        let source_out_degree = self.graph.out_degree(source);
        let target_in_degree = self.graph.in_degree(target);
        
        // Heuristic: use method that starts from node with higher degree
        if source_out_degree > target_in_degree * 2.0 {
            // Use backward push (start from target)
            let backward_solver = BackwardPushSolver::new(
                self.graph.clone(),
                self.backward_config.clone(),
            );
            backward_solver.query_transition_probability(source, target)
        } else if target_in_degree > source_out_degree * 2.0 {
            // Use forward push (start from source)
            let forward_solver = crate::solver::forward_push::ForwardPushSolver::new(
                self.graph.clone(),
                self.forward_config.clone(),
            );
            forward_solver.query_single_entry(source, target)
        } else {
            // Use bidirectional approach
            self.solve_bidirectional(source, target)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::CompressedSparseRow;
    
    fn create_test_graph() -> PushGraph {
        let mut csr = CompressedSparseRow::new(4, 4);
        csr.row_ptr = vec![0, 2, 4, 6, 7];
        csr.col_indices = vec![1, 2, 0, 3, 0, 3, 1];
        csr.values = vec![0.5, 0.5, 0.8, 0.2, 0.6, 0.4, 1.0];
        
        PushGraph::from_matrix(&csr)
    }
    
    #[test]
    fn test_backward_push_single_target() {
        let graph = create_test_graph();
        let config = BackwardPushConfig::default();
        let solver = BackwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_target(3);
        
        assert!(result.push_count > 0);
        assert!(result.nodes_visited > 0);
        assert!(result.estimate[3] > 0.0);
        assert!(result.residual_norm >= 0.0);
    }
    
    #[test]
    fn test_backward_push_query_transition() {
        let graph = create_test_graph();
        let config = BackwardPushConfig::default();
        let solver = BackwardPushSolver::new(graph, config);
        
        let prob = solver.query_transition_probability(0, 3);
        assert!(prob >= 0.0 && prob <= 1.0);
    }
    
    #[test]
    fn test_bidirectional_solver() {
        let graph = create_test_graph();
        let forward_config = crate::solver::forward_push::ForwardPushConfig::default();
        let backward_config = BackwardPushConfig::default();
        
        let solver = BidirectionalPushSolver::new(graph, forward_config, backward_config);
        let prob = solver.solve_bidirectional(0, 3);
        
        assert!(prob >= 0.0);
    }
    
    #[test]
    fn test_adaptive_solve() {
        let graph = create_test_graph();
        let forward_config = crate::solver::forward_push::ForwardPushConfig::default();
        let backward_config = BackwardPushConfig::default();
        
        let solver = BidirectionalPushSolver::new(graph, forward_config, backward_config);
        let prob = solver.adaptive_solve(0, 3);
        
        assert!(prob >= 0.0);
    }
}
