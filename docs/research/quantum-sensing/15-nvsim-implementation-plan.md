# NV-Diamond Sensor Simulator — Implementation Plan

## Quantum Sensing Series (15/—) — Executable Build Spec

**Date**: 2026-04-25
**Status**: Plan only — no source code yet
**Branch**: `feat/nvsim-pipeline-simulator` (untracked artefact)
**Companion**: `14-nv-diamond-sensor-simulator.md` (SOTA + verdict + scope caveats)
**Drives**: `/loop` — six independently shippable passes, one module per iteration

Working document. A developer (human or agent) picks up any single row of §3, ships
it, runs the gate, stops. Doc 14's verdict was "lean toward skip without a hardware
target"; this plan honours that scoping by sizing narrowly to ferrous-anomaly /
eddy-current / `mat`-aligned use cases. Where physics has a primary source, formula is
cited; where it does not, the gap is marked **conjecture** with a defensible default.

---

## Section 1 — Crate scaffold

### 1.1 Crate name — locked: **`nvsim`**

Standalone, *not* prefixed with `wifi-densepose-`: the simulator is generally useful
outside RuView's WiFi-CSI context (magnetic-anomaly modeling, NV-physics teaching,
COTS-sensor noise-floor sanity checks), so it lives in the workspace as a peer leaf.
Public API: `use nvsim::scene::DipoleSource;`. Placement: `v2/crates/nvsim/`, pure leaf
crate (no internal RuView deps).

### 1.2 Cargo.toml

```toml
[package]
name = "nvsim"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Deterministic NV-diamond magnetometer pipeline simulator (source -> propagation -> NV -> ADC)"

[dependencies]
ndarray = { workspace = true }                 # 3-vector field math, time-series buffers
rustfft = { workspace = true }                 # spectral analysis + lockin demod cross-check
num-complex = { workspace = true }             # phasor algebra in lockin
num-traits = { workspace = true }
rand = "0.8"                                   # Monte-Carlo shot noise (NOT in workspace yet -> add)
rand_chacha = "0.3"                            # deterministic seed -> ChaCha20 PRNG
sha2 = "0.10"                                  # witness hashing (already used in -core)
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
wifi-densepose-core = { path = "../wifi-densepose-core" }   # FrameKind extension only

[dev-dependencies]
criterion = "0.5"
approx = "0.5"

[features]
default = []
ruvector = ["dep:ruvector-core"]               # optional witness/sketch reuse — Section 4
[dependencies.ruvector-core]
path = "../../../vendor/ruvector/crates/ruvector-core"
optional = true

[[bench]]
name = "pipeline_throughput"
harness = false
```

### 1.3 Module layout (one file each, < 500 lines per CLAUDE.md)

| File | LoC budget | Purpose |
|---|---|---|
| `src/lib.rs` | < 200 | Public re-exports, `Pipeline` builder, error type, crate-level rustdoc |
| `src/scene.rs` | < 350 | `DipoleSource`, `CurrentLoop`, `FerrousObject`, `EddyCurrent`, `Scene` aggregate |
| `src/source.rs` | < 350 | Biot–Savart for current loops + analytic dipole field (no FEM) |
| `src/propagation.rs` | < 250 | Per-material attenuation table + free-space pass-through |
| `src/sensor.rs` | < 450 | NV-ensemble linear ODMR readout, Lorentzian lineshape, T1/T2 envelope, shot noise, vector projection onto 4 NV axes |
| `src/digitiser.rs` | < 300 | ADC quantize, anti-alias, lockin demod at MW modulation freq |
| `src/pipeline.rs` | < 250 | Wires the four layers; emits `MagFrame` stream |
| `src/frame.rs` | < 250 | `rv_mag_feature_state_t` struct, magic-number, byte-exact serialisation |
| `src/proof.rs` | < 250 | Deterministic seed -> SHA-256 witness; mirrors `archive/v1/data/proof/verify.py` |

Total: ~2,650 LoC Rust + ~400 LoC tests + 1 bench. 3-week sprint per doc 14 §5.

### 1.4 Frame magic number

ADR-018 reserves `0xC51F...` for CSI. Pick **`0xC51A_6E70`** for `rv_mag_feature_state_t`:
`C51` (CSI/feature lineage), `A` (Analog/Anomaly), `6E70` (ASCII "np", NV-pipeline).
u32 little-endian, first 4 bytes of every frame. Consumers reading `0xC51F...` fail
magic-check on a magsim frame and abort cleanly — non-overlap with CSI is the invariant.

### 1.5 Workspace wiring

Append `crates/nvsim` to `v2/Cargo.toml` members after `wifi-densepose-vitals`. No
publishing-order changes (pure leaf, no internal deps). Update CLAUDE.md crate table
in a separate PR after Pass 6 ships.

---

## Section 2 — Physics-model commitments (no-mocks part)

Per layer: formula, units, primary source. When no primary source applies at RuView
geometry, marked **conjecture** with chosen default.

### 2.1 `source.rs` — magnetic source synthesis

| Primitive | Formula | Units | Source |
|---|---|---|---|
| Magnetic dipole | `B(r) = (μ₀ / 4π r³) · [3(m·r̂)r̂ − m]` with `μ₀ = 4π×10⁻⁷ T·m/A` | T (output), m (position), A·m² (moment) | Jackson, *Classical Electrodynamics* 3e, §5.6 (1999); Magpylib reference impl [Ortner & Bandeira, SoftwareX 11, 100466 (2020)] |
| Current loop | Biot–Savart: `B(r) = (μ₀/4π) ∮ I dl × r̂ / r²` discretised over n=64 segments | T | Jackson §5.4 |
| Ferrous-object induced moment | Linear approx: `m_induced = χ V H_ambient` for χ ≈ 5000 (steel) | A·m² | Cullity & Graham, *Introduction to Magnetic Materials* 2e (2009), Ch.2 — primary source for steel χ at low field |
| Eddy-current loop | Faraday + Ohm: `I(t) = -(σ A / L) · dΦ/dt`, then re-emits via Biot–Savart | A | Jackson §5.18; **no primary source** for arbitrary geometry — conjecture: assume thin-disc geometry, scalar L per object |

Sign convention: right-hand rule on current; `m` parallel to coil normal. Units: SI;
convert to pT at frame-emit time only. Singularity at r→0: clamp `r_min = 1 mm`; below
that, return `B = 0` and set `flags |= SATURATION_NEAR_FIELD` (conjectural — no
published guidance for sub-mm dipole at RuView geometry — but deterministic).

### 2.2 `propagation.rs` — attenuation through air + materials

| Material | Model / coeff (DC–10 kHz) | Source |
|---|---|---|
| Air / vacuum | μ = μ₀, σ ≈ 0; 0 dB/m | Jackson §5.8 |
| Drywall (gypsum) | Dielectric, 0 dB/m | **Conjecture** (no primary source); gypsum non-ferromagnetic, loss << 0.1 dB/m |
| Brick (dry) | Dielectric, 0 dB/m | **Conjecture**; same logic |
| Concrete (dry) | 0.5 dB/m default | **Conjecture** (Ulrich *NDT&E Int.* 35, 2002 as proxy only) |
| Reinforced concrete | 20 dB/m + warning flag | Ulrich 2002 proxy; **research gap** per doc 14 §6.3 |
| Sheet steel | Skin depth `δ = √(2/μσω)`, freq-dependent | Jackson §8.1 |

Propagation is intentionally thin: free-space 1/r³ lives in `source.rs`. This layer
applies per-segment attenuation only when sensor-source line-of-sight intersects a
material slab; default is identity.

### 2.3 `sensor.rs` — NV-ensemble response

Full Hamiltonian is *not* solved (doc 14 §4.4 defers Lindblad dynamics to QuTiP). We
implement the linear-readout proxy that Barry 2020 §III.A validates as adequate for
ensemble magnetometers in the linear regime:

| Quantity | Formula / value | Source |
|---|---|---|
| ODMR transition | `ν± = D ± γ_e |B_∥|`; `D = 2.87 GHz`, `γ_e = 28 GHz/T` | Doherty *Phys. Rep.* 528 (2013) §3 |
| Lineshape | Lorentzian, `Γ ≈ 1 MHz` FWHM | Barry *RMP* 92 (2020), Fig. 4 |
| Shot-noise δB | `1 / (γ_e · C · √(N · t))` (leading order) | Barry 2020 Eq. 35; Taylor *Nat. Phys.* 4 (2008) |
| C (ODMR contrast) | 0.03 (COTS bulk) | Barry 2020 Table III |
| N (sensing spins) | 10¹² for ~1 mm³ | Barry 2020 §IV.A |
| T1 / T2 / T2* | 5 ms / 1 µs / 200 ns | Jarmola *PRL* 108 (2012); Barry 2020 Table III |
| Vector projection | 4 NV axes [111], [11̄1̄], [1̄11̄], [1̄1̄1] | Doherty 2013 §3 |

Layer takes `B_field: [f64; 3]` from propagation, projects onto each of 4 axes, applies
Lorentzian response at f_mod, scales by bandwidth-integrated noise `δB · √(BW)`, then
returns 3-vector via least-squares inversion of the 4-axis projection matrix.

Sanity floor derived from above (must hold in tests): `δB(t=1s, BW=1Hz) ≈ 1.2 pT/√Hz`,
within 4× of Wolf 2015's 0.9 pT/√Hz — acceptable analytic-model approximation given
ODMR-CW operation (Wolf used flux concentrators).

### 2.4 `digitiser.rs` — ADC + lockin demod

| Step | Model / default | Source |
|---|---|---|
| Anti-alias | 4th-order Butterworth, `f_c = f_s/2.5` | Oppenheim & Schafer 3e §7 |
| Sampling | `f_s = 10 kHz`, jitter 100 ns RMS | **Conjecture** — DNV-B1 1 kHz × 10 headroom |
| Quantisation | 16-bit signed, ±10 µT FS, LSB ≈ 305 pT | DNV-B1 datasheet (proxy) |
| Lockin demod | `y = LP[x·cos(2π f_mod t)]`, BW = f_s/1000, f_mod = 1 kHz | SR830 app note + standard DSP |
| Output | 3-axis B in pT, per-axis σ estimate | — |

Lockin is the final SNR-determining stage; Pass 5 pins it empirically.

---

## Section 3 — Six-pass implementation plan

Each pass is one `/loop` iteration — independently shippable. Gate must pass before
next pass begins; if not, abort and replan (§7).

| Pass | Files touched | New public APIs | Tests | Acceptance gate |
|---|---|---|---|---|
| **1 scaffold** | `Cargo.toml`, `lib.rs`, `scene.rs`, `frame.rs`, `v2/Cargo.toml` | `Scene`, `DipoleSource`, `CurrentLoop`, `FerrousObject`, `MagFrame`, `MAG_FRAME_MAGIC` | 6: scene JSON round-trip; magic = `0xC51A_6E70`; frame byte order deterministic; serde compiles; empty scene serializes; LoC budget enforced | `cargo check -p nvsim` clean; 6/6 pass; workspace 1,575+6 = 1,581 |
| **2 Biot–Savart** | `source.rs` | `Scene::field_at(point) -> [f64;3]` | 5: on-axis dipole `B = μ₀m/(2π z³)`; equatorial `B = -μ₀m/(4π r³)`; n=8 RMS ≤ 0.5%; loop on-axis `B_z = μ₀ I a²/[2(a²+z²)^{3/2}]`; r→0 clamp = 0+flag | n=8 ≤ 0.5%; else **abort §7-1** |
| **3 propagation** | `propagation.rs`, `lib.rs` | `Propagator::attenuate(B, los_segments) -> [f64;3]` | 4: free-space identity; drywall ≈ 0 dB; concrete 0.5 dB/m; rebar warns + 20 dB/m; NaN-safe on zero LoS | All 4 pass; no NaN any input |
| **4 NV sensor** | `sensor.rs` | `NvSensor::sample(B_in, dt) -> NvReading` | 6: FWHM = 1.0 ± 0.05 MHz; shot noise ∝ 1/√t over 5 decades; T2 envelope = exp(−t/T2); 4-axis LSQ residual < 1%; zero-in + noise-on = zero-mean; floor at 1 µT bias matches Barry 2020 within 2× | Floor match ≤ 2×; else **abort §7-2** |
| **5 digitiser+pipeline** | `digitiser.rs`, `pipeline.rs` | `Pipeline::new(scene,config).run(n) -> Vec<MagFrame>`; `Lockin::demod` | 5: `(scene, seed=42)` → SHA-256 witness; same seed = byte-identical; 1 nT @ 1 kHz vs 1 nT/√Hz floor → SNR ≥ 10 in 1 s; ADC saturates + flags above ±10 µT; anti-alias ≥ 40 dB at f_s/2+1 Hz | All 5 pass; SNR floor met |
| **6 proof+bench** | `proof.rs`, `benches/pipeline_throughput.rs`, `lib.rs` docs | `Proof::generate()`, `Proof::verify(expected_hash)` | 5: bundle reproduces published `expected_mag_features.sha256`; x86_64+aarch64 cross-platform OK; criterion ≥ 1 kHz dev; doc 14 xrefs resolve; workspace ≈ 1,606 | Bench ≥ 1 kHz dev AND ≥ 1 kHz Cortex-A53 (instr-count proxy); else **abort §7-3** |

Cumulative test budget: 6+5+4+6+5+5 = **31 new tests**, raising workspace from 1,575
to ~1,606. Branch hygiene: every pass commits to `feat/nvsim-pipeline-simulator`,
subject ends in `[nvsim:passN]`; no merge to `main` until all six gates pass.

---

## Section 4 — ruvector integration points

Doc 14 §4.6 did *not* mandate ruvector. Survey of legitimate uses with honest no-fit
calls:

| ruvector primitive | Use in nvsim | Decision |
|---|---|---|
| `sha2` (already in workspace) | Hash time-series in `proof.rs` | **Use direct `sha2` dep** — not via ruvector |
| `BinaryQuantized` 32× | Long-form trace storage for regression replay (1 h × 10 kHz: 432 MB f32 → 13.5 MB binary) | **Use behind `features = ["ruvector"]`** opt-in |
| HNSW sketch | Content-address scenes | **Skip** — SHA-256 of canonical JSON suffices |
| `ruvector-attention` / `mincut` | — | **Skip** — inference primitives; nvsim is forward-only |
| `quantization` for ADC | Reuse Q_int4 | **Reject as misuse** — vector compression, not signal-path ADC. Implement directly. |

Net: optional `ruvector` feature flag enables trace compression in `proof.rs` only.
Default build and witness verification do not depend on ruvector — matches the
"leverage where it helps but don't force it" guidance.

---

## Section 5 — Acceptance numbers the simulator commits to

Verbatim, measurable, non-aspirational.

- **Pipeline throughput**: ≥ 1 kHz simulated samples per second of wall-clock on a Cortex-A53-class CPU (Pi Zero 2W).
- **Determinism**: same `(scene, seed)` produces byte-identical proof-bundle output across runs and machines.
- **Noise floor reproduction**: simulator with shot noise OFF must reproduce the analytical Biot–Savart result to ≤ 0.1% RMS error.
- **Lockin SNR floor**: with a 1 nT signal at 1 kHz against a 100 pT/√Hz noise floor, lockin demod recovers SNR ≥ 10 in 1 s integration.

All four are Pass-6 acceptance tests or bench assertions. Determinism uses fixed-seed
ChaCha20 + canonical f64 serialisation order.

---

## Section 6 — Out of scope (committed to NOT building)

Explicit non-goals. Ruling them out is half the value of the plan.

| Excluded | Reason |
|---|---|
| Single-NV imaging / ODMR scanning microscopy | Room-scale, not nm; doc 14 §4.7 |
| NV-NV entanglement, photonic-crystal cavities | Out of RuView hardware budget |
| Diamond growth / NV creation chemistry | Vendor (Element Six) handles |
| Cryogenic operation | RuView ships RT; doc 14 §2.2 |
| Real hardware control (laser, MW, AOM) | Simulator is forward-only |
| Full Hamiltonian + Lindblad solver | Defer to QuTiP if ever needed; doc 14 §3.1 |
| Pulsed dynamical-decoupling sequence design | Hardware-firmware concern; doc 14 §4.7 |
| fT-floor sensitivity | Out of COTS reach 2026; simulator commits to pT-floor |
| CSI+MAG paired training data | No ground-truth pairs exist; doc 14 §5 |
| Network transport / live ingestion | Defer to `wifi-densepose-api` |

---

## Section 7 — Risk register and abort conditions

Three risks ordered by largest uncaught-downside payoff. Each has a concrete
iteration-level abort. If abort fires, loop halts; replan required.

| # | Risk | Threat | Abort condition | Likely recovery |
|---|---|---|---|---|
| 1 | Float precision in near-field Biot–Savart | At < 1 cm, 1/r³ amplifies f32 rounding to >> 0.5%; Pass 2's n=8 analytic test fails | Pass 2 cannot achieve ≤ 0.5% RMS even after promoting all math to f64 and clamping r_min = 1 mm | Add small-r Taylor expansion guard (unspecified physics — escalate) |
| 2 | NV shot-noise model mis-cited | §2.3 is leading-order; if 1 µT-bias floor differs from Barry 2020 Fig. 8 by > 2×, the simulator is making claims its model cannot back | Pass 4 noise-floor test fails 2× tolerance at 1 µT | (a) include strain-broadening term, or (b) downgrade Section 5 lockin-SNR commitment — escalate |
| 3 | Pipeline throughput < 1 kHz wall-clock | Per-sample cost dominated by Pass 4 LSQ inversion + Pass 5 lockin convolution; on Cortex-A53 (4–6× slower) sub-1 kHz orphans deployability | Pass 6 criterion bench < 1 kHz on x86_64 dev hardware | (a) cache pseudo-inverse, (b) IIR lockin, (c) drop f_s to 1 kHz and restate §5 — no auto-merge |

---

## Section 8 — How `/loop` consumes this plan

`/loop` reads §3, picks the next un-shipped row, ships exactly that pass: (1) read row;
(2) verify previous gate PASS via `git log --grep '\[nvsim:passN-1\]'`; (3) implement
only the row's "Files touched"; (4) run row tests + `cargo test --workspace --no-default-features`; (5) commit, subject ends `[nvsim:passN]`; (6) stop. Test failure: no commit. §7
abort fires: halt loop, surface to user.

---

*Entry point for `/loop` on `nvsim`. Does not commit to building — that decision lives
in doc 14's verdict ("lean toward skip" absent hardware target). If the verdict flips,
this is the plan that ships.*
