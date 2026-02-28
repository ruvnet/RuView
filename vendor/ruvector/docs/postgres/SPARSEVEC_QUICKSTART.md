# SparseVec Quick Start Guide

## What is SparseVec?

SparseVec is a native PostgreSQL type for storing and querying **sparse vectors** - vectors where most elements are zero. It's optimized for:

- **Text embeddings** (TF-IDF, BM25)
- **Recommender systems** (user-item matrices)
- **Graph embeddings** (node features)
- **High-dimensional data** with low density

## Key Benefits

✅ **Memory Efficient:** 99%+ reduction for very sparse data
✅ **Fast Operations:** SIMD-optimized merge-join and scatter-gather algorithms
✅ **Zero-Copy:** Direct varlena access without deserialization
✅ **PostgreSQL Native:** Integrates seamlessly with existing vector infrastructure

## Quick Examples

### Basic Usage

```sql
-- Create a sparse vector: {index:value,...}/dimensions
SELECT '{0:1.5, 3:2.5, 7:3.5}/10'::sparsevec;

-- Get dimensions and non-zero count
SELECT sparsevec_dims('{0:1.5, 3:2.5}/10'::sparsevec);    -- Returns: 10
SELECT sparsevec_nnz('{0:1.5, 3:2.5}/10'::sparsevec);     -- Returns: 2
SELECT sparsevec_sparsity('{0:1.5, 3:2.5}/10'::sparsevec); -- Returns: 0.2
```

### Distance Calculations

```sql
-- Cosine distance (best for similarity)
SELECT sparsevec_cosine_distance(
    '{0:1.0, 2:2.0}/5'::sparsevec,
    '{0:2.0, 2:4.0}/5'::sparsevec
);

-- L2 distance (Euclidean)
SELECT sparsevec_l2_distance(
    '{0:1.0, 2:2.0}/5'::sparsevec,
    '{1:1.0, 2:1.0}/5'::sparsevec
);

-- Inner product distance
SELECT sparsevec_ip_distance(
    '{0:1.0, 2:2.0}/5'::sparsevec,
    '{2:1.0, 4:3.0}/5'::sparsevec
);
```

### Conversions

```sql
-- Dense to sparse with threshold
SELECT vector_to_sparsevec('[0.001,0.5,0.002,1.0]'::ruvector, 0.01);
-- Returns: {1:0.5,3:1.0}/4

-- Sparse to dense
SELECT sparsevec_to_vector('{0:1.0, 3:2.0}/5'::sparsevec);
-- Returns: [1.0, 0.0, 0.0, 2.0, 0.0]
```

## Real-World Use Cases

### 1. Document Similarity (TF-IDF)

```sql
-- Create table
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    title TEXT,
    embedding sparsevec(10000)  -- 10K vocabulary
);

-- Insert documents
INSERT INTO documents (title, embedding) VALUES
('Machine Learning Basics', '{45:0.8, 123:0.6, 789:0.9}/10000'),
('Deep Learning Guide', '{45:0.3, 234:0.9, 789:0.4}/10000');

-- Find similar documents
SELECT d.id, d.title,
       sparsevec_cosine_distance(d.embedding, query.embedding) AS distance
FROM documents d,
     (SELECT embedding FROM documents WHERE id = 1) AS query
WHERE d.id != 1
ORDER BY distance ASC
LIMIT 5;
```

### 2. Recommender System

```sql
-- User preferences (sparse item ratings)
CREATE TABLE user_profiles (
    user_id INT PRIMARY KEY,
    preferences sparsevec(100000)  -- 100K items
);

-- Find similar users
SELECT u2.user_id,
       sparsevec_cosine_distance(u1.preferences, u2.preferences) AS similarity
FROM user_profiles u1, user_profiles u2
WHERE u1.user_id = $1 AND u2.user_id != $1
ORDER BY similarity ASC
LIMIT 10;
```

### 3. Graph Node Embeddings

```sql
-- Store graph embeddings
CREATE TABLE graph_nodes (
    node_id BIGINT PRIMARY KEY,
    embedding sparsevec(50000)
);

-- Nearest neighbor search
SELECT node_id,
       sparsevec_l2_distance(embedding, $1) AS distance
FROM graph_nodes
ORDER BY distance ASC
LIMIT 100;
```

## Function Reference

### Distance Functions

| Function | Description | Use Case |
|----------|-------------|----------|
| `sparsevec_l2_distance(a, b)` | Euclidean distance | General similarity |
| `sparsevec_cosine_distance(a, b)` | Cosine distance | Text/semantic similarity |
| `sparsevec_ip_distance(a, b)` | Inner product | Recommendation scores |

### Utility Functions

| Function | Description | Example |
|----------|-------------|---------|
| `sparsevec_dims(v)` | Total dimensions | `sparsevec_dims(v) -> 10` |
| `sparsevec_nnz(v)` | Non-zero count | `sparsevec_nnz(v) -> 3` |
| `sparsevec_sparsity(v)` | Sparsity ratio | `sparsevec_sparsity(v) -> 0.3` |
| `sparsevec_norm(v)` | L2 norm | `sparsevec_norm(v) -> 5.0` |
| `sparsevec_normalize(v)` | Unit normalization | Returns normalized vector |
| `sparsevec_get(v, idx)` | Get value at index | `sparsevec_get(v, 3) -> 2.5` |

### Vector Operations

| Function | Description |
|----------|-------------|
| `sparsevec_add(a, b)` | Element-wise addition |
| `sparsevec_mul_scalar(v, s)` | Scalar multiplication |

### Conversions

| Function | Description |
|----------|-------------|
| `vector_to_sparsevec(dense, threshold)` | Dense → Sparse |
| `sparsevec_to_vector(sparse)` | Sparse → Dense |
| `array_to_sparsevec(arr, threshold)` | Array → Sparse |
| `sparsevec_to_array(sparse)` | Sparse → Array |

## Performance Tips

### When to Use Sparse Vectors

✅ **Good Use Cases:**
- Text embeddings (TF-IDF, BM25) - typically <5% non-zero
- User-item matrices - most users rate <1% of items
- Graph features - sparse connectivity
- High-dimensional data (>1000 dims) with <10% non-zero

❌ **Not Recommended:**
- Dense embeddings (Word2Vec, BERT) - use `ruvector` instead
- Small dimensions (<100)
- High sparsity (>50% non-zero)

### Memory Savings

```
For 10,000-dimensional vector with N non-zeros:
- Dense:  40,000 bytes
- Sparse: 8 + 4N + 4N = 8 + 8N bytes

Savings = (40,000 - 8 - 8N) / 40,000 × 100%

Examples:
- 10 non-zeros:   99.78% savings
- 100 non-zeros:  98.00% savings
- 1000 non-zeros: 80.00% savings
```

### Query Optimization

```sql
-- ✅ GOOD: Filter before distance calculation
SELECT id, sparsevec_cosine_distance(embedding, $1) AS dist
FROM documents
WHERE category = 'tech'  -- Reduce rows first
ORDER BY dist ASC
LIMIT 10;

-- ❌ BAD: Calculate distance on all rows
SELECT id, sparsevec_cosine_distance(embedding, $1) AS dist
FROM documents
ORDER BY dist ASC
LIMIT 10;
```

## Storage Format

### Text Format
```
{index:value,index:value,...}/dimensions

Examples:
{0:1.5, 3:2.5, 7:3.5}/10
{}/100                        # Empty vector
{0:1.0, 1:2.0, 2:3.0}/3      # Dense representation
```

### Binary Layout (Varlena)
```
┌─────────────┬──────────────┬──────────┬──────────┬──────────┐
│  VARHDRSZ   │  dimensions  │   nnz    │ indices  │  values  │
│  (4 bytes)  │  (4 bytes)   │ (4 bytes)│ (4*nnz)  │ (4*nnz)  │
└─────────────┴──────────────┴──────────┴──────────┴──────────┘
```

## Algorithm Details

### Sparse-Sparse Distance (Merge-Join)

```
Time:  O(nnz_a + nnz_b)
Space: O(1)

Process:
1. Compare indices from both vectors
2. If equal: compute on both values
3. If a < b: compute on a's value (b is zero)
4. If b < a: compute on b's value (a is zero)
```

### Sparse-Dense Distance (Scatter-Gather)

```
Time:  O(nnz_sparse)
Space: O(1)

Process:
1. Iterate only over sparse indices
2. Gather dense values at those indices
3. Compute distance components
```

## Common Patterns

### Batch Insert with Threshold

```sql
INSERT INTO embeddings (id, vec)
SELECT id, vector_to_sparsevec(dense_vec, 0.01)
FROM raw_embeddings;
```

### Similarity Search with Threshold

```sql
SELECT id, title
FROM documents
WHERE sparsevec_cosine_distance(embedding, $query) < 0.3
ORDER BY sparsevec_cosine_distance(embedding, $query)
LIMIT 50;
```

### Aggregate Statistics

```sql
SELECT
    AVG(sparsevec_sparsity(embedding)) AS avg_sparsity,
    AVG(sparsevec_nnz(embedding)) AS avg_nnz,
    AVG(sparsevec_norm(embedding)) AS avg_norm
FROM documents;
```

## Troubleshooting

### Vector Dimension Mismatch
```
ERROR: Cannot compute distance between vectors of different dimensions (1000 vs 500)
```
**Solution:** Ensure all vectors have the same total dimensions, even if nnz differs.

### Index Out of Bounds
```
ERROR: Index 1500 out of bounds for dimension 1000
```
**Solution:** Indices must be in range [0, dimensions-1].

### Invalid Format
```
ERROR: Invalid sparsevec format: expected {pairs}/dim
```
**Solution:** Use format `{idx:val,idx:val}/dim`, e.g., `{0:1.5,3:2.5}/10`

## Next Steps

1. **Read full documentation:** `/home/user/ruvector/docs/SPARSEVEC_IMPLEMENTATION.md`
2. **Try examples:** `/home/user/ruvector/docs/examples/sparsevec_examples.sql`
3. **Benchmark your use case:** Compare sparse vs dense for your data
4. **Index support:** Coming soon - HNSW and IVFFlat indexes for sparse vectors

## Resources

- **Implementation:** `/home/user/ruvector/crates/ruvector-postgres/src/types/sparsevec.rs`
- **SQL Examples:** `/home/user/ruvector/docs/examples/sparsevec_examples.sql`
- **Full Documentation:** `/home/user/ruvector/docs/SPARSEVEC_IMPLEMENTATION.md`

---

**Questions or Issues?** Check the full implementation documentation or review the unit tests for additional examples.
