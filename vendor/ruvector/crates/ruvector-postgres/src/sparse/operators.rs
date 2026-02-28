//! PostgreSQL operators and functions for sparse vectors.

use super::distance::{sparse_bm25, sparse_cosine, sparse_dot, sparse_euclidean, sparse_manhattan};
use super::types::SparseVec;
use pgrx::prelude::*;

// ============================================================================
// Distance Functions
// ============================================================================

/// Sparse dot product (inner product) operator.
///
/// Returns the dot product of two sparse vectors.
/// Only non-zero elements are multiplied, making this very efficient for sparse data.
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_sparse_dot(
///     '{1:0.5, 2:0.3}'::sparsevec,
///     '{2:0.4, 3:0.2}'::sparsevec
/// );
/// -- Returns: 0.12 (only index 2 overlaps: 0.3 * 0.4)
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_sparse_dot")]
fn pg_sparse_dot(a: SparseVec, b: SparseVec) -> f32 {
    sparse_dot(&a, &b)
}

/// Sparse cosine similarity operator.
///
/// Returns the cosine similarity between two sparse vectors.
/// Result is in [-1, 1] where 1 means identical direction.
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_sparse_cosine(
///     '{1:0.5, 2:0.3}'::sparsevec,
///     '{1:0.5, 2:0.3}'::sparsevec
/// );
/// -- Returns: 1.0 (identical vectors)
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_sparse_cosine")]
fn pg_sparse_cosine(a: SparseVec, b: SparseVec) -> f32 {
    sparse_cosine(&a, &b)
}

/// Sparse Euclidean distance operator.
///
/// Returns the L2 distance between two sparse vectors.
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_sparse_euclidean(
///     '{0:3.0}'::sparsevec,
///     '{1:4.0}'::sparsevec
/// );
/// -- Returns: 5.0 (sqrt(3^2 + 4^2))
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_sparse_euclidean")]
fn pg_sparse_euclidean(a: SparseVec, b: SparseVec) -> f32 {
    sparse_euclidean(&a, &b)
}

/// Sparse Manhattan distance operator (L1 distance).
///
/// Returns the L1 distance between two sparse vectors.
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_sparse_manhattan(
///     '{0:1.0, 2:3.0}'::sparsevec,
///     '{0:4.0, 2:1.0}'::sparsevec
/// );
/// -- Returns: 5.0 (|1-4| + |3-1|)
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_sparse_manhattan")]
fn pg_sparse_manhattan(a: SparseVec, b: SparseVec) -> f32 {
    sparse_manhattan(&a, &b)
}

// ============================================================================
// Construction Functions
// ============================================================================

/// Create a sparse vector from arrays of indices and values.
///
/// # Arguments
/// * `indices` - Array of non-zero indices
/// * `values` - Array of values corresponding to indices
/// * `dim` - Total dimensionality of the vector
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_to_sparse(
///     ARRAY[1024, 2048, 4096]::int[],
///     ARRAY[0.5, 0.3, 0.8]::real[],
///     30000
/// );
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_to_sparse")]
fn pg_to_sparse(indices: Vec<i32>, values: Vec<f32>, dim: i32) -> SparseVec {
    let indices: Vec<u32> = indices.into_iter().map(|i| i as u32).collect();
    SparseVec::new(indices, values, dim as u32)
        .unwrap_or_else(|e| panic!("Failed to create sparse vector: {}", e))
}

/// Get the number of non-zero elements in a sparse vector.
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_sparse_nnz('{1:0.5, 2:0.3, 5:0.8}'::sparsevec);
/// -- Returns: 3
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_sparse_nnz")]
fn pg_sparse_nnz(sparse: SparseVec) -> i32 {
    sparse.nnz() as i32
}

/// Get the dimensionality of a sparse vector.
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_sparse_dim('{1:0.5, 2:0.3}'::sparsevec);
/// -- Returns: 3 (max index + 1)
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_sparse_dim")]
fn pg_sparse_dim(sparse: SparseVec) -> i32 {
    sparse.dim() as i32
}

/// Get the L2 norm of a sparse vector.
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_sparse_norm('{0:3.0, 1:4.0}'::sparsevec);
/// -- Returns: 5.0 (sqrt(9 + 16))
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_sparse_norm")]
fn pg_sparse_norm(sparse: SparseVec) -> f32 {
    sparse.norm()
}

// ============================================================================
// Sparsification Functions
// ============================================================================

/// Keep only the top-k elements by absolute value.
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_sparse_top_k(
///     '{0:0.1, 1:0.5, 2:0.05, 3:0.8}'::sparsevec,
///     2
/// );
/// -- Returns: {1:0.5, 3:0.8}
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_sparse_top_k")]
fn pg_sparse_top_k(sparse: SparseVec, k: i32) -> SparseVec {
    sparse.top_k(k as usize)
}

/// Prune elements below a threshold.
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_sparse_prune(
///     '{0:0.1, 1:0.5, 2:0.05, 3:0.8}'::sparsevec,
///     0.2
/// );
/// -- Returns: {1:0.5, 3:0.8}
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_sparse_prune")]
fn pg_sparse_prune(sparse: SparseVec, threshold: f32) -> SparseVec {
    let mut result = sparse;
    result.prune(threshold);
    result
}

/// Convert a dense vector (array) to sparse representation.
///
/// Only non-zero elements are kept. Useful for converting existing
/// dense embeddings to sparse format.
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_dense_to_sparse(ARRAY[0, 0.5, 0, 0.3, 0]::real[]);
/// -- Returns: {1:0.5, 3:0.3}
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_dense_to_sparse")]
fn pg_dense_to_sparse(dense: Vec<f32>) -> SparseVec {
    let mut indices = Vec::new();
    let mut values = Vec::new();

    for (i, &val) in dense.iter().enumerate() {
        if val != 0.0 {
            indices.push(i as u32);
            values.push(val);
        }
    }

    let dim = dense.len() as u32;
    SparseVec::new(indices, values, dim)
        .unwrap_or_else(|e| panic!("Failed to create sparse vector: {}", e))
}

/// Convert a sparse vector to dense array representation.
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_sparse_to_dense('{1:0.5, 3:0.3}'::sparsevec);
/// -- Returns: ARRAY[0, 0.5, 0, 0.3]
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_sparse_to_dense")]
fn pg_sparse_to_dense(sparse: SparseVec) -> Vec<f32> {
    sparse.to_dense()
}

// ============================================================================
// BM25 Functions
// ============================================================================

/// BM25 scoring for sparse term vectors.
///
/// Implements BM25 ranking function commonly used in text search.
///
/// # Arguments
/// * `query` - Query sparse vector (IDF weights)
/// * `doc` - Document sparse vector (term frequencies)
/// * `doc_len` - Document length (number of terms)
/// * `avg_doc_len` - Average document length in collection
/// * `k1` - Term frequency saturation (default 1.2)
/// * `b` - Length normalization (default 0.75)
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_sparse_bm25(
///     query_sparse,
///     doc_sparse,
///     doc_length,
///     avg_doc_length,
///     1.2,  -- k1
///     0.75  -- b
/// ) AS bm25_score
/// FROM documents;
/// ```
#[pg_extern(immutable, parallel_safe, name = "ruvector_sparse_bm25")]
fn pg_sparse_bm25(
    query: SparseVec,
    doc: SparseVec,
    doc_len: f32,
    avg_doc_len: f32,
    k1: default!(f32, 1.2),
    b: default!(f32, 0.75),
) -> f32 {
    sparse_bm25(&query, &doc, doc_len, avg_doc_len, k1, b)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_pg_sparse_dot() {
        let a = SparseVec::new(vec![0, 2, 5], vec![1.0, 2.0, 3.0], 10).unwrap();
        let b = SparseVec::new(vec![2, 3, 5], vec![4.0, 5.0, 6.0], 10).unwrap();

        let result = pg_sparse_dot(a, b);
        assert!((result - 26.0).abs() < 1e-5);
    }

    #[pg_test]
    fn test_pg_sparse_cosine() {
        let a = SparseVec::new(vec![0, 1], vec![3.0, 4.0], 10).unwrap();
        let b = SparseVec::new(vec![0, 1], vec![3.0, 4.0], 10).unwrap();

        let result = pg_sparse_cosine(a, b);
        assert!((result - 1.0).abs() < 1e-5);
    }

    #[pg_test]
    fn test_pg_to_sparse() {
        let indices = vec![1, 2, 5];
        let values = vec![0.5, 0.3, 0.8];
        let dim = 10;

        let sparse = pg_to_sparse(indices, values, dim);
        assert_eq!(sparse.nnz(), 3);
        assert_eq!(sparse.dim(), 10);
    }

    #[pg_test]
    fn test_pg_sparse_top_k() {
        let sparse = SparseVec::new(vec![0, 1, 2, 3], vec![0.1, 0.5, 0.05, 0.8], 10).unwrap();
        let top2 = pg_sparse_top_k(sparse, 2);

        assert_eq!(top2.nnz(), 2);
    }

    #[pg_test]
    fn test_pg_dense_to_sparse() {
        let dense = vec![0.0, 0.5, 0.0, 0.3, 0.0];
        let sparse = pg_dense_to_sparse(dense);

        assert_eq!(sparse.nnz(), 2);
        assert_eq!(sparse.get(1), 0.5);
        assert_eq!(sparse.get(3), 0.3);
    }
}
