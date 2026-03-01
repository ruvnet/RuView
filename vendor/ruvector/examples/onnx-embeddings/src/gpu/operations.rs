//! GPU-Accelerated Operations
//!
//! High-level GPU operations for embeddings with automatic fallback to CPU.

use crate::{EmbeddingError, Result};
use super::backend::{GpuBackend, BufferUsage};
use super::shaders::ShaderRegistry;
use rayon::prelude::*;
use std::sync::Arc;

#[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
use bytemuck;

// ==================== GPU Pooler ====================

/// GPU-accelerated pooling operations
pub struct GpuPooler {
    use_gpu: bool,
    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    backend: Option<Arc<dyn GpuBackend>>,
}

impl GpuPooler {
    /// Create new GPU pooler
    pub fn new(backend: &dyn GpuBackend, _shaders: &ShaderRegistry) -> Result<Self> {
        let use_gpu = backend.is_available() && backend.device_info().supports_compute;

        Ok(Self {
            use_gpu,
            #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
            backend: None, // Will be set by GpuAccelerator
        })
    }

    /// Set the backend for GPU operations
    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    pub fn set_backend(&mut self, backend: Arc<dyn GpuBackend>) {
        self.backend = Some(backend);
    }

    /// Mean pooling (GPU or CPU fallback)
    pub fn mean_pool(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        batch_size: usize,
        seq_length: usize,
        hidden_size: usize,
    ) -> Result<Vec<f32>> {
        // GPU implementation requires minimum batch size for efficiency
        #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
        if self.use_gpu && batch_size >= 8 && self.backend.is_some() {
            return self.mean_pool_gpu(token_embeddings, attention_mask, batch_size, seq_length, hidden_size);
        }

        Ok(self.mean_pool_cpu(token_embeddings, attention_mask, batch_size, seq_length, hidden_size))
    }

    /// CLS pooling (GPU or CPU fallback)
    pub fn cls_pool(
        &self,
        token_embeddings: &[f32],
        batch_size: usize,
        hidden_size: usize,
    ) -> Result<Vec<f32>> {
        // CLS pooling is simple copy, CPU is often faster
        Ok(self.cls_pool_cpu(token_embeddings, batch_size, hidden_size))
    }

    /// Max pooling (GPU or CPU fallback)
    pub fn max_pool(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        batch_size: usize,
        seq_length: usize,
        hidden_size: usize,
    ) -> Result<Vec<f32>> {
        #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
        if self.use_gpu && batch_size >= 8 && self.backend.is_some() {
            return self.max_pool_gpu(token_embeddings, attention_mask, batch_size, seq_length, hidden_size);
        }

        Ok(self.max_pool_cpu(token_embeddings, attention_mask, batch_size, seq_length, hidden_size))
    }

    // GPU implementations

    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    fn mean_pool_gpu(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        batch_size: usize,
        seq_length: usize,
        hidden_size: usize,
    ) -> Result<Vec<f32>> {
        let backend = self.backend.as_ref().ok_or_else(|| {
            EmbeddingError::GpuOperationFailed {
                operation: "mean_pool".to_string(),
                reason: "Backend not initialized".to_string(),
            }
        })?;

        // Create buffers
        let token_buf = backend.create_buffer(
            (token_embeddings.len() * 4) as u64,
            BufferUsage::Storage,
        )?;
        let mask_buf = backend.create_buffer(
            (attention_mask.len() * 8) as u64,
            BufferUsage::Storage,
        )?;
        let output_buf = backend.create_buffer(
            (batch_size * hidden_size * 4) as u64,
            BufferUsage::Storage,
        )?;

        // Create params buffer (batch_size, seq_length, hidden_size)
        let params: [u32; 3] = [batch_size as u32, seq_length as u32, hidden_size as u32];
        let params_buf = backend.create_buffer(16, BufferUsage::Uniform)?; // 16 bytes aligned
        backend.write_buffer(&params_buf, bytemuck::cast_slice(&params))?;

        // Write input data
        backend.write_buffer(&token_buf, bytemuck::cast_slice(token_embeddings))?;
        backend.write_buffer(&mask_buf, bytemuck::cast_slice(attention_mask))?;

        // Create pipeline with mean pool shader
        let shader = super::shaders::MEAN_POOL_SHADER;
        let pipeline = backend.create_pipeline(shader, "mean_pool", [64, 1, 1])?;

        // Dispatch with params buffer as 4th binding
        let total_outputs = batch_size * hidden_size;
        let workgroups = [total_outputs.div_ceil(64) as u32, 1, 1];
        backend.dispatch(&pipeline, &[&token_buf, &mask_buf, &output_buf, &params_buf], workgroups)?;
        backend.sync()?;

        // Read output
        let output_bytes = backend.read_buffer(&output_buf, (batch_size * hidden_size * 4) as u64)?;
        let output: Vec<f32> = bytemuck::cast_slice(&output_bytes).to_vec();

        // Cleanup
        backend.release_buffer(token_buf)?;
        backend.release_buffer(mask_buf)?;
        backend.release_buffer(output_buf)?;
        backend.release_buffer(params_buf)?;
        backend.release_pipeline(pipeline)?;

        Ok(output)
    }

    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    fn max_pool_gpu(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        batch_size: usize,
        seq_length: usize,
        hidden_size: usize,
    ) -> Result<Vec<f32>> {
        let backend = self.backend.as_ref().ok_or_else(|| {
            EmbeddingError::GpuOperationFailed {
                operation: "max_pool".to_string(),
                reason: "Backend not initialized".to_string(),
            }
        })?;

        // Create buffers
        let token_buf = backend.create_buffer(
            (token_embeddings.len() * 4) as u64,
            BufferUsage::Storage,
        )?;
        let mask_buf = backend.create_buffer(
            (attention_mask.len() * 8) as u64,
            BufferUsage::Storage,
        )?;
        let output_buf = backend.create_buffer(
            (batch_size * hidden_size * 4) as u64,
            BufferUsage::Storage,
        )?;

        // Create params buffer (batch_size, seq_length, hidden_size)
        let params: [u32; 3] = [batch_size as u32, seq_length as u32, hidden_size as u32];
        let params_buf = backend.create_buffer(16, BufferUsage::Uniform)?;
        backend.write_buffer(&params_buf, bytemuck::cast_slice(&params))?;

        // Write input data
        backend.write_buffer(&token_buf, bytemuck::cast_slice(token_embeddings))?;
        backend.write_buffer(&mask_buf, bytemuck::cast_slice(attention_mask))?;

        // Create pipeline with max pool shader
        let shader = super::shaders::MAX_POOL_SHADER;
        let pipeline = backend.create_pipeline(shader, "max_pool", [64, 1, 1])?;

        // Dispatch with params buffer as 4th binding
        let total_outputs = batch_size * hidden_size;
        let workgroups = [total_outputs.div_ceil(64) as u32, 1, 1];
        backend.dispatch(&pipeline, &[&token_buf, &mask_buf, &output_buf, &params_buf], workgroups)?;
        backend.sync()?;

        // Read output
        let output_bytes = backend.read_buffer(&output_buf, (batch_size * hidden_size * 4) as u64)?;
        let output: Vec<f32> = bytemuck::cast_slice(&output_bytes).to_vec();

        // Cleanup
        backend.release_buffer(token_buf)?;
        backend.release_buffer(mask_buf)?;
        backend.release_buffer(output_buf)?;
        backend.release_buffer(params_buf)?;
        backend.release_pipeline(pipeline)?;

        Ok(output)
    }

    // CPU implementations

    fn mean_pool_cpu(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        batch_size: usize,
        seq_length: usize,
        hidden_size: usize,
    ) -> Vec<f32> {
        let mut output = vec![0.0f32; batch_size * hidden_size];

        output
            .par_chunks_mut(hidden_size)
            .enumerate()
            .for_each(|(batch_idx, out_chunk)| {
                let tokens_base = batch_idx * seq_length * hidden_size;
                let mask_base = batch_idx * seq_length;

                let mut count = 0.0f32;

                for seq_idx in 0..seq_length {
                    if attention_mask[mask_base + seq_idx] == 1 {
                        let start = tokens_base + seq_idx * hidden_size;
                        for (j, out_val) in out_chunk.iter_mut().enumerate() {
                            *out_val += token_embeddings[start + j];
                        }
                        count += 1.0;
                    }
                }

                if count > 0.0 {
                    for val in out_chunk.iter_mut() {
                        *val /= count;
                    }
                }
            });

        output
    }

    fn cls_pool_cpu(
        &self,
        token_embeddings: &[f32],
        batch_size: usize,
        hidden_size: usize,
    ) -> Vec<f32> {
        let seq_length = token_embeddings.len() / (batch_size * hidden_size);
        let mut output = vec![0.0f32; batch_size * hidden_size];

        for batch_idx in 0..batch_size {
            let src_start = batch_idx * seq_length * hidden_size;
            let dst_start = batch_idx * hidden_size;
            output[dst_start..dst_start + hidden_size]
                .copy_from_slice(&token_embeddings[src_start..src_start + hidden_size]);
        }

        output
    }

    fn max_pool_cpu(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        batch_size: usize,
        seq_length: usize,
        hidden_size: usize,
    ) -> Vec<f32> {
        let mut output = vec![f32::NEG_INFINITY; batch_size * hidden_size];

        output
            .par_chunks_mut(hidden_size)
            .enumerate()
            .for_each(|(batch_idx, out_chunk)| {
                let tokens_base = batch_idx * seq_length * hidden_size;
                let mask_base = batch_idx * seq_length;

                for seq_idx in 0..seq_length {
                    if attention_mask[mask_base + seq_idx] == 1 {
                        let start = tokens_base + seq_idx * hidden_size;
                        for (j, out_val) in out_chunk.iter_mut().enumerate() {
                            let val = token_embeddings[start + j];
                            if val > *out_val {
                                *out_val = val;
                            }
                        }
                    }
                }

                // Replace -inf with 0
                for val in out_chunk.iter_mut() {
                    if val.is_infinite() {
                        *val = 0.0;
                    }
                }
            });

        output
    }
}

// ==================== GPU Similarity ====================

/// GPU-accelerated similarity computations
pub struct GpuSimilarity {
    use_gpu: bool,
    min_candidates: usize,
    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    backend: Option<Arc<dyn GpuBackend>>,
}

impl GpuSimilarity {
    /// Create new GPU similarity calculator
    pub fn new(backend: &dyn GpuBackend, _shaders: &ShaderRegistry) -> Result<Self> {
        Ok(Self {
            use_gpu: backend.is_available() && backend.device_info().supports_compute,
            min_candidates: 64, // Minimum candidates to use GPU
            #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
            backend: None,
        })
    }

    /// Set the backend for GPU operations
    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    pub fn set_backend(&mut self, backend: Arc<dyn GpuBackend>) {
        self.backend = Some(backend);
    }

    /// Batch cosine similarity
    pub fn batch_cosine(&self, query: &[f32], candidates: &[&[f32]]) -> Result<Vec<f32>> {
        #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
        if self.use_gpu && candidates.len() >= self.min_candidates && self.backend.is_some() {
            return self.batch_cosine_gpu(query, candidates);
        }

        Ok(self.batch_cosine_cpu(query, candidates))
    }

    /// Batch dot product
    pub fn batch_dot_product(&self, query: &[f32], candidates: &[&[f32]]) -> Result<Vec<f32>> {
        #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
        if self.use_gpu && candidates.len() >= self.min_candidates && self.backend.is_some() {
            return self.batch_dot_product_gpu(query, candidates);
        }

        Ok(self.batch_dot_product_cpu(query, candidates))
    }

    /// Batch Euclidean distance
    pub fn batch_euclidean(&self, query: &[f32], candidates: &[&[f32]]) -> Result<Vec<f32>> {
        #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
        if self.use_gpu && candidates.len() >= self.min_candidates && self.backend.is_some() {
            return self.batch_euclidean_gpu(query, candidates);
        }

        Ok(self.batch_euclidean_cpu(query, candidates))
    }

    /// Find top-k most similar
    pub fn top_k(&self, query: &[f32], candidates: &[&[f32]], k: usize) -> Result<Vec<(usize, f32)>> {
        let similarities = self.batch_cosine(query, candidates)?;

        let mut indexed: Vec<(usize, f32)> = similarities.into_iter().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        indexed.truncate(k);

        Ok(indexed)
    }

    // GPU implementations

    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    fn batch_cosine_gpu(&self, query: &[f32], candidates: &[&[f32]]) -> Result<Vec<f32>> {
        let backend = self.backend.as_ref().ok_or_else(|| {
            EmbeddingError::GpuOperationFailed {
                operation: "batch_cosine".to_string(),
                reason: "Backend not initialized".to_string(),
            }
        })?;

        let dimension = query.len();
        let num_candidates = candidates.len();

        // Flatten candidates into contiguous buffer
        let candidates_flat: Vec<f32> = candidates.iter().flat_map(|c| c.iter().copied()).collect();

        // Create buffers
        let query_buf = backend.create_buffer((dimension * 4) as u64, BufferUsage::Storage)?;
        let candidates_buf = backend.create_buffer((candidates_flat.len() * 4) as u64, BufferUsage::Storage)?;
        let output_buf = backend.create_buffer((num_candidates * 4) as u64, BufferUsage::Storage)?;

        // Create params buffer (dimension, num_candidates)
        let params: [u32; 2] = [dimension as u32, num_candidates as u32];
        let params_buf = backend.create_buffer(8, BufferUsage::Uniform)?;
        backend.write_buffer(&params_buf, bytemuck::cast_slice(&params))?;

        // Write input data
        backend.write_buffer(&query_buf, bytemuck::cast_slice(query))?;
        backend.write_buffer(&candidates_buf, bytemuck::cast_slice(&candidates_flat))?;

        // Create pipeline with batch cosine shader
        let shader = super::shaders::BATCH_COSINE_SIMILARITY_SHADER;
        let pipeline = backend.create_pipeline(shader, "batch_cosine_similarity", [256, 1, 1])?;

        // Dispatch with params buffer as 4th binding
        let workgroups = [num_candidates.div_ceil(256) as u32, 1, 1];
        backend.dispatch(&pipeline, &[&query_buf, &candidates_buf, &output_buf, &params_buf], workgroups)?;
        backend.sync()?;

        // Read output
        let output_bytes = backend.read_buffer(&output_buf, (num_candidates * 4) as u64)?;
        let output: Vec<f32> = bytemuck::cast_slice(&output_bytes).to_vec();

        // Cleanup
        backend.release_buffer(query_buf)?;
        backend.release_buffer(candidates_buf)?;
        backend.release_buffer(output_buf)?;
        backend.release_buffer(params_buf)?;
        backend.release_pipeline(pipeline)?;

        Ok(output)
    }

    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    fn batch_dot_product_gpu(&self, query: &[f32], candidates: &[&[f32]]) -> Result<Vec<f32>> {
        let backend = self.backend.as_ref().ok_or_else(|| {
            EmbeddingError::GpuOperationFailed {
                operation: "batch_dot_product".to_string(),
                reason: "Backend not initialized".to_string(),
            }
        })?;

        let dimension = query.len();
        let num_candidates = candidates.len();

        // Flatten candidates into contiguous buffer
        let candidates_flat: Vec<f32> = candidates.iter().flat_map(|c| c.iter().copied()).collect();

        // Create buffers
        let query_buf = backend.create_buffer((dimension * 4) as u64, BufferUsage::Storage)?;
        let candidates_buf = backend.create_buffer((candidates_flat.len() * 4) as u64, BufferUsage::Storage)?;
        let output_buf = backend.create_buffer((num_candidates * 4) as u64, BufferUsage::Storage)?;

        // Create params buffer (dimension, num_candidates)
        let params: [u32; 2] = [dimension as u32, num_candidates as u32];
        let params_buf = backend.create_buffer(8, BufferUsage::Uniform)?;
        backend.write_buffer(&params_buf, bytemuck::cast_slice(&params))?;

        // Write input data
        backend.write_buffer(&query_buf, bytemuck::cast_slice(query))?;
        backend.write_buffer(&candidates_buf, bytemuck::cast_slice(&candidates_flat))?;

        // Create pipeline
        let shader = super::shaders::DOT_PRODUCT_SHADER;
        let pipeline = backend.create_pipeline(shader, "dot_product", [256, 1, 1])?;

        // Dispatch with params buffer as 4th binding
        let workgroups = [num_candidates.div_ceil(256) as u32, 1, 1];
        backend.dispatch(&pipeline, &[&query_buf, &candidates_buf, &output_buf, &params_buf], workgroups)?;
        backend.sync()?;

        // Read output
        let output_bytes = backend.read_buffer(&output_buf, (num_candidates * 4) as u64)?;
        let output: Vec<f32> = bytemuck::cast_slice(&output_bytes).to_vec();

        // Cleanup
        backend.release_buffer(query_buf)?;
        backend.release_buffer(candidates_buf)?;
        backend.release_buffer(output_buf)?;
        backend.release_buffer(params_buf)?;
        backend.release_pipeline(pipeline)?;

        Ok(output)
    }

    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    fn batch_euclidean_gpu(&self, query: &[f32], candidates: &[&[f32]]) -> Result<Vec<f32>> {
        let backend = self.backend.as_ref().ok_or_else(|| {
            EmbeddingError::GpuOperationFailed {
                operation: "batch_euclidean".to_string(),
                reason: "Backend not initialized".to_string(),
            }
        })?;

        let dimension = query.len();
        let num_candidates = candidates.len();

        // Flatten candidates into contiguous buffer
        let candidates_flat: Vec<f32> = candidates.iter().flat_map(|c| c.iter().copied()).collect();

        // Create buffers
        let query_buf = backend.create_buffer((dimension * 4) as u64, BufferUsage::Storage)?;
        let candidates_buf = backend.create_buffer((candidates_flat.len() * 4) as u64, BufferUsage::Storage)?;
        let output_buf = backend.create_buffer((num_candidates * 4) as u64, BufferUsage::Storage)?;

        // Create params buffer (dimension, num_candidates)
        let params: [u32; 2] = [dimension as u32, num_candidates as u32];
        let params_buf = backend.create_buffer(8, BufferUsage::Uniform)?;
        backend.write_buffer(&params_buf, bytemuck::cast_slice(&params))?;

        // Write input data
        backend.write_buffer(&query_buf, bytemuck::cast_slice(query))?;
        backend.write_buffer(&candidates_buf, bytemuck::cast_slice(&candidates_flat))?;

        // Create pipeline
        let shader = super::shaders::EUCLIDEAN_DISTANCE_SHADER;
        let pipeline = backend.create_pipeline(shader, "euclidean_distance", [256, 1, 1])?;

        // Dispatch with params buffer as 4th binding
        let workgroups = [num_candidates.div_ceil(256) as u32, 1, 1];
        backend.dispatch(&pipeline, &[&query_buf, &candidates_buf, &output_buf, &params_buf], workgroups)?;
        backend.sync()?;

        // Read output
        let output_bytes = backend.read_buffer(&output_buf, (num_candidates * 4) as u64)?;
        let output: Vec<f32> = bytemuck::cast_slice(&output_bytes).to_vec();

        // Cleanup
        backend.release_buffer(query_buf)?;
        backend.release_buffer(candidates_buf)?;
        backend.release_buffer(output_buf)?;
        backend.release_buffer(params_buf)?;
        backend.release_pipeline(pipeline)?;

        Ok(output)
    }

    // CPU implementations

    fn batch_cosine_cpu(&self, query: &[f32], candidates: &[&[f32]]) -> Vec<f32> {
        candidates
            .par_iter()
            .map(|c| cosine_similarity_cpu(query, c))
            .collect()
    }

    fn batch_dot_product_cpu(&self, query: &[f32], candidates: &[&[f32]]) -> Vec<f32> {
        candidates
            .par_iter()
            .map(|c| dot_product_cpu(query, c))
            .collect()
    }

    fn batch_euclidean_cpu(&self, query: &[f32], candidates: &[&[f32]]) -> Vec<f32> {
        candidates
            .par_iter()
            .map(|c| euclidean_distance_cpu(query, c))
            .collect()
    }
}

// ==================== GPU Vector Operations ====================

/// GPU-accelerated vector operations
pub struct GpuVectorOps {
    use_gpu: bool,
    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    backend: Option<Arc<dyn GpuBackend>>,
}

impl GpuVectorOps {
    /// Create new GPU vector operations
    pub fn new(backend: &dyn GpuBackend, _shaders: &ShaderRegistry) -> Result<Self> {
        Ok(Self {
            use_gpu: backend.is_available() && backend.device_info().supports_compute,
            #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
            backend: None,
        })
    }

    /// Set the backend for GPU operations
    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    pub fn set_backend(&mut self, backend: Arc<dyn GpuBackend>) {
        self.backend = Some(backend);
    }

    /// L2 normalize batch of vectors
    pub fn normalize_batch(&self, vectors: &mut [f32], dimension: usize) -> Result<()> {
        #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
        if self.use_gpu && vectors.len() >= dimension * 64 && self.backend.is_some() {
            return self.normalize_batch_gpu(vectors, dimension);
        }

        self.normalize_batch_cpu(vectors, dimension);
        Ok(())
    }

    /// Matrix-vector multiplication
    pub fn matmul(&self, matrix: &[f32], vector: &[f32], rows: usize, cols: usize) -> Result<Vec<f32>> {
        #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
        if self.use_gpu && rows >= 64 && self.backend.is_some() {
            return self.matmul_gpu(matrix, vector, rows, cols);
        }

        Ok(self.matmul_cpu(matrix, vector, rows, cols))
    }

    /// Batch vector addition
    pub fn batch_add(&self, a: &[f32], b: &[f32]) -> Result<Vec<f32>> {
        if a.len() != b.len() {
            return Err(EmbeddingError::dimension_mismatch(a.len(), b.len()));
        }

        #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
        if self.use_gpu && a.len() >= 1024 && self.backend.is_some() {
            return self.batch_add_gpu(a, b);
        }

        Ok(a.par_iter().zip(b.par_iter()).map(|(x, y)| x + y).collect())
    }

    /// Batch vector scaling
    pub fn batch_scale(&self, vectors: &mut [f32], scale: f32) -> Result<()> {
        vectors.par_iter_mut().for_each(|v| *v *= scale);
        Ok(())
    }

    // GPU implementations

    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    fn normalize_batch_gpu(&self, vectors: &mut [f32], dimension: usize) -> Result<()> {
        let backend = self.backend.as_ref().ok_or_else(|| {
            EmbeddingError::GpuOperationFailed {
                operation: "normalize_batch".to_string(),
                reason: "Backend not initialized".to_string(),
            }
        })?;

        let num_vectors = vectors.len() / dimension;

        // Create buffers (input, dummy, output, params)
        let input_buf = backend.create_buffer((vectors.len() * 4) as u64, BufferUsage::Storage)?;
        let dummy_buf = backend.create_buffer(4, BufferUsage::Storage)?;
        let output_buf = backend.create_buffer((vectors.len() * 4) as u64, BufferUsage::Storage)?;

        // Create params buffer (dimension, num_vectors)
        let params: [u32; 2] = [dimension as u32, num_vectors as u32];
        let params_buf = backend.create_buffer(8, BufferUsage::Uniform)?;
        backend.write_buffer(&params_buf, bytemuck::cast_slice(&params))?;

        // Write input data
        backend.write_buffer(&input_buf, bytemuck::cast_slice(vectors))?;

        // Create pipeline
        let shader = super::shaders::L2_NORMALIZE_SHADER;
        let pipeline = backend.create_pipeline(shader, "l2_normalize", [256, 1, 1])?;

        // Dispatch with 4 bindings
        let workgroups = [num_vectors.div_ceil(256) as u32, 1, 1];
        backend.dispatch(&pipeline, &[&input_buf, &dummy_buf, &output_buf, &params_buf], workgroups)?;
        backend.sync()?;

        // Read output
        let output_bytes = backend.read_buffer(&output_buf, (vectors.len() * 4) as u64)?;
        let output: &[f32] = bytemuck::cast_slice(&output_bytes);
        vectors.copy_from_slice(output);

        // Cleanup
        backend.release_buffer(input_buf)?;
        backend.release_buffer(dummy_buf)?;
        backend.release_buffer(output_buf)?;
        backend.release_buffer(params_buf)?;
        backend.release_pipeline(pipeline)?;

        Ok(())
    }

    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    fn matmul_gpu(&self, matrix: &[f32], vector: &[f32], rows: usize, cols: usize) -> Result<Vec<f32>> {
        let backend = self.backend.as_ref().ok_or_else(|| {
            EmbeddingError::GpuOperationFailed {
                operation: "matmul".to_string(),
                reason: "Backend not initialized".to_string(),
            }
        })?;

        // Create buffers
        let mat_buf = backend.create_buffer((matrix.len() * 4) as u64, BufferUsage::Storage)?;
        let vec_buf = backend.create_buffer((vector.len() * 4) as u64, BufferUsage::Storage)?;
        let output_buf = backend.create_buffer((rows * 4) as u64, BufferUsage::Storage)?;

        // Create params buffer (rows, cols)
        let params: [u32; 2] = [rows as u32, cols as u32];
        let params_buf = backend.create_buffer(8, BufferUsage::Uniform)?;
        backend.write_buffer(&params_buf, bytemuck::cast_slice(&params))?;

        // Write input data
        backend.write_buffer(&mat_buf, bytemuck::cast_slice(matrix))?;
        backend.write_buffer(&vec_buf, bytemuck::cast_slice(vector))?;

        // Create pipeline
        let shader = super::shaders::MATMUL_SHADER;
        let pipeline = backend.create_pipeline(shader, "matmul", [16, 16, 1])?;

        // Dispatch with params buffer as 4th binding
        let workgroups = [rows.div_ceil(16) as u32, 1, 1];
        backend.dispatch(&pipeline, &[&mat_buf, &vec_buf, &output_buf, &params_buf], workgroups)?;
        backend.sync()?;

        // Read output
        let output_bytes = backend.read_buffer(&output_buf, (rows * 4) as u64)?;
        let output: Vec<f32> = bytemuck::cast_slice(&output_bytes).to_vec();

        // Cleanup
        backend.release_buffer(mat_buf)?;
        backend.release_buffer(vec_buf)?;
        backend.release_buffer(output_buf)?;
        backend.release_buffer(params_buf)?;
        backend.release_pipeline(pipeline)?;

        Ok(output)
    }

    #[cfg(any(feature = "gpu", feature = "cuda-wasm"))]
    fn batch_add_gpu(&self, a: &[f32], b: &[f32]) -> Result<Vec<f32>> {
        let backend = self.backend.as_ref().ok_or_else(|| {
            EmbeddingError::GpuOperationFailed {
                operation: "batch_add".to_string(),
                reason: "Backend not initialized".to_string(),
            }
        })?;

        // Create buffers
        let buf_a = backend.create_buffer((a.len() * 4) as u64, BufferUsage::Storage)?;
        let buf_b = backend.create_buffer((b.len() * 4) as u64, BufferUsage::Storage)?;
        let output_buf = backend.create_buffer((a.len() * 4) as u64, BufferUsage::Storage)?;

        // Create params buffer (length)
        let params: [u32; 1] = [a.len() as u32];
        let params_buf = backend.create_buffer(4, BufferUsage::Uniform)?;
        backend.write_buffer(&params_buf, bytemuck::cast_slice(&params))?;

        // Write input data
        backend.write_buffer(&buf_a, bytemuck::cast_slice(a))?;
        backend.write_buffer(&buf_b, bytemuck::cast_slice(b))?;

        // Create pipeline
        let shader = super::shaders::VECTOR_ADD_SHADER;
        let pipeline = backend.create_pipeline(shader, "vector_add", [256, 1, 1])?;

        // Dispatch with params buffer as 4th binding
        let workgroups = [a.len().div_ceil(256) as u32, 1, 1];
        backend.dispatch(&pipeline, &[&buf_a, &buf_b, &output_buf, &params_buf], workgroups)?;
        backend.sync()?;

        // Read output
        let output_bytes = backend.read_buffer(&output_buf, (a.len() * 4) as u64)?;
        let output: Vec<f32> = bytemuck::cast_slice(&output_bytes).to_vec();

        // Cleanup
        backend.release_buffer(buf_a)?;
        backend.release_buffer(buf_b)?;
        backend.release_buffer(output_buf)?;
        backend.release_buffer(params_buf)?;
        backend.release_pipeline(pipeline)?;

        Ok(output)
    }

    // CPU implementations

    fn normalize_batch_cpu(&self, vectors: &mut [f32], dimension: usize) {
        vectors
            .par_chunks_mut(dimension)
            .for_each(|chunk| {
                let norm: f32 = chunk.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 1e-12 {
                    for val in chunk.iter_mut() {
                        *val /= norm;
                    }
                }
            });
    }

    fn matmul_cpu(&self, matrix: &[f32], vector: &[f32], rows: usize, cols: usize) -> Vec<f32> {
        let mut result = vec![0.0f32; rows];

        result
            .par_iter_mut()
            .enumerate()
            .for_each(|(row, out)| {
                let row_start = row * cols;
                *out = matrix[row_start..row_start + cols]
                    .iter()
                    .zip(vector.iter())
                    .map(|(m, v)| m * v)
                    .sum();
            });

        result
    }
}

// ==================== Standalone Functions ====================

/// Batch cosine similarity (GPU-accelerated if available)
pub fn batch_cosine_similarity_gpu(query: &[f32], candidates: &[&[f32]]) -> Vec<f32> {
    candidates
        .par_iter()
        .map(|c| cosine_similarity_cpu(query, c))
        .collect()
}

/// Batch dot product (GPU-accelerated if available)
pub fn batch_dot_product_gpu(query: &[f32], candidates: &[&[f32]]) -> Vec<f32> {
    candidates
        .par_iter()
        .map(|c| dot_product_cpu(query, c))
        .collect()
}

/// Batch Euclidean distance (GPU-accelerated if available)
pub fn batch_euclidean_gpu(query: &[f32], candidates: &[&[f32]]) -> Vec<f32> {
    candidates
        .par_iter()
        .map(|c| euclidean_distance_cpu(query, c))
        .collect()
}

// ==================== CPU Helper Functions ====================

#[inline]
fn cosine_similarity_cpu(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a > 1e-12 && norm_b > 1e-12 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

#[inline]
fn dot_product_cpu(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[inline]
fn euclidean_distance_cpu(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];

        assert!((cosine_similarity_cpu(&a, &b) - 1.0).abs() < 1e-6);
        assert!(cosine_similarity_cpu(&a, &c).abs() < 1e-6);
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];

        assert!((dot_product_cpu(&a, &b) - 32.0).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];

        assert!((euclidean_distance_cpu(&a, &b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_batch_cosine() {
        let query = vec![1.0, 0.0, 0.0];
        let candidates: Vec<&[f32]> = vec![
            &[1.0, 0.0, 0.0][..],
            &[0.0, 1.0, 0.0][..],
            &[0.707, 0.707, 0.0][..],
        ];

        let results = batch_cosine_similarity_gpu(&query, &candidates);

        assert_eq!(results.len(), 3);
        assert!((results[0] - 1.0).abs() < 1e-6);
        assert!(results[1].abs() < 1e-6);
    }

    #[test]
    fn test_mean_pool_cpu() {
        let pooler = GpuPooler {
            use_gpu: false,
            #[cfg(feature = "gpu")]
            backend: None,
        };

        // batch=2, seq=2, hidden=3
        let tokens = vec![
            1.0, 2.0, 3.0,  // batch 0, seq 0
            4.0, 5.0, 6.0,  // batch 0, seq 1
            7.0, 8.0, 9.0,  // batch 1, seq 0
            10.0, 11.0, 12.0, // batch 1, seq 1
        ];
        let mask = vec![1i64, 1, 1, 1];

        let result = pooler.mean_pool_cpu(&tokens, &mask, 2, 2, 3);

        assert_eq!(result.len(), 6);
        // Batch 0: mean of [1,2,3] and [4,5,6] = [2.5, 3.5, 4.5]
        assert!((result[0] - 2.5).abs() < 1e-6);
        assert!((result[1] - 3.5).abs() < 1e-6);
        assert!((result[2] - 4.5).abs() < 1e-6);
    }
}
