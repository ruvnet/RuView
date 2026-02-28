//! MicroLoRA - Tiny Low-Rank Adaptation for ESP32
//!
//! Adapted from ruvLLM's SONA architecture for on-device adaptation.
//! Uses INT8 weights with rank 1-2 for minimal memory footprint.

use heapless::Vec as HVec;
use crate::quantized::QuantParams;

/// Maximum LoRA rank (keep very small for ESP32)
pub const MAX_LORA_RANK: usize = 2;
/// Maximum dimension for LoRA matrices
pub const MAX_LORA_DIM: usize = 64;

/// MicroLoRA configuration
#[derive(Debug, Clone, Copy)]
pub struct LoRAConfig {
    /// Rank of the low-rank matrices (1 or 2 for ESP32)
    pub rank: usize,
    /// Input/output dimension
    pub dim: usize,
    /// Scaling factor (alpha / rank)
    pub scale: i8,
    /// Whether LoRA is frozen (inference-only)
    pub frozen: bool,
}

impl Default for LoRAConfig {
    fn default() -> Self {
        Self {
            rank: 1,
            dim: 32,
            scale: 8, // alpha=8, rank=1 -> scale=8
            frozen: true,
        }
    }
}

/// MicroLoRA adapter for a single layer
///
/// Implements: output = input + scale * (input @ A) @ B
/// Where A is [dim, rank] and B is [rank, dim]
pub struct MicroLoRA {
    /// Down projection: A matrix [dim, rank] as INT8
    a_weights: HVec<i8, { MAX_LORA_DIM * MAX_LORA_RANK }>,
    /// Up projection: B matrix [rank, dim] as INT8
    b_weights: HVec<i8, { MAX_LORA_RANK * MAX_LORA_DIM }>,
    /// Configuration
    config: LoRAConfig,
    /// Quantization params for A
    a_params: QuantParams,
    /// Quantization params for B
    b_params: QuantParams,
    /// Intermediate buffer for rank-sized vector
    intermediate: [i32; MAX_LORA_RANK],
}

impl MicroLoRA {
    /// Create new MicroLoRA with random initialization
    pub fn new(config: LoRAConfig, seed: u32) -> crate::Result<Self> {
        if config.rank > MAX_LORA_RANK || config.dim > MAX_LORA_DIM {
            return Err(crate::Error::InvalidModel("LoRA dimensions too large"));
        }

        let mut a_weights = HVec::new();
        let mut b_weights = HVec::new();

        let mut rng_state = seed;
        let mut next_rand = || {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            (((rng_state >> 16) & 0x3F) as i16 - 32) as i8 // Small values [-32, 31]
        };

        // Initialize A with small random values
        for _ in 0..(config.dim * config.rank) {
            a_weights.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
        }

        // Initialize B with zeros (LoRA starts as identity)
        for _ in 0..(config.rank * config.dim) {
            b_weights.push(0).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self {
            a_weights,
            b_weights,
            config,
            a_params: QuantParams::default(),
            b_params: QuantParams::default(),
            intermediate: [0; MAX_LORA_RANK],
        })
    }

    /// Create MicroLoRA from pre-trained weights
    pub fn from_weights(
        config: LoRAConfig,
        a_weights: &[i8],
        b_weights: &[i8],
    ) -> crate::Result<Self> {
        if a_weights.len() != config.dim * config.rank {
            return Err(crate::Error::InvalidModel("A weights size mismatch"));
        }
        if b_weights.len() != config.rank * config.dim {
            return Err(crate::Error::InvalidModel("B weights size mismatch"));
        }

        let mut a_vec = HVec::new();
        let mut b_vec = HVec::new();

        for &w in a_weights {
            a_vec.push(w).map_err(|_| crate::Error::BufferOverflow)?;
        }
        for &w in b_weights {
            b_vec.push(w).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self {
            a_weights: a_vec,
            b_weights: b_vec,
            config,
            a_params: QuantParams::default(),
            b_params: QuantParams::default(),
            intermediate: [0; MAX_LORA_RANK],
        })
    }

    /// Apply LoRA adaptation to input
    ///
    /// Computes: output = input + scale * (input @ A) @ B
    /// All operations in INT8/INT32
    #[inline]
    pub fn apply(&mut self, input: &[i8], output: &mut [i32]) {
        let dim = self.config.dim;
        let rank = self.config.rank;
        let scale = self.config.scale as i32;

        // Clear intermediate buffer
        for i in 0..rank {
            self.intermediate[i] = 0;
        }

        // Step 1: intermediate = input @ A (down projection)
        // A is [dim, rank], input is [dim], result is [rank]
        for r in 0..rank {
            let mut sum: i32 = 0;
            for d in 0..dim {
                sum += input[d] as i32 * self.a_weights[d * rank + r] as i32;
            }
            self.intermediate[r] = sum >> 4; // Scale down to prevent overflow
        }

        // Step 2: lora_output = intermediate @ B (up projection)
        // B is [rank, dim], intermediate is [rank], result is [dim]
        for d in 0..dim {
            let mut sum: i32 = 0;
            for r in 0..rank {
                sum += self.intermediate[r] * self.b_weights[r * dim + d] as i32;
            }
            // Add scaled LoRA output to original output
            output[d] += (sum * scale) >> 8;
        }
    }

    /// Apply LoRA and store result in-place
    pub fn apply_inplace(&mut self, data: &mut [i32], input: &[i8]) {
        self.apply(input, data);
    }

    /// Memory size of this LoRA adapter
    pub fn memory_size(&self) -> usize {
        self.a_weights.len() + self.b_weights.len()
    }

    /// Update LoRA weights with gradient (simplified for on-device learning)
    ///
    /// Uses a simple gradient accumulation approach suitable for ESP32:
    /// A += lr * input^T @ grad_intermediate
    /// B += lr * intermediate^T @ grad_output
    #[cfg(not(feature = "frozen"))]
    pub fn update(&mut self, input: &[i8], grad_output: &[i32], learning_rate: i8) {
        let dim = self.config.dim;
        let rank = self.config.rank;
        let lr = learning_rate as i32;

        // Compute gradient for intermediate (simplified)
        let mut grad_intermediate = [0i32; MAX_LORA_RANK];
        for r in 0..rank {
            let mut sum: i32 = 0;
            for d in 0..dim {
                sum += grad_output[d] * self.b_weights[r * dim + d] as i32;
            }
            grad_intermediate[r] = sum >> 8;
        }

        // Update A weights: A += lr * outer(input, grad_intermediate)
        for d in 0..dim {
            for r in 0..rank {
                let grad = (input[d] as i32 * grad_intermediate[r] * lr) >> 12;
                let idx = d * rank + r;
                let new_val = self.a_weights[idx] as i32 + grad;
                self.a_weights[idx] = new_val.clamp(-127, 127) as i8;
            }
        }

        // Update B weights: B += lr * outer(intermediate, grad_output)
        for r in 0..rank {
            for d in 0..dim {
                let grad = (self.intermediate[r] * grad_output[d] * lr) >> 12;
                let idx = r * dim + d;
                let new_val = self.b_weights[idx] as i32 + grad;
                self.b_weights[idx] = new_val.clamp(-127, 127) as i8;
            }
        }
    }
}

/// Collection of MicroLoRA adapters for all layers
pub struct LoRAStack<const NUM_LAYERS: usize> {
    /// LoRA adapters per layer
    adapters: [Option<MicroLoRA>; NUM_LAYERS],
    /// Number of active adapters
    active_count: usize,
}

impl<const NUM_LAYERS: usize> LoRAStack<NUM_LAYERS> {
    /// Create empty LoRA stack
    pub fn new() -> Self {
        Self {
            adapters: core::array::from_fn(|_| None),
            active_count: 0,
        }
    }

    /// Add LoRA adapter to a layer
    pub fn add_adapter(&mut self, layer_idx: usize, adapter: MicroLoRA) -> crate::Result<()> {
        if layer_idx >= NUM_LAYERS {
            return Err(crate::Error::InvalidModel("Layer index out of range"));
        }
        self.adapters[layer_idx] = Some(adapter);
        self.active_count += 1;
        Ok(())
    }

    /// Get adapter for a layer (if exists)
    pub fn get(&mut self, layer_idx: usize) -> Option<&mut MicroLoRA> {
        self.adapters.get_mut(layer_idx).and_then(|a| a.as_mut())
    }

    /// Total memory used by all adapters
    pub fn total_memory(&self) -> usize {
        self.adapters.iter()
            .filter_map(|a| a.as_ref())
            .map(|a| a.memory_size())
            .sum()
    }
}

impl<const N: usize> Default for LoRAStack<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_micro_lora_creation() {
        let config = LoRAConfig {
            rank: 2,
            dim: 32,
            scale: 8,
            frozen: true,
        };

        let lora = MicroLoRA::new(config, 42).unwrap();

        // A: 32 * 2 = 64 bytes, B: 2 * 32 = 64 bytes
        assert_eq!(lora.memory_size(), 128);
    }

    #[test]
    fn test_lora_apply() {
        let config = LoRAConfig {
            rank: 1,
            dim: 4,
            scale: 64, // Larger scale for testing
            frozen: true,
        };

        // Create with known weights - larger values to survive scaling
        let a_weights = [16i8, 32, 48, 64]; // [4, 1]
        let b_weights = [64i8, 64, 64, 64]; // [1, 4]

        let mut lora = MicroLoRA::from_weights(config, &a_weights, &b_weights).unwrap();

        let input = [64i8, 64, 64, 64];
        let mut output = [0i32; 4];

        lora.apply(&input, &mut output);

        // With larger values, the output should be non-zero after scaling
        // intermediate = sum(64 * [16,32,48,64]) >> 4 = (10240) >> 4 = 640
        // output = (640 * 64 * scale) >> 8
        // This should produce non-zero results
        let non_zero_count = output.iter().filter(|&&o| o != 0).count();
        assert!(non_zero_count > 0, "At least some outputs should be non-zero, got {:?}", output);
    }

    #[test]
    fn test_lora_stack() {
        let mut stack = LoRAStack::<4>::new();

        let config = LoRAConfig::default();
        let adapter = MicroLoRA::new(config, 42).unwrap();

        stack.add_adapter(0, adapter).unwrap();

        assert!(stack.get(0).is_some());
        assert!(stack.get(1).is_none());
        assert!(stack.total_memory() > 0);
    }
}
