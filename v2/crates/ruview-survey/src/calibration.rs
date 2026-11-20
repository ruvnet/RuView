//! Per-boot range calibration.
//!
//! On the 2026-10-04 FTM bench the spread inside one responder start was
//! ±0.2 to 0.4 m with no drift beyond that over two minutes (40 sessions,
//! half medians 3.30 and 3.00 m), but the bias changed with
//! every responder start (−0.5 to +1.9 m at 1.14 m). So an offset is only
//! trusted for the link and responder boot it was measured on. An
//! observation from any other boot passes through uncalibrated.
//!
//! The reference must be a known non-trivial distance. Zero-separation
//! calibration (ESP-IDF's `cm0`) is suspect on this hardware because
//! readings clamped to 0 below about 0.8 m.

use std::collections::BTreeMap;

use crate::{median, RangeObservation, SurveyError};

/// Shortest reference distance accepted, in metres. Below this the bench
/// readings clamped to zero, so an offset measured there is meaningless.
pub const MIN_REFERENCE_M: f64 = 1.0;

/// Offsets keyed by `(initiator, responder, responder_boot)`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CalibrationTable {
    offsets: BTreeMap<(u8, u8, u32), f64>,
}

impl CalibrationTable {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an offset from sessions taken at a known reference distance.
    ///
    /// Every observation must share initiator, responder and responder boot.
    /// The offset is `median(range_m) - reference_m` and is returned.
    pub fn record_reference(
        &mut self,
        observations: &[RangeObservation],
        reference_m: f64,
    ) -> Result<f64, SurveyError> {
        if !reference_m.is_finite() || reference_m < MIN_REFERENCE_M {
            return Err(SurveyError::Calibration(format!(
                "reference {reference_m} m is below {MIN_REFERENCE_M} m"
            )));
        }
        let first = observations
            .first()
            .ok_or_else(|| SurveyError::Calibration("no reference observations".into()))?;
        let boot = first.responder_boot.ok_or_else(|| {
            SurveyError::Calibration("reference observations carry no responder boot".into())
        })?;
        let key = (first.initiator, first.responder, boot);
        let mut ranges = Vec::with_capacity(observations.len());
        for o in observations {
            o.validate()?;
            if (o.initiator, o.responder, o.responder_boot) != (key.0, key.1, Some(key.2)) {
                return Err(SurveyError::Calibration(
                    "reference observations mix links or responder boots".into(),
                ));
            }
            ranges.push(o.range_m);
        }
        let offset = median(&ranges).unwrap_or(0.0) - reference_m;
        self.offsets.insert(key, offset);
        Ok(offset)
    }

    /// The offset for an observation's link and boot, if one was recorded.
    pub fn offset_for(&self, obs: &RangeObservation) -> Option<f64> {
        let boot = obs.responder_boot?;
        self.offsets
            .get(&(obs.initiator, obs.responder, boot))
            .copied()
    }

    /// Return a calibrated copy when an offset matches, else an unchanged copy.
    pub fn apply(&self, obs: &RangeObservation) -> RangeObservation {
        let mut out = obs.clone();
        if obs.calibrated {
            return out;
        }
        if let Some(offset) = self.offset_for(obs) {
            out.range_m = (obs.range_m - offset).max(0.0);
            out.calibrated = true;
        }
        out
    }

    /// Number of recorded offsets.
    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    /// True when no offsets are recorded.
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(range: f64, boot: Option<u32>) -> RangeObservation {
        RangeObservation {
            responder_boot: boot,
            ..RangeObservation::ftm(1, 2, range, 0.3)
        }
    }

    #[test]
    fn offset_applies_only_to_the_same_boot() {
        let mut t = CalibrationTable::new();
        let off = t
            .record_reference(
                &[obs(3.0, Some(7)), obs(3.1, Some(7)), obs(3.2, Some(7))],
                1.143,
            )
            .unwrap();
        assert!((off - (3.1 - 1.143)).abs() < 1e-9);

        let same = t.apply(&obs(5.0, Some(7)));
        assert!(same.calibrated);
        assert!((same.range_m - (5.0 - off)).abs() < 1e-9);

        let other_boot = t.apply(&obs(5.0, Some(8)));
        assert!(!other_boot.calibrated);
        assert_eq!(other_boot.range_m, 5.0);

        let no_boot = t.apply(&obs(5.0, None));
        assert!(!no_boot.calibrated);
    }

    #[test]
    fn rejects_short_reference_and_mixed_links() {
        let mut t = CalibrationTable::new();
        assert!(t.record_reference(&[obs(0.0, Some(1))], 0.5).is_err());
        assert!(t.record_reference(&[obs(1.0, None)], 1.2).is_err());
        assert!(t
            .record_reference(&[obs(1.0, Some(1)), obs(1.0, Some(2))], 1.2)
            .is_err());
        assert!(t.record_reference(&[], 1.2).is_err());
        assert!(t.is_empty());
    }

    #[test]
    fn calibrated_range_never_goes_negative() {
        let mut t = CalibrationTable::new();
        t.record_reference(&[obs(3.0, Some(1))], 1.0).unwrap();
        assert_eq!(t.apply(&obs(0.5, Some(1))).range_m, 0.0);
    }
}
