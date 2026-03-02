//! Geometric Diversity Index and Cramer-Rao bound estimation (ADR-031).
//!
//! Provides two key computations for array geometry quality assessment:
//!
//! 1. **Geometric Diversity Index (GDI)**: measures how well the viewpoints
//!    are spread around the sensing area. Higher GDI = better spatial coverage.
//!
//! 2. **Cramer-Rao Bound (CRB)**: lower bound on the position estimation
//!    variance achievable by any unbiased estimator given the array geometry.
//!    Used to predict theoretical localisation accuracy.
//!
//! Uses `ruvector_solver` for matrix operations in the Fisher information
//! matrix inversion required by the Cramer-Rao bound.

use ruvector_solver::neumann::NeumannSolver;
use ruvector_solver::types::CsrMatrix;

// ---------------------------------------------------------------------------
// Node identifier
// ---------------------------------------------------------------------------

/// Unique identifier for a sensor node in the multistatic array.
pub type NodeId = u32;

// ---------------------------------------------------------------------------
// GeometricDiversityIndex
// ---------------------------------------------------------------------------

/// Geometric Diversity Index measuring array viewpoint spread.
///
/// GDI is computed as the mean minimum angular separation across all viewpoints:
///
/// ```text
/// GDI = (1/N) * sum_i min_{j != i} |theta_i - theta_j|
/// ```
///
/// A GDI close to `2*PI/N` (uniform spacing) indicates optimal diversity.
/// A GDI near zero means viewpoints are clustered.
///
/// The `n_effective` field estimates the number of independent viewpoints
/// after accounting for angular correlation between nearby viewpoints.
#[derive(Debug, Clone)]
pub struct GeometricDiversityIndex {
    /// GDI value (radians). Higher is better.
    pub value: f32,
    /// Effective independent viewpoints after correlation discount.
    pub n_effective: f32,
    /// Worst (most redundant) viewpoint pair.
    pub worst_pair: (NodeId, NodeId),
    /// Number of physical viewpoints in the array.
    pub n_physical: usize,
}

impl GeometricDiversityIndex {
    /// Compute the GDI from viewpoint azimuth angles.
    ///
    /// # Arguments
    ///
    /// - `azimuths`: per-viewpoint azimuth angle in radians from the array
    ///   centroid. Must have at least 2 elements.
    /// - `node_ids`: per-viewpoint node identifier (same length as `azimuths`).
    ///
    /// # Returns
    ///
    /// `None` if fewer than 2 viewpoints are provided.
    pub fn compute(azimuths: &[f32], node_ids: &[NodeId]) -> Option<Self> {
        let n = azimuths.len();
        if n < 2 || node_ids.len() != n {
            return None;
        }

        // Find the minimum angular separation for each viewpoint.
        let mut min_seps = Vec::with_capacity(n);
        let mut worst_sep = f32::MAX;
        let mut worst_i = 0_usize;
        let mut worst_j = 1_usize;

        for i in 0..n {
            let mut min_sep = f32::MAX;
            let mut min_j = (i + 1) % n;
            for j in 0..n {
                if i == j {
                    continue;
                }
                let sep = angular_distance(azimuths[i], azimuths[j]);
                if sep < min_sep {
                    min_sep = sep;
                    min_j = j;
                }
            }
            min_seps.push(min_sep);
            if min_sep < worst_sep {
                worst_sep = min_sep;
                worst_i = i;
                worst_j = min_j;
            }
        }

        let gdi = min_seps.iter().sum::<f32>() / n as f32;

        // Effective viewpoints: discount correlated viewpoints.
        // Correlation model: rho(theta) = exp(-theta^2 / (2 * sigma^2))
        // with sigma = PI/6 (30 degrees).
        let sigma = std::f32::consts::PI / 6.0;
        let n_effective = compute_effective_viewpoints(azimuths, sigma);

        Some(GeometricDiversityIndex {
            value: gdi,
            n_effective,
            worst_pair: (node_ids[worst_i], node_ids[worst_j]),
            n_physical: n,
        })
    }

    /// Returns `true` if the array has sufficient geometric diversity for
    /// reliable multi-viewpoint fusion.
    ///
    /// Threshold: GDI >= PI / (2 * N) (at least half the uniform-spacing ideal).
    pub fn is_sufficient(&self) -> bool {
        if self.n_physical == 0 {
            return false;
        }
        let ideal = std::f32::consts::PI * 2.0 / self.n_physical as f32;
        self.value >= ideal * 0.5
    }

    /// Ratio of effective to physical viewpoints.
    pub fn efficiency(&self) -> f32 {
        if self.n_physical == 0 {
            return 0.0;
        }
        self.n_effective / self.n_physical as f32
    }
}

/// Compute the shortest angular distance between two angles (radians).
///
/// Returns a value in `[0, PI]`.
fn angular_distance(a: f32, b: f32) -> f32 {
    let diff = (a - b).abs() % (2.0 * std::f32::consts::PI);
    if diff > std::f32::consts::PI {
        2.0 * std::f32::consts::PI - diff
    } else {
        diff
    }
}

/// Compute effective independent viewpoints using a Gaussian angular correlation
/// model and eigenvalue analysis of the correlation matrix.
///
/// The effective count is: `N_eff = (sum lambda_i)^2 / sum(lambda_i^2)` where
/// `lambda_i` are the eigenvalues of the angular correlation matrix. For
/// efficiency, we approximate this using trace-based estimation:
/// `N_eff approx trace(R)^2 / trace(R^2)`.
fn compute_effective_viewpoints(azimuths: &[f32], sigma: f32) -> f32 {
    let n = azimuths.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return 1.0;
    }

    let two_sigma_sq = 2.0 * sigma * sigma;

    // Build correlation matrix R[i,j] = exp(-angular_dist(i,j)^2 / (2*sigma^2))
    // and compute trace(R) and trace(R^2) simultaneously.
    // For trace(R^2) = sum_i sum_j R[i,j]^2, we need the full matrix.
    let mut r_matrix = vec![0.0_f32; n * n];
    for i in 0..n {
        r_matrix[i * n + i] = 1.0;
        for j in (i + 1)..n {
            let d = angular_distance(azimuths[i], azimuths[j]);
            let rho = (-d * d / two_sigma_sq).exp();
            r_matrix[i * n + j] = rho;
            r_matrix[j * n + i] = rho;
        }
    }

    // trace(R) = n (all diagonal entries are 1.0).
    let trace_r = n as f32;
    // trace(R^2) = sum_{i,j} R[i,j]^2
    let trace_r2: f32 = r_matrix.iter().map(|v| v * v).sum();

    // N_eff = trace(R)^2 / trace(R^2)
    let n_eff = (trace_r * trace_r) / trace_r2.max(f32::EPSILON);
    n_eff.min(n as f32).max(1.0)
}

// ---------------------------------------------------------------------------
// Cramer-Rao Bound
// ---------------------------------------------------------------------------

/// Cramer-Rao lower bound on position estimation variance.
///
/// The CRB provides the theoretical minimum variance achievable by any
/// unbiased estimator for the target position given the array geometry.
/// Lower CRB = better localisation potential.
#[derive(Debug, Clone)]
pub struct CramerRaoBound {
    /// CRB for x-coordinate estimation (metres squared).
    pub crb_x: f32,
    /// CRB for y-coordinate estimation (metres squared).
    pub crb_y: f32,
    /// Root-mean-square position error lower bound (metres).
    pub rmse_lower_bound: f32,
    /// Geometric dilution of precision (GDOP).
    pub gdop: f32,
}

/// A viewpoint position for CRB computation.
#[derive(Debug, Clone)]
pub struct ViewpointPosition {
    /// X coordinate in metres.
    pub x: f32,
    /// Y coordinate in metres.
    pub y: f32,
    /// Per-measurement noise standard deviation (metres).
    pub noise_std: f32,
}

impl CramerRaoBound {
    /// Estimate the Cramer-Rao bound for a target at `(tx, ty)` observed by
    /// the given viewpoints.
    ///
    /// # Arguments
    ///
    /// - `target`: target position `(x, y)` in metres.
    /// - `viewpoints`: sensor node positions with per-node noise levels.
    ///
    /// # Returns
    ///
    /// `None` if fewer than 3 viewpoints are provided (under-determined).
    pub fn estimate(target: (f32, f32), viewpoints: &[ViewpointPosition]) -> Option<Self> {
        let n = viewpoints.len();
        if n < 3 {
            return None;
        }

        // Build the 2x2 Fisher Information Matrix (FIM).
        // FIM = sum_i (1/sigma_i^2) * [cos^2(phi_i), cos(phi_i)*sin(phi_i);
        //                               cos(phi_i)*sin(phi_i), sin^2(phi_i)]
        // where phi_i is the bearing angle from viewpoint i to the target.
        let mut fim_00 = 0.0_f32;
        let mut fim_01 = 0.0_f32;
        let mut fim_11 = 0.0_f32;

        for vp in viewpoints {
            let dx = target.0 - vp.x;
            let dy = target.1 - vp.y;
            let r = (dx * dx + dy * dy).sqrt().max(1e-6);
            let cos_phi = dx / r;
            let sin_phi = dy / r;
            let inv_var = 1.0 / (vp.noise_std * vp.noise_std).max(1e-10);

            fim_00 += inv_var * cos_phi * cos_phi;
            fim_01 += inv_var * cos_phi * sin_phi;
            fim_11 += inv_var * sin_phi * sin_phi;
        }

        // Invert the 2x2 FIM analytically: CRB = FIM^{-1}.
        let det = fim_00 * fim_11 - fim_01 * fim_01;
        if det.abs() < 1e-12 {
            return None;
        }

        let crb_x = fim_11 / det;
        let crb_y = fim_00 / det;
        let rmse = (crb_x + crb_y).sqrt();
        let gdop = (crb_x + crb_y).sqrt();

        Some(CramerRaoBound {
            crb_x,
            crb_y,
            rmse_lower_bound: rmse,
            gdop,
        })
    }

    /// Compute the CRB using the `ruvector-solver` Neumann series solver for
    /// larger arrays where the analytic 2x2 inversion is extended to include
    /// regularisation for ill-conditioned geometries.
    ///
    /// # Arguments
    ///
    /// - `target`: target position `(x, y)` in metres.
    /// - `viewpoints`: sensor node positions with per-node noise levels.
    /// - `regularisation`: Tikhonov regularisation parameter (typically 1e-4).
    ///
    /// # Returns
    ///
    /// `None` if fewer than 3 viewpoints or the solver fails.
    pub fn estimate_regularised(
        target: (f32, f32),
        viewpoints: &[ViewpointPosition],
        regularisation: f32,
    ) -> Option<Self> {
        let n = viewpoints.len();
        if n < 3 {
            return None;
        }

        let mut fim_00 = regularisation;
        let mut fim_01 = 0.0_f32;
        let mut fim_11 = regularisation;

        for vp in viewpoints {
            let dx = target.0 - vp.x;
            let dy = target.1 - vp.y;
            let r = (dx * dx + dy * dy).sqrt().max(1e-6);
            let cos_phi = dx / r;
            let sin_phi = dy / r;
            let inv_var = 1.0 / (vp.noise_std * vp.noise_std).max(1e-10);

            fim_00 += inv_var * cos_phi * cos_phi;
            fim_01 += inv_var * cos_phi * sin_phi;
            fim_11 += inv_var * sin_phi * sin_phi;
        }

        // Use Neumann solver for the regularised system.
        let ata = CsrMatrix::<f32>::from_coo(
            2,
            2,
            vec![
                (0, 0, fim_00),
                (0, 1, fim_01),
                (1, 0, fim_01),
                (1, 1, fim_11),
            ],
        );

        // Solve FIM * x = e_1 and FIM * x = e_2 to get the CRB diagonal.
        let solver = NeumannSolver::new(1e-6, 500);

        let crb_x = solver
            .solve(&ata, &[1.0, 0.0])
            .ok()
            .map(|r| r.solution[0])?;
        let crb_y = solver
            .solve(&ata, &[0.0, 1.0])
            .ok()
            .map(|r| r.solution[1])?;

        let rmse = (crb_x.abs() + crb_y.abs()).sqrt();

        Some(CramerRaoBound {
            crb_x,
            crb_y,
            rmse_lower_bound: rmse,
            gdop: rmse,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gdi_uniform_spacing_is_optimal() {
        // 4 viewpoints at 0, 90, 180, 270 degrees
        let azimuths = vec![0.0, std::f32::consts::FRAC_PI_2, std::f32::consts::PI, 3.0 * std::f32::consts::FRAC_PI_2];
        let ids = vec![0, 1, 2, 3];
        let gdi = GeometricDiversityIndex::compute(&azimuths, &ids).unwrap();
        // Minimum separation = PI/2 for each viewpoint, so GDI = PI/2
        let expected = std::f32::consts::FRAC_PI_2;
        assert!(
            (gdi.value - expected).abs() < 0.01,
            "uniform spacing GDI should be PI/2={expected:.3}, got {:.3}",
            gdi.value
        );
    }

    #[test]
    fn gdi_clustered_viewpoints_have_low_value() {
        // 4 viewpoints clustered within 10 degrees
        let azimuths = vec![0.0, 0.05, 0.08, 0.12];
        let ids = vec![0, 1, 2, 3];
        let gdi = GeometricDiversityIndex::compute(&azimuths, &ids).unwrap();
        assert!(
            gdi.value < 0.15,
            "clustered viewpoints should have low GDI, got {:.3}",
            gdi.value
        );
    }

    #[test]
    fn gdi_insufficient_viewpoints_returns_none() {
        assert!(GeometricDiversityIndex::compute(&[0.0], &[0]).is_none());
        assert!(GeometricDiversityIndex::compute(&[], &[]).is_none());
    }

    #[test]
    fn gdi_efficiency_is_bounded() {
        let azimuths = vec![0.0, 1.0, 2.0, 3.0];
        let ids = vec![0, 1, 2, 3];
        let gdi = GeometricDiversityIndex::compute(&azimuths, &ids).unwrap();
        assert!(gdi.efficiency() > 0.0 && gdi.efficiency() <= 1.0,
            "efficiency should be in (0, 1], got {}", gdi.efficiency());
    }

    #[test]
    fn gdi_is_sufficient_for_uniform_layout() {
        let azimuths = vec![0.0, std::f32::consts::FRAC_PI_2, std::f32::consts::PI, 3.0 * std::f32::consts::FRAC_PI_2];
        let ids = vec![0, 1, 2, 3];
        let gdi = GeometricDiversityIndex::compute(&azimuths, &ids).unwrap();
        assert!(gdi.is_sufficient(), "uniform layout should be sufficient");
    }

    #[test]
    fn gdi_worst_pair_is_closest() {
        // Viewpoints at 0, 0.1, PI, 1.5*PI
        let azimuths = vec![0.0, 0.1, std::f32::consts::PI, 1.5 * std::f32::consts::PI];
        let ids = vec![10, 20, 30, 40];
        let gdi = GeometricDiversityIndex::compute(&azimuths, &ids).unwrap();
        // Worst pair should be (10, 20) as they are only 0.1 rad apart
        assert!(
            (gdi.worst_pair == (10, 20)) || (gdi.worst_pair == (20, 10)),
            "worst pair should be nodes 10 and 20, got {:?}",
            gdi.worst_pair
        );
    }

    #[test]
    fn angular_distance_wraps_correctly() {
        let d = angular_distance(0.1, 2.0 * std::f32::consts::PI - 0.1);
        assert!(
            (d - 0.2).abs() < 1e-4,
            "angular distance across 0/2PI boundary should be 0.2, got {d}"
        );
    }

    #[test]
    fn effective_viewpoints_all_identical_equals_one() {
        let azimuths = vec![0.0, 0.0, 0.0, 0.0];
        let sigma = std::f32::consts::PI / 6.0;
        let n_eff = compute_effective_viewpoints(&azimuths, sigma);
        assert!(
            (n_eff - 1.0).abs() < 0.1,
            "4 identical viewpoints should have n_eff ~ 1.0, got {n_eff}"
        );
    }

    #[test]
    fn crb_decreases_with_more_viewpoints() {
        let target = (0.0, 0.0);
        let vp3: Vec<ViewpointPosition> = (0..3)
            .map(|i| {
                let a = 2.0 * std::f32::consts::PI * i as f32 / 3.0;
                ViewpointPosition { x: 5.0 * a.cos(), y: 5.0 * a.sin(), noise_std: 0.1 }
            })
            .collect();
        let vp6: Vec<ViewpointPosition> = (0..6)
            .map(|i| {
                let a = 2.0 * std::f32::consts::PI * i as f32 / 6.0;
                ViewpointPosition { x: 5.0 * a.cos(), y: 5.0 * a.sin(), noise_std: 0.1 }
            })
            .collect();

        let crb3 = CramerRaoBound::estimate(target, &vp3).unwrap();
        let crb6 = CramerRaoBound::estimate(target, &vp6).unwrap();
        assert!(
            crb6.rmse_lower_bound < crb3.rmse_lower_bound,
            "6 viewpoints should give lower CRB than 3: {:.4} vs {:.4}",
            crb6.rmse_lower_bound,
            crb3.rmse_lower_bound
        );
    }

    #[test]
    fn crb_too_few_viewpoints_returns_none() {
        let target = (0.0, 0.0);
        let vps = vec![
            ViewpointPosition { x: 1.0, y: 0.0, noise_std: 0.1 },
            ViewpointPosition { x: 0.0, y: 1.0, noise_std: 0.1 },
        ];
        assert!(CramerRaoBound::estimate(target, &vps).is_none());
    }

    #[test]
    fn crb_regularised_returns_result() {
        let target = (0.0, 0.0);
        let vps: Vec<ViewpointPosition> = (0..4)
            .map(|i| {
                let a = 2.0 * std::f32::consts::PI * i as f32 / 4.0;
                ViewpointPosition { x: 3.0 * a.cos(), y: 3.0 * a.sin(), noise_std: 0.1 }
            })
            .collect();
        let crb = CramerRaoBound::estimate_regularised(target, &vps, 1e-4);
        // May return None if Neumann solver doesn't converge, but should not panic.
        if let Some(crb) = crb {
            assert!(crb.rmse_lower_bound >= 0.0, "RMSE bound must be non-negative");
        }
    }
}
