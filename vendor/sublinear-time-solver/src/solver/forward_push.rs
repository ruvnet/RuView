//! Forward push algorithm implementation
//!
//! Implements the forward push method for computing PageRank-style solutions
//! with sublinear time complexity for single-source queries.

use crate::graph::{PushGraph, WorkQueue, VisitedTracker};
use std::collections::HashMap;

/// Result of a forward push operation
#[derive(Debug, Clone)]
pub struct ForwardPushResult {
    /// The computed estimate vector
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

/// Forward push solver configuration
#[derive(Debug, Clone)]
pub struct ForwardPushConfig {
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

impl Default for ForwardPushConfig {
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

/// Forward push algorithm implementation
#[derive(Debug)]
pub struct ForwardPushSolver {
    /// Graph representation optimized for push operations
    graph: PushGraph,
    /// Configuration parameters
    config: ForwardPushConfig,
}

impl ForwardPushSolver {
    /// Create a new forward push solver
    pub fn new(graph: PushGraph, config: ForwardPushConfig) -> Self {
        Self { graph, config }
    }
    
    /// Solve for a single source node
    pub fn solve_single_source(&self, source: usize) -> ForwardPushResult {
        let n = self.graph.num_nodes();
        let mut estimate = vec![0.0; n];
        let mut residual = vec![0.0; n];
        
        // Initialize with unit mass at source
        if source < n {
            residual[source] = 1.0;
        } else {
            return ForwardPushResult {
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
        
        // Add source to work queue if it meets threshold
        let source_degree = self.graph.out_degree(source).max(1.0);
        work_queue.push_if_threshold(source, residual[source], source_degree);
        
        while !work_queue.is_empty() && push_count < self.config.max_pushes {
            if let Some((node, _priority)) = work_queue.pop() {
                // Check if node still meets threshold (residual might have changed)
                let node_degree = self.graph.out_degree(node).max(1.0);
                if residual[node] < self.config.epsilon * node_degree {
                    continue;
                }
                
                // Perform push operation
                self.push_node(node, &mut estimate, &mut residual, &mut work_queue);
                visited.mark_visited(node);
                push_count += 1;
                
                // Adaptive threshold adjustment
                if self.config.adaptive_threshold && push_count % 1000 == 0 {
                    work_queue.adaptive_threshold(10000, 100);
                }
            }
        }
        
        let residual_norm = self.compute_residual_norm(&residual);
        
        ForwardPushResult {
            estimate,
            residual,
            push_count,
            nodes_visited: visited.num_visited(),
            residual_norm,
        }
    }
    
    /// Solve for multiple source nodes
    pub fn solve_multi_source(&self, sources: &[usize]) -> ForwardPushResult {
        let n = self.graph.num_nodes();
        let mut estimate = vec![0.0; n];
        let mut residual = vec![0.0; n];
        
        // Initialize with uniform mass at sources
        let mass_per_source = 1.0 / sources.len() as f64;
        for &source in sources {
            if source < n {
                residual[source] += mass_per_source;
            }
        }
        
        let mut work_queue = WorkQueue::new(self.config.queue_threshold);
        let mut visited = VisitedTracker::new(n);
        let mut push_count = 0;
        
        // Add all sources to work queue
        for &source in sources {
            if source < n {
                let source_degree = self.graph.out_degree(source).max(1.0);
                work_queue.push_if_threshold(source, residual[source], source_degree);
            }
        }
        
        while !work_queue.is_empty() && push_count < self.config.max_pushes {
            if let Some((node, _priority)) = work_queue.pop() {
                let node_degree = self.graph.out_degree(node).max(1.0);
                if residual[node] < self.config.epsilon * node_degree {
                    continue;
                }
                
                self.push_node(node, &mut estimate, &mut residual, &mut work_queue);
                visited.mark_visited(node);
                push_count += 1;
                
                if self.config.adaptive_threshold && push_count % 1000 == 0 {
                    work_queue.adaptive_threshold(10000, 100);
                }
            }
        }
        
        let residual_norm = self.compute_residual_norm(&residual);
        
        ForwardPushResult {
            estimate,
            residual,
            push_count,
            nodes_visited: visited.num_visited(),
            residual_norm,
        }
    }
    
    /// Push operation on a single node
    fn push_node(
        &self,
        node: usize,
        estimate: &mut [f64],
        residual: &mut [f64],
        work_queue: &mut WorkQueue,
    ) {
        if residual[node] <= 0.0 {
            return;
        }
        
        // Add alpha fraction to estimate
        let push_amount = self.config.alpha * residual[node];
        estimate[node] += push_amount;
        
        // Distribute remaining mass to neighbors
        let remaining_mass = (1.0 - self.config.alpha) * residual[node];
        residual[node] = 0.0;
        
        let node_degree = self.graph.out_degree(node);
        
        if node_degree > 0.0 {
            // Distribute to neighbors based on edge weights
            for (neighbor, weight) in self.graph.forward_neighbors(node) {
                let mass_to_transfer = remaining_mass * weight / node_degree;
                residual[neighbor] += mass_to_transfer;
                
                // Add to work queue if threshold is met
                let neighbor_degree = self.graph.out_degree(neighbor).max(1.0);
                work_queue.push_if_threshold(neighbor, residual[neighbor], neighbor_degree);
            }
        } else {
            // Self-loop for nodes with no outgoing edges
            residual[node] += remaining_mass;
            let node_degree_effective = 1.0;
            work_queue.push_if_threshold(node, residual[node], node_degree_effective);
        }
    }
    
    /// Compute the L2 norm of the residual vector
    fn compute_residual_norm(&self, residual: &[f64]) -> f64 {
        residual.iter().map(|&x| x * x).sum::<f64>().sqrt()
    }
    
    /// Get a single entry from the solution (useful for personalized PageRank queries)
    pub fn query_single_entry(&self, source: usize, target: usize) -> f64 {
        let result = self.solve_single_source(source);
        if target < result.estimate.len() {
            result.estimate[target]
        } else {
            0.0
        }
    }
    
    /// Solve with early termination when target reaches desired precision
    pub fn solve_with_target(&self, source: usize, target: usize, target_precision: f64) -> ForwardPushResult {
        let n = self.graph.num_nodes();
        let mut estimate = vec![0.0; n];
        let mut residual = vec![0.0; n];
        
        if source >= n || target >= n {
            return ForwardPushResult {
                estimate,
                residual,
                push_count: 0,
                nodes_visited: 0,
                residual_norm: 0.0,
            };
        }
        
        residual[source] = 1.0;
        
        let mut work_queue = WorkQueue::new(self.config.queue_threshold);
        let mut visited = VisitedTracker::new(n);
        let mut push_count = 0;
        
        let source_degree = self.graph.out_degree(source).max(1.0);
        work_queue.push_if_threshold(source, residual[source], source_degree);
        
        while !work_queue.is_empty() && push_count < self.config.max_pushes {
            // Check if target has reached desired precision
            if estimate[target] > target_precision && residual[target] < target_precision * 0.1 {
                break;
            }
            
            if let Some((node, _priority)) = work_queue.pop() {
                let node_degree = self.graph.out_degree(node).max(1.0);
                if residual[node] < self.config.epsilon * node_degree {
                    continue;
                }
                
                self.push_node(node, &mut estimate, &mut residual, &mut work_queue);
                visited.mark_visited(node);
                push_count += 1;
                
                if self.config.adaptive_threshold && push_count % 1000 == 0 {
                    work_queue.adaptive_threshold(10000, 100);
                }
            }
        }
        
        let residual_norm = self.compute_residual_norm(&residual);
        
        ForwardPushResult {
            estimate,
            residual,
            push_count,
            nodes_visited: visited.num_visited(),
            residual_norm,
        }
    }
    
    /// Estimate the final solution by extrapolating from current residual
    pub fn extrapolated_solution(&self, result: &ForwardPushResult) -> Vec<f64> {
        let mut solution = result.estimate.clone();
        
        // Add residual contribution (assumes uniform distribution)
        for (i, &res) in result.residual.iter().enumerate() {
            solution[i] += self.config.alpha * res;
        }
        
        solution
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
    fn test_forward_push_single_source() {
        let graph = create_test_graph();
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_source(0);
        
        assert!(result.push_count > 0);
        assert!(result.nodes_visited > 0);
        assert!(result.estimate[0] > 0.0);
        assert!(result.residual_norm >= 0.0);
    }
    
    #[test]
    fn test_forward_push_query_single_entry() {
        let graph = create_test_graph();
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        let value = solver.query_single_entry(0, 1);
        assert!(value >= 0.0);
    }
    
    #[test]
    fn test_forward_push_multi_source() {
        let graph = create_test_graph();
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        let sources = vec![0, 2];
        let result = solver.solve_multi_source(&sources);
        
        assert!(result.push_count > 0);
        assert!(result.estimate.iter().sum::<f64>() > 0.0);
    }
    
    #[test]
    fn test_mass_conservation() {
        let graph = create_test_graph();
        let config = ForwardPushConfig::default();
        let solver = ForwardPushSolver::new(graph, config);
        
        let result = solver.solve_single_source(0);
        let final_solution = solver.extrapolated_solution(&result);
        
        // Total mass should be approximately conserved
        let total_mass = final_solution.iter().sum::<f64>();
        assert!((total_mass - 1.0).abs() < 0.1);
    }
}
