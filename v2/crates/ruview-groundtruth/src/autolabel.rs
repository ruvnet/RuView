//! Occupancy auto-labelling from a RuView recording plus a radar log.
//!
//! A RuView recording (`.csi.jsonl`, one line per CSI frame in the
//! `RecordedFrame` shape of the sensing server's `recording.rs`) and a radar log (see [`crate::radar`]) cover the same session. This
//! module cuts the recording at the radar's window boundaries and gives each
//! cut an occupancy label (empty / occupied) and a person count taken from the
//! radar. The output is training and evaluation data for CSI occupancy models;
//! the radar never becomes an input to them.
//!
//! The radar and the recording are stamped by different clocks. The offset
//! between them is a required, reported parameter, never estimated silently.
//!
//! Windows are dropped, with a counted reason, when the radar window has no
//! label, when too few CSI frames fall inside it, or when it says "empty" too
//! soon after an occupied window (a tracker can lose a person who stops
//! moving, so a short gap is more likely a miss than an exit).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::agreement::EvidenceGrade;
use crate::error::{check_bound, check_nonempty, GroundTruthError};
use crate::radar::{LabelStatus, RadarLabels, UnknownReason};
use crate::series::MAX_SAMPLES;

/// The largest recording line accepted, in bytes, bounding allocation.
pub const MAX_RECORDING_LINE_BYTES: usize = 1024 * 1024;

#[derive(Deserialize)]
struct RecordedFrameLine {
    timestamp: f64,
    #[serde(default)]
    subcarriers: Vec<f64>,
    #[serde(default)]
    rssi: Option<f64>,
}

/// One CSI frame from a recording, reduced to what windowing needs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsiFrame {
    /// Frame time on the recording clock, Unix milliseconds.
    pub at_unix_ms: i64,
    /// Received signal strength, dBm, when recorded.
    pub rssi: Option<f64>,
    /// Mean subcarrier amplitude of the frame, when it has subcarriers.
    pub amplitude_mean: Option<f64>,
}

/// Parse a RuView `.csi.jsonl` recording. Blank lines are skipped; a malformed
/// line or a non-finite timestamp is a row-numbered error.
///
/// # Errors
/// [`GroundTruthError::InvalidRow`] or [`GroundTruthError::TooManySamples`].
pub fn parse_recording_jsonl(text: &str) -> Result<Vec<CsiFrame>, GroundTruthError> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let row = i + 1;
        if line.len() > MAX_RECORDING_LINE_BYTES {
            return Err(GroundTruthError::InvalidRow {
                row,
                reason: format!(
                    "line is {} bytes, max {MAX_RECORDING_LINE_BYTES}",
                    line.len()
                ),
            });
        }
        let f: RecordedFrameLine =
            serde_json::from_str(line).map_err(|e| GroundTruthError::InvalidRow {
                row,
                reason: e.to_string(),
            })?;
        if !f.timestamp.is_finite() || f.timestamp < 0.0 {
            return Err(GroundTruthError::InvalidRow {
                row,
                reason: "timestamp must be finite, non-negative Unix seconds".into(),
            });
        }
        let finite: Vec<f64> = f
            .subcarriers
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .collect();
        out.push(CsiFrame {
            at_unix_ms: (f.timestamp * 1000.0).round() as i64,
            rssi: f.rssi.filter(|r| r.is_finite()),
            amplitude_mean: (!finite.is_empty())
                .then(|| finite.iter().map(|v| v.abs()).sum::<f64>() / finite.len() as f64),
        });
        if out.len() > MAX_SAMPLES {
            return Err(GroundTruthError::TooManySamples {
                len: out.len(),
                max: MAX_SAMPLES,
            });
        }
    }
    Ok(out)
}

/// Occupancy class of a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Occupancy {
    /// The radar saw nobody.
    Empty,
    /// The radar saw at least one person.
    Occupied,
}

/// A compact summary of the CSI frames inside one window.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsiWindowSummary {
    /// Frames in the window.
    pub n_frames: usize,
    /// Mean RSSI over frames that recorded one.
    pub rssi_mean: Option<f64>,
    /// Mean of per-frame mean amplitude.
    pub amplitude_mean: Option<f64>,
    /// Population variance of per-frame mean amplitude, a crude motion proxy.
    pub amplitude_var: Option<f64>,
}

fn mean_var(xs: &[f64]) -> (Option<f64>, Option<f64>) {
    if xs.is_empty() {
        return (None, None);
    }
    let n = xs.len() as f64;
    let m = xs.iter().sum::<f64>() / n;
    let v = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / n;
    (Some(m), Some(v))
}

fn summarize(frames: &[CsiFrame]) -> CsiWindowSummary {
    let rssi: Vec<f64> = frames.iter().filter_map(|f| f.rssi).collect();
    let amp: Vec<f64> = frames.iter().filter_map(|f| f.amplitude_mean).collect();
    let (amplitude_mean, amplitude_var) = mean_var(&amp);
    CsiWindowSummary {
        n_frames: frames.len(),
        rssi_mean: mean_var(&rssi).0,
        amplitude_mean,
        amplitude_var,
    }
}

fn claimed() -> EvidenceGrade {
    EvidenceGrade::Claimed
}

/// One labelled window of a recording.
///
/// The label comes from a radar, not a human, so `evidence` is always
/// [`EvidenceGrade::Claimed`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabelledWindow {
    /// Always [`EvidenceGrade::Claimed`] (radar-derived, not human ground truth).
    #[serde(default = "claimed")]
    pub evidence: EvidenceGrade,
    /// Session (recording) name; the unit of a disjoint split.
    pub session: String,
    /// Window start on the recording clock, Unix milliseconds.
    pub start_ms: i64,
    /// Window end on the recording clock, Unix milliseconds.
    pub end_ms: i64,
    /// Occupancy label.
    pub occupancy: Occupancy,
    /// Person count, when the radar reported one.
    pub person_count: Option<u32>,
    /// Lowest count the radar window carried.
    pub count_low: Option<u32>,
    /// Highest count the radar window carried.
    pub count_high: Option<u32>,
    /// Index of the radar window in its log.
    pub radar_window_index: usize,
    /// Summary of the CSI frames in the window.
    pub csi: CsiWindowSummary,
}

/// Why a radar window produced no labelled window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// The radar window had no label.
    RadarUnknown,
    /// Fewer CSI frames than `min_csi_frames` fell inside the window.
    TooFewCsiFrames,
    /// An empty label within `empty_hold_ms` of an occupied one.
    RecentlyOccupied,
}

/// How a session is labelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoLabelConfig {
    /// Added to radar timestamps to put them on the recording clock,
    /// milliseconds (recording clock minus radar clock).
    pub radar_offset_ms: i64,
    /// Fewest CSI frames a window needs to be kept.
    pub min_csi_frames: usize,
    /// An empty window starting within this long after an occupied window
    /// ends is dropped, milliseconds. `0` keeps every empty window.
    pub empty_hold_ms: i64,
}

impl Default for AutoLabelConfig {
    fn default() -> Self {
        Self {
            radar_offset_ms: 0,
            min_csi_frames: 5,
            empty_hold_ms: 5_000,
        }
    }
}

/// The labelled windows of one session plus what was dropped and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoLabelOutput {
    /// Session name.
    pub session: String,
    /// The configuration used, including the clock offset.
    pub config: AutoLabelConfig,
    /// Kept windows, in time order.
    pub windows: Vec<LabelledWindow>,
    /// Dropped radar windows by reason.
    pub skipped: BTreeMap<SkipReason, usize>,
    /// Unlabelled radar windows by the radar adapter's reason.
    pub radar_unknown: BTreeMap<UnknownReason, usize>,
}

/// Label one session.
///
/// `frames` need not be sorted. Radar labels are processed in time order.
///
/// # Errors
/// [`GroundTruthError::EmptyField`]/[`GroundTruthError::TooLong`] for a bad
/// session name, or [`GroundTruthError::InvalidConfig`] for a negative hold.
pub fn auto_label(
    session: &str,
    frames: &[CsiFrame],
    radar: &RadarLabels,
    cfg: AutoLabelConfig,
) -> Result<AutoLabelOutput, GroundTruthError> {
    check_nonempty("session", session)?;
    check_bound("session", session)?;
    if cfg.empty_hold_ms < 0 {
        return Err(GroundTruthError::InvalidConfig {
            reason: "empty_hold_ms must be non-negative",
        });
    }
    let mut frames = frames.to_vec();
    frames.sort_by_key(|f| f.at_unix_ms);

    let mut labels: Vec<_> = radar.labels.iter().collect();
    labels.sort_by_key(|l| (l.window_start_ms, l.window_end_ms));

    let mut out = AutoLabelOutput {
        session: session.to_owned(),
        config: cfg,
        windows: Vec::new(),
        skipped: BTreeMap::new(),
        radar_unknown: BTreeMap::new(),
    };
    let mut last_occupied_end: Option<i64> = None;

    for l in labels {
        if let LabelStatus::Unknown(r) = l.status {
            *out.radar_unknown.entry(r).or_default() += 1;
            *out.skipped.entry(SkipReason::RadarUnknown).or_default() += 1;
            continue;
        }
        let Some(present) = l.presence else {
            *out.skipped.entry(SkipReason::RadarUnknown).or_default() += 1;
            continue;
        };
        let start = l.window_start_ms.saturating_add(cfg.radar_offset_ms);
        let end = l.window_end_ms.saturating_add(cfg.radar_offset_ms);
        let occupancy = if present {
            Occupancy::Occupied
        } else {
            Occupancy::Empty
        };

        if occupancy == Occupancy::Occupied {
            last_occupied_end = Some(end);
        } else if matches!(last_occupied_end, Some(e) if start.saturating_sub(e) < cfg.empty_hold_ms)
        {
            *out.skipped.entry(SkipReason::RecentlyOccupied).or_default() += 1;
            continue;
        }

        // Half-open [start, end); a zero-length window takes the frames at `start`.
        let lo = frames.partition_point(|f| f.at_unix_ms < start);
        let hi = if end > start {
            frames.partition_point(|f| f.at_unix_ms < end)
        } else {
            frames.partition_point(|f| f.at_unix_ms <= start)
        };
        let inside = &frames[lo..hi];
        if inside.len() < cfg.min_csi_frames.max(1) {
            *out.skipped.entry(SkipReason::TooFewCsiFrames).or_default() += 1;
            continue;
        }

        out.windows.push(LabelledWindow {
            evidence: l.evidence,
            session: session.to_owned(),
            start_ms: start,
            end_ms: end,
            occupancy,
            person_count: l.person_count,
            count_low: l.uncertainty.count_low,
            count_high: l.uncertainty.count_high,
            radar_window_index: l.window_index,
            csi: summarize(inside),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::radar::{RadarAdapterConfig, RadarWindow, RadarWindowSource};

    /// Synthetic CSI at 10 Hz from `t0` for `secs` seconds.
    fn csi(t0_ms: i64, secs: i64) -> Vec<CsiFrame> {
        (0..secs * 10)
            .map(|i| CsiFrame {
                at_unix_ms: t0_ms + i * 100,
                rssi: Some(-50.0),
                amplitude_mean: Some(10.0 + (i % 3) as f64),
            })
            .collect()
    }

    fn radar(t0_ms: i64, counts: &[Option<u32>]) -> RadarLabels {
        let windows: Vec<RadarWindow> = counts
            .iter()
            .enumerate()
            .map(|(i, c)| RadarWindow {
                timestamp_ms: t0_ms + (i as i64 + 1) * 1000,
                window_start_ms: Some(t0_ms + i as i64 * 1000),
                health: Some("ok".into()),
                quality: Some(1.0),
                source: Some(RadarWindowSource {
                    verified: c.is_some(),
                    simulated: false,
                }),
                presence: None,
                target_count: Some(c.unwrap_or(0)),
                max_target_count: None,
                distance_cm: None,
                targets: Vec::new(),
            })
            .collect();
        RadarLabels::from_windows("radar-1", "ld2450", &windows, RadarAdapterConfig::default())
            .unwrap()
    }

    #[test]
    fn parses_recording_lines() {
        let text = r#"{"timestamp":1700000000.25,"subcarriers":[1.0,-3.0],"rssi":-48.0,"noise_floor":-90.0,"features":{}}

{"timestamp":1700000000.35,"subcarriers":[],"rssi":-49.0,"noise_floor":-90.0,"features":{}}"#;
        let f = parse_recording_jsonl(text).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].at_unix_ms, 1_700_000_000_250);
        assert_eq!(f[0].amplitude_mean, Some(2.0));
        assert_eq!(f[1].amplitude_mean, None);
    }

    #[test]
    fn bad_recording_rows_are_numbered() {
        let err = parse_recording_jsonl("{\"timestamp\":1.0}\nnope").unwrap_err();
        assert!(matches!(err, GroundTruthError::InvalidRow { row: 2, .. }));
        let err = parse_recording_jsonl("{\"timestamp\":-1.0}").unwrap_err();
        assert!(matches!(err, GroundTruthError::InvalidRow { row: 1, .. }));
    }

    #[test]
    fn labels_follow_radar_windows_and_count_frames() {
        let r = radar(0, &[Some(0), Some(1), Some(2)]);
        let cfg = AutoLabelConfig {
            empty_hold_ms: 0,
            ..AutoLabelConfig::default()
        };
        let out = auto_label("s1", &csi(0, 3), &r, cfg).unwrap();
        assert_eq!(out.windows.len(), 3);
        assert_eq!(out.windows[0].occupancy, Occupancy::Empty);
        assert_eq!(out.windows[2].person_count, Some(2));
        assert!(out.windows.iter().all(|w| w.csi.n_frames == 10));
        assert_eq!(out.windows[1].csi.rssi_mean, Some(-50.0));
        assert!(out.skipped.is_empty());
    }

    #[test]
    fn clock_offset_moves_radar_onto_the_recording() {
        // Radar clock runs 60 s behind the recording.
        let r = radar(0, &[Some(1), Some(1)]);
        let frames = csi(60_000, 2);
        let none = auto_label("s1", &frames, &r, AutoLabelConfig::default()).unwrap();
        assert_eq!(none.windows.len(), 0);
        assert_eq!(none.skipped.get(&SkipReason::TooFewCsiFrames), Some(&2));

        let cfg = AutoLabelConfig {
            radar_offset_ms: 60_000,
            ..AutoLabelConfig::default()
        };
        let out = auto_label("s1", &frames, &r, cfg).unwrap();
        assert_eq!(out.windows.len(), 2);
        assert_eq!(out.windows[0].start_ms, 60_000);
    }

    #[test]
    fn short_empty_gap_after_occupancy_is_dropped() {
        let r = radar(
            0,
            &[
                Some(1),
                Some(0),
                Some(0),
                Some(0),
                Some(0),
                Some(0),
                Some(0),
            ],
        );
        let cfg = AutoLabelConfig {
            empty_hold_ms: 3_000,
            ..AutoLabelConfig::default()
        };
        let out = auto_label("s1", &csi(0, 7), &r, cfg).unwrap();
        // Occupied ends at 1000; empties starting at 1000, 2000, 3000 are within 3 s.
        assert_eq!(out.skipped.get(&SkipReason::RecentlyOccupied), Some(&3));
        let kept: Vec<_> = out
            .windows
            .iter()
            .map(|w| (w.start_ms, w.occupancy))
            .collect();
        assert_eq!(
            kept,
            [
                (0, Occupancy::Occupied),
                (4000, Occupancy::Empty),
                (5000, Occupancy::Empty),
                (6000, Occupancy::Empty)
            ]
        );
    }

    #[test]
    fn unknown_radar_windows_are_counted_by_reason() {
        let r = radar(0, &[Some(1), None, Some(1)]);
        let out = auto_label("s1", &csi(0, 3), &r, AutoLabelConfig::default()).unwrap();
        assert_eq!(out.windows.len(), 2);
        assert_eq!(out.skipped.get(&SkipReason::RadarUnknown), Some(&1));
        assert_eq!(out.radar_unknown.get(&UnknownReason::Unverified), Some(&1));
    }

    #[test]
    fn bad_inputs_are_rejected() {
        let r = radar(0, &[Some(1)]);
        assert!(auto_label("", &[], &r, AutoLabelConfig::default()).is_err());
        let cfg = AutoLabelConfig {
            empty_hold_ms: -1,
            ..AutoLabelConfig::default()
        };
        assert!(auto_label("s", &[], &r, cfg).is_err());
    }
}
