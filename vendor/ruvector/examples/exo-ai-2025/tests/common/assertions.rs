//! Custom assertions for integration tests
//!
//! Provides domain-specific assertions for cognitive substrate testing.

#![allow(dead_code)]

/// Assert two embeddings are approximately equal (within epsilon)
pub fn assert_embeddings_approx_equal(a: &[f32], b: &[f32], epsilon: f32) {
    assert_eq!(
        a.len(),
        b.len(),
        "Embeddings have different dimensions: {} vs {}",
        a.len(),
        b.len()
    );

    for (i, (av, bv)) in a.iter().zip(b.iter()).enumerate() {
        let diff = (av - bv).abs();
        assert!(
            diff < epsilon,
            "Embedding mismatch at index {}: |{} - {}| = {} >= {}",
            i,
            av,
            bv,
            diff,
            epsilon
        );
    }
}

/// Assert similarity scores are in descending order
pub fn assert_scores_descending(scores: &[f32]) {
    for window in scores.windows(2) {
        assert!(
            window[0] >= window[1],
            "Scores not in descending order: {} < {}",
            window[0],
            window[1]
        );
    }
}

/// Assert causal ordering is respected
pub fn assert_causal_order(results: &[String], expected_order: &[String]) {
    // TODO: Implement once CausalResult type exists
    // Verify results respect causal dependencies
    assert_eq!(results.len(), expected_order.len(), "Result count mismatch");
}

/// Assert CRDT states are convergent
pub fn assert_crdt_convergence(state1: &str, state2: &str) {
    // TODO: Implement once CRDT types exist
    // Verify eventual consistency
    assert_eq!(state1, state2, "CRDT states did not converge");
}

/// Assert topological invariants match expected values
pub fn assert_betti_numbers(betti: &[usize], expected: &[usize]) {
    assert_eq!(
        betti.len(),
        expected.len(),
        "Betti number dimension mismatch"
    );

    for (i, (actual, exp)) in betti.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            actual, exp,
            "Betti number b_{} mismatch: {} != {}",
            i, actual, exp
        );
    }
}

/// Assert consensus proof is valid
pub fn assert_valid_consensus_proof(proof: &str, threshold: usize) {
    // TODO: Implement once CommitProof type exists
    // Verify proof has sufficient signatures
    assert!(
        !proof.is_empty(),
        "Consensus proof is empty (need {} votes)",
        threshold
    );
}

/// Assert temporal ordering is consistent
pub fn assert_temporal_order(timestamps: &[u64]) {
    for window in timestamps.windows(2) {
        assert!(
            window[0] <= window[1],
            "Timestamps not in temporal order: {} > {}",
            window[0],
            window[1]
        );
    }
}

/// Assert pattern is within manifold region
pub fn assert_in_manifold_region(embedding: &[f32], center: &[f32], radius: f32) {
    let distance = euclidean_distance(embedding, center);
    assert!(
        distance <= radius,
        "Pattern outside manifold region: distance {} > radius {}",
        distance,
        radius
    );
}

// Helper: Compute Euclidean distance
fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .map(|(av, bv)| (av - bv).powi(2))
        .sum::<f32>()
        .sqrt()
}
