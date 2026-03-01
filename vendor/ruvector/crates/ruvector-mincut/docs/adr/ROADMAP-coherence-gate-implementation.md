# Implementation Roadmap: Anytime-Valid Coherence Gate

**Version**: 1.0
**Date**: 2026-01-17
**Related**: ADR-001, DDC-001

## Executive Summary

This document provides a phased implementation roadmap for the Anytime-Valid Coherence Gate (AVCG), integrating:
1. **Dynamic Min-Cut** (existing, enhanced)
2. **Online Conformal Prediction** (new)
3. **E-Values/E-Processes** (new)

The implementation is designed for incremental delivery with each phase providing standalone value.

---

## Phase 0: Preparation (Current State Analysis)

### Existing Infrastructure ✅

| Component | Location | Status |
|-----------|----------|--------|
| `SubpolynomialMinCut` | `src/subpolynomial/mod.rs` | Production-ready |
| `WitnessTree` | `src/witness/mod.rs` | Production-ready |
| `CutCertificate` | `src/certificate/mod.rs` | Production-ready |
| `DeterministicLocalKCut` | `src/localkcut/` | Production-ready |
| `GateController` | `mincut-gated-transformer/src/gate.rs` | Production-ready |
| `GatePacket` | `mincut-gated-transformer/src/packets.rs` | Production-ready |

### Dependencies to Add

```toml
# Cargo.toml additions for ruvector-mincut
[dependencies]
# Statistics
statrs = "0.17"           # Statistical distributions
rand = "0.8"              # Random number generation
rand_distr = "0.4"        # Probability distributions

# Serialization for receipts
serde_json = "1.0"
bincode = "1.3"
blake3 = "1.5"            # Fast cryptographic hashing

# Optional: async support
tokio = { version = "1", features = ["sync"], optional = true }
```

---

## Phase 1: E-Process Foundation

**Goal**: Implement core e-value and e-process infrastructure.

### Task 1.1: E-Value Module

Create `src/eprocess/evalue.rs`:

```rust
/// Core e-value type with validity guarantees
pub struct EValue {
    value: f64,
    /// Null hypothesis under which E[e] ≤ 1
    null: NullHypothesis,
    /// Computation timestamp
    timestamp: u64,
}

/// Supported null hypotheses
pub enum NullHypothesis {
    /// P(unsafe outcome) ≤ p0
    ActionSafety { p0: f64 },
    /// Current state ~ reference distribution
    StateStability { reference: DistributionId },
    /// Policy matches reference
    PolicyConsistency { reference: PolicyId },
}

impl EValue {
    /// Create from likelihood ratio
    pub fn from_likelihood_ratio(
        likelihood_h1: f64,
        likelihood_h0: f64,
    ) -> Self;

    /// Create mixture e-value for robustness
    pub fn from_mixture(
        components: &[EValue],
        weights: &[f64],
    ) -> Self;

    /// Verify E[e] ≤ 1 property empirically
    pub fn verify_validity(&self, samples: &[f64]) -> bool;
}
```

### Task 1.2: E-Process Module

Create `src/eprocess/process.rs`:

```rust
/// E-process for continuous monitoring
pub struct EProcess {
    /// Current accumulated value
    current: f64,
    /// History for audit
    history: Vec<EValue>,
    /// Update rule
    update_rule: UpdateRule,
}

pub enum UpdateRule {
    /// E_t = Π e_i (aggressive)
    Product,
    /// E_t = (1/t) Σ e_i (conservative)
    Average,
    /// E_t = λe_t + (1-λ)E_{t-1}
    ExponentialMoving { lambda: f64 },
    /// E_t = Σ w_j E_t^{(j)}
    Mixture { weights: Vec<f64> },
}

impl EProcess {
    pub fn new(rule: UpdateRule) -> Self;
    pub fn update(&mut self, e: EValue);
    pub fn current_value(&self) -> f64;

    /// Check stopping condition
    pub fn should_stop(&self, threshold: f64) -> bool;

    /// Export for audit
    pub fn to_evidence_receipt(&self) -> EvidenceReceipt;
}
```

### Task 1.3: Stopping Rules

Create `src/eprocess/stopping.rs`:

```rust
/// Anytime-valid stopping rule
pub struct StoppingRule {
    /// Threshold for rejection
    reject_threshold: f64,  // typically 1/α
    /// Threshold for acceptance (optional)
    accept_threshold: Option<f64>,
}

impl StoppingRule {
    /// Check if we can stop now
    pub fn can_stop(&self, e_process: &EProcess) -> StoppingDecision;

    /// Get confidence at current stopping time
    pub fn confidence_at_stop(&self, e_process: &EProcess) -> f64;
}

pub enum StoppingDecision {
    /// Continue accumulating evidence
    Continue,
    /// Reject null (evidence of incoherence)
    Reject { confidence: f64 },
    /// Accept null (evidence of coherence)
    Accept { confidence: f64 },
}
```

### Deliverables Phase 1
- [ ] `src/eprocess/mod.rs` - module organization
- [ ] `src/eprocess/evalue.rs` - e-value implementation
- [ ] `src/eprocess/process.rs` - e-process implementation
- [ ] `src/eprocess/stopping.rs` - stopping rules
- [ ] `src/eprocess/mixture.rs` - mixture e-values
- [ ] Unit tests with ≥95% coverage
- [ ] Integration with `CutCertificate`

### Acceptance Criteria Phase 1
- [ ] E[e] ≤ 1 verified for all implemented e-value types
- [ ] E-process maintains supermartingale property
- [ ] Stopping rule provides valid Type I error control
- [ ] Computation time < 1ms for single e-value

---

## Phase 2: Conformal Prediction

**Goal**: Implement online conformal prediction with shift adaptation.

### Task 2.1: Prediction Set Core

Create `src/conformal/prediction_set.rs`:

```rust
/// Conformal prediction set
pub struct PredictionSet<T> {
    /// Elements in the set
    elements: Vec<T>,
    /// Coverage target
    coverage: f64,
    /// Non-conformity scores
    scores: Vec<f64>,
}

impl<T> PredictionSet<T> {
    /// Check if outcome is in set
    pub fn contains(&self, outcome: &T) -> bool;

    /// Get set size (measure of uncertainty)
    pub fn size(&self) -> usize;

    /// Get normalized uncertainty measure
    pub fn uncertainty(&self) -> f64;
}
```

### Task 2.2: Non-Conformity Scores

Create `src/conformal/scores.rs`:

```rust
/// Non-conformity score function
pub trait NonConformityScore {
    type Input;
    type Output;

    fn score(&self, input: &Self::Input, output: &Self::Output) -> f64;
}

/// Absolute residual score
pub struct AbsoluteResidual<P: Predictor> {
    predictor: P,
}

/// Normalized residual score
pub struct NormalizedResidual<P: Predictor + UncertaintyEstimator> {
    predictor: P,
}

/// Conformalized Quantile Regression (CQR)
pub struct CQRScore<Q: QuantilePredictor> {
    quantile_predictor: Q,
}
```

### Task 2.3: Online Conformal with Adaptation

Create `src/conformal/online.rs`:

```rust
/// Online conformal predictor with shift adaptation
pub struct OnlineConformal<S: NonConformityScore> {
    score_fn: S,
    /// Calibration buffer
    calibration: RingBuffer<f64>,
    /// Current quantile
    quantile: f64,
    /// Adaptation method
    adaptation: AdaptationMethod,
}

pub enum AdaptationMethod {
    /// Adaptive Conformal Inference
    ACI { learning_rate: f64 },
    /// Retrospective adjustment
    Retrospective { window: usize },
    /// Conformal Optimistic Prediction
    COP { cdf_estimator: Box<dyn CDFEstimator> },
}

impl<S: NonConformityScore> OnlineConformal<S> {
    /// Generate prediction set
    pub fn predict(&self, input: &S::Input) -> PredictionSet<S::Output>;

    /// Update with observed outcome
    pub fn update(&mut self, input: &S::Input, outcome: &S::Output);

    /// Get current coverage estimate
    pub fn coverage_estimate(&self) -> f64;
}
```

### Task 2.4: CORE RL-Based Adaptation

Create `src/conformal/core.rs`:

```rust
/// CORE: RL-based conformal adaptation
pub struct COREConformal<S: NonConformityScore> {
    base: OnlineConformal<S>,
    /// RL agent for quantile adjustment
    agent: QuantileAgent,
    /// Coverage as reward signal
    coverage_target: f64,
}

/// Simple TD-learning agent for quantile adjustment
struct QuantileAgent {
    q_value: f64,
    learning_rate: f64,
    discount: f64,
}

impl<S: NonConformityScore> COREConformal<S> {
    /// Predict with RL-adjusted quantile
    pub fn predict(&self, input: &S::Input) -> PredictionSet<S::Output>;

    /// Update agent and base conformal
    pub fn update(&mut self, input: &S::Input, outcome: &S::Output, covered: bool);
}
```

### Deliverables Phase 2
- [ ] `src/conformal/mod.rs` - module organization
- [ ] `src/conformal/prediction_set.rs` - prediction set types
- [ ] `src/conformal/scores.rs` - non-conformity scores
- [ ] `src/conformal/online.rs` - online conformal with ACI
- [ ] `src/conformal/retrospective.rs` - retrospective adjustment
- [ ] `src/conformal/cop.rs` - Conformal Optimistic Prediction
- [ ] `src/conformal/core.rs` - RL-based adaptation
- [ ] Unit tests with ≥90% coverage

### Acceptance Criteria Phase 2
- [ ] Marginal coverage ≥ 1 - α on exchangeable data
- [ ] Coverage maintained under gradual shift (δ < 0.1/step)
- [ ] Recovery within 100 steps after abrupt shift
- [ ] Prediction latency < 10ms

---

## Phase 3: Gate Integration

**Goal**: Integrate all components into unified gate controller.

### Task 3.1: Anytime Gate Policy

Create `src/anytime_gate/policy.rs`:

```rust
/// Policy for anytime-valid gate
pub struct AnytimeGatePolicy {
    /// E-process thresholds
    pub e_deny_threshold: f64,      // τ_deny
    pub e_permit_threshold: f64,    // τ_permit

    /// Conformal thresholds
    pub uncertainty_threshold: f64,  // θ_uncertainty
    pub confidence_threshold: f64,   // θ_confidence

    /// Min-cut thresholds (from existing GatePolicy)
    pub lambda_min: u32,
    pub boundary_max: u16,

    /// Adaptation settings
    pub adaptive_thresholds: bool,
    pub threshold_learning_rate: f64,
}
```

### Task 3.2: Unified Gate Controller

Create `src/anytime_gate/controller.rs`:

```rust
/// Unified anytime-valid coherence gate
pub struct AnytimeGateController<S: NonConformityScore> {
    /// Existing min-cut infrastructure
    mincut: SubpolynomialMinCut,

    /// Conformal predictor
    conformal: OnlineConformal<S>,

    /// E-process for evidence
    e_process: EProcess,

    /// Policy
    policy: AnytimeGatePolicy,
}

impl<S: NonConformityScore> AnytimeGateController<S> {
    /// Evaluate gate for action
    pub fn evaluate(&mut self, action: &Action, context: &Context) -> GateResult;

    /// Update after observing outcome
    pub fn update(&mut self, action: &Action, outcome: &Outcome);

    /// Generate witness receipt
    pub fn receipt(&self, decision: &GateDecision) -> WitnessReceipt;
}

pub struct GateResult {
    pub decision: GateDecision,

    // From min-cut
    pub cut_value: f64,
    pub witness_partition: Option<WitnessPartition>,

    // From conformal
    pub prediction_set_size: f64,
    pub uncertainty: f64,

    // From e-process
    pub e_value: f64,
    pub evidence_sufficient: bool,
}
```

### Task 3.3: Witness Receipt

Create `src/anytime_gate/receipt.rs`:

```rust
/// Cryptographically sealed witness receipt
#[derive(Serialize, Deserialize)]
pub struct WitnessReceipt {
    /// Receipt metadata
    pub id: Uuid,
    pub timestamp: u64,
    pub action_id: ActionId,
    pub decision: GateDecision,

    /// Structural witness (from min-cut)
    pub structural: StructuralWitness,

    /// Predictive witness (from conformal)
    pub predictive: PredictiveWitness,

    /// Evidential witness (from e-process)
    pub evidential: EvidentialWitness,

    /// Cryptographic seal
    pub hash: [u8; 32],
    pub signature: Option<[u8; 64]>,
}

#[derive(Serialize, Deserialize)]
pub struct StructuralWitness {
    pub cut_value: f64,
    pub partition_hash: [u8; 32],
    pub critical_edge_count: usize,
}

#[derive(Serialize, Deserialize)]
pub struct PredictiveWitness {
    pub prediction_set_size: usize,
    pub coverage_target: f64,
    pub adaptation_rate: f64,
}

#[derive(Serialize, Deserialize)]
pub struct EvidentialWitness {
    pub e_value: f64,
    pub e_process_cumulative: f64,
    pub null_hypothesis: String,
    pub stopping_valid: bool,
}

impl WitnessReceipt {
    pub fn seal(&mut self) {
        self.hash = blake3::hash(&self.to_bytes()).into();
    }

    pub fn verify(&self) -> bool {
        self.hash == blake3::hash(&self.to_bytes_without_hash()).into()
    }
}
```

### Deliverables Phase 3
- [ ] `src/anytime_gate/mod.rs` - module organization
- [ ] `src/anytime_gate/policy.rs` - gate policy
- [ ] `src/anytime_gate/controller.rs` - unified controller
- [ ] `src/anytime_gate/decision.rs` - decision types
- [ ] `src/anytime_gate/receipt.rs` - witness receipts
- [ ] Integration tests with full pipeline
- [ ] Benchmarks for latency validation

### Acceptance Criteria Phase 3
- [ ] Gate latency p99 < 50ms
- [ ] All three signals integrated correctly
- [ ] Witness receipts pass verification
- [ ] Graceful degradation on component failure

---

## Phase 4: Production Hardening

**Goal**: Production-ready implementation with monitoring and optimization.

### Task 4.1: Performance Optimization
- [ ] SIMD-optimized e-value computation
- [ ] Lazy evaluation for conformal sets
- [ ] Batched graph updates for min-cut
- [ ] Memory-mapped receipt storage

### Task 4.2: Monitoring & Alerting
- [ ] Prometheus metrics for gate decisions
- [ ] Coverage drift detection
- [ ] E-process anomaly alerts
- [ ] Latency histogram tracking

### Task 4.3: Operational Tooling
- [ ] Receipt query API
- [ ] Threshold tuning dashboard
- [ ] A/B testing framework for policy comparison
- [ ] Incident replay from receipts

### Task 4.4: Documentation
- [ ] API documentation
- [ ] Operator runbook
- [ ] Threshold tuning guide
- [ ] Troubleshooting guide

---

## Timeline Summary

| Phase | Duration | Dependencies | Deliverable |
|-------|----------|--------------|-------------|
| Phase 0 | Complete | - | Requirements analysis |
| Phase 1 | 2 weeks | None | E-process module |
| Phase 2 | 3 weeks | Phase 1 | Conformal module |
| Phase 3 | 2 weeks | Phase 1, 2 | Unified gate |
| Phase 4 | 2 weeks | Phase 3 | Production hardening |

**Total estimated effort**: 9 weeks

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| E-value power too low | Medium | High | Mixture e-values; tuned alternatives |
| Conformal sets too large | Medium | Medium | COP for tighter sets; better base predictor |
| Latency exceeds budget | Low | High | Early profiling; lazy evaluation |
| Integration complexity | Medium | Medium | Phased delivery; isolated modules |
| Threshold tuning difficulty | High | Medium | Adaptive thresholds; meta-learning |

---

## Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| False deny rate | < 5% | Simulation |
| Missed unsafe rate | < 0.1% | Simulation |
| Gate latency p99 | < 50ms | Production |
| Coverage maintenance | ≥ 85% | Production |
| Receipt verification pass | 100% | Audit |

---

## References

1. El-Hayek, Henzinger, Li. arXiv:2512.13105 (Dec 2025)
2. Online Conformal with Retrospective. arXiv:2511.04275 (Nov 2025)
3. Ramdas, Wang. "Hypothesis Testing with E-values" (2025)
4. ICML 2025 Tutorial on SAVI
5. Distribution-informed Conformal (COP). arXiv:2512.07770 (Dec 2025)
