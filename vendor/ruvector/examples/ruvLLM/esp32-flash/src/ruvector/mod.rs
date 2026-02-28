//! RuVector Integration for ESP32
//!
//! Vector database capabilities:
//! - Micro HNSW (1000+ vectors)
//! - Semantic memory with context
//! - RAG (Retrieval-Augmented Generation)
//! - Anomaly detection
//! - Federated search across chips

pub mod micro_hnsw;
pub mod semantic_memory;
pub mod rag;
pub mod anomaly;

pub use micro_hnsw::{MicroHNSW, HNSWConfig, SearchResult, INDEX_CAPACITY, MAX_LAYERS, DEFAULT_M};
pub use semantic_memory::{SemanticMemory, Memory, MemoryType, MAX_MEMORIES, MEMORY_DIM};
pub use rag::{MicroRAG, RAGConfig, RAGResult, MAX_KNOWLEDGE_ENTRIES};
pub use anomaly::{AnomalyDetector, AnomalyConfig, AnomalyResult};

use heapless::Vec as HVec;

pub const MAX_DIMENSIONS: usize = 128;
pub const MAX_VECTORS: usize = 1000;
pub const MAX_NEIGHBORS: usize = 16;

/// Quantized vector for ESP32
#[derive(Debug, Clone)]
pub struct MicroVector<const DIM: usize> {
    pub data: HVec<i8, DIM>,
    pub id: u32,
}

impl<const DIM: usize> MicroVector<DIM> {
    pub fn from_i8(data: &[i8], id: u32) -> Option<Self> {
        if data.len() > DIM { return None; }
        let mut vec = HVec::new();
        for &v in data { vec.push(v).ok()?; }
        Some(Self { data: vec, id })
    }

    pub fn from_f32(data: &[f32], id: u32) -> Option<Self> {
        if data.len() > DIM { return None; }
        let mut vec = HVec::new();
        for &v in data {
            let q = (v * 127.0).clamp(-128.0, 127.0) as i8;
            vec.push(q).ok()?;
        }
        Some(Self { data: vec, id })
    }

    pub fn dim(&self) -> usize { self.data.len() }
}

/// Distance metrics
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistanceMetric {
    Euclidean,
    Cosine,
    Manhattan,
    Hamming,
    DotProduct,
}

impl DistanceMetric {
    pub fn distance(&self, a: &[i8], b: &[i8]) -> i32 {
        match self {
            Self::Euclidean => euclidean_distance_i8(a, b),
            Self::Cosine => cosine_distance_i8(a, b),
            Self::Manhattan => manhattan_distance_i8(a, b),
            Self::Hamming => hamming_distance_i8(a, b),
            Self::DotProduct => -dot_product_i8(a, b),
        }
    }
}

pub fn euclidean_distance_i8(a: &[i8], b: &[i8]) -> i32 {
    a.iter().zip(b.iter()).map(|(&x, &y)| {
        let d = x as i32 - y as i32;
        d * d
    }).sum()
}

pub fn cosine_distance_i8(a: &[i8], b: &[i8]) -> i32 {
    let mut dot: i32 = 0;
    let mut norm_a: i32 = 0;
    let mut norm_b: i32 = 0;

    for (&x, &y) in a.iter().zip(b.iter()) {
        let xi = x as i32;
        let yi = y as i32;
        dot += xi * yi;
        norm_a += xi * xi;
        norm_b += yi * yi;
    }

    if norm_a == 0 || norm_b == 0 { return i32::MAX; }
    let norm_product = ((norm_a as i64) * (norm_b as i64)).min(i64::MAX);
    let norm_sqrt = isqrt(norm_product as u64) as i32;
    if norm_sqrt == 0 { return i32::MAX; }
    1000 - ((dot * 1000) / norm_sqrt)
}

pub fn manhattan_distance_i8(a: &[i8], b: &[i8]) -> i32 {
    a.iter().zip(b.iter()).map(|(&x, &y)| ((x as i32) - (y as i32)).abs()).sum()
}

pub fn hamming_distance_i8(a: &[i8], b: &[i8]) -> i32 {
    a.iter().zip(b.iter()).map(|(&x, &y)| (x ^ y).count_ones() as i32).sum()
}

pub fn dot_product_i8(a: &[i8], b: &[i8]) -> i32 {
    a.iter().zip(b.iter()).map(|(&x, &y)| (x as i32) * (y as i32)).sum()
}

fn isqrt(n: u64) -> u64 {
    if n == 0 { return 0; }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x { x = y; y = (x + n / x) / 2; }
    x
}
