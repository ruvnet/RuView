//! # `ruview-survey` — node self-survey, check mode (ADR-381)
//!
//! **Check mode only. Warn-only. This crate never writes node positions.**
//!
//! `--node-positions` is typed by hand and keyed by node id (#1804, #1866).
//! A swapped id, positions entered in feet, or a node moved after setup all
//! degrade multistatic fusion without any error. This crate takes pairwise
//! node-to-node ranges ([`RangeObservation`], from Wi-Fi FTM today and UWB
//! later) and the configured positions, and reports:
//!
//! - a per-link residual (measured minus configured distance) with a
//!   tolerance and a status ([`LinkResidual`]);
//! - a fitted scale, and a unit-mismatch finding when the scale sits near a
//!   feet/inches/centimetre factor ([`Finding::UnitMismatch`]);
//! - a suggested id swap when a different assignment of the configured
//!   positions to node ids fits much better ([`Finding::IdSwapSuggested`]),
//!   or an explicit ambiguity when the layout is symmetric;
//! - a node whose links disagree as a group ([`Finding::NodeMoved`]).
//!
//! Ranges see only distances, so any rotation, reflection or translation of
//! the layout fits equally well. Check mode compares distances, not
//! coordinates, and reports symmetric ambiguity rather than guessing.
//!
//! ## Evidence
//!
//! Default tolerances for FTM ([`CheckParams::ftm_uncalibrated`]) come from a
//! MEASURED two-board ESP32-S3 bench on 2026-10-04 (20 MHz, channel 1, 32
//! frames per session). Within one responder start the spread was ±0.2 to
//! 0.4 m, but the bias moved from −0.5 to +1.9 m between responder starts at
//! 1.14 m, a 3.71 m link read 7.8 m, and readings clamp to 0 below about
//! 0.8 m. That is good enough to catch gross configuration mistakes and not
//! good enough to place nodes. Fill mode (writing solved positions) is gated
//! off by [`fill_gate`] until a source has a MEASURED p90 link error of at
//! most [`FILL_MAX_P90_M`].
//!
//! ## Determinism and bounds
//!
//! No I/O, no clock, no randomness. Inputs are bounded ([`MAX_NODES`],
//! [`MAX_OBSERVATIONS`], [`MAX_RANGE_M`]); the id-swap search runs only up to
//! [`MAX_SWAP_NODES`] nodes. Malformed input yields a typed [`SurveyError`],
//! never a panic.
//!
//! ```
//! use ruview_survey::*;
//!
//! let positions = parse_node_positions("1:0,0,1;2:4,0,1;3:0,3,1").unwrap();
//! let obs = [
//!     RangeObservation::ftm(1, 2, 4.2, 0.3),
//!     RangeObservation::ftm(1, 3, 3.1, 0.3),
//!     RangeObservation::ftm(2, 3, 5.4, 0.3),
//! ];
//! let report = check(&positions, &obs, &CheckParams::ftm_uncalibrated()).unwrap();
//! assert_eq!(report.verdict, Verdict::Consistent);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod calibration;
mod check;
mod fill;
mod observation;
mod positions;
mod report;

pub use calibration::CalibrationTable;
pub use check::{check, MAX_SWAP_NODES};
pub use fill::{fill_gate, EvidenceTag, FillBlocked, SourceEvidence, FILL_MAX_P90_M};
pub use observation::{RangeMethod, RangeObservation, MAX_OBSERVATIONS, MAX_RANGE_M};
pub use positions::{parse_node_positions, NodePositions, MAX_NODES};
pub use report::{CheckParams, CheckReport, Finding, LinkResidual, LinkStatus, ScaleFit, Verdict};

/// Errors from malformed survey input.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SurveyError {
    /// A range observation failed validation.
    #[error("invalid range observation: {0}")]
    InvalidObservation(String),
    /// The node position string failed to parse.
    #[error("invalid node positions: {0}")]
    InvalidPositions(String),
    /// An input exceeded its bound.
    #[error("too many {what}: {got} > {max}")]
    TooMany {
        /// What overflowed.
        what: &'static str,
        /// How many were supplied.
        got: usize,
        /// The bound.
        max: usize,
    },
    /// A calibration reference was unusable.
    #[error("calibration: {0}")]
    Calibration(String),
}

/// Median of a non-empty slice. Returns `None` for an empty slice.
pub(crate) fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let mid = v.len() / 2;
    Some(if v.len() % 2 == 0 {
        (v[mid - 1] + v[mid]) / 2.0
    } else {
        v[mid]
    })
}

#[cfg(test)]
mod tests {
    use super::median;

    #[test]
    fn median_handles_odd_even_and_empty() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
    }
}
