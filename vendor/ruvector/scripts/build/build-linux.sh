#!/bin/bash
# Build NAPI-RS bindings for Linux platforms only
# Usage: ./scripts/build/build-linux.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
NPM_PLATFORMS_DIR="$PROJECT_ROOT/npm/core/platforms"
NPM_NATIVE_DIR="$PROJECT_ROOT/npm/core/native"

echo "Building Ruvector NAPI-RS for Linux platforms..."

# Ensure directories exist
mkdir -p "$NPM_PLATFORMS_DIR"/{linux-x64-gnu,linux-arm64-gnu}
mkdir -p "$NPM_NATIVE_DIR"/linux-x64

# Build Linux x64
echo "Building for x86_64-unknown-linux-gnu..."
cargo build --release -p ruvector-node --target x86_64-unknown-linux-gnu

# Copy binary
cp "$PROJECT_ROOT/target/x86_64-unknown-linux-gnu/release/libruvector_node.so" \
   "$NPM_PLATFORMS_DIR/linux-x64-gnu/ruvector.node"
cp "$PROJECT_ROOT/target/x86_64-unknown-linux-gnu/release/libruvector_node.so" \
   "$NPM_NATIVE_DIR/linux-x64/ruvector.node"

echo "✓ Linux x64 build complete"

# Build Linux ARM64
echo "Building for aarch64-unknown-linux-gnu..."
cargo build --release -p ruvector-node --target aarch64-unknown-linux-gnu

# Copy binary
cp "$PROJECT_ROOT/target/aarch64-unknown-linux-gnu/release/libruvector_node.so" \
   "$NPM_PLATFORMS_DIR/linux-arm64-gnu/ruvector.node"

echo "✓ Linux ARM64 build complete"

# Show results
echo ""
echo "Built binaries:"
ls -lh "$NPM_PLATFORMS_DIR"/linux-*/ruvector.node
ls -lh "$NPM_NATIVE_DIR"/linux-x64/ruvector.node
