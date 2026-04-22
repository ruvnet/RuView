---
Research Document ID: RD-C-09
Date: 2026-04-21
Status: Draft
Authors: RuView Research Team
Related ADRs: proposed ADR-084, ADR-085, ADR-086, ADR-087, ADR-088
---

# RD-C-09: Four-Layer CC-OS Architecture

## Abstract

This document integrates the substrate (02), circuit analysis (03),
neural dynamics runtime (04), cross-region attention fusion (05),
embodied simulator (06), behavioral-episode encoding (07), and
counterfactual perturbation (08) into a single four-layer architecture.
We specify the proposed new workspace crates, DDD bounded contexts,
domain events, data-flow diagram, control-plane CLI subcommands,
end-to-end latency and memory budgets, concurrency model, observability
conventions, and the integration points with existing RuView code. The
architecture is designed to slot into the existing `wifi-densepose` Rust
workspace without disturbing the publishing order or the
cargo-test-workspace convention.

---

## Table of Contents

1. Layer stack overview
2. Proposed new workspace crates
3. DDD bounded contexts
4. Data-flow diagram
5. Control plane — CLI subcommands
6. Latency budget
7. Memory budget
8. Concurrency model
9. Observability and witness logs
10. Integration with existing RuView modules
11. Build, test, and benchmark commands
12. Non-goals
13. References

---

## 1. Layer Stack Overview

```
┌──────────────────────────────────────────────────────────────────────┐
│  Layer 4:  Analysis & Adaptation                                     │
│  - ruvector-mincut fragility (03, 08)                                │
│  - NeuralFusionArray cross-region attention (05)                     │
│  - BehaviorPipeline CRV episode encoding (07)                        │
│  - CoherenceGate population state (05, 07)                           │
│  - Counterfactual perturbation runner (08)                           │
│  Crate: wifi-densepose-ruvector (extended)                           │
├──────────────────────────────────────────────────────────────────────┤
│  Layer 3:  Embodied Simulator (Closed Loop)                          │
│  - Rapier physics, fly body schema, sensor/actuator adapters (06)    │
│  - Optional NeuroMechFly bridge                                      │
│  Crate: wifi-densepose-embody (proposed)                             │
├──────────────────────────────────────────────────────────────────────┤
│  Layer 2:  Neural Dynamics Runtime                                   │
│  - Event-driven LIF kernel (04)                                      │
│  - ruvector-solver rate-code + perturbation (04 §5)                  │
│  - ruvector-temporal-tensor voltage/spike storage (02 §7, 04 §6)     │
│  Crate: wifi-densepose-neuro (proposed)                              │
├──────────────────────────────────────────────────────────────────────┤
│  Layer 1:  Connectome Graph Substrate                                │
│  - Neuron / Synapse / Region schema (02)                             │
│  - ruvector-mincut adjacency + triplets (02 §5)                      │
│  - Neuron embeddings (02 §6)                                         │
│  Crate: wifi-densepose-connectome (proposed)                         │
└──────────────────────────────────────────────────────────────────────┘
```

## 2. Proposed New Workspace Crates

| Crate | Slot | Depends on | Feature flags |
|-------|------|-----------|---------------|
| `wifi-densepose-connectome` | Layer 1 | `core`, `db`, `ruvector` (for mincut + temporal-tensor) | `flywire`, `micons`, `larva` |
| `wifi-densepose-neuro` | Layer 2 | `core`, `connectome`, `ruvector` | `rate-code`, `gpu` (deferred) |
| `wifi-densepose-embody` | Layer 3 | `core`, `neuro` | `nmf-bridge`, `vision` |

Publishing order (extends the list in `CLAUDE.md`):

```
... → wifi-densepose-ruvector
   → wifi-densepose-connectome   (new)
   → wifi-densepose-neuro        (new)
   → wifi-densepose-embody       (new)
   → wifi-densepose-train
   → wifi-densepose-mat
   ...
```

Layer 4 capabilities extend the existing `wifi-densepose-ruvector` crate
rather than spawning another crate: the aggregates
(`NeuralFusionArray`, `BehaviorPipeline`, perturbation runner) live
alongside `MultistaticArray` and `WifiCrvPipeline`.

## 3. DDD Bounded Contexts

| Context | Aggregate root | Invariants | Domain events |
|---------|----------------|------------|---------------|
| ConnectomeGraph | `ConnectomeGraph` | Synapses reference extant neurons; no duplicates; NT matches presynaptic neuron | `ConnectomeLoaded`, `SynapseAblated`, `SynapseRestored`, `EmbeddingRecomputed` |
| NeuralRuntime | `SimulationRun` | Seeded RNG; ordered event tiebreaks; append-only voltage/spike logs | `SpikeObserved`, `RunStarted`, `RunCompleted`, `RunAborted` |
| Body | `Body` + `BodyState` | Joint angles within physical limits; contact forces non-negative | `ContactOpened`, `ContactClosed`, `ActuatorSaturated`, `PhysicsDiverged` |
| RegionFusion | `NeuralFusionArray` | Embed dim consistent; coherence gate state monotonic within a window | `FusionEmitted`, `CoherenceGateTransitioned` |
| BehaviorEpisode | `BehaviorPipeline` (wraps `CrvSessionManager`) | Stage order (I→VI); bout id unique; coherence gate consistent | `BoutStarted`, `StageCompleted`, `BoutFinalized`, `CircuitIdentified` |
| Perturbation | `PerturbationRunner` | Every perturbation has baseline manifest; score computed from ≥N bouts | `PerturbationApplied`, `PerturbationReverted`, `FragilityScored` |

## 4. Data-Flow Diagram

```
 stimulus / ctx
      │
      ▼
[ SensorEncoder ] ─ spike injections ─▶ [ SpikeQueue ]
  (Layer 3)                                    │
                                               ▼
                                       [ LIF kernel ]
                                        (Layer 2)
                                               │
                              ┌────────────────┼───────────────┐
                              ▼                ▼               ▼
                     [ VoltageBuffer ]  [ SpikeLog ]  [ regionAggregator ]
                      (TemporalTensor)               (50 ms window)
                              │                              │
                              │                              ▼
                              │                 [ NeuralFusionArray ]
                              │                     (Layer 4 / SDPA)
                              │                              │
                              │                              ▼
                              │                [ BehaviorPipeline ]
                              │                     (CRV stages I–VI)
                              │                              │
                              │                              ▼
                              │               [ StageVI MinCut / fragility ]
                              │                              │
                              ▼                              ▼
                       [ MotorDecoder ] ◀─── (next-step command) ─── analysis products
                        (Layer 3)                                      │
                              │                                        ▼
                              ▼                                  witness bundle
                       [ Rapier physics ]                         (ADR-028 lineage)
                              │
                              └──── body state ─▶ SensorEncoder ...  (loop)
```

## 5. Control Plane — CLI Subcommands

Proposed additions to the `wifi-densepose` CLI binary:

| Subcommand | Purpose |
|------------|---------|
| `brain load <connectome.bin>` | Load a connectome into the store; emits `ConnectomeLoaded` with SHA-256 |
| `brain simulate --duration <s> --subgraph <selector> [--seed N]` | Run closed-loop simulation, produce witness bundle |
| `brain ablate <spec.json>` | Apply perturbation events; idempotent given the manifest |
| `brain fragility --baseline <manifest> --perturbation <spec>` | Compute $\mathcal{F}(P)$ from 08 |
| `brain replay <manifest.json>` | Deterministic replay; SHA-256 verification |
| `brain inspect <run-id>` | Pretty-print witness bundle, cuts, fragility |

## 6. Latency Budget

End-to-end for 100 Hz closed-loop control with 50k-neuron subgraph:

| Stage | Target | Worst-case |
|-------|--------|------------|
| Sensor read + encoding | 0.5 ms | 1 ms |
| Spike queue insert | 0.1 ms | 0.3 ms |
| LIF kernel (10 ms window) | 5 ms | 8 ms |
| Fusion (50 ms cadence, amortised) | 0.5 ms | 1.5 ms |
| CRV episode update (infrequent) | 1 ms | 3 ms |
| Motor decoding | 0.2 ms | 0.5 ms |
| Physics step (1 kHz) | 1 ms | 2 ms |
| **End-to-end** | **< 10 ms** | **< 16 ms** |

1 kHz aspirational target requires reduced subgraph (10k–15k), batched
fusion (every 10 ms), and no-alloc inner loops throughout.

## 7. Memory Budget

| Scale | Graph | Voltage (60s, tiered) | Spike log | Embeddings | Total |
|-------|-------|------------------------|-----------|------------|-------|
| 10k  | 10 MB | 400 MB | 50 MB | 5 MB | ~0.5 GB |
| 50k  | 48 MB | 3.0 GB | 240 MB | 26 MB | ~4 GB |
| 139k | 1.3 GB (synapses) | 8 GB | 700 MB | 71 MB | ~12 GB |

Laptop-grade (32 GB) handles 50k comfortably. Full FlyWire at 139k fits
in 32 GB but leaves little headroom; 64 GB recommended.

## 8. Concurrency Model

| Layer | Concurrency | Framework |
|-------|-------------|-----------|
| 1 (graph) | Immutable after load; readers lock-free | `arc-swap` or `parking_lot::RwLock` |
| 2 inner LIF | Single-threaded spike queue, rayon fan-out per time slot | `rayon` |
| 2 I/O | Async | `tokio` |
| 3 physics | Single-threaded tight loop | Rapier native |
| 3 ingress / egress | Async | `tokio` |
| 4 analysis | Async, read-only access | `tokio` |

Critical path (LIF + physics) is single-threaded per simulation
instance for determinism. Multiple instances (e.g. for
parallel-perturbation sweeps) run as independent processes.

## 9. Observability and Witness Logs

Every run, bout, and perturbation produces a manifest compatible with
the ADR-028 witness-bundle convention. A run's witness bundle includes:

- `connectome_sha256` (Layer 1)
- `run_manifest.json` (Layer 2)
- Voltage + spike compressed segments (Layer 2)
- `bout_manifest.json` per bout (Layer 3)
- `episode_manifest.json` per episode (Layer 4)
- `perturbation_manifest.json` per perturbation (Layer 4)
- `VERIFY.sh` self-verification script (ADR-028 style)

Bundle tarball: `dist/witness-bundle-CCOS-<run-id>-<sha>.tar.gz`.

## 10. Integration with Existing RuView Modules

| Existing module | Reused how |
|-----------------|-----------|
| `viewpoint/fusion.rs` `MultistaticArray` | Shape template for `NeuralFusionArray` |
| `viewpoint/coherence.rs` `CoherenceGate` | Reused verbatim with new coherence definition |
| `viewpoint/geometry.rs` `GeometricDiversityIndex` | Shape template for FDI |
| `crv/mod.rs` `WifiCrvPipeline` | Shape template for `BehaviorPipeline` |
| `signal/subcarrier.rs` virtual source/sink pattern | Reused for connectome min-cut |
| `signal/spectrogram.rs` attention-gated mincut | Reused for behavior-conditioned cuts |
| `mat/breathing.rs` `CompressedBreathingBuffer` | Pattern for `VoltageBuffer` |
| `mat/triangulation.rs` Neumann solver usage | Pattern for rate-code and perturbation solves |
| `scripts/generate-witness-bundle.sh` | Extended with CC-OS artifacts |
| ADR-028 witness convention | Applied verbatim |

## 11. Build, Test, and Benchmark Commands

```bash
# Workspace test (add new crates to the existing convention)
cd rust-port/wifi-densepose-rs
cargo test --workspace --no-default-features

# Targeted tests
cargo test -p wifi-densepose-connectome --no-default-features
cargo test -p wifi-densepose-neuro      --no-default-features
cargo test -p wifi-densepose-embody     --no-default-features

# Benchmarks (Criterion)
cargo bench -p wifi-densepose-neuro --bench lif_kernel_throughput
cargo bench -p wifi-densepose-neuro --bench voltage_buffer_compression

# End-to-end acceptance
cargo test -p wifi-densepose-embody --test grooming_acceptance

# Witness bundle generation (extended)
bash scripts/generate-witness-bundle.sh --ccos
```

## 12. Non-Goals

- **Real-time human-scale simulation.**
- **Cloud-distributed multi-node simulation in v1.** Single workstation.
- **GPU-accelerated LIF in v1.** Deferred to v2.
- **Live web visualisation in v1.** CLI + witness bundle only.
- **Replace the RF sensing pipeline.** CC-OS is additive.

## 13. References

1. Eric Evans, *Domain-Driven Design* (2003).
2. ADR-017 — RuVector signal + MAT integration (DDD pattern).
3. ADR-028 — ESP32 capability audit + witness verification.
4. RuVector v2.0.4 documentation.
5. `CLAUDE.md` — project configuration, crate publishing order.
6. Dorkenwald, S., et al. (2024). *Neuronal wiring diagram of an adult
   brain.* Nature.

---

**Next**: 10-acceptance-test-grooming.md — the concrete pass/fail test
that validates this architecture.
