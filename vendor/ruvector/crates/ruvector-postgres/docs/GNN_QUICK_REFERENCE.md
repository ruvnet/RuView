# GNN Quick Reference Guide

## SQL Functions

### 1. GCN Forward Pass

```sql
ruvector_gcn_forward(
    embeddings FLOAT[][],  -- Node embeddings [num_nodes x in_dim]
    src INT[],             -- Source node indices
    dst INT[],             -- Destination node indices
    weights FLOAT[],       -- Edge weights (optional)
    out_dim INT            -- Output dimension
) RETURNS FLOAT[][]        -- Updated embeddings [num_nodes x out_dim]
```

**Example**:
```sql
SELECT ruvector_gcn_forward(
    ARRAY[ARRAY[1.0, 2.0], ARRAY[3.0, 4.0]],
    ARRAY[0],
    ARRAY[1],
    NULL,
    8
);
```

### 2. GNN Aggregation

```sql
ruvector_gnn_aggregate(
    messages FLOAT[][],    -- Neighbor messages
    method TEXT            -- 'sum', 'mean', or 'max'
) RETURNS FLOAT[]          -- Aggregated message
```

**Example**:
```sql
SELECT ruvector_gnn_aggregate(
    ARRAY[ARRAY[1.0, 2.0], ARRAY[3.0, 4.0]],
    'mean'
);
-- Returns: [2.0, 3.0]
```

### 3. GraphSAGE Forward Pass

```sql
ruvector_graphsage_forward(
    embeddings FLOAT[][],  -- Node embeddings
    src INT[],             -- Source node indices
    dst INT[],             -- Destination node indices
    out_dim INT,           -- Output dimension
    num_samples INT        -- Neighbors to sample per node
) RETURNS FLOAT[][]        -- Updated embeddings
```

**Example**:
```sql
SELECT ruvector_graphsage_forward(
    node_embeddings,
    edge_src,
    edge_dst,
    64,
    10
)
FROM my_graph;
```

### 4. Multi-Hop Message Passing

```sql
ruvector_message_pass(
    node_table TEXT,       -- Table with node features
    edge_table TEXT,       -- Table with edges
    embedding_col TEXT,    -- Column name for embeddings
    hops INT,              -- Number of hops
    layer_type TEXT        -- 'gcn' or 'sage'
) RETURNS TEXT             -- Description of operation
```

**Example**:
```sql
SELECT ruvector_message_pass(
    'nodes',
    'edges',
    'embedding',
    3,
    'gcn'
);
```

### 5. Batch GNN Processing

```sql
ruvector_gnn_batch_forward(
    embeddings_batch FLOAT[][],   -- Batch of embeddings
    edge_indices_batch INT[],     -- Flattened edge indices
    graph_sizes INT[],            -- Nodes per graph
    layer_type TEXT,              -- 'gcn' or 'sage'
    out_dim INT                   -- Output dimension
) RETURNS FLOAT[][]               -- Batch of results
```

## Common Patterns

### Pattern 1: Node Classification

```sql
-- Create node embeddings table
CREATE TABLE node_embeddings (
    node_id INT PRIMARY KEY,
    embedding FLOAT[]
);

-- Create edge table
CREATE TABLE edges (
    src INT,
    dst INT,
    weight FLOAT DEFAULT 1.0
);

-- Apply GCN
WITH gcn_output AS (
    SELECT ruvector_gcn_forward(
        ARRAY_AGG(embedding ORDER BY node_id),
        ARRAY_AGG(src ORDER BY edge_id),
        ARRAY_AGG(dst ORDER BY edge_id),
        ARRAY_AGG(weight ORDER BY edge_id),
        128
    ) as updated_embeddings
    FROM node_embeddings
    CROSS JOIN edges
)
SELECT * FROM gcn_output;
```

### Pattern 2: Link Prediction

```sql
-- Compute edge embeddings using node embeddings
WITH node_features AS (
    SELECT ruvector_graphsage_forward(
        embeddings,
        sources,
        targets,
        64,
        10
    ) as new_embeddings
    FROM graph_data
),
edge_features AS (
    SELECT
        e.src,
        e.dst,
        nf.new_embeddings[e.src] || nf.new_embeddings[e.dst] as edge_embedding
    FROM edges e
    CROSS JOIN node_features nf
)
SELECT * FROM edge_features;
```

### Pattern 3: Graph Classification

```sql
-- Aggregate node embeddings to graph embedding
WITH node_embeddings AS (
    SELECT
        graph_id,
        ruvector_gcn_forward(
            ARRAY_AGG(features),
            ARRAY_AGG(src),
            ARRAY_AGG(dst),
            NULL,
            128
        ) as embeddings
    FROM graphs
    GROUP BY graph_id
),
graph_embeddings AS (
    SELECT
        graph_id,
        ruvector_gnn_aggregate(embeddings, 'mean') as graph_embedding
    FROM node_embeddings
)
SELECT * FROM graph_embeddings;
```

## Aggregation Methods

| Method | Formula | Use Case |
|--------|---------|----------|
| `sum` | Σ messages | Counting, accumulation |
| `mean` | (Σ messages) / n | Averaging features |
| `max` | max(messages) | Feature selection |

## Layer Types

### GCN (Graph Convolutional Network)

**When to use**:
- Transductive learning (fixed graph)
- Homophilic graphs (similar nodes connected)
- Need interpretable aggregation

**Characteristics**:
- Degree normalization
- All neighbors considered
- Memory efficient

### GraphSAGE

**When to use**:
- Inductive learning (new nodes)
- Large graphs (need sampling)
- Heterogeneous graphs

**Characteristics**:
- Neighbor sampling
- Separate self/neighbor weights
- L2 normalization

## Performance Tips

1. **Use Sampling for Large Graphs**:
   ```sql
   -- Instead of all neighbors
   SELECT ruvector_graphsage_forward(..., 10);  -- Sample 10 neighbors
   ```

2. **Batch Processing**:
   ```sql
   -- Process multiple graphs at once
   SELECT ruvector_gnn_batch_forward(...);
   ```

3. **Index Edges**:
   ```sql
   CREATE INDEX idx_edges_src ON edges(src);
   CREATE INDEX idx_edges_dst ON edges(dst);
   ```

4. **Materialize Intermediate Results**:
   ```sql
   CREATE MATERIALIZED VIEW layer1_output AS
   SELECT ruvector_gcn_forward(...);
   ```

## Typical Dimensions

| Layer | Input Dim | Output Dim | Hidden Dim |
|-------|-----------|------------|------------|
| Layer 1 | Raw features (varies) | 128-256 | - |
| Layer 2 | 128-256 | 64-128 | - |
| Layer 3 | 64-128 | 32-64 | - |
| Output | 32-64 | # classes | - |

## Error Handling

```sql
-- Check for empty inputs
SELECT CASE
    WHEN ARRAY_LENGTH(embeddings, 1) = 0
    THEN NULL
    ELSE ruvector_gcn_forward(embeddings, src, dst, NULL, 64)
END;

-- Handle disconnected nodes
-- (automatically handled - returns original features)
```

## Integration with PostgreSQL

### Create Extension
```sql
CREATE EXTENSION ruvector;
```

### Check Version
```sql
SELECT ruvector_version();
```

### View Available Functions
```sql
\df ruvector_*
```

## Complete Example

```sql
-- 1. Create tables
CREATE TABLE papers (
    paper_id INT PRIMARY KEY,
    features FLOAT[],
    label INT
);

CREATE TABLE citations (
    citing INT,
    cited INT,
    FOREIGN KEY (citing) REFERENCES papers(paper_id),
    FOREIGN KEY (cited) REFERENCES papers(paper_id)
);

-- 2. Load data
INSERT INTO papers VALUES
    (1, ARRAY[0.1, 0.2, 0.3], 0),
    (2, ARRAY[0.4, 0.5, 0.6], 1),
    (3, ARRAY[0.7, 0.8, 0.9], 0);

INSERT INTO citations VALUES
    (1, 2),
    (2, 3),
    (3, 1);

-- 3. Apply 2-layer GCN
WITH layer1 AS (
    SELECT ruvector_gcn_forward(
        ARRAY_AGG(features ORDER BY paper_id),
        ARRAY_AGG(citing ORDER BY citing, cited),
        ARRAY_AGG(cited ORDER BY citing, cited),
        NULL,
        128
    ) as h1
    FROM papers
    CROSS JOIN citations
),
layer2 AS (
    SELECT ruvector_gcn_forward(
        h1,
        ARRAY_AGG(citing ORDER BY citing, cited),
        ARRAY_AGG(cited ORDER BY citing, cited),
        NULL,
        64
    ) as h2
    FROM layer1
    CROSS JOIN citations
)
SELECT * FROM layer2;
```

## Troubleshooting

### Issue: Dimension Mismatch
```sql
-- Check input dimensions
SELECT ARRAY_LENGTH(features, 1) FROM papers LIMIT 1;
```

### Issue: Out of Memory
```sql
-- Use GraphSAGE with sampling
SELECT ruvector_graphsage_forward(..., 10);  -- Limit neighbors
```

### Issue: Slow Performance
```sql
-- Create indexes
CREATE INDEX ON edges(src, dst);

-- Use parallel queries
SET max_parallel_workers_per_gather = 4;
```

---

**Quick Start**: Copy the "Complete Example" above to get started immediately!
