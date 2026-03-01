# IVFFlat Index Usage Examples

## Basic Setup

### 1. Create Table with Vector Column

```sql
CREATE TABLE products (
    id serial PRIMARY KEY,
    name text NOT NULL,
    description text,
    embedding vector(1536),  -- OpenAI ada-002 embeddings
    created_at timestamp DEFAULT now()
);
```

### 2. Insert Sample Data

```sql
-- Insert products with embeddings
INSERT INTO products (name, description, embedding) VALUES
    ('Laptop', 'High-performance laptop', '[0.1, 0.2, 0.3, ...]'),
    ('Mouse', 'Wireless mouse', '[0.4, 0.5, 0.6, ...]'),
    ('Keyboard', 'Mechanical keyboard', '[0.7, 0.8, 0.9, ...]');

-- Or insert from a data source
INSERT INTO products (name, description, embedding)
SELECT
    name,
    description,
    get_embedding(description)  -- Your embedding function
FROM source_table;
```

## Index Creation

### Default Configuration

```sql
-- Create index with default settings (100 lists, probe 1)
CREATE INDEX products_embedding_idx
ON products
USING ruivfflat (embedding vector_l2_ops);
```

### Optimized for Small Datasets (< 10K vectors)

```sql
CREATE INDEX products_embedding_idx
ON products
USING ruivfflat (embedding vector_l2_ops)
WITH (lists = 50);
```

### Optimized for Medium Datasets (10K - 100K vectors)

```sql
CREATE INDEX products_embedding_idx
ON products
USING ruivfflat (embedding vector_l2_ops)
WITH (lists = 100);
```

### Optimized for Large Datasets (> 100K vectors)

```sql
CREATE INDEX products_embedding_idx
ON products
USING ruivfflat (embedding vector_l2_ops)
WITH (lists = 500);
```

### Very Large Datasets (> 1M vectors)

```sql
CREATE INDEX products_embedding_idx
ON products
USING ruivfflat (embedding vector_l2_ops)
WITH (lists = 1000);
```

## Distance Metrics

### Euclidean Distance (L2)

```sql
-- Best for: General-purpose similarity search
CREATE INDEX products_embedding_l2_idx
ON products
USING ruivfflat (embedding vector_l2_ops)
WITH (lists = 100);

-- Query
SELECT name, embedding <-> '[0.1, 0.2, ...]' AS distance
FROM products
ORDER BY embedding <-> '[0.1, 0.2, ...]'
LIMIT 10;
```

### Cosine Distance

```sql
-- Best for: Normalized vectors, text embeddings
CREATE INDEX products_embedding_cosine_idx
ON products
USING ruivfflat (embedding vector_cosine_ops)
WITH (lists = 100);

-- Query
SELECT name, embedding <=> '[0.1, 0.2, ...]' AS distance
FROM products
ORDER BY embedding <=> '[0.1, 0.2, ...]'
LIMIT 10;
```

### Inner Product

```sql
-- Best for: Maximum similarity (negative distance)
CREATE INDEX products_embedding_ip_idx
ON products
USING ruivfflat (embedding vector_ip_ops)
WITH (lists = 100);

-- Query
SELECT name, embedding <#> '[0.1, 0.2, ...]' AS distance
FROM products
ORDER BY embedding <#> '[0.1, 0.2, ...]'
LIMIT 10;
```

## Search Queries

### Basic KNN Search

```sql
-- Find 10 most similar products
SELECT
    id,
    name,
    description,
    embedding <-> '[0.1, 0.2, ...]'::vector AS distance
FROM products
ORDER BY embedding <-> '[0.1, 0.2, ...]'::vector
LIMIT 10;
```

### Search with Filters

```sql
-- Find similar products in a category
SELECT
    id,
    name,
    embedding <-> '[0.1, 0.2, ...]'::vector AS distance
FROM products
WHERE category = 'Electronics'
ORDER BY embedding <-> '[0.1, 0.2, ...]'::vector
LIMIT 10;
```

### Search with Multiple Conditions

```sql
-- Find recent similar products
SELECT
    id,
    name,
    created_at,
    embedding <=> '[0.1, 0.2, ...]'::vector AS distance
FROM products
WHERE
    created_at > now() - interval '30 days'
    AND price < 1000
ORDER BY embedding <=> '[0.1, 0.2, ...]'::vector
LIMIT 10;
```

## Performance Tuning

### Adjusting Probes

```sql
-- Fast search (lower recall ~70%)
SET ruvector.ivfflat_probes = 1;

-- Balanced search (medium recall ~85%)
SET ruvector.ivfflat_probes = 5;

-- Accurate search (high recall ~95%)
SET ruvector.ivfflat_probes = 10;

-- Very accurate search (very high recall ~98%)
SET ruvector.ivfflat_probes = 20;
```

### Session-Level Configuration

```sql
-- Set for current session
SET ruvector.ivfflat_probes = 10;

-- Verify setting
SHOW ruvector.ivfflat_probes;

-- Reset to default
RESET ruvector.ivfflat_probes;
```

### Transaction-Level Configuration

```sql
BEGIN;
SET LOCAL ruvector.ivfflat_probes = 15;
-- Query will use probes = 15
SELECT * FROM products ORDER BY embedding <-> '[...]' LIMIT 10;
COMMIT;
-- Back to session default
```

### Query-Level Configuration

```sql
SELECT
    id,
    name,
    embedding <-> '[0.1, 0.2, ...]'::vector AS distance
FROM products
ORDER BY embedding <-> '[0.1, 0.2, ...]'::vector
LIMIT 10
SETTINGS (ruvector.ivfflat_probes = 10);
```

## Advanced Use Cases

### Semantic Search with Ranking

```sql
WITH similar_products AS (
    SELECT
        id,
        name,
        description,
        embedding <-> query_embedding AS vector_distance,
        ts_rank(to_tsvector('english', description),
                to_tsquery('laptop')) AS text_rank
    FROM products,
         (SELECT '[0.1, 0.2, ...]'::vector AS query_embedding) q
    ORDER BY embedding <-> query_embedding
    LIMIT 100
)
SELECT
    id,
    name,
    description,
    vector_distance,
    text_rank,
    (0.7 * (1 - vector_distance) + 0.3 * text_rank) AS combined_score
FROM similar_products
ORDER BY combined_score DESC
LIMIT 10;
```

### Multi-Vector Search

```sql
-- Find products similar to multiple queries
WITH queries AS (
    SELECT unnest(ARRAY[
        '[0.1, 0.2, ...]'::vector,
        '[0.4, 0.5, ...]'::vector,
        '[0.7, 0.8, ...]'::vector
    ]) AS query_vec
),
all_results AS (
    SELECT DISTINCT
        p.id,
        p.name,
        MIN(p.embedding <-> q.query_vec) AS min_distance
    FROM products p
    CROSS JOIN queries q
    GROUP BY p.id, p.name
)
SELECT id, name, min_distance
FROM all_results
ORDER BY min_distance
LIMIT 10;
```

### Batch Processing

```sql
-- Process embeddings in batches
DO $$
DECLARE
    batch_size INT := 1000;
    offset_val INT := 0;
    total_count INT;
BEGIN
    SELECT COUNT(*) INTO total_count FROM unprocessed_products;

    WHILE offset_val < total_count LOOP
        -- Process batch
        WITH batch AS (
            SELECT id, description
            FROM unprocessed_products
            ORDER BY id
            LIMIT batch_size
            OFFSET offset_val
        )
        UPDATE products p
        SET embedding = get_embedding(b.description)
        FROM batch b
        WHERE p.id = b.id;

        offset_val := offset_val + batch_size;
        RAISE NOTICE 'Processed % of % vectors', offset_val, total_count;
    END LOOP;
END $$;
```

## Monitoring and Maintenance

### Check Index Statistics

```sql
-- Get index metadata
SELECT * FROM ruvector_ivfflat_stats('products_embedding_idx');

-- Check index size
SELECT
    schemaname,
    tablename,
    indexname,
    pg_size_pretty(pg_relation_size(indexrelid)) AS index_size,
    pg_size_pretty(pg_table_size(tablename::regclass)) AS table_size
FROM pg_indexes
JOIN pg_stat_user_indexes USING (schemaname, tablename, indexname)
WHERE indexname = 'products_embedding_idx';
```

### Analyze Query Performance

```sql
-- Enable timing
\timing on

-- Explain analyze
EXPLAIN (ANALYZE, BUFFERS)
SELECT id, name
FROM products
ORDER BY embedding <-> '[0.1, 0.2, ...]'::vector
LIMIT 10;
```

### Rebuild Index

```sql
-- After significant data changes
REINDEX INDEX products_embedding_idx;

-- Or rebuild concurrently (PostgreSQL 12+)
REINDEX INDEX CONCURRENTLY products_embedding_idx;
```

### Vacuum and Analyze

```sql
-- Update statistics
ANALYZE products;

-- Vacuum to reclaim space
VACUUM products;

-- Or full vacuum
VACUUM FULL products;
```

## Best Practices

### 1. Choose Appropriate Number of Lists

```sql
-- Rule of thumb: lists = sqrt(total_vectors)

-- Example for 100K vectors
CREATE INDEX ON products USING ruivfflat (embedding vector_l2_ops)
WITH (lists = 316);  -- sqrt(100000) ≈ 316

-- Example for 1M vectors
CREATE INDEX ON products USING ruivfflat (embedding vector_l2_ops)
WITH (lists = 1000);  -- sqrt(1000000) = 1000
```

### 2. Balance Speed vs Accuracy

```sql
-- Production: Start conservative, increase probes if needed
SET ruvector.ivfflat_probes = 5;

-- Development/Testing: Higher probes for better results
SET ruvector.ivfflat_probes = 10;

-- Critical queries: Maximum accuracy
SET ruvector.ivfflat_probes = 20;
```

### 3. Regular Maintenance

```sql
-- Weekly or after large data changes
VACUUM ANALYZE products;
REINDEX INDEX CONCURRENTLY products_embedding_idx;
```

### 4. Monitor Index Health

```sql
-- Create monitoring view
CREATE VIEW index_health AS
SELECT
    indexname,
    pg_size_pretty(pg_relation_size(indexrelid)) AS size,
    idx_scan AS scans,
    idx_tup_read AS tuples_read,
    idx_tup_fetch AS tuples_fetched,
    (idx_tup_read::float / NULLIF(idx_scan, 0))::numeric(10,2) AS avg_tuples_per_scan
FROM pg_stat_user_indexes
WHERE indexrelname LIKE '%embedding%';

-- Check regularly
SELECT * FROM index_health;
```

## Troubleshooting

### Slow Queries

```sql
-- Increase probes
SET ruvector.ivfflat_probes = 10;

-- Check if index is being used
EXPLAIN SELECT * FROM products ORDER BY embedding <-> '[...]' LIMIT 10;

-- Rebuild index
REINDEX INDEX products_embedding_idx;
```

### Low Recall

```sql
-- Increase probes
SET ruvector.ivfflat_probes = 15;

-- Or rebuild with more lists
DROP INDEX products_embedding_idx;
CREATE INDEX products_embedding_idx ON products
USING ruivfflat (embedding vector_l2_ops)
WITH (lists = 500);
```

### Memory Issues

```sql
-- Reduce lists during build
CREATE INDEX products_embedding_idx ON products
USING ruivfflat (embedding vector_l2_ops)
WITH (lists = 100);  -- Smaller lists = less memory

-- Or build in multiple steps
```
