#!/bin/bash
# Comprehensive benchmark runner for edge-net
# Usage: ./scripts/run-benchmarks.sh [options]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BASELINE_FILE="baseline-benchmarks.txt"
CURRENT_FILE="current-benchmarks.txt"
REPORT_DIR="benchmark-reports"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Parse arguments
PROFILE=false
COMPARE=false
SAVE_BASELINE=false
CATEGORY=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --profile)
            PROFILE=true
            shift
            ;;
        --compare)
            COMPARE=true
            shift
            ;;
        --save-baseline)
            SAVE_BASELINE=true
            shift
            ;;
        --category)
            CATEGORY="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --profile         Enable profiling with flamegraph"
            echo "  --compare         Compare with baseline"
            echo "  --save-baseline   Save current results as new baseline"
            echo "  --category NAME   Run specific benchmark category"
            echo "  --help            Show this help message"
            echo ""
            echo "Categories: credit, qdag, task, security, topology, economic, evolution, optimization, network"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# Create report directory
mkdir -p "$REPORT_DIR"

echo -e "${BLUE}═══════════════════════════════════════════════${NC}"
echo -e "${BLUE}    Edge-Net Performance Benchmark Suite${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════${NC}"
echo ""

# Check for nightly toolchain
if ! rustup toolchain list | grep -q nightly; then
    echo -e "${YELLOW}Installing nightly toolchain...${NC}"
    rustup install nightly
fi

# Build benchmarks
echo -e "${GREEN}Building benchmarks...${NC}"
cargo +nightly build --release --features=bench --benches

# Run benchmarks
echo ""
echo -e "${GREEN}Running benchmarks...${NC}"
echo ""

if [ -n "$CATEGORY" ]; then
    echo -e "${BLUE}Category: $CATEGORY${NC}"
    cargo +nightly bench --features=bench "$CATEGORY" 2>&1 | tee "$REPORT_DIR/bench_${CATEGORY}_${TIMESTAMP}.txt"
else
    cargo +nightly bench --features=bench 2>&1 | tee "$CURRENT_FILE"
fi

# Save baseline if requested
if [ "$SAVE_BASELINE" = true ]; then
    echo ""
    echo -e "${GREEN}Saving baseline...${NC}"
    cp "$CURRENT_FILE" "$BASELINE_FILE"
    echo -e "${GREEN}✓ Baseline saved to $BASELINE_FILE${NC}"
fi

# Compare with baseline if requested
if [ "$COMPARE" = true ]; then
    if [ ! -f "$BASELINE_FILE" ]; then
        echo -e "${YELLOW}⚠ No baseline file found. Run with --save-baseline first.${NC}"
    else
        echo ""
        echo -e "${GREEN}Comparing with baseline...${NC}"
        echo ""

        # Install cargo-benchcmp if needed
        if ! command -v cargo-benchcmp &> /dev/null; then
            echo -e "${YELLOW}Installing cargo-benchcmp...${NC}"
            cargo install cargo-benchcmp
        fi

        cargo benchcmp "$BASELINE_FILE" "$CURRENT_FILE" | tee "$REPORT_DIR/comparison_${TIMESTAMP}.txt"
    fi
fi

# Generate profiling data if requested
if [ "$PROFILE" = true ]; then
    echo ""
    echo -e "${GREEN}Generating flamegraph...${NC}"

    # Install flamegraph if needed
    if ! command -v flamegraph &> /dev/null; then
        echo -e "${YELLOW}Installing flamegraph...${NC}"
        cargo install flamegraph
    fi

    # Requires root on Linux for perf
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo -e "${YELLOW}Note: Flamegraph requires root privileges for perf${NC}"
        sudo cargo flamegraph --bench benchmarks --features=bench -o "$REPORT_DIR/flamegraph_${TIMESTAMP}.svg"
    else
        cargo flamegraph --bench benchmarks --features=bench -o "$REPORT_DIR/flamegraph_${TIMESTAMP}.svg"
    fi

    echo -e "${GREEN}✓ Flamegraph saved to $REPORT_DIR/flamegraph_${TIMESTAMP}.svg${NC}"
fi

# Generate summary report
echo ""
echo -e "${GREEN}Generating summary report...${NC}"

cat > "$REPORT_DIR/summary_${TIMESTAMP}.md" << EOF
# Benchmark Summary Report

**Date**: $(date)
**Git Commit**: $(git rev-parse --short HEAD 2>/dev/null || echo "N/A")
**Rust Version**: $(rustc --version)

## System Information

- **OS**: $(uname -s)
- **Arch**: $(uname -m)
- **CPU**: $(grep "model name" /proc/cpuinfo 2>/dev/null | head -1 | cut -d: -f2 | xargs || echo "N/A")
- **Cores**: $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo "N/A")
- **Memory**: $(free -h 2>/dev/null | awk '/^Mem:/ {print $2}' || echo "N/A")

## Benchmark Results

### Credit Operations
$(grep -A 1 "bench_credit" "$CURRENT_FILE" 2>/dev/null | head -20 || echo "No results")

### QDAG Operations
$(grep -A 1 "bench_qdag" "$CURRENT_FILE" 2>/dev/null | head -20 || echo "No results")

### Task Queue Operations
$(grep -A 1 "bench_task" "$CURRENT_FILE" 2>/dev/null | head -20 || echo "No results")

### Security Operations
$(grep -A 1 "bench.*security\|bench_rate\|bench_reputation\|bench_qlearning\|bench_attack" "$CURRENT_FILE" 2>/dev/null | head -30 || echo "No results")

### Network Topology
$(grep -A 1 "bench.*topology\|bench_node_registration\|bench_peer\|bench_cluster" "$CURRENT_FILE" 2>/dev/null | head -20 || echo "No results")

### Economic Engine
$(grep -A 1 "bench.*economic\|bench_reward\|bench_epoch\|bench_sustainability" "$CURRENT_FILE" 2>/dev/null | head -20 || echo "No results")

## Performance Analysis

### Critical Bottlenecks

See [performance-analysis.md](../docs/performance-analysis.md) for detailed analysis.

### Recommendations

Based on current results:

1. Monitor operations >1ms
2. Investigate operations with high variance (>10%)
3. Profile hot paths with flamegraph
4. Consider caching for O(n) operations

## Next Steps

- [ ] Review bottlenecks above 1ms
- [ ] Implement caching for balance calculation
- [ ] Optimize attack pattern detection
- [ ] Add memory profiling
EOF

echo -e "${GREEN}✓ Summary saved to $REPORT_DIR/summary_${TIMESTAMP}.md${NC}"

# Display quick summary
echo ""
echo -e "${BLUE}═══════════════════════════════════════════════${NC}"
echo -e "${BLUE}    Quick Summary${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════${NC}"
echo ""

if [ -f "$CURRENT_FILE" ]; then
    echo -e "${YELLOW}Top 5 Slowest Operations:${NC}"
    grep "bench:" "$CURRENT_FILE" | sort -t':' -k2 -rn | head -5 | while read -r line; do
        echo "  $line"
    done
    echo ""

    echo -e "${YELLOW}Top 5 Fastest Operations:${NC}"
    grep "bench:" "$CURRENT_FILE" | sort -t':' -k2 -n | head -5 | while read -r line; do
        echo "  $line"
    done
fi

echo ""
echo -e "${GREEN}✓ Benchmarks complete!${NC}"
echo -e "${BLUE}Results saved to:${NC}"
echo -e "  - Current: $CURRENT_FILE"
echo -e "  - Reports: $REPORT_DIR/"
echo ""

# Open flamegraph if generated
if [ "$PROFILE" = true ] && [ -f "$REPORT_DIR/flamegraph_${TIMESTAMP}.svg" ]; then
    echo -e "${BLUE}Opening flamegraph...${NC}"
    if command -v xdg-open &> /dev/null; then
        xdg-open "$REPORT_DIR/flamegraph_${TIMESTAMP}.svg" &
    elif command -v open &> /dev/null; then
        open "$REPORT_DIR/flamegraph_${TIMESTAMP}.svg" &
    fi
fi
