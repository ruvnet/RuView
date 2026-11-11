//! Adaptive multi-antenna MUSIC angle-of-arrival estimation.
//!
//! Computes the MUSIC (Multiple Signal Classification) pseudo-spectrum
//! `P(θ) = 1 / (aᴴ(θ) Eₙ Eₙᴴ a(θ))` from multi-antenna CSI/CFR snapshots of a
//! uniform linear array, with optional forward-backward averaging and spatial
//! smoothing (for coherent multipath).
//!
//! # Aperture gate — no fabricated angles
//! Angle estimation requires spatial aperture. With `M = 1` antenna there is no
//! spatial spectrum to compute, and [`MusicEstimator::estimate`] returns
//! [`AoaError::InsufficientAperture`] so callers fall back to the legacy
//! single-channel pipeline (phase-variance tracking, coarse 2D pose). The
//! high-resolution path unlocks only when real multi-antenna data arrives.
//!
//! # Honest scope
//! This module outputs an **AoA pseudo-spectrum and peak angles** for one
//! array. It does not output positions: going from per-node AoA to (x, y, z)
//! requires multiple calibrated nodes and a separate fusion stage, and accuracy
//! depends on array calibration, aperture, and SNR — none of which this module
//! can guarantee by itself.
//!
//! # No-BLAS design
//! The workspace tests build with `--no-default-features` (no `ndarray-linalg`
//! / BLAS), so the Hermitian eigendecomposition is done in pure Rust: the M×M
//! complex Hermitian covariance `R = X + iY` is embedded as the real symmetric
//! `B = [[X, −Y], [Y, X]]` (2M×2M) and diagonalized with a cyclic Jacobi
//! sweep. For a complex eigenvector `z = u + iv` of `R`, both `(u, v)` and
//! `(−v, u)` are eigenvectors of `B` with the same eigenvalue, and for any
//! steering vector `a` (embedded as `ã = [Re a; Im a]`):
//! `ãᵀ Qₙ ã = aᴴ Eₙ Eₙᴴ a` where `Qₙ` spans the bottom `2(M−K)` real
//! eigenvectors — the noise projector is basis-independent, so eigenvalue
//! degeneracy (equal noise eigenvalues) cannot corrupt the spectrum. This
//! identity and the full pipeline were validated numerically against
//! `numpy.linalg.eigh` before porting.

use num_complex::Complex64;
use thiserror::Error;

/// Errors from MUSIC AoA estimation.
#[derive(Debug, Error)]
pub enum AoaError {
    /// Not enough antennas for a spatial spectrum. Callers must fall back to
    /// the single-channel pipeline — this module never fabricates an angle.
    #[error("Insufficient aperture: MUSIC needs >= 2 antennas, got {got}")]
    InsufficientAperture { got: usize },

    /// More sources requested than the (possibly smoothed) array can resolve.
    #[error("Too many sources: {sources} requested with {antennas} effective antennas")]
    TooManySources { sources: usize, antennas: usize },

    /// Malformed input (empty snapshots, ragged rows, bad config).
    #[error("Bad input: {0}")]
    BadInput(String),
}

/// Configuration for the MUSIC estimator.
#[derive(Debug, Clone)]
pub struct MusicConfig {
    /// Element spacing as a fraction of carrier wavelength (d/λ). 0.5 for a
    /// standard half-wavelength ULA.
    pub d_over_lambda: f64,
    /// Number of sources (K) to resolve. Must be < effective antenna count.
    pub n_sources: usize,
    /// Scan grid start angle (degrees, broadside = 0).
    pub grid_start_deg: f64,
    /// Scan grid stop angle (degrees).
    pub grid_stop_deg: f64,
    /// Scan grid step (degrees).
    pub grid_step_deg: f64,
    /// Forward-backward averaging of the covariance (decorrelates one pair of
    /// coherent paths, improves conditioning). Recommended on.
    pub forward_backward: bool,
    /// Spatial smoothing subarray length L (2..=M). Averages the covariance
    /// over M−L+1 overlapping subarrays to decorrelate coherent multipath, at
    /// the cost of reducing effective aperture to L. `None` disables.
    pub subarray_len: Option<usize>,
}

impl Default for MusicConfig {
    fn default() -> Self {
        Self {
            d_over_lambda: 0.5,
            n_sources: 1,
            grid_start_deg: -90.0,
            grid_stop_deg: 90.0,
            grid_step_deg: 0.5,
            forward_backward: true,
            subarray_len: None,
        }
    }
}

/// Result of a MUSIC scan: the pseudo-spectrum and its peaks.
#[derive(Debug, Clone)]
pub struct AoaSpectrum {
    /// Scan grid (degrees).
    pub grid_deg: Vec<f64>,
    /// MUSIC pseudo-spectrum, one value per grid angle.
    pub spectrum: Vec<f64>,
    /// Top-K peak angles (degrees), ascending.
    pub peaks_deg: Vec<f64>,
    /// Covariance eigenvalues, descending (deduplicated from the real
    /// embedding). The signal/noise gap is a quality diagnostic.
    pub eigenvalues: Vec<f64>,
    /// Effective antenna count after optional spatial smoothing.
    pub effective_antennas: usize,
}

/// MUSIC AoA estimator for a uniform linear array.
#[derive(Debug, Clone)]
pub struct MusicEstimator {
    config: MusicConfig,
}

impl MusicEstimator {
    /// Create an estimator. Validates the configuration.
    pub fn new(config: MusicConfig) -> Result<Self, AoaError> {
        if config.d_over_lambda <= 0.0 {
            return Err(AoaError::BadInput("d_over_lambda must be > 0".into()));
        }
        if config.grid_step_deg <= 0.0 || config.grid_stop_deg <= config.grid_start_deg {
            return Err(AoaError::BadInput("invalid scan grid".into()));
        }
        if config.n_sources == 0 {
            return Err(AoaError::BadInput("n_sources must be >= 1".into()));
        }
        Ok(Self { config })
    }

    /// Estimate the AoA pseudo-spectrum from snapshots.
    ///
    /// `snapshots[t][m]` is the complex CFR sample of antenna `m` at time `t`
    /// (one subcarrier, or one per-subcarrier slice treated as a snapshot).
    ///
    /// Returns [`AoaError::InsufficientAperture`] when `M < 2` — the adaptive
    /// gate that routes single-antenna streams to the legacy pipeline.
    pub fn estimate(&self, snapshots: &[Vec<Complex64>]) -> Result<AoaSpectrum, AoaError> {
        let t = snapshots.len();
        if t == 0 {
            return Err(AoaError::BadInput("no snapshots".into()));
        }
        let m0 = snapshots[0].len();
        if snapshots.iter().any(|row| row.len() != m0) {
            return Err(AoaError::BadInput("ragged snapshot rows".into()));
        }
        // ── THE GATE ────────────────────────────────────────────────────────
        if m0 < 2 {
            return Err(AoaError::InsufficientAperture { got: m0 });
        }

        // Sample covariance R[i][j] = (1/T) Σ_t x[t][i] · conj(x[t][j])
        let mut r = vec![Complex64::new(0.0, 0.0); m0 * m0];
        for row in snapshots {
            for i in 0..m0 {
                for j in 0..m0 {
                    r[i * m0 + j] += row[i] * row[j].conj();
                }
            }
        }
        let tf = t as f64;
        for v in r.iter_mut() {
            *v /= tf;
        }

        // Optional spatial smoothing: average over overlapping subarrays.
        let mut m = m0;
        if let Some(l) = self.config.subarray_len {
            if l < 2 || l > m0 {
                return Err(AoaError::BadInput(format!(
                    "subarray_len {l} out of range 2..={m0}"
                )));
            }
            let p = m0 - l + 1;
            let mut rs = vec![Complex64::new(0.0, 0.0); l * l];
            for s in 0..p {
                for i in 0..l {
                    for j in 0..l {
                        rs[i * l + j] += r[(s + i) * m0 + (s + j)];
                    }
                }
            }
            let pf = p as f64;
            for v in rs.iter_mut() {
                *v /= pf;
            }
            r = rs;
            m = l;
        }

        if self.config.n_sources >= m {
            return Err(AoaError::TooManySources {
                sources: self.config.n_sources,
                antennas: m,
            });
        }

        // Optional forward-backward averaging: R ← (R + J·conj(R)·J)/2,
        // elementwise (J R* J)[i][j] = conj(R[m−1−i][m−1−j]).
        if self.config.forward_backward {
            let mut rfb = vec![Complex64::new(0.0, 0.0); m * m];
            for i in 0..m {
                for j in 0..m {
                    let fb = r[(m - 1 - i) * m + (m - 1 - j)].conj();
                    rfb[i * m + j] = (r[i * m + j] + fb) * 0.5;
                }
            }
            r = rfb;
        }

        // Real symmetric embedding B = [[X, −Y], [Y, X]] of R = X + iY.
        let n = 2 * m;
        let mut b = vec![0.0_f64; n * n];
        for i in 0..m {
            for j in 0..m {
                let x = r[i * m + j].re;
                let y = r[i * m + j].im;
                b[i * n + j] = x;
                b[i * n + (j + m)] = -y;
                b[(i + m) * n + j] = y;
                b[(i + m) * n + (j + m)] = x;
            }
        }

        let (eigvals, eigvecs) = jacobi_eigh(&mut b, n);

        // Sort eigen-indices by eigenvalue, descending.
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &c| {
            eigvals[c]
                .partial_cmp(&eigvals[a])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Noise subspace: all real eigenvectors after the top 2K.
        let k2 = 2 * self.config.n_sources;
        let noise_cols: Vec<usize> = order[k2..].to_vec();

        // Scan the grid: P(θ) = 1 / ‖Qₙᵀ ã(θ)‖².
        let cfg = &self.config;
        let n_grid =
            ((cfg.grid_stop_deg - cfg.grid_start_deg) / cfg.grid_step_deg).floor() as usize + 1;
        let mut grid_deg = Vec::with_capacity(n_grid);
        let mut spectrum = Vec::with_capacity(n_grid);
        let two_pi_d = 2.0 * std::f64::consts::PI * cfg.d_over_lambda;
        for gi in 0..n_grid {
            let theta_deg = cfg.grid_start_deg + gi as f64 * cfg.grid_step_deg;
            let sin_t = theta_deg.to_radians().sin();
            // ã = [Re a; Im a], a_mm = exp(i·2π(d/λ)·mm·sinθ)
            let mut a_tilde = vec![0.0_f64; n];
            for mm in 0..m {
                let ph = two_pi_d * mm as f64 * sin_t;
                a_tilde[mm] = ph.cos();
                a_tilde[mm + m] = ph.sin();
            }
            let mut denom = 0.0_f64;
            for &c in &noise_cols {
                let mut dot = 0.0_f64;
                for row in 0..n {
                    dot += a_tilde[row] * eigvecs[row * n + c];
                }
                denom += dot * dot;
            }
            grid_deg.push(theta_deg);
            spectrum.push(1.0 / denom.max(1e-18));
        }

        // Peaks: local maxima, top K by height, ascending by angle.
        let mut maxima: Vec<usize> = (1..spectrum.len().saturating_sub(1))
            .filter(|&i| spectrum[i] >= spectrum[i - 1] && spectrum[i] >= spectrum[i + 1])
            .collect();
        maxima.sort_by(|&a, &c| {
            spectrum[c]
                .partial_cmp(&spectrum[a])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut peaks_deg: Vec<f64> = maxima
            .into_iter()
            .take(self.config.n_sources)
            .map(|i| grid_deg[i])
            .collect();
        peaks_deg.sort_by(|a, c| a.partial_cmp(c).unwrap_or(std::cmp::Ordering::Equal));

        // Deduplicate paired eigenvalues (each complex eigenvalue appears
        // twice in the real embedding): take every other, descending.
        let eigenvalues: Vec<f64> = order.iter().step_by(2).map(|&i| eigvals[i]).collect();

        Ok(AoaSpectrum {
            grid_deg,
            spectrum,
            peaks_deg,
            eigenvalues,
            effective_antennas: m,
        })
    }
}

/// Cyclic Jacobi eigendecomposition of a real symmetric matrix (flat,
/// row-major, `n × n`, modified in place). Returns `(eigenvalues,
/// eigenvectors)` with eigenvectors as columns of the returned flat matrix.
///
/// O(n³) per sweep; array sizes here are tiny (n = 2M ≤ 16), so this converges
/// in a handful of sweeps. Validated against `numpy.linalg.eigh`.
pub(crate) fn jacobi_eigh(a: &mut [f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    const MAX_SWEEPS: usize = 64;
    const TOL: f64 = 1e-12;

    let mut v = vec![0.0_f64; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }

    for _ in 0..MAX_SWEEPS {
        let mut off = 0.0_f64;
        for p in 0..n.saturating_sub(1) {
            for q in (p + 1)..n {
                off += a[p * n + q] * a[p * n + q];
            }
        }
        if (2.0 * off).sqrt() < TOL {
            break;
        }
        for p in 0..n - 1 {
            for q in (p + 1)..n {
                let apq = a[p * n + q];
                if apq.abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                // Column rotation
                for k in 0..n {
                    let akp = a[k * n + p];
                    let akq = a[k * n + q];
                    a[k * n + p] = c * akp - s * akq;
                    a[k * n + q] = s * akp + c * akq;
                }
                // Row rotation
                for k in 0..n {
                    let apk = a[p * n + k];
                    let aqk = a[q * n + k];
                    a[p * n + k] = c * apk - s * aqk;
                    a[q * n + k] = s * apk + c * aqk;
                }
                // Accumulate eigenvectors
                for k in 0..n {
                    let vkp = v[k * n + p];
                    let vkq = v[k * n + q];
                    v[k * n + p] = c * vkp - s * vkq;
                    v[k * n + q] = s * vkp + c * vkq;
                }
            }
        }
    }

    let w: Vec<f64> = (0..n).map(|i| a[i * n + i]).collect();
    (w, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// xorshift64* — deterministic test noise, no rand dependency.
    struct Rng(u64);
    impl Rng {
        fn next_f64(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
        }
        /// Uniform in [-0.5, 0.5).
        fn centered(&mut self) -> f64 {
            self.next_f64() - 0.5
        }
    }

    fn steering(m: usize, d_over_lambda: f64, theta_deg: f64) -> Vec<Complex64> {
        let sin_t = theta_deg.to_radians().sin();
        (0..m)
            .map(|mm| {
                Complex64::from_polar(
                    1.0,
                    2.0 * std::f64::consts::PI * d_over_lambda * mm as f64 * sin_t,
                )
            })
            .collect()
    }

    /// Snapshots of unit-modulus random-phase sources at given angles + noise.
    fn make_snapshots(
        m: usize,
        t: usize,
        angles_deg: &[f64],
        coherent: bool,
        noise_amp: f64,
        seed: u64,
    ) -> Vec<Vec<Complex64>> {
        let mut rng = Rng(seed);
        let steer: Vec<Vec<Complex64>> =
            angles_deg.iter().map(|&d| steering(m, 0.5, d)).collect();
        (0..t)
            .map(|_| {
                let s0 =
                    Complex64::from_polar(1.0, 2.0 * std::f64::consts::PI * rng.next_f64());
                let sources: Vec<Complex64> = (0..angles_deg.len())
                    .map(|k| {
                        if coherent {
                            s0 * 0.9_f64.powi(k as i32)
                        } else if k == 0 {
                            s0
                        } else {
                            Complex64::from_polar(
                                1.0,
                                2.0 * std::f64::consts::PI * rng.next_f64(),
                            )
                        }
                    })
                    .collect();
                (0..m)
                    .map(|mm| {
                        let mut x = Complex64::new(
                            noise_amp * rng.centered(),
                            noise_amp * rng.centered(),
                        );
                        for (k, s) in sources.iter().enumerate() {
                            x += s * steer[k][mm];
                        }
                        x
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn test_jacobi_known_2x2() {
        // [[2, 1], [1, 2]] has eigenvalues 1 and 3.
        let mut a = vec![2.0, 1.0, 1.0, 2.0];
        let (mut w, v) = jacobi_eigh(&mut a, 2);
        w.sort_by(|x, y| x.partial_cmp(y).unwrap());
        assert!((w[0] - 1.0).abs() < 1e-10 && (w[1] - 3.0).abs() < 1e-10);
        // Eigenvector columns orthonormal.
        let dot = v[0] * v[1] + v[2] * v[3];
        assert!(dot.abs() < 1e-10);
    }

    #[test]
    fn test_two_uncorrelated_sources() {
        let snaps = make_snapshots(6, 400, &[-20.0, 35.0], false, 0.3, 42);
        let est = MusicEstimator::new(MusicConfig {
            n_sources: 2,
            ..Default::default()
        })
        .unwrap();
        let out = est.estimate(&snaps).unwrap();
        assert_eq!(out.peaks_deg.len(), 2);
        assert!(
            (out.peaks_deg[0] - (-20.0)).abs() <= 1.0,
            "peak 0 at {}",
            out.peaks_deg[0]
        );
        assert!(
            (out.peaks_deg[1] - 35.0).abs() <= 1.0,
            "peak 1 at {}",
            out.peaks_deg[1]
        );
        // Signal/noise eigenvalue gap present.
        assert!(out.eigenvalues[1] / out.eigenvalues[2].max(1e-12) > 5.0);
    }

    #[test]
    fn test_coherent_sources_need_smoothing() {
        let snaps = make_snapshots(8, 400, &[-15.0, 25.0], true, 0.15, 7);
        let est = MusicEstimator::new(MusicConfig {
            n_sources: 2,
            subarray_len: Some(5),
            ..Default::default()
        })
        .unwrap();
        let out = est.estimate(&snaps).unwrap();
        assert_eq!(out.effective_antennas, 5);
        assert!(
            (out.peaks_deg[0] - (-15.0)).abs() <= 2.0,
            "peak 0 at {}",
            out.peaks_deg[0]
        );
        assert!(
            (out.peaks_deg[1] - 25.0).abs() <= 2.0,
            "peak 1 at {}",
            out.peaks_deg[1]
        );
    }

    #[test]
    fn test_single_source_various_angles() {
        for &angle in &[0.0, -55.0, 48.0] {
            let snaps = make_snapshots(4, 400, &[angle], false, 0.1, 99);
            let est = MusicEstimator::new(MusicConfig::default()).unwrap();
            let out = est.estimate(&snaps).unwrap();
            assert!(
                (out.peaks_deg[0] - angle).abs() <= 1.0,
                "angle {angle}: est {}",
                out.peaks_deg[0]
            );
        }
    }

    #[test]
    fn test_m1_gate_refuses() {
        // Single antenna: must refuse, never fabricate an angle.
        let snaps: Vec<Vec<Complex64>> = (0..100)
            .map(|i| vec![Complex64::from_polar(1.0, i as f64 * 0.1)])
            .collect();
        let est = MusicEstimator::new(MusicConfig::default()).unwrap();
        match est.estimate(&snaps) {
            Err(AoaError::InsufficientAperture { got: 1 }) => {}
            other => panic!("expected InsufficientAperture, got {other:?}"),
        }
    }

    #[test]
    fn test_too_many_sources() {
        let snaps = make_snapshots(3, 50, &[10.0], false, 0.1, 5);
        let est = MusicEstimator::new(MusicConfig {
            n_sources: 3,
            ..Default::default()
        })
        .unwrap();
        assert!(matches!(
            est.estimate(&snaps),
            Err(AoaError::TooManySources { .. })
        ));
    }

    #[test]
    fn test_bad_inputs() {
        let est = MusicEstimator::new(MusicConfig::default()).unwrap();
        assert!(matches!(est.estimate(&[]), Err(AoaError::BadInput(_))));
        let ragged = vec![
            vec![Complex64::new(1.0, 0.0); 4],
            vec![Complex64::new(1.0, 0.0); 3],
        ];
        assert!(matches!(est.estimate(&ragged), Err(AoaError::BadInput(_))));
        assert!(MusicEstimator::new(MusicConfig {
            n_sources: 0,
            ..Default::default()
        })
        .is_err());
    }
}
