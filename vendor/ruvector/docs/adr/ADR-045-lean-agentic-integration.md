# ADR-045: Lean-Agentic Integration — Formal Verification & AI-Native Type Theory for RuVector

## Status

Proposed

## Date

2026-02-24

## Authors

ruv.io, RuVector Architecture Team

## Deciders

Architecture Review Board

## SDK

Claude-Flow V3

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-24 | ruv.io | Initial deep review and integration proposal |
| 0.2 | 2026-02-24 | ruv.io | Address all gaps: workspace mechanics, error types, compilable code, WASM strategy, CI/CD, proof attestation, testing, benchmarks, migration, compatibility matrix, consequences |
| 0.3 | 2026-02-24 | ruv.io | Ultra-optimization addendum: SolverArena bump alloc, SIMD hash probe, fused substitution, thread-local pools, conversion cache, coherence-gated proof routing, bounds-check elimination. Target: 10-20x speedup to sub-100us proofs |

---

## 1. Executive Summary

This ADR proposes integrating the [`lean-agentic`](https://crates.io/crates/lean-agentic) crate (v0.1.0, Apache-2.0) into the RuVector workspace. Lean-Agentic provides a hash-consed dependent type theory kernel with 150x faster equality checking (0.3ns term comparison), formal verification primitives, and an AI optimization layer including JIT compilation, multi-lane LLM routing, and AgentDB vector memory. Integration gives RuVector proof-carrying vectors, formally verified pipeline invariants, and a compile-time safety layer for critical paths like genomic analysis, financial computation, and cognitive containers.

**Decision**: Add `lean-agentic = "=0.1.0"` as a pinned workspace dependency, creating a new `ruvector-verified` bridge crate (independent versioning at `0.1.0`, initially `publish = false`) that maps RuVector primitives to lean-agentic's type system. All verification is feature-gated — zero impact on existing builds.

**License note**: lean-agentic is Apache-2.0; the RuVector workspace is MIT. These are compatible (Apache-2.0 can be consumed by MIT projects). The new `ruvector-verified` crate will use `MIT OR Apache-2.0` dual licensing to align with both.

---

## 2. Deep Review of `lean-agentic`

### 2.1 Crate Identity

| Field | Value |
|-------|-------|
| **Name** | `lean-agentic` |
| **Version** | 0.1.0 (upstream workspace at 0.3.0) |
| **Published** | 2025-10-25 |
| **License** | Apache-2.0 |
| **Downloads** | ~3,141 total |
| **Crate size** | 19,333 bytes |
| **Code** | 1,871 lines across 10 files (core kernel) |
| **Repository** | [agenticsorg/lean-agentic](https://github.com/agenticsorg/lean-agentic) |
| **Documentation** | [docs.rs/lean-agentic](https://docs.rs/lean-agentic) |
| **Publisher** | rUv (ruvnet) |
| **Categories** | Development tools, Mathematics, WebAssembly |
| **Keywords** | agentic, dependent-types, formal-verification, lean, theorem-prover |

### 2.2 Workspace Architecture

The lean-agentic repository is a 10-crate Rust workspace:

```
lean-agentic/              # Core: hash-consed dependent types (published to crates.io)
leanr-syntax/              # Surface syntax parsing
leanr-elab/                # Elaboration (surface -> core)
leanr-inductive/           # Inductive type definitions
leanr-eval-lite/           # Lightweight evaluation
leanr-compat/              # Lean 4 compatibility layer
leanr-rag-gateway/         # Multi-lane RAG gateway with proof obligations
leanr-wasm/                # WASM bindings (NOT published to crates.io)
leanr-theorems/            # Reference theorem implementations
runtime/                   # Agent runtime with work-stealing scheduler
src/                       # AI optimization layer (AgentDB, JIT, multi-lane)
```

**Important**: Only `lean-agentic` (the core kernel) is published to crates.io. The WASM bindings (`leanr-wasm`) and other workspace crates are not published — WASM support must be built into `ruvector-verified` directly.

### 2.3 Core Kernel Analysis (lean-agentic crate)

The published crate implements a **trusted type theory kernel** for Lean 4 in Rust. Total: ~76.6KB across 10 source files.

#### Module Breakdown

| Module | Size | Purpose |
|--------|------|---------|
| `arena.rs` | 6.6KB | Hash-consing arena with deduplication (85% memory reduction) |
| `term.rs` | 6.5KB | Dependent type terms: Sort, Const, Var, App, Lam, Pi, Let, MVar, Lit |
| `typechecker.rs` | 11.1KB | Trusted kernel: bidirectional type inference/checking |
| `conversion.rs` | 13.3KB | Definitional equality via WHNF (beta/delta/zeta/iota reduction) |
| `unification.rs` | 11.8KB | First-order constraint solving with occurs check |
| `environment.rs` | 9.0KB | Global declarations and constant definitions |
| `context.rs` | 6.1KB | Local variable typing context |
| `level.rs` | 6.3KB | Universe levels for predicative type system |
| `symbol.rs` | 3.7KB | Name interning for memory-efficient identifiers |
| `lib.rs` | 2.3KB | Module exports and error types |

#### Key Data Structures

```rust
// Hash-consed term identifier -- O(1) equality via pointer comparison
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TermId(u32);

// Core term variants
pub enum TermKind {
    Sort(LevelId),                 // Type universes
    Const(SymbolId, Vec<LevelId>), // Named constants
    Var(u32),                      // De Bruijn indexed variables
    App(TermId, TermId),           // Function application
    Lam(Binder, TermId),           // Lambda abstraction
    Pi(Binder, TermId),            // Dependent function types
    Let(Binder, TermId, TermId),   // Let bindings
    MVar(MetaVarId),               // Metavariables for elaboration
    Lit(Literal),                  // Nat/String literals
}

// Binder with implicit/explicit annotation
pub struct Binder {
    pub name: SymbolId,
    pub ty: TermId,
    pub implicit: bool,
    pub info: BinderInfo, // Default, Implicit, StrictImplicit, InstImplicit
}

// Arena with deduplication cache
pub struct Arena {
    terms: Vec<Term>,
    cache: HashMap<u64, Vec<TermId>>,
    stats: ArenaStats,
}
```

#### Type Checker Architecture

The type checker implements a **bidirectional** algorithm:

1. **`infer(term)`** -- Synthesizes a type for a term:
   - Sort: returns Sort(level+1)
   - Const: looked up in environment
   - Var: looked up in context
   - App: infers function type, WHNF-reduces to Pi, checks argument
   - Lam: extends context, infers body type, constructs Pi
   - Pi: checks domain and codomain are sorts, computes max level
   - Let: type-checks value, substitutes into body

2. **`check(term, expected_type)`** -- Verifies term has expected type via definitional equality

3. **`check_declaration(decl)`** -- Validates declarations before environment admission

#### Conversion Engine

WHNF reduction with fuel (10,000 step limit):
- **Beta reduction**: `(Lx.t) s -> t[x:=s]`
- **Delta reduction**: Constant unfolding for reducible definitions
- **Zeta reduction**: Let-expression substitution
- **Iota reduction**: Pattern matching (placeholder)
- **Memoization cache** keyed on (term, context_length)
- **Substitution** with proper De Bruijn index shifting

#### Unification Engine

First-order unification for dependent types:
- Three constraint types: `Unify(t1, t2)`, `IsSort(t)`, `HasType(m, t)`
- Occurs check preventing infinite types
- Structural decomposition of App, Lam, Pi
- Fixed-point substitution application

### 2.4 AI Optimization Layer (workspace src/)

Beyond the core kernel, the upstream workspace provides four AI-native modules (not published to crates.io, listed for context):

| Module | Purpose |
|--------|---------|
| **AgentDB** | Vector memory with `AgentDb`, `AgentDbConfig`, `Episode`, `SemanticFact` |
| **LLM Compiler** | AI-driven compilation with optimization passes |
| **JIT Runtime** | 4-tier adaptive JIT (interpreter to 200x optimization) |
| **Multi-Lane** | Cost-optimized LLM routing (40%+ savings) |

### 2.5 Performance Characteristics

| Metric | Value | Notes |
|--------|-------|-------|
| Term equality | 0.3ns | Hash-consed pointer comparison |
| Memory reduction | 85% | Arena deduplication |
| Agent spawn | <500ns | Work-stealing scheduler |
| Vector search P99 | <10ms | AgentDB integration |
| Cache hit rate | 95%+ | Memoized WHNF |
| Compilation | <100ms | Incremental, function-level |
| Key generation | 152us | Ed25519 proof signatures |

### 2.6 Dependency Profile

The published `lean-agentic` crate has **zero mandatory dependencies**. Optional feature:
- `serde` -- serialization support

The upstream workspace-level dependencies (not pulled by the published crate) include:
- `tokio` (async), `serde`/`bincode` (serialization)
- `ed25519-dalek` (proof signatures), `sha2` (hashing)
- `bumpalo` (arena allocator), `im` (persistent collections)
- `wasm-bindgen`/`js-sys`/`web-sys` (WASM target)

### 2.7 Quality Assessment

| Criteria | Rating | Notes |
|----------|--------|-------|
| **API surface** | Clean | 8 modules, well-separated concerns |
| **Safety** | Strong | `#![deny(unsafe_op_in_unsafe_fn)]`, hash-consing prevents aliasing bugs |
| **Documentation** | Good | `#![warn(missing_docs)]` enforced, docs.rs published |
| **Testing** | Adequate | Unit tests per module, 50+ tests workspace-wide |
| **Maturity** | Early | v0.1.0, but sound type-theory foundation |
| **License** | Compatible | Apache-2.0 (consumable by MIT projects) |
| **Size** | Minimal | 19KB crate, zero mandatory deps |
| **Maintenance** | Active | Same org (agenticsorg), same maintainer (ruvnet) |

**`no_std` compatibility**: The core kernel uses `HashMap` (std). For WASM targets (`wasm32-unknown-unknown`), `HashMap` is available because WASM has std support (no mmap/fs needed). For `no_std` builds, the arena would need a `hashbrown` replacement -- this is a Phase 4 concern.

---

## 3. Integration Rationale

### 3.1 Why Lean-Agentic for RuVector

RuVector's crate ecosystem handles high-stakes computation: genomic analysis, financial risk, graph neural networks, quantum simulation. These domains demand **correctness guarantees** beyond what unit tests alone provide.

Lean-Agentic provides:

1. **Proof-Carrying Vectors** -- Attach type-theoretic proofs to HNSW index operations, certifying dimensionality, distance metric correctness, and recall bounds.

2. **Verified Pipeline Invariants** -- Use dependent types to encode that DNA pipeline stages (ADR-001 through ADR-015) preserve data integrity across transformations.

3. **Formal Safety for Cognitive Containers** -- RVF containers (ADR-029/030) can carry machine-checked proofs of their behavioral contracts, chaining into the existing WITNESS_SEG format.

4. **Zero-Cost at Runtime** -- Compile-time proof erasure means zero overhead in release builds. Only the 0.3ns equality check remains for runtime assertions.

5. **Shared Toolchain** -- Same maintainer, same org, Rust-native. No FFI boundaries or language impedance.

### 3.2 RuVector Integration Points

| RuVector Crate | Integration | Lean-Agentic Module | Phase |
|---------------|-------------|---------------------|-------|
| `ruvector-core` (HNSW) | Proven dimensionality/metric invariants | `typechecker`, `term` | 2 |
| `ruvector-attention` | Verified attention mask shapes | `typechecker`, `conversion` | 4 |
| `ruvector-solver` | Proven convergence properties | `unification`, `environment` | 4 |
| `ruvector-coherence` | Formal sheaf consistency proofs | `typechecker`, `level` | 4 |
| `ruvector-gnn` | Verified graph topology invariants | `term`, `arena` | 4 |
| `ruvector-delta-consensus` | Proven CRDT merge commutativity | `conversion`, `unification` | 4 |
| `prime-radiant` | Formal categorical coherence | `level`, `typechecker` | 4 |
| `cognitum-gate-kernel` | Verified gate predicates | `typechecker`, `environment` | 4 |
| `ruvector-temporal-tensor` | Proven temporal ordering invariants | `term`, `conversion` | 4 |
| `ruvector-cognitive-container` | Proof-carrying cognitive containers via `WitnessChain` | Full kernel | 3 |
| DNA pipeline (`examples/dna`) | Verified genomic transformation chain | Full kernel | 3 |

**Interop note for `ruvector-coherence`**: The coherence crate currently has a `spectral` feature flag and depends only on `serde`/`serde_json`. Sheaf consistency proofs will live in `ruvector-verified` (not in `ruvector-coherence`) to avoid adding lean-agentic as a dependency to the coherence crate. `ruvector-verified` will import `ruvector-coherence` types and wrap them.

**Interop note for `ruvector-cognitive-container`**: This crate already exports `WitnessChain`, `CoherenceDecision`, and `ContainerWitnessReceipt`. `ruvector-verified` will produce `ProofAttestation` values that serialize into `WitnessChain` entries -- no parallel types, just a producer-consumer relationship.

---

## 4. Design

### 4.1 Workspace Changes

#### Diff to `/workspaces/ruvector/Cargo.toml`

Add to `[workspace.members]` (after `ruvector-cognitive-container`):

```toml
    "crates/ruvector-verified",
```

Add to `[workspace.dependencies]`:

```toml
# Formal verification
lean-agentic = "=0.1.0"
```

Note: `optional = true` is NOT valid in `[workspace.dependencies]`. Optionality is declared in each consuming crate's `[dependencies]` section via `optional = true`.

This adds one entry to `Cargo.lock` (lean-agentic itself, which has zero transitive deps without the `serde` feature).

#### Version Strategy

`ruvector-verified` uses **independent versioning** at `0.1.0`, not `version.workspace = true` (which would give it `2.0.4`). Rationale: this is an experimental bridge crate; it should not inherit the mature workspace version until it reaches feature parity with other `2.x` crates.

### 4.2 New Crate: `ruvector-verified`

```
crates/
  ruvector-verified/
    Cargo.toml
    src/
      lib.rs              # Public API, re-exports, ProofEnvironment
      error.rs            # VerificationError enum
      vector_types.rs     # Dependent types for vector operations
      proof_store.rs      # Ed25519-signed proof attestation
      pipeline.rs         # Verified pipeline composition
      invariants.rs       # Pre-built invariant library
    benches/
      proof_generation.rs # Criterion benchmarks
      arena_throughput.rs
```

#### `Cargo.toml`

```toml
[package]
name = "ruvector-verified"
version = "0.1.0"
edition = "2021"
rust-version = "1.77"
license = "MIT OR Apache-2.0"
description = "Formal verification layer for RuVector using lean-agentic dependent types"
publish = false  # Until Phase 2 complete

[dependencies]
# Core verification kernel (always required -- this crate IS the verification layer)
lean-agentic = { workspace = true }
thiserror = { workspace = true }

# Optional integrations with RuVector crates
ruvector-core = { path = "../ruvector-core", optional = true, default-features = false }
ruvector-coherence = { path = "../ruvector-coherence", optional = true }
ruvector-cognitive-container = { path = "../ruvector-cognitive-container", optional = true }
rvf-types = { path = "../rvf/rvf-types", optional = true }
rvf-crypto = { path = "../rvf/rvf-crypto", optional = true, features = ["ed25519"] }

# Serialization (for proof persistence)
serde = { workspace = true, optional = true }
serde_json = { workspace = true, optional = true }

[dev-dependencies]
criterion = { workspace = true }
proptest = { workspace = true }

[features]
default = []
hnsw-proofs = ["dep:ruvector-core", "ruvector-core/hnsw"]
rvf-proofs = ["dep:rvf-types", "dep:rvf-crypto", "dep:ruvector-cognitive-container"]
coherence-proofs = ["dep:ruvector-coherence"]
serde = ["dep:serde", "dep:serde_json", "lean-agentic/serde"]
all-proofs = ["hnsw-proofs", "rvf-proofs", "coherence-proofs"]

[[bench]]
name = "proof_generation"
harness = false

[[bench]]
name = "arena_throughput"
harness = false
```

#### Feature Propagation

```
ruvector-verified
  |-- [always]    lean-agentic =0.1.0
  |-- [always]    thiserror 2.0
  |-- [hnsw-proofs]    ruvector-core (hnsw feature)
  |-- [rvf-proofs]     rvf-types + rvf-crypto (ed25519) + ruvector-cognitive-container
  |-- [coherence-proofs] ruvector-coherence
  |-- [serde]          serde + serde_json + lean-agentic/serde

Downstream crates opt in:
  ruvector-core/Cargo.toml:
    [features]
    formal-verification = ["dep:ruvector-verified", "ruvector-verified/hnsw-proofs"]
```

### 4.3 Error Types

```rust
// crates/ruvector-verified/src/error.rs

use thiserror::Error;

/// Errors from the formal verification layer.
#[derive(Debug, Error)]
pub enum VerificationError {
    /// Vector dimension does not match the index dimension.
    /// Contains (expected, actual).
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: u32, actual: u32 },

    /// The lean-agentic type checker rejected the proof term.
    /// Contains the kernel error message.
    #[error("type check failed: {0}")]
    TypeCheckFailed(String),

    /// Proof construction failed during term building.
    #[error("proof construction failed: {0}")]
    ProofConstructionFailed(String),

    /// The conversion engine exhausted its fuel budget (10,000 reductions).
    #[error("conversion timeout: exceeded {max_reductions} reduction steps")]
    ConversionTimeout { max_reductions: u32 },

    /// Unification of proof constraints failed.
    #[error("unification failed: {0}")]
    UnificationFailed(String),

    /// The arena ran out of term slots (u32 overflow at 4B terms).
    #[error("arena exhausted: {allocated} terms allocated")]
    ArenaExhausted { allocated: u32 },

    /// A required declaration was not found in the proof environment.
    #[error("declaration not found: {name}")]
    DeclarationNotFound { name: String },

    /// Ed25519 proof signing or verification failed.
    #[error("attestation error: {0}")]
    AttestationError(String),
}

/// Maps lean-agentic's internal `lean_agentic::Error` to `VerificationError`.
impl From<lean_agentic::Error> for VerificationError {
    fn from(e: lean_agentic::Error) -> Self {
        match e {
            lean_agentic::Error::TypeError(msg) => Self::TypeCheckFailed(msg),
            lean_agentic::Error::UniverseError(msg) => Self::TypeCheckFailed(msg),
            lean_agentic::Error::UnificationError(msg) => Self::UnificationFailed(msg),
            lean_agentic::Error::NotFound(msg) => Self::DeclarationNotFound { name: msg },
            lean_agentic::Error::ConversionError { expected, actual } => {
                Self::TypeCheckFailed(format!("expected {expected}, got {actual}"))
            }
            lean_agentic::Error::Internal(msg) => Self::ProofConstructionFailed(msg),
        }
    }
}

pub type Result<T> = std::result::Result<T, VerificationError>;
```

### 4.4 Core Types and API

```rust
// crates/ruvector-verified/src/lib.rs

pub mod error;
pub mod vector_types;
pub mod proof_store;
pub mod pipeline;
pub mod invariants;

pub use error::{VerificationError, Result};
pub use vector_types::{mk_vector_type, mk_nat_literal};
pub use proof_store::ProofAttestation;
pub use pipeline::VerifiedStage;

use lean_agentic::{Arena, Context, Environment, TypeChecker};

/// The proof environment bundles lean-agentic's kernel state.
/// One per thread (not Send/Sync due to Arena interior mutability).
pub struct ProofEnvironment {
    /// Hash-consing arena for term allocation.
    pub arena: Arena,
    /// Global declarations (vector types, distance metrics, etc.).
    pub env: Environment,
    /// Local typing context for the current proof obligation.
    pub ctx: Context,
    /// Type checker instance.
    pub checker: TypeChecker,
}

impl ProofEnvironment {
    /// Create a new proof environment pre-loaded with RuVector type declarations.
    pub fn new() -> Self {
        let mut arena = Arena::new();
        let mut env = Environment::new();
        let ctx = Context::new();
        let checker = TypeChecker::default();

        // Register built-in types: Nat, Vec, DistanceMetric
        invariants::register_builtins(&mut arena, &mut env);

        Self { arena, env, ctx, checker }
    }

    /// Get arena statistics (cache hit rate, terms allocated).
    pub fn stats(&self) -> &lean_agentic::ArenaStats {
        self.arena.stats()
    }
}

impl Default for ProofEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

/// A vector operation with a machine-checked type proof.
pub struct VerifiedOp<T> {
    /// The operation result.
    pub value: T,
    /// Proof term ID in the arena.
    /// In release builds without debug_assertions, this is still present
    /// but can be ignored (it is a Copy u32, zero overhead).
    pub proof: lean_agentic::TermId,
}
```

```rust
// crates/ruvector-verified/src/vector_types.rs

use lean_agentic::{Arena, Environment, TermId, SymbolId, Binder, BinderInfo};
use crate::error::{Result, VerificationError};

/// Construct a Nat literal term in the arena.
///
/// Nat values are used as dimension indices for dependent vector types.
pub fn mk_nat_literal(arena: &mut Arena, n: u32) -> TermId {
    arena.mk_const(
        arena.symbol_table().intern("Nat"),
        vec![],
    )
    // For dimension checking, we use Lit(Nat(n)) directly:
    // arena.intern(TermKind::Lit(Literal::Nat(n as u64)))
}

/// Construct the type `RuVec n` representing a vector of dimension `n`.
///
/// In the type theory: `RuVec : Nat -> Type`
/// Applied as: `RuVec 128` for a 128-dimensional vector.
///
/// # Example
/// ```rust,ignore
/// let mut proof_env = ProofEnvironment::new();
/// let vec128_ty = mk_vector_type(&mut proof_env.arena, &mut proof_env.env, 128);
/// // vec128_ty represents the type `RuVec 128`
/// ```
pub fn mk_vector_type(arena: &mut Arena, env: &Environment, dim: u32) -> TermId {
    // Look up the pre-registered RuVec constant
    let ruvec_sym = arena.symbol_table().intern("RuVec");
    let ruvec_const = arena.mk_const(ruvec_sym, vec![]);

    // Construct the dimension as a Nat literal
    let dim_term = arena.intern(lean_agentic::TermKind::Lit(
        lean_agentic::Literal::Nat(dim as u64),
    ));

    // Apply: RuVec dim
    arena.mk_app(ruvec_const, dim_term)
}

/// Prove that two dimensions are equal, returning the proof term.
///
/// If `expected != actual`, returns `DimensionMismatch` error.
/// If equal, constructs a `refl` proof term: `Eq.refl : expected = actual`.
pub fn prove_dim_eq(
    arena: &mut Arena,
    expected: u32,
    actual: u32,
) -> Result<TermId> {
    if expected != actual {
        return Err(VerificationError::DimensionMismatch { expected, actual });
    }

    // Construct: refl {Nat} {expected}
    let refl_sym = arena.symbol_table().intern("Eq.refl");
    let nat_lit = arena.intern(lean_agentic::TermKind::Lit(
        lean_agentic::Literal::Nat(expected as u64),
    ));
    let refl_const = arena.mk_const(refl_sym, vec![]);
    Ok(arena.mk_app(refl_const, nat_lit))
}

/// Verified HNSW insert: proves dimensionality match before insertion.
///
/// # Type signature in dependent types:
/// ```text
/// verified_insert : (idx : HnswIndex n) -> (v : RuVec m) -> (p : n = m) -> InsertResult
/// ```
#[cfg(feature = "hnsw-proofs")]
pub fn verified_insert(
    index_dim: u32,
    vector: &[f32],
    proof_env: &mut crate::ProofEnvironment,
) -> Result<VerifiedOp<()>> {
    let actual_dim = vector.len() as u32;

    // 1. Construct proof term: dim_eq : index_dim = vector.len()
    let proof = prove_dim_eq(&mut proof_env.arena, index_dim, actual_dim)?;

    // 2. Type-check the proof in the lean-agentic kernel
    let expected_ty = {
        let eq_sym = proof_env.arena.symbol_table().intern("Eq");
        let n = proof_env.arena.intern(lean_agentic::TermKind::Lit(
            lean_agentic::Literal::Nat(index_dim as u64),
        ));
        let eq_const = proof_env.arena.mk_const(eq_sym, vec![]);
        proof_env.arena.mk_app(proof_env.arena.mk_app(eq_const, n), n)
    };

    proof_env.checker.check(
        &proof_env.arena,
        &proof_env.env,
        &proof_env.ctx,
        proof,
        expected_ty,
    ).map_err(VerificationError::from)?;

    // 3. Return verified result (actual HNSW insert is caller's responsibility)
    Ok(VerifiedOp { value: (), proof })
}

use crate::VerifiedOp;
```

```rust
// crates/ruvector-verified/src/pipeline.rs

use std::marker::PhantomData;
use lean_agentic::TermId;

/// A verified pipeline stage with proven input/output type compatibility.
///
/// `A` and `B` are phantom type parameters representing the stage's
/// logical input and output types (not runtime types).
///
/// The `proof` field contains a lean-agentic term proving that the
/// stage's implementation correctly transforms `A` to `B`.
pub struct VerifiedStage<A, B> {
    /// Human-readable stage name (e.g., "kmer_embedding", "variant_call").
    pub name: String,
    /// Proof term: `stage_correct : A -> B` is well-typed.
    pub proof: TermId,
    /// Input type term in the arena.
    pub input_ty: TermId,
    /// Output type term in the arena.
    pub output_ty: TermId,
    _phantom: PhantomData<(A, B)>,
}

impl<A, B> VerifiedStage<A, B> {
    /// Create a new verified stage with its correctness proof.
    pub fn new(name: String, proof: TermId, input_ty: TermId, output_ty: TermId) -> Self {
        Self {
            name,
            proof,
            input_ty,
            output_ty,
            _phantom: PhantomData,
        }
    }
}

/// Compose two verified stages, producing a proof that the pipeline is type-safe.
///
/// Checks that `f.output_ty` is definitionally equal to `g.input_ty` using
/// the lean-agentic conversion engine.
///
/// # Errors
/// Returns `TypeCheckFailed` if the output type of `f` does not match
/// the input type of `g`.
pub fn compose_stages<A, B, C>(
    f: &VerifiedStage<A, B>,
    g: &VerifiedStage<B, C>,
    proof_env: &mut crate::ProofEnvironment,
) -> crate::Result<VerifiedStage<A, C>> {
    // Verify output(f) = input(g) via definitional equality
    let converter = lean_agentic::Converter::default();
    let eq = converter.is_def_eq(
        &proof_env.arena,
        &proof_env.env,
        &proof_env.ctx,
        f.output_ty,
        g.input_ty,
    );

    if !eq {
        return Err(crate::VerificationError::TypeCheckFailed(format!(
            "pipeline type mismatch: stage '{}' output != stage '{}' input",
            f.name, g.name,
        )));
    }

    // Construct composed proof: g . f
    let composed = proof_env.arena.mk_app(g.proof, f.proof);

    Ok(VerifiedStage::new(
        format!("{} >> {}", f.name, g.name),
        composed,
        f.input_ty,
        g.output_ty,
    ))
}
```

### 4.5 Proof Attestation and RVF Witness Integration

```rust
// crates/ruvector-verified/src/proof_store.rs

/// A proof attestation that can be serialized into an RVF WITNESS_SEG entry.
///
/// Witness type `0x0E` = FORMAL_PROOF_VERIFICATION (new type code).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProofAttestation {
    /// SHAKE-256 hash of the serialized proof term.
    pub proof_term_hash: [u8; 32],
    /// SHAKE-256 hash of the environment (declarations used).
    pub environment_hash: [u8; 32],
    /// Nanosecond UNIX timestamp of verification.
    pub verification_timestamp_ns: u64,
    /// lean-agentic version that performed the check (0x00_01_00_00 = 0.1.0).
    pub verifier_version: u32,
    /// Number of type-check reduction steps consumed.
    pub reduction_steps: u32,
    /// Arena cache hit rate at time of verification (0..10000 = 0.00%..100.00%).
    pub cache_hit_rate_bps: u16,
}

/// Witness type code for formal verification proofs.
/// Extends the existing codes: 0x01=PROVENANCE, 0x02=COMPUTATION.
pub const WITNESS_TYPE_FORMAL_PROOF: u8 = 0x0E;

/// Convert a `ProofAttestation` into an `rvf_crypto::WitnessEntry` for chaining
/// into an existing WITNESS_SEG.
#[cfg(feature = "rvf-proofs")]
pub fn to_witness_entry(
    attestation: &ProofAttestation,
    prev_hash: [u8; 32],
) -> rvf_crypto::WitnessEntry {
    rvf_crypto::WitnessEntry {
        prev_hash,
        action_hash: attestation.proof_term_hash,
        timestamp_ns: attestation.verification_timestamp_ns,
        witness_type: WITNESS_TYPE_FORMAL_PROOF,
    }
}

/// Sign a proof attestation with Ed25519 using rvf-crypto's signing infrastructure.
///
/// The signed attestation can be embedded in an RVF container alongside
/// TEE attestation quotes (ADR-042). When both are present, the container
/// carries dual certification: mathematical proof AND hardware attestation.
#[cfg(feature = "rvf-proofs")]
pub fn sign_attestation(
    attestation: &ProofAttestation,
    header: &rvf_types::SegmentHeader,
    key: &ed25519_dalek::SigningKey,
) -> rvf_crypto::SignatureFooter {
    let payload = attestation_to_bytes(attestation);
    rvf_crypto::sign_segment(header, &payload, key)
}

/// Serialize attestation to bytes for signing/hashing.
fn attestation_to_bytes(a: &ProofAttestation) -> Vec<u8> {
    let mut buf = Vec::with_capacity(78);
    buf.extend_from_slice(&a.proof_term_hash);
    buf.extend_from_slice(&a.environment_hash);
    buf.extend_from_slice(&a.verification_timestamp_ns.to_le_bytes());
    buf.extend_from_slice(&a.verifier_version.to_le_bytes());
    buf.extend_from_slice(&a.reduction_steps.to_le_bytes());
    buf.extend_from_slice(&a.cache_hit_rate_bps.to_le_bytes());
    buf
}
```

#### Proof Attestation Flow

```
                          Feature: rvf-proofs
                          ~~~~~~~~~~~~~~~~~~~~
+--------------+    +----------------+    +--------------------+
| RuVector Op  |--->| lean-agentic   |--->| ProofAttestation   |
| (insert,     |    | TypeChecker    |    | proof_term_hash    |
|  query, etc) |    | + Converter    |    | environment_hash   |
+--------------+    +----------------+    | timestamp_ns       |
                                          | verifier_version   |
                                          +--------+-----------+
                                                   |
                                          +--------v-----------+
                                          | to_witness_entry() |
                                          | witness_type=0x0E  |
                                          +--------+-----------+
                                                   |
                                          +--------v-----------+
                                          | rvf_crypto::       |
                                          | create_witness_    |
                                          | chain()            |
                                          +--------+-----------+
                                                   |
                                          +--------v-----------+
                                          | WITNESS_SEG in     |
                                          | .rvf container     |
                                          | (+ optional TEE    |
                                          |  attestation from  |
                                          |  ADR-042)          |
                                          +--------------------+
```

---

## 5. Compatibility Matrix

| Dependency | lean-agentic requires | RuVector workspace has | Compatible? |
|------------|----------------------|----------------------|-------------|
| **Rust MSRV** | edition 2021 (1.56+) | `rust-version = "1.77"` | Yes (1.77 >= 1.56) |
| **serde** | `1.0` (optional) | `serde = "1.0"` | Yes (identical) |
| **ed25519-dalek** | `2` (upstream workspace only) | `rvf-crypto`: `ed25519-dalek = "2"` | Yes (same major) |
| **HashMap** | std (arena.rs) | std available on all targets | Yes |
| **thiserror** | Not used | `thiserror = "2.0"` | N/A (ruvector-verified uses 2.0) |
| **no_std** | Not supported (uses HashMap) | WASM targets use std | OK for wasm32-unknown-unknown |

**Version pinning**: `lean-agentic = "=0.1.0"` (exact pin). The bridge crate insulates downstream from API changes. When lean-agentic releases 0.2.0, we update the pin and adapt the bridge -- downstream crates see no change.

---

## 6. WASM Strategy

### 6.1 Target

`wasm32-unknown-unknown` (same as `ruvector-wasm`, `rvf-wasm`, `ruvector-solver-wasm`).

### 6.2 Approach

The published `lean-agentic` crate compiles to WASM because:
- Zero mandatory dependencies
- Uses `HashMap` from std (available in `wasm32-unknown-unknown`)
- No filesystem, mmap, or OS-specific APIs
- No `unsafe` blocks that assume pointer widths

`leanr-wasm` (the upstream WASM binding crate) is **NOT published to crates.io**. Therefore `ruvector-verified` will provide its own WASM surface using `wasm-bindgen`, following the pattern of `ruvector-solver-wasm`.

### 6.3 Binary Size Budget

| Component | Estimated Size |
|-----------|---------------|
| lean-agentic kernel (arena + typechecker + conversion) | ~40KB |
| ruvector-verified bridge logic | ~15KB |
| wasm-bindgen glue | ~10KB |
| **Total (wasm-opt -Oz)** | **< 80KB** |

For reference: `rvf-solver-wasm` is 132KB post-optimization. The verification WASM should be smaller due to zero floating-point math.

### 6.4 Phase 4 Deliverable

A new `crates/ruvector-verified-wasm/` crate with `wasm-bindgen` exports:

```rust
#[wasm_bindgen]
pub fn verify_dimension_proof(index_dim: u32, vector_dim: u32) -> bool;

#[wasm_bindgen]
pub fn create_proof_environment() -> *mut ProofEnvironment;

#[wasm_bindgen]
pub fn free_proof_environment(ptr: *mut ProofEnvironment);
```

---

## 7. Testing Strategy

### 7.1 Unit Tests (Phase 1)

| Test Name | Description | Input | Expected |
|-----------|-------------|-------|----------|
| `test_dim_eq_same` | Equal dimensions produce valid proof | `prove_dim_eq(128, 128)` | `Ok(TermId)` |
| `test_dim_eq_mismatch` | Unequal dimensions error | `prove_dim_eq(128, 256)` | `Err(DimensionMismatch{128, 256})` |
| `test_mk_vector_type` | Vector type construction | `mk_vector_type(arena, env, 128)` | `TermId` for `RuVec 128` |
| `test_proof_env_builtins` | Environment has RuVec, Nat, Eq.refl | `ProofEnvironment::new()` | No panic, symbols interned |
| `test_arena_cache_rate` | Cache efficiency under duplication | Create 1000 identical terms | Cache hit rate > 95% |

### 7.2 Integration Tests (Phase 2)

| Test Name | Description |
|-----------|-------------|
| `test_verified_insert_roundtrip` | `verified_insert` succeeds for matching dims, rejects mismatch |
| `test_proof_attestation_witness_chain` | `ProofAttestation` -> `WitnessEntry` -> `create_witness_chain` -> `verify_witness_chain` round-trip |
| `test_proof_signing_verification` | Ed25519 sign + verify attestation via rvf-crypto |

### 7.3 Property-Based Tests (proptest)

```rust
proptest! {
    #[test]
    fn dim_match_always_succeeds(dim in 1u32..10_000) {
        let mut env = ProofEnvironment::new();
        let result = verified_insert(dim, &vec![0.0f32; dim as usize], &mut env);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn dim_mismatch_always_fails(
        index_dim in 1u32..10_000,
        vector_dim in 1u32..10_000,
    ) {
        prop_assume!(index_dim != vector_dim);
        let mut env = ProofEnvironment::new();
        let result = verified_insert(index_dim, &vec![0.0f32; vector_dim as usize], &mut env);
        prop_assert!(matches!(result, Err(VerificationError::DimensionMismatch { .. })));
    }

    #[test]
    fn arena_never_panics(n in 1u32..100_000) {
        let mut arena = Arena::new();
        for i in 0..n {
            arena.intern(TermKind::Lit(Literal::Nat(i as u64)));
        }
        prop_assert!(arena.terms() <= n as usize);
    }
}
```

### 7.4 Negative / Soundness Tests

| Test Name | Description |
|-----------|-------------|
| `test_reject_unsound_proof` | Manually construct an ill-typed proof term; verify `checker.check()` rejects it |
| `test_conversion_fuel_exhaustion` | Create a term requiring >10,000 reductions; verify `ConversionTimeout` error |
| `test_occurs_check_prevents_infinite` | Attempt circular unification; verify `UnificationFailed` error |

### 7.5 Benchmark Harness

```rust
// benches/proof_generation.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_verified_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("verified_insert");
    for dim in [32, 128, 512, 1024, 4096] {
        group.bench_with_input(
            BenchmarkId::from_parameter(dim),
            &dim,
            |b, &dim| {
                let vector = vec![0.0f32; dim];
                b.iter(|| {
                    let mut env = ProofEnvironment::new();
                    verified_insert(dim as u32, &vector, &mut env).unwrap();
                });
            },
        );
    }
    group.finish();
}

fn bench_proof_vs_unverified(c: &mut Criterion) {
    let mut group = c.benchmark_group("overhead_comparison");
    let dim = 128;
    let vector = vec![0.0f32; dim];

    group.bench_function("with_proof", |b| {
        b.iter(|| {
            let mut env = ProofEnvironment::new();
            verified_insert(dim as u32, &vector, &mut env).unwrap();
        });
    });

    group.bench_function("without_proof", |b| {
        b.iter(|| {
            // Raw dimension check (no proof generation)
            assert_eq!(vector.len(), dim);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_verified_insert, bench_proof_vs_unverified);
criterion_main!(benches);
```

```rust
// benches/arena_throughput.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_arena_intern_unique(c: &mut Criterion) {
    c.bench_function("arena_intern_10k_unique", |b| {
        b.iter(|| {
            let mut arena = Arena::new();
            for i in 0u64..10_000 {
                arena.intern(TermKind::Lit(Literal::Nat(i)));
            }
        });
    });
}

fn bench_arena_intern_dedup(c: &mut Criterion) {
    c.bench_function("arena_intern_10k_dedup", |b| {
        b.iter(|| {
            let mut arena = Arena::new();
            for _ in 0..10_000 {
                arena.intern(TermKind::Lit(Literal::Nat(42)));
            }
            assert!(arena.cache_hit_rate() > 0.99);
        });
    });
}

criterion_group!(benches, bench_arena_intern_unique, bench_arena_intern_dedup);
criterion_main!(benches);
```

---

## 8. CI/CD Changes

### 8.1 New Workflow: `.github/workflows/build-verified.yml`

```yaml
name: ruvector-verified

on:
  push:
    paths:
      - 'crates/ruvector-verified/**'
      - 'Cargo.lock'
  pull_request:
    paths:
      - 'crates/ruvector-verified/**'

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        features:
          - ""                    # No features (core only)
          - "hnsw-proofs"         # HNSW integration
          - "rvf-proofs"          # RVF witness chain
          - "all-proofs"          # Everything
          - "all-proofs,serde"    # Everything + serialization
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test -p ruvector-verified --features "${{ matrix.features }}"
      - run: cargo test -p ruvector-verified --features "${{ matrix.features }}" --release

  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo bench -p ruvector-verified --features all-proofs -- --output-format bencher

  no-default-features:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo check -p ruvector-verified --no-default-features
```

### 8.2 Existing Workflow Updates

Add to the main `build-native.yml` matrix (if present):

```yaml
- run: cargo check -p ruvector-verified --features all-proofs
```

### 8.3 WASM CI (Phase 4)

```yaml
  wasm:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - run: cargo build -p ruvector-verified --target wasm32-unknown-unknown --no-default-features
      - run: |
          wasm_size=$(stat -c%s target/wasm32-unknown-unknown/release/ruvector_verified.wasm)
          if [ "$wasm_size" -gt 81920 ]; then
            echo "WASM binary too large: ${wasm_size} bytes (limit: 80KB)"
            exit 1
          fi
```

---

## 9. Migration Path

### 9.1 Impact on Existing Users

| Concern | Answer |
|---------|--------|
| **Existing crate APIs change?** | No. Zero changes to any existing public API. |
| **Default features change?** | No. No existing crate gains new default features. |
| **New transitive dependencies?** | Only if `formal-verification` feature is enabled. Otherwise, `Cargo.lock` gains one entry (`lean-agentic`), but it is never compiled. |
| **SemVer implications** | Adding `formal-verification` as a non-default feature to `ruvector-core` is a minor version bump (2.0.4 -> 2.1.0) under Rust SemVer conventions. |
| **Breaking changes** | None. The new crate is a leaf with no reverse dependencies initially. |

### 9.2 Adoption Path

1. **Phase 1-2**: `ruvector-verified` exists as an independent crate. Users who want verification add it explicitly: `cargo add ruvector-verified --features hnsw-proofs`.
2. **Phase 3+**: Downstream crates (e.g., `ruvector-core`) gain optional `formal-verification` feature flags. Users enable with: `cargo build --features ruvector-core/formal-verification`.
3. **No existing behavior changes** at any phase. Verification is purely additive.

### 9.3 Publishing Sequence

1. `lean-agentic` is already published (v0.1.0 on crates.io). No action needed.
2. `ruvector-verified` starts with `publish = false`.
3. When Phase 2 is complete and API stabilizes: set `publish = true`, publish as `ruvector-verified = "0.1.0"`.
4. `ruvector-verified-wasm` (Phase 4): publish after `ruvector-verified`.

---

## 10. Implementation Plan

### Phase 1: Foundation (Week 1-2)

1. Add `lean-agentic = "=0.1.0"` to `[workspace.dependencies]`
2. Add `"crates/ruvector-verified"` to `[workspace.members]`
3. Create `crates/ruvector-verified/` with `Cargo.toml` as specified in Section 4.2
4. Implement `error.rs` with full `VerificationError` enum (Section 4.3)
5. Implement `ProofEnvironment` (Section 4.4)
6. Implement `vector_types.rs`: `mk_vector_type`, `mk_nat_literal`, `prove_dim_eq`
7. Implement `invariants.rs`: `register_builtins` for Nat, RuVec, Eq.refl
8. Unit tests (Section 7.1): all 5 tests passing
9. Benchmark: `arena_throughput` bench running, baseline established

**Exit criteria**: `cargo test -p ruvector-verified` passes. `cargo bench -p ruvector-verified` runs.

### Phase 2: Core Integration (Week 3-4)

1. Enable `hnsw-proofs` feature, implement `verified_insert`
2. Add `rvf-proofs` feature, implement `proof_store.rs` (Section 4.5)
3. Implement `to_witness_entry()` and `sign_attestation()`
4. Integration tests (Section 7.2): verified insert round-trip, witness chain round-trip
5. Property-based tests (Section 7.3): proptest generators
6. Negative tests (Section 7.4): soundness verification
7. CI: create `.github/workflows/build-verified.yml` (Section 8.1)
8. Benchmark: `proof_generation` bench, verify < 1ms per op

**Exit criteria**: `cargo test -p ruvector-verified --features all-proofs` passes. CI green.

### Phase 3: Pipeline Verification (Week 5-6)

1. Implement `pipeline.rs`: `VerifiedStage`, `compose_stages` (Section 4.4)
2. Map DNA pipeline stages (examples/dna ADR-001..015) to verified chain
3. Integrate with `ruvector-cognitive-container` via `WitnessChain`:
   - `ProofAttestation` -> `to_witness_entry()` -> `WitnessChain::push()`
4. Implement proof serialization for `.rvf` containers via `rvf-types`
5. Tests: pipeline composition soundness, container round-trip

**Exit criteria**: DNA pipeline stages compose without type errors. RVF containers carry proof witnesses.

### Phase 4: Extended Coverage (Week 7-8)

1. Attention mask shape verification (`ruvector-attention`)
2. Solver convergence proofs (`ruvector-solver`)
3. CRDT commutativity proofs (`ruvector-delta-consensus`)
4. Create `ruvector-verified-wasm` with `wasm-bindgen` exports
5. WASM CI with binary size assertion (< 80KB)
6. Performance benchmarks: proof generation overhead < 1ms per op across all integrations
7. Set `publish = true` on `ruvector-verified` if API is stable

**Exit criteria**: WASM binary < 80KB. All benchmarks < 1ms. Feature-gated downstream crates compile.

---

## 11. Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| Crate at v0.1.0, API unstable | Medium | Exact pin `=0.1.0`, bridge crate insulates downstream |
| Dependent types learning curve | Medium | Pre-built invariant library in `invariants.rs` |
| Proof generation overhead | Low | Benchmarked per-phase, < 1ms target, arena caching |
| Workspace bloat | Low | Zero mandatory deps, feature-gated, 19KB crate |
| Upstream abandonment | Low | Same maintainer (ruvnet), Apache-2.0 fork rights |
| Type theory expressiveness limits | Medium | Start with dimension/equality proofs, expand iteratively |
| `HashMap` blocks `no_std` | Low | Phase 4 concern only; `wasm32-unknown-unknown` has std |
| ed25519-dalek version conflict | Low | Both use `"2"` -- verified compatible (Section 5) |
| License mismatch (MIT vs Apache-2.0) | Low | Dual-license `ruvector-verified` as `MIT OR Apache-2.0` |

---

## 12. Alternatives Considered

| Alternative | Rejected Because |
|------------|-----------------|
| **Creusot** (Rust -> Why3) | External toolchain, not embeddable as library |
| **Prusti** (Rust verifier) | Compiler plugin, heavyweight, not composable |
| **Kani** (bounded model checker) | Checks safety, not functional correctness |
| **Custom proof system** | Reinventing wheel, lean-agentic already implements CoC |
| **No verification** | Unacceptable for safety-critical genomic/financial paths |
| **Runtime assertions only** | No compile-time guarantees, performance overhead |
| **Depend on full lean-agentic workspace** | Only the core kernel is published; AI layer not needed |

---

## 13. Success Criteria

| Metric | Target | Measurement |
|--------|--------|-------------|
| Proof generation latency | < 1ms per vector operation | `cargo bench -p ruvector-verified` |
| Runtime overhead (release) | 0% (proof term is a Copy u32) | A/B benchmark: with vs without proof |
| Runtime overhead (debug) | < 5% | A/B benchmark in debug mode |
| API coverage | HNSW insert/query, pipeline compose, RVF witness | Feature flag test matrix |
| Test coverage | > 90% for `ruvector-verified` | `cargo tarpaulin` |
| WASM binary size | < 80KB (wasm-opt -Oz) | CI assertion in build-verified.yml |
| Build time increase | < 3s incremental | CI timing comparison |
| Arena cache hit rate | > 95% under dedup workload | `arena_throughput` bench |
| Crate size on crates.io | < 50KB | `cargo package --list` |

---

## 14. Consequences

### Positive

- RuVector gains **formal correctness guarantees** for safety-critical pipelines (genomics, finance) without runtime overhead.
- The proof attestation format chains into existing RVF witness infrastructure (ADR-042), enabling dual mathematical + hardware certification.
- Zero impact on existing builds -- all verification is feature-gated and opt-in.
- The bridge crate pattern insulates the workspace from upstream API churn.
- Hash-consed arena provides 85% memory reduction for proof term storage.

### Negative

- Adds one external dependency to `Cargo.lock` (lean-agentic, 19KB, zero transitive deps).
- Developers working on verification features need basic dependent type theory knowledge.
- Bridge crate adds maintenance surface (~6 source files, ~500 lines).
- Exact version pin (`=0.1.0`) means manual updates when upstream releases.

### Neutral

- No existing API changes. No default feature changes. No SemVer major bump.
- `ruvector-verified` starts unpublished (`publish = false`), reducing community expectations.
- WASM binary adds ~80KB to edge deployment bundles (only when verification feature is enabled).
- The `MIT OR Apache-2.0` dual license on the new crate is standard practice in the Rust ecosystem.

---

## 15. Ultra-Optimization: RuVector Performance Primitives for the Kernel

### 15.1 Motivation

lean-agentic's kernel achieves 0.3ns term equality via hash-consing, but the surrounding operations -- arena allocation, WHNF reduction, substitution, unification constraint solving -- use vanilla `HashMap` and `Vec` data structures. By applying RuVector's battle-tested SIMD, arena, caching, and algorithmic patterns, we can reduce proof generation from the ~1ms target to **sub-100us**, making verification viable even in hot HNSW insert loops.

### 15.2 Optimization Map

| Kernel Bottleneck | RuVector Pattern | Source Crate | Expected Speedup |
|-------------------|-----------------|--------------|-----------------|
| Arena `HashMap` lookup | SIMD hash + 4-wide probe | `ruvector-core/simd_intrinsics` | 3-5x |
| Term allocation | Bump allocator with O(1) reset | `ruvector-solver/arena` | 10-100x vs heap |
| WHNF substitution | Bounds-check elimination + fused kernel | `ruvector-solver/neumann` | 2-3x |
| Unification constraint queue | Thread-local pool with auto-return | `ruvector-mincut/pool` | 90%+ reuse |
| Conversion equality cache | LRU with prefetch prediction | `ruvector-mincut/optimization/cache` | 10x for repeated |
| Type tag comparison | INT8 quantized SIMD distance | `ruvector-core/quantization` | 4-8x |
| De Bruijn index shifting | 4-wide unrolled scalar loop | `ruvector-solver/cg` (ILP pattern) | 3-4x |
| Pipeline composition | Early exit via coherence gate | `ruvector-mincut-gated-transformer` | 30-50% skip |

### 15.3 Optimization 1: SolverArena for Term Allocation

**Problem**: lean-agentic's `Arena` uses `Vec<Term>` + `HashMap<u64, Vec<TermId>>`. Each `intern()` call may trigger HashMap resizing and Vec growth with per-element drop overhead.

**Solution**: Replace the backing store with `ruvector-solver`'s `SolverArena` pattern -- a bump allocator with O(1) reset.

```rust
// crates/ruvector-verified/src/fast_arena.rs

use std::cell::RefCell;

/// High-performance term arena using bump allocation.
/// Modeled after ruvector-solver's SolverArena (crates/ruvector-solver/src/arena.rs).
///
/// Key differences from lean-agentic's default Arena:
/// - Single contiguous allocation (cache-friendly)
/// - O(1) reset reclaims all memory
/// - 64-byte cache-line alignment for SIMD access
/// - No per-term drop overhead
pub struct FastTermArena {
    /// Backing buffer: contiguous, cache-aligned
    buf: RefCell<Vec<u8>>,
    /// Bump pointer
    offset: RefCell<usize>,
    /// Term count (for TermId generation)
    count: RefCell<u32>,
    /// Hash-consing cache: open-addressing with linear probe
    /// Layout: [hash: u64, term_id: u32, padding: u32] x capacity
    cache_buf: RefCell<Vec<u64>>,
    cache_mask: usize, // capacity - 1 (power of 2)
}

impl FastTermArena {
    /// Pre-allocate for expected term count.
    /// Typical: 4096 terms for a dimension proof, 65536 for pipeline verification.
    pub fn with_capacity(max_terms: usize) -> Self {
        let term_bytes = max_terms * std::mem::size_of::<Term>();
        let cache_cap = (max_terms * 2).next_power_of_two(); // 50% load factor

        Self {
            buf: RefCell::new(vec![0u8; term_bytes]),
            offset: RefCell::new(0),
            count: RefCell::new(0),
            cache_buf: RefCell::new(vec![0u64; cache_cap * 2]), // hash + id pairs
            cache_mask: cache_cap - 1,
        }
    }

    /// Intern a term with open-addressing hash lookup.
    /// Uses FxHash (multiply-shift) for speed over SipHash.
    #[inline]
    pub fn intern_fast(&self, kind: &TermKind) -> TermId {
        let hash = fx_hash(kind);
        let mask = self.cache_mask;
        let cache = self.cache_buf.borrow();

        // Linear probe with 4-wide unroll
        let mut slot = (hash as usize) & mask;
        for _ in 0..4 {
            let stored_hash = cache[slot * 2];
            if stored_hash == hash {
                // Potential hit -- verify structural equality
                let id = cache[slot * 2 + 1] as u32;
                return TermId(id);
            }
            if stored_hash == 0 {
                break; // Empty slot -- cache miss
            }
            slot = (slot + 1) & mask;
        }
        drop(cache);

        // Cache miss: allocate via bump pointer
        self.alloc_and_cache(kind, hash, slot)
    }

    /// O(1) reset -- reclaim all terms.
    /// Called between proof obligations.
    pub fn reset(&self) {
        *self.offset.borrow_mut() = 0;
        *self.count.borrow_mut() = 0;
        self.cache_buf.borrow_mut().fill(0);
    }
}

/// FxHash: multiply-shift hash (used by rustc internally).
/// 5x faster than SipHash for small keys.
#[inline]
fn fx_hash(kind: &TermKind) -> u64 {
    // Hash the discriminant + first 8 bytes of payload
    let discriminant = std::mem::discriminant(kind);
    let mut h = 0xcbf29ce484222325u64; // FNV offset
    h = h.wrapping_mul(0x100000001b3); // FNV prime
    h ^= unsafe { std::mem::transmute_copy::<_, u64>(&discriminant) };
    h
}
```

**Performance**: Eliminates heap allocation in hot path. Reset between proofs is O(1) vs O(n) drop.

### 15.4 Optimization 2: SIMD-Accelerated Hash-Consing

**Problem**: Hash-consing equality check requires computing hash of `TermKind`, then comparing against cached candidates. Default `HashMap` uses SipHash (DoS-resistant but slow).

**Solution**: Apply `ruvector-core`'s SIMD distance pattern for hash-table probing.

```rust
// 4-wide parallel hash probe using the ILP pattern from
// ruvector-solver/src/cg.rs (dot_product_f64 with 4 accumulators)

#[inline]
fn probe_4wide(cache: &[u64], hash: u64, mask: usize, start: usize) -> Option<u32> {
    let s0 = start & mask;
    let s1 = (start + 1) & mask;
    let s2 = (start + 2) & mask;
    let s3 = (start + 3) & mask;

    // 4 independent loads -- CPU can execute in parallel
    let h0 = cache[s0 * 2];
    let h1 = cache[s1 * 2];
    let h2 = cache[s2 * 2];
    let h3 = cache[s3 * 2];

    // 4 independent comparisons
    if h0 == hash { return Some(cache[s0 * 2 + 1] as u32); }
    if h1 == hash { return Some(cache[s1 * 2 + 1] as u32); }
    if h2 == hash { return Some(cache[s2 * 2 + 1] as u32); }
    if h3 == hash { return Some(cache[s3 * 2 + 1] as u32); }

    None // Continue probing
}
```

**On AVX2 targets** (feature-gated):

```rust
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn probe_avx2(cache: &[u64], hash: u64, mask: usize, start: usize) -> Option<u32> {
    use std::arch::x86_64::*;

    // Broadcast search hash to 4 lanes
    let needle = _mm256_set1_epi64x(hash as i64);

    // Load 4 consecutive hash entries
    let s = start & mask;
    let ptr = cache.as_ptr().add(s * 2) as *const __m256i;
    let entries = _mm256_loadu_si256(ptr); // [h0, id0, h1, id1]

    // Compare: yields 0xFFFF... on match
    let cmp = _mm256_cmpeq_epi64(entries, needle);
    let bitmask = _mm256_movemask_epi8(cmp);

    if bitmask != 0 {
        let lane = bitmask.trailing_zeros() / 8;
        let id_offset = s * 2 + (lane as usize) + 1;
        return Some(cache[id_offset] as u32);
    }
    None
}
```

**Expected speedup**: 3-5x over `HashMap::get()` for the hash-consing hot path.

### 15.5 Optimization 3: Fused Substitution + Shift Kernel

**Problem**: WHNF substitution `t[x := s]` requires traversing the term, shifting De Bruijn indices, and rebuilding. The default implementation makes 3 passes: traverse, shift, rebuild.

**Solution**: Apply `ruvector-solver/neumann.rs` fused residual pattern -- do it in one pass.

```rust
/// Fused substitute-and-shift in a single traversal.
/// Modeled after ruvector-solver's fused_residual_norm_sq which combines
/// SpMV + subtraction + norm into one memory pass.
///
/// Instead of:
///   pass 1: find variables to substitute
///   pass 2: shift remaining indices
///   pass 3: rebuild term
///
/// We do all three in a single recursive descent.
pub fn fused_substitute_shift(
    arena: &mut FastTermArena,
    term: TermId,
    var_idx: u32,       // De Bruijn index to substitute
    replacement: TermId, // The replacement term
    shift: i32,         // Current shift amount
) -> TermId {
    match arena.kind(term) {
        TermKind::Var(n) => {
            if *n == var_idx {
                replacement // Substitute
            } else if *n > var_idx {
                // Shift down: variable was bound above the substitution
                arena.intern_fast(&TermKind::Var((*n as i32 + shift) as u32))
            } else {
                term // Below substitution point -- unchanged (return same TermId)
            }
        }
        TermKind::App(f, a) => {
            let f2 = fused_substitute_shift(arena, *f, var_idx, replacement, shift);
            let a2 = fused_substitute_shift(arena, *a, var_idx, replacement, shift);
            // Short-circuit: if nothing changed, return original (hash-consing dedup)
            if f2 == *f && a2 == *a { return term; }
            arena.mk_app(f2, a2)
        }
        TermKind::Lam(binder, body) => {
            let ty2 = fused_substitute_shift(arena, binder.ty, var_idx, replacement, shift);
            // Under a binder: increment var_idx and shift
            let body2 = fused_substitute_shift(arena, *body, var_idx + 1, replacement, shift);
            if ty2 == binder.ty && body2 == *body { return term; }
            let new_binder = Binder { ty: ty2, ..binder.clone() };
            arena.intern_fast(&TermKind::Lam(new_binder, body2))
        }
        TermKind::Pi(binder, body) => {
            let ty2 = fused_substitute_shift(arena, binder.ty, var_idx, replacement, shift);
            let body2 = fused_substitute_shift(arena, *body, var_idx + 1, replacement, shift);
            if ty2 == binder.ty && body2 == *body { return term; }
            let new_binder = Binder { ty: ty2, ..binder.clone() };
            arena.intern_fast(&TermKind::Pi(new_binder, body2))
        }
        // Sort, Const, Lit, MVar: no variables to substitute
        _ => term,
    }
}
```

**Key trick**: The `if f2 == *f && a2 == *a { return term; }` short-circuit leverages hash-consing -- if sub-terms are unchanged, the parent term is also unchanged, saving allocation.

### 15.6 Optimization 4: Thread-Local Resource Pools

**Problem**: Each proof obligation creates fresh `Context`, constraint `VecDeque`, and `HashSet` for unification. These allocate and drop on every call.

**Solution**: Apply `ruvector-mincut/pool/mod.rs` pattern -- thread-local pools with auto-return.

```rust
// Thread-local pool for proof-checking resources.
// Pattern from ruvector-mincut's BfsPool (90%+ hit rate after warmup).

use std::cell::RefCell;
use std::collections::{VecDeque, HashSet};

thread_local! {
    static PROOF_POOL: RefCell<ProofResourcePool> = RefCell::new(ProofResourcePool::new());
}

struct ProofResourcePool {
    contexts: Vec<Context>,
    constraint_queues: Vec<VecDeque<Constraint>>,
    visited_sets: Vec<HashSet<TermId>>,
    acquires: usize,
    hits: usize,
}

impl ProofResourcePool {
    fn new() -> Self {
        Self {
            contexts: Vec::new(),
            constraint_queues: Vec::new(),
            visited_sets: Vec::new(),
            acquires: 0,
            hits: 0,
        }
    }
}

/// Acquire pooled resources for a proof obligation.
/// Auto-returns to pool when `ProofResources` is dropped.
pub fn acquire_proof_resources() -> ProofResources {
    PROOF_POOL.with(|pool| {
        let mut p = pool.borrow_mut();
        p.acquires += 1;

        let ctx = p.contexts.pop().unwrap_or_else(Context::new);
        let queue = p.constraint_queues.pop().unwrap_or_default();
        let visited = p.visited_sets.pop().unwrap_or_default();

        if !p.contexts.is_empty() || !p.constraint_queues.is_empty() {
            p.hits += 1;
        }

        ProofResources { ctx, queue, visited }
    })
}

pub struct ProofResources {
    pub ctx: Context,
    pub queue: VecDeque<Constraint>,
    pub visited: HashSet<TermId>,
}

impl Drop for ProofResources {
    fn drop(&mut self) {
        // Clear but retain capacity, then return to pool
        self.ctx.clear();
        self.queue.clear();
        self.visited.clear();

        PROOF_POOL.with(|pool| {
            let mut p = pool.borrow_mut();
            p.contexts.push(std::mem::take(&mut self.ctx));
            p.constraint_queues.push(std::mem::take(&mut self.queue));
            p.visited_sets.push(std::mem::take(&mut self.visited));
        });
    }
}
```

**Expected**: After warmup, 90%+ pool hit rate. Zero heap churn in steady state.

### 15.7 Optimization 5: Conversion Cache with Prefetch

**Problem**: The lean-agentic conversion engine memoizes WHNF results keyed on `(TermId, context_length)`, but uses a basic `HashMap`.

**Solution**: Apply `ruvector-mincut/optimization/cache.rs` LRU with access-pattern prefetch.

```rust
/// Conversion result cache with access-pattern prediction.
/// Modeled after ruvector-mincut's PathDistanceCache (10x for repeated queries).
pub struct ConversionCache {
    /// Open-addressing hash table: (key_hash, whnf_result)
    entries: Vec<CacheEntry>,
    mask: usize,
    /// Recent access pattern for prefetch prediction
    history: VecDeque<u64>,
    stats: CacheStats,
}

#[derive(Default, Clone)]
struct CacheEntry {
    key_hash: u64,
    term_id: TermId,
    whnf_result: TermId,
    access_count: u16,
}

struct CacheStats {
    hits: u64,
    misses: u64,
    prefetch_hits: u64,
}

impl ConversionCache {
    pub fn with_capacity(cap: usize) -> Self {
        let cap = cap.next_power_of_two();
        Self {
            entries: vec![CacheEntry::default(); cap],
            mask: cap - 1,
            history: VecDeque::with_capacity(64),
            stats: CacheStats { hits: 0, misses: 0, prefetch_hits: 0 },
        }
    }

    /// Look up cached WHNF result.
    #[inline]
    pub fn get(&mut self, term: TermId, ctx_len: usize) -> Option<TermId> {
        let hash = self.key_hash(term, ctx_len);
        let slot = (hash as usize) & self.mask;

        // Prefetch next likely access based on history
        if let Some(&predicted) = self.history.front() {
            let pred_slot = (predicted as usize) & self.mask;
            #[cfg(target_arch = "x86_64")]
            unsafe {
                std::arch::x86_64::_mm_prefetch(
                    (&self.entries[pred_slot]) as *const _ as *const i8,
                    std::arch::x86_64::_MM_HINT_T0,
                );
            }
        }

        let entry = &self.entries[slot];
        if entry.key_hash == hash {
            self.stats.hits += 1;
            self.history.push_back(hash);
            if self.history.len() > 64 { self.history.pop_front(); }
            Some(entry.whnf_result)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    #[inline]
    fn key_hash(&self, term: TermId, ctx_len: usize) -> u64 {
        let mut h = term.0 as u64;
        h = h.wrapping_mul(0x517cc1b727220a95);
        h ^= ctx_len as u64;
        h = h.wrapping_mul(0x6c62272e07bb0142);
        h
    }
}
```

### 15.8 Optimization 6: Coherence-Gated Proof Depth

**Problem**: Not all proof obligations are equally complex. Simple dimension checks need 2-3 reduction steps; pipeline composition may need hundreds. Spending the same effort on both wastes cycles.

**Solution**: Apply `ruvector-mincut-gated-transformer`'s 3-tier compute routing.

```rust
/// Adaptive proof depth routing, modeled after the GateController
/// in ruvector-mincut-gated-transformer/src/gate.rs.
///
/// Routes proof obligations to different compute tiers:
/// - Reflex (<10us): Dimension equality, literal comparison
/// - Standard (<100us): Single-step type inference
/// - Deep (<1ms): Full WHNF + unification
pub enum ProofTier {
    /// Tier 0: Direct comparison, no reduction needed
    /// e.g., prove_dim_eq(128, 128) -- just check n == m
    Reflex,
    /// Tier 1: Shallow inference, 1-10 reduction steps
    /// e.g., verified_insert with known types
    Standard { max_fuel: u32 },
    /// Tier 2: Full kernel, up to 10,000 steps
    /// e.g., pipeline composition with dependent types
    Deep,
}

/// Route a proof obligation to the cheapest tier that can handle it.
pub fn route_proof(kind: &TermKind, env: &Environment) -> ProofTier {
    match kind {
        // Literals and variables: direct comparison
        TermKind::Lit(_) | TermKind::Var(_) => ProofTier::Reflex,

        // Constants: check if reducible in environment
        TermKind::Const(sym, _) => {
            if env.is_reducible(*sym) {
                ProofTier::Standard { max_fuel: 100 }
            } else {
                ProofTier::Reflex
            }
        }

        // Applications: check depth
        TermKind::App(f, _) => {
            // If function is a known constructor, shallow
            ProofTier::Standard { max_fuel: 500 }
        }

        // Binders always need full checking
        TermKind::Lam(_, _) | TermKind::Pi(_, _) | TermKind::Let(_, _, _) => {
            ProofTier::Deep
        }

        _ => ProofTier::Standard { max_fuel: 100 },
    }
}

/// Execute proof with tiered fuel budget.
pub fn verify_tiered(
    arena: &mut FastTermArena,
    env: &Environment,
    term: TermId,
    expected_ty: TermId,
    tier: ProofTier,
) -> crate::Result<()> {
    match tier {
        ProofTier::Reflex => {
            // O(1): pointer equality via hash-consing
            if term == expected_ty { return Ok(()); }
            // Fallback to Standard
            verify_tiered(arena, env, term, expected_ty,
                         ProofTier::Standard { max_fuel: 100 })
        }
        ProofTier::Standard { max_fuel } => {
            // Limited fuel conversion
            let mut converter = Converter::with_fuel(max_fuel);
            if converter.is_def_eq(arena, env, &Context::new(), term, expected_ty) {
                Ok(())
            } else if max_fuel < 10_000 {
                // Escalate to Deep
                verify_tiered(arena, env, term, expected_ty, ProofTier::Deep)
            } else {
                Err(VerificationError::ConversionTimeout { max_reductions: max_fuel })
            }
        }
        ProofTier::Deep => {
            // Full kernel with default 10,000 fuel
            let checker = TypeChecker::default();
            checker.check(arena, env, &Context::new(), term, expected_ty)
                .map_err(VerificationError::from)
        }
    }
}
```

### 15.9 Optimization 7: Bounds-Check Elimination in Hot Loops

**Problem**: Substitution and WHNF inner loops perform bounds checks on every `arena.get(TermId)`.

**Solution**: Apply `ruvector-solver/neumann.rs` pattern -- validate once, then use `get_unchecked`.

```rust
/// Validate arena integrity once, then enter unchecked mode.
/// Pattern from ruvector-solver's spmv_unchecked (validates CSR once, then raw ptrs).
pub fn enter_unchecked_mode(arena: &FastTermArena) -> UncheckedArenaView<'_> {
    // Validate: all TermIds in range, no dangling references
    let count = *arena.count.borrow();
    let buf = arena.buf.borrow();
    assert!(buf.len() >= count as usize * std::mem::size_of::<Term>());

    UncheckedArenaView {
        ptr: buf.as_ptr() as *const Term,
        count,
    }
}

pub struct UncheckedArenaView<'a> {
    ptr: *const Term,
    count: u32,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> UncheckedArenaView<'a> {
    /// O(1) term access without bounds check.
    /// SAFETY: Caller must ensure id.0 < self.count (validated at construction).
    #[inline(always)]
    pub unsafe fn get(&self, id: TermId) -> &Term {
        debug_assert!(id.0 < self.count);
        &*self.ptr.add(id.0 as usize)
    }
}
```

### 15.10 Combined Performance Targets

| Operation | Before (lean-agentic default) | After (RuVector optimized) | Speedup |
|-----------|------------------------------|---------------------------|---------|
| Term allocation | ~50ns (HashMap + Vec push) | ~5ns (bump + FxHash probe) | 10x |
| Hash-consing lookup | ~30ns (SipHash + HashMap get) | ~8ns (FxHash + 4-wide probe) | 4x |
| WHNF substitution | ~200ns (3-pass) | ~70ns (fused single-pass) | 3x |
| Conversion equality | ~100ns (uncached) | ~10ns (LRU cache hit) | 10x |
| Resource acquire | ~500ns (alloc + init) | ~20ns (pool acquire) | 25x |
| Proof tier routing | N/A (always full kernel) | ~5ns (match on TermKind) | skip 30-50% |
| **End-to-end: dimension proof** | ~1,000ns | **< 50ns** | **20x** |
| **End-to-end: pipeline compose** | ~100,000ns | **< 10,000ns** | **10x** |

### 15.11 Implementation Priority

These optimizations are **Phase 5** work (Week 9-10), after the base integration is stable:

| Priority | Optimization | Effort | Impact |
|----------|-------------|--------|--------|
| P0 | SolverArena bump allocator (15.3) | 1 day | 10x allocation |
| P0 | Thread-local resource pools (15.6) | 1 day | 25x resource acquire |
| P1 | Fused substitution kernel (15.5) | 2 days | 3x WHNF |
| P1 | Coherence-gated proof depth (15.8) | 1 day | Skip 30-50% work |
| P2 | SIMD hash probe (15.4) | 2 days | 4x hash-consing |
| P2 | Conversion cache with prefetch (15.7) | 1 day | 10x repeated |
| P3 | Bounds-check elimination (15.9) | 1 day | 2x inner loops |

**Total**: ~9 engineering days for 10-20x end-to-end speedup.

### 15.12 New Dependencies for Optimization Phase

```toml
# Added to crates/ruvector-verified/Cargo.toml in Phase 5

[features]
fast-arena = []          # SolverArena-style bump allocator
simd-hash = []           # AVX2/NEON hash-consing probe
gated-proofs = []        # Coherence-gated proof depth routing
ultra = ["fast-arena", "simd-hash", "gated-proofs"]  # All optimizations

# No new external dependencies -- all patterns are inlined from RuVector crates.
# The optimizations use std::arch intrinsics directly (same as ruvector-core).
```

### 15.13 Benchmark Additions for Optimization Phase

```toml
# Additional bench entries for Phase 5

[[bench]]
name = "fast_arena_vs_default"
harness = false

[[bench]]
name = "simd_hash_probe"
harness = false

[[bench]]
name = "fused_substitute"
harness = false

[[bench]]
name = "tiered_routing"
harness = false
```

---

## 16. References

- [lean-agentic on crates.io](https://crates.io/crates/lean-agentic)
- [lean-agentic documentation](https://docs.rs/lean-agentic)
- [lean-agentic repository](https://github.com/agenticsorg/lean-agentic)
- [Lean 4 type theory](https://leanprover.github.io/lean4/doc/)
- ADR-001: RuVector Core Architecture
- ADR-014: Coherence Engine
- ADR-029: RVF Canonical Format
- ADR-030: RVF Cognitive Container
- ADR-039: RVF Solver WASM AGI Integration
- ADR-042: Security RVF AIDefence TEE
- ADR-044: ruvector-postgres v0.3 Extension Upgrade

### Optimization Pattern Sources (Section 15)

| Pattern | Source File | Lines |
|---------|------------|-------|
| Bump allocator | `crates/ruvector-solver/src/arena.rs` | 1-176 |
| 4-wide ILP unroll | `crates/ruvector-solver/src/cg.rs` | 76-102 |
| Fused kernel | `crates/ruvector-solver/src/neumann.rs` | 121-150 |
| Bounds-check elimination | `crates/ruvector-solver/src/types.rs` | 86-111 |
| Thread-local pool | `crates/ruvector-mincut/src/pool/mod.rs` | BfsPool |
| LRU cache + prefetch | `crates/ruvector-mincut/src/optimization/cache.rs` | PathDistanceCache |
| SIMD distance | `crates/ruvector-mincut/src/optimization/simd_distance.rs` | DistanceArray |
| Coherence gate routing | `crates/ruvector-mincut-gated-transformer/src/gate.rs` | GateController |
| WeightArena (bump) | `crates/ruvector-mincut-gated-transformer/src/arena.rs` | alloc_f32 |
| AVX2 SpMV | `crates/ruvector-solver/src/simd.rs` | spmv_avx2 |
| AVX2 horizontal sum | `crates/ruvector-core/src/simd_intrinsics.rs` | horizontal_sum |
| FxHash (multiply-shift) | `crates/ruvector-core/src/simd_intrinsics.rs` | distance dispatch |
| Cache-aligned struct | `crates/ruvector-core/src/arena.rs` | CACHE_LINE_SIZE=64 |
| INT8 quantized SIMD | `crates/ruvector-core/src/simd_intrinsics.rs` | 979-1212 |
| Early exit (coherence) | `crates/ruvector-mincut-gated-transformer/src/early_exit.rs` | CoherenceEarlyExit |
