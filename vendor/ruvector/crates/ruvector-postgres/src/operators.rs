//! SQL operators and distance functions for vector similarity search
//!
//! Provides both array-based and native ruvector type distance functions with SIMD optimization.

use pgrx::prelude::*;

use crate::distance::{
    cosine_distance, euclidean_distance, inner_product_distance, manhattan_distance,
};
use crate::types::RuVector;

// ============================================================================
// Native RuVector Type Distance Functions (Zero-Copy SIMD)
// ============================================================================
// These functions use the native ruvector type directly for maximum performance

/// Compute L2 (Euclidean) distance between two native ruvector types
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_l2_distance(a: RuVector, b: RuVector) -> f32 {
    if a.dimensions() != b.dimensions() {
        pgrx::error!(
            "Cannot compute distance between vectors of different dimensions ({} vs {})",
            a.dimensions(),
            b.dimensions()
        );
    }
    euclidean_distance(a.as_slice(), b.as_slice())
}

/// Compute cosine distance between two native ruvector types
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_cosine_distance(a: RuVector, b: RuVector) -> f32 {
    if a.dimensions() != b.dimensions() {
        pgrx::error!(
            "Cannot compute distance between vectors of different dimensions ({} vs {})",
            a.dimensions(),
            b.dimensions()
        );
    }
    cosine_distance(a.as_slice(), b.as_slice())
}

/// Compute inner product between two native ruvector types
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_inner_product(a: RuVector, b: RuVector) -> f32 {
    if a.dimensions() != b.dimensions() {
        pgrx::error!(
            "Cannot compute inner product between vectors of different dimensions ({} vs {})",
            a.dimensions(),
            b.dimensions()
        );
    }
    -inner_product_distance(a.as_slice(), b.as_slice())
}

/// Compute Manhattan (L1) distance between two native ruvector types
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_l1_distance(a: RuVector, b: RuVector) -> f32 {
    if a.dimensions() != b.dimensions() {
        pgrx::error!(
            "Cannot compute distance between vectors of different dimensions ({} vs {})",
            a.dimensions(),
            b.dimensions()
        );
    }
    manhattan_distance(a.as_slice(), b.as_slice())
}

/// Get dimensions of a native ruvector
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_dims(v: RuVector) -> i32 {
    v.dimensions() as i32
}

/// Get L2 norm of a native ruvector
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_norm(v: RuVector) -> f32 {
    v.norm()
}

/// Normalize a native ruvector to unit length
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_normalize(v: RuVector) -> RuVector {
    v.normalize()
}

/// Add two native ruvector types
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_add(a: RuVector, b: RuVector) -> RuVector {
    if a.dimensions() != b.dimensions() {
        pgrx::error!("Vectors must have the same dimensions");
    }
    a.add(&b)
}

/// Subtract two native ruvector types
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_sub(a: RuVector, b: RuVector) -> RuVector {
    if a.dimensions() != b.dimensions() {
        pgrx::error!("Vectors must have the same dimensions");
    }
    a.sub(&b)
}

/// Multiply native ruvector by scalar
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_mul_scalar(v: RuVector, scalar: f32) -> RuVector {
    v.mul_scalar(scalar)
}

// ============================================================================
// Distance Functions (Array-based) with SIMD Optimization
// ============================================================================

/// Compute L2 (Euclidean) distance between two float arrays
/// Uses SIMD acceleration (AVX-512, AVX2, or NEON) automatically
#[pg_extern(immutable, parallel_safe)]
pub fn l2_distance_arr(a: Vec<f32>, b: Vec<f32>) -> f32 {
    if a.len() != b.len() {
        pgrx::error!(
            "Cannot compute distance between vectors of different dimensions ({} vs {})",
            a.len(),
            b.len()
        );
    }
    euclidean_distance(&a, &b)
}

/// Compute inner product between two float arrays
/// Uses SIMD acceleration automatically
#[pg_extern(immutable, parallel_safe)]
pub fn inner_product_arr(a: Vec<f32>, b: Vec<f32>) -> f32 {
    if a.len() != b.len() {
        pgrx::error!(
            "Cannot compute distance between vectors of different dimensions ({} vs {})",
            a.len(),
            b.len()
        );
    }
    -inner_product_distance(&a, &b)
}

/// Compute negative inner product (for ORDER BY ASC nearest neighbor)
/// Uses SIMD acceleration automatically
#[pg_extern(immutable, parallel_safe)]
pub fn neg_inner_product_arr(a: Vec<f32>, b: Vec<f32>) -> f32 {
    if a.len() != b.len() {
        pgrx::error!(
            "Cannot compute distance between vectors of different dimensions ({} vs {})",
            a.len(),
            b.len()
        );
    }
    inner_product_distance(&a, &b)
}

/// Compute cosine distance between two float arrays
/// Uses SIMD acceleration automatically
#[pg_extern(immutable, parallel_safe)]
pub fn cosine_distance_arr(a: Vec<f32>, b: Vec<f32>) -> f32 {
    if a.len() != b.len() {
        pgrx::error!(
            "Cannot compute distance between vectors of different dimensions ({} vs {})",
            a.len(),
            b.len()
        );
    }
    cosine_distance(&a, &b)
}

/// Compute cosine similarity between two float arrays
#[pg_extern(immutable, parallel_safe)]
pub fn cosine_similarity_arr(a: Vec<f32>, b: Vec<f32>) -> f32 {
    1.0 - cosine_distance_arr(a, b)
}

/// Compute L1 (Manhattan) distance between two float arrays
/// Uses SIMD acceleration automatically
#[pg_extern(immutable, parallel_safe)]
pub fn l1_distance_arr(a: Vec<f32>, b: Vec<f32>) -> f32 {
    if a.len() != b.len() {
        pgrx::error!(
            "Cannot compute distance between vectors of different dimensions ({} vs {})",
            a.len(),
            b.len()
        );
    }
    manhattan_distance(&a, &b)
}

// ============================================================================
// Vector Utility Functions
// ============================================================================

/// Normalize a vector to unit length
#[pg_extern(immutable, parallel_safe)]
pub fn vector_normalize(v: Vec<f32>) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        return v;
    }
    v.iter().map(|x| x / norm).collect()
}

/// Add two vectors element-wise
#[pg_extern(immutable, parallel_safe)]
pub fn vector_add(a: Vec<f32>, b: Vec<f32>) -> Vec<f32> {
    if a.len() != b.len() {
        pgrx::error!("Vectors must have the same dimensions");
    }
    a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
}

/// Subtract two vectors element-wise
#[pg_extern(immutable, parallel_safe)]
pub fn vector_sub(a: Vec<f32>, b: Vec<f32>) -> Vec<f32> {
    if a.len() != b.len() {
        pgrx::error!("Vectors must have the same dimensions");
    }
    a.iter().zip(b.iter()).map(|(x, y)| x - y).collect()
}

/// Multiply vector by scalar
#[pg_extern(immutable, parallel_safe)]
pub fn vector_mul_scalar(v: Vec<f32>, scalar: f32) -> Vec<f32> {
    v.iter().map(|x| x * scalar).collect()
}

/// Get vector dimensions
#[pg_extern(immutable, parallel_safe)]
pub fn vector_dims(v: Vec<f32>) -> i32 {
    v.len() as i32
}

/// Get vector L2 norm
#[pg_extern(immutable, parallel_safe)]
pub fn vector_norm(v: Vec<f32>) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Average two vectors
#[pg_extern(immutable, parallel_safe)]
pub fn vector_avg2(a: Vec<f32>, b: Vec<f32>) -> Vec<f32> {
    if a.len() != b.len() {
        pgrx::error!("Vectors must have the same dimensions");
    }
    a.iter().zip(b.iter()).map(|(x, y)| (x + y) / 2.0).collect()
}

// ============================================================================
// Fast Pre-Normalized Cosine Distance
// ============================================================================

/// Compute fast cosine distance for pre-normalized vectors
/// Only computes dot product (3x faster than regular cosine)
#[pg_extern(immutable, parallel_safe)]
pub fn cosine_distance_normalized_arr(a: Vec<f32>, b: Vec<f32>) -> f32 {
    if a.len() != b.len() {
        pgrx::error!(
            "Cannot compute distance between vectors of different dimensions ({} vs {})",
            a.len(),
            b.len()
        );
    }
    crate::distance::cosine_distance_normalized(&a, &b)
}

// ============================================================================
// Temporal Compression Functions (Time-Series Vector Optimization)
// ============================================================================

/// Compute delta between two consecutive vectors (for temporal compression)
#[pg_extern(immutable, parallel_safe)]
pub fn temporal_delta(current: Vec<f32>, previous: Vec<f32>) -> Vec<f32> {
    if current.len() != previous.len() {
        pgrx::error!("Vectors must have same dimensions");
    }
    current
        .iter()
        .zip(previous.iter())
        .map(|(c, p)| c - p)
        .collect()
}

/// Reconstruct vector from delta and previous vector
#[pg_extern(immutable, parallel_safe)]
pub fn temporal_undelta(delta: Vec<f32>, previous: Vec<f32>) -> Vec<f32> {
    if delta.len() != previous.len() {
        pgrx::error!("Vectors must have same dimensions");
    }
    delta
        .iter()
        .zip(previous.iter())
        .map(|(d, p)| d + p)
        .collect()
}

/// Compute exponential moving average update
/// Returns: alpha * current + (1-alpha) * ema_prev
#[pg_extern(immutable, parallel_safe)]
pub fn temporal_ema_update(current: Vec<f32>, ema_prev: Vec<f32>, alpha: f32) -> Vec<f32> {
    if current.len() != ema_prev.len() {
        pgrx::error!("Vectors must have same dimensions");
    }
    if alpha <= 0.0 || alpha > 1.0 {
        pgrx::error!("Alpha must be in (0, 1]");
    }

    current
        .iter()
        .zip(ema_prev.iter())
        .map(|(c, e)| alpha * c + (1.0 - alpha) * e)
        .collect()
}

/// Compute temporal drift (rate of change) between vectors
#[pg_extern(immutable, parallel_safe)]
pub fn temporal_drift(v1: Vec<f32>, v2: Vec<f32>, time_delta: f32) -> f32 {
    if v1.len() != v2.len() {
        pgrx::error!("Vectors must have same dimensions");
    }
    if time_delta <= 0.0 {
        pgrx::error!("Time delta must be positive");
    }

    euclidean_distance(&v1, &v2) / time_delta
}

/// Compute vector velocity (first derivative approximation)
#[pg_extern(immutable, parallel_safe)]
pub fn temporal_velocity(v_t0: Vec<f32>, v_t1: Vec<f32>, dt: f32) -> Vec<f32> {
    if v_t0.len() != v_t1.len() {
        pgrx::error!("Vectors must have same dimensions");
    }
    if dt <= 0.0 {
        pgrx::error!("Time delta must be positive");
    }

    v_t1.iter()
        .zip(v_t0.iter())
        .map(|(t1, t0)| (t1 - t0) / dt)
        .collect()
}

// ============================================================================
// Attention Mechanism Functions (Scaled Dot-Product Attention)
// ============================================================================

/// Compute scaled dot-product attention score between query and single key
/// Returns (Q·K) / sqrt(d_k) - use with aggregate for multiple keys
#[pg_extern(immutable, parallel_safe)]
pub fn attention_score(query: Vec<f32>, key: Vec<f32>) -> f32 {
    if query.len() != key.len() {
        pgrx::error!("Query and key must have same dimensions");
    }
    let dim = query.len();
    let scale = (dim as f32).sqrt();
    let dot: f32 = query.iter().zip(key.iter()).map(|(q, k)| q * k).sum();
    dot / scale
}

/// Apply softmax to array of scores
#[pg_extern(immutable, parallel_safe)]
pub fn attention_softmax(scores: Vec<f32>) -> Vec<f32> {
    if scores.is_empty() {
        return vec![];
    }

    let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exp_scores: Vec<f32> = scores.iter().map(|s| (s - max_score).exp()).collect();
    let sum: f32 = exp_scores.iter().sum();

    exp_scores.iter().map(|s| s / sum).collect()
}

/// Weighted vector combination: result = weight * value + accumulator
/// Use iteratively to apply attention weights
#[pg_extern(immutable, parallel_safe)]
pub fn attention_weighted_add(accumulator: Vec<f32>, value: Vec<f32>, weight: f32) -> Vec<f32> {
    if accumulator.len() != value.len() {
        pgrx::error!("Accumulator and value must have same dimensions");
    }
    accumulator
        .iter()
        .zip(value.iter())
        .map(|(a, v)| a + weight * v)
        .collect()
}

/// Initialize attention accumulator (zero vector)
#[pg_extern(immutable, parallel_safe)]
pub fn attention_init(dim: i32) -> Vec<f32> {
    vec![0.0f32; dim as usize]
}

/// Compute attention between query and single key-value pair
/// Returns weighted value: softmax_weight * value (for use with sum aggregate)
#[pg_extern(immutable, parallel_safe)]
pub fn attention_single(
    query: Vec<f32>,
    key: Vec<f32>,
    value: Vec<f32>,
    score_offset: f32,
) -> pgrx::JsonB {
    if query.len() != key.len() {
        pgrx::error!("Query and key must have same dimensions");
    }
    let dim = query.len();
    let scale = (dim as f32).sqrt();
    let raw_score: f32 = query
        .iter()
        .zip(key.iter())
        .map(|(q, k)| q * k)
        .sum::<f32>()
        / scale;

    pgrx::JsonB(serde_json::json!({
        "score": raw_score,
        "value": value,
        "score_offset": score_offset
    }))
}

// ============================================================================
// Graph Traversal Utilities (For Vector + Graph Hybrid Queries)
// ============================================================================

/// Compute edge similarity between two vectors (for graph edge weighting)
#[pg_extern(immutable, parallel_safe)]
pub fn graph_edge_similarity(source: Vec<f32>, target: Vec<f32>) -> f32 {
    if source.len() != target.len() {
        pgrx::error!("Vectors must have same dimensions");
    }
    1.0 - cosine_distance(&source, &target)
}

/// Compute PageRank contribution from a node to its neighbors
/// Returns contribution per neighbor: damping * importance / num_neighbors
#[pg_extern(immutable, parallel_safe)]
pub fn graph_pagerank_contribution(importance: f32, num_neighbors: i32, damping: f32) -> f32 {
    if num_neighbors <= 0 {
        return 0.0;
    }
    if damping < 0.0 || damping > 1.0 {
        pgrx::error!("Damping factor must be in [0, 1]");
    }
    damping * importance / (num_neighbors as f32)
}

/// Initialize PageRank base importance
#[pg_extern(immutable, parallel_safe)]
pub fn graph_pagerank_base(num_nodes: i32, damping: f32) -> f32 {
    if num_nodes <= 0 {
        pgrx::error!("Number of nodes must be positive");
    }
    if damping < 0.0 || damping > 1.0 {
        pgrx::error!("Damping factor must be in [0, 1]");
    }
    (1.0 - damping) / (num_nodes as f32)
}

/// Check if two vectors are semantically connected (similarity >= threshold)
#[pg_extern(immutable, parallel_safe)]
pub fn graph_is_connected(v1: Vec<f32>, v2: Vec<f32>, threshold: f32) -> bool {
    if v1.len() != v2.len() {
        pgrx::error!("Vectors must have same dimensions");
    }
    let sim = 1.0 - cosine_distance(&v1, &v2);
    sim >= threshold
}

/// Compute weighted centroid update (for graph-based clustering)
#[pg_extern(immutable, parallel_safe)]
pub fn graph_centroid_update(centroid: Vec<f32>, neighbor: Vec<f32>, weight: f32) -> Vec<f32> {
    if centroid.len() != neighbor.len() {
        pgrx::error!("Vectors must have same dimensions");
    }
    centroid
        .iter()
        .zip(neighbor.iter())
        .map(|(c, n)| c + weight * (n - c))
        .collect()
}

/// Compute bipartite matching score (for RAG graph queries)
#[pg_extern(immutable, parallel_safe)]
pub fn graph_bipartite_score(query: Vec<f32>, node: Vec<f32>, edge_weight: f32) -> f32 {
    if query.len() != node.len() {
        pgrx::error!("Vectors must have same dimensions");
    }
    let sim = 1.0 - cosine_distance(&query, &node);
    sim * edge_weight
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_l2_distance() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];
        let dist = l2_distance_arr(a, b);
        assert!((dist - 5.0).abs() < 1e-5);
    }

    #[pg_test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let dist = cosine_distance_arr(a, b);
        assert!(dist.abs() < 1e-5);
    }

    #[pg_test]
    fn test_inner_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let ip = inner_product_arr(a, b);
        assert!((ip - 32.0).abs() < 1e-5);
    }

    #[pg_test]
    fn test_vector_normalize() {
        let v = vec![3.0, 4.0];
        let n = vector_normalize(v);
        let norm: f32 = n.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[pg_test]
    fn test_l1_distance() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 6.0, 8.0];
        let dist = l1_distance_arr(a, b);
        // |4-1| + |6-2| + |8-3| = 3 + 4 + 5 = 12
        assert!((dist - 12.0).abs() < 1e-5);
    }

    #[pg_test]
    fn test_simd_various_sizes() {
        // Test various sizes to ensure SIMD remainder handling works
        for size in [1, 3, 7, 8, 15, 16, 31, 32, 63, 64, 127, 128, 256] {
            let a_data: Vec<f32> = (0..size).map(|i| i as f32).collect();
            let b_data: Vec<f32> = (0..size).map(|i| (i + 1) as f32).collect();

            let dist = l2_distance_arr(a_data, b_data);
            assert!(
                dist.is_finite() && dist > 0.0,
                "L2 distance failed for size {}",
                size
            );
        }
    }
}
