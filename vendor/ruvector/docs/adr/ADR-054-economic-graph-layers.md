# ADR-054: Economic Graph Transformer Layers

## Status

Accepted

## Date

2026-02-25

## Context

Standard graph neural networks assume cooperative nodes: every vertex computes its feature update faithfully and passes honest messages. This assumption fails in federated learning, multi-stakeholder knowledge graphs, decentralized finance, supply chain networks, and autonomous vehicle coordination -- settings where nodes belong to independent agents with competing objectives. Without economic reasoning, GNNs are vulnerable to free-riding, Sybil attacks, and strategic information withholding.

RuVector already contains the economic and game-theoretic building blocks:

- `ruvector-economy-wasm/src/stake.rs`: staking and slashing mechanisms
- `ruvector-economy-wasm/src/reputation.rs`: reputation scoring and decay
- `ruvector-economy-wasm/src/ledger.rs`: CRDT-based distributed ledger
- `ruvector-economy-wasm/src/curve.rs`: bonding curves for token economics
- `ruvector-dag/src/qudag/tokens/staking.rs`: stake-weighted DAG consensus
- `ruvector-dag/src/qudag/tokens/rewards.rs`: reward distribution
- `ruvector-dag/src/qudag/tokens/governance.rs`: governance token mechanics
- `ruvector-dag/src/qudag/consensus.rs`: Byzantine fault-tolerant consensus
- `ruvector-verified/src/gated.rs`: proof-gated verification for budget proofs

However, there is no module that embeds game-theoretic reasoning into graph attention itself -- attention as Nash equilibrium, VCG mechanisms for truthful message passing, Shapley attribution for fair contribution measurement, or market-based routing for attention bandwidth allocation. The research at `docs/research/gnn-v2/29-economic-graph-transformers.md` describes the theory but defines no implementation path through existing crate APIs.

## Decision

We will implement an `economic` module in `ruvector-graph-transformer` behind the `economic` feature flag (not in the default feature set due to the additional complexity and dependency on `ruvector-economy-wasm`). The module provides four layer types: `GameTheoreticAttention`, `VcgMessagePassing`, `IncentiveAlignedMPNN`, and `ShapleyAttention`.

### GameTheoreticAttention

Nash equilibrium computation via iterated best response:

```rust
/// Game-theoretic attention where each node maximizes expected payoff.
///
/// Replaces softmax(QK^T / sqrt(d)) with equilibrium attention:
/// each node selects an attention distribution that maximizes
/// U_v(sigma_v, sigma_{-v}) = relevance - cost + externality.
///
/// Convergence: O(log(1/epsilon)) rounds for potential games,
/// O(1/epsilon^2) for general games. In practice 3-5 rounds suffice.
pub struct GameTheoreticAttention {
    /// Per-node utility parameters [relevance_w, cost_w, externality_w].
    utility_weights: Vec<[f32; 3]>,
    /// Strategy temperature (controls exploration vs exploitation).
    temperature: f32,
    /// Best-response iterations to approximate Nash equilibrium.
    best_response_iters: usize,
    /// Convergence threshold (L-infinity distance between rounds).
    convergence_threshold: f32,
    /// Proof requirement: equilibrium convergence certificate.
    equilibrium_proof: ProofRequirement,
}

impl GameTheoreticAttention {
    /// Compute equilibrium attention weights.
    ///
    /// Initializes with uniform attention, then iterates best response:
    /// each node selects softmax(payoff / temperature) over neighbors.
    ///
    /// Proof gate: verifies convergence (max strategy change < threshold)
    /// via ProofTier::Standard. If not converged after max iterations,
    /// falls back to standard softmax attention and logs a warning.
    pub fn compute_equilibrium(
        &self,
        queries: &[f32],
        keys: &[f32],
        values: &[f32],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<EquilibriumOutput>>;

    /// Compute social welfare: sum of all nodes' utilities at equilibrium.
    pub fn social_welfare(&self, equilibrium: &EquilibriumOutput) -> f64;

    /// Compute Price of Anarchy: ratio of optimal welfare to equilibrium welfare.
    pub fn price_of_anarchy(
        &self,
        equilibrium: &EquilibriumOutput,
        optimal: &AttentionOutput,
    ) -> f64;
}
```

### VcgMessagePassing

Vickrey-Clarke-Groves mechanism for truthful message passing:

```rust
/// VCG mechanism for incentive-compatible graph message passing.
///
/// Allocation rule: attention mechanism selects message weights.
/// Payment rule: each node pays a tax equal to the externality
/// its message imposes on others.
///
/// payment(u -> v) = sum_{w != u} U_w(alloc_with_u)
///                 - sum_{w != u} U_w(alloc_without_u)
///
/// Truthful reporting is a dominant strategy under VCG.
pub struct VcgMessagePassing {
    /// Base attention mechanism for allocation.
    base_attention: Box<dyn SublinearGraphAttention>,
    /// Number of samples for approximate VCG (reduces O(n^2) to O(n log n)).
    vcg_samples: usize,
    /// Proof requirement: incentive compatibility certificate.
    incentive_proof: ProofRequirement,
}

impl VcgMessagePassing {
    /// Forward pass with VCG payments.
    ///
    /// 1. Compute attention allocation with all nodes.
    /// 2. For each sampled node u, recompute allocation without u.
    /// 3. Payment(u) = marginal externality.
    ///
    /// Proof gate: verifies individual rationality (all payments >= 0
    /// for non-strategic nodes) and approximate budget balance
    /// (sum of payments within epsilon of zero).
    /// Routes to ProofTier::Standard (sum computation).
    pub fn forward(
        &self,
        features: &[f32],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<VcgOutput>>;
}

pub struct VcgOutput {
    /// Message passing output (node features).
    pub features: Vec<f32>,
    /// Per-node VCG payments.
    pub payments: Vec<f64>,
    /// Budget surplus (should be near zero).
    pub budget_surplus: f64,
}
```

### IncentiveAlignedMPNN

Stake-weighted messaging with slashing from `ruvector-economy-wasm`:

```rust
/// Incentive-aligned message passing with stake and reputation.
///
/// Bridges to:
/// - ruvector_economy_wasm::stake::StakeRegistry for stake management
/// - ruvector_economy_wasm::reputation::ReputationScore for quality tracking
/// - ruvector_economy_wasm::ledger::CrdtLedger for distributed state
///
/// Nodes must stake tokens to send messages. Messages from high-reputation
/// nodes receive amplified attention. Low-quality messages trigger slashing.
pub struct IncentiveAlignedMPNN {
    /// Stake registry from ruvector-economy-wasm.
    stake_registry: StakeRegistry,
    /// Reputation ledger (CRDT-based).
    reputation_ledger: CrdtLedger,
    /// Message quality model (learned scorer).
    quality_model: MessageQualityModel,
    /// Slashing fraction for low-quality messages.
    slash_fraction: f64,
    /// Minimum stake to participate in message passing.
    min_stake: u64,
    /// Proof requirement: stake sufficiency.
    stake_proof: ProofRequirement,
}

impl IncentiveAlignedMPNN {
    /// Forward pass with economic incentives.
    ///
    /// 1. Verify each sender has sufficient stake (ProofTier::Reflex).
    /// 2. Weight messages by reputation * stake.
    /// 3. Score message quality after aggregation.
    /// 4. Update reputation: high-quality messages earn reputation,
    ///    low-quality messages lose reputation and stake.
    ///
    /// Returns both the updated features and an economic ledger update
    /// recording all stake movements and reputation changes.
    pub fn forward(
        &mut self,
        features: &[f32],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<EconomicOutput>>;

    /// Slash a node for provably bad behavior.
    /// Requires proof of misbehavior via ruvector-verified.
    pub fn slash(
        &mut self,
        node: NodeId,
        proof: &ProofAttestation,
    ) -> Result<SlashResult>;
}

pub struct EconomicOutput {
    pub features: Vec<f32>,
    pub ledger_update: LedgerUpdate,
    pub slashed_nodes: Vec<NodeId>,
    pub total_stake_moved: u64,
}
```

### ShapleyAttention

Fair attribution via Monte Carlo Shapley values:

```rust
/// Shapley attention for fair contribution attribution.
///
/// Computes the Shapley value of each neighbor's message to each
/// target node. The Shapley value is the average marginal contribution
/// over all possible orderings of neighbors.
///
/// Exact computation is O(2^|N(v)|) per node, so we use Monte Carlo
/// approximation with configurable sample count.
pub struct ShapleyAttention {
    /// Number of Monte Carlo permutations per node.
    num_permutations: usize,
    /// Base attention mechanism for evaluating coalitions.
    base_attention: Box<dyn SublinearGraphAttention>,
    /// Proof requirement: Shapley efficiency (values sum to v(N)).
    efficiency_proof: ProofRequirement,
}

impl ShapleyAttention {
    /// Compute Shapley attention values.
    ///
    /// For each target node v, samples random orderings of N(v),
    /// computes marginal contribution of each neighbor at its
    /// position in the ordering, and averages.
    ///
    /// Proof gate: verifies Shapley efficiency axiom --
    /// sum of Shapley values equals total coalition value v(N(v)).
    /// Routes to ProofTier::Standard (sum comparison).
    pub fn forward(
        &self,
        features: &[f32],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<ShapleyOutput>>;
}

pub struct ShapleyOutput {
    /// Updated node features.
    pub features: Vec<f32>,
    /// Per-edge Shapley values (attribution weights).
    pub shapley_values: Vec<f64>,
}
```

### Proof-Gated Economic Invariants

| Operation | Proof Requirement | Tier | Latency |
|-----------|------------------|------|---------|
| Stake sufficiency check | `stake >= min_stake` | Reflex | < 10 ns |
| Equilibrium convergence | Max strategy delta < threshold | Standard(200) | < 2 us |
| VCG individual rationality | All payments >= 0 | Standard(100) | < 1 us |
| VCG budget balance | `|sum(payments)| < epsilon` | Standard(100) | < 1 us |
| Shapley efficiency | `sum(phi_i) == v(N)` | Standard(100) | < 1 us |
| Slashing proof | Proof of misbehavior valid | Deep | < 100 us |

### Feature Flag

```toml
# In crates/ruvector-graph-transformer/Cargo.toml
[features]
economic = [
    "ruvector-economy-wasm",
    "ruvector-dag/tokens",
]
```

The `economic` feature is intentionally NOT part of the `default` or `full` feature sets. Users must explicitly opt in because it introduces economic state (staking, reputation) that requires careful lifecycle management.

## Consequences

### Positive

- Incentive compatibility via VCG ensures nodes cannot profit from sending dishonest messages
- Stake-weighted messaging makes Sybil attacks economically prohibitive (each fake identity requires its own stake)
- Shapley attribution provides theoretically fair contribution measurement, enabling equitable reward distribution in federated graph learning
- Game-theoretic attention reveals the economic structure of the graph (which nodes are strategic, which are cooperative)
- Proof-gated economic invariants create an auditable trail of all stake movements and slashing events

### Negative

- Nash equilibrium computation adds O(best_response_iters * n * avg_degree) overhead per attention layer
- VCG payments require recomputing attention without each sampled node, adding O(vcg_samples * n) cost
- Shapley Monte Carlo approximation has O(num_permutations * avg_degree) variance per node
- Economic state (stake registry, reputation ledger) adds persistent state that must be serialized and recovered across sessions
- The `economic` feature introduces a dependency on `ruvector-economy-wasm`, which is a WASM-target crate; native builds require the `ruvector-economy-wasm` crate to expose a native API

### Risks

- Game-theoretic attention may not converge for adversarial graph topologies (star graphs with a single high-degree node). Mitigation: fallback to standard softmax after max iterations with a logged convergence failure
- VCG approximate budget balance (via sampling) may have high variance for small sample counts. Mitigation: adaptive sampling that increases count until budget surplus stabilizes below epsilon
- Slashing without proper adjudication creates centralization risk. Mitigation: slashing requires a `ProofAttestation` (Deep tier) proving the misbehavior, preventing unilateral slashing
- Token economics (bonding curves from `ruvector-economy-wasm::curve`) may create perverse incentives if parameters are misconfigured. Mitigation: parameter bounds enforced via proof gate (min/max stake, max slash fraction)

## Implementation

1. Create `crates/ruvector-graph-transformer/src/economic/mod.rs` re-exporting all types
2. Implement `GameTheoreticAttention` in `economic/game_theory.rs` with iterated best response
3. Implement `VcgMessagePassing` in `economic/vcg.rs` with approximate VCG via sampling
4. Implement `IncentiveAlignedMPNN` in `economic/incentive.rs`, bridging to `ruvector-economy-wasm::{stake, reputation, ledger}`
5. Implement `ShapleyAttention` in `economic/shapley.rs` with Monte Carlo Shapley approximation
6. Add benchmark: `benches/economic_bench.rs` measuring equilibrium convergence on a 10K-node graph with 5 best-response rounds
7. Integration test: `IncentiveAlignedMPNN` with 100 nodes, inject 10 adversarial nodes, verify slashing and reputation update
8. Verify build: `cargo test --features economic -p ruvector-graph-transformer`

## References

- ADR-046: Graph Transformer Unified Architecture (module structure, `AttentionRegistry`)
- ADR-047: Proof-Gated Mutation Protocol (`ProofGate<T>`, economic invariant proofs)
- ADR-048: Sublinear Graph Attention (`SublinearGraphAttention` trait used by VCG and Shapley)
- Research: `docs/research/gnn-v2/29-economic-graph-transformers.md`
- `crates/ruvector-economy-wasm/src/stake.rs`: `StakeRegistry`, staking/slashing
- `crates/ruvector-economy-wasm/src/reputation.rs`: `ReputationScore`, decay
- `crates/ruvector-economy-wasm/src/ledger.rs`: `CrdtLedger` for distributed state
- `crates/ruvector-economy-wasm/src/curve.rs`: bonding curves
- `crates/ruvector-dag/src/qudag/tokens/staking.rs`: stake-weighted consensus
- `crates/ruvector-dag/src/qudag/tokens/rewards.rs`: reward distribution
- `crates/ruvector-dag/src/qudag/consensus.rs`: BFT consensus
- `crates/ruvector-verified/src/gated.rs`: `ProofTier`, `route_proof`
- `crates/ruvector-verified/src/proof_store.rs`: `ProofAttestation`
- Vickrey, "Counterspeculation, Auctions, and Competitive Sealed Tenders" (J Finance, 1961)
- Clarke, "Multipart Pricing of Public Goods" (Public Choice, 1971)
- Shapley, "A Value for n-Person Games" (Contributions to Theory of Games, 1953)
- Nash, "Equilibrium Points in N-Person Games" (PNAS, 1950)
