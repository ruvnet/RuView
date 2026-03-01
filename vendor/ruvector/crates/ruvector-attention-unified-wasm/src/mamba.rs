//! Mamba SSM (Selective State Space Model) Attention Mechanism
//!
//! Implements the Mamba architecture's selective scan mechanism for efficient
//! sequence modeling with linear time complexity O(n).
//!
//! Key Features:
//! - **Selective Scan**: Input-dependent state transitions
//! - **Linear Complexity**: O(n) vs O(n^2) for standard attention
//! - **Hardware Efficient**: Optimized for parallel scan operations
//! - **Long Context**: Handles very long sequences efficiently
//!
//! ## Architecture
//!
//! Mamba uses a selective state space model:
//! ```text
//! h_t = A_t * h_{t-1} + B_t * x_t
//! y_t = C_t * h_t
//! ```
//!
//! Where A_t, B_t, C_t are input-dependent (selective), computed from x_t.
//!
//! ## References
//!
//! - Mamba: Linear-Time Sequence Modeling with Selective State Spaces (Gu & Dao, 2023)

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for Mamba SSM attention
#[derive(Debug, Clone, Serialize, Deserialize)]
#[wasm_bindgen]
pub struct MambaConfig {
    /// Model dimension (d_model)
    pub dim: usize,
    /// State space dimension (n)
    pub state_dim: usize,
    /// Expansion factor for inner dimension
    pub expand_factor: usize,
    /// Convolution kernel size
    pub conv_kernel_size: usize,
    /// Delta (discretization step) range minimum
    pub dt_min: f32,
    /// Delta range maximum
    pub dt_max: f32,
    /// Whether to use learnable D skip connection
    pub use_d_skip: bool,
}

#[wasm_bindgen]
impl MambaConfig {
    /// Create a new Mamba configuration
    #[wasm_bindgen(constructor)]
    pub fn new(dim: usize) -> MambaConfig {
        MambaConfig {
            dim,
            state_dim: 16,
            expand_factor: 2,
            conv_kernel_size: 4,
            dt_min: 0.001,
            dt_max: 0.1,
            use_d_skip: true,
        }
    }

    /// Set state space dimension
    #[wasm_bindgen(js_name = withStateDim)]
    pub fn with_state_dim(mut self, state_dim: usize) -> MambaConfig {
        self.state_dim = state_dim;
        self
    }

    /// Set expansion factor
    #[wasm_bindgen(js_name = withExpandFactor)]
    pub fn with_expand_factor(mut self, factor: usize) -> MambaConfig {
        self.expand_factor = factor;
        self
    }

    /// Set convolution kernel size
    #[wasm_bindgen(js_name = withConvKernelSize)]
    pub fn with_conv_kernel_size(mut self, size: usize) -> MambaConfig {
        self.conv_kernel_size = size;
        self
    }
}

impl Default for MambaConfig {
    fn default() -> Self {
        MambaConfig::new(256)
    }
}

// ============================================================================
// State Space Parameters
// ============================================================================

/// Selective state space parameters (input-dependent)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SelectiveSSMParams {
    /// Discretized A matrix diagonal (batch, seq_len, state_dim)
    a_bar: Vec<Vec<Vec<f32>>>,
    /// Discretized B matrix (batch, seq_len, state_dim)
    b_bar: Vec<Vec<Vec<f32>>>,
    /// Output projection C (batch, seq_len, state_dim)
    c: Vec<Vec<Vec<f32>>>,
    /// Discretization step delta (batch, seq_len, inner_dim)
    delta: Vec<Vec<Vec<f32>>>,
}

// ============================================================================
// Mamba SSM Attention
// ============================================================================

/// Mamba Selective State Space Model for sequence attention
///
/// Provides O(n) attention-like mechanism using selective state spaces
#[wasm_bindgen]
pub struct MambaSSMAttention {
    config: MambaConfig,
    /// Inner dimension after expansion
    inner_dim: usize,
    /// A parameter (state_dim,) - diagonal of continuous A
    a_log: Vec<f32>,
    /// D skip connection (inner_dim,)
    d_skip: Vec<f32>,
    /// Projection weights (simplified for WASM)
    in_proj: Vec<Vec<f32>>,
    out_proj: Vec<Vec<f32>>,
}

#[wasm_bindgen]
impl MambaSSMAttention {
    /// Create a new Mamba SSM attention layer
    #[wasm_bindgen(constructor)]
    pub fn new(config: MambaConfig) -> MambaSSMAttention {
        let inner_dim = config.dim * config.expand_factor;

        // Initialize A as negative values (for stability) - log of eigenvalues
        let a_log: Vec<f32> = (0..config.state_dim)
            .map(|i| -((i + 1) as f32).ln())
            .collect();

        // D skip connection
        let d_skip = vec![1.0; inner_dim];

        // Simplified projection matrices (identity-like for stub)
        let in_proj: Vec<Vec<f32>> = (0..inner_dim)
            .map(|i| {
                let mut row = vec![0.0; config.dim];
                if i < config.dim {
                    row[i] = 1.0;
                }
                row
            })
            .collect();

        let out_proj: Vec<Vec<f32>> = (0..config.dim)
            .map(|i| {
                let mut row = vec![0.0; inner_dim];
                if i < inner_dim {
                    row[i] = 1.0;
                }
                row
            })
            .collect();

        MambaSSMAttention {
            config,
            inner_dim,
            a_log,
            d_skip,
            in_proj,
            out_proj,
        }
    }

    /// Create with default configuration
    #[wasm_bindgen(js_name = withDefaults)]
    pub fn with_defaults(dim: usize) -> MambaSSMAttention {
        MambaSSMAttention::new(MambaConfig::new(dim))
    }

    /// Forward pass through Mamba SSM
    ///
    /// # Arguments
    /// * `input` - Input sequence (seq_len, dim) flattened to 1D
    /// * `seq_len` - Sequence length
    ///
    /// # Returns
    /// Output sequence (seq_len, dim) flattened to 1D
    #[wasm_bindgen]
    pub fn forward(&self, input: Vec<f32>, seq_len: usize) -> Result<Vec<f32>, JsError> {
        let dim = self.config.dim;

        if input.len() != seq_len * dim {
            return Err(JsError::new(&format!(
                "Input size mismatch: expected {} ({}x{}), got {}",
                seq_len * dim,
                seq_len,
                dim,
                input.len()
            )));
        }

        // Reshape input to 2D
        let input_2d: Vec<Vec<f32>> = (0..seq_len)
            .map(|t| input[t * dim..(t + 1) * dim].to_vec())
            .collect();

        // Step 1: Input projection to inner_dim
        let projected = self.project_in(&input_2d);

        // Step 2: Compute selective SSM parameters from input
        let ssm_params = self.compute_selective_params(&projected);

        // Step 3: Run selective scan
        let ssm_output = self.selective_scan(&projected, &ssm_params);

        // Step 4: Apply D skip connection
        let with_skip: Vec<Vec<f32>> = ssm_output
            .iter()
            .zip(projected.iter())
            .map(|(y, x)| {
                y.iter()
                    .zip(x.iter())
                    .zip(self.d_skip.iter())
                    .map(|((yi, xi), di)| yi + di * xi)
                    .collect()
            })
            .collect();

        // Step 5: Output projection
        let output = self.project_out(&with_skip);

        // Flatten output
        Ok(output.into_iter().flatten().collect())
    }

    /// Get the configuration
    #[wasm_bindgen(getter)]
    pub fn config(&self) -> MambaConfig {
        self.config.clone()
    }

    /// Get the inner dimension
    #[wasm_bindgen(getter, js_name = innerDim)]
    pub fn inner_dim(&self) -> usize {
        self.inner_dim
    }

    /// Compute attention-like scores (for visualization/analysis)
    ///
    /// Returns pseudo-attention scores showing which positions influence output
    #[wasm_bindgen(js_name = getAttentionScores)]
    pub fn get_attention_scores(
        &self,
        input: Vec<f32>,
        seq_len: usize,
    ) -> Result<Vec<f32>, JsError> {
        let dim = self.config.dim;

        if input.len() != seq_len * dim {
            return Err(JsError::new(&format!(
                "Input size mismatch: expected {}, got {}",
                seq_len * dim,
                input.len()
            )));
        }

        // Compute approximate attention scores based on state decay
        // This shows how much each position can "attend to" previous positions
        let mut scores = vec![0.0f32; seq_len * seq_len];

        for t in 0..seq_len {
            for s in 0..=t {
                // Exponential decay based on distance and A parameters
                let distance = (t - s) as f32;
                let decay: f32 = self
                    .a_log
                    .iter()
                    .map(|&a| (a * distance).exp())
                    .sum::<f32>()
                    / self.config.state_dim as f32;

                scores[t * seq_len + s] = decay;
            }
        }

        Ok(scores)
    }
}

// Internal implementation methods
impl MambaSSMAttention {
    /// Project input from dim to inner_dim
    fn project_in(&self, input: &[Vec<f32>]) -> Vec<Vec<f32>> {
        input
            .iter()
            .map(|x| {
                self.in_proj
                    .iter()
                    .map(|row| row.iter().zip(x.iter()).map(|(w, xi)| w * xi).sum())
                    .collect()
            })
            .collect()
    }

    /// Project from inner_dim back to dim
    fn project_out(&self, input: &[Vec<f32>]) -> Vec<Vec<f32>> {
        input
            .iter()
            .map(|x| {
                self.out_proj
                    .iter()
                    .map(|row| row.iter().zip(x.iter()).map(|(w, xi)| w * xi).sum())
                    .collect()
            })
            .collect()
    }

    /// Compute selective SSM parameters from input
    fn compute_selective_params(&self, input: &[Vec<f32>]) -> SelectiveSSMParams {
        let seq_len = input.len();
        let state_dim = self.config.state_dim;

        // Compute input-dependent delta, B, C
        // Simplified: use sigmoid/tanh of input projections

        let mut a_bar = vec![vec![vec![0.0; state_dim]; self.inner_dim]; seq_len];
        let mut b_bar = vec![vec![vec![0.0; state_dim]; self.inner_dim]; seq_len];
        let mut c = vec![vec![vec![0.0; state_dim]; self.inner_dim]; seq_len];
        let mut delta = vec![vec![vec![0.0; self.inner_dim]; 1]; seq_len];

        for (t, x) in input.iter().enumerate() {
            // Compute delta from input (softplus of projection)
            let dt: Vec<f32> = x
                .iter()
                .map(|&xi| {
                    let raw = xi * 0.1; // Simple scaling
                    let dt_val = (1.0 + raw.exp()).ln(); // Softplus
                    dt_val.clamp(self.config.dt_min, self.config.dt_max)
                })
                .collect();
            delta[t][0] = dt.clone();

            for d in 0..self.inner_dim.min(x.len()) {
                let dt_d = dt[d.min(dt.len() - 1)];

                for n in 0..state_dim {
                    // Discretize A: A_bar = exp(delta * A)
                    let a_continuous = self.a_log[n].exp(); // Negative
                    a_bar[t][d][n] = (dt_d * a_continuous).exp();

                    // Discretize B: B_bar = delta * B (simplified)
                    // B is input-dependent
                    let b_input = if d < x.len() { x[d] } else { 0.0 };
                    b_bar[t][d][n] = dt_d * Self::sigmoid(b_input * 0.1);

                    // C is input-dependent
                    c[t][d][n] = Self::tanh(b_input * 0.1);
                }
            }
        }

        SelectiveSSMParams {
            a_bar,
            b_bar,
            c,
            delta,
        }
    }

    /// Run selective scan (parallel associative scan in practice)
    fn selective_scan(&self, input: &[Vec<f32>], params: &SelectiveSSMParams) -> Vec<Vec<f32>> {
        let seq_len = input.len();
        let state_dim = self.config.state_dim;

        // Initialize hidden state
        let mut hidden = vec![vec![0.0f32; state_dim]; self.inner_dim];
        let mut output = vec![vec![0.0f32; self.inner_dim]; seq_len];

        for t in 0..seq_len {
            for d in 0..self.inner_dim {
                let x_d = if d < input[t].len() { input[t][d] } else { 0.0 };

                // Update hidden state: h_t = A_bar * h_{t-1} + B_bar * x_t
                for n in 0..state_dim {
                    hidden[d][n] =
                        params.a_bar[t][d][n] * hidden[d][n] + params.b_bar[t][d][n] * x_d;
                }

                // Compute output: y_t = C * h_t
                output[t][d] = hidden[d]
                    .iter()
                    .zip(params.c[t][d].iter())
                    .map(|(h, c)| h * c)
                    .sum();
            }
        }

        output
    }

    #[inline]
    fn sigmoid(x: f32) -> f32 {
        1.0 / (1.0 + (-x).exp())
    }

    #[inline]
    fn tanh(x: f32) -> f32 {
        x.tanh()
    }
}

// ============================================================================
// Hybrid Mamba-Attention
// ============================================================================

/// Hybrid layer combining Mamba SSM with standard attention
///
/// Uses Mamba for long-range dependencies and attention for local patterns
#[wasm_bindgen]
pub struct HybridMambaAttention {
    mamba: MambaSSMAttention,
    local_window: usize,
    use_attention_for_local: bool,
}

#[wasm_bindgen]
impl HybridMambaAttention {
    /// Create a new hybrid Mamba-Attention layer
    #[wasm_bindgen(constructor)]
    pub fn new(config: MambaConfig, local_window: usize) -> HybridMambaAttention {
        HybridMambaAttention {
            mamba: MambaSSMAttention::new(config),
            local_window,
            use_attention_for_local: true,
        }
    }

    /// Forward pass
    #[wasm_bindgen]
    pub fn forward(&self, input: Vec<f32>, seq_len: usize) -> Result<Vec<f32>, JsError> {
        let dim = self.mamba.config.dim;

        // Run Mamba for global context
        let mamba_output = self.mamba.forward(input.clone(), seq_len)?;

        // Apply local attention mixing (simplified)
        let mut output = mamba_output.clone();

        if self.use_attention_for_local {
            for t in 0..seq_len {
                let start = t.saturating_sub(self.local_window / 2);
                let end = (t + self.local_window / 2 + 1).min(seq_len);

                // Simple local averaging
                for d in 0..dim {
                    let mut local_sum = 0.0;
                    let mut count = 0;
                    for s in start..end {
                        local_sum += input[s * dim + d];
                        count += 1;
                    }
                    // Mix global (Mamba) and local
                    let local_avg = local_sum / count as f32;
                    output[t * dim + d] = 0.7 * output[t * dim + d] + 0.3 * local_avg;
                }
            }
        }

        Ok(output)
    }

    /// Get local window size
    #[wasm_bindgen(getter, js_name = localWindow)]
    pub fn local_window(&self) -> usize {
        self.local_window
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_mamba_config() {
        let config = MambaConfig::new(256);
        assert_eq!(config.dim, 256);
        assert_eq!(config.state_dim, 16);
        assert_eq!(config.expand_factor, 2);
    }

    #[wasm_bindgen_test]
    fn test_mamba_creation() {
        let config = MambaConfig::new(64);
        let mamba = MambaSSMAttention::new(config);
        assert_eq!(mamba.inner_dim(), 128); // 64 * 2
    }

    #[wasm_bindgen_test]
    fn test_mamba_forward() {
        let config = MambaConfig::new(8);
        let mamba = MambaSSMAttention::new(config);

        // Input: 4 tokens of dimension 8
        let input = vec![0.1f32; 32];
        let output = mamba.forward(input, 4);

        assert!(output.is_ok());
        let out = output.unwrap();
        assert_eq!(out.len(), 32); // Same shape as input
    }

    #[wasm_bindgen_test]
    fn test_attention_scores() {
        let config = MambaConfig::new(8);
        let mamba = MambaSSMAttention::new(config);

        let input = vec![0.1f32; 24]; // 3 tokens
        let scores = mamba.get_attention_scores(input, 3);

        assert!(scores.is_ok());
        let s = scores.unwrap();
        assert_eq!(s.len(), 9); // 3x3 attention matrix

        // Causal: upper triangle should be 0
        assert_eq!(s[0 * 3 + 1], 0.0); // t=0 cannot attend to t=1
        assert_eq!(s[0 * 3 + 2], 0.0); // t=0 cannot attend to t=2
    }

    #[wasm_bindgen_test]
    fn test_hybrid_mamba() {
        let config = MambaConfig::new(8);
        let hybrid = HybridMambaAttention::new(config, 4);

        let input = vec![0.5f32; 40]; // 5 tokens
        let output = hybrid.forward(input, 5);

        assert!(output.is_ok());
        assert_eq!(output.unwrap().len(), 40);
    }
}
