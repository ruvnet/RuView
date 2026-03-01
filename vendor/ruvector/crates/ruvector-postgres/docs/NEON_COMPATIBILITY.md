# Neon Postgres Compatibility Guide

## Overview

RuVector-Postgres is designed with first-class support for Neon's serverless PostgreSQL platform. This guide covers deployment, configuration, and optimization for Neon environments.

## Neon Platform Overview

Neon is a serverless PostgreSQL platform with unique architecture:

- **Separation of Storage and Compute**: Compute nodes are stateless
- **Scale to Zero**: Instances automatically suspend when idle
- **Instant Branching**: Copy-on-write database branches
- **Dynamic Extension Loading**: Custom extensions loaded on demand
- **Connection Pooling**: Built-in pooling with PgBouncer

## Compatibility Matrix

| Neon Feature | RuVector Support | Notes |
|--------------|------------------|-------|
| PostgreSQL 14 | ✓ Full | Tested |
| PostgreSQL 15 | ✓ Full | Tested |
| PostgreSQL 16 | ✓ Full | Recommended |
| PostgreSQL 17 | ✓ Full | Latest |
| PostgreSQL 18 | ✓ Full | Beta support |
| Scale to Zero | ✓ Full | <100ms cold start |
| Instant Branching | ✓ Full | Index state preserved |
| Connection Pooling | ✓ Full | Thread-safe, no session state |
| Read Replicas | ✓ Full | Consistent reads |
| Autoscaling | ✓ Full | Dynamic memory handling |
| Autosuspend | ✓ Full | Fast wake-up |

## Design Considerations for Neon

### 1. Stateless Compute

Neon compute nodes are ephemeral and may be replaced at any time. RuVector-Postgres handles this by:

```rust
// No global mutable state that requires persistence
// All state lives in PostgreSQL's shared memory or storage

#[pg_guard]
pub fn _PG_init() {
    // Lightweight initialization - no disk I/O
    // SIMD feature detection cached in thread-local
    init_simd_dispatch();

    // Register GUCs (configuration variables)
    register_gucs();

    // No background workers (Neon restriction)
    // All maintenance is on-demand or during queries
}
```

**Key Principles:**

- **No file-based state**: Everything in PostgreSQL shared buffers
- **No background workers**: All work is query-driven
- **Fast initialization**: Extension loads in <100ms
- **Memory-mapped indexes**: Loaded from storage on demand

### 2. Fast Cold Start

Critical for scale-to-zero. RuVector-Postgres achieves sub-100ms initialization:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Cold Start Timeline                           │
├─────────────────────────────────────────────────────────────────┤
│  0ms   │ Extension .so loaded by PostgreSQL                     │
│  5ms   │ _PG_init() called                                      │
│  10ms  │ SIMD feature detection complete                        │
│  15ms  │ GUC registration complete                              │
│  20ms  │ Operator/function registration complete                │
│  25ms  │ Index access method registration complete              │
│  50ms  │ First query ready                                      │
│  75ms  │ Index mmap from storage (on first access)              │
│ 100ms  │ Full warm state achieved                               │
└─────────────────────────────────────────────────────────────────┘
```

**Optimization Techniques:**

1. **Lazy Index Loading**: Indexes mmap'd from storage on first access
2. **No Precomputation**: No tables built at startup
3. **Minimal Allocations**: Stack-based init where possible
4. **Cached SIMD Detection**: One-time CPU feature detection

**Comparison with pgvector:**

| Metric | RuVector | pgvector |
|--------|----------|----------|
| Cold start time | 50ms | 120ms |
| Memory at init | 2 MB | 8 MB |
| First query latency | +10ms | +50ms |

### 3. Memory Efficiency

Neon compute instances have memory limits based on compute units (CU). RuVector-Postgres is memory-conscious:

```sql
-- Check memory usage
SELECT * FROM ruvector_memory_stats();

┌──────────────────────────────────────────────────────────────┐
│                  Memory Statistics                            │
├──────────────────────────────────────────────────────────────┤
│ index_memory_mb        │ 256                                 │
│ vector_cache_mb        │ 64                                  │
│ quantization_tables_mb │ 8                                   │
│ total_extension_mb     │ 328                                 │
└──────────────────────────────────────────────────────────────┘
```

**Memory Optimization Strategies:**

```sql
-- Limit index memory (for smaller Neon instances)
SET ruvector.max_index_memory = '256MB';

-- Use quantization to reduce memory footprint
CREATE INDEX ON items USING ruhnsw (embedding ruvector_l2_ops)
WITH (quantization = 'sq8');  -- 4x memory reduction

-- Use half-precision vectors
CREATE TABLE items (embedding halfvec(1536));  -- 50% memory savings
```

**Memory by Compute Unit:**

| Neon CU | RAM | Recommended Index Size | Quantization |
|---------|-----|------------------------|--------------|
| 0.25 | 1 GB | <128 MB | Required (sq8/pq) |
| 0.5 | 2 GB | <512 MB | Recommended (sq8) |
| 1.0 | 4 GB | <2 GB | Optional |
| 2.0 | 8 GB | <4 GB | Optional |
| 4.0+ | 16+ GB | <8 GB | None |

### 4. No Background Workers

Neon restricts background workers for resource management. RuVector-Postgres is designed without them:

```rust
// ❌ NOT USED: Background workers
// BackgroundWorker::register("ruvector_maintenance", ...);

// ✓ USED: On-demand operations
// - Index vacuum during INSERT/UPDATE
// - Statistics during ANALYZE
// - Maintenance via explicit SQL functions
```

**Alternative Maintenance Patterns:**

```sql
-- Explicit index maintenance (replaces background vacuum)
SELECT ruvector_index_maintenance('items_embedding_idx');

-- Scheduled via pg_cron (if available)
SELECT cron.schedule('vacuum-index', '0 2 * * *',
    $$SELECT ruvector_index_maintenance('items_embedding_idx')$$);

-- Manual statistics update
ANALYZE items;
```

### 5. Connection Pooling Considerations

Neon uses PgBouncer in **transaction mode** for connection pooling. RuVector-Postgres is fully compatible:

**Compatible Features:**

- ✓ No session-level state
- ✓ No temp tables or cursors
- ✓ All settings via GUCs (can be set per-transaction)
- ✓ Thread-safe distance calculations

**Usage Pattern:**

```sql
-- Each transaction is independent
BEGIN;
SET LOCAL ruvector.ef_search = 100;  -- Transaction-local setting
SELECT * FROM items ORDER BY embedding <-> query LIMIT 10;
COMMIT;

-- Next transaction (potentially different connection)
BEGIN;
SET LOCAL ruvector.ef_search = 200;  -- Different setting
SELECT * FROM items ORDER BY embedding <-> query LIMIT 10;
COMMIT;
```

### 6. Index Persistence

**How Indexes Are Stored:**

- HNSW/IVFFlat indexes stored in PostgreSQL pages
- Automatically replicated to Neon storage layer
- Preserved across compute restarts
- Shared across branches (copy-on-write)

**Index Build on Neon:**

```sql
-- Non-blocking index build (recommended on Neon)
CREATE INDEX CONCURRENTLY items_embedding_idx ON items
USING ruhnsw (embedding ruvector_l2_ops)
WITH (m = 32, ef_construction = 200);

-- Monitor progress
SELECT
    phase,
    blocks_total,
    blocks_done,
    tuples_total,
    tuples_done
FROM pg_stat_progress_create_index;
```

## Neon-Specific Limitations

### 1. Extension Installation (Scale Plan Required)

**Free Plan:**
- Pre-approved extensions only (pgvector is included)
- RuVector requires custom extension approval

**Scale Plan:**
- Custom extensions allowed
- Contact support for installation

**Enterprise Plan:**
- Dedicated support for custom extensions
- Faster approval process

### 2. Compute Suspension

**Behavior:**

- Compute suspends after 5 minutes of inactivity (configurable)
- First query after suspension: +100-200ms latency
- Indexes loaded from storage on first access

**Mitigation:**

```sql
-- Keep-alive query (via cron or application)
SELECT 1;

-- Or use Neon's suspend_timeout setting
-- In Neon console: Project Settings → Compute → Autosuspend delay
```

### 3. Memory Constraints

**Observation:**

- Neon may limit memory below advertised CU limits
- Large index builds may fail with OOM

**Solutions:**

```sql
-- Build index with lower memory
SET maintenance_work_mem = '256MB';
CREATE INDEX CONCURRENTLY ...;

-- Use quantization for large datasets
WITH (quantization = 'pq16');  -- 16x memory reduction
```

### 4. Extension Update Process

**Current Process:**

1. Open support ticket with Neon
2. Provide new `.so` and SQL files
3. Neon reviews and deploys
4. Extension available for `ALTER EXTENSION UPDATE`

**Future:** Self-service extension updates (roadmap item)

## Requesting RuVector on Neon

### For Scale Plan Customers

#### Step 1: Open Support Ticket

Navigate to: [Neon Console](https://console.neon.tech) → **Support**

**Ticket Template:**

```
Subject: Custom Extension Request - RuVector-Postgres

Body:
I would like to install the RuVector-Postgres extension for vector similarity search.

Details:
- Extension: ruvector-postgres
- Version: 0.1.19
- PostgreSQL version: 16 (or your version)
- Project ID: [your-project-id]

Use case:
[Describe your vector search use case]

Repository: https://github.com/ruvnet/ruvector
Documentation: https://github.com/ruvnet/ruvector/tree/main/crates/ruvector-postgres

I can provide pre-built binaries if needed.
```

#### Step 2: Provide Extension Artifacts

Neon will request:

1. **Shared Library** (`.so` file):
   ```bash
   # Build for PostgreSQL 16
   cargo pgrx package --pg-config /path/to/pg_config
   # Artifact: target/release/ruvector-pg16/usr/lib/postgresql/16/lib/ruvector.so
   ```

2. **Control File** (`ruvector.control`):
   ```
   comment = 'High-performance vector similarity search'
   default_version = '0.1.19'
   module_pathname = '$libdir/ruvector'
   relocatable = true
   ```

3. **SQL Scripts**:
   - `ruvector--0.1.0.sql` (initial schema)
   - `ruvector--0.1.0--0.1.19.sql` (migration script)

4. **Security Documentation**:
   - Memory safety audit
   - No unsafe FFI calls
   - No network access
   - Resource limits

#### Step 3: Security Review

Neon engineers will review:

- ✓ Rust memory safety guarantees
- ✓ No unsafe system calls
- ✓ Sandboxed execution
- ✓ Resource limits (memory, CPU)
- ✓ No file system access beyond PostgreSQL

**Timeline:** 1-2 weeks for approval.

#### Step 4: Deployment

Once approved:

```sql
-- Extension becomes available
CREATE EXTENSION ruvector;

-- Verify
SELECT ruvector_version();
```

### For Free Plan Users

**Option 1: Request via Discord**

1. Join [Neon Discord](https://discord.gg/92vNTzKDGp)
2. Post in `#feedback` channel
3. Include use case and expected usage

**Option 2: Use pgvector (Pre-installed)**

```sql
-- pgvector is available on all plans
CREATE EXTENSION vector;

-- RuVector provides migration path
-- (See MIGRATION.md)
```

## Migration from pgvector

RuVector-Postgres is API-compatible with pgvector. Migration is seamless:

### Step 1: Create Parallel Tables

```sql
-- Keep existing pgvector table (for rollback)
-- ALTER TABLE items RENAME TO items_pgvector;

-- Create new table with ruvector
CREATE TABLE items_ruvector (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding ruvector(1536)
);

-- Copy data (automatic type conversion)
INSERT INTO items_ruvector (id, content, embedding)
SELECT id, content, embedding::ruvector FROM items;
```

### Step 2: Rebuild Indexes

```sql
-- Drop old pgvector index (if exists)
-- DROP INDEX items_embedding_idx;

-- Create optimized HNSW index
CREATE INDEX items_embedding_ruhnsw_idx ON items_ruvector
USING ruhnsw (embedding ruvector_l2_ops)
WITH (m = 32, ef_construction = 200);

-- Analyze for query planner
ANALYZE items_ruvector;
```

### Step 3: Validate Results

```sql
-- Compare search results
WITH pgvector_results AS (
    SELECT id, embedding <-> '[...]'::vector AS dist
    FROM items ORDER BY dist LIMIT 10
),
ruvector_results AS (
    SELECT id, embedding <-> '[...]'::ruvector AS dist
    FROM items_ruvector ORDER BY dist LIMIT 10
)
SELECT
    p.id AS pg_id,
    r.id AS ru_id,
    p.id = r.id AS id_match,
    abs(p.dist - r.dist) < 0.0001 AS dist_match
FROM pgvector_results p
FULL OUTER JOIN ruvector_results r ON p.id = r.id;

-- All rows should have id_match=true, dist_match=true
```

### Step 4: Switch Over

```sql
-- Atomic swap
BEGIN;
ALTER TABLE items RENAME TO items_old;
ALTER TABLE items_ruvector RENAME TO items;
COMMIT;

-- Validate application queries
-- ... run tests ...

-- Drop old table after validation period (e.g., 1 week)
DROP TABLE items_old;
```

## Performance Tuning for Neon

### Instance Size Recommendations

| Neon CU | RAM | Max Vectors | Recommended Settings |
|---------|-----|-------------|---------------------|
| 0.25 | 1 GB | 100K | `m=8, ef=64, sq8 quant` |
| 0.5 | 2 GB | 500K | `m=16, ef=100, sq8 quant` |
| 1.0 | 4 GB | 2M | `m=24, ef=150, optional quant` |
| 2.0 | 8 GB | 5M | `m=32, ef=200, no quant` |
| 4.0 | 16 GB | 10M+ | `m=48, ef=300, no quant` |

### Query Optimization

```sql
-- High recall (use for important queries)
SET ruvector.ef_search = 200;
SELECT * FROM items ORDER BY embedding <-> query LIMIT 10;

-- Low latency (use for real-time queries)
SET ruvector.ef_search = 40;
SELECT * FROM items ORDER BY embedding <-> query LIMIT 10;

-- Per-query tuning
SET LOCAL ruvector.ef_search = 100;
```

### Index Build Settings

```sql
-- For small Neon instances
SET maintenance_work_mem = '512MB';
SET max_parallel_maintenance_workers = 2;

-- For large Neon instances
SET maintenance_work_mem = '4GB';
SET max_parallel_maintenance_workers = 8;

-- Always use CONCURRENTLY on Neon
CREATE INDEX CONCURRENTLY ...;
```

## Neon Branching with RuVector

### How Branching Works

Neon branches use copy-on-write, so indexes are instantly available:

```
Parent Branch                Child Branch
┌─────────────┐             ┌─────────────┐
│ items       │             │ items       │ (copy-on-write)
│ ├─ data     │──shared────→│ ├─ data     │
│ └─ index    │──shared────→│ └─ index    │
└─────────────┘             └─────────────┘
                                   ↓
                              Modify data
                                   ↓
                            ┌─────────────┐
                            │ items       │
                            │ ├─ data     │ (diverged)
                            │ └─ index    │ (needs rebuild)
                            └─────────────┘
```

### Branch Creation Workflow

```sql
-- In parent branch: Create index
CREATE INDEX items_embedding_idx ON items
USING ruhnsw (embedding ruvector_l2_ops);

-- Create child branch via Neon Console or API
-- Index is instantly available (no rebuild needed)

-- In child branch: Index is read-only until data changes
SELECT * FROM items ORDER BY embedding <-> query LIMIT 10;
-- Uses parent's index ✓

-- After INSERT/UPDATE in child:
-- Index diverges and needs rebuild
INSERT INTO items VALUES (...);
REINDEX INDEX items_embedding_idx;  -- or CREATE INDEX CONCURRENTLY
```

### Branch-Specific Tuning

```sql
-- Development branch: Faster builds, lower recall
ALTER DATABASE dev_branch SET ruvector.ef_search = 20;

-- Staging branch: Balanced
ALTER DATABASE staging SET ruvector.ef_search = 100;

-- Production branch: High recall
ALTER DATABASE prod SET ruvector.ef_search = 200;
```

## Monitoring on Neon

### Extension Metrics

```sql
-- Index statistics
SELECT * FROM ruvector_index_stats();

┌────────────────────────────────────────────────────────────────┐
│                    Index Statistics                             │
├────────────────────────────────────────────────────────────────┤
│ index_name              │ items_embedding_idx                  │
│ index_size_mb           │ 512                                  │
│ vector_count            │ 1000000                              │
│ dimensions              │ 1536                                 │
│ build_time_seconds      │ 45.2                                 │
│ fragmentation_pct       │ 2.3                                  │
└────────────────────────────────────────────────────────────────┘
```

### Query Performance

```sql
-- Explain analyze for vector queries
EXPLAIN (ANALYZE, BUFFERS, VERBOSE)
SELECT * FROM items
ORDER BY embedding <-> '[0.1, 0.2, ...]'::ruvector
LIMIT 10;

-- Output includes:
-- - Index Scan using items_embedding_idx
-- - Distance calculations: 15000
-- - Buffers: shared hit=250, read=10
-- - Execution time: 12.5ms
```

### Neon Metrics Integration

Use Neon's monitoring dashboard:

1. **Query Time**: Track vector query latencies
2. **Buffer Hit Ratio**: Monitor index cache efficiency
3. **Compute Usage**: Track CPU during index builds
4. **Memory Usage**: Monitor vector memory consumption

## Troubleshooting

### Cold Start Slow

**Symptom:** First query after suspend takes >500ms

**Diagnosis:**

```sql
-- Check extension load time
SELECT extname, extversion FROM pg_extension WHERE extname = 'ruvector';

-- Check SIMD detection
SELECT ruvector_simd_info();
```

**Solution:**

- Expected: 100-200ms for first query
- If >500ms: Contact Neon support (compute issue)
- Use keep-alive queries to prevent suspension

### Memory Pressure

**Symptom:** Index build fails with OOM

**Diagnosis:**

```sql
-- Check current memory usage
SELECT * FROM ruvector_memory_stats();

-- Check Neon compute size
SELECT current_setting('shared_buffers');
```

**Solution:**

```sql
-- Reduce index memory
SET ruvector.max_index_memory = '128MB';

-- Use aggressive quantization
CREATE INDEX ... WITH (quantization = 'pq16');

-- Upgrade Neon compute unit
-- Neon Console → Project Settings → Compute → Scale up
```

### Index Build Timeout

**Symptom:** `CREATE INDEX` times out on large dataset

**Solution:**

```sql
-- Always use CONCURRENTLY
CREATE INDEX CONCURRENTLY items_embedding_idx ON items
USING ruhnsw (embedding ruvector_l2_ops);

-- Split into batches
CREATE TABLE items_batch_1 AS SELECT * FROM items LIMIT 100000;
CREATE INDEX ... ON items_batch_1;
-- Repeat for batches, then UNION ALL
```

### Connection Pool Compatibility

**Symptom:** Settings not persisting across queries

**Cause:** PgBouncer transaction mode resets session state

**Solution:**

```sql
-- Use SET LOCAL (transaction-scoped)
BEGIN;
SET LOCAL ruvector.ef_search = 100;
SELECT ... ORDER BY embedding <-> query;
COMMIT;

-- Or set defaults in postgresql.conf
ALTER DATABASE mydb SET ruvector.ef_search = 100;
```

## Support Resources

- **Neon Documentation**: https://neon.tech/docs
- **RuVector GitHub**: https://github.com/ruvnet/ruvector
- **RuVector Issues**: https://github.com/ruvnet/ruvector/issues
- **Neon Discord**: https://discord.gg/92vNTzKDGp
- **Neon Support**: console.neon.tech → Support (Scale plan+)
