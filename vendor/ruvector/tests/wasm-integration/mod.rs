//! WASM Integration Tests
//!
//! Comprehensive test suite for the new edge-net WASM crates:
//! - ruvector-attention-unified-wasm: Multi-head attention, Mamba SSM, etc.
//! - ruvector-learning-wasm: MicroLoRA, SONA adaptive learning
//! - ruvector-nervous-system-wasm: Bio-inspired neural components
//! - ruvector-economy-wasm: Economic mechanisms for agent coordination
//! - ruvector-exotic-wasm: NAOs, Morphogenetic Networks, Time Crystals
//!
//! These tests are designed to run in both Node.js and browser environments
//! using wasm-bindgen-test.

pub mod attention_unified_tests;
pub mod learning_tests;
pub mod nervous_system_tests;
pub mod economy_tests;
pub mod exotic_tests;

// Re-export common test utilities
pub mod common {
    use wasm_bindgen::prelude::*;

    /// Generate random f32 vector for testing
    pub fn random_vector(dim: usize) -> Vec<f32> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        dim.hash(&mut hasher);
        let seed = hasher.finish();

        (0..dim)
            .map(|i| {
                let x = ((seed.wrapping_mul(i as u64 + 1)) % 1000) as f32 / 1000.0;
                x * 2.0 - 1.0 // Range [-1, 1]
            })
            .collect()
    }

    /// Assert that two vectors are approximately equal
    pub fn assert_vectors_approx_eq(a: &[f32], b: &[f32], epsilon: f32) {
        assert_eq!(a.len(), b.len(), "Vector lengths must match");
        for (i, (&ai, &bi)) in a.iter().zip(b.iter()).enumerate() {
            assert!(
                (ai - bi).abs() < epsilon,
                "Vectors differ at index {}: {} vs {} (epsilon: {})",
                i, ai, bi, epsilon
            );
        }
    }

    /// Assert all values in a vector are finite (not NaN or Inf)
    pub fn assert_finite(v: &[f32]) {
        for (i, &x) in v.iter().enumerate() {
            assert!(x.is_finite(), "Value at index {} is not finite: {}", i, x);
        }
    }

    /// Assert vector values are within a given range
    pub fn assert_in_range(v: &[f32], min: f32, max: f32) {
        for (i, &x) in v.iter().enumerate() {
            assert!(
                x >= min && x <= max,
                "Value at index {} is out of range [{}, {}]: {}",
                i, min, max, x
            );
        }
    }

    /// Create a simple identity-like attention pattern for testing
    pub fn create_test_attention_pattern(seq_len: usize, dim: usize) -> (Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Vec<f32>>) {
        let queries: Vec<Vec<f32>> = (0..seq_len)
            .map(|i| {
                let mut v = vec![0.0; dim];
                if i < dim {
                    v[i] = 1.0;
                }
                v
            })
            .collect();

        let keys = queries.clone();
        let values = queries.clone();

        (queries, keys, values)
    }

    /// Softmax for verification
    pub fn softmax(v: &[f32]) -> Vec<f32> {
        let max = v.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = v.iter().map(|x| (x - max).exp()).sum();
        v.iter().map(|x| (x - max).exp() / exp_sum).collect()
    }

    /// Compute cosine similarity
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }
}
