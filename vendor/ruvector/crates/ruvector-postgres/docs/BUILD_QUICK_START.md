# Build System Quick Start

## Files Created

### Core Build Files
- **`build.rs`** - SIMD feature detection and build configuration
- **`Makefile`** - Common build operations and shortcuts
- **`Dockerfile`** - Multi-stage Docker build for distribution
- **`.dockerignore`** - Docker build optimization

### CI/CD
- **`.github/workflows/postgres-extension-ci.yml`** - GitHub Actions workflow

### Documentation
- **`docs/BUILD.md`** - Comprehensive build system documentation
- **`docs/BUILD_QUICK_START.md`** - This file

## Updated Files
- **`Cargo.toml`** - Added new features: `simd-native`, `index-all`, `quant-all`

## Quick Commands

### Build
```bash
# Basic build
make build

# All features enabled
make build-all

# Native CPU optimizations
make build-native

# Specific PostgreSQL version
make build PGVER=15
```

### Test
```bash
# Test current version
make test

# Test all PostgreSQL versions
make test-all

# Run benchmarks
make bench
```

### Install
```bash
# Install to default location
make install

# Install with sudo
make install-sudo

# Install to custom location
make install PG_CONFIG=/custom/path/pg_config
```

### Development
```bash
# Initialize pgrx (first time only)
make pgrx-init

# Start development server
make dev

# Connect to database
make pgrx-connect
```

### Docker
```bash
# Build Docker image
docker build -t ruvector-postgres:latest \
  -f crates/ruvector-postgres/Dockerfile .

# Run container
docker run -d \
  -e POSTGRES_PASSWORD=postgres \
  -p 5432:5432 \
  ruvector-postgres:latest

# Test extension
docker exec -it <container> psql -U postgres -c "CREATE EXTENSION ruvector;"
```

## Feature Flags

### SIMD Optimization
```bash
# Auto-detect and use native CPU features
make build SIMD_NATIVE=1

# Specific SIMD instruction set
cargo build --features pg16,simd-avx512 --release
```

### Index Types
```bash
# Enable all index types (HNSW, IVFFlat)
make build INDEX_ALL=1

# Specific index
cargo build --features pg16,index-hnsw --release
```

### Quantization
```bash
# Enable all quantization methods
make build QUANT_ALL=1

# Specific quantization
cargo build --features pg16,quantization-scalar --release
```

### Combine Features
```bash
# Kitchen sink build
make build-native INDEX_ALL=1 QUANT_ALL=1

# Or with cargo
cargo build --features pg16,simd-native,index-all,quant-all --release
```

## CI/CD Pipeline

The GitHub Actions workflow automatically:

1. **Tests** on PostgreSQL 14, 15, 16, 17
2. **Builds** on Ubuntu and macOS
3. **Runs** security audits
4. **Checks** code formatting and linting
5. **Benchmarks** on pull requests
6. **Packages** artifacts for releases
7. **Tests** Docker integration

Triggered on:
- Push to `main`, `develop`, or `claude/**` branches
- Pull requests to `main` or `develop`
- Manual workflow dispatch

## Build Output

### Makefile Status
The build.rs script reports detected features:
```
cargo:warning=Building with SSE4.2 support
cargo:warning=Feature Status:
cargo:warning=  ✓ HNSW index enabled
cargo:warning=  ✓ IVFFlat index enabled
```

### Artifacts
Built extension is located at:
```
target/release/ruvector-postgres-pg16/
├── usr/
│   ├── lib/postgresql/16/lib/
│   │   └── ruvector.so
│   └── share/postgresql/16/extension/
│       ├── ruvector.control
│       └── ruvector--*.sql
```

## Configuration

### View Current Config
```bash
make config
```

Output example:
```
Configuration:
  PG_CONFIG:     pg_config
  PGVER:         16
  PREFIX:        /usr
  PKGLIBDIR:     /usr/lib/postgresql/16/lib
  EXTENSION_DIR: /usr/share/postgresql/16/extension
  BUILD_MODE:    release
  FEATURES:      pg16
  CARGO_FLAGS:   --features pg16 --release
```

## Troubleshooting

### pg_config not found
```bash
# Set PG_CONFIG environment variable
export PG_CONFIG=/usr/lib/postgresql/16/bin/pg_config
make build
```

### cargo-pgrx not installed
```bash
cargo install cargo-pgrx --version 0.12.0 --locked
```

### pgrx not initialized
```bash
make pgrx-init
```

### Permission denied during install
```bash
make install-sudo
```

## Performance Tips

### Maximum Performance Build
```bash
# Native CPU + LTO + All optimizations
RUSTFLAGS="-C target-cpu=native -C lto=fat" \
  make build INDEX_ALL=1 QUANT_ALL=1
```

### Faster Development Builds
```bash
# Debug mode for faster compilation
make build BUILD_MODE=debug
```

## Next Steps

1. Read full documentation: `docs/BUILD.md`
2. Run tests: `make test`
3. Try Docker: Build and run containerized version
4. Benchmark: `make bench` to measure performance
5. Install: `make install` to deploy extension

## Support

- Build Issues: Check `docs/BUILD.md` troubleshooting section
- Feature Requests: Open GitHub issue
- CI/CD: Review `.github/workflows/postgres-extension-ci.yml`
