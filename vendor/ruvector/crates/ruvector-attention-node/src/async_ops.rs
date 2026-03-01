//! NAPI-RS bindings for async and batch operations
//!
//! Provides Node.js bindings for:
//! - Async attention computation with tokio
//! - Batch processing utilities
//! - Parallel attention computation

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_attention::{
    attention::ScaledDotProductAttention,
    hyperbolic::{HyperbolicAttention, HyperbolicAttentionConfig},
    sparse::{FlashAttention, LinearAttention, LocalGlobalAttention},
    traits::Attention,
};
use std::sync::Arc;

// ============================================================================
// Batch Processing Configuration
// ============================================================================

/// Batch processing configuration
#[napi(object)]
pub struct BatchConfig {
    pub batch_size: u32,
    pub num_workers: Option<u32>,
    pub prefetch: Option<bool>,
}

/// Batch processing result
#[napi(object)]
pub struct BatchResult {
    pub outputs: Vec<Float32Array>,
    pub elapsed_ms: f64,
    pub throughput: f64,
}

// ============================================================================
// Async Attention Operations
// ============================================================================

/// Async scaled dot-product attention computation
#[napi]
pub async fn compute_attention_async(
    query: Float32Array,
    keys: Vec<Float32Array>,
    values: Vec<Float32Array>,
    dim: u32,
) -> Result<Float32Array> {
    let query_vec = query.to_vec();
    let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
    let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();

    let result = tokio::task::spawn_blocking(move || {
        let attention = ScaledDotProductAttention::new(dim as usize);
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        attention.compute(&query_vec, &keys_refs, &values_refs)
    })
    .await
    .map_err(|e| Error::from_reason(e.to_string()))?
    .map_err(|e| Error::from_reason(e.to_string()))?;

    Ok(Float32Array::new(result))
}

/// Async flash attention computation
#[napi]
pub async fn compute_flash_attention_async(
    query: Float32Array,
    keys: Vec<Float32Array>,
    values: Vec<Float32Array>,
    dim: u32,
    block_size: u32,
) -> Result<Float32Array> {
    let query_vec = query.to_vec();
    let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
    let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();

    let result = tokio::task::spawn_blocking(move || {
        let attention = FlashAttention::new(dim as usize, block_size as usize);
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        attention.compute(&query_vec, &keys_refs, &values_refs)
    })
    .await
    .map_err(|e| Error::from_reason(e.to_string()))?
    .map_err(|e| Error::from_reason(e.to_string()))?;

    Ok(Float32Array::new(result))
}

/// Async hyperbolic attention computation
#[napi]
pub async fn compute_hyperbolic_attention_async(
    query: Float32Array,
    keys: Vec<Float32Array>,
    values: Vec<Float32Array>,
    dim: u32,
    curvature: f64,
) -> Result<Float32Array> {
    let query_vec = query.to_vec();
    let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
    let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();

    let result = tokio::task::spawn_blocking(move || {
        let config = HyperbolicAttentionConfig {
            dim: dim as usize,
            curvature: curvature as f32,
            ..Default::default()
        };
        let attention = HyperbolicAttention::new(config);
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        attention.compute(&query_vec, &keys_refs, &values_refs)
    })
    .await
    .map_err(|e| Error::from_reason(e.to_string()))?
    .map_err(|e| Error::from_reason(e.to_string()))?;

    Ok(Float32Array::new(result))
}

// ============================================================================
// Batch Processing
// ============================================================================

/// Process a batch of attention computations
#[napi]
pub async fn batch_attention_compute(
    queries: Vec<Float32Array>,
    keys: Vec<Vec<Float32Array>>,
    values: Vec<Vec<Float32Array>>,
    dim: u32,
) -> Result<BatchResult> {
    let start = std::time::Instant::now();
    let batch_size = queries.len();

    // Convert to owned vectors for thread safety
    let queries_vec: Vec<Vec<f32>> = queries.into_iter().map(|q| q.to_vec()).collect();
    let keys_vec: Vec<Vec<Vec<f32>>> = keys
        .into_iter()
        .map(|k| k.into_iter().map(|arr| arr.to_vec()).collect())
        .collect();
    let values_vec: Vec<Vec<Vec<f32>>> = values
        .into_iter()
        .map(|v| v.into_iter().map(|arr| arr.to_vec()).collect())
        .collect();

    let dim_usize = dim as usize;

    let results = tokio::task::spawn_blocking(move || {
        let attention = ScaledDotProductAttention::new(dim_usize);
        let mut outputs = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let keys_refs: Vec<&[f32]> = keys_vec[i].iter().map(|k| k.as_slice()).collect();
            let values_refs: Vec<&[f32]> = values_vec[i].iter().map(|v| v.as_slice()).collect();

            match attention.compute(&queries_vec[i], &keys_refs, &values_refs) {
                Ok(output) => outputs.push(output),
                Err(e) => return Err(e.to_string()),
            }
        }

        Ok(outputs)
    })
    .await
    .map_err(|e| Error::from_reason(e.to_string()))?
    .map_err(|e| Error::from_reason(e))?;

    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let throughput = batch_size as f64 / start.elapsed().as_secs_f64();

    Ok(BatchResult {
        outputs: results.into_iter().map(Float32Array::new).collect(),
        elapsed_ms,
        throughput,
    })
}

/// Process a batch with flash attention
#[napi]
pub async fn batch_flash_attention_compute(
    queries: Vec<Float32Array>,
    keys: Vec<Vec<Float32Array>>,
    values: Vec<Vec<Float32Array>>,
    dim: u32,
    block_size: u32,
) -> Result<BatchResult> {
    let start = std::time::Instant::now();
    let batch_size = queries.len();

    let queries_vec: Vec<Vec<f32>> = queries.into_iter().map(|q| q.to_vec()).collect();
    let keys_vec: Vec<Vec<Vec<f32>>> = keys
        .into_iter()
        .map(|k| k.into_iter().map(|arr| arr.to_vec()).collect())
        .collect();
    let values_vec: Vec<Vec<Vec<f32>>> = values
        .into_iter()
        .map(|v| v.into_iter().map(|arr| arr.to_vec()).collect())
        .collect();

    let dim_usize = dim as usize;
    let block_usize = block_size as usize;

    let results = tokio::task::spawn_blocking(move || {
        let attention = FlashAttention::new(dim_usize, block_usize);
        let mut outputs = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let keys_refs: Vec<&[f32]> = keys_vec[i].iter().map(|k| k.as_slice()).collect();
            let values_refs: Vec<&[f32]> = values_vec[i].iter().map(|v| v.as_slice()).collect();

            match attention.compute(&queries_vec[i], &keys_refs, &values_refs) {
                Ok(output) => outputs.push(output),
                Err(e) => return Err(e.to_string()),
            }
        }

        Ok(outputs)
    })
    .await
    .map_err(|e| Error::from_reason(e.to_string()))?
    .map_err(|e| Error::from_reason(e))?;

    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let throughput = batch_size as f64 / start.elapsed().as_secs_f64();

    Ok(BatchResult {
        outputs: results.into_iter().map(Float32Array::new).collect(),
        elapsed_ms,
        throughput,
    })
}

// ============================================================================
// Parallel Attention Computation
// ============================================================================

/// Attention type for parallel computation
#[napi(string_enum)]
pub enum AttentionType {
    ScaledDotProduct,
    Flash,
    Linear,
    LocalGlobal,
    Hyperbolic,
}

/// Configuration for parallel attention
#[napi(object)]
pub struct ParallelConfig {
    pub attention_type: AttentionType,
    pub dim: u32,
    pub block_size: Option<u32>,
    pub num_features: Option<u32>,
    pub local_window: Option<u32>,
    pub global_tokens: Option<u32>,
    pub curvature: Option<f64>,
}

/// Parallel attention computation across multiple queries
#[napi]
pub async fn parallel_attention_compute(
    config: ParallelConfig,
    queries: Vec<Float32Array>,
    keys: Vec<Vec<Float32Array>>,
    values: Vec<Vec<Float32Array>>,
) -> Result<BatchResult> {
    let start = std::time::Instant::now();
    let batch_size = queries.len();

    let queries_vec: Vec<Vec<f32>> = queries.into_iter().map(|q| q.to_vec()).collect();
    let keys_vec: Vec<Vec<Vec<f32>>> = keys
        .into_iter()
        .map(|k| k.into_iter().map(|arr| arr.to_vec()).collect())
        .collect();
    let values_vec: Vec<Vec<Vec<f32>>> = values
        .into_iter()
        .map(|v| v.into_iter().map(|arr| arr.to_vec()).collect())
        .collect();

    let dim = config.dim as usize;
    let attention_type = config.attention_type;
    let block_size = config.block_size.unwrap_or(64) as usize;
    let num_features = config.num_features.unwrap_or(64) as usize;
    let local_window = config.local_window.unwrap_or(128) as usize;
    let global_tokens = config.global_tokens.unwrap_or(8) as usize;
    let curvature = config.curvature.unwrap_or(1.0) as f32;

    let results = tokio::task::spawn_blocking(move || {
        let mut outputs = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let keys_refs: Vec<&[f32]> = keys_vec[i].iter().map(|k| k.as_slice()).collect();
            let values_refs: Vec<&[f32]> = values_vec[i].iter().map(|v| v.as_slice()).collect();

            let result = match attention_type {
                AttentionType::ScaledDotProduct => {
                    let attention = ScaledDotProductAttention::new(dim);
                    attention.compute(&queries_vec[i], &keys_refs, &values_refs)
                }
                AttentionType::Flash => {
                    let attention = FlashAttention::new(dim, block_size);
                    attention.compute(&queries_vec[i], &keys_refs, &values_refs)
                }
                AttentionType::Linear => {
                    let attention = LinearAttention::new(dim, num_features);
                    attention.compute(&queries_vec[i], &keys_refs, &values_refs)
                }
                AttentionType::LocalGlobal => {
                    let attention = LocalGlobalAttention::new(dim, local_window, global_tokens);
                    attention.compute(&queries_vec[i], &keys_refs, &values_refs)
                }
                AttentionType::Hyperbolic => {
                    let config = HyperbolicAttentionConfig {
                        dim,
                        curvature,
                        ..Default::default()
                    };
                    let attention = HyperbolicAttention::new(config);
                    attention.compute(&queries_vec[i], &keys_refs, &values_refs)
                }
            };

            match result {
                Ok(output) => outputs.push(output),
                Err(e) => return Err(e.to_string()),
            }
        }

        Ok(outputs)
    })
    .await
    .map_err(|e| Error::from_reason(e.to_string()))?
    .map_err(|e| Error::from_reason(e))?;

    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let throughput = batch_size as f64 / start.elapsed().as_secs_f64();

    Ok(BatchResult {
        outputs: results.into_iter().map(Float32Array::new).collect(),
        elapsed_ms,
        throughput,
    })
}

// ============================================================================
// Streaming Processing
// ============================================================================

/// Stream processor for handling attention in chunks
#[napi]
pub struct StreamProcessor {
    dim: usize,
    buffer: Vec<Vec<f32>>,
    max_buffer_size: usize,
}

#[napi]
impl StreamProcessor {
    /// Create a new stream processor
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `max_buffer_size` - Maximum number of items to buffer
    #[napi(constructor)]
    pub fn new(dim: u32, max_buffer_size: u32) -> Self {
        Self {
            dim: dim as usize,
            buffer: Vec::new(),
            max_buffer_size: max_buffer_size as usize,
        }
    }

    /// Add a vector to the buffer
    #[napi]
    pub fn push(&mut self, vector: Float32Array) -> bool {
        if self.buffer.len() >= self.max_buffer_size {
            return false;
        }
        self.buffer.push(vector.to_vec());
        true
    }

    /// Process buffered vectors with attention against a query
    #[napi]
    pub fn process(&self, query: Float32Array) -> Result<Float32Array> {
        if self.buffer.is_empty() {
            return Err(Error::from_reason("Buffer is empty"));
        }

        let attention = ScaledDotProductAttention::new(self.dim);
        let query_slice = query.as_ref();
        let keys_refs: Vec<&[f32]> = self.buffer.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = self.buffer.iter().map(|v| v.as_slice()).collect();

        let result = attention
            .compute(query_slice, &keys_refs, &values_refs)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    /// Clear the buffer
    #[napi]
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Get current buffer size
    #[napi(getter)]
    pub fn size(&self) -> u32 {
        self.buffer.len() as u32
    }

    /// Check if buffer is full
    #[napi(getter)]
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.max_buffer_size
    }
}

// ============================================================================
// Benchmark Utilities
// ============================================================================

/// Benchmark result
#[napi(object)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: u32,
    pub total_ms: f64,
    pub avg_ms: f64,
    pub ops_per_sec: f64,
    pub min_ms: f64,
    pub max_ms: f64,
}

/// Run attention benchmark
#[napi]
pub async fn benchmark_attention(
    attention_type: AttentionType,
    dim: u32,
    seq_length: u32,
    iterations: u32,
) -> Result<BenchmarkResult> {
    let dim_usize = dim as usize;
    let seq_usize = seq_length as usize;
    let iter_usize = iterations as usize;

    let result = tokio::task::spawn_blocking(move || {
        // Generate test data
        let query: Vec<f32> = (0..dim_usize).map(|i| (i as f32 * 0.01).sin()).collect();
        let keys: Vec<Vec<f32>> = (0..seq_usize)
            .map(|j| {
                (0..dim_usize)
                    .map(|i| ((i + j) as f32 * 0.01).cos())
                    .collect()
            })
            .collect();
        let values: Vec<Vec<f32>> = keys.clone();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let name = match attention_type {
            AttentionType::ScaledDotProduct => "ScaledDotProduct",
            AttentionType::Flash => "Flash",
            AttentionType::Linear => "Linear",
            AttentionType::LocalGlobal => "LocalGlobal",
            AttentionType::Hyperbolic => "Hyperbolic",
        }
        .to_string();

        let mut times: Vec<f64> = Vec::with_capacity(iter_usize);

        for _ in 0..iter_usize {
            let start = std::time::Instant::now();

            match attention_type {
                AttentionType::ScaledDotProduct => {
                    let attention = ScaledDotProductAttention::new(dim_usize);
                    let _ = attention.compute(&query, &keys_refs, &values_refs);
                }
                AttentionType::Flash => {
                    let attention = FlashAttention::new(dim_usize, 64);
                    let _ = attention.compute(&query, &keys_refs, &values_refs);
                }
                AttentionType::Linear => {
                    let attention = LinearAttention::new(dim_usize, 64);
                    let _ = attention.compute(&query, &keys_refs, &values_refs);
                }
                AttentionType::LocalGlobal => {
                    let attention = LocalGlobalAttention::new(dim_usize, 128, 8);
                    let _ = attention.compute(&query, &keys_refs, &values_refs);
                }
                AttentionType::Hyperbolic => {
                    let config = HyperbolicAttentionConfig {
                        dim: dim_usize,
                        curvature: 1.0,
                        ..Default::default()
                    };
                    let attention = HyperbolicAttention::new(config);
                    let _ = attention.compute(&query, &keys_refs, &values_refs);
                }
            }

            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        let total_ms: f64 = times.iter().sum();
        let avg_ms = total_ms / iter_usize as f64;
        let min_ms = times.iter().copied().fold(f64::INFINITY, f64::min);
        let max_ms = times.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let ops_per_sec = 1000.0 / avg_ms;

        BenchmarkResult {
            name,
            iterations: iterations,
            total_ms,
            avg_ms,
            ops_per_sec,
            min_ms,
            max_ms,
        }
    })
    .await
    .map_err(|e| Error::from_reason(e.to_string()))?;

    Ok(result)
}
