#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Downloading RuVector Mathpix ONNX Models${NC}"
echo ""

# Configuration
MODELS_DIR="models"
GITHUB_REPO="ruvnet/ruvector"
RELEASE_TAG="scipix-models-v1.0.0"

# Model configurations
declare -A MODELS=(
    ["scipix_encoder.onnx"]="https://github.com/${GITHUB_REPO}/releases/download/${RELEASE_TAG}/scipix_encoder.onnx"
    ["scipix_decoder.onnx"]="https://github.com/${GITHUB_REPO}/releases/download/${RELEASE_TAG}/scipix_decoder.onnx"
    ["scipix_tokenizer.onnx"]="https://github.com/${GITHUB_REPO}/releases/download/${RELEASE_TAG}/scipix_tokenizer.onnx"
)

# SHA256 checksums (these should be updated with actual checksums)
declare -A CHECKSUMS=(
    ["scipix_encoder.onnx"]="SHA256_PLACEHOLDER"
    ["scipix_decoder.onnx"]="SHA256_PLACEHOLDER"
    ["scipix_tokenizer.onnx"]="SHA256_PLACEHOLDER"
)

# Create models directory
mkdir -p "${MODELS_DIR}"

# Function to download a file with progress
download_file() {
    local url=$1
    local output=$2

    if command -v curl &> /dev/null; then
        curl -L --progress-bar -o "${output}" "${url}"
    elif command -v wget &> /dev/null; then
        wget --show-progress -O "${output}" "${url}"
    else
        echo -e "${RED}Error: Neither curl nor wget is available. Please install one.${NC}"
        exit 1
    fi
}

# Function to verify checksum
verify_checksum() {
    local file=$1
    local expected=$2

    if [ "${expected}" = "SHA256_PLACEHOLDER" ]; then
        echo -e "${YELLOW}Warning: No checksum available for ${file}. Skipping verification.${NC}"
        return 0
    fi

    if command -v sha256sum &> /dev/null; then
        local actual=$(sha256sum "${file}" | cut -d' ' -f1)
    elif command -v shasum &> /dev/null; then
        local actual=$(shasum -a 256 "${file}" | cut -d' ' -f1)
    else
        echo -e "${YELLOW}Warning: No SHA256 tool available. Skipping verification.${NC}"
        return 0
    fi

    if [ "${actual}" = "${expected}" ]; then
        echo -e "${GREEN}Checksum verified for ${file}${NC}"
        return 0
    else
        echo -e "${RED}Checksum mismatch for ${file}!${NC}"
        echo -e "${RED}Expected: ${expected}${NC}"
        echo -e "${RED}Got: ${actual}${NC}"
        return 1
    fi
}

# Download each model
for model in "${!MODELS[@]}"; do
    output_path="${MODELS_DIR}/${model}"

    # Check if model already exists
    if [ -f "${output_path}" ]; then
        echo -e "${YELLOW}${model} already exists. Verifying...${NC}"
        if verify_checksum "${output_path}" "${CHECKSUMS[$model]}"; then
            echo -e "${GREEN}${model} is valid. Skipping download.${NC}"
            continue
        else
            echo -e "${YELLOW}${model} verification failed. Re-downloading...${NC}"
            rm -f "${output_path}"
        fi
    fi

    echo -e "${BLUE}Downloading ${model}...${NC}"

    # Try to download from GitHub releases
    if download_file "${MODELS[$model]}" "${output_path}"; then
        echo -e "${GREEN}Downloaded ${model}${NC}"

        # Verify checksum
        if ! verify_checksum "${output_path}" "${CHECKSUMS[$model]}"; then
            echo -e "${RED}Failed to verify ${model}. Removing file.${NC}"
            rm -f "${output_path}"
            exit 1
        fi
    else
        echo -e "${YELLOW}Failed to download from releases. Trying alternative sources...${NC}"

        # Alternative: Download from Hugging Face (if available)
        HF_URL="https://huggingface.co/ruvnet/scipix-models/resolve/main/${model}"
        if download_file "${HF_URL}" "${output_path}"; then
            echo -e "${GREEN}Downloaded ${model} from Hugging Face${NC}"
            verify_checksum "${output_path}" "${CHECKSUMS[$model]}" || true
        else
            echo -e "${RED}Failed to download ${model} from all sources${NC}"

            # Create a placeholder file with instructions
            cat > "${output_path}.README" << EOF
Model: ${model}

This model file could not be downloaded automatically.

Please download it manually from one of these sources:
1. GitHub Releases: ${MODELS[$model]}
2. Hugging Face: https://huggingface.co/ruvnet/scipix-models

After downloading, place the file at:
${output_path}

Expected SHA256 checksum: ${CHECKSUMS[$model]}
EOF
            echo -e "${YELLOW}Created instructions at ${output_path}.README${NC}"
        fi
    fi
done

# Create model configuration file
echo -e "${BLUE}Creating model configuration...${NC}"
cat > "${MODELS_DIR}/config.json" << EOF
{
  "models": {
    "encoder": {
      "path": "scipix_encoder.onnx",
      "type": "image_encoder",
      "input_shape": [1, 3, 224, 224],
      "output_dim": 768
    },
    "decoder": {
      "path": "scipix_decoder.onnx",
      "type": "sequence_decoder",
      "vocab_size": 50000,
      "max_length": 512
    },
    "tokenizer": {
      "path": "scipix_tokenizer.onnx",
      "type": "tokenizer",
      "vocab_size": 50000
    }
  },
  "version": "1.0.0",
  "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF

# Verify all models are present
echo ""
echo -e "${BLUE}Verifying model files...${NC}"
missing_models=0
for model in "${!MODELS[@]}"; do
    if [ -f "${MODELS_DIR}/${model}" ]; then
        size=$(du -h "${MODELS_DIR}/${model}" | cut -f1)
        echo -e "${GREEN}✓ ${model} (${size})${NC}"
    else
        echo -e "${RED}✗ ${model} (missing)${NC}"
        ((missing_models++))
    fi
done

echo ""
if [ ${missing_models} -eq 0 ]; then
    echo -e "${GREEN}====================================${NC}"
    echo -e "${GREEN}All models downloaded successfully!${NC}"
    echo -e "${GREEN}====================================${NC}"
    echo ""
    echo -e "${BLUE}Models are located in: ${MODELS_DIR}/${NC}"
    echo -e "${BLUE}Configuration file: ${MODELS_DIR}/config.json${NC}"
    exit 0
else
    echo -e "${YELLOW}====================================${NC}"
    echo -e "${YELLOW}Warning: ${missing_models} model(s) missing${NC}"
    echo -e "${YELLOW}====================================${NC}"
    echo ""
    echo -e "${YELLOW}Please check the .README files in ${MODELS_DIR}/ for manual download instructions.${NC}"
    exit 1
fi
