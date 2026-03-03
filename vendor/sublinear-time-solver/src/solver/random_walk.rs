use std::collections::{HashMap, VecDeque};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use crate::core::{SparseMatrix, Vector, Result, SublinearError};
use crate::algorithms::{Precision, ConvergenceMetrics};

/// Configuration for random walk algorithms
#[derive(Debug, Clone)]
pub struct RandomWalkConfig {
    pub max_steps: usize,
    pub step_size: f64,
    pub convergence_tolerance: f64,
    pub variance_reduction: VarianceReduction,
    pub restart_probability: f64,
    pub seed: Option<u64>,
}

impl Default for RandomWalkConfig {
    fn default() -> Self {
        Self {
            max_steps: 10000,
            step_size: 0.85,
            convergence_tolerance: 1e-6,
            variance_reduction: VarianceReduction::Antithetic,
            restart_probability: 0.15,
            seed: None,
        }
    }
}

/// Variance reduction techniques for Monte Carlo estimation
#[derive(Debug, Clone, PartialEq)]
pub enum VarianceReduction {
    None,
    Antithetic,
    ControlVariates,
    ImportanceSampling,
    StratifiedSampling,
}

/// Random walk engine for solving linear systems and optimization problems
pub struct RandomWalkEngine {
    config: RandomWalkConfig,
    rng: ChaCha8Rng,
    convergence_history: Vec<f64>,
    step_count: usize,
}

impl RandomWalkEngine {
    pub fn new(config: RandomWalkConfig) -> Self {
        let rng = match config.seed {
            Some(seed) => ChaCha8Rng::seed_from_u64(seed),
            None => ChaCha8Rng::from_entropy(),
        };

        Self {
            config,
            rng,
            convergence_history: Vec::new(),
            step_count: 0,
        }
    }

    /// Solve linear system Ax = b using Monte Carlo random walks
    pub fn solve_linear_system(&mut self, a: &SparseMatrix, b: &Vector) -> Result<Vector> {
        if a.rows() != a.cols() {
            return Err(SublinearError::InvalidDimensions);
        }
        if a.rows() != b.len() {
            return Err(SublinearError::InvalidDimensions);
        }

        let n = a.rows();
        let mut solution = vec![0.0; n];
        let mut estimates = vec![Vec::new(); n];

        // Monte Carlo estimation with variance reduction
        for iteration in 0..self.config.max_steps {
            for start_vertex in 0..n {
                let estimate = self.random_walk_estimate(a, b, start_vertex)?;
                estimates[start_vertex].push(estimate);

                // Update running average
                let count = estimates[start_vertex].len() as f64;
                solution[start_vertex] = (solution[start_vertex] * (count - 1.0) + estimate) / count;
            }

            // Check convergence every 100 iterations
            if iteration % 100 == 0 && iteration > 0 {
                let convergence = self.compute_convergence(&estimates);
                self.convergence_history.push(convergence);

                if convergence < self.config.convergence_tolerance {
                    break;
                }
            }
        }

        self.step_count = self.convergence_history.len() * 100;
        Ok(solution)
    }

    /// Perform single random walk from start vertex
    fn random_walk_estimate(&mut self, a: &SparseMatrix, b: &Vector, start: usize) -> Result<f64> {
        let mut current = start;
        let mut path_sum = 0.0;
        let mut path_weight = 1.0;
        let mut steps = 0;

        loop {
            // Add contribution from current vertex
            path_sum += path_weight * b[current];

            // Check for restart or termination
            if self.rng.gen::<f64>() < self.config.restart_probability || steps >= self.config.max_steps {
                break;
            }

            // Choose next vertex based on transition probabilities
            let next_vertex = self.choose_next_vertex(a, current)?;
            if let Some(next) = next_vertex {
                // Update path weight based on transition probability
                let transition_prob = self.compute_transition_probability(a, current, next)?;
                path_weight *= transition_prob / self.config.step_size;
                current = next;
                steps += 1;
            } else {
                break;
            }
        }

        // Apply variance reduction if configured
        match self.config.variance_reduction {
            VarianceReduction::Antithetic => {
                let antithetic_estimate = self.antithetic_walk_estimate(a, b, start)?;
                Ok((path_sum + antithetic_estimate) / 2.0)
            }
            _ => Ok(path_sum),
        }
    }

    /// Choose next vertex in random walk based on matrix structure
    fn choose_next_vertex(&mut self, a: &SparseMatrix, current: usize) -> Result<Option<usize>> {
        let row = a.get_row(current);
        if row.is_empty() {
            return Ok(None);
        }

        // Compute cumulative distribution for vertex selection
        let mut cumulative_probs = Vec::new();
        let mut total_weight = 0.0;

        for (col, &weight) in row {
            total_weight += weight.abs();
            cumulative_probs.push((col, total_weight));
        }

        if total_weight == 0.0 {
            return Ok(None);
        }

        // Sample from cumulative distribution
        let sample = self.rng.gen::<f64>() * total_weight;
        for (col, cumulative) in cumulative_probs {
            if sample <= cumulative {
                return Ok(Some(col));
            }
        }

        Ok(None)
    }

    /// Compute transition probability between vertices
    fn compute_transition_probability(&self, a: &SparseMatrix, from: usize, to: usize) -> Result<f64> {
        let row = a.get_row(from);
        let total_weight: f64 = row.iter().map(|(_, &w)| w.abs()).sum();

        if total_weight == 0.0 {
            return Ok(0.0);
        }

        if let Some(&weight) = row.get(&to) {
            Ok(weight.abs() / total_weight)
        } else {
            Ok(0.0)
        }
    }

    /// Antithetic variance reduction technique
    fn antithetic_walk_estimate(&mut self, a: &SparseMatrix, b: &Vector, start: usize) -> Result<f64> {
        // Store current RNG state
        let original_rng = self.rng.clone();

        // Generate antithetic random numbers (1 - u instead of u)
        let mut current = start;
        let mut path_sum = 0.0;
        let mut path_weight = 1.0;
        let mut steps = 0;

        loop {
            path_sum += path_weight * b[current];

            let restart_rand = 1.0 - self.rng.gen::<f64>(); // Antithetic
            if restart_rand < self.config.restart_probability || steps >= self.config.max_steps {
                break;
            }

            let next_vertex = self.choose_next_vertex_antithetic(a, current)?;
            if let Some(next) = next_vertex {
                let transition_prob = self.compute_transition_probability(a, current, next)?;
                path_weight *= transition_prob / self.config.step_size;
                current = next;
                steps += 1;
            } else {
                break;
            }
        }

        // Restore original RNG state
        self.rng = original_rng;
        Ok(path_sum)
    }

    /// Choose next vertex using antithetic sampling
    fn choose_next_vertex_antithetic(&mut self, a: &SparseMatrix, current: usize) -> Result<Option<usize>> {
        let row = a.get_row(current);
        if row.is_empty() {
            return Ok(None);
        }

        let mut cumulative_probs = Vec::new();
        let mut total_weight = 0.0;

        for (col, &weight) in row {
            total_weight += weight.abs();
            cumulative_probs.push((col, total_weight));
        }

        if total_weight == 0.0 {
            return Ok(None);
        }

        // Antithetic sampling: use (1 - u) instead of u
        let sample = (1.0 - self.rng.gen::<f64>()) * total_weight;
        for (col, cumulative) in cumulative_probs {
            if sample <= cumulative {
                return Ok(Some(col));
            }
        }

        Ok(None)
    }

    /// Compute convergence metric across all estimates
    fn compute_convergence(&self, estimates: &[Vec<f64>]) -> f64 {
        let mut max_variance = 0.0;

        for vertex_estimates in estimates {
            if vertex_estimates.len() < 2 {
                continue;
            }

            let mean = vertex_estimates.iter().sum::<f64>() / vertex_estimates.len() as f64;
            let variance = vertex_estimates.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / (vertex_estimates.len() - 1) as f64;

            max_variance = max_variance.max(variance.sqrt() / mean.abs().max(1e-10));
        }

        max_variance
    }

    /// Get convergence metrics
    pub fn get_metrics(&self) -> ConvergenceMetrics {
        ConvergenceMetrics {
            iterations: self.step_count,
            residual: self.convergence_history.last().copied().unwrap_or(f64::INFINITY),
            convergence_rate: self.compute_convergence_rate(),
            precision: if self.convergence_history.last().unwrap_or(&f64::INFINITY) < &self.config.convergence_tolerance {
                Precision::High
            } else {
                Precision::Low
            },
        }
    }

    fn compute_convergence_rate(&self) -> f64 {
        if self.convergence_history.len() < 2 {
            return 0.0;
        }

        let n = self.convergence_history.len();
        let recent_slope = (self.convergence_history[n-1] - self.convergence_history[n-2]).abs();
        recent_slope.max(1e-12)
    }
}

/// Bidirectional random walk for improved convergence
pub struct BidirectionalWalk {
    forward_engine: RandomWalkEngine,
    backward_engine: RandomWalkEngine,
}

impl BidirectionalWalk {
    pub fn new(config: RandomWalkConfig) -> Self {
        let mut backward_config = config.clone();
        backward_config.seed = config.seed.map(|s| s.wrapping_add(1));

        Self {
            forward_engine: RandomWalkEngine::new(config),
            backward_engine: RandomWalkEngine::new(backward_config),
        }
    }

    /// Solve using bidirectional random walks
    pub fn solve_linear_system(&mut self, a: &SparseMatrix, b: &Vector) -> Result<Vector> {
        // Solve forward problem: Ax = b
        let forward_solution = self.forward_engine.solve_linear_system(a, b)?;

        // Solve backward problem: A^T y = x, then use for refinement
        let a_transpose = a.transpose();
        let backward_solution = self.backward_engine.solve_linear_system(&a_transpose, &forward_solution)?;

        // Combine solutions with optimal weighting
        let alpha = self.compute_optimal_weight(&forward_solution, &backward_solution);
        let mut combined = vec![0.0; forward_solution.len()];

        for i in 0..combined.len() {
            combined[i] = alpha * forward_solution[i] + (1.0 - alpha) * backward_solution[i];
        }

        Ok(combined)
    }

    fn compute_optimal_weight(&self, forward: &Vector, backward: &Vector) -> f64 {
        let forward_metrics = self.forward_engine.get_metrics();
        let backward_metrics = self.backward_engine.get_metrics();

        // Weight based on convergence quality
        let forward_quality = 1.0 / (forward_metrics.residual + 1e-10);
        let backward_quality = 1.0 / (backward_metrics.residual + 1e-10);

        forward_quality / (forward_quality + backward_quality)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;

    #[test]
    fn test_random_walk_engine_creation() {
        let config = RandomWalkConfig::default();
        let engine = RandomWalkEngine::new(config);
        assert_eq!(engine.step_count, 0);
    }

    #[test]
    fn test_simple_linear_system() {
        let mut config = RandomWalkConfig::default();
        config.max_steps = 1000;
        config.seed = Some(42);

        let mut engine = RandomWalkEngine::new(config);

        // Simple 2x2 system: [2, -1; -1, 2] * x = [1; 1]
        let mut matrix = SparseMatrix::new(2, 2);
        matrix.insert(0, 0, 2.0);
        matrix.insert(0, 1, -1.0);
        matrix.insert(1, 0, -1.0);
        matrix.insert(1, 1, 2.0);

        let b = vec![1.0, 1.0];
        let solution = engine.solve_linear_system(&matrix, &b).unwrap();

        // Expected solution is approximately [1, 1]
        assert!((solution[0] - 1.0).abs() < 0.1);
        assert!((solution[1] - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_bidirectional_walk() {
        let mut config = RandomWalkConfig::default();
        config.max_steps = 500;
        config.seed = Some(123);

        let mut bidirectional = BidirectionalWalk::new(config);

        let mut matrix = SparseMatrix::new(3, 3);
        matrix.insert(0, 0, 3.0);
        matrix.insert(0, 1, -1.0);
        matrix.insert(1, 0, -1.0);
        matrix.insert(1, 1, 3.0);
        matrix.insert(1, 2, -1.0);
        matrix.insert(2, 1, -1.0);
        matrix.insert(2, 2, 3.0);

        let b = vec![2.0, 1.0, 2.0];
        let solution = bidirectional.solve_linear_system(&matrix, &b).unwrap();

        assert_eq!(solution.len(), 3);
        // Verify solution quality by checking residual
        let residual = compute_residual(&matrix, &solution, &b);
        assert!(residual < 0.5);
    }
}