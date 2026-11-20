//! Check-mode parameters and report types.

use serde::Serialize;

/// Tolerances and gates for [`crate::check`].
///
/// A link's sigma is the larger of the median per-session sigma and, with
/// three or more sessions, the spread of the session ranges. Its tolerance
/// is `floor + sigma_k × sigma`, where `floor` is
/// [`CheckParams::calibrated_floor_m`] when every observation on the link was
/// calibrated for the current responder boot and [`CheckParams::floor_m`]
/// otherwise. A link whose sigma exceeds [`CheckParams::max_sigma_m`] is not
/// scored: a wide tolerance would otherwise accept anything.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckParams {
    /// Tolerance floor for uncalibrated links, in metres.
    pub floor_m: f64,
    /// Tolerance floor for calibrated links, in metres.
    pub calibrated_floor_m: f64,
    /// Multiplier on the per-link sigma.
    pub sigma_k: f64,
    /// Links whose configured length is below this are not scored.
    pub min_expected_m: f64,
    /// Observations below this RSSI are dropped.
    pub min_rssi_dbm: i16,
    /// Links with a sigma above this are not scored.
    pub max_sigma_m: f64,
    /// A swap is suggested only if it cuts the assignment cost to at most
    /// this fraction of the configured assignment's cost.
    pub swap_gain: f64,
    /// A swap is suggested only if the configured assignment's cost is at
    /// least this (in squared tolerances).
    pub min_swap_cost: f64,
    /// Relative band around a unit factor that counts as a unit match.
    pub unit_band: f64,
}

impl CheckParams {
    /// Wi-Fi FTM, ESP32-S3, 20 MHz, from the 2026-10-04 bench (MEASURED).
    ///
    /// - `floor_m = 2.0`: bias between responder starts at 1.14 m ranged
    ///   from −0.5 to +1.9 m. One start at 3.71 m read +4.1 m and will be
    ///   flagged as a link outlier; that is the honest state of uncalibrated
    ///   FTM, so a single-link outlier is reported as such and not as a
    ///   configuration error.
    /// - `calibrated_floor_m = 0.6`: spread within one start was ±0.2 to
    ///   0.4 m with no drift beyond that over two minutes. Longer stability is untested
    ///   (ESTIMATE).
    /// - `min_expected_m = 1.0`: readings clamped to 0 below about 0.8 m.
    /// - `min_rssi_dbm = -70`: Espressif's guidance. The bench's 7.7 m
    ///   placement outlier had a median of −68 dBm and passed this gate.
    /// - `max_sigma_m = 1.0`: that outlier's session spread was 1.92 m; every
    ///   other run's spread was 0.49 m or less.
    pub fn ftm_uncalibrated() -> Self {
        Self {
            floor_m: 2.0,
            calibrated_floor_m: 0.6,
            sigma_k: 3.0,
            min_expected_m: 1.0,
            min_rssi_dbm: -70,
            max_sigma_m: 1.0,
            swap_gain: 0.5,
            min_swap_cost: 1.0,
            unit_band: 0.2,
        }
    }

    /// UWB two-way ranging. ESTIMATE pending our own bench: published
    /// line-of-sight accuracy is about 0.1 m (CLAIMED).
    pub fn uwb() -> Self {
        Self {
            floor_m: 0.3,
            calibrated_floor_m: 0.3,
            sigma_k: 3.0,
            min_expected_m: 0.0,
            min_rssi_dbm: i16::MIN,
            max_sigma_m: 0.3,
            swap_gain: 0.5,
            min_swap_cost: 1.0,
            unit_band: 0.1,
        }
    }
}

/// Status of one link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkStatus {
    /// Residual within tolerance.
    Ok,
    /// Residual beyond tolerance.
    Outlier,
    /// Configured length below `min_expected_m`; not scored.
    TooShort,
    /// Every observation was below `min_rssi_dbm`; not scored.
    Weak,
    /// Sigma above `max_sigma_m`; not scored.
    Noisy,
}

/// Measured versus configured length of one node pair.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LinkResidual {
    /// Smaller node id.
    pub a: u8,
    /// Larger node id.
    pub b: u8,
    /// Median measured range, metres.
    pub measured_m: f64,
    /// Distance between the configured positions, metres.
    pub expected_m: f64,
    /// `measured_m − expected_m`.
    pub residual_m: f64,
    /// Tolerance applied.
    pub tolerance_m: f64,
    /// Link sigma (see [`CheckParams`]).
    pub sigma_m: f64,
    /// Observations behind the median.
    pub observations: usize,
    /// True if every observation used was calibrated.
    pub calibrated: bool,
    /// Status.
    pub status: LinkStatus,
}

/// Least-squares scale `s` in `measured ≈ s × expected`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScaleFit {
    /// Fitted scale.
    pub scale: f64,
    /// Links used.
    pub links: usize,
}

/// A warning or note from [`crate::check`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Finding {
    /// Observations name a node that has no configured position.
    UnknownNode {
        /// Node id.
        node: u8,
        /// Observations dropped.
        observations: usize,
    },
    /// A configured node has no scored link, so it was not checked.
    UnrangedNode {
        /// Node id.
        node: u8,
    },
    /// The fitted scale matches a unit factor: positions were probably
    /// entered in that unit while the server reads metres.
    UnitMismatch {
        /// Unit name.
        unit: String,
        /// Factor from that unit to metres.
        factor: f64,
        /// Fitted scale.
        fitted_scale: f64,
    },
    /// Another assignment of the configured positions fits much better.
    /// Each pair is `(node, position_of)`: the node's ranges match the
    /// position configured for `position_of`. When several assignments fit
    /// equally well, this is the one that changes the fewest ids.
    IdSwapSuggested {
        /// Changed assignments only.
        assignment: Vec<(u8, u8)>,
        /// Cost of the configured assignment.
        cost_configured: f64,
        /// Cost of the suggested assignment.
        cost_suggested: f64,
        /// Assignments that fit as well as the suggestion, itself included.
        /// Above 1, the layout is (nearly) symmetric and the suggestion
        /// rests on the fewest-changes rule.
        equivalent_assignments: usize,
    },
    /// The configured assignment fits badly and the best-fitting
    /// assignments that change the fewest ids are tied (symmetric layout);
    /// no single swap can be named.
    IdSwapAmbiguous {
        /// Number of equally good alternative assignments.
        equivalent_assignments: usize,
    },
    /// Most of a node's links disagree: it was probably moved.
    NodeMoved {
        /// Node id.
        node: u8,
        /// Outlier links touching it.
        flagged_links: usize,
        /// Scored links touching it.
        links: usize,
    },
    /// One link is out of tolerance with no configuration explanation
    /// (bias, a body or wall in the path, or a bad session).
    LinkOutlier {
        /// Smaller node id.
        a: u8,
        /// Larger node id.
        b: u8,
        /// Residual, metres.
        residual_m: f64,
    },
    /// Too many nodes for the exhaustive id-swap search.
    SwapSearchSkipped {
        /// Nodes with scored links.
        nodes: usize,
    },
}

impl Finding {
    /// True for findings that should raise the verdict to warnings.
    pub fn is_warning(&self) -> bool {
        !matches!(
            self,
            Finding::UnrangedNode { .. } | Finding::SwapSearchSkipped { .. }
        )
    }
}

/// Overall result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Every scored link agrees with the configuration.
    Consistent,
    /// At least one warning finding.
    Warnings,
    /// No link could be scored.
    Insufficient,
}

/// Result of [`crate::check`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckReport {
    /// One entry per observed pair whose nodes are both configured.
    pub links: Vec<LinkResidual>,
    /// Scale fit over scored links, when at least two exist.
    pub scale: Option<ScaleFit>,
    /// Findings, in the order they were derived.
    pub findings: Vec<Finding>,
    /// Verdict.
    pub verdict: Verdict,
}
