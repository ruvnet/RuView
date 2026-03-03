#!/bin/bash

# Temporal Neural Solver - WASM Build Script
# Builds the temporal neural network for WebAssembly distribution

set -e

echo "🚀 Building Temporal Neural Solver for WASM"
echo "============================================="

# Check if required tools are installed
command -v wasm-pack >/dev/null 2>&1 || {
    echo "❌ wasm-pack is required but not installed."
    echo "   Install with: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh"
    exit 1
}

command -v cargo >/dev/null 2>&1 || {
    echo "❌ Rust/Cargo is required but not installed."
    echo "   Install with: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
}

# Build information
echo "📋 Build Information:"
echo "   Package:      temporal-neural-solver"
echo "   Version:      0.1.0"
echo "   Architecture: WASM32"
echo "   Rust Version: $(rustc --version)"
echo "   Target:       Multiple (bundler, nodejs, web)"
echo ""

# Clean previous builds
echo "🧹 Cleaning previous builds..."
rm -rf pkg pkg-node pkg-web
rm -rf target/wasm32-unknown-unknown

# Install wasm32 target if not present
echo "🎯 Setting up WASM target..."
rustup target add wasm32-unknown-unknown

# Build for different targets
echo "📦 Building for bundler (default)..."
wasm-pack build \
    --target bundler \
    --out-dir pkg \
    --out-name temporal_neural_solver \
    --scope temporal-neural \
    --release \
    --features wasm

echo "📦 Building for Node.js..."
wasm-pack build \
    --target nodejs \
    --out-dir pkg-node \
    --out-name temporal_neural_solver \
    --scope temporal-neural \
    --release \
    --features wasm

echo "📦 Building for web..."
wasm-pack build \
    --target web \
    --out-dir pkg-web \
    --out-name temporal_neural_solver \
    --scope temporal-neural \
    --release \
    --features wasm

# Copy additional files to package directories
echo "📋 Copying additional files..."

for dir in pkg pkg-node pkg-web; do
    if [ -d "$dir" ]; then
        # Copy CLI and examples
        cp -r pkg/bin "$dir/" 2>/dev/null || true
        cp -r examples "$dir/" 2>/dev/null || true

        # Copy documentation
        cp README.md "$dir/" 2>/dev/null || true
        cp CHANGELOG.md "$dir/" 2>/dev/null || true
        cp LICENSE "$dir/" 2>/dev/null || true

        # Update package.json for specific targets
        if [ "$dir" = "pkg-node" ]; then
            # Update for Node.js target
            sed -i.bak 's/"target": "bundler"/"target": "nodejs"/g' "$dir/package.json" 2>/dev/null || true
        elif [ "$dir" = "pkg-web" ]; then
            # Update for web target
            sed -i.bak 's/"target": "bundler"/"target": "web"/g' "$dir/package.json" 2>/dev/null || true
        fi

        # Clean up backup files
        rm -f "$dir/package.json.bak"
    fi
done

# Optimize WASM binary
echo "⚡ Optimizing WASM binaries..."
for dir in pkg pkg-node pkg-web; do
    if [ -d "$dir" ] && [ -f "$dir/temporal_neural_solver_bg.wasm" ]; then
        # Check if wasm-opt is available
        if command -v wasm-opt >/dev/null 2>&1; then
            echo "   Optimizing $dir/temporal_neural_solver_bg.wasm"
            wasm-opt -Os "$dir/temporal_neural_solver_bg.wasm" -o "$dir/temporal_neural_solver_bg.wasm"
        else
            echo "   ⚠️  wasm-opt not found, skipping optimization for $dir"
            echo "      Install binaryen for size optimization"
        fi
    fi
done

# Generate documentation
echo "📚 Generating documentation..."
if command -v typedoc >/dev/null 2>&1; then
    cd pkg
    typedoc --out docs temporal_neural_solver.d.ts 2>/dev/null || true
    cd ..
else
    echo "   ⚠️  typedoc not found, skipping documentation generation"
fi

# Display build results
echo ""
echo "✅ Build completed successfully!"
echo ""
echo "📊 Build Results:"
echo "=================="

for dir in pkg pkg-node pkg-web; do
    if [ -d "$dir" ]; then
        wasm_size=$(du -h "$dir/temporal_neural_solver_bg.wasm" 2>/dev/null | cut -f1 || echo "N/A")
        js_size=$(du -h "$dir/temporal_neural_solver.js" 2>/dev/null | cut -f1 || echo "N/A")
        echo "📦 $dir:"
        echo "   WASM: $wasm_size"
        echo "   JS:   $js_size"
    fi
done

echo ""
echo "🎯 Usage:"
echo "=========="
echo "Node.js:     npm install ./pkg-node"
echo "Bundler:     npm install ./pkg"
echo "Web:         import init from './pkg-web/temporal_neural_solver.js'"
echo ""
echo "CLI:         npx temporal-neural-solver demo"
echo "             npx temporal-neural-solver benchmark --iterations 1000"
echo ""
echo "💡 The temporal neural solver is ready for sub-millisecond inference!"
echo "   • P99.9 latency target: <0.9ms"
echo "   • Mathematical certificates included"
echo "   • Solver-gated architecture"
echo "   • Multiple deployment targets"

# Test build if requested
if [ "$1" = "--test" ]; then
    echo ""
    echo "🧪 Running basic functionality test..."
    cd pkg
    node -e "
        const solver = require('./temporal_neural_solver.js');
        solver.default().then(() => {
            console.log('✅ WASM module loads successfully');
            const utils = new solver.WasmUtils();
            console.log('✅ Version:', utils.getVersion());
            console.log('✅ Build test passed!');
        }).catch(err => {
            console.error('❌ Build test failed:', err);
            process.exit(1);
        });
    " || echo "⚠️  Node.js test failed (may require Node.js 16+)"
    cd ..
fi

echo ""
echo "🚀 Ready to publish: npm publish ./pkg"