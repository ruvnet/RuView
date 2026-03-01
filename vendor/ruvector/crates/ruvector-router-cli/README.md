# Router CLI (`ruvector`)

[![Crate](https://img.shields.io/crates/v/router-cli.svg)](https://crates.io/crates/router-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**High-performance command-line interface for the Ruvector vector database.**

> The `ruvector` CLI provides powerful tools for managing, testing, and benchmarking vector databases with sub-millisecond performance. Perfect for development, testing, and operational workflows.

## 🌟 Features

- ⚡ **Fast Operations**: Sub-millisecond vector operations with HNSW indexing
- 🔧 **Database Management**: Create, configure, and manage vector databases
- 📊 **Performance Benchmarking**: Built-in benchmarks for insert and search operations
- 📈 **Real-time Statistics**: Monitor database metrics and performance
- 🎯 **Production Ready**: Battle-tested CLI for operational workflows
- 🛠️ **Developer Friendly**: Intuitive commands with helpful output formatting

## 📦 Installation

### From Crates.io (Recommended)

```bash
cargo install router-cli
```

### From Source

```bash
# Clone the repository
git clone https://github.com/ruvnet/ruvector.git
cd ruvector

# Build and install from workspace
cargo install --path crates/router-cli
```

### Verify Installation

```bash
ruvector --help
```

## ⚡ Quick Start

### Create a Database

```bash
# Create a database with default settings (384 dimensions, cosine similarity)
ruvector create

# Create with custom configuration
ruvector create \
  --path ./my_vectors.db \
  --dimensions 768 \
  --metric cosine
```

### Insert Vectors

```bash
# Insert a single vector
ruvector insert \
  --path ./vectors.db \
  --id "doc1" \
  --vector "0.1,0.2,0.3,0.4"
```

### Search Similar Vectors

```bash
# Search for top 10 similar vectors
ruvector search \
  --path ./vectors.db \
  --vector "0.1,0.2,0.3,0.4" \
  --k 10
```

### View Statistics

```bash
# Get database statistics and metrics
ruvector stats --path ./vectors.db
```

### Run Benchmarks

```bash
# Benchmark with 1000 vectors of 384 dimensions
ruvector benchmark \
  --path ./vectors.db \
  --num-vectors 1000 \
  --dimensions 384
```

## 📚 Command Reference

### `create` - Create Vector Database

Create a new vector database with specified configuration.

**Usage:**
```bash
ruvector create [OPTIONS]
```

**Options:**
- `-p, --path <PATH>` - Database file path (default: `./vectors.db`)
- `-d, --dimensions <DIMS>` - Vector dimensions (default: `384`)
- `-m, --metric <METRIC>` - Distance metric (default: `cosine`)

**Distance Metrics:**
- `cosine` - Cosine similarity (best for normalized vectors)
- `euclidean`, `l2` - Euclidean distance
- `dot`, `dotproduct` - Dot product similarity
- `manhattan`, `l1` - Manhattan distance

**Examples:**
```bash
# Create database for sentence embeddings (384D)
ruvector create --dimensions 384 --metric cosine

# Create database for image embeddings (512D, L2 distance)
ruvector create --dimensions 512 --metric euclidean --path ./images.db

# Create database for large language model embeddings (1536D)
ruvector create --dimensions 1536 --metric cosine --path ./llm_embeddings.db
```

---

### `insert` - Insert Vector

Insert a single vector into the database.

**Usage:**
```bash
ruvector insert [OPTIONS] --id <ID> --vector <VECTOR>
```

**Options:**
- `-p, --path <PATH>` - Database file path (default: `./vectors.db`)
- `-i, --id <ID>` - Unique vector identifier (required)
- `-v, --vector <VECTOR>` - Comma-separated vector values (required)

**Examples:**
```bash
# Insert a document embedding
ruvector insert \
  --id "doc_001" \
  --vector "0.23,0.45,0.67,0.12"

# Insert into specific database
ruvector insert \
  --path ./embeddings.db \
  --id "user_profile_42" \
  --vector "0.1,0.2,0.3,0.4,0.5"
```

**Performance:**
- Typical insert latency: <1ms
- Includes HNSW index update
- Thread-safe for concurrent inserts

---

### `search` - Search Similar Vectors

Search for the most similar vectors in the database.

**Usage:**
```bash
ruvector search [OPTIONS] --vector <VECTOR>
```

**Options:**
- `-p, --path <PATH>` - Database file path (default: `./vectors.db`)
- `-v, --vector <VECTOR>` - Query vector (comma-separated values, required)
- `-k <K>` - Number of results to return (default: `10`)

**Examples:**
```bash
# Find 10 most similar vectors
ruvector search --vector "0.1,0.2,0.3,0.4" --k 10

# Find top 100 matches with specific database
ruvector search \
  --path ./my_vectors.db \
  --vector "0.5,0.3,0.1,0.7" \
  --k 100
```

**Output Format:**
```
✓ Found 10 results
  Query time: 423µs

1. doc_001 (score: 0.9823)
2. doc_045 (score: 0.9456)
3. doc_123 (score: 0.9234)
...
```

**Performance:**
- Typical query latency: <0.5ms (p50)
- HNSW-based approximate nearest neighbor search
- 95%+ recall accuracy

---

### `stats` - Database Statistics

Display comprehensive database statistics and performance metrics.

**Usage:**
```bash
ruvector stats [OPTIONS]
```

**Options:**
- `-p, --path <PATH>` - Database file path (default: `./vectors.db`)

**Example:**
```bash
ruvector stats --path ./vectors.db
```

**Output:**
```
✓ Database Statistics

  Total vectors: 50,000
  Average query latency: 423.45 μs
  QPS: 2,361.23
  Index size: 12,345,678 bytes
```

**Metrics Explained:**
- **Total vectors**: Number of vectors stored
- **Average query latency**: Mean search time in microseconds
- **QPS**: Queries per second (throughput)
- **Index size**: HNSW index size in bytes

---

### `benchmark` - Performance Benchmarking

Run comprehensive performance benchmarks for insert and search operations.

**Usage:**
```bash
ruvector benchmark [OPTIONS]
```

**Options:**
- `-p, --path <PATH>` - Database file path (default: `./vectors.db`)
- `-n, --num-vectors <N>` - Number of vectors to test (default: `1000`)
- `-d, --dimensions <DIMS>` - Vector dimensions (default: `384`)

**Examples:**
```bash
# Standard benchmark (1K vectors, 384D)
ruvector benchmark

# Large-scale benchmark (100K vectors, 768D)
ruvector benchmark \
  --num-vectors 100000 \
  --dimensions 768

# Quick test (100 vectors)
ruvector benchmark --num-vectors 100
```

**Output:**
```
→ Running benchmark...
  Vectors: 1000
  Dimensions: 384

→ Generating vectors...
→ Inserting vectors...
✓ Inserted 1000 vectors in 1.234s
  Throughput: 810 inserts/sec

→ Running search benchmark...
✓ Completed 100 queries in 42.3ms
  Average latency: 423µs
  QPS: 2,364
```

**Benchmark Process:**
1. Generates random vectors with specified dimensions
2. Measures batch insert performance
3. Runs 100 search queries
4. Reports throughput and latency metrics

---

## 🎯 Use Cases

### Development Workflows

```bash
# 1. Create database for development
ruvector create --dimensions 384 --path ./dev.db

# 2. Insert test vectors
ruvector insert --id "test1" --vector "0.1,0.2,0.3,..." --path ./dev.db

# 3. Test search functionality
ruvector search --vector "0.1,0.2,0.3,..." --k 5 --path ./dev.db

# 4. Monitor performance
ruvector stats --path ./dev.db
```

### Performance Testing

```bash
# Test different vector sizes
for dims in 128 384 768 1536; do
  echo "Testing ${dims} dimensions..."
  ruvector benchmark --dimensions $dims --num-vectors 10000
done

# Compare distance metrics
for metric in cosine euclidean dot manhattan; do
  ruvector create --metric $metric --path ./test_${metric}.db
  ruvector benchmark --path ./test_${metric}.db
done
```

### Production Operations

```bash
# Check production database health
ruvector stats --path /var/lib/vectors/prod.db

# Benchmark production-scale data
ruvector benchmark \
  --path /var/lib/vectors/prod.db \
  --num-vectors 1000000 \
  --dimensions 1536

# Verify search performance
ruvector search \
  --path /var/lib/vectors/prod.db \
  --vector "$(cat query_vector.txt)" \
  --k 100
```

## 🔧 Configuration

### Database Configuration

The CLI uses the `router-core` configuration system with the following defaults:

```rust
VectorDbConfig {
    dimensions: 384,              // Vector dimensions
    max_elements: 1_000_000,      // Maximum vectors
    distance_metric: Cosine,      // Distance metric
    hnsw_m: 32,                   // HNSW connections per node
    hnsw_ef_construction: 200,    // HNSW build-time parameter
    hnsw_ef_search: 100,          // HNSW search-time parameter
    quantization: None,           // Quantization type
    storage_path: "./vectors.db", // Database path
    mmap_vectors: true,           // Enable memory mapping
}
```

### HNSW Parameters

**M (connections per node):**
- Lower values (16): Less memory, slower search
- Higher values (64): More memory, faster search
- Default: 32 (balanced)

**ef_construction:**
- Build-time quality parameter
- Higher = better index quality, slower construction
- Default: 200

**ef_search:**
- Search-time accuracy parameter
- Higher = better recall, slower search
- Default: 100

### Distance Metrics

Choose based on your data characteristics:

| Metric | Best For | Normalization Required |
|--------|----------|----------------------|
| **Cosine** | Text embeddings, semantic search | Yes (recommended) |
| **Euclidean** | Image embeddings, spatial data | No |
| **Dot Product** | Pre-normalized vectors | Yes (required) |
| **Manhattan** | High-dimensional sparse data | No |

## 📊 Performance Tuning

### Optimize for Speed

```bash
# Use dot product for pre-normalized vectors (fastest)
ruvector create --metric dot --dimensions 384

# Reduce ef_search for faster queries (lower recall)
# Note: Currently requires code modification
```

### Optimize for Accuracy

```bash
# Use higher dimensions for better semantic separation
ruvector create --dimensions 1536

# Use cosine similarity for normalized embeddings
ruvector create --metric cosine
```

### Optimize for Memory

```bash
# Use lower dimensions
ruvector create --dimensions 128

# Consider quantization (requires code configuration)
# Product quantization: 4-8x memory reduction
# Scalar quantization: 4x memory reduction
```

## 🔗 Integration with Router Core

The CLI is built on `router-core` and provides access to its features:

### Core Features

- **HNSW Indexing**: Fast approximate nearest neighbor search
- **Multiple Distance Metrics**: Cosine, Euclidean, Dot Product, Manhattan
- **SIMD Optimization**: Hardware-accelerated vector operations
- **Memory Mapping**: Efficient large-scale data handling
- **Thread Safety**: Concurrent operations support
- **Persistent Storage**: Durable vector storage with redb

### API Compatibility

The CLI uses the same `VectorDB` API available in Rust applications:

```rust
use router_core::{VectorDB, VectorEntry, SearchQuery};

// Same underlying implementation as CLI
let db = VectorDB::builder()
    .dimensions(384)
    .storage_path("./vectors.db")
    .build()?;
```

## 🐛 Troubleshooting

### Common Issues

**Database not found:**
```bash
# Ensure you've created the database first
ruvector create --path ./vectors.db

# Or specify the correct path
ruvector search --path ./correct/path/vectors.db --vector "..."
```

**Dimension mismatch:**
```bash
# Error: Expected 384 dimensions, got 768

# Solution: Use consistent dimensions
ruvector create --dimensions 768
ruvector insert --vector "..." --dimensions 768
```

**Parse errors:**
```bash
# Ensure vector values are comma-separated floats
ruvector insert --vector "0.1,0.2,0.3" --id "test"

# Not: "0.1 0.2 0.3" or "[0.1,0.2,0.3]"
```

### Performance Issues

**Slow inserts:**
- Use batch insert operations in your application code
- Reduce `hnsw_ef_construction` for faster builds
- Consider quantization for very large datasets

**Slow searches:**
- Reduce `k` (number of results)
- Reduce `ef_search` parameter (requires code modification)
- Ensure proper distance metric for your data

## 📖 Examples

### RAG System Vector Database

```bash
# Create database for document embeddings
ruvector create \
  --dimensions 384 \
  --metric cosine \
  --path ./documents.db

# Insert document embeddings (from your application)
# Typically done via Rust/Node.js API, not CLI

# Search for relevant documents
ruvector search \
  --path ./documents.db \
  --vector "$(cat query_embedding.txt)" \
  --k 5
```

### Semantic Search Testing

```bash
# Create test database
ruvector create --dimensions 768 --path ./semantic.db

# Run benchmark to establish baseline
ruvector benchmark \
  --path ./semantic.db \
  --num-vectors 10000 \
  --dimensions 768

# Test search with different query vectors
for query in query_*.txt; do
  echo "Testing $query..."
  ruvector search \
    --path ./semantic.db \
    --vector "$(cat $query)" \
    --k 10
done
```

### Performance Comparison

```bash
# Compare metrics
metrics=("cosine" "euclidean" "dot" "manhattan")

for metric in "${metrics[@]}"; do
  echo "=== Testing $metric ==="
  ruvector create --metric $metric --path ./test_$metric.db
  ruvector benchmark --path ./test_$metric.db --num-vectors 5000
  echo ""
done
```

## 🔗 Related Documentation

### Ruvector Core Documentation
- [Ruvector Main README](../../README.md) - Complete project overview
- [Router Core API](../router-core/README.md) - Core library documentation
- [Rust API Reference](../../docs/api/RUST_API.md) - Detailed API docs
- [Performance Tuning](../../docs/optimization/PERFORMANCE_TUNING_GUIDE.md) - Optimization guide

### Getting Started
- [Quick Start Guide](../../docs/guide/GETTING_STARTED.md) - 5-minute tutorial
- [Installation Guide](../../docs/guide/INSTALLATION.md) - Detailed setup
- [Basic Tutorial](../../docs/guide/BASIC_TUTORIAL.md) - Step-by-step guide

### Advanced Topics
- [Advanced Features](../../docs/guide/ADVANCED_FEATURES.md) - Quantization, indexing
- [Benchmarking Guide](../../docs/benchmarks/BENCHMARKING_GUIDE.md) - Performance testing
- [Build Optimization](../../docs/optimization/BUILD_OPTIMIZATION.md) - Compilation tips

## 🤝 Contributing

We welcome contributions! Here's how to contribute to the router-cli:

1. **Fork** the repository at [github.com/ruvnet/ruvector](https://github.com/ruvnet/ruvector)
2. **Create** a feature branch (`git checkout -b feature/cli-improvement`)
3. **Make** your changes to `crates/router-cli/`
4. **Test** thoroughly:
   ```bash
   cd crates/router-cli
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ```
5. **Build** and test the binary:
   ```bash
   cargo build --release
   ./target/release/ruvector --help
   ```
6. **Commit** your changes (`git commit -m 'Add amazing CLI feature'`)
7. **Push** to the branch (`git push origin feature/cli-improvement`)
8. **Open** a Pull Request

### Development Setup

```bash
# Clone and navigate to CLI crate
git clone https://github.com/ruvnet/ruvector.git
cd ruvector/crates/router-cli

# Build in development mode
cargo build

# Run with cargo
cargo run -- create --dimensions 384

# Run tests
cargo test

# Run with detailed logging
RUST_LOG=debug cargo run -- benchmark
```

## 📜 License

**MIT License** - see [LICENSE](../../LICENSE) for details.

Part of the Ruvector project by [rUv](https://ruv.io).

## 🙏 Acknowledgments

Built with:
- **clap** - Command-line argument parsing
- **colored** - Terminal color output
- **router-core** - Vector database engine
- **chrono** - Timestamp handling

---

<div align="center">

**Built by [rUv](https://ruv.io) • Part of [Ruvector](https://github.com/ruvnet/ruvector)**

[![GitHub](https://img.shields.io/badge/GitHub-ruvnet/ruvector-blue.svg)](https://github.com/ruvnet/ruvector)
[![Documentation](https://img.shields.io/badge/docs-README-green.svg)](../../README.md)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

[Main Documentation](../../README.md) • [API Reference](../../docs/api/RUST_API.md) • [Contributing](../../docs/development/CONTRIBUTING.md)

</div>
