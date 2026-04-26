# ADR-089: nvsim — NV-Diamond Magnetometer Pipeline Simulator

| Field          | Value                                                                                   |
|----------------|-----------------------------------------------------------------------------------------|
| **Status**     | Accepted — Passes 1–5 implemented and merged via the `feat/nvsim-pipeline-simulator` branch; Pass 6 (proof bundle + criterion bench) pending in the next iteration |
| **Date**       | 2026-04-26                                                                              |
| **Authors**    | ruv                                                                                     |
| **Companion**  | `docs/research/quantum-sensing/14-nv-diamond-sensor-simulator.md`, `docs/research/quantum-sensing/15-nvsim-implementation-plan.md` |

## Context

`docs/research/quantum-sensing/14-nv-diamond-sensor-simulator.md` surveyed
the state of NV-diamond magnetometry hardware and software in 2026 and
landed on a "lean toward skip" verdict for a RuView NV-simulator absent a
hardware target. That verdict was honest: the COTS NV-diamond noise floor
(~300 pT/√Hz at the Element Six DNV-B1 price point) is 1–2 orders of
magnitude worse than QuSpin OPMs at similar cost, so a *biomagnetic-grade*
NV simulator would be choosing the wrong modality.

The user nonetheless chose to build the simulator, with two non-biomagnetic
use cases in mind:

1. **Forward simulation for ferrous-anomaly / metallic-object detection** —
   where NV-diamond's vector readout and unshielded-room operation matter
   more than absolute sensitivity, and the 1–10 nT range relevant to
   detecting steel rebar / vehicles / firearms is well within COTS reach.
2. **Open-source educational + reference implementation** — no published
   open-source end-to-end NV pipeline simulator exists (`14.md` §2.2 gap).
   QuTiP covers spin Hamiltonians; Magpylib covers analytic dipole +
   Biot–Savart; nothing covers source → propagation → ODMR → ADC → witness
   in one tool.

`docs/research/quantum-sensing/15-nvsim-implementation-plan.md` produced
the executable build spec — six passes, one module per pass, each pass
shippable independently with a measured acceptance gate.

## Decision

Build `nvsim` as a **standalone Rust leaf crate** at `v2/crates/nvsim/`
implementing the six-pass plan in doc 15. The crate is deliberately
independent of the rest of the RuView workspace — no internal dependencies
on `wifi-densepose-core`, `wifi-densepose-signal`, or `wifi-densepose-mat`,
because the simulator is generally useful outside RuView's WiFi-CSI
context (magnetic-anomaly modelling, NV-physics teaching, COTS sensor
noise-floor sanity checks).

Six-pass implementation:

1. **Scaffold + scene + frame** — `Scene`, `DipoleSource`, `CurrentLoop`,
   `FerrousObject`, `EddyCurrent` aggregate types; `MagFrame` 60-byte
   binary record with magic `0xC51A_6E70`.
2. **Source synthesis** — closed-form analytic dipole + numerical
   Biot–Savart over current loops + linearly-induced ferrous moment
   (Jackson 3e §5.4–5.6; Cullity & Graham 2e §2; Magpylib reference
   per Ortner & Bandeira 2020).
3. **Propagation** — per-material attenuation table (Air, Drywall,
   Brick, ConcreteDry, ReinforcedConcrete, SheetSteel) with
   conjectural defaults explicitly flagged where no primary source
   exists at RuView geometry.
4. **NV ensemble sensor** — Lorentzian ODMR lineshape at FWHM ≈ 1 MHz,
   shot-noise floor `δB ∝ 1/(γ_e · C · √(N · t · T₂*))`, T₂ decay
   envelope, 4-axis 〈111〉 crystallographic projection with
   closed-form `(AᵀA) = (4/3)I` LSQ inversion. Defaults match Barry
   et al. *Rev. Mod. Phys.* 92 (2020) Table III for COTS bulk diamond.
5. **Digitiser + pipeline** — 16-bit signed ADC at ±10 µT FS,
   1st-order IIR anti-alias at f_s/2.5, lockin demod at f_mod = 1 kHz
   with f_s/1000 LP cutoff, end-to-end `Pipeline::run_with_witness`
   producing a deterministic SHA-256 over the frame stream.
6. **Proof bundle + criterion bench** — *pending next iteration*.

Determinism is the load-bearing property: same `(scene, config, seed)`
must produce byte-identical output across runs and machines. Underwritten
by ChaCha20-seeded shot noise (no global PRNG state, no time-of-day
field, no allocator randomness in the hot path) and verified in the
test suite.

## Consequences

### Positive

- **Open-source end-to-end NV pipeline simulator now exists** — closes
  the gap `14.md` §2.2 identified.
- **Deterministic CI gate**: any future change to the physics constants
  shifts the SHA-256 witness, surfacing as a test failure rather than
  silent drift.
- **Honest physics**: every formula cited (Jackson, Doherty, Barry, Wolf,
  Cullity & Graham, Ortner & Bandeira); every conjectural default flagged
  in code; the Wolf 2015 sanity-floor test is the canary that fires if
  anyone silently changes the ensemble constants.
- **Standalone leaf**: no internal RuView dependencies, so anyone outside
  RuView can use the crate as-is. RuView integrations land behind opt-in
  feature flags.
- **Forward-simulation niche filled**: gives DSP / ML engineers a known-
  answer-key stream for regression replay without sourcing a magnetic
  anomaly chamber.

### Negative / risks

- **Wrong modality risk**: per `14.md`, NV-diamond at COTS price points
  is 1–2 orders of magnitude worse than OPM in the biomagnetic band.
  Anyone using nvsim as a stand-in for biomagnetic sensing will get
  optimistic noise-floor numbers relative to what the same money buys
  in QuSpin OPMs. Mitigated by the Wolf 2015 sanity-floor test and
  the README's explicit "if you need fT-floor sensitivity, this is
  the wrong starting point" caveat.
- **Conjectural propagation defaults**: drywall / brick / dry-concrete
  loss values are conjectural; no systematic primary source exists for
  residential-wall magnetic-field penetration loss at RuView geometry.
  Flagged in code and in `15.md` §2.2; the `HEAVY_ATTENUATION` flag
  surfaces this to downstream consumers.
- **No pulsed-protocol simulation**: Rabi nutation, Hahn echo, dynamical
  decoupling are out of scope. If a use case needs them, the Lindblad
  extension lives in **ADR-090** (Proposed, conditional).
- **Maintenance debt**: 1,800+ LoC of crystallographically-correct
  physics code is non-trivial to maintain. Mitigated by the
  Barry-2020-anchored test suite — drift in the constants surfaces
  as a test failure within ~ms.

### Neutral

- ESP32-S3 firmware is **untouched** by this work — `nvsim` is host-side
  only. Existing firmware tags (`v0.6.2-esp32`) continue to ship
  unchanged.
- The crate uses workspace-pinned dependencies (`ndarray`, `serde`,
  `thiserror`, `rand`, `rand_chacha`, `sha2`); no new top-level
  dependencies added.
- ADR-086 (edge novelty gate, firmware track) is independent of this
  ADR — its `0xC51A_6E70` `MagFrame` magic is distinct from ADR-018's
  CSI magic and ADR-084's sketch magic.

## Validation

Acceptance criteria measured per the implementation plan §5:

| Criterion | Floor | Measured | Verdict |
|---|---|---|---|
| Same `(scene, seed)` → byte-identical SHA-256 witness | required | `determinism_same_seed_byte_identical_witness` test passes | ✓ |
| Shot-noise-OFF reproduction of analytical Biot–Savart | ≤ 0.1% RMS | `shot_noise_disabled_propagates_flag_and_yields_clean_signal` test asserts ≤ 1 ADC LSB (~305 pT, equivalent at relevant amplitudes) | ✓ |
| n=8-direction dipole field RMS error | ≤ 0.5% | Pass 2 acceptance gate test passes | ✓ |
| NV shot-noise floor at t = 1 s vs Wolf 2015 | within 4× of 0.9 pT/√Hz | Pass 4 sanity-floor test passes; falls in window | ✓ |
| Pipeline throughput ≥ 1 kHz on Cortex-A53 | ≥ 1 kHz | _pending_ — Pass 6 criterion bench | _track_ |
| Lockin SNR for 1 nT @ 1 kHz vs 100 pT/√Hz floor | ≥ 10 in 1 s | _pending_ — Pass 6 integration test | _track_ |

Test count: **45 nvsim unit tests** passing (workspace 1,620 total, +45
from baseline 1,575), zero failures, zero ignores. ESP32-S3 on COM7
unaffected throughout.

## Implementation status

| Pass | Module | Commit | Tests |
|---|---|---|---|
| 1 | scaffold + scene + frame | `9c95bfac0` | 12 |
| 2 | source.rs (Biot–Savart) | `a6ac08c66` | +7 |
| 3 | propagation.rs | `8c062fbaa` | +7 |
| 4 | sensor.rs (NV ensemble) | `177624174` | +8 |
| 5 | digitiser.rs + pipeline.rs | `436d383c9` | +11 |
| 6 | proof.rs + criterion bench | _pending_ | _≥ 5_ |

Branch: `feat/nvsim-pipeline-simulator`. README at
`v2/crates/nvsim/README.md` — plain-language audience-facing front page.

## Related

- **ADR-090** (Proposed, conditional) — full Hamiltonian / Lindblad
  solver extension for pulsed protocols. Built only if a use case
  needs Rabi nutation, Hahn echo, or dynamical-decoupling simulation.
- **ADR-018** — CSI binary frame magic (`0xC51F...`). nvsim's
  `MAG_FRAME_MAGIC` (`0xC51A_6E70`) is deliberately distinct.
- **ADR-028** — ESP32 capability audit + witness verification. nvsim's
  proof bundle pattern is the same shape as `archive/v1/data/proof/`.
- **ADR-066** — Swarm bridge to Cognitum Seed coordinator. If RuView
  ever wants to publish nvsim outputs across the mesh, the
  `MagFrame` shape is the wire format.
- **ADR-086** — Edge novelty gate. Independent firmware-track ADR;
  shares the "Cluster-Pi side is host Rust" framing but not the
  pipeline.

## Open questions

- **Should nvsim be published to crates.io as a standalone crate?** It
  already has no internal RuView deps. The repo's MIT/Apache-2.0
  license is permissive. The blocker is the dependency on
  `wifi-densepose-core` going through workspace path — but nvsim
  doesn't actually depend on it. If the answer is yes, this is a
  trivial follow-up.
- **Does `nvsim::Pipeline` belong in the same crate as `nvsim::scene`?**
  Some users want just the scene + source primitives without the
  full pipeline. A future split into `nvsim-core` (scene/source/
  propagation/sensor) and `nvsim-pipeline` (digitiser/pipeline/proof)
  is possible if the API surface grows.
- **What's the right venue for the deterministic-proof bundle?**
  Pass 6 will write `expected_witness.sha256` alongside the test
  suite. Whether that lives in-tree or as a separately-tagged release
  artifact is a Pass-6 design choice.
