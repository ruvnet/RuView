-- RuVector Optimized Benchmark Runner
-- Tests performance of optimized operations

\timing on

-- ============================================================================
-- Test 1: HNSW Vector Search (Target: ~24ms for 1000 vectors)
-- ============================================================================
\echo '=== Test 1: HNSW Vector Search ==='

-- Warm up
SELECT id, embedding <-> ruvector_random(128) AS distance
FROM benchmark_vectors
ORDER BY distance
LIMIT 10;

-- Benchmark: Find 10 nearest neighbors
EXPLAIN ANALYZE
SELECT id, embedding <-> ruvector_random(128) AS distance
FROM benchmark_vectors
ORDER BY distance
LIMIT 10;

-- ============================================================================
-- Test 2: Hamming Distance with bit_count (Target: ~7.6ms)
-- ============================================================================
\echo '=== Test 2: Hamming Distance ==='

EXPLAIN ANALYZE
SELECT
    a.id AS id_a,
    b.id AS id_b,
    bench_hamming_distance(a.binary_quantized, b.binary_quantized) AS hamming_dist
FROM benchmark_quantized a
CROSS JOIN benchmark_quantized b
WHERE a.id < b.id
LIMIT 1000;

-- ============================================================================
-- Test 3: Full-Text Search with GIN (Target: ~3.5ms)
-- ============================================================================
\echo '=== Test 3: Full-Text Search ==='

EXPLAIN ANALYZE
SELECT id, content, ts_rank(content_tsvector, query) AS rank
FROM benchmark_documents, plainto_tsquery('english', 'vector database search') query
WHERE content_tsvector @@ query
ORDER BY rank DESC
LIMIT 20;

-- ============================================================================
-- Test 4: GraphSAGE Aggregation (Target: ~2.6ms)
-- ============================================================================
\echo '=== Test 4: GraphSAGE Neighbor Aggregation ==='

EXPLAIN ANALYZE
WITH neighbor_features AS (
    SELECT
        e.source_id,
        ruvector_mean(ARRAY_AGG(n.features)) AS mean_neighbor
    FROM benchmark_edges e
    JOIN benchmark_nodes n ON e.target_id = n.id
    GROUP BY e.source_id
)
SELECT
    s.id,
    ruvector_concat(s.features, COALESCE(nf.mean_neighbor, s.features)) AS aggregated
FROM benchmark_nodes s
LEFT JOIN neighbor_features nf ON s.id = nf.source_id
LIMIT 50;

-- ============================================================================
-- Test 5: Sparse Vector Dot Product (Target: ~27ms)
-- ============================================================================
\echo '=== Test 5: Sparse Dot Product ==='

EXPLAIN ANALYZE
SELECT
    a.id AS id_a,
    b.id AS id_b,
    bench_sparse_dot(a.sparse_embedding, b.sparse_embedding) AS similarity
FROM benchmark_documents a
CROSS JOIN benchmark_documents b
WHERE a.id < b.id
LIMIT 500;

-- ============================================================================
-- Test 6: Graph Edge Lookup (Target: ~5ms)
-- ============================================================================
\echo '=== Test 6: Graph Edge Lookup ==='

EXPLAIN ANALYZE
SELECT
    e.*,
    s.features AS source_features,
    t.features AS target_features
FROM benchmark_edges e
JOIN benchmark_nodes s ON e.source_id = s.id
JOIN benchmark_nodes t ON e.target_id = t.id
WHERE e.source_id IN (SELECT id FROM benchmark_nodes ORDER BY random() LIMIT 10);

-- ============================================================================
-- Test 7: Scalar Quantization Compression (Target: ~75ms)
-- ============================================================================
\echo '=== Test 7: Scalar Quantization ==='

EXPLAIN ANALYZE
SELECT
    id,
    octet_length(scalar_quantized) AS compressed_size,
    ruvector_dim(original) * 4 AS original_size,
    ROUND(100.0 * octet_length(scalar_quantized) / (ruvector_dim(original) * 4), 2) AS compression_ratio
FROM benchmark_quantized
LIMIT 100;

-- ============================================================================
-- Test 8: Binary Quantization + Hamming (Target: ~85ms)
-- ============================================================================
\echo '=== Test 8: Binary Quantization Search ==='

EXPLAIN ANALYZE
WITH query_binary AS (
    SELECT ruvector_binary_quantize(ruvector_random(128)) AS q
)
SELECT
    bq.id,
    bench_hamming_distance(bq.binary_quantized, query_binary.q) AS hamming_dist
FROM benchmark_quantized bq, query_binary
ORDER BY hamming_dist
LIMIT 20;

-- ============================================================================
-- Summary
-- ============================================================================
\echo '=== Benchmark Summary ==='
SELECT
    'benchmark_vectors' AS table_name,
    COUNT(*) AS row_count,
    pg_size_pretty(pg_relation_size('benchmark_vectors')) AS table_size,
    pg_size_pretty(pg_indexes_size('benchmark_vectors')) AS index_size
FROM benchmark_vectors
UNION ALL
SELECT
    'benchmark_documents',
    COUNT(*),
    pg_size_pretty(pg_relation_size('benchmark_documents')),
    pg_size_pretty(pg_indexes_size('benchmark_documents'))
FROM benchmark_documents
UNION ALL
SELECT
    'benchmark_nodes',
    COUNT(*),
    pg_size_pretty(pg_relation_size('benchmark_nodes')),
    pg_size_pretty(pg_indexes_size('benchmark_nodes'))
FROM benchmark_nodes
UNION ALL
SELECT
    'benchmark_edges',
    COUNT(*),
    pg_size_pretty(pg_relation_size('benchmark_edges')),
    pg_size_pretty(pg_indexes_size('benchmark_edges'))
FROM benchmark_edges
UNION ALL
SELECT
    'benchmark_quantized',
    COUNT(*),
    pg_size_pretty(pg_relation_size('benchmark_quantized')),
    pg_size_pretty(pg_indexes_size('benchmark_quantized'))
FROM benchmark_quantized;

\timing off
