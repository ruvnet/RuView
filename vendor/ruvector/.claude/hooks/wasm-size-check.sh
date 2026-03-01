#!/bin/bash
# WASM size checker for rvlite and other WASM crates
# Ensures bundles stay within target size (<3MB gzipped)

set -e

CRATE="${1:-rvlite}"
MAX_SIZE_KB="${2:-3072}"  # 3MB default

cd /workspaces/ruvector

echo "📏 WASM Size Checker"
echo "==================="
echo ""

check_wasm_size() {
    local crate_dir=$1
    local pkg_dir="$crate_dir/pkg"

    if [ ! -d "$pkg_dir" ]; then
        echo "⚠️  No pkg/ directory found for $crate_dir"
        echo "   Run: cd $crate_dir && wasm-pack build --release"
        return 1
    fi

    echo "📦 Checking: $crate_dir"

    # Find .wasm files
    for wasm_file in "$pkg_dir"/*.wasm; do
        if [ -f "$wasm_file" ]; then
            # Raw size
            RAW_SIZE=$(stat -c%s "$wasm_file" 2>/dev/null || stat -f%z "$wasm_file")
            RAW_SIZE_KB=$((RAW_SIZE / 1024))

            # Gzipped size
            GZIP_SIZE=$(gzip -c "$wasm_file" | wc -c)
            GZIP_SIZE_KB=$((GZIP_SIZE / 1024))

            echo "   📄 $(basename "$wasm_file")"
            echo "      Raw:     ${RAW_SIZE_KB} KB"
            echo "      Gzipped: ${GZIP_SIZE_KB} KB"

            if [ "$GZIP_SIZE_KB" -gt "$MAX_SIZE_KB" ]; then
                echo "      ❌ EXCEEDS target of ${MAX_SIZE_KB} KB!"
                return 1
            else
                PERCENT=$((GZIP_SIZE_KB * 100 / MAX_SIZE_KB))
                echo "      ✅ ${PERCENT}% of budget"
            fi
        fi
    done

    return 0
}

case "$CRATE" in
    "all")
        echo "Checking all WASM crates..."
        echo ""
        for dir in crates/*-wasm crates/rvlite; do
            if [ -d "$dir" ]; then
                check_wasm_size "$dir" || true
                echo ""
            fi
        done
        ;;

    "rvlite")
        check_wasm_size "crates/rvlite"
        ;;

    "ruvector-wasm"|"core")
        check_wasm_size "crates/ruvector-wasm"
        ;;

    "graph"|"ruvector-graph-wasm")
        check_wasm_size "crates/ruvector-graph-wasm"
        ;;

    "gnn"|"ruvector-gnn-wasm")
        check_wasm_size "crates/ruvector-gnn-wasm"
        ;;

    "attention"|"ruvector-attention-wasm")
        check_wasm_size "crates/ruvector-attention-wasm"
        ;;

    "mincut"|"ruvector-mincut-wasm")
        check_wasm_size "crates/ruvector-mincut-wasm"
        ;;

    *)
        if [ -d "crates/$CRATE" ]; then
            check_wasm_size "crates/$CRATE"
        else
            echo "Usage: $0 [all|rvlite|core|graph|gnn|attention|mincut|<crate-name>]"
            exit 1
        fi
        ;;
esac

echo ""
echo "✅ Size check complete"
