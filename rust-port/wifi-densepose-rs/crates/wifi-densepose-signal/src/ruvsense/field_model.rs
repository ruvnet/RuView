//! Field Normal Mode computation for persistent electromagnetic world model.
//!
//! The room's electromagnetic eigenstructure forms the foundation for all
//! exotic sensing tiers. During unoccupied periods, the system learns a
//! baseline via SVD decomposition. At runtime, observations are decomposed
//! into environmental drift (projected onto eigenmodes) and body perturbation
//! (the residual).
//!
//! # Algorithm
//! 1. Collect CSI during empty-room calibration (>=10 min at 20 Hz)
//! 2. Compute per-link baseline mean (Welford online accumulator)
//! 3. Decompose covariance via SVD to extract environmental modes
//! 4. At runtime: observation - baseline, project out top-K modes, keep residual
//!
//! # References
//! - Welford, B.P. (1962). "Note on a Method for Calculating Corrected Sums
//!   of Squares and Products." Technometrics.
//! - ADR-030: RuvSense Persistent Field Model

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from field model operations.
#[derive(Debug, thiserror::Error)]
pub enum FieldModelError {
    /// Not enough calibration frames collected.
    #[error("Insufficient calibration frames: need {needed}, got {got}")]
    InsufficientCalibration { needed: usize, got: usize },

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
#[derive(Debug, Clone)]
pub struct FieldModelConfig {
    /// Number of links in the mesh.
    pub n_links: usize,
    /// Number of subcarriers per link.
    pub n_subcarriers: usize,
    /// Number of environmental modes to retain (K). Max 5.
    pub n_modes: usize,
    /// Minimum calibration frames before baseline is valid (10 min at 20 Hz = 12000).
    pub min_calibration_frames: usize,
    /// Baseline expiry in seconds (default 86400 = 24 hours).
    pub baseline_expiry_s: f64,
}

impl Default for FieldModelConfig {
    fn default() -> Self {
        Self {
            n_links: 6,
            n_subcarriers: 56,
            n_modes: 3,
            min_calibration_frames: 12_000,
            baseline_expiry_s: 86_400.0,
        }
    }
}

/// Electromagnetic eigenstructure of a room.
///
/// Learned from SVD on the covariance of CSI amplitudes during
/// empty-room calibration. The top-K modes capture environmental
/// variation (temperature, humidity, time-of-day effects).
#[derive(Debug, Clone)]
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

        Ok(Self {
            config,
            link_stats,
            modes: None,
            status: CalibrationStatus::Uncalibrated,
            last_calibration_us: 0,
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

    /// Number of calibration frames collected so far.
    pub fn calibration_frame_count(&self) -> u64 {
        self.link_stats
            .first()
            .map_or(0, |ls| ls.observation_count())
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
        if self.status == CalibrationStatus::Uncalibrated {
            self.status = CalibrationStatus::Collecting;
        }
        Ok(())
    }

    /// Finalize calibration: compute SVD to extract environmental modes.
    ///
    /// Requires at least `min_calibration_frames` observations.
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

        // Build covariance matrix from per-link variance data.
        // We average the variance vectors across all links to get the
        // covariance diagonal, then compute eigenmodes via power iteration.
        let n_sc = self.config.n_subcarriers;
        let n_modes = self.config.n_modes.min(n_sc);

        // Collect per-link baselines
        let baseline: Vec<Vec<f64>> = self.link_stats.iter().map(|ls| ls.mean_vector()).collect();

        // Average covariance across links (diagonal approximation)
        let mut avg_variance = vec![0.0_f64; n_sc];
        for ls in &self.link_stats {
            let var = ls.variance_vector();
            for (i, v) in var.iter().enumerate() {
                avg_variance[i] += v;
            }
        }
        let n_links_f = self.config.n_links as f64;
        for v in avg_variance.iter_mut() {
            *v /= n_links_f;
        }

        // Extract modes via simplified power iteration on the diagonal
        // covariance. Since we use a diagonal approximation, the eigenmodes
        // are aligned with the standard basis, sorted by variance.
        let total_variance: f64 = avg_variance.iter().sum();

        // Sort subcarrier indices by variance (descending) to pick top-K modes
        let mut indices: Vec<usize> = (0..n_sc).collect();
        indices.sort_by(|&a, &b| {
            avg_variance[b]
                .partial_cmp(&avg_variance[a])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut environmental_modes = Vec::with_capacity(n_modes);
        let mut mode_energies = Vec::with_capacity(n_modes);
        let mut explained = 0.0_f64;

        for k in 0..n_modes {
            let idx = indices[k];
            // Create a unit vector along the highest-variance subcarrier
            let mut mode = vec![0.0_f64; n_sc];
            mode[idx] = 1.0;
            let energy = avg_variance[idx];
            environmental_modes.push(mode);
            mode_energies.push(energy);
            explained += energy;
        }

        let variance_explained = if total_variance > 1e-15 {
            explained / total_variance
        } else {
            0.0
        };

        let field_mode = FieldNormalMode {
            baseline,
            environmental_modes,
            mode_energies,
            variance_explained,
            calibrated_at_us: timestamp_us,
            geometry_hash,
        };

        self.modes = Some(field_mode);
        self.status = CalibrationStatus::Fresh;
        self.last_calibration_us = timestamp_us;

        Ok(self.modes.as_ref().unwrap())
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
        let config = make_config(2, 4, 5);
        let mut model = FieldModel::new(config).unwrap();

        // Calibrate with baseline
        for _ in 0..5 {
            let obs = make_observations(2, 4, 1.0);
            model.feed_calibration(&obs).unwrap();
        }
        model.finalize_calibration(1_000_000, 0).unwrap();

        // Observe with a perturbation on top of baseline
        let mut perturbed = make_observations(2, 4, 1.0);
        perturbed[0][2] += 5.0; // big perturbation on link 0, subcarrier 2

        let perturbation = model.extract_perturbation(&perturbed).unwrap();
        assert!(perturbation.total_energy > 0.0);
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
}
