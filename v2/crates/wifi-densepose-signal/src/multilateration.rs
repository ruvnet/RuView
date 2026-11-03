//! Range-only multilateration: node positions + target ranges → position.
//!
//! Solves `min Σᵢ (‖p − nᵢ‖ − rᵢ)²` with a closed-form linearized
//! initialization (pairwise difference of sphere equations) refined by
//! damped Gauss-Newton, using the crate's pure-Rust Cholesky solver
//! (no BLAS — builds under `--no-default-features`).
//!
//! # Honest scope
//! This solver turns **ranges into positions**; it cannot make the ranges
//! good. ESP32-class hardware has no fine ToF timestamping, so ranges derived
//! from RSSI path-loss models carry errors of *meters* in multipath indoor
//! environments — the output position inherits that error, and callers must
//! surface [`PositionEstimate::rms_residual_m`] rather than present the point
//! as exact. Measured solver behaviour (validated numerically before porting):
//! exact recovery at zero range noise; with σ = 0.1 m range noise and 6 nodes
//! spanning ~16 m horizontally but only ~2.4 m vertically, horizontal p95
//! error ≈ 0.15 m while 3D p95 ≈ 0.6 m — **vertical DOP dominates for flat
//! (near-coplanar) node arrays**, which is why coplanar arrays must use
//! [`solve_2d`] instead of getting a fabricated z.
//!
//! # Degeneracy is refused, not fudged
//! - Fewer than `dims + 1` nodes → [`MlatError::InsufficientNodes`].
//! - Node geometry that does not span the solve dimensions (collinear nodes in
//!   a 3D solve, coplanar-z nodes in a 3D solve — the mirror-ambiguous case) →
//!   [`MlatError::DegenerateGeometry`]. The solver never silently picks one of
//!   two mirror solutions.

use thiserror::Error;

use crate::aoa_music::jacobi_eigh;
use crate::ruvsense::tomography::cholesky_solve;

/// Errors from multilateration.
#[derive(Debug, Error)]
pub enum MlatError {
    /// Not enough nodes for the requested solve dimensionality.
    #[error("Insufficient nodes: need >= {needed} for a {dims}D solve, got {got}")]
    InsufficientNodes {
        needed: usize,
        got: usize,
        dims: usize,
    },

    /// Node geometry cannot resolve a unique position (collinear / coplanar).
    #[error("Degenerate node geometry: {0}")]
    DegenerateGeometry(String),

    /// Ranges and nodes disagree in length, or a range is not finite/positive.
    #[error("Bad input: {0}")]
    BadInput(String),
}

/// Solver configuration.
#[derive(Debug, Clone)]
pub struct MlatConfig {
    /// Gauss-Newton iteration cap.
    pub max_iterations: usize,
    /// Step-norm convergence tolerance (metres).
    pub tolerance: f64,
    /// Levenberg damping added to the normal matrix diagonal.
    pub damping: f64,
    /// Relative eigenvalue threshold below which node geometry is declared
    /// degenerate (smallest/largest spread eigenvalue).
    pub rank_rel_tol: f64,
}

impl Default for MlatConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            tolerance: 1e-10,
            damping: 1e-9,
            rank_rel_tol: 1e-9,
        }
    }
}

/// A solved position with quality diagnostics.
#[derive(Debug, Clone, Copy)]
pub struct PositionEstimate {
    /// Estimated position (z = 0.0 for 2D solves).
    pub position: [f64; 3],
    /// RMS of per-node range residuals (metres) at the solution — the honest
    /// quality figure to surface alongside the point.
    pub rms_residual_m: f64,
    /// Gauss-Newton iterations used.
    pub iterations: usize,
}

/// Solve for (x, y, z) from >= 4 nodes with 3D geometric spread.
pub fn solve_3d(
    nodes: &[[f64; 3]],
    ranges: &[f64],
    config: &MlatConfig,
) -> Result<PositionEstimate, MlatError> {
    solve(nodes, ranges, 3, config)
}

/// Solve for (x, y) from >= 3 nodes with 2D geometric spread. Node z values
/// are ignored; ranges should be horizontal (or accepted as approximate when
/// target and nodes are at similar heights). This is the correct mode for
/// near-coplanar node arrays, which a 3D solve rejects as mirror-ambiguous.
pub fn solve_2d(
    nodes: &[[f64; 3]],
    ranges: &[f64],
    config: &MlatConfig,
) -> Result<PositionEstimate, MlatError> {
    solve(nodes, ranges, 2, config)
}

fn solve(
    nodes: &[[f64; 3]],
    ranges: &[f64],
    dims: usize,
    config: &MlatConfig,
) -> Result<PositionEstimate, MlatError> {
    let n = nodes.len();
    let needed = dims + 1;
    if n < needed {
        return Err(MlatError::InsufficientNodes {
            needed,
            got: n,
            dims,
        });
    }
    if ranges.len() != n {
        return Err(MlatError::BadInput(format!(
            "{} nodes but {} ranges",
            n,
            ranges.len()
        )));
    }
    if ranges.iter().any(|r| !r.is_finite() || *r < 0.0) {
        return Err(MlatError::BadInput("non-finite or negative range".into()));
    }

    // Geometry rank check: the node spread covariance must span `dims`
    // dimensions, else the position is not unique (mirror/line ambiguity).
    let mut centroid = [0.0_f64; 3];
    for node in nodes {
        for d in 0..dims {
            centroid[d] += node[d];
        }
    }
    for c in centroid.iter_mut().take(dims) {
        *c /= n as f64;
    }
    let mut cov = vec![0.0_f64; dims * dims];
    for node in nodes {
        for i in 0..dims {
            for j in 0..dims {
                cov[i * dims + j] += (node[i] - centroid[i]) * (node[j] - centroid[j]);
            }
        }
    }
    let (eigvals, _) = jacobi_eigh(&mut cov, dims);
    let ev_max = eigvals.iter().cloned().fold(f64::MIN, f64::max);
    let ev_min = eigvals.iter().cloned().fold(f64::MAX, f64::min);
    if ev_min < config.rank_rel_tol * ev_max.max(1e-12) {
        return Err(MlatError::DegenerateGeometry(format!(
            "node spread rank-deficient in {dims}D (eig min/max = {ev_min:.3e}/{ev_max:.3e})"
        )));
    }

    // Closed-form init: subtract node-0 sphere equation from each other's,
    // giving the linear system 2(nᵢ − n₀)ᵀ p = r₀² − rᵢ² + ‖nᵢ‖² − ‖n₀‖².
    let n0 = &nodes[0];
    let r0 = ranges[0];
    let sq = |v: &[f64; 3]| -> f64 { v[..dims].iter().map(|x| x * x).sum() };
    let mut ata = vec![0.0_f64; dims * dims];
    let mut atb = vec![0.0_f64; dims];
    for i in 1..n {
        let ni = &nodes[i];
        let mut row = [0.0_f64; 3];
        for d in 0..dims {
            row[d] = 2.0 * (ni[d] - n0[d]);
        }
        let b = r0 * r0 - ranges[i] * ranges[i] + sq(ni) - sq(n0);
        for a in 0..dims {
            atb[a] += row[a] * b;
            for c in 0..dims {
                ata[a * dims + c] += row[a] * row[c];
            }
        }
    }
    for d in 0..dims {
        ata[d * dims + d] += config.damping;
    }
    let p0 = cholesky_solve(&mut ata, &atb, dims).ok_or_else(|| {
        MlatError::DegenerateGeometry("linearized init not solvable".into())
    })?;

    // Damped Gauss-Newton refinement of min Σ (‖p − nᵢ‖ − rᵢ)².
    let mut p = [0.0_f64; 3];
    p[..dims].copy_from_slice(&p0[..dims]);
    let mut rms = 0.0_f64;
    let mut iterations = 0;
    for iter in 0..config.max_iterations {
        let mut jtj = vec![0.0_f64; dims * dims];
        let mut jtf = vec![0.0_f64; dims];
        let mut sum_f2 = 0.0_f64;
        for (node, &r) in nodes.iter().zip(ranges.iter()) {
            let mut d = [0.0_f64; 3];
            for k in 0..dims {
                d[k] = p[k] - node[k];
            }
            let dist = d[..dims]
                .iter()
                .map(|x| x * x)
                .sum::<f64>()
                .sqrt()
                .max(1e-12);
            let f = dist - r;
            sum_f2 += f * f;
            for a in 0..dims {
                let ja = d[a] / dist;
                jtf[a] += ja * f;
                for c in 0..dims {
                    jtj[a * dims + c] += ja * (d[c] / dist);
                }
            }
        }
        rms = (sum_f2 / n as f64).sqrt();
        for k in 0..dims {
            jtj[k * dims + k] += config.damping;
        }
        let neg_jtf: Vec<f64> = jtf.iter().map(|v| -v).collect();
        let step = cholesky_solve(&mut jtj, &neg_jtf, dims).ok_or_else(|| {
            MlatError::DegenerateGeometry("Gauss-Newton normal matrix breakdown".into())
        })?;
        for k in 0..dims {
            p[k] += step[k];
        }
        iterations = iter + 1;
        let step_norm = step.iter().map(|s| s * s).sum::<f64>().sqrt();
        if step_norm < config.tolerance {
            break;
        }
    }

    Ok(PositionEstimate {
        position: p,
        rms_residual_m: rms,
        iterations,
    })
}

/// Exponential (alpha) smoothing filter for a tracked 3D position — the
/// operational anti-jitter stage between raw per-frame solves and display.
/// Measured in the numeric proof: at α = 0.35 it roughly halves mean tracking
/// error against σ = 0.2 m per-frame position noise on a slowly moving target.
#[derive(Debug, Clone)]
pub struct AlphaFilter3 {
    alpha: f64,
    state: Option<[f64; 3]>,
}

impl AlphaFilter3 {
    /// `alpha` in (0, 1]: higher tracks faster, lower smooths harder.
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha: alpha.clamp(1e-6, 1.0),
            state: None,
        }
    }

    /// Feed a raw position, get the smoothed one.
    pub fn update(&mut self, raw: [f64; 3]) -> [f64; 3] {
        let s = match self.state {
            None => raw,
            Some(prev) => {
                let mut s = prev;
                for k in 0..3 {
                    s[k] += self.alpha * (raw[k] - prev[k]);
                }
                s
            }
        };
        self.state = Some(s);
        s
    }

    /// Clear the filter (e.g. after a track loss).
    pub fn reset(&mut self) {
        self.state = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NODES4: [[f64; 3]; 4] = [
        [1.2, 0.5, 1.0],
        [18.5, 2.1, 1.2],
        [9.0, 14.2, 2.5],
        [3.0, 12.0, 0.4],
    ];

    fn dist(a: &[f64; 3], b: &[f64; 3], dims: usize) -> f64 {
        (0..dims).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()
    }

    #[test]
    fn test_3d_exact_recovery() {
        let target = [7.5, 6.2, 1.4];
        let ranges: Vec<f64> = NODES4.iter().map(|n| dist(&target, n, 3)).collect();
        let est = solve_3d(&NODES4, &ranges, &MlatConfig::default()).unwrap();
        for k in 0..3 {
            assert!(
                (est.position[k] - target[k]).abs() < 1e-8,
                "axis {k}: {} vs {}",
                est.position[k],
                target[k]
            );
        }
        assert!(est.rms_residual_m < 1e-8);
    }

    #[test]
    fn test_2d_exact_recovery() {
        let target = [10.0, 7.0, 0.0];
        let ranges: Vec<f64> = NODES4[..3].iter().map(|n| dist(&target, n, 2)).collect();
        let est = solve_2d(&NODES4[..3], &ranges, &MlatConfig::default()).unwrap();
        assert!((est.position[0] - 10.0).abs() < 1e-8);
        assert!((est.position[1] - 7.0).abs() < 1e-8);
        assert_eq!(est.position[2], 0.0);
    }

    #[test]
    fn test_noisy_ranges_bounded_error() {
        // xorshift noise, 50 trials: horizontal p95 must stay < 0.3 m at
        // sigma ~ 0.1 m (mirrors the numeric proof, which measured 0.152 m).
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut rand_pm = || {
            // sum of 4 uniforms - 2 => approx zero-mean, sigma ~= 0.577/... scaled below
            let mut s = 0.0;
            for _ in 0..4 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                s += (state >> 11) as f64 / (1u64 << 53) as f64;
            }
            (s - 2.0) * 0.173 // approx sigma 0.1
        };
        let nodes: Vec<[f64; 3]> = NODES4
            .iter()
            .cloned()
            .chain([[15.0, 12.0, 2.0], [2.0, 3.0, 2.8]])
            .collect();
        let target = [7.5, 6.2, 1.4];
        let mut errs: Vec<f64> = (0..50)
            .map(|_| {
                let ranges: Vec<f64> = nodes
                    .iter()
                    .map(|n| dist(&target, n, 3) + rand_pm())
                    .collect();
                let est = solve_3d(&nodes, &ranges, &MlatConfig::default()).unwrap();
                ((est.position[0] - target[0]).powi(2) + (est.position[1] - target[1]).powi(2))
                    .sqrt()
            })
            .collect();
        errs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p95 = errs[47];
        assert!(p95 < 0.3, "horizontal p95 {p95}");
    }

    #[test]
    fn test_insufficient_nodes_refused() {
        let ranges = [5.0, 6.0];
        assert!(matches!(
            solve_3d(&NODES4[..2], &ranges, &MlatConfig::default()),
            Err(MlatError::InsufficientNodes { needed: 4, .. })
        ));
        assert!(matches!(
            solve_2d(&NODES4[..2], &ranges, &MlatConfig::default()),
            Err(MlatError::InsufficientNodes { needed: 3, .. })
        ));
    }

    #[test]
    fn test_collinear_nodes_refused() {
        let nodes: Vec<[f64; 3]> = (0..4).map(|i| [i as f64 * 2.0, i as f64 * 2.0, 1.0]).collect();
        let target = [7.5, 6.2, 1.4];
        let ranges: Vec<f64> = nodes.iter().map(|n| dist(&target, n, 3)).collect();
        assert!(matches!(
            solve_3d(&nodes, &ranges, &MlatConfig::default()),
            Err(MlatError::DegenerateGeometry(_))
        ));
    }

    #[test]
    fn test_coplanar_z_refused_in_3d_but_fine_in_2d() {
        // All nodes at z = 1.0: mirror-ambiguous in 3D, must refuse.
        let nodes: Vec<[f64; 3]> = NODES4.iter().map(|n| [n[0], n[1], 1.0]).collect();
        let target = [7.5, 6.2, 1.4];
        let r3: Vec<f64> = nodes.iter().map(|n| dist(&target, n, 3)).collect();
        assert!(matches!(
            solve_3d(&nodes, &r3, &MlatConfig::default()),
            Err(MlatError::DegenerateGeometry(_))
        ));
        // Same array is fine as a 2D solve with horizontal ranges.
        let r2: Vec<f64> = nodes.iter().map(|n| dist(&target, n, 2)).collect();
        let est = solve_2d(&nodes, &r2, &MlatConfig::default()).unwrap();
        assert!((est.position[0] - 7.5).abs() < 1e-8);
        assert!((est.position[1] - 6.2).abs() < 1e-8);
    }

    #[test]
    fn test_bad_inputs() {
        let ranges3 = [1.0, 2.0, 3.0];
        assert!(matches!(
            solve_3d(&NODES4, &ranges3, &MlatConfig::default()),
            Err(MlatError::BadInput(_))
        ));
        let bad = [1.0, f64::NAN, 3.0, 4.0];
        assert!(matches!(
            solve_3d(&NODES4, &bad, &MlatConfig::default()),
            Err(MlatError::BadInput(_))
        ));
    }

    #[test]
    fn test_alpha_filter_smooths() {
        let mut f = AlphaFilter3::new(0.35);
        // First sample passes through.
        assert_eq!(f.update([1.0, 2.0, 3.0]), [1.0, 2.0, 3.0]);
        // A step input converges monotonically toward the new value.
        let mut last = f.update([2.0, 2.0, 3.0]);
        assert!((last[0] - 1.35).abs() < 1e-12);
        for _ in 0..50 {
            last = f.update([2.0, 2.0, 3.0]);
        }
        assert!((last[0] - 2.0).abs() < 1e-6);
        f.reset();
        assert_eq!(f.update([9.0, 9.0, 9.0]), [9.0, 9.0, 9.0]);
    }
}
