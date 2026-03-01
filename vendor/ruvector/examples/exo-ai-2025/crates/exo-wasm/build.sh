#!/bin/bash
# Build script for exo-wasm

set -e

echo "🔨 Building exo-wasm for browser deployment..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "❌ wasm-pack is not installed"
    echo "📦 Install with: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh"
    exit 1
fi

# Build for web (ES modules)
echo "📦 Building for web target..."
wasm-pack build --target web --release

# Build for Node.js
echo "📦 Building for Node.js target..."
wasm-pack build --target nodejs --release --out-dir pkg-node

# Build for bundlers (Webpack/Rollup)
echo "📦 Building for bundler target..."
wasm-pack build --target bundler --release --out-dir pkg-bundler

echo "✅ Build complete!"
echo ""
echo "📂 Output directories:"
echo "  - pkg/          (web/ES modules)"
echo "  - pkg-node/     (Node.js)"
echo "  - pkg-bundler/  (Webpack/Rollup)"
echo ""
echo "🌐 To test in browser:"
echo "  1. Copy examples/browser_demo.html to pkg/"
echo "  2. Start a local server (e.g., python -m http.server)"
echo "  3. Open http://localhost:8000/browser_demo.html"
