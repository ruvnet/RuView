# Build System Documentation

This document describes the build system for the ruvector-postgres extension.

## Overview

The build system supports multiple PostgreSQL versions (14-17), various SIMD optimizations, and optional features like different index types and quantization methods.

## Prerequisites

- Rust 1.75 or later
- PostgreSQL 14, 15, 16, or 17
- cargo-pgrx 0.12.0
- Build essentials (gcc, make, etc.)

## Quick Start

### Using Make (Recommended)

```bash
# Build for PostgreSQL 16 (default)
make build

# Build with all features
make build-all

# Build with native CPU optimizations
make build-native

# Run tests
make test

# Install extension
make install
```

### Using Cargo

```bash
# Build for PostgreSQL 16
cargo pgrx package --features pg16

# Build with specific features
cargo pgrx package --features pg16,index-all,quant-all

# Run tests
cargo pgrx test pg16
```

## Build Features

### PostgreSQL Versions

Choose one PostgreSQL version feature:

- `pg14` - PostgreSQL 14
- `pg15` - PostgreSQL 15
- `pg16` - PostgreSQL 16 (default)
- `pg17` - PostgreSQL 17

Example:
```bash
make build PGVER=15
```

### SIMD Optimizations

SIMD features for performance optimization:

- `simd-native` - Use native CPU features (auto-detected at build time)
- `simd-avx512` - Enable AVX-512 instructions
- `simd-avx2` - Enable AVX2 instructions
- `simd-neon` - Enable ARM NEON instructions
- `simd-auto` - Runtime auto-detection (default)

Example:
```bash
# Build with native CPU optimizations
make build-native

# Build with specific SIMD
cargo build --features pg16,simd-avx512 --release
```

### Index Types

- `index-hnsw` - HNSW (Hierarchical Navigable Small World) index
- `index-ivfflat` - IVFFlat (Inverted File with Flat compression) index
- `index-all` - Enable all index types

Example:
```bash
make build INDEX_ALL=1
```

### Quantization Methods

- `quantization-scalar` - Scalar quantization
- `quantization-product` - Product quantization
- `quantization-binary` - Binary quantization
- `quantization-all` - Enable all quantization methods
- `quant-all` - Alias for `quantization-all`

Example:
```bash
make build QUANT_ALL=1
```

### Optional Features

- `hybrid-search` - Hybrid search capabilities
- `filtered-search` - Filtered search support
- `neon-compat` - Neon-specific optimizations

## Build Modes

### Debug Mode

```bash
make build BUILD_MODE=debug
```

Debug builds include:
- Debug symbols
- Assertions enabled
- No optimizations
- Faster compile times

### Release Mode (Default)

```bash
make build BUILD_MODE=release
```

Release builds include:
- Full optimizations
- No debug symbols
- Smaller binary size
- Better performance

## Build Script (build.rs)

The `build.rs` script automatically:

1. **Detects CPU features** at build time
2. **Configures SIMD optimizations** based on target architecture
3. **Prints feature status** during compilation
4. **Sets up PostgreSQL paths** from environment

### CPU Feature Detection

For x86_64 systems:
- Checks for AVX-512, AVX2, and SSE4.2 support
- Enables appropriate compiler flags
- Prints build configuration

For ARM systems:
- Enables NEON support on AArch64
- Configures appropriate SIMD features

### Native Optimization

When building with `simd-native`, the build script adds:
```
RUSTFLAGS=-C target-cpu=native
```

This enables all CPU features available on the build machine.

## Makefile Targets

### Build Targets

- `make build` - Build for default PostgreSQL version
- `make build-all` - Build with all features enabled
- `make build-native` - Build with native CPU optimizations
- `make package` - Create distributable package

### Test Targets

- `make test` - Run tests for current PostgreSQL version
- `make test-all` - Run tests for all PostgreSQL versions
- `make bench` - Run all benchmarks
- `make bench-<name>` - Run specific benchmark

### Development Targets

- `make dev` - Start development server
- `make pgrx-init` - Initialize pgrx (first-time setup)
- `make pgrx-start` - Start PostgreSQL for development
- `make pgrx-stop` - Stop PostgreSQL
- `make pgrx-connect` - Connect to development database

### Quality Targets

- `make check` - Run cargo check
- `make clippy` - Run clippy linter
- `make fmt` - Format code
- `make fmt-check` - Check code formatting

### Other Targets

- `make clean` - Clean build artifacts
- `make doc` - Generate documentation
- `make config` - Show current configuration
- `make help` - Show all available targets

## Configuration Variables

### PostgreSQL Configuration

```bash
# Specify pg_config path
make build PG_CONFIG=/usr/pgsql-16/bin/pg_config

# Set PostgreSQL version
make test PGVER=15

# Set installation prefix
make install PREFIX=/opt/postgresql
```

### Build Configuration

```bash
# Enable features via environment
make build SIMD_NATIVE=1 INDEX_ALL=1 QUANT_ALL=1

# Change build mode
make build BUILD_MODE=debug

# Combine options
make test PGVER=16 BUILD_MODE=release QUANT_ALL=1
```

## CI/CD Integration

The GitHub Actions workflow (`postgres-extension-ci.yml`) provides:

### Test Matrix

- Tests on Ubuntu and macOS
- PostgreSQL versions 14, 15, 16, 17
- Stable Rust toolchain

### Build Steps

1. Install PostgreSQL and development headers
2. Set up Rust toolchain with caching
3. Install and initialize cargo-pgrx
4. Run formatting and linting checks
5. Build extension
6. Run tests
7. Package artifacts

### Additional Checks

- Security audit with cargo-audit
- Benchmark comparison on pull requests
- Integration tests with Docker
- Package creation for releases

## Docker Build

### Building Docker Image

```bash
# Build image
docker build -t ruvector-postgres:latest -f crates/ruvector-postgres/Dockerfile .

# Run container
docker run -d \
  -e POSTGRES_PASSWORD=postgres \
  -p 5432:5432 \
  ruvector-postgres:latest
```

### Multi-stage Build

The Dockerfile uses multi-stage builds:

1. **Builder stage**: Compiles extension with all features
2. **Runtime stage**: Creates minimal PostgreSQL image with extension

### Docker Features

- Based on official PostgreSQL 16 image
- Extension pre-installed and ready to use
- Automatic extension creation on startup
- Health checks configured
- Optimized layer caching

## Troubleshooting

### Common Issues

**Issue**: `pg_config not found`
```bash
# Solution: Set PG_CONFIG
export PG_CONFIG=/usr/lib/postgresql/16/bin/pg_config
make build
```

**Issue**: `cargo-pgrx not installed`
```bash
# Solution: Install cargo-pgrx
cargo install cargo-pgrx --version 0.12.0 --locked
```

**Issue**: `pgrx not initialized`
```bash
# Solution: Initialize pgrx
make pgrx-init
```

**Issue**: Build fails with SIMD errors
```bash
# Solution: Build without SIMD optimizations
cargo build --features pg16 --release
```

### Debug Build Issues

Enable verbose output:
```bash
cargo build --features pg16 --release --verbose
```

Check build configuration:
```bash
make config
```

### Test Failures

Run tests with output:
```bash
cargo pgrx test pg16 -- --nocapture
```

Run specific test:
```bash
cargo test --features pg16 test_name
```

## Performance Optimization

### Compile-time Optimizations

```bash
# Native CPU features
make build-native

# Link-time optimization (slower build, faster runtime)
RUSTFLAGS="-C lto=fat" make build

# Combine optimizations
RUSTFLAGS="-C target-cpu=native -C lto=fat" make build
```

### Profile-guided Optimization (PGO)

```bash
# 1. Build with instrumentation
RUSTFLAGS="-C profile-generate=/tmp/pgo-data" make build

# 2. Run benchmarks to collect profiles
make bench

# 3. Build with profile data
RUSTFLAGS="-C profile-use=/tmp/pgo-data" make build
```

## Cross-compilation

### For ARM64

```bash
# Add target
rustup target add aarch64-unknown-linux-gnu

# Build
cargo build --target aarch64-unknown-linux-gnu \
  --features pg16,simd-neon \
  --release
```

### For Different PostgreSQL Versions

```bash
# Build for all versions
for pgver in 14 15 16 17; do
  make build PGVER=$pgver
done
```

## Distribution

### Creating Packages

```bash
# Create package for distribution
make package

# Package location
ls target/release/ruvector-postgres-pg16/
```

### Installation from Package

```bash
# Copy files
sudo cp target/release/ruvector-postgres-pg16/usr/lib/postgresql/16/lib/*.so \
  /usr/lib/postgresql/16/lib/
sudo cp target/release/ruvector-postgres-pg16/usr/share/postgresql/16/extension/* \
  /usr/share/postgresql/16/extension/

# Verify installation
psql -c "CREATE EXTENSION ruvector;"
```

## References

- [pgrx Documentation](https://github.com/pgcentralfoundation/pgrx)
- [PostgreSQL Extension Building](https://www.postgresql.org/docs/current/extend-extensions.html)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
