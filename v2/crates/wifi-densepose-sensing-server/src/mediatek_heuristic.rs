//! A clearly-labelled heuristic presence/motion detector for real MediaTek
//! MTC1 CSI, used only because the validated ESP32 pipeline
//! (`extract_features_from_frame` / `VitalSignDetector`) needs per-subcarrier
//! amplitude and phase arrays that `MediatekCsiSnapshot` does not retain (see
//! its doc comment on `AppStateInner::latest_mediatek_csi`: "raw matrices are
//! not retained here"). All this module has to work with, per frame, is
//! `mean_amplitude`, `peak_amplitude`, and per-chain `rssi_dbm`.
//!
//! Every [`DeviceVerdict`] carries [`CLASSIFIER_ID`] (via
//! `mediatek_room_update` in main.rs) so nothing downstream can mistake it
//! for the validated ESP32 detector. Per repository policy (RuView
//! `CLAUDE.md`: "Never present WiFi sensing as camera-grade... tag
//! MEASURED/CLAIMED/SYNTHETIC"), treat this output as `CLAIMED`, not
//! `MEASURED` — it has not been validated against ground truth.
//!
//! # 2026-09-19 field data and the hysteresis/unknown-state fix
//!
//! In a multi-person field run with one person walking between the two
//! receivers (CLAIMED: from field notes, not a labelled evaluation), the v0
//! design (a fixed 64-sample count window, single-sample threshold crossing)
//! flickered `present_moving` -> `absent` on 5+ of ~90
//! two-second samples, sometimes for 10 s runs, with confidence collapsing to
//! 0.01-0.04 while both receivers were fresh (age < 1 s) and clearly
//! non-flat. Two causes, both addressed below:
//!
//! - The dump-loop clients are bursty (0.25-6 s gaps between arrivals per
//!   receiver), so a **count**-based window mixed old and new activity
//!   unpredictably depending on arrival timing rather than elapsed time. The
//!   window is now **time**-based ([`WINDOW_DURATION`]): samples older than
//!   the window are pruned by age on every observation, not by count, and a
//!   gap does not clear or reset the window — it just means fewer samples
//!   remain in it until fresh ones arrive.
//! - A single transient dip below threshold (a real one — e.g. a deep RF
//!   fade mid-walk, not a bug) instantly flipped `presence` from `true` to
//!   `false`. Presence now has hysteresis: fresh evidence sets presence
//!   immediately, but leaving presence requires BOTH the metric being
//!   continuously below threshold for [`CONFIRM_ABSENT_DURATION`] AND being
//!   past [`HOLD_DURATION`] since the last positive evidence.
//! - Confidence still legitimately collapses sometimes (e.g. a genuinely
//!   sparse window right after a receiver has been quiet). Reading a
//!   low-confidence sample as a confident "absent" was itself the bug ("one
//!   person stood up" was reported as `absent 0.09`). Below
//!   [`UNKNOWN_CONFIDENCE_THRESHOLD`] the device now reports
//!   [`PresenceState::Unknown`] (`presence: None`) instead of asserting
//!   either way, and abstains from the room-level vote in
//!   `mediatek_room_update` rather than getting to assert "absent" on weak
//!   evidence.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Tags every output of this module so it is never mistaken for the
/// validated ESP32 `extract_features_from_frame` / `VitalSignDetector`
/// pipeline. Bump the trailing version if the method below changes in a way
/// that would invalidate a prior comparison.
pub(crate) const CLASSIFIER_ID: &str = "mediatek-amplitude-heuristic-v0";

/// Time-based amplitude window. Replaces a fixed sample count (v0) so a
/// bursty arrival pattern (dump-loop clients see 0.25-6 s gaps) doesn't make
/// the window's real time coverage depend on how lucky recent arrivals were.
/// Samples are pruned by age, not count; a gap does not reset the window.
pub(crate) const WINDOW_DURATION: Duration = Duration::from_secs(10);

/// How long presence persists after the last positive evidence before a
/// flip to absent is even considered. A transient dip inside this window
/// never flips presence on its own.
pub(crate) const HOLD_DURATION: Duration = Duration::from_secs(10);

/// Once past `HOLD_DURATION`, the metric must additionally read
/// continuously below threshold for this long before presence actually
/// commits to absent — one low sample is not enough.
pub(crate) const CONFIRM_ABSENT_DURATION: Duration = Duration::from_secs(3);

/// Below this confidence, a device reports [`PresenceState::Unknown`]
/// (`presence: None`) rather than asserting either "present" or "absent",
/// and abstains from the room-level vote. Raw CSI abstains rather than
/// guesses, same convention the ADR-267 provenance gate already uses
/// elsewhere in this crate.
pub(crate) const UNKNOWN_CONFIDENCE_THRESHOLD: f64 = 0.3;

/// Coefficient of variation (stdev / |mean|) of `mean_amplitude` over the
/// window above which motion is declared. Loosely tuned against the ADR-266
/// simulator's synthetic motion phase; revisit as more real WN586X3 capture
/// accumulates.
pub(crate) const MOTION_CV_THRESHOLD: f64 = 0.05;

/// Fractional deviation of the latest `mean_amplitude` from the slow
/// baseline above which "present but still" is declared even with low
/// frame-to-frame variance (a body blocking/reflecting the channel without
/// moving).
pub(crate) const BASELINE_DEVIATION_THRESHOLD: f64 = 0.08;

/// EMA smoothing factor for the slow-moving empty-room baseline, applied
/// per-*sample* (not per elapsed time) — a known simplification: during a
/// dense burst the baseline drifts faster than during a quiet gap. Small
/// enough that a person entering the room doesn't drag the baseline toward
/// "occupied is normal" within a few samples either way.
pub(crate) const BASELINE_EMA_ALPHA: f64 = 0.02;

/// Coarse motion label, matching the string vocabulary `RoomInference` /
/// `ClassificationInfo` already use elsewhere in this crate. This is the
/// *raw*, instantaneous reading for this observation, before hysteresis;
/// see [`PresenceState`] for the debounced, room-vote-eligible state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MotionLevel {
    Absent,
    PresentStill,
    PresentMoving,
}

impl MotionLevel {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            MotionLevel::Absent => "absent",
            MotionLevel::PresentStill => "present_still",
            MotionLevel::PresentMoving => "present_moving",
        }
    }
}

/// The hysteresis-committed, confidence-gated presence call. Distinct from
/// the raw `MotionLevel` above: this is what a device is allowed to assert
/// to the room vote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresenceState {
    Present,
    Absent,
    /// Confidence below `UNKNOWN_CONFIDENCE_THRESHOLD`. Not a vote either
    /// way — see `mediatek_room_update`'s exclusion of Unknown devices from
    /// `fuse_room`.
    Unknown,
}

impl PresenceState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            PresenceState::Present => "present",
            PresenceState::Absent => "absent",
            PresenceState::Unknown => "unknown",
        }
    }
}

/// Per-device amplitude-window statistics, exposed in `nodes[]` (as
/// `MediatekNodeDiagnostics` in main.rs) so a client can show why a device
/// reads the way it does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WindowStats {
    pub(crate) coefficient_of_variation: f64,
    pub(crate) baseline_deviation: f64,
    pub(crate) sample_count: usize,
    pub(crate) span_ms: u64,
}

/// One device's verdict for the current moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DeviceVerdict {
    /// Raw, instantaneous motion reading (not hysteresis-debounced) — kept
    /// for diagnostics; `presence_state`/`presence` are what should drive
    /// any decision.
    pub(crate) motion_level: MotionLevel,
    pub(crate) presence_state: PresenceState,
    /// `Some(true)`/`Some(false)` mirror `presence_state`'s Present/Absent;
    /// `None` when `presence_state` is `Unknown`. Nullable specifically so a
    /// low-confidence reading is never silently read as a confident
    /// "absent" (2026-09-19: "one person stood up" was reported as
    /// `absent 0.09` before this fix).
    pub(crate) presence: Option<bool>,
    /// This device's own confidence: window fill x how far past (or safely
    /// under, for an absent-leaning reading) the relevant threshold the
    /// signal sits. Cross-device agreement is applied separately by
    /// [`apply_agreement`], since a single device has no visibility into
    /// its peers.
    pub(crate) raw_confidence: f64,
    pub(crate) window: WindowStats,
}

/// Time-windowed per-device amplitude history plus the slow baseline and
/// hysteresis state it's judged against. One instance lives per `device_id`
/// for the life of the process (or until the caller decides to drop a
/// long-silent device).
#[derive(Debug, Clone, Default)]
pub(crate) struct DeviceHistory {
    /// (arrival time, mean_amplitude), oldest first. Pruned by age against
    /// `WINDOW_DURATION` on every observation — never by count.
    samples: VecDeque<(Instant, f64)>,
    baseline: Option<f64>,
    /// Last time raw evidence (motion or sustained deviation) was seen.
    /// Presence cannot commit to absent within `HOLD_DURATION` of this.
    last_present_evidence: Option<Instant>,
    /// When the metric most recently became continuously below threshold.
    /// `None` whenever the metric is currently at/above threshold. Presence
    /// cannot commit to absent until `now - this >= CONFIRM_ABSENT_DURATION`.
    below_threshold_since: Option<Instant>,
    /// The last hysteresis-committed presence call (pre-confidence-gate).
    /// Defaults to `false`: a device with no evidence yet has nothing to
    /// hold onto, and starts out below the confidence threshold anyway (an
    /// empty window), so it reports Unknown until it has real data —
    /// this default is never observable as a false "present".
    committed_presence: bool,
}

impl DeviceHistory {
    /// Fold in one new frame's `mean_amplitude`, observed at `now`, and
    /// return this device's verdict. `peak_amplitude` is accepted for
    /// future use (e.g. a saturation guard) but not yet part of the
    /// decision.
    pub(crate) fn observe(
        &mut self,
        now: Instant,
        mean_amplitude: f64,
        _peak_amplitude: f64,
    ) -> DeviceVerdict {
        self.samples.push_back((now, mean_amplitude));
        self.prune(now);

        self.baseline = Some(match self.baseline {
            None => mean_amplitude,
            Some(b) => b + BASELINE_EMA_ALPHA * (mean_amplitude - b),
        });

        self.verdict_at(now)
    }

    /// Recompute this device's verdict at `now` without folding in a new
    /// sample — used for devices that aren't the one whose frame just
    /// arrived this tick, so the room picture still reflects every
    /// currently-fresh receiver. Still prunes the window by age (a gap
    /// since the last real sample must age the window normally).
    pub(crate) fn verdict(&mut self, now: Instant) -> DeviceVerdict {
        self.prune(now);
        self.verdict_at(now)
    }

    fn prune(&mut self, now: Instant) {
        while let Some(&(t, _)) = self.samples.front() {
            if now.saturating_duration_since(t) > WINDOW_DURATION {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }

    fn verdict_at(&mut self, now: Instant) -> DeviceVerdict {
        let n = self.samples.len();
        if n == 0 {
            return DeviceVerdict {
                motion_level: MotionLevel::Absent,
                presence_state: PresenceState::Unknown,
                presence: None,
                raw_confidence: 0.0,
                window: WindowStats {
                    coefficient_of_variation: 0.0,
                    baseline_deviation: 0.0,
                    sample_count: 0,
                    span_ms: 0,
                },
            };
        }

        let span_ms = self
            .samples
            .back()
            .zip(self.samples.front())
            .map(|((newest, _), (oldest, _))| {
                newest.saturating_duration_since(*oldest).as_millis() as u64
            })
            .unwrap_or(0);
        let window_fill = (span_ms as f64 / WINDOW_DURATION.as_millis() as f64).min(1.0);

        let mean: f64 = self.samples.iter().map(|(_, v)| v).sum::<f64>() / n as f64;
        let variance = if n > 1 {
            self.samples
                .iter()
                .map(|(_, v)| (v - mean).powi(2))
                .sum::<f64>()
                / n as f64
        } else {
            0.0
        };
        let cv = if mean.abs() > f64::EPSILON {
            variance.sqrt() / mean.abs()
        } else {
            0.0
        };

        let latest = self.samples.back().map(|(_, v)| *v).unwrap_or(mean);
        let baseline = self.baseline.unwrap_or(latest);
        let deviation = if baseline.abs() > f64::EPSILON {
            (latest - baseline).abs() / baseline.abs()
        } else {
            0.0
        };

        let motion = cv > MOTION_CV_THRESHOLD;
        let sustained = deviation > BASELINE_DEVIATION_THRESHOLD;
        let raw_evidence = motion || sustained;
        let motion_level = if motion {
            MotionLevel::PresentMoving
        } else if sustained {
            MotionLevel::PresentStill
        } else {
            MotionLevel::Absent
        };

        // Hysteresis: feed the evidence timers, then decide whether a flip
        // to absent is actually allowed yet.
        if raw_evidence {
            self.last_present_evidence = Some(now);
            self.below_threshold_since = None;
            self.committed_presence = true;
        } else {
            if self.below_threshold_since.is_none() {
                self.below_threshold_since = Some(now);
            }
            let held_by_recency = self
                .last_present_evidence
                .is_some_and(|t| now.saturating_duration_since(t) < HOLD_DURATION);
            let confirmed_absent = self
                .below_threshold_since
                .is_some_and(|t| now.saturating_duration_since(t) >= CONFIRM_ABSENT_DURATION);
            if !held_by_recency && confirmed_absent {
                self.committed_presence = false;
            }
            // Otherwise: keep whatever `committed_presence` already was — a
            // transient dip inside the hold/confirm window never flips it.
        }

        // Signal margin: how far past (present) or safely under (absent)
        // threshold the stronger signal sits, normalized into roughly
        // [0, 1] either way — based on the *committed* call, not the raw
        // instantaneous one, so confidence doesn't itself flicker on a
        // single-sample blip that hysteresis is about to override anyway.
        let motion_margin = if MOTION_CV_THRESHOLD > 0.0 {
            cv / MOTION_CV_THRESHOLD
        } else {
            0.0
        };
        let deviation_margin = if BASELINE_DEVIATION_THRESHOLD > 0.0 {
            deviation / BASELINE_DEVIATION_THRESHOLD
        } else {
            0.0
        };
        let signal_margin = if self.committed_presence {
            (motion_margin.max(deviation_margin) / 2.0).clamp(0.0, 1.0)
        } else {
            (1.0 - motion_margin.max(deviation_margin).min(1.0)).clamp(0.0, 1.0)
        };

        let raw_confidence = (window_fill * signal_margin).clamp(0.0, 1.0);
        let presence_state = if raw_confidence < UNKNOWN_CONFIDENCE_THRESHOLD {
            PresenceState::Unknown
        } else if self.committed_presence {
            PresenceState::Present
        } else {
            PresenceState::Absent
        };
        let presence = match presence_state {
            PresenceState::Present => Some(true),
            PresenceState::Absent => Some(false),
            PresenceState::Unknown => None,
        };

        DeviceVerdict {
            motion_level,
            presence_state,
            presence,
            raw_confidence,
            window: WindowStats {
                coefficient_of_variation: cv,
                baseline_deviation: deviation,
                sample_count: n,
                span_ms,
            },
        }
    }
}

/// Apply cross-device agreement to a set of already-computed verdicts, keyed
/// by `device_id`. Only devices with a definite `presence: Some(_)`
/// participate — an `Unknown` device neither votes nor is voted against, and
/// its own confidence is left unscaled (it is already below the unknown
/// threshold; scaling it further changes nothing observable).
///
/// A lone definite device (or all-but-one Unknown) is never penalized —
/// there is nothing to disagree with. Otherwise each definite device's
/// confidence is scaled by `0.5 + 0.5 * agreement`, where `agreement` is the
/// fraction of *other* definite devices sharing its `presence` call: full
/// agreement leaves confidence unchanged, full disagreement halves it. A
/// hard zero on any disagreement was considered and rejected — two routers
/// legitimately cover different zones of a room (a doorway vs. a couch), so
/// routine partial disagreement should dampen confidence, not erase it.
pub(crate) fn apply_agreement(verdicts: &HashMap<String, DeviceVerdict>) -> HashMap<String, f64> {
    let definite: Vec<(&String, bool)> = verdicts
        .iter()
        .filter_map(|(id, v)| v.presence.map(|p| (id, p)))
        .collect();
    let total = definite.len();

    verdicts
        .iter()
        .map(|(id, v)| {
            let confidence = match v.presence {
                None => v.raw_confidence, // Unknown: left as-is.
                Some(_) if total <= 1 => v.raw_confidence,
                Some(p) => {
                    let others = total - 1;
                    let agreeing_others = definite.iter().filter(|(_, o)| *o == p).count() - 1;
                    let agreement = agreeing_others as f64 / others as f64;
                    v.raw_confidence * (0.5 + 0.5 * agreement)
                }
            };
            (id.clone(), confidence.clamp(0.0, 1.0))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_amplitude_stream_is_absent_once_confidently_settled() {
        let mut h = DeviceHistory::default();
        let start = Instant::now();
        let mut verdict = h.observe(start, 100.0, 102.0);
        // Immediately after the first sample the window barely covers any
        // time, so confidence should read Unknown, not a confident absent.
        assert_eq!(verdict.presence_state, PresenceState::Unknown);

        // Feed flat samples spanning past HOLD_DURATION + CONFIRM_ABSENT_DURATION
        // so the window fills and any (nonexistent) presence hold expires.
        let mut t = start;
        for _ in 0..40 {
            t += Duration::from_millis(500);
            verdict = h.observe(t, 100.0, 102.0);
        }
        assert_eq!(verdict.motion_level, MotionLevel::Absent);
        assert_eq!(verdict.presence_state, PresenceState::Absent);
        assert_eq!(verdict.presence, Some(false));
        assert!(
            verdict.raw_confidence > 0.7,
            "confidence was {}",
            verdict.raw_confidence
        );
    }

    #[test]
    fn injected_variance_is_present_moving() {
        let mut h = DeviceHistory::default();
        let start = Instant::now();
        let mut t = start;
        let mut verdict = h.observe(t, 100.0, 102.0);
        for i in 0..40 {
            t += Duration::from_millis(500);
            let sample = if i % 2 == 0 { 100.0 } else { 130.0 };
            verdict = h.observe(t, sample, sample + 2.0);
        }
        assert_eq!(verdict.motion_level, MotionLevel::PresentMoving);
        assert_eq!(verdict.presence_state, PresenceState::Present);
        assert_eq!(verdict.presence, Some(true));
    }

    /// The exact 2026-09-19 field bug: presence is well-established, then one
    /// sample dips below threshold (a real transient fade, not a bug in the
    /// underlying signal) — hysteresis must not flip presence to absent on
    /// that single sample.
    #[test]
    fn a_single_transient_dip_does_not_flip_presence_to_absent() {
        let mut h = DeviceHistory::default();
        let start = Instant::now();
        let mut t = start;
        let mut verdict = h.observe(t, 100.0, 102.0);
        // Establish confident, sustained motion.
        for i in 0..40 {
            t += Duration::from_millis(500);
            let sample = if i % 2 == 0 { 100.0 } else { 130.0 };
            verdict = h.observe(t, sample, sample + 2.0);
        }
        assert_eq!(
            verdict.presence,
            Some(true),
            "must be confidently present before the dip"
        );

        // One flat sample right after (a fade) — presence must hold.
        t += Duration::from_millis(500);
        verdict = h.observe(t, 115.0, 117.0);
        assert_eq!(
            verdict.presence,
            Some(true),
            "a single dip within HOLD_DURATION/CONFIRM_ABSENT_DURATION must not flip presence"
        );
    }

    /// If the dip is real and sustained (not a blip), presence must
    /// eventually commit to absent — hysteresis dampens, it doesn't hide a
    /// genuine departure forever.
    #[test]
    fn a_sustained_dip_past_hold_and_confirm_duration_flips_to_absent() {
        let mut h = DeviceHistory::default();
        let start = Instant::now();
        let mut t = start;
        let mut verdict = h.observe(t, 100.0, 102.0);
        for i in 0..40 {
            t += Duration::from_millis(500);
            let sample = if i % 2 == 0 { 100.0 } else { 130.0 };
            verdict = h.observe(t, sample, sample + 2.0);
        }
        assert_eq!(verdict.presence, Some(true));

        // Flat for well past HOLD_DURATION + CONFIRM_ABSENT_DURATION.
        for _ in 0..40 {
            t += Duration::from_millis(500);
            verdict = h.observe(t, 115.0, 117.0);
        }
        assert_eq!(
            verdict.presence,
            Some(false),
            "a genuinely sustained dip must eventually commit to absent"
        );
    }

    /// A bursty two-device timeline: one device reports in dense bursts with
    /// multi-second gaps between them (per the real dump-loop behaviour).
    /// The time-based window must not starve during a gap the way a
    /// count-based window with a low arrival rate effectively would.
    #[test]
    fn bursty_arrivals_still_fill_a_time_based_window() {
        let mut h = DeviceHistory::default();
        let start = Instant::now();
        let mut t = start;
        let mut verdict = h.observe(t, 100.0, 102.0);
        // Three bursts of 5 samples each, 3 s apart (a gap well inside
        // WINDOW_DURATION, matching observed dump-loop gaps).
        for _burst in 0..3 {
            for i in 0..5 {
                t += Duration::from_millis(100);
                let sample = if i % 2 == 0 { 100.0 } else { 130.0 };
                verdict = h.observe(t, sample, sample + 2.0);
            }
            t += Duration::from_secs(3);
        }
        // All three bursts are within WINDOW_DURATION (10s) of the last one,
        // so the window should have accumulated all 16 samples, not just
        // the most recent burst.
        assert_eq!(verdict.window.sample_count, 16);
        assert!(
            verdict.window.span_ms > 6_000,
            "span was {}",
            verdict.window.span_ms
        );
    }

    /// A gap longer than WINDOW_DURATION must age out the old burst rather
    /// than let it linger forever.
    #[test]
    fn a_gap_longer_than_the_window_ages_out_old_samples() {
        let mut h = DeviceHistory::default();
        let start = Instant::now();
        let mut t = start;
        for i in 0..5 {
            t += Duration::from_millis(100);
            let sample = if i % 2 == 0 { 100.0 } else { 130.0 };
            let _ = h.observe(t, sample, sample + 2.0);
        }
        t += WINDOW_DURATION + Duration::from_secs(1);
        let verdict = h.observe(t, 100.0, 102.0);
        assert_eq!(
            verdict.window.sample_count, 1,
            "the stale burst must have aged out"
        );
    }

    #[test]
    fn low_confidence_reads_as_unknown_not_absent() {
        // A single sample: window barely covers any time, so window_fill is
        // near zero regardless of the amplitude value — must read Unknown,
        // never a confident "absent" (2026-09-19: "one person stood up" was
        // reported as `absent 0.09` before this fix).
        let mut h = DeviceHistory::default();
        let verdict = h.observe(Instant::now(), 1800.0, 9000.0);
        assert_eq!(verdict.presence_state, PresenceState::Unknown);
        assert_eq!(verdict.presence, None);
        assert!(verdict.raw_confidence < UNKNOWN_CONFIDENCE_THRESHOLD);
    }

    #[test]
    fn lone_definite_device_is_never_penalized_by_agreement() {
        let mut verdicts = HashMap::new();
        verdicts.insert(
            "only-device".to_string(),
            DeviceVerdict {
                motion_level: MotionLevel::PresentMoving,
                presence_state: PresenceState::Present,
                presence: Some(true),
                raw_confidence: 0.9,
                window: WindowStats {
                    coefficient_of_variation: 0.1,
                    baseline_deviation: 0.0,
                    sample_count: 20,
                    span_ms: 9000,
                },
            },
        );
        let adjusted = apply_agreement(&verdicts);
        assert_eq!(adjusted["only-device"], 0.9);
    }

    #[test]
    fn unknown_device_neither_votes_nor_is_voted_against() {
        let mut verdicts = HashMap::new();
        verdicts.insert(
            "dev-present".to_string(),
            DeviceVerdict {
                motion_level: MotionLevel::PresentMoving,
                presence_state: PresenceState::Present,
                presence: Some(true),
                raw_confidence: 0.9,
                window: WindowStats {
                    coefficient_of_variation: 0.1,
                    baseline_deviation: 0.0,
                    sample_count: 20,
                    span_ms: 9000,
                },
            },
        );
        verdicts.insert(
            "dev-unknown".to_string(),
            DeviceVerdict {
                motion_level: MotionLevel::Absent,
                presence_state: PresenceState::Unknown,
                presence: None,
                raw_confidence: 0.05,
                window: WindowStats {
                    coefficient_of_variation: 0.01,
                    baseline_deviation: 0.0,
                    sample_count: 2,
                    span_ms: 200,
                },
            },
        );
        let adjusted = apply_agreement(&verdicts);
        // Only one *definite* device -> no disagreement possible, full confidence kept.
        assert_eq!(adjusted["dev-present"], 0.9);
        // Unknown device's own confidence is left untouched.
        assert_eq!(adjusted["dev-unknown"], 0.05);
    }

    #[test]
    fn two_definite_devices_disagreeing_have_lowered_confidence() {
        let mut verdicts = HashMap::new();
        verdicts.insert(
            "dev-moving".to_string(),
            DeviceVerdict {
                motion_level: MotionLevel::PresentMoving,
                presence_state: PresenceState::Present,
                presence: Some(true),
                raw_confidence: 0.9,
                window: WindowStats {
                    coefficient_of_variation: 0.1,
                    baseline_deviation: 0.0,
                    sample_count: 20,
                    span_ms: 9000,
                },
            },
        );
        verdicts.insert(
            "dev-absent".to_string(),
            DeviceVerdict {
                motion_level: MotionLevel::Absent,
                presence_state: PresenceState::Absent,
                presence: Some(false),
                raw_confidence: 0.9,
                window: WindowStats {
                    coefficient_of_variation: 0.01,
                    baseline_deviation: 0.0,
                    sample_count: 20,
                    span_ms: 9000,
                },
            },
        );
        let adjusted = apply_agreement(&verdicts);
        assert!((adjusted["dev-moving"] - 0.45).abs() < 1e-9);
        assert!((adjusted["dev-absent"] - 0.45).abs() < 1e-9);
    }
}
