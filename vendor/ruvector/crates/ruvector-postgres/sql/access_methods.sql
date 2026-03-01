-- ============================================================================
-- HNSW Access Method
-- ============================================================================

-- HNSW Access Method Handler
CREATE OR REPLACE FUNCTION hnsw_handler(internal)
RETURNS index_am_handler
AS 'MODULE_PATHNAME', 'hnsw_handler_wrapper'
LANGUAGE C STRICT;

-- Create HNSW Access Method
CREATE ACCESS METHOD hnsw TYPE INDEX HANDLER hnsw_handler;

-- ============================================================================
-- Operator Classes for HNSW
-- ============================================================================

-- HNSW Operator Class for L2 (Euclidean) distance
CREATE OPERATOR CLASS ruvector_l2_ops
    DEFAULT FOR TYPE ruvector USING hnsw AS
    OPERATOR 1 <-> (ruvector, ruvector) FOR ORDER BY float_ops,
    FUNCTION 1 ruvector_l2_distance(ruvector, ruvector);

COMMENT ON OPERATOR CLASS ruvector_l2_ops USING hnsw IS
'ruvector HNSW operator class for L2/Euclidean distance';

-- HNSW Operator Class for Cosine distance
CREATE OPERATOR CLASS ruvector_cosine_ops
    FOR TYPE ruvector USING hnsw AS
    OPERATOR 1 <=> (ruvector, ruvector) FOR ORDER BY float_ops,
    FUNCTION 1 ruvector_cosine_distance(ruvector, ruvector);

COMMENT ON OPERATOR CLASS ruvector_cosine_ops USING hnsw IS
'ruvector HNSW operator class for cosine distance';

-- HNSW Operator Class for Inner Product
CREATE OPERATOR CLASS ruvector_ip_ops
    FOR TYPE ruvector USING hnsw AS
    OPERATOR 1 <#> (ruvector, ruvector) FOR ORDER BY float_ops,
    FUNCTION 1 ruvector_inner_product(ruvector, ruvector);

COMMENT ON OPERATOR CLASS ruvector_ip_ops USING hnsw IS
'ruvector HNSW operator class for inner product (max similarity)';
