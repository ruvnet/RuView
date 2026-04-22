---
Research Document ID: RD-C-00
Date: 2026-04-21
Status: Draft
Authors: RuView Research Team
Related ADRs: ADR-001 through ADR-081 (existing corpus); proposed ADR-084 through ADR-088 (this compendium)
---

# RD-C-00: Connectome-Embodied-Brain Compendium — Index

## Abstract

This compendium evaluates whether RuVector v2.0.4 — the vector, graph, attention, solver, and CRV toolkit already integrated into the wifi-densepose Rust workspace — can serve as the substrate for a four-layer **coherence-aware connectome operating system (CC-OS)**. The design is inspired by the 2024 Lappalainen et al. connectome-constrained deep mechanistic networks for the Drosophila visual system, the Kakaria and de Bivort (2017) leaky-integrate-and-fire (LIF) central-complex model, and the NeuroMechFly v1/v2 embodied simulator lineage, but the RuView framing is deliberately narrower and more auditable than press coverage of "whole-fly-brain-in-silico" demonstrations. We do **not** claim mind upload, phenomenal consciousness, or biologically faithful cognition. We claim the ability to (a) ingest a published connectome as a typed graph, (b) run event-driven LIF dynamics at millisecond resolution, (c) close the loop through an embodied simulator, and (d) apply RuVector's structural circuit analysis (ruvector-mincut, ruvector-attn-mincut), counterfactual perturbation (via existing CoherenceGate / CrvSessionManager patterns), and witness-style auditing (ADR-028 lineage) to produce explainable behavioral episodes. The twelve documents that follow specify the substrate, runtime, simulator, analysis loop, risks, and acceptance tests. This index document is the entry point and positioning statement. Readers in a hurry should read §4 (positioning) and §5 (key findings preview) first.

## 1. Compendium Structure

The compendium is organized as twelve documents (00 through 11). Clusters A–D correspond to the four architectural layers; cluster E covers cross-cutting concerns.

| Doc | ID | Cluster | Owner focus | One-line description |
|-----|----|---------|-------------|----------------------|
| 00 | RD-C-00 | E: Meta | Index | This document — ToC, positioning, quick-start |
| 01 | RD-C-01 | E: Foundations | Literature | Published-literature survey: connectomes, LIF models, embodiment |
| 02 | RD-C-02 | A: Substrate | Layer 1 | Connectome graph schema, vector store, temporal-tensor storage |
| 03 | RD-C-03 | A: Substrate | Layer 1 | MinCut circuit analysis, structural fragility metric |
| 04 | RD-C-04 | B: Runtime | Layer 2 | Event-driven LIF neural dynamics runtime (`wifi-densepose-neuro`) |
| 05 | RD-C-05 | B: Runtime | Layer 2 | Cross-region attention fusion (`NeuralFusionArray`) |
| 06 | RD-C-06 | C: Embodiment | Layer 3 | Embodied simulator closed loop (`wifi-densepose-embody`) |
| 07 | RD-C-07 | D: Analysis | Layer 4 | Coherence + CRV behavioral episodes (`BehaviorPipeline`) |
| 08 | RD-C-08 | D: Analysis | Layer 4 | Counterfactual perturbation fragility protocol |
| 09 | RD-C-09 | E: Integration | Architecture | Four-layer architecture: data flow, interfaces, invariants |
| 10 | RD-C-10 | E: Acceptance | Testing | Acceptance test grooming — pass/fail criteria per layer |
| 11 | RD-C-11 | E: Governance | Roadmap | Risks, positioning, roadmap, proposed ADR-084..ADR-088 |

Documents 00, 01, and 02 are delivered in the first writing pass. Documents 03–11 are delivered by parallel clusters referenced by name throughout this index.

## 2. Key Crate → Layer Mapping

The CC-OS design reuses existing RuVector v2.0.4 crates rather than introducing new numerical kernels. Two new wifi-densepose crates are proposed (`wifi-densepose-neuro`, `wifi-densepose-embody`); the rest of the runtime is assembled from crates already in the workspace.

| Layer | Responsibility | RuVector crate(s) | wifi-densepose crate(s) |
|-------|----------------|-------------------|-------------------------|
| 1. Substrate | Connectome graph + per-neuron embeddings + temporal voltages | `ruvector-mincut`, `ruvector-attn-mincut`, `ruvector-temporal-tensor` | `wifi-densepose-core`, `wifi-densepose-db` |
| 2. Runtime | LIF dynamics, spike event bus, region attention | `ruvector-solver` (Neumann), `ruvector-attention` (SDPA) | **`wifi-densepose-neuro`** (proposed) |
| 3. Embodiment | Body simulator, sensors, actuators, closed loop | — (external: NeuroMechFly/MuJoCo/Rapier) | **`wifi-densepose-embody`** (proposed) |
| 4. Analysis | Mincut fragility, CRV episodes, coherence gating, counterfactuals | `ruvector-mincut`, `ruvector-attention`, `ruvector-crv`, `ruvector-gnn` (feature-gated) | `wifi-densepose-signal` (reuse RuvSense patterns) |

The RuvSense module set (signal/src/ruvsense/) is reused not by lifting WiFi-specific code but by lifting **patterns**: `CoherenceGate` (coherence_gate.rs) becomes a spike-train coherence gate; `MultistaticArray` (viewpoint/fusion.rs) becomes the `NeuralFusionArray` aggregate; `GeometricDiversityIndex` (viewpoint/geometry.rs) becomes a circuit-diversity index; `WifiCrvPipeline` (crv/mod.rs) becomes the `BehaviorPipeline` facade. Section 4 of doc 09 lists every pattern reuse explicitly.

## 3. Key Findings Preview

The full findings, with evidence, live in docs 01 through 11. The one-sentence previews:

| # | Finding | Evidence location |
|---|---------|-------------------|
| F1 | RuVector's `MinCutBuilder::new().exact()` can score connectome bottleneck fragility at 10k–150k node scale. | see doc 03 |
| F2 | `NeumannSolver` is appropriate for steady-state firing-rate approximation but not for event-driven spiking; event-driven LIF needs an explicit spike queue. | see doc 04 |
| F3 | `TemporalTensorCompressor` can store ~60 s of 1 kHz voltage traces for 50k neurons in roughly 3–4 GB using tiered 8/5–7/3-bit compression. | see doc 02 §8 |
| F4 | `ScaledDotProductAttention` is sufficient as a cross-region fusion primitive provided per-region embeddings are dimensionally consistent (32-d or 128-d). | see doc 05 |
| F5 | The six-stage `CrvSessionManager` pipeline maps 1:1 onto a behavioral-episode lifecycle (stimulus → sensory response → motor plan → action → audit → composite). | see doc 07 |
| F6 | Counterfactual "silence neuron X, rerun, measure divergence" is directly expressible as a perturbation on the spike event bus with a fragility score computed over the divergence embedding. | see doc 08 |
| F7 | End-to-end closed-loop real-time factor of 0.1×–0.5× on a single workstation is plausible for the full adult Drosophila FlyWire dataset (~139k neurons, ~54M synapses); realtime (1.0×) requires batching and GPU assistance. | see doc 09 |
| F8 | The design cleanly excludes phenomenal-consciousness claims; it is a structural / dynamical / behavioral audit tool, not a substrate for mind. | see doc 01 §8, doc 11 |
| F9 | ESP32 / WiFi-CSI hardware remains relevant only as an external sensor channel feeding layer-3 sensor inputs; it is not part of the neural substrate. | see doc 06 |
| F10 | Five new ADRs (ADR-084 substrate, ADR-085 runtime, ADR-086 embodiment, ADR-087 analysis loop, ADR-088 governance) are proposed to land this compendium. | see doc 11 |

## 4. Positioning: Coherence-Aware Connectome Operating System (CC-OS)

RuView's framing is **not** "digital mind" and not "whole-brain emulation in the Sandberg–Bostrom sense." The framing is a **coherence-aware connectome operating system**: a runtime that (i) loads a typed, provenance-tagged connectome graph; (ii) simulates its neural dynamics at millisecond resolution; (iii) closes the sensorimotor loop through an embodied simulator; and (iv) provides auditable, counterfactual analysis of the resulting behavior. The three key adjectives are:

- **Coherence-aware** — borrowed from the WiFi CSI pipeline: every layer produces a numerical coherence score with a `CoherenceGate` decision (Accept / PredictOnly / Reject / Recalibrate). This is not a claim about consciousness; it is a signal-quality gate.
- **Connectome** — the graph is sourced from published, peer-reviewed reconstructions (FlyWire, MICrONS, Winding et al. 2023). We do **not** generate connectomes; we ingest them.
- **Operating system** — the runtime exposes a scheduler, event bus, memory tiers, and auditable hooks. It is not a single monolithic model; it is a platform on which circuit analyses, behavioral episodes, and counterfactuals are scheduled and logged.

What this is **not**:

1. Not a mind, not a subject, not a locus of experience. Phenomenal consciousness is explicitly out of scope (doc 01 §8 elaborates, citing Seth 2021 and the Markov-blanket framing).
2. Not a whole-mammal brain emulator. At current compute, the feasibility frontier is adult Drosophila (~139k neurons) and perhaps larval zebrafish (~100k neurons). Mouse cortex (~75M neurons) is out of scope.
3. Not "Eon-style." Recent press coverage of "the fly brain running on a laptop" conflates three separable achievements — visual-system circuit fit (Lappalainen 2024), central-complex LIF dynamics (Kakaria 2017), and NeuroMechFly body loop — into a single headline. RuView keeps those three layers separate, with distinct acceptance criteria (doc 10), and does not make behavioral claims the layer does not support.
4. Not a replacement for NEURON, Brian2, Nengo, or Genesis. Those are biophysically richer. CC-OS prioritizes auditable graph-plus-LIF at connectome scale with built-in fragility analysis; it trades multi-compartment biophysics for throughput and governance.

## 5. Related ADRs

### 5.1 Existing ADR corpus referenced

- ADR-001 through ADR-043 — the existing wifi-densepose decision record corpus
- ADR-014 — SOTA signal processing (coherence primitives reused in layer 4)
- ADR-016 — RuVector training pipeline integration (precedent for ruvector adoption)
- ADR-024 — Contrastive CSI embedding / AETHER (precedent for per-node embedding design)
- ADR-027 — Cross-environment domain generalization / MERIDIAN (precedent for region fusion)
- ADR-028 — ESP32 capability audit + witness verification (precedent for audit protocol)
- ADR-029 through ADR-032 — RuvSense multistatic / coherence / security ADRs (pattern source)

References to ADR-044 through ADR-081 are understood to exist in the broader RuView corpus; specific numbers are not hard-coded here because the corpus continues to grow.

### 5.2 ADRs proposed by this compendium

Full text for each lives in doc 11. Previews:

| ADR | Title | Status | Doc of record |
|-----|-------|--------|---------------|
| ADR-084 | Connectome graph substrate + per-neuron embeddings | Proposed | RD-C-02, RD-C-03 |
| ADR-085 | Event-driven LIF neural runtime (`wifi-densepose-neuro`) | Proposed | RD-C-04, RD-C-05 |
| ADR-086 | Embodied simulator integration (`wifi-densepose-embody`) | Proposed | RD-C-06 |
| ADR-087 | Coherence + CRV + counterfactual analysis loop (`BehaviorPipeline`) | Proposed | RD-C-07, RD-C-08 |
| ADR-088 | Governance, positioning, and non-claims for CC-OS | Proposed | RD-C-11 |

## 6. Quick-Start Table of Contents

Read in this order for a 30-minute overview:

1. This document (RD-C-00) — §4 Positioning and §5 ADR previews.
2. [RD-C-01 Foundations and Connectome Neuroscience](./01-foundations-connectome-neuroscience.md) — §6 Scaling frontier table and §8 "Not mind upload" section.
3. [RD-C-09 Four-Layer Architecture](./09-four-layer-architecture.md) — the data-flow diagram.
4. [RD-C-11 Risks, Positioning, Roadmap](./11-risks-positioning-roadmap.md) — the non-claims and the milestone table.

Read in this order for a technical deep dive:

1. [RD-C-02 Connectome Graph Substrate](./02-connectome-graph-substrate.md)
2. [RD-C-03 MinCut Circuit Analysis](./03-mincut-circuit-analysis.md)
3. [RD-C-04 Neural Dynamics Runtime](./04-neural-dynamics-runtime.md)
4. [RD-C-05 Cross-Region Attention Fusion](./05-cross-region-attention-fusion.md)
5. [RD-C-06 Embodied Simulator Closed Loop](./06-embodied-simulator-closed-loop.md)
6. [RD-C-07 Coherence + CRV Behavioral Episodes](./07-coherence-crv-behavioral-episodes.md)
7. [RD-C-08 Counterfactual Perturbation](./08-counterfactual-perturbation.md)
8. [RD-C-10 Acceptance Test Grooming](./10-acceptance-test-grooming.md)

## 7. Document Conventions

- Every RD-C-* document uses the same front-matter block (ID, date, status, authors, related ADRs).
- Every claim cites author + year + venue. No DOIs are fabricated.
- Every proposed Rust API is tagged **(proposed)** and distinguished from observed v2.0.4 APIs.
- Every neural-quantity number has units and a sanity-check paragraph.
- Every non-claim (what CC-OS deliberately is not) is surfaced explicitly, not buried.

## 8. Next Steps

1. Land docs 00, 01, 02 (this writing pass).
2. Parallel clusters land docs 03–11.
3. Convert finding F1–F10 into the five proposed ADRs.
4. Validate the §3 feasibility claims against a small FlyWire-subset prototype on the `claude/connectome-embodied-brain-COE3I` branch.
5. Re-run the witness bundle (ADR-028 pattern) once prototype code exists.
