# DDC-001: Anytime-Valid Coherence Gate - Design Decision Criteria

**Version**: 1.0
**Date**: 2026-01-17
**Related ADR**: ADR-001-anytime-valid-coherence-gate

## Purpose

This document specifies the design decision criteria for implementing the Anytime-Valid Coherence Gate (AVCG). It provides concrete guidance for architectural choices, implementation trade-offs, and acceptance criteria.

---

## 1. Graph Model Design Decisions

### DDC-1.1: Action Graph Construction

**Decision Required**: How to construct the action graph G_t from agent state?

| Option | Description | Pros | Cons | Recommendation |
|--------|-------------|------|------|----------------|
| **A. State-Action Pairs** | Nodes = (state, action), Edges = transitions | Fine-grained control; precise cuts | Large graphs; O(|S|·|A|) nodes | Use for high-stakes domains |
| **B. Abstract State Clusters** | Nodes = state clusters, Edges = aggregate transitions | Smaller graphs; faster updates | May miss nuanced boundaries | **Recommended for v0** |
| **C. Learned Embeddings** | Nodes = learned state embeddings | Adaptive; captures latent structure | Requires training data; less interpretable | Future enhancement |

**Acceptance Criteria**:
- [ ] Graph construction completes in < 100μs for typical agent states
- [ ] Graph accurately represents reachability to unsafe states
- [ ] Witness partitions are human-interpretable

### DDC-1.2: Edge Weight Semantics

**Decision Required**: What do edge weights represent?

| Option | Interpretation | Use Case |
|--------|---------------|----------|
| **A. Risk Scores** | Higher weight = higher risk of unsafe outcome | Min-cut = minimum total risk to unsafe |
| **B. Inverse Probability** | Higher weight = less likely transition | Min-cut = least likely path to unsafe |
| **C. Unit Weights** | All edges weight 1.0 | Min-cut = fewest actions to unsafe |
| **D. Conformal Set Size** | Weight = |C_t| for that action | Natural integration with predictive uncertainty |

**Recommendation**: Option D creates natural integration between min-cut and conformal prediction.

**Acceptance Criteria**:
- [ ] Weight semantics are documented and consistent
- [ ] Min-cut value has interpretable meaning for operators
- [ ] Weights update correctly on new observations

---

## 2. Conformal Predictor Architecture

### DDC-2.1: Base Predictor Selection

**Decision Required**: Which base predictor to wrap with conformal prediction?

| Option | Characteristics | Computational Cost |
|--------|----------------|-------------------|
| **A. Neural Network** | High capacity; requires calibration | Medium-High |
| **B. Random Forest** | Built-in uncertainty; robust | Medium |
| **C. Gaussian Process** | Natural uncertainty; O(n³) training | High |
| **D. Ensemble with Dropout** | Approximate Bayesian; scalable | Medium |

**Recommendation**: Option D (Ensemble with Dropout) for balance of capacity and uncertainty.

**Acceptance Criteria**:
- [ ] Base predictor achieves acceptable accuracy on held-out data
- [ ] Prediction latency < 10ms for single action
- [ ] Uncertainty estimates correlate with actual error rates

### DDC-2.2: Non-Conformity Score Function

**Decision Required**: How to compute non-conformity scores?

| Option | Formula | Properties |
|--------|---------|------------|
| **A. Absolute Residual** | s(x,y) = |y - ŷ(x)| | Simple; symmetric |
| **B. Normalized Residual** | s(x,y) = |y - ŷ(x)| / σ̂(x) | Scale-invariant |
| **C. CQR** | s(x,y) = max(q̂_lo - y, y - q̂_hi) | Heteroscedastic coverage |

**Recommendation**: Option C (CQR) for heteroscedastic agent environments.

**Acceptance Criteria**:
- [ ] Marginal coverage ≥ 1 - α over calibration window
- [ ] Conditional coverage approximately uniform across feature space
- [ ] Prediction sets are not trivially large

### DDC-2.3: Shift Adaptation Method

**Decision Required**: How to adapt conformal predictor to distribution shift?

| Method | Adaptation Speed | Conservativeness |
|--------|-----------------|------------------|
| **A. ACI (Adaptive Conformal)** | Medium | High |
| **B. Retrospective Adjustment** | Fast | Medium |
| **C. COP (Conformal Optimistic)** | Fastest | Low (but valid) |
| **D. CORE (RL-based)** | Adaptive | Task-dependent |

**Recommendation**: Hybrid approach:
- Use COP for normal operation (fast, less conservative)
- Fall back to ACI under detected severe shift
- Use retrospective adjustment for post-hoc correction

**Acceptance Criteria**:
- [ ] Coverage maintained during gradual shift (δ < 0.1/step)
- [ ] Recovery to target coverage within 100 steps after abrupt shift
- [ ] No catastrophic coverage failures (coverage never < 0.5)

---

## 3. E-Process Construction

### DDC-3.1: E-Value Computation Method

**Decision Required**: How to compute per-action e-values?

| Method | Requirements | Robustness |
|--------|--------------|------------|
| **A. Likelihood Ratio** | Density models for H₀ and H₁ | Low (model-dependent) |
| **B. Universal Inference** | Split data; no density needed | Medium |
| **C. Mixture E-Values** | Multiple alternatives | High (hedged) |
| **D. Betting E-Values** | Online learning framework | High (adaptive) |

**Recommendation**: Option C (Mixture E-Values) for robustness:
```
e_t = (1/K) Σ_k e_t^{(k)}
```
Where each e_t^{(k)} tests a different alternative hypothesis.

**Acceptance Criteria**:
- [ ] E[e_t | H₀] ≤ 1 verified empirically
- [ ] Power against reasonable alternatives > 0.5
- [ ] Computation time < 1ms per e-value

### DDC-3.2: E-Process Update Rule

**Decision Required**: How to update the e-process over time?

| Rule | Formula | Properties |
|------|---------|------------|
| **A. Product** | E_t = Π_{i=1}^t e_i | Aggressive; exponential power |
| **B. Average** | E_t = (1/t) Σ_{i=1}^t e_i | Conservative; bounded |
| **C. Exponential Moving** | E_t = λ·e_t + (1-λ)·E_{t-1} | Balanced; forgetting |
| **D. Mixture Supermartingale** | E_t = Σ_j w_j · E_t^{(j)} | Robust; hedged |

**Recommendation**:
- Option A (Product) for high-stakes single decisions
- Option D (Mixture) for continuous monitoring

**Acceptance Criteria**:
- [ ] E_t remains nonnegative supermartingale
- [ ] Stopping time τ has valid Type I error: P(E_τ ≥ 1/α) ≤ α
- [ ] Power grows with evidence accumulation

### DDC-3.3: Null Hypothesis Specification

**Decision Required**: What constitutes the "coherence" null hypothesis?

| Formulation | Meaning |
|-------------|---------|
| **A. Action Safety** | H₀: P(action leads to unsafe state) ≤ p₀ |
| **B. State Stability** | H₀: P(state deviates from normal) ≤ p₀ |
| **C. Policy Consistency** | H₀: Current policy ≈ reference policy |
| **D. Composite** | H₀: (A) ∧ (B) ∧ (C) |

**Recommendation**: Start with Option A, extend to Option D for production.

**Acceptance Criteria**:
- [ ] H₀ is well-specified and testable
- [ ] False alarm rate matches target α
- [ ] Null violations are meaningfully dangerous

---

## 4. Integration Architecture

### DDC-4.1: Signal Combination Strategy

**Decision Required**: How to combine the three signals into a gate decision?

| Strategy | Logic | Properties |
|----------|-------|------------|
| **A. Sequential Short-Circuit** | Cut → Conformal → E-process | Fast rejection; ordered |
| **B. Parallel with Voting** | All evaluate; majority rules | Robust; slower |
| **C. Weighted Integration** | score = w₁·cut + w₂·conf + w₃·e | Flexible; needs tuning |
| **D. Hierarchical** | E-process gates conformal gates cut | Layered authority |

**Recommendation**: Option A (Sequential Short-Circuit):
1. Min-cut DENY is immediate (structural safety)
2. Conformal uncertainty gates e-process (no point accumulating evidence if outcome unpredictable)
3. E-process makes final permit/defer decision

**Acceptance Criteria**:
- [ ] Gate latency < 50ms for typical decisions
- [ ] No single-point-of-failure (graceful degradation)
- [ ] Decision audit trail is complete

### DDC-4.2: Graceful Degradation

**Decision Required**: How should the gate behave when components fail?

| Component Failure | Fallback Behavior |
|-------------------|-------------------|
| Min-cut unavailable | Defer all actions; alert operator |
| Conformal predictor fails | Use widened prediction sets (conservative) |
| E-process computation fails | Use last valid e-value; decay confidence |
| All components fail | Full DENY; require human approval |

**Acceptance Criteria**:
- [ ] Failure detection within 100ms
- [ ] Fallback never less safe than full DENY
- [ ] Recovery is automatic when component restores

### DDC-4.3: Latency Budget Allocation

**Decision Required**: How to allocate total latency budget across components?

Given total budget T_total (e.g., 50ms):

| Component | Allocation | Rationale |
|-----------|------------|-----------|
| Min-cut update | 0.2 · T | Amortized; subpolynomial |
| Conformal prediction | 0.4 · T | Main computation |
| E-process update | 0.2 · T | Arithmetic; fast |
| Decision logic | 0.1 · T | Simple rules |
| Receipt generation | 0.1 · T | Hashing; logging |

**Acceptance Criteria**:
- [ ] p99 latency < T_total
- [ ] No component exceeds 2× its budget
- [ ] Latency monitoring in place

---

## 5. Operational Parameters

### DDC-5.1: Threshold Configuration

| Parameter | Symbol | Default | Range | Tuning Guidance |
|-----------|--------|---------|-------|-----------------|
| E-process deny threshold | τ_deny | 0.01 | [0.001, 0.1] | Lower = more conservative |
| E-process permit threshold | τ_permit | 100 | [10, 1000] | Higher = more evidence required |
| Uncertainty threshold | θ_uncertainty | 0.5 | [0.1, 1.0] | Fraction of outcome space |
| Confidence threshold | θ_confidence | 0.1 | [0.01, 0.3] | Fraction of outcome space |
| Conformal coverage target | 1-α | 0.9 | [0.8, 0.99] | Higher = larger sets |

### DDC-5.2: Audit Requirements

| Requirement | Specification |
|-------------|---------------|
| Receipt retention | 90 days minimum |
| Receipt format | JSON + protobuf |
| Receipt signing | Ed25519 signature |
| Receipt searchability | Indexed by action_id, timestamp, decision |
| Receipt integrity | Merkle tree for batch verification |

---

## 6. Testing & Validation Criteria

### DDC-6.1: Unit Test Coverage

| Module | Coverage Target | Critical Paths |
|--------|-----------------|----------------|
| conformal/ | ≥ 90% | Prediction set generation; shift adaptation |
| eprocess/ | ≥ 95% | E-value validity; supermartingale property |
| anytime_gate/ | ≥ 90% | Decision logic; receipt generation |

### DDC-6.2: Integration Test Scenarios

| Scenario | Expected Behavior |
|----------|-------------------|
| Normal operation | Permit rate > 90% |
| Gradual shift | Coverage maintained; permit rate may decrease |
| Abrupt shift | Temporary DEFER; recovery within 100 steps |
| Adversarial probe | DENY rate increases; alerts generated |
| Component failure | Graceful degradation; no unsafe permits |

### DDC-6.3: Benchmark Requirements

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Gate latency p50 | < 10ms | Continuous profiling |
| Gate latency p99 | < 50ms | Continuous profiling |
| False deny rate | < 5% | Simulation with known-safe actions |
| Missed unsafe rate | < 0.1% | Simulation with known-unsafe actions |
| Coverage maintenance | ≥ 85% | Real distribution shift scenarios |

---

## 7. Implementation Phases

### Phase 1: Foundation (v0.1)
- [ ] E-value and e-process core implementation
- [ ] Basic conformal prediction with ACI
- [ ] Integration with existing `GateController`
- [ ] Simple witness receipts

### Phase 2: Adaptation (v0.2)
- [ ] COP and retrospective adjustment
- [ ] Mixture e-values for robustness
- [ ] Graph model with conformal-based weights
- [ ] Enhanced audit trail

### Phase 3: Production (v1.0)
- [ ] CORE RL-based adaptation
- [ ] Learned graph construction
- [ ] Cryptographic receipt signing
- [ ] Full monitoring and alerting

---

## 8. Open Questions for Review

1. **Graph Model Scope**: Should the action graph include only immediate actions or multi-step lookahead?

2. **E-Process Null**: Is "action safety" the right null hypothesis, or should we test "policy consistency"?

3. **Threshold Learning**: Should thresholds be fixed or learned via meta-optimization?

4. **Human-in-Loop**: How should DEFER decisions be presented to human operators?

5. **Adversarial Robustness**: How does AVCG perform against adaptive adversaries who observe gate decisions?

---

## 9. Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Architecture Lead | | | |
| Security Lead | | | |
| ML Lead | | | |
| Engineering Lead | | | |

---

## Appendix A: Glossary

| Term | Definition |
|------|------------|
| **E-value** | Nonnegative test statistic with E[e] ≤ 1 under null |
| **E-process** | Sequence of e-values forming a nonnegative supermartingale |
| **Conformal Prediction** | Distribution-free method for calibrated uncertainty |
| **Witness Partition** | Explicit (S, V\S) showing which vertices are separated |
| **Anytime-Valid** | Guarantee holds at any stopping time |
| **COP** | Conformal Optimistic Prediction |
| **CORE** | Conformal Regression via Reinforcement Learning |
| **ACI** | Adaptive Conformal Inference |

## Appendix B: Key Equations

### E-Value Validity
```
E_H₀[e] ≤ 1
```

### Anytime-Valid Type I Error
```
P_H₀(∃t: E_t ≥ 1/α) ≤ α
```

### Conformal Coverage
```
P(Y_{t+1} ∈ C_t(X_{t+1})) ≥ 1 - α
```

### E-Value Composition
```
e₁ · e₂ is valid if e₁, e₂ independent
```
