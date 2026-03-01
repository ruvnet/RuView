//! Mamba State Space Model layer.
//!
//! Provides O(n) attention alternative with selective state updates.
//! Input-dependent B, C, Δ parameters enable content-based reasoning.
//!
//! ## Academic Foundations
//!
//! Based on:
//! - **Mamba** (Gu & Dao, 2023) - Selective State Space Models with 5× speedup over Transformers
//! - **Mamba-2** (Dao & Gu, 2024) - Improved selective SSMs with structured state space duality
//!
//! Key innovations:
//! 1. **Selective State Space**: Input-dependent A, B, C matrices enable content-based filtering
//! 2. **Hardware-Aware Design**: Uses parallel scan for training, recurrent mode for inference
//! 3. **Linear Complexity**: O(N) in sequence length vs O(N²) for attention
//! 4. **Long-Range Dependencies**: Maintains O(1) memory per step during inference
//!
//! ## Implementation Notes
//!
//! This implementation provides:
//! - Recurrent mode for O(1) memory inference
//! - Sequence mode for training compatibility
//! - Input-dependent discretization of continuous SSM parameters
//! - Selective scan operation with content-based gating
//!
//! ## References
//!
//! - Gu, A., & Dao, T. (2023). Mamba: Linear-Time Sequence Modeling with Selective State Spaces. arXiv:2312.00752
//! - Dao, T., & Gu, A. (2024). Transformers are SSMs: Generalized Models and Efficient Algorithms Through Structured State Space Duality. arXiv:2405.21060

#![cfg_attr(feature = "no_std_gateway", no_std)]

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;
use core::f32;

/// Mamba layer configuration
#[derive(Clone, Debug)]
pub struct MambaConfig {
    /// Model dimension
    pub d_model: usize,

    /// SSM state dimension (typically 16 for efficiency)
    pub d_state: usize,

    /// Local convolution width (typically 4)
    pub d_conv: usize,

    /// Expansion factor (typically 2)
    pub expand: usize,

    /// Rank for Δ projection
    pub dt_rank: usize,

    /// Minimum discretization step
    pub dt_min: f32,

    /// Maximum discretization step
    pub dt_max: f32,
}

impl Default for MambaConfig {
    fn default() -> Self {
        Self {
            d_model: 256,
            d_state: 16,
            d_conv: 4,
            expand: 2,
            dt_rank: 16,
            dt_min: 0.001,
            dt_max: 0.1,
        }
    }
}

impl MambaConfig {
    /// Create a micro configuration for embedded/edge devices
    pub fn micro() -> Self {
        Self {
            d_model: 128,
            d_state: 8,
            d_conv: 4,
            expand: 2,
            dt_rank: 8,
            dt_min: 0.001,
            dt_max: 0.1,
        }
    }

    /// Create a baseline configuration
    pub fn baseline() -> Self {
        Self::default()
    }

    /// Compute inner dimension
    #[inline]
    pub fn d_inner(&self) -> usize {
        self.d_model * self.expand
    }
}

/// SSM state for recurrent inference
#[derive(Clone, Debug)]
pub struct MambaState {
    /// Hidden state [d_inner, d_state]
    pub h: Vec<f32>,

    /// Convolution buffer [d_inner, d_conv]
    pub conv_state: Vec<f32>,
}

impl MambaState {
    /// Create new state from configuration
    pub fn new(config: &MambaConfig) -> Self {
        let d_inner = config.d_inner();
        Self {
            h: vec![0.0; d_inner * config.d_state],
            conv_state: vec![0.0; d_inner * config.d_conv],
        }
    }

    /// Reset state to zeros
    pub fn reset(&mut self) {
        for x in &mut self.h {
            *x = 0.0;
        }
        for x in &mut self.conv_state {
            *x = 0.0;
        }
    }
}

/// Mamba layer weights
#[derive(Clone, Debug)]
pub struct MambaWeights {
    /// Input projection [d_model, d_inner * 2]
    pub in_proj: Vec<f32>,

    /// 1D convolution weights [d_inner, d_conv]
    pub conv1d: Vec<f32>,

    /// Projection to dt, B, C [d_inner, dt_rank + d_state * 2]
    pub x_proj: Vec<f32>,

    /// dt projection [dt_rank, d_inner]
    pub dt_proj: Vec<f32>,

    /// Log of A matrix [d_inner, d_state]
    pub a_log: Vec<f32>,

    /// Skip connection [d_inner]
    pub d: Vec<f32>,

    /// Output projection [d_inner, d_model]
    pub out_proj: Vec<f32>,
}

impl MambaWeights {
    /// Create empty weights for configuration
    pub fn empty(config: &MambaConfig) -> Self {
        let d_inner = config.d_inner();
        Self {
            in_proj: vec![0.0; config.d_model * d_inner * 2],
            conv1d: vec![0.0; d_inner * config.d_conv],
            x_proj: vec![0.0; d_inner * (config.dt_rank + config.d_state * 2)],
            dt_proj: vec![0.0; config.dt_rank * d_inner],
            a_log: vec![0.0; d_inner * config.d_state],
            d: vec![0.0; d_inner],
            out_proj: vec![0.0; d_inner * config.d_model],
        }
    }

    /// Initialize with random values (for testing)
    #[cfg(test)]
    pub fn random(config: &MambaConfig, seed: u64) -> Self {
        use core::num::Wrapping;

        let mut rng = Wrapping(seed);
        let mut rand_f32 = || {
            rng = rng * Wrapping(1664525) + Wrapping(1013904223);
            ((rng.0 as f32) / (u64::MAX as f32)) * 0.1 - 0.05
        };

        let mut weights = Self::empty(config);

        for w in &mut weights.in_proj {
            *w = rand_f32();
        }
        for w in &mut weights.conv1d {
            *w = rand_f32();
        }
        for w in &mut weights.x_proj {
            *w = rand_f32();
        }
        for w in &mut weights.dt_proj {
            *w = rand_f32();
        }
        for w in &mut weights.a_log {
            *w = -rand_f32().abs() - 1.0;
        } // Negative for stability
        for w in &mut weights.d {
            *w = rand_f32();
        }
        for w in &mut weights.out_proj {
            *w = rand_f32();
        }

        weights
    }
}

/// Mamba layer
pub struct MambaLayer {
    config: MambaConfig,
    d_inner: usize,
}

impl MambaLayer {
    /// Create new Mamba layer
    pub fn new(config: MambaConfig) -> Self {
        let d_inner = config.d_inner();
        Self { config, d_inner }
    }

    /// Get configuration
    pub fn config(&self) -> &MambaConfig {
        &self.config
    }

    /// Forward pass for single token (recurrent mode)
    ///
    /// This is the O(1) memory mode used during inference.
    /// Updates state in-place and returns output for this timestep.
    pub fn forward_step(
        &self,
        weights: &MambaWeights,
        x: &[f32], // [d_model]
        state: &mut MambaState,
    ) -> Vec<f32> {
        debug_assert_eq!(x.len(), self.config.d_model);

        // Input projection: x -> (x_proj, z)
        let mut x_and_z = vec![0.0; self.d_inner * 2];
        self.linear(
            x,
            &weights.in_proj,
            self.config.d_model,
            self.d_inner * 2,
            &mut x_and_z,
        );

        let (x_proj, z) = x_and_z.split_at(self.d_inner);
        let mut x_proj = x_proj.to_vec();
        let z = z.to_vec();

        // Causal 1D convolution using state
        self.causal_conv1d_step(&mut x_proj, &weights.conv1d, state);

        // Compute input-dependent parameters
        let mut params = vec![0.0; self.config.dt_rank + self.config.d_state * 2];
        self.linear(
            &x_proj,
            &weights.x_proj,
            self.d_inner,
            self.config.dt_rank + self.config.d_state * 2,
            &mut params,
        );

        let dt_proj_input = &params[..self.config.dt_rank];
        let b = &params[self.config.dt_rank..self.config.dt_rank + self.config.d_state];
        let c = &params[self.config.dt_rank + self.config.d_state..];

        // Project dt
        let mut delta = vec![0.0; self.d_inner];
        self.linear(
            dt_proj_input,
            &weights.dt_proj,
            self.config.dt_rank,
            self.d_inner,
            &mut delta,
        );

        // Apply softplus: dt = softplus(delta)
        for d in &mut delta {
            *d = Self::softplus(*d);
            *d = d.clamp(self.config.dt_min, self.config.dt_max);
        }

        // Selective scan step
        let y = self.selective_scan_step(&x_proj, &delta, &weights.a_log, b, c, &weights.d, state);

        // Gated output: y * silu(z)
        let mut output = vec![0.0; self.d_inner];
        for i in 0..self.d_inner {
            output[i] = y[i] * Self::silu(z[i]);
        }

        // Output projection
        let mut result = vec![0.0; self.config.d_model];
        self.linear(
            &output,
            &weights.out_proj,
            self.d_inner,
            self.config.d_model,
            &mut result,
        );

        result
    }

    /// Forward pass for sequence (parallel mode, for training)
    ///
    /// Processes entire sequence at once. Less memory efficient than recurrent mode.
    pub fn forward_sequence(
        &self,
        weights: &MambaWeights,
        x: &[f32], // [seq_len, d_model]
        seq_len: usize,
    ) -> Vec<f32> {
        debug_assert_eq!(x.len(), seq_len * self.config.d_model);

        let mut output = vec![0.0; seq_len * self.config.d_model];
        let mut state = MambaState::new(&self.config);

        // Process sequence token by token
        for t in 0..seq_len {
            let x_t = &x[t * self.config.d_model..(t + 1) * self.config.d_model];
            let y_t = self.forward_step(weights, x_t, &mut state);
            output[t * self.config.d_model..(t + 1) * self.config.d_model].copy_from_slice(&y_t);
        }

        output
    }

    /// Causal 1D convolution for single step
    fn causal_conv1d_step(
        &self,
        x: &mut [f32],        // [d_inner]
        conv_weights: &[f32], // [d_inner, d_conv]
        state: &mut MambaState,
    ) {
        debug_assert_eq!(x.len(), self.d_inner);

        let mut output = vec![0.0; self.d_inner];

        for i in 0..self.d_inner {
            // Shift conv state
            for j in (1..self.config.d_conv).rev() {
                state.conv_state[i * self.config.d_conv + j] =
                    state.conv_state[i * self.config.d_conv + j - 1];
            }
            state.conv_state[i * self.config.d_conv] = x[i];

            // Apply convolution
            let mut sum = 0.0;
            for j in 0..self.config.d_conv {
                sum += state.conv_state[i * self.config.d_conv + j]
                    * conv_weights[i * self.config.d_conv + j];
            }
            output[i] = sum;
        }

        x.copy_from_slice(&output);
    }

    /// Selective scan for single step (updates state)
    fn selective_scan_step(
        &self,
        u: &[f32],     // Input [d_inner]
        delta: &[f32], // Time steps [d_inner]
        a_log: &[f32], // A matrix (log space) [d_inner, d_state]
        b: &[f32],     // B matrix [d_state]
        c: &[f32],     // C matrix [d_state]
        d: &[f32],     // Skip connection [d_inner]
        state: &mut MambaState,
    ) -> Vec<f32> {
        let mut y = vec![0.0; self.d_inner];

        for i in 0..self.d_inner {
            let dt = delta[i];

            // Discretize A and B for this channel
            // A_bar = exp(dt * A), B_bar = dt * B
            let mut a_bar = vec![0.0; self.config.d_state];
            let mut b_bar = vec![0.0; self.config.d_state];

            for n in 0..self.config.d_state {
                let a_val = (-a_log[i * self.config.d_state + n].abs()).exp(); // A = exp(a_log)
                a_bar[n] = (dt * a_val).exp();
                b_bar[n] = dt * b[n];
            }

            // Update state: h = A_bar * h + B_bar * u
            for n in 0..self.config.d_state {
                let h_idx = i * self.config.d_state + n;
                state.h[h_idx] = a_bar[n] * state.h[h_idx] + b_bar[n] * u[i];
            }

            // Compute output: y = C * h + D * u
            let mut y_val = 0.0;
            for n in 0..self.config.d_state {
                y_val += c[n] * state.h[i * self.config.d_state + n];
            }
            y_val += d[i] * u[i];

            y[i] = y_val;
        }

        y
    }

    /// Discretize continuous SSM parameters (Zero-Order Hold)
    ///
    /// Returns (A_bar, B_bar) where:
    /// - A_bar = exp(Δ * A)
    /// - B_bar = Δ * B
    #[allow(dead_code)]
    fn discretize(
        &self,
        a: &[f32],     // Continuous A [d_inner, d_state]
        b: &[f32],     // Continuous B [d_inner, d_state]
        delta: &[f32], // Time steps [d_inner]
    ) -> (Vec<f32>, Vec<f32>) {
        let mut a_bar = vec![0.0; self.d_inner * self.config.d_state];
        let mut b_bar = vec![0.0; self.d_inner * self.config.d_state];

        for i in 0..self.d_inner {
            let dt = delta[i];
            for n in 0..self.config.d_state {
                let idx = i * self.config.d_state + n;
                a_bar[idx] = (dt * a[idx]).exp();
                b_bar[idx] = dt * b[idx];
            }
        }

        (a_bar, b_bar)
    }

    /// Selective scan operation (full sequence)
    #[allow(dead_code)]
    fn selective_scan(
        &self,
        u: &[f32],     // Input [seq_len, d_inner]
        delta: &[f32], // Time steps [seq_len, d_inner]
        a: &[f32],     // A matrix [d_inner, d_state]
        b: &[f32],     // B matrix [seq_len, d_state]
        c: &[f32],     // C matrix [seq_len, d_state]
        d: &[f32],     // D matrix (skip) [d_inner]
        seq_len: usize,
    ) -> Vec<f32> {
        let mut y = vec![0.0; seq_len * self.d_inner];
        let mut h = vec![0.0; self.d_inner * self.config.d_state];

        for t in 0..seq_len {
            for i in 0..self.d_inner {
                let dt = delta[t * self.d_inner + i];

                // Discretize and update state
                for n in 0..self.config.d_state {
                    let a_bar = (dt * a[i * self.config.d_state + n]).exp();
                    let b_bar = dt * b[t * self.config.d_state + n];

                    let h_idx = i * self.config.d_state + n;
                    h[h_idx] = a_bar * h[h_idx] + b_bar * u[t * self.d_inner + i];
                }

                // Compute output
                let mut y_val = 0.0;
                for n in 0..self.config.d_state {
                    y_val += c[t * self.config.d_state + n] * h[i * self.config.d_state + n];
                }
                y_val += d[i] * u[t * self.d_inner + i];

                y[t * self.d_inner + i] = y_val;
            }
        }

        y
    }

    /// Simple linear layer (matrix multiply)
    #[inline]
    fn linear(&self, x: &[f32], w: &[f32], in_dim: usize, out_dim: usize, out: &mut [f32]) {
        debug_assert_eq!(x.len(), in_dim);
        debug_assert_eq!(w.len(), in_dim * out_dim);
        debug_assert_eq!(out.len(), out_dim);

        for i in 0..out_dim {
            let mut sum = 0.0;
            for j in 0..in_dim {
                sum += x[j] * w[i * in_dim + j];
            }
            out[i] = sum;
        }
    }

    /// SiLU activation: x * sigmoid(x)
    #[inline]
    fn silu(x: f32) -> f32 {
        x / (1.0 + (-x).exp())
    }

    /// Softplus activation: log(1 + exp(x))
    #[inline]
    fn softplus(x: f32) -> f32 {
        if x > 20.0 {
            x // Avoid overflow
        } else {
            (1.0 + x.exp()).ln()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mamba_config() {
        let config = MambaConfig::default();
        assert_eq!(config.d_model, 256);
        assert_eq!(config.d_state, 16);
        assert_eq!(config.d_conv, 4);
        assert_eq!(config.d_inner(), 512);

        let micro = MambaConfig::micro();
        assert_eq!(micro.d_model, 128);
        assert_eq!(micro.d_state, 8);
    }

    #[test]
    fn test_mamba_state() {
        let config = MambaConfig::micro();
        let mut state = MambaState::new(&config);

        let d_inner = config.d_inner();
        assert_eq!(state.h.len(), d_inner * config.d_state);
        assert_eq!(state.conv_state.len(), d_inner * config.d_conv);

        // All zeros initially
        assert!(state.h.iter().all(|&x| x == 0.0));

        // Modify and reset
        state.h[0] = 1.0;
        state.reset();
        assert!(state.h.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_activation_functions() {
        // SiLU
        assert!((MambaLayer::silu(0.0) - 0.0).abs() < 1e-5);
        assert!(MambaLayer::silu(1.0) > 0.5);
        assert!(MambaLayer::silu(-1.0) < 0.0);

        // Softplus
        assert!((MambaLayer::softplus(0.0) - 0.693).abs() < 0.01);
        assert!(MambaLayer::softplus(1.0) > 1.0);
        assert!(MambaLayer::softplus(25.0) > 24.0); // Test overflow handling
    }

    #[test]
    fn test_forward_step_shape() {
        let config = MambaConfig::micro();
        let layer = MambaLayer::new(config.clone());
        let weights = MambaWeights::random(&config, 42);
        let mut state = MambaState::new(&config);

        let x = vec![0.1; config.d_model];
        let y = layer.forward_step(&weights, &x, &mut state);

        assert_eq!(y.len(), config.d_model);
    }

    #[test]
    fn test_forward_step_deterministic() {
        let config = MambaConfig::micro();
        let layer = MambaLayer::new(config.clone());
        let weights = MambaWeights::random(&config, 42);

        let x = vec![0.1; config.d_model];

        // Two identical runs should produce identical results
        let mut state1 = MambaState::new(&config);
        let y1 = layer.forward_step(&weights, &x, &mut state1);

        let mut state2 = MambaState::new(&config);
        let y2 = layer.forward_step(&weights, &x, &mut state2);

        for (a, b) in y1.iter().zip(y2.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_forward_sequence_matches_steps() {
        let config = MambaConfig::micro();
        let layer = MambaLayer::new(config.clone());
        let weights = MambaWeights::random(&config, 42);

        let seq_len = 4;
        let x = vec![0.1; seq_len * config.d_model];

        // Sequence mode
        let y_seq = layer.forward_sequence(&weights, &x, seq_len);

        // Step-by-step mode
        let mut state = MambaState::new(&config);
        let mut y_steps = vec![0.0; seq_len * config.d_model];
        for t in 0..seq_len {
            let x_t = &x[t * config.d_model..(t + 1) * config.d_model];
            let y_t = layer.forward_step(&weights, x_t, &mut state);
            y_steps[t * config.d_model..(t + 1) * config.d_model].copy_from_slice(&y_t);
        }

        // Should match
        for (a, b) in y_seq.iter().zip(y_steps.iter()) {
            assert!((a - b).abs() < 1e-5, "Mismatch: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_state_persistence() {
        let config = MambaConfig::micro();
        let layer = MambaLayer::new(config.clone());
        let weights = MambaWeights::random(&config, 42);
        let mut state = MambaState::new(&config);

        let x1 = vec![0.1; config.d_model];
        let x2 = vec![0.2; config.d_model];

        // First step
        let y1 = layer.forward_step(&weights, &x1, &mut state);

        // State should have changed
        let has_nonzero = state.h.iter().any(|&x| x != 0.0);
        assert!(has_nonzero, "State should be updated after forward pass");

        // Second step with different input
        let y2 = layer.forward_step(&weights, &x2, &mut state);

        // Output should depend on previous state
        assert_ne!(y1, y2);

        // Reset and run again - should get same result as first step
        state.reset();
        let y1_again = layer.forward_step(&weights, &x1, &mut state);

        for (a, b) in y1.iter().zip(y1_again.iter()) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn test_linear_layer() {
        let config = MambaConfig::micro();
        let layer = MambaLayer::new(config);

        let x = vec![1.0, 2.0, 3.0];
        let w = vec![
            1.0, 0.0, 0.0, // First output: 1*1 + 2*0 + 3*0 = 1
            0.0, 1.0, 0.0, // Second output: 1*0 + 2*1 + 3*0 = 2
        ];
        let mut out = vec![0.0; 2];

        layer.linear(&x, &w, 3, 2, &mut out);

        assert!((out[0] - 1.0).abs() < 1e-5);
        assert!((out[1] - 2.0).abs() < 1e-5);
    }

    #[test]
    fn test_discretize() {
        let config = MambaConfig::micro();
        let layer = MambaLayer::new(config.clone());

        let d_inner = config.d_inner();
        let a = vec![-1.0; d_inner * config.d_state];
        let b = vec![1.0; d_inner * config.d_state];
        let delta = vec![0.1; d_inner];

        let (a_bar, b_bar) = layer.discretize(&a, &b, &delta);

        // A_bar = exp(dt * A) should be < 1 for negative A
        for &val in &a_bar {
            assert!(val < 1.0 && val > 0.0);
        }

        // B_bar = dt * B should be scaled by dt
        for &val in &b_bar {
            assert!((val - 0.1).abs() < 1e-5);
        }
    }

    #[test]
    fn test_empty_weights() {
        let config = MambaConfig::micro();
        let weights = MambaWeights::empty(&config);

        let d_inner = config.d_inner();
        assert_eq!(weights.in_proj.len(), config.d_model * d_inner * 2);
        assert_eq!(weights.conv1d.len(), d_inner * config.d_conv);
        assert_eq!(weights.a_log.len(), d_inner * config.d_state);
    }

    #[test]
    fn test_different_configs() {
        // Test that different configs produce appropriately sized outputs
        for config in &[MambaConfig::micro(), MambaConfig::baseline()] {
            let layer = MambaLayer::new(config.clone());
            let weights = MambaWeights::random(config, 42);
            let mut state = MambaState::new(config);

            let x = vec![0.1; config.d_model];
            let y = layer.forward_step(&weights, &x, &mut state);

            assert_eq!(y.len(), config.d_model);
        }
    }
}
