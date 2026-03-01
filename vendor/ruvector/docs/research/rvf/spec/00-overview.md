# RVF: RuVector Format Specification

## The Universal Substrate for Living Intelligence

**Version**: 0.1.0-draft
**Status**: Research
**Date**: 2026-02-13

---

## What RVF Is

RVF is not a file format. It is a **runtime substrate** — a living, self-reorganizing
binary medium that stores, streams, indexes, and adapts vector intelligence across
any domain, any scale, and any hardware tier.

Where traditional formats are snapshots of data, RVF is a **continuously evolving
organism**. It ingests without rewriting. It answers queries before it finishes loading.
It reorganizes its own layout to match access patterns. It survives crashes without
journals. It fits on a 64 KB WASM tile or scales to a petabyte hub.

## The Four Laws of RVF

Every design decision in RVF derives from four inviolable laws:

### Law 1: Truth Lives at the Tail

The most recent `MANIFEST_SEG` at the tail of the file is the sole source of truth.
No front-loaded metadata. No section directory that must be rewritten on mutation.
Readers scan backward from EOF to find the latest manifest and know exactly what
to map.

**Consequence**: Append-only writes. Streaming ingest. No global rewrite ever.

### Law 2: Every Segment Is Independently Valid

Each segment carries its own magic number, length, content hash, and type tag.
A reader encountering any segment in isolation can verify it, identify it, and
decide whether to process it. No segment depends on prior segments for structural
validity.

**Consequence**: Crash safety for free. Parallel verification. Segment-level
integrity without a global checksum.

### Law 3: Data and State Are Separated

Vector payloads, index structures, overlay graphs, quantization dictionaries, and
runtime metadata live in distinct segment types. The manifest binds them together
but they never intermingle. This means you can replace the index without touching
vectors, update the overlay without rebuilding adjacency, or swap quantization
without re-encoding.

**Consequence**: Incremental updates. Modular evolution. Zero-copy segment reuse.

### Law 4: The Format Adapts to Its Workload

RVF monitors access patterns through lightweight sketches and periodically
reorganizes: promoting hot vectors to faster tiers, compacting stale overlays,
lazily building deeper index layers. The format is not static — it converges
toward the optimal layout for its actual workload.

**Consequence**: Self-tuning performance. No manual optimization. The file gets
faster the more you use it.

## Design Coordinates

| Property | RVF Answer |
|----------|-----------|
| Write model | Append-only segments + background compaction |
| Read model | Tail-manifest scan, then progressive mmap |
| Index model | Layered availability (entry points -> partial -> full) |
| Compression | Temperature-tiered (fp16 hot, 5-7 bit warm, 3 bit cold) |
| Alignment | 64-byte for SIMD (AVX-512, NEON, WASM v128) |
| Crash safety | Segment-level hashes, no WAL required |
| Crypto | Post-quantum (ML-DSA-65 signatures, SHAKE-256 hashes) |
| Streaming | Yes — first query before full load |
| Hardware | 8 KB tile to petabyte hub |
| Domain | Universal — genomics, text, graph, vision as profiles |

## Acceptance Test

> Cold start on a 10 million vector file: load and answer the first query with a
> useful (recall >= 0.7) result without reading more than the last 4 MB, then
> converge to full quality (recall >= 0.95) as it progressively maps more segments.

## Document Map

| Document | Path | Content |
|----------|------|---------|
| This overview | `spec/00-overview.md` | Philosophy, laws, design coordinates |
| Segment model | `spec/01-segment-model.md` | Segment types, headers, append-only rules |
| Manifest system | `spec/02-manifest-system.md` | Two-level manifests, hotset pointers |
| Temperature tiering | `spec/03-temperature-tiering.md` | Adaptive layout, access sketches, promotion |
| Progressive indexing | `spec/04-progressive-indexing.md` | Layered HNSW, partial availability |
| Overlay epochs | `spec/05-overlay-epochs.md` | Streaming min-cut, epoch boundaries |
| Wire format | `wire/binary-layout.md` | Byte-level binary format reference |
| WASM microkernel | `microkernel/wasm-runtime.md` | Cognitum tile mapping, WASM exports |
| Domain profiles | `profiles/domain-profiles.md` | RVDNA, RVText, RVGraph, RVVision |
| Crypto spec | `crypto/quantum-signatures.md` | Post-quantum primitives, segment signing |
| Benchmarks | `benchmarks/acceptance-tests.md` | Performance targets, test methodology |

## Relationship to RVDNA

RVDNA (RuVector DNA) was the first domain-specific format for genomic vector
intelligence. In the RVF model, RVDNA becomes a **profile** — a set of conventions
for how genomic data maps onto the universal RVF substrate:

```
RVF (universal substrate)
  |
  +-- RVF Core Profile    (minimal, fits on 64KB tile)
  +-- RVF Hot Profile      (chip-optimized, SIMD-heavy)
  +-- RVF Full Profile     (hub-scale, all features)
  |
  +-- Domain Profiles
       +-- RVDNA           (genomics: codons, motifs, k-mers)
       +-- RVText          (language: embeddings, token graphs)
       +-- RVGraph         (networks: adjacency, partitions)
       +-- RVVision        (imagery: feature maps, patch vectors)
```

The substrate carries the laws. The profiles carry the semantics.

## Design Answers

**Q: Random writes or append-only plus compaction?**
A: Append-only plus compaction. This gives speed and crash safety almost for free.
Random writes add complexity for marginal benefit in the vector workload.

**Q: Primary target mmap on desktop CPUs or also microcontroller tiles?**
A: Both. RVF defines three hardware profiles. The Core profile fits in 8 KB code +
8 KB data + 64 KB SIMD scratch. The Full profile assumes mmap on desktop-class
memory. The wire format is identical — only the runtime behavior changes.

**Q: Which property matters most?**
A: All four are non-negotiable, but the priority order for conflict resolution is:
1. **Streamable** (never block on write)
2. **Progressive** (answer before fully loaded)
3. **Adaptive** (self-optimize over time)
4. **p95 speed** (predictable tail latency)
