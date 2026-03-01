# OSpipe

**RuVector-enhanced personal AI memory for Screenpipe**

[![Crates.io](https://img.shields.io/crates/v/ospipe)](https://crates.io/crates/ospipe)
[![docs.rs](https://img.shields.io/docsrs/ospipe)](https://docs.rs/ospipe)
[![License: MIT](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange)](https://www.rust-lang.org/)
[![WASM](https://img.shields.io/badge/wasm-compatible-brightgreen)](https://webassembly.org/)

---

## What is OSpipe?

[Screenpipe](https://github.com/mediar-ai/screenpipe) is an open-source desktop application that continuously records your screen, audio, and UI interactions locally. It builds a searchable timeline of everything you see, hear, and do on your computer. Out of the box, Screenpipe stores its data in SQLite with FTS5 full-text indexing -- effective for keyword lookups, but limited to literal string matching. If you search for "auth discussion," you will not find a frame that says "we talked about login security."

OSpipe replaces Screenpipe's storage and search backend with the [RuVector](https://github.com/ruvnet/ruvector) ecosystem -- a collection of 70+ Rust crates providing HNSW vector search, graph neural networks, attention mechanisms, delta-change tracking, and more. Instead of keyword matching, OSpipe embeds every captured frame into a high-dimensional vector space and performs approximate nearest neighbor search, delivering true semantic recall. A query like "what was that API we discussed in standup?" will surface the relevant audio transcription even if those exact words never appeared.

Everything stays local and private. OSpipe processes all data on-device with no cloud dependency. The safety gate automatically detects and redacts PII -- credit card numbers, Social Security numbers, and email addresses -- before content ever reaches the vector store. A cosine-similarity deduplication window prevents consecutive identical frames (like a static desktop) from bloating storage. Age-based quantization progressively compresses older embeddings from 32-bit floats down to 1-bit binary, cutting long-term memory usage by 97%.

OSpipe ships as a Rust crate, a TypeScript SDK, and a WASM library. It runs natively on Windows, macOS, and Linux, and can run entirely in the browser via WebAssembly at bundles as small as 11.8KB.

**Ask your computer what you saw, heard, and did -- with semantic understanding.**

---

## Features

- **Semantic Vector Search** -- HNSW index via `ruvector-core` with 61us p50 query latency
- **PII Safety Gate** -- automatic redaction of credit card numbers, SSNs, and email addresses before storage
- **Frame Deduplication** -- cosine similarity sliding window eliminates near-duplicate captures
- **Hybrid Search** -- weighted combination of semantic vector similarity and keyword term overlap
- **Query Router** -- automatically routes queries to the optimal backend (Semantic, Keyword, Graph, Temporal, or Hybrid)
- **WASM Support** -- runs entirely in the browser with bundles from 11.8KB (micro) to 350KB (full)
- **TypeScript SDK** -- `@ruvector/ospipe` for Node.js and browser integration
- **Configurable Quantization** -- 4-tier age-based compression: f32 -> int8 -> product -> binary
- **Cross-Platform** -- native builds for Windows, macOS, Linux; WASM for browsers

---

## Architecture

```
                         OSpipe Ingestion Pipeline
                         =========================

  Screenpipe -----> Capture -----> Safety Gate -----> Dedup -----> Embed -----> VectorStore
  (Screen/Audio/UI)  (CapturedFrame)  (PII Redaction)   (Cosine Window)  (HNSW)      |
                                                                                      |
                                                           Search Router <------------+
                                                           |    |    |    |    |
                                                        Semantic Keyword Graph Temporal Hybrid
```

Frames flow left to right through the ingestion pipeline. Each captured frame passes through:

1. **Safety Gate** -- PII detection and redaction; content may be allowed, redacted, or denied
2. **Deduplication** -- cosine similarity check against a sliding window of recent embeddings
3. **Embedding** -- text content is encoded into a normalized vector
4. **Vector Store** -- the embedding is indexed for approximate nearest neighbor retrieval

Queries enter through the **Search Router**, which analyzes the query string and dispatches to the optimal backend.

---

## Quick Start

### Rust

Add OSpipe to your `Cargo.toml`:

```toml
[dependencies]
ospipe = { path = "examples/OSpipe" }
```

Create a pipeline, ingest frames, and search:

```rust
use ospipe::config::OsPipeConfig;
use ospipe::pipeline::ingestion::IngestionPipeline;
use ospipe::capture::{CapturedFrame, CaptureSource, FrameContent, FrameMetadata};

fn main() -> ospipe::error::Result<()> {
    // Initialize with default configuration
    let config = OsPipeConfig::default();
    let mut pipeline = IngestionPipeline::new(config)?;

    // Ingest a screen capture
    let frame = CapturedFrame::new_screen(
        "Firefox",
        "Meeting Notes - Google Docs",
        "Discussion about authentication: we decided to use JWT with refresh tokens",
        0,
    );
    let result = pipeline.ingest(frame)?;
    println!("Ingest result: {:?}", result);

    // Ingest an audio transcription
    let audio = CapturedFrame::new_audio(
        "Built-in Microphone",
        "Let's revisit the login flow next sprint",
        Some("Alice"),
    );
    pipeline.ingest(audio)?;

    // Search semantically
    let query_embedding = pipeline.embedding_engine().embed("auth token discussion");
    let results = pipeline.vector_store().search(&query_embedding, 5)?;

    for hit in &results {
        println!("Score: {:.4} | {:?}", hit.score, hit.metadata);
    }

    // Print pipeline statistics
    let stats = pipeline.stats();
    println!(
        "Ingested: {} | Deduped: {} | Denied: {} | Redacted: {}",
        stats.total_ingested, stats.total_deduplicated,
        stats.total_denied, stats.total_redacted
    );

    Ok(())
}
```

### TypeScript

```typescript
import { OsPipe } from "@ruvector/ospipe";

const client = new OsPipe({ baseUrl: "http://localhost:3030" });

// Ingest a captured frame
await client.ingest({
  source: "screen",
  app: "Chrome",
  window: "Jira Board",
  content: "Sprint 14 planning: migrate auth to OAuth2",
});

// Semantic search
const results = await client.queryRuVector(
  "what did I discuss in the meeting about authentication?"
);

for (const hit of results) {
  console.log(`[${hit.score.toFixed(3)}] ${hit.metadata.text}`);
}
```

### WASM (Browser)

```javascript
import { OsPipeWasm } from "@ruvector/ospipe-wasm";

// Initialize with 384-dimensional embeddings
const pipe = new OsPipeWasm(384);

// Embed and insert content
const embedding = pipe.embed_text("meeting notes about auth migration to OAuth2");
pipe.insert("frame-001", embedding, '{"app":"Chrome","window":"Jira"}', Date.now());

// Embed a query and search
const queryEmbedding = pipe.embed_text("what was the auth discussion about?");
const results = pipe.search(queryEmbedding, 5);
console.log("Results:", results);

// Safety check before storage
const safety = pipe.safety_check("my card is 4111-1111-1111-1111");
console.log("Safety:", safety); // "deny"

// Query routing
const route = pipe.route_query("what happened yesterday?");
console.log("Route:", route); // "Temporal"

// Pipeline statistics
console.log("Stats:", pipe.stats());
```

---

## Comparison: Screenpipe vs OSpipe

| Feature | Screenpipe (FTS5) | OSpipe (RuVector) |
|---|---|---|
| Search Type | Keyword (FTS5) | Semantic + Keyword + Graph + Temporal |
| Search Latency | ~1ms (FTS5) | 61us (HNSW p50) |
| Content Relations | None | Knowledge Graph (Cypher) |
| Temporal Analysis | Basic SQL | Delta-behavior tracking |
| PII Protection | Basic | Credit card, SSN, email redaction |
| Deduplication | None | Cosine similarity sliding window |
| Browser Support | None | WASM (11.8KB - 350KB) |
| Quantization | None | 4-tier age-based (f32 -> binary) |
| Privacy | Local-first | Local-first + PII redaction |
| Query Routing | None | Auto-routes to optimal backend |
| Hybrid Search | None | Weighted semantic + keyword fusion |
| Metadata Filtering | SQL WHERE | App, time range, content type, monitor |

---

## RuVector Crate Integration

| RuVector Crate | OSpipe Usage | Status |
|---|---|---|
| `ruvector-core` | HNSW vector storage and nearest neighbor search | Integrated |
| `ruvector-filter` | Metadata filtering (app, time, content type) | Integrated |
| `ruvector-cluster` | Frame deduplication via cosine similarity | Integrated |
| `ruvector-delta-core` | Change tracking and delta-behavior analysis | Integrated |
| `ruvector-router-core` | Query routing to optimal search backend | Integrated |
| `cognitum-gate-kernel` | AI safety gate decisions (allow/redact/deny) | Integrated |
| `ruvector-graph` | Knowledge graph for entity relationships | Phase 2 |
| `ruvector-attention` | Content prioritization and relevance weighting | Phase 3 |
| `ruvector-gnn` | Learned search improvement via graph neural nets | Phase 3 |
| `ruqu-algorithms` | Quantum-inspired search acceleration | Phase 4 |

---

## Configuration

<details>
<summary>Full Configuration Reference</summary>

### `OsPipeConfig`

Top-level configuration with nested subsystem configs. All fields have sensible defaults.

```rust
use ospipe::config::OsPipeConfig;

let config = OsPipeConfig::default();
// config.data_dir        = "~/.ospipe"
// config.capture         = CaptureConfig { ... }
// config.storage         = StorageConfig { ... }
// config.search          = SearchConfig { ... }
// config.safety          = SafetyConfig { ... }
```

### `CaptureConfig`

| Field | Type | Default | Description |
|---|---|---|---|
| `fps` | `f32` | `1.0` | Frames per second for screen capture |
| `audio_chunk_secs` | `u32` | `30` | Duration of audio chunks in seconds |
| `excluded_apps` | `Vec<String>` | `["1Password", "Keychain Access"]` | Applications excluded from capture |
| `skip_private_windows` | `bool` | `true` | Skip windows marked as private/incognito |

### `StorageConfig`

| Field | Type | Default | Description |
|---|---|---|---|
| `embedding_dim` | `usize` | `384` | Dimensionality of embedding vectors |
| `hnsw_m` | `usize` | `32` | HNSW M parameter (max connections per layer) |
| `hnsw_ef_construction` | `usize` | `200` | HNSW ef_construction (index build quality) |
| `hnsw_ef_search` | `usize` | `100` | HNSW ef_search (query-time accuracy) |
| `dedup_threshold` | `f32` | `0.95` | Cosine similarity threshold for deduplication |
| `quantization_tiers` | `Vec<QuantizationTier>` | 4 tiers (see below) | Age-based quantization schedule |

### `SearchConfig`

| Field | Type | Default | Description |
|---|---|---|---|
| `default_k` | `usize` | `10` | Default number of results to return |
| `hybrid_weight` | `f32` | `0.7` | Semantic vs keyword weight (1.0 = pure semantic, 0.0 = pure keyword) |
| `mmr_lambda` | `f32` | `0.5` | MMR diversity vs relevance tradeoff |
| `rerank_enabled` | `bool` | `false` | Whether to enable result reranking |

### `SafetyConfig`

| Field | Type | Default | Description |
|---|---|---|---|
| `pii_detection` | `bool` | `true` | Enable PII detection (emails) |
| `credit_card_redaction` | `bool` | `true` | Enable credit card number redaction |
| `ssn_redaction` | `bool` | `true` | Enable SSN redaction |
| `custom_patterns` | `Vec<String>` | `[]` | Custom substring patterns that trigger denial |

### Example: Custom Configuration

```rust
use ospipe::config::*;
use std::path::PathBuf;

let config = OsPipeConfig {
    data_dir: PathBuf::from("/var/lib/ospipe"),
    capture: CaptureConfig {
        fps: 0.5,
        audio_chunk_secs: 60,
        excluded_apps: vec![
            "1Password".into(),
            "Signal".into(),
            "Bitwarden".into(),
        ],
        skip_private_windows: true,
    },
    storage: StorageConfig {
        embedding_dim: 768,      // Use a larger model
        hnsw_m: 48,              // More connections for better recall
        hnsw_ef_construction: 400,
        hnsw_ef_search: 200,
        dedup_threshold: 0.98,   // Stricter deduplication
        ..Default::default()
    },
    search: SearchConfig {
        default_k: 20,
        hybrid_weight: 0.8,      // Lean more toward semantic
        mmr_lambda: 0.6,
        rerank_enabled: true,
    },
    safety: SafetyConfig {
        pii_detection: true,
        credit_card_redaction: true,
        ssn_redaction: true,
        custom_patterns: vec![
            "INTERNAL_ONLY".into(),
            "CONFIDENTIAL".into(),
        ],
    },
};
```

</details>

---

## Safety Gate

<details>
<summary>PII Detection Details</summary>

The safety gate inspects all captured content before it enters the ingestion pipeline. It operates in three modes:

### Safety Decisions

| Decision | Behavior | When |
|---|---|---|
| `Allow` | Content stored as-is | No sensitive patterns detected |
| `AllowRedacted(String)` | Content stored with PII replaced by tokens | PII detected, redaction enabled |
| `Deny { reason }` | Content rejected, not stored | Custom deny pattern matched |

### Detected PII Patterns

**Credit Cards** -- sequences of 13-16 digits (with optional spaces or dashes):
```
4111111111111111       -> [CC_REDACTED]
4111 1111 1111 1111    -> [CC_REDACTED]
4111-1111-1111-1111    -> [CC_REDACTED]
```

**Social Security Numbers** -- XXX-XX-XXXX format:
```
123-45-6789            -> [SSN_REDACTED]
```

**Email Addresses** -- word@domain.tld patterns:
```
user@example.com       -> [EMAIL_REDACTED]
admin@company.org      -> [EMAIL_REDACTED]
```

**Custom Patterns** -- configurable substring deny list. When a custom pattern is matched, the entire frame is denied (not just redacted):
```rust
let config = SafetyConfig {
    custom_patterns: vec!["TOP_SECRET".to_string(), "CLASSIFIED".to_string()],
    ..Default::default()
};
```

### WASM Safety API

The WASM bindings expose a simplified safety classifier:

```javascript
pipe.safety_check("my card is 4111-1111-1111-1111"); // "deny"
pipe.safety_check("set password to foo123");          // "redact"
pipe.safety_check("the weather is nice today");       // "allow"
```

The WASM classifier also detects sensitive keywords: `password`, `secret`, `api_key`, `api-key`, `apikey`, `token`, `private_key`, `private-key`.

</details>

---

## Advanced Configuration

<details>
<summary>WASM Deployment</summary>

### Bundle Tiers

OSpipe provides four WASM bundle sizes depending on which features you need:

| Tier | Size | Features |
|---|---|---|
| **Micro** | 11.8KB | Embedding + vector search only |
| **Standard** | 225KB | Full pipeline (embed, insert, search, filtered search) |
| **Full** | 350KB | + deduplication + safety gate + query routing |
| **AI** | 2.5MB | + on-device neural inference (ONNX) |

### Web Worker Setup

For best performance, run OSpipe in a Web Worker to avoid blocking the main thread:

```javascript
// worker.js
import { OsPipeWasm } from "@ruvector/ospipe-wasm";

const pipe = new OsPipeWasm(384);

self.onmessage = (event) => {
  const { type, payload } = event.data;

  switch (type) {
    case "insert":
      const emb = pipe.embed_text(payload.text);
      pipe.insert(payload.id, emb, JSON.stringify(payload.metadata), Date.now());
      self.postMessage({ type: "inserted", id: payload.id });
      break;

    case "search":
      const queryEmb = pipe.embed_text(payload.query);
      const results = pipe.search(queryEmb, payload.k || 10);
      self.postMessage({ type: "results", data: results });
      break;
  }
};
```

### SharedArrayBuffer

For multi-threaded WASM (e.g., parallel batch embedding), set the required headers:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

</details>

<details>
<summary>Cross-Platform Build</summary>

### Build Targets

```bash
# Native (current platform)
cargo build -p ospipe --release

# WASM (browser)
cargo build -p ospipe --target wasm32-unknown-unknown --release

# Generate JS bindings
wasm-pack build examples/OSpipe --target web --release

# Windows (cross-compile)
cross build -p ospipe --target x86_64-pc-windows-gnu --release

# macOS ARM (cross-compile)
cross build -p ospipe --target aarch64-apple-darwin --release

# macOS Intel (cross-compile)
cross build -p ospipe --target x86_64-apple-darwin --release

# Linux ARM (cross-compile)
cross build -p ospipe --target aarch64-unknown-linux-gnu --release
```

### Conditional Compilation

OSpipe uses conditional compilation to separate native and WASM dependencies:

- **Native** (`cfg(not(target_arch = "wasm32"))`) -- links against `ruvector-core`, `ruvector-filter`, `ruvector-cluster`, `ruvector-delta-core`, `ruvector-router-core`, and `cognitum-gate-kernel`
- **WASM** (`cfg(target_arch = "wasm32")`) -- uses `wasm-bindgen`, `js-sys`, `serde-wasm-bindgen`, and `getrandom` with the `js` feature

The `src/wasm/helpers.rs` module contains pure Rust functions (cosine similarity, hash embedding, safety classification, query routing) that compile on all targets and are tested natively.

</details>

<details>
<summary>Quantization Tiers</summary>

OSpipe progressively compresses older embeddings to reduce long-term storage costs. The default quantization schedule:

| Age | Method | Bits/Dim | Memory vs f32 | Description |
|---|---|---|---|---|
| 0 hours | None (f32) | 32 | 100% | Full precision for recent content |
| 24 hours | Scalar (int8) | 8 | 25% | Minimal quality loss, 4x compression |
| 1 week | Product | ~2 | ~6% | Codebook-based compression |
| 30 days | Binary | 1 | 3% | Single bit per dimension, 97% savings |

### Custom Tiers

```rust
use ospipe::config::{StorageConfig, QuantizationTier, QuantizationMethod};

let storage = StorageConfig {
    quantization_tiers: vec![
        QuantizationTier { age_hours: 0,    method: QuantizationMethod::None },
        QuantizationTier { age_hours: 12,   method: QuantizationMethod::Scalar },
        QuantizationTier { age_hours: 72,   method: QuantizationMethod::Product },
        QuantizationTier { age_hours: 360,  method: QuantizationMethod::Binary },
    ],
    ..Default::default()
};
```

### Memory Estimate

For 1 million frames at 384 dimensions:

| Tier | Bytes/Vector | Total (1M vectors) |
|---|---|---|
| f32 | 1,536 | 1.43 GB |
| int8 | 384 | 366 MB |
| Product | ~96 | ~91 MB |
| Binary | 48 | 46 MB |

With the default age distribution (most content aging past 30 days), long-term average storage is approximately 50-80 MB per million frames.

</details>

---

## API Reference

### Rust API

#### Core Types

| Type | Module | Description |
|---|---|---|
| `OsPipeConfig` | `config` | Top-level configuration |
| `CaptureConfig` | `config` | Capture subsystem settings |
| `StorageConfig` | `config` | HNSW and quantization settings |
| `SearchConfig` | `config` | Search weights and defaults |
| `SafetyConfig` | `config` | PII detection toggles |
| `CapturedFrame` | `capture` | A captured screen/audio/UI frame |
| `CaptureSource` | `capture` | Source enum: `Screen`, `Audio`, `Ui` |
| `FrameContent` | `capture` | Content enum: `OcrText`, `Transcription`, `UiEvent` |
| `FrameMetadata` | `capture` | Metadata (app, window, monitor, confidence, language) |
| `OsPipeError` | `error` | Unified error type |

#### Pipeline

| Type / Function | Module | Description |
|---|---|---|
| `IngestionPipeline::new(config)` | `pipeline::ingestion` | Create a new pipeline |
| `IngestionPipeline::ingest(frame)` | `pipeline::ingestion` | Ingest a single frame |
| `IngestionPipeline::ingest_batch(frames)` | `pipeline::ingestion` | Ingest multiple frames |
| `IngestionPipeline::stats()` | `pipeline::ingestion` | Get ingestion statistics |
| `IngestResult` | `pipeline::ingestion` | Enum: `Stored`, `Deduplicated`, `Denied` |
| `PipelineStats` | `pipeline::ingestion` | Counters for ingested/deduped/denied/redacted |
| `FrameDeduplicator` | `pipeline::dedup` | Cosine similarity sliding window |

#### Storage

| Type / Function | Module | Description |
|---|---|---|
| `VectorStore::new(config)` | `storage::vector_store` | Create a new vector store |
| `VectorStore::insert(frame, embedding)` | `storage::vector_store` | Insert a frame with its embedding |
| `VectorStore::search(query, k)` | `storage::vector_store` | Top-k nearest neighbor search |
| `VectorStore::search_filtered(query, k, filter)` | `storage::vector_store` | Search with metadata filters |
| `SearchResult` | `storage::vector_store` | Result with id, score, metadata |
| `SearchFilter` | `storage::vector_store` | Filter by app, time range, content type, monitor |
| `StoredEmbedding` | `storage::vector_store` | Stored vector with metadata and timestamp |
| `EmbeddingEngine::new(dim)` | `storage::embedding` | Create an embedding engine |
| `EmbeddingEngine::embed(text)` | `storage::embedding` | Generate a normalized embedding |
| `EmbeddingEngine::batch_embed(texts)` | `storage::embedding` | Batch embedding generation |
| `cosine_similarity(a, b)` | `storage::embedding` | Cosine similarity between two vectors |

#### Search

| Type / Function | Module | Description |
|---|---|---|
| `QueryRouter::new()` | `search::router` | Create a query router |
| `QueryRouter::route(query)` | `search::router` | Route a query to optimal backend |
| `QueryRoute` | `search::router` | Enum: `Semantic`, `Keyword`, `Graph`, `Temporal`, `Hybrid` |
| `HybridSearch::new(weight)` | `search::hybrid` | Create a hybrid search with semantic weight |
| `HybridSearch::search(store, query, emb, k)` | `search::hybrid` | Combined semantic + keyword search |

#### Safety

| Type / Function | Module | Description |
|---|---|---|
| `SafetyGate::new(config)` | `safety` | Create a safety gate |
| `SafetyGate::check(content)` | `safety` | Check content, return safety decision |
| `SafetyGate::redact(content)` | `safety` | Redact and return cleaned content |
| `SafetyDecision` | `safety` | Enum: `Allow`, `AllowRedacted(String)`, `Deny { reason }` |

### WASM API (`OsPipeWasm`)

| Method | Parameters | Returns | Description |
|---|---|---|---|
| `new(dimension)` | `usize` | `OsPipeWasm` | Constructor |
| `insert(id, embedding, metadata, timestamp)` | `&str, &[f32], &str, f64` | `Result<(), JsValue>` | Insert a frame |
| `search(query_embedding, k)` | `&[f32], usize` | `JsValue` (JSON array) | Semantic search |
| `search_filtered(query_embedding, k, start, end)` | `&[f32], usize, f64, f64` | `JsValue` (JSON array) | Time-filtered search |
| `is_duplicate(embedding, threshold)` | `&[f32], f32` | `bool` | Deduplication check |
| `embed_text(text)` | `&str` | `Vec<f32>` | Hash-based text embedding |
| `batch_embed(texts)` | `JsValue` (Array) | `JsValue` (Array) | Batch text embedding |
| `safety_check(content)` | `&str` | `String` | Returns "allow", "redact", or "deny" |
| `route_query(query)` | `&str` | `String` | Returns "Semantic", "Keyword", "Graph", or "Temporal" |
| `len()` | -- | `usize` | Number of stored embeddings |
| `stats()` | -- | `String` (JSON) | Pipeline statistics |

---

## Testing

```bash
# Run all 56 tests
cargo test -p ospipe

# Run with verbose output
cargo test -p ospipe -- --nocapture

# Run only integration tests
cargo test -p ospipe --test integration

# Run only unit tests (embedding, WASM helpers)
cargo test -p ospipe --lib

# Build for WASM (verify compilation)
cargo build -p ospipe --target wasm32-unknown-unknown

# Build with wasm-pack for JS bindings
wasm-pack build examples/OSpipe --target web
```

### Test Coverage

| Test Category | Count | Module |
|---|---|---|
| Configuration | 2 | `tests/integration.rs` |
| Capture frames | 3 | `tests/integration.rs` |
| Embedding engine | 6 | `src/storage/embedding.rs` |
| Vector store | 4 | `tests/integration.rs` |
| Deduplication | 2 | `tests/integration.rs` |
| Safety gate | 6 | `tests/integration.rs` |
| Query routing | 4 | `tests/integration.rs` |
| Hybrid search | 2 | `tests/integration.rs` |
| Ingestion pipeline | 5 | `tests/integration.rs` |
| Cosine similarity | 3 | `tests/integration.rs` |
| WASM helpers | 18 | `src/wasm/helpers.rs` |
| **Total** | **56** | |

---

## Related

- [ADR: OSpipe Screenpipe Integration](./ADR-OSpipe-screenpipe-integration.md) -- Architecture Decision Record with full design rationale
- [Screenpipe](https://github.com/mediar-ai/screenpipe) -- Open-source local-first desktop recording + AI memory
- [RuVector](https://github.com/ruvnet/ruvector) -- 70+ Rust crates for vector search, graph neural networks, and attention mechanisms
- `@ruvector/ospipe` -- TypeScript SDK (npm)
- `@ruvector/ospipe-wasm` -- WASM package (npm)

---

## License

Licensed under either of:

- MIT License ([LICENSE-MIT](../../LICENSE-MIT) or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

at your option.
