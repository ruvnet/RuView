#!/bin/bash
# CPU profiling script using perf

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
REPORTS_DIR="$SCRIPT_DIR/../reports"

mkdir -p "$REPORTS_DIR"

echo "🔥 Running CPU profiling with perf..."
echo "Project root: $PROJECT_ROOT"

cd "$PROJECT_ROOT"

# Build in release mode with debug symbols
echo "Building with debug symbols..."
CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release

# Run perf record on benchmarks
echo "Recording performance data..."
sudo perf record -F 99 -g --call-graph=dwarf \
    cargo bench --bench distance_metrics -- --profile-time=10 \
    2>&1 | tee "$REPORTS_DIR/perf_record.log" || true

# Generate perf report
echo "Generating perf report..."
sudo perf report --stdio > "$REPORTS_DIR/perf_report.txt" || true
sudo perf report --stdio --sort=dso,symbol > "$REPORTS_DIR/perf_report_detailed.txt" || true

# Generate annotated source
echo "Generating annotated source..."
sudo perf annotate --stdio > "$REPORTS_DIR/perf_annotate.txt" || true

# Analyze cache performance
echo "Analyzing cache performance..."
sudo perf stat -e cache-references,cache-misses,L1-dcache-loads,L1-dcache-load-misses \
    cargo bench --bench distance_metrics -- --profile-time=5 \
    2>&1 | tee "$REPORTS_DIR/cache_stats.txt" || true

echo "✅ CPU profiling complete!"
echo "Reports saved to: $REPORTS_DIR"
echo ""
echo "Key files:"
echo "  - perf_report.txt: Overall performance report"
echo "  - perf_report_detailed.txt: Detailed symbol analysis"
echo "  - perf_annotate.txt: Annotated source code"
echo "  - cache_stats.txt: Cache performance statistics"

# Cleanup
sudo chown -R $USER:$USER perf.data "$REPORTS_DIR" 2>/dev/null || true
