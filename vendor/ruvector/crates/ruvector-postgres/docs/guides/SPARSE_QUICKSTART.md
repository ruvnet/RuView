# Sparse Vectors Quick Start

## 5-Minute Setup

### 1. Install Extension

```sql
CREATE EXTENSION IF NOT EXISTS ruvector_postgres;
```

### 2. Create Table

```sql
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    sparse_embedding sparsevec
);
```

### 3. Insert Data

```sql
-- From string format
INSERT INTO documents (content, sparse_embedding) VALUES
    ('Document 1', '{1:0.5, 2:0.3, 5:0.8}'::sparsevec),
    ('Document 2', '{2:0.4, 3:0.2, 5:0.9}'::sparsevec),
    ('Document 3', '{1:0.6, 3:0.7, 4:0.1}'::sparsevec);

-- From arrays
INSERT INTO documents (content, sparse_embedding) VALUES
    ('Document 4',
     ruvector_to_sparse(
         ARRAY[10, 20, 30]::int[],
         ARRAY[0.5, 0.3, 0.8]::real[],
         100  -- dimension
     )
    );
```

### 4. Search

```sql
-- Dot product search
SELECT id, content,
       ruvector_sparse_dot(
           sparse_embedding,
           '{1:0.5, 2:0.3, 5:0.8}'::sparsevec
       ) AS score
FROM documents
ORDER BY score DESC
LIMIT 5;

-- Cosine similarity search
SELECT id, content,
       ruvector_sparse_cosine(
           sparse_embedding,
           '{1:0.5, 2:0.3}'::sparsevec
       ) AS similarity
FROM documents
WHERE ruvector_sparse_cosine(sparse_embedding, '{1:0.5, 2:0.3}'::sparsevec) > 0.5;
```

## Common Patterns

### BM25 Text Search

```sql
-- Create table with term frequencies
CREATE TABLE articles (
    id SERIAL PRIMARY KEY,
    title TEXT,
    content TEXT,
    term_frequencies sparsevec,
    doc_length REAL
);

-- Search with BM25
WITH collection_stats AS (
    SELECT AVG(doc_length) AS avg_doc_len FROM articles
)
SELECT id, title,
       ruvector_sparse_bm25(
           query_idf,           -- Your query with IDF weights
           term_frequencies,    -- Document term frequencies
           doc_length,
           (SELECT avg_doc_len FROM collection_stats),
           1.2,                 -- k1 parameter
           0.75                 -- b parameter
       ) AS bm25_score
FROM articles, collection_stats
ORDER BY bm25_score DESC
LIMIT 10;
```

### Sparse Embeddings (SPLADE)

```sql
-- Store learned sparse embeddings
CREATE TABLE ml_documents (
    id SERIAL PRIMARY KEY,
    text TEXT,
    splade_embedding sparsevec  -- From SPLADE model
);

-- Efficient sparse search
SELECT id, text,
       ruvector_sparse_dot(splade_embedding, query_embedding) AS relevance
FROM ml_documents
ORDER BY relevance DESC
LIMIT 10;
```

### Convert Dense to Sparse

```sql
-- Convert existing dense vectors
CREATE TABLE vectors (
    id SERIAL PRIMARY KEY,
    dense_vec REAL[],
    sparse_vec sparsevec
);

-- Populate sparse from dense
UPDATE vectors
SET sparse_vec = ruvector_dense_to_sparse(dense_vec);

-- Prune small values
UPDATE vectors
SET sparse_vec = ruvector_sparse_prune(sparse_vec, 0.1);

-- Keep only top 100 elements
UPDATE vectors
SET sparse_vec = ruvector_sparse_top_k(sparse_vec, 100);
```

## Utility Functions

```sql
-- Get properties
SELECT
    ruvector_sparse_nnz(sparse_embedding) AS num_nonzero,
    ruvector_sparse_dim(sparse_embedding) AS dimension,
    ruvector_sparse_norm(sparse_embedding) AS l2_norm
FROM documents;

-- Sparsify
SELECT ruvector_sparse_top_k(sparse_embedding, 50) FROM documents;
SELECT ruvector_sparse_prune(sparse_embedding, 0.2) FROM documents;

-- Convert formats
SELECT ruvector_sparse_to_dense(sparse_embedding) FROM documents;
SELECT ruvector_dense_to_sparse(ARRAY[0, 0.5, 0, 0.3]::real[]);
```

## Example Queries

### Find Similar Documents

```sql
-- Find documents similar to document #1
WITH query AS (
    SELECT sparse_embedding AS query_vec
    FROM documents
    WHERE id = 1
)
SELECT d.id, d.content,
       ruvector_sparse_cosine(d.sparse_embedding, q.query_vec) AS similarity
FROM documents d, query q
WHERE d.id != 1
ORDER BY similarity DESC
LIMIT 5;
```

### Hybrid Search

```sql
-- Combine dense and sparse signals
CREATE TABLE hybrid_docs (
    id SERIAL PRIMARY KEY,
    content TEXT,
    dense_embedding vector(768),
    sparse_embedding sparsevec
);

-- Hybrid search with weighted combination
SELECT id, content,
       0.7 * (1 - (dense_embedding <=> query_dense)) +
       0.3 * ruvector_sparse_dot(sparse_embedding, query_sparse) AS combined_score
FROM hybrid_docs
ORDER BY combined_score DESC
LIMIT 10;
```

### Batch Processing

```sql
-- Process multiple queries efficiently
WITH queries(query_id, query_vec) AS (
    VALUES
        (1, '{1:0.5, 2:0.3}'::sparsevec),
        (2, '{3:0.8, 5:0.2}'::sparsevec),
        (3, '{1:0.1, 4:0.9}'::sparsevec)
)
SELECT q.query_id, d.id, d.content,
       ruvector_sparse_dot(d.sparse_embedding, q.query_vec) AS score
FROM documents d
CROSS JOIN queries q
ORDER BY q.query_id, score DESC;
```

## Performance Tips

1. **Use appropriate sparsity**: 100-1000 non-zero elements typically optimal
2. **Prune small values**: Remove noise with `ruvector_sparse_prune(vec, 0.1)`
3. **Top-k sparsification**: Keep most important features with `ruvector_sparse_top_k(vec, 100)`
4. **Monitor sizes**: Use `pg_column_size(sparse_embedding)` to check storage
5. **Batch operations**: Process multiple queries together for better performance

## Troubleshooting

### Parse Error

```sql
-- ❌ Wrong: missing braces
SELECT '{1:0.5, 2:0.3'::sparsevec;

-- ✅ Correct: proper format
SELECT '{1:0.5, 2:0.3}'::sparsevec;
```

### Length Mismatch

```sql
-- ❌ Wrong: different array lengths
SELECT ruvector_to_sparse(ARRAY[1,2]::int[], ARRAY[0.5]::real[], 10);

-- ✅ Correct: same lengths
SELECT ruvector_to_sparse(ARRAY[1,2]::int[], ARRAY[0.5,0.3]::real[], 10);
```

### Index Out of Bounds

```sql
-- ❌ Wrong: index 100 >= dimension 10
SELECT ruvector_to_sparse(ARRAY[100]::int[], ARRAY[0.5]::real[], 10);

-- ✅ Correct: all indices < dimension
SELECT ruvector_to_sparse(ARRAY[5]::int[], ARRAY[0.5]::real[], 10);
```

## Next Steps

- Read the [full guide](SPARSE_VECTORS.md) for advanced features
- Check [implementation details](../integration-plans/05-sparse-vectors.md)
- Explore [hybrid search patterns](SPARSE_VECTORS.md#hybrid-dense--sparse-search)
- Learn about [BM25 tuning](SPARSE_VECTORS.md#bm25-text-search)
