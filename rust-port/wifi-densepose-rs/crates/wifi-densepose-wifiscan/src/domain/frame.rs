//! Multi-AP frame value object.
//!
//! A `MultiApFrame` is a snapshot of all BSSID observations at a single point
//! in time. It serves as the input to the signal intelligence pipeline
//! (Bounded Context 2 in ADR-022), providing the multi-dimensional
//! pseudo-CSI data that replaces the single-RSSI approach.

use std::collections::VecDeque;
use std::time::Instant;

/// A snapshot of all tracked BSSIDs at a single point in time.
///
/// This value object is produced by [`BssidRegistry::to_multi_ap_frame`] and
/// consumed by the signal intelligence pipeline. Each index `i` in the
/// vectors corresponds to the `i`-th entry in the registry's subcarrier map.
///
/// [`BssidRegistry::to_multi_ap_frame`]: crate::domain::registry::BssidRegistry::to_multi_ap_frame
#[derive(Debug, Clone)]
pub struct MultiApFrame {
    /// Number of BSSIDs (pseudo-subcarriers) in this frame.
    pub bssid_count: usize,

    /// RSSI values in dBm, one per BSSID.
    ///
    /// Index matches the subcarrier map ordering.
    pub rssi_dbm: Vec<f64>,

    /// Linear amplitudes derived from RSSI via `10^((rssi + 100) / 20)`.
    ///
    /// This maps -100 dBm to amplitude 1.0, providing a scale that is
    /// compatible with the downstream attention and correlation stages.
    pub amplitudes: Vec<f64>,

    /// Pseudo-phase values derived from channel numbers.
    ///
    /// Encoded as `(channel / 48) * pi`, giving a value in `[0, pi]`.
    /// This is a heuristic that provides spatial diversity information
    /// to pipeline stages that expect phase data.
    pub phases: Vec<f64>,

    /// Per-BSSID RSSI variance (Welford), one per BSSID.
    ///
    /// High variance indicates a BSSID whose signal is modulated by body
    /// movement; low variance indicates a static background AP.
    pub per_bssid_variance: Vec<f64>,

    /// Per-BSSID RSSI history (ring buffer), one per BSSID.
    ///
    /// Used by the spatial correlator and breathing extractor to compute
    /// cross-correlation and spectral features.
    pub histories: Vec<VecDeque<f64>>,

    /// Estimated effective sample rate in Hz.
    ///
    /// Tier 1 (netsh): approximately 2 Hz.
    /// Tier 2 (wlanapi): approximately 10-20 Hz.
    pub sample_rate_hz: f64,

    /// When this frame was constructed.
    pub timestamp: Instant,
}

impl MultiApFrame {
    /// Whether this frame has enough BSSIDs for multi-AP sensing.
    ///
    /// The `min_bssids` parameter comes from `WindowsWifiConfig::min_bssids`.
    pub fn is_sufficient(&self, min_bssids: usize) -> bool {
        self.bssid_count >= min_bssids
    }

    /// The maximum amplitude across all BSSIDs. Returns 0.0 for empty frames.
    pub fn max_amplitude(&self) -> f64 {
        self.amplitudes
            .iter()
            .copied()
            .fold(0.0_f64, f64::max)
    }

    /// The mean RSSI across all BSSIDs in dBm. Returns `f64::NEG_INFINITY`
    /// for empty frames.
    pub fn mean_rssi(&self) -> f64 {
        if self.rssi_dbm.is_empty() {
            return f64::NEG_INFINITY;
        }
        let sum: f64 = self.rssi_dbm.iter().sum();
        sum / self.rssi_dbm.len() as f64
    }

    /// The total variance across all BSSIDs (sum of per-BSSID variances).
    ///
    /// Higher values indicate more environmental change, which correlates
    /// with human presence and movement.
    pub fn total_variance(&self) -> f64 {
        self.per_bssid_variance.iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(bssid_count: usize, rssi_values: &[f64]) -> MultiApFrame {
        let amplitudes: Vec<f64> = rssi_values
            .iter()
            .map(|&r| 10.0_f64.powf((r + 100.0) / 20.0))
            .collect();
        MultiApFrame {
            bssid_count,
            rssi_dbm: rssi_values.to_vec(),
            amplitudes,
            phases: vec![0.0; bssid_count],
            per_bssid_variance: vec![0.1; bssid_count],
            histories: vec![VecDeque::new(); bssid_count],
            sample_rate_hz: 2.0,
            timestamp: Instant::now(),
        }
    }

    #[test]
    fn is_sufficient_checks_threshold() {
        let frame = make_frame(5, &[-60.0, -65.0, -70.0, -75.0, -80.0]);
        assert!(frame.is_sufficient(3));
        assert!(frame.is_sufficient(5));
        assert!(!frame.is_sufficient(6));
    }

    #[test]
    fn mean_rssi_calculation() {
        let frame = make_frame(3, &[-60.0, -70.0, -80.0]);
        assert!((frame.mean_rssi() - (-70.0)).abs() < 1e-9);
    }

    #[test]
    fn empty_frame_handles_gracefully() {
        let frame = make_frame(0, &[]);
        assert_eq!(frame.max_amplitude(), 0.0);
        assert!(frame.mean_rssi().is_infinite());
        assert_eq!(frame.total_variance(), 0.0);
        assert!(!frame.is_sufficient(1));
    }

    #[test]
    fn total_variance_sums_per_bssid() {
        let mut frame = make_frame(3, &[-60.0, -70.0, -80.0]);
        frame.per_bssid_variance = vec![0.1, 0.2, 0.3];
        assert!((frame.total_variance() - 0.6).abs() < 1e-9);
    }
}
