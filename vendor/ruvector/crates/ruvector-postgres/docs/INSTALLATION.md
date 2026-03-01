# RuVector-Postgres Installation Guide

## Overview

This guide covers installation of RuVector-Postgres on various platforms including standard PostgreSQL, Neon, Supabase, and containerized environments.

## Prerequisites

### System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| PostgreSQL | 14+ | 16+ |
| RAM | 4 GB | 16+ GB |
| CPU | x86_64 or ARM64 | x86_64 with AVX2+ |
| Disk | 10 GB | SSD recommended |

### PostgreSQL Version Requirements

RuVector-Postgres supports PostgreSQL 14-18:

| PostgreSQL Version | Status | Notes |
|-------------------|--------|-------|
| 18 | ✓ Full support | Latest features |
| 17 | ✓ Full support | Recommended |
| 16 | ✓ Full support | Stable |
| 15 | ✓ Full support | Stable |
| 14 | ✓ Full support | Minimum version |
| 13 and below | ✗ Not supported | Use pgvector |

### Build Requirements

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.75+ | Compilation |
| Cargo | 1.75+ | Build system |
| pgrx | 0.12.9+ | PostgreSQL extension framework |
| PostgreSQL Dev | 14-18 | Headers and libraries |
| clang | 14+ | LLVM backend for pgrx |
| pkg-config | any | Dependency management |
| git | 2.0+ | Source checkout |

#### pgrx Version Requirements

**Critical:** RuVector-Postgres requires pgrx **0.12.9 or higher**.

```bash
# Install specific pgrx version
cargo install --locked cargo-pgrx@0.12.9

# Verify version
cargo pgrx --version
# Should output: cargo-pgrx 0.12.9 or higher
```

**Known Issues with Earlier Versions:**

- pgrx 0.11.x: Missing varlena APIs, incompatible type system
- pgrx 0.12.0-0.12.8: Potential memory alignment issues

## Installation Methods

### Method 1: Build from Source (Recommended)

#### Step 1: Install Rust

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify installation
rustc --version  # Should be 1.75.0 or higher
cargo --version
```

#### Step 2: Install System Dependencies

**Ubuntu/Debian:**

```bash
# PostgreSQL and development headers
sudo apt-get update
sudo apt-get install -y \
    postgresql-16 \
    postgresql-server-dev-16 \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    clang \
    git

# Verify pg_config
pg_config --version
```

**RHEL/CentOS/Fedora:**

```bash
# PostgreSQL and development headers
sudo dnf install -y \
    postgresql16-server \
    postgresql16-devel \
    gcc \
    gcc-c++ \
    pkg-config \
    openssl-devel \
    clang-devel \
    git

# Verify pg_config
/usr/pgsql-16/bin/pg_config --version
```

**macOS:**

```bash
# Install PostgreSQL via Homebrew
brew install postgresql@16

# Install build dependencies
brew install llvm pkg-config

# Add pg_config to PATH
export PATH="/opt/homebrew/opt/postgresql@16/bin:$PATH"

# Verify
pg_config --version
```

#### Step 3: Install pgrx

```bash
# Install pgrx CLI (locked version)
cargo install --locked cargo-pgrx@0.12.9

# Initialize pgrx for your PostgreSQL version
cargo pgrx init --pg16 $(which pg_config)

# Or for multiple versions:
cargo pgrx init \
    --pg14 /usr/lib/postgresql/14/bin/pg_config \
    --pg15 /usr/lib/postgresql/15/bin/pg_config \
    --pg16 /usr/lib/postgresql/16/bin/pg_config

# Verify initialization
ls ~/.pgrx/
# Should show: 16.x, data-16, etc.
```

#### Step 4: Build the Extension

```bash
# Clone the repository
git clone https://github.com/ruvnet/ruvector.git
cd ruvector/crates/ruvector-postgres

# Build for your PostgreSQL version
cargo pgrx package --pg-config $(which pg_config)

# The built extension will be in:
# target/release/ruvector-pg16/usr/share/postgresql/16/extension/
# target/release/ruvector-pg16/usr/lib/postgresql/16/lib/
```

**Build Options:**

```bash
# Debug build (for development)
cargo pgrx package --pg-config $(which pg_config) --debug

# Release build with optimizations (default)
cargo pgrx package --pg-config $(which pg_config) --release

# Test before installing
cargo pgrx test pg16
```

#### Step 5: Install the Extension

```bash
# Copy files to PostgreSQL directories
sudo cp target/release/ruvector-pg16/usr/share/postgresql/16/extension/* \
    /usr/share/postgresql/16/extension/

sudo cp target/release/ruvector-pg16/usr/lib/postgresql/16/lib/* \
    /usr/lib/postgresql/16/lib/

# Set proper permissions
sudo chmod 644 /usr/share/postgresql/16/extension/ruvector*
sudo chmod 755 /usr/lib/postgresql/16/lib/ruvector.so

# Restart PostgreSQL
sudo systemctl restart postgresql

# Or on macOS:
brew services restart postgresql@16
```

#### Step 6: Enable in Database

```sql
-- Connect to your database
psql -U postgres -d your_database

-- Create the extension
CREATE EXTENSION ruvector;

-- Verify installation
SELECT ruvector_version();
-- Expected output: 0.1.19 (or current version)

-- Check SIMD capabilities
SELECT ruvector_simd_info();
-- Expected: AVX512, AVX2, NEON, or Scalar
```

### Method 2: Docker Deployment

#### Quick Start with Docker

```bash
# Pull the pre-built image (when available)
docker pull ruvector/postgres:16

# Run container
docker run -d \
    --name ruvector-postgres \
    -e POSTGRES_PASSWORD=mysecretpassword \
    -e POSTGRES_DB=vectordb \
    -p 5432:5432 \
    -v ruvector-data:/var/lib/postgresql/data \
    ruvector/postgres:16

# Connect and enable extension
docker exec -it ruvector-postgres psql -U postgres -d vectordb
```

#### Building Custom Docker Image

Create a `Dockerfile`:

```dockerfile
# Dockerfile for RuVector-Postgres
FROM postgres:16

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    clang \
    curl \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain 1.75.0

# Install pgrx
RUN cargo install --locked cargo-pgrx@0.12.9
RUN cargo pgrx init --pg16 /usr/lib/postgresql/16/bin/pg_config

# Copy and build extension
COPY . /app/ruvector
WORKDIR /app/ruvector/crates/ruvector-postgres
RUN cargo pgrx install --release --pg-config /usr/lib/postgresql/16/bin/pg_config

# Clean up build dependencies to reduce image size
RUN apt-get remove -y build-essential git curl && \
    apt-get autoremove -y && \
    rm -rf /usr/local/cargo/registry /app/ruvector

# Auto-enable extension on database creation
RUN echo "CREATE EXTENSION IF NOT EXISTS ruvector;" > /docker-entrypoint-initdb.d/init-ruvector.sql

EXPOSE 5432
```

Build and run:

```bash
# Build image
docker build -t ruvector-postgres:custom .

# Run container
docker run -d \
    --name ruvector-db \
    -e POSTGRES_PASSWORD=secret \
    -e POSTGRES_DB=vectordb \
    -p 5432:5432 \
    -v $(pwd)/data:/var/lib/postgresql/data \
    ruvector-postgres:custom

# Verify installation
docker exec -it ruvector-db psql -U postgres -d vectordb -c "SELECT ruvector_version();"
```

#### Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  postgres:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: ruvector-postgres
    environment:
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-secret}
      POSTGRES_DB: vectordb
      PGDATA: /var/lib/postgresql/data/pgdata
    ports:
      - "5432:5432"
    volumes:
      - postgres-data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
    restart: unless-stopped

volumes:
  postgres-data:
    driver: local
```

Deploy:

```bash
# Start services
docker-compose up -d

# View logs
docker-compose logs -f

# Stop services
docker-compose down

# Stop and remove volumes
docker-compose down -v
```

### Method 3: Cloud Platforms

#### Neon (Serverless PostgreSQL)

See [NEON_COMPATIBILITY.md](./NEON_COMPATIBILITY.md) for detailed instructions.

**Requirements:**
- Neon Scale plan or higher
- Support ticket for custom extension

**Process:**

1. **Request Installation** (Scale Plan customers):
   ```
   Navigate to: console.neon.tech → Support
   Subject: Custom Extension Request - RuVector-Postgres
   Details:
   - PostgreSQL version: 16 (or your version)
   - Extension: ruvector-postgres v0.1.19
   - Use case: Vector similarity search
   ```

2. **Provide Artifacts**:
   - Pre-built `.so` files
   - Control file (`ruvector.control`)
   - SQL scripts (`ruvector--0.1.0.sql`)

3. **Enable After Approval**:
   ```sql
   CREATE EXTENSION ruvector;
   SELECT ruvector_version();
   ```

#### Supabase

```sql
-- Contact Supabase support for custom extension installation
-- support@supabase.io or via dashboard

-- Once installed:
CREATE EXTENSION ruvector;

-- Verify
SELECT ruvector_version();
```

#### AWS RDS

**Note:** RDS does not support custom extensions. Use EC2 with self-managed PostgreSQL.

**Alternative: RDS with pgvector, migrate later:**

```sql
-- On RDS: Use pgvector
CREATE EXTENSION vector;

-- Migrate to EC2 with RuVector when needed
-- Follow Method 1 (Build from Source)
```

## Configuration

### PostgreSQL Configuration

Add to `postgresql.conf`:

```ini
# RuVector settings
shared_preload_libraries = 'ruvector'  # Optional, for background workers

# Memory settings for vector operations
maintenance_work_mem = '2GB'           # For index builds
work_mem = '256MB'                     # For queries
shared_buffers = '4GB'                 # For caching

# Parallel query settings
max_parallel_workers_per_gather = 4
max_parallel_maintenance_workers = 8
max_worker_processes = 16

# Logging (optional)
log_min_messages = INFO
log_min_duration_statement = 1000      # Log slow queries (1s+)
```

Restart PostgreSQL:

```bash
sudo systemctl restart postgresql
```

### Extension Settings (GUCs)

```sql
-- Search quality (higher = better recall, slower)
SET ruvector.ef_search = 100;          -- Default: 40, Range: 1-1000

-- IVFFlat probes (higher = better recall, slower)
SET ruvector.probes = 10;              -- Default: 1, Range: 1-10000

-- Set globally in postgresql.conf:
ALTER SYSTEM SET ruvector.ef_search = 100;
ALTER SYSTEM SET ruvector.probes = 10;
SELECT pg_reload_conf();
```

### Per-Session Settings

```sql
-- For high-recall queries
BEGIN;
SET LOCAL ruvector.ef_search = 200;
SET LOCAL ruvector.probes = 20;
SELECT * FROM items ORDER BY embedding <-> query LIMIT 10;
COMMIT;

-- For low-latency queries
BEGIN;
SET LOCAL ruvector.ef_search = 20;
SET LOCAL ruvector.probes = 1;
SELECT * FROM items ORDER BY embedding <-> query LIMIT 10;
COMMIT;
```

## Verification

### Check Installation

```sql
-- Verify extension is installed
SELECT * FROM pg_extension WHERE extname = 'ruvector';
-- Expected: extname=ruvector, extversion=0.1.19

-- Check version
SELECT ruvector_version();
-- Expected: 0.1.19

-- Check SIMD capabilities
SELECT ruvector_simd_info();
-- Expected: AVX512, AVX2, NEON, or Scalar
```

### Basic Functionality Test

```sql
-- Create test table
CREATE TABLE test_vectors (
    id SERIAL PRIMARY KEY,
    embedding ruvector(3)
);

-- Insert vectors
INSERT INTO test_vectors (embedding) VALUES
    ('[1, 2, 3]'),
    ('[4, 5, 6]'),
    ('[7, 8, 9]');

-- Test distance calculation
SELECT id, embedding <-> '[1, 1, 1]'::ruvector AS distance
FROM test_vectors
ORDER BY distance
LIMIT 3;

-- Expected output:
-- id | distance
-- ---+-----------
--  1 | 2.449...
--  2 | 6.782...
--  3 | 11.224...

-- Clean up
DROP TABLE test_vectors;
```

### Index Creation Test

```sql
-- Create table with embeddings
CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    embedding ruvector(128)
);

-- Insert sample data (10,000 vectors)
INSERT INTO items (embedding)
SELECT ('[' || array_to_string(array_agg(random()), ',') || ']')::ruvector
FROM generate_series(1, 128) d
CROSS JOIN generate_series(1, 10000) i
GROUP BY i;

-- Create HNSW index
CREATE INDEX items_embedding_idx ON items
USING ruhnsw (embedding ruvector_l2_ops)
WITH (m = 16, ef_construction = 100);

-- Test search with index
EXPLAIN ANALYZE
SELECT * FROM items
ORDER BY embedding <-> (SELECT embedding FROM items LIMIT 1)
LIMIT 10;

-- Verify index usage in plan
-- Should show: "Index Scan using items_embedding_idx"

-- Clean up
DROP TABLE items;
```

## Troubleshooting

### Common Installation Issues

#### 1. Extension Won't Load

```bash
# Check library path
pg_config --pkglibdir
ls -la $(pg_config --pkglibdir)/ruvector*

# Expected output:
# -rwxr-xr-x ... ruvector.so

# Check extension path
pg_config --sharedir
ls -la $(pg_config --sharedir)/extension/ruvector*

# Expected output:
# -rw-r--r-- ... ruvector.control
# -rw-r--r-- ... ruvector--0.1.0.sql

# Check PostgreSQL logs
sudo tail -100 /var/log/postgresql/postgresql-16-main.log
```

**Fix:** Reinstall with correct permissions:

```bash
sudo chmod 755 $(pg_config --pkglibdir)/ruvector.so
sudo chmod 644 $(pg_config --sharedir)/extension/ruvector*
sudo systemctl restart postgresql
```

#### 2. pgrx Version Mismatch

**Error:** `error: failed to load manifest at .../Cargo.toml`

**Cause:** pgrx version < 0.12.9

**Fix:**

```bash
# Uninstall old version
cargo uninstall cargo-pgrx

# Install correct version
cargo install --locked cargo-pgrx@0.12.9

# Re-initialize
cargo pgrx init --pg16 $(which pg_config)

# Rebuild
cargo pgrx package --pg-config $(which pg_config)
```

#### 3. SIMD Not Detected

```sql
-- Check detected SIMD
SELECT ruvector_simd_info();
-- Output: Scalar (unexpected on modern CPUs)
```

**Diagnose:**

```bash
# Linux: Check CPU capabilities
cat /proc/cpuinfo | grep -E 'avx2|avx512'

# macOS: Check CPU features
sysctl -a | grep machdep.cpu.features
```

**Possible Causes:**

- Running in VM without AVX passthrough
- Old CPU without AVX2 support
- Scalar build (missing `target-cpu=native`)

**Fix:** Rebuild with native optimizations:

```bash
# Set Rust flags
export RUSTFLAGS="-C target-cpu=native"

# Rebuild
cargo pgrx package --pg-config $(which pg_config)
sudo systemctl restart postgresql
```

#### 4. Index Build Slow or OOM

**Symptoms:** Index creation times out or crashes

**Solutions:**

```sql
-- Increase maintenance memory
SET maintenance_work_mem = '8GB';

-- Increase parallelism
SET max_parallel_maintenance_workers = 16;

-- Use CONCURRENTLY for non-blocking builds
CREATE INDEX CONCURRENTLY items_embedding_idx ON items
USING ruhnsw (embedding ruvector_l2_ops);

-- Monitor progress
SELECT * FROM pg_stat_progress_create_index;
```

#### 5. Connection Issues

```bash
# Check PostgreSQL is running
sudo systemctl status postgresql

# Check listen addresses
grep listen_addresses /etc/postgresql/16/main/postgresql.conf
# Should be: listen_addresses = '*' or '0.0.0.0'

# Check pg_hba.conf for authentication
sudo cat /etc/postgresql/16/main/pg_hba.conf
# Add: host all all 0.0.0.0/0 md5

# Restart
sudo systemctl restart postgresql
```

## Upgrading

### Minor Version Upgrade (0.1.19 → 0.1.20)

```sql
-- Check current version
SELECT ruvector_version();

-- Upgrade extension
ALTER EXTENSION ruvector UPDATE TO '0.1.20';

-- Verify
SELECT ruvector_version();
```

### Major Version Upgrade

```bash
# Stop PostgreSQL
sudo systemctl stop postgresql

# Install new version
cd ruvector/crates/ruvector-postgres
git pull
cargo pgrx package --pg-config $(which pg_config)
sudo cp target/release/ruvector-pg16/usr/lib/postgresql/16/lib/* \
    $(pg_config --pkglibdir)/

# Start PostgreSQL
sudo systemctl start postgresql

# Upgrade in database
psql -U postgres -d your_database -c "ALTER EXTENSION ruvector UPDATE;"
```

## Uninstallation

```sql
-- Drop all dependent objects first
DROP INDEX IF EXISTS items_embedding_idx;

-- Drop extension
DROP EXTENSION ruvector CASCADE;
```

```bash
# Remove library files
sudo rm $(pg_config --pkglibdir)/ruvector.so
sudo rm $(pg_config --sharedir)/extension/ruvector*

# Restart PostgreSQL
sudo systemctl restart postgresql
```

## Support

- **Documentation**: https://github.com/ruvnet/ruvector/tree/main/crates/ruvector-postgres/docs
- **Issues**: https://github.com/ruvnet/ruvector/issues
- **Discussions**: https://github.com/ruvnet/ruvector/discussions
