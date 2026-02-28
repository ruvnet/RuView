#!/usr/bin/env bash
# RuVector-Postgres Benchmark Runner Script
# Runs performance benchmarks and generates reports

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_header() { echo -e "${CYAN}=== $1 ===${NC}"; }

# Configuration
PG_VERSION="${PG_VERSION:-17}"
RESULTS_DIR="${RESULTS_DIR:-/benchmark-results}"
BASELINE_DIR="${BASELINE_DIR:-/baseline}"
COMPARE_BASELINE="${COMPARE_BASELINE:-false}"
BENCHMARK_FILTER="${BENCHMARK_FILTER:-}"

# Ensure results directory exists
mkdir -p "${RESULTS_DIR}"

log_header "RuVector-Postgres Benchmark Runner"
log_info "PostgreSQL Version: ${PG_VERSION}"
log_info "Results Directory: ${RESULTS_DIR}"
log_info "Compare Baseline: ${COMPARE_BASELINE}"

# Navigate to the crate directory
cd /app/crates/ruvector-postgres 2>/dev/null || cd /app

# Check if we have the source code
if [ ! -f "Cargo.toml" ]; then
    log_error "Cargo.toml not found. Mount the source code to /app"
    exit 1
fi

# Start benchmark execution
START_TIME=$(date +%s)
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_DIR="${RESULTS_DIR}/${TIMESTAMP}"
mkdir -p "${REPORT_DIR}"

# Build with optimizations
log_info "Building with release optimizations..."
cargo build --release --features pg${PG_VERSION}

# Run Criterion benchmarks
log_header "Running Criterion Benchmarks"

BENCH_CMD="cargo bench --features pg${PG_VERSION}"
if [ -n "${BENCHMARK_FILTER}" ]; then
    BENCH_CMD="${BENCH_CMD} -- ${BENCHMARK_FILTER}"
fi

# Run benchmarks and capture output
log_info "Executing: ${BENCH_CMD}"
set +e
${BENCH_CMD} 2>&1 | tee "${REPORT_DIR}/benchmark.log"
BENCH_EXIT_CODE=${PIPESTATUS[0]}
set -e

# Copy Criterion report if it exists
if [ -d "target/criterion" ]; then
    log_info "Copying Criterion HTML reports..."
    cp -r target/criterion "${REPORT_DIR}/"
fi

# Run individual benchmark suites with detailed output
log_header "Running Detailed Benchmark Suites"

# Distance benchmarks
log_info "Running distance_bench..."
cargo bench --features pg${PG_VERSION} --bench distance_bench -- --output-format bencher 2>&1 \
    | tee "${REPORT_DIR}/distance_bench.txt" || true

# Quantization benchmarks
log_info "Running quantization_bench..."
cargo bench --features pg${PG_VERSION} --bench quantization_bench -- --output-format bencher 2>&1 \
    | tee "${REPORT_DIR}/quantization_bench.txt" || true

# Index benchmarks
log_info "Running index_bench..."
cargo bench --features pg${PG_VERSION} --bench index_bench -- --output-format bencher 2>&1 \
    | tee "${REPORT_DIR}/index_bench.txt" || true

# Quantized distance benchmarks
log_info "Running quantized_distance_bench..."
cargo bench --features pg${PG_VERSION} --bench quantized_distance_bench -- --output-format bencher 2>&1 \
    | tee "${REPORT_DIR}/quantized_distance_bench.txt" || true

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# Compare with baseline if requested
if [ "${COMPARE_BASELINE}" == "true" ] && [ -d "${BASELINE_DIR}" ]; then
    log_header "Comparing with Baseline"

    # Simple comparison using diff
    for bench_file in distance_bench.txt quantization_bench.txt index_bench.txt quantized_distance_bench.txt; do
        if [ -f "${BASELINE_DIR}/${bench_file}" ] && [ -f "${REPORT_DIR}/${bench_file}" ]; then
            log_info "Comparing ${bench_file}..."
            diff -u "${BASELINE_DIR}/${bench_file}" "${REPORT_DIR}/${bench_file}" \
                > "${REPORT_DIR}/diff_${bench_file}" 2>&1 || true
        fi
    done
fi

# Generate summary report
log_header "Generating Summary Report"

cat > "${REPORT_DIR}/summary.json" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "pg_version": "${PG_VERSION}",
  "duration_seconds": ${DURATION},
  "benchmark_exit_code": ${BENCH_EXIT_CODE},
  "benchmarks_run": [
    "distance_bench",
    "quantization_bench",
    "index_bench",
    "quantized_distance_bench"
  ],
  "report_directory": "${REPORT_DIR}"
}
EOF

# Generate markdown report
cat > "${REPORT_DIR}/REPORT.md" << EOF
# RuVector-Postgres Benchmark Report

**Date**: $(date)
**PostgreSQL Version**: ${PG_VERSION}
**Duration**: ${DURATION}s

## Benchmark Results

### Distance Benchmarks
\`\`\`
$(cat "${REPORT_DIR}/distance_bench.txt" 2>/dev/null | head -50 || echo "No results")
\`\`\`

### Quantization Benchmarks
\`\`\`
$(cat "${REPORT_DIR}/quantization_bench.txt" 2>/dev/null | head -50 || echo "No results")
\`\`\`

### Index Benchmarks
\`\`\`
$(cat "${REPORT_DIR}/index_bench.txt" 2>/dev/null | head -50 || echo "No results")
\`\`\`

### Quantized Distance Benchmarks
\`\`\`
$(cat "${REPORT_DIR}/quantized_distance_bench.txt" 2>/dev/null | head -50 || echo "No results")
\`\`\`

## Full Reports

See the \`criterion/\` directory for detailed HTML reports.
EOF

# Create symlink to latest results
ln -sfn "${REPORT_DIR}" "${RESULTS_DIR}/latest"

# Print summary
echo ""
echo "=========================================="
echo "         BENCHMARK SUMMARY"
echo "=========================================="
echo "PostgreSQL Version: ${PG_VERSION}"
echo "Duration: ${DURATION}s"
echo "Exit Code: ${BENCH_EXIT_CODE}"
echo "Report: ${REPORT_DIR}/REPORT.md"
echo "HTML Reports: ${REPORT_DIR}/criterion/"
echo "=========================================="

if [ "${BENCH_EXIT_CODE}" != "0" ]; then
    log_warn "Some benchmarks may have failed"
    exit ${BENCH_EXIT_CODE}
fi

log_success "Benchmarks completed successfully!"
exit 0
