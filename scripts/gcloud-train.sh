#!/bin/bash
# ==============================================================================
# GCloud GPU Training Script for WiFi-DensePose
# ==============================================================================
#
# Creates a GCloud VM with GPU, runs the Rust training pipeline, downloads
# the trained model artifacts, and tears down the VM to avoid ongoing costs.
#
# Usage:
#   bash scripts/gcloud-train.sh [OPTIONS]
#
# Options:
#   --gpu        l4|a100|h100       GPU type (default: l4)
#   --zone       ZONE               GCloud zone (default: us-central1-a)
#   --hours      N                  Max VM lifetime in hours (default: 2)
#   --config     FILE               Training config JSON (default: scripts/training-config-sweep.json entry 0)
#   --data-dir   DIR                Local data directory to upload (default: data/recordings)
#   --dry-run                       Run smoke test with synthetic data
#   --sweep                         Run full hyperparameter sweep (all configs)
#   --keep-vm                       Do not delete VM after training
#   --instance   NAME               Custom VM instance name
#
# Prerequisites:
#   - gcloud CLI authenticated: gcloud auth login
#   - Project set: gcloud config set project cognitum-20260110
#   - Quota for GPUs in the selected zone
#
# Cost estimates:
#   L4 (~$0.80/hr) — good for prototyping and small sweeps
#   A100 40GB (~$3.60/hr) — full training runs
#   H100 80GB (~$11.00/hr) — large batch / fast iteration
# ==============================================================================

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────

PROJECT="cognitum-20260110"
GPU_TYPE="l4"
ZONE="us-central1-a"
MAX_HOURS=2
CONFIG_FILE=""
DATA_DIR="data/recordings"
DRY_RUN=false
SWEEP=false
KEEP_VM=false
INSTANCE_NAME=""
REPO_URL="https://github.com/ruvnet/wifi-densepose.git"
BRANCH="main"

# ── Parse arguments ───────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --gpu)       GPU_TYPE="$2";      shift 2 ;;
        --zone)      ZONE="$2";          shift 2 ;;
        --hours)     MAX_HOURS="$2";     shift 2 ;;
        --config)    CONFIG_FILE="$2";   shift 2 ;;
        --data-dir)  DATA_DIR="$2";      shift 2 ;;
        --dry-run)   DRY_RUN=true;       shift   ;;
        --sweep)     SWEEP=true;         shift   ;;
        --keep-vm)   KEEP_VM=true;       shift   ;;
        --instance)  INSTANCE_NAME="$2"; shift 2 ;;
        --branch)    BRANCH="$2";        shift 2 ;;
        -h|--help)
            head -35 "$0" | tail -30
            exit 0
            ;;
        *)
            echo "ERROR: Unknown option: $1"
            exit 1
            ;;
    esac
done

# ── GPU configuration map ────────────────────────────────────────────────────

declare -A GPU_ACCELERATOR=(
    [l4]="nvidia-l4"
    [a100]="nvidia-tesla-a100"
    [h100]="nvidia-h100-80gb"
)

declare -A GPU_MACHINE_TYPE=(
    [l4]="g2-standard-8"
    [a100]="a2-highgpu-1g"
    [h100]="a3-highgpu-1g"
)

declare -A GPU_BOOT_DISK=(
    [l4]="200"
    [a100]="300"
    [h100]="300"
)

if [[ -z "${GPU_ACCELERATOR[$GPU_TYPE]+x}" ]]; then
    echo "ERROR: Unknown GPU type '$GPU_TYPE'. Choose: l4, a100, h100"
    exit 1
fi

ACCELERATOR="${GPU_ACCELERATOR[$GPU_TYPE]}"
MACHINE_TYPE="${GPU_MACHINE_TYPE[$GPU_TYPE]}"
BOOT_DISK_GB="${GPU_BOOT_DISK[$GPU_TYPE]}"

# ── Instance naming ──────────────────────────────────────────────────────────

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
if [[ -z "$INSTANCE_NAME" ]]; then
    INSTANCE_NAME="wdp-train-${GPU_TYPE}-${TIMESTAMP}"
fi

# ── Announce plan ────────────────────────────────────────────────────────────

echo "============================================================"
echo "  WiFi-DensePose GCloud GPU Training"
echo "============================================================"
echo "  Project:      $PROJECT"
echo "  Instance:     $INSTANCE_NAME"
echo "  Zone:         $ZONE"
echo "  GPU:          $GPU_TYPE ($ACCELERATOR)"
echo "  Machine:      $MACHINE_TYPE"
echo "  Boot disk:    ${BOOT_DISK_GB}GB"
echo "  Max runtime:  ${MAX_HOURS}h"
echo "  Data dir:     $DATA_DIR"
echo "  Dry run:      $DRY_RUN"
echo "  Sweep:        $SWEEP"
echo "  Branch:       $BRANCH"
echo "============================================================"
echo ""

# ── Verify gcloud auth ──────────────────────────────────────────────────────

if ! gcloud auth list --filter=status:ACTIVE --format="value(account)" 2>/dev/null | head -1 | grep -q '@'; then
    echo "ERROR: No active gcloud account. Run: gcloud auth login"
    exit 1
fi

gcloud config set project "$PROJECT" --quiet

# ── Build startup script ─────────────────────────────────────────────────────

STARTUP_SCRIPT=$(cat <<'STARTUP_EOF'
#!/bin/bash
set -euo pipefail
exec > /var/log/wdp-setup.log 2>&1

echo "=== WiFi-DensePose GPU VM Setup ==="
echo "Started: $(date)"

# Wait for GPU driver
echo "Waiting for NVIDIA driver..."
for i in $(seq 1 60); do
    if nvidia-smi &>/dev/null; then
        echo "GPU ready after ${i}s"
        nvidia-smi
        break
    fi
    sleep 5
done

if ! nvidia-smi &>/dev/null; then
    echo "ERROR: GPU driver not available after 300s"
    exit 1
fi

# Install Rust toolchain
echo "Installing Rust toolchain..."
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
source "$HOME/.cargo/env"
rustc --version
cargo --version

# Install system dependencies
echo "Installing system dependencies..."
apt-get update -qq
apt-get install -y -qq pkg-config libssl-dev cmake clang

# Find libtorch from the Deep Learning VM's PyTorch installation
echo "Locating libtorch..."
PYTORCH_LIB=$(python3 -c "import torch; print(torch.__path__[0] + '/lib')" 2>/dev/null || echo "")
if [[ -n "$PYTORCH_LIB" && -d "$PYTORCH_LIB" ]]; then
    export LIBTORCH="$PYTORCH_LIB"
    export LD_LIBRARY_PATH="${LIBTORCH}:${LD_LIBRARY_PATH:-}"
    echo "Found libtorch at: $LIBTORCH"
else
    echo "WARNING: PyTorch not found in system Python. Installing via pip..."
    pip3 install torch --index-url https://download.pytorch.org/whl/cu121
    PYTORCH_LIB=$(python3 -c "import torch; print(torch.__path__[0] + '/lib')")
    export LIBTORCH="$PYTORCH_LIB"
    export LD_LIBRARY_PATH="${LIBTORCH}:${LD_LIBRARY_PATH:-}"
fi

# Persist env vars
cat >> /etc/environment <<ENV_VARS
LIBTORCH=$LIBTORCH
LD_LIBRARY_PATH=$LIBTORCH:\$LD_LIBRARY_PATH
PATH=$HOME/.cargo/bin:\$PATH
ENV_VARS

echo "=== Setup complete: $(date) ==="
touch /tmp/wdp-setup-done
STARTUP_EOF
)

# ── Step 1: Create the VM ────────────────────────────────────────────────────

echo "[1/7] Creating VM instance: $INSTANCE_NAME ..."

gcloud compute instances create "$INSTANCE_NAME" \
    --project="$PROJECT" \
    --zone="$ZONE" \
    --machine-type="$MACHINE_TYPE" \
    --accelerator="type=$ACCELERATOR,count=1" \
    --image-family="common-cu121-ubuntu-2204" \
    --image-project="deeplearning-platform-release" \
    --boot-disk-size="${BOOT_DISK_GB}GB" \
    --boot-disk-type="pd-ssd" \
    --maintenance-policy=TERMINATE \
    --metadata="install-nvidia-driver=True" \
    --metadata-from-file="startup-script=<(echo "$STARTUP_SCRIPT")" \
    --scopes="default,storage-rw" \
    --labels="purpose=wdp-training,gpu=${GPU_TYPE}" \
    --quiet

echo "  VM created. Waiting for startup script to complete..."

# ── Step 2: Wait for setup ───────────────────────────────────────────────────

echo "[2/7] Waiting for setup to complete (GPU driver + Rust toolchain)..."

for i in $(seq 1 60); do
    if gcloud compute ssh "$INSTANCE_NAME" --zone="$ZONE" --command="test -f /tmp/wdp-setup-done" --quiet 2>/dev/null; then
        echo "  Setup complete after $((i * 15))s"
        break
    fi
    if [[ $i -eq 60 ]]; then
        echo "ERROR: Setup timed out after 15 minutes."
        echo "Check logs: gcloud compute ssh $INSTANCE_NAME --zone=$ZONE --command='cat /var/log/wdp-setup.log'"
        if [[ "$KEEP_VM" == "false" ]]; then
            echo "Cleaning up VM..."
            gcloud compute instances delete "$INSTANCE_NAME" --zone="$ZONE" --quiet
        fi
        exit 1
    fi
    sleep 15
done

# ── Step 3: Clone repo and build ─────────────────────────────────────────────

echo "[3/7] Cloning repository and building training binary..."

gcloud compute ssh "$INSTANCE_NAME" --zone="$ZONE" --command="$(cat <<CLONE_EOF
set -euo pipefail
source \$HOME/.cargo/env

# Clone the repo
if [[ ! -d ~/wifi-densepose ]]; then
    git clone --depth 1 --branch "$BRANCH" "$REPO_URL" ~/wifi-densepose
fi

# Set libtorch environment
export LIBTORCH=\$(python3 -c "import torch; print(torch.__path__[0] + '/lib')")
export LD_LIBRARY_PATH="\${LIBTORCH}:\${LD_LIBRARY_PATH:-}"

# Build the training binary with tch-backend
cd ~/wifi-densepose/v2
echo "Building with LIBTORCH=\$LIBTORCH ..."
cargo build --release --features tch-backend --bin train 2>&1 | tail -5

echo "Build complete."
ls -lh target/release/train
CLONE_EOF
)"

# ── Step 4: Upload training data ─────────────────────────────────────────────

echo "[4/7] Uploading training data..."

if [[ -d "$DATA_DIR" ]] && [[ "$(ls -A "$DATA_DIR" 2>/dev/null)" ]]; then
    # Create a tarball of the data directory
    DATA_TAR="/tmp/wdp-training-data-${TIMESTAMP}.tar.gz"
    tar czf "$DATA_TAR" -C "$(dirname "$DATA_DIR")" "$(basename "$DATA_DIR")"
    DATA_SIZE=$(du -h "$DATA_TAR" | cut -f1)
    echo "  Uploading ${DATA_SIZE} of training data..."

    gcloud compute scp "$DATA_TAR" "${INSTANCE_NAME}:~/training-data.tar.gz" --zone="$ZONE" --quiet
    gcloud compute ssh "$INSTANCE_NAME" --zone="$ZONE" --command="
        mkdir -p ~/wifi-densepose/data
        tar xzf ~/training-data.tar.gz -C ~/wifi-densepose/data/
        echo 'Data extracted:'
        find ~/wifi-densepose/data -name '*.jsonl' -o -name '*.csi.jsonl' | head -20
    "
    rm -f "$DATA_TAR"
else
    echo "  No local data at '$DATA_DIR'. Training will use --dry-run or MM-Fi."
    if [[ "$DRY_RUN" == "false" && "$SWEEP" == "false" ]]; then
        echo "  WARNING: No data and --dry-run not set. Forcing --dry-run."
        DRY_RUN=true
    fi
fi

# ── Step 5: Upload config and run training ────────────────────────────────────

echo "[5/7] Running training..."

# Upload sweep config if doing a sweep
if [[ "$SWEEP" == "true" ]]; then
    SWEEP_FILE="scripts/training-config-sweep.json"
    if [[ -f "$SWEEP_FILE" ]]; then
        gcloud compute scp "$SWEEP_FILE" "${INSTANCE_NAME}:~/sweep-configs.json" --zone="$ZONE" --quiet
    else
        echo "ERROR: Sweep config not found at $SWEEP_FILE"
        exit 1
    fi
fi

# Upload single config if specified
if [[ -n "$CONFIG_FILE" ]]; then
    gcloud compute scp "$CONFIG_FILE" "${INSTANCE_NAME}:~/train-config.json" --zone="$ZONE" --quiet
fi

# Build the training command
TRAIN_CMD_BASE="
set -euo pipefail
source \$HOME/.cargo/env
export LIBTORCH=\$(python3 -c \"import torch; print(torch.__path__[0] + '/lib')\")
export LD_LIBRARY_PATH=\"\${LIBTORCH}:\${LD_LIBRARY_PATH:-}\"
cd ~/wifi-densepose/v2

# Set auto-shutdown timer (safety net)
sudo shutdown -P +$((MAX_HOURS * 60)) &

TRAIN_BIN=./target/release/train
"

if [[ "$SWEEP" == "true" ]]; then
    # Run all configs in the sweep file
    gcloud compute ssh "$INSTANCE_NAME" --zone="$ZONE" --command="$(cat <<SWEEP_EOF
$TRAIN_CMD_BASE

echo "=== Hyperparameter Sweep ==="
SWEEP_FILE=~/sweep-configs.json
NUM_CONFIGS=\$(python3 -c "import json; print(len(json.load(open('\$SWEEP_FILE'))['configs']))")
echo "Running \$NUM_CONFIGS configurations..."

mkdir -p ~/results

for i in \$(seq 0 \$((NUM_CONFIGS - 1))); do
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  Config \$((i+1)) / \$NUM_CONFIGS"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    # Extract single config to temp file
    python3 -c "
import json, sys
sweep = json.load(open('\$SWEEP_FILE'))
cfg = sweep['configs'][\$i]
# Merge with base config
base = sweep.get('base', {})
merged = {**base, **cfg}
# Set checkpoint dir per config
merged['checkpoint_dir'] = f'checkpoints/sweep_{i:02d}'
merged['log_dir'] = f'logs/sweep_{i:02d}'
json.dump(merged, open('/tmp/sweep_config_\${i}.json', 'w'), indent=2)
print(f\"Config \${i}: lr={merged.get('learning_rate', '?')}, bs={merged.get('batch_size', '?')}, bb={merged.get('backbone_channels', '?')}\")
"

    START_TIME=\$(date +%s)

    \$TRAIN_BIN --config /tmp/sweep_config_\${i}.json --cuda $( [[ "$DRY_RUN" == "true" ]] && echo "--dry-run" ) 2>&1 | tee ~/results/sweep_\${i}.log || true

    END_TIME=\$(date +%s)
    ELAPSED=\$(( END_TIME - START_TIME ))
    echo "  Completed in \${ELAPSED}s"
done

echo ""
echo "=== Sweep Complete ==="
echo "Results in ~/results/"
ls -lh ~/results/
SWEEP_EOF
)"
elif [[ -n "$CONFIG_FILE" ]]; then
    # Single config run
    gcloud compute ssh "$INSTANCE_NAME" --zone="$ZONE" --command="$(cat <<SINGLE_EOF
$TRAIN_CMD_BASE
echo "=== Training with custom config ==="
\$TRAIN_BIN --config ~/train-config.json --cuda $( [[ "$DRY_RUN" == "true" ]] && echo "--dry-run" ) 2>&1 | tee ~/train.log
SINGLE_EOF
)"
else
    # Default config run
    gcloud compute ssh "$INSTANCE_NAME" --zone="$ZONE" --command="$(cat <<DEFAULT_EOF
$TRAIN_CMD_BASE
echo "=== Training with default config ==="
\$TRAIN_BIN --cuda $( [[ "$DRY_RUN" == "true" ]] && echo "--dry-run --dry-run-samples 256" ) 2>&1 | tee ~/train.log
DEFAULT_EOF
)"
fi

# ── Step 6: Download results ─────────────────────────────────────────────────

echo "[6/7] Downloading trained model artifacts..."

LOCAL_RESULTS="training-results/${INSTANCE_NAME}"
mkdir -p "$LOCAL_RESULTS"

# Package results on the VM
gcloud compute ssh "$INSTANCE_NAME" --zone="$ZONE" --command="
cd ~/wifi-densepose/v2
tar czf ~/training-artifacts.tar.gz \
    checkpoints/ \
    logs/ \
    2>/dev/null || true

# Also grab sweep results if they exist
if [[ -d ~/results ]]; then
    tar czf ~/sweep-results.tar.gz -C ~ results/ 2>/dev/null || true
fi

ls -lh ~/training-artifacts.tar.gz ~/sweep-results.tar.gz 2>/dev/null || true
"

# Download artifacts
gcloud compute scp "${INSTANCE_NAME}:~/training-artifacts.tar.gz" \
    "${LOCAL_RESULTS}/training-artifacts.tar.gz" --zone="$ZONE" --quiet 2>/dev/null || true

if [[ "$SWEEP" == "true" ]]; then
    gcloud compute scp "${INSTANCE_NAME}:~/sweep-results.tar.gz" \
        "${LOCAL_RESULTS}/sweep-results.tar.gz" --zone="$ZONE" --quiet 2>/dev/null || true
fi

# Download training log
gcloud compute scp "${INSTANCE_NAME}:~/train.log" \
    "${LOCAL_RESULTS}/train.log" --zone="$ZONE" --quiet 2>/dev/null || true

# Extract locally
if [[ -f "${LOCAL_RESULTS}/training-artifacts.tar.gz" ]]; then
    tar xzf "${LOCAL_RESULTS}/training-artifacts.tar.gz" -C "$LOCAL_RESULTS/"
    echo "  Artifacts extracted to: $LOCAL_RESULTS/"
    find "$LOCAL_RESULTS" -name "*.pt" -o -name "*.onnx" -o -name "*.rvf" 2>/dev/null | head -20
fi

# ── Step 7: Cleanup ──────────────────────────────────────────────────────────

if [[ "$KEEP_VM" == "true" ]]; then
    echo "[7/7] Keeping VM alive (--keep-vm). Remember to delete it manually:"
    echo "  gcloud compute instances delete $INSTANCE_NAME --zone=$ZONE --quiet"
    echo "  SSH: gcloud compute ssh $INSTANCE_NAME --zone=$ZONE"
else
    echo "[7/7] Deleting VM to avoid ongoing costs..."
    gcloud compute instances delete "$INSTANCE_NAME" --zone="$ZONE" --quiet
    echo "  VM deleted."
fi

# ── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "============================================================"
echo "  Training Complete"
echo "============================================================"
echo "  Results:  $LOCAL_RESULTS/"
echo "  GPU:      $GPU_TYPE ($ZONE)"
echo "  Instance: $INSTANCE_NAME"
if [[ "$KEEP_VM" == "true" ]]; then
    echo "  VM:       STILL RUNNING (delete manually!)"
fi
echo "============================================================"
