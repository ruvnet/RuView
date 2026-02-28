//! Stage II Encoder: Sensory Data via Multi-Head Attention Vectors
//!
//! CRV Stage II captures sensory impressions (textures, colors, temperatures,
//! sounds, etc.). Each sensory modality is encoded as a separate attention head,
//! with the multi-head mechanism combining them into a unified 384-dimensional
//! representation.
//!
//! # Architecture
//!
//! Sensory descriptors are hashed into feature vectors per modality, then
//! processed through multi-head attention where each head specializes in
//! a different sensory channel.

use crate::error::{CrvError, CrvResult};
use crate::types::{CrvConfig, SensoryModality, StageIIData};
use ruvector_attention::traits::Attention;
use ruvector_attention::MultiHeadAttention;

/// Number of sensory modality heads.
const NUM_MODALITIES: usize = 8;

/// Stage II encoder using multi-head attention for sensory fusion.
pub struct StageIIEncoder {
    /// Embedding dimensionality.
    dim: usize,
    /// Multi-head attention mechanism (one head per modality).
    attention: MultiHeadAttention,
}

impl std::fmt::Debug for StageIIEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StageIIEncoder")
            .field("dim", &self.dim)
            .field("attention", &"MultiHeadAttention { .. }")
            .finish()
    }
}

impl StageIIEncoder {
    /// Create a new Stage II encoder.
    pub fn new(config: &CrvConfig) -> Self {
        let dim = config.dimensions;
        // Ensure dim is divisible by NUM_MODALITIES
        let effective_heads = if dim % NUM_MODALITIES == 0 {
            NUM_MODALITIES
        } else {
            // Fall back to a divisor
            let mut h = NUM_MODALITIES;
            while dim % h != 0 && h > 1 {
                h -= 1;
            }
            h
        };

        let attention = MultiHeadAttention::new(dim, effective_heads);

        Self { dim, attention }
    }

    /// Encode a sensory descriptor string into a feature vector.
    ///
    /// Uses a deterministic hash-based encoding to convert text descriptors
    /// into fixed-dimension vectors. Each modality gets a distinct subspace.
    fn encode_descriptor(&self, modality: SensoryModality, descriptor: &str) -> Vec<f32> {
        let mut features = vec![0.0f32; self.dim];
        let modality_offset = modality_index(modality) * (self.dim / NUM_MODALITIES.max(1));
        let subspace_size = self.dim / NUM_MODALITIES.max(1);

        // Simple deterministic hash encoding
        let bytes = descriptor.as_bytes();
        for (i, &byte) in bytes.iter().enumerate() {
            let dim_idx = modality_offset + (i % subspace_size);
            if dim_idx < self.dim {
                // Distribute byte values across the subspace with varied phases
                let phase = (i as f32) * 0.618_034; // golden ratio
                features[dim_idx] += (byte as f32 / 255.0) * (phase * std::f32::consts::PI).cos();
            }
        }

        // Add modality-specific bias
        if modality_offset < self.dim {
            features[modality_offset] += 1.0;
        }

        // L2 normalize
        let norm: f32 = features.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-6 {
            for f in &mut features {
                *f /= norm;
            }
        }

        features
    }

    /// Encode Stage II data into a unified sensory embedding.
    ///
    /// Each sensory impression becomes a key-value pair in the attention
    /// mechanism. A learned query (based on the modality distribution)
    /// attends over all impressions to produce the fused output.
    pub fn encode(&self, data: &StageIIData) -> CrvResult<Vec<f32>> {
        if data.impressions.is_empty() {
            return Err(CrvError::EmptyInput("No sensory impressions".to_string()));
        }

        // If a pre-computed feature vector exists, use it
        if let Some(ref fv) = data.feature_vector {
            if fv.len() == self.dim {
                return Ok(fv.clone());
            }
        }

        // Encode each impression into a feature vector
        let encoded: Vec<Vec<f32>> = data
            .impressions
            .iter()
            .map(|(modality, descriptor)| self.encode_descriptor(*modality, descriptor))
            .collect();

        // Build query from modality distribution
        let query = self.build_modality_query(&data.impressions);

        let keys: Vec<&[f32]> = encoded.iter().map(|v| v.as_slice()).collect();
        let values: Vec<&[f32]> = encoded.iter().map(|v| v.as_slice()).collect();

        let result = self.attention.compute(&query, &keys, &values)?;
        Ok(result)
    }

    /// Build a query vector from the distribution of modalities present.
    fn build_modality_query(&self, impressions: &[(SensoryModality, String)]) -> Vec<f32> {
        let mut query = vec![0.0f32; self.dim];
        let subspace_size = self.dim / NUM_MODALITIES.max(1);

        // Count modality occurrences
        let mut counts = [0usize; NUM_MODALITIES];
        for (modality, _) in impressions {
            let idx = modality_index(*modality);
            if idx < NUM_MODALITIES {
                counts[idx] += 1;
            }
        }

        // Encode counts as the query
        let total: f32 = counts.iter().sum::<usize>() as f32;
        for (m, &count) in counts.iter().enumerate() {
            let weight = count as f32 / total.max(1.0);
            let offset = m * subspace_size;
            for d in 0..subspace_size.min(self.dim - offset) {
                query[offset + d] = weight * (1.0 + d as f32 * 0.01);
            }
        }

        // L2 normalize
        let norm: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-6 {
            for f in &mut query {
                *f /= norm;
            }
        }

        query
    }

    /// Compute similarity between two Stage II embeddings.
    pub fn similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a < 1e-6 || norm_b < 1e-6 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }
}

/// Map sensory modality to index.
fn modality_index(m: SensoryModality) -> usize {
    match m {
        SensoryModality::Texture => 0,
        SensoryModality::Color => 1,
        SensoryModality::Temperature => 2,
        SensoryModality::Sound => 3,
        SensoryModality::Smell => 4,
        SensoryModality::Taste => 5,
        SensoryModality::Dimension => 6,
        SensoryModality::Luminosity => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CrvConfig {
        CrvConfig {
            dimensions: 32, // 32 / 8 = 4 dims per head
            ..CrvConfig::default()
        }
    }

    #[test]
    fn test_encoder_creation() {
        let config = test_config();
        let encoder = StageIIEncoder::new(&config);
        assert_eq!(encoder.dim, 32);
    }

    #[test]
    fn test_descriptor_encoding() {
        let config = test_config();
        let encoder = StageIIEncoder::new(&config);

        let v = encoder.encode_descriptor(SensoryModality::Texture, "rough grainy");
        assert_eq!(v.len(), 32);

        // Should be normalized
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_full_encode() {
        let config = test_config();
        let encoder = StageIIEncoder::new(&config);

        let data = StageIIData {
            impressions: vec![
                (SensoryModality::Texture, "rough".to_string()),
                (SensoryModality::Color, "blue-gray".to_string()),
                (SensoryModality::Temperature, "cold".to_string()),
            ],
            feature_vector: None,
        };

        let embedding = encoder.encode(&data).unwrap();
        assert_eq!(embedding.len(), 32);
    }

    #[test]
    fn test_similarity() {
        let config = test_config();
        let encoder = StageIIEncoder::new(&config);

        let a = vec![1.0; 32];
        let b = vec![1.0; 32];
        let sim = encoder.similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_empty_impressions() {
        let config = test_config();
        let encoder = StageIIEncoder::new(&config);

        let data = StageIIData {
            impressions: vec![],
            feature_vector: None,
        };

        assert!(encoder.encode(&data).is_err());
    }
}
