# Graph Operations, Cypher & SPARQL Module

This module provides graph database capabilities for the ruvector-postgres extension, including graph storage, traversal algorithms, Cypher query support, and W3C-standard SPARQL for RDF data.

## Features

- **Concurrent Graph Storage**: Thread-safe graph storage using DashMap
- **Node & Edge Management**: Full-featured node and edge storage with properties
- **Label Indexing**: Fast node lookups by label
- **Adjacency Lists**: Efficient edge traversal with O(1) neighbor access
- **Graph Traversal**: BFS, DFS, and Dijkstra's shortest path algorithms
- **Cypher Support**: Simplified Cypher query language for graph operations
- **SPARQL 1.1 Support**: W3C-standard query language for RDF triple stores
- **RDF Triple Store**: Efficient storage with SPO/POS/OSP indexing
- **PostgreSQL Integration**: Native pgrx-based PostgreSQL functions

## Architecture

### Storage Layer (`storage.rs`)

```rust
// Node with labels and properties
pub struct Node {
    pub id: u64,
    pub labels: Vec<String>,
    pub properties: HashMap<String, JsonValue>,
}

// Edge with type and properties
pub struct Edge {
    pub id: u64,
    pub source: u64,
    pub target: u64,
    pub edge_type: String,
    pub properties: HashMap<String, JsonValue>,
}

// Concurrent storage with indexing
pub struct GraphStore {
    pub nodes: NodeStore,  // DashMap-based
    pub edges: EdgeStore,  // DashMap-based
}
```

### Traversal Layer (`traversal.rs`)

Implements common graph algorithms:

- **BFS**: Breadth-first search for shortest path by hop count
- **DFS**: Depth-first search with visitor pattern
- **Dijkstra**: Weighted shortest path with custom edge weights
- **All Paths**: Find multiple paths between nodes

### Cypher Layer (`cypher/`)

Simplified Cypher query language support:

- **AST** (`ast.rs`): Complete abstract syntax tree for Cypher
- **Parser** (`parser.rs`): Basic parser for common Cypher patterns
- **Executor** (`executor.rs`): Query execution engine

Supported Cypher clauses:
- `CREATE`: Create nodes and relationships
- `MATCH`: Pattern matching
- `WHERE`: Filtering
- `RETURN`: Result projection
- `SET`, `DELETE`, `WITH`: Basic support

### SPARQL Layer (`sparql/`)

W3C SPARQL 1.1 implementation for RDF data:

- **AST** (`ast.rs`): Complete SPARQL abstract syntax tree
- **Parser** (`parser.rs`): Full SPARQL 1.1 query parser
- **Executor** (`executor.rs`): Query execution with BGP matching, JOINs
- **Triple Store** (`triple_store.rs`): Efficient RDF storage with SPO/POS/OSP indexes
- **Functions** (`functions.rs`): 50+ built-in SPARQL functions
- **Results** (`results.rs`): JSON, XML, CSV, TSV formatters

Supported SPARQL features:
- Query forms: `SELECT`, `CONSTRUCT`, `ASK`, `DESCRIBE`
- Graph patterns: `OPTIONAL`, `UNION`, `MINUS`, `FILTER`
- Property paths: `/`, `|`, `^`, `*`, `+`, `?`
- Aggregates: `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`, `GROUP_CONCAT`
- Solution modifiers: `ORDER BY`, `LIMIT`, `OFFSET`, `GROUP BY`, `HAVING`
- Update operations: `INSERT DATA`, `DELETE DATA`, `DELETE/INSERT WHERE`

## PostgreSQL Functions

### Graph Management

```sql
-- Create a new graph
SELECT ruvector_create_graph('my_graph');

-- List all graphs
SELECT ruvector_list_graphs();

-- Delete a graph
SELECT ruvector_delete_graph('my_graph');

-- Get graph statistics
SELECT ruvector_graph_stats('my_graph');
-- Returns: {"name": "my_graph", "node_count": 100, "edge_count": 250, ...}
```

### Node Operations

```sql
-- Add a node
SELECT ruvector_add_node(
    'my_graph',
    ARRAY['Person', 'Employee'],  -- Labels
    '{"name": "Alice", "age": 30, "department": "Engineering"}'::jsonb
);
-- Returns: node_id (bigint)

-- Get a node by ID
SELECT ruvector_get_node('my_graph', 1);
-- Returns: {"id": 1, "labels": ["Person"], "properties": {...}}

-- Find nodes by label
SELECT ruvector_find_nodes_by_label('my_graph', 'Person');
-- Returns: array of nodes
```

### Edge Operations

```sql
-- Add an edge
SELECT ruvector_add_edge(
    'my_graph',
    1,  -- source_id
    2,  -- target_id
    'KNOWS',  -- edge_type
    '{"since": 2020, "weight": 0.8}'::jsonb
);
-- Returns: edge_id (bigint)

-- Get an edge by ID
SELECT ruvector_get_edge('my_graph', 1);

-- Get neighbors of a node
SELECT ruvector_get_neighbors('my_graph', 1);
-- Returns: array of node IDs
```

### Graph Traversal

```sql
-- Find shortest path (unweighted)
SELECT ruvector_shortest_path(
    'my_graph',
    1,    -- start_id
    10,   -- end_id
    5     -- max_hops
);
-- Returns: {"nodes": [1, 3, 7, 10], "edges": [12, 45, 89], "length": 4, "cost": 0}

-- Find weighted shortest path
SELECT ruvector_shortest_path_weighted(
    'my_graph',
    1,    -- start_id
    10,   -- end_id
    'weight'  -- property name for edge weights
);
-- Returns: {"nodes": [...], "edges": [...], "length": 4, "cost": 2.5}
```

### Cypher Queries

```sql
-- Create nodes
SELECT ruvector_cypher(
    'my_graph',
    'CREATE (n:Person {name: ''Alice'', age: 30}) RETURN n',
    NULL
);

-- Match and filter
SELECT ruvector_cypher(
    'my_graph',
    'MATCH (n:Person) WHERE n.age > 25 RETURN n.name, n.age',
    NULL
);

-- Parameterized queries
SELECT ruvector_cypher(
    'my_graph',
    'MATCH (n:Person) WHERE n.name = $name RETURN n',
    '{"name": "Alice"}'::jsonb
);

-- Create relationships
SELECT ruvector_cypher(
    'my_graph',
    'CREATE (a:Person {name: ''Alice''})-[:KNOWS {since: 2020}]->(b:Person {name: ''Bob''}) RETURN a, b',
    NULL
);
```

### SPARQL / RDF Operations

```sql
-- Create RDF triple store
SELECT ruvector_create_rdf_store('my_knowledge_base');

-- Insert individual triples
SELECT ruvector_insert_triple(
    'my_knowledge_base',
    '<http://example.org/person/alice>',
    '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>',
    '<http://example.org/Person>'
);

-- Insert triple into named graph
SELECT ruvector_insert_triple_graph(
    'my_knowledge_base',
    '<http://example.org/person/alice>',
    '<http://xmlns.com/foaf/0.1/name>',
    '"Alice Smith"',
    'http://example.org/people'
);

-- Bulk load N-Triples format
SELECT ruvector_load_ntriples('my_knowledge_base', '
    <http://example.org/person/bob> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> .
    <http://example.org/person/bob> <http://xmlns.com/foaf/0.1/name> "Bob Jones" .
    <http://example.org/person/alice> <http://xmlns.com/foaf/0.1/knows> <http://example.org/person/bob> .
');

-- Execute SPARQL SELECT query
SELECT ruvector_sparql('my_knowledge_base', '
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    SELECT ?person ?name
    WHERE {
        ?person a <http://example.org/Person> .
        ?person foaf:name ?name .
    }
    ORDER BY ?name
', 'json');

-- SPARQL ASK query
SELECT ruvector_sparql('my_knowledge_base',
    'ASK { <http://example.org/person/alice> <http://xmlns.com/foaf/0.1/knows> ?friend }',
    'json'
);

-- Get results as JSONB
SELECT ruvector_sparql_json('my_knowledge_base',
    'SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 10'
);

-- Query triples by pattern (NULL = wildcard)
SELECT ruvector_query_triples('my_knowledge_base',
    '<http://example.org/person/alice>',  -- subject
    NULL,                                   -- any predicate
    NULL                                    -- any object
);

-- Get store statistics
SELECT ruvector_rdf_stats('my_knowledge_base');
-- Returns: {"name": "...", "triple_count": 5, "subject_count": 2, ...}

-- SPARQL UPDATE
SELECT ruvector_sparql_update('my_knowledge_base', '
    INSERT DATA {
        <http://example.org/person/charlie> <http://xmlns.com/foaf/0.1/name> "Charlie" .
    }
');

-- Clear store
SELECT ruvector_clear_rdf_store('my_knowledge_base');

-- Delete store
SELECT ruvector_delete_rdf_store('my_knowledge_base');

-- List all stores
SELECT ruvector_list_rdf_stores();
```

## Usage Examples

### Social Network

```sql
-- Create graph
SELECT ruvector_create_graph('social_network');

-- Add users
WITH users AS (
    SELECT ruvector_add_node('social_network', ARRAY['Person'],
        jsonb_build_object('name', name, 'age', age))
    FROM (VALUES
        ('Alice', 30),
        ('Bob', 25),
        ('Charlie', 35),
        ('Diana', 28)
    ) AS t(name, age)
)

-- Create friendships
SELECT ruvector_add_edge('social_network', 1, 2, 'FRIENDS',
    '{"since": "2020-01-15"}'::jsonb);
SELECT ruvector_add_edge('social_network', 2, 3, 'FRIENDS',
    '{"since": "2019-06-20"}'::jsonb);
SELECT ruvector_add_edge('social_network', 1, 4, 'FRIENDS',
    '{"since": "2021-03-10"}'::jsonb);

-- Find connection between Alice and Charlie
SELECT ruvector_shortest_path('social_network', 1, 3, 10);

-- Cypher: Find all friends of friends
SELECT ruvector_cypher(
    'social_network',
    'MATCH (a:Person)-[:FRIENDS]->(b:Person)-[:FRIENDS]->(c:Person)
     WHERE a.name = ''Alice'' RETURN c.name',
    NULL
);
```

### SPARQL Knowledge Graph

```sql
-- Create RDF knowledge graph
SELECT ruvector_create_rdf_store('dbpedia_subset');

-- Load sample data
SELECT ruvector_load_ntriples('dbpedia_subset', '
    <http://dbpedia.org/resource/Albert_Einstein> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Scientist> .
    <http://dbpedia.org/resource/Albert_Einstein> <http://xmlns.com/foaf/0.1/name> "Albert Einstein" .
    <http://dbpedia.org/resource/Albert_Einstein> <http://dbpedia.org/ontology/birthPlace> <http://dbpedia.org/resource/Ulm> .
    <http://dbpedia.org/resource/Albert_Einstein> <http://dbpedia.org/ontology/field> <http://dbpedia.org/resource/Physics> .
    <http://dbpedia.org/resource/Marie_Curie> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Scientist> .
    <http://dbpedia.org/resource/Marie_Curie> <http://xmlns.com/foaf/0.1/name> "Marie Curie" .
    <http://dbpedia.org/resource/Marie_Curie> <http://dbpedia.org/ontology/field> <http://dbpedia.org/resource/Physics> .
');

-- Find all scientists in physics
SELECT ruvector_sparql('dbpedia_subset', '
    PREFIX dbo: <http://dbpedia.org/ontology/>
    PREFIX dbr: <http://dbpedia.org/resource/>
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    SELECT ?name
    WHERE {
        ?person a dbo:Scientist .
        ?person dbo:field dbr:Physics .
        ?person foaf:name ?name .
    }
', 'json');

-- Check if Einstein was a scientist
SELECT ruvector_sparql('dbpedia_subset', '
    PREFIX dbo: <http://dbpedia.org/ontology/>
    PREFIX dbr: <http://dbpedia.org/resource/>

    ASK { dbr:Albert_Einstein a dbo:Scientist }
', 'json');

-- Get all properties of Einstein
SELECT ruvector_query_triples('dbpedia_subset',
    '<http://dbpedia.org/resource/Albert_Einstein>',
    NULL,
    NULL
);
```

### Knowledge Graph

```sql
-- Create knowledge graph
SELECT ruvector_create_graph('knowledge');

-- Add concepts
SELECT ruvector_add_node('knowledge', ARRAY['Concept'],
    '{"name": "Machine Learning", "category": "AI"}'::jsonb);
SELECT ruvector_add_node('knowledge', ARRAY['Concept'],
    '{"name": "Neural Networks", "category": "AI"}'::jsonb);
SELECT ruvector_add_node('knowledge', ARRAY['Concept'],
    '{"name": "Deep Learning", "category": "AI"}'::jsonb);

-- Create relationships
SELECT ruvector_add_edge('knowledge', 1, 2, 'INCLUDES',
    '{"strength": 0.9}'::jsonb);
SELECT ruvector_add_edge('knowledge', 2, 3, 'SPECIALIZES_IN',
    '{"strength": 0.95}'::jsonb);

-- Find weighted path
SELECT ruvector_shortest_path_weighted('knowledge', 1, 3, 'strength');
```

### Recommendation System

```sql
-- Create graph
SELECT ruvector_create_graph('recommendations');

-- Add users and items
SELECT ruvector_cypher('recommendations',
    'CREATE (u:User {name: ''Alice''})
     CREATE (m1:Movie {title: ''Inception''})
     CREATE (m2:Movie {title: ''Interstellar''})
     CREATE (u)-[:WATCHED {rating: 5}]->(m1)
     CREATE (u)-[:WATCHED {rating: 4}]->(m2)
     RETURN u, m1, m2',
    NULL
);

-- Find similar users or items
SELECT ruvector_cypher('recommendations',
    'MATCH (u1:User)-[:WATCHED]->(m:Movie)<-[:WATCHED]-(u2:User)
     WHERE u1.name = ''Alice''
     RETURN u2.name, COUNT(m) AS common_movies
     ORDER BY common_movies DESC',
    NULL
);
```

## Performance Characteristics

### Storage

- **Node Lookup**: O(1) by ID, O(k) by label (k = nodes with label)
- **Edge Lookup**: O(1) by ID, O(d) for neighbors (d = degree)
- **Concurrent Access**: Lock-free reads, minimal contention on writes

### Traversal

- **BFS**: O(V + E) time, O(V) space
- **DFS**: O(V + E) time, O(h) space (h = max depth)
- **Dijkstra**: O((V + E) log V) time with binary heap

### Scalability

- Thread-safe concurrent operations
- Memory-efficient adjacency lists
- Label and type indexing for fast filtering

## Implementation Details

### Concurrent Storage

Uses `DashMap` for lock-free concurrent access:

```rust
pub struct NodeStore {
    nodes: DashMap<u64, Node>,
    label_index: DashMap<String, HashSet<u64>>,
    next_id: AtomicU64,
}
```

### Graph Registry

Global registry for named graphs:

```rust
static GRAPH_REGISTRY: Lazy<DashMap<String, Arc<GraphStore>>> = ...
```

### Cypher Parser

Basic recursive descent parser:
- Handles common patterns: `(n:Label {prop: value})`
- Relationship patterns: `-[:TYPE]->`, `<-[:TYPE]-`
- WHERE conditions, RETURN projections
- Property extraction and type inference

## Limitations

### Current Parser Limitations

The Cypher parser is simplified for demonstration:
- No support for complex WHERE conditions (AND/OR)
- Limited expression support (basic comparisons only)
- No aggregation functions (COUNT, SUM, etc.)
- No ORDER BY or GROUP BY clauses
- Basic pattern matching only

### Production Recommendations

For production use, consider:
- Using a proper parser library (nom, pest, lalrpop)
- Adding comprehensive error messages
- Implementing full Cypher specification
- Query optimization and planning
- Transaction support
- Persistence layer

## Testing

Comprehensive test suite included:

```bash
# Run all tests
cargo pgrx test

# Run specific test
cargo pgrx test test_create_graph
```

Test coverage:
- Node and edge CRUD operations
- Graph traversal algorithms
- Cypher query execution
- PostgreSQL function integration
- Concurrent access patterns

## Future Enhancements

- [x] SPARQL 1.1 query support
- [x] RDF triple store with indexing
- [ ] Graph analytics (PageRank, community detection)
- [ ] Temporal graphs (time-aware edges)
- [ ] Property graph constraints
- [ ] Full-text search on properties
- [ ] Persistent storage backend
- [ ] Query optimization
- [ ] Distributed graph support
- [ ] GraphQL interface
- [ ] SPARQL federated queries
- [ ] OWL/RDFS reasoning

## References

- [Cypher Query Language](https://neo4j.com/developer/cypher/)
- [Property Graph Model](https://en.wikipedia.org/wiki/Graph_database#Labeled-property_graph)
- [Graph Algorithms](https://en.wikipedia.org/wiki/Graph_traversal)
- [SPARQL 1.1 Query Language](https://www.w3.org/TR/sparql11-query/)
- [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/)
- [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/)
- [pgrx Documentation](https://github.com/pgcentralfoundation/pgrx)
