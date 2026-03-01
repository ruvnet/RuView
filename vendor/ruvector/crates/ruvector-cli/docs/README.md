# Ruvector CLI and MCP Server

High-performance command-line interface and Model Context Protocol (MCP) server for Ruvector vector database.

## Table of Contents

- [Installation](#installation)
- [CLI Usage](#cli-usage)
- [MCP Server](#mcp-server)
- [Configuration](#configuration)
- [Examples](#examples)
- [Shell Completions](#shell-completions)

## Installation

```bash
# Build from source
cargo build --release -p ruvector-cli

# Install binaries
cargo install --path crates/ruvector-cli

# The following binaries will be available:
# - ruvector (CLI tool)
# - ruvector-mcp (MCP server)
```

## CLI Usage

### Create a Database

```bash
# Create with specific dimensions
ruvector create --path ./my-vectors.db --dimensions 384

# Use default location (./ruvector.db)
ruvector create --dimensions 1536
```

### Insert Vectors

```bash
# From JSON file
ruvector insert --db ./my-vectors.db --input vectors.json --format json

# From CSV file
ruvector insert --db ./my-vectors.db --input vectors.csv --format csv

# From NumPy file
ruvector insert --db ./my-vectors.db --input embeddings.npy --format npy

# Hide progress bar
ruvector insert --db ./my-vectors.db --input vectors.json --no-progress
```

#### Input Format Examples

**JSON format:**
```json
[
  {
    "id": "doc1",
    "vector": [0.1, 0.2, 0.3, ...],
    "metadata": {
      "title": "Document 1",
      "category": "science"
    }
  },
  {
    "id": "doc2",
    "vector": [0.4, 0.5, 0.6, ...],
    "metadata": {
      "title": "Document 2",
      "category": "tech"
    }
  }
]
```

**CSV format:**
```csv
id,vector,metadata
doc1,"[0.1, 0.2, 0.3]","{\"title\": \"Document 1\"}"
doc2,"[0.4, 0.5, 0.6]","{\"title\": \"Document 2\"}"
```

### Search Vectors

```bash
# Search with JSON array
ruvector search --db ./my-vectors.db --query "[0.1, 0.2, 0.3]" --top-k 10

# Search with comma-separated values
ruvector search --db ./my-vectors.db --query "0.1, 0.2, 0.3" -k 5

# Show full vectors in results
ruvector search --db ./my-vectors.db --query "[0.1, 0.2, 0.3]" --show-vectors
```

### Database Info

```bash
# Show database statistics
ruvector info --db ./my-vectors.db
```

Output example:
```
Database Statistics
  Vectors: 10000
  Dimensions: 384
  Distance Metric: Cosine

HNSW Configuration:
  M: 32
  ef_construction: 200
  ef_search: 100
```

### Benchmark Performance

```bash
# Run 1000 queries
ruvector benchmark --db ./my-vectors.db --queries 1000

# Custom number of queries
ruvector benchmark --db ./my-vectors.db -n 5000
```

Output example:
```
Running benchmark...
  Queries: 1000
  Dimensions: 384

Benchmark Results:
  Total time: 2.45s
  Queries per second: 408
  Average latency: 2.45ms
```

### Export Database

```bash
# Export to JSON
ruvector export --db ./my-vectors.db --output backup.json --format json

# Export to CSV
ruvector export --db ./my-vectors.db --output backup.csv --format csv
```

### Import from Other Databases

```bash
# Import from FAISS (coming soon)
ruvector import --db ./my-vectors.db --source faiss --source-path index.faiss

# Import from Pinecone (coming soon)
ruvector import --db ./my-vectors.db --source pinecone --source-path config.json
```

### Global Options

```bash
# Use custom config file
ruvector --config ./custom-config.toml info --db ./my-vectors.db

# Enable debug mode
ruvector --debug search --db ./my-vectors.db --query "[0.1, 0.2, 0.3]"

# Disable colors
ruvector --no-color info --db ./my-vectors.db
```

## MCP Server

The Ruvector MCP server provides programmatic access via the Model Context Protocol.

### Start Server

```bash
# STDIO transport (for local communication)
ruvector-mcp --transport stdio

# SSE transport (for HTTP streaming)
ruvector-mcp --transport sse --host 127.0.0.1 --port 3000

# With custom config
ruvector-mcp --config ./mcp-config.toml --transport sse

# Debug mode
ruvector-mcp --debug --transport stdio
```

### MCP Tools

The server exposes the following tools:

#### 1. vector_db_create

Create a new vector database.

**Parameters:**
- `path` (string, required): Database file path
- `dimensions` (integer, required): Vector dimensions
- `distance_metric` (string, optional): Distance metric (euclidean, cosine, dotproduct, manhattan)

**Example:**
```json
{
  "name": "vector_db_create",
  "arguments": {
    "path": "./my-db.db",
    "dimensions": 384,
    "distance_metric": "cosine"
  }
}
```

#### 2. vector_db_insert

Insert vectors into database.

**Parameters:**
- `db_path` (string, required): Database path
- `vectors` (array, required): Array of vector objects

**Example:**
```json
{
  "name": "vector_db_insert",
  "arguments": {
    "db_path": "./my-db.db",
    "vectors": [
      {
        "id": "vec1",
        "vector": [0.1, 0.2, 0.3],
        "metadata": {"label": "test"}
      }
    ]
  }
}
```

#### 3. vector_db_search

Search for similar vectors.

**Parameters:**
- `db_path` (string, required): Database path
- `query` (array, required): Query vector
- `k` (integer, optional, default: 10): Number of results
- `filter` (object, optional): Metadata filters

**Example:**
```json
{
  "name": "vector_db_search",
  "arguments": {
    "db_path": "./my-db.db",
    "query": [0.1, 0.2, 0.3],
    "k": 5
  }
}
```

#### 4. vector_db_stats

Get database statistics.

**Parameters:**
- `db_path` (string, required): Database path

**Example:**
```json
{
  "name": "vector_db_stats",
  "arguments": {
    "db_path": "./my-db.db"
  }
}
```

#### 5. vector_db_backup

Backup database to file.

**Parameters:**
- `db_path` (string, required): Database path
- `backup_path` (string, required): Backup file path

**Example:**
```json
{
  "name": "vector_db_backup",
  "arguments": {
    "db_path": "./my-db.db",
    "backup_path": "./backup.db"
  }
}
```

### MCP Resources

The server provides access to database resources via URIs:

- `database://local/default`: Default database resource

### MCP Prompts

Available prompt templates:

- `semantic-search`: Generate semantic search queries

## Configuration

Ruvector can be configured via TOML files, environment variables, or CLI arguments.

### Configuration File

Create a `ruvector.toml` file:

```toml
[database]
storage_path = "./ruvector.db"
dimensions = 384
distance_metric = "Cosine"

[database.hnsw]
m = 32
ef_construction = 200
ef_search = 100
max_elements = 10000000

[cli]
progress = true
colors = true
batch_size = 1000

[mcp]
host = "127.0.0.1"
port = 3000
cors = true
```

### Environment Variables

```bash
export RUVECTOR_STORAGE_PATH="./my-db.db"
export RUVECTOR_DIMENSIONS=384
export RUVECTOR_DISTANCE_METRIC="cosine"
export RUVECTOR_MCP_HOST="0.0.0.0"
export RUVECTOR_MCP_PORT=8080
```

### Configuration Precedence

1. CLI arguments (highest priority)
2. Environment variables
3. Configuration file
4. Default values (lowest priority)

### Default Config Locations

Ruvector looks for config files in these locations:

1. `./ruvector.toml`
2. `./.ruvector.toml`
3. `~/.config/ruvector/config.toml`
4. `/etc/ruvector/config.toml`

## Examples

### Building a Semantic Search Engine

```bash
# 1. Create database
ruvector create --path ./search.db --dimensions 384

# 2. Generate embeddings (external script)
python generate_embeddings.py --input documents/ --output embeddings.json

# 3. Insert embeddings
ruvector insert --db ./search.db --input embeddings.json

# 4. Search
ruvector search --db ./search.db --query "[0.1, 0.2, ...]" -k 10
```

### Batch Processing Pipeline

```bash
#!/bin/bash

DB="./vectors.db"
DIMS=768

# Create database
ruvector create --path $DB --dimensions $DIMS

# Process batches
for file in data/batch_*.json; do
  echo "Processing $file..."
  ruvector insert --db $DB --input $file --no-progress
done

# Verify
ruvector info --db $DB

# Benchmark
ruvector benchmark --db $DB --queries 1000
```

### Using with Claude Code

```bash
# Start MCP server
ruvector-mcp --transport stdio

# Claude Code can now use vector database tools
# Example prompt: "Create a vector database and insert embeddings from my documents"
```

## Shell Completions

Generate shell completions for better CLI experience:

```bash
# Bash
ruvector --generate-completions bash > ~/.local/share/bash-completion/completions/ruvector

# Zsh
ruvector --generate-completions zsh > ~/.zsh/completions/_ruvector

# Fish
ruvector --generate-completions fish > ~/.config/fish/completions/ruvector.fish
```

## Error Handling

Ruvector provides helpful error messages:

```bash
# Missing required argument
$ ruvector create
Error: Missing required argument: --dimensions

# Invalid vector dimensions
$ ruvector insert --db test.db --input vectors.json
Error: Vector dimension mismatch. Expected: 384, Got: 768
Suggestion: Ensure all vectors have the correct dimensionality

# Database not found
$ ruvector info --db nonexistent.db
Error: Failed to open database: No such file or directory
Suggestion: Create the database first with: ruvector create --path nonexistent.db --dimensions <dims>

# Use --debug for full stack traces
$ ruvector --debug info --db nonexistent.db
```

## Performance Tips

1. **Batch Inserts**: Insert vectors in batches for better performance
2. **HNSW Tuning**: Adjust `ef_construction` and `ef_search` based on your accuracy/speed requirements
3. **Quantization**: Enable quantization for memory-constrained environments
4. **Dimensions**: Use appropriate dimensions for your use case (384 for smaller models, 1536 for larger)
5. **Distance Metric**: Choose based on your embeddings:
   - Cosine: Normalized embeddings (most common)
   - Euclidean: Absolute distances
   - Dot Product: When magnitude matters

## Troubleshooting

### Build Issues

```bash
# Ensure Rust is up to date
rustup update

# Clean build
cargo clean && cargo build --release -p ruvector-cli
```

### Runtime Issues

```bash
# Enable debug logging
RUST_LOG=debug ruvector info --db test.db

# Check database integrity
ruvector info --db test.db

# Backup before operations
cp test.db test.db.backup
```

## Contributing

See the main Ruvector repository for contribution guidelines.

## License

MIT License - see LICENSE file for details.
