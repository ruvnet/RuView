# nvsim

**Deterministic Rust simulator for NV-diamond ensemble magnetometers.**
Synthesise the magnetic-field trace a real sensor *would have produced* —
without the hardware, the lab, or the $8 K vendor receipt.

---

## What this is, in one paragraph

NV-diamond magnetometers are exotic but real: they detect magnetic fields by
shining green laser at a diamond and watching how its red fluorescence shifts
under microwave excitation. They are sensitive enough to feel a person's
heartbeat from across a room — when they work. The catch: a working ensemble
sensor costs ~$8 K and lives in a lab. **`nvsim` runs the same forward
pipeline in software**, end-to-end, deterministically, so you can ask "what
would my magnetometer have seen if a steel rebar walked past it" without
wiring up any of it.

It is **not** a hardware-control stack, microscope simulator, full
Hamiltonian solver, or claim of fT-level sensitivity. This crate does not
control lasers, microwave sources, ADC hardware, or real NV sensors. It is
a deterministic Rust simulator with **explicit physics approximations and
no hidden mocks** — every formula is cited; every conjectural default is
flagged in code; every random number comes from a seeded ChaCha20 PRNG.

## Why you might use it

| If you are a… | …`nvsim` lets you… |
|---|---|
| **Sensor researcher** evaluating a new pipeline | Replay a synthetic trace through your own DSP and check it against a published-physics ground truth before buying hardware |
| **DSP / ML engineer** building anomaly detectors | Generate magnetic-anomaly traces with a known answer key — useful for regression replay, deterministic CI, and "did my detector regress?" gates |
| **Educator** teaching magnetometry / NV physics | Run real Biot-Savart, Lorentzian ODMR, and 4-axis projection in Rust without standing up a Python+QuTiP environment |
| **RuView pipeline contributor** | Get a binary `MagFrame` shape (`0xC51A_6E70`) you can plumb into existing observability, with optional ruvector trace compression behind a feature flag |
| **Auditor / compliance reviewer** | Re-run the included determinism check (`same scene + seed → byte-identical proof bundle`) and verify the simulator's output across machines without re-running the whole pipeline |

## Capabilities (what's shipping today)

| Capability | What's in the crate |
|---|---|
| **Scene primitives** | `DipoleSource`, `CurrentLoop`, `FerrousObject`, `EddyCurrent`, `Scene` aggregate. JSON round-trip safe. |
| **Magnetic-field synthesis** | Closed-form analytic dipole, numerical Biot-Savart over 64-segment current loops, linearly-induced ferrous-object moment, multi-source aggregation. **All in `f64`** for near-field stability; clamped at 1 mm with a saturation flag. |
| **Per-material attenuation** | Air / drywall / brick / dry concrete / reinforced concrete / sheet steel — with a `HEAVY_ATTENUATION` flag for the materials whose loss values are admittedly conjectural. **NaN-safe** on adversarial input (negative or non-finite path lengths). |
| **NV-ensemble physics** | ODMR Lorentzian (FWHM ≈ 1 MHz), shot-noise floor `δB ∝ 1/(γ_e·C·√(N·t·T₂*))`, T₂ decay envelope, 4-axis 〈111〉 crystallographic projection with closed-form LSQ inversion. Defaults match Barry et al. *Rev. Mod. Phys.* 92 (2020) Table III for COTS bulk diamond. |
| **Determinism** | Same `(B_in, dt, seed)` → byte-identical `NvReading`. ChaCha20-seeded shot noise; no global state, no time-of-day field, no allocator randomness. |
| **Binary frame format** | `MagFrame` — 60-byte fixed-layout record, magic `0xC51A_6E70` (distinct from ADR-018 CSI `0xC51F...` and ADR-084 sketch `0xC511_0084`). Round-trips byte-exact, deserialiser rejects bad magic / bad version / wrong length without panicking. |

### Not yet shipped (next two passes)

- `digitiser.rs` — ADC quantization + 4ᵗʰ-order Butterworth anti-alias + lockin demodulation
- `pipeline.rs` — wires every stage end-to-end and emits a `MagFrame` stream
- `proof.rs` + criterion bench — deterministic SHA-256 witness bundle + ≥ 1 kHz wall-clock throughput target

These complete the six-pass plan in
`docs/research/quantum-sensing/15-nvsim-implementation-plan.md`.

## How it compares

The closest existing tools each cover one slice of what `nvsim` covers
end-to-end. Nothing in the open-source ecosystem (as of early 2026) covers
the whole forward pipeline at once — see
`docs/research/quantum-sensing/14-nv-diamond-sensor-simulator.md` §2.2.

| Tool | Source synthesis | Material attenuation | NV ensemble physics | Digitiser + lockin | Witness bundle | Language |
|---|---|---|---|---|---|---|
| [Magpylib](https://magpylib.readthedocs.io/) | ✅ analytic dipole + Biot-Savart | ❌ | ❌ | ❌ | ❌ | Python |
| [QuTiP](https://qutip.org/) NV scripts | ❌ | ❌ | ✅ full Hamiltonian + Lindblad | ❌ | ❌ | Python |
| Vendor sims (Element Six, etc.) | partial | partial | ✅ proprietary | partial | ❌ | closed |
| **`nvsim`** | ✅ analytic + Biot-Savart | ✅ 6 materials, NaN-safe | ✅ leading-order ensemble proxy | 🚧 Pass 5 | 🚧 Pass 6 | Rust, deterministic |

`nvsim` deliberately **does not** try to compete with QuTiP on Hamiltonian
fidelity (full Lindblad solver is plan §6 out-of-scope). It picks the
linear-readout proxy that Barry 2020 §III.A validates as adequate for
ensemble magnetometers in the linear regime, and ships that path
end-to-end with witness-anchored reproducibility.

## Value proposition

You get **three things at once** that no other open simulator combines:

1. **Forward end-to-end pipeline.** Scene → source → propagation → NV → digitiser → frame → witness, in one crate, in one language. No Python ↔ Rust marshalling, no manual gluing of three half-tools.
2. **Strong determinism.** Same inputs and seed → byte-identical output across machines, runs, and time. CI pipelines treat the simulator's output as a content-addressable artifact: a SHA-256 over the frame stream is the build's "did the physics drift?" canary.
3. **Honest physics.** Every formula is cited. Every conjectural default is flagged in code, not buried in a footnote. The acceptance suite includes a Wolf 2015 sanity-floor test that fires if anyone silently changes the ensemble constants — i.e. the simulator can tell you when its own model breaks.

The cost: `nvsim` is a *forward simulator only*. It does not do inverse
problems (estimating field sources from sensor readings), full Hamiltonian
dynamics, or hardware control. If you need those, you escalate to QuTiP,
COMSOL, or a real lab respectively.

## Usage guide

### Install

```bash
# Inside the workspace:
cargo build -p nvsim --no-default-features
cargo test  -p nvsim --no-default-features      # currently 34 passing
```

`nvsim` is a standalone leaf crate. It depends only on `serde`, `thiserror`,
`tracing`, `rand`, and `rand_chacha`. RuView ecosystem integrations
(`wifi-densepose-core` frame alignment, `ruvector-core` trace compression)
land behind feature flags after the core simulator is shipping. None are
required to use this crate.

### Synthesize a scene's magnetic field at a sensor

```rust
use nvsim::{Scene, DipoleSource, scene_field_at};

let mut scene = Scene::new();
// 1 mA·m² dipole at (0,0,0.5 m) pointing along +ẑ
scene.add_dipole(DipoleSource::new([0.0, 0.0, 0.5], [0.0, 0.0, 1.0e-3]));

// Field at the origin
let (b_tesla, near_field_flag) = scene_field_at(&scene, [0.0, 0.0, 0.0]);
println!("B = {:?} T  (near-field saturated: {})", b_tesla, near_field_flag);
```

### Run the full sensor model

```rust
use nvsim::{NvSensor, NvSensorConfig};

let sensor = NvSensor::cots_defaults();
let b_in = [1.0e-9, 0.0, 0.0];   // 1 nT along +x̂
let dt = 1.0e-3;                  // 1 ms integration
let seed = 0xCAFE_BABE;

let reading = sensor.sample(b_in, dt, seed);
println!("recovered B = {:?}", reading.b_recovered);
println!("σ per axis  = {:?} T", reading.sigma_per_axis);
println!("δB floor    = {:e} T/√Hz", reading.noise_floor_t_sqrt_hz);
```

### Apply per-material attenuation

```rust
use nvsim::{attenuate, LosSegment, Material};

let b_in = [1.0e-9, 0.0, 0.0];
let segments = [
    LosSegment { material: Material::Air,         path_m: 1.0 },
    LosSegment { material: Material::Drywall,     path_m: 0.1 },
    LosSegment { material: Material::ReinforcedConcrete, path_m: 0.2 },  // raises HEAVY flag
];
let (b_attenuated, heavy) = attenuate(b_in, &segments);
```

### Serialise a binary frame

```rust
use nvsim::{MagFrame, MAG_FRAME_MAGIC};
use nvsim::frame::flag;

let mut f = MagFrame::empty(7);            // sensor_id 7
f.b_pt = [1500.0, -250.0, 800.0];          // pT
f.set_flag(flag::ADC_SATURATED);

let bytes = f.to_bytes();                   // 60 bytes, deterministic
let parsed = MagFrame::from_bytes(&bytes)
    .expect("round-trip must succeed");
assert_eq!(parsed, f);
```

## Acceptance commitments (per implementation plan §5)

These are the four numbers `nvsim` commits to as a finished simulator:

- **Pipeline throughput**: ≥ 1 kHz simulated samples per second of wall-clock on a Cortex-A53-class CPU.
- **Determinism**: same `(scene, seed)` produces byte-identical proof-bundle output across runs and machines.
- **Noise-floor reproduction**: simulator with shot noise OFF reproduces the analytical Biot-Savart result to ≤ 0.1% RMS.
- **Lockin SNR floor**: 1 nT @ 1 kHz vs 100 pT/√Hz floor → SNR ≥ 10 in 1 s.

The first and last numbers come online with Pass 5/6. The middle two are
already enforced in the test suite.

## Physics primary sources

- Jackson, *Classical Electrodynamics* 3e (1999), §5.4–5.8 — Biot–Savart, dipole field.
- Doherty et al., *Phys. Rep.* 528, 1 (2013) — NV ground-state Hamiltonian, ODMR transition.
- Barry et al., *Rev. Mod. Phys.* 92, 015004 (2020) — NV-ensemble sensitivity, Lorentzian lineshape, T₁/T₂/T₂*, contrast and spin-count defaults.
- Wolf et al., *Phys. Rev. X* 5, 041001 (2015) — bulk-diamond pT/√Hz reference floor used as the sanity-floor test boundary.
- Cullity & Graham, *Introduction to Magnetic Materials* 2e (2009), Ch. 2 — χ_steel for ferrous-object linear-induced moment.
- Ortner & Bandeira, *SoftwareX* 11, 100466 (2020) — Magpylib reference implementation for analytic dipole / current-loop fields.

For the full SOTA survey and the build/skip verdict, see
`docs/research/quantum-sensing/14-nv-diamond-sensor-simulator.md`. For the
six-pass implementation plan that drives the build, see
`docs/research/quantum-sensing/15-nvsim-implementation-plan.md`.

## Limitations and out-of-scope

Per `15-nvsim-implementation-plan.md` §6:

- Single-NV imaging / ODMR scanning microscopy — `nvsim` is room-scale, not nm.
- Full Lindblad solver, NV-NV entanglement, photonic-crystal cavities — escalate to QuTiP if needed.
- Diamond growth / NV creation chemistry — vendor (Element Six, Adamas) handles.
- Cryogenic operation — RuView ships room-temperature; `nvsim` follows.
- Real hardware control (laser drivers, microwave sources, AOM) — `nvsim` is forward-only.
- Pulsed dynamical-decoupling sequences — defer to dedicated tooling.
- fT-floor sensitivity claims — out of COTS reach in 2026; `nvsim` commits to a pT-floor honestly.
- Inverse problems — given sensor readings, the simulator does not estimate scene parameters back.

If your use case needs any of the above, `nvsim` is the wrong starting
point. If your use case is *forward simulation of a deterministic NV
magnetometer pipeline you can run in CI*, it is the right one.

## License

MIT OR Apache-2.0 (matches workspace default).
