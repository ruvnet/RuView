#!/bin/bash
# Build script for Delta-Behavior WASM bindings
# This script builds the Rust code to WebAssembly using wasm-pack

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Delta-Behavior WASM Build ==="
echo "Project directory: $PROJECT_DIR"
echo ""

# Check for wasm-pack
if ! command -v wasm-pack &> /dev/null; then
    echo "wasm-pack not found. Installing..."
    cargo install wasm-pack
fi

# Check for wasm32 target
if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
    echo "Adding wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

cd "$PROJECT_DIR"

# Parse arguments
TARGET="web"
PROFILE="release"
OUT_DIR="wasm/pkg"

while [[ $# -gt 0 ]]; do
    case $1 in
        --target)
            TARGET="$2"
            shift 2
            ;;
        --dev)
            PROFILE="dev"
            shift
            ;;
        --release)
            PROFILE="release"
            shift
            ;;
        --out-dir)
            OUT_DIR="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --target <TARGET>   Build target: web, nodejs, bundler (default: web)"
            echo "  --dev               Build in development mode"
            echo "  --release           Build in release mode (default)"
            echo "  --out-dir <DIR>     Output directory (default: wasm/pkg)"
            echo "  --help, -h          Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                           # Build for web in release mode"
            echo "  $0 --target nodejs           # Build for Node.js"
            echo "  $0 --dev                     # Build in development mode"
            echo "  $0 --target bundler          # Build for bundlers (webpack, etc.)"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo "Building with:"
echo "  Target: $TARGET"
echo "  Profile: $PROFILE"
echo "  Output: $OUT_DIR"
echo ""

# Build options
BUILD_OPTS="--target $TARGET --out-dir $OUT_DIR"

if [ "$PROFILE" = "dev" ]; then
    BUILD_OPTS="$BUILD_OPTS --dev"
else
    BUILD_OPTS="$BUILD_OPTS --release"
fi

# Run wasm-pack build
echo "Running: wasm-pack build $BUILD_OPTS"
wasm-pack build $BUILD_OPTS

# Post-build: Copy TypeScript declarations
if [ -f "wasm/index.d.ts" ]; then
    echo ""
    echo "Copying TypeScript declarations..."
    cp wasm/index.d.ts "$OUT_DIR/"
fi

# Calculate sizes
if [ -f "$OUT_DIR/delta_behavior_bg.wasm" ]; then
    WASM_SIZE=$(wc -c < "$OUT_DIR/delta_behavior_bg.wasm")
    WASM_SIZE_KB=$((WASM_SIZE / 1024))
    echo ""
    echo "=== Build Complete ==="
    echo "WASM size: ${WASM_SIZE_KB}KB ($WASM_SIZE bytes)"
fi

# List output files
echo ""
echo "Output files in $OUT_DIR:"
ls -la "$OUT_DIR/"

echo ""
echo "To use in a web project:"
echo "  import init, { WasmCoherence, WasmEventHorizon } from './$OUT_DIR/delta_behavior.js';"
echo "  await init();"
echo ""
echo "To use in Node.js (if built with --target nodejs):"
echo "  const { WasmCoherence, WasmEventHorizon } = require('./$OUT_DIR/delta_behavior.js');"
echo ""
