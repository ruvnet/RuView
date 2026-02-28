//! Advanced Optimizations from Ruvector
//!
//! This module brings key optimizations from the ruvector ecosystem to ESP32:
//! - Binary quantization (32x compression)
//! - Product quantization (8-32x compression)
//! - Hamming distance with POPCNT
//! - Fixed-point softmax with lookup tables
//! - MicroLoRA for on-device adaptation
//! - Sparse attention patterns
//! - MinCut-inspired layer pruning

pub mod binary_quant;
pub mod product_quant;
pub mod lookup_tables;
pub mod micro_lora;
pub mod sparse_attention;
pub mod pruning;

// Re-exports
pub use binary_quant::{BinaryVector, BinaryEmbedding, hamming_distance, hamming_similarity};
pub use product_quant::{ProductQuantizer, PQCode};
pub use lookup_tables::{SoftmaxLUT, ExpLUT, DistanceLUT};
pub use micro_lora::{MicroLoRA, LoRAConfig};
pub use sparse_attention::{SparseAttention, AttentionPattern};
pub use pruning::{LayerPruner, PruningConfig};
