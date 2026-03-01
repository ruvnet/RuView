# RuVector Distance Operators - Quick Reference

## 🚀 Zero-Copy Operators (Use These!)

All operators use SIMD-optimized zero-copy access automatically.

### SQL Operators

```sql
-- L2 (Euclidean) Distance
SELECT * FROM items ORDER BY embedding <-> '[1,2,3]' LIMIT 10;

-- Inner Product (Maximum similarity)
SELECT * FROM items ORDER BY embedding <#> '[1,2,3]' LIMIT 10;

-- Cosine Distance (Semantic similarity)
SELECT * FROM items ORDER BY embedding <=> '[1,2,3]' LIMIT 10;

-- L1 (Manhattan) Distance
SELECT * FROM items ORDER BY embedding <+> '[1,2,3]' LIMIT 10;
```

### Function Forms

```sql
-- When you need the distance value explicitly
SELECT
    id,
    ruvector_l2_distance(embedding, '[1,2,3]') as l2_dist,
    ruvector_ip_distance(embedding, '[1,2,3]') as ip_dist,
    ruvector_cosine_distance(embedding, '[1,2,3]') as cos_dist,
    ruvector_l1_distance(embedding, '[1,2,3]') as l1_dist
FROM items;
```

## 📊 Operator Comparison

| Operator | Math Formula | Range | Best For |
|----------|--------------|-------|----------|
| `<->` | `√Σ(aᵢ-bᵢ)²` | [0, ∞) | General similarity, geometry |
| `<#>` | `-Σ(aᵢ×bᵢ)` | (-∞, ∞) | MIPS, recommendations |
| `<=>` | `1-(a·b)/(‖a‖‖b‖)` | [0, 2] | Text, semantic search |
| `<+>` | `Σ\|aᵢ-bᵢ\|` | [0, ∞) | Sparse vectors, L1 norm |

## 💡 Common Patterns

### Nearest Neighbors
```sql
-- Find 10 nearest neighbors
SELECT id, content, embedding <-> $query AS dist
FROM documents
ORDER BY embedding <-> $query
LIMIT 10;
```

### Filtered Search
```sql
-- Search within a category
SELECT * FROM products
WHERE category = 'electronics'
ORDER BY embedding <=> $query
LIMIT 20;
```

### Distance Threshold
```sql
-- Find all items within distance 0.5
SELECT * FROM items
WHERE embedding <-> $query < 0.5;
```

### Batch Distances
```sql
-- Compare one vector against many
SELECT id, embedding <-> '[1,2,3]' AS distance
FROM items
WHERE id IN (1, 2, 3, 4, 5);
```

## 🏗️ Index Creation

```sql
-- HNSW index (best for most cases)
CREATE INDEX ON items USING hnsw (embedding ruvector_l2_ops)
WITH (m = 16, ef_construction = 64);

-- IVFFlat index (good for large datasets)
CREATE INDEX ON items USING ivfflat (embedding ruvector_cosine_ops)
WITH (lists = 100);
```

## ⚡ Performance Tips

1. **Use RuVector type, not arrays**: `ruvector` type enables zero-copy
2. **Create indexes**: Essential for large datasets
3. **Normalize for cosine**: Pre-normalize vectors if using cosine often
4. **Check SIMD**: Run `SELECT ruvector_simd_info()` to verify acceleration

## 🔄 Migration from pgvector

RuVector operators are **drop-in compatible** with pgvector:

```sql
-- pgvector syntax works unchanged
SELECT * FROM items ORDER BY embedding <-> '[1,2,3]' LIMIT 10;

-- Just change the type from 'vector' to 'ruvector'
ALTER TABLE items ALTER COLUMN embedding TYPE ruvector(384);
```

## 📏 Dimension Support

- **Maximum**: 16,000 dimensions
- **Recommended**: 128-2048 for most use cases
- **Performance**: Optimal at multiples of 16 (AVX-512) or 8 (AVX2)

## 🐛 Debugging

```sql
-- Check SIMD support
SELECT ruvector_simd_info();

-- Verify vector dimensions
SELECT array_length(embedding::float4[], 1) FROM items LIMIT 1;

-- Test distance calculation
SELECT '[1,2,3]'::ruvector <-> '[4,5,6]'::ruvector;
-- Should return: 5.196152 (≈√27)
```

## 🎯 Choosing the Right Metric

| Your Data | Recommended Operator |
|-----------|---------------------|
| Text embeddings (BERT, OpenAI) | `<=>` (cosine) |
| Image features (ResNet, CLIP) | `<->` (L2) |
| Recommender systems | `<#>` (inner product) |
| Document vectors (TF-IDF) | `<=>` (cosine) |
| Sparse features | `<+>` (L1) |
| General floating-point | `<->` (L2) |

## ✅ Validation

```sql
-- Test basic functionality
CREATE TEMP TABLE test_vectors (v ruvector(3));
INSERT INTO test_vectors VALUES ('[1,2,3]'), ('[4,5,6]');

-- Should return distances
SELECT a.v <-> b.v AS l2,
       a.v <#> b.v AS ip,
       a.v <=> b.v AS cosine,
       a.v <+> b.v AS l1
FROM test_vectors a, test_vectors b
WHERE a.v <> b.v;
```

Expected output:
```
   l2    |   ip    |  cosine  |  l1
---------+---------+----------+------
 5.19615 | -32.000 | 0.025368 | 9.00
```

## 📚 Further Reading

- [Complete Documentation](./zero-copy-operators.md)
- [SIMD Implementation](../crates/ruvector-postgres/src/distance/simd.rs)
- [Benchmarks](../benchmarks/distance_bench.md)
