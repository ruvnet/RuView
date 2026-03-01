# ADR-047: Proof-Gated Mutation Protocol

## Status

Accepted

## Date

2026-02-25

## Context

RuVector's graph transformer operates on mutable graph state -- nodes are added, edges are rewired, attention weights are updated, and topology evolves during self-organizing operations. In safety-critical deployments (genomic pipelines, financial computation, cognitive containers), every mutation must be auditable and formally justified.

The existing `ruvector-verified` crate provides `ProofEnvironment`, `VerifiedOp<T>`, `ProofAttestation` (82-byte witnesses), and three-tier proof routing (`Reflex`, `Standard`, `Deep`) in `crates/ruvector-verified/src/gated.rs`. However, there is no protocol for composing these primitives into a mutation control substrate -- no defined lifecycle for how a graph mutation acquires its proof, how local proofs compose into regional proofs, how proof scopes align with min-cut partition boundaries, or how the attestation chain grows without unbounded memory.

We need a protocol that makes "no proof, no mutation" the default, while keeping hot-path overhead below 2%.

## Decision

We will implement the Proof-Gated Mutation Protocol as the `proof_gated` module within `ruvector-graph-transformer`. The protocol defines a type-level gate (`ProofGate<T>`), a scoping mechanism (`ProofScope`), a composition algebra for attestation chains, and epoch boundaries for protocol upgrades.

### The ProofGate<T> Type

`ProofGate<T>` is a wrapper that makes the inner value inaccessible without a valid proof:

```rust
/// A value gated behind a machine-checked proof.
///
/// The inner `T` cannot be accessed without presenting a proof that
/// satisfies the gate's `ProofRequirement`. This is enforced at the
/// type level -- there is no `unsafe` escape hatch.
pub struct ProofGate<T> {
    /// The gated value. Private -- only accessible via `unlock()`.
    inner: T,
    /// The proof requirement that must be satisfied.
    requirement: ProofRequirement,
    /// Attestation produced when the gate was satisfied.
    attestation: Option<ProofAttestation>,
}

impl<T> ProofGate<T> {
    /// Create a new proof gate with the given requirement.
    pub fn new(value: T, requirement: ProofRequirement) -> Self;

    /// Attempt to unlock the gate by providing a proof.
    /// Returns `&T` on success, `Err(ProofGateRejected)` on failure.
    pub fn unlock(&self, env: &mut ProofEnvironment) -> Result<&T>;

    /// Consume the gate, returning the value and its attestation chain.
    pub fn into_inner(self, env: &mut ProofEnvironment) -> Result<(T, ProofAttestation)>;

    /// Check if this gate has been satisfied (attestation present).
    pub fn is_satisfied(&self) -> bool;
}
```

`ProofRequirement` is an enum that maps to `ruvector-verified::gated::ProofKind`:

```rust
pub enum ProofRequirement {
    /// Dimension equality: vector has expected dimension.
    DimensionMatch { expected: u32 },
    /// Type constructor: node/edge type matches schema.
    TypeMatch { schema_id: u64 },
    /// Invariant preservation: graph property holds after mutation.
    InvariantPreserved { invariant_id: u32 },
    /// Coherence bound: attention coherence above threshold.
    CoherenceBound { min_coherence: f64 },
    /// Composition: all sub-requirements must be satisfied.
    Composite(Vec<ProofRequirement>),
}
```

### Three-Tier Routing

Every mutation routes through the existing `ruvector-verified::gated::route_proof` function, which selects the cheapest sufficient proof tier:

| Tier | Target Latency | Use Case | Implementation |
|------|---------------|----------|----------------|
| **Reflex** | < 10 ns | Dimension checks, reflexivity, literal equality | Direct comparison, no reduction engine. Maps to `ProofTier::Reflex` |
| **Standard** | < 1 us | Type application (depth <= 5), short pipelines (<=3 stages) | Bounded fuel via `ProofTier::Standard { max_fuel }`, auto-escalates on failure |
| **Deep** | < 100 us | Long pipelines, custom proofs, invariant verification | Full 10,000-step kernel via `ProofTier::Deep` |

Routing is automatic: the `ProofRequirement` is classified into a `ProofKind`, passed to `route_proof()`, and the returned `TierDecision` determines which verification path to take. If a tier fails, it escalates to the next tier (Reflex -> Standard -> Deep) via `verify_tiered()` as implemented in `crates/ruvector-verified/src/gated.rs`.

### Attestation Chain

Each successful proof produces a `ProofAttestation` (82 bytes, defined in `crates/ruvector-verified/src/proof_store.rs`). Attestations are stored in a `MutationLedger`:

```rust
pub struct MutationLedger {
    /// Append-only log of attestations for this scope.
    attestations: Vec<ProofAttestation>,
    /// Running content hash (FNV-1a) over all attestation bytes.
    chain_hash: u64,
    /// Epoch counter for proof algebra versioning.
    epoch: u64,
    /// Maximum attestations before compaction.
    compaction_threshold: usize,
}

impl MutationLedger {
    /// Append an attestation. Returns the chain position.
    pub fn append(&mut self, att: ProofAttestation) -> u64;

    /// Compact old attestations into a single summary attestation.
    /// Preserves the chain hash but reduces memory.
    pub fn compact(&mut self) -> ProofAttestation;

    /// Verify the chain hash is consistent.
    pub fn verify_integrity(&self) -> bool;
}
```

### Proof Composition

Local proofs compose into regional proofs via `compose_chain`:

```rust
/// Compose a sequence of local proof attestations into a regional proof.
///
/// The regional proof's `proof_term_hash` is the hash of all constituent
/// attestation hashes. The `reduction_steps` field is the sum of all
/// constituent steps. This is sound because proofs are append-only and
/// each attestation covers a disjoint mutation.
pub fn compose_chain(attestations: &[ProofAttestation]) -> ProofAttestation;
```

Composition respects partition boundaries: a `ProofScope` is defined by a min-cut partition (from `ruvector-mincut`), and proofs within a scope compose locally. Cross-scope composition requires a `GlobalCoherenceProof` that verifies the boundary edges between partitions maintain coherence above the threshold.

### Proof Scope and Min-Cut Alignment

```rust
pub struct ProofScope {
    /// Partition ID from ruvector-mincut.
    partition_id: u32,
    /// Boundary nodes shared with adjacent partitions.
    boundary_nodes: Vec<u64>,
    /// The ledger for this scope.
    ledger: MutationLedger,
    /// Coherence measurement for this scope.
    coherence: Option<f64>,
}
```

When the graph self-organizes (topology changes via `ruvector-mincut`), proof scopes are re-derived from the new partition. Attestations from the old scope are sealed with a `ScopeTransitionAttestation` that records the old and new partition IDs, the min-cut value at transition, and the composition proof of the old scope.

### Monotonic Semantics

Attestations are append-only. There is no `delete` operation on the `MutationLedger`. Rollback is achieved by appending a **supersession proof** -- a new attestation that proves the rolled-back state is valid, referencing the original attestation by position:

```rust
pub struct SupersessionProof {
    /// Position of the attestation being superseded.
    superseded_position: u64,
    /// The new attestation that replaces it.
    replacement: ProofAttestation,
    /// Proof that the replacement is sound (e.g., inverse mutation).
    soundness_proof_id: u32,
}
```

### Epoch Boundaries

The proof algebra may be upgraded (new invariants, changed reduction limits, new built-in symbols). Epoch boundaries are explicit:

```rust
pub struct EpochBoundary {
    /// Previous epoch number.
    from_epoch: u64,
    /// New epoch number.
    to_epoch: u64,
    /// Summary attestation sealing all proofs in the previous epoch.
    seal: ProofAttestation,
    /// New proof environment configuration.
    new_config: ProofEnvironmentConfig,
}
```

At an epoch boundary, the `MutationLedger` is compacted, a seal attestation is produced, and the `ProofEnvironment` is reconfigured with new symbols and fuel budgets. Old proofs remain valid (sealed) but new proofs use the updated algebra.

### Performance Budget

The target is less than 2% overhead on the hot path. This is achieved by:

1. **Reflex tier dominance**: In steady-state graph transformer inference, 90%+ of mutations are dimension checks and reflexivity proofs, which route to Reflex (< 10 ns)
2. **FastTermArena**: Bump allocation with O(1) dedup from `crates/ruvector-verified/src/fast_arena.rs` avoids heap allocation
3. **Proof caching**: `ProofEnvironment::cache_lookup` avoids re-proving identical obligations
4. **Lazy attestation**: `ProofAttestation` is constructed only when the caller requests `proof_chain()`, not on every mutation
5. **Batch gating**: Multiple mutations within a single forward pass share one `ProofScope`, amortizing the scope setup cost

Benchmarks must demonstrate: Reflex < 10 ns, Standard < 1 us, Deep < 100 us, composition of 1000 attestations < 50 us, ledger compaction of 10,000 entries < 1 ms.

## Consequences

### Positive

- Every graph mutation carries a machine-checked proof -- auditable, reproducible, and tamper-evident
- Three-tier routing keeps the common case (Reflex) at near-zero cost
- Attestation chains provide a complete audit trail for compliance (GDPR provenance, SOC2 audit logs)
- Epoch boundaries allow upgrading the proof system without invalidating historical proofs
- Monotonic semantics prevent accidental attestation loss

### Negative

- `ProofGate<T>` adds one level of indirection to every graph access
- Developers must reason about `ProofRequirement` when defining new mutation types
- Supersession proofs add complexity compared to simple deletion
- The `MutationLedger` grows linearly with mutations until compaction (mitigated by compaction threshold)

### Risks

- If Reflex tier coverage drops below 90%, the 2% overhead budget may be exceeded. Mitigated by monitoring `ProofStats::cache_hits` ratio in production
- Attestation chain integrity depends on FNV-1a hash -- not cryptographically secure. For production audit trails, upgrade to BLAKE3 (available via `ruvector-graph`'s `blake3` dependency)
- Epoch boundary migration is a manual operation -- if forgotten, the ledger grows unbounded. Mitigated by a configurable auto-epoch threshold in `GraphTransformerConfig`

## Implementation

1. Implement `ProofGate<T>` and `ProofRequirement` in `crates/ruvector-graph-transformer/src/proof_gated/gate.rs`
2. Implement `MutationLedger` with append, compact, and verify in `crates/ruvector-graph-transformer/src/proof_gated/mod.rs`
3. Implement `compose_chain` and `ProofScope` in `crates/ruvector-graph-transformer/src/proof_gated/attestation.rs`
4. Implement `EpochBoundary` in `crates/ruvector-graph-transformer/src/proof_gated/epoch.rs`
5. Add benchmark suite: `benches/proof_gate_bench.rs` covering all three tiers, composition, and compaction
6. Integration test: full forward pass with 10,000 mutations, verifying attestation chain integrity

## References

- ADR-045: Lean-Agentic Integration (establishes `ProofEnvironment`, `ProofAttestation`, `FastTermArena`)
- ADR-046: Graph Transformer Unified Architecture (module structure)
- `crates/ruvector-verified/src/gated.rs`: `ProofTier`, `ProofKind`, `route_proof`, `verify_tiered`
- `crates/ruvector-verified/src/proof_store.rs`: `ProofAttestation`, `ATTESTATION_SIZE` (82 bytes)
- `crates/ruvector-verified/src/fast_arena.rs`: `FastTermArena`, bump allocation with FxHash dedup
- `crates/ruvector-verified/src/error.rs`: `VerificationError` variants
- `crates/ruvector-mincut/Cargo.toml`: `canonical` feature for pseudo-deterministic min-cut
- `crates/ruvector-mincut-gated-transformer/src/energy_gate.rs`: `EnergyGate` decision model
