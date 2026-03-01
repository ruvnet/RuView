//! Integration tests for Embedding Context
//!
//! Tests for ONNX model loading, embedding generation, L2 normalization,
//! quantization/dequantization, and batch embedding operations.

use vibecast_tests::fixtures::*;
use vibecast_tests::mocks::*;

// ============================================================================
// ONNX Model Loading Tests
// ============================================================================

mod model_loading {
    use super::*;

    #[test]
    fn test_model_version_configuration() {
        let model = ModelVersion::default();

        assert_eq!(model.name, "perch");
        assert_eq!(model.version, "2.0");
        assert_eq!(model.dimensions, 1536);
    }

    #[test]
    fn test_mock_model_adapter_creation() {
        let adapter = MockEmbeddingModelAdapter::new();
        let version = adapter.model_version();

        assert_eq!(version.name, "perch");
        assert_eq!(version.dimensions, 1536);
    }

    #[test]
    fn test_model_adapter_with_custom_dimensions() {
        let adapter = MockEmbeddingModelAdapter::new().with_dimensions(768);
        let version = adapter.model_version();

        assert_eq!(version.dimensions, 768);
    }

    #[test]
    fn test_model_output_dimensions_match_config() {
        let adapter = MockEmbeddingModelAdapter::new();
        let audio = create_test_audio_samples(5000, 32000);

        let embedding = adapter.embed(&audio).unwrap();
        let version = adapter.model_version();

        assert_eq!(embedding.len(), version.dimensions);
    }
}

// ============================================================================
// Embedding Generation Tests
// ============================================================================

mod embedding_generation {
    use super::*;

    #[test]
    fn test_generate_embedding_from_audio() {
        let adapter = MockEmbeddingModelAdapter::new();
        let audio = create_test_audio_samples(5000, 32000);

        let embedding = adapter.embed(&audio).unwrap();

        assert_eq!(embedding.len(), 1536);
        assert!(
            embedding.iter().all(|x| !x.is_nan() && !x.is_infinite()),
            "Embedding should not contain NaN or Inf"
        );
    }

    #[test]
    fn test_embedding_output_is_normalized() {
        let adapter = MockEmbeddingModelAdapter::new();
        let audio = create_test_audio_samples(5000, 32000);

        let embedding = adapter.embed(&audio).unwrap();

        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.0001,
            "Embedding should be L2-normalized, got norm {}",
            norm
        );
    }

    #[test]
    fn test_embedding_deterministic() {
        let adapter = MockEmbeddingModelAdapter::new();
        let audio = create_test_audio_samples(5000, 32000);

        let embedding1 = adapter.embed(&audio).unwrap();
        let embedding2 = adapter.embed(&audio).unwrap();

        assert_eq!(embedding1, embedding2, "Same input should produce same output");
    }

    #[test]
    fn test_different_audio_produces_different_embeddings() {
        let adapter = MockEmbeddingModelAdapter::new();

        let audio1 = create_test_audio_samples(5000, 32000);
        let audio2: Vec<f32> = audio1.iter().map(|x| x * 0.5).collect();

        let embedding1 = adapter.embed(&audio1).unwrap();
        let embedding2 = adapter.embed(&audio2).unwrap();

        let distance = cosine_distance(&embedding1, &embedding2);
        assert!(
            distance > 0.01,
            "Different audio should produce different embeddings"
        );
    }

    #[test]
    fn test_embedding_entity_creation() {
        let embedding = create_test_embedding();

        assert_eq!(embedding.vector.len(), 1536);
        assert!(embedding.norm > 0.0);
        assert_eq!(embedding.model_version.name, "perch");
    }

    #[test]
    fn test_embedding_with_specific_vector() {
        let vector = vec![1.0; 1536];
        let embedding = create_test_embedding_with_vector(vector.clone());

        assert_eq!(embedding.vector.len(), 1536);
        let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - embedding.norm).abs() < 0.0001);
    }
}

// ============================================================================
// L2 Normalization Tests
// ============================================================================

mod normalization {
    use super::*;

    #[test]
    fn test_l2_normalize_unit_vector() {
        let vector = vec![1.0, 0.0, 0.0];
        let normalized = l2_normalize(&vector);

        assert_eq!(normalized, vec![1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_l2_normalize_simple_vector() {
        let vector = vec![3.0, 4.0];
        let normalized = l2_normalize(&vector);

        assert!((normalized[0] - 0.6).abs() < 0.0001);
        assert!((normalized[1] - 0.8).abs() < 0.0001);
    }

    #[test]
    fn test_l2_normalize_preserves_direction() {
        let vector = vec![1.0, 2.0, 3.0, 4.0];
        let normalized = l2_normalize(&vector);

        // Check ratios are preserved
        let original_ratio = vector[1] / vector[0];
        let normalized_ratio = normalized[1] / normalized[0];
        assert!((original_ratio - normalized_ratio).abs() < 0.0001);
    }

    #[test]
    fn test_l2_normalize_high_dimensional() {
        let vector = create_random_vector(1536);
        let normalized = l2_normalize(&vector);

        assert_normalized(&normalized, 0.0001);
    }

    #[test]
    fn test_l2_normalize_zero_vector() {
        let vector = vec![0.0; 10];
        let normalized = l2_normalize(&vector);

        // Zero vector should remain zero
        assert!(normalized.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn test_l2_normalize_idempotent() {
        let vector = create_random_vector(1536);
        let normalized1 = l2_normalize(&vector);
        let normalized2 = l2_normalize(&normalized1);

        for (a, b) in normalized1.iter().zip(normalized2.iter()) {
            assert!((a - b).abs() < 0.0001);
        }
    }

    #[test]
    fn test_normalized_vector_creation() {
        let vector = create_normalized_vector(1536);
        assert_normalized(&vector, 0.0001);
    }

    #[test]
    fn test_batch_normalization() {
        let vectors: Vec<Vec<f32>> = (0..10).map(|i| create_deterministic_vector(1536, i)).collect();

        let normalized: Vec<Vec<f32>> = vectors.iter().map(|v| l2_normalize(v)).collect();

        for (i, norm_vec) in normalized.iter().enumerate() {
            assert_normalized(norm_vec, 0.0001);
        }
    }
}

// ============================================================================
// Quantization/Dequantization Tests
// ============================================================================

mod quantization {
    use super::*;

    /// Quantize f32 vector to i8 (scalar quantization)
    /// Uses symmetric quantization around the midpoint to properly utilize i8 range
    fn quantize_i8(vector: &[f32]) -> (Vec<i8>, f32, f32) {
        let min_val = vector.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_val = vector.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        // Use symmetric quantization: map [min, max] to [-127, 127]
        let scale = (max_val - min_val) / 254.0;  // 254 = 127 - (-127)
        let zero_point = (min_val + max_val) / 2.0;  // Midpoint of input range

        let quantized: Vec<i8> = vector
            .iter()
            .map(|v| {
                if scale == 0.0 {
                    0i8
                } else {
                    let scaled = ((v - zero_point) / scale).round();
                    (scaled as i16).clamp(-127, 127) as i8
                }
            })
            .collect();

        (quantized, scale, zero_point)
    }

    /// Dequantize i8 vector back to f32
    fn dequantize_i8(quantized: &[i8], scale: f32, zero_point: f32) -> Vec<f32> {
        quantized
            .iter()
            .map(|q| (*q as f32) * scale + zero_point)
            .collect()
    }

    #[test]
    fn test_quantization_roundtrip() {
        let original = create_random_vector(1536);
        let (quantized, scale, zero_point) = quantize_i8(&original);
        let dequantized = dequantize_i8(&quantized, scale, zero_point);

        assert_eq!(quantized.len(), original.len());
        assert_eq!(dequantized.len(), original.len());

        // Check reconstruction error is small
        let mse: f32 = original
            .iter()
            .zip(dequantized.iter())
            .map(|(o, d)| (o - d).powi(2))
            .sum::<f32>()
            / original.len() as f32;

        let rmse = mse.sqrt();
        assert!(
            rmse < 0.1,
            "Reconstruction RMSE {} is too high",
            rmse
        );
    }

    #[test]
    fn test_quantization_compression_ratio() {
        let original = create_random_vector(1536);
        let (quantized, _, _) = quantize_i8(&original);

        let original_bytes = original.len() * std::mem::size_of::<f32>();
        let quantized_bytes = quantized.len() * std::mem::size_of::<i8>();

        let compression_ratio = original_bytes as f32 / quantized_bytes as f32;
        assert!(
            compression_ratio >= 3.9,
            "i8 quantization should achieve ~4x compression, got {}x",
            compression_ratio
        );
    }

    #[test]
    fn test_quantization_preserves_relative_order() {
        let vector1 = vec![0.1, 0.5, 0.9];
        let vector2 = vec![0.2, 0.4, 0.8];

        let (q1, s1, z1) = quantize_i8(&vector1);
        let (q2, s2, z2) = quantize_i8(&vector2);

        // Verify relative ordering within vectors
        assert!(q1[0] < q1[1] && q1[1] < q1[2]);
        assert!(q2[0] < q2[1] && q2[1] < q2[2]);
    }

    #[test]
    fn test_quantization_similarity_preservation() {
        // Create two similar vectors
        let v1 = create_deterministic_vector(1536, 0);
        let v2: Vec<f32> = v1.iter().map(|x| x + 0.01).collect();
        let v2 = l2_normalize(&v2);
        let v1 = l2_normalize(&v1);

        let original_similarity = 1.0 - cosine_distance(&v1, &v2);

        // Quantize and compute similarity
        let (q1, s1, z1) = quantize_i8(&v1);
        let (q2, s2, z2) = quantize_i8(&v2);

        let d1 = dequantize_i8(&q1, s1, z1);
        let d2 = dequantize_i8(&q2, s2, z2);

        let quantized_similarity = 1.0 - cosine_distance(&d1, &d2);

        assert!(
            (original_similarity - quantized_similarity).abs() < 0.1,
            "Similarity should be preserved: original={}, quantized={}",
            original_similarity,
            quantized_similarity
        );
    }

    #[test]
    fn test_product_quantization_concept() {
        // Simulate product quantization by splitting vector into subvectors
        let vector = create_random_vector(1536);
        let num_subvectors = 48;
        let subvector_dim = 1536 / num_subvectors;

        let subvectors: Vec<&[f32]> = vector.chunks(subvector_dim).collect();
        assert_eq!(subvectors.len(), num_subvectors);

        // Each subvector can be quantized independently
        for subvec in &subvectors {
            let (quantized, _, _) = quantize_i8(subvec);
            assert_eq!(quantized.len(), subvector_dim);
        }
    }
}

// ============================================================================
// Batch Embedding Tests
// ============================================================================

mod batch_embedding {
    use super::*;

    #[test]
    fn test_batch_embed_multiple_segments() {
        let adapter = MockEmbeddingModelAdapter::new();

        let audio_batch: Vec<Vec<f32>> = (0..5)
            .map(|i| create_test_audio_samples(5000, 32000))
            .collect();

        let embeddings = adapter.embed_batch(&audio_batch).unwrap();

        assert_eq!(embeddings.len(), 5);
        for embedding in &embeddings {
            assert_eq!(embedding.len(), 1536);
            assert_normalized(embedding, 0.0001);
        }
    }

    #[test]
    fn test_batch_embedding_consistency() {
        let adapter = MockEmbeddingModelAdapter::new();
        let audio = create_test_audio_samples(5000, 32000);

        // Single embedding
        let single = adapter.embed(&audio).unwrap();

        // Batch embedding with same audio
        let batch = adapter.embed_batch(&vec![audio]).unwrap();

        assert_eq!(single, batch[0], "Single and batch should produce same result");
    }

    #[test]
    fn test_batch_embedding_performance_scaling() {
        let adapter = MockEmbeddingModelAdapter::new();

        // Test with increasing batch sizes
        let batch_sizes = vec![1, 10, 50, 100];

        for batch_size in batch_sizes {
            let audio_batch: Vec<Vec<f32>> = (0..batch_size)
                .map(|_| create_test_audio_samples(5000, 32000))
                .collect();

            let embeddings = adapter.embed_batch(&audio_batch).unwrap();
            assert_eq!(embeddings.len(), batch_size);
        }
    }

    #[test]
    fn test_batch_embedding_handles_empty_batch() {
        let adapter = MockEmbeddingModelAdapter::new();
        let empty_batch: Vec<Vec<f32>> = vec![];

        let embeddings = adapter.embed_batch(&empty_batch).unwrap();
        assert_eq!(embeddings.len(), 0);
    }

    #[test]
    fn test_embedding_batch_factory() {
        let embeddings = create_embedding_batch(10);

        assert_eq!(embeddings.len(), 10);
        for embedding in &embeddings {
            assert_eq!(embedding.vector.len(), 1536);
        }
    }

    #[test]
    fn test_similar_embeddings_factory() {
        let base = create_normalized_vector(1536);
        let similar = create_similar_embeddings(&base, 5, 0.1);

        assert_eq!(similar.len(), 5);

        // All should be similar to base
        for emb in &similar {
            let distance = cosine_distance(&base, &emb.vector);
            assert!(
                distance < 0.5,
                "Similar embedding should be close to base"
            );
        }
    }
}

// ============================================================================
// Embedding Repository Tests
// ============================================================================

mod repository {
    use super::*;

    #[test]
    fn test_embedding_repository_crud() {
        let repo = MockEmbeddingRepository::new();

        let embedding = create_test_embedding();
        let id = embedding.id;
        let segment_id = embedding.segment_id;

        // Create
        repo.save(embedding).unwrap();
        assert_eq!(repo.count(), 1);

        // Read by ID
        let found = repo.find_by_id(&id).unwrap().unwrap();
        assert_eq!(found.id, id);

        // Read by segment
        let by_segment = repo.find_by_segment(&segment_id).unwrap().unwrap();
        assert_eq!(by_segment.id, id);

        // Delete
        repo.delete(&id).unwrap();
        assert_eq!(repo.count(), 0);
    }

    #[test]
    fn test_embedding_repository_batch_save() {
        let repo = MockEmbeddingRepository::new();
        let embeddings = create_embedding_batch(10);

        repo.batch_save(embeddings).unwrap();
        assert_eq!(repo.count(), 10);
    }

    #[test]
    fn test_embedding_repository_find_by_model() {
        let repo = MockEmbeddingRepository::new();

        // Add embeddings with different models
        for i in 0..5 {
            let mut embedding = create_test_embedding();
            embedding.model_version.name = if i % 2 == 0 {
                "perch".to_string()
            } else {
                "birdnet".to_string()
            };
            repo.save(embedding).unwrap();
        }

        let perch_embeddings = repo.find_by_model("perch").unwrap();
        let birdnet_embeddings = repo.find_by_model("birdnet").unwrap();

        assert_eq!(perch_embeddings.len(), 3);
        assert_eq!(birdnet_embeddings.len(), 2);
    }

    #[test]
    fn test_embedding_repository_get_all_vectors() {
        let repo = MockEmbeddingRepository::new();
        let embeddings = create_embedding_batch(5);

        for emb in embeddings {
            repo.save(emb).unwrap();
        }

        let all_vectors = repo.get_all_vectors();
        assert_eq!(all_vectors.len(), 5);

        for (_, vector) in &all_vectors {
            assert_eq!(vector.len(), 1536);
        }
    }
}

// ============================================================================
// Model Error Handling Tests
// ============================================================================

mod error_handling {
    use super::*;

    #[test]
    fn test_model_with_failure_rate() {
        let adapter = MockEmbeddingModelAdapter::new().with_failure_rate(0.0);

        // Should succeed with 0% failure rate
        let audio = create_test_audio_samples(5000, 32000);
        let result = adapter.embed(&audio);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_vector_dimensions_detected() {
        let embedding = create_test_embedding_with_vector(vec![1.0; 768]); // Wrong dims
        assert_eq!(embedding.vector.len(), 768);
        assert_ne!(embedding.vector.len(), 1536); // Not Perch dimensions
    }

    #[test]
    fn test_embedding_validation() {
        let embeddings = create_embedding_batch(10);

        // All embeddings should be valid
        assert_valid_embeddings(&embeddings, 1536);
    }

    #[test]
    fn test_dimension_assertion() {
        let vector = create_random_vector(1536);
        assert_dimensions(&vector, 1536);
    }

    #[test]
    #[should_panic]
    fn test_dimension_assertion_fails_on_mismatch() {
        let vector = create_random_vector(768);
        assert_dimensions(&vector, 1536); // Should panic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_integration_smoke_test() {
        // Create adapter
        let adapter = MockEmbeddingModelAdapter::new();

        // Create audio
        let audio = create_test_audio_samples(5000, 32000);

        // Generate embedding
        let vector = adapter.embed(&audio).unwrap();

        // Verify properties
        assert_eq!(vector.len(), 1536);
        assert_normalized(&vector, 0.0001);

        // Store in repository
        let repo = MockEmbeddingRepository::new();
        let embedding = create_test_embedding_with_vector(vector);
        repo.save(embedding).unwrap();

        assert_eq!(repo.count(), 1);
    }
}
