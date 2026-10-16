//! Enrollment protocol — per-anchor capture with an adaptive quality gate
//! (ADR-151 Stage 2).
//!
//! Bad anchors poison small calibrated models far more than large ones, so an
//! anchor is only *accepted* when its captured statistics match what the anchor
//! is supposed to teach: a person present (or absent for `empty`), and the
//! expected stillness/motion. Failed anchors are re-prompted, not silently kept.
//!
//! Quality is measured against the ADR-135 empty-room baseline via
//! [`wifi_densepose_signal::BaselineCalibration::deviation`], whose
//! `CalibrationDeviationScore` gives a per-frame amplitude z-score (presence
//! strength).
//!
//! **Motion is NOT taken from the score's `motion_flagged`** (ADR-152 finding,
//! "z-band squeeze"): that flag fires on `amplitude_z_median > 2.0` — deviation
//! from the *empty* baseline — which conflates presence strength with motion. A
//! strongly-reflecting person standing perfectly still (z > 2 on every frame)
//! would be rejected as "too much motion". Instead the recorder derives motion
//! from the frame-to-frame *change* in the deviation series (|Δz| and |Δφ|),
//! which is presence-independent: a still strong reflector has high z but a
//! flat z-series; a moving person has a jittery one.

use wifi_densepose_core::types::CsiFrame;
use wifi_densepose_signal::{BaselineCalibration, CalibrationDeviationScore};

use crate::anchor::{Anchor, AnchorLabel, AnchorQuality};

/// Thresholds for accepting an anchor.
#[derive(Debug, Clone, Copy)]
pub struct AnchorQualityGate {
    /// Minimum mean amplitude z-score to consider a person present.
    pub min_presence_z: f32,
    /// Minimum **relative** mean-amplitude shift versus the session's own
    /// `empty` capture that counts as "a person is present" (0.0075 = 0.75%).
    ///
    /// The per-subcarrier `presence_z` test measures frame-to-frame jitter, not
    /// the presence shift: live ESP32-S3 data showed a person standing still
    /// shifting the capture mean by +0.5..+1.8 amplitude units (6-12 sigma over
    /// ~1300 frames) while `presence_z` stayed flat at ~0.8 for both the empty
    /// and the occupied room. When `mean_shift_rel` is supplied to `evaluate`,
    /// it replaces the z test. `0.0` keeps the legacy z behaviour.
    pub min_mean_shift: f32,
    /// For `empty`: maximum mean z-score to consider the room truly empty.
    pub empty_max_z: f32,
    /// For "still" anchors: maximum motion-flag rate tolerated.
    pub max_still_motion: f32,
    /// For the "move" anchor: minimum motion-flag rate required.
    pub min_move_motion: f32,
    /// Minimum frames required to evaluate an anchor.
    pub min_frames: u32,
}

impl Default for AnchorQualityGate {
    fn default() -> Self {
        Self {
            min_presence_z: 1.5,
            empty_max_z: 1.0,
            min_mean_shift: 0.0,
            max_still_motion: 0.6,
            min_move_motion: 0.3,
            min_frames: 60,
        }
    }
}

impl AnchorQualityGate {
    /// Evaluate accumulated stats for `label`, returning the quality verdict
    /// and (on rejection) a human-readable reason.
    pub fn evaluate(
        &self,
        label: AnchorLabel,
        presence_z: f32,
        motion_rate: f32,
        frames: u32,
        mean_shift_rel: Option<f32>,
    ) -> (AnchorQuality, Option<String>) {
        let mut reason: Option<String> = None;

        if frames < self.min_frames {
            reason = Some(format!(
                "only {frames} frames (need ≥{}); is the ESP32 streaming?",
                self.min_frames
            ));
        } else if label.expects_presence() {
            // Magnitude, not sign: a person changes the multipath phase, so the
            // capture mean can move either way (live data: -13.7%..+3.7%).
            let present = match mean_shift_rel {
                Some(shift) => shift.abs() >= self.min_mean_shift,
                None => presence_z >= self.min_presence_z,
            };
            if !present {
                reason = Some(match mean_shift_rel {
                    Some(shift) => format!(
                        "no presence shift (|capture mean {:+.2}%| vs this session's empty capture, need >= {:.2}%) — move into the sensed area",
                        100.0 * shift,
                        100.0 * self.min_mean_shift
                    ),
                    None => format!(
                        "no person detected (presence_z {presence_z:.2} < {:.2}) — move closer / face the sensor",
                        self.min_presence_z
                    ),
                });
            } else if label.expects_still() && motion_rate > self.max_still_motion {
                reason = Some(format!(
                    "too much motion ({:.0}% > {:.0}%) for a still anchor — hold still",
                    motion_rate * 100.0,
                    self.max_still_motion * 100.0
                ));
            } else if !label.expects_still() && motion_rate < self.min_move_motion {
                reason = Some(format!(
                    "not enough motion ({:.0}% < {:.0}%) — move a bit more",
                    motion_rate * 100.0,
                    self.min_move_motion * 100.0
                ));
            }
        } else {
            // `empty` anchor: the room must actually be empty.
            if presence_z > self.empty_max_z {
                reason = Some(format!(
                    "room not empty (presence_z {presence_z:.2} > {:.2}) — clear the room",
                    self.empty_max_z
                ));
            }
        }

        let quality = AnchorQuality {
            presence_z,
            motion_rate,
            frames,
            accepted: reason.is_none(),
        };
        (quality, reason)
    }
}

/// Frame-to-frame amplitude-z change above which a frame counts as motion.
///
/// Presence-independent by construction: a still person shifts the z *level*
/// but not its frame-to-frame delta (only noise-scale jitter survives), while
/// body movement modulates the reflected paths every frame. Sized well above
/// the delta the baseline's own noise floor produces (≲0.3σ) and well below
/// the delta even small limb movements produce (≳1σ). See ADR-152.
pub const Z_DELTA_MOTION: f32 = 0.5;

/// Frame-to-frame phase-drift change above which a frame counts as motion.
/// Same constant family as the absolute π/6 drift bound in
/// `CalibrationDeviationScore`, applied to the delta (static body phase shift
/// cancels out).
pub const PHASE_DELTA_MOTION: f32 = std::f32::consts::PI / 6.0;

/// Accumulates per-frame deviation statistics for a single anchor capture.
pub struct AnchorRecorder {
    label: AnchorLabel,
    z_sum: f64,
    motion_count: u32,
    frames: u32,
    /// Previous frame's (amplitude_z_median, phase_drift_median) for the
    /// delta-based motion measure (ADR-152 z-band-squeeze fix).
    prev: Option<(f32, f32)>,
}

impl AnchorRecorder {
    /// Start recording the given anchor.
    pub fn new(label: AnchorLabel) -> Self {
        Self {
            label,
            z_sum: 0.0,
            motion_count: 0,
            frames: 0,
            prev: None,
        }
    }

    /// The anchor being recorded.
    pub fn label(&self) -> AnchorLabel {
        self.label
    }

    /// Frames recorded so far.
    pub fn frames(&self) -> u32 {
        self.frames
    }

    /// Record a pre-computed deviation score (caller runs `baseline.deviation`).
    ///
    /// Motion is derived from the frame-to-frame change of the deviation
    /// series, NOT from `score.motion_flagged` — the flag conflates presence
    /// strength with motion (z-band squeeze, see module docs / ADR-152). The
    /// first frame of a capture is never motion (no predecessor).
    pub fn record_score(&mut self, score: &CalibrationDeviationScore) {
        let z = score.amplitude_z_median;
        let phase = score.phase_drift_median;
        if let Some((pz, pp)) = self.prev {
            if (z - pz).abs() > Z_DELTA_MOTION || (phase - pp).abs() > PHASE_DELTA_MOTION {
                self.motion_count += 1;
            }
        }
        self.prev = Some((z, phase));
        self.z_sum += z as f64;
        self.frames += 1;
    }

    /// Convenience: record a CSI frame directly against a baseline.
    /// Frames that fail baseline geometry checks are skipped (not counted).
    pub fn record_frame(&mut self, baseline: &BaselineCalibration, frame: &CsiFrame) {
        if let Ok(score) = baseline.deviation(frame) {
            self.record_score(&score);
        }
    }

    /// Mean presence z-score over the capture.
    pub fn presence_z(&self) -> f32 {
        if self.frames == 0 {
            0.0
        } else {
            (self.z_sum / self.frames as f64) as f32
        }
    }

    /// Fraction of frames flagged as motion.
    pub fn motion_rate(&self) -> f32 {
        if self.frames == 0 {
            0.0
        } else {
            self.motion_count as f32 / self.frames as f32
        }
    }

    /// Evaluate the capture against the gate and produce an `Anchor` (accepted
    /// or not) plus a rejection reason.
    pub fn finalize(
        &self,
        gate: &AnchorQualityGate,
        at_unix_s: i64,
        mean_shift_rel: Option<f32>,
    ) -> (Anchor, Option<String>) {
        let (quality, reason) = gate.evaluate(
            self.label,
            self.presence_z(),
            self.motion_rate(),
            self.frames,
            mean_shift_rel,
        );
        (
            Anchor {
                label: self.label,
                captured_at_unix_s: at_unix_s,
                quality,
            },
            reason,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a score the way `BaselineCalibration::deviation` actually would:
    /// `motion_flagged` is DERIVED from z (z > 2.0 ⇒ flagged), never free.
    /// The old tests mocked `(z=3.0, motion=false)` — a combination the real
    /// producer can never emit, which is exactly how the z-band squeeze hid.
    fn score(z: f32) -> CalibrationDeviationScore {
        CalibrationDeviationScore {
            amplitude_z_median: z,
            amplitude_z_max: z + 1.0,
            phase_drift_median: 0.05,
            motion_flagged: z > 2.0,
        }
    }

    /// Record a z-series and finalize against the default gate.
    fn run_series(label: AnchorLabel, zs: &[f32]) -> (Anchor, Option<String>) {
        let mut r = AnchorRecorder::new(label);
        for &z in zs {
            r.record_score(&score(z));
        }
        r.finalize(&AnchorQualityGate::default(), 100, None)
    }

    /// Constant z (a perfectly still capture at the given presence strength).
    fn run_still(label: AnchorLabel, z: f32, n: usize) -> (Anchor, Option<String>) {
        run_series(label, &vec![z; n])
    }

    /// Alternating z (every frame's |Δz| exceeds Z_DELTA_MOTION ⇒ all motion).
    fn run_jittery(label: AnchorLabel, z: f32, n: usize) -> (Anchor, Option<String>) {
        let zs: Vec<f32> = (0..n)
            .map(|i| {
                if i % 2 == 0 {
                    z
                } else {
                    z + 2.0 * Z_DELTA_MOTION
                }
            })
            .collect();
        run_series(label, &zs)
    }

    /// ADR-152 z-band-squeeze regression: a STRONGLY-reflecting still person
    /// (z = 3.0, so every frame is motion_flagged by the baseline heuristic)
    /// must still pass a still anchor — presence strength is not motion.
    #[test]
    fn still_anchor_with_strong_still_person_accepts() {
        let (a, reason) = run_still(AnchorLabel::StandStill, 3.0, 400);
        assert!(a.quality.accepted, "z-band squeeze is back: {reason:?}");
        assert!(reason.is_none());
        assert!(
            a.quality.motion_rate < 0.05,
            "flat z-series must read still"
        );
    }

    #[test]
    fn still_anchor_rejects_when_no_presence() {
        let (a, reason) = run_still(AnchorLabel::Sit, 0.4, 400);
        assert!(!a.quality.accepted);
        assert!(reason.unwrap().contains("no person"));
    }

    #[test]
    fn still_anchor_rejects_on_motion() {
        let (a, reason) = run_jittery(AnchorLabel::LieDown, 3.0, 400);
        assert!(!a.quality.accepted);
        assert!(reason.unwrap().contains("motion"));
    }

    #[test]
    fn move_anchor_requires_motion() {
        let (still, r1) = run_still(AnchorLabel::SmallMove, 3.0, 400);
        assert!(!still.quality.accepted);
        assert!(r1.unwrap().contains("not enough motion"));
        let (moving, r2) = run_jittery(AnchorLabel::SmallMove, 3.0, 400);
        assert!(moving.quality.accepted, "reason: {r2:?}");
    }

    #[test]
    fn phase_delta_also_counts_as_motion() {
        // Constant z but a phase-drift series that swings past PHASE_DELTA_MOTION
        // every frame — motion must be detected from the phase channel alone.
        let mut r = AnchorRecorder::new(AnchorLabel::LieDown);
        for i in 0..400 {
            let mut s = score(1.8);
            s.phase_drift_median = if i % 2 == 0 {
                0.0
            } else {
                PHASE_DELTA_MOTION * 1.5
            };
            r.record_score(&s);
        }
        let (a, reason) = r.finalize(&AnchorQualityGate::default(), 100, None);
        assert!(!a.quality.accepted);
        assert!(reason.unwrap().contains("motion"));
    }

    #[test]
    fn empty_anchor_rejects_when_occupied() {
        let (occupied, reason) = run_still(AnchorLabel::Empty, 3.0, 400);
        assert!(!occupied.quality.accepted);
        assert!(reason.unwrap().contains("not empty"));
        let (empty, _) = run_still(AnchorLabel::Empty, 0.3, 400);
        assert!(empty.quality.accepted);
    }

    #[test]
    fn too_few_frames_rejected() {
        let (a, reason) = run_still(AnchorLabel::Sit, 3.0, 10);
        assert!(!a.quality.accepted);
        assert!(reason.unwrap().contains("frames"));
    }
}
