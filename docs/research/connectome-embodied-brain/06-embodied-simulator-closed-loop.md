---
Research Document ID: RD-C-06
Date: 2026-04-21
Status: Draft
Authors: RuView Research Team
Related ADRs: proposed ADR-086
---

# RD-C-06: Embodied Simulator — Closed Sensorimotor Loop (Layer 3)

## Abstract

Without a body, connectome dynamics simulate a disembodied sensor
network; the behaviors we want to study — grooming, walking, feeding —
exist only when motor output reaches a physical body and sensory input
comes back as contact, proprioception, and optic flow. Layer 3 of CC-OS
is a closed-loop embodied simulator. We compare candidate platforms
(NeuroMechFly v1/v2, MuJoCo / MJX, Brax, PyBullet, Isaac Gym, Rapier),
recommend a **Rapier-native Rust inner loop with an optional
NeuroMechFly bridge** for biomechanical-validation runs, specify the
sensory encoders (bristle mechanosensors, Johnston's-organ
proprioception, optional ocellar luminance), the motor decoders
(descending MN rate → joint torque), the latency budget required to
sustain 100–1000 Hz control, and the witness-log schema that keeps every
closed-loop bout auditable.

---

## Table of Contents

1. Requirements
2. Platform comparison
3. Recommendation
4. Body schema
5. Sensory adapters
6. Motor adapters
7. Latency budget
8. Cross-process vs in-process bridges
9. Proposed crate: `wifi-densepose-embody`
10. Witness and provenance
11. Failure modes
12. Non-goals
13. References

---

## 1. Requirements

| Requirement | Target | Notes |
|-------------|--------|-------|
| Articulated body | Yes | 60–80 DoF for fly |
| Contact sensing | Yes | For grooming and substrate interaction |
| Proprioception | Yes | Joint angle + angular velocity |
| Vision (optional v1) | No | Adds ~1 ms per frame if simple |
| Physics rate | ≥ 1 kHz | To not starve LIF inner loop |
| Control rate | 100–1000 Hz | LIF fan-out cadence |
| Determinism | Yes (seeded) | Witness-log compatibility |
| Round-trip latency | < 10 ms @ 100 Hz control; < 1 ms aspirational | Tight neural-body coupling |

## 2. Platform Comparison

| Platform | Language | Body model | Physics | In-proc with Rust? | Pro | Con |
|----------|----------|-----------|---------|--------------------|-----|-----|
| NeuroMechFly v1 | Python + MuJoCo | Published fly | Rigid + contact | IPC only | Biomechanically validated | Python overhead, platform-specific |
| NeuroMechFly v2 | Python + MuJoCo-MJX | Published fly + vision | JAX | IPC only | Adds vision | Heavier stack |
| MuJoCo (bare) | C / Python | Author-your-own | Rigid + contact | FFI possible | Fast, reliable | Body must be authored |
| MJX | JAX | — | JAX-vectorised | Poor | Fast batched rollouts | Not for 1-instance realtime |
| Brax | JAX | — | Rigid | Poor | Fast, modern | Same |
| PyBullet | Python (wraps Bullet) | — | Rigid + contact | IPC | Easy | Medium fidelity |
| Isaac Gym | C++/Python | — | PhysX | Poor | Massive parallel RL | GPU-only, not auditable |
| **Rapier** | **Rust** | Authorable | Rigid + contact | **Yes, in-proc** | Native Rust, deterministic | No published fly body |

## 3. Recommendation

**v1 (internal)**: Rapier-native Rust inner loop with a hand-authored
minimal fly body. Prioritises determinism, latency, and staying inside
the Rust workspace.

**v1 (validation)**: NeuroMechFly bridge over shared memory or gRPC for
biomechanical cross-checks. The tight inner loop stays in Rust; periodic
validation runs use NeuroMechFly as ground truth.

**v2 (optional)**: MJX or Brax for parallel rollouts when exploring
perturbation sweeps.

## 4. Body Schema

Minimal fly body for v1: 6 legs × 5 joints + head (2) + abdomen (3) +
antennae (3 × 2) = 41 DoF. NeuroMechFly v2 goes to ~70 DoF including
detailed wing articulation; we skip wings in v1.

```text
FlyBody {
    joints:         Vec<Joint>,          // ~41
    links:          Vec<Link>,
    sensors:        Vec<Sensor>,         // contact, bristle, Johnston's
    actuators:      Vec<Actuator>,       // joint torque / position
    contact_model:  ContactModel,        // friction coefficients
}

Sensor ::= Bristle { link_id, threshold_n }
        |  Johnston { joint_id }         // antennal JO mechanosensor
        |  LegContact { link_id }
        |  Optic { link_id, fov }        // v2

Actuator ::= JointTorque { joint_id, max_nm }
          |  JointPosition { joint_id, pd_gains }
```

## 5. Sensory Adapters

Sensor → spike-input injector. Each physical sensor maps to a pool of
sensory neurons (02 §2) whose IDs come from the loaded connectome.

### 5.1 Bristle mechanoreceptors

```text
on bristle contact force f ≥ threshold:
    rate_hz = saturating(α · f, max_rate_hz)
    emit Poisson spike train into bristle-neuron pool
```

Maps to the ~1,300 FlyWire bristle neurons listed as sensory class.

### 5.2 Johnston's-organ proprioception

Antennal angle velocity drives a Poisson rate into JO pool. Pool size
~480 in FlyWire. Reference: Tuthill & Wilson 2016 (*Curr Biol*).

### 5.3 Leg contact

Binary contact → pulse train into corresponding leg contact pool (if
present in the subgraph; optional).

## 6. Motor Adapters

Descending motor neuron population rate → joint command.

### 6.1 Rate-coded torque

$$
  \tau_{\text{joint}} = g \cdot \sum_{i \in \text{MN pool}} (r_i - r_{\text{baseline}}),
$$

with $g$ a calibration gain and $r_{\text{baseline}}$ a per-pool tonic
rate. For antennal grooming, the prothoracic-leg MN pool maps onto leg
joint torques around the sweep arc.

### 6.2 Central pattern generators (CPG)

For locomotion (not the v1 acceptance target), Cruse 1990 Walknet
provides a phase-coupling model between legs that can be layered on top
of the motor adapters. Out of scope for v1 but worth noting as a
v2 extension.

## 7. Latency Budget

At 100 Hz closed-loop control:

| Stage | Budget | Notes |
|-------|--------|-------|
| Sensor read + encoding | 0.5 ms | per frame |
| Spike injection | 0.1 ms | into priority queue |
| LIF step | 5 ms | for 50k neurons (§ 04-11) scaled to 10 ms window |
| Region fusion | 0.5 ms | 80 regions × 128-d |
| CRV episode encoding | 1 ms | occasional |
| Motor decoding | 0.2 ms | per active pool |
| Physics step | 1 ms | at 1 kHz physics |
| Total | < 10 ms | target met |

For 1 kHz control (aspirational), every stage must shrink 10×. Feasible
with reduced-subgraph LIF, batched fusion, and careful no-alloc inner
loops.

## 8. Cross-Process vs In-Process Bridges

| Bridge | Overhead | Determinism | When |
|--------|----------|-------------|------|
| Rapier in-proc FFI | < 1 µs | Yes (seeded) | v1 tight loop |
| Shared memory (NeuroMechFly) | 20–50 µs | Needs coordination | Validation runs |
| gRPC localhost | 200–500 µs | Needs timestamps | Debugging / UI |
| TCP remote | 1–5 ms | Difficult | Not recommended |

## 9. Proposed Crate: `wifi-densepose-embody`

Location: `rust-port/wifi-densepose-rs/crates/wifi-densepose-embody/`

```text
src/
├── lib.rs
├── body.rs           Body schema, URDF-ish loader
├── sensors.rs        Encoders: bristle, JO, contact, optic
├── actuators.rs      Decoders: torque, PD, CPG stub
├── physics.rs        Rapier integration
├── bridge_nmf.rs     Optional NeuroMechFly bridge (feature-gated)
├── witness.rs        ADR-028 body-state bundle writer
└── config.rs
```

Dependencies:

```toml
wifi-densepose-core     = { path = "../wifi-densepose-core" }
wifi-densepose-neuro    = { path = "../wifi-densepose-neuro" }   # Layer 2
rapier3d                = "0.18"
serde                   = { workspace = true }
thiserror               = { workspace = true }
```

Feature flags:

```toml
[features]
default = []
nmf-bridge = ["dep:zmq"]   # optional
vision     = []             # v2
```

Publishing order: after `wifi-densepose-neuro`, before
`wifi-densepose-mat`.

## 10. Witness and Provenance

Every closed-loop bout produces:

```text
bout_manifest.json {
    bout_id:            "bout-0001",
    seed:               42,
    connectome_sha256:  "...",
    body_sha256:        "...",
    config_sha256:      "...",
    t_start_ms:         0,
    t_end_ms:           10000,
    frames_compressed:  "bout-0001-frames.ttseg",  // TemporalTensorCompressor
    voltage_ref:        "vbuf-0",
    spike_ref:          "splog-0",
    output_sha256:      "...",
}
```

Frames are body-state + sensor readings compressed via the same
TemporalTensorCompressor pattern (02 §7). Replaying the bout produces a
byte-identical SHA-256 if and only if the LIF runtime and the physics
are both deterministic under the seed.

## 11. Failure Modes

| Failure | Signature | Mitigation |
|---------|-----------|------------|
| Physics divergence | NaN joint angles | Clamp + halt + emit `PhysicsDiverged` event |
| Spike firehose | Queue length > threshold | Throttle or drop to PredictOnly coherence state (05) |
| Actuator saturation | Torque > max | Clip + warn |
| Sensor aliasing | Rate exceeds neuron pool refractory | Decimate encoding |
| Starved LIF | Simulation time lags wall-clock | Reduce control rate or subgraph size |
| Non-deterministic replay | SHA-256 mismatch | Re-examine RNG seeding and tiebreaks |

## 12. Non-Goals

- **Full wing + flight aerodynamics** — v2 target; requires air-model and
  much higher physics fidelity.
- **Fluid dynamics** (e.g. olfactory plume modelling) — external tool.
- **Photorealistic rendering** — not relevant; optical-flow only at
  most.
- **Multi-agent interactions** — single-organism v1.

## 13. References

1. Lobato-Rios, V., et al. (2022). *NeuroMechFly, a neuromechanical model
   of adult Drosophila melanogaster.* Nat Methods.
2. Wang-Chen, S., et al. (2024). *NeuroMechFly v2 with vision (preprint).*
3. Todorov, E., Erez, T., Tassa, Y. (2012). *MuJoCo: A physics engine for
   model-based control.* IROS.
4. Freeman, C. D., et al. (2021). *Brax — a differentiable physics
   engine.* NeurIPS workshop.
5. Cruse, H. (1990). *What mechanisms coordinate leg movement in walking
   arthropods?* Trends Neurosci.
6. Tuthill, J. C., Wilson, R. I. (2016). *Mechanosensation and adaptive
   motor control in insects.* Curr Biol.
7. Seeds, A. M., et al. (2014). *A suppression hierarchy among competing
   motor programs drives sequential grooming in Drosophila.* eLife.
8. Rapier physics engine docs — https://rapier.rs (reference only; no
   DOI).
9. ADR-028 — ESP32 capability audit + witness verification.

---

**Next**: 07-coherence-crv-behavioral-episodes.md — turning closed-loop
bouts into audited, queryable episodes.
