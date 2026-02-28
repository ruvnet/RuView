# Attention Mechanisms Usage Guide

## Overview

The ruvector-postgres extension implements 10 attention mechanisms optimized for PostgreSQL vector operations. This guide covers installation, usage, and examples.

## Available Attention Types

| Type | Complexity | Best For |
|------|-----------|----------|
| `scaled_dot` | O(n²) | Small sequences (<512) |
| `multi_head` | O(n²) | General purpose, parallel processing |
| `flash_v2` | O(n²) memory-efficient | GPU acceleration, large sequences |
| `linear` | O(n) | Very long sequences (>4K) |
| `gat` | O(E) | Graph-structured data |
| `sparse` | O(n√n) | Ultra-long sequences (>16K) |
| `moe` | O(n*k) | Conditional computation, routing |
| `cross` | O(n*m) | Query-document matching |
| `sliding` | O(n*w) | Local context, streaming |
| `poincare` | O(n²) | Hierarchical data structures |

## Installation

```sql
-- Load the extension
CREATE EXTENSION ruvector_postgres;

-- Verify installation
SELECT ruvector_version();
```

## Basic Usage

### 1. Single Attention Score

Compute attention score between two vectors:

```sql
SELECT ruvector_attention_score(
    ARRAY[1.0, 0.0, 0.0, 0.0]::float4[],  -- query
    ARRAY[1.0, 0.0, 0.0, 0.0]::float4[],  -- key
    'scaled_dot'                          -- attention type
) AS score;
```

### 2. Softmax Operation

Apply softmax to an array of scores:

```sql
SELECT ruvector_softmax(
    ARRAY[1.0, 2.0, 3.0, 4.0]::float4[]
) AS probabilities;

-- Result: {0.032, 0.087, 0.236, 0.645}
```

### 3. Multi-Head Attention

Compute multi-head attention across multiple keys:

```sql
SELECT ruvector_multi_head_attention(
    ARRAY[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]::float4[],  -- query (8-dim)
    ARRAY[
        ARRAY[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],        -- key 1
        ARRAY[0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]         -- key 2
    ]::float4[][],                                              -- keys
    ARRAY[
        ARRAY[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],        -- value 1
        ARRAY[8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]         -- value 2
    ]::float4[][],                                              -- values
    4                                                          -- num_heads
) AS output;
```

### 4. Flash Attention

Memory-efficient attention for large sequences:

```sql
SELECT ruvector_flash_attention(
    query_vector,
    key_vectors,
    value_vectors,
    64  -- block_size
) AS result
FROM documents;
```

### 5. Attention Scores for Multiple Keys

Get attention distribution across all keys:

```sql
SELECT ruvector_attention_scores(
    ARRAY[1.0, 0.0, 0.0]::float4[],  -- query
    ARRAY[
        ARRAY[1.0, 0.0, 0.0],        -- key 1: high similarity
        ARRAY[0.0, 1.0, 0.0],        -- key 2: orthogonal
        ARRAY[0.5, 0.5, 0.0]         -- key 3: partial match
    ]::float4[][]                     -- all keys
) AS attention_weights;

-- Result: {0.576, 0.212, 0.212} (probabilities sum to 1.0)
```

## Practical Examples

### Example 1: Document Reranking with Attention

```sql
-- Create documents table
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    title TEXT,
    embedding vector(768)
);

-- Insert sample documents
INSERT INTO documents (title, embedding)
VALUES
    ('Deep Learning', array_fill(random()::float4, ARRAY[768])),
    ('Machine Learning', array_fill(random()::float4, ARRAY[768])),
    ('Neural Networks', array_fill(random()::float4, ARRAY[768]));

-- Query with attention-based reranking
WITH query AS (
    SELECT array_fill(0.5::float4, ARRAY[768]) AS qvec
),
initial_results AS (
    SELECT
        id,
        title,
        embedding,
        embedding <-> (SELECT qvec FROM query) AS distance
    FROM documents
    ORDER BY distance
    LIMIT 20
)
SELECT
    id,
    title,
    ruvector_attention_score(
        (SELECT qvec FROM query),
        embedding,
        'scaled_dot'
    ) AS attention_score,
    distance
FROM initial_results
ORDER BY attention_score DESC
LIMIT 10;
```

### Example 2: Multi-Head Attention for Semantic Search

```sql
-- Find documents using multi-head attention
CREATE OR REPLACE FUNCTION semantic_search_with_attention(
    query_embedding float4[],
    num_results int DEFAULT 10,
    num_heads int DEFAULT 8
)
RETURNS TABLE (
    id int,
    title text,
    attention_score float4
) AS $$
BEGIN
    RETURN QUERY
    WITH candidates AS (
        SELECT d.id, d.title, d.embedding
        FROM documents d
        ORDER BY d.embedding <-> query_embedding
        LIMIT num_results * 2
    ),
    attention_scores AS (
        SELECT
            c.id,
            c.title,
            ruvector_attention_score(
                query_embedding,
                c.embedding,
                'multi_head'
            ) AS score
        FROM candidates c
    )
    SELECT a.id, a.title, a.score
    FROM attention_scores a
    ORDER BY a.score DESC
    LIMIT num_results;
END;
$$ LANGUAGE plpgsql;

-- Use the function
SELECT * FROM semantic_search_with_attention(
    ARRAY[0.1, 0.2, ...]::float4[]
);
```

### Example 3: Cross-Attention for Query-Document Matching

```sql
-- Create queries and documents tables
CREATE TABLE queries (
    id SERIAL PRIMARY KEY,
    text TEXT,
    embedding vector(384)
);

CREATE TABLE knowledge_base (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding vector(384)
);

-- Find best matching document for each query
SELECT
    q.id AS query_id,
    q.text AS query_text,
    kb.id AS doc_id,
    kb.content AS doc_content,
    ruvector_attention_score(
        q.embedding,
        kb.embedding,
        'cross'
    ) AS relevance_score
FROM queries q
CROSS JOIN LATERAL (
    SELECT id, content, embedding
    FROM knowledge_base
    ORDER BY embedding <-> q.embedding
    LIMIT 5
) kb
ORDER BY q.id, relevance_score DESC;
```

### Example 4: Flash Attention for Long Documents

```sql
-- Process long documents with memory-efficient Flash Attention
CREATE TABLE long_documents (
    id SERIAL PRIMARY KEY,
    chunks vector(512)[],  -- Array of chunk embeddings
    metadata JSONB
);

-- Query with Flash Attention (handles long sequences efficiently)
WITH query AS (
    SELECT array_fill(0.5::float4, ARRAY[512]) AS qvec
)
SELECT
    ld.id,
    ld.metadata->>'title' AS title,
    ruvector_flash_attention(
        (SELECT qvec FROM query),
        ld.chunks,
        ld.chunks,  -- Use same chunks as values
        128  -- block_size for tiled processing
    ) AS attention_output
FROM long_documents ld
LIMIT 10;
```

### Example 5: List All Attention Types

```sql
-- View all available attention mechanisms
SELECT * FROM ruvector_attention_types();

-- Result:
-- | name        | complexity              | best_for                        |
-- |-------------|-------------------------|---------------------------------|
-- | scaled_dot  | O(n²)                  | Small sequences (<512)          |
-- | multi_head  | O(n²)                  | General purpose, parallel       |
-- | flash_v2    | O(n²) memory-efficient | GPU acceleration, large seqs    |
-- | linear      | O(n)                   | Very long sequences (>4K)       |
-- | ...         | ...                    | ...                             |
```

## Performance Tips

### 1. Choose the Right Attention Type

- **Small sequences (<512 tokens)**: Use `scaled_dot`
- **Medium sequences (512-4K)**: Use `multi_head` or `flash_v2`
- **Long sequences (>4K)**: Use `linear` or `sparse`
- **Graph data**: Use `gat`

### 2. Optimize Block Size for Flash Attention

```sql
-- Small GPU memory: use smaller blocks
SELECT ruvector_flash_attention(q, k, v, 32);

-- Large GPU memory: use larger blocks
SELECT ruvector_flash_attention(q, k, v, 128);
```

### 3. Use Multi-Head Attention for Better Parallelization

```sql
-- More heads = better parallelization (but more computation)
SELECT ruvector_multi_head_attention(query, keys, values, 8);  -- 8 heads
SELECT ruvector_multi_head_attention(query, keys, values, 16); -- 16 heads
```

### 4. Batch Processing

```sql
-- Process multiple queries efficiently
WITH queries AS (
    SELECT id, embedding AS qvec FROM user_queries
),
documents AS (
    SELECT id, embedding AS dvec FROM document_store
)
SELECT
    q.id AS query_id,
    d.id AS doc_id,
    ruvector_attention_score(q.qvec, d.dvec, 'scaled_dot') AS score
FROM queries q
CROSS JOIN documents d
ORDER BY q.id, score DESC;
```

## Advanced Features

### Custom Attention Pipelines

Combine multiple attention mechanisms:

```sql
WITH first_stage AS (
    -- Use fast scaled_dot for initial filtering
    SELECT id, embedding,
           ruvector_attention_score(query, embedding, 'scaled_dot') AS score
    FROM documents
    ORDER BY score DESC
    LIMIT 100
),
second_stage AS (
    -- Use multi-head for refined ranking
    SELECT id,
           ruvector_multi_head_attention(query,
                                        ARRAY_AGG(embedding),
                                        ARRAY_AGG(embedding),
                                        8) AS refined_score
    FROM first_stage
)
SELECT * FROM second_stage ORDER BY refined_score DESC LIMIT 10;
```

## Benchmarks

Performance characteristics on a sample dataset:

| Operation | Sequence Length | Time (ms) | Memory (MB) |
|-----------|----------------|-----------|-------------|
| scaled_dot | 128 | 0.5 | 1.2 |
| scaled_dot | 512 | 2.1 | 4.8 |
| multi_head (8 heads) | 512 | 1.8 | 5.2 |
| flash_v2 (block=64) | 512 | 1.6 | 2.1 |
| flash_v2 (block=64) | 2048 | 6.8 | 3.4 |

## Troubleshooting

### Common Issues

1. **Dimension Mismatch Error**
   ```sql
   ERROR: Query and key dimensions must match: 768 vs 384
   ```
   **Solution**: Ensure all vectors have the same dimensionality.

2. **Multi-Head Division Error**
   ```sql
   ERROR: Query dimension 768 must be divisible by num_heads 5
   ```
   **Solution**: Use num_heads that divides evenly into your embedding dimension.

3. **Memory Issues with Large Sequences**
   **Solution**: Use Flash Attention (`flash_v2`) or Linear Attention (`linear`) for sequences >1K.

## See Also

- [PostgreSQL Vector Operations](./vector-operations.md)
- [Performance Tuning Guide](./performance-tuning.md)
- [SIMD Optimization](./simd-optimization.md)
