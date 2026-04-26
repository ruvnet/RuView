# NV-Diamond Sensor Simulator: SOTA Survey and Build/Skip Decision

## SOTA Research Document — Quantum Sensing Series (14/—)

**Date**: 2026-04-25
**Domain**: NV-Diamond Magnetometry × Sensor Simulation × RuView Pipeline Integration
**Status**: Research Survey + Crate Proposal
**Branch**: `research/nv-diamond-sensor-simulator` (no commits, no production code)
**Prior**: `13-nv-diamond-neural-magnetometry.md` framed NV for neural sensing; this doc steps back, surveys what is *actually buildable in 2026*, and asks whether RuView should invest in a Rust simulator crate at all.

---

## 1. Why this document exists

`13-nv-diamond-neural-magnetometry.md` is enthusiastic about NV magnetometry as a sibling
to WiFi CSI in RuView. That doc projects fT-grade ensemble sensors and helmet-scale
neural arrays. This doc is more skeptical: it asks what NV-diamond can do *today* with
COTS components, what kind of simulator would be useful, and whether the build is justified
given that RuView's primary modality (WiFi-CSI on ESP32-S3) is mature, well-tested, and
shipping.

The doc is structured for a build/skip decision:

1. SOTA of NV-diamond hardware (commercial + academic)
2. SOTA of NV-diamond simulators (what is open, what is missing)
3. Concrete crate proposal *if* RuView decides to build
4. Open questions that materially change the answer

---

## 2. NV-Diamond Hardware SOTA (2024–2026)

### 2.1 Commercial sensors and what they actually output

The NV-magnetometry COTS market is small and mostly aimed at scanning-probe microscopy
or NMR enhancement, not the room-scale "sensor at distance" use case that would matter
for RuView.

| Vendor | Product | Sensitivity (vendor claim) | Bandwidth | Form factor | Notes |
|---|---|---|---|---|---|
| Qnami | ProteusQ | ≈100 nT/√Hz at AFM tip [Qnami datasheet, 2024] | DC–kHz | Benchtop AFM | Single-NV scanning, not bulk |
| QZabre | NV microscope | ≈100 nT/√Hz [QZabre site] | DC–kHz | Benchtop | Single-NV |
| Element Six | DNV-B14, DNV-B1 boards | ≈300 pT/√Hz [Element Six DNV-B1 datasheet] | DC–1 kHz | Embedded module | Bulk ensemble, USB output |
| Adamas Nanotechnologies | Diamond material | Material vendor | — | Powders/films | Substrate supplier only |
| ODMR Technologies | DNV magnetometer | ≈1 nT/√Hz (claimed) | DC–10 kHz | Benchtop | Limited published data |
| Thorlabs | (none yet COTS for NV) | — | — | — | OdMR/NVMag *not* a current Thorlabs catalog item; vendor cited in user prompt — no primary source found |

Honest correction to the prompt: **Thorlabs does not currently sell an NV magnetometer
product** as of this survey (no primary source found; the closest items are diamond
samples sold via Element Six and lock-in amplifiers via Stanford Research / Zurich
Instruments that are *used* in NV setups). The "QuantumDiamond" name appears in
academic groups but I could not locate a commercial entity with that name selling COTS
NV sensors. Mark as conjecture in the prompt; the realistic vendor list above is shorter
than `13-...md` implied.

The Element Six **DNV-B1** is the most concrete COTS reference point. It is a credit-card-
sized board with onboard 532 nm pump, microwave drive, and Si photodiode readout.
Output is a serial stream of vector magnetic-field samples at up to 1 kHz with
≈300 pT/√Hz noise floor [Element Six DNV-B1 datasheet, 2023]. Cost: ≈$8K–$15K,
unsuitable for RuView's $200–$500/sensor target.

### 2.2 Academic SOTA at room temperature, ensemble, COTS-ish

Best published bulk-diamond ensemble sensitivities at room temperature with
table-top (not cryogenic, not vacuum) optics:

- **Wolf et al., Phys. Rev. X 5, 041001 (2015)** — 0.9 pT/√Hz at 10 Hz, 13.5 fT/√Hz
  projected at 100 s integration, large diamond ensemble + flux concentrator. Earliest
  pT-floor demonstration. (~10 yr old; still the canonical reference floor.)
- **Barry et al., Rev. Mod. Phys. 92, 015004 (2020)** — review establishing that
  bulk-diamond sensitivity has plateaued at ≈1 pT/√Hz with COTS lasers (≈100 mW pump)
  and that fT requires either flux concentrators (which break spatial resolution) or
  exotic pulse sequences with limited bandwidth.
- **Fescenko et al., Phys. Rev. Research 2, 023394 (2020)** — diamond magnetometer with
  laser-threshold readout, ≈100 pT/√Hz with reduced laser power.
- **Zhang et al., Nat. Comm. 12, 2737 (2021)** — Hahn-echo at 0.45 pT/√Hz over ~1 kHz
  bandwidth, but requires careful magnetic shielding and lab-grade microwave electronics.
- **Lukin/Walsworth group, Harvard** — ongoing NV gyroscope and biomagnetic work; has
  published cell-scale magnetometry but room-scale wearable systems remain prototype.
- **Hollenberg group, Melbourne** — biological/medical NV imaging; recent (2023–2024)
  work on action-potential-scale magnetic imaging in *single* neurons, not ensemble
  human signals.
- **Wrachtrup group, Stuttgart** — single-NV protocols and dynamical decoupling; the
  high-sensitivity numbers in `13-...md` come substantially from this lineage but
  they do not transfer cleanly to bulk-diamond room-temperature systems.

**Realistic 2026 noise floor** at room temperature with COTS components:

| Configuration | Floor | Bandwidth | Source |
|---|---|---|---|
| COTS ensemble board (DNV-B1) | ≈300 pT/√Hz | DC–1 kHz | Element Six datasheet |
| Tabletop ensemble + flux concentrator | ≈1–5 pT/√Hz | DC–100 Hz | Wolf 2015, Fescenko 2020 |
| Pulsed DD + magnetically shielded room | ≈100 fT/√Hz to 1 pT/√Hz | narrow band | Zhang 2021, Barry 2020 |
| RF-band detection (GHz) via NV-AC | nT/√Hz, 1–10 MHz BW | narrow band | various |

The fT-floor numbers in `13-...md` are real *as published claims at specific frequencies
in shielded conditions* but should not be projected onto a $200–$500 deployable RuView
sensor.

### 2.3 NV-diamond vs OPM (the real comparison anchor)

Optically pumped magnetometers (OPMs / SERF) are the actually-deployed COTS competitor
for biomagnetic sensing. **QuSpin QZFM** is the dominant product:

- ≈7–15 fT/√Hz in DC–150 Hz band [QuSpin QZFM Gen-3 datasheet, 2023]
- ≈$8K–$15K per sensor
- Requires ambient-field nulling (passive shield or active bi-planar coils) — this is
  the operational constraint that limits OPM deployment outside MEG labs
- Already used in commercial wearable MEG (Cerca Magnetics, FieldLine) at clinical scale

**OPM beats NV-diamond on pure sensitivity by 1–2 orders of magnitude** at sub-kHz, at
similar cost-per-sensor. NV-diamond's distinctive value lives elsewhere:

| Axis | NV-Diamond | OPM | Winner for RuView |
|---|---|---|---|
| DC–100 Hz sensitivity | pT/√Hz | fT/√Hz | OPM |
| Vector readout (no rotation) | Yes (4 NV axes) | No | NV |
| Operating range to high field | Wide (no SERF saturation) | Narrow (<200 nT) | NV |
| Bandwidth above 1 kHz | Up to GHz | < 1 kHz | NV |
| Heating near subject | Negligible | 150 °C cell | NV |
| Shielding requirement | Light | Heavy | NV |
| Laser power budget | 50–500 mW | <50 mW | OPM |
| Maturity for biomagnetics | Lab | Shipping | OPM |

The honest summary: **for vital-signs-from-magnetic-field, NV-diamond loses to OPM today.**
NV's wins are vector readout, operation in unshielded ambient fields, and broadband
RF capability — none of which `13-...md` actually exploited.

---

## 3. NV-Diamond Simulator SOTA

### 3.1 Spin-Hamiltonian level (mature, open-source)

These simulate the NV electronic state under microwave + optical drive and reproduce
ODMR contrast, Rabi nutation, T1/T2 decay. They are *backend* tools — they would sit
inside `sensor.rs` of a RuView simulator, not be the simulator themselves.

- **QuTiP** [Johansson et al., Comp. Phys. Comm. 184, 1234 (2013)] — Python toolbox for
  open quantum systems. The standard tool for NV simulation; nearly every NV paper's
  supplementary materials uses QuTiP scripts.
- **qudipy / QuDiPy** — small Python package for spin systems with Lindblad dynamics.
  Less mature than QuTiP; useful for educational examples.
- **Spinach** [Hogben et al., J. Magn. Reson. 208, 179 (2011)] — MATLAB-only. Very fast
  for large spin systems but license-encumbered.
- **EasySpin** [Stoll & Schweiger, J. Magn. Reson. 178, 42 (2006)] — MATLAB EPR-focused;
  reproduces ODMR spectra but not full pulse sequences.
- **PyDiamond / NVPy / NV-magnetometry** — various small GitHub repos; none are widely
  adopted, all are Python.

**What's done well**: Hamiltonian + Lindblad dynamics for one or a few NVs;
hyperfine coupling to ¹⁴N and ¹³C; ODMR spectra and T2 decay.

**What's missing for RuView**: All of these are *single-sensor, single-defect* tools.
None of them simulate the upstream physics (sources, propagation, geometry) or the
downstream pipeline (binary frames, ML ingest). And none are in Rust.

### 3.2 Magnetic-field synthesis level (sparse, application-specific)

This is the layer that would matter most for RuView but is the least developed:

- **Magpylib** [Ortner & Bandeira, SoftwareX 11, 100466 (2020)] — Python library for
  analytical magnetic-field computation from permanent magnets, current loops, dipoles.
  Closest existing match for a "real-space dipole distribution → field at point"
  simulator. Pure Python; ~1k LOC core; no Rust port; no lossy-medium propagation.
- **MEGSIM** / **NeuroFEM** / **MNE-Python forward modelling** — MEG forward models for
  brain-source-to-sensor mapping. Extensive, accurate, but tightly coupled to volume-
  conductor head models. Overkill for room-scale RuView sensing.
- **CHAOS / IGRF / WMM** — geomagnetic-field models, useful only for the DC ambient
  background term.

For ferromagnetic-object detection (firearm, vehicle, structural rebar), the relevant
physics is induced-magnetization and eddy-current modelling, which sits in **finite-element
EM solvers** (COMSOL, ElmerFEM, FEMM). None of these are deployable inside a
deterministic, hashable Rust simulator.

### 3.3 End-to-end pipeline simulators

I could not find a single open-source simulator that goes
**source → propagation → diamond → ODMR → digital → ML pipeline**. The closest published
work:

- **Schloss et al., Phys. Rev. Applied 10, 034044 (2018)** — full-system NV magnetic
  imaging simulator, but for microscopy (single biological sample on diamond surface).
- **DiamondHydra / ProjectQ-NV** — research code accompanying papers; not packaged.

This gap is the strongest argument *for* RuView building one.

---

## 4. RuView NV-Diamond Sensor Simulator — Proposal

### 4.1 Use-case scoping (the part that has to be honest)

`13-...md` proposed neural sensing as the primary use case. Re-evaluating against
SOTA hardware noise floors and OPM as competitor, the honest ranking of plausible
RuView use cases is:

| Use case | Realistic with COTS NV in 2026? | Better answered by | RuView fit |
|---|---|---|---|
| Cortical neural fT signals | No (OPM wins, requires shielded room either way) | OPM helmet (Cerca) | Weak |
| Cardiac MCG (~50 pT QRS, surface) | **Marginal** with pT-floor sensor at <5 cm standoff | OPM | Plausible |
| Respiration MCG (~5 pT) | No (below floor with COTS sensor) | RF / radar / WiFi-CSI | Skip |
| Ferromagnetic object presence (firearm, vehicle, rebar) | **Yes** — DC anomaly is nT–μT scale, well above floor | NV / fluxgate | Strong |
| Through-wall metal detection | **Yes** — magnetic fields penetrate dielectrics | NV / induction | Strong |
| Eddy-current motion (metal door, vehicle wheel) | **Yes** — kHz-band signal, NV broadband helps | NV | Strong |
| Biomagnetic vital signs through wall | No (drywall is dielectric — fine — but dipole 1/r³ kills SNR by ~3 m) | Skip | Skip |
| Indoor magnetic mapping for SLAM | Yes — DC-field gradients, mature | Smartphone IMU | Mature elsewhere |

**The honest reframing**: NV-diamond's RuView niche is **passive magnetic anomaly
detection** for ferrous-object presence, motion, and eddy-current signatures —
*complementing* WiFi-CSI's pose estimation rather than replacing or duplicating it.
Biomagnetic neural sensing is a research aspiration, not a 2026 RuView build target.

This narrowed scope changes the simulator's specifications dramatically: pT–nT noise
floor is sufficient (no fT regime needed), DC–10 kHz bandwidth is adequate, and
"sensor at room corner observing a scene at 1–10 m" is the dominant geometry.

### 4.2 Simulator inputs (matching the proof-bundle pattern)

The cleanest design mirrors `archive/v1/data/proof/`:

```
deterministic synthetic scene
    ├── scene.json          # source dipole positions, currents, motion
    ├── geometry.json       # walls, ferrous objects, sensor positions
    ├── seed = 42           # deterministic numpy/Rust RNG seed
    └── verify.rs           # produces SHA-256 of output, compares to expected
```

This extends ADR-028 (witness verification) naturally: the NV simulator gets its own
`expected_output.sha256` and gets included in the witness bundle.

### 4.3 Simulator outputs (matching ADR-018 / ADR-081 frame layout)

`rv_feature_state_t` is the existing binary feature frame used by `ADR-018` and
referenced through `ADR-081` (adaptive CSI mesh firmware kernel). To let downstream
consumers (mat, train, api) ingest synthetic NV data without bespoke plumbing, the
simulator output frame should be a *parallel* type, not a re-use:

```
rv_mag_feature_state_t {
    timestamp_us: u64,
    sensor_id: u8,
    bxyz_pT: [i32; 3],          // vector field, pT
    sigma_xyz_pT: [u16; 3],      // per-axis noise estimate
    quality: u8,                 // 0..255 like CSI quality
    flags: u8,                   // saturation, calibration state
}
```

The framing is intentionally close enough to `rv_feature_state_t` that the same
producer/consumer ring-buffer plumbing can be templated, but distinct enough that a
downstream consumer can't accidentally interpret a magnetic frame as CSI.

### 4.4 Physics-layer breakdown (one Rust module per layer)

| Module | Physics | What it does | What it does NOT do |
|---|---|---|---|
| `source.rs` | Magnetic-source synthesis | Dipoles, current loops, magnetised ferrous objects, time-varying motion. Magpylib-style API in Rust. | NV-NV entanglement, single-defect imaging, growth defects |
| `propagation.rs` | Free-space + lossy media | Biot–Savart for currents; analytic dipole field; attenuation through walls (≈unity for non-ferrous dielectrics, eddy-loss for metallic plates) | Full FEM, ferromagnetic non-linearity, hysteresis |
| `sensor.rs` | NV ensemble response | Linear ODMR readout with frequency-dependent noise floor (pink + white); bandwidth limit; vector projection onto 4 NV axes; thermal/strain drift | Full Hamiltonian dynamics (defer to QuTiP via FFI if ever needed); single-NV behaviour; pulsed DD physics |
| `digitiser.rs` | ADC + frame packer | Integer scaling, saturation, jitter, frame timestamping, SHA-256 over output stream | Network transport (defer to existing API plumbing) |

Each module is independently testable and independently swappable (e.g., replace the
coarse `propagation.rs` with a FEM-backed implementation later without touching
`sensor.rs`).

### 4.5 Crate naming

Two candidates considered:

- **`wifi-densepose-magsim`** — describes the modality (magnetic) and operation
  (simulator). Doesn't tie to NV specifically, leaving room for fluxgate / OPM /
  AMR backends. **Recommended.** Also the shorter name.
- **`wifi-densepose-nvsim`** — explicitly NV. Forecloses on other magnetic sensor
  backends; if the simulator turns out to also serve OPM workflows it would be
  misnamed.

Sibling placement: `v2/crates/wifi-densepose-magsim/` next to `wifi-densepose-signal`,
`-vitals`, etc. Matches the existing 15-crate workspace pattern.

### 4.6 Integration points with existing crates

- `wifi-densepose-core` — extend `FrameKind` enum to include `MagneticVector` so
  the unified frame plumbing routes magnetic frames correctly.
- `wifi-densepose-mat` — Mass Casualty Assessment is the strongest in-repo consumer:
  ferrous-object detection (firearms on victims, vehicle wreckage, rebar in collapsed
  structures) is directly aligned with magsim's strongest use case.
- `wifi-densepose-signal/ruvsense/` — `field_model.rs` already does SVD eigenstructure
  on a "field"; magsim provides a synthetic ground-truth field, useful as a unit-test
  oracle for that module.
- `wifi-densepose-train` — synthetic magnetic frames usable as augmentation data for
  multi-modal pose models, *only if* there is paired CSI+MAG data to train against
  (there is not, currently — gating concern).
- `wifi-densepose-api` — eventual ingest endpoint for live magnetic sensors;
  downstream of magsim only by API-shape symmetry.

### 4.7 Out of scope (explicit non-goals)

- Single-NV imaging (nm-scale microscopy). Not RuView's geometry.
- NV-NV entanglement protocols. Not RuView's hardware budget.
- Full Hamiltonian + Lindblad solver. Defer to QuTiP via offline pre-computed
  noise spectra if ever needed.
- Diamond growth simulation. Material-science problem; vendor-handled.
- fT-floor sensitivity claims. Outside COTS deliverable in 2026.
- Pulsed dynamical-decoupling sequence design. Hardware-firmware concern, not
  simulator concern.

---

## 5. Verdict on whether to build

### Build arguments
1. There is a real *gap* in open-source end-to-end NV-pipeline simulators (Sec 3.3).
2. Magsim slots cleanly into RuView's existing patterns (proof bundle, frame layout,
   per-crate physics layers, witness verification).
3. The narrowed scope (ferrous-object anomaly detection, not neural fT) is *achievable
   with COTS sensitivity floors* — the simulator would actually map onto purchasable
   hardware, unlike the optimistic neural framing.
4. `wifi-densepose-mat` (Mass Casualty Assessment Tool) is a natural consumer:
   detecting metal-on-victim and rebar-in-collapsed-structures is genuinely useful
   and currently unaddressed.

### Skip arguments
1. **OPM wins on sensitivity at similar cost** for any biomagnetic use case. If the
   eventual goal is biomag, RuView should simulate OPM, not NV.
2. **No paired training data**. Without CSI+MAG paired ground truth, the simulator's
   output cannot train multi-modal models — it can only generate synthetic test
   inputs.
3. **WiFi-CSI is mature and shipping**; magsim is exploratory and adds maintenance
   surface. The 15-crate workspace is already large for a small team.
4. **The hardware decision precedes the simulator**. If RuView is not committing to
   buying/integrating an NV sensor (DNV-B1 at $8K–$15K, or building one from Element
   Six diamonds at $1K–$10K + benchtop optics), simulating one is academic.

### Honest verdict

**Lean toward "skip for now, revisit when there is a concrete hardware procurement
or `mat` use case driving it."** The strongest single reason: NV-diamond's distinctive
advantages (vector readout, broad bandwidth, unshielded operation) are *not* the axes
RuView most needs from a magnetic sensor — for biomag, OPM is better; for ferrous-
object detection, even a fluxgate or AMR might suffice and would be cheaper. Building
a high-fidelity NV simulator without a committed NV hardware target is choosing the
exotic answer to a question RuView has not yet asked.

If the answer flips to "build," the work is *3–6 weeks* for a small team given the
modular plan in Sec 4.4 and the existing proof-bundle/witness-verification scaffolding.

---

## 6. Open questions that would change the verdict

### 6.1 Is COTS NV noise floor competitive with OPM at RuView's sensor budget?

**Answer (with primary sources)**: No, at the $200–$500/sensor target. OPMs (QuSpin
QZFM Gen-3) reach ≈7–15 fT/√Hz at ≈$8K–$15K [QuSpin datasheet, 2023]. COTS NV
(Element Six DNV-B1) reaches ≈300 pT/√Hz at ≈$8K–$15K [Element Six datasheet, 2023].
Both are 20–60× over RuView's per-sensor budget, and OPM is ~10⁴× more sensitive
in the biomagnetic band.

**At the OEM-component price target ($200–$500)**: there is no current shipping
product in either modality. No primary source found. Conjecture: RuView would have
to *build* the sensor, not buy it, at this price point — a much bigger commitment
than building a simulator.

### 6.2 Is end-to-end SNR positive for chest-surface QRS with a DIY NV setup?

**With Wolf 2015's 0.9 pT/√Hz at 10 Hz, signal=50 pT, bandwidth=10 Hz**:
SNR ≈ 50 / (0.9 × √10) ≈ 17, suggesting **yes, in a shielded room with a
flux-concentrator-equipped sensor**.

**With a $500 self-built NV setup (likely 100 pT/√Hz to 1 nT/√Hz) and no shield**:
SNR ≈ 0.05–0.5, below detection threshold. **No.**

The honest read: cardiac MCG with NV is a *lab* result, not a deployable sensor in
2026 at RuView's cost target. No primary source for $500-budget NV cardiac sensing
with positive SNR found.

### 6.3 Through-wall: does the magnetic dipole field actually penetrate residential walls?

**Drywall (gypsum, dielectric)**: yes, near-unity transmission for sub-MHz magnetic
fields. No primary source needed; dielectrics have μ ≈ μ₀.

**Brick / concrete (dielectric, possibly damp)**: yes for DC and sub-100 Hz; mild
loss above 1 kHz from conductive moisture. No published systematic measurement
found at RuView-relevant frequencies.

**Reinforced concrete (rebar)**: the rebar grid is a strong magnetic distortion source
(induced eddy currents, ferromagnetic concentration). Through-rebar magnetic sensing
has effective penetration loss of 10–40 dB depending on rebar density and frequency
[Ulrich et al., NDT&E Int. 35, 137 (2002), for civil-engineering NDT — not RuView-
specific]. **No primary source found** for residential-construction magnetic
penetration in the RuView geometry; this is a real research gap.

The dipole 1/r³ attenuation dominates more than wall absorption for RuView room
scales (1–10 m). Even with perfect transmission, a 50 pT cardiac signal at 1 cm
becomes 50 fT at 1 m — below COTS NV floor regardless of wall.

---

## 7. If the verdict flips to "build" — three follow-up ADRs

1. **ADR: Magsim crate scope and frame format**. Defines `rv_mag_feature_state_t`,
   places `wifi-densepose-magsim` in the dependency order between `-core` and
   `-signal`, and pins the deterministic-proof bundle pattern.
2. **ADR: Magnetic-anomaly hardware target selection**. Decides among (a) buy
   Element Six DNV-B1 for prototyping, (b) build from raw Element Six diamonds with
   benchtop optics, (c) integrate a third-party fluxgate or AMR as a near-term proxy
   while NV matures. Drives sensor-layer noise model in `sensor.rs`.
3. **ADR: MAT (Mass Casualty Assessment) magnetic-anomaly extension**. Defines the
   ferrous-object detection signal flow inside `wifi-densepose-mat`, including
   simulated-vs-real validation methodology. Without a clear MAT use case, magsim
   is orphaned.

---

## 8. Open primary-source gaps

What I searched for and did not find a primary source for:

- A Thorlabs-branded NV magnetometer COTS product (the prompt named "OdMR / NVMag"
  but neither is in the current Thorlabs catalog as best I could tell).
- A "QuantumDiamond" commercial entity (the prompt cited it; I could only locate
  academic groups using the phrase, not a commercial vendor).
- Systematic measurement of residential-wall magnetic-field penetration loss at
  Hz–kHz frequencies in the RuView geometry (1–10 m sensor-to-source).
- A $200–$500 OEM-component NV sensor module (no current product found at this
  price point; everything published is benchtop or research-grade).
- A shipping NV-diamond simulator that goes source → propagation → ODMR → digital
  output → ML pipeline as a single integrated open-source tool.

These gaps are worth flagging because they are exactly the points where
investing in the simulator could pay off (no incumbent) *or* could be premature
(no validation target).

---

## 9. References (primary sources cited inline)

- Wolf, T. *et al.* "Subpicotesla Diamond Magnetometry." *Phys. Rev. X* **5**,
  041001 (2015).
- Barry, J. F. *et al.* "Sensitivity optimization for NV-diamond magnetometry."
  *Rev. Mod. Phys.* **92**, 015004 (2020).
- Fescenko, I. *et al.* "Diamond magnetometer enhanced by ferrite flux concentrators."
  *Phys. Rev. Research* **2**, 023394 (2020).
- Zhang, C. *et al.* "Diamond magnetometry of meV-scale magnetic fluctuations."
  *Nat. Comm.* **12**, 2737 (2021).
- Schloss, J. M. *et al.* "Simultaneous broadband vector magnetometry using
  solid-state spins." *Phys. Rev. Applied* **10**, 034044 (2018).
- Ortner, M. & Bandeira, L. G. C. "Magpylib: A free Python package for magnetic field
  computation." *SoftwareX* **11**, 100466 (2020).
- Johansson, J. R., Nation, P. D., Nori, F. "QuTiP: An open-source Python framework
  for the dynamics of open quantum systems." *Comp. Phys. Comm.* **184**, 1234 (2013).
- Element Six DNV-B1 datasheet (2023). Material vendor publication.
- QuSpin QZFM Gen-3 datasheet (2023). Vendor publication.
- Ulrich, R. K. *et al.* on rebar magnetic NDT: *NDT&E Int.* **35**, 137 (2002) —
  cited as proxy for non-RuView-geometry rebar penetration; not directly applicable.

Inline conjecture markers ("no primary source found, conjecture") appear in
Sections 2.1, 6.1, 6.2, and 6.3 where claims could not be grounded.

---

*This document is part of the Quantum Sensing research series. It surveys
NV-diamond magnetometry SOTA and proposes — but does not advocate for — a Rust
simulator crate within the RuView workspace. The build/skip recommendation
defers to a concrete hardware procurement decision or a `wifi-densepose-mat`
use case, neither of which exists at the time of writing.*
