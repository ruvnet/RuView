# IVFFlat Index - Quick Reference

## Installation

```sql
-- 1. Load extension
CREATE EXTENSION ruvector;

-- 2. Create access method (run once)
\i sql/ivfflat_am.sql

-- 3. Verify
SELECT * FROM pg_am WHERE amname = 'ruivfflat';
```

## Create Index

```sql
-- Small dataset (< 10K vectors)
CREATE INDEX idx_name ON table_name
USING ruivfflat (embedding vector_l2_ops)
WITH (lists = 50);

-- Medium dataset (10K-100K vectors)
CREATE INDEX idx_name ON table_name
USING ruivfflat (embedding vector_l2_ops)
WITH (lists = 100);

-- Large dataset (> 100K vectors)
CREATE INDEX idx_name ON table_name
USING ruivfflat (embedding vector_l2_ops)
WITH (lists = 500);
```

## Distance Metrics

```sql
-- Euclidean (L2)
CREATE INDEX ON table USING ruivfflat (embedding vector_l2_ops);
SELECT * FROM table ORDER BY embedding <-> '[...]' LIMIT 10;

-- Cosine
CREATE INDEX ON table USING ruivfflat (embedding vector_cosine_ops);
SELECT * FROM table ORDER BY embedding <=> '[...]' LIMIT 10;

-- Inner Product
CREATE INDEX ON table USING ruivfflat (embedding vector_ip_ops);
SELECT * FROM table ORDER BY embedding <#> '[...]' LIMIT 10;
```

## Performance Tuning

```sql
-- Fast (70% recall)
SET ruvector.ivfflat_probes = 1;

-- Balanced (85% recall)
SET ruvector.ivfflat_probes = 5;

-- Accurate (95% recall)
SET ruvector.ivfflat_probes = 10;

-- Very accurate (98% recall)
SET ruvector.ivfflat_probes = 20;
```

## Common Operations

```sql
-- Get index stats
SELECT * FROM ruvector_ivfflat_stats('idx_name');

-- Check index size
SELECT pg_size_pretty(pg_relation_size('idx_name'));

-- Rebuild index
REINDEX INDEX idx_name;

-- Drop index
DROP INDEX idx_name;
```

## File Structure

```
Implementation Files (2,106 lines total):
├── src/index/ivfflat_am.rs (673 lines)      - Access method callbacks
├── src/index/ivfflat_storage.rs (347 lines) - Storage management
├── sql/ivfflat_am.sql (61 lines)            - SQL installation
├── docs/ivfflat_access_method.md (304 lines)- Architecture docs
├── examples/ivfflat_usage.md (472 lines)    - Usage examples
└── tests/ivfflat_am_test.sql (249 lines)    - Test suite
```

## Key Implementation Features

✅ **PostgreSQL Access Method**: Full IndexAmRoutine with all callbacks
✅ **Storage Layout**: Page 0 (metadata), 1-N (centroids), N+1-M (lists)
✅ **K-means Clustering**: K-means++ init + Lloyd's algorithm
✅ **Search Algorithm**: Probe nearest centroids, re-rank candidates
✅ **Zero-Copy**: Direct heap tuple access
✅ **GUC Variables**: Configurable via ruvector.ivfflat_probes
✅ **Multiple Metrics**: L2, Cosine, Inner Product, Manhattan

## Performance Guidelines

| Dataset Size | Lists | Probes | Expected QPS | Recall |
|--------------|-------|--------|--------------|--------|
| 10K          | 50    | 5      | 1000         | 85%    |
| 100K         | 100   | 10     | 500          | 92%    |
| 1M           | 500   | 10     | 250          | 95%    |
| 10M          | 1000  | 10     | 125          | 95%    |

## Troubleshooting

**Slow queries?**
```sql
SET ruvector.ivfflat_probes = 1;  -- Reduce probes
```

**Low recall?**
```sql
SET ruvector.ivfflat_probes = 20;  -- Increase probes
-- OR
CREATE INDEX ... WITH (lists = 1000);  -- More lists
```

**Index build fails?**
```sql
-- Reduce lists if memory constrained
CREATE INDEX ... WITH (lists = 50);
```

## Documentation

- **Architecture**: `docs/ivfflat_access_method.md`
- **Usage Examples**: `examples/ivfflat_usage.md`
- **Test Suite**: `tests/ivfflat_am_test.sql`
- **Overview**: `README_IVFFLAT.md`
- **Summary**: `IMPLEMENTATION_SUMMARY.md`
