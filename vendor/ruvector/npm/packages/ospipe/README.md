# @ruvector/ospipe

**RuVector-enhanced personal AI memory SDK for Screenpipe**

[![npm](https://img.shields.io/npm/v/@ruvector/ospipe.svg)](https://www.npmjs.com/package/@ruvector/ospipe)
[![npm](https://img.shields.io/npm/v/@ruvector/ospipe-wasm.svg?label=wasm)](https://www.npmjs.com/package/@ruvector/ospipe-wasm)
[![crates.io](https://img.shields.io/crates/v/ospipe.svg)](https://crates.io/crates/ospipe)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](https://opensource.org/licenses/MIT)
[![WASM](https://img.shields.io/badge/wasm-compatible-brightgreen)](https://webassembly.org/)

---

## What is OSpipe?

[Screenpipe](https://github.com/screenpipe/screenpipe) is an open-source desktop application that continuously records your screen, audio, and UI interactions locally. It builds a searchable timeline of everything you see, hear, and do on your computer. Out of the box, Screenpipe stores its data in SQLite with FTS5 full-text indexing -- effective for keyword lookups, but limited to literal string matching. If you search for "auth discussion," you will not find a frame that says "we talked about login security."

OSpipe replaces Screenpipe's storage and search backend with the [RuVector](https://github.com/ruvnet/ruvector) ecosystem -- a collection of 70+ Rust crates providing HNSW vector search, graph neural networks, attention mechanisms, delta-change tracking, and more. Instead of keyword matching, OSpipe embeds every captured frame into a high-dimensional vector space and performs approximate nearest neighbor search, delivering true semantic recall. A query like *"what was that API we discussed in standup?"* will surface the relevant audio transcription even if those exact words never appeared.

Everything stays local and private. OSpipe processes all data on-device with no cloud dependency. The safety gate automatically detects and redacts PII -- credit card numbers, Social Security numbers, and email addresses -- before content ever reaches the vector store. A cosine-similarity deduplication window prevents consecutive identical frames (like a static desktop) from bloating storage. Age-based quantization progressively compresses older embeddings from 32-bit floats down to 1-bit binary, cutting long-term memory usage by 97%.

**Ask your computer what you saw, heard, and did -- with semantic understanding.**

---

## Install

```bash
npm install @ruvector/ospipe
```

Also available:

| Package | Install | Description |
|---------|---------|-------------|
| [`@ruvector/ospipe`](https://www.npmjs.com/package/@ruvector/ospipe) | `npm install @ruvector/ospipe` | TypeScript SDK for Node.js and browser |
| [`@ruvector/ospipe-wasm`](https://www.npmjs.com/package/@ruvector/ospipe-wasm) | `npm install @ruvector/ospipe-wasm` | WASM bindings (145 KB) for browser-only use |
| [`ospipe`](https://crates.io/crates/ospipe) | `cargo add ospipe` | Rust crate with full pipeline |

---

## Features

- **Semantic Vector Search** -- HNSW index via `ruvector-core` with 61us p50 query latency
- **Knowledge Graph** -- Cypher queries over extracted entities (people, apps, topics, meetings)
- **Temporal Deltas** -- track how content changed over time with delta-behavior analysis
- **Attention Streaming** -- real-time SSE stream of attention-weighted events
- **PII Safety Gate** -- automatic redaction of credit card numbers, SSNs, and email addresses before storage
- **Frame Deduplication** -- cosine similarity sliding window eliminates near-duplicate captures
- **Query Router** -- automatically routes queries to the optimal backend (Semantic, Keyword, Graph, Temporal, or Hybrid)
- **Hybrid Search** -- weighted combination of semantic vector similarity and keyword term overlap
- **WASM Support** -- runs entirely in the browser with bundles from 11.8KB (micro) to 350KB (full)
- **Configurable Quantization** -- 4-tier age-based compression: f32 -> int8 -> product -> binary (97% savings)
- **Retry + Timeout** -- exponential backoff, AbortSignal support, configurable timeout
- **Screenpipe Compatible** -- backward-compatible `queryScreenpipe()` for existing code

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

### TypeScript SDK

```typescript
import { OsPipe } from "@ruvector/ospipe";

const client = new OsPipe({ baseUrl: "http://localhost:3030" });

// Semantic search across everything you've seen, heard, and done
const results = await client.queryRuVector(
  "what did we discuss about authentication?"
);

for (const hit of results) {
  console.log(`[${hit.score.toFixed(3)}] ${hit.content}`);
  console.log(`  app: ${hit.metadata.app}, time: ${hit.timestamp}`);
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

### Start the OSpipe Server

```bash
# Using the Rust binary
cargo install ospipe
ospipe-server --port 3030

# Or build from source
cargo build -p ospipe --release --bin ospipe-server
./target/release/ospipe-server --port 3030 --data-dir ~/.ospipe
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

## API Reference

### Constructor

```typescript
const client = new OsPipe({
  baseUrl: "http://localhost:3030",   // OSpipe server URL
  apiVersion: "v2",                   // API version ("v1" | "v2")
  defaultK: 10,                       // Default number of results
  hybridWeight: 0.7,                  // Semantic vs keyword weight (0-1)
  rerank: true,                       // Enable MMR deduplication
  timeout: 10_000,                    // Request timeout in ms
  maxRetries: 3,                      // Retry attempts for 5xx/network errors
});
```

### `queryRuVector(query, options?)` -- Semantic Search

```typescript
const results = await client.queryRuVector("user login issues", {
  k: 5,
  metric: "cosine",                   // "cosine" | "euclidean" | "dot"
  rerank: true,                       // MMR deduplication
  confidence: true,                   // Include confidence bounds
  filters: {
    app: "Chrome",
    contentType: "screen",            // "screen" | "audio" | "ui" | "all"
    timeRange: { start: "2026-02-12T00:00:00Z", end: "2026-02-12T23:59:59Z" },
    speaker: "Alice",
    monitor: 0,
    language: "en",
  },
});
```

Returns `SearchResult[]`:

```typescript
interface SearchResult {
  id: string;
  score: number;
  content: string;
  source: "screen" | "audio" | "ui";
  timestamp: string;
  metadata: {
    app?: string;
    window?: string;
    monitor?: number;
    speaker?: string;
    confidence?: number;
    language?: string;
  };
}
```

### `queryGraph(cypher)` -- Knowledge Graph

```typescript
const result = await client.queryGraph(
  "MATCH (p:Person)-[:MENTIONED_IN]->(m:Meeting) RETURN p, m LIMIT 10"
);

console.log(result.nodes);  // GraphNode[] with id, label, type, properties
console.log(result.edges);  // GraphEdge[] with source, target, type
```

Node types: `App`, `Window`, `Person`, `Topic`, `Meeting`, `Symbol`.

### `queryDelta(options)` -- Temporal Changes

```typescript
const deltas = await client.queryDelta({
  app: "VSCode",
  timeRange: {
    start: "2026-02-12T09:00:00Z",
    end: "2026-02-12T17:00:00Z",
  },
  includeChanges: true,
});

for (const delta of deltas) {
  console.log(`${delta.timestamp} [${delta.app}]`);
  for (const change of delta.changes) {
    console.log(`  -${change.removed} +${change.added}`);
  }
}
```

### `streamAttention(options?)` -- Real-Time Events

```typescript
for await (const event of client.streamAttention({
  threshold: 0.5,
  categories: ["code_change", "meeting_start"],
  signal: AbortSignal.timeout(60_000),
})) {
  console.log(`[${event.category}] ${event.summary} (${event.attention})`);
}
```

Event categories: `code_change`, `person_mention`, `topic_shift`, `context_switch`, `meeting_start`, `meeting_end`.

### `routeQuery(query)` -- Query Routing

```typescript
const route = await client.routeQuery("who mentioned auth yesterday?");
// route: "semantic" | "keyword" | "graph" | "temporal" | "hybrid"
```

### `stats()` -- Pipeline Statistics

```typescript
const stats = await client.stats();
// { totalIngested, totalDeduplicated, totalDenied, storageBytes, indexSize, uptime }
```

### `health()` -- Server Health

```typescript
const health = await client.health();
// { status: "ok", version: "0.1.0", backends: ["hnsw", "keyword", "graph"] }
```

### `queryScreenpipe(options)` -- Legacy API

Backward-compatible with `@screenpipe/js`:

```typescript
const results = await client.queryScreenpipe({
  q: "meeting notes",
  contentType: "ocr",         // "all" | "ocr" | "audio"
  limit: 20,
  appName: "Notion",
  startTime: "2026-02-12T00:00:00Z",
  endTime: "2026-02-12T23:59:59Z",
});
```

---

## Safety Gate

<details>
<summary>PII Detection Details</summary>

The safety gate inspects all captured content before it enters the ingestion pipeline. It operates in three modes:

| Decision | Behavior | When |
|---|---|---|
| **Allow** | Content stored as-is | No sensitive patterns detected |
| **AllowRedacted** | Content stored with PII replaced by tokens | PII detected, redaction enabled |
| **Deny** | Content rejected, not stored | Custom deny pattern matched |

**Detected PII patterns:**

- **Credit Cards** -- sequences of 13-16 digits (with optional spaces or dashes) -> `[CC_REDACTED]`
- **Social Security Numbers** -- XXX-XX-XXXX format -> `[SSN_REDACTED]`
- **Email Addresses** -- word@domain.tld patterns -> `[EMAIL_REDACTED]`
- **Sensitive Keywords** (WASM) -- `password`, `secret`, `api_key`, `api-key`, `apikey`, `token`, `private_key`, `private-key`

**WASM safety API:**

```javascript
pipe.safety_check("my card is 4111-1111-1111-1111"); // "deny"
pipe.safety_check("set password to foo123");          // "redact"
pipe.safety_check("the weather is nice today");       // "allow"
```

</details>

---

## Configuration Guide

<details>
<summary>Client Configuration</summary>

All configuration options with defaults:

| Option | Type | Default | Description |
|---|---|---|---|
| `baseUrl` | `string` | `"http://localhost:3030"` | OSpipe server URL |
| `apiVersion` | `"v1" \| "v2"` | `"v2"` | API version |
| `defaultK` | `number` | `10` | Default number of results |
| `hybridWeight` | `number` | `0.7` | Semantic vs keyword weight (0 = pure keyword, 1 = pure semantic) |
| `rerank` | `boolean` | `true` | Enable MMR deduplication |
| `timeout` | `number` | `10000` | Request timeout in milliseconds |
| `maxRetries` | `number` | `3` | Retry attempts for 5xx/network errors |

**Retry behavior:** The SDK uses exponential backoff starting at 300ms. Only network errors and HTTP 5xx responses are retried. Client errors (4xx) are never retried. Each request has an independent `AbortController` timeout.

```typescript
// High-throughput configuration
const client = new OsPipe({
  baseUrl: "http://localhost:3030",
  defaultK: 50,
  hybridWeight: 0.9,       // lean heavily toward semantic
  timeout: 30_000,          // 30s for large result sets
  maxRetries: 5,
});

// Low-latency configuration
const fast = new OsPipe({
  defaultK: 3,
  hybridWeight: 1.0,        // pure semantic, skip keyword
  rerank: false,             // skip MMR reranking
  timeout: 2_000,
  maxRetries: 0,             // fail fast, no retries
});
```

</details>

<details>
<summary>Server Configuration (Rust)</summary>

The OSpipe server is configured via `OsPipeConfig` with nested subsystem configs. All fields have sensible defaults.

| Subsystem | Key Fields | Defaults |
|-----------|-----------|----------|
| **Capture** | `fps`, `audio_chunk_secs`, `excluded_apps`, `skip_private_windows` | 1.0 fps, 30s chunks, excludes 1Password/Keychain |
| **Storage** | `embedding_dim`, `hnsw_m`, `hnsw_ef_construction`, `dedup_threshold` | 384 dims, M=32, ef=200, 0.95 threshold |
| **Search** | `default_k`, `hybrid_weight`, `mmr_lambda`, `rerank_enabled` | k=10, 0.7 hybrid, 0.5 MMR lambda |
| **Safety** | `pii_detection`, `credit_card_redaction`, `ssn_redaction`, `custom_patterns` | All enabled, no custom patterns |

```bash
# Start with defaults
ospipe-server --port 3030

# Custom data directory
ospipe-server --port 3030 --data-dir /var/lib/ospipe
```

</details>

---

## WASM Deployment

<details>
<summary>Bundle Tiers & Web Worker Setup</summary>

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

### WASM API Reference

| Method | Parameters | Returns | Description |
|---|---|---|---|
| `new(dimension)` | `number` | `OsPipeWasm` | Constructor |
| `insert(id, embedding, metadata, timestamp)` | `string, Float32Array, string, number` | `void` | Insert a frame |
| `search(query_embedding, k)` | `Float32Array, number` | `JSON array` | Semantic search |
| `search_filtered(query_embedding, k, start, end)` | `Float32Array, number, number, number` | `JSON array` | Time-filtered search |
| `is_duplicate(embedding, threshold)` | `Float32Array, number` | `boolean` | Deduplication check |
| `embed_text(text)` | `string` | `Float32Array` | Hash-based text embedding |
| `batch_embed(texts)` | `string[]` | `Float32Array[]` | Batch text embedding |
| `safety_check(content)` | `string` | `string` | Returns "allow", "redact", or "deny" |
| `route_query(query)` | `string` | `string` | Returns "Semantic", "Keyword", "Graph", or "Temporal" |
| `len()` | -- | `number` | Number of stored embeddings |
| `stats()` | -- | `string` (JSON) | Pipeline statistics |

</details>

---

## Quantization Tiers

<details>
<summary>Age-Based Memory Compression</summary>

OSpipe progressively compresses older embeddings to reduce long-term storage costs. The default quantization schedule:

| Age | Method | Bits/Dim | Memory vs f32 | Description |
|---|---|---|---|---|
| 0 hours | None (f32) | 32 | 100% | Full precision for recent content |
| 24 hours | Scalar (int8) | 8 | 25% | Minimal quality loss, 4x compression |
| 1 week | Product | ~2 | ~6% | Codebook-based compression |
| 30 days | Binary | 1 | 3% | Single bit per dimension, 97% savings |

### Memory Estimate

For 1 million frames at 384 dimensions:

| Tier | Bytes/Vector | Total (1M vectors) |
|---|---|---|
| f32 | 1,536 | 1.43 GB |
| int8 | 384 | 366 MB |
| Product | ~96 | ~91 MB |
| Binary | 48 | 46 MB |

With the default age distribution (most content aging past 30 days), long-term average storage is approximately **50-80 MB per million frames**.

</details>

---

## RuVector Crate Integration

OSpipe integrates 10 crates from the [RuVector](https://github.com/ruvnet/ruvector) ecosystem:

| RuVector Crate | OSpipe Usage | Status |
|---|---|---|
| `ruvector-core` | HNSW vector storage and nearest neighbor search | Integrated |
| `ruvector-filter` | Metadata filtering (app, time, content type) | Integrated |
| `ruvector-cluster` | Frame deduplication via cosine similarity | Integrated |
| `ruvector-delta-core` | Change tracking and delta-behavior analysis | Integrated |
| `ruvector-router-core` | Query routing to optimal search backend | Integrated |
| `cognitum-gate-kernel` | AI safety gate decisions (allow/redact/deny) | Integrated |
| `ruvector-graph` | Knowledge graph for entity relationships | Integrated |
| `ruvector-attention` | Content prioritization and relevance weighting | Integrated |
| `ruvector-gnn` | Learned search improvement via graph neural nets | Integrated |
| `ruqu-algorithms` | Quantum-inspired search diversity (MMR) | Integrated |

---

## Testing

```bash
# Run all 82 tests
cargo test -p ospipe

# Build for WASM (verify compilation)
cargo build -p ospipe --target wasm32-unknown-unknown

# Build with wasm-pack for JS bindings
wasm-pack build examples/OSpipe --target web
```

---

## Related

| Package | Description |
|---------|-------------|
| [`@ruvector/ospipe-wasm`](https://www.npmjs.com/package/@ruvector/ospipe-wasm) | WASM bindings for browser (145 KB) |
| [`ospipe`](https://crates.io/crates/ospipe) | Rust crate with full pipeline |
| [`ruvector`](https://www.npmjs.com/package/ruvector) | RuVector vector database |

- [Full Documentation & ADR](https://github.com/ruvnet/ruvector/tree/main/examples/OSpipe)
- [RuVector Ecosystem](https://github.com/ruvnet/ruvector) (70+ Rust crates)
- [Screenpipe](https://github.com/screenpipe/screenpipe)

---

## License

MIT
