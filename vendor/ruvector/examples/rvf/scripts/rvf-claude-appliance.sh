#!/usr/bin/env bash
# rvf-claude-appliance.sh — Build & boot the Claude Code Appliance (Linux/macOS)
# Prerequisites: Docker, Rust 1.87+, QEMU (optional, for booting)
# Usage: bash scripts/rvf-claude-appliance.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo "=== Claude Code Appliance Builder (Linux/macOS) ==="

# ── 1. Check prerequisites ──────────────────────────────────
echo "[1/5] Checking prerequisites..."
command -v cargo >/dev/null 2>&1 || { echo "ERROR: Rust/cargo not found. Install from https://rustup.rs"; exit 1; }
command -v docker >/dev/null 2>&1 || { echo "ERROR: Docker not found. Install from https://docker.com"; exit 1; }
echo "  cargo: $(cargo --version)"
echo "  docker: $(docker --version)"

# ── 2. Build the appliance ──────────────────────────────────
echo "[2/5] Building Claude Code Appliance (this builds a real Linux kernel)..."
cd "$SCRIPT_DIR"
cargo run --example claude_code_appliance
echo "  Built: output/claude_code_appliance.rvf"

# ── 3. Inspect the result ───────────────────────────────────
echo "[3/5] Inspecting appliance segments..."
APPLIANCE="output/claude_code_appliance.rvf"
ls -lh "$APPLIANCE"
rvf inspect "$APPLIANCE" 2>/dev/null || echo "  (install rvf-cli for detailed inspection)"
echo ""

# ── 4. Query the embedded vector store ──────────────────────
echo "[4/5] Querying package database..."
rvf query "$APPLIANCE" --vector "0.1,0.2,0.3" --k 3 2>/dev/null || \
  echo "  (install rvf-cli to query, or use the Rust API)"

# ── 5. Boot (optional) ─────────────────────────────────────
echo "[5/5] Boot instructions:"
echo ""
echo "  # Option A: RVF launcher (auto-detects KVM or TCG)"
echo "  rvf launch $APPLIANCE"
echo ""
echo "  # Option B: Manual QEMU"
echo "  rvf launch $APPLIANCE --memory 512M --cpus 2 --port-forward 2222:22,8080:8080"
echo ""
echo "  # Connect:"
echo "  ssh -p 2222 deploy@localhost"
echo "  curl -s localhost:8080/query -d '{\"vector\":[0.1,...], \"k\":5}'"
echo ""

if command -v qemu-system-x86_64 >/dev/null 2>&1; then
  read -rp "QEMU detected. Boot the appliance now? [y/N] " answer
  if [[ "$answer" =~ ^[Yy]$ ]]; then
    echo "Booting..."
    rvf launch "$APPLIANCE" 2>/dev/null || \
      echo "Install rvf-cli to use the launcher, or extract kernel manually."
  fi
fi

echo ""
echo "=== Claude Code Appliance ready ==="
echo "  File: $APPLIANCE"
echo "  Size: $(du -h "$APPLIANCE" | cut -f1)"
echo "  5.1 MB single .rvf — boots Linux, serves queries, runs Claude Code."
