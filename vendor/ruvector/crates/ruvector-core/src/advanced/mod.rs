//! # Advanced Techniques
//!
//! This module contains experimental and advanced features for next-generation vector search:
//! - **Hypergraphs**: n-ary relationships beyond pairwise similarity
//! - **Learned Indexes**: Neural network-based index structures
//! - **Neural Hashing**: Similarity-preserving binary projections
//! - **Topological Data Analysis**: Embedding quality assessment

pub mod hypergraph;
pub mod learned_index;
pub mod neural_hash;
pub mod tda;

pub use hypergraph::{CausalMemory, Hyperedge, HypergraphIndex, TemporalHyperedge};
pub use learned_index::{HybridIndex, LearnedIndex, RecursiveModelIndex};
pub use neural_hash::{DeepHashEmbedding, NeuralHash};
pub use tda::{EmbeddingQuality, TopologicalAnalyzer};
