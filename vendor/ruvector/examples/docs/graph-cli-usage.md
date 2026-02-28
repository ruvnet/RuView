# RuVector Graph CLI - Usage Examples

This guide demonstrates practical usage of the RuVector graph database CLI commands.

## Quick Start

### 1. Create a New Graph Database

```bash
# Create with default settings
ruvector graph create --path ./my-graph.db --name production

# Create with property indexing enabled
ruvector graph create --path ./my-graph.db --name production --indexed
```

**Output:**
```
✓ Creating graph database at: ./my-graph.db
  Graph name: production
  Property indexing: enabled
✓ Graph database created successfully!
ℹ Use 'ruvector graph shell' to start interactive mode
```

### 2. Execute Cypher Queries

```bash
# Simple query
ruvector graph query -b ./my-graph.db -q "MATCH (n:Person) RETURN n"

# Query with output format
ruvector graph query -b ./my-graph.db -q "MATCH (n) RETURN n" --format json

# Show query execution plan
ruvector graph query -b ./my-graph.db -q "MATCH (n)-[r]->(m) RETURN n, r, m" --explain
```

**Note:** Use `-b` for database path (not `-d`, which is global debug flag)
**Note:** Use `-q` for cypher query (not `-c`, which is global config flag)

### 3. Interactive Shell (REPL)

```bash
# Start shell
ruvector graph shell -b ./my-graph.db

# Start shell with multiline mode (queries end with semicolon)
ruvector graph shell -b ./my-graph.db --multiline
```

**Example Session:**
```
RuVector Graph Shell
Database: ./my-graph.db
Type :exit to exit, :help for help

cypher> CREATE (alice:Person {name: 'Alice', age: 30})
✓ Query completed in 5.23ms

cypher> CREATE (bob:Person {name: 'Bob', age: 25})
✓ Query completed in 3.12ms

cypher> MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
   ... CREATE (a)-[:KNOWS {since: 2020}]->(b)
✓ Query completed in 4.56ms

cypher> MATCH (n:Person) RETURN n.name, n.age
+--------+-------+
| name   | age   |
+--------+-------+
| Alice  | 30    |
| Bob    | 25    |
+--------+-------+

cypher> :exit
✓ Goodbye!
```

**Shell Commands:**
- `:exit`, `:quit`, `:q` - Exit the shell
- `:help`, `:h` - Show help
- `:clear` - Clear query buffer

### 4. Import Graph Data

#### JSON Format

```bash
ruvector graph import -b ./my-graph.db -i data.json --format json -g production
```

**Example `data.json`:**
```json
{
  "nodes": [
    {
      "id": "1",
      "labels": ["Person"],
      "properties": {
        "name": "Alice",
        "age": 30,
        "city": "San Francisco"
      }
    },
    {
      "id": "2",
      "labels": ["Person"],
      "properties": {
        "name": "Bob",
        "age": 25,
        "city": "New York"
      }
    }
  ],
  "relationships": [
    {
      "id": "r1",
      "type": "KNOWS",
      "startNode": "1",
      "endNode": "2",
      "properties": {
        "since": 2020,
        "closeness": 0.8
      }
    }
  ]
}
```

#### Cypher Format

```bash
ruvector graph import -b ./my-graph.db -i init.cypher --format cypher
```

**Example `init.cypher`:**
```cypher
CREATE (alice:Person {name: 'Alice', age: 30, city: 'SF'});
CREATE (bob:Person {name: 'Bob', age: 25, city: 'NYC'});
CREATE (carol:Person {name: 'Carol', age: 28, city: 'LA'});

MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
CREATE (a)-[:KNOWS {since: 2020}]->(b);

MATCH (b:Person {name: 'Bob'}), (c:Person {name: 'Carol'})
CREATE (b)-[:KNOWS {since: 2021}]->(c);
```

#### CSV Format

```bash
ruvector graph import -b ./my-graph.db -i nodes.csv --format csv
```

**Example `nodes.csv`:**
```csv
id,labels,properties
1,"[""Person""]","{""name"": ""Alice"", ""age"": 30}"
2,"[""Person""]","{""name"": ""Bob"", ""age"": 25}"
```

### 5. Export Graph Data

```bash
# Export to JSON
ruvector graph export -b ./my-graph.db -o backup.json --format json

# Export to Cypher statements
ruvector graph export -b ./my-graph.db -o backup.cypher --format cypher

# Export to GraphML (for Gephi, Cytoscape, etc.)
ruvector graph export -b ./my-graph.db -o graph.graphml --format graphml

# Export specific graph
ruvector graph export -b ./my-graph.db -o prod-backup.json -g production
```

### 6. Database Information

```bash
# Basic info
ruvector graph info -b ./my-graph.db

# Detailed statistics
ruvector graph info -b ./my-graph.db --detailed
```

**Output:**
```
Graph Database Statistics
  Database: ./my-graph.db
  Graphs: 1
  Total nodes: 1,234
  Total relationships: 5,678
  Node labels: 3
  Relationship types: 5

Storage Information:
  Store size: 45.2 MB
  Index size: 12.8 MB

Configuration:
  Cache size: 128 MB
  Page size: 4096 bytes
```

### 7. Performance Benchmarks

```bash
# Traverse benchmark
ruvector graph benchmark -b ./my-graph.db -n 1000 -t traverse

# Pattern matching benchmark
ruvector graph benchmark -b ./my-graph.db -n 5000 -t pattern

# Aggregation benchmark
ruvector graph benchmark -b ./my-graph.db -n 2000 -t aggregate
```

**Output:**
```
Running graph benchmark...
  Benchmark type: traverse
  Queries: 1000

Benchmark Results:
  Total time: 2.45s
  Queries per second: 408
  Average latency: 2.45ms
```

### 8. Start Graph Server

```bash
# Basic server
ruvector graph serve -b ./my-graph.db

# Custom ports
ruvector graph serve -b ./my-graph.db --http-port 9000 --grpc-port 50052

# Public server with GraphQL
ruvector graph serve -b ./my-graph.db --host 0.0.0.0 --graphql

# Full configuration
ruvector graph serve \
  -b ./my-graph.db \
  --host 0.0.0.0 \
  --http-port 8080 \
  --grpc-port 50051 \
  --graphql
```

**Server Endpoints:**
- HTTP: `http://localhost:8080/query`
- gRPC: `localhost:50051`
- GraphQL: `http://localhost:8080/graphql` (if enabled)

## Common Workflows

### Building a Social Network

```bash
# Create database
ruvector graph create --path social.db --name social --indexed

# Start interactive shell
ruvector graph shell -b social.db --multiline
```

**In the shell:**
```cypher
-- Create nodes
CREATE (alice:Person {name: 'Alice', age: 30, interests: ['AI', 'Databases']});
CREATE (bob:Person {name: 'Bob', age: 25, interests: ['Rust', 'Systems']});
CREATE (carol:Person {name: 'Carol', age: 28, interests: ['AI', 'Rust']});

-- Create relationships
MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
CREATE (a)-[:KNOWS {since: 2020, strength: 0.8}]->(b);

MATCH (b:Person {name: 'Bob'}), (c:Person {name: 'Carol'})
CREATE (b)-[:KNOWS {since: 2021, strength: 0.9}]->(c);

-- Query friends
MATCH (a:Person {name: 'Alice'})-[:KNOWS]->(friend)
RETURN friend.name, friend.age;

-- Find friends of friends
MATCH (a:Person {name: 'Alice'})-[:KNOWS*2..3]-(fof)
WHERE fof.name <> 'Alice'
RETURN DISTINCT fof.name;

-- Find common interests
MATCH (p:Person)
WHERE ANY(interest IN p.interests WHERE interest IN ['AI', 'Rust'])
RETURN p.name, p.interests;
```

### Knowledge Graph RAG System

```bash
# Create knowledge graph
ruvector graph create --path knowledge.db --name kg --indexed

# Import from JSON
ruvector graph import -b knowledge.db -i documents.json --format json

# Query for similar concepts
ruvector graph query -b knowledge.db -q \
  "MATCH (d:Document)-[:MENTIONS]->(c:Concept)
   WHERE c.name = 'Machine Learning'
   RETURN d.title, d.content"

# Export for backup
ruvector graph export -b knowledge.db -o kg-backup.cypher --format cypher
```

### Recommendation Engine

```bash
# Create recommendations graph
ruvector graph create --path recommendations.db --name rec

# Import user-item interactions
ruvector graph import -b recommendations.db -i interactions.csv --format csv

# Find recommendations via collaborative filtering
ruvector graph query -b recommendations.db -q \
  "MATCH (u:User {id: '123'})-[:LIKED]->(i:Item)<-[:LIKED]-(other:User)
   MATCH (other)-[:LIKED]->(rec:Item)
   WHERE NOT (u)-[:LIKED]->(rec)
   RETURN rec.name, COUNT(*) as score
   ORDER BY score DESC
   LIMIT 10"
```

## Advanced Usage

### Batch Import with Error Handling

```bash
# Skip errors and continue importing
ruvector graph import -b ./db.db -i large-dataset.json --skip-errors
```

### Performance Testing

```bash
# Run comprehensive benchmarks
for type in traverse pattern aggregate; do
  echo "Testing $type..."
  ruvector graph benchmark -b ./db.db -n 10000 -t $type
done
```

### Multi-Graph Management

```bash
# Create multiple graphs in same database
ruvector graph query -b ./db.db -q "CREATE DATABASE users"
ruvector graph query -b ./db.db -q "CREATE DATABASE products"

# Import to specific graphs
ruvector graph import -b ./db.db -i users.json -g users
ruvector graph import -b ./db.db -i products.json -g products

# Query specific graph
ruvector graph query -b ./db.db -g users -q "MATCH (n:User) RETURN n"
```

### Server Deployment

```bash
# Development server
ruvector graph serve -b ./dev.db --host 127.0.0.1 --http-port 8080

# Production server with GraphQL
ruvector graph serve \
  -b /data/prod.db \
  --host 0.0.0.0 \
  --http-port 8080 \
  --grpc-port 50051 \
  --graphql
```

## Global Options

All graph commands support these global options:

```bash
# Use custom config
ruvector --config ./custom-config.toml graph query -q "MATCH (n) RETURN n"

# Enable debug mode
ruvector --debug graph info -b ./db.db

# Disable colors (for scripting)
ruvector --no-color graph query -q "MATCH (n) RETURN n" --format json
```

## Tips and Best Practices

1. **Use Short Flags for Common Options:**
   - `-b` for `--db` (database path)
   - `-q` for `--cypher` (query string)
   - `-i` for `--input` (import file)
   - `-o` for `--output` (export file)
   - `-g` for `--graph` (graph name)
   - `-n` for `--queries` (benchmark count)
   - `-t` for `--bench-type` (benchmark type)

2. **Interactive Mode Best Practices:**
   - Use `--multiline` for complex queries
   - End queries with `;` in multiline mode
   - Use `:clear` to reset query buffer
   - Use `:help` for shell commands

3. **Performance:**
   - Enable `--indexed` for large graphs
   - Use benchmarks to test query performance
   - Monitor with `--detailed` info flag

4. **Data Management:**
   - Always backup with `export` before major changes
   - Use `--skip-errors` for large imports
   - Export to multiple formats for compatibility

## Troubleshooting

### Command Not Found
```bash
# Ensure binary is built
cargo build --package ruvector-cli --bin ruvector

# Use from target directory
./target/debug/ruvector graph --help
```

### Flag Conflicts
Remember that global flags take precedence:
- Use `-b` for `--db` (NOT `-d`, which is `--debug`)
- Use `-q` for `--cypher` (NOT `-c`, which is `--config`)

### Server Won't Start
```bash
# Check if port is in use
lsof -i :8080

# Use different port
ruvector graph serve -b ./db.db --http-port 9000
```

## Next Steps

1. See [cli-graph-commands.md](../docs/cli-graph-commands.md) for detailed command reference
2. Check [neo4j-integration.md](../docs/neo4j-integration.md) for integration details
3. Read [configuration.md](../docs/configuration.md) for advanced settings

## Integration Status

Current implementation provides CLI interface with placeholder functions. All commands are ready for integration with the `ruvector-neo4j` crate for full Neo4j-compatible graph database functionality.

**TODO markers in code indicate integration points:**
```rust
// TODO: Integrate with ruvector-neo4j Neo4jGraph implementation
```
