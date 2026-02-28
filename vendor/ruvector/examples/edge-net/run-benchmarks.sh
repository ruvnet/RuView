#!/bin/bash
# Edge-Net Performance Benchmark Runner
# Usage: ./run-benchmarks.sh [--baseline|--compare|--profile]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "========================================"
echo "Edge-Net Performance Benchmark Suite"
echo "========================================"
echo ""

# Check if cargo bench is available
if ! command -v cargo &> /dev/null; then
    echo "Error: cargo not found. Please install Rust toolchain."
    exit 1
fi

# Parse arguments
MODE="run"
if [ "$1" == "--baseline" ]; then
    MODE="baseline"
elif [ "$1" == "--compare" ]; then
    MODE="compare"
elif [ "$1" == "--profile" ]; then
    MODE="profile"
fi

case $MODE in
    baseline)
        echo "Creating performance baseline..."
        cargo bench --features=bench 2>&1 | tee benchmarks-baseline.txt
        echo ""
        echo "✅ Baseline saved to: benchmarks-baseline.txt"
        ;;
    
    compare)
        if [ ! -f "benchmarks-baseline.txt" ]; then
            echo "Error: No baseline found. Run with --baseline first."
            exit 1
        fi
        echo "Running benchmarks and comparing with baseline..."
        cargo bench --features=bench 2>&1 | tee benchmarks-current.txt
        echo ""
        echo "Comparison Report:"
        echo "=================="
        echo "Baseline file: benchmarks-baseline.txt"
        echo "Current file:  benchmarks-current.txt"
        echo ""
        echo "To compare, install cargo-benchcmp:"
        echo "  cargo install cargo-benchcmp"
        echo "  cargo benchcmp benchmarks-baseline.txt benchmarks-current.txt"
        ;;
    
    profile)
        echo "Running with profiling (flamegraph)..."
        if ! command -v cargo-flamegraph &> /dev/null; then
            echo "Installing cargo-flamegraph..."
            cargo install flamegraph
        fi
        cargo flamegraph --bench benchmarks --features=bench
        echo ""
        echo "✅ Flamegraph saved to: flamegraph.svg"
        echo "Open with: firefox flamegraph.svg (or your browser)"
        ;;
    
    *)
        echo "Running all benchmarks..."
        echo ""
        cargo bench --features=bench
        echo ""
        echo "✅ Benchmarks complete!"
        echo ""
        echo "Usage:"
        echo "  ./run-benchmarks.sh              # Run benchmarks"
        echo "  ./run-benchmarks.sh --baseline   # Save baseline"
        echo "  ./run-benchmarks.sh --compare    # Compare with baseline"
        echo "  ./run-benchmarks.sh --profile    # Generate flamegraph"
        ;;
esac

echo ""
echo "Performance reports available:"
echo "  - PERFORMANCE_ANALYSIS.md"
echo "  - OPTIMIZATIONS_APPLIED.md"
echo "  - OPTIMIZATION_SUMMARY.md"
