//! Vital sign domain types (ADR-021).

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Status of a vital sign measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum VitalStatus {
    /// Valid measurement with clinical-grade confidence.
    Valid,
    /// Measurement present but with reduced confidence.
    Degraded,
    /// Measurement unreliable (e.g., single RSSI source).
    Unreliable,
    /// No measurement possible.
    Unavailable,
}

/// A single vital sign estimate.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VitalEstimate {
    /// Estimated value in BPM (beats/breaths per minute).
    pub value_bpm: f64,
    /// Confidence in the estimate [0.0, 1.0].
    pub confidence: f64,
    /// Measurement status.
    pub status: VitalStatus,
}

/// Combined vital sign reading.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VitalReading {
    /// Respiratory rate estimate.
    pub respiratory_rate: VitalEstimate,
    /// Heart rate estimate.
    pub heart_rate: VitalEstimate,
    /// Number of subcarriers used.
    pub subcarrier_count: usize,
    /// Signal quality score [0.0, 1.0].
    pub signal_quality: f64,
    /// Timestamp (seconds since epoch).
    pub timestamp_secs: f64,
}

/// Input frame for the vital sign pipeline.
#[derive(Debug, Clone)]
pub struct CsiFrame {
    /// Per-subcarrier amplitudes.
    pub amplitudes: Vec<f64>,
    /// Per-subcarrier phases (radians).
    pub phases: Vec<f64>,
    /// Number of subcarriers.
    pub n_subcarriers: usize,
    /// Sample index (monotonically increasing).
    pub sample_index: u64,
    /// Sample rate in Hz.
    pub sample_rate_hz: f64,
}

impl CsiFrame {
    /// Create a new CSI frame, validating that amplitude and phase
    /// vectors match the declared subcarrier count.
    ///
    /// Returns `None` if the lengths are inconsistent.
    pub fn new(
        amplitudes: Vec<f64>,
        phases: Vec<f64>,
        n_subcarriers: usize,
        sample_index: u64,
        sample_rate_hz: f64,
    ) -> Option<Self> {
        if amplitudes.len() != n_subcarriers || phases.len() != n_subcarriers {
            return None;
        }
        Some(Self {
            amplitudes,
            phases,
            n_subcarriers,
            sample_index,
            sample_rate_hz,
        })
    }
}

impl VitalEstimate {
    /// Create an unavailable estimate (no measurement possible).
    pub fn unavailable() -> Self {
        Self {
            value_bpm: 0.0,
            confidence: 0.0,
            status: VitalStatus::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vital_status_equality() {
        assert_eq!(VitalStatus::Valid, VitalStatus::Valid);
        assert_ne!(VitalStatus::Valid, VitalStatus::Degraded);
    }

    #[test]
    fn vital_estimate_unavailable() {
        let est = VitalEstimate::unavailable();
        assert_eq!(est.status, VitalStatus::Unavailable);
        assert!((est.value_bpm - 0.0).abs() < f64::EPSILON);
        assert!((est.confidence - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn csi_frame_new_valid() {
        let frame = CsiFrame::new(
            vec![1.0, 2.0, 3.0],
            vec![0.1, 0.2, 0.3],
            3,
            0,
            100.0,
        );
        assert!(frame.is_some());
        let f = frame.unwrap();
        assert_eq!(f.n_subcarriers, 3);
        assert_eq!(f.amplitudes.len(), 3);
    }

    #[test]
    fn csi_frame_new_mismatched_lengths() {
        let frame = CsiFrame::new(
            vec![1.0, 2.0],
            vec![0.1, 0.2, 0.3],
            3,
            0,
            100.0,
        );
        assert!(frame.is_none());
    }

    #[test]
    fn csi_frame_clone() {
        let frame = CsiFrame::new(vec![1.0], vec![0.5], 1, 42, 50.0).unwrap();
        let cloned = frame.clone();
        assert_eq!(cloned.sample_index, 42);
        assert_eq!(cloned.n_subcarriers, 1);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn vital_reading_serde_roundtrip() {
        let reading = VitalReading {
            respiratory_rate: VitalEstimate {
                value_bpm: 15.0,
                confidence: 0.9,
                status: VitalStatus::Valid,
            },
            heart_rate: VitalEstimate {
                value_bpm: 72.0,
                confidence: 0.85,
                status: VitalStatus::Valid,
            },
            subcarrier_count: 56,
            signal_quality: 0.92,
            timestamp_secs: 1_700_000_000.0,
        };
        let json = serde_json::to_string(&reading).unwrap();
        let parsed: VitalReading = serde_json::from_str(&json).unwrap();
        assert!((parsed.heart_rate.value_bpm - 72.0).abs() < f64::EPSILON);
    }
}
