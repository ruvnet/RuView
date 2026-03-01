# HNSW Index Implementation

## Overview

This document describes the HNSW (Hierarchical Navigable Small World) index implementation as a PostgreSQL Access Method for the RuVector extension.

## What is HNSW?

HNSW is a graph-based algorithm for approximate nearest neighbor (ANN) search in high-dimensional spaces. It provides:

- **Logarithmic search complexity**: O(log N) average case
- **High recall**: >95% recall achievable with proper parameters
- **Incremental updates**: Supports efficient insertions and deletions
- **Multi-layer graph structure**: Hierarchical organization for fast traversal

## Architecture

### Page-Based Storage

The HNSW index stores data in PostgreSQL pages for durability and memory management:

```
Page 0 (Metadata):
├─ Magic number: 0x484E5357 ("HNSW")
├─ Version: 1
├─ Dimensions: Vector dimensionality
├─ Parameters: m, m0, ef_construction
├─ Entry point: Block number of top-level node
├─ Max layer: Highest layer in the graph
└─ Metric: Distance metric (L2/Cosine/IP)

Page 1+ (Node Pages):
├─ Node Header:
│  ├─ Page type: HNSW_PAGE_NODE
│  ├─ Max layer: Highest layer for this node
│  └─ Item pointer: TID of heap tuple
├─ Vector data: [f32; dimensions]
├─ Layer 0 neighbors: [BlockNumber; m0]
└─ Layer 1+ neighbors: [[BlockNumber; m]; max_layer]
```

### Access Method Callbacks

The implementation provides all required PostgreSQL index AM callbacks:

1. **`ambuild`** - Builds index from table data
2. **`ambuildempty`** - Creates empty index structure
3. **`aminsert`** - Inserts a single vector
4. **`ambulkdelete`** - Bulk deletion support
5. **`amvacuumcleanup`** - Vacuum cleanup operations
6. **`amcostestimate`** - Query cost estimation
7. **`amgettuple`** - Sequential tuple retrieval
8. **`amgetbitmap`** - Bitmap scan support
9. **`amcanreturn`** - Index-only scan capability
10. **`amoptions`** - Index option parsing

## Usage

### Creating an HNSW Index

```sql
-- Basic index creation (L2 distance, default parameters)
CREATE INDEX ON items USING hnsw (embedding hnsw_l2_ops);

-- With custom parameters
CREATE INDEX ON items USING hnsw (embedding hnsw_l2_ops)
    WITH (m = 32, ef_construction = 128);

-- Cosine distance
CREATE INDEX ON items USING hnsw (embedding hnsw_cosine_ops);

-- Inner product
CREATE INDEX ON items USING hnsw (embedding hnsw_ip_ops);
```

### Querying

```sql
-- Find 10 nearest neighbors using L2 distance
SELECT id, embedding <-> ARRAY[0.1, 0.2, 0.3]::real[] AS distance
FROM items
ORDER BY embedding <-> ARRAY[0.1, 0.2, 0.3]::real[]
LIMIT 10;

-- Find 10 nearest neighbors using cosine distance
SELECT id, embedding <=> ARRAY[0.1, 0.2, 0.3]::real[] AS distance
FROM items
ORDER BY embedding <=> ARRAY[0.1, 0.2, 0.3]::real[]
LIMIT 10;

-- Find vectors with largest inner product
SELECT id, embedding <#> ARRAY[0.1, 0.2, 0.3]::real[] AS neg_ip
FROM items
ORDER BY embedding <#> ARRAY[0.1, 0.2, 0.3]::real[]
LIMIT 10;
```

## Parameters

### Index Build Parameters

| Parameter | Type | Default | Range | Description |
|-----------|------|---------|-------|-------------|
| `m` | integer | 16 | 2-128 | Maximum connections per layer |
| `ef_construction` | integer | 64 | 4-1000 | Size of dynamic candidate list during build |
| `metric` | string | 'l2' | l2/cosine/ip | Distance metric |

**Parameter Tuning Guidelines:**

- **`m`**: Higher values improve recall but increase memory usage
  - Low (8-16): Fast build, lower memory, good for small datasets
  - Medium (16-32): Balanced performance
  - High (32-64): Better recall, slower build, more memory

- **`ef_construction`**: Higher values improve index quality but slow down build
  - Low (32-64): Fast build, may sacrifice recall
  - Medium (64-128): Balanced
  - High (128-500): Best quality, slow build

### Query-Time Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `ruvector.ef_search` | integer | 40 | Size of dynamic candidate list during search |

**Setting ef_search:**

```sql
-- Global setting (postgresql.conf or ALTER SYSTEM)
ALTER SYSTEM SET ruvector.ef_search = 100;

-- Session setting (per-connection)
SET ruvector.ef_search = 100;

-- Query with increased recall
SET LOCAL ruvector.ef_search = 200;
SELECT ... ORDER BY embedding <-> query LIMIT 10;
```

## Distance Metrics

### L2 (Euclidean) Distance

- **Operator**: `<->`
- **Formula**: `√(Σ(a[i] - b[i])²)`
- **Use case**: General-purpose distance
- **Range**: [0, ∞)

```sql
CREATE INDEX ON items USING hnsw (embedding hnsw_l2_ops);
SELECT * FROM items ORDER BY embedding <-> query_vector LIMIT 10;
```

### Cosine Distance

- **Operator**: `<=>`
- **Formula**: `1 - (a·b)/(||a||·||b||)`
- **Use case**: Direction similarity (text embeddings)
- **Range**: [0, 2]

```sql
CREATE INDEX ON items USING hnsw (embedding hnsw_cosine_ops);
SELECT * FROM items ORDER BY embedding <=> query_vector LIMIT 10;
```

### Inner Product

- **Operator**: `<#>`
- **Formula**: `-Σ(a[i] * b[i])`
- **Use case**: Maximum similarity (normalized vectors)
- **Range**: (-∞, ∞)

```sql
CREATE INDEX ON items USING hnsw (embedding hnsw_ip_ops);
SELECT * FROM items ORDER BY embedding <#> query_vector LIMIT 10;
```

## Performance

### Build Performance

- **Time Complexity**: O(N log N) with high probability
- **Space Complexity**: O(N * M * L) where L is average layer count
- **Typical Build Rate**: 1000-10000 vectors/sec (depends on dimensions)

### Query Performance

- **Time Complexity**: O(ef_search * log N)
- **Typical Query Time**:
  - <1ms for 100K vectors (128D)
  - <5ms for 1M vectors (128D)
  - <10ms for 10M vectors (128D)

### Memory Usage

```
Memory per vector ≈ dimensions * 4 bytes + m * 8 bytes * average_layers
Average layers ≈ log₂(N) / log₂(m)

Example (1M vectors, 128D, m=16):
- Vector data: 1M * 128 * 4 = 512 MB
- Graph edges: 1M * 16 * 8 * 4 = 512 MB
- Total: ~1 GB
```

## Operator Classes

### hnsw_l2_ops

For L2 (Euclidean) distance on `real[]` vectors.

```sql
CREATE OPERATOR CLASS hnsw_l2_ops
    FOR TYPE real[] USING hnsw
    FAMILY hnsw_l2_ops AS
    OPERATOR 1 <-> (real[], real[]) FOR ORDER BY float_ops,
    FUNCTION 1 l2_distance_arr(real[], real[]);
```

### hnsw_cosine_ops

For cosine distance on `real[]` vectors.

```sql
CREATE OPERATOR CLASS hnsw_cosine_ops
    FOR TYPE real[] USING hnsw
    FAMILY hnsw_cosine_ops AS
    OPERATOR 1 <=> (real[], real[]) FOR ORDER BY float_ops,
    FUNCTION 1 cosine_distance_arr(real[], real[]);
```

### hnsw_ip_ops

For inner product on `real[]` vectors.

```sql
CREATE OPERATOR CLASS hnsw_ip_ops
    FOR TYPE real[] USING hnsw
    FAMILY hnsw_ip_ops AS
    OPERATOR 1 <#> (real[], real[]) FOR ORDER BY float_ops,
    FUNCTION 1 neg_inner_product_arr(real[], real[]);
```

## Monitoring and Maintenance

### Index Statistics

```sql
-- View memory usage
SELECT ruvector_memory_stats();

-- Check index size
SELECT pg_size_pretty(pg_relation_size('items_embedding_idx'));

-- View index definition
SELECT indexdef FROM pg_indexes WHERE indexname = 'items_embedding_idx';
```

### Index Maintenance

```sql
-- Perform maintenance (optimize connections, rebuild degraded nodes)
SELECT ruvector_index_maintenance('items_embedding_idx');

-- Vacuum to reclaim space after deletes
VACUUM items;

-- Rebuild index if heavily modified
REINDEX INDEX items_embedding_idx;
```

### Query Plan Analysis

```sql
-- Analyze query execution
EXPLAIN (ANALYZE, BUFFERS)
SELECT id, embedding <-> query AS distance
FROM items
ORDER BY embedding <-> query
LIMIT 10;
```

## Best Practices

### 1. Index Creation

- Build indexes on stable data when possible
- Use higher `ef_construction` for better quality
- Consider using `maintenance_work_mem` for large builds:
  ```sql
  SET maintenance_work_mem = '2GB';
  CREATE INDEX ...;
  ```

### 2. Query Optimization

- Adjust `ef_search` based on recall requirements
- Use prepared statements for repeated queries
- Consider query result caching for common queries

### 3. Data Management

- Normalize vectors for cosine similarity
- Batch inserts when possible
- Schedule index maintenance during low-traffic periods

### 4. Monitoring

- Track index size growth
- Monitor query performance metrics
- Set up alerts for memory usage

## Limitations

### Current Version

- **Single column only**: Multi-column indexes not supported
- **No parallel scans**: Query parallelism not yet implemented
- **No index-only scans**: Must access heap tuples
- **Array type only**: Custom vector type support coming soon

### PostgreSQL Version Requirements

- PostgreSQL 14+
- pgrx 0.12+

## Troubleshooting

### Index Build Fails

**Problem**: Out of memory during index build
**Solution**: Increase `maintenance_work_mem` or reduce `ef_construction`

```sql
SET maintenance_work_mem = '4GB';
```

### Slow Queries

**Problem**: Queries are slower than expected
**Solution**: Increase `ef_search` or rebuild index with higher `m`

```sql
SET ruvector.ef_search = 100;
```

### Low Recall

**Problem**: Not finding correct nearest neighbors
**Solution**: Increase `ef_search` or rebuild with higher `ef_construction`

```sql
REINDEX INDEX items_embedding_idx;
```

## Comparison with Other Methods

| Feature | HNSW | IVFFlat | Brute Force |
|---------|------|---------|-------------|
| Search Time | O(log N) | O(√N) | O(N) |
| Build Time | O(N log N) | O(N) | O(1) |
| Memory | High | Medium | Low |
| Recall | >95% | >90% | 100% |
| Updates | Good | Poor | Excellent |

## Future Enhancements

- [ ] Parallel index scans
- [ ] Custom vector type support
- [ ] Index-only scans
- [ ] Dynamic parameter tuning
- [ ] Graph compression
- [ ] Multi-column indexes
- [ ] Distributed HNSW

## References

1. Malkov, Y. A., & Yashunin, D. A. (2018). "Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs." IEEE transactions on pattern analysis and machine intelligence.

2. PostgreSQL Index Access Method documentation: https://www.postgresql.org/docs/current/indexam.html

3. pgrx documentation: https://github.com/pgcentralfoundation/pgrx

## License

MIT License - See LICENSE file for details.
