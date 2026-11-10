//! Fixed encoding of the sensing server's per-node features (ADR-426 D3).
//!
//! The room learner's frozen AETHER features are not on `main` yet, so replay
//! and shadow runs on `main` use what the server already computes per node.
//! The encoding is a fixed, stateless squash: nothing is fitted, so a head is
//! valid for any row encoded under [`NODE_FEATURE_SCHEMA`], and a change to
//! this function must change the schema id.

/// Schema id bound into a head's [`crate::HeadContext`].
pub const NODE_FEATURE_SCHEMA: &str = "ruview.node-features/1";
/// Number of encoded features.
pub const NODE_FEATURE_COUNT: usize = 8;

/// The server's per-node feature snapshot, as broadcast in `node_features[]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeFeatures {
    pub mean_rssi: f64,
    pub variance: f64,
    pub motion_band_power: f64,
    pub breathing_band_power: f64,
    pub dominant_freq_hz: f64,
    pub change_points: f64,
    pub spectral_power: f64,
    pub rssi_dbm: f64,
}

/// `ln(1 + max(x, 0))`, capped, scaled to roughly `[0, 1]`.
fn log_squash(x: f64) -> f32 {
    (x.max(0.0).ln_1p().min(12.0) / 12.0) as f32
}

/// dBm in `[-100, 0]` mapped to `[0, 1]`.
fn dbm(x: f64) -> f32 {
    ((x.clamp(-100.0, 0.0) + 100.0) / 100.0) as f32
}

/// Encode one snapshot. Returns `None` when any input is not finite, so a
/// broken reading is skipped rather than scored.
pub fn encode(f: &NodeFeatures) -> Option<[f32; NODE_FEATURE_COUNT]> {
    let raw = [
        f.mean_rssi,
        f.variance,
        f.motion_band_power,
        f.breathing_band_power,
        f.dominant_freq_hz,
        f.change_points,
        f.spectral_power,
        f.rssi_dbm,
    ];
    if !raw.iter().all(|v| v.is_finite()) {
        return None;
    }
    Some([
        dbm(f.mean_rssi),
        log_squash(f.variance),
        log_squash(f.motion_band_power),
        log_squash(f.breathing_band_power),
        (f.dominant_freq_hz.clamp(0.0, 5.0) / 5.0) as f32,
        log_squash(f.change_points),
        log_squash(f.spectral_power),
        dbm(f.rssi_dbm),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> NodeFeatures {
        NodeFeatures {
            mean_rssi: -62.0,
            variance: 3.5,
            motion_band_power: 120.0,
            breathing_band_power: 0.4,
            dominant_freq_hz: 0.25,
            change_points: 3.0,
            spectral_power: 9_000.0,
            rssi_dbm: -60.0,
        }
    }

    #[test]
    fn encoding_is_bounded_and_fixed() {
        let x = encode(&sample()).unwrap();
        assert!(x.iter().all(|v| (0.0..=1.0).contains(v)), "{x:?}");
        assert_eq!(x[0], 0.38);
        assert_eq!(x[7], 0.40);
        assert_eq!(x[4], 0.05);
    }

    #[test]
    fn extremes_saturate_and_non_finite_is_refused() {
        let mut f = sample();
        f.spectral_power = 1e300;
        f.variance = -5.0;
        f.mean_rssi = 40.0;
        let x = encode(&f).unwrap();
        assert_eq!((x[6], x[1], x[0]), (1.0, 0.0, 1.0));
        f.motion_band_power = f64::NAN;
        assert!(encode(&f).is_none());
    }
}
