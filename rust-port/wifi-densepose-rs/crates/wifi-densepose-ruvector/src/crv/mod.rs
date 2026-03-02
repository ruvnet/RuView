//! CRV (Coordinate Remote Viewing) signal-line integration for WiFi-DensePose.
//!
//! Maps the 6-stage CRV protocol from [`ruvector_crv`] to WiFi CSI sensing:
//!
//! | CRV Stage | WiFi-DensePose Mapping |
//! |-----------|------------------------|
//! | I  (Gestalt)       | CSI amplitude/phase pattern classification |
//! | II (Sensory)       | CSI feature extraction (roughness, spectral centroid, power, ...) |
//! | III (Dimensional)  | AP mesh topology as spatial graph |
//! | IV (Emotional/AOL) | Coherence gate state as AOL detection |
//! | V  (Interrogation) | Differentiable search over accumulated CSI features |
//! | VI (Composite)     | MinCut person partitioning |
//!
//! # Entry Point
//!
//! [`WifiCrvPipeline`] is the main facade. Create one with [`WifiCrvConfig`],
//! then feed CSI frames through the pipeline stages.

use ruvector_crv::{
    AOLDetection, ConvergenceResult, CrvConfig, CrvError, CrvSessionManager, GestaltType,
    GeometricKind, SensoryModality, SketchElement, SpatialRelationType, SpatialRelationship,
    StageIData, StageIIData, StageIIIData, StageIVData, StageVData, StageVIData,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// An access point node in the WiFi mesh topology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApNode {
    /// Unique identifier for this access point.
    pub id: String,
    /// Position in 2D floor-plan coordinates (metres).
    pub position: (f32, f32),
    /// Estimated coverage radius (metres).
    pub coverage_radius: f32,
}

/// A link between two access points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApLink {
    /// Source AP identifier.
    pub from: String,
    /// Destination AP identifier.
    pub to: String,
    /// Measured signal strength between the two APs (0.0-1.0 normalised).
    pub signal_strength: f32,
}

/// Coherence gate state mapped to CRV AOL interpretation.
///
/// The coherence gate from the viewpoint module produces a binary
/// accept/reject decision. This enum extends it with richer semantics
/// for the CRV pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoherenceGateState {
    /// Clean signal line -- coherence is high, proceed normally.
    Accept,
    /// Mild AOL -- use prediction but flag for review.
    PredictOnly,
    /// Strong AOL -- pure noise, discard this frame.
    Reject,
    /// Environment shift detected -- recalibrate the pipeline.
    Recalibrate,
}

/// Result of processing a single CSI frame through Stages I and II.
#[derive(Debug, Clone)]
pub struct CsiCrvResult {
    /// Classified gestalt type for this frame.
    pub gestalt: GestaltType,
    /// Confidence of the gestalt classification (0.0-1.0).
    pub gestalt_confidence: f32,
    /// Stage I embedding (Poincare ball).
    pub gestalt_embedding: Vec<f32>,
    /// Stage II sensory embedding.
    pub sensory_embedding: Vec<f32>,
}

/// Thresholds for gestalt classification from CSI statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GestaltThresholds {
    /// Variance threshold above which the signal is considered dynamic.
    pub variance_high: f32,
    /// Variance threshold below which the signal is considered static.
    pub variance_low: f32,
    /// Periodicity score above which the signal is considered periodic.
    pub periodicity_threshold: f32,
    /// Energy spike threshold (ratio of max to mean amplitude).
    pub energy_spike_ratio: f32,
    /// Structure score threshold for manmade detection.
    pub structure_threshold: f32,
    /// Null-subcarrier fraction above which the signal is classified as Water.
    pub null_fraction_threshold: f32,
}

impl Default for GestaltThresholds {
    fn default() -> Self {
        Self {
            variance_high: 0.15,
            variance_low: 0.03,
            periodicity_threshold: 0.5,
            energy_spike_ratio: 3.0,
            structure_threshold: 0.6,
            null_fraction_threshold: 0.3,
        }
    }
}

/// Configuration for the WiFi CRV pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiCrvConfig {
    /// Embedding dimensionality (passed to [`CrvConfig`]).
    pub dimensions: usize,
    /// Thresholds for CSI gestalt classification.
    pub gestalt_thresholds: GestaltThresholds,
    /// Convergence threshold for cross-session matching (0.0-1.0).
    pub convergence_threshold: f32,
}

impl Default for WifiCrvConfig {
    fn default() -> Self {
        Self {
            dimensions: 32,
            gestalt_thresholds: GestaltThresholds::default(),
            convergence_threshold: 0.6,
        }
    }
}

// ---------------------------------------------------------------------------
// CsiGestaltClassifier  (Stage I)
// ---------------------------------------------------------------------------

/// Classifies raw CSI amplitude/phase patterns into CRV gestalt types.
///
/// The mapping from WiFi signal characteristics to gestalt primitives:
///
/// - **Movement**: high variance + periodic (person walking)
/// - **Land**: low variance + stable (empty room)
/// - **Energy**: sudden amplitude spikes (door opening, appliance)
/// - **Natural**: smooth gradual changes (temperature drift, slow fading)
/// - **Manmade**: regular structured patterns (HVAC, machinery)
/// - **Water**: many null/zero subcarriers (deep absorption)
#[derive(Debug, Clone)]
pub struct CsiGestaltClassifier {
    thresholds: GestaltThresholds,
}

impl CsiGestaltClassifier {
    /// Create a new classifier with the given thresholds.
    pub fn new(thresholds: GestaltThresholds) -> Self {
        Self { thresholds }
    }

    /// Classify a CSI frame into a gestalt type with confidence.
    ///
    /// Computes variance, periodicity, energy-spike, structure, and
    /// null-fraction metrics from the amplitude and phase arrays, then
    /// selects the best-matching gestalt type.
    ///
    /// Returns `(gestalt_type, confidence)` where confidence is in `[0, 1]`.
    pub fn classify(&self, amplitudes: &[f32], phases: &[f32]) -> (GestaltType, f32) {
        if amplitudes.is_empty() {
            return (GestaltType::Land, 0.0);
        }

        let variance = Self::compute_variance(amplitudes);
        let periodicity = Self::compute_periodicity(amplitudes);
        let energy_spike = Self::compute_energy_spike(amplitudes);
        let structure = Self::compute_structure(amplitudes, phases);
        let null_frac = Self::compute_null_fraction(amplitudes);

        // Score each gestalt type using priority-based gating.
        //
        // Evaluation order:
        //   1. Water (null subcarriers -- very distinctive, takes priority)
        //   2. Energy (sudden spikes)
        //   3. Movement (high variance + periodic)
        //   4. Land (low variance, stable)
        //   5. Natural (moderate variance, smooth)
        //   6. Manmade (structured -- suppressed when others are strong)
        let mut scores = [(GestaltType::Land, 0.0f32); 6];

        // Water: many null subcarriers (highest priority).
        let water_score = if null_frac > self.thresholds.null_fraction_threshold {
            0.7 + 0.3 * null_frac
        } else {
            0.1 * null_frac
        };
        scores[5] = (GestaltType::Water, water_score);

        // Energy: sudden spikes.
        let energy_score = if energy_spike > self.thresholds.energy_spike_ratio {
            (energy_spike / (self.thresholds.energy_spike_ratio * 2.0)).min(1.0)
        } else {
            0.1 * energy_spike / self.thresholds.energy_spike_ratio.max(1e-6)
        };
        scores[2] = (GestaltType::Energy, energy_score);

        // Movement: high variance + periodic.
        // Suppress when water or energy are strong indicators.
        let movement_suppress = water_score.max(energy_score);
        let movement_score = if variance > self.thresholds.variance_high
            && movement_suppress < 0.6
        {
            0.6 + 0.4 * periodicity
        } else if variance > self.thresholds.variance_high {
            (0.6 + 0.4 * periodicity) * (1.0 - movement_suppress)
        } else {
            0.15 * periodicity
        };
        scores[0] = (GestaltType::Movement, movement_score);

        // Land: low variance + stable.
        let land_score = if variance < self.thresholds.variance_low {
            0.7 + 0.3 * (1.0 - periodicity)
        } else {
            0.1 * (1.0 - variance.min(1.0))
        };
        scores[1] = (GestaltType::Land, land_score);

        // Natural: smooth gradual changes (moderate variance, low periodicity).
        // Structure score being high should not prevent Natural when variance
        // is in the moderate range and periodicity is low.
        let natural_score = if variance > self.thresholds.variance_low
            && variance < self.thresholds.variance_high
            && periodicity < self.thresholds.periodicity_threshold
        {
            0.7 + 0.3 * (1.0 - periodicity)
        } else {
            0.1
        };
        scores[3] = (GestaltType::Natural, natural_score);

        // Manmade: regular structured patterns.
        // Suppress when any other strong indicator is present.
        let manmade_suppress = water_score
            .max(energy_score)
            .max(movement_score)
            .max(natural_score);
        let manmade_score = if structure > self.thresholds.structure_threshold
            && manmade_suppress < 0.5
        {
            0.5 + 0.5 * structure
        } else {
            0.15 * structure * (1.0 - manmade_suppress).max(0.0)
        };
        scores[4] = (GestaltType::Manmade, manmade_score);

        // Pick the highest-scoring type.
        let (best_type, best_score) =
            scores
                .iter()
                .fold((GestaltType::Land, 0.0f32), |(bt, bs), &(gt, gs)| {
                    if gs > bs {
                        (gt, gs)
                    } else {
                        (bt, bs)
                    }
                });

        (best_type, best_score.clamp(0.0, 1.0))
    }

    /// Compute the variance of the amplitude array.
    fn compute_variance(amplitudes: &[f32]) -> f32 {
        let n = amplitudes.len() as f32;
        if n < 2.0 {
            return 0.0;
        }
        let mean = amplitudes.iter().sum::<f32>() / n;
        let var = amplitudes.iter().map(|a| (a - mean).powi(2)).sum::<f32>() / (n - 1.0);
        var / mean.powi(2).max(1e-6) // coefficient of variation squared
    }

    /// Estimate periodicity via autocorrelation of detrended signal.
    ///
    /// Removes the linear trend first so that monotonic signals (ramps, drifts)
    /// do not produce false periodicity peaks. Then searches for the highest
    /// autocorrelation at lags >= 2 (lag 1 is always near 1.0 for smooth signals).
    fn compute_periodicity(amplitudes: &[f32]) -> f32 {
        let n = amplitudes.len();
        if n < 6 {
            return 0.0;
        }

        // Detrend: remove the least-squares linear fit.
        let nf = n as f32;
        let mean_x = (nf - 1.0) / 2.0;
        let mean_y = amplitudes.iter().sum::<f32>() / nf;
        let mut cov_xy = 0.0f32;
        let mut var_x = 0.0f32;
        for (i, &a) in amplitudes.iter().enumerate() {
            let dx = i as f32 - mean_x;
            cov_xy += dx * (a - mean_y);
            var_x += dx * dx;
        }
        let slope = if var_x > 1e-12 { cov_xy / var_x } else { 0.0 };
        let intercept = mean_y - slope * mean_x;

        let detrended: Vec<f32> = amplitudes
            .iter()
            .enumerate()
            .map(|(i, &a)| a - (slope * i as f32 + intercept))
            .collect();

        // Autocorrelation at lag 0.
        let r0: f32 = detrended.iter().map(|x| x * x).sum();
        if r0 < 1e-12 {
            return 0.0;
        }

        // Search for the peak autocorrelation at lags >= 2.
        let mut max_r = 0.0f32;
        for lag in 2..=(n / 2) {
            let r: f32 = detrended
                .iter()
                .zip(detrended[lag..].iter())
                .map(|(a, b)| a * b)
                .sum();
            max_r = max_r.max(r / r0);
        }

        max_r.clamp(0.0, 1.0)
    }

    /// Compute energy spike ratio (max / mean).
    fn compute_energy_spike(amplitudes: &[f32]) -> f32 {
        let mean = amplitudes.iter().sum::<f32>() / amplitudes.len().max(1) as f32;
        let max = amplitudes.iter().cloned().fold(0.0f32, f32::max);
        max / mean.max(1e-6)
    }

    /// Compute a structure score from amplitude and phase regularity.
    ///
    /// High structure score indicates regular, repeating patterns typical
    /// of manmade signals (e.g. periodic OFDM pilot tones, HVAC interference).
    /// A purely smooth/monotonic signal (like a slow ramp) is penalised because
    /// "structure" in the WiFi context implies non-trivial oscillation amplitude.
    fn compute_structure(amplitudes: &[f32], phases: &[f32]) -> f32 {
        if amplitudes.len() < 4 {
            return 0.0;
        }

        // Compute successive differences.
        let diffs: Vec<f32> = amplitudes
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .collect();
        let mean_diff = diffs.iter().sum::<f32>() / diffs.len().max(1) as f32;
        let var_diff = if diffs.len() > 1 {
            diffs.iter().map(|d| (d - mean_diff).powi(2)).sum::<f32>() / (diffs.len() - 1) as f32
        } else {
            0.0
        };

        // Low variance of differences implies regular structure.
        let amp_regularity = 1.0 / (1.0 + var_diff);

        // Require non-trivial oscillation: mean diff must be a meaningful
        // fraction of the signal range. A slow ramp (tiny diffs) should not
        // score high on structure.
        let min_a = amplitudes.iter().cloned().fold(f32::MAX, f32::min);
        let max_a = amplitudes.iter().cloned().fold(f32::MIN, f32::max);
        let range = (max_a - min_a).max(1e-6);
        let diff_significance = (mean_diff / range).clamp(0.0, 1.0);

        // Phase regularity: how linear is the phase progression?
        let phase_regularity = if phases.len() >= 4 {
            let pd: Vec<f32> = phases.windows(2).map(|w| w[1] - w[0]).collect();
            let mean_pd = pd.iter().sum::<f32>() / pd.len() as f32;
            let var_pd = pd.iter().map(|d| (d - mean_pd).powi(2)).sum::<f32>()
                / (pd.len().max(1) - 1).max(1) as f32;
            1.0 / (1.0 + var_pd)
        } else {
            0.5
        };

        let raw = amp_regularity * 0.6 + phase_regularity * 0.4;
        // Scale by diff significance so smooth/monotonic signals get low structure.
        (raw * diff_significance).clamp(0.0, 1.0)
    }

    /// Fraction of subcarriers with near-zero amplitude.
    fn compute_null_fraction(amplitudes: &[f32]) -> f32 {
        let threshold = 1e-3;
        let nulls = amplitudes.iter().filter(|&&a| a.abs() < threshold).count();
        nulls as f32 / amplitudes.len().max(1) as f32
    }
}

// ---------------------------------------------------------------------------
// CsiSensoryEncoder  (Stage II)
// ---------------------------------------------------------------------------

/// Extracts sensory-like features from CSI data for Stage II encoding.
///
/// The mapping from signal processing metrics to sensory modalities:
///
/// - **Texture** -> amplitude roughness (high-frequency variance)
/// - **Color** -> frequency-domain spectral centroid
/// - **Temperature** -> signal energy (total power)
/// - **Sound** -> temporal periodicity (breathing/heartbeat frequency)
/// - **Luminosity** -> SNR / coherence level
/// - **Dimension** -> subcarrier spread (bandwidth utilisation)
#[derive(Debug, Clone)]
pub struct CsiSensoryEncoder;

impl CsiSensoryEncoder {
    /// Create a new sensory encoder.
    pub fn new() -> Self {
        Self
    }

    /// Extract sensory impressions from a CSI frame.
    ///
    /// Returns a list of `(SensoryModality, descriptor_string)` pairs
    /// suitable for feeding into [`ruvector_crv::StageIIEncoder`].
    pub fn extract(
        &self,
        amplitudes: &[f32],
        phases: &[f32],
    ) -> Vec<(SensoryModality, String)> {
        let mut impressions = Vec::new();

        // Texture: amplitude roughness (high-freq variance).
        let roughness = self.amplitude_roughness(amplitudes);
        let texture_desc = if roughness > 0.5 {
            "rough coarse"
        } else if roughness > 0.2 {
            "moderate grain"
        } else {
            "smooth flat"
        };
        impressions.push((SensoryModality::Texture, texture_desc.to_string()));

        // Color: spectral centroid (maps to a pseudo colour).
        let centroid = self.spectral_centroid(amplitudes);
        let color_desc = if centroid > 0.7 {
            "blue high-freq"
        } else if centroid > 0.4 {
            "green mid-freq"
        } else {
            "red low-freq"
        };
        impressions.push((SensoryModality::Color, color_desc.to_string()));

        // Temperature: signal energy (total power).
        let energy = self.signal_energy(amplitudes);
        let temp_desc = if energy > 1.0 {
            "hot high-power"
        } else if energy > 0.3 {
            "warm moderate"
        } else {
            "cold low-power"
        };
        impressions.push((SensoryModality::Temperature, temp_desc.to_string()));

        // Sound: temporal periodicity.
        let periodicity = CsiGestaltClassifier::compute_periodicity(amplitudes);
        let sound_desc = if periodicity > 0.6 {
            "rhythmic periodic"
        } else if periodicity > 0.3 {
            "hum steady"
        } else {
            "silent still"
        };
        impressions.push((SensoryModality::Sound, sound_desc.to_string()));

        // Luminosity: phase coherence as SNR proxy.
        let snr = self.phase_coherence(phases);
        let lum_desc = if snr > 0.7 {
            "bright clear"
        } else if snr > 0.4 {
            "dim moderate"
        } else {
            "dark noisy"
        };
        impressions.push((SensoryModality::Luminosity, lum_desc.to_string()));

        // Dimension: subcarrier spread.
        let spread = self.subcarrier_spread(amplitudes);
        let dim_desc = if spread > 0.7 {
            "vast wide"
        } else if spread > 0.3 {
            "medium regular"
        } else {
            "narrow compact"
        };
        impressions.push((SensoryModality::Dimension, dim_desc.to_string()));

        impressions
    }

    /// Amplitude roughness: mean absolute difference normalised by signal range.
    ///
    /// High roughness means large sample-to-sample jumps relative to the
    /// dynamic range, indicating irregular high-frequency amplitude variation.
    fn amplitude_roughness(&self, amplitudes: &[f32]) -> f32 {
        if amplitudes.len() < 3 {
            return 0.0;
        }
        let min_a = amplitudes.iter().cloned().fold(f32::MAX, f32::min);
        let max_a = amplitudes.iter().cloned().fold(f32::MIN, f32::max);
        let range = (max_a - min_a).max(1e-6);

        let mean_abs_diff: f32 = amplitudes
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .sum::<f32>()
            / (amplitudes.len() - 1) as f32;

        (mean_abs_diff / range).clamp(0.0, 1.0)
    }

    /// Spectral centroid: weighted mean of subcarrier indices.
    fn spectral_centroid(&self, amplitudes: &[f32]) -> f32 {
        let total: f32 = amplitudes.iter().sum();
        if total < 1e-12 {
            return 0.5;
        }
        let weighted: f32 = amplitudes
            .iter()
            .enumerate()
            .map(|(i, &a)| i as f32 * a)
            .sum();
        let centroid = weighted / total;
        let n = amplitudes.len().max(1) as f32;
        (centroid / n).clamp(0.0, 1.0)
    }

    /// Signal energy: mean squared amplitude.
    fn signal_energy(&self, amplitudes: &[f32]) -> f32 {
        let n = amplitudes.len().max(1) as f32;
        amplitudes.iter().map(|a| a * a).sum::<f32>() / n
    }

    /// Phase coherence: magnitude of the mean unit phasor.
    fn phase_coherence(&self, phases: &[f32]) -> f32 {
        if phases.is_empty() {
            return 0.0;
        }
        let n = phases.len() as f32;
        let sum_cos: f32 = phases.iter().map(|p| p.cos()).sum();
        let sum_sin: f32 = phases.iter().map(|p| p.sin()).sum();
        ((sum_cos / n).powi(2) + (sum_sin / n).powi(2)).sqrt()
    }

    /// Subcarrier spread: fraction of subcarriers above a threshold.
    fn subcarrier_spread(&self, amplitudes: &[f32]) -> f32 {
        if amplitudes.is_empty() {
            return 0.0;
        }
        let max = amplitudes.iter().cloned().fold(0.0f32, f32::max);
        let threshold = max * 0.1;
        let active = amplitudes.iter().filter(|&&a| a > threshold).count();
        active as f32 / amplitudes.len() as f32
    }
}

// ---------------------------------------------------------------------------
// WifiCrvPipeline  (main entry point)
// ---------------------------------------------------------------------------

/// Main entry point for the WiFi CRV signal-line integration.
///
/// Wraps [`CrvSessionManager`] with WiFi-DensePose domain logic so that
/// callers feed CSI frames and AP topology rather than raw CRV stage data.
pub struct WifiCrvPipeline {
    /// Underlying CRV session manager.
    manager: CrvSessionManager,
    /// Gestalt classifier for Stage I.
    gestalt: CsiGestaltClassifier,
    /// Sensory encoder for Stage II.
    sensory: CsiSensoryEncoder,
    /// Pipeline configuration.
    config: WifiCrvConfig,
}

impl WifiCrvPipeline {
    /// Create a new WiFi CRV pipeline.
    pub fn new(config: WifiCrvConfig) -> Self {
        let crv_config = CrvConfig {
            dimensions: config.dimensions,
            convergence_threshold: config.convergence_threshold,
            ..CrvConfig::default()
        };
        let manager = CrvSessionManager::new(crv_config);
        let gestalt = CsiGestaltClassifier::new(config.gestalt_thresholds.clone());
        let sensory = CsiSensoryEncoder::new();

        Self {
            manager,
            gestalt,
            sensory,
            config,
        }
    }

    /// Create a new CRV session for a room.
    ///
    /// The `session_id` identifies the sensing session and `room_id`
    /// acts as the CRV target coordinate so that cross-session
    /// convergence can be computed per room.
    pub fn create_session(
        &mut self,
        session_id: &str,
        room_id: &str,
    ) -> Result<(), CrvError> {
        self.manager
            .create_session(session_id.to_string(), room_id.to_string())
    }

    /// Process a CSI frame through Stages I and II.
    ///
    /// Classifies the frame into a gestalt type, extracts sensory features,
    /// and adds both embeddings to the session.
    pub fn process_csi_frame(
        &mut self,
        session_id: &str,
        amplitudes: &[f32],
        phases: &[f32],
    ) -> Result<CsiCrvResult, CrvError> {
        if amplitudes.is_empty() {
            return Err(CrvError::EmptyInput(
                "CSI amplitudes are empty".to_string(),
            ));
        }

        // Stage I: Gestalt classification.
        let (gestalt_type, confidence) = self.gestalt.classify(amplitudes, phases);

        // Build a synthetic ideogram stroke from the amplitude envelope
        // so the CRV Stage I encoder can produce a Poincare ball embedding.
        let stroke: Vec<(f32, f32)> = amplitudes
            .iter()
            .enumerate()
            .map(|(i, &a)| (i as f32 / amplitudes.len().max(1) as f32, a))
            .collect();

        let stage_i = StageIData {
            stroke,
            spontaneous_descriptor: format!("{:?}", gestalt_type).to_lowercase(),
            classification: gestalt_type,
            confidence,
        };

        let gestalt_embedding = self.manager.add_stage_i(session_id, &stage_i)?;

        // Stage II: Sensory feature extraction.
        let impressions = self.sensory.extract(amplitudes, phases);
        let stage_ii = StageIIData {
            impressions,
            feature_vector: None,
        };

        let sensory_embedding = self.manager.add_stage_ii(session_id, &stage_ii)?;

        Ok(CsiCrvResult {
            gestalt: gestalt_type,
            gestalt_confidence: confidence,
            gestalt_embedding,
            sensory_embedding,
        })
    }

    /// Add AP mesh topology as Stage III spatial data.
    ///
    /// Each AP becomes a sketch element positioned at its floor-plan
    /// coordinates with scale proportional to coverage radius. Links
    /// become spatial relationships with strength from signal strength.
    ///
    /// Returns the Stage III embedding.
    pub fn add_mesh_topology(
        &mut self,
        session_id: &str,
        nodes: &[ApNode],
        links: &[ApLink],
    ) -> Result<Vec<f32>, CrvError> {
        if nodes.is_empty() {
            return Err(CrvError::EmptyInput("No AP nodes provided".to_string()));
        }

        let sketch_elements: Vec<SketchElement> = nodes
            .iter()
            .map(|ap| SketchElement {
                label: ap.id.clone(),
                kind: GeometricKind::Circle,
                position: ap.position,
                scale: Some(ap.coverage_radius),
            })
            .collect();

        let relationships: Vec<SpatialRelationship> = links
            .iter()
            .map(|link| SpatialRelationship {
                from: link.from.clone(),
                to: link.to.clone(),
                relation: SpatialRelationType::Connected,
                strength: link.signal_strength,
            })
            .collect();

        let stage_iii = StageIIIData {
            sketch_elements,
            relationships,
        };

        self.manager.add_stage_iii(session_id, &stage_iii)
    }

    /// Add a coherence gate state as Stage IV AOL data.
    ///
    /// Maps the coherence gate decision to AOL semantics:
    /// - `Accept` -> clean signal line (no AOL)
    /// - `PredictOnly` -> mild AOL (flagged but usable)
    /// - `Reject` -> strong AOL (noise, discard)
    /// - `Recalibrate` -> environment shift (AOL + tangible change)
    ///
    /// Returns the Stage IV embedding.
    pub fn add_coherence_state(
        &mut self,
        session_id: &str,
        state: CoherenceGateState,
        score: f32,
    ) -> Result<Vec<f32>, CrvError> {
        let (emotional_impact, tangibles, intangibles, aol_detections) = match state {
            CoherenceGateState::Accept => (
                vec![("confidence".to_string(), 0.8)],
                vec!["stable environment".to_string()],
                vec!["clean signal line".to_string()],
                vec![],
            ),
            CoherenceGateState::PredictOnly => (
                vec![("uncertainty".to_string(), 0.5)],
                vec!["prediction mode".to_string()],
                vec!["mild interference".to_string()],
                vec![AOLDetection {
                    content: "mild coherence loss".to_string(),
                    timestamp_ms: 0,
                    flagged: true,
                    anomaly_score: score.clamp(0.0, 1.0),
                }],
            ),
            CoherenceGateState::Reject => (
                vec![("noise".to_string(), 0.9)],
                vec![],
                vec!["signal contaminated".to_string()],
                vec![AOLDetection {
                    content: "strong incoherence".to_string(),
                    timestamp_ms: 0,
                    flagged: true,
                    anomaly_score: 1.0,
                }],
            ),
            CoherenceGateState::Recalibrate => (
                vec![("disruption".to_string(), 0.7)],
                vec!["environment change".to_string()],
                vec!["recalibration needed".to_string()],
                vec![AOLDetection {
                    content: "environment shift".to_string(),
                    timestamp_ms: 0,
                    flagged: false,
                    anomaly_score: score.clamp(0.0, 1.0),
                }],
            ),
        };

        let stage_iv = StageIVData {
            emotional_impact,
            tangibles,
            intangibles,
            aol_detections,
        };

        self.manager.add_stage_iv(session_id, &stage_iv)
    }

    /// Run Stage V interrogation on a session.
    ///
    /// Given a query embedding (e.g. encoding of "is person moving?"),
    /// probes the accumulated session data via differentiable search.
    pub fn interrogate(
        &mut self,
        session_id: &str,
        query_embedding: &[f32],
    ) -> Result<StageVData, CrvError> {
        if query_embedding.is_empty() {
            return Err(CrvError::EmptyInput(
                "Query embedding is empty".to_string(),
            ));
        }

        // Probe all stages 1-4 with the query.
        let probes: Vec<(&str, u8, Vec<f32>)> = (1..=4)
            .map(|stage| ("csi-query", stage, query_embedding.to_vec()))
            .collect();

        let k = 3.min(self.manager.session_entry_count(session_id));
        if k == 0 {
            return Err(CrvError::EmptyInput(
                "Session has no entries to interrogate".to_string(),
            ));
        }

        self.manager.run_stage_v(session_id, &probes, k)
    }

    /// Run Stage VI person partitioning on a session.
    ///
    /// Uses MinCut to partition the accumulated session data into
    /// distinct target aspects -- in the WiFi sensing context these
    /// correspond to distinct persons or environment zones.
    pub fn partition_persons(
        &mut self,
        session_id: &str,
    ) -> Result<StageVIData, CrvError> {
        self.manager.run_stage_vi(session_id)
    }

    /// Find cross-session convergence for a room.
    ///
    /// Analyses all sessions targeting the given `room_id` to find
    /// agreement between independent sensing sessions. Higher convergence
    /// indicates that multiple sessions see the same signal patterns.
    pub fn find_cross_room_convergence(
        &self,
        room_id: &str,
        min_similarity: f32,
    ) -> Result<ConvergenceResult, CrvError> {
        self.manager.find_convergence(room_id, min_similarity)
    }

    /// Get the number of entries in a session.
    pub fn session_entry_count(&self, session_id: &str) -> usize {
        self.manager.session_entry_count(session_id)
    }

    /// Get the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.manager.session_count()
    }

    /// Remove a session.
    pub fn remove_session(&mut self, session_id: &str) -> bool {
        self.manager.remove_session(session_id)
    }

    /// Get the pipeline configuration.
    pub fn config(&self) -> &WifiCrvConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Helpers --

    fn test_config() -> WifiCrvConfig {
        WifiCrvConfig {
            dimensions: 32,
            gestalt_thresholds: GestaltThresholds::default(),
            convergence_threshold: 0.5,
        }
    }

    /// Generate a periodic amplitude signal.
    fn periodic_signal(n: usize, freq: f32, amplitude: f32) -> Vec<f32> {
        (0..n)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / n as f32).sin().abs() + 0.1)
            .collect()
    }

    /// Generate a constant (static) amplitude signal.
    fn static_signal(n: usize, level: f32) -> Vec<f32> {
        vec![level; n]
    }

    /// Generate linear phases.
    fn linear_phases(n: usize) -> Vec<f32> {
        (0..n).map(|i| i as f32 * 0.1).collect()
    }

    /// Generate random-ish phases.
    fn varied_phases(n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (i as f32 * 2.718).sin() * std::f32::consts::PI)
            .collect()
    }

    // ---- Stage I: Gestalt Classification ----

    #[test]
    fn gestalt_movement_high_variance_periodic() {
        let classifier = CsiGestaltClassifier::new(GestaltThresholds::default());
        let amps = periodic_signal(64, 4.0, 1.0);
        let phases = linear_phases(64);
        let (gestalt, conf) = classifier.classify(&amps, &phases);
        assert_eq!(gestalt, GestaltType::Movement);
        assert!(conf > 0.3, "movement confidence should be reasonable: {conf}");
    }

    #[test]
    fn gestalt_land_low_variance_stable() {
        let classifier = CsiGestaltClassifier::new(GestaltThresholds::default());
        let amps = static_signal(64, 0.5);
        let phases = linear_phases(64);
        let (gestalt, conf) = classifier.classify(&amps, &phases);
        assert_eq!(gestalt, GestaltType::Land);
        assert!(conf > 0.5, "land confidence: {conf}");
    }

    #[test]
    fn gestalt_energy_spike() {
        let classifier = CsiGestaltClassifier::new(GestaltThresholds::default());
        let mut amps = vec![0.1f32; 64];
        amps[32] = 5.0; // large spike
        let phases = linear_phases(64);
        let (gestalt, conf) = classifier.classify(&amps, &phases);
        assert_eq!(gestalt, GestaltType::Energy);
        assert!(conf > 0.3, "energy confidence: {conf}");
    }

    #[test]
    fn gestalt_water_null_subcarriers() {
        let classifier = CsiGestaltClassifier::new(GestaltThresholds::default());
        let mut amps = vec![0.5f32; 64];
        // Set half the subcarriers to zero.
        for i in 0..32 {
            amps[i] = 0.0;
        }
        let phases = linear_phases(64);
        let (gestalt, conf) = classifier.classify(&amps, &phases);
        assert_eq!(gestalt, GestaltType::Water);
        assert!(conf > 0.3, "water confidence: {conf}");
    }

    #[test]
    fn gestalt_manmade_structured() {
        let classifier = CsiGestaltClassifier::new(GestaltThresholds {
            structure_threshold: 0.4,
            ..GestaltThresholds::default()
        });
        // Perfectly regular alternating pattern.
        let amps: Vec<f32> = (0..64).map(|i| if i % 2 == 0 { 1.0 } else { 0.8 }).collect();
        let phases = linear_phases(64);
        let (gestalt, conf) = classifier.classify(&amps, &phases);
        assert_eq!(gestalt, GestaltType::Manmade);
        assert!(conf > 0.3, "manmade confidence: {conf}");
    }

    #[test]
    fn gestalt_natural_smooth_gradual() {
        let classifier = CsiGestaltClassifier::new(GestaltThresholds {
            variance_low: 0.001,
            variance_high: 0.5,
            ..GestaltThresholds::default()
        });
        // Slow ramp -- moderate variance, low periodicity, low structure.
        let amps: Vec<f32> = (0..64).map(|i| 0.3 + 0.005 * i as f32).collect();
        let phases = varied_phases(64);
        let (gestalt, conf) = classifier.classify(&amps, &phases);
        assert_eq!(gestalt, GestaltType::Natural);
        assert!(conf > 0.3, "natural confidence: {conf}");
    }

    #[test]
    fn gestalt_empty_amplitudes() {
        let classifier = CsiGestaltClassifier::new(GestaltThresholds::default());
        let (gestalt, conf) = classifier.classify(&[], &[]);
        assert_eq!(gestalt, GestaltType::Land);
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn gestalt_single_subcarrier() {
        let classifier = CsiGestaltClassifier::new(GestaltThresholds::default());
        let (gestalt, _conf) = classifier.classify(&[1.0], &[0.0]);
        // With a single value variance is 0 => Land.
        assert_eq!(gestalt, GestaltType::Land);
    }

    // ---- Stage II: Sensory Feature Extraction ----

    #[test]
    fn sensory_extraction_returns_six_modalities() {
        let encoder = CsiSensoryEncoder::new();
        let amps = periodic_signal(32, 2.0, 0.5);
        let phases = linear_phases(32);
        let impressions = encoder.extract(&amps, &phases);
        assert_eq!(impressions.len(), 6);
    }

    #[test]
    fn sensory_texture_rough_for_noisy() {
        let encoder = CsiSensoryEncoder::new();
        // Very spiky signal -> rough texture.
        let amps: Vec<f32> = (0..64)
            .map(|i| if i % 2 == 0 { 2.0 } else { 0.01 })
            .collect();
        let phases = linear_phases(64);
        let impressions = encoder.extract(&amps, &phases);
        let texture = &impressions[0];
        assert_eq!(texture.0, SensoryModality::Texture);
        assert!(
            texture.1.contains("rough") || texture.1.contains("coarse"),
            "expected rough texture, got: {}",
            texture.1
        );
    }

    #[test]
    fn sensory_luminosity_bright_for_coherent() {
        let encoder = CsiSensoryEncoder::new();
        let amps = static_signal(32, 1.0);
        let phases = vec![0.5f32; 32]; // identical phases = high coherence
        let impressions = encoder.extract(&amps, &phases);
        let lum = impressions.iter().find(|(m, _)| *m == SensoryModality::Luminosity);
        assert!(lum.is_some());
        let desc = &lum.unwrap().1;
        assert!(
            desc.contains("bright"),
            "expected bright for coherent phases, got: {desc}"
        );
    }

    #[test]
    fn sensory_temperature_cold_for_low_power() {
        let encoder = CsiSensoryEncoder::new();
        let amps = static_signal(32, 0.01);
        let phases = linear_phases(32);
        let impressions = encoder.extract(&amps, &phases);
        let temp = impressions.iter().find(|(m, _)| *m == SensoryModality::Temperature);
        assert!(temp.is_some());
        assert!(
            temp.unwrap().1.contains("cold"),
            "expected cold for low power"
        );
    }

    #[test]
    fn sensory_empty_amplitudes() {
        let encoder = CsiSensoryEncoder::new();
        let impressions = encoder.extract(&[], &[]);
        // Should still return impressions (with default/zero-ish values).
        assert_eq!(impressions.len(), 6);
    }

    // ---- Stage III: Mesh Topology ----

    #[test]
    fn mesh_topology_two_aps() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();

        let nodes = vec![
            ApNode {
                id: "ap-1".into(),
                position: (0.0, 0.0),
                coverage_radius: 10.0,
            },
            ApNode {
                id: "ap-2".into(),
                position: (5.0, 0.0),
                coverage_radius: 8.0,
            },
        ];
        let links = vec![ApLink {
            from: "ap-1".into(),
            to: "ap-2".into(),
            signal_strength: 0.7,
        }];

        let embedding = pipeline.add_mesh_topology("s1", &nodes, &links).unwrap();
        assert_eq!(embedding.len(), 32);
    }

    #[test]
    fn mesh_topology_empty_nodes_errors() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();
        let result = pipeline.add_mesh_topology("s1", &[], &[]);
        assert!(result.is_err());
    }

    #[test]
    fn mesh_topology_single_ap_no_links() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();

        let nodes = vec![ApNode {
            id: "ap-solo".into(),
            position: (1.0, 2.0),
            coverage_radius: 5.0,
        }];

        let embedding = pipeline.add_mesh_topology("s1", &nodes, &[]).unwrap();
        assert_eq!(embedding.len(), 32);
    }

    // ---- Stage IV: Coherence -> AOL ----

    #[test]
    fn coherence_accept_clean_signal() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();

        let emb = pipeline
            .add_coherence_state("s1", CoherenceGateState::Accept, 0.9)
            .unwrap();
        assert_eq!(emb.len(), 32);
    }

    #[test]
    fn coherence_reject_noisy() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();

        let emb = pipeline
            .add_coherence_state("s1", CoherenceGateState::Reject, 0.1)
            .unwrap();
        assert_eq!(emb.len(), 32);
    }

    #[test]
    fn coherence_predict_only() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();

        let emb = pipeline
            .add_coherence_state("s1", CoherenceGateState::PredictOnly, 0.5)
            .unwrap();
        assert_eq!(emb.len(), 32);
    }

    #[test]
    fn coherence_recalibrate() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();

        let emb = pipeline
            .add_coherence_state("s1", CoherenceGateState::Recalibrate, 0.6)
            .unwrap();
        assert_eq!(emb.len(), 32);
    }

    // ---- Full Pipeline Flow ----

    #[test]
    fn full_pipeline_create_process_interrogate_partition() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();

        // Process two CSI frames.
        let amps1 = periodic_signal(32, 2.0, 0.8);
        let phases1 = linear_phases(32);
        let result1 = pipeline.process_csi_frame("s1", &amps1, &phases1).unwrap();
        assert_eq!(result1.gestalt_embedding.len(), 32);
        assert_eq!(result1.sensory_embedding.len(), 32);

        let amps2 = static_signal(32, 0.5);
        let phases2 = linear_phases(32);
        let result2 = pipeline.process_csi_frame("s1", &amps2, &phases2).unwrap();
        assert_ne!(result1.gestalt, result2.gestalt);

        // Add mesh topology.
        let nodes = vec![
            ApNode { id: "ap-1".into(), position: (0.0, 0.0), coverage_radius: 10.0 },
            ApNode { id: "ap-2".into(), position: (5.0, 3.0), coverage_radius: 8.0 },
        ];
        let links = vec![ApLink {
            from: "ap-1".into(),
            to: "ap-2".into(),
            signal_strength: 0.8,
        }];
        pipeline.add_mesh_topology("s1", &nodes, &links).unwrap();

        // Add coherence state.
        pipeline
            .add_coherence_state("s1", CoherenceGateState::Accept, 0.85)
            .unwrap();

        assert_eq!(pipeline.session_entry_count("s1"), 6);

        // Interrogate.
        let query = vec![0.5f32; 32];
        let stage_v = pipeline.interrogate("s1", &query).unwrap();
        // Should have probes for stages 1-4 that have entries.
        assert!(!stage_v.probes.is_empty());

        // Partition.
        let stage_vi = pipeline.partition_persons("s1").unwrap();
        assert!(!stage_vi.partitions.is_empty());
    }

    #[test]
    fn pipeline_session_not_found() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        let result = pipeline.process_csi_frame("nonexistent", &[1.0], &[0.0]);
        assert!(result.is_err());
    }

    #[test]
    fn pipeline_empty_csi_frame() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();
        let result = pipeline.process_csi_frame("s1", &[], &[]);
        assert!(result.is_err());
    }

    #[test]
    fn pipeline_empty_query_interrogation() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();
        let result = pipeline.interrogate("s1", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn pipeline_interrogate_empty_session() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();
        let result = pipeline.interrogate("s1", &[1.0; 32]);
        assert!(result.is_err());
    }

    // ---- Cross-Session Convergence ----

    #[test]
    fn cross_session_convergence_same_room() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("viewer-a", "room-1").unwrap();
        pipeline.create_session("viewer-b", "room-1").unwrap();

        // Both viewers see the same periodic signal.
        let amps = periodic_signal(32, 2.0, 0.8);
        let phases = linear_phases(32);

        pipeline
            .process_csi_frame("viewer-a", &amps, &phases)
            .unwrap();
        pipeline
            .process_csi_frame("viewer-b", &amps, &phases)
            .unwrap();

        let convergence = pipeline
            .find_cross_room_convergence("room-1", 0.5)
            .unwrap();
        assert!(
            !convergence.scores.is_empty(),
            "identical frames should converge"
        );
        assert!(convergence.scores[0] > 0.5);
    }

    #[test]
    fn cross_session_convergence_different_signals() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("a", "room-2").unwrap();
        pipeline.create_session("b", "room-2").unwrap();

        // Very different signals.
        let amps_a = periodic_signal(32, 8.0, 2.0);
        let amps_b = static_signal(32, 0.01);
        let phases = linear_phases(32);

        pipeline
            .process_csi_frame("a", &amps_a, &phases)
            .unwrap();
        pipeline
            .process_csi_frame("b", &amps_b, &phases)
            .unwrap();

        let convergence = pipeline.find_cross_room_convergence("room-2", 0.95);
        // May or may not converge at high threshold; the key is no panic.
        assert!(convergence.is_ok());
    }

    #[test]
    fn cross_session_needs_two_sessions() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("solo", "room-3").unwrap();
        pipeline
            .process_csi_frame("solo", &[1.0; 32], &[0.0; 32])
            .unwrap();

        let result = pipeline.find_cross_room_convergence("room-3", 0.5);
        assert!(result.is_err(), "convergence requires at least 2 sessions");
    }

    // ---- Session management ----

    #[test]
    fn session_create_and_remove() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();
        assert_eq!(pipeline.session_count(), 1);
        assert!(pipeline.remove_session("s1"));
        assert_eq!(pipeline.session_count(), 0);
        assert!(!pipeline.remove_session("s1"));
    }

    #[test]
    fn session_duplicate_errors() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();
        let result = pipeline.create_session("s1", "room-a");
        assert!(result.is_err());
    }

    // ---- Edge cases ----

    #[test]
    fn zero_amplitude_frame() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();

        let amps = vec![0.0f32; 32];
        let phases = vec![0.0f32; 32];
        let result = pipeline.process_csi_frame("s1", &amps, &phases);
        // Should succeed (all-zero is a valid edge case).
        assert!(result.is_ok());
    }

    #[test]
    fn single_subcarrier_frame() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();

        let result = pipeline.process_csi_frame("s1", &[1.0], &[0.5]);
        assert!(result.is_ok());
    }

    #[test]
    fn large_frame_256_subcarriers() {
        let mut pipeline = WifiCrvPipeline::new(test_config());
        pipeline.create_session("s1", "room-a").unwrap();

        let amps = periodic_signal(256, 10.0, 1.0);
        let phases = linear_phases(256);
        let result = pipeline.process_csi_frame("s1", &amps, &phases);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().gestalt_embedding.len(), 32);
    }

    // ---- CsiGestaltClassifier helpers ----

    #[test]
    fn compute_variance_static() {
        let v = CsiGestaltClassifier::compute_variance(&[1.0; 32]);
        assert!(v < 1e-6, "static signal should have near-zero variance");
    }

    #[test]
    fn compute_periodicity_constant() {
        let p = CsiGestaltClassifier::compute_periodicity(&[1.0; 32]);
        // Constant signal: autocorrelation peak ratio depends on zero-variance handling.
        assert!(p >= 0.0 && p <= 1.0);
    }

    #[test]
    fn compute_null_fraction_all_zeros() {
        let f = CsiGestaltClassifier::compute_null_fraction(&[0.0; 32]);
        assert!((f - 1.0).abs() < 1e-6, "all zeros should give null fraction 1.0");
    }

    #[test]
    fn compute_null_fraction_none_zero() {
        let f = CsiGestaltClassifier::compute_null_fraction(&[1.0; 32]);
        assert!(f < 1e-6, "no nulls should give null fraction 0.0");
    }

    // ---- CsiSensoryEncoder helpers ----

    #[test]
    fn spectral_centroid_uniform() {
        let encoder = CsiSensoryEncoder::new();
        let amps = vec![1.0f32; 32];
        let centroid = encoder.spectral_centroid(&amps);
        // Uniform -> centroid at midpoint.
        assert!(
            (centroid - 0.484).abs() < 0.1,
            "uniform spectral centroid should be near 0.5, got {centroid}"
        );
    }

    #[test]
    fn signal_energy_known() {
        let encoder = CsiSensoryEncoder::new();
        let energy = encoder.signal_energy(&[2.0, 2.0, 2.0, 2.0]);
        assert!((energy - 4.0).abs() < 1e-6, "energy of [2,2,2,2] should be 4.0");
    }

    #[test]
    fn phase_coherence_identical() {
        let encoder = CsiSensoryEncoder::new();
        let c = encoder.phase_coherence(&[1.0; 100]);
        assert!(c > 0.99, "identical phases should give coherence ~1.0, got {c}");
    }

    #[test]
    fn phase_coherence_empty() {
        let encoder = CsiSensoryEncoder::new();
        let c = encoder.phase_coherence(&[]);
        assert_eq!(c, 0.0);
    }

    #[test]
    fn subcarrier_spread_all_active() {
        let encoder = CsiSensoryEncoder::new();
        let spread = encoder.subcarrier_spread(&[1.0; 32]);
        assert!((spread - 1.0).abs() < 1e-6, "all active should give spread 1.0");
    }

    #[test]
    fn subcarrier_spread_empty() {
        let encoder = CsiSensoryEncoder::new();
        let spread = encoder.subcarrier_spread(&[]);
        assert_eq!(spread, 0.0);
    }
}
