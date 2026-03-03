use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::core::{SparseMatrix, Vector, Result, SublinearError};
use crate::algorithms::{Precision, ConvergenceMetrics, Algorithm};
use crate::solver::random_walk::{RandomWalkEngine, RandomWalkConfig, BidirectionalWalk, VarianceReduction};
use crate::solver::sampling::{AdaptiveSampler, SamplingConfig, SamplingStrategy, MultiLevelSampler};
use crate::solver::forward_push::{ForwardPushSolver, ForwardPushConfig, ForwardPushResult};
use crate::graph::PushGraph;

/// Phase of the hybrid solver
#[derive(Debug, Clone, PartialEq)]
pub enum HybridPhase {
    ForwardPush,
    RandomWalk,
    ConjugateGradient,
    Adaptive,
}

/// Hybrid solver configuration combining multiple approaches
#[derive(Debug, Clone)]
pub struct HybridConfig {
    pub max_iterations: usize,
    pub convergence_tolerance: f64,
    pub use_deterministic: bool,
    pub use_random_walk: bool,
    pub use_bidirectional: bool,
    pub use_multilevel: bool,
    pub use_forward_push: bool,
    pub deterministic_weight: f64,
    pub random_walk_config: RandomWalkConfig,
    pub forward_push_config: ForwardPushConfig,
    pub sampling_config: SamplingConfig,
    pub adaptation_interval: usize,
    pub parallel_execution: bool,
    pub memory_limit: usize, // MB
    // Hybrid-specific parameters
    pub phase_switch_threshold: f64,
    pub min_phase_iterations: usize,
    pub max_phase_iterations: usize,
    pub convergence_window: usize,
    pub improvement_threshold: f64,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10000,
            convergence_tolerance: 1e-8,
            use_deterministic: true,
            use_random_walk: true,
            use_bidirectional: false,
            use_multilevel: false,
            use_forward_push: true,
            deterministic_weight: 0.7,
            random_walk_config: RandomWalkConfig::default(),
            forward_push_config: ForwardPushConfig::default(),
            sampling_config: SamplingConfig::default(),
            adaptation_interval: 100,
            parallel_execution: true,
            memory_limit: 1024,
            // Hybrid-specific defaults
            phase_switch_threshold: 0.1,  // Switch if improvement < 10%
            min_phase_iterations: 50,
            max_phase_iterations: 2000,
            convergence_window: 20,
            improvement_threshold: 0.05,  // 5% improvement required
        }
    }
}

/// Hybrid solver combining deterministic and stochastic methods
pub struct HybridSolver {
    config: HybridConfig,
    convergence_history: Vec<f64>,
    method_performance: HashMap<String, MethodMetrics>,
    adaptive_weights: AdaptiveWeights,
    current_iteration: usize,
    current_phase: HybridPhase,
    phase_start_iteration: usize,
    phase_history: Vec<(HybridPhase, usize, f64, Duration)>,
    global_best_solution: Option<Vector>,
    global_best_residual: f64,
    phase_convergence_rates: HashMap<HybridPhase, Vec<f64>>,
}

#[derive(Debug, Clone)]
struct MethodMetrics {
    convergence_rate: f64,
    computation_time: Duration,
    memory_usage: usize,
    accuracy: f64,
    reliability: f64,
}

impl Default for MethodMetrics {
    fn default() -> Self {
        Self {
            convergence_rate: 0.0,
            computation_time: Duration::from_secs(0),
            memory_usage: 0,
            accuracy: 0.0,
            reliability: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
struct AdaptiveWeights {
    deterministic: f64,
    random_walk: f64,
    bidirectional: f64,
    multilevel: f64,
}

impl Default for AdaptiveWeights {
    fn default() -> Self {
        Self {
            deterministic: 0.4,
            random_walk: 0.3,
            bidirectional: 0.2,
            multilevel: 0.1,
        }
    }
}

impl HybridSolver {
    pub fn new(config: HybridConfig) -> Self {
        Self {
            config,
            convergence_history: Vec::new(),
            method_performance: HashMap::new(),
            adaptive_weights: AdaptiveWeights::default(),
            current_iteration: 0,
            current_phase: HybridPhase::ForwardPush,
            phase_start_iteration: 0,
            phase_history: Vec::new(),
            global_best_solution: None,
            global_best_residual: f64::INFINITY,
            phase_convergence_rates: HashMap::new(),
        }
    }

    /// Solve linear system using hybrid 3-phase approach
    pub fn solve_linear_system(&mut self, a: &SparseMatrix, b: &Vector) -> Result<Vector> {
        if a.rows() != a.cols() {
            return Err(SublinearError::InvalidDimensions);
        }
        if a.rows() != b.len() {
            return Err(SublinearError::InvalidDimensions);
        }

        let n = a.rows();
        let mut solution = vec![0.0; n];
        self.global_best_solution = Some(solution.clone());
        self.global_best_residual = f64::INFINITY;

        let start_time = Instant::now();
        self.current_phase = HybridPhase::ForwardPush;
        self.phase_start_iteration = 0;

        // PHASE 1: Forward Push - Fast local computation
        if self.config.use_forward_push {
            let phase_start_time = Instant::now();
            solution = self.solve_forward_push_phase(a, b)?;
            let phase_time = phase_start_time.elapsed();
            let residual = self.compute_residual(a, &solution, b);

            self.record_phase_completion(HybridPhase::ForwardPush, residual, phase_time);
            self.update_global_best(&solution, residual);

            println!("Phase 1 (Forward Push): residual = {:.2e}, time = {:?}", residual, phase_time);
        }

        // PHASE 2: Random Walk - Global accuracy improvement
        if self.config.use_random_walk && !self.has_converged() {
            let phase_start_time = Instant::now();
            self.current_phase = HybridPhase::RandomWalk;
            self.phase_start_iteration = self.current_iteration;

            solution = self.solve_random_walk_phase(a, b, solution)?;
            let phase_time = phase_start_time.elapsed();
            let residual = self.compute_residual(a, &solution, b);

            self.record_phase_completion(HybridPhase::RandomWalk, residual, phase_time);
            self.update_global_best(&solution, residual);

            println!("Phase 2 (Random Walk): residual = {:.2e}, time = {:?}", residual, phase_time);
        }

        // PHASE 3: Conjugate Gradient - Final convergence polish
        if self.config.use_deterministic && !self.has_converged() {
            let phase_start_time = Instant::now();
            self.current_phase = HybridPhase::ConjugateGradient;
            self.phase_start_iteration = self.current_iteration;

            solution = self.solve_conjugate_gradient_phase(a, b, solution)?;
            let phase_time = phase_start_time.elapsed();
            let residual = self.compute_residual(a, &solution, b);

            self.record_phase_completion(HybridPhase::ConjugateGradient, residual, phase_time);
            self.update_global_best(&solution, residual);

            println!("Phase 3 (Conjugate Gradient): residual = {:.2e}, time = {:?}", residual, phase_time);
        }

        // Return the globally best solution found
        let final_solution = self.global_best_solution.clone().unwrap_or(solution);
        let total_time = start_time.elapsed();
        self.update_global_metrics(total_time, self.global_best_residual);

        println!("Hybrid solver completed: best residual = {:.2e}, total time = {:?}",
                self.global_best_residual, total_time);

        Ok(final_solution)
    }

    /// Phase 1: Forward Push - Fast initial estimate
    fn solve_forward_push_phase(&mut self, a: &SparseMatrix, b: &Vector) -> Result<Vector> {
        // Convert sparse matrix to push graph format
        let push_graph = self.matrix_to_push_graph(a)?;
        let forward_push_solver = ForwardPushSolver::new(push_graph, self.config.forward_push_config.clone());

        let n = a.rows();
        let mut solution = vec![0.0; n];

        // Perform forward push for each unit vector to build solution
        for i in 0..n {
            if b[i] != 0.0 {
                let result = forward_push_solver.solve_single_source(i);

                // Weight by RHS value and accumulate
                for j in 0..n {
                    solution[j] += b[i] * result.estimate[j];
                }

                self.current_iteration += result.push_count;
            }
        }

        // Record convergence for this phase
        let residual = self.compute_residual(a, &solution, b);
        self.convergence_history.push(residual);

        Ok(solution)
    }

    /// Phase 2: Random Walk - Refine with stochastic sampling
    fn solve_random_walk_phase(&mut self, a: &SparseMatrix, b: &Vector, initial_solution: Vector) -> Result<Vector> {
        let mut random_walk_engine = RandomWalkEngine::new(self.config.random_walk_config.clone());

        // Use random walk to refine the solution
        let mut solution = initial_solution;
        let max_iterations = self.config.max_phase_iterations.min(1000);

        for iteration in 0..max_iterations {
            // Perform random walk estimation
            let walk_solution = random_walk_engine.solve_linear_system(a, b)?;

            // Adaptive blending with previous solution
            let blend_factor = 0.3 * (1.0 - iteration as f64 / max_iterations as f64);
            for i in 0..solution.len() {
                solution[i] = (1.0 - blend_factor) * solution[i] + blend_factor * walk_solution[i];
            }

            self.current_iteration += 1;

            // Check phase convergence
            let residual = self.compute_residual(a, &solution, b);
            self.convergence_history.push(residual);

            if self.should_switch_phase(iteration) {
                break;
            }
        }

        Ok(solution)
    }

    /// Phase 3: Conjugate Gradient - Final polish
    fn solve_conjugate_gradient_phase(&mut self, a: &SparseMatrix, b: &Vector, initial_solution: Vector) -> Result<Vector> {
        let mut x = initial_solution;
        let mut r = self.compute_residual_vector(a, &x, b);
        let mut p = r.clone();
        let mut rsold = self.dot_product(&r, &r);

        let max_iterations = self.config.max_phase_iterations.min(500);

        for iteration in 0..max_iterations {
            // Compute Ap
            let ap = self.matrix_vector_multiply(a, &p);
            let alpha = rsold / self.dot_product(&p, &ap);

            // Update solution and residual
            for i in 0..x.len() {
                x[i] += alpha * p[i];
                r[i] -= alpha * ap[i];
            }

            let rsnew = self.dot_product(&r, &r);
            let residual_norm = rsnew.sqrt();

            self.current_iteration += 1;
            self.convergence_history.push(residual_norm);

            // Check convergence
            if residual_norm < self.config.convergence_tolerance {
                break;
            }

            if self.should_switch_phase(iteration) {
                break;
            }

            // Update search direction
            let beta = rsnew / rsold;
            for i in 0..p.len() {
                p[i] = r[i] + beta * p[i];
            }

            rsold = rsnew;
        }

        Ok(x)
    }

    /// Convert sparse matrix to push graph format
    fn matrix_to_push_graph(&self, a: &SparseMatrix) -> Result<PushGraph> {
        // This is a simplified conversion - in practice would need proper graph construction
        // For now, create a simple graph representation
        let n = a.rows();
        let mut edges = Vec::new();

        for i in 0..n {
            let row = a.get_row(i);
            for (&j, &weight) in row {
                if i != j && weight != 0.0 {
                    edges.push((i, j, weight.abs()));
                }
            }
        }

        // Create push graph from edges
        PushGraph::from_edges(n, &edges)
    }

    /// Check if current phase should switch based on convergence rate
    fn should_switch_phase(&self, phase_iteration: usize) -> bool {
        if phase_iteration < self.config.min_phase_iterations {
            return false;
        }

        if phase_iteration >= self.config.max_phase_iterations {
            return true;
        }

        // Check improvement rate in recent window
        if self.convergence_history.len() >= self.config.convergence_window {
            let window_size = self.config.convergence_window;
            let recent_start = self.convergence_history.len() - window_size;
            let start_residual = self.convergence_history[recent_start];
            let end_residual = self.convergence_history.last().unwrap();

            let improvement_rate = if start_residual > 0.0 {
                (start_residual - end_residual) / start_residual
            } else {
                0.0
            };

            return improvement_rate < self.config.improvement_threshold;
        }

        false
    }

    /// Check if solver has converged globally
    fn has_converged(&self) -> bool {
        self.global_best_residual < self.config.convergence_tolerance
    }

    /// Update global best solution if current is better
    fn update_global_best(&mut self, solution: &Vector, residual: f64) {
        if residual < self.global_best_residual {
            self.global_best_residual = residual;
            self.global_best_solution = Some(solution.clone());
        }
    }

    /// Record completion of a phase
    fn record_phase_completion(&mut self, phase: HybridPhase, residual: f64, duration: Duration) {
        let iterations_in_phase = self.current_iteration - self.phase_start_iteration;
        self.phase_history.push((phase.clone(), iterations_in_phase, residual, duration));

        // Track convergence rate for this phase
        self.phase_convergence_rates.entry(phase)
            .or_insert_with(Vec::new)
            .push(residual);
    }

    /// Compute residual vector (r = b - Ax)
    fn compute_residual_vector(&self, a: &SparseMatrix, x: &Vector, b: &Vector) -> Vector {
        let ax = self.matrix_vector_multiply(a, x);
        ax.iter().zip(b.iter()).map(|(ax_i, b_i)| b_i - ax_i).collect()
    }

    /// Matrix-vector multiplication
    fn matrix_vector_multiply(&self, a: &SparseMatrix, x: &Vector) -> Vector {
        let mut result = vec![0.0; a.rows()];

        for i in 0..a.rows() {
            let row = a.get_row(i);
            for (&j, &value) in row {
                result[i] += value * x[j];
            }
        }

        result
    }

    /// Dot product of two vectors
    fn dot_product(&self, a: &Vector, b: &Vector) -> f64 {
        a.iter().zip(b.iter()).map(|(ai, bi)| ai * bi).sum()
    }

    /// Solve using sequential execution of methods
    fn solve_sequential(
        &mut self,
        a: &SparseMatrix,
        b: &Vector,
        random_walk_engine: &mut Option<RandomWalkEngine>,
        bidirectional_walk: &mut Option<BidirectionalWalk>,
        multilevel_sampler: &mut Option<MultiLevelSampler>,
    ) -> Result<Vector> {
        let n = a.rows();
        let mut solutions = Vec::new();

        // Deterministic method
        if self.config.use_deterministic {
            let start = Instant::now();
            if let Ok(solution) = Self::solve_deterministic(a, b) {
                let time = start.elapsed();
                solutions.push((solution, self.adaptive_weights.deterministic, time, "deterministic"));
                self.update_method_metrics("deterministic", time, 0);
            }
        }

        // Random walk method
        if self.config.use_random_walk {
            if let Some(ref mut engine) = random_walk_engine {
                let start = Instant::now();
                if let Ok(solution) = engine.solve_linear_system(a, b) {
                    let time = start.elapsed();
                    solutions.push((solution, self.adaptive_weights.random_walk, time, "random_walk"));
                    self.update_method_metrics("random_walk", time, 0);
                }
            }
        }

        // Bidirectional walk method
        if self.config.use_bidirectional {
            if let Some(ref mut solver) = bidirectional_walk {
                let start = Instant::now();
                if let Ok(solution) = solver.solve_linear_system(a, b) {
                    let time = start.elapsed();
                    solutions.push((solution, self.adaptive_weights.bidirectional, time, "bidirectional"));
                    self.update_method_metrics("bidirectional", time, 0);
                }
            }
        }

        if solutions.is_empty() {
            return Err(SublinearError::ComputationError("No solutions computed".to_string()));
        }

        self.combine_solutions(&solutions, n)
    }

    /// Simple deterministic solver (Jacobi iteration)
    fn solve_deterministic(a: &SparseMatrix, b: &Vector) -> Result<Vector> {
        let n = a.rows();
        let mut x = vec![0.0; n];
        let mut x_new = vec![0.0; n];
        let max_iter = 1000;
        let tolerance = 1e-10;

        for _ in 0..max_iter {
            for i in 0..n {
                let mut sum = 0.0;
                let row = a.get_row(i);
                let diagonal = row.get(&i).copied().unwrap_or(1.0);

                if diagonal.abs() < 1e-12 {
                    continue;
                }

                for (&j, &value) in row {
                    if i != j {
                        sum += value * x[j];
                    }
                }

                x_new[i] = (b[i] - sum) / diagonal;
            }

            // Check convergence
            let mut norm_diff = 0.0;
            for i in 0..n {
                norm_diff += (x_new[i] - x[i]).powi(2);
            }
            norm_diff = norm_diff.sqrt();

            x.copy_from_slice(&x_new);

            if norm_diff < tolerance {
                break;
            }
        }

        Ok(x)
    }

    /// Combine solutions from different methods
    fn combine_solutions(&self, solutions: &[(Vector, f64, Duration, &str)], n: usize) -> Result<Vector> {
        let mut combined = vec![0.0; n];
        let mut total_weight = 0.0;

        // Compute quality-adjusted weights
        let mut adjusted_solutions = Vec::new();
        for (solution, base_weight, time, method) in solutions {
            let quality = self.assess_solution_quality(solution);
            let time_penalty = 1.0 / (1.0 + time.as_secs_f64());
            let adjusted_weight = base_weight * quality * time_penalty;

            adjusted_solutions.push((solution, adjusted_weight, method));
            total_weight += adjusted_weight;
        }

        if total_weight == 0.0 {
            return Err(SublinearError::ComputationError("Zero total weight".to_string()));
        }

        // Weighted combination
        for (solution, weight, _) in adjusted_solutions {
            let normalized_weight = weight / total_weight;
            for i in 0..n {
                combined[i] += normalized_weight * solution[i];
            }
        }

        Ok(combined)
    }

    /// Assess solution quality based on various metrics
    fn assess_solution_quality(&self, solution: &Vector) -> f64 {
        // Simple quality assessment based on solution properties
        let norm = solution.iter().map(|x| x.powi(2)).sum::<f64>().sqrt();
        let stability = 1.0 / (1.0 + solution.iter().map(|x| x.abs()).fold(0.0f64, f64::max));
        let consistency = 1.0 / (1.0 + solution.iter().map(|x| if x.is_finite() { 0.0 } else { 1.0 }).sum::<f64>());

        (stability * consistency).min(1.0).max(0.1)
    }

    /// Adapt method weights based on performance
    fn adapt_method_weights(&mut self) {
        let learning_rate = 0.1;
        let recent_window = 10;

        if self.convergence_history.len() < recent_window {
            return;
        }

        let recent_improvement = self.compute_recent_improvement(recent_window);

        // Adjust weights based on recent performance
        if recent_improvement > 0.0 {
            // Good progress - slightly increase current method weights
            self.adaptive_weights.deterministic *= 1.0 + learning_rate * recent_improvement;
            self.adaptive_weights.random_walk *= 1.0 + learning_rate * recent_improvement;
        } else {
            // Poor progress - shift weights toward alternative methods
            self.adaptive_weights.bidirectional *= 1.0 + learning_rate * 0.1;
            self.adaptive_weights.multilevel *= 1.0 + learning_rate * 0.1;
        }

        // Normalize weights
        let total = self.adaptive_weights.deterministic + self.adaptive_weights.random_walk
                  + self.adaptive_weights.bidirectional + self.adaptive_weights.multilevel;

        if total > 0.0 {
            self.adaptive_weights.deterministic /= total;
            self.adaptive_weights.random_walk /= total;
            self.adaptive_weights.bidirectional /= total;
            self.adaptive_weights.multilevel /= total;
        }
    }

    /// Compute recent improvement rate
    fn compute_recent_improvement(&self, window: usize) -> f64 {
        let len = self.convergence_history.len();
        if len < window {
            return 0.0;
        }

        let start_idx = len - window;
        let start_value = self.convergence_history[start_idx];
        let end_value = self.convergence_history[len - 1];

        if start_value > 0.0 {
            (start_value - end_value) / start_value
        } else {
            0.0
        }
    }

    /// Update method performance metrics
    fn update_method_metrics(&mut self, method: &str, time: Duration, memory: usize) {
        let metrics = self.method_performance.entry(method.to_string()).or_default();
        metrics.computation_time = time;
        metrics.memory_usage = memory;

        // Update convergence rate
        if self.convergence_history.len() >= 2 {
            let recent_rate = (self.convergence_history[self.convergence_history.len()-2]
                             - self.convergence_history[self.convergence_history.len()-1]).abs();
            metrics.convergence_rate = recent_rate;
        }
    }

    /// Compute residual norm
    fn compute_residual(&self, a: &SparseMatrix, x: &Vector, b: &Vector) -> f64 {
        let mut residual = vec![0.0; b.len()];

        // Compute Ax
        for i in 0..a.rows() {
            let row = a.get_row(i);
            for (&j, &value) in row {
                residual[i] += value * x[j];
            }
            residual[i] -= b[i];
        }

        // Compute L2 norm
        residual.iter().map(|r| r.powi(2)).sum::<f64>().sqrt()
    }

    /// Estimate current memory usage
    fn estimate_memory_usage(&self) -> usize {
        let base_size = std::mem::size_of::<Self>();
        let history_size = self.convergence_history.len() * std::mem::size_of::<f64>();
        let metrics_size = self.method_performance.len() * 200; // Rough estimate

        base_size + history_size + metrics_size
    }

    /// Clean up memory usage
    fn cleanup_memory(&mut self) {
        // Keep only recent convergence history
        let keep_size = 100;
        if self.convergence_history.len() > keep_size {
            let start_idx = self.convergence_history.len() - keep_size;
            self.convergence_history = self.convergence_history[start_idx..].to_vec();
        }

        // Clear old method metrics except recent ones
        self.method_performance.retain(|_, metrics| {
            metrics.computation_time.as_secs() < 300 // Keep metrics from last 5 minutes
        });
    }

    /// Update global performance metrics
    fn update_global_metrics(&mut self, total_time: Duration, final_residual: f64) {
        // Update reliability scores based on final performance
        for (_, metrics) in self.method_performance.iter_mut() {
            if final_residual < self.config.convergence_tolerance {
                metrics.reliability = (metrics.reliability * 0.9 + 0.1).min(1.0);
            } else {
                metrics.reliability = (metrics.reliability * 0.9).max(0.1);
            }

            metrics.accuracy = 1.0 / (1.0 + final_residual);
        }
    }

    /// Get comprehensive solver metrics
    pub fn get_metrics(&self) -> HybridMetrics {
        let final_residual = self.global_best_residual;
        let convergence_rate = self.compute_recent_improvement(10);

        HybridMetrics {
            total_iterations: self.current_iteration,
            final_residual,
            convergence_rate,
            method_weights: self.adaptive_weights.clone(),
            method_performance: self.method_performance.clone(),
            memory_usage: self.estimate_memory_usage(),
            precision: if final_residual < self.config.convergence_tolerance {
                Precision::High
            } else if final_residual < self.config.convergence_tolerance * 100.0 {
                Precision::Medium
            } else {
                Precision::Low
            },
            phase_history: self.phase_history.clone(),
            phase_convergence_rates: self.phase_convergence_rates.clone(),
            current_phase: self.current_phase.clone(),
        }
    }
}

/// Comprehensive metrics for hybrid solver
#[derive(Debug, Clone)]
pub struct HybridMetrics {
    pub total_iterations: usize,
    pub final_residual: f64,
    pub convergence_rate: f64,
    pub method_weights: AdaptiveWeights,
    pub method_performance: HashMap<String, MethodMetrics>,
    pub memory_usage: usize,
    pub precision: Precision,
    pub phase_history: Vec<(HybridPhase, usize, f64, Duration)>,
    pub phase_convergence_rates: HashMap<HybridPhase, Vec<f64>>,
    pub current_phase: HybridPhase,
}

impl Algorithm for HybridSolver {
    fn solve(&mut self, matrix: &SparseMatrix, target: &Vector) -> Result<Vector> {
        self.solve_linear_system(matrix, target)
    }

    fn get_metrics(&self) -> ConvergenceMetrics {
        let hybrid_metrics = self.get_metrics();
        ConvergenceMetrics {
            iterations: hybrid_metrics.total_iterations,
            residual: hybrid_metrics.final_residual,
            convergence_rate: hybrid_metrics.convergence_rate,
            precision: hybrid_metrics.precision,
        }
    }

    fn update_config(&mut self, _params: std::collections::HashMap<String, f64>) {
        // Update configuration based on provided parameters
        // Implementation depends on specific parameter mapping
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;

    #[test]
    fn test_hybrid_solver_creation() {
        let config = HybridConfig::default();
        let solver = HybridSolver::new(config);
        assert_eq!(solver.current_iteration, 0);
    }

    #[test]
    fn test_simple_linear_system() {
        let mut config = HybridConfig::default();
        config.max_iterations = 100;
        config.convergence_tolerance = 1e-4;
        config.parallel_execution = false; // Simpler for testing

        let mut solver = HybridSolver::new(config);

        // Simple 2x2 system: [3, -1; -1, 3] * x = [2; 2]
        let mut matrix = SparseMatrix::new(2, 2);
        matrix.insert(0, 0, 3.0);
        matrix.insert(0, 1, -1.0);
        matrix.insert(1, 0, -1.0);
        matrix.insert(1, 1, 3.0);

        let b = vec![2.0, 2.0];
        let solution = solver.solve_linear_system(&matrix, &b).unwrap();

        // Expected solution is approximately [1, 1]
        assert!((solution[0] - 1.0).abs() < 0.2);
        assert!((solution[1] - 1.0).abs() < 0.2);

        let metrics = solver.get_metrics();
        assert!(metrics.total_iterations > 0);
    }

    #[test]
    fn test_adaptive_weight_adjustment() {
        let config = HybridConfig {
            adaptation_interval: 5,
            ..Default::default()
        };
        let mut solver = HybridSolver::new(config);

        // Simulate convergence history
        solver.convergence_history = vec![1.0, 0.8, 0.6, 0.4, 0.2, 0.1];

        let initial_weights = solver.adaptive_weights.clone();
        solver.adapt_method_weights();

        // Weights should be normalized
        let total = solver.adaptive_weights.deterministic + solver.adaptive_weights.random_walk
                  + solver.adaptive_weights.bidirectional + solver.adaptive_weights.multilevel;
        assert!((total - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_solution_quality_assessment() {
        let config = HybridConfig::default();
        let solver = HybridSolver::new(config);

        let good_solution = vec![1.0, 1.0, 1.0];
        let bad_solution = vec![f64::INFINITY, f64::NAN, 1e10];

        let good_quality = solver.assess_solution_quality(&good_solution);
        let bad_quality = solver.assess_solution_quality(&bad_solution);

        assert!(good_quality > bad_quality);
        assert!(good_quality <= 1.0);
        assert!(bad_quality >= 0.1);
    }

    #[test]
    fn test_memory_cleanup() {
        let config = HybridConfig::default();
        let mut solver = HybridSolver::new(config);

        // Fill with large history
        solver.convergence_history = vec![1.0; 1000];
        let initial_size = solver.convergence_history.len();

        solver.cleanup_memory();

        assert!(solver.convergence_history.len() <= 100);
        assert!(solver.convergence_history.len() < initial_size);
    }
}