//! # RuVector ONNX Embeddings
//!
//! A reimagined embedding pipeline for RuVector using ONNX Runtime in pure Rust.
//!
//! This crate provides:
//! - Native ONNX model inference for embedding generation
//! - HuggingFace tokenizer integration
//! - Batch processing with SIMD optimization
//! - Direct RuVector vector database integration
//! - Model management and caching
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    RuVector ONNX Embeddings                      │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
//! │  │   Text Input │ -> │  Tokenizer   │ -> │ Token IDs    │       │
//! │  └──────────────┘    └──────────────┘    └──────────────┘       │
//! │                                                 │                │
//! │                                                 v                │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
//! │  │  Embeddings  │ <- │ ONNX Runtime │ <- │ Input Tensor │       │
//! │  └──────────────┘    └──────────────┘    └──────────────┘       │
//! │         │                                                        │
//! │         v                                                        │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
//! │  │   Normalize  │ -> │ Mean Pooling │ -> │  RuVector DB │       │
//! │  └──────────────┘    └──────────────┘    └──────────────┘       │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvector_onnx_embeddings::{Embedder, EmbedderConfig, ModelSource};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create embedder with default model (all-MiniLM-L6-v2)
//!     let embedder = Embedder::new(EmbedderConfig::default()).await?;
//!
//!     // Generate embeddings
//!     let texts = vec!["Hello, world!", "Rust is awesome!"];
//!     let embeddings = embedder.embed(&texts)?;
//!
//!     // Use with RuVector
//!     let db = embedder.create_ruvector_index("my_index")?;
//!     db.insert_with_embeddings(&texts, &embeddings)?;
//!
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod embedder;
pub mod error;
pub mod model;
pub mod pooling;
pub mod ruvector_integration;
pub mod tokenizer;

/// GPU acceleration module (optional, requires `gpu` feature)
#[cfg(feature = "gpu")]
pub mod gpu;

/// GPU module stub for when feature is disabled
#[cfg(not(feature = "gpu"))]
pub mod gpu {
    //! GPU acceleration is not available without the `gpu` feature.
    //!
    //! Enable with: `cargo build --features gpu`

    /// Placeholder for GpuConfig when GPU feature is disabled
    #[derive(Debug, Clone, Default)]
    pub struct GpuConfig;

    impl GpuConfig {
        /// Create default config (no-op without GPU feature)
        pub fn auto() -> Self { Self }
        /// CPU-only config
        pub fn cpu_only() -> Self { Self }
    }

    /// Check if GPU is available (always false without feature)
    pub async fn is_gpu_available() -> bool { false }
}

// Re-exports
pub use config::{EmbedderConfig, ModelSource, PoolingStrategy};
pub use embedder::{Embedder, EmbedderBuilder, EmbeddingOutput};
pub use error::{EmbeddingError, Result};
pub use model::{OnnxModel, ModelInfo};
pub use pooling::Pooler;
pub use ruvector_integration::{
    Distance, IndexConfig, RagPipeline, RuVectorBuilder, RuVectorEmbeddings, SearchResult, VectorId,
};
pub use tokenizer::Tokenizer;

// GPU exports (conditional)
#[cfg(feature = "gpu")]
pub use gpu::{
    GpuAccelerator, GpuConfig, GpuMode, GpuInfo, GpuBackend,
    HybridAccelerator, is_gpu_available,
};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        Distance, Embedder, EmbedderBuilder, EmbedderConfig, EmbeddingError,
        IndexConfig, ModelSource, PoolingStrategy, RagPipeline, Result,
        RuVectorBuilder, RuVectorEmbeddings, SearchResult, VectorId,
    };
}

/// Supported embedding models with pre-configured settings
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PretrainedModel {
    /// all-MiniLM-L6-v2: 384 dimensions, fast inference
    #[default]
    AllMiniLmL6V2,
    /// all-MiniLM-L12-v2: 384 dimensions, better quality
    AllMiniLmL12V2,
    /// all-mpnet-base-v2: 768 dimensions, high quality
    AllMpnetBaseV2,
    /// multi-qa-MiniLM-L6: 384 dimensions, optimized for QA
    MultiQaMiniLmL6,
    /// paraphrase-MiniLM-L6-v2: 384 dimensions, paraphrase detection
    ParaphraseMiniLmL6V2,
    /// BGE-small-en-v1.5: 384 dimensions, BAAI General Embeddings
    BgeSmallEnV15,
    /// E5-small-v2: 384 dimensions, Microsoft E5 model
    E5SmallV2,
    /// GTE-small: 384 dimensions, Alibaba GTE model
    GteSmall,
}

impl PretrainedModel {
    /// Get the HuggingFace model ID
    pub fn model_id(&self) -> &'static str {
        match self {
            Self::AllMiniLmL6V2 => "sentence-transformers/all-MiniLM-L6-v2",
            Self::AllMiniLmL12V2 => "sentence-transformers/all-MiniLM-L12-v2",
            Self::AllMpnetBaseV2 => "sentence-transformers/all-mpnet-base-v2",
            Self::MultiQaMiniLmL6 => "sentence-transformers/multi-qa-MiniLM-L6-cos-v1",
            Self::ParaphraseMiniLmL6V2 => "sentence-transformers/paraphrase-MiniLM-L6-v2",
            Self::BgeSmallEnV15 => "BAAI/bge-small-en-v1.5",
            Self::E5SmallV2 => "intfloat/e5-small-v2",
            Self::GteSmall => "thenlper/gte-small",
        }
    }

    /// Get the embedding dimension
    pub fn dimension(&self) -> usize {
        match self {
            Self::AllMiniLmL6V2
            | Self::AllMiniLmL12V2
            | Self::MultiQaMiniLmL6
            | Self::ParaphraseMiniLmL6V2
            | Self::BgeSmallEnV15
            | Self::E5SmallV2
            | Self::GteSmall => 384,
            Self::AllMpnetBaseV2 => 768,
        }
    }

    /// Get recommended max sequence length
    pub fn max_seq_length(&self) -> usize {
        match self {
            Self::AllMiniLmL6V2
            | Self::AllMiniLmL12V2
            | Self::MultiQaMiniLmL6
            | Self::ParaphraseMiniLmL6V2 => 256,
            Self::AllMpnetBaseV2 => 384,
            Self::BgeSmallEnV15 | Self::E5SmallV2 | Self::GteSmall => 512,
        }
    }

    /// Whether the model requires normalized outputs
    pub fn normalize_output(&self) -> bool {
        true
    }
}

