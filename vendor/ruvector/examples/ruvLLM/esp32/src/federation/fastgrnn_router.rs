//! FastGRNN-Inspired Micro Router for ESP32
//!
//! Lightweight gated routing for dynamic chip selection.
//! Adapted from ruvector's FastGRNN for minimal compute overhead.
//!
//! Key differences from full FastGRNN:
//! - INT8 weights instead of FP32
//! - Fixed-point gate computation
//! - Minimal hidden dimension (4-8)

use heapless::Vec as HVec;
use super::protocol::ChipId;

/// Maximum hidden dimension for micro router
pub const MAX_ROUTER_HIDDEN: usize = 8;
/// Maximum input features
pub const MAX_ROUTER_INPUT: usize = 16;

/// Micro FastGRNN configuration
#[derive(Debug, Clone, Copy)]
pub struct MicroGRNNConfig {
    /// Input dimension
    pub input_dim: usize,
    /// Hidden dimension
    pub hidden_dim: usize,
    /// Number of output classes (chips)
    pub num_chips: usize,
    /// Zeta parameter (gate scaling)
    pub zeta: i8,
    /// Nu parameter (update scaling)
    pub nu: i8,
}

impl Default for MicroGRNNConfig {
    fn default() -> Self {
        Self {
            input_dim: 8,
            hidden_dim: 4,
            num_chips: 5,
            zeta: 16,
            nu: 16,
        }
    }
}

/// Micro FastGRNN cell for routing decisions
pub struct MicroFastGRNN {
    config: MicroGRNNConfig,
    /// Gate weights: W_g [input_dim * hidden_dim] + U_g [hidden_dim * hidden_dim]
    w_gate: HVec<i8, 128>,
    u_gate: HVec<i8, 64>,
    /// Update weights: W_u, U_u
    w_update: HVec<i8, 128>,
    u_update: HVec<i8, 64>,
    /// Biases
    bias_gate: HVec<i8, MAX_ROUTER_HIDDEN>,
    bias_update: HVec<i8, MAX_ROUTER_HIDDEN>,
    /// Output projection to chips
    w_output: HVec<i8, 64>,
    /// Hidden state
    hidden: HVec<i32, MAX_ROUTER_HIDDEN>,
}

impl MicroFastGRNN {
    /// Create new micro FastGRNN
    pub fn new(config: MicroGRNNConfig, seed: u32) -> crate::Result<Self> {
        let mut rng_state = seed;
        let mut next_rand = || {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            (((rng_state >> 16) & 0x3F) as i16 - 32) as i8
        };

        // Initialize weights
        let gate_size = config.input_dim * config.hidden_dim;
        let hidden_size = config.hidden_dim * config.hidden_dim;
        let output_size = config.hidden_dim * config.num_chips;

        let mut w_gate = HVec::new();
        let mut u_gate = HVec::new();
        let mut w_update = HVec::new();
        let mut u_update = HVec::new();
        let mut w_output = HVec::new();
        let mut bias_gate = HVec::new();
        let mut bias_update = HVec::new();
        let mut hidden = HVec::new();

        for _ in 0..gate_size {
            w_gate.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
            w_update.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
        }
        for _ in 0..hidden_size {
            u_gate.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
            u_update.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
        }
        for _ in 0..output_size {
            w_output.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
        }
        for _ in 0..config.hidden_dim {
            bias_gate.push(0).map_err(|_| crate::Error::BufferOverflow)?;
            bias_update.push(0).map_err(|_| crate::Error::BufferOverflow)?;
            hidden.push(0).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self {
            config,
            w_gate,
            u_gate,
            w_update,
            u_update,
            bias_gate,
            bias_update,
            w_output,
            hidden,
        })
    }

    /// Reset hidden state
    pub fn reset(&mut self) {
        for h in self.hidden.iter_mut() {
            *h = 0;
        }
    }

    /// Fixed-point sigmoid approximation
    #[inline]
    fn sigmoid_fp(x: i32) -> i32 {
        // Piecewise linear sigmoid: clamp to [0, 256] representing [0, 1]
        if x < -512 { 0 }
        else if x > 512 { 256 }
        else { (x + 512) >> 2 }
    }

    /// Fixed-point tanh approximation
    #[inline]
    fn tanh_fp(x: i32) -> i32 {
        // Piecewise linear tanh: clamp to [-256, 256] representing [-1, 1]
        if x < -512 { -256 }
        else if x > 512 { 256 }
        else { x >> 1 }
    }

    /// Matrix-vector multiply (INT8 weights, INT32 accumulator)
    fn matmul(&self, weights: &[i8], input: &[i32], rows: usize, cols: usize) -> HVec<i32, MAX_ROUTER_HIDDEN> {
        let mut output = HVec::new();

        for r in 0..rows {
            let mut sum: i32 = 0;
            for c in 0..cols {
                if c < input.len() {
                    sum += weights[r * cols + c] as i32 * input[c];
                }
            }
            let _ = output.push(sum >> 8); // Scale down
        }

        output
    }

    /// One step of FastGRNN computation
    ///
    /// h_new = (1 - z) ⊙ h + z ⊙ tanh(W_u*x + U_u*h + b_u)
    /// where z = sigmoid(W_g*x + U_g*h + b_g)
    pub fn step(&mut self, input: &[i8]) -> crate::Result<()> {
        // Convert input to i32
        let input_i32: HVec<i32, MAX_ROUTER_INPUT> = input.iter()
            .take(self.config.input_dim)
            .map(|&x| x as i32 * 16) // Scale up
            .collect();

        // Compute gate: z = sigmoid(W_g * x + U_g * h + b_g)
        let wx_gate = self.matmul(&self.w_gate, &input_i32, self.config.hidden_dim, self.config.input_dim);
        let uh_gate = self.matmul(&self.u_gate, &self.hidden, self.config.hidden_dim, self.config.hidden_dim);

        let mut gate = HVec::<i32, MAX_ROUTER_HIDDEN>::new();
        for i in 0..self.config.hidden_dim {
            let wx = wx_gate.get(i).copied().unwrap_or(0);
            let uh = uh_gate.get(i).copied().unwrap_or(0);
            let b = self.bias_gate.get(i).copied().unwrap_or(0) as i32 * 16;
            let z = Self::sigmoid_fp((wx + uh + b) * self.config.zeta as i32 / 16);
            let _ = gate.push(z);
        }

        // Compute update: u = tanh(W_u * x + U_u * h + b_u)
        let wx_update = self.matmul(&self.w_update, &input_i32, self.config.hidden_dim, self.config.input_dim);
        let uh_update = self.matmul(&self.u_update, &self.hidden, self.config.hidden_dim, self.config.hidden_dim);

        // Update hidden state: h = (1 - z) * h + z * u
        for i in 0..self.config.hidden_dim {
            let wx = wx_update.get(i).copied().unwrap_or(0);
            let uh = uh_update.get(i).copied().unwrap_or(0);
            let b = self.bias_update.get(i).copied().unwrap_or(0) as i32 * 16;
            let u = Self::tanh_fp((wx + uh + b) * self.config.nu as i32 / 16);

            let z = gate.get(i).copied().unwrap_or(128);
            let h = self.hidden.get(i).copied().unwrap_or(0);

            // h_new = (256 - z) * h / 256 + z * u / 256
            let h_new = ((256 - z) * h + z * u) >> 8;
            self.hidden[i] = h_new;
        }

        Ok(())
    }

    /// Get routing decision (which chip to use)
    pub fn route(&self) -> ChipId {
        // Output projection: scores = W_o * hidden
        let mut scores = [0i32; 8];

        for chip in 0..self.config.num_chips {
            let mut sum: i32 = 0;
            for h in 0..self.config.hidden_dim {
                let w_idx = chip * self.config.hidden_dim + h;
                let w = self.w_output.get(w_idx).copied().unwrap_or(0) as i32;
                let hidden = self.hidden.get(h).copied().unwrap_or(0);
                sum += w * hidden;
            }
            scores[chip] = sum;
        }

        // Find argmax
        let mut best_chip = 0;
        let mut best_score = scores[0];
        for (i, &score) in scores[..self.config.num_chips].iter().enumerate() {
            if score > best_score {
                best_score = score;
                best_chip = i;
            }
        }

        ChipId(best_chip as u8)
    }

    /// Get routing probabilities (softmax-like)
    pub fn route_probs(&self) -> HVec<u8, 8> {
        let mut probs = HVec::new();
        let mut scores = [0i32; 8];
        let mut max_score = i32::MIN;

        // Compute scores
        for chip in 0..self.config.num_chips {
            let mut sum: i32 = 0;
            for h in 0..self.config.hidden_dim {
                let w_idx = chip * self.config.hidden_dim + h;
                let w = self.w_output.get(w_idx).copied().unwrap_or(0) as i32;
                let hidden = self.hidden.get(h).copied().unwrap_or(0);
                sum += w * hidden;
            }
            scores[chip] = sum;
            if sum > max_score {
                max_score = sum;
            }
        }

        // Simple softmax approximation
        let mut total: i32 = 0;
        for chip in 0..self.config.num_chips {
            let exp_score = (scores[chip] - max_score + 256).max(1);
            scores[chip] = exp_score;
            total += exp_score;
        }

        for chip in 0..self.config.num_chips {
            let prob = (scores[chip] * 255 / total.max(1)) as u8;
            let _ = probs.push(prob);
        }

        probs
    }

    /// Memory size
    pub fn memory_size(&self) -> usize {
        self.w_gate.len() + self.u_gate.len() +
        self.w_update.len() + self.u_update.len() +
        self.w_output.len() +
        self.bias_gate.len() + self.bias_update.len() +
        self.hidden.len() * 4
    }
}

/// Feature extractor for routing input
pub struct RoutingFeatures {
    /// Token embedding summary (mean)
    pub embed_mean: i8,
    /// Token embedding variance proxy
    pub embed_var: i8,
    /// Current sequence position (normalized)
    pub position: i8,
    /// Current load on each chip (0-127)
    pub chip_loads: [i8; 5],
}

impl RoutingFeatures {
    /// Convert to input vector
    pub fn to_input(&self) -> [i8; 8] {
        [
            self.embed_mean,
            self.embed_var,
            self.position,
            self.chip_loads[0],
            self.chip_loads[1],
            self.chip_loads[2],
            self.chip_loads[3],
            self.chip_loads[4],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_micro_fastgrnn() {
        let config = MicroGRNNConfig::default();
        let mut router = MicroFastGRNN::new(config, 42).unwrap();

        // Test step
        let input = [10i8, 20, 30, 40, 50, 60, 70, 80];
        router.step(&input).unwrap();

        // Should produce valid routing
        let chip = router.route();
        assert!(chip.0 < 5);

        println!("Memory: {} bytes", router.memory_size());
    }

    #[test]
    fn test_routing_probs() {
        let config = MicroGRNNConfig::default();
        let mut router = MicroFastGRNN::new(config, 42).unwrap();

        let input = [10i8; 8];
        router.step(&input).unwrap();

        let probs = router.route_probs();
        assert_eq!(probs.len(), 5);

        // Sum should be approximately 255
        let sum: i32 = probs.iter().map(|&p| p as i32).sum();
        assert!(sum > 200 && sum < 280);
    }
}
