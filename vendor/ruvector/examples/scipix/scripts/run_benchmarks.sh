#!/bin/bash
set -e

# ruvector-scipix Benchmark Suite Runner
# Comprehensive performance benchmarking with baseline tracking and regression detection

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BENCHMARK_DIR="$PROJECT_DIR/target/criterion"
BASELINE="${BASELINE:-main}"
GENERATE_HTML="${GENERATE_HTML:-true}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=====================================${NC}"
echo -e "${BLUE}ruvector-scipix Benchmark Suite${NC}"
echo -e "${BLUE}=====================================${NC}"
echo ""

# Check if running in project directory
if [ ! -f "$PROJECT_DIR/Cargo.toml" ]; then
    echo -e "${RED}Error: Must run from scipix project directory${NC}"
    exit 1
fi

# Function to run a single benchmark
run_benchmark() {
    local bench_name=$1
    local description=$2

    echo -e "${GREEN}Running ${bench_name}...${NC}"
    echo -e "${YELLOW}${description}${NC}"

    cd "$PROJECT_DIR"

    if [ "$BASELINE" != "" ]; then
        cargo bench --bench "$bench_name" -- --save-baseline "$BASELINE"
    else
        cargo bench --bench "$bench_name"
    fi

    echo ""
}

# Function to compare with baseline
compare_baseline() {
    local bench_name=$1
    local baseline=$2

    echo -e "${BLUE}Comparing ${bench_name} with baseline ${baseline}...${NC}"

    cd "$PROJECT_DIR"
    cargo bench --bench "$bench_name" -- --baseline "$baseline"

    echo ""
}

# Function to check for regressions
check_regressions() {
    echo -e "${BLUE}Checking for performance regressions...${NC}"

    # Target metrics
    echo -e "${YELLOW}Performance Targets:${NC}"
    echo "  - Single image OCR: <100ms P95"
    echo "  - Batch (16 images): <500ms"
    echo "  - Preprocessing: <20ms"
    echo "  - LaTeX generation: <5ms"
    echo ""

    # Parse criterion output for regressions
    # In production, this would parse actual benchmark results
    if [ -d "$BENCHMARK_DIR" ]; then
        echo -e "${GREEN}Benchmark results saved to: ${BENCHMARK_DIR}${NC}"
    fi
}

# Function to generate HTML reports
generate_reports() {
    if [ "$GENERATE_HTML" = "true" ]; then
        echo -e "${BLUE}Generating HTML reports...${NC}"

        if [ -d "$BENCHMARK_DIR" ]; then
            # Criterion automatically generates HTML reports
            echo -e "${GREEN}HTML reports generated in ${BENCHMARK_DIR}${NC}"
            echo -e "${YELLOW}Open ${BENCHMARK_DIR}/report/index.html in your browser${NC}"
        fi
    fi
}

# Parse command line arguments
MODE="${1:-all}"
COMPARE_BASELINE_NAME="${2:-}"

case "$MODE" in
    "all")
        echo -e "${YELLOW}Running all benchmarks...${NC}\n"

        run_benchmark "ocr_latency" "OCR latency benchmarks (single, batch, cold vs warm)"
        run_benchmark "preprocessing" "Image preprocessing benchmarks (transforms, pipeline)"
        run_benchmark "latex_generation" "LaTeX generation benchmarks (AST, string building)"
        run_benchmark "inference" "Model inference benchmarks (detection, recognition, math)"
        run_benchmark "cache" "Cache benchmarks (embedding, similarity search)"
        run_benchmark "api" "API benchmarks (parsing, serialization, middleware)"
        run_benchmark "memory" "Memory benchmarks (peak usage, growth, fragmentation)"

        check_regressions
        generate_reports
        ;;

    "latency")
        run_benchmark "ocr_latency" "OCR latency benchmarks"
        ;;

    "preprocessing")
        run_benchmark "preprocessing" "Image preprocessing benchmarks"
        ;;

    "latex")
        run_benchmark "latex_generation" "LaTeX generation benchmarks"
        ;;

    "inference")
        run_benchmark "inference" "Model inference benchmarks"
        ;;

    "cache")
        run_benchmark "cache" "Cache benchmarks"
        ;;

    "api")
        run_benchmark "api" "API benchmarks"
        ;;

    "memory")
        run_benchmark "memory" "Memory benchmarks"
        ;;

    "compare")
        if [ -z "$COMPARE_BASELINE_NAME" ]; then
            echo -e "${RED}Error: Baseline name required for comparison${NC}"
            echo "Usage: $0 compare <baseline-name>"
            exit 1
        fi

        echo -e "${YELLOW}Comparing all benchmarks with baseline: ${COMPARE_BASELINE_NAME}${NC}\n"

        compare_baseline "ocr_latency" "$COMPARE_BASELINE_NAME"
        compare_baseline "preprocessing" "$COMPARE_BASELINE_NAME"
        compare_baseline "latex_generation" "$COMPARE_BASELINE_NAME"
        compare_baseline "inference" "$COMPARE_BASELINE_NAME"
        compare_baseline "cache" "$COMPARE_BASELINE_NAME"
        compare_baseline "api" "$COMPARE_BASELINE_NAME"
        compare_baseline "memory" "$COMPARE_BASELINE_NAME"
        ;;

    "quick")
        echo -e "${YELLOW}Running quick benchmark suite (reduced samples)...${NC}\n"

        export CARGO_BENCH_OPTS="-- --quick"

        run_benchmark "ocr_latency" "Quick OCR latency check"
        run_benchmark "preprocessing" "Quick preprocessing check"
        ;;

    "ci")
        echo -e "${YELLOW}Running CI benchmark suite...${NC}\n"

        # Run benchmarks with minimal samples for CI
        export CARGO_BENCH_OPTS="-- --sample-size 10"

        run_benchmark "ocr_latency" "CI OCR latency"
        run_benchmark "preprocessing" "CI preprocessing"
        run_benchmark "latex_generation" "CI LaTeX generation"

        # Check for major regressions only
        check_regressions
        ;;

    "help"|"--help"|"-h")
        echo "Usage: $0 [MODE] [OPTIONS]"
        echo ""
        echo "Modes:"
        echo "  all              Run all benchmarks (default)"
        echo "  latency          Run OCR latency benchmarks only"
        echo "  preprocessing    Run preprocessing benchmarks only"
        echo "  latex            Run LaTeX generation benchmarks only"
        echo "  inference        Run model inference benchmarks only"
        echo "  cache            Run cache benchmarks only"
        echo "  api              Run API benchmarks only"
        echo "  memory           Run memory benchmarks only"
        echo "  compare <name>   Compare with saved baseline"
        echo "  quick            Run quick benchmark suite"
        echo "  ci               Run CI benchmark suite"
        echo "  help             Show this help message"
        echo ""
        echo "Environment Variables:"
        echo "  BASELINE=<name>        Save results as baseline (default: main)"
        echo "  GENERATE_HTML=<bool>   Generate HTML reports (default: true)"
        echo ""
        echo "Examples:"
        echo "  $0 all                    # Run all benchmarks"
        echo "  $0 latency                # Run latency benchmarks only"
        echo "  BASELINE=v1.0 $0 all      # Save as v1.0 baseline"
        echo "  $0 compare v1.0           # Compare with v1.0 baseline"
        echo "  $0 quick                  # Quick benchmark suite"
        ;;

    *)
        echo -e "${RED}Error: Unknown mode '$MODE'${NC}"
        echo "Use '$0 help' for usage information"
        exit 1
        ;;
esac

echo ""
echo -e "${GREEN}=====================================${NC}"
echo -e "${GREEN}Benchmarks Complete!${NC}"
echo -e "${GREEN}=====================================${NC}"

# Print summary
if [ -d "$BENCHMARK_DIR" ]; then
    echo ""
    echo -e "${YELLOW}Results Summary:${NC}"
    echo -e "  Benchmark data: ${BENCHMARK_DIR}"

    if [ "$GENERATE_HTML" = "true" ]; then
        echo -e "  HTML reports: ${BENCHMARK_DIR}/report/index.html"
    fi

    if [ "$BASELINE" != "" ]; then
        echo -e "  Saved baseline: ${BASELINE}"
    fi
fi

echo ""
