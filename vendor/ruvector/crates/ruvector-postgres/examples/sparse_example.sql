-- Sparse Vectors Example Usage
-- This file demonstrates the sparse vector functionality

-- ============================================================================
-- Setup
-- ============================================================================

-- Create extension (assuming already installed)
-- CREATE EXTENSION IF NOT EXISTS ruvector_postgres;

-- Create sample tables
CREATE TABLE IF NOT EXISTS sparse_documents (
    id SERIAL PRIMARY KEY,
    title TEXT,
    content TEXT,
    sparse_embedding sparsevec,
    created_at TIMESTAMP DEFAULT NOW()
);

-- ============================================================================
-- Inserting Data
-- ============================================================================

-- Method 1: String format
INSERT INTO sparse_documents (title, content, sparse_embedding) VALUES
    ('Machine Learning Basics', 
     'Introduction to neural networks and deep learning',
     '{1024:0.5, 2048:0.3, 4096:0.8, 8192:0.2}'::sparsevec),
    
    ('Natural Language Processing',
     'Text processing and language models',
     '{1024:0.3, 3072:0.7, 4096:0.4, 9216:0.6}'::sparsevec),
    
    ('Computer Vision',
     'Image recognition and object detection',
     '{2048:0.9, 5120:0.4, 6144:0.5, 7168:0.3}'::sparsevec);

-- Method 2: Array construction
INSERT INTO sparse_documents (title, content, sparse_embedding) VALUES
    ('Reinforcement Learning',
     'Q-learning and policy gradients',
     ruvector_to_sparse(
         ARRAY[1024, 4096, 10240]::int[],
         ARRAY[0.6, 0.8, 0.4]::real[],
         30000
     ));

-- Method 3: Convert from dense
INSERT INTO sparse_documents (title, sparse_embedding)
SELECT 'From Dense Vector',
       ruvector_dense_to_sparse(
           ARRAY[0, 0.5, 0, 0.3, 0, 0, 0.8, 0, 0, 0.2]::real[]
       );

-- ============================================================================
-- Basic Queries
-- ============================================================================

-- View all documents with sparse vectors
SELECT id, title,
       ruvector_sparse_nnz(sparse_embedding) as num_nonzero,
       ruvector_sparse_dim(sparse_embedding) as dimension,
       ruvector_sparse_norm(sparse_embedding) as l2_norm
FROM sparse_documents;

-- ============================================================================
-- Similarity Search
-- ============================================================================

-- Define a query vector
WITH query AS (
    SELECT '{1024:0.5, 2048:0.3, 4096:0.8}'::sparsevec AS query_vec
)
-- Search by dot product (inner product)
SELECT d.id, d.title,
       ruvector_sparse_dot(d.sparse_embedding, q.query_vec) AS dot_product,
       ruvector_sparse_cosine(d.sparse_embedding, q.query_vec) AS cosine_sim,
       ruvector_sparse_euclidean(d.sparse_embedding, q.query_vec) AS euclidean_dist
FROM sparse_documents d, query q
ORDER BY dot_product DESC
LIMIT 5;

-- Find documents with high cosine similarity
WITH query AS (
    SELECT '{1024:0.5, 4096:0.8}'::sparsevec AS query_vec
)
SELECT id, title,
       ruvector_sparse_cosine(sparse_embedding, query_vec) AS similarity
FROM sparse_documents, query
WHERE ruvector_sparse_cosine(sparse_embedding, query_vec) > 0.3
ORDER BY similarity DESC;

-- ============================================================================
-- Sparsification Operations
-- ============================================================================

-- Keep only top-k elements
SELECT id, title,
       sparse_embedding AS original,
       ruvector_sparse_top_k(sparse_embedding, 2) AS top_2_elements
FROM sparse_documents
LIMIT 3;

-- Prune small values
SELECT id, title,
       sparse_embedding AS original,
       ruvector_sparse_prune(sparse_embedding, 0.4) AS pruned
FROM sparse_documents
LIMIT 3;

-- ============================================================================
-- BM25 Text Search Example
-- ============================================================================

-- Create BM25-specific table
CREATE TABLE IF NOT EXISTS bm25_articles (
    id SERIAL PRIMARY KEY,
    title TEXT,
    content TEXT,
    term_frequencies sparsevec,  -- TF values
    doc_length REAL
);

-- Insert sample documents with term frequencies
INSERT INTO bm25_articles (title, content, term_frequencies, doc_length) VALUES
    ('AI Research Paper',
     'Deep learning models for natural language processing',
     '{100:2.0, 200:1.0, 300:3.0, 400:1.0}'::sparsevec,  -- TF values
     7.0),
    
    ('Machine Learning Tutorial',
     'Introduction to supervised and unsupervised learning',
     '{100:1.0, 250:2.0, 300:1.0, 500:2.0}'::sparsevec,
     6.0),
    
    ('Data Science Guide',
     'Statistical analysis and data visualization techniques',
     '{150:1.0, 250:1.0, 350:2.0, 450:1.0}'::sparsevec,
     6.0);

-- BM25 search
WITH 
    query AS (
        -- Query with IDF weights (normally computed from corpus)
        SELECT '{100:1.5, 300:2.0, 400:1.2}'::sparsevec AS query_idf
    ),
    collection_stats AS (
        SELECT AVG(doc_length) AS avg_doc_len
        FROM bm25_articles
    )
SELECT a.id, a.title,
       ruvector_sparse_bm25(
           q.query_idf,
           a.term_frequencies,
           a.doc_length,
           cs.avg_doc_len,
           1.2,    -- k1 parameter
           0.75    -- b parameter
       ) AS bm25_score
FROM bm25_articles a, query q, collection_stats cs
ORDER BY bm25_score DESC
LIMIT 5;

-- ============================================================================
-- Hybrid Search (Dense + Sparse)
-- ============================================================================

-- Create hybrid table (requires vector extension)
-- Uncomment if you have dense vector support
/*
CREATE TABLE IF NOT EXISTS hybrid_documents (
    id SERIAL PRIMARY KEY,
    title TEXT,
    dense_embedding vector(768),
    sparse_embedding sparsevec
);

-- Hybrid search combining both signals
WITH query AS (
    SELECT 
        random_vector(768) AS query_dense,  -- Replace with actual query
        '{1024:0.5, 2048:0.3}'::sparsevec AS query_sparse
)
SELECT id, title,
       0.7 * (1 - (dense_embedding <=> query_dense)) +  -- Dense similarity
       0.3 * ruvector_sparse_dot(sparse_embedding, query_sparse) AS hybrid_score
FROM hybrid_documents, query
ORDER BY hybrid_score DESC
LIMIT 10;
*/

-- ============================================================================
-- Utility Operations
-- ============================================================================

-- Convert sparse to dense
SELECT id, title,
       ruvector_sparse_to_dense(sparse_embedding) AS dense_array
FROM sparse_documents
LIMIT 3;

-- Get vector statistics
SELECT 
    COUNT(*) as num_documents,
    AVG(ruvector_sparse_nnz(sparse_embedding)) AS avg_nonzero,
    MIN(ruvector_sparse_nnz(sparse_embedding)) AS min_nonzero,
    MAX(ruvector_sparse_nnz(sparse_embedding)) AS max_nonzero,
    AVG(ruvector_sparse_norm(sparse_embedding)) AS avg_norm
FROM sparse_documents;

-- Find documents with similar sparsity
WITH target AS (
    SELECT sparse_embedding, ruvector_sparse_nnz(sparse_embedding) AS target_nnz
    FROM sparse_documents
    WHERE id = 1
)
SELECT d.id, d.title,
       ruvector_sparse_nnz(d.sparse_embedding) AS doc_nnz,
       ABS(ruvector_sparse_nnz(d.sparse_embedding) - t.target_nnz) AS nnz_diff
FROM sparse_documents d, target t
WHERE d.id != 1
ORDER BY nnz_diff
LIMIT 5;

-- ============================================================================
-- Performance Analysis
-- ============================================================================

-- Check storage size
SELECT id, title,
       pg_column_size(sparse_embedding) AS sparse_bytes,
       ruvector_sparse_nnz(sparse_embedding) AS num_nonzero,
       pg_column_size(sparse_embedding)::float / 
           GREATEST(ruvector_sparse_nnz(sparse_embedding), 1) AS bytes_per_element
FROM sparse_documents
ORDER BY sparse_bytes DESC;

-- Batch similarity computation
EXPLAIN ANALYZE
WITH queries AS (
    SELECT generate_series(1, 3) AS query_id,
           '{1024:0.5, 2048:0.3}'::sparsevec AS query_vec
)
SELECT q.query_id, d.id, d.title,
       ruvector_sparse_dot(d.sparse_embedding, q.query_vec) AS score
FROM sparse_documents d
CROSS JOIN queries q
ORDER BY q.query_id, score DESC;

-- ============================================================================
-- Cleanup (optional)
-- ============================================================================

-- DROP TABLE IF EXISTS sparse_documents CASCADE;
-- DROP TABLE IF EXISTS bm25_articles CASCADE;
-- DROP TABLE IF EXISTS hybrid_documents CASCADE;
