---
Research Document ID: RD-C-11
Date: 2026-04-21
Status: Draft
Authors: RuView Research Team
Related ADRs: proposed ADR-084 through ADR-088
---

# RD-C-11: Risks, Positioning, and Roadmap

## Abstract

This is the governance document for the compendium. It specifies what
CC-OS is allowed to claim in public artifacts, what it must never
claim, the dual-use and ethics considerations implied by running the
same counterfactual-perturbation pipeline on any connectome,
comparisons against prior art (Eon, OpenWorm, Blue Brain, Brian2 +
NeuroMechFly, BrainScaleS), the decision rubric from the originating
proposal, full drafts of five proposed ADRs (ADR-084 through ADR-088),
and a five-phase implementation roadmap with phase gates, success
KPIs, and out-of-scope items for v1. It closes with a publication plan
that prioritises honest scope-limited claims over headline-chasing.

---

## Table of Contents

1. Scientific positioning — CC-OS
2. Hype and ethics risk matrix
3. Scope limits
4. Security / dual-use considerations
5. Comparison with prior art
6. Decision rubric
7. Proposed ADRs (ADR-084 through ADR-088)
8. Five-phase implementation roadmap
9. Success KPIs per phase
10. Out-of-scope items for v1
11. Publication and talk plan
12. Appendix A: 200-word short-form summary
13. References

---

## 1. Scientific Positioning — CC-OS

**CC-OS is a coherence-aware connectome operating system.** It loads a
published connectome, simulates its neural dynamics at millisecond
resolution, closes the sensorimotor loop through an embodied simulator,
and provides auditable counterfactual analysis of the resulting
behavior. It is a platform for circuit science, not a model of mind.

What CC-OS **is**:

- a graph-native connectome runtime;
- a Rust event-driven LIF engine at 10k–139k neuron scale;
- an embodied sensorimotor loop closed in real time;
- a structural + behavioral fragility pipeline;
- an auditable, witness-logged, reproducible platform.

What CC-OS **is not**:

- not a mind, not a subject, not a locus of experience;
- not a whole-brain emulation in the Sandberg–Bostrom sense;
- not a replacement for NEURON, Brian2, Nengo, or Blue Brain;
- not a claim about consciousness, identity, or memory continuity.

## 2. Hype and Ethics Risk Matrix

| Claim | Risk | Mitigation |
|-------|------|------------|
| "Mind upload" / "digital consciousness" | VERY HIGH | Never use in any RuView artifact. Formal prohibition. |
| "Simulated fly brain" (unqualified) | HIGH | Use only with "structural + dynamical model of a subgraph of the adult Drosophila connectome, not a whole organism" qualifier. |
| "Fly running on a laptop" | MEDIUM | Acceptable if accompanied by scope caveat and acceptance-test reference. |
| "Coherence-aware connectome OS" | LOW | Preferred framing. |
| "Auditable circuit discovery" | LOW | Core value proposition; encouraged. |
| "Counterfactual circuit fragility scoring" | LOW | Novel, defensible, auditable. |
| "Digital fly companion / pet" | HIGH | Avoid anthropomorphic framing; simulator, not companion. |
| "Road to mammalian minds" | VERY HIGH | Out of scope; do not gesture at. |

Public posts, READMEs, and talk abstracts must pass a checklist that
rejects the VERY HIGH and HIGH items above by default.

## 3. Scope Limits

- **v1 organism**: adult *Drosophila melanogaster* only.
- **v2 candidates**: larval *Drosophila*, larval zebrafish (when
  proofread connectomes ship).
- **Out of scope**: mouse, rat, primate, human connectomes — pending
  external ethics review and a dedicated ADR.
- **Out of scope**: clinical / medical / patient-facing applications.
- **Out of scope**: consciousness or phenomenal-experience claims.
- **Out of scope**: real-time human-scale simulation.

## 4. Security / Dual-Use Considerations

The same pipeline that discovers behaviour-responsible circuits can be
used to design perturbations that *abolish* behavior. At fly scale this
is a research tool; at any biological-organism scale it is an
experimental design, not an action. The dual-use risk is structurally
mitigated by the witness-log property: every perturbation has a
manifest with an SHA-256 fingerprint, making post-hoc audit feasible
regardless of author intent. Guidelines:

- CC-OS runs shall emit witness bundles for any perturbation sweep.
- CC-OS shall not accept non-published connectomes (no BYO-connectome
  in v1 to prevent unaudited organism targeting).
- Perturbation recipes shall be version-pinned and hashed so that a
  published paper's experimental design is reproducible.

## 5. Comparison with Prior Art

| System | Scale | Lang | Graph-native | Coherence-gated | Counterfactual | Witness-audit | Open-source |
|--------|-------|------|--------------|-----------------|----------------|---------------|-------------|
| OpenWorm | 302 (C. elegans) | C++ / Python | Partial | No | No | No | Yes |
| NEURON | Arbitrary | C / Python | No (biophysics) | No | Manual | No | Yes |
| Brian2 | Arbitrary | Python | Partial | No | Manual | No | Yes |
| Nengo | Arbitrary | Python | No (NEF) | No | Manual | No | Yes |
| NeuroMechFly + Kakaria-LIF | ~10k | Python | Partial | No | No | No | Partial |
| Blue Brain Nexus | 10^6+ cortex | C++ / Python | Partial | No | No | Partial | No |
| BrainScaleS | 10^5 (hardware) | Hardware | No | No | Yes (hardware) | No | No |
| "Eon"-style stack | ~100k | Python | Partial | No | No | No | No |
| **CC-OS (this compendium)** | 10k–139k | **Rust** | **Yes** | **Yes** | **Yes** | **Yes (ADR-028)** | **Yes** |

The differentiation is not scale or biophysical fidelity; it is the
combination of graph-native storage + coherence gating + counterfactual
fragility + witness audit + Rust.

## 6. Decision Rubric

From the originating proposal (user quote, verbatim structure):

| Dimension | Verdict |
|-----------|---------|
| Feasibility today | HIGH |
| Novelty with RuVector | HIGH |
| Scientific validity if carefully positioned | MEDIUM–HIGH |
| Risk of hype if framed as "mind upload" | VERY HIGH |
| Best positioning | Embodied connectome simulation + coherence analysis |

We accept this rubric and commit to the "best positioning" in all
external artifacts.

## 7. Proposed ADRs

Five ADRs land this compendium in the project's decision record. ADR
numbers are reserved starting at **ADR-084** (the last existing ADR
before this compendium is **ADR-081**; ADR-082 and ADR-083 are reserved
for unrelated in-flight work).

Each ADR follows the existing RuView format: Status, Context,
Decision, Consequences, Links.

### 7.1 ADR-084 — Connectome Graph Substrate (Layer 1)

- **Status**: Proposed
- **Context**: CC-OS requires a typed, provenance-tagged,
  performance-adequate graph substrate for 10k–139k neuron connectomes
  with 1M–60M synapses. Existing `wifi-densepose-db` is row-store
  oriented; existing `wifi-densepose-ruvector` is signal/MAT focused.
  Neither covers the connectome use case directly.
- **Decision**: Introduce `wifi-densepose-connectome` crate implementing
  the `ConnectomeGraph` aggregate (02-connectome-graph-substrate.md
  §10), `Neuron` / `Synapse` / `Region` value objects, and adapters to
  `ruvector-mincut` edge triplets and `ruvector-temporal-tensor`
  per-neuron voltage buffers. FlyWire loader first; MICrONS and larval
  *Drosophila* loaders behind feature flags.
- **Consequences** (positive): graph-native circuit queries;
  provenance-tagged synapses; witness-log-compatible. (negative):
  adds a new crate to maintain; widens `NodeId` to `u64` in a new
  namespace. (neutral): publishing order changes.
- **Links**: RD-C-02, RD-C-03.

### 7.2 ADR-085 — Neural Dynamics Runtime (Layer 2)

- **Status**: Proposed
- **Context**: CC-OS requires an event-driven LIF runtime at 1 kHz
  real-time for 50k-neuron subgraphs, with deterministic replay and
  compressed voltage/spike storage.
- **Decision**: Introduce `wifi-densepose-neuro` crate
  (04-neural-dynamics-runtime.md §10). Uses `ruvector-solver` for
  rate-code and perturbation solves; `ruvector-temporal-tensor` for
  voltage and spike storage; `ruvector-attention` for motif queries.
  Inner loop single-threaded for determinism; rayon fan-out per time
  slot for throughput.
- **Consequences**: full cross-region LIF runtime available to the
  workspace; reuses RuVector patterns without duplication; introduces
  `rand_chacha` and `rayon` dependencies to a new crate.
- **Links**: RD-C-04, RD-C-05.

### 7.3 ADR-086 — Embodied Simulator Closed Loop (Layer 3)

- **Status**: Proposed
- **Context**: Without a body, circuit dynamics do not produce
  behavior. CC-OS needs a deterministic Rust-native inner-loop body
  simulator at 1 kHz physics / 100 Hz control, with an optional bridge
  to NeuroMechFly for biomechanical validation.
- **Decision**: Introduce `wifi-densepose-embody` crate using Rapier
  for physics and a hand-authored minimal fly body (41 DoF). Optional
  `nmf-bridge` feature for NeuroMechFly cross-validation.
- **Consequences**: tight in-proc Rust loop; no Python dependency on
  the critical path; validation story intact via optional bridge;
  `rapier3d` becomes a workspace dependency.
- **Links**: RD-C-06.

### 7.4 ADR-087 — CRV Behavioral Episodes + Coherence Gating (Layer 4)

- **Status**: Proposed
- **Context**: Behaviors are episodic. CC-OS needs a bout-level
  encoding that is reproducible, queryable, and integrates with the
  existing `ruvector-crv` six-stage protocol and `CoherenceGate`.
- **Decision**: Implement `BehaviorPipeline` inside the existing
  `wifi-densepose-ruvector` crate as a sibling of `WifiCrvPipeline`.
  Map CRV stages I–VI to behavior-class / neural-sensory-feature /
  body-pose-sketch / coherence-gate-state / circuit-query / min-cut
  respectively (07-coherence-crv-behavioral-episodes.md §2).
- **Consequences**: six-stage bout encoding for free via
  `CrvSessionManager`; Stage VI's MinCut directly yields
  behavior-responsible circuits; cross-subject convergence via
  `find_convergence` retargeted to `behavior_class`.
- **Links**: RD-C-07, RD-C-05.

### 7.5 ADR-088 — Governance, Positioning, and Counterfactual Protocol

- **Status**: Proposed
- **Context**: Running connectome dynamics with counterfactual
  perturbation invites both scientific mis-statement ("mind upload")
  and dual-use misuse. A governance ADR fixes the framing, the
  allowed-claim boundaries, and the perturbation-audit protocol.
- **Decision**:
  1. CC-OS is positioned as a "coherence-aware connectome operating
     system". The terms "mind upload", "digital consciousness", and
     their synonyms are prohibited in RuView public artifacts.
  2. Counterfactual perturbations (08-counterfactual-perturbation.md)
     must emit witness bundles compatible with the ADR-028 convention.
  3. Only published, peer-reviewed connectomes may be loaded in v1
     (FlyWire, MICrONS partial).
  4. Mammalian connectomes are out of scope for v1 without external
     ethics review.
- **Consequences**: clear framing for public communications; dual-use
  risk structurally mitigated by audit requirements; mammalian
  exploration is gated behind process, not policy.
- **Links**: RD-C-11 (this document), RD-C-08, RD-C-01.

## 8. Five-Phase Implementation Roadmap

| Phase | Weeks | Deliverables | Gates |
|-------|-------|--------------|-------|
| 1 — Connectome import | 1–4 | `wifi-densepose-connectome` crate; FlyWire loader; graph storage and query benchmarks | Load 139k-neuron FlyWire in < 10 s; k-hop query < 10 ms |
| 2 — LIF runtime | 5–10 | `wifi-densepose-neuro` crate; event-driven kernel; voltage + spike storage; deterministic replay | 1 kHz real-time for 10k-neuron subgraph; replay SHA-256 reproducible |
| 3 — Closed loop | 11–16 | `wifi-densepose-embody` crate; Rapier fly body; sensorimotor loop; 100 Hz control | Stable 100 Hz loop for 60 s without divergence |
| 4 — Grooming acceptance | 17–20 | All four criteria of 10-acceptance-test-grooming.md pass on CI | Acceptance test green |
| 5 — Fragility + convergence | 21–26 | `BehaviorPipeline` + `PerturbationRunner`; cross-subject convergence via `find_convergence`; first public witness bundle | Fragility correlates with published Hampel 2015 result; witness bundle self-verifies 7/7 |

Total: 26 weeks (6 months) from kickoff to first publishable witness
bundle.

## 9. Success KPIs per Phase

| Phase | Primary KPI | Secondary KPIs |
|-------|-------------|----------------|
| 1 | Load time, query latency | Memory footprint, loader coverage |
| 2 | Real-time factor | Replay reproducibility, compression ratio |
| 3 | Closed-loop stability duration | Physics step rate, actuator saturation rate |
| 4 | All four acceptance criteria green | CI wall-clock, flake rate |
| 5 | Behavioral fragility $\mathcal{F}$ distribution | Jaccard with published circuits, convergence score |

Each KPI has a target and a minimum; missing the minimum blocks
promotion to the next phase.

## 10. Out-of-Scope Items for v1 (Explicit)

- Mammalian connectomes (mouse cortex, larger).
- Consciousness or phenomenal-experience claims.
- Real-time human-scale simulation.
- GPU-accelerated LIF.
- Distributed multi-node simulation.
- Live web visualisation.
- Unsupervised behavior discovery.
- Plasticity in the inner loop.
- Wing aerodynamics.
- Photorealistic rendering or optic-flow vision beyond simple
  luminance.
- Companion or anthropomorphic framing.

## 11. Publication and Talk Plan

| Venue | Submission | Target claim | Artifact |
|-------|-----------|--------------|----------|
| NeurIPS workshop on neural connectomics | 2026 | Coherence-aware runtime + fragility pipeline | First witness bundle |
| eLife methods (or PLOS Comp Biol) | 2026 | CC-OS architecture + grooming reproduction | Peer-reviewed paper |
| RustConf | 2026 | Rust systems architecture + determinism | Live demo |
| Strange Loop | 2026 | Coherence-aware framing + dual-use ethics | Talk |
| bioRxiv preprint | Month 6 | Full methods | Accompanies ADRs |

No press-first releases. Every external communication follows the
positioning and avoids the VERY HIGH / HIGH risk items of §2.

## 12. Appendix A: 200-Word Short-Form Summary

> We built CC-OS, a **coherence-aware connectome operating system**, as
> a substrate for studying circuits that drive behavior in published
> insect connectomes. CC-OS is Rust-native, graph-first, and
> deterministic. It loads a peer-reviewed connectome (FlyWire is the
> v1 target at 139,255 neurons and 54M synapses), runs an event-driven
> LIF neural runtime at real-time rates for 50k-neuron subgraphs,
> closes the sensorimotor loop through a Rapier-based fly body, and
> provides auditable counterfactual perturbation with min-cut-based
> fragility scoring. Every run emits a witness bundle with SHA-256
> provenance that can be independently replayed. The first acceptance
> test reproduces the published fly antennal-grooming circuit (Hampel
> 2015) inside simulation, and shows the RuView min-cut identifies the
> same minimal sufficient circuit that optogenetic dissection did. CC-OS
> is **not** a mind, not a consciousness upload, not a replacement for
> NEURON or Brian2, and not a gesture at human-brain emulation. It is
> a platform for reproducible, auditable, graph-native connectome
> circuit science at insect scale.

## 13. References

1. Sandberg, A., Bostrom, N. (2008). *Whole Brain Emulation: A
   Roadmap.* Future of Humanity Institute, Oxford.
2. Seth, A. (2021). *Being You: A New Science of Consciousness.*
   Faber.
3. Friston, K. (2013). *Life as we know it.* J. R. Soc. Interface.
4. ADR-028 — ESP32 capability audit + witness verification.
5. ADR-017 — RuVector signal + MAT integration.
6. ADR-011 — Python proof of reality.
7. Dorkenwald, S., et al. (2024). *Neuronal wiring diagram of an adult
   brain.* Nature.
8. Hampel, S., et al. (2015). *A neural command circuit for grooming
   movement control.* eLife.
9. Seeds, A. M., et al. (2014). *A suppression hierarchy among
   competing motor programs drives sequential grooming in
   Drosophila.* eLife.
10. Lappalainen, J. K., et al. (2024). *Connectome-constrained networks
    predict neural activity across the fly visual system.* Nature.
11. Kakaria, K. S., de Bivort, B. L. (2017). *Ring attractor dynamics
    emerge from a spiking model of the entire protocerebral bridge.*
    Front Behav Neurosci.

---

**End of compendium.** Return to [00-index.md](./00-index.md) for the
table of contents.
