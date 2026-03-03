# Signal Processing Domain Model

## Domain-Driven Design Specification

Based on ADR-014 (SOTA Signal Processing) and the `wifi-densepose-signal` crate.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **CsiFrame** | A single CSI measurement: amplitude + phase per antenna per subcarrier at one timestamp |
| **Conjugate Multiplication** | `H_ref[k] * conj(H_target[k])` — cancels CFO/SFO/PDD, isolating environment-induced phase |
| **CSI Ratio** | The complex result of conjugate multiplication between two antenna streams |
| **Hampel Filter** | Running median +/- scaled MAD outlier detector; resists up to 50% contamination |
| **Phase Sanitization** | Pipeline of unwrapping, outlier removal, smoothing, and noise filtering on raw CSI phase |
| **Spectrogram** | 2D time-frequency matrix from STFT, standard CNN input for WiFi activity recognition |
| **Subcarrier Sensitivity** | Variance ratio (motion var / static var) ranking how responsive a subcarrier is to motion |
| **Body Velocity Profile (BVP)** | Doppler-derived velocity x time 2D matrix; domain-independent motion representation |
| **Fresnel Zone** | Ellipsoidal region between TX and RX where signal reflection/diffraction occurs |
| **Breathing Estimate** | BPM + amplitude + confidence derived from Fresnel zone boundary crossings |
| **Motion Score** | Composite (0.0-1.0) from variance, correlation, phase, and optional Doppler components |
| **Presence State** | Binary detection result: human present/absent with smoothed confidence |
| **Calibration** | Recording baseline variance during a known-empty period for adaptive detection |

---

## Bounded Contexts

### 1. CSI Preprocessing Context

**Responsibility**: Produce clean, hardware-artifact-free CSI data from raw measurements.

```
+-----------------------------------------------------------+
|               CSI Preprocessing Context                    |
+-----------------------------------------------------------+
|                                                            |
|  +--------------+    +--------------+    +------------+    |
|  |  Conjugate   |    |   Hampel     |    |   Phase    |    |
|  | Multiplication|   |   Filter     |    | Sanitizer  |    |
|  +------+-------+    +------+-------+    +-----+------+    |
|         |                   |                  |           |
|         v                   v                  v           |
|  +------+-------+    +------+-------+    +-----+------+    |
|  |  CsiRatio    |    | HampelResult |    | Sanitized  |    |
|  | (clean phase)|    |(outlier-free)|    |   Phase    |    |
|  +--------------+    +--------------+    +------------+    |
|         |                   |                  |           |
|         +-------------------+------------------+           |
|                             |                              |
|                             v                              |
|                     +-------+--------+                     |
|                     |  CsiProcessor  |--> CleanedCsiData   |
|                     +----------------+                     |
|                                                            |
+-----------------------------------------------------------+
```

**Aggregates**: `CsiProcessor` (Aggregate Root)

**Value Objects**: `CsiData`, `CsiRatio`, `HampelResult`, `HampelConfig`, `PhaseSanitizerConfig`

**Domain Services**: `CsiPreprocessor`, `PhaseSanitizer`

---

### 2. Feature Extraction Context

**Responsibility**: Transform clean CSI data into ML-ready feature representations.

```
+-----------------------------------------------------------+
|              Feature Extraction Context                    |
+-----------------------------------------------------------+
|                                                            |
|  +--------------+    +--------------+    +------------+    |
|  |    STFT      |    | Subcarrier   |    |  Doppler   |    |
|  | Spectrogram  |    |  Selection   |    | BVP Engine |    |
|  +------+-------+    +------+-------+    +-----+------+    |
|         |                   |                  |           |
|         v                   v                  v           |
|  +------+-------+    +------+-------+    +-----+------+    |
|  | Spectrogram  |    | Subcarrier   |    |  BodyVel   |    |
|  |   (2D TF)    |    |  Selection   |    |  Profile   |    |
|  +--------------+    +--------------+    +------------+    |
|         |                   |                  |           |
|         +-------------------+------------------+           |
|                             |                              |
|                             v                              |
|                  +----------+----------+                   |
|                  | FeatureExtractor    |--> CsiFeatures     |
|                  +---------------------+                   |
|                                                            |
+-----------------------------------------------------------+
```

**Aggregates**: `FeatureExtractor` (Aggregate Root)

**Value Objects**: `Spectrogram`, `SubcarrierSelection`, `BodyVelocityProfile`, `CsiFeatures`

**Domain Services**: `SpectrogramConfig`, `SubcarrierSelectionConfig`, `BvpConfig`

---

### 3. Motion Analysis Context

**Responsibility**: Detect and classify human motion and vital signs from CSI features.

```
+-----------------------------------------------------------+
|               Motion Analysis Context                      |
+-----------------------------------------------------------+
|                                                            |
|  +--------------+    +--------------+                      |
|  |   Motion     |    |   Fresnel    |                      |
|  |  Detector    |    |  Breathing   |                      |
|  +------+-------+    +------+-------+                      |
|         |                   |                              |
|         v                   v                              |
|  +------+-------+    +------+-------+                      |
|  | MotionScore  |    | Breathing    |                      |
|  |+ Detection   |    |  Estimate    |                      |
|  +--------------+    +--------------+                      |
|         |                   |                              |
|         +-------------------+                              |
|                   |                                        |
|                   v                                        |
|          +--------+--------+                               |
|          | HumanDetection  |--> PresenceState              |
|          |    Result       |                               |
|          +-----------------+                               |
|                                                            |
+-----------------------------------------------------------+
```

**Aggregates**: `MotionDetector` (Aggregate Root)

**Value Objects**: `MotionScore`, `MotionAnalysis`, `HumanDetectionResult`, `BreathingEstimate`, `FresnelGeometry`

**Domain Services**: `FresnelBreathingEstimator`

---

## Aggregates

### CsiProcessor (CSI Preprocessing Root)

```rust
pub struct CsiProcessor {
    config: CsiProcessorConfig,
    preprocessor: CsiPreprocessor,
    history: VecDeque<CsiData>,
    previous_detection_confidence: f64,
    statistics: ProcessingStatistics,
}

impl CsiProcessor {
    /// Create with validated configuration
    pub fn new(config: CsiProcessorConfig) -> Result<Self, CsiProcessorError>;

    /// Full preprocessing pipeline: noise removal -> windowing -> normalization
    pub fn preprocess(&self, csi_data: &CsiData) -> Result<CsiData, CsiProcessorError>;

    /// Maintain temporal history for downstream feature extraction
    pub fn add_to_history(&mut self, csi_data: CsiData);

    /// Apply exponential moving average to detection confidence
    pub fn apply_temporal_smoothing(&mut self, raw_confidence: f64) -> f64;
}
```

### FeatureExtractor (Feature Extraction Root)

```rust
pub struct FeatureExtractor {
    config: FeatureExtractorConfig,
}

impl FeatureExtractor {
    /// Extract all feature types from a single CsiData snapshot
    pub fn extract(&self, csi_data: &CsiData) -> CsiFeatures;
}
```

### MotionDetector (Motion Analysis Root)

```rust
pub struct MotionDetector {
    config: MotionDetectorConfig,
    previous_confidence: f64,
    motion_history: VecDeque<MotionScore>,
    baseline_variance: Option<f64>,
}

impl MotionDetector {
    /// Analyze motion from extracted features
    pub fn analyze_motion(&self, features: &CsiFeatures) -> MotionAnalysis;

    /// Full detection pipeline: analyze -> score -> smooth -> threshold
    pub fn detect_human(&mut self, features: &CsiFeatures) -> HumanDetectionResult;

    /// Record baseline variance for adaptive detection
    pub fn calibrate(&mut self, features: &CsiFeatures);
}
```

---

## Value Objects

### CsiData

```rust
pub struct CsiData {
    pub timestamp: DateTime<Utc>,
    pub amplitude: Array2<f64>,     // (num_antennas x num_subcarriers)
    pub phase: Array2<f64>,         // (num_antennas x num_subcarriers), radians
    pub frequency: f64,             // center frequency in Hz
    pub bandwidth: f64,             // bandwidth in Hz
    pub num_subcarriers: usize,
    pub num_antennas: usize,
    pub snr: f64,                   // signal-to-noise ratio in dB
    pub metadata: CsiMetadata,
}
```

### Spectrogram

```rust
pub struct Spectrogram {
    pub data: Array2<f64>,          // (n_freq x n_time) power/magnitude
    pub n_freq: usize,             // frequency bins (window_size/2 + 1)
    pub n_time: usize,             // time frames
    pub freq_resolution: f64,      // Hz per bin
    pub time_resolution: f64,      // seconds per frame
}
```

### SubcarrierSelection

```rust
pub struct SubcarrierSelection {
    pub selected_indices: Vec<usize>,       // ranked by sensitivity, descending
    pub sensitivity_scores: Vec<f64>,       // variance ratio for ALL subcarriers
    pub selected_data: Option<Array2<f64>>, // filtered matrix (optional)
}
```

### BodyVelocityProfile

```rust
pub struct BodyVelocityProfile {
    pub data: Array2<f64>,          // (n_velocity_bins x n_time_frames)
    pub velocity_bins: Vec<f64>,   // velocity value for each row (m/s)
    pub n_time: usize,
    pub time_resolution: f64,      // seconds per frame
    pub velocity_resolution: f64,  // m/s per bin
}
```

### BreathingEstimate

```rust
pub struct BreathingEstimate {
    pub rate_bpm: f64,              // breaths per minute
    pub confidence: f64,           // combined confidence (0.0-1.0)
    pub period_seconds: f64,       // estimated breathing period
    pub autocorrelation_peak: f64, // periodicity quality
    pub fresnel_confidence: f64,   // Fresnel model match
    pub amplitude_variation: f64,  // observed amplitude variation
}
```

### MotionScore

```rust
pub struct MotionScore {
    pub total: f64,                 // weighted composite (0.0-1.0)
    pub variance_component: f64,
    pub correlation_component: f64,
    pub phase_component: f64,
    pub doppler_component: Option<f64>,
}
```

### HampelResult

```rust
pub struct HampelResult {
    pub filtered: Vec<f64>,         // outliers replaced with local median
    pub outlier_indices: Vec<usize>,
    pub medians: Vec<f64>,         // local median at each sample
    pub sigma_estimates: Vec<f64>, // estimated local sigma at each sample
}
```

### FresnelGeometry

```rust
pub struct FresnelGeometry {
    pub d_tx_body: f64,             // TX to body distance (meters)
    pub d_body_rx: f64,             // body to RX distance (meters)
    pub frequency: f64,            // carrier frequency (Hz)
}

impl FresnelGeometry {
    pub fn wavelength(&self) -> f64;
    pub fn fresnel_radius(&self, n: u32) -> f64;
    pub fn phase_change(&self, displacement_m: f64) -> f64;
    pub fn expected_amplitude_variation(&self, displacement_m: f64) -> f64;
}
```

---

## Domain Events

### Preprocessing Events

```rust
pub enum PreprocessingEvent {
    /// Raw CSI frame cleaned through the full pipeline
    FrameCleaned {
        timestamp: DateTime<Utc>,
        num_antennas: usize,
        num_subcarriers: usize,
        noise_filtered: bool,
        windowed: bool,
        normalized: bool,
    },

    /// Outliers detected and replaced by Hampel filter
    OutliersDetected {
        subcarrier_indices: Vec<usize>,
        replacement_values: Vec<f64>,
        contamination_ratio: f64,
    },

    /// Phase sanitization completed
    PhaseSanitized {
        method: UnwrappingMethod,
        outliers_removed: usize,
        smoothing_applied: bool,
    },
}
```

### Feature Extraction Events

```rust
pub enum FeatureExtractionEvent {
    /// Spectrogram computed from temporal CSI stream
    SpectrogramGenerated {
        n_time: usize,
        n_freq: usize,
        window_size: usize,
        window_fn: WindowFunction,
    },

    /// Top-K sensitive subcarriers selected
    SubcarriersSelected {
        top_k_indices: Vec<usize>,
        sensitivity_scores: Vec<f64>,
        min_sensitivity_threshold: f64,
    },

    /// Body Velocity Profile extracted
    BvpExtracted {
        n_velocity_bins: usize,
        n_time_frames: usize,
        max_velocity: f64,
        carrier_frequency: f64,
    },
}
```

### Motion Analysis Events

```rust
pub enum MotionAnalysisEvent {
    /// Human motion detected above threshold
    MotionDetected {
        score: MotionScore,
        confidence: f64,
        threshold: f64,
        timestamp: DateTime<Utc>,
    },

    /// Breathing detected via Fresnel zone model
    BreathingDetected {
        rate_bpm: f64,
        amplitude_variation: f64,
        fresnel_confidence: f64,
        autocorrelation_peak: f64,
    },

    /// Presence state changed (entered or left)
    PresenceChanged {
        previous: bool,
        current: bool,
        smoothed_confidence: f64,
        timestamp: DateTime<Utc>,
    },

    /// Detector calibrated with baseline variance
    BaselineCalibrated {
        baseline_variance: f64,
        timestamp: DateTime<Utc>,
    },
}
```

---

## Invariants

### CSI Preprocessing Invariants

1. **Conjugate multiplication requires >= 2 antenna elements.** `compute_ratio_matrix` returns `CsiRatioError::InsufficientAntennas` if `n_ant < 2`. Without two antennas, there is no pair to cancel common-mode offsets.

2. **Hampel filter window must be >= 1 (half_window > 0).** A zero-width window cannot compute a local median. Enforced by `HampelError::InvalidWindow`.

3. **Phase data must be within configured range before sanitization.** Default range is `[-pi, pi]`. Enforced by `PhaseSanitizer::validate_phase_data`.

4. **Antenna stream lengths must match for conjugate multiplication.** `conjugate_multiply` returns `CsiRatioError::LengthMismatch` if `h_ref.len() != h_target.len()`.

### Feature Extraction Invariants

5. **Spectrogram window size must be > 0 and signal must be >= window_size samples.** Enforced by `SpectrogramError::SignalTooShort` and `SpectrogramError::InvalidWindowSize`.

6. **Subcarrier selection must receive matching subcarrier counts.** Motion and static data must have the same number of columns. Enforced by `SelectionError::SubcarrierCountMismatch`.

7. **BVP requires >= window_size temporal samples.** Insufficient history prevents STFT computation. Enforced by `BvpError::InsufficientSamples`.

8. **BVP carrier frequency must be > 0 for wavelength calculation.** Zero frequency would produce a division-by-zero in the Doppler-to-velocity mapping.

### Motion Analysis Invariants

9. **Fresnel geometry requires positive distances (d_tx_body > 0, d_body_rx > 0).** Zero or negative distances are physically impossible. Enforced by `FresnelError::InvalidDistance`.

10. **Fresnel frequency must be positive.** Required for wavelength computation. Enforced by `FresnelError::InvalidFrequency`.

11. **Breathing estimation requires >= 10 amplitude samples.** Fewer samples cannot support autocorrelation analysis. Enforced by `FresnelError::InsufficientData`.

12. **Motion detector history does not exceed configured max size.** Oldest entries are evicted via `VecDeque::pop_front` when capacity is reached.

---

## Domain Services

### CsiPreprocessor

Orchestrates the cleaning pipeline for a single CSI frame.

```rust
pub struct CsiPreprocessor {
    noise_threshold: f64,
}

impl CsiPreprocessor {
    /// Remove subcarriers below noise floor (amplitude in dB < threshold)
    pub fn remove_noise(&self, csi_data: &CsiData) -> Result<CsiData, CsiProcessorError>;

    /// Apply Hamming window to reduce spectral leakage
    pub fn apply_windowing(&self, csi_data: &CsiData) -> Result<CsiData, CsiProcessorError>;

    /// Normalize amplitude to unit variance
    pub fn normalize_amplitude(&self, csi_data: &CsiData) -> Result<CsiData, CsiProcessorError>;
}
```

### PhaseSanitizer

Full phase cleaning pipeline: unwrap -> outlier removal -> smoothing -> noise filtering.

```rust
pub struct PhaseSanitizer {
    config: PhaseSanitizerConfig,
    statistics: SanitizationStatistics,
}

impl PhaseSanitizer {
    /// Complete sanitization pipeline (all four stages)
    pub fn sanitize_phase(
        &mut self,
        phase_data: &Array2<f64>,
    ) -> Result<Array2<f64>, PhaseSanitizationError>;
}
```

### FresnelBreathingEstimator

Physics-based breathing detection using Fresnel zone geometry.

```rust
pub struct FresnelBreathingEstimator {
    geometry: FresnelGeometry,
    min_displacement: f64,  // 3mm default
    max_displacement: f64,  // 15mm default
}

impl FresnelBreathingEstimator {
    /// Check if amplitude variation matches Fresnel breathing model
    pub fn breathing_confidence(&self, observed_amplitude_variation: f64) -> f64;

    /// Estimate breathing rate via autocorrelation + Fresnel validation
    pub fn estimate_breathing_rate(
        &self,
        amplitude_signal: &[f64],
        sample_rate: f64,
    ) -> Result<BreathingEstimate, FresnelError>;
}
```

---

## Context Map

```
+--------------------------------------------------------------+
|              Signal Processing System                         |
+--------------------------------------------------------------+
|                                                               |
|  +----------------+  Published   +------------------+         |
|  |     CSI        | Language     | Feature          |         |
|  | Preprocessing  |------------>| Extraction       |         |
|  |    Context     |  CsiData    |    Context        |         |
|  +-------+--------+             +--------+---------+         |
|          |                               |                    |
|          | Publishes                     | Publishes          |
|          | CleanedCsiData               | CsiFeatures        |
|          v                               v                    |
|  +-------+-------------------------------+---------+         |
|  |              Event Bus (Domain Events)           |         |
|  +---------------------------+---------------------+         |
|                              |                                |
|                              | Subscribes                     |
|                              v                                |
|                    +---------+---------+                      |
|                    |    Motion         |                      |
|                    |    Analysis       |                      |
|                    |    Context        |                      |
|                    +-------------------+                      |
|                                                               |
+---------------------------------------------------------------+
|                   DOWNSTREAM (Customer/Supplier)              |
|  +-----------------+  +------------------+ +--------------+   |
|  | wifi-densepose  |  | wifi-densepose   | |wifi-densepose|   |
|  |      -nn        |  |      -mat        | |   -train     |   |
|  | (consumes       |  | (consumes        | |(consumes     |   |
|  |  CsiFeatures,   |  |  BreathingEst,   | | CsiFeatures) |   |
|  |  Spectrogram)   |  |  MotionScore)    | |              |   |
|  +-----------------+  +------------------+ +--------------+   |
+---------------------------------------------------------------+
|                   UPSTREAM (Conformist)                        |
|  +-----------------+  +------------------+                    |
|  | wifi-densepose  |  | wifi-densepose   |                    |
|  |     -core       |  |    -hardware     |                    |
|  | (CsiFrame       |  | (ESP32 raw CSI   |                    |
|  |  primitives)    |  |  data ingestion) |                    |
|  +-----------------+  +------------------+                    |
+---------------------------------------------------------------+
```

**Relationship Types**:
- Preprocessing -> Feature Extraction: **Published Language** (CsiData is the shared contract)
- Preprocessing -> Motion Analysis: **Customer/Supplier** (Preprocessing supplies cleaned data)
- Feature Extraction -> Motion Analysis: **Customer/Supplier** (Features supplies CsiFeatures)
- Signal -> wifi-densepose-nn: **Customer/Supplier** (Signal publishes Spectrogram, BVP)
- Signal -> wifi-densepose-mat: **Customer/Supplier** (Signal publishes BreathingEstimate, MotionScore)
- Signal <- wifi-densepose-core: **Conformist** (Signal adapts to core CsiFrame types)
- Signal <- wifi-densepose-hardware: **Conformist** (Signal adapts to raw ESP32 CSI format)

---

## Anti-Corruption Layers

### Hardware ACL (Upstream)

Translates raw ESP32 CSI packets into the signal crate's `CsiData` value object, normalizing hardware-specific quirks (LLTF/HT-LTF format differences, antenna mapping, null subcarrier handling).

```rust
/// Normalizes vendor-specific CSI frames to canonical CsiData
pub struct HardwareNormalizer {
    hardware_type: HardwareType,
}

impl HardwareNormalizer {
    /// Convert raw hardware bytes to canonical CsiData
    pub fn normalize(
        &self,
        raw_csi: &[u8],
        hardware_type: HardwareType,
    ) -> Result<CanonicalCsiFrame, HardwareNormError>;
}

pub enum HardwareType {
    Esp32S3,
    Intel5300,
    AtherosAr9580,
    Simulation,
}
```

### Neural Network ACL (Downstream)

Adapts signal processing outputs (Spectrogram, BVP, CsiFeatures) into tensor formats expected by the `wifi-densepose-nn` crate. This boundary prevents neural network model details from leaking into the signal processing domain.

```rust
/// Adapts signal crate types to neural network tensor format
pub struct SignalToTensorAdapter;

impl SignalToTensorAdapter {
    /// Convert Spectrogram to CNN-ready 2D tensor
    pub fn spectrogram_to_tensor(spec: &Spectrogram) -> Array2<f32> {
        spec.data.mapv(|v| v as f32)
    }

    /// Convert BVP to domain-independent velocity tensor
    pub fn bvp_to_tensor(bvp: &BodyVelocityProfile) -> Array2<f32> {
        bvp.data.mapv(|v| v as f32)
    }

    /// Convert selected subcarrier data to reduced-dimension input
    pub fn selected_csi_to_tensor(
        selection: &SubcarrierSelection,
        data: &Array2<f64>,
    ) -> Result<Array2<f32>, SelectionError> {
        let extracted = extract_selected(data, selection)?;
        Ok(extracted.mapv(|v| v as f32))
    }
}
```

### MAT ACL (Downstream)

Adapts motion analysis outputs for the Mass Casualty Assessment Tool, translating domain-generic motion scores and breathing estimates into disaster-context vital signs.

```rust
/// Adapts signal processing outputs for disaster assessment
pub struct SignalToMatAdapter;

impl SignalToMatAdapter {
    /// Convert BreathingEstimate to MAT-domain BreathingPattern
    pub fn to_breathing_pattern(est: &BreathingEstimate) -> BreathingPattern {
        BreathingPattern {
            rate_bpm: est.rate_bpm as f32,
            amplitude: est.amplitude_variation as f32,
            regularity: est.autocorrelation_peak as f32,
            pattern_type: classify_breathing_type(est.rate_bpm),
        }
    }

    /// Convert MotionScore to MAT-domain presence indicator
    pub fn to_presence_indicator(score: &MotionScore) -> PresenceIndicator {
        PresenceIndicator {
            detected: score.total > 0.3,
            confidence: score.total,
            motion_level: classify_motion_level(score),
        }
    }
}
```
