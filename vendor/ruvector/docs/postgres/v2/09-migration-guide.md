# RuVector Postgres v2 - Migration Guide

## Overview

This guide provides step-by-step instructions for migrating from pgvector to RuVector Postgres v2. The migration is designed to be **non-disruptive** with zero data loss and minimal downtime.

---

## Migration Approaches

### Approach 1: In-Place Extension Swap (Recommended)

Swap the extension while keeping data in place. Fastest with zero data copy.

**Downtime**: < 5 minutes
**Risk**: Low

### Approach 2: Parallel Run with Gradual Cutover

Run both extensions simultaneously, gradually shifting traffic.

**Downtime**: Zero
**Risk**: Very Low

### Approach 3: Full Data Migration

Export and re-import all data. Use when changing schema significantly.

**Downtime**: Proportional to data size
**Risk**: Medium

---

## Pre-Migration Checklist

### 1. Verify Compatibility

```sql
-- Check pgvector version
SELECT extversion FROM pg_extension WHERE extname = 'vector';

-- Check PostgreSQL version (RuVector requires 14+)
SELECT version();

-- Count vectors and indexes
SELECT
    relname AS table_name,
    pg_size_pretty(pg_relation_size(c.oid)) AS size,
    (SELECT COUNT(*) FROM pg_class WHERE relname = c.relname) AS rows
FROM pg_class c
JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind = 'r'
  AND EXISTS (
      SELECT 1 FROM pg_attribute a
      JOIN pg_type t ON a.atttypid = t.oid
      WHERE a.attrelid = c.oid AND t.typname = 'vector'
  );

-- List vector indexes
SELECT
    i.relname AS index_name,
    t.relname AS table_name,
    am.amname AS index_type,
    pg_size_pretty(pg_relation_size(i.oid)) AS size
FROM pg_index ix
JOIN pg_class i ON ix.indexrelid = i.oid
JOIN pg_class t ON ix.indrelid = t.oid
JOIN pg_am am ON i.relam = am.oid
WHERE am.amname IN ('hnsw', 'ivfflat');
```

### 2. Backup

```bash
# Full database backup
pg_dump -Fc -f backup_before_migration.dump mydb

# Or just schema with vector data
pg_dump -Fc --table='*embedding*' -f vector_tables.dump mydb
```

### 3. Test Environment

```bash
# Restore to test environment
createdb mydb_test
pg_restore -d mydb_test backup_before_migration.dump

# Install RuVector extension for testing
psql mydb_test -c "CREATE EXTENSION ruvector"
```

---

## Approach 1: In-Place Extension Swap

### Step 1: Install RuVector Extension

```bash
# Install RuVector package
# Option A: From source
cd ruvector-postgres
cargo pgrx install --release

# Option B: From package (when available)
apt install postgresql-16-ruvector
```

### Step 2: Stop Application Writes

```sql
-- Optional: Put tables in read-only mode
BEGIN;
LOCK TABLE items IN EXCLUSIVE MODE;
-- Keep transaction open to block writes
```

### Step 3: Drop pgvector Indexes

```sql
-- Save index definitions for recreation
SELECT indexdef
FROM pg_indexes
WHERE indexname IN (
    SELECT i.relname
    FROM pg_index ix
    JOIN pg_class i ON ix.indexrelid = i.oid
    JOIN pg_am am ON i.relam = am.oid
    WHERE am.amname IN ('hnsw', 'ivfflat')
);

-- Drop indexes (saves original DDL first)
DO $$
DECLARE
    idx RECORD;
BEGIN
    FOR idx IN
        SELECT i.relname AS index_name
        FROM pg_index ix
        JOIN pg_class i ON ix.indexrelid = i.oid
        JOIN pg_am am ON i.relam = am.oid
        WHERE am.amname IN ('hnsw', 'ivfflat')
    LOOP
        EXECUTE format('DROP INDEX IF EXISTS %I', idx.index_name);
    END LOOP;
END $$;
```

### Step 4: Swap Extensions

```sql
-- Drop pgvector
DROP EXTENSION vector CASCADE;

-- Create RuVector
CREATE EXTENSION ruvector;
```

### Step 5: Recreate Indexes

```sql
-- Recreate HNSW index (same syntax)
CREATE INDEX idx_items_embedding ON items
USING hnsw (embedding vector_l2_ops)
WITH (m = 16, ef_construction = 64);

-- Or with RuVector-specific options
CREATE INDEX idx_items_embedding ON items
USING hnsw (embedding vector_l2_ops)
WITH (m = 16, ef_construction = 64);
```

### Step 6: Verify

```sql
-- Check extension
SELECT * FROM pg_extension WHERE extname = 'ruvector';

-- Test query
EXPLAIN ANALYZE
SELECT id, embedding <-> '[0.1, 0.2, ...]' AS distance
FROM items
ORDER BY embedding <-> '[0.1, 0.2, ...]'
LIMIT 10;

-- Compare recall (optional)
-- Run same query with and without index
SET enable_indexscan = off;
-- Query without index (exact)
SET enable_indexscan = on;
-- Query with index (approximate)
```

### Step 7: Resume Application

```sql
-- Release lock
ROLLBACK;  -- If you started a transaction for locking
```

---

## Approach 2: Parallel Run

### Step 1: Install RuVector (Different Schema)

```sql
-- Create schema for RuVector
CREATE SCHEMA ruvector_new;

-- Install RuVector in new schema
CREATE EXTENSION ruvector WITH SCHEMA ruvector_new;
```

### Step 2: Create Shadow Tables

```sql
-- Create shadow table with same structure
CREATE TABLE ruvector_new.items AS
SELECT * FROM items WHERE false;

-- Add vector column using RuVector type
ALTER TABLE ruvector_new.items
    ALTER COLUMN embedding TYPE ruvector_new.vector(768);

-- Copy data
INSERT INTO ruvector_new.items
SELECT * FROM items;

-- Create index
CREATE INDEX ON ruvector_new.items
USING hnsw (embedding ruvector_new.vector_l2_ops)
WITH (m = 16, ef_construction = 64);
```

### Step 3: Set Up Triggers for Sync

```sql
-- Sync inserts
CREATE OR REPLACE FUNCTION sync_to_ruvector()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO ruvector_new.items VALUES (NEW.*);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_sync_insert
    AFTER INSERT ON items
    FOR EACH ROW EXECUTE FUNCTION sync_to_ruvector();

-- Sync updates
CREATE TRIGGER trg_sync_update
    AFTER UPDATE ON items
    FOR EACH ROW EXECUTE FUNCTION sync_to_ruvector_update();

-- Sync deletes
CREATE TRIGGER trg_sync_delete
    AFTER DELETE ON items
    FOR EACH ROW EXECUTE FUNCTION sync_to_ruvector_delete();
```

### Step 4: Gradual Cutover

```python
# Application code with gradual cutover
import random

def search_embeddings(query_vector, use_ruvector_pct=0):
    """
    Gradually shift traffic to RuVector.
    Start with 0%, increase to 100% over time.
    """
    if random.random() * 100 < use_ruvector_pct:
        # Use RuVector
        return db.execute("""
            SELECT id, embedding <-> %s AS distance
            FROM ruvector_new.items
            ORDER BY embedding <-> %s
            LIMIT 10
        """, [query_vector, query_vector])
    else:
        # Use pgvector
        return db.execute("""
            SELECT id, embedding <-> %s AS distance
            FROM items
            ORDER BY embedding <-> %s
            LIMIT 10
        """, [query_vector, query_vector])
```

### Step 5: Complete Migration

Once 100% traffic on RuVector with no issues:

```sql
-- Rename tables
ALTER TABLE items RENAME TO items_pgvector_backup;
ALTER TABLE ruvector_new.items RENAME TO items;
ALTER TABLE items SET SCHEMA public;

-- Drop pgvector
DROP EXTENSION vector CASCADE;
DROP TABLE items_pgvector_backup;

-- Clean up triggers
DROP FUNCTION sync_to_ruvector CASCADE;
```

---

## Approach 3: Full Data Migration

### Step 1: Export Data

```sql
-- Export to CSV
\copy (SELECT id, embedding::text, metadata FROM items) TO 'items_export.csv' CSV;

-- Or to binary format
\copy items TO 'items_export.bin' BINARY;
```

### Step 2: Switch Extensions

```sql
DROP EXTENSION vector CASCADE;
CREATE EXTENSION ruvector;
```

### Step 3: Recreate Tables

```sql
-- Recreate with RuVector type
CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    embedding vector(768),
    metadata JSONB
);

-- Import data
\copy items FROM 'items_export.csv' CSV;

-- Create index
CREATE INDEX ON items USING hnsw (embedding vector_l2_ops);
```

---

## Query Compatibility Reference

### Identical Syntax (No Changes Needed)

```sql
-- Vector type declaration
CREATE TABLE items (embedding vector(768));

-- Distance operators
SELECT * FROM items ORDER BY embedding <-> query LIMIT 10;  -- L2
SELECT * FROM items ORDER BY embedding <=> query LIMIT 10;  -- Cosine
SELECT * FROM items ORDER BY embedding <#> query LIMIT 10;  -- Inner product

-- Index creation
CREATE INDEX ON items USING hnsw (embedding vector_l2_ops);
CREATE INDEX ON items USING hnsw (embedding vector_cosine_ops);
CREATE INDEX ON items USING ivfflat (embedding vector_l2_ops) WITH (lists = 100);

-- Operator classes
vector_l2_ops
vector_cosine_ops
vector_ip_ops

-- Utility functions
SELECT vector_dims(embedding) FROM items LIMIT 1;
SELECT vector_norm(embedding) FROM items LIMIT 1;
```

### Extended Syntax (RuVector Only)

```sql
-- New distance operators
SELECT * FROM items ORDER BY embedding <+> query LIMIT 10;  -- L1/Manhattan

-- Collection registration
SELECT ruvector_register_collection(
    'my_embeddings',
    'public',
    'items',
    'embedding',
    768,
    'l2'
);

-- Advanced search options
SELECT * FROM ruvector_search(
    'my_embeddings',
    query_vector,
    10,           -- k
    100,          -- ef_search
    FALSE,        -- use_gnn
    '{"category": "electronics"}'  -- filter
);

-- Tiered storage
SELECT ruvector_set_tiers('my_embeddings', 24, 168, 720);
SELECT ruvector_tier_report('my_embeddings');

-- Graph integration
SELECT ruvector_graph_create('knowledge_graph');
SELECT ruvector_cypher('knowledge_graph', 'MATCH (n) RETURN n LIMIT 10');

-- Integrity monitoring
SELECT ruvector_integrity_status('my_embeddings');
```

---

## GUC Parameter Mapping

| pgvector | RuVector | Notes |
|----------|----------|-------|
| `ivfflat.probes` | `ruvector.probes` | Same behavior |
| `hnsw.ef_search` | `ruvector.ef_search` | Same behavior |
| N/A | `ruvector.use_simd` | Enable/disable SIMD |
| N/A | `ruvector.max_index_memory` | Memory limit |

```sql
-- Set runtime parameters (same syntax)
SET ruvector.ef_search = 100;
SET ruvector.probes = 10;
```

---

## Common Migration Issues

### Issue 1: Type Mismatch After Migration

```sql
-- Error: operator does not exist: ruvector.vector <-> public.vector
-- Solution: Ensure all tables use the new type

SELECT
    c.relname AS table_name,
    a.attname AS column_name,
    t.typname AS type_name,
    n.nspname AS type_schema
FROM pg_attribute a
JOIN pg_class c ON a.attrelid = c.oid
JOIN pg_type t ON a.atttypid = t.oid
JOIN pg_namespace n ON t.typnamespace = n.oid
WHERE t.typname = 'vector';

-- Fix by recreating column
ALTER TABLE items ALTER COLUMN embedding TYPE ruvector.vector(768);
```

### Issue 2: Index Not Using RuVector AM

```sql
-- Check which AM is being used
SELECT
    i.relname AS index_name,
    am.amname AS access_method
FROM pg_index ix
JOIN pg_class i ON ix.indexrelid = i.oid
JOIN pg_am am ON i.relam = am.oid;

-- Rebuild index with correct AM
DROP INDEX old_index;
CREATE INDEX new_index ON items USING hnsw (embedding vector_l2_ops);
```

### Issue 3: Different Recall/Performance

```sql
-- RuVector may have different default parameters
-- Adjust ef_search for recall
SET ruvector.ef_search = 200;  -- Higher for better recall

-- Check actual ef being used
EXPLAIN (ANALYZE, VERBOSE)
SELECT * FROM items ORDER BY embedding <-> query LIMIT 10;
```

### Issue 4: Extension Dependencies

```sql
-- Check what depends on vector extension
SELECT
    dependent.relname AS dependent_object,
    dependent.relkind AS object_type
FROM pg_depend d
JOIN pg_extension e ON d.refobjid = e.oid
JOIN pg_class dependent ON d.objid = dependent.oid
WHERE e.extname = 'vector';

-- May need to drop dependent objects first
```

---

## Rollback Procedure

If migration fails, rollback to pgvector:

```bash
# Restore from backup
pg_restore -d mydb --clean backup_before_migration.dump

# Or manually:
```

```sql
-- Drop RuVector
DROP EXTENSION ruvector CASCADE;

-- Reinstall pgvector
CREATE EXTENSION vector;

-- Restore schema (from saved DDL)
-- Recreate indexes (from saved DDL)
```

---

## Performance Validation

### Compare Query Performance

```python
import time
import psycopg2
import numpy as np

def benchmark_extension(conn, query_vector, n_queries=100):
    """Benchmark query latency"""
    latencies = []

    for _ in range(n_queries):
        start = time.time()
        with conn.cursor() as cur:
            cur.execute("""
                SELECT id, embedding <-> %s AS distance
                FROM items
                ORDER BY embedding <-> %s
                LIMIT 10
            """, [query_vector, query_vector])
            cur.fetchall()
        latencies.append((time.time() - start) * 1000)

    return {
        'p50': np.percentile(latencies, 50),
        'p95': np.percentile(latencies, 95),
        'p99': np.percentile(latencies, 99),
        'mean': np.mean(latencies),
    }

# Run before migration (pgvector)
pgvector_results = benchmark_extension(conn, query_vec)

# Run after migration (RuVector)
ruvector_results = benchmark_extension(conn, query_vec)

print(f"pgvector p50: {pgvector_results['p50']:.2f}ms")
print(f"RuVector p50: {ruvector_results['p50']:.2f}ms")
```

### Compare Recall

```python
def measure_recall(conn, query_vectors, k=10):
    """Measure recall@k against brute force"""
    recalls = []

    for query in query_vectors:
        # Index scan result
        with conn.cursor() as cur:
            cur.execute("""
                SELECT id FROM items
                ORDER BY embedding <-> %s
                LIMIT %s
            """, [query, k])
            index_results = set(row[0] for row in cur.fetchall())

        # Brute force (disable index)
        with conn.cursor() as cur:
            cur.execute("SET enable_indexscan = off")
            cur.execute("""
                SELECT id FROM items
                ORDER BY embedding <-> %s
                LIMIT %s
            """, [query, k])
            exact_results = set(row[0] for row in cur.fetchall())
            cur.execute("SET enable_indexscan = on")

        recall = len(index_results & exact_results) / k
        recalls.append(recall)

    return np.mean(recalls)
```

---

## Post-Migration Steps

### 1. Register Collections (Optional but Recommended)

```sql
-- Register for RuVector-specific features
SELECT ruvector_register_collection(
    'items_embeddings',
    'public',
    'items',
    'embedding',
    768,
    'l2'
);
```

### 2. Enable Tiered Storage (Optional)

```sql
-- Configure tiers
SELECT ruvector_set_tiers('items_embeddings', 24, 168, 720);
```

### 3. Set Up Integrity Monitoring (Optional)

```sql
-- Enable integrity monitoring
SELECT ruvector_integrity_policy_set('items_embeddings', 'default', '{
    "threshold_high": 0.8,
    "threshold_low": 0.3
}'::jsonb);
```

### 4. Update Application Code

```python
# Minimal changes needed for basic operations

# No change needed:
cursor.execute("SELECT * FROM items ORDER BY embedding <-> %s LIMIT 10", [vec])

# Optional: Use new features
cursor.execute("SELECT * FROM ruvector_search('items_embeddings', %s, 10)", [vec])
```

---

## Support

- GitHub Issues: https://github.com/ruvnet/ruvector/issues
- Documentation: https://ruvector.dev/docs
- Migration Support: migration@ruvector.dev
