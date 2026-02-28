# RuVector-Postgres API Reference

## Overview

Complete API reference for RuVector-Postgres extension, including SQL functions, operators, types, and GUC variables.

## Table of Contents

- [Data Types](#data-types)
- [SQL Functions](#sql-functions)
- [Operators](#operators)
- [Index Methods](#index-methods)
- [GUC Variables](#guc-variables)
- [Operator Classes](#operator-classes)
- [Usage Examples](#usage-examples)

## Data Types

### `ruvector(n)`

Primary vector type for dense floating-point vectors.

**Syntax:**

```sql
ruvector(dimensions)
```

**Parameters:**

- `dimensions`: Integer, 1 to 16,000

**Storage:**

- Header: 8 bytes
- Data: 4 bytes per dimension (f32)
- Total: 8 + (4 × dimensions) bytes

**Example:**

```sql
CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    embedding ruvector(1536)  -- OpenAI ada-002 dimensions
);

INSERT INTO items (embedding) VALUES ('[1.0, 2.0, 3.0]');
INSERT INTO items (embedding) VALUES (ARRAY[1.0, 2.0, 3.0]::ruvector);
```

### `halfvec(n)`

Half-precision (16-bit float) vector type.

**Syntax:**

```sql
halfvec(dimensions)
```

**Parameters:**

- `dimensions`: Integer, 1 to 16,000

**Storage:**

- Header: 8 bytes
- Data: 2 bytes per dimension (f16)
- Total: 8 + (2 × dimensions) bytes

**Benefits:**

- 50% memory reduction vs `ruvector`
- <0.01% accuracy loss for most embeddings
- SIMD f16 support on modern CPUs

**Example:**

```sql
CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    embedding halfvec(1536)  -- 3,080 bytes vs 6,152 for ruvector
);

-- Automatic conversion from ruvector
INSERT INTO items (embedding)
SELECT embedding::halfvec FROM ruvector_table;
```

### `sparsevec(n)`

Sparse vector type for high-dimensional sparse data.

**Syntax:**

```sql
sparsevec(dimensions)
```

**Parameters:**

- `dimensions`: Integer, 1 to 1,000,000

**Storage:**

- Header: 12 bytes
- Data: 8 bytes per non-zero element (u32 index + f32 value)
- Total: 12 + (8 × nnz) bytes

**Use Cases:**

- BM25 text embeddings
- TF-IDF vectors
- High-dimensional sparse features

**Example:**

```sql
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    sparse_embedding sparsevec(50000)  -- Only stores non-zero values
);

-- Sparse vector with 3 non-zero values
INSERT INTO documents (sparse_embedding)
VALUES ('{1:0.5, 100:0.8, 5000:0.3}/50000');
```

## SQL Functions

### Information Functions

#### `ruvector_version()`

Returns the extension version.

**Syntax:**

```sql
ruvector_version() → text
```

**Example:**

```sql
SELECT ruvector_version();
-- Output: '0.1.19'
```

#### `ruvector_simd_info()`

Returns detected SIMD capabilities.

**Syntax:**

```sql
ruvector_simd_info() → text
```

**Returns:**

- `'AVX512'`: AVX-512 support detected
- `'AVX2'`: AVX2 support detected
- `'NEON'`: ARM NEON support detected
- `'Scalar'`: No SIMD support

**Example:**

```sql
SELECT ruvector_simd_info();
-- Output: 'AVX2'
```

### Distance Functions

#### `ruvector_l2_distance(a, b)`

Compute L2 (Euclidean) distance.

**Syntax:**

```sql
ruvector_l2_distance(a ruvector, b ruvector) → float4
```

**Formula:**

```
L2(a, b) = sqrt(Σ(a[i] - b[i])²)
```

**Properties:**

- SIMD optimized
- Parallel safe
- Immutable

**Example:**

```sql
SELECT ruvector_l2_distance(
    '[1.0, 2.0, 3.0]'::ruvector,
    '[4.0, 5.0, 6.0]'::ruvector
);
-- Output: 5.196...
```

#### `ruvector_cosine_distance(a, b)`

Compute cosine distance.

**Syntax:**

```sql
ruvector_cosine_distance(a ruvector, b ruvector) → float4
```

**Formula:**

```
Cosine(a, b) = 1 - (a·b) / (||a|| ||b||)
```

**Range:** [0, 2]

- 0: Vectors point in same direction
- 1: Vectors are orthogonal
- 2: Vectors point in opposite directions

**Example:**

```sql
SELECT ruvector_cosine_distance(
    '[1.0, 0.0]'::ruvector,
    '[0.0, 1.0]'::ruvector
);
-- Output: 1.0 (orthogonal)
```

#### `ruvector_ip_distance(a, b)`

Compute inner product (negative dot product) distance.

**Syntax:**

```sql
ruvector_ip_distance(a ruvector, b ruvector) → float4
```

**Formula:**

```
IP(a, b) = -Σ(a[i] * b[i])
```

**Note:** Negative to work with `ORDER BY ASC`.

**Example:**

```sql
SELECT ruvector_ip_distance(
    '[1.0, 2.0, 3.0]'::ruvector,
    '[4.0, 5.0, 6.0]'::ruvector
);
-- Output: -32.0 (negative of 1*4 + 2*5 + 3*6)
```

#### `ruvector_l1_distance(a, b)`

Compute L1 (Manhattan) distance.

**Syntax:**

```sql
ruvector_l1_distance(a ruvector, b ruvector) → float4
```

**Formula:**

```
L1(a, b) = Σ|a[i] - b[i]|
```

**Example:**

```sql
SELECT ruvector_l1_distance(
    '[1.0, 2.0, 3.0]'::ruvector,
    '[4.0, 5.0, 6.0]'::ruvector
);
-- Output: 9.0
```

### Utility Functions

#### `ruvector_norm(v)`

Compute L2 norm (magnitude) of a vector.

**Syntax:**

```sql
ruvector_norm(v ruvector) → float4
```

**Formula:**

```
||v|| = sqrt(Σv[i]²)
```

**Example:**

```sql
SELECT ruvector_norm('[3.0, 4.0]'::ruvector);
-- Output: 5.0
```

#### `ruvector_normalize(v)`

Normalize vector to unit length.

**Syntax:**

```sql
ruvector_normalize(v ruvector) → ruvector
```

**Formula:**

```
normalize(v) = v / ||v||
```

**Example:**

```sql
SELECT ruvector_normalize('[3.0, 4.0]'::ruvector);
-- Output: [0.6, 0.8]
```

### Index Maintenance Functions

#### `ruvector_index_stats(index_name)`

Get statistics for a vector index.

**Syntax:**

```sql
ruvector_index_stats(index_name text) → TABLE(
    index_name text,
    index_size_mb numeric,
    vector_count bigint,
    dimensions int,
    build_time_seconds numeric,
    fragmentation_pct numeric
)
```

**Example:**

```sql
SELECT * FROM ruvector_index_stats('items_embedding_idx');

-- Output:
-- index_name          | items_embedding_idx
-- index_size_mb       | 512
-- vector_count        | 1000000
-- dimensions          | 1536
-- build_time_seconds  | 45.2
-- fragmentation_pct   | 2.3
```

#### `ruvector_index_maintenance(index_name)`

Perform maintenance on a vector index.

**Syntax:**

```sql
ruvector_index_maintenance(index_name text) → void
```

**Operations:**

- Removes deleted nodes
- Rebuilds fragmented layers
- Updates statistics

**Example:**

```sql
SELECT ruvector_index_maintenance('items_embedding_idx');
```

## Operators

### Distance Operators

| Operator | Name | Distance Metric | Order |
|----------|------|----------------|-------|
| `<->` | L2 | Euclidean | ASC |
| `<#>` | IP | Inner Product (negative) | ASC |
| `<=>` | Cosine | Cosine Distance | ASC |
| `<+>` | L1 | Manhattan | ASC |

**Properties:**

- All operators are IMMUTABLE
- All operators are PARALLEL SAFE
- All operators support index scans

### L2 Distance Operator (`<->`)

**Syntax:**

```sql
vector1 <-> vector2
```

**Example:**

```sql
SELECT * FROM items
ORDER BY embedding <-> '[1.0, 2.0, 3.0]'::ruvector
LIMIT 10;
```

### Cosine Distance Operator (`<=>`)

**Syntax:**

```sql
vector1 <=> vector2
```

**Example:**

```sql
SELECT * FROM items
ORDER BY embedding <=> '[1.0, 2.0, 3.0]'::ruvector
LIMIT 10;
```

### Inner Product Operator (`<#>`)

**Syntax:**

```sql
vector1 <#> vector2
```

**Note:** Returns negative dot product for ascending order.

**Example:**

```sql
SELECT * FROM items
ORDER BY embedding <#> '[1.0, 2.0, 3.0]'::ruvector
LIMIT 10;
```

### Manhattan Distance Operator (`<+>`)

**Syntax:**

```sql
vector1 <+> vector2
```

**Example:**

```sql
SELECT * FROM items
ORDER BY embedding <+> '[1.0, 2.0, 3.0]'::ruvector
LIMIT 10;
```

## Index Methods

### HNSW Index (`ruhnsw`)

Hierarchical Navigable Small World graph index.

**Syntax:**

```sql
CREATE INDEX index_name ON table_name
USING ruhnsw (column operator_class)
WITH (options);
```

**Options:**

| Option | Type | Default | Range | Description |
|--------|------|---------|-------|-------------|
| `m` | integer | 16 | 2-100 | Max connections per layer |
| `ef_construction` | integer | 64 | 4-1000 | Build-time search breadth |
| `quantization` | text | NULL | sq8, pq16, binary | Quantization method |

**Operator Classes:**

- `ruvector_l2_ops`: For `<->` operator
- `ruvector_ip_ops`: For `<#>` operator
- `ruvector_cosine_ops`: For `<=>` operator

**Example:**

```sql
-- Basic HNSW index
CREATE INDEX items_embedding_idx ON items
USING ruhnsw (embedding ruvector_l2_ops);

-- High recall HNSW index
CREATE INDEX items_embedding_idx ON items
USING ruhnsw (embedding ruvector_l2_ops)
WITH (m = 32, ef_construction = 200);

-- HNSW with quantization
CREATE INDEX items_embedding_idx ON items
USING ruhnsw (embedding ruvector_l2_ops)
WITH (m = 16, ef_construction = 100, quantization = 'sq8');
```

**Performance:**

- Search: O(log n)
- Insert: O(log n)
- Memory: ~1.5x vector data size
- Recall: 95-99%+ with tuned parameters

### IVFFlat Index (`ruivfflat`)

Inverted file with flat (uncompressed) vectors.

**Syntax:**

```sql
CREATE INDEX index_name ON table_name
USING ruivfflat (column operator_class)
WITH (lists = n);
```

**Options:**

| Option | Type | Default | Range | Description |
|--------|------|---------|-------|-------------|
| `lists` | integer | sqrt(rows) | 1-100000 | Number of clusters |

**Operator Classes:**

- `ruvector_l2_ops`: For `<->` operator
- `ruvector_ip_ops`: For `<#>` operator
- `ruvector_cosine_ops`: For `<=>` operator

**Example:**

```sql
-- Basic IVFFlat index
CREATE INDEX items_embedding_idx ON items
USING ruivfflat (embedding ruvector_l2_ops)
WITH (lists = 100);

-- IVFFlat for large dataset
CREATE INDEX items_embedding_idx ON items
USING ruivfflat (embedding ruvector_l2_ops)
WITH (lists = 1000);
```

**Performance:**

- Search: O(√n)
- Insert: O(1) after training
- Memory: Minimal overhead
- Recall: 90-95% with appropriate probes

**Training:**

IVFFlat requires training to find cluster centroids:

```sql
-- Index is automatically trained during creation
-- Training uses k-means on a sample of vectors
```

## GUC Variables

### `ruvector.ef_search`

Controls HNSW search quality (higher = better recall, slower).

**Syntax:**

```sql
SET ruvector.ef_search = value;
```

**Default:** 40

**Range:** 1-1000

**Scope:** Session, transaction, or global

**Example:**

```sql
-- Session-level
SET ruvector.ef_search = 200;

-- Transaction-level
BEGIN;
SET LOCAL ruvector.ef_search = 100;
SELECT ... ORDER BY embedding <-> query;
COMMIT;

-- Global
ALTER SYSTEM SET ruvector.ef_search = 100;
SELECT pg_reload_conf();
```

### `ruvector.probes`

Controls IVFFlat search quality (higher = better recall, slower).

**Syntax:**

```sql
SET ruvector.probes = value;
```

**Default:** 1

**Range:** 1-10000

**Recommended:** sqrt(lists) for 90%+ recall

**Example:**

```sql
-- For lists = 100, use probes = 10
SET ruvector.probes = 10;
```

## Operator Classes

### `ruvector_l2_ops`

For L2 (Euclidean) distance queries.

**Usage:**

```sql
CREATE INDEX ... USING ruhnsw (embedding ruvector_l2_ops);
SELECT ... ORDER BY embedding <-> query;
```

### `ruvector_ip_ops`

For inner product distance queries.

**Usage:**

```sql
CREATE INDEX ... USING ruhnsw (embedding ruvector_ip_ops);
SELECT ... ORDER BY embedding <#> query;
```

### `ruvector_cosine_ops`

For cosine distance queries.

**Usage:**

```sql
CREATE INDEX ... USING ruhnsw (embedding ruvector_cosine_ops);
SELECT ... ORDER BY embedding <=> query;
```

## Usage Examples

### Basic Vector Search

```sql
-- Create table
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding ruvector(1536)
);

-- Insert vectors
INSERT INTO documents (content, embedding) VALUES
    ('Document 1', '[0.1, 0.2, ...]'::ruvector),
    ('Document 2', '[0.3, 0.4, ...]'::ruvector);

-- Create index
CREATE INDEX documents_embedding_idx ON documents
USING ruhnsw (embedding ruvector_l2_ops);

-- Search
SELECT content, embedding <-> '[0.5, 0.6, ...]'::ruvector AS distance
FROM documents
ORDER BY distance
LIMIT 10;
```

### Filtered Vector Search

```sql
-- Search with WHERE clause
SELECT content, embedding <-> query AS distance
FROM documents
WHERE category = 'technology'
ORDER BY distance
LIMIT 10;
```

### Batch Distance Calculation

```sql
-- Compute distances to multiple vectors
WITH queries AS (
    SELECT id, embedding AS query FROM queries_table
)
SELECT
    q.id AS query_id,
    d.id AS doc_id,
    d.embedding <-> q.query AS distance
FROM documents d
CROSS JOIN queries q
ORDER BY q.id, distance
LIMIT 100;
```

### Vector Arithmetic

```sql
-- Add vectors
SELECT (embedding1 + embedding2) AS sum FROM ...;

-- Subtract vectors
SELECT (embedding1 - embedding2) AS diff FROM ...;

-- Scalar multiplication
SELECT (embedding * 2.0) AS scaled FROM ...;
```

### Hybrid Search (Vector + Text)

```sql
-- Combine vector similarity with text search
SELECT
    content,
    embedding <-> query_vector AS vector_score,
    ts_rank(to_tsvector(content), to_tsquery('search terms')) AS text_score,
    (0.7 * (1 / (1 + embedding <-> query_vector)) +
     0.3 * ts_rank(to_tsvector(content), to_tsquery('search terms'))) AS combined_score
FROM documents
WHERE to_tsvector(content) @@ to_tsquery('search terms')
ORDER BY combined_score DESC
LIMIT 10;
```

### Index Parameter Tuning

```sql
-- Test different ef_search values
DO $$
DECLARE
    ef_val INTEGER;
BEGIN
    FOR ef_val IN 10, 20, 40, 80, 160 LOOP
        EXECUTE format('SET LOCAL ruvector.ef_search = %s', ef_val);
        RAISE NOTICE 'ef_search = %', ef_val;

        PERFORM * FROM items
        ORDER BY embedding <-> '[...]'::ruvector
        LIMIT 10;
    END LOOP;
END $$;
```

## Performance Tips

1. **Choose the right index:**
   - HNSW: Best for high recall, fast queries
   - IVFFlat: Best for memory-constrained environments

2. **Tune index parameters:**
   - Higher `m` and `ef_construction`: Better recall, larger index
   - Higher `ef_search`: Better recall, slower queries

3. **Use appropriate vector type:**
   - `ruvector`: Full precision
   - `halfvec`: 50% memory savings, minimal accuracy loss
   - `sparsevec`: Massive savings for sparse data

4. **Enable parallelism:**
   ```sql
   SET max_parallel_workers_per_gather = 4;
   ```

5. **Use quantization for large datasets:**
   ```sql
   WITH (quantization = 'sq8')  -- 4x memory reduction
   ```

## See Also

- [ARCHITECTURE.md](./ARCHITECTURE.md) - System architecture
- [SIMD_OPTIMIZATION.md](./SIMD_OPTIMIZATION.md) - Performance details
- [MIGRATION.md](./MIGRATION.md) - Migrating from pgvector
