# HNSW Index - Complete Usage Example

This guide provides a complete, practical example of using the HNSW index for vector similarity search in PostgreSQL.

## Prerequisites

```bash
# Install the extension
cd /home/user/ruvector/crates/ruvector-postgres
cargo pgrx install

# Or package for deployment
cargo pgrx package
```

## Step 1: Create Database and Enable Extension

```sql
-- Create a new database for vector search
CREATE DATABASE vector_search;
\c vector_search

-- Enable the RuVector extension
CREATE EXTENSION ruvector;

-- Verify installation
SELECT ruvector_version();
SELECT ruvector_simd_info();
```

## Step 2: Create Table with Vectors

```sql
-- Create a table for storing document embeddings
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT,
    embedding real[],  -- 384-dimensional embeddings
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Add some metadata indexes
CREATE INDEX idx_documents_created ON documents(created_at);
CREATE INDEX idx_documents_title ON documents USING gin(to_tsvector('english', title));
```

## Step 3: Insert Sample Data

```sql
-- Insert sample documents with random embeddings (in practice, use real embeddings)
INSERT INTO documents (title, content, embedding)
SELECT
    'Document ' || i,
    'This is the content of document ' || i,
    array_agg(random())::real[]
FROM generate_series(1, 10000) AS i
CROSS JOIN generate_series(1, 384) AS dim
GROUP BY i;

-- Verify data
SELECT COUNT(*), pg_size_pretty(pg_total_relation_size('documents'))
FROM documents;
```

## Step 4: Create HNSW Index

```sql
-- Create HNSW index with L2 distance (default parameters)
CREATE INDEX idx_documents_embedding_hnsw
ON documents USING hnsw (embedding hnsw_l2_ops);

-- Check index size
SELECT
    indexname,
    pg_size_pretty(pg_relation_size(indexname::regclass)) AS size
FROM pg_indexes
WHERE tablename = 'documents';
```

## Step 5: Basic Similarity Search

```sql
-- Find 10 most similar documents to a query vector
WITH query AS (
    -- In practice, this would be an embedding from your model
    SELECT array_agg(random())::real[] AS vec
    FROM generate_series(1, 384)
)
SELECT
    d.id,
    d.title,
    d.embedding <-> query.vec AS distance
FROM documents d, query
ORDER BY d.embedding <-> query.vec
LIMIT 10;
```

## Step 6: Advanced Queries

### Filtered Search

```sql
-- Find similar documents created in the last 7 days
WITH query AS (
    SELECT array_agg(random())::real[] AS vec
    FROM generate_series(1, 384)
)
SELECT
    d.id,
    d.title,
    d.created_at,
    d.embedding <-> query.vec AS distance
FROM documents d, query
WHERE d.created_at > CURRENT_TIMESTAMP - INTERVAL '7 days'
ORDER BY d.embedding <-> query.vec
LIMIT 10;
```

### Hybrid Search (Text + Vector)

```sql
-- Combine full-text search with vector similarity
WITH query AS (
    SELECT array_agg(random())::real[] AS vec
    FROM generate_series(1, 384)
)
SELECT
    d.id,
    d.title,
    ts_rank(to_tsvector('english', d.title), to_tsquery('document')) AS text_score,
    d.embedding <-> query.vec AS vector_distance,
    -- Combined score (weighted)
    (0.3 * ts_rank(to_tsvector('english', d.title), to_tsquery('document'))) +
    (0.7 * (1.0 / (1.0 + (d.embedding <-> query.vec)))) AS combined_score
FROM documents d, query
WHERE to_tsvector('english', d.title) @@ to_tsquery('document')
ORDER BY combined_score DESC
LIMIT 10;
```

### Batch Similarity Search

```sql
-- Find similar documents for multiple queries
WITH queries AS (
    SELECT
        q_id,
        array_agg(random())::real[] AS vec
    FROM generate_series(1, 5) AS q_id
    CROSS JOIN generate_series(1, 384)
    GROUP BY q_id
),
results AS (
    SELECT
        q.q_id,
        d.id AS doc_id,
        d.title,
        d.embedding <-> q.vec AS distance,
        ROW_NUMBER() OVER (PARTITION BY q.q_id ORDER BY d.embedding <-> q.vec) AS rank
    FROM queries q
    CROSS JOIN documents d
)
SELECT *
FROM results
WHERE rank <= 10
ORDER BY q_id, rank;
```

## Step 7: Performance Tuning

### Adjust ef_search for Better Recall

```sql
-- Show current setting
SHOW ruvector.ef_search;

-- Increase for better recall (slower queries)
SET ruvector.ef_search = 100;

-- Run query
WITH query AS (
    SELECT array_agg(random())::real[] AS vec
    FROM generate_series(1, 384)
)
SELECT
    d.id,
    d.title,
    d.embedding <-> query.vec AS distance
FROM documents d, query
ORDER BY d.embedding <-> query.vec
LIMIT 10;

-- Reset to default
RESET ruvector.ef_search;
```

### Analyze Query Performance

```sql
-- Explain query plan
EXPLAIN (ANALYZE, BUFFERS)
WITH query AS (
    SELECT array_agg(random())::real[] AS vec
    FROM generate_series(1, 384)
)
SELECT
    d.id,
    d.embedding <-> query.vec AS distance
FROM documents d, query
ORDER BY d.embedding <-> query.vec
LIMIT 10;
```

## Step 8: Different Distance Metrics

### Cosine Distance

```sql
-- Create index with cosine distance
CREATE INDEX idx_documents_embedding_cosine
ON documents USING hnsw (embedding hnsw_cosine_ops);

-- Query with cosine distance (normalized vectors work best)
WITH query AS (
    SELECT vector_normalize(array_agg(random())::real[]) AS vec
    FROM generate_series(1, 384)
)
SELECT
    d.id,
    d.title,
    d.embedding <=> query.vec AS cosine_distance,
    1.0 - (d.embedding <=> query.vec) AS cosine_similarity
FROM documents d, query
ORDER BY d.embedding <=> query.vec
LIMIT 10;
```

### Inner Product

```sql
-- Create index with inner product
CREATE INDEX idx_documents_embedding_ip
ON documents USING hnsw (embedding hnsw_ip_ops);

-- Query with inner product
WITH query AS (
    SELECT array_agg(random())::real[] AS vec
    FROM generate_series(1, 384)
)
SELECT
    d.id,
    d.title,
    d.embedding <#> query.vec AS neg_inner_product,
    -(d.embedding <#> query.vec) AS inner_product
FROM documents d, query
ORDER BY d.embedding <#> query.vec
LIMIT 10;
```

## Step 9: Index Maintenance

### Monitor Index Health

```sql
-- Get memory statistics
SELECT ruvector_memory_stats();

-- Check index bloat
SELECT
    schemaname,
    tablename,
    indexname,
    pg_size_pretty(pg_relation_size(indexrelid)) AS index_size,
    pg_size_pretty(pg_relation_size(relid)) AS table_size,
    ROUND(100.0 * pg_relation_size(indexrelid) /
          NULLIF(pg_relation_size(relid), 0), 2) AS index_ratio
FROM pg_stat_user_indexes
WHERE schemaname = 'public'
  AND tablename = 'documents';
```

### Perform Maintenance

```sql
-- Run index maintenance
SELECT ruvector_index_maintenance('idx_documents_embedding_hnsw');

-- Vacuum after many deletes
VACUUM ANALYZE documents;

-- Rebuild index if heavily degraded
REINDEX INDEX idx_documents_embedding_hnsw;
```

## Step 10: Production Best Practices

### Partitioning for Large Datasets

```sql
-- Create partitioned table for time-series data
CREATE TABLE documents_partitioned (
    id BIGSERIAL,
    title TEXT NOT NULL,
    embedding real[],
    created_at TIMESTAMP NOT NULL
) PARTITION BY RANGE (created_at);

-- Create monthly partitions
CREATE TABLE documents_2024_01 PARTITION OF documents_partitioned
    FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');

CREATE TABLE documents_2024_02 PARTITION OF documents_partitioned
    FOR VALUES FROM ('2024-02-01') TO ('2024-03-01');

-- Create HNSW index on each partition
CREATE INDEX idx_documents_2024_01_embedding
ON documents_2024_01 USING hnsw (embedding hnsw_l2_ops);

CREATE INDEX idx_documents_2024_02_embedding
ON documents_2024_02 USING hnsw (embedding hnsw_l2_ops);
```

### Connection Pooling Setup

```python
# Python example with psycopg2
import psycopg2
from psycopg2 import pool
import numpy as np

# Create connection pool
db_pool = psycopg2.pool.ThreadedConnectionPool(
    minconn=1,
    maxconn=20,
    host="localhost",
    database="vector_search",
    user="postgres",
    password="password"
)

def search_similar(query_vector, k=10):
    """Search for k most similar documents"""
    conn = db_pool.getconn()
    try:
        with conn.cursor() as cur:
            # Set ef_search for this query
            cur.execute("SET LOCAL ruvector.ef_search = 100")

            # Execute similarity search
            cur.execute("""
                SELECT id, title, embedding <-> %s AS distance
                FROM documents
                ORDER BY embedding <-> %s
                LIMIT %s
            """, (query_vector.tolist(), query_vector.tolist(), k))

            return cur.fetchall()
    finally:
        db_pool.putconn(conn)

# Example usage
query = np.random.randn(384).astype(np.float32)
results = search_similar(query, k=10)
for doc_id, title, distance in results:
    print(f"{title}: {distance:.4f}")
```

### Monitoring Queries

```sql
-- Create view for monitoring slow vector queries
CREATE OR REPLACE VIEW slow_vector_queries AS
SELECT
    calls,
    total_exec_time,
    mean_exec_time,
    max_exec_time,
    query
FROM pg_stat_statements
WHERE query LIKE '%<->%'
   OR query LIKE '%<=>%'
   OR query LIKE '%<#>%'
ORDER BY mean_exec_time DESC;

-- Monitor slow queries
SELECT * FROM slow_vector_queries LIMIT 10;
```

## Step 11: Application Integration

### REST API Example (Node.js + Express)

```javascript
const express = require('express');
const { Pool } = require('pg');

const app = express();
const pool = new Pool({
    host: 'localhost',
    database: 'vector_search',
    user: 'postgres',
    password: 'password',
    max: 20
});

app.use(express.json());

// Search endpoint
app.post('/api/search', async (req, res) => {
    const { query_vector, k = 10, ef_search = 40 } = req.body;

    try {
        const client = await pool.connect();

        // Set ef_search for this session
        await client.query('SET LOCAL ruvector.ef_search = $1', [ef_search]);

        // Execute search
        const result = await client.query(`
            SELECT id, title, embedding <-> $1::real[] AS distance
            FROM documents
            ORDER BY embedding <-> $1::real[]
            LIMIT $2
        `, [query_vector, k]);

        client.release();

        res.json({
            results: result.rows,
            count: result.rowCount
        });
    } catch (err) {
        console.error(err);
        res.status(500).json({ error: 'Search failed' });
    }
});

app.listen(3000, () => {
    console.log('Vector search API running on port 3000');
});
```

## Complete Example: Semantic Document Search

```sql
-- 1. Create schema
CREATE TABLE articles (
    id SERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    author TEXT,
    content TEXT NOT NULL,
    embedding real[],  -- 768-dimensional BERT embeddings
    tags TEXT[],
    published_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- 2. Create indexes
CREATE INDEX idx_articles_embedding_hnsw
ON articles USING hnsw (embedding hnsw_cosine_ops)
WITH (m = 32, ef_construction = 128);

CREATE INDEX idx_articles_tags ON articles USING gin(tags);
CREATE INDEX idx_articles_published ON articles(published_at);

-- 3. Insert articles (with embeddings from your model)
INSERT INTO articles (title, author, content, embedding, tags, published_at)
VALUES
    ('Introduction to Vector Databases', 'Alice', 'Content...',
     array_agg(random())::real[], ARRAY['database', 'vectors'], '2024-01-15'),
    -- ... more articles
;

-- 4. Semantic search with filters
WITH query AS (
    SELECT array_agg(random())::real[] AS vec  -- Replace with actual embedding
    FROM generate_series(1, 768)
)
SELECT
    a.id,
    a.title,
    a.author,
    a.published_at,
    a.tags,
    a.embedding <=> query.vec AS similarity_score
FROM articles a, query
WHERE
    a.published_at >= CURRENT_DATE - INTERVAL '30 days'  -- Recent articles
    AND a.tags && ARRAY['database', 'search']  -- Tag filter
ORDER BY a.embedding <=> query.vec
LIMIT 20;

-- 5. Analyze performance
EXPLAIN (ANALYZE, BUFFERS, VERBOSE)
SELECT id, title, embedding <=> $1 AS score
FROM articles
WHERE published_at >= CURRENT_DATE - INTERVAL '30 days'
ORDER BY embedding <=> $1
LIMIT 20;
```

## Troubleshooting Common Issues

### Issue: Slow Index Build

```sql
-- Solution: Increase memory and adjust parameters
SET maintenance_work_mem = '4GB';
ALTER TABLE documents SET (autovacuum_enabled = false);

-- Rebuild with lower ef_construction
DROP INDEX idx_documents_embedding_hnsw;
CREATE INDEX idx_documents_embedding_hnsw
ON documents USING hnsw (embedding hnsw_l2_ops)
WITH (m = 16, ef_construction = 64);

-- Re-enable autovacuum
ALTER TABLE documents SET (autovacuum_enabled = true);
```

### Issue: Low Recall

```sql
-- Increase ef_search globally
ALTER SYSTEM SET ruvector.ef_search = 100;
SELECT pg_reload_conf();

-- Or rebuild index with better parameters
CREATE INDEX idx_documents_embedding_hnsw_v2
ON documents USING hnsw (embedding hnsw_l2_ops)
WITH (m = 32, ef_construction = 200);
```

### Issue: High Memory Usage

```sql
-- Monitor memory
SELECT ruvector_memory_stats();

-- Reduce index size with lower m
CREATE INDEX idx_documents_embedding_small
ON documents USING hnsw (embedding hnsw_l2_ops)
WITH (m = 8, ef_construction = 32);
```

## Conclusion

This example demonstrates the complete workflow for using HNSW indexes in production:

1. Extension installation and setup
2. Table creation with vector columns
3. HNSW index creation with tuning
4. Various query patterns (basic, filtered, hybrid)
5. Performance optimization
6. Maintenance and monitoring
7. Application integration

For more details, see:
- [HNSW Index Documentation](HNSW_INDEX.md)
- [Implementation Summary](HNSW_IMPLEMENTATION_SUMMARY.md)
