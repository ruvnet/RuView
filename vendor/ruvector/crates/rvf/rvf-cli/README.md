# rvf — RuVector Format CLI

Standalone command-line tool for creating, inspecting, querying, and managing RVF vector stores. Runs on Windows, macOS, and Linux with zero runtime dependencies.

## Install

### Pre-built binaries (recommended)

Download from [GitHub Releases](https://github.com/ruvnet/ruvector/releases):

```bash
# macOS (Apple Silicon)
curl -L -o rvf https://github.com/ruvnet/ruvector/releases/latest/download/rvf-darwin-arm64
chmod +x rvf && sudo mv rvf /usr/local/bin/

# macOS (Intel)
curl -L -o rvf https://github.com/ruvnet/ruvector/releases/latest/download/rvf-darwin-x64
chmod +x rvf && sudo mv rvf /usr/local/bin/

# Linux x64
curl -L -o rvf https://github.com/ruvnet/ruvector/releases/latest/download/rvf-linux-x64
chmod +x rvf && sudo mv rvf /usr/local/bin/

# Linux ARM64
curl -L -o rvf https://github.com/ruvnet/ruvector/releases/latest/download/rvf-linux-arm64
chmod +x rvf && sudo mv rvf /usr/local/bin/
```

**Windows (PowerShell):**
```powershell
Invoke-WebRequest -Uri https://github.com/ruvnet/ruvector/releases/latest/download/rvf-windows-x64.exe -OutFile rvf.exe
```

### Build from source

Requires [Rust](https://rustup.rs):

```bash
cargo install --git https://github.com/ruvnet/ruvector.git rvf-cli
```

Or clone and build:

```bash
git clone https://github.com/ruvnet/ruvector.git
cd ruvector
cargo build -p rvf-cli --release
# Binary: target/release/rvf (or rvf.exe on Windows)
```

## Quick start

```bash
# Create a 128-dimensional vector store with cosine distance
rvf create mydb.rvf --dimension 128 --metric cosine

# Ingest vectors from JSON
rvf ingest mydb.rvf --input vectors.json

# Search for nearest neighbors
rvf query mydb.rvf --vector "0.1,0.2,0.3,..." --k 10

# Check store status
rvf status mydb.rvf
```

## Running the examples

The repo includes 48 pre-built `.rvf` example stores in `examples/rvf/output/`. Use the CLI to inspect, query, and manipulate them:

```bash
# List all example stores
ls examples/rvf/output/*.rvf

# Inspect a store
rvf status examples/rvf/output/basic_store.rvf
rvf inspect examples/rvf/output/basic_store.rvf

# Query the semantic search example (500 vectors, 384 dimensions)
rvf status examples/rvf/output/semantic_search.rvf
rvf inspect examples/rvf/output/semantic_search.rvf --json

# Inspect the RAG pipeline store
rvf status examples/rvf/output/rag_pipeline.rvf

# Look at COW lineage (parent -> child)
rvf inspect examples/rvf/output/lineage_parent.rvf
rvf inspect examples/rvf/output/lineage_child.rvf

# Check financial signals store
rvf status examples/rvf/output/financial_signals.rvf

# View the compacted store
rvf status examples/rvf/output/compacted.rvf

# Inspect agent memory store
rvf inspect examples/rvf/output/agent_memory.rvf

# View all stores at once (JSON)
for f in examples/rvf/output/*.rvf; do
  echo "--- $(basename $f) ---"
  rvf status "$f" 2>/dev/null
done
```

### Available example stores

| Store | Vectors | Dim | Description |
|-------|---------|-----|-------------|
| `basic_store.rvf` | 100 | 384 | Basic vector store |
| `semantic_search.rvf` | 500 | 384 | Semantic search embeddings |
| `rag_pipeline.rvf` | 300 | 256 | RAG pipeline embeddings |
| `embedding_cache.rvf` | 500 | 384 | Embedding cache |
| `filtered_search.rvf` | 200 | 256 | Filtered search with metadata |
| `financial_signals.rvf` | 100 | 512 | Financial signal vectors |
| `recommendation.rvf` | 100 | 256 | Recommendation engine |
| `medical_imaging.rvf` | 100 | 768 | Medical imaging features |
| `multimodal_fusion.rvf` | 100 | 2048 | Multimodal fusion vectors |
| `legal_discovery.rvf` | 100 | 768 | Legal document embeddings |
| `progressive_index.rvf` | 1000 | 384 | Progressive HNSW index |
| `quantization.rvf` | 1000 | 384 | Quantized vectors |
| `swarm_knowledge.rvf` | 100 | 128 | Swarm intelligence KB |
| `agent_memory.rvf` | 50 | 128 | Agent conversation memory |
| `experience_replay.rvf` | 50 | 64 | RL experience replay buffer |
| `lineage_parent.rvf` | — | — | COW parent (lineage demo) |
| `lineage_child.rvf` | — | — | COW child (lineage demo) |
| `compacted.rvf` | — | — | Post-compaction store |

## Commands

### create

Create a new empty RVF store.

```bash
rvf create store.rvf --dimension 128 --metric cosine
rvf create store.rvf -d 384 -m l2 --profile 1 --json
```

Options:
- `-d, --dimension` — Vector dimensionality (required)
- `-m, --metric` — Distance metric: `l2`, `ip` (inner product), `cosine` (default: `l2`)
- `-p, --profile` — Hardware profile 0-3 (default: `0`)
- `--json` — Output as JSON

### ingest

Import vectors from a JSON file.

```bash
rvf ingest store.rvf --input data.json
rvf ingest store.rvf -i data.json --batch-size 500 --json
```

Input JSON format:
```json
[
  {"id": 1, "vector": [0.1, 0.2, 0.3, ...]},
  {"id": 2, "vector": [0.4, 0.5, 0.6, ...]}
]
```

### query

Search for k nearest neighbors.

```bash
rvf query store.rvf --vector "1.0,0.0,0.5,0.3" --k 10
rvf query store.rvf -v "0.5,0.5,0.0,0.0" -k 5 --json
```

With filters:
```bash
rvf query store.rvf -v "1.0,0.0" -k 10 \
  --filter '{"eq":{"field":0,"value":{"string":"category_a"}}}'
```

### delete

Delete vectors by ID or filter.

```bash
rvf delete store.rvf --ids 1,2,3
rvf delete store.rvf --filter '{"gt":{"field":0,"value":{"u64":100}}}'
```

### status

Show store status.

```bash
rvf status store.rvf
rvf status store.rvf --json
```

### inspect

Inspect store segments and lineage.

```bash
rvf inspect store.rvf
rvf inspect store.rvf --json
```

### compact

Reclaim dead space from deleted vectors.

```bash
rvf compact store.rvf
rvf compact store.rvf --strip-unknown --json
```

### derive

Create a derived child store (COW branching).

```bash
rvf derive parent.rvf child.rvf --derivation-type clone
rvf derive parent.rvf child.rvf -t snapshot --json
```

Derivation types: `clone`, `filter`, `merge`, `quantize`, `reindex`, `transform`, `snapshot`

### freeze

Snapshot-freeze the current state.

```bash
rvf freeze store.rvf
```

### verify-witness

Verify the tamper-evident witness chain.

```bash
rvf verify-witness store.rvf
```

### verify-attestation

Verify kernel binding and attestation.

```bash
rvf verify-attestation store.rvf
```

### serve

Start an HTTP server (requires `serve` feature).

```bash
cargo build -p rvf-cli --features serve
rvf serve store.rvf --port 8080
```

### launch

Boot an RVF file in a QEMU microVM (requires `launch` feature).

```bash
cargo build -p rvf-cli --features launch
rvf launch store.rvf --port 8080 --memory-mb 256
```

## JSON output

All commands support `--json` for machine-readable output:

```bash
rvf status store.rvf --json | jq '.total_vectors'
rvf query store.rvf -v "1,0,0,0" -k 5 --json | jq '.results[].id'
```

## Platform scripts

Platform-specific quickstart scripts are in `examples/rvf/scripts/`:

```bash
# Linux / macOS
bash examples/rvf/scripts/rvf-quickstart.sh

# Windows PowerShell
.\examples\rvf\scripts\rvf-quickstart.ps1
```

## License

MIT OR Apache-2.0
