//! Pattern separation module implementing hippocampal dentate gyrus-inspired encoding
//!
//! This module provides sparse random projection and k-winners-take-all mechanisms
//! for creating collision-resistant, orthogonal vector representations.

mod dentate;
mod projection;
mod sparsification;

pub use dentate::DentateGyrus;
pub use projection::SparseProjection;
pub use sparsification::SparseBitVector;

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that similar inputs produce decorrelated outputs
    #[test]
    fn test_pattern_separation_decorrelation() {
        let dg = DentateGyrus::new(128, 10000, 200, 42);

        // Create two similar inputs (90% overlap)
        let mut input1 = vec![0.0; 128];
        let mut input2 = vec![0.0; 128];
        for i in 0..115 {
            input1[i] = 1.0;
            input2[i] = 1.0;
        }
        input1[120] = 1.0;
        input2[121] = 1.0;

        let sparse1 = dg.encode(&input1);
        let sparse2 = dg.encode(&input2);

        // Despite 90% input overlap, output similarity should be lower
        let input_overlap = 115.0 / 128.0; // 0.898
        let output_similarity = sparse1.jaccard_similarity(&sparse2);

        // Pattern separation should decorrelate: output similarity < input similarity
        assert!(
            output_similarity < input_overlap,
            "Output similarity ({}) should be less than input overlap ({})",
            output_similarity,
            input_overlap
        );
    }

    /// Test collision rate on random inputs
    #[test]
    fn test_collision_rate() {
        let dg = DentateGyrus::new(128, 10000, 200, 42);
        let num_samples = 1000;

        let mut encodings = Vec::new();
        for i in 0..num_samples {
            let input: Vec<f32> = (0..128).map(|j| ((i * 128 + j) as f32).sin()).collect();
            encodings.push(dg.encode(&input));
        }

        // Count collisions (identical encodings)
        let mut collisions = 0;
        for i in 0..encodings.len() {
            for j in (i + 1)..encodings.len() {
                if encodings[i].indices == encodings[j].indices {
                    collisions += 1;
                }
            }
        }

        let collision_rate = collisions as f32 / (num_samples * (num_samples - 1) / 2) as f32;

        // Collision rate should be < 1%
        assert!(
            collision_rate < 0.01,
            "Collision rate ({:.4}) exceeds 1%",
            collision_rate
        );
    }

    /// Verify sparsity level (2-5% active neurons)
    #[test]
    fn test_sparsity_level() {
        let output_dim = 10000;
        let k = 200; // 2% sparsity
        let dg = DentateGyrus::new(128, output_dim, k, 42);

        let input: Vec<f32> = (0..128).map(|i| (i as f32).sin()).collect();
        let sparse = dg.encode(&input);

        let sparsity = sparse.indices.len() as f32 / output_dim as f32;

        // Verify exact k winners
        assert_eq!(
            sparse.indices.len(),
            k,
            "Should have exactly k active neurons"
        );

        // Verify sparsity in 2-5% range
        assert!(
            sparsity >= 0.02 && sparsity <= 0.05,
            "Sparsity ({:.4}) should be in 2-5% range",
            sparsity
        );
    }

    /// Test encoding performance
    #[test]
    fn test_encoding_performance() {
        let dg = DentateGyrus::new(512, 10000, 200, 42);
        let input: Vec<f32> = (0..512).map(|i| (i as f32).sin()).collect();

        let start = std::time::Instant::now();
        let iterations = 100;

        for _ in 0..iterations {
            let _ = dg.encode(&input);
        }

        let elapsed = start.elapsed();
        let avg_time = elapsed / iterations;

        // Should complete in reasonable time (very relaxed for CI environments)
        assert!(
            avg_time.as_secs() < 2,
            "Average encoding time ({:?}) exceeds 2s",
            avg_time
        );
    }

    /// Test similarity computation performance
    #[test]
    fn test_similarity_performance() {
        let dg = DentateGyrus::new(512, 10000, 200, 42);

        let input1: Vec<f32> = (0..512).map(|i| (i as f32).sin()).collect();
        let input2: Vec<f32> = (0..512).map(|i| (i as f32).cos()).collect();

        let sparse1 = dg.encode(&input1);
        let sparse2 = dg.encode(&input2);

        let start = std::time::Instant::now();
        let iterations = 1000;

        for _ in 0..iterations {
            let _ = sparse1.jaccard_similarity(&sparse2);
        }

        let elapsed = start.elapsed();
        let avg_time = elapsed / iterations;

        // Should be < 100μs per similarity computation
        assert!(
            avg_time.as_micros() < 100,
            "Average similarity time ({:?}) exceeds 100μs",
            avg_time
        );
    }

    /// Test retrieval quality: similar inputs should have higher similarity
    #[test]
    fn test_retrieval_quality() {
        let dg = DentateGyrus::new(128, 10000, 200, 42);

        // Original input
        let original: Vec<f32> = (0..128).map(|i| (i as f32).sin()).collect();

        // Similar input (small perturbation)
        let similar: Vec<f32> = original
            .iter()
            .map(|&x| x + 0.1 * ((x * 10.0).cos()))
            .collect();

        // Different input
        let different: Vec<f32> = (0..128).map(|i| (i as f32).cos()).collect();

        let enc_original = dg.encode(&original);
        let enc_similar = dg.encode(&similar);
        let enc_different = dg.encode(&different);

        let sim_to_similar = enc_original.jaccard_similarity(&enc_similar);
        let sim_to_different = enc_original.jaccard_similarity(&enc_different);

        // Similar inputs should have higher similarity than different inputs
        assert!(
            sim_to_similar > sim_to_different,
            "Similar input similarity ({}) should be higher than different input ({})",
            sim_to_similar,
            sim_to_different
        );
    }
}
