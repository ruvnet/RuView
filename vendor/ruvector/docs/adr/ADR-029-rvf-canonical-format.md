# ADR-029: RVF as Canonical Binary Format Across All RuVector Libraries

**Status**: Accepted
**Date**: 2026-02-13
**Authors**: ruv.io, RuVector Architecture Team
**Deciders**: Architecture Review Board
**SDK**: Claude-Flow
**Supersedes**: Portions of ADR-001 (storage layer), ADR-018 (block-based storage)

## Context

### The Format Fragmentation Problem

The RuVector ecosystem currently spans 70+ Rust crates and 50+ npm packages across multiple libraries:

- **ruvector-core** — HNSW-based vector database with REDB storage
- **agentdb** (`npx agentdb`) — AI agent memory with HNSW indexing
- **claude-flow** (`npx @claude-flow/cli@latest`) — Multi-agent orchestration with memory subsystem
- **agentic-flow** (`npx agentic-flow`) — Swarm coordination with shared memory
- **ospipe** — Observation-State pipeline with vector persistence
- **rvlite** — Lightweight embedded vector store
- **sona** — Self-optimizing neural architecture with vector storage

Each library invented its own serialization: REDB tables, bincode blobs, JSON-backed HNSW dumps, custom binary formats. This fragmentation means:

1. **No interoperability** — An agentdb memory file cannot be queried by claude-flow
2. **Duplicated effort** — Each library re-implements indexing, quantization, persistence
3. **No progressive loading** — All formats require full deserialization before first query
4. **No hardware adaptation** — No format targets both WASM tiles and server-class hardware
5. **No crash safety** — Most formats rely on external journaling or are not crash-safe

### The RVF Research Outcome

The RVF (RuVector Format) research (`docs/research/rvf/`) produced a comprehensive specification for a universal, self-reorganizing binary substrate. RVF provides:

- Append-only segments with per-segment integrity (crash-safe without WAL)
- Two-level manifest with 4 KB instant boot
- Progressive indexing (Layer A/B/C) for first-query-before-full-load
- Temperature-tiered quantization (fp16/int8/PQ/binary)
- WASM microkernel for 64 KB Cognitum tiles to petabyte hubs
- Post-quantum cryptographic signatures (ML-DSA-65)
- Domain profiles (RVDNA, RVText, RVGraph, RVVision)
- Full wire format specification with batch query/ingest/delete APIs

## Decision

### Adopt RVF as the single canonical binary format for all RuVector libraries

### Segment Forward Compatibility

RVF readers and rewriters MUST skip segment types they do not recognize and MUST preserve them byte-for-byte on rewrite. This prevents older tools from silently deleting newer segment types (e.g., KERNEL_SEG, EBPF_SEG) when compacting or migrating files. The rule is: if you did not create it and do not understand it, pass it through unchanged.

All libraries in the RuVector ecosystem that persist or exchange vector data MUST use RVF as their storage and interchange format. This applies to:

| Library | Current Format | Migration Path |
|---------|---------------|----------------|
| ruvector-core | REDB + bincode | RVF as primary, REDB as optional metadata store |
| agentdb | Custom HNSW + JSON | RVF with RVText profile |
| claude-flow memory | JSON + flat files | RVF with WITNESS_SEG for audit trails |
| agentic-flow | Shared memory blobs | RVF streaming protocol for inter-agent exchange |
| ospipe | Custom binary | RVF with META_SEG for observation state |
| rvlite | bincode dump | RVF Core Profile (minimal, fits WASM) |
| sona | Custom persistence | RVF with SKETCH_SEG for learning patterns |

### Implementation Architecture

```
                     ┌─────────────────────────────────┐
                     │         Application Layer        │
                     │  claude-flow │ agentdb │ agentic │
                     └─────────┬───────────────────────┘
                               │
                     ┌─────────▼───────────────────────┐
                     │      RVF SDK Layer (rvf crate)   │
                     │  read │ write │ query │ stream   │
                     │  progressive │ manifest │ crypto │
                     └─────────┬───────────────────────┘
                               │
              ┌────────────────┼────────────────┐
              │                │                │
     ┌────────▼──────┐ ┌──────▼───────┐ ┌──────▼───────┐
     │  rvf-core     │ │  rvf-wasm    │ │  rvf-node    │
     │  (Rust lib)   │ │  (WASM pkg)  │ │  (N-API pkg) │
     │  Full Profile │ │  Core Profile│ │  Full Profile│
     └───────────────┘ └──────────────┘ └──────────────┘
```

### Crate Structure

```
crates/
  rvf/                    # Core RVF library (no_std compatible)
    rvf-types/            # Segment types, headers, enums (no_std)
    rvf-wire/             # Wire format read/write (no_std)
    rvf-index/            # HNSW progressive indexing
    rvf-manifest/         # Two-level manifest system
    rvf-quant/            # Temperature-tiered quantization
    rvf-crypto/           # ML-DSA-65, SHAKE-256, segment signing
    rvf-runtime/          # Full runtime with compaction, streaming
    rvf-wasm/             # WASM microkernel (Cognitum tile target)
    rvf-node/             # N-API bindings for Node.js
    rvf-server/           # TCP/HTTP streaming server
```

### NPM Package Structure

```
npm/packages/
  rvf/                    # Main npm package (TypeScript API)
  rvf-wasm/               # WASM build for browsers
  rvf-node/               # Native N-API for Node.js (platform-specific)
  rvf-node-linux-x64-gnu/ # Platform binary
  rvf-node-darwin-arm64/  # Platform binary
  rvf-node-win32-x64/     # Platform binary
```

### Library Integration Points

#### claude-flow Integration

```rust
// claude-flow memory stores become RVF files
// Memory search -> RVF query with progressive indexing
// Agent audit trail -> WITNESS_SEG with hash chains
// Cross-session persistence -> RVF append-only segments

use rvf_runtime::RvfStore;

let store = RvfStore::open("agent-memory.rvf")?;
store.ingest_batch(&embeddings, &metadata)?;
let results = store.query(&query_vector, k, ef_search)?;
```

#### agentdb Integration

```rust
// AgentDB HNSW index -> RVF INDEX_SEG (Layer A/B/C)
// AgentDB memory patterns -> RVF with RVText profile
// AgentDB vector search -> RVF progressive query path
// AgentDB persistence -> RVF segment model

use rvf_runtime::{RvfStore, Profile};

let store = RvfStore::create("agent.rvf", Profile::RVText)?;
// Existing AgentDB API wraps RVF operations
```

#### agentic-flow Integration

```rust
// Inter-agent memory sharing -> RVF streaming protocol
// Swarm coordination state -> RVF META_SEG
// Agent learning patterns -> RVF SKETCH_SEG
// Distributed consensus -> RVF WITNESS_SEG with signatures

use rvf_runtime::streaming::RvfStream;

let stream = RvfStream::connect("agent-hub:9090")?;
stream.subscribe(epoch_since)?;
```

### Confidential Core Attestation

RVF supports hardware Confidential Computing attestation via the
Confidential Core model. TEE attestation quotes are stored in WITNESS_SEG
payloads alongside vector data, enabling verifiable proof of:

1. **Platform Attestation** (`witness_type = 0x05`): Proof that vector
   operations occurred in a verified TEE (SGX, SEV-SNP, TDX, ARM CCA).
   Segments produced inside an attested TEE set `SegmentFlags::ATTESTED`
   (bit 10) for fast scanning.

2. **Key Binding** (`witness_type = 0x06`): Encryption keys sealed to
   a TEE measurement via `key_type = 4` in CRYPTO_SEG. Data is only
   accessible within environments matching the recorded measurement.

3. **Computation Proofs** (`witness_type = 0x07`): Verifiable records
   that specific queries or operations were performed inside the enclave,
   with query/result hashes in the report data.

4. **Data Provenance** (`witness_type = 0x08`): Chain of custody from
   embedding model through TEE to RVF file, binding model identity
   to the attestation nonce.

#### Attestation Wire Format

Attestation records use a 112-byte `AttestationHeader` (`repr(C)`)
followed by variable-length `report_data` and an opaque platform
attestation `quote`. The `TeePlatform` enum identifies hardware
(SGX=0, SEV-SNP=1, TDX=2, ARM CCA=3), and quote contents are
platform-specific bytes verified through the `QuoteVerifier` trait.

The witness chain binds each attestation record via
`action_hash = SHAKE-256-256(record)`, ensuring tamper-evident linkage.

#### Key Properties

| Property | Mechanism |
|----------|-----------|
| Platform identity | `AttestationHeader.measurement` (MRENCLAVE / launch digest) |
| Anti-replay | `AttestationHeader.nonce` (caller-provided, 16 bytes) |
| Debug detection | `FLAG_DEBUGGABLE` (bit 0 of attestation flags) |
| Key sealing | `TeeBoundKeyRecord` in CRYPTO_SEG with measurement binding |
| no_std support | All types compile without std (TEE-compatible) |
| CI testing | `SoftwareTee` platform variant (0xFE) for synthetic quotes |

### Cryptographic Key Authority

RVF defines two signing algorithms with distinct roles:

| Algorithm | Use Case | When Required |
|-----------|----------|---------------|
| **Ed25519** | Developer iteration, local trust, fast signing | Default for development builds, CI, internal distribution |
| **ML-DSA-65** (FIPS 204) | Long-lived artifacts, public distribution, post-quantum resistance | Required for published releases and any file with `REQUIRES_PQ` flag |

Trust root rotation: A `SignatureFooter` MAY contain dual signatures (Ed25519 + ML-DSA-65) to support migration periods. Verifiers accept either signature during migration; after a declared cutover date, only ML-DSA-65 is accepted for files with `REQUIRES_PQ` set.

The canonical trust chain for public artifacts is:
1. Signing key → signs CRYPTO_SEG → covers all data segments
2. Kernel signing key → signs KERNEL_SEG → covers boot image (ADR-030)
3. TEE measurement → binds both to hardware attestation quote

### Segment Type Registry (Implemented)

All segment types below are implemented in `rvf-types/src/segment_type.rs` with `TryFrom<u8>` round-trip support and unit tests (23 variants total):

| Value | Name | Description | Source |
|-------|------|-------------|--------|
| 0x00 | Invalid | Uninitialized / zeroed region | Core |
| 0x01 | Vec | Raw vector payloads (embeddings) | Core |
| 0x02 | Index | HNSW adjacency lists, entry points, routing tables | Core |
| 0x03 | Overlay | Graph overlay deltas, partition updates, min-cut witnesses | Core |
| 0x04 | Journal | Metadata mutations (label changes, deletions, moves) | Core |
| 0x05 | Manifest | Segment directory, hotset pointers, epoch state | Core |
| 0x06 | Quant | Quantization dictionaries and codebooks | Core |
| 0x07 | Meta | Arbitrary key-value metadata (tags, provenance, lineage) | Core |
| 0x08 | Hot | Temperature-promoted hot data (vectors + neighbors) | Core |
| 0x09 | Sketch | Access counter sketches for temperature decisions | Core |
| 0x0A | Witness | Capability manifests, proof of computation, audit trails | Core |
| 0x0B | Profile | Domain profile declarations (RVDNA, RVText, etc.) | Core |
| 0x0C | Crypto | Key material, signature chains, certificate anchors | Core |
| 0x0D | MetaIdx | Metadata inverted indexes for filtered search | Core |
| 0x0E | Kernel | Embedded kernel / unikernel image for self-booting | ADR-030 |
| 0x0F | Ebpf | Embedded eBPF program for kernel fast path | ADR-030 |
| 0x10 | Wasm | Embedded WASM bytecode for self-bootstrapping | ADR-030/032 |
| 0x20 | CowMap | COW cluster mapping | ADR-031 |
| 0x21 | Refcount | Cluster reference counts | ADR-031 |
| 0x22 | Membership | Vector membership filter | ADR-031 |
| 0x23 | Delta | Sparse delta patches | ADR-031 |
| 0x30 | TransferPrior | Cross-domain posterior summaries + cost EMAs | Domain expansion |
| 0x31 | PolicyKernel | Policy kernel configuration and performance history | Domain expansion |
| 0x32 | CostCurve | Cost curve convergence data for acceleration tracking | Domain expansion |

Available ranges: 0x11-0x1F, 0x24-0x2F, 0x33-0xEF. Values 0xF0-0xFF are reserved.

## Consequences

### Benefits

1. **Single format everywhere** — Any tool can read any RuVector data file
2. **Progressive loading** — First query in <5ms, full quality in seconds
3. **Crash safety for free** — Append-only + segment hashes, no WAL needed
4. **Hardware portability** — Same format on WASM tile and server
5. **Post-quantum ready** — ML-DSA-65 signatures from day one
6. **Self-optimizing** — Temperature tiering adapts to workload automatically
7. **Ecosystem coherence** — All libraries share indexing, quantization, crypto code
8. **Confidential Computing** — Hardware TEE attestation built into the format with platform-agnostic abstraction

### Write Atomicity Invariant

A segment is committed if and only if:
1. Its complete header (64 bytes) and payload are present on disk
2. The content hash in the header matches the payload bytes
3. The Level 0 manifest pointer has been updated to reference it

The two-fsync protocol enforces this: first fsync commits the segment data, second fsync commits the manifest update. A crash between fsyncs leaves the segment orphaned but the manifest consistent — the segment is invisible until the next successful manifest write. This is the write invariant that makes "crash safe without WAL" precise.

### Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Migration disrupts existing users | Medium | Medium | Provide rvf-import tools for each legacy format |
| RVF overhead for small datasets | Low | Low | Core Profile keeps overhead minimal (<1 KB) |
| Spec complexity delays implementation | Medium | High | Phase implementation (see guidance doc) |
| WASM binary size for microkernel | Low | Low | Budget verified at ~5.5 KB (within 8 KB) |

The WASM microkernel binary size MUST be verified in CI as an acceptance test. The current budget is 8 KB maximum. A CI job runs `wasm-opt -Oz` on the output and asserts `stat -c %s < 8192`. Any commit that exceeds this budget fails the build.

### Performance Targets (from RVF acceptance tests)

| Metric | Target |
|--------|--------|
| Cold boot | <5 ms (4 KB read) |
| First query recall@10 | >= 0.70 |
| Full quality recall@10 | >= 0.95 |
| Query latency p50 (10M vectors) | 0.1-0.3 ms |
| Streaming ingest (NVMe) | 200K-500K vectors/s |
| WASM query latency p50 | <3 ms |

### DNA-Style Lineage Provenance

RVF files carry a `FileIdentity` (68 bytes) in the Level0Root reserved area
at offset 0xF00, enabling provenance chains across file generations. This is
fully backward compatible — old readers see zeros in the reserved area and
continue working normally.

```
Parent.rvf ──derive()──> Child.rvf ──derive()──> Grandchild.rvdna
  file_id: A               file_id: B               file_id: C
  parent_id: [0;16]         parent_id: A              parent_id: B
  parent_hash: [0;32]       parent_hash: hash(A)      parent_hash: hash(B)
  depth: 0                  depth: 1                  depth: 2
```

#### Lineage Types

| Type | Description |
|------|-------------|
| `FileIdentity` (68B) | file_id[16] + parent_id[16] + parent_hash[32] + depth(u32) |
| `LineageRecord` (128B) | Full derivation record with description for witness chains |
| `DerivationType` (u8) | Clone=0, Filter=1, Merge=2, Quantize=3, Reindex=4, Transform=5, Snapshot=6 |

#### Witness Integration

Lineage events are recorded in WITNESS_SEG with new type codes:

| Witness Type | Code | Purpose |
|-------------|------|---------|
| DERIVATION | 0x09 | File derived from parent |
| LINEAGE_MERGE | 0x0A | Multi-parent merge |
| LINEAGE_SNAPSHOT | 0x0B | Point-in-time snapshot |
| LINEAGE_TRANSFORM | 0x0C | Arbitrary transformation |
| LINEAGE_VERIFY | 0x0D | Lineage chain verification |

#### Extension Aliasing

`.rvdna` is an alternative extension for RVF files using `DomainProfile::Rvdna`.
The authoritative profile lives in the `Level0Root.profile_id` byte; extensions
serve as hints for tooling and file managers.

| Extension | Profile | Domain |
|-----------|---------|--------|
| `.rvf` | Generic | General-purpose vectors |
| `.rvdna` | Rvdna | Genomics (codon, k-mer, motif embeddings) |
| `.rvtext` | RvText | Language (sentence, document embeddings) |
| `.rvgraph` | RvGraph | Graph (node, edge, subgraph embeddings) |
| `.rvvis` | RvVision | Vision (patch, image, object embeddings) |

### Quantum Vector Space Optimizations

RVF is designed to be quantum-ready at the storage layer. Quantum vector space
optimizations extend the format's utility for quantum-classical hybrid workloads:

1. **Quantum State Vectors**: RVF's VEC_SEG natively supports complex-valued
   vectors (fp32 pairs) for storing quantum state amplitudes. The `DataType`
   enum accommodates complex64 and complex128 types.

2. **Hilbert Space Indexing**: HNSW layers in INDEX_SEG can index over
   quantum fidelity metrics (trace distance, Bures distance) via pluggable
   distance functions in the runtime's `DistanceMetric` trait.

3. **Quantum Error Correction Metadata**: META_SEG stores syndrome tables,
   stabilizer codes, and logical-physical qubit mappings alongside vectors,
   enabling QEC-aware retrieval.

4. **Tensor Product Decomposition**: RVF segments support factored storage
   where large quantum state vectors are stored as tensor products of smaller
   sub-vectors, reducing storage from O(2^n) to O(n * 2^k) for k-local states.

5. **Post-Quantum Cryptographic Signatures**: ML-DSA-65 (Dilithium) signatures
   in CRYPTO_SEG ensure quantum-resistant integrity verification.

6. **Variational Quantum Eigensolver (VQE) Snapshots**: SKETCH_SEG stores
   parameterized circuit snapshots and their corresponding expectation values,
   enabling efficient VQE optimization history retrieval.

### RuVLLM Integration

RVF serves as the native storage format for RuVLLM (RuVector Large Language
Model) inference and fine-tuning pipelines:

1. **KV-Cache Persistence**: RVF segments store attention key-value caches
   for LLM inference resumption. VEC_SEG holds projected K/V matrices with
   per-layer segment tagging, enabling instant context restoration.

2. **Embedding Store**: Model embedding tables (token, position, type) are
   stored as RVF VEC_SEGs with HNSW indexing for semantic token retrieval
   and vocabulary expansion experiments.

3. **LoRA Adapter Storage**: Low-rank adaptation matrices are stored as
   compact VEC_SEGs with quantization (int4/int8 via QUANT_SEG), enabling
   efficient adapter switching during multi-tenant inference.

4. **Activation Checkpointing**: Intermediate activations during gradient
   computation are stored as temperature-tiered RVF segments — hot layers
   in HOT_SEG, cold layers in standard VEC_SEG — with automatic promotion.

5. **Prompt Cache / RAG Store**: Retrieval-augmented generation corpora are
   RVF files with RVText profile, enabling sub-millisecond semantic search
   over cached prompt-response pairs with lineage tracking.

6. **Model Provenance**: Lineage chains track model derivation — base model
   → fine-tuned → quantized → deployed — with cryptographic hashes ensuring
   the exact model lineage is verifiable.

## File Extension

- `.rvf` — RuVector Format file (Generic profile)
- `.rvdna` — Genomics domain (Rvdna profile)
- `.rvtext` — Language/text domain (RvText profile)
- `.rvgraph` — Graph/network domain (RvGraph profile)
- `.rvvis` — Vision/imagery domain (RvVision profile)
- `.rvf.cold.N` — Cold shard N (multi-file mode)
- `.rvf.idx.N` — Index shard N (multi-file mode)

## MIME Type

- `application/x-ruvector-format` (pending IANA registration)

## Related Decisions

- **ADR-001**: Core architecture (storage layer superseded by RVF)
- **ADR-003**: SIMD optimization (RVF adopts 64-byte alignment strategy)
- **ADR-005**: WASM runtime (RVF microkernel replaces ad-hoc WASM builds)
- **ADR-006**: Memory management (RVF segment model replaces custom arena)
- **ADR-018**: Block-based storage (RVF VEC_SEG block model supersedes)
- **ADR-021**: Delta compression (RVF OVERLAY_SEG adopts delta approach)
- **RVF Spec**: `docs/research/rvf/` (full specification)

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-13 | ruv.io | Initial adoption decision |
| 1.1 | 2026-02-16 | implementation review | Added complete segment type registry documenting all 23 implemented variants including Wasm (0x10), COW segments (0x20-0x23), and domain expansion segments (0x30-0x32). All types have `TryFrom<u8>` round-trip tests in rvf-types. |
