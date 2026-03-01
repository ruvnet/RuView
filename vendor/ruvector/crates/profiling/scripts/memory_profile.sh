#!/bin/bash
# Memory profiling script using valgrind and heaptrack

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
MEMORY_DIR="$SCRIPT_DIR/../memory"

mkdir -p "$MEMORY_DIR"

echo "💾 Running memory profiling..."

cd "$PROJECT_ROOT"

# Build in release mode
echo "Building release binary..."
cargo build --release --bin ruvector-cli 2>&1 | tee "$MEMORY_DIR/build.log"

# Run valgrind memcheck
echo "Running valgrind memcheck..."
valgrind --leak-check=full \
    --show-leak-kinds=all \
    --track-origins=yes \
    --verbose \
    --log-file="$MEMORY_DIR/valgrind_memcheck.txt" \
    target/release/ruvector-cli --version || echo "Valgrind memcheck completed with issues"

# Run valgrind massif (heap profiler)
echo "Running valgrind massif..."
valgrind --tool=massif \
    --massif-out-file="$MEMORY_DIR/massif.out" \
    target/release/ruvector-cli --version || echo "Massif completed"

# Generate massif report
echo "Generating massif report..."
ms_print "$MEMORY_DIR/massif.out" > "$MEMORY_DIR/massif_report.txt" || true

# Run heaptrack if available
if command -v heaptrack &> /dev/null; then
    echo "Running heaptrack..."
    heaptrack --output="$MEMORY_DIR/heaptrack.data" \
        target/release/ruvector-cli --version || echo "Heaptrack completed"

    echo "Analyzing heaptrack data..."
    heaptrack_print "$MEMORY_DIR/heaptrack.data" > "$MEMORY_DIR/heaptrack_report.txt" || true
else
    echo "Heaptrack not available, skipping..."
fi

echo "✅ Memory profiling complete!"
echo "Reports saved to: $MEMORY_DIR"
echo ""
echo "Key files:"
echo "  - valgrind_memcheck.txt: Memory leak analysis"
echo "  - massif_report.txt: Heap usage over time"
if command -v heaptrack &> /dev/null; then
    echo "  - heaptrack_report.txt: Detailed heap allocations"
fi
