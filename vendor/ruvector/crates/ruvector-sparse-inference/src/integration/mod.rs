//! Integration modules for Ruvector and RuvLLM ecosystems
//!
//! This module provides seamless integration with the Ruvector vector database
//! and RuvLLM language model inference framework.

pub mod ruvector;
pub mod ruvllm;

pub use ruvector::SparseEmbeddingProvider;
pub use ruvllm::SparseInferenceBackend;
