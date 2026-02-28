//! Local embedding generation module for ruvector-postgres
//!
//! Provides text-to-embedding functionality using fastembed-rs (ONNX-based models).
//! Supports multiple embedding models with lazy loading and thread-safe caching.
//!
//! # Features
//!
//! - Local embedding generation (no external API calls)
//! - Multiple model support (MiniLM, BGE, MPNet)
//! - Lazy model loading (loads on first use)
//! - Thread-safe model cache
//! - Batch embedding for efficiency
//!
//! # SQL Functions
//!
//! ```sql
//! -- Generate embedding from text
//! SELECT ruvector_embed('Hello world');
//! SELECT ruvector_embed('Hello world', 'all-MiniLM-L6-v2');
//!
//! -- Batch embedding
//! SELECT ruvector_embed_batch(ARRAY['text1', 'text2']);
//!
//! -- List available models
//! SELECT * FROM ruvector_embedding_models();
//!
//! -- Model management
//! SELECT ruvector_load_model('all-MiniLM-L6-v2');
//! SELECT ruvector_model_info('all-MiniLM-L6-v2');
//! ```

mod cache;
mod functions;
mod models;

pub use cache::ModelCache;
pub use functions::*;
pub use models::{EmbeddingModel, ModelInfo};

/// Default embedding model
pub const DEFAULT_MODEL: &str = "all-MiniLM-L6-v2";

/// Maximum batch size for embedding generation
pub const MAX_BATCH_SIZE: usize = 256;

/// Maximum text length (in characters) for embedding
pub const MAX_TEXT_LENGTH: usize = 8192;
