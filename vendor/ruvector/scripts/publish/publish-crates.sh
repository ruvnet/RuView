#!/bin/bash
set -e

# Ruvector Crates Publishing Script
# This script publishes all Ruvector crates to crates.io in the correct dependency order
#
# Prerequisites:
# - Rust and Cargo installed
# - CRATES_API_KEY set in .env file
# - All crates build successfully
# - All tests pass

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Load environment variables from .env
if [ -f .env ]; then
    export $(grep -v '^#' .env | grep CRATES_API_KEY | xargs)
else
    echo -e "${RED}Error: .env file not found${NC}"
    exit 1
fi

# Check if CRATES_API_KEY is set
if [ -z "$CRATES_API_KEY" ]; then
    echo -e "${RED}Error: CRATES_API_KEY not found in .env${NC}"
    exit 1
fi

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}   Ruvector Crates Publishing Script${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Configure cargo authentication
echo -e "${YELLOW}Configuring cargo authentication...${NC}"
cargo login "$CRATES_API_KEY"
echo -e "${GREEN}✓ Authentication configured${NC}"
echo ""

# Function to publish a crate
publish_crate() {
    local crate_path=$1
    local crate_name=$(basename "$crate_path")

    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}Publishing: ${crate_name}${NC}"
    echo -e "${BLUE}========================================${NC}"

    cd "$crate_path"

    # Verify the package
    echo -e "${YELLOW}Verifying package...${NC}"
    if cargo package --allow-dirty; then
        echo -e "${GREEN}✓ Package verification successful${NC}"
    else
        echo -e "${RED}✗ Package verification failed${NC}"
        cd - > /dev/null
        return 1
    fi

    # Publish the package
    echo -e "${YELLOW}Publishing to crates.io...${NC}"
    if cargo publish --allow-dirty; then
        echo -e "${GREEN}✓ ${crate_name} published successfully${NC}"
    else
        echo -e "${RED}✗ Failed to publish ${crate_name}${NC}"
        cd - > /dev/null
        return 1
    fi

    cd - > /dev/null

    # Wait a bit for crates.io to index the crate
    echo -e "${YELLOW}Waiting 30 seconds for crates.io to index...${NC}"
    sleep 30

    echo ""
}

# Function to check if crate is already published
check_published() {
    local crate_name=$1
    local version=$2

    if cargo search "$crate_name" --limit 1 | grep -q "^$crate_name = \"$version\""; then
        return 0  # Already published
    else
        return 1  # Not published
    fi
}

# Get version from workspace
VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo -e "${BLUE}Publishing version: ${VERSION}${NC}"
echo ""

# Publishing order (dependencies first)
CRATES=(
    # Base dependencies (no internal dependencies)
    "crates/ruvector-core"
    "crates/router-core"

    # Depends on ruvector-core
    "crates/ruvector-node"
    "crates/ruvector-wasm"
    "crates/ruvector-cli"
    "crates/ruvector-bench"

    # Depends on router-core
    "crates/router-cli"
    "crates/router-ffi"
    "crates/router-wasm"
)

# Track success/failure
SUCCESS_COUNT=0
FAILED_CRATES=()

# Publish each crate
for crate in "${CRATES[@]}"; do
    if [ ! -d "$crate" ]; then
        echo -e "${YELLOW}Warning: $crate directory not found, skipping${NC}"
        continue
    fi

    crate_name=$(basename "$crate")

    # Check if already published
    if check_published "$crate_name" "$VERSION"; then
        echo -e "${YELLOW}$crate_name v$VERSION already published, skipping${NC}"
        ((SUCCESS_COUNT++))
        echo ""
        continue
    fi

    if publish_crate "$crate"; then
        ((SUCCESS_COUNT++))
    else
        FAILED_CRATES+=("$crate_name")
    fi
done

# Summary
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}   Publishing Summary${NC}"
echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}Successfully published: ${SUCCESS_COUNT}/${#CRATES[@]}${NC}"

if [ ${#FAILED_CRATES[@]} -gt 0 ]; then
    echo -e "${RED}Failed to publish:${NC}"
    for crate in "${FAILED_CRATES[@]}"; do
        echo -e "${RED}  - $crate${NC}"
    done
    exit 1
else
    echo -e "${GREEN}All crates published successfully! 🎉${NC}"
fi

echo ""
echo -e "${BLUE}View your crates at: https://crates.io/users/ruvector${NC}"
