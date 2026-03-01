# GNN Usage Examples

## Table of Contents
- [Basic Examples](#basic-examples)
- [Real-World Applications](#real-world-applications)
- [Advanced Patterns](#advanced-patterns)
- [Performance Tuning](#performance-tuning)

## Basic Examples

### Example 1: Simple GCN Forward Pass

```sql
-- Create sample data
CREATE TABLE nodes (
    id INT PRIMARY KEY,
    features FLOAT[]
);

CREATE TABLE edges (
    source INT,
    target INT
);

INSERT INTO nodes VALUES
    (0, ARRAY[1.0, 2.0, 3.0]),
    (1, ARRAY[4.0, 5.0, 6.0]),
    (2, ARRAY[7.0, 8.0, 9.0]);

INSERT INTO edges VALUES
    (0, 1),
    (1, 2),
    (2, 0);

-- Apply GCN layer
SELECT ruvector_gcn_forward(
    (SELECT ARRAY_AGG(features ORDER BY id) FROM nodes),
    (SELECT ARRAY_AGG(source ORDER BY source, target) FROM edges),
    (SELECT ARRAY_AGG(target ORDER BY source, target) FROM edges),
    NULL,  -- No edge weights
    16     -- Output dimension
) AS gcn_output;
```

### Example 2: Message Aggregation

```sql
-- Aggregate neighbor features using different methods
WITH neighbor_messages AS (
    SELECT ARRAY[
        ARRAY[1.0, 2.0, 3.0],
        ARRAY[4.0, 5.0, 6.0],
        ARRAY[7.0, 8.0, 9.0]
    ]::FLOAT[][] as messages
)
SELECT
    ruvector_gnn_aggregate(messages, 'sum') as sum_agg,
    ruvector_gnn_aggregate(messages, 'mean') as mean_agg,
    ruvector_gnn_aggregate(messages, 'max') as max_agg
FROM neighbor_messages;

-- Results:
-- sum_agg:  [12.0, 15.0, 18.0]
-- mean_agg: [4.0, 5.0, 6.0]
-- max_agg:  [7.0, 8.0, 9.0]
```

### Example 3: GraphSAGE with Sampling

```sql
-- Apply GraphSAGE with neighbor sampling
SELECT ruvector_graphsage_forward(
    (SELECT ARRAY_AGG(features ORDER BY id) FROM nodes),
    (SELECT ARRAY_AGG(source ORDER BY source, target) FROM edges),
    (SELECT ARRAY_AGG(target ORDER BY source, target) FROM edges),
    32,  -- Output dimension
    5    -- Sample 5 neighbors per node
) AS sage_output;
```

## Real-World Applications

### Application 1: Citation Network Analysis

```sql
-- Schema for academic papers
CREATE TABLE papers (
    paper_id INT PRIMARY KEY,
    title TEXT,
    abstract_embedding FLOAT[],  -- 768-dim BERT embedding
    year INT,
    venue TEXT
);

CREATE TABLE citations (
    citing_paper INT REFERENCES papers(paper_id),
    cited_paper INT REFERENCES papers(paper_id),
    PRIMARY KEY (citing_paper, cited_paper)
);

-- Build 3-layer GCN for paper classification
WITH layer1 AS (
    SELECT ruvector_gcn_forward(
        (SELECT ARRAY_AGG(abstract_embedding ORDER BY paper_id) FROM papers),
        (SELECT ARRAY_AGG(citing_paper ORDER BY citing_paper, cited_paper) FROM citations),
        (SELECT ARRAY_AGG(cited_paper ORDER BY citing_paper, cited_paper) FROM citations),
        NULL,
        256  -- First hidden layer: 768 -> 256
    ) as h1
),
layer2 AS (
    SELECT ruvector_gcn_forward(
        (SELECT h1 FROM layer1),
        (SELECT ARRAY_AGG(citing_paper ORDER BY citing_paper, cited_paper) FROM citations),
        (SELECT ARRAY_AGG(cited_paper ORDER BY citing_paper, cited_paper) FROM citations),
        NULL,
        128  -- Second hidden layer: 256 -> 128
    ) as h2
),
layer3 AS (
    SELECT ruvector_gcn_forward(
        (SELECT h2 FROM layer2),
        (SELECT ARRAY_AGG(citing_paper ORDER BY citing_paper, cited_paper) FROM citations),
        (SELECT ARRAY_AGG(cited_paper ORDER BY citing_paper, cited_paper) FROM citations),
        NULL,
        10  -- Output layer: 128 -> 10 (for 10 research topics)
    ) as h3
)
SELECT
    p.paper_id,
    p.title,
    (SELECT h3 FROM layer3) as topic_scores
FROM papers p;
```

### Application 2: Social Network Influence Prediction

```sql
-- Schema for social network
CREATE TABLE users (
    user_id BIGINT PRIMARY KEY,
    profile_features FLOAT[],  -- Demographics, activity, etc.
    follower_count INT,
    verified BOOLEAN
);

CREATE TABLE follows (
    follower_id BIGINT REFERENCES users(user_id),
    followee_id BIGINT REFERENCES users(user_id),
    interaction_score FLOAT DEFAULT 1.0,  -- Weight based on interactions
    PRIMARY KEY (follower_id, followee_id)
);

-- Predict user influence using weighted GraphSAGE
WITH user_embeddings AS (
    SELECT ruvector_graphsage_forward(
        (SELECT ARRAY_AGG(profile_features ORDER BY user_id) FROM users),
        (SELECT ARRAY_AGG(follower_id ORDER BY follower_id, followee_id) FROM follows),
        (SELECT ARRAY_AGG(followee_id ORDER BY follower_id, followee_id) FROM follows),
        64,   -- Embedding dimension
        20    -- Sample top 20 connections
    ) as embeddings
),
influence_scores AS (
    SELECT
        u.user_id,
        u.follower_count,
        -- Use mean aggregation to get influence score
        ruvector_gnn_aggregate(
            ARRAY[ue.embeddings],
            'mean'
        ) as influence_embedding
    FROM users u
    CROSS JOIN user_embeddings ue
)
SELECT
    user_id,
    follower_count,
    -- Compute influence score from embedding
    (SELECT SUM(val) FROM UNNEST(influence_embedding) as val) as influence_score
FROM influence_scores
ORDER BY influence_score DESC
LIMIT 100;
```

### Application 3: Product Recommendation

```sql
-- Schema for e-commerce
CREATE TABLE products (
    product_id INT PRIMARY KEY,
    category TEXT,
    features FLOAT[],  -- Price, ratings, attributes
    in_stock BOOLEAN
);

CREATE TABLE product_relations (
    product_a INT REFERENCES products(product_id),
    product_b INT REFERENCES products(product_id),
    relation_type TEXT,  -- 'bought_together', 'similar', 'complementary'
    strength FLOAT DEFAULT 1.0
);

-- Generate product embeddings with GCN
WITH product_graph AS (
    SELECT
        product_id,
        features,
        (SELECT ARRAY_AGG(product_a ORDER BY product_a, product_b)
         FROM product_relations) as sources,
        (SELECT ARRAY_AGG(product_b ORDER BY product_a, product_b)
         FROM product_relations) as targets,
        (SELECT ARRAY_AGG(strength ORDER BY product_a, product_b)
         FROM product_relations) as weights
    FROM products
),
product_embeddings AS (
    SELECT ruvector_gcn_forward(
        (SELECT ARRAY_AGG(features ORDER BY product_id) FROM products),
        (SELECT sources[1] FROM product_graph LIMIT 1),
        (SELECT targets[1] FROM product_graph LIMIT 1),
        (SELECT weights[1] FROM product_graph LIMIT 1),
        128  -- Embedding dimension
    ) as embeddings
)
-- Use embeddings for recommendation
SELECT
    p.product_id,
    p.category,
    pe.embeddings as product_embedding
FROM products p
CROSS JOIN product_embeddings pe
WHERE p.in_stock = true;
```

## Advanced Patterns

### Pattern 1: Multi-Graph Batch Processing

```sql
-- Process multiple user sessions as separate graphs
CREATE TABLE user_sessions (
    session_id INT,
    node_id INT,
    node_features FLOAT[],
    PRIMARY KEY (session_id, node_id)
);

CREATE TABLE session_interactions (
    session_id INT,
    from_node INT,
    to_node INT,
    FOREIGN KEY (session_id, from_node) REFERENCES user_sessions(session_id, node_id),
    FOREIGN KEY (session_id, to_node) REFERENCES user_sessions(session_id, node_id)
);

-- Batch process all sessions
WITH session_graphs AS (
    SELECT
        session_id,
        COUNT(*) as num_nodes
    FROM user_sessions
    GROUP BY session_id
),
flattened_data AS (
    SELECT
        ARRAY_AGG(us.node_features ORDER BY us.session_id, us.node_id) as all_embeddings,
        ARRAY_AGG(si.from_node ORDER BY si.session_id, si.from_node, si.to_node) as all_sources,
        ARRAY_AGG(si.to_node ORDER BY si.session_id, si.from_node, si.to_node) as all_targets,
        ARRAY_AGG(sg.num_nodes ORDER BY sg.session_id) as graph_sizes
    FROM user_sessions us
    JOIN session_interactions si USING (session_id)
    JOIN session_graphs sg USING (session_id)
)
SELECT ruvector_gnn_batch_forward(
    (SELECT all_embeddings FROM flattened_data),
    (SELECT all_sources || all_targets FROM flattened_data),  -- Flattened edges
    (SELECT graph_sizes FROM flattened_data),
    'sage',  -- Use GraphSAGE
    64       -- Output dimension
) as batch_results;
```

### Pattern 2: Heterogeneous Graph Networks

```sql
-- Different node types in knowledge graph
CREATE TABLE entities (
    entity_id INT PRIMARY KEY,
    entity_type TEXT,  -- 'person', 'organization', 'location'
    features FLOAT[]
);

CREATE TABLE relations (
    subject_id INT REFERENCES entities(entity_id),
    predicate TEXT,  -- 'works_at', 'located_in', 'collaborates_with'
    object_id INT REFERENCES entities(entity_id),
    confidence FLOAT DEFAULT 1.0
);

-- Type-specific GCN layers
WITH person_subgraph AS (
    SELECT
        e.entity_id,
        e.features,
        ARRAY_AGG(r.subject_id ORDER BY r.subject_id, r.object_id) as sources,
        ARRAY_AGG(r.object_id ORDER BY r.subject_id, r.object_id) as targets,
        ARRAY_AGG(r.confidence ORDER BY r.subject_id, r.object_id) as weights
    FROM entities e
    JOIN relations r ON e.entity_id = r.subject_id OR e.entity_id = r.object_id
    WHERE e.entity_type = 'person'
    GROUP BY e.entity_id, e.features
),
org_subgraph AS (
    SELECT
        e.entity_id,
        e.features,
        ARRAY_AGG(r.subject_id ORDER BY r.subject_id, r.object_id) as sources,
        ARRAY_AGG(r.object_id ORDER BY r.subject_id, r.object_id) as targets,
        ARRAY_AGG(r.confidence ORDER BY r.subject_id, r.object_id) as weights
    FROM entities e
    JOIN relations r ON e.entity_id = r.subject_id OR e.entity_id = r.object_id
    WHERE e.entity_type = 'organization'
    GROUP BY e.entity_id, e.features
),
person_embeddings AS (
    SELECT ruvector_gcn_forward(
        (SELECT ARRAY_AGG(features ORDER BY entity_id) FROM person_subgraph),
        (SELECT sources[1] FROM person_subgraph LIMIT 1),
        (SELECT targets[1] FROM person_subgraph LIMIT 1),
        (SELECT weights[1] FROM person_subgraph LIMIT 1),
        128
    ) as embeddings
),
org_embeddings AS (
    SELECT ruvector_gcn_forward(
        (SELECT ARRAY_AGG(features ORDER BY entity_id) FROM org_subgraph),
        (SELECT sources[1] FROM org_subgraph LIMIT 1),
        (SELECT targets[1] FROM org_subgraph LIMIT 1),
        (SELECT weights[1] FROM org_subgraph LIMIT 1),
        128
    ) as embeddings
)
-- Combine embeddings
SELECT * FROM person_embeddings
UNION ALL
SELECT * FROM org_embeddings;
```

### Pattern 3: Temporal Graph Learning

```sql
-- Time-evolving graphs
CREATE TABLE temporal_nodes (
    node_id INT,
    timestamp TIMESTAMP,
    features FLOAT[],
    PRIMARY KEY (node_id, timestamp)
);

CREATE TABLE temporal_edges (
    source_id INT,
    target_id INT,
    timestamp TIMESTAMP,
    edge_features FLOAT[]
);

-- Learn embeddings for different time windows
WITH time_windows AS (
    SELECT
        DATE_TRUNC('hour', timestamp) as time_window,
        node_id,
        features
    FROM temporal_nodes
),
hourly_graphs AS (
    SELECT
        time_window,
        ruvector_gcn_forward(
            ARRAY_AGG(features ORDER BY node_id),
            (SELECT ARRAY_AGG(source_id ORDER BY source_id, target_id)
             FROM temporal_edges te
             WHERE DATE_TRUNC('hour', te.timestamp) = tw.time_window),
            (SELECT ARRAY_AGG(target_id ORDER BY source_id, target_id)
             FROM temporal_edges te
             WHERE DATE_TRUNC('hour', te.timestamp) = tw.time_window),
            NULL,
            64
        ) as embeddings
    FROM time_windows tw
    GROUP BY time_window
)
SELECT
    time_window,
    embeddings
FROM hourly_graphs
ORDER BY time_window;
```

## Performance Tuning

### Optimization 1: Materialized Views for Large Graphs

```sql
-- Precompute GNN layers for faster queries
CREATE MATERIALIZED VIEW gcn_layer1 AS
SELECT ruvector_gcn_forward(
    (SELECT ARRAY_AGG(features ORDER BY node_id) FROM nodes),
    (SELECT ARRAY_AGG(source ORDER BY source, target) FROM edges),
    (SELECT ARRAY_AGG(target ORDER BY source, target) FROM edges),
    NULL,
    256
) as layer1_output;

CREATE INDEX idx_gcn_layer1 ON gcn_layer1 USING gin(layer1_output);

-- Refresh periodically
REFRESH MATERIALIZED VIEW CONCURRENTLY gcn_layer1;
```

### Optimization 2: Partitioned Graphs

```sql
-- Partition large graphs by community
CREATE TABLE graph_partitions (
    partition_id INT,
    node_id INT,
    features FLOAT[],
    PRIMARY KEY (partition_id, node_id)
) PARTITION BY LIST (partition_id);

CREATE TABLE graph_partitions_p1 PARTITION OF graph_partitions
    FOR VALUES IN (1);
CREATE TABLE graph_partitions_p2 PARTITION OF graph_partitions
    FOR VALUES IN (2);

-- Process partitions in parallel
WITH partition_results AS (
    SELECT
        partition_id,
        ruvector_gcn_forward(
            ARRAY_AGG(features ORDER BY node_id),
            -- Edges within partition only
            (SELECT ARRAY_AGG(source) FROM edges e
             WHERE e.source IN (SELECT node_id FROM graph_partitions gp2
                               WHERE gp2.partition_id = gp.partition_id)),
            (SELECT ARRAY_AGG(target) FROM edges e
             WHERE e.target IN (SELECT node_id FROM graph_partitions gp2
                               WHERE gp2.partition_id = gp.partition_id)),
            NULL,
            128
        ) as partition_embedding
    FROM graph_partitions gp
    GROUP BY partition_id
)
SELECT * FROM partition_results;
```

### Optimization 3: Sampling Strategies

```sql
-- Use GraphSAGE with adaptive sampling
CREATE FUNCTION adaptive_graphsage(
    node_table TEXT,
    edge_table TEXT,
    max_neighbors INT DEFAULT 10
)
RETURNS TABLE (node_id INT, embedding FLOAT[]) AS $$
BEGIN
    -- Automatically adjust sampling based on degree distribution
    RETURN QUERY EXECUTE format('
        WITH node_degrees AS (
            SELECT
                n.id as node_id,
                COUNT(e.*) as degree
            FROM %I n
            LEFT JOIN %I e ON n.id = e.source OR n.id = e.target
            GROUP BY n.id
        ),
        adaptive_samples AS (
            SELECT
                node_id,
                LEAST(degree, %s) as sample_size
            FROM node_degrees
        )
        SELECT
            a.node_id,
            ruvector_graphsage_forward(
                (SELECT ARRAY_AGG(features ORDER BY id) FROM %I),
                (SELECT ARRAY_AGG(source) FROM %I),
                (SELECT ARRAY_AGG(target) FROM %I),
                64,
                a.sample_size
            )[a.node_id + 1] as embedding
        FROM adaptive_samples a
    ', node_table, edge_table, max_neighbors, node_table, edge_table, edge_table);
END;
$$ LANGUAGE plpgsql;
```

---

## Additional Resources

- [GNN Implementation Summary](./GNN_IMPLEMENTATION_SUMMARY.md)
- [GNN Quick Reference](./GNN_QUICK_REFERENCE.md)
- PostgreSQL Documentation: https://www.postgresql.org/docs/
- Graph Neural Networks: https://distill.pub/2021/gnn-intro/
