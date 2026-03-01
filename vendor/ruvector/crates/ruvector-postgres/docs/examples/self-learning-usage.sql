-- =============================================================================
-- RuVector Self-Learning Module Usage Examples
-- =============================================================================
-- This file demonstrates how to use the self-learning and ReasoningBank
-- features for adaptive query optimization.

-- -----------------------------------------------------------------------------
-- 1. Basic Setup: Enable Learning
-- -----------------------------------------------------------------------------

-- Enable learning for a table with default configuration
SELECT ruvector_enable_learning('my_vectors');

-- Enable with custom configuration
SELECT ruvector_enable_learning(
    'my_vectors',
    '{"max_trajectories": 2000, "num_clusters": 15}'::jsonb
);

-- -----------------------------------------------------------------------------
-- 2. Recording Query Trajectories
-- -----------------------------------------------------------------------------

-- Trajectories are typically recorded automatically by search functions,
-- but you can also record them manually for testing or custom workflows.

-- Record a query trajectory
SELECT ruvector_record_trajectory(
    'my_vectors',                    -- table name
    ARRAY[0.1, 0.2, 0.3, 0.4],      -- query vector
    ARRAY[1, 2, 3, 4, 5]::bigint[], -- result IDs
    1500,                            -- latency in microseconds
    50,                              -- ef_search used
    10                               -- probes used
);

-- -----------------------------------------------------------------------------
-- 3. Providing Relevance Feedback
-- -----------------------------------------------------------------------------

-- After seeing query results, users can provide feedback about which
-- results were actually relevant

SELECT ruvector_record_feedback(
    'my_vectors',                    -- table name
    ARRAY[0.1, 0.2, 0.3, 0.4],      -- query vector
    ARRAY[1, 2, 5]::bigint[],       -- relevant IDs
    ARRAY[3, 4]::bigint[]           -- irrelevant IDs
);

-- -----------------------------------------------------------------------------
-- 4. Extracting and Managing Patterns
-- -----------------------------------------------------------------------------

-- Extract patterns from recorded trajectories using k-means clustering
SELECT ruvector_extract_patterns(
    'my_vectors',  -- table name
    10             -- number of clusters
);

-- Get current learning statistics
SELECT ruvector_learning_stats('my_vectors');

-- Example output:
-- {
--   "trajectories": {
--     "total": 150,
--     "with_feedback": 45,
--     "avg_latency_us": 1234.5,
--     "avg_precision": 0.85,
--     "avg_recall": 0.78
--   },
--   "patterns": {
--     "total": 10,
--     "total_samples": 150,
--     "avg_confidence": 0.87,
--     "total_usage": 523
--   }
-- }

-- -----------------------------------------------------------------------------
-- 5. Auto-Tuning Search Parameters
-- -----------------------------------------------------------------------------

-- Auto-tune for balanced performance (default)
SELECT ruvector_auto_tune('my_vectors');

-- Auto-tune optimizing for speed
SELECT ruvector_auto_tune('my_vectors', 'speed');

-- Auto-tune optimizing for accuracy
SELECT ruvector_auto_tune('my_vectors', 'accuracy');

-- Auto-tune with sample queries
SELECT ruvector_auto_tune(
    'my_vectors',
    'balanced',
    ARRAY[
        ARRAY[0.1, 0.2, 0.3],
        ARRAY[0.4, 0.5, 0.6],
        ARRAY[0.7, 0.8, 0.9]
    ]
);

-- -----------------------------------------------------------------------------
-- 6. Getting Optimized Search Parameters
-- -----------------------------------------------------------------------------

-- Get optimized search parameters for a specific query
SELECT ruvector_get_search_params(
    'my_vectors',
    ARRAY[0.1, 0.2, 0.3, 0.4]
);

-- Example output:
-- {
--   "ef_search": 52,
--   "probes": 12,
--   "confidence": 0.89
-- }

-- Use these parameters in your search:
-- SET ruvector.ef_search = 52;
-- SET ruvector.probes = 12;
-- SELECT * FROM my_vectors ORDER BY embedding <-> '[0.1, 0.2, 0.3, 0.4]' LIMIT 10;

-- -----------------------------------------------------------------------------
-- 7. Pattern Consolidation and Pruning
-- -----------------------------------------------------------------------------

-- Consolidate similar patterns to reduce memory usage
-- Patterns with similarity >= 0.95 will be merged
SELECT ruvector_consolidate_patterns('my_vectors', 0.95);

-- Prune low-quality patterns
-- Remove patterns with usage < 5 or confidence < 0.5
SELECT ruvector_prune_patterns(
    'my_vectors',
    5,    -- min_usage
    0.5   -- min_confidence
);

-- -----------------------------------------------------------------------------
-- 8. Complete Workflow Example
-- -----------------------------------------------------------------------------

-- Create a table with vectors
CREATE TABLE documents (
    id BIGSERIAL PRIMARY KEY,
    title TEXT,
    embedding vector(384)
);

-- Insert some sample data
INSERT INTO documents (title, embedding)
SELECT
    'Document ' || i,
    ruvector_random(384)
FROM generate_series(1, 1000) i;

-- Create an HNSW index
CREATE INDEX ON documents USING hnsw (embedding vector_cosine_ops);

-- Enable learning for adaptive optimization
SELECT ruvector_enable_learning('documents');

-- Simulate user queries and collect trajectories
DO $$
DECLARE
    query_vec vector(384);
    results bigint[];
    start_time bigint;
    end_time bigint;
BEGIN
    FOR i IN 1..50 LOOP
        -- Generate random query
        query_vec := ruvector_random(384);

        -- Execute search and measure time
        start_time := EXTRACT(EPOCH FROM clock_timestamp()) * 1000000;

        SELECT array_agg(id) INTO results
        FROM (
            SELECT id FROM documents
            ORDER BY embedding <=> query_vec
            LIMIT 10
        ) t;

        end_time := EXTRACT(EPOCH FROM clock_timestamp()) * 1000000;

        -- Record trajectory
        PERFORM ruvector_record_trajectory(
            'documents',
            query_vec::float4[],
            results,
            (end_time - start_time)::bigint,
            50,  -- current ef_search
            10   -- current probes
        );

        -- Occasionally provide feedback
        IF i % 5 = 0 THEN
            PERFORM ruvector_record_feedback(
                'documents',
                query_vec::float4[],
                results[1:3],  -- first 3 were relevant
                results[8:10]  -- last 3 were not relevant
            );
        END IF;
    END LOOP;
END $$;

-- Extract patterns from collected data
SELECT ruvector_extract_patterns('documents', 10);

-- View learning statistics
SELECT ruvector_learning_stats('documents');

-- Auto-tune for optimal performance
SELECT ruvector_auto_tune('documents', 'balanced');

-- Get optimized parameters for a new query
WITH query AS (
    SELECT ruvector_random(384) AS vec
),
params AS (
    SELECT ruvector_get_search_params('documents', (SELECT vec::float4[] FROM query)) AS p
)
SELECT
    (p->'ef_search')::int AS ef_search,
    (p->'probes')::int AS probes,
    (p->'confidence')::float AS confidence
FROM params;

-- -----------------------------------------------------------------------------
-- 9. Monitoring and Maintenance
-- -----------------------------------------------------------------------------

-- Regularly consolidate patterns (can be run in a cron job)
SELECT ruvector_consolidate_patterns('documents', 0.92);

-- Prune low-quality patterns monthly
SELECT ruvector_prune_patterns('documents', 10, 0.6);

-- Clear all learning data if needed
SELECT ruvector_clear_learning('documents');

-- -----------------------------------------------------------------------------
-- 10. Advanced: Integration with Application Code
-- -----------------------------------------------------------------------------

-- Example: Python application using learned parameters

/*
import psycopg2

def search_with_learning(conn, table, query_vector, limit=10):
    """Search using learned optimal parameters"""

    # Get optimized parameters
    with conn.cursor() as cur:
        cur.execute("""
            SELECT ruvector_get_search_params(%s, %s::float4[])
        """, (table, query_vector))
        params = cur.fetchone()[0]

    # Apply parameters and search
    with conn.cursor() as cur:
        cur.execute(f"""
            SET ruvector.ef_search = {params['ef_search']};
            SET ruvector.probes = {params['probes']};

            SELECT id, title, embedding <=> %s::vector AS distance
            FROM {table}
            ORDER BY embedding <=> %s::vector
            LIMIT %s
        """, (query_vector, query_vector, limit))

        results = cur.fetchall()

    return results, params

# Use it
conn = psycopg2.connect("dbname=mydb")
results, params = search_with_learning(
    conn,
    'documents',
    [0.1, 0.2, 0.3, ...],
    limit=10
)

print(f"Search completed with ef_search={params['ef_search']}, "
      f"confidence={params['confidence']:.2f}")
*/

-- -----------------------------------------------------------------------------
-- 11. Best Practices
-- -----------------------------------------------------------------------------

-- 1. Collect enough trajectories before extracting patterns (50+ recommended)
-- 2. Provide relevance feedback when possible for better learning
-- 3. Consolidate patterns regularly to manage memory
-- 4. Prune low-quality patterns periodically
-- 5. Monitor learning statistics to track improvement
-- 6. Start with balanced optimization, adjust based on needs
-- 7. Re-extract patterns when query patterns change significantly

-- Example monitoring query:
SELECT
    jsonb_pretty(ruvector_learning_stats('documents')) AS stats,
    CASE
        WHEN (stats->'trajectories'->>'total')::int < 50
        THEN 'Collecting data - need more trajectories'
        WHEN (stats->'patterns'->>'total')::int = 0
        THEN 'Ready to extract patterns'
        WHEN (stats->'patterns'->>'avg_confidence')::float < 0.7
        THEN 'Low confidence - collect more feedback'
        ELSE 'System is learning well'
    END AS recommendation
FROM (
    SELECT ruvector_learning_stats('documents') AS stats
) t;
