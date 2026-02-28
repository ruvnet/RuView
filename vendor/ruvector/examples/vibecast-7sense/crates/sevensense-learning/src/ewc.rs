//! Elastic Weight Consolidation (EWC) for continual learning.
//!
//! EWC prevents catastrophic forgetting by regularizing weight updates
//! based on their importance for previously learned tasks.
//!
//! Reference: Kirkpatrick et al., "Overcoming catastrophic forgetting in neural networks", PNAS 2017

use serde::{Deserialize, Serialize};

use crate::infrastructure::gnn_model::GnnModel;

/// Fisher Information matrix (diagonal approximation).
///
/// Stores the importance weights for each parameter in the model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FisherInformation {
    /// Diagonal elements of the Fisher matrix per layer
    pub diagonal: Vec<Vec<f32>>,
    /// Optional parameter names for debugging
    #[serde(default)]
    pub param_names: Vec<String>,
    /// Number of samples used to compute Fisher
    pub num_samples: usize,
}

impl FisherInformation {
    /// Create a new Fisher information structure
    #[must_use]
    pub fn new(num_layers: usize) -> Self {
        Self {
            diagonal: vec![Vec::new(); num_layers],
            param_names: Vec::new(),
            num_samples: 0,
        }
    }

    /// Create from parameter gradients (diagonal approximation)
    ///
    /// F_ii = E[(∂L/∂θ_i)²]
    #[must_use]
    pub fn from_gradients(gradients: &[Vec<f32>]) -> Self {
        let diagonal: Vec<Vec<f32>> = gradients
            .iter()
            .map(|grads| grads.iter().map(|g| g * g).collect())
            .collect();

        Self {
            diagonal,
            param_names: Vec::new(),
            num_samples: 1,
        }
    }

    /// Update Fisher with new gradient samples (online estimation)
    pub fn update(&mut self, gradients: &[Vec<f32>]) {
        let n = self.num_samples as f32;

        for (layer_idx, grads) in gradients.iter().enumerate() {
            if layer_idx >= self.diagonal.len() {
                self.diagonal.push(grads.iter().map(|g| g * g).collect());
            } else {
                // Running average: F_new = (n * F_old + g²) / (n + 1)
                for (i, &g) in grads.iter().enumerate() {
                    if i >= self.diagonal[layer_idx].len() {
                        self.diagonal[layer_idx].push(g * g);
                    } else {
                        let old_val = self.diagonal[layer_idx][i];
                        self.diagonal[layer_idx][i] = (n * old_val + g * g) / (n + 1.0);
                    }
                }
            }
        }

        self.num_samples += 1;
    }

    /// Get the importance of a parameter
    #[must_use]
    pub fn get_importance(&self, layer_idx: usize, param_idx: usize) -> f32 {
        self.diagonal
            .get(layer_idx)
            .and_then(|layer| layer.get(param_idx))
            .copied()
            .unwrap_or(0.0)
    }

    /// Get total importance (sum of all diagonal elements)
    #[must_use]
    pub fn total_importance(&self) -> f32 {
        self.diagonal.iter().flat_map(|l| l.iter()).sum()
    }

    /// Normalize Fisher information
    pub fn normalize(&mut self) {
        let total = self.total_importance();
        if total > 1e-10 {
            for layer in &mut self.diagonal {
                for val in layer.iter_mut() {
                    *val /= total;
                }
            }
        }
    }

    /// Apply decay to Fisher information (for progressive consolidation)
    pub fn decay(&mut self, factor: f32) {
        for layer in &mut self.diagonal {
            for val in layer.iter_mut() {
                *val *= factor;
            }
        }
    }
}

/// State snapshot for EWC regularization.
///
/// Stores the model parameters and their importance from previous tasks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EwcState {
    /// Optimal parameters for previous task(s)
    pub optimal_params: Vec<Vec<f32>>,
    /// Fisher information (importance weights)
    pub fisher: FisherInformation,
    /// Task identifier
    pub task_id: Option<String>,
    /// Number of tasks consolidated
    pub num_tasks: usize,
}

impl EwcState {
    /// Create a new EWC state from model parameters and Fisher information
    #[must_use]
    pub fn new(params: Vec<f32>, fisher: FisherInformation) -> Self {
        Self {
            optimal_params: vec![params],
            fisher,
            task_id: None,
            num_tasks: 1,
        }
    }

    /// Create from model with computed Fisher
    #[must_use]
    pub fn from_model(model: &GnnModel, gradients: &[Vec<f32>]) -> Self {
        let params = model.get_parameters();
        let fisher = FisherInformation::from_gradients(gradients);

        Self::new(params, fisher)
    }

    /// Merge with another EWC state (for multi-task learning)
    pub fn merge(&mut self, other: &EwcState) {
        // Add new optimal params
        self.optimal_params.extend(other.optimal_params.clone());
        self.num_tasks += other.num_tasks;

        // Merge Fisher information (average)
        for (layer_idx, other_layer) in other.fisher.diagonal.iter().enumerate() {
            if layer_idx >= self.fisher.diagonal.len() {
                self.fisher.diagonal.push(other_layer.clone());
            } else {
                for (i, &val) in other_layer.iter().enumerate() {
                    if i >= self.fisher.diagonal[layer_idx].len() {
                        self.fisher.diagonal[layer_idx].push(val);
                    } else {
                        // Average Fisher values
                        let old_n = (self.num_tasks - other.num_tasks) as f32;
                        let new_n = other.num_tasks as f32;
                        let total_n = self.num_tasks as f32;

                        self.fisher.diagonal[layer_idx][i] =
                            (old_n * self.fisher.diagonal[layer_idx][i] + new_n * val) / total_n;
                    }
                }
            }
        }
    }
}

/// EWC regularizer for computing the penalty term.
#[derive(Debug, Clone)]
pub struct EwcRegularizer {
    /// Lambda coefficient (importance of old task knowledge)
    lambda: f32,
    /// Optional per-layer lambda scaling
    layer_lambdas: Option<Vec<f32>>,
}

impl Default for EwcRegularizer {
    fn default() -> Self {
        Self {
            lambda: 5000.0,
            layer_lambdas: None,
        }
    }
}

impl EwcRegularizer {
    /// Create a new EWC regularizer with the given lambda
    #[must_use]
    pub fn new(lambda: f32) -> Self {
        Self {
            lambda,
            layer_lambdas: None,
        }
    }

    /// Create with per-layer lambda scaling
    #[must_use]
    pub fn with_layer_lambdas(mut self, lambdas: Vec<f32>) -> Self {
        self.layer_lambdas = Some(lambdas);
        self
    }

    /// Get lambda for a specific layer
    fn get_layer_lambda(&self, layer_idx: usize) -> f32 {
        self.layer_lambdas
            .as_ref()
            .and_then(|l| l.get(layer_idx))
            .copied()
            .unwrap_or(self.lambda)
    }

    /// Compute the EWC penalty term.
    ///
    /// L_EWC = (λ/2) * Σ F_i * (θ_i - θ*_i)²
    ///
    /// # Arguments
    /// * `model` - Current model
    /// * `ewc_state` - Saved EWC state from previous task
    ///
    /// # Returns
    /// The EWC penalty value to add to the loss
    #[must_use]
    pub fn compute_penalty(&self, model: &GnnModel, ewc_state: &EwcState) -> f32 {
        let current_params = model.get_parameters();

        // Flatten optimal params (use the most recent one if multiple)
        let optimal_params = ewc_state
            .optimal_params
            .last()
            .map(|p| p.as_slice())
            .unwrap_or(&[]);

        if current_params.len() != optimal_params.len() {
            return 0.0;
        }

        // Compute penalty: (λ/2) * Σ F_i * (θ_i - θ*_i)²
        let mut penalty = 0.0;
        let mut param_idx = 0;

        for (layer_idx, layer_fisher) in ewc_state.fisher.diagonal.iter().enumerate() {
            let _layer_lambda = self.get_layer_lambda(layer_idx);

            for &fisher_val in layer_fisher {
                if param_idx < current_params.len() && param_idx < optimal_params.len() {
                    let diff = current_params[param_idx] - optimal_params[param_idx];
                    penalty += fisher_val * diff * diff;
                }
                param_idx += 1;
            }
        }

        (self.lambda / 2.0) * penalty
    }

    /// Compute EWC gradient contribution.
    ///
    /// ∂L_EWC/∂θ_i = λ * F_i * (θ_i - θ*_i)
    #[must_use]
    pub fn compute_gradient(&self, model: &GnnModel, ewc_state: &EwcState) -> Vec<f32> {
        let current_params = model.get_parameters();

        let optimal_params = ewc_state
            .optimal_params
            .last()
            .map(|p| p.as_slice())
            .unwrap_or(&[]);

        let mut gradient = vec![0.0; current_params.len()];

        if current_params.len() != optimal_params.len() {
            return gradient;
        }

        let mut param_idx = 0;

        for (layer_idx, layer_fisher) in ewc_state.fisher.diagonal.iter().enumerate() {
            let layer_lambda = self.get_layer_lambda(layer_idx);

            for &fisher_val in layer_fisher {
                if param_idx < current_params.len() {
                    let diff = current_params[param_idx] - optimal_params[param_idx];
                    gradient[param_idx] = layer_lambda * fisher_val * diff;
                }
                param_idx += 1;
            }
        }

        gradient
    }
}

/// Online EWC implementation (for streaming/continual learning).
///
/// Uses a running estimate of the Fisher information.
#[derive(Debug, Clone)]
pub struct OnlineEwc {
    /// Current EWC state
    state: Option<EwcState>,
    /// EWC lambda
    lambda: f32,
    /// Gamma for Fisher decay (between tasks)
    gamma: f32,
}

impl OnlineEwc {
    /// Create a new Online EWC instance
    #[must_use]
    pub fn new(lambda: f32, gamma: f32) -> Self {
        Self {
            state: None,
            lambda,
            gamma,
        }
    }

    /// Update the EWC state after completing a task
    pub fn update(&mut self, model: &GnnModel, gradients: &[Vec<f32>]) {
        let new_fisher = FisherInformation::from_gradients(gradients);
        let params = model.get_parameters();

        if let Some(ref mut state) = self.state {
            // Decay old Fisher
            state.fisher.decay(self.gamma);

            // Add new Fisher (scaled by (1 - gamma))
            for (layer_idx, new_layer) in new_fisher.diagonal.iter().enumerate() {
                if layer_idx >= state.fisher.diagonal.len() {
                    state.fisher.diagonal.push(new_layer.clone());
                } else {
                    for (i, &val) in new_layer.iter().enumerate() {
                        if i >= state.fisher.diagonal[layer_idx].len() {
                            state.fisher.diagonal[layer_idx].push((1.0 - self.gamma) * val);
                        } else {
                            state.fisher.diagonal[layer_idx][i] += (1.0 - self.gamma) * val;
                        }
                    }
                }
            }

            // Update optimal params (keep only the latest)
            state.optimal_params = vec![params];
            state.num_tasks += 1;
        } else {
            self.state = Some(EwcState::new(params, new_fisher));
        }
    }

    /// Compute the EWC penalty
    #[must_use]
    pub fn compute_penalty(&self, model: &GnnModel) -> f32 {
        if let Some(ref state) = self.state {
            let regularizer = EwcRegularizer::new(self.lambda);
            regularizer.compute_penalty(model, state)
        } else {
            0.0
        }
    }

    /// Compute the EWC gradient contribution
    #[must_use]
    pub fn compute_gradient(&self, model: &GnnModel) -> Vec<f32> {
        if let Some(ref state) = self.state {
            let regularizer = EwcRegularizer::new(self.lambda);
            regularizer.compute_gradient(model, state)
        } else {
            vec![0.0; model.get_parameters().len()]
        }
    }

    /// Get the current state
    #[must_use]
    pub fn state(&self) -> Option<&EwcState> {
        self.state.as_ref()
    }
}

/// Progress & Compress (P&C) - improved EWC variant.
///
/// Alternates between progress (learning new task) and compress (consolidation).
#[derive(Debug, Clone)]
pub struct ProgressAndCompress {
    /// Knowledge base (compressed knowledge from all tasks)
    knowledge_base: Option<EwcState>,
    /// Active column (for current task)
    active_params: Option<Vec<f32>>,
    /// EWC lambda
    lambda: f32,
}

impl ProgressAndCompress {
    /// Create a new P&C instance
    #[must_use]
    pub fn new(lambda: f32) -> Self {
        Self {
            knowledge_base: None,
            active_params: None,
            lambda,
        }
    }

    /// Begin progress phase (start learning new task)
    pub fn begin_progress(&mut self, model: &GnnModel) {
        self.active_params = Some(model.get_parameters());
    }

    /// End progress phase and begin compress
    pub fn compress(&mut self, model: &GnnModel, gradients: &[Vec<f32>]) {
        let fisher = FisherInformation::from_gradients(gradients);
        let params = model.get_parameters();
        let new_state = EwcState::new(params, fisher);

        if let Some(ref mut kb) = self.knowledge_base {
            kb.merge(&new_state);
        } else {
            self.knowledge_base = Some(new_state);
        }

        self.active_params = None;
    }

    /// Compute penalty during progress phase
    #[must_use]
    pub fn compute_penalty(&self, model: &GnnModel) -> f32 {
        if let Some(ref kb) = self.knowledge_base {
            let regularizer = EwcRegularizer::new(self.lambda);
            regularizer.compute_penalty(model, kb)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GnnModelType, LearningConfig};

    #[test]
    fn test_fisher_information() {
        let mut fisher = FisherInformation::new(2);

        // Update with gradients
        fisher.update(&[vec![1.0, 2.0], vec![3.0]]);
        fisher.update(&[vec![2.0, 1.0], vec![4.0]]);

        assert_eq!(fisher.num_samples, 2);
        assert_eq!(fisher.diagonal.len(), 2);

        // Check averaging
        assert!(fisher.get_importance(0, 0) > 0.0);
    }

    #[test]
    fn test_fisher_from_gradients() {
        let gradients = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0]];
        let fisher = FisherInformation::from_gradients(&gradients);

        assert_eq!(fisher.diagonal[0], vec![1.0, 4.0, 9.0]);
        assert_eq!(fisher.diagonal[1], vec![16.0, 25.0]);
    }

    #[test]
    fn test_fisher_normalize() {
        let mut fisher = FisherInformation::from_gradients(&[vec![3.0, 4.0]]);
        fisher.normalize();

        let total: f32 = fisher.diagonal.iter().flat_map(|l| l.iter()).sum();
        assert!((total - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_ewc_state() {
        let params = vec![1.0, 2.0, 3.0];
        let fisher = FisherInformation::from_gradients(&[vec![0.1, 0.2, 0.3]]);

        let state = EwcState::new(params.clone(), fisher);
        assert_eq!(state.optimal_params.len(), 1);
        assert_eq!(state.num_tasks, 1);
    }

    #[test]
    fn test_ewc_state_merge() {
        let mut state1 = EwcState::new(
            vec![1.0, 2.0],
            FisherInformation::from_gradients(&[vec![0.1, 0.2]]),
        );

        let state2 = EwcState::new(
            vec![1.5, 2.5],
            FisherInformation::from_gradients(&[vec![0.3, 0.4]]),
        );

        state1.merge(&state2);
        assert_eq!(state1.num_tasks, 2);
        assert_eq!(state1.optimal_params.len(), 2);
    }

    #[test]
    fn test_ewc_regularizer() {
        let mut config = LearningConfig::default();
        config.input_dim = 4;
        config.output_dim = 2;
        config.hyperparameters.num_layers = 1;
        config.hyperparameters.hidden_dim = 4;

        let model = crate::infrastructure::gnn_model::GnnModel::new(
            GnnModelType::Gcn,
            4, 2, 1, 4, 1, 0.0,
        );

        let params = model.get_parameters();
        let fisher = FisherInformation::from_gradients(&[vec![0.1; params.len()]]);
        let ewc_state = EwcState::new(params.clone(), fisher);

        let regularizer = EwcRegularizer::new(1000.0);
        let penalty = regularizer.compute_penalty(&model, &ewc_state);

        // Penalty should be 0 when params haven't changed
        assert!(penalty.abs() < 1e-6);
    }

    #[test]
    fn test_ewc_gradient() {
        let model = crate::infrastructure::gnn_model::GnnModel::new(
            GnnModelType::Gcn,
            4, 2, 1, 4, 1, 0.0,
        );

        // Create state with slightly different params
        let mut optimal_params = model.get_parameters();
        for p in &mut optimal_params {
            *p += 0.1;
        }

        let fisher = FisherInformation::from_gradients(&[vec![1.0; optimal_params.len()]]);
        let ewc_state = EwcState::new(optimal_params, fisher);

        let regularizer = EwcRegularizer::new(1.0);
        let gradient = regularizer.compute_gradient(&model, &ewc_state);

        // Gradient should push towards optimal params
        assert!(!gradient.is_empty());
        for &g in &gradient {
            // Gradient should be non-zero since params differ
            assert!(g.abs() > 0.0);
        }
    }

    #[test]
    fn test_online_ewc() {
        let model = crate::infrastructure::gnn_model::GnnModel::new(
            GnnModelType::Gcn,
            4, 2, 1, 4, 1, 0.0,
        );

        let mut online = OnlineEwc::new(1000.0, 0.9);

        // Initially no penalty
        assert_eq!(online.compute_penalty(&model), 0.0);

        // Update after task 1
        let gradients = vec![vec![0.1; 20]];
        online.update(&model, &gradients);

        assert!(online.state().is_some());
        assert_eq!(online.state().unwrap().num_tasks, 1);
    }

    #[test]
    fn test_progress_and_compress() {
        let model = crate::infrastructure::gnn_model::GnnModel::new(
            GnnModelType::Gcn,
            4, 2, 1, 4, 1, 0.0,
        );

        let mut pc = ProgressAndCompress::new(1000.0);

        // Begin progress
        pc.begin_progress(&model);
        assert!(pc.active_params.is_some());

        // Compress
        let gradients = vec![vec![0.1; 20]];
        pc.compress(&model, &gradients);

        assert!(pc.active_params.is_none());
        assert!(pc.knowledge_base.is_some());
    }

    #[test]
    fn test_fisher_decay() {
        let mut fisher = FisherInformation::from_gradients(&[vec![1.0, 2.0]]);
        fisher.decay(0.5);

        assert_eq!(fisher.diagonal[0], vec![0.5, 2.0]);
    }
}
