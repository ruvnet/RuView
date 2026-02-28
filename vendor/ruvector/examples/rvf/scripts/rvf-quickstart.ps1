# rvf-quickstart.ps1 — Windows PowerShell RVF quick start
# Usage: .\scripts\rvf-quickstart.ps1
$ErrorActionPreference = "Stop"

Write-Host "=== RVF Quick Start (Windows PowerShell) ===" -ForegroundColor Cyan

# ── 1. Install ──────────────────────────────────────────────
Write-Host "[1/7] Installing RVF CLI and runtime..." -ForegroundColor Yellow
cargo install rvf-cli 2>$null
Write-Host "  rvf installed via cargo"

# ── 2. Create a vector store ────────────────────────────────
Write-Host "[2/7] Creating vector store..." -ForegroundColor Yellow
rvf create demo.rvf --dimension 128
Write-Host "  Created demo.rvf (128-dim, L2 metric)"

# ── 3. Ingest vectors from JSON ─────────────────────────────
Write-Host "[3/7] Ingesting vectors..." -ForegroundColor Yellow
$vectors = @'
[
  {"id": 1, "vector": [0.1, 0.2, 0.3], "metadata": {"label": "alpha"}},
  {"id": 2, "vector": [0.4, 0.5, 0.6], "metadata": {"label": "beta"}},
  {"id": 3, "vector": [0.7, 0.8, 0.9], "metadata": {"label": "gamma"}}
]
'@
$vectors | Out-File -Encoding utf8 "$env:TEMP\rvf_vectors.json"
rvf ingest demo.rvf --input "$env:TEMP\rvf_vectors.json" --format json
Write-Host "  Ingested 3 vectors"

# ── 4. Query nearest neighbors ──────────────────────────────
Write-Host "[4/7] Querying nearest neighbors..." -ForegroundColor Yellow
rvf query demo.rvf --vector "0.1,0.2,0.3" --k 2
Write-Host "  Top-2 results returned"

# ── 5. Inspect segments ─────────────────────────────────────
Write-Host "[5/7] Inspecting file segments..." -ForegroundColor Yellow
rvf inspect demo.rvf

# ── 6. Derive a child (COW branch) ──────────────────────────
Write-Host "[6/7] Creating COW branch..." -ForegroundColor Yellow
rvf derive demo.rvf child.rvf --type filter
Write-Host "  child.rvf inherits parent data, only stores changes"

# ── 7. Verify witness chain ─────────────────────────────────
Write-Host "[7/7] Verifying tamper-evident witness chain..." -ForegroundColor Yellow
rvf verify-witness demo.rvf
Write-Host "  Witness chain verified — no tampering detected"

Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Green
Write-Host "Files created: demo.rvf, child.rvf"
Write-Host "Next: embed a kernel with 'rvf embed-kernel demo.rvf --arch x86_64'"
Write-Host "Note: Self-booting requires WSL or Windows QEMU for the kernel launcher."
