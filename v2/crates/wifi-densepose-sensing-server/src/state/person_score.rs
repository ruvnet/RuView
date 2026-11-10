//! Helper for computing person score from AppStateInner rolling percentiles.

use wifi_densepose_sensing_server::domain::FeatureInfo;
use crate::state::AppStateInner;

pub fn compute_person_score(state: &AppStateInner, feat: &FeatureInfo) -> f64 {
    wifi_densepose_sensing_server::domain::fusion::compute_person_score(
        state.p95_variance.current(),
        state.p95_motion_band_power.current(),
        state.p95_spectral_power.current(),
        feat,
    )
}

