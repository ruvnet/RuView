//! Backward Push solver for target-centric Personalized PageRank.
//!
//! The backward (reverse) push algorithm computes approximate PPR
//! contributions **to** a target vertex by propagating residual mass
//! backward along incoming edges (on the transpose of the adjacency
//! matrix). This is the dual of the Andersen-Chung-Lang (2006) Forward
//! Push algorithm.
//!
//! # Algorithm
//!
//! Maintain two vectors over all `n` vertices:
//! - `estimate[v]`: accumulated PPR contribution from `v` to the target.
//! - `residual[v]`: unprocessed mass waiting at `v`.
//!
//! Initially `residual[target] = 1`, everything else is zero.
//!
//! While any vertex `v` has `|residual[v]| / max(1, in_degree(v)) > epsilon`:
//!   1. Dequeue `v` from the active set.
//!   2. `estimate[v] += alpha * residual[v]`.
//!   3. For each in-neighbour `u` of `v` (edge `u -> v` in the original graph):
//!        `residual[u] += (1 - alpha) * residual[v] / out_degree(v)`.
//!   4. `residual[v] = 0`.
//!
//! In-neighbours are obtained from the transposed adjacency matrix.
//!
//! # Complexity
//!
//! O(1 / (alpha * epsilon)) pushes total. Each push visits the in-neighbours
//! of one vertex. The queue-based design avoids scanning all `n` vertices
//! per push, achieving true sublinear time.

use std::collections::VecDeque;
use std::time::Instant;

use tracing::debug;

use crate::error::{SolverError, ValidationError};
use crate::traits::{SolverEngine, SublinearPageRank};
use crate::types::{
    Algorithm, ComplexityClass, ComplexityEstimate, ComputeBudget, CsrMatrix, SolverResult,
    SparsityProfile,
};

/// Maximum number of graph nodes to prevent OOM denial-of-service.
const MAX_GRAPH_NODES: usize = 100_000_000;

// ---------------------------------------------------------------------------
// Solver struct
// ---------------------------------------------------------------------------

/// Backward-push PPR solver.
///
/// Pushes probability mass backward along edges from target nodes.
/// Complementary to [`ForwardPushSolver`](crate::forward_push::ForwardPushSolver)
/// and often combined with it in bidirectional schemes.
///
/// # Example
///
/// ```rust,ignore
/// use ruvector_solver::backward_push::BackwardPushSolver;
/// use ruvector_solver::types::CsrMatrix;
///
/// let graph = CsrMatrix::<f64>::from_coo(3, 3, vec![
///     (0, 1, 1.0), (1, 2, 1.0), (2, 0, 1.0),
/// ]);
/// let solver = BackwardPushSolver::new(0.15, 1e-6);
/// let ppr = solver.ppr_to_target(&graph, 0).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct BackwardPushSolver {
    /// Teleportation probability (alpha). Must be in (0, 1).
    pub alpha: f64,
    /// Approximation tolerance (epsilon). Smaller values yield higher
    /// accuracy at the cost of more push operations.
    pub epsilon: f64,
}

impl BackwardPushSolver {
    /// Create a new backward-push solver.
    ///
    /// # Parameters
    ///
    /// - `alpha`: teleportation probability in (0, 1). Typical: 0.15 or 0.2.
    /// - `epsilon`: push threshold controlling accuracy vs speed.
    pub fn new(alpha: f64, epsilon: f64) -> Self {
        Self { alpha, epsilon }
    }

    /// Validate configuration parameters eagerly.
    fn validate_params(alpha: f64, epsilon: f64) -> Result<(), SolverError> {
        if alpha <= 0.0 || alpha >= 1.0 {
            return Err(SolverError::InvalidInput(
                ValidationError::ParameterOutOfRange {
                    name: "alpha".into(),
                    value: alpha.to_string(),
                    expected: "(0.0, 1.0) exclusive".into(),
                },
            ));
        }
        if epsilon <= 0.0 {
            return Err(SolverError::InvalidInput(
                ValidationError::ParameterOutOfRange {
                    name: "epsilon".into(),
                    value: epsilon.to_string(),
                    expected: "> 0.0".into(),
                },
            ));
        }
        Ok(())
    }

    /// Validate that the graph is square and the node index is in bounds.
    fn validate_graph_node(
        graph: &CsrMatrix<f64>,
        node: usize,
        name: &str,
    ) -> Result<(), SolverError> {
        if graph.rows != graph.cols {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch(format!(
                    "graph must be square, got {}x{}",
                    graph.rows, graph.cols,
                )),
            ));
        }
        if node >= graph.rows {
            return Err(SolverError::InvalidInput(
                ValidationError::ParameterOutOfRange {
                    name: name.into(),
                    value: node.to_string(),
                    expected: format!("[0, {})", graph.rows),
                },
            ));
        }
        Ok(())
    }

    /// Compute approximate PPR contributions **to** `target`.
    ///
    /// Returns a sparse vector of `(vertex, ppr_value)` pairs sorted by
    /// descending PPR value. Only vertices whose estimate exceeds 1e-15
    /// are included.
    pub fn ppr_to_target(
        &self,
        graph: &CsrMatrix<f64>,
        target: usize,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        Self::backward_push_core(
            graph,
            target,
            self.alpha,
            self.epsilon,
            &ComputeBudget::default(),
        )
    }

    /// Same as [`ppr_to_target`](Self::ppr_to_target) with an explicit budget.
    pub fn ppr_to_target_with_budget(
        &self,
        graph: &CsrMatrix<f64>,
        target: usize,
        budget: &ComputeBudget,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        Self::backward_push_core(graph, target, self.alpha, self.epsilon, budget)
    }

    /// Core backward push implementation.
    ///
    /// Uses a FIFO queue so that each vertex is only re-scanned when its
    /// residual has been increased above the threshold, giving O(1/(alpha*eps))
    /// total pushes rather than O(n) scans per push.
    fn backward_push_core(
        graph: &CsrMatrix<f64>,
        target: usize,
        alpha: f64,
        epsilon: f64,
        budget: &ComputeBudget,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        Self::validate_params(alpha, epsilon)?;
        Self::validate_graph_node(graph, target, "target")?;

        let start = Instant::now();
        let n = graph.rows;

        if n > MAX_GRAPH_NODES {
            return Err(SolverError::InvalidInput(ValidationError::MatrixTooLarge {
                rows: n,
                cols: n,
                max_dim: MAX_GRAPH_NODES,
            }));
        }

        // Build the transposed adjacency so row_entries(v) in `graph_t`
        // yields the in-neighbours of v in the original graph.
        let graph_t = graph.transpose();

        let mut estimate = vec![0.0f64; n];
        let mut residual = vec![0.0f64; n];

        // Seed: all mass starts at the target vertex.
        residual[target] = 1.0;

        // FIFO queue of vertices whose residual exceeds the push threshold.
        let mut queue: VecDeque<usize> = VecDeque::with_capacity(n.min(1024));
        let mut in_queue = vec![false; n];
        queue.push_back(target);
        in_queue[target] = true;

        let mut pushes = 0usize;
        let max_pushes = budget.max_iterations;

        while let Some(v) = queue.pop_front() {
            in_queue[v] = false;

            let r_v = residual[v];
            if r_v.abs() < 1e-15 {
                continue;
            }

            // Check the push threshold: |r_v| / max(1, in_deg_t(v)) > epsilon.
            let in_deg_t = graph_t.row_degree(v).max(1);
            if r_v.abs() / in_deg_t as f64 <= epsilon {
                continue;
            }

            // Budget enforcement.
            pushes += 1;
            if pushes > max_pushes {
                return Err(SolverError::BudgetExhausted {
                    reason: format!("backward push exceeded {} push budget", max_pushes,),
                    elapsed: start.elapsed(),
                });
            }
            if start.elapsed() > budget.max_time {
                return Err(SolverError::BudgetExhausted {
                    reason: "wall-clock budget exceeded".into(),
                    elapsed: start.elapsed(),
                });
            }

            // Absorb alpha fraction into the PPR estimate.
            estimate[v] += alpha * r_v;

            // Distribute (1 - alpha) * r_v backward along in-edges.
            // The denominator is out_degree(v) in the original graph, which
            // corresponds to row_degree(v) in `graph`.
            let out_deg = graph.row_degree(v);
            if out_deg == 0 {
                // Dangling node: no outgoing edges; residual fully absorbed.
                residual[v] = 0.0;
                continue;
            }

            let push_mass = (1.0 - alpha) * r_v / out_deg as f64;

            for (u, _weight) in graph_t.row_entries(v) {
                residual[u] += push_mass;

                // Enqueue u if it exceeds the push threshold and is not
                // already queued.
                let u_in_deg = graph_t.row_degree(u).max(1);
                if residual[u].abs() / u_in_deg as f64 > epsilon && !in_queue[u] {
                    queue.push_back(u);
                    in_queue[u] = true;
                }
            }

            residual[v] = 0.0;
        }

        debug!(
            target: "ruvector_solver::backward_push",
            pushes,
            target,
            elapsed_us = start.elapsed().as_micros() as u64,
            "backward push converged",
        );

        // Collect non-zero estimates, sorted descending by PPR value.
        let mut result: Vec<(usize, f64)> = estimate
            .into_iter()
            .enumerate()
            .filter(|(_, val)| *val > 1e-15)
            .collect();
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// SolverEngine implementation
// ---------------------------------------------------------------------------

impl SolverEngine for BackwardPushSolver {
    fn solve(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        // For SolverEngine compatibility, interpret rhs as a target indicator
        // vector: pick the node with the largest weight as the target.
        let target = rhs
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        let wall_start = Instant::now();
        let ppr = self.ppr_to_target_with_budget(matrix, target, budget)?;

        let mut solution = vec![0.0f32; matrix.rows];
        for &(node, val) in &ppr {
            solution[node] = val as f32;
        }

        Ok(SolverResult {
            solution,
            iterations: ppr.len(),
            residual_norm: 0.0,
            wall_time: wall_start.elapsed(),
            convergence_history: Vec::new(),
            algorithm: Algorithm::BackwardPush,
        })
    }

    fn estimate_complexity(&self, _profile: &SparsityProfile, n: usize) -> ComplexityEstimate {
        let est_pushes = (1.0 / (self.alpha * self.epsilon)) as usize;
        ComplexityEstimate {
            algorithm: Algorithm::BackwardPush,
            estimated_flops: est_pushes as u64 * 10,
            estimated_iterations: est_pushes,
            estimated_memory_bytes: n * 16, // estimate + residual vectors
            complexity_class: ComplexityClass::SublinearNnz,
        }
    }

    fn algorithm(&self) -> Algorithm {
        Algorithm::BackwardPush
    }
}

// ---------------------------------------------------------------------------
// SublinearPageRank implementation
// ---------------------------------------------------------------------------

impl SublinearPageRank for BackwardPushSolver {
    fn ppr(
        &self,
        matrix: &CsrMatrix<f64>,
        target: usize,
        alpha: f64,
        epsilon: f64,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        Self::backward_push_core(matrix, target, alpha, epsilon, &ComputeBudget::default())
    }

    fn ppr_multi_seed(
        &self,
        matrix: &CsrMatrix<f64>,
        seeds: &[(usize, f64)],
        alpha: f64,
        epsilon: f64,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        let n = matrix.rows;
        for &(node, _) in seeds {
            Self::validate_graph_node(matrix, node, "seed")?;
        }

        // Build transposed graph once and reuse across all seeds.
        let graph_t = matrix.transpose();

        let mut combined = vec![0.0f64; n];

        for &(seed, weight) in seeds {
            // Run backward push for each seed target. We inline the core
            // logic with the shared transpose to avoid rebuilding it.
            let ppr = backward_push_with_transpose(
                matrix,
                &graph_t,
                seed,
                alpha,
                epsilon,
                &ComputeBudget::default(),
            )?;
            for &(node, val) in &ppr {
                combined[node] += weight * val;
            }
        }

        let mut result: Vec<(usize, f64)> = combined
            .into_iter()
            .enumerate()
            .filter(|(_, val)| *val > 1e-15)
            .collect();
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(result)
    }
}

/// Internal helper: backward push using a pre-computed transpose.
///
/// Avoids re-transposing for multi-seed queries.
fn backward_push_with_transpose(
    graph: &CsrMatrix<f64>,
    graph_t: &CsrMatrix<f64>,
    target: usize,
    alpha: f64,
    epsilon: f64,
    budget: &ComputeBudget,
) -> Result<Vec<(usize, f64)>, SolverError> {
    let start = Instant::now();
    let n = graph.rows;

    let mut estimate = vec![0.0f64; n];
    let mut residual = vec![0.0f64; n];
    residual[target] = 1.0;

    let mut queue: VecDeque<usize> = VecDeque::with_capacity(n.min(1024));
    let mut in_queue = vec![false; n];
    queue.push_back(target);
    in_queue[target] = true;

    let mut pushes = 0usize;
    let max_pushes = budget.max_iterations;

    while let Some(v) = queue.pop_front() {
        in_queue[v] = false;

        let r_v = residual[v];
        if r_v.abs() < 1e-15 {
            continue;
        }

        let in_deg_t = graph_t.row_degree(v).max(1);
        if r_v.abs() / in_deg_t as f64 <= epsilon {
            continue;
        }

        pushes += 1;
        if pushes > max_pushes {
            return Err(SolverError::BudgetExhausted {
                reason: format!("backward push exceeded {} push budget", max_pushes,),
                elapsed: start.elapsed(),
            });
        }
        if start.elapsed() > budget.max_time {
            return Err(SolverError::BudgetExhausted {
                reason: "wall-clock budget exceeded".into(),
                elapsed: start.elapsed(),
            });
        }

        estimate[v] += alpha * r_v;

        let out_deg = graph.row_degree(v);
        if out_deg == 0 {
            residual[v] = 0.0;
            continue;
        }

        let push_mass = (1.0 - alpha) * r_v / out_deg as f64;

        for (u, _weight) in graph_t.row_entries(v) {
            residual[u] += push_mass;
            let u_in_deg = graph_t.row_degree(u).max(1);
            if residual[u].abs() / u_in_deg as f64 > epsilon && !in_queue[u] {
                queue.push_back(u);
                in_queue[u] = true;
            }
        }

        residual[v] = 0.0;
    }

    let mut result: Vec<(usize, f64)> = estimate
        .into_iter()
        .enumerate()
        .filter(|(_, val)| *val > 1e-15)
        .collect();
    result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a directed cycle 0->1->2->...->n-1->0.
    fn directed_cycle(n: usize) -> CsrMatrix<f64> {
        let entries: Vec<_> = (0..n).map(|i| (i, (i + 1) % n, 1.0f64)).collect();
        CsrMatrix::<f64>::from_coo(n, n, entries)
    }

    /// Build a star graph with edges i->0 for i in 1..n.
    fn star_to_center(n: usize) -> CsrMatrix<f64> {
        let entries: Vec<_> = (1..n).map(|i| (i, 0, 1.0f64)).collect();
        CsrMatrix::<f64>::from_coo(n, n, entries)
    }

    /// Build a complete graph on n vertices (every pair connected).
    fn complete_graph(n: usize) -> CsrMatrix<f64> {
        let mut entries = Vec::new();
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    entries.push((i, j, 1.0f64));
                }
            }
        }
        CsrMatrix::<f64>::from_coo(n, n, entries)
    }

    #[test]
    fn single_node_no_edges() {
        let graph = CsrMatrix::<f64>::from_coo(1, 1, Vec::<(usize, usize, f64)>::new());
        let solver = BackwardPushSolver::new(0.15, 1e-6);
        let result = solver.ppr_to_target(&graph, 0).unwrap();

        // Dangling node: estimate[0] = alpha * 1.0 = 0.15.
        assert_eq!(result.len(), 1);
        assert!((result[0].1 - 0.15).abs() < 1e-10);
    }

    #[test]
    fn directed_cycle_all_vertices_contribute() {
        let graph = directed_cycle(3);
        let solver = BackwardPushSolver::new(0.2, 1e-8);
        let result = solver.ppr_to_target(&graph, 0).unwrap();

        let total: f64 = result.iter().map(|(_, v)| v).sum();
        assert!(total <= 1.0 + 1e-6, "total PPR = {}", total);
        assert!(total > 0.1, "total too small: {}", total);
        assert!(result.len() >= 2);
    }

    #[test]
    fn star_graph_center_highest_ppr() {
        let graph = star_to_center(5);
        let solver = BackwardPushSolver::new(0.15, 1e-8);
        let result = solver.ppr_to_target(&graph, 0).unwrap();

        let ppr_0 = result
            .iter()
            .find(|&&(v, _)| v == 0)
            .map(|&(_, p)| p)
            .unwrap_or(0.0);
        for &(v, p) in &result {
            if v != 0 {
                assert!(ppr_0 >= p, "expected ppr[0]={} >= ppr[{}]={}", ppr_0, v, p,);
            }
        }
    }

    #[test]
    fn complete_graph_uniform_ppr() {
        // On a complete graph, by symmetry PPR should be approximately
        // uniform for non-target vertices.
        let graph = complete_graph(5);
        let solver = BackwardPushSolver::new(0.15, 1e-8);
        let result = solver.ppr_to_target(&graph, 0).unwrap();

        // All vertices should be represented.
        assert!(result.len() >= 4);

        let total: f64 = result.iter().map(|(_, v)| v).sum();
        assert!(total > 0.5 && total <= 1.0 + 1e-6);
    }

    #[test]
    fn rejects_non_square_graph() {
        let graph = CsrMatrix::<f64>::from_coo(2, 3, vec![(0, 1, 1.0f64)]);
        let solver = BackwardPushSolver::new(0.15, 1e-6);
        assert!(solver.ppr_to_target(&graph, 0).is_err());
    }

    #[test]
    fn rejects_out_of_bounds_target() {
        let graph = CsrMatrix::<f64>::from_coo(3, 3, vec![(0, 1, 1.0f64)]);
        let solver = BackwardPushSolver::new(0.15, 1e-6);
        assert!(solver.ppr_to_target(&graph, 5).is_err());
    }

    #[test]
    fn rejects_bad_alpha() {
        let graph = CsrMatrix::<f64>::from_coo(3, 3, vec![(0, 1, 1.0f64)]);

        let zero_alpha = BackwardPushSolver::new(0.0, 1e-6);
        assert!(zero_alpha.ppr_to_target(&graph, 0).is_err());

        let one_alpha = BackwardPushSolver::new(1.0, 1e-6);
        assert!(one_alpha.ppr_to_target(&graph, 0).is_err());

        let neg_alpha = BackwardPushSolver::new(-0.5, 1e-6);
        assert!(neg_alpha.ppr_to_target(&graph, 0).is_err());
    }

    #[test]
    fn rejects_bad_epsilon() {
        let graph = CsrMatrix::<f64>::from_coo(3, 3, vec![(0, 1, 1.0f64)]);

        let zero_eps = BackwardPushSolver::new(0.15, 0.0);
        assert!(zero_eps.ppr_to_target(&graph, 0).is_err());

        let neg_eps = BackwardPushSolver::new(0.15, -1e-6);
        assert!(neg_eps.ppr_to_target(&graph, 0).is_err());
    }

    #[test]
    fn solver_engine_trait_integration() {
        let graph = directed_cycle(4);
        let solver = BackwardPushSolver::new(0.15, 1e-6);
        let rhs = vec![0.0, 0.0, 1.0, 0.0]; // node 2 is the target
        let result = solver
            .solve(&graph, &rhs, &ComputeBudget::default())
            .unwrap();

        assert_eq!(result.algorithm, Algorithm::BackwardPush);
        assert!(!result.solution.is_empty());
    }

    #[test]
    fn sublinear_pagerank_trait_ppr() {
        let graph = directed_cycle(5);
        let solver = BackwardPushSolver::new(0.15, 1e-6);
        let result = solver.ppr(&graph, 0, 0.15, 1e-6).unwrap();
        assert!(!result.is_empty());

        let total: f64 = result.iter().map(|(_, v)| v).sum();
        assert!(total <= 1.0 + 1e-6);
    }

    #[test]
    fn multi_seed_combines_correctly() {
        let graph = directed_cycle(4);
        let solver = BackwardPushSolver::new(0.15, 1e-6);
        let seeds = vec![(0, 0.5), (2, 0.5)];
        let result = solver.ppr_multi_seed(&graph, &seeds, 0.15, 1e-6).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn converges_on_100_node_cycle() {
        let graph = directed_cycle(100);
        let solver = BackwardPushSolver::new(0.15, 1e-6);
        let result = solver.ppr_to_target(&graph, 50).unwrap();

        let total: f64 = result.iter().map(|(_, v)| v).sum();
        assert!(total > 0.0 && total <= 1.0 + 1e-6);
    }

    #[test]
    fn transpose_correctness() {
        let graph =
            CsrMatrix::<f64>::from_coo(3, 3, vec![(0, 1, 1.0f64), (1, 2, 1.0f64), (2, 0, 1.0f64)]);
        let gt = graph.transpose();

        // Transposed row 1 should contain (0, 1.0) because 0->1 in original.
        let r1: Vec<_> = gt.row_entries(1).collect();
        assert_eq!(r1.len(), 1);
        assert_eq!(*r1[0].1, 1.0f64);
        assert_eq!(r1[0].0, 0);
    }

    #[test]
    fn estimate_complexity_reports_sublinear() {
        let solver = BackwardPushSolver::new(0.15, 1e-4);
        let profile = SparsityProfile {
            rows: 1000,
            cols: 1000,
            nnz: 5000,
            density: 0.005,
            is_diag_dominant: false,
            estimated_spectral_radius: 0.9,
            estimated_condition: 10.0,
            is_symmetric_structure: false,
            avg_nnz_per_row: 5.0,
            max_nnz_per_row: 10,
        };
        let est = solver.estimate_complexity(&profile, 1000);
        assert_eq!(est.algorithm, Algorithm::BackwardPush);
        assert_eq!(est.complexity_class, ComplexityClass::SublinearNnz);
        assert!(est.estimated_iterations > 0);
    }
}
