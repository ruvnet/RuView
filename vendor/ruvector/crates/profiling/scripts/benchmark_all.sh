#!/bin/bash
# Comprehensive benchmarking script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BENCHMARK_DIR="$SCRIPT_DIR/../benchmarks"

mkdir -p "$BENCHMARK_DIR"

echo "📊 Running comprehensive benchmark suite..."

cd "$PROJECT_ROOT"

# Get CPU info
echo "CPU Information:" > "$BENCHMARK_DIR/system_info.txt"
lscpu >> "$BENCHMARK_DIR/system_info.txt" 2>&1 || true
echo "" >> "$BENCHMARK_DIR/system_info.txt"
echo "Memory Information:" >> "$BENCHMARK_DIR/system_info.txt"
free -h >> "$BENCHMARK_DIR/system_info.txt" 2>&1 || true

# Build with different optimization levels
echo "Building with optimizations..."
cargo build --release

# Run criterion benchmarks
echo "Running criterion benchmarks..."
cargo bench --bench distance_metrics -- --save-baseline before 2>&1 | tee "$BENCHMARK_DIR/distance_metrics.txt"
cargo bench --bench hnsw_search -- --save-baseline before 2>&1 | tee "$BENCHMARK_DIR/hnsw_search.txt"

# Test with different thread counts
echo "Benchmarking with different thread counts..."
for threads in 1 2 4 8 16 32; do
    echo "Testing with $threads threads..."
    RAYON_NUM_THREADS=$threads cargo bench --bench distance_metrics -- --profile-time=5 \
        2>&1 | tee "$BENCHMARK_DIR/threads_${threads}.txt"
done

# Generate summary
echo "Generating benchmark summary..."
cat > "$BENCHMARK_DIR/summary.txt" << 'EOF'
# Ruvector Performance Benchmark Summary

## Test Environment
$(cat system_info.txt)

## Benchmark Results

### Distance Metrics
$(grep "time:" distance_metrics.txt | head -20)

### HNSW Search
$(grep "time:" hnsw_search.txt | head -20)

### Thread Scaling
EOF

for threads in 1 2 4 8 16 32; do
    echo "#### $threads threads" >> "$BENCHMARK_DIR/summary.txt"
    grep "time:" "$BENCHMARK_DIR/threads_${threads}.txt" | head -5 >> "$BENCHMARK_DIR/summary.txt" || true
    echo "" >> "$BENCHMARK_DIR/summary.txt"
done

echo "✅ Benchmark suite complete!"
echo "Results saved to: $BENCHMARK_DIR"
echo ""
echo "Key files:"
echo "  - distance_metrics.txt: Distance calculation benchmarks"
echo "  - hnsw_search.txt: HNSW search benchmarks"
echo "  - threads_*.txt: Thread scaling tests"
echo "  - summary.txt: Overall benchmark summary"
