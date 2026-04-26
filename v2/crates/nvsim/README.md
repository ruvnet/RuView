# nvsim

Deterministic Rust simulator for NV-diamond ensemble magnetometer pipelines.

`nvsim` models a forward-only magnetic sensing path:

```
scene
  → magnetic source synthesis
  → material attenuation
  → NV-ensemble response
  → digitisation
  → binary magnetic feature frames
  → deterministic SHA-256 witness
```

It is designed for ferrous-anomaly modeling, eddy-current sanity checks,
synthetic magnetic traces, sensor education, and regression testing.

It is **not** a hardware-control stack, microscope simulator, full Hamiltonian
solver, or claim of fT-level sensitivity. This crate does not control lasers,
microwave sources, ADC hardware, or real NV sensors.

Deterministic in the strong sense: a simulator with explicit physics
approximations, conjectural propagation defaults that are documented as
such, a linear NV-ensemble readout proxy validated by Barry et al.
*Rev. Mod. Phys.* 92, 015004 (2020) §III.A, and **no hidden mocks**.

## Quick start

```rust
use nvsim::scene::{Scene, DipoleSource};
use nvsim::frame::{MagFrame, MAG_FRAME_MAGIC};

let mut scene = Scene::new();
scene.add_dipole(DipoleSource::new([0.0, 0.0, 0.5], [0.0, 0.0, 1e-6]));
scene.add_sensor([0.0, 0.0, 0.0]);

// Pass 2+ adds source synthesis, propagation, sensor, digitiser, pipeline.
```

## Acceptance commitments (per implementation plan §5)

- **Pipeline throughput**: ≥ 1 kHz simulated samples per second of wall-clock on a Cortex-A53-class CPU.
- **Determinism**: same `(scene, seed)` produces byte-identical proof-bundle output across runs and machines.
- **Noise floor reproduction**: simulator with shot-noise OFF reproduces the analytical Biot–Savart result to ≤ 0.1% RMS.
- **Lockin SNR floor**: 1 nT @ 1 kHz vs 100 pT/√Hz floor → SNR ≥ 10 in 1 s.

Pass 1 (this build) ships only the scaffold + scene types + binary frame
shape; the four acceptance numbers come online over Passes 2–6 per the plan.

## Physics primary sources

- Jackson, *Classical Electrodynamics* 3e (1999), §5.4–5.8 — Biot–Savart, dipole field.
- Doherty et al., *Phys. Rep.* 528, 1 (2013) — NV ground-state Hamiltonian, ODMR transition.
- Barry et al., *Rev. Mod. Phys.* 92, 015004 (2020) — NV-ensemble sensitivity, Lorentzian lineshape.
- Wolf et al., *Phys. Rev. X* 5, 041001 (2015) — bulk-diamond pT/√Hz reference floor.
- Ortner & Bandeira, *SoftwareX* 11, 100466 (2020) — Magpylib reference implementation.

See `docs/research/quantum-sensing/14-nv-diamond-sensor-simulator.md` for context
and `15-nvsim-implementation-plan.md` for the build spec.

## Optional integrations

`nvsim` is a standalone leaf crate. RuView ecosystem integrations
(`wifi-densepose-core` frame alignment, `ruvector-core` trace compression,
etc.) land behind feature flags in follow-up passes once the core simulator
ships. None are required to use this crate.

## License

MIT OR Apache-2.0 (matches workspace default).
