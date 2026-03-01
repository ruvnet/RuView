# HNSW Index - Quick Reference Guide

## Installation

```bash
# Build and install
cd /home/user/ruvector/crates/ruvector-postgres
cargo pgrx install

# Enable in database
CREATE EXTENSION ruvector;
```

## Index Creation

```sql
-- L2 distance (default)
CREATE INDEX ON table USING hnsw (column hnsw_l2_ops);

-- With custom parameters
CREATE INDEX ON table USING hnsw (column hnsw_l2_ops)
    WITH (m = 32, ef_construction = 128);

-- Cosine distance
CREATE INDEX ON table USING hnsw (column hnsw_cosine_ops);

-- Inner product
CREATE INDEX ON table USING hnsw (column hnsw_ip_ops);
```

## Query Syntax

```sql
-- L2 distance
SELECT * FROM table ORDER BY column <-> query_vector LIMIT 10;

-- Cosine distance
SELECT * FROM table ORDER BY column <=> query_vector LIMIT 10;

-- Inner product
SELECT * FROM table ORDER BY column <#> query_vector LIMIT 10;
```

## Parameters

### Index Build Parameters

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `m` | 16 | 2-128 | Max connections per layer |
| `ef_construction` | 64 | 4-1000 | Build candidate list size |

### Query Parameters

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `ruvector.ef_search` | 40 | 1-1000 | Search candidate list size |

```sql
-- Set globally
ALTER SYSTEM SET ruvector.ef_search = 100;

-- Set per session
SET ruvector.ef_search = 100;

-- Set per transaction
SET LOCAL ruvector.ef_search = 100;
```

## Distance Metrics

| Metric | Operator | Use Case | Formula |
|--------|----------|----------|---------|
| L2 | `<->` | General distance | √(Σ(a-b)²) |
| Cosine | `<=>` | Direction similarity | 1-(a·b)/(‖a‖‖b‖) |
| Inner Product | `<#>` | Max similarity | -Σ(a*b) |

## Performance Tuning

### For Better Recall

```sql
-- Increase ef_search
SET ruvector.ef_search = 100;

-- Rebuild with higher ef_construction
WITH (ef_construction = 200);
```

### For Faster Build

```sql
-- Lower ef_construction
WITH (ef_construction = 32);

-- Increase memory
SET maintenance_work_mem = '4GB';
```

### For Less Memory

```sql
-- Lower m
WITH (m = 8);
```

## Common Queries

### Basic Similarity Search

```sql
SELECT id, column <-> query AS dist
FROM table
ORDER BY column <-> query
LIMIT 10;
```

### Filtered Search

```sql
SELECT id, column <-> query AS dist
FROM table
WHERE created_at > NOW() - INTERVAL '7 days'
ORDER BY column <-> query
LIMIT 10;
```

### Hybrid Search

```sql
SELECT
    id,
    0.3 * text_rank + 0.7 * (1/(1+vector_dist)) AS score
FROM table
WHERE text_column @@ search_query
ORDER BY score DESC
LIMIT 10;
```

## Maintenance

```sql
-- View statistics
SELECT ruvector_memory_stats();

-- Perform maintenance
SELECT ruvector_index_maintenance('index_name');

-- Vacuum
VACUUM ANALYZE table;

-- Rebuild index
REINDEX INDEX index_name;
```

## Monitoring

```sql
-- Check index size
SELECT pg_size_pretty(pg_relation_size('index_name'));

-- Explain query
EXPLAIN (ANALYZE, BUFFERS)
SELECT * FROM table ORDER BY column <-> query LIMIT 10;
```

## Operators Reference

```sql
-- Distance operators
ARRAY[1,2,3]::real[] <-> ARRAY[4,5,6]::real[]  -- L2
ARRAY[1,2,3]::real[] <=> ARRAY[4,5,6]::real[]  -- Cosine
ARRAY[1,2,3]::real[] <#> ARRAY[4,5,6]::real[]  -- Inner product

-- Vector utilities
vector_normalize(ARRAY[3,4]::real[])           -- Normalize
vector_norm(ARRAY[3,4]::real[])                -- L2 norm
vector_add(a::real[], b::real[])               -- Add vectors
vector_sub(a::real[], b::real[])               -- Subtract
```

## Typical Performance

| Dataset | Dimensions | Build Time | Query Time | Memory |
|---------|------------|------------|------------|--------|
| 10K | 128 | ~1s | <1ms | ~10MB |
| 100K | 128 | ~20s | ~2ms | ~100MB |
| 1M | 128 | ~5min | ~5ms | ~1GB |
| 10M | 128 | ~1hr | ~10ms | ~10GB |

## Parameter Recommendations

### Small Dataset (<100K vectors)

```sql
WITH (m = 16, ef_construction = 64)
SET ruvector.ef_search = 40;
```

### Medium Dataset (100K-1M vectors)

```sql
WITH (m = 16, ef_construction = 128)
SET ruvector.ef_search = 64;
```

### Large Dataset (>1M vectors)

```sql
WITH (m = 32, ef_construction = 200)
SET ruvector.ef_search = 100;
```

## Troubleshooting

### Slow Queries

- ✓ Increase `ef_search`
- ✓ Check index exists: `\d table`
- ✓ Analyze query: `EXPLAIN ANALYZE`

### Low Recall

- ✓ Increase `ef_search`
- ✓ Rebuild with higher `ef_construction`
- ✓ Use higher `m` value

### Out of Memory

- ✓ Lower `m` value
- ✓ Increase `maintenance_work_mem`
- ✓ Build index in batches

### Index Build Fails

- ✓ Check data quality (no NULLs)
- ✓ Verify dimensions match
- ✓ Increase `maintenance_work_mem`

## Files and Documentation

- **Implementation**: `/home/user/ruvector/crates/ruvector-postgres/src/index/hnsw_am.rs`
- **SQL**: `/home/user/ruvector/crates/ruvector-postgres/sql/hnsw_index.sql`
- **Tests**: `/home/user/ruvector/crates/ruvector-postgres/tests/hnsw_index_tests.sql`
- **Docs**: `/home/user/ruvector/docs/HNSW_INDEX.md`
- **Examples**: `/home/user/ruvector/docs/HNSW_USAGE_EXAMPLE.md`
- **Summary**: `/home/user/ruvector/docs/HNSW_IMPLEMENTATION_SUMMARY.md`

## Version Info

- **Implementation Version**: 1.0
- **PostgreSQL**: 14, 15, 16, 17
- **Extension**: ruvector 0.1.0
- **pgrx**: 0.12.x

## Support

- GitHub: https://github.com/ruvnet/ruvector
- Issues: https://github.com/ruvnet/ruvector/issues
- Docs: `/home/user/ruvector/docs/`

---

**Last Updated**: December 2, 2025
