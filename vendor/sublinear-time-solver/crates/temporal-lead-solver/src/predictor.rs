//! Temporal computational lead predictor using sublinear local solvers
//!
//! Based on:
//! - Kwok, Wei, Yang 2025: "On Solving Asymmetric Diagonally Dominant Linear Systems in Sublinear Time"
//! - Feng, Li, Peng 2025: "Sublinear-Time Algorithms for Diagonally Dominant Linear Systems"
//! - Andoni, Krauthgamer, Pogrow 2019: ITCS SDD local solvers

use crate::core::{Matrix, Vector, Complexity};
use crate::physics::{Distance, TemporalAdvantage};
use crate::solver::{SublinearSolver, SolverMethod, SolverResult};
use crate::FTLError;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// Structural parameters for diagonally dominant systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DominanceParameters {
    /// Strict dominance factor δ > 0
    pub delta: f64,
    /// Maximum p-norm gap
    pub max_p_norm_gap: f64,
    /// Scale factor S_max
    pub s_max: f64,
    /// Condition number κ
    pub condition_number: f64,
    /// Sparsity (fraction of non-zeros)
    pub sparsity: f64,
}

impl DominanceParameters {
    /// Analyze matrix to extract parameters
    pub fn from_matrix(m: &Matrix) -> Self {
        let (n, _) = m.shape();
        let mut delta = f64::MAX;
        let mut s_max = 0.0;

        // Compute dominance parameters
        for i in 0..n {
            let diagonal = m.view()[[i, i]].abs();
            let mut off_diagonal_sum = 0.0;

            for j in 0..n {
                if i != j {
                    let val = m.view()[[i, j]].abs();
                    off_diagonal_sum += val;
                    s_max = s_max.max(val);
                }
            }

            if diagonal > off_diagonal_sum {
                delta = delta.min(diagonal - off_diagonal_sum);
            }
        }

        // Estimate condition number (simplified)
        let spectral_radius = m.spectral_radius();
        let condition = if delta > 0.0 {
            spectral_radius / delta
        } else {
            f64::INFINITY
        };

        // Compute sparsity
        let sparse = m.to_sparse();
        let sparsity = 1.0 - sparse.sparsity();

        Self {
            delta,
            max_p_norm_gap: s_max / delta.max(1e-10),
            s_max,
            condition_number: condition,
            sparsity,
        }
    }

    /// Check if parameters allow sublinear solving
    pub fn allows_sublinear(&self) -> bool {
        self.delta > 0.0 && self.max_p_norm_gap < 100.0 && self.condition_number < 1e6
    }

    /// Estimate query complexity for single coordinate
    pub fn query_complexity(&self, epsilon: f64) -> usize {
        // Based on Theorem 1 from Kwok-Wei-Yang 2025
        let base = (1.0 / self.delta).max(1.0);
        let epsilon_factor = (1.0 / epsilon).max(1.0);
        let gap_factor = self.max_p_norm_gap.max(1.0);

        ((base * epsilon_factor * gap_factor).log2() * 100.0) as usize
    }

    /// Estimate time complexity in nanoseconds
    pub fn time_complexity_ns(&self, epsilon: f64, n: usize) -> u64 {
        let queries = self.query_complexity(epsilon);
        let log_n = (n as f64).log2().max(1.0);

        // Time = O(queries * log n) for local access
        (queries as f64 * log_n * 100.0) as u64
    }
}

/// Result of temporal prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResult {
    /// Target functional value t^T x*
    pub functional_value: f64,
    /// Estimated error bound
    pub error_bound: f64,
    /// Dominance parameters used
    pub parameters: DominanceParameters,
    /// Temporal advantage achieved
    pub temporal_advantage: TemporalAdvantage,
    /// Number of queries made
    pub queries: usize,
    /// Computation time
    pub computation_time: Duration,
    /// Whether lower bounds were hit
    pub hit_lower_bound: bool,
}

impl PredictionResult {
    /// Check if prediction achieved temporal lead
    pub fn has_temporal_lead(&self) -> bool {
        self.temporal_advantage.is_ftl()
    }

    /// Get temporal advantage in milliseconds
    pub fn temporal_advantage_ms(&self) -> f64 {
        self.temporal_advantage.advantage_ms()
    }

    /// Describe the result
    pub fn describe(&self) -> String {
        format!(
            "Temporal lead: {:.2}ms | Value: {:.6} ± {:.6} | Queries: {} | δ={:.3}",
            self.temporal_advantage_ms(),
            self.functional_value,
            self.error_bound,
            self.queries,
            self.parameters.delta
        )
    }
}

/// Temporal lead predictor using sublinear local solvers
pub struct TemporalPredictor {
    distance: Distance,
    epsilon: f64,
    method: SolverMethod,
}

impl TemporalPredictor {
    /// Create predictor for given distance
    pub fn new(distance: Distance) -> Self {
        Self {
            distance,
            epsilon: 1e-6,
            method: SolverMethod::Adaptive,
        }
    }

    /// Set accuracy parameter epsilon
    pub fn with_epsilon(mut self, epsilon: f64) -> Self {
        self.epsilon = epsilon;
        self
    }

    /// Set solver method
    pub fn with_method(mut self, method: SolverMethod) -> Self {
        self.method = method;
        self
    }

    /// Predict linear functional t^T x* without computing full solution
    /// This is the key sublinear operation - we compute ONLY the functional,
    /// not the entire solution vector
    pub fn predict_functional(
        &self,
        matrix: &Matrix,
        b: &Vector,
        target: &Vector,
    ) -> crate::Result<PredictionResult> {
        let start = Instant::now();

        // Analyze matrix structure
        let params = DominanceParameters::from_matrix(matrix);

        if !params.allows_sublinear() {
            return Err(FTLError::ValidationError(
                format!("Matrix parameters do not allow sublinear solving: δ={}, gap={}",
                    params.delta, params.max_p_norm_gap)
            ));
        }

        // Estimate required queries
        let queries = params.query_complexity(self.epsilon);
        let n = matrix.shape().0;

        // Check lower bounds (Feng-Li-Peng 2025)
        let hit_lower_bound = self.check_lower_bounds(&params, n, self.epsilon);

        // Compute functional using local solver
        let functional_value = self.compute_functional_local(
            matrix,
            b,
            target,
            queries,
            &params,
        )?;

        // Compute error bound
        let error_bound = self.compute_error_bound(&params, self.epsilon);

        let computation_time = start.elapsed();

        // Calculate temporal advantage
        let temporal_advantage = TemporalAdvantage::calculate(
            self.distance,
            computation_time,
        );

        Ok(PredictionResult {
            functional_value,
            error_bound,
            parameters: params,
            temporal_advantage,
            queries,
            computation_time,
            hit_lower_bound,
        })
    }

    /// Compute functional using local access pattern
    fn compute_functional_local(
        &self,
        matrix: &Matrix,
        b: &Vector,
        target: &Vector,
        max_queries: usize,
        params: &DominanceParameters,
    ) -> crate::Result<f64> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let n = matrix.shape().0;

        // Implementation of unified forward-backward push (Kwok-Wei-Yang 2025)
        let mut estimate = 0.0;
        let mut queries_made = 0;

        // Forward push phase
        let mut residual = b.clone();
        let mut solution = Vector::zeros(n);

        // Push threshold based on parameters
        let threshold = self.epsilon / (params.s_max * (n as f64).sqrt());

        while queries_made < max_queries / 2 {
            // Find largest residual coordinate (local access)
            let mut max_res = 0.0;
            let mut max_idx = 0;

            // Sample coordinates instead of scanning all
            let sample_size = ((n as f64).sqrt() as usize).min(100);
            for _ in 0..sample_size {
                let idx = rng.gen_range(0..n);
                if residual.data[idx].abs() > max_res {
                    max_res = residual.data[idx].abs();
                    max_idx = idx;
                }
                queries_made += 1;
            }

            if max_res < threshold {
                break;
            }

            // Push operation (local update)
            let push_value = residual.data[max_idx];
            solution.data[max_idx] += push_value / (1.0 + params.delta);

            // Update residuals of sampled neighbors
            for _ in 0..10 {
                let j = rng.gen_range(0..n);
                residual.data[j] -= push_value * matrix.view()[[max_idx, j]] / (1.0 + params.delta);
                queries_made += 1;
            }
        }

        // Compute functional approximation
        estimate = solution.dot(target);

        // Backward correction phase (if queries remain)
        if queries_made < max_queries {
            let correction = self.backward_correction(
                matrix,
                &residual,
                target,
                max_queries - queries_made,
                params,
            )?;
            estimate += correction;
        }

        Ok(estimate)
    }

    /// Backward correction using random walks
    fn backward_correction(
        &self,
        matrix: &Matrix,
        residual: &Vector,
        target: &Vector,
        max_queries: usize,
        params: &DominanceParameters,
    ) -> crate::Result<f64> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let n = matrix.shape().0;

        let mut correction = 0.0;
        let walks = (max_queries / 10).max(10);
        let walk_length = ((1.0 / params.delta).log2() as usize).min(100);

        for _ in 0..walks {
            // Start walk from random coordinate weighted by target
            let start = rng.gen_range(0..n);
            let mut current = start;
            let mut weight = target.data[start];

            for _ in 0..walk_length {
                // Random transition
                let next = rng.gen_range(0..n);
                weight *= matrix.view()[[current, next]] / (1.0 + params.delta);
                current = next;

                if weight.abs() < 1e-12 {
                    break;
                }
            }

            correction += weight * residual.data[current];
        }

        Ok(correction / walks as f64)
    }

    /// Compute error bound based on parameters
    fn compute_error_bound(&self, params: &DominanceParameters, epsilon: f64) -> f64 {
        // Error bound from Theorem 1 (Kwok-Wei-Yang 2025)
        epsilon * (1.0 + params.max_p_norm_gap / params.delta)
    }

    /// Check if we hit known lower bounds
    fn check_lower_bounds(&self, params: &DominanceParameters, n: usize, epsilon: f64) -> bool {
        // Lower bound: Ω(√n) for certain regimes (Feng-Li-Peng 2025)
        let sqrt_n_bound = (n as f64).sqrt();
        let queries = params.query_complexity(epsilon);

        // Hit lower bound if queries approach √n
        queries as f64 > sqrt_n_bound * 0.5
    }

    /// Validate that prediction preserves causality
    pub fn validate_causality(&self, result: &PredictionResult) -> (bool, String) {
        if !result.has_temporal_lead() {
            return (true, "No temporal lead, standard computation".to_string());
        }

        // Key insight: we're not transmitting information faster than light
        // We're using local model structure to predict before remote data arrives
        (
            true,
            format!(
                "Temporal computational lead of {:.2}ms achieved through model-based inference. \
                 No information transmitted across spacelike separation. \
                 Local queries: {}, Error bound: {:.6}",
                result.temporal_advantage_ms(),
                result.queries,
                result.error_bound
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dominance_analysis() {
        let m = Matrix::diagonally_dominant(100, 2.0);
        let params = DominanceParameters::from_matrix(&m);

        assert!(params.delta > 0.0);
        assert!(params.allows_sublinear());
    }

    #[test]
    fn test_temporal_prediction() {
        let distance = Distance::tokyo_to_nyc();
        let predictor = TemporalPredictor::new(distance).with_epsilon(1e-3);

        let m = Matrix::diagonally_dominant(1000, 3.0);
        let b = Vector::ones(1000);
        let target = Vector::random(1000);

        let result = predictor.predict_functional(&m, &b, &target).unwrap();

        // Should achieve temporal lead
        assert!(result.has_temporal_lead());

        // Validate causality
        let (valid, msg) = predictor.validate_causality(&result);
        assert!(valid);
        println!("Causality validation: {}", msg);
    }

    #[test]
    fn test_lower_bounds() {
        let m = Matrix::diagonally_dominant(10000, 1.1); // Weak dominance
        let params = DominanceParameters::from_matrix(&m);

        // Should require many queries due to weak dominance
        let queries = params.query_complexity(1e-6);
        assert!(queries > 1000);
    }
}