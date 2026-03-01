//! GPU Acceleration Module for RuVector ONNX Embeddings
//!
//! This module provides optional GPU acceleration using cuda-wasm for:
//! - Pooling operations
//! - Similarity computations
//! - Batch vector operations
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    GPU Acceleration Layer                        │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐ │
//! │  │  GpuBackend │ -> │  Shaders    │ -> │  WebGPU Runtime     │ │
//! │  │  (Trait)    │    │  (WGSL)     │    │  (wgpu)             │ │
//! │  └─────────────┘    └─────────────┘    └─────────────────────┘ │
//! │         │                                       │              │
//! │         v                                       v              │
//! │  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐ │
//! │  │ GpuPooler   │    │ GpuSimilar  │    │ GpuVectorOps        │ │
//! │  │             │    │             │    │                     │ │
//! │  └─────────────┘    └─────────────┘    └─────────────────────┘ │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Feature Flags
//!
//! - `gpu`: Enable GPU acceleration (WebGPU backend)
//! - `cuda-wasm`: Enable CUDA-WASM transpilation support
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruvector_onnx_embeddings::gpu::{GpuAccelerator, GpuConfig};
//!
//! // Create GPU accelerator with auto-detection
//! let gpu = GpuAccelerator::new(GpuConfig::auto()).await?;
//!
//! // GPU-accelerated similarity search
//! let similarities = gpu.batch_cosine_similarity(&query, &candidates)?;
//!
//! // GPU-accelerated pooling
//! let pooled = gpu.mean_pool(&token_embeddings, &attention_mask)?;
//! ```

mod backend;
mod config;
mod operations;
mod shaders;

#[cfg(test)]
mod tests;

pub use backend::{GpuBackend, GpuDevice, GpuInfo};
pub use config::{GpuConfig, GpuMode, PowerPreference};
pub use operations::{
    GpuPooler, GpuSimilarity, GpuVectorOps,
    batch_cosine_similarity_gpu, batch_dot_product_gpu, batch_euclidean_gpu,
};
pub use shaders::ShaderRegistry;

use crate::Result;
use std::sync::Arc;

/// GPU Accelerator - Main entry point for GPU operations
///
/// Provides unified access to GPU-accelerated operations with automatic
/// fallback to CPU when GPU is unavailable.
pub struct GpuAccelerator {
    backend: Arc<dyn GpuBackend>,
    config: GpuConfig,
    pooler: GpuPooler,
    similarity: GpuSimilarity,
    vector_ops: GpuVectorOps,
}

impl GpuAccelerator {
    /// Create a new GPU accelerator with the given configuration
    pub async fn new(config: GpuConfig) -> Result<Self> {
        let backend: Arc<dyn GpuBackend> = Arc::from(backend::create_backend(&config).await?);
        let shader_registry = ShaderRegistry::new();

        let mut pooler = GpuPooler::new(backend.as_ref(), &shader_registry)?;
        let mut similarity = GpuSimilarity::new(backend.as_ref(), &shader_registry)?;
        let mut vector_ops = GpuVectorOps::new(backend.as_ref(), &shader_registry)?;

        // Wire up the backend to all components for GPU dispatch
        #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
        {
            pooler.set_backend(Arc::clone(&backend));
            similarity.set_backend(Arc::clone(&backend));
            vector_ops.set_backend(Arc::clone(&backend));
        }

        Ok(Self {
            backend,
            config,
            pooler,
            similarity,
            vector_ops,
        })
    }

    /// Create with automatic configuration
    pub async fn auto() -> Result<Self> {
        Self::new(GpuConfig::auto()).await
    }

    /// Check if GPU acceleration is available
    pub fn is_available(&self) -> bool {
        self.backend.is_available()
    }

    /// Get GPU device information
    pub fn device_info(&self) -> GpuInfo {
        self.backend.device_info()
    }

    /// Get the current configuration
    pub fn config(&self) -> &GpuConfig {
        &self.config
    }

    // ==================== Pooling Operations ====================

    /// Mean pooling over token embeddings (GPU-accelerated)
    pub fn mean_pool(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        batch_size: usize,
        seq_length: usize,
        hidden_size: usize,
    ) -> Result<Vec<f32>> {
        self.pooler.mean_pool(
            token_embeddings,
            attention_mask,
            batch_size,
            seq_length,
            hidden_size,
        )
    }

    /// CLS token pooling (GPU-accelerated)
    pub fn cls_pool(
        &self,
        token_embeddings: &[f32],
        batch_size: usize,
        hidden_size: usize,
    ) -> Result<Vec<f32>> {
        self.pooler.cls_pool(token_embeddings, batch_size, hidden_size)
    }

    /// Max pooling over token embeddings (GPU-accelerated)
    pub fn max_pool(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        batch_size: usize,
        seq_length: usize,
        hidden_size: usize,
    ) -> Result<Vec<f32>> {
        self.pooler.max_pool(
            token_embeddings,
            attention_mask,
            batch_size,
            seq_length,
            hidden_size,
        )
    }

    // ==================== Similarity Operations ====================

    /// Batch cosine similarity (GPU-accelerated)
    pub fn batch_cosine_similarity(
        &self,
        query: &[f32],
        candidates: &[&[f32]],
    ) -> Result<Vec<f32>> {
        self.similarity.batch_cosine(query, candidates)
    }

    /// Batch dot product (GPU-accelerated)
    pub fn batch_dot_product(
        &self,
        query: &[f32],
        candidates: &[&[f32]],
    ) -> Result<Vec<f32>> {
        self.similarity.batch_dot_product(query, candidates)
    }

    /// Batch Euclidean distance (GPU-accelerated)
    pub fn batch_euclidean_distance(
        &self,
        query: &[f32],
        candidates: &[&[f32]],
    ) -> Result<Vec<f32>> {
        self.similarity.batch_euclidean(query, candidates)
    }

    /// Find top-k most similar vectors (GPU-accelerated)
    pub fn top_k_similar(
        &self,
        query: &[f32],
        candidates: &[&[f32]],
        k: usize,
    ) -> Result<Vec<(usize, f32)>> {
        self.similarity.top_k(query, candidates, k)
    }

    // ==================== Vector Operations ====================

    /// L2 normalize vectors (GPU-accelerated)
    pub fn normalize_batch(&self, vectors: &mut [f32], dimension: usize) -> Result<()> {
        self.vector_ops.normalize_batch(vectors, dimension)
    }

    /// Matrix-vector multiplication (GPU-accelerated)
    pub fn matmul(
        &self,
        matrix: &[f32],
        vector: &[f32],
        rows: usize,
        cols: usize,
    ) -> Result<Vec<f32>> {
        self.vector_ops.matmul(matrix, vector, rows, cols)
    }

    /// Batch vector addition (GPU-accelerated)
    pub fn batch_add(&self, a: &[f32], b: &[f32]) -> Result<Vec<f32>> {
        self.vector_ops.batch_add(a, b)
    }

    /// Batch vector scaling (GPU-accelerated)
    pub fn batch_scale(&self, vectors: &mut [f32], scale: f32) -> Result<()> {
        self.vector_ops.batch_scale(vectors, scale)
    }
}

/// Convenience function to check GPU availability without creating accelerator
pub async fn is_gpu_available() -> bool {
    backend::probe_gpu().await
}

/// Get GPU device info without full initialization
pub async fn get_gpu_info() -> Option<GpuInfo> {
    backend::get_device_info().await
}

/// Fallback wrapper that tries GPU first, then CPU
pub struct HybridAccelerator {
    gpu: Option<GpuAccelerator>,
    use_gpu: bool,
}

impl HybridAccelerator {
    /// Create hybrid accelerator with GPU if available
    pub async fn new() -> Self {
        let gpu = GpuAccelerator::auto().await.ok();
        let use_gpu = gpu.is_some();
        Self { gpu, use_gpu }
    }

    /// Check if GPU is being used
    pub fn using_gpu(&self) -> bool {
        self.use_gpu && self.gpu.is_some()
    }

    /// Disable GPU (use CPU only)
    pub fn disable_gpu(&mut self) {
        self.use_gpu = false;
    }

    /// Enable GPU if available
    pub fn enable_gpu(&mut self) {
        self.use_gpu = self.gpu.is_some();
    }

    /// Batch cosine similarity with automatic backend selection
    pub fn batch_cosine_similarity(
        &self,
        query: &[f32],
        candidates: &[Vec<f32>],
    ) -> Vec<f32> {
        if self.use_gpu {
            if let Some(ref gpu) = self.gpu {
                let refs: Vec<&[f32]> = candidates.iter().map(|v| v.as_slice()).collect();
                if let Ok(result) = gpu.batch_cosine_similarity(query, &refs) {
                    return result;
                }
            }
        }

        // CPU fallback
        crate::pooling::batch_cosine_similarity(query, candidates)
    }
}
