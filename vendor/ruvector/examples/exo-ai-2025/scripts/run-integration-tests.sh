#!/bin/bash
# Integration Test Runner for EXO-AI 2025
#
# This script runs all integration tests for the cognitive substrate.
# It can run tests individually, in parallel, or with coverage reporting.

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
WORKSPACE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COVERAGE=false
PARALLEL=false
VERBOSE=false
FILTER=""

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --coverage)
            COVERAGE=true
            shift
            ;;
        --parallel)
            PARALLEL=true
            shift
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --filter)
            FILTER="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --coverage    Generate coverage report"
            echo "  --parallel    Run tests in parallel"
            echo "  --verbose     Enable verbose output"
            echo "  --filter STR  Run only tests matching STR"
            echo "  --help        Show this help message"
            exit 0
            ;;
        *)
            echo -e "${RED}Error: Unknown option $1${NC}"
            exit 1
            ;;
    esac
done

cd "$WORKSPACE_DIR"

echo -e "${GREEN}=== EXO-AI 2025 Integration Test Suite ===${NC}"
echo ""

# Check if crates exist
echo -e "${YELLOW}Checking for implemented crates...${NC}"
CRATES_EXIST=true

for crate in exo-core exo-manifold exo-hypergraph exo-temporal exo-federation; do
    if [ ! -d "crates/$crate" ]; then
        echo -e "${YELLOW}  ⚠ Crate not found: crates/$crate${NC}"
        CRATES_EXIST=false
    else
        echo -e "${GREEN}  ✓ Found: crates/$crate${NC}"
    fi
done

echo ""

if [ "$CRATES_EXIST" = false ]; then
    echo -e "${YELLOW}WARNING: Some crates are not implemented yet.${NC}"
    echo -e "${YELLOW}Integration tests are currently in TDD mode (all tests ignored).${NC}"
    echo -e "${YELLOW}Remove #[ignore] attributes as crates are implemented.${NC}"
    echo ""
fi

# Build test command
TEST_CMD="cargo test --workspace"

if [ "$VERBOSE" = true ]; then
    TEST_CMD="$TEST_CMD -- --nocapture --test-threads=1"
elif [ "$PARALLEL" = true ]; then
    TEST_CMD="$TEST_CMD -- --test-threads=8"
else
    TEST_CMD="$TEST_CMD -- --test-threads=4"
fi

if [ -n "$FILTER" ]; then
    TEST_CMD="$TEST_CMD $FILTER"
fi

# Run tests
echo -e "${GREEN}Running integration tests...${NC}"
echo -e "Command: ${YELLOW}$TEST_CMD${NC}"
echo ""

if [ "$COVERAGE" = true ]; then
    # Check if cargo-tarpaulin is installed
    if ! command -v cargo-tarpaulin &> /dev/null; then
        echo -e "${RED}Error: cargo-tarpaulin not installed${NC}"
        echo "Install with: cargo install cargo-tarpaulin"
        exit 1
    fi

    echo -e "${GREEN}Running with coverage...${NC}"
    cargo tarpaulin \
        --workspace \
        --out Html \
        --output-dir coverage \
        --exclude-files "tests/*" \
        --engine llvm

    echo ""
    echo -e "${GREEN}Coverage report generated: coverage/index.html${NC}"
else
    # Run standard tests
    if $TEST_CMD; then
        echo ""
        echo -e "${GREEN}✓ All tests passed!${NC}"
    else
        echo ""
        echo -e "${RED}✗ Some tests failed${NC}"
        exit 1
    fi
fi

echo ""
echo -e "${GREEN}=== Test Suite Complete ===${NC}"
