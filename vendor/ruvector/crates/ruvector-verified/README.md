# ruvector-verified

[![Crates.io](https://img.shields.io/crates/v/ruvector-verified.svg)](https://crates.io/crates/ruvector-verified)
[![docs.rs](https://img.shields.io/docsrs/ruvector-verified)](https://docs.rs/ruvector-verified)
[![License](https://img.shields.io/crates/l/ruvector-verified.svg)](https://github.com/ruvnet/ruvector)
[![CI](https://img.shields.io/github/actions/workflow/status/ruvnet/ruvector/build-verified.yml?label=CI)](https://github.com/ruvnet/ruvector/actions)
[![MSRV](https://img.shields.io/badge/MSRV-1.77-blue.svg)](https://blog.rust-lang.org/2024/03/21/Rust-1.77.0.html)

**Proof-carrying vector operations for Rust.** Every dimension check, HNSW insert, and pipeline composition produces a machine-checked proof witness -- catching bugs that `assert!` misses, with less than 2% runtime overhead.

Built on [lean-agentic](https://crates.io/crates/lean-agentic) dependent types. Part of the [RuVector](https://github.com/ruvnet/ruvector) ecosystem.

---

### The Problem

Vector databases silently corrupt results when dimensions mismatch. A 384-dim query against a 768-dim index doesn't panic -- it returns wrong answers. Traditional approaches either:

- **Runtime `assert!`** -- panics in production, no proof trail
- **Const generics** -- catches errors at compile time, but can't handle dynamic dimensions from user input, config files, or model outputs

### The Solution

`ruvector-verified` generates **formal proofs** that dimensions match, types align, and pipelines compose correctly. Each proof is a replayable term -- not just a boolean check -- producing an 82-byte attestation that can be stored, audited, or embedded in RVF witness chains.

```rust
use ruvector_verified::{ProofEnvironment, prove_dim_eq, vector_types};

let mut env = ProofEnvironment::new(); // ~470ns, pre-loaded with 11 type declarations

// Prove dimensions match -- returns a reusable proof term, not just Ok/Err
let proof_id = prove_dim_eq(&mut env, 384, 384)?;   // ~500ns first call, ~15ns cached

// Wrong dimensions produce typed errors, not panics
let err = prove_dim_eq(&mut env, 384, 128);          // Err(DimensionMismatch { expected: 384, actual: 128 })

// Batch-verify 1000 vectors in ~11us (11ns per vector)
let vecs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
let verified = vector_types::verify_batch_dimensions(&mut env, 384, &vecs)?;
assert_eq!(verified.value, 1000);  // verified.proof_id traces back to the proof term

// Create an 82-byte attestation for audit/storage
let attestation = ruvector_verified::proof_store::create_attestation(&env, proof_id);
let bytes = attestation.to_bytes();  // embeddable in RVF witness chain (type 0x0E)
```

### Key Capabilities

- **Sub-microsecond proofs** -- dimension equality in 496ns, batch verification at 11ns/vector
- **Proof-carrying results** -- every `VerifiedOp<T>` bundles the result with its proof term ID
- **3-tier gated routing** -- automatically routes proofs to Reflex (<10ns), Standard (<1us), or Deep (<100us) based on complexity
- **82-byte attestations** -- formal proof witnesses that serialize into RVF containers
- **Thread-local pools** -- zero-contention resource reuse, 876ns acquire with auto-return
- **Pipeline composition** -- type-safe `A -> B >> B -> C` stage chaining with machine-checked proofs
- **Works with `Vec<f32>`** -- no special array types required, verifies standard Rust slices

## Performance

All operations benchmarked on a single core (no SIMD, no parallelism):

| Operation | Latency | Notes |
|-----------|---------|-------|
| `ProofEnvironment::new()` | **466ns** | Pre-loads 11 type declarations |
| `prove_dim_eq(384, 384)` | **496ns** | FxHash-cached, subsequent calls ~15ns |
| `mk_vector_type(384)` | **503ns** | Cached after first call |
| `verify_batch_dimensions(1000 vecs)` | **~11us** | Amortized ~11ns/vector |
| `FastTermArena::intern()` (hit) | **1.6ns** | 4-wide linear probe, 99%+ hit rate |
| `gated::route_proof()` | **1.2ns** | 3-tier routing decision |
| `ConversionCache::get()` | **9.6ns** | Open-addressing, 1000 entries |
| `pools::acquire()` | **876ns** | Thread-local, auto-return on Drop |
| `ProofAttestation::roundtrip` | **<1ns** | 82-byte serialize/deserialize |
| `env.reset()` | **379ns** | O(1) pointer reset |

**Overhead vs unverified operations: <2%** on batch vector ingest.

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `fast-arena` | - | `FastTermArena`: O(1) bump allocation with 4-wide dedup cache |
| `simd-hash` | - | AVX2/NEON accelerated hash-consing |
| `gated-proofs` | - | 3-tier Reflex/Standard/Deep proof routing |
| `ultra` | - | All optimizations (`fast-arena` + `simd-hash` + `gated-proofs`) |
| `hnsw-proofs` | - | Verified HNSW insert/query (requires `ruvector-core`) |
| `rvf-proofs` | - | RVF witness chain integration |
| `coherence-proofs` | - | Sheaf coherence verification |
| `all-proofs` | - | All proof integrations |
| `serde` | - | Serialization for `ProofAttestation` |

```toml
# Minimal: just dimension proofs
ruvector-verified = "0.1.0"

# All optimizations (recommended for production)
ruvector-verified = { version = "0.1.0", features = ["ultra"] }

# Everything
ruvector-verified = { version = "0.1.0", features = ["ultra", "all-proofs", "serde"] }
```

## Architecture

```
                        +-----------------------+
                        |   ProofEnvironment    |  Pre-loaded type declarations
                        |  (symbols, cache,     |  Nat, RuVec, Eq, HnswIndex, ...
                        |   term allocator)     |
                        +-----------+-----------+
                                    |
           +------------------------+------------------------+
           |                        |                        |
   +-------v-------+    +----------v----------+    +--------v--------+
   | vector_types  |    |     pipeline        |    |  proof_store    |
   | prove_dim_eq  |    | compose_stages      |    | ProofAttestation|
   | verify_batch  |    | compose_chain       |    | 82-byte witness |
   +-------+-------+    +----------+----------+    +--------+--------+
           |                        |                        |
           +----------- gated routing (3-tier) -------------+
           |            Reflex | Standard | Deep             |
           +-------- FastTermArena (bump + dedup) ----------+
           |        ConversionCache (open-addressing)        |
           +---------- pools (thread-local reuse) ----------+
```

## Comparison

| Feature | ruvector-verified | Runtime `assert!` | `ndarray` shape check | `nalgebra` const generics |
|---------|:-:|:-:|:-:|:-:|
| Proof-carrying operations | **Yes** | No | No | No |
| Dimension errors caught | At proof time | At runtime (panic) | At runtime | At compile time |
| Supports dynamic dimensions | **Yes** | Yes | Yes | No |
| Formal attestation (82-byte witness) | **Yes** | No | No | No |
| Pipeline type composition | **Yes** | No | No | Partial |
| Sub-microsecond overhead | **Yes** | Yes | Yes | Zero |
| Works with existing `Vec<f32>` | **Yes** | Yes | No | No |
| 3-tier proof routing | **Yes** | N/A | N/A | N/A |
| Thread-local resource pooling | **Yes** | N/A | N/A | N/A |

## Core API

### Dimension Proofs

```rust
use ruvector_verified::{ProofEnvironment, prove_dim_eq, vector_types};

let mut env = ProofEnvironment::new();

// Prove dimensions match (returns proof term ID)
let proof_id = prove_dim_eq(&mut env, 384, 384)?;

// Verify a single vector against an index
let vector = vec![0.5f32; 384];
let verified = vector_types::verified_dim_check(&mut env, 384, &vector)?;
// verified.proof_id is the machine-checked proof

// Batch verify
let batch: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
let batch_ok = vector_types::verify_batch_dimensions(&mut env, 384, &batch)?;
assert_eq!(batch_ok.value, vectors.len());
```

### Pipeline Composition

```rust
use ruvector_verified::{ProofEnvironment, VerifiedStage, pipeline::compose_stages};

let mut env = ProofEnvironment::new();

// Type-safe pipeline: Embedding(384) -> Quantized(128) -> Index
let embed: VerifiedStage<(), ()> = VerifiedStage::new("embed", 0, 1, 2);
let quant: VerifiedStage<(), ()> = VerifiedStage::new("quantize", 1, 2, 3);
let composed = compose_stages(&embed, &quant, &mut env)?;
assert_eq!(composed.name(), "embed >> quantize");
```

### Proof Attestation (82-byte Witness)

```rust
use ruvector_verified::{ProofEnvironment, proof_store};

let mut env = ProofEnvironment::new();
let proof_id = env.alloc_term();

let attestation = proof_store::create_attestation(&env, proof_id);
let bytes = attestation.to_bytes(); // exactly 82 bytes
assert_eq!(bytes.len(), 82);

// Round-trip
let recovered = ruvector_verified::ProofAttestation::from_bytes(&bytes)?;
```

<details>
<summary><strong>Ultra Optimizations (feature: <code>ultra</code>)</strong></summary>

### FastTermArena (feature: `fast-arena`)

O(1) bump allocation with 4-wide linear probe dedup cache. Modeled after `ruvector-solver`'s `SolverArena`.

```rust
use ruvector_verified::fast_arena::{FastTermArena, fx_hash_pair};

let arena = FastTermArena::with_capacity(4096);

// First intern: cache miss, allocates new term
let (id, was_cached) = arena.intern(fx_hash_pair(384, 384));
assert!(!was_cached);

// Second intern: cache hit, returns same ID in ~1.6ns
let (id2, was_cached) = arena.intern(fx_hash_pair(384, 384));
assert!(was_cached);
assert_eq!(id, id2);

// O(1) reset
arena.reset();
assert_eq!(arena.term_count(), 0);

// Statistics
let stats = arena.stats();
println!("hit rate: {:.1}%", stats.cache_hit_rate() * 100.0);
```

### Gated Proof Routing (feature: `gated-proofs`)

Routes proof obligations to the cheapest sufficient compute tier. Inspired by `ruvector-mincut-gated-transformer`'s GateController.

```rust
use ruvector_verified::{ProofEnvironment, gated::{route_proof, ProofKind, ProofTier}};

let env = ProofEnvironment::new();

// Reflexivity -> Reflex tier (~1.2ns)
let decision = route_proof(ProofKind::Reflexivity, &env);
assert!(matches!(decision.tier, ProofTier::Reflex));

// Dimension equality -> Reflex tier (literal comparison)
let decision = route_proof(
    ProofKind::DimensionEquality { expected: 384, actual: 384 },
    &env,
);
assert_eq!(decision.estimated_steps, 1);

// Long pipeline -> Deep tier (full kernel)
let decision = route_proof(
    ProofKind::PipelineComposition { stages: 10 },
    &env,
);
assert!(matches!(decision.tier, ProofTier::Deep));
```

**Tier latency targets:**

| Tier | Latency | Use Case |
|------|---------|----------|
| Reflex | <10ns | `a = a`, literal dimension match |
| Standard | <1us | Shallow type application, short pipelines |
| Deep | <100us | Full kernel with 10,000 step budget |

### ConversionCache

Open-addressing conversion result cache with FxHash. Modeled after `ruvector-mincut`'s PathDistanceCache.

```rust
use ruvector_verified::cache::ConversionCache;

let mut cache = ConversionCache::with_capacity(1024);

cache.insert(/* term_id */ 0, /* ctx_len */ 384, /* result_id */ 42);
assert_eq!(cache.get(0, 384), Some(42));

let stats = cache.stats();
println!("hit rate: {:.1}%", stats.hit_rate() * 100.0);
```

### Thread-Local Pools

Zero-contention resource reuse via Drop-based auto-return.

```rust
use ruvector_verified::pools;

{
    let resources = pools::acquire(); // ~876ns
    // resources.env: fresh ProofEnvironment
    // resources.scratch: reusable HashMap
} // auto-returned to pool on drop

let (acquires, hits, hit_rate) = pools::pool_stats();
```

</details>

<details>
<summary><strong>HNSW Proofs (feature: <code>hnsw-proofs</code>)</strong></summary>

Verified HNSW operations that prove dimensionality and metric compatibility before allowing insert/query.

```rust
use ruvector_verified::{ProofEnvironment, vector_types};

let mut env = ProofEnvironment::new();

// Prove insert preconditions
let vector = vec![1.0f32; 384];
let verified = vector_types::verified_insert(&mut env, 384, &vector, "L2")?;
assert_eq!(verified.value.dim, 384);
assert_eq!(verified.value.metric, "L2");

// Build typed index type term
let index_type = vector_types::mk_hnsw_index_type(&mut env, 384, "Cosine")?;
```

</details>

<details>
<summary><strong>Error Handling</strong></summary>

All errors are typed via `VerificationError`:

```rust
use ruvector_verified::error::VerificationError;

match result {
    Err(VerificationError::DimensionMismatch { expected, actual }) => {
        eprintln!("vector has {actual} dimensions, index expects {expected}");
    }
    Err(VerificationError::TypeCheckFailed(msg)) => {
        eprintln!("type check failed: {msg}");
    }
    Err(VerificationError::ConversionTimeout { max_reductions }) => {
        eprintln!("proof too complex: exceeded {max_reductions} steps");
    }
    Err(VerificationError::ArenaExhausted { allocated }) => {
        eprintln!("arena full: {allocated} terms");
    }
    _ => {}
}
```

**Error variants:** `DimensionMismatch`, `TypeCheckFailed`, `ProofConstructionFailed`, `ConversionTimeout`, `UnificationFailed`, `ArenaExhausted`, `DeclarationNotFound`, `AttestationError`

</details>

<details>
<summary><strong>Built-in Type Declarations</strong></summary>

`ProofEnvironment::new()` pre-registers these domain types:

| Symbol | Arity | Description |
|--------|-------|-------------|
| `Nat` | 0 | Natural numbers (dimensions) |
| `RuVec` | 1 | `RuVec : Nat -> Type` (dimension-indexed vector) |
| `Eq` | 2 | Propositional equality |
| `Eq.refl` | 1 | Reflexivity proof constructor |
| `DistanceMetric` | 0 | L2, Cosine, Dot |
| `HnswIndex` | 2 | `HnswIndex : Nat -> DistanceMetric -> Type` |
| `InsertResult` | 0 | HNSW insert result |
| `PipelineStage` | 2 | `PipelineStage : Type -> Type -> Type` |

</details>

<details>
<summary><strong>Running Benchmarks</strong></summary>

```bash
# All benchmarks
cargo bench -p ruvector-verified --features "ultra,hnsw-proofs"

# Quick run
cargo bench -p ruvector-verified --features "ultra,hnsw-proofs" -- --quick

# Specific group
cargo bench -p ruvector-verified --features ultra -- "prove_dim_eq"
```

**Sample output (AMD EPYC, single core):**

```
prove_dim_eq/384          time:   [496 ns]
mk_vector_type/384        time:   [503 ns]
ProofEnvironment::new     time:   [466 ns]
pool_acquire_release      time:   [876 ns]
env_reset                 time:   [379 ns]
cache_lookup_1000_hits    time:   [9.6 us]
attestation_roundtrip     time:   [<1 ns]
```

</details>

<details>
<summary><strong>End-to-End Example: Kernel-Embedded RVF</strong></summary>

See [`examples/rvf-kernel-optimized`](../../examples/rvf-kernel-optimized/) for a complete example that combines:

- Verified vector ingest with dimension proofs
- Linux kernel + eBPF embedding into RVF containers
- 3-tier gated proof routing
- FastTermArena dedup with 99%+ cache hit rate
- 82-byte proof attestations in the RVF witness chain

```bash
cargo run -p rvf-kernel-optimized
cargo test -p rvf-kernel-optimized
cargo bench -p rvf-kernel-optimized
```

</details>

<details>
<summary><strong>10 Exotic Applications (examples/verified-applications)</strong></summary>

See [`examples/verified-applications`](../../examples/verified-applications/) -- 33 tests across 10 real-world domains:

| # | Domain | Module | What It Proves |
|---|--------|--------|----------------|
| 1 | **Autonomous Weapons Filter** | `weapons_filter` | Sensor dim + metric + 3-stage pipeline composition before firing |
| 2 | **Medical Diagnostics** | `medical_diagnostics` | ECG embedding -> similarity -> risk classifier with regulatory receipts |
| 3 | **Financial Order Routing** | `financial_routing` | Feature dim + metric + risk pipeline with replayable proof hash per trade |
| 4 | **Multi-Agent Contracts** | `agent_contracts` | Per-message dim/metric gate -- logic firewall for agent state transitions |
| 5 | **Sensor Swarm Consensus** | `sensor_swarm` | Node-level dim proofs; divergent nodes detected via proof mismatch |
| 6 | **Quantization Proofs** | `quantization_proof` | Dim preserved + reconstruction error within epsilon = certified transform |
| 7 | **Verifiable AGI Memory** | `verified_memory` | Every insertion has a proof term + witness chain entry + replay audit |
| 8 | **Cryptographic Vector Signatures** | `vector_signatures` | content_hash + model_hash + proof_hash = cross-org trust fabric |
| 9 | **Simulation Integrity** | `simulation_integrity` | Per-step tensor dim proof + pipeline composition = reproducible physics |
| 10 | **Legal Forensics** | `legal_forensics` | Full proof replay, witness chain, structural invariants = mathematical evidence |

```bash
cargo run -p verified-applications    # run all 10 demos
cargo test -p verified-applications   # 33 tests
```

</details>

## Minimum Supported Rust Version

1.77

## License

MIT OR Apache-2.0
