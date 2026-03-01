//! Enhanced sensing result value object.
//!
//! The `EnhancedSensingResult` is the output of the signal intelligence
//! pipeline, carrying motion, breathing, posture, and quality metrics
//! derived from multi-BSSID pseudo-CSI data.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// MotionLevel
// ---------------------------------------------------------------------------

/// Coarse classification of detected motion intensity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MotionLevel {
    /// No significant change in BSSID variance; room likely empty.
    None,
    /// Very small fluctuations consistent with a stationary person
    /// (e.g., breathing, minor fidgeting).
    Minimal,
    /// Moderate changes suggesting slow movement (e.g., walking, gesturing).
    Moderate,
    /// Large variance swings indicating vigorous or rapid movement.
    High,
}

impl MotionLevel {
    /// Map a normalised motion score `[0.0, 1.0]` to a `MotionLevel`.
    ///
    /// The thresholds are tuned for multi-BSSID RSSI variance and can be
    /// overridden via `WindowsWifiConfig` in the pipeline layer.
    pub fn from_score(score: f64) -> Self {
        if score < 0.05 {
            Self::None
        } else if score < 0.20 {
            Self::Minimal
        } else if score < 0.60 {
            Self::Moderate
        } else {
            Self::High
        }
    }
}

// ---------------------------------------------------------------------------
// MotionEstimate
// ---------------------------------------------------------------------------

/// Quantitative motion estimate from the multi-BSSID pipeline.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MotionEstimate {
    /// Normalised motion score in `[0.0, 1.0]`.
    pub score: f64,
    /// Coarse classification derived from the score.
    pub level: MotionLevel,
    /// The number of BSSIDs contributing to this estimate.
    pub contributing_bssids: usize,
}

// ---------------------------------------------------------------------------
// BreathingEstimate
// ---------------------------------------------------------------------------

/// Coarse respiratory rate estimate extracted from body-sensitive BSSIDs.
///
/// Only valid when motion level is `Minimal` (person stationary) and at
/// least 3 body-correlated BSSIDs are available. The accuracy is limited
/// by the low sample rate of Tier 1 scanning (~2 Hz).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BreathingEstimate {
    /// Estimated breaths per minute (typical: 12-20 for adults at rest).
    pub rate_bpm: f64,
    /// Confidence in the estimate, `[0.0, 1.0]`.
    pub confidence: f64,
    /// Number of BSSIDs used for the spectral analysis.
    pub bssid_count: usize,
}

// ---------------------------------------------------------------------------
// PostureClass
// ---------------------------------------------------------------------------

/// Coarse posture classification from BSSID fingerprint matching.
///
/// Based on Hopfield template matching of the multi-BSSID amplitude
/// signature against stored reference patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PostureClass {
    /// Room appears empty.
    Empty,
    /// Person standing.
    Standing,
    /// Person sitting.
    Sitting,
    /// Person lying down.
    LyingDown,
    /// Person walking / in motion.
    Walking,
    /// Unknown posture (insufficient confidence).
    Unknown,
}

// ---------------------------------------------------------------------------
// SignalQuality
// ---------------------------------------------------------------------------

/// Signal quality metrics for the current multi-BSSID frame.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SignalQuality {
    /// Overall quality score `[0.0, 1.0]`, where 1.0 is excellent.
    pub score: f64,
    /// Number of BSSIDs in the current frame.
    pub bssid_count: usize,
    /// Spectral gap from the BSSID correlation graph.
    /// A large gap indicates good signal separation.
    pub spectral_gap: f64,
    /// Mean RSSI across all tracked BSSIDs (dBm).
    pub mean_rssi_dbm: f64,
}

// ---------------------------------------------------------------------------
// Verdict
// ---------------------------------------------------------------------------

/// Quality gate verdict from the ruQu three-filter pipeline.
///
/// The pipeline evaluates structural integrity, statistical shift
/// significance, and evidence accumulation before permitting a reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Verdict {
    /// Reading passed all quality gates and is reliable.
    Permit,
    /// Reading shows some anomalies but is usable with reduced confidence.
    Warn,
    /// Reading failed quality checks and should be discarded.
    Deny,
}

// ---------------------------------------------------------------------------
// EnhancedSensingResult
// ---------------------------------------------------------------------------

/// The output of the multi-BSSID signal intelligence pipeline.
///
/// This value object carries all sensing information derived from a single
/// scan cycle. It is converted to a `SensingUpdate` by the Sensing Output
/// bounded context for delivery to the UI.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EnhancedSensingResult {
    /// Motion detection result.
    pub motion: MotionEstimate,
    /// Coarse respiratory rate, if detectable.
    pub breathing: Option<BreathingEstimate>,
    /// Posture classification, if available.
    pub posture: Option<PostureClass>,
    /// Signal quality metrics for the current frame.
    pub signal_quality: SignalQuality,
    /// Number of BSSIDs used in this sensing cycle.
    pub bssid_count: usize,
    /// Quality gate verdict.
    pub verdict: Verdict,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_level_thresholds() {
        assert_eq!(MotionLevel::from_score(0.0), MotionLevel::None);
        assert_eq!(MotionLevel::from_score(0.04), MotionLevel::None);
        assert_eq!(MotionLevel::from_score(0.05), MotionLevel::Minimal);
        assert_eq!(MotionLevel::from_score(0.19), MotionLevel::Minimal);
        assert_eq!(MotionLevel::from_score(0.20), MotionLevel::Moderate);
        assert_eq!(MotionLevel::from_score(0.59), MotionLevel::Moderate);
        assert_eq!(MotionLevel::from_score(0.60), MotionLevel::High);
        assert_eq!(MotionLevel::from_score(1.0), MotionLevel::High);
    }

    #[test]
    fn enhanced_result_construction() {
        let result = EnhancedSensingResult {
            motion: MotionEstimate {
                score: 0.3,
                level: MotionLevel::Moderate,
                contributing_bssids: 10,
            },
            breathing: Some(BreathingEstimate {
                rate_bpm: 16.0,
                confidence: 0.7,
                bssid_count: 5,
            }),
            posture: Some(PostureClass::Standing),
            signal_quality: SignalQuality {
                score: 0.85,
                bssid_count: 15,
                spectral_gap: 0.42,
                mean_rssi_dbm: -65.0,
            },
            bssid_count: 15,
            verdict: Verdict::Permit,
        };

        assert_eq!(result.motion.level, MotionLevel::Moderate);
        assert_eq!(result.verdict, Verdict::Permit);
        assert_eq!(result.bssid_count, 15);
    }
}
