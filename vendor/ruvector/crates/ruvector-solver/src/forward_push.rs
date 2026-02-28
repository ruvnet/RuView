//! Forward Push solver for Personalized PageRank (Andersen-Chung-Lang).
//!
//! Computes approximate PPR from a single source vertex in O(1/epsilon) time,
//! independent of graph size. The algorithm maintains two sparse vectors:
//!
//! - **estimate**: accumulated PPR values (the output).
//! - **residual**: probability mass yet to be distributed.
//!
//! At each step a vertex whose residual exceeds `epsilon * degree(u)` is
//! popped from a work-queue and its mass is "pushed" to its neighbours.
//!
//! # References
//!
//! Andersen, Chung, Lang.  *Local Graph Partitioning using PageRank Vectors.*
//! FOCS 2006.

use std::collections::VecDeque;

use crate::error::SolverError;
use crate::traits::{SolverEngine, SublinearPageRank};
use crate::types::{
    Algorithm, ComplexityClass, ComplexityEstimate, ComputeBudget, CsrMatrix, SolverResult,
    SparsityProfile,
};

// ---------------------------------------------------------------------------
// ForwardPushSolver
// ---------------------------------------------------------------------------

/// Forward Push solver for Personalized PageRank.
///
/// Given a graph encoded as a `CsrMatrix<f64>` (adjacency list in CSR
/// format), computes the PPR vector from a single source vertex.
///
/// # Parameters
///
/// - `alpha` -- teleport probability (fraction absorbed per push).
///   Default: `0.85`.
/// - `epsilon` -- push threshold.  Vertices with
///   `residual[u] > epsilon * degree(u)` are eligible for a push.  Smaller
///   values yield more accurate results at the cost of more work.
///
/// # Complexity
///
/// O(1 / epsilon) pushes in total, independent of |V| or |E|.
#[derive(Debug, Clone)]
pub struct ForwardPushSolver {
    /// Teleportation probability (alpha).
    pub alpha: f64,
    /// Approximation tolerance (epsilon).
    pub epsilon: f64,
}

impl ForwardPushSolver {
    /// Create a new forward-push solver.
    ///
    /// Parameters are validated lazily at the start of each computation
    /// (see [`validate_params`](Self::validate_params)).
    pub fn new(alpha: f64, epsilon: f64) -> Self {
        Self { alpha, epsilon }
    }

    /// Validate that `alpha` and `epsilon` are within acceptable ranges.
    ///
    /// # Errors
    ///
    /// - [`SolverError::InvalidInput`] if `alpha` is not in `(0, 1)` exclusive.
    /// - [`SolverError::InvalidInput`] if `epsilon` is not positive.
    fn validate_params(&self) -> Result<(), SolverError> {
        if self.alpha <= 0.0 || self.alpha >= 1.0 {
            return Err(SolverError::InvalidInput(
                crate::error::ValidationError::ParameterOutOfRange {
                    name: "alpha".into(),
                    value: self.alpha.to_string(),
                    expected: "(0.0, 1.0) exclusive".into(),
                },
            ));
        }
        if self.epsilon <= 0.0 {
            return Err(SolverError::InvalidInput(
                crate::error::ValidationError::ParameterOutOfRange {
                    name: "epsilon".into(),
                    value: self.epsilon.to_string(),
                    expected: "> 0.0".into(),
                },
            ));
        }
        Ok(())
    }

    /// Create a solver with default parameters (`alpha = 0.85`,
    /// `epsilon = 1e-6`).
    pub fn default_params() -> Self {
        Self {
            alpha: 0.85,
            epsilon: 1e-6,
        }
    }

    /// Compute PPR from `source` returning sparse `(vertex, score)` pairs
    /// sorted by score descending.
    ///
    /// # Errors
    ///
    /// - [`SolverError::InvalidInput`] if `source >= graph.rows`.
    /// - [`SolverError::NumericalInstability`] if the mass invariant is
    ///   violated after convergence.
    pub fn ppr_from_source(
        &self,
        graph: &CsrMatrix<f64>,
        source: usize,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        self.validate_params()?;
        validate_vertex(graph, source, "source")?;
        self.forward_push_core(graph, &[(source, 1.0)])
    }

    /// Compute PPR from `source` and return only the top-`k` entries.
    ///
    /// Convenience wrapper around [`ppr_from_source`](Self::ppr_from_source).
    pub fn top_k(
        &self,
        graph: &CsrMatrix<f64>,
        source: usize,
        k: usize,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        let mut result = self.ppr_from_source(graph, source)?;
        result.truncate(k);
        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Core push loop (Andersen-Chung-Lang)
    // -----------------------------------------------------------------------

    /// Run the forward push from a (possibly multi-seed) initial residual
    /// distribution.
    ///
    /// Uses a `VecDeque` work-queue with a membership bitvec to achieve
    /// O(1/epsilon) total work, independent of graph size.
    /// Maximum number of graph nodes to prevent OOM DoS.
    const MAX_GRAPH_NODES: usize = 100_000_000;

    fn forward_push_core(
        &self,
        graph: &CsrMatrix<f64>,
        seeds: &[(usize, f64)],
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        self.validate_params()?;

        let n = graph.rows;
        if n > Self::MAX_GRAPH_NODES {
            return Err(SolverError::InvalidInput(
                crate::error::ValidationError::MatrixTooLarge {
                    rows: n,
                    cols: graph.cols,
                    max_dim: Self::MAX_GRAPH_NODES,
                },
            ));
        }

        let mut estimate = vec![0.0f64; n];
        let mut residual = vec![0.0f64; n];

        // BFS-style work-queue with a membership bitvec.
        let mut in_queue = vec![false; n];
        let mut queue: VecDeque<usize> = VecDeque::new();

        // Initialise residuals from seed distribution.
        for &(v, mass) in seeds {
            residual[v] += mass;
            if !in_queue[v] && should_push(residual[v], graph.row_degree(v), self.epsilon) {
                queue.push_back(v);
                in_queue[v] = true;
            }
        }

        // ----- Main push loop -----
        while let Some(u) = queue.pop_front() {
            in_queue[u] = false;

            let r_u = residual[u];

            // Re-check: the residual may have decayed since enqueue.
            if !should_push(r_u, graph.row_degree(u), self.epsilon) {
                continue;
            }

            // Absorb alpha fraction into the estimate.
            estimate[u] += self.alpha * r_u;

            let degree = graph.row_degree(u);
            if degree > 0 {
                let push_amount = (1.0 - self.alpha) * r_u / degree as f64;

                // Zero out the residual at u BEFORE distributing to
                // neighbours. This is critical for self-loops: if u has an
                // edge to itself, the push_amount added via the self-loop
                // must not be overwritten.
                residual[u] = 0.0;

                for (v, _weight) in graph.row_entries(u) {
                    residual[v] += push_amount;

                    if !in_queue[v] && should_push(residual[v], graph.row_degree(v), self.epsilon) {
                        queue.push_back(v);
                        in_queue[v] = true;
                    }
                }
            } else {
                // Dangling vertex (degree 0): the (1-alpha) fraction cannot
                // be distributed to neighbours.  Keep it in the residual so
                // the mass invariant is preserved.  Re-enqueue if the
                // leftover still exceeds the push threshold, which will
                // converge geometrically since each push multiplies the
                // residual by (1-alpha).
                let leftover = (1.0 - self.alpha) * r_u;
                residual[u] = leftover;

                if !in_queue[u] && should_push(leftover, 0, self.epsilon) {
                    queue.push_back(u);
                    in_queue[u] = true;
                }
            }
        }

        // Mass invariant: sum(estimate) + sum(residual) must approximate the
        // total initial mass.
        let total_seed_mass: f64 = seeds.iter().map(|(_, m)| *m).sum();
        check_mass_invariant(&estimate, &residual, total_seed_mass)?;

        // Collect non-zero estimates into a sparse result, sorted descending.
        let mut result: Vec<(usize, f64)> = estimate
            .iter()
            .enumerate()
            .filter(|(_, val)| **val > 0.0)
            .map(|(i, val)| (i, *val))
            .collect();

        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(result)
    }
}

/// Compute the estimate and residual vectors simultaneously.
///
/// Returns `(estimate, residual)` as dense `Vec<f64>` for use by hybrid
/// random-walk algorithms that need to inspect residuals.
pub fn forward_push_with_residuals(
    matrix: &CsrMatrix<f64>,
    source: usize,
    alpha: f64,
    epsilon: f64,
) -> Result<(Vec<f64>, Vec<f64>), SolverError> {
    validate_vertex(matrix, source, "source")?;

    let n = matrix.rows;
    let mut estimate = vec![0.0f64; n];
    let mut residual = vec![0.0f64; n];

    residual[source] = 1.0;

    let mut in_queue = vec![false; n];
    let mut queue: VecDeque<usize> = VecDeque::new();

    if should_push(1.0, matrix.row_degree(source), epsilon) {
        queue.push_back(source);
        in_queue[source] = true;
    }

    while let Some(u) = queue.pop_front() {
        in_queue[u] = false;
        let r_u = residual[u];

        if !should_push(r_u, matrix.row_degree(u), epsilon) {
            continue;
        }

        estimate[u] += alpha * r_u;

        let degree = matrix.row_degree(u);
        if degree > 0 {
            let push_amount = (1.0 - alpha) * r_u / degree as f64;
            // Zero before distributing (self-loop safety).
            residual[u] = 0.0;
            for (v, _) in matrix.row_entries(u) {
                residual[v] += push_amount;
                if !in_queue[v] && should_push(residual[v], matrix.row_degree(v), epsilon) {
                    queue.push_back(v);
                    in_queue[v] = true;
                }
            }
        } else {
            // Dangling vertex: keep (1-alpha) portion as residual.
            let leftover = (1.0 - alpha) * r_u;
            residual[u] = leftover;
            if !in_queue[u] && should_push(leftover, 0, epsilon) {
                queue.push_back(u);
                in_queue[u] = true;
            }
        }
    }

    Ok((estimate, residual))
}

// ---------------------------------------------------------------------------
// Free-standing helpers
// ---------------------------------------------------------------------------

/// Whether a vertex with the given `residual` and `degree` should be pushed.
///
/// For isolated vertices (degree 0) we use a fallback threshold of `epsilon`
/// to avoid infinite loops while still absorbing meaningful residual.
#[inline]
fn should_push(residual: f64, degree: usize, epsilon: f64) -> bool {
    if degree == 0 {
        residual > epsilon
    } else {
        residual > epsilon * degree as f64
    }
}

/// Validate that a vertex index is within bounds.
fn validate_vertex(graph: &CsrMatrix<f64>, vertex: usize, name: &str) -> Result<(), SolverError> {
    if vertex >= graph.rows {
        return Err(SolverError::InvalidInput(
            crate::error::ValidationError::ParameterOutOfRange {
                name: name.into(),
                value: vertex.to_string(),
                expected: format!("0..{}", graph.rows),
            },
        ));
    }
    Ok(())
}

/// Verify the mass invariant: `sum(estimate) + sum(residual) ~ expected`.
fn check_mass_invariant(
    estimate: &[f64],
    residual: &[f64],
    expected_mass: f64,
) -> Result<(), SolverError> {
    let mass: f64 = estimate.iter().sum::<f64>() + residual.iter().sum::<f64>();
    if (mass - expected_mass).abs() > 1e-6 {
        return Err(SolverError::NumericalInstability {
            iteration: 0,
            detail: format!(
                "mass invariant violated: sum(estimate)+sum(residual) = {mass:.10}, \
                 expected {expected_mass:.10}",
            ),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// SolverEngine trait implementation
// ---------------------------------------------------------------------------

impl SolverEngine for ForwardPushSolver {
    /// Adapt forward-push PPR to the generic solver interface.
    ///
    /// The `rhs` vector is interpreted as a source indicator: the index of
    /// the first non-zero entry is taken as the source vertex. If `rhs` is
    /// all zeros, vertex 0 is used. The returned `SolverResult.solution`
    /// contains the dense PPR vector.
    fn solve(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        _budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        let start = std::time::Instant::now();

        let source = rhs.iter().position(|&v| v != 0.0).unwrap_or(0);
        let sparse_result = self.ppr_from_source(matrix, source)?;

        let n = matrix.rows;
        let mut solution = vec![0.0f32; n];
        for &(idx, score) in &sparse_result {
            solution[idx] = score as f32;
        }

        Ok(SolverResult {
            solution,
            iterations: sparse_result.len(),
            residual_norm: 0.0,
            wall_time: start.elapsed(),
            convergence_history: Vec::new(),
            algorithm: Algorithm::ForwardPush,
        })
    }

    fn estimate_complexity(&self, _profile: &SparsityProfile, _n: usize) -> ComplexityEstimate {
        let est_ops = (1.0 / self.epsilon).min(usize::MAX as f64) as usize;
        ComplexityEstimate {
            algorithm: Algorithm::ForwardPush,
            estimated_flops: est_ops as u64 * 10,
            estimated_iterations: est_ops,
            estimated_memory_bytes: est_ops * 16,
            complexity_class: ComplexityClass::SublinearNnz,
        }
    }

    fn algorithm(&self) -> Algorithm {
        Algorithm::ForwardPush
    }
}

// ---------------------------------------------------------------------------
// SublinearPageRank trait implementation
// ---------------------------------------------------------------------------

impl SublinearPageRank for ForwardPushSolver {
    fn ppr(
        &self,
        matrix: &CsrMatrix<f64>,
        source: usize,
        alpha: f64,
        epsilon: f64,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        let solver = ForwardPushSolver::new(alpha, epsilon);
        solver.ppr_from_source(matrix, source)
    }

    fn ppr_multi_seed(
        &self,
        matrix: &CsrMatrix<f64>,
        seeds: &[(usize, f64)],
        alpha: f64,
        epsilon: f64,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        for &(v, _) in seeds {
            validate_vertex(matrix, v, "seed vertex")?;
        }
        let solver = ForwardPushSolver::new(alpha, epsilon);
        solver.forward_push_core(matrix, seeds)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Kahan (compensated) summation accumulator (test-only).
    #[derive(Debug, Clone, Copy)]
    struct KahanAccumulator {
        sum: f64,
        compensation: f64,
    }

    impl KahanAccumulator {
        #[inline]
        const fn new() -> Self {
            Self {
                sum: 0.0,
                compensation: 0.0,
            }
        }

        #[inline]
        fn add(&mut self, value: f64) {
            let y = value - self.compensation;
            let t = self.sum + y;
            self.compensation = (t - self.sum) - y;
            self.sum = t;
        }

        #[inline]
        fn value(&self) -> f64 {
            self.sum
        }
    }

    /// 4-vertex graph with bidirectional edges:
    ///   0 -- 1, 0 -- 2, 1 -- 2, 1 -- 3
    fn triangle_graph() -> CsrMatrix<f64> {
        CsrMatrix::<f64>::from_coo(
            4,
            4,
            vec![
                (0, 1, 1.0f64),
                (0, 2, 1.0f64),
                (1, 0, 1.0f64),
                (1, 2, 1.0f64),
                (1, 3, 1.0f64),
                (2, 0, 1.0f64),
                (2, 1, 1.0f64),
                (3, 1, 1.0f64),
            ],
        )
    }

    /// Directed path: 0 -> 1 -> 2 -> 3
    fn path_graph() -> CsrMatrix<f64> {
        CsrMatrix::<f64>::from_coo(4, 4, vec![(0, 1, 1.0f64), (1, 2, 1.0f64), (2, 3, 1.0f64)])
    }

    /// Star graph centred at vertex 0 with 5 leaves, bidirectional.
    fn star_graph() -> CsrMatrix<f64> {
        let n = 6;
        let mut entries = Vec::new();
        for leaf in 1..n {
            entries.push((0, leaf, 1.0f64));
            entries.push((leaf, 0, 1.0f64));
        }
        CsrMatrix::<f64>::from_coo(n, n, entries)
    }

    #[test]
    fn basic_ppr_triangle() {
        let graph = triangle_graph();
        let solver = ForwardPushSolver::default_params();
        let result = solver.ppr_from_source(&graph, 0).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result[0].0, 0, "source should be top-ranked");
        assert!(result[0].1 > 0.0);

        for &(_, score) in &result {
            assert!(score > 0.0);
        }

        for w in result.windows(2) {
            assert!(w[0].1 >= w[1].1, "results should be sorted descending");
        }
    }

    #[test]
    fn ppr_path_graph_monotone_decay() {
        let graph = path_graph();
        let solver = ForwardPushSolver::new(0.85, 1e-8);
        let result = solver.ppr_from_source(&graph, 0).unwrap();

        let mut scores = vec![0.0f64; 4];
        for &(v, s) in &result {
            scores[v] = s;
        }
        assert!(scores[0] > scores[1], "score[0] > score[1]");
        assert!(scores[1] > scores[2], "score[1] > score[2]");
        assert!(scores[2] > scores[3], "score[2] > score[3]");
    }

    #[test]
    fn ppr_star_symmetry() {
        let graph = star_graph();
        let solver = ForwardPushSolver::new(0.85, 1e-8);
        let result = solver.ppr_from_source(&graph, 0).unwrap();

        let leaf_scores: Vec<f64> = result
            .iter()
            .filter(|(v, _)| *v != 0)
            .map(|(_, s)| *s)
            .collect();
        assert_eq!(leaf_scores.len(), 5);

        let mean = leaf_scores.iter().sum::<f64>() / leaf_scores.len() as f64;
        for &s in &leaf_scores {
            assert!(
                (s - mean).abs() < 1e-6,
                "leaf scores should be equal: got {s} vs mean {mean}",
            );
        }
    }

    #[test]
    fn top_k_truncates() {
        let graph = triangle_graph();
        let solver = ForwardPushSolver::default_params();
        let result = solver.top_k(&graph, 0, 2).unwrap();

        assert!(result.len() <= 2);
        assert_eq!(result[0].0, 0);
    }

    #[test]
    fn mass_invariant_holds() {
        let graph = triangle_graph();
        let solver = ForwardPushSolver::default_params();
        assert!(solver.ppr_from_source(&graph, 0).is_ok());
    }

    #[test]
    fn invalid_source_errors() {
        let graph = triangle_graph();
        let solver = ForwardPushSolver::default_params();
        assert!(solver.ppr_from_source(&graph, 100).is_err());
    }

    #[test]
    fn isolated_vertex_receives_zero() {
        // Vertex 3 has no edges.
        let graph = CsrMatrix::<f64>::from_coo(
            4,
            4,
            vec![
                (0, 1, 1.0f64),
                (1, 0, 1.0f64),
                (1, 2, 1.0f64),
                (2, 1, 1.0f64),
            ],
        );
        let solver = ForwardPushSolver::default_params();
        let result = solver.ppr_from_source(&graph, 0).unwrap();

        let v3_score = result.iter().find(|(v, _)| *v == 3).map_or(0.0, |p| p.1);
        assert!(
            v3_score.abs() < 1e-10,
            "isolated vertex should have ~zero PPR",
        );
    }

    #[test]
    fn isolated_source_converges_to_one() {
        // An isolated vertex (degree 0) keeps pushing until residual drops
        // below epsilon.  The estimate converges to
        // 1 - (1-alpha)^k ~ 1.0 for small epsilon.
        let graph = CsrMatrix::<f64>::from_coo(
            4,
            4,
            vec![
                (0, 1, 1.0f64),
                (1, 0, 1.0f64),
                (1, 2, 1.0f64),
                (2, 1, 1.0f64),
            ],
        );
        let solver = ForwardPushSolver::default_params();
        let result = solver.ppr_from_source(&graph, 3).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 3);
        // With alpha=0.85 and epsilon=1e-6, the estimate converges very
        // close to 1.0 (within epsilon).
        assert!(
            (result[0].1 - 1.0).abs() < 1e-4,
            "isolated source estimate should converge near 1.0: got {}",
            result[0].1,
        );
    }

    #[test]
    fn single_vertex_graph() {
        let graph = CsrMatrix::<f64>::from_coo(1, 1, Vec::<(usize, usize, f64)>::new());
        let solver = ForwardPushSolver::default_params();
        let result = solver.ppr_from_source(&graph, 0).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 0);
        // Single isolated vertex converges to ~1.0 (not 0.85) because the
        // dangling node keeps absorbing alpha on each push iteration.
        assert!(
            (result[0].1 - 1.0).abs() < 1e-4,
            "single vertex PPR should converge near 1.0: got {}",
            result[0].1,
        );
    }

    #[test]
    fn solver_engine_trait() {
        let graph = triangle_graph();
        let solver = ForwardPushSolver::default_params();

        let mut rhs = vec![0.0f64; 4];
        rhs[1] = 1.0;
        let budget = ComputeBudget::default();

        let result = solver.solve(&graph, &rhs, &budget).unwrap();
        assert_eq!(result.algorithm, Algorithm::ForwardPush);
        assert_eq!(result.solution.len(), 4);

        let max_idx = result
            .solution
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap()
            .0;
        assert_eq!(max_idx, 1);
    }

    #[test]
    fn sublinear_ppr_trait() {
        let graph = triangle_graph();
        let solver = ForwardPushSolver::default_params();
        let result = solver.ppr(&graph, 0, 0.85, 1e-6).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result[0].0, 0, "source should rank first via ppr trait");
    }

    #[test]
    fn multi_seed_ppr() {
        let graph = triangle_graph();
        let solver = ForwardPushSolver::default_params();

        let seeds = vec![(0, 0.5), (1, 0.5)];
        let result = solver.ppr_multi_seed(&graph, &seeds, 0.85, 1e-6).unwrap();

        assert!(!result.is_empty());
        let has_0 = result.iter().any(|(v, _)| *v == 0);
        let has_1 = result.iter().any(|(v, _)| *v == 1);
        assert!(has_0 && has_1, "both seeds should appear in output");
    }

    #[test]
    fn forward_push_with_residuals_mass_conservation() {
        let graph = triangle_graph();
        let (p, r) = forward_push_with_residuals(&graph, 0, 0.85, 1e-6).unwrap();

        let total: f64 = p.iter().sum::<f64>() + r.iter().sum::<f64>();
        assert!(
            (total - 1.0).abs() < 1e-6,
            "mass should be conserved: got {total}",
        );
    }

    #[test]
    fn kahan_accuracy() {
        let mut acc = KahanAccumulator::new();
        let n = 1_000_000;
        let small = 1e-10;
        for _ in 0..n {
            acc.add(small);
        }
        let expected = n as f64 * small;
        let relative_error = (acc.value() - expected).abs() / expected;
        assert!(
            relative_error < 1e-10,
            "Kahan relative error {relative_error} should be tiny",
        );
    }

    #[test]
    fn self_loop_graph() {
        let graph = CsrMatrix::<f64>::from_coo(
            3,
            3,
            vec![
                (0, 0, 1.0f64),
                (0, 1, 1.0f64),
                (1, 1, 1.0f64),
                (1, 2, 1.0f64),
                (2, 2, 1.0f64),
                (2, 0, 1.0f64),
            ],
        );
        let solver = ForwardPushSolver::default_params();
        let result = solver.ppr_from_source(&graph, 0);
        assert!(result.is_ok(), "self-loop graph failed: {:?}", result.err());
    }

    #[test]
    fn complete_graph_symmetry() {
        let n = 4;
        let mut entries = Vec::new();
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    entries.push((i, j, 1.0f64));
                }
            }
        }
        let graph = CsrMatrix::<f64>::from_coo(n, n, entries);
        let solver = ForwardPushSolver::new(0.85, 1e-8);
        let result = solver.ppr_from_source(&graph, 0).unwrap();

        assert_eq!(result[0].0, 0);

        let other_scores: Vec<f64> = result
            .iter()
            .filter(|(v, _)| *v != 0)
            .map(|(_, s)| *s)
            .collect();
        assert_eq!(other_scores.len(), 3);
        let mean = other_scores.iter().sum::<f64>() / 3.0;
        for &s in &other_scores {
            assert!((s - mean).abs() < 1e-6);
        }
    }

    #[test]
    fn estimate_complexity_sublinear() {
        let solver = ForwardPushSolver::new(0.85, 1e-4);
        let profile = SparsityProfile {
            rows: 1000,
            cols: 1000,
            nnz: 5000,
            density: 0.005,
            is_diag_dominant: false,
            estimated_spectral_radius: 0.9,
            estimated_condition: 10.0,
            is_symmetric_structure: true,
            avg_nnz_per_row: 5.0,
            max_nnz_per_row: 10,
        };
        let est = solver.estimate_complexity(&profile, 1000);
        assert_eq!(est.algorithm, Algorithm::ForwardPush);
        assert_eq!(est.complexity_class, ComplexityClass::SublinearNnz);
        assert!(est.estimated_iterations > 0);
    }
}
