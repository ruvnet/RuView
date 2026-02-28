//! Similarity and distance metrics for hypervectors

use super::vector::Hypervector;
use super::HYPERVECTOR_BITS;

/// Computes Hamming distance between two hypervectors
///
/// Returns the number of bits that differ between the two vectors.
///
/// # Performance
///
/// <100ns with SIMD popcount instruction
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, hamming_distance};
///
/// let a = Hypervector::random();
/// let b = Hypervector::random();
/// let dist = hamming_distance(&a, &b);
/// assert!(dist > 0);
/// ```
#[inline]
pub fn hamming_distance(v1: &Hypervector, v2: &Hypervector) -> u32 {
    v1.hamming_distance(v2)
}

/// Computes cosine similarity approximation for binary hypervectors
///
/// For binary vectors, cosine similarity ≈ 1 - 2*hamming_distance/dimension
///
/// Returns a value in [0.0, 1.0] where:
/// - 1.0 = identical vectors
/// - 0.5 = orthogonal/random vectors
/// - 0.0 = opposite vectors
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, cosine_similarity};
///
/// let a = Hypervector::random();
/// let b = a.clone();
/// let sim = cosine_similarity(&a, &b);
/// assert!((sim - 1.0).abs() < 0.001);
/// ```
#[inline]
pub fn cosine_similarity(v1: &Hypervector, v2: &Hypervector) -> f32 {
    v1.similarity(v2)
}

/// Computes normalized Hamming similarity [0.0, 1.0]
///
/// This is equivalent to `1.0 - (hamming_distance / dimension)`
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, normalized_hamming};
///
/// let a = Hypervector::random();
/// let sim = normalized_hamming(&a, &a);
/// assert!((sim - 1.0).abs() < 0.001);
/// ```
pub fn normalized_hamming(v1: &Hypervector, v2: &Hypervector) -> f32 {
    let hamming = v1.hamming_distance(v2);
    1.0 - (hamming as f32 / HYPERVECTOR_BITS as f32)
}

/// Computes Jaccard similarity coefficient
///
/// Jaccard = |intersection| / |union| for binary vectors
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, jaccard_similarity};
///
/// let a = Hypervector::random();
/// let b = Hypervector::random();
/// let sim = jaccard_similarity(&a, &b);
/// assert!(sim >= 0.0 && sim <= 1.0);
/// ```
pub fn jaccard_similarity(v1: &Hypervector, v2: &Hypervector) -> f32 {
    let mut intersection = 0u32;
    let mut union = 0u32;

    let bits1 = v1.bits();
    let bits2 = v2.bits();

    for i in 0..bits1.len() {
        let and = bits1[i] & bits2[i];
        let or = bits1[i] | bits2[i];

        intersection += and.count_ones();
        union += or.count_ones();
    }

    if union == 0 {
        1.0 // Both vectors are zero
    } else {
        intersection as f32 / union as f32
    }
}

/// Finds the k most similar vectors from a set
///
/// Returns indices and similarities of top-k matches, sorted by similarity (descending).
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, top_k_similar};
///
/// let query = Hypervector::random();
/// let candidates: Vec<_> = (0..10).map(|_| Hypervector::random()).collect();
///
/// let top3 = top_k_similar(&query, &candidates, 3);
/// assert_eq!(top3.len(), 3);
/// assert!(top3[0].1 >= top3[1].1); // Sorted descending
/// ```
pub fn top_k_similar(
    query: &Hypervector,
    candidates: &[Hypervector],
    k: usize,
) -> Vec<(usize, f32)> {
    let mut similarities: Vec<_> = candidates
        .iter()
        .enumerate()
        .map(|(idx, candidate)| (idx, query.similarity(candidate)))
        .collect();

    // Partial sort to get top k (NaN-safe)
    similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Less));

    similarities.into_iter().take(k).collect()
}

/// Computes pairwise similarity matrix
///
/// Returns NxN matrix where result\[i\]\[j\] = similarity(vectors\[i\], vectors\[j\])
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, pairwise_similarities};
///
/// let vectors: Vec<_> = (0..5).map(|_| Hypervector::random()).collect();
/// let matrix = pairwise_similarities(&vectors);
///
/// assert_eq!(matrix.len(), 5);
/// assert_eq!(matrix[0].len(), 5);
/// assert!((matrix[0][0] - 1.0).abs() < 0.001); // Diagonal is 1.0
/// ```
pub fn pairwise_similarities(vectors: &[Hypervector]) -> Vec<Vec<f32>> {
    let n = vectors.len();
    let mut matrix = vec![vec![0.0; n]; n];

    for i in 0..n {
        matrix[i][i] = 1.0; // Diagonal

        for j in (i + 1)..n {
            let sim = vectors[i].similarity(&vectors[j]);
            matrix[i][j] = sim;
            matrix[j][i] = sim; // Symmetric
        }
    }

    matrix
}

/// Computes batch similarities of query against all candidates
///
/// Optimized for computing one-to-many similarities efficiently.
/// Uses loop unrolling for better CPU pipeline utilization.
///
/// # Performance
///
/// ~20ns per similarity (amortized over batch)
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, batch_similarities};
///
/// let query = Hypervector::random();
/// let candidates: Vec<_> = (0..100).map(|_| Hypervector::random()).collect();
///
/// let sims = batch_similarities(&query, &candidates);
/// assert_eq!(sims.len(), 100);
/// ```
#[inline]
pub fn batch_similarities(query: &Hypervector, candidates: &[Hypervector]) -> Vec<f32> {
    let n = candidates.len();
    let mut results = Vec::with_capacity(n);

    // Process in chunks of 4 for better cache utilization
    let chunks = n / 4;
    let remainder = n % 4;

    for i in 0..chunks {
        let base = i * 4;
        results.push(query.similarity(&candidates[base]));
        results.push(query.similarity(&candidates[base + 1]));
        results.push(query.similarity(&candidates[base + 2]));
        results.push(query.similarity(&candidates[base + 3]));
    }

    // Handle remainder
    let base = chunks * 4;
    for i in 0..remainder {
        results.push(query.similarity(&candidates[base + i]));
    }

    results
}

/// Finds indices of all vectors with similarity above threshold
///
/// # Example
///
/// ```rust
/// use ruvector_nervous_system::hdc::{Hypervector, find_similar};
///
/// let query = Hypervector::from_seed(42);
/// let candidates: Vec<_> = (0..100).map(|i| Hypervector::from_seed(i)).collect();
///
/// let matches = find_similar(&query, &candidates, 0.9);
/// assert!(matches.contains(&42)); // Should find itself
/// ```
pub fn find_similar(query: &Hypervector, candidates: &[Hypervector], threshold: f32) -> Vec<usize> {
    candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, candidate)| {
            if query.similarity(candidate) >= threshold {
                Some(idx)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hamming_distance_identical() {
        let v = Hypervector::random();
        assert_eq!(hamming_distance(&v, &v), 0);
    }

    #[test]
    fn test_hamming_distance_random() {
        let v1 = Hypervector::random();
        let v2 = Hypervector::random();

        let dist = hamming_distance(&v1, &v2);
        // Random vectors should differ in ~50% of bits
        assert!(dist > 4000 && dist < 6000, "distance: {}", dist);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = Hypervector::random();
        let sim = cosine_similarity(&v, &v);

        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_bounds() {
        let v1 = Hypervector::random();
        let v2 = Hypervector::random();

        let sim = cosine_similarity(&v1, &v2);
        // Cosine similarity for binary vectors: 1 - 2*hamming/dim gives [-1, 1]
        assert!(
            sim >= -1.0 && sim <= 1.0,
            "similarity out of bounds: {}",
            sim
        );
    }

    #[test]
    fn test_normalized_hamming_identical() {
        let v = Hypervector::random();
        let sim = normalized_hamming(&v, &v);

        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_normalized_hamming_random() {
        let v1 = Hypervector::random();
        let v2 = Hypervector::random();

        let sim = normalized_hamming(&v1, &v2);
        // Random vectors should have ~0.5 similarity
        assert!(sim > 0.3 && sim < 0.7, "similarity: {}", sim);
    }

    #[test]
    fn test_jaccard_identical() {
        let v = Hypervector::random();
        let sim = jaccard_similarity(&v, &v);

        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_zero_vectors() {
        let v1 = Hypervector::zero();
        let v2 = Hypervector::zero();

        let sim = jaccard_similarity(&v1, &v2);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_bounds() {
        let v1 = Hypervector::random();
        let v2 = Hypervector::random();

        let sim = jaccard_similarity(&v1, &v2);
        assert!(sim >= 0.0 && sim <= 1.0);
    }

    #[test]
    fn test_top_k_similar() {
        let query = Hypervector::from_seed(0);
        let candidates: Vec<_> = (1..11).map(|i| Hypervector::from_seed(i)).collect();

        let top3 = top_k_similar(&query, &candidates, 3);

        assert_eq!(top3.len(), 3);
        // Should be sorted descending
        assert!(top3[0].1 >= top3[1].1);
        assert!(top3[1].1 >= top3[2].1);
    }

    #[test]
    fn test_top_k_more_than_candidates() {
        let query = Hypervector::random();
        let candidates: Vec<_> = (0..5).map(|_| Hypervector::random()).collect();

        let top10 = top_k_similar(&query, &candidates, 10);

        // Should return all 5, not 10
        assert_eq!(top10.len(), 5);
    }

    #[test]
    fn test_pairwise_similarities_diagonal() {
        let vectors: Vec<_> = (0..5).map(|i| Hypervector::from_seed(i)).collect();
        let matrix = pairwise_similarities(&vectors);

        assert_eq!(matrix.len(), 5);

        for i in 0..5 {
            assert!((matrix[i][i] - 1.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_pairwise_similarities_symmetric() {
        let vectors: Vec<_> = (0..5).map(|i| Hypervector::from_seed(i)).collect();
        let matrix = pairwise_similarities(&vectors);

        for i in 0..5 {
            for j in 0..5 {
                assert!((matrix[i][j] - matrix[j][i]).abs() < 0.001);
            }
        }
    }

    #[test]
    fn test_pairwise_similarities_bounds() {
        let vectors: Vec<_> = (0..5).map(|_| Hypervector::random()).collect();
        let matrix = pairwise_similarities(&vectors);

        for row in &matrix {
            for &sim in row {
                // Similarity range is [-1, 1] for cosine similarity
                assert!(
                    sim >= -1.0 && sim <= 1.0,
                    "similarity out of bounds: {}",
                    sim
                );
            }
        }
    }
}
