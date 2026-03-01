-- Realistic workload benchmark for ruvector vs pgvector
-- This script tests common operations with realistic dataset sizes

\timing on
\set ECHO all

-- Configuration
\set num_vectors 1000000
\set num_queries 1000
\set dims 1536
\set k 10

BEGIN;

-- ============================================================================
-- Setup Test Tables
-- ============================================================================

DROP TABLE IF EXISTS vectors_ruvector CASCADE;
DROP TABLE IF EXISTS vectors_pgvector CASCADE;
DROP TABLE IF EXISTS queries CASCADE;

-- Create tables
CREATE TABLE vectors_ruvector (
    id SERIAL PRIMARY KEY,
    embedding ruvector(:dims),
    metadata JSONB
);

CREATE TABLE vectors_pgvector (
    id SERIAL PRIMARY KEY,
    embedding vector(:dims),
    metadata JSONB
);

CREATE TABLE queries (
    id SERIAL PRIMARY KEY,
    query_vector ruvector(:dims)
);

-- ============================================================================
-- Generate Test Data
-- ============================================================================

\echo 'Generating test data...'

-- Insert vectors (ruvector)
INSERT INTO vectors_ruvector (embedding, metadata)
SELECT
    array_to_ruvector(ARRAY(
        SELECT random()::real
        FROM generate_series(1, :dims)
    )),
    jsonb_build_object('category', i % 100)
FROM generate_series(1, :num_vectors) i;

-- Insert vectors (pgvector)
INSERT INTO vectors_pgvector (embedding, metadata)
SELECT
    ARRAY(
        SELECT random()::real
        FROM generate_series(1, :dims)
    )::vector(:dims),
    jsonb_build_object('category', i % 100)
FROM generate_series(1, :num_vectors) i;

-- Generate query vectors
INSERT INTO queries (query_vector)
SELECT
    array_to_ruvector(ARRAY(
        SELECT random()::real
        FROM generate_series(1, :dims)
    ))
FROM generate_series(1, :num_queries);

COMMIT;

-- ============================================================================
-- Benchmark 1: Sequential Scan (No Index)
-- ============================================================================

\echo ''
\echo '=== Benchmark 1: Sequential Scan (No Index) ==='
\echo ''

-- Get a test query
\set test_query 'SELECT query_vector FROM queries WHERE id = 1'

-- RuVector scan
\echo 'RuVector sequential scan (p50, p99 latency):'
SELECT
    percentile_cont(0.5) WITHIN GROUP (ORDER BY duration) AS p50_ms,
    percentile_cont(0.99) WITHIN GROUP (ORDER BY duration) AS p99_ms,
    AVG(duration) AS avg_ms,
    MIN(duration) AS min_ms,
    MAX(duration) AS max_ms
FROM (
    SELECT
        id,
        extract(milliseconds FROM (clock_timestamp() - start_time)) AS duration
    FROM (
        SELECT
            id,
            clock_timestamp() AS start_time,
            (SELECT id FROM vectors_ruvector v ORDER BY v.embedding <-> (:test_query)::ruvector LIMIT :k)
        FROM queries
        LIMIT 100
    ) t
) times;

-- PGVector scan
\echo 'pgvector sequential scan (p50, p99 latency):'
SELECT
    percentile_cont(0.5) WITHIN GROUP (ORDER BY duration) AS p50_ms,
    percentile_cont(0.99) WITHIN GROUP (ORDER BY duration) AS p99_ms,
    AVG(duration) AS avg_ms,
    MIN(duration) AS min_ms,
    MAX(duration) AS max_ms
FROM (
    SELECT
        id,
        extract(milliseconds FROM (clock_timestamp() - start_time)) AS duration
    FROM (
        SELECT
            id,
            clock_timestamp() AS start_time,
            (SELECT id FROM vectors_pgvector v ORDER BY v.embedding <-> (SELECT query_vector::vector FROM queries WHERE id = 1) LIMIT :k)
        FROM queries
        LIMIT 100
    ) t
) times;

-- ============================================================================
-- Benchmark 2: Build Index
-- ============================================================================

\echo ''
\echo '=== Benchmark 2: Index Build Time ==='
\echo ''

-- RuVector HNSW
\echo 'Building ruvector HNSW index...'
\timing on
CREATE INDEX vectors_ruvector_hnsw_idx ON vectors_ruvector
USING hnsw (embedding ruvector_l2_ops)
WITH (m = 16, ef_construction = 64);

-- PGVector HNSW
\echo 'Building pgvector HNSW index...'
\timing on
CREATE INDEX vectors_pgvector_hnsw_idx ON vectors_pgvector
USING hnsw (embedding vector_l2_ops)
WITH (m = 16, ef_construction = 64);

-- ============================================================================
-- Benchmark 3: Index Search Performance
-- ============================================================================

\echo ''
\echo '=== Benchmark 3: Index Search (HNSW) ==='
\echo ''

-- Warm up
SELECT COUNT(*) FROM vectors_ruvector v, queries q
WHERE v.embedding <-> q.query_vector < 1000 LIMIT 100;

-- RuVector HNSW search
\echo 'RuVector HNSW search (p50, p99 latency):'
SELECT
    percentile_cont(0.5) WITHIN GROUP (ORDER BY duration) AS p50_ms,
    percentile_cont(0.99) WITHIN GROUP (ORDER BY duration) AS p99_ms,
    AVG(duration) AS avg_ms,
    MIN(duration) AS min_ms,
    MAX(duration) AS max_ms
FROM (
    SELECT
        id,
        extract(milliseconds FROM (clock_timestamp() - start_time)) AS duration
    FROM (
        SELECT
            q.id,
            clock_timestamp() AS start_time,
            (SELECT id FROM vectors_ruvector v ORDER BY v.embedding <-> q.query_vector LIMIT :k)
        FROM queries q
        LIMIT 1000
    ) t
) times;

-- PGVector HNSW search
\echo 'pgvector HNSW search (p50, p99 latency):'
SELECT
    percentile_cont(0.5) WITHIN GROUP (ORDER BY duration) AS p50_ms,
    percentile_cont(0.99) WITHIN GROUP (ORDER BY duration) AS p99_ms,
    AVG(duration) AS avg_ms,
    MIN(duration) AS min_ms,
    MAX(duration) AS max_ms
FROM (
    SELECT
        id,
        extract(milliseconds FROM (clock_timestamp() - start_time)) AS duration
    FROM (
        SELECT
            q.id,
            clock_timestamp() AS start_time,
            (SELECT id FROM vectors_pgvector v ORDER BY v.embedding <-> q.query_vector::vector LIMIT :k)
        FROM queries q
        LIMIT 1000
    ) t
) times;

-- ============================================================================
-- Benchmark 4: Distance Function Performance
-- ============================================================================

\echo ''
\echo '=== Benchmark 4: Distance Functions ==='
\echo ''

-- L2 Distance
\echo 'L2 Distance (100k calculations):'
\timing on
SELECT SUM(ruvector_l2_distance(v1.embedding, v2.embedding))
FROM vectors_ruvector v1
CROSS JOIN vectors_ruvector v2
WHERE v1.id <= 100 AND v2.id <= 1000;

\timing on
SELECT SUM(v1.embedding <-> v2.embedding)
FROM vectors_pgvector v1
CROSS JOIN vectors_pgvector v2
WHERE v1.id <= 100 AND v2.id <= 1000;

-- Cosine Distance
\echo 'Cosine Distance (100k calculations):'
\timing on
SELECT SUM(ruvector_cosine_distance(v1.embedding, v2.embedding))
FROM vectors_ruvector v1
CROSS JOIN vectors_ruvector v2
WHERE v1.id <= 100 AND v2.id <= 1000;

\timing on
SELECT SUM(v1.embedding <=> v2.embedding)
FROM vectors_pgvector v1
CROSS JOIN vectors_pgvector v2
WHERE v1.id <= 100 AND v2.id <= 1000;

-- Inner Product
\echo 'Inner Product (100k calculations):'
\timing on
SELECT SUM(ruvector_inner_product(v1.embedding, v2.embedding))
FROM vectors_ruvector v1
CROSS JOIN vectors_ruvector v2
WHERE v1.id <= 100 AND v2.id <= 1000;

\timing on
SELECT SUM(v1.embedding <#> v2.embedding)
FROM vectors_pgvector v1
CROSS JOIN vectors_pgvector v2
WHERE v1.id <= 100 AND v2.id <= 1000;

-- ============================================================================
-- Benchmark 5: Index Recall Accuracy
-- ============================================================================

\echo ''
\echo '=== Benchmark 5: Index Recall ==='
\echo ''

-- Create ground truth table
DROP TABLE IF EXISTS ground_truth;
CREATE TEMP TABLE ground_truth AS
SELECT
    q.id AS query_id,
    ARRAY_AGG(v.id ORDER BY v.embedding <-> q.query_vector) AS true_neighbors
FROM queries q
CROSS JOIN LATERAL (
    SELECT id, embedding
    FROM vectors_ruvector
    ORDER BY embedding <-> q.query_vector
    LIMIT :k
) v
WHERE q.id <= 100
GROUP BY q.id;

-- Compute recall for ruvector HNSW
WITH hnsw_results AS (
    SELECT
        q.id AS query_id,
        ARRAY_AGG(v.id ORDER BY v.embedding <-> q.query_vector) AS hnsw_neighbors
    FROM queries q
    CROSS JOIN LATERAL (
        SELECT id
        FROM vectors_ruvector
        ORDER BY embedding <-> q.query_vector
        LIMIT :k
    ) v
    WHERE q.id <= 100
    GROUP BY q.id
)
SELECT
    AVG(
        (
            SELECT COUNT(*)
            FROM unnest(h.hnsw_neighbors) AS hn
            WHERE hn = ANY(g.true_neighbors)
        )::float / :k
    ) AS recall
FROM hnsw_results h
JOIN ground_truth g ON h.query_id = g.query_id;

-- ============================================================================
-- Benchmark 6: Memory Usage
-- ============================================================================

\echo ''
\echo '=== Benchmark 6: Memory Usage ==='
\echo ''

-- Table sizes
\echo 'Table sizes:'
SELECT
    'ruvector' AS type,
    pg_size_pretty(pg_total_relation_size('vectors_ruvector')) AS total_size,
    pg_size_pretty(pg_relation_size('vectors_ruvector')) AS table_size,
    pg_size_pretty(pg_indexes_size('vectors_ruvector')) AS index_size
UNION ALL
SELECT
    'pgvector' AS type,
    pg_size_pretty(pg_total_relation_size('vectors_pgvector')) AS total_size,
    pg_size_pretty(pg_relation_size('vectors_pgvector')) AS table_size,
    pg_size_pretty(pg_indexes_size('vectors_pgvector')) AS index_size;

-- Index sizes
\echo 'Index sizes:'
SELECT
    indexname,
    pg_size_pretty(pg_relation_size(indexname::regclass)) AS size
FROM pg_indexes
WHERE tablename IN ('vectors_ruvector', 'vectors_pgvector')
ORDER BY tablename, indexname;

-- ============================================================================
-- Benchmark 7: Quantization Performance
-- ============================================================================

\echo ''
\echo '=== Benchmark 7: Quantization ==='
\echo ''

-- Create quantized tables
DROP TABLE IF EXISTS vectors_scalar;
CREATE TABLE vectors_scalar (
    id SERIAL PRIMARY KEY,
    embedding scalarvec
);

INSERT INTO vectors_scalar (embedding)
SELECT quantize_scalar(embedding)
FROM vectors_ruvector
LIMIT 100000;

-- Quantized search
\echo 'Scalar quantized search:'
\timing on
SELECT id
FROM vectors_scalar
ORDER BY embedding <-> quantize_scalar((SELECT query_vector FROM queries WHERE id = 1))
LIMIT :k;

-- ============================================================================
-- Cleanup
-- ============================================================================

\echo ''
\echo '=== Benchmark Complete ==='
\echo ''

DROP TABLE IF EXISTS vectors_ruvector CASCADE;
DROP TABLE IF EXISTS vectors_pgvector CASCADE;
DROP TABLE IF EXISTS queries CASCADE;
DROP TABLE IF EXISTS vectors_scalar CASCADE;
