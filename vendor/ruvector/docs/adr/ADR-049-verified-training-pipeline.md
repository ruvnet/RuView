# ADR-049: Verified Training Pipeline

## Status

Accepted

## Date

2026-02-25

## Context

Training graph transformers involves thousands of gradient steps, each of which modifies model weights. In safety-critical applications, we need guarantees that training did not introduce pathological behavior: unbounded loss spikes, conservation law violations, equivariance breakage, or adversarial vulnerability. Post-hoc auditing of trained models is expensive and often misses subtle training-time regressions.

The RuVector workspace provides the building blocks for verified training:

- `ruvector-gnn` provides `Optimizer` (SGD, Adam), `ElasticWeightConsolidation` (EWC), `LearningRateScheduler`, `ReplayBuffer`, and a training loop with `TrainConfig` in `crates/ruvector-gnn/src/training.rs`
- `ruvector-verified` provides `ProofEnvironment`, `ProofAttestation` (82 bytes), `FastTermArena` for high-throughput proof allocation, and tiered verification via `ProofTier`
- `ruvector-coherence` provides `SpectralCoherenceScore` and `SpectralTracker` (behind `spectral` feature) for monitoring model quality during training
- `ruvector-mincut-gated-transformer` provides `EnergyGate` in `crates/ruvector-mincut-gated-transformer/src/energy_gate.rs` for energy-based decision making

However, there is no mechanism for issuing per-step invariant proofs during training, no `TrainingCertificate` that attests to the training run's integrity, and no integration between the proof system and the gradient update loop.

## Decision

We will implement a `verified_training` module in `ruvector-graph-transformer` that wraps `ruvector-gnn`'s training infrastructure with proof gates, producing per-step invariant proofs and a final `TrainingCertificate` that attests to the entire training run.

### VerifiedTrainer

```rust
/// A training wrapper that issues proof attestations per gradient step.
///
/// Wraps ruvector_gnn::training::Optimizer and composes with
/// ruvector_verified::ProofEnvironment for per-step invariant verification.
pub struct VerifiedTrainer {
    /// The underlying GNN optimizer (SGD or Adam).
    optimizer: Optimizer,
    /// EWC for continual learning (optional).
    ewc: Option<ElasticWeightConsolidation>,
    /// Learning rate scheduler.
    scheduler: LearningRateScheduler,
    /// Proof environment for generating attestations.
    proof_env: ProofEnvironment,
    /// Fast arena for high-throughput proof allocation.
    arena: FastTermArena,
    /// Per-step invariant specifications.
    invariants: Vec<TrainingInvariant>,
    /// Accumulated attestations for the training run.
    ledger: MutationLedger,
    /// Configuration.
    config: VerifiedTrainerConfig,
}
```

### Per-Step Invariant Proofs

Each gradient step is bracketed by invariant checks. The `TrainingInvariant` enum defines what is verified:

```rust
pub enum TrainingInvariant {
    /// Loss stability: loss stays within a bounded envelope relative to
    /// a moving average. Raw loss is NOT monotonic in SGD — this invariant
    /// captures what is actually enforceable: bounded deviation from trend.
    ///
    /// **This is a true invariant**, not a heuristic: the proof certifies
    /// that loss_t <= moving_avg(loss, window) * (1 + spike_cap).
    LossStabilityBound {
        /// Maximum spike relative to moving average (e.g., 0.10 = 10% above MA).
        spike_cap: f64,
        /// Window size for exponential moving average.
        window: usize,
        /// Gradient norm cap: reject step if ||grad|| > this value.
        max_gradient_norm: f64,
        /// Step size cap: reject step if effective lr * ||grad|| > this value.
        max_step_size: f64,
    },

    /// Weight norm conservation: ||W_t|| stays within bounds per layer.
    /// Prevents gradient explosion/vanishing.
    ///
    /// Rollback strategy: **delta-apply** — gradients are applied to a
    /// scratch buffer, norms checked, then committed only if bounds hold.
    /// This avoids doubling peak memory via full snapshots.
    WeightNormBound {
        /// Maximum L2 norm per layer.
        max_norm: f64,
        /// Minimum L2 norm per layer (prevents collapse).
        min_norm: f64,
        /// Rollback strategy.
        rollback: RollbackStrategy,
    },

    /// Equivariance: model output is equivariant to graph permutations.
    /// **This is a statistical test, not a formal proof.** The certificate
    /// records the exact scope: rng seed, sample count, permutation ID hashes.
    /// A verifier can replay the exact same permutations to confirm.
    PermutationEquivariance {
        /// Number of random permutations to test per check.
        samples: usize,
        /// Maximum allowed deviation (L2 distance / output norm).
        max_deviation: f64,
        /// RNG seed for reproducibility. Bound into the proof scope.
        rng_seed: u64,
    },

    /// Lipschitz bound: **estimated** Lipschitz constant stays below threshold.
    /// Verified per-layer via spectral norm power iteration.
    ///
    /// **Attestation scope:** The certificate records that the estimated bound
    /// (via K power iterations with tolerance eps) stayed below max_lipschitz.
    /// This does NOT certify the true Lipschitz constant — it certifies
    /// that the estimate with stated parameters was within bounds.
    LipschitzBound {
        /// Maximum Lipschitz constant per layer.
        max_lipschitz: f64,
        /// Power iteration steps for spectral norm estimation.
        power_iterations: usize,
        /// Convergence tolerance for power iteration.
        tolerance: f64,
    },

    /// Coherence: spectral coherence score stays above threshold.
    /// Uses ruvector-coherence::spectral::SpectralCoherenceScore.
    ///
    /// **Attestation scope:** Like Lipschitz, this is an estimate based on
    /// sampled eigenvalues. The certificate records the estimation parameters.
    CoherenceBound {
        /// Minimum coherence score.
        min_coherence: f64,
        /// Number of eigenvalue samples for estimation.
        eigenvalue_samples: usize,
    },

    /// Energy gate: compute energy or coherence proxy BEFORE applying
    /// gradients. If below threshold, require a stronger proof tier,
    /// reduce learning rate, or refuse the step entirely.
    ///
    /// Integrates with ruvector-mincut-gated-transformer::EnergyGate
    /// to make training behave like inference gating.
    EnergyGate {
        /// Minimum energy threshold for standard-tier step.
        min_energy: f64,
        /// If energy < min_energy, force this tier for verification.
        escalation_tier: ProofTier,
        /// If energy < critical_energy, refuse the step entirely.
        critical_energy: f64,
    },

    /// Custom invariant with a user-provided verification function.
    Custom {
        /// Name for logging and attestation.
        name: String,
        /// Estimated proof complexity (for tier routing).
        complexity: u32,
    },
}

/// Rollback strategy for failed invariant checks.
pub enum RollbackStrategy {
    /// Apply gradients to a scratch buffer, check invariants, then commit.
    /// Peak memory: weights + one layer's gradients. No full snapshot.
    DeltaApply,
    /// Store per-layer deltas, revert only modified layers on failure.
    /// Peak memory: weights + delta buffer (typically < 10% of weights).
    ChunkedRollback,
    /// Full snapshot (doubles peak memory). Use only when other strategies
    /// are insufficient (e.g., cross-layer invariants).
    FullSnapshot,
}
```

### Invariant Verification Flow

```rust
impl VerifiedTrainer {
    /// Execute one verified training step.
    ///
    /// 1. Compute gradients via the underlying optimizer
    /// 2. Before applying gradients, verify pre-step invariants
    /// 3. Apply gradients
    /// 4. Verify post-step invariants
    /// 5. Issue attestation for this step
    /// 6. If any invariant fails, roll back gradients and return error
    pub fn step(
        &mut self,
        loss: f64,
        gradients: &Gradients,
        weights: &mut Weights,
    ) -> Result<StepAttestation> {
        // 1. Pre-step: verify gradient bounds and loss stability
        let pre_proofs = self.verify_invariants(
            InvariantPhase::PreStep,
            loss, weights,
        )?;

        // 2. Energy gate: compute energy/coherence proxy BEFORE mutation.
        //    If below threshold, escalate proof tier or refuse step.
        if let Some(energy_gate) = &self.energy_gate {
            let energy = energy_gate.evaluate(weights, gradients);
            if energy < energy_gate.critical_energy {
                return Err(GraphTransformerError::MutationRejected {
                    reason: format!("energy {} < critical {}", energy, energy_gate.critical_energy),
                });
            }
            if energy < energy_gate.min_energy {
                // Force escalation to stronger proof tier
                self.current_tier_override = Some(energy_gate.escalation_tier);
            }
        }

        // 3. Apply gradients via delta-apply strategy (default).
        //    Gradients go into a scratch buffer, not directly into weights.
        let delta = self.optimizer.compute_delta(gradients, weights)?;

        // 4. Post-step verification on proposed (weights + delta).
        //    No mutation has occurred yet.
        match self.verify_invariants_on_proposed(
            InvariantPhase::PostStep, loss, weights, &delta
        ) {
            Ok(post_proofs) => {
                // 5. Commit: apply delta to actual weights.
                weights.apply_delta(&delta);

                // 6. Compose attestation and append to ledger.
                let attestation = self.compose_step_attestation(
                    pre_proofs, post_proofs,
                );
                self.ledger.append(attestation.clone());
                self.scheduler.step();
                self.current_tier_override = None;
                Ok(StepAttestation {
                    step: self.ledger.len() as u64,
                    attestation,
                    loss,
                    invariants_checked: self.invariants.len(),
                    overridden: false,
                })
            }
            Err(e) if self.config.allow_override => {
                // Degraded mode: step proceeds with OverrideProof.
                // The override is visible in the certificate.
                let override_proof = self.create_override_proof(&e)?;
                weights.apply_delta(&delta);
                self.ledger.append(override_proof.clone());
                self.override_count += 1;
                Ok(StepAttestation {
                    step: self.ledger.len() as u64,
                    attestation: override_proof,
                    loss,
                    invariants_checked: self.invariants.len(),
                    overridden: true,
                })
            }
            Err(e) => {
                // Fail-closed: delta is discarded, weights unchanged.
                // Refusal is recorded in the ledger.
                let refusal = self.create_refusal_attestation(&e);
                self.ledger.append(refusal);
                Err(e)
            }
        }
    }
}
```

### Tier Routing for Training Invariants

Training invariant verification uses the same three-tier routing as ADR-047:

| Invariant | Typical Tier | Rationale | Formally Proven? |
|-----------|-------------|-----------|------------------|
| `LossStabilityBound` | Reflex | Moving avg comparison + gradient norm check, < 10 ns | **Yes** — bounded comparison |
| `WeightNormBound` | Standard(100) | L2 norm computation, < 1 us | **Yes** — exact computation |
| `PermutationEquivariance` | Deep | Random permutation + forward pass, < 100 us | **No** — statistical test with bound scope |
| `LipschitzBound` | Standard(500) | Power iteration spectral norm, < 10 us | **No** — estimate with stated tolerance |
| `CoherenceBound` | Standard(200) | Spectral coherence from sampled eigenvalues, < 5 us | **No** — estimate with stated sample count |
| `EnergyGate` | Reflex/Standard | Energy proxy evaluation, < 100 ns | **Yes** — threshold comparison |
| `Custom` | Routed by `complexity` field | User-defined | Depends on implementation |

**Distinction between proven and estimated invariants:** The certificate explicitly records which invariants are formally proven (exact computation within the proof system) and which are statistical estimates with bound scope (rng_seed, sample_count, iterations, tolerance). A verifier knows exactly what was tested and can replay it.

The routing decision is made by converting each `TrainingInvariant` into a `ProofKind` and calling `ruvector_verified::gated::route_proof`. For example, `LossStabilityBound` maps to `ProofKind::DimensionEquality` (literal comparison), while `PermutationEquivariance` maps to `ProofKind::Custom { estimated_complexity: samples * 100 }`.

### Certified Adversarial Robustness

For models that require adversarial robustness certification, the `verified_training` module provides an IBP (Interval Bound Propagation) / DeepPoly integration:

```rust
pub struct RobustnessCertifier {
    /// Perturbation radius (L-infinity norm).
    epsilon: f64,
    /// Certification method.
    method: CertificationMethod,
}

pub enum CertificationMethod {
    /// Interval Bound Propagation -- fast but loose.
    IBP,
    /// DeepPoly -- tighter but slower.
    DeepPoly,
    /// Combined: IBP for initial bound, DeepPoly for refinement.
    Hybrid { ibp_warmup_epochs: usize },
}

impl RobustnessCertifier {
    /// Certify that the model's output is stable within epsilon-ball.
    /// Returns a ProofGate<RobustnessCertificate> with the certified radius.
    pub fn certify(
        &self,
        model: &GraphTransformer<impl GraphRepr>,
        input: &GraphBatch,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<RobustnessCertificate>>;
}

pub struct RobustnessCertificate {
    /// Certified perturbation radius.
    pub certified_radius: f64,
    /// Fraction of nodes certified robust.
    pub certified_fraction: f64,
    /// Method used.
    pub method: CertificationMethod,
    /// Attestation.
    pub attestation: ProofAttestation,
}
```

### Training Certificate

At the end of a training run, a `TrainingCertificate` is produced by composing all step attestations:

```rust
pub struct TrainingCertificate {
    /// Total training steps completed.
    pub total_steps: u64,
    /// Total invariant violations (zero if fully verified).
    pub violations: u64,
    /// Number of steps that proceeded via OverrideProof (degraded mode).
    pub overridden_steps: u64,
    /// Composed attestation over all steps via compose_chain.
    pub attestation: ProofAttestation,
    /// Final loss value.
    pub final_loss: f64,
    /// Final coherence score (if CoherenceBound invariant was active).
    pub final_coherence: Option<f64>,
    /// Robustness certificate (if adversarial certification was run).
    pub robustness: Option<RobustnessCertificate>,
    /// Epoch at which the certificate was sealed.
    pub epoch: u64,
    /// Per-invariant statistics.
    pub invariant_stats: Vec<InvariantStats>,

    // --- Artifact binding (hardening move #7) ---

    /// BLAKE3 hash of the final model weights. Binds certificate to
    /// the exact model artifact. Cannot be separated.
    pub weights_hash: [u8; 32],
    /// BLAKE3 hash of the VerifiedTrainerConfig (serialized).
    pub config_hash: [u8; 32],
    /// BLAKE3 hash of the dataset manifest (or RVF manifest root).
    /// None if no dataset manifest was provided.
    pub dataset_manifest_hash: Option<[u8; 32]>,
    /// BLAKE3 hash of the code (build hash / git commit).
    /// None if not provided.
    pub code_build_hash: Option<[u8; 32]>,
}

pub struct InvariantStats {
    /// Invariant name.
    pub name: String,
    /// Whether this invariant is formally proven or a statistical estimate.
    pub proof_class: ProofClass,
    /// Number of times checked.
    pub checks: u64,
    /// Number of times satisfied.
    pub satisfied: u64,
    /// Number of times overridden (degraded mode).
    pub overridden: u64,
    /// Average verification latency.
    pub avg_latency_ns: u64,
    /// Proof tier distribution: [reflex_count, standard_count, deep_count].
    pub tier_distribution: [u64; 3],
}

pub enum ProofClass {
    /// Formally proven: exact computation within the proof system.
    Formal,
    /// Statistical estimate with bound scope. Certificate records
    /// the estimation parameters (rng_seed, iterations, tolerance).
    Statistical {
        rng_seed: Option<u64>,
        iterations: usize,
        tolerance: f64,
    },
}

impl VerifiedTrainer {
    /// Seal the training run and produce a certificate.
    ///
    /// 1. Compacts the mutation ledger (proof-gated: compaction itself
    ///    produces a composed attestation + witness that the compacted
    ///    chain corresponds exactly to the original sequence).
    /// 2. Computes BLAKE3 hashes of weights, config, and optional manifests.
    /// 3. Composes all attestations into the final certificate.
    ///
    /// The sealed certificate is a product artifact: verifiable by
    /// third parties without trusting training logs.
    pub fn seal(self, weights: &Weights) -> TrainingCertificate;
}
```

### Performance Budget

The target is proof overhead < 5% of training step time. For a typical GNN training step of ~10 ms (on CPU):

- `LossMonotonicity` (Reflex): < 10 ns = 0.0001%
- `WeightNormBound` (Standard): < 1 us = 0.01%
- `LipschitzBound` (Standard): < 10 us = 0.1%
- `CoherenceBound` (Standard): < 5 us = 0.05%
- `PermutationEquivariance` (Deep, sampled): < 100 us = 1%
- Attestation composition: < 1 us = 0.01%
- **Total**: < 120 us = 1.2% (well within 5% budget)

For GPU-accelerated training (step time ~1 ms), `PermutationEquivariance` with many samples may exceed 5%. Mitigation: reduce sample count or check equivariance every N steps (configurable via `check_interval` in `VerifiedTrainerConfig`).

### Integration with EWC and Replay Buffer

The `VerifiedTrainer` composes with `ruvector-gnn`'s continual learning primitives:

```rust
pub struct VerifiedTrainerConfig {
    /// Optimizer type (from ruvector-gnn).
    pub optimizer: OptimizerType,
    /// EWC lambda (0.0 = disabled). Uses ruvector_gnn::ElasticWeightConsolidation.
    pub ewc_lambda: f64,
    /// Replay buffer size (0 = disabled). Uses ruvector_gnn::ReplayBuffer.
    pub replay_buffer_size: usize,
    /// Scheduler type (from ruvector-gnn).
    pub scheduler: SchedulerType,
    /// Invariants to verify per step.
    pub invariants: Vec<TrainingInvariant>,
    /// Check interval for expensive invariants (e.g., equivariance).
    /// Cheap invariants (Reflex tier) run every step.
    pub expensive_check_interval: usize,
    /// Warmup steps during which invariant violations are logged but
    /// do not trigger rollback. After warmup, fail-closed applies.
    pub warmup_steps: usize,
    /// Robustness certification config (None = disabled).
    pub robustness: Option<RobustnessCertifier>,
    /// Energy gate config (None = disabled).
    /// If enabled, energy is evaluated before every gradient application.
    pub energy_gate: Option<EnergyGateConfig>,
    /// Default rollback strategy for invariant failures.
    pub rollback_strategy: RollbackStrategy,
    /// Allow degraded mode: if true, failed invariant checks produce
    /// an OverrideProof and increment a visible violation counter
    /// instead of stopping the step. Default: false (fail-closed).
    pub allow_override: bool,
    /// Optional dataset manifest hash for binding to the certificate.
    pub dataset_manifest_hash: Option<[u8; 32]>,
    /// Optional code build hash for binding to the certificate.
    pub code_build_hash: Option<[u8; 32]>,
}
```

When EWC is enabled, the `WeightNormBound` invariant is automatically adjusted to account for the EWC penalty term. When the replay buffer is active, replayed samples also go through invariant verification.

## Consequences

### Positive

- Every training run produces a `TrainingCertificate` bound to the exact model weights via BLAKE3 hash — portable, verifiable by third parties without trusting logs
- Per-step invariant proofs catch regressions immediately — loss spikes, norm explosions, equivariance breaks become training-stopping events, not evaluation surprises
- Clear distinction between formally proven invariants and statistical estimates — the certificate is defensible because it states exactly what was proven and what was estimated
- EnergyGate integration makes training behave like inference gating — consistent proof-gated mutation across the full lifecycle
- Delta-apply rollback strategy avoids doubling peak memory while preserving proof-gated semantics
- Fail-closed by default with explicit OverrideProof for degraded mode — violations are visible, not silent

### Negative

- `PermutationEquivariance` is a statistical test, not a formal proof — the certificate is honest about this, but it means equivariance is not guaranteed, only tested with bound scope
- `LipschitzBound` via power iteration is an estimate — the certificate attests the estimate was within bounds, not the true Lipschitz constant
- The `TrainingCertificate` is only as strong as the invariants specified — missing invariants are not caught
- Robustness certification (IBP/DeepPoly) produces loose bounds for deep models; the certified radius may be conservative
- Over-conservative invariants can stop learning — mitigated by check intervals, warmup periods, and adaptive thresholds (which are themselves bounded)

### Risks

- **Proof cache hit rate drops**: High learning rate causes diverse weight states, Standard/Deep proofs dominate and exceed 5% budget. Mitigated by caching invariant structure (not values) — proof terms depend on structure, values are parameters. Monitor `ProofStats::cache_hit_rate` and alert below 80%
- **GPU steps dominated by Deep checks**: Schedule deep checks asynchronously with two-phase commit: provisional update, finalize after deep check, revert if failed. Mitigation preserves proof-gated semantics without blocking the training loop
- **EWC Fisher information**: O(n_params^2) in naive case. The existing diagonal approximation may miss cross-parameter interactions. Mitigated by periodic full Fisher computation (every K epochs) as a Deep-tier invariant
- **Attestation chain growth**: 82 bytes per step * 100,000 steps = 8 MB. Mitigated by `MutationLedger::compact` — compaction is itself proof-gated: it produces a composed attestation plus a witness that the compacted chain corresponds exactly to the original sequence under the current epoch algebra
- **Certificate separation**: Without artifact binding, the certificate can be detached from the model. Mitigated by BLAKE3 hashes of weights, config, dataset manifest, and code build hash in the certificate

### Acceptance Test

Train 200 steps with invariants enabled, then intentionally inject one bad gradient update that would push a layer norm above `max_norm`. The system must:
1. Reject the step (fail-closed)
2. Emit a refusal attestation to the ledger
3. Leave weights unchanged (delta-apply was not committed)
4. The sealed `TrainingCertificate` must show exactly one violation with the correct step index and invariant name
5. The `weights_hash` in the certificate must match the actual final weights

## Implementation

1. Define `TrainingInvariant` enum and `VerifiedTrainerConfig` in `crates/ruvector-graph-transformer/src/verified_training/invariants.rs`
2. Implement `VerifiedTrainer` wrapping `ruvector_gnn::training::Optimizer` in `crates/ruvector-graph-transformer/src/verified_training/pipeline.rs`
3. Implement invariant-to-ProofKind mapping for tier routing
4. Implement `RobustnessCertifier` with IBP and DeepPoly in `crates/ruvector-graph-transformer/src/verified_training/mod.rs`
5. Implement `TrainingCertificate` and `seal()` method
6. Add benchmarks: verified training step overhead on a 3-layer GNN (128-dim, 10K nodes)
7. Integration test: train a small GNN for 100 steps with all invariants, verify certificate

## References

- ADR-045: Lean-Agentic Integration (`ProofEnvironment`, `FastTermArena`)
- ADR-046: Graph Transformer Unified Architecture (module structure)
- ADR-047: Proof-Gated Mutation Protocol (`ProofGate<T>`, `MutationLedger`, `compose_chain`)
- `crates/ruvector-gnn/src/training.rs`: `Optimizer`, `OptimizerType`, `TrainConfig`, `sgd_step`
- `crates/ruvector-gnn/src/ewc.rs`: `ElasticWeightConsolidation`
- `crates/ruvector-gnn/src/scheduler.rs`: `LearningRateScheduler`, `SchedulerType`
- `crates/ruvector-gnn/src/replay.rs`: `ReplayBuffer`, `ReplayEntry`
- `crates/ruvector-verified/src/gated.rs`: `ProofTier`, `route_proof`, `verify_tiered`
- `crates/ruvector-verified/src/proof_store.rs`: `ProofAttestation`, `create_attestation`
- `crates/ruvector-verified/src/fast_arena.rs`: `FastTermArena`
- `crates/ruvector-coherence/src/spectral.rs`: `SpectralCoherenceScore`, `SpectralTracker`
- `crates/ruvector-mincut-gated-transformer/src/energy_gate.rs`: `EnergyGate`
- Gowal et al., "Scalable Verified Training" (ICML 2019) -- IBP training
- Singh et al., "Abstract Interpretation with DeepPoly" (POPL 2019)
