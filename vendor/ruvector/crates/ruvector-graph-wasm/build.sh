#!/bin/bash
set -e

echo "Building RuVector Graph WASM..."

# Build for web (default)
echo "Building for web target..."
wasm-pack build --target web --out-dir ../../npm/packages/graph-wasm

# Build for Node.js
echo "Building for Node.js target..."
wasm-pack build --target nodejs --out-dir ../../npm/packages/graph-wasm/node

# Build for bundlers
echo "Building for bundler target..."
wasm-pack build --target bundler --out-dir ../../npm/packages/graph-wasm/bundler

echo "Build complete!"
echo "Web: npm/packages/graph-wasm/"
echo "Node.js: npm/packages/graph-wasm/node/"
echo "Bundler: npm/packages/graph-wasm/bundler/"
