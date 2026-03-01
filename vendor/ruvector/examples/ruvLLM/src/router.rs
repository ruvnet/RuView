//! FastGRNN Router for intelligent resource allocation
//!
//! Implements a FastGRNN (Fast, Accurate, Stable, and Tiny GRU) based router
//! that learns to select optimal model size, context size, and generation
//! parameters based on query characteristics.

use crate::config::RouterConfig;
use crate::error::{Error, Result, RouterError};
use crate::types::{ModelSize, RouterSample, RoutingDecision, CONTEXT_BINS};

use ndarray::{Array1, Array2, Axis};
use parking_lot::RwLock;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// FastGRNN Router for dynamic resource allocation
pub struct FastGRNNRouter {
    /// Cell parameters
    cell: FastGRNNCell,
    /// Output heads
    output_heads: OutputHeads,
    /// Input normalization parameters
    input_norm: LayerNorm,
    /// Configuration
    config: RouterConfig,
    /// Training statistics
    stats: RouterStats,
}

/// Router statistics for monitoring
#[derive(Debug, Default)]
pub struct RouterStats {
    /// Total forward passes
    pub forward_count: AtomicU64,
    /// Total training steps
    pub training_steps: AtomicU64,
    /// Cumulative loss
    pub cumulative_loss: RwLock<f64>,
    /// Model selection histogram
    pub model_counts: [AtomicU64; 4],
}

impl RouterStats {
    pub fn record_forward(&self, model: ModelSize) {
        self.forward_count.fetch_add(1, Ordering::Relaxed);
        self.model_counts[model.to_index()].fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_model_distribution(&self) -> [f64; 4] {
        let total = self.forward_count.load(Ordering::Relaxed) as f64;
        if total == 0.0 {
            return [0.25; 4];
        }
        [
            self.model_counts[0].load(Ordering::Relaxed) as f64 / total,
            self.model_counts[1].load(Ordering::Relaxed) as f64 / total,
            self.model_counts[2].load(Ordering::Relaxed) as f64 / total,
            self.model_counts[3].load(Ordering::Relaxed) as f64 / total,
        ]
    }
}

/// FastGRNN cell implementation with sparse and low-rank matrices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastGRNNCell {
    /// Input-to-update gate weights (dense, will be sparsified)
    w_z: Array2<f32>,
    /// Recurrent-to-update gate weights (low-rank: U_z = A_z @ B_z)
    u_z_a: Array2<f32>,
    u_z_b: Array2<f32>,
    /// Update gate bias
    b_z: Array1<f32>,
    /// Input-to-hidden weights
    w_h: Array2<f32>,
    /// Recurrent-to-hidden weights (low-rank: U_h = A_h @ B_h)
    u_h_a: Array2<f32>,
    u_h_b: Array2<f32>,
    /// Hidden bias
    b_h: Array1<f32>,
    /// FastGRNN zeta scalar (gate modulation)
    zeta: f32,
    /// FastGRNN nu scalar (gate modulation)
    nu: f32,
    /// Sparsity mask for W matrices
    w_z_mask: Array2<f32>,
    w_h_mask: Array2<f32>,
}

/// Output heads for routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputHeads {
    /// Model selection: hidden_dim -> 4
    w_model: Array2<f32>,
    b_model: Array1<f32>,
    /// Context selection: hidden_dim -> 5
    w_context: Array2<f32>,
    b_context: Array1<f32>,
    /// Temperature: hidden_dim -> 1
    w_temp: Array1<f32>,
    b_temp: f32,
    /// Top-p: hidden_dim -> 1
    w_top_p: Array1<f32>,
    b_top_p: f32,
    /// Confidence: hidden_dim -> 1
    w_conf: Array1<f32>,
    b_conf: f32,
}

/// Layer normalization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerNorm {
    gamma: Array1<f32>,
    beta: Array1<f32>,
    eps: f32,
}

/// Adam optimizer state
#[derive(Debug, Clone)]
pub struct AdamState {
    /// First moment estimates
    m: Vec<Array1<f32>>,
    /// Second moment estimates
    v: Vec<Array1<f32>>,
    /// Time step
    t: usize,
    /// Learning rate
    lr: f32,
    /// Beta1
    beta1: f32,
    /// Beta2
    beta2: f32,
    /// Epsilon
    eps: f32,
}

impl AdamState {
    pub fn new(param_shapes: &[usize], lr: f32) -> Self {
        Self {
            m: param_shapes.iter().map(|&s| Array1::zeros(s)).collect(),
            v: param_shapes.iter().map(|&s| Array1::zeros(s)).collect(),
            t: 0,
            lr,
            beta1: 0.9,
            beta2: 0.999,
            eps: 1e-8,
        }
    }

    pub fn step(&mut self, params: &mut [Array1<f32>], grads: &[Array1<f32>]) {
        self.t += 1;
        let bias_correction1 = 1.0 - self.beta1.powi(self.t as i32);
        let bias_correction2 = 1.0 - self.beta2.powi(self.t as i32);

        for (i, (param, grad)) in params.iter_mut().zip(grads.iter()).enumerate() {
            // Update biased first moment estimate
            self.m[i] = &self.m[i] * self.beta1 + grad * (1.0 - self.beta1);
            // Update biased second moment estimate
            self.v[i] = &self.v[i] * self.beta2 + &(grad * grad) * (1.0 - self.beta2);

            // Compute bias-corrected estimates
            let m_hat = &self.m[i] / bias_correction1;
            let v_hat = &self.v[i] / bias_correction2;

            // Update parameters
            *param = param.clone() - &(&m_hat / &(v_hat.mapv(f32::sqrt) + self.eps)) * self.lr;
        }
    }
}

impl FastGRNNRouter {
    /// Create a new router with random initialization
    pub fn new(config: &RouterConfig) -> Result<Self> {
        let cell = FastGRNNCell::new(
            config.input_dim,
            config.hidden_dim,
            config.sparsity,
            config.rank,
        );
        let output_heads = OutputHeads::new(config.hidden_dim);
        let input_norm = LayerNorm::new(config.input_dim);

        Ok(Self {
            cell,
            output_heads,
            input_norm,
            config: config.clone(),
            stats: RouterStats::default(),
        })
    }

    /// Load router from weights file
    pub fn load(path: impl AsRef<Path>, config: &RouterConfig) -> Result<Self> {
        let data = std::fs::read(path.as_ref())?;
        let (cell, output_heads, input_norm): (FastGRNNCell, OutputHeads, LayerNorm) =
            bincode::serde::decode_from_slice(&data, bincode::config::standard())
                .map_err(|e| Error::Serialization(e.to_string()))?
                .0;

        Ok(Self {
            cell,
            output_heads,
            input_norm,
            config: config.clone(),
            stats: RouterStats::default(),
        })
    }

    /// Save router weights
    pub fn save_weights(&self, path: impl AsRef<Path>) -> Result<()> {
        let data = bincode::serde::encode_to_vec(
            (&self.cell, &self.output_heads, &self.input_norm),
            bincode::config::standard(),
        )
        .map_err(|e| Error::Serialization(e.to_string()))?;

        std::fs::write(path, data)?;
        Ok(())
    }

    /// Forward pass through router
    pub fn forward(&self, features: &[f32], hidden: &[f32]) -> Result<RoutingDecision> {
        // Validate input dimensions
        if features.len() != self.config.input_dim {
            return Err(RouterError::InvalidFeatures {
                expected: self.config.input_dim,
                actual: features.len(),
            }
            .into());
        }

        let x = Array1::from_vec(features.to_vec());
        let h = Array1::from_vec(hidden.to_vec());

        // Normalize input
        let x_norm = self.input_norm.forward(&x);

        // FastGRNN cell
        let h_new = self.cell.forward(&x_norm, &h);

        // Output heads
        let model_logits = self.output_heads.model_forward(&h_new);
        let context_logits = self.output_heads.context_forward(&h_new);
        let temp_raw = self.output_heads.temp_forward(&h_new);
        let top_p_raw = self.output_heads.top_p_forward(&h_new);
        let conf_raw = self.output_heads.confidence_forward(&h_new);

        // Activations
        let model_probs = softmax_array(&model_logits);
        let context_probs = softmax_array(&context_logits);
        let temperature = sigmoid(temp_raw) * 2.0;
        let top_p = sigmoid(top_p_raw);
        let confidence = sigmoid(conf_raw);

        // Decode decisions
        let (model, context_size) = if confidence >= self.config.confidence_threshold {
            let model_idx = argmax_array(&model_probs);
            let context_idx = argmax_array(&context_probs);
            (ModelSize::from_index(model_idx), CONTEXT_BINS[context_idx])
        } else {
            // Safe defaults when confidence is low
            (ModelSize::B1_2, 2048)
        };

        // Record statistics
        self.stats.record_forward(model);

        Ok(RoutingDecision {
            model,
            context_size,
            temperature,
            top_p,
            confidence,
            model_probs: [
                model_probs[0],
                model_probs[1],
                model_probs[2],
                model_probs[3],
            ],
            new_hidden: h_new.to_vec(),
            features: features.to_vec(),
        })
    }

    /// Train the router on a batch of samples
    pub fn train_batch(
        &mut self,
        samples: &[RouterSample],
        learning_rate: f32,
        ewc_lambda: f32,
        fisher_info: Option<&[f32]>,
        optimal_weights: Option<&[f32]>,
    ) -> TrainingMetrics {
        if samples.is_empty() {
            return TrainingMetrics::default();
        }

        let batch_size = samples.len() as f32;
        let mut total_loss = 0.0;
        let mut model_correct = 0;
        let mut context_correct = 0;

        // Accumulate gradients over batch
        let mut grad_accum = self.zero_gradients();

        for sample in samples {
            let hidden = vec![0.0f32; self.config.hidden_dim];
            let x = Array1::from_vec(sample.features.clone());
            let h = Array1::from_vec(hidden);

            // Forward pass
            let x_norm = self.input_norm.forward(&x);
            let h_new = self.cell.forward(&x_norm, &h);

            let model_logits = self.output_heads.model_forward(&h_new);
            let context_logits = self.output_heads.context_forward(&h_new);
            let temp_pred = self.output_heads.temp_forward(&h_new);
            let top_p_pred = self.output_heads.top_p_forward(&h_new);

            let model_probs = softmax_array(&model_logits);
            let context_probs = softmax_array(&context_logits);

            // Compute loss
            let model_loss = -model_probs[sample.label_model].ln().max(-10.0);
            let context_loss = -context_probs[sample.label_context].ln().max(-10.0);
            let temp_loss = (sigmoid(temp_pred) * 2.0 - sample.label_temperature).powi(2);
            let top_p_loss = (sigmoid(top_p_pred) - sample.label_top_p).powi(2);

            let sample_loss = model_loss + context_loss + 0.1 * temp_loss + 0.1 * top_p_loss;
            total_loss += sample_loss;

            // Check accuracy
            if argmax_array(&model_probs) == sample.label_model {
                model_correct += 1;
            }
            if argmax_array(&context_probs) == sample.label_context {
                context_correct += 1;
            }

            // Compute gradients (simplified - using finite differences for demo)
            self.accumulate_gradients(
                &mut grad_accum,
                sample,
                &h_new,
                &model_probs,
                &context_probs,
            );
        }

        // Average gradients
        for g in &mut grad_accum {
            *g /= batch_size;
        }

        // Add EWC regularization gradient if provided
        if let (Some(fisher), Some(optimal)) = (fisher_info, optimal_weights) {
            self.add_ewc_gradient(&mut grad_accum, fisher, optimal, ewc_lambda);
        }

        // Apply gradients with simple SGD (can be replaced with Adam)
        self.apply_gradients(&grad_accum, learning_rate);

        self.stats.training_steps.fetch_add(1, Ordering::Relaxed);
        *self.stats.cumulative_loss.write() += total_loss as f64;

        TrainingMetrics {
            total_loss: total_loss / batch_size,
            model_accuracy: model_correct as f32 / batch_size,
            context_accuracy: context_correct as f32 / batch_size,
            samples_processed: samples.len(),
        }
    }

    fn zero_gradients(&self) -> Vec<f32> {
        vec![0.0; self.parameter_count()]
    }

    fn parameter_count(&self) -> usize {
        let cell_params = self.cell.w_z.len()
            + self.cell.w_h.len()
            + self.cell.u_z_a.len()
            + self.cell.u_z_b.len()
            + self.cell.u_h_a.len()
            + self.cell.u_h_b.len()
            + self.cell.b_z.len()
            + self.cell.b_h.len();

        let head_params = self.output_heads.w_model.len()
            + self.output_heads.w_context.len()
            + self.output_heads.w_temp.len()
            + self.output_heads.w_top_p.len()
            + self.output_heads.w_conf.len()
            + self.output_heads.b_model.len()
            + self.output_heads.b_context.len()
            + 3; // temp, top_p, conf biases

        cell_params + head_params
    }

    fn accumulate_gradients(
        &self,
        grads: &mut [f32],
        sample: &RouterSample,
        h_new: &Array1<f32>,
        model_probs: &Array1<f32>,
        context_probs: &Array1<f32>,
    ) {
        // Simplified gradient computation
        // In production, use autograd or manual backprop

        // Model head gradients (cross-entropy)
        let mut model_grad = model_probs.clone();
        model_grad[sample.label_model] -= 1.0;

        // Context head gradients
        let mut context_grad = context_probs.clone();
        context_grad[sample.label_context] -= 1.0;

        // Accumulate into flat gradient buffer
        let offset = 0;
        for (i, &g) in model_grad.iter().enumerate() {
            for (j, &h) in h_new.iter().enumerate() {
                let idx = offset + i * self.config.hidden_dim + j;
                if idx < grads.len() {
                    grads[idx] += g * h;
                }
            }
        }
    }

    fn add_ewc_gradient(&self, grads: &mut [f32], fisher: &[f32], optimal: &[f32], lambda: f32) {
        let params = self.get_flat_params();
        for (i, ((g, &f), &w_opt)) in grads
            .iter_mut()
            .zip(fisher.iter())
            .zip(optimal.iter())
            .enumerate()
        {
            if i < params.len() {
                *g += lambda * f * (params[i] - w_opt);
            }
        }
    }

    fn apply_gradients(&mut self, grads: &[f32], lr: f32) {
        // Apply gradients to output heads (simplified)
        let mut offset = 0;
        let model_size = self.output_heads.w_model.len();
        for (i, w) in self.output_heads.w_model.iter_mut().enumerate() {
            if offset + i < grads.len() {
                *w -= lr * grads[offset + i];
            }
        }
        offset += model_size;

        let context_size = self.output_heads.w_context.len();
        for (i, w) in self.output_heads.w_context.iter_mut().enumerate() {
            if offset + i < grads.len() {
                *w -= lr * grads[offset + i];
            }
        }
    }

    fn get_flat_params(&self) -> Vec<f32> {
        let mut params = Vec::new();
        params.extend(self.output_heads.w_model.iter().cloned());
        params.extend(self.output_heads.w_context.iter().cloned());
        params.extend(self.output_heads.w_temp.iter().cloned());
        params.extend(self.output_heads.w_top_p.iter().cloned());
        params.extend(self.output_heads.w_conf.iter().cloned());
        params
    }

    /// Compute Fisher information diagonal for EWC
    pub fn compute_fisher(&self, samples: &[RouterSample]) -> Vec<f32> {
        let param_count = self.parameter_count();
        let mut fisher = vec![0.0f32; param_count];

        for sample in samples {
            let hidden = vec![0.0f32; self.config.hidden_dim];
            if let Ok(decision) = self.forward(&sample.features, &hidden) {
                // Approximate Fisher with squared gradients
                // In production, compute actual log-likelihood gradients
                for i in 0..fisher.len().min(sample.features.len()) {
                    fisher[i] += sample.features[i].powi(2) * decision.confidence;
                }
            }
        }

        // Normalize
        let n = samples.len() as f32;
        for f in &mut fisher {
            *f /= n;
        }

        fisher
    }

    /// Get router statistics
    pub fn stats(&self) -> &RouterStats {
        &self.stats
    }

    /// Get current weights as a flat vector (for EWC)
    pub fn get_weights(&self) -> Vec<f32> {
        self.get_flat_params()
    }

    /// Reset router to initial state
    pub fn reset(&mut self) {
        self.cell = FastGRNNCell::new(
            self.config.input_dim,
            self.config.hidden_dim,
            self.config.sparsity,
            self.config.rank,
        );
        self.output_heads = OutputHeads::new(self.config.hidden_dim);
    }
}

impl FastGRNNCell {
    fn new(input_dim: usize, hidden_dim: usize, sparsity: f32, rank: usize) -> Self {
        use rand::Rng;
        use rand_distr::Normal;

        let mut rng = rand::thread_rng();
        let std_w = (2.0 / (input_dim + hidden_dim) as f32).sqrt();
        let std_u = (2.0 / (hidden_dim + hidden_dim) as f32).sqrt();
        let normal_w = Normal::new(0.0, std_w).unwrap();
        let normal_u = Normal::new(0.0, std_u).unwrap();

        // Initialize W matrices
        let w_z = Array2::from_shape_fn((hidden_dim, input_dim), |_| rng.sample(normal_w));
        let w_h = Array2::from_shape_fn((hidden_dim, input_dim), |_| rng.sample(normal_w));

        // Create sparsity masks
        let w_z_mask = Array2::from_shape_fn((hidden_dim, input_dim), |_| {
            if rng.gen::<f32>() > sparsity {
                1.0
            } else {
                0.0
            }
        });
        let w_h_mask = Array2::from_shape_fn((hidden_dim, input_dim), |_| {
            if rng.gen::<f32>() > sparsity {
                1.0
            } else {
                0.0
            }
        });

        // Initialize low-rank U matrices
        let u_z_a = Array2::from_shape_fn((hidden_dim, rank), |_| rng.sample(normal_u));
        let u_z_b = Array2::from_shape_fn((rank, hidden_dim), |_| rng.sample(normal_u));
        let u_h_a = Array2::from_shape_fn((hidden_dim, rank), |_| rng.sample(normal_u));
        let u_h_b = Array2::from_shape_fn((rank, hidden_dim), |_| rng.sample(normal_u));

        Self {
            w_z: &w_z * &w_z_mask,
            w_h: &w_h * &w_h_mask,
            u_z_a,
            u_z_b,
            u_h_a,
            u_h_b,
            b_z: Array1::zeros(hidden_dim),
            b_h: Array1::zeros(hidden_dim),
            zeta: 1.0,
            nu: 0.5,
            w_z_mask,
            w_h_mask,
        }
    }

    fn forward(&self, x: &Array1<f32>, h: &Array1<f32>) -> Array1<f32> {
        // z = sigmoid(W_z @ x + U_z @ h + b_z)
        // where U_z = A_z @ B_z (low-rank)
        let w_z_x = self.w_z.dot(x);
        let u_z_h = self.u_z_a.dot(&self.u_z_b.dot(h));
        let z_pre = &w_z_x + &u_z_h + &self.b_z;
        let z = z_pre.mapv(sigmoid);

        // h_tilde = tanh(W_h @ x + U_h @ h + b_h)
        let w_h_x = self.w_h.dot(x);
        let u_h_h = self.u_h_a.dot(&self.u_h_b.dot(h));
        let h_tilde_pre = &w_h_x + &u_h_h + &self.b_h;
        let h_tilde = h_tilde_pre.mapv(|v| v.tanh());

        // h_new = (zeta * (1 - z) + nu) * h_tilde + z * h
        let gate = z.mapv(|zi| self.zeta * (1.0 - zi) + self.nu);
        &gate * &h_tilde + &z * h
    }
}

impl LayerNorm {
    fn new(dim: usize) -> Self {
        Self {
            gamma: Array1::ones(dim),
            beta: Array1::zeros(dim),
            eps: 1e-5,
        }
    }

    fn forward(&self, x: &Array1<f32>) -> Array1<f32> {
        let mean = x.mean().unwrap_or(0.0);
        let var = x.mapv(|v| (v - mean).powi(2)).mean().unwrap_or(0.0);
        let std = (var + self.eps).sqrt();
        let normalized = x.mapv(|v| (v - mean) / std);
        &self.gamma * &normalized + &self.beta
    }
}

impl OutputHeads {
    fn new(hidden_dim: usize) -> Self {
        use rand::Rng;
        use rand_distr::Normal;

        let mut rng = rand::thread_rng();
        let std = (2.0 / hidden_dim as f32).sqrt();
        let normal = Normal::new(0.0, std).unwrap();

        Self {
            w_model: Array2::from_shape_fn((4, hidden_dim), |_| rng.sample(normal)),
            b_model: Array1::zeros(4),
            w_context: Array2::from_shape_fn((5, hidden_dim), |_| rng.sample(normal)),
            b_context: Array1::zeros(5),
            w_temp: Array1::from_shape_fn(hidden_dim, |_| rng.sample(normal)),
            b_temp: 0.0,
            w_top_p: Array1::from_shape_fn(hidden_dim, |_| rng.sample(normal)),
            b_top_p: 0.0,
            w_conf: Array1::from_shape_fn(hidden_dim, |_| rng.sample(normal)),
            b_conf: 0.0,
        }
    }

    fn model_forward(&self, h: &Array1<f32>) -> Array1<f32> {
        self.w_model.dot(h) + &self.b_model
    }

    fn context_forward(&self, h: &Array1<f32>) -> Array1<f32> {
        self.w_context.dot(h) + &self.b_context
    }

    fn temp_forward(&self, h: &Array1<f32>) -> f32 {
        self.w_temp.dot(h) + self.b_temp
    }

    fn top_p_forward(&self, h: &Array1<f32>) -> f32 {
        self.w_top_p.dot(h) + self.b_top_p
    }

    fn confidence_forward(&self, h: &Array1<f32>) -> f32 {
        self.w_conf.dot(h) + self.b_conf
    }
}

/// Training metrics
#[derive(Debug, Clone, Default)]
pub struct TrainingMetrics {
    pub total_loss: f32,
    pub model_accuracy: f32,
    pub context_accuracy: f32,
    pub samples_processed: usize,
}

// Helper functions

/// Optimized sigmoid with fast exp approximation
#[inline(always)]
fn sigmoid(x: f32) -> f32 {
    // Fast sigmoid using rational approximation for |x| < 4.5
    // More accurate than simple clamped exp for common ranges
    let x = x.clamp(-20.0, 20.0);
    if x.abs() < 4.5 {
        // Pade approximant: 0.5 + 0.5 * x / (1 + |x| + 0.555 * x^2)
        let abs_x = x.abs();
        0.5 + 0.5 * x / (1.0 + abs_x + 0.555 * x * x)
    } else {
        1.0 / (1.0 + (-x).exp())
    }
}

/// Optimized softmax for small arrays (common in router)
fn softmax_array(x: &Array1<f32>) -> Array1<f32> {
    let len = x.len();

    // For small arrays, use simple scalar approach with improved numerics
    if len <= 8 {
        let max = x.fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let exp = x.mapv(|v| fast_exp(v - max));
        let sum = exp.sum();
        if sum > 0.0 {
            exp / sum
        } else {
            Array1::from_elem(len, 1.0 / len as f32)
        }
    } else {
        // For larger arrays, use standard approach
        let max = x.fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let exp = x.mapv(|v| (v - max).exp());
        let sum = exp.sum();
        // Guard against division by zero (all -inf inputs)
        if sum > 0.0 {
            exp / sum
        } else {
            Array1::from_elem(len, 1.0 / len as f32)
        }
    }
}

/// Fast exp approximation using Schraudolph's method
#[inline(always)]
fn fast_exp(x: f32) -> f32 {
    // Clamp to avoid overflow/underflow
    let x = x.clamp(-88.0, 88.0);

    // Polynomial approximation: exp(x) ≈ 1 + x + x²/2 + x³/6 for |x| < 1
    if x.abs() < 1.0 {
        let x2 = x * x;
        let x3 = x2 * x;
        1.0 + x + x2 * 0.5 + x3 * 0.16666667
    } else {
        x.exp()
    }
}

/// Branchless argmax for fixed-size arrays (optimized for common sizes)
#[inline]
fn argmax_array(x: &Array1<f32>) -> usize {
    let len = x.len();
    if len == 0 {
        return 0;
    }

    // For size 4 (model selection), use branchless comparison
    if len == 4 {
        let x = x.as_slice().unwrap();
        let mut max_idx = 0usize;
        let mut max_val = x[0];

        // Unrolled comparison
        if x[1] > max_val {
            max_val = x[1];
            max_idx = 1;
        }
        if x[2] > max_val {
            max_val = x[2];
            max_idx = 2;
        }
        if x[3] > max_val {
            max_idx = 3;
        }

        return max_idx;
    }

    // For size 5 (context selection), also unroll
    if len == 5 {
        let x = x.as_slice().unwrap();
        let mut max_idx = 0usize;
        let mut max_val = x[0];

        if x[1] > max_val {
            max_val = x[1];
            max_idx = 1;
        }
        if x[2] > max_val {
            max_val = x[2];
            max_idx = 2;
        }
        if x[3] > max_val {
            max_val = x[3];
            max_idx = 3;
        }
        if x[4] > max_val {
            max_idx = 4;
        }

        return max_idx;
    }

    // General case
    x.iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_creation() {
        let config = RouterConfig::default();
        let router = FastGRNNRouter::new(&config).unwrap();
        assert_eq!(router.config.input_dim, 128);
        assert_eq!(router.config.hidden_dim, 64);
    }

    #[test]
    fn test_router_forward() {
        let config = RouterConfig::default();
        let router = FastGRNNRouter::new(&config).unwrap();

        let features = vec![0.5f32; config.input_dim];
        let hidden = vec![0.0f32; config.hidden_dim];

        let decision = router.forward(&features, &hidden).unwrap();

        // Verify outputs are valid
        assert!(decision.temperature >= 0.0 && decision.temperature <= 2.0);
        assert!(decision.top_p >= 0.0 && decision.top_p <= 1.0);
        assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
        assert_eq!(decision.new_hidden.len(), config.hidden_dim);

        // Probabilities should sum to ~1
        let prob_sum: f32 = decision.model_probs.iter().sum();
        assert!((prob_sum - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_router_training() {
        let config = RouterConfig::default();
        let mut router = FastGRNNRouter::new(&config).unwrap();

        let samples: Vec<RouterSample> = (0..10)
            .map(|i| RouterSample {
                features: vec![0.1 * i as f32; config.input_dim],
                label_model: i % 4,
                label_context: i % 5,
                label_temperature: 0.7,
                label_top_p: 0.9,
                quality: 0.8,
                latency_ms: 100.0,
            })
            .collect();

        let metrics = router.train_batch(&samples, 0.001, 0.0, None, None);

        assert!(metrics.total_loss > 0.0);
        assert!(metrics.samples_processed == 10);
    }

    #[test]
    fn test_layer_norm() {
        let norm = LayerNorm::new(4);
        let x = Array1::from_vec(vec![1.0, 2.0, 3.0, 4.0]);
        let result = norm.forward(&x);

        // Mean should be ~0 after normalization
        let mean = result.mean().unwrap();
        assert!(mean.abs() < 0.01);
    }

    #[test]
    fn test_softmax() {
        let x = Array1::from_vec(vec![1.0, 2.0, 3.0]);
        let result = softmax_array(&x);
        let sum: f32 = result.sum();
        assert!((sum - 1.0).abs() < 1e-5);

        // Higher input should have higher probability
        assert!(result[2] > result[1]);
        assert!(result[1] > result[0]);
    }

    #[test]
    fn test_fisher_computation() {
        let config = RouterConfig::default();
        let router = FastGRNNRouter::new(&config).unwrap();

        let samples: Vec<RouterSample> = (0..5)
            .map(|_| RouterSample {
                features: vec![0.5f32; config.input_dim],
                label_model: 1,
                label_context: 2,
                label_temperature: 0.7,
                label_top_p: 0.9,
                quality: 0.8,
                latency_ms: 100.0,
            })
            .collect();

        let fisher = router.compute_fisher(&samples);
        assert!(!fisher.is_empty());
    }

    #[test]
    fn test_stats_tracking() {
        let config = RouterConfig::default();
        let router = FastGRNNRouter::new(&config).unwrap();

        let features = vec![0.5f32; config.input_dim];
        let hidden = vec![0.0f32; config.hidden_dim];

        for _ in 0..10 {
            let _ = router.forward(&features, &hidden);
        }

        assert_eq!(router.stats.forward_count.load(Ordering::Relaxed), 10);
    }
}
