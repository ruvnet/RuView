//! LoRA (Low-Rank Adaptation) implementations for SONA
//!
//! Two-tier LoRA system:
//! - MicroLoRA: Rank 1-2, per-request adaptation (<100μs)
//! - BaseLoRA: Rank 4-16, background adaptation (hourly)

use crate::types::LearningSignal;
use serde::{Deserialize, Serialize};

/// Optimal batch size for processing (benchmark-validated)
pub const OPTIMAL_BATCH_SIZE: usize = 32;

/// Micro-LoRA for per-request adaptation
///
/// Uses rank 1-2 for ultra-low latency updates.
/// Forward pass: output += scale * (input @ down) @ up
///
/// **Performance notes (from benchmarks):**
/// - Rank-2 is ~5% faster than Rank-1 due to better SIMD vectorization
/// - Batch size 32 optimal: 0.447ms per-vector, 2,236 ops/sec throughput
/// - SIMD-enabled: +10% speedup over scalar
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicroLoRA {
    /// Down projection (hidden_dim -> rank)
    down_proj: Vec<f32>,
    /// Up projection (rank -> hidden_dim)
    up_proj: Vec<f32>,
    /// Rank (1-2 for micro updates)
    rank: usize,
    /// Hidden dimension
    hidden_dim: usize,
    /// Accumulated gradients for down
    #[serde(skip)]
    grad_down: Vec<f32>,
    /// Accumulated gradients for up
    #[serde(skip)]
    grad_up: Vec<f32>,
    /// Update count for averaging
    #[serde(skip)]
    update_count: usize,
    /// Scaling factor
    scale: f32,
}

impl MicroLoRA {
    /// Create new Micro-LoRA adapter
    ///
    /// # Arguments
    /// * `hidden_dim` - Model hidden dimension
    /// * `rank` - LoRA rank (must be 1-2)
    ///
    /// # Panics
    /// Panics if rank > 2
    pub fn new(hidden_dim: usize, rank: usize) -> Self {
        assert!(
            (1..=2).contains(&rank),
            "MicroLoRA rank must be 1-2, got {}",
            rank
        );

        // Initialize down with small random-like values (deterministic for reproducibility)
        let down_proj: Vec<f32> = (0..hidden_dim * rank)
            .map(|i| {
                let x = (i as f32 * 0.618_034) % 1.0;
                (x - 0.5) * 0.02
            })
            .collect();

        // Initialize up to zero (standard LoRA init)
        let up_proj = vec![0.0f32; rank * hidden_dim];

        Self {
            down_proj,
            up_proj,
            rank,
            hidden_dim,
            grad_down: vec![0.0; hidden_dim * rank],
            grad_up: vec![0.0; rank * hidden_dim],
            update_count: 0,
            scale: 1.0 / (rank as f32).sqrt(),
        }
    }

    /// Scalar forward pass (fallback)
    pub fn forward_scalar(&self, input: &[f32], output: &mut [f32]) {
        assert_eq!(input.len(), self.hidden_dim);
        assert_eq!(output.len(), self.hidden_dim);

        // Down projection: hidden_dim -> rank
        let mut intermediate = vec![0.0f32; self.rank];
        for (r, inter) in intermediate.iter_mut().enumerate() {
            let mut sum = 0.0f32;
            let offset = r * self.hidden_dim;
            for (i, &inp) in input.iter().enumerate() {
                sum += inp * self.down_proj[offset + i];
            }
            *inter = sum;
        }

        // Up projection: rank -> hidden_dim
        for (i, out) in output.iter_mut().enumerate() {
            let mut sum = 0.0f32;
            for (r, &inter) in intermediate.iter().enumerate() {
                sum += inter * self.up_proj[r * self.hidden_dim + i];
            }
            *out += sum * self.scale;
        }
    }

    /// SIMD-optimized forward pass (AVX2)
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    pub fn forward_simd(&self, input: &[f32], output: &mut [f32]) {
        use std::arch::x86_64::*;

        assert_eq!(input.len(), self.hidden_dim);
        assert_eq!(output.len(), self.hidden_dim);

        unsafe {
            // Down projection: hidden_dim -> rank
            let mut intermediate = vec![0.0f32; self.rank];

            for r in 0..self.rank {
                let mut sum = _mm256_setzero_ps();
                let offset = r * self.hidden_dim;

                let mut i = 0;
                while i + 8 <= self.hidden_dim {
                    let inp = _mm256_loadu_ps(input[i..].as_ptr());
                    let weight = _mm256_loadu_ps(self.down_proj[offset + i..].as_ptr());
                    sum = _mm256_fmadd_ps(inp, weight, sum);
                    i += 8;
                }

                // Horizontal sum
                let mut result = [0.0f32; 8];
                _mm256_storeu_ps(result.as_mut_ptr(), sum);
                intermediate[r] = result.iter().sum();

                // Handle remaining elements
                for j in i..self.hidden_dim {
                    intermediate[r] += input[j] * self.down_proj[offset + j];
                }
            }

            // Up projection: rank -> hidden_dim
            let scale_vec = _mm256_set1_ps(self.scale);

            let mut i = 0;
            while i + 8 <= self.hidden_dim {
                let mut sum = _mm256_setzero_ps();

                for r in 0..self.rank {
                    let up_offset = r * self.hidden_dim;
                    let weight = _mm256_loadu_ps(self.up_proj[up_offset + i..].as_ptr());
                    let inter = _mm256_set1_ps(intermediate[r]);
                    sum = _mm256_fmadd_ps(inter, weight, sum);
                }

                // Scale and add to output
                sum = _mm256_mul_ps(sum, scale_vec);
                let existing = _mm256_loadu_ps(output[i..].as_ptr());
                let result = _mm256_add_ps(existing, sum);
                _mm256_storeu_ps(output[i..].as_mut_ptr(), result);

                i += 8;
            }

            // Handle remaining elements
            for j in i..self.hidden_dim {
                let mut val = 0.0;
                for r in 0..self.rank {
                    val += intermediate[r] * self.up_proj[r * self.hidden_dim + j];
                }
                output[j] += val * self.scale;
            }
        }
    }

    /// Forward pass with automatic SIMD detection
    pub fn forward(&self, input: &[f32], output: &mut [f32]) {
        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        {
            self.forward_simd(input, output);
            return;
        }

        #[allow(unreachable_code)]
        self.forward_scalar(input, output);
    }

    /// Accumulate gradient from learning signal
    pub fn accumulate_gradient(&mut self, signal: &LearningSignal) {
        if signal.gradient_estimate.len() != self.hidden_dim {
            return;
        }

        let quality = signal.quality_score;

        // Simplified gradient: outer product scaled by quality
        // This approximates the true gradient for rank-1 LoRA
        for r in 0..self.rank {
            for i in 0..self.hidden_dim {
                let grad_idx = r * self.hidden_dim + i;
                // Update up projection gradient (main target)
                self.grad_up[grad_idx] += signal.gradient_estimate[i] * quality;
            }
        }

        self.update_count += 1;
    }

    /// Apply accumulated gradients with learning rate
    pub fn apply_accumulated(&mut self, learning_rate: f32) {
        if self.update_count == 0 {
            return;
        }

        let scale = learning_rate / self.update_count as f32;

        // Update up projection (main adaptation target)
        for (w, g) in self.up_proj.iter_mut().zip(self.grad_up.iter()) {
            *w += g * scale;
        }

        // Reset accumulators
        self.grad_up.fill(0.0);
        self.grad_down.fill(0.0);
        self.update_count = 0;
    }

    /// Reset adapter to initial state
    pub fn reset(&mut self) {
        self.up_proj.fill(0.0);
        self.grad_up.fill(0.0);
        self.grad_down.fill(0.0);
        self.update_count = 0;
    }

    /// Get rank
    pub fn rank(&self) -> usize {
        self.rank
    }

    /// Get hidden dimension
    pub fn hidden_dim(&self) -> usize {
        self.hidden_dim
    }

    /// Get parameter count
    pub fn param_count(&self) -> usize {
        self.down_proj.len() + self.up_proj.len()
    }

    /// Get scale factor
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Set scale factor
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }

    /// Get pending update count
    pub fn pending_updates(&self) -> usize {
        self.update_count
    }

    /// Get LoRA weights for export (lora_a, lora_b)
    pub fn get_weights(&self) -> (&Vec<f32>, &Vec<f32>) {
        (&self.down_proj, &self.up_proj)
    }
}

/// Base LoRA for background adaptation
///
/// Higher rank (4-16) for more expressive adaptation.
/// Applied hourly during background learning cycles.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BaseLoRA {
    /// LoRA layers
    pub layers: Vec<LoRALayer>,
    /// Rank
    pub rank: usize,
    /// Hidden dimension
    pub hidden_dim: usize,
    /// Alpha scaling factor
    pub alpha: f32,
}

/// Single LoRA layer
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoRALayer {
    /// Down projection weights
    pub down_proj: Vec<f32>,
    /// Up projection weights
    pub up_proj: Vec<f32>,
    /// Layer index
    pub layer_idx: usize,
}

impl BaseLoRA {
    /// Create new Base LoRA
    pub fn new(hidden_dim: usize, rank: usize, num_layers: usize) -> Self {
        let layers = (0..num_layers)
            .map(|idx| LoRALayer {
                down_proj: vec![0.0; hidden_dim * rank],
                up_proj: vec![0.0; rank * hidden_dim],
                layer_idx: idx,
            })
            .collect();

        Self {
            layers,
            rank,
            hidden_dim,
            alpha: rank as f32,
        }
    }

    /// Forward pass for single layer
    pub fn forward_layer(&self, layer_idx: usize, input: &[f32], output: &mut [f32]) {
        if layer_idx >= self.layers.len() {
            return;
        }

        let layer = &self.layers[layer_idx];
        let scale = self.alpha / self.rank as f32;

        // Down projection
        let mut intermediate = vec![0.0f32; self.rank];
        for (r, inter) in intermediate.iter_mut().enumerate() {
            let offset = r * self.hidden_dim;
            *inter = input
                .iter()
                .zip(&layer.down_proj[offset..offset + self.hidden_dim])
                .map(|(a, b)| a * b)
                .sum();
        }

        // Up projection
        for (i, out) in output.iter_mut().enumerate() {
            let mut sum = 0.0f32;
            for (r, &inter) in intermediate.iter().enumerate() {
                sum += inter * layer.up_proj[r * self.hidden_dim + i];
            }
            *out += sum * scale;
        }
    }

    /// Merge LoRA weights into model weights (for inference optimization)
    pub fn merge_into(&self, model_weights: &mut [f32], layer_idx: usize) {
        if layer_idx >= self.layers.len() {
            return;
        }

        let layer = &self.layers[layer_idx];
        let scale = self.alpha / self.rank as f32;

        // W' = W + scale * (down @ up)
        // Assumes model_weights is [hidden_dim x hidden_dim]
        for i in 0..self.hidden_dim {
            for j in 0..self.hidden_dim {
                let mut delta = 0.0f32;
                for r in 0..self.rank {
                    delta +=
                        layer.down_proj[i * self.rank + r] * layer.up_proj[r * self.hidden_dim + j];
                }
                model_weights[i * self.hidden_dim + j] += delta * scale;
            }
        }
    }

    /// Get number of layers
    pub fn num_layers(&self) -> usize {
        self.layers.len()
    }

    /// Get total parameter count
    pub fn param_count(&self) -> usize {
        self.layers.len() * (self.hidden_dim * self.rank + self.rank * self.hidden_dim)
    }

    /// Get weights for a specific layer for export (lora_a, lora_b)
    pub fn get_layer_weights(&self, layer_idx: usize) -> Option<(&Vec<f32>, &Vec<f32>)> {
        self.layers
            .get(layer_idx)
            .map(|layer| (&layer.down_proj, &layer.up_proj))
    }
}

/// Combined LoRA engine managing both tiers
#[derive(Clone, Debug)]
pub struct LoRAEngine {
    /// Micro-LoRA for instant adaptation
    pub micro: MicroLoRA,
    /// Base LoRA for background adaptation
    pub base: BaseLoRA,
    /// Whether micro-LoRA is enabled
    pub micro_enabled: bool,
    /// Whether base LoRA is enabled
    pub base_enabled: bool,
}

impl LoRAEngine {
    /// Create new LoRA engine
    pub fn new(hidden_dim: usize, micro_rank: usize, base_rank: usize, num_layers: usize) -> Self {
        Self {
            micro: MicroLoRA::new(hidden_dim, micro_rank.clamp(1, 2)),
            base: BaseLoRA::new(hidden_dim, base_rank, num_layers),
            micro_enabled: true,
            base_enabled: true,
        }
    }

    /// Apply both LoRA tiers
    pub fn forward(&self, layer_idx: usize, input: &[f32], output: &mut [f32]) {
        if self.micro_enabled {
            self.micro.forward(input, output);
        }
        if self.base_enabled && layer_idx < self.base.num_layers() {
            self.base.forward_layer(layer_idx, input, output);
        }
    }

    /// Accumulate micro-LoRA gradient
    pub fn accumulate_micro(&mut self, signal: &LearningSignal) {
        if self.micro_enabled {
            self.micro.accumulate_gradient(signal);
        }
    }

    /// Apply micro-LoRA updates
    pub fn apply_micro(&mut self, learning_rate: f32) {
        if self.micro_enabled {
            self.micro.apply_accumulated(learning_rate);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_micro_lora_creation() {
        let lora = MicroLoRA::new(256, 1);
        assert_eq!(lora.rank(), 1);
        assert_eq!(lora.hidden_dim(), 256);
        assert_eq!(lora.param_count(), 256 + 256);
    }

    #[test]
    fn test_micro_lora_forward() {
        let lora = MicroLoRA::new(64, 1);
        let input = vec![1.0f32; 64];
        let mut output = vec![0.0f32; 64];

        lora.forward(&input, &mut output);

        // Output should be modified (even if small due to init)
        // With zero-init up_proj, output should still be zero
        let sum: f32 = output.iter().sum();
        assert!(
            sum.abs() < 1e-6,
            "Expected ~0 with zero up_proj, got {}",
            sum
        );
    }

    #[test]
    fn test_micro_lora_learning() {
        let mut lora = MicroLoRA::new(64, 1);

        let signal = LearningSignal::with_gradient(vec![0.1; 64], vec![0.5; 64], 0.8);

        lora.accumulate_gradient(&signal);
        assert_eq!(lora.pending_updates(), 1);

        lora.apply_accumulated(0.01);
        assert_eq!(lora.pending_updates(), 0);

        // Now forward should produce non-zero output
        let input = vec![1.0f32; 64];
        let mut output = vec![0.0f32; 64];
        lora.forward(&input, &mut output);

        let sum: f32 = output.iter().map(|x| x.abs()).sum();
        assert!(sum > 0.0, "Expected non-zero output after learning");
    }

    #[test]
    fn test_base_lora() {
        let lora = BaseLoRA::new(64, 4, 12);
        assert_eq!(lora.num_layers(), 12);
        assert_eq!(lora.rank, 4);
    }

    #[test]
    fn test_lora_engine() {
        let mut engine = LoRAEngine::new(64, 1, 4, 12);

        let signal = LearningSignal::with_gradient(vec![0.1; 64], vec![0.5; 64], 0.9);

        engine.accumulate_micro(&signal);
        engine.apply_micro(0.01);

        let input = vec![1.0f32; 64];
        let mut output = vec![0.0f32; 64];
        engine.forward(0, &input, &mut output);
    }

    #[test]
    #[should_panic(expected = "MicroLoRA rank must be 1-2")]
    fn test_invalid_rank() {
        MicroLoRA::new(64, 5);
    }
}
