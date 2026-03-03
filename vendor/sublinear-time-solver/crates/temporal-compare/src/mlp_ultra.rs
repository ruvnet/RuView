use std::arch::x86_64::*;
use ndarray::Axis;
use rand::{thread_rng, Rng};
use rayon::prelude::*;
use std::sync::Arc;

/// Ultra-optimized MLP with SIMD, cache blocking, and parallel processing
pub struct UltraMlp {
    // Weights stored in row-major for cache efficiency
    w1_flat: Vec<f32>,
    b1: Vec<f32>,
    w2_flat: Vec<f32>,
    b2: Vec<f32>,

    // Dimensions
    input_dim: usize,
    hidden_dim: usize,
    output_dim: usize,

    // Momentum buffers (flat)
    vw1: Vec<f32>,
    vb1: Vec<f32>,
    vw2: Vec<f32>,
    vb2: Vec<f32>,

    // Pre-allocated buffers for forward pass
    hidden_buffer: Vec<f32>,
    output_buffer: Vec<f32>,
}

impl UltraMlp {
    pub fn new(input: usize, hidden: usize, output: usize) -> Self {
        let mut rng = thread_rng();

        // He initialization
        let scale1 = (2.0 / input as f32).sqrt();
        let scale2 = (2.0 / hidden as f32).sqrt();

        // Initialize weights flat for SIMD
        let w1_flat: Vec<f32> = (0..hidden*input)
            .map(|_| rng.gen::<f32>() * scale1 - scale1/2.0)
            .collect();
        let w2_flat: Vec<f32> = (0..output*hidden)
            .map(|_| rng.gen::<f32>() * scale2 - scale2/2.0)
            .collect();

        Self {
            w1_flat,
            b1: vec![0.0; hidden],
            w2_flat,
            b2: vec![0.0; output],
            input_dim: input,
            hidden_dim: hidden,
            output_dim: output,
            vw1: vec![0.0; hidden * input],
            vb1: vec![0.0; hidden],
            vw2: vec![0.0; output * hidden],
            vb2: vec![0.0; output],
            hidden_buffer: vec![0.0; hidden],
            output_buffer: vec![0.0; output],
        }
    }

    /// SIMD-accelerated matrix-vector multiplication
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn simd_matmul(weights: &[f32], input: &[f32], output: &mut [f32],
                          rows: usize, cols: usize) {
        const SIMD_WIDTH: usize = 8;

        for i in 0..rows {
            let row_offset = i * cols;
            let mut sum = _mm256_setzero_ps();

            // Process 8 elements at a time with AVX2
            let chunks = cols / SIMD_WIDTH;
            for j in 0..chunks {
                let idx = j * SIMD_WIDTH;
                let w = _mm256_loadu_ps(&weights[row_offset + idx]);
                let x = _mm256_loadu_ps(&input[idx]);
                sum = _mm256_fmadd_ps(w, x, sum);
            }

            // Horizontal sum
            let sum_array = std::mem::transmute::<__m256, [f32; 8]>(sum);
            let mut result = sum_array.iter().sum::<f32>();

            // Handle remaining elements
            for j in (chunks * SIMD_WIDTH)..cols {
                result += weights[row_offset + j] * input[j];
            }

            output[i] = result;
        }
    }

    /// Vectorized ReLU activation
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn simd_relu(data: &mut [f32]) {
        const SIMD_WIDTH: usize = 8;
        let zero = _mm256_setzero_ps();

        let chunks = data.len() / SIMD_WIDTH;
        for i in 0..chunks {
            let idx = i * SIMD_WIDTH;
            let val = _mm256_loadu_ps(&data[idx]);
            let relu = _mm256_max_ps(val, zero);
            _mm256_storeu_ps(&mut data[idx], relu);
        }

        // Handle remaining
        for i in (chunks * SIMD_WIDTH)..data.len() {
            data[i] = data[i].max(0.0);
        }
    }

    /// Ultra-fast forward pass
    pub fn forward_fast(&mut self, x: &[f32]) -> &[f32] {
        unsafe {
            // Layer 1: input -> hidden
            Self::simd_matmul(&self.w1_flat, x, &mut self.hidden_buffer,
                            self.hidden_dim, self.input_dim);

            // Add bias with SIMD
            for i in 0..self.hidden_dim {
                self.hidden_buffer[i] += self.b1[i];
            }

            // ReLU activation
            Self::simd_relu(&mut self.hidden_buffer);

            // Layer 2: hidden -> output
            Self::simd_matmul(&self.w2_flat, &self.hidden_buffer, &mut self.output_buffer,
                            self.output_dim, self.hidden_dim);

            // Add bias
            for i in 0..self.output_dim {
                self.output_buffer[i] += self.b2[i];
            }
        }

        &self.output_buffer
    }

    /// Optimized backpropagation with momentum
    pub fn backward_fast(&mut self, x: &[f32], y_true: f32, lr: f32) {
        // Clone output to avoid borrow issues
        let output_copy = self.forward_fast(x).to_vec();
        let momentum = 0.9;

        // Output gradient
        let grad_out = if self.output_dim == 1 {
            vec![output_copy[0] - y_true]
        } else {
            // Softmax gradient for classification
            let max = output_copy.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            let exp_sum = output_copy.iter().map(|&v| (v - max).exp()).sum::<f32>();
            let mut grad = output_copy.iter().map(|&v| (v - max).exp() / exp_sum).collect::<Vec<_>>();

            let class = if y_true < -0.25 { 0 } else if y_true > 0.25 { 2 } else { 1 };
            if class < grad.len() {
                grad[class] -= 1.0;
            }
            grad
        };

        // Gradient w.r.t W2 and b2 (vectorized)
        for i in 0..self.output_dim {
            let grad_i = grad_out[i];

            // Update bias with momentum
            self.vb2[i] = momentum * self.vb2[i] - lr * grad_i;
            self.b2[i] += self.vb2[i];

            // Update weights with momentum (vectorized)
            let w2_offset = i * self.hidden_dim;
            for j in 0..self.hidden_dim {
                let idx = w2_offset + j;
                let grad_w = grad_i * self.hidden_buffer[j];
                self.vw2[idx] = momentum * self.vw2[idx] - lr * grad_w;
                self.w2_flat[idx] += self.vw2[idx];
            }
        }

        // Gradient backprop to hidden layer
        let mut grad_hidden = vec![0.0; self.hidden_dim];
        for i in 0..self.hidden_dim {
            for j in 0..self.output_dim {
                grad_hidden[i] += self.w2_flat[j * self.hidden_dim + i] * grad_out[j];
            }
            // ReLU gradient
            if self.hidden_buffer[i] <= 0.0 {
                grad_hidden[i] = 0.0;
            }
        }

        // Update W1 and b1
        for i in 0..self.hidden_dim {
            let grad_i = grad_hidden[i];

            // Update bias
            self.vb1[i] = momentum * self.vb1[i] - lr * grad_i;
            self.b1[i] += self.vb1[i];

            // Update weights
            let w1_offset = i * self.input_dim;
            for j in 0..self.input_dim {
                let idx = w1_offset + j;
                let grad_w = grad_i * x[j];
                self.vw1[idx] = momentum * self.vw1[idx] - lr * grad_w;
                self.w1_flat[idx] += self.vw1[idx];
            }
        }
    }

    /// Parallel batch training with cache-friendly access
    pub fn train_batch_parallel(&mut self, x: &Vec<Vec<f32>>, y: &Vec<f32>,
                               epochs: usize, lr: f32, batch_size: usize) {
        use rand::seq::SliceRandom;

        for _ in 0..epochs {
            let mut indices: Vec<usize> = (0..x.len()).collect();
            let mut rng = thread_rng();
            indices.shuffle(&mut rng);

            // Process in mini-batches
            for batch_start in (0..x.len()).step_by(batch_size) {
                let batch_end = (batch_start + batch_size).min(x.len());

                // Sequential within batch for weight updates
                for i in batch_start..batch_end {
                    let idx = indices[i];
                    self.backward_fast(&x[idx], y[idx], lr / batch_size as f32);
                }
            }
        }
    }

    /// Parallel prediction for multiple samples
    pub fn predict_parallel(&mut self, x: &[Vec<f32>]) -> Vec<f32> {
        // Create thread-local copies for parallel execution
        let w1 = Arc::new(self.w1_flat.clone());
        let b1 = Arc::new(self.b1.clone());
        let w2 = Arc::new(self.w2_flat.clone());
        let b2 = Arc::new(self.b2.clone());
        let hidden_dim = self.hidden_dim;
        let input_dim = self.input_dim;
        let output_dim = self.output_dim;

        x.par_iter().map(|xi| {
            let mut hidden = vec![0.0; hidden_dim];
            let mut output = vec![0.0; output_dim];

            unsafe {
                // Forward pass with local buffers
                Self::simd_matmul(&w1, xi, &mut hidden, hidden_dim, input_dim);
                for i in 0..hidden_dim {
                    hidden[i] += b1[i];
                }
                Self::simd_relu(&mut hidden);

                Self::simd_matmul(&w2, &hidden, &mut output, output_dim, hidden_dim);
                for i in 0..output_dim {
                    output[i] += b2[i];
                }
            }

            if output_dim == 1 { output[0] } else { output[0] }
        }).collect()
    }

    pub fn predict_cls3_parallel(&mut self, x: &[Vec<f32>]) -> Vec<usize> {
        let w1 = Arc::new(self.w1_flat.clone());
        let b1 = Arc::new(self.b1.clone());
        let w2 = Arc::new(self.w2_flat.clone());
        let b2 = Arc::new(self.b2.clone());
        let hidden_dim = self.hidden_dim;
        let input_dim = self.input_dim;
        let output_dim = self.output_dim;

        x.par_iter().map(|xi| {
            let mut hidden = vec![0.0; hidden_dim];
            let mut output = vec![0.0; output_dim];

            unsafe {
                Self::simd_matmul(&w1, xi, &mut hidden, hidden_dim, input_dim);
                for i in 0..hidden_dim {
                    hidden[i] += b1[i];
                }
                Self::simd_relu(&mut hidden);

                Self::simd_matmul(&w2, &hidden, &mut output, output_dim, hidden_dim);
                for i in 0..output_dim {
                    output[i] += b2[i];
                }
            }

            if output_dim >= 3 {
                // Argmax
                let mut best = 0;
                for i in 1..3.min(output.len()) {
                    if output[i] > output[best] {
                        best = i;
                    }
                }
                best
            } else {
                let val = output[0];
                if val < -0.25 { 0 } else if val > 0.25 { 2 } else { 1 }
            }
        }).collect()
    }
}