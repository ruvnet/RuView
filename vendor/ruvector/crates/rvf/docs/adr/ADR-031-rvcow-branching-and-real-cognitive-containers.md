# ADR-031: Vector-Native COW Branching (RVCOW) and Real Cognitive Containers

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-02-14 |
| **Deciders** | RuVector Core Team |
| **Supersedes** | — |
| **Extends** | ADR-030 (Cognitive Containers) |

---

## 1. Context and Problem Statement

RVF files today are immutable after initial write. To create a "branch" — e.g., a LoRA fine-tune overlay, a filtered subset, or a user-specific shard — the entire file must be copied. For a 1M-vector file at dimension 128 in f32, that's ~512 MB of redundant data even when only 100 vectors changed.

Separately, ADR-030 introduced `KERNEL_SEG` (0x0E) and `EBPF_SEG` (0x0F) with complete type systems (`KernelHeader` at 128 bytes, `EbpfHeader` at 64 bytes, 15 kernel flags, architecture/transport enums). However, the actual kernel images and eBPF programs embedded today are **stub artifacts** — test payloads rather than production binaries. The `serve` CLI command is a stub that prints "not yet implemented."

This ADR addresses both problems in a single coordinated design:

1. **RVCOW** — Vector-native copy-on-write branching at cluster granularity, with SIMD-aligned slabs, delta encoding for LoRA, membership filters for shared HNSW indexes, and provenance tracking per cluster.

2. **Real Cognitive Containers** — Replace all stub artifacts with production binaries: a custom MicroLinux kernel, QEMU microVM launcher, real eBPF programs compiled from C, and a working `serve` command.

### Why Ship Together (Phased)

RVCOW's `KernelBinding` footer ties the kernel's signed data to the manifest root hash, which now includes the COW map. The wire format change for KernelBinding is introduced now with RVCOW, but real kernel artifacts (MicroLinux, QEMU launcher, eBPF programs) are phased — they ship after the COW runtime stabilizes. This avoids two separate breaking changes to the KERNEL_SEG wire format while reducing coupling in the execution plan.

---

## 2. Decision Drivers

- **Storage efficiency**: Derived files should store only the delta, not a full copy.
- **Query performance**: COW reads must not degrade latency by more than 10% vs. monolithic files.
- **SIMD alignment**: All cluster boundaries must align to 64-byte AVX-512 / cache-line boundaries.
- **Crash safety**: Mid-write failures must never corrupt the parent file or leave the child unreadable.
- **Security**: Kernel images must be cryptographically bound to the manifest they serve; swapping segments between files must fail verification.
- **Forward compatibility**: Existing tools that don't understand COW segments must still read the file (unknown segments are preserved).
- **Incremental adoption**: RVCOW ships first as a pure types+runtime change; real kernel/eBPF artifacts ship second as an additive layer.

---

## 3. Considered Options

### 3.1 COW Strategy

| Option | Description | Verdict |
|--------|-------------|---------|
| **A. Immutable COW with generations** | Every write creates a new generation; old generations frozen | Rejected — excessive GC pressure |
| **B. Mutable COW with snapshot-freeze** | Writes mutate in-place; explicit freeze creates immutable snapshot | **Accepted** — matches Git/QCOW2 model |
| **C. Log-structured merge** | All writes go to a WAL; periodic merge into base | Rejected — poor random-read latency |

### 3.2 Shared HNSW Index Strategy

| Option | Description | Verdict |
|--------|-------------|---------|
| **A. Membership filter at query time** | Base HNSW shared; excluded nodes used for routing only, never returned | **Accepted** — zero index rebuild cost |
| **B. Separate HNSW per branch** | Each branch builds its own index | Rejected — O(N log N) per branch |
| **C. Lazy index patching** | Patch base HNSW in-place per branch | Rejected — complex, error-prone |

### 3.3 Refcount Management

| Option | Description | Verdict |
|--------|-------------|---------|
| **A. WAL-backed refcounts** | Every COW increment/decrement logged to WAL | Rejected — write amplification |
| **B. Rebuildable metadata** | Refcounts derived from COW map chain; recomputed during compaction | **Accepted** — simpler, no WAL needed |

### 3.4 Kernel Binding Mechanism

| Option | Description | Verdict |
|--------|-------------|---------|
| **A. Command-line parameter** | Pass manifest hash via kernel cmdline | Rejected — cmdline easily stripped/modified |
| **B. Signed footer in KERNEL_SEG** | 128-byte `KernelBinding` (padded) included in signed_data | **Accepted** — tamper-evident, part of signature |

---

## 4. Decision

We adopt **mutable COW with snapshot-freeze** using **four new segment types** (0x20-0x23) and a **KernelBinding footer** (128 bytes, padded) for the cognitive container. The complete design follows.

---

## 5. Segment ID Registry

### 5.1 Current Allocation (ADR-001 through ADR-030)

```
0x00  Invalid       — Uninitialized/zeroed region
0x01  Vec           — Raw vector payloads
0x02  Index         — HNSW adjacency lists
0x03  Overlay       — Graph overlay deltas
0x04  Journal       — Metadata mutations
0x05  Manifest      — Segment directory, epoch state
0x06  Quant         — Quantization dictionaries
0x07  Meta          — Key-value metadata
0x08  Hot           — Temperature-promoted data
0x09  Sketch        — Access counter sketches
0x0A  Witness       — Audit trails, proofs
0x0B  Profile       — Domain profile declarations
0x0C  Crypto        — Key material, signatures
0x0D  MetaIdx       — Metadata inverted indexes
0x0E  Kernel        — Embedded kernel image (ADR-030)
0x0F  Ebpf          — Embedded eBPF program (ADR-030)
```

### 5.2 New Allocation (This ADR)

```
0x10-0x1F  Reserved    — Witness subtypes, future near-compute segments
0x20  CowMap          — Copy-on-write cluster mapping (NEW)
0x21  Refcount        — Cluster reference counts (NEW)
0x22  Membership      — Vector membership filter for shared HNSW (NEW)
0x23  Delta           — Sparse delta patches for clusters (NEW)
0x24-0xEF  Available  — Future growth
0xF0-0xFF  Reserved   — Format-level reserved
```

### 5.3 Witness Event Types (New)

```
0x0E  CLUSTER_COW     — Full slab copy event
0x0F  CLUSTER_DELTA   — Delta patch event
0x10  CLUSTER_REVERT  — Compaction reverted local slab to ParentRef (hash match)
```

These extend the existing witness type namespace (0x01-0x0D already assigned by prior ADRs).

`CLUSTER_REVERT` is emitted during compaction (Mode 2: Space Reclaim) when a local slab is found to be identical to the parent slab. Without this event, the provenance chain would have a silent state change — the slab disappears without audit trail.

---

## 6. RVCOW Type Definitions

### 6.1 COW_MAP_SEG (0x20) — `CowMapHeader`

The COW map is the core data structure that maps cluster IDs to their physical locations. A "cluster" is a contiguous block of vectors sized for SIMD processing.

**Cluster addressing**: `cluster_id = vector_id / vectors_per_cluster`

**Default cluster sizes**:
- 256 KB for f32/fp16 (general workloads)
- 128 KB for frequent-edit workloads (LoRA)
- 512 KB for read-heavy/archival workloads

```
CowMapHeader (64 bytes, repr(C)):
┌──────────────────────┬──────┬──────────────────────────────────────────────┐
│ Offset │ Type        │ Size │ Field                                        │
├────────┼─────────────┼──────┼──────────────────────────────────────────────┤
│ 0x00   │ u32         │  4   │ magic = 0x5256_434D ("RVCM")                │
│ 0x04   │ u16         │  2   │ version                                      │
│ 0x06   │ u8          │  1   │ map_format (0=flat_array, 1=art_tree,        │
│        │             │      │             2=extent_list)                    │
│ 0x07   │ u8          │  1   │ compression_policy                           │
│ 0x08   │ u32         │  4   │ cluster_size_bytes (power of 2, SIMD-aligned)│
│ 0x0C   │ u32         │  4   │ vectors_per_cluster                          │
│ 0x10   │ [u8; 16]    │ 16   │ base_file_id (UUID of parent file)           │
│ 0x20   │ [u8; 32]    │ 32   │ base_file_hash (SHAKE-256-256 of parent      │
│        │             │      │  Level0Root manifest)                        │
│ 0x40   │ u64         │  8   │ map_root_offset (offset to map data)         │
│ 0x48   │ u32         │  4   │ cluster_count (total clusters in base)       │
│ 0x4C   │ u32         │  4   │ local_cluster_count (clusters stored locally)│
│ 0x50   │ u8          │  1   │ extent_support (1=extents enabled)           │
│ 0x51   │ [u8; 3]     │  3   │ _reserved (must be zero)                     │
│ 0x54   │ [u8; 12]    │ 12   │ _reserved2 (must be zero)                    │
└────────┴─────────────┴──────┴──────────────────────────────────────────────┘
Total: 64 bytes (compile-time assertion required)
```

**Lookup contract**:
```
cluster_id → LocalOffset(u64)   // cluster data stored in this file
           | ParentRef          // cluster lives in parent; follow chain
           | Unallocated        // cluster not yet assigned
```

**Invariants**:
- Map is **deterministic** — cluster IDs must not be reordered.
- Map must be **memory-mappable** — fixed-size pages for mmap + prefetch.
- **Extent support**: Sequential runs of local clusters stored as single entries → O(extents) not O(clusters).

**ART (Adaptive Radix Tree) implementation** (when `map_format=1`):
- Fixed-size 256-byte nodes for direct mmap.
- Path compression for sparse maps.
- Extent entries for sequential runs.
- Prefetch hints at inner nodes for NVMe latency hiding.

**Source files**:
- Types: `rvf-types/src/cow_map.rs` (~200 lines)
- Runtime: `rvf-runtime/src/cow_map.rs` (ART implementation)

### 6.2 REFCOUNT_SEG (0x21) — `RefcountHeader`

Refcounts track how many derived files reference each cluster in a base file. They are **rebuildable metadata** — derived from the COW map chain, never edited in-place, and recomputed during compaction.

```
RefcountHeader (32 bytes, repr(C)):
┌──────────────────────┬──────┬──────────────────────────────────────────────┐
│ Offset │ Type        │ Size │ Field                                        │
├────────┼─────────────┼──────┼──────────────────────────────────────────────┤
│ 0x00   │ u32         │  4   │ magic = 0x5256_5243 ("RVRC")                │
│ 0x04   │ u16         │  2   │ version                                      │
│ 0x06   │ u8          │  1   │ refcount_width (1, 2, or 4 bytes per entry)  │
│ 0x07   │ u8          │  1   │ _pad (must be zero)                          │
│ 0x08   │ u32         │  4   │ cluster_count                                │
│ 0x0C   │ u32         │  4   │ max_refcount (highest value in array)        │
│ 0x10   │ u64         │  8   │ array_offset (offset to refcount array)      │
│ 0x18   │ u32         │  4   │ snapshot_epoch (0=mutable, >0=frozen)        │
│ 0x1C   │ u32         │  4   │ _reserved (must be zero)                     │
└────────┴─────────────┴──────┴──────────────────────────────────────────────┘
Total: 32 bytes (compile-time assertion required)
```

**Rebuild algorithm**: Walk the COW map chain from all known derived files → count references per cluster → write new REFCOUNT_SEG. This happens during compaction only.

**Source file**: `rvf-types/src/refcount.rs` (~120 lines)

### 6.3 MEMBERSHIP_SEG (0x22) — `MembershipHeader`

The membership filter controls which vectors are visible when querying a shared HNSW index. This enables Strategy A: the base HNSW is shared across branches; the filter decides which results to return.

```
MembershipHeader (96 bytes, repr(C)):
┌──────────────────────┬──────┬──────────────────────────────────────────────┐
│ Offset │ Type        │ Size │ Field                                        │
├────────┼─────────────┼──────┼──────────────────────────────────────────────┤
│ 0x00   │ u32         │  4   │ magic = 0x5256_4D42 ("RVMB")                │
│ 0x04   │ u16         │  2   │ version                                      │
│ 0x06   │ u8          │  1   │ filter_type (0=bitmap, 1=roaring_bitmap)     │
│ 0x07   │ u8          │  1   │ filter_mode (0=include [default], 1=exclude) │
│ 0x08   │ u64         │  8   │ vector_count (total vectors in base)         │
│ 0x10   │ u64         │  8   │ member_count (vectors matching filter)       │
│ 0x18   │ u64         │  8   │ filter_offset (offset to filter data)        │
│ 0x20   │ u32         │  4   │ filter_size (size of filter data in bytes)   │
│ 0x24   │ u32         │  4   │ generation_id (monotonic, anti-replay)       │
│ 0x28   │ [u8; 32]    │ 32   │ filter_hash (SHAKE-256-256 of filter data)   │
│ 0x48   │ u64         │  8   │ bloom_offset (optional bloom accelerator,    │
│        │             │      │  0=none)                                     │
│ 0x50   │ u32         │  4   │ bloom_size                                   │
│ 0x54   │ u32         │  4   │ _reserved (must be zero)                     │
│ 0x58   │ [u8; 8]     │  8   │ _reserved2 (must be zero)                    │
└────────┴─────────────┴──────┴──────────────────────────────────────────────┘
Total: 96 bytes (compile-time assertion required)
```

**Include mode** (default, `filter_mode=0`):
- A vector is visible iff `filter.contains(vector_id)`.
- **Empty filter = empty view** (fail-safe). This prevents accidental full-scan on uninitialized filters.

**Exclude mode** (`filter_mode=1`):
- A vector is visible iff `!filter.contains(vector_id)`.
- Empty filter = full view.

**Anti-replay**: `generation_id` is monotonically increasing. Enforcement rules:

**On open** (enforced by runtime, not advisory):
```
if Level0Root.membership_generation > MembershipHeader.generation_id:
    return Err(MembershipInvalid)  // stale filter, refuse to use
```

**On update** (enforced by runtime):
```
new_generation_id must be strictly > Level0Root.membership_generation
old MEMBERSHIP_SEG is preserved in the segment directory (superseded, not deleted)
Level0Root.membership_generation updated to new_generation_id
```

**Same rules apply to `cow_map_generation`**:
```
On open:  Level0Root.cow_map_generation > CowMapHeader version → CowMapCorrupt
On update: new generation must strictly increase; old COW_MAP_SEG preserved
```

This prevents replay attacks and makes L0 cache invalidation safe — any cached generation less than the manifest generation is stale and must be evicted.

**Source file**: `rvf-types/src/membership.rs` (~150 lines)

### 6.4 DELTA_SEG (0x23) — `DeltaHeader`

Delta segments store sparse patches for clusters where only a few vectors changed. This is the key primitive for LoRA overlays.

```
DeltaHeader (64 bytes, repr(C)):
┌──────────────────────┬──────┬──────────────────────────────────────────────┐
│ Offset │ Type        │ Size │ Field                                        │
├────────┼─────────────┼──────┼──────────────────────────────────────────────┤
│ 0x00   │ u32         │  4   │ magic = 0x5256_444C ("RVDL")                │
│ 0x04   │ u16         │  2   │ version                                      │
│ 0x06   │ u8          │  1   │ encoding (0=sparse_rows, 1=low_rank,         │
│        │             │      │           2=full_patch)                       │
│ 0x07   │ u8          │  1   │ _pad (must be zero)                          │
│ 0x08   │ u32         │  4   │ base_cluster_id                              │
│ 0x0C   │ u32         │  4   │ affected_count (number of modified vectors)  │
│ 0x10   │ u64         │  8   │ delta_size (bytes of delta payload)          │
│ 0x18   │ [u8; 32]    │ 32   │ delta_hash (SHAKE-256-256 of delta payload)  │
│ 0x38   │ [u8; 8]     │  8   │ _reserved (must be zero)                     │
└────────┴─────────────┴──────┴──────────────────────────────────────────────┘
Total: 64 bytes (compile-time assertion required)
```

**Delta policy** (hard rule at runtime — not a suggestion):
```
if changed_rows / vectors_per_cluster < delta_threshold (default 0.10):
    store DELTA_SEG (sparse delta)
else:
    full slab COW (copy entire cluster)
```

**Hot path guard** (hard rule — enforced, not advisory):
If `access_counter` for a cluster exceeds `hot_threshold`, delta segments for that cluster are **rejected** and the cluster is upgraded to a full slab copy on the next write. This prevents delta chains from degrading read performance on hot data.

**Encoding modes**:
- `sparse_rows` (0): Array of `(vector_offset, new_vector_data)` pairs.
- `low_rank` (1): Low-rank factorization for LoRA-style updates.
- `full_patch` (2): Complete replacement of affected rows (dense patch).

**Source file**: `rvf-types/src/delta.rs` (~100 lines)

---

## 7. KernelBinding Footer

### 7.1 Problem

Without binding, an attacker can:
1. Take a signed kernel from file A.
2. Embed it into file B (different vectors, different manifest).
3. The kernel boots and serves file B's data with file A's attestation.

### 7.2 Solution: Signed KernelBinding

A 128-byte `KernelBinding` footer is added to the KERNEL_SEG payload, positioned between the `KernelHeader` and the command line. It is included in the `signed_data` computation, making it tamper-evident.

The structure is padded to 128 bytes (from 68 bytes of active fields) to allow future evolution without another wire format change. Future fields (minimum runtime version, allowed segment mask, attestation policy hash, capability flags) will consume reserved space.

```
KernelBinding (128 bytes, repr(C)):
┌──────────────────────┬──────┬──────────────────────────────────────────────┐
│ Offset │ Type        │ Size │ Field                                        │
├────────┼─────────────┼──────┼──────────────────────────────────────────────┤
│ 0x00   │ [u8; 32]    │ 32   │ manifest_root_hash (SHAKE-256-256 of         │
│        │             │      │  Level0Root manifest bytes 0x000..0xFFB)     │
│ 0x20   │ [u8; 32]    │ 32   │ policy_hash (SHAKE-256-256 of security       │
│        │             │      │  policy: signing requirements, ACLs, etc.)   │
│ 0x40   │ u16         │  2   │ binding_version                              │
│ 0x42   │ u16         │  2   │ min_runtime_version (0=any, reserved)        │
│ 0x44   │ u32         │  4   │ allowed_segment_mask (reserved, must be 0)   │
│ 0x48   │ u32         │  4   │ capability_flags (reserved, must be 0)       │
│ 0x4C   │ [u8; 52]    │ 52   │ _reserved (must be zero — future evolution)  │
└────────┴─────────────┴──────┴──────────────────────────────────────────────┘
Total: 128 bytes (compile-time assertion required)
```

**Rationale for 128-byte padding**: Adding fields later (e.g., min_runtime_version, allowed_segment_mask, attestation policy hash, capability flags) would otherwise require another wire format break. The reserved space absorbs future growth at the cost of 60 bytes per KERNEL_SEG — negligible compared to kernel image sizes.

### 7.3 Updated KERNEL_SEG Wire Format

```
[SegmentHeader: 64 bytes]          — Standard RVF segment header (type=0x0E)
[KernelHeader: 128 bytes]          — Kernel metadata (ADR-030, unchanged)
[KernelBinding: 128 bytes]         — NEW: manifest + policy binding (padded)
[cmdline: cmdline_length bytes]    — Kernel command line (optional)
[compressed kernel image]          — Kernel binary
[Optional: SignatureFooter]        — Ed25519 or ML-DSA-65 signature
```

### 7.4 Updated Signed Data

```
signed_data = KernelHeader (128B) || KernelBinding (128B) || cmdline || compressed_image
```

This is a **breaking change** to the KERNEL_SEG payload format. Existing test stubs that don't include KernelBinding will fail signature verification if signed. Unsigned stubs remain readable (KernelBinding is validated only when signatures are present).

### 7.5 Launcher Verification Sequence

```
1. Extract KernelBinding from KERNEL_SEG payload
2. Compute SHAKE-256-256 of current Level0Root (bytes 0x000..0xFFB)
3. Compare computed hash to KernelBinding.manifest_root_hash
4. IF mismatch → REFUSE TO BOOT (segments may have been swapped)
5. IF signed → verify signature over signed_data
6. Boot kernel with verified manifest binding
```

**Source file**: `rvf-types/src/kernel_binding.rs` (~60 lines)

---

## 8. RVCOW Runtime Design

### 8.1 Read Path

```
query(vector_id) →
  1. cluster_id = vector_id / vectors_per_cluster
  2. Lookup cluster_id in ART map (or flat array / extent list)
  3. Result:
     a. LocalOffset(offset) → mmap local cluster, return pointer to vector
     b. ParentRef → follow parent chain (recursive read from parent file)
     c. Unallocated → error (vector does not exist in this branch)
  4. Distance compute on mapped slab (SIMD-aligned)
```

**Parent resolution contract** (explicit, enforced by runtime):

When a `ParentRef` is encountered, the runtime resolves the parent file using this priority order:
```
1. Explicit path stored in META_SEG key "parent_path" (if present)
2. Same directory: scan for file whose FileIdentity.file_id matches CowMapHeader.base_file_id
3. User-provided search paths (via RvfOptions.parent_search_paths)
4. Fail with ParentChainBroken (0x0702)
```

**Depth limit**: Parent chain traversal is capped at **64 levels**. If `lineage_depth > 64`, the runtime refuses to open the file and returns `ParentChainBroken`. This prevents cycles and malicious chains from causing unbounded recursion.

**L0 cache**: Resolved `cluster_id → final_offset` mappings cached in a lock-free hash map. After warmup, lookups take <100 ns. Cache entries are tagged with the generation counter and evicted when the generation changes.

**Performance contract**: COW read path adds at most one extra indirection (ART lookup) compared to monolithic read. Target: <10% latency overhead.

### 8.2 Write Path (Mutable COW)

```
mutate(vector_id, new_data) →
  1. cluster_id = vector_id / vectors_per_cluster
  2. Resolve current location via ART map
  3. IF inherited from parent (ParentRef):
     a. Allocate new local cluster (append to EOF)
     b. Copy parent slab → local slab (full cluster copy)
     c. Apply mutation to local slab
     d. Update ART map: cluster_id → LocalOffset(new_offset)
     e. Emit CLUSTER_COW witness event
  4. IF already local (LocalOffset):
     a. Apply mutation in-place to local slab
     b. (No witness event — already COW'd)
  5. Invalidate L0 cache entry for cluster_id
```

**Write coalescing**: When multiple writes target vectors in the same inherited cluster within a single batch, only **one** slab copy occurs. The runtime buffers pending writes per cluster, copies the parent slab once, applies all mutations, then commits.

**Acceptance test**: 10 writes to vectors in the same inherited slab = exactly 1 slab copy.

### 8.3 Write Atomicity and Crash Recovery

**Commit sequence** (ordered, each step depends on the previous):

```
Step 1: Write new slab data (append to EOF)
Step 2: Write slab hash (append)
Step 3: Write CLUSTER_COW witness event (append)
Step 4: fsync()
Step 5: Update Level0Root manifest (4096-byte atomic rewrite at fixed offset)
        — COW map root pointer stored in Level0Root reserved area
Step 6: fsync()
```

**Crash recovery semantics**:
- Crash before Step 4: Appended data is orphaned. No references in manifest. Invisible to readers.
- Crash during Step 5 (torn Level0Root write): **Double-root scheme** recovers.
- Crash after Step 6: Fully committed.

**Double-root scheme** (concrete layout and semantics):

Two Level0Root copies are stored at fixed, known offsets:

```
Root slot A: file_size - 8192  (offset EOF - 8192)
Root slot B: file_size - 4096  (offset EOF - 4096)

Each slot contains:
┌──────────┬──────┬──────────────────────────────────────────┐
│ Offset   │ Size │ Field                                    │
├──────────┼──────┼──────────────────────────────────────────┤
│ 0x000    │ 4092 │ Level0Root data (bytes 0x000..0xFFB)     │
│ 0xFFC    │    4 │ root_checksum (CRC32C of 0x000..0xFFB)  │
└──────────┴──────┴──────────────────────────────────────────┘

Cross-validation fields (inside Level0Root reserved area):
  double_root_generation: u64 at 0xF60  — monotonically increasing
  double_root_hash: [u8; 32] at 0xF68  — SHAKE-256-256 of the OTHER slot
```

**Read rule** (deterministic, auditable):
```
1. Read slot A and slot B
2. Validate each: CRC32C must pass AND magic must match
3. For each valid slot: verify double_root_hash matches
   SHAKE-256-256 of the OTHER slot's raw bytes (cross-check)
4. Pick the slot with the highest double_root_generation
   where CRC passes AND root_hash cross-check passes
5. If BOTH invalid → return DoubleRootCorrupt (0x0708)
6. If only one valid → use it (the other was mid-write)
```

**Write rule** (ordered, crash-safe):
```
1. Determine which slot has the LOWER generation (= older)
2. Write the NEW root into the OLDER slot with generation = max + 1
3. Compute double_root_hash = SHAKE-256-256 of the OTHER (still valid) slot
4. fsync()
5. Update the OTHER slot's double_root_hash to point to the newly written slot
6. Increment the OTHER slot's generation if needed
7. fsync()
```

At no point are both slots invalid simultaneously. A crash at any step leaves at least one slot with a valid CRC and a consistent generation counter.

**Acceptance test**: Simulate torn Level0Root write → verify fallback to previous valid root → child file readable and consistent.

### 8.4 HNSW Traversal Under Membership Filter

When querying a derived file that shares the parent's HNSW index but has a membership filter:

```rust
fn search_with_filter(
    query: &[f32],
    graph: &HnswGraph,
    membership: &MembershipFilter,
    ef: usize,
) -> Vec<(u64, f32)> {
    let mut exploration_heap = BinaryHeap::new();  // for routing
    let mut result_heap = BinaryHeap::new();        // for final results
    let mut ef_remaining = ef;

    // ... standard HNSW entry point selection ...

    while ef_remaining > 0 && !exploration_heap.is_empty() {
        let current = exploration_heap.pop();

        for neighbor in graph.neighbors(current) {
            let dist = distance(query, neighbor);

            // ALWAYS push to exploration heap — excluded nodes are routing waypoints
            exploration_heap.push(neighbor, dist);

            // ONLY push to result heap if member of this branch
            if membership.contains(neighbor) {
                result_heap.push(neighbor, dist);
                ef_remaining -= 1;
            }
            // Excluded nodes DO NOT decrement ef_remaining
        }
    }

    result_heap.into_sorted_vec()
}
```

**Rules** (explicit, not optional):
1. Excluded nodes **MAY** be pushed onto the exploration heap (they serve as routing waypoints).
2. Excluded nodes **MUST NOT** be pushed onto the result heap.
3. Excluded nodes **DO NOT** decrement `ef_remaining`.

This ensures recall is not degraded by filtered-out nodes blocking the search frontier.

### 8.4.1 Filter + Delta Composition Order

When both delta patches and membership filters are active, the composition order is strict:

```
1. Compose slab + delta first  (apply delta to base cluster data)
2. Apply membership visibility  (filter which vectors are visible)
3. Compute distance             (only on visible, patched vectors)
```

This ordering prevents information leakage: a delta overlay that modifies a hidden vector's data must not cause that vector to appear in results. The membership filter is always the final gate before distance computation.

### 8.5 COW-Aware Compaction

**Mode 1 — Read Optimize**:
- Rewrite hot clusters contiguously (sequential I/O).
- Upgrade ART map to use extent entries for sequential runs.
- Inline delta segments that exceed the hot-path threshold.

**Mode 2 — Space Reclaim**:
- For each local cluster: compute hash, compare to parent cluster hash.
- If `hash(local) == hash(parent)`: replace local with `ParentRef`, free storage.
- Net effect: undoes unnecessary COW copies where mutations were reverted.

**Refcount rebuild**: Walk entire COW map chain from all known derived files. Recount all references. Write new REFCOUNT_SEG.

**Segment preservation rule**: Unknown segments (types not recognized by the compactor) are **copied forward unchanged** unless the user passes `--strip-unknown`. This ensures forward compatibility.

### 8.6 Snapshot-Freeze

Setting `RefcountHeader.snapshot_epoch > 0` freezes the current state. All further writes create a **new derived generation** — the frozen snapshot becomes an immutable base.

Freeze is a metadata-only operation (no data copy). It:
1. Writes a new REFCOUNT_SEG with `snapshot_epoch = current_epoch + 1`.
2. Updates the Level0Root manifest.
3. Emits a `WITNESS_LINEAGE_SNAPSHOT` witness event.

---

## 9. Deterministic Kernel Selection

When multiple `KERNEL_SEG` segments exist in a single file (e.g., x86_64 + aarch64 builds):

```
Selection priority (first match wins):
1. Exact architecture match for host CPU
2. SIGNED required — reject unsigned kernels if ANY kernel in the file is signed
3. ML-DSA-65 (post-quantum) signature preferred over Ed25519
4. Highest compatible api_version
5. Lowest segment_offset (first in file) as tiebreaker
```

This prevents:
- Running an unsigned kernel when signed alternatives exist (downgrade attack).
- Running an incompatible architecture kernel.
- Non-deterministic selection that could vary across boots.

---

## 10. Real Computational Container Artifacts

### 10.1 Wire `serve` Command

**Current state**: `rvf-cli/src/cmd/serve.rs` prints a stub message.

**Target**: Wire the existing `rvf-server` crate (axum HTTP + TCP) into the CLI `serve` subcommand. No Docker, no QEMU — just start the server on the host with the specified RVF file.

**Implementation**: Replace stub body with:
```rust
let config = ServerConfig {
    http_port: args.port,
    tcp_port: args.tcp_port.unwrap_or(args.port + 1000),
    data_path: args.file.clone(),
    dimension: 0, // auto-detect from file
};
rvf_server::run(config).await
```

### 10.2 Custom MicroLinux Kernel (`rvf-kernel` crate)

**Build pipeline**: Docker-based, producing:
- `bzImage` (~1.5 MB uncompressed)
- `initramfs.cpio.gz` (~512 KB)
- Combined ZSTD-compressed: ~800 KB

**Kernel config highlights**:
| Category | Options |
|----------|---------|
| TEE | `AMD_MEM_ENCRYPT`, `INTEL_TDX_GUEST` |
| Speed | `PREEMPT_NONE`, `NO_HZ_FULL`, `SLAB` |
| Security | `LOCKDOWN`, `KASLR`, `STACKPROTECTOR_STRONG`, `STATIC_USERMODEHELPER` |
| I/O | `VIRTIO_PCI`, `VIRTIO_BLK`, `VIRTIO_NET`, `VSOCK` |
| BPF | `BPF_JIT`, `BPF_SYSCALL` |
| Disabled | modules, sound, USB, DRM, wireless, Bluetooth |

**Initramfs contents**: busybox (static) + dropbear (SSH) + rvf-server (static musl). `/init` script boots networking and starts the RVF query server.

**API**:
```rust
pub struct KernelBuilder { ... }
impl KernelBuilder {
    pub fn build() -> Result<KernelArtifact, Error>;       // Docker build
    pub fn from_prebuilt() -> Result<KernelArtifact, Error>; // bundled binary
    pub fn embed(store: &mut RvfStore, artifact: &KernelArtifact) -> Result<u64, Error>;
}

pub struct KernelVerifier { ... }
impl KernelVerifier {
    pub fn extract_and_verify(store: &RvfStore) -> Result<VerifiedKernel, Error>;
}
```

### 10.3 QEMU Launcher (`rvf-launch` crate)

**Launch configuration**:
```
qemu-system-x86_64 \
  -machine microvm,accel=kvm \
  -m 64M \
  -kernel bzImage \
  -initrd initramfs.cpio.gz \
  -drive id=rvf,file=data.rvf,format=raw,if=none,readonly=on \
  -device virtio-blk-device,drive=rvf \
  -netdev user,id=net0,hostfwd=tcp::8080-:8080 \
  -device virtio-net-device,netdev=net0 \
  -nographic -no-reboot
```

**API**:
```rust
pub struct Launcher { ... }
impl Launcher {
    pub fn launch(rvf_path: &Path, config: LaunchConfig) -> Result<MicroVm, Error>;
}

pub struct MicroVm { ... }
impl MicroVm {
    pub fn wait_ready(&self, timeout: Duration) -> Result<(), Error>;
    pub fn query(&self, vector: &[f32], k: usize) -> Result<Vec<SearchResult>, Error>;
    pub fn shutdown(self) -> Result<(), Error>;
}
```

### 10.4 Real eBPF Programs (`rvf-ebpf` crate)

**Programs** (compiled with `clang -target bpf -O2`):

| Program | Type | Purpose |
|---------|------|---------|
| `xdp_distance.c` | XDP | Inline L2/cosine distance on packet ingress |
| `socket_filter.c` | Socket Filter | Query preprocessing and validation |
| `tc_query_route.c` | TC Classifier | Route queries to optimal NUMA node |

**API**:
```rust
pub struct EbpfCompiler { ... }
impl EbpfCompiler {
    pub fn compile(source: &Path) -> Result<EbpfArtifact, Error>;
    pub fn embed(store: &mut RvfStore, artifact: &EbpfArtifact) -> Result<u64, Error>;
}
```

---

## 11. CLI Commands

### 11.1 New Commands

| Command | Description |
|---------|-------------|
| `rvf launch <file>` | Boot RVF in QEMU microVM, forward API port |
| `rvf embed-kernel <file> [--arch x86_64] [--prebuilt]` | Embed kernel image into RVF |
| `rvf embed-ebpf <file> --program <source.c>` | Compile and embed eBPF program |
| `rvf filter <file> --include <id-list>` | Create MEMBERSHIP_SEG with include filter |
| `rvf freeze <file>` | Snapshot-freeze current state |
| `rvf verify-witness <file>` | Verify all witness events in chain |
| `rvf verify-attestation <file>` | Verify KernelBinding + attestation |
| `rvf rebuild-refcounts <file>` | Recompute REFCOUNT_SEG from COW map chain |

### 11.2 Updated Commands

| Command | Change |
|---------|--------|
| `rvf serve <file>` | Wire real `rvf-server` (replace stub) |
| `rvf compact <file>` | Add `--strip-unknown` flag for segment stripping |

---

## 12. Level0Root Reserved Area Layout

The 252-byte reserved area (offset 0xF00-0xFFB) in `Level0Root` is partitioned:

```
┌──────────┬──────┬────────────────────────────────────────────┐
│ Offset   │ Size │ Field                                      │
├──────────┼──────┼────────────────────────────────────────────┤
│ 0xF00    │  68  │ FileIdentity (existing, ADR-029)           │
│ 0xF44    │   8  │ cow_map_offset (u64, 0=no COW map)        │
│ 0xF4C    │   4  │ cow_map_generation (u32, monotonic)        │
│ 0xF50    │   8  │ membership_offset (u64, 0=no filter)      │
│ 0xF58    │   4  │ membership_generation (u32, monotonic)     │
│ 0xF5C    │   4  │ snapshot_epoch (u32, 0=mutable)            │
│ 0xF60    │   4  │ double_root_generation (u32, for crash     │
│          │      │  recovery — monotonic counter)             │
│ 0xF64    │  32  │ double_root_hash (SHAKE-256-256 of the     │
│          │      │  OTHER Level0Root copy, for cross-check)   │
│ 0xF84    │ 120  │ _reserved_future (must be zero)            │
└──────────┴──────┴────────────────────────────────────────────┘
```

This fits within the existing 252-byte reserved area without changing the Level0Root size (still 4096 bytes).

---

## 13. Threat Model

| Threat | Mitigation |
|--------|------------|
| **Host compromise** | VMM isolation. Signed kernel. KernelBinding prevents segment swap. |
| **Guest compromise** | Minimal kernel (no modules, hardened). eBPF verifier. Append-only witness log. |
| **TEE integrity** | `KERNEL_FLAG_MEASURED` + `WITNESS_SEG` attestation. Covers code + manifest. |
| **Supply chain** | ML-DSA-65 signatures. Reproducible Docker builds. `build_id` tracing. |
| **Replay attack** | Monotonic epoch in witness. Nonce in attestation. `generation_id` in membership filter. |
| **Data swap** | `KernelBinding.manifest_root_hash` verified before boot. |
| **Malicious alt kernel** | Deterministic selection (Section 9). Signed-required downgrade prevention. |
| **COW map poisoning** | Deterministic map ordering. Compaction verifies cluster hashes vs parent. |
| **Stale membership filter** | `generation_id` monotonic check. Lower generation rejected at query time. |

**Out of scope**: TEE side channels, physical access, DoS via resource exhaustion, network-level attacks on the API transport.

---

## 14. Error Codes (New)

Added to `ErrorCode` enum in `rvf-types/src/error.rs`:

```
// Category 0x07: COW Errors
0x0700  CowMapCorrupt        — COW map structure invalid
0x0701  ClusterNotFound      — Referenced cluster not in map or parent
0x0702  ParentChainBroken    — Parent file not accessible during COW read
0x0703  DeltaThresholdExceeded — Delta too large, requires full slab COW
0x0704  SnapshotFrozen       — Write attempted on frozen snapshot
0x0705  MembershipInvalid    — Membership filter hash mismatch
0x0706  GenerationStale      — Membership/COW map generation too old
0x0707  KernelBindingMismatch — KernelBinding.manifest_root_hash != current manifest
0x0708  DoubleRootCorrupt    — Both Level0Root copies invalid (unrecoverable)
```

---

## 15. Files Changed

### New Files

| File | Crate | Description |
|------|-------|-------------|
| `rvf-types/src/cow_map.rs` | rvf-types | CowMapHeader (64B) + ART node types |
| `rvf-types/src/refcount.rs` | rvf-types | RefcountHeader (32B) |
| `rvf-types/src/membership.rs` | rvf-types | MembershipHeader (96B) + filter types |
| `rvf-types/src/delta.rs` | rvf-types | DeltaHeader (64B) |
| `rvf-types/src/kernel_binding.rs` | rvf-types | KernelBinding (128B, padded) |
| `rvf-runtime/src/cow.rs` | rvf-runtime | Read/write paths, slab copy, write coalescing |
| `rvf-runtime/src/cow_map.rs` | rvf-runtime | ART map implementation with extents |
| `rvf-runtime/src/cow_compact.rs` | rvf-runtime | COW-aware compaction |
| `rvf-runtime/src/membership.rs` | rvf-runtime | Membership filter for HNSW traversal |
| `rvf-kernel/` | rvf-kernel | Kernel builder + verifier + Docker pipeline |
| `rvf-launch/` | rvf-launch | QEMU microVM launcher |
| `rvf-ebpf/` | rvf-ebpf | eBPF compiler + loader + C source |
| `rvf-cli/src/cmd/launch.rs` | rvf-cli | Launch command |
| `rvf-cli/src/cmd/embed_kernel.rs` | rvf-cli | Embed kernel command |
| `rvf-cli/src/cmd/embed_ebpf.rs` | rvf-cli | Embed eBPF command |
| `rvf-cli/src/cmd/filter.rs` | rvf-cli | Filter command |
| `rvf-cli/src/cmd/freeze.rs` | rvf-cli | Freeze command |
| `rvf-cli/src/cmd/verify_witness.rs` | rvf-cli | Verify witness command |
| `rvf-cli/src/cmd/verify_attestation.rs` | rvf-cli | Verify attestation command |
| `rvf-cli/src/cmd/rebuild_refcounts.rs` | rvf-cli | Rebuild refcounts command |

### Modified Files

| File | Change |
|------|--------|
| `rvf-types/src/segment_type.rs` | Add CowMap=0x20, Refcount=0x21, Membership=0x22, Delta=0x23 |
| `rvf-types/src/lib.rs` | Add module declarations + re-exports |
| `rvf-types/src/error.rs` | Add Category 0x07 COW error codes |
| `rvf-manifest/src/level0.rs` | COW pointers in reserved area, double-root scheme |
| `rvf-runtime/src/store.rs` | derive/branch/snapshot APIs, double-root, write coalescing |
| `rvf-cli/src/main.rs` | 8 new subcommands |
| `rvf-cli/src/cmd/serve.rs` | Wire real rvf-server |

### New Test Files

| File | Coverage |
|------|----------|
| `tests/rvf-integration/tests/cow_branching.rs` | 1M-vector benchmark, slab copy counting |
| `tests/rvf-integration/tests/cow_crash_recovery.rs` | Torn Level0Root write recovery |
| `tests/rvf-integration/tests/segment_preservation.rs` | Unknown segments survive compaction |
| `tests/rvf-integration/tests/filter_traversal.rs` | HNSW + membership recall >= 0.70 |
| `tests/rvf-integration/tests/kernel_selection.rs` | Deterministic multi-kernel selection |
| `tests/rvf-integration/tests/lineage_verification.rs` | Provenance chain validation |
| `tests/rvf-integration/tests/real_kernel_boot.rs` | Embed → boot → query |
| `tests/rvf-integration/tests/real_ebpf.rs` | clang → embed → verify |

---

## 16. Execution Order

```
Phase 1: RVCOW Types + IDs
  → cow_map.rs, refcount.rs, membership.rs, delta.rs, kernel_binding.rs
  → segment_type.rs edits, lib.rs module declarations
  → All compile-time size + field offset assertions

Phase 2: RVCOW Runtime
  → derive, mutate, freeze, compact
  → preserve unknown segments, double-root scheme
  → Write coalescing, L0 cache

Phase 3: COW Integration Tests
  → cow_branching (1M benchmark)
  → cow_crash_recovery (torn Level0Root)
  → segment_preservation
  → filter_traversal (HNSW + membership)

Phase 4: Wire `serve` command
  → Real rvf-server on host runtime

Phase 5: Kernel Binding + Selection
  → KernelBinding footer, deterministic selection, tests

Phase 6: rvf-kernel crate
  → Docker build pipeline, prebuilt kernel, embed API

Phase 7: rvf-launch crate
  → QEMU launcher, wait_ready, query, shutdown

Phase 8: rvf-ebpf crate
  → Real BPF programs (C source), clang pipeline

Phase 9: CLI Commands
  → All 8 new commands + serve update + compact update

Phase 10: ADR finalization + updated examples
```

---

## 17. Acceptance Benchmark

A single scripted run must prove all of the following:

```
1. Create base file with 1M vectors (dim 128, f32)
   → File size ≈ 512 MB

2. Derive child with include membership set (50% of vectors)
   → Child contains MEMBERSHIP_SEG, no vector data copy

3. Modify 100 vectors across 10 clusters in the child
   → Exactly 10 slab copies (one per cluster)
   → Exactly 10 CLUSTER_COW witness events

4. Child file size = ~10 slabs + map + metadata
   → NOT 1M vectors (must be << 512 MB)

5. Query recall >= 0.70 on child (using parent's HNSW with membership filter)
   → Before full index rebuild
   → Excluded nodes used as routing waypoints only

6. Crash mid-write (simulated torn Level0Root)
   → Child file readable and consistent after recovery
   → Fallback to previous valid root via double-root scheme

7. Compact child with --strip-unknown
   → Unknown segment types removed
   → Known segments preserved and rewritten contiguously

8. Embed signed kernel with KernelBinding
   → Swap KERNEL_SEG from different file → verification FAILS
   → Original KERNEL_SEG → verification PASSES
```

---

## 18. Verification Commands

```bash
# Type-level tests (headers, size assertions, offset checks)
cargo test -p rvf-types

# COW runtime tests
cargo test --test cow_branching          # 1M benchmark
cargo test --test cow_crash_recovery     # torn write recovery
cargo test --test segment_preservation   # unknown segments survive
cargo test --test filter_traversal       # HNSW + membership recall

# Kernel tests
cargo test --test kernel_selection       # deterministic multi-kernel
cargo test --test lineage_verification   # provenance chain

# Real artifact tests (require Docker / QEMU / clang)
cargo test --test real_kernel_boot       # embed → boot → query
cargo test --test real_ebpf              # clang → embed → verify

# Full workspace
cargo test --workspace
cargo clippy --workspace --exclude rvf-wasm
```

---

## 19. Consequences

### Positive

- **Storage**: A 1M-vector derived file with 100 mutations stores ~10 clusters (~2.5 MB) instead of copying the full 512 MB base.
- **Performance**: Shared HNSW with membership filter avoids O(N log N) index rebuild per branch.
- **Security**: KernelBinding prevents segment-swap attacks. Double-root prevents bricking on crash.
- **Forward compatibility**: Unknown segments preserved by default. Older tools gracefully ignore COW segments.
- **Ecosystem**: Real kernel + launcher enables `rvf serve` → `rvf launch` pipeline for self-booting vector databases.

### Negative

- **Complexity**: Four new segment types, new runtime paths, crash recovery logic.
- **Parent dependency**: COW children require access to parent file for inherited clusters. Broken parent chain = broken child.
- **Docker/QEMU dependency**: Real kernel builds require Docker. Real launches require QEMU+KVM. CI must provision these.
- **Wire format change**: KernelBinding breaks signed KERNEL_SEG backward compatibility (unsigned stubs unaffected).

### Neutral

- Refcounts are rebuildable (no WAL), trading write simplicity for compaction cost.
- ART map adds constant overhead per derived file (~few KB for most workloads).

---

## 20. Related Decisions

| ADR | Relationship |
|-----|-------------|
| ADR-001 | Segment type registry (extended with 0x20-0x23) |
| ADR-029 | FileIdentity / lineage (used in COW map parent references) |
| ADR-030 | Computational containers (extended with KernelBinding, real artifacts) |

---

## Appendix A: Magic Number Registry

| Magic | Hex | Segment |
|-------|-----|---------|
| `RVFS` | `0x5256_4653` | SegmentHeader (all segments) |
| `RVM0` | `0x5256_4D30` | Level0Root manifest |
| `RVKN` | `0x5256_4B4E` | KernelHeader |
| `RVBP` | `0x5256_4250` | EbpfHeader |
| `RVCM` | `0x5256_434D` | CowMapHeader (NEW) |
| `RVRC` | `0x5256_5243` | RefcountHeader (NEW) |
| `RVMB` | `0x5256_4D42` | MembershipHeader (NEW) |
| `RVDL` | `0x5256_444C` | DeltaHeader (NEW) |

## Appendix B: Compile-Time Assertions Required

```rust
const _: () = assert!(size_of::<CowMapHeader>() == 64);
const _: () = assert!(size_of::<RefcountHeader>() == 32);
const _: () = assert!(size_of::<MembershipHeader>() == 96);
const _: () = assert!(size_of::<DeltaHeader>() == 64);
const _: () = assert!(size_of::<KernelBinding>() == 128);
// Existing (must not regress):
const _: () = assert!(size_of::<KernelHeader>() == 128);
const _: () = assert!(size_of::<EbpfHeader>() == 64);
const _: () = assert!(size_of::<SegmentHeader>() == 64);
const _: () = assert!(size_of::<Level0Root>() == 4096);
const _: () = assert!(size_of::<FileIdentity>() == 68);
const _: () = assert!(size_of::<AttestationHeader>() == 112);
```
