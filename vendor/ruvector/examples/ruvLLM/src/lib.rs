//! # RuvLLM - Self-Learning LLM
//!
//! A self-learning language model system integrating LFM2 with Ruvector.
//!
//! ## Architecture
//!
//! The system is built on a three-layer architecture:
//!
//! - **LFM2** (Frozen core): Stable reasoning engine (350M-2.6B parameters)
//! - **Ruvector** (Living memory): Adaptive synaptic mesh that learns continuously
//! - **FastGRNN** (Control circuit): Intelligent router for resource allocation
//!
//! > "The intelligence is not in one model anymore. It is in the loop."
//!
//! ## Self-Learning Loops
//!
//! The system learns through three feedback loops:
//!
//! ### Loop A: Memory Growth & Refinement
//! - Every interaction writes to ruvector (Q&A, context, outcome)
//! - Graph edges strengthen/weaken based on success patterns
//! - Same LFM2 checkpoint → different answers over time
//!
//! ### Loop B: Router Learning
//! - FastGRNN learns optimal model selection
//! - Prefers cheaper routes when quality holds
//! - Escalates only when necessary
//!
//! ### Loop C: Compression & Abstraction
//! - Periodic summarization creates concept hierarchies
//! - Prevents unbounded memory growth
//! - Old nodes archived, concepts stay accessible
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ruvllm::{RuvLLM, Config};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = Config::builder()
//!         .db_path("./memory.db")
//!         .build()?;
//!
//!     let llm = RuvLLM::new(config).await?;
//!
//!     let response = llm.query("What is machine learning?").await?;
//!     println!("Response: {}", response.text);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Optimized Kernels (v2.0)
//!
//! Version 2.0 integrates the `ruvllm` crate for optimized inference:
//!
//! - **Flash Attention 2**: Tiled computation with online softmax (3-6x speedup)
//! - **NEON GEMM/GEMV**: M4 Pro optimized with 12x4 micro-kernels
//! - **Multi-threaded**: Parallel attention and matmul (4-6x speedup)
//! - **Quantized**: INT8/INT4/Q4K quantized inference
//!
//! ### Using Optimized Kernels
//!
//! ```rust,ignore
//! use ruvllm::kernels::{
//!     flash_attention_neon, gemm_neon, gemv_neon,
//!     AttentionConfig, is_neon_available,
//! };
//!
//! // Check NEON availability
//! if is_neon_available() {
//!     let output = flash_attention_neon(&query, &key, &value, scale, causal);
//! }
//! ```

#![warn(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(clippy::excessive_precision)]

pub mod attention;
pub mod compression;
pub mod config;
pub mod embedding;
pub mod error;
pub mod inference;
pub mod learning;
pub mod memory;
pub mod orchestrator;
pub mod router;
pub mod simd_inference;
pub mod sona;
pub mod training;
pub mod types;

#[cfg(feature = "real-inference")]
pub mod inference_real;

#[cfg(feature = "napi")]
pub mod napi;

// =============================================================================
// Re-exports from ruvllm for optimized kernels and backends
// =============================================================================

/// Optimized NEON/SIMD kernels from ruvllm.
///
/// Provides highly optimized kernels for LLM inference:
/// - Flash Attention 2 with online softmax
/// - GEMM/GEMV with 12x4 micro-kernels
/// - RMSNorm, LayerNorm
/// - RoPE (Rotary Position Embeddings)
/// - INT8/INT4/Q4K quantized inference
pub mod kernels {
    pub use ruvllm_lib::kernels::*;
}

/// LLM inference backends (Candle, mistral-rs).
pub mod backends {
    pub use ruvllm_lib::backends::*;
}

/// Two-tier KV cache with FP16 + quantized storage.
pub mod kv_cache {
    pub use ruvllm_lib::kv_cache::*;
}

/// Memory pool and arena allocators for inference.
pub mod memory_pool {
    pub use ruvllm_lib::memory_pool::*;
}

/// Speculative decoding for faster generation.
pub mod speculative {
    pub use ruvllm_lib::speculative::*;
}

/// LoRA adapter management and composition.
pub mod lora {
    pub use ruvllm_lib::lora::*;
}

// Re-export key types from ruvllm at crate root
pub use ruvllm_lib::{
    RuvLLMConfig as IntegrationConfig,
    RuvLLMEngine as IntegrationEngine,
    PagedAttention, PagedAttentionConfig, PageTable, PageBlock,
    TwoTierKvCache, KvCacheConfig, CacheTier,
    AdapterManager, LoraAdapter, AdapterConfig,
    SonaIntegration, SonaConfig as IntegrationSonaConfig, LearningLoop,
};

// Re-exports from local modules
pub use config::{Config, ConfigBuilder};
pub use error::{Error, Result};
pub use inference::{GenerationConfig, GenerationResult, InferenceMode, InferencePool};
pub use orchestrator::RuvLLM;
pub use simd_inference::{SimdGenerationConfig, SimdInferenceEngine, SimdOps};
pub use sona::{BackgroundLoop, InstantLoop, LoopCoordinator, SonaConfig};
pub use types::{Feedback, Request, Response, RoutingInfo, Session};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
