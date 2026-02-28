#!/bin/bash
# Test script for @ruvector/postgres-cli npx installation
# This script tests the CLI package in a clean Docker environment

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== RuVector PostgreSQL CLI - NPX Installation Test ==="
echo ""

# Step 1: Build the package
echo "Step 1: Building the package..."
cd "$PACKAGE_DIR"
npm run build
echo "✓ Build complete"
echo ""

# Step 2: Create the tarball
echo "Step 2: Creating package tarball..."
npm pack
TARBALL=$(ls -t *.tgz | head -1)
mv "$TARBALL" tests/ruvector-postgres-cli.tgz
echo "✓ Tarball created: tests/ruvector-postgres-cli.tgz"
echo ""

# Step 3: Build the test Docker image
echo "Step 3: Building test Docker image..."
cd tests
docker build -f Dockerfile.npx-test -t ruvector-cli-test .
echo "✓ Docker image built"
echo ""

# Step 4: Run tests inside the container
echo "Step 4: Running CLI tests..."
echo ""

echo "--- Test: ruvector-pg --help ---"
docker run --rm ruvector-cli-test ruvector-pg --help
echo ""

echo "--- Test: ruvector-pg --version ---"
docker run --rm ruvector-cli-test ruvector-pg --version
echo ""

echo "--- Test: rvpg (alias) --help ---"
docker run --rm ruvector-cli-test rvpg --help | head -10
echo ""

echo "--- Test: ruvector-pg vector --help ---"
docker run --rm ruvector-cli-test ruvector-pg vector --help
echo ""

echo "--- Test: ruvector-pg attention --help ---"
docker run --rm ruvector-cli-test ruvector-pg attention --help
echo ""

echo "--- Test: ruvector-pg hyperbolic --help ---"
docker run --rm ruvector-cli-test ruvector-pg hyperbolic --help
echo ""

echo "--- Test: ruvector-pg routing --help ---"
docker run --rm ruvector-cli-test ruvector-pg routing --help
echo ""

echo "--- Test: ruvector-pg sparse --help ---"
docker run --rm ruvector-cli-test ruvector-pg sparse --help
echo ""

echo "--- Test: ruvector-pg learning --help ---"
docker run --rm ruvector-cli-test ruvector-pg learning --help
echo ""

echo "--- Test: ruvector-pg gnn --help ---"
docker run --rm ruvector-cli-test ruvector-pg gnn --help
echo ""

echo "--- Test: ruvector-pg graph --help ---"
docker run --rm ruvector-cli-test ruvector-pg graph --help
echo ""

echo "--- Test: ruvector-pg bench --help ---"
docker run --rm ruvector-cli-test ruvector-pg bench --help
echo ""

echo "--- Test: ruvector-pg quant --help ---"
docker run --rm ruvector-cli-test ruvector-pg quant --help
echo ""

echo "--- Test: ruvector-pg install --help ---"
docker run --rm ruvector-cli-test ruvector-pg install --help
echo ""

# Clean up
echo "Step 5: Cleaning up..."
rm -f ruvector-postgres-cli.tgz
docker rmi ruvector-cli-test 2>/dev/null || true
echo "✓ Cleanup complete"
echo ""

echo "=== All tests passed! ==="
