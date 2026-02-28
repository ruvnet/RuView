#!/bin/bash
# Comprehensive profiling and analysis script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "=========================================="
echo " Ruvector Performance Analysis Suite"
echo "=========================================="
echo ""
echo "Project: $PROJECT_ROOT"
echo "Date: $(date)"
echo ""

cd "$PROJECT_ROOT"

# Step 1: Install tools
echo "Step 1: Checking tools..."
"$SCRIPT_DIR/install_tools.sh" || echo "Some tools may not be available"
echo ""

# Step 2: Run benchmarks (baseline)
echo "Step 2: Running baseline benchmarks..."
"$SCRIPT_DIR/benchmark_all.sh"
echo ""

# Step 3: CPU profiling
echo "Step 3: CPU profiling..."
"$SCRIPT_DIR/cpu_profile.sh"
echo ""

# Step 4: Generate flamegraphs
echo "Step 4: Generating flamegraphs..."
"$SCRIPT_DIR/generate_flamegraph.sh"
echo ""

# Step 5: Memory profiling
echo "Step 5: Memory profiling..."
"$SCRIPT_DIR/memory_profile.sh"
echo ""

# Step 6: Generate comprehensive report
echo "Step 6: Generating comprehensive report..."

REPORT_FILE="$SCRIPT_DIR/../reports/COMPREHENSIVE_REPORT.md"

cat > "$REPORT_FILE" << 'REPORT_HEADER'
# Ruvector Performance Analysis Report

**Generated**: $(date)
**System**: $(uname -a)
**CPU**: $(lscpu | grep "Model name" | sed 's/Model name: *//')

## Executive Summary

This report contains comprehensive performance analysis of Ruvector vector database including:

- CPU profiling and hotspot analysis
- Memory allocation patterns
- Cache utilization
- Thread scaling characteristics
- SIMD optimization effectiveness
- Lock-free data structure performance

REPORT_HEADER

echo "" >> "$REPORT_FILE"
echo "## System Information" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo '```' >> "$REPORT_FILE"
cat "$SCRIPT_DIR/../benchmarks/system_info.txt" >> "$REPORT_FILE" 2>/dev/null || echo "System info not available"
echo '```' >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

echo "## Benchmark Results" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo "### Distance Metrics Performance" >> "$REPORT_FILE"
echo '```' >> "$REPORT_FILE"
grep "time:" "$SCRIPT_DIR/../benchmarks/distance_metrics.txt" | head -20 >> "$REPORT_FILE" 2>/dev/null || echo "Benchmarks not available"
echo '```' >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

echo "### HNSW Search Performance" >> "$REPORT_FILE"
echo '```' >> "$REPORT_FILE"
grep "time:" "$SCRIPT_DIR/../benchmarks/hnsw_search.txt" | head -20 >> "$REPORT_FILE" 2>/dev/null || echo "Benchmarks not available"
echo '```' >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

echo "### Thread Scaling" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
for threads in 1 2 4 8 16 32; do
    if [ -f "$SCRIPT_DIR/../benchmarks/threads_${threads}.txt" ]; then
        echo "#### ${threads} threads" >> "$REPORT_FILE"
        echo '```' >> "$REPORT_FILE"
        grep "time:" "$SCRIPT_DIR/../benchmarks/threads_${threads}.txt" | head -5 >> "$REPORT_FILE"
        echo '```' >> "$REPORT_FILE"
        echo "" >> "$REPORT_FILE"
    fi
done

echo "## CPU Profiling Analysis" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo "### Top Hotspots" >> "$REPORT_FILE"
echo '```' >> "$REPORT_FILE"
head -50 "$SCRIPT_DIR/../reports/perf_report.txt" >> "$REPORT_FILE" 2>/dev/null || echo "Perf report not available"
echo '```' >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

echo "### Cache Performance" >> "$REPORT_FILE"
echo '```' >> "$REPORT_FILE"
cat "$SCRIPT_DIR/../reports/cache_stats.txt" >> "$REPORT_FILE" 2>/dev/null || echo "Cache stats not available"
echo '```' >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

echo "## Memory Analysis" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo "### Massif Heap Profile" >> "$REPORT_FILE"
echo '```' >> "$REPORT_FILE"
head -100 "$SCRIPT_DIR/../memory/massif_report.txt" >> "$REPORT_FILE" 2>/dev/null || echo "Massif report not available"
echo '```' >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

echo "## Recommendations" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo "Based on the analysis:" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo "1. **CPU Optimization**: Review flamegraphs to identify hotspots" >> "$REPORT_FILE"
echo "2. **Memory Optimization**: Check for allocation patterns in hot paths" >> "$REPORT_FILE"
echo "3. **Cache Optimization**: Analyze cache miss rates and data structures" >> "$REPORT_FILE"
echo "4. **Parallelization**: Evaluate thread scaling efficiency" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo "See detailed optimization guides in /docs/optimization/" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

echo "✅ Comprehensive analysis complete!"
echo ""
echo "=========================================="
echo " Analysis Results Summary"
echo "=========================================="
echo ""
echo "Reports Directory: $SCRIPT_DIR/../reports"
echo "Flamegraphs: $SCRIPT_DIR/../flamegraphs"
echo "Memory Analysis: $SCRIPT_DIR/../memory"
echo "Benchmarks: $SCRIPT_DIR/../benchmarks"
echo ""
echo "📊 Comprehensive Report: $REPORT_FILE"
echo ""
echo "Next Steps:"
echo "1. Review flamegraphs: firefox $SCRIPT_DIR/../flamegraphs/*.svg"
echo "2. Check benchmark results: cat $SCRIPT_DIR/../benchmarks/summary.txt"
echo "3. Analyze CPU hotspots: cat $SCRIPT_DIR/../reports/perf_report.txt"
echo "4. Review memory usage: cat $SCRIPT_DIR/../memory/massif_report.txt"
echo ""
