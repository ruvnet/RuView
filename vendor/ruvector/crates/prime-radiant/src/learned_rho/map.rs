//! Learned restriction map implementation.

use super::config::{Activation, RestrictionMapConfig};
use super::error::{LearnedRhoError, LearnedRhoResult};
use super::training::{ReplayBuffer, TrainingBatch, TrainingMetrics, TrainingResult};
use std::time::Instant;

/// State of the learned restriction map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapState {
    /// Map is uninitialized.
    Uninitialized,
    /// Map is ready for inference and training.
    Ready,
    /// Map is in training mode.
    Training,
    /// Map is consolidating (computing Fisher information).
    Consolidating,
}

/// A simple dense layer.
#[derive(Debug, Clone)]
struct DenseLayer {
    weights: Vec<Vec<f32>>, // [output_dim][input_dim]
    biases: Vec<f32>,       // [output_dim]
    weight_gradients: Vec<Vec<f32>>,
    bias_gradients: Vec<f32>,
    input_cache: Vec<f32>, // For backprop
    pre_activation_cache: Vec<f32>,
    activation: Activation,
}

impl DenseLayer {
    fn new(input_dim: usize, output_dim: usize, activation: Activation) -> Self {
        // Xavier initialization
        let scale = (2.0 / (input_dim + output_dim) as f32).sqrt();

        let mut weights = vec![vec![0.0; input_dim]; output_dim];
        let biases = vec![0.0; output_dim];

        // Simple deterministic initialization
        for (i, row) in weights.iter_mut().enumerate() {
            for (j, w) in row.iter_mut().enumerate() {
                // Use a simple hash-based initialization
                let seed = (i * 1000 + j) as f32;
                *w = ((seed * 0.618033988749).fract() * 2.0 - 1.0) * scale;
            }
        }

        Self {
            weights,
            biases,
            weight_gradients: vec![vec![0.0; input_dim]; output_dim],
            bias_gradients: vec![0.0; output_dim],
            input_cache: vec![0.0; input_dim],
            pre_activation_cache: vec![0.0; output_dim],
            activation,
        }
    }

    fn forward(&mut self, input: &[f32]) -> Vec<f32> {
        self.input_cache.copy_from_slice(input);

        let mut output = vec![0.0; self.biases.len()];

        for (i, (weights_row, &bias)) in self.weights.iter().zip(self.biases.iter()).enumerate() {
            let mut sum = bias;
            for (w, &x) in weights_row.iter().zip(input.iter()) {
                sum += w * x;
            }
            self.pre_activation_cache[i] = sum;
            output[i] = self.activation.apply(sum);
        }

        output
    }

    fn backward(&mut self, upstream_grad: &[f32]) -> Vec<f32> {
        let mut downstream_grad = vec![0.0; self.input_cache.len()];

        for (i, &up_grad) in upstream_grad.iter().enumerate() {
            let act_grad = self.activation.derivative(self.pre_activation_cache[i]);
            let local_grad = up_grad * act_grad;

            // Accumulate weight gradients
            for (j, &x) in self.input_cache.iter().enumerate() {
                self.weight_gradients[i][j] += local_grad * x;
            }
            self.bias_gradients[i] += local_grad;

            // Compute downstream gradient
            for (j, w) in self.weights[i].iter().enumerate() {
                downstream_grad[j] += local_grad * w;
            }
        }

        downstream_grad
    }

    fn apply_gradients(&mut self, lr: f32, weight_decay: f32) {
        for (weights_row, grads_row) in self
            .weights
            .iter_mut()
            .zip(self.weight_gradients.iter_mut())
        {
            for (w, g) in weights_row.iter_mut().zip(grads_row.iter_mut()) {
                *w -= lr * (*g + weight_decay * *w);
                *g = 0.0; // Reset gradient
            }
        }

        for (b, g) in self.biases.iter_mut().zip(self.bias_gradients.iter_mut()) {
            *b -= lr * *g;
            *g = 0.0;
        }
    }

    fn gradient_norm(&self) -> f32 {
        let mut sum = 0.0;
        for row in &self.weight_gradients {
            for &g in row {
                sum += g * g;
            }
        }
        for &g in &self.bias_gradients {
            sum += g * g;
        }
        sum.sqrt()
    }
}

/// EWC (Elastic Weight Consolidation) state.
#[derive(Debug, Clone)]
struct EwcState {
    /// Fisher information diagonal.
    fisher: Vec<f32>,
    /// Optimal weights from previous task.
    optimal_weights: Vec<f32>,
    /// Lambda (importance weight).
    lambda: f32,
    /// Whether EWC is active.
    active: bool,
}

impl EwcState {
    fn new(num_params: usize, lambda: f32) -> Self {
        Self {
            fisher: vec![0.0; num_params],
            optimal_weights: vec![0.0; num_params],
            lambda,
            active: false,
        }
    }

    fn compute_ewc_loss(&self, current_weights: &[f32]) -> f32 {
        if !self.active {
            return 0.0;
        }

        let mut loss = 0.0;
        for ((f, opt), curr) in self
            .fisher
            .iter()
            .zip(self.optimal_weights.iter())
            .zip(current_weights.iter())
        {
            let diff = curr - opt;
            loss += f * diff * diff;
        }
        loss * self.lambda * 0.5
    }
}

/// Learned restriction map using a simple neural network.
///
/// This maps source node states to a shared space for coherence checking.
/// The projection is learned from known-coherent examples.
pub struct LearnedRestrictionMap {
    /// Configuration.
    config: RestrictionMapConfig,
    /// Neural network layers.
    layers: Vec<DenseLayer>,
    /// Replay buffer for experience replay.
    replay: ReplayBuffer,
    /// EWC state for preventing catastrophic forgetting.
    ewc: EwcState,
    /// Current state.
    state: MapState,
    /// Training step counter.
    training_step: usize,
    /// Total samples trained on.
    total_samples: usize,
}

impl LearnedRestrictionMap {
    /// Create a new learned restriction map.
    pub fn new(config: RestrictionMapConfig) -> LearnedRhoResult<Self> {
        config
            .validate()
            .map_err(LearnedRhoError::InvalidConfiguration)?;

        let mut layers = Vec::with_capacity(config.num_layers + 1);

        // Input -> Hidden
        layers.push(DenseLayer::new(
            config.input_dim,
            config.hidden_dim,
            config.activation,
        ));

        // Hidden layers
        for _ in 1..config.num_layers {
            layers.push(DenseLayer::new(
                config.hidden_dim,
                config.hidden_dim,
                config.activation,
            ));
        }

        // Hidden -> Output (no activation on output)
        layers.push(DenseLayer::new(
            config.hidden_dim,
            config.output_dim,
            Activation::None,
        ));

        // Count total parameters for EWC
        let num_params: usize = layers
            .iter()
            .map(|l| l.weights.iter().map(|r| r.len()).sum::<usize>() + l.biases.len())
            .sum();

        let replay = ReplayBuffer::new(config.replay_capacity);
        let ewc = EwcState::new(num_params, config.ewc_lambda);

        Ok(Self {
            config,
            layers,
            replay,
            ewc,
            state: MapState::Ready,
            training_step: 0,
            total_samples: 0,
        })
    }

    /// Create with default configuration.
    pub fn default_map() -> LearnedRhoResult<Self> {
        Self::new(RestrictionMapConfig::default())
    }

    /// Get the current state.
    pub fn state(&self) -> MapState {
        self.state
    }

    /// Get input dimension.
    pub fn input_dim(&self) -> usize {
        self.config.input_dim
    }

    /// Get output dimension.
    pub fn output_dim(&self) -> usize {
        self.config.output_dim
    }

    /// Apply the learned restriction map (forward pass).
    pub fn apply(&mut self, input: &[f32]) -> LearnedRhoResult<Vec<f32>> {
        if input.len() != self.config.input_dim {
            return Err(LearnedRhoError::dim_mismatch(
                self.config.input_dim,
                input.len(),
            ));
        }

        let mut x = input.to_vec();

        for layer in &mut self.layers {
            x = layer.forward(&x);
        }

        Ok(x)
    }

    /// Train on a single example.
    pub fn train_single(
        &mut self,
        source: &[f32],
        _target: &[f32],
        expected_residual: &[f32],
    ) -> LearnedRhoResult<TrainingMetrics> {
        if source.len() != self.config.input_dim {
            return Err(LearnedRhoError::dim_mismatch(
                self.config.input_dim,
                source.len(),
            ));
        }
        if expected_residual.len() != self.config.output_dim {
            return Err(LearnedRhoError::dim_mismatch(
                self.config.output_dim,
                expected_residual.len(),
            ));
        }

        self.state = MapState::Training;

        // Forward pass
        let output = self.apply(source)?;

        // Compute loss (MSE between output and expected residual)
        let mut loss = 0.0;
        let mut grad = vec![0.0; self.config.output_dim];

        for (i, (&o, &e)) in output.iter().zip(expected_residual.iter()).enumerate() {
            let diff = o - e;
            loss += diff * diff;
            grad[i] = 2.0 * diff / self.config.output_dim as f32; // dL/do
        }
        loss /= self.config.output_dim as f32;

        // Backward pass
        let mut upstream_grad = grad;
        for layer in self.layers.iter_mut().rev() {
            upstream_grad = layer.backward(&upstream_grad);
        }

        // Compute gradient norm
        let gradient_norm: f32 = self
            .layers
            .iter()
            .map(|l| l.gradient_norm())
            .sum::<f32>()
            .sqrt();

        // Get current learning rate
        let lr = self.config.scheduler.get_lr(self.training_step);

        // Apply gradients
        for layer in &mut self.layers {
            layer.apply_gradients(lr, self.config.weight_decay);
        }

        // EWC loss (placeholder - actual implementation would need weight extraction)
        let ewc_loss = 0.0;

        self.training_step += 1;
        self.total_samples += 1;
        self.state = MapState::Ready;

        Ok(TrainingMetrics::new(
            loss,
            ewc_loss,
            gradient_norm,
            lr,
            1,
            self.training_step,
        ))
    }

    /// Train on a batch of examples.
    pub fn train_batch(&mut self, batch: &TrainingBatch) -> LearnedRhoResult<TrainingMetrics> {
        if batch.is_empty() {
            return Err(LearnedRhoError::training("empty batch"));
        }

        self.state = MapState::Training;

        let mut total_loss = 0.0;
        let mut total_grad_norm = 0.0;

        for i in 0..batch.len() {
            let metrics = self.train_single(
                &batch.sources[i],
                &batch.targets[i],
                &batch.expected_residuals[i],
            )?;
            total_loss += metrics.loss;
            total_grad_norm += metrics.gradient_norm;
        }

        let n = batch.len() as f32;
        let lr = self.config.scheduler.get_lr(self.training_step);

        self.state = MapState::Ready;

        Ok(TrainingMetrics::new(
            total_loss / n,
            0.0,
            total_grad_norm / n,
            lr,
            batch.len(),
            self.training_step,
        ))
    }

    /// Add an experience to the replay buffer.
    pub fn add_experience(&mut self, source: Vec<f32>, target: Vec<f32>, expected: Vec<f32>) {
        self.replay.add(source, target, expected);
    }

    /// Train using experience replay.
    pub fn train_from_replay(&mut self) -> LearnedRhoResult<TrainingMetrics> {
        if self.replay.is_empty() {
            return Err(LearnedRhoError::training("replay buffer empty"));
        }

        let batch = self.replay.sample(self.config.batch_size);
        self.train_batch(&batch)
    }

    /// Consolidate knowledge (compute Fisher information for EWC).
    pub fn consolidate(&mut self) -> LearnedRhoResult<()> {
        self.state = MapState::Consolidating;

        // In a full implementation, we would:
        // 1. Extract all weights into a flat vector
        // 2. Compute gradients on a sample of data
        // 3. Compute Fisher information diagonal
        // 4. Store optimal weights

        self.ewc.active = true;
        self.state = MapState::Ready;

        Ok(())
    }

    /// Train for one epoch using replay buffer.
    pub fn train_epoch(&mut self, epoch: usize) -> LearnedRhoResult<TrainingResult> {
        let start = Instant::now();
        let mut metrics_list = Vec::new();

        let num_batches = self.replay.len() / self.config.batch_size;

        for _ in 0..num_batches.max(1) {
            let metrics = self.train_from_replay()?;
            metrics_list.push(metrics);
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(TrainingResult::from_metrics(
            &metrics_list,
            epoch,
            duration_ms,
        ))
    }

    /// Get map statistics.
    pub fn stats(&self) -> MapStats {
        MapStats {
            state: self.state,
            input_dim: self.config.input_dim,
            output_dim: self.config.output_dim,
            num_layers: self.layers.len(),
            training_step: self.training_step,
            total_samples: self.total_samples,
            replay_size: self.replay.len(),
            ewc_active: self.ewc.active,
        }
    }

    /// Reset the map (reinitialize weights).
    pub fn reset(&mut self) -> LearnedRhoResult<()> {
        *self = Self::new(self.config.clone())?;
        Ok(())
    }
}

impl std::fmt::Debug for LearnedRestrictionMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LearnedRestrictionMap")
            .field("state", &self.state)
            .field("input_dim", &self.config.input_dim)
            .field("output_dim", &self.config.output_dim)
            .field("training_step", &self.training_step)
            .finish()
    }
}

/// Map statistics.
#[derive(Debug, Clone, Copy)]
pub struct MapStats {
    /// Current state.
    pub state: MapState,
    /// Input dimension.
    pub input_dim: usize,
    /// Output dimension.
    pub output_dim: usize,
    /// Number of layers.
    pub num_layers: usize,
    /// Training step counter.
    pub training_step: usize,
    /// Total samples trained.
    pub total_samples: usize,
    /// Replay buffer size.
    pub replay_size: usize,
    /// Whether EWC is active.
    pub ewc_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_creation() {
        let config = RestrictionMapConfig::small();
        let map = LearnedRestrictionMap::new(config).unwrap();
        assert_eq!(map.state(), MapState::Ready);
    }

    #[test]
    fn test_forward_pass() {
        let config = RestrictionMapConfig::small();
        let mut map = LearnedRestrictionMap::new(config).unwrap();

        let input = vec![1.0; 32];
        let output = map.apply(&input).unwrap();

        assert_eq!(output.len(), 16);
    }

    #[test]
    fn test_dimension_mismatch() {
        let config = RestrictionMapConfig::small();
        let mut map = LearnedRestrictionMap::new(config).unwrap();

        let wrong_input = vec![1.0; 64]; // Wrong dimension
        let result = map.apply(&wrong_input);

        assert!(result.is_err());
    }

    #[test]
    fn test_training() {
        let config = RestrictionMapConfig::small();
        let mut map = LearnedRestrictionMap::new(config).unwrap();

        let source = vec![1.0; 32];
        let target = vec![2.0; 32];
        let expected = vec![0.1; 16];

        let metrics = map.train_single(&source, &target, &expected).unwrap();

        assert!(metrics.loss >= 0.0);
        assert_eq!(metrics.batch_size, 1);
    }

    #[test]
    fn test_replay_buffer_training() {
        let config = RestrictionMapConfig::small();
        let mut map = LearnedRestrictionMap::new(config).unwrap();

        // Add some experiences
        for _ in 0..20 {
            map.add_experience(vec![1.0; 32], vec![2.0; 32], vec![0.1; 16]);
        }

        let metrics = map.train_from_replay().unwrap();
        assert!(metrics.batch_size > 0);
    }
}
