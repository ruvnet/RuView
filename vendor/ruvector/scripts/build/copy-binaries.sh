#!/bin/bash
# Copy built binaries to npm package directories
# Usage: ./scripts/build/copy-binaries.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TARGET_DIR="$PROJECT_ROOT/target"
NPM_PLATFORMS_DIR="$PROJECT_ROOT/npm/core/platforms"
NPM_NATIVE_DIR="$PROJECT_ROOT/npm/core/native"

echo "Copying built binaries to npm packages..."

# Ensure directories exist
mkdir -p "$NPM_PLATFORMS_DIR"/{linux-x64-gnu,linux-arm64-gnu,darwin-x64,darwin-arm64,win32-x64-msvc}
mkdir -p "$NPM_NATIVE_DIR"/linux-x64

# Copy Linux x64
if [ -f "$TARGET_DIR/x86_64-unknown-linux-gnu/release/libruvector_node.so" ]; then
    cp "$TARGET_DIR/x86_64-unknown-linux-gnu/release/libruvector_node.so" \
       "$NPM_PLATFORMS_DIR/linux-x64-gnu/ruvector.node"
    cp "$TARGET_DIR/x86_64-unknown-linux-gnu/release/libruvector_node.so" \
       "$NPM_NATIVE_DIR/linux-x64/ruvector.node"
    echo "✓ Copied linux-x64-gnu"
fi

# Copy Linux ARM64
if [ -f "$TARGET_DIR/aarch64-unknown-linux-gnu/release/libruvector_node.so" ]; then
    cp "$TARGET_DIR/aarch64-unknown-linux-gnu/release/libruvector_node.so" \
       "$NPM_PLATFORMS_DIR/linux-arm64-gnu/ruvector.node"
    echo "✓ Copied linux-arm64-gnu"
fi

# Copy macOS x64
if [ -f "$TARGET_DIR/x86_64-apple-darwin/release/libruvector_node.dylib" ]; then
    cp "$TARGET_DIR/x86_64-apple-darwin/release/libruvector_node.dylib" \
       "$NPM_PLATFORMS_DIR/darwin-x64/ruvector.node"
    echo "✓ Copied darwin-x64"
fi

# Copy macOS ARM64
if [ -f "$TARGET_DIR/aarch64-apple-darwin/release/libruvector_node.dylib" ]; then
    cp "$TARGET_DIR/aarch64-apple-darwin/release/libruvector_node.dylib" \
       "$NPM_PLATFORMS_DIR/darwin-arm64/ruvector.node"
    echo "✓ Copied darwin-arm64"
fi

# Copy Windows x64
if [ -f "$TARGET_DIR/x86_64-pc-windows-msvc/release/ruvector_node.dll" ]; then
    cp "$TARGET_DIR/x86_64-pc-windows-msvc/release/ruvector_node.dll" \
       "$NPM_PLATFORMS_DIR/win32-x64-msvc/ruvector.node"
    echo "✓ Copied win32-x64-msvc"
fi

echo ""
echo "Current npm platform binaries:"
find "$NPM_PLATFORMS_DIR" -name "ruvector.node" -exec ls -lh {} \;
