//! Web Workers for Parallel Inference in WASM
//!
//! This module provides multi-threaded execution in browsers using Web Workers
//! with SharedArrayBuffer for zero-copy data sharing.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      Main Thread                                 │
//! │  ┌──────────────────┐  ┌──────────────────┐                     │
//! │  │ ParallelInference│  │ SharedBufferMgr  │                     │
//! │  └────────┬─────────┘  └────────┬─────────┘                     │
//! │           │                     │                               │
//! │           ▼                     ▼                               │
//! │  ┌────────────────────────────────────────┐                     │
//! │  │            WorkerPool                   │                     │
//! │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐│                     │
//! │  │  │TaskQueue │ │SharedMem │ │ Workers  ││                     │
//! │  │  └──────────┘ └──────────┘ └──────────┘│                     │
//! │  └────────────────────────────────────────┘                     │
//! └─────────────────────────────────────────────────────────────────┘
//!                        │ postMessage │
//!                        ▼             ▼
//! ┌────────────────┐ ┌────────────────┐ ┌────────────────┐
//! │   Worker 0     │ │   Worker 1     │ │   Worker N     │
//! │ ┌────────────┐ │ │ ┌────────────┐ │ │ ┌────────────┐ │
//! │ │SharedArray │ │ │ │SharedArray │ │ │ │SharedArray │ │
//! │ │  Buffer    │ │ │ │  Buffer    │ │ │ │  Buffer    │ │
//! │ │   View     │ │ │ │   View     │ │ │ │   View     │ │
//! │ └────────────┘ │ │ └────────────┘ │ │ └────────────┘ │
//! └────────────────┘ └────────────────┘ └────────────────┘
//! ```
//!
//! # Features
//!
//! - **SharedArrayBuffer**: Zero-copy memory sharing between threads
//! - **Atomics**: Thread synchronization primitives
//! - **Dynamic Worker Count**: Based on `navigator.hardwareConcurrency`
//! - **Graceful Fallback**: Single-threaded mode when SharedArrayBuffer unavailable
//!
//! # Example
//!
//! ```javascript
//! import { ParallelInference } from 'ruvllm-wasm';
//!
//! // Create parallel inference engine
//! const engine = await ParallelInference.new(4); // 4 workers
//!
//! // Check capabilities
//! console.log('Workers:', engine.workerCount());
//! console.log('Shared memory:', engine.isSharedMemoryAvailable());
//!
//! // Parallel matrix multiplication
//! const result = await engine.matmul(a, b, m, n, k);
//! ```
//!
//! # Browser Requirements
//!
//! For SharedArrayBuffer to work, the page must be served with:
//! - `Cross-Origin-Opener-Policy: same-origin`
//! - `Cross-Origin-Embedder-Policy: require-corp`

pub mod feature_detect;
pub mod messages;
pub mod pool;
pub mod shared;

pub use feature_detect::*;
pub use messages::*;
pub use pool::*;
pub use shared::*;

use wasm_bindgen::prelude::*;

/// Maximum recommended workers (prevent resource exhaustion)
pub const MAX_WORKERS: usize = 16;

/// Default minimum workers
pub const MIN_WORKERS: usize = 2;

/// WASM page size in bytes (64KB)
pub const WASM_PAGE_SIZE: usize = 65536;

/// Alignment for SIMD operations (16 bytes for 128-bit SIMD)
pub const SIMD_ALIGNMENT: usize = 16;

/// Main parallel inference interface for WASM.
///
/// Provides high-level API for parallel compute operations in the browser.
/// Automatically manages worker pool and shared memory.
#[wasm_bindgen]
pub struct ParallelInference {
    pool: WorkerPool,
    shared_buffers: SharedBufferManager,
    initialized: bool,
}

#[wasm_bindgen]
impl ParallelInference {
    /// Create a new ParallelInference instance.
    ///
    /// # Arguments
    /// * `num_workers` - Number of workers to spawn. If None, uses optimal count.
    ///
    /// # Returns
    /// A Promise that resolves to ParallelInference instance.
    ///
    /// # Example (JavaScript)
    /// ```javascript
    /// const inference = await ParallelInference.new(4);
    /// ```
    #[wasm_bindgen(constructor)]
    pub async fn new(num_workers: Option<usize>) -> Result<ParallelInference, JsValue> {
        crate::utils::set_panic_hook();

        let worker_count = num_workers.unwrap_or_else(optimal_worker_count);
        let worker_count = worker_count.clamp(MIN_WORKERS, MAX_WORKERS);

        crate::utils::log(&format!(
            "Initializing ParallelInference with {} workers",
            worker_count
        ));

        // Check for SharedArrayBuffer support
        let shared_memory_available = is_shared_array_buffer_available();
        if !shared_memory_available {
            crate::utils::warn(
                "SharedArrayBuffer not available. Using fallback mode with message passing.",
            );
        }

        // Check cross-origin isolation
        if shared_memory_available && !cross_origin_isolated() {
            crate::utils::warn(
                "Page is not cross-origin isolated. SharedArrayBuffer may not work correctly.",
            );
        }

        let pool = WorkerPool::new(worker_count).await?;
        let shared_buffers = SharedBufferManager::new();

        crate::utils::log("ParallelInference initialized successfully");

        Ok(ParallelInference {
            pool,
            shared_buffers,
            initialized: true,
        })
    }

    /// Perform parallel matrix multiplication.
    ///
    /// Computes C = A * B where:
    /// - A is m x k
    /// - B is k x n
    /// - C is m x n
    ///
    /// # Arguments
    /// * `a` - Matrix A as flat array (row-major)
    /// * `b` - Matrix B as flat array (row-major)
    /// * `m` - Number of rows in A
    /// * `n` - Number of columns in B
    /// * `k` - Number of columns in A / rows in B
    ///
    /// # Returns
    /// Result matrix C as Float32Array
    #[wasm_bindgen]
    pub async fn matmul(
        &mut self,
        a: &[f32],
        b: &[f32],
        m: usize,
        n: usize,
        k: usize,
    ) -> Result<Vec<f32>, JsValue> {
        if !self.initialized {
            return Err(JsValue::from_str("ParallelInference not initialized"));
        }

        // Validate dimensions
        if a.len() != m * k {
            return Err(JsValue::from_str(&format!(
                "Matrix A size mismatch: expected {} ({}x{}), got {}",
                m * k,
                m,
                k,
                a.len()
            )));
        }
        if b.len() != k * n {
            return Err(JsValue::from_str(&format!(
                "Matrix B size mismatch: expected {} ({}x{}), got {}",
                k * n,
                k,
                n,
                b.len()
            )));
        }

        // For small matrices, compute directly on main thread
        if m * n * k < 10000 {
            return Ok(self.matmul_single_thread(a, b, m, n, k));
        }

        // Use parallel computation
        self.pool.parallel_matmul(a, b, m, n, k).await
    }

    /// Perform parallel multi-head attention.
    ///
    /// Computes softmax(Q * K^T / sqrt(d_k)) * V for each attention head.
    ///
    /// # Arguments
    /// * `q` - Query tensor (batch_size, num_heads, seq_len, head_dim)
    /// * `k` - Key tensor (batch_size, num_heads, seq_len, head_dim)
    /// * `v` - Value tensor (batch_size, num_heads, seq_len, head_dim)
    /// * `num_heads` - Number of attention heads
    /// * `head_dim` - Dimension of each head
    /// * `seq_len` - Sequence length
    ///
    /// # Returns
    /// Output tensor (batch_size, num_heads, seq_len, head_dim)
    #[wasm_bindgen(js_name = attention)]
    pub async fn parallel_attention(
        &mut self,
        q: &[f32],
        k: &[f32],
        v: &[f32],
        num_heads: usize,
        head_dim: usize,
        seq_len: usize,
    ) -> Result<Vec<f32>, JsValue> {
        if !self.initialized {
            return Err(JsValue::from_str("ParallelInference not initialized"));
        }

        // Validate dimensions
        let expected_size = num_heads * seq_len * head_dim;
        if q.len() != expected_size || k.len() != expected_size || v.len() != expected_size {
            return Err(JsValue::from_str(&format!(
                "Tensor size mismatch: expected {}, got Q={}, K={}, V={}",
                expected_size,
                q.len(),
                k.len(),
                v.len()
            )));
        }

        // For small tensors, compute on main thread
        if expected_size < 10000 {
            return Ok(self.attention_single_thread(q, k, v, num_heads, head_dim, seq_len));
        }

        self.pool
            .parallel_attention(q, k, v, num_heads, head_dim, seq_len)
            .await
    }

    /// Perform parallel layer normalization.
    ///
    /// # Arguments
    /// * `input` - Input tensor
    /// * `gamma` - Scale parameter
    /// * `beta` - Shift parameter
    /// * `epsilon` - Small constant for numerical stability
    ///
    /// # Returns
    /// Normalized tensor
    #[wasm_bindgen(js_name = layerNorm)]
    pub async fn layer_norm(
        &mut self,
        input: &[f32],
        gamma: &[f32],
        beta: &[f32],
        epsilon: f32,
    ) -> Result<Vec<f32>, JsValue> {
        if !self.initialized {
            return Err(JsValue::from_str("ParallelInference not initialized"));
        }

        if input.len() < 1000 {
            return Ok(self.layer_norm_single_thread(input, gamma, beta, epsilon));
        }

        self.pool.parallel_norm(input, gamma, beta, epsilon).await
    }

    /// Get the number of active workers.
    #[wasm_bindgen(js_name = workerCount)]
    pub fn worker_count(&self) -> usize {
        self.pool.worker_count()
    }

    /// Check if SharedArrayBuffer is available.
    #[wasm_bindgen(js_name = isSharedMemoryAvailable)]
    pub fn is_shared_memory_available(&self) -> bool {
        is_shared_array_buffer_available()
    }

    /// Check if the page is cross-origin isolated.
    #[wasm_bindgen(js_name = isCrossOriginIsolated)]
    pub fn is_cross_origin_isolated(&self) -> bool {
        cross_origin_isolated()
    }

    /// Check if Atomics API is available.
    #[wasm_bindgen(js_name = isAtomicsAvailable)]
    pub fn is_atomics_available(&self) -> bool {
        is_atomics_available()
    }

    /// Get optimal worker count for the current hardware.
    #[wasm_bindgen(js_name = optimalWorkerCount)]
    pub fn get_optimal_worker_count() -> usize {
        optimal_worker_count()
    }

    /// Terminate all workers and clean up resources.
    #[wasm_bindgen]
    pub fn terminate(&mut self) {
        self.pool.terminate();
        self.shared_buffers.clear();
        self.initialized = false;
        crate::utils::log("ParallelInference terminated");
    }

    /// Get statistics about worker pool.
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> Result<String, JsValue> {
        let stats = self.pool.stats();
        serde_json::to_string(&stats).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // Private helper methods for single-threaded fallback

    fn matmul_single_thread(&self, a: &[f32], b: &[f32], m: usize, n: usize, k: usize) -> Vec<f32> {
        let mut c = vec![0.0f32; m * n];

        for i in 0..m {
            for j in 0..n {
                let mut sum = 0.0f32;
                for l in 0..k {
                    sum += a[i * k + l] * b[l * n + j];
                }
                c[i * n + j] = sum;
            }
        }

        c
    }

    fn attention_single_thread(
        &self,
        q: &[f32],
        k: &[f32],
        v: &[f32],
        num_heads: usize,
        head_dim: usize,
        seq_len: usize,
    ) -> Vec<f32> {
        let mut output = vec![0.0f32; num_heads * seq_len * head_dim];
        let scale = 1.0 / (head_dim as f32).sqrt();

        for h in 0..num_heads {
            let head_offset = h * seq_len * head_dim;

            // Compute attention scores: Q * K^T
            let mut scores = vec![0.0f32; seq_len * seq_len];
            for i in 0..seq_len {
                for j in 0..seq_len {
                    let mut dot = 0.0f32;
                    for d in 0..head_dim {
                        dot +=
                            q[head_offset + i * head_dim + d] * k[head_offset + j * head_dim + d];
                    }
                    scores[i * seq_len + j] = dot * scale;
                }
            }

            // Softmax
            for i in 0..seq_len {
                let row_start = i * seq_len;
                let max_val = scores[row_start..row_start + seq_len]
                    .iter()
                    .fold(f32::NEG_INFINITY, |a, &b| a.max(b));

                let mut sum = 0.0f32;
                for j in 0..seq_len {
                    scores[row_start + j] = (scores[row_start + j] - max_val).exp();
                    sum += scores[row_start + j];
                }

                for j in 0..seq_len {
                    scores[row_start + j] /= sum;
                }
            }

            // Compute output: scores * V
            for i in 0..seq_len {
                for d in 0..head_dim {
                    let mut sum = 0.0f32;
                    for j in 0..seq_len {
                        sum += scores[i * seq_len + j] * v[head_offset + j * head_dim + d];
                    }
                    output[head_offset + i * head_dim + d] = sum;
                }
            }
        }

        output
    }

    fn layer_norm_single_thread(
        &self,
        input: &[f32],
        gamma: &[f32],
        beta: &[f32],
        epsilon: f32,
    ) -> Vec<f32> {
        let n = input.len();
        let hidden_dim = gamma.len();

        if n % hidden_dim != 0 {
            return input.to_vec(); // Fallback: return input unchanged
        }

        let batch_size = n / hidden_dim;
        let mut output = vec![0.0f32; n];

        for b in 0..batch_size {
            let start = b * hidden_dim;
            let end = start + hidden_dim;
            let slice = &input[start..end];

            // Compute mean
            let mean: f32 = slice.iter().sum::<f32>() / hidden_dim as f32;

            // Compute variance
            let variance: f32 =
                slice.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / hidden_dim as f32;

            // Normalize
            let std = (variance + epsilon).sqrt();
            for i in 0..hidden_dim {
                output[start + i] = ((input[start + i] - mean) / std) * gamma[i] + beta[i];
            }
        }

        output
    }
}

impl Drop for ParallelInference {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matmul_single_thread() {
        let inference = ParallelInference {
            pool: WorkerPool::empty(),
            shared_buffers: SharedBufferManager::new(),
            initialized: true,
        };

        // 2x3 * 3x2 = 2x2
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];

        let c = inference.matmul_single_thread(&a, &b, 2, 2, 3);

        // Expected: [[22, 28], [49, 64]]
        assert_eq!(c.len(), 4);
        assert!((c[0] - 22.0).abs() < 0.001);
        assert!((c[1] - 28.0).abs() < 0.001);
        assert!((c[2] - 49.0).abs() < 0.001);
        assert!((c[3] - 64.0).abs() < 0.001);
    }

    #[test]
    fn test_layer_norm_single_thread() {
        let inference = ParallelInference {
            pool: WorkerPool::empty(),
            shared_buffers: SharedBufferManager::new(),
            initialized: true,
        };

        let input = vec![1.0, 2.0, 3.0, 4.0];
        let gamma = vec![1.0, 1.0, 1.0, 1.0];
        let beta = vec![0.0, 0.0, 0.0, 0.0];
        let epsilon = 1e-5;

        let output = inference.layer_norm_single_thread(&input, &gamma, &beta, epsilon);

        // After normalization, mean should be ~0 and std ~1
        let mean: f32 = output.iter().sum::<f32>() / output.len() as f32;
        assert!(mean.abs() < 0.001);
    }
}
