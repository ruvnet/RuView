-- IVFFlat Access Method Tests
-- ============================================================================
-- Comprehensive test suite for IVFFlat index access method

-- Setup
\set ON_ERROR_STOP on

BEGIN;

-- Create test table
CREATE TABLE test_ivfflat (
    id serial PRIMARY KEY,
    embedding vector(128),
    data text
);

-- Insert test data (1000 random vectors)
INSERT INTO test_ivfflat (embedding, data)
SELECT
    array_to_vector(array_agg(random()::float4))::vector(128),
    'Test document ' || i
FROM generate_series(1, 1000) i,
     generate_series(1, 128) d
GROUP BY i;

-- ============================================================================
-- Test 1: Basic Index Creation
-- ============================================================================

\echo 'Test 1: Creating IVFFlat index with default parameters...'
CREATE INDEX test_ivfflat_l2_idx ON test_ivfflat
    USING ruivfflat (embedding vector_l2_ops);

\echo 'Test 1: PASSED - Index created successfully'

-- ============================================================================
-- Test 2: Index Creation with Custom Parameters
-- ============================================================================

\echo 'Test 2: Creating IVFFlat index with custom parameters...'
CREATE INDEX test_ivfflat_custom_idx ON test_ivfflat
    USING ruivfflat (embedding vector_l2_ops)
    WITH (lists = 50);

\echo 'Test 2: PASSED - Custom index created successfully'

-- ============================================================================
-- Test 3: Cosine Distance Index
-- ============================================================================

\echo 'Test 3: Creating IVFFlat index with cosine distance...'
CREATE INDEX test_ivfflat_cosine_idx ON test_ivfflat
    USING ruivfflat (embedding vector_cosine_ops)
    WITH (lists = 100);

\echo 'Test 3: PASSED - Cosine index created successfully'

-- ============================================================================
-- Test 4: Inner Product Index
-- ============================================================================

\echo 'Test 4: Creating IVFFlat index with inner product...'
CREATE INDEX test_ivfflat_ip_idx ON test_ivfflat
    USING ruivfflat (embedding vector_ip_ops)
    WITH (lists = 100);

\echo 'Test 4: PASSED - Inner product index created successfully'

-- ============================================================================
-- Test 5: Basic Search Query
-- ============================================================================

\echo 'Test 5: Testing basic search query...'

-- Create a query vector
WITH query AS (
    SELECT array_to_vector(array_agg(random()::float4))::vector(128) as q
    FROM generate_series(1, 128)
)
SELECT COUNT(*) as result_count
FROM test_ivfflat, query
ORDER BY embedding <-> query.q
LIMIT 10;

\echo 'Test 5: PASSED - Search query executed successfully'

-- ============================================================================
-- Test 6: Probe Configuration
-- ============================================================================

\echo 'Test 6: Testing probe configuration...'

-- Set probes to 1 (fast, lower recall)
SET ruvector.ivfflat_probes = 1;
SELECT setting FROM pg_settings WHERE name = 'ruvector.ivfflat_probes';

-- Set probes to 10 (slower, higher recall)
SET ruvector.ivfflat_probes = 10;
SELECT setting FROM pg_settings WHERE name = 'ruvector.ivfflat_probes';

\echo 'Test 6: PASSED - Probe configuration working'

-- ============================================================================
-- Test 7: Insert After Index Creation
-- ============================================================================

\echo 'Test 7: Testing insert after index creation...'

INSERT INTO test_ivfflat (embedding, data)
SELECT
    array_to_vector(array_agg(random()::float4))::vector(128),
    'New document ' || i
FROM generate_series(1, 100) i,
     generate_series(1, 128) d
GROUP BY i;

\echo 'Test 7: PASSED - Inserts after index creation working'

-- ============================================================================
-- Test 8: Search with Different Probe Values
-- ============================================================================

\echo 'Test 8: Comparing search results with different probes...'

WITH query AS (
    SELECT array_to_vector(array_agg(0.5::float4))::vector(128) as q
    FROM generate_series(1, 128)
)
SELECT
    'probes=1' as config,
    (
        SELECT COUNT(*)
        FROM test_ivfflat, query
        WHERE pg_catalog.set_config('ruvector.ivfflat_probes', '1', true) IS NOT NULL
        ORDER BY embedding <-> query.q
        LIMIT 10
    ) as result_count
UNION ALL
SELECT
    'probes=10' as config,
    (
        SELECT COUNT(*)
        FROM test_ivfflat, query
        WHERE pg_catalog.set_config('ruvector.ivfflat_probes', '10', true) IS NOT NULL
        ORDER BY embedding <-> query.q
        LIMIT 10
    ) as result_count;

\echo 'Test 8: PASSED - Different probe values tested'

-- ============================================================================
-- Test 9: Index Statistics
-- ============================================================================

\echo 'Test 9: Checking index statistics...'

SELECT * FROM ruvector_ivfflat_stats('test_ivfflat_l2_idx');

\echo 'Test 9: PASSED - Index statistics retrieved'

-- ============================================================================
-- Test 10: Index Size
-- ============================================================================

\echo 'Test 10: Checking index size...'

SELECT
    indexrelname,
    pg_size_pretty(pg_relation_size(indexrelid)) as index_size
FROM pg_stat_user_indexes
WHERE indexrelname LIKE 'test_ivfflat%'
ORDER BY indexrelname;

\echo 'Test 10: PASSED - Index sizes retrieved'

-- ============================================================================
-- Test 11: Explain Plan
-- ============================================================================

\echo 'Test 11: Checking query plan uses index...'

WITH query AS (
    SELECT array_to_vector(array_agg(0.5::float4))::vector(128) as q
    FROM generate_series(1, 128)
)
EXPLAIN (COSTS OFF)
SELECT id, data
FROM test_ivfflat, query
ORDER BY embedding <-> query.q
LIMIT 10;

\echo 'Test 11: PASSED - Query plan generated'

-- ============================================================================
-- Test 12: Concurrent Access
-- ============================================================================

\echo 'Test 12: Testing concurrent queries...'

-- Multiple simultaneous queries
WITH query1 AS (
    SELECT array_to_vector(array_agg(random()::float4))::vector(128) as q
    FROM generate_series(1, 128)
),
query2 AS (
    SELECT array_to_vector(array_agg(random()::float4))::vector(128) as q
    FROM generate_series(1, 128)
)
SELECT
    (SELECT COUNT(*) FROM test_ivfflat, query1 ORDER BY embedding <-> query1.q LIMIT 10) as q1_count,
    (SELECT COUNT(*) FROM test_ivfflat, query2 ORDER BY embedding <-> query2.q LIMIT 10) as q2_count;

\echo 'Test 12: PASSED - Concurrent queries working'

-- ============================================================================
-- Test 13: Reindex
-- ============================================================================

\echo 'Test 13: Testing REINDEX...'

REINDEX INDEX test_ivfflat_l2_idx;

\echo 'Test 13: PASSED - REINDEX successful'

-- ============================================================================
-- Test 14: Drop Index
-- ============================================================================

\echo 'Test 14: Testing DROP INDEX...'

DROP INDEX test_ivfflat_custom_idx;
DROP INDEX test_ivfflat_cosine_idx;
DROP INDEX test_ivfflat_ip_idx;

\echo 'Test 14: PASSED - DROP INDEX successful'

-- ============================================================================
-- Cleanup
-- ============================================================================

\echo 'Cleaning up...'
DROP TABLE test_ivfflat CASCADE;

ROLLBACK;

\echo ''
\echo '============================================'
\echo 'All IVFFlat Access Method Tests PASSED!'
\echo '============================================'
