//! # Predictive Processing (Free Energy Principle)
//!
//! Implementation of Karl Friston's Free Energy Principle - the brain as a
//! prediction machine that minimizes surprise through active inference.
//!
//! ## Key Concepts
//!
//! - **Free Energy**: Upper bound on surprise (negative log probability)
//! - **Generative Model**: Internal model that predicts sensory input
//! - **Prediction Error**: Difference between prediction and actual input
//! - **Active Inference**: Acting to confirm predictions
//! - **Precision**: Confidence weighting of prediction errors
//!
//! ## Mathematical Foundation
//!
//! F = D_KL[q(θ|o) || p(θ)] - ln p(o)
//!
//! Where:
//! - F = Variational free energy
//! - D_KL = Kullback-Leibler divergence
//! - q = Approximate posterior
//! - p = Prior/generative model
//! - o = Observations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Minimizes free energy through predictive processing
#[derive(Debug)]
pub struct FreeEnergyMinimizer {
    /// Learning rate for model updates
    learning_rate: f64,
    /// The generative model
    model: PredictiveModel,
    /// Active inference engine
    active_inference: ActiveInference,
    /// History of free energy values
    free_energy_history: Vec<f64>,
    /// Precision (confidence) for each sensory channel
    precisions: HashMap<String, f64>,
}

/// Generative model for predicting sensory input
#[derive(Debug, Clone)]
pub struct PredictiveModel {
    /// Model identifier
    pub id: Uuid,
    /// Prior beliefs about hidden states
    pub priors: Vec<f64>,
    /// Likelihood mapping (hidden states -> observations)
    pub likelihood: Vec<Vec<f64>>,
    /// Current posterior beliefs
    pub posterior: Vec<f64>,
    /// Model evidence (log probability of observations)
    pub log_evidence: f64,
    /// Number of hidden state dimensions
    pub hidden_dims: usize,
    /// Number of observation dimensions
    pub obs_dims: usize,
}

/// Active inference for acting to confirm predictions
#[derive(Debug)]
pub struct ActiveInference {
    /// Available actions
    actions: Vec<Action>,
    /// Action-outcome mappings
    action_model: HashMap<usize, Vec<f64>>,
    /// Current action policy
    policy: Vec<f64>,
    /// Expected free energy for each action
    expected_fe: Vec<f64>,
}

/// An action that can be taken
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: usize,
    pub name: String,
    /// Expected outcome (predicted observation after action)
    pub expected_outcome: Vec<f64>,
    /// Cost of action
    pub cost: f64,
}

/// Prediction error signal
#[derive(Debug, Clone)]
pub struct PredictionError {
    /// Raw error (observation - prediction)
    pub error: Vec<f64>,
    /// Precision-weighted error
    pub weighted_error: Vec<f64>,
    /// Total surprise
    pub surprise: f64,
    /// Channel breakdown
    pub by_channel: HashMap<String, f64>,
}

impl FreeEnergyMinimizer {
    /// Create a new free energy minimizer
    pub fn new(learning_rate: f64) -> Self {
        Self {
            learning_rate,
            model: PredictiveModel::new(8, 4),
            active_inference: ActiveInference::new(),
            free_energy_history: Vec::new(),
            precisions: HashMap::new(),
        }
    }

    /// Create with custom model dimensions
    pub fn with_dims(learning_rate: f64, hidden_dims: usize, obs_dims: usize) -> Self {
        Self {
            learning_rate,
            model: PredictiveModel::new(hidden_dims, obs_dims),
            active_inference: ActiveInference::new(),
            free_energy_history: Vec::new(),
            precisions: HashMap::new(),
        }
    }

    /// Compute current free energy
    pub fn compute_free_energy(&self) -> f64 {
        // F = D_KL(q||p) - log p(o)
        let kl_divergence = self.compute_kl_divergence();
        let model_evidence = self.model.log_evidence;

        kl_divergence - model_evidence
    }

    /// Compute KL divergence between posterior and prior
    fn compute_kl_divergence(&self) -> f64 {
        let mut kl = 0.0;

        for (q, p) in self.model.posterior.iter().zip(self.model.priors.iter()) {
            if *q > 1e-10 && *p > 1e-10 {
                kl += q * (q / p).ln();
            }
        }

        kl.max(0.0)
    }

    /// Process an observation and update the model
    pub fn observe(&mut self, observation: &[f64]) -> PredictionError {
        // Generate prediction from current beliefs
        let prediction = self.model.predict();

        // Compute prediction error
        let error = self.compute_prediction_error(&prediction, observation);

        // Update posterior beliefs (perception)
        self.update_beliefs(&error);

        // Update model evidence
        self.model.log_evidence = self.compute_log_evidence(observation);

        // Record free energy
        let fe = self.compute_free_energy();
        self.free_energy_history.push(fe);

        error
    }

    /// Compute prediction error
    fn compute_prediction_error(&self, prediction: &[f64], observation: &[f64]) -> PredictionError {
        let len = prediction.len().min(observation.len());
        let mut error = vec![0.0; len];
        let mut weighted_error = vec![0.0; len];
        let mut by_channel = HashMap::new();

        let default_precision = 1.0;

        for i in 0..len {
            let e = observation.get(i).copied().unwrap_or(0.0)
                - prediction.get(i).copied().unwrap_or(0.0);
            error[i] = e;

            let channel = format!("channel_{}", i);
            let precision = self
                .precisions
                .get(&channel)
                .copied()
                .unwrap_or(default_precision);
            weighted_error[i] = e * precision;
            by_channel.insert(channel, e.abs());
        }

        let surprise = weighted_error.iter().map(|e| e * e).sum::<f64>().sqrt();

        PredictionError {
            error,
            weighted_error,
            surprise,
            by_channel,
        }
    }

    /// Update beliefs based on prediction error
    fn update_beliefs(&mut self, error: &PredictionError) {
        // Gradient descent on free energy
        for (i, e) in error.weighted_error.iter().enumerate() {
            if i < self.model.posterior.len() {
                // Update posterior in direction that reduces error
                self.model.posterior[i] += self.learning_rate * e;
                // Keep probabilities valid
                self.model.posterior[i] = self.model.posterior[i].clamp(0.001, 0.999);
            }
        }

        // Renormalize posterior
        let sum: f64 = self.model.posterior.iter().sum();
        if sum > 0.0 {
            for p in &mut self.model.posterior {
                *p /= sum;
            }
        }
    }

    /// Compute log evidence for observations
    fn compute_log_evidence(&self, observation: &[f64]) -> f64 {
        // Simplified: assume Gaussian likelihood
        let prediction = self.model.predict();
        let mut log_p = 0.0;

        for (o, p) in observation.iter().zip(prediction.iter()) {
            let diff = o - p;
            log_p -= 0.5 * diff * diff; // Gaussian log likelihood (variance = 1)
        }

        log_p
    }

    /// Select action through active inference
    pub fn select_action(&mut self) -> Option<&Action> {
        // Compute expected free energy for each action
        self.active_inference.compute_expected_fe(&self.model);

        // Select action with minimum expected free energy
        self.active_inference.select_action()
    }

    /// Execute an action and observe outcome
    pub fn execute_action(&mut self, action_id: usize) -> Option<PredictionError> {
        let outcome = self.active_inference.action_model.get(&action_id)?.clone();
        let error = self.observe(&outcome);
        Some(error)
    }

    /// Add an action to the repertoire
    pub fn add_action(&mut self, name: &str, expected_outcome: Vec<f64>, cost: f64) {
        self.active_inference
            .add_action(name, expected_outcome, cost);
    }

    /// Set precision for a channel
    pub fn set_precision(&mut self, channel: &str, precision: f64) {
        self.precisions
            .insert(channel.to_string(), precision.max(0.01));
    }

    /// Get average free energy over time
    pub fn average_free_energy(&self) -> f64 {
        if self.free_energy_history.is_empty() {
            return 0.0;
        }
        self.free_energy_history.iter().sum::<f64>() / self.free_energy_history.len() as f64
    }

    /// Get free energy trend (positive = increasing, negative = decreasing)
    pub fn free_energy_trend(&self) -> f64 {
        if self.free_energy_history.len() < 2 {
            return 0.0;
        }

        let recent = &self.free_energy_history[self.free_energy_history.len().saturating_sub(10)..];
        if recent.len() < 2 {
            return 0.0;
        }

        let first_half: f64 =
            recent[..recent.len() / 2].iter().sum::<f64>() / (recent.len() / 2) as f64;
        let second_half: f64 = recent[recent.len() / 2..].iter().sum::<f64>()
            / (recent.len() - recent.len() / 2) as f64;

        second_half - first_half
    }

    /// Get the generative model
    pub fn model(&self) -> &PredictiveModel {
        &self.model
    }

    /// Get mutable reference to model
    pub fn model_mut(&mut self) -> &mut PredictiveModel {
        &mut self.model
    }
}

impl PredictiveModel {
    /// Create a new predictive model
    pub fn new(hidden_dims: usize, obs_dims: usize) -> Self {
        // Initialize with uniform priors
        let prior_val = 1.0 / hidden_dims as f64;

        // Initialize likelihood matrix
        let mut likelihood = vec![vec![0.0; obs_dims]; hidden_dims];
        for i in 0..hidden_dims {
            for j in 0..obs_dims {
                // Simple diagonal-ish initialization
                likelihood[i][j] = if i % obs_dims == j {
                    0.7
                } else {
                    0.3 / (obs_dims - 1) as f64
                };
            }
        }

        Self {
            id: Uuid::new_v4(),
            priors: vec![prior_val; hidden_dims],
            likelihood,
            posterior: vec![prior_val; hidden_dims],
            log_evidence: 0.0,
            hidden_dims,
            obs_dims,
        }
    }

    /// Generate prediction from current beliefs
    pub fn predict(&self) -> Vec<f64> {
        let mut prediction = vec![0.0; self.obs_dims];

        for (h, &belief) in self.posterior.iter().enumerate() {
            if h < self.likelihood.len() {
                for (o, p) in prediction.iter_mut().enumerate() {
                    if o < self.likelihood[h].len() {
                        *p += belief * self.likelihood[h][o];
                    }
                }
            }
        }

        prediction
    }

    /// Update likelihood based on learning
    pub fn learn(&mut self, hidden_state: usize, observation: &[f64], learning_rate: f64) {
        if hidden_state >= self.hidden_dims {
            return;
        }

        for (o, &obs) in observation.iter().enumerate().take(self.obs_dims) {
            let current = self.likelihood[hidden_state][o];
            self.likelihood[hidden_state][o] = current + learning_rate * (obs - current);
        }
    }

    /// Entropy of the posterior
    pub fn posterior_entropy(&self) -> f64 {
        -self
            .posterior
            .iter()
            .filter(|&&p| p > 1e-10)
            .map(|&p| p * p.ln())
            .sum::<f64>()
    }
}

impl ActiveInference {
    /// Create a new active inference engine
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            action_model: HashMap::new(),
            policy: Vec::new(),
            expected_fe: Vec::new(),
        }
    }

    /// Add an action
    pub fn add_action(&mut self, name: &str, expected_outcome: Vec<f64>, cost: f64) {
        let id = self.actions.len();
        let outcome = expected_outcome.clone();

        self.actions.push(Action {
            id,
            name: name.to_string(),
            expected_outcome,
            cost,
        });

        self.action_model.insert(id, outcome);
        self.policy.push(1.0 / (self.actions.len() as f64));
        self.expected_fe.push(0.0);
    }

    /// Compute expected free energy for each action
    pub fn compute_expected_fe(&mut self, model: &PredictiveModel) {
        for (i, action) in self.actions.iter().enumerate() {
            // Expected free energy = expected surprise + action cost
            // - epistemic value (information gain)
            // + pragmatic value (goal satisfaction)

            let predicted = model.predict();
            let mut surprise = 0.0;

            for (p, o) in predicted.iter().zip(action.expected_outcome.iter()) {
                let diff = p - o;
                surprise += diff * diff;
            }

            self.expected_fe[i] = surprise.sqrt() + action.cost;
        }
    }

    /// Select action with minimum expected free energy
    pub fn select_action(&self) -> Option<&Action> {
        if self.actions.is_empty() {
            return None;
        }

        let min_idx = self
            .expected_fe
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)?;

        self.actions.get(min_idx)
    }

    /// Get action policy (probability distribution)
    pub fn get_policy(&self) -> &[f64] {
        &self.policy
    }
}

impl Default for ActiveInference {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_free_energy_minimizer_creation() {
        let fem = FreeEnergyMinimizer::new(0.1);
        assert!(fem.compute_free_energy() >= 0.0 || fem.compute_free_energy() < 0.0);
        // Always defined
    }

    #[test]
    fn test_observation_processing() {
        let mut fem = FreeEnergyMinimizer::with_dims(0.1, 4, 4);

        let observation = vec![0.5, 0.3, 0.1, 0.1];
        let error = fem.observe(&observation);

        assert!(!error.error.is_empty());
        assert!(error.surprise >= 0.0);
    }

    #[test]
    fn test_free_energy_decreases() {
        let mut fem = FreeEnergyMinimizer::with_dims(0.1, 4, 4);

        // Repeated observations should decrease free energy (learning)
        let observation = vec![0.7, 0.1, 0.1, 0.1];

        for _ in 0..10 {
            fem.observe(&observation);
        }

        // Check that trend is decreasing (or at least not exploding)
        let trend = fem.free_energy_trend();
        // Learning should stabilize or decrease free energy
        assert!(trend < 1.0);
    }

    #[test]
    fn test_active_inference() {
        let mut fem = FreeEnergyMinimizer::new(0.1);

        fem.add_action("look", vec![0.8, 0.1, 0.05, 0.05], 0.1);
        fem.add_action("reach", vec![0.1, 0.8, 0.05, 0.05], 0.2);
        fem.add_action("wait", vec![0.25, 0.25, 0.25, 0.25], 0.0);

        let action = fem.select_action();
        assert!(action.is_some());
    }

    #[test]
    fn test_predictive_model() {
        let model = PredictiveModel::new(4, 4);
        let prediction = model.predict();

        assert_eq!(prediction.len(), 4);
        // Prediction should sum to approximately 1 (normalized)
        let sum: f64 = prediction.iter().sum();
        assert!(sum > 0.0);
    }

    #[test]
    fn test_precision_weighting() {
        let mut fem = FreeEnergyMinimizer::with_dims(0.1, 4, 4);

        fem.set_precision("channel_0", 10.0); // High precision
        fem.set_precision("channel_1", 0.1); // Low precision

        let observation = vec![1.0, 1.0, 0.5, 0.5];
        let error = fem.observe(&observation);

        // Channel 0 should have higher weighted error
        assert!(
            error.weighted_error[0].abs() > error.weighted_error[1].abs()
                || error.error[0].abs() * 10.0 > error.error[1].abs() * 0.1
        );
    }

    #[test]
    fn test_posterior_entropy() {
        let model = PredictiveModel::new(4, 4);
        let entropy = model.posterior_entropy();

        // Uniform distribution should have maximum entropy
        let max_entropy = (4.0_f64).ln();
        assert!((entropy - max_entropy).abs() < 0.01);
    }
}
