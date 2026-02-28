//! Feature engineering for candidate scoring
//!
//! Combines semantic similarity, recency, frequency, and other metrics

use crate::error::{Result, TinyDancerError};
use crate::types::Candidate;
use chrono::Utc;
use simsimd::SpatialSimilarity;

/// Feature vector for a candidate
#[derive(Debug, Clone)]
pub struct FeatureVector {
    /// Semantic similarity score (0.0 to 1.0)
    pub semantic_similarity: f32,
    /// Recency score (0.0 to 1.0)
    pub recency_score: f32,
    /// Frequency score (0.0 to 1.0)
    pub frequency_score: f32,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f32,
    /// Metadata overlap score (0.0 to 1.0)
    pub metadata_overlap: f32,
    /// Combined feature vector
    pub features: Vec<f32>,
}

/// Feature engineering configuration
#[derive(Debug, Clone)]
pub struct FeatureConfig {
    /// Weight for semantic similarity (default: 0.4)
    pub similarity_weight: f32,
    /// Weight for recency (default: 0.2)
    pub recency_weight: f32,
    /// Weight for frequency (default: 0.15)
    pub frequency_weight: f32,
    /// Weight for success rate (default: 0.15)
    pub success_weight: f32,
    /// Weight for metadata overlap (default: 0.1)
    pub metadata_weight: f32,
    /// Decay factor for recency (default: 0.001)
    pub recency_decay: f32,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            similarity_weight: 0.4,
            recency_weight: 0.2,
            frequency_weight: 0.15,
            success_weight: 0.15,
            metadata_weight: 0.1,
            recency_decay: 0.001,
        }
    }
}

/// Feature engineering for candidate scoring
pub struct FeatureEngineer {
    config: FeatureConfig,
}

impl FeatureEngineer {
    /// Create a new feature engineer with default configuration
    pub fn new() -> Self {
        Self {
            config: FeatureConfig::default(),
        }
    }

    /// Create a new feature engineer with custom configuration
    pub fn with_config(config: FeatureConfig) -> Self {
        Self { config }
    }

    /// Extract features from a candidate
    pub fn extract_features(
        &self,
        query_embedding: &[f32],
        candidate: &Candidate,
        query_metadata: Option<&std::collections::HashMap<String, serde_json::Value>>,
    ) -> Result<FeatureVector> {
        // 1. Semantic similarity (cosine similarity)
        let semantic_similarity = self.cosine_similarity(query_embedding, &candidate.embedding)?;

        // 2. Recency score (exponential decay)
        let recency_score = self.recency_score(candidate.created_at);

        // 3. Frequency score (normalized access count)
        let frequency_score = self.frequency_score(candidate.access_count);

        // 4. Success rate (direct from candidate)
        let success_rate = candidate.success_rate;

        // 5. Metadata overlap
        let metadata_overlap = if let Some(query_meta) = query_metadata {
            self.metadata_overlap_score(query_meta, &candidate.metadata)
        } else {
            0.0
        };

        // Combine features into a weighted vector
        let features = vec![
            semantic_similarity * self.config.similarity_weight,
            recency_score * self.config.recency_weight,
            frequency_score * self.config.frequency_weight,
            success_rate * self.config.success_weight,
            metadata_overlap * self.config.metadata_weight,
        ];

        Ok(FeatureVector {
            semantic_similarity,
            recency_score,
            frequency_score,
            success_rate,
            metadata_overlap,
            features,
        })
    }

    /// Extract features for a batch of candidates
    pub fn extract_batch_features(
        &self,
        query_embedding: &[f32],
        candidates: &[Candidate],
        query_metadata: Option<&std::collections::HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<FeatureVector>> {
        candidates
            .iter()
            .map(|candidate| self.extract_features(query_embedding, candidate, query_metadata))
            .collect()
    }

    /// Compute cosine similarity using SIMD-optimized simsimd
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> Result<f32> {
        if a.len() != b.len() {
            return Err(TinyDancerError::InvalidInput(format!(
                "Vector dimension mismatch: {} vs {}",
                a.len(),
                b.len()
            )));
        }

        // Use simsimd for SIMD-accelerated cosine similarity
        let similarity = f32::cosine(a, b)
            .ok_or_else(|| TinyDancerError::FeatureError("Cosine similarity failed".to_string()))?;

        // Convert distance to similarity (simsimd returns distance as f64)
        Ok(1.0_f32 - similarity as f32)
    }

    /// Calculate recency score using exponential decay
    fn recency_score(&self, created_at: i64) -> f32 {
        let now = Utc::now().timestamp();
        let age_seconds = (now - created_at).max(0) as f32;

        // Exponential decay: score = exp(-λ * age)
        (-self.config.recency_decay * age_seconds).exp()
    }

    /// Calculate frequency score (normalized)
    fn frequency_score(&self, access_count: u64) -> f32 {
        // Use logarithmic scaling for frequency
        // score = log(1 + count) / log(1 + max_expected)
        let max_expected = 10000.0_f32; // Expected maximum access count
        ((1.0 + access_count as f32).ln() / (1.0 + max_expected).ln()).min(1.0)
    }

    /// Calculate metadata overlap score
    fn metadata_overlap_score(
        &self,
        query_metadata: &std::collections::HashMap<String, serde_json::Value>,
        candidate_metadata: &std::collections::HashMap<String, serde_json::Value>,
    ) -> f32 {
        if query_metadata.is_empty() || candidate_metadata.is_empty() {
            return 0.0;
        }

        let mut matches = 0;
        let total = query_metadata.len();

        for (key, value) in query_metadata {
            if let Some(candidate_value) = candidate_metadata.get(key) {
                if value == candidate_value {
                    matches += 1;
                }
            }
        }

        matches as f32 / total as f32
    }

    /// Get the configuration
    pub fn config(&self) -> &FeatureConfig {
        &self.config
    }
}

impl Default for FeatureEngineer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_feature_extraction() {
        let engineer = FeatureEngineer::new();
        let query = vec![1.0, 0.0, 0.0];
        let candidate = Candidate {
            id: "test".to_string(),
            embedding: vec![0.9, 0.1, 0.0],
            metadata: HashMap::new(),
            created_at: Utc::now().timestamp(),
            access_count: 10,
            success_rate: 0.95,
        };

        let features = engineer.extract_features(&query, &candidate, None).unwrap();
        assert!(features.semantic_similarity > 0.8);
        assert!(features.recency_score > 0.9);
    }

    #[test]
    fn test_cosine_similarity() {
        let engineer = FeatureEngineer::new();
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let similarity = engineer.cosine_similarity(&a, &b).unwrap();
        assert!((similarity - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_recency_score() {
        let engineer = FeatureEngineer::new();
        let now = Utc::now().timestamp();
        let score_recent = engineer.recency_score(now);
        let score_old = engineer.recency_score(now - 86400); // 1 day ago
        assert!(score_recent > score_old);
    }
}
