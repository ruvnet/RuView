-- ============================================================================
-- HNSW Index Access Method
-- ============================================================================
-- This file defines the HNSW (Hierarchical Navigable Small World) index
-- access method for PostgreSQL, providing fast approximate nearest neighbor
-- search for vector similarity queries.
--
-- The HNSW index stores vectors in a multi-layer graph structure optimized
-- for logarithmic search complexity.

-- ============================================================================
-- Access Method Registration
-- ============================================================================

-- Register HNSW as a PostgreSQL index access method
CREATE ACCESS METHOD hnsw TYPE INDEX HANDLER hnsw_handler;

COMMENT ON ACCESS METHOD hnsw IS 'HNSW (Hierarchical Navigable Small World) index for approximate nearest neighbor search';

-- ============================================================================
-- Operator Families
-- ============================================================================

-- L2 (Euclidean) distance operator family
CREATE OPERATOR FAMILY hnsw_l2_ops USING hnsw;

-- Cosine distance operator family
CREATE OPERATOR FAMILY hnsw_cosine_ops USING hnsw;

-- Inner product operator family
CREATE OPERATOR FAMILY hnsw_ip_ops USING hnsw;

-- ============================================================================
-- Distance Operators (using array-based functions for now)
-- ============================================================================
-- Note: These operators work with real[] type
-- Future version will support custom vector types

-- L2 distance operator: <->
CREATE OPERATOR <-> (
    LEFTARG = real[],
    RIGHTARG = real[],
    FUNCTION = l2_distance_arr,
    COMMUTATOR = '<->'
);

COMMENT ON OPERATOR <->(real[], real[]) IS 'L2 (Euclidean) distance';

-- Cosine distance operator: <=>
CREATE OPERATOR <=> (
    LEFTARG = real[],
    RIGHTARG = real[],
    FUNCTION = cosine_distance_arr,
    COMMUTATOR = '<=>'
);

COMMENT ON OPERATOR <=>(real[], real[]) IS 'Cosine distance';

-- Inner product operator: <#>
CREATE OPERATOR <#> (
    LEFTARG = real[],
    RIGHTARG = real[],
    FUNCTION = neg_inner_product_arr,
    COMMUTATOR = '<#>'
);

COMMENT ON OPERATOR <#>(real[], real[]) IS 'Negative inner product (for ORDER BY)';

-- ============================================================================
-- Operator Classes for HNSW - L2 Distance
-- ============================================================================

CREATE OPERATOR CLASS hnsw_l2_ops
    FOR TYPE real[] USING hnsw
    FAMILY hnsw_l2_ops AS
    -- Distance operator for ORDER BY
    OPERATOR 1 <-> (real[], real[]) FOR ORDER BY float_ops,
    -- Support function: distance calculation
    FUNCTION 1 l2_distance_arr(real[], real[]);

COMMENT ON OPERATOR CLASS hnsw_l2_ops USING hnsw IS
    'HNSW index operator class for L2 (Euclidean) distance on real[] vectors';

-- ============================================================================
-- Operator Classes for HNSW - Cosine Distance
-- ============================================================================

CREATE OPERATOR CLASS hnsw_cosine_ops
    FOR TYPE real[] USING hnsw
    FAMILY hnsw_cosine_ops AS
    -- Distance operator for ORDER BY
    OPERATOR 1 <=> (real[], real[]) FOR ORDER BY float_ops,
    -- Support function: distance calculation
    FUNCTION 1 cosine_distance_arr(real[], real[]);

COMMENT ON OPERATOR CLASS hnsw_cosine_ops USING hnsw IS
    'HNSW index operator class for cosine distance on real[] vectors';

-- ============================================================================
-- Operator Classes for HNSW - Inner Product
-- ============================================================================

CREATE OPERATOR CLASS hnsw_ip_ops
    FOR TYPE real[] USING hnsw
    FAMILY hnsw_ip_ops AS
    -- Distance operator for ORDER BY
    OPERATOR 1 <#> (real[], real[]) FOR ORDER BY float_ops,
    -- Support function: distance calculation
    FUNCTION 1 neg_inner_product_arr(real[], real[]);

COMMENT ON OPERATOR CLASS hnsw_ip_ops USING hnsw IS
    'HNSW index operator class for inner product on real[] vectors';

-- ============================================================================
-- Index Creation Syntax Examples
-- ============================================================================

/*
-- Create table with vectors
CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    embedding real[]
);

-- Create HNSW index with L2 distance (default)
CREATE INDEX ON items USING hnsw (embedding hnsw_l2_ops);

-- Create HNSW index with options
CREATE INDEX ON items USING hnsw (embedding hnsw_l2_ops)
    WITH (m = 16, ef_construction = 64);

-- Create HNSW index with cosine distance
CREATE INDEX ON items USING hnsw (embedding hnsw_cosine_ops);

-- Create HNSW index with inner product
CREATE INDEX ON items USING hnsw (embedding hnsw_ip_ops);

-- Query examples:

-- Find 10 nearest neighbors using L2 distance
SELECT id, embedding <-> ARRAY[0.1, 0.2, 0.3]::real[] AS distance
FROM items
ORDER BY embedding <-> ARRAY[0.1, 0.2, 0.3]::real[]
LIMIT 10;

-- Find 10 nearest neighbors using cosine distance
SELECT id, embedding <=> ARRAY[0.1, 0.2, 0.3]::real[] AS distance
FROM items
ORDER BY embedding <=> ARRAY[0.1, 0.2, 0.3]::real[]
LIMIT 10;

-- Find 10 nearest neighbors using inner product
SELECT id, embedding <#> ARRAY[0.1, 0.2, 0.3]::real[] AS distance
FROM items
ORDER BY embedding <#> ARRAY[0.1, 0.2, 0.3]::real[]
LIMIT 10;

-- Index parameters:
-- - m: Maximum number of connections per layer (default: 16)
--      Higher values improve recall but increase memory usage
-- - ef_construction: Size of dynamic candidate list during construction (default: 64)
--      Higher values improve index quality but slow down build time
-- - ef_search: Size of dynamic candidate list during search (default: 40, set via GUC)
--      Higher values improve recall but slow down queries
--      Can be set per-session: SET ruvector.ef_search = 100;
*/

-- ============================================================================
-- Index Options Support
-- ============================================================================
-- Note: The actual options parsing is handled in the Rust code via hnsw_options callback
-- Supported options:
-- - m (integer): Maximum connections per layer, default 16, range 2-128
-- - ef_construction (integer): Construction candidate list size, default 64, range 4-1000
-- - metric (string): Distance metric 'l2', 'cosine', or 'ip', default 'l2'

-- ============================================================================
-- Performance Tuning
-- ============================================================================

-- Global settings (in postgresql.conf or ALTER SYSTEM):
-- ruvector.ef_search = 40          # Query-time candidate list size
-- ruvector.maintenance_work_mem    # Use standard PostgreSQL setting

-- Session settings:
-- SET ruvector.ef_search = 100;    # Increase recall for current session
-- SET maintenance_work_mem = '1GB'; # Increase for faster index builds

-- ============================================================================
-- Monitoring and Maintenance
-- ============================================================================

-- View index statistics
SELECT ruvector_memory_stats();

-- Perform index maintenance (rebuild connections, optimize graph)
SELECT ruvector_index_maintenance('items_embedding_idx');

-- Check index size
SELECT pg_size_pretty(pg_relation_size('items_embedding_idx'));

-- View index definition
SELECT indexdef FROM pg_indexes WHERE indexname = 'items_embedding_idx';
