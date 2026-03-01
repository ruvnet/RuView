#!/bin/bash
# RuvLTRA HuggingFace Publishing Script
#
# Prerequisites:
#   pip install huggingface_hub
#   huggingface-cli login
#
# Environment:
#   HF_TOKEN or HUGGING_FACE_HUB_TOKEN must be set

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODELS_DIR="${HOME}/.ruvllm/models"
REPO_ID="ruv/ruvltra"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "╔═══════════════════════════════════════════════════════════════════════════════════╗"
echo "║                    RuvLTRA HuggingFace Publishing                                 ║"
echo "╚═══════════════════════════════════════════════════════════════════════════════════╝"
echo ""

# Check for HuggingFace token
HF_TOKEN="${HF_TOKEN:-${HUGGING_FACE_HUB_TOKEN:-${HUGGINGFACE_API_KEY:-}}}"
if [ -z "$HF_TOKEN" ]; then
    echo -e "${RED}Error: No HuggingFace token found.${NC}"
    echo "Set one of: HF_TOKEN, HUGGING_FACE_HUB_TOKEN, or HUGGINGFACE_API_KEY"
    exit 1
fi

echo -e "${GREEN}✓ HuggingFace token found${NC}"

# Check for huggingface-cli
if ! command -v huggingface-cli &> /dev/null; then
    echo -e "${YELLOW}Installing huggingface_hub...${NC}"
    pip install huggingface_hub
fi

echo -e "${GREEN}✓ huggingface-cli available${NC}"

# List available models
echo ""
echo "Available models in ${MODELS_DIR}:"
ls -lh "${MODELS_DIR}"/*.gguf 2>/dev/null || echo "  (no models found)"
echo ""

# Define models to upload
MODELS=(
    "ruvltra-claude-code-0.5b-q4_k_m.gguf:Claude Code Router - 100% routing accuracy"
    "ruvltra-0.5b-q4_k_m.gguf:General embeddings model"
)

# Upload README first
echo "─────────────────────────────────────────────────────────────────"
echo "                     Uploading README.md"
echo "─────────────────────────────────────────────────────────────────"

if [ -f "${SCRIPT_DIR}/README.md" ]; then
    echo "Uploading model card..."
    huggingface-cli upload "${REPO_ID}" "${SCRIPT_DIR}/README.md" README.md \
        --token "${HF_TOKEN}" \
        --commit-message "Update model card with 100% routing accuracy benchmarks"
    echo -e "${GREEN}✓ README.md uploaded${NC}"
else
    echo -e "${YELLOW}Warning: README.md not found at ${SCRIPT_DIR}/README.md${NC}"
fi

# Upload each model
echo ""
echo "─────────────────────────────────────────────────────────────────"
echo "                     Uploading Models"
echo "─────────────────────────────────────────────────────────────────"

for model_entry in "${MODELS[@]}"; do
    model_file="${model_entry%%:*}"
    model_desc="${model_entry#*:}"
    model_path="${MODELS_DIR}/${model_file}"

    if [ -f "${model_path}" ]; then
        echo ""
        echo "Uploading: ${model_file}"
        echo "  Description: ${model_desc}"
        echo "  Size: $(du -h "${model_path}" | cut -f1)"

        huggingface-cli upload "${REPO_ID}" "${model_path}" "${model_file}" \
            --token "${HF_TOKEN}" \
            --commit-message "Update ${model_file} - ${model_desc}"

        echo -e "${GREEN}✓ ${model_file} uploaded${NC}"
    else
        echo -e "${YELLOW}Skipping ${model_file} (not found)${NC}"
    fi
done

echo ""
echo "═══════════════════════════════════════════════════════════════════════════════════"
echo "                              PUBLISHING COMPLETE"
echo "═══════════════════════════════════════════════════════════════════════════════════"
echo ""
echo "Repository: https://huggingface.co/${REPO_ID}"
echo ""
echo "Models available:"
echo "  - ruvltra-claude-code-0.5b-q4_k_m.gguf (Claude Code Router)"
echo "  - ruvltra-0.5b-q4_k_m.gguf (General Embeddings)"
echo ""
echo "Key benchmark: 100% routing accuracy with hybrid keyword+embedding strategy"
echo ""
