# RuVector CLI - Graph Database Commands

The RuVector CLI now includes comprehensive graph database support with Neo4j-compatible Cypher query capabilities.

## Available Graph Commands

### 1. Create Graph Database

Create a new graph database with optional property indexing.

```bash
ruvector graph create --path ./my-graph.db --name my-graph --indexed
```

**Options:**
- `--path, -p` - Database file path (default: `./ruvector-graph.db`)
- `--name, -n` - Graph name (default: `default`)
- `--indexed` - Enable property indexing for faster queries

### 2. Execute Cypher Query

Run a Cypher query against the graph database.

```bash
ruvector graph query -b ./my-graph.db -q "MATCH (n:Person) RETURN n" --format table
```

**Options:**
- `--db, -b` - Database file path (default: `./ruvector-graph.db`)
- `--cypher, -q` - Cypher query to execute
- `--format` - Output format: `table`, `json`, or `csv` (default: `table`)
- `--explain` - Show query execution plan

**Note:** Use `-b` for database (NOT `-d`, which is for `--debug`) and `-q` for query (NOT `-c`, which is for `--config`)

**Examples:**

```bash
# Create a node
ruvector graph query -q "CREATE (n:Person {name: 'Alice', age: 30})"

# Find nodes
ruvector graph query -q "MATCH (n:Person) WHERE n.age > 25 RETURN n"

# Create relationships
ruvector graph query -q "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) CREATE (a)-[:KNOWS]->(b)"

# Pattern matching
ruvector graph query -q "MATCH (a)-[r:KNOWS]->(b) RETURN a.name, b.name"

# Get execution plan
ruvector graph query -q "MATCH (n:Person) RETURN n" --explain

# Specify database and output format
ruvector graph query -b ./my-graph.db -q "MATCH (n) RETURN n" --format json
```

### 3. Interactive Cypher Shell (REPL)

Start an interactive shell for executing Cypher queries.

```bash
ruvector graph shell --db ./my-graph.db --multiline
```

**Options:**
- `--db, -b` - Database file path (default: `./ruvector-graph.db`)
- `--multiline` - Enable multiline mode (queries end with `;`)

**Shell Commands:**
- `:exit`, `:quit`, `:q` - Exit the shell
- `:help`, `:h` - Show help message
- `:clear` - Clear query buffer

**Example Session:**

```
RuVector Graph Shell
Database: ./my-graph.db
Type :exit to exit, :help for help

cypher> CREATE (n:Person {name: 'Alice'})
✓ Query completed in 12.34ms

cypher> MATCH (n:Person) RETURN n.name
+--------+
| n.name |
+--------+
| Alice  |
+--------+

cypher> :exit
✓ Goodbye!
```

### 4. Import Graph Data

Import data from CSV, JSON, or Cypher files.

```bash
ruvector graph import -b ./my-graph.db -i data.json --format json -g default
```

**Options:**
- `--db, -b` - Database file path (default: `./ruvector-graph.db`)
- `--input, -i` - Input file path
- `--format` - Input format: `csv`, `json`, or `cypher` (default: `json`)
- `--graph, -g` - Graph name (default: `default`)
- `--skip-errors` - Continue on errors

**JSON Format Example:**

```json
{
  "nodes": [
    {
      "id": "1",
      "labels": ["Person"],
      "properties": {"name": "Alice", "age": 30}
    }
  ],
  "relationships": [
    {
      "id": "1",
      "type": "KNOWS",
      "startNode": "1",
      "endNode": "2",
      "properties": {"since": 2020}
    }
  ]
}
```

**CSV Format:**
- Nodes: `nodes.csv` with columns: `id,labels,properties`
- Relationships: `relationships.csv` with columns: `id,type,start,end,properties`

**Cypher Format:**
Plain text file with Cypher CREATE statements.

### 5. Export Graph Data

Export graph data to various formats.

```bash
ruvector graph export -b ./my-graph.db -o backup.json --format json
```

**Options:**
- `--db, -b` - Database file path (default: `./ruvector-graph.db`)
- `--output, -o` - Output file path
- `--format` - Output format: `json`, `csv`, `cypher`, or `graphml` (default: `json`)
- `--graph, -g` - Graph name (default: `default`)

**Output Formats:**
- `json` - JSON graph format (nodes and relationships)
- `csv` - Separate CSV files for nodes and relationships
- `cypher` - Cypher CREATE statements
- `graphml` - GraphML XML format for visualization tools

### 6. Graph Database Info

Display statistics and information about the graph database.

```bash
ruvector graph info -b ./my-graph.db --detailed
```

**Options:**
- `--db, -b` - Database file path (default: `./ruvector-graph.db`)
- `--detailed` - Show detailed statistics including storage and configuration

**Example Output:**

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

### 7. Graph Benchmarks

Run performance benchmarks on the graph database.

```bash
ruvector graph benchmark -b ./my-graph.db -n 1000 -t traverse
```

**Options:**
- `--db, -b` - Database file path (default: `./ruvector-graph.db`)
- `--queries, -n` - Number of queries to run (default: `1000`)
- `--bench-type, -t` - Benchmark type: `traverse`, `pattern`, or `aggregate` (default: `traverse`)

**Benchmark Types:**
- `traverse` - Graph traversal operations
- `pattern` - Pattern matching queries
- `aggregate` - Aggregation queries

**Example Output:**

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

Start an HTTP/gRPC server for remote graph access.

```bash
ruvector graph serve -b ./my-graph.db --host 0.0.0.0 --http-port 8080 --grpc-port 50051 --graphql
```

**Options:**
- `--db, -b` - Database file path (default: `./ruvector-graph.db`)
- `--host` - Server host (default: `127.0.0.1`)
- `--http-port` - HTTP port (default: `8080`)
- `--grpc-port` - gRPC port (default: `50051`)
- `--graphql` - Enable GraphQL endpoint

**Endpoints:**
- HTTP: `http://localhost:8080/query` - Execute Cypher queries via HTTP POST
- gRPC: `localhost:50051` - High-performance RPC interface
- GraphQL: `http://localhost:8080/graphql` - GraphQL endpoint (if enabled)

## Integration with RuVector Neo4j

These CLI commands are designed to work seamlessly with the `ruvector-neo4j` crate for full Neo4j-compatible graph database functionality. The current implementation provides placeholder functionality that will be integrated with the actual graph database implementation.

## Common Workflows

### Building a Social Network Graph

```bash
# Create database
ruvector graph create --path social.db --name social --indexed

# Start shell
ruvector graph shell --db social.db

# In the shell:
CREATE (alice:Person {name: 'Alice', age: 30})
CREATE (bob:Person {name: 'Bob', age: 25})
CREATE (carol:Person {name: 'Carol', age: 28})
MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) CREATE (a)-[:KNOWS {since: 2020}]->(b)
MATCH (b:Person {name: 'Bob'}), (c:Person {name: 'Carol'}) CREATE (b)-[:KNOWS {since: 2021}]->(c)

# Find friends of friends
MATCH (a:Person {name: 'Alice'})-[:KNOWS*2..3]-(fof) RETURN DISTINCT fof.name
```

### Import and Export

```bash
# Import from JSON
ruvector graph import -b mydb.db -i data.json --format json

# Export to Cypher for backup
ruvector graph export -b mydb.db -o backup.cypher --format cypher

# Export to GraphML for visualization
ruvector graph export -b mydb.db -o graph.graphml --format graphml
```

### Performance Testing

```bash
# Run traversal benchmark
ruvector graph benchmark -b mydb.db -n 10000 -t traverse

# Run pattern matching benchmark
ruvector graph benchmark -b mydb.db -n 5000 -t pattern
```

## Global Options

All graph commands support these global options (inherited from main CLI):

- `--config, -c` - Configuration file path
- `--debug, -d` - Enable debug mode
- `--no-color` - Disable colored output

## See Also

- [Main CLI Documentation](./cli-usage.md)
- [Vector Database Commands](./cli-vector-commands.md)
- [Configuration Guide](./configuration.md)
- [RuVector Neo4j Documentation](./neo4j-integration.md)
