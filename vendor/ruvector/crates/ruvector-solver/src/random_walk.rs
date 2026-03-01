//! Hybrid Random Walk Monte Carlo for Personalized PageRank estimation.
//!
//! Estimates pairwise PPR(s, t) via random walks. Each walk starts at the
//! source vertex and at each step either teleports (with probability alpha)
//! or moves to a random neighbour (with probability 1 - alpha). The fraction
//! of walks landing at the target approximates PPR(s, t).
//!
//! # Variance tracking
//!
//! Uses Welford's online algorithm to track the mean and variance of the
//! binary indicator `I[walk lands at target]`. Early termination triggers
//! when the coefficient of variation (CV = stddev / mean) drops below 0.1.
//!
//! # Complexity
//!
//! Each walk has expected length `1/alpha`. For single-entry estimation
//! with additive error epsilon and failure probability delta, `num_walks =
//! ceil(3 * ln(2/delta) / epsilon^2)` suffices. Total work:
//! `O(num_walks / alpha)`.

use std::time::Instant;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tracing::debug;

use crate::error::{SolverError, ValidationError};
use crate::traits::{SolverEngine, SublinearPageRank};
use crate::types::{
    Algorithm, ComplexityClass, ComplexityEstimate, ComputeBudget, ConvergenceInfo, CsrMatrix,
    SolverResult, SparsityProfile,
};

// ---------------------------------------------------------------------------
// Welford's online variance tracker
// ---------------------------------------------------------------------------

/// Tracks running mean and variance via Welford's numerically stable
/// online algorithm. Used for early-termination decisions.
#[derive(Debug, Clone)]
struct WelfordAccumulator {
    count: u64,
    mean: f64,
    m2: f64,
}

impl WelfordAccumulator {
    fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    #[inline]
    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    #[inline]
    fn variance(&self) -> f64 {
        if self.count < 2 {
            return f64::INFINITY;
        }
        self.m2 / self.count as f64
    }

    #[inline]
    fn stddev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Coefficient of variation: stddev / |mean|.
    #[inline]
    fn cv(&self) -> f64 {
        if self.mean.abs() < 1e-15 {
            return f64::INFINITY;
        }
        self.stddev() / self.mean.abs()
    }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default failure probability for walk-count formula.
const DEFAULT_DELTA: f64 = 0.01;

/// CV threshold for early termination.
const CV_THRESHOLD: f64 = 0.1;

/// Minimum walks before checking CV.
const MIN_WALKS_BEFORE_CV_CHECK: usize = 100;

// ---------------------------------------------------------------------------
// Solver struct
// ---------------------------------------------------------------------------

/// Hybrid random-walk PPR solver.
///
/// Performs random walks from the source node, each terminating with
/// probability `alpha` at each step. The empirical distribution over
/// walk endpoints approximates the PPR vector.
///
/// # Example
///
/// ```rust,ignore
/// use ruvector_solver::random_walk::HybridRandomWalkSolver;
/// use ruvector_solver::types::CsrMatrix;
///
/// let graph = CsrMatrix::<f64>::from_coo(4, 4, vec![
///     (0, 1, 1.0), (1, 2, 1.0), (2, 3, 1.0), (3, 0, 1.0),
/// ]);
/// let solver = HybridRandomWalkSolver::new(0.15, 10_000);
/// let ppr_01 = solver.estimate_entry(&graph, 0, 1).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct HybridRandomWalkSolver {
    /// Teleportation probability (alpha). Must be in (0, 1).
    pub alpha: f64,
    /// Number of random walks to simulate.
    pub num_walks: usize,
    /// Random seed for reproducibility (0 = use entropy source).
    pub seed: u64,
}

impl HybridRandomWalkSolver {
    /// Create a new hybrid random-walk solver.
    pub fn new(alpha: f64, num_walks: usize) -> Self {
        Self {
            alpha,
            num_walks,
            seed: 0,
        }
    }

    /// Create a solver calibrated for additive error `epsilon` with
    /// failure probability `delta`.
    ///
    /// Formula: `num_walks = ceil(3 * ln(2/delta) / epsilon^2)`.
    pub fn from_epsilon(alpha: f64, epsilon: f64, delta: f64) -> Self {
        let num_walks = Self::walks_for_epsilon(epsilon, delta);
        Self {
            alpha,
            num_walks,
            seed: 0,
        }
    }

    /// Number of walks for additive error `epsilon` and failure
    /// probability `delta` (Chernoff-style bound).
    pub fn walks_for_epsilon(epsilon: f64, delta: f64) -> usize {
        let eps = epsilon.max(1e-10);
        let d = delta.max(1e-15);
        ((3.0 * (2.0 / d).ln()) / (eps * eps)).ceil() as usize
    }

    /// Set the random seed for reproducible results.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    fn make_rng(&self) -> StdRng {
        if self.seed == 0 {
            StdRng::from_entropy()
        } else {
            StdRng::seed_from_u64(self.seed)
        }
    }

    fn validate_params(&self) -> Result<(), SolverError> {
        if self.alpha <= 0.0 || self.alpha >= 1.0 {
            return Err(SolverError::InvalidInput(
                ValidationError::ParameterOutOfRange {
                    name: "alpha".into(),
                    value: self.alpha.to_string(),
                    expected: "(0.0, 1.0) exclusive".into(),
                },
            ));
        }
        if self.num_walks == 0 {
            return Err(SolverError::InvalidInput(
                ValidationError::ParameterOutOfRange {
                    name: "num_walks".into(),
                    value: "0".into(),
                    expected: "> 0".into(),
                },
            ));
        }
        Ok(())
    }

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

    // -----------------------------------------------------------------------
    // Core walk simulation
    // -----------------------------------------------------------------------

    /// Simulate a single random walk from `start`. Returns the endpoint.
    #[inline]
    fn single_walk(graph: &CsrMatrix<f64>, start: usize, alpha: f64, rng: &mut StdRng) -> usize {
        let mut current = start;
        loop {
            if rng.gen::<f64>() < alpha {
                return current;
            }
            let degree = graph.row_degree(current);
            if degree == 0 {
                return current; // dangling node
            }
            let row_start = graph.row_ptr[current];
            current = graph.col_indices[row_start + rng.gen_range(0..degree)];
        }
    }

    // -----------------------------------------------------------------------
    // Public estimation methods
    // -----------------------------------------------------------------------

    /// Estimate PPR(source, target) via random walks with Welford
    /// variance tracking and early termination.
    pub fn estimate_entry(
        &self,
        graph: &CsrMatrix<f64>,
        source: usize,
        target: usize,
    ) -> Result<f64, SolverError> {
        self.validate_params()?;
        Self::validate_graph_node(graph, source, "source")?;
        Self::validate_graph_node(graph, target, "target")?;

        let mut rng = self.make_rng();
        let mut welford = WelfordAccumulator::new();
        let mut hit_count = 0u64;

        for w in 0..self.num_walks {
            let endpoint = Self::single_walk(graph, source, self.alpha, &mut rng);
            let indicator = if endpoint == target { 1.0 } else { 0.0 };
            welford.update(indicator);
            if endpoint == target {
                hit_count += 1;
            }

            if w >= MIN_WALKS_BEFORE_CV_CHECK && welford.cv() < CV_THRESHOLD {
                debug!(
                    target: "ruvector_solver::random_walk",
                    walks = w + 1,
                    cv = welford.cv(),
                    "early termination: CV below threshold",
                );
                return Ok(hit_count as f64 / (w + 1) as f64);
            }
        }

        Ok(hit_count as f64 / self.num_walks as f64)
    }

    /// Batch estimation of PPR(source, target) for multiple pairs.
    pub fn estimate_batch(
        &self,
        graph: &CsrMatrix<f64>,
        pairs: &[(usize, usize)],
    ) -> Result<Vec<f64>, SolverError> {
        self.validate_params()?;
        for &(s, t) in pairs {
            Self::validate_graph_node(graph, s, "source")?;
            Self::validate_graph_node(graph, t, "target")?;
        }

        let mut rng = self.make_rng();
        let mut results = Vec::with_capacity(pairs.len());

        for &(source, target) in pairs {
            let mut welford = WelfordAccumulator::new();
            let mut hit_count = 0u64;
            let mut completed = self.num_walks;

            for w in 0..self.num_walks {
                let endpoint = Self::single_walk(graph, source, self.alpha, &mut rng);
                welford.update(if endpoint == target { 1.0 } else { 0.0 });
                if endpoint == target {
                    hit_count += 1;
                }
                if w >= MIN_WALKS_BEFORE_CV_CHECK && welford.cv() < CV_THRESHOLD {
                    completed = w + 1;
                    break;
                }
            }

            results.push(hit_count as f64 / completed as f64);
        }

        Ok(results)
    }

    /// Compute a full approximate PPR vector from `source`.
    pub fn ppr_from_source(
        &self,
        graph: &CsrMatrix<f64>,
        source: usize,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        self.ppr_from_source_with_params(graph, source, self.alpha, self.num_walks)
    }

    fn ppr_from_source_with_params(
        &self,
        graph: &CsrMatrix<f64>,
        source: usize,
        alpha: f64,
        num_walks: usize,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        Self::validate_graph_node(graph, source, "source")?;

        #[cfg(feature = "parallel")]
        {
            return self.ppr_from_source_parallel(graph, source, alpha, num_walks);
        }

        #[cfg(not(feature = "parallel"))]
        {
            let mut rng = self.make_rng();
            let mut counts = vec![0u64; graph.rows];

            for _ in 0..num_walks {
                let endpoint = Self::single_walk(graph, source, alpha, &mut rng);
                counts[endpoint] += 1;
            }

            let inv = 1.0 / num_walks as f64;
            let mut result: Vec<(usize, f64)> = counts
                .into_iter()
                .enumerate()
                .filter(|(_, c)| *c > 0)
                .map(|(v, c)| (v, c as f64 * inv))
                .collect();
            result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            Ok(result)
        }
    }

    #[cfg(feature = "parallel")]
    fn ppr_from_source_parallel(
        &self,
        graph: &CsrMatrix<f64>,
        source: usize,
        alpha: f64,
        num_walks: usize,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        use rayon::prelude::*;

        let n = graph.rows;

        // Split walks across threads, each with its own RNG derived from the base seed.
        let num_chunks = rayon::current_num_threads().max(1);
        let walks_per_chunk = num_walks / num_chunks;
        let remainder = num_walks % num_chunks;

        let counts: Vec<u64> = (0..num_chunks)
            .into_par_iter()
            .map(|chunk_idx| {
                // Derive a per-chunk seed from the base seed.
                let chunk_seed = if self.seed == 0 {
                    chunk_idx as u64 + 1
                } else {
                    self.seed.wrapping_add(chunk_idx as u64 * 1000003)
                };
                let mut rng = StdRng::seed_from_u64(chunk_seed);

                let chunk_walks = walks_per_chunk + if chunk_idx < remainder { 1 } else { 0 };
                let mut local_counts = vec![0u64; n];

                for _ in 0..chunk_walks {
                    let endpoint = Self::single_walk(graph, source, alpha, &mut rng);
                    local_counts[endpoint] += 1;
                }

                local_counts
            })
            .reduce(
                || vec![0u64; n],
                |mut a, b| {
                    for (i, &v) in b.iter().enumerate() {
                        a[i] += v;
                    }
                    a
                },
            );

        let inv = 1.0 / num_walks as f64;
        let mut result: Vec<(usize, f64)> = counts
            .into_iter()
            .enumerate()
            .filter(|(_, c)| *c > 0)
            .map(|(v, c)| (v, c as f64 * inv))
            .collect();
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// SolverEngine
// ---------------------------------------------------------------------------

impl SolverEngine for HybridRandomWalkSolver {
    fn solve(
        &self,
        matrix: &CsrMatrix<f64>,
        rhs: &[f64],
        budget: &ComputeBudget,
    ) -> Result<SolverResult, SolverError> {
        let n = matrix.rows;
        if n != matrix.cols {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch(format!(
                    "HybridRandomWalk requires square matrix, got {}x{}",
                    matrix.rows, matrix.cols,
                )),
            ));
        }
        if rhs.len() != n {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch(format!(
                    "rhs length {} != matrix rows {}",
                    rhs.len(),
                    n,
                )),
            ));
        }
        if n == 0 {
            return Err(SolverError::InvalidInput(
                ValidationError::DimensionMismatch("empty matrix".into()),
            ));
        }

        let start_time = Instant::now();

        // Interpret rhs as a source distribution.
        let rhs_sum: f64 = rhs.iter().map(|v| v.abs()).sum();
        if rhs_sum < 1e-30 {
            return Ok(SolverResult {
                solution: vec![0.0f32; n],
                iterations: 0,
                residual_norm: 0.0,
                wall_time: start_time.elapsed(),
                convergence_history: vec![],
                algorithm: Algorithm::HybridRandomWalk,
            });
        }

        // Build CDF for source distribution.
        let mut cdf = Vec::with_capacity(n);
        let mut cumulative = 0.0;
        for val in rhs.iter() {
            cumulative += val.abs() / rhs_sum;
            cdf.push(cumulative);
        }

        let walks = self.num_walks.min(budget.max_iterations.saturating_mul(10));

        #[cfg(feature = "parallel")]
        let counts = {
            use rayon::prelude::*;

            let num_chunks = rayon::current_num_threads().max(1);
            let walks_per_chunk = walks / num_chunks;
            let remainder = walks % num_chunks;

            (0..num_chunks)
                .into_par_iter()
                .map(|chunk_idx| {
                    let chunk_seed = if self.seed == 0 {
                        chunk_idx as u64 + 1
                    } else {
                        self.seed.wrapping_add(chunk_idx as u64 * 1000003)
                    };
                    let mut rng = StdRng::seed_from_u64(chunk_seed);
                    let chunk_walks = walks_per_chunk + if chunk_idx < remainder { 1 } else { 0 };
                    let mut local_counts = vec![0.0f64; n];

                    for _ in 0..chunk_walks {
                        let r: f64 = rng.gen();
                        let start_node = cdf.partition_point(|&c| c < r).min(n - 1);
                        let endpoint = Self::single_walk(matrix, start_node, self.alpha, &mut rng);
                        local_counts[endpoint] += 1.0;
                    }
                    local_counts
                })
                .reduce(
                    || vec![0.0f64; n],
                    |mut a, b| {
                        for (i, &v) in b.iter().enumerate() {
                            a[i] += v;
                        }
                        a
                    },
                )
        };

        #[cfg(not(feature = "parallel"))]
        let counts = {
            let mut rng = self.make_rng();
            let mut counts = vec![0.0f64; n];
            for _ in 0..walks {
                if start_time.elapsed() > budget.max_time {
                    return Err(SolverError::BudgetExhausted {
                        reason: "wall-clock time limit exceeded".into(),
                        elapsed: start_time.elapsed(),
                    });
                }

                let r: f64 = rng.gen();
                let start_node = cdf.partition_point(|&c| c < r).min(n - 1);
                let endpoint = Self::single_walk(matrix, start_node, self.alpha, &mut rng);
                counts[endpoint] += 1.0;
            }
            counts
        };

        let scale = rhs_sum / (walks as f64);
        let solution: Vec<f32> = counts.iter().map(|&c| (c * scale) as f32).collect();

        // Compute residual: r = b - Ax.
        let sol_f64: Vec<f64> = solution.iter().map(|&v| v as f64).collect();
        let mut ax = vec![0.0f64; n];
        matrix.spmv(&sol_f64, &mut ax);
        let residual_norm = rhs
            .iter()
            .zip(ax.iter())
            .map(|(&b, &a)| (b - a) * (b - a))
            .sum::<f64>()
            .sqrt();

        Ok(SolverResult {
            solution,
            iterations: walks,
            residual_norm,
            wall_time: start_time.elapsed(),
            convergence_history: vec![ConvergenceInfo {
                iteration: 0,
                residual_norm,
            }],
            algorithm: Algorithm::HybridRandomWalk,
        })
    }

    fn estimate_complexity(&self, _profile: &SparsityProfile, _n: usize) -> ComplexityEstimate {
        let avg_walk_len = (1.0 / self.alpha).ceil() as u64;
        ComplexityEstimate {
            algorithm: Algorithm::HybridRandomWalk,
            estimated_flops: self.num_walks as u64 * avg_walk_len * 2,
            estimated_iterations: self.num_walks,
            estimated_memory_bytes: self.num_walks * 8,
            complexity_class: ComplexityClass::SublinearNnz,
        }
    }

    fn algorithm(&self) -> Algorithm {
        Algorithm::HybridRandomWalk
    }
}

// ---------------------------------------------------------------------------
// SublinearPageRank
// ---------------------------------------------------------------------------

impl SublinearPageRank for HybridRandomWalkSolver {
    fn ppr(
        &self,
        matrix: &CsrMatrix<f64>,
        source: usize,
        alpha: f64,
        epsilon: f64,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        Self::validate_graph_node(matrix, source, "source")?;

        let num_walks = Self::walks_for_epsilon(epsilon, DEFAULT_DELTA).max(self.num_walks);
        let solver = HybridRandomWalkSolver {
            alpha,
            num_walks,
            seed: self.seed,
        };
        solver.ppr_from_source_with_params(matrix, source, alpha, num_walks)
    }

    fn ppr_multi_seed(
        &self,
        matrix: &CsrMatrix<f64>,
        seeds: &[(usize, f64)],
        alpha: f64,
        epsilon: f64,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        for &(s, _) in seeds {
            Self::validate_graph_node(matrix, s, "seed")?;
        }

        let n = matrix.rows;
        let num_walks = Self::walks_for_epsilon(epsilon, DEFAULT_DELTA).max(self.num_walks);

        // Build CDF over seed weights.
        let weight_sum: f64 = seeds.iter().map(|(_, w)| w.abs()).sum();
        if weight_sum < 1e-30 {
            return Ok(Vec::new());
        }

        let mut cdf = Vec::with_capacity(seeds.len());
        let mut cumulative = 0.0;
        for &(_, w) in seeds {
            cumulative += w.abs() / weight_sum;
            cdf.push(cumulative);
        }

        let mut rng = self.make_rng();
        let mut counts = vec![0u64; n];

        for _ in 0..num_walks {
            let r: f64 = rng.gen();
            let seed_idx = cdf.partition_point(|&c| c < r).min(seeds.len() - 1);
            let start = seeds[seed_idx].0;

            let endpoint = Self::single_walk(matrix, start, alpha, &mut rng);
            counts[endpoint] += 1;
        }

        let inv = 1.0 / num_walks as f64;
        let mut result: Vec<(usize, f64)> = counts
            .into_iter()
            .enumerate()
            .filter(|(_, c)| *c > 0)
            .map(|(v, c)| (v, c as f64 * inv))
            .collect();
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn directed_cycle(n: usize) -> CsrMatrix<f64> {
        let entries: Vec<_> = (0..n).map(|i| (i, (i + 1) % n, 1.0f64)).collect();
        CsrMatrix::<f64>::from_coo(n, n, entries)
    }

    fn star_to_center(n: usize) -> CsrMatrix<f64> {
        let entries: Vec<_> = (1..n).map(|i| (i, 0, 1.0f64)).collect();
        CsrMatrix::<f64>::from_coo(n, n, entries)
    }

    fn undirected_chain(n: usize) -> CsrMatrix<f64> {
        let mut entries = Vec::new();
        for i in 0..n {
            let next = (i + 1) % n;
            entries.push((i, next, 1.0f64));
            entries.push((next, i, 1.0f64));
        }
        CsrMatrix::<f64>::from_coo(n, n, entries)
    }

    // ---- Welford ----

    #[test]
    fn welford_constant() {
        let mut w = WelfordAccumulator::new();
        for _ in 0..100 {
            w.update(5.0);
        }
        assert!((w.mean - 5.0).abs() < 1e-12);
        assert!(w.variance() < 1e-12);
    }

    #[test]
    fn welford_binary() {
        let mut w = WelfordAccumulator::new();
        for i in 0..100 {
            w.update(if i < 50 { 1.0 } else { 0.0 });
        }
        assert!((w.mean - 0.5).abs() < 1e-12);
        assert!((w.variance() - 0.25).abs() < 0.01);
    }

    // ---- walks_for_epsilon ----

    #[test]
    fn walks_formula_reasonable() {
        let w = HybridRandomWalkSolver::walks_for_epsilon(0.01, 0.01);
        assert!(w > 100_000 && w < 500_000);
    }

    // ---- single_walk ----

    #[test]
    fn walk_single_node() {
        let g = CsrMatrix::<f64>::from_coo(1, 1, Vec::<(usize, usize, f64)>::new());
        let mut rng = StdRng::seed_from_u64(42);
        assert_eq!(
            HybridRandomWalkSolver::single_walk(&g, 0, 0.15, &mut rng),
            0
        );
    }

    #[test]
    fn walk_high_alpha_stays_at_start() {
        let g = directed_cycle(5);
        let mut rng = StdRng::seed_from_u64(42);
        assert_eq!(
            HybridRandomWalkSolver::single_walk(&g, 2, 0.9999, &mut rng),
            2,
        );
    }

    // ---- estimate_entry ----

    #[test]
    fn entry_self_single_node() {
        let g = CsrMatrix::<f64>::from_coo(1, 1, Vec::<(usize, usize, f64)>::new());
        let s = HybridRandomWalkSolver::new(0.15, 1000).with_seed(42);
        assert!((s.estimate_entry(&g, 0, 0).unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn entry_cycle_self_ppr() {
        let g = directed_cycle(4);
        let s = HybridRandomWalkSolver::new(0.15, 50_000).with_seed(123);
        let p = s.estimate_entry(&g, 0, 0).unwrap();
        assert!(p > 0.05 && p < 1.0, "ppr(0,0)={}", p);
    }

    #[test]
    fn entry_star_to_center() {
        let g = star_to_center(5);
        let s = HybridRandomWalkSolver::new(0.15, 50_000).with_seed(99);
        let p = s.estimate_entry(&g, 1, 0).unwrap();
        assert!(p > 0.5, "expected > 0.5, got {}", p);
    }

    // ---- estimate_batch ----

    #[test]
    fn batch_non_negative() {
        let g = directed_cycle(4);
        let s = HybridRandomWalkSolver::new(0.15, 10_000).with_seed(42);
        let b = s.estimate_batch(&g, &[(0, 0), (0, 1), (0, 2)]).unwrap();
        assert_eq!(b.len(), 3);
        assert!(b.iter().all(|&v| v >= 0.0));
    }

    // ---- ppr_from_source ----

    #[test]
    fn ppr_sums_to_one() {
        let g = directed_cycle(5);
        let s = HybridRandomWalkSolver::new(0.15, 50_000).with_seed(77);
        let ppr = s.ppr_from_source(&g, 0).unwrap();
        let total: f64 = ppr.iter().map(|(_, v)| v).sum();
        assert!((total - 1.0).abs() < 0.05, "sum={}", total);
    }

    #[test]
    fn ppr_sorted_descending() {
        let g = directed_cycle(5);
        let s = HybridRandomWalkSolver::new(0.15, 50_000).with_seed(88);
        let ppr = s.ppr_from_source(&g, 0).unwrap();
        for w in ppr.windows(2) {
            assert!(w[0].1 >= w[1].1);
        }
    }

    // ---- validation ----

    #[test]
    fn rejects_non_square() {
        let g = CsrMatrix::<f64>::from_coo(2, 3, vec![(0, 1, 1.0f64)]);
        let s = HybridRandomWalkSolver::new(0.15, 100);
        assert!(s.estimate_entry(&g, 0, 0).is_err());
    }

    #[test]
    fn rejects_oob() {
        let g = CsrMatrix::<f64>::from_coo(3, 3, vec![(0, 1, 1.0f64)]);
        let s = HybridRandomWalkSolver::new(0.15, 100);
        assert!(s.estimate_entry(&g, 5, 0).is_err());
    }

    #[test]
    fn rejects_bad_alpha() {
        let g = CsrMatrix::<f64>::from_coo(3, 3, vec![(0, 1, 1.0f64)]);
        assert!(HybridRandomWalkSolver::new(0.0, 100)
            .estimate_entry(&g, 0, 0)
            .is_err());
        assert!(HybridRandomWalkSolver::new(1.0, 100)
            .estimate_entry(&g, 0, 0)
            .is_err());
    }

    #[test]
    fn rejects_zero_walks() {
        let g = CsrMatrix::<f64>::from_coo(3, 3, vec![(0, 1, 1.0f64)]);
        assert!(HybridRandomWalkSolver::new(0.15, 0)
            .estimate_entry(&g, 0, 0)
            .is_err());
    }

    // ---- SolverEngine ----

    #[test]
    fn solver_engine() {
        let g = directed_cycle(4);
        let s = HybridRandomWalkSolver::new(0.15, 5_000).with_seed(42);
        let r = s
            .solve(&g, &[1.0, 0.0, 0.0, 0.0], &ComputeBudget::default())
            .unwrap();
        assert_eq!(r.algorithm, Algorithm::HybridRandomWalk);
        assert_eq!(r.solution.len(), 4);
    }

    // ---- SublinearPageRank ----

    #[test]
    fn ppr_basic() {
        let g = undirected_chain(5);
        let s = HybridRandomWalkSolver::new(0.15, 10_000).with_seed(42);
        let ppr = s.ppr(&g, 0, 0.15, 0.05).unwrap();

        let source_ppr = ppr
            .iter()
            .find(|&&(n, _)| n == 0)
            .map(|&(_, v)| v)
            .unwrap_or(0.0);
        assert!(source_ppr > 0.0);

        let total: f64 = ppr.iter().map(|&(_, v)| v).sum();
        assert!((total - 1.0).abs() < 0.1, "sum={}", total);
    }

    #[test]
    fn ppr_multi_seed() {
        let g = undirected_chain(5);
        let s = HybridRandomWalkSolver::new(0.15, 10_000).with_seed(42);
        let ppr = s
            .ppr_multi_seed(&g, &[(0, 0.5), (2, 0.5)], 0.15, 0.05)
            .unwrap();
        let total: f64 = ppr.iter().map(|&(_, v)| v).sum();
        assert!((total - 1.0).abs() < 0.1, "sum={}", total);
    }

    #[test]
    fn invalid_source_ppr() {
        let g = undirected_chain(3);
        let s = HybridRandomWalkSolver::new(0.15, 100);
        assert!(s.ppr(&g, 10, 0.15, 0.01).is_err());
    }

    // ---- complexity estimate ----

    #[test]
    fn complexity_reasonable() {
        let s = HybridRandomWalkSolver::new(0.15, 10_000);
        let p = SparsityProfile {
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
        let e = s.estimate_complexity(&p, 1000);
        assert_eq!(e.algorithm, Algorithm::HybridRandomWalk);
        assert_eq!(e.estimated_iterations, 10_000);
    }

    // ---- early termination ----

    #[test]
    fn early_termination() {
        let g = CsrMatrix::<f64>::from_coo(1, 1, Vec::<(usize, usize, f64)>::new());
        let s = HybridRandomWalkSolver::new(0.15, 1_000_000).with_seed(42);
        let p = s.estimate_entry(&g, 0, 0).unwrap();
        assert!((p - 1.0).abs() < 1e-10);
    }

    // ---- reproducibility ----

    #[test]
    fn deterministic_seed() {
        let g = directed_cycle(10);
        let s = HybridRandomWalkSolver::new(0.15, 10_000).with_seed(42);
        let r1 = s.ppr_from_source(&g, 0).unwrap();
        let r2 = s.ppr_from_source(&g, 0).unwrap();
        assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert_eq!(a.0, b.0);
            assert!((a.1 - b.1).abs() < 1e-12);
        }
    }
}
