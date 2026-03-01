# Sparse Vectors Guide

## Overview

The sparse vector module provides efficient storage and operations for high-dimensional sparse vectors, commonly used in:

- **Text search**: BM25, TF-IDF representations
- **Learned sparse retrieval**: SPLADE, SPLADEv2
- **Sparse embeddings**: Domain-specific sparse representations

## Features

- **COO Format**: Coordinate (index, value) storage for efficient sparse operations
- **Sparse-Sparse Operations**: Optimized merge-based algorithms
- **PostgreSQL Integration**: Full pgrx-based type system
- **Flexible Parsing**: String and array-based construction

## SQL Usage

### Creating Tables

```sql
-- Create table with sparse vectors
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    sparse_embedding sparsevec,
    metadata JSONB
);
```

### Inserting Data

```sql
-- From string format (index:value pairs)
INSERT INTO documents (content, sparse_embedding)
VALUES (
    'Machine learning tutorial',
    '{1024:0.5, 2048:0.3, 4096:0.8}'::sparsevec
);

-- From arrays
INSERT INTO documents (content, sparse_embedding)
VALUES (
    'Natural language processing',
    ruvector_to_sparse(
        ARRAY[1024, 2048, 4096]::int[],
        ARRAY[0.5, 0.3, 0.8]::real[],
        30000  -- dimension
    )
);

-- From dense vector
INSERT INTO documents (sparse_embedding)
VALUES (
    ruvector_dense_to_sparse(ARRAY[0, 0.5, 0, 0.3, 0]::real[])
);
```

### Distance Operations

```sql
-- Sparse dot product (inner product)
SELECT id, content,
       ruvector_sparse_dot(sparse_embedding, query_vec) AS score
FROM documents
ORDER BY score DESC
LIMIT 10;

-- Cosine similarity
SELECT id,
       ruvector_sparse_cosine(sparse_embedding, query_vec) AS similarity
FROM documents
WHERE ruvector_sparse_cosine(sparse_embedding, query_vec) > 0.5;

-- Euclidean distance
SELECT id,
       ruvector_sparse_euclidean(sparse_embedding, query_vec) AS distance
FROM documents
ORDER BY distance ASC
LIMIT 10;

-- Manhattan distance
SELECT id,
       ruvector_sparse_manhattan(sparse_embedding, query_vec) AS distance
FROM documents
ORDER BY distance ASC
LIMIT 10;
```

### BM25 Text Search

```sql
-- BM25 scoring
SELECT id, content,
       ruvector_sparse_bm25(
           query_sparse,           -- Query with IDF weights
           sparse_embedding,       -- Document term frequencies
           doc_length,             -- Document length
           avg_doc_length,         -- Collection average
           1.2,                    -- k1 parameter
           0.75                    -- b parameter
       ) AS bm25_score
FROM documents
ORDER BY bm25_score DESC
LIMIT 10;
```

### Utility Functions

```sql
-- Get number of non-zero elements
SELECT ruvector_sparse_nnz(sparse_embedding) FROM documents;

-- Get dimension
SELECT ruvector_sparse_dim(sparse_embedding) FROM documents;

-- Get L2 norm
SELECT ruvector_sparse_norm(sparse_embedding) FROM documents;

-- Keep top-k elements by magnitude
SELECT ruvector_sparse_top_k(sparse_embedding, 100) FROM documents;

-- Prune elements below threshold
SELECT ruvector_sparse_prune(sparse_embedding, 0.1) FROM documents;

-- Convert to dense array
SELECT ruvector_sparse_to_dense(sparse_embedding) FROM documents;
```

## Rust API

### Creating Sparse Vectors

```rust
use ruvector_postgres::sparse::SparseVec;

// From indices and values
let sparse = SparseVec::new(
    vec![0, 2, 5],
    vec![1.0, 2.0, 3.0],
    10  // dimension
)?;

// From string
let sparse: SparseVec = "{1:0.5, 2:0.3, 5:0.8}".parse()?;

// Properties
assert_eq!(sparse.nnz(), 3);      // Number of non-zero elements
assert_eq!(sparse.dim(), 10);     // Total dimension
assert_eq!(sparse.get(2), 2.0);   // Get value at index
assert_eq!(sparse.norm(), ...);   // L2 norm
```

### Distance Computations

```rust
use ruvector_postgres::sparse::distance::*;

let a = SparseVec::new(vec![0, 2, 5], vec![1.0, 2.0, 3.0], 10)?;
let b = SparseVec::new(vec![2, 3, 5], vec![4.0, 5.0, 6.0], 10)?;

// Sparse dot product (O(nnz(a) + nnz(b)))
let dot = sparse_dot(&a, &b);  // 2*4 + 3*6 = 26

// Cosine similarity
let sim = sparse_cosine(&a, &b);

// Euclidean distance
let dist = sparse_euclidean(&a, &b);

// Manhattan distance
let l1 = sparse_manhattan(&a, &b);

// BM25 scoring
let score = sparse_bm25(&query, &doc, doc_len, avg_len, 1.2, 0.75);
```

### Sparsification

```rust
// Prune elements below threshold
let mut sparse = SparseVec::new(...)?;
sparse.prune(0.2);

// Keep only top-k elements
let top100 = sparse.top_k(100);

// Convert to/from dense
let dense = sparse.to_dense();
```

## Performance

### Complexity

| Operation | Time Complexity | Space Complexity |
|-----------|----------------|------------------|
| Creation | O(n log n) | O(n) |
| Get value | O(log n) | O(1) |
| Dot product | O(nnz(a) + nnz(b)) | O(1) |
| Cosine | O(nnz(a) + nnz(b)) | O(1) |
| Euclidean | O(nnz(a) + nnz(b)) | O(1) |
| Top-k | O(n log n) | O(n) |

Where `n` is the number of non-zero elements.

### Benchmarks

Typical performance on modern hardware:

| Operation | NNZ (query) | NNZ (doc) | Dim | Time (μs) |
|-----------|-------------|-----------|-----|-----------|
| Dot Product | 100 | 100 | 30K | 0.8 |
| Cosine | 100 | 100 | 30K | 1.2 |
| Euclidean | 100 | 100 | 30K | 1.0 |
| BM25 | 100 | 100 | 30K | 1.5 |

## Storage Format

### COO (Coordinate) Format

Sparse vectors are stored as sorted (index, value) pairs:

```
Indices: [1, 3, 7, 15]
Values:  [0.5, 0.3, 0.8, 0.2]
Dim:     20
```

This represents the vector: `[0, 0.5, 0, 0.3, 0, 0, 0, 0.8, ..., 0.2, ..., 0]`

**Benefits:**
- Minimal storage for sparse data
- Efficient sparse-sparse operations via merge
- Natural ordering for binary search

### PostgreSQL Storage

Sparse vectors are stored using pgrx's `PostgresType` serialization:

```rust
#[derive(PostgresType, Serialize, Deserialize)]
#[pgx(sql = "CREATE TYPE sparsevec")]
pub struct SparseVec {
    indices: Vec<u32>,
    values: Vec<f32>,
    dim: u32,
}
```

TOAST-aware for large sparse vectors (> 2KB).

## Use Cases

### 1. Text Search with BM25

```sql
-- Create table for documents
CREATE TABLE articles (
    id SERIAL PRIMARY KEY,
    title TEXT,
    content TEXT,
    term_freq sparsevec,  -- Term frequencies
    doc_length REAL
);

-- Search with BM25
WITH avg_len AS (
    SELECT AVG(doc_length) AS avg FROM articles
)
SELECT id, title,
       ruvector_sparse_bm25(
           query_idf_vec,
           term_freq,
           doc_length,
           (SELECT avg FROM avg_len),
           1.2,
           0.75
       ) AS score
FROM articles
ORDER BY score DESC
LIMIT 10;
```

### 2. SPLADE Learned Sparse Retrieval

```sql
-- Store SPLADE embeddings
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    splade_vec sparsevec  -- Learned sparse representation
);

-- Efficient search
SELECT id, content,
       ruvector_sparse_dot(splade_vec, query_splade) AS score
FROM documents
ORDER BY score DESC
LIMIT 10;
```

### 3. Hybrid Dense + Sparse Search

```sql
-- Combine dense and sparse signals
SELECT id, content,
       0.7 * (1 - (dense_embedding <=> query_dense)) +
       0.3 * ruvector_sparse_dot(sparse_embedding, query_sparse) AS hybrid_score
FROM documents
ORDER BY hybrid_score DESC
LIMIT 10;
```

## Error Handling

```rust
use ruvector_postgres::sparse::types::SparseError;

match SparseVec::new(indices, values, dim) {
    Ok(sparse) => { /* use sparse */ },
    Err(SparseError::LengthMismatch) => {
        // indices.len() != values.len()
    },
    Err(SparseError::IndexOutOfBounds(idx, dim)) => {
        // Index >= dimension
    },
    Err(e) => { /* other errors */ }
}
```

## Migration from Dense Vectors

```sql
-- Convert existing dense vectors to sparse
UPDATE documents
SET sparse_embedding = ruvector_dense_to_sparse(dense_embedding);

-- Only keep significant elements
UPDATE documents
SET sparse_embedding = ruvector_sparse_prune(sparse_embedding, 0.1);

-- Further compress with top-k
UPDATE documents
SET sparse_embedding = ruvector_sparse_top_k(sparse_embedding, 100);
```

## Best Practices

1. **Choose appropriate sparsity**: Top-k or pruning threshold depends on your data
2. **Normalize when needed**: Use cosine similarity for normalized comparisons
3. **Index efficiently**: Consider inverted index for very sparse data (future feature)
4. **Batch operations**: Use array operations for bulk processing
5. **Monitor storage**: Use `pg_column_size()` to track sparse vector sizes

## Future Features

- **Inverted Index**: Fast approximate search for very sparse vectors
- **Quantization**: 8-bit quantized sparse vectors
- **Hybrid Index**: Combined dense + sparse indexing
- **WAND Algorithm**: Efficient top-k retrieval
- **Batch operations**: SIMD-optimized batch distance computations
