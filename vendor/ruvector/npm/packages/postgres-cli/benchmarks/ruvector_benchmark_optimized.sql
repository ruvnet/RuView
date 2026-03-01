-- RuVector Optimized Benchmark Setup
-- Performance-optimized schema with indexes and parallel-safe functions

-- Enable extension
CREATE EXTENSION IF NOT EXISTS ruvector;

-- ============================================================================
-- Optimized Vector Table with HNSW Index
-- ============================================================================
DROP TABLE IF EXISTS benchmark_vectors CASCADE;
CREATE TABLE benchmark_vectors (
    id SERIAL PRIMARY KEY,
    embedding ruvector,
    category TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Insert test vectors (1000 random 128-dim vectors)
INSERT INTO benchmark_vectors (embedding, category)
SELECT
    ruvector_random(128),
    'category_' || (random() * 10)::int
FROM generate_series(1, 1000);

-- Create HNSW index for fast similarity search
-- m=16: connections per layer, ef_construction=100: build-time accuracy
CREATE INDEX IF NOT EXISTS idx_vectors_hnsw
ON benchmark_vectors USING hnsw (embedding ruvector_cosine_ops)
WITH (m = 16, ef_construction = 100);

-- ============================================================================
-- Optimized Full-Text Search with GIN Index
-- ============================================================================
DROP TABLE IF EXISTS benchmark_documents CASCADE;
CREATE TABLE benchmark_documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    content_tsvector TSVECTOR GENERATED ALWAYS AS (to_tsvector('english', content)) STORED,
    sparse_embedding TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Insert test documents
INSERT INTO benchmark_documents (content, sparse_embedding)
SELECT
    'Document ' || i || ' contains words like vector database similarity search embedding neural network',
    ruvector_sparse_from_dense(ARRAY[random(), 0, random(), 0, random(), 0, random(), 0]::float4[])
FROM generate_series(1, 500) i;

-- GIN index for full-text search
CREATE INDEX IF NOT EXISTS idx_documents_fts
ON benchmark_documents USING gin (content_tsvector);

-- ============================================================================
-- Optimized Graph Tables with B-tree Indexes
-- ============================================================================
DROP TABLE IF EXISTS benchmark_edges CASCADE;
DROP TABLE IF EXISTS benchmark_nodes CASCADE;

CREATE TABLE benchmark_nodes (
    id SERIAL PRIMARY KEY,
    features ruvector,
    node_type TEXT
);

CREATE TABLE benchmark_edges (
    id SERIAL PRIMARY KEY,
    source_id INT REFERENCES benchmark_nodes(id),
    target_id INT REFERENCES benchmark_nodes(id),
    edge_type TEXT,
    weight FLOAT DEFAULT 1.0
);

-- Insert test graph data
INSERT INTO benchmark_nodes (features, node_type)
SELECT
    ruvector_random(64),
    'type_' || (random() * 5)::int
FROM generate_series(1, 200);

INSERT INTO benchmark_edges (source_id, target_id, edge_type, weight)
SELECT
    (random() * 199 + 1)::int,
    (random() * 199 + 1)::int,
    'edge_' || (random() * 3)::int,
    random()
FROM generate_series(1, 1000);

-- B-tree indexes for fast edge lookups
CREATE INDEX IF NOT EXISTS idx_edges_source ON benchmark_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_edges_target ON benchmark_edges(target_id);
CREATE INDEX IF NOT EXISTS idx_edges_source_target ON benchmark_edges(source_id, target_id);

-- ============================================================================
-- Optimized Quantization Tables
-- ============================================================================
DROP TABLE IF EXISTS benchmark_quantized CASCADE;
CREATE TABLE benchmark_quantized (
    id SERIAL PRIMARY KEY,
    original ruvector,
    binary_quantized BIT VARYING,
    scalar_quantized BYTEA
);

-- Insert and quantize vectors
INSERT INTO benchmark_quantized (original, binary_quantized, scalar_quantized)
SELECT
    v.embedding,
    ruvector_binary_quantize(v.embedding),
    ruvector_scalar_quantize(v.embedding, 8)
FROM benchmark_vectors v
LIMIT 500;

-- ============================================================================
-- Parallel-Safe Helper Functions
-- ============================================================================

-- Parallel-safe cosine distance function
CREATE OR REPLACE FUNCTION bench_cosine_distance(a ruvector, b ruvector)
RETURNS float8 AS $$
    SELECT ruvector_distance(a, b, 'cosine')
$$ LANGUAGE SQL IMMUTABLE PARALLEL SAFE;

-- Parallel-safe Hamming distance using bit_count
CREATE OR REPLACE FUNCTION bench_hamming_distance(a BIT VARYING, b BIT VARYING)
RETURNS int AS $$
    SELECT bit_count(a # b)::int
$$ LANGUAGE SQL IMMUTABLE PARALLEL SAFE;

-- Parallel-safe sparse dot product
CREATE OR REPLACE FUNCTION bench_sparse_dot(a TEXT, b TEXT)
RETURNS float8 AS $$
    SELECT ruvector_sparse_distance(a, b, 'cosine')
$$ LANGUAGE SQL IMMUTABLE PARALLEL SAFE;

-- ============================================================================
-- Statistics Update
-- ============================================================================
ANALYZE benchmark_vectors;
ANALYZE benchmark_documents;
ANALYZE benchmark_nodes;
ANALYZE benchmark_edges;
ANALYZE benchmark_quantized;

SELECT 'Optimized benchmark setup complete' AS status;
