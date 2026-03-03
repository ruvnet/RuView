use crate::quantization::{QuantizedMlp, QuantizedWeights};
use rand::Rng;

/// Quantized MLP wrapper for temporal-compare
/// Provides 4x model size reduction with minimal accuracy loss
pub struct QuantizedMlpBackend {
    quantized: Option<QuantizedMlp>,

    // Training happens in FP32, quantize after
    weights1: Vec<f32>,
    bias1: Vec<f32>,
    weights2: Vec<f32>,
    bias2: Vec<f32>,

    input_dim: usize,
    hidden_dim: usize,
    output_dim: usize,

    // Track compression stats
    original_size: usize,
    quantized_size: usize,
}

impl QuantizedMlpBackend {
    pub fn new(input_dim: usize, hidden_dim: usize, output_dim: usize) -> Self {
        let mut rng = rand::thread_rng();

        // Xavier initialization
        let scale1 = (2.0 / input_dim as f32).sqrt();
        let scale2 = (2.0 / hidden_dim as f32).sqrt();

        let weights1: Vec<f32> = (0..hidden_dim * input_dim)
            .map(|_| rng.gen_range(-scale1..scale1))
            .collect();

        let weights2: Vec<f32> = (0..output_dim * hidden_dim)
            .map(|_| rng.gen_range(-scale2..scale2))
            .collect();

        let original_size = (weights1.len() + weights2.len() + hidden_dim + output_dim) * 4;

        Self {
            quantized: None,
            weights1,
            bias1: vec![0.0; hidden_dim],
            weights2,
            bias2: vec![0.0; output_dim],
            input_dim,
            hidden_dim,
            output_dim,
            original_size,
            quantized_size: 0,
        }
    }

    /// Train in FP32 for best accuracy
    pub fn train(&mut self, x: &[Vec<f32>], y: &[f32], epochs: usize, lr: f32) {
        for epoch in 0..epochs {
            let mut total_loss = 0.0;

            for (xi, &yi) in x.iter().zip(y.iter()) {
                // Forward pass (FP32)
                let mut hidden = vec![0.0f32; self.hidden_dim];

                // Layer 1
                for i in 0..self.hidden_dim {
                    let mut sum = self.bias1[i];
                    for j in 0..self.input_dim {
                        sum += self.weights1[i * self.input_dim + j] * xi[j];
                    }
                    hidden[i] = sum.max(0.0); // ReLU
                }

                // Layer 2
                let mut output = self.bias2[0];
                for i in 0..self.hidden_dim {
                    output += self.weights2[i] * hidden[i];
                }

                // Loss (MSE)
                let error = output - yi;
                total_loss += error * error;

                // Backward pass
                // Output layer gradients
                for i in 0..self.hidden_dim {
                    self.weights2[i] -= lr * error * hidden[i];
                }
                self.bias2[0] -= lr * error;

                // Hidden layer gradients
                for i in 0..self.hidden_dim {
                    if hidden[i] > 0.0 {
                        let grad = error * self.weights2[i];

                        for j in 0..self.input_dim {
                            self.weights1[i * self.input_dim + j] -= lr * grad * xi[j];
                        }
                        self.bias1[i] -= lr * grad;
                    }
                }
            }

            if epoch % 10 == 0 {
                println!("Epoch {}: Loss = {:.6}", epoch, total_loss / x.len() as f32);
            }
        }

        // Quantize after training
        self.quantize();
    }

    /// Quantize the trained FP32 model to INT8
    pub fn quantize(&mut self) {
        let qmlp = QuantizedMlp::from_float_mlp(
            &self.weights1,
            &self.bias1,
            &self.weights2,
            &self.bias2,
            self.input_dim,
            self.hidden_dim,
            self.output_dim
        );

        self.quantized_size = qmlp.model_size();
        self.quantized = Some(qmlp);
    }

    /// Predict using quantized weights (fast)
    pub fn predict(&self, x: &[Vec<f32>]) -> Vec<f32> {
        match &self.quantized {
            Some(qmlp) => {
                x.iter().map(|xi| {
                    let mut output = vec![0.0f32; self.output_dim];

                    #[cfg(target_arch = "x86_64")]
                    {
                        qmlp.forward_avx2(xi, &mut output);
                    }
                    #[cfg(not(target_arch = "x86_64"))]
                    {
                        qmlp.forward(xi, &mut output);
                    }

                    output[0]
                }).collect()
            }
            None => {
                // Fallback to FP32 if not quantized
                self.predict_fp32(x)
            }
        }
    }

    /// Predict using FP32 weights (for comparison)
    pub fn predict_fp32(&self, x: &[Vec<f32>]) -> Vec<f32> {
        x.iter().map(|xi| {
            let mut hidden = vec![0.0f32; self.hidden_dim];

            // Layer 1
            for i in 0..self.hidden_dim {
                let mut sum = self.bias1[i];
                for j in 0..self.input_dim {
                    sum += self.weights1[i * self.input_dim + j] * xi[j];
                }
                hidden[i] = sum.max(0.0);
            }

            // Layer 2
            let mut output = self.bias2[0];
            for i in 0..self.hidden_dim {
                output += self.weights2[i] * hidden[i];
            }

            output
        }).collect()
    }

    /// Classification prediction
    pub fn predict_class(&self, x: &[Vec<f32>]) -> Vec<usize> {
        self.predict(x).iter().map(|&y| {
            if y < -0.25 { 0 }
            else if y > 0.25 { 2 }
            else { 1 }
        }).collect()
    }

    /// Get compression statistics
    pub fn get_compression_stats(&self) -> (usize, usize, f32) {
        let ratio = if self.quantized_size > 0 {
            self.original_size as f32 / self.quantized_size as f32
        } else {
            1.0
        };

        (self.original_size, self.quantized_size, ratio)
    }

    /// Compare FP32 vs INT8 performance
    pub fn benchmark_inference(&self, x: &[Vec<f32>], iterations: usize) {
        use std::time::Instant;

        // Warm up
        let _ = self.predict_fp32(&x[..1.min(x.len())]);
        let _ = self.predict(&x[..1.min(x.len())]);

        // Benchmark FP32
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = self.predict_fp32(x);
        }
        let fp32_time = start.elapsed();

        // Benchmark INT8
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = self.predict(x);
        }
        let int8_time = start.elapsed();

        let speedup = fp32_time.as_secs_f32() / int8_time.as_secs_f32();

        println!("\n=== Quantization Benchmark ===");
        println!("FP32 time: {:.3}s", fp32_time.as_secs_f32());
        println!("INT8 time: {:.3}s", int8_time.as_secs_f32());
        println!("Speedup: {:.2}x", speedup);

        let (orig, quant, ratio) = self.get_compression_stats();
        println!("\n=== Model Size ===");
        println!("Original: {} bytes", orig);
        println!("Quantized: {} bytes", quant);
        println!("Compression: {:.2}x", ratio);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantized_mlp() {
        let mut model = QuantizedMlpBackend::new(32, 64, 1);

        // Create dummy data
        let x: Vec<Vec<f32>> = (0..100)
            .map(|_| (0..32).map(|_| rand::random()).collect())
            .collect();
        let y: Vec<f32> = (0..100).map(|_| rand::random()).collect();

        // Train and quantize
        model.train(&x, &y, 10, 0.01);

        // Check predictions work
        let pred_fp32 = model.predict_fp32(&x[..10]);
        let pred_int8 = model.predict(&x[..10]);

        // Should be similar but not identical
        for (p32, p8) in pred_fp32.iter().zip(&pred_int8) {
            let diff = (p32 - p8).abs();
            assert!(diff < 0.1, "Quantization error too large: {}", diff);
        }

        // Check compression
        let (_, _, ratio) = model.get_compression_stats();
        assert!(ratio > 3.0, "Compression ratio too low: {}", ratio);
    }
}