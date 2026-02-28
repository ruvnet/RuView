#!/bin/bash
# Comprehensive Benchmark Runner for Edge-Net

set -e

echo "=========================================="
echo "Edge-Net Comprehensive Benchmark Suite"
echo "=========================================="
echo ""

# Create benchmark output directory
BENCH_DIR="benchmark_results"
mkdir -p "$BENCH_DIR"

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="$BENCH_DIR/benchmark_report_$TIMESTAMP.md"

echo "Running benchmarks..."
echo "Results will be saved to: $REPORT_FILE"
echo ""

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "Error: Must be run from the edge-net directory"
    exit 1
fi

# Run benchmarks with the bench feature
echo "Building with bench feature..."
cargo build --release --features bench

echo ""
echo "Running benchmark suite..."
echo "This may take several minutes..."
echo ""

# Run specific benchmark categories
echo "1. Spike-Driven Attention Benchmarks..."
cargo bench --features bench -- spike_encoding 2>&1 | tee -a "$BENCH_DIR/spike_encoding.txt"
cargo bench --features bench -- spike_attention 2>&1 | tee -a "$BENCH_DIR/spike_attention.txt"

echo ""
echo "2. RAC Coherence Benchmarks..."
cargo bench --features bench -- rac_ 2>&1 | tee -a "$BENCH_DIR/rac_benchmarks.txt"

echo ""
echo "3. Learning Module Benchmarks..."
cargo bench --features bench -- reasoning_bank 2>&1 | tee -a "$BENCH_DIR/learning_benchmarks.txt"
cargo bench --features bench -- trajectory 2>&1 | tee -a "$BENCH_DIR/trajectory_benchmarks.txt"

echo ""
echo "4. Multi-Head Attention Benchmarks..."
cargo bench --features bench -- multi_head 2>&1 | tee -a "$BENCH_DIR/attention_benchmarks.txt"

echo ""
echo "5. Integration Benchmarks..."
cargo bench --features bench -- integration 2>&1 | tee -a "$BENCH_DIR/integration_benchmarks.txt"
cargo bench --features bench -- end_to_end 2>&1 | tee -a "$BENCH_DIR/e2e_benchmarks.txt"

echo ""
echo "=========================================="
echo "Benchmark Suite Complete!"
echo "=========================================="
echo ""
echo "Results saved to: $BENCH_DIR/"
echo ""
echo "To view results:"
echo "  cat $BENCH_DIR/*.txt"
echo ""
