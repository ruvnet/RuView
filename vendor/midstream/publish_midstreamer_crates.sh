#!/bin/bash
set -e

# Read token from .env
export CARGO_REGISTRY_TOKEN=$(grep "^CRATES_API_KEY=" .env | cut -d'=' -f2)

echo "🚀 Publishing Midstreamer Platform crates to crates.io"
echo "======================================================"
echo ""
echo "This will publish 6 core Midstreamer crates in dependency order."
echo "Total estimated time: ~30 minutes"
echo ""

# Phase 1: Foundation crates (no dependencies on unpublished crates)
echo "📦 PHASE 1: Foundation Crates (4 crates, ~20 minutes)"
echo "========================================================"
echo ""

# 1. midstreamer-temporal-compare (no deps)
echo "[1/6] Publishing midstreamer-temporal-compare..."
cd /workspaces/midstream/crates/temporal-compare
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ midstreamer-temporal-compare v0.1.0 published"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# 2. midstreamer-scheduler (no deps)
echo "[2/6] Publishing midstreamer-scheduler..."
cd /workspaces/midstream/crates/nanosecond-scheduler
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ midstreamer-scheduler v0.1.0 published"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# 3. midstreamer-neural-solver (depends on midstreamer-scheduler)
echo "[3/6] Publishing midstreamer-neural-solver..."
cd /workspaces/midstream/crates/temporal-neural-solver
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ midstreamer-neural-solver v0.1.0 published"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# 4. midstreamer-attractor (depends on midstreamer-temporal-compare)
echo "[4/6] Publishing midstreamer-attractor..."
cd /workspaces/midstream/crates/temporal-attractor-studio
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ midstreamer-attractor v0.1.0 published"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# Phase 2: Advanced crates
echo ""
echo "📦 PHASE 2: Advanced Crates (2 crates, ~10 minutes)"
echo "====================================================="
echo ""

# 5. midstreamer-quic (no deps)
echo "[5/6] Publishing midstreamer-quic..."
cd /workspaces/midstream/crates/quic-multistream
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ midstreamer-quic v0.1.0 published"
echo ""
echo "⏳ Waiting 180 seconds for crates.io indexing..."
sleep 180

# 6. midstreamer-strange-loop (depends on all above)
echo "[6/6] Publishing midstreamer-strange-loop..."
cd /workspaces/midstream/crates/strange-loop
cargo publish --token "$CARGO_REGISTRY_TOKEN"
echo "✅ midstreamer-strange-loop v0.1.0 published"
echo ""

echo "🎉 All Midstreamer crates published successfully!"
echo ""
echo "Published crates:"
echo "  1. midstreamer-temporal-compare v0.1.0"
echo "  2. midstreamer-scheduler v0.1.0"
echo "  3. midstreamer-neural-solver v0.1.0"
echo "  4. midstreamer-attractor v0.1.0"
echo "  5. midstreamer-quic v0.1.0"
echo "  6. midstreamer-strange-loop v0.1.0"
echo ""
echo "View at: https://crates.io/search?q=midstreamer"
echo ""
echo "✅ Ready to publish AIMDS crates (aimds-detection, aimds-analysis, aimds-response)"
