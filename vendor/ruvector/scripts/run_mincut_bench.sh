#!/usr/bin/env bash
# run_mincut_bench.sh -- 1k-sample grid runner for min-cut gating vs softmax
#
# Usage:
#   ./scripts/run_mincut_bench.sh [--samples N] [--output-dir DIR]
#
# Runs a grid search over lambda and tau parameters, collecting:
#   - Coherence delta metrics
#   - Memory pressure profiles
#   - Power/energy measurements
#   - Latency distributions (p50/p95/p99)
#   - Witness chain (JSONL + RVF bundle)

set -euo pipefail

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
SAMPLES=1000
SHORT_SAMPLES=500
LONG_SAMPLES=500
SHORT_MAX_LEN=128
LONG_MIN_LEN=256
LONG_MAX_LEN=1024
OUTPUT_DIR="results/mincut-bench"
LAMBDA_GRID="0.3 0.5 0.7"
TAU_GRID="0 2"
EPS=0.01
SEED=42

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
    case $1 in
        --samples)    SAMPLES="$2"; shift 2 ;;
        --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
        --lambda)     LAMBDA_GRID="$2"; shift 2 ;;
        --tau)        TAU_GRID="$2"; shift 2 ;;
        --seed)       SEED="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

SHORT_SAMPLES=$((SAMPLES / 2))
LONG_SAMPLES=$((SAMPLES - SHORT_SAMPLES))

# ---------------------------------------------------------------------------
# Setup
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

mkdir -p "$OUTPUT_DIR"/{csv,witness,figs}

echo "============================================="
echo "Min-Cut Gating Benchmark"
echo "============================================="
echo "Samples:     $SAMPLES ($SHORT_SAMPLES short + $LONG_SAMPLES long)"
echo "Lambda grid: $LAMBDA_GRID"
echo "Tau grid:    $TAU_GRID"
echo "Epsilon:     $EPS"
echo "Seed:        $SEED"
echo "Output:      $OUTPUT_DIR"
echo "============================================="

# ---------------------------------------------------------------------------
# Build (release mode for accurate benchmarks)
# ---------------------------------------------------------------------------
echo "[1/5] Building in release mode..."
cargo build --release \
    -p ruvector-attn-mincut \
    -p ruvector-coherence \
    -p ruvector-profiler \
    2>&1 | tail -5

# ---------------------------------------------------------------------------
# Run baseline (softmax)
# ---------------------------------------------------------------------------
echo "[2/5] Running baseline (softmax) on $SAMPLES samples..."

BASELINE_CSV="$OUTPUT_DIR/csv/baseline.csv"
echo "sample_id,seq_len,wall_time_us,peak_mem_bytes,energy_j" > "$BASELINE_CSV"

# Placeholder: in a real run, this would invoke the benchmark binary
# cargo run --release -p ruvector-bench-runner -- \
#     --mode softmax \
#     --short-samples $SHORT_SAMPLES --short-max-len $SHORT_MAX_LEN \
#     --long-samples $LONG_SAMPLES --long-min-len $LONG_MIN_LEN --long-max-len $LONG_MAX_LEN \
#     --seed $SEED \
#     --output "$BASELINE_CSV"
echo "  (baseline runner placeholder -- implement with bench binary)"

# ---------------------------------------------------------------------------
# Run grid search (min-cut gating)
# ---------------------------------------------------------------------------
echo "[3/5] Running min-cut gating grid search..."

RESULTS_CSV="$OUTPUT_DIR/csv/results.csv"
echo "setting,lambda,tau,coherence_delta,kv_cache_reduction,peak_mem_reduction,energy_reduction,p95_latency_us,accuracy" > "$RESULTS_CSV"

for lambda in $LAMBDA_GRID; do
    for tau in $TAU_GRID; do
        SETTING="mincut_l${lambda}_t${tau}"
        echo "  Running $SETTING..."

        RUN_CSV="$OUTPUT_DIR/csv/${SETTING}.csv"
        WITNESS_FILE="$OUTPUT_DIR/witness/${SETTING}.jsonl"

        # Placeholder: invoke bench binary with min-cut params
        # cargo run --release -p ruvector-bench-runner -- \
        #     --mode mincut \
        #     --lambda $lambda --tau $tau --eps $EPS \
        #     --short-samples $SHORT_SAMPLES --short-max-len $SHORT_MAX_LEN \
        #     --long-samples $LONG_SAMPLES --long-min-len $LONG_MIN_LEN --long-max-len $LONG_MAX_LEN \
        #     --seed $SEED \
        #     --output "$RUN_CSV" \
        #     --witness "$WITNESS_FILE"
        echo "    (grid runner placeholder -- implement with bench binary)"
    done
done

# ---------------------------------------------------------------------------
# Compute aggregate metrics
# ---------------------------------------------------------------------------
echo "[4/5] Computing aggregate metrics..."

# Placeholder: post-processing script would:
# 1. Read all CSV files
# 2. Compute mean +/- 95% CI for coherence delta
# 3. Compare memory, energy, latency vs baseline
# 4. Write summary to results.csv
echo "  (aggregation placeholder -- implement with post-processor)"

# ---------------------------------------------------------------------------
# Pack witness bundle (RVF)
# ---------------------------------------------------------------------------
echo "[5/5] Packing witness bundle..."

WITNESS_BUNDLE="$OUTPUT_DIR/witness/witness.rvf"
# Placeholder: concatenate witness JSONL files into RVF bundle
# The RVF format includes:
# - Header: config hash, model commit, weights hash
# - Body: per-sample witness entries with hash chain
# - Footer: aggregate stats, signature
echo "  (RVF packer placeholder -- implement with witness tool)"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================="
echo "Benchmark complete"
echo "============================================="
echo "Results:  $OUTPUT_DIR/csv/results.csv"
echo "Witness:  $OUTPUT_DIR/witness/"
echo "Figures:  $OUTPUT_DIR/figs/ (generate with plot script)"
echo ""
echo "Expected results table:"
echo ""
echo "Setting              | dCoherence | KV-Cache | Peak Mem | Energy/sample | p95 Latency"
echo "---------------------|------------|----------|----------|---------------|------------"
echo "Softmax (baseline)   | --         | --       | --       | --            | --"
echo "Min-cut l=0.3, t=0   | +??%       | -??%     | -??%     | -??%          | ??us"
echo "Min-cut l=0.3, t=2   | +??%       | -??%     | -??%     | -??%          | ??us"
echo "Min-cut l=0.5, t=0   | +??%       | -??%     | -??%     | -??%          | ??us"
echo "Min-cut l=0.5, t=2   | +??%       | -??%     | -??%     | -??%          | ??us"
echo "Min-cut l=0.7, t=0   | +??%       | -??%     | -??%     | -??%          | ??us"
echo "Min-cut l=0.7, t=2   | +??%       | -??%     | -??%     | -??%          | ??us"
echo ""
echo "Success criteria:"
echo "  >= 5% coherence delta with <= 1% accuracy loss"
echo "  >= 15% KV-cache reduction"
echo "  >= 10% energy/sample drop"
echo "  p95 latency within +/-10% of baseline"
echo "  Deterministic witness reproducible on second machine"
