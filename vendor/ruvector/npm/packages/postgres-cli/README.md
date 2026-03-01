# @ruvector/postgres-cli

[![npm version](https://img.shields.io/npm/v/@ruvector/postgres-cli.svg)](https://www.npmjs.com/package/@ruvector/postgres-cli)
[![npm downloads](https://img.shields.io/npm/dm/@ruvector/postgres-cli.svg)](https://www.npmjs.com/package/@ruvector/postgres-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Node.js](https://img.shields.io/badge/Node.js-18+-green.svg)](https://nodejs.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-14--17-blue.svg)](https://www.postgresql.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.x-blue.svg)](https://www.typescriptlang.org/)
[![Docker](https://img.shields.io/badge/Docker-available-blue.svg)](https://hub.docker.com/r/ruvnet/ruvector-postgres)

**The most advanced AI vector database CLI for PostgreSQL.** A drop-in pgvector replacement with 53+ SQL functions, 39 attention mechanisms, GNN layers, hyperbolic embeddings, and self-learning capabilities.

## Quick Start (One Command Install)

```bash
# Install everything (PostgreSQL + RuVector extension) in one command
npx @ruvector/postgres-cli install

# Or install natively without Docker
npx @ruvector/postgres-cli install --method native
```

That's it! The installer will:
1. Detect your system (Linux/macOS)
2. Install PostgreSQL if not present
3. Install Rust if not present (for native installs)
4. Build and install the RuVector extension
5. Create a ready-to-use database

## Supported Environments

| Platform | Architecture | Docker | Native | Package Manager |
|----------|-------------|--------|--------|-----------------|
| **Ubuntu/Debian** | x64, arm64 | ✅ | ✅ | apt |
| **RHEL/CentOS/Fedora** | x64, arm64 | ✅ | ✅ | dnf/yum |
| **Arch Linux** | x64 | ✅ | ✅ | pacman |
| **macOS** | Intel, Apple Silicon | ✅ | ✅ | Homebrew |
| **Windows** | x64 | ✅ (WSL2) | ❌ | - |

**PostgreSQL Versions**: 14, 15, 16, 17 (native), Docker supports all

## Why RuVector?

| Feature | pgvector | RuVector |
|---------|----------|----------|
| Vector Search | HNSW, IVFFlat | HNSW, IVFFlat |
| Distance Metrics | 3 | 8+ (including hyperbolic) |
| Attention Mechanisms | - | 39 types |
| Graph Neural Networks | - | GCN, GraphSAGE, GAT |
| Hyperbolic Embeddings | - | Poincare, Lorentz |
| Sparse Vectors / BM25 | - | Full support |
| Self-Learning | - | ReasoningBank |
| Agent Routing | - | Tiny Dancer |

## Installation Options

### Option 1: Docker (Fastest - Recommended)

```bash
# Auto-detect and install via Docker
npx @ruvector/postgres-cli install

# Or explicitly use Docker
npx @ruvector/postgres-cli install --method docker

# Customize port and credentials
npx @ruvector/postgres-cli install \
  --port 5433 \
  --user myuser \
  --password mypass \
  --database mydb
```

**Direct Docker Hub Usage** (without CLI):

```bash
# Pull and run from Docker Hub
docker run -d --name ruvector-pg \
  -e POSTGRES_PASSWORD=secret \
  -p 5432:5432 \
  ruvnet/ruvector-postgres:latest

# Connect with psql
docker exec -it ruvector-pg psql -U postgres
```

### Option 2: Native Installation (No Docker Required)

```bash
# Full native installation - installs everything
npx @ruvector/postgres-cli install --method native

# Specify PostgreSQL version
npx @ruvector/postgres-cli install --method native --pg-version 17

# Use existing PostgreSQL (skip PostgreSQL install)
npx @ruvector/postgres-cli install --method native --skip-postgres

# Use existing Rust (skip Rust install)
npx @ruvector/postgres-cli install --method native --skip-rust
```

### Option 3: Global CLI

```bash
# Install globally
npm install -g @ruvector/postgres-cli

# Then use anywhere
ruvector-pg install
ruvector-pg info
ruvector-pg vector create embeddings --dim 384
```

## Install Command Options

```bash
npx @ruvector/postgres-cli install [options]

Options:
  -m, --method <type>     Installation method: docker, native, auto (default: "auto")
  -p, --port <number>     PostgreSQL port (default: "5432")
  -u, --user <name>       Database user (default: "ruvector")
  --password <pass>       Database password (default: "ruvector")
  -d, --database <name>   Database name (default: "ruvector")
  --data-dir <path>       Persistent data directory (Docker only)
  --name <name>           Container name (default: "ruvector-postgres")
  --version <version>     RuVector version (default: "0.2.5")
  --pg-version <version>  PostgreSQL version for native (14, 15, 16, 17) (default: "16")
  --skip-postgres         Skip PostgreSQL installation (use existing)
  --skip-rust             Skip Rust installation (use existing)
```

## Server Management

```bash
# Check installation status
npx @ruvector/postgres-cli status

# Start/Stop the server
npx @ruvector/postgres-cli start
npx @ruvector/postgres-cli stop

# View logs
npx @ruvector/postgres-cli logs
npx @ruvector/postgres-cli logs --follow

# Connect with psql
npx @ruvector/postgres-cli psql
npx @ruvector/postgres-cli psql "SELECT ruvector_version();"

# Uninstall
npx @ruvector/postgres-cli uninstall
```

## Tutorial 1: Semantic Search in 5 Minutes

### Step 1: Install & Connect

```bash
# Install RuVector PostgreSQL
npx @ruvector/postgres-cli install

# Verify installation
npx @ruvector/postgres-cli info
```

### Step 2: Create Vector Table

```bash
# Create table with 384-dimensional vectors and HNSW index
npx @ruvector/postgres-cli vector create documents --dim 384 --index hnsw
```

### Step 3: Insert Vectors

```bash
# Insert from JSON file
echo '[
  {"vector": [0.1, 0.2, 0.3], "metadata": {"title": "AI Overview"}},
  {"vector": [0.4, 0.5, 0.6], "metadata": {"title": "ML Basics"}}
]' > docs.json

npx @ruvector/postgres-cli vector insert documents --file docs.json
```

### Step 4: Search

```bash
# Find similar documents
npx @ruvector/postgres-cli vector search documents \
  --query "[0.15, 0.25, 0.35]" \
  --top-k 5 \
  --metric cosine
```

## Tutorial 2: Hybrid Search with BM25

Combine semantic vectors with keyword search:

```bash
# Create sparse vector for text matching
npx @ruvector/postgres-cli sparse create \
  --indices "[0, 5, 10]" \
  --values "[0.5, 0.3, 0.2]" \
  --dim 10000

# Compute BM25 score
npx @ruvector/postgres-cli sparse bm25 \
  --query '{"indices": [1,5,10], "values": [0.8,0.5,0.3]}' \
  --doc '{"indices": [1,5], "values": [2,1]}' \
  --doc-len 150 \
  --avg-doc-len 200
```

## Tutorial 3: Knowledge Graph with Hyperbolic Embeddings

Perfect for hierarchical data like taxonomies:

```bash
# Create a graph
npx @ruvector/postgres-cli graph create taxonomy

# Add nodes
npx @ruvector/postgres-cli graph create-node taxonomy \
  --labels "Category" \
  --properties '{"name": "Science"}'

npx @ruvector/postgres-cli graph create-node taxonomy \
  --labels "Category" \
  --properties '{"name": "Physics"}'

# Add edge
npx @ruvector/postgres-cli graph create-edge taxonomy \
  --from 1 --to 2 --type "SUBCATEGORY"

# Compute hyperbolic distance (better for hierarchies)
npx @ruvector/postgres-cli hyperbolic poincare-distance \
  --a "[0.1, 0.2]" \
  --b "[0.3, 0.4]" \
  --curvature -1.0
```

## Tutorial 4: Self-Learning Search

Enable the system to learn from user feedback:

```bash
# Enable learning for a table
npx @ruvector/postgres-cli learning enable documents \
  --max-trajectories 1000 \
  --num-clusters 10

# Record search trajectory
npx @ruvector/postgres-cli learning record \
  --input "[0.1, 0.2, ...]" \
  --output "[0.3, 0.4, ...]" \
  --success true

# Get optimized search parameters
npx @ruvector/postgres-cli learning get-params documents \
  --query "[0.15, 0.25, ...]"

# View learning statistics
npx @ruvector/postgres-cli learning stats documents
```

## Commands Reference

### Vector Operations
```bash
ruvector-pg vector create <table> --dim <n> --index <hnsw|ivfflat>
ruvector-pg vector insert <table> --file data.json
ruvector-pg vector search <table> --query "[...]" --top-k 10 --metric cosine
ruvector-pg vector distance --a "[...]" --b "[...]" --metric <cosine|l2|ip>
ruvector-pg vector normalize --vector "[0.5, 0.3, 0.2]"
```

### Attention Mechanisms (39 types)
```bash
ruvector-pg attention compute --query "[...]" --keys "[[...]]" --values "[[...]]" --type scaled_dot
ruvector-pg attention list-types
```

### Graph Neural Networks
```bash
ruvector-pg gnn create <name> --type gcn --input-dim 384 --output-dim 128
ruvector-pg gnn forward <layer> --features features.json --edges edges.json
```

### Hyperbolic Geometry
```bash
ruvector-pg hyperbolic poincare-distance --a "[...]" --b "[...]"
ruvector-pg hyperbolic lorentz-distance --a "[...]" --b "[...]"
ruvector-pg hyperbolic mobius-add --a "[...]" --b "[...]"
ruvector-pg hyperbolic exp-map --base "[...]" --tangent "[...]"
ruvector-pg hyperbolic poincare-to-lorentz --vector "[...]"
```

### Sparse Vectors & BM25
```bash
ruvector-pg sparse create --indices "[...]" --values "[...]" --dim 10000
ruvector-pg sparse bm25 --query "..." --doc "..." --doc-len 150 --avg-doc-len 200
ruvector-pg sparse distance --a "..." --b "..." --metric cosine
```

### Agent Routing (Tiny Dancer)
```bash
ruvector-pg routing register --name "agent1" --type llm --capabilities "..." --cost 0.01 --latency 100 --quality 0.9
ruvector-pg routing route --embedding "[...]" --optimize-for balanced
ruvector-pg routing list
```

### Quantization
```bash
ruvector-pg quantization binary --vector "[...]"
ruvector-pg quantization scalar --vector "[...]"
ruvector-pg quantization compare "[0.1, 0.2, ...]"
```

### Benchmarking
```bash
ruvector-pg bench run --type all --size 10000 --dim 384
ruvector-pg bench report --format table
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    @ruvector/postgres-cli                          │
├─────────────────────────────────────────────────────────────────────┤
│  Installation Layer                                                 │
│    ├── Docker      - Pull/build image, run container              │
│    └── Native      - Install PG + Rust + pgrx + extension         │
├─────────────────────────────────────────────────────────────────────┤
│  CLI Layer (Commander.js)                                          │
│    ├── install/status/start/stop/logs - Server management         │
│    ├── vector    - CRUD & search operations                        │
│    ├── attention - 39 attention mechanism types                    │
│    ├── gnn       - Graph Neural Network layers                     │
│    ├── graph     - Cypher queries & traversal                      │
│    ├── hyperbolic- Poincare/Lorentz embeddings                     │
│    ├── sparse    - BM25/SPLADE scoring                             │
│    ├── routing   - Tiny Dancer agent routing                       │
│    ├── learning  - ReasoningBank self-learning                     │
│    └── bench     - Performance benchmarking                        │
├─────────────────────────────────────────────────────────────────────┤
│  Client Layer (pg with connection pooling)                         │
│    ├── Connection pooling (max 10, idle timeout 30s)               │
│    ├── Automatic retry (3 attempts, exponential backoff)           │
│    └── SQL injection protection                                    │
├─────────────────────────────────────────────────────────────────────┤
│  PostgreSQL Extension (ruvector-postgres 0.2.5)                    │
│    └── 53+ SQL functions exposed via pgrx                          │
└─────────────────────────────────────────────────────────────────────┘
```

## Benchmarks

Performance measured on AMD EPYC 7763 (64 cores), 256GB RAM:

| Operation | 10K vectors | 100K vectors | 1M vectors |
|-----------|-------------|--------------|------------|
| HNSW Build | 0.8s | 8.2s | 95s |
| HNSW Search (top-10) | 0.3ms | 0.5ms | 1.2ms |
| Cosine Distance | 0.01ms | 0.01ms | 0.01ms |
| Poincare Distance | 0.02ms | 0.02ms | 0.02ms |
| GCN Forward | 2.1ms | 18ms | 180ms |
| BM25 Score | 0.05ms | 0.08ms | 0.15ms |

*Dimensions: 384 for vector ops, 128 for GNN*

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | `postgresql://localhost:5432/ruvector` |
| `RUVECTOR_POOL_SIZE` | Connection pool size | `10` |
| `RUVECTOR_TIMEOUT` | Query timeout (ms) | `30000` |
| `RUVECTOR_RETRIES` | Max retry attempts | `3` |

## Troubleshooting

### Docker Issues

```bash
# Check if Docker is running
docker info

# View container logs
npx @ruvector/postgres-cli logs

# Restart container
npx @ruvector/postgres-cli stop && npx @ruvector/postgres-cli start
```

### Native Installation Issues

```bash
# Check PostgreSQL is running
sudo systemctl status postgresql

# Check pgrx installation
cargo pgrx --version

# Reinstall extension
npx @ruvector/postgres-cli install --method native --skip-postgres --skip-rust
```

### Permission Issues

```bash
# Native install may require sudo for PostgreSQL operations
# The installer will prompt for sudo password when needed
```

## Related Packages

- [`ruvector-postgres`](https://crates.io/crates/ruvector-postgres) - Rust PostgreSQL extension (v0.2.5)
- [`ruvector-core`](https://crates.io/crates/ruvector-core) - Core vector operations library

## Contributing

Contributions welcome! See [CONTRIBUTING.md](https://github.com/ruvnet/ruvector/blob/main/CONTRIBUTING.md).

## License

MIT - see [LICENSE](https://github.com/ruvnet/ruvector/blob/main/LICENSE)
