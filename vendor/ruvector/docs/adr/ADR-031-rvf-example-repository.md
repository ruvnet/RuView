# ADR-031: RVF Example Repository — 24 Demonstrations Across Four Categories

- **Status**: Accepted
- **Date**: 2026-02-14
- **Supersedes**: None
- **Related**: ADR-029 (RVF Canonical Format), ADR-030 (Computational Container)

## Context

RVF (RuVector Format) is the unified agentic AI format — storage, transfer, and cognitive runtime in one file. The existing six examples (`basic_store`, `progressive_index`, `quantization`, `wire_format`, `crypto_signing`, `filtered_search`) demonstrate core storage and indexing features but do not cover:

- Agentic AI patterns (agent memory, swarm coordination, reasoning traces)
- Practical production patterns (RAG, recommendations, caching, deduplication)
- Vertical domain applications (genomics, finance, medical, legal)
- Exotic capabilities (quantum state, neuromorphic search, self-booting, eBPF)
- Runtime targets (browser/WASM, edge/IoT, serverless, ruvLLM inference)

Without concrete examples, users cannot discover or adopt the full scope of RVF.

## Decision

Create 24 new runnable examples organized into four categories, plus a cross-cutting runtime-targets group. Each example is a standalone `fn main()` in `examples/rvf/examples/` with inline documentation explaining the pattern.

### Category A: Agentic AI (6 examples)

| # | Example | File | What It Demonstrates |
|---|---------|------|---------------------|
| A1 | Agent Memory | `agent_memory.rs` | Persistent agent memory with witness audit trails, session recall |
| A2 | Swarm Knowledge | `swarm_knowledge.rs` | Multi-agent shared knowledge base with concurrent writes |
| A3 | Reasoning Trace | `reasoning_trace.rs` | Store chain-of-thought reasoning with lineage derivation |
| A4 | Tool Cache | `tool_cache.rs` | Cache tool call results with metadata filters and TTL |
| A5 | Agent Handoff | `agent_handoff.rs` | Transfer agent state between instances via RVF file |
| A6 | Experience Replay | `experience_replay.rs` | RL-style experience replay buffer with priority sampling |

### Category B: Practical Production (5 examples)

| # | Example | File | What It Demonstrates |
|---|---------|------|---------------------|
| B1 | Semantic Search | `semantic_search.rs` | Document search engine with metadata-filtered k-NN |
| B2 | Recommendation Engine | `recommendation.rs` | Item recommendations with collaborative filtering embeddings |
| B3 | RAG Pipeline | `rag_pipeline.rs` | Retrieval-augmented generation: chunk, embed, retrieve, rerank |
| B4 | Embedding Cache | `embedding_cache.rs` | LRU embedding cache with temperature tiering and eviction |
| B5 | Dedup Detector | `dedup_detector.rs` | Near-duplicate detection with threshold-based clustering |

### Category C: Vertical Domains (4 examples)

| # | Example | File | What It Demonstrates |
|---|---------|------|---------------------|
| C1 | Genomic Pipeline | `genomic_pipeline.rs` | DNA k-mer embeddings with `.rvdna` domain profile and lineage |
| C2 | Financial Signals | `financial_signals.rs` | Market signal embeddings with TEE attestation witness chains |
| C3 | Medical Imaging | `medical_imaging.rs` | Radiology embedding search with `.rvvis` profile |
| C4 | Legal Discovery | `legal_discovery.rs` | Legal document similarity with `.rvtext` profile and audit trails |

### Category D: Exotic Capabilities (5 examples)

| # | Example | File | What It Demonstrates |
|---|---------|------|---------------------|
| D1 | Self-Booting Service | `self_booting.rs` | RVF with embedded unikernel that boots as a microservice |
| D2 | eBPF Accelerator | `ebpf_accelerator.rs` | eBPF hot-path acceleration for sub-microsecond lookups |
| D3 | Hyperbolic Taxonomy | `hyperbolic_taxonomy.rs` | Hierarchy-aware search in hyperbolic space |
| D4 | Multi-Modal Fusion | `multimodal_fusion.rs` | Text + image embeddings in one RVF file with cross-modal search |
| D5 | Sealed Cognitive Engine | `sealed_engine.rs` | Full cognitive engine: vectors + LoRA + GNN + kernel + witness chain |

### Category E: Runtime Targets (4 examples)

| # | Example | File | What It Demonstrates |
|---|---------|------|---------------------|
| E1 | Browser WASM | `browser_wasm.rs` | Client-side vector search via 5.5 KB WASM microkernel |
| E2 | Edge IoT | `edge_iot.rs` | Constrained device with rvlite-style minimal API |
| E3 | Serverless Function | `serverless_function.rs` | Cold-start optimized RVF for Lambda/Cloud Functions |
| E4 | ruvLLM Inference | `ruvllm_inference.rs` | LLM KV cache + LoRA adapter management backed by RVF |

## Implementation

### File Organization

```
examples/rvf/
  Cargo.toml                          # Updated with 24 new [[example]] entries
  examples/
    # Existing (6)
    basic_store.rs
    progressive_index.rs
    quantization.rs
    wire_format.rs
    crypto_signing.rs
    filtered_search.rs
    # Agentic (6)
    agent_memory.rs
    swarm_knowledge.rs
    reasoning_trace.rs
    tool_cache.rs
    agent_handoff.rs
    experience_replay.rs
    # Practical (5)
    semantic_search.rs
    recommendation.rs
    rag_pipeline.rs
    embedding_cache.rs
    dedup_detector.rs
    # Vertical (4)
    genomic_pipeline.rs
    financial_signals.rs
    medical_imaging.rs
    legal_discovery.rs
    # Exotic (5)
    self_booting.rs
    ebpf_accelerator.rs
    hyperbolic_taxonomy.rs
    multimodal_fusion.rs
    sealed_engine.rs
    # Runtime Targets (4)
    browser_wasm.rs
    edge_iot.rs
    serverless_function.rs
    ruvllm_inference.rs
```

### Example Structure

Each example follows this pattern:

```rust
//! # Example Title
//!
//! Category: Agentic | Practical | Vertical | Exotic | Runtime
//!
//! **What this demonstrates:**
//! - Feature A
//! - Feature B
//!
//! **RVF segments used:** VEC, INDEX, WITNESS, ...
//!
//! **Run:** `cargo run --example example_name`

fn main() {
    // Self-contained, deterministic, no external dependencies
}
```

### Design Constraints

1. **Self-contained**: Each example runs without external services (databases, APIs, models)
2. **Deterministic**: Seeded RNG produces identical output across runs
3. **Fast**: Each completes in < 2 seconds on commodity hardware
4. **Documented**: Module-level doc comments explain the pattern and RVF segments used
5. **Buildable**: All examples compile against existing RVF crate APIs

### Dependencies

No new crate dependencies beyond what `examples/rvf/Cargo.toml` already provides:
- `rvf-runtime`, `rvf-types`, `rvf-wire`, `rvf-manifest`, `rvf-index`, `rvf-quant`, `rvf-crypto`
- `rand`, `tempfile`, `ed25519-dalek`

## Consequences

### Positive

- Users can discover all RVF capabilities through runnable code
- Each category targets a different audience (AI engineers, domain specialists, systems programmers)
- Examples serve as integration tests for advanced API surface
- The repository becomes a reference implementation catalog

### Negative

- 24 additional files to maintain (mitigated by CI: `cargo build --examples`)
- Some examples simulate external systems (LLM tokens, genomic data) with synthetic data
- Examples may drift from API as crates evolve (mitigated by workspace-level `cargo test`)

### Neutral

- Examples are not benchmarks; performance numbers are illustrative
- Domain-specific examples (genomics, finance) use synthetic data, not real datasets
