# ADR-042: Coherent Human Channel Imaging (CHCI) — Beyond WiFi CSI

**Status**: Proposed
**Date**: 2026-03-03
**Deciders**: @ruvnet
**Supersedes**: None
**Related**: ADR-014, ADR-017, ADR-029, ADR-039, ADR-040, ADR-041

---

## Context

WiFi-DensePose currently relies on passive Channel State Information (CSI) extracted from standard 802.11 traffic frames. CSI is one specific way of estimating a channel response, but it is fundamentally constrained by a protocol designed for throughput and interoperability — not for sensing.

### Fundamental Limitations of Passive WiFi CSI

| Constraint | Root Cause | Impact on Sensing |
|-----------|-----------|-------------------|
| MAC-layer jitter | CSMA/CA random backoff, retransmissions | Non-uniform sample timing, aliased Doppler |
| Rate adaptation | MCS selection varies bandwidth and modulation | Inconsistent subcarrier count per frame |
| LO phase drift | Independent oscillators at TX and RX | Phase noise floor ~5° on ESP32, limiting displacement sensitivity to ~0.87 mm at 2.4 GHz |
| Frame overhead | 802.11 preamble, headers, FCS | Wasted airtime that could carry sensing symbols |
| Bandwidth fragmentation | Channel bonding decisions by AP | Variable spectral coverage per observation |
| Multi-node asynchrony | No shared timing reference | TDM coordination requires statistical phase correction (current `phase_align.rs`) |

These constraints impose a hard floor on sensing fidelity. Breathing detection (4–12 mm chest displacement) is reliable, but heartbeat detection (0.2–0.5 mm) is marginal. Pose estimation accuracy is limited by amplitude-only tomography rather than coherent phase imaging.

### What We Actually Want

The real objective is **coherent multipath sensing** — measuring the complex-valued impulse response of the human-occupied channel with sufficient phase stability and temporal resolution to reconstruct body surface geometry and sub-millimeter physiological motion.

WiFi is optimized for throughput and interoperability. DensePose is optimized for phase stability and micro-Doppler fidelity. Those goals are not aligned.

### IEEE 802.11bf Changes the Landscape

IEEE Std 802.11bf-2025 was published on September 26, 2025, defining WLAN Sensing as a first-class MAC/PHY capability. Key provisions:

- **Null Data PPDU (NDP) sounding**: Deterministic, known waveforms with no data payload — purpose-built for channel measurement
- **Sensing Measurement Setup (SMS)**: Negotiation protocol between sensing initiator and responder with unique session IDs
- **Trigger-Based Sensing Measurement Exchange (TB SME)**: AP-coordinated sounding with Sensing Availability Windows (SAW)
- **Multiband support**: Sub-7 GHz (2.4, 5, 6 GHz) plus 60 GHz mmWave
- **Bistatic and multistatic modes**: Standard-defined multi-node sensing

This transforms WiFi sensing from passive traffic sniffing into an intentional, standards-compliant sensing protocol. The question is whether to adopt 802.11bf incrementally or to design a purpose-built coherent sensing architecture that goes beyond what 802.11bf specifies.

### ESPARGOS Proves Phase Coherence at ESP32 Cost

The ESPARGOS project (University of Stuttgart, IEEE 2024) demonstrates that phase-coherent WiFi sensing is achievable with commodity ESP32 hardware:

- 8 antennas per board, each on an ESP32-S2
- Phase coherence via shared 40 MHz reference clock + 2.4 GHz phase reference signal distributed over coaxial cable
- Multiple boards combinable into larger coherent arrays
- Public datasets with reference positioning labels
- Ultra-low cost compared to commercial radar platforms

This proves the hardware architecture described in this ADR is feasible at the ESP32-S3 price point ($3–5 per node).

### SOTA Displacement Sensitivity

| Technology | Frequency | Displacement Resolution | Range | Cost/Node |
|-----------|-----------|------------------------|-------|-----------|
| Passive WiFi CSI (current) | 2.4/5 GHz | ~0.87 mm (limited by 5° phase noise) | 1–8 m | $3 |
| 802.11bf NDP sounding | 2.4/5/6 GHz | ~0.4 mm (coherent averaging) | 1–8 m | $3 |
| ESPARGOS phase-coherent | 2.4 GHz | ~0.1 mm (8-antenna coherent) | Room-scale | $5 |
| CW Doppler radar (ISM) | 2.4 GHz | ~10 μm | 1–5 m | $15 |
| Infineon BGT60TR13C | 58–63.5 GHz | Sub-mm | Up to 15 m | $20 |
| Vayyar 4D imaging | 3–81 GHz | High (4D imaging) | Room-scale | $200+ |
| Novelda X4 UWB | 7.29/8.748 GHz | Sub-mm | 0.4–10 m | $15–50 |

The gap between passive WiFi CSI (~0.87 mm) and coherent phase processing (~0.1 mm) represents a 9x improvement in displacement sensitivity — the difference between marginal and reliable heartbeat detection at ISM bands.

---

## Decision

We define **Coherent Human Channel Imaging (CHCI)** — a purpose-built coherent RF sensing protocol optimized for structural human motion, vital sign extraction, and body surface reconstruction. CHCI is not WiFi in the traditional sense. It is a sensing protocol that operates within ISM band regulatory constraints and can optionally maintain backward compatibility with 802.11bf.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    CHCI System Architecture                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐              │
│  │  CHCI Node  │     │  CHCI Node  │     │  CHCI Node  │              │
│  │  (TX + RX)  │     │  (TX + RX)  │     │  (TX + RX)  │              │
│  │  ESP32-S3   │     │  ESP32-S3   │     │  ESP32-S3   │              │
│  └──────┬──────┘     └──────┬──────┘     └──────┬──────┘              │
│         │                   │                   │                      │
│         └───────────┬───────┴───────────────────┘                      │
│                     │                                                   │
│            ┌────────┴────────┐                                         │
│            │  Reference Clock │  ← 40 MHz TCXO + PLL distribution     │
│            │  Distribution    │  ← 2.4/5 GHz phase reference          │
│            └────────┬────────┘                                         │
│                     │                                                   │
│  ┌──────────────────┴──────────────────────────────┐                   │
│  │          Waveform Controller                      │                  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐ │                  │
│  │  │ NDP Sound  │  │ Micro-Burst│  │ Chirp Gen  │ │                  │
│  │  │ (802.11bf) │  │ (5 kHz)    │  │ (Multi-BW) │ │                  │
│  │  └────────────┘  └────────────┘  └────────────┘ │                  │
│  │         │              │               │          │                  │
│  │         └──────────────┼───────────────┘          │                  │
│  │                        ▼                          │                  │
│  │              ┌─────────────────┐                  │                  │
│  │              │ Cognitive Engine │ ← Scene state   │                  │
│  │              │ (Waveform Adapt) │   feedback loop │                  │
│  │              └─────────────────┘                  │                  │
│  └───────────────────────────────────────────────────┘                  │
│                        │                                                │
│                        ▼                                                │
│  ┌───────────────────────────────────────────────────┐                  │
│  │          Signal Processing Pipeline                │                 │
│  │  ┌──────────┐  ┌───────────┐  ┌────────────────┐ │                 │
│  │  │ Coherent  │  │ Multi-Band│  │ Diffraction    │ │                 │
│  │  │ Phase     │  │ Fusion    │  │ Tomography     │ │                 │
│  │  │ Alignment │  │ (2.4+5+6) │  │ (Complex CSI)  │ │                 │
│  │  └──────────┘  └───────────┘  └────────────────┘ │                 │
│  │         │              │               │          │                 │
│  │         └──────────────┼───────────────┘          │                 │
│  │                        ▼                          │                 │
│  │              ┌─────────────────┐                  │                 │
│  │              │ Body Model      │                  │                 │
│  │              │ Reconstruction  │ ── DensePose UV  │                 │
│  │              └─────────────────┘                  │                 │
│  └───────────────────────────────────────────────────┘                  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 1. Intentional OFDM Sounding (Replaces Passive CSI Sniffing)

**What changes**: Instead of waiting for random WiFi packets and extracting CSI as a side effect, transmit deterministic OFDM sounding frames at a fixed cadence with known pilot symbol structure.

**Waveform specification**:

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Symbol type | 802.11bf NDP (Null Data PPDU) | Standards-compliant, no data payload overhead |
| Sounding cadence | 50–200 Hz (configurable) | 50 Hz minimum for heartbeat Doppler; 200 Hz for gesture |
| Bandwidth | 20/40/80 MHz (per band) | 20 MHz default; 80 MHz for maximum range resolution |
| Pilot structure | L-LTF + HT-LTF (standard) | Known phase structure enables coherent processing |
| Burst duration | ≤10 ms per sounding event | ETSI EN 300 328 burst limit compliance |
| Subcarrier count | 56 (20 MHz) / 114 (40 MHz) / 242 (80 MHz) | Standard OFDM subcarrier allocation |

**Phase stability improvement**:

```
Passive CSI:     σ_φ ≈ 5° per subcarrier (random MCS, no averaging)
NDP Sounding:    σ_φ ≈ 5° / √N  where N = coherent averages per epoch
                 At 50 Hz cadence, 10-frame average: σ_φ ≈ 1.6°
                 Displacement floor: 0.87 mm → 0.28 mm at 2.4 GHz
```

**Implementation**: New ESP32-S3 firmware mode alongside existing passive CSI. Uses `esp_wifi_80211_tx()` for NDP transmission and existing CSI callback for reception. Sounding schedule coordinated by the Waveform Controller.

### 2. Phase-Locked Dual-Radio Architecture

**What changes**: All CHCI nodes share a common reference clock, eliminating per-node LO phase drift that currently requires statistical correction in `phase_align.rs`.

**Clock distribution design** (based on ESPARGOS architecture):

```
┌──────────────────────────────────────────────────┐
│              Reference Clock Module               │
│                                                    │
│  ┌──────────┐     ┌──────────────┐               │
│  │ 40 MHz   │────▶│ PLL          │               │
│  │ TCXO     │     │ Synthesizer  │               │
│  │ (±0.5ppm)│     │ (SI5351A)    │               │
│  └──────────┘     └──────┬───────┘               │
│                          │                        │
│           ┌──────────────┼──────────────┐        │
│           ▼              ▼              ▼        │
│     ┌──────────┐   ┌──────────┐   ┌──────────┐ │
│     │ 40 MHz   │   │ 40 MHz   │   │ 40 MHz   │ │
│     │ to Node 1│   │ to Node 2│   │ to Node 3│ │
│     └──────────┘   └──────────┘   └──────────┘ │
│                                                    │
│     ┌──────────┐   ┌──────────┐   ┌──────────┐ │
│     │ 2.4 GHz  │   │ 2.4 GHz  │   │ 2.4 GHz  │ │
│     │ Phase Ref│   │ Phase Ref│   │ Phase Ref│ │
│     │ to Node 1│   │ to Node 2│   │ to Node 3│ │
│     └──────────┘   └──────────┘   └──────────┘ │
│                                                    │
│  Distribution: coaxial cable with power splitters  │
│  Phase ref: CW tone at center of operating band    │
└──────────────────────────────────────────────────┘
```

**Components per node** (incremental cost ~$2):

| Component | Part | Cost | Purpose |
|-----------|------|------|---------|
| TCXO | SiT8008 40 MHz ±0.5 ppm | $0.50 | Reference oscillator (1 per system) |
| PLL synthesizer | SI5351A | $1.00 | Generates 40 MHz + 2.4 GHz references (1 per system) |
| Coax splitter | Mini-Circuits PSC-4-1+ | $0.30/port | Distributes reference to nodes |
| SMA connector | Edge-mount | $0.20 | Reference clock input on each node |

**Acceptance metric**: Phase variance per subcarrier under static conditions ≤ 0.5° RMS over 10 minutes (vs current ~5° with statistical correction).

**Impact on displacement sensitivity**:

```
Current (incoherent):     δ_min ≈ λ/(4π) × σ_φ = 12.5cm/(4π) × 5° × π/180 ≈ 0.87 mm
Coherent (shared clock):  δ_min ≈ λ/(4π) × 0.5° × π/180 ≈ 0.087 mm

With 8-antenna coherent averaging:
  δ_min ≈ 0.087 mm / √8 ≈ 0.031 mm
```

This puts heartbeat detection (0.2–0.5 mm chest displacement) well within the sensitivity envelope.

### 3. Multi-Band Coherent Fusion

**What changes**: Transmit sounding frames simultaneously at 2.4 GHz and 5 GHz (optionally 6 GHz with WiFi 6E), fusing them as projections of the same latent motion field in RuVector embedding space.

**Band characteristics for coherent fusion**:

| Property | 2.4 GHz | 5 GHz | 6 GHz |
|----------|---------|-------|-------|
| Wavelength | 12.5 cm | 6.0 cm | 5.0 cm |
| Wall penetration | Excellent | Good | Moderate |
| Displacement sensitivity (0.5° phase) | 0.087 mm | 0.042 mm | 0.035 mm |
| Range resolution (20 MHz) | 7.5 m | 7.5 m | 7.5 m |
| Fresnel zone radius (2 m) | 22.4 cm | 15.5 cm | 14.1 cm |
| Subcarrier spacing (20 MHz) | 312.5 kHz | 312.5 kHz | 312.5 kHz |

**Fusion architecture**:

```
2.4 GHz CSI ──▶ ┌───────────────────┐
                │ Band-Specific      │     ┌─────────────────────┐
                │ Phase Alignment    │────▶│                     │
                │ (per-band ref)     │     │ Contrastive         │
                └───────────────────┘     │ Cross-Band          │
                                          │ Fusion              │
5 GHz CSI ────▶ ┌───────────────────┐     │                     │
                │ Band-Specific      │────▶│ Body model priors   │
                │ Phase Alignment    │     │ constrain phase     │
                │ (per-band ref)     │     │ relationships       │
                └───────────────────┘     │                     │
                                          │ Output: unified     │
6 GHz CSI ────▶ ┌───────────────────┐     │ complex channel     │
  (optional)    │ Band-Specific      │────▶│ response            │
                │ Phase Alignment    │     │                     │
                └───────────────────┘     └─────────────────────┘
                                                    │
                                                    ▼
                                          ┌─────────────────────┐
                                          │ RuVector Contrastive │
                                          │ Embedding Space      │
                                          │ (body surface latent)│
                                          └─────────────────────┘
```

**Key insight**: Lower frequency penetrates better (through-wall sensing, NLOS paths). Higher frequency provides finer spatial resolution. By treating each band as a projection of the same physical scene, the fusion model can achieve super-resolution beyond any single band — using body model priors (known human dimensions, joint angle constraints) to constrain the phase relationships across bands.

**Integration with existing code**: Extends `multiband.rs` from independent per-channel fusion to coherent cross-band phase alignment. The existing `CrossViewpointAttention` mechanism in `ruvector/src/viewpoint/attention.rs` provides the attention-weighted fusion foundation.

### 4. Time-Coded Micro-Bursts

**What changes**: Replace continuous WiFi packet streams with very short deterministic OFDM bursts at high cadence, maximizing temporal resolution of Doppler shifts without 802.11 frame overhead.

**Burst specification**:

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Burst cadence | 1–5 kHz | 5 kHz enables 2.5 kHz Doppler bandwidth (Nyquist) |
| Burst duration | 4–20 μs | Single OFDM symbol + CP = 4 μs minimum |
| Symbols per burst | 1–4 | Minimal overhead per measurement |
| Duty cycle | 0.4–10% | Compliant with ETSI 10 ms burst limit |
| Inter-burst gap | 196–996 μs | Available for normal WiFi traffic |

**Doppler resolution comparison**:

```
Passive WiFi CSI (random, ~30 Hz):
  Doppler resolution: Δf_D = 1/T_obs = 1/33ms ≈ 30 Hz
  Minimum detectable velocity: v_min = λ × Δf_D / 2 ≈ 1.9 m/s at 2.4 GHz

CHCI micro-burst (5 kHz cadence):
  Doppler resolution: Δf_D = 1/(N × T_burst) = 1/(256 × 0.2ms) ≈ 20 Hz
  BUT: unambiguous Doppler: ±2500 Hz → v_max = ±156 m/s
  Minimum detectable velocity: v_min ≈ λ × 20 / 2 ≈ 1.25 m/s

  With coherent integration over 1 second (5000 bursts):
  Δf_D = 1/1s = 1 Hz → v_min ≈ 0.063 m/s (6.3 cm/s)
  Chest wall velocity during breathing: ~1–5 cm/s ✓
  Chest wall velocity during heartbeat: ~0.5–2 cm/s ✓
```

**Regulatory compliance**: At 5 kHz burst cadence with 4 μs bursts, duty cycle is 2%. ETSI EN 300 328 allows up to 10 ms continuous transmission followed by mandatory idle. A 4 μs burst followed by 196 μs idle is well within limits. FCC Part 15.247 requires digital modulation (OFDM qualifies) or spread spectrum.

### 5. MIMO Geometry Optimization

**What changes**: Instead of 2×2 WiFi-style antenna layout (optimized for throughput diversity), design antenna spacing tuned for human-scale wavelengths and chest wall displacement sensitivity.

**Antenna geometry design**:

```
Current WiFi-DensePose (throughput-optimized):
  ┌─────────────────┐
  │  ANT1      ANT2 │  ← λ/2 spacing = 6.25 cm at 2.4 GHz
  │                  │     Optimized for spatial diversity
  │  ESP32-S3       │
  └─────────────────┘

Proposed CHCI (sensing-optimized):
  ┌───────────────────────────────────────┐
  │                                        │
  │  ANT1    ANT2    ANT3    ANT4         │  ← λ/4 spacing = 3.125 cm
  │   ●───────●───────●───────●           │     at 2.4 GHz
  │                                        │     Linear array for 1D AoA
  │  ESP32-S3 (Node A)                    │
  └───────────────────────────────────────┘
        λ/4 = 3.125 cm

  Alternative: L-shaped for 2D AoA:
  ┌────────────────────┐
  │  ANT4              │
  │   ●                │
  │   │ λ/4            │
  │  ANT3              │
  │   ●                │
  │   │ λ/4            │
  │  ANT2              │
  │   ●                │
  │   │ λ/4            │
  │  ANT1──●──ANT5──●──ANT6──●──ANT7    │
  │                                       │
  │  ESP32-S3 (Node A)                   │
  └────────────────────┘
```

**Design rationale**:

| Design parameter | WiFi (throughput) | CHCI (sensing) |
|-----------------|-------------------|----------------|
| Spacing | λ/2 (6.25 cm) | λ/4 (3.125 cm) |
| Goal | Maximize diversity gain | Maximize angular resolution |
| Array factor | Broad main lobe | Narrow main lobe, grating lobe suppression |
| Geometry | Dual-antenna diversity | Linear or L-shaped phased array |
| Target signal | Far-field plane wave | Near-field chest wall displacement |

**Virtual aperture synthesis**: With 4 nodes × 4 antennas = 16 physical elements, MIMO virtual aperture provides 16 × 16 = 256 virtual channels. Combined with MUSIC or ESPRIT algorithms, this enables sub-degree angle-of-arrival estimation — sufficient to resolve individual body segments.

### 6. Cognitive Waveform Adaptation

**What changes**: The sensing waveform adapts in real-time based on the current scene state, driven by delta coherence feedback from the body model.

**Cognitive sensing modes**:

```
┌───────────────────────────────────────────────────────────────┐
│                    Cognitive Waveform Engine                    │
│                                                                │
│  Scene State ─────▶ ┌────────────────┐ ─────▶ Waveform Config │
│  (from body model)  │ Mode Selector  │        (to TX nodes)    │
│                     └───────┬────────┘                         │
│                             │                                  │
│              ┌──────────────┼──────────────────┐              │
│              ▼              ▼                  ▼              │
│     ┌────────────┐  ┌────────────┐    ┌────────────┐         │
│     │   IDLE     │  │   ALERT    │    │   ACTIVE   │         │
│     │            │  │            │    │            │         │
│     │ 1 Hz NDP   │  │ 10 Hz NDP  │    │ 50-200 Hz  │         │
│     │ Single band│  │ Dual band  │    │ All bands  │         │
│     │ Low power  │  │ Med power  │    │ Full power │         │
│     │            │  │            │    │            │         │
│     │ Presence   │  │ Tracking   │    │ DensePose  │         │
│     │ detection  │  │ + coarse   │    │ + vitals   │         │
│     │ only       │  │ pose       │    │ + micro-   │         │
│     │            │  │            │    │ Doppler    │         │
│     └────────────┘  └────────────┘    └────────────┘         │
│           │              │                  │                 │
│           ▼              ▼                  ▼                 │
│     ┌────────────┐  ┌────────────┐    ┌────────────┐         │
│     │   VITAL    │  │   GESTURE  │    │   SLEEP    │         │
│     │            │  │            │    │            │         │
│     │ 100 Hz     │  │ 200 Hz     │    │ 20 Hz      │         │
│     │ Subset of  │  │ Full band  │    │ Single     │         │
│     │ optimal    │  │ Max bursts │    │ band       │         │
│     │ subcarriers│  │            │    │ Low power  │         │
│     │            │  │            │    │            │         │
│     │ Breathing, │  │ DTW match  │    │ Apnea,     │         │
│     │ HR, HRV    │  │ + classify │    │ movement,  │         │
│     │            │  │            │    │ stages     │         │
│     └────────────┘  └────────────┘    └────────────┘         │
│                                                                │
│  Transition triggers:                                          │
│    IDLE → ALERT:   Coherence delta > threshold                │
│    ALERT → ACTIVE: Person detected with confidence > 0.8      │
│    ACTIVE → VITAL: Static person, body model stable           │
│    ACTIVE → GESTURE: Motion spike with periodic structure     │
│    ACTIVE → SLEEP: Supine pose detected, low ambient motion   │
│    * → IDLE:       No detection for 30 seconds                │
│                                                                │
└───────────────────────────────────────────────────────────────┘
```

**Power efficiency**: Cognitive adaptation reduces average power consumption by 60–80% compared to constant full-rate sounding. In IDLE mode (1 Hz, single band, low power), the system draws <10 mA from the ESP32-S3 radio — enabling battery-powered deployment.

**Integration with ADR-039**: The cognitive waveform modes map directly to ADR-039 edge processing tiers. Tier 0 (raw CSI) corresponds to IDLE/ALERT. Tier 1 (phase unwrap, stats) corresponds to ACTIVE. Tier 2 (vitals, fall detection) corresponds to VITAL/SLEEP. The cognitive engine adds the waveform adaptation feedback loop that ADR-039 lacks.

### 7. Coherent Diffraction Tomography

**What changes**: Current tomography (`tomography.rs`) uses amplitude-only attenuation for voxel reconstruction. With coherent phase data from CHCI, we upgrade to diffraction tomography — resolving body surfaces rather than volumetric shadows.

**Mathematical foundation**:

```
Current (amplitude tomography):
  I(x,y,z) = Σ_links |H_measured(f)| × W_link(x,y,z)
  Output: scalar opacity per voxel (shadow image)

Proposed (coherent diffraction tomography):
  O(x,y,z) = F^{-1}[ Σ_links H_measured(f,θ) / H_reference(f,θ) ]
  Where:
    H_measured = complex channel response with human present
    H_reference = complex channel response of empty room (calibration)
    f = frequency (across all bands)
    θ = link angle (across all node pairs)
  Output: complex permittivity contrast per voxel (body surface)
```

**Key advantage**: Diffraction tomography produces body surface geometry, not just occupancy maps. This directly feeds the DensePose UV mapping pipeline with geometric constraints — reducing the neural network's burden from "guess the surface from shadows" to "refine the surface from holographic reconstruction."

**Performance projection** (based on ESPARGOS results and multi-band coverage):

| Metric | Current (Amplitude) | Proposed (Coherent Diffraction) |
|--------|--------------------|---------------------------------|
| Spatial resolution | ~15 cm (limited by wavelength) | ~3 cm (multi-band synthesis) |
| Body segment discrimination | Coarse (torso vs limb) | Fine (individual limbs) |
| Surface vs volume | Volumetric opacity | Surface geometry |
| Through-wall capability | Yes (amplitude penetrates) | Partial (phase coherence degrades) |
| Calibration requirement | None | Empty room reference scan |

### Acceptance Test

**Primary acceptance criterion**: Demonstrate 0.1 mm displacement detection repeatably at 2 meters in a static controlled room.

**Full acceptance test protocol**:

| Test | Metric | Target | Method |
|------|--------|--------|--------|
| AT-1: Phase stability | σ_φ per subcarrier, static, 10 min | ≤ 0.5° RMS | Record CSI, compute variance |
| AT-2: Displacement | Detectable displacement at 2 m | ≤ 0.1 mm | Precision linear stage, sinusoidal motion |
| AT-3: Breathing rate | BPM error, 3 subjects, 5 min each | ≤ 0.2 BPM | Reference: respiratory belt |
| AT-4: Heart rate | BPM error, 3 subjects, seated, 2 min | ≤ 3 BPM | Reference: pulse oximeter |
| AT-5: Multi-person | Pose detection, 3 persons, 4×4 m room | ≥ 90% keypoint detection | Reference: camera ground truth |
| AT-6: Power | Average draw in IDLE mode | ≤ 10 mA (radio) | Current meter on 3.3 V rail |
| AT-7: Latency | End-to-end pose update latency | ≤ 50 ms | Timestamp injection |
| AT-8: Regulatory | Conducted emissions, 2.4 GHz ISM | FCC 15.247 + ETSI 300 328 | Spectrum analyzer |

### Backward Compatibility

**Question 1: Do you want backward compatibility with normal WiFi routers?**

CHCI supports a **dual-mode architecture**:

| Mode | Description | When to Use |
|------|-------------|-------------|
| **Legacy CSI** | Passive sniffing of existing WiFi traffic | Retrofit into existing WiFi environments, no hardware changes |
| **802.11bf NDP** | Standard-compliant NDP sounding | WiFi AP supports 802.11bf, moderate improvement over legacy |
| **CHCI Native** | Full coherent sounding with shared clock | Purpose-deployed sensing mesh, maximum fidelity |

The firmware can switch between modes at runtime. The signal processing pipeline (`signal/src/ruvsense/`) accepts CSI from any mode — the coherent processing path activates when shared-clock metadata is present in the CSI frame header.

**Question 2: Are you willing to own both transmitter and receiver hardware?**

Yes. CHCI requires owning both TX and RX to achieve phase coherence. The system is deployed as a self-contained sensing mesh — not parasitic on existing WiFi infrastructure. This is the fundamental architectural trade: compatibility for control. For sensing, that is a good trade.

### Hardware Bill of Materials (per CHCI node)

| Component | Part | Quantity | Unit Cost | Purpose |
|-----------|------|----------|-----------|---------|
| ESP32-S3-WROOM-1 | Espressif | 1 | $2.50 | Main MCU + WiFi radio |
| External antenna | 2.4/5 GHz dual-band | 2–4 | $0.30 each | Sensing antennas (λ/4 spacing) |
| SMA connector | Edge-mount | 1 | $0.20 | Reference clock input |
| Coax cable | RG-174 | 1 m | $0.15 | Clock distribution |
| PCB | Custom 4-layer | 1 | $0.50 | Integration (at volume) |
| **Node total** | | | **$4.25** | |
| Reference clock module | SI5351A + TCXO + splitter | 1 per system | $3.00 | Shared clock source |
| **4-node system total** | | | **$20.00** | |

This is 10× cheaper than the nearest comparable coherent sensing platform (Novelda X4 at $50/node, Vayyar at $200+).

### Implementation Phases

| Phase | Timeline | Deliverables | Dependencies |
|-------|----------|-------------|--------------|
| **Phase 1: NDP Sounding** | 4 weeks | ESP32-S3 firmware for 802.11bf NDP TX/RX, sounding scheduler, CSI extraction from NDP frames | ESP-IDF 5.2+, existing firmware |
| **Phase 2: Clock Distribution** | 6 weeks | Reference clock PCB design, SI5351A driver, phase reference distribution, `phase_align.rs` upgrade | Phase 1, PCB fabrication |
| **Phase 3: Coherent Processing** | 4 weeks | Coherent diffraction tomography in `tomography.rs`, complex-valued CSI pipeline, calibration procedure | Phase 2 |
| **Phase 4: Multi-Band Fusion** | 4 weeks | Simultaneous 2.4+5 GHz sounding, cross-band phase alignment, contrastive fusion in RuVector space | Phase 1, Phase 3 |
| **Phase 5: Cognitive Engine** | 3 weeks | Waveform adaptation state machine, coherence delta feedback, power management modes | Phase 3, Phase 4 |
| **Phase 6: Acceptance Testing** | 3 weeks | AT-1 through AT-8, precision displacement rig, regulatory pre-scan | Phase 5 |

### Crate Architecture

New and modified crates:

| Crate | Type | Description |
|-------|------|-------------|
| `wifi-densepose-chci` | **New** | CHCI protocol definition, waveform specs, cognitive engine |
| `wifi-densepose-signal` | Modified | Add coherent diffraction tomography, upgrade `phase_align.rs` |
| `wifi-densepose-hardware` | Modified | Reference clock driver, NDP sounding firmware, antenna geometry config |
| `wifi-densepose-ruvector` | Modified | Cross-band contrastive fusion in viewpoint attention |
| `wifi-densepose-wasm-edge` | Modified | New WASM modules for CHCI-specific edge processing |

### Module Impact Matrix

| Existing Module | Current Function | CHCI Upgrade |
|----------------|-----------------|-------------|
| `phase_align.rs` | Statistical LO offset estimation | Replace with shared-clock phase reference alignment |
| `multiband.rs` | Independent per-channel fusion | Coherent cross-band phase alignment with body priors |
| `coherence.rs` | Z-score coherence scoring | Complex-valued coherence metric (phasor domain) |
| `coherence_gate.rs` | Accept/Reject gate decisions | Add waveform adaptation feedback to cognitive engine |
| `tomography.rs` | Amplitude-only ISTA L1 solver | Coherent diffraction tomography with complex CSI |
| `multistatic.rs` | Attention-weighted fusion | Add PLL-disciplined synchronization path |
| `field_model.rs` | SVD room eigenstructure | Coherent room transfer function model with phase |
| `intention.rs` | Pre-movement lead signals | Enhanced micro-Doppler from high-cadence bursts |
| `gesture.rs` | DTW template matching | Phase-domain gesture features (higher discrimination) |

---

## Consequences

### Positive

- **9× displacement sensitivity improvement**: From 0.87 mm (incoherent) to 0.031 mm (coherent 8-antenna) at 2.4 GHz, enabling reliable heartbeat detection at ISM bands
- **Standards-compliant path**: 802.11bf NDP sounding is a published IEEE standard (September 2025), providing regulatory clarity
- **10× cost advantage**: $4.25/node vs $50+ for nearest comparable coherent sensing platform
- **Through-wall preservation**: Operates at 2.4/5 GHz ISM bands, maintaining the through-wall sensing advantage that mmWave systems lack
- **Backward compatible**: Dual-mode firmware supports legacy CSI, 802.11bf NDP, and native CHCI — deployable incrementally
- **Privacy-preserving**: No cameras, no audio — same RF-only sensing paradigm as current WiFi-DensePose
- **Power-efficient**: Cognitive waveform adaptation reduces average power 60–80% vs constant-rate sounding
- **Body surface reconstruction**: Coherent diffraction tomography produces geometric constraints for DensePose, reducing neural network inference burden
- **Proven feasibility**: ESPARGOS demonstrates phase-coherent WiFi sensing at ESP32 cost point (IEEE 2024)

### Negative

- **Custom hardware required**: Cannot parasitically sense from existing WiFi routers in CHCI Native mode (802.11bf mode can use compliant APs)
- **PCB design needed**: Reference clock distribution requires custom PCB — not a pure firmware upgrade
- **Calibration burden**: Coherent diffraction tomography requires empty-room reference scan — adds deployment friction
- **Clock distribution complexity**: Coaxial cable distribution limits deployment flexibility vs fully wireless mesh
- **Two-phase deployment**: Full CHCI requires Phases 1–6 (~24 weeks). Intermediate modes (NDP-only, Phase 1) provide incremental value.

### Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| ESP32-S3 WiFi hardware does not support NDP TX at 802.11bf spec | Medium | High | Fall back to raw 802.11 frame injection with known preamble; validate with `esp_wifi_80211_tx()` |
| Phase coherence degrades over cable length >2 m | Low | Medium | Use matched-length cables; add per-node phase calibration step |
| ETSI/FCC regulatory rejection of custom sounding cadence | Low | High | Stay within 802.11bf NDP specification; use standard-compliant waveforms only |
| Coherent diffraction tomography computationally exceeds ESP32 | Medium | Medium | Run tomography on aggregator (Rust server), not on edge. ESP32 sends coherent CSI only |
| Multi-band simultaneous TX causes self-interference | Medium | Medium | Time-division between bands (alternating 2.4/5 GHz per burst slot) or frequency planning |
| Body model priors over-constrain fusion, missing novel poses | Low | Medium | Use priors as soft constraints (regularization) not hard constraints |

---

## References

### Standards

1. IEEE Std 802.11bf-2025, "Standard for Information Technology — Telecommunications and Information Exchange between Systems — Local and Metropolitan Area Networks — Specific Requirements — Part 11: Wireless LAN Medium Access Control (MAC) and Physical Layer (PHY) Specifications — Amendment: Enhancements for Wireless Local Area Network (WLAN) Sensing," IEEE, September 2025.
2. ETSI EN 300 328 V2.2.2, "Wideband transmission systems; Data transmission equipment operating in the 2.4 GHz band," ETSI, July 2019.
3. FCC 47 CFR Part 15.247, "Operation within the bands 902–928 MHz, 2400–2483.5 MHz, and 5725–5850 MHz."

### Research Papers

4. Euchner, F., et al., "ESPARGOS: An Ultra Low-Cost, Realtime-Capable Multi-Antenna WiFi Channel Sounder for Phase-Coherent Sensing," IEEE, 2024. [arXiv:2502.09405]
5. Restuccia, F., "IEEE 802.11bf: Toward Ubiquitous Wi-Fi Sensing," IEEE Communications Standards Magazine, 2024. [arXiv:2310.05765]
6. Pegoraro, J., et al., "Sensing Performance of the IEEE 802.11bf Protocol," IEEE, 2024. [arXiv:2403.19825]
7. Chen, Y., et al., "Multi-Band Wi-Fi Neural Dynamic Fusion for Sensing," IEEE ICASSP, 2024. [arXiv:2407.12937]
8. Samsung Research, "Optimal Preprocessing of WiFi CSI for Sensing Applications," IEEE, 2024. [arXiv:2307.12126]
9. Yan, Y., et al., "Person-in-WiFi 3D: End-to-End Multi-Person 3D Pose Estimation with Wi-Fi," CVPR 2024.
10. Geng, J., et al., "DensePose From WiFi," Carnegie Mellon University, 2023. [arXiv:2301.00250]
11. Pegoraro, J., et al., "802.11bf Multiband Passive Sensing," IEEE, 2025. [arXiv:2507.22591]
12. Liu, J., et al., "Monitoring Vital Signs and Postures During Sleep Using WiFi Signals," MobiCom, 2020.

### Commercial Systems

13. Vayyar Imaging, "4D Imaging Radar Technology Platform," https://vayyar.com/technology/
14. Infineon Technologies, "BGT60TR13C 60 GHz Radar Sensor IC Datasheet," 2024.
15. Novelda AS, "X4 UWB Radar SoC Datasheet," https://novelda.com/technology/
16. Texas Instruments, "IWR6843 Single-Chip 60-GHz mmWave Sensor," 2024.
17. ESPARGOS Project, https://espargos.net/

### Related ADRs

18. ADR-014: SOTA Signal Processing (phase alignment, coherence scoring)
19. ADR-017: RuVector Signal + MAT Integration (embedding fusion)
20. ADR-029: RuvSense Multistatic Sensing Mode (multi-node coordination)
21. ADR-039: ESP32 Edge Intelligence (tiered processing, power management)
22. ADR-040: WASM Programmable Sensing (edge compute architecture)
23. ADR-041: WASM Module Collection (algorithm registry)
