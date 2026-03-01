//! RuVector Integration for ESP32
//!
//! Brings vector database capabilities to microcontrollers:
//! - Micro HNSW index for similarity search
//! - Semantic memory for context-aware AI
//! - RAG (Retrieval-Augmented Generation)
//! - Anomaly detection via embedding distance
//! - Federated vector search across chip clusters
//!
//! # Memory Budget
//!
//! | Component | Size | Vectors |
//! |-----------|------|---------|
//! | Micro HNSW (64-dim, 100 vectors) | ~8 KB | 100 |
//! | Binary HNSW (64-dim, 1000 vectors) | ~10 KB | 1000 |
//! | Semantic Memory (50 memories) | ~4 KB | 50 |
//! | RAG Context Cache (10 docs) | ~2 KB | 10 |
//!
//! # Capabilities from RuVector
//!
//! - HNSW approximate nearest neighbor (adapted for fixed memory)
//! - Binary quantization (32x compression)
//! - Product quantization (8-64x compression)
//! - Cosine/Euclidean/Hamming distance
//! - Self-learning pattern recognition

pub mod micro_hnsw;
pub mod semantic_memory;
pub mod rag;
pub mod anomaly;
pub mod federated_search;

// Re-exports
pub use micro_hnsw::{MicroHNSW, HNSWConfig, SearchResult};
pub use semantic_memory::{SemanticMemory, Memory, MemoryType};
pub use rag::{MicroRAG, RAGConfig, RAGResult};
pub use anomaly::{AnomalyDetector, AnomalyConfig, AnomalyResult};
pub use federated_search::{FederatedIndex, ShardConfig};

use heapless::Vec as HVec;

/// Maximum dimensions for vectors on ESP32
pub const MAX_DIMENSIONS: usize = 128;
/// Maximum vectors in a single index
pub const MAX_VECTORS: usize = 1000;
/// Maximum neighbors per node in HNSW
pub const MAX_NEIGHBORS: usize = 16;

/// Quantized vector type for ESP32
#[derive(Debug, Clone)]
pub struct MicroVector<const DIM: usize> {
    /// INT8 quantized components
    pub data: HVec<i8, DIM>,
    /// Optional metadata ID
    pub id: u32,
}

impl<const DIM: usize> MicroVector<DIM> {
    /// Create from i8 slice
    pub fn from_i8(data: &[i8], id: u32) -> Option<Self> {
        if data.len() > DIM {
            return None;
        }
        let mut vec = HVec::new();
        for &v in data {
            vec.push(v).ok()?;
        }
        Some(Self { data: vec, id })
    }

    /// Create from f32 slice (quantizes to INT8)
    pub fn from_f32(data: &[f32], id: u32) -> Option<Self> {
        if data.len() > DIM {
            return None;
        }
        let mut vec = HVec::new();
        for &v in data {
            let quantized = (v * 127.0).clamp(-128.0, 127.0) as i8;
            vec.push(quantized).ok()?;
        }
        Some(Self { data: vec, id })
    }

    /// Dimension count
    pub fn dim(&self) -> usize {
        self.data.len()
    }
}

/// Distance metrics
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistanceMetric {
    /// Euclidean (L2) distance
    Euclidean,
    /// Cosine similarity (returned as 1 - cosine)
    Cosine,
    /// Manhattan (L1) distance
    Manhattan,
    /// Hamming distance (for binary vectors)
    Hamming,
    /// Dot product (for normalized vectors)
    DotProduct,
}

impl DistanceMetric {
    /// Calculate distance between two INT8 vectors
    pub fn distance(&self, a: &[i8], b: &[i8]) -> i32 {
        match self {
            Self::Euclidean => euclidean_distance_i8(a, b),
            Self::Cosine => cosine_distance_i8(a, b),
            Self::Manhattan => manhattan_distance_i8(a, b),
            Self::Hamming => hamming_distance_i8(a, b),
            Self::DotProduct => -dot_product_i8(a, b), // Negate for min-heap
        }
    }
}

/// INT8 Euclidean distance squared (avoids sqrt)
pub fn euclidean_distance_i8(a: &[i8], b: &[i8]) -> i32 {
    let mut sum: i32 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        let diff = (*x as i32) - (*y as i32);
        sum += diff * diff;
    }
    sum
}

/// INT8 Cosine distance (1 - similarity) scaled to i32
pub fn cosine_distance_i8(a: &[i8], b: &[i8]) -> i32 {
    let mut dot: i32 = 0;
    let mut norm_a: i32 = 0;
    let mut norm_b: i32 = 0;

    for (x, y) in a.iter().zip(b.iter()) {
        let xi = *x as i32;
        let yi = *y as i32;
        dot += xi * yi;
        norm_a += xi * xi;
        norm_b += yi * yi;
    }

    // Avoid division by zero
    if norm_a == 0 || norm_b == 0 {
        return i32::MAX;
    }

    // Return (1 - cosine) * 1000 for precision
    // cosine = dot / (sqrt(norm_a) * sqrt(norm_b))
    // Approximate with fixed-point: 1000 - (dot * 1000) / sqrt(norm_a * norm_b)
    let norm_product = ((norm_a as i64) * (norm_b as i64)).min(i64::MAX as i64);
    let norm_sqrt = isqrt(norm_product as u64) as i32;

    if norm_sqrt == 0 {
        return i32::MAX;
    }

    1000 - ((dot * 1000) / norm_sqrt)
}

/// INT8 Manhattan distance
pub fn manhattan_distance_i8(a: &[i8], b: &[i8]) -> i32 {
    let mut sum: i32 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        sum += ((*x as i32) - (*y as i32)).abs();
    }
    sum
}

/// Hamming distance (count differing bits)
pub fn hamming_distance_i8(a: &[i8], b: &[i8]) -> i32 {
    let mut count = 0i32;
    for (x, y) in a.iter().zip(b.iter()) {
        count += (*x ^ *y).count_ones() as i32;
    }
    count
}

/// INT8 dot product
pub fn dot_product_i8(a: &[i8], b: &[i8]) -> i32 {
    let mut sum: i32 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        sum += (*x as i32) * (*y as i32);
    }
    sum
}

/// Integer square root (no floating point)
fn isqrt(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_euclidean_distance() {
        let a = [10i8, 20, 30, 40];
        let b = [11i8, 21, 31, 41];
        let dist = euclidean_distance_i8(&a, &b);
        assert_eq!(dist, 4); // 1 + 1 + 1 + 1 = 4
    }

    #[test]
    fn test_micro_vector() {
        let data = [1i8, 2, 3, 4, 5, 6, 7, 8];
        let vec: MicroVector<16> = MicroVector::from_i8(&data, 42).unwrap();
        assert_eq!(vec.dim(), 8);
        assert_eq!(vec.id, 42);
    }

    #[test]
    fn test_cosine_distance() {
        // Same direction = 0 distance
        let a = [100i8, 0, 0, 0];
        let b = [50i8, 0, 0, 0];
        let dist = cosine_distance_i8(&a, &b);
        assert!(dist < 100); // Should be close to 0
    }
}
