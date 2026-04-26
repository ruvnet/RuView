//! Stage 8: Pipeline orchestrator (Domain Service).
//!
//! `WindowsWifiPipeline` connects all pipeline stages (1-7) into a
//! single processing step that transforms a `MultiApFrame` into an
//! `EnhancedSensingResult`.
//!
//! This is the Domain Service described in ADR-022 section 3.2.

use crate::domain::frame::MultiApFrame;
use crate::domain::result::{
    BreathingEstimate as DomainBreathingEstimate, EnhancedSensingResult,
    MotionEstimate as DomainMotionEstimate, MotionLevel, PostureClass, SignalQuality,
    Verdict as DomainVerdict,
};

use super::attention_weighter::AttentionWeighter;
use super::breathing_extractor::CoarseBreathingExtractor;
use super::correlator::BssidCorrelator;
use super::fingerprint_matcher::FingerprintMatcher;
use super::motion_estimator::MultiApMotionEstimator;
use super::predictive_gate::PredictiveGate;
use super::quality_gate::{QualityGate, Verdict};

/// Configuration for the Windows `WiFi` sensing pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Maximum number of BSSID slots.
    pub max_bssids: usize,
    /// Residual gating threshold (stage 1).
    pub gate_threshold: f32,
    /// Correlation window size in frames (stage 3).
    pub correlation_window: usize,
    /// Correlation threshold for co-varying classification (stage 3).
    pub correlation_threshold: f32,
    /// Minimum BSSIDs for a valid frame.
    pub min_bssids: usize,
    /// Enable breathing extraction (stage 5).
    pub enable_breathing: bool,
    /// Enable fingerprint matching (stage 7).
    pub enable_fingerprint: bool,
    /// Sample rate in Hz.
    pub sample_rate: f32,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            max_bssids: 32,
            gate_threshold: 0.05,
            correlation_window: 30,
            correlation_threshold: 0.7,
            min_bssids: 3,
            enable_breathing: true,
            enable_fingerprint: true,
            sample_rate: 2.0,
        }
    }
}

/// The complete Windows `WiFi` sensing pipeline (Domain Service).
///
/// Connects stages 1-7 into a single `process()` call that transforms
/// a `MultiApFrame` into an `EnhancedSensingResult`.
///
/// Stages:
/// 1. Predictive gating (EMA residual filter)
/// 2. Attention weighting (softmax dot-product)
/// 3. Spatial correlation (Pearson + clustering)
/// 4. Motion estimation (weighted variance + EMA)
/// 5. Breathing extraction (bandpass + zero-crossing)
/// 6. Quality gate (three-filter: structural / shift / evidence)
/// 7. Fingerprint matching (cosine similarity templates)
pub struct WindowsWifiPipeline {
    gate: PredictiveGate,
    attention: AttentionWeighter,
    correlator: BssidCorrelator,
    motion: MultiApMotionEstimator,
    breathing: CoarseBreathingExtractor,
    quality: QualityGate,
    fingerprint: FingerprintMatcher,
    config: PipelineConfig,
    /// Whether fingerprint defaults have been initialised.
    fingerprints_initialised: bool,
    /// Frame counter.
    frame_count: u64,
}

impl WindowsWifiPipeline {
    /// Create a new pipeline with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(PipelineConfig::default())
    }

    /// Create with default configuration (alias for `new`).
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new()
    }

    /// Create a new pipeline with custom configuration.
    #[must_use]
    pub fn with_config(config: PipelineConfig) -> Self {
        Self {
            gate: PredictiveGate::new(config.max_bssids, config.gate_threshold),
            attention: AttentionWeighter::new(1),
            correlator: BssidCorrelator::new(
                config.max_bssids,
                config.correlation_window,
                config.correlation_threshold,
            ),
            motion: MultiApMotionEstimator::new(),
            breathing: CoarseBreathingExtractor::new(
                config.max_bssids,
                config.sample_rate,
                0.1,
                0.5,
            ),
            quality: QualityGate::new(),
            fingerprint: FingerprintMatcher::new(config.max_bssids, 0.5),
            fingerprints_initialised: false,
            frame_count: 0,
            config,
        }
    }

    /// Process a single multi-BSSID frame through all pipeline stages.
    ///
    /// Returns an `EnhancedSensingResult` with motion, breathing,
    /// posture, and quality information.
    pub fn process(&mut self, frame: &MultiApFrame) -> EnhancedSensingResult {
        self.frame_count += 1;

        let n = frame.bssid_count;

        // Convert f64 amplitudes to f32 for pipeline stages.
        #[allow(clippy::cast_possible_truncation)]
        let amps_f32: Vec<f32> = frame.amplitudes.iter().map(|&a| a as f32).collect();

        // Initialise fingerprint defaults on first frame with enough BSSIDs.
        if !self.fingerprints_initialised
            && self.config.enable_fingerprint
            && amps_f32.len() == self.config.max_bssids
        {
            self.fingerprint.generate_defaults(&amps_f32);
            self.fingerprints_initialised = true;
        }

        // Check minimum BSSID count.
        if n < self.config.min_bssids {
            return Self::make_empty_result(frame, n);
        }

        // -- Stage 1: Predictive gating --
        let Some(residuals) = self.gate.gate(&amps_f32) else {
            // Static environment, no body present.
            return Self::make_empty_result(frame, n);
        };

        // -- Stage 2: Attention weighting --
        #[allow(clippy::cast_precision_loss)]
        let mean_residual =
            residuals.iter().map(|r| r.abs()).sum::<f32>() / residuals.len().max(1) as f32;
        let query = vec![mean_residual];
        let keys: Vec<Vec<f32>> = residuals.iter().map(|&r| vec![r]).collect();
        let values: Vec<Vec<f32>> = amps_f32.iter().map(|&a| vec![a]).collect();
        let (_weighted, weights) = self.attention.weight(&query, &keys, &values);

        // -- Stage 3: Spatial correlation --
        let corr = self.correlator.update(&amps_f32);

        // -- Stage 4: Motion estimation --
        let motion = self.motion.estimate(&residuals, &weights, &corr.diversity);

        // -- Stage 5: Breathing extraction (only when stationary) --
        let breathing = if self.config.enable_breathing && motion.level == MotionLevel::Minimal {
            self.breathing.extract(&residuals, &weights)
        } else {
            None
        };

        // -- Stage 6: Quality gate --
        let quality_result = self.quality.evaluate(
            n,
            frame.mean_rssi(),
            f64::from(corr.mean_correlation()),
            motion.score,
        );

        // -- Stage 7: Fingerprint matching --
        let posture = if self.config.enable_fingerprint {
            self.fingerprint.classify(&amps_f32).map(|(p, _sim)| p)
        } else {
            None
        };

        // Count body-sensitive BSSIDs (attention weight above 1.5x average).
        #[allow(clippy::cast_precision_loss)]
        let avg_weight = 1.0 / n.max(1) as f32;
        let sensitive_count = weights.iter().filter(|&&w| w > avg_weight * 1.5).count();

        // Map internal quality gate verdict to domain Verdict.
        let domain_verdict = match &quality_result.verdict {
            Verdict::Permit => DomainVerdict::Permit,
            Verdict::Defer => DomainVerdict::Warn,
            Verdict::Deny(_) => DomainVerdict::Deny,
        };

        // Build the domain BreathingEstimate if we have one.
        let domain_breathing = breathing.map(|b| DomainBreathingEstimate {
            rate_bpm: f64::from(b.bpm),
            confidence: f64::from(b.confidence),
            bssid_count: sensitive_count,
        });

        EnhancedSensingResult {
            motion: DomainMotionEstimate {
                score: f64::from(motion.score),
                level: motion.level,
                contributing_bssids: motion.n_contributing,
            },
            breathing: domain_breathing,
            posture,
            signal_quality: SignalQuality {
                score: quality_result.quality,
                bssid_count: n,
                spectral_gap: f64::from(corr.mean_correlation()),
                mean_rssi_dbm: frame.mean_rssi(),
            },
            bssid_count: n,
            verdict: domain_verdict,
        }
    }

    /// Build an empty/gated result for frames that don't pass initial checks.
    fn make_empty_result(frame: &MultiApFrame, n: usize) -> EnhancedSensingResult {
        EnhancedSensingResult {
            motion: DomainMotionEstimate {
                score: 0.0,
                level: MotionLevel::None,
                contributing_bssids: 0,
            },
            breathing: None,
            posture: None,
            signal_quality: SignalQuality {
                score: 0.0,
                bssid_count: n,
                spectral_gap: 0.0,
                mean_rssi_dbm: frame.mean_rssi(),
            },
            bssid_count: n,
            verdict: DomainVerdict::Deny,
        }
    }

    /// Store a reference fingerprint pattern.
    ///
    /// # Errors
    ///
    /// Returns an error if the pattern dimension does not match `max_bssids`.
    pub fn store_fingerprint(
        &mut self,
        pattern: Vec<f32>,
        label: PostureClass,
    ) -> Result<(), String> {
        self.fingerprint.store_pattern(pattern, label)
    }

    /// Reset all pipeline state.
    pub fn reset(&mut self) {
        self.gate = PredictiveGate::new(self.config.max_bssids, self.config.gate_threshold);
        self.correlator = BssidCorrelator::new(
            self.config.max_bssids,
            self.config.correlation_window,
            self.config.correlation_threshold,
        );
        self.motion.reset();
        self.breathing.reset();
        self.quality.reset();
        self.fingerprint.clear();
        self.fingerprints_initialised = false;
        self.frame_count = 0;
    }

    /// Number of frames processed.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Current pipeline configuration.
    #[must_use]
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }
}

impl Default for WindowsWifiPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::time::Instant;

    fn make_frame(bssid_count: usize, rssi_values: &[f64]) -> MultiApFrame {
        let amplitudes: Vec<f64> = rssi_values
            .iter()
            .map(|&r| 10.0_f64.powf((r + 100.0) / 20.0))
            .collect();
        MultiApFrame {
            bssid_count,
            rssi_dbm: rssi_values.to_vec(),
            amplitudes,
            phases: vec![0.0; bssid_count],
            per_bssid_variance: vec![0.1; bssid_count],
            histories: vec![VecDeque::new(); bssid_count],
            sample_rate_hz: 2.0,
            timestamp: Instant::now(),
        }
    }

    #[test]
    fn pipeline_creates_ok() {
        let pipeline = WindowsWifiPipeline::with_defaults();
        assert_eq!(pipeline.frame_count(), 0);
        assert_eq!(pipeline.config().max_bssids, 32);
    }

    #[test]
    fn too_few_bssids_returns_deny() {
        let mut pipeline = WindowsWifiPipeline::new();
        let frame = make_frame(2, &[-60.0, -70.0]);
        let result = pipeline.process(&frame);
        assert_eq!(result.verdict, DomainVerdict::Deny);
    }

    #[test]
    fn first_frame_increments_count() {
        let mut pipeline = WindowsWifiPipeline::with_config(PipelineConfig {
            min_bssids: 1,
            max_bssids: 4,
            ..Default::default()
        });
        let frame = make_frame(4, &[-60.0, -65.0, -70.0, -75.0]);
        let _result = pipeline.process(&frame);
        assert_eq!(pipeline.frame_count(), 1);
    }

    #[test]
    fn static_signal_returns_deny_after_learning() {
        let mut pipeline = WindowsWifiPipeline::with_config(PipelineConfig {
            min_bssids: 1,
            max_bssids: 4,
            ..Default::default()
        });
        let frame = make_frame(4, &[-60.0, -65.0, -70.0, -75.0]);

        // Train on static signal.
        pipeline.process(&frame);
        pipeline.process(&frame);
        pipeline.process(&frame);

        // After learning, static signal should be gated (Deny verdict).
        let result = pipeline.process(&frame);
        assert_eq!(
            result.verdict,
            DomainVerdict::Deny,
            "static signal should be gated"
        );
    }

    #[test]
    fn changing_signal_increments_count() {
        let mut pipeline = WindowsWifiPipeline::with_config(PipelineConfig {
            min_bssids: 1,
            max_bssids: 4,
            ..Default::default()
        });
        let baseline = make_frame(4, &[-60.0, -65.0, -70.0, -75.0]);

        // Learn baseline.
        for _ in 0..5 {
            pipeline.process(&baseline);
        }

        // Significant change should be noticed.
        let changed = make_frame(4, &[-60.0, -65.0, -70.0, -30.0]);
        pipeline.process(&changed);
        assert!(pipeline.frame_count() > 5);
    }

    #[test]
    fn reset_clears_state() {
        let mut pipeline = WindowsWifiPipeline::new();
        let frame = make_frame(4, &[-60.0, -65.0, -70.0, -75.0]);
        pipeline.process(&frame);
        assert_eq!(pipeline.frame_count(), 1);
        pipeline.reset();
        assert_eq!(pipeline.frame_count(), 0);
    }

    #[test]
    fn default_creates_pipeline() {
        let _pipeline = WindowsWifiPipeline::default();
    }

    #[test]
    fn pipeline_throughput_benchmark() {
        let mut pipeline = WindowsWifiPipeline::with_config(PipelineConfig {
            min_bssids: 1,
            max_bssids: 4,
            ..Default::default()
        });
        let frame = make_frame(4, &[-60.0, -65.0, -70.0, -75.0]);

        let start = Instant::now();
        let n_frames = 10_000;
        for _ in 0..n_frames {
            pipeline.process(&frame);
        }
        let elapsed = start.elapsed();
        #[allow(clippy::cast_precision_loss)]
        let fps = n_frames as f64 / elapsed.as_secs_f64();
        println!("Pipeline throughput: {fps:.0} frames/sec ({elapsed:?} for {n_frames} frames)");
        assert!(fps > 100.0, "Pipeline should process >100 frames/sec, got {fps:.0}");
    }
}
