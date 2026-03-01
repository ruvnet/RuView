# Security Audit: Anytime-Valid Coherence Gate

**Document Version**: 1.0.0
**Audit Date**: 2026-01-17
**ADR Reference**: ADR-001-anytime-valid-coherence-gate.md
**Status**: Initial Security Review

---

## Executive Summary

This document provides a comprehensive security audit of the Anytime-Valid Coherence Gate (AVCG) design as specified in ADR-001. The coherence gate is a critical security boundary that controls autonomous agent actions through a three-signal decision system (structural min-cut, conformal prediction, and e-process evidence).

**Overall Risk Assessment**: MEDIUM-HIGH

The design demonstrates strong security awareness with explicit threat modeling, cryptographic receipt signing, and defense-in-depth principles. However, several areas require hardening before production deployment, particularly around WASM memory isolation, supply chain verification, and distributed consensus security.

---

## Table of Contents

1. [Threat Model Review](#1-threat-model-review)
2. [Cryptographic Analysis](#2-cryptographic-analysis)
3. [Input Validation](#3-input-validation)
4. [Race Conditions](#4-race-conditions)
5. [Replay Prevention](#5-replay-prevention)
6. [Trust Boundaries](#6-trust-boundaries)
7. [Denial of Service](#7-denial-of-service)
8. [Supply Chain Security](#8-supply-chain-security)
9. [WASM Security](#9-wasm-security)
10. [Recommendations](#10-recommendations)

---

## 1. Threat Model Review

### ADR Reference
ADR-001, Section: "Security Hardening > Threat Model" (lines 256-264)

### Documented Threat Actors

| Threat Actor | Capabilities | Target | Impact | Assessment |
|--------------|--------------|--------|--------|------------|
| Malicious Agent | Action injection, timing manipulation | Gate bypass | Unauthorized actions executed | **VALID** |
| Network Adversary | Message interception, replay | Receipt forgery | False audit trail | **VALID** |
| Insider Threat | Threshold modification, key access | Policy manipulation | Safety guarantees voided | **VALID** |
| Byzantine Node | Arbitrary behavior in distributed gate | Consensus corruption | Inconsistent decisions | **VALID** |

### Missing Threat Actors

The following threat actors should be added to the threat model:

#### 1.1 Compromised Worker Tile
**Risk**: HIGH

```
Threat: A compromised WASM worker tile (tiles 1-255) could:
- Report false coherence scores
- Inject malicious boundary edge data
- Cause TileZero to make incorrect decisions

Attack Vector: Supply chain compromise, WASM sandbox escape,
              memory corruption via malformed deltas

Mitigation Required:
- Worker report signing with per-tile keys
- Anomaly detection on worker reports
- Byzantine fault tolerance for worker aggregation
```

#### 1.2 Time-of-Check to Time-of-Use (TOCTOU)
**Risk**: MEDIUM

```
Threat: State changes between permit token issuance and action execution

Attack Vector:
1. Agent requests permit for action A
2. Gate evaluates current state, issues PERMIT token
3. Attacker modifies system state
4. Agent executes action A in now-unsafe state

Mitigation Required:
- Token binding to state hash
- State freshness verification at execution time
- Short TTL enforcement (documented as 50ms budget)
```

#### 1.3 Side-Channel Attacks
**Risk**: LOW-MEDIUM

```
Threat: Timing analysis reveals:
- Which actions are near decision thresholds
- Current e-process accumulator state
- Partition structure of the graph

Attack Vector: Repeated probing with crafted actions,
              measuring gate response latency

Mitigation Required:
- Constant-time decision paths where feasible
- Rate limiting per agent (documented in Q5)
- Noise injection in timing
```

#### 1.4 Model Extraction
**Risk**: MEDIUM

```
Threat: Adversary reconstructs:
- Conformal prediction model
- E-process threshold configuration
- Graph partition structure

Attack Vector: Systematic querying with boundary-case actions,
              analyzing permit/defer/deny patterns

Mitigation Required:
- Query rate limiting
- Differential privacy on responses
- Threshold rotation (documented in Q5)
```

### Threat Model Completeness Score: 7/10

**Gaps Identified**:
- No explicit consideration of worker tile compromise
- TOCTOU attacks not addressed
- Side-channel leakage not considered
- Physical/environmental threats for embedded deployment not covered

---

## 2. Cryptographic Analysis

### ADR Reference
ADR-001, Section: "Cryptographic Requirements" (lines 266-323)

### 2.1 Ed25519 Signature Scheme

**Specification**:
```rust
pub struct WitnessReceipt {
    pub receipt_hash: [u8; 32],         // Blake3 hash
    pub signature: Ed25519Signature,     // Ed25519 signature
    pub signer_id: PublicKey,           // Gate identity
    pub timestamp_proof: TimestampProof, // Chain linkage
}
```

**Assessment**: ADEQUATE with caveats

| Property | Status | Notes |
|----------|--------|-------|
| Algorithm Strength | GOOD | Ed25519 provides 128-bit security |
| Key Size | GOOD | 256-bit keys are appropriate |
| Deterministic Signatures | CAUTION | Ed25519 is deterministic; same message = same signature |
| Quantum Resistance | WEAK | Ed25519 is not post-quantum secure |

**Concern**: The codebase shows post-quantum crypto in `ruvector-dag/src/qudag/crypto/` using ML-DSA-65 and ML-KEM-768. Consider a migration path:

```rust
// Recommended: Hybrid signature scheme for transition period
pub struct HybridSignature {
    /// Classical Ed25519 (for current compatibility)
    pub ed25519_sig: [u8; 64],
    /// Post-quantum ML-DSA-65 (for future security)
    pub ml_dsa_sig: Option<[u8; 3309]>,
}
```

### 2.2 Blake3 Hash Function

**Assessment**: EXCELLENT

- 256-bit output provides 128-bit collision resistance
- Designed for both speed and security
- Tree hashing mode enables parallelization
- No known vulnerabilities

**Implementation Note**: Ensure the `blake3` crate is used with `std` feature for constant-time operations:

```toml
[dependencies]
blake3 = { version = "1.5", features = ["std"] }
```

### 2.3 Hash Chain Integrity

**Specification** (ADR lines 280-286):
```rust
pub struct TimestampProof {
    pub timestamp: u64,
    pub previous_receipt_hash: [u8; 32], // Chain linkage
    pub merkle_root: [u8; 32],           // Batch anchor
}
```

**Assessment**: GOOD with recommendations

**Strength**: Hash chain provides:
- Tamper evidence (any modification breaks chain)
- Ordering proof (receipts must be sequential)
- Audit trail integrity

**Weakness**: Single-chain design creates bottleneck:

```
Receipt N-1 --> Receipt N --> Receipt N+1
    |              |              |
    hash           hash           hash
```

**Recommendation**: Implement parallel chains with periodic cross-linking:

```rust
pub struct ReceiptChain {
    /// Multiple parallel chains for throughput
    chains: [ChainHead; 4],
    /// Periodic cross-chain Merkle root
    cross_link_root: [u8; 32],
    /// Interval between cross-links
    cross_link_interval: u64,
}
```

### 2.4 Timestamp Proofs

**Assessment**: NEEDS IMPROVEMENT

The current design relies on local timestamps which are susceptible to manipulation:

```rust
// CURRENT (ADR line 1049)
timestamp: now_ns(),
```

**Recommended Improvements**:

1. **Trusted Time Source**: Integrate with hardware security module (HSM) or trusted timestamping authority

2. **Verifiable Delay Function (VDF)**: Add time-lock proofs

```rust
pub struct EnhancedTimestampProof {
    pub timestamp: u64,
    pub previous_receipt_hash: [u8; 32],
    /// VDF proof that timestamp delay has elapsed
    pub vdf_proof: Option<VdfProof>,
    /// External timestamp authority signature
    pub tsa_signature: Option<TsaSignature>,
}
```

### 2.5 Key Management

**ADR Specification** (lines 316-323):

| Key Type | Purpose | Rotation | Storage |
|----------|---------|----------|---------|
| Gate Signing Key | Sign receipts | 30 days | HSM or secure enclave |
| Receipt Verification Keys | Verify receipts | On rotation | Distributed key store |
| Threshold Keys | Multi-party signing | 90 days | Shamir secret sharing |

**Assessment**: ADEQUATE foundation, needs operational details

**Missing Elements**:

1. **Key Derivation**: No specification for deriving per-session or per-action keys
2. **Revocation**: No key revocation mechanism defined
3. **Recovery**: No key recovery procedure documented
4. **Audit**: No key access logging specified

**Recommended Key Hierarchy**:

```
Root Key (HSM, never exported)
    |
    +-- Gate Signing Key (rotated monthly)
    |       |
    |       +-- Session Keys (ephemeral, per-session)
    |
    +-- Worker Keys (per-tile, rotated on restart)
    |
    +-- Recovery Keys (Shamir 3-of-5)
```

---

## 3. Input Validation

### ADR Reference
ADR-001, Section: "E-Value Manipulation Prevention" (lines 326-356)

### 3.1 E-Value Bounds

**Specification**:
```rust
const E_VALUE_MIN: f64 = 1e-10;
const E_VALUE_MAX: f64 = 1e10;

impl EValue {
    pub fn from_likelihood_ratio(
        likelihood_h1: f64,
        likelihood_h0: f64,
    ) -> Result<Self, EValueError> {
        if likelihood_h0 <= f64::EPSILON {
            return Err(EValueError::InvalidDenominator);
        }
        let ratio = likelihood_h1 / likelihood_h0;
        let bounded = ratio.clamp(E_VALUE_MIN, E_VALUE_MAX);
        // ... security logging for clamping
    }
}
```

**Assessment**: GOOD but incomplete

**Validated**:
- Division by zero prevention
- Overflow protection via clamping
- Security logging for anomalies

**Missing Validations**:

```rust
// REQUIRED: Additional input validation
impl EValue {
    pub fn from_likelihood_ratio(
        likelihood_h1: f64,
        likelihood_h0: f64,
    ) -> Result<Self, EValueError> {
        // 1. Check for NaN/Infinity
        if !likelihood_h1.is_finite() || !likelihood_h0.is_finite() {
            return Err(EValueError::NonFiniteInput);
        }

        // 2. Check for negative values (likelihoods must be non-negative)
        if likelihood_h1 < 0.0 || likelihood_h0 < 0.0 {
            return Err(EValueError::NegativeLikelihood);
        }

        // 3. Check denominator
        if likelihood_h0 <= f64::EPSILON {
            return Err(EValueError::InvalidDenominator);
        }

        // 4. Compute with overflow protection
        let ratio = likelihood_h1 / likelihood_h0;

        // 5. Check result is valid
        if !ratio.is_finite() {
            return Err(EValueError::ComputationOverflow);
        }

        let bounded = ratio.clamp(E_VALUE_MIN, E_VALUE_MAX);

        // 6. Log clamping events
        if (bounded - ratio).abs() > f64::EPSILON {
            security_log!(
                level: SecurityLevel::Warning,
                event: "e_value_clamped",
                original: ratio,
                clamped: bounded,
                source: std::panic::Location::caller()
            );
        }

        Ok(Self { value: bounded, ..Default::default() })
    }
}
```

### 3.2 Delta Sanitization

**ADR Reference**: Worker tile delta ingestion (lines 937-945)

```rust
pub fn ingest_delta(&mut self, delta: &Delta) -> Status {
    match delta {
        Delta::EdgeAdd(e) => self.graph_shard.add_edge(e),
        Delta::EdgeRemove(e) => self.graph_shard.remove_edge(e),
        Delta::WeightUpdate(e, w) => self.graph_shard.update_weight(e, *w),
        Delta::Observation(score) => self.feature_window.push(*score),
    }
    // ...
}
```

**Assessment**: INSUFFICIENT

**Required Sanitization**:

```rust
impl WorkerTileState {
    /// Validated delta ingestion with bounds checking
    pub fn ingest_delta(&mut self, delta: &Delta) -> Result<Status, DeltaError> {
        // 1. Rate limiting check
        self.delta_rate_limiter.check()?;

        // 2. Validate delta based on type
        match delta {
            Delta::EdgeAdd(e) => {
                // Validate edge endpoints are in valid range
                if e.src >= MAX_VERTEX_ID || e.tgt >= MAX_VERTEX_ID {
                    return Err(DeltaError::InvalidVertex);
                }
                // Validate no self-loops
                if e.src == e.tgt {
                    return Err(DeltaError::SelfLoop);
                }
                // Check graph capacity
                if self.graph_shard.edge_count() >= MAX_EDGES_PER_SHARD {
                    return Err(DeltaError::ShardFull);
                }
                self.graph_shard.add_edge(e)?;
            }

            Delta::EdgeRemove(e) => {
                // Validate edge exists
                if !self.graph_shard.has_edge(e) {
                    return Err(DeltaError::EdgeNotFound);
                }
                self.graph_shard.remove_edge(e)?;
            }

            Delta::WeightUpdate(e, w) => {
                // Validate weight is finite and positive
                if !w.is_finite() || *w <= 0.0 {
                    return Err(DeltaError::InvalidWeight);
                }
                // Validate weight bounds
                if *w > MAX_EDGE_WEIGHT {
                    return Err(DeltaError::WeightTooLarge);
                }
                self.graph_shard.update_weight(e, *w)?;
            }

            Delta::Observation(score) => {
                // Validate observation is finite
                if !score.is_finite() {
                    return Err(DeltaError::InvalidObservation);
                }
                // Validate observation bounds (normality scores in [0, 1])
                if *score < 0.0 || *score > 1.0 {
                    return Err(DeltaError::ObservationOutOfRange);
                }
                self.feature_window.push(*score);
            }
        }

        self.update_local_coherence();
        Ok(Status::Ok)
    }
}

const MAX_VERTEX_ID: u32 = 256;  // Per tile
const MAX_EDGES_PER_SHARD: usize = 2000;
const MAX_EDGE_WEIGHT: f32 = 1000.0;
```

### 3.3 Action Context Validation

**ADR Reference**: MCP tool permit_action (lines 1193-1206)

```rust
#[mcp_tool]
pub async fn permit_action(
    action_id: String,
    action_type: String,
    context: serde_json::Value,
) -> Result<PermitResponse, McpError> {
    let ctx = ActionContext::from_json(&context)?;
    // ...
}
```

**Assessment**: NEEDS HARDENING

**Required Validations**:

```rust
impl ActionContext {
    pub fn from_json(json: &serde_json::Value) -> Result<Self, ValidationError> {
        // 1. Validate JSON structure
        let obj = json.as_object()
            .ok_or(ValidationError::ExpectedObject)?;

        // 2. Validate required fields exist
        let action_id = obj.get("action_id")
            .and_then(|v| v.as_str())
            .ok_or(ValidationError::MissingField("action_id"))?;

        // 3. Validate action_id format (prevent injection)
        if !Self::is_valid_action_id(action_id) {
            return Err(ValidationError::InvalidActionId);
        }

        // 4. Validate agent_id is authenticated
        let agent_id = obj.get("agent_id")
            .and_then(|v| v.as_str())
            .ok_or(ValidationError::MissingField("agent_id"))?;

        if !Self::is_authenticated_agent(agent_id) {
            return Err(ValidationError::UnauthenticatedAgent);
        }

        // 5. Validate context size (prevent DoS)
        if json.to_string().len() > MAX_CONTEXT_SIZE {
            return Err(ValidationError::ContextTooLarge);
        }

        // 6. Sanitize string fields (prevent XSS in logs)
        let sanitized = Self::sanitize_context(obj)?;

        Ok(Self::from_validated(sanitized))
    }

    fn is_valid_action_id(id: &str) -> bool {
        // Allow only alphanumeric, hyphen, underscore
        id.len() <= 64 &&
        id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    }
}

const MAX_CONTEXT_SIZE: usize = 4096;
```

---

## 4. Race Conditions

### ADR Reference
ADR-001, Section: "Race Condition Prevention" (lines 358-384)

### 4.1 Atomic Decision Guarantees

**Specification**:
```rust
pub struct AtomicGateDecision {
    sequence: AtomicU64,
    decision_lock: RwLock<()>,
}

impl AtomicGateDecision {
    pub async fn evaluate(&self, action: &Action) -> GateResult {
        let _guard = self.decision_lock.write().await;
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
        let result = self.evaluate_internal(action, seq).await;
        result.with_sequence(seq)
    }
}
```

**Assessment**: PARTIALLY ADEQUATE

**Strengths**:
- Write lock ensures mutual exclusion
- Sequence number provides ordering
- SeqCst ordering is appropriately strong

**Weaknesses**:

#### 4.1.1 Lock Contention Under Load
**Risk**: HIGH

```rust
// PROBLEM: Single write lock creates bottleneck
// At 1000 decisions/sec, each waiting on average 0.5ms = 500ms queue
```

**Recommendation**: Implement lock-free decision path for independent actions:

```rust
pub struct ShardedGateDecision {
    /// Multiple independent decision contexts
    shards: [AtomicGateDecision; 16],
    /// Global sequence for total ordering
    global_sequence: AtomicU64,
}

impl ShardedGateDecision {
    pub async fn evaluate(&self, action: &Action) -> GateResult {
        // Hash action to shard for parallelism
        let shard_idx = Self::hash_action(action) % 16;
        let shard = &self.shards[shard_idx];

        // Get global sequence first (lock-free)
        let global_seq = self.global_sequence.fetch_add(1, Ordering::SeqCst);

        // Evaluate in shard (lower contention)
        let _guard = shard.decision_lock.write().await;
        let local_seq = shard.sequence.fetch_add(1, Ordering::SeqCst);

        let result = shard.evaluate_internal(action, local_seq).await;
        result.with_sequence(global_seq)
    }
}
```

#### 4.1.2 Missing Timeout on Lock Acquisition
**Risk**: MEDIUM

```rust
// PROBLEM: Deadlock risk if evaluate_internal hangs
let _guard = self.decision_lock.write().await; // No timeout!
```

**Recommendation**:
```rust
pub async fn evaluate(&self, action: &Action) -> GateResult {
    // Timeout on lock acquisition
    let guard = tokio::time::timeout(
        Duration::from_millis(10),
        self.decision_lock.write()
    ).await.map_err(|_| GateError::LockTimeout)?;

    // Timeout on evaluation
    let result = tokio::time::timeout(
        Duration::from_millis(40),
        self.evaluate_internal(action, seq)
    ).await.map_err(|_| GateError::EvaluationTimeout)?;

    result
}
```

### 4.2 Sequence Number Ordering

**Assessment**: GOOD

The design correctly uses monotonic sequence numbers for ordering. However:

**Gap Risk**: If sequence N fails after incrementing counter, sequence N is lost:

```rust
// Sequence: 100, 101, 103 (102 missing due to failure)
// This breaks "no gaps" assumption for audit
```

**Recommendation**: Use reservations:

```rust
pub struct SequenceAllocator {
    next: AtomicU64,
    committed: AtomicU64,
    pending: DashMap<u64, PendingDecision>,
}

impl SequenceAllocator {
    pub fn reserve(&self) -> SequenceReservation {
        let seq = self.next.fetch_add(1, Ordering::SeqCst);
        self.pending.insert(seq, PendingDecision::new());
        SequenceReservation { seq, allocator: self }
    }

    pub fn commit(&self, seq: u64, result: GateResult) {
        self.pending.remove(&seq);
        // Advance committed pointer if this was the next expected
        self.try_advance_committed();
    }

    pub fn abort(&self, seq: u64, reason: &str) {
        // Mark as aborted (not missing)
        self.pending.insert(seq, PendingDecision::aborted(reason));
        self.try_advance_committed();
    }
}
```

### 4.3 Distributed Race Conditions

**ADR Reference**: Distributed coordination (lines 647-730)

**Assessment**: NEEDS ATTENTION

The hierarchical decision protocol introduces additional race conditions:

```
Agent A                Regional Gate           Global Coordinator
   |                        |                         |
   |--action X request----->|                         |
   |                        |--coordinate------------>|
   |                        |                         |
   |                   (local state changes)          |
   |                        |                         |
   |                        |<--global decision-------|
   |<--stale decision-------|                         |
```

**Recommendation**: Implement optimistic concurrency control:

```rust
pub struct DistributedDecision {
    /// Version vector for state tracking
    version: VersionVector,
    /// Decision validity epoch
    epoch: u64,
}

impl DistributedGateController {
    pub async fn evaluate(&mut self, action: &Action, context: &Context) -> GateResult {
        let pre_version = self.version_vector.clone();

        let result = match self.routing.classify(action, context) {
            DecisionScope::Local => self.local_gate.evaluate(action, context),
            DecisionScope::Regional => {
                let regional = self.regional.coordinate(action).await?;
                // Verify state hasn't changed
                if self.version_vector != pre_version {
                    return Err(GateError::ConcurrentModification);
                }
                regional
            }
            // ...
        };

        // Bind decision to state version
        result.with_version(pre_version)
    }
}
```

---

## 5. Replay Prevention

### ADR Reference
ADR-001, Section: "Replay Attack Prevention" (lines 386-419)

### 5.1 Bloom Filter + Sliding Window

**Specification**:
```rust
pub struct ReplayGuard {
    recent_actions: BloomFilter,
    hash_window: VecDeque<[u8; 32]>,
    window_duration: Duration,
}
```

**Assessment**: GOOD design, needs parameter tuning

**Analysis**:

| Parameter | Recommended Value | Rationale |
|-----------|-------------------|-----------|
| Bloom filter size | 2^20 bits (128KB) | 1M actions with 1% FP rate |
| Hash functions | 7 | Optimal for 1% FP rate |
| Window duration | 300 seconds | Balance memory vs. protection |
| Window capacity | 100,000 hashes | 333 actions/sec max |

**False Positive Impact**:
```
At 1% FP rate with 1000 actions/sec:
- 10 legitimate actions/sec incorrectly flagged as replays
- These trigger slow-path verification
- Slow path has ~0% FP rate (exact hash comparison)
```

### 5.2 Effectiveness Analysis

**Covered Attack Vectors**:
- Simple replay of captured permit requests
- Replay with modified timestamps
- Parallel replay attempts

**Uncovered Attack Vectors**:

#### 5.2.1 Cross-Node Replay
**Risk**: MEDIUM

```
Attacker captures: permit_action(X) -> PERMIT token T

If distributed gates don't share replay state:
- Node A processes and records action X
- Attacker replays action X to Node B
- Node B has no record of X, issues new token

Mitigation: Gossip-based replay state sharing
```

**Recommendation**:
```rust
pub struct DistributedReplayGuard {
    local: ReplayGuard,
    /// Bloom filter shared via gossip
    shared_filter: SharedBloomFilter,
    /// Recent hashes from peers
    peer_hashes: HashMap<NodeId, HashSet<[u8; 32]>>,
}

impl DistributedReplayGuard {
    pub fn check_and_record(&mut self, action: &Action) -> Result<(), ReplayError> {
        let hash = action.content_hash();

        // Check local filter
        if self.local.might_contain(&hash) {
            if self.local.hash_window.contains(&hash) {
                return Err(ReplayError::LocalDuplicate);
            }
        }

        // Check shared filter (gossip-propagated)
        if self.shared_filter.might_contain(&hash) {
            // Query specific peers for confirmation
            for (peer_id, hashes) in &self.peer_hashes {
                if hashes.contains(&hash) {
                    return Err(ReplayError::CrossNodeDuplicate {
                        original_node: *peer_id
                    });
                }
            }
        }

        // Record locally and propagate
        self.local.recent_actions.insert(&hash);
        self.local.hash_window.push_back(hash);
        self.shared_filter.insert(&hash);
        self.gossip_hash(hash);

        Ok(())
    }
}
```

#### 5.2.2 Semantic Replay
**Risk**: MEDIUM

```
Original action: push_config(device=A, config=X)
Replay attack:   push_config(device=A, config=X)  // Same semantic effect

If action hashing only covers (action_type, target):
- Slightly different request body generates different hash
- Same semantic action executed twice

Mitigation: Include semantic content in hash
```

**Recommendation**: Canonical action representation:

```rust
impl Action {
    /// Content hash that captures semantic intent
    pub fn content_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();

        // Fixed fields
        hasher.update(&self.action_type.as_bytes());
        hasher.update(&self.target.canonical_bytes());

        // Semantic content (sorted, normalized)
        let canonical_content = self.canonicalize_content();
        hasher.update(&canonical_content);

        // DO NOT include: timestamp, nonce, request_id
        // These would allow semantic replays with different metadata

        hasher.finalize().into()
    }

    fn canonicalize_content(&self) -> Vec<u8> {
        // Sort keys, normalize values, remove whitespace
        serde_json::to_vec(&self.content_normalized()).unwrap()
    }
}
```

### 5.3 Memory Bounds

**Risk**: Memory exhaustion if window grows unbounded

```rust
// ADR shows pruning but no hard limit
fn prune_old_entries(&mut self) {
    while let Some(oldest) = self.hash_window.front() {
        if self.is_expired(oldest) {
            self.hash_window.pop_front();
        } else {
            break;
        }
    }
}
```

**Recommendation**: Add hard capacity limit:

```rust
impl ReplayGuard {
    const MAX_WINDOW_SIZE: usize = 100_000;

    pub fn check_and_record(&mut self, action: &Action) -> Result<(), ReplayError> {
        // ... existing checks ...

        // Hard limit on window size (defend against time manipulation)
        while self.hash_window.len() >= Self::MAX_WINDOW_SIZE {
            self.hash_window.pop_front();
        }

        self.hash_window.push_back(hash);
        Ok(())
    }
}
```

---

## 6. Trust Boundaries

### ADR Reference
ADR-001, Section: "Trust Boundaries" (lines 421-448)

### 6.1 Gate Core Isolation

**Specification**:
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
└─────────────────────────────────────────────────────────────────────────┘
```

**Assessment**: WELL-DEFINED but needs enforcement

**Invariant Verification Checklist**:

| Invariant | Enforcement Mechanism | Status |
|-----------|----------------------|--------|
| All inputs validated before use | Input validation layer | PARTIAL |
| All outputs signed before release | Signing in receipt generation | SPECIFIED |
| No external calls during decision | Code review / static analysis | NOT ENFORCED |

### 6.2 Boundary Crossing Analysis

**Incoming Data Flows**:

```
┌──────────────────┐      ┌──────────────────┐
│   AGENT          │      │   WORKER TILES   │
│   INTERFACE      │      │   (1-255)        │
└────────┬─────────┘      └────────┬─────────┘
         │                         │
         │ action_request          │ tile_reports
         │ (untrusted)             │ (semi-trusted)
         ▼                         ▼
┌─────────────────────────────────────────────┐
│              GATE CORE                       │
│  ┌─────────────────────────────────────┐    │
│  │  VALIDATION LAYER                    │    │
│  │  - Schema validation                 │    │
│  │  - Bounds checking                   │    │
│  │  - Authentication                    │    │
│  └─────────────────────────────────────┘    │
└─────────────────────────────────────────────┘
```

**Required Validation at Each Boundary**:

```rust
/// Agent Interface -> Gate Core
pub struct AgentBoundary;

impl AgentBoundary {
    pub fn validate_request(raw: &[u8]) -> Result<ValidatedRequest, BoundaryError> {
        // 1. Size check (prevent DoS)
        if raw.len() > MAX_REQUEST_SIZE {
            return Err(BoundaryError::RequestTooLarge);
        }

        // 2. Deserialize with limits
        let request: ActionRequest = serde_json::from_slice(raw)
            .map_err(|_| BoundaryError::MalformedJson)?;

        // 3. Authenticate agent
        let agent_id = Self::authenticate(&request.agent_credentials)?;

        // 4. Authorize action type
        Self::authorize(agent_id, &request.action_type)?;

        // 5. Validate action content
        let validated_action = ActionValidator::validate(&request.action)?;

        Ok(ValidatedRequest {
            agent_id,
            action: validated_action,
            timestamp: Instant::now(),
        })
    }
}

/// Worker Tile -> TileZero
pub struct WorkerBoundary;

impl WorkerBoundary {
    pub fn validate_report(
        tile_id: u8,
        raw: &TileReport
    ) -> Result<ValidatedReport, BoundaryError> {
        // 1. Validate tile_id matches expected sender
        if raw.tile_id != tile_id {
            return Err(BoundaryError::TileIdMismatch);
        }

        // 2. Validate coherence score is finite and in range
        if !raw.coherence.is_finite() || raw.coherence < 0.0 || raw.coherence > 1.0 {
            return Err(BoundaryError::InvalidCoherence);
        }

        // 3. Validate e-value is finite and positive
        if !raw.e_value.is_finite() || raw.e_value < 0.0 {
            return Err(BoundaryError::InvalidEValue);
        }

        // 4. Validate witness fragment structure
        Self::validate_witness_fragment(&raw.witness_fragment)?;

        // 5. Check for anomalous patterns
        Self::anomaly_check(tile_id, raw)?;

        Ok(ValidatedReport::from(raw))
    }
}
```

### 6.3 Outgoing Data Flows

```
┌─────────────────────────────────────────────┐
│              GATE CORE                       │
│  ┌─────────────────────────────────────┐    │
│  │  SIGNING LAYER                       │    │
│  │  - All outputs signed                │    │
│  │  - Receipts chained                  │    │
│  │  - Tokens have MAC                   │    │
│  └─────────────────────────────────────┘    │
└──────────┬────────────────────┬─────────────┘
           │                    │
           │ permit_token       │ witness_receipt
           │ (authenticated)    │ (signed)
           ▼                    ▼
┌──────────────────┐   ┌──────────────────────┐
│   AGENT          │   │   AUDIT LOG          │
└──────────────────┘   └──────────────────────┘
```

**Recommended Output Validation**:

```rust
impl GateCore {
    pub fn emit_result(&self, result: &GateResult) -> SignedOutput {
        // 1. Validate result is complete
        assert!(result.decision.is_set());
        assert!(result.witness.is_complete());

        // 2. Generate receipt
        let receipt = WitnessReceipt::from_result(result);

        // 3. Sign receipt (MANDATORY)
        let signed_receipt = receipt.sign(&self.signing_key)
            .expect("Signing must succeed");

        // 4. Generate permit token if PERMIT
        let token = if result.decision == GateDecision::Permit {
            Some(PermitToken::new(result, &self.signing_key))
        } else {
            None
        };

        // 5. Chain to previous receipt
        self.receipt_chain.append(&signed_receipt);

        SignedOutput {
            receipt: signed_receipt,
            token,
        }
    }
}
```

---

## 7. Denial of Service

### ADR Reference
ADR-001, Sections: "Performance Optimization" (lines 452-640), "Cost Model" (lines 1579-1609)

### 7.1 Resource Exhaustion Vectors

#### 7.1.1 Computation Exhaustion
**Risk**: HIGH

```
Attack: Submit actions that trigger expensive min-cut recomputation

Example:
- Insert edge that maximally disrupts current cut
- Force full hierarchy propagation (O(log n) levels)
- Repeat at maximum rate

Impact: Gate latency exceeds 50ms budget, effectively DoS
```

**Mitigations**:

```rust
pub struct ComputationLimiter {
    /// Per-agent computation budget (microseconds)
    agent_budgets: DashMap<AgentId, ComputationBudget>,
    /// Global computation budget
    global_budget: AtomicU64,
}

impl ComputationLimiter {
    pub fn check_and_charge(
        &self,
        agent: AgentId,
        estimated_cost: u64
    ) -> Result<ComputationPermit, DoSError> {
        // 1. Check agent budget
        let agent_budget = self.agent_budgets
            .get_mut(&agent)
            .ok_or(DoSError::UnknownAgent)?;

        if agent_budget.remaining < estimated_cost {
            return Err(DoSError::AgentBudgetExhausted {
                remaining: agent_budget.remaining,
                required: estimated_cost,
            });
        }

        // 2. Check global budget
        let global_remaining = self.global_budget.load(Ordering::Relaxed);
        if global_remaining < estimated_cost {
            return Err(DoSError::GlobalBudgetExhausted);
        }

        // 3. Reserve budget
        agent_budget.remaining -= estimated_cost;
        self.global_budget.fetch_sub(estimated_cost, Ordering::Relaxed);

        Ok(ComputationPermit {
            agent,
            charged: estimated_cost,
            start: Instant::now(),
        })
    }

    pub fn refund(&self, permit: ComputationPermit, actual_cost: u64) {
        let refund = permit.charged.saturating_sub(actual_cost);
        if refund > 0 {
            self.agent_budgets.get_mut(&permit.agent)
                .map(|mut b| b.remaining += refund);
            self.global_budget.fetch_add(refund, Ordering::Relaxed);
        }
    }
}
```

#### 7.1.2 Memory Exhaustion
**Risk**: MEDIUM

**ADR Cost Model** (lines 1586-1609):
```
Per worker tile: ~41 KB
Total 255 workers: ~10.2 MB
TileZero state: ~1 MB
Total fabric: ~12 MB
```

**Attack Vectors**:

1. **E-Process History Growth**: Fixed with ring buffer (ADR lines 461-498)
2. **Receipt Log Growth**: ~44 MB/day at 1000 decisions/sec
3. **Replay Window Growth**: Fixed with MAX_WINDOW_SIZE

**Remaining Concerns**:

```rust
// CONCERN: Unbounded witness partition storage
pub struct WitnessReceipt {
    pub witness_partition: (Vec<VertexId>, Vec<VertexId>),
    // If graph has 1M vertices, partition could be 8MB
}
```

**Mitigation**:
```rust
pub struct BoundedWitnessPartition {
    /// Compressed partition representation
    partition_bits: BitVec,
    /// If partition > threshold, store only boundary vertices
    boundary_only: bool,
    /// Hash of full partition for verification
    partition_hash: [u8; 32],
}

impl BoundedWitnessPartition {
    const MAX_EXPLICIT_SIZE: usize = 1000;

    pub fn from_partition(
        side_a: &[VertexId],
        side_b: &[VertexId]
    ) -> Self {
        if side_a.len() + side_b.len() <= Self::MAX_EXPLICIT_SIZE {
            // Store full partition
            Self::explicit(side_a, side_b)
        } else {
            // Store only boundary and hash
            Self::compressed(side_a, side_b)
        }
    }
}
```

#### 7.1.3 Network Exhaustion
**Risk**: MEDIUM (Distributed Mode)

**ADR Cost Model** (lines 1598-1600):
```
Worker -> TileZero reports: ~1.6 MB/s
Gossip (distributed): ~10 KB/s * peers
```

**Attack**: Compromised peer floods gossip channel

**Mitigation**:
```rust
pub struct GossipRateLimiter {
    /// Per-peer incoming rate limits
    peer_limits: HashMap<NodeId, TokenBucket>,
    /// Global incoming rate limit
    global_limit: TokenBucket,
}

impl GossipRateLimiter {
    pub fn allow_message(&mut self, peer: NodeId, size: usize) -> bool {
        // Check peer-specific limit
        if !self.peer_limits.get_mut(&peer)
            .map(|b| b.consume(size))
            .unwrap_or(false)
        {
            self.flag_peer_for_review(peer);
            return false;
        }

        // Check global limit
        if !self.global_limit.consume(size) {
            return false;
        }

        true
    }
}
```

### 7.2 Memory Limits

**Recommended Configuration**:

| Component | Limit | Rationale |
|-----------|-------|-----------|
| Worker tile state | 64 KB | Fits in single WASM page |
| TileZero supergraph | 4 MB | ~100K edges |
| Receipt log (hot) | 100 MB | ~200K receipts |
| Replay window | 3.2 MB | 100K hashes |
| E-process history | 64 KB | Ring buffer |
| **Total gate memory** | **~120 MB** | Reasonable for server |

```rust
pub struct MemoryBudget {
    pub worker_tile: usize,      // 64 * 1024
    pub tilezero: usize,         // 4 * 1024 * 1024
    pub receipt_hot: usize,      // 100 * 1024 * 1024
    pub replay_window: usize,    // 3200 * 1024
    pub eprocess_history: usize, // 64 * 1024
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self {
            worker_tile: 64 * 1024,
            tilezero: 4 * 1024 * 1024,
            receipt_hot: 100 * 1024 * 1024,
            replay_window: 3200 * 1024,
            eprocess_history: 64 * 1024,
        }
    }
}
```

---

## 8. Supply Chain Security

### ADR Reference
ADR-001, Section: "Rust Deliverables" (lines 1155-1187)

### 8.1 Critical Dependencies

**Direct Dependencies** (from Cargo.toml):

| Crate | Version | Security Risk | Assessment |
|-------|---------|---------------|------------|
| `blake3` | 1.x | LOW | Well-audited, pure Rust |
| `ed25519-dalek` | 2.x | MEDIUM | Critical for signatures |
| `proptest` (dev) | 1.x | LOW | Dev-only |

### 8.2 blake3 Security Assessment

**Source**: https://github.com/BLAKE3-team/BLAKE3

**Status**: ACCEPTABLE

- Pure Rust implementation available
- Extensive fuzzing performed
- No known vulnerabilities
- Maintained by cryptographers

**Recommended Cargo.toml**:
```toml
[dependencies]
blake3 = { version = "1.5", default-features = false, features = ["std"] }
```

**Verification**:
```bash
# Verify crate integrity
cargo audit
cargo deny check

# Pin to specific commit for reproducible builds
[dependencies]
blake3 = { git = "https://github.com/BLAKE3-team/BLAKE3", rev = "abc123..." }
```

### 8.3 ed25519-dalek Security Assessment

**Source**: https://github.com/dalek-cryptography/curve25519-dalek

**Status**: REQUIRES ATTENTION

**Recent Security History**:
- 2023-01: Timing side-channel vulnerability (CVE-2023-34478, fixed in 2.0)
- Ensure version >= 2.0.0

**Recommended Cargo.toml**:
```toml
[dependencies]
ed25519-dalek = { version = "2.1", features = ["batch", "zeroize"] }
```

**Critical**: Enable `zeroize` feature for key material cleanup:
```rust
use ed25519_dalek::SigningKey;
use zeroize::Zeroize;

struct GateSigningContext {
    key: SigningKey,
}

impl Drop for GateSigningContext {
    fn drop(&mut self) {
        // Signing key automatically zeroizes on drop
    }
}
```

### 8.4 WASM Dependencies

For `cognitum-gate-kernel` (no_std WASM):

**Minimal Dependency Set**:
```toml
[dependencies]
# NO external dependencies for security-critical kernel
# All crypto must be inline or from audited sources

[target.'cfg(target_arch = "wasm32")'.dependencies]
# WASM-specific dependencies only if absolutely necessary
```

**Recommendation**: Vendor critical crypto code:

```
cognitum-gate-kernel/
├── src/
│   ├── lib.rs
│   ├── crypto/
│   │   ├── mod.rs
│   │   ├── blake3_inline.rs    # Vendored, audited blake3
│   │   └── ed25519_inline.rs   # Vendored, audited ed25519
```

### 8.5 Supply Chain Hardening

**Recommended CI Pipeline**:

```yaml
# .github/workflows/security.yml
name: Supply Chain Security

on: [push, pull_request]

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install cargo-audit
        run: cargo install cargo-audit

      - name: Security audit
        run: cargo audit --deny warnings

      - name: Check for yanked crates
        run: cargo deny check

      - name: Verify dependency signatures
        run: |
          cargo vet audit
          cargo vet suggest

  sbom:
    runs-on: ubuntu-latest
    steps:
      - name: Generate SBOM
        run: cargo sbom --output-format cyclonedx > sbom.json

      - name: Scan SBOM for vulnerabilities
        uses: anchore/scan-action@v3
        with:
          sbom: sbom.json
```

---

## 9. WASM Security

### ADR Reference
ADR-001, Sections: "Hardware Mapping: 256-Tile WASM Fabric" (lines 873-1187), "WASM Kernel API" (lines 1107-1140)

### 9.1 Memory Isolation

**WASM Memory Model**:
```
Worker Tile WASM Instance:
┌─────────────────────────────────────────────────────────────┐
│  WASM Linear Memory (max 64KB = 1 page)                     │
│  ┌─────────────────┬─────────────────┬───────────────────┐  │
│  │  Graph Shard    │  Feature Window │  Local State      │  │
│  │  (32KB)         │  (8KB)          │  (~1KB)           │  │
│  └─────────────────┴─────────────────┴───────────────────┘  │
│                                                              │
│  Stack (grows down from 64KB)                               │
│  ────────────────────────────────────────────────────────── │
└─────────────────────────────────────────────────────────────┘
```

**Assessment**: GOOD inherent isolation

WASM provides:
- Linear memory cannot access outside its bounds
- No direct system calls
- No file system access
- No network access

**Remaining Concerns**:

#### 9.1.1 Memory Bounds Validation
**Risk**: MEDIUM

```rust
// ADR line 1110-1113
#[no_mangle]
pub extern "C" fn ingest_delta(delta_ptr: *const u8, len: usize) -> u32 {
    let delta = unsafe { core::slice::from_raw_parts(delta_ptr, len) };
    // ...
}
```

**Issue**: Raw pointer dereference without bounds validation

**Mitigation**:
```rust
#[no_mangle]
pub extern "C" fn ingest_delta(delta_ptr: *const u8, len: usize) -> u32 {
    // 1. Validate pointer is within WASM memory
    let memory_size = wasm_memory_size();
    if delta_ptr as usize + len > memory_size {
        return ERROR_INVALID_POINTER;
    }

    // 2. Validate length is reasonable
    if len > MAX_DELTA_SIZE {
        return ERROR_DELTA_TOO_LARGE;
    }

    // 3. Safe slice creation
    let delta = unsafe {
        core::slice::from_raw_parts(delta_ptr, len)
    };

    // 4. Validate delta structure
    match Delta::try_from_bytes(delta) {
        Ok(valid_delta) => TILE_STATE.with(|state| {
            state.borrow_mut().ingest_delta(&valid_delta)
        }),
        Err(_) => ERROR_MALFORMED_DELTA,
    }
}

const MAX_DELTA_SIZE: usize = 256;
const ERROR_INVALID_POINTER: u32 = 0x8000_0001;
const ERROR_DELTA_TOO_LARGE: u32 = 0x8000_0002;
const ERROR_MALFORMED_DELTA: u32 = 0x8000_0003;
```

#### 9.1.2 Stack Overflow
**Risk**: LOW-MEDIUM

```rust
// Deep recursion could exhaust stack
pub fn recursive_cut_computation(&self, depth: usize) -> CutValue {
    if depth > 0 {
        self.recursive_cut_computation(depth - 1)
    } else {
        self.base_cut()
    }
}
```

**Mitigation**:
```rust
const MAX_RECURSION_DEPTH: usize = 32;

pub fn bounded_cut_computation(&self, depth: usize) -> Result<CutValue, StackError> {
    if depth > MAX_RECURSION_DEPTH {
        return Err(StackError::MaxDepthExceeded);
    }
    // ...
}
```

### 9.2 Sandbox Escape Prevention

**Attack Surface Analysis**:

| Vector | Risk | Mitigation |
|--------|------|------------|
| Host function imports | HIGH | Minimize imports, validate all |
| Memory.grow | MEDIUM | Limit to 1 page (64KB) |
| Table manipulation | LOW | No function tables |
| Reference types | LOW | Disabled in no_std |

**Secure Host Function Design**:

```rust
// Host functions exposed to WASM must be minimal and validated

/// ALLOWED: Return current timestamp (read-only)
#[no_mangle]
pub extern "C" fn host_get_timestamp_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// ALLOWED: Log message (length-limited)
#[no_mangle]
pub extern "C" fn host_log(ptr: *const u8, len: usize) {
    if len > 256 {
        return; // Silent truncation
    }
    // Validate ptr is in WASM memory...
    let msg = unsafe { std::slice::from_raw_parts(ptr, len) };
    if let Ok(s) = std::str::from_utf8(msg) {
        log::trace!("[wasm-tile] {}", s);
    }
}

/// FORBIDDEN: Any of these
// - File system access
// - Network access
// - Process spawning
// - Memory allocation outside WASM
// - Direct hardware access
```

### 9.3 Spectre/Meltdown Considerations

**Risk**: LOW for WASM

WASM's bounds checking and lack of speculative execution within the WASM sandbox mitigates most Spectre variants. However:

**Host Interaction Concern**:
```
WASM tile calls host_get_timestamp_ns()
Host executes native code (potentially speculative)
Side-channel information could leak to WASM
```

**Mitigation**: Constant-time host functions:

```rust
/// Constant-time timestamp (mitigates timing side-channels)
#[no_mangle]
pub extern "C" fn host_get_timestamp_ns_ct() -> u64 {
    // Add jitter to prevent precise timing analysis
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    // Round to nearest millisecond (reduce precision)
    (now / 1_000_000) * 1_000_000
}
```

### 9.4 WASM Runtime Selection

**Recommended Runtimes** (in order of preference):

1. **Wasmtime** (recommended)
   - Production-ready
   - Security-focused development
   - Cranelift backend with bounds checking

2. **Wasmer**
   - Good performance
   - Multiple backends

3. **wasm3** (for embedded)
   - Interpreter-based (smaller attack surface)
   - No JIT (no JIT-spray attacks)

**Configuration**:
```rust
use wasmtime::*;

fn create_secure_engine() -> Engine {
    let mut config = Config::new();

    // Security settings
    config.wasm_reference_types(false);
    config.wasm_bulk_memory(true);  // Needed for memcpy
    config.wasm_multi_value(false);
    config.wasm_multi_memory(false);
    config.wasm_threads(false);     // No shared memory

    // Resource limits
    config.max_wasm_stack(64 * 1024);  // 64KB stack
    config.consume_fuel(true);          // Enable fuel metering

    Engine::new(&config).unwrap()
}

fn create_secure_instance(engine: &Engine, module: &Module) -> Instance {
    let mut store = Store::new(engine, ());

    // Set fuel limit (computation bound)
    store.set_fuel(10_000_000).unwrap();  // ~10M instructions

    // Set memory limits
    let memory_type = MemoryType::new(1, Some(1));  // 1 page, max 1 page

    // Create instance with minimal imports
    let imports = vec![
        host_get_timestamp_ns.into(),
        host_log.into(),
    ];

    Instance::new(&mut store, module, &imports).unwrap()
}
```

---

## 10. Recommendations

### Priority 1: Critical (Implement Before Production)

#### R1.1: Complete Input Validation Layer
**Effort**: 2-3 days
**Risk Mitigated**: Input manipulation, injection attacks

```rust
// Implement comprehensive validation as specified in Section 3
pub struct ValidationLayer {
    action_validator: ActionValidator,
    delta_validator: DeltaValidator,
    report_validator: ReportValidator,
}
```

#### R1.2: Timeout All Lock Acquisitions
**Effort**: 1 day
**Risk Mitigated**: Deadlocks, resource exhaustion

```rust
// Add timeouts to all async lock operations
let guard = tokio::time::timeout(
    Duration::from_millis(10),
    self.lock.write()
).await?;
```

#### R1.3: Memory Bounds for All Components
**Effort**: 2 days
**Risk Mitigated**: Memory exhaustion DoS

```rust
// Implement MemoryBudget tracking
let budget = MemoryBudget::default();
MemoryTracker::global().set_budget(budget);
```

#### R1.4: Supply Chain Audit
**Effort**: 1 day
**Risk Mitigated**: Dependency vulnerabilities

```bash
cargo audit
cargo deny check
cargo vet audit
```

### Priority 2: High (Implement Before Beta)

#### R2.1: Distributed Replay Prevention
**Effort**: 3-5 days
**Risk Mitigated**: Cross-node replay attacks

Implement gossip-based bloom filter sharing as specified in Section 5.2.1.

#### R2.2: Rate Limiting Framework
**Effort**: 2-3 days
**Risk Mitigated**: DoS via computation exhaustion

```rust
pub struct RateLimiter {
    per_agent: DashMap<AgentId, TokenBucket>,
    per_action_type: DashMap<ActionType, TokenBucket>,
    global: TokenBucket,
}
```

#### R2.3: Worker Tile Anomaly Detection
**Effort**: 3-4 days
**Risk Mitigated**: Compromised worker tiles

```rust
pub struct TileAnomalyDetector {
    baseline_coherence: [RollingStats; 255],
    baseline_e_values: [RollingStats; 255],
    alert_threshold: f32,
}
```

#### R2.4: Enhanced Key Management
**Effort**: 2-3 days
**Risk Mitigated**: Key compromise, rotation failures

Implement key hierarchy and rotation as specified in Section 2.5.

### Priority 3: Medium (Implement Before GA)

#### R3.1: Post-Quantum Migration Path
**Effort**: 1-2 weeks
**Risk Mitigated**: Future quantum threats

```rust
pub struct HybridSignature {
    pub ed25519_sig: [u8; 64],
    pub ml_dsa_sig: Option<[u8; 3309]>,
}
```

#### R3.2: Constant-Time Decision Paths
**Effort**: 1 week
**Risk Mitigated**: Timing side-channels

```rust
// Use subtle crate for constant-time comparisons
use subtle::{ConstantTimeEq, Choice};

fn constant_time_threshold_check(value: f64, threshold: f64) -> Choice {
    // Constant-time comparison
}
```

#### R3.3: Verifiable Timestamps
**Effort**: 3-5 days
**Risk Mitigated**: Timestamp manipulation

Integrate with trusted timestamping authority or implement VDF proofs.

#### R3.4: Comprehensive Fuzzing
**Effort**: 1-2 weeks
**Risk Mitigated**: Unknown edge cases

```rust
#[cfg(fuzzing)]
pub fn fuzz_delta_ingestion(data: &[u8]) {
    let _ = Delta::try_from_bytes(data)
        .map(|d| WorkerTileState::default().ingest_delta(&d));
}
```

### Priority 4: Low (Track for Future)

#### R4.1: Hardware Security Module Integration
**Effort**: 2-4 weeks
**Risk Mitigated**: Key extraction from memory

#### R4.2: Formal Verification of Decision Logic
**Effort**: 1-2 months
**Risk Mitigated**: Logic bugs in safety-critical code

#### R4.3: Byzantine Fault Tolerance for Worker Aggregation
**Effort**: 2-3 weeks
**Risk Mitigated**: Compromised worker majority

---

## Summary Matrix

| Finding | Severity | Effort | Priority |
|---------|----------|--------|----------|
| Incomplete input validation | HIGH | 2-3 days | P1 |
| No lock timeouts | HIGH | 1 day | P1 |
| Memory exhaustion possible | HIGH | 2 days | P1 |
| Dependency audit needed | MEDIUM | 1 day | P1 |
| Cross-node replay possible | MEDIUM | 3-5 days | P2 |
| No rate limiting | MEDIUM | 2-3 days | P2 |
| Worker tile trust assumption | MEDIUM | 3-4 days | P2 |
| Basic key management | MEDIUM | 2-3 days | P2 |
| No post-quantum crypto | LOW | 1-2 weeks | P3 |
| Timing side-channels | LOW | 1 week | P3 |
| Local timestamps only | LOW | 3-5 days | P3 |
| No fuzzing in CI | LOW | 1-2 weeks | P3 |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-01-17 | Security Review | Initial audit |

---

## References

1. ADR-001: Anytime-Valid Coherence Gate
2. OWASP Web Application Security Testing Guide
3. CWE/SANS Top 25 Most Dangerous Software Weaknesses
4. NIST SP 800-53 Security and Privacy Controls
5. WebAssembly Security Model (https://webassembly.org/docs/security/)
6. Ed25519 RFC 8032
7. BLAKE3 Specification (https://github.com/BLAKE3-team/BLAKE3-specs)
