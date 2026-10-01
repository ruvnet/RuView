//! Explicit real-CSI preprocessing pipeline.
//!
//! Raw public-dataset CSI becomes model input through named, ordered stages —
//! no hidden transformations inside model code:
//!
//! ```text
//! raw amplitude/phase [T, n_tx, n_rx, n_sc]
//!   -> shape validation
//!   -> NaN / infinite rejection (fail-closed)
//!   -> phase sanitization + unwrapping (per lane, subcarrier axis)
//!   -> amplitude denoising (Hampel, per time lane)
//!   -> per-antenna-pair standardization
//!   -> subcarrier conversion (documented linear interpolation)
//!   -> model input tensor + [`ProcessedSample`]
//! ```
//!
//! Every stage reuses existing RuView code: phase work comes from
//! [`wifi_densepose_signal::PhaseSanitizer`], amplitude denoising from
//! [`wifi_densepose_signal::hampel_filter`], and subcarrier conversion from
//! [`crate::subcarrier::interpolate_subcarriers`]. What is new here is the
//! explicit staging, the fail-closed quality handling, and the per-sample
//! provenance ([`ProcessedSampleMeta`]).
//!
//! # Subcarrier conversion math
//!
//! Conversion is piecewise-linear interpolation over the normalized
//! subcarrier position `x = k / (n_sc - 1)`, `k = 0..n_sc-1`. Output lane
//! `j` samples the input at `x_j = j / (target - 1)`:
//!
//! ```text
//! out[j] = in[i] + (in[i+1] - in[i]) * (x_j - x_i) / (x_{i+1} - x_i)
//! ```
//!
//! where `i` is the largest input index with `x_i <= x_j`. Converted data is
//! recorded as converted in [`ProcessedSampleMeta`]: it must never be
//! described as a native capture at the target width.
//!
//! # Determinism
//!
//! The pipeline uses no RNG. Identical input arrays and an identical
//! [`RealCsiConfig`] always produce bit-identical outputs.
//!
//! [`ProcessedSampleMeta`]: crate::public_dataset::ProcessedSampleMeta

use ndarray::{Array2, Array4};
use thiserror::Error;

use wifi_densepose_signal::{hampel_filter, HampelConfig, PhaseSanitizer, PhaseSanitizerConfig};

use crate::public_dataset::{ProcessedSampleMeta, MANIFEST_PREPROCESSING_VERSION};
use crate::subcarrier::interpolate_subcarriers;

/// Preprocessing pipeline version stamped into every sample and manifest.
pub const PREPROCESSING_VERSION: &str = MANIFEST_PREPROCESSING_VERSION;

/// Default model input width: 56 subcarriers (Atheros 20 MHz layout).
pub const DEFAULT_TARGET_SUBCARRIERS: usize = 56;

/// Small constant added to the per-pair standard deviation so a perfectly
/// flat window cannot divide by zero.
const NORM_EPS: f32 = 1e-6;

/// Errors from the real-CSI preprocessing pipeline.
///
/// Fail-closed by design: corrupted or degenerate windows are reported, never
/// silently repaired into plausible-looking model inputs.
#[derive(Debug, Error)]
pub enum PreprocessError {
    /// Amplitude/phase shapes disagree or a dimension is zero.
    #[error("shape mismatch: amplitude {amp:?}, phase {phase:?}: {message}")]
    ShapeMismatch {
        /// Amplitude shape.
        amp: Vec<usize>,
        /// Phase shape.
        phase: Vec<usize>,
        /// What was wrong.
        message: String,
    },

    /// Non-finite (NaN / infinite) values present in the input window.
    #[error(
        "non-finite input values: {count} affected elements (fail-closed, no repair attempted)"
    )]
    NonFiniteInput {
        /// Number of non-finite elements found.
        count: usize,
    },

    /// Fewer frames than the configured minimum.
    #[error("insufficient temporal context: {found} frames, need at least {min}")]
    InsufficientFrames {
        /// Frames present.
        found: usize,
        /// Minimum required.
        min: usize,
    },

    /// Hampel outlier fraction above the configured ceiling: the window is
    /// too corrupted to trust.
    #[error("degraded signal: outlier fraction {fraction:.3} exceeds maximum {max:.3}")]
    DegradedSignal {
        /// Measured outlier fraction.
        fraction: f64,
        /// Configured ceiling.
        max: f64,
    },

    /// Invalid pipeline configuration.
    #[error("invalid preprocessing configuration: {0}")]
    Config(String),

    /// Phase sanitization failed inside `wifi-densepose-signal`.
    #[error("phase sanitization failed: {0}")]
    PhaseSanitization(String),

    /// Amplitude denoising failed inside `wifi-densepose-signal`.
    #[error("amplitude denoising failed: {0}")]
    Denoising(String),
}

/// Configuration for [`RealCsiPipeline`].
#[derive(Debug, Clone)]
pub struct RealCsiConfig {
    /// Model input subcarrier width (default 56).
    pub target_subcarriers: usize,
    /// Minimum window frames (default 2: statistics need context).
    pub min_frames: usize,
    /// Hampel half-window for amplitude denoising (default 3).
    pub hampel_half_window: usize,
    /// Hampel sigma threshold (default 3.0).
    pub hampel_threshold: f64,
    /// Maximum tolerated Hampel outlier fraction before the window is
    /// rejected as degraded (default 0.5: the Hampel filter's breakdown
    /// point with median/MAD estimation).
    pub max_outlier_fraction: f64,
    /// Run phase sanitization (default true).
    pub enable_phase_sanitization: bool,
    /// Run amplitude denoising (default true).
    pub enable_denoising: bool,
}

impl Default for RealCsiConfig {
    fn default() -> Self {
        Self {
            target_subcarriers: DEFAULT_TARGET_SUBCARRIERS,
            min_frames: 2,
            hampel_half_window: 3,
            hampel_threshold: 3.0,
            max_outlier_fraction: 0.5,
            enable_phase_sanitization: true,
            enable_denoising: true,
        }
    }
}

impl RealCsiConfig {
    /// Check the configuration before any data flows through it.
    pub fn validate(&self) -> Result<(), PreprocessError> {
        if self.target_subcarriers == 0 {
            return Err(PreprocessError::Config(
                "target_subcarriers must be > 0".into(),
            ));
        }
        if self.min_frames == 0 {
            return Err(PreprocessError::Config("min_frames must be > 0".into()));
        }
        if self.hampel_half_window == 0 {
            return Err(PreprocessError::Config(
                "hampel_half_window must be > 0".into(),
            ));
        }
        if !(0.0 < self.max_outlier_fraction && self.max_outlier_fraction <= 1.0) {
            return Err(PreprocessError::Config(format!(
                "max_outlier_fraction must be in (0, 1], got {}",
                self.max_outlier_fraction
            )));
        }
        Ok(())
    }
}

/// Quality accounting for one processed window.
#[derive(Debug, Clone)]
pub struct QualityReport {
    /// Lanes processed (`T * n_tx * n_rx`).
    pub lanes: usize,
    /// Amplitude elements replaced by the Hampel filter.
    pub hampel_outliers_replaced: usize,
    /// `hampel_outliers_replaced / total amplitude elements`.
    pub outlier_fraction: f64,
    /// Phase lanes passed through the sanitizer.
    pub phase_lanes_sanitized: usize,
    /// Conversion description, e.g. `"linear-interpolation 114->56"` or
    /// `"native-passthrough 56->56"`.
    pub conversion_method: String,
    /// Native subcarrier count of the raw window.
    pub original_subcarriers: usize,
    /// Subcarrier count after conversion.
    pub processed_subcarriers: usize,
}

/// One fully processed window: model-ready tensors plus provenance.
#[derive(Debug, Clone)]
pub struct ProcessedSample {
    /// Denoised, normalized, converted amplitude `[T, n_tx, n_rx, target_sc]`.
    pub amplitude: Array4<f32>,
    /// Sanitized, converted phase (radians) `[T, n_tx, n_rx, target_sc]`.
    pub phase: Array4<f32>,
    /// Provenance for this window.
    pub meta: ProcessedSampleMeta,
    /// Per-window quality accounting.
    pub quality: QualityReport,
}

/// Source description used to build [`ProcessedSampleMeta`] from a loader
/// sample (see [`crate::dataset::CsiSample`]).
#[derive(Debug, Clone)]
pub struct RealCsiSourceInfo {
    /// Dataset key, e.g. `"mmfi"`.
    pub source_dataset: String,
    /// Antenna layout string, e.g. `"1TXx3RX"`.
    pub antenna_configuration: String,
    /// Capture sampling rate in Hz, when known.
    pub sampling_rate_hz: Option<f32>,
    /// Environment / room id, when the layout provides one.
    pub environment_id: Option<u32>,
}

/// Explicit staged real-CSI preprocessing pipeline.
///
/// Construct once per configuration; [`Self::process`] is deterministic and
/// free of interior mutability beyond the phase sanitizer's statistics.
pub struct RealCsiPipeline {
    config: RealCsiConfig,
    sanitizer: PhaseSanitizer,
    hampel: HampelConfig,
}

impl RealCsiPipeline {
    /// Build the pipeline, validating the configuration up front.
    ///
    /// # Errors
    ///
    /// [`PreprocessError::Config`] for invalid settings, or
    /// [`PreprocessError::PhaseSanitization`] when the sanitizer's own
    /// defaults fail its validation.
    pub fn new(config: RealCsiConfig) -> Result<Self, PreprocessError> {
        config.validate()?;
        let sanitizer = PhaseSanitizer::new(PhaseSanitizerConfig::builder().build())
            .map_err(|e| PreprocessError::PhaseSanitization(e.to_string()))?;
        let hampel = HampelConfig {
            half_window: config.hampel_half_window,
            threshold: config.hampel_threshold,
        };
        Ok(Self {
            config,
            sanitizer,
            hampel,
        })
    }

    /// Pipeline configuration.
    pub fn config(&self) -> &RealCsiConfig {
        &self.config
    }

    /// Run the full staged pipeline over one amplitude/phase window pair.
    ///
    /// Both arrays must share shape `[T, n_tx, n_rx, n_sc]`. The returned
    /// tensors have shape `[T, n_tx, n_rx, target_sc]`; `meta` is filled in
    /// with the conversion record and [`PREPROCESSING_VERSION`].
    pub fn process(
        &mut self,
        amplitude: &Array4<f32>,
        phase: &Array4<f32>,
        mut meta: ProcessedSampleMeta,
    ) -> Result<ProcessedSample, PreprocessError> {
        self.validate_shapes(amplitude, phase)?;
        let shape = amplitude.shape().to_vec();
        let (n_t, n_tx, n_rx, n_sc) = (shape[0], shape[1], shape[2], shape[3]);
        if n_t < self.config.min_frames {
            return Err(PreprocessError::InsufficientFrames {
                found: n_t,
                min: self.config.min_frames,
            });
        }
        self.reject_non_finite(amplitude, phase)?;

        let clean_phase = self.sanitize_phase_lanes(phase, n_t, n_tx, n_rx, n_sc)?;
        let (clean_amp, outliers) = self.denoise_amplitude(amplitude, n_t, n_tx, n_rx, n_sc)?;
        let norm_amp = standardize_per_pair(&clean_amp);

        let total_elements = (n_t * n_tx * n_rx * n_sc) as f64;
        let outlier_fraction = outliers as f64 / total_elements.max(1.0);
        if outlier_fraction > self.config.max_outlier_fraction {
            return Err(PreprocessError::DegradedSignal {
                fraction: outlier_fraction,
                max: self.config.max_outlier_fraction,
            });
        }

        let target = self.config.target_subcarriers;
        let conversion_method = if n_sc == target {
            "native-passthrough".to_string()
        } else {
            "linear-interpolation".to_string()
        };
        let out_amp = interpolate_subcarriers(&norm_amp, target);
        let out_phase = interpolate_subcarriers(&clean_phase, target);

        meta.original_subcarrier_count = n_sc;
        meta.processed_subcarrier_count = target;
        meta.preprocessing_version = PREPROCESSING_VERSION.to_string();

        Ok(ProcessedSample {
            amplitude: out_amp,
            phase: out_phase,
            meta,
            quality: QualityReport {
                lanes: n_t * n_tx * n_rx,
                hampel_outliers_replaced: outliers,
                outlier_fraction,
                phase_lanes_sanitized: if self.config.enable_phase_sanitization {
                    n_t * n_tx * n_rx
                } else {
                    0
                },
                conversion_method: format!("{conversion_method} {n_sc}->{target}"),
                original_subcarriers: n_sc,
                processed_subcarriers: target,
            },
        })
    }

    /// Bridge a loader [`crate::dataset::CsiSample`] into the pipeline,
    /// filling [`ProcessedSampleMeta`] from the sample's own provenance.
    pub fn process_loader_sample(
        &mut self,
        sample: &crate::dataset::CsiSample,
        source: &RealCsiSourceInfo,
    ) -> Result<ProcessedSample, PreprocessError> {
        let meta = ProcessedSampleMeta {
            source_dataset: source.source_dataset.clone(),
            subject_id: sample.subject_id,
            environment_id: source.environment_id,
            recording_id: format!("S{:02}/A{:02}", sample.subject_id, sample.action_id),
            activity_id: sample.action_id,
            frame_index: sample.frame_id,
            original_subcarrier_count: sample.amplitude.shape()[3],
            processed_subcarrier_count: self.config.target_subcarriers,
            antenna_configuration: source.antenna_configuration.clone(),
            sampling_rate_hz: source.sampling_rate_hz,
            preprocessing_version: PREPROCESSING_VERSION.to_string(),
        };
        self.process(&sample.amplitude, &sample.phase, meta)
    }

    fn validate_shapes(
        &self,
        amplitude: &Array4<f32>,
        phase: &Array4<f32>,
    ) -> Result<(), PreprocessError> {
        let a = amplitude.shape().to_vec();
        let p = phase.shape().to_vec();
        if a.len() != 4 || p.len() != 4 || a != p {
            return Err(PreprocessError::ShapeMismatch {
                amp: a,
                phase: p,
                message: "amplitude and phase must share shape [T, n_tx, n_rx, n_sc]".into(),
            });
        }
        if a.contains(&0) {
            return Err(PreprocessError::ShapeMismatch {
                amp: a,
                phase: p,
                message: "window dimensions must all be > 0".into(),
            });
        }
        Ok(())
    }

    fn reject_non_finite(
        &self,
        amplitude: &Array4<f32>,
        phase: &Array4<f32>,
    ) -> Result<(), PreprocessError> {
        let count = amplitude.iter().filter(|v| !v.is_finite()).count()
            + phase.iter().filter(|v| !v.is_finite()).count();
        if count > 0 {
            return Err(PreprocessError::NonFiniteInput { count });
        }
        Ok(())
    }

    /// Unwrap + clean phase per `(t, tx, rx)` lane along the subcarrier axis.
    fn sanitize_phase_lanes(
        &mut self,
        phase: &Array4<f32>,
        n_t: usize,
        n_tx: usize,
        n_rx: usize,
        n_sc: usize,
    ) -> Result<Array4<f32>, PreprocessError> {
        if !self.config.enable_phase_sanitization {
            return Ok(phase.clone());
        }
        let lanes = n_t * n_tx * n_rx;
        let flat: Array2<f64> = phase
            .mapv(|v| v as f64)
            .into_shape_with_order((lanes, n_sc))
            .map_err(|e| PreprocessError::ShapeMismatch {
                amp: phase.shape().to_vec(),
                phase: phase.shape().to_vec(),
                message: format!("cannot view phase as (lanes, subcarriers): {e}"),
            })?;
        let clean = self
            .sanitizer
            .sanitize_phase(&flat)
            .map_err(|e| PreprocessError::PhaseSanitization(e.to_string()))?;
        clean
            .mapv(|v| v as f32)
            .into_shape_with_order((n_t, n_tx, n_rx, n_sc))
            .map_err(|e| PreprocessError::ShapeMismatch {
                amp: vec![lanes, n_sc],
                phase: vec![lanes, n_sc],
                message: format!("cannot restore sanitized phase window shape: {e}"),
            })
    }

    /// Hampel-denoise amplitude along time, independently per
    /// `(tx, rx, sc)` lane. Returns the cleaned window plus the number of
    /// replaced elements.
    fn denoise_amplitude(
        &self,
        amplitude: &Array4<f32>,
        n_t: usize,
        n_tx: usize,
        n_rx: usize,
        n_sc: usize,
    ) -> Result<(Array4<f32>, usize), PreprocessError> {
        if !self.config.enable_denoising {
            return Ok((amplitude.clone(), 0));
        }
        let mut out = amplitude.clone();
        let mut replaced = 0usize;
        let mut lane = vec![0.0f64; n_t];
        for tx in 0..n_tx {
            for rx in 0..n_rx {
                for sc in 0..n_sc {
                    for t in 0..n_t {
                        lane[t] = amplitude[[t, tx, rx, sc]] as f64;
                    }
                    let result = hampel_filter(&lane, &self.hampel)
                        .map_err(|e| PreprocessError::Denoising(e.to_string()))?;
                    replaced += result.outlier_indices.len();
                    for t in 0..n_t {
                        out[[t, tx, rx, sc]] = result.filtered[t] as f32;
                    }
                }
            }
        }
        Ok((out, replaced))
    }
}

/// Standardize each `(tx, rx)` antenna pair over its `(T, SC)` window:
///
/// ```text
/// z = (x - mean) / (std + 1e-6)      (population std, ddof = 0)
/// ```
///
/// Per-pair (rather than global) normalization keeps one hot antenna from
/// dominating the window while preserving the relative time-frequency
/// structure the model reads. Moments accumulate in `f64`: raw amplitudes
/// can carry large DC offsets where `f32` summation loses the mean.
fn standardize_per_pair(amplitude: &Array4<f32>) -> Array4<f32> {
    let shape = amplitude.shape().to_vec();
    let (n_t, n_tx, n_rx, n_sc) = (shape[0], shape[1], shape[2], shape[3]);
    let mut out = amplitude.clone();
    for tx in 0..n_tx {
        for rx in 0..n_rx {
            let n = (n_t * n_sc) as f64;
            let mut mean = 0.0f64;
            for t in 0..n_t {
                for sc in 0..n_sc {
                    mean += amplitude[[t, tx, rx, sc]] as f64;
                }
            }
            mean /= n;
            let mut var = 0.0f64;
            for t in 0..n_t {
                for sc in 0..n_sc {
                    let d = amplitude[[t, tx, rx, sc]] as f64 - mean;
                    var += d * d;
                }
            }
            let denom = (var / n).sqrt() + NORM_EPS as f64;
            for t in 0..n_t {
                for sc in 0..n_sc {
                    out[[t, tx, rx, sc]] =
                        ((amplitude[[t, tx, rx, sc]] as f64 - mean) / denom) as f32;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::public_dataset::ProcessedSampleMeta;

    fn test_meta() -> ProcessedSampleMeta {
        ProcessedSampleMeta {
            source_dataset: "mmfi-test".to_string(),
            subject_id: 1,
            environment_id: None,
            recording_id: "S01/A01".to_string(),
            activity_id: 1,
            frame_index: 0,
            original_subcarrier_count: 0,
            processed_subcarrier_count: 0,
            antenna_configuration: "1TXx3RX".to_string(),
            sampling_rate_hz: Some(100.0),
            preprocessing_version: String::new(),
        }
    }

    /// Deterministic synthetic window (unit-test fixture only — never a
    /// stand-in for real CSI).
    fn synthetic_window(
        n_t: usize,
        n_tx: usize,
        n_rx: usize,
        n_sc: usize,
    ) -> (Array4<f32>, Array4<f32>) {
        let amp = Array4::<f32>::from_shape_fn((n_t, n_tx, n_rx, n_sc), |(t, tx, rx, sc)| {
            0.5 + 0.4 * (((t * 7 + tx * 3 + rx * 2 + sc) % 17) as f32 / 17.0)
        });
        let phase = Array4::<f32>::from_shape_fn((n_t, n_tx, n_rx, n_sc), |(t, tx, rx, sc)| {
            ((t + tx + rx + sc) as f32 * 0.05).sin() * 2.0
        });
        (amp, phase)
    }

    #[test]
    fn config_validation_rejects_nonsense() {
        RealCsiConfig::default()
            .validate()
            .expect("defaults must validate");
        assert!(RealCsiConfig {
            target_subcarriers: 0,
            ..RealCsiConfig::default()
        }
        .validate()
        .is_err());
        assert!(RealCsiConfig {
            max_outlier_fraction: 1.5,
            ..RealCsiConfig::default()
        }
        .validate()
        .is_err());
        assert!(RealCsiConfig {
            hampel_half_window: 0,
            ..RealCsiConfig::default()
        }
        .validate()
        .is_err());
        assert!(RealCsiPipeline::new(RealCsiConfig {
            target_subcarriers: 0,
            ..RealCsiConfig::default()
        })
        .is_err());
    }

    #[test]
    fn converts_114_to_56_and_records_provenance() {
        let mut pipe = RealCsiPipeline::new(RealCsiConfig::default()).expect("pipeline");
        let (amp, phase) = synthetic_window(10, 1, 3, 114);
        let out = pipe.process(&amp, &phase, test_meta()).expect("process");
        assert_eq!(out.amplitude.shape(), &[10, 1, 3, 56]);
        assert_eq!(out.phase.shape(), &[10, 1, 3, 56]);
        assert_eq!(out.meta.original_subcarrier_count, 114);
        assert_eq!(out.meta.processed_subcarrier_count, 56);
        assert_eq!(out.meta.preprocessing_version, PREPROCESSING_VERSION);
        assert!(out
            .quality
            .conversion_method
            .contains("linear-interpolation"));
        assert!(out.amplitude.iter().all(|v| v.is_finite()));
        assert!(out.phase.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn native_width_passes_through_untouched_shape() {
        let mut pipe = RealCsiPipeline::new(RealCsiConfig {
            enable_phase_sanitization: false,
            enable_denoising: false,
            ..RealCsiConfig::default()
        })
        .expect("pipeline");
        let (amp, phase) = synthetic_window(8, 1, 3, 56);
        let out = pipe.process(&amp, &phase, test_meta()).expect("process");
        assert_eq!(out.amplitude.shape(), &[8, 1, 3, 56]);
        assert!(out.quality.conversion_method.contains("native-passthrough"));
        assert_eq!(out.quality.phase_lanes_sanitized, 0);
    }

    #[test]
    fn processing_is_deterministic() {
        let (amp, phase) = synthetic_window(10, 1, 3, 114);
        let mut a = RealCsiPipeline::new(RealCsiConfig::default()).expect("a");
        let mut b = RealCsiPipeline::new(RealCsiConfig::default()).expect("b");
        let ra = a.process(&amp, &phase, test_meta()).expect("a process");
        let rb = b.process(&amp, &phase, test_meta()).expect("b process");
        assert_eq!(ra.amplitude, rb.amplitude);
        assert_eq!(ra.phase, rb.phase);
    }

    #[test]
    fn nan_input_is_rejected_fail_closed() {
        let mut pipe = RealCsiPipeline::new(RealCsiConfig::default()).expect("pipeline");
        let (mut amp, phase) = synthetic_window(8, 1, 3, 56);
        amp[[3, 0, 1, 7]] = f32::NAN;
        let err = pipe
            .process(&amp, &phase, test_meta())
            .expect_err("NaN must fail");
        assert!(
            matches!(err, PreprocessError::NonFiniteInput { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn infinite_phase_is_rejected() {
        let mut pipe = RealCsiPipeline::new(RealCsiConfig::default()).expect("pipeline");
        let (amp, mut phase) = synthetic_window(8, 1, 3, 56);
        phase[[0, 0, 0, 0]] = f32::INFINITY;
        assert!(pipe.process(&amp, &phase, test_meta()).is_err());
    }

    #[test]
    fn shape_mismatch_is_rejected() {
        let mut pipe = RealCsiPipeline::new(RealCsiConfig::default()).expect("pipeline");
        let (amp, _) = synthetic_window(8, 1, 3, 56);
        let (_, phase) = synthetic_window(8, 1, 3, 30);
        let err = pipe
            .process(&amp, &phase, test_meta())
            .expect_err("must fail");
        assert!(
            matches!(err, PreprocessError::ShapeMismatch { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn insufficient_frames_are_rejected() {
        let mut pipe = RealCsiPipeline::new(RealCsiConfig::default()).expect("pipeline");
        let (amp, phase) = synthetic_window(1, 1, 3, 56);
        let err = pipe
            .process(&amp, &phase, test_meta())
            .expect_err("must fail");
        assert!(
            matches!(err, PreprocessError::InsufficientFrames { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn corrupted_window_trips_degraded_signal() {
        let mut pipe = RealCsiPipeline::new(RealCsiConfig {
            max_outlier_fraction: 0.01,
            ..RealCsiConfig::default()
        })
        .expect("pipeline");
        // Every lane gets one large spike; with a 1% ceiling the window must
        // be rejected instead of silently repaired.
        let (mut amp, phase) = synthetic_window(8, 1, 3, 56);
        for tx in 0..1 {
            for rx in 0..3 {
                for sc in 0..56 {
                    amp[[4, tx, rx, sc]] = 1e6;
                }
            }
        }
        let err = pipe
            .process(&amp, &phase, test_meta())
            .expect_err("must fail");
        assert!(
            matches!(err, PreprocessError::DegradedSignal { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn antenna_pairs_are_standardized() {
        // One pair carries a large DC offset; after standardization both
        // pairs must have ~zero mean over their (T, SC) window.
        let mut pipe = RealCsiPipeline::new(RealCsiConfig {
            enable_phase_sanitization: false,
            enable_denoising: false,
            ..RealCsiConfig::default()
        })
        .expect("pipeline");
        let (mut amp, phase) = synthetic_window(8, 1, 3, 56);
        for t in 0..8 {
            for sc in 0..56 {
                amp[[t, 0, 2, sc]] += 100.0;
            }
        }
        let out = pipe.process(&amp, &phase, test_meta()).expect("process");
        for rx in 0..3 {
            let mut mean = 0.0f64;
            let mut n = 0usize;
            for t in 0..8 {
                for sc in 0..56 {
                    mean += out.amplitude[[t, 0, rx, sc]] as f64;
                    n += 1;
                }
            }
            mean /= n as f64;
            assert!(
                mean.abs() < 1e-4,
                "pair rx={rx} mean {mean} not standardized"
            );
        }
    }
}
