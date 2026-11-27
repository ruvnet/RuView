#!/bin/bash
# Publish WiFi-DensePose pre-trained models to HuggingFace Hub
#
# Retrieves the HuggingFace API token from Google Cloud Secrets,
# then uploads model files from dist/models/ to a HuggingFace repo.
#
# Prerequisites:
#   - gcloud CLI authenticated with access to cognitum-20260110
#   - Python 3.8+ with pip
#   - Model files present in dist/models/
#
# Usage:
#   bash scripts/publish-huggingface.sh
#   bash scripts/publish-huggingface.sh --repo ruvnet/wifi-densepose-pretrained --version v0.5.4
#   bash scripts/publish-huggingface.sh --dry-run

set -euo pipefail

# ---------- defaults ----------
REPO="ruvnet/wifi-densepose-pretrained"
VERSION=""
GCLOUD_PROJECT="cognitum-20260110"
SECRET_NAME="HUGGINGFACE_API_KEY"
MODEL_DIR="dist/models"
DRY_RUN=false

# ---------- parse args ----------
while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)       REPO="$2";       shift 2 ;;
    --version)    VERSION="$2";    shift 2 ;;
    --model-dir)  MODEL_DIR="$2";  shift 2 ;;
    --project)    GCLOUD_PROJECT="$2"; shift 2 ;;
    --secret)     SECRET_NAME="$2"; shift 2 ;;
    --dry-run)    DRY_RUN=true;    shift ;;
    -h|--help)
      echo "Usage: bash scripts/publish-huggingface.sh [OPTIONS]"
      echo ""
      echo "Options:"
      echo "  --repo REPO        HuggingFace repo (default: ruvnet/wifi-densepose-pretrained)"
      echo "  --version VERSION  Version tag (default: auto from git describe)"
      echo "  --model-dir DIR    Model directory (default: dist/models)"
      echo "  --project PROJECT  GCloud project (default: cognitum-20260110)"
      echo "  --secret SECRET    GCloud secret name (default: HUGGINGFACE_API_KEY)"
      echo "  --dry-run          Show what would be uploaded without uploading"
      echo "  -h, --help         Show this help"
      exit 0
      ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

# ---------- auto-detect version ----------
if [ -z "$VERSION" ]; then
  VERSION=$(git describe --tags --always 2>/dev/null || echo "dev")
  echo "Auto-detected version: ${VERSION}"
fi

# ---------- validate model files ----------
EXPECTED_FILES=(
  "config.json"
  "model.safetensors"
  "model-q2.bin"
  "model-q4.bin"
  "model-q8.bin"
  "model.rvf.jsonl"
  "csi-embed-v2.safetensors"
  "csi-embed-v2-int4.bin"
  "csi-embed-v2.py"
  "csi-embed-v2-metrics.json"
  "presence-head.json"
  "node-1.json"
  "node-2.json"
  "training-metrics.json"
  "README.md"
)

echo "=== WiFi-DensePose HuggingFace Publisher ==="
echo "Repo:      ${REPO}"
echo "Version:   ${VERSION}"
echo "Model dir: ${MODEL_DIR}"
echo ""

MISSING=0
for f in "${EXPECTED_FILES[@]}"; do
  if [ -f "${MODEL_DIR}/${f}" ]; then
    SIZE=$(stat --printf="%s" "${MODEL_DIR}/${f}" 2>/dev/null || stat -f "%z" "${MODEL_DIR}/${f}" 2>/dev/null || echo "?")
    echo "  [OK] ${f} (${SIZE} bytes)"
  else
    echo "  [MISSING] ${f}"
    MISSING=$((MISSING + 1))
  fi
done

if [ "$MISSING" -gt 0 ]; then
  echo ""
  echo "WARNING: ${MISSING} expected file(s) missing from ${MODEL_DIR}/"
  echo "The upload will proceed with available files only."
  echo ""
fi

# Count actual files to upload
FILE_COUNT=$(find "${MODEL_DIR}" -maxdepth 1 -type f | wc -l)
if [ "$FILE_COUNT" -eq 0 ]; then
  echo "ERROR: No files found in ${MODEL_DIR}/. Nothing to upload."
  exit 1
fi

# ---------- dry run ----------
if [ "$DRY_RUN" = true ]; then
  echo ""
  echo "[DRY RUN] Would upload ${FILE_COUNT} files to https://huggingface.co/${REPO}"
  echo "[DRY RUN] Files:"
  find "${MODEL_DIR}" -maxdepth 1 -type f -exec basename {} \; | sort | while read -r fname; do
    echo "  - ${fname}"
  done
  echo "[DRY RUN] Version tag: ${VERSION}"
  echo ""
  echo "Run without --dry-run to actually upload."
  exit 0
fi

# ---------- retrieve HuggingFace token ----------
echo ""
echo "Retrieving HuggingFace token from GCloud Secrets..."
HF_TOKEN=$(gcloud secrets versions access latest \
  --secret="${SECRET_NAME}" \
  --project="${GCLOUD_PROJECT}" 2>/dev/null)

if [ -z "$HF_TOKEN" ]; then
  echo "ERROR: Failed to retrieve secret '${SECRET_NAME}' from project '${GCLOUD_PROJECT}'."
  echo "Make sure you are authenticated: gcloud auth login"
  echo "And have access to the secret: gcloud secrets list --project=${GCLOUD_PROJECT}"
  exit 1
fi
echo "Token retrieved successfully."

# ---------- install huggingface_hub if needed ----------
if ! python3 -c "import huggingface_hub" 2>/dev/null; then
  echo "Installing huggingface_hub..."
  pip3 install --quiet huggingface_hub
fi

# ---------- upload via Python ----------
echo ""
echo "Uploading to https://huggingface.co/${REPO} ..."

python3 - <<PYEOF
import os
from huggingface_hub import HfApi, login

token = os.environ.get("HF_TOKEN_OVERRIDE") or """${HF_TOKEN}"""
repo_id = "${REPO}"
model_dir = "${MODEL_DIR}"
version = "${VERSION}"

login(token=token, add_to_git_credential=False)
api = HfApi()

# Create repo if it doesn't exist
api.create_repo(
    repo_id=repo_id,
    repo_type="model",
    exist_ok=True,
    private=False,
)

# Upload the entire folder
commit_info = api.upload_folder(
    folder_path=model_dir,
    repo_id=repo_id,
    repo_type="model",
    commit_message=f"Upload WiFi-DensePose pretrained models ({version})",
)

# Create a tag for this version
try:
    api.create_tag(
        repo_id=repo_id,
        repo_type="model",
        tag=version,
        tag_message=f"WiFi-DensePose pretrained models {version}",
    )
    print(f"Tagged as: {version}")
except Exception as e:
    print(f"Tag '{version}' may already exist: {e}")

print()
print("=" * 60)
print(f"Published successfully!")
print(f"URL: https://huggingface.co/{repo_id}")
print(f"Version: {version}")
print(f"Commit: {commit_info.commit_url}")
print("=" * 60)
PYEOF

echo ""
echo "Done."
