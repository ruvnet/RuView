# ADR-001: Anytime-Valid Coherence Gate

**Status**: Proposed
**Date**: 2026-01-17
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board
**SDK**: Claude-Flow

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-17 | ruv.io | Initial draft with three-filter architecture |
| 0.2 | 2026-01-17 | ruv.io | Added security hardening, performance optimization |
| 0.3 | 2026-01-17 | ruv.io | Added 256-tile WASM fabric mapping |
| 0.4 | 2026-01-17 | ruv.io | Added API contract, migration, observability |
| 0.5 | 2026-01-17 | ruv.io | Added hybrid agent/human workflow |
| 0.6 | 2026-01-17 | ruv.io | Added testing strategy, config format, error recovery |

## Plain Language Summary

**What is it?**

An Anytime-Valid Coherence Gate is a small control loop that decides, at any moment:

> "Is it safe to act right now, or should we pause or escalate?"

It does not try to be smart. It tries to be **safe**, **calm**, and **correct** about permission.

**Why "anytime-valid"?**

Because you can stop the computation at any time and still trust the decision.

Like a smoke detector:
- It can keep listening forever
- The moment it has enough evidence, it triggers
- If you stop listening early, whatever it already concluded is still valid

You are not waiting for a model to finish thinking. You are continuously monitoring stability.

**Why "coherence"?**

Coherence means: does the system's current state agree with itself?

In RuVector, coherence is measured from structure:
- RuVector holds relationships as vectors plus a graph
- Min-cut and boundary signals tell you when the graph is becoming fragile or splitting into conflicting regions
- If the system is splitting, you do not let it take big actions

**What it outputs:**

| Decision | Meaning |
|----------|---------|
| **Permit** | Stable enough, proceed |
| **Defer** | Uncertain, escalate to a stronger model or human |
| **Deny** | Unstable or policy-violating, block the action |

Every decision returns a short "receipt" explaining why.

**A concrete example:**

An agent wants to push a config change to a network device.
- If the dependency graph is stable and similar changes worked before → **Permit**
- If signals are weird (new dependencies, new actors, drift) → **Defer** and ask for confirmation
- If the change crosses a fragile boundary (touches a partition already unstable) → **Deny**

**Why it matters:**

It turns autonomy into something enterprises can trust because:
- Actions are bounded
- Uncertainty is handled explicitly
- You get an audit trail

*"Attention becomes a permission system, not a popularity contest"* — applied to whole-system actions instead of token attention.

---

## Context

The RuVector ecosystem requires a principled mechanism for controlling autonomous agent actions with:
- **Formal safety guarantees** under distribution shift
- **Computational efficiency** suitable for real-time enforcement
- **Auditable decision trails** with cryptographic receipts

Current approaches (threshold classifiers, rule-based systems, periodic audits) lack one or more of these properties. This ADR proposes the **Anytime-Valid Coherence Gate (AVCG)** - a 3-way algorithmic combination that converts coherence measurement into a deterministic control loop.

## Decision

We will implement an Anytime-Valid Coherence Gate that integrates three cutting-edge algorithmic components:

### 1. Dynamic Min-Cut with Witness Partitions

**Source**: El-Hayek, Henzinger, Li (arXiv:2512.13105, December 2025)

**Key Innovation**: Exact deterministic n^{o(1)} update time for cuts up to 2^{Θ(log^{3/4-c}n)}

**Integration**:
- Extends existing `SubpolynomialMinCut` in `ruvector-mincut/src/subpolynomial/mod.rs`
- Leverages existing `WitnessTree` for explicit partition certificates
- Uses deterministic `LocalKCut` for local cut verification

**Role in Gate**: Provides the **structural coherence signal** - identifies minimal intervention points in the agent action graph with explicit witness partitions showing which actions form the critical boundary to unsafe states.

### 2. Online Conformal Prediction with Shift-Awareness

**Sources**:
- Retrospective Adjustment (arXiv:2511.04275, November 2025)
- Conformal Optimistic Prediction (COP) (December 2025)
- CORE: RL-based Conformal Regression (October 2025)

**Key Innovation**: Distribution-free coverage guarantees that adapt to arbitrary distribution shift with faster recalibration via retrospective adjustment.

**Integration**:
- New module: `ruvector-mincut/src/conformal/` for prediction sets
- Interfaces with existing `GatePolicy` thresholds
- Wraps action outcome predictions with calibrated uncertainty

**Role in Gate**: Provides the **predictive uncertainty signal** - quantifies confidence in action outcomes, triggering DEFER when prediction sets are too large.

### 3. E-Values and E-Processes for Anytime-Valid Inference

**Sources**:
- Ramdas & Wang "Hypothesis Testing with E-values" (FnTStA 2025)
- ICML 2025 Tutorial on SAVI
- Sequential Randomization Tests (arXiv:2512.04366, December 2025)

**Key Innovation**: Evidence accumulation that remains valid at any stopping time, with multiplicative composition across experiments.

**Definition**: E-value e satisfies E[e] ≤ 1 under null hypothesis. E-processes are nonnegative supermartingales with E_0 = 1.

**Integration**:
- New module: `ruvector-mincut/src/eprocess/` for evidence tracking
- Integrates with existing `CutCertificate` for audit trails
- Enables anytime-valid stopping decisions

**Role in Gate**: Provides the **evidential validity signal** - accumulates statistical evidence for/against coherence with formal Type I error control at any stopping time.

## Gate Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    ANYTIME-VALID COHERENCE GATE                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐ │
│   │  DYNAMIC MIN-CUT │    │    CONFORMAL     │    │   E-PROCESS      │ │
│   │    (Structural)  │    │   (Predictive)   │    │  (Evidential)    │ │
│   │                  │    │                  │    │                  │ │
│   │  SubpolynomialMC │    │  ShiftAdaptive   │    │  CoherenceTest   │ │
│   │  WitnessTree     │───▶│  PredictionSet   │───▶│  EvidenceAccum   │ │
│   │  LocalKCut       │    │  COP/CORE        │    │  StoppingRule    │ │
│   └──────────────────┘    └──────────────────┘    └──────────────────┘ │
│            │                       │                       │           │
│            ▼                       ▼                       ▼           │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │                    DECISION LOGIC                              │   │
│   │                                                                │   │
│   │   PERMIT: E_t > τ_permit ∧ action ∉ CriticalCut ∧ |C_t| small │   │
│   │   DEFER:  |C_t| large ∨ τ_deny < E_t < τ_permit               │   │
│   │   DENY:   E_t < τ_deny ∨ action ∈ WitnessPartition(unsafe)    │   │
│   │                                                                │   │
│   └────────────────────────────────────────────────────────────────┘   │
│                               │                                        │
│                               ▼                                        │
│                    ┌─────────────────────┐                            │
│                    │   WITNESS RECEIPT   │                            │
│                    │  (cut + conf + e)   │                            │
│                    └─────────────────────┘                            │
└─────────────────────────────────────────────────────────────────────────┘
```

## Integration with Existing Architecture

### Extension Points

| Component | Current Implementation | AVCG Extension |
|-----------|----------------------|----------------|
| `GatePacket` | λ as point estimate | Add `lambda_confidence_q15`, `e_value_log_q15` |
| `GateController` | Rule-based thresholds | Add `AnytimeGatePolicy` with adaptive thresholds |
| `WitnessTree` | Cut value only | Add `ConfidenceWitness` with staleness tracking |
| `CutCertificate` | Static verification | Add `EvidenceReceipt` with e-value trace |
| `TierDecision` | Fixed tiers | Add `required_confidence_for_tier` |

### New Modules

```
ruvector-mincut/
├── src/
│   ├── conformal/           # NEW: Online conformal prediction
│   │   ├── mod.rs
│   │   ├── prediction_set.rs
│   │   ├── cop.rs           # Conformal Optimistic Prediction
│   │   ├── retrospective.rs # Retrospective adjustment
│   │   └── core.rs          # RL-based conformal
│   ├── eprocess/            # NEW: E-value and e-process tracking
│   │   ├── mod.rs
│   │   ├── evalue.rs
│   │   ├── evidence_accum.rs
│   │   ├── stopping.rs
│   │   └── mixture.rs
│   ├── anytime_gate/        # NEW: Integrated gate controller
│   │   ├── mod.rs
│   │   ├── policy.rs
│   │   ├── decision.rs
│   │   └── receipt.rs
│   └── ...existing modules...
```

## Decision Rules

### Permit Conditions (all must hold)
1. E-process value E_t > τ_permit (sufficient evidence of coherence)
2. Action not in witness partition of critical cut
3. Conformal prediction set |C_t| < θ_confidence (confident prediction)

### Defer Conditions (any triggers)
1. Conformal prediction set |C_t| > θ_uncertainty (uncertain outcome)
2. E-process in indeterminate range: τ_deny < E_t < τ_permit
3. Deadline approaching without sufficient confidence

### Deny Conditions (any triggers)
1. E-process value E_t < τ_deny (strong evidence of incoherence)
2. Action in witness partition crossing to unsafe states
3. Structural impossibility via min-cut topology

## Threshold Configuration

| Threshold | Meaning | Recommended Default |
|-----------|---------|---------------------|
| τ_deny | E-process level indicating incoherence | 0.01 (1% false alarm) |
| τ_permit | E-process level indicating coherence | 100 (strong evidence) |
| θ_uncertainty | Conformal set size requiring deferral | Task-dependent |
| θ_confidence | Conformal set size for confident permit | Task-dependent |

## Witness Receipt Structure

```rust
pub struct WitnessReceipt {
    /// Timestamp of decision
    pub timestamp: u64,
    /// Action that was evaluated
    pub action_id: ActionId,
    /// Gate decision
    pub decision: GateDecision,

    // Structural witness (from min-cut)
    pub cut_value: f64,
    pub witness_partition: (Vec<VertexId>, Vec<VertexId>),
    pub critical_edges: Vec<EdgeId>,

    // Predictive witness (from conformal)
    pub prediction_set: ConformalSet,
    pub coverage_target: f32,
    pub shift_adaptation_rate: f32,

    // Evidential witness (from e-process)
    pub e_value: f64,
    pub e_process_cumulative: f64,
    pub stopping_valid: bool,

    // Cryptographic seal
    pub receipt_hash: [u8; 32],
}
```

## Security Hardening

### Threat Model

| Threat Actor | Capabilities | Target | Impact |
|--------------|--------------|--------|--------|
| **Malicious Agent** | Action injection, timing manipulation | Gate bypass | Unauthorized actions executed |
| **Network Adversary** | Message interception, replay | Receipt forgery | False audit trail |
| **Insider Threat** | Threshold modification, key access | Policy manipulation | Safety guarantees voided |
| **Byzantine Node** | Arbitrary behavior in distributed gate | Consensus corruption | Inconsistent decisions |

### Cryptographic Requirements

#### Receipt Signing (CRITICAL)

```rust
pub struct WitnessReceipt {
    // ... existing fields ...

    // Cryptographic seal (REQUIRED)
    pub receipt_hash: [u8; 32],         // Blake3 hash of serialized content
    pub signature: Ed25519Signature,     // REQUIRED, not optional
    pub signer_id: PublicKey,           // Identity of signing gate
    pub timestamp_proof: TimestampProof, // Prevents backdating
}

/// Timestamp proof prevents replay and backdating
pub struct TimestampProof {
    pub timestamp: u64,
    pub previous_receipt_hash: [u8; 32], // Chain linkage
    pub merkle_root: [u8; 32],           // Batch anchor
}

impl WitnessReceipt {
    /// Sign receipt - MUST be called before any external use
    pub fn sign(&mut self, key: &SigningKey) -> Result<(), CryptoError> {
        let content = self.serialize_without_signature();
        self.receipt_hash = blake3::hash(&content).into();
        self.signature = key.sign(&self.receipt_hash);
        Ok(())
    }

    /// Verify receipt integrity and authenticity
    pub fn verify(&self, trusted_keys: &KeyStore) -> Result<(), VerifyError> {
        // 1. Verify hash
        let expected_hash = blake3::hash(&self.serialize_without_signature());
        if self.receipt_hash != expected_hash.into() {
            return Err(VerifyError::HashMismatch);
        }

        // 2. Verify signature
        let public_key = trusted_keys.get(&self.signer_id)?;
        public_key.verify(&self.receipt_hash, &self.signature)?;

        // 3. Verify timestamp chain
        self.timestamp_proof.verify()?;

        Ok(())
    }
}
```

#### Key Management

| Key Type | Purpose | Rotation | Storage |
|----------|---------|----------|---------|
| Gate Signing Key | Sign receipts | 30 days | HSM or secure enclave |
| Receipt Verification Keys | Verify receipts | On rotation | Distributed key store |
| Threshold Keys | Multi-party signing | 90 days | Shamir secret sharing |

### Attack Mitigations

#### E-Value Manipulation Prevention

```rust
/// Bounds checking for e-value inputs
impl EValue {
    pub fn from_likelihood_ratio(
        likelihood_h1: f64,
        likelihood_h0: f64,
    ) -> Result<Self, EValueError> {
        // Prevent division by zero
        if likelihood_h0 <= f64::EPSILON {
            return Err(EValueError::InvalidDenominator);
        }

        let ratio = likelihood_h1 / likelihood_h0;

        // Bound extreme values to prevent overflow attacks
        let bounded = ratio.clamp(E_VALUE_MIN, E_VALUE_MAX);

        // Log if clamping occurred (potential attack indicator)
        if (bounded - ratio).abs() > f64::EPSILON {
            security_log!("E-value clamped: {} -> {}", ratio, bounded);
        }

        Ok(Self { value: bounded, ..Default::default() })
    }
}

const E_VALUE_MIN: f64 = 1e-10;
const E_VALUE_MAX: f64 = 1e10;
```

#### Race Condition Prevention

```rust
/// Atomic gate decision with sequence numbers
pub struct AtomicGateDecision {
    /// Monotonic sequence for ordering
    sequence: AtomicU64,
    /// Lock for decision atomicity
    decision_lock: RwLock<()>,
}

impl AtomicGateDecision {
    pub async fn evaluate(&self, action: &Action) -> GateResult {
        // Acquire exclusive lock for decision
        let _guard = self.decision_lock.write().await;

        // Get sequence number BEFORE evaluation
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);

        // Evaluate all three signals atomically
        let result = self.evaluate_internal(action, seq).await;

        // Sequence number in receipt ensures ordering
        result.with_sequence(seq)
    }
}
```

#### Replay Attack Prevention

```rust
/// Replay prevention via nonce tracking
pub struct ReplayGuard {
    /// Recent action hashes (bloom filter for efficiency)
    recent_actions: BloomFilter,
    /// Sliding window of full hashes for false positive resolution
    hash_window: VecDeque<[u8; 32]>,
    /// Maximum age of tracked actions
    window_duration: Duration,
}

impl ReplayGuard {
    pub fn check_and_record(&mut self, action: &Action) -> Result<(), ReplayError> {
        let hash = action.content_hash();

        // Fast path: bloom filter check
        if self.recent_actions.might_contain(&hash) {
            // Slow path: verify against full hash window
            if self.hash_window.contains(&hash) {
                return Err(ReplayError::DuplicateAction { hash });
            }
        }

        // Record action
        self.recent_actions.insert(&hash);
        self.hash_window.push_back(hash);
        self.prune_old_entries();

        Ok(())
    }
}
```

### Trust Boundaries

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         TRUST BOUNDARY: GATE CORE                       │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │  • E-process computation    • Min-cut evaluation                 │  │
│  │  • Conformal prediction     • Decision logic                     │  │
│  │  • Receipt signing          • Key material                       │  │
│  │                                                                   │  │
│  │  Invariants:                                                      │  │
│  │  - All inputs validated before use                               │  │
│  │  - All outputs signed before release                             │  │
│  │  - No external calls during decision                             │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                         (authenticated channel)                         │
│                                    │                                    │
└────────────────────────────────────┼────────────────────────────────────┘
                                     │
┌────────────────────────────────────┼────────────────────────────────────┐
│                    TRUST BOUNDARY: AGENT INTERFACE                      │
│                                    │                                    │
│  • Action submission (validated)   │  • Decision receipt (verified)    │
│  • Context provision (sanitized)   │  • Witness query (authenticated)  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Performance Optimization

### Identified Bottlenecks & Solutions

#### 1. E-Process History Management

**Problem**: Unbounded history growth in `EProcess.history: Vec<EValue>`

**Solution**: Ring buffer with configurable retention

```rust
pub struct EProcess {
    /// Current accumulated value (always maintained)
    current: f64,

    /// Bounded history ring buffer
    history: RingBuffer<EValueSummary>,

    /// Checkpoint for long-term audit (sampled)
    checkpoints: Vec<EProcessCheckpoint>,
}

/// Compact summary for history
pub struct EValueSummary {
    value: f32,           // Reduced precision for storage
    timestamp: u32,       // Relative to epoch
    flags: u8,            // Metadata bits
}

impl EProcess {
    const HISTORY_CAPACITY: usize = 1024;
    const CHECKPOINT_INTERVAL: usize = 100;

    pub fn update(&mut self, e: EValue) {
        // Update current (always)
        self.current = self.update_rule.apply(self.current, e.value);

        // Add to ring buffer (bounded)
        self.history.push(e.to_summary());

        // Periodic checkpoint for audit
        if self.history.len() % Self::CHECKPOINT_INTERVAL == 0 {
            self.checkpoints.push(self.checkpoint());
        }
    }
}
```

#### 2. Min-Cut Hierarchy Updates

**Problem**: Sequential iteration over all hierarchy levels

**Solution**: Lazy propagation with dirty tracking

```rust
pub struct LazyHierarchy {
    levels: Vec<HierarchyLevel>,
    /// Bitmap of levels needing update
    dirty_levels: u64,
    /// Deferred updates queue
    pending_updates: VecDeque<DeferredUpdate>,
}

impl LazyHierarchy {
    pub fn insert(&mut self, edge: Edge) {
        // Only update lowest level immediately
        self.levels[0].insert(edge);
        self.dirty_levels |= 1;

        // Defer higher level updates
        self.pending_updates.push_back(DeferredUpdate::Insert(edge));
    }

    pub fn get_cut(&mut self) -> CutValue {
        // Propagate only if needed for query
        if self.dirty_levels != 0 {
            self.propagate_lazy();
        }
        self.levels.last().unwrap().cut_value()
    }

    fn propagate_lazy(&mut self) {
        // Process only dirty levels
        while self.dirty_levels != 0 {
            let level = self.dirty_levels.trailing_zeros() as usize;
            self.update_level(level);
            self.dirty_levels &= !(1 << level);
        }
    }
}
```

#### 3. SIMD-Optimized E-Value Computation

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Batch e-value computation with SIMD
pub fn compute_mixture_evalue_simd(
    likelihoods_h1: &[f64],
    likelihoods_h0: &[f64],
    weights: &[f64],
) -> f64 {
    assert_eq!(likelihoods_h1.len(), likelihoods_h0.len());
    assert_eq!(likelihoods_h1.len(), weights.len());

    #[cfg(target_feature = "avx2")]
    unsafe {
        let mut sum = _mm256_setzero_pd();

        for i in (0..likelihoods_h1.len()).step_by(4) {
            let h1 = _mm256_loadu_pd(likelihoods_h1.as_ptr().add(i));
            let h0 = _mm256_loadu_pd(likelihoods_h0.as_ptr().add(i));
            let w = _mm256_loadu_pd(weights.as_ptr().add(i));

            let ratio = _mm256_div_pd(h1, h0);
            let weighted = _mm256_mul_pd(ratio, w);
            sum = _mm256_add_pd(sum, weighted);
        }

        // Horizontal sum
        horizontal_sum_pd(sum)
    }

    #[cfg(not(target_feature = "avx2"))]
    {
        // Scalar fallback
        likelihoods_h1.iter()
            .zip(likelihoods_h0.iter())
            .zip(weights.iter())
            .map(|((h1, h0), w)| (h1 / h0) * w)
            .sum()
    }
}
```

#### 4. Receipt Serialization Optimization

```rust
/// Zero-copy receipt serialization
pub struct ReceiptBuffer {
    /// Pre-allocated buffer pool
    pool: BufferPool,
    /// Current buffer
    current: Buffer,
}

impl WitnessReceipt {
    /// Serialize to pre-allocated buffer (zero-copy)
    pub fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, SerializeError> {
        let mut cursor = 0;

        // Fixed-size header (no allocation)
        cursor += self.write_header(&mut buffer[cursor..])?;

        // Structural witness (fixed size)
        cursor += self.structural.write_to(&mut buffer[cursor..])?;

        // Predictive witness (bounded size)
        cursor += self.predictive.write_to(&mut buffer[cursor..])?;

        // Evidential witness (fixed size)
        cursor += self.evidential.write_to(&mut buffer[cursor..])?;

        // Hash and signature (fixed size)
        buffer[cursor..cursor + 32].copy_from_slice(&self.receipt_hash);
        cursor += 32;
        buffer[cursor..cursor + 64].copy_from_slice(&self.signature.to_bytes());
        cursor += 64;

        Ok(cursor)
    }
}
```

### Latency Budget (Revised)

| Component | Budget | Optimization | Measured p99 |
|-----------|--------|--------------|--------------|
| Min-cut query | 10ms | Lazy propagation | TBD |
| Conformal prediction | 15ms | Cached quantiles | TBD |
| E-process update | 5ms | SIMD mixture | TBD |
| Decision logic | 5ms | Short-circuit | TBD |
| Receipt generation | 10ms | Zero-copy serialize | TBD |
| Signing | 5ms | Ed25519 batch | TBD |
| **Total** | **50ms** | | |

---

## Distributed Coordination

### Multi-Agent Gate Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    DISTRIBUTED COHERENCE GATE                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐     │
│  │   REGIONAL      │    │   REGIONAL      │    │   REGIONAL      │     │
│  │   GATE (Raft)   │    │   GATE (Raft)   │    │   GATE (Raft)   │     │
│  │                 │    │                 │    │                 │     │
│  │  • Local cuts   │    │  • Local cuts   │    │  • Local cuts   │     │
│  │  • Local conf   │    │  • Local conf   │    │  • Local conf   │     │
│  │  • Local e-proc │    │  • Local e-proc │    │  • Local e-proc │     │
│  └────────┬────────┘    └────────┬────────┘    └────────┬────────┘     │
│           │                      │                      │              │
│           └──────────────────────┼──────────────────────┘              │
│                                  │                                     │
│                    ┌─────────────▼─────────────┐                       │
│                    │   GLOBAL COORDINATOR      │                       │
│                    │   (DAG Consensus)         │                       │
│                    │                           │                       │
│                    │  • Cross-region cuts      │                       │
│                    │  • Aggregated e-process   │                       │
│                    │  • Boundary arbitration   │                       │
│                    └───────────────────────────┘                       │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Hierarchical Decision Protocol

```rust
/// Distributed gate with hierarchical coordination
pub struct DistributedGateController {
    /// Local gate for fast-path decisions
    local_gate: AnytimeGateController,

    /// Regional coordinator (Raft consensus)
    regional: RegionalCoordinator,

    /// Global coordinator (DAG consensus)
    global: GlobalCoordinator,

    /// Decision routing policy
    routing: DecisionRoutingPolicy,
}

pub enum DecisionScope {
    /// Action affects only local partition
    Local,
    /// Action crosses regional boundary
    Regional,
    /// Action has global implications
    Global,
}

impl DistributedGateController {
    pub async fn evaluate(&mut self, action: &Action, context: &Context) -> GateResult {
        // 1. Determine scope
        let scope = self.routing.classify(action, context);

        // 2. Route to appropriate level
        match scope {
            DecisionScope::Local => {
                // Fast path: local decision only
                self.local_gate.evaluate(action, context)
            }

            DecisionScope::Regional => {
                // Medium path: coordinate with regional peers
                let local_result = self.local_gate.evaluate(action, context);
                let regional_result = self.regional.coordinate(action, &local_result).await?;
                self.merge_results(local_result, regional_result)
            }

            DecisionScope::Global => {
                // Slow path: full coordination
                let local_result = self.local_gate.evaluate(action, context);
                let regional_result = self.regional.coordinate(action, &local_result).await?;
                let global_result = self.global.arbitrate(action, &regional_result).await?;
                self.merge_all_results(local_result, regional_result, global_result)
            }
        }
    }
}
```

### Distributed E-Process Aggregation

```rust
/// E-process that aggregates across distributed gates
pub struct DistributedEProcess {
    /// Local e-process
    local: EProcess,

    /// Peer e-process summaries (received via gossip)
    peer_summaries: HashMap<NodeId, EProcessSummary>,

    /// Aggregation method
    aggregation: AggregationMethod,
}

pub enum AggregationMethod {
    /// Conservative: minimum across all nodes
    Minimum,
    /// Average with confidence weighting
    WeightedAverage,
    /// Consensus-based (requires agreement)
    Consensus { threshold: f64 },
}

impl DistributedEProcess {
    /// Get aggregated e-value for distributed decision
    pub fn aggregated_value(&self) -> f64 {
        match self.aggregation {
            AggregationMethod::Minimum => {
                let local = self.local.current_value();
                let peer_min = self.peer_summaries.values()
                    .map(|s| s.current_value)
                    .fold(f64::INFINITY, f64::min);
                local.min(peer_min)
            }

            AggregationMethod::WeightedAverage => {
                let total_weight: f64 = 1.0 + self.peer_summaries.values()
                    .map(|s| s.confidence_weight)
                    .sum::<f64>();

                let weighted_sum = self.local.current_value()
                    + self.peer_summaries.values()
                        .map(|s| s.current_value * s.confidence_weight)
                        .sum::<f64>();

                weighted_sum / total_weight
            }

            AggregationMethod::Consensus { threshold } => {
                // Requires threshold fraction of nodes to agree
                let values: Vec<f64> = std::iter::once(self.local.current_value())
                    .chain(self.peer_summaries.values().map(|s| s.current_value))
                    .collect();

                // Return median if sufficient agreement, else conservative min
                if self.check_agreement(&values, threshold) {
                    statistical_median(&values)
                } else {
                    values.iter().cloned().fold(f64::INFINITY, f64::min)
                }
            }
        }
    }
}
```

### Fault Tolerance

```rust
/// Fault-tolerant gate with automatic failover
pub struct FaultTolerantGate {
    /// Primary gate
    primary: AnytimeGateController,

    /// Standby gates (hot standbys)
    standbys: Vec<AnytimeGateController>,

    /// Health monitor
    health: HealthMonitor,

    /// Failover policy
    failover: FailoverPolicy,
}

pub struct FailoverPolicy {
    /// Maximum consecutive failures before failover
    max_failures: u32,
    /// Health check interval
    check_interval: Duration,
    /// Recovery grace period
    recovery_grace: Duration,
}

impl FaultTolerantGate {
    pub async fn evaluate(&mut self, action: &Action, context: &Context) -> GateResult {
        // Try primary
        match self.try_primary(action, context).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                self.health.record_failure(&e);
            }
        }

        // Failover to standbys
        for (idx, standby) in self.standbys.iter_mut().enumerate() {
            match standby.evaluate(action, context) {
                Ok(result) => {
                    // Promote standby if primary unhealthy
                    if self.health.should_failover() {
                        self.promote_standby(idx);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    self.health.record_standby_failure(idx, &e);
                }
            }
        }

        // All gates failed - safe default
        Ok(GateResult {
            decision: GateDecision::Deny,
            reason: "All gates unavailable - failing safe".into(),
            ..Default::default()
        })
    }
}
```

### Integration with RuVector Consensus

| Consensus Layer | RuVector Module | Gate Integration |
|-----------------|-----------------|------------------|
| Regional (Raft) | `ruvector-raft` | Local cut coordination, leader-based decisions |
| Global (DAG) | `ruvector-cluster` | Cross-region boundary arbitration |
| State Sync | `ruvector-sync` | E-process summary propagation |
| Receipt Chain | `ruvector-merkle` | Distributed receipt verification |

---

## Hardware Mapping: 256-Tile WASM Fabric

The coherence gate is an ideal workload for event-driven WASM hardware: **mostly silent, then extremely decisive when boundaries move**.

### Tile Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         256-TILE COGNITUM FABRIC                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                        TILE ZERO (Arbiter)                       │   │
│  │                                                                  │   │
│  │  • Merge worker reports      • Hierarchical min-cut             │   │
│  │  • Global gate decision      • Permit token issuance            │   │
│  │  • Witness receipt log       • Hash-chained eventlog            │   │
│  └──────────────────────────────┬───────────────────────────────────┘   │
│                                 │                                       │
│            ┌────────────────────┼────────────────────┐                 │
│            │                    │                    │                  │
│            ▼                    ▼                    ▼                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐             │
│  │  Workers     │    │  Workers     │    │  Workers     │   ...       │
│  │  [1-85]      │    │  [86-170]    │    │  [171-255]   │             │
│  │              │    │              │    │              │             │
│  │  Shard A     │    │  Shard B     │    │  Shard C     │             │
│  │  Local cuts  │    │  Local cuts  │    │  Local cuts  │             │
│  │  E-accum     │    │  E-accum     │    │  E-accum     │             │
│  └──────────────┘    └──────────────┘    └──────────────┘             │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Worker Tile Responsibilities

Each of the 255 worker tiles maintains a **local shard**:

```rust
/// Worker tile state (fits in ~64KB WASM memory)
#[repr(C)]
pub struct WorkerTileState {
    /// Compact neighborhood graph (edges + weights)
    graph_shard: CompactGraph,          // ~32KB

    /// Rolling feature window for normality scores
    feature_window: RingBuffer<f32>,    // ~8KB

    /// Local coherence score
    coherence: f32,

    /// Local boundary candidates (top-k edges)
    boundary_edges: [EdgeId; 8],

    /// Local e-value accumulator
    e_accumulator: f64,

    /// Tick counter
    tick: u64,
}

/// Per-tick processing: only deltas
impl WorkerTileState {
    /// Process incoming delta (edge add/remove/weight update)
    pub fn ingest_delta(&mut self, delta: &Delta) -> Status {
        match delta {
            Delta::EdgeAdd(e) => self.graph_shard.add_edge(e),
            Delta::EdgeRemove(e) => self.graph_shard.remove_edge(e),
            Delta::WeightUpdate(e, w) => self.graph_shard.update_weight(e, *w),
            Delta::Observation(score) => self.feature_window.push(*score),
        }
        self.update_local_coherence();
        Status::Ok
    }

    /// Tick: compute and emit report
    pub fn tick(&mut self, now_ns: u64) -> TileReport {
        self.tick = now_ns;

        // Tiny math: update e-accumulator
        self.e_accumulator = self.compute_local_evalue();

        TileReport {
            tile_id: self.id,
            coherence: self.coherence,
            boundary_moved: self.detect_boundary_movement(),
            suspicious_edges: self.top_k_suspicious(),
            e_value: self.e_accumulator as f32,
            witness_fragment: self.extract_witness_fragment(),
        }
    }
}

/// Fixed-size report (fits in single cache line)
#[repr(C, align(64))]
pub struct TileReport {
    tile_id: u8,
    coherence: f32,
    boundary_moved: bool,
    suspicious_edges: [EdgeId; 4],
    e_value: f32,
    witness_fragment: WitnessFragment,
}
```

### TileZero Responsibilities

TileZero acts as the **arbiter** that issues final decisions:

```rust
/// TileZero: Global gate decision and permit issuance
pub struct TileZero {
    /// Merged supergraph (reduced from worker summaries)
    supergraph: ReducedGraph,

    /// Canonical permit token state
    permit_state: PermitState,

    /// Hash-chained witness receipt log
    receipt_log: ReceiptLog,

    /// Threshold configuration
    thresholds: GateThresholds,
}

impl TileZero {
    /// Collect reports from all worker tiles
    pub fn collect_reports(&mut self, reports: &[TileReport; 255]) {
        // Merge worker summaries into supergraph
        for report in reports {
            if report.boundary_moved {
                self.supergraph.update_from_fragment(&report.witness_fragment);
            }
            self.supergraph.update_coherence(report.tile_id, report.coherence);
        }
    }

    /// Issue gate decision (microsecond latency)
    pub fn decide(&mut self, action_ctx: &ActionContext) -> PermitToken {
        // Three stacked filters:

        // 1. Structural filter (global cut on reduced graph)
        let structural_ok = self.supergraph.global_cut() >= self.thresholds.min_cut;

        // 2. Shift filter (aggregated shift pressure)
        let shift_pressure = self.aggregate_shift_pressure();
        let shift_ok = shift_pressure < self.thresholds.max_shift;

        // 3. Evidence filter (can stop immediately if enough evidence)
        let e_aggregate = self.aggregate_evidence();
        let evidence_decision = self.evidence_decision(e_aggregate);

        // Combined decision
        let decision = match (structural_ok, shift_ok, evidence_decision) {
            (false, _, _) => GateDecision::Deny,  // Structure broken
            (_, false, _) => GateDecision::Defer, // Shift detected
            (_, _, EvidenceDecision::Reject) => GateDecision::Deny,
            (_, _, EvidenceDecision::Continue) => GateDecision::Defer,
            (true, true, EvidenceDecision::Accept) => GateDecision::Permit,
        };

        // Issue token
        self.issue_permit_token(action_ctx, decision)
    }

    /// Issue permit token (a signed capability)
    fn issue_permit_token(
        &mut self,
        ctx: &ActionContext,
        decision: GateDecision,
    ) -> PermitToken {
        let witness_hash = self.compute_witness_hash();

        let token = PermitToken {
            decision,
            action_id: ctx.action_id,
            timestamp: now_ns(),
            ttl_ns: self.thresholds.permit_ttl,
            witness_hash,
            sequence: self.permit_state.next_sequence(),
        };

        // MAC or sign the token
        let mac = self.permit_state.sign(&token);

        // Emit receipt
        self.emit_receipt(&token, &mac);

        PermitToken { mac, ..token }
    }

    /// Emit witness receipt (hash-chained)
    fn emit_receipt(&mut self, token: &PermitToken, mac: &[u8; 32]) {
        let receipt = WitnessReceipt {
            token: token.clone(),
            mac: *mac,
            previous_hash: self.receipt_log.last_hash(),
            witness_summary: self.supergraph.witness_summary(),
        };

        self.receipt_log.append(receipt);
    }
}

/// Permit token: a capability that agents must present
#[repr(C)]
pub struct PermitToken {
    pub decision: GateDecision,
    pub action_id: ActionId,
    pub timestamp: u64,
    pub ttl_ns: u64,
    pub witness_hash: [u8; 32],
    pub sequence: u64,
    pub mac: [u8; 32],  // HMAC or signature
}

impl PermitToken {
    /// Agents must present valid token to perform actions
    pub fn is_valid(&self, verifier: &Verifier) -> bool {
        // Check TTL
        if now_ns() > self.timestamp + self.ttl_ns {
            return false;
        }

        // Verify MAC/signature
        verifier.verify(self, &self.mac)
    }
}
```

### WASM Kernel API

Each tile runs a minimal WASM kernel:

```rust
/// Worker tile WASM exports
#[no_mangle]
pub extern "C" fn ingest_delta(delta_ptr: *const u8, len: usize) -> u32 {
    let delta = unsafe { core::slice::from_raw_parts(delta_ptr, len) };
    TILE_STATE.with(|state| state.borrow_mut().ingest_delta(delta))
}

#[no_mangle]
pub extern "C" fn tick(now_ns: u64) -> *const TileReport {
    TILE_STATE.with(|state| state.borrow_mut().tick(now_ns))
}

#[no_mangle]
pub extern "C" fn get_witness_fragment(id: u32) -> *const u8 {
    TILE_STATE.with(|state| state.borrow().get_witness_fragment(id))
}

/// TileZero WASM/native exports
#[no_mangle]
pub extern "C" fn collect_reports(reports_ptr: *const TileReport, count: usize) {
    TILEZERO.with(|tz| tz.borrow_mut().collect_reports(reports_ptr, count))
}

#[no_mangle]
pub extern "C" fn decide(action_ctx_ptr: *const ActionContext) -> *const PermitToken {
    TILEZERO.with(|tz| tz.borrow_mut().decide(action_ctx_ptr))
}

#[no_mangle]
pub extern "C" fn get_receipt(sequence: u64) -> *const WitnessReceipt {
    TILEZERO.with(|tz| tz.borrow().get_receipt(sequence))
}
```

### v0 Implementation Strategy

Ship fast by layering:

| Phase | Components | Skip Initially |
|-------|------------|----------------|
| **v0.1** | Structural coherence + witness receipt | Shift filter, evidence filter |
| **v0.2** | Add shift filter (normality scores) | CORE RL adaptation |
| **v0.3** | Add evidence filter (e-values) | Mixture e-values |
| **v1.0** | Full three-filter stack | - |

### Rust Deliverables

| Crate | Description | Dependencies |
|-------|-------------|--------------|
| `cognitum-gate-kernel` | `no_std` WASM kernel for worker tiles | `ruvector-mincut` (core algorithms) |
| `cognitum-gate-tilezero` | Native arbiter for TileZero | `ruvector-mincut`, `blake3`, `ed25519` |
| `mcp-gate` | MCP server for agent integration | `cognitum-gate-tilezero` |

```
cognitum-gate/
├── cognitum-gate-kernel/      # no_std WASM
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs             # WASM exports
│       ├── shard.rs           # Compact graph shard
│       ├── evidence.rs        # Local e-accumulator
│       └── report.rs          # TileReport generation
│
├── cognitum-gate-tilezero/    # Native arbiter
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── merge.rs           # Report merging
│       ├── supergraph.rs      # Reduced global graph
│       ├── permit.rs          # Token issuance
│       └── receipt.rs         # Hash-chained log
│
└── mcp-gate/                  # MCP integration
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── tools.rs           # permit_action, get_receipt, replay_decision
        └── server.rs          # MCP server
```

### MCP Gate Tools

```rust
/// MCP tool: Request permission for an action
#[mcp_tool]
pub async fn permit_action(
    action_id: String,
    action_type: String,
    context: serde_json::Value,
) -> Result<PermitResponse, McpError> {
    let ctx = ActionContext::from_json(&context)?;
    let token = TILEZERO.decide(&ctx);

    Ok(PermitResponse {
        decision: token.decision.to_string(),
        token: token.encode_base64(),
        witness_hash: hex::encode(&token.witness_hash),
        valid_until_ns: token.timestamp + token.ttl_ns,
    })
}

/// MCP tool: Get witness receipt for audit
#[mcp_tool]
pub async fn get_receipt(sequence: u64) -> Result<ReceiptResponse, McpError> {
    let receipt = TILEZERO.get_receipt(sequence)?;

    Ok(ReceiptResponse {
        sequence,
        decision: receipt.token.decision.to_string(),
        timestamp: receipt.token.timestamp,
        witness_summary: receipt.witness_summary.to_json(),
        previous_hash: hex::encode(&receipt.previous_hash),
        receipt_hash: hex::encode(&receipt.hash()),
    })
}

/// MCP tool: Replay decision for debugging/audit
#[mcp_tool]
pub async fn replay_decision(
    sequence: u64,
    verify_chain: bool,
) -> Result<ReplayResponse, McpError> {
    let receipt = TILEZERO.get_receipt(sequence)?;

    // Optionally verify hash chain
    if verify_chain {
        TILEZERO.verify_chain_to(sequence)?;
    }

    // Replay the decision with logged state
    let replayed = TILEZERO.replay(&receipt)?;

    Ok(ReplayResponse {
        original_decision: receipt.token.decision.to_string(),
        replayed_decision: replayed.decision.to_string(),
        match_confirmed: receipt.token.decision == replayed.decision,
        state_snapshot: replayed.state_snapshot.to_json(),
    })
}
```

### The Practical Win

This gives Cognitum a clear job that buyers understand:

> **"We do not just detect issues, we prevent unsafe actions."**
> **"We can prove why we blocked or allowed it."**
> **"We stay calm until structure breaks."**

The permit token as a capability means:
- Agents cannot act without presenting a valid token
- Tokens expire (TTL-bounded)
- Every token is backed by a witness receipt
- The entire chain is cryptographically verifiable

---

## API Contract

### Request: Permit Action

```json
{
  "action_id": "cfg-push-7a3f",
  "action_type": "config_change",
  "target": {
    "device": "router-west-03",
    "path": "/network/interfaces/eth0"
  },
  "context": {
    "agent_id": "ops-agent-12",
    "session_id": "sess-abc123",
    "prior_actions": ["cfg-push-7a3e"],
    "urgency": "normal"
  }
}
```

### Response: Permit

```json
{
  "decision": "permit",
  "token": "eyJ0eXAiOiJQVCIsImFsZyI6IkVkMjU1MTkifQ...",
  "valid_until_ns": 1737158400000000000,
  "witness": {
    "structural": {
      "cut_value": 12.7,
      "partition": "stable",
      "critical_edges": 0
    },
    "predictive": {
      "set_size": 3,
      "coverage": 0.92
    },
    "evidential": {
      "e_value": 847.3,
      "verdict": "accept"
    }
  },
  "receipt_sequence": 1847392
}
```

### Response: Defer

```json
{
  "decision": "defer",
  "reason": "shift_detected",
  "detail": "Distribution shift pressure 0.73 exceeds threshold 0.5",
  "escalation": {
    "to": "human_operator",
    "context_url": "/receipts/1847393/context",
    "timeout_ns": 300000000000
  },
  "witness": {
    "structural": { "cut_value": 11.2, "partition": "stable" },
    "predictive": { "set_size": 18, "coverage": 0.91 },
    "evidential": { "e_value": 3.2, "verdict": "continue" }
  },
  "receipt_sequence": 1847393
}
```

### Response: Deny

```json
{
  "decision": "deny",
  "reason": "boundary_violation",
  "detail": "Action crosses fragile partition (cut=2.1 < min=5.0)",
  "witness": {
    "structural": {
      "cut_value": 2.1,
      "partition": "fragile",
      "critical_edges": 4,
      "boundary": ["edge-17", "edge-23", "edge-41", "edge-52"]
    },
    "predictive": { "set_size": 47, "coverage": 0.88 },
    "evidential": { "e_value": 0.004, "verdict": "reject" }
  },
  "receipt_sequence": 1847394
}
```

---

## Migration Path

### Phase M1: Shadow Mode

Run AVCG alongside existing `GateController`. Compare decisions, don't enforce.

```rust
impl HybridGate {
    pub fn evaluate(&mut self, action: &Action) -> GateResult {
        // Existing gate makes the decision
        let legacy_result = self.legacy_gate.evaluate(action);

        // AVCG runs in shadow, logs disagreements
        let avcg_result = self.avcg_gate.evaluate(action);

        if legacy_result.decision != avcg_result.decision {
            metrics::counter!("gate.shadow.disagreement").increment(1);
            log::info!(
                "Shadow disagreement: legacy={:?} avcg={:?} action={}",
                legacy_result.decision,
                avcg_result.decision,
                action.id
            );
        }

        legacy_result  // Legacy still decides
    }
}
```

**Exit criteria**: <1% disagreement rate over 7 days, zero false denies on known-safe actions.

### Phase M2: Canary Enforcement

AVCG enforces for 5% of traffic, legacy handles rest.

```rust
impl CanaryGate {
    pub fn evaluate(&mut self, action: &Action) -> GateResult {
        let canary = self.canary_selector.select(action);

        if canary {
            metrics::counter!("gate.canary.avcg").increment(1);
            self.avcg_gate.evaluate(action)
        } else {
            self.legacy_gate.evaluate(action)
        }
    }
}
```

**Exit criteria**: No incidents attributed to AVCG decisions over 14 days.

### Phase M3: Majority Rollout

AVCG handles 95%, legacy available for fallback.

### Phase M4: Full Cutover

Legacy removed. AVCG is the gate.

```
Timeline:
M1 (Shadow)     → 2-4 weeks
M2 (Canary 5%)  → 2 weeks
M3 (Majority)   → 2 weeks
M4 (Full)       → 1 week
                  ─────────
Total           → 7-9 weeks
```

---

## Observability

### Metrics (Prometheus)

```
# Decision counters
gate_decisions_total{decision="permit|defer|deny", reason="..."}

# Latency histograms
gate_latency_seconds{phase="mincut|conformal|eprocess|decision|receipt"}

# Signal values
gate_cut_value{quantile="0.5|0.9|0.99"}
gate_prediction_set_size{quantile="0.5|0.9|0.99"}
gate_evalue{quantile="0.5|0.9|0.99"}

# Health
gate_healthy{component="mincut|conformal|eprocess"}
gate_failover_total{from="primary|standby_N"}

# Coverage tracking
gate_conformal_coverage_rate  # Should stay ≥ 0.85
gate_eprocess_power           # Evidence accumulation rate
```

### Alerting Thresholds

| Alert | Condition | Severity |
|-------|-----------|----------|
| `GateHighDenyRate` | deny_rate > 10% for 5m | Warning |
| `GateLatencyHigh` | p99 > 100ms for 5m | Warning |
| `GateCoverageDrift` | coverage < 0.80 for 15m | Critical |
| `GateUnhealthy` | any component unhealthy for 1m | Critical |
| `GateReceiptChainBroken` | hash verification fails | Critical |

### Debug Query: Why Was This Denied?

```bash
# Get full decision context
curl /api/gate/receipts/1847394/explain

# Response:
{
  "receipt_sequence": 1847394,
  "decision": "deny",
  "explanation": {
    "primary_reason": "structural",
    "structural": {
      "cut_value": 2.1,
      "threshold": 5.0,
      "failed": true,
      "boundary_edges": [
        {"id": "edge-17", "weight": 0.3, "endpoints": ["node-a", "node-b"]},
        ...
      ],
      "partition_context": "Device router-west-03 is in partition P7 which has been unstable since 14:32:07 UTC"
    },
    "predictive": { "failed": false, "detail": "Set size 47 within bounds" },
    "evidential": { "failed": true, "detail": "E-value 0.004 < τ_deny 0.01" }
  },
  "suggested_action": "Wait for partition P7 to stabilize or escalate to human approval",
  "similar_past_decisions": [1847201, 1846998, 1846754]
}
```

---

## Open Questions Resolution

### Q1: Graph model scope — immediate actions or multi-step lookahead?

**Decision**: Immediate actions for v0, optional 1-step lookahead for v1.

**Rationale**: Multi-step lookahead requires predicting action sequences, which adds latency and complexity. Start simple: evaluate the action being requested *right now*. If the current action is safe but would lead to an unsafe state, the *next* action will be denied when it's requested.

### Q2: E-process null — "action safety" vs "policy consistency"?

**Decision**: Action safety as primary null, policy consistency as secondary.

**Rationale**:
- Primary H₀: P(action leads to unsafe state) ≤ p₀
- Secondary (optional): Current action consistent with established policy

Action safety is more fundamental. Policy consistency can be added as a separate e-process that runs in parallel.

### Q3: Threshold learning — fixed or adaptive?

**Decision**: Fixed for v0, adaptive via meta-learning for v1.

**Rationale**: Fixed thresholds are easier to audit and explain. Once we have production data, we can train a meta-learner to adjust thresholds based on observed false positive/negative rates.

### Q4: Human-in-loop — how are DEFER decisions presented?

**Decision**: Structured escalation with timeout and context link.

```json
{
  "escalation": {
    "to": "human_operator",
    "channel": "slack|pagerduty|dashboard",
    "context_url": "/receipts/{seq}/context",
    "timeout_ns": 300000000000,
    "default_on_timeout": "deny"
  }
}
```

If human doesn't respond within timeout, default to DENY (fail-safe).

See **Hybrid Agent/Human Workflow** section for full details.

### Q5: Adversarial robustness — adaptive adversaries?

**Decision**: Defense in depth + rate limiting + anomaly detection.

**Mitigations**:
1. Rate limit: Max N decisions per agent per minute
2. Anomaly: Flag agents with unusually high deny rates
3. Honeypots: Inject synthetic "trap" actions to detect probing
4. Rotation: Periodically rotate threshold parameters within safe bounds

---

## Definition of Done

### v0.1 Shippable Criteria

| Criterion | Metric | Target |
|-----------|--------|--------|
| **Structural filter works** | Min-cut correctly identifies fragile partitions | 100% on test suite |
| **Receipts are signed** | All receipts have valid Ed25519 signature | 100% |
| **Receipts are chained** | Hash chain verifies for all receipts | 100% |
| **Latency acceptable** | p99 gate decision time | < 50ms |
| **No false denies** | Known-safe actions are permitted | 100% on test suite |
| **Demo scenario runs** | Network security control plane demo | End-to-end pass |

### v0.1 Minimum Viable Demo

**Scenario**: Agent requests config push to network device.

1. Agent calls `permit_action` with device target
2. Gate evaluates structural coherence (min-cut)
3. Gate returns PERMIT with signed receipt
4. Agent presents token to device
5. Device verifies token, accepts config

**Success**: Auditor can replay decision from receipt and get same result.

---

## Cost Model

### Memory per Tile (WASM)

| Component | Size | Notes |
|-----------|------|-------|
| Graph shard | 32 KB | ~2000 edges at 16 bytes each |
| Feature window | 8 KB | 2048 f32 values |
| E-accumulator | 64 B | f64 + metadata |
| Boundary edges | 64 B | 8 × EdgeId |
| **Total per worker** | **~41 KB** | Fits in 64KB WASM page |
| **Total 255 workers** | **~10.2 MB** | |
| TileZero state | ~1 MB | Supergraph + receipt log head |
| **Total fabric** | **~12 MB** | |

### Network Bandwidth

| Flow | Frequency | Size | Bandwidth |
|------|-----------|------|-----------|
| Worker → TileZero reports | 1/tick (10ms) | 64 B × 255 | ~1.6 MB/s |
| Receipt log append | per decision | ~512 B | Variable |
| Gossip (distributed) | 1/100ms | ~1 KB × peers | ~10 KB/s × P |

### Storage Growth

| Item | Size | Retention | Growth |
|------|------|-----------|--------|
| Receipt | ~512 B | 90 days | ~44 MB/day @ 1000 decisions/s |
| E-process checkpoint | ~128 B | Forever | ~11 MB/day @ 1000 decisions/s |
| Audit log | ~256 B | 1 year | ~22 MB/day @ 1000 decisions/s |

**90-day storage**: ~7 GB receipts + ~1 GB checkpoints ≈ **8 GB**

---

## Hybrid Agent/Human Workflow

The coherence gate is designed for **bounded autonomy**, not full autonomy. Humans stay in the loop at critical decision points.

### Design Philosophy

```
┌─────────────────────────────────────────────────────────────────────────┐
│                                                                         │
│   "Agents handle the routine. Humans handle the novel."                │
│                                                                         │
│   PERMIT  → Agent proceeds autonomously (low risk, high confidence)    │
│   DEFER   → Human decides (uncertain, boundary case, policy gap)       │
│   DENY    → Blocked automatically (structural violation, unsafe)       │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

The gate doesn't replace human judgment—it **routes decisions to humans when judgment is needed**.

### Escalation Tiers

| Tier | Trigger | Responder | SLA | Example |
|------|---------|-----------|-----|---------|
| **T0** | PERMIT | None (automated) | 0 | Routine config within stable partition |
| **T1** | DEFER (shift) | On-call operator | 5 min | New dependency pattern detected |
| **T2** | DEFER (boundary) | Senior engineer | 15 min | Action crosses partition boundary |
| **T3** | DEFER (policy gap) | Policy team | 1 hour | No precedent for this action type |
| **T4** | DENY override request | Security + Management | 4 hours | Agent requesting exception to denial |

### Human Decision Interface

When a DEFER is escalated, humans see:

```
┌─────────────────────────────────────────────────────────────────────────┐
│  DECISION REQUIRED                                    Timeout: 4:32    │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Agent: ops-agent-12                                                   │
│  Action: Push config to router-west-03 /network/interfaces/eth0        │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  WHY DEFERRED                                                    │   │
│  │                                                                  │   │
│  │  • Shift detected: New dependency pattern (0.73 > 0.5 threshold)│   │
│  │  • This device was added to the graph 2 hours ago               │   │
│  │  • Similar actions on established devices: 847 permits, 0 denies│   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  CONTEXT                                                         │   │
│  │                                                                  │   │
│  │  Structural coherence: 11.2 (healthy)                           │   │
│  │  Prediction set size: 18 outcomes (moderate uncertainty)        │   │
│  │  Evidence accumulator: 3.2 (inconclusive)                       │   │
│  │                                                                  │   │
│  │  [View full witness receipt] [View similar past decisions]      │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────────────────┐   │
│  │   APPROVE     │  │    DENY       │  │  ESCALATE TO T3           │   │
│  │   (proceed)   │  │   (block)     │  │  (need policy guidance)   │   │
│  └───────────────┘  └───────────────┘  └───────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Human Decision Recording

Human decisions become part of the audit trail:

```rust
pub struct HumanDecision {
    /// Original deferred receipt
    pub deferred_receipt_seq: u64,

    /// Human's decision
    pub decision: HumanVerdict,

    /// Human identity (authenticated)
    pub decider_id: AuthenticatedUserId,

    /// Reasoning (required for audit)
    pub rationale: String,

    /// Timestamp
    pub decided_at: u64,

    /// Signature (human signs their decision)
    pub signature: Ed25519Signature,
}

pub enum HumanVerdict {
    /// Approve the action
    Approve {
        /// Add to training data for future automation
        learn_from_this: bool,
    },
    /// Deny the action
    Deny {
        /// Reason for denial
        reason: String,
    },
    /// Escalate to higher tier
    Escalate {
        to_tier: EscalationTier,
        reason: String,
    },
    /// Request more information
    NeedMoreInfo {
        questions: Vec<String>,
    },
}
```

### Override Protocol

Humans can override DENY decisions, but with friction and accountability:

```rust
pub struct DenyOverride {
    /// Which denial is being overridden
    pub denied_receipt_seq: u64,

    /// Who is overriding (must be T4 authority)
    pub overrider_id: AuthenticatedUserId,

    /// Second approver required
    pub second_approver_id: AuthenticatedUserId,

    /// Business justification (required, min 50 chars)
    pub justification: String,

    /// Time-bounded: override expires
    pub valid_until: u64,

    /// Scope-limited: only this specific action
    pub action_id: ActionId,

    /// Both signatures required
    pub overrider_signature: Ed25519Signature,
    pub approver_signature: Ed25519Signature,
}
```

**Override constraints**:
- Two humans required (four-eyes principle)
- Must provide written justification
- Time-limited (max 24 hours)
- Scope-limited (only the specific action)
- All overrides flagged for security review

### Learning from Human Decisions

Human decisions improve the gate over time:

```rust
/// When human approves a DEFER, optionally learn from it
pub fn learn_from_approval(
    deferred: &WitnessReceipt,
    human: &HumanDecision,
) {
    if human.decision.learn_from_this() {
        // Add to calibration data
        conformal_calibrator.add_observation(
            deferred.context.clone(),
            Outcome::Safe,  // Human judged it safe
        );

        // Update e-process null hypothesis
        eprocess_trainer.add_positive_example(
            deferred.action.clone(),
        );

        // Adjust threshold candidates (for meta-learning in v1)
        threshold_learner.record_human_permit(
            deferred.signals.clone(),
        );
    }
}
```

### Workload Distribution Target

The goal is **minimal human burden** while maintaining safety:

| Decision | Target Rate | Human Workload |
|----------|-------------|----------------|
| PERMIT | 90-95% | Zero |
| DEFER | 4-9% | Human decides |
| DENY | 1-2% | Zero (unless override requested) |

If DEFER rate exceeds 10%, the gate is too conservative—tune thresholds.
If DENY rate exceeds 5%, something is wrong—investigate root cause.

### Integration Channels

| Channel | Use Case | Response Format |
|---------|----------|-----------------|
| **Slack** | On-call escalation | Interactive buttons |
| **PagerDuty** | Critical/timed decisions | Acknowledge + decision API |
| **Dashboard** | Batch review | Web UI with full context |
| **CLI** | Developer/ops workflow | `ruvector gate approve <seq>` |
| **API** | Programmatic integration | REST/gRPC |

### Audit Trail for Human Decisions

Every human decision is:
1. **Authenticated**: Decider identity verified via SSO/MFA
2. **Signed**: Human signs their decision with personal key
3. **Chained**: Added to the same receipt chain as gate decisions
4. **Timestamped**: Immutable record of when decision was made
5. **Justified**: Rationale captured for later review

```
Receipt Chain:
  [1847392] PERMIT (automated) → agent executed
  [1847393] DEFER (automated) → escalated to human
  [1847393-H] APPROVE (human: alice@corp) → agent executed
  [1847394] DENY (automated) → blocked
  [1847394-O] OVERRIDE (humans: bob@corp + carol@corp) → exception granted
```

---

## Consequences

### Benefits

1. **Formal Guarantees**: Type I error control at any stopping time
2. **Distribution Shift Robustness**: Conformal prediction adapts without retraining
3. **Computational Efficiency**: O(n^{o(1)}) update time from subpolynomial min-cut
4. **Audit Trail**: Every decision has cryptographic witness receipt
5. **Defense in Depth**: Three independent signals must concur for permit
6. **Cryptographic Integrity**: All receipts signed with Ed25519
7. **Attack Resistance**: E-value bounds, replay guards, race condition prevention
8. **Distributed Scalability**: Hierarchical coordination with regional and global tiers
9. **Fault Tolerance**: Automatic failover with safe defaults

### Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Computational overhead | Lazy evaluation; batch updates; SIMD optimization |
| E-value power under uncertainty | Mixture e-values for robustness |
| Graph model mismatch | Learn graph structure from trajectories |
| Threshold tuning | Adaptive thresholds via meta-learning |
| Receipt forgery | Mandatory Ed25519 signing; chain linkage |
| E-value manipulation | Input bounds; clamping with security logging |
| Race conditions | Atomic decisions with sequence numbers |
| Replay attacks | Bloom filter + sliding window guard |
| Network partitions | Hierarchical decisions; local autonomy |
| Byzantine nodes | Consensus-based aggregation; safe defaults |

### Complexity Analysis

| Operation | Current | With AVCG | Distributed AVCG |
|-----------|---------|-----------|------------------|
| Edge update | O(n^{o(1)}) | O(n^{o(1)}) | O(n^{o(1)}) + network |
| Gate evaluation | O(1) | O(k) prediction set | O(k) + O(R) regional |
| Witness generation | O(m) | O(m) amortized | O(m) + signing |
| Certificate verification | O(n) | O(n + log T) | O(n + log T) + sig verify |
| Receipt signing | N/A | O(1) Ed25519 | O(1) + HSM latency |
| Distributed consensus | N/A | N/A | O(log N) Raft |
| E-process aggregation | N/A | O(1) | O(P) peers |

Where: k = prediction set size, T = history length, R = regional peers, N = cluster size, P = peer count

## References

### Dynamic Min-Cut
1. El-Hayek, Henzinger, Li. "Deterministic and Exact Fully-dynamic Minimum Cut of Superpolylogarithmic Size in Subpolynomial Time." arXiv:2512.13105, December 2025.
2. Jin, Sun, Thorup. "Fully Dynamic Exact Minimum Cut in Subpolynomial Time." SODA 2024.

### Online Conformal Prediction
3. "Online Conformal Inference with Retrospective Adjustment for Faster Adaptation to Distribution Shift." arXiv:2511.04275, November 2025.
4. "Distribution-informed Online Conformal Prediction (COP)." December 2025.
5. "CORE: Conformal Regression under Distribution Shift via Reinforcement Learning." October 2025.

### E-Values and E-Processes
6. Ramdas, Wang. "Hypothesis Testing with E-values." Foundations and Trends in Statistics, 2025.
7. ICML 2025 Tutorial: "Game-theoretic Statistics and Sequential Anytime-Valid Inference."
8. "Sequential Randomization Tests Using e-values." arXiv:2512.04366, December 2025.

### AI Agent Control
9. "Bounded Autonomy: A Pragmatic Response to Concerns About Fully Autonomous AI Agents." XMPRO, 2025.
10. "Customizable Runtime Enforcement for Safe and Reliable LLM Agents." arXiv:2503.18666, 2025.

## Testing Strategy

### Unit Tests

| Component | Coverage Target | Key Test Cases |
|-----------|----------------|----------------|
| `CompactGraph` | 95% | Add/remove edges, weight updates, min-cut estimation |
| `EvidenceAccumulator` | 95% | Bounds checking, update rules, stopping decisions |
| `TileReport` | 90% | Serialization roundtrip, checksum verification |
| `PermitToken` | 95% | Signing, verification, TTL expiration |
| `ReceiptLog` | 95% | Hash chain integrity, tamper detection |
| `ThreeFilterDecision` | 100% | All Permit/Defer/Deny paths |

### Integration Tests

| Scenario | Description | Expected Outcome |
|----------|-------------|------------------|
| Happy path | Stable graph, safe action | PERMIT with valid receipt |
| Boundary crossing | Action crosses fragile partition | DENY with boundary edges |
| Shift detection | New dependency pattern | DEFER with escalation |
| Human approval | DEFER → human approves | Token issued, learning recorded |
| Replay verification | Replay historical decision | Deterministic match |
| Hash chain audit | Verify 1000 receipts | All hashes valid |

### Property-Based Tests

```rust
#[proptest]
fn e_value_always_positive(e1: f64, e2: f64) {
    let result = combine_evalues(e1.abs(), e2.abs());
    prop_assert!(result > 0.0);
}

#[proptest]
fn receipt_hash_deterministic(receipt: WitnessReceipt) {
    let hash1 = receipt.compute_hash();
    let hash2 = receipt.compute_hash();
    prop_assert_eq!(hash1, hash2);
}

#[proptest]
fn serialization_roundtrip(report: TileReport) {
    let bytes = report.serialize();
    let restored = TileReport::deserialize(&bytes);
    prop_assert_eq!(report, restored);
}
```

### Security Tests

| Test | Attack Vector | Expected Behavior |
|------|---------------|-------------------|
| Forged signature | Invalid Ed25519 sig | Verification fails |
| Replay attack | Duplicate action | ReplayGuard blocks |
| E-value overflow | Extreme likelihood ratio | Clamped to bounds |
| Race condition | Concurrent evaluations | Sequence numbers ordered |
| Tampered receipt | Modified hash | Chain verification fails |

### Benchmark Tests

| Metric | Target | Measurement |
|--------|--------|-------------|
| Gate decision latency | p99 < 50ms | `criterion` benchmark |
| Receipt signing | < 5ms | `criterion` benchmark |
| 255-tile report merge | < 10ms | `criterion` benchmark |
| Hash chain verification (1000) | < 100ms | `criterion` benchmark |
| Memory per worker tile | < 64KB | Static analysis |

---

## Configuration Format

### TOML Configuration

```toml
# gate-config.toml

[gate]
# Gate identification
gate_id = "gate-west-01"
version = "0.1.0"

[thresholds]
# E-process thresholds
tau_deny = 0.01          # E-value below this → DENY
tau_permit = 100.0       # E-value above this → PERMIT

# Structural thresholds
min_cut = 5.0            # Cut value below this → DENY
max_shift = 0.5          # Shift pressure above this → DEFER

# Conformal thresholds
max_prediction_set = 20  # Set size above this → DEFER
coverage_target = 0.90   # Target coverage rate

[timing]
# Permit token TTL
permit_ttl_seconds = 300

# Decision timeout
decision_timeout_ms = 50

# Tick interval for worker tiles
tick_interval_ms = 10

[security]
# Key rotation
signing_key_rotation_days = 30
threshold_key_rotation_days = 90

# Replay prevention
replay_window_seconds = 3600
bloom_filter_size = 1000000

[distributed]
# Coordination settings
regional_peers = ["gate-west-02", "gate-west-03"]
global_coordinator = "coordinator-global-01"
raft_heartbeat_ms = 100
consensus_timeout_ms = 1000

[escalation]
# Human-in-loop settings
default_timeout_seconds = 300
default_on_timeout = "deny"

[escalation.channels.slack]
webhook_url = "${SLACK_WEBHOOK_URL}"
channel = "#gate-escalations"

[escalation.channels.pagerduty]
api_key = "${PAGERDUTY_API_KEY}"
service_id = "gate-critical"

[observability]
# Metrics endpoint
metrics_port = 9090
metrics_path = "/metrics"

# Tracing
tracing_enabled = true
tracing_sample_rate = 0.1
jaeger_endpoint = "http://jaeger:14268/api/traces"

[storage]
# Receipt storage
receipt_backend = "postgresql"
receipt_retention_days = 90
checkpoint_interval = 100

[storage.postgresql]
host = "${DB_HOST}"
port = 5432
database = "gate_receipts"
username = "${DB_USER}"
password = "${DB_PASSWORD}"
```

### Environment Variables

```bash
# Required
export GATE_SIGNING_KEY_PATH=/etc/gate/keys/signing.key
export GATE_CONFIG_PATH=/etc/gate/config.toml

# Optional overrides
export GATE_TAU_DENY=0.01
export GATE_TAU_PERMIT=100.0
export GATE_MIN_CUT=5.0
export GATE_MAX_SHIFT=0.5
export GATE_PERMIT_TTL_SECONDS=300

# Secrets (never in config file)
export SLACK_WEBHOOK_URL=https://hooks.slack.com/...
export PAGERDUTY_API_KEY=...
export DB_PASSWORD=...
```

---

## Error Recovery Procedures

### Gate Decision Failures

| Failure | Detection | Recovery | Fallback |
|---------|-----------|----------|----------|
| Min-cut timeout | Decision exceeds 50ms | Log, retry once | DEFER |
| E-process NaN | `is_nan()` check | Reset accumulator | DENY |
| Signing failure | Ed25519 error | Rotate to backup key | DENY (unsigned) |
| Receipt log full | Capacity check | Archive, start new segment | DENY |

### Distributed Failures

```rust
impl FaultRecovery {
    pub async fn handle_regional_failure(&mut self, error: RegionalError) -> GateResult {
        match error {
            RegionalError::LeaderUnavailable => {
                // Wait for new leader election
                tokio::time::sleep(Duration::from_millis(200)).await;
                self.retry_with_new_leader().await
            }

            RegionalError::NetworkPartition => {
                // Fall back to local-only decision
                log::warn!("Network partition detected, using local gate");
                self.local_gate.evaluate_standalone()
            }

            RegionalError::ConsensusTimeout => {
                // Use conservative decision
                Ok(GateResult {
                    decision: GateDecision::Defer,
                    reason: "Consensus timeout - escalating to human".into(),
                    ..Default::default()
                })
            }
        }
    }
}
```

### Receipt Chain Recovery

```rust
impl ReceiptLog {
    /// Recover from corrupted receipt chain
    pub fn recover_chain(&mut self, last_known_good: u64) -> Result<(), RecoveryError> {
        // 1. Truncate corrupted entries
        self.truncate_after(last_known_good)?;

        // 2. Rebuild from checkpoint
        let checkpoint = self.find_nearest_checkpoint(last_known_good)?;
        self.rebuild_from_checkpoint(checkpoint)?;

        // 3. Mark recovery in audit log
        self.append_recovery_marker(last_known_good)?;

        // 4. Alert operators
        alert::send("Receipt chain recovery performed", Severity::Warning);

        Ok(())
    }
}
```

### Worker Tile Recovery

| Failure | Detection | Recovery Time | Data Loss |
|---------|-----------|---------------|-----------|
| Single tile crash | Heartbeat timeout | < 100ms | Last tick |
| Tile memory corruption | Checksum mismatch | < 500ms | Current shard |
| TileZero crash | Primary unavailable | < 1s | None (standbys) |
| Full fabric restart | All tiles down | < 5s | Rebuild from checkpoint |

### Runbook: Gate Unresponsive

```bash
# 1. Check gate health
curl http://gate:9090/health

# 2. If unhealthy, check logs
kubectl logs -l app=gate --tail=100

# 3. Check for resource exhaustion
kubectl top pods -l app=gate

# 4. If memory high, trigger GC
curl -X POST http://gate:9090/admin/gc

# 5. If still unresponsive, rolling restart
kubectl rollout restart deployment/gate

# 6. Verify recovery
curl http://gate:9090/health
curl http://gate:9090/metrics | grep gate_healthy
```

---

## Appendix: Mathematical Foundations

### E-Value Composition

For independent e-values e₁, e₂:
```
e_combined = e₁ · e₂
E[e_combined] = E[e₁] · E[e₂] ≤ 1 · 1 = 1
```

This enables **optional continuation**: evidence accumulates validly across sessions.

### Conformal Coverage

Under exchangeability or bounded distribution shift:
```
P(Y_{t+1} ∈ C_t(X_{t+1})) ≥ 1 - α - δ_t
```

Where δ_t → 0 as the algorithm adapts via retrospective adjustment.

### Anytime-Valid Stopping

For any stopping time τ (possibly data-dependent):
```
P_H₀(E_τ ≥ 1/α) ≤ α
```

This holds because E_t is a nonnegative supermartingale with E[E_0] = 1.
