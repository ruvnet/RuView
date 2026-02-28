//! Fresnel Zone Breathing Model
//!
//! Models WiFi signal variation as a function of human chest displacement
//! crossing Fresnel zone boundaries. At 5 GHz (λ=60mm), chest displacement
//! of 5-10mm during breathing is a significant fraction of the Fresnel zone
//! width, producing measurable phase and amplitude changes.
//!
//! # References
//! - FarSense: Pushing the Range Limit (MobiCom 2019)
//! - Wi-Sleep: Contactless Sleep Staging (UbiComp 2021)

use ruvector_solver::neumann::NeumannSolver;
use ruvector_solver::types::CsrMatrix;
use std::f64::consts::PI;

/// Physical constants and defaults for WiFi sensing.
pub const SPEED_OF_LIGHT: f64 = 2.998e8; // m/s

/// Fresnel zone geometry for a TX-RX-body configuration.
#[derive(Debug, Clone)]
pub struct FresnelGeometry {
    /// Distance from TX to body reflection point (meters)
    pub d_tx_body: f64,
    /// Distance from body reflection point to RX (meters)
    pub d_body_rx: f64,
    /// Carrier frequency in Hz (e.g., 5.8e9 for 5.8 GHz)
    pub frequency: f64,
}

impl FresnelGeometry {
    /// Create geometry for a given TX-body-RX configuration.
    pub fn new(d_tx_body: f64, d_body_rx: f64, frequency: f64) -> Result<Self, FresnelError> {
        if d_tx_body <= 0.0 || d_body_rx <= 0.0 {
            return Err(FresnelError::InvalidDistance);
        }
        if frequency <= 0.0 {
            return Err(FresnelError::InvalidFrequency);
        }
        Ok(Self {
            d_tx_body,
            d_body_rx,
            frequency,
        })
    }

    /// Wavelength in meters.
    pub fn wavelength(&self) -> f64 {
        SPEED_OF_LIGHT / self.frequency
    }

    /// Radius of the nth Fresnel zone at the body point.
    ///
    /// F_n = sqrt(n * λ * d1 * d2 / (d1 + d2))
    pub fn fresnel_radius(&self, n: u32) -> f64 {
        let lambda = self.wavelength();
        let d1 = self.d_tx_body;
        let d2 = self.d_body_rx;
        (n as f64 * lambda * d1 * d2 / (d1 + d2)).sqrt()
    }

    /// Phase change caused by a small body displacement Δd (meters).
    ///
    /// The reflected path changes by 2*Δd (there and back), producing
    /// phase change: ΔΦ = 2π * 2Δd / λ
    pub fn phase_change(&self, displacement_m: f64) -> f64 {
        2.0 * PI * 2.0 * displacement_m / self.wavelength()
    }

    /// Expected amplitude variation from chest displacement.
    ///
    /// The signal amplitude varies as |sin(ΔΦ/2)| when the reflection
    /// point crosses Fresnel zone boundaries.
    pub fn expected_amplitude_variation(&self, displacement_m: f64) -> f64 {
        let delta_phi = self.phase_change(displacement_m);
        (delta_phi / 2.0).sin().abs()
    }
}

/// Breathing rate estimation using Fresnel zone model.
#[derive(Debug, Clone)]
pub struct FresnelBreathingEstimator {
    geometry: FresnelGeometry,
    /// Expected chest displacement range (meters) for breathing
    min_displacement: f64,
    max_displacement: f64,
}

impl FresnelBreathingEstimator {
    /// Create estimator with geometry and chest displacement bounds.
    ///
    /// Typical adult chest displacement: 4-12mm (0.004-0.012 m)
    pub fn new(geometry: FresnelGeometry) -> Self {
        Self {
            geometry,
            min_displacement: 0.003,
            max_displacement: 0.015,
        }
    }

    /// Check if observed amplitude variation is consistent with breathing.
    ///
    /// Returns confidence (0.0-1.0) based on whether the observed signal
    /// variation matches the expected Fresnel model prediction for chest
    /// displacements in the breathing range.
    pub fn breathing_confidence(&self, observed_amplitude_variation: f64) -> f64 {
        let min_expected = self.geometry.expected_amplitude_variation(self.min_displacement);
        let max_expected = self.geometry.expected_amplitude_variation(self.max_displacement);

        let (low, high) = if min_expected < max_expected {
            (min_expected, max_expected)
        } else {
            (max_expected, min_expected)
        };

        if observed_amplitude_variation >= low && observed_amplitude_variation <= high {
            // Within expected range: high confidence
            1.0
        } else if observed_amplitude_variation < low {
            // Below range: scale linearly
            (observed_amplitude_variation / low).clamp(0.0, 1.0)
        } else {
            // Above range: could be larger motion (walking), lower confidence for breathing
            (high / observed_amplitude_variation).clamp(0.0, 1.0)
        }
    }

    /// Estimate breathing rate from temporal amplitude signal using the Fresnel model.
    ///
    /// Uses autocorrelation to find periodicity, then validates against
    /// expected Fresnel amplitude range. Returns (rate_bpm, confidence).
    pub fn estimate_breathing_rate(
        &self,
        amplitude_signal: &[f64],
        sample_rate: f64,
    ) -> Result<BreathingEstimate, FresnelError> {
        if amplitude_signal.len() < 10 {
            return Err(FresnelError::InsufficientData {
                needed: 10,
                got: amplitude_signal.len(),
            });
        }
        if sample_rate <= 0.0 {
            return Err(FresnelError::InvalidFrequency);
        }

        // Remove DC (mean)
        let mean: f64 = amplitude_signal.iter().sum::<f64>() / amplitude_signal.len() as f64;
        let centered: Vec<f64> = amplitude_signal.iter().map(|x| x - mean).collect();

        // Autocorrelation to find periodicity
        let n = centered.len();
        let max_lag = (sample_rate * 10.0) as usize; // Up to 10 seconds (6 BPM)
        let min_lag = (sample_rate * 1.5) as usize; // At least 1.5 seconds (40 BPM)
        let max_lag = max_lag.min(n / 2);

        if min_lag >= max_lag {
            return Err(FresnelError::InsufficientData {
                needed: (min_lag * 2 + 1),
                got: n,
            });
        }

        // Compute autocorrelation for breathing-range lags
        let mut best_lag = min_lag;
        let mut best_corr = f64::NEG_INFINITY;
        let norm: f64 = centered.iter().map(|x| x * x).sum();

        if norm < 1e-15 {
            return Err(FresnelError::NoSignal);
        }

        for lag in min_lag..max_lag {
            let mut corr = 0.0;
            for i in 0..(n - lag) {
                corr += centered[i] * centered[i + lag];
            }
            corr /= norm;

            if corr > best_corr {
                best_corr = corr;
                best_lag = lag;
            }
        }

        let period_seconds = best_lag as f64 / sample_rate;
        let rate_bpm = 60.0 / period_seconds;

        // Compute amplitude variation for Fresnel confidence
        let amp_var = amplitude_variation(&centered);
        let fresnel_conf = self.breathing_confidence(amp_var);

        // Autocorrelation quality (>0.3 is good periodicity)
        let autocorr_conf = best_corr.max(0.0).min(1.0);

        let confidence = fresnel_conf * 0.4 + autocorr_conf * 0.6;

        Ok(BreathingEstimate {
            rate_bpm,
            confidence,
            period_seconds,
            autocorrelation_peak: best_corr,
            fresnel_confidence: fresnel_conf,
            amplitude_variation: amp_var,
        })
    }
}

/// Result of breathing rate estimation.
#[derive(Debug, Clone)]
pub struct BreathingEstimate {
    /// Estimated breathing rate in breaths per minute
    pub rate_bpm: f64,
    /// Combined confidence (0.0-1.0)
    pub confidence: f64,
    /// Estimated breathing period in seconds
    pub period_seconds: f64,
    /// Peak autocorrelation value at detected period
    pub autocorrelation_peak: f64,
    /// Confidence from Fresnel model match
    pub fresnel_confidence: f64,
    /// Observed amplitude variation
    pub amplitude_variation: f64,
}

/// Compute peak-to-peak amplitude variation (normalized).
fn amplitude_variation(signal: &[f64]) -> f64 {
    if signal.is_empty() {
        return 0.0;
    }
    let max = signal.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min = signal.iter().cloned().fold(f64::INFINITY, f64::min);
    max - min
}

/// Estimate TX-body and body-RX distances from multi-subcarrier Fresnel observations.
///
/// When exact geometry is unknown, multiple subcarrier wavelengths provide
/// different Fresnel zone crossings for the same chest displacement. This
/// function solves the resulting over-determined system to estimate d1 (TX→body)
/// and d2 (body→RX) distances.
///
/// # Arguments
/// * `observations` - Vec of (wavelength_m, observed_amplitude_variation) from different subcarriers
/// * `d_total` - Known TX-RX straight-line distance in metres
///
/// # Returns
/// Some((d1, d2)) if solvable with ≥3 observations, None otherwise
pub fn solve_fresnel_geometry(
    observations: &[(f32, f32)],
    d_total: f32,
) -> Option<(f32, f32)> {
    let n = observations.len();
    if n < 3 {
        return None;
    }

    // Collect per-wavelength coefficients
    let inv_w_sq_sum: f32 = observations.iter().map(|(w, _)| 1.0 / (w * w)).sum();
    let a_over_w_sum: f32 = observations.iter().map(|(w, a)| a / w).sum();

    // Normal equations for [d1, d2]^T with relative Tikhonov regularization λ=0.5*inv_w_sq_sum.
    // Relative scaling ensures the Jacobi iteration matrix has spectral radius ~0.667,
    // well within the convergence bound required by NeumannSolver.
    // (A^T A + λI) x = A^T b
    // For the linearized system: coefficient[0] = 1/w, coefficient[1] = -1/w
    // So A^T A = [[inv_w_sq_sum, -inv_w_sq_sum], [-inv_w_sq_sum, inv_w_sq_sum]] + λI
    let lambda = 0.5 * inv_w_sq_sum;
    let a00 = inv_w_sq_sum + lambda;
    let a11 = inv_w_sq_sum + lambda;
    let a01 = -inv_w_sq_sum;

    let ata = CsrMatrix::<f32>::from_coo(
        2,
        2,
        vec![(0, 0, a00), (0, 1, a01), (1, 0, a01), (1, 1, a11)],
    );
    let atb = vec![a_over_w_sum, -a_over_w_sum];

    let solver = NeumannSolver::new(1e-5, 300);
    match solver.solve(&ata, &atb) {
        Ok(result) => {
            let d1 = result.solution[0].abs().clamp(0.1, d_total - 0.1);
            let d2 = (d_total - d1).clamp(0.1, d_total - 0.1);
            Some((d1, d2))
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod solver_fresnel_tests {
    use super::*;

    #[test]
    fn fresnel_geometry_insufficient_obs() {
        // < 3 observations → None
        let obs = vec![(0.06_f32, 0.5_f32), (0.05, 0.4)];
        assert!(solve_fresnel_geometry(&obs, 5.0).is_none());
    }

    #[test]
    fn fresnel_geometry_returns_valid_distances() {
        let obs = vec![
            (0.06_f32, 0.3_f32),
            (0.055, 0.25),
            (0.05, 0.35),
            (0.045, 0.2),
        ];
        let result = solve_fresnel_geometry(&obs, 5.0);
        assert!(result.is_some(), "should solve with 4 observations");
        let (d1, d2) = result.unwrap();
        assert!(d1 > 0.0 && d1 < 5.0, "d1={d1} out of range");
        assert!(d2 > 0.0 && d2 < 5.0, "d2={d2} out of range");
        assert!((d1 + d2 - 5.0).abs() < 0.01, "d1+d2 should ≈ d_total");
    }
}

/// Errors from Fresnel computations.
#[derive(Debug, thiserror::Error)]
pub enum FresnelError {
    #[error("Distance must be positive")]
    InvalidDistance,

    #[error("Frequency must be positive")]
    InvalidFrequency,

    #[error("Insufficient data: need {needed}, got {got}")]
    InsufficientData { needed: usize, got: usize },

    #[error("No signal detected (zero variance)")]
    NoSignal,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_geometry() -> FresnelGeometry {
        // TX 3m from body, body 2m from RX, 5 GHz WiFi
        FresnelGeometry::new(3.0, 2.0, 5.0e9).unwrap()
    }

    #[test]
    fn test_wavelength() {
        let g = test_geometry();
        let lambda = g.wavelength();
        assert!((lambda - 0.06).abs() < 0.001); // 5 GHz → 60mm
    }

    #[test]
    fn test_fresnel_radius() {
        let g = test_geometry();
        let f1 = g.fresnel_radius(1);
        // F1 = sqrt(λ * d1 * d2 / (d1 + d2))
        let lambda = g.wavelength(); // actual: 2.998e8 / 5e9 = 0.05996
        let expected = (lambda * 3.0 * 2.0 / 5.0_f64).sqrt();
        assert!((f1 - expected).abs() < 1e-6);
        assert!(f1 > 0.1 && f1 < 0.5); // Reasonable range
    }

    #[test]
    fn test_phase_change_from_displacement() {
        let g = test_geometry();
        // 5mm chest displacement at 5 GHz
        let delta_phi = g.phase_change(0.005);
        // ΔΦ = 2π * 2 * 0.005 / λ
        let lambda = g.wavelength();
        let expected = 2.0 * PI * 2.0 * 0.005 / lambda;
        assert!((delta_phi - expected).abs() < 1e-6);
    }

    #[test]
    fn test_amplitude_variation_breathing_range() {
        let g = test_geometry();
        // 5mm displacement should produce detectable variation
        let var_5mm = g.expected_amplitude_variation(0.005);
        assert!(var_5mm > 0.01, "5mm should produce measurable variation");

        // 10mm should produce more variation
        let var_10mm = g.expected_amplitude_variation(0.010);
        assert!(var_10mm > var_5mm || (var_10mm - var_5mm).abs() < 0.1);
    }

    #[test]
    fn test_breathing_confidence() {
        let g = test_geometry();
        let estimator = FresnelBreathingEstimator::new(g.clone());

        // Signal matching expected breathing range → high confidence
        let expected_var = g.expected_amplitude_variation(0.007);
        let conf = estimator.breathing_confidence(expected_var);
        assert!(conf > 0.5, "Expected breathing variation should give high confidence");

        // Zero variation → low confidence
        let conf_zero = estimator.breathing_confidence(0.0);
        assert!(conf_zero < 0.5);
    }

    #[test]
    fn test_breathing_rate_estimation() {
        let g = test_geometry();
        let estimator = FresnelBreathingEstimator::new(g);

        // Generate 30 seconds of breathing signal at 16 BPM (0.267 Hz)
        let sample_rate = 100.0; // Hz
        let duration = 30.0;
        let n = (sample_rate * duration) as usize;
        let breathing_freq = 0.267; // 16 BPM

        let signal: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / sample_rate;
                0.5 + 0.1 * (2.0 * PI * breathing_freq * t).sin()
            })
            .collect();

        let result = estimator
            .estimate_breathing_rate(&signal, sample_rate)
            .unwrap();

        // Should detect ~16 BPM (within 2 BPM tolerance)
        assert!(
            (result.rate_bpm - 16.0).abs() < 2.0,
            "Expected ~16 BPM, got {:.1}",
            result.rate_bpm
        );
        assert!(result.confidence > 0.3);
        assert!(result.autocorrelation_peak > 0.5);
    }

    #[test]
    fn test_invalid_geometry() {
        assert!(FresnelGeometry::new(-1.0, 2.0, 5e9).is_err());
        assert!(FresnelGeometry::new(1.0, 0.0, 5e9).is_err());
        assert!(FresnelGeometry::new(1.0, 2.0, 0.0).is_err());
    }

    #[test]
    fn test_insufficient_data() {
        let g = test_geometry();
        let estimator = FresnelBreathingEstimator::new(g);
        let short_signal = vec![1.0; 5];
        assert!(matches!(
            estimator.estimate_breathing_rate(&short_signal, 100.0),
            Err(FresnelError::InsufficientData { .. })
        ));
    }
}
