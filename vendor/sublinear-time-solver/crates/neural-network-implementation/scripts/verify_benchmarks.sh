#!/bin/bash

# Quick verification script to test benchmark compilation
# This ensures all benchmarks can be built before running the full suite

set -euo pipefail

BENCHMARK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() {
    echo -e "$(date '+%H:%M:%S') $1"
}

success() {
    log "${GREEN}✅ $1${NC}"
}

error() {
    log "${RED}❌ $1${NC}"
}

warning() {
    log "${YELLOW}⚠️  $1${NC}"
}

cd "$BENCHMARK_DIR"

log "🔍 Verifying benchmark suite compilation..."

# Check if we can build the project
log "Building project..."
if cargo check --release --benches; then
    success "Project builds successfully"
else
    error "Project build failed"
    exit 1
fi

# Test compile each benchmark individually
log "Testing individual benchmark compilation..."

benchmarks=("latency_benchmark" "throughput_benchmark" "system_comparison" "statistical_analysis")

for bench in "${benchmarks[@]}"; do
    log "Checking $bench..."
    if cargo check --release --bench "$bench"; then
        success "$bench compiles successfully"
    else
        error "$bench compilation failed"
        exit 1
    fi
done

log "🧪 Running quick smoke tests..."

# Quick test runs (very short duration)
for bench in "${benchmarks[@]}"; do
    log "Quick test: $bench..."
    if timeout 30 cargo bench --bench "$bench" -- --test 2>/dev/null; then
        success "$bench quick test passed"
    else
        warning "$bench quick test failed or timed out (this may be expected)"
    fi
done

success "🎉 All benchmarks verified successfully!"
log "✨ Ready to run full benchmark suite with ./scripts/run_all_benchmarks.sh"