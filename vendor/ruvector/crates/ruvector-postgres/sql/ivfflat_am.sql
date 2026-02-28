-- IVFFlat Index Access Method Installation
-- ============================================================================
-- Creates the ruivfflat access method for PostgreSQL
-- Compatible with pgvector's ivfflat interface

-- Create access method
CREATE ACCESS METHOD ruivfflat TYPE INDEX HANDLER ruivfflat_handler;

-- Create operator classes for different distance metrics

-- L2 (Euclidean) distance operator class
CREATE OPERATOR CLASS ruvector_ivfflat_l2_ops
    FOR TYPE vector USING ruivfflat AS
    OPERATOR 1 <-> (vector, vector) FOR ORDER BY float_ops,
    FUNCTION 1 ruvector_l2_distance(vector, vector);

-- Inner product distance operator class
CREATE OPERATOR CLASS ruvector_ivfflat_ip_ops
    FOR TYPE vector USING ruivfflat AS
    OPERATOR 1 <#> (vector, vector) FOR ORDER BY float_ops,
    FUNCTION 1 ruvector_ip_distance(vector, vector);

-- Cosine distance operator class
CREATE OPERATOR CLASS ruvector_ivfflat_cosine_ops
    FOR TYPE vector USING ruivfflat AS
    OPERATOR 1 <=> (vector, vector) FOR ORDER BY float_ops,
    FUNCTION 1 ruvector_cosine_distance(vector, vector);

-- Helper function to get IVFFlat index statistics
CREATE OR REPLACE FUNCTION ruvector_ivfflat_stats(index_name text)
RETURNS TABLE(
    lists integer,
    probes integer,
    dimensions integer,
    trained boolean,
    vector_count bigint,
    metric text
)
AS $$
BEGIN
    -- This would query the index metadata
    -- For now, return dummy data
    RETURN QUERY SELECT
        100::integer as lists,
        1::integer as probes,
        0::integer as dimensions,
        false::boolean as trained,
        0::bigint as vector_count,
        'euclidean'::text as metric;
END;
$$ LANGUAGE plpgsql;

-- Example usage:
--
-- CREATE INDEX ON items USING ruivfflat (embedding vector_l2_ops)
--   WITH (lists = 100, probes = 1);
--
-- CREATE INDEX ON items USING ruivfflat (embedding vector_cosine_ops)
--   WITH (lists = 500, probes = 10);
--
-- SELECT * FROM ruvector_ivfflat_stats('items_embedding_idx');
