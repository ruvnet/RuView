use std::arch::x86_64::*;
use std::alloc::{alloc, dealloc, Layout};
use std::ptr;

/// Ultra-optimized CPU-only MLP with AVX-512 for lowest possible latency
/// Key optimizations:
/// - AVX-512: Process 16 floats at once (vs 8 with AVX2)
/// - Cache-aligned memory: Prevent false sharing
/// - Prefetching: Hide memory latency
/// - Loop unrolling: Reduce branch overhead
/// - Compile-time dimensions: Enable better optimization
pub struct UltraLowLatencyMlp<const INPUT: usize, const HIDDEN: usize, const OUTPUT: usize> {
    // Cache-aligned weight storage (64-byte aligned for AVX-512)
    w1: *mut f32,  // HIDDEN x INPUT (row-major for sequential access)
    b1: *mut f32,  // HIDDEN
    w2: *mut f32,  // OUTPUT x HIDDEN
    b2: *mut f32,  // OUTPUT

    // Pre-allocated buffers for zero-copy operation
    hidden_buf: *mut f32,  // HIDDEN (aligned)
}

impl<const I: usize, const H: usize, const O: usize> UltraLowLatencyMlp<I, H, O> {
    pub fn new() -> Self {
        unsafe {
            // Allocate cache-aligned memory (64 bytes for AVX-512)
            let align = 64;

            let w1 = Self::alloc_aligned(H * I, align);
            let b1 = Self::alloc_aligned(H, align);
            let w2 = Self::alloc_aligned(O * H, align);
            let b2 = Self::alloc_aligned(O, align);
            let hidden_buf = Self::alloc_aligned(H, align);

            // Initialize with small random weights
            Self::init_weights(w1, H * I);
            Self::init_weights(w2, O * H);
            ptr::write_bytes(b1, 0, H);
            ptr::write_bytes(b2, 0, O);

            Self { w1, b1, w2, b2, hidden_buf }
        }
    }

    #[inline(always)]
    unsafe fn alloc_aligned(size: usize, align: usize) -> *mut f32 {
        let layout = Layout::from_size_align(size * 4, align).unwrap();
        alloc(layout) as *mut f32
    }

    unsafe fn init_weights(ptr: *mut f32, size: usize) {
        let scale = (2.0 / size as f32).sqrt();
        for i in 0..size {
            *ptr.add(i) = (rand::random::<f32>() - 0.5) * scale;
        }
    }

    /// Ultra-fast forward pass with AVX-512
    /// Latency target: <100ns for typical sizes
    #[target_feature(enable = "avx512f")]
    #[inline]
    pub unsafe fn forward_avx512(&self, input: &[f32; I], output: &mut [f32; O]) {
        // Layer 1: Input -> Hidden with AVX-512 (16 floats at once)
        self.matmul_avx512(input.as_ptr(), self.w1, self.b1, self.hidden_buf, H, I);

        // ReLU activation (vectorized)
        self.relu_avx512(self.hidden_buf, H);

        // Layer 2: Hidden -> Output
        self.matmul_avx512(self.hidden_buf, self.w2, self.b2, output.as_mut_ptr(), O, H);
    }

    /// AVX-512 matrix multiply with prefetching
    #[target_feature(enable = "avx512f")]
    unsafe fn matmul_avx512(&self, x: *const f32, w: *const f32, b: *const f32,
                            out: *mut f32, rows: usize, cols: usize) {
        const SIMD_WIDTH: usize = 16;  // AVX-512 processes 16 floats

        // Process each output neuron
        for i in 0..rows {
            let row_offset = i * cols;

            // Prefetch next cache line
            _mm_prefetch(w.add(row_offset + 64) as *const i8, _MM_HINT_T0);

            // Initialize accumulator with bias
            let mut sum = _mm512_set1_ps(*b.add(i));

            // Process 16 elements at a time
            let chunks = cols / SIMD_WIDTH;
            let mut j = 0;

            // Unroll by 4 for better pipelining
            while j + 3 < chunks {
                let w0 = _mm512_load_ps(w.add(row_offset + j * SIMD_WIDTH));
                let x0 = _mm512_load_ps(x.add(j * SIMD_WIDTH));
                sum = _mm512_fmadd_ps(w0, x0, sum);

                let w1 = _mm512_load_ps(w.add(row_offset + (j + 1) * SIMD_WIDTH));
                let x1 = _mm512_load_ps(x.add((j + 1) * SIMD_WIDTH));
                sum = _mm512_fmadd_ps(w1, x1, sum);

                let w2 = _mm512_load_ps(w.add(row_offset + (j + 2) * SIMD_WIDTH));
                let x2 = _mm512_load_ps(x.add((j + 2) * SIMD_WIDTH));
                sum = _mm512_fmadd_ps(w2, x2, sum);

                let w3 = _mm512_load_ps(w.add(row_offset + (j + 3) * SIMD_WIDTH));
                let x3 = _mm512_load_ps(x.add((j + 3) * SIMD_WIDTH));
                sum = _mm512_fmadd_ps(w3, x3, sum);

                j += 4;
            }

            // Handle remaining chunks
            while j < chunks {
                let wv = _mm512_load_ps(w.add(row_offset + j * SIMD_WIDTH));
                let xv = _mm512_load_ps(x.add(j * SIMD_WIDTH));
                sum = _mm512_fmadd_ps(wv, xv, sum);
                j += 1;
            }

            // Horizontal sum (reduce 16 -> 1)
            let result = _mm512_reduce_add_ps(sum);

            // Handle remaining elements (non-SIMD)
            let mut scalar_sum = result;
            for k in (chunks * SIMD_WIDTH)..cols {
                scalar_sum += *w.add(row_offset + k) * *x.add(k);
            }

            *out.add(i) = scalar_sum;
        }
    }

    /// Vectorized ReLU with AVX-512
    #[target_feature(enable = "avx512f")]
    unsafe fn relu_avx512(&self, data: *mut f32, size: usize) {
        let zero = _mm512_setzero_ps();
        let chunks = size / 16;

        for i in 0..chunks {
            let val = _mm512_load_ps(data.add(i * 16));
            let relu = _mm512_max_ps(val, zero);
            _mm512_store_ps(data.add(i * 16), relu);
        }

        // Handle remainder
        for i in (chunks * 16)..size {
            let val = *data.add(i);
            *data.add(i) = val.max(0.0);
        }
    }

    /// Fallback for non-AVX512 systems (still optimized)
    pub fn forward_fallback(&self, input: &[f32; I], output: &mut [f32; O]) {
        unsafe {
            // Use AVX2 if available, otherwise scalar
            #[cfg(target_feature = "avx2")]
            {
                self.forward_avx512(input, output);
            }
            #[cfg(not(target_feature = "avx2"))]
            {
                // Scalar fallback implementation
                unsafe {
                    for i in 0..H {
                        let mut sum = *self.b1.add(i);
                        for j in 0..I {
                            sum += input[j] * *self.w1.add(i * I + j);
                        }
                        *self.hidden_buf.add(i) = sum.max(0.0);
                    }
                    for i in 0..O {
                        let mut sum = *self.b2.add(i);
                        for j in 0..H {
                            sum += *self.hidden_buf.add(j) * *self.w2.add(i * H + j);
                        }
                        output[i] = sum;
                    }
                }
            }
        }
    }

    /// Train with low-latency SGD (no allocations in hot path)
    pub fn train_fast(&mut self, x: &[f32; I], y: f32, lr: f32) {
        unsafe {
            let mut output = [0.0; O];

            // Forward pass
            self.forward_avx512(x, &mut output);

            // Compute error
            let error = output[0] - y;

            // Backward pass (simplified, no allocation)
            // Update output weights
            for i in 0..H {
                let grad = error * (*self.hidden_buf.add(i));
                *self.w2.add(i) -= lr * grad;
            }
            *self.b2 -= lr * error;

            // Backprop to hidden
            for i in 0..H {
                if *self.hidden_buf.add(i) > 0.0 {  // ReLU gradient
                    let hidden_error = error * (*self.w2.add(i));

                    // Update hidden weights
                    for j in 0..I {
                        *self.w1.add(i * I + j) -= lr * hidden_error * x[j];
                    }
                    *self.b1.add(i) -= lr * hidden_error;
                }
            }
        }
    }

    /// Batch prediction with minimal latency
    #[inline]
    pub fn predict_batch(&self, inputs: &[[f32; I]], outputs: &mut [[f32; O]]) {
        // Sequential processing for thread safety
        unsafe {
            for (x, y) in inputs.iter().zip(outputs.iter_mut()) {
                self.forward_avx512(x, y);
            }
        }
    }
}

impl<const I: usize, const H: usize, const O: usize> Drop for UltraLowLatencyMlp<I, H, O> {
    fn drop(&mut self) {
        unsafe {
            let align = 64;
            dealloc(self.w1 as *mut u8, Layout::from_size_align(H * I * 4, align).unwrap());
            dealloc(self.b1 as *mut u8, Layout::from_size_align(H * 4, align).unwrap());
            dealloc(self.w2 as *mut u8, Layout::from_size_align(O * H * 4, align).unwrap());
            dealloc(self.b2 as *mut u8, Layout::from_size_align(O * 4, align).unwrap());
            dealloc(self.hidden_buf as *mut u8, Layout::from_size_align(H * 4, align).unwrap());
        }
    }
}

/// Wrapper for dynamic dimensions
pub struct DynamicAvx512Mlp {
    weights_flat: Vec<f32>,
    dims: (usize, usize, usize),
}

impl DynamicAvx512Mlp {
    pub fn new(input: usize, hidden: usize, output: usize) -> Self {
        let total_params = (input * hidden) + hidden + (hidden * output) + output;
        let mut weights_flat = Vec::with_capacity(total_params);

        // Initialize
        let scale = (2.0 / input as f32).sqrt();
        for _ in 0..total_params {
            weights_flat.push((rand::random::<f32>() - 0.5) * scale);
        }

        Self { weights_flat, dims: (input, hidden, output) }
    }

    /// Predict with dynamic dimensions (still uses SIMD where possible)
    pub fn predict(&self, x: &[Vec<f32>]) -> Vec<f32> {
        let (input_dim, hidden_dim, _) = self.dims;

        x.iter().map(|xi| {
            let mut hidden = vec![0.0f32; hidden_dim];

            // Layer 1: Use AVX2 if available
            #[cfg(target_feature = "avx2")]
            unsafe {
                self.matmul_avx2_dynamic(&xi, &self.weights_flat[0..input_dim * hidden_dim],
                                         &mut hidden, hidden_dim, input_dim);
            }

            #[cfg(not(target_feature = "avx2"))]
            {
                // Scalar fallback
                for i in 0..hidden_dim {
                    let mut sum = self.weights_flat[input_dim * hidden_dim + i]; // bias
                    for j in 0..input_dim {
                        sum += xi[j] * self.weights_flat[i * input_dim + j];
                    }
                    hidden[i] = sum.max(0.0);
                }
            }

            // Layer 2 (simplified for single output)
            let w2_start = input_dim * hidden_dim + hidden_dim;
            let mut output = self.weights_flat[w2_start + hidden_dim]; // bias

            for i in 0..hidden_dim {
                output += hidden[i] * self.weights_flat[w2_start + i];
            }

            output
        }).collect()
    }

    #[cfg(target_feature = "avx2")]
    #[target_feature(enable = "avx2")]
    unsafe fn matmul_avx2_dynamic(&self, x: &[f32], w: &[f32], out: &mut [f32],
                                  rows: usize, cols: usize) {
        const SIMD_WIDTH: usize = 8;

        for i in 0..rows {
            let row_offset = i * cols;
            let mut sum = _mm256_setzero_ps();

            // Process 8 elements at a time
            let chunks = cols / SIMD_WIDTH;
            for j in 0..chunks {
                let idx = row_offset + j * SIMD_WIDTH;
                let wv = _mm256_loadu_ps(&w[idx]);
                let xv = _mm256_loadu_ps(&x[j * SIMD_WIDTH]);
                sum = _mm256_fmadd_ps(wv, xv, sum);
            }

            // Horizontal sum
            let sum_array: [f32; 8] = std::mem::transmute(sum);
            let mut result: f32 = sum_array.iter().sum();

            // Handle remainder
            for j in (chunks * SIMD_WIDTH)..cols {
                result += w[row_offset + j] * x[j];
            }

            out[i] = result.max(0.0); // ReLU
        }
    }

    pub fn predict_class(&self, x: &[Vec<f32>]) -> Vec<usize> {
        self.predict(x).iter().map(|&y| {
            if y < -0.25 { 0 }
            else if y > 0.25 { 2 }
            else { 1 }
        }).collect()
    }
}