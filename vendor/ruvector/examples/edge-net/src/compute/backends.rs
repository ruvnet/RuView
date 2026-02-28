//! Compute backend implementations
//!
//! Provides trait implementations for different compute backends:
//! - WebGPU (primary, fastest)
//! - WebGL2 (fallback for older browsers)
//! - WebWorker (parallel CPU)
//! - SIMD (WASM SIMD intrinsics)
//! - Naive (pure Rust fallback)

use super::tensor::{DType, LoraAdapter, Shape, Tensor, WorkloadType};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// Backend type identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BackendType {
    /// WebGPU compute shaders (fastest)
    WebGpu,
    /// WebGL2 with compute emulation via fragment shaders
    WebGl2,
    /// Web Workers for parallel CPU
    WebWorker,
    /// WASM SIMD intrinsics
    Simd,
    /// Pure Rust naive implementation (always available)
    Naive,
}

impl BackendType {
    /// Get relative speed factor (1.0 = naive baseline)
    pub fn speed_factor(&self) -> f32 {
        match self {
            BackendType::WebGpu => 100.0,  // GPU is ~100x faster for large matmuls
            BackendType::WebGl2 => 50.0,   // WebGL2 is ~50x
            BackendType::WebWorker => 4.0, // 4 workers = 4x parallelism
            BackendType::Simd => 4.0,      // SIMD = 4x vectorization
            BackendType::Naive => 1.0,     // Baseline
        }
    }

    /// Get priority for fallback chain
    pub fn priority(&self) -> u8 {
        match self {
            BackendType::WebGpu => 5,
            BackendType::WebGl2 => 4,
            BackendType::WebWorker => 3,
            BackendType::Simd => 2,
            BackendType::Naive => 1,
        }
    }
}

/// Backend capability information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackendInfo {
    /// Backend type
    pub backend_type: BackendType,
    /// Whether this backend is available
    pub available: bool,
    /// Maximum tensor size in bytes
    pub max_tensor_size: usize,
    /// Maximum concurrent operations
    pub max_concurrent: usize,
    /// Supported data types
    pub supported_dtypes: Vec<DType>,
    /// Estimated throughput in GFLOPS
    pub estimated_gflops: f32,
}

/// Core compute operations trait - all backends must implement this
pub trait ComputeOps {
    /// Matrix multiplication: C = A @ B
    fn matmul(&self, a: &Tensor, b: &Tensor) -> Tensor;

    /// Scaled dot-product attention
    fn attention(&self, q: &Tensor, k: &Tensor, v: &Tensor) -> Tensor;

    /// LoRA forward pass: out = x + scaling * (B @ (A @ x))
    fn lora_forward(&self, x: &Tensor, adapter: &LoraAdapter) -> Tensor;

    /// Batch inference for multiple inputs
    fn batch_inference(&self, inputs: &[Tensor]) -> Vec<Tensor>;

    /// Element-wise ReLU
    fn relu(&self, x: &Tensor) -> Tensor;

    /// Element-wise GELU (Gaussian Error Linear Unit)
    fn gelu(&self, x: &Tensor) -> Tensor;

    /// Softmax along last dimension
    fn softmax(&self, x: &Tensor) -> Tensor;

    /// Layer normalization
    fn layer_norm(&self, x: &Tensor, weight: &Tensor, bias: &Tensor, eps: f32) -> Tensor;

    /// Get backend info
    fn info(&self) -> BackendInfo;

    /// Synchronize all pending operations
    fn sync(&self);
}

// ============================================================================
// Naive Backend (Pure Rust - Always Available)
// ============================================================================

/// Naive compute backend - pure Rust implementation
#[derive(Clone)]
pub struct NaiveCompute {
    /// Maximum tensor size
    max_size: usize,
}

impl Default for NaiveCompute {
    fn default() -> Self {
        Self::new()
    }
}

impl NaiveCompute {
    pub fn new() -> Self {
        Self {
            max_size: 256 * 1024 * 1024, // 256MB
        }
    }
}

impl ComputeOps for NaiveCompute {
    fn matmul(&self, a: &Tensor, b: &Tensor) -> Tensor {
        let a_shape = a.shape();
        let b_shape = b.shape();

        assert!(
            a_shape.matmul_compatible(b_shape),
            "Incompatible shapes for matmul: {} @ {}",
            a_shape,
            b_shape
        );

        let m = a_shape.dim(a_shape.ndim() - 2.max(1) + 1 - 1);
        let k = a_shape.dim(a_shape.ndim() - 1);
        let n = b_shape.dim(b_shape.ndim() - 1);

        // Handle different dimensionalities
        let (m, k, n) = if a_shape.ndim() == 1 && b_shape.ndim() == 1 {
            // Dot product
            (1, a_shape.dim(0), 1)
        } else if a_shape.ndim() == 1 {
            // Vector @ Matrix
            (1, a_shape.dim(0), b_shape.dim(1))
        } else if b_shape.ndim() == 1 {
            // Matrix @ Vector
            (a_shape.dim(0), a_shape.dim(1), 1)
        } else {
            // Matrix @ Matrix
            (a_shape.dim(0), a_shape.dim(1), b_shape.dim(1))
        };

        let a_data = a.to_vec();
        let b_data = b.to_vec();
        let mut c_data = vec![0.0f32; m * n];

        // Standard matrix multiplication O(m*n*k)
        for i in 0..m {
            for j in 0..n {
                let mut sum = 0.0f32;
                for l in 0..k {
                    sum += a_data[i * k + l] * b_data[l * n + j];
                }
                c_data[i * n + j] = sum;
            }
        }

        if m == 1 && n == 1 {
            Tensor::from_vec(c_data, Shape::d1(1))
        } else if m == 1 {
            Tensor::from_vec(c_data, Shape::d1(n))
        } else if n == 1 {
            Tensor::from_vec(c_data, Shape::d1(m))
        } else {
            Tensor::from_vec(c_data, Shape::d2(m, n))
        }
    }

    fn attention(&self, q: &Tensor, k: &Tensor, v: &Tensor) -> Tensor {
        // Scaled dot-product attention: softmax(Q @ K^T / sqrt(d_k)) @ V
        let d_k = q.shape().dim(q.shape().ndim() - 1) as f32;
        let scale = 1.0 / d_k.sqrt();

        // Q @ K^T
        let k_t = k.transpose();
        let scores = self.matmul(q, &k_t);

        // Scale
        let scores_data: Vec<f32> = scores.to_vec().iter().map(|&x| x * scale).collect();
        let scores_scaled = Tensor::from_vec(scores_data, scores.shape().clone());

        // Softmax
        let attn_weights = self.softmax(&scores_scaled);

        // @ V
        self.matmul(&attn_weights, v)
    }

    fn lora_forward(&self, x: &Tensor, adapter: &LoraAdapter) -> Tensor {
        // LoRA: out = x + scaling * (B @ (A @ x))
        let ax = self.matmul(&adapter.a.transpose(), x);
        let bax = self.matmul(&adapter.b.transpose(), &ax);

        // Add residual with scaling
        let x_data = x.to_vec();
        let bax_data = bax.to_vec();
        let out_data: Vec<f32> = x_data
            .iter()
            .zip(bax_data.iter())
            .map(|(&xi, &bi)| xi + adapter.scaling * bi)
            .collect();

        Tensor::from_vec(out_data, x.shape().clone())
    }

    fn batch_inference(&self, inputs: &[Tensor]) -> Vec<Tensor> {
        // For naive, just process sequentially
        inputs.iter().map(|x| self.relu(x)).collect()
    }

    fn relu(&self, x: &Tensor) -> Tensor {
        let data: Vec<f32> = x.to_vec().iter().map(|&v| v.max(0.0)).collect();
        Tensor::from_vec(data, x.shape().clone())
    }

    fn gelu(&self, x: &Tensor) -> Tensor {
        // GELU approximation: 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
        let sqrt_2_pi = (2.0 / std::f32::consts::PI).sqrt();
        let data: Vec<f32> = x
            .to_vec()
            .iter()
            .map(|&v| {
                let inner = sqrt_2_pi * (v + 0.044715 * v * v * v);
                0.5 * v * (1.0 + inner.tanh())
            })
            .collect();
        Tensor::from_vec(data, x.shape().clone())
    }

    fn softmax(&self, x: &Tensor) -> Tensor {
        let data = x.to_vec();
        let shape = x.shape();

        // Softmax along last dimension
        let last_dim = shape.dim(shape.ndim() - 1);
        let num_rows = data.len() / last_dim;

        let mut result = vec![0.0f32; data.len()];

        for row in 0..num_rows {
            let start = row * last_dim;
            let end = start + last_dim;
            let row_data = &data[start..end];

            // Numerical stability: subtract max
            let max_val = row_data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let exp_sum: f32 = row_data.iter().map(|&v| (v - max_val).exp()).sum();

            for (i, &v) in row_data.iter().enumerate() {
                result[start + i] = (v - max_val).exp() / exp_sum;
            }
        }

        Tensor::from_vec(result, shape.clone())
    }

    fn layer_norm(&self, x: &Tensor, weight: &Tensor, bias: &Tensor, eps: f32) -> Tensor {
        let data = x.to_vec();
        let w = weight.to_vec();
        let b = bias.to_vec();
        let shape = x.shape();

        let last_dim = shape.dim(shape.ndim() - 1);
        let num_rows = data.len() / last_dim;

        let mut result = vec![0.0f32; data.len()];

        for row in 0..num_rows {
            let start = row * last_dim;
            let end = start + last_dim;
            let row_data = &data[start..end];

            // Compute mean
            let mean: f32 = row_data.iter().sum::<f32>() / last_dim as f32;

            // Compute variance
            let variance: f32 =
                row_data.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / last_dim as f32;

            // Normalize
            let std = (variance + eps).sqrt();
            for (i, &v) in row_data.iter().enumerate() {
                let norm = (v - mean) / std;
                result[start + i] = norm * w[i % w.len()] + b[i % b.len()];
            }
        }

        Tensor::from_vec(result, shape.clone())
    }

    fn info(&self) -> BackendInfo {
        BackendInfo {
            backend_type: BackendType::Naive,
            available: true,
            max_tensor_size: self.max_size,
            max_concurrent: 1,
            supported_dtypes: vec![DType::F32, DType::I8],
            estimated_gflops: 0.5, // Rough estimate for single-threaded
        }
    }

    fn sync(&self) {
        // No-op for synchronous backend
    }
}

// ============================================================================
// SIMD Backend (WASM SIMD)
// ============================================================================

/// SIMD compute backend using WASM SIMD intrinsics
#[derive(Clone)]
pub struct SimdCompute {
    /// Fallback for non-SIMD operations
    fallback: NaiveCompute,
    /// Whether SIMD is available
    simd_available: bool,
}

impl Default for SimdCompute {
    fn default() -> Self {
        Self::new()
    }
}

impl SimdCompute {
    pub fn new() -> Self {
        // Check if SIMD is available at compile time
        #[cfg(target_feature = "simd128")]
        let simd_available = true;
        #[cfg(not(target_feature = "simd128"))]
        let simd_available = false;

        Self {
            fallback: NaiveCompute::new(),
            simd_available,
        }
    }

    /// SIMD dot product for f32x4
    #[cfg(target_feature = "simd128")]
    fn simd_dot_product(&self, a: &[f32], b: &[f32]) -> f32 {
        use std::arch::wasm32::*;

        assert_eq!(a.len(), b.len());
        let n = a.len();
        let chunks = n / 4;

        let mut sum = f32x4_splat(0.0);

        for i in 0..chunks {
            let offset = i * 4;
            unsafe {
                let va = v128_load(a.as_ptr().add(offset) as *const v128);
                let vb = v128_load(b.as_ptr().add(offset) as *const v128);
                sum = f32x4_add(sum, f32x4_mul(va, vb));
            }
        }

        // Horizontal sum
        let arr: [f32; 4] = unsafe { std::mem::transmute(sum) };
        let mut result = arr[0] + arr[1] + arr[2] + arr[3];

        // Handle remainder
        for i in (chunks * 4)..n {
            result += a[i] * b[i];
        }

        result
    }

    /// SIMD ReLU
    #[cfg(target_feature = "simd128")]
    fn simd_relu_inplace(&self, data: &mut [f32]) {
        use std::arch::wasm32::*;

        let zero = f32x4_splat(0.0);
        let chunks = data.len() / 4;

        for i in 0..chunks {
            let offset = i * 4;
            unsafe {
                let v = v128_load(data.as_ptr().add(offset) as *const v128);
                let result = f32x4_max(v, zero);
                v128_store(data.as_mut_ptr().add(offset) as *mut v128, result);
            }
        }

        // Handle remainder
        for i in (chunks * 4)..data.len() {
            data[i] = data[i].max(0.0);
        }
    }
}

impl ComputeOps for SimdCompute {
    fn matmul(&self, a: &Tensor, b: &Tensor) -> Tensor {
        #[cfg(target_feature = "simd128")]
        {
            let a_shape = a.shape();
            let b_shape = b.shape();

            if a_shape.ndim() == 2 && b_shape.ndim() == 2 && self.simd_available {
                let m = a_shape.dim(0);
                let k = a_shape.dim(1);
                let n = b_shape.dim(1);

                let a_data = a.to_vec();
                let b_data = b.to_vec();
                let mut c_data = vec![0.0f32; m * n];

                // Transpose B for better cache access
                let mut b_t = vec![0.0f32; k * n];
                for i in 0..k {
                    for j in 0..n {
                        b_t[j * k + i] = b_data[i * n + j];
                    }
                }

                // SIMD matmul
                for i in 0..m {
                    for j in 0..n {
                        let a_row = &a_data[i * k..(i + 1) * k];
                        let b_col = &b_t[j * k..(j + 1) * k];
                        c_data[i * n + j] = self.simd_dot_product(a_row, b_col);
                    }
                }

                return Tensor::from_vec(c_data, Shape::d2(m, n));
            }
        }

        // Fallback to naive
        self.fallback.matmul(a, b)
    }

    fn attention(&self, q: &Tensor, k: &Tensor, v: &Tensor) -> Tensor {
        // Use SIMD for the matmuls, fallback for softmax
        let d_k = q.shape().dim(q.shape().ndim() - 1) as f32;
        let scale = 1.0 / d_k.sqrt();

        let k_t = k.transpose();
        let scores = self.matmul(q, &k_t);

        let scores_data: Vec<f32> = scores.to_vec().iter().map(|&x| x * scale).collect();
        let scores_scaled = Tensor::from_vec(scores_data, scores.shape().clone());

        let attn_weights = self.fallback.softmax(&scores_scaled);
        self.matmul(&attn_weights, v)
    }

    fn lora_forward(&self, x: &Tensor, adapter: &LoraAdapter) -> Tensor {
        let ax = self.matmul(&adapter.a.transpose(), x);
        let bax = self.matmul(&adapter.b.transpose(), &ax);

        let x_data = x.to_vec();
        let bax_data = bax.to_vec();
        let out_data: Vec<f32> = x_data
            .iter()
            .zip(bax_data.iter())
            .map(|(&xi, &bi)| xi + adapter.scaling * bi)
            .collect();

        Tensor::from_vec(out_data, x.shape().clone())
    }

    fn batch_inference(&self, inputs: &[Tensor]) -> Vec<Tensor> {
        inputs.iter().map(|x| self.relu(x)).collect()
    }

    fn relu(&self, x: &Tensor) -> Tensor {
        #[cfg(target_feature = "simd128")]
        {
            if self.simd_available {
                let mut data = x.to_vec();
                self.simd_relu_inplace(&mut data);
                return Tensor::from_vec(data, x.shape().clone());
            }
        }
        self.fallback.relu(x)
    }

    fn gelu(&self, x: &Tensor) -> Tensor {
        // GELU is complex, use fallback
        self.fallback.gelu(x)
    }

    fn softmax(&self, x: &Tensor) -> Tensor {
        self.fallback.softmax(x)
    }

    fn layer_norm(&self, x: &Tensor, weight: &Tensor, bias: &Tensor, eps: f32) -> Tensor {
        self.fallback.layer_norm(x, weight, bias, eps)
    }

    fn info(&self) -> BackendInfo {
        BackendInfo {
            backend_type: BackendType::Simd,
            available: self.simd_available,
            max_tensor_size: 256 * 1024 * 1024,
            max_concurrent: 1,
            supported_dtypes: vec![DType::F32],
            estimated_gflops: 2.0, // ~4x naive
        }
    }

    fn sync(&self) {
        // No-op for synchronous backend
    }
}

// ============================================================================
// WebWorker Backend
// ============================================================================

/// WebWorker compute backend for parallel CPU execution
#[derive(Clone)]
pub struct WorkerPoolCompute {
    /// Number of workers
    num_workers: usize,
    /// Fallback for single operations
    fallback: SimdCompute,
    /// Whether workers are available
    workers_available: bool,
}

impl Default for WorkerPoolCompute {
    fn default() -> Self {
        Self::new(4)
    }
}

impl WorkerPoolCompute {
    pub fn new(num_workers: usize) -> Self {
        // In WASM, we'd check navigator.hardwareConcurrency
        // For now, assume workers are available
        Self {
            num_workers,
            fallback: SimdCompute::new(),
            workers_available: true, // Would be detected at runtime
        }
    }
}

impl ComputeOps for WorkerPoolCompute {
    fn matmul(&self, a: &Tensor, b: &Tensor) -> Tensor {
        // For single matmul, use SIMD (workers have overhead)
        self.fallback.matmul(a, b)
    }

    fn attention(&self, q: &Tensor, k: &Tensor, v: &Tensor) -> Tensor {
        self.fallback.attention(q, k, v)
    }

    fn lora_forward(&self, x: &Tensor, adapter: &LoraAdapter) -> Tensor {
        self.fallback.lora_forward(x, adapter)
    }

    fn batch_inference(&self, inputs: &[Tensor]) -> Vec<Tensor> {
        if !self.workers_available || inputs.len() < self.num_workers {
            return self.fallback.batch_inference(inputs);
        }

        // In real implementation, would dispatch to workers
        // For now, simulate parallel execution
        inputs.iter().map(|x| self.fallback.relu(x)).collect()
    }

    fn relu(&self, x: &Tensor) -> Tensor {
        self.fallback.relu(x)
    }

    fn gelu(&self, x: &Tensor) -> Tensor {
        self.fallback.gelu(x)
    }

    fn softmax(&self, x: &Tensor) -> Tensor {
        self.fallback.softmax(x)
    }

    fn layer_norm(&self, x: &Tensor, weight: &Tensor, bias: &Tensor, eps: f32) -> Tensor {
        self.fallback.layer_norm(x, weight, bias, eps)
    }

    fn info(&self) -> BackendInfo {
        BackendInfo {
            backend_type: BackendType::WebWorker,
            available: self.workers_available,
            max_tensor_size: 128 * 1024 * 1024, // Workers have memory limits
            max_concurrent: self.num_workers,
            supported_dtypes: vec![DType::F32],
            estimated_gflops: 2.0 * self.num_workers as f32,
        }
    }

    fn sync(&self) {
        // Would wait for all workers to complete
    }
}

// ============================================================================
// WebGL2 Compute Backend
// ============================================================================

/// WebGL2 compute backend (compute via fragment shaders)
#[derive(Clone)]
pub struct WebGl2Compute {
    /// Fallback for unsupported operations
    fallback: SimdCompute,
    /// Whether WebGL2 is available
    webgl2_available: bool,
    /// Maximum texture size
    max_texture_size: usize,
}

impl Default for WebGl2Compute {
    fn default() -> Self {
        Self::new()
    }
}

impl WebGl2Compute {
    pub fn new() -> Self {
        // In WASM, we'd check for WebGL2 context availability
        Self {
            fallback: SimdCompute::new(),
            webgl2_available: true, // Would be detected at runtime
            max_texture_size: 4096,
        }
    }

    /// Check if a tensor can fit in a texture
    fn fits_in_texture(&self, shape: &Shape) -> bool {
        if shape.ndim() < 2 {
            return shape.dim(0) <= self.max_texture_size;
        }
        shape.dim(0) <= self.max_texture_size && shape.dim(1) <= self.max_texture_size
    }
}

impl ComputeOps for WebGl2Compute {
    fn matmul(&self, a: &Tensor, b: &Tensor) -> Tensor {
        if !self.webgl2_available
            || !self.fits_in_texture(a.shape())
            || !self.fits_in_texture(b.shape())
        {
            return self.fallback.matmul(a, b);
        }

        // In real implementation, would:
        // 1. Upload A and B as textures
        // 2. Render fragment shader for matmul
        // 3. Read result from framebuffer
        // For now, use fallback
        self.fallback.matmul(a, b)
    }

    fn attention(&self, q: &Tensor, k: &Tensor, v: &Tensor) -> Tensor {
        // WebGL2 can accelerate attention via texture ops
        self.fallback.attention(q, k, v)
    }

    fn lora_forward(&self, x: &Tensor, adapter: &LoraAdapter) -> Tensor {
        self.fallback.lora_forward(x, adapter)
    }

    fn batch_inference(&self, inputs: &[Tensor]) -> Vec<Tensor> {
        self.fallback.batch_inference(inputs)
    }

    fn relu(&self, x: &Tensor) -> Tensor {
        // Simple element-wise ops are efficient in WebGL2
        self.fallback.relu(x)
    }

    fn gelu(&self, x: &Tensor) -> Tensor {
        self.fallback.gelu(x)
    }

    fn softmax(&self, x: &Tensor) -> Tensor {
        self.fallback.softmax(x)
    }

    fn layer_norm(&self, x: &Tensor, weight: &Tensor, bias: &Tensor, eps: f32) -> Tensor {
        self.fallback.layer_norm(x, weight, bias, eps)
    }

    fn info(&self) -> BackendInfo {
        BackendInfo {
            backend_type: BackendType::WebGl2,
            available: self.webgl2_available,
            max_tensor_size: self.max_texture_size * self.max_texture_size * 4, // RGBA float
            max_concurrent: 1,
            supported_dtypes: vec![DType::F32, DType::F16],
            estimated_gflops: 50.0, // GPU dependent
        }
    }

    fn sync(&self) {
        // Would call gl.finish()
    }
}

// ============================================================================
// WebGPU Compute Backend
// ============================================================================

/// WebGPU compute backend (fastest, uses compute shaders)
#[derive(Clone)]
pub struct WebGpuCompute {
    /// Fallback for when WebGPU is unavailable
    fallback: WebGl2Compute,
    /// Whether WebGPU is available
    webgpu_available: bool,
    /// Device limits
    max_buffer_size: usize,
    max_workgroup_size: usize,
}

impl Default for WebGpuCompute {
    fn default() -> Self {
        Self::new()
    }
}

impl WebGpuCompute {
    pub fn new() -> Self {
        // In WASM, we'd check navigator.gpu availability
        Self {
            fallback: WebGl2Compute::new(),
            webgpu_available: true, // Would be detected at runtime
            max_buffer_size: 256 * 1024 * 1024,
            max_workgroup_size: 256,
        }
    }

    /// Check if WebGPU should be used for this tensor size
    fn should_use_gpu(&self, numel: usize) -> bool {
        // GPU overhead isn't worth it for small tensors
        self.webgpu_available && numel > 1024
    }
}

impl ComputeOps for WebGpuCompute {
    fn matmul(&self, a: &Tensor, b: &Tensor) -> Tensor {
        let total_numel = a.numel() + b.numel();

        if !self.should_use_gpu(total_numel) {
            return self.fallback.matmul(a, b);
        }

        // In real implementation, would:
        // 1. Create GPU buffers for A, B, C
        // 2. Dispatch compute shader for matmul
        // 3. Read result buffer
        // For now, use fallback
        self.fallback.matmul(a, b)
    }

    fn attention(&self, q: &Tensor, k: &Tensor, v: &Tensor) -> Tensor {
        let total_numel = q.numel() + k.numel() + v.numel();

        if !self.should_use_gpu(total_numel) {
            return self.fallback.attention(q, k, v);
        }

        // Would use fused attention kernel
        self.fallback.attention(q, k, v)
    }

    fn lora_forward(&self, x: &Tensor, adapter: &LoraAdapter) -> Tensor {
        if !self.should_use_gpu(x.numel()) {
            return self.fallback.lora_forward(x, adapter);
        }

        // Would use fused LoRA kernel
        self.fallback.lora_forward(x, adapter)
    }

    fn batch_inference(&self, inputs: &[Tensor]) -> Vec<Tensor> {
        if inputs.is_empty() {
            return vec![];
        }

        let total_numel: usize = inputs.iter().map(|t| t.numel()).sum();

        if !self.should_use_gpu(total_numel) {
            return self.fallback.batch_inference(inputs);
        }

        // Would batch all inputs into single GPU dispatch
        self.fallback.batch_inference(inputs)
    }

    fn relu(&self, x: &Tensor) -> Tensor {
        if !self.should_use_gpu(x.numel()) {
            return self.fallback.relu(x);
        }
        self.fallback.relu(x)
    }

    fn gelu(&self, x: &Tensor) -> Tensor {
        if !self.should_use_gpu(x.numel()) {
            return self.fallback.gelu(x);
        }
        self.fallback.gelu(x)
    }

    fn softmax(&self, x: &Tensor) -> Tensor {
        if !self.should_use_gpu(x.numel()) {
            return self.fallback.softmax(x);
        }
        self.fallback.softmax(x)
    }

    fn layer_norm(&self, x: &Tensor, weight: &Tensor, bias: &Tensor, eps: f32) -> Tensor {
        if !self.should_use_gpu(x.numel()) {
            return self.fallback.layer_norm(x, weight, bias, eps);
        }
        self.fallback.layer_norm(x, weight, bias, eps)
    }

    fn info(&self) -> BackendInfo {
        BackendInfo {
            backend_type: BackendType::WebGpu,
            available: self.webgpu_available,
            max_tensor_size: self.max_buffer_size,
            max_concurrent: 8, // Multiple command encoders
            supported_dtypes: vec![DType::F32, DType::F16, DType::I8],
            estimated_gflops: 500.0, // GPU dependent
        }
    }

    fn sync(&self) {
        // Would wait for GPU queue to complete
    }
}

// ============================================================================
// Unified Compute Backend Enum
// ============================================================================

/// Unified compute backend - dispatches to available backends
#[derive(Clone)]
pub enum ComputeBackend {
    WebGpu(WebGpuCompute),
    WebGl2(WebGl2Compute),
    WebWorker(WorkerPoolCompute),
    Simd(SimdCompute),
    Naive(NaiveCompute),
}

impl ComputeBackend {
    /// Get backend type
    pub fn backend_type(&self) -> BackendType {
        match self {
            ComputeBackend::WebGpu(_) => BackendType::WebGpu,
            ComputeBackend::WebGl2(_) => BackendType::WebGl2,
            ComputeBackend::WebWorker(_) => BackendType::WebWorker,
            ComputeBackend::Simd(_) => BackendType::Simd,
            ComputeBackend::Naive(_) => BackendType::Naive,
        }
    }

    /// Check if backend is available
    pub fn is_available(&self) -> bool {
        self.info().available
    }
}

impl ComputeOps for ComputeBackend {
    fn matmul(&self, a: &Tensor, b: &Tensor) -> Tensor {
        match self {
            ComputeBackend::WebGpu(c) => c.matmul(a, b),
            ComputeBackend::WebGl2(c) => c.matmul(a, b),
            ComputeBackend::WebWorker(c) => c.matmul(a, b),
            ComputeBackend::Simd(c) => c.matmul(a, b),
            ComputeBackend::Naive(c) => c.matmul(a, b),
        }
    }

    fn attention(&self, q: &Tensor, k: &Tensor, v: &Tensor) -> Tensor {
        match self {
            ComputeBackend::WebGpu(c) => c.attention(q, k, v),
            ComputeBackend::WebGl2(c) => c.attention(q, k, v),
            ComputeBackend::WebWorker(c) => c.attention(q, k, v),
            ComputeBackend::Simd(c) => c.attention(q, k, v),
            ComputeBackend::Naive(c) => c.attention(q, k, v),
        }
    }

    fn lora_forward(&self, x: &Tensor, adapter: &LoraAdapter) -> Tensor {
        match self {
            ComputeBackend::WebGpu(c) => c.lora_forward(x, adapter),
            ComputeBackend::WebGl2(c) => c.lora_forward(x, adapter),
            ComputeBackend::WebWorker(c) => c.lora_forward(x, adapter),
            ComputeBackend::Simd(c) => c.lora_forward(x, adapter),
            ComputeBackend::Naive(c) => c.lora_forward(x, adapter),
        }
    }

    fn batch_inference(&self, inputs: &[Tensor]) -> Vec<Tensor> {
        match self {
            ComputeBackend::WebGpu(c) => c.batch_inference(inputs),
            ComputeBackend::WebGl2(c) => c.batch_inference(inputs),
            ComputeBackend::WebWorker(c) => c.batch_inference(inputs),
            ComputeBackend::Simd(c) => c.batch_inference(inputs),
            ComputeBackend::Naive(c) => c.batch_inference(inputs),
        }
    }

    fn relu(&self, x: &Tensor) -> Tensor {
        match self {
            ComputeBackend::WebGpu(c) => c.relu(x),
            ComputeBackend::WebGl2(c) => c.relu(x),
            ComputeBackend::WebWorker(c) => c.relu(x),
            ComputeBackend::Simd(c) => c.relu(x),
            ComputeBackend::Naive(c) => c.relu(x),
        }
    }

    fn gelu(&self, x: &Tensor) -> Tensor {
        match self {
            ComputeBackend::WebGpu(c) => c.gelu(x),
            ComputeBackend::WebGl2(c) => c.gelu(x),
            ComputeBackend::WebWorker(c) => c.gelu(x),
            ComputeBackend::Simd(c) => c.gelu(x),
            ComputeBackend::Naive(c) => c.gelu(x),
        }
    }

    fn softmax(&self, x: &Tensor) -> Tensor {
        match self {
            ComputeBackend::WebGpu(c) => c.softmax(x),
            ComputeBackend::WebGl2(c) => c.softmax(x),
            ComputeBackend::WebWorker(c) => c.softmax(x),
            ComputeBackend::Simd(c) => c.softmax(x),
            ComputeBackend::Naive(c) => c.softmax(x),
        }
    }

    fn layer_norm(&self, x: &Tensor, weight: &Tensor, bias: &Tensor, eps: f32) -> Tensor {
        match self {
            ComputeBackend::WebGpu(c) => c.layer_norm(x, weight, bias, eps),
            ComputeBackend::WebGl2(c) => c.layer_norm(x, weight, bias, eps),
            ComputeBackend::WebWorker(c) => c.layer_norm(x, weight, bias, eps),
            ComputeBackend::Simd(c) => c.layer_norm(x, weight, bias, eps),
            ComputeBackend::Naive(c) => c.layer_norm(x, weight, bias, eps),
        }
    }

    fn info(&self) -> BackendInfo {
        match self {
            ComputeBackend::WebGpu(c) => c.info(),
            ComputeBackend::WebGl2(c) => c.info(),
            ComputeBackend::WebWorker(c) => c.info(),
            ComputeBackend::Simd(c) => c.info(),
            ComputeBackend::Naive(c) => c.info(),
        }
    }

    fn sync(&self) {
        match self {
            ComputeBackend::WebGpu(c) => c.sync(),
            ComputeBackend::WebGl2(c) => c.sync(),
            ComputeBackend::WebWorker(c) => c.sync(),
            ComputeBackend::Simd(c) => c.sync(),
            ComputeBackend::Naive(c) => c.sync(),
        }
    }
}

/// Detect available backends and return them in priority order
pub fn detect_backends() -> Vec<ComputeBackend> {
    let mut backends = Vec::new();

    // Try each backend in priority order
    let webgpu = WebGpuCompute::new();
    if webgpu.info().available {
        backends.push(ComputeBackend::WebGpu(webgpu));
    }

    let webgl2 = WebGl2Compute::new();
    if webgl2.info().available {
        backends.push(ComputeBackend::WebGl2(webgl2));
    }

    let workers = WorkerPoolCompute::new(4);
    if workers.info().available {
        backends.push(ComputeBackend::WebWorker(workers));
    }

    let simd = SimdCompute::new();
    if simd.info().available {
        backends.push(ComputeBackend::Simd(simd));
    }

    // Naive is always available
    backends.push(ComputeBackend::Naive(NaiveCompute::new()));

    backends
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_naive_matmul() {
        let a = Tensor::from_slice(&[1.0, 2.0, 3.0, 4.0], Shape::d2(2, 2));
        let b = Tensor::from_slice(&[5.0, 6.0, 7.0, 8.0], Shape::d2(2, 2));

        let naive = NaiveCompute::new();
        let c = naive.matmul(&a, &b);

        let expected = vec![19.0, 22.0, 43.0, 50.0];
        assert_eq!(c.to_vec(), expected);
    }

    #[test]
    fn test_naive_relu() {
        let x = Tensor::from_slice(&[-1.0, 0.0, 1.0, 2.0], Shape::d1(4));
        let naive = NaiveCompute::new();
        let y = naive.relu(&x);

        assert_eq!(y.to_vec(), vec![0.0, 0.0, 1.0, 2.0]);
    }

    #[test]
    fn test_naive_softmax() {
        let x = Tensor::from_slice(&[1.0, 2.0, 3.0], Shape::d1(3));
        let naive = NaiveCompute::new();
        let y = naive.softmax(&x);

        let sum: f32 = y.to_vec().iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_backend_detection() {
        let backends = detect_backends();
        assert!(!backends.is_empty());
        // Naive should always be present
        assert!(backends
            .iter()
            .any(|b| b.backend_type() == BackendType::Naive));
    }

    #[test]
    fn test_compute_backend_dispatch() {
        let a = Tensor::from_slice(&[1.0, 2.0, 3.0, 4.0], Shape::d2(2, 2));
        let b = Tensor::from_slice(&[5.0, 6.0, 7.0, 8.0], Shape::d2(2, 2));

        let backend = ComputeBackend::Naive(NaiveCompute::new());
        let c = backend.matmul(&a, &b);

        let expected = vec![19.0, 22.0, 43.0, 50.0];
        assert_eq!(c.to_vec(), expected);
    }
}
