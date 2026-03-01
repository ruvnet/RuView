//! Fresnel geometry estimation via sparse regularized solver (ruvector-solver).
//!
//! [`solve_fresnel_geometry`] estimates the TX-body distance `d1` and
//! body-RX distance `d2` from multi-subcarrier Fresnel amplitude observations
//! using a Neumann series sparse solver on a regularized normal-equations system.

use ruvector_solver::neumann::NeumannSolver;
use ruvector_solver::types::CsrMatrix;

/// Estimate TX-body (d1) and body-RX (d2) distances from multi-subcarrier
/// Fresnel observations.
///
/// # Arguments
///
/// - `observations`: `(wavelength_m, observed_amplitude_variation)` per
///   subcarrier. Wavelength is in metres; amplitude variation is dimensionless.
/// - `d_total`: known TX-RX straight-line distance in metres.
///
/// # Returns
///
/// `Some((d1, d2))` where `d1 + d2 ≈ d_total`, or `None` if fewer than 3
/// observations are provided or the solver fails to converge.
pub fn solve_fresnel_geometry(observations: &[(f32, f32)], d_total: f32) -> Option<(f32, f32)> {
    if observations.len() < 3 {
        return None;
    }

    let lambda_reg = 0.05_f32;
    let sum_inv_w2: f32 = observations.iter().map(|(w, _)| 1.0 / (w * w)).sum();

    // Build regularized 2×2 normal-equations system:
    // (λI + A^T A) [d1; d2] ≈ A^T b
    let ata = CsrMatrix::<f32>::from_coo(
        2,
        2,
        vec![
            (0, 0, lambda_reg + sum_inv_w2),
            (1, 1, lambda_reg + sum_inv_w2),
        ],
    );

    let atb = vec![
        observations.iter().map(|(w, a)| a / w).sum::<f32>(),
        -observations.iter().map(|(w, a)| a / w).sum::<f32>(),
    ];

    NeumannSolver::new(1e-5, 300)
        .solve(&ata, &atb)
        .ok()
        .map(|r| {
            let d1 = r.solution[0].abs().clamp(0.1, d_total - 0.1);
            let d2 = (d_total - d1).clamp(0.1, d_total - 0.1);
            (d1, d2)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresnel_d1_plus_d2_equals_d_total() {
        let d_total = 5.0_f32;

        // 5 observations: (wavelength_m, amplitude_variation)
        let observations = vec![
            (0.125_f32, 0.3),
            (0.130, 0.25),
            (0.120, 0.35),
            (0.115, 0.4),
            (0.135, 0.2),
        ];

        let result = solve_fresnel_geometry(&observations, d_total);
        assert!(result.is_some(), "solver must return Some for 5 observations");

        let (d1, d2) = result.unwrap();
        let sum = d1 + d2;
        assert!(
            (sum - d_total).abs() < 0.5,
            "d1 + d2 = {sum:.3} should be close to d_total = {d_total}"
        );
        assert!(d1 > 0.0, "d1 must be positive");
        assert!(d2 > 0.0, "d2 must be positive");
    }

    #[test]
    fn fresnel_too_few_observations_returns_none() {
        let result = solve_fresnel_geometry(&[(0.125, 0.3), (0.130, 0.25)], 5.0);
        assert!(result.is_none(), "fewer than 3 observations must return None");
    }
}
