#!/bin/bash
set -e

# Read token from .env
export CARGO_REGISTRY_TOKEN=$(grep "^CRATES_API_KEY=" .env | cut -d'=' -f2)

echo "🚀 Publishing Midstream Platform crates to crates.io"
echo "======================================================"
echo ""
echo "This will publish 6 core Midstream crates in dependency order."
echo "Total estimated time: ~35 minutes"
echo ""

# Phase 1: Foundation crates (no dependencies on unpublished crates)
echo "📦 PHASE 1: Foundation Crates (4 crates, ~20 minutes)"
echo "========================================================"
echo ""

# 1. temporal-compare (no deps)
echo "[1/6] Publishing temporal-compare..."
cd /workspaces/midstream/crates/temporal-compare
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ temporal-compare v0.1.0 published"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# 2. nanosecond-scheduler (no deps)
echo "[2/6] Publishing nanosecond-scheduler..."
cd /workspaces/midstream/crates/nanosecond-scheduler
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ nanosecond-scheduler v0.1.0 published"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# 3. temporal-neural-solver (depends on nanosecond-scheduler)
echo "[3/6] Publishing temporal-neural-solver..."
cd /workspaces/midstream/crates/temporal-neural-solver
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ temporal-neural-solver v0.1.0 published"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# 4. temporal-attractor-studio (depends on temporal-compare)
echo "[4/6] Publishing temporal-attractor-studio..."
cd /workspaces/midstream/crates/temporal-attractor-studio
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ temporal-attractor-studio v0.1.0 published"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# Phase 2: Advanced crates
echo ""
echo "📦 PHASE 2: Advanced Crates (2 crates, ~10 minutes)"
echo "====================================================="
echo ""

# 5. quic-multistream (no deps)
echo "[5/6] Publishing quic-multistream..."
cd /workspaces/midstream/crates/quic-multistream
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ quic-multistream v0.1.0 published"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# 6. strange-loop (depends on all above)
echo "[6/6] Publishing strange-loop..."
cd /workspaces/midstream/crates/strange-loop
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ strange-loop v0.1.0 published"
echo ""

echo "🎉 All Midstream crates published successfully!"
echo ""
echo "Published crates:"
echo "  1. temporal-compare v0.1.0"
echo "  2. nanosecond-scheduler v0.1.0"
echo "  3. temporal-neural-solver v0.1.0"
echo "  4. temporal-attractor-studio v0.1.0"
echo "  5. quic-multistream v0.1.0"
echo "  6. strange-loop v0.1.0"
echo ""
echo "View at: https://crates.io/search?q=temporal OR https://crates.io/search?q=midstream"
echo ""
echo "✅ Ready to publish AIMDS crates (aimds-detection, aimds-analysis, aimds-response)"
