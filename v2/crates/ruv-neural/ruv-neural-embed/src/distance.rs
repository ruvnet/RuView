//! Distance metrics for neural embeddings.
//!
//! Provides cosine similarity, Euclidean distance, k-nearest-neighbor search,
//! and a DTW-inspired trajectory distance for comparing embedding sequences.

use ruv_neural_core::embedding::{EmbeddingTrajectory, NeuralEmbedding};

/// Cosine similarity between two embeddings.
///
/// Returns a value in [-1, 1] where 1 means identical direction, 0 means
/// orthogonal, and -1 means opposite.
///
/// Returns 0.0 if either embedding has zero norm.
pub fn cosine_similarity(a: &NeuralEmbedding, b: &NeuralEmbedding) -> f64 {
    let len = a.vector.len().min(b.vector.len());
    if len == 0 {
        return 0.0;
    }

    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for i in 0..len {
        dot += a.vector[i] * b.vector[i];
        norm_a += a.vector[i] * a.vector[i];
        norm_b += b.vector[i] * b.vector[i];
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom < 1e-12 {
        return 0.0;
    }

    dot / denom
}

/// Euclidean (L2) distance between two embeddings.
///
/// If the embeddings have different dimensions, only the overlapping
/// portion is compared.
pub fn euclidean_distance(a: &NeuralEmbedding, b: &NeuralEmbedding) -> f64 {
    let len = a.vector.len().min(b.vector.len());
    if len == 0 {
        return 0.0;
    }

    let mut sum_sq = 0.0;
    for i in 0..len {
        let diff = a.vector[i] - b.vector[i];
        sum_sq += diff * diff;
    }

    sum_sq.sqrt()
}

/// Manhattan (L1) distance between two embeddings.
pub fn manhattan_distance(a: &NeuralEmbedding, b: &NeuralEmbedding) -> f64 {
    let len = a.vector.len().min(b.vector.len());
    let mut sum = 0.0;
    for i in 0..len {
        sum += (a.vector[i] - b.vector[i]).abs();
    }
    sum
}

/// Find the k nearest neighbors to a query embedding.
///
/// Returns a vector of `(index, distance)` tuples sorted by ascending
/// Euclidean distance. `index` refers to the position in `candidates`.
pub fn k_nearest(
    query: &NeuralEmbedding,
    candidates: &[NeuralEmbedding],
    k: usize,
) -> Vec<(usize, f64)> {
    let mut distances: Vec<(usize, f64)> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| (i, euclidean_distance(query, c)))
        .collect();

    distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    distances.truncate(k);
    distances
}

/// Dynamic Time Warping (DTW) distance between two embedding trajectories.
///
/// Measures the cost of aligning two temporal sequences of embeddings,
/// allowing for non-linear time warping. The cost at each cell is the
/// Euclidean distance between the corresponding embeddings.
pub fn trajectory_distance(a: &EmbeddingTrajectory, b: &EmbeddingTrajectory) -> f64 {
    let n = a.embeddings.len();
    let m = b.embeddings.len();

    if n == 0 || m == 0 {
        return f64::INFINITY;
    }

    let mut dtw = vec![vec![f64::INFINITY; m + 1]; n + 1];
    dtw[0][0] = 0.0;

    for i in 1..=n {
        for j in 1..=m {
            let cost = euclidean_distance(&a.embeddings[i - 1], &b.embeddings[j - 1]);
            dtw[i][j] = cost
                + dtw[i - 1][j]
                    .min(dtw[i][j - 1])
                    .min(dtw[i - 1][j - 1]);
        }
    }

    dtw[n][m]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_metadata;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::embedding::NeuralEmbedding;

    fn emb(values: Vec<f64>) -> NeuralEmbedding {
        let meta = default_metadata("test", Atlas::Custom(1));
        NeuralEmbedding::new(values, 0.0, meta).unwrap()
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = emb(vec![1.0, 2.0, 3.0]);
        let b = emb(vec![1.0, 2.0, 3.0]);
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim - 1.0).abs() < 1e-10,
            "Identical embeddings: cos sim should be 1.0"
        );
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = emb(vec![1.0, 0.0]);
        let b = emb(vec![0.0, 1.0]);
        let sim = cosine_similarity(&a, &b);
        assert!(
            sim.abs() < 1e-10,
            "Orthogonal embeddings: cos sim should be 0.0"
        );
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = emb(vec![1.0, 2.0]);
        let b = emb(vec![-1.0, -2.0]);
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim + 1.0).abs() < 1e-10,
            "Opposite embeddings: cos sim should be -1.0"
        );
    }

    #[test]
    fn test_euclidean_distance_identical() {
        let a = emb(vec![1.0, 2.0, 3.0]);
        let b = emb(vec![1.0, 2.0, 3.0]);
        let dist = euclidean_distance(&a, &b);
        assert!(
            dist.abs() < 1e-10,
            "Identical embeddings: distance should be 0.0"
        );
    }

    #[test]
    fn test_euclidean_distance_known() {
        let a = emb(vec![0.0, 0.0]);
        let b = emb(vec![3.0, 4.0]);
        let dist = euclidean_distance(&a, &b);
        assert!((dist - 5.0).abs() < 1e-10, "Distance should be 5.0");
    }

    #[test]
    fn test_k_nearest_returns_correct() {
        let query = emb(vec![0.0, 0.0]);
        let candidates = vec![
            emb(vec![10.0, 10.0]),
            emb(vec![1.0, 0.0]),
            emb(vec![5.0, 5.0]),
            emb(vec![0.5, 0.5]),
        ];

        let nearest = k_nearest(&query, &candidates, 2);
        assert_eq!(nearest.len(), 2);
        assert_eq!(nearest[0].0, 3);
        assert_eq!(nearest[1].0, 1);
    }

    #[test]
    fn test_k_nearest_k_larger_than_candidates() {
        let query = emb(vec![0.0]);
        let candidates = vec![emb(vec![1.0]), emb(vec![2.0])];
        let nearest = k_nearest(&query, &candidates, 10);
        assert_eq!(nearest.len(), 2);
    }

    #[test]
    fn test_trajectory_distance_identical() {
        let traj = EmbeddingTrajectory {
            embeddings: vec![emb(vec![1.0, 2.0]), emb(vec![3.0, 4.0])],
            timestamps: vec![0.0, 0.5],
        };
        let dist = trajectory_distance(&traj, &traj);
        assert!(
            dist.abs() < 1e-10,
            "Identical trajectories: DTW distance should be 0.0"
        );
    }

    #[test]
    fn test_trajectory_distance_different() {
        let a = EmbeddingTrajectory {
            embeddings: vec![emb(vec![0.0, 0.0]), emb(vec![1.0, 0.0])],
            timestamps: vec![0.0, 0.5],
        };
        let b = EmbeddingTrajectory {
            embeddings: vec![emb(vec![0.0, 0.0]), emb(vec![0.0, 1.0])],
            timestamps: vec![0.0, 0.5],
        };
        let dist = trajectory_distance(&a, &b);
        assert!(
            dist > 0.0,
            "Different trajectories should have non-zero DTW distance"
        );
    }

    #[test]
    fn test_trajectory_distance_empty() {
        let a = EmbeddingTrajectory {
            embeddings: vec![],
            timestamps: vec![],
        };
        let b = EmbeddingTrajectory {
            embeddings: vec![emb(vec![1.0])],
            timestamps: vec![0.0],
        };
        let dist = trajectory_distance(&a, &b);
        assert!(dist.is_infinite());
    }
}
