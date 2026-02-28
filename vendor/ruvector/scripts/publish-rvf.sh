#!/usr/bin/env bash
# Publish all RVF crates to crates.io in dependency order.
# Usage: ./scripts/publish-rvf.sh [--dry-run]
#
# Publishing order (each crate depends on those before it):
#   1. rvf-types    (no internal deps)
#   2. rvf-wire     (depends on rvf-types)
#   3. rvf-manifest (depends on rvf-types)
#   4. rvf-index    (no internal deps currently)
#   5. rvf-quant    (depends on rvf-types)
#   6. rvf-crypto   (depends on rvf-types)
#   7. rvf-runtime  (depends on rvf-types)
#   8. rvf-wasm     (depends on rvf-types)
#   9. rvf-node     (depends on rvf-runtime, rvf-types)
#  10. rvf-server   (depends on rvf-runtime, rvf-types)

set -euo pipefail

DRY_RUN=""
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN="--dry-run"
    echo "=== DRY RUN MODE ==="
fi

CRATES_DIR="$(cd "$(dirname "$0")/../crates/rvf" && pwd)"
DELAY_SECONDS=30

CRATES=(
    rvf-types
    rvf-wire
    rvf-manifest
    rvf-index
    rvf-quant
    rvf-crypto
    rvf-runtime
    rvf-wasm
    rvf-node
    rvf-server
)

for crate in "${CRATES[@]}"; do
    echo ""
    echo "=== Publishing ${crate} ==="
    cargo publish \
        --manifest-path "${CRATES_DIR}/${crate}/Cargo.toml" \
        --allow-dirty \
        ${DRY_RUN}

    if [[ -z "${DRY_RUN}" ]]; then
        echo "Waiting ${DELAY_SECONDS}s for crates.io index to update..."
        sleep "${DELAY_SECONDS}"
    fi
done

echo ""
echo "=== All RVF crates published successfully ==="
