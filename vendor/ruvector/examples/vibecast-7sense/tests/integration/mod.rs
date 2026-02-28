//! Integration tests for 7sense bioacoustics platform
//!
//! This module organizes integration tests across all six bounded contexts:
//! - Audio Ingestion Context
//! - Embedding Context
//! - Vector Space Context
//! - Learning Context (via Analysis)
//! - Analysis Context
//! - Interpretation Context
//!
//! Tests are organized by context and follow the domain-driven design boundaries.
//!
//! Note: Individual test files (audio_test.rs, etc.) are compiled as separate
//! test binaries via [[test]] entries in Cargo.toml.

// Re-export commonly used test utilities
pub use crate::fixtures::*;
pub use crate::mocks::*;

/// Common test configuration
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Sample rate for audio (32kHz for Perch 2.0)
    pub sample_rate: u32,
    /// Embedding dimensions (1536 for Perch 2.0)
    pub embedding_dims: usize,
    /// Default segment duration in ms
    pub segment_duration_ms: u64,
    /// HNSW M parameter
    pub hnsw_m: usize,
    /// HNSW ef_construction
    pub hnsw_ef_construction: usize,
    /// HNSW ef_search
    pub hnsw_ef_search: usize,
    /// Minimum cluster size for HDBSCAN
    pub min_cluster_size: usize,
    /// Target recall@10 for vector search
    pub target_recall_at_10: f32,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            sample_rate: 32000,
            embedding_dims: 1536,
            segment_duration_ms: 5000,
            hnsw_m: 16,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 100,
            min_cluster_size: 5,
            target_recall_at_10: 0.95,
        }
    }
}

impl TestConfig {
    /// Create config for fast tests (lower quality but faster)
    pub fn fast() -> Self {
        Self {
            hnsw_m: 8,
            hnsw_ef_construction: 50,
            hnsw_ef_search: 20,
            min_cluster_size: 3,
            target_recall_at_10: 0.90,
            ..Default::default()
        }
    }

    /// Create config for high-quality tests (slower but more accurate)
    pub fn high_quality() -> Self {
        Self {
            hnsw_m: 32,
            hnsw_ef_construction: 400,
            hnsw_ef_search: 200,
            min_cluster_size: 10,
            target_recall_at_10: 0.99,
            ..Default::default()
        }
    }
}

/// Shared test context that can be used across integration tests
pub struct IntegrationTestContext {
    pub config: TestConfig,
    pub recording_repo: MockRecordingRepository,
    pub segment_repo: MockSegmentRepository,
    pub embedding_repo: MockEmbeddingRepository,
    pub vector_index: MockVectorIndex,
    pub clustering_service: MockClusteringService,
    pub evidence_builder: MockEvidencePackBuilder,
    pub interpretation_generator: MockInterpretationGenerator,
}

impl Default for IntegrationTestContext {
    fn default() -> Self {
        Self::new()
    }
}

impl IntegrationTestContext {
    pub fn new() -> Self {
        let config = TestConfig::default();
        Self {
            config: config.clone(),
            recording_repo: MockRecordingRepository::new(),
            segment_repo: MockSegmentRepository::new(),
            embedding_repo: MockEmbeddingRepository::new(),
            vector_index: MockVectorIndex::with_config(HnswConfig {
                m: config.hnsw_m,
                ef_construction: config.hnsw_ef_construction,
                ef_search: config.hnsw_ef_search,
                max_layers: 6,
            }),
            clustering_service: MockClusteringService::with_params(config.min_cluster_size, 3),
            evidence_builder: MockEvidencePackBuilder::new(),
            interpretation_generator: MockInterpretationGenerator::new(),
        }
    }

    pub fn with_config(config: TestConfig) -> Self {
        Self {
            config: config.clone(),
            recording_repo: MockRecordingRepository::new(),
            segment_repo: MockSegmentRepository::new(),
            embedding_repo: MockEmbeddingRepository::new(),
            vector_index: MockVectorIndex::with_config(HnswConfig {
                m: config.hnsw_m,
                ef_construction: config.hnsw_ef_construction,
                ef_search: config.hnsw_ef_search,
                max_layers: 6,
            }),
            clustering_service: MockClusteringService::with_params(config.min_cluster_size, 3),
            evidence_builder: MockEvidencePackBuilder::new(),
            interpretation_generator: MockInterpretationGenerator::new(),
        }
    }

    /// Populate context with test data
    pub fn with_test_data(self, num_recordings: usize, segments_per_recording: usize) -> Self {
        for _ in 0..num_recordings {
            let recording = create_test_recording();
            let recording_id = recording.id;
            self.recording_repo.save(recording).unwrap();

            for i in 0..segments_per_recording {
                let start_ms = i as u64 * 5500;
                let segment = CallSegment {
                    id: SegmentId::new(),
                    recording_id,
                    start_ms,
                    end_ms: start_ms + 5000,
                    ..Default::default()
                };
                let segment_id = segment.id;
                self.segment_repo.save(segment).unwrap();
                self.recording_repo
                    .add_segment_link(recording_id, segment_id);

                let embedding = Embedding {
                    segment_id,
                    ..Default::default()
                };
                let embedding_id = embedding.id;
                let vector = embedding.vector.clone();
                self.embedding_repo.save(embedding).unwrap();
                self.vector_index.insert(embedding_id, vector).unwrap();
            }
        }
        self
    }
}

/// Helper macro for async test setup
#[macro_export]
macro_rules! setup_test {
    () => {
        IntegrationTestContext::new()
    };
    (fast) => {
        IntegrationTestContext::with_config(TestConfig::fast())
    };
    (high_quality) => {
        IntegrationTestContext::with_config(TestConfig::high_quality())
    };
    (populated) => {
        IntegrationTestContext::new().with_test_data(5, 10)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration_context_creation() {
        let ctx = IntegrationTestContext::new();
        assert_eq!(ctx.config.sample_rate, 32000);
        assert_eq!(ctx.config.embedding_dims, 1536);
    }

    #[test]
    fn test_context_with_test_data() {
        let ctx = IntegrationTestContext::new().with_test_data(2, 5);
        assert_eq!(ctx.recording_repo.count(), 2);
        assert_eq!(ctx.segment_repo.count(), 10);
        assert_eq!(ctx.embedding_repo.count(), 10);
        assert_eq!(ctx.vector_index.count(), 10);
    }

    #[test]
    fn test_config_variants() {
        let fast = TestConfig::fast();
        let hq = TestConfig::high_quality();

        assert!(fast.hnsw_m < hq.hnsw_m);
        assert!(fast.hnsw_ef_construction < hq.hnsw_ef_construction);
        assert!(fast.target_recall_at_10 < hq.target_recall_at_10);
    }
}
