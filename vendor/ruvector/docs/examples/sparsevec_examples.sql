-- ============================================================================
-- SparseVec PostgreSQL Type - Usage Examples
-- ============================================================================

-- Basic Usage
-- ============================================================================

-- Create a sparse vector with format {idx:val,idx:val,...}/dimensions
SELECT '{0:1.5,3:2.5,7:3.5}/10'::sparsevec;

-- Create an empty sparse vector
SELECT '{}/100'::sparsevec;

-- Create a dense sparse vector (many non-zeros)
SELECT '{0:1.0,1:2.0,2:3.0,3:4.0,4:5.0}/5'::sparsevec;

-- Introspection
-- ============================================================================

-- Get dimensions
SELECT sparsevec_dims('{0:1.5,3:2.5,7:3.5}/10'::sparsevec);
-- Returns: 10

-- Get number of non-zero elements
SELECT sparsevec_nnz('{0:1.5,3:2.5,7:3.5}/10'::sparsevec);
-- Returns: 3

-- Get sparsity ratio
SELECT sparsevec_sparsity('{0:1.5,3:2.5,7:3.5}/10'::sparsevec);
-- Returns: 0.3 (30% non-zero)

-- Get L2 norm
SELECT sparsevec_norm('{0:3.0,1:4.0}/5'::sparsevec);
-- Returns: 5.0

-- Get value at specific index
SELECT sparsevec_get('{0:1.5,3:2.5,7:3.5}/10'::sparsevec, 3);
-- Returns: 2.5

SELECT sparsevec_get('{0:1.5,3:2.5,7:3.5}/10'::sparsevec, 5);
-- Returns: 0.0 (not present)

-- Parse and inspect
SELECT sparsevec_parse('{0:1.5,3:2.5,7:3.5}/10');
-- Returns JSON with full details

-- Distance Calculations
-- ============================================================================

-- L2 (Euclidean) distance
SELECT sparsevec_l2_distance(
    '{0:1.0,2:2.0,4:3.0}/5'::sparsevec,
    '{1:1.0,2:1.0,3:2.0}/5'::sparsevec
);

-- Inner product distance (negative dot product)
SELECT sparsevec_ip_distance(
    '{0:1.0,2:2.0}/5'::sparsevec,
    '{2:1.0,4:3.0}/5'::sparsevec
);
-- Returns: -2.0 (only index 2 overlaps: -(2*1))

-- Cosine distance
SELECT sparsevec_cosine_distance(
    '{0:1.0,2:2.0}/5'::sparsevec,
    '{0:2.0,2:4.0}/5'::sparsevec
);
-- Returns: ~0.0 (same direction)

-- Mixed sparse-dense distances
SELECT sparsevec_vector_l2_distance(
    '{0:1.0,3:2.0}/5'::sparsevec,
    '[1.0,0.0,0.0,2.0,0.0]'::ruvector
);

SELECT sparsevec_vector_cosine_distance(
    '{0:1.0,3:2.0}/5'::sparsevec,
    '[1.0,0.0,0.0,2.0,0.0]'::ruvector
);

-- Vector Operations
-- ============================================================================

-- Normalize to unit length
SELECT sparsevec_normalize('{0:3.0,1:4.0}/5'::sparsevec);
-- Returns: {0:0.6,1:0.8}/5

-- Add two sparse vectors
SELECT sparsevec_add(
    '{0:1.0,2:2.0}/5'::sparsevec,
    '{1:3.0,2:1.0}/5'::sparsevec
);
-- Returns: {0:1.0,1:3.0,2:3.0}/5

-- Multiply by scalar
SELECT sparsevec_mul_scalar('{0:1.0,2:2.0}/5'::sparsevec, 2.5);
-- Returns: {0:2.5,2:5.0}/5

-- Conversions
-- ============================================================================

-- Sparse to dense vector
SELECT sparsevec_to_vector('{0:1.0,3:2.0}/5'::sparsevec);
-- Returns: [1.0, 0.0, 0.0, 2.0, 0.0]

-- Dense to sparse with threshold
SELECT vector_to_sparsevec('[0.001,0.5,0.002,1.0,0.003]'::ruvector, 0.01);
-- Returns: {1:0.5,3:1.0}/5 (filters values ≤ 0.01)

-- Sparse to array
SELECT sparsevec_to_array('{0:1.0,3:2.0}/5'::sparsevec);

-- Array to sparse
SELECT array_to_sparsevec(ARRAY[0.001, 0.5, 0.002, 1.0, 0.003]::float4[], 0.01);

-- Table Creation and Queries
-- ============================================================================

-- Create table for text embeddings (TF-IDF)
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT,
    embedding sparsevec(10000)  -- 10K vocabulary
);

-- Insert documents with sparse embeddings
INSERT INTO documents (title, content, embedding) VALUES
('Document 1', 'machine learning artificial intelligence',
 '{45:0.8,123:0.6,789:0.9,1024:0.7}/10000'),
('Document 2', 'deep learning neural networks',
 '{45:0.3,234:0.9,789:0.4,2048:0.8}/10000'),
('Document 3', 'natural language processing',
 '{123:0.7,456:0.9,3072:0.6}/10000');

-- Find similar documents using cosine distance
SELECT
    d.id,
    d.title,
    sparsevec_cosine_distance(d.embedding, query.embedding) AS distance
FROM
    documents d,
    (SELECT embedding FROM documents WHERE id = 1) AS query
WHERE
    d.id != 1
ORDER BY
    distance ASC
LIMIT 5;

-- Find nearest neighbors using L2 distance
SELECT
    d.id,
    d.title,
    sparsevec_l2_distance(d.embedding,
        '{45:0.8,123:0.6,789:0.9}/10000'::sparsevec) AS distance
FROM
    documents d
ORDER BY
    distance ASC
LIMIT 10;

-- Recommender System Example
-- ============================================================================

-- User-item interaction matrix (sparse)
CREATE TABLE user_profiles (
    user_id INT PRIMARY KEY,
    username TEXT NOT NULL,
    preferences sparsevec(100000)  -- 100K items
);

-- Insert user profiles with sparse preference vectors
INSERT INTO user_profiles (user_id, username, preferences) VALUES
(1, 'alice', '{123:5.0,456:4.5,789:3.5,1024:4.0}/100000'),
(2, 'bob', '{123:4.0,234:5.0,789:4.5,2048:3.5}/100000'),
(3, 'carol', '{456:5.0,890:4.0,2048:4.5,3072:5.0}/100000');

-- Collaborative filtering: Find similar users
SELECT
    u2.user_id,
    u2.username,
    sparsevec_cosine_distance(u1.preferences, u2.preferences) AS similarity
FROM
    user_profiles u1,
    user_profiles u2
WHERE
    u1.user_id = 1
    AND u2.user_id != 1
ORDER BY
    similarity ASC
LIMIT 10;

-- Find items user might like (based on similar users)
WITH similar_users AS (
    SELECT
        u2.user_id,
        u2.preferences,
        sparsevec_cosine_distance(u1.preferences, u2.preferences) AS similarity
    FROM
        user_profiles u1,
        user_profiles u2
    WHERE
        u1.user_id = 1
        AND u2.user_id != 1
    ORDER BY
        similarity ASC
    LIMIT 5
)
SELECT
    user_id,
    similarity
FROM
    similar_users;

-- Graph Embeddings Example
-- ============================================================================

-- Store graph node embeddings
CREATE TABLE graph_nodes (
    node_id BIGINT PRIMARY KEY,
    node_type TEXT,
    sparse_embedding sparsevec(50000)
);

-- Insert graph nodes with embeddings
INSERT INTO graph_nodes (node_id, node_type, sparse_embedding) VALUES
(1, 'person', '{100:0.9,500:0.7,1000:0.8}/50000'),
(2, 'product', '{200:0.8,600:0.9,1500:0.7}/50000'),
(3, 'company', '{100:0.5,300:0.8,2000:0.9}/50000');

-- Find nearest neighbors in embedding space
SELECT
    node_id,
    node_type,
    sparsevec_l2_distance(sparse_embedding,
        '{100:0.9,500:0.7,1000:0.8}/50000'::sparsevec) AS distance
FROM
    graph_nodes
WHERE
    node_id != 1
ORDER BY
    distance ASC
LIMIT 20;

-- Statistics and Analytics
-- ============================================================================

-- Analyze sparsity distribution
SELECT
    percentile_cont(0.5) WITHIN GROUP (ORDER BY sparsevec_sparsity(embedding)) AS median_sparsity,
    AVG(sparsevec_sparsity(embedding)) AS avg_sparsity,
    MIN(sparsevec_nnz(embedding)) AS min_nnz,
    MAX(sparsevec_nnz(embedding)) AS max_nnz
FROM
    documents;

-- Find documents with highest/lowest sparsity
SELECT
    id,
    title,
    sparsevec_nnz(embedding) AS non_zeros,
    sparsevec_sparsity(embedding) AS sparsity_ratio
FROM
    documents
ORDER BY
    sparsity_ratio DESC
LIMIT 10;

-- Performance Comparison
-- ============================================================================

-- Compare storage efficiency
SELECT
    'Dense' AS type,
    pg_column_size('[' || array_to_string(array_agg(i::text), ',') || ']'::ruvector) AS bytes
FROM generate_series(1, 10000) AS i
UNION ALL
SELECT
    'Sparse (1% non-zero)' AS type,
    pg_column_size('{' || array_to_string(
        array_agg(i || ':1.0'), ',') || '}/10000'::sparsevec) AS bytes
FROM generate_series(1, 100) AS i;

-- Advanced Queries
-- ============================================================================

-- Batch distance calculation
WITH query_vector AS (
    SELECT '{0:1.0,100:2.0,500:3.0}/10000'::sparsevec AS vec
)
SELECT
    d.id,
    d.title,
    sparsevec_cosine_distance(d.embedding, q.vec) AS distance
FROM
    documents d,
    query_vector q
ORDER BY
    distance ASC;

-- Filter by distance threshold
SELECT
    d.id,
    d.title
FROM
    documents d
WHERE
    sparsevec_cosine_distance(d.embedding,
        '{45:0.8,123:0.6}/10000'::sparsevec) < 0.5
ORDER BY
    id;

-- Aggregate operations
SELECT
    AVG(sparsevec_norm(embedding)) AS avg_norm,
    STDDEV(sparsevec_norm(embedding)) AS stddev_norm
FROM
    documents;

-- Index Creation (Future Enhancement)
-- ============================================================================

-- These would be available once index support is added:
-- CREATE INDEX idx_doc_embedding ON documents
--     USING hnsw (embedding sparsevec_cosine_ops);

-- CREATE INDEX idx_user_prefs ON user_profiles
--     USING ivfflat (preferences sparsevec_l2_ops);

-- Cleanup
-- ============================================================================

-- DROP TABLE IF EXISTS documents;
-- DROP TABLE IF EXISTS user_profiles;
-- DROP TABLE IF EXISTS graph_nodes;
