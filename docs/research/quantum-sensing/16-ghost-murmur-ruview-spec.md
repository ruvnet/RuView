# Ghost Murmur on RuView — A Specification for an Open, Honest, Multi-Modal Heartbeat Mesh

## SOTA Research + Build Spec — Quantum Sensing Series (16/—)

| Field | Value |
|---|---|
| **Date** | 2026-04-26 |
| **Domain** | NV-diamond magnetometry × 60 GHz mmWave radar × WiFi CSI × multistatic fusion |
| **Status** | Research spec — speculative architecture, **not** a delivered system. Educational + safety-critical use cases only. |
| **Refines** | ADR-089 (nvsim simulator), ADR-029 (RuvSense multistatic), ADR-021 (vitals), ADR-022 (wifiscan) |
| **Companion docs** | `14-nv-diamond-sensor-simulator.md`, `15-nvsim-implementation-plan.md`, `13-nv-diamond-neural-magnetometry.md` |
| **Audience** | RuView contributors, sensing researchers, journalists fact-checking the news, students learning multimodal RF + quantum sensing |

---

## TL;DR

In early April 2026, the CIA reportedly used a Lockheed Skunk Works system called **"Ghost Murmur"** to help locate a downed F-15E pilot in southern Iran by detecting his heartbeat. Officials publicly suggested detection ranges as long as **40 miles**. Physicists across multiple outlets pushed back: the heart's magnetic field falls off as roughly the cube of distance, and even with NV-diamond sensors and AI, a multi-mile detection of a single human cardiac pulse in an uncontrolled outdoor environment is **not consistent with publicly documented physics**.

This doc does two things:

1. **Reality-check the news.** Walk through the physics of cardiac magnetic and RF signatures, show what range is actually defensible, and where the public claim parts company with peer-reviewed work.
2. **Map a sober version onto RuView.** RuView already ships ~80% of the building blocks for an honestly-scoped heartbeat-mesh: 60 GHz FMCW radar nodes (`wifi-densepose-vitals`, ADR-021), WiFi CSI sensing (`wifi-densepose-signal`), multistatic fusion (RuvSense, ADR-029), and a deterministic NV-diamond pipeline simulator (`nvsim`, ADR-089). What we *don't* ship is a magic 40-mile sensor — and we're explicit about why nobody does.

This is a research spec, not a build directive. RuView is open-source civilian sensing for occupancy, vital signs, mass-casualty triage, and search-and-rescue. The spec exists so that:

- A practitioner reading the news can understand which parts of "Ghost Murmur" are physically plausible, which are press-release physics, and what a real implementation would look like.
- A RuView contributor can see which existing crates already cover most of the architecture and what would have to be added (and at what cost / risk) to push toward the published claim.
- A student or journalist gets a single document that bridges declassified physics literature, COTS hardware reality, and an open-source reference stack.

---

## 1. What was reported

On Good Friday, **3 April 2026**, US Air Force F-15E pilot "Dude 44 Bravo" went down in southern Iran during the regional exchange and evaded for roughly two days before being recovered in a US-led joint operation. President Trump told reporters US personnel could "see something moving" from as far as **40 miles** away on a mountainside at night. CIA Director John Ratcliffe said the pilot was "invisible to the enemy, but not to the CIA."

In the days that followed, multiple outlets named the technology:

- **Newsweek** — "Ghost Murmur ... a secretive CIA tool linked to the Iran airman rescue."
- **Open The Magazine** — "Found by his heartbeat."
- **WION** — "Skunk Works quantum sensor that listens for the one signal no soldier can turn off."
- **Yahoo Finance / Military.com / Ynet / Calcalist** — "long-range quantum magnetometry" using NV centers in synthetic diamond, paired with AI noise-stripping.
- **Hacker News** thread — community discussion of which parts are plausible.

The recurring technical claims:

| Claim | Source quoted |
|---|---|
| Sensors built around **nitrogen-vacancy (NV) defects in synthetic diamond** | All outlets |
| **AI** strips environmental noise to isolate cardiac signal | All outlets |
| Operates at **room temperature** in smaller packages than SQUIDs | Military.com |
| Detection range "tens of miles" | Trump remarks, Open The Magazine, WION |
| Developed by **Lockheed Martin Skunk Works** | All outlets |
| First operational use in this rescue | Newsweek, Yahoo |

The recurring technical objections:

| Objection | Source |
|---|---|
| At 10 cm from chest, magnetocardiography (MCG) is "just barely detectable" | Wikswo (Vanderbilt), via Scientific American |
| At 1 m: ~10⁻³ of 10 cm signal | Wikswo |
| At 1 km: ~10⁻¹² of 10 cm signal | Orzel (Union College) |
| 60 years of MCG has required **shielding** + cm-scale standoff | Roth (Oakland) |
| A helicopter-borne MCG would be "not incremental but transformative" | Roth |
| The actual rescue involved "multiple aircraft and a survival beacon" | Scientific American |

> The most intellectually honest read: NV-diamond magnetometry **is** a real, fast-moving field; long-range magnetic detection of a human heart at 40 miles in a desert **is not** a documented capability. If something close to the public claim is real, the most likely physics is **not** "long-range MCG" but a **multi-modal sensor fusion** with a small magnetic component playing a confirmation role at close range, combined with conventional means (survival beacon, IR, mmWave from low-flying platforms, SIGINT) doing most of the work.

---

## 2. Cardiac signatures — what nature actually gives you

The human heart emits four physically distinct signatures a remote sensor can in principle detect. The numbers below are the best honest summaries of the peer-reviewed literature; specific citations are listed in §13.

### 2.1 Magnetocardiogram (MCG)

The heart's electrical depolarisation produces a magnetic field with a peak QRS amplitude of ~50 pT measured 10 cm above the chest [Cohen 1970; Bison 2009; Barry 2020]. The dipole approximation gives field strength ∝ 1/r³ in the far field:

| Distance | Peak QRS field (order-of-magnitude) |
|---|---|
| 10 cm | 50 pT |
| 1 m | 50 fT |
| 10 m | 50 aT (10⁻¹⁸ T) |
| 1 km | 5 × 10⁻²³ T |
| 40 mi (65 km) | 10⁻²⁸ T |

Earth's magnetic field is ~50 µT — i.e. **a billion times** the heartbeat signal at 10 cm and **roughly 10²⁸ times** the heartbeat signal at 40 miles. Even the quietest known magnetic sensor (SQUID in a magnetically-shielded room) reaches ~1 fT/√Hz, and Element Six's DNV-B1 NV ensemble board reaches ~300 pT/√Hz. NV's published ensemble laboratory record is around 0.9 pT/√Hz [Wolf 2015]. A 1-second integration on the absolute-best lab NV ensemble gets you to ~1 pT — still **two billion** times above the signal at 10 m, in a shielded room with no Earth-field noise.

**Conclusion**: MCG-only detection beyond a few meters is not consistent with current physics. Press-release "miles-scale MCG" is implausible.

### 2.2 Cardiac mechanical signature (mmWave / micro-Doppler)

The chest wall and large arteries pulsate at ~1.0–1.5 Hz (heart rate) plus 0.2–0.5 Hz (respiration). Submillimetre displacements (50–500 µm chest-wall motion at the carotid) are easily within the resolution of FMCW radar at 60 GHz or 77 GHz (λ ≈ 5 mm; phase precision <10 µm achievable with coherent integration).

| Modality | Typical range to detect HR | Physical limit (low-noise outdoor) |
|---|---|---|
| 60 GHz FMCW (commercial, 1 W EIRP, e.g. MR60BHA2) | 1–3 m | ~10 m |
| 77 GHz FMCW (automotive) | 5–15 m | ~30 m |
| L-band SAR / through-wall radar | 5–30 m, **through walls** | ~100 m |
| Long-range surveillance radar (Ka-band, kW class) | tens of km for vehicles | not used for HR |

**This** is the modality where the "tens of miles" claim becomes more interesting. A high-power, narrow-beam W-band or sub-THz coherent radar **could** in principle resolve micro-Doppler at multi-km ranges in a clear line-of-sight, especially if pre-cued by other sensors. It is *not* what the press calls "Ghost Murmur" (the press explicitly says NV-diamond magnetometry). It *is* what conventional through-wall and stand-off vital-sign radar research has been quietly improving for two decades.

### 2.3 IR thermal signature

A human at rest emits ~100 W. At ambient 20 °C, peak emission is ~9.5 µm (mid-LWIR). Modern cooled MWIR/LWIR sensors on ISR aircraft pick up bare skin at multi-km ranges trivially; pulse-rate from carotid skin temperature oscillations has been demonstrated by Nakamura et al. (Nat. Biomed. Eng. 2018) at meter scales with HD thermal cameras.

This is almost certainly part of how the actual rescue worked. It does not need a quantum sensor.

### 2.4 RF emissions and reflections from worn electronics

A pilot's survival kit includes a **PRC-112 / CSEL** or equivalent personal locator beacon broadcasting on 121.5/243/406 MHz and a UHF SATCOM uplink. Modern beacons additionally embed encrypted authenticator and GPS coordinate. *This is what actually finds downed pilots.* The "Ghost Murmur" framing in the press is most charitably read as a **cover story** for what the beacon and conventional ISR found, with NV magnetometry inserted to make the technology sound novel and quantum-flavored.

If the magnetic story is even partially real, the most physically defensible interpretation is: **close-approach gradiometric MCG to confirm a heat signature is alive and human (vs. e.g. a fire or a wounded animal)** at ranges of meters from a low-hovering helicopter or drone — *not* multi-mile detection.

---

## 3. The RuView mapping

RuView already ships, today, the building blocks for a *sober* version of the same concept — a **multi-modal heartbeat mesh** that detects, localises, and tracks human vital signs at room-to-building-to-block scale, using commodity hardware in the $5–$50 per node range and a quantum-sensor *simulator* for the magnetometry tier.

| Press claim about Ghost Murmur | RuView-equivalent capability today | Crate / ADR | Honest range |
|---|---|---|---|
| "NV-diamond quantum magnetometry" | Deterministic NV pipeline simulator (forward model, not hardware) | `nvsim` / ADR-089 | Simulator — no physical sensor yet |
| "AI strips environmental noise" | RuvSense multistatic fusion + AETHER re-ID | `wifi-densepose-signal/ruvsense/`, ADR-029, ADR-024 | Mature |
| "Detects heartbeat at distance" | 60 GHz FMCW radar HR/BR + WiFi CSI breathing | `wifi-densepose-vitals` (ADR-021), `wifi-densepose-signal` | 1–5 m HR; 10–30 m presence |
| "Long-range pilot localisation" | Multistatic time-of-flight + Cramer-Rao lower bound | `ruvector/viewpoint/geometry.rs` | Limited by node spacing |
| "Operates from a moving platform" | UAV-mounted ESP32-C6+MR60BHA2 sensor pod (sketch) | Hardware integration TBD | Active research |

The architectural pattern: **rings of sensors of decreasing cost and increasing range, fused by a Bayesian / attention-weighted backend that knows the physics-determined precision of each tier.** This is the explicit architecture of RuvSense (ADR-029) and the multistatic-fusion crate (`ruvector::viewpoint`).

---

## 4. Architecture: the three-tier RuView heartbeat mesh

The proposed architecture has three layers, each with a different physical modality and a different role in the fusion graph. Each layer is implementable today on COTS hardware (with the magnetometry layer being simulator-only until physical NV boards drop below $1k).

```
                      ┌──────────────────────────┐
                      │   Tier 3 — NV-diamond    │  Range: 0.1–2 m (today, lab)
                      │     magnetometer ring    │  Status: nvsim simulator only
                      │     (close-confirm)      │  Hardware: $$$ ($8k–15k DNV-B1)
                      └──────────┬───────────────┘
                                 │
                      ┌──────────┴───────────────┐
                      │   Tier 2 — 60 GHz FMCW   │  Range: 1–10 m HR/BR
                      │     mmWave radar mesh    │  Status: shipping (ADR-021)
                      │   (vital signs, posture) │  Hardware: $15 (MR60BHA2 + ESP32-C6)
                      └──────────┬───────────────┘
                                 │
                      ┌──────────┴───────────────┐
                      │  Tier 1 — WiFi CSI mesh  │  Range: 10–30 m through-wall
                      │   (presence, breathing,  │  Status: shipping (ADR-014, ADR-029)
                      │    pose, intention)      │  Hardware: $9 (ESP32-S3 8MB)
                      └──────────┬───────────────┘
                                 │
                                 ▼
                  ┌────────────────────────────────┐
                  │  RuvSense multistatic fusion   │
                  │   + cross-viewpoint attention  │
                  │   + AETHER re-ID embeddings    │
                  │   + Cramer-Rao gating          │
                  └────────────────────────────────┘
                                 │
                                 ▼
                       (Bayesian person hypothesis
                         with vital-sign vector)
```

Each tier *individually* is too weak to make the press-release claim. Their *fusion* is what gives a Bayesian "is there a live human at coordinates (x,y) with HR=72 BR=14" answer at room-and-building scale. Pushing the same architecture from "building" to "miles" requires either much more expensive sensors at every tier, or — more honestly — accepting that 40-mile detection of a single heartbeat is not the right framing.

### 4.1 What the three tiers *together* can credibly do

- **Indoor occupancy + vital signs at room scale**: shipping today. ESP32-S3 mesh + 60 GHz radar + breathing extraction. Sub-meter localisation, ±2 bpm heart rate, ±0.5 br/min respiration.
- **Through-wall presence + breathing at building scale**: shipping today. WiFi CSI alone, 10–30 m. ±5 br/min respiration.
- **Room-to-room transition tracking**: shipping (ADR-029 cross-room module). Environment fingerprinting + Kalman re-ID.
- **Outdoor presence at 50–200 m with directional WiFi or mmWave**: feasible with directional antennas + FCC Part 15 power. Not currently in the RuView stack.
- **Search-and-rescue cardiac confirmation at 0.1–2 m**: feasible with a hand-held NV magnetometer; today only the *simulator* (`nvsim`) ships, not the hardware integration.
- **Multi-mile single-heartbeat detection**: not feasible. Press-release physics.

---

## 5. Tier 1 — WiFi CSI mesh (the foundation, shipping today)

This is RuView's primary modality and is fully shipping. The crates (`wifi-densepose-signal`, `wifi-densepose-mat`, `wifi-densepose-train`, etc.) and ESP32-S3 firmware have been validated on real hardware (COM7, MAC `3c:0f:02:e9:b5:f8`) per ADR-028 with deterministic SHA-256 witness verification.

### 5.1 What it gives the heartbeat mesh

| Feature | Mechanism | Range | Crate / ADR |
|---|---|---|---|
| Through-wall **presence** | CSI amplitude perturbation | 10–30 m | `signal/occupancy.rs` |
| **Breathing** rate | CSI phase oscillation 0.2–0.5 Hz | 5–20 m | `signal/breathing.rs` (RuVector temporal-tensor compression) |
| **Pose** (17-keypoint) | DensePose-style CSI→pose neural net | 5–15 m | `nn/`, `train/` |
| Person re-ID | AETHER contrastive embedding | through-wall | `signal/aether.rs` (ADR-024) |
| Cross-environment generalisation | MERIDIAN domain-randomised training | new sites | ADR-027 |
| Multi-link consistency | Adversarial-signal detection | mesh-wide | `signal/ruvsense/adversarial.rs` |

### 5.2 Why CSI is the foundation

Two reasons. First, **cost**: ESP32-S3 8MB nodes are $9 each. Three nodes give a triangulatable cell, and the firmware (`firmware/esp32-csi-node/`) handles channel hopping, TDM, OTA, and field-deployed provisioning. Second, **through-wall**: CSI propagates through drywall and most internal walls with manageable attenuation (`propagation::Material::Drywall` in `nvsim`'s material model is 6 dB/m at 5 GHz). 60 GHz radar does not.

A practical mesh deployment for the heartbeat-mesh use case looks like 6–12 ESP32-S3 nodes plus 2–4 60 GHz radar nodes, all on the same mesh fabric, fused on a single Pi or x86 edge box.

### 5.3 What it cannot do

- Resolve heart rate (the 1 Hz oscillation is buried in the much-larger breathing oscillation; CSI's amplitude precision is ~10⁻² which doesn't reach the 10⁻⁴ needed for HR phase extraction)
- Detect pure cardiac **electrical/magnetic** activity (CSI is RF reflection, not bio-electric/magnetic)
- Operate at multi-km ranges (FCC Part 15 + 5 GHz path loss caps usable mesh distance at <100 m without directional antennas; <500 m with)

---

## 6. Tier 2 — 60 GHz mmWave radar mesh (shipping today)

This is where heart rate enters the architecture. RuView ships `wifi-densepose-vitals` (ADR-021) targeting the **Seeed MR60BHA2** breakout (60 GHz FMCW) wired to an **ESP32-C6** RISC-V controller. Total cost ~$15 per node.

### 6.1 What 60 GHz FMCW gives you

The MR60BHA2 ships with a vendor-provided heart-rate / respiration / presence DSP, but the more useful integration for RuView is the raw I/Q stream. From there, the standard pipeline is:

1. **Range-Doppler FFT** → distance + radial velocity per scatterer
2. **CFAR detection** → find the ~10 cm² chest-wall scatterer at 1–3 m
3. **Phase tracking** at the chest range bin → micro-displacement waveform
4. **Bandpass** at 0.7–3 Hz → cardiac micro-Doppler
5. **Fundamental frequency estimation** → heart rate (±2 bpm typical)

| Metric | Achievable on MR60BHA2 (1 m) | Achievable on 77 GHz auto radar (5 m) |
|---|---|---|
| HR accuracy | ±2 bpm | ±3 bpm |
| BR accuracy | ±0.5 br/min | ±1 br/min |
| Presence | binary | binary |
| Posture (sitting/standing/falling) | possible with ML | possible |
| Through-wall | weak (drywall ok, brick poor) | weak (drywall ok) |

### 6.2 The mesh role

A single 60 GHz node has a narrow beamwidth (~30° az, 30° el on the MR60BHA2), so room coverage requires 2–4 nodes. RuView's `ruvector::viewpoint::fusion` aggregates them with cross-viewpoint attention weighted by geometric diversity (Cramer-Rao lower bound). This is exactly the architecture you'd want for a "find a live person in a room" detector.

The honest range cap is ~10 m for HR detection in clear LOS. Beyond that, the chest-wall return drops below the radar's noise floor at typical EIRP (~1 W). Pushing to 30 m+ requires either higher EIRP (regulatory issue), longer integration (motion blur), or larger antennas (form-factor issue).

### 6.3 The "stand-off military version" not in scope here

77 GHz automotive radars at higher power and 100–200 GHz coherent sub-THz radars **can** resolve cardiac micro-Doppler at 50–500 m in clear LOS. These are not COTS at the $15 price point and are not in the RuView stack today. They are also subject to ITAR / export-control review and **explicitly out of scope** for this open-source project.

---

## 7. Tier 3 — NV-diamond magnetometer mesh (simulator only today)

This is the layer that maps directly to the press-release "Ghost Murmur" technology. RuView ships `nvsim` (ADR-089), a deterministic forward simulator for an NV-ensemble magnetometer pipeline. **It does not control physical hardware.** It is a tool for designing fusion algorithms, validating signal-processing chains, and stress-testing what physical performance you would actually need from a hypothetical sensor to make a given system-level claim true.

### 7.1 What `nvsim` already simulates

- 4 〈111〉 NV crystallographic axes
- ODMR linear-readout proxy (Barry RMP 2020 §III.A)
- Shot-noise floor δB ∝ 1/(γ_e·C·√(N·t·T₂*))
- Material attenuation through Air / Drywall / Brick / Concrete / ReinforcedConcrete / SteelSheet
- Biot-Savart current loops, dipole sources, induced ferrous moments
- 16-bit ADC + lock-in demodulation
- Deterministic SHA-256 witness for reproducibility

`nvsim` benches at ~4.5 M samples/s on x86_64 (~4500× the Cortex-A53 target). It is WASM-ready by construction (no `std::time/fs/env/process/thread`).

### 7.2 What an NV-diamond mesh node would need to look like

Today's COTS reference is the **Element Six DNV-B1** ($8–15k, ~300 pT/√Hz, 1 kHz BW). For a heartbeat-mesh role, a useful node would need:

| Spec | DNV-B1 today | What you'd need for cardiac at 1 m | What you'd need for cardiac at 10 m |
|---|---|---|---|
| Sensitivity | 300 pT/√Hz | <1 pT/√Hz (1 s integration) | <1 fT/√Hz (impossible today) |
| Bandwidth | 1 kHz | 100 Hz sufficient | 100 Hz sufficient |
| Cost | $8–15k | <$1k for mesh deployment | irrelevant if sensitivity infeasible |
| Form factor | credit card | mesh-friendly (palm size) | drone-friendly |
| Gradiometric? | No (single sensor) | **Yes** (3-axis gradiometer needed for ambient rejection) | yes |

The 1 m case is plausible **with** a 2–4 sensor gradiometric array and a magnetically-shielded test enclosure. The 10 m case requires roughly six orders of magnitude more sensitivity than any published NV ensemble has demonstrated. Press-release "miles" requires twelve.

### 7.3 What `nvsim` is for

The simulator's role is **system-design honesty**. Before anyone builds a physical NV node for RuView, you should be able to drop the sensor model into the multistatic fusion graph and answer:

- "If my NV node has 100 pT/√Hz sensitivity, what's the joint posterior P(human alive at (x,y)) given my CSI + 60 GHz + NV evidence at 0.5 m, 2 m, 5 m?"
- "What sensitivity does my NV node need to add useful information beyond the 60 GHz radar at 2 m?"
- "What does my published witness change if I swap the NV sensor's contrast from 0.03 to 0.10?"

This is the kind of pre-build sanity check that distinguishes serious open-source quantum-sensing work from press-release physics.

---

## 8. Multi-modal fusion (the real "AI" in the public claims)

The "AI strips environmental noise to isolate cardiac signal" line in the news is doing a lot of work. The honest version is:

1. **Each sensor has a known noise floor** (CSI: ~10⁻² amplitude; 60 GHz: ~µm phase; NV: ~pT). The fusion stage knows this.
2. **Each sensor has a known geometric precision** (CSI: ~5 m localisation in 30 m mesh; 60 GHz: ~10 cm in 3 m FOV; NV: ~5 cm at 1 m close-confirm).
3. **Bayesian fusion** combines them with priors (room geometry, human anatomy, expected HR/BR ranges).
4. **AI** lives in the *learned* parts: AETHER re-ID embeddings, MERIDIAN domain-generalisation, gesture DTW templates, intention pre-movement nets. Not in "magic noise stripping."

RuView's `ruvector::viewpoint::attention::CrossViewpointAttention` is the fusion primitive: a softmax over per-sensor evidence weighted by a geometric-bias matrix `G_bias` (Cramer-Rao Fisher information). The fusion is **physics-aware**: a sensor with low Fisher information for the target's location automatically gets low attention weight.

This is **not** the press's "AI does magic." It's standard sensor-fusion theory. The novelty in RuView is not the fusion — it's the fact that all the layers (CSI / 60 GHz / NV-simulator) live in one Rust workspace with a coherent type system and a single fusion crate.

### 8.1 Concrete fusion data flow

```rust
// Pseudocode showing the multistatic fusion graph
let csi_evidence    = csi_pipeline.run(csi_frames)?;          // ~10 Hz, 30 m range
let radar_evidence  = mr60bha2_pipeline.run(radar_frames)?;   // ~50 Hz, 3 m range
let nv_evidence     = nvsim_pipeline.run(simulated_nv)?;       // ~10 kHz, 1 m range (sim)

let geometric_bias  = GeometricBias::from_node_layout(&nodes);
let fused_persons   = MultistaticArray::fuse(
    &[csi_evidence, radar_evidence, nv_evidence],
    &geometric_bias,
    &PriorRoomGeometry::load(&room_id)?,
)?;

// Each fused person carries: (x, y, z, HR_bpm, BR_brpm, vector_pose, person_id_embedding,
//                              p_alive, p_human, novelty_flag, witness_hash)
```

This is **already** the architecture in `ruvector::viewpoint::fusion::MultistaticArray`. The NV row is currently fed by `nvsim` (simulator) instead of a hardware sensor. Everything else is shipping.

---

## 9. Privacy, ethics, legal — the part the press skipped

A heartbeat-detecting mesh is dual-use. It can find a heart-attack victim trapped in rubble (the original Mass Casualty Assessment Tool / `wifi-densepose-mat` use case, ADR-014) **or** it can surveil people in their homes. RuView's project line is unambiguous on this:

1. **Civilian, opt-in deployments only.** Search-and-rescue, elder-care, building occupancy for HVAC, hospital ICU vitals. Not surveillance.
2. **No directional pursuit.** RuView does not ship beam-steering, target-following, or remote person-of-interest tracking primitives. The mesh is designed for fixed-area observation with consent.
3. **Data minimisation.** The fused output is `(presence, HR, BR, pose, p_alive)` — not raw CSI / radar / NV streams. Raw streams are processed at the edge and discarded after fusion.
4. **PII detection on the wire.** ADR-040 (PII gates) blocks identifying biometric streams from leaving the local mesh without explicit user authorisation.
5. **Adversarial-signal detection.** `ruvsense::adversarial` flags physically-impossible signal patterns that would arise from a malicious node trying to inject false detections — protection against mesh attacks.
6. **No export-controlled hardware.** RuView targets <$50 COTS components. ITAR / EAR-listed sub-THz coherent radars and shielded NV ensembles are explicitly out of scope.

The Ghost Murmur press story exists in a different ethical universe — covert military intelligence ops with no consent, no notice, and no opt-out. **RuView is not that.** This spec is the open-source version: same physics, opposite governance.

### 9.1 Legal boundaries (US, non-exhaustive)

- **18 USC §2511** (federal wiretap) — RF sensing of presence and vital signs is generally not a "wire/oral communication" intercept, but state-law recording statutes can apply if audio is involved.
- **HIPAA** — vital-sign data from medical contexts requires HIPAA-covered handling.
- **FCC Part 15** — ESP32 and 60 GHz radar emissions must remain compliant (RuView firmware defaults to compliant power).
- **ITAR / EAR** — high-power coherent sub-THz radar, shielded NV ensembles, and certain ML models trained on pose data may be export-controlled. RuView avoids this category.
- **State biometric laws (BIPA, CCPA, similar)** — pose / gait / cardiac signatures may qualify as biometric identifiers; consent regimes vary.

If you are deploying RuView outside a controlled research setting, talk to a lawyer who actually does this for a living.

---

## 10. How to actually implement, on RuView, today

This section is the build guide. It assumes you're starting from a clean RuView checkout and want a working 3-node CSI mesh + 1 mmWave node + a simulated NV row, fused into a single `(x, y, HR, BR, p_alive)` stream.

### 10.1 Hardware bill of materials

| Tier | Component | Qty | Per-unit | Total |
|---|---|---|---|---|
| 1 | ESP32-S3 8 MB DevKit | 3 | $9 | $27 |
| 1 | Mini-PoE injector + cat6 | 3 | $6 | $18 |
| 2 | ESP32-C6 + Seeed MR60BHA2 | 1 | $15 | $15 |
| 3 | (NV node — simulated only) | 0 | — | — |
| Edge | Raspberry Pi 5 (8 GB) or Mini PC | 1 | $80 | $80 |
| Network | unmanaged GbE switch | 1 | $25 | $25 |
| **Total** | | | | **$165** |

NV-diamond hardware is intentionally absent: it stays as `nvsim` output until COTS NV boards drop below $1k.

### 10.2 Firmware build + flash

Use the procedure in `CLAUDE.local.md` (Python subprocess wrapper, ESP-IDF v5.4 on Windows; native bash on Linux). The relevant binaries are:

```bash
# CSI node firmware (ESP32-S3, 8 MB)
firmware/esp32-csi-node/build/esp32-csi-node.bin

# Vitals node firmware (ESP32-C6 + MR60BHA2, ADR-021)
# See `wifi-densepose-vitals` crate for ESP32-C6 builds
```

Provision each CSI node with target IP and channel:

```bash
python firmware/esp32-csi-node/provision.py \
  --port COM7 \
  --ssid "RuViewMesh" \
  --password "your-mesh-key" \
  --target-ip 192.168.50.20 \
  --channel 6
```

Repeat with `--target-ip 192.168.50.21`, `.22` for the other two nodes.

### 10.3 Edge software stack

On the Pi or mini-PC:

```bash
git clone https://github.com/ruvnet/RuView.git
cd RuView/v2
cargo build --release \
  --bin wifi-densepose \
  --bin wifi-densepose-sensing-server \
  --no-default-features
```

This produces `wifi-densepose` (CLI) and `wifi-densepose-sensing-server` (Axum web UI) without the optional `eigenvalue` BLAS feature, so no vcpkg/openblas dependency.

### 10.4 Configure the mesh

Drop a `mesh.toml` next to the binary:

```toml
[mesh]
name = "ghost-mesh-pilot"
nodes = [
  { id = "csi-1",   ip = "192.168.50.20", role = "csi",      channel = 6 },
  { id = "csi-2",   ip = "192.168.50.21", role = "csi",      channel = 6 },
  { id = "csi-3",   ip = "192.168.50.22", role = "csi",      channel = 6 },
  { id = "mmw-1",   ip = "192.168.50.30", role = "mmwave-60ghz" },
]

[fusion]
strategy = "multistatic-attention"
csi_weight = 1.0
mmw_weight = 2.0          # higher Fisher information per ADR-029
nv_sim_weight = 0.0       # disabled by default (simulator-only)
geometric_diversity_floor = 0.3

[vitals]
hr_band_hz   = [0.7, 3.0]
br_band_hz   = [0.1, 0.5]
hr_method    = "phase-fft"
br_method    = "csi-amplitude-fft"

[privacy]
mode                 = "edge-only"     # never ship raw CSI off-mesh
retention_seconds    = 300
pii_gate             = "strict"
adversarial_detector = "on"
```

### 10.5 Running with a simulated NV row

To pretend you have an NV magnetometer in the fusion graph (for stress-testing the architecture without buying $8k of hardware), enable the `nvsim` row in `mesh.toml`:

```toml
[fusion]
nv_sim_weight = 0.5     # any value >0 enables the simulated row

[nv_sim]
seed              = 42
sensor_position   = [0.0, 0.0, 1.5]      # x, y, z metres in mesh frame
ambient_field_uT  = [50.0, 0.0, 0.0]     # earth's field
config            = "default"            # PipelineConfig::default()
```

The fusion stage will treat the simulated row as if it were a real sensor with known noise model. Drop the `nv_sim_weight` to `0.0` to remove it. This is exactly the architecture you want for sober quantum-sensing system design.

### 10.6 Web UI

```bash
./wifi-densepose-sensing-server --config mesh.toml --listen 0.0.0.0:8080
```

Open `http://<pi-ip>:8080`. You get:

- live 2D occupancy plot per node and fused
- HR / BR per detected person
- pose skeleton (17 keypoints, AETHER re-ID)
- multistatic Fisher-information overlay
- Cramer-Rao precision ellipse per detection
- privacy-mode controls (record/erase/quarantine)

This is the closest open-source approximation to "the operator console for a Ghost Murmur node" that anyone can actually deploy in their living room with $165 of hardware.

### 10.7 Honest performance you can expect on this build

| Metric | Expected (3-node CSI + 1 mmW + nvsim row) |
|---|---|
| Person detection (LOS) | 95% TPR, 5% FPR at 0–15 m |
| Person detection (through 1 wall) | 85% TPR, 8% FPR at 0–10 m |
| HR accuracy (LOS, 0–3 m) | ±2 bpm |
| HR accuracy (through 1 wall) | not reliable on this hardware |
| BR accuracy (any mode, 0–10 m) | ±1 br/min |
| Pose keypoint error (LOS) | ~10 cm at 0–5 m |
| Latency (sensor → fused output) | 80–150 ms |

**This is not 40 miles.** It's a small house. That's the entire point of this spec.

---

## 11. Open research questions

Things that would *materially* push this stack closer to a credible "Ghost Murmur" capability — and which RuView is open to PRs on:

1. **Sub-$1k NV-ensemble board**. Rumored development at QDM Tech, NVision, Adamas Nanotechnologies; nothing shipping yet.
2. **Active stand-off cardiac radar at 76–81 GHz** with FCC-compliant power. Possible but $$ for the chipset.
3. **Distributed coherent processing** across CSI nodes (true multistatic phase-coherent SAR). Requires sub-ns clock sync (PTP or GPS-disciplined).
4. **RaBitQ binary-sketch novelty gate on ESP32** (ADR-086). Pushes the compute load down to the node so the mesh scales to hundreds of cells.
5. **Adversarial-signal detection at the firmware tier**. Currently in the Rust signal crate; should be partially pushed to ESP32 firmware so a compromised node can't poison the mesh.
6. **Privacy-preserving fusion**. Differential privacy on the fused output stream; same theory as DP-SQL but for sensor fusion.
7. **Validated `nvsim` against published MCG measurements**. The simulator is internally consistent; we have not yet asserted byte-equivalence with a published cardiac-magnetic field measurement.

---

## 12. Comparison: RuView vs. Ghost Murmur (as reported)

| Dimension | RuView heartbeat mesh (this spec) | Press-claimed Ghost Murmur |
|---|---|---|
| Range | 0.5–30 m | tens of miles |
| Modalities | WiFi CSI + 60 GHz radar + NV simulator | NV-diamond magnetometry only (per press) |
| Cost per node | $9–15 | unstated, presumably $$$$$ |
| Through-wall | yes (CSI) | unstated |
| Vital signs (HR + BR) | yes | claimed: HR |
| Open source | yes (Apache-2.0 / MIT) | classified |
| Independent verification | yes (SHA-256 witnesses, ADR-028) | no |
| Plausible per published physics | yes | not at the claimed ranges |
| Ethics governance | civilian opt-in only | covert military |
| Build today on $200 | yes | no |

**The honest framing**: RuView is not Ghost Murmur. Ghost Murmur (as reported) is not Ghost Murmur either — the physics doesn't support it. Both names point at the same family of capabilities. RuView is the one you can actually build in your garage.

---

## 13. References

### Primary physics

- Cohen, D. (1970). "Magnetocardiograms taken inside a shielded room with a superconducting point-contact magnetometer." *Appl. Phys. Lett.* 16, 278.
- Bison, G. et al. (2009). "A room temperature 19-channel magnetic field mapping device for cardiac signals." *Appl. Phys. Lett.* 95, 173701.
- Wolf, T. et al. (2015). "Subpicotesla diamond magnetometry." *Phys. Rev. X* 5, 041001.
- Barry, J. F. et al. (2020). "Sensitivity optimization for NV-diamond magnetometry." *Rev. Mod. Phys.* 92, 015004. **(The proxy validity reference for `nvsim`.)**
- Doherty, M. W. et al. (2013). "The nitrogen-vacancy colour centre in diamond." *Phys. Rep.* 528, 1–45.
- Jackson, J. D. (1999). *Classical Electrodynamics, 3e*, §5.6, §5.8 (dipole and Biot-Savart).

### mmWave and through-wall

- Gu, C. et al. (2013). "Hybrid feature-based remote sensing of human vital signs using radar." *IEEE Tran. Microwave Theory Tech.* 61, 4621.
- Adib, F. et al. (2015). "Smart homes that monitor breathing and heart rate." *CHI 2015*.
- Mostafanezhad, I. & Boric-Lubecke, O. (2014). "Benefits of coherent low-IF for vital signs monitoring." *IEEE Microw. Wireless Compon. Lett.* 24.

### WiFi CSI

- Geng, J., Huang, D., De la Torre, F. (2022). "DensePose from WiFi." arXiv:2301.00250.
- Wang, Z. et al. (2024). "MM-Fi: Multi-modal Non-Intrusive 4D Human Dataset for Versatile Wireless Sensing." NeurIPS Datasets and Benchmarks.

### News (April 2026, "Ghost Murmur")

- Newsweek — "What Is Ghost Murmur? Secretive CIA Tool Linked to Iran Airman Rescue."
- Scientific American — "What is the quantum 'Ghost Murmur' purportedly used in Iran? Scientists question CIA's claim."
- Military.com — "Ghost Murmur: The Heartbeat-Tracking Tech That Has Experts Questioning the Laws of Physics."
- Open The Magazine — "Inside CIA's Chilling New Tech 'Ghost Murmur'."
- WION — "How the CIA used secret futuristic tech to rescue downed US F-15E pilot 'Dude 44 Bravo'."
- Yahoo Finance — "Ghost Murmur: Lockheed's Quantum Heartbeat Hunter."
- Calcalist — "Spy tech or science fiction? Experts question CIA Ghost Murmur claims."
- Hacker News thread #47679241 — community discussion.

### RuView ADRs and crates referenced

- ADR-014 — SOTA signal processing
- ADR-021 — ESP32 CSI-grade vital sign extraction
- ADR-022 — Multi-BSSID WiFi scanning
- ADR-024 — AETHER contrastive embedding
- ADR-027 — MERIDIAN cross-environment domain generalisation
- ADR-028 — ESP32 capability audit + witness verification
- ADR-029 — RuvSense multistatic sensing mode
- ADR-040 — PII detection gates
- ADR-086 — ESP32-side novelty gate (RaBitQ)
- ADR-089 — `nvsim` NV-diamond pipeline simulator
- ADR-090 — `nvsim` Lindblad/Hamiltonian extension (proposed, conditional)

---

## 14. Status, license, and how this doc evolves

- **Status**: research spec, advisory only. **Not** a delivered system. **Not** a recommendation to deploy at scale.
- **License**: Apache-2.0 OR MIT (matches the rest of RuView).
- **Versioning**: bump the doc number (16/17/...) for a major rework; in-place edits for typos and citation fixes.
- **Disagreements welcome**. If you can show a peer-reviewed reference that pushes any number in §2 by an order of magnitude, please open a PR or issue.
- **No classified content.** This doc is built entirely from public news reporting, peer-reviewed physics, and RuView's own open-source architecture. Nothing here is sourced from leaks or classified material; if you have such material, do not contribute it to this document.

---

*RuView is an open-source civilian sensing platform. It is not affiliated with the United States government, the CIA, Lockheed Martin, or any classified program. References to "Ghost Murmur" in this document refer exclusively to the publicly-reported program of that name as covered in the open press in April 2026.*
