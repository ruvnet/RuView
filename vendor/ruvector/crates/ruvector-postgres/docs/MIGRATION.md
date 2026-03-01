# Migration Guide from pgvector to RuVector-Postgres

## Overview

This guide provides step-by-step instructions for migrating from pgvector to RuVector-Postgres. RuVector-Postgres is designed as a **drop-in replacement** for pgvector with 100% SQL API compatibility and significant performance improvements.

## Key Benefits of Migration

| Feature | pgvector 0.8.0 | RuVector-Postgres | Improvement |
|---------|---------------|-------------------|-------------|
| **Query Performance** | Baseline | 2-10x faster | SIMD optimization |
| **Index Build Speed** | Baseline | 1.5-3x faster | Parallel construction |
| **Memory Usage** | Baseline | 50-75% less | Quantization options |
| **SIMD Support** | Partial AVX2 | Full AVX-512/AVX2/NEON | Better hardware utilization |
| **Quantization** | Binary only | SQ8, PQ, Binary, f16 | More options |
| **ARM Support** | Limited | Full NEON | Optimized for Apple M/Graviton |

## Migration Strategies

### Strategy 1: Parallel Deployment (Zero-Downtime)

**Best for:** Production systems requiring zero downtime

**Steps:**

1. Install RuVector-Postgres alongside pgvector
2. Create parallel tables with RuVector types
3. Dual-write to both tables during transition
4. Validate RuVector results match pgvector
5. Switch reads to RuVector tables
6. Remove pgvector after validation period

**Downtime:** None

**Risk:** Low (rollback available)

### Strategy 2: Blue-Green Deployment

**Best for:** Systems with scheduled maintenance windows

**Steps:**

1. Create complete RuVector environment (green)
2. Replicate data from pgvector (blue) to RuVector
3. Test thoroughly in green environment
4. Switch traffic from blue to green
5. Keep blue as backup for rollback

**Downtime:** Minutes (during switch)

**Risk:** Low (blue environment available for rollback)

### Strategy 3: In-Place Migration

**Best for:** Development/staging environments, or systems with flexible downtime

**Steps:**

1. Backup database
2. Install RuVector-Postgres
3. Convert types and rebuild indexes in-place
4. Restart application
5. Validate functionality

**Downtime:** 1-4 hours (depends on data size)

**Risk:** Medium (requires backup for rollback)

## Pre-Migration Checklist

### 1. Compatibility Assessment

```sql
-- Check pgvector version
SELECT extversion FROM pg_extension WHERE extname = 'vector';
-- Supported: 0.5.0 - 0.8.0

-- Identify vector types in use
SELECT DISTINCT
    n.nspname AS schema,
    c.relname AS table,
    a.attname AS column,
    t.typname AS type
FROM pg_attribute a
JOIN pg_class c ON a.attrelid = c.oid
JOIN pg_namespace n ON c.relnamespace = n.oid
JOIN pg_type t ON a.atttypid = t.oid
WHERE t.typname IN ('vector', 'halfvec', 'sparsevec')
ORDER BY schema, table, column;

-- Check index types
SELECT
    schemaname,
    tablename,
    indexname,
    indexdef
FROM pg_indexes
WHERE indexdef LIKE '%vector%'
ORDER BY schemaname, tablename;
```

### 2. Backup Current State

```bash
# Full database backup
pg_dump -Fc -f backup_before_migration_$(date +%Y%m%d).dump your_database

# Backup pgvector extension version
psql -c "SELECT extversion FROM pg_extension WHERE extname = 'vector'" > pgvector_version.txt

# Export vector data for validation
psql -c "\COPY (SELECT * FROM your_vector_table) TO 'vector_data_export.csv' WITH CSV HEADER"
```

### 3. Performance Baseline

```sql
-- Benchmark current pgvector performance
\timing on
SELECT COUNT(*) FROM items WHERE embedding <-> '[...]'::vector < 0.5;
-- Record execution time

-- Benchmark index scan
EXPLAIN ANALYZE
SELECT * FROM items
ORDER BY embedding <-> '[...]'::vector
LIMIT 10;
-- Record planning time, execution time, rows scanned
```

### 4. Resource Planning

| Data Size | Estimated Migration Time | Required Disk Space | Recommended RAM |
|-----------|-------------------------|---------------------|-----------------|
| <1M vectors | 30 min - 1 hour | 2x current | 4 GB |
| 1M - 10M | 1 - 4 hours | 2x current | 16 GB |
| 10M - 100M | 4 - 12 hours | 2x current | 32 GB |
| 100M+ | 12+ hours | 2x current | 64 GB+ |

## Step-by-Step Migration

### Step 1: Install RuVector-Postgres

See [INSTALLATION.md](./INSTALLATION.md) for detailed instructions.

```bash
# Install RuVector-Postgres extension
cd ruvector/crates/ruvector-postgres
cargo pgrx package --pg-config $(which pg_config)
sudo cp target/release/ruvector-pg16/usr/lib/postgresql/16/lib/* /usr/lib/postgresql/16/lib/
sudo cp target/release/ruvector-pg16/usr/share/postgresql/16/extension/* /usr/share/postgresql/16/extension/
sudo systemctl restart postgresql
```

```sql
-- Verify installation
CREATE EXTENSION ruvector;
SELECT ruvector_version();
-- Expected: 0.1.19

-- pgvector can coexist (for parallel deployment)
SELECT extname, extversion FROM pg_extension WHERE extname IN ('vector', 'ruvector');
```

### Step 2: Schema Conversion

#### Type Mapping

| pgvector Type | RuVector Type | Notes |
|---------------|---------------|-------|
| `vector(n)` | `ruvector(n)` | Direct replacement |
| `halfvec(n)` | `halfvec(n)` | Same name, compatible |
| `sparsevec(n)` | `sparsevec(n)` | Same name, compatible |

#### Table Creation

**Parallel Deployment (Strategy 1):**

```sql
-- Original pgvector table (keep running)
-- CREATE TABLE items (id int, embedding vector(1536), ...);

-- Create RuVector table
CREATE TABLE items_ruvector (
    id INT PRIMARY KEY,
    content TEXT,
    metadata JSONB,
    embedding ruvector(1536),
    created_at TIMESTAMP DEFAULT NOW()
);

-- Copy data with automatic type conversion
INSERT INTO items_ruvector (id, content, metadata, embedding, created_at)
SELECT id, content, metadata, embedding::ruvector, created_at
FROM items;

-- Verify row counts match
SELECT
    (SELECT COUNT(*) FROM items) AS pgvector_count,
    (SELECT COUNT(*) FROM items_ruvector) AS ruvector_count;
```

**In-Place Migration (Strategy 3):**

```sql
-- Rename original table
ALTER TABLE items RENAME TO items_pgvector;

-- Create new table with ruvector type
CREATE TABLE items (
    id INT PRIMARY KEY,
    content TEXT,
    metadata JSONB,
    embedding ruvector(1536),
    created_at TIMESTAMP DEFAULT NOW()
);

-- Copy data
INSERT INTO items (id, content, metadata, embedding, created_at)
SELECT id, content, metadata, embedding::ruvector, created_at
FROM items_pgvector;

-- Verify
SELECT COUNT(*) FROM items;
SELECT COUNT(*) FROM items_pgvector;
```

### Step 3: Index Migration

#### Index Type Mapping

| pgvector Index | RuVector Index | Notes |
|----------------|----------------|-------|
| `USING hnsw` | `USING ruhnsw` | Compatible parameters |
| `USING ivfflat` | `USING ruivfflat` | Compatible parameters |

#### Create HNSW Index

```sql
-- pgvector HNSW index (for reference)
-- CREATE INDEX items_embedding_idx ON items
-- USING hnsw (embedding vector_l2_ops)
-- WITH (m = 16, ef_construction = 64);

-- RuVector HNSW index (compatible parameters)
CREATE INDEX items_embedding_idx ON items_ruvector
USING ruhnsw (embedding ruvector_l2_ops)
WITH (m = 16, ef_construction = 64);

-- Recommended: Use higher parameters for better recall
CREATE INDEX items_embedding_idx ON items_ruvector
USING ruhnsw (embedding ruvector_l2_ops)
WITH (m = 32, ef_construction = 200);

-- Optional: Add quantization for memory savings
CREATE INDEX items_embedding_idx ON items_ruvector
USING ruhnsw (embedding ruvector_l2_ops)
WITH (m = 32, ef_construction = 200, quantization = 'sq8');

-- Monitor index build
SELECT * FROM pg_stat_progress_create_index;
```

#### Create IVFFlat Index

```sql
-- pgvector IVFFlat index (for reference)
-- CREATE INDEX items_embedding_idx ON items
-- USING ivfflat (embedding vector_l2_ops)
-- WITH (lists = 100);

-- RuVector IVFFlat index
CREATE INDEX items_embedding_idx ON items_ruvector
USING ruivfflat (embedding ruvector_l2_ops)
WITH (lists = 100);

-- Recommended: Scale lists with data size
-- For 1M vectors: lists = 1000
-- For 10M vectors: lists = 10000
CREATE INDEX items_embedding_idx ON items_ruvector
USING ruivfflat (embedding ruvector_l2_ops)
WITH (lists = 1000);
```

### Step 4: Query Conversion

#### Operator Mapping

| pgvector | RuVector | Description |
|----------|----------|-------------|
| `<->` | `<->` | L2 (Euclidean) distance |
| `<#>` | `<#>` | Inner product (negative) |
| `<=>` | `<=>` | Cosine distance |
| `<+>` | `<+>` | L1 (Manhattan) distance |

#### Query Examples

**Basic Similarity Search:**

```sql
-- pgvector query
SELECT * FROM items
ORDER BY embedding <-> '[0.1, 0.2, ...]'::vector
LIMIT 10;

-- RuVector query (identical syntax)
SELECT * FROM items_ruvector
ORDER BY embedding <-> '[0.1, 0.2, ...]'::ruvector
LIMIT 10;
```

**Filtered Search:**

```sql
-- pgvector query
SELECT * FROM items
WHERE category = 'technology'
ORDER BY embedding <-> query_vector
LIMIT 10;

-- RuVector query (identical)
SELECT * FROM items_ruvector
WHERE category = 'technology'
ORDER BY embedding <-> query_vector
LIMIT 10;
```

**Distance Threshold:**

```sql
-- pgvector query
SELECT * FROM items
WHERE embedding <-> '[...]'::vector < 0.5;

-- RuVector query (identical)
SELECT * FROM items_ruvector
WHERE embedding <-> '[...]'::ruvector < 0.5;
```

### Step 5: Validation

#### Functional Validation

```sql
-- Compare results between pgvector and RuVector
WITH pgvector_results AS (
    SELECT id, embedding <-> '[...]'::vector AS distance
    FROM items
    ORDER BY distance
    LIMIT 100
),
ruvector_results AS (
    SELECT id, embedding <-> '[...]'::ruvector AS distance
    FROM items_ruvector
    ORDER BY distance
    LIMIT 100
)
SELECT
    p.id AS pg_id,
    r.id AS ru_id,
    p.distance AS pg_dist,
    r.distance AS ru_dist,
    p.id = r.id AS id_match,
    abs(p.distance - r.distance) < 0.0001 AS distance_match
FROM pgvector_results p
FULL OUTER JOIN ruvector_results r ON p.id = r.id
WHERE p.id != r.id OR abs(p.distance - r.distance) >= 0.0001;

-- Expected: Empty result set (all rows match)
```

#### Performance Validation

```sql
-- Benchmark RuVector
\timing on
SELECT COUNT(*) FROM items_ruvector WHERE embedding <-> '[...]'::ruvector < 0.5;
-- Compare with pgvector baseline

EXPLAIN ANALYZE
SELECT * FROM items_ruvector
ORDER BY embedding <-> '[...]'::ruvector
LIMIT 10;
-- Compare planning time, execution time, rows scanned
```

#### Data Integrity Checks

```sql
-- Check row counts
SELECT
    (SELECT COUNT(*) FROM items) AS pgvector_count,
    (SELECT COUNT(*) FROM items_ruvector) AS ruvector_count,
    (SELECT COUNT(*) FROM items) = (SELECT COUNT(*) FROM items_ruvector) AS counts_match;

-- Check for NULL vectors
SELECT COUNT(*) FROM items_ruvector WHERE embedding IS NULL;

-- Check dimension consistency
SELECT DISTINCT array_length(embedding::float4[], 1) AS dims
FROM items_ruvector;
-- Expected: Single row with correct dimension count
```

### Step 6: Application Updates

#### Connection String (No Change)

```python
# No changes needed - same database, same tables (if in-place migration)
conn = psycopg2.connect("postgresql://user:pass@localhost/dbname")
```

#### Query Updates (Minimal)

**Python (psycopg2):**

```python
# pgvector code
cursor.execute("""
    SELECT * FROM items
    ORDER BY embedding <-> %s
    LIMIT 10
""", (query_vector,))

# RuVector code (identical)
cursor.execute("""
    SELECT * FROM items_ruvector
    ORDER BY embedding <-> %s
    LIMIT 10
""", (query_vector,))
```

**Node.js (pg):**

```javascript
// pgvector code
const result = await client.query(
    'SELECT * FROM items ORDER BY embedding <-> $1 LIMIT 10',
    [queryVector]
);

// RuVector code (identical)
const result = await client.query(
    'SELECT * FROM items_ruvector ORDER BY embedding <-> $1 LIMIT 10',
    [queryVector]
);
```

**Go (pgx):**

```go
// pgvector code
rows, err := conn.Query(ctx,
    "SELECT * FROM items ORDER BY embedding <-> $1 LIMIT 10",
    queryVector)

// RuVector code (identical)
rows, err := conn.Query(ctx,
    "SELECT * FROM items_ruvector ORDER BY embedding <-> $1 LIMIT 10",
    queryVector)
```

### Step 7: Cutover

#### For Parallel Deployment (Strategy 1)

```sql
-- Step 1: Stop writes to pgvector table
-- (Update application to write only to items_ruvector)

-- Step 2: Sync any final changes (if dual-writing was used)
INSERT INTO items_ruvector (id, content, metadata, embedding, created_at)
SELECT id, content, metadata, embedding::ruvector, created_at
FROM items
WHERE id NOT IN (SELECT id FROM items_ruvector)
ON CONFLICT (id) DO NOTHING;

-- Step 3: Switch reads to RuVector table
-- (Update application queries from 'items' to 'items_ruvector')

-- Step 4: Rename tables for seamless transition
BEGIN;
ALTER TABLE items RENAME TO items_pgvector_old;
ALTER TABLE items_ruvector RENAME TO items;
COMMIT;

-- Step 5: Verify application still works

-- Step 6: Drop old table after validation period
-- DROP TABLE items_pgvector_old;
```

#### For In-Place Migration (Strategy 3)

```sql
-- Already completed in Step 2 (table already renamed)

-- Just drop backup after validation
DROP TABLE items_pgvector;
```

## Performance Tuning After Migration

### 1. Configure GUC Variables

```sql
-- Set globally in postgresql.conf
ALTER SYSTEM SET ruvector.ef_search = 100;  -- Higher = better recall
ALTER SYSTEM SET ruvector.probes = 10;      -- For IVFFlat indexes
SELECT pg_reload_conf();

-- Or set per-session
SET ruvector.ef_search = 200;  -- For high-recall queries
SET ruvector.ef_search = 40;   -- For low-latency queries
```

### 2. Index Optimization

```sql
-- Check index statistics
SELECT * FROM ruvector_index_stats('items_embedding_idx');

-- Rebuild index with optimized parameters
DROP INDEX items_embedding_idx;
CREATE INDEX items_embedding_idx ON items
USING ruhnsw (embedding ruvector_l2_ops)
WITH (
    m = 32,                    -- Higher for better recall
    ef_construction = 200,     -- Higher for better build quality
    quantization = 'sq8'       -- Optional: 4x memory reduction
);
```

### 3. Query Optimization

```sql
-- Use EXPLAIN ANALYZE to verify index usage
EXPLAIN (ANALYZE, BUFFERS)
SELECT * FROM items
ORDER BY embedding <-> query
LIMIT 10;

-- Should show:
-- "Index Scan using items_embedding_idx"
-- Buffers: shared hit=XXX (high cache hits are good)
```

### 4. Memory Tuning

```sql
-- Adjust PostgreSQL memory settings
ALTER SYSTEM SET shared_buffers = '8GB';
ALTER SYSTEM SET maintenance_work_mem = '2GB';
ALTER SYSTEM SET work_mem = '256MB';
SELECT pg_reload_conf();
```

## Troubleshooting

### Issue: Type Conversion Errors

**Error:**

```
ERROR: cannot cast type vector to ruvector
```

**Solution:**

```sql
-- Explicit conversion
INSERT INTO items_ruvector (embedding)
SELECT embedding::text::ruvector FROM items;

-- Or use intermediate array
INSERT INTO items_ruvector (embedding)
SELECT (embedding::text)::ruvector FROM items;
```

### Issue: Index Build Fails with OOM

**Error:**

```
ERROR: out of memory
```

**Solution:**

```sql
-- Increase maintenance memory
SET maintenance_work_mem = '8GB';

-- Build with lower parameters first
CREATE INDEX items_embedding_idx ON items
USING ruhnsw (embedding ruvector_l2_ops)
WITH (m = 8, ef_construction = 32);

-- Or use quantization
CREATE INDEX items_embedding_idx ON items
USING ruhnsw (embedding ruvector_l2_ops)
WITH (quantization = 'pq16');  -- 16x memory reduction
```

### Issue: Performance Worse Than pgvector

**Diagnosis:**

```sql
-- Check SIMD support
SELECT ruvector_simd_info();
-- Expected: AVX2 or AVX512 (not Scalar)

-- Check index usage
EXPLAIN SELECT * FROM items ORDER BY embedding <-> query LIMIT 10;
-- Should show "Index Scan using items_embedding_idx"

-- Check ef_search setting
SHOW ruvector.ef_search;
-- Try increasing: SET ruvector.ef_search = 100;
```

### Issue: Results Differ from pgvector

**Cause:** Floating-point precision differences

**Validation:**

```sql
-- Check if differences are within acceptable threshold
WITH comparison AS (
    SELECT
        p.id,
        p.distance AS pg_dist,
        r.distance AS ru_dist,
        abs(p.distance - r.distance) AS diff
    FROM pgvector_results p
    JOIN ruvector_results r ON p.id = r.id
)
SELECT
    MAX(diff) AS max_difference,
    AVG(diff) AS avg_difference
FROM comparison;

-- Expected: max < 0.0001, avg < 0.00001
```

## Rollback Plan

### From Parallel Deployment

```sql
-- Switch back to pgvector table
BEGIN;
ALTER TABLE items RENAME TO items_ruvector;
ALTER TABLE items_pgvector_old RENAME TO items;
COMMIT;

-- Drop RuVector extension (optional)
DROP EXTENSION ruvector CASCADE;
```

### From In-Place Migration

```bash
# Restore from backup
pg_restore -d your_database backup_before_migration.dump

# Verify
psql -c "SELECT COUNT(*) FROM items" your_database
```

## Post-Migration Checklist

- [ ] All tables migrated and validated
- [ ] All indexes rebuilt and tested
- [ ] Application queries updated and tested
- [ ] Performance meets or exceeds pgvector baseline
- [ ] Backup of pgvector data retained for rollback period
- [ ] Monitoring and alerting configured
- [ ] Documentation updated
- [ ] Team trained on RuVector-specific features

## Schema Compatibility Notes

### Compatible SQL Functions

| pgvector | RuVector | Compatible |
|----------|----------|------------|
| `vector_dims(v)` | `ruvector_dims(v)` | ✓ |
| `vector_norm(v)` | `ruvector_norm(v)` | ✓ |
| `l2_distance(a, b)` | `ruvector_l2_distance(a, b)` | ✓ |
| `cosine_distance(a, b)` | `ruvector_cosine_distance(a, b)` | ✓ |
| `inner_product(a, b)` | `ruvector_ip_distance(a, b)` | ✓ |

### New Features in RuVector

Features **not** available in pgvector:

```sql
-- Scalar quantization (4x memory reduction)
CREATE INDEX ... WITH (quantization = 'sq8');

-- Product quantization (16x memory reduction)
CREATE INDEX ... WITH (quantization = 'pq16');

-- f16 SIMD support (2x throughput)
CREATE TABLE items (embedding halfvec(1536));

-- Index maintenance function
SELECT ruvector_index_maintenance('items_embedding_idx');

-- Memory statistics
SELECT * FROM ruvector_memory_stats();
```

## Support and Resources

- **Documentation**: [/docs](/docs) directory
- **API Reference**: [API.md](./API.md)
- **Performance Guide**: [SIMD_OPTIMIZATION.md](./SIMD_OPTIMIZATION.md)
- **GitHub Issues**: https://github.com/ruvnet/ruvector/issues
- **Community Forum**: https://github.com/ruvnet/ruvector/discussions

## Migration Checklist Template

```markdown
## Pre-Migration
- [ ] Backup database
- [ ] Record pgvector version
- [ ] Document current schema
- [ ] Benchmark current performance
- [ ] Install RuVector extension

## Migration
- [ ] Create RuVector tables
- [ ] Copy data with type conversion
- [ ] Build indexes
- [ ] Validate row counts
- [ ] Compare query results
- [ ] Test application integration

## Post-Migration
- [ ] Performance meets expectations
- [ ] Application fully functional
- [ ] Monitoring configured
- [ ] Rollback plan tested
- [ ] Team trained
- [ ] Documentation updated

## Cleanup (after validation period)
- [ ] Drop old pgvector tables
- [ ] Drop pgvector extension (optional)
- [ ] Archive backups
```
