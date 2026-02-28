#!/bin/bash
# Build NAPI-RS bindings for all platforms
# Usage: ./scripts/build/build-all-platforms.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
NPM_PLATFORMS_DIR="$PROJECT_ROOT/npm/core/platforms"
NPM_NATIVE_DIR="$PROJECT_ROOT/npm/core/native"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=========================================="
echo "  Ruvector NAPI-RS Multi-Platform Build"
echo "=========================================="
echo ""

# Ensure output directories exist
mkdir -p "$NPM_PLATFORMS_DIR"/{linux-x64-gnu,linux-arm64-gnu,darwin-x64,darwin-arm64,win32-x64-msvc}
mkdir -p "$NPM_NATIVE_DIR"/linux-x64

# Function to build for a target
build_target() {
    local target=$1
    local platform_dir=$2
    local binary_name="libruvector_node.so"

    # Adjust binary name for different platforms
    case $target in
        *darwin*)
            binary_name="libruvector_node.dylib"
            ;;
        *windows*|*msvc*)
            binary_name="ruvector_node.dll"
            ;;
    esac

    echo -e "${YELLOW}Building for $target...${NC}"

    if cargo build --release -p ruvector-node --target "$target" 2>&1; then
        local src="$PROJECT_ROOT/target/$target/release/$binary_name"
        local dest="$NPM_PLATFORMS_DIR/$platform_dir/ruvector.node"

        if [ -f "$src" ]; then
            cp "$src" "$dest"
            echo -e "${GREEN}✓ Built and copied to $platform_dir${NC}"
            return 0
        else
            echo -e "${RED}✗ Binary not found at $src${NC}"
            return 1
        fi
    else
        echo -e "${RED}✗ Build failed for $target${NC}"
        return 1
    fi
}

# Track results
declare -A RESULTS

# Build Linux x64 (native)
echo ""
echo "--- Linux x64 GNU ---"
if build_target "x86_64-unknown-linux-gnu" "linux-x64-gnu"; then
    RESULTS["linux-x64-gnu"]="success"
    # Also copy to native directory for direct usage
    cp "$NPM_PLATFORMS_DIR/linux-x64-gnu/ruvector.node" "$NPM_NATIVE_DIR/linux-x64/ruvector.node"
else
    RESULTS["linux-x64-gnu"]="failed"
fi

# Build Linux ARM64
echo ""
echo "--- Linux ARM64 GNU ---"
if build_target "aarch64-unknown-linux-gnu" "linux-arm64-gnu"; then
    RESULTS["linux-arm64-gnu"]="success"
else
    RESULTS["linux-arm64-gnu"]="failed"
fi

# Build macOS x64 (cross-compile - may fail without proper toolchain)
echo ""
echo "--- macOS x64 (cross-compile) ---"
if build_target "x86_64-apple-darwin" "darwin-x64"; then
    RESULTS["darwin-x64"]="success"
else
    RESULTS["darwin-x64"]="skipped"
    echo -e "${YELLOW}Note: macOS builds require osxcross or native macOS. Use CI for production builds.${NC}"
fi

# Build macOS ARM64 (cross-compile - may fail without proper toolchain)
echo ""
echo "--- macOS ARM64 (cross-compile) ---"
if build_target "aarch64-apple-darwin" "darwin-arm64"; then
    RESULTS["darwin-arm64"]="success"
else
    RESULTS["darwin-arm64"]="skipped"
    echo -e "${YELLOW}Note: macOS builds require osxcross or native macOS. Use CI for production builds.${NC}"
fi

# Build Windows x64 (cross-compile - may fail without proper toolchain)
echo ""
echo "--- Windows x64 MSVC (cross-compile) ---"
if build_target "x86_64-pc-windows-msvc" "win32-x64-msvc"; then
    RESULTS["win32-x64-msvc"]="success"
else
    RESULTS["win32-x64-msvc"]="skipped"
    echo -e "${YELLOW}Note: Windows MSVC builds require proper toolchain. Use CI for production builds.${NC}"
fi

# Summary
echo ""
echo "=========================================="
echo "              Build Summary"
echo "=========================================="
for platform in "${!RESULTS[@]}"; do
    status="${RESULTS[$platform]}"
    case $status in
        success)
            echo -e "${GREEN}✓${NC} $platform: $status"
            ;;
        failed)
            echo -e "${RED}✗${NC} $platform: $status"
            ;;
        skipped)
            echo -e "${YELLOW}○${NC} $platform: $status (requires native toolchain)"
            ;;
    esac
done

echo ""
echo "Binaries located in: $NPM_PLATFORMS_DIR"
echo ""

# Show file sizes
echo "Binary sizes:"
find "$NPM_PLATFORMS_DIR" -name "*.node" -exec ls -lh {} \; 2>/dev/null || true
