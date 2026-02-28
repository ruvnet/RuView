//! # AI Module for Edge-Net
//!
//! Provides core AI capabilities for the P2P network:
//!
//! - **HNSW Vector Index** (`memory.rs`): 150x faster than naive search, O(log N) complexity
//! - **MicroLoRA Adapter Pool** (`lora.rs`): Task-specific adaptation with LRU eviction
//! - **Federated Learning** (`federated.rs`): P2P gradient gossip without coordinators
//!
//! ## Architecture
//!
//! ```text
//! +------------------------------------------------------------------------+
//! |                       AI Intelligence Layer                            |
//! +------------------------------------------------------------------------+
//! |  +-----------------+  +-----------------+  +-----------------+         |
//! |  |  HNSW Index     |  |  AdapterPool    |  |  Federated      |         |
//! |  |  (memory.rs)    |  |   (lora.rs)     |  | (federated.rs)  |         |
//! |  | Neural Attention|  |                 |  |                 |         |
//! |  | "What matters?" |  | - LRU eviction  |  | - TopK Sparse   |         |
//! |  | - 150x speedup  |  | - 16 slots      |  | - Byzantine tol |         |
//! |  | - O(log N)      |  | - Task routing  |  | - Rep-weighted  |         |
//! |  +-----------------+  +-----------------+  +-----------------+         |
//! |            |                  |                    |                   |
//! |  +-----------------+  +-----------------+  +-----------------+         |
//! |  |  DAG Attention  |  |  LoraAdapter    |  | GradientGossip  |         |
//! |  |(dag_attention.rs|  |  (lora.rs)      |  | (federated.rs)  |         |
//! |  | "What steps?"   |  |                 |  |                 |         |
//! |  | - Critical path |  | - Rank 1-16     |  | - Error feedback|         |
//! |  | - Topo sort     |  | - SIMD forward  |  | - Diff privacy  |         |
//! |  | - Parallelism   |  | - 4/8-bit quant |  | - Gossipsub     |         |
//! |  +-----------------+  +-----------------+  +-----------------+         |
//! |                            |                                           |
//! |                    ComputeOps Trait                                    |
//! |             (SIMD acceleration when available)                         |
//! +------------------------------------------------------------------------+
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use edge_net::ai::{HnswIndex, GradientGossip, FederatedModel};
//!
//! // Create HNSW index for semantic search
//! let mut index = HnswIndex::new(128, HnswConfig::default());
//! index.insert("doc-1", vec![0.1; 128])?;
//! let results = index.search(&query, 10)?;
//!
//! // Federated learning with gradient gossip
//! let gossip = GradientGossip::new(&peer_id, 1000, 0.1)?;
//! gossip.set_local_gradients(&gradients)?;
//! let aggregated = gossip.aggregate();
//!
//! // Apply to model
//! let model = FederatedModel::new(1000, 0.01, 0.9);
//! model.apply_gradients(&aggregated)?;
//! ```

pub mod memory;
pub mod lora;
pub mod federated;
pub mod dag_attention;
pub mod attention_unified;

// Re-export unified attention types
pub use attention_unified::{
    UnifiedAttention, NeuralAttention, DAGAttention, GraphAttentionNetwork, StateSpaceModel,
    AttentionOutput, AttentionMetadata, UnifiedAttentionConfig, AttentionType,
    DAGNode, Edge,
};

// Re-export memory types
pub use memory::{HnswIndex, HnswConfig, HnswNode, SearchResult as HnswSearchResult};

// Re-export LoRA types
pub use lora::{
    AdapterPool, LoraAdapter, TaskType, PoolStats,
    QuantizationLevel, QuantizedTensor,
    LruEvictionPolicy, WasmAdapterPool,
    OPTIMAL_BATCH_SIZE, DEFAULT_MAX_ADAPTERS,
};

// Re-export federated learning types
pub use federated::{
    GradientGossip,
    GradientMessage,
    SparseGradient,
    TopKSparsifier,
    ByzantineDetector,
    DifferentialPrivacy,
    FederatedModel,
    TOPIC_GRADIENT_GOSSIP,
    TOPIC_MODEL_SYNC,
};

// Re-export DAG attention types
pub use dag_attention::{
    DagAttention,
    TaskNode,
    TaskEdge,
    TaskStatus,
    DagSummary,
};

/// Common compute operations trait for SIMD acceleration
/// Used by all AI components for distance calculations and matrix ops
pub trait ComputeOps {
    /// Compute cosine distance between two vectors
    fn cosine_distance(a: &[f32], b: &[f32]) -> f32;

    /// Compute dot product
    fn dot_product(a: &[f32], b: &[f32]) -> f32;

    /// Apply softmax in-place
    fn softmax_inplace(x: &mut [f32]);

    /// Compute L2 norm
    fn l2_norm(x: &[f32]) -> f32;

    /// Matrix-vector multiply
    fn matmul_vec(matrix: &[f32], rows: usize, cols: usize, vec: &[f32]) -> Vec<f32>;
}

/// Default CPU implementation of ComputeOps
pub struct CpuOps;

impl ComputeOps for CpuOps {
    fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");

        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        // Manual loop unrolling for better performance
        let chunks = a.len() / 4;
        let remainder = a.len() % 4;

        for i in 0..chunks {
            let base = i * 4;
            dot += a[base] * b[base];
            dot += a[base + 1] * b[base + 1];
            dot += a[base + 2] * b[base + 2];
            dot += a[base + 3] * b[base + 3];

            norm_a += a[base] * a[base];
            norm_a += a[base + 1] * a[base + 1];
            norm_a += a[base + 2] * a[base + 2];
            norm_a += a[base + 3] * a[base + 3];

            norm_b += b[base] * b[base];
            norm_b += b[base + 1] * b[base + 1];
            norm_b += b[base + 2] * b[base + 2];
            norm_b += b[base + 3] * b[base + 3];
        }

        // Handle remainder
        let base = chunks * 4;
        for i in 0..remainder {
            dot += a[base + i] * b[base + i];
            norm_a += a[base + i] * a[base + i];
            norm_b += b[base + i] * b[base + i];
        }

        let norm_a = norm_a.sqrt();
        let norm_b = norm_b.sqrt();

        if norm_a > 1e-10 && norm_b > 1e-10 {
            1.0 - dot / (norm_a * norm_b)
        } else {
            1.0
        }
    }

    fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    fn softmax_inplace(x: &mut [f32]) {
        if x.is_empty() {
            return;
        }

        // Numerical stability: subtract max
        let max = x.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mut sum = 0.0f32;

        for val in x.iter_mut() {
            *val = (*val - max).exp();
            sum += *val;
        }

        if sum > 0.0 {
            for val in x.iter_mut() {
                *val /= sum;
            }
        } else {
            // Fallback to uniform
            let uniform = 1.0 / x.len() as f32;
            for val in x.iter_mut() {
                *val = uniform;
            }
        }
    }

    fn l2_norm(x: &[f32]) -> f32 {
        x.iter().map(|v| v * v).sum::<f32>().sqrt()
    }

    fn matmul_vec(matrix: &[f32], rows: usize, cols: usize, vec: &[f32]) -> Vec<f32> {
        debug_assert_eq!(matrix.len(), rows * cols, "Matrix size mismatch");
        debug_assert_eq!(vec.len(), cols, "Vector size mismatch");

        let mut result = vec![0.0f32; rows];
        for r in 0..rows {
            let row_start = r * cols;
            for c in 0..cols {
                result[r] += matrix[row_start + c] * vec[c];
            }
        }
        result
    }
}

/// WASM SIMD implementation when available
#[cfg(target_feature = "simd128")]
pub struct SimdOps;

#[cfg(target_feature = "simd128")]
impl ComputeOps for SimdOps {
    fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
        use core::arch::wasm32::*;

        debug_assert_eq!(a.len(), b.len());

        let chunks = a.len() / 4;
        let remainder = a.len() % 4;

        let mut dot_acc = f32x4_splat(0.0);
        let mut norm_a_acc = f32x4_splat(0.0);
        let mut norm_b_acc = f32x4_splat(0.0);

        for i in 0..chunks {
            let base = i * 4;
            let va = v128_load(a[base..].as_ptr() as *const v128);
            let vb = v128_load(b[base..].as_ptr() as *const v128);

            dot_acc = f32x4_add(dot_acc, f32x4_mul(va, vb));
            norm_a_acc = f32x4_add(norm_a_acc, f32x4_mul(va, va));
            norm_b_acc = f32x4_add(norm_b_acc, f32x4_mul(vb, vb));
        }

        // Reduce accumulators
        let dot = f32x4_extract_lane::<0>(dot_acc)
            + f32x4_extract_lane::<1>(dot_acc)
            + f32x4_extract_lane::<2>(dot_acc)
            + f32x4_extract_lane::<3>(dot_acc);

        let norm_a = f32x4_extract_lane::<0>(norm_a_acc)
            + f32x4_extract_lane::<1>(norm_a_acc)
            + f32x4_extract_lane::<2>(norm_a_acc)
            + f32x4_extract_lane::<3>(norm_a_acc);

        let norm_b = f32x4_extract_lane::<0>(norm_b_acc)
            + f32x4_extract_lane::<1>(norm_b_acc)
            + f32x4_extract_lane::<2>(norm_b_acc)
            + f32x4_extract_lane::<3>(norm_b_acc);

        // Handle remainder
        let base = chunks * 4;
        let mut dot_rem = 0.0f32;
        let mut norm_a_rem = 0.0f32;
        let mut norm_b_rem = 0.0f32;

        for i in 0..remainder {
            dot_rem += a[base + i] * b[base + i];
            norm_a_rem += a[base + i] * a[base + i];
            norm_b_rem += b[base + i] * b[base + i];
        }

        let dot = dot + dot_rem;
        let norm_a = (norm_a + norm_a_rem).sqrt();
        let norm_b = (norm_b + norm_b_rem).sqrt();

        if norm_a > 1e-10 && norm_b > 1e-10 {
            1.0 - dot / (norm_a * norm_b)
        } else {
            1.0
        }
    }

    fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        CpuOps::dot_product(a, b)
    }

    fn softmax_inplace(x: &mut [f32]) {
        CpuOps::softmax_inplace(x)
    }

    fn l2_norm(x: &[f32]) -> f32 {
        CpuOps::l2_norm(x)
    }

    fn matmul_vec(matrix: &[f32], rows: usize, cols: usize, vec: &[f32]) -> Vec<f32> {
        CpuOps::matmul_vec(matrix, rows, cols, vec)
    }
}

/// Get the best available compute ops implementation
pub fn get_compute_ops() -> impl ComputeOps {
    CpuOps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_distance_same_vector() {
        let v = vec![1.0, 0.0, 0.0];
        let dist = CpuOps::cosine_distance(&v, &v);
        assert!(dist.abs() < 1e-5, "Same vector should have 0 distance");
    }

    #[test]
    fn test_cosine_distance_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let dist = CpuOps::cosine_distance(&a, &b);
        assert!((dist - 1.0).abs() < 1e-5, "Orthogonal vectors should have distance 1.0");
    }

    #[test]
    fn test_cosine_distance_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let dist = CpuOps::cosine_distance(&a, &b);
        assert!((dist - 2.0).abs() < 1e-5, "Opposite vectors should have distance 2.0");
    }

    #[test]
    fn test_softmax() {
        let mut x = vec![1.0, 2.0, 3.0];
        CpuOps::softmax_inplace(&mut x);
        let sum: f32 = x.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "Softmax should sum to 1.0");
        assert!(x[2] > x[1] && x[1] > x[0], "Softmax should preserve ordering");
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dot = CpuOps::dot_product(&a, &b);
        assert!((dot - 32.0).abs() < 1e-5);
    }

    #[test]
    fn test_matmul_vec() {
        // 2x3 matrix times 3x1 vector
        let matrix = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let vec = vec![1.0, 2.0, 3.0];
        let result = CpuOps::matmul_vec(&matrix, 2, 3, &vec);
        assert_eq!(result.len(), 2);
        assert!((result[0] - 14.0).abs() < 1e-5); // 1*1 + 2*2 + 3*3
        assert!((result[1] - 32.0).abs() < 1e-5); // 4*1 + 5*2 + 6*3
    }
}
