#!/bin/bash
set -e

# Extract CRATES_API_KEY from .env
export CARGO_REGISTRY_TOKEN=$(grep "^CRATES_API_KEY=" .env | cut -d'=' -f2)

if [ -z "$CARGO_REGISTRY_TOKEN" ]; then
    echo "Error: CRATES_API_KEY not found in .env"
    exit 1
fi

echo "Token found: ${CARGO_REGISTRY_TOKEN:0:10}..."
echo ""
echo "Publishing AIMDS crates to crates.io..."
echo ""

# 1. aimds-core
echo "=== Publishing aimds-core v0.1.0 ==="
cd /workspaces/midstream/AIMDS/crates/aimds-core
cargo publish --token $CARGO_REGISTRY_TOKEN
echo "✅ aimds-core published successfully"
echo "Waiting 3 minutes for crates.io indexing..."
sleep 180

# 2. aimds-detection
echo ""
echo "=== Publishing aimds-detection v0.1.0 ==="
cd /workspaces/midstream/AIMDS/crates/aimds-detection
cargo publish --token $CARGO_REGISTRY_TOKEN
echo "✅ aimds-detection published successfully"
echo "Waiting 3 minutes for crates.io indexing..."
sleep 180

# 3. aimds-analysis
echo ""
echo "=== Publishing aimds-analysis v0.1.0 ==="
cd /workspaces/midstream/AIMDS/crates/aimds-analysis
cargo publish --token $CARGO_REGISTRY_TOKEN
echo "✅ aimds-analysis published successfully"
echo "Waiting 3 minutes for crates.io indexing..."
sleep 180

# 4. aimds-response
echo ""
echo "=== Publishing aimds-response v0.1.0 ==="
cd /workspaces/midstream/AIMDS/crates/aimds-response
cargo publish --token $CARGO_REGISTRY_TOKEN
echo "✅ aimds-response published successfully"

echo ""
echo "🎉 All AIMDS crates published successfully!"
echo ""
echo "View published crates at:"
echo "- https://crates.io/crates/aimds-core"
echo "- https://crates.io/crates/aimds-detection"
echo "- https://crates.io/crates/aimds-analysis"
echo "- https://crates.io/crates/aimds-response"
