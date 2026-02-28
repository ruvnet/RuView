#!/bin/bash
# Benchmark runner with baseline comparison for RuVector
# Integrates with criterion benchmarks and stores results

set -e

CRATE="${1:-all}"
BASELINE_DIR="/workspaces/ruvector/.claude-flow/metrics/benchmarks"
mkdir -p "$BASELINE_DIR"

cd /workspaces/ruvector

echo "📊 RuVector Benchmark Runner"
echo "============================"
echo ""

run_bench() {
    local crate=$1
    local bench_name=$2
    local output_file="$BASELINE_DIR/${crate}-$(date +%Y%m%d-%H%M%S).json"

    echo "🏃 Running: cargo bench -p $crate"

    # Run benchmark and capture output
    if cargo bench -p "$crate" -- --noplot 2>&1 | tee /tmp/bench-output.txt; then
        # Extract timing info from criterion output
        grep -E "time:" /tmp/bench-output.txt | head -10

        # Store raw output
        cp /tmp/bench-output.txt "$output_file.txt"
        echo ""
        echo "📁 Results saved to: $output_file.txt"
    else
        echo "⚠️  Benchmark failed for $crate"
    fi
}

case "$CRATE" in
    "all")
        echo "Running all available benchmarks..."
        echo ""

        # Core benchmarks
        if [ -d "crates/ruvector-bench" ]; then
            run_bench "ruvector-bench" "core"
        fi

        # MinCut benchmarks
        if [ -d "crates/ruvector-mincut" ]; then
            run_bench "ruvector-mincut" "mincut"
        fi

        # Attention benchmarks
        if [ -d "crates/ruvector-attention" ]; then
            run_bench "ruvector-attention" "attention"
        fi
        ;;

    "core"|"ruvector-bench")
        run_bench "ruvector-bench" "core"
        ;;

    "mincut"|"ruvector-mincut")
        run_bench "ruvector-mincut" "mincut"
        ;;

    "attention"|"ruvector-attention")
        run_bench "ruvector-attention" "attention"
        ;;

    "graph"|"ruvector-graph")
        run_bench "ruvector-graph" "graph"
        ;;

    "quick")
        echo "Running quick sanity benchmarks..."
        cargo bench -p ruvector-bench -- --noplot "insert" 2>&1 | tail -10
        ;;

    *)
        echo "Usage: $0 [all|core|mincut|attention|graph|quick|<crate-name>]"
        echo ""
        echo "Available benchmark crates:"
        echo "  core/ruvector-bench  - Core vector operations"
        echo "  mincut               - Min-cut algorithms"
        echo "  attention            - Attention mechanisms"
        echo "  graph                - Graph operations"
        echo "  quick                - Fast sanity check"
        exit 1
        ;;
esac

echo ""
echo "✅ Benchmarks complete"
echo "📁 Results in: $BASELINE_DIR/"
