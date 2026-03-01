#!/bin/bash
# Comprehensive benchmark runner script

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BENCHMARK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESULTS_DIR="${BENCHMARK_DIR}/results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Create results directory
mkdir -p "${RESULTS_DIR}"

echo -e "${BLUE}==================================================${NC}"
echo -e "${BLUE}  RuVector Comprehensive Benchmark Suite${NC}"
echo -e "${BLUE}==================================================${NC}"
echo ""

# ============================================================================
# Rust Benchmarks
# ============================================================================

echo -e "${GREEN}Running Rust benchmarks...${NC}"
echo ""

# Distance benchmarks
echo -e "${YELLOW}1. Distance function benchmarks${NC}"
cargo bench --bench distance_bench -- --output-format bencher | tee "${RESULTS_DIR}/distance_${TIMESTAMP}.txt"

# Index benchmarks
echo -e "${YELLOW}2. HNSW index benchmarks${NC}"
cargo bench --bench index_bench -- --output-format bencher | tee "${RESULTS_DIR}/index_${TIMESTAMP}.txt"

# Quantization benchmarks
echo -e "${YELLOW}3. Quantization benchmarks${NC}"
cargo bench --bench quantization_bench -- --output-format bencher | tee "${RESULTS_DIR}/quantization_${TIMESTAMP}.txt"

# Quantized distance benchmarks
echo -e "${YELLOW}4. Quantized distance benchmarks${NC}"
cargo bench --bench quantized_distance_bench -- --output-format bencher | tee "${RESULTS_DIR}/quantized_distance_${TIMESTAMP}.txt"

# ============================================================================
# SQL Benchmarks (if PostgreSQL is available)
# ============================================================================

if command -v psql &> /dev/null; then
    echo ""
    echo -e "${GREEN}Running SQL benchmarks...${NC}"
    echo ""

    # Check if test database exists
    if psql -lqt | cut -d \| -f 1 | grep -qw ruvector_bench; then
        echo -e "${YELLOW}5. Quick SQL benchmark${NC}"
        psql -d ruvector_bench -f "${BENCHMARK_DIR}/sql/quick_benchmark.sql" | tee "${RESULTS_DIR}/sql_quick_${TIMESTAMP}.txt"

        echo -e "${YELLOW}6. Full workload benchmark${NC}"
        echo -e "${RED}Warning: This may take several minutes...${NC}"
        psql -d ruvector_bench -f "${BENCHMARK_DIR}/sql/benchmark_workload.sql" | tee "${RESULTS_DIR}/sql_workload_${TIMESTAMP}.txt"
    else
        echo -e "${YELLOW}Skipping SQL benchmarks (database 'ruvector_bench' not found)${NC}"
        echo -e "${YELLOW}To run SQL benchmarks:${NC}"
        echo -e "  createdb ruvector_bench"
        echo -e "  psql -d ruvector_bench -c 'CREATE EXTENSION ruvector;'"
        echo -e "  psql -d ruvector_bench -c 'CREATE EXTENSION pgvector;'"
    fi
else
    echo -e "${YELLOW}Skipping SQL benchmarks (psql not found)${NC}"
fi

# ============================================================================
# Generate Summary Report
# ============================================================================

echo ""
echo -e "${GREEN}Generating summary report...${NC}"

cat > "${RESULTS_DIR}/summary_${TIMESTAMP}.md" <<EOF
# RuVector Benchmark Results

**Date:** $(date)
**Platform:** $(uname -s) $(uname -m)
**Rust Version:** $(rustc --version)

## Benchmark Files

- Distance functions: \`distance_${TIMESTAMP}.txt\`
- HNSW index: \`index_${TIMESTAMP}.txt\`
- Quantization: \`quantization_${TIMESTAMP}.txt\`
- Quantized distance: \`quantized_distance_${TIMESTAMP}.txt\`

## SQL Benchmarks

EOF

if [ -f "${RESULTS_DIR}/sql_quick_${TIMESTAMP}.txt" ]; then
    cat >> "${RESULTS_DIR}/summary_${TIMESTAMP}.md" <<EOF
- Quick benchmark: \`sql_quick_${TIMESTAMP}.txt\`
- Full workload: \`sql_workload_${TIMESTAMP}.txt\`

EOF
else
    cat >> "${RESULTS_DIR}/summary_${TIMESTAMP}.md" <<EOF
SQL benchmarks were not run. See setup instructions above.

EOF
fi

cat >> "${RESULTS_DIR}/summary_${TIMESTAMP}.md" <<EOF
## System Information

\`\`\`
$(uname -a)
\`\`\`

### CPU Information

\`\`\`
$(lscpu 2>/dev/null || sysctl -a | grep machdep.cpu || echo "CPU info not available")
\`\`\`

### Memory Information

\`\`\`
$(free -h 2>/dev/null || vm_stat || echo "Memory info not available")
\`\`\`

## Running the Benchmarks

To reproduce these results:

\`\`\`bash
cd crates/ruvector-postgres
bash benches/scripts/run_benchmarks.sh
\`\`\`

## Comparing with Previous Results

\`\`\`bash
# Install cargo-criterion for better comparison
cargo install cargo-criterion

# Run with baseline
cargo criterion --bench distance_bench --baseline main
\`\`\`
EOF

echo ""
echo -e "${GREEN}==================================================${NC}"
echo -e "${GREEN}  Benchmark Complete!${NC}"
echo -e "${GREEN}==================================================${NC}"
echo ""
echo -e "Results saved to: ${BLUE}${RESULTS_DIR}${NC}"
echo -e "Summary report: ${BLUE}${RESULTS_DIR}/summary_${TIMESTAMP}.md${NC}"
echo ""

# ============================================================================
# Optional: Open results in browser if criterion HTML is available
# ============================================================================

if [ -d "target/criterion" ]; then
    echo -e "${YELLOW}Criterion HTML reports available at:${NC}"
    echo -e "  ${BLUE}file://$(pwd)/target/criterion/report/index.html${NC}"
fi

echo ""
echo -e "${GREEN}Done!${NC}"
