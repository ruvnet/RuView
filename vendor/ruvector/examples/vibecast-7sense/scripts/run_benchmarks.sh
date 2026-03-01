#!/bin/bash
# 7sense Benchmark Runner
#
# This script runs all benchmarks and generates reports.
# Performance targets from ADR-004:
# - HNSW Search: 150x speedup vs brute force, p99 < 50ms
# - Embedding inference: >100 segments/second
# - Batch ingestion: 1M vectors/minute
# - Query latency: <100ms total

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BASELINE_NAME="${BASELINE_NAME:-main}"
OUTPUT_DIR="${OUTPUT_DIR:-target/criterion}"
REPORT_DIR="${REPORT_DIR:-target/benchmark-reports}"
COMPARE_BASELINE="${COMPARE_BASELINE:-false}"

# Benchmark suites
ALL_BENCHMARKS=(
    "hnsw_benchmark"
    "embedding_benchmark"
    "clustering_benchmark"
    "api_benchmark"
)

print_header() {
    echo ""
    echo "============================================================"
    echo -e "${BLUE}$1${NC}"
    echo "============================================================"
    echo ""
}

print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Create output directories
setup_directories() {
    print_status "Setting up output directories..."
    mkdir -p "$OUTPUT_DIR"
    mkdir -p "$REPORT_DIR"
    mkdir -p "$REPORT_DIR/html"
    mkdir -p "$REPORT_DIR/json"
}

# Run a single benchmark suite
run_benchmark() {
    local bench_name="$1"
    local extra_args="${2:-}"

    print_header "Running $bench_name"

    if [[ "$COMPARE_BASELINE" == "true" ]]; then
        print_status "Comparing against baseline: $BASELINE_NAME"
        cargo bench --bench "$bench_name" -- --baseline "$BASELINE_NAME" $extra_args
    else
        cargo bench --bench "$bench_name" -- $extra_args
    fi

    # Check if benchmark completed successfully
    if [[ $? -eq 0 ]]; then
        print_status "$bench_name completed successfully"
    else
        print_error "$bench_name failed"
        return 1
    fi
}

# Run all benchmarks
run_all_benchmarks() {
    print_header "Running All Benchmarks"

    local failed=()

    for bench in "${ALL_BENCHMARKS[@]}"; do
        if ! run_benchmark "$bench"; then
            failed+=("$bench")
        fi
    done

    if [[ ${#failed[@]} -gt 0 ]]; then
        print_error "Failed benchmarks: ${failed[*]}"
        return 1
    fi

    print_status "All benchmarks completed successfully"
}

# Save baseline for future comparisons
save_baseline() {
    local baseline_name="${1:-$BASELINE_NAME}"

    print_header "Saving Baseline: $baseline_name"

    for bench in "${ALL_BENCHMARKS[@]}"; do
        print_status "Saving baseline for $bench..."
        cargo bench --bench "$bench" -- --save-baseline "$baseline_name"
    done

    print_status "Baseline '$baseline_name' saved"
}

# Run quick benchmarks (reduced sample size)
run_quick() {
    print_header "Running Quick Benchmarks"

    # Set sample size to minimum for quick testing
    export CRITERION_DEBUG=1

    for bench in "${ALL_BENCHMARKS[@]}"; do
        print_status "Quick run: $bench"
        cargo bench --bench "$bench" -- --quick
    done
}

# Run specific benchmark groups
run_hnsw() {
    run_benchmark "hnsw_benchmark" "$@"
}

run_embedding() {
    run_benchmark "embedding_benchmark" "$@"
}

run_clustering() {
    run_benchmark "clustering_benchmark" "$@"
}

run_api() {
    run_benchmark "api_benchmark" "$@"
}

# Generate summary report
generate_summary() {
    print_header "Generating Summary Report"

    local report_file="$REPORT_DIR/summary_$(date +%Y%m%d_%H%M%S).txt"

    {
        echo "7sense Benchmark Summary Report"
        echo "Generated: $(date)"
        echo "============================================================"
        echo ""
        echo "Performance Targets (from ADR-004):"
        echo "  - HNSW Search: 150x speedup vs brute force"
        echo "  - Query Latency p50: < 10ms"
        echo "  - Query Latency p99: < 50ms"
        echo "  - Recall@10: >= 0.95"
        echo "  - Recall@100: >= 0.98"
        echo "  - Embedding inference: >100 segments/second"
        echo "  - Batch ingestion: 1M vectors/minute"
        echo "  - Total query latency: <100ms"
        echo ""
        echo "============================================================"
        echo ""

        # Parse criterion output if available
        if [[ -d "$OUTPUT_DIR" ]]; then
            echo "Benchmark Results:"
            echo ""

            for bench in "${ALL_BENCHMARKS[@]}"; do
                if [[ -d "$OUTPUT_DIR/$bench" ]]; then
                    echo "--- $bench ---"
                    # List all benchmark groups
                    find "$OUTPUT_DIR/$bench" -name "estimates.json" -exec dirname {} \; 2>/dev/null | \
                        while read -r dir; do
                            local name=$(basename "$dir")
                            echo "  $name"
                        done
                    echo ""
                fi
            done
        else
            echo "No benchmark results found. Run benchmarks first."
        fi
    } > "$report_file"

    print_status "Summary saved to: $report_file"
    cat "$report_file"
}

# Run performance analysis tests
run_analysis() {
    print_header "Running Performance Analysis"

    print_status "Running speedup analysis..."
    cargo test --release -- --ignored run_speedup_analysis --nocapture 2>/dev/null || true

    print_status "Running latency analysis..."
    cargo test --release -- --ignored run_latency_analysis --nocapture 2>/dev/null || true

    print_status "Running throughput analysis..."
    cargo test --release -- --ignored run_throughput_analysis --nocapture 2>/dev/null || true

    print_status "Running API latency analysis..."
    cargo test --release -- --ignored run_api_latency_analysis --nocapture 2>/dev/null || true
}

# Clean benchmark artifacts
clean() {
    print_header "Cleaning Benchmark Artifacts"

    rm -rf "$OUTPUT_DIR"
    rm -rf "$REPORT_DIR"

    print_status "Cleaned benchmark artifacts"
}

# Show help
show_help() {
    echo "7sense Benchmark Runner"
    echo ""
    echo "Usage: $0 [command] [options]"
    echo ""
    echo "Commands:"
    echo "  all              Run all benchmarks (default)"
    echo "  quick            Run quick benchmarks with reduced samples"
    echo "  hnsw             Run HNSW benchmarks only"
    echo "  embedding        Run embedding benchmarks only"
    echo "  clustering       Run clustering benchmarks only"
    echo "  api              Run API benchmarks only"
    echo "  save-baseline    Save current results as baseline"
    echo "  compare          Run benchmarks and compare against baseline"
    echo "  analysis         Run detailed performance analysis"
    echo "  summary          Generate summary report"
    echo "  clean            Clean benchmark artifacts"
    echo "  help             Show this help message"
    echo ""
    echo "Environment Variables:"
    echo "  BASELINE_NAME    Name for baseline comparison (default: main)"
    echo "  OUTPUT_DIR       Criterion output directory (default: target/criterion)"
    echo "  REPORT_DIR       Report output directory (default: target/benchmark-reports)"
    echo "  COMPARE_BASELINE Set to 'true' to compare against baseline"
    echo ""
    echo "Examples:"
    echo "  $0                           # Run all benchmarks"
    echo "  $0 hnsw                      # Run HNSW benchmarks only"
    echo "  $0 save-baseline v1.0.0      # Save baseline named 'v1.0.0'"
    echo "  COMPARE_BASELINE=true $0     # Compare against baseline"
    echo ""
    echo "Performance Targets (ADR-004):"
    echo "  - HNSW Search: 150x speedup vs brute force"
    echo "  - Query Latency p99: < 50ms"
    echo "  - Recall@10: >= 0.95"
    echo "  - Embedding: >100 segments/second"
    echo "  - Batch: 1M vectors/minute"
}

# Main entry point
main() {
    local command="${1:-all}"
    shift || true

    setup_directories

    case "$command" in
        all)
            run_all_benchmarks
            generate_summary
            ;;
        quick)
            run_quick
            ;;
        hnsw)
            run_hnsw "$@"
            ;;
        embedding)
            run_embedding "$@"
            ;;
        clustering)
            run_clustering "$@"
            ;;
        api)
            run_api "$@"
            ;;
        save-baseline)
            save_baseline "${1:-$BASELINE_NAME}"
            ;;
        compare)
            COMPARE_BASELINE=true run_all_benchmarks
            ;;
        analysis)
            run_analysis
            ;;
        summary)
            generate_summary
            ;;
        clean)
            clean
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            print_error "Unknown command: $command"
            show_help
            exit 1
            ;;
    esac
}

main "$@"
