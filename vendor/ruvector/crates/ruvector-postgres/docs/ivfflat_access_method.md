# IVFFlat Index Access Method

## Overview

The IVFFlat (Inverted File with Flat quantization) index is a PostgreSQL access method implementation for approximate nearest neighbor (ANN) search. It partitions the vector space into clusters using k-means clustering, enabling fast similarity search by probing only the most relevant clusters.

## Architecture

### Storage Layout

The IVFFlat index uses PostgreSQL's page-based storage with the following structure:

```
┌─────────────────┬──────────────────────┬─────────────────────┐
│  Page 0         │  Pages 1-N           │  Pages N+1-M        │
│  (Metadata)     │  (Centroids)         │  (Inverted Lists)   │
└─────────────────┴──────────────────────┴─────────────────────┘
```

#### Page 0: Metadata Page
```rust
struct IvfFlatMetaPage {
    magic: u32,              // 0x49564646 ("IVFF")
    lists: u32,              // Number of clusters
    probes: u32,             // Default probes for search
    dimensions: u32,         // Vector dimensions
    trained: u32,            // 0=untrained, 1=trained
    vector_count: u64,       // Total vectors indexed
    metric: u32,             // Distance metric (0=L2, 1=IP, 2=Cosine, 3=L1)
    centroid_start_page: u32,// First centroid page
    lists_start_page: u32,   // First inverted list page
    reserved: [u32; 16],     // Future expansion
}
```

#### Pages 1-N: Centroid Pages
Each centroid entry contains:
- Cluster ID
- Inverted list page reference
- Vector count in cluster
- Centroid vector data (dimensions × 4 bytes)

#### Pages N+1-M: Inverted List Pages
Each vector entry contains:
- Heap tuple ID (block number + offset)
- Vector data (dimensions × 4 bytes)

## Index Building

### 1. Training Phase

The index must be trained before use:

```sql
-- Create index with training
CREATE INDEX ON items USING ruivfflat (embedding vector_l2_ops)
  WITH (lists = 100);
```

Training process:
1. **Sample Collection**: Up to 50,000 random vectors sampled from the heap
2. **K-means++ Initialization**: Intelligent centroid seeding for better convergence
3. **K-means Clustering**: 10 iterations of Lloyd's algorithm
4. **Centroid Storage**: Trained centroids written to index pages

### 2. Vector Assignment

After training, all vectors are assigned to their nearest centroid:
- Calculate distance to each centroid
- Assign to nearest centroid's inverted list
- Store in inverted list pages

## Search Process

### Query Execution

```sql
SELECT * FROM items
ORDER BY embedding <-> '[1,2,3,...]'
LIMIT 10;
```

Search algorithm:
1. **Find Nearest Centroids**: Calculate distance from query to all centroids
2. **Probe Selection**: Select `probes` nearest centroids
3. **List Scanning**: Scan inverted lists for selected centroids
4. **Re-ranking**: Calculate exact distances to all candidates
5. **Top-K Selection**: Return k nearest vectors

### Performance Tuning

#### Lists Parameter

Controls the number of clusters:
- **Small values (10-50)**: Faster build, slower search, lower recall
- **Medium values (100-200)**: Balanced performance
- **Large values (500-1000)**: Slower build, faster search, higher recall

Rule of thumb: `lists = sqrt(total_vectors)`

#### Probes Parameter

Controls search accuracy vs speed:
- **Low probes (1-3)**: Fast search, lower recall
- **Medium probes (5-10)**: Balanced
- **High probes (20-50)**: Slower search, higher recall

Set dynamically:
```sql
SET ruvector.ivfflat_probes = 10;
```

## Configuration

### GUC Variables

```sql
-- Set default probes for IVFFlat searches
SET ruvector.ivfflat_probes = 10;

-- View current setting
SHOW ruvector.ivfflat_probes;
```

### Index Options

```sql
CREATE INDEX ON table USING ruivfflat (column opclass)
  WITH (lists = value, probes = value);
```

Available options:
- `lists`: Number of clusters (default: 100)
- `probes`: Default probes for searches (default: 1)

## Operator Classes

### Vector L2 (Euclidean)
```sql
CREATE INDEX ON items USING ruivfflat (embedding vector_l2_ops)
  WITH (lists = 100);
```

### Vector Inner Product
```sql
CREATE INDEX ON items USING ruivfflat (embedding vector_ip_ops)
  WITH (lists = 100);
```

### Vector Cosine
```sql
CREATE INDEX ON items USING ruivfflat (embedding vector_cosine_ops)
  WITH (lists = 100);
```

## Performance Characteristics

### Time Complexity
- **Build**: O(n × k × d × iterations) where n=vectors, k=lists, d=dimensions
- **Insert**: O(k × d) - find nearest centroid
- **Search**: O(probes × (n/k) × d) - probe lists and re-rank

### Space Complexity
- **Index Size**: O(n × d × 4 + k × d × 4)
- Approximately same size as raw vectors plus centroids

### Recall vs Speed Trade-offs

| Probes | Recall | Speed    | Use Case                    |
|--------|--------|----------|-----------------------------|
| 1      | 60-70% | Fastest  | Very fast approximate search|
| 5      | 80-85% | Fast     | Balanced performance        |
| 10     | 90-95% | Medium   | High recall applications    |
| 20+    | 95-99% | Slower   | Near-exact search           |

## Examples

### Basic Usage

```sql
-- Create table
CREATE TABLE documents (
    id serial PRIMARY KEY,
    content text,
    embedding vector(1536)
);

-- Insert vectors
INSERT INTO documents (content, embedding)
VALUES
    ('First document', '[0.1, 0.2, ...]'),
    ('Second document', '[0.3, 0.4, ...]');

-- Create IVFFlat index
CREATE INDEX ON documents USING ruivfflat (embedding vector_l2_ops)
  WITH (lists = 100);

-- Search
SELECT id, content, embedding <-> '[0.5, 0.6, ...]' AS distance
FROM documents
ORDER BY embedding <-> '[0.5, 0.6, ...]'
LIMIT 10;
```

### Advanced Configuration

```sql
-- Large dataset with many lists
CREATE INDEX ON large_table USING ruivfflat (embedding vector_cosine_ops)
  WITH (lists = 1000);

-- High-recall search
SET ruvector.ivfflat_probes = 20;
SELECT * FROM large_table
ORDER BY embedding <=> '[...]'
LIMIT 100;
```

### Index Statistics

```sql
-- Get index information
SELECT * FROM ruvector_ivfflat_stats('documents_embedding_idx');

-- Returns:
-- lists | probes | dimensions | trained | vector_count | metric
--------+--------+------------+---------+--------------+-----------
-- 100   | 1      | 1536       | true    | 1000000     | euclidean
```

## Comparison with HNSW

| Feature          | IVFFlat           | HNSW                |
|------------------|-------------------|---------------------|
| Build Time       | Fast (minutes)    | Slow (hours)        |
| Search Speed     | Fast              | Faster              |
| Recall           | 80-95%            | 95-99%              |
| Memory           | Low               | High                |
| Incremental Insert| Fast             | Medium              |
| Best For         | Large static datasets | High-recall queries |

## Maintenance

### Rebuilding Index

After significant data changes, rebuild for better clustering:

```sql
REINDEX INDEX documents_embedding_idx;
```

### Monitoring

```sql
-- Check index size
SELECT pg_size_pretty(pg_relation_size('documents_embedding_idx'));

-- Check if trained
SELECT * FROM ruvector_ivfflat_stats('documents_embedding_idx');
```

## Implementation Details

### Zero-Copy Vector Access

The implementation uses zero-copy techniques:
- Read vector data directly from heap tuples
- No intermediate buffer allocation
- Compare directly with centroids in-place

### Memory Management

- Uses PostgreSQL's palloc/pfree memory contexts
- Automatic cleanup on transaction end
- No manual memory management required

### Concurrency

- Safe for concurrent reads
- Index building is single-threaded
- Inserts are serialized per cluster

## Limitations

1. **Training Required**: Cannot insert before training completes
2. **Fixed Clusters**: Number of lists cannot change after build
3. **No Updates**: Update requires delete + insert
4. **Memory**: All centroids must fit in memory during search

## Future Enhancements

- [ ] Parallel index building
- [ ] Incremental training for inserts
- [ ] Product quantization (IVF-PQ)
- [ ] GPU acceleration
- [ ] Adaptive probe selection
- [ ] Cluster rebalancing

## References

1. [pgvector](https://github.com/pgvector/pgvector) - Original IVFFlat implementation
2. [FAISS](https://github.com/facebookresearch/faiss) - Facebook AI Similarity Search
3. "Product Quantization for Nearest Neighbor Search" - Jégou et al., 2011
4. PostgreSQL Index Access Method Documentation
