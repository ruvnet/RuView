#!/usr/bin/env bash
# EXO-AI 2025 Benchmark Runner
# Performance analysis suite for cognitive substrate

set -e

PROJECT_ROOT="/home/user/ruvector/examples/exo-ai-2025"
RESULTS_DIR="$PROJECT_ROOT/target/criterion"

cd "$PROJECT_ROOT"

echo "======================================"
echo "EXO-AI 2025 Performance Benchmarks"
echo "======================================"
echo ""

# Check if crates compile first
echo "Step 1: Checking crate compilation..."
if cargo check --benches; then
    echo "✓ All crates compile successfully"
else
    echo "✗ Compilation errors detected. Please fix before benchmarking."
    exit 1
fi

echo ""
echo "Step 2: Running benchmark suites..."
echo ""

# Run all benchmarks
echo "→ Running Manifold benchmarks..."
cargo bench --bench manifold_bench

echo ""
echo "→ Running Hypergraph benchmarks..."
cargo bench --bench hypergraph_bench

echo ""
echo "→ Running Temporal benchmarks..."
cargo bench --bench temporal_bench

echo ""
echo "→ Running Federation benchmarks..."
cargo bench --bench federation_bench

echo ""
echo "======================================"
echo "Benchmark Complete!"
echo "======================================"
echo ""
echo "Results saved to: $RESULTS_DIR"
echo "HTML reports available at: $RESULTS_DIR/report/index.html"
echo ""
echo "To compare against baseline:"
echo "  cargo bench -- --save-baseline initial"
echo "  cargo bench -- --baseline initial"
echo ""
