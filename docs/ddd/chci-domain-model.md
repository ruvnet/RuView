# Coherent Human Channel Imaging (CHCI) Domain Model

## Domain-Driven Design Specification

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Coherent Human Channel Imaging (CHCI)** | A purpose-built RF sensing protocol that uses phase-locked sounding, multi-band fusion, and cognitive waveform adaptation to reconstruct human body surfaces and physiological motion at sub-millimeter resolution |
| **Sounding Frame** | A deterministic OFDM transmission (NDP or custom burst) with known pilot structure, transmitted at fixed cadence for channel measurement — as opposed to passive CSI extracted from data traffic |
| **Phase Coherence** | The property of multiple radio nodes sharing a common phase reference, enabling complex-valued channel measurements without per-node LO drift correction |
| **Reference Clock** | A shared oscillator (TCXO + PLL) distributed to all CHCI nodes via coaxial cable, providing both 40 MHz timing reference and in-band phase reference signal |
| **Cognitive Waveform** | A sounding waveform whose parameters (cadence, bandwidth, band selection, power, subcarrier subset) adapt in real-time based on the current scene state inferred from the body model |
| **Diffraction Tomography** | Coherent reconstruction of body surface geometry from complex-valued channel responses across multiple node pairs and frequency bands — produces surface contours rather than volumetric opacity |
| **Sensing Mode** | One of six operational states (IDLE, ALERT, ACTIVE, VITAL, GESTURE, SLEEP) that determine waveform parameters and processing pipeline configuration |
| **Micro-Burst** | A very short (4–20 μs) deterministic OFDM symbol transmitted at high cadence (1–5 kHz) for maximizing Doppler resolution without full 802.11 frame overhead |
| **Multi-Band Fusion** | Simultaneous sounding at 2.4 GHz and 5 GHz (optionally 6 GHz), fused as projections of the same latent motion field using body model priors as constraints |
| **Displacement Floor** | The minimum detectable surface displacement at a given range, determined by phase noise, coherent averaging depth, and antenna count: δ_min = λ/(4π) × σ_φ/√(N_ant × N_avg) |
| **Channel Contrast** | The ratio of complex channel response with human present to the empty-room reference response — the input to diffraction tomography |
| **Coherence Delta** | The change in phase coherence metric between consecutive observation windows — the trigger signal for cognitive waveform transitions |
| **NDP** | Null Data PPDU — an 802.11bf-standard sounding frame containing only preamble and training fields, no data payload |
| **Sensing Availability Window (SAW)** | An 802.11bf-defined time interval during which NDP sounding exchanges are permitted between sensing initiator and responder |
| **Body Model Prior** | Geometric constraints derived from known human body dimensions (segment lengths, joint angle limits) used to regularize cross-band fusion and tomographic reconstruction |
| **Phase Reference Signal** | A continuous-wave tone at the operating band center frequency, distributed alongside the 40 MHz clock, enabling all nodes to measure and compensate residual phase offset |

---

## Bounded Contexts

### 1. Waveform Generation Context

**Responsibility**: Generating, scheduling, and transmitting deterministic sounding waveforms across all CHCI nodes.

```
┌──────────────────────────────────────────────────────────────┐
│              Waveform Generation Context                      │
├──────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌───────────────┐    ┌───────────────┐    ┌──────────────┐  │
│  │ NDP Sounding  │    │ Micro-Burst   │    │ Chirp        │  │
│  │ Generator     │    │ Generator     │    │ Generator    │  │
│  │ (802.11bf)    │    │ (Custom OFDM) │    │ (Multi-BW)   │  │
│  └───────┬───────┘    └───────┬───────┘    └──────┬───────┘  │
│          │                    │                    │          │
│          └────────────┬───────┴────────────────────┘          │
│                       ▼                                       │
│            ┌──────────────────┐                               │
│            │ Sounding         │                               │
│            │ Scheduler        │ ← Cadence, band, power from  │
│            │ (Aggregate Root) │   Cognitive Engine             │
│            └────────┬─────────┘                               │
│                     │                                         │
│          ┌──────────┴──────────┐                             │
│          ▼                     ▼                             │
│  ┌──────────────┐    ┌──────────────┐                       │
│  │ TX Chain     │    │ TX Chain     │                       │
│  │ (2.4 GHz)   │    │ (5 GHz)      │                       │
│  └──────────────┘    └──────────────┘                       │
│                                                               │
│  Events emitted:                                             │
│    SoundingFrameTransmitted { band, timestamp, seq_id }      │
│    BurstSequenceCompleted { burst_count, duration }           │
│    WaveformConfigChanged { old_mode, new_mode }               │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `SoundingScheduler` (Aggregate Root) — Orchestrates sounding frame transmission across nodes and bands according to the current waveform configuration

**Entities:**
- `SoundingFrame` — A single NDP or micro-burst transmission with sequence ID, band, timestamp, and pilot structure
- `BurstSequence` — An ordered set of micro-bursts within one observation window, used for coherent Doppler integration
- `WaveformConfig` — The current waveform parameter set (cadence, bandwidth, band selection, power level, subcarrier mask)

**Value Objects:**
- `SoundingCadence` — Transmission rate in Hz (1–5000), constrained by regulatory duty cycle limits
- `BandSelection` — Set of active bands {2.4 GHz, 5 GHz, 6 GHz} for current mode
- `SubcarrierMask` — Bit vector selecting active subcarriers for focused sensing (vital mode uses optimal subset)
- `BurstDuration` — Single burst length in microseconds (4–20 μs)
- `DutyCycle` — Computed duty cycle percentage, must not exceed regulatory limit (ETSI: 10 ms max burst)

**Domain Services:**
- `RegulatoryComplianceChecker` — Validates that any waveform configuration satisfies FCC Part 15.247 and ETSI EN 300 328 constraints before applying
- `BandCoordinator` — Manages time-division or simultaneous multi-band sounding to avoid self-interference

---

### 2. Clock Synchronization Context

**Responsibility**: Distributing and maintaining phase-coherent timing across all CHCI nodes in the sensing mesh.

```
┌──────────────────────────────────────────────────────────────┐
│              Clock Synchronization Context                    │
├──────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌───────────────┐                                           │
│  │ Reference      │                                           │
│  │ Clock Module   │ ← TCXO (40 MHz, ±0.5 ppm)               │
│  │ (Aggregate     │                                           │
│  │  Root)         │                                           │
│  └───────┬────────┘                                           │
│          │                                                    │
│  ┌───────┴────────┐                                           │
│  │ PLL Synthesizer│ ← SI5351A: generates 40 MHz clock        │
│  │                │   + 2.4/5 GHz CW phase reference         │
│  └───────┬────────┘                                           │
│          │                                                    │
│    ┌─────┼─────────────────┐                                 │
│    ▼     ▼                 ▼                                 │
│  ┌─────┐ ┌─────┐        ┌─────┐                            │
│  │Node1│ │Node2│  ...   │NodeN│                            │
│  │Phase│ │Phase│        │Phase│                            │
│  │Lock │ │Lock │        │Lock │                            │
│  └──┬──┘ └──┬──┘        └──┬──┘                            │
│     │       │              │                                 │
│     └───────┼──────────────┘                                 │
│             ▼                                                │
│  ┌──────────────────┐                                        │
│  │ Phase Calibration │ ← Measures residual offset            │
│  │ Service           │   per node at startup                 │
│  └──────────────────┘                                        │
│                                                               │
│  Events emitted:                                             │
│    ClockLockAcquired { node_id, offset_ppm }                 │
│    PhaseDriftDetected { node_id, drift_deg_per_min }         │
│    CalibrationCompleted { residual_offsets: Vec<f64> }        │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `ReferenceClockModule` (Aggregate Root) — The single source of timing truth for the entire CHCI mesh

**Entities:**
- `NodePhaseLock` — Per-node state tracking lock status, residual offset, and drift rate
- `CalibrationSession` — A timed procedure that measures and records per-node phase offsets under static conditions

**Value Objects:**
- `PhaseOffset` — Residual phase offset in degrees after clock distribution, per node per subcarrier
- `DriftRate` — Phase drift in degrees per minute, must remain below threshold (0.05°/min for heartbeat sensing)
- `LockStatus` — Enum {Acquiring, Locked, Drifting, Lost} indicating current synchronization state

**Domain Services:**
- `PhaseCalibrationService` — Runs startup and periodic calibration routines; replaces statistical LO estimation in current `phase_align.rs`
- `DriftMonitor` — Continuous background service that detects when any node exceeds drift threshold and triggers recalibration

**Invariants:**
- All nodes must achieve `Locked` status before CHCI sensing begins
- Phase variance per subcarrier must remain ≤ 0.5° RMS over any 10-minute window
- If any node transitions to `Lost`, system falls back to statistical phase correction (legacy mode)

---

### 3. Coherent Signal Processing Context

**Responsibility**: Processing raw coherent CSI into body-surface representations using diffraction tomography and multi-band fusion.

```
┌──────────────────────────────────────────────────────────────────┐
│              Coherent Signal Processing Context                   │
├──────────────────────────────────────────────────────────────────┤
│                                                                    │
│  ┌───────────────┐    ┌───────────────┐    ┌──────────────────┐  │
│  │ Coherent CSI  │    │ Reference     │    │ Calibration      │  │
│  │ Stream        │    │ Channel       │    │ Store            │  │
│  │ (per node     │    │ (empty room)  │    │ (per deployment) │  │
│  │  per band)    │    │               │    │                  │  │
│  └───────┬───────┘    └───────┬───────┘    └────────┬─────────┘  │
│          │                    │                     │            │
│          └────────────┬───────┴─────────────────────┘            │
│                       ▼                                           │
│           ┌───────────────────────┐                              │
│           │ Channel Contrast      │                              │
│           │ Computer              │                              │
│           │ H_c = H_meas / H_ref  │                              │
│           └───────────┬───────────┘                              │
│                       │                                           │
│            ┌──────────┴──────────┐                               │
│            ▼                     ▼                               │
│  ┌──────────────────┐  ┌──────────────────┐                    │
│  │ Diffraction      │  │ Multi-Band       │                    │
│  │ Tomography       │  │ Coherent Fusion  │                    │
│  │ Engine           │  │                  │                    │
│  │ (Aggregate Root) │  │ Body model priors │                    │
│  │                  │  │ as soft           │                    │
│  │ Complex          │  │ constraints       │                    │
│  │ permittivity     │  │                  │                    │
│  │ contrast per     │  │ Cross-band phase  │                    │
│  │ voxel            │  │ alignment         │                    │
│  └────────┬─────────┘  └────────┬─────────┘                    │
│           │                     │                               │
│           └──────────┬──────────┘                               │
│                      ▼                                          │
│           ┌──────────────────┐                                  │
│           │ Body Surface     │──▶ DensePose UV Mapping          │
│           │ Reconstruction   │                                  │
│           └──────────────────┘                                  │
│                                                                  │
│  Events emitted:                                                │
│    VoxelGridUpdated { grid_dims, resolution_cm, timestamp }      │
│    BodySurfaceReconstructed { n_vertices, confidence }           │
│    CoherenceDegradation { node_id, band, severity }              │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `DiffractionTomographyEngine` (Aggregate Root) — Reconstructs 3D body surface geometry from coherent channel contrast measurements across all node pairs and frequency bands

**Entities:**
- `CoherentCsiFrame` — A single coherent channel measurement: complex-valued H(f) per subcarrier, with phase-lock metadata, node ID, band, sequence ID, and timestamp
- `ReferenceChannel` — The empty-room complex channel response per link per band, used as the denominator in channel contrast computation
- `VoxelGrid` — 3D grid of complex permittivity contrast values, the output of diffraction tomography
- `BodySurface` — Extracted iso-surface from voxel grid, represented as triangulated mesh or point cloud

**Value Objects:**
- `ChannelContrast` — Complex ratio H_measured/H_reference per subcarrier per link — the fundamental input to tomography
- `SubcarrierResponse` — Complex-valued (amplitude + phase) channel response at a single subcarrier frequency
- `VoxelCoordinate` — (x, y, z) position in room coordinate frame with associated complex permittivity value
- `SurfaceNormal` — Orientation vector at each surface vertex, derived from permittivity gradient
- `CoherenceMetric` — Complex-valued coherence score (magnitude + phase) replacing the current real-valued Z-score

**Domain Services:**
- `ChannelContrastComputer` — Divides measured channel by reference to isolate human-induced perturbation
- `MultiBandFuser` — Aligns phase across bands using body model priors and combines into unified spectral response
- `SurfaceExtractor` — Applies marching cubes or similar iso-surface algorithm to permittivity contrast grid

**RuVector Integration:**
- `ruvector-attention` → Cross-band attention weights for frequency fusion (extends `CrossViewpointAttention`)
- `ruvector-solver` → Sparse reconstruction for under-determined tomographic inversions
- `ruvector-temporal-tensor` → Temporal coherence of surface reconstructions across frames

---

### 4. Cognitive Waveform Context

**Responsibility**: Adapting the sensing waveform in real-time based on scene state, optimizing the tradeoff between sensing fidelity and power consumption.

```
┌──────────────────────────────────────────────────────────────┐
│              Cognitive Waveform Context                       │
├──────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌───────────────────────────────────────────────────────┐   │
│  │              Scene State Observer                       │   │
│  │                                                         │   │
│  │  Body Model ──▶ ┌──────────────┐                       │   │
│  │                  │ Coherence    │                       │   │
│  │  Coherence   ──▶│ Delta        │──▶ Mode Transition    │   │
│  │  Metrics        │ Analyzer     │    Signal              │   │
│  │                  └──────────────┘                       │   │
│  │  Motion      ──▶                                       │   │
│  │  Classifier                                            │   │
│  └───────────────────────────────────────────────────────┘   │
│                       │                                       │
│                       ▼                                       │
│           ┌───────────────────────┐                           │
│           │ Sensing Mode          │                           │
│           │ State Machine         │                           │
│           │ (Aggregate Root)      │                           │
│           │                       │                           │
│           │ IDLE ──▶ ALERT ──▶ ACTIVE                        │
│           │                   ╱  │  ╲                         │
│           │              VITAL  GESTURE  SLEEP               │
│           │                                                   │
│           └───────────┬───────────┘                           │
│                       │                                       │
│                       ▼                                       │
│           ┌───────────────────────┐                           │
│           │ Waveform Parameter    │                           │
│           │ Computer              │                           │
│           │                       │──▶ WaveformConfig          │
│           │ Mode → {cadence,      │    (to Waveform            │
│           │   bandwidth, bands,   │     Generation Context)    │
│           │   power, subcarriers} │                           │
│           └───────────────────────┘                           │
│                                                               │
│  Events emitted:                                             │
│    SensingModeChanged { from, to, trigger_reason }            │
│    PowerBudgetAdjusted { new_budget_mw, mode }                │
│    SubcarrierSubsetOptimized { selected: Vec<u16>, criterion }│
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `SensingModeStateMachine` (Aggregate Root) — Manages transitions between six sensing modes based on coherence delta, motion classification, and body model state

**Entities:**
- `SensingMode` — One of {IDLE, ALERT, ACTIVE, VITAL, GESTURE, SLEEP} with associated waveform parameter set
- `ModeTransition` — A state change event with trigger reason, timestamp, and hysteresis counter
- `PowerBudget` — Per-mode power allocation constraining cadence and TX power

**Value Objects:**
- `CoherenceDelta` — Magnitude of coherence change between consecutive observation windows — the primary mode transition trigger
- `MotionClassification` — Enum {Static, Breathing, Walking, Gesturing, Falling} derived from micro-Doppler signature
- `ModeHysteresis` — Counter preventing rapid mode oscillation: requires N consecutive trigger events before transition (default N=3)
- `OptimalSubcarrierSet` — The subset of subcarriers with highest SNR for vital sign extraction, computed from recent channel statistics

**Domain Services:**
- `SceneStateObserver` — Fuses body model output, coherence metrics, and motion classifier into a unified scene state descriptor
- `ModeTransitionEvaluator` — Applies hysteresis and priority rules to determine if a mode change should occur
- `SubcarrierSelector` — Identifies optimal subcarrier subset for vital mode using Fisher information criterion or SNR ranking
- `PowerManager` — Computes TX power and duty cycle to stay within regulatory and battery constraints per mode

**Invariants:**
- IDLE mode must be entered after 30 seconds of no detection (configurable)
- Mode transitions must satisfy hysteresis: ≥3 consecutive trigger events
- Power budget must never exceed regulatory limit (20 dBm EIRP at 2.4 GHz)
- Subcarrier subset in VITAL mode must include ≥16 subcarriers for statistical reliability

---

### 5. Displacement Measurement Context

**Responsibility**: Extracting sub-millimeter physiological displacement (breathing, heartbeat, tremor) from coherent phase time series.

```
┌──────────────────────────────────────────────────────────────┐
│              Displacement Measurement Context                 │
├──────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌──────────────┐                                            │
│  │ Phase Time    │ ← Coherent CSI phase per subcarrier       │
│  │ Series Buffer │   per link, at sounding cadence           │
│  └──────┬───────┘                                            │
│         │                                                     │
│         ▼                                                     │
│  ┌──────────────────┐                                        │
│  │ Phase-to-         │                                        │
│  │ Displacement      │                                        │
│  │ Converter         │                                        │
│  │ δ = λΔφ / (4π)    │                                        │
│  └──────┬────────────┘                                        │
│         │                                                     │
│  ┌──────┴──────────────────────────┐                         │
│  │                                  │                         │
│  ▼                                  ▼                         │
│  ┌──────────────────┐  ┌──────────────────┐                 │
│  │ Respiratory       │  │ Cardiac          │                 │
│  │ Analyzer          │  │ Analyzer         │                 │
│  │ (Aggregate Root)  │  │                  │                 │
│  │                   │  │ Bandpass:        │                 │
│  │ Bandpass:         │  │ 0.8–3.0 Hz      │                 │
│  │ 0.1–0.6 Hz       │  │ (48–180 BPM)    │                 │
│  │ (6–36 BPM)       │  │                  │                 │
│  │                   │  │ Harmonic cancel  │                 │
│  │ Amplitude: 4–12mm │  │ (remove respir.  │                 │
│  │                   │  │  harmonics)      │                 │
│  └────────┬──────────┘  │                  │                 │
│           │             │ Amplitude:       │                 │
│           │             │ 0.2–0.5 mm       │                 │
│           │             └────────┬─────────┘                 │
│           │                      │                            │
│           └──────────┬───────────┘                            │
│                      ▼                                        │
│           ┌──────────────────┐                               │
│           │ Vital Signs      │                               │
│           │ Fusion           │──▶ VitalSignReport             │
│           │ (multi-link,     │                               │
│           │  multi-band)     │                               │
│           └──────────────────┘                               │
│                                                               │
│  Events emitted:                                             │
│    BreathingRateEstimated { bpm, confidence, method }         │
│    HeartRateEstimated { bpm, confidence, hrv_ms }             │
│    ApneaEventDetected { duration_s, severity }                │
│    DisplacementAnomaly { max_displacement_mm, location }      │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `RespiratoryAnalyzer` (Aggregate Root) — Extracts breathing rate and pattern from 0.1–0.6 Hz displacement band

**Entities:**
- `PhaseTimeSeries` — Windowed buffer of unwrapped phase values per subcarrier per link, at sounding cadence
- `DisplacementTimeSeries` — Converted from phase: δ(t) = λΔφ(t) / (4π), represents physical surface displacement in mm
- `VitalSignReport` — Fused output containing breathing rate, heart rate, HRV, confidence scores, and anomaly flags

**Value Objects:**
- `PhaseUnwrapped` — Continuous (unwrapped) phase in radians, free from 2π ambiguity
- `DisplacementSample` — Single displacement value in mm with timestamp and confidence
- `BreathingRate` — BPM value (6–36 range) with confidence score
- `HeartRate` — BPM value (48–180 range) with confidence score and HRV interval
- `ApneaEvent` — Duration, severity, and confidence of detected breathing cessation

**Domain Services:**
- `PhaseUnwrapper` — Continuous phase unwrapping with outlier rejection; critical for displacement conversion
- `RespiratoryHarmonicCanceller` — Removes breathing harmonics from cardiac band to isolate heartbeat signal
- `MultilinkFuser` — Combines displacement estimates across node pairs using SNR-weighted averaging
- `AnomalyDetector` — Flags displacement patterns inconsistent with normal physiology (fall, seizure, cardiac arrest)

**Invariants:**
- Phase unwrapping must maintain continuity: |Δφ| < π between consecutive samples
- Displacement floor must be validated against acceptance metric (AT-2: ≤ 0.1 mm at 2 m)
- Heart rate estimation requires minimum 10 seconds of stable data (cardiac analyzer warmup)
- Multi-link fusion must use ≥2 independent links for confidence scoring

---

### 6. Regulatory Compliance Context

**Responsibility**: Ensuring all CHCI transmissions comply with applicable ISM band regulations across deployment jurisdictions.

```
┌──────────────────────────────────────────────────────────────┐
│              Regulatory Compliance Context                    │
├──────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌───────────────┐    ┌───────────────┐    ┌──────────────┐  │
│  │ FCC Part 15   │    │ ETSI EN       │    │ 802.11bf     │  │
│  │ Rules         │    │ 300 328       │    │ Compliance   │  │
│  │               │    │               │    │              │  │
│  │ - 30 dBm max  │    │ - 20 dBm EIRP│    │ - NDP format │  │
│  │ - Digital mod │    │ - LBT or 10ms │    │ - SAW window │  │
│  │ - Spread      │    │   burst max   │    │ - SMS setup  │  │
│  │   spectrum    │    │ - Duty cycle  │    │              │  │
│  └───────┬───────┘    └───────┬───────┘    └──────┬───────┘  │
│          │                    │                    │          │
│          └────────────┬───────┴────────────────────┘          │
│                       ▼                                       │
│            ┌──────────────────┐                               │
│            │ Compliance       │                               │
│            │ Validator        │                               │
│            │ (Aggregate Root) │                               │
│            │                  │                               │
│            │ Validates every  │                               │
│            │ WaveformConfig   │                               │
│            │ before TX        │                               │
│            └────────┬─────────┘                               │
│                     │                                         │
│                     ▼                                         │
│            ┌──────────────────┐                               │
│            │ Jurisdiction     │                               │
│            │ Registry         │                               │
│            │                  │                               │
│            │ US → FCC         │                               │
│            │ EU → ETSI        │                               │
│            │ JP → ARIB        │                               │
│            │ ...              │                               │
│            └──────────────────┘                               │
│                                                               │
│  Events emitted:                                             │
│    ComplianceCheckPassed { jurisdiction, config_hash }         │
│    ComplianceViolation { rule, parameter, value, limit }       │
│    JurisdictionChanged { from, to }                           │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `ComplianceValidator` (Aggregate Root) — Gate that must approve every waveform configuration before transmission is permitted

**Entities:**
- `JurisdictionProfile` — Complete set of regulatory constraints for a given region (FCC, ETSI, ARIB, etc.)
- `ComplianceRecord` — Audit trail of compliance checks with timestamps and configuration hashes

**Value Objects:**
- `MaxEIRP` — Maximum effective isotropic radiated power in dBm, per band per jurisdiction
- `MaxBurstDuration` — Maximum continuous transmission time (ETSI: 10 ms)
- `MinIdleTime` — Minimum idle period between bursts
- `ModulationType` — Must be digital modulation (OFDM qualifies) or spread spectrum for FCC
- `DutyCycleLimit` — Maximum percentage of time occupied by transmissions

**Invariants:**
- No transmission shall occur without a passing `ComplianceCheckPassed` event
- Duty cycle must be recalculated and validated on every cadence change
- Jurisdiction must be set during deployment configuration; default is most restrictive (ETSI)

---

## Core Domain Entities

### CoherentCsiFrame (Entity)

```rust
pub struct CoherentCsiFrame {
    /// Unique sequence identifier for this sounding frame
    seq_id: u64,
    /// Node that received this frame
    rx_node_id: NodeId,
    /// Node that transmitted this frame (known from sounding schedule)
    tx_node_id: NodeId,
    /// Frequency band: Band2_4GHz, Band5GHz, Band6GHz
    band: FrequencyBand,
    /// UTC timestamp with microsecond precision
    timestamp_us: u64,
    /// Complex channel response per subcarrier: (amplitude, phase) pairs
    subcarrier_responses: Vec<Complex64>,
    /// Phase lock status at time of capture
    phase_lock: LockStatus,
    /// Residual phase offset from calibration (degrees)
    residual_offset_deg: f64,
    /// Signal-to-noise ratio estimate (dB)
    snr_db: f32,
    /// Sounding mode that produced this frame
    source_mode: SoundingMode,
}
```

**Invariants:**
- `phase_lock` must be `Locked` for frame to be used in coherent processing
- `subcarrier_responses.len()` must match expected count for `band` and bandwidth (56 for 20 MHz)
- `snr_db` must be ≥ 10 dB for frame to contribute to displacement estimation
- `timestamp_us` must be monotonically increasing per `rx_node_id`

### WaveformConfig (Value Object)

```rust
pub struct WaveformConfig {
    /// Active sensing mode
    mode: SensingMode,
    /// Sounding cadence in Hz
    cadence_hz: f64,
    /// Active frequency bands
    bands: BandSet,
    /// Bandwidth per band
    bandwidth_mhz: u8,
    /// Transmit power in dBm
    tx_power_dbm: f32,
    /// Subcarrier mask (None = all subcarriers active)
    subcarrier_mask: Option<BitVec>,
    /// Burst duration in microseconds
    burst_duration_us: u16,
    /// Number of symbols per burst
    symbols_per_burst: u8,
    /// Computed duty cycle (must pass compliance check)
    duty_cycle_pct: f64,
}
```

**Invariants:**
- `cadence_hz` must be ≥ 1.0 and ≤ 5000.0
- `duty_cycle_pct` must not exceed jurisdiction limit (ETSI: derived from 10 ms burst max)
- `tx_power_dbm` must not exceed jurisdiction max EIRP
- `bandwidth_mhz` must be one of {20, 40, 80}
- `burst_duration_us` must be ≥ 4 (single OFDM symbol + CP)

### SensingMode (Value Object)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensingMode {
    /// 1 Hz, single band, presence detection only
    Idle,
    /// 10 Hz, dual band, coarse tracking
    Alert,
    /// 50-200 Hz, all bands, full DensePose + vitals
    Active,
    /// 100 Hz, optimal subcarrier subset, breathing + HR + HRV
    Vital,
    /// 200 Hz, full band, DTW gesture classification
    Gesture,
    /// 20 Hz, single band, apnea/movement/stage detection
    Sleep,
}

impl SensingMode {
    pub fn default_config(&self) -> WaveformConfig {
        match self {
            Self::Idle => WaveformConfig {
                mode: *self,
                cadence_hz: 1.0,
                bands: BandSet::single(Band::Band2_4GHz),
                bandwidth_mhz: 20,
                tx_power_dbm: 10.0,
                subcarrier_mask: None,
                burst_duration_us: 4,
                symbols_per_burst: 1,
                duty_cycle_pct: 0.0004,
            },
            Self::Alert => WaveformConfig {
                mode: *self,
                cadence_hz: 10.0,
                bands: BandSet::dual(Band::Band2_4GHz, Band::Band5GHz),
                bandwidth_mhz: 20,
                tx_power_dbm: 15.0,
                subcarrier_mask: None,
                burst_duration_us: 8,
                symbols_per_burst: 2,
                duty_cycle_pct: 0.008,
            },
            Self::Active => WaveformConfig {
                mode: *self,
                cadence_hz: 100.0,
                bands: BandSet::all(),
                bandwidth_mhz: 40,
                tx_power_dbm: 20.0,
                subcarrier_mask: None,
                burst_duration_us: 16,
                symbols_per_burst: 4,
                duty_cycle_pct: 0.16,
            },
            Self::Vital => WaveformConfig {
                mode: *self,
                cadence_hz: 100.0,
                bands: BandSet::dual(Band::Band2_4GHz, Band::Band5GHz),
                bandwidth_mhz: 20,
                tx_power_dbm: 18.0,
                subcarrier_mask: Some(optimal_vital_subcarriers()),
                burst_duration_us: 8,
                symbols_per_burst: 2,
                duty_cycle_pct: 0.08,
            },
            Self::Gesture => WaveformConfig {
                mode: *self,
                cadence_hz: 200.0,
                bands: BandSet::all(),
                bandwidth_mhz: 40,
                tx_power_dbm: 20.0,
                subcarrier_mask: None,
                burst_duration_us: 16,
                symbols_per_burst: 4,
                duty_cycle_pct: 0.32,
            },
            Self::Sleep => WaveformConfig {
                mode: *self,
                cadence_hz: 20.0,
                bands: BandSet::single(Band::Band2_4GHz),
                bandwidth_mhz: 20,
                tx_power_dbm: 12.0,
                subcarrier_mask: None,
                burst_duration_us: 4,
                symbols_per_burst: 1,
                duty_cycle_pct: 0.008,
            },
        }
    }
}
```

### VitalSignReport (Value Object)

```rust
pub struct VitalSignReport {
    /// Timestamp of this report
    timestamp_us: u64,
    /// Breathing rate in BPM (None if not measurable)
    breathing_bpm: Option<f64>,
    /// Breathing confidence [0.0, 1.0]
    breathing_confidence: f64,
    /// Heart rate in BPM (None if not measurable — requires CHCI coherent mode)
    heart_rate_bpm: Option<f64>,
    /// Heart rate confidence [0.0, 1.0]
    heart_rate_confidence: f64,
    /// Heart rate variability: RMSSD in milliseconds
    hrv_rmssd_ms: Option<f64>,
    /// Detected anomalies
    anomalies: Vec<VitalAnomaly>,
    /// Number of independent links contributing to this estimate
    contributing_links: u16,
    /// Sensing mode that produced this report
    source_mode: SensingMode,
}

pub enum VitalAnomaly {
    Apnea { duration_s: f64, severity: Severity },
    Tachycardia { bpm: f64 },
    Bradycardia { bpm: f64 },
    IrregularRhythm { irregularity_score: f64 },
    FallDetected { impact_g: f64 },
    NoMotion { duration_s: f64 },
}
```

### NodeId and FrequencyBand (Value Objects)

```rust
/// Unique identifier for a CHCI node in the sensing mesh
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u8);

/// Operating frequency band
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyBand {
    /// 2.4 GHz ISM band (2400-2483.5 MHz), λ = 12.5 cm
    Band2_4GHz,
    /// 5 GHz UNII band (5150-5850 MHz), λ = 6.0 cm
    Band5GHz,
    /// 6 GHz band (5925-7125 MHz), λ = 5.0 cm, WiFi 6E
    Band6GHz,
}

impl FrequencyBand {
    pub fn wavelength_m(&self) -> f64 {
        match self {
            Self::Band2_4GHz => 0.125,
            Self::Band5GHz => 0.060,
            Self::Band6GHz => 0.050,
        }
    }

    /// Displacement per radian of phase change: λ/(4π)
    pub fn displacement_per_radian_mm(&self) -> f64 {
        self.wavelength_m() * 1000.0 / (4.0 * std::f64::consts::PI)
    }
}
```

---

## Domain Events

### Waveform Events

```rust
pub enum WaveformEvent {
    /// A sounding frame was transmitted
    SoundingFrameTransmitted {
        seq_id: u64,
        tx_node: NodeId,
        band: FrequencyBand,
        timestamp_us: u64,
    },
    /// A burst sequence completed (micro-burst mode)
    BurstSequenceCompleted {
        burst_count: u32,
        total_duration_us: u64,
    },
    /// Waveform configuration changed (mode transition)
    WaveformConfigChanged {
        old_mode: SensingMode,
        new_mode: SensingMode,
        trigger: ModeTransitionTrigger,
    },
}

pub enum ModeTransitionTrigger {
    CoherenceDeltaThreshold { delta: f64 },
    PersonDetected { confidence: f64 },
    PersonLost { absence_duration_s: f64 },
    PoseClassification { pose: PoseClass },
    MotionSpike { magnitude: f64 },
    Manual,
}
```

### Clock Events

```rust
pub enum ClockEvent {
    /// A node achieved phase lock
    ClockLockAcquired {
        node_id: NodeId,
        residual_offset_deg: f64,
    },
    /// Phase drift detected on a node
    PhaseDriftDetected {
        node_id: NodeId,
        drift_deg_per_min: f64,
    },
    /// Phase lock lost on a node — triggers fallback to statistical correction
    ClockLockLost {
        node_id: NodeId,
        reason: LockLossReason,
    },
    /// Calibration procedure completed
    CalibrationCompleted {
        residual_offsets: Vec<(NodeId, f64)>,
        max_residual_deg: f64,
    },
}
```

### Measurement Events

```rust
pub enum MeasurementEvent {
    /// Body surface reconstructed from diffraction tomography
    BodySurfaceReconstructed {
        n_vertices: u32,
        resolution_cm: f64,
        confidence: f64,
        timestamp_us: u64,
    },
    /// Vital signs estimated
    VitalSignsUpdated {
        report: VitalSignReport,
    },
    /// Displacement anomaly detected
    DisplacementAnomaly {
        max_displacement_mm: f64,
        anomaly_type: VitalAnomaly,
    },
    /// Coherence degradation on a link (may trigger recalibration)
    CoherenceDegradation {
        tx_node: NodeId,
        rx_node: NodeId,
        band: FrequencyBand,
        severity: Severity,
    },
}
```

---

## Context Map

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         CHCI Context Map                                │
│                                                                         │
│  ┌────────────────┐         ┌────────────────┐                         │
│  │   Waveform     │ ◀─────  │   Cognitive    │                         │
│  │   Generation   │ config  │   Waveform     │                         │
│  │   Context      │         │   Context      │                         │
│  └───────┬────────┘         └───────▲────────┘                         │
│          │                          │                                   │
│          │ sounding                 │ scene state                       │
│          │ frames                   │ feedback                          │
│          ▼                          │                                   │
│  ┌────────────────┐         ┌───────┴────────┐                         │
│  │   Clock        │ phase   │   Coherent     │                         │
│  │   Synchro-     │ lock ──▶│   Signal       │                         │
│  │   nization     │ status  │   Processing   │                         │
│  │   Context      │         │   Context      │                         │
│  └────────────────┘         └───────┬────────┘                         │
│                                     │                                   │
│                              body surface,                              │
│                              coherence metrics                          │
│                                     │                                   │
│                                     ▼                                   │
│                             ┌────────────────┐                         │
│                             │  Displacement   │                         │
│                             │  Measurement    │                         │
│                             │  Context        │                         │
│                             └────────────────┘                         │
│                                                                         │
│  ┌────────────────┐                                                    │
│  │  Regulatory    │ ◀── validates all WaveformConfig before TX         │
│  │  Compliance    │                                                    │
│  │  Context       │                                                    │
│  └────────────────┘                                                    │
│                                                                         │
│  ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─       │
│  Integration with existing WiFi-DensePose bounded contexts:             │
│                                                                         │
│  ┌────────────────┐    ┌────────────────┐    ┌────────────────┐       │
│  │  RuvSense      │    │  RuVector      │    │  DensePose     │       │
│  │  Multistatic   │    │  Cross-View    │    │  Body Model    │       │
│  │  (ADR-029)     │    │  Fusion        │    │  (Core)        │       │
│  └────────────────┘    └────────────────┘    └────────────────┘       │
│                                                                         │
│  CHCI Signal Processing feeds directly into existing                   │
│  RuvSense/RuVector/DensePose pipeline — coherent CSI                   │
│  replaces incoherent CSI as input, same output interface               │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Anti-Corruption Layers

| Boundary | Direction | Mechanism |
|----------|-----------|-----------|
| CHCI Signal Processing → RuvSense | Downstream | `CoherentCsiFrame` adapts to existing `CsiFrame` trait via `IntoLegacyCsi` adapter — existing pipeline works unmodified |
| Cognitive Waveform → ADR-039 Edge Tiers | Bidirectional | Sensing modes map to edge tiers: IDLE→Tier0, ACTIVE→Tier1, VITAL→Tier2. Shared `EdgeConfig` value object |
| Clock Synchronization → Hardware | Downstream | `ClockDriver` trait abstracts SI5351A hardware specifics; mock implementation for testing |
| Regulatory Compliance → All TX Contexts | Upstream | Compliance Validator acts as a policy gateway — no transmission without passing check |

---

## Integration with Existing Codebase

### Modified Modules

| File | Current | CHCI Change |
|------|---------|-------------|
| `signal/src/ruvsense/phase_align.rs` | Statistical LO offset estimation via circular mean | Add `SharedClockAligner` path: when `phase_lock == Locked`, skip statistical estimation, apply only residual calibration offset |
| `signal/src/ruvsense/multiband.rs` | Independent per-channel fusion | Add `CoherentCrossBandFuser`: phase-aligns across bands using body model priors before fusion |
| `signal/src/ruvsense/coherence.rs` | Z-score coherence scoring (real-valued) | Add `ComplexCoherenceMetric`: phasor-domain coherence using both magnitude and phase information |
| `signal/src/ruvsense/tomography.rs` | Amplitude-only ISTA L1 solver | Add `DiffractionTomographyEngine`: complex-valued reconstruction using channel contrast |
| `signal/src/ruvsense/coherence_gate.rs` | Accept/Reject gate decisions | Add cognitive waveform feedback: gate decisions emit `CoherenceDelta` events to mode state machine |
| `signal/src/ruvsense/multistatic.rs` | Attention-weighted fusion | Add clock synchronization status as fusion weight modifier |
| `hardware/src/esp32/` | TDM protocol, channel hopping | Add NDP sounding mode, reference clock driver, phase reference input |
| `ruvector/src/viewpoint/attention.rs` | CrossViewpointAttention | Extend to cross-band attention with frequency-dependent geometric bias |

### New Crate: `wifi-densepose-chci`

```
wifi-densepose-chci/
├── src/
│   ├── lib.rs                    # Crate root, re-exports
│   ├── waveform/
│   │   ├── mod.rs
│   │   ├── ndp_generator.rs      # 802.11bf NDP sounding frame generation
│   │   ├── burst_generator.rs    # Micro-burst OFDM symbol generation
│   │   ├── scheduler.rs          # Sounding schedule orchestration
│   │   └── compliance.rs         # Regulatory compliance validation
│   ├── clock/
│   │   ├── mod.rs
│   │   ├── reference.rs          # Reference clock module abstraction
│   │   ├── pll_driver.rs         # SI5351A PLL synthesizer driver
│   │   ├── calibration.rs        # Phase calibration procedures
│   │   └── drift_monitor.rs      # Continuous drift detection
│   ├── cognitive/
│   │   ├── mod.rs
│   │   ├── mode.rs               # SensingMode enum and transitions
│   │   ├── state_machine.rs      # Mode state machine with hysteresis
│   │   ├── scene_observer.rs     # Scene state fusion from body model + coherence
│   │   ├── subcarrier_select.rs  # Optimal subcarrier subset for vital mode
│   │   └── power_manager.rs      # Power budget per mode
│   ├── tomography/
│   │   ├── mod.rs
│   │   ├── contrast.rs           # Channel contrast computation
│   │   ├── diffraction.rs        # Coherent diffraction tomography engine
│   │   └── surface.rs            # Iso-surface extraction (marching cubes)
│   ├── displacement/
│   │   ├── mod.rs
│   │   ├── phase_to_disp.rs      # Phase-to-displacement conversion
│   │   ├── respiratory.rs        # Breathing rate analyzer
│   │   ├── cardiac.rs            # Heart rate + HRV analyzer
│   │   └── anomaly.rs            # Vital sign anomaly detection
│   └── types.rs                  # Shared types (NodeId, FrequencyBand, etc.)
├── Cargo.toml
└── tests/
    ├── integration/
    │   ├── acceptance_tests.rs   # AT-1 through AT-8
    │   └── mode_transitions.rs   # Cognitive state machine tests
    └── unit/
        ├── compliance_tests.rs
        ├── displacement_tests.rs
        └── tomography_tests.rs
```
