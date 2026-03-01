#!/bin/bash
set -euo pipefail

echo "Building ruvector-solver..."

# Native build
cargo build --release -p ruvector-solver

# WASM build (if wasm-pack available)
if command -v wasm-pack &> /dev/null; then
    echo "Building WASM..."
    cd crates/ruvector-solver-wasm
    wasm-pack build --target web --release
    cd ../..
fi

echo "Build complete!"
