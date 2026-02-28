//! Sparse vector distance functions optimized for sparse-sparse computations.

use super::types::SparseVec;
use std::cmp::Ordering;

/// Sparse dot product (inner product).
///
/// Efficiently computes the dot product by only iterating over
/// shared non-zero indices using merge-based iteration.
///
/// # Complexity
/// O(nnz(a) + nnz(b)) where nnz is the number of non-zero elements
///
/// # Example
/// ```ignore
/// let a = SparseVec::new(vec![0, 2, 5], vec![1.0, 2.0, 3.0], 10)?;
/// let b = SparseVec::new(vec![2, 3, 5], vec![4.0, 5.0, 6.0], 10)?;
/// let dot = sparse_dot(&a, &b); // 2*4 + 3*6 = 26
/// ```
#[inline]
pub fn sparse_dot(a: &SparseVec, b: &SparseVec) -> f32 {
    let mut result = 0.0;
    let mut i = 0;
    let mut j = 0;

    let a_indices = a.indices();
    let b_indices = b.indices();
    let a_values = a.values();
    let b_values = b.values();

    // Merge-based iteration: only multiply when indices match
    while i < a_indices.len() && j < b_indices.len() {
        match a_indices[i].cmp(&b_indices[j]) {
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
            Ordering::Equal => {
                result += a_values[i] * b_values[j];
                i += 1;
                j += 1;
            }
        }
    }

    result
}

/// Sparse cosine similarity.
///
/// Computes cosine similarity: dot(a, b) / (norm(a) * norm(b))
///
/// # Returns
/// Value in [-1, 1] where 1 means identical direction, -1 opposite, 0 orthogonal
///
/// # Example
/// ```ignore
/// let similarity = sparse_cosine(&a, &b);
/// ```
#[inline]
pub fn sparse_cosine(a: &SparseVec, b: &SparseVec) -> f32 {
    let dot = sparse_dot(a, b);
    let norm_a = a.norm();
    let norm_b = b.norm();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Sparse Euclidean distance (L2 distance).
///
/// Computes sqrt(sum((a_i - b_i)^2)) efficiently for sparse vectors.
/// Uses merge-based iteration to handle non-overlapping indices.
///
/// # Complexity
/// O(nnz(a) + nnz(b))
///
/// # Example
/// ```ignore
/// let distance = sparse_euclidean(&a, &b);
/// ```
#[inline]
pub fn sparse_euclidean(a: &SparseVec, b: &SparseVec) -> f32 {
    let mut result = 0.0;
    let mut i = 0;
    let mut j = 0;

    let a_indices = a.indices();
    let b_indices = b.indices();
    let a_values = a.values();
    let b_values = b.values();

    // Merge iteration handling all three cases:
    // - Only in a: contribute a_i^2
    // - Only in b: contribute b_j^2
    // - In both: contribute (a_i - b_j)^2
    while i < a_indices.len() || j < b_indices.len() {
        let idx_a = a_indices.get(i).copied().unwrap_or(u32::MAX);
        let idx_b = b_indices.get(j).copied().unwrap_or(u32::MAX);

        match idx_a.cmp(&idx_b) {
            Ordering::Less => {
                result += a_values[i] * a_values[i];
                i += 1;
            }
            Ordering::Greater => {
                result += b_values[j] * b_values[j];
                j += 1;
            }
            Ordering::Equal => {
                let diff = a_values[i] - b_values[j];
                result += diff * diff;
                i += 1;
                j += 1;
            }
        }
    }

    result.sqrt()
}

/// Sparse Manhattan distance (L1 distance).
///
/// Computes sum(|a_i - b_i|) efficiently for sparse vectors.
#[inline]
pub fn sparse_manhattan(a: &SparseVec, b: &SparseVec) -> f32 {
    let mut result = 0.0;
    let mut i = 0;
    let mut j = 0;

    let a_indices = a.indices();
    let b_indices = b.indices();
    let a_values = a.values();
    let b_values = b.values();

    while i < a_indices.len() || j < b_indices.len() {
        let idx_a = a_indices.get(i).copied().unwrap_or(u32::MAX);
        let idx_b = b_indices.get(j).copied().unwrap_or(u32::MAX);

        match idx_a.cmp(&idx_b) {
            Ordering::Less => {
                result += a_values[i].abs();
                i += 1;
            }
            Ordering::Greater => {
                result += b_values[j].abs();
                j += 1;
            }
            Ordering::Equal => {
                result += (a_values[i] - b_values[j]).abs();
                i += 1;
                j += 1;
            }
        }
    }

    result
}

/// BM25 scoring for sparse term vectors.
///
/// Implements BM25 ranking function commonly used in text search.
/// Query values should be IDF weights, document values should be term frequencies.
///
/// # Arguments
/// * `query` - Query sparse vector (IDF weights)
/// * `doc` - Document sparse vector (term frequencies)
/// * `doc_len` - Document length (number of terms)
/// * `avg_doc_len` - Average document length in collection
/// * `k1` - Term frequency saturation parameter (typically 1.2-2.0)
/// * `b` - Length normalization parameter (typically 0.75)
///
/// # Returns
/// BM25 score (higher is better)
#[inline]
pub fn sparse_bm25(
    query: &SparseVec,
    doc: &SparseVec,
    doc_len: f32,
    avg_doc_len: f32,
    k1: f32,
    b: f32,
) -> f32 {
    let mut score = 0.0;
    let mut i = 0;
    let mut j = 0;

    let q_indices = query.indices();
    let d_indices = doc.indices();
    let q_values = query.values();
    let d_values = doc.values();

    while i < q_indices.len() && j < d_indices.len() {
        match q_indices[i].cmp(&d_indices[j]) {
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
            Ordering::Equal => {
                let idf = q_values[i]; // Query values are IDF weights
                let tf = d_values[j]; // Doc values are term frequencies

                let numerator = tf * (k1 + 1.0);
                let denominator = tf + k1 * (1.0 - b + b * doc_len / avg_doc_len);

                score += idf * numerator / denominator;
                i += 1;
                j += 1;
            }
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_dot() {
        let a = SparseVec::new(vec![0, 2, 5], vec![1.0, 2.0, 3.0], 10).unwrap();
        let b = SparseVec::new(vec![2, 3, 5], vec![4.0, 5.0, 6.0], 10).unwrap();

        // Dot product: 2*4 + 3*6 = 8 + 18 = 26
        let dot = sparse_dot(&a, &b);
        assert!((dot - 26.0).abs() < 1e-5);
    }

    #[test]
    fn test_sparse_dot_no_overlap() {
        let a = SparseVec::new(vec![0, 1], vec![1.0, 2.0], 10).unwrap();
        let b = SparseVec::new(vec![3, 4], vec![3.0, 4.0], 10).unwrap();

        let dot = sparse_dot(&a, &b);
        assert_eq!(dot, 0.0);
    }

    #[test]
    fn test_sparse_cosine() {
        let a = SparseVec::new(vec![0, 1], vec![3.0, 4.0], 10).unwrap();
        let b = SparseVec::new(vec![0, 1], vec![3.0, 4.0], 10).unwrap();

        // Identical vectors should have cosine similarity 1.0
        let cos = sparse_cosine(&a, &b);
        assert!((cos - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_sparse_cosine_orthogonal() {
        let a = SparseVec::new(vec![0], vec![1.0], 10).unwrap();
        let b = SparseVec::new(vec![1], vec![1.0], 10).unwrap();

        // Orthogonal vectors should have cosine similarity 0.0
        let cos = sparse_cosine(&a, &b);
        assert_eq!(cos, 0.0);
    }

    #[test]
    fn test_sparse_euclidean() {
        let a = SparseVec::new(vec![0, 2], vec![0.0, 3.0], 10).unwrap();
        let b = SparseVec::new(vec![0, 2], vec![4.0, 0.0], 10).unwrap();

        // Distance: sqrt(16 + 9) = 5
        let dist = sparse_euclidean(&a, &b);
        assert!((dist - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_sparse_euclidean_different_indices() {
        let a = SparseVec::new(vec![0], vec![3.0], 10).unwrap();
        let b = SparseVec::new(vec![1], vec![4.0], 10).unwrap();

        // Distance: sqrt(9 + 16) = 5
        let dist = sparse_euclidean(&a, &b);
        assert!((dist - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_sparse_manhattan() {
        let a = SparseVec::new(vec![0, 2], vec![1.0, 3.0], 10).unwrap();
        let b = SparseVec::new(vec![0, 2], vec![4.0, 1.0], 10).unwrap();

        // Distance: |1-4| + |3-1| = 3 + 2 = 5
        let dist = sparse_manhattan(&a, &b);
        assert_eq!(dist, 5.0);
    }

    #[test]
    fn test_sparse_bm25() {
        // Query with IDF weights
        let query = SparseVec::new(vec![0, 2], vec![2.0, 3.0], 10).unwrap();
        // Document with term frequencies
        let doc = SparseVec::new(vec![0, 2], vec![1.0, 2.0], 10).unwrap();

        let score = sparse_bm25(&query, &doc, 10.0, 10.0, 1.2, 0.75);
        assert!(score > 0.0);
    }
}
