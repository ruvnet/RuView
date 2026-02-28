#!/bin/bash
# Generate flamegraphs for CPU profiling

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
FLAMEGRAPH_DIR="$SCRIPT_DIR/../flamegraphs"

mkdir -p "$FLAMEGRAPH_DIR"

echo "🔥 Generating flamegraphs..."

cd "$PROJECT_ROOT"

# Generate flamegraph for distance metrics benchmark
echo "Flamegraph: distance_metrics..."
sudo cargo flamegraph --bench distance_metrics --output="$FLAMEGRAPH_DIR/distance_metrics.svg" -- --profile-time=30 || echo "Failed to generate distance_metrics flamegraph"

# Generate flamegraph for HNSW search benchmark
echo "Flamegraph: hnsw_search..."
sudo cargo flamegraph --bench hnsw_search --output="$FLAMEGRAPH_DIR/hnsw_search.svg" -- --profile-time=30 || echo "Failed to generate hnsw_search flamegraph"

# Change ownership
sudo chown -R $USER:$USER "$FLAMEGRAPH_DIR" 2>/dev/null || true

echo "✅ Flamegraph generation complete!"
echo "Flamegraphs saved to: $FLAMEGRAPH_DIR"
echo ""
echo "View flamegraphs:"
echo "  firefox $FLAMEGRAPH_DIR/distance_metrics.svg"
echo "  firefox $FLAMEGRAPH_DIR/hnsw_search.svg"
