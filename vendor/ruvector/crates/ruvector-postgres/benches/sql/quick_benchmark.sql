-- Quick benchmark script for development testing
-- Smaller dataset for faster iteration

\timing on
\set ECHO all

-- Configuration
\set num_vectors 10000
\set num_queries 100
\set dims 768
\set k 10

BEGIN;

-- ============================================================================
-- Setup
-- ============================================================================

DROP TABLE IF EXISTS test_vectors CASCADE;
DROP TABLE IF EXISTS test_queries CASCADE;

CREATE TABLE test_vectors (
    id SERIAL PRIMARY KEY,
    embedding ruvector(:dims)
);

CREATE TABLE test_queries (
    id SERIAL PRIMARY KEY,
    query_vector ruvector(:dims)
);

-- ============================================================================
-- Load Data
-- ============================================================================

\echo 'Loading test data...'

INSERT INTO test_vectors (embedding)
SELECT
    array_to_ruvector(ARRAY(
        SELECT random()::real
        FROM generate_series(1, :dims)
    ))
FROM generate_series(1, :num_vectors);

INSERT INTO test_queries (query_vector)
SELECT
    array_to_ruvector(ARRAY(
        SELECT random()::real
        FROM generate_series(1, :dims)
    ))
FROM generate_series(1, :num_queries);

COMMIT;

-- ============================================================================
-- Sequential Scan Baseline
-- ============================================================================

\echo ''
\echo 'Sequential scan baseline:'
EXPLAIN ANALYZE
SELECT id
FROM test_vectors
ORDER BY embedding <-> (SELECT query_vector FROM test_queries WHERE id = 1)
LIMIT :k;

-- ============================================================================
-- Build HNSW Index
-- ============================================================================

\echo ''
\echo 'Building HNSW index...'
CREATE INDEX test_vectors_hnsw_idx ON test_vectors
USING hnsw (embedding ruvector_l2_ops)
WITH (m = 16, ef_construction = 64);

-- ============================================================================
-- Index Search
-- ============================================================================

\echo ''
\echo 'HNSW index search:'
EXPLAIN ANALYZE
SELECT id
FROM test_vectors
ORDER BY embedding <-> (SELECT query_vector FROM test_queries WHERE id = 1)
LIMIT :k;

-- ============================================================================
-- Distance Functions
-- ============================================================================

\echo ''
\echo 'Distance function performance (1000 calculations):'

-- L2
\timing on
SELECT SUM(ruvector_l2_distance(v1.embedding, v2.embedding))
FROM test_vectors v1, test_vectors v2
WHERE v1.id <= 10 AND v2.id <= 100;

-- Cosine
\timing on
SELECT SUM(ruvector_cosine_distance(v1.embedding, v2.embedding))
FROM test_vectors v1, test_vectors v2
WHERE v1.id <= 10 AND v2.id <= 100;

-- Inner Product
\timing on
SELECT SUM(ruvector_inner_product(v1.embedding, v2.embedding))
FROM test_vectors v1, test_vectors v2
WHERE v1.id <= 10 AND v2.id <= 100;

-- ============================================================================
-- Cleanup
-- ============================================================================

DROP TABLE IF EXISTS test_vectors CASCADE;
DROP TABLE IF EXISTS test_queries CASCADE;

\echo ''
\echo 'Quick benchmark complete!'
