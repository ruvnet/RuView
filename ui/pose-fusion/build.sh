#!/bin/bash
# Build WASM packages for the dual-modal pose estimation demo.
# Requires: wasm-pack (cargo install wasm-pack)
#
# Usage: ./build.sh
#
# Output: pkg/ruvector_cnn_wasm/ - WASM CNN embedder for browser

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENDOR_DIR="$SCRIPT_DIR/../../vendor/ruvector"
OUT_DIR="$SCRIPT_DIR/pkg/ruvector_cnn_wasm"

echo "Building ruvector-cnn-wasm..."
wasm-pack build "$VENDOR_DIR/crates/ruvector-cnn-wasm" \
  --target web \
  --out-dir "$OUT_DIR" \
  --no-typescript

# Remove .gitignore so we can commit the build output for GitHub Pages
rm -f "$OUT_DIR/.gitignore"

echo ""
echo "Build complete!"
echo "  WASM: $(du -sh "$OUT_DIR/ruvector_cnn_wasm_bg.wasm" | cut -f1)"
echo "  JS:   $(du -sh "$OUT_DIR/ruvector_cnn_wasm.js" | cut -f1)"
echo ""
echo "Serve the demo: cd $SCRIPT_DIR/.. && python3 -m http.server 8080"
echo "Open: http://localhost:8080/pose-fusion.html"
