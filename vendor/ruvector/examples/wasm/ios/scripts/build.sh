#!/bin/bash
# =============================================================================
# iOS WASM Build Script
# Optimized for minimal binary size and sub-100ms latency
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_DIR/dist"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}iOS WASM Recommendation Engine Builder${NC}"
echo -e "${BLUE}========================================${NC}"

# Check prerequisites
check_prerequisites() {
    echo -e "\n${YELLOW}Checking prerequisites...${NC}"

    if ! command -v rustup &> /dev/null; then
        echo -e "${RED}Error: rustup not found. Install from https://rustup.rs${NC}"
        exit 1
    fi

    if ! rustup target list --installed | grep -q "wasm32-wasip1"; then
        echo -e "${YELLOW}Installing wasm32-wasip1 target...${NC}"
        rustup target add wasm32-wasip1
    fi

    if ! command -v wasm-opt &> /dev/null; then
        echo -e "${YELLOW}Warning: wasm-opt not found. Install binaryen for optimal size reduction.${NC}"
        echo -e "${YELLOW}  brew install binaryen (macOS)${NC}"
        echo -e "${YELLOW}  apt install binaryen (Ubuntu)${NC}"
        WASM_OPT_AVAILABLE=false
    else
        WASM_OPT_AVAILABLE=true
        echo -e "${GREEN}✓ wasm-opt available${NC}"
    fi

    echo -e "${GREEN}✓ All prerequisites met${NC}"
}

# Build the WASM module
build_wasm() {
    echo -e "\n${YELLOW}Building WASM module...${NC}"

    cd "$PROJECT_DIR"

    # Build with maximum optimization
    RUSTFLAGS="-C target-feature=+bulk-memory,+mutable-globals" \
    cargo build --target wasm32-wasip1 --release

    echo -e "${GREEN}✓ Build completed${NC}"
}

# Optimize the WASM binary
optimize_wasm() {
    echo -e "\n${YELLOW}Optimizing WASM binary...${NC}"

    mkdir -p "$OUTPUT_DIR"

    WASM_INPUT="$PROJECT_DIR/target/wasm32-wasip1/release/ruvector_ios_wasm.wasm"
    WASM_OUTPUT="$OUTPUT_DIR/recommendation.wasm"

    if [ ! -f "$WASM_INPUT" ]; then
        echo -e "${RED}Error: WASM file not found at $WASM_INPUT${NC}"
        exit 1
    fi

    if [ "$WASM_OPT_AVAILABLE" = true ]; then
        echo "Running wasm-opt with aggressive size optimization (-Oz)..."

        # Single-pass maximum optimization
        # Enable all required WASM features for wasip1 target
        wasm-opt -Oz \
            --enable-bulk-memory \
            --enable-mutable-globals \
            --enable-nontrapping-float-to-int \
            --enable-sign-ext \
            --strip-debug \
            --strip-dwarf \
            --strip-producers \
            --coalesce-locals \
            --reorder-locals \
            --reorder-functions \
            --remove-unused-names \
            --simplify-locals \
            --vacuum \
            --dce \
            -o "$WASM_OUTPUT" \
            "$WASM_INPUT"

        echo -e "${GREEN}✓ wasm-opt optimization completed${NC}"
    else
        cp "$WASM_INPUT" "$WASM_OUTPUT"
        echo -e "${YELLOW}⚠ Skipped wasm-opt (not installed)${NC}"
    fi
}

# Strip and analyze binary
analyze_binary() {
    echo -e "\n${YELLOW}Binary Analysis:${NC}"

    WASM_OUTPUT="$OUTPUT_DIR/recommendation.wasm"

    if [ -f "$WASM_OUTPUT" ]; then
        SIZE_BYTES=$(wc -c < "$WASM_OUTPUT")
        SIZE_KB=$((SIZE_BYTES / 1024))
        SIZE_MB=$(echo "scale=2; $SIZE_BYTES / 1048576" | bc 2>/dev/null || echo "N/A")

        echo -e "  Output: ${GREEN}$WASM_OUTPUT${NC}"
        echo -e "  Size: ${GREEN}${SIZE_BYTES} bytes (${SIZE_KB} KB / ${SIZE_MB} MB)${NC}"

        # Target check
        if [ "$SIZE_KB" -lt 5120 ]; then
            echo -e "  Target: ${GREEN}✓ Under 5MB target${NC}"
        else
            echo -e "  Target: ${YELLOW}⚠ Exceeds 5MB target${NC}"
        fi

        # List exports if wabt is available
        if command -v wasm-objdump &> /dev/null; then
            echo -e "\n  ${BLUE}Exports:${NC}"
            wasm-objdump -x "$WASM_OUTPUT" 2>/dev/null | grep "func\[" | head -20 || true
        fi
    fi
}

# Copy to Swift project
copy_to_swift() {
    SWIFT_RESOURCES="$PROJECT_DIR/swift/Resources"

    if [ -d "$SWIFT_RESOURCES" ]; then
        echo -e "\n${YELLOW}Copying to Swift resources...${NC}"
        cp "$OUTPUT_DIR/recommendation.wasm" "$SWIFT_RESOURCES/"
        echo -e "${GREEN}✓ Copied to $SWIFT_RESOURCES/recommendation.wasm${NC}"
    fi
}

# Generate TypeScript/JavaScript bindings (optional)
generate_bindings() {
    echo -e "\n${YELLOW}Generating bindings...${NC}"

    cat > "$OUTPUT_DIR/recommendation.d.ts" << 'EOF'
// TypeScript declarations for iOS WASM Recommendation Engine

export interface RecommendationEngine {
    /** Initialize the engine */
    init(dim: number, actions: number): number;

    /** Get memory pointer */
    get_memory_ptr(): number;

    /** Allocate memory */
    alloc(size: number): number;

    /** Reset memory pool */
    reset_memory(): void;

    /** Embed content and return pointer */
    embed_content(
        content_id: bigint,
        content_type: number,
        duration_secs: number,
        category_flags: number,
        popularity: number,
        recency: number
    ): number;

    /** Set vibe state */
    set_vibe(
        energy: number,
        mood: number,
        focus: number,
        time_context: number,
        pref0: number,
        pref1: number,
        pref2: number,
        pref3: number
    ): void;

    /** Get recommendations */
    get_recommendations(
        candidates_ptr: number,
        candidates_len: number,
        top_k: number,
        out_ptr: number
    ): number;

    /** Update learning */
    update_learning(
        content_id: bigint,
        interaction_type: number,
        time_spent: number,
        position: number
    ): void;

    /** Compute similarity */
    compute_similarity(id_a: bigint, id_b: bigint): number;

    /** Save state */
    save_state(): number;

    /** Load state */
    load_state(ptr: number, len: number): number;

    /** Get embedding dimension */
    get_embedding_dim(): number;

    /** Get exploration rate */
    get_exploration_rate(): number;

    /** Get update count */
    get_update_count(): bigint;
}

export function instantiate(wasmModule: WebAssembly.Module): Promise<RecommendationEngine>;
EOF

    echo -e "${GREEN}✓ Generated recommendation.d.ts${NC}"
}

# Main execution
main() {
    check_prerequisites
    build_wasm
    optimize_wasm
    analyze_binary
    copy_to_swift
    generate_bindings

    echo -e "\n${GREEN}========================================${NC}"
    echo -e "${GREEN}Build completed successfully!${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo -e "\nOutput: ${BLUE}$OUTPUT_DIR/recommendation.wasm${NC}"
}

main "$@"
