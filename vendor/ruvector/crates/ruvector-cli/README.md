# Ruvector CLI

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**Command-line interface and MCP server for high-performance vector database operations.**

> Professional CLI tools for managing Ruvector vector databases with sub-millisecond query performance, batch operations, and MCP integration.

## 🌟 Overview

The Ruvector CLI provides a comprehensive command-line interface for:

- **Database Management**: Create and configure vector databases
- **Data Operations**: Insert, search, and export vector data
- **Performance Benchmarking**: Test query performance and throughput
- **Format Support**: JSON, CSV, and NumPy array formats
- **MCP Server**: Model Context Protocol server for AI integrations
- **Batch Processing**: Efficient bulk operations with progress tracking

## ⚡ Quick Start

### Installation

Install via Cargo:

```bash
cargo install ruvector-cli
```

Or build from source:

```bash
# Clone repository
git clone https://github.com/ruvnet/ruvector.git
cd ruvector

# Build CLI
cargo build --release -p ruvector-cli

# Install locally
cargo install --path crates/ruvector-cli
```

### Basic Usage

```bash
# Create a new database
ruvector create --dimensions 384 --path ./my-vectors.db

# Insert vectors from JSON
ruvector insert --db ./my-vectors.db --input vectors.json --format json

# Search for similar vectors
ruvector search --db ./my-vectors.db --query "[0.1, 0.2, 0.3, ...]" --top-k 10

# Show database information
ruvector info --db ./my-vectors.db

# Run performance benchmark
ruvector benchmark --db ./my-vectors.db --queries 1000
```

## 📋 Command Reference

### Global Options

All commands support these global options:

```bash
-c, --config <FILE>    Configuration file path
-d, --debug            Enable debug logging
    --no-color         Disable colored output
-h, --help             Print help information
-V, --version          Print version information
```

### Commands

#### `create` - Create a New Database

Create a new vector database with specified dimensions.

```bash
ruvector create [OPTIONS] --dimensions <DIMENSIONS>

Options:
  -p, --path <PATH>             Database file path [default: ./ruvector.db]
  -d, --dimensions <DIMENSIONS> Vector dimensions (required)
```

**Examples:**

```bash
# Create database for 384-dimensional embeddings (e.g., MiniLM)
ruvector create --dimensions 384

# Create database with custom path
ruvector create --dimensions 1536 --path ./embeddings.db

# Create for large embeddings (e.g., text-embedding-3-large)
ruvector create --dimensions 3072 --path ./large-embeddings.db
```

#### `insert` - Insert Vectors from File

Bulk insert vectors from JSON, CSV, or NumPy files.

```bash
ruvector insert [OPTIONS] --input <FILE>

Options:
  -d, --db <PATH>          Database file path [default: ./ruvector.db]
  -i, --input <FILE>       Input file path (required)
  -f, --format <FORMAT>    Input format: json, csv, npy [default: json]
      --no-progress        Hide progress bar
```

**Input Formats:**

**JSON** (array of vector entries):
```json
[
  {
    "id": "doc_1",
    "vector": [0.1, 0.2, 0.3, ...],
    "metadata": {"title": "Document 1", "category": "tech"}
  },
  {
    "id": "doc_2",
    "vector": [0.4, 0.5, 0.6, ...],
    "metadata": {"title": "Document 2", "category": "science"}
  }
]
```

**CSV** (id, vector_json, metadata_json):
```csv
id,vector,metadata
doc_1,"[0.1, 0.2, 0.3]","{\"title\": \"Document 1\"}"
doc_2,"[0.4, 0.5, 0.6]","{\"title\": \"Document 2\"}"
```

**NumPy** (.npy file with 2D array):
```python
import numpy as np
vectors = np.random.randn(1000, 384).astype(np.float32)
np.save('vectors.npy', vectors)
```

**Examples:**

```bash
# Insert from JSON file
ruvector insert --input embeddings.json --format json

# Insert from CSV with progress
ruvector insert --input data.csv --format csv

# Insert from NumPy array
ruvector insert --input vectors.npy --format npy

# Batch insert without progress bar
ruvector insert --input large-dataset.json --no-progress
```

#### `search` - Search for Similar Vectors

Find k-nearest neighbors for a query vector.

```bash
ruvector search [OPTIONS] --query <VECTOR>

Options:
  -d, --db <PATH>          Database file path [default: ./ruvector.db]
  -q, --query <VECTOR>     Query vector (comma-separated or JSON array)
  -k, --top-k <K>          Number of results to return [default: 10]
      --show-vectors       Show full vectors in results
```

**Query Formats:**

```bash
# Comma-separated floats
ruvector search --query "0.1, 0.2, 0.3, 0.4, ..."

# JSON array
ruvector search --query "[0.1, 0.2, 0.3, 0.4, ...]"

# From file (using shell)
ruvector search --query "$(cat query.json)"
```

**Examples:**

```bash
# Search for top 10 similar vectors
ruvector search --query "[0.1, 0.2, 0.3, ...]" --top-k 10

# Search with full vector output
ruvector search --query "0.1, 0.2, 0.3, ..." --show-vectors

# Search for top 50 results
ruvector search --query "[0.1, 0.2, ...]" -k 50
```

**Output:**

```
🔍 Search Results (top 10)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  #1  doc_42      similarity: 0.9876
  #2  doc_128     similarity: 0.9543
  #3  doc_89      similarity: 0.9321
  ...

Search completed in 0.48ms
```

#### `info` - Show Database Information

Display database statistics and configuration.

```bash
ruvector info [OPTIONS]

Options:
  -d, --db <PATH>    Database file path [default: ./ruvector.db]
```

**Examples:**

```bash
# Show default database info
ruvector info

# Show custom database info
ruvector info --db ./embeddings.db
```

**Output:**

```
📊 Database Statistics
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Total vectors:     1,234,567
  Dimensions:        384
  Distance metric:   Cosine

HNSW Configuration:
  M:                 16
  ef_construction:   200
  ef_search:         100
```

#### `benchmark` - Run Performance Benchmark

Test query performance with random vectors.

```bash
ruvector benchmark [OPTIONS]

Options:
  -d, --db <PATH>       Database file path [default: ./ruvector.db]
  -n, --queries <N>     Number of queries to run [default: 1000]
```

**Examples:**

```bash
# Quick benchmark (1000 queries)
ruvector benchmark

# Extended benchmark (10,000 queries)
ruvector benchmark --queries 10000

# Benchmark specific database
ruvector benchmark --db ./prod.db --queries 5000
```

**Output:**

```
Running benchmark...
  Queries:     1000
  Dimensions:  384

Benchmark Results:
  Total time:           0.48s
  Queries per second:   2083
  Average latency:      0.48ms
```

#### `export` - Export Database to File

Export vector data to JSON or CSV format.

```bash
ruvector export [OPTIONS] --output <FILE>

Options:
  -d, --db <PATH>          Database file path [default: ./ruvector.db]
  -o, --output <FILE>      Output file path (required)
  -f, --format <FORMAT>    Output format: json, csv [default: json]
```

**Examples:**

```bash
# Export to JSON
ruvector export --output backup.json --format json

# Export to CSV
ruvector export --output export.csv --format csv

# Export with custom database
ruvector export --db ./prod.db --output prod-backup.json
```

> **Note**: Export functionality requires `VectorDB::all_ids()` method. This feature is planned for a future release.

#### `import` - Import from Other Vector Databases

Import vectors from external vector database formats.

```bash
ruvector import [OPTIONS] --source <TYPE> --source-path <PATH>

Options:
  -d, --db <PATH>              Database file path [default: ./ruvector.db]
  -s, --source <TYPE>          Source database type: faiss, pinecone, weaviate
  -p, --source-path <PATH>     Source file or connection path
```

**Examples:**

```bash
# Import from FAISS index
ruvector import --source faiss --source-path ./index.faiss

# Import from Pinecone export
ruvector import --source pinecone --source-path ./pinecone-export.json

# Import from Weaviate backup
ruvector import --source weaviate --source-path ./weaviate-backup.json
```

> **Note**: Import functionality for external databases is planned for future releases.

## 🔧 Configuration

### Configuration File

Create a `ruvector.toml` configuration file for default settings:

```toml
[database]
storage_path = "./ruvector.db"
dimensions = 384
distance_metric = "Cosine"  # Cosine, Euclidean, DotProduct, Manhattan

[database.hnsw]
m = 16
ef_construction = 200
ef_search = 100

[database.quantization]
type = "Scalar"  # Scalar, Product, or None

[cli]
progress = true
colors = true
batch_size = 1000

[mcp]
host = "127.0.0.1"
port = 3000
cors = true
```

### Configuration Locations

The CLI searches for configuration files in this order:

1. Path specified via `--config` flag
2. `./ruvector.toml` (current directory)
3. `./.ruvector.toml` (current directory, hidden)
4. `~/.config/ruvector/config.toml` (user config)
5. `/etc/ruvector/config.toml` (system config)

### Environment Variables

Override configuration with environment variables:

```bash
# Database settings
export RUVECTOR_STORAGE_PATH="./my-db.db"
export RUVECTOR_DIMENSIONS=384
export RUVECTOR_DISTANCE_METRIC="cosine"

# MCP server settings
export RUVECTOR_MCP_HOST="0.0.0.0"
export RUVECTOR_MCP_PORT=3000

# Run with environment overrides
ruvector info
```

## 🔌 MCP Server

The Ruvector CLI includes a **Model Context Protocol (MCP)** server for AI agent integration.

### Start MCP Server

**STDIO Transport** (for local AI tools):

```bash
ruvector-mcp --transport stdio
```

**SSE Transport** (for web-based AI tools):

```bash
ruvector-mcp --transport sse --host 0.0.0.0 --port 3000
```

**With Configuration:**

```bash
ruvector-mcp --config ./ruvector.toml --transport sse --debug
```

### MCP Integration Examples

**Claude Desktop Integration** (`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "ruvector": {
      "command": "ruvector-mcp",
      "args": ["--transport", "stdio"],
      "env": {
        "RUVECTOR_STORAGE_PATH": "/path/to/vectors.db"
      }
    }
  }
}
```

**HTTP/SSE Client:**

```javascript
const evtSource = new EventSource('http://localhost:3000/sse');

evtSource.addEventListener('message', (event) => {
  const data = JSON.parse(event.data);
  console.log('MCP Response:', data);
});

// Send search request
fetch('http://localhost:3000/mcp', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    method: 'search',
    params: {
      query: [0.1, 0.2, 0.3],
      k: 10
    }
  })
});
```

## 📊 Common Workflows

### RAG System Setup

Build a retrieval-augmented generation (RAG) system:

```bash
# 1. Create database for your embedding model
ruvector create --dimensions 384 --path ./rag-embeddings.db

# 2. Generate embeddings and save to JSON
# (Use your preferred embedding model)

# 3. Insert embeddings
ruvector insert --db ./rag-embeddings.db --input embeddings.json

# 4. Query for relevant context
ruvector search --db ./rag-embeddings.db \
  --query "[0.123, 0.456, ...]" \
  --top-k 5

# 5. Start MCP server for AI agent access
ruvector-mcp --transport stdio
```

### Semantic Search Engine

Build a semantic search system:

```bash
# Create database
ruvector create --dimensions 768 --path ./search-engine.db

# Batch insert documents
ruvector insert \
  --db ./search-engine.db \
  --input documents.json \
  --format json

# Benchmark performance
ruvector benchmark --db ./search-engine.db --queries 10000

# Search interface via MCP
ruvector-mcp --transport sse --port 8080
```

### Migration from Other Databases

Migrate from existing vector databases:

```bash
# 1. Export from source database
# (Use source database's export tools)

# 2. Create Ruvector database
ruvector create --dimensions 1536 --path ./migrated.db

# 3. Import data (planned feature)
ruvector import \
  --db ./migrated.db \
  --source pinecone \
  --source-path ./pinecone-export.json

# 4. Verify migration
ruvector info --db ./migrated.db
ruvector benchmark --db ./migrated.db
```

### Performance Testing

Test vector database performance:

```bash
# Create test database
ruvector create --dimensions 384 --path ./benchmark.db

# Generate synthetic test data
python generate_test_vectors.py --count 100000 --dims 384 --output test.npy

# Insert test data
ruvector insert --db ./benchmark.db --input test.npy --format npy

# Run comprehensive benchmark
ruvector benchmark --db ./benchmark.db --queries 10000

# Test search performance
time ruvector search --db ./benchmark.db --query "[0.1, 0.2, ...]" -k 100
```

## 🎯 Shell Completion

Generate shell completion scripts for faster command entry:

### Bash

```bash
# Generate completion script
ruvector --help > /dev/null  # Trigger clap completion
complete -C ruvector ruvector

# Or add to ~/.bashrc
echo 'complete -C ruvector ruvector' >> ~/.bashrc
```

### Zsh

```bash
# Add to ~/.zshrc
autoload -U compinit && compinit
complete -o nospace -C ruvector ruvector
```

### Fish

```bash
# Generate and save completion
ruvector --help > /dev/null
complete -c ruvector -f
```

## ⚙️ Performance Tips

### Optimize Insertion

```bash
# Use larger batch sizes for bulk inserts (set in config)
[cli]
batch_size = 10000

# Disable progress bar for maximum speed
ruvector insert --input large-file.json --no-progress
```

### Optimize Search

Configure HNSW parameters for your use case:

```toml
[database.hnsw]
# Higher M = better recall, more memory
m = 32

# Higher ef_construction = better index quality, slower builds
ef_construction = 400

# Higher ef_search = better recall, slower queries
ef_search = 200
```

### Memory Optimization

Enable quantization to reduce memory usage:

```toml
[database.quantization]
type = "Product"  # 4-8x memory reduction
```

### Benchmarking Tips

```bash
# Run warm-up queries first
ruvector search --query "[...]" -k 10
ruvector search --query "[...]" -k 10

# Then benchmark
ruvector benchmark --queries 10000

# Test different k values
for k in 10 50 100; do
  time ruvector search --query "[...]" -k $k
done
```

## 🔗 Related Documentation

- **[Rust API Reference](../../docs/api/RUST_API.md)** - Core Ruvector API
- **[Getting Started Guide](../../docs/guide/GETTING_STARTED.md)** - Complete tutorial
- **[Performance Tuning](../../docs/optimization/PERFORMANCE_TUNING_GUIDE.md)** - Optimization guide
- **[Main README](../../README.md)** - Project overview

## 🐛 Troubleshooting

### Common Issues

**Database file not found:**
```bash
# Ensure database exists
ruvector info --db ./ruvector.db

# Or create it first
ruvector create --dimensions 384 --path ./ruvector.db
```

**Dimension mismatch:**
```bash
# Error: "Vector dimension mismatch"
# Solution: Ensure all vectors match database dimensions

# Check database dimensions
ruvector info --db ./ruvector.db
```

**Invalid query format:**
```bash
# Use proper JSON or comma-separated format
ruvector search --query "[0.1, 0.2, 0.3]"  # JSON
ruvector search --query "0.1, 0.2, 0.3"    # CSV
```

**MCP server connection issues:**
```bash
# Check if port is available
lsof -i :3000

# Try different port
ruvector-mcp --transport sse --port 8080

# Enable debug logging
ruvector-mcp --transport sse --debug
```

## 🤝 Contributing

Contributions welcome! Please see the [Contributing Guidelines](../../docs/development/CONTRIBUTING.md).

### Development Setup

```bash
# Clone repository
git clone https://github.com/ruvnet/ruvector.git
cd ruvector/crates/ruvector-cli

# Run tests
cargo test

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy -- -D warnings

# Build release
cargo build --release
```

## 📜 License

MIT License - see [LICENSE](../../LICENSE) for details.

## 🙏 Acknowledgments

Built with:
- **clap** - Command-line argument parsing
- **tokio** - Async runtime
- **serde** - Serialization framework
- **indicatif** - Progress bars and spinners
- **colored** - Terminal colors

---

**Built by [rUv](https://ruv.io) • Part of the [Ruvector](https://github.com/ruvnet/ruvector) ecosystem**

[Main Documentation](../../README.md) • [API Reference](../../docs/api/RUST_API.md) • [GitHub](https://github.com/ruvnet/ruvector)
