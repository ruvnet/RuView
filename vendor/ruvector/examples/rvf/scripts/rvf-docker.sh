#!/usr/bin/env bash
# rvf-docker.sh — Run RVF in Docker (any platform)
# Usage: bash scripts/rvf-docker.sh
set -euo pipefail

echo "=== RVF Docker Quick Start ==="

# ── 1. Build the Docker image ───────────────────────────────
echo "[1/4] Building RVF Docker image..."
cat > /tmp/Dockerfile.rvf <<'DOCKERFILE'
FROM rust:1.87-bookworm AS builder
WORKDIR /app
COPY . .
RUN cd crates/rvf && cargo build -p rvf-cli --release
RUN cp target/release/rvf /usr/local/bin/rvf

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/bin/rvf /usr/local/bin/rvf
WORKDIR /data
ENTRYPOINT ["rvf"]
DOCKERFILE

REPO_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
docker build -f /tmp/Dockerfile.rvf -t rvf:latest "$REPO_ROOT"
echo "  Built: rvf:latest"

# ── 2. Create and populate a store ──────────────────────────
echo "[2/4] Creating and populating vector store..."
docker run --rm -v "$(pwd)/output:/data" rvf:latest create demo.rvf --dimension 128
echo "  Created output/demo.rvf"

# ── 3. Ingest sample data ──────────────────────────────────
echo "[3/4] Ingesting sample vectors..."
cat > /tmp/rvf_sample.json <<'JSON'
[
  {"id": 1, "vector": [0.1, 0.2, 0.3]},
  {"id": 2, "vector": [0.4, 0.5, 0.6]},
  {"id": 3, "vector": [0.7, 0.8, 0.9]}
]
JSON
docker run --rm \
  -v "$(pwd)/output:/data" \
  -v "/tmp/rvf_sample.json:/input.json:ro" \
  rvf:latest ingest demo.rvf --input /input.json --format json
echo "  Ingested 3 vectors"

# ── 4. Query and inspect ───────────────────────────────────
echo "[4/4] Querying and inspecting..."
docker run --rm -v "$(pwd)/output:/data" rvf:latest query demo.rvf --vector "0.1,0.2,0.3" --k 2
docker run --rm -v "$(pwd)/output:/data" rvf:latest inspect demo.rvf

echo ""
echo "=== Done ==="
echo "  Docker image: rvf:latest"
echo "  Store: output/demo.rvf"
echo "  Run any rvf command: docker run --rm -v \$(pwd)/output:/data rvf:latest <command>"
