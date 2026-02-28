//! MicroLoRA: Rank-2 Low-Rank Adaptation with <100us latency
//!
//! Implements the core LoRA algorithm: output = input + alpha * (input @ A @ B)
//! where A: [d x 2] and B: [2 x d] for rank-2 adaptation.

use wasm_bindgen::prelude::*;

/// Configuration for MicroLoRA
#[derive(Debug, Clone, Copy)]
pub struct LoRAConfig {
    /// Embedding dimension (typically 256)
    pub dim: usize,
    /// LoRA rank (1-2 for micro, default 2)
    pub rank: usize,
    /// Scaling factor alpha (default 0.1)
    pub alpha: f32,
    /// Learning rate for adaptation (default 0.01)
    pub learning_rate: f32,
    /// Dropout rate (0.0 = no dropout)
    pub dropout: f32,
}

impl Default for LoRAConfig {
    fn default() -> Self {
        Self {
            dim: 256,
            rank: 2,
            alpha: 0.1,
            learning_rate: 0.01,
            dropout: 0.0,
        }
    }
}

/// A single LoRA adapter pair (A and B matrices)
///
/// For rank-2:
/// - A: [dim x 2] - Down projection
/// - B: [2 x dim] - Up projection (initialized to zero)
///
/// Forward: output = input + alpha * (input @ A @ B)
#[derive(Clone)]
pub struct LoRAPair {
    /// Down projection matrix A: [dim][rank]
    /// Stored as Vec<[f32; 2]> for rank-2
    a: Vec<[f32; 2]>,
    /// Up projection matrix B: [rank][dim]
    /// Stored as [[f32; 256]; 2] for fixed 256-dim embeddings
    b: [[f32; 256]; 2],
    /// Scaling factor
    alpha: f32,
    /// Learning rate
    lr: f32,
    /// Embedding dimension
    dim: usize,
    /// Adaptation count for statistics
    adapt_count: u64,
}

impl LoRAPair {
    /// Create a new LoRA pair with Kaiming initialization for A, zeros for B
    pub fn new(config: &LoRAConfig) -> Self {
        let dim = config.dim.min(256); // Cap at 256 for fixed-size B
        let rank = config.rank.min(2); // Cap at 2 for micro

        // Initialize A with small random values (Kaiming-like)
        // Using deterministic pseudo-random for reproducibility
        let mut a = Vec::with_capacity(dim);
        let scale = (2.0 / dim as f32).sqrt() * 0.1; // Small initialization

        for i in 0..dim {
            let seed = i as u32;
            let r0 = pseudo_random(seed) * scale - scale / 2.0;
            let r1 = if rank > 1 {
                pseudo_random(seed.wrapping_add(1000)) * scale - scale / 2.0
            } else {
                0.0
            };
            a.push([r0, r1]);
        }

        // B initialized to zeros (LoRA standard practice)
        let b = [[0.0f32; 256]; 2];

        Self {
            a,
            b,
            alpha: config.alpha,
            lr: config.learning_rate,
            dim,
            adapt_count: 0,
        }
    }

    /// Forward pass: output = input + alpha * (input @ A @ B)
    ///
    /// Complexity: O(d * r + r * d) = O(2dr) for rank r
    /// For rank-2, d=256: ~1024 ops = <100us
    #[inline]
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        let n = input.len().min(self.dim);
        let mut output = input.to_vec();

        // Compute low_rank = input @ A (result: [2])
        let mut low_rank = [0.0f32; 2];
        for i in 0..n {
            low_rank[0] += input[i] * self.a[i][0];
            low_rank[1] += input[i] * self.a[i][1];
        }

        // Compute delta = low_rank @ B (result: [dim])
        // Output = input + alpha * delta
        for i in 0..n {
            let delta = low_rank[0] * self.b[0][i] + low_rank[1] * self.b[1][i];
            output[i] += self.alpha * delta;
        }

        output
    }

    /// Forward pass into pre-allocated buffer (zero-allocation hot path)
    #[inline]
    pub fn forward_into(&self, input: &[f32], output: &mut [f32]) {
        let n = input.len().min(self.dim).min(output.len());

        // Copy input to output
        output[..n].copy_from_slice(&input[..n]);

        // Compute low_rank = input @ A
        let mut low_rank = [0.0f32; 2];
        for i in 0..n {
            low_rank[0] += input[i] * self.a[i][0];
            low_rank[1] += input[i] * self.a[i][1];
        }

        // Add delta to output
        for i in 0..n {
            let delta = low_rank[0] * self.b[0][i] + low_rank[1] * self.b[1][i];
            output[i] += self.alpha * delta;
        }
    }

    /// Adapt weights based on gradient signal
    ///
    /// Uses rank-1 outer product update to B matrix for instant adaptation.
    /// Target latency: <100us
    #[inline]
    pub fn adapt(&mut self, gradient: &[f32]) {
        let n = gradient.len().min(self.dim);

        // Compute gradient norm for normalization
        let mut grad_norm_sq = 0.0f32;
        for i in 0..n {
            grad_norm_sq += gradient[i] * gradient[i];
        }

        if grad_norm_sq < 1e-16 {
            return; // Skip if gradient is too small
        }

        let grad_norm = fast_sqrt(grad_norm_sq);
        let inv_norm = 1.0 / grad_norm;

        // Compute column sums of A for scaling
        let mut a_col_sum = [0.0f32; 2];
        for i in 0..n {
            a_col_sum[0] += self.a[i][0];
            a_col_sum[1] += self.a[i][1];
        }

        // Update B using outer product: B += lr * a_sum * normalized_grad^T
        for j in 0..n {
            let normalized_grad = gradient[j] * inv_norm;
            self.b[0][j] += self.lr * a_col_sum[0] * normalized_grad;
            self.b[1][j] += self.lr * a_col_sum[1] * normalized_grad;
        }

        self.adapt_count += 1;
    }

    /// Adapt with improvement signal (for reinforcement learning)
    ///
    /// Uses the improvement ratio to scale the update magnitude.
    #[inline]
    pub fn adapt_with_reward(&mut self, gradient: &[f32], improvement: f32) {
        if improvement <= 0.0 {
            return; // Only learn from positive improvements
        }

        let n = gradient.len().min(self.dim);

        // Scale learning rate by improvement (clamped)
        let scaled_lr = self.lr * improvement.min(2.0);

        // Compute gradient norm
        let mut grad_norm_sq = 0.0f32;
        for i in 0..n {
            grad_norm_sq += gradient[i] * gradient[i];
        }

        if grad_norm_sq < 1e-16 {
            return;
        }

        let inv_norm = 1.0 / fast_sqrt(grad_norm_sq);

        // Compute A column sums
        let mut a_col_sum = [0.0f32; 2];
        for i in 0..n {
            a_col_sum[0] += self.a[i][0];
            a_col_sum[1] += self.a[i][1];
        }

        // Update B
        for j in 0..n {
            let normalized_grad = gradient[j] * inv_norm;
            self.b[0][j] += scaled_lr * a_col_sum[0] * normalized_grad;
            self.b[1][j] += scaled_lr * a_col_sum[1] * normalized_grad;
        }

        self.adapt_count += 1;
    }

    /// Reset B matrix to zeros (fresh start)
    pub fn reset(&mut self) {
        for i in 0..256 {
            self.b[0][i] = 0.0;
            self.b[1][i] = 0.0;
        }
        self.adapt_count = 0;
    }

    /// Get the number of adaptations performed
    pub fn adapt_count(&self) -> u64 {
        self.adapt_count
    }

    /// Get the effective weight delta norm (for monitoring)
    pub fn delta_norm(&self) -> f32 {
        let mut norm_sq = 0.0f32;
        for i in 0..self.dim {
            let delta = self.b[0][i] * self.b[0][i] + self.b[1][i] * self.b[1][i];
            norm_sq += delta;
        }
        fast_sqrt(norm_sq) * self.alpha
    }

    /// Get parameter count
    pub fn param_count(&self) -> usize {
        self.a.len() * 2 + 256 * 2
    }
}

/// Main MicroLoRA engine managing multiple LoRA pairs
pub struct MicroLoRAEngine {
    /// Default LoRA pair for unscoped operations
    default_lora: LoRAPair,
    /// Configuration (kept for potential future use)
    #[allow(dead_code)]
    config: LoRAConfig,
    /// Total forward passes
    forward_count: u64,
    /// Total adaptations
    total_adapt_count: u64,
}

impl MicroLoRAEngine {
    /// Create a new MicroLoRA engine
    pub fn new(config: LoRAConfig) -> Self {
        Self {
            default_lora: LoRAPair::new(&config),
            config,
            forward_count: 0,
            total_adapt_count: 0,
        }
    }

    /// Forward pass through the default LoRA
    #[inline]
    pub fn forward(&mut self, input: &[f32]) -> Vec<f32> {
        self.forward_count += 1;
        self.default_lora.forward(input)
    }

    /// Adapt the default LoRA with gradient
    #[inline]
    pub fn adapt(&mut self, gradient: &[f32]) {
        self.default_lora.adapt(gradient);
        self.total_adapt_count += 1;
    }

    /// Adapt with improvement reward
    #[inline]
    pub fn adapt_with_reward(&mut self, gradient: &[f32], improvement: f32) {
        self.default_lora.adapt_with_reward(gradient, improvement);
        self.total_adapt_count += 1;
    }

    /// Reset the engine
    pub fn reset(&mut self) {
        self.default_lora.reset();
        self.forward_count = 0;
        self.total_adapt_count = 0;
    }

    /// Get statistics
    pub fn stats(&self) -> (u64, u64, f32) {
        (
            self.forward_count,
            self.total_adapt_count,
            self.default_lora.delta_norm(),
        )
    }

    /// Get the underlying LoRA pair for advanced use
    pub fn lora(&self) -> &LoRAPair {
        &self.default_lora
    }

    /// Get mutable reference to underlying LoRA
    pub fn lora_mut(&mut self) -> &mut LoRAPair {
        &mut self.default_lora
    }
}

impl Default for MicroLoRAEngine {
    fn default() -> Self {
        Self::new(LoRAConfig::default())
    }
}

// ============ Helper Functions ============

/// Fast inverse square root (Quake III style)
#[inline(always)]
fn fast_sqrt(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let i = 0x5f3759df - (x.to_bits() >> 1);
    let y = f32::from_bits(i);
    x * y * (1.5 - 0.5 * x * y * y)
}

/// Deterministic pseudo-random number generator
#[inline(always)]
fn pseudo_random(seed: u32) -> f32 {
    // Simple xorshift
    let mut x = seed;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    (x as f32) / (u32::MAX as f32)
}

// ============ WASM Bindings ============

pub mod wasm_exports {
    use super::*;
    #[allow(unused_imports)]
    use wasm_bindgen::prelude::*;

    /// WASM-exposed MicroLoRA engine
    #[wasm_bindgen]
    pub struct WasmMicroLoRA {
        engine: MicroLoRAEngine,
        // Pre-allocated buffers for zero-allocation hot paths
        input_buffer: Vec<f32>,
        output_buffer: Vec<f32>,
    }

    #[wasm_bindgen]
    impl WasmMicroLoRA {
        /// Create a new MicroLoRA engine
        ///
        /// @param dim - Embedding dimension (default 256, max 256)
        /// @param alpha - Scaling factor (default 0.1)
        /// @param learning_rate - Learning rate (default 0.01)
        #[wasm_bindgen(constructor)]
        pub fn new(dim: Option<usize>, alpha: Option<f32>, learning_rate: Option<f32>) -> Self {
            let config = LoRAConfig {
                dim: dim.unwrap_or(256).min(256),
                rank: 2,
                alpha: alpha.unwrap_or(0.1),
                learning_rate: learning_rate.unwrap_or(0.01),
                dropout: 0.0,
            };

            let actual_dim = config.dim;
            Self {
                engine: MicroLoRAEngine::new(config),
                input_buffer: vec![0.0; actual_dim],
                output_buffer: vec![0.0; actual_dim],
            }
        }

        /// Get pointer to input buffer for direct memory access
        #[wasm_bindgen]
        pub fn get_input_ptr(&mut self) -> *mut f32 {
            self.input_buffer.as_mut_ptr()
        }

        /// Get pointer to output buffer for direct memory access
        #[wasm_bindgen]
        pub fn get_output_ptr(&self) -> *const f32 {
            self.output_buffer.as_ptr()
        }

        /// Get embedding dimension
        #[wasm_bindgen]
        pub fn dim(&self) -> usize {
            self.input_buffer.len()
        }

        /// Forward pass using internal buffers (zero-allocation)
        ///
        /// Write input to get_input_ptr(), call forward(), read from get_output_ptr()
        #[wasm_bindgen]
        pub fn forward(&mut self) {
            self.engine
                .default_lora
                .forward_into(&self.input_buffer, &mut self.output_buffer);
            self.engine.forward_count += 1;
        }

        /// Forward pass with typed array input (allocates output)
        #[wasm_bindgen]
        pub fn forward_array(&mut self, input: &[f32]) -> Vec<f32> {
            self.engine.forward(input)
        }

        /// Adapt using input buffer as gradient
        #[wasm_bindgen]
        pub fn adapt(&mut self) {
            self.engine.adapt(&self.input_buffer.clone());
        }

        /// Adapt with typed array gradient
        #[wasm_bindgen]
        pub fn adapt_array(&mut self, gradient: &[f32]) {
            self.engine.adapt(gradient);
        }

        /// Adapt with improvement reward using input buffer as gradient
        #[wasm_bindgen]
        pub fn adapt_with_reward(&mut self, improvement: f32) {
            self.engine
                .adapt_with_reward(&self.input_buffer.clone(), improvement);
        }

        /// Reset the engine
        #[wasm_bindgen]
        pub fn reset(&mut self) {
            self.engine.reset();
        }

        /// Get forward pass count
        #[wasm_bindgen]
        pub fn forward_count(&self) -> u64 {
            self.engine.forward_count
        }

        /// Get adaptation count
        #[wasm_bindgen]
        pub fn adapt_count(&self) -> u64 {
            self.engine.total_adapt_count
        }

        /// Get delta norm (weight change magnitude)
        #[wasm_bindgen]
        pub fn delta_norm(&self) -> f32 {
            self.engine.default_lora.delta_norm()
        }

        /// Get parameter count
        #[wasm_bindgen]
        pub fn param_count(&self) -> usize {
            self.engine.default_lora.param_count()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lora_pair_creation() {
        let config = LoRAConfig::default();
        let lora = LoRAPair::new(&config);
        assert_eq!(lora.dim, 256);
        assert_eq!(lora.adapt_count, 0);
    }

    #[test]
    fn test_lora_forward() {
        let config = LoRAConfig::default();
        let lora = LoRAPair::new(&config);

        let input = vec![1.0; 256];
        let output = lora.forward(&input);

        assert_eq!(output.len(), 256);
        // Initially B is zeros, so output should equal input
        for i in 0..256 {
            assert!((output[i] - input[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn test_lora_adapt() {
        let config = LoRAConfig::default();
        let mut lora = LoRAPair::new(&config);

        let gradient = vec![0.1; 256];
        lora.adapt(&gradient);

        assert_eq!(lora.adapt_count, 1);
        assert!(lora.delta_norm() > 0.0);
    }

    #[test]
    fn test_lora_forward_after_adapt() {
        let config = LoRAConfig::default();
        let mut lora = LoRAPair::new(&config);

        // Adapt
        let gradient = vec![0.1; 256];
        lora.adapt(&gradient);

        // Forward should now produce different output
        let input = vec![1.0; 256];
        let output = lora.forward(&input);

        // Output should differ from input after adaptation
        let mut diff = 0.0f32;
        for i in 0..256 {
            diff += (output[i] - input[i]).abs();
        }
        assert!(
            diff > 0.0,
            "Output should differ from input after adaptation"
        );
    }

    #[test]
    fn test_engine_stats() {
        let mut engine = MicroLoRAEngine::default();

        let input = vec![1.0; 256];
        let _ = engine.forward(&input);
        engine.adapt(&input);

        let (forwards, adapts, delta) = engine.stats();
        assert_eq!(forwards, 1);
        assert_eq!(adapts, 1);
        assert!(delta >= 0.0);
    }
}
