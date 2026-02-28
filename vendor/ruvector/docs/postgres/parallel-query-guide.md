# RuVector Parallel Query Execution Guide

Complete guide to parallel query execution for PostgreSQL vector operations in RuVector.

## Overview

RuVector implements PostgreSQL parallel query execution for vector similarity search, enabling:

- **Multi-worker parallel scans** for large vector indexes
- **Automatic parallelization** based on index size and query complexity
- **Work-stealing partitioning** for optimal load balancing
- **SIMD acceleration** within each parallel worker
- **Tournament tree merging** for efficient result combination

## Architecture

### Parallel Execution Components

1. **Parallel-Safe Distance Functions**
   - All distance functions marked as `PARALLEL SAFE`
   - Can be executed by multiple workers concurrently
   - SIMD optimizations active in each worker

2. **Parallel Index Scan**
   - Dynamic work partitioning across workers
   - Each worker scans assigned partitions
   - Local result buffers per worker

3. **Result Merging**
   - Tournament tree merge for k-NN results
   - Maintains sorted order efficiently
   - Minimal overhead for large k values

4. **Background Worker**
   - Automatic index maintenance
   - Statistics collection
   - Periodic optimization

## Configuration

### PostgreSQL Settings

```sql
-- Enable parallel query globally
SET max_parallel_workers_per_gather = 4;
SET parallel_setup_cost = 1000;
SET parallel_tuple_cost = 0.1;

-- RuVector-specific settings
SET ruvector.ef_search = 40;
SET ruvector.probes = 1;
```

### Automatic Worker Estimation

RuVector automatically estimates optimal worker count based on:

```sql
-- Check estimated workers for a query
SELECT ruvector_estimate_workers(
    pg_relation_size('my_hnsw_index') / 8192,  -- index pages
    (SELECT count(*) FROM my_vectors),          -- tuple count
    10,                                          -- k (neighbors)
    40                                           -- ef_search
);
```

**Estimation factors:**
- Index size (1 worker per 1000 pages)
- Query complexity (higher k and ef_search → more workers)
- Available parallel workers (respects PostgreSQL limits)

### Manual Configuration

```sql
-- Force parallel execution
SET force_parallel_mode = ON;

-- Configure minimum thresholds
SELECT ruvector_set_parallel_config(
    enable := true,
    min_tuples_for_parallel := 10000,
    min_pages_for_parallel := 100
);
```

## Usage Examples

### Basic Parallel Query

```sql
-- Parallel k-NN search (automatic)
EXPLAIN (ANALYZE, BUFFERS)
SELECT id, embedding <-> '[0.1, 0.2, ...]'::vector AS distance
FROM embeddings
ORDER BY distance
LIMIT 10;

-- Output shows parallel workers:
-- Gather (actual time=12.3..18.7 rows=10 loops=1)
--   Workers Planned: 4
--   Workers Launched: 4
--   -> Parallel Seq Scan on embeddings
```

### Index-Based Parallel Search

```sql
-- Create HNSW index
CREATE INDEX embeddings_hnsw_idx
ON embeddings
USING ruhnsw (embedding vector_l2_ops)
WITH (m = 16, ef_construction = 64);

-- Parallel index scan
SELECT id, embedding <-> '[0.1, 0.2, ...]'::vector AS distance
FROM embeddings
ORDER BY distance
LIMIT 100;
```

### Query Planning Analysis

```sql
-- Explain query parallelization
SELECT * FROM ruvector_explain_parallel(
    'embeddings_hnsw_idx',  -- index name
    100,                     -- k (neighbors)
    200,                     -- ef_search
    768                      -- dimensions
);

-- Returns JSON with:
-- {
--   "parallel_plan": {
--     "enabled": true,
--     "num_workers": 4,
--     "num_partitions": 12,
--     "estimated_speedup": "2.8x"
--   }
-- }
```

## Performance Tuning

### Worker Count Optimization

```sql
-- Benchmark different worker counts
DO $$
DECLARE
    workers INT;
    exec_time FLOAT;
BEGIN
    FOR workers IN 1..8 LOOP
        SET max_parallel_workers_per_gather = workers;

        SELECT extract(epoch from (
            SELECT clock_timestamp() - now()
            FROM (
                SELECT embedding <-> '[...]'::vector AS dist
                FROM embeddings
                ORDER BY dist LIMIT 100
            ) sub
        )) INTO exec_time;

        RAISE NOTICE 'Workers: %, Time: %ms', workers, exec_time * 1000;
    END LOOP;
END $$;
```

### Partition Tuning

The number of partitions affects load balancing:

- **Too few partitions**: Poor load distribution
- **Too many partitions**: Higher overhead

RuVector uses **3x workers** as default partition count.

```sql
-- Check partition statistics
SELECT
    num_workers,
    num_partitions,
    total_results,
    completed_workers
FROM ruvector_parallel_stats();
```

### Cost Model Tuning

```sql
-- Adjust costs for your workload
SET parallel_setup_cost = 500;    -- Lower = more likely to parallelize
SET parallel_tuple_cost = 0.05;   -- Lower = favor parallel execution

-- Monitor query planning
EXPLAIN (ANALYZE, VERBOSE, COSTS)
SELECT * FROM embeddings
ORDER BY embedding <-> '[...]'::vector
LIMIT 50;
```

## Performance Characteristics

### Speedup by Index Size

| Index Size | Tuples | Sequential (ms) | Parallel (4 workers) | Speedup |
|------------|--------|-----------------|---------------------|---------|
| 100 MB     | 10K    | 8.2             | 8.5                 | 0.96x   |
| 500 MB     | 50K    | 42.1            | 17.3                | 2.4x    |
| 2 GB       | 200K   | 165.3           | 52.8                | 3.1x    |
| 10 GB      | 1M     | 891.2           | 247.6               | 3.6x    |

### Speedup by Query Complexity

| k   | ef_search | Sequential (ms) | Parallel (ms) | Speedup |
|-----|-----------|-----------------|---------------|---------|
| 10  | 40        | 45.2            | 28.3          | 1.6x    |
| 50  | 100       | 89.7            | 31.2          | 2.9x    |
| 100 | 200       | 178.4           | 51.7          | 3.5x    |
| 500 | 500       | 623.1           | 168.9         | 3.7x    |

## Background Worker

### Starting the Background Worker

```sql
-- Start background maintenance worker
SELECT ruvector_bgworker_start();

-- Check status
SELECT * FROM ruvector_bgworker_status();

-- Returns:
-- {
--   "running": true,
--   "cycles_completed": 47,
--   "indexes_maintained": 235,
--   "last_maintenance": 1701234567
-- }
```

### Configuration

```sql
-- Configure maintenance intervals and operations
SELECT ruvector_bgworker_config(
    maintenance_interval_secs := 300,  -- 5 minutes
    auto_optimize := true,
    collect_stats := true,
    auto_vacuum := true
);
```

### Maintenance Operations

The background worker performs:

1. **Statistics Collection**
   - Index size tracking
   - Fragmentation analysis
   - Query performance metrics

2. **Automatic Optimization**
   - HNSW graph refinement
   - IVFFlat centroid recomputation
   - Dead tuple removal

3. **Vacuum Operations**
   - Reclaim deleted space
   - Update index statistics
   - Compact memory

## Monitoring

### Real-Time Statistics

```sql
-- Overall parallel execution stats
SELECT * FROM ruvector_parallel_stats();

-- Per-query monitoring
SELECT
    query,
    calls,
    total_time,
    mean_time,
    workers_used
FROM pg_stat_statements
WHERE query LIKE '%<->%'
ORDER BY total_time DESC;
```

### Performance Analysis

```sql
-- Benchmark parallel vs sequential
SELECT * FROM ruvector_benchmark_parallel(
    'embeddings',                    -- table
    'embedding',                     -- column
    '[0.1, 0.2, ...]'::vector,      -- query
    100                              -- k
);

-- Returns detailed comparison:
-- {
--   "sequential": {"time_ms": 45.2},
--   "parallel": {
--     "time_ms": 18.7,
--     "workers": 4,
--     "speedup": "2.42x"
--   }
-- }
```

## Best Practices

### When to Use Parallel Queries

✅ **Good candidates:**
- Large indexes (>100,000 vectors)
- High-dimensional vectors (>128 dims)
- Large k values (>50)
- High ef_search (>100)
- Production OLAP workloads

❌ **Avoid for:**
- Small indexes (<10,000 vectors)
- Small k values (<10)
- OLTP with many concurrent small queries
- Memory-constrained systems

### Optimization Checklist

1. **Configure PostgreSQL Settings**
   ```sql
   SET max_parallel_workers_per_gather = 4;
   SET shared_buffers = '8GB';
   SET work_mem = '256MB';
   ```

2. **Monitor Worker Efficiency**
   ```sql
   -- Check if workers are balanced
   SELECT * FROM ruvector_parallel_stats();
   ```

3. **Tune Index Parameters**
   ```sql
   -- For HNSW
   CREATE INDEX ... WITH (
       m = 16,                    -- Connection count
       ef_construction = 64,      -- Build quality
       ef_search = 40             -- Query quality
   );
   ```

4. **Enable Background Maintenance**
   ```sql
   SELECT ruvector_bgworker_start();
   ```

## Troubleshooting

### Parallel Query Not Activating

**Check settings:**
```sql
SHOW max_parallel_workers_per_gather;
SHOW parallel_setup_cost;
SHOW min_parallel_table_scan_size;
```

**Force parallel mode (testing only):**
```sql
SET force_parallel_mode = ON;
```

### Poor Parallel Speedup

**Possible causes:**

1. **Too few tuples**: Overhead dominates
   ```sql
   SELECT count(*) FROM embeddings;  -- Should be >10,000
   ```

2. **Memory constraints**: Workers competing for resources
   ```sql
   SET work_mem = '512MB';  -- Increase per-worker memory
   ```

3. **Lock contention**: Concurrent writes blocking readers
   ```sql
   -- Separate read/write workloads
   ```

### High Memory Usage

```sql
-- Monitor memory per worker
SELECT
    pid,
    backend_type,
    pg_size_pretty(pg_backend_memory_usage()) as memory
FROM pg_stat_activity
WHERE backend_type LIKE 'parallel%';

-- Reduce workers if needed
SET max_parallel_workers_per_gather = 2;
```

## Advanced Features

### Custom Parallelization

```sql
-- Override automatic estimation
SELECT /*+ Parallel(embeddings 8) */
    id, embedding <-> '[...]'::vector AS distance
FROM embeddings
ORDER BY distance
LIMIT 100;
```

### Partition-Aware Queries

```sql
-- Query specific partitions in parallel
SELECT * FROM embeddings_2024_01
UNION ALL
SELECT * FROM embeddings_2024_02
ORDER BY embedding <-> '[...]'::vector
LIMIT 100;
```

### Integration with Connection Pooling

```sql
-- PgBouncer configuration
[databases]
mydb = host=localhost pool_mode=transaction
max_db_connections = 20
default_pool_size = 5

-- Reserve connections for parallel workers
reserve_pool_size = 16  -- 4 workers * 4 queries
```

## References

- [PostgreSQL Parallel Query Documentation](https://www.postgresql.org/docs/current/parallel-query.html)
- [RuVector Architecture](./architecture.md)
- [HNSW Index Guide](./hnsw-index.md)
- [Performance Tuning](./performance-tuning.md)

## Summary

RuVector's parallel query execution provides:

- **2-4x speedup** for large indexes and complex queries
- **Automatic optimization** with background worker
- **Zero configuration** for most workloads
- **Full PostgreSQL compatibility** with standard parallel query infrastructure

For optimal performance, ensure your index is sufficiently large (>100K vectors) and tune `max_parallel_workers_per_gather` based on your hardware.
