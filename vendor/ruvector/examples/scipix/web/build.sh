#!/bin/bash
set -e

echo "Building Mathpix WASM module..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "wasm-pack not found. Installing..."
    cargo install wasm-pack
fi

cd "$(dirname "$0")/.."

# Build for production
echo "Building release..."
wasm-pack build \
    --target web \
    --out-dir web/pkg \
    --release \
    -- --features wasm

echo "✓ Build complete!"
echo "  Output: web/pkg/"
echo "  Size: $(du -sh web/pkg/ruvector_scipix_bg.wasm | cut -f1)"

# Run demo server
if [ "$1" = "--serve" ]; then
    echo ""
    echo "Starting demo server on http://localhost:8080"
    cd web
    python3 -m http.server 8080
fi
