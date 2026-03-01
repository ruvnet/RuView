-- ============================================================================
-- RuVector Parallel Query Execution Examples
-- ============================================================================
--
-- This file demonstrates how to use RuVector's parallel query execution
-- for high-performance vector similarity search in PostgreSQL.

-- ============================================================================
-- Setup
-- ============================================================================

-- Load the RuVector extension
CREATE EXTENSION IF NOT EXISTS ruvector;

-- Configure PostgreSQL for parallel execution
SET max_parallel_workers_per_gather = 4;
SET parallel_setup_cost = 1000;
SET parallel_tuple_cost = 0.1;
SET min_parallel_table_scan_size = '8MB';

-- Create a sample table with vector embeddings
CREATE TABLE embeddings (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding vector(768),
    created_at TIMESTAMP DEFAULT NOW()
);

-- Insert sample data (simulating 100K embeddings)
-- In production, you would load real embeddings
INSERT INTO embeddings (content, embedding)
SELECT
    'Document ' || i,
    -- Generate random 768-dimensional vector
    array_to_string(array_agg(random()::real), ',')::vector(768)
FROM generate_series(1, 100000) i,
     generate_series(1, 768) j
GROUP BY i;

-- ============================================================================
-- Index Creation with Parallel-Safe Support
-- ============================================================================

-- Create HNSW index for L2 distance
CREATE INDEX embeddings_hnsw_l2_idx
ON embeddings
USING ruhnsw (embedding vector_l2_ops)
WITH (
    m = 16,                  -- Connections per node
    ef_construction = 64     -- Build-time quality
);

-- Create HNSW index for cosine distance
CREATE INDEX embeddings_hnsw_cosine_idx
ON embeddings
USING ruhnsw (embedding vector_cosine_ops)
WITH (
    m = 16,
    ef_construction = 64
);

-- ============================================================================
-- Basic Parallel Query Examples
-- ============================================================================

-- Example 1: Simple k-NN search with automatic parallelization
-- The query planner will automatically use parallel workers if beneficial
EXPLAIN (ANALYZE, BUFFERS, VERBOSE)
SELECT
    id,
    content,
    embedding <-> '[0.1, 0.2, ...]'::vector(768) AS distance
FROM embeddings
ORDER BY distance
LIMIT 10;

-- Example 2: Larger k with parallel execution
SELECT
    id,
    content,
    embedding <-> '[0.1, 0.2, ...]'::vector(768) AS distance
FROM embeddings
ORDER BY distance
LIMIT 100;

-- Example 3: Cosine distance search
SELECT
    id,
    content,
    embedding <=> '[0.1, 0.2, ...]'::vector(768) AS distance
FROM embeddings
ORDER BY distance
LIMIT 50;

-- ============================================================================
-- Monitoring and Diagnostics
-- ============================================================================

-- Check parallel query capabilities
SELECT * FROM ruvector_parallel_info();

-- Estimate workers for a specific query
SELECT ruvector_estimate_workers(
    pg_relation_size('embeddings_hnsw_l2_idx') / 8192,  -- pages
    (SELECT count(*) FROM embeddings),                   -- tuples
    100,                                                  -- k
    100                                                   -- ef_search
) AS recommended_workers;

-- Explain how query will be parallelized
SELECT * FROM ruvector_explain_parallel(
    'embeddings_hnsw_l2_idx',
    100,   -- k
    100,   -- ef_search
    768    -- dimensions
);

-- Get parallel execution statistics
SELECT * FROM ruvector_parallel_stats();

-- ============================================================================
-- Performance Benchmarking
-- ============================================================================

-- Benchmark parallel vs sequential execution
SELECT * FROM ruvector_benchmark_parallel(
    'embeddings',
    'embedding',
    '[0.1, 0.2, ...]'::vector(768),
    100
);

-- Compare different worker counts
DO $$
DECLARE
    workers INT;
    start_time TIMESTAMP;
    end_time TIMESTAMP;
    duration INTERVAL;
BEGIN
    CREATE TEMP TABLE benchmark_results (
        workers INT,
        duration_ms FLOAT
    );

    FOR workers IN 1..8 LOOP
        -- Set worker count
        EXECUTE 'SET max_parallel_workers_per_gather = ' || workers;

        -- Run query and measure time
        start_time := clock_timestamp();

        PERFORM id
        FROM embeddings
        ORDER BY embedding <-> '[0.1, 0.2, ...]'::vector(768)
        LIMIT 100;

        end_time := clock_timestamp();
        duration := end_time - start_time;

        -- Record result
        INSERT INTO benchmark_results
        VALUES (workers, EXTRACT(EPOCH FROM duration) * 1000);

        RAISE NOTICE 'Workers: %, Duration: %ms', workers, EXTRACT(EPOCH FROM duration) * 1000;
    END LOOP;

    -- Show results
    SELECT * FROM benchmark_results ORDER BY workers;
END $$;

-- ============================================================================
-- Advanced Query Patterns
-- ============================================================================

-- Example 4: Filter + k-NN with parallel execution
EXPLAIN (ANALYZE)
SELECT
    id,
    content,
    created_at,
    embedding <-> '[0.1, 0.2, ...]'::vector(768) AS distance
FROM embeddings
WHERE created_at > NOW() - INTERVAL '7 days'
ORDER BY distance
LIMIT 50;

-- Example 5: Join with parallel execution
CREATE TABLE categories (
    id SERIAL PRIMARY KEY,
    name TEXT,
    embedding vector(768)
);

-- Find similar documents across categories
SELECT
    e.id,
    e.content,
    c.name AS category,
    e.embedding <-> c.embedding AS distance
FROM embeddings e
CROSS JOIN LATERAL (
    SELECT name, embedding
    FROM categories
    ORDER BY categories.embedding <-> e.embedding
    LIMIT 1
) c
ORDER BY distance
LIMIT 100;

-- Example 6: Aggregate queries with parallel execution
SELECT
    bucket,
    count(*) AS doc_count,
    avg(distance) AS avg_distance
FROM (
    SELECT
        width_bucket(
            embedding <-> '[0.1, 0.2, ...]'::vector(768),
            0, 2, 10
        ) AS bucket,
        embedding <-> '[0.1, 0.2, ...]'::vector(768) AS distance
    FROM embeddings
) sub
GROUP BY bucket
ORDER BY bucket;

-- ============================================================================
-- Background Worker Management
-- ============================================================================

-- Start background maintenance worker
SELECT ruvector_bgworker_start();

-- Check background worker status
SELECT * FROM ruvector_bgworker_status();

-- Configure background worker
SELECT ruvector_bgworker_config(
    maintenance_interval_secs := 300,  -- 5 minutes
    auto_optimize := true,
    collect_stats := true,
    auto_vacuum := true
);

-- Stop background worker
-- SELECT ruvector_bgworker_stop();

-- ============================================================================
-- Configuration Tuning
-- ============================================================================

-- Configure parallel execution behavior
SELECT ruvector_set_parallel_config(
    enable := true,
    min_tuples_for_parallel := 10000,
    min_pages_for_parallel := 100
);

-- Adjust HNSW search parameters
SET ruvector.ef_search = 100;  -- Higher = better recall, slower

-- Adjust PostgreSQL parallel query costs
SET parallel_setup_cost = 500;     -- Lower = more likely to parallelize
SET parallel_tuple_cost = 0.05;    -- Lower = favor parallel execution

-- ============================================================================
-- Query Plan Analysis
-- ============================================================================

-- Analyze query plan with parallel workers
EXPLAIN (ANALYZE, BUFFERS, VERBOSE, COSTS, TIMING)
SELECT
    id,
    embedding <-> '[0.1, 0.2, ...]'::vector(768) AS distance
FROM embeddings
ORDER BY distance
LIMIT 100;

-- Compare with forced sequential execution
SET max_parallel_workers_per_gather = 0;
EXPLAIN (ANALYZE)
SELECT
    id,
    embedding <-> '[0.1, 0.2, ...]'::vector(768) AS distance
FROM embeddings
ORDER BY distance
LIMIT 100;

-- Reset to parallel
SET max_parallel_workers_per_gather = 4;

-- ============================================================================
-- Production Best Practices
-- ============================================================================

-- 1. Create indexes with appropriate parameters
CREATE INDEX CONCURRENTLY embeddings_hnsw_idx
ON embeddings
USING ruhnsw (embedding vector_l2_ops)
WITH (
    m = 16,
    ef_construction = 64
);

-- 2. Analyze table statistics
ANALYZE embeddings;

-- 3. Monitor query performance
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;

SELECT
    query,
    calls,
    mean_exec_time,
    total_exec_time,
    rows
FROM pg_stat_statements
WHERE query LIKE '%<->%'
ORDER BY mean_exec_time DESC
LIMIT 10;

-- 4. Check index usage
SELECT
    schemaname,
    tablename,
    indexname,
    idx_scan,
    idx_tup_read,
    idx_tup_fetch
FROM pg_stat_user_indexes
WHERE indexname LIKE '%hnsw%';

-- 5. Monitor memory usage
SELECT
    pid,
    backend_type,
    pg_size_pretty(pg_backend_memory_contexts()) as memory_context
FROM pg_stat_activity
WHERE backend_type LIKE 'parallel%';

-- ============================================================================
-- Performance Testing Queries
-- ============================================================================

-- Test 1: Small k (should be fast even without parallelism)
\timing on
SELECT id, embedding <-> '[0.1, 0.2, ...]'::vector(768) AS distance
FROM embeddings
ORDER BY distance
LIMIT 10;

-- Test 2: Medium k (benefits from parallelism)
SELECT id, embedding <-> '[0.1, 0.2, ...]'::vector(768) AS distance
FROM embeddings
ORDER BY distance
LIMIT 100;

-- Test 3: Large k (maximum benefit from parallelism)
SELECT id, embedding <-> '[0.1, 0.2, ...]'::vector(768) AS distance
FROM embeddings
ORDER BY distance
LIMIT 1000;

\timing off

-- ============================================================================
-- Cleanup
-- ============================================================================

-- Drop temporary tables
DROP TABLE IF EXISTS benchmark_results;

-- Optionally drop the sample table
-- DROP TABLE IF EXISTS embeddings CASCADE;
-- DROP TABLE IF EXISTS categories CASCADE;

-- ============================================================================
-- Additional Functions
-- ============================================================================

-- Get RuVector version and capabilities
SELECT ruvector_version();
SELECT ruvector_simd_info();

-- Get memory statistics
SELECT * FROM ruvector_memory_stats();

-- Get index information
SELECT * FROM ruhnsw_index_info('embeddings_hnsw_l2_idx');

-- Perform manual index maintenance
SELECT ruvector_index_maintenance('embeddings_hnsw_l2_idx');
