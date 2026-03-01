# Security Review: RuVector Graph Transformer Foundation Crates

**Auditor**: Security Auditor Agent (V3)
**Date**: 2026-02-25
**Scope**: ruvector-verified, ruvector-verified-wasm, ruvector-gnn, ruvector-attention
**Classification**: INTERNAL -- SECURITY SENSITIVE

---

## Executive Summary

This security review covers the four foundational crates that underpin the RuVector Graph Transformer: the formal verification engine (`ruvector-verified`), its WASM bindings (`ruvector-verified-wasm`), the GNN training pipeline (`ruvector-gnn`), and the attention mechanisms (`ruvector-attention`).

**Overall Assessment**: The codebase demonstrates security-conscious design in several areas -- notably the use of `checked_add` for arena allocation, `checked_mul` in mmap offset calculations, and input validation at system boundaries. However, **13 findings** were identified across severity levels, with **2 HIGH**, **6 MEDIUM**, and **5 LOW** issues. No CRITICAL vulnerabilities were found that would allow arbitrary code execution, but several issues could enable denial of service, proof-system integrity degradation, or attestation forgery in adversarial environments.

The most significant findings are: (1) the `MmapGradientAccumulator` lacks bounds checking on `node_id` in its `accumulate()` and `get_grad()` methods despite performing raw pointer arithmetic in unsafe blocks, and (2) the `ProofAttestation` system uses non-cryptographic hashing (FNV-1a) and includes no signature mechanism, meaning attestations can be trivially forged.

---

## Findings Table

| ID | Severity | Category | Location | Description |
|----|----------|----------|----------|-------------|
| SEC-001 | HIGH | Memory Safety | `ruvector-gnn/src/mmap.rs:461-496` | `MmapGradientAccumulator::accumulate()` and `get_grad()` perform unchecked pointer arithmetic on `node_id` |
| SEC-002 | HIGH | Proof Integrity | `ruvector-verified/src/proof_store.rs:100-108,112-139` | Attestations use non-cryptographic hash and lack signatures; trivially forgeable |
| SEC-003 | MEDIUM | DoS | `ruvector-verified-wasm/src/lib.rs:111-127` | `verify_batch_flat()` panics on `dim=0` due to division by zero |
| SEC-004 | MEDIUM | Cache Poisoning | `ruvector-verified/src/cache.rs:56-71` | Hash collision in `ConversionCache` silently returns wrong proof result |
| SEC-005 | MEDIUM | DoS | `ruvector-verified/src/fast_arena.rs:51-59` | `FastTermArena::with_capacity()` can allocate unbounded memory via large `expected_terms` |
| SEC-006 | MEDIUM | Proof Integrity | `ruvector-verified/src/lib.rs:93-100` | `alloc_term()` panics on u32 overflow instead of returning `Result` |
| SEC-007 | MEDIUM | Integer Overflow | `ruvector-verified/src/vector_types.rs:106,125` | `vector.len() as u32` truncates silently on vectors longer than 4 billion elements |
| SEC-008 | MEDIUM | Memory Safety | `ruvector-gnn/src/mmap.rs:148-186` | `MmapManager::new()` uses unchecked multiplication for `file_size` calculation |
| SEC-009 | LOW | WASM | `ruvector-verified-wasm/src/utils.rs:4-7` | `set_panic_hook()` is a no-op; panics in WASM will abort without diagnostics |
| SEC-010 | LOW | Cache Integrity | `ruvector-verified/src/fast_arena.rs:70-91` | Arena intern with `hash=0` is silently uncacheable, skipping dedup |
| SEC-011 | LOW | Timestamp | `ruvector-verified/src/proof_store.rs:142-147` | Attestation timestamp uses `as u64` truncation on 128-bit nanosecond value |
| SEC-012 | LOW | Concurrency | `ruvector-gnn/src/mmap.rs:590-591` | `unsafe impl Send/Sync` for `MmapGradientAccumulator` relies on `UnsafeCell<MmapMut>` correctness |
| SEC-013 | LOW | Info Disclosure | `ruvector-verified/src/error.rs` | Error messages expose internal term IDs and symbol counts |

---

## Detailed Analysis

### SEC-001: Unchecked Bounds in MmapGradientAccumulator (HIGH)

**File**: `/workspaces/ruvector/crates/ruvector-gnn/src/mmap.rs`
**Lines**: 461-496, 545-556

**Description**: The `MmapGradientAccumulator` methods `accumulate()`, `get_grad()`, and `grad_offset()` perform raw pointer arithmetic without validating that `node_id` is within bounds. Unlike `MmapManager` which has a `validate_node_id()` check, the gradient accumulator directly computes an offset and dereferences it inside unsafe blocks.

```rust
// grad_offset performs unchecked arithmetic
pub fn grad_offset(&self, node_id: u64) -> usize {
    (node_id as usize) * self.d_embed * std::mem::size_of::<f32>()
    // No bounds check! No checked_mul!
}

pub fn accumulate(&self, node_id: u64, grad: &[f32]) {
    // ... only checks grad.len() == self.d_embed ...
    let offset = self.grad_offset(node_id);  // unchecked
    unsafe {
        let mmap = &mut *self.grad_mmap.get();
        let ptr = mmap.as_mut_ptr().add(offset) as *mut f32;  // OOB write possible
        let grad_slice = std::slice::from_raw_parts_mut(ptr, self.d_embed);
        // ...
    }
}
```

A `node_id` value exceeding `n_nodes` causes out-of-bounds memory access in a memory-mapped region. Additionally, `(node_id as usize) * self.d_embed * std::mem::size_of::<f32>()` can overflow on 32-bit targets (or even 64-bit with extreme values) since it uses unchecked arithmetic, unlike `MmapManager::embedding_offset()` which correctly uses `checked_mul`.

The `lock_idx` calculation `(node_id as usize) / self.lock_granularity` can also index out of bounds in the `self.locks` vector if `node_id >= n_nodes`.

**Impact**: Out-of-bounds read/write in the memory-mapped region. On Linux, this could write past the end of the mmap'd file, potentially causing SIGBUS or corrupting adjacent memory mappings.

**Recommendation**:
1. Add a `validate_node_id()` method mirroring `MmapManager`'s implementation.
2. Use `checked_mul` for offset computation.
3. Assert `node_id < self.n_nodes` before any pointer arithmetic.
4. Assert `lock_idx < self.locks.len()` before lock acquisition.

---

### SEC-002: Attestation Forgery -- No Cryptographic Binding (HIGH)

**File**: `/workspaces/ruvector/crates/ruvector-verified/src/proof_store.rs`
**Lines**: 100-108, 112-139

**Description**: The `ProofAttestation` struct and its `create_attestation()` function claim to provide "Ed25519-signed proof attestation" (per the module doc comment on line 1), but the actual implementation contains **no signature, no HMAC, and no cryptographic binding** of any kind.

The `content_hash()` method uses FNV-1a, a non-cryptographic hash:

```rust
pub fn content_hash(&self) -> u64 {
    let bytes = self.to_bytes();
    let mut h: u64 = 0xcbf29ce484222325;  // FNV offset basis
    for &b in &bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);  // FNV prime
    }
    h
}
```

Furthermore, `create_attestation()` constructs hashes that are trivially predictable:

```rust
let mut proof_hash = [0u8; 32];
let id_bytes = proof_id.to_le_bytes();
proof_hash[0..4].copy_from_slice(&id_bytes);           // only 4 bytes populated
proof_hash[4..8].copy_from_slice(&env.terms_allocated().to_le_bytes());  // predictable

let mut env_hash = [0u8; 32];
let sym_count = env.symbols.len() as u32;
env_hash[0..4].copy_from_slice(&sym_count.to_le_bytes());  // always ~11
```

The `proof_term_hash` and `environment_hash` fields (both 32 bytes, suggesting SHA-256) are almost entirely zero-filled, with only 4-8 bytes of predictable, non-cryptographic content. An adversary can construct arbitrary attestations by filling in the known values.

**Impact**: Any party can forge proof attestations that appear valid. If these attestations are later used for trust decisions (e.g., in RVF WITNESS_SEG entries), forged attestations could certify unverified computations as formally proven.

**Recommendation**:
1. Implement the Ed25519 signing described in the module doc, or remove the claim.
2. Use a cryptographic hash (BLAKE3 or SHA-256) for `proof_term_hash` and `environment_hash`, computed over the actual proof term and environment state -- not just the counter values.
3. Include a proper signature field in `ProofAttestation` and increase `ATTESTATION_SIZE` accordingly (82 + 64 = 146 bytes with Ed25519).
4. Consider a keyed MAC at minimum if full signatures are too expensive for the hot path.

---

### SEC-003: WASM Division by Zero on dim=0 (MEDIUM)

**File**: `/workspaces/ruvector/crates/ruvector-verified-wasm/src/lib.rs`
**Lines**: 111-127

**Description**: The `verify_batch_flat()` function converts `dim` to `usize` and uses it as a divisor without checking for zero:

```rust
pub fn verify_batch_flat(&mut self, dim: u32, flat_vectors: &[f32]) -> Result<u32, JsError> {
    let d = dim as usize;
    if flat_vectors.len() % d != 0 {   // panics if d == 0
        // ...
    }
    let slices: Vec<&[f32]> = flat_vectors.chunks_exact(d).collect();  // panics if d == 0
    // ...
}
```

When called from JavaScript with `dim=0`, this causes a panic in the modulo operation (`% 0`), which in WASM results in an `unreachable` trap. Since `set_panic_hook()` is a no-op (SEC-009), the browser receives no useful error message.

**Impact**: A browser-side caller (potentially adversarial JavaScript) can crash the WASM module with a single call. If the WASM module is long-lived (e.g., in a service worker), this is a denial-of-service vector.

**Recommendation**:
1. Add `if dim == 0 { return Err(JsError::new("dimension must be > 0")); }` at the top of `verify_batch_flat()`.
2. Apply the same check to `verify_dim_check()`, `prove_dim_eq()`, and `mk_vector_type()` at the WASM boundary.

---

### SEC-004: Cache Collision Causes Silent Proof Mismatch (MEDIUM)

**File**: `/workspaces/ruvector/crates/ruvector-verified/src/cache.rs`
**Lines**: 56-71

**Description**: The `ConversionCache` uses direct-mapped (1-way associative) open addressing. When two different `(term_id, ctx_len)` pairs hash to the same slot, the newer entry silently evicts the older one. Subsequent lookups for the evicted entry will miss, which is correct. However, if two *different* pairs produce the *same* `key_hash` value (a hash collision), the `get()` method will return the wrong `result_id`:

```rust
pub fn get(&mut self, term_id: u32, ctx_len: u32) -> Option<u32> {
    let hash = self.key_hash(term_id, ctx_len);
    let slot = (hash as usize) & self.mask;
    let entry = &self.entries[slot];
    if entry.key_hash == hash && entry.key_hash != 0 {
        // Only checks hash equality, not (term_id, ctx_len) equality!
        self.stats.hits += 1;
        Some(entry.result_id)  // could be the wrong result
    }
    // ...
}
```

The `CacheEntry` struct stores `input_id` but it is marked `#[allow(dead_code)]` and never checked during lookup. This means hash collisions in the `key_hash` function directly translate to returning incorrect proof results.

The `key_hash` function uses FxHash-style multiply-shift, which is fast but has known collision patterns. For a 64-bit hash space with 16K entries, collisions are astronomically unlikely in normal use, but the *correctness* of a proof system should not rely on probabilistic assumptions.

**Impact**: In pathological cases (adversarially chosen inputs or high cache load), the conversion cache could return a proof result for the wrong term, silently corrupting proof integrity. The formal verification guarantee degrades from "provably correct" to "probably correct."

**Recommendation**:
1. Store and compare the full `(term_id, ctx_len)` key in `get()`, not just the hash.
2. Remove `#[allow(dead_code)]` from `input_id` and add a `ctx_len` field.
3. Alternatively, document this as an accepted probabilistic cache and ensure the proof checker re-validates cached results.

---

### SEC-005: Unbounded Memory Allocation in FastTermArena (MEDIUM)

**File**: `/workspaces/ruvector/crates/ruvector-verified/src/fast_arena.rs`
**Lines**: 51-59

**Description**: `FastTermArena::with_capacity()` allocates cache proportional to `expected_terms * 2`, rounded up to the next power of two, with no upper bound:

```rust
pub fn with_capacity(expected_terms: usize) -> Self {
    let cache_cap = (expected_terms * 2).next_power_of_two().max(64);
    Self {
        // ...
        cache: RefCell::new(vec![0u64; cache_cap * 2]),  // 16 bytes per slot
        // ...
    }
}
```

An input of `expected_terms = usize::MAX / 2` would attempt to allocate approximately `2^64` bytes of memory. Even more moderate values like `expected_terms = 1_000_000_000` would allocate ~32 GB.

In the WASM context (via `JsProofEnv`), the arena is hardcoded to `with_capacity(4096)` which is safe, but any native caller can trigger OOM.

**Impact**: A caller providing a large capacity value can cause the process to exhaust available memory and be killed by the OOM killer.

**Recommendation**:
1. Add a maximum capacity constant (e.g., `const MAX_ARENA_CAPACITY: usize = 1 << 24`) and clamp the input.
2. Return a `Result` instead of panicking on allocation failure.

---

### SEC-006: Arena Overflow Panics Instead of Returning Error (MEDIUM)

**File**: `/workspaces/ruvector/crates/ruvector-verified/src/lib.rs`
**Lines**: 93-100

**Description**: `ProofEnvironment::alloc_term()` uses `checked_add(1)` (good), but converts the overflow to a panic via `.expect("arena overflow")`:

```rust
pub fn alloc_term(&mut self) -> u32 {
    let id = self.term_counter;
    self.term_counter = self.term_counter.checked_add(1)
        .ok_or_else(|| VerificationError::ArenaExhausted { allocated: id })
        .expect("arena overflow");  // <-- panics
    // ...
}
```

The error variant `ArenaExhausted` is correctly defined and even constructed, but then immediately unwrapped. The same pattern exists in `FastTermArena::alloc_with_hash()` and `FastTermArena::alloc()`.

**Impact**: After 2^32 allocations without reset, the proof environment panics instead of returning a recoverable error. In a long-running server context, this terminates the process.

**Recommendation**:
1. Change `alloc_term()` to return `Result<u32>` and propagate the `ArenaExhausted` error.
2. Update all callers to handle the Result.
3. Apply the same change to `FastTermArena::alloc()` and `alloc_with_hash()`.

---

### SEC-007: Silent Truncation of Vector Length to u32 (MEDIUM)

**File**: `/workspaces/ruvector/crates/ruvector-verified/src/vector_types.rs`
**Lines**: 106, 125, 162

**Description**: Multiple functions cast `vector.len()` (a `usize`) to `u32` without checking for truncation:

```rust
let actual_dim = vector.len() as u32;
let dim_proof = prove_dim_eq(env, index_dim, vector.len() as u32)?;
```

On 64-bit platforms, a vector with length `0x1_0000_0080` (4,294,967,424) would truncate to `128` when cast to `u32`. A dimension proof for `prove_dim_eq(env, 128, 128)` would then succeed, falsely certifying that a vector of length ~4.3 billion matches a 128-dimensional index.

**Impact**: In theory, an adversary could craft an over-sized vector that passes dimension verification by exploiting u32 truncation. In practice, allocating a 4-billion-element f32 vector requires ~16 GB of RAM, making this difficult to exploit but not impossible in high-memory environments.

**Recommendation**:
1. Add `assert!(vector.len() <= u32::MAX as usize)` or use `u32::try_from(vector.len()).map_err(...)` before the cast.
2. Consider using `usize` for dimensions throughout the proof system to avoid this class of error entirely.

---

### SEC-008: Unchecked File Size Calculation in MmapManager (MEDIUM)

**File**: `/workspaces/ruvector/crates/ruvector-gnn/src/mmap.rs`
**Lines**: 148-162

**Description**: The `MmapManager::new()` constructor computes file size with unchecked multiplication:

```rust
let embedding_size = d_embed * std::mem::size_of::<f32>();
let file_size = max_nodes * embedding_size;
```

With `d_embed = 65536` and `max_nodes = 65536`, `file_size` would be `65536 * 65536 * 4 = 17,179,869,184` (~16 GB), which is large but valid. With `d_embed = 1_000_000` and `max_nodes = 1_000_000`, the multiplication overflows on 64-bit (`4 * 10^12`), though on most systems this would fail at `file.set_len()` before causing memory issues.

Notably, `MmapGradientAccumulator::new()` has the identical pattern at lines 408-411.

The irony is that `MmapManager::embedding_offset()` correctly uses `checked_mul`, but the constructor that determines the file size does not.

**Impact**: On 32-bit targets or with extreme parameters, integer overflow could create a smaller-than-expected file, leading to out-of-bounds access when embeddings are written to the expected (larger) address space.

**Recommendation**:
1. Use `checked_mul` for the file size calculation and return an error if it overflows.
2. Add reasonable upper bounds for `d_embed` and `max_nodes` (e.g., both < 2^24).

---

### SEC-009: WASM Panic Hook is No-Op (LOW)

**File**: `/workspaces/ruvector/crates/ruvector-verified-wasm/src/utils.rs`
**Lines**: 4-7

**Description**: The `set_panic_hook()` function is a no-op:

```rust
pub fn set_panic_hook() {
    // No-op if console_error_panic_hook is not available.
}
```

This means any panic in the WASM module (from SEC-003, SEC-006, or any other panic path) will produce an opaque `RuntimeError: unreachable` in JavaScript with no stack trace or context.

**Impact**: Debugging production WASM issues becomes extremely difficult. Callers cannot distinguish between different failure modes.

**Recommendation**:
1. Add the `console_error_panic_hook` crate and call `console_error_panic_hook::set_once()`.
2. This is a one-line fix that dramatically improves WASM debuggability.

---

### SEC-010: Hash Value Zero Bypasses Arena Dedup (LOW)

**File**: `/workspaces/ruvector/crates/ruvector-verified/src/fast_arena.rs`
**Lines**: 70-97, 113

**Description**: The `intern()` method uses `hash == 0` as a sentinel for "empty slot" in the open-addressing table. If a caller provides `hash = 0`, the dedup check on line 80 (`if stored_hash == hash && hash != 0`) always fails, and the insert on line 113 (`if hash != 0`) also skips insertion. This means every call to `intern(0)` allocates a new term, defeating deduplication.

The `key_hash()` in `ConversionCache` correctly handles this (`if h == 0 { h = 1; }`), but `FastTermArena` does not.

**Impact**: An adversary or buggy caller using hash value 0 would cause unbounded term allocation, potentially exhausting the arena more quickly.

**Recommendation**:
1. Add `let hash = if hash == 0 { 1 } else { hash };` at the start of `intern()`.
2. Document that hash value 0 is reserved.

---

### SEC-011: Timestamp Truncation (LOW)

**File**: `/workspaces/ruvector/crates/ruvector-verified/src/proof_store.rs`
**Lines**: 142-147

**Description**: The timestamp conversion uses `d.as_nanos() as u64`, which truncates the 128-bit nanosecond value to 64 bits. A u64 can represent nanoseconds up to approximately year 2554, so this is not an immediate concern, but it is a latent truncation.

**Impact**: Minimal. The truncation becomes relevant only after year 2554.

**Recommendation**: Document the truncation or use `u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)`.

---

### SEC-012: Manual Send/Sync Impls for MmapGradientAccumulator (LOW)

**File**: `/workspaces/ruvector/crates/ruvector-gnn/src/mmap.rs`
**Lines**: 590-591

**Description**: The `MmapGradientAccumulator` uses `UnsafeCell<MmapMut>` for interior mutability and manually implements `Send` and `Sync`:

```rust
unsafe impl Send for MmapGradientAccumulator {}
unsafe impl Sync for MmapGradientAccumulator {}
```

The safety argument is that "access is protected by RwLocks." However, the lock granularity is per-region (64 nodes), not per-struct. The `zero_grad()` method modifies the entire mmap without acquiring any locks, creating a potential data race if another thread is concurrently calling `accumulate()`:

```rust
pub fn zero_grad(&mut self) {
    unsafe {
        let mmap = &mut *self.grad_mmap.get();
        for byte in mmap.iter_mut() {
            *byte = 0;
        }
    }
}
```

The `&mut self` receiver provides compile-time exclusivity via the borrow checker, so this is not unsound *if* `zero_grad()` is only called when no shared references exist. The `apply()` method calls `zero_grad()` via `&mut self`, which is correct.

**Impact**: Low risk currently because `&mut self` enforces exclusivity. However, if the API ever changes to take `&self` (e.g., for concurrent flush), this would become a data race.

**Recommendation**:
1. Add a comment documenting the invariant that `zero_grad()` requires exclusive access.
2. Consider acquiring all locks in `zero_grad()` for defense in depth.

---

### SEC-013: Internal State Leakage in Error Messages (LOW)

**File**: `/workspaces/ruvector/crates/ruvector-verified/src/error.rs`

**Description**: Error variants like `ArenaExhausted { allocated: u32 }`, `DimensionMismatch`, and the formatted messages in `TypeCheckFailed` expose internal term IDs, allocation counts, and type system details. In the WASM binding, these are passed directly to JavaScript via `JsError::new(&e.to_string())`.

**Impact**: An adversary probing the WASM API could use error messages to learn about internal state (number of terms allocated, specific type IDs), aiding in crafting more targeted attacks.

**Recommendation**:
1. In the WASM layer, sanitize error messages to expose only the error category, not internal counters.
2. Log detailed errors server-side (where applicable) and return generic messages to callers.

---

## Positive Security Observations

The following security-positive patterns were observed:

1. **Checked arithmetic in MmapManager**: The `embedding_offset()` method correctly uses `checked_mul` for all pointer arithmetic, and `get_embedding()`/`set_embedding()` validate bounds before unsafe dereference.

2. **`deny(unsafe_op_in_unsafe_fn)` in ruvector-gnn**: This lint ensures that unsafe operations inside unsafe functions must still be explicitly marked, improving auditability.

3. **Fuel-bounded verification in gated.rs**: The tiered proof system (`Reflex` / `Standard` / `Deep`) includes explicit fuel budgets (`max_fuel`, `max_reductions: 10_000`) preventing unbounded computation during proof checking.

4. **Input validation at WASM boundary**: The `verify_batch_flat()` function validates that the flat vector length is divisible by the dimension (modulo the dim=0 issue in SEC-003).

5. **Thread-local pools**: The `pools.rs` module uses `thread_local!` storage, avoiding cross-thread sharing of `ProofEnvironment` state.

6. **No unsafe code in ruvector-verified**: The entire proof engine (excluding WASM bindings) contains zero unsafe blocks, relying entirely on safe Rust abstractions.

7. **Numerical stability in training**: The `Loss` implementation uses epsilon clamping (`EPS = 1e-7`) and gradient clipping (`MAX_GRAD = 1e6`) to prevent numerical explosion in cross-entropy and BCE loss functions.

---

## Recommendations for the New Graph Transformer Crate

Based on this audit, the following security guidelines should be adopted for the `ruvector-graph-transformer` crate:

### 1. Proof-Gated Mutation Integrity

- Before using the `ruvector-verified` proof system to gate mutations, address SEC-002 (attestation forgery) and SEC-004 (cache collision). Without these fixes, the "proof-carrying" guarantee is aspirational rather than actual.
- Any proof-gated mutation path should verify attestation signatures (once implemented) at the point of use, not just at creation time.

### 2. Memory Safety for Graph Operations

- All graph operations that compute offsets from node/edge IDs must use `checked_mul` and `checked_add`, following the pattern in `MmapManager::embedding_offset()`.
- Node and edge counts should be validated at construction time with upper bounds.
- Prefer `u64` for node IDs with explicit `usize::try_from()` at use sites rather than `as usize` casts.

### 3. DoS Resistance

- Cap the maximum number of attention heads, graph layers, and batch sizes at construction time.
- Implement memory budget tracking: pre-compute the memory required for a graph transformer forward pass and reject inputs that would exceed a configurable limit.
- For the attention mechanisms (imported from `ruvector-attention`), validate that sequence lengths and dimensions are within bounds before entering the hot loop.

### 4. WASM-Specific Hardening

- Enable `console_error_panic_hook` in all WASM builds.
- Validate all inputs at the WASM boundary (dim > 0, lengths within u32 range, non-empty inputs).
- Consider using `wasm_bindgen`'s `#[wasm_bindgen(catch)]` pattern so that Rust panics convert to JavaScript exceptions rather than aborts.
- Set a WASM memory growth limit to prevent runaway allocations.

### 5. Adversarial Input Handling

- Graph transformer inputs (adjacency matrices, feature matrices, edge weights) should be validated for:
  - Non-negative edge counts
  - Consistent dimensions across all feature matrices
  - Absence of NaN/Inf values in floating-point inputs
  - Reasonable sparsity (reject fully-connected graphs above a size threshold)

### 6. Data Poisoning Defenses

- For the training pipeline (building on `ruvector-gnn`), implement:
  - Input sanitization for training data (reject NaN/Inf embeddings)
  - Gradient norm clipping as a mandatory defense (not just the loss-level clipping already in place)
  - Learning rate warmup to reduce the impact of early poisoned batches
  - Consider certified robustness bounds for the graph attention mechanism

---

## Summary of Required Actions

| Priority | Finding | Action Required |
|----------|---------|----------------|
| P0 | SEC-001 | Add bounds checking to `MmapGradientAccumulator` before next release |
| P0 | SEC-002 | Implement cryptographic attestation or remove forgery-prone API |
| P1 | SEC-003 | Add dim=0 guard at WASM boundary |
| P1 | SEC-004 | Store full key in ConversionCache, not just hash |
| P1 | SEC-005 | Cap arena capacity at a safe maximum |
| P1 | SEC-006 | Change `alloc_term()` to return Result |
| P2 | SEC-007 | Use `u32::try_from()` for vector length conversion |
| P2 | SEC-008 | Use `checked_mul` in MmapManager/Accumulator constructors |
| P3 | SEC-009 | Enable console_error_panic_hook |
| P3 | SEC-010 | Handle hash=0 sentinel in FastTermArena |
| P3 | SEC-011 | Document or guard timestamp truncation |
| P3 | SEC-012 | Document Send/Sync safety invariants |
| P3 | SEC-013 | Sanitize error messages at WASM boundary |

---

*End of security review. Questions and follow-ups should be directed to the security auditor agent.*
