#!/usr/bin/env bash
#
# RuVector Comprehensive Benchmark Runner
# =======================================
#
# This script runs all benchmarks and outputs results in JSON format
# suitable for CI/CD tracking and historical comparison.
#
# Usage:
#   ./scripts/run_benchmarks.sh              # Run all benchmarks
#   ./scripts/run_benchmarks.sh --quick      # Quick mode (reduced iterations)
#   ./scripts/run_benchmarks.sh --json       # Output JSON only
#   ./scripts/run_benchmarks.sh --help       # Show help
#

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUTPUT_DIR="${PROJECT_ROOT}/bench_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
JSON_OUTPUT="${OUTPUT_DIR}/benchmark_${TIMESTAMP}.json"

# Default settings
QUICK_MODE=false
JSON_ONLY=false
VECTORS=10000
QUERIES=100
DIMENSIONS=384

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --quick)
            QUICK_MODE=true
            VECTORS=1000
            QUERIES=50
            shift
            ;;
        --json)
            JSON_ONLY=true
            shift
            ;;
        --help|-h)
            echo "RuVector Benchmark Runner"
            echo ""
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --quick    Run with reduced iterations for faster results"
            echo "  --json     Output JSON only (suppress console output)"
            echo "  --help     Show this help message"
            echo ""
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Logging functions
log_info() {
    if [ "$JSON_ONLY" = false ]; then
        echo -e "${BLUE}[INFO]${NC} $1"
    fi
}

log_success() {
    if [ "$JSON_ONLY" = false ]; then
        echo -e "${GREEN}[SUCCESS]${NC} $1"
    fi
}

log_warning() {
    if [ "$JSON_ONLY" = false ]; then
        echo -e "${YELLOW}[WARNING]${NC} $1"
    fi
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

# Create output directory
mkdir -p "${OUTPUT_DIR}"

# Get system information
get_system_info() {
    local cpu_info=""
    local memory=""
    local os_version=""
    local rust_version=""

    # CPU info
    if [[ "$OSTYPE" == "darwin"* ]]; then
        cpu_info=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "Unknown")
        memory=$(sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.0f GB", $0/1024/1024/1024}')
        os_version=$(sw_vers -productVersion 2>/dev/null || echo "Unknown")
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        cpu_info=$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d':' -f2 | xargs || echo "Unknown")
        memory=$(free -h 2>/dev/null | awk '/^Mem:/ {print $2}' || echo "Unknown")
        os_version=$(cat /etc/os-release 2>/dev/null | grep -m1 VERSION= | cut -d'"' -f2 || echo "Unknown")
    fi

    rust_version=$(rustc --version 2>/dev/null | awk '{print $2}' || echo "Unknown")

    cat << EOF
{
    "cpu": "${cpu_info}",
    "memory": "${memory}",
    "os": "${os_version}",
    "rust_version": "${rust_version}",
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "quick_mode": ${QUICK_MODE}
}
EOF
}

# Run NEON SIMD benchmark
run_neon_benchmark() {
    log_info "Running NEON SIMD benchmark..."

    local output
    output=$(cd "${PROJECT_ROOT}" && cargo run --example neon_benchmark --release -p ruvector-core 2>&1 | tail -20)

    # Parse results
    local euclidean_simd euclidean_scalar euclidean_speedup
    local dot_simd dot_scalar dot_speedup
    local cosine_simd cosine_scalar cosine_speedup

    euclidean_simd=$(echo "$output" | grep -A1 "Euclidean" | grep "SIMD:" | awk '{print $2}')
    euclidean_scalar=$(echo "$output" | grep -A2 "Euclidean" | grep "Scalar:" | awk '{print $2}')
    euclidean_speedup=$(echo "$output" | grep -A3 "Euclidean" | grep "Speedup:" | awk '{print $2}' | tr -d 'x')

    dot_simd=$(echo "$output" | grep -A1 "Dot Product" | grep "SIMD:" | awk '{print $2}')
    dot_scalar=$(echo "$output" | grep -A2 "Dot Product" | grep "Scalar:" | awk '{print $2}')
    dot_speedup=$(echo "$output" | grep -A3 "Dot Product" | grep "Speedup:" | awk '{print $2}' | tr -d 'x')

    cosine_simd=$(echo "$output" | grep -A1 "Cosine" | grep "SIMD:" | awk '{print $2}')
    cosine_scalar=$(echo "$output" | grep -A2 "Cosine" | grep "Scalar:" | awk '{print $2}')
    cosine_speedup=$(echo "$output" | grep -A3 "Cosine" | grep "Speedup:" | awk '{print $2}' | tr -d 'x')

    cat << EOF
{
    "euclidean": {
        "simd_ms": ${euclidean_simd:-0},
        "scalar_ms": ${euclidean_scalar:-0},
        "speedup": ${euclidean_speedup:-0}
    },
    "dot_product": {
        "simd_ms": ${dot_simd:-0},
        "scalar_ms": ${dot_scalar:-0},
        "speedup": ${dot_speedup:-0}
    },
    "cosine": {
        "simd_ms": ${cosine_simd:-0},
        "scalar_ms": ${cosine_scalar:-0},
        "speedup": ${cosine_speedup:-0}
    }
}
EOF

    log_success "NEON benchmark complete"
}

# Run Criterion benchmarks
run_criterion_benchmarks() {
    log_info "Running Criterion benchmarks..."

    local bench_args=""
    if [ "$QUICK_MODE" = true ]; then
        bench_args="-- --quick"
    fi

    # Run distance metrics benchmark
    cd "${PROJECT_ROOT}/crates/ruvector-core"
    cargo bench --bench distance_metrics ${bench_args} 2>&1 | grep -E "time:" | head -20 > "${OUTPUT_DIR}/distance_metrics_raw.txt" || true

    # Run HNSW search benchmark
    cargo bench --bench hnsw_search ${bench_args} 2>&1 | grep -E "time:" | head -10 > "${OUTPUT_DIR}/hnsw_search_raw.txt" || true

    # Run quantization benchmark
    cargo bench --bench quantization_bench ${bench_args} 2>&1 | grep -E "time:" | head -20 > "${OUTPUT_DIR}/quantization_raw.txt" || true

    log_success "Criterion benchmarks complete"

    # Return placeholder JSON (real parsing would be more complex)
    echo '{"criterion_complete": true}'
}

# Run comparison benchmark
run_comparison_benchmark() {
    log_info "Running comparison benchmark..."

    cd "${PROJECT_ROOT}"
    cargo run -p ruvector-bench --bin comparison-benchmark --release -- \
        --num-vectors ${VECTORS} \
        --queries ${QUERIES} \
        --dimensions ${DIMENSIONS} \
        --output "${OUTPUT_DIR}" 2>&1 | tail -10

    # Read the generated JSON
    if [ -f "${OUTPUT_DIR}/comparison_benchmark.json" ]; then
        cat "${OUTPUT_DIR}/comparison_benchmark.json"
    else
        echo '{"error": "comparison benchmark output not found"}'
    fi

    log_success "Comparison benchmark complete"
}

# Main function
main() {
    log_info "=========================================="
    log_info "RuVector Benchmark Suite"
    log_info "=========================================="
    log_info "Output directory: ${OUTPUT_DIR}"
    log_info "Quick mode: ${QUICK_MODE}"
    log_info ""

    # Collect system info
    log_info "Collecting system information..."
    local system_info
    system_info=$(get_system_info)

    # Run benchmarks
    log_info ""
    log_info "Starting benchmarks..."
    log_info ""

    local neon_results
    neon_results=$(run_neon_benchmark)

    local criterion_results
    criterion_results=$(run_criterion_benchmarks)

    local comparison_results
    comparison_results=$(run_comparison_benchmark)

    # Combine all results into final JSON
    local final_json
    final_json=$(cat << EOF
{
    "system_info": ${system_info},
    "neon_simd": ${neon_results},
    "criterion": ${criterion_results},
    "comparison": ${comparison_results},
    "summary": {
        "vectors_tested": ${VECTORS},
        "queries_tested": ${QUERIES},
        "dimensions": ${DIMENSIONS}
    }
}
EOF
)

    # Save JSON output
    echo "${final_json}" > "${JSON_OUTPUT}"
    log_success "Benchmark results saved to: ${JSON_OUTPUT}"

    # Output JSON if requested
    if [ "$JSON_ONLY" = true ]; then
        echo "${final_json}"
    else
        log_info ""
        log_info "=========================================="
        log_info "Benchmark Summary"
        log_info "=========================================="
        echo ""
        echo "SIMD Speedups:"
        echo "  Euclidean: $(echo "$neon_results" | grep -o '"speedup": [0-9.]*' | head -1 | awk '{print $2}')x"
        echo "  Dot Product: $(echo "$neon_results" | grep -o '"speedup": [0-9.]*' | sed -n '2p' | awk '{print $2}')x"
        echo "  Cosine: $(echo "$neon_results" | grep -o '"speedup": [0-9.]*' | tail -1 | awk '{print $2}')x"
        echo ""
        log_success "All benchmarks complete!"
        log_info "Full results: ${JSON_OUTPUT}"
        log_info "Markdown report: ${OUTPUT_DIR}/comparison_benchmark.md"
    fi
}

# Run main
main "$@"
