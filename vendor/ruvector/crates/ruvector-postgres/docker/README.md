# RuVector-Postgres v2 Docker

High-performance PostgreSQL vector database extension with 230+ SQL functions, SIMD acceleration, Flash Attention, GNN layers, hybrid search, multi-tenancy, and self-healing capabilities.

## Docker Hub

```bash
# Pull the latest image
docker pull ruvector/ruvector-postgres:latest

# Or specific PostgreSQL version
docker pull ruvector/ruvector-postgres:2.0.0-pg17
docker pull ruvector/ruvector-postgres:2.0.0-pg16
docker pull ruvector/ruvector-postgres:2.0.0-pg15
docker pull ruvector/ruvector-postgres:2.0.0-pg14

# Run container
docker run -d \
  --name ruvector \
  -p 5432:5432 \
  -e POSTGRES_PASSWORD=secret \
  ruvector/ruvector-postgres:latest

# Connect and test
psql -h localhost -U ruvector -d ruvector_test -c "SELECT ruvector_version();"
```

## v2 Features

| Feature | Description | SQL Functions |
|---------|-------------|---------------|
| **Core Vectors** | pgvector-compatible vector type | `vector`, `<->`, `<=>`, `<#>` |
| **SIMD** | AVX2/AVX512/NEON acceleration | `ruvector_simd_info()` |
| **HNSW Index** | Approximate nearest neighbor | `CREATE INDEX ... USING hnsw` |
| **IVFFlat Index** | Inverted file index | `CREATE INDEX ... USING ivfflat` |
| **Quantization** | Binary, scalar, product quantization | 6 functions |
| **Flash Attention** | Memory-efficient attention | `ruvector_flash_attention()` |
| **Multi-Head Attention** | Transformer attention | `ruvector_multi_head_attention()` |
| **GNN Layers** | GCN, GraphSAGE, GAT | 5+ functions |
| **Hybrid Search** | BM25 + vector fusion | 7 functions |
| **Multi-Tenancy** | Tenant isolation, quotas | 17 functions |
| **Self-Healing** | Automatic recovery | 23 functions |
| **Self-Learning** | Adaptive optimization | 10 functions |
| **Hyperbolic** | Poincaré/Lorentz embeddings | 8+ functions |
| **Graph** | Cypher-style queries | 25+ functions |

## Quick Start

### Development Environment

```bash
# Start development environment
./dev.sh start

# Open psql shell
./dev.sh psql

# Watch for changes and auto-reload
./dev.sh watch

# Stop environment
./dev.sh stop
```

### Running Tests

```bash
# Run full test suite
./run-tests.sh

# Run integration tests only
./run-tests.sh --integration

# Keep container running for debugging
./run-tests.sh --keep-running

# Clean rebuild
./run-tests.sh --clean
```

## Scripts Overview

### `dev.sh` - Development Environment

Manages a PostgreSQL development environment with hot-reload support.

**Commands:**
- `start` - Start development environment (default)
- `stop` - Stop development environment
- `restart` - Restart development environment
- `logs` - Show PostgreSQL logs
- `psql` - Open psql shell
- `watch` - Start file watcher for hot-reload (requires cargo-watch)
- `rebuild` - Rebuild and reload extension
- `status` - Show container status

**Options:**
- `-p, --port PORT` - PostgreSQL port (default: 5432)
- `-u, --user USER` - PostgreSQL user (default: postgres)
- `-d, --database DB` - PostgreSQL database (default: ruvector_dev)
- `-f, --foreground` - Start in foreground with logs
- `-h, --help` - Show help message

**Examples:**
```bash
# Start on custom port
./dev.sh --port 5433 start

# View logs
./dev.sh logs

# Rebuild extension
./dev.sh rebuild
```

### `run-tests.sh` - Test Runner

Builds Docker image, runs tests, and manages test infrastructure.

**Options:**
- `-b, --build-only` - Build Docker image only, don't run tests
- `-t, --test-only` - Run tests only (skip build)
- `-i, --integration` - Run integration tests only
- `-k, --keep-running` - Keep container running after tests
- `-c, --clean` - Clean up before starting
- `-v, --keep-volumes` - Keep volumes after cleanup
- `-p, --port PORT` - PostgreSQL port (default: 5433)
- `-h, --help` - Show help message

**Examples:**
```bash
# Build and test
./run-tests.sh

# Integration tests with container kept running
./run-tests.sh --integration --keep-running

# Clean rebuild
./run-tests.sh --clean --build-only
```

## Docker Files

### `Dockerfile` - Main Build File

Multi-stage Docker build for PostgreSQL 16 with pgrx 0.12.6 support.

**Features:**
- Rust 1.75 with Bookworm base
- PostgreSQL 16 with development headers
- cargo-pgrx 0.12.6 pre-installed
- Optimized layer caching for dependencies
- Health checks built-in

### `docker-compose.yml` - Orchestration

Complete development stack with PostgreSQL and pgAdmin.

**Services:**
- `postgres` - PostgreSQL 16 with ruvector extension
- `pgadmin` - Web-based database management (port 5050)

**Usage:**
```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f

# Stop services
docker-compose down

# Access pgAdmin
# URL: http://localhost:5050
# Email: admin@ruvector.dev
# Password: admin
```

### `init.sql` - Database Initialization

SQL script for automatic database setup with:
- Extension creation
- Sample tables and indexes
- Test data
- Performance monitoring views

## Development Workflow

### 1. Initial Setup

```bash
# Start development environment
./dev.sh start

# This will:
# - Pull PostgreSQL 16 image
# - Create development database
# - Expose on localhost:5432
# - Show connection string
```

### 2. Build Extension

```bash
cd /workspaces/ruvector/crates/ruvector-postgres

# Build and install extension
cargo pgrx install --release
```

### 3. Test Changes

```bash
# Quick test in psql
./dev.sh psql

# In psql:
# CREATE EXTENSION ruvector_postgres;
# SELECT '[1,2,3]'::vector;
```

### 4. Hot-Reload Development

```bash
# Install cargo-watch (one time)
cargo install cargo-watch

# Start watching for changes
./dev.sh watch

# Now edit code - extension auto-reloads on save!
```

### 5. Run Full Test Suite

```bash
# Run all tests
./run-tests.sh

# Or run just integration tests
./run-tests.sh --integration
```

## Environment Variables

### Development (`dev.sh`)

```bash
POSTGRES_PORT=5432          # PostgreSQL port
POSTGRES_USER=postgres      # PostgreSQL user
POSTGRES_PASSWORD=postgres  # PostgreSQL password
POSTGRES_DB=ruvector_dev    # Database name
```

### Testing (`run-tests.sh`)

```bash
POSTGRES_PORT=5433          # PostgreSQL port (different from dev)
POSTGRES_USER=ruvector      # PostgreSQL user
POSTGRES_PASSWORD=ruvector  # PostgreSQL password
POSTGRES_DB=ruvector_test   # Test database name
KEEP_VOLUMES=false          # Keep volumes after cleanup
EXPORT_DB=false             # Export database dump
```

## Platform Support

Both scripts support:
- ✅ Linux (Ubuntu, Debian, RHEL, etc.)
- ✅ macOS (Intel and Apple Silicon)
- ✅ Windows (via WSL2)

The scripts automatically detect the platform and adjust behavior accordingly.

## Troubleshooting

### Port Already in Use

```bash
# Check what's using the port
lsof -i :5432

# Use a different port
./dev.sh --port 5433 start
```

### Extension Not Loading

```bash
# Rebuild extension
./dev.sh rebuild

# Or manually:
cd /workspaces/ruvector/crates/ruvector-postgres
cargo pgrx install --release

# Then reload in database
./dev.sh psql
# DROP EXTENSION ruvector_postgres CASCADE;
# CREATE EXTENSION ruvector_postgres;
```

### Docker Build Fails

```bash
# Clean build
docker system prune -a
./run-tests.sh --clean --build-only

# Check Docker resources
docker info
```

### Tests Fail

```bash
# Keep container running to debug
./run-tests.sh --keep-running

# Connect to inspect
./dev.sh psql

# View logs
docker logs ruvector-postgres-test
```

## Performance Tips

### Build Optimization

```bash
# Use BuildKit for faster builds
export DOCKER_BUILDKIT=1
./run-tests.sh

# Parallel builds
docker build --build-arg MAKEFLAGS="-j$(nproc)" ...
```

### Development Speed

```bash
# Use cargo-watch for instant feedback
./dev.sh watch

# Or use cargo-pgrx run for interactive development
cd /workspaces/ruvector/crates/ruvector-postgres
cargo pgrx run pg16
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Test RuVector-Postgres

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run tests
        run: |
          cd crates/ruvector-postgres/docker
          ./run-tests.sh
```

### GitLab CI Example

```yaml
test:
  image: docker:latest
  services:
    - docker:dind
  script:
    - cd crates/ruvector-postgres/docker
    - ./run-tests.sh
```

## Resources

- [pgrx Documentation](https://github.com/pgcentralfoundation/pgrx)
- [PostgreSQL Docker Hub](https://hub.docker.com/_/postgres)
- [RuVector Repository](https://github.com/ruvnet/ruvector)

## License

MIT License - See project root for details
