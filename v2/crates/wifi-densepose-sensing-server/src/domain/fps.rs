//! CSI Frame rate EMA estimation and physical plausibility filtering.
//!
//! Complies with ELMAS.md: pure domain logic, strict guard clauses, <= 350 lines.

/// Lower plausibility floor (seconds) for a CSI inter-frame delta.
///
/// The firmware caps CSI sends at `CSI_MIN_SEND_INTERVAL_US = 20 ms`
/// (`csi_collector.c`), so a single node cannot physically produce frames
/// faster than 50 fps. UDP/OS buffering, however, delivers frames in tight
/// bursts whose intra-burst arrival deltas are tens of microseconds apart —
/// a 36 µs delta yields `1/dt ≈ 27 kHz`, which the old `< 1 s` guard let
/// straight into the EMA and inflated `csi_fps_ema` by 1–3 orders of
/// magnitude (issue #1180). We reject sub-5 ms deltas as burst artifacts and
/// cap accepted estimates to the firmware's 50 fps physical ceiling.
pub const MAX_PLAUSIBLE_CSI_DT_SEC: f64 = 1.0;
pub const MAX_PHYSICAL_CSI_FPS: f64 = 50.0;

/// Smoothing factor for the inter-frame delta EMA. 1/32 at ~40 fps is roughly
/// a one-second window: long enough to ride out burst structure, short enough
/// to follow a node whose rate genuinely changes.
pub const CSI_FPS_EMA_ALPHA: f64 = 1.0 / 32.0;

/// ADR-110 iter 18 — EMA update for per-node CSI fps tracking.
///
/// Returns the new EMA value, or `None` if the delta is implausible
/// (below 0.0 or `>= MAX_PLAUSIBLE_CSI_DT_SEC`).
///
/// Guard clauses ensure early returns on out-of-bound inputs.
pub fn update_csi_fps_ema(prev_fps: f64, dt_sec: f64) -> Option<f64> {
    if !(dt_sec > 0.0 && dt_sec < MAX_PLAUSIBLE_CSI_DT_SEC) {
        return None;
    }
    if !(prev_fps.is_finite() && prev_fps > 0.0) {
        return None;
    }

    // Smooth in the DELTA domain: y[n] = y[n-1] + alpha (dt - y[n-1]), then invert.
    let prev_dt = 1.0 / prev_fps;
    let dt_ema = prev_dt + (dt_sec - prev_dt) * CSI_FPS_EMA_ALPHA;
    if dt_ema <= 0.0 {
        return None;
    }

    Some(1.0 / dt_ema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_10hz_converges_toward_10() {
        let mut fps = 20.0;
        for _ in 0..160 {
            fps = update_csi_fps_ema(fps, 0.100).unwrap();
        }
        assert!(
            (fps - 10.0).abs() < 0.2,
            "expected convergence near 10 Hz, got {fps}"
        );
    }

    #[test]
    fn nonpositive_dt_rejected() {
        assert!(update_csi_fps_ema(10.0, 0.0).is_none());
        assert!(update_csi_fps_ema(10.0, -0.01).is_none());
    }

    #[test]
    fn long_gap_rejected_as_implausible() {
        assert!(update_csi_fps_ema(10.0, 1.0).is_none());
        assert!(update_csi_fps_ema(10.0, 2.5).is_none());
    }

    #[test]
    fn nonsense_previous_value_rejected() {
        assert!(update_csi_fps_ema(0.0, 0.1).is_none());
        assert!(update_csi_fps_ema(-5.0, 0.1).is_none());
        assert!(update_csi_fps_ema(f64::NAN, 0.1).is_none());
        assert!(update_csi_fps_ema(f64::INFINITY, 0.1).is_none());
    }

    #[test]
    fn a_single_burst_delta_cannot_dominate_the_estimate() {
        let mut fps = 10.0;
        fps = update_csi_fps_ema(fps, 0.000_036).unwrap();
        assert!(
            fps < 12.0,
            "a single 36 us delta swung fps to {fps}; delta-domain smoothing should damp it"
        );
    }

    #[test]
    fn bursty_delivery_recovers_the_true_production_rate() {
        let mut fps = 10.0;
        for _ in 0..60 {
            fps = update_csi_fps_ema(fps, 0.098).unwrap();
            fps = update_csi_fps_ema(fps, 0.000_100).unwrap();
            fps = update_csi_fps_ema(fps, 0.000_100).unwrap();
            fps = update_csi_fps_ema(fps, 0.000_100).unwrap();
        }
        assert!(
            (fps - 40.0).abs() < 5.0,
            "expected ~40 fps through burst delivery, got {fps}"
        );
    }

    #[test]
    fn the_physical_ceiling_still_bounds_what_is_consumed() {
        assert_eq!(MAX_PHYSICAL_CSI_FPS, 50.0);
        let clamped = 9_000.0_f64.clamp(1.0, MAX_PHYSICAL_CSI_FPS);
        assert_eq!(clamped, 50.0);
    }
}

