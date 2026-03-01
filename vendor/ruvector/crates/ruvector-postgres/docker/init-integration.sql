-- RuVector-Postgres Integration Test Initialization
-- Sets up comprehensive test environment with multiple schemas and test data

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS ruvector;

-- Log initialization
DO $$
BEGIN
    RAISE NOTICE '========================================';
    RAISE NOTICE 'RuVector Integration Test Initialization';
    RAISE NOTICE '========================================';
END $$;

-- ============================================================================
-- Test Schemas
-- ============================================================================

-- pgvector compatibility tests
CREATE SCHEMA IF NOT EXISTS test_pgvector;
COMMENT ON SCHEMA test_pgvector IS 'pgvector SQL compatibility tests';

-- Integrity system tests
CREATE SCHEMA IF NOT EXISTS test_integrity;
COMMENT ON SCHEMA test_integrity IS 'Integrity and mincut tests';

-- Hybrid search tests
CREATE SCHEMA IF NOT EXISTS test_hybrid;
COMMENT ON SCHEMA test_hybrid IS 'Hybrid BM25+vector search tests';

-- Multi-tenancy tests
CREATE SCHEMA IF NOT EXISTS test_tenancy;
COMMENT ON SCHEMA test_tenancy IS 'Multi-tenant isolation tests';

-- Self-healing tests
CREATE SCHEMA IF NOT EXISTS test_healing;
COMMENT ON SCHEMA test_healing IS 'Self-healing and recovery tests';

-- Performance tests
CREATE SCHEMA IF NOT EXISTS test_perf;
COMMENT ON SCHEMA test_perf IS 'Performance benchmarks';

-- ============================================================================
-- Test Tables
-- ============================================================================

-- pgvector compatibility test table
CREATE TABLE test_pgvector.vectors (
    id SERIAL PRIMARY KEY,
    embedding vector(128),
    metadata JSONB,
    category TEXT,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Table for HNSW index testing
CREATE TABLE test_pgvector.hnsw_vectors (
    id SERIAL PRIMARY KEY,
    embedding vector(128),
    label TEXT
);

-- Table for IVFFlat index testing
CREATE TABLE test_pgvector.ivfflat_vectors (
    id SERIAL PRIMARY KEY,
    embedding vector(128),
    label TEXT
);

-- Integrity test tables
CREATE TABLE test_integrity.graph_nodes (
    id SERIAL PRIMARY KEY,
    embedding vector(64),
    layer INTEGER DEFAULT 0,
    connections INTEGER[]
);

CREATE TABLE test_integrity.metrics (
    id SERIAL PRIMARY KEY,
    timestamp TIMESTAMP DEFAULT NOW(),
    mincut_value INTEGER,
    load_factor FLOAT,
    error_rate FLOAT,
    state TEXT
);

-- Hybrid search test tables
CREATE TABLE test_hybrid.documents (
    id SERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    embedding vector(384),
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE test_hybrid.search_results (
    id SERIAL PRIMARY KEY,
    query_id INTEGER,
    doc_id INTEGER,
    vector_score FLOAT,
    text_score FLOAT,
    fused_score FLOAT,
    rank INTEGER
);

-- Multi-tenancy test tables
CREATE TABLE test_tenancy.tenant_config (
    tenant_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    max_vectors BIGINT DEFAULT 100000,
    max_storage_bytes BIGINT DEFAULT 1073741824,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE test_tenancy.tenant_vectors (
    id SERIAL,
    tenant_id UUID NOT NULL,
    embedding vector(128),
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW(),
    PRIMARY KEY (tenant_id, id)
);

CREATE TABLE test_tenancy.tenant_usage (
    tenant_id UUID PRIMARY KEY,
    vector_count BIGINT DEFAULT 0,
    storage_bytes BIGINT DEFAULT 0,
    query_count BIGINT DEFAULT 0,
    last_updated TIMESTAMP DEFAULT NOW()
);

-- Self-healing test tables
CREATE TABLE test_healing.health_metrics (
    id SERIAL PRIMARY KEY,
    timestamp TIMESTAMP DEFAULT NOW(),
    metric_name TEXT NOT NULL,
    metric_value FLOAT NOT NULL,
    threshold FLOAT,
    status TEXT
);

CREATE TABLE test_healing.remediation_log (
    id SERIAL PRIMARY KEY,
    timestamp TIMESTAMP DEFAULT NOW(),
    problem_type TEXT NOT NULL,
    action_taken TEXT NOT NULL,
    success BOOLEAN,
    recovery_time_ms INTEGER,
    notes TEXT
);

CREATE TABLE test_healing.learning_records (
    id SERIAL PRIMARY KEY,
    timestamp TIMESTAMP DEFAULT NOW(),
    problem_context JSONB,
    action TEXT,
    outcome JSONB,
    confidence FLOAT DEFAULT 0.5
);

-- Performance test tables
CREATE TABLE test_perf.benchmark_vectors (
    id SERIAL PRIMARY KEY,
    embedding vector(128),
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE test_perf.benchmark_results (
    id SERIAL PRIMARY KEY,
    benchmark_name TEXT NOT NULL,
    timestamp TIMESTAMP DEFAULT NOW(),
    iterations INTEGER,
    total_time_ms FLOAT,
    avg_time_ms FLOAT,
    p50_time_ms FLOAT,
    p95_time_ms FLOAT,
    p99_time_ms FLOAT,
    throughput FLOAT,
    notes TEXT
);

-- ============================================================================
-- Indexes
-- ============================================================================

-- HNSW indexes for different test scenarios
CREATE INDEX test_pgvector_vectors_hnsw ON test_pgvector.vectors
    USING hnsw (embedding vector_l2_ops) WITH (m = 16, ef_construction = 64);

CREATE INDEX test_pgvector_hnsw_idx ON test_pgvector.hnsw_vectors
    USING hnsw (embedding vector_l2_ops) WITH (m = 16, ef_construction = 64);

-- IVFFlat index
CREATE INDEX test_pgvector_ivfflat_idx ON test_pgvector.ivfflat_vectors
    USING ivfflat (embedding vector_l2_ops) WITH (lists = 100);

-- Performance benchmark index
CREATE INDEX test_perf_benchmark_hnsw ON test_perf.benchmark_vectors
    USING hnsw (embedding vector_l2_ops) WITH (m = 16, ef_construction = 64);

-- Hybrid search indexes
CREATE INDEX test_hybrid_docs_embedding ON test_hybrid.documents
    USING hnsw (embedding vector_l2_ops) WITH (m = 16, ef_construction = 64);

-- GIN index for text search
CREATE INDEX test_hybrid_docs_content ON test_hybrid.documents
    USING gin (to_tsvector('english', content));

-- Multi-tenancy indexes
CREATE INDEX test_tenancy_vectors_tenant ON test_tenancy.tenant_vectors (tenant_id);
CREATE INDEX test_tenancy_vectors_hnsw ON test_tenancy.tenant_vectors
    USING hnsw (embedding vector_l2_ops) WITH (m = 16, ef_construction = 64);

-- ============================================================================
-- Test Data
-- ============================================================================

-- Insert pgvector compatibility test data
INSERT INTO test_pgvector.vectors (embedding, metadata, category)
SELECT
    (SELECT array_agg(random()::real) FROM generate_series(1, 128))::vector,
    jsonb_build_object('idx', i, 'batch', 'init'),
    CASE WHEN i % 3 = 0 THEN 'A' WHEN i % 3 = 1 THEN 'B' ELSE 'C' END
FROM generate_series(1, 1000) i;

-- Insert HNSW test data
INSERT INTO test_pgvector.hnsw_vectors (embedding, label)
SELECT
    (SELECT array_agg(random()::real) FROM generate_series(1, 128))::vector,
    'hnsw_' || i
FROM generate_series(1, 500) i;

-- Insert IVFFlat test data
INSERT INTO test_pgvector.ivfflat_vectors (embedding, label)
SELECT
    (SELECT array_agg(random()::real) FROM generate_series(1, 128))::vector,
    'ivf_' || i
FROM generate_series(1, 500) i;

-- Insert hybrid search test data
INSERT INTO test_hybrid.documents (title, content, embedding)
VALUES
    ('Machine Learning Basics', 'Introduction to supervised and unsupervised learning algorithms.',
     (SELECT array_agg(random()::real) FROM generate_series(1, 384))::vector),
    ('Deep Learning', 'Neural networks and deep learning architectures for complex pattern recognition.',
     (SELECT array_agg(random()::real) FROM generate_series(1, 384))::vector),
    ('Natural Language Processing', 'Text processing and understanding using transformer models.',
     (SELECT array_agg(random()::real) FROM generate_series(1, 384))::vector),
    ('Computer Vision', 'Image recognition and object detection with convolutional networks.',
     (SELECT array_agg(random()::real) FROM generate_series(1, 384))::vector),
    ('Reinforcement Learning', 'Agent-based learning through reward optimization.',
     (SELECT array_agg(random()::real) FROM generate_series(1, 384))::vector);

-- Insert multi-tenancy test data
INSERT INTO test_tenancy.tenant_config (tenant_id, name, max_vectors, max_storage_bytes)
VALUES
    ('00000000-0000-0000-0000-000000000001', 'Tenant A', 100000, 1073741824),
    ('00000000-0000-0000-0000-000000000002', 'Tenant B', 50000, 536870912),
    ('00000000-0000-0000-0000-000000000003', 'Tenant C', 200000, 2147483648);

-- Insert vectors for each tenant
INSERT INTO test_tenancy.tenant_vectors (tenant_id, embedding, metadata)
SELECT
    '00000000-0000-0000-0000-00000000000' || ((i % 3) + 1)::text,
    (SELECT array_agg(random()::real) FROM generate_series(1, 128))::vector,
    jsonb_build_object('idx', i)
FROM generate_series(1, 300) i;

-- Update usage tracking
INSERT INTO test_tenancy.tenant_usage (tenant_id, vector_count, storage_bytes)
SELECT
    tenant_id,
    COUNT(*),
    COUNT(*) * 512  -- Approximate bytes per vector
FROM test_tenancy.tenant_vectors
GROUP BY tenant_id;

-- ============================================================================
-- Row-Level Security Setup
-- ============================================================================

-- Enable RLS on tenant tables
ALTER TABLE test_tenancy.tenant_vectors ENABLE ROW LEVEL SECURITY;

-- Create tenant isolation policy
CREATE POLICY tenant_isolation ON test_tenancy.tenant_vectors
    USING (tenant_id = COALESCE(
        NULLIF(current_setting('app.tenant_id', true), '')::uuid,
        tenant_id
    ));

-- ============================================================================
-- Statistics and Verification
-- ============================================================================

-- Analyze all test tables
ANALYZE test_pgvector.vectors;
ANALYZE test_pgvector.hnsw_vectors;
ANALYZE test_pgvector.ivfflat_vectors;
ANALYZE test_hybrid.documents;
ANALYZE test_tenancy.tenant_vectors;
ANALYZE test_perf.benchmark_vectors;

-- Verify setup
DO $$
DECLARE
    vec_count INTEGER;
    idx_count INTEGER;
    schema_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO vec_count FROM test_pgvector.vectors;
    SELECT COUNT(*) INTO idx_count FROM pg_indexes WHERE schemaname LIKE 'test_%';
    SELECT COUNT(*) INTO schema_count FROM information_schema.schemata WHERE schema_name LIKE 'test_%';

    RAISE NOTICE '========================================';
    RAISE NOTICE 'Integration Test Setup Complete';
    RAISE NOTICE '========================================';
    RAISE NOTICE 'Test schemas created: %', schema_count;
    RAISE NOTICE 'Test vectors inserted: %', vec_count;
    RAISE NOTICE 'Test indexes created: %', idx_count;
    RAISE NOTICE '';
    RAISE NOTICE 'Extension version: %', ruvector_version();
    RAISE NOTICE 'SIMD info: %', ruvector_simd_info();
    RAISE NOTICE '========================================';
END $$;
