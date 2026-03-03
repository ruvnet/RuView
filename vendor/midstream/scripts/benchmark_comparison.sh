#!/bin/bash
# Compare benchmark results between two git branches
# Usage: ./benchmark_comparison.sh <baseline-branch> <feature-branch>

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

BASELINE_BRANCH="${1:-main}"
FEATURE_BRANCH="${2:-$(git branch --show-current)}"

echo "📊 Benchmark Comparison Tool"
echo "============================"
echo ""
echo "Baseline: $BASELINE_BRANCH"
echo "Feature:  $FEATURE_BRANCH"
echo ""

# Save current branch
CURRENT_BRANCH=$(git branch --show-current)

# Function to run benchmarks on a branch
run_benchmarks_on_branch() {
    local branch=$1
    local baseline_name=$2

    echo -e "${YELLOW}Checking out $branch...${NC}"
    git checkout "$branch"

    echo -e "${YELLOW}Building $branch...${NC}"
    cargo build --release --all-features

    echo -e "${YELLOW}Running benchmarks on $branch...${NC}"
    cargo bench --all -- --save-baseline "$baseline_name"

    echo -e "${GREEN}✓ Benchmarks for $branch completed${NC}"
    echo ""
}

# Run benchmarks on baseline
run_benchmarks_on_branch "$BASELINE_BRANCH" "baseline"

# Run benchmarks on feature branch
run_benchmarks_on_branch "$FEATURE_BRANCH" "feature"

# Compare results
echo -e "${YELLOW}Generating comparison...${NC}"

# Return to original branch
git checkout "$CURRENT_BRANCH"

# Run comparison
cargo bench --all -- --baseline baseline

echo ""
echo -e "${GREEN}Comparison complete!${NC}"
echo ""
echo "Results:"
echo "  - Baseline: $BASELINE_BRANCH"
echo "  - Feature:  $FEATURE_BRANCH"
echo "  - Reports:  target/criterion/*/report/index.html"
echo ""
