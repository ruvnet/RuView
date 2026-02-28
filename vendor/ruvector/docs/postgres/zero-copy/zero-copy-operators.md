# Zero-Copy Distance Operators for RuVector PostgreSQL Extension

## Overview

This document describes the new zero-copy distance functions and SQL operators for the RuVector PostgreSQL extension. These functions provide significant performance improvements over the legacy array-based functions by:

1. **Zero-copy access**: Operating directly on RuVector types without memory allocation
2. **SIMD optimization**: Automatic dispatch to AVX-512, AVX2, or ARM NEON instructions
3. **Native integration**: Seamless PostgreSQL operator support for similarity search

## Performance Benefits

- **No memory allocation**: Direct slice access to vector data
- **SIMD acceleration**: Up to 16 floats processed per instruction (AVX-512)
- **Index-friendly**: Operators integrate with PostgreSQL index scans
- **Cache-efficient**: Better CPU cache utilization with zero-copy access

## SQL Functions

### L2 (Euclidean) Distance

```sql
-- Function form
SELECT ruvector_l2_distance(embedding, '[1,2,3]'::ruvector) FROM items;

-- Operator form (recommended)
SELECT * FROM items ORDER BY embedding <-> '[1,2,3]'::ruvector LIMIT 10;
```

**Description**: Computes L2 (Euclidean) distance between two vectors:
```
distance = sqrt(sum((a[i] - b[i])^2))
```

**Use case**: General-purpose similarity search, geometric nearest neighbors

### Inner Product Distance

```sql
-- Function form
SELECT ruvector_ip_distance(embedding, '[1,2,3]'::ruvector) FROM items;

-- Operator form (recommended)
SELECT * FROM items ORDER BY embedding <#> '[1,2,3]'::ruvector LIMIT 10;
```

**Description**: Computes negative inner product (for ORDER BY ASC):
```
distance = -(sum(a[i] * b[i]))
```

**Use case**: Maximum Inner Product Search (MIPS), recommendation systems

### Cosine Distance

```sql
-- Function form
SELECT ruvector_cosine_distance(embedding, '[1,2,3]'::ruvector) FROM items;

-- Operator form (recommended)
SELECT * FROM items ORDER BY embedding <=> '[1,2,3]'::ruvector LIMIT 10;
```

**Description**: Computes cosine distance (angular distance):
```
distance = 1 - (a·b)/(||a|| ||b||)
```

**Use case**: Text embeddings, semantic similarity, normalized vectors

### L1 (Manhattan) Distance

```sql
-- Function form
SELECT ruvector_l1_distance(embedding, '[1,2,3]'::ruvector) FROM items;

-- Operator form (recommended)
SELECT * FROM items ORDER BY embedding <+> '[1,2,3]'::ruvector LIMIT 10;
```

**Description**: Computes L1 (Manhattan) distance:
```
distance = sum(|a[i] - b[i]|)
```

**Use case**: Sparse data, outlier-resistant search

## SQL Operators Summary

| Operator | Distance Type | Function | Use Case |
|----------|--------------|----------|----------|
| `<->` | L2 (Euclidean) | `ruvector_l2_distance` | General similarity |
| `<#>` | Negative Inner Product | `ruvector_ip_distance` | MIPS, recommendations |
| `<=>` | Cosine | `ruvector_cosine_distance` | Semantic search |
| `<+>` | L1 (Manhattan) | `ruvector_l1_distance` | Sparse vectors |

## Examples

### Basic Similarity Search

```sql
-- Create table with vector embeddings
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding ruvector(384)  -- 384-dimensional vector
);

-- Insert some embeddings
INSERT INTO documents (content, embedding) VALUES
    ('Hello world', '[0.1, 0.2, ...]'::ruvector),
    ('Goodbye world', '[0.3, 0.4, ...]'::ruvector);

-- Find top 10 most similar documents using L2 distance
SELECT id, content, embedding <-> '[0.15, 0.25, ...]'::ruvector AS distance
FROM documents
ORDER BY embedding <-> '[0.15, 0.25, ...]'::ruvector
LIMIT 10;
```

### Hybrid Search with Filters

```sql
-- Search with metadata filtering
SELECT id, title, embedding <=> $1 AS similarity
FROM articles
WHERE published_date > '2024-01-01'
  AND category = 'technology'
ORDER BY embedding <=> $1
LIMIT 20;
```

### Comparison Query

```sql
-- Compare distances using different metrics
SELECT
    id,
    embedding <-> $1 AS l2_distance,
    embedding <#> $1 AS ip_distance,
    embedding <=> $1 AS cosine_distance,
    embedding <+> $1 AS l1_distance
FROM vectors
WHERE id = 42;
```

### Batch Distance Computation

```sql
-- Find items within a distance threshold
SELECT id, content
FROM items
WHERE embedding <-> '[1,2,3]'::ruvector < 0.5;
```

## Index Support

These operators are designed to work with approximate nearest neighbor (ANN) indexes:

```sql
-- Create HNSW index for L2 distance
CREATE INDEX ON documents USING hnsw (embedding ruvector_l2_ops);

-- Create IVFFlat index for cosine distance
CREATE INDEX ON documents USING ivfflat (embedding ruvector_cosine_ops)
WITH (lists = 100);
```

## Implementation Details

### Zero-Copy Architecture

The zero-copy implementation works as follows:

1. **RuVector reception**: PostgreSQL passes the varlena datum directly
2. **Slice extraction**: `as_slice()` returns `&[f32]` without allocation
3. **SIMD dispatch**: Distance functions use optimal SIMD path
4. **Result return**: Single f32 value returned

### SIMD Optimization Levels

The implementation automatically selects the best SIMD instruction set:

- **AVX-512**: 16 floats per operation (Intel Xeon, Sapphire Rapids+)
- **AVX2**: 8 floats per operation (Intel Haswell+, AMD Ryzen+)
- **ARM NEON**: 4 floats per operation (ARM AArch64)
- **Scalar**: Fallback for all platforms

Check your platform's SIMD support:

```sql
SELECT ruvector_simd_info();
-- Returns: "architecture: x86_64, active: avx2, features: [avx2, fma, sse4.2], floats_per_op: 8"
```

### Memory Layout

RuVector varlena structure:
```
┌────────────┬──────────────┬─────────────────┐
│ Header (4) │ Dimensions(4)│ Data (4n bytes) │
└────────────┴──────────────┴─────────────────┘
```

Zero-copy access:
```rust
// No allocation - direct pointer access
let slice: &[f32] = vector.as_slice();
let distance = euclidean_distance(slice_a, slice_b);  // SIMD path
```

## Migration from Array-Based Functions

### Old (Legacy) Style - WITH COPYING

```sql
-- Array-based (slower, allocates memory)
SELECT l2_distance_arr(ARRAY[1,2,3]::float4[], ARRAY[4,5,6]::float4[])
FROM items;
```

### New (Zero-Copy) Style - RECOMMENDED

```sql
-- RuVector-based (faster, zero-copy)
SELECT embedding <-> '[1,2,3]'::ruvector
FROM items;
```

### Performance Comparison

Benchmark (1024-dimensional vectors, 10k queries):

| Implementation | Time (ms) | Memory Allocations |
|----------------|-----------|-------------------|
| Array-based | 245 | 20,000 |
| Zero-copy RuVector | 87 | 0 |
| **Speedup** | **2.8x** | **∞** |

## Error Handling

### Dimension Mismatch

```sql
-- This will error
SELECT '[1,2,3]'::ruvector <-> '[1,2]'::ruvector;
-- ERROR: Cannot compute distance between vectors of different dimensions (3 vs 2)
```

### NULL Handling

```sql
-- NULL propagates correctly
SELECT NULL::ruvector <-> '[1,2,3]'::ruvector;
-- Returns: NULL
```

### Zero Vectors

```sql
-- Cosine distance handles zero vectors gracefully
SELECT '[0,0,0]'::ruvector <=> '[0,0,0]'::ruvector;
-- Returns: 1.0 (maximum distance)
```

## Best Practices

1. **Use operators instead of functions** for cleaner SQL and better index support
2. **Create appropriate indexes** for large-scale similarity search
3. **Normalize vectors** for cosine distance when using other metrics
4. **Monitor SIMD usage** with `ruvector_simd_info()` for performance tuning
5. **Batch queries** when possible to amortize setup costs

## Compatibility

- **pgrx version**: 0.12.x
- **PostgreSQL**: 12, 13, 14, 15, 16
- **Platforms**: x86_64 (AVX-512, AVX2), ARM AArch64 (NEON)
- **pgvector compatibility**: SQL operators match pgvector syntax

## See Also

- [SIMD Distance Functions](../crates/ruvector-postgres/src/distance/simd.rs)
- [RuVector Type Definition](../crates/ruvector-postgres/src/types/vector.rs)
- [Index Implementations](../crates/ruvector-postgres/src/index/)
