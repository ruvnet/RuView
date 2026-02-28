#!/bin/bash
# Run complete Ruvector benchmark suite

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BENCH_DIR="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="${BENCH_DIR}/bench_results"

echo "╔════════════════════════════════════════╗"
echo "║   Ruvector Benchmark Suite Runner     ║"
echo "╚════════════════════════════════════════╝"
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Parse arguments
QUICK_MODE=false
PROFILE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --quick)
            QUICK_MODE=true
            shift
            ;;
        --profile)
            PROFILE=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--quick] [--profile]"
            exit 1
            ;;
    esac
done

# Set benchmark parameters based on mode
if [ "$QUICK_MODE" = true ]; then
    echo "Running in QUICK mode (reduced dataset sizes)..."
    VECTORS=10000
    QUERIES=500
else
    echo "Running in FULL mode (standard dataset sizes)..."
    VECTORS=100000
    QUERIES=1000
fi

echo "Output directory: $OUTPUT_DIR"
echo ""

# Build benchmarks
echo "═══════════════════════════════════════════════════════════════"
echo "Building benchmark suite..."
echo "═══════════════════════════════════════════════════════════════"
cd "$BENCH_DIR"
cargo build --release
echo "✓ Build complete"
echo ""

# Run ANN Benchmarks
echo "═══════════════════════════════════════════════════════════════"
echo "1. ANN Benchmarks (SIFT/GIST/Deep1M compatibility)"
echo "═══════════════════════════════════════════════════════════════"
cargo run --release --bin ann-benchmark -- \
    --dataset synthetic \
    --num-vectors $VECTORS \
    --queries $QUERIES \
    --dimensions 128 \
    --output "$OUTPUT_DIR"
echo ""

# Run AgenticDB Benchmarks
echo "═══════════════════════════════════════════════════════════════"
echo "2. AgenticDB Workload Benchmarks"
echo "═══════════════════════════════════════════════════════════════"
cargo run --release --bin agenticdb-benchmark -- \
    --episodes $VECTORS \
    --skills $(($VECTORS / 10)) \
    --queries $QUERIES \
    --output "$OUTPUT_DIR"
echo ""

# Run Latency Benchmarks
echo "═══════════════════════════════════════════════════════════════"
echo "3. Latency Profiling"
echo "═══════════════════════════════════════════════════════════════"
cargo run --release --bin latency-benchmark -- \
    --num-vectors $(($VECTORS / 2)) \
    --queries $QUERIES \
    --dimensions 384 \
    --threads "1,4,8" \
    --output "$OUTPUT_DIR"
echo ""

# Run Memory Benchmarks
echo "═══════════════════════════════════════════════════════════════"
echo "4. Memory Profiling"
echo "═══════════════════════════════════════════════════════════════"
if [ "$QUICK_MODE" = true ]; then
    SCALES="1000,10000"
else
    SCALES="1000,10000,100000"
fi

cargo run --release --bin memory-benchmark -- \
    --dimensions 384 \
    --scales "$SCALES" \
    --output "$OUTPUT_DIR"
echo ""

# Run Comparison Benchmarks
echo "═══════════════════════════════════════════════════════════════"
echo "5. Cross-System Comparison"
echo "═══════════════════════════════════════════════════════════════"
cargo run --release --bin comparison-benchmark -- \
    --num-vectors $(($VECTORS / 2)) \
    --queries $QUERIES \
    --dimensions 384 \
    --output "$OUTPUT_DIR"
echo ""

# Run Profiling (optional)
if [ "$PROFILE" = true ]; then
    echo "═══════════════════════════════════════════════════════════════"
    echo "6. Performance Profiling with Flamegraph"
    echo "═══════════════════════════════════════════════════════════════"
    cargo run --release --features profiling --bin profiling-benchmark -- \
        --num-vectors $(($VECTORS / 2)) \
        --queries $QUERIES \
        --dimensions 384 \
        --flamegraph \
        --output "$OUTPUT_DIR/profiling"
    echo ""
fi

# Generate summary report
echo "═══════════════════════════════════════════════════════════════"
echo "Generating Summary Report"
echo "═══════════════════════════════════════════════════════════════"

SUMMARY_FILE="$OUTPUT_DIR/SUMMARY.md"

cat > "$SUMMARY_FILE" << EOF
# Ruvector Benchmark Results Summary

**Generated:** $(date)
**Mode:** $([ "$QUICK_MODE" = true ] && echo "Quick" || echo "Full")

## Configuration
- Vectors: $VECTORS
- Queries: $QUERIES
- Profiling: $([ "$PROFILE" = true ] && echo "Enabled" || echo "Disabled")

## Results Location
All benchmark results are saved in: \`$OUTPUT_DIR\`

## Available Reports

### 1. ANN Benchmarks
- JSON: \`ann_benchmark.json\`
- CSV: \`ann_benchmark.csv\`
- Report: \`ann_benchmark.md\`

### 2. AgenticDB Workloads
- JSON: \`agenticdb_benchmark.json\`
- CSV: \`agenticdb_benchmark.csv\`
- Report: \`agenticdb_benchmark.md\`

### 3. Latency Profiling
- JSON: \`latency_benchmark.json\`
- CSV: \`latency_benchmark.csv\`
- Report: \`latency_benchmark.md\`

### 4. Memory Profiling
- JSON: \`memory_benchmark.json\`
- CSV: \`memory_benchmark.csv\`
- Report: \`memory_benchmark.md\`

### 5. System Comparison
- JSON: \`comparison_benchmark.json\`
- CSV: \`comparison_benchmark.csv\`
- Report: \`comparison_benchmark.md\`

EOF

if [ "$PROFILE" = true ]; then
    cat >> "$SUMMARY_FILE" << EOF

### 6. Performance Profiling
- Flamegraph: \`profiling/flamegraph.svg\`
- Profile: \`profiling/profile.txt\`

EOF
fi

cat >> "$SUMMARY_FILE" << EOF

## Quick Analysis

To view individual benchmark reports, use:
\`\`\`bash
cat $OUTPUT_DIR/ann_benchmark.md
cat $OUTPUT_DIR/agenticdb_benchmark.md
cat $OUTPUT_DIR/latency_benchmark.md
cat $OUTPUT_DIR/memory_benchmark.md
cat $OUTPUT_DIR/comparison_benchmark.md
\`\`\`

To view CSV data for analysis:
\`\`\`bash
column -t -s, $OUTPUT_DIR/ann_benchmark.csv | less -S
\`\`\`

EOF

echo "✓ Summary report generated: $SUMMARY_FILE"
echo ""

echo "════════════════════════════════════════════════════════════════"
echo "✓ All benchmarks complete!"
echo "════════════════════════════════════════════════════════════════"
echo ""
echo "Results saved to: $OUTPUT_DIR"
echo "Summary report: $SUMMARY_FILE"
echo ""
echo "View results:"
echo "  cat $SUMMARY_FILE"
echo "  cat $OUTPUT_DIR/*.md"
echo ""

# Display quick stats if available
if [ -f "$OUTPUT_DIR/comparison_benchmark.csv" ]; then
    echo "Quick Performance Summary:"
    echo "─────────────────────────────────────────"
    grep "ruvector_optimized" "$OUTPUT_DIR/comparison_benchmark.csv" | \
        awk -F',' '{printf "  Optimized QPS: %s\n  Latency p50: %sms\n  Latency p99: %sms\n", $7, $8, $10}'
    echo ""
fi

echo "To run again:"
echo "  ./scripts/run_all_benchmarks.sh           # Full benchmarks"
echo "  ./scripts/run_all_benchmarks.sh --quick   # Quick mode"
echo "  ./scripts/run_all_benchmarks.sh --profile # With profiling"
