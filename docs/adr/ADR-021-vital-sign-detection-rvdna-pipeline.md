# ADR-021: Vital Sign Detection via rvdna Signal Processing Pipeline

| Field | Value |
|-------|-------|
| **Status** | Partially Implemented |
| **Date** | 2026-02-28 |
| **Deciders** | ruv |
| **Relates to** | ADR-014 (SOTA Signal Processing), ADR-017 (RuVector-Signal-MAT), ADR-019 (Sensing-Only UI), ADR-020 (Rust RuVector AI Model Migration) |

## Context

### The Need for Vital Sign Detection

WiFi-based vital sign monitoring is a rapidly maturing field. Channel State Information (CSI) captures fine-grained multipath propagation changes caused by physiological movements -- chest displacement from respiration (1-5 mm amplitude, 0.1-0.5 Hz) and body surface displacement from cardiac activity (0.1-0.5 mm, 0.8-2.0 Hz). Our existing WiFi-DensePose project already implements motion detection, presence sensing, and body velocity profiling (BVP), but lacks a dedicated vital sign extraction pipeline.

Vital sign detection extends the project's value from occupancy sensing into health monitoring, enabling contactless respiratory rate and heart rate estimation for applications in eldercare, sleep monitoring, disaster survivor detection (ADR-001), and clinical triage.

### What rvdna (RuVector DNA) Offers

The `vendor/ruvector` codebase provides a rich set of signal processing primitives that map directly to vital sign detection requirements. Rather than building from scratch, we can compose existing rvdna components into a vital sign pipeline. The key crates and their relevance:

| Crate | Key Primitives | Vital Sign Relevance |
|-------|---------------|---------------------|
| `ruvector-temporal-tensor` | `TemporalTensorCompressor`, `TieredStore`, `TierPolicy`, tiered quantization (8/7/5/3-bit) | Stores compressed CSI temporal streams with adaptive precision -- hot (real-time vital signs) at 8-bit, warm (historical) at 5-bit, cold (archive) at 3-bit |
| `ruvector-nervous-system` | `PredictiveLayer`, `OscillatoryRouter`, `GlobalWorkspace`, `DVSEvent`, `EventRingBuffer`, `ShardedEventBus`, `EpropSynapse`, `Dendrite`, `ModernHopfield` | Predictive coding suppresses static CSI components (90-99% bandwidth reduction), oscillatory routing isolates respiratory vs cardiac frequency bands, event bus handles high-throughput CSI streams |
| `ruvector-attention` | `ScaledDotProductAttention`, Mixture of Experts (MoE), PDE attention, sparse attention | Attention-weighted subcarrier selection for vital sign sensitivity, already used in BVP extraction |
| `ruvector-coherence` | `SpectralCoherenceScore`, `HnswHealthMonitor`, spectral gap estimation, Fiedler value | Spectral analysis of CSI time series, coherence between subcarrier pairs for breathing/heartbeat isolation |
| `ruvector-gnn` | `GnnLayer`, `Linear`, `LayerNorm`, graph attention, EWC training | Graph neural network over subcarrier correlation topology, learning which subcarrier groups carry vital sign information |
| `ruvector-core` | `VectorDB`, HNSW index, SIMD distance, quantization | Fingerprint-based pattern matching of vital sign waveform templates |
| `sona` | `SonaEngine`, `TrajectoryBuilder`, micro-LoRA, EWC++ | Self-optimizing adaptation of vital sign extraction parameters per environment |
| `ruvector-sparse-inference` | Sparse model execution, precision management | Efficient inference on edge devices with constrained compute |
| `ruQu` | `FilterPipeline` (Structural/Shift/Evidence), `AdaptiveThresholds` (Welford, EMA, CUSUM-style), `DriftDetector` (step-change, variance expansion, oscillation), `QuantumFabric` (256-tile parallel processing) | **Three-filter decision pipeline** for vital sign gating -- structural filter detects signal partition/degradation, shift filter catches distribution drift in vital sign baselines, evidence filter provides anytime-valid statistical rigor. `DriftDetector` directly detects respiratory/cardiac parameter drift. `AdaptiveThresholds` self-tunes anomaly thresholds with outcome feedback (precision/recall/F1). 256-tile fabric maps to parallel subcarrier processing. |
| DNA example (`examples/dna`) | `BiomarkerProfile`, `StreamProcessor`, `RingBuffer`, `BiomarkerReading`, z-score anomaly detection, CUSUM changepoint detection, EMA, trend analysis | Direct analog -- the biomarker streaming engine processes time-series health data with anomaly detection, which maps exactly to vital sign monitoring |

### Current Project State

The Rust port (`rust-port/wifi-densepose-rs/`) already contains:

- **`wifi-densepose-signal`**: CSI processing, BVP extraction, phase sanitization, Hampel filter, spectrogram generation, Fresnel geometry, motion detection, subcarrier selection
- **`wifi-densepose-sensing-server`**: Axum server receiving ESP32 CSI frames (UDP 5005), WebSocket broadcasting sensing updates, signal field generation, with three data source modes:
  - **ESP32 mode** (`--source esp32`): Receives ADR-018 binary frames via UDP `:5005`. Frame format: magic `0xC511_0001`, 20-byte header (`node_id`, `n_antennas`, `n_subcarriers`, `freq_mhz`, `sequence`, `rssi`, `noise_floor`), packed I/Q pairs. The `parse_esp32_frame()` function extracts amplitude (`sqrt(I^2+Q^2)`) and phase (`atan2(Q,I)`) per subcarrier. ESP32 mode also runs a `broadcast_tick_task` for re-broadcasting buffered state to WebSocket clients between frames.
  - **Windows WiFi mode** (`--source wifi`): Uses `netsh wlan show interfaces` to extract RSSI/signal% and creates pseudo-single-subcarrier frames. Useful for development but lacks multi-subcarrier CSI.
  - **Simulation mode** (`--source simulate`): Generates synthetic 56-subcarrier frames with sinusoidal amplitude/phase variation. Used for UI testing.
- **Auto-detection**: `main()` probes ESP32 UDP first, then Windows WiFi, then falls back to simulation. The vital sign module must integrate with all three modes but will only produce meaningful HR/RR in ESP32 mode (multi-subcarrier CSI).
- **Existing features used by vitals**: `extract_features_from_frame()` already computes `breathing_band_power` (low-frequency subcarrier variance) and `motion_band_power` (high-frequency variance). The `generate_signal_field()` function already models a `breath_ring` modulated by variance and tick. These serve as integration anchors for the vital sign pipeline.
- **Existing ADR-019/020**: Sensing-only UI mode with Three.js visualization and Rust migration plan

What is missing is a dedicated vital sign extraction stage between the CSI processing pipeline and the UI visualization.

## Decision

Implement a **vital sign detection module** as a new crate `wifi-densepose-vitals` within the Rust port workspace, composed from rvdna primitives. The module extracts heart rate (HR) and respiratory rate (RR) from WiFi CSI data and integrates with the existing sensing server and UI.

### Core Design Principles

1. **Composition over invention**: Use existing rvdna crates as building blocks rather than reimplementing signal processing from scratch.
2. **Streaming-first architecture**: Process CSI frames as they arrive using ring buffers and event-driven processing, modeled on the `biomarker_stream::StreamProcessor` pattern.
3. **Environment-adaptive**: Use SONA's self-optimizing loop to adapt extraction parameters (filter cutoffs, subcarrier weights, noise thresholds) per deployment.
4. **Tiered storage**: Use `ruvector-temporal-tensor` to store vital sign time series at variable precision based on access patterns.
5. **Privacy by design**: All processing is local and on-device; no raw CSI data leaves the device.

## Architecture

### Component Diagram

```
                        ┌─────────────────────────────────────────────────────────┐
                        │              wifi-densepose-vitals crate                │
                        │                                                         │
ESP32 CSI (UDP:5005) ──▶│  ┌──────────────────┐    ┌──────────────────────────┐  │
                        │  │ CsiVitalPreproc   │    │  VitalSignExtractor       │  │
    ┌───────────────────│  │ (ruvector-nervous  │──▶│  ┌────────────────────┐   │  │
    │                   │  │  -system:          │    │  │ BreathingExtractor │   │  │──▶ WebSocket
    │  wifi-densepose-  │  │  PredictiveLayer   │    │  │ (Bandpass 0.1-0.5) │   │  │    (/ws/vitals)
    │  signal crate     │  │  + EventRingBuffer)│    │  └────────────────────┘   │  │
    │  ┌─────────────┐  │  └──────────────────┘    │  ┌────────────────────┐   │  │──▶ REST API
    │  │CsiProcessor │  │           │               │  │ HeartRateExtractor │   │  │    (/api/v1/vitals)
    │  │PhaseSntzr   │──│───────────┘               │  │ (Bandpass 0.8-2.0) │   │  │
    │  │HampelFilter │  │                           │  └────────────────────┘   │  │
    │  │SubcarrierSel│  │  ┌──────────────────┐    │  ┌────────────────────┐   │  │
    │  └─────────────┘  │  │ SubcarrierWeighter│    │  │ MotionArtifact    │   │  │
    │                   │  │ (ruvector-attention│    │  │ Rejector          │   │  │
    └───────────────────│  │  + ruvector-gnn)   │──▶│  └────────────────────┘   │  │
                        │  └──────────────────┘    └──────────────────────────┘  │
                        │                                       │                 │
                        │  ┌──────────────────┐    ┌──────────────────────────┐  │
                        │  │ VitalSignStore    │    │  AnomalyDetector         │  │
                        │  │ (ruvector-temporal │◀──│  (biomarker_stream        │  │
                        │  │  -tensor:TieredSt)│    │   pattern: z-score,      │  │
                        │  └──────────────────┘    │   CUSUM, EMA, trend)     │  │
                        │                           └──────────────────────────┘  │
                        │  ┌──────────────────┐    ┌──────────────────────────┐  │
                        │  │ VitalCoherenceGate│    │  PatternMatcher          │  │
                        │  │ (ruQu: 3-filter   │    │  (ruvector-core:VectorDB │  │
                        │  │  pipeline, drift  │    │   + ModernHopfield)      │  │
                        │  │  detection,       │    └──────────────────────────┘  │
                        │  │  adaptive thresh) │                                  │
                        │  └──────────────────┘    ┌──────────────────────────┐  │
                        │  ┌──────────────────┐    │  SonaAdaptation          │  │
                        │  │ ESP32 Frame Input │    │  (sona:SonaEngine        │  │
                        │  │ (UDP:5005, magic  │    │   micro-LoRA adapt)      │  │
                        │  │  0xC511_0001,     │    └──────────────────────────┘  │
                        │  │  20B hdr + I/Q)   │                                  │
                        │  └──────────────────┘                                   │
                        └─────────────────────────────────────────────────────────┘
```

### Module Structure

```
rust-port/wifi-densepose-rs/crates/wifi-densepose-vitals/
├── Cargo.toml
└── src/
    ├── lib.rs                 # Public API and re-exports
    ├── config.rs              # VitalSignConfig, band definitions
    ├── preprocess.rs          # CsiVitalPreprocessor (PredictiveLayer-based)
    ├── extractor.rs           # VitalSignExtractor (breathing + heartrate)
    ├── breathing.rs           # BreathingExtractor (respiratory rate)
    ├── heartrate.rs           # HeartRateExtractor (cardiac rate)
    ├── subcarrier_weight.rs   # AttentionSubcarrierWeighter (GNN + attention)
    ├── artifact.rs            # MotionArtifactRejector
    ├── anomaly.rs             # VitalAnomalyDetector (z-score, CUSUM, EMA)
    ├── coherence_gate.rs      # VitalCoherenceGate (ruQu three-filter pipeline + drift detection)
    ├── store.rs               # VitalSignStore (TieredStore wrapper)
    ├── pattern.rs             # VitalPatternMatcher (Hopfield + HNSW)
    ├── adaptation.rs          # SonaVitalAdapter (environment adaptation)
    ├── types.rs               # VitalReading, VitalSign, VitalStatus
    └── error.rs               # VitalError type
```

## Signal Processing Pipeline

### Stage 1: CSI Preprocessing (Existing + PredictiveLayer)

The existing `wifi-densepose-signal` crate handles raw CSI ingestion:

1. **ESP32 frame parsing**: `parse_esp32_frame()` extracts I/Q amplitudes and phases from the ADR-018 binary frame format (magic `0xC511_0001`, 20-byte header + packed I/Q pairs).
2. **Phase sanitization**: `PhaseSanitizer` performs linear phase removal, unwrapping, and Hampel outlier filtering.
3. **Subcarrier selection**: `subcarrier_selection` module identifies motion-sensitive subcarriers.

The vital sign module adds a **PredictiveLayer** gate from `ruvector-nervous-system::routing`:

```rust
use ruvector_nervous_system::routing::PredictiveLayer;

pub struct CsiVitalPreprocessor {
    /// Predictive coding layer -- suppresses static CSI components.
    /// Only transmits residuals (changes) exceeding threshold.
    /// Achieves 90-99% bandwidth reduction on stable environments.
    predictive: PredictiveLayer,

    /// Ring buffer for CSI amplitude history per subcarrier.
    /// Modeled on biomarker_stream::RingBuffer.
    amplitude_buffers: Vec<RingBuffer<f64>>,

    /// Phase difference buffers (consecutive packet delta-phase).
    phase_diff_buffers: Vec<RingBuffer<f64>>,

    /// Number of subcarriers being tracked.
    n_subcarriers: usize,

    /// Sampling rate derived from ESP32 packet arrival rate.
    sample_rate_hz: f64,
}

impl CsiVitalPreprocessor {
    pub fn new(n_subcarriers: usize, window_size: usize) -> Self {
        Self {
            // 10% threshold: only transmit when CSI changes by >10%
            predictive: PredictiveLayer::new(n_subcarriers, 0.10),
            amplitude_buffers: (0..n_subcarriers)
                .map(|_| RingBuffer::new(window_size))
                .collect(),
            phase_diff_buffers: (0..n_subcarriers)
                .map(|_| RingBuffer::new(window_size))
                .collect(),
            n_subcarriers,
            sample_rate_hz: 100.0, // Default; calibrated from packet timing
        }
    }

    /// Ingest a new CSI frame and return preprocessed vital-sign-ready data.
    /// Returns None if the frame is predictable (no change).
    pub fn ingest(&mut self, amplitudes: &[f64], phases: &[f64]) -> Option<VitalFrame> {
        let amp_f32: Vec<f32> = amplitudes.iter().map(|&a| a as f32).collect();

        // PredictiveLayer gates: only process if residual exceeds threshold
        if !self.predictive.should_transmit(&amp_f32) {
            self.predictive.update(&amp_f32);
            return None; // Static environment, skip processing
        }

        self.predictive.update(&amp_f32);

        // Buffer amplitude and phase-difference data
        for (i, (&amp, &phase)) in amplitudes.iter().zip(phases.iter()).enumerate() {
            if i < self.n_subcarriers {
                self.amplitude_buffers[i].push(amp);
                self.phase_diff_buffers[i].push(phase);
            }
        }

        Some(VitalFrame {
            amplitudes: amplitudes.to_vec(),
            phases: phases.to_vec(),
            timestamp_us: /* from ESP32 frame */,
        })
    }
}
```

### Stage 2: Subcarrier Weighting (Attention + GNN)

Not all subcarriers carry vital sign information equally. Some are dominated by static multipath, others by motion artifacts. The subcarrier weighting stage uses `ruvector-attention` and `ruvector-gnn` to learn which subcarriers are most sensitive to physiological movements.

```rust
use ruvector_attention::ScaledDotProductAttention;
use ruvector_attention::traits::Attention;

pub struct AttentionSubcarrierWeighter {
    /// Attention mechanism for subcarrier importance scoring.
    /// Keys: subcarrier variance profiles.
    /// Queries: target vital sign frequency band power.
    /// Values: subcarrier amplitude time series.
    attention: ScaledDotProductAttention,

    /// GNN layer operating on subcarrier correlation graph.
    /// Nodes = subcarriers, edges = cross-correlation strength.
    /// Learns spatial-spectral patterns indicative of vital signs.
    gnn_layer: ruvector_gnn::GnnLayer,

    /// Weights per subcarrier (updated each processing window).
    weights: Vec<f32>,
}
```

The approach mirrors how BVP extraction in `wifi-densepose-signal::bvp` already uses `ScaledDotProductAttention` to weight subcarrier contributions to velocity profiles. For vital signs, the attention query vector encodes the expected spectral content (breathing band 0.1-0.5 Hz, cardiac band 0.8-2.0 Hz), and the keys encode each subcarrier's current spectral profile.

The GNN layer from `ruvector-gnn::layer` builds a correlation graph over subcarriers (node = subcarrier, edge weight = cross-correlation coefficient), then performs message passing to identify subcarrier clusters that exhibit coherent vital-sign-band oscillations. This is directly analogous to ADR-006's GNN-enhanced CSI pattern recognition.

### Stage 3: Vital Sign Extraction

Two parallel extractors operate on the weighted, preprocessed CSI data:

#### 3a: Respiratory Rate Extraction

```rust
pub struct BreathingExtractor {
    /// Bandpass filter: 0.1 - 0.5 Hz (6-30 breaths/min)
    filter_low: f64,  // 0.1 Hz
    filter_high: f64, // 0.5 Hz

    /// Oscillatory router from ruvector-nervous-system.
    /// Configured at ~0.25 Hz (mean breathing frequency).
    /// Phase-locks to the dominant respiratory component in CSI.
    oscillator: OscillatoryRouter,

    /// Ring buffer of filtered breathing-band signal.
    /// Modeled on biomarker_stream::RingBuffer<f64>.
    signal_buffer: RingBuffer<f64>,

    /// Peak detector state for breath counting.
    last_peak_time: Option<u64>,
    peak_intervals: RingBuffer<f64>,
}

impl BreathingExtractor {
    pub fn extract(&mut self, weighted_csi: &[f64], timestamp_us: u64) -> BreathingEstimate {
        // 1. Bandpass filter CSI to breathing band (0.1-0.5 Hz)
        let breathing_signal = self.bandpass_filter(weighted_csi);

        // 2. Aggregate across subcarriers (weighted sum)
        let composite = self.aggregate(breathing_signal);

        // 3. Buffer and detect peaks
        self.signal_buffer.push(composite);

        // 4. Count inter-peak intervals for rate estimation
        // Uses Welford online mean/variance (same as biomarker_stream::window_mean_std)
        let rate_bpm = self.estimate_rate();

        BreathingEstimate {
            rate_bpm,
            confidence: self.compute_confidence(),
            waveform_sample: composite,
            timestamp_us,
        }
    }
}
```

#### 3b: Heart Rate Extraction

```rust
pub struct HeartRateExtractor {
    /// Bandpass filter: 0.8 - 2.0 Hz (48-120 beats/min)
    filter_low: f64,  // 0.8 Hz
    filter_high: f64, // 2.0 Hz

    /// Hopfield network for cardiac pattern template matching.
    /// Stores learned heartbeat waveform templates.
    /// Retrieval acts as matched filter against noisy CSI.
    hopfield: ModernHopfield,

    /// Signal buffer for spectral analysis.
    signal_buffer: RingBuffer<f64>,

    /// Spectral coherence tracker from ruvector-coherence.
    coherence: SpectralTracker,
}
```

Heart rate extraction is inherently harder than breathing due to the much smaller displacement (0.1-0.5 mm vs 1-5 mm). The `ModernHopfield` network from `ruvector-nervous-system::hopfield` stores learned cardiac waveform templates with exponential storage capacity (Ramsauer et al. 2020 formulation). Retrieval performs a soft matched filter: the noisy CSI signal is compared against all stored templates via the transformer-style attention mechanism (`beta`-parameterized softmax), and the closest template's period determines heart rate.

The `ruvector-coherence::spectral::SpectralTracker` monitors the spectral gap and Fiedler value of the subcarrier correlation graph over time. A strong spectral gap in the cardiac band indicates high signal quality and reliable HR estimation.

### Stage 4: Motion Artifact Rejection

Large body movements (walking, gesturing) overwhelm the subtle vital sign signals. The artifact rejector uses the existing `MotionDetector` from `wifi-densepose-signal::motion` and the `DVSEvent`/`EventRingBuffer` system from `ruvector-nervous-system::eventbus`:

```rust
pub struct MotionArtifactRejector {
    /// Event ring buffer for motion events.
    /// DVSEvent.polarity=true indicates motion onset, false indicates motion offset.
    event_buffer: EventRingBuffer<DVSEvent>,

    /// Backpressure controller from ruvector-nervous-system::eventbus.
    /// Suppresses vital sign output during high-motion periods.
    backpressure: BackpressureController,

    /// Global workspace from ruvector-nervous-system::routing.
    /// Limited-capacity broadcast (Miller's Law: 4-7 items).
    /// Vital signs compete with motion signals for workspace slots.
    /// Only when motion signal loses the competition can vital signs broadcast.
    workspace: GlobalWorkspace,

    /// Motion energy threshold for blanking.
    motion_threshold: f64,

    /// Blanking duration after motion event (seconds).
    blanking_duration: f64,
}
```

The `GlobalWorkspace` (Baars 1988 model) from the nervous system routing module implements limited-capacity competition. Vital sign representations and motion representations compete for workspace access. During high motion, motion signals dominate the workspace and vital sign output is suppressed. When motion subsides, vital sign representations win the competition and are broadcast to consumers.

### Stage 5: Anomaly Detection

Modeled directly on `examples/dna/src/biomarker_stream.rs::StreamProcessor`:

```rust
pub struct VitalAnomalyDetector {
    /// Per-vital-sign ring buffers and rolling statistics.
    /// Directly mirrors biomarker_stream::StreamProcessor architecture.
    buffers: HashMap<VitalSignType, RingBuffer<f64>>,
    stats: HashMap<VitalSignType, VitalStats>,

    /// Z-score threshold for anomaly detection (default: 2.5, same as biomarker_stream).
    z_threshold: f64,

    /// CUSUM changepoint detection parameters.
    /// Detects sustained shifts in vital signs (e.g., respiratory arrest onset).
    cusum_threshold: f64, // 4.0 (same as biomarker_stream)
    cusum_drift: f64,     // 0.5

    /// EMA smoothing factor (alpha = 0.1).
    ema_alpha: f64,
}

pub struct VitalStats {
    pub mean: f64,
    pub variance: f64,
    pub min: f64,
    pub max: f64,
    pub count: u64,
    pub anomaly_rate: f64,
    pub trend_slope: f64,
    pub ema: f64,
    pub cusum_pos: f64,
    pub cusum_neg: f64,
    pub changepoint_detected: bool,
}
```

This is a near-direct port of the `biomarker_stream` architecture. The same Welford online algorithm computes rolling mean and standard deviation, the same CUSUM algorithm detects changepoints (apnea onset, tachycardia), and the same linear regression computes trend slopes.

### Stage 5b: ruQu Coherence Gate (Three-Filter Signal Quality Assessment)

The `ruQu` crate provides a production-grade **three-filter decision pipeline** originally designed for quantum error correction, but its abstractions map precisely to vital sign signal quality gating. Rather than reimplementing quality gates from scratch, we compose ruQu's filters into a vital sign coherence gate:

```rust
use ruqu::{
    AdaptiveThresholds, DriftDetector, DriftConfig, DriftProfile, LearningConfig,
    FilterPipeline, FilterConfig, Verdict,
};

pub struct VitalCoherenceGate {
    /// Three-filter pipeline adapted for vital sign gating:
    /// - Structural: min-cut on subcarrier correlation graph (low cut = signal degradation)
    /// - Shift: distribution drift in vital sign baselines (detects environmental changes)
    /// - Evidence: anytime-valid e-value accumulation for statistical rigor
    filter_pipeline: FilterPipeline,

    /// Adaptive thresholds that self-tune based on outcome feedback.
    /// Uses Welford online stats, EMA tracking, and precision/recall/F1 scoring.
    /// Directly ports ruQu's AdaptiveThresholds with LearningConfig.
    adaptive: AdaptiveThresholds,

    /// Drift detector for vital sign baselines.
    /// Detects 5 drift profiles from ruQu:
    /// - Stable: normal operation
    /// - Linear: gradual respiratory rate shift (e.g., falling asleep)
    /// - StepChange: sudden HR change (e.g., startle response)
    /// - Oscillating: periodic artifact (e.g., fan interference)
    /// - VarianceExpansion: increasing noise (e.g., subject moving)
    rr_drift: DriftDetector,
    hr_drift: DriftDetector,
}

impl VitalCoherenceGate {
    pub fn new() -> Self {
        Self {
            filter_pipeline: FilterPipeline::new(FilterConfig::default()),
            adaptive: AdaptiveThresholds::new(LearningConfig {
                learning_rate: 0.01,
                history_window: 10_000,
                warmup_samples: 500,  // ~5 seconds at 100 Hz
                ema_decay: 0.99,
                auto_adjust: true,
                ..Default::default()
            }),
            rr_drift: DriftDetector::with_config(DriftConfig {
                window_size: 300,  // 3-second window at 100 Hz
                min_samples: 100,
                mean_shift_threshold: 2.0,
                variance_threshold: 1.5,
                trend_sensitivity: 0.1,
            }),
            hr_drift: DriftDetector::with_config(DriftConfig {
                window_size: 500,  // 5-second window (cardiac needs longer baseline)
                min_samples: 200,
                mean_shift_threshold: 2.5,
                variance_threshold: 2.0,
                trend_sensitivity: 0.05,
            }),
        }
    }

    /// Gate a vital sign reading: returns Verdict (Permit/Deny/Defer)
    pub fn gate(&mut self, reading: &VitalReading) -> Verdict {
        // Feed respiratory rate to drift detector
        self.rr_drift.push(reading.respiratory_rate.value_bpm);
        self.hr_drift.push(reading.heart_rate.value_bpm);

        // Record metrics for adaptive threshold learning
        let cut = reading.signal_quality;
        let shift = self.rr_drift.severity().max(self.hr_drift.severity());
        let evidence = reading.respiratory_rate.confidence.min(reading.heart_rate.confidence);
        self.adaptive.record_metrics(cut, shift, evidence);

        // Three-filter decision: all must pass for PERMIT
        // This ensures only high-confidence vital signs reach the UI
        let verdict = self.filter_pipeline.evaluate(cut, shift, evidence);

        // If drift detected, compensate adaptive thresholds
        if let Some(profile) = self.rr_drift.detect() {
            if !matches!(profile, DriftProfile::Stable) {
                self.adaptive.apply_drift_compensation(&profile);
            }
        }

        verdict
    }

    /// Record whether the gate decision was correct (for learning)
    pub fn record_outcome(&mut self, was_deny: bool, was_actually_bad: bool) {
        self.adaptive.record_outcome(was_deny, was_actually_bad);
    }
}
```

**Why ruQu fits here:**

| ruQu Concept | Vital Sign Mapping |
|---|---|
| Syndrome round (detector bitmap) | CSI frame (subcarrier amplitudes/phases) |
| Structural min-cut | Subcarrier correlation graph connectivity (low cut = signal breakup) |
| Shift filter (distribution drift) | Respiratory/cardiac baseline drift from normal |
| Evidence filter (e-value) | Statistical confidence accumulation over time |
| `DriftDetector` with 5 profiles | Detects sleep onset (Linear), startle (StepChange), fan interference (Oscillating), subject motion (VarianceExpansion) |
| `AdaptiveThresholds` with Welford/EMA | Self-tuning anomaly thresholds with outcome-based F1 optimization |
| PERMIT / DENY / DEFER | Only emit vital signs to UI when quality is proven |
| 256-tile `QuantumFabric` | Future: parallel per-subcarrier processing on WASM |

### Stage 6: Tiered Storage

```rust
use ruvector_temporal_tensor::{TieredStore, TierPolicy, Tier};
use ruvector_temporal_tensor::core_trait::{TensorStore, TensorStoreExt};

pub struct VitalSignStore {
    store: TieredStore,
    tier_policy: TierPolicy,
}
```

Vital sign data is stored in the `TieredStore` from `ruvector-temporal-tensor`:

| Tier | Bits | Compression | Purpose |
|------|------|-------------|---------|
| Tier1 (Hot) | 8-bit | 4x | Real-time vital signs (last 5 minutes), fed to UI |
| Tier2 (Warm) | 5-bit | 6.4x | Recent history (last 1 hour), trend analysis |
| Tier3 (Cold) | 3-bit | 10.67x | Long-term archive (24+ hours), pattern library |
| Tier0 (Evicted) | metadata only | N/A | Expired data with reconstruction policy |

The `BlockKey` maps naturally to vital sign storage:
- `tensor_id`: encodes vital sign type (0 = breathing rate, 1 = heart rate, 2 = composite waveform)
- `block_index`: encodes time window index

### Stage 7: Environment Adaptation (SONA)

```rust
use sona::{SonaEngine, SonaConfig, TrajectoryBuilder};

pub struct SonaVitalAdapter {
    engine: SonaEngine,
}

impl SonaVitalAdapter {
    pub fn begin_extraction(&self, csi_embedding: Vec<f32>) -> TrajectoryBuilder {
        self.engine.begin_trajectory(csi_embedding)
    }

    pub fn end_extraction(&self, builder: TrajectoryBuilder, quality: f32) {
        // quality = confidence * accuracy of vital sign estimate
        self.engine.end_trajectory(builder, quality);
    }

    /// Apply micro-LoRA adaptation to filter parameters.
    pub fn adapt_filters(&self, filter_params: &[f32], adapted: &mut [f32]) {
        self.engine.apply_micro_lora(filter_params, adapted);
    }
}
```

The SONA engine's 4-step intelligence pipeline (RETRIEVE, JUDGE, DISTILL, CONSOLIDATE) enables:
1. **RETRIEVE**: Find past successful extraction parameters for similar environments via HNSW.
2. **JUDGE**: Score extraction quality based on physiological plausibility (HR 40-180 BPM, RR 4-40 BPM).
3. **DISTILL**: Extract key parameter adjustments via micro-LoRA.
4. **CONSOLIDATE**: Prevent forgetting of previously learned environments via EWC++.

## Data Flow

### End-to-End Pipeline

```
ESP32 CSI Frame (UDP :5005)
│  Magic: 0xC511_0001 | 20-byte header | packed I/Q pairs
│  parse_esp32_frame() → Esp32Frame { node_id, n_antennas,
│     n_subcarriers, freq_mhz, sequence, rssi, noise_floor,
│     amplitudes: Vec<f64>, phases: Vec<f64> }
│
▼
[wifi-densepose-signal] CsiProcessor + PhaseSanitizer + HampelFilter
│
▼
[wifi-densepose-vitals] CsiVitalPreprocessor (PredictiveLayer gate)
│
├──▶ Static environment? (predictable) ──▶ Skip (90-99% frames filtered)
│
▼ (residual frames with physiological changes)
[wifi-densepose-vitals] AttentionSubcarrierWeighter (attention + GNN)
│
▼
[wifi-densepose-vitals] MotionArtifactRejector (GlobalWorkspace competition)
│
├──▶ High motion? ──▶ Blank vital sign output, report motion-only
│
▼ (low-motion frames)
├──▶ BreathingExtractor ──▶ RR estimate (BPM + confidence)
├──▶ HeartRateExtractor ──▶ HR estimate (BPM + confidence)
│
▼
[wifi-densepose-vitals] VitalAnomalyDetector (z-score, CUSUM, EMA)
│
├──▶ Anomaly? ──▶ Alert (apnea, tachycardia, bradycardia)
│
▼
[wifi-densepose-vitals] VitalCoherenceGate (ruQu three-filter pipeline)
│
├──▶ DENY (low quality)  ──▶ Suppress reading, keep previous valid
├──▶ DEFER (accumulating) ──▶ Buffer, await more evidence
│
▼ PERMIT (high-confidence vital signs)
[wifi-densepose-vitals] VitalSignStore (TieredStore: 8/5/3-bit)
│
▼
[wifi-densepose-sensing-server] WebSocket broadcast (/ws/vitals)
│  AppStateInner extended with latest_vitals + vitals_tx channel
│  ESP32 mode: udp_receiver_task feeds amplitudes/phases to VitalSignExtractor
│  WiFi mode: pseudo-frame (single subcarrier) → VitalStatus::Unreliable
│  Simulate mode: synthetic CSI → calibration/demo vital signs
│
▼
[UI] SensingTab.js: vital sign visualization overlay
```

**ESP32 Integration Detail:** The `udp_receiver_task` in the sensing server already receives and parses ESP32 frames. The vital sign module hooks into this path:

```rust
// In udp_receiver_task, after parse_esp32_frame():
if let Some(frame) = parse_esp32_frame(&buf[..len]) {
    let (features, classification) = extract_features_from_frame(&frame);

    // NEW: Feed into vital sign extractor
    let vital_reading = s.vital_extractor.process_frame(
        &frame.amplitudes,
        &frame.phases,
        frame.sequence as u64 * 10_000, // approximate timestamp_us
    );

    if let Some(reading) = vital_reading {
        s.latest_vitals = Some(reading.into());
        if let Ok(json) = serde_json::to_string(&s.latest_vitals) {
            let _ = s.vitals_tx.send(json);
        }
    }
    // ... existing sensing update logic unchanged ...
}
```

### WebSocket Message Schema

```json
{
  "type": "vital_update",
  "timestamp": 1709146800.123,
  "source": "esp32",
  "vitals": {
    "respiratory_rate": {
      "value_bpm": 16.2,
      "confidence": 0.87,
      "waveform": [0.12, 0.15, 0.21, ...],
      "status": "normal"
    },
    "heart_rate": {
      "value_bpm": 72.5,
      "confidence": 0.63,
      "waveform": [0.02, 0.03, 0.05, ...],
      "status": "normal"
    },
    "motion_level": "low",
    "signal_quality": 0.78
  },
  "anomalies": [],
  "stats": {
    "rr_mean": 15.8,
    "rr_trend": -0.02,
    "hr_mean": 71.3,
    "hr_trend": 0.01,
    "rr_ema": 16.0,
    "hr_ema": 72.1
  }
}
```

## Integration Points

### 1. Sensing Server Integration

The `wifi-densepose-sensing-server` crate's `AppStateInner` is extended with vital sign state:

```rust
struct AppStateInner {
    latest_update: Option<SensingUpdate>,
    latest_vitals: Option<VitalUpdate>,   // NEW
    vital_extractor: VitalSignExtractor,  // NEW
    rssi_history: VecDeque<f64>,
    tick: u64,
    source: String,
    tx: broadcast::Sender<String>,
    vitals_tx: broadcast::Sender<String>, // NEW: separate channel for vitals
    total_detections: u64,
    start_time: std::time::Instant,
}
```

New Axum routes:

```rust
Router::new()
    .route("/ws/vitals", get(ws_vitals_handler))
    .route("/api/v1/vitals/current", get(get_current_vitals))
    .route("/api/v1/vitals/history", get(get_vital_history))
    .route("/api/v1/vitals/config", get(get_vital_config).put(set_vital_config))
```

### 2. UI Integration

The existing SensingTab.js Gaussian splat visualization (ADR-019) is extended with:

- **Breathing ring**: Already prototyped in `generate_signal_field()` as the `breath_ring` variable -- amplitude modulated by `variance` and `tick`. This is replaced with the actual breathing waveform from the vital sign extractor.
- **Heart rate indicator**: Pulsing opacity overlay synced to estimated heart rate.
- **Vital sign panel**: Side panel showing HR/RR values, trend sparklines, and anomaly alerts.

### 3. Existing Signal Crate Integration

`wifi-densepose-vitals` depends on `wifi-densepose-signal` for CSI preprocessing and on the rvdna crates for its core algorithms. The dependency graph:

```
wifi-densepose-vitals
├── wifi-densepose-signal          (CSI preprocessing)
├── ruvector-nervous-system        (PredictiveLayer, EventBus, Hopfield, GlobalWorkspace)
├── ruvector-attention             (subcarrier attention weighting)
├── ruvector-gnn                   (subcarrier correlation graph)
├── ruvector-coherence             (spectral analysis, signal quality)
├── ruvector-temporal-tensor       (tiered storage)
├── ruvector-core                  (VectorDB for pattern matching)
├── ruqu                           (three-filter coherence gate, adaptive thresholds, drift detection)
└── sona                           (environment adaptation)
```

## API Design

### Core Public API

```rust
/// Main vital sign extraction engine.
pub struct VitalSignExtractor {
    preprocessor: CsiVitalPreprocessor,
    weighter: AttentionSubcarrierWeighter,
    breathing: BreathingExtractor,
    heartrate: HeartRateExtractor,
    artifact_rejector: MotionArtifactRejector,
    anomaly_detector: VitalAnomalyDetector,
    coherence_gate: VitalCoherenceGate,  // ruQu three-filter quality gate
    store: VitalSignStore,
    adapter: SonaVitalAdapter,
    config: VitalSignConfig,
}

impl VitalSignExtractor {
    /// Create a new extractor with default configuration.
    pub fn new(config: VitalSignConfig) -> Self;

    /// Process a single CSI frame and return vital sign estimates.
    /// Returns None during motion blanking or static environment periods.
    pub fn process_frame(
        &mut self,
        amplitudes: &[f64],
        phases: &[f64],
        timestamp_us: u64,
    ) -> Option<VitalReading>;

    /// Get current vital sign estimates.
    pub fn current(&self) -> VitalStatus;

    /// Get historical vital sign data from tiered store.
    pub fn history(&mut self, duration_secs: u64) -> Vec<VitalReading>;

    /// Get anomaly alerts.
    pub fn anomalies(&self) -> Vec<VitalAnomaly>;

    /// Get signal quality assessment.
    pub fn signal_quality(&self) -> SignalQuality;
}

/// Configuration for vital sign extraction.
pub struct VitalSignConfig {
    /// Number of subcarriers to track.
    pub n_subcarriers: usize,
    /// CSI sampling rate (Hz). Calibrated from ESP32 packet rate.
    pub sample_rate_hz: f64,
    /// Ring buffer window size (samples).
    pub window_size: usize,
    /// Breathing band (Hz).
    pub breathing_band: (f64, f64),
    /// Heart rate band (Hz).
    pub heartrate_band: (f64, f64),
    /// PredictiveLayer residual threshold.
    pub predictive_threshold: f32,
    /// Z-score anomaly threshold.
    pub anomaly_z_threshold: f64,
    /// Motion blanking duration (seconds).
    pub motion_blank_secs: f64,
    /// Tiered store capacity (bytes).
    pub store_capacity: usize,
    /// Enable SONA adaptation.
    pub enable_adaptation: bool,
}

impl Default for VitalSignConfig {
    fn default() -> Self {
        Self {
            n_subcarriers: 56,
            sample_rate_hz: 100.0,
            window_size: 1024,    // ~10 seconds at 100 Hz
            breathing_band: (0.1, 0.5),
            heartrate_band: (0.8, 2.0),
            predictive_threshold: 0.10,
            anomaly_z_threshold: 2.5,
            motion_blank_secs: 2.0,
            store_capacity: 4 * 1024 * 1024, // 4 MB
            enable_adaptation: true,
        }
    }
}

/// Single vital sign reading at a point in time.
pub struct VitalReading {
    pub timestamp_us: u64,
    pub respiratory_rate: VitalEstimate,
    pub heart_rate: VitalEstimate,
    pub motion_level: MotionLevel,
    pub signal_quality: f64,
}

/// Estimated vital sign value with confidence.
pub struct VitalEstimate {
    pub value_bpm: f64,
    pub confidence: f64,
    pub waveform_sample: f64,
    pub status: VitalStatus,
}

pub enum VitalStatus {
    Normal,
    Elevated,
    Depressed,
    Critical,
    Unreliable,  // Confidence below threshold
    Blanked,     // Motion artifact blanking
}

pub enum MotionLevel {
    Static,
    Minimal,  // Micro-movements (breathing, heartbeat)
    Low,      // Small movements (fidgeting)
    Moderate, // Walking
    High,     // Running, exercising
}
```

## Performance Considerations

### Latency Budget

| Stage | Target Latency | Mechanism |
|-------|---------------|-----------|
| CSI frame parsing | <50 us | Existing `parse_esp32_frame()` |
| Predictive gating | <10 us | `PredictiveLayer.should_transmit()` is a single RMS computation |
| Subcarrier weighting | <100 us | Attention: O(n_subcarriers * dim), GNN: single layer forward |
| Bandpass filtering | <50 us | FIR filter, vectorized |
| Peak detection | <10 us | Simple threshold comparison |
| Anomaly detection | <5 us | Welford online update + CUSUM |
| Tiered store put | <20 us | Quantize + memcpy |
| **Total per frame** | **<250 us** | **Well within 10ms frame budget at 100 Hz** |

### Bandwidth Reduction

The `PredictiveLayer` from `ruvector-nervous-system::routing` achieves 90-99% bandwidth reduction on stable signals. For vital sign monitoring where the subject is stationary (the primary use case), most CSI frames are predictable. Only frames with physiological residuals (breathing, heartbeat) pass through, reducing computational load by 10-100x.

### Memory Budget

| Component | Estimated Memory |
|-----------|-----------------|
| Ring buffers (56 subcarriers x 1024 samples x 8 bytes) | ~450 KB |
| Attention weights (56 x 64 dim) | ~14 KB |
| GNN layer (56 nodes, single layer) | ~25 KB |
| Hopfield network (128-dim, 100 templates) | ~50 KB |
| TieredStore (4 MB budget) | 4 MB |
| SONA engine (64-dim hidden) | ~10 KB |
| **Total** | **~4.6 MB** |

This fits comfortably within the sensing server's target footprint (ADR-019: ~5 MB RAM for the whole server).

### Accuracy Expectations

Based on WiFi vital sign literature and the quality of rvdna primitives:

| Metric | Target | Notes |
|--------|--------|-------|
| Respiratory rate error | < 1.5 BPM (median) | Breathing is the easier signal; large chest displacement |
| Heart rate error | < 5 BPM (median) | Harder; requires high SNR, stationary subject |
| Detection latency | < 15 seconds | Time to first reliable estimate after initialization |
| Motion rejection | > 95% true positive | Correctly blanks during gross motion |
| False anomaly rate | < 2% | CUSUM + z-score with conservative thresholds |

## Security Considerations

### Health Data Privacy

1. **No cloud transmission**: All vital sign processing occurs on-device. CSI data and extracted vital signs never leave the local network.
2. **No PII in CSI**: WiFi CSI captures environmental propagation patterns, not biometric identifiers. Vital signs are statistical aggregates (rates), not waveforms that could identify individuals.
3. **Local storage encryption**: The `TieredStore` can be wrapped with at-rest encryption for the cold tier. The existing `rvf-crypto` crate in the rvdna workspace provides post-quantum cryptographic primitives (ADR-007).
4. **Access control**: REST API endpoints for vital sign history require authentication when deployed in multi-user environments.
5. **Data retention**: Configurable TTL on `TieredStore` blocks. Default: hot tier expires after 5 minutes, warm after 1 hour, cold after 24 hours.

### Medical Disclaimer

Vital signs extracted from WiFi CSI are **not medical devices** and should not be used for clinical diagnosis. The system provides wellness-grade monitoring suitable for:
- Occupancy-aware HVAC optimization
- Eldercare activity monitoring (alert on prolonged stillness)
- Sleep quality estimation
- Disaster survivor detection (ADR-001)

## Alternatives Considered

### Alternative 1: Pure FFT-Based Extraction (No rvdna)

Implement simple bandpass filters and FFT peak detection without using rvdna components.

**Rejected because**: This approach lacks adaptive subcarrier selection, environment calibration, artifact rejection sophistication, and anomaly detection. The resulting system would be fragile across environments and sensor placements. The rvdna components provide production-grade primitives for exactly these challenges.

### Alternative 2: Python-Based Vital Sign Module

Extend the existing Python `ws_server.py` with scipy signal processing.

**Rejected because**: ADR-020 establishes Rust as the primary backend. Adding vital sign processing in Python contradicts the migration direction and doubles the dependency burden. The rvdna crates are Rust-native and already vendored.

### Alternative 3: External ML Model (ONNX)

Train a deep learning model to extract vital signs from raw CSI and run it via ONNX Runtime.

**Partially adopted**: ONNX-based models may be added in Phase 3 as an alternative extractor. However, the primary pipeline uses interpretable signal processing (bandpass + peak detection) because: (a) it works without training data, (b) it is debuggable, (c) it runs on resource-constrained edge devices without ONNX Runtime. The SONA adaptation layer provides learned optimization on top of the interpretable pipeline.

### Alternative 4: Radar-Based Vital Signs (Not WiFi)

Use dedicated FMCW radar hardware instead of WiFi CSI.

**Rejected because**: WiFi CSI reuses existing infrastructure (commodity routers, ESP32). No additional hardware is required. The project's core value proposition is infrastructure-free sensing.

## Consequences

### Positive

- **Extends sensing capabilities**: The project goes from presence/motion detection to vital sign monitoring without additional hardware.
- **Leverages existing investment**: Reuses rvdna crates already vendored and understood, avoiding new dependencies.
- **Production-grade primitives**: PredictiveLayer, TieredStore, CUSUM, Hopfield matching, SONA adaptation are all tested components with known performance characteristics.
- **Composable architecture**: Each stage is independently testable and replaceable.
- **Edge-friendly**: 4.6 MB memory footprint and <250 us per-frame latency fit ESP32-class devices.
- **Privacy-preserving**: Local-only processing with no cloud dependency.

### Negative

- **Signal-to-noise challenge**: WiFi-based heart rate detection has inherently low SNR. Confidence scores may frequently be "Unreliable" in noisy environments.
- **Calibration requirement**: Each deployment environment has different multipath characteristics. SONA adaptation mitigates this but requires an initial calibration period (15-60 seconds).
- **Single-person limitation**: Multi-person vital sign separation from a single TX-RX pair is an open research problem. This design assumes one dominant subject in the sensing zone.
- **Additional crate dependencies**: The vital sign module adds 6 rvdna crate dependencies to the workspace, increasing compile time.
- **Not medical grade**: Cannot replace clinical monitoring devices. Must be clearly labeled as wellness-grade.

## Implementation Roadmap

### Phase 1: Core Pipeline (Weeks 1-2)

- Create `wifi-densepose-vitals` crate with module structure
- Implement `CsiVitalPreprocessor` with `PredictiveLayer` gate
- Implement `BreathingExtractor` with bandpass filter and peak detection
- Implement `VitalAnomalyDetector` (port `biomarker_stream::StreamProcessor` pattern)
- Basic unit tests with synthetic CSI data
- Integration with `wifi-densepose-sensing-server` WebSocket

### Phase 2: Enhanced Extraction (Weeks 3-4)

- Implement `AttentionSubcarrierWeighter` using `ruvector-attention`
- Implement `HeartRateExtractor` with `ModernHopfield` template matching
- Implement `MotionArtifactRejector` with `GlobalWorkspace` competition
- Implement `VitalSignStore` with `TieredStore`
- End-to-end integration test with ESP32 CSI data

### Phase 3: Adaptation and UI (Weeks 5-6)

- Implement `SonaVitalAdapter` for environment calibration
- Add GNN-based subcarrier correlation analysis
- Extend UI SensingTab with vital sign visualization
- Add REST API endpoints for vital sign history
- Performance benchmarking and optimization

### Phase 4: Hardening (Weeks 7-8)

- CUSUM changepoint detection for apnea/tachycardia alerts
- Multi-environment testing and SONA training
- Security review (data retention, access control)
- Documentation and API reference
- Optional: ONNX-based alternative extractor

## Windows WiFi Mode Enhancement

The current Windows WiFi mode (`--source wifi`) uses `netsh wlan show interfaces` to extract a single RSSI/signal% value per tick. This yields a pseudo-single-subcarrier frame that is insufficient for multi-subcarrier vital sign extraction. However, ruQu and rvdna primitives can still enhance this mode:

### What Works in Windows WiFi Mode

| Capability | Mechanism | Quality |
|---|---|---|
| **Presence detection** | RSSI variance over time via `DriftDetector` | Good -- ruQu detects StepChange when a person enters/leaves |
| **Coarse breathing estimate** | RSSI temporal modulation at 0.1-0.5 Hz | Fair -- single-signal source, needs 30+ seconds of stationary RSSI |
| **Environmental drift** | `AdaptiveThresholds` + `DriftDetector` on RSSI series | Good -- detects linear trends, step changes, oscillating interference |
| **Signal quality gating** | ruQu `FilterPipeline` gates unreliable readings | Good -- suppresses false readings during WiFi fluctuations |

### What Does NOT Work in Windows WiFi Mode

| Capability | Why Not |
|---|---|
| Heart rate extraction | Requires multi-subcarrier CSI phase coherence (0.1-0.5 mm displacement resolution) |
| Multi-person separation | Single omnidirectional RSSI cannot distinguish spatial sources |
| Subcarrier attention weighting | Only 1 subcarrier available |
| GNN correlation graph | Needs >= 2 subcarrier nodes |

### Enhancement Strategy (Windows WiFi)

```rust
// In windows_wifi_task, after collecting RSSI:
// Feed RSSI time series to a simplified vital pipeline
let mut wifi_vitals = WifiRssiVitalEstimator {
    // ruQu adaptive thresholds for RSSI gating
    adaptive: AdaptiveThresholds::new(LearningConfig::conservative()),
    // Drift detection on RSSI (detects presence events)
    drift: DriftDetector::new(60), // 60 samples = ~30 seconds at 2 Hz
    // Simple breathing estimator on RSSI temporal modulation
    breathing_buffer: RingBuffer::new(120), // 60 seconds of RSSI history
};

// Every tick:
wifi_vitals.breathing_buffer.push(rssi_dbm);
wifi_vitals.drift.push(rssi_dbm);

// Attempt coarse breathing rate from RSSI oscillation
let rr_estimate = wifi_vitals.estimate_breathing_from_rssi();

// Gate quality using ruQu
let verdict = wifi_vitals.adaptive.current_thresholds();
// Only emit if signal quality justifies it
let vitals = VitalReading {
    respiratory_rate: VitalEstimate {
        value_bpm: rr_estimate.unwrap_or(0.0),
        confidence: if rr_estimate.is_some() { 0.3 } else { 0.0 },
        status: VitalStatus::Unreliable, // Always marked as low-confidence
        ..
    },
    heart_rate: VitalEstimate {
        confidence: 0.0,
        status: VitalStatus::Unreliable, // Cannot estimate from single RSSI
        ..
    },
    ..
};
```

**Bottom line:** Windows WiFi mode gets presence/drift detection and coarse breathing via ruQu's adaptive thresholds and drift detector. For meaningful vital signs (HR, high-confidence RR), ESP32 CSI is required.

## Implementation Status (2026-02-28)

### Completed: ADR-022 Windows WiFi Multi-BSSID Pipeline

The `wifi-densepose-wifiscan` crate implements the Windows WiFi enhancement strategy described above as a complete 8-stage pipeline (ADR-022 Phase 2). All stages are pure Rust with no external vendor dependencies:

| Stage | Module | Implementation | Tests |
|-------|--------|---------------|-------|
| 1. Predictive Gating | `predictive_gate.rs` | EMA-based residual filter (replaces `PredictiveLayer`) | 4 |
| 2. Attention Weighting | `attention_weighter.rs` | Softmax dot-product attention (replaces `ScaledDotProductAttention`) | 4 |
| 3. Spatial Correlation | `correlator.rs` | Pearson correlation + BFS clustering | 5 |
| 4. Motion Estimation | `motion_estimator.rs` | Weighted variance + EMA smoothing | 6 |
| 5. Breathing Extraction | `breathing_extractor.rs` | IIR bandpass (0.1-0.5 Hz) + zero-crossing | 6 |
| 6. Quality Gate | `quality_gate.rs` | Three-filter (structural/shift/evidence) inspired by ruQu | 8 |
| 7. Fingerprint Matching | `fingerprint_matcher.rs` | Cosine similarity templates (replaces `ModernHopfield`) | 8 |
| 8. Orchestrator | `orchestrator.rs` | `WindowsWifiPipeline` domain service composing stages 1-7 | 7 |

**Total: 124 passing tests, 0 failures.**

Domain model (Phase 1) includes:
- `MultiApFrame`: Multi-BSSID frame value object with amplitudes, phases, variances, histories
- `BssidRegistry`: Aggregate root managing BSSID lifecycle with Welford running statistics
- `NetshBssidScanner`: Adapter parsing `netsh wlan show networks mode=bssid` output
- `EnhancedSensingResult`: Pipeline output with motion, breathing, posture, quality metrics

### Remaining: ADR-021 Dedicated Vital Sign Crate

The `wifi-densepose-vitals` crate (ESP32 CSI-grade vital signs) has not yet been implemented. Required for:
- Heart rate extraction from multi-subcarrier CSI phase coherence
- Multi-person vital sign separation
- SONA-based environment adaptation
- VitalSignStore with tiered temporal compression

## References

- Ramsauer et al. (2020). "Hopfield Networks is All You Need." ICLR 2021. (ModernHopfield formulation)
- Fries (2015). "Rhythms for Cognition: Communication through Coherence." Neuron. (OscillatoryRouter basis)
- Bellec et al. (2020). "A solution to the learning dilemma for recurrent networks of spiking neurons." Nature Communications. (E-prop online learning)
- Baars (1988). "A Cognitive Theory of Consciousness." Cambridge UP. (GlobalWorkspace model)
- Liu et al. (2023). "WiFi-based Contactless Breathing and Heart Rate Monitoring." IEEE Sensors Journal.
- Wang et al. (2022). "Robust Vital Signs Monitoring Using WiFi CSI." ACM MobiSys.
- Widar 3.0 (MobiSys 2019). "Zero-Effort Cross-Domain Gesture Recognition with WiFi." (BVP extraction basis)
