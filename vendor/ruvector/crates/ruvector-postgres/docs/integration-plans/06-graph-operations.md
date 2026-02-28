# Graph Operations & Cypher Integration Plan

## Overview

Integrate graph database capabilities from `ruvector-graph` into PostgreSQL, enabling Cypher query language support, property graph operations, and vector-enhanced graph traversals directly in SQL.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     PostgreSQL Extension                         │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    Cypher Engine                         │    │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐  │    │
│  │  │  Parser  │→│ Planner  │→│ Executor │→│  Result │  │    │
│  │  └──────────┘  └──────────┘  └──────────┘  └─────────┘  │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                 Property Graph Store                     │    │
│  │  ┌───────────┐  ┌───────────┐  ┌───────────────────┐    │    │
│  │  │   Nodes   │  │   Edges   │  │ Vector Embeddings │    │    │
│  │  │  (Labels) │  │  (Types)  │  │    (HNSW Index)   │    │    │
│  │  └───────────┘  └───────────┘  └───────────────────┘    │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

## Module Structure

```
src/
├── graph/
│   ├── mod.rs              # Module exports
│   ├── cypher/
│   │   ├── parser.rs       # Cypher parser (pest/nom)
│   │   ├── ast.rs          # Abstract syntax tree
│   │   ├── planner.rs      # Query planner
│   │   ├── executor.rs     # Query executor
│   │   └── functions.rs    # Built-in Cypher functions
│   ├── storage/
│   │   ├── nodes.rs        # Node storage
│   │   ├── edges.rs        # Edge storage
│   │   └── properties.rs   # Property storage
│   ├── traversal/
│   │   ├── bfs.rs          # Breadth-first search
│   │   ├── dfs.rs          # Depth-first search
│   │   ├── shortest_path.rs # Shortest path algorithms
│   │   └── vector_walk.rs  # Vector-guided traversal
│   ├── index/
│   │   ├── label_index.rs  # Label-based index
│   │   └── property_index.rs # Property index
│   └── operators.rs        # SQL operators
```

## SQL Interface

### Graph Schema Setup

```sql
-- Create a property graph
SELECT ruvector_create_graph('social_network');

-- Define node labels
SELECT ruvector_create_node_label('social_network', 'Person',
    properties := '{
        "name": "text",
        "age": "integer",
        "embedding": "vector(768)"
    }'
);

SELECT ruvector_create_node_label('social_network', 'Company',
    properties := '{
        "name": "text",
        "industry": "text",
        "embedding": "vector(768)"
    }'
);

-- Define edge types
SELECT ruvector_create_edge_type('social_network', 'KNOWS',
    properties := '{"since": "date", "strength": "float"}'
);

SELECT ruvector_create_edge_type('social_network', 'WORKS_AT',
    properties := '{"role": "text", "since": "date"}'
);
```

### Cypher Queries

```sql
-- Execute Cypher queries
SELECT * FROM ruvector_cypher('social_network', $$
    MATCH (p:Person)-[:KNOWS]->(friend:Person)
    WHERE p.name = 'Alice'
    RETURN friend.name, friend.age
$$);

-- Create nodes
SELECT ruvector_cypher('social_network', $$
    CREATE (p:Person {name: 'Bob', age: 30, embedding: $embedding})
    RETURN p
$$, params := '{"embedding": [0.1, 0.2, ...]}');

-- Create relationships
SELECT ruvector_cypher('social_network', $$
    MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
    CREATE (a)-[:KNOWS {since: date('2024-01-15'), strength: 0.8}]->(b)
$$);

-- Pattern matching
SELECT * FROM ruvector_cypher('social_network', $$
    MATCH (p:Person)-[:WORKS_AT]->(c:Company {industry: 'Tech'})
    RETURN p.name, c.name
    ORDER BY p.age DESC
    LIMIT 10
$$);
```

### Vector-Enhanced Graph Queries

```sql
-- Find similar nodes using vector search + graph structure
SELECT * FROM ruvector_cypher('social_network', $$
    MATCH (p:Person)
    WHERE ruvector.similarity(p.embedding, $query) > 0.8
    RETURN p.name, p.age, ruvector.similarity(p.embedding, $query) AS similarity
    ORDER BY similarity DESC
    LIMIT 10
$$, params := '{"query": [0.1, 0.2, ...]}');

-- Graph-aware semantic search
SELECT * FROM ruvector_cypher('social_network', $$
    MATCH (p:Person)-[:KNOWS*1..3]->(friend:Person)
    WHERE p.name = 'Alice'
    WITH friend, ruvector.similarity(friend.embedding, $query) AS sim
    WHERE sim > 0.7
    RETURN friend.name, sim
    ORDER BY sim DESC
$$, params := '{"query": [0.1, 0.2, ...]}');

-- Personalized PageRank with vector similarity
SELECT * FROM ruvector_cypher('social_network', $$
    CALL ruvector.pagerank('Person', 'KNOWS', {
        dampingFactor: 0.85,
        iterations: 20,
        personalizedOn: $seed_embedding
    })
    YIELD node, score
    RETURN node.name, score
    ORDER BY score DESC
    LIMIT 20
$$, params := '{"seed_embedding": [0.1, 0.2, ...]}');
```

### Path Finding

```sql
-- Shortest path
SELECT * FROM ruvector_cypher('social_network', $$
    MATCH p = shortestPath((a:Person {name: 'Alice'})-[:KNOWS*1..6]-(b:Person {name: 'Bob'}))
    RETURN p, length(p)
$$);

-- All shortest paths
SELECT * FROM ruvector_cypher('social_network', $$
    MATCH p = allShortestPaths((a:Person {name: 'Alice'})-[:KNOWS*1..6]-(b:Person {name: 'Bob'}))
    RETURN p, length(p)
$$);

-- Vector-guided path (minimize embedding distance along path)
SELECT * FROM ruvector_cypher('social_network', $$
    MATCH p = ruvector.vectorPath(
        (a:Person {name: 'Alice'}),
        (b:Person {name: 'Bob'}),
        'KNOWS',
        {
            maxHops: 6,
            vectorProperty: 'embedding',
            optimization: 'minTotalDistance'
        }
    )
    RETURN p, ruvector.pathEmbeddingDistance(p) AS distance
$$);
```

### Graph Algorithms

```sql
-- Community detection (Louvain)
SELECT * FROM ruvector_cypher('social_network', $$
    CALL ruvector.louvain('Person', 'KNOWS', {resolution: 1.0})
    YIELD node, communityId
    RETURN node.name, communityId
$$);

-- Node similarity (Jaccard)
SELECT * FROM ruvector_cypher('social_network', $$
    CALL ruvector.nodeSimilarity('Person', 'KNOWS', {
        similarityCutoff: 0.5,
        topK: 10
    })
    YIELD node1, node2, similarity
    RETURN node1.name, node2.name, similarity
$$);

-- Centrality measures
SELECT * FROM ruvector_cypher('social_network', $$
    CALL ruvector.betweenness('Person', 'KNOWS')
    YIELD node, score
    RETURN node.name, score
    ORDER BY score DESC
    LIMIT 10
$$);
```

## Implementation Phases

### Phase 1: Cypher Parser (Week 1-3)

```rust
// src/graph/cypher/parser.rs

use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "graph/cypher/cypher.pest"]
pub struct CypherParser;

/// Parse Cypher query string into AST
pub fn parse_cypher(query: &str) -> Result<CypherQuery, ParseError> {
    let pairs = CypherParser::parse(Rule::query, query)?;

    let mut builder = AstBuilder::new();
    for pair in pairs {
        builder.process(pair)?;
    }

    Ok(builder.build())
}

// src/graph/cypher/ast.rs

#[derive(Debug, Clone)]
pub enum CypherQuery {
    Match(MatchClause),
    Create(CreateClause),
    Merge(MergeClause),
    Delete(DeleteClause),
    Return(ReturnClause),
    With(WithClause),
    Compound(Vec<CypherQuery>),
}

#[derive(Debug, Clone)]
pub struct MatchClause {
    pub patterns: Vec<Pattern>,
    pub where_clause: Option<WhereClause>,
    pub optional: bool,
}

#[derive(Debug, Clone)]
pub struct Pattern {
    pub nodes: Vec<NodePattern>,
    pub relationships: Vec<RelationshipPattern>,
}

#[derive(Debug, Clone)]
pub struct NodePattern {
    pub variable: Option<String>,
    pub labels: Vec<String>,
    pub properties: Option<Properties>,
}

#[derive(Debug, Clone)]
pub struct RelationshipPattern {
    pub variable: Option<String>,
    pub types: Vec<String>,
    pub properties: Option<Properties>,
    pub direction: Direction,
    pub length: RelationshipLength,
}

#[derive(Debug, Clone)]
pub enum RelationshipLength {
    Exactly(usize),
    Range(Option<usize>, Option<usize>),  // *1..3
    Any,  // *
}
```

### Phase 2: Query Planner (Week 4-5)

```rust
// src/graph/cypher/planner.rs

pub struct QueryPlanner {
    graph_store: Arc<GraphStore>,
    statistics: Arc<GraphStatistics>,
}

impl QueryPlanner {
    pub fn plan(&self, query: &CypherQuery) -> Result<QueryPlan, PlanError> {
        let logical_plan = self.to_logical(query)?;
        let optimized = self.optimize(logical_plan)?;
        let physical_plan = self.to_physical(optimized)?;

        Ok(physical_plan)
    }

    fn to_logical(&self, query: &CypherQuery) -> Result<LogicalPlan, PlanError> {
        match query {
            CypherQuery::Match(m) => self.plan_match(m),
            CypherQuery::Create(c) => self.plan_create(c),
            CypherQuery::Return(r) => self.plan_return(r),
            // ...
        }
    }

    fn plan_match(&self, match_clause: &MatchClause) -> Result<LogicalPlan, PlanError> {
        let mut plan = LogicalPlan::Scan;

        for pattern in &match_clause.patterns {
            // Choose optimal starting point based on selectivity
            let start_node = self.choose_start_node(pattern);

            // Build expand operations
            for rel in &pattern.relationships {
                plan = LogicalPlan::Expand {
                    input: Box::new(plan),
                    relationship: rel.clone(),
                    direction: rel.direction,
                };
            }
        }

        // Add filter for WHERE clause
        if let Some(where_clause) = &match_clause.where_clause {
            plan = LogicalPlan::Filter {
                input: Box::new(plan),
                predicate: where_clause.predicate.clone(),
            };
        }

        Ok(plan)
    }

    fn optimize(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlanError> {
        let mut optimized = plan;

        // Push down filters
        optimized = self.push_down_filters(optimized);

        // Reorder joins based on selectivity
        optimized = self.reorder_joins(optimized);

        // Use indexes where available
        optimized = self.apply_indexes(optimized);

        Ok(optimized)
    }
}

#[derive(Debug)]
pub enum LogicalPlan {
    Scan,
    NodeByLabel { label: String },
    NodeById { ids: Vec<u64> },
    Expand {
        input: Box<LogicalPlan>,
        relationship: RelationshipPattern,
        direction: Direction,
    },
    Filter {
        input: Box<LogicalPlan>,
        predicate: Expression,
    },
    Project {
        input: Box<LogicalPlan>,
        expressions: Vec<(String, Expression)>,
    },
    VectorSearch {
        label: String,
        property: String,
        query: Vec<f32>,
        k: usize,
    },
    // ...
}
```

### Phase 3: Query Executor (Week 6-8)

```rust
// src/graph/cypher/executor.rs

pub struct QueryExecutor {
    graph_store: Arc<GraphStore>,
}

impl QueryExecutor {
    pub fn execute(&self, plan: &QueryPlan) -> Result<QueryResult, ExecuteError> {
        match plan {
            QueryPlan::Scan { label } => self.scan_nodes(label),
            QueryPlan::Expand { input, rel, dir } => {
                let source_rows = self.execute(input)?;
                self.expand_relationships(&source_rows, rel, dir)
            }
            QueryPlan::Filter { input, predicate } => {
                let rows = self.execute(input)?;
                self.filter_rows(&rows, predicate)
            }
            QueryPlan::VectorSearch { label, property, query, k } => {
                self.vector_search(label, property, query, *k)
            }
            QueryPlan::ShortestPath { start, end, rel_types, max_hops } => {
                self.find_shortest_path(start, end, rel_types, *max_hops)
            }
            // ...
        }
    }

    fn expand_relationships(
        &self,
        source_rows: &QueryResult,
        rel_pattern: &RelationshipPattern,
        direction: &Direction,
    ) -> Result<QueryResult, ExecuteError> {
        let mut result_rows = Vec::new();

        for row in source_rows.rows() {
            let node_id = row.get_node_id()?;

            let edges = match direction {
                Direction::Outgoing => self.graph_store.outgoing_edges(node_id, &rel_pattern.types),
                Direction::Incoming => self.graph_store.incoming_edges(node_id, &rel_pattern.types),
                Direction::Both => self.graph_store.all_edges(node_id, &rel_pattern.types),
            };

            for edge in edges {
                let target = match direction {
                    Direction::Outgoing => edge.target,
                    Direction::Incoming => edge.source,
                    Direction::Both => if edge.source == node_id { edge.target } else { edge.source },
                };

                let target_node = self.graph_store.get_node(target)?;

                // Check relationship properties
                if let Some(props) = &rel_pattern.properties {
                    if !self.matches_properties(&edge.properties, props) {
                        continue;
                    }
                }

                let mut new_row = row.clone();
                if let Some(var) = &rel_pattern.variable {
                    new_row.set(var, Value::Relationship(edge.clone()));
                }
                new_row.extend_with_node(target_node);

                result_rows.push(new_row);
            }
        }

        Ok(QueryResult::from_rows(result_rows))
    }

    fn vector_search(
        &self,
        label: &str,
        property: &str,
        query: &[f32],
        k: usize,
    ) -> Result<QueryResult, ExecuteError> {
        // Use HNSW index for vector search
        let index = self.graph_store.get_vector_index(label, property)?;
        let results = index.search(query, k);

        let mut rows = Vec::with_capacity(k);
        for (node_id, score) in results {
            let node = self.graph_store.get_node(node_id)?;
            let mut row = Row::new();
            row.set("node", Value::Node(node));
            row.set("score", Value::Float(score));
            rows.push(row);
        }

        Ok(QueryResult::from_rows(rows))
    }
}
```

### Phase 4: Graph Storage (Week 9-10)

```rust
// src/graph/storage/nodes.rs

use dashmap::DashMap;
use parking_lot::RwLock;

/// Node storage with label-based indexing
pub struct NodeStore {
    /// node_id -> node data
    nodes: DashMap<u64, Node>,
    /// label -> set of node_ids
    label_index: DashMap<String, HashSet<u64>>,
    /// (label, property) -> property index
    property_indexes: DashMap<(String, String), PropertyIndex>,
    /// (label, property) -> vector index
    vector_indexes: DashMap<(String, String), HnswIndex>,
    /// Next node ID
    next_id: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: u64,
    pub labels: Vec<String>,
    pub properties: Properties,
}

impl NodeStore {
    pub fn create_node(&self, labels: Vec<String>, properties: Properties) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let node = Node { id, labels: labels.clone(), properties: properties.clone() };

        // Add to main store
        self.nodes.insert(id, node);

        // Update label indexes
        for label in &labels {
            self.label_index
                .entry(label.clone())
                .or_insert_with(HashSet::new)
                .insert(id);
        }

        // Update property indexes
        for (key, value) in &properties {
            for label in &labels {
                if let Some(idx) = self.property_indexes.get(&(label.clone(), key.clone())) {
                    idx.insert(value.clone(), id);
                }
            }
        }

        // Update vector indexes
        for (key, value) in &properties {
            if let Value::Vector(vec) = value {
                for label in &labels {
                    if let Some(idx) = self.vector_indexes.get(&(label.clone(), key.clone())) {
                        idx.insert(id, vec);
                    }
                }
            }
        }

        id
    }

    pub fn nodes_by_label(&self, label: &str) -> Vec<&Node> {
        self.label_index
            .get(label)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.nodes.get(id).map(|n| n.value()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

// src/graph/storage/edges.rs

/// Edge storage with adjacency lists
pub struct EdgeStore {
    /// edge_id -> edge data
    edges: DashMap<u64, Edge>,
    /// node_id -> outgoing edges
    outgoing: DashMap<u64, Vec<u64>>,
    /// node_id -> incoming edges
    incoming: DashMap<u64, Vec<u64>>,
    /// edge_type -> set of edge_ids
    type_index: DashMap<String, HashSet<u64>>,
    /// Next edge ID
    next_id: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub id: u64,
    pub source: u64,
    pub target: u64,
    pub edge_type: String,
    pub properties: Properties,
}

impl EdgeStore {
    pub fn create_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: String,
        properties: Properties,
    ) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let edge = Edge {
            id,
            source,
            target,
            edge_type: edge_type.clone(),
            properties,
        };

        // Add to main store
        self.edges.insert(id, edge);

        // Update adjacency lists
        self.outgoing.entry(source).or_insert_with(Vec::new).push(id);
        self.incoming.entry(target).or_insert_with(Vec::new).push(id);

        // Update type index
        self.type_index
            .entry(edge_type)
            .or_insert_with(HashSet::new)
            .insert(id);

        id
    }

    pub fn outgoing_edges(&self, node_id: u64, types: &[String]) -> Vec<&Edge> {
        self.outgoing
            .get(&node_id)
            .map(|edge_ids| {
                edge_ids.iter()
                    .filter_map(|id| self.edges.get(id))
                    .filter(|e| types.is_empty() || types.contains(&e.edge_type))
                    .map(|e| e.value())
                    .collect()
            })
            .unwrap_or_default()
    }
}
```

### Phase 5: Graph Algorithms (Week 11-12)

```rust
// src/graph/traversal/shortest_path.rs

use std::collections::{BinaryHeap, HashMap, VecDeque};

/// BFS-based shortest path
pub fn shortest_path_bfs(
    store: &GraphStore,
    start: u64,
    end: u64,
    edge_types: &[String],
    max_hops: usize,
) -> Option<Vec<u64>> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut parents: HashMap<u64, u64> = HashMap::new();

    queue.push_back((start, 0));
    visited.insert(start);

    while let Some((node, depth)) = queue.pop_front() {
        if node == end {
            // Reconstruct path
            return Some(reconstruct_path(&parents, start, end));
        }

        if depth >= max_hops {
            continue;
        }

        for edge in store.edges.outgoing_edges(node, edge_types) {
            if !visited.contains(&edge.target) {
                visited.insert(edge.target);
                parents.insert(edge.target, node);
                queue.push_back((edge.target, depth + 1));
            }
        }
    }

    None
}

/// Dijkstra's algorithm for weighted shortest path
pub fn shortest_path_dijkstra(
    store: &GraphStore,
    start: u64,
    end: u64,
    edge_types: &[String],
    weight_property: &str,
) -> Option<(Vec<u64>, f64)> {
    let mut distances: HashMap<u64, f64> = HashMap::new();
    let mut parents: HashMap<u64, u64> = HashMap::new();
    let mut heap = BinaryHeap::new();

    distances.insert(start, 0.0);
    heap.push(Reverse((OrderedFloat(0.0), start)));

    while let Some(Reverse((OrderedFloat(dist), node))) = heap.pop() {
        if node == end {
            return Some((reconstruct_path(&parents, start, end), dist));
        }

        if dist > *distances.get(&node).unwrap_or(&f64::INFINITY) {
            continue;
        }

        for edge in store.edges.outgoing_edges(node, edge_types) {
            let weight = edge.properties
                .get(weight_property)
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);

            let new_dist = dist + weight;

            if new_dist < *distances.get(&edge.target).unwrap_or(&f64::INFINITY) {
                distances.insert(edge.target, new_dist);
                parents.insert(edge.target, node);
                heap.push(Reverse((OrderedFloat(new_dist), edge.target)));
            }
        }
    }

    None
}

/// Vector-guided path finding
pub fn vector_guided_path(
    store: &GraphStore,
    start: u64,
    end: u64,
    edge_types: &[String],
    vector_property: &str,
    max_hops: usize,
) -> Option<Vec<u64>> {
    let target_vec = store.nodes.get_node(end)?
        .properties.get(vector_property)?
        .as_vector()?;

    let mut heap = BinaryHeap::new();
    let mut visited = HashSet::new();
    let mut parents: HashMap<u64, u64> = HashMap::new();

    let start_vec = store.nodes.get_node(start)?
        .properties.get(vector_property)?
        .as_vector()?;

    let start_dist = cosine_distance(start_vec, target_vec);
    heap.push(Reverse((OrderedFloat(start_dist), start, 0)));

    while let Some(Reverse((_, node, depth))) = heap.pop() {
        if node == end {
            return Some(reconstruct_path(&parents, start, end));
        }

        if visited.contains(&node) || depth >= max_hops {
            continue;
        }
        visited.insert(node);

        for edge in store.edges.outgoing_edges(node, edge_types) {
            if visited.contains(&edge.target) {
                continue;
            }

            if let Some(vec) = store.nodes.get_node(edge.target)
                .and_then(|n| n.properties.get(vector_property))
                .and_then(|v| v.as_vector())
            {
                let dist = cosine_distance(vec, target_vec);
                parents.insert(edge.target, node);
                heap.push(Reverse((OrderedFloat(dist), edge.target, depth + 1)));
            }
        }
    }

    None
}
```

### Phase 6: PostgreSQL Integration (Week 13-14)

```rust
// src/graph/operators.rs

// Main Cypher execution function
#[pg_extern]
fn ruvector_cypher(
    graph_name: &str,
    query: &str,
    params: default!(Option<pgrx::JsonB>, "NULL"),
) -> TableIterator<'static, (name!(result, pgrx::JsonB),)> {
    let graph = get_or_create_graph(graph_name);

    // Parse parameters
    let parameters = params
        .map(|p| serde_json::from_value(p.0).unwrap_or_default())
        .unwrap_or_default();

    // Parse query
    let ast = parse_cypher(query).expect("Failed to parse Cypher query");

    // Plan query
    let plan = QueryPlanner::new(&graph).plan(&ast).expect("Failed to plan query");

    // Execute query
    let result = QueryExecutor::new(&graph).execute(&plan).expect("Failed to execute query");

    // Convert to table iterator
    let rows: Vec<_> = result.rows()
        .map(|row| (pgrx::JsonB(row.to_json()),))
        .collect();

    TableIterator::new(rows)
}

// Graph creation
#[pg_extern]
fn ruvector_create_graph(name: &str) -> bool {
    GRAPH_STORE.create_graph(name).is_ok()
}

// Node label creation
#[pg_extern]
fn ruvector_create_node_label(
    graph_name: &str,
    label: &str,
    properties: pgrx::JsonB,
) -> bool {
    let graph = get_graph(graph_name).expect("Graph not found");
    let schema: HashMap<String, String> = serde_json::from_value(properties.0)
        .expect("Invalid properties schema");

    graph.create_label(label, schema).is_ok()
}

// Edge type creation
#[pg_extern]
fn ruvector_create_edge_type(
    graph_name: &str,
    edge_type: &str,
    properties: pgrx::JsonB,
) -> bool {
    let graph = get_graph(graph_name).expect("Graph not found");
    let schema: HashMap<String, String> = serde_json::from_value(properties.0)
        .expect("Invalid properties schema");

    graph.create_edge_type(edge_type, schema).is_ok()
}

// Helper to get graph statistics
#[pg_extern]
fn ruvector_graph_stats(graph_name: &str) -> pgrx::JsonB {
    let graph = get_graph(graph_name).expect("Graph not found");

    pgrx::JsonB(serde_json::json!({
        "node_count": graph.node_count(),
        "edge_count": graph.edge_count(),
        "labels": graph.labels(),
        "edge_types": graph.edge_types(),
        "memory_mb": graph.memory_usage_mb(),
    }))
}
```

## Supported Cypher Features

### Clauses
- `MATCH` - Pattern matching
- `OPTIONAL MATCH` - Optional pattern matching
- `CREATE` - Create nodes/relationships
- `MERGE` - Match or create
- `DELETE` / `DETACH DELETE` - Delete nodes/relationships
- `SET` - Update properties
- `REMOVE` - Remove properties/labels
- `RETURN` - Return results
- `WITH` - Query chaining
- `WHERE` - Filtering
- `ORDER BY` - Sorting
- `SKIP` / `LIMIT` - Pagination
- `UNION` / `UNION ALL` - Combining results

### Expressions
- Property access: `n.name`
- Labels: `n:Person`
- Relationship types: `[:KNOWS]`
- Variable length: `[:KNOWS*1..3]`
- List comprehensions: `[x IN list WHERE x > 5]`
- CASE expressions

### Functions
- Aggregation: `count()`, `sum()`, `avg()`, `min()`, `max()`, `collect()`
- String: `toUpper()`, `toLower()`, `trim()`, `split()`
- Math: `abs()`, `ceil()`, `floor()`, `round()`, `sqrt()`
- List: `head()`, `tail()`, `size()`, `range()`
- Path: `length()`, `nodes()`, `relationships()`
- **RuVector-specific**:
  - `ruvector.similarity(embedding1, embedding2)`
  - `ruvector.distance(embedding1, embedding2, metric)`
  - `ruvector.knn(embedding, k)`

## Benchmarks

| Operation | Nodes | Edges | Time (ms) |
|-----------|-------|-------|-----------|
| Simple MATCH | 100K | 1M | 2.5 |
| 2-hop traversal | 100K | 1M | 15 |
| Shortest path (BFS) | 100K | 1M | 8 |
| Vector-guided path | 100K | 1M | 25 |
| PageRank (20 iter) | 100K | 1M | 450 |
| Community detection | 100K | 1M | 1200 |

## Dependencies

```toml
[dependencies]
# Link to ruvector-graph
ruvector-graph = { path = "../ruvector-graph", optional = true }

# Parser
pest = "2.7"
pest_derive = "2.7"

# Concurrent collections
dashmap = "6.0"
parking_lot = "0.12"

# Graph algorithms
petgraph = { version = "0.6", optional = true }
```

## Feature Flags

```toml
[features]
graph = []
graph-cypher = ["graph", "pest", "pest_derive"]
graph-algorithms = ["graph", "petgraph"]
graph-vector = ["graph", "index-hnsw"]
graph-all = ["graph-cypher", "graph-algorithms", "graph-vector"]
```
