use crate::domain::types::FeatureInfo;

/// Compute composite person score using adaptive or fallback denominators.
///
/// Returns normalized activity score in `[0.0, 1.0]`.
pub fn compute_person_score(
    p95_variance: Option<f64>,
    p95_motion: Option<f64>,
    p95_spectral: Option<f64>,
    feat: &FeatureInfo,
) -> f64 {
    let var_denom = p95_variance.map(|p| p.max(50.0)).unwrap_or(300.0);
    let motion_denom = p95_motion.map(|p| p.max(50.0)).unwrap_or(250.0);
    let sp_denom = p95_spectral.map(|p| p.max(100.0)).unwrap_or(500.0);

    let var_norm = (feat.variance / var_denom).clamp(0.0, 1.0);
    let cp_norm = (feat.change_points as f64 / 30.0).clamp(0.0, 1.0);
    let motion_norm = (feat.motion_band_power / motion_denom).clamp(0.0, 1.0);
    let sp_norm = (feat.spectral_power / sp_denom).clamp(0.0, 1.0);

    var_norm * 0.40 + cp_norm * 0.20 + motion_norm * 0.25 + sp_norm * 0.15
}

