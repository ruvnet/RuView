#!/bin/bash

# Performance Benchmark Runner Script for Psycho-Symbolic Reasoner
# This script runs all benchmark suites and generates comprehensive reports

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BENCHMARK_DIR="benches"
OUTPUT_DIR="benchmark_results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
RESULTS_DIR="${OUTPUT_DIR}/${TIMESTAMP}"

echo -e "${BLUE}🚀 Starting Psycho-Symbolic Reasoner Benchmark Suite${NC}"
echo -e "${BLUE}Timestamp: ${TIMESTAMP}${NC}"

# Create output directory
mkdir -p "${RESULTS_DIR}"

# Function to run a single benchmark
run_benchmark() {
    local benchmark_name=$1
    local description=$2

    echo -e "${YELLOW}📊 Running ${benchmark_name} - ${description}${NC}"

    # Run benchmark with HTML output
    if cargo bench --bench "${benchmark_name}" -- --output-format html > "${RESULTS_DIR}/${benchmark_name}.log" 2>&1; then
        echo -e "${GREEN}✅ ${benchmark_name} completed successfully${NC}"

        # Move HTML report if it exists
        if [ -d "target/criterion" ]; then
            cp -r "target/criterion" "${RESULTS_DIR}/${benchmark_name}_html"
        fi
    else
        echo -e "${RED}❌ ${benchmark_name} failed${NC}"
        echo "Check ${RESULTS_DIR}/${benchmark_name}.log for details"
    fi
}

# Function to run benchmark with profiling
run_profiled_benchmark() {
    local benchmark_name=$1
    local description=$2

    echo -e "${YELLOW}🔍 Running ${benchmark_name} with profiling - ${description}${NC}"

    # Check if perf is available
    if command -v perf &> /dev/null; then
        echo "Running with perf profiling..."
        perf record -g cargo bench --bench "${benchmark_name}" > "${RESULTS_DIR}/${benchmark_name}_profiled.log" 2>&1 || true
        if [ -f "perf.data" ]; then
            perf report > "${RESULTS_DIR}/${benchmark_name}_perf_report.txt" 2>/dev/null || true
            rm -f perf.data
        fi
    else
        echo "perf not available, running without profiling"
        cargo bench --bench "${benchmark_name}" > "${RESULTS_DIR}/${benchmark_name}_profiled.log" 2>&1 || true
    fi
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "${BENCHMARK_DIR}" ]; then
    echo -e "${RED}❌ Error: Must be run from the psycho-symbolic-reasoner root directory${NC}"
    exit 1
fi

# Build the project first
echo -e "${BLUE}🔨 Building project in release mode...${NC}"
if cargo build --release > "${RESULTS_DIR}/build.log" 2>&1; then
    echo -e "${GREEN}✅ Build successful${NC}"
else
    echo -e "${RED}❌ Build failed${NC}"
    cat "${RESULTS_DIR}/build.log"
    exit 1
fi

# System information
echo -e "${BLUE}💻 Collecting system information...${NC}"
{
    echo "=== System Information ==="
    echo "Date: $(date)"
    echo "Hostname: $(hostname)"
    echo "OS: $(uname -a)"
    echo "CPU: $(cat /proc/cpuinfo | grep 'model name' | head -1 | cut -d: -f2 | xargs)"
    echo "Memory: $(free -h | grep '^Mem:' | awk '{print $2}')"
    echo "Rust version: $(rustc --version)"
    echo "Cargo version: $(cargo --version)"
    echo ""
    echo "=== Git Information ==="
    echo "Branch: $(git branch --show-current 2>/dev/null || echo 'unknown')"
    echo "Commit: $(git rev-parse HEAD 2>/dev/null || echo 'unknown')"
    echo "Status: $(git status --porcelain 2>/dev/null | wc -l) modified files"
    echo ""
} > "${RESULTS_DIR}/system_info.txt"

# Start benchmark execution
echo -e "${BLUE}🏁 Starting benchmark execution...${NC}"

# Core performance benchmarks
run_benchmark "graph_reasoning" "Graph reasoning query performance and memory usage"
run_benchmark "text_extraction" "Text processing speed and accuracy"
run_benchmark "planning_algorithms" "Planning algorithm efficiency and scalability"

# Comparative benchmarks
run_benchmark "wasm_vs_native" "WASM vs Native Rust performance comparison"
run_benchmark "baseline_comparison" "Comparison against baseline AI reasoning systems"

# System benchmarks
run_benchmark "memory_usage" "Memory usage profiling for long-running processes"
run_benchmark "mcp_overhead" "MCP tool invocation overhead analysis"

# Quality assurance
run_benchmark "regression_tests" "Performance regression detection"

# Run selected benchmarks with profiling for bottleneck analysis
echo -e "${BLUE}🔍 Running profiled benchmarks for bottleneck analysis...${NC}"
run_profiled_benchmark "graph_reasoning" "Graph reasoning with profiling"
run_profiled_benchmark "planning_algorithms" "Planning algorithms with profiling"

# Generate comprehensive report
echo -e "${BLUE}📋 Generating comprehensive report...${NC}"

REPORT_FILE="${RESULTS_DIR}/benchmark_summary.md"

{
    echo "# Psycho-Symbolic Reasoner Benchmark Report"
    echo ""
    echo "**Generated:** $(date)"
    echo "**Duration:** $(date -d @${TIMESTAMP:8:2}${TIMESTAMP:10:2}${TIMESTAMP:12:2} '+%H:%M:%S') - $(date '+%H:%M:%S')"
    echo ""

    echo "## System Information"
    echo "\`\`\`"
    cat "${RESULTS_DIR}/system_info.txt"
    echo "\`\`\`"
    echo ""

    echo "## Benchmark Results Summary"
    echo ""

    # Process each benchmark log for summary
    for benchmark in graph_reasoning text_extraction planning_algorithms wasm_vs_native baseline_comparison memory_usage mcp_overhead regression_tests; do
        if [ -f "${RESULTS_DIR}/${benchmark}.log" ]; then
            echo "### ${benchmark}"
            echo ""

            # Extract key performance metrics from criterion output
            if grep -q "time:" "${RESULTS_DIR}/${benchmark}.log"; then
                echo "**Key Metrics:**"
                echo "\`\`\`"
                grep -A 2 -B 1 "time:" "${RESULTS_DIR}/${benchmark}.log" | head -20
                echo "\`\`\`"
            fi

            # Check for any warnings or errors
            if grep -i "regression\|warning\|error" "${RESULTS_DIR}/${benchmark}.log" >/dev/null 2>&1; then
                echo ""
                echo "**Warnings/Issues:**"
                echo "\`\`\`"
                grep -i "regression\|warning\|error" "${RESULTS_DIR}/${benchmark}.log" | head -10
                echo "\`\`\`"
            fi

            echo ""
        fi
    done

    echo "## Performance Analysis"
    echo ""
    echo "### Bottlenecks Identified"
    echo ""

    # Analyze profiling data if available
    for profile in "${RESULTS_DIR}"/*_perf_report.txt; do
        if [ -f "$profile" ]; then
            benchmark_name=$(basename "$profile" _perf_report.txt)
            echo "#### ${benchmark_name}"
            echo "\`\`\`"
            head -20 "$profile"
            echo "\`\`\`"
            echo ""
        fi
    done

    echo "## Recommendations"
    echo ""
    echo "Based on the benchmark results, consider the following optimizations:"
    echo ""
    echo "1. **Memory Usage**: Monitor for potential memory leaks in long-running processes"
    echo "2. **Concurrency**: Evaluate opportunities for parallel processing"
    echo "3. **Algorithm Efficiency**: Focus on components with highest time complexity"
    echo "4. **WASM Optimization**: Consider native alternatives for performance-critical paths"
    echo "5. **Caching**: Implement caching for frequently accessed data"
    echo ""

    echo "## Files Generated"
    echo ""
    echo "- \`system_info.txt\`: System configuration details"
    echo "- \`*.log\`: Detailed benchmark output for each test suite"
    echo "- \`*_html/\`: HTML reports with interactive charts (if available)"
    echo "- \`*_perf_report.txt\`: Profiling data for bottleneck analysis"
    echo ""

} > "$REPORT_FILE"

# Check for regressions and critical issues
echo -e "${BLUE}🔍 Analyzing results for critical issues...${NC}"

ISSUES_FOUND=0

# Check for performance regressions
if grep -r -i "regression detected" "${RESULTS_DIR}"/*.log >/dev/null 2>&1; then
    echo -e "${RED}⚠️  Performance regressions detected!${NC}"
    grep -r -i "regression detected" "${RESULTS_DIR}"/*.log
    ISSUES_FOUND=1
fi

# Check for memory leaks
if grep -r -i "memory leak" "${RESULTS_DIR}"/*.log >/dev/null 2>&1; then
    echo -e "${RED}⚠️  Potential memory leaks detected!${NC}"
    grep -r -i "memory leak" "${RESULTS_DIR}"/*.log
    ISSUES_FOUND=1
fi

# Check for failed benchmarks
if grep -r -i "error\|failed" "${RESULTS_DIR}"/*.log >/dev/null 2>&1; then
    echo -e "${YELLOW}⚠️  Some benchmarks had errors (check logs for details)${NC}"
fi

# Generate optimization recommendations
echo -e "${BLUE}💡 Generating optimization recommendations...${NC}"

OPTIMIZATION_FILE="${RESULTS_DIR}/optimization_recommendations.md"

{
    echo "# Performance Optimization Recommendations"
    echo ""
    echo "## High Priority Optimizations"
    echo ""
    echo "### 1. Memory Management"
    echo "- Implement object pooling for frequently allocated structures"
    echo "- Use memory-mapped files for large datasets"
    echo "- Consider arena allocation for temporary objects"
    echo ""
    echo "### 2. Algorithm Optimizations"
    echo "- Implement parallel graph traversal for large knowledge graphs"
    echo "- Use bloom filters for existence checks before expensive operations"
    echo "- Cache inference results to avoid recomputation"
    echo ""
    echo "### 3. WASM Performance"
    echo "- Use SIMD instructions where available"
    echo "- Minimize allocations in hot paths"
    echo "- Consider using Web Workers for concurrent processing"
    echo ""
    echo "## Medium Priority Optimizations"
    echo ""
    echo "### 1. I/O Optimization"
    echo "- Implement batch processing for MCP tool invocations"
    echo "- Use connection pooling for external services"
    echo "- Consider streaming for large data transfers"
    echo ""
    echo "### 2. Caching Strategy"
    echo "- Implement LRU cache for query results"
    echo "- Cache compiled regex patterns"
    echo "- Store preprocessed text analysis models"
    echo ""
    echo "## Low Priority Optimizations"
    echo ""
    echo "### 1. Code Quality"
    echo "- Profile and optimize hot functions"
    echo "- Reduce allocations in inner loops"
    echo "- Use more efficient data structures where appropriate"
    echo ""
    echo "### 2. Monitoring"
    echo "- Add performance counters for key operations"
    echo "- Implement automatic performance regression detection"
    echo "- Create alerting for performance degradation"
    echo ""
} > "$OPTIMIZATION_FILE"

# Create index file
INDEX_FILE="${RESULTS_DIR}/index.html"

{
    echo "<!DOCTYPE html>"
    echo "<html><head><title>Benchmark Results - ${TIMESTAMP}</title></head><body>"
    echo "<h1>Psycho-Symbolic Reasoner Benchmark Results</h1>"
    echo "<p><strong>Generated:</strong> $(date)</p>"
    echo "<h2>Reports</h2>"
    echo "<ul>"
    echo "<li><a href='benchmark_summary.md'>Benchmark Summary</a></li>"
    echo "<li><a href='optimization_recommendations.md'>Optimization Recommendations</a></li>"
    echo "<li><a href='system_info.txt'>System Information</a></li>"
    echo "</ul>"
    echo "<h2>Detailed Results</h2>"
    echo "<ul>"
    for log in "${RESULTS_DIR}"/*.log; do
        if [ -f "$log" ]; then
            basename_log=$(basename "$log")
            echo "<li><a href='$basename_log'>$basename_log</a></li>"
        fi
    done
    echo "</ul>"
    echo "<h2>HTML Reports</h2>"
    echo "<ul>"
    for html_dir in "${RESULTS_DIR}"/*_html; do
        if [ -d "$html_dir" ]; then
            basename_html=$(basename "$html_dir")
            echo "<li><a href='$basename_html/index.html'>$basename_html</a></li>"
        fi
    done
    echo "</ul>"
    echo "</body></html>"
} > "$INDEX_FILE"

# Final summary
echo ""
echo -e "${GREEN}🎉 Benchmark suite completed!${NC}"
echo -e "${BLUE}📁 Results saved to: ${RESULTS_DIR}${NC}"
echo -e "${BLUE}📊 Summary report: ${REPORT_FILE}${NC}"
echo -e "${BLUE}💡 Optimization recommendations: ${OPTIMIZATION_FILE}${NC}"
echo -e "${BLUE}🌐 Index page: ${INDEX_FILE}${NC}"

if [ $ISSUES_FOUND -eq 1 ]; then
    echo ""
    echo -e "${RED}⚠️  Critical issues found - please review the results!${NC}"
    exit 1
else
    echo ""
    echo -e "${GREEN}✅ No critical issues detected${NC}"
fi

echo ""
echo -e "${BLUE}To view results in browser: open ${INDEX_FILE}${NC}"