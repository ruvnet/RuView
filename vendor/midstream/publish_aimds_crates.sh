#!/bin/bash
set -e

# Read token from .env
export CARGO_REGISTRY_TOKEN=$(grep "^CRATES_API_KEY=" .env | cut -d'=' -f2)

echo "🚀 Publishing AIMDS crates to crates.io"
echo "========================================"
echo ""

# 1. Publish aimds-core (base dependency)
echo "📦 [1/4] Publishing aimds-core..."
cd /workspaces/midstream/AIMDS/crates/aimds-core
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ aimds-core published successfully"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# 2. Publish aimds-detection (depends on aimds-core)
echo "📦 [2/4] Publishing aimds-detection..."
cd /workspaces/midstream/AIMDS/crates/aimds-detection
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ aimds-detection published successfully"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# 3. Publish aimds-analysis (depends on aimds-core and aimds-detection)
echo "📦 [3/4] Publishing aimds-analysis..."
cd /workspaces/midstream/AIMDS/crates/aimds-analysis
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ aimds-analysis published successfully"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# 4. Publish aimds-response (depends on aimds-core, aimds-detection, aimds-analysis)
echo "📦 [4/4] Publishing aimds-response..."
cd /workspaces/midstream/AIMDS/crates/aimds-response
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ aimds-response published successfully"
echo ""

echo "🎉 All AIMDS crates published successfully!"
echo ""
echo "Published crates:"
echo "  1. aimds-core v0.1.0"
echo "  2. aimds-detection v0.1.0"
echo "  3. aimds-analysis v0.1.0"
echo "  4. aimds-response v0.1.0"
echo ""
echo "View at: https://crates.io/search?q=aimds"
