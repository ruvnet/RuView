//! # MicroLoRA Adapter Pool for Edge-Net
//!
//! Multi-adapter pooling system for task-specific adaptation in P2P AI networks.
//! Ported from ruvLLM with enhancements for distributed compute.
//!
//! ## Features
//!
//! - **AdapterPool**: LRU-managed pool of task-specific adapters (16 slots default)
//! - **LoraAdapter**: Rank 1-16 low-rank adaptation with SIMD optimization
//! - **Adapter Merging**: Combine multiple adapters with learned weights
//! - **Quantization**: 4-bit and 8-bit quantized adapters for memory efficiency
//! - **P2P Shareable**: Serializable adapters for peer-to-peer distribution
//!
//! ## Performance Targets
//!
//! - Rank-1 forward: <50us
//! - Rank-2 forward: <100us (5% slower than rank-1)
//! - Throughput: 2,236+ ops/sec with batch size 32
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      AdapterPool                             │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐           │
//! │  │ LoraAdapter │ │ LoraAdapter │ │ LoraAdapter │  ...      │
//! │  │ (vectors)   │ │ (embeddings)│ │ (inference) │           │
//! │  └─────────────┘ └─────────────┘ └─────────────┘           │
//! │                                                              │
//! │  ┌──────────────┐  ┌───────────────┐  ┌────────────────┐   │
//! │  │ LRU Eviction │  │ Adapter Merge │  │ Quantization   │   │
//! │  │   Policy     │  │   (weighted)  │  │ (4-bit/8-bit)  │   │
//! │  └──────────────┘  └───────────────┘  └────────────────┘   │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;

/// Optimal batch size for SIMD processing (benchmark-validated)
pub const OPTIMAL_BATCH_SIZE: usize = 32;

/// Default maximum concurrent adapters
pub const DEFAULT_MAX_ADAPTERS: usize = 16;

// ============================================================================
// Task Types for Adapter Routing
// ============================================================================

/// Task types supported by the adapter pool
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum TaskType {
    /// Vector similarity search
    VectorSearch,
    /// Embedding generation
    Embedding,
    /// Neural inference
    Inference,
    /// Encryption/decryption
    Crypto,
    /// Task scheduling
    Scheduling,
    /// Network routing
    Routing,
    /// Pattern recognition
    PatternRecognition,
    /// Custom task with string identifier
    Custom(String),
}

impl TaskType {
    /// Create a task embedding for routing
    pub fn to_embedding(&self) -> Vec<f32> {
        // 64-dimensional task embedding
        let mut embedding = vec![0.0f32; 64];

        match self {
            TaskType::VectorSearch => {
                embedding[0..8].copy_from_slice(&[1.0, 0.8, 0.5, 0.3, 0.0, 0.0, 0.2, 0.1]);
            }
            TaskType::Embedding => {
                embedding[8..16].copy_from_slice(&[1.0, 0.9, 0.7, 0.4, 0.2, 0.1, 0.0, 0.0]);
            }
            TaskType::Inference => {
                embedding[16..24].copy_from_slice(&[0.9, 1.0, 0.8, 0.6, 0.4, 0.3, 0.2, 0.1]);
            }
            TaskType::Crypto => {
                embedding[24..32].copy_from_slice(&[0.5, 0.5, 1.0, 0.8, 0.6, 0.4, 0.2, 0.0]);
            }
            TaskType::Scheduling => {
                embedding[32..40].copy_from_slice(&[0.3, 0.4, 0.5, 1.0, 0.8, 0.6, 0.4, 0.2]);
            }
            TaskType::Routing => {
                embedding[40..48].copy_from_slice(&[0.2, 0.3, 0.4, 0.6, 1.0, 0.8, 0.6, 0.4]);
            }
            TaskType::PatternRecognition => {
                embedding[48..56].copy_from_slice(&[0.4, 0.5, 0.6, 0.7, 0.8, 1.0, 0.9, 0.7]);
            }
            TaskType::Custom(name) => {
                // Hash the custom name to create a unique embedding
                let hash = name.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
                for i in 0..8 {
                    embedding[56 + i] = ((hash >> (i * 8)) & 0xFF) as f32 / 255.0;
                }
            }
        }

        embedding
    }
}

// ============================================================================
// Quantization Support
// ============================================================================

/// Quantization level for adapter weights
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantizationLevel {
    /// Full 32-bit floating point
    F32,
    /// 8-bit quantization (4x memory reduction)
    Q8,
    /// 4-bit quantization (8x memory reduction)
    Q4,
}

/// Quantized tensor representation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuantizedTensor {
    /// Quantized data (packed for Q4)
    data: Vec<u8>,
    /// Scale factor for dequantization
    scale: f32,
    /// Zero point for asymmetric quantization
    zero_point: f32,
    /// Original shape (rows, cols)
    shape: (usize, usize),
    /// Quantization level
    level: QuantizationLevel,
}

impl QuantizedTensor {
    /// Create a quantized tensor from f32 data
    pub fn quantize(data: &[f32], shape: (usize, usize), level: QuantizationLevel) -> Self {
        match level {
            QuantizationLevel::F32 => {
                // No quantization, store as bytes
                let bytes: Vec<u8> = data.iter()
                    .flat_map(|f| f.to_le_bytes())
                    .collect();
                Self {
                    data: bytes,
                    scale: 1.0,
                    zero_point: 0.0,
                    shape,
                    level,
                }
            }
            QuantizationLevel::Q8 => {
                let (min, max) = data.iter().fold((f32::MAX, f32::MIN), |(min, max), &v| {
                    (min.min(v), max.max(v))
                });
                let scale = (max - min) / 255.0;
                let zero_point = min;

                let quantized: Vec<u8> = data.iter()
                    .map(|&v| ((v - zero_point) / scale).clamp(0.0, 255.0) as u8)
                    .collect();

                Self {
                    data: quantized,
                    scale,
                    zero_point,
                    shape,
                    level,
                }
            }
            QuantizationLevel::Q4 => {
                let (min, max) = data.iter().fold((f32::MAX, f32::MIN), |(min, max), &v| {
                    (min.min(v), max.max(v))
                });
                let scale = (max - min) / 15.0;
                let zero_point = min;

                // Pack two 4-bit values per byte
                let mut packed = Vec::with_capacity((data.len() + 1) / 2);
                for chunk in data.chunks(2) {
                    let lo = ((chunk[0] - zero_point) / scale).clamp(0.0, 15.0) as u8;
                    let hi = if chunk.len() > 1 {
                        ((chunk[1] - zero_point) / scale).clamp(0.0, 15.0) as u8
                    } else {
                        0
                    };
                    packed.push((hi << 4) | lo);
                }

                Self {
                    data: packed,
                    scale,
                    zero_point,
                    shape,
                    level,
                }
            }
        }
    }

    /// Dequantize to f32 vector
    pub fn dequantize(&self) -> Vec<f32> {
        match self.level {
            QuantizationLevel::F32 => {
                self.data.chunks(4)
                    .map(|bytes| {
                        let arr = [bytes[0], bytes[1], bytes[2], bytes[3]];
                        f32::from_le_bytes(arr)
                    })
                    .collect()
            }
            QuantizationLevel::Q8 => {
                self.data.iter()
                    .map(|&q| q as f32 * self.scale + self.zero_point)
                    .collect()
            }
            QuantizationLevel::Q4 => {
                let mut result = Vec::with_capacity(self.shape.0 * self.shape.1);
                for &byte in &self.data {
                    let lo = (byte & 0x0F) as f32 * self.scale + self.zero_point;
                    let hi = ((byte >> 4) & 0x0F) as f32 * self.scale + self.zero_point;
                    result.push(lo);
                    result.push(hi);
                }
                result.truncate(self.shape.0 * self.shape.1);
                result
            }
        }
    }

    /// Get memory size in bytes
    pub fn memory_size(&self) -> usize {
        self.data.len() + 8 // data + scale + zero_point
    }
}

// ============================================================================
// LoRA Adapter
// ============================================================================

/// A single LoRA adapter for task-specific adaptation
///
/// Uses low-rank decomposition: W' = W + (A @ B) * (alpha / rank)
/// Where A is down projection and B is up projection.
#[derive(Debug, Serialize, Deserialize)]
pub struct LoraAdapter {
    /// Rank of the adapter (1-16)
    pub rank: u8,
    /// Scaling factor (alpha / rank)
    pub alpha: f32,
    /// Down projection matrix [hidden_dim, rank]
    a_matrix: Vec<f32>,
    /// Up projection matrix [rank, hidden_dim]
    b_matrix: Vec<f32>,
    /// Task embedding for routing
    pub task_embedding: Vec<f32>,
    /// Hidden dimension
    hidden_dim: usize,
    /// Usage count for LRU
    #[serde(skip)]
    usage_count: AtomicU64,
    /// Last used timestamp (ms since epoch)
    #[serde(skip)]
    last_used: AtomicU64,
    /// Quantization level
    quantization: QuantizationLevel,
    /// Quantized A matrix (if quantized)
    a_quantized: Option<QuantizedTensor>,
    /// Quantized B matrix (if quantized)
    b_quantized: Option<QuantizedTensor>,
}

impl Clone for LoraAdapter {
    fn clone(&self) -> Self {
        Self {
            rank: self.rank,
            alpha: self.alpha,
            a_matrix: self.a_matrix.clone(),
            b_matrix: self.b_matrix.clone(),
            task_embedding: self.task_embedding.clone(),
            hidden_dim: self.hidden_dim,
            usage_count: AtomicU64::new(self.usage_count.load(Ordering::Relaxed)),
            last_used: AtomicU64::new(self.last_used.load(Ordering::Relaxed)),
            quantization: self.quantization,
            a_quantized: self.a_quantized.clone(),
            b_quantized: self.b_quantized.clone(),
        }
    }
}

impl LoraAdapter {
    /// Create a new LoRA adapter
    ///
    /// # Arguments
    /// * `hidden_dim` - Model hidden dimension
    /// * `rank` - LoRA rank (1-16)
    /// * `alpha` - Scaling factor (typically equal to rank)
    /// * `task_embedding` - 64-dimensional task embedding for routing
    pub fn new(hidden_dim: usize, rank: u8, alpha: f32, task_embedding: Vec<f32>) -> Self {
        let rank = rank.clamp(1, 16);
        let rank_usize = rank as usize;

        // Initialize A with small random-like values (deterministic for reproducibility)
        // Kaiming initialization scaled for low-rank
        let a_matrix: Vec<f32> = (0..hidden_dim * rank_usize)
            .map(|i| {
                let x = (i as f32 * 0.618033988749895) % 1.0;
                (x - 0.5) * (2.0 / (hidden_dim as f32).sqrt())
            })
            .collect();

        // Initialize B to zero (standard LoRA init - output starts at identity)
        let b_matrix = vec![0.0f32; rank_usize * hidden_dim];

        Self {
            rank,
            alpha: alpha / rank as f32,
            a_matrix,
            b_matrix,
            task_embedding,
            hidden_dim,
            usage_count: AtomicU64::new(0),
            last_used: AtomicU64::new(0),
            quantization: QuantizationLevel::F32,
            a_quantized: None,
            b_quantized: None,
        }
    }

    /// Create a new adapter for a specific task type
    pub fn for_task(hidden_dim: usize, rank: u8, task_type: &TaskType) -> Self {
        Self::new(hidden_dim, rank, rank as f32, task_type.to_embedding())
    }

    /// Quantize the adapter to reduce memory usage
    pub fn quantize(&mut self, level: QuantizationLevel) {
        if level == QuantizationLevel::F32 {
            self.a_quantized = None;
            self.b_quantized = None;
        } else {
            self.a_quantized = Some(QuantizedTensor::quantize(
                &self.a_matrix,
                (self.hidden_dim, self.rank as usize),
                level,
            ));
            self.b_quantized = Some(QuantizedTensor::quantize(
                &self.b_matrix,
                (self.rank as usize, self.hidden_dim),
                level,
            ));
        }
        self.quantization = level;
    }

    /// Get the effective A matrix (dequantized if needed)
    fn get_a_matrix(&self) -> std::borrow::Cow<'_, [f32]> {
        match &self.a_quantized {
            Some(q) => std::borrow::Cow::Owned(q.dequantize()),
            None => std::borrow::Cow::Borrowed(&self.a_matrix),
        }
    }

    /// Get the effective B matrix (dequantized if needed)
    fn get_b_matrix(&self) -> std::borrow::Cow<'_, [f32]> {
        match &self.b_quantized {
            Some(q) => std::borrow::Cow::Owned(q.dequantize()),
            None => std::borrow::Cow::Borrowed(&self.b_matrix),
        }
    }

    /// Scalar forward pass
    fn forward_scalar(&self, input: &[f32], output: &mut [f32]) {
        let a = self.get_a_matrix();
        let b = self.get_b_matrix();
        let rank = self.rank as usize;

        // Down projection: hidden_dim -> rank
        let mut intermediate = vec![0.0f32; rank];
        for r in 0..rank {
            let mut sum = 0.0f32;
            let offset = r * self.hidden_dim;
            for i in 0..self.hidden_dim {
                sum += input[i] * a[offset + i];
            }
            intermediate[r] = sum;
        }

        // Up projection: rank -> hidden_dim
        for i in 0..self.hidden_dim {
            let mut sum = 0.0f32;
            for r in 0..rank {
                sum += intermediate[r] * b[r * self.hidden_dim + i];
            }
            output[i] += sum * self.alpha;
        }
    }

    /// SIMD-optimized forward pass (AVX2)
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    fn forward_simd(&self, input: &[f32], output: &mut [f32]) {
        use std::arch::x86_64::*;

        let a = self.get_a_matrix();
        let b = self.get_b_matrix();
        let rank = self.rank as usize;

        unsafe {
            // Down projection: hidden_dim -> rank
            let mut intermediate = vec![0.0f32; rank];

            for r in 0..rank {
                let mut sum = _mm256_setzero_ps();
                let offset = r * self.hidden_dim;

                let mut i = 0;
                while i + 8 <= self.hidden_dim {
                    let inp = _mm256_loadu_ps(input[i..].as_ptr());
                    let weight = _mm256_loadu_ps(a[offset + i..].as_ptr());
                    sum = _mm256_fmadd_ps(inp, weight, sum);
                    i += 8;
                }

                // Horizontal sum
                let mut result = [0.0f32; 8];
                _mm256_storeu_ps(result.as_mut_ptr(), sum);
                intermediate[r] = result.iter().sum();

                // Handle remaining elements
                for j in i..self.hidden_dim {
                    intermediate[r] += input[j] * a[offset + j];
                }
            }

            // Up projection: rank -> hidden_dim
            let scale_vec = _mm256_set1_ps(self.alpha);

            let mut i = 0;
            while i + 8 <= self.hidden_dim {
                let mut sum = _mm256_setzero_ps();

                for r in 0..rank {
                    let up_offset = r * self.hidden_dim;
                    let weight = _mm256_loadu_ps(b[up_offset + i..].as_ptr());
                    let inter = _mm256_set1_ps(intermediate[r]);
                    sum = _mm256_fmadd_ps(inter, weight, sum);
                }

                // Scale and add to output
                sum = _mm256_mul_ps(sum, scale_vec);
                let existing = _mm256_loadu_ps(output[i..].as_ptr());
                let result = _mm256_add_ps(existing, sum);
                _mm256_storeu_ps(output[i..].as_mut_ptr(), result);

                i += 8;
            }

            // Handle remaining elements
            for j in i..self.hidden_dim {
                let mut val = 0.0;
                for r in 0..rank {
                    val += intermediate[r] * b[r * self.hidden_dim + j];
                }
                output[j] += val * self.alpha;
            }
        }
    }

    /// WASM SIMD forward pass
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    fn forward_simd(&self, input: &[f32], output: &mut [f32]) {
        use std::arch::wasm32::*;

        let a = self.get_a_matrix();
        let b = self.get_b_matrix();
        let rank = self.rank as usize;

        // Down projection: hidden_dim -> rank
        let mut intermediate = vec![0.0f32; rank];

        for r in 0..rank {
            let mut sum = f32x4_splat(0.0);
            let offset = r * self.hidden_dim;

            let mut i = 0;
            while i + 4 <= self.hidden_dim {
                let inp = v128_load(input[i..].as_ptr() as *const v128);
                let weight = v128_load(a[offset + i..].as_ptr() as *const v128);
                sum = f32x4_add(sum, f32x4_mul(inp, weight));
                i += 4;
            }

            // Horizontal sum
            intermediate[r] = f32x4_extract_lane::<0>(sum)
                + f32x4_extract_lane::<1>(sum)
                + f32x4_extract_lane::<2>(sum)
                + f32x4_extract_lane::<3>(sum);

            // Handle remaining elements
            for j in i..self.hidden_dim {
                intermediate[r] += input[j] * a[offset + j];
            }
        }

        // Up projection: rank -> hidden_dim
        let scale_vec = f32x4_splat(self.alpha);

        let mut i = 0;
        while i + 4 <= self.hidden_dim {
            let mut sum = f32x4_splat(0.0);

            for r in 0..rank {
                let up_offset = r * self.hidden_dim;
                let weight = v128_load(b[up_offset + i..].as_ptr() as *const v128);
                let inter = f32x4_splat(intermediate[r]);
                sum = f32x4_add(sum, f32x4_mul(inter, weight));
            }

            // Scale and add to output
            sum = f32x4_mul(sum, scale_vec);
            let existing = v128_load(output[i..].as_ptr() as *const v128);
            let result = f32x4_add(existing, sum);
            v128_store(output[i..].as_mut_ptr() as *mut v128, result);

            i += 4;
        }

        // Handle remaining elements
        for j in i..self.hidden_dim {
            let mut val = 0.0;
            for r in 0..rank {
                val += intermediate[r] * b[r * self.hidden_dim + j];
            }
            output[j] += val * self.alpha;
        }
    }

    /// Forward pass with automatic SIMD detection
    pub fn forward(&self, input: &[f32], output: &mut [f32]) {
        assert_eq!(input.len(), self.hidden_dim, "Input dimension mismatch");
        assert_eq!(output.len(), self.hidden_dim, "Output dimension mismatch");

        // Update usage stats
        self.usage_count.fetch_add(1, Ordering::Relaxed);
        #[cfg(target_arch = "wasm32")]
        {
            self.last_used.store(js_sys::Date::now() as u64, Ordering::Relaxed);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
            self.last_used.store(now, Ordering::Relaxed);
        }

        #[cfg(any(
            all(target_arch = "x86_64", target_feature = "avx2"),
            all(target_arch = "wasm32", target_feature = "simd128")
        ))]
        {
            self.forward_simd(input, output);
            return;
        }

        #[allow(unreachable_code)]
        self.forward_scalar(input, output);
    }

    /// Batch forward pass with optimal chunking
    pub fn forward_batch(&self, inputs: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let mut outputs: Vec<Vec<f32>> = inputs
            .iter()
            .map(|_| vec![0.0f32; self.hidden_dim])
            .collect();

        // Process in optimal batch sizes
        for chunk_start in (0..inputs.len()).step_by(OPTIMAL_BATCH_SIZE) {
            let chunk_end = (chunk_start + OPTIMAL_BATCH_SIZE).min(inputs.len());
            for i in chunk_start..chunk_end {
                self.forward(&inputs[i], &mut outputs[i]);
            }
        }

        outputs
    }

    /// Accumulate gradient for online learning
    pub fn accumulate_gradient(&mut self, gradient: &[f32], learning_rate: f32) {
        if gradient.len() != self.hidden_dim {
            return;
        }

        // Simple SGD update on B matrix (main adaptation target)
        for r in 0..self.rank as usize {
            for i in 0..self.hidden_dim {
                let idx = r * self.hidden_dim + i;
                self.b_matrix[idx] += gradient[i] * learning_rate;
            }
        }

        // Clear quantized cache if updated
        if self.quantization != QuantizationLevel::F32 {
            self.b_quantized = Some(QuantizedTensor::quantize(
                &self.b_matrix,
                (self.rank as usize, self.hidden_dim),
                self.quantization,
            ));
        }
    }

    /// Get usage count
    pub fn usage_count(&self) -> u64 {
        self.usage_count.load(Ordering::Relaxed)
    }

    /// Get last used timestamp
    pub fn last_used(&self) -> u64 {
        self.last_used.load(Ordering::Relaxed)
    }

    /// Get parameter count
    pub fn param_count(&self) -> usize {
        self.a_matrix.len() + self.b_matrix.len()
    }

    /// Get memory size in bytes
    pub fn memory_size(&self) -> usize {
        match self.quantization {
            QuantizationLevel::F32 => {
                (self.a_matrix.len() + self.b_matrix.len()) * 4
            }
            _ => {
                let a_size = self.a_quantized.as_ref().map(|q| q.memory_size()).unwrap_or(0);
                let b_size = self.b_quantized.as_ref().map(|q| q.memory_size()).unwrap_or(0);
                a_size + b_size
            }
        }
    }

    /// Serialize to bytes for P2P sharing
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        bincode::deserialize(bytes).ok()
    }

    /// Calculate cosine similarity to a task embedding
    pub fn similarity_to(&self, task_embedding: &[f32]) -> f32 {
        if task_embedding.len() != self.task_embedding.len() {
            return 0.0;
        }

        let dot: f32 = self.task_embedding.iter()
            .zip(task_embedding)
            .map(|(a, b)| a * b)
            .sum();

        let norm_a: f32 = self.task_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = task_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }
}

// ============================================================================
// Compute Operations Trait
// ============================================================================

/// Trait for compute operations (abstraction for different backends)
pub trait ComputeOps: Send + Sync {
    /// Matrix-vector multiplication
    fn matvec(&self, matrix: &[f32], vector: &[f32], rows: usize, cols: usize) -> Vec<f32>;

    /// Dot product
    fn dot(&self, a: &[f32], b: &[f32]) -> f32;
}

/// Default CPU compute operations
#[derive(Clone, Default)]
pub struct CpuComputeOps;

impl ComputeOps for CpuComputeOps {
    fn matvec(&self, matrix: &[f32], vector: &[f32], rows: usize, cols: usize) -> Vec<f32> {
        let mut result = vec![0.0f32; rows];
        for r in 0..rows {
            let offset = r * cols;
            result[r] = matrix[offset..offset + cols]
                .iter()
                .zip(vector)
                .map(|(m, v)| m * v)
                .sum();
        }
        result
    }

    fn dot(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }
}

// ============================================================================
// Adapter Pool
// ============================================================================

/// Pool entry with metadata
struct PoolEntry {
    adapter: LoraAdapter,
    task_type: TaskType,
}

/// LRU eviction policy
#[derive(Clone, Copy, Debug, Default)]
pub struct LruEvictionPolicy {
    /// Minimum usage count to consider for eviction
    min_usage_threshold: u64,
}

impl LruEvictionPolicy {
    /// Create a new LRU eviction policy
    pub fn new() -> Self {
        Self {
            min_usage_threshold: 0,
        }
    }

    /// Set minimum usage threshold
    pub fn with_min_usage(mut self, threshold: u64) -> Self {
        self.min_usage_threshold = threshold;
        self
    }
}

/// Adapter pool for managing task-specific LoRA adapters
///
/// Features:
/// - LRU eviction when pool is full
/// - Task embedding-based routing
/// - Adapter merging with learned weights
/// - Quantization support
pub struct AdapterPool {
    /// Adapters indexed by task type
    adapters: RwLock<FxHashMap<TaskType, PoolEntry>>,
    /// Maximum concurrent adapters
    active_slots: usize,
    /// Eviction policy
    eviction_policy: LruEvictionPolicy,
    /// Compute operations backend
    compute: Arc<dyn ComputeOps>,
    /// Default hidden dimension
    hidden_dim: usize,
    /// Default rank
    default_rank: u8,
}

impl AdapterPool {
    /// Create a new adapter pool
    pub fn new(hidden_dim: usize, active_slots: usize) -> Self {
        Self {
            adapters: RwLock::new(FxHashMap::default()),
            active_slots: active_slots.max(1),
            eviction_policy: LruEvictionPolicy::new(),
            compute: Arc::new(CpuComputeOps),
            hidden_dim,
            default_rank: 2,
        }
    }

    /// Create with custom compute backend
    pub fn with_compute(mut self, compute: Arc<dyn ComputeOps>) -> Self {
        self.compute = compute;
        self
    }

    /// Set eviction policy
    pub fn with_eviction_policy(mut self, policy: LruEvictionPolicy) -> Self {
        self.eviction_policy = policy;
        self
    }

    /// Set default rank
    pub fn with_default_rank(mut self, rank: u8) -> Self {
        self.default_rank = rank.clamp(1, 16);
        self
    }

    /// Get or create an adapter for a task type
    pub fn get_or_create(&self, task_type: &TaskType) -> LoraAdapter {
        // Try to get existing adapter
        {
            let adapters = self.adapters.read();
            if let Some(entry) = adapters.get(task_type) {
                return entry.adapter.clone();
            }
        }

        // Create new adapter
        self.create_adapter(task_type)
    }

    /// Create a new adapter for a task type
    pub fn create_adapter(&self, task_type: &TaskType) -> LoraAdapter {
        let adapter = LoraAdapter::for_task(self.hidden_dim, self.default_rank, task_type);

        // Check if we need to evict
        let mut adapters = self.adapters.write();
        if adapters.len() >= self.active_slots {
            self.evict_lru(&mut adapters);
        }

        let cloned = adapter.clone();
        adapters.insert(task_type.clone(), PoolEntry {
            adapter,
            task_type: task_type.clone(),
        });

        cloned
    }

    /// Evict the least recently used adapter
    fn evict_lru(&self, adapters: &mut FxHashMap<TaskType, PoolEntry>) {
        if adapters.is_empty() {
            return;
        }

        // Find LRU adapter (lowest last_used timestamp that meets threshold)
        let lru_key = adapters.iter()
            .filter(|(_, entry)| {
                entry.adapter.usage_count() >= self.eviction_policy.min_usage_threshold
            })
            .min_by_key(|(_, entry)| entry.adapter.last_used())
            .map(|(k, _)| k.clone());

        // If all adapters are below threshold, evict the oldest anyway
        let lru_key = lru_key.or_else(|| {
            adapters.iter()
                .min_by_key(|(_, entry)| entry.adapter.last_used())
                .map(|(k, _)| k.clone())
        });

        if let Some(key) = lru_key {
            adapters.remove(&key);
        }
    }

    /// Insert an adapter directly
    pub fn insert(&self, task_type: TaskType, adapter: LoraAdapter) {
        let mut adapters = self.adapters.write();
        if adapters.len() >= self.active_slots {
            self.evict_lru(&mut adapters);
        }
        adapters.insert(task_type.clone(), PoolEntry {
            adapter,
            task_type,
        });
    }

    /// Remove an adapter
    pub fn remove(&self, task_type: &TaskType) -> Option<LoraAdapter> {
        self.adapters.write().remove(task_type).map(|e| e.adapter)
    }

    /// Get adapter count
    pub fn len(&self) -> usize {
        self.adapters.read().len()
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        self.adapters.read().is_empty()
    }

    // ========================================================================
    // Exotic Features
    // ========================================================================

    /// Merge multiple adapters with learned weights
    ///
    /// Creates a new adapter by combining multiple adapters using weighted
    /// averaging of their parameters. Useful for task transfer learning.
    pub fn merge_adapters(&self, adapters: &[&LoraAdapter], weights: &[f32]) -> LoraAdapter {
        if adapters.is_empty() || adapters.len() != weights.len() {
            return LoraAdapter::new(self.hidden_dim, self.default_rank, self.default_rank as f32, vec![0.0; 64]);
        }

        // Normalize weights
        let weight_sum: f32 = weights.iter().sum();
        let normalized: Vec<f32> = if weight_sum > 0.0 {
            weights.iter().map(|w| w / weight_sum).collect()
        } else {
            vec![1.0 / adapters.len() as f32; adapters.len()]
        };

        // Use the first adapter as template
        let template = adapters[0];
        let hidden_dim = template.hidden_dim;
        let rank = template.rank;

        // Merge A matrices
        let mut merged_a = vec![0.0f32; hidden_dim * rank as usize];
        for (adapter, &weight) in adapters.iter().zip(normalized.iter()) {
            let a = adapter.get_a_matrix();
            for (i, val) in a.iter().enumerate() {
                merged_a[i] += val * weight;
            }
        }

        // Merge B matrices
        let mut merged_b = vec![0.0f32; rank as usize * hidden_dim];
        for (adapter, &weight) in adapters.iter().zip(normalized.iter()) {
            let b = adapter.get_b_matrix();
            for (i, val) in b.iter().enumerate() {
                merged_b[i] += val * weight;
            }
        }

        // Merge task embeddings
        let mut merged_embedding = vec![0.0f32; 64];
        for (adapter, &weight) in adapters.iter().zip(normalized.iter()) {
            for (i, val) in adapter.task_embedding.iter().enumerate() {
                if i < merged_embedding.len() {
                    merged_embedding[i] += val * weight;
                }
            }
        }

        // Create merged adapter
        let mut merged = LoraAdapter::new(hidden_dim, rank, rank as f32, merged_embedding);
        merged.a_matrix = merged_a;
        merged.b_matrix = merged_b;

        merged
    }

    /// Apply quantization-aware adaptation
    ///
    /// Performs forward pass with automatic quantization/dequantization
    /// for memory-efficient inference.
    pub fn adapt_quantized(&self, x: &[f32], adapter: &LoraAdapter) -> Vec<f32> {
        let mut output = x.to_vec();
        adapter.forward(x, &mut output);
        output
    }

    /// Route to the best matching adapter based on task embedding
    ///
    /// Uses cosine similarity to find the adapter with the most similar
    /// task embedding. Returns None if no adapters are available.
    pub fn route_to_adapter(&self, task_embedding: &[f32]) -> Option<LoraAdapter> {
        let adapters = self.adapters.read();

        adapters.values()
            .max_by(|a, b| {
                let sim_a = a.adapter.similarity_to(task_embedding);
                let sim_b = b.adapter.similarity_to(task_embedding);
                sim_a.partial_cmp(&sim_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|entry| entry.adapter.clone())
    }

    /// Get statistics about the pool
    pub fn stats(&self) -> PoolStats {
        let adapters = self.adapters.read();

        let total_memory: usize = adapters.values()
            .map(|e| e.adapter.memory_size())
            .sum();

        let total_usage: u64 = adapters.values()
            .map(|e| e.adapter.usage_count())
            .sum();

        let avg_usage = if adapters.is_empty() {
            0.0
        } else {
            total_usage as f64 / adapters.len() as f64
        };

        PoolStats {
            adapter_count: adapters.len(),
            max_slots: self.active_slots,
            total_memory_bytes: total_memory,
            total_usage_count: total_usage,
            avg_usage_count: avg_usage,
        }
    }

    /// Export all adapters for P2P sharing
    pub fn export_all(&self) -> Vec<(TaskType, Vec<u8>)> {
        self.adapters.read()
            .iter()
            .map(|(task_type, entry)| {
                (task_type.clone(), entry.adapter.to_bytes())
            })
            .collect()
    }

    /// Import adapters from P2P peers
    pub fn import(&self, task_type: TaskType, bytes: &[u8]) -> bool {
        if let Some(adapter) = LoraAdapter::from_bytes(bytes) {
            self.insert(task_type, adapter);
            true
        } else {
            false
        }
    }
}

/// Pool statistics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolStats {
    /// Number of active adapters
    pub adapter_count: usize,
    /// Maximum adapter slots
    pub max_slots: usize,
    /// Total memory usage in bytes
    pub total_memory_bytes: usize,
    /// Total usage count across all adapters
    pub total_usage_count: u64,
    /// Average usage count per adapter
    pub avg_usage_count: f64,
}

// ============================================================================
// WASM Bindings
// ============================================================================

use wasm_bindgen::prelude::*;

/// WASM-compatible adapter pool wrapper
#[wasm_bindgen]
pub struct WasmAdapterPool {
    inner: AdapterPool,
}

#[wasm_bindgen]
impl WasmAdapterPool {
    /// Create a new adapter pool
    #[wasm_bindgen(constructor)]
    pub fn new(hidden_dim: usize, max_slots: usize) -> Self {
        Self {
            inner: AdapterPool::new(hidden_dim, max_slots),
        }
    }

    /// Get or create an adapter for a task type
    #[wasm_bindgen(js_name = getAdapter)]
    pub fn get_adapter(&self, task_type: &str) -> JsValue {
        let task = match task_type {
            "vector_search" => TaskType::VectorSearch,
            "embedding" => TaskType::Embedding,
            "inference" => TaskType::Inference,
            "crypto" => TaskType::Crypto,
            "scheduling" => TaskType::Scheduling,
            "routing" => TaskType::Routing,
            "pattern_recognition" => TaskType::PatternRecognition,
            other => TaskType::Custom(other.to_string()),
        };

        let adapter = self.inner.get_or_create(&task);
        serde_wasm_bindgen::to_value(&AdapterInfo {
            rank: adapter.rank,
            hidden_dim: adapter.hidden_dim,
            param_count: adapter.param_count(),
            memory_bytes: adapter.memory_size(),
            usage_count: adapter.usage_count(),
        }).unwrap_or(JsValue::NULL)
    }

    /// Apply adapter to input
    #[wasm_bindgen(js_name = forward)]
    pub fn forward(&self, task_type: &str, input: &[f32]) -> Vec<f32> {
        let task = match task_type {
            "vector_search" => TaskType::VectorSearch,
            "embedding" => TaskType::Embedding,
            "inference" => TaskType::Inference,
            "crypto" => TaskType::Crypto,
            other => TaskType::Custom(other.to_string()),
        };

        let adapter = self.inner.get_or_create(&task);
        let mut output = input.to_vec();
        adapter.forward(input, &mut output);
        output
    }

    /// Route to best adapter by task embedding
    #[wasm_bindgen(js_name = routeToAdapter)]
    pub fn route_to_adapter(&self, task_embedding: &[f32]) -> JsValue {
        match self.inner.route_to_adapter(task_embedding) {
            Some(adapter) => serde_wasm_bindgen::to_value(&AdapterInfo {
                rank: adapter.rank,
                hidden_dim: adapter.hidden_dim,
                param_count: adapter.param_count(),
                memory_bytes: adapter.memory_size(),
                usage_count: adapter.usage_count(),
            }).unwrap_or(JsValue::NULL),
            None => JsValue::NULL,
        }
    }

    /// Get pool statistics
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> JsValue {
        let stats = self.inner.stats();
        serde_wasm_bindgen::to_value(&stats).unwrap_or(JsValue::NULL)
    }

    /// Get adapter count
    #[wasm_bindgen(js_name = adapterCount)]
    pub fn adapter_count(&self) -> usize {
        self.inner.len()
    }

    /// Export adapter to bytes for P2P sharing
    #[wasm_bindgen(js_name = exportAdapter)]
    pub fn export_adapter(&self, task_type: &str) -> Vec<u8> {
        let task = match task_type {
            "vector_search" => TaskType::VectorSearch,
            "embedding" => TaskType::Embedding,
            "inference" => TaskType::Inference,
            other => TaskType::Custom(other.to_string()),
        };

        let adapters = self.inner.adapters.read();
        adapters.get(&task)
            .map(|e| e.adapter.to_bytes())
            .unwrap_or_default()
    }

    /// Import adapter from bytes
    #[wasm_bindgen(js_name = importAdapter)]
    pub fn import_adapter(&self, task_type: &str, bytes: &[u8]) -> bool {
        let task = match task_type {
            "vector_search" => TaskType::VectorSearch,
            "embedding" => TaskType::Embedding,
            "inference" => TaskType::Inference,
            other => TaskType::Custom(other.to_string()),
        };

        self.inner.import(task, bytes)
    }
}

/// Adapter info for JavaScript
#[derive(Clone, Debug, Serialize, Deserialize)]
struct AdapterInfo {
    rank: u8,
    hidden_dim: usize,
    param_count: usize,
    memory_bytes: usize,
    usage_count: u64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = LoraAdapter::new(128, 2, 2.0, vec![0.0; 64]);
        assert_eq!(adapter.rank, 2);
        assert_eq!(adapter.hidden_dim, 128);
        assert_eq!(adapter.param_count(), 128 * 2 + 2 * 128);
    }

    #[test]
    fn test_adapter_forward() {
        let adapter = LoraAdapter::new(64, 1, 1.0, vec![0.0; 64]);
        let input = vec![1.0f32; 64];
        let mut output = vec![0.0f32; 64];

        adapter.forward(&input, &mut output);

        // With zero-init B matrix, output should still be zero
        let sum: f32 = output.iter().sum();
        assert!(sum.abs() < 1e-6, "Expected ~0 with zero B_matrix, got {}", sum);
    }

    #[test]
    fn test_adapter_quantization() {
        let mut adapter = LoraAdapter::new(64, 2, 2.0, vec![0.0; 64]);

        let initial_size = adapter.memory_size();
        adapter.quantize(QuantizationLevel::Q8);
        let q8_size = adapter.memory_size();

        // Q8 should be ~4x smaller
        assert!(q8_size < initial_size, "Q8 should reduce memory");

        adapter.quantize(QuantizationLevel::Q4);
        let q4_size = adapter.memory_size();

        // Q4 should be ~8x smaller than F32
        assert!(q4_size < q8_size, "Q4 should reduce memory further");
    }

    #[test]
    fn test_pool_creation() {
        let pool = AdapterPool::new(128, 8);
        assert_eq!(pool.active_slots, 8);
        assert!(pool.is_empty());
    }

    #[test]
    fn test_pool_get_or_create() {
        let pool = AdapterPool::new(64, 4);

        let adapter1 = pool.get_or_create(&TaskType::VectorSearch);
        assert_eq!(adapter1.hidden_dim, 64);

        let adapter2 = pool.get_or_create(&TaskType::VectorSearch);
        assert_eq!(adapter2.hidden_dim, 64);

        // Should only have one adapter
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn test_pool_eviction() {
        let pool = AdapterPool::new(64, 2);

        pool.get_or_create(&TaskType::VectorSearch);
        pool.get_or_create(&TaskType::Embedding);
        assert_eq!(pool.len(), 2);

        // This should trigger eviction
        pool.get_or_create(&TaskType::Inference);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn test_adapter_merge() {
        let pool = AdapterPool::new(64, 4);

        let adapter1 = pool.get_or_create(&TaskType::VectorSearch);
        let adapter2 = pool.get_or_create(&TaskType::Embedding);

        let merged = pool.merge_adapters(&[&adapter1, &adapter2], &[0.7, 0.3]);

        assert_eq!(merged.hidden_dim, 64);
        assert_eq!(merged.rank, adapter1.rank);
    }

    #[test]
    fn test_adapter_routing() {
        let pool = AdapterPool::new(64, 4);

        pool.get_or_create(&TaskType::VectorSearch);
        pool.get_or_create(&TaskType::Embedding);

        let query_embedding = TaskType::VectorSearch.to_embedding();
        let routed = pool.route_to_adapter(&query_embedding);

        assert!(routed.is_some());
    }

    #[test]
    fn test_adapter_serialization() {
        let adapter = LoraAdapter::new(64, 2, 2.0, vec![0.5; 64]);

        let bytes = adapter.to_bytes();
        assert!(!bytes.is_empty());

        let restored = LoraAdapter::from_bytes(&bytes);
        assert!(restored.is_some());

        let restored = restored.unwrap();
        assert_eq!(restored.rank, adapter.rank);
        assert_eq!(restored.hidden_dim, adapter.hidden_dim);
    }

    #[test]
    fn test_quantized_tensor() {
        let data = vec![0.0, 0.25, 0.5, 0.75, 1.0];

        let q8 = QuantizedTensor::quantize(&data, (1, 5), QuantizationLevel::Q8);
        let dequantized = q8.dequantize();

        // Should be approximately equal
        for (orig, deq) in data.iter().zip(dequantized.iter()) {
            assert!((orig - deq).abs() < 0.01, "Q8 dequantization error too high");
        }

        let q4 = QuantizedTensor::quantize(&data, (1, 5), QuantizationLevel::Q4);
        let dequantized = q4.dequantize();

        // Q4 has more error but should still be close
        for (orig, deq) in data.iter().zip(dequantized.iter().take(data.len())) {
            assert!((orig - deq).abs() < 0.1, "Q4 dequantization error too high");
        }
    }

    #[test]
    fn test_task_type_embedding() {
        let embedding1 = TaskType::VectorSearch.to_embedding();
        let embedding2 = TaskType::Embedding.to_embedding();

        assert_eq!(embedding1.len(), 64);
        assert_eq!(embedding2.len(), 64);

        // Different task types should have different embeddings
        assert_ne!(embedding1, embedding2);
    }

    #[test]
    fn test_pool_stats() {
        let pool = AdapterPool::new(64, 4);

        pool.get_or_create(&TaskType::VectorSearch);
        pool.get_or_create(&TaskType::Embedding);

        let stats = pool.stats();
        assert_eq!(stats.adapter_count, 2);
        assert_eq!(stats.max_slots, 4);
        assert!(stats.total_memory_bytes > 0);
    }

    #[test]
    fn test_adapter_gradient() {
        let mut adapter = LoraAdapter::new(64, 2, 2.0, vec![0.0; 64]);

        let gradient = vec![0.1f32; 64];
        adapter.accumulate_gradient(&gradient, 0.01);

        let input = vec![1.0f32; 64];
        let mut output = vec![0.0f32; 64];
        adapter.forward(&input, &mut output);

        // After gradient update, output should be non-zero
        let sum: f32 = output.iter().map(|x| x.abs()).sum();
        assert!(sum > 0.0, "Expected non-zero output after gradient update");
    }
}
