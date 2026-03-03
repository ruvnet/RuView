//! Strange Loop Operator with Contraction for Temporal Consciousness
//!
//! This module implements the strange loop operator that creates self-referential
//! patterns necessary for consciousness emergence. The operator uses contraction
//! mapping with Lipschitz constant < 1 to ensure convergence.

use std::collections::VecDeque;
use super::TemporalResult;

/// Metrics for contraction convergence analysis
#[derive(Debug, Clone, Default)]
pub struct ContractionMetrics {
    pub iterations_to_convergence: usize,
    pub convergence_rate: f64,
    pub lipschitz_constant: f64,
    pub final_fixed_point: Vec<f64>,
    pub contraction_achieved: bool,
    pub stability_measure: f64,
}

/// Strange loop state representing self-referential structure
#[derive(Debug, Clone)]
struct LoopState {
    pub level: usize,
    pub state_vector: Vec<f64>,
    pub self_reference: f64,
    pub emergence_factor: f64,
    pub timestamp: u64,
}

/// Contraction mapping parameters
#[derive(Debug, Clone)]
struct ContractionParams {
    pub lipschitz_bound: f64,
    pub convergence_threshold: f64,
    pub max_iterations: usize,
    pub stability_window: usize,
}

/// Strange Loop Operator implementing self-referential consciousness patterns
pub struct StrangeLoopOperator {
    params: ContractionParams,
    loop_states: VecDeque<LoopState>,
    metrics: ContractionMetrics,
    fixed_point: Vec<f64>,
    iteration_count: u64,
    contraction_history: VecDeque<f64>,
    
    // Self-reference tracking
    self_reference_strength: f64,
    emergence_level: f64,
    loop_depth: usize,
}

impl StrangeLoopOperator {
    /// Create a new strange loop operator
    pub fn new(lipschitz_bound: f64, max_iterations: usize) -> Self {
        Self {
            params: ContractionParams {
                lipschitz_bound: lipschitz_bound.min(0.99), // Ensure < 1
                convergence_threshold: 1e-6,
                max_iterations,
                stability_window: 10,
            },
            loop_states: VecDeque::with_capacity(1000),
            metrics: ContractionMetrics::default(),
            fixed_point: Vec::new(),
            iteration_count: 0,
            contraction_history: VecDeque::with_capacity(1000),
            
            self_reference_strength: 0.0,
            emergence_level: 0.0,
            loop_depth: 0,
        }
    }
    
    /// Process one iteration of the strange loop with contraction
    pub fn process_iteration(&mut self, time: f64, state: &[f64]) -> TemporalResult<ContractionMetrics> {
        self.iteration_count += 1;
        
        // Create new loop state with self-reference
        let loop_state = self.create_loop_state(time, state)?;
        
        // Apply contraction mapping
        let contracted_state = self.apply_contraction_mapping(&loop_state.state_vector)?;
        
        // Update fixed point estimate
        self.update_fixed_point(&contracted_state)?;
        
        // Check convergence
        let convergence_info = self.check_convergence(&contracted_state)?;
        
        // Update self-reference and emergence
        self.update_self_reference(&loop_state)?;
        self.update_emergence_level(&contracted_state)?;
        
        // Store state history
        self.store_loop_state(loop_state);
        
        // Update metrics
        self.update_metrics(convergence_info)?;
        
        Ok(self.metrics.clone())
    }
    
    /// Get current contraction metrics
    pub fn get_metrics(&self) -> &ContractionMetrics {
        &self.metrics
    }
    
    /// Get current emergence level
    pub fn get_emergence_level(&self) -> f64 {
        self.emergence_level
    }
    
    /// Get self-reference strength
    pub fn get_self_reference_strength(&self) -> f64 {
        self.self_reference_strength
    }
    
    /// Get current loop depth
    pub fn get_loop_depth(&self) -> usize {
        self.loop_depth
    }
    
    /// Get fixed point estimate
    pub fn get_fixed_point(&self) -> &[f64] {
        &self.fixed_point
    }
    
    /// Reset the operator state
    pub fn reset(&mut self) {
        self.loop_states.clear();
        self.fixed_point.clear();
        self.iteration_count = 0;
        self.contraction_history.clear();
        self.self_reference_strength = 0.0;
        self.emergence_level = 0.0;
        self.loop_depth = 0;
        self.metrics = ContractionMetrics::default();
    }
    
    /// Force convergence check (for testing)
    pub fn force_convergence_check(&mut self, state: &[f64]) -> TemporalResult<bool> {
        let convergence_info = self.check_convergence(state)?;
        Ok(convergence_info.converged)
    }
    
    // Private helper methods
    
    fn create_loop_state(&mut self, time: f64, state: &[f64]) -> TemporalResult<LoopState> {
        // Calculate self-reference by looking at state history
        let self_ref = self.calculate_self_reference(state)?;
        
        // Calculate emergence factor based on loop complexity
        let emergence = self.calculate_emergence_factor(state)?;
        
        // Determine current loop depth
        self.loop_depth = self.calculate_loop_depth(state);
        
        Ok(LoopState {
            level: self.loop_depth,
            state_vector: state.to_vec(),
            self_reference: self_ref,
            emergence_factor: emergence,
            timestamp: time as u64,
        })
    }
    
    fn apply_contraction_mapping(&self, state: &[f64]) -> TemporalResult<Vec<f64>> {
        if state.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut contracted = Vec::with_capacity(state.len());
        
        for (i, &value) in state.iter().enumerate() {
            // Apply contraction with self-reference
            let self_ref_component = if i < self.loop_states.len() {
                self.loop_states[i % self.loop_states.len()].self_reference
            } else {
                0.0
            };
            
            // Contraction mapping: f(x) = L * x + c, where L < 1
            let contracted_value = self.params.lipschitz_bound * value 
                + (1.0 - self.params.lipschitz_bound) * self_ref_component;
            
            // Apply strange loop transformation
            let loop_transformed = self.apply_strange_loop_transform(contracted_value, i);
            
            contracted.push(loop_transformed);
        }
        
        Ok(contracted)
    }
    
    fn apply_strange_loop_transform(&self, value: f64, index: usize) -> f64 {
        // Strange loop: the output influences the input through self-reference
        let loop_factor = (self.self_reference_strength * (index as f64 + 1.0).ln()).sin();
        let self_modulation = 1.0 + 0.1 * loop_factor;
        
        // Apply bounded transformation to maintain stability
        let transformed = value * self_modulation;
        transformed.tanh() // Bounded between -1 and 1
    }
    
    fn update_fixed_point(&mut self, contracted_state: &[f64]) -> TemporalResult<()> {
        if self.fixed_point.is_empty() {
            self.fixed_point = contracted_state.to_vec();
        } else {
            // Update fixed point estimate using exponential moving average
            let alpha = 0.1; // Learning rate
            
            for (i, &new_value) in contracted_state.iter().enumerate() {
                if i < self.fixed_point.len() {
                    self.fixed_point[i] = (1.0 - alpha) * self.fixed_point[i] + alpha * new_value;
                } else {
                    self.fixed_point.push(new_value);
                }
            }
        }
        
        Ok(())
    }
    
    fn check_convergence(&mut self, state: &[f64]) -> TemporalResult<ConvergenceInfo> {
        if self.fixed_point.is_empty() || state.len() != self.fixed_point.len() {
            return Ok(ConvergenceInfo {
                converged: false,
                distance: f64::INFINITY,
                iterations: self.iteration_count as usize,
            });
        }
        
        // Calculate L2 distance to fixed point
        let distance: f64 = state.iter()
            .zip(self.fixed_point.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        
        self.contraction_history.push_back(distance);
        if self.contraction_history.len() > 1000 {
            self.contraction_history.pop_front();
        }
        
        let converged = distance < self.params.convergence_threshold;
        
        Ok(ConvergenceInfo {
            converged,
            distance,
            iterations: self.iteration_count as usize,
        })
    }
    
    fn calculate_self_reference(&self, state: &[f64]) -> TemporalResult<f64> {
        if self.loop_states.is_empty() {
            return Ok(0.0);
        }
        
        // Calculate correlation with previous states
        let mut total_correlation = 0.0;
        let mut count = 0;
        
        for prev_state in self.loop_states.iter().rev().take(10) {
            if prev_state.state_vector.len() == state.len() {
                let correlation = self.calculate_correlation(&prev_state.state_vector, state)?;
                total_correlation += correlation * prev_state.emergence_factor;
                count += 1;
            }
        }
        
        if count > 0 {
            Ok(total_correlation / count as f64)
        } else {
            Ok(0.0)
        }
    }
    
    fn calculate_emergence_factor(&self, state: &[f64]) -> TemporalResult<f64> {
        if state.is_empty() {
            return Ok(0.0);
        }
        
        // Calculate emergence based on state complexity and self-reference
        let complexity = self.calculate_state_complexity(state);
        let self_ref_factor = self.self_reference_strength;
        let loop_depth_factor = (self.loop_depth as f64).ln().max(0.0);
        
        let emergence = (complexity * (1.0 + self_ref_factor) * (1.0 + loop_depth_factor)).tanh();
        Ok(emergence)
    }
    
    fn calculate_loop_depth(&self, state: &[f64]) -> usize {
        // Calculate how deep the self-reference goes
        let mut depth = 0;
        let threshold = 0.1;
        
        for prev_state in self.loop_states.iter().rev() {
            if prev_state.state_vector.len() == state.len() {
                let correlation = self.calculate_correlation(&prev_state.state_vector, state)
                    .unwrap_or(0.0);
                
                if correlation > threshold {
                    depth += 1;
                } else {
                    break;
                }
            }
            
            if depth > 100 { // Limit depth for performance
                break;
            }
        }
        
        depth
    }
    
    fn calculate_correlation(&self, state1: &[f64], state2: &[f64]) -> TemporalResult<f64> {
        if state1.len() != state2.len() || state1.is_empty() {
            return Ok(0.0);
        }
        
        let mean1: f64 = state1.iter().sum::<f64>() / state1.len() as f64;
        let mean2: f64 = state2.iter().sum::<f64>() / state2.len() as f64;
        
        let mut numerator = 0.0;
        let mut sum_sq1 = 0.0;
        let mut sum_sq2 = 0.0;
        
        for (v1, v2) in state1.iter().zip(state2.iter()) {
            let diff1 = v1 - mean1;
            let diff2 = v2 - mean2;
            
            numerator += diff1 * diff2;
            sum_sq1 += diff1 * diff1;
            sum_sq2 += diff2 * diff2;
        }
        
        let denominator = (sum_sq1 * sum_sq2).sqrt();
        
        if denominator > 0.0 {
            Ok(numerator / denominator)
        } else {
            Ok(0.0)
        }
    }
    
    fn calculate_state_complexity(&self, state: &[f64]) -> f64 {
        if state.is_empty() {
            return 0.0;
        }
        
        // Calculate entropy-based complexity measure
        let mut complexity = 0.0;
        
        // Variance component
        let mean: f64 = state.iter().sum::<f64>() / state.len() as f64;
        let variance: f64 = state.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / state.len() as f64;
        
        complexity += variance.sqrt();
        
        // Information content component
        for &value in state {
            if value.abs() > 1e-10 {
                complexity += -value.abs().ln() / state.len() as f64;
            }
        }
        
        complexity.min(10.0) // Bound complexity
    }
    
    fn update_self_reference(&mut self, loop_state: &LoopState) -> TemporalResult<()> {
        // Update self-reference strength based on loop state
        let alpha = 0.05; // Learning rate
        self.self_reference_strength = (1.0 - alpha) * self.self_reference_strength 
            + alpha * loop_state.self_reference;
        
        // Bound self-reference strength
        self.self_reference_strength = self.self_reference_strength.clamp(0.0, 1.0);
        
        Ok(())
    }
    
    fn update_emergence_level(&mut self, contracted_state: &[f64]) -> TemporalResult<()> {
        // Calculate new emergence level
        let new_emergence = self.calculate_emergence_factor(contracted_state)?;
        
        // Smooth update
        let alpha = 0.1;
        self.emergence_level = (1.0 - alpha) * self.emergence_level + alpha * new_emergence;
        
        Ok(())
    }
    
    fn store_loop_state(&mut self, state: LoopState) {
        self.loop_states.push_back(state);
        
        // Keep history bounded
        while self.loop_states.len() > 1000 {
            self.loop_states.pop_front();
        }
    }
    
    fn update_metrics(&mut self, convergence_info: ConvergenceInfo) -> TemporalResult<()> {
        self.metrics.iterations_to_convergence = convergence_info.iterations;
        self.metrics.contraction_achieved = convergence_info.converged;
        
        // Calculate convergence rate from recent history
        if self.contraction_history.len() >= 2 {
            let recent_distances: Vec<f64> = self.contraction_history.iter()
                .rev()
                .take(10)
                .cloned()
                .collect();
            
            if recent_distances.len() >= 2 {
                let rate = recent_distances[0] / recent_distances[recent_distances.len() - 1].max(1e-10);
                self.metrics.convergence_rate = rate.min(1.0);
            }
        }
        
        self.metrics.lipschitz_constant = self.params.lipschitz_bound;
        self.metrics.final_fixed_point = self.fixed_point.clone();
        
        // Calculate stability measure
        self.metrics.stability_measure = self.calculate_stability_measure();
        
        Ok(())
    }
    
    fn calculate_stability_measure(&self) -> f64 {
        if self.contraction_history.len() < self.params.stability_window {
            return 0.0;
        }
        
        let recent_distances: Vec<f64> = self.contraction_history.iter()
            .rev()
            .take(self.params.stability_window)
            .cloned()
            .collect();
        
        let mean: f64 = recent_distances.iter().sum::<f64>() / recent_distances.len() as f64;
        let variance: f64 = recent_distances.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / recent_distances.len() as f64;
        
        // Stability is inverse of variance (lower variance = higher stability)
        1.0 / (1.0 + variance)
    }
}

/// Convergence information for internal use
#[derive(Debug, Clone)]
struct ConvergenceInfo {
    converged: bool,
    distance: f64,
    iterations: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_strange_loop_creation() {
        let operator = StrangeLoopOperator::new(0.9, 100);
        assert_eq!(operator.params.lipschitz_bound, 0.9);
        assert_eq!(operator.params.max_iterations, 100);
        assert_eq!(operator.emergence_level, 0.0);
    }
    
    #[test]
    fn test_contraction_mapping() {
        let mut operator = StrangeLoopOperator::new(0.8, 100);
        let state = vec![1.0, 2.0, 3.0];
        
        let contracted = operator.apply_contraction_mapping(&state).unwrap();
        assert_eq!(contracted.len(), state.len());
        
        // Contracted values should be bounded
        for &value in &contracted {
            assert!(value.abs() <= 1.0);
        }
    }
    
    #[test]
    fn test_convergence_detection() {
        let mut operator = StrangeLoopOperator::new(0.9, 100);
        let state = vec![0.1, 0.1, 0.1];
        
        // Set up fixed point
        operator.fixed_point = vec![0.1, 0.1, 0.1];
        
        let convergence = operator.check_convergence(&state).unwrap();
        assert!(convergence.converged);
        assert!(convergence.distance < 1e-6);
    }
    
    #[test]
    fn test_self_reference_calculation() {
        let mut operator = StrangeLoopOperator::new(0.9, 100);
        let state = vec![1.0, 2.0, 3.0];
        
        // Process a few iterations to build history
        for i in 0..5 {
            operator.process_iteration(i as f64, &state).unwrap();
        }
        
        assert!(operator.self_reference_strength > 0.0);
        assert!(operator.emergence_level >= 0.0);
    }
    
    #[test]
    fn test_loop_depth_calculation() {
        let mut operator = StrangeLoopOperator::new(0.9, 100);
        let state = vec![1.0, 1.0, 1.0];
        
        // Process multiple iterations with similar states
        for _ in 0..10 {
            operator.process_iteration(0.0, &state).unwrap();
        }
        
        assert!(operator.get_loop_depth() > 0);
    }
    
    #[test]
    fn test_emergence_level_growth() {
        let mut operator = StrangeLoopOperator::new(0.9, 100);
        let mut state = vec![0.5, 0.5, 0.5];
        
        let initial_emergence = operator.get_emergence_level();
        
        // Process iterations with evolving state
        for i in 0..20 {
            state[0] += 0.01 * (i as f64).sin();
            operator.process_iteration(i as f64, &state).unwrap();
        }
        
        let final_emergence = operator.get_emergence_level();
        assert!(final_emergence >= initial_emergence);
    }
    
    #[test]
    fn test_metrics_update() {
        let mut operator = StrangeLoopOperator::new(0.9, 100);
        let state = vec![1.0, 2.0, 3.0];
        
        let metrics = operator.process_iteration(0.0, &state).unwrap();
        
        assert!(metrics.iterations_to_convergence > 0);
        assert!(metrics.lipschitz_constant == 0.9);
        assert!(metrics.stability_measure >= 0.0);
    }
}