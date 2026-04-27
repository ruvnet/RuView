#!/bin/bash
set -euo pipefail

echo "=== WiFi-DensePose Mac Mini M4 Pro Training Pipeline ==="
echo "Host: $(hostname) | $(sysctl -n hw.ncpu 2>/dev/null || nproc) cores | $(sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.0f GB", $1/1073741824}' || free -h | awk '/Mem:/{print $2}')"
echo ""

REPO_DIR="${HOME}/Projects/wifi-densepose"
WINDOWS_HOST="${WINDOWS_HOST:-}"  # Set via env: export WINDOWS_HOST=<tailscale-ip>

# Step 1: Clone or update repo
echo "[1/7] Setting up repository..."
if [ -d "$REPO_DIR/.git" ]; then
  cd "$REPO_DIR" && git pull origin main
else
  git clone https://github.com/ruvnet/RuView.git "$REPO_DIR"
  cd "$REPO_DIR"
fi

# Step 2: Install Node.js if needed
echo "[2/7] Checking Node.js..."
if ! command -v node &>/dev/null; then
  echo "Installing Node.js via Homebrew..."
  brew install node
fi
echo "Node $(node --version)"

# Step 3: Copy training data from Windows via Tailscale
echo "[3/7] Copying training data from Windows machine..."
mkdir -p data/recordings
scp -o ConnectTimeout=5 "ruv@${WINDOWS_HOST}:Projects/wifi-densepose/data/recordings/pretrain-*.csi.jsonl" data/recordings/ 2>/dev/null || {
  echo "  Could not reach Windows machine. Checking for local data..."
  if ls data/recordings/pretrain-*.csi.jsonl &>/dev/null; then
    echo "  Found local training data."
  else
    echo "  ERROR: No training data found. Run collect-training-data.py on Windows first."
    exit 1
  fi
}
echo "  Data: $(wc -l data/recordings/pretrain-*.csi.jsonl | tail -1)"

# Step 4: Run enhanced training (larger model, more epochs)
echo "[4/7] Training (enhanced config for M4 Pro)..."
time node scripts/train-ruvllm.js \
  --data data/recordings/pretrain-*.csi.jsonl \
  2>&1 | tee models/csi-ruvllm/training.log

# Step 5: Benchmark
echo "[5/7] Benchmarking..."
node scripts/benchmark-ruvllm.js \
  --model models/csi-ruvllm \
  --data data/recordings/pretrain-*.csi.jsonl \
  2>&1 | tee models/csi-ruvllm/benchmark.log

# Step 6: Copy results back to Windows
echo "[6/7] Syncing results back to Windows..."
scp -r -o ConnectTimeout=5 models/csi-ruvllm/ "ruv@${WINDOWS_HOST}:Projects/wifi-densepose/models/csi-ruvllm-m4pro/" 2>/dev/null || {
  echo "  Could not reach Windows. Results are in: $REPO_DIR/models/csi-ruvllm/"
}

# Step 7: Publish to HuggingFace
echo "[7/7] Publishing to HuggingFace..."
if command -v gcloud &>/dev/null; then
  mkdir -p dist/models
  cp models/csi-ruvllm/model.safetensors dist/models/
  cp models/csi-ruvllm/config.json dist/models/
  cp models/csi-ruvllm/presence-head.json dist/models/
  cp models/csi-ruvllm/quantized/* dist/models/ 2>/dev/null || true
  cp models/csi-ruvllm/lora/* dist/models/ 2>/dev/null || true
  cp models/csi-ruvllm/model.rvf.jsonl dist/models/ 2>/dev/null || true
  cp models/csi-ruvllm/training-metrics.json dist/models/ 2>/dev/null || true
  cp docs/huggingface/MODEL_CARD.md dist/models/README.md 2>/dev/null || true
  bash scripts/publish-huggingface.sh --version v0.5.4 2>&1 || echo "  HF publish skipped (check gcloud auth)"
else
  echo "  gcloud not installed — skipping HF publish. Run manually:"
  echo "  bash scripts/publish-huggingface.sh --version v0.5.4"
fi

echo ""
echo "=== Complete ==="
echo "Models: $REPO_DIR/models/csi-ruvllm/"
echo "Logs: training.log, benchmark.log"
