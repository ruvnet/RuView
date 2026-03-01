# RVF: RuVector Format

## A Living, Self-Reorganizing Runtime Substrate for Vector Intelligence

---

### Document Index

#### Core Specification (`spec/`)

| # | Document | Description |
|---|----------|-------------|
| 00 | [Overview](spec/00-overview.md) | The Four Laws, design coordinates, philosophy |
| 01 | [Segment Model](spec/01-segment-model.md) | Append-only segments, headers, lifecycle, multi-file |
| 02 | [Manifest System](spec/02-manifest-system.md) | Two-level manifests, hotset pointers, progressive boot |
| 03 | [Temperature Tiering](spec/03-temperature-tiering.md) | Adaptive layout, access sketches, promotion/demotion |
| 04 | [Progressive Indexing](spec/04-progressive-indexing.md) | Layer A/B/C availability, lazy build, partial search |
| 05 | [Overlay Epochs](spec/05-overlay-epochs.md) | Streaming min-cut, epoch boundaries, rollback |
| 06 | [Query Optimization](spec/06-query-optimization.md) | SIMD alignment, prefetch, varint IDs, cache analysis |
| 07 | [Deletion & Lifecycle](spec/07-deletion-lifecycle.md) | Vector deletion, JOURNAL_SEG wire format, deletion bitmaps, compaction |
| 08 | [Filtered Search](spec/08-filtered-search.md) | META_SEG wire format, filter expressions, metadata indexes |
| 09 | [Concurrency & Versioning](spec/09-concurrency-versioning.md) | Writer locking, reader-writer coordination, space reclamation |
| 10 | [Operations API](spec/10-operations-api.md) | Batch ops, error codes, network streaming, compaction scheduling |

#### Wire Format (`wire/`)

| Document | Description |
|----------|-------------|
| [Binary Layout](wire/binary-layout.md) | Byte-level format reference, all segment payloads |

#### WASM Microkernel (`microkernel/`)

| Document | Description |
|----------|-------------|
| [WASM Runtime](microkernel/wasm-runtime.md) | Cognitum tile mapping, 14 exports, hub-tile protocol |

#### Domain Profiles (`profiles/`)

| Document | Description |
|----------|-------------|
| [Domain Profiles](profiles/domain-profiles.md) | RVDNA, RVText, RVGraph, RVVision specifications |

#### Cryptography (`crypto/`)

| Document | Description |
|----------|-------------|
| [Quantum Signatures](crypto/quantum-signatures.md) | ML-DSA-65, SHAKE-256, hybrid encryption, witnesses |

#### Benchmarks (`benchmarks/`)

| Document | Description |
|----------|-------------|
| [Acceptance Tests](benchmarks/acceptance-tests.md) | Performance targets, crash safety, scalability |

---

### Quick Reference

**The Four Laws**
1. Truth lives at the tail
2. Every segment is independently valid
3. Data and state are separated
4. The format adapts to its workload

**Minimal Upgrade Path** (smallest changes that unlock everything)
1. Add tail manifest segments
2. Make every payload a segment with its own hash and length
3. Add hotset pointers in the manifest
4. Add an epoch overlay model

**Hardware Profiles**
- **Core**: 8 KB code + 8 KB data + 64 KB SIMD (Cognitum tile)
- **Hot**: Multi-tile chip with shared memory
- **Full**: Desktop/server with mmap and full feature set

**Key Numbers**
- Boot: 4 KB read, < 5 ms
- First query: <= 4 MB read, recall >= 0.70
- Full quality: recall >= 0.95
- Signing: ML-DSA-65, 3,309 B signatures, ~4,500 sign/s
- Distance: 384-dim fp16 L2 in ~12 AVX-512 cycles
- Hot entry: 960 bytes (vector + 16 neighbors, cache-line aligned)

**Design Choices**
- Append-only + compaction (not random writes)
- Both mmap desktop and microcontroller tiles
- Priority: streamable > progressive > adaptive > p95 speed
