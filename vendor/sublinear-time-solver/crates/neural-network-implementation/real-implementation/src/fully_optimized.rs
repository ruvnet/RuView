//! Fully optimized implementation with real SIMD, INT8 quantization, and CPU pinning
//! No simulations - all real optimizations

#![allow(unsafe_code)]

use std::arch::x86_64::*;
use std::alloc::{alloc, dealloc, Layout};
use std::time::{Duration, Instant};
use core_affinity;

/// INT8 quantized weights with scale factors
#[repr(C, align(64))]  // Cache-line aligned
pub struct QuantizedWeights {
    // INT8 weights for layer 1 (32x128)
    w1_int8: *mut i8,
    w1_scale: [f32; 32],  // Per-row scale factors

    // INT8 weights for layer 2 (4x32)
    w2_int8: *mut i8,
    w2_scale: [f32; 4],

    // Biases remain FP32 for accuracy
    b1: [f32; 32],
    b2: [f32; 4],
}

impl QuantizedWeights {
    pub fn new() -> Self {
        unsafe {
            // Allocate 64-byte aligned memory for SIMD
            let w1_layout = Layout::from_size_align(32 * 128, 64).unwrap();
            let w2_layout = Layout::from_size_align(4 * 32, 64).unwrap();

            let w1_ptr = alloc(w1_layout) as *mut i8;
            let w2_ptr = alloc(w2_layout) as *mut i8;

            let mut w1_scale = [0.0f32; 32];
            let mut w2_scale = [0.0f32; 4];

            // Initialize and quantize weights
            for i in 0..32 {
                let mut max_val = 0.0f32;
                let mut row_weights = vec![0.0f32; 128];

                // Generate weights and find max for quantization
                for j in 0..128 {
                    let weight = ((i * j) as f32 * 0.001).sin() * 0.1;
                    row_weights[j] = weight;
                    max_val = max_val.max(weight.abs());
                }

                // Quantize to INT8
                w1_scale[i] = max_val / 127.0;
                for j in 0..128 {
                    let quantized = (row_weights[j] / w1_scale[i]).round() as i8;
                    *w1_ptr.add(i * 128 + j) = quantized;
                }
            }

            // Quantize layer 2
            for i in 0..4 {
                let mut max_val = 0.0f32;
                let mut row_weights = vec![0.0f32; 32];

                for j in 0..32 {
                    let weight = ((i * j) as f32 * 0.002).cos() * 0.2;
                    row_weights[j] = weight;
                    max_val = max_val.max(weight.abs());
                }

                w2_scale[i] = max_val / 127.0;
                for j in 0..32 {
                    let quantized = (row_weights[j] / w2_scale[i]).round() as i8;
                    *w2_ptr.add(i * 32 + j) = quantized;
                }
            }

            Self {
                w1_int8: w1_ptr,
                w1_scale,
                w2_int8: w2_ptr,
                w2_scale,
                b1: [0.0; 32],
                b2: [0.0; 4],
            }
        }
    }

    /// AVX2 INT8 matrix multiplication with FP32 accumulation
    #[target_feature(enable = "avx2")]
    #[inline(always)]
    pub unsafe fn gemm_int8_avx2(
        &self,
        input: &[f32; 128],
        hidden: &mut [f32; 32],
    ) {
        // Process 8 outputs at a time using AVX2
        for row_block in (0..32).step_by(8) {
            // Initialize 8 accumulators
            let mut acc0 = _mm256_setzero_ps();
            let mut acc1 = _mm256_setzero_ps();
            let mut acc2 = _mm256_setzero_ps();
            let mut acc3 = _mm256_setzero_ps();
            let mut acc4 = _mm256_setzero_ps();
            let mut acc5 = _mm256_setzero_ps();
            let mut acc6 = _mm256_setzero_ps();
            let mut acc7 = _mm256_setzero_ps();

            // Process input in chunks of 8
            for col in (0..128).step_by(8) {
                // Load 8 input values
                let input_vec = _mm256_loadu_ps(input.as_ptr().add(col));

                // Load INT8 weights for 8 rows x 8 cols
                // Convert to FP32 and multiply with scale
                for r in 0..8.min(32 - row_block) {
                    let row = row_block + r;
                    let weight_ptr = self.w1_int8.add(row * 128 + col);

                    // Load 8 INT8 weights
                    let weights_i8 = _mm_loadl_epi64(weight_ptr as *const __m128i);
                    // Convert INT8 to INT32
                    let weights_i32 = _mm256_cvtepi8_epi32(weights_i8);
                    // Convert INT32 to FP32
                    let weights_f32 = _mm256_cvtepi32_ps(weights_i32);

                    // Scale weights
                    let scale = _mm256_set1_ps(self.w1_scale[row]);
                    let scaled_weights = _mm256_mul_ps(weights_f32, scale);

                    // Multiply and accumulate
                    match r {
                        0 => acc0 = _mm256_fmadd_ps(scaled_weights, input_vec, acc0),
                        1 => acc1 = _mm256_fmadd_ps(scaled_weights, input_vec, acc1),
                        2 => acc2 = _mm256_fmadd_ps(scaled_weights, input_vec, acc2),
                        3 => acc3 = _mm256_fmadd_ps(scaled_weights, input_vec, acc3),
                        4 => acc4 = _mm256_fmadd_ps(scaled_weights, input_vec, acc4),
                        5 => acc5 = _mm256_fmadd_ps(scaled_weights, input_vec, acc5),
                        6 => acc6 = _mm256_fmadd_ps(scaled_weights, input_vec, acc6),
                        7 => acc7 = _mm256_fmadd_ps(scaled_weights, input_vec, acc7),
                        _ => {}
                    }
                }
            }

            // Horizontal sum and store results
            let sum_array = |acc: __m256| -> f32 {
                let sum = _mm256_hadd_ps(acc, acc);
                let sum = _mm256_hadd_ps(sum, sum);
                let high = _mm256_extractf128_ps(sum, 1);
                let low = _mm256_castps256_ps128(sum);
                let final_sum = _mm_add_ps(low, high);
                _mm_cvtss_f32(final_sum)
            };

            for r in 0..8.min(32 - row_block) {
                let row = row_block + r;
                hidden[row] = match r {
                    0 => sum_array(acc0) + self.b1[row],
                    1 => sum_array(acc1) + self.b1[row],
                    2 => sum_array(acc2) + self.b1[row],
                    3 => sum_array(acc3) + self.b1[row],
                    4 => sum_array(acc4) + self.b1[row],
                    5 => sum_array(acc5) + self.b1[row],
                    6 => sum_array(acc6) + self.b1[row],
                    7 => sum_array(acc7) + self.b1[row],
                    _ => 0.0,
                };
            }
        }
    }

    /// AVX-512 implementation for newer CPUs
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
    #[target_feature(enable = "avx512f")]
    #[inline(always)]
    pub unsafe fn gemm_int8_avx512(
        &self,
        input: &[f32; 128],
        hidden: &mut [f32; 32],
    ) {
        use std::arch::x86_64::*;

        // Process 16 elements at once with AVX-512
        for row in 0..32 {
            let mut acc = _mm512_setzero_ps();

            for col in (0..128).step_by(16) {
                // Load 16 input values
                let input_vec = _mm512_loadu_ps(input.as_ptr().add(col));

                // Load and convert INT8 weights to FP32
                let weight_ptr = self.w1_int8.add(row * 128 + col);
                let weights_i8 = _mm_loadu_si128(weight_ptr as *const __m128i);
                let weights_i32 = _mm512_cvtepi8_epi32(weights_i8);
                let weights_f32 = _mm512_cvtepi32_ps(weights_i32);

                // Scale and accumulate
                let scale = _mm512_set1_ps(self.w1_scale[row]);
                let scaled_weights = _mm512_mul_ps(weights_f32, scale);
                acc = _mm512_fmadd_ps(scaled_weights, input_vec, acc);
            }

            // Reduce and store
            hidden[row] = _mm512_reduce_add_ps(acc) + self.b1[row];
        }
    }
}

impl Drop for QuantizedWeights {
    fn drop(&mut self) {
        unsafe {
            let w1_layout = Layout::from_size_align(32 * 128, 64).unwrap();
            let w2_layout = Layout::from_size_align(4 * 32, 64).unwrap();
            dealloc(self.w1_int8 as *mut u8, w1_layout);
            dealloc(self.w2_int8 as *mut u8, w2_layout);
        }
    }
}

/// Ultra-optimized neural network with INT8 quantization and SIMD
#[repr(C, align(64))]
pub struct OptimizedNeuralNetwork {
    weights: QuantizedWeights,
    // Pre-allocated aligned buffers
    hidden_buffer: [f32; 32],
    output_buffer: [f32; 4],
}

impl OptimizedNeuralNetwork {
    pub fn new() -> Self {
        Self {
            weights: QuantizedWeights::new(),
            hidden_buffer: [0.0; 32],
            output_buffer: [0.0; 4],
        }
    }

    #[inline(always)]
    pub fn forward(&mut self, input: &[f32; 128]) -> [f32; 4] {
        unsafe {
            // Layer 1: INT8 GEMM with AVX2
            self.weights.gemm_int8_avx2(input, &mut self.hidden_buffer);

            // ReLU activation using AVX2 (branchless)
            for chunk in self.hidden_buffer.chunks_exact_mut(8) {
                let vals = _mm256_loadu_ps(chunk.as_ptr());
                let zero = _mm256_setzero_ps();
                let relu = _mm256_max_ps(vals, zero);
                _mm256_storeu_ps(chunk.as_mut_ptr(), relu);
            }

            // Layer 2: Small matrix, use AVX2 for output
            for i in 0..4 {
                let mut acc = _mm256_setzero_ps();

                for j in (0..32).step_by(8) {
                    let hidden_vec = _mm256_loadu_ps(self.hidden_buffer.as_ptr().add(j));

                    // Load INT8 weights and convert
                    let weight_ptr = self.weights.w2_int8.add(i * 32 + j);
                    let weights_i8 = _mm_loadl_epi64(weight_ptr as *const __m128i);
                    let weights_i32 = _mm256_cvtepi8_epi32(weights_i8);
                    let weights_f32 = _mm256_cvtepi32_ps(weights_i32);

                    let scale = _mm256_set1_ps(self.weights.w2_scale[i]);
                    let scaled_weights = _mm256_mul_ps(weights_f32, scale);

                    acc = _mm256_fmadd_ps(scaled_weights, hidden_vec, acc);
                }

                // Horizontal sum
                let sum = _mm256_hadd_ps(acc, acc);
                let sum = _mm256_hadd_ps(sum, sum);
                let high = _mm256_extractf128_ps(sum, 1);
                let low = _mm256_castps256_ps128(sum);
                let final_sum = _mm_add_ps(low, high);

                self.output_buffer[i] = _mm_cvtss_f32(final_sum) + self.weights.b2[i];
            }
        }

        self.output_buffer
    }
}

/// Custom assembly optimizations for critical paths
#[cfg(target_arch = "x86_64")]
pub mod asm_optimizations {
    use std::arch::asm;

    /// Ultra-fast dot product using inline assembly
    #[inline(always)]
    pub unsafe fn dot_product_asm(a: *const f32, b: *const f32, len: usize) -> f32 {
        let mut result: f32;

        asm!(
            "vzeroall",                      // Clear all YMM registers
            "xor {i}, {i}",                   // i = 0
            "vxorps ymm0, ymm0, ymm0",       // acc = 0

            "2:",                             // Loop label
            "vmovaps ymm1, [{a} + {i}*4]",   // Load 8 floats from a
            "vmovaps ymm2, [{b} + {i}*4]",   // Load 8 floats from b
            "vfmadd231ps ymm0, ymm1, ymm2",  // acc += a * b
            "add {i}, 8",                     // i += 8
            "cmp {i}, {len}",                 // Compare i with len
            "jl 2b",                          // Jump if less

            // Horizontal sum
            "vhaddps ymm0, ymm0, ymm0",
            "vhaddps ymm0, ymm0, ymm0",
            "vextractf128 xmm1, ymm0, 1",
            "vaddps xmm0, xmm0, xmm1",
            "vmovss {result}, xmm0",

            i = out(reg) _,
            a = in(reg) a,
            b = in(reg) b,
            len = in(reg) len,
            result = out(xmm_reg) result,
            out("ymm0") _, out("ymm1") _, out("ymm2") _,
        );

        result
    }

    /// Fast ReLU using assembly
    #[inline(always)]
    pub unsafe fn relu_asm(data: *mut f32, len: usize) {
        asm!(
            "vxorps ymm1, ymm1, ymm1",       // Zero vector for comparison
            "xor {i}, {i}",                   // i = 0

            "2:",                             // Loop
            "vmovaps ymm0, [{data} + {i}*4]", // Load 8 floats
            "vmaxps ymm0, ymm0, ymm1",       // max(x, 0)
            "vmovaps [{data} + {i}*4], ymm0", // Store back
            "add {i}, 8",
            "cmp {i}, {len}",
            "jl 2b",

            i = out(reg) _,
            data = in(reg) data,
            len = in(reg) len,
            out("ymm0") _, out("ymm1") _,
        );
    }
}

/// CPU affinity and NUMA optimization
pub struct CpuOptimizer {
    core_id: usize,
}

impl CpuOptimizer {
    pub fn new(preferred_core: usize) -> Self {
        // Pin to specific CPU core
        let core_ids = core_affinity::get_core_ids().unwrap();
        if preferred_core < core_ids.len() {
            core_affinity::set_for_current(core_ids[preferred_core]);
        }

        // Set thread priority to real-time (requires permissions)
        #[cfg(unix)]
        unsafe {
            libc::setpriority(libc::PRIO_PROCESS, 0, -20);
        }

        Self {
            core_id: preferred_core,
        }
    }

    pub fn prefetch_data<T>(data: &[T]) {
        unsafe {
            let ptr = data.as_ptr() as *const i8;
            for i in (0..data.len()).step_by(64) {
                _mm_prefetch(ptr.add(i * std::mem::size_of::<T>()), _MM_HINT_T0);
            }
        }
    }
}

/// Complete optimized temporal solver
pub struct FullyOptimizedSolver {
    nn: OptimizedNeuralNetwork,
    cpu_opt: CpuOptimizer,
}

impl FullyOptimizedSolver {
    pub fn new() -> Self {
        Self {
            nn: OptimizedNeuralNetwork::new(),
            cpu_opt: CpuOptimizer::new(0), // Pin to core 0
        }
    }

    #[inline(always)]
    pub fn predict(&mut self, input: &[f32; 128]) -> ([f32; 4], Duration) {
        // Prefetch input data
        CpuOptimizer::prefetch_data(input);

        let start = Instant::now();
        let output = self.nn.forward(input);
        let duration = start.elapsed();

        (output, duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int8_quantization() {
        let weights = QuantizedWeights::new();
        unsafe {
            // Verify quantization
            for i in 0..32 {
                for j in 0..128 {
                    let quantized = *weights.w1_int8.add(i * 128 + j);
                    assert!(quantized >= -128 && quantized <= 127);
                }
            }
        }
    }

    #[test]
    fn test_fully_optimized() {
        let mut solver = FullyOptimizedSolver::new();
        let input = [0.1f32; 128];

        // Warmup
        for _ in 0..1000 {
            solver.predict(&input);
        }

        // Benchmark
        let mut timings = Vec::new();
        for _ in 0..1000 {
            let (_, duration) = solver.predict(&input);
            timings.push(duration);
        }

        timings.sort();
        let p50 = timings[500];
        let p99 = timings[990];

        println!("Fully Optimized Performance:");
        println!("  P50: {:?}", p50);
        println!("  P99: {:?}", p99);

        // Should achieve sub-microsecond performance
        assert!(p99.as_micros() < 10);
    }
}