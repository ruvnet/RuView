# Makefile for ruvector-postgres extension
# Provides common operations for building, testing, and installing

# PostgreSQL configuration
PG_CONFIG ?= pg_config
PGVER ?= 16

# Build configuration
CARGO ?= cargo
FEATURES ?= pg$(PGVER)
BUILD_MODE ?= release

# Installation paths
DESTDIR ?=
PREFIX ?= $(shell $(PG_CONFIG) --prefix)
PKGLIBDIR ?= $(shell $(PG_CONFIG) --pkglibdir)
SHAREDIR ?= $(shell $(PG_CONFIG) --sharedir)
EXTENSION_DIR ?= $(SHAREDIR)/extension

# Build flags
CARGO_FLAGS = --features $(FEATURES)
ifeq ($(BUILD_MODE),release)
	CARGO_FLAGS += --release
	TARGET_DIR = target/release
else
	TARGET_DIR = target/debug
endif

# SIMD features
ifdef SIMD_NATIVE
	CARGO_FLAGS += --features simd-native
	export RUSTFLAGS=-C target-cpu=native
endif

ifdef SIMD_AVX512
	CARGO_FLAGS += --features simd-avx512
endif

ifdef SIMD_AVX2
	CARGO_FLAGS += --features simd-avx2
endif

# Index features
ifdef INDEX_ALL
	CARGO_FLAGS += --features index-all
endif

# Quantization features
ifdef QUANT_ALL
	CARGO_FLAGS += --features quant-all
endif

.PHONY: all build test install clean check bench doc package help

# Default target
all: build

# Build the extension
build:
	@echo "Building ruvector-postgres for PostgreSQL $(PGVER)..."
	$(CARGO) pgrx package $(CARGO_FLAGS)

# Build with all features enabled
build-all:
	@echo "Building with all features enabled..."
	$(MAKE) build INDEX_ALL=1 QUANT_ALL=1

# Build with native CPU optimizations
build-native:
	@echo "Building with native CPU optimizations..."
	$(MAKE) build SIMD_NATIVE=1

# Run tests
test:
	@echo "Running tests for PostgreSQL $(PGVER)..."
	$(CARGO) pgrx test pg$(PGVER) $(CARGO_FLAGS)

# Run tests for all PostgreSQL versions
test-all:
	@echo "Running tests for all PostgreSQL versions..."
	$(CARGO) pgrx test pg14
	$(CARGO) pgrx test pg15
	$(CARGO) pgrx test pg16
	$(CARGO) pgrx test pg17

# Install the extension
install:
	@echo "Installing ruvector-postgres to $(PREFIX)..."
	$(CARGO) pgrx install --pg-config $(PG_CONFIG) $(CARGO_FLAGS)

# Install with sudo (for system-wide installation)
install-sudo:
	@echo "Installing ruvector-postgres with sudo..."
	sudo $(CARGO) pgrx install --pg-config $(PG_CONFIG) $(CARGO_FLAGS)

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	$(CARGO) clean
	rm -rf target/

# Run cargo check
check:
	@echo "Running cargo check..."
	$(CARGO) check $(CARGO_FLAGS)

# Run clippy linter
clippy:
	@echo "Running clippy..."
	$(CARGO) clippy $(CARGO_FLAGS) -- -D warnings

# Run cargo fmt
fmt:
	@echo "Formatting code..."
	$(CARGO) fmt --all

# Check formatting
fmt-check:
	@echo "Checking code formatting..."
	$(CARGO) fmt --all -- --check

# Run benchmarks
bench:
	@echo "Running benchmarks..."
	$(CARGO) bench $(CARGO_FLAGS)

# Run specific benchmark
bench-%:
	@echo "Running $* benchmark..."
	$(CARGO) bench --bench $* $(CARGO_FLAGS)

# Generate documentation
doc:
	@echo "Generating documentation..."
	$(CARGO) doc $(CARGO_FLAGS) --no-deps --open

# Create distributable package
package:
	@echo "Creating package for PostgreSQL $(PGVER)..."
	$(CARGO) pgrx package $(CARGO_FLAGS)
	@echo "Package created in target/$(BUILD_MODE)/ruvector-postgres-pg$(PGVER)/"

# Initialize pgrx (first-time setup)
pgrx-init:
	@echo "Initializing pgrx..."
	$(CARGO) pgrx init

# Start PostgreSQL for development
pgrx-start:
	@echo "Starting PostgreSQL $(PGVER) for development..."
	$(CARGO) pgrx start pg$(PGVER)

# Stop PostgreSQL
pgrx-stop:
	@echo "Stopping PostgreSQL $(PGVER)..."
	$(CARGO) pgrx stop pg$(PGVER)

# Connect to development database
pgrx-connect:
	@echo "Connecting to PostgreSQL $(PGVER)..."
	$(CARGO) pgrx connect pg$(PGVER)

# Run development server with extension loaded
dev:
	@echo "Starting development server..."
	$(CARGO) pgrx run pg$(PGVER) $(CARGO_FLAGS)

# Show configuration
config:
	@echo "Configuration:"
	@echo "  PG_CONFIG:     $(PG_CONFIG)"
	@echo "  PGVER:         $(PGVER)"
	@echo "  PREFIX:        $(PREFIX)"
	@echo "  PKGLIBDIR:     $(PKGLIBDIR)"
	@echo "  EXTENSION_DIR: $(EXTENSION_DIR)"
	@echo "  BUILD_MODE:    $(BUILD_MODE)"
	@echo "  FEATURES:      $(FEATURES)"
	@echo "  CARGO_FLAGS:   $(CARGO_FLAGS)"

# Help target
help:
	@echo "ruvector-postgres Makefile"
	@echo ""
	@echo "Common targets:"
	@echo "  make build              - Build the extension"
	@echo "  make build-all          - Build with all features"
	@echo "  make build-native       - Build with native CPU optimizations"
	@echo "  make test               - Run tests for current PostgreSQL version"
	@echo "  make test-all           - Run tests for all PostgreSQL versions"
	@echo "  make install            - Install the extension"
	@echo "  make install-sudo       - Install with sudo"
	@echo "  make clean              - Clean build artifacts"
	@echo "  make check              - Run cargo check"
	@echo "  make clippy             - Run clippy linter"
	@echo "  make fmt                - Format code"
	@echo "  make fmt-check          - Check code formatting"
	@echo "  make bench              - Run all benchmarks"
	@echo "  make bench-<name>       - Run specific benchmark"
	@echo "  make doc                - Generate documentation"
	@echo "  make package            - Create distributable package"
	@echo ""
	@echo "Development targets:"
	@echo "  make pgrx-init          - Initialize pgrx (first-time setup)"
	@echo "  make pgrx-start         - Start PostgreSQL for development"
	@echo "  make pgrx-stop          - Stop PostgreSQL"
	@echo "  make pgrx-connect       - Connect to development database"
	@echo "  make dev                - Run development server"
	@echo ""
	@echo "Configuration variables:"
	@echo "  PG_CONFIG=<path>        - Path to pg_config (default: pg_config)"
	@echo "  PGVER=<version>         - PostgreSQL version (14, 15, 16, 17; default: 16)"
	@echo "  BUILD_MODE=<mode>       - Build mode (debug, release; default: release)"
	@echo "  SIMD_NATIVE=1           - Enable native CPU optimizations"
	@echo "  SIMD_AVX512=1           - Enable AVX-512"
	@echo "  SIMD_AVX2=1             - Enable AVX2"
	@echo "  INDEX_ALL=1             - Enable all index types"
	@echo "  QUANT_ALL=1             - Enable all quantization methods"
	@echo ""
	@echo "Examples:"
	@echo "  make build PGVER=15"
	@echo "  make test PGVER=16 BUILD_MODE=debug"
	@echo "  make install PG_CONFIG=/usr/pgsql-16/bin/pg_config"
	@echo "  make build-native INDEX_ALL=1 QUANT_ALL=1"
