//! MicroLoRA: Ultra-fast per-query adaptation

use ndarray::{Array1, Array2};

#[derive(Debug, Clone)]
pub struct MicroLoRAConfig {
    pub rank: usize,  // 1-2 for micro
    pub alpha: f32,   // Scaling factor
    pub dropout: f32, // Dropout rate
}

impl Default for MicroLoRAConfig {
    fn default() -> Self {
        Self {
            rank: 2,
            alpha: 1.0,
            dropout: 0.0,
        }
    }
}

pub struct MicroLoRA {
    config: MicroLoRAConfig,
    a_matrix: Array2<f32>, // (in_dim, rank)
    b_matrix: Array2<f32>, // (rank, out_dim)
    #[allow(dead_code)]
    in_dim: usize,
    #[allow(dead_code)]
    out_dim: usize,
}

impl MicroLoRA {
    pub fn new(config: MicroLoRAConfig, dim: usize) -> Self {
        let rank = config.rank;
        // Initialize A with small random values, B with zeros
        let a_matrix = Array2::from_shape_fn((dim, rank), |_| (rand::random::<f32>() - 0.5) * 0.01);
        let b_matrix = Array2::zeros((rank, dim));

        Self {
            config,
            a_matrix,
            b_matrix,
            in_dim: dim,
            out_dim: dim,
        }
    }

    /// Forward pass: x + alpha * (x @ A @ B)
    pub fn forward(&self, x: &Array1<f32>) -> Array1<f32> {
        let low_rank = x.dot(&self.a_matrix).dot(&self.b_matrix);
        x + &(low_rank * self.config.alpha)
    }

    /// Adapt weights based on gradient signal
    pub fn adapt(&mut self, gradient: &Array1<f32>, learning_rate: f32) {
        // Update B matrix based on gradient (rank-1 update)
        // This is the "instant" adaptation - must be <100μs
        let grad_norm = gradient.mapv(|x| x * x).sum().sqrt();
        if grad_norm > 1e-8 {
            let normalized = gradient / grad_norm;
            // Outer product update to B
            for i in 0..self.config.rank {
                for j in 0..self.out_dim {
                    self.b_matrix[[i, j]] +=
                        learning_rate * self.a_matrix.column(i).sum() * normalized[j];
                }
            }
        }
    }

    /// Reset to initial state
    pub fn reset(&mut self) {
        self.b_matrix.fill(0.0);
    }

    /// Get parameter count
    pub fn param_count(&self) -> usize {
        self.a_matrix.len() + self.b_matrix.len()
    }
}
