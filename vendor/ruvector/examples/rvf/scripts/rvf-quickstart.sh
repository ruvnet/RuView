#!/usr/bin/env bash
# rvf-quickstart.sh — Linux / macOS RVF quick start
# Usage: bash scripts/rvf-quickstart.sh
set -euo pipefail

echo "=== RVF Quick Start (Linux/macOS) ==="

# ── 1. Install ──────────────────────────────────────────────
echo "[1/7] Installing RVF CLI and runtime..."
cargo install rvf-cli 2>/dev/null || echo "  (already installed)"
echo "  rvf version: $(rvf --version 2>/dev/null || echo 'build from source below')"

# ── 2. Create a vector store ────────────────────────────────
echo "[2/7] Creating vector store..."
rvf create demo.rvf --dimension 128
echo "  Created demo.rvf (128-dim, L2 metric)"

# ── 3. Ingest vectors from JSON ─────────────────────────────
echo "[3/7] Ingesting vectors..."
cat > /tmp/rvf_vectors.json <<'VECTORS'
[
  {"id": 1, "vector": [0.1, 0.2, 0.3], "metadata": {"label": "alpha"}},
  {"id": 2, "vector": [0.4, 0.5, 0.6], "metadata": {"label": "beta"}},
  {"id": 3, "vector": [0.7, 0.8, 0.9], "metadata": {"label": "gamma"}}
]
VECTORS
rvf ingest demo.rvf --input /tmp/rvf_vectors.json --format json
echo "  Ingested 3 vectors"

# ── 4. Query nearest neighbors ──────────────────────────────
echo "[4/7] Querying nearest neighbors..."
rvf query demo.rvf --vector "0.1,0.2,0.3" --k 2
echo "  Top-2 results returned"

# ── 5. Inspect segments ─────────────────────────────────────
echo "[5/7] Inspecting file segments..."
rvf inspect demo.rvf

# ── 6. Derive a child (COW branch) ──────────────────────────
echo "[6/7] Creating COW branch..."
rvf derive demo.rvf child.rvf --type filter
echo "  child.rvf inherits parent data, only stores changes"

# ── 7. Verify witness chain ─────────────────────────────────
echo "[7/7] Verifying tamper-evident witness chain..."
rvf verify-witness demo.rvf
echo "  Witness chain verified — no tampering detected"

echo ""
echo "=== Done ==="
echo "Files created: demo.rvf, child.rvf"
echo "Next: embed a kernel with 'rvf embed-kernel demo.rvf --arch x86_64'"
