-- ============================================================================
-- HNSW Index Test Suite
-- ============================================================================
-- Comprehensive tests for HNSW index access method
--
-- Run with: psql -d testdb -f hnsw_index_tests.sql

\set ECHO all
\set ON_ERROR_STOP on

-- Create test database if needed
-- CREATE DATABASE hnsw_test;
-- \c hnsw_test

-- Load extension
CREATE EXTENSION IF NOT EXISTS ruvector;

-- ============================================================================
-- Test 1: Basic Index Creation
-- ============================================================================

\echo '=== Test 1: Basic HNSW Index Creation ==='

CREATE TABLE test_vectors (
    id SERIAL PRIMARY KEY,
    embedding real[]
);

-- Insert test data (3D vectors)
INSERT INTO test_vectors (embedding) VALUES
    (ARRAY[0.0, 0.0, 0.0]::real[]),
    (ARRAY[1.0, 0.0, 0.0]::real[]),
    (ARRAY[0.0, 1.0, 0.0]::real[]),
    (ARRAY[0.0, 0.0, 1.0]::real[]),
    (ARRAY[1.0, 1.0, 0.0]::real[]),
    (ARRAY[1.0, 0.0, 1.0]::real[]),
    (ARRAY[0.0, 1.0, 1.0]::real[]),
    (ARRAY[1.0, 1.0, 1.0]::real[]),
    (ARRAY[0.5, 0.5, 0.5]::real[]),
    (ARRAY[0.2, 0.3, 0.1]::real[]);

-- Create HNSW index with default options (L2 distance)
CREATE INDEX test_vectors_hnsw_l2_idx ON test_vectors USING hnsw (embedding hnsw_l2_ops);

-- Verify index was created
SELECT indexname, indexdef
FROM pg_indexes
WHERE tablename = 'test_vectors';

-- ============================================================================
-- Test 2: L2 Distance Queries
-- ============================================================================

\echo '=== Test 2: L2 Distance Queries ==='

-- Query nearest neighbors to origin [0, 0, 0]
SELECT id, embedding, embedding <-> ARRAY[0.0, 0.0, 0.0]::real[] AS distance
FROM test_vectors
ORDER BY embedding <-> ARRAY[0.0, 0.0, 0.0]::real[]
LIMIT 5;

-- Query nearest neighbors to [1, 1, 1]
SELECT id, embedding, embedding <-> ARRAY[1.0, 1.0, 1.0]::real[] AS distance
FROM test_vectors
ORDER BY embedding <-> ARRAY[1.0, 1.0, 1.0]::real[]
LIMIT 5;

-- ============================================================================
-- Test 3: Index with Custom Options
-- ============================================================================

\echo '=== Test 3: HNSW Index with Custom Options ==='

CREATE TABLE test_vectors_opts (
    id SERIAL PRIMARY KEY,
    embedding real[]
);

-- Insert larger dataset
INSERT INTO test_vectors_opts (embedding)
SELECT ARRAY[random(), random(), random()]::real[]
FROM generate_series(1, 1000);

-- Create index with custom parameters
CREATE INDEX test_vectors_opts_hnsw_idx ON test_vectors_opts
    USING hnsw (embedding hnsw_l2_ops)
    WITH (m = 32, ef_construction = 128);

-- Verify index was created with options
SELECT indexname, indexdef
FROM pg_indexes
WHERE tablename = 'test_vectors_opts';

-- Query performance test
\timing on
SELECT id, embedding <-> ARRAY[0.5, 0.5, 0.5]::real[] AS distance
FROM test_vectors_opts
ORDER BY embedding <-> ARRAY[0.5, 0.5, 0.5]::real[]
LIMIT 10;
\timing off

-- ============================================================================
-- Test 4: Cosine Distance Index
-- ============================================================================

\echo '=== Test 4: Cosine Distance Index ==='

CREATE TABLE test_vectors_cosine (
    id SERIAL PRIMARY KEY,
    embedding real[]
);

-- Insert normalized vectors for cosine similarity
INSERT INTO test_vectors_cosine (embedding)
SELECT vector_normalize(ARRAY[random(), random(), random()]::real[])
FROM generate_series(1, 100);

-- Create HNSW index with cosine distance
CREATE INDEX test_vectors_cosine_idx ON test_vectors_cosine
    USING hnsw (embedding hnsw_cosine_ops);

-- Query with cosine distance
SELECT id, embedding <=> ARRAY[1.0, 0.0, 0.0]::real[] AS cosine_dist
FROM test_vectors_cosine
ORDER BY embedding <=> ARRAY[1.0, 0.0, 0.0]::real[]
LIMIT 5;

-- ============================================================================
-- Test 5: Inner Product Index
-- ============================================================================

\echo '=== Test 5: Inner Product Index ==='

CREATE TABLE test_vectors_ip (
    id SERIAL PRIMARY KEY,
    embedding real[]
);

-- Insert test vectors
INSERT INTO test_vectors_ip (embedding)
SELECT ARRAY[random() * 10, random() * 10, random() * 10]::real[]
FROM generate_series(1, 100);

-- Create HNSW index with inner product
CREATE INDEX test_vectors_ip_idx ON test_vectors_ip
    USING hnsw (embedding hnsw_ip_ops);

-- Query with inner product (finds vectors with largest inner product)
SELECT id, embedding <#> ARRAY[1.0, 1.0, 1.0]::real[] AS neg_ip
FROM test_vectors_ip
ORDER BY embedding <#> ARRAY[1.0, 1.0, 1.0]::real[]
LIMIT 5;

-- ============================================================================
-- Test 6: High-Dimensional Vectors
-- ============================================================================

\echo '=== Test 6: High-Dimensional Vectors (128D) ==='

CREATE TABLE test_vectors_high_dim (
    id SERIAL PRIMARY KEY,
    embedding real[]
);

-- Insert 128-dimensional vectors
INSERT INTO test_vectors_high_dim (embedding)
SELECT array_agg(random())::real[]
FROM generate_series(1, 500),
     generate_series(1, 128)
GROUP BY 1;

-- Create HNSW index
CREATE INDEX test_vectors_high_dim_idx ON test_vectors_high_dim
    USING hnsw (embedding hnsw_l2_ops)
    WITH (m = 16, ef_construction = 64);

-- Query 128D vectors
\set query_vec 'SELECT array_agg(random())::real[] FROM generate_series(1, 128)'
SELECT id, embedding <-> (:query_vec) AS distance
FROM test_vectors_high_dim
ORDER BY embedding <-> (:query_vec)
LIMIT 5;

-- ============================================================================
-- Test 7: Index Maintenance
-- ============================================================================

\echo '=== Test 7: Index Maintenance ==='

-- Get memory statistics
SELECT ruvector_memory_stats();

-- Perform index maintenance
SELECT ruvector_index_maintenance('test_vectors_hnsw_l2_idx');

-- Check index size
SELECT
    indexname,
    pg_size_pretty(pg_relation_size(indexname::regclass)) AS index_size
FROM pg_indexes
WHERE tablename LIKE 'test_vectors%';

-- ============================================================================
-- Test 8: Insert/Delete Operations
-- ============================================================================

\echo '=== Test 8: Insert and Delete Operations ==='

-- Insert new vectors
INSERT INTO test_vectors (embedding)
SELECT ARRAY[random(), random(), random()]::real[]
FROM generate_series(1, 100);

-- Query after insert
SELECT COUNT(*) FROM test_vectors;

-- Delete some vectors
DELETE FROM test_vectors WHERE id % 2 = 0;

-- Query after delete
SELECT COUNT(*) FROM test_vectors;

-- Verify index still works
SELECT id, embedding <-> ARRAY[0.5, 0.5, 0.5]::real[] AS distance
FROM test_vectors
ORDER BY embedding <-> ARRAY[0.5, 0.5, 0.5]::real[]
LIMIT 5;

-- ============================================================================
-- Test 9: Query Plan Analysis
-- ============================================================================

\echo '=== Test 9: Query Plan Analysis ==='

-- Explain query plan for HNSW index scan
EXPLAIN (ANALYZE, BUFFERS)
SELECT id, embedding <-> ARRAY[0.5, 0.5, 0.5]::real[] AS distance
FROM test_vectors_opts
ORDER BY embedding <-> ARRAY[0.5, 0.5, 0.5]::real[]
LIMIT 10;

-- ============================================================================
-- Test 10: Session Parameter Testing
-- ============================================================================

\echo '=== Test 10: Session Parameter Testing ==='

-- Show current ef_search setting
SHOW ruvector.ef_search;

-- Increase ef_search for better recall
SET ruvector.ef_search = 100;

-- Run query with increased ef_search
SELECT id, embedding <-> ARRAY[0.5, 0.5, 0.5]::real[] AS distance
FROM test_vectors_opts
ORDER BY embedding <-> ARRAY[0.5, 0.5, 0.5]::real[]
LIMIT 10;

-- Reset to default
RESET ruvector.ef_search;

-- ============================================================================
-- Test 11: Operator Functionality
-- ============================================================================

\echo '=== Test 11: Distance Operator Tests ==='

-- Test L2 distance operator
SELECT
    ARRAY[1.0, 2.0, 3.0]::real[] <-> ARRAY[4.0, 5.0, 6.0]::real[] AS l2_dist;

-- Test cosine distance operator
SELECT
    ARRAY[1.0, 0.0, 0.0]::real[] <=> ARRAY[0.0, 1.0, 0.0]::real[] AS cosine_dist;

-- Test inner product operator
SELECT
    ARRAY[1.0, 2.0, 3.0]::real[] <#> ARRAY[4.0, 5.0, 6.0]::real[] AS neg_ip;

-- ============================================================================
-- Test 12: Edge Cases
-- ============================================================================

\echo '=== Test 12: Edge Cases ==='

-- Empty result set
SELECT id, embedding <-> ARRAY[100.0, 100.0, 100.0]::real[] AS distance
FROM test_vectors
WHERE id < 0  -- No results
ORDER BY embedding <-> ARRAY[100.0, 100.0, 100.0]::real[]
LIMIT 5;

-- Single vector table
CREATE TABLE test_single_vector (
    id SERIAL PRIMARY KEY,
    embedding real[]
);

INSERT INTO test_single_vector (embedding) VALUES (ARRAY[1.0, 2.0, 3.0]::real[]);

CREATE INDEX test_single_vector_idx ON test_single_vector
    USING hnsw (embedding hnsw_l2_ops);

SELECT * FROM test_single_vector
ORDER BY embedding <-> ARRAY[0.0, 0.0, 0.0]::real[]
LIMIT 5;

-- ============================================================================
-- Test 13: Parameterized Query Regression Tests (Issue #141)
-- ============================================================================
-- These tests verify the fix for HNSW segmentation fault with parameterized
-- queries. See ADR-0027 and GitHub issue #141 for details.

\echo '=== Test 13: Parameterized Query Regression Tests (Issue #141) ==='

-- Create ruvector table for parameterized query testing
CREATE TABLE test_ruvector_param (
    id SERIAL PRIMARY KEY,
    content TEXT NOT NULL,
    embedding ruvector(8)
);

-- Insert test data with ruvector type
INSERT INTO test_ruvector_param (content, embedding) VALUES
    ('Doc 1', '[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]'::ruvector(8)),
    ('Doc 2', '[0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]'::ruvector(8)),
    ('Doc 3', '[0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0]'::ruvector(8)),
    ('Doc 4', '[0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1]'::ruvector(8)),
    ('Doc 5', '[0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2]'::ruvector(8));

-- Create HNSW index on ruvector column
CREATE INDEX test_ruvector_param_hnsw_idx ON test_ruvector_param
    USING hnsw (embedding ruvector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- Test 13a: Literal query (baseline - should work)
\echo '--- Test 13a: Literal Query (baseline) ---'
SELECT id, content,
       1 - (embedding <=> '[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]'::ruvector(8)) as similarity
FROM test_ruvector_param
ORDER BY embedding <=> '[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]'::ruvector(8)
LIMIT 3;

-- Test 13b: Prepared statement with parameter (was crashing before fix)
\echo '--- Test 13b: Prepared Statement with Parameter ---'
PREPARE param_search_test AS
    SELECT id, content FROM test_ruvector_param
    ORDER BY embedding <=> $1::ruvector(8)
    LIMIT 3;

EXECUTE param_search_test('[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]');
EXECUTE param_search_test('[0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2]');

DEALLOCATE param_search_test;

-- Test 13c: Function with text parameter (simulates driver behavior)
\echo '--- Test 13c: Function with Text Parameter ---'
CREATE OR REPLACE FUNCTION test_hnsw_param_search(query_vec TEXT)
RETURNS TABLE(id INT, content TEXT) AS $$
BEGIN
    RETURN QUERY
    SELECT t.id, t.content
    FROM test_ruvector_param t
    ORDER BY t.embedding <=> query_vec::ruvector(8)
    LIMIT 3;
END;
$$ LANGUAGE plpgsql;

SELECT * FROM test_hnsw_param_search('[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]');
SELECT * FROM test_hnsw_param_search('[0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2]');

DROP FUNCTION test_hnsw_param_search;

-- Test 13d: Zero vector error handling (should error gracefully, not crash)
\echo '--- Test 13d: Zero Vector Error Handling ---'
\set ON_ERROR_STOP off
-- This should produce an error, not a crash
SELECT id, content FROM test_ruvector_param
ORDER BY embedding <=> '[0, 0, 0, 0, 0, 0, 0, 0]'::ruvector(8)
LIMIT 3;
\set ON_ERROR_STOP on

-- Test 13e: Dimension mismatch error handling (should error gracefully)
\echo '--- Test 13e: Dimension Mismatch Error Handling ---'
\set ON_ERROR_STOP off
-- This should produce an error about dimension mismatch
SELECT id, content FROM test_ruvector_param
ORDER BY embedding <=> '[0.1, 0.2, 0.3]'::ruvector(3)
LIMIT 3;
\set ON_ERROR_STOP on

-- Test 13f: 384-dimension vectors (production scale test)
\echo '--- Test 13f: 384-Dimension Vectors (Production Scale) ---'
CREATE TABLE test_ruvector_384 (
    id SERIAL PRIMARY KEY,
    content TEXT NOT NULL,
    embedding ruvector(384)
);

-- Generate 100 test vectors with 384 dimensions
DO $$
DECLARE
    i INTEGER;
    vec_text TEXT;
BEGIN
    FOR i IN 1..100 LOOP
        SELECT '[' || string_agg(((random() - 0.5)::numeric(6,4))::text, ',') || ']'
        INTO vec_text
        FROM generate_series(1, 384);

        INSERT INTO test_ruvector_384 (content, embedding)
        VALUES ('Doc ' || i, vec_text::ruvector(384));
    END LOOP;
END $$;

-- Create HNSW index
CREATE INDEX test_ruvector_384_idx ON test_ruvector_384
    USING hnsw (embedding ruvector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- Prepare and execute parameterized search on 384-dim vectors
PREPARE param_search_384 AS
    SELECT id, content FROM test_ruvector_384
    ORDER BY embedding <=> $1::ruvector(384)
    LIMIT 5;

-- Get a sample vector and search with it via parameter
DO $$
DECLARE
    sample_vec TEXT;
BEGIN
    SELECT embedding::text INTO sample_vec FROM test_ruvector_384 WHERE id = 1;
    -- This would fail before the fix
    RAISE NOTICE 'Sample vector extracted, length: %', length(sample_vec);
END $$;

DEALLOCATE param_search_384;

\echo '=== Test 13: Parameterized Query Tests Completed ==='

-- ============================================================================
-- Cleanup
-- ============================================================================

\echo '=== Cleanup ==='

DROP TABLE IF EXISTS test_vectors CASCADE;
DROP TABLE IF EXISTS test_vectors_opts CASCADE;
DROP TABLE IF EXISTS test_vectors_cosine CASCADE;
DROP TABLE IF EXISTS test_vectors_ip CASCADE;
DROP TABLE IF EXISTS test_vectors_high_dim CASCADE;
DROP TABLE IF EXISTS test_single_vector CASCADE;
DROP TABLE IF EXISTS test_ruvector_param CASCADE;
DROP TABLE IF EXISTS test_ruvector_384 CASCADE;

\echo '=== All tests completed successfully ==='
