//! mmWave radar windows as reference labels (ADR-303 §1).
//!
//! A radar reader (an LD2450-class target tracker, an LD6002/MR60-class vitals
//! radar, or anything that writes the same shape) summarises its UART stream
//! into one JSON object per time window. This module turns those windows into
//! reference labels for presence, person count and nearest-target range, each
//! carrying where it came from and how uncertain it is, and then into the
//! crate's [`ReferenceSeries`] so the usual alignment and agreement machinery
//! applies.
//!
//! The radar stays on the validation plane: nothing here feeds an estimator.
//!
//! ## Input shape
//!
//! One JSON object per line. Every field except `timestamp_ms` is optional and
//! unknown fields are ignored, so a reader that reports less (no targets, no
//! presence flag) still parses:
//!
//! ```json
//! {"timestamp_ms": 1700000001000, "window_start_ms": 1700000000000,
//!  "health": "ok", "quality": 1.0,
//!  "source": {"verified": true, "simulated": false},
//!  "presence": true, "target_count": 2, "max_target_count": 2,
//!  "distance_cm": 140.0,
//!  "targets": [{"x_m": 0.4, "y_m": 1.3}, {"x_m": -0.9, "y_m": 2.2}]}
//! ```
//!
//! ## What a label means
//!
//! - **Presence** comes from the explicit `presence` flag when there is one,
//!   otherwise from the count. If the flag and the count disagree, the window is
//!   [`LabelStatus::Unknown`], never resolved by guessing.
//! - **Count** is reported as an interval: the low and high of every count the
//!   window carries (`target_count`, `max_target_count`, the number of
//!   `targets`). The label value is the latest-frame count when present. A
//!   radar that tracks moving targets can miss a person who is sitting still,
//!   so a count is a lower bound in practice; the interval only captures
//!   disagreement inside the window.
//! - **Range** is the nearest target's planar distance `sqrt(x² + y²)`, or
//!   `distance_cm` when no target positions are given. Its uncertainty is the
//!   configured `range_sigma_m`, which is a datasheet figure (CLAIMED), not a
//!   measured one.
//! - A window from a simulated source, an unverified source, or a reader that
//!   reports `health: "no_source"` is `Unknown` with a reason.

use serde::{Deserialize, Serialize};

use crate::agreement::EvidenceGrade;
use crate::error::{check_bound, GroundTruthError};
use crate::model::Measurand;
use crate::series::{ReferenceObservation, ReferenceSeries, MAX_SAMPLES};
use crate::source::{ReferenceModality, ReferenceSource};

/// The largest single JSONL line accepted, in bytes, bounding allocation.
pub const MAX_RADAR_LINE_BYTES: usize = 64 * 1024;
/// The most targets read from one window; extra entries are ignored.
pub const MAX_RADAR_TARGETS: usize = 64;
/// Label used for an occupied window in a presence series.
pub const LABEL_PRESENT: &str = "present";
/// Label used for an empty window in a presence series.
pub const LABEL_ABSENT: &str = "absent";

/// One target position in the radar's local frame, metres (x across, y away).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadarTarget {
    /// Lateral offset, metres.
    pub x_m: f64,
    /// Distance away from the radar face, metres.
    pub y_m: f64,
}

/// Where a radar window came from, as reported by its reader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadarWindowSource {
    /// True only when checksum-valid frames arrived from real hardware.
    #[serde(default)]
    pub verified: bool,
    /// True when the window came from a simulator or replay generator.
    #[serde(default)]
    pub simulated: bool,
}

/// One radar summary window as written by a reader.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadarWindow {
    /// End of the window, Unix milliseconds.
    pub timestamp_ms: i64,
    /// Start of the window, Unix milliseconds.
    #[serde(default)]
    pub window_start_ms: Option<i64>,
    /// Reader health: `ok`, `degraded` or `no_source`.
    #[serde(default)]
    pub health: Option<String>,
    /// Reader-reported frame quality in `[0, 1]`.
    #[serde(default)]
    pub quality: Option<f64>,
    /// Source flags.
    #[serde(default)]
    pub source: Option<RadarWindowSource>,
    /// Explicit presence flag (vitals-class radars).
    #[serde(default)]
    pub presence: Option<bool>,
    /// Targets in the latest frame of the window.
    #[serde(default)]
    pub target_count: Option<u32>,
    /// Most targets in any frame of the window.
    #[serde(default)]
    pub max_target_count: Option<u32>,
    /// Distance to the nearest target, centimetres.
    #[serde(default)]
    pub distance_cm: Option<f64>,
    /// Target positions.
    #[serde(default)]
    pub targets: Vec<RadarTarget>,
}

impl RadarWindow {
    /// The time a label from this window is stamped at: the window midpoint
    /// when the start is known, else the window end.
    #[must_use]
    pub fn label_time_ms(&self) -> i64 {
        match self.window_start_ms {
            Some(s) if s <= self.timestamp_ms => s + (self.timestamp_ms - s) / 2,
            _ => self.timestamp_ms,
        }
    }
}

/// Parse radar windows from JSONL text. Blank lines are skipped. A line that
/// is not a valid window is a row-numbered error, never silently dropped.
///
/// # Errors
/// [`GroundTruthError::InvalidRow`] for an over-long or malformed line, and
/// [`GroundTruthError::TooManySamples`] past [`MAX_SAMPLES`] windows.
pub fn parse_radar_jsonl(text: &str) -> Result<Vec<RadarWindow>, GroundTruthError> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let row = i + 1;
        if line.len() > MAX_RADAR_LINE_BYTES {
            return Err(GroundTruthError::InvalidRow {
                row,
                reason: format!("line is {} bytes, max {MAX_RADAR_LINE_BYTES}", line.len()),
            });
        }
        let mut w: RadarWindow =
            serde_json::from_str(line).map_err(|e| GroundTruthError::InvalidRow {
                row,
                reason: e.to_string(),
            })?;
        w.targets.truncate(MAX_RADAR_TARGETS);
        if let Some(h) = &w.health {
            check_bound("health", h)?;
        }
        out.push(w);
        if out.len() > MAX_SAMPLES {
            return Err(GroundTruthError::TooManySamples {
                len: out.len(),
                max: MAX_SAMPLES,
            });
        }
    }
    Ok(out)
}

/// How radar windows are turned into labels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadarAdapterConfig {
    /// One-sigma range uncertainty of a single target, metres. A datasheet
    /// figure; it is reported with every range label.
    pub range_sigma_m: f64,
    /// Targets farther than this are ignored as implausible, metres.
    pub max_range_m: f64,
    /// Refuse windows whose source is not flagged `verified`.
    pub require_verified: bool,
    /// Accept windows from a simulated source. Off by default: a simulated
    /// radar is not ground truth.
    pub allow_simulated: bool,
    /// Windows below this reader quality are refused. `0.0` accepts all.
    pub min_quality: f64,
}

impl Default for RadarAdapterConfig {
    fn default() -> Self {
        Self {
            range_sigma_m: 0.15,
            max_range_m: 10.0,
            require_verified: true,
            allow_simulated: false,
            min_quality: 0.0,
        }
    }
}

impl RadarAdapterConfig {
    /// Validate the configuration.
    ///
    /// # Errors
    /// [`GroundTruthError::InvalidConfig`] for a non-finite or negative sigma,
    /// a non-positive range limit, or a quality floor outside `[0, 1]`.
    pub fn validate(&self) -> Result<(), GroundTruthError> {
        if !self.range_sigma_m.is_finite() || self.range_sigma_m < 0.0 {
            return Err(GroundTruthError::InvalidConfig {
                reason: "range_sigma_m must be finite and non-negative",
            });
        }
        if !self.max_range_m.is_finite() || self.max_range_m <= 0.0 {
            return Err(GroundTruthError::InvalidConfig {
                reason: "max_range_m must be finite and positive",
            });
        }
        if !(0.0..=1.0).contains(&self.min_quality) {
            return Err(GroundTruthError::InvalidConfig {
                reason: "min_quality must be within [0, 1]",
            });
        }
        Ok(())
    }
}

/// Why a window produced no label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownReason {
    /// The reader had no sensor data for the window.
    NoSource,
    /// The source was not flagged as verified hardware.
    Unverified,
    /// The source was a simulator.
    Simulated,
    /// The reader's quality was below the configured floor.
    LowQuality,
    /// The presence flag and the target count disagree.
    PresenceCountConflict,
    /// The window carried neither a presence flag nor any count.
    NoObservation,
    /// The window's timestamps were inconsistent (start after end).
    BadTimestamps,
}

/// Whether a window yielded a label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status", content = "reason")]
pub enum LabelStatus {
    /// The window produced a label.
    Labelled,
    /// The window produced no label; UNKNOWN is a value, not an error.
    Unknown(UnknownReason),
}

/// Uncertainty attached to one radar label.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabelUncertainty {
    /// Lowest count the window carries.
    pub count_low: Option<u32>,
    /// Highest count the window carries.
    pub count_high: Option<u32>,
    /// One-sigma range uncertainty, metres (from configuration, CLAIMED).
    pub range_sigma_m: Option<f64>,
    /// Reader-reported quality, when given.
    pub quality: Option<f64>,
}

/// Where a radar label came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceBasis {
    /// The window's explicit presence flag (agreeing with the count, if any).
    PresenceFlag,
    /// Derived from the target count.
    TargetCount,
}

fn claimed() -> EvidenceGrade {
    EvidenceGrade::Claimed
}

/// One radar window turned into reference labels.
///
/// Radar labels are **not** human ground truth: a tracker misses stationary
/// people and can ghost on clutter, and its range sigma is a datasheet figure.
/// Every label therefore carries [`EvidenceGrade::Claimed`] in `evidence`; it
/// is never `Measured` until a labeller checks it against an independent
/// human count.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadarLabel {
    /// Always [`EvidenceGrade::Claimed`] (radar-derived, not human-verified).
    #[serde(default = "claimed")]
    pub evidence: EvidenceGrade,
    /// Label time, Unix milliseconds (window midpoint, see
    /// [`RadarWindow::label_time_ms`]).
    pub at_unix_ms: i64,
    /// Window start, Unix milliseconds (the end when the reader omits it).
    pub window_start_ms: i64,
    /// Window end, Unix milliseconds.
    pub window_end_ms: i64,
    /// Whether a label was produced.
    pub status: LabelStatus,
    /// Someone present.
    pub presence: Option<bool>,
    /// What presence was derived from.
    pub presence_basis: Option<PresenceBasis>,
    /// Person count (latest-frame count when given).
    pub person_count: Option<u32>,
    /// Nearest-target planar range, metres.
    pub nearest_range_m: Option<f64>,
    /// Uncertainty of the above.
    pub uncertainty: LabelUncertainty,
    /// 0-based index of the source window in its log.
    pub window_index: usize,
}

impl RadarLabel {
    fn unknown(w: &RadarWindow, index: usize, reason: UnknownReason) -> Self {
        Self {
            evidence: EvidenceGrade::Claimed,
            at_unix_ms: w.label_time_ms(),
            window_start_ms: w.window_start_ms.unwrap_or(w.timestamp_ms),
            window_end_ms: w.timestamp_ms,
            status: LabelStatus::Unknown(reason),
            presence: None,
            presence_basis: None,
            person_count: None,
            nearest_range_m: None,
            uncertainty: LabelUncertainty {
                count_low: None,
                count_high: None,
                range_sigma_m: None,
                quality: w.quality,
            },
            window_index: index,
        }
    }
}

/// Turn one radar window into a label.
#[must_use]
pub fn label_window(w: &RadarWindow, index: usize, cfg: &RadarAdapterConfig) -> RadarLabel {
    if matches!(w.window_start_ms, Some(s) if s > w.timestamp_ms) {
        return RadarLabel::unknown(w, index, UnknownReason::BadTimestamps);
    }
    if w.health.as_deref() == Some("no_source") {
        return RadarLabel::unknown(w, index, UnknownReason::NoSource);
    }
    let src = w.source.unwrap_or_default();
    if src.simulated && !cfg.allow_simulated {
        return RadarLabel::unknown(w, index, UnknownReason::Simulated);
    }
    if cfg.require_verified && !src.verified && !(src.simulated && cfg.allow_simulated) {
        return RadarLabel::unknown(w, index, UnknownReason::Unverified);
    }
    if cfg.min_quality > 0.0 && !matches!(w.quality, Some(q) if q >= cfg.min_quality) {
        return RadarLabel::unknown(w, index, UnknownReason::LowQuality);
    }

    let in_range: Vec<&RadarTarget> = w
        .targets
        .iter()
        .filter(|t| t.x_m.is_finite() && t.y_m.is_finite())
        .filter(|t| t.x_m.hypot(t.y_m) <= cfg.max_range_m)
        .collect();

    let positions = (!w.targets.is_empty()).then_some(in_range.len() as u32);
    let counts = [w.target_count, w.max_target_count, positions];
    let count_low = counts.iter().flatten().copied().min();
    let count_high = counts.iter().flatten().copied().max();
    let person_count = w.target_count.or(positions).or(w.max_target_count);

    let (presence, basis) = match (w.presence, person_count) {
        (Some(p), Some(c)) if p != (c > 0) => {
            return RadarLabel::unknown(w, index, UnknownReason::PresenceCountConflict);
        }
        (Some(p), _) => (p, PresenceBasis::PresenceFlag),
        (None, Some(c)) => (c > 0, PresenceBasis::TargetCount),
        (None, None) => return RadarLabel::unknown(w, index, UnknownReason::NoObservation),
    };

    let nearest_range_m = in_range
        .iter()
        .map(|t| t.x_m.hypot(t.y_m))
        .fold(None, |acc: Option<f64>, r| {
            Some(acc.map_or(r, |a| a.min(r)))
        })
        .or_else(|| {
            w.distance_cm
                .filter(|d| presence && d.is_finite() && *d > 0.0)
                .map(|d| d / 100.0)
                .filter(|r| *r <= cfg.max_range_m)
        });

    RadarLabel {
        evidence: EvidenceGrade::Claimed,
        at_unix_ms: w.label_time_ms(),
        window_start_ms: w.window_start_ms.unwrap_or(w.timestamp_ms),
        window_end_ms: w.timestamp_ms,
        status: LabelStatus::Labelled,
        presence: Some(presence),
        presence_basis: Some(basis),
        person_count,
        nearest_range_m,
        uncertainty: LabelUncertainty {
            count_low,
            count_high,
            range_sigma_m: nearest_range_m.map(|_| cfg.range_sigma_m),
            quality: w.quality,
        },
        window_index: index,
    }
}

/// Labels for a whole radar log, with the source they are attributed to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadarLabels {
    /// The reference source (always [`ReferenceModality::MmWave`]).
    pub source: ReferenceSource,
    /// The configuration the labels were made with.
    pub config: RadarAdapterConfig,
    /// One label per window, in log order.
    pub labels: Vec<RadarLabel>,
}

impl RadarLabels {
    /// Label every window of a radar log.
    ///
    /// # Errors
    /// [`GroundTruthError::InvalidConfig`] for a bad configuration, or a
    /// metadata error from [`ReferenceSource::new`].
    pub fn from_windows(
        name: &str,
        device: &str,
        windows: &[RadarWindow],
        cfg: RadarAdapterConfig,
    ) -> Result<Self, GroundTruthError> {
        cfg.validate()?;
        let source = ReferenceSource::new(ReferenceModality::MmWave, name, device, "fmcw-radar")?;
        let labels = windows
            .iter()
            .enumerate()
            .map(|(i, w)| label_window(w, i, &cfg))
            .collect();
        Ok(Self {
            source,
            config: cfg,
            labels,
        })
    }

    /// Labelled windows only.
    pub fn labelled(&self) -> impl Iterator<Item = &RadarLabel> {
        self.labels
            .iter()
            .filter(|l| l.status == LabelStatus::Labelled)
    }

    fn series(
        &self,
        measurand: Measurand,
        f: impl Fn(&RadarLabel) -> Option<ReferenceObservation>,
    ) -> Result<ReferenceSeries, GroundTruthError> {
        let samples = self.labelled().filter_map(f).collect();
        ReferenceSeries::new(self.source.clone(), measurand, samples)
    }

    /// A [`Measurand::Presence`] series of `present`/`absent` labels.
    ///
    /// # Errors
    /// As [`ReferenceSeries::new`] (an empty or non-monotonic series).
    pub fn presence_series(&self) -> Result<ReferenceSeries, GroundTruthError> {
        self.series(Measurand::Presence, |l| {
            l.presence.map(|p| {
                ReferenceObservation::label(
                    l.at_unix_ms,
                    if p { LABEL_PRESENT } else { LABEL_ABSENT },
                )
            })
        })
    }

    /// A [`Measurand::PersonCount`] series.
    ///
    /// # Errors
    /// As [`ReferenceSeries::new`].
    pub fn person_count_series(&self) -> Result<ReferenceSeries, GroundTruthError> {
        self.series(Measurand::PersonCount, |l| {
            l.person_count
                .map(|c| ReferenceObservation::scalar(l.at_unix_ms, f64::from(c)))
        })
    }

    /// A [`Measurand::RangeMeters`] series of nearest-target range.
    ///
    /// # Errors
    /// As [`ReferenceSeries::new`].
    pub fn range_series(&self) -> Result<ReferenceSeries, GroundTruthError> {
        self.series(Measurand::RangeMeters, |l| {
            l.nearest_range_m
                .map(|r| ReferenceObservation::scalar(l.at_unix_ms, r))
        })
    }
}

#[cfg(test)]
mod tests;
