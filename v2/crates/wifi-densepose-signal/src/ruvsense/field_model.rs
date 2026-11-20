//! Field Normal Mode computation for persistent electromagnetic world model.
//!
//! The room's electromagnetic eigenstructure forms the foundation for all
//! exotic sensing tiers. During unoccupied periods, the system learns a
//! baseline via SVD decomposition. At runtime, observations are decomposed
//! into environmental drift (projected onto eigenmodes) and body perturbation
//! (the residual).
//!
//! # Algorithm
//! 1. Collect CSI during empty-room calibration (>=10 min wall-clock and at
//!    least twenty complete runtime-sized background windows)
//! 2. Compute per-link baseline mean (Welford online accumulator)
//! 3. Decompose covariance via SVD to extract environmental modes
//! 4. At runtime: observation - baseline, project out top-K modes, keep residual
//!
//! # References
//! - Welford, B.P. (1962). "Note on a Method for Calculating Corrected Sums
//!   of Squares and Products." Technometrics.
//! - ADR-030: RuvSense Persistent Field Model

use ndarray::Array2;
#[cfg(feature = "eigenvalue")]
use ndarray_linalg::Eigh;
#[cfg(feature = "eigenvalue")]
use ndarray_linalg::UPLO;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Calibration window constants
// ---------------------------------------------------------------------------

/// Intended wall-clock duration of the empty-room calibration window (s).
///
/// The Welford statistics exist to absorb slow environmental variation —
/// HVAC cycles, thermal drift — so the calibration gate is expressed in
/// wall-clock time. Every accepted node packet advances the same field model,
/// so N nodes reach any frame target N times faster; frames alone therefore
/// cannot gate calibration (#1756).
pub const CALIBRATION_DURATION_S: f64 = 600.0;

/// A recent window whose mean remains within this residual energy of the
/// empty room manifold is treated as background before eigenvalue counting.
const EMPTY_ROOM_RESIDUAL_ENERGY_MAX: f64 = 1.0;

/// Runtime occupancy is evaluated over fifty frames. Keep ten raw windows
/// from the end of calibration for covariance rank learning, plus bounded
/// privacy reduced window means spanning the full capture for residual
/// conformance scoring.
const RUNTIME_OCCUPANCY_WINDOW: usize = 50;
const CALIBRATION_REFERENCE_FRAMES: usize = RUNTIME_OCCUPANCY_WINDOW * 10;
/// Minimum independent runtime sized means required to learn an empirical
/// empty room residual distribution. Wall clock duration remains the primary
/// coverage gate, so slow hardware is not forced to imitate a nominal rate.
pub const MIN_BACKGROUND_REFERENCE_WINDOWS: usize = 20;
pub const MIN_CALIBRATION_FRAMES: usize =
    RUNTIME_OCCUPANCY_WINDOW * MIN_BACKGROUND_REFERENCE_WINDOWS;
const CALIBRATION_BACKGROUND_WINDOWS_MAX: usize = 512;
const EMPTY_ROOM_RESIDUAL_MARGIN: f64 = 1.5;
const EMPTY_ROOM_HELD_OUT_MARGIN: f64 = 1.10;
const EMPTY_ROOM_REFINEMENT_MAX_LIFT: f64 = 1.25;
const EMPTY_ROOM_REFINEMENT_MIN_SAMPLES: usize = 10;
const EMPTY_ROOM_REFINEMENT_MAX_SAMPLES: usize = 64;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from field model operations.
#[derive(Debug, thiserror::Error)]
pub enum FieldModelError {
    /// Not enough calibration frames collected.
    #[error("Insufficient calibration frames: need {needed}, got {got}")]
    InsufficientCalibration { needed: usize, got: usize },

    /// Calibration window shorter than the intended wall-clock duration.
    /// A frame count alone cannot gate calibration: N nodes streaming in
    /// aggregate reach any frame target in 1/N of the intended window (#1756).
    #[error("Calibration window too short: need {needed_s:.1}s of wall-clock time, got {got_s:.1}s")]
    InsufficientCalibrationDuration { needed_s: f64, got_s: f64 },

    /// Dimensionality mismatch between observation and baseline.
    #[error("Dimension mismatch: baseline has {expected} subcarriers, observation has {got}")]
    DimensionMismatch { expected: usize, got: usize },

    /// SVD computation failed.
    #[error("SVD computation failed: {0}")]
    SvdFailed(String),

    /// No links configured for the field model.
    #[error("No links configured")]
    NoLinks,

    /// Baseline has expired and needs recalibration.
    #[error("Baseline expired: calibrated {elapsed_s:.1}s ago, max {max_s:.1}s")]
    BaselineExpired { elapsed_s: f64, max_s: f64 },

    /// Invalid configuration parameter.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Model has not been calibrated yet.
    #[error("Field model not calibrated")]
    NotCalibrated,

    /// Not enough data for the requested operation.
    #[error("Insufficient data: need {need}, have {have}")]
    InsufficientData { need: usize, have: usize },
}

// ---------------------------------------------------------------------------
// Welford online statistics (f64 precision for accumulation)
// ---------------------------------------------------------------------------

/// Welford's online algorithm for computing running mean and variance.
///
/// Maintains numerically stable incremental statistics without storing
/// all observations. Uses f64 for accumulation precision even when
/// runtime values are f32.
///
/// # References
/// Welford (1962), Knuth TAOCP Vol 2 Section 4.2.2.
#[derive(Debug, Clone)]
pub struct WelfordStats {
    /// Number of observations accumulated.
    pub count: u64,
    /// Running mean.
    pub mean: f64,
    /// Running sum of squared deviations (M2).
    pub m2: f64,
}

impl WelfordStats {
    /// Create a new empty accumulator.
    pub fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    /// Add a new observation.
    pub fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    /// Population variance (biased). Returns 0.0 if count < 2.
    ///
    /// The `count < 2` guard is the n=0 NaN guard (ADR-154 §7.4 #10): at n=0,
    /// `m2 = 0` and `count = 0` would yield `0.0/0.0 = NaN`. Pinned by
    /// `welford_finite_at_n0_and_n1`.
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / self.count as f64
        }
    }

    /// Population standard deviation.
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Sample variance (unbiased). Returns 0.0 if count < 2.
    ///
    /// The `count < 2` guard is load-bearing (ADR-154 §7.4 #10): at n=0 the
    /// `(self.count - 1)` term would underflow `0usize − 1` and at n=1 it would
    /// divide by zero. Pinned by `welford_finite_at_n0_and_n1`.
    pub fn sample_variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }

    /// Compute z-score of a value against accumulated statistics.
    /// Returns 0.0 if standard deviation is near zero.
    pub fn z_score(&self, value: f64) -> f64 {
        let sd = self.std_dev();
        if sd < 1e-15 {
            0.0
        } else {
            (value - self.mean) / sd
        }
    }

    /// Merge two Welford accumulators (parallel Welford).
    pub fn merge(&mut self, other: &WelfordStats) {
        if other.count == 0 {
            return;
        }
        if self.count == 0 {
            *self = other.clone();
            return;
        }
        let total = self.count + other.count;
        let delta = other.mean - self.mean;
        let combined_mean = self.mean + delta * (other.count as f64 / total as f64);
        let combined_m2 = self.m2
            + other.m2
            + delta * delta * (self.count as f64 * other.count as f64 / total as f64);
        self.count = total;
        self.mean = combined_mean;
        self.m2 = combined_m2;
    }
}

impl Default for WelfordStats {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Multivariate Welford for per-subcarrier statistics
// ---------------------------------------------------------------------------

/// Per-subcarrier Welford accumulator for a single link.
///
/// Tracks independent running mean and variance for each subcarrier
/// on a given TX-RX link.
#[derive(Debug, Clone)]
pub struct LinkBaselineStats {
    /// Per-subcarrier accumulators.
    pub subcarriers: Vec<WelfordStats>,
}

impl LinkBaselineStats {
    /// Create accumulators for `n_subcarriers`.
    pub fn new(n_subcarriers: usize) -> Self {
        Self {
            subcarriers: (0..n_subcarriers).map(|_| WelfordStats::new()).collect(),
        }
    }

    /// Number of subcarriers tracked.
    pub fn n_subcarriers(&self) -> usize {
        self.subcarriers.len()
    }

    /// Update with a new CSI amplitude observation for this link.
    /// `amplitudes` must have the same length as `n_subcarriers`.
    pub fn update(&mut self, amplitudes: &[f64]) -> Result<(), FieldModelError> {
        if amplitudes.len() != self.subcarriers.len() {
            return Err(FieldModelError::DimensionMismatch {
                expected: self.subcarriers.len(),
                got: amplitudes.len(),
            });
        }
        for (stats, &amp) in self.subcarriers.iter_mut().zip(amplitudes.iter()) {
            stats.update(amp);
        }
        Ok(())
    }

    /// Extract the baseline mean vector.
    pub fn mean_vector(&self) -> Vec<f64> {
        self.subcarriers.iter().map(|s| s.mean).collect()
    }

    /// Extract the variance vector.
    pub fn variance_vector(&self) -> Vec<f64> {
        self.subcarriers.iter().map(|s| s.variance()).collect()
    }

    /// Number of observations accumulated.
    pub fn observation_count(&self) -> u64 {
        self.subcarriers.first().map_or(0, |s| s.count)
    }
}

// ---------------------------------------------------------------------------
// Field Normal Mode
// ---------------------------------------------------------------------------

/// Configuration for field model calibration and runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FieldModelConfig {
    /// Number of links in the mesh.
    pub n_links: usize,
    /// Number of subcarriers per link.
    pub n_subcarriers: usize,
    /// Number of environmental modes to retain (K). Max 5.
    pub n_modes: usize,
    /// Minimum calibration frames before baseline is valid. This is the
    /// runtime window size times the minimum number of independent background
    /// references. A frame count alone is not sufficient: N nodes streaming
    /// in aggregate reach it in 1/N of the window, so
    /// `min_calibration_duration_s` also gates finalization (#1756).
    pub min_calibration_frames: usize,
    /// Minimum wall-clock duration of the calibration window in seconds.
    /// Slow environmental variation (HVAC cycles, thermal drift) can only be
    /// observed over time, regardless of how many frames a fast fleet
    /// delivers (#1756). 0 disables the duration gate (tests only).
    pub min_calibration_duration_s: f64,
    /// Baseline expiry in seconds (default 86400 = 24 hours).
    pub baseline_expiry_s: f64,
}

impl Default for FieldModelConfig {
    fn default() -> Self {
        Self {
            n_links: 6,
            n_subcarriers: 56,
            n_modes: 3,
            min_calibration_frames: MIN_CALIBRATION_FRAMES,
            min_calibration_duration_s: CALIBRATION_DURATION_S,
            baseline_expiry_s: 86_400.0,
        }
    }
}

/// Electromagnetic eigenstructure of a room.
///
/// Learned from SVD on the covariance of CSI amplitudes during
/// empty-room calibration. The top-K modes capture environmental
/// variation (temperature, humidity, time-of-day effects).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FieldNormalMode {
    /// Per-link baseline mean: `[n_links][n_subcarriers]`.
    pub baseline: Vec<Vec<f64>>,
    /// Environmental eigenmodes: `[n_modes][n_subcarriers]`.
    /// Each mode is an orthonormal vector in subcarrier space.
    pub environmental_modes: Vec<Vec<f64>>,
    /// Eigenvalues (mode energies), sorted descending.
    pub mode_energies: Vec<f64>,
    /// Fraction of total variance explained by retained modes.
    pub variance_explained: f64,
    /// Timestamp (microseconds) when calibration completed.
    pub calibrated_at_us: u64,
    /// Hash of mesh geometry at calibration time.
    pub geometry_hash: u64,
    /// Baseline eigenvalue count above Marcenko-Pastur threshold (empty-room).
    pub baseline_eigenvalue_count: usize,
    /// Baseline noise variance estimate (median of bottom-half positive
    /// eigenvalues from the calibration covariance). Persisted so that
    /// `estimate_occupancy` can anchor its Marcenko-Pastur threshold to the
    /// calibration noise floor instead of letting it drift with the
    /// per-window sample size. Defaults to 0.0 in the diagonal-fallback path.
    /// Issue #942.
    pub baseline_noise_var: f64,
    /// Empty room significant eigenvalue count learned from runtime sized
    /// windows. The percentile is stored instead of raw CSI. `None` identifies
    /// snapshots created before runtime window calibration was introduced.
    #[serde(default)]
    pub baseline_runtime_eigenvalue_count: Option<usize>,
    /// Number of frames used for the runtime window reference above.
    #[serde(default)]
    pub baseline_runtime_window_size: Option<usize>,
    /// Hardware adaptive upper bound for the residual energy of an empty room
    /// window mean. Only this aggregate boundary is persisted.
    #[serde(default)]
    pub empty_room_residual_energy_threshold: Option<f64>,
    /// Sorted residual energies of privacy reduced runtime window means sampled
    /// across the full calibration. Raw CSI is never persisted. These scalar
    /// references support an empirical background match score without turning
    /// an empty room model into positive person evidence.
    #[serde(default)]
    pub empty_room_residual_energy_reference: Vec<f64>,
    /// Number of operator-confirmed held-out empty refinements. Version one
    /// permits one bounded refinement so repeated calls cannot ratchet the
    /// background boundary upward until a weak occupant is suppressed.
    #[serde(default)]
    pub empty_room_residual_refinement_count: u8,
}

/// A bounded comparison between one runtime window and the learned empty room
/// residual distribution. `score` is an empirical conformance score, not a
/// probability that a person is absent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmptyRoomMatch {
    pub matches_empty: bool,
    pub score: Option<f64>,
    /// Robust residual distance from the quiet reference median.
    pub normalized_residual_z: Option<f64>,
    /// Reference coverage relative to the normal calibration target.
    pub maturity: f64,
    /// Whether the reference set is large and valid enough to suppress change.
    pub reliable: bool,
    pub residual_energy: f64,
    pub residual_energy_threshold: f64,
    pub window_size: usize,
    pub reference_window_count: usize,
}

/// Receipt for one bounded, operator-confirmed empty-room refinement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmptyRoomRefinement {
    pub threshold_before: f64,
    pub threshold_after: f64,
    pub held_out_p95: f64,
    pub held_out_sample_count: usize,
    pub reference_window_count: usize,
}

/// Body perturbation extracted from a CSI observation.
///
/// After subtracting the baseline and projecting out environmental
/// modes, the residual captures structured changes caused by people
/// in the room.
#[derive(Debug, Clone)]
pub struct BodyPerturbation {
    /// Per-link residual amplitudes: `[n_links][n_subcarriers]`.
    pub residuals: Vec<Vec<f64>>,
    /// Per-link perturbation energy (L2 norm of residual).
    pub energies: Vec<f64>,
    /// Total perturbation energy across all links.
    pub total_energy: f64,
    /// Per-link environmental projection magnitude.
    pub environmental_projections: Vec<f64>,
}

/// Calibration status of the field model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationStatus {
    /// No calibration data yet.
    Uncalibrated,
    /// Collecting calibration frames.
    Collecting,
    /// Calibration complete and fresh.
    Fresh,
    /// Calibration older than half expiry.
    Stale,
    /// Calibration has expired.
    Expired,
}

/// Privacy reduced restart image for a completed field model.
///
/// This contains only aggregate baseline statistics and environmental modes.
/// It excludes raw CSI frames, device addresses, room names, credentials, and
/// calibration authority. A restored image is only a bootstrap prior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FieldModelSnapshotV1 {
    pub config: FieldModelConfig,
    pub modes: FieldNormalMode,
}

/// The persistent field model for a single room.
///
/// Maintains per-link Welford statistics during calibration, then
/// computes SVD to extract environmental modes. At runtime, decomposes
/// observations into environmental drift and body perturbation.
#[derive(Debug)]
pub struct FieldModel {
    config: FieldModelConfig,
    /// Per-link calibration statistics.
    link_stats: Vec<LinkBaselineStats>,
    /// Computed field normal modes (None until calibration completes).
    modes: Option<FieldNormalMode>,
    /// Current calibration status.
    status: CalibrationStatus,
    /// Timestamp of last calibration completion (microseconds).
    last_calibration_us: u64,
    /// Running outer-product sum for full covariance SVD: [n_sub x n_sub].
    covariance_sum: Option<Array2<f64>>,
    /// Number of frames accumulated into covariance_sum.
    covariance_count: u64,
    /// Monotonic timestamp of the first accepted calibration frame of the
    /// current session; `None` until collection starts (#1756).
    calibration_started: Option<std::time::Instant>,
    /// Bounded, memory only tail used to learn inference sized empty room
    /// covariance statistics. It is never included in a snapshot.
    calibration_reference_tail: VecDeque<Vec<Vec<f64>>>,
    /// Per link sum for the current privacy reduced runtime sized window.
    calibration_window_sum: Vec<Vec<f64>>,
    /// Number of accepted observations in `calibration_window_sum`.
    calibration_window_count: usize,
    /// Runtime sized per link means spanning the full calibration. This is
    /// bounded, memory only, and reduced to scalar residuals at finalization.
    calibration_window_means: VecDeque<Vec<Vec<f64>>>,
}

/// Diagonal variance fallback for when full covariance SVD is unavailable.
///
/// Returns `(mode_energies, environmental_modes, baseline_eigenvalue_count)`.
fn diagonal_fallback(
    link_stats: &[LinkBaselineStats],
    n_sc: usize,
    n_modes: usize,
) -> (Vec<f64>, Vec<Vec<f64>>, usize) {
    // Average variance across links (diagonal approximation)
    let mut avg_variance = vec![0.0_f64; n_sc];
    for ls in link_stats {
        let var = ls.variance_vector();
        for (i, v) in var.iter().enumerate() {
            avg_variance[i] += v;
        }
    }
    let n_links_f = link_stats.len() as f64;
    if n_links_f > 0.0 {
        for v in avg_variance.iter_mut() {
            *v /= n_links_f;
        }
    }

    // Sort subcarrier indices by variance (descending) to pick top-K modes
    let mut indices: Vec<usize> = (0..n_sc).collect();
    indices.sort_by(|&a, &b| {
        avg_variance[b]
            .partial_cmp(&avg_variance[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut environmental_modes = Vec::with_capacity(n_modes);
    let mut mode_energies = Vec::with_capacity(n_modes);

    for &idx in indices.iter().take(n_modes.min(n_sc)) {
        let mut mode = vec![0.0_f64; n_sc];
        mode[idx] = 1.0;
        mode_energies.push(avg_variance[idx]);
        environmental_modes.push(mode);
    }

    // For diagonal fallback, estimate baseline eigenvalue count from variance
    let total_var: f64 = avg_variance.iter().sum();
    let mean_var = if n_sc > 0 {
        total_var / n_sc as f64
    } else {
        0.0
    };
    let baseline_count = avg_variance.iter().filter(|&&v| v > mean_var * 2.0).count();

    (mode_energies, environmental_modes, baseline_count)
}

#[cfg(feature = "eigenvalue")]
fn significant_eigenvalue_count(
    frames: &[Vec<f64>],
    n_subcarriers: usize,
    baseline_noise_var: f64,
) -> Option<usize> {
    if frames.len() < 10 {
        return None;
    }

    let mut mean = vec![0.0_f64; n_subcarriers];
    let mut count = 0_usize;
    for frame in frames {
        if frame.len() >= n_subcarriers {
            for index in 0..n_subcarriers {
                mean[index] += frame[index];
            }
            count += 1;
        }
    }
    if count < 2 {
        return None;
    }
    for value in &mut mean {
        *value /= count as f64;
    }

    let mut covariance = Array2::<f64>::zeros((n_subcarriers, n_subcarriers));
    for frame in frames {
        if frame.len() >= n_subcarriers {
            for row in 0..n_subcarriers {
                let centered_row = frame[row] - mean[row];
                for column in row..n_subcarriers {
                    let value = centered_row * (frame[column] - mean[column]);
                    covariance[[row, column]] += value;
                    if row != column {
                        covariance[[column, row]] += value;
                    }
                }
            }
        }
    }
    covariance *= 1.0 / (count as f64 - 1.0);

    let (eigenvalues, _) = covariance.eigh(UPLO::Upper).ok()?;
    let mut positive: Vec<f64> = eigenvalues
        .iter()
        .copied()
        .filter(|value| *value > 1e-10)
        .collect();
    positive.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let local_noise_var = if positive.len() >= 4 {
        let half = positive.len() / 2;
        positive[..half].iter().sum::<f64>() / half as f64
    } else {
        positive.first().copied()?
    };
    let noise_var = if baseline_noise_var > 0.0 {
        local_noise_var.max(baseline_noise_var)
    } else {
        local_noise_var
    };
    let ratio = n_subcarriers as f64 / count as f64;
    let threshold = noise_var * (1.0 + ratio.sqrt()).powi(2);
    Some(
        eigenvalues
            .iter()
            .filter(|&&value| value > threshold)
            .count(),
    )
}

fn residual_energy_for_link(
    modes: &FieldNormalMode,
    link_index: usize,
    observation: &[f64],
) -> Option<f64> {
    let baseline = modes.baseline.get(link_index)?;
    if observation.len() != baseline.len() {
        return None;
    }
    let mut residual: Vec<f64> = observation
        .iter()
        .zip(baseline.iter())
        .map(|(value, reference)| value - reference)
        .collect();
    for mode in &modes.environmental_modes {
        let projection: f64 = residual
            .iter()
            .zip(mode.iter())
            .map(|(value, basis)| value * basis)
            .sum();
        for (value, basis) in residual.iter_mut().zip(mode.iter()) {
            *value -= projection * basis;
        }
    }
    Some(
        residual
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt(),
    )
}

fn percentile_95(mut values: Vec<f64>) -> Option<f64> {
    values.retain(|value| value.is_finite());
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let index = values
        .len()
        .saturating_mul(95)
        .div_ceil(100)
        .saturating_sub(1);
    values.get(index).copied()
}

fn robust_residual_z(reference: &[f64], residual: f64) -> Option<f64> {
    if !residual.is_finite() || reference.is_empty() {
        return None;
    }
    let mut sorted: Vec<f64> = reference
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .collect();
    if sorted.len() != reference.len() {
        return None;
    }
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[sorted.len() / 2];
    let mut deviations: Vec<f64> = sorted.iter().map(|value| (value - median).abs()).collect();
    deviations.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let mad = deviations[deviations.len() / 2];
    let robust_scale = (1.4826 * mad).max((median.abs() * 0.01).max(f64::EPSILON));
    Some((residual - median) / robust_scale)
}

#[cfg(feature = "eigenvalue")]
fn percentile_95_usize(mut values: Vec<usize>) -> Option<usize> {
    values.sort_unstable();
    let index = values
        .len()
        .saturating_mul(95)
        .div_ceil(100)
        .saturating_sub(1);
    values.get(index).copied()
}

impl FieldModel {
    /// Create a new field model for the given configuration.
    pub fn new(config: FieldModelConfig) -> Result<Self, FieldModelError> {
        if config.n_links == 0 {
            return Err(FieldModelError::NoLinks);
        }
        if config.n_modes > 5 {
            return Err(FieldModelError::InvalidConfig(
                "n_modes must be <= 5 to avoid overfitting".into(),
            ));
        }
        if config.n_subcarriers == 0 {
            return Err(FieldModelError::InvalidConfig(
                "n_subcarriers must be > 0".into(),
            ));
        }

        let link_stats = (0..config.n_links)
            .map(|_| LinkBaselineStats::new(config.n_subcarriers))
            .collect();
        let calibration_window_sum = vec![vec![0.0_f64; config.n_subcarriers]; config.n_links];

        Ok(Self {
            config,
            link_stats,
            modes: None,
            status: CalibrationStatus::Uncalibrated,
            last_calibration_us: 0,
            covariance_sum: None,
            covariance_count: 0,
            calibration_started: None,
            calibration_reference_tail: VecDeque::with_capacity(CALIBRATION_REFERENCE_FRAMES),
            calibration_window_sum,
            calibration_window_count: 0,
            calibration_window_means: VecDeque::with_capacity(CALIBRATION_BACKGROUND_WINDOWS_MAX),
        })
    }

    /// Current calibration status.
    pub fn status(&self) -> CalibrationStatus {
        self.status
    }

    /// Access the computed field normal modes, if available.
    pub fn modes(&self) -> Option<&FieldNormalMode> {
        self.modes.as_ref()
    }

    /// Hardware adaptive empty room boundary learned at finalization. Returns
    /// `None` for legacy snapshots that predate runtime window calibration.
    pub fn empty_room_residual_energy_threshold(&self) -> Option<f64> {
        self.modes
            .as_ref()
            .and_then(|modes| modes.empty_room_residual_energy_threshold)
    }

    /// Refine a completed single-link empty-room boundary with scalar residuals
    /// measured during a separate operator-confirmed empty holdout.
    ///
    /// This never ingests or persists raw CSI. The lift is capped at 25 percent
    /// and may happen only once per snapshot, preventing repeated calls from
    /// silently erasing weak occupied evidence.
    pub fn refine_empty_room_residual_boundary(
        &mut self,
        held_out_residuals: &[f64],
    ) -> Result<EmptyRoomRefinement, FieldModelError> {
        if self.config.n_links != 1 || self.status != CalibrationStatus::Fresh {
            return Err(FieldModelError::NotCalibrated);
        }
        if !(EMPTY_ROOM_REFINEMENT_MIN_SAMPLES..=EMPTY_ROOM_REFINEMENT_MAX_SAMPLES)
            .contains(&held_out_residuals.len())
            || held_out_residuals
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(FieldModelError::InvalidConfig(
                "held-out empty residuals are missing, excessive, or non-finite".into(),
            ));
        }
        let modes = self.modes.as_mut().ok_or(FieldModelError::NotCalibrated)?;
        if modes.empty_room_residual_refinement_count != 0 {
            return Err(FieldModelError::InvalidConfig(
                "the empty-room boundary already has its one permitted held-out refinement"
                    .into(),
            ));
        }
        if modes.empty_room_residual_energy_reference.len() + held_out_residuals.len()
            > CALIBRATION_BACKGROUND_WINDOWS_MAX
        {
            return Err(FieldModelError::InvalidConfig(
                "the empty-room residual reference limit would be exceeded".into(),
            ));
        }
        let threshold_before = modes
            .empty_room_residual_energy_threshold
            .ok_or(FieldModelError::NotCalibrated)?
            .max(EMPTY_ROOM_RESIDUAL_ENERGY_MAX);
        let held_out_p95 = percentile_95(held_out_residuals.to_vec())
            .ok_or(FieldModelError::NotCalibrated)?;
        let proposed = held_out_p95 * EMPTY_ROOM_HELD_OUT_MARGIN;
        let threshold_after = threshold_before
            .max(proposed.min(threshold_before * EMPTY_ROOM_REFINEMENT_MAX_LIFT));

        modes
            .empty_room_residual_energy_reference
            .extend_from_slice(held_out_residuals);
        modes
            .empty_room_residual_energy_reference
            .sort_by(|left, right| {
                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
            });
        modes.empty_room_residual_energy_threshold = Some(threshold_after);
        modes.empty_room_residual_refinement_count = 1;

        Ok(EmptyRoomRefinement {
            threshold_before,
            threshold_after,
            held_out_p95,
            held_out_sample_count: held_out_residuals.len(),
            reference_window_count: modes.empty_room_residual_energy_reference.len(),
        })
    }

    /// Export aggregate model state suitable for local restart persistence.
    /// Raw calibration observations and Welford accumulators are not exported.
    pub fn export_snapshot(&self) -> Result<FieldModelSnapshotV1, FieldModelError> {
        let modes = self.modes.clone().ok_or(FieldModelError::NotCalibrated)?;
        Ok(FieldModelSnapshotV1 {
            config: self.config.clone(),
            modes,
        })
    }

    /// Restore a bounded, validated aggregate snapshot.
    pub fn from_snapshot(
        snapshot: FieldModelSnapshotV1,
        current_us: u64,
    ) -> Result<Self, FieldModelError> {
        let config = &snapshot.config;
        if config.n_links > 16 || config.n_subcarriers > 2_048 {
            return Err(FieldModelError::InvalidConfig(
                "snapshot dimensions exceed bounded limits".into(),
            ));
        }
        if !config.min_calibration_duration_s.is_finite()
            || config.min_calibration_duration_s < 0.0
            || !config.baseline_expiry_s.is_finite()
            || config.baseline_expiry_s <= 0.0
            || config.baseline_expiry_s > 604_800.0
        {
            return Err(FieldModelError::InvalidConfig(
                "snapshot timing values are invalid".into(),
            ));
        }

        let modes = &snapshot.modes;
        let baseline_shape_valid = modes.baseline.len() == config.n_links
            && modes
                .baseline
                .iter()
                .all(|link| link.len() == config.n_subcarriers);
        let mode_shape_valid = modes.environmental_modes.len() == modes.mode_energies.len()
            && modes.environmental_modes.len() <= config.n_modes
            && modes
                .environmental_modes
                .iter()
                .all(|mode| mode.len() == config.n_subcarriers);
        let values_valid = modes
            .baseline
            .iter()
            .flatten()
            .chain(modes.environmental_modes.iter().flatten())
            .all(|value| value.is_finite())
            && modes
                .mode_energies
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0)
            && modes.variance_explained.is_finite()
            && (0.0..=10.0).contains(&modes.variance_explained)
            && modes.baseline_noise_var.is_finite()
            && modes.baseline_noise_var >= 0.0
            && modes.baseline_eigenvalue_count <= config.n_subcarriers
            && modes
                .baseline_runtime_eigenvalue_count
                .is_none_or(|count| count <= config.n_subcarriers)
            && modes
                .baseline_runtime_window_size
                .is_none_or(|size| (10..=CALIBRATION_REFERENCE_FRAMES).contains(&size))
            && modes
                .empty_room_residual_energy_threshold
                .is_none_or(|threshold| threshold.is_finite() && threshold >= 0.0)
            && modes.empty_room_residual_energy_reference.len()
                <= CALIBRATION_BACKGROUND_WINDOWS_MAX
            && modes.empty_room_residual_refinement_count <= 1
            && modes
                .empty_room_residual_energy_reference
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0)
            && modes
                .empty_room_residual_energy_reference
                .windows(2)
                .all(|window| window[0] <= window[1]);
        if !baseline_shape_valid || !mode_shape_valid || !values_valid {
            return Err(FieldModelError::InvalidConfig(
                "snapshot modes are malformed or non finite".into(),
            ));
        }
        if modes.calibrated_at_us == 0
            || modes.calibrated_at_us > current_us.saturating_add(300_000_000)
        {
            return Err(FieldModelError::InvalidConfig(
                "snapshot calibration time is invalid".into(),
            ));
        }
        let elapsed_s = current_us.saturating_sub(modes.calibrated_at_us) as f64 / 1_000_000.0;
        if elapsed_s > config.baseline_expiry_s {
            return Err(FieldModelError::BaselineExpired {
                elapsed_s,
                max_s: config.baseline_expiry_s,
            });
        }

        let mut model = Self::new(snapshot.config)?;
        model.status = if elapsed_s > model.config.baseline_expiry_s * 0.5 {
            CalibrationStatus::Stale
        } else {
            CalibrationStatus::Fresh
        };
        model.last_calibration_us = modes.calibrated_at_us;
        model.modes = Some(snapshot.modes);
        Ok(model)
    }

    /// Number of calibration frames collected so far.
    pub fn calibration_frame_count(&self) -> u64 {
        self.link_stats
            .first()
            .map_or(0, |ls| ls.observation_count())
    }

    /// Minimum frames required before `finalize_calibration` will succeed.
    pub fn min_calibration_frames(&self) -> usize {
        self.config.min_calibration_frames
    }

    /// Minimum wall-clock duration (s) of the calibration window required
    /// before `finalize_calibration` will succeed (#1756).
    pub fn min_calibration_duration_s(&self) -> f64 {
        self.config.min_calibration_duration_s
    }

    /// Wall-clock seconds elapsed since the first accepted calibration frame
    /// of the current session, or 0.0 if collection has not started (#1756).
    pub fn calibration_elapsed_s(&self) -> f64 {
        self.calibration_started
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }

    /// Effective aggregate frames per second since collection started, or 0.0
    /// if collection has not started (#1756). With N nodes streaming, this is
    /// roughly N times the single-node rate.
    pub fn calibration_frames_per_second(&self) -> f64 {
        let elapsed = self.calibration_elapsed_s();
        if elapsed > 0.0 {
            self.calibration_frame_count() as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Feed a calibration frame (one CSI observation per link during empty room).
    ///
    /// `observations` is `[n_links][n_subcarriers]` amplitude data.
    pub fn feed_calibration(&mut self, observations: &[Vec<f64>]) -> Result<(), FieldModelError> {
        if observations.len() != self.config.n_links {
            return Err(FieldModelError::DimensionMismatch {
                expected: self.config.n_links,
                got: observations.len(),
            });
        }
        for (link_stat, obs) in self.link_stats.iter_mut().zip(observations.iter()) {
            link_stat.update(obs)?;
        }
        // Start the wall-clock calibration window on the first accepted frame
        // (#1756): finalize gates on elapsed duration as well as frame count.
        if self.calibration_started.is_none() {
            self.calibration_started = Some(std::time::Instant::now());
        }
        if self.status == CalibrationStatus::Uncalibrated {
            self.status = CalibrationStatus::Collecting;
        }

        // Accumulate raw outer products for SVD covariance (no centering here —
        // mean subtraction is deferred to finalize_calibration to avoid bias).
        // We average across links so covariance_count tracks frames, not links.
        let n = self.config.n_subcarriers;
        let cov = self
            .covariance_sum
            .get_or_insert_with(|| Array2::zeros((n, n)));
        let _n_links = observations.len();
        for obs in observations {
            if obs.len() >= n {
                // Rank-1 update: cov += obs * obs^T (raw, un-centered)
                for i in 0..n {
                    for j in i..n {
                        let val = obs[i] * obs[j];
                        cov[[i, j]] += val;
                        if i != j {
                            cov[[j, i]] += val;
                        }
                    }
                }
            }
        }
        // Count once per frame (not per link) for correct MP ratio
        self.covariance_count += 1;

        if self.calibration_reference_tail.len() == CALIBRATION_REFERENCE_FRAMES {
            self.calibration_reference_tail.pop_front();
        }
        self.calibration_reference_tail
            .push_back(observations.to_vec());

        for (link_sum, observation) in self
            .calibration_window_sum
            .iter_mut()
            .zip(observations.iter())
        {
            for (sum, value) in link_sum.iter_mut().zip(observation.iter()) {
                *sum += value;
            }
        }
        self.calibration_window_count += 1;
        if self.calibration_window_count == RUNTIME_OCCUPANCY_WINDOW {
            let divisor = self.calibration_window_count as f64;
            let means: Vec<Vec<f64>> = self
                .calibration_window_sum
                .iter()
                .map(|link_sum| link_sum.iter().map(|value| value / divisor).collect())
                .collect();
            if self.calibration_window_means.len() == CALIBRATION_BACKGROUND_WINDOWS_MAX {
                self.calibration_window_means.pop_front();
            }
            self.calibration_window_means.push_back(means);
            for link_sum in &mut self.calibration_window_sum {
                link_sum.fill(0.0);
            }
            self.calibration_window_count = 0;
        }

        Ok(())
    }

    /// Finalize calibration: compute SVD to extract environmental modes.
    ///
    /// Requires at least `min_calibration_frames` observations collected over
    /// at least `min_calibration_duration_s` of wall-clock time (#1756).
    /// `timestamp_us` is the current timestamp in microseconds.
    /// `geometry_hash` identifies the mesh geometry at calibration time.
    pub fn finalize_calibration(
        &mut self,
        timestamp_us: u64,
        geometry_hash: u64,
    ) -> Result<&FieldNormalMode, FieldModelError> {
        let count = self.calibration_frame_count();
        if count < self.config.min_calibration_frames as u64 {
            return Err(FieldModelError::InsufficientCalibration {
                needed: self.config.min_calibration_frames,
                got: count as usize,
            });
        }
        // #1756: the frame gate encodes "duration at the single-node rate",
        // but every accepted node packet advances this model, so a fleet of N
        // nodes satisfies it in ~1/N of the intended window. Gate on the
        // wall-clock duration as well so the baseline covers the slow
        // environmental variation the Welford statistics are meant to absorb.
        let elapsed_s = self.calibration_elapsed_s();
        let need_s = self.config.min_calibration_duration_s;
        if need_s > 0.0 && elapsed_s < need_s {
            return Err(FieldModelError::InsufficientCalibrationDuration {
                needed_s: need_s,
                got_s: elapsed_s,
            });
        }

        let n_sc = self.config.n_subcarriers;
        let n_modes = self.config.n_modes.min(n_sc);

        // Collect per-link baselines
        let baseline: Vec<Vec<f64>> = self.link_stats.iter().map(|ls| ls.mean_vector()).collect();

        // --- True eigenvalue decomposition (with diagonal fallback) ---
        // Returns: (energies, modes, baseline_count, baseline_noise_var).
        // The noise_var slot is 0.0 in the diagonal-fallback paths; the
        // estimation hot path treats 0.0 as "no anchored noise floor" and
        // falls back to per-window noise_var, preserving pre-#942 behavior.
        let (mode_energies, environmental_modes, baseline_eig_count, baseline_noise_var) =
            if let Some(ref cov_sum) = self.covariance_sum {
                if self.covariance_count > 1 {
                    // Compute sample covariance from raw outer products:
                    //   cov = (sum_xx / N - mean * mean^T) * N / (N-1)
                    // where sum_xx accumulated obs * obs^T across all links per frame.
                    // We average per-link means for centering.
                    let n_frames = self.covariance_count as f64;
                    let n_links = self.config.n_links as f64;
                    // Average mean across all links
                    let mut avg_mean = vec![0.0f64; n_sc];
                    for ls in &self.link_stats {
                        let m = ls.mean_vector();
                        for (a, &mi) in avg_mean.iter_mut().zip(m.iter()) {
                            *a += mi;
                        }
                    }
                    for a in avg_mean.iter_mut() {
                        *a /= n_links;
                    }
                    // cov = sum_xx / (N * n_links) - mean * mean^T, then Bessel correction
                    let total_obs = n_frames * n_links;
                    let mut covariance = cov_sum / total_obs;
                    for i in 0..n_sc {
                        for j in 0..n_sc {
                            covariance[[i, j]] -= avg_mean[i] * avg_mean[j];
                        }
                    }
                    // Bessel's correction: multiply by N/(N-1) where N = total observations
                    let bessel = total_obs / (total_obs - 1.0);
                    covariance *= bessel;

                    // Symmetric eigendecomposition (requires eigenvalue feature / BLAS)
                    #[cfg(feature = "eigenvalue")]
                    match covariance.eigh(UPLO::Upper) {
                        Ok((eigenvalues, eigenvectors)) => {
                            // eigenvalues are in ascending order from ndarray-linalg
                            // Reverse to get descending
                            let len = eigenvalues.len();
                            let mut sorted_indices: Vec<usize> = (0..len).collect();
                            sorted_indices.sort_by(|&a, &b| {
                                eigenvalues[b]
                                    .partial_cmp(&eigenvalues[a])
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            });

                            // Extract top n_modes
                            let modes: Vec<Vec<f64>> = sorted_indices
                                .iter()
                                .take(n_modes)
                                .map(|&idx| eigenvectors.column(idx).to_vec())
                                .collect();
                            let energies: Vec<f64> = sorted_indices
                                .iter()
                                .take(n_modes)
                                .map(|&idx| eigenvalues[idx].max(0.0))
                                .collect();

                            // Marcenko-Pastur noise estimate: median of POSITIVE
                            // eigenvalues in the bottom half. Excludes zeros from
                            // rank-deficient matrices (when p > n).
                            let noise_var = {
                                let mut positive: Vec<f64> =
                                    eigenvalues.iter().copied().filter(|&e| e > 1e-10).collect();
                                positive.sort_by(|a, b| {
                                    a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                                });
                                if positive.len() >= 4 {
                                    let half = positive.len() / 2;
                                    positive[..half].iter().sum::<f64>() / half as f64
                                } else if !positive.is_empty() {
                                    positive[0]
                                } else {
                                    1e-10
                                }
                            };
                            // MP ratio: p/n where n = total observations (frames * links)
                            let total_obs_mp =
                                self.covariance_count as f64 * self.config.n_links as f64;
                            let ratio = n_sc as f64 / total_obs_mp;
                            let mp_threshold = noise_var * (1.0 + ratio.sqrt()).powi(2);
                            let baseline_count =
                                eigenvalues.iter().filter(|&&ev| ev > mp_threshold).count();

                            (energies, modes, baseline_count, noise_var)
                        }
                        Err(_) => {
                            // Fallback to diagonal approximation on SVD failure
                            let (e, m, b) = diagonal_fallback(&self.link_stats, n_sc, n_modes);
                            (e, m, b, 0.0_f64)
                        }
                    }
                    // When eigenvalue feature is disabled, use diagonal fallback
                    #[cfg(not(feature = "eigenvalue"))]
                    {
                        let (e, m, b) = diagonal_fallback(&self.link_stats, n_sc, n_modes);
                        (e, m, b, 0.0_f64)
                    }
                } else {
                    let (e, m, b) = diagonal_fallback(&self.link_stats, n_sc, n_modes);
                    (e, m, b, 0.0_f64)
                }
            } else {
                let (e, m, b) = diagonal_fallback(&self.link_stats, n_sc, n_modes);
                (e, m, b, 0.0_f64)
            };

        // Compute variance explained using the same centered covariance as modes.
        // total_variance = trace(centered_covariance) = sum of ALL eigenvalues.
        let total_energy: f64 = mode_energies.iter().sum();
        let total_variance = if let Some(ref cov_sum) = self.covariance_sum {
            if self.covariance_count > 1 {
                let n_links_f = self.config.n_links as f64;
                let total_obs = self.covariance_count as f64 * n_links_f;
                // Centered trace: E[x^2] - E[x]^2, with Bessel correction
                let mut avg_mean = vec![0.0f64; n_sc];
                for ls in &self.link_stats {
                    let m = ls.mean_vector();
                    for (a, &mi) in avg_mean.iter_mut().zip(m.iter()) {
                        *a += mi;
                    }
                }
                for a in avg_mean.iter_mut() {
                    *a /= n_links_f;
                }
                let raw_trace: f64 = (0..n_sc).map(|i| cov_sum[[i, i]] / total_obs).sum();
                let mean_sq: f64 = avg_mean.iter().map(|m| m * m).sum();
                (raw_trace - mean_sq).max(0.0) * total_obs / (total_obs - 1.0)
            } else {
                total_energy
            }
        } else {
            total_energy
        };
        let variance_explained = if total_variance > 1e-15 {
            total_energy / total_variance
        } else {
            0.0
        };

        let mut field_mode = FieldNormalMode {
            baseline,
            environmental_modes,
            mode_energies,
            variance_explained,
            calibrated_at_us: timestamp_us,
            geometry_hash,
            baseline_eigenvalue_count: baseline_eig_count,
            baseline_noise_var,
            baseline_runtime_eigenvalue_count: None,
            baseline_runtime_window_size: None,
            empty_room_residual_energy_threshold: None,
            empty_room_residual_energy_reference: Vec::new(),
            empty_room_residual_refinement_count: 0,
        };

        // The full calibration covariance and a fifty frame runtime covariance
        // have different Marcenko Pastur aspect ratios. This first version is
        // deliberately single link: combining link specific residual scales
        // requires a separate multistatic calibration model.
        if self.config.n_links == 1 {
            let reference_frames: Vec<Vec<f64>> = self
                .calibration_reference_tail
                .iter()
                .filter_map(|observation| observation.first().cloned())
                .collect();
            let complete_windows: Vec<&[Vec<f64>]> = reference_frames
                .chunks(RUNTIME_OCCUPANCY_WINDOW)
                .filter(|window| window.len() == RUNTIME_OCCUPANCY_WINDOW)
                .collect();

            #[cfg(feature = "eigenvalue")]
            {
                let counts: Vec<usize> = complete_windows
                    .iter()
                    .filter_map(|window| {
                        significant_eigenvalue_count(window, n_sc, baseline_noise_var)
                    })
                    .collect();
                field_mode.baseline_runtime_eigenvalue_count = percentile_95_usize(counts);
            }

            let mut residual_energies: Vec<f64> = self
                .calibration_window_means
                .iter()
                .filter_map(|means| means.first())
                .filter_map(|mean| residual_energy_for_link(&field_mode, 0, mean))
                .filter(|value| value.is_finite() && *value >= 0.0)
                .collect();
            if residual_energies.is_empty() {
                residual_energies = complete_windows
                    .iter()
                    .filter_map(|window| {
                        let mut mean = vec![0.0_f64; n_sc];
                        for frame in *window {
                            for (sum, value) in mean.iter_mut().zip(frame.iter()) {
                                *sum += value;
                            }
                        }
                        for value in &mut mean {
                            *value /= window.len() as f64;
                        }
                        residual_energy_for_link(&field_mode, 0, &mean)
                    })
                    .collect();
            }
            residual_energies.sort_by(|left, right| {
                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
            });
            field_mode.empty_room_residual_energy_threshold =
                percentile_95(residual_energies.clone()).map(|value| {
                    (value * EMPTY_ROOM_RESIDUAL_MARGIN).max(EMPTY_ROOM_RESIDUAL_ENERGY_MAX)
                });
            field_mode.empty_room_residual_energy_reference = residual_energies;
            if field_mode.baseline_runtime_eigenvalue_count.is_some()
                || field_mode.empty_room_residual_energy_threshold.is_some()
            {
                field_mode.baseline_runtime_window_size = Some(RUNTIME_OCCUPANCY_WINDOW);
            }
        }

        self.modes = Some(field_mode);
        self.status = CalibrationStatus::Fresh;
        self.last_calibration_us = timestamp_us;
        self.calibration_reference_tail.clear();
        self.calibration_window_means.clear();
        for link_sum in &mut self.calibration_window_sum {
            link_sum.fill(0.0);
        }
        self.calibration_window_count = 0;

        Ok(self.modes.as_ref().unwrap())
    }

    /// Compare a runtime sized single link window with the learned empty room
    /// distribution. The score is empirical conformance while the learned
    /// boundary holds. Crossing the boundary returns zero. Legacy snapshots
    /// without residual references retain binary suppression but no score.
    pub fn empty_room_match(&self, recent_frames: &[Vec<f64>]) -> Option<EmptyRoomMatch> {
        let modes = self.modes.as_ref()?;
        if self.config.n_links != 1 {
            return None;
        }
        let window_size = modes.baseline_runtime_window_size?;
        if recent_frames.len() < window_size {
            return None;
        }
        let mut mean = vec![0.0_f64; self.config.n_subcarriers];
        for frame in recent_frames.iter().take(window_size) {
            if frame.len() < self.config.n_subcarriers {
                return None;
            }
            for (sum, value) in mean.iter_mut().zip(frame.iter()) {
                *sum += value;
            }
        }
        for value in &mut mean {
            *value /= window_size as f64;
        }
        let residual_energy = residual_energy_for_link(modes, 0, &mean)?;
        let residual_energy_threshold = modes
            .empty_room_residual_energy_threshold?
            .max(EMPTY_ROOM_RESIDUAL_ENERGY_MAX);
        let reference_window_count = modes.empty_room_residual_energy_reference.len();
        let maturity = (reference_window_count as f64
            / MIN_BACKGROUND_REFERENCE_WINDOWS as f64)
            .clamp(0.0, 1.0);
        let reliable = reference_window_count >= 10
            && modes
                .empty_room_residual_energy_reference
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0);
        let matches_empty = reliable && residual_energy <= residual_energy_threshold;
        let score = if !reliable {
            None
        } else if !matches_empty {
            Some(0.0)
        } else {
            let at_or_below = modes
                .empty_room_residual_energy_reference
                .partition_point(|value| *value <= residual_energy);
            let empirical_quantile = at_or_below as f64 / reference_window_count as f64;
            Some((1.0 - empirical_quantile * 0.5).clamp(0.5, 1.0))
        };
        Some(EmptyRoomMatch {
            matches_empty,
            score,
            normalized_residual_z: robust_residual_z(
                &modes.empty_room_residual_energy_reference,
                residual_energy,
            ),
            maturity,
            reliable,
            residual_energy,
            residual_energy_threshold,
            window_size,
            reference_window_count,
        })
    }

    /// Extract body perturbation from a runtime observation.
    ///
    /// Subtracts baseline, projects out environmental modes, returns residual.
    /// `observations` is `[n_links][n_subcarriers]` amplitude data.
    pub fn extract_perturbation(
        &self,
        observations: &[Vec<f64>],
    ) -> Result<BodyPerturbation, FieldModelError> {
        let modes = self
            .modes
            .as_ref()
            .ok_or(FieldModelError::InsufficientCalibration {
                needed: self.config.min_calibration_frames,
                got: 0,
            })?;

        if observations.len() != self.config.n_links {
            return Err(FieldModelError::DimensionMismatch {
                expected: self.config.n_links,
                got: observations.len(),
            });
        }

        let n_sc = self.config.n_subcarriers;
        let mut residuals = Vec::with_capacity(self.config.n_links);
        let mut energies = Vec::with_capacity(self.config.n_links);
        let mut environmental_projections = Vec::with_capacity(self.config.n_links);

        for (link_idx, obs) in observations.iter().enumerate() {
            if obs.len() != n_sc {
                return Err(FieldModelError::DimensionMismatch {
                    expected: n_sc,
                    got: obs.len(),
                });
            }

            // Step 1: subtract baseline
            let mut residual = vec![0.0_f64; n_sc];
            for i in 0..n_sc {
                residual[i] = obs[i] - modes.baseline[link_idx][i];
            }

            // Step 2: project out environmental modes
            let mut env_proj_magnitude = 0.0_f64;
            for mode in &modes.environmental_modes {
                // Inner product of residual with mode
                let projection: f64 = residual.iter().zip(mode.iter()).map(|(r, m)| r * m).sum();
                env_proj_magnitude += projection.abs();

                // Subtract projection
                for i in 0..n_sc {
                    residual[i] -= projection * mode[i];
                }
            }

            // Step 3: compute energy (L2 norm)
            let energy: f64 = residual.iter().map(|r| r * r).sum::<f64>().sqrt();

            environmental_projections.push(env_proj_magnitude);
            energies.push(energy);
            residuals.push(residual);
        }

        let total_energy: f64 = energies.iter().sum();

        Ok(BodyPerturbation {
            residuals,
            energies,
            total_energy,
            environmental_projections,
        })
    }

    /// Estimate room occupancy from eigenvalue analysis of recent CSI frames.
    ///
    /// `recent_frames`: sliding window of amplitude vectors (recommend 50 frames
    /// ~ 2.5s at 20 Hz). Returns estimated person count (0 = empty room).
    ///
    /// Requires the `eigenvalue` feature (BLAS). Returns `NotCalibrated` when
    /// the feature is disabled.
    #[cfg(feature = "eigenvalue")]
    pub fn estimate_occupancy(&self, recent_frames: &[Vec<f64>]) -> Result<usize, FieldModelError> {
        let modes = self.modes.as_ref().ok_or(FieldModelError::NotCalibrated)?;

        let n = self.config.n_subcarriers;
        if recent_frames.len() < 10 {
            return Err(FieldModelError::InsufficientData {
                need: 10,
                have: recent_frames.len(),
            });
        }

        // Build covariance matrix from recent frames
        let mut mean = vec![0.0f64; n];
        let mut count = 0usize;
        for frame in recent_frames {
            if frame.len() >= n {
                for i in 0..n {
                    mean[i] += frame[i];
                }
                count += 1;
            }
        }
        if count < 2 {
            return Ok(0);
        }
        for m in &mut mean {
            *m /= count as f64;
        }

        // Rank alone is not person evidence. A short continuation window can
        // expose more significant eigenvalues while its mean remains on the
        // learned empty room manifold. Use the hardware adaptive residual
        // boundary when available.
        let empty_room_residual_energy_max = modes
            .empty_room_residual_energy_threshold
            .unwrap_or(EMPTY_ROOM_RESIDUAL_ENERGY_MAX)
            .max(EMPTY_ROOM_RESIDUAL_ENERGY_MAX);
        let mean_residual_energy = self
            .extract_perturbation(&[mean.clone()])
            .ok()
            .map(|perturbation| perturbation.total_energy);
        if mean_residual_energy.is_some_and(|energy| energy <= empty_room_residual_energy_max) {
            return Ok(0);
        }

        let mut cov = Array2::<f64>::zeros((n, n));
        for frame in recent_frames {
            if frame.len() >= n {
                for i in 0..n {
                    let ci = frame[i] - mean[i];
                    for j in i..n {
                        let val = ci * (frame[j] - mean[j]);
                        cov[[i, j]] += val;
                        if i != j {
                            cov[[j, i]] += val;
                        }
                    }
                }
            }
        }
        let scale = 1.0 / (count as f64 - 1.0);
        cov *= scale;

        // Eigendecompose
        let eigenvalues = match cov.eigh(UPLO::Upper) {
            Ok((evals, _)) => evals,
            Err(_) => return Ok(0), // SVD failure = can't estimate
        };

        // Marcenko-Pastur noise estimate: median of POSITIVE eigenvalues
        // in the bottom half. Excludes zeros from rank-deficient matrices
        // (common when n_subcarriers > n_frames, e.g. 56 subcarriers / 50 frames).
        let local_noise_var = {
            let mut positive: Vec<f64> =
                eigenvalues.iter().copied().filter(|&e| e > 1e-10).collect();
            positive.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if positive.len() >= 4 {
                let half = positive.len() / 2;
                positive[..half].iter().sum::<f64>() / half as f64
            } else if !positive.is_empty() {
                positive[0]
            } else {
                return Ok(0); // All zero eigenvalues — can't estimate
            }
        };

        // Issue #942: anchor the noise floor to the calibration's noise_var
        // when it's available. Per-window noise_var drifts with sample size —
        // a short estimation window can produce a small local_noise_var that
        // inflates `significant` and breaks the test_estimate_occupancy_noise_only
        // invariant. The max of (calibration noise, local noise) keeps the
        // threshold from collapsing on small windows while still letting the
        // per-window noise dominate when it's the larger estimate. Falls back
        // to local_noise_var when baseline_noise_var == 0 (diagonal-fallback
        // calibration path, or pre-#942 stored modes).
        let noise_var = if modes.baseline_noise_var > 0.0 {
            local_noise_var.max(modes.baseline_noise_var)
        } else {
            local_noise_var
        };

        let ratio = n as f64 / count as f64;
        let mp_threshold = noise_var * (1.0 + ratio.sqrt()).powi(2);

        let significant = eigenvalues.iter().filter(|&&ev| ev > mp_threshold).count();
        let reference_count = if modes.baseline_runtime_window_size == Some(count) {
            modes
                .baseline_runtime_eigenvalue_count
                .unwrap_or(modes.baseline_eigenvalue_count)
        } else {
            modes.baseline_eigenvalue_count
        };
        let rank_occupancy = significant.saturating_sub(reference_count);
        // A stable person can shift the window mean without increasing its
        // covariance rank. Preserve at least one occupant after the learned
        // empty room residual boundary is exceeded.
        let occupancy =
            if mean_residual_energy.is_some_and(|energy| energy > empty_room_residual_energy_max) {
                rank_occupancy.max(1)
            } else {
                rank_occupancy
            };

        Ok(occupancy.min(10)) // Cap at 10 persons
    }

    /// Stub when eigenvalue feature is disabled — always returns NotCalibrated.
    #[cfg(not(feature = "eigenvalue"))]
    pub fn estimate_occupancy(
        &self,
        _recent_frames: &[Vec<f64>],
    ) -> Result<usize, FieldModelError> {
        Err(FieldModelError::NotCalibrated)
    }

    /// Check calibration freshness against a given timestamp.
    pub fn check_freshness(&self, current_us: u64) -> CalibrationStatus {
        if self.modes.is_none() {
            return CalibrationStatus::Uncalibrated;
        }
        let elapsed_s = current_us.saturating_sub(self.last_calibration_us) as f64 / 1_000_000.0;
        if elapsed_s > self.config.baseline_expiry_s {
            CalibrationStatus::Expired
        } else if elapsed_s > self.config.baseline_expiry_s * 0.5 {
            CalibrationStatus::Stale
        } else {
            CalibrationStatus::Fresh
        }
    }

    /// Reset calibration and begin collecting again.
    pub fn reset_calibration(&mut self) {
        self.link_stats = (0..self.config.n_links)
            .map(|_| LinkBaselineStats::new(self.config.n_subcarriers))
            .collect();
        self.modes = None;
        self.status = CalibrationStatus::Uncalibrated;
        self.covariance_sum = None;
        self.covariance_count = 0;
        self.calibration_started = None;
        self.calibration_reference_tail.clear();
        self.calibration_window_count = 0;
        for link_sum in &mut self.calibration_window_sum {
            link_sum.fill(0.0);
        }
        self.calibration_window_means.clear();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(n_links: usize, n_sc: usize, min_frames: usize) -> FieldModelConfig {
        FieldModelConfig {
            n_links,
            n_subcarriers: n_sc,
            n_modes: 3,
            min_calibration_frames: min_frames,
            // Tests feed frames instantly; disable the wall-clock gate (#1756).
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 86_400.0,
        }
    }

    fn make_observations(n_links: usize, n_sc: usize, base: f64) -> Vec<Vec<f64>> {
        (0..n_links)
            .map(|l| {
                (0..n_sc)
                    .map(|s| base + 0.1 * l as f64 + 0.01 * s as f64)
                    .collect()
            })
            .collect()
    }

    #[test]
    fn test_welford_basic() {
        let mut w = WelfordStats::new();
        for v in &[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0] {
            w.update(*v);
        }
        assert!((w.mean - 5.0).abs() < 1e-10);
        assert!((w.variance() - 4.0).abs() < 1e-10);
        assert_eq!(w.count, 8);
    }

    #[test]
    fn test_welford_z_score() {
        let mut w = WelfordStats::new();
        for v in 0..100 {
            w.update(v as f64);
        }
        let z = w.z_score(w.mean);
        assert!(z.abs() < 1e-10, "z-score of mean should be 0");
    }

    #[test]
    fn test_welford_merge() {
        let mut a = WelfordStats::new();
        let mut b = WelfordStats::new();
        for v in 0..50 {
            a.update(v as f64);
        }
        for v in 50..100 {
            b.update(v as f64);
        }
        a.merge(&b);
        assert_eq!(a.count, 100);
        assert!((a.mean - 49.5).abs() < 1e-10);
    }

    #[test]
    fn test_welford_single_value() {
        let mut w = WelfordStats::new();
        w.update(42.0);
        assert_eq!(w.count, 1);
        assert!((w.mean - 42.0).abs() < 1e-10);
        assert!((w.variance() - 0.0).abs() < 1e-10);
    }

    /// ADR-154 §7.4 #10: every statistic must stay FINITE at the n=0 and n=1
    /// boundaries. This pins the load-bearing `count < 2` guards: without them
    /// `sample_variance` at n=0 underflows `(0usize − 1)` and divides by a huge
    /// bogus divisor, and `variance`/`z_score` produce `0.0/0.0 = NaN`. Same
    /// family as the §4 divide-by-(n−1) window trio.
    #[test]
    fn welford_finite_at_n0_and_n1() {
        // n = 0: fresh accumulator, nothing observed.
        let w0 = WelfordStats::new();
        assert_eq!(w0.count, 0);
        for v in [
            w0.mean,
            w0.variance(),
            w0.sample_variance(),
            w0.std_dev(),
            w0.z_score(123.0),
        ] {
            assert!(v.is_finite(), "n=0 statistic must be finite, got {v}");
        }
        // Documented sentinels at n=0.
        assert_eq!(w0.variance(), 0.0);
        assert_eq!(w0.sample_variance(), 0.0);
        assert_eq!(w0.std_dev(), 0.0);
        assert_eq!(w0.z_score(123.0), 0.0);

        // n = 1: a single observation has no spread.
        let mut w1 = WelfordStats::new();
        w1.update(7.5);
        assert_eq!(w1.count, 1);
        for v in [
            w1.mean,
            w1.variance(),
            w1.sample_variance(),
            w1.std_dev(),
            w1.z_score(7.5),
            w1.z_score(999.0),
        ] {
            assert!(v.is_finite(), "n=1 statistic must be finite, got {v}");
        }
        assert_eq!(w1.variance(), 0.0);
        assert_eq!(w1.sample_variance(), 0.0);
        assert_eq!(w1.std_dev(), 0.0);
        // z_score guards on near-zero sd → 0.0 even for an off-mean query.
        assert_eq!(w1.z_score(999.0), 0.0);
    }

    #[test]
    fn test_link_baseline_stats() {
        let mut stats = LinkBaselineStats::new(4);
        stats.update(&[1.0, 2.0, 3.0, 4.0]).unwrap();
        stats.update(&[2.0, 3.0, 4.0, 5.0]).unwrap();

        let mean = stats.mean_vector();
        assert!((mean[0] - 1.5).abs() < 1e-10);
        assert!((mean[3] - 4.5).abs() < 1e-10);
    }

    #[test]
    fn test_link_baseline_dimension_mismatch() {
        let mut stats = LinkBaselineStats::new(4);
        let result = stats.update(&[1.0, 2.0]);
        assert!(result.is_err());
    }

    #[test]
    fn test_field_model_creation() {
        let config = make_config(6, 56, 100);
        let model = FieldModel::new(config).unwrap();
        assert_eq!(model.status(), CalibrationStatus::Uncalibrated);
        assert!(model.modes().is_none());
    }

    #[test]
    fn test_field_model_no_links_error() {
        let config = FieldModelConfig {
            n_links: 0,
            ..Default::default()
        };
        assert!(matches!(
            FieldModel::new(config),
            Err(FieldModelError::NoLinks)
        ));
    }

    #[test]
    fn test_field_model_too_many_modes() {
        let config = FieldModelConfig {
            n_modes: 6,
            ..Default::default()
        };
        assert!(matches!(
            FieldModel::new(config),
            Err(FieldModelError::InvalidConfig(_))
        ));
    }

    #[test]
    fn test_calibration_flow() {
        let config = make_config(2, 4, 10);
        let mut model = FieldModel::new(config).unwrap();

        // Feed calibration frames
        for i in 0..10 {
            let obs = make_observations(2, 4, 1.0 + 0.01 * i as f64);
            model.feed_calibration(&obs).unwrap();
        }

        assert_eq!(model.status(), CalibrationStatus::Collecting);
        assert_eq!(model.calibration_frame_count(), 10);

        // Finalize
        let modes = model.finalize_calibration(1_000_000, 0xDEAD).unwrap();
        assert_eq!(modes.environmental_modes.len(), 3);
        assert!(modes.variance_explained > 0.0);
        assert_eq!(model.status(), CalibrationStatus::Fresh);
    }

    #[test]
    fn test_calibration_insufficient_frames() {
        let config = make_config(2, 4, 100);
        let mut model = FieldModel::new(config).unwrap();

        for i in 0..5 {
            let obs = make_observations(2, 4, 1.0 + 0.01 * i as f64);
            model.feed_calibration(&obs).unwrap();
        }

        assert!(matches!(
            model.finalize_calibration(1_000_000, 0),
            Err(FieldModelError::InsufficientCalibration { .. })
        ));
    }

    #[test]
    fn test_perturbation_extraction() {
        // Use 8 subcarriers and only 2 modes so that most subcarriers
        // are NOT captured by environmental modes, leaving body perturbation
        // visible in the residual.
        let config = FieldModelConfig {
            n_links: 2,
            n_subcarriers: 8,
            n_modes: 2,
            min_calibration_frames: 5,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 86_400.0,
        };
        let mut model = FieldModel::new(config).unwrap();

        // Calibrate with drift on subcarriers 0 and 1 only
        for i in 0..10 {
            let obs = vec![
                vec![
                    1.0 + 0.5 * i as f64,
                    2.0 + 0.3 * i as f64,
                    3.0,
                    4.0,
                    5.0,
                    6.0,
                    7.0,
                    8.0,
                ],
                vec![
                    1.1 + 0.5 * i as f64,
                    2.1 + 0.3 * i as f64,
                    3.1,
                    4.1,
                    5.1,
                    6.1,
                    7.1,
                    8.1,
                ],
            ];
            model.feed_calibration(&obs).unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();

        // Observe with a big perturbation on subcarrier 5 (not an env mode)
        let mean_0 = 1.0 + 0.5 * 4.5; // midpoint mean
        let mean_1 = 2.0 + 0.3 * 4.5;
        let mut perturbed = vec![
            vec![mean_0, mean_1, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
            vec![mean_0 + 0.1, mean_1 + 0.1, 3.1, 4.1, 5.1, 6.1, 7.1, 8.1],
        ];
        perturbed[0][5] += 10.0; // big perturbation on link 0, subcarrier 5

        let perturbation = model.extract_perturbation(&perturbed).unwrap();
        assert!(
            perturbation.total_energy > 0.0,
            "Perturbation on non-mode subcarrier should be visible, got {}",
            perturbation.total_energy
        );
        assert!(perturbation.energies[0] > perturbation.energies[1]);
    }

    #[test]
    fn test_perturbation_baseline_observation_same() {
        let config = make_config(2, 4, 5);
        let mut model = FieldModel::new(config).unwrap();

        let obs = make_observations(2, 4, 1.0);
        for _ in 0..5 {
            model.feed_calibration(&obs).unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();

        let perturbation = model.extract_perturbation(&obs).unwrap();
        assert!(
            perturbation.total_energy < 0.01,
            "Same-as-baseline should yield near-zero perturbation"
        );
    }

    #[test]
    fn test_perturbation_dimension_mismatch() {
        let config = make_config(2, 4, 5);
        let mut model = FieldModel::new(config).unwrap();

        let obs = make_observations(2, 4, 1.0);
        for _ in 0..5 {
            model.feed_calibration(&obs).unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();

        // Wrong number of links
        let wrong_obs = make_observations(3, 4, 1.0);
        assert!(model.extract_perturbation(&wrong_obs).is_err());
    }

    #[test]
    fn test_calibration_freshness() {
        let config = make_config(2, 4, 5);
        let mut model = FieldModel::new(config).unwrap();

        let obs = make_observations(2, 4, 1.0);
        for _ in 0..5 {
            model.feed_calibration(&obs).unwrap();
        }
        model.finalize_calibration(0, 0).unwrap();

        assert_eq!(model.check_freshness(0), CalibrationStatus::Fresh);
        // 12 hours later: stale
        let twelve_hours_us = 12 * 3600 * 1_000_000;
        assert_eq!(
            model.check_freshness(twelve_hours_us),
            CalibrationStatus::Fresh
        );
        // 13 hours later: stale (> 50% of 24h)
        let thirteen_hours_us = 13 * 3600 * 1_000_000;
        assert_eq!(
            model.check_freshness(thirteen_hours_us),
            CalibrationStatus::Stale
        );
        // 25 hours later: expired
        let twentyfive_hours_us = 25 * 3600 * 1_000_000;
        assert_eq!(
            model.check_freshness(twentyfive_hours_us),
            CalibrationStatus::Expired
        );
    }

    #[test]
    fn test_reset_calibration() {
        let config = make_config(2, 4, 5);
        let mut model = FieldModel::new(config).unwrap();

        let obs = make_observations(2, 4, 1.0);
        for _ in 0..5 {
            model.feed_calibration(&obs).unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();
        assert!(model.modes().is_some());

        model.reset_calibration();
        assert!(model.modes().is_none());
        assert_eq!(model.status(), CalibrationStatus::Uncalibrated);
        assert_eq!(model.calibration_frame_count(), 0);
    }

    #[test]
    fn test_environmental_modes_sorted_by_energy() {
        let config = make_config(1, 8, 5);
        let mut model = FieldModel::new(config).unwrap();

        // Create observations with high variance on subcarrier 3
        for i in 0..20 {
            let mut obs = vec![vec![1.0; 8]];
            obs[0][3] += (i as f64) * 0.5; // high variance
            obs[0][7] += (i as f64) * 0.1; // lower variance
            model.feed_calibration(&obs).unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();

        let modes = model.modes().unwrap();
        // Eigenvalues should be in descending order
        for w in modes.mode_energies.windows(2) {
            assert!(w[0] >= w[1], "Mode energies must be descending");
        }
    }

    #[test]
    fn test_covariance_accumulation() {
        let config = make_config(2, 4, 5);
        let mut model = FieldModel::new(config).unwrap();

        // Feed calibration data
        for i in 0..10 {
            let obs = make_observations(2, 4, 1.0 + 0.1 * i as f64);
            model.feed_calibration(&obs).unwrap();
        }

        // covariance_sum should be populated
        assert!(model.covariance_sum.is_some());
        assert!(model.covariance_count > 0);
        let cov = model.covariance_sum.as_ref().unwrap();
        assert_eq!(cov.shape(), &[4, 4]);
        // Diagonal entries should be non-negative (sum of squares)
        for i in 0..4 {
            assert!(cov[[i, i]] >= 0.0, "Diagonal covariance entry must be >= 0");
        }
        // Matrix should be symmetric
        for i in 0..4 {
            for j in 0..4 {
                assert!(
                    (cov[[i, j]] - cov[[j, i]]).abs() < 1e-10,
                    "Covariance matrix must be symmetric"
                );
            }
        }
    }

    #[test]
    fn test_svd_finalize_produces_orthonormal_modes() {
        let config = FieldModelConfig {
            n_links: 1,
            n_subcarriers: 8,
            n_modes: 3,
            min_calibration_frames: 20,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 86_400.0,
        };
        let mut model = FieldModel::new(config).unwrap();

        // Feed frames with correlated subcarrier patterns to produce
        // non-trivial eigenmodes
        for i in 0..50 {
            let t = i as f64 * 0.1;
            let obs = vec![vec![
                1.0 + t.sin(),
                2.0 + t.cos(),
                3.0 + 0.5 * t.sin(),
                4.0 + 0.3 * t.cos(),
                5.0 + 0.1 * t,
                6.0,
                7.0 + 0.2 * (2.0 * t).sin(),
                8.0 + 0.1 * (2.0 * t).cos(),
            ]];
            model.feed_calibration(&obs).unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();

        let modes = model.modes().unwrap();
        // Each mode should be approximately unit length
        for (k, mode) in modes.environmental_modes.iter().enumerate() {
            let norm: f64 = mode.iter().map(|x| x * x).sum::<f64>().sqrt();
            assert!(
                (norm - 1.0).abs() < 0.01,
                "Mode {} has norm {} (expected ~1.0)",
                k,
                norm
            );
        }
        // Modes should be approximately orthogonal
        for i in 0..modes.environmental_modes.len() {
            for j in (i + 1)..modes.environmental_modes.len() {
                let dot: f64 = modes.environmental_modes[i]
                    .iter()
                    .zip(modes.environmental_modes[j].iter())
                    .map(|(a, b)| a * b)
                    .sum();
                assert!(
                    dot.abs() < 0.05,
                    "Modes {} and {} have dot product {} (expected ~0)",
                    i,
                    j,
                    dot
                );
            }
        }
    }

    // estimate_occupancy() falls back to a NotCalibrated stub without the
    // `eigenvalue` feature, so this test only makes sense with BLAS enabled.
    #[cfg(feature = "eigenvalue")]
    #[test]
    fn test_estimate_occupancy_noise_only() {
        let config = FieldModelConfig {
            n_links: 1,
            n_subcarriers: 8,
            n_modes: 3,
            min_calibration_frames: 20,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 86_400.0,
        };
        let mut model = FieldModel::new(config).unwrap();

        // Calibrate with some deterministic noise-like pattern
        for i in 0..50 {
            let t = i as f64 * 0.1;
            let obs = vec![vec![
                1.0 + 0.01 * t.sin(),
                2.0 + 0.01 * t.cos(),
                3.0 + 0.01 * (2.0 * t).sin(),
                4.0 + 0.01 * (2.0 * t).cos(),
                5.0 + 0.01 * (3.0 * t).sin(),
                6.0 + 0.01 * (3.0 * t).cos(),
                7.0 + 0.01 * (4.0 * t).sin(),
                8.0 + 0.01 * (4.0 * t).cos(),
            ]];
            model.feed_calibration(&obs).unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();

        // Estimate occupancy with similar noise-only frames
        let frames: Vec<Vec<f64>> = (0..20)
            .map(|i| {
                let t = (i + 50) as f64 * 0.1;
                vec![
                    1.0 + 0.01 * t.sin(),
                    2.0 + 0.01 * t.cos(),
                    3.0 + 0.01 * (2.0 * t).sin(),
                    4.0 + 0.01 * (2.0 * t).cos(),
                    5.0 + 0.01 * (3.0 * t).sin(),
                    6.0 + 0.01 * (3.0 * t).cos(),
                    7.0 + 0.01 * (4.0 * t).sin(),
                    8.0 + 0.01 * (4.0 * t).cos(),
                ]
            })
            .collect();
        let occupancy = model.estimate_occupancy(&frames).unwrap();
        assert_eq!(occupancy, 0, "Noise-only frames should yield 0 occupancy");
    }

    #[cfg(feature = "eigenvalue")]
    #[test]
    fn runtime_sized_empty_reference_scores_held_out_background() {
        let config = FieldModelConfig {
            n_links: 1,
            n_subcarriers: 56,
            n_modes: 3,
            min_calibration_frames: 500,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 86_400.0,
        };
        let mut model = FieldModel::new(config).unwrap();
        let mut calibration = Vec::new();
        for frame_index in 0..600 {
            let time = frame_index as f64 * 0.071;
            let frame: Vec<f64> = (0..56)
                .map(|subcarrier| {
                    let carrier = subcarrier as f64;
                    18.0 + carrier * 0.08
                        + (time + carrier * 0.13).sin() * 0.8
                        + (time * 0.37 + carrier * 0.031).cos() * 0.35
                })
                .collect();
            model.feed_calibration(&[frame.clone()]).unwrap();
            calibration.push(frame);
        }
        model.finalize_calibration(1_000_000, 0).unwrap();

        let modes = model.modes().unwrap();
        assert_eq!(
            modes.baseline_runtime_window_size,
            Some(RUNTIME_OCCUPANCY_WINDOW)
        );
        assert!(modes.baseline_runtime_eigenvalue_count.is_some());
        assert!(modes.empty_room_residual_energy_threshold.is_some());
        assert_eq!(modes.empty_room_residual_energy_reference.len(), 12);

        let held_out_empty = calibration[550..600].to_vec();
        assert_eq!(model.estimate_occupancy(&held_out_empty).unwrap(), 0);
        let background = model
            .empty_room_match(&held_out_empty)
            .expect("background match");
        assert!(background.matches_empty);
        assert_eq!(background.reference_window_count, 12);
        assert!(background.reliable);
        assert_eq!(background.maturity, 0.6);
        assert!(background.normalized_residual_z.is_some());
        assert!(background
            .score
            .is_some_and(|score| (0.5..=1.0).contains(&score)));
    }

    #[test]
    fn held_out_empty_refinement_is_bounded_and_single_use() {
        let config = FieldModelConfig {
            n_links: 1,
            n_subcarriers: 8,
            n_modes: 2,
            min_calibration_frames: 100,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 86_400.0,
        };
        let mut model = FieldModel::new(config).unwrap();
        for frame_index in 0..1_000 {
            let phase = frame_index as f64 * 0.071;
            let frame = (0..8)
                .map(|index| 20.0 + index as f64 * 0.1 + (phase + index as f64).sin())
                .collect();
            model.feed_calibration(&[frame]).unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();

        let before = model.empty_room_residual_energy_threshold().unwrap();
        let residuals = vec![before * 1.4; 12];
        let receipt = model
            .refine_empty_room_residual_boundary(&residuals)
            .unwrap();
        assert_eq!(receipt.threshold_before, before);
        assert!(receipt.threshold_after > before);
        assert!(receipt.threshold_after <= before * EMPTY_ROOM_REFINEMENT_MAX_LIFT);
        assert_eq!(receipt.held_out_sample_count, 12);
        assert_eq!(
            model.modes().unwrap().empty_room_residual_refinement_count,
            1
        );
        assert!(model
            .refine_empty_room_residual_boundary(&residuals)
            .is_err());
    }

    #[cfg(feature = "eigenvalue")]
    #[test]
    fn runtime_reference_rejects_shifted_held_out_window() {
        let config = FieldModelConfig {
            n_links: 1,
            n_subcarriers: 56,
            n_modes: 3,
            min_calibration_frames: 500,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 86_400.0,
        };
        let mut model = FieldModel::new(config).unwrap();
        for frame_index in 0..600 {
            let time = frame_index as f64 * 0.071;
            let frame: Vec<f64> = (0..56)
                .map(|subcarrier| {
                    let carrier = subcarrier as f64;
                    18.0 + carrier * 0.08
                        + (time + carrier * 0.13).sin() * 0.8
                        + (time * 0.37 + carrier * 0.031).cos() * 0.35
                })
                .collect();
            model.feed_calibration(&[frame]).unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();

        let shifted: Vec<Vec<f64>> = (0..RUNTIME_OCCUPANCY_WINDOW)
            .map(|frame_index| {
                let time = (600 + frame_index) as f64 * 0.071;
                (0..56)
                    .map(|subcarrier| {
                        let carrier = subcarrier as f64;
                        let body_shift = if subcarrier % 3 == 0 { 8.0 } else { -5.0 };
                        18.0 + carrier * 0.08
                            + (time + carrier * 0.13).sin() * 0.8
                            + (time * 0.37 + carrier * 0.031).cos() * 0.35
                            + body_shift
                    })
                    .collect()
            })
            .collect();
        assert!(model.estimate_occupancy(&shifted).unwrap() >= 1);
        let background = model.empty_room_match(&shifted).expect("shifted match");
        assert!(!background.matches_empty);
        assert_eq!(background.score, Some(0.0));
        assert!(background.reliable);
        assert!(background.normalized_residual_z.is_some());
    }

    #[cfg(feature = "eigenvalue")]
    #[test]
    fn sparse_background_reference_cannot_suppress_runtime_change() {
        let config = FieldModelConfig {
            n_links: 1,
            n_subcarriers: 56,
            n_modes: 3,
            min_calibration_frames: 500,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 86_400.0,
        };
        let mut model = FieldModel::new(config).unwrap();
        let mut calibration = Vec::new();
        for frame_index in 0..600 {
            let time = frame_index as f64 * 0.071;
            let frame: Vec<f64> = (0..56)
                .map(|subcarrier| {
                    let carrier = subcarrier as f64;
                    18.0 + carrier * 0.08
                        + (time + carrier * 0.13).sin() * 0.8
                        + (time * 0.37 + carrier * 0.031).cos() * 0.35
                })
                .collect();
            model.feed_calibration(&[frame.clone()]).unwrap();
            calibration.push(frame);
        }
        model.finalize_calibration(1_000_000, 0).unwrap();
        model
            .modes
            .as_mut()
            .unwrap()
            .empty_room_residual_energy_reference
            .truncate(9);

        let result = model
            .empty_room_match(&calibration[550..600])
            .expect("background comparison");
        assert!(!result.reliable);
        assert!(!result.matches_empty);
        assert_eq!(result.score, None);
        assert_eq!(result.maturity, 0.45);
    }

    #[test]
    fn runtime_background_reference_is_single_link_only() {
        let config = FieldModelConfig {
            n_links: 2,
            n_subcarriers: 8,
            n_modes: 2,
            min_calibration_frames: 100,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 86_400.0,
        };
        let mut model = FieldModel::new(config).unwrap();
        for frame_index in 0..100 {
            let phase = frame_index as f64 * 0.1;
            model
                .feed_calibration(&[
                    (0..8).map(|index| phase + index as f64).collect(),
                    (0..8).map(|index| phase - index as f64).collect(),
                ])
                .unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();

        let modes = model.modes().unwrap();
        assert_eq!(modes.baseline_runtime_window_size, None);
        assert_eq!(modes.baseline_runtime_eigenvalue_count, None);
        assert_eq!(modes.empty_room_residual_energy_threshold, None);
        assert!(modes.empty_room_residual_energy_reference.is_empty());
    }

    #[test]
    fn test_baseline_eigenvalue_count_stored() {
        let config = FieldModelConfig {
            n_links: 1,
            n_subcarriers: 8,
            n_modes: 3,
            min_calibration_frames: 20,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 86_400.0,
        };
        let mut model = FieldModel::new(config).unwrap();

        // Feed frames with structured variance so eigenvalues are meaningful
        for i in 0..50 {
            let t = i as f64 * 0.1;
            let obs = vec![vec![
                1.0 + t.sin(),
                2.0 + t.cos(),
                3.0 + 0.5 * t.sin(),
                4.0 + 0.3 * t.cos(),
                5.0 + 0.1 * t,
                6.0,
                7.0,
                8.0,
            ]];
            model.feed_calibration(&obs).unwrap();
        }
        let modes = model.finalize_calibration(1_000_000, 0).unwrap();
        // baseline_eigenvalue_count should exist and be a reasonable value
        // (at least 0, at most n_subcarriers)
        assert!(
            modes.baseline_eigenvalue_count <= 8,
            "baseline_eigenvalue_count should be <= n_subcarriers"
        );
    }

    #[test]
    fn snapshot_round_trip_preserves_aggregate_modes_without_raw_frames() {
        let config = make_config(1, 4, 10);
        let mut model = FieldModel::new(config).unwrap();
        for i in 0..10 {
            model
                .feed_calibration(&[vec![1.0 + i as f64 * 0.01, 2.0, 3.0, 4.0]])
                .unwrap();
        }
        model.finalize_calibration(1_000_000, 0xCA1).unwrap();

        let snapshot = model.export_snapshot().unwrap();
        let restored = FieldModel::from_snapshot(snapshot.clone(), 2_000_000).unwrap();

        assert_eq!(restored.status(), CalibrationStatus::Fresh);
        assert_eq!(restored.calibration_frame_count(), 0);
        assert_eq!(restored.export_snapshot().unwrap(), snapshot);
    }

    #[test]
    fn snapshot_restore_rejects_expired_and_malformed_images() {
        let config = make_config(1, 4, 10);
        let mut model = FieldModel::new(config).unwrap();
        for _ in 0..10 {
            model.feed_calibration(&[vec![1.0, 2.0, 3.0, 4.0]]).unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();
        let snapshot = model.export_snapshot().unwrap();

        assert!(matches!(
            FieldModel::from_snapshot(snapshot.clone(), 90_000_000_000),
            Err(FieldModelError::BaselineExpired { .. })
        ));

        let mut malformed = snapshot;
        malformed.modes.baseline[0].pop();
        assert!(matches!(
            FieldModel::from_snapshot(malformed, 2_000_000),
            Err(FieldModelError::InvalidConfig(_))
        ));
    }

    #[test]
    fn test_environmental_projection_removes_drift() {
        let config = make_config(1, 4, 10);
        let mut model = FieldModel::new(config).unwrap();

        // Calibrate with drift on subcarrier 0
        for i in 0..10 {
            let obs = vec![vec![
                1.0 + 0.5 * i as f64, // drifting
                2.0,
                3.0,
                4.0,
            ]];
            model.feed_calibration(&obs).unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();

        // Observe with same drift pattern (no body)
        let obs = vec![vec![1.0 + 0.5 * 5.0, 2.0, 3.0, 4.0]];
        let perturbation = model.extract_perturbation(&obs).unwrap();

        // The drift on subcarrier 0 should be mostly captured by
        // environmental modes, leaving small residual
        assert!(
            perturbation.environmental_projections[0] > 0.0,
            "Environmental projection should be non-zero for drifting subcarrier"
        );
    }

    // -----------------------------------------------------------------
    // Wall-clock calibration gate (#1756)
    // -----------------------------------------------------------------

    #[test]
    fn test_default_frame_target_covers_background_reference_windows() {
        let cfg = FieldModelConfig::default();
        assert_eq!(cfg.min_calibration_duration_s, CALIBRATION_DURATION_S);
        assert_eq!(
            cfg.min_calibration_frames, MIN_CALIBRATION_FRAMES,
            "frame target must cover the minimum independent runtime windows"
        );
        assert_eq!(MIN_CALIBRATION_FRAMES, 1_000);
        assert_eq!(MIN_BACKGROUND_REFERENCE_WINDOWS, 20);
    }

    #[test]
    fn test_finalize_requires_wall_clock_window() {
        // Enough frames, but fed instantly: the fleet-fast scenario from #1756.
        let mut config = make_config(1, 4, 5);
        config.min_calibration_duration_s = 600.0;
        let mut model = FieldModel::new(config).unwrap();
        for _ in 0..10 {
            model
                .feed_calibration(&make_observations(1, 4, 1.0))
                .unwrap();
        }
        assert!(model.calibration_frame_count() >= 5);
        match model.finalize_calibration(1_000_000, 0) {
            Err(FieldModelError::InsufficientCalibrationDuration { needed_s, got_s }) => {
                assert!((needed_s - 600.0).abs() < 1e-9);
                assert!(got_s < 1.0, "test feeds frames instantly, got {got_s}s");
            }
            other => panic!("expected InsufficientCalibrationDuration, got {other:?}"),
        }
    }

    #[test]
    fn test_duration_gate_disabled_with_zero() {
        // min_calibration_duration_s = 0 keeps the legacy frame-only gate.
        let mut model = FieldModel::new(make_config(1, 4, 5)).unwrap();
        for _ in 0..5 {
            model
                .feed_calibration(&make_observations(1, 4, 1.0))
                .unwrap();
        }
        assert!(model.finalize_calibration(1_000_000, 0).is_ok());
    }

    #[test]
    fn test_calibration_clock_accessors() {
        let mut model = FieldModel::new(make_config(1, 4, 5)).unwrap();
        // No frames yet: the session clock has not started.
        assert_eq!(model.calibration_elapsed_s(), 0.0);
        assert_eq!(model.calibration_frames_per_second(), 0.0);

        model
            .feed_calibration(&make_observations(1, 4, 1.0))
            .unwrap();
        assert!(model.calibration_elapsed_s() >= 0.0);
        assert!(model.calibration_frames_per_second() >= 0.0);

        // Reset clears the session clock.
        model.reset_calibration();
        assert_eq!(model.calibration_elapsed_s(), 0.0);
        assert_eq!(model.calibration_frames_per_second(), 0.0);
    }
}
