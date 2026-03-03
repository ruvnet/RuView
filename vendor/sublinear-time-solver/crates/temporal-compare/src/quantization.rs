use std::ops::{Add, Mul};

/// INT8 Quantization for 4x size reduction and 2x faster inference
/// Symmetric quantization: maps [-max, max] to [-127, 127]
#[derive(Clone, Debug)]
pub struct Int8Quantizer {
    pub scale: f32,
    pub zero_point: i8,
}

impl Int8Quantizer {
    /// Create quantizer from float weights
    pub fn from_weights(weights: &[f32]) -> Self {
        // Find min/max for dynamic range
        let (min, max) = weights.iter().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(min, max), &w| (min.min(w), max.max(w))
        );

        // Symmetric quantization for better accuracy
        let abs_max = min.abs().max(max.abs());
        let scale = abs_max / 127.0;

        Self {
            scale,
            zero_point: 0, // Symmetric around zero
        }
    }

    /// Quantize float to INT8
    #[inline]
    pub fn quantize(&self, value: f32) -> i8 {
        let scaled = value / self.scale;
        scaled.round().clamp(-127.0, 127.0) as i8
    }

    /// Dequantize INT8 back to float
    #[inline]
    pub fn dequantize(&self, value: i8) -> f32 {
        value as f32 * self.scale
    }

    /// Quantize entire array
    pub fn quantize_array(&self, values: &[f32]) -> Vec<i8> {
        values.iter().map(|&v| self.quantize(v)).collect()
    }

    /// Dequantize entire array
    pub fn dequantize_array(&self, values: &[i8]) -> Vec<f32> {
        values.iter().map(|&v| self.dequantize(v)).collect()
    }
}

/// Quantized weight storage for neural networks
pub struct QuantizedWeights {
    pub weights: Vec<i8>,
    pub quantizer: Int8Quantizer,
    pub shape: (usize, usize),
}

impl QuantizedWeights {
    pub fn from_float_matrix(weights: &[f32], rows: usize, cols: usize) -> Self {
        let quantizer = Int8Quantizer::from_weights(weights);
        let quantized = quantizer.quantize_array(weights);

        Self {
            weights: quantized,
            quantizer,
            shape: (rows, cols),
        }
    }

    /// Get dequantized weight at position
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> f32 {
        let idx = row * self.shape.1 + col;
        self.quantizer.dequantize(self.weights[idx])
    }

    /// Quantized matrix multiply with on-the-fly dequantization
    /// Still faster than FP32 due to better cache utilization
    pub fn matmul_quantized(&self, input: &[f32], output: &mut [f32]) {
        let (rows, cols) = self.shape;

        for i in 0..rows {
            let mut sum = 0.0f32;
            let row_offset = i * cols;

            // Process in chunks for better cache performance
            for j in 0..cols {
                let w_int8 = self.weights[row_offset + j];
                // Delay dequantization to minimize float operations
                sum += (w_int8 as f32) * input[j];
            }

            // Apply scale once at the end
            output[i] = sum * self.quantizer.scale;
        }
    }

    /// SIMD-accelerated quantized matmul (AVX2)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn matmul_quantized_avx2(&self, input: &[f32], output: &mut [f32]) {
        use std::arch::x86_64::*;

        let (rows, cols) = self.shape;
        let scale = _mm256_set1_ps(self.quantizer.scale);

        for i in 0..rows {
            let mut sum = _mm256_setzero_ps();
            let row_offset = i * cols;

            // Process 8 elements at once
            let chunks = cols / 8;
            for j in 0..chunks {
                let idx = j * 8;

                // Load 8 INT8 weights and convert to float
                let w_ptr = self.weights.as_ptr().add(row_offset + idx);
                let w_i8 = _mm_loadl_epi64(w_ptr as *const __m128i);
                let w_i32 = _mm256_cvtepi8_epi32(w_i8);
                let w_f32 = _mm256_cvtepi32_ps(w_i32);

                // Load 8 input floats
                let x = _mm256_loadu_ps(&input[idx]);

                // Multiply and accumulate
                sum = _mm256_fmadd_ps(w_f32, x, sum);
            }

            // Horizontal sum
            let sum_array: [f32; 8] = std::mem::transmute(sum);
            let mut result = sum_array.iter().sum::<f32>();

            // Handle remainder
            for j in (chunks * 8)..cols {
                result += (self.weights[row_offset + j] as f32) * input[j];
            }

            // Apply scale
            output[i] = result * self.quantizer.scale;
        }
    }

    /// Get memory size in bytes
    pub fn memory_size(&self) -> usize {
        self.weights.len() // INT8 = 1 byte each
    }
}

/// Quantized MLP with INT8 weights
pub struct QuantizedMlp {
    pub w1: QuantizedWeights,
    pub b1: Vec<f32>, // Keep biases as FP32 (small overhead)
    pub w2: QuantizedWeights,
    pub b2: Vec<f32>,
    hidden_dim: usize,
}

impl QuantizedMlp {
    /// Create from existing float MLP
    pub fn from_float_mlp(w1: &[f32], b1: &[f32], w2: &[f32], b2: &[f32],
                          input_dim: usize, hidden_dim: usize, output_dim: usize) -> Self {
        Self {
            w1: QuantizedWeights::from_float_matrix(w1, hidden_dim, input_dim),
            b1: b1.to_vec(),
            w2: QuantizedWeights::from_float_matrix(w2, output_dim, hidden_dim),
            b2: b2.to_vec(),
            hidden_dim,
        }
    }

    /// Forward pass with INT8 weights
    pub fn forward(&self, input: &[f32], output: &mut [f32]) {
        let mut hidden = vec![0.0f32; self.hidden_dim];

        // Layer 1: Input -> Hidden
        self.w1.matmul_quantized(input, &mut hidden);

        // Add bias and ReLU
        for (h, &b) in hidden.iter_mut().zip(&self.b1) {
            *h = (*h + b).max(0.0);
        }

        // Layer 2: Hidden -> Output
        self.w2.matmul_quantized(&hidden, output);

        // Add bias
        for (o, &b) in output.iter_mut().zip(&self.b2) {
            *o += b;
        }
    }

    /// Forward pass with AVX2 acceleration
    #[cfg(target_arch = "x86_64")]
    pub fn forward_avx2(&self, input: &[f32], output: &mut [f32]) {
        unsafe {
            let mut hidden = vec![0.0f32; self.hidden_dim];

            // Layer 1 with SIMD
            self.w1.matmul_quantized_avx2(input, &mut hidden);

            // Vectorized bias + ReLU
            use std::arch::x86_64::*;
            let zero = _mm256_setzero_ps();

            for i in (0..self.hidden_dim).step_by(8) {
                if i + 8 <= self.hidden_dim {
                    let h = _mm256_loadu_ps(&hidden[i]);
                    let b = _mm256_loadu_ps(&self.b1[i]);
                    let sum = _mm256_add_ps(h, b);
                    let relu = _mm256_max_ps(sum, zero);
                    _mm256_storeu_ps(&mut hidden[i], relu);
                } else {
                    // Handle remainder
                    for j in i..self.hidden_dim {
                        hidden[j] = (hidden[j] + self.b1[j]).max(0.0);
                    }
                }
            }

            // Layer 2 with SIMD
            self.w2.matmul_quantized_avx2(&hidden, output);

            // Add output bias
            for (o, &b) in output.iter_mut().zip(&self.b2) {
                *o += b;
            }
        }
    }

    /// Get total model size in bytes
    pub fn model_size(&self) -> usize {
        self.w1.memory_size() +
        self.w2.memory_size() +
        (self.b1.len() + self.b2.len()) * 4 // FP32 biases
    }

    /// Compression ratio vs FP32
    pub fn compression_ratio(&self, original_params: usize) -> f32 {
        let original_bytes = original_params * 4; // FP32
        let quantized_bytes = self.model_size();
        original_bytes as f32 / quantized_bytes as f32
    }
}

/// Quantization-aware training for better INT8 accuracy
pub struct QuantizationAwareTraining {
    pub fake_quantize: bool,
    pub num_bits: u8,
}

impl QuantizationAwareTraining {
    pub fn new() -> Self {
        Self {
            fake_quantize: true,
            num_bits: 8,
        }
    }

    /// Fake quantization during training to simulate INT8 effects
    pub fn fake_quantize_weights(&self, weights: &mut [f32]) {
        if !self.fake_quantize {
            return;
        }

        let quantizer = Int8Quantizer::from_weights(weights);

        for w in weights.iter_mut() {
            // Quantize and immediately dequantize to simulate INT8 effects
            let quantized = quantizer.quantize(*w);
            *w = quantizer.dequantize(quantized);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantization_round_trip() {
        let weights = vec![-1.5, -0.5, 0.0, 0.5, 1.5];
        let quantizer = Int8Quantizer::from_weights(&weights);

        for &w in &weights {
            let q = quantizer.quantize(w);
            let dq = quantizer.dequantize(q);

            // Should be close but not exact due to quantization
            assert!((w - dq).abs() < 0.02);
        }
    }

    #[test]
    fn test_compression_ratio() {
        let input_dim = 32;
        let hidden_dim = 64;
        let output_dim = 3;

        let total_params = (input_dim * hidden_dim) + hidden_dim +
                          (hidden_dim * output_dim) + output_dim;

        let w1 = vec![0.1; input_dim * hidden_dim];
        let b1 = vec![0.0; hidden_dim];
        let w2 = vec![0.1; hidden_dim * output_dim];
        let b2 = vec![0.0; output_dim];

        let qmlp = QuantizedMlp::from_float_mlp(
            &w1, &b1, &w2, &b2,
            input_dim, hidden_dim, output_dim
        );

        let ratio = qmlp.compression_ratio(total_params);

        // Should achieve ~3.5-4x compression (weights are INT8, biases stay FP32)
        assert!(ratio > 3.0 && ratio < 4.5);
        println!("Compression ratio: {:.2}x", ratio);
        println!("Original size: {} bytes", total_params * 4);
        println!("Quantized size: {} bytes", qmlp.model_size());
    }
}