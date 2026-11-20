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

/// Schema id of the windowed encoding produced by [`WindowEncoder`].
pub const WINDOW_FEATURE_SCHEMA: &str = "ruview.node-features/2";
/// Two statistics for each of the eight base features.
pub const WINDOW_FEATURE_COUNT: usize = 2 * NODE_FEATURE_COUNT;
/// Length of the window the statistics are taken over.
pub const WINDOW_MS: i64 = 30_000;
/// A gap this long between observations restarts the window.
pub const WINDOW_GAP_RESET_MS: i64 = 5_000;
/// No output until the window holds this many observations over this span.
pub const WINDOW_MIN_SAMPLES: usize = 20;
pub const WINDOW_MIN_SPAN_MS: i64 = 15_000;
const WINDOW_CAPACITY: usize = 1024;
/// Variability of a `[0, 1]` feature is small; this maps it onto `[0, 1]`.
const VARIABILITY_GAIN: f32 = 10.0;

/// Drift-resistant encoding of one node's recent history (ADR-426 D6).
///
/// The base features are absolute levels (RSSI, band powers), and levels
/// drift over tens of minutes. This encoder keeps the last [`WINDOW_MS`] of
/// base vectors and emits, per base feature, how much it *varied*: the
/// standard deviation over the window, then the mean absolute step between
/// consecutive observations. Neither depends on the level itself.
///
/// One encoder per node. It is deterministic and has no fitted state.
#[derive(Debug, Clone, Default)]
pub struct WindowEncoder {
    history: std::collections::VecDeque<(i64, [f32; NODE_FEATURE_COUNT])>,
}

impl WindowEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one base vector. Returns the windowed features once the window is
    /// warm; `None` while it is filling, after a gap, or for a timestamp that
    /// does not move forward.
    pub fn push(
        &mut self,
        timestamp_ms: i64,
        base: [f32; NODE_FEATURE_COUNT],
    ) -> Option<[f32; WINDOW_FEATURE_COUNT]> {
        if let Some(&(last, _)) = self.history.back() {
            if timestamp_ms <= last {
                return None;
            }
            if timestamp_ms - last > WINDOW_GAP_RESET_MS {
                self.history.clear();
            }
        }
        self.history.push_back((timestamp_ms, base));
        while self
            .history
            .front()
            .is_some_and(|&(t, _)| timestamp_ms - t > WINDOW_MS)
            || self.history.len() > WINDOW_CAPACITY
        {
            self.history.pop_front();
        }
        let n = self.history.len();
        let span = timestamp_ms - self.history.front()?.0;
        if n < WINDOW_MIN_SAMPLES || span < WINDOW_MIN_SPAN_MS {
            return None;
        }
        let mut out = [0.0f32; WINDOW_FEATURE_COUNT];
        for i in 0..NODE_FEATURE_COUNT {
            let mean = self
                .history
                .iter()
                .map(|(_, v)| f64::from(v[i]))
                .sum::<f64>()
                / n as f64;
            let var = self
                .history
                .iter()
                .map(|(_, v)| (f64::from(v[i]) - mean).powi(2))
                .sum::<f64>()
                / n as f64;
            let steps = self
                .history
                .iter()
                .zip(self.history.iter().skip(1))
                .map(|((_, a), (_, b))| f64::from((b[i] - a[i]).abs()))
                .sum::<f64>()
                / (n - 1) as f64;
            out[i] = (var.sqrt() as f32 * VARIABILITY_GAIN).min(1.0);
            out[NODE_FEATURE_COUNT + i] = (steps as f32 * VARIABILITY_GAIN).min(1.0);
        }
        Some(out)
    }
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
    fn window_encoder_reports_variability_not_level() {
        let run = |level: f32, wobble: f32| {
            let mut enc = WindowEncoder::new();
            let mut last = None;
            for k in 0..100i64 {
                let mut base = [level; NODE_FEATURE_COUNT];
                base[0] += if k % 2 == 0 { wobble } else { -wobble };
                last = enc.push(k * 300, base).or(last);
            }
            last.unwrap()
        };
        // Same wobble at two different levels: the same output, to rounding.
        for (a, b) in run(0.2, 0.01).iter().zip(run(0.7, 0.01)) {
            assert!((a - b).abs() < 1e-4, "{a} vs {b}");
        }
        let steady = run(0.5, 0.0);
        assert!(steady.iter().all(|v| *v == 0.0));
        let busy = run(0.5, 0.02);
        // std of +-0.02 is 0.02 -> 0.2; mean step is 0.04 -> 0.4.
        assert!((busy[0] - 0.2).abs() < 1e-3 && (busy[NODE_FEATURE_COUNT] - 0.4).abs() < 1e-3);
        assert!(busy[1..NODE_FEATURE_COUNT].iter().all(|v| *v == 0.0));
    }

    #[test]
    fn window_encoder_warms_up_resets_on_gaps_and_refuses_stale_time() {
        let mut enc = WindowEncoder::new();
        let base = [0.5; NODE_FEATURE_COUNT];
        let mut first = None;
        for k in 0..80i64 {
            if enc.push(k * 300, base).is_some() && first.is_none() {
                first = Some(k);
            }
        }
        // 15 s of span at 300 ms spacing.
        assert_eq!(first, Some(50));
        assert!(enc.push(79 * 300, base).is_none(), "time must move forward");
        // A 6 s gap empties the window: warm-up starts again.
        assert!(enc.push(79 * 300 + 6_000, base).is_none());
        // Old observations leave the window.
        let mut enc = WindowEncoder::new();
        for k in 0..400i64 {
            enc.push(k * 300, base);
        }
        assert!(enc.history.len() <= 101);
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
