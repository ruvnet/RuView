//! Test library for 7sense bioacoustics platform integration tests
//!
//! This library provides test fixtures, mocks, and integration tests for
//! all six bounded contexts of the 7sense system:
//!
//! - **Audio Ingestion Context**: Audio file loading, resampling, segmentation
//! - **Embedding Context**: Perch 2.0 model integration, vector generation
//! - **Vector Space Context**: HNSW index operations, k-NN search
//! - **Analysis Context**: HDBSCAN clustering, motif detection, entropy
//! - **Interpretation Context**: RAB evidence packs, citation validation
//! - **API Context**: REST endpoints, GraphQL, rate limiting
//!
//! ## Test Organization
//!
//! - `fixtures/` - Test data factories and builders
//! - `mocks/` - Mock implementations of repositories and services
//! - `integration/` - Integration tests organized by bounded context
//!
//! ## Usage
//!
//! Run all tests with:
//! ```bash
//! cargo test -p vibecast-tests
//! ```
//!
//! Run specific context tests:
//! ```bash
//! cargo test -p vibecast-tests --test audio_test
//! cargo test -p vibecast-tests --test vector_test
//! ```

pub mod fixtures;
pub mod mocks;
pub mod integration;

// Re-export commonly used types for convenience
pub use fixtures::*;
pub use mocks::*;
pub use integration::{IntegrationTestContext, TestConfig};

/// Version of the test suite
pub const TEST_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Target recall@10 for HNSW index tests (from ADR requirements)
pub const TARGET_RECALL_AT_10: f32 = 0.95;

/// Perch 2.0 embedding dimensions
pub const PERCH_EMBEDDING_DIMS: usize = 1536;

/// Required sample rate for audio processing
pub const REQUIRED_SAMPLE_RATE: u32 = 32000;

/// Mel spectrogram dimensions (frames x mel bins)
pub const MEL_FRAMES: usize = 500;
pub const MEL_BINS: usize = 128;

/// HNSW default parameters
pub const HNSW_M: usize = 16;
pub const HNSW_EF_CONSTRUCTION: usize = 200;
pub const HNSW_EF_SEARCH: usize = 100;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_match_requirements() {
        // Verify test constants match ADR requirements
        assert_eq!(PERCH_EMBEDDING_DIMS, 1536, "Perch 2.0 uses 1536-D embeddings");
        assert_eq!(REQUIRED_SAMPLE_RATE, 32000, "Perch 2.0 requires 32kHz audio");
        assert_eq!(MEL_FRAMES, 500, "Spectrogram should have 500 frames");
        assert_eq!(MEL_BINS, 128, "Spectrogram should have 128 mel bins");
        assert!(TARGET_RECALL_AT_10 >= 0.95, "Recall@10 must be >= 0.95");
    }

    #[test]
    fn test_hnsw_params_match_defaults() {
        let config = HnswConfig::default();
        assert_eq!(config.m, HNSW_M);
        assert_eq!(config.ef_construction, HNSW_EF_CONSTRUCTION);
        assert_eq!(config.ef_search, HNSW_EF_SEARCH);
    }
}
