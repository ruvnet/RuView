#!/bin/bash
# ==============================================================================
# GCloud GPU driver for the MERIDIAN CSI masked-autoencoder pre-train (ADR-027 §2.0)
# ==============================================================================
#
# Creates a GCloud VM with a GPU, builds wifi-densepose-train with the
# `tch-backend` (+ `cuda`) feature, runs the `pretrain-mae` binary, downloads
# the pre-trained variable store (`.ot`), and tears the VM down.
#
# STATUS: prototype wiring stub (ADR-027 §2.0, iteration 3). The `pretrain-mae`
# binary currently drives the *deterministic SyntheticCsiDataset* — that's the
# end-to-end smoke path. The real heterogeneous-CSI pre-train (MM-Fi + Wi-Pose +
# data/recordings/ + multi-band virtual sub-carriers) needs the ingest pipeline
# tracked in ADR-027 §2.0 "Iteration 3 plan"; the TODO markers below show where
# it plugs in. This script is intentionally a thin, reviewable shell of the real
# gcloud-train.sh (which it mirrors) — it has NOT been run.
#
# Usage:
#   bash scripts/pretrain-mae-gcloud.sh [OPTIONS]
#
# Options:
#   --gpu        l4|a100|h100   GPU type (default: l4)
#   --zone       ZONE           GCloud zone (default: us-central1-a)
#   --hours      N              Max VM lifetime in hours (default: 3)
#   --epochs     N              Pre-train epochs (default: 20)
#   --samples    N              Synthetic samples (until the real ingest lands) (default: 4096)
#   --batch      N              Mini-batch size (default: 64)
#   --mask-ratio R              Token mask ratio (default: 0.75)
#   --lr         R              Adam learning rate (default: 1e-3)
#   --out        FILE           Local path for the downloaded .ot (default: data/models/mae-pretrained.ot)
#   --data-dir   DIR            (future) heterogeneous CSI corpus to upload — see TODO below
#   --dry-run                   Build + run a tiny pre-train locally with synthetic data; no VM
#   --keep-vm                   Do not delete the VM after the run
#   --instance   NAME           Custom VM instance name
#
# Prerequisites (same as gcloud-train.sh):
#   - gcloud CLI authenticated:  gcloud auth login
#   - Project set:               gcloud config set project cognitum-20260110
#   - GPU quota in the chosen zone
#
# Cost (same envelope as gcloud-train.sh):
#   L4 ~$0.80/hr (prototyping) · A100 40GB ~$3.60/hr (full pre-train) · H100 80GB ~$11/hr
# ==============================================================================

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────
PROJECT="cognitum-20260110"
GPU_TYPE="l4"
ZONE="us-central1-a"
HOURS=3
EPOCHS=20
SAMPLES=4096
BATCH=64
MASK_RATIO=0.75
LR="1e-3"
OUT="data/models/mae-pretrained.ot"
DATA_DIR=""
DRY_RUN=0
KEEP_VM=0
INSTANCE="meridian-mae-$(date +%s)"

# ── Arg parse ─────────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --gpu)        GPU_TYPE="$2"; shift 2;;
    --zone)       ZONE="$2"; shift 2;;
    --hours)      HOURS="$2"; shift 2;;
    --epochs)     EPOCHS="$2"; shift 2;;
    --samples)    SAMPLES="$2"; shift 2;;
    --batch)      BATCH="$2"; shift 2;;
    --mask-ratio) MASK_RATIO="$2"; shift 2;;
    --lr)         LR="$2"; shift 2;;
    --out)        OUT="$2"; shift 2;;
    --data-dir)   DATA_DIR="$2"; shift 2;;
    --dry-run)    DRY_RUN=1; shift;;
    --keep-vm)    KEEP_VM=1; shift;;
    --instance)   INSTANCE="$2"; shift 2;;
    -h|--help)    sed -n '2,46p' "$0"; exit 0;;
    *) echo "unknown option: $1" >&2; exit 2;;
  esac
done

case "$GPU_TYPE" in
  l4)   ACCEL="type=nvidia-l4,count=1";        MACHINE="g2-standard-8";;
  a100) ACCEL="type=nvidia-tesla-a100,count=1"; MACHINE="a2-highgpu-1g";;
  h100) ACCEL="type=nvidia-h100-80gb,count=1";  MACHINE="a3-highgpu-1g";;
  *) echo "unknown --gpu: $GPU_TYPE (l4|a100|h100)" >&2; exit 2;;
esac

PRETRAIN_ARGS="--epochs $EPOCHS --samples $SAMPLES --batch $BATCH --mask-ratio $MASK_RATIO --lr $LR --save mae-pretrained.ot"

# ── Dry run: build + tiny pre-train locally (synthetic data), no VM ───────────
if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "[dry-run] cargo run -p wifi-densepose-train --features tch-backend --bin pretrain-mae -- --epochs 2 --samples 64 --batch 8"
  echo "[dry-run] (requires LibTorch — set LIBTORCH or use a tch download-libtorch feature build)"
  cd "$(dirname "$0")/../v2"
  cargo run -p wifi-densepose-train --features tch-backend --bin pretrain-mae -- --epochs 2 --samples 64 --batch 8
  exit 0
fi

# ── Provision VM ──────────────────────────────────────────────────────────────
echo "==> Project: $PROJECT  Zone: $ZONE  GPU: $GPU_TYPE  Machine: $MACHINE  Instance: $INSTANCE"
gcloud config set project "$PROJECT" >/dev/null
gcloud compute instances create "$INSTANCE" \
  --zone="$ZONE" --machine-type="$MACHINE" \
  --accelerator="$ACCEL" --maintenance-policy=TERMINATE \
  --image-family=pytorch-latest-gpu --image-project=deeplearning-platform-release \
  --boot-disk-size=128GB --metadata="install-nvidia-driver=True" \
  --max-run-duration="${HOURS}h" --instance-termination-action=DELETE

cleanup() {
  if [[ "$KEEP_VM" -eq 0 ]]; then
    echo "==> Deleting VM $INSTANCE"
    gcloud compute instances delete "$INSTANCE" --zone="$ZONE" --quiet || true
  else
    echo "==> --keep-vm set; VM $INSTANCE left running (remember to delete it)."
  fi
}
trap cleanup EXIT

run_remote() { gcloud compute ssh "$INSTANCE" --zone="$ZONE" --command="$1"; }

echo "==> Waiting for SSH..."
for _ in $(seq 1 30); do run_remote "true" 2>/dev/null && break; sleep 10; done

echo "==> Provisioning toolchain on the VM"
run_remote 'set -e
  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
  # The pytorch-latest-gpu image ships libtorch; point tch at it.
  TORCH_DIR="$(python -c "import torch,os;print(os.path.dirname(torch.__file__))")"
  echo "export LIBTORCH=$TORCH_DIR" >> "$HOME/.bashrc"
  echo "export LD_LIBRARY_PATH=$TORCH_DIR/lib:\$LD_LIBRARY_PATH" >> "$HOME/.bashrc"
  sudo apt-get update -qq && sudo apt-get install -y -qq git build-essential pkg-config'

echo "==> Uploading repo"
# rsync the repo (excluding build artifacts) — same approach as gcloud-train.sh.
gcloud compute scp --recurse --zone="$ZONE" \
  ../v2 ../scripts ../docs "$INSTANCE":~/ruview/ >/dev/null

# TODO (ADR-027 §2.0, iter 3 ingest): when --data-dir is given, upload the
# heterogeneous CSI corpus and point pretrain-mae at it instead of the synthetic
# dataset (needs a `--data-dir`/`--datasets` flag on the bin first — see the plan).
if [[ -n "$DATA_DIR" ]]; then
  echo "==> Uploading CSI corpus from $DATA_DIR"
  gcloud compute scp --recurse --zone="$ZONE" "$DATA_DIR" "$INSTANCE":~/ruview/csi-corpus/ >/dev/null
  PRETRAIN_ARGS="$PRETRAIN_ARGS # TODO: --data-dir ~/ruview/csi-corpus"
fi

echo "==> Building + running pre-train on the VM"
run_remote "set -e; source \$HOME/.cargo/env; source \$HOME/.bashrc
  cd ~/ruview/v2
  cargo build --release -p wifi-densepose-train --features tch-backend,cuda
  cargo run --release -p wifi-densepose-train --features tch-backend,cuda --bin pretrain-mae -- $PRETRAIN_ARGS"

echo "==> Downloading pre-trained variable store → $OUT"
mkdir -p "$(dirname "$OUT")"
gcloud compute scp --zone="$ZONE" "$INSTANCE":~/ruview/v2/mae-pretrained.ot "$OUT"

echo "==> Done. Pre-trained encoder: $OUT"
echo "    Next: fine-tune the ADR-027 §2.x heads on top of it (see §2.0 'Iteration 3 plan')."
