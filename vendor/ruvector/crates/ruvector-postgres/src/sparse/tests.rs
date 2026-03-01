//! Comprehensive tests for sparse vector functionality.

#[cfg(feature = "pg_test")]
mod sparse_tests {
    use super::super::*;
    use pgrx::prelude::*;

    // ============================================================================
    // Type Tests
    // ============================================================================

    #[pg_test]
    fn test_sparse_creation() {
        let sparse = SparseVec::new(vec![0, 2, 5], vec![1.0, 2.0, 3.0], 10).unwrap();
        assert_eq!(sparse.nnz(), 3);
        assert_eq!(sparse.dim(), 10);
    }

    #[pg_test]
    fn test_sparse_get() {
        let sparse = SparseVec::new(vec![1, 3, 7], vec![0.5, 0.8, 0.2], 10).unwrap();
        assert_eq!(sparse.get(1), 0.5);
        assert_eq!(sparse.get(3), 0.8);
        assert_eq!(sparse.get(7), 0.2);
        assert_eq!(sparse.get(0), 0.0); // Missing index
        assert_eq!(sparse.get(5), 0.0); // Missing index
    }

    #[pg_test]
    fn test_sparse_parse() {
        let sparse: SparseVec = "{1:0.5, 2:0.3, 5:0.8}".parse().unwrap();
        assert_eq!(sparse.nnz(), 3);
        assert_eq!(sparse.get(1), 0.5);
        assert_eq!(sparse.get(2), 0.3);
        assert_eq!(sparse.get(5), 0.8);
    }

    #[pg_test]
    fn test_sparse_display() {
        let sparse = SparseVec::new(vec![1, 2, 5], vec![0.5, 0.3, 0.8], 10).unwrap();
        let s = format!("{}", sparse);
        assert_eq!(s, "{1:0.5, 2:0.3, 5:0.8}");
    }

    #[pg_test]
    fn test_sparse_sorted() {
        // Unsorted input should be sorted
        let sparse = SparseVec::new(vec![5, 1, 3], vec![0.8, 0.5, 0.3], 10).unwrap();
        assert_eq!(sparse.indices(), &[1, 3, 5]);
        assert_eq!(sparse.values(), &[0.5, 0.3, 0.8]);
    }

    #[pg_test]
    fn test_sparse_dedup() {
        // Duplicate indices should be deduplicated
        let sparse = SparseVec::new(vec![1, 2, 2, 5], vec![0.5, 0.3, 0.9, 0.8], 10).unwrap();
        assert_eq!(sparse.nnz(), 3);
        assert_eq!(sparse.get(2), 0.9); // Last value wins
    }

    #[pg_test]
    fn test_sparse_empty() {
        let sparse = SparseVec::new(vec![], vec![], 10).unwrap();
        assert_eq!(sparse.nnz(), 0);
        assert_eq!(sparse.dim(), 10);
        assert_eq!(sparse.norm(), 0.0);
    }

    #[pg_test]
    fn test_sparse_norm() {
        let sparse = SparseVec::new(vec![0, 1, 2], vec![3.0, 4.0, 0.0], 10).unwrap();
        assert!((sparse.norm() - 5.0).abs() < 1e-5); // sqrt(9 + 16 + 0)
    }

    #[pg_test]
    fn test_sparse_prune() {
        let mut sparse = SparseVec::new(vec![0, 1, 2, 3], vec![0.1, 0.5, 0.05, 0.8], 10).unwrap();
        sparse.prune(0.2);
        assert_eq!(sparse.nnz(), 2);
        assert_eq!(sparse.get(1), 0.5);
        assert_eq!(sparse.get(3), 0.8);
        assert_eq!(sparse.get(0), 0.0); // Pruned
    }

    #[pg_test]
    fn test_sparse_top_k() {
        let sparse = SparseVec::new(vec![0, 1, 2, 3], vec![0.1, 0.5, 0.05, 0.8], 10).unwrap();
        let top2 = sparse.top_k(2);
        assert_eq!(top2.nnz(), 2);
        assert!(top2.indices().contains(&1));
        assert!(top2.indices().contains(&3));
    }

    // ============================================================================
    // Distance Function Tests
    // ============================================================================

    #[pg_test]
    fn test_sparse_dot_basic() {
        let a = SparseVec::new(vec![0, 2, 5], vec![1.0, 2.0, 3.0], 10).unwrap();
        let b = SparseVec::new(vec![2, 3, 5], vec![4.0, 5.0, 6.0], 10).unwrap();

        // Dot product: 2*4 + 3*6 = 8 + 18 = 26
        let dot = sparse_dot(&a, &b);
        assert!((dot - 26.0).abs() < 1e-5);
    }

    #[pg_test]
    fn test_sparse_dot_no_overlap() {
        let a = SparseVec::new(vec![0, 1], vec![1.0, 2.0], 10).unwrap();
        let b = SparseVec::new(vec![3, 4], vec![3.0, 4.0], 10).unwrap();

        let dot = sparse_dot(&a, &b);
        assert_eq!(dot, 0.0);
    }

    #[pg_test]
    fn test_sparse_dot_full_overlap() {
        let a = SparseVec::new(vec![0, 1, 2], vec![1.0, 2.0, 3.0], 10).unwrap();
        let b = SparseVec::new(vec![0, 1, 2], vec![4.0, 5.0, 6.0], 10).unwrap();

        // Dot product: 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32
        let dot = sparse_dot(&a, &b);
        assert_eq!(dot, 32.0);
    }

    #[pg_test]
    fn test_sparse_cosine_identical() {
        let a = SparseVec::new(vec![0, 1], vec![3.0, 4.0], 10).unwrap();
        let b = SparseVec::new(vec![0, 1], vec![3.0, 4.0], 10).unwrap();

        let cos = sparse_cosine(&a, &b);
        assert!((cos - 1.0).abs() < 1e-5);
    }

    #[pg_test]
    fn test_sparse_cosine_orthogonal() {
        let a = SparseVec::new(vec![0], vec![1.0], 10).unwrap();
        let b = SparseVec::new(vec![1], vec![1.0], 10).unwrap();

        let cos = sparse_cosine(&a, &b);
        assert_eq!(cos, 0.0);
    }

    #[pg_test]
    fn test_sparse_cosine_opposite() {
        let a = SparseVec::new(vec![0, 1], vec![1.0, 0.0], 10).unwrap();
        let b = SparseVec::new(vec![0, 1], vec![-1.0, 0.0], 10).unwrap();

        let cos = sparse_cosine(&a, &b);
        assert!((cos + 1.0).abs() < 1e-5); // -1.0
    }

    #[pg_test]
    fn test_sparse_euclidean_basic() {
        let a = SparseVec::new(vec![0, 2], vec![0.0, 3.0], 10).unwrap();
        let b = SparseVec::new(vec![0, 2], vec![4.0, 0.0], 10).unwrap();

        // Distance: sqrt(16 + 9) = 5
        let dist = sparse_euclidean(&a, &b);
        assert!((dist - 5.0).abs() < 1e-5);
    }

    #[pg_test]
    fn test_sparse_euclidean_different_indices() {
        let a = SparseVec::new(vec![0], vec![3.0], 10).unwrap();
        let b = SparseVec::new(vec![1], vec![4.0], 10).unwrap();

        // Distance: sqrt(9 + 16) = 5
        let dist = sparse_euclidean(&a, &b);
        assert!((dist - 5.0).abs() < 1e-5);
    }

    #[pg_test]
    fn test_sparse_manhattan_basic() {
        let a = SparseVec::new(vec![0, 2], vec![1.0, 3.0], 10).unwrap();
        let b = SparseVec::new(vec![0, 2], vec![4.0, 1.0], 10).unwrap();

        // Distance: |1-4| + |3-1| = 3 + 2 = 5
        let dist = sparse_manhattan(&a, &b);
        assert_eq!(dist, 5.0);
    }

    // ============================================================================
    // PostgreSQL Operator Tests
    // ============================================================================

    #[pg_test]
    fn test_pg_to_sparse() {
        let indices = vec![1, 2, 5];
        let values = vec![0.5, 0.3, 0.8];
        let dim = 10;

        let sparse = operators::pg_to_sparse(indices, values, dim);
        assert_eq!(sparse.nnz(), 3);
        assert_eq!(sparse.dim(), 10);
    }

    #[pg_test]
    fn test_pg_sparse_nnz() {
        let sparse = SparseVec::new(vec![1, 2, 5], vec![0.5, 0.3, 0.8], 10).unwrap();
        assert_eq!(operators::pg_sparse_nnz(sparse), 3);
    }

    #[pg_test]
    fn test_pg_sparse_dim() {
        let sparse = SparseVec::new(vec![1, 2], vec![0.5, 0.3], 10).unwrap();
        assert_eq!(operators::pg_sparse_dim(sparse), 10);
    }

    #[pg_test]
    fn test_pg_sparse_norm() {
        let sparse = SparseVec::new(vec![0, 1], vec![3.0, 4.0], 10).unwrap();
        let norm = operators::pg_sparse_norm(sparse);
        assert!((norm - 5.0).abs() < 1e-5);
    }

    #[pg_test]
    fn test_pg_dense_to_sparse() {
        let dense = vec![0.0, 0.5, 0.0, 0.3, 0.0];
        let sparse = operators::pg_dense_to_sparse(dense);

        assert_eq!(sparse.nnz(), 2);
        assert_eq!(sparse.get(1), 0.5);
        assert_eq!(sparse.get(3), 0.3);
    }

    #[pg_test]
    fn test_pg_sparse_to_dense() {
        let sparse = SparseVec::new(vec![1, 3], vec![0.5, 0.3], 5).unwrap();
        let dense = operators::pg_sparse_to_dense(sparse);

        assert_eq!(dense.len(), 5);
        assert_eq!(dense, vec![0.0, 0.5, 0.0, 0.3, 0.0]);
    }

    #[pg_test]
    fn test_pg_sparse_top_k() {
        let sparse = SparseVec::new(vec![0, 1, 2, 3], vec![0.1, 0.5, 0.05, 0.8], 10).unwrap();
        let top2 = operators::pg_sparse_top_k(sparse, 2);

        assert_eq!(top2.nnz(), 2);
    }

    #[pg_test]
    fn test_pg_sparse_prune() {
        let sparse = SparseVec::new(vec![0, 1, 2, 3], vec![0.1, 0.5, 0.05, 0.8], 10).unwrap();
        let pruned = operators::pg_sparse_prune(sparse, 0.2);

        assert_eq!(pruned.nnz(), 2);
        assert_eq!(pruned.get(1), 0.5);
        assert_eq!(pruned.get(3), 0.8);
    }

    #[pg_test]
    fn test_bm25_basic() {
        // Query with IDF weights
        let query = SparseVec::new(vec![0, 2], vec![2.0, 3.0], 10).unwrap();
        // Document with term frequencies
        let doc = SparseVec::new(vec![0, 2], vec![1.0, 2.0], 10).unwrap();

        let score = sparse_bm25(&query, &doc, 10.0, 10.0, 1.2, 0.75);
        assert!(score > 0.0);
    }
}
