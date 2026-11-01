//! Fill-mode gate.
//!
//! Fill mode would write solved positions to a file the operator then passes
//! to `--node-positions`. It is not implemented here. This gate records the
//! ADR-381 rule so that whatever implements it later cannot be enabled on a
//! source that has not earned it: MEASURED evidence, enough links, and a p90
//! link error of at most [`FILL_MAX_P90_M`].

use serde::{Deserialize, Serialize};

use crate::RangeMethod;

/// Largest p90 link error, in metres, at which fill mode may be enabled.
pub const FILL_MAX_P90_M: f64 = 0.3;

/// Fewest ground-truth links behind the p90 figure.
pub const FILL_MIN_LINKS: usize = 6;

/// Evidence tag of a source accuracy figure (repository rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceTag {
    /// Our own bench, with a reproducer.
    Measured,
    /// Vendor or paper figure we have not reproduced.
    Claimed,
    /// Simulation.
    Synthetic,
}

/// Accuracy evidence for one ranging source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceEvidence {
    /// The source.
    pub method: RangeMethod,
    /// p90 of |measured − ground truth| over the links below, in metres.
    pub p90_link_error_m: f64,
    /// Number of ground-truth links behind the figure.
    pub links: usize,
    /// Evidence tag.
    pub tag: EvidenceTag,
}

/// Why fill mode stays off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum FillBlocked {
    /// No evidence was supplied for the source.
    NoEvidence,
    /// The evidence is not MEASURED.
    NotMeasured {
        /// The tag that was supplied.
        tag: EvidenceTag,
    },
    /// Too few ground-truth links.
    TooFewLinks {
        /// Links supplied.
        links: usize,
    },
    /// p90 link error above [`FILL_MAX_P90_M`].
    TooInaccurate {
        /// p90 supplied.
        p90_link_error_m: f64,
    },
}

/// Decide whether fill mode may run for a source.
pub fn fill_gate(evidence: Option<&SourceEvidence>) -> Result<(), FillBlocked> {
    let e = evidence.ok_or(FillBlocked::NoEvidence)?;
    if e.tag != EvidenceTag::Measured {
        return Err(FillBlocked::NotMeasured { tag: e.tag });
    }
    if e.links < FILL_MIN_LINKS {
        return Err(FillBlocked::TooFewLinks { links: e.links });
    }
    if !e.p90_link_error_m.is_finite() || e.p90_link_error_m > FILL_MAX_P90_M {
        return Err(FillBlocked::TooInaccurate {
            p90_link_error_m: e.p90_link_error_m,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(p90: f64, links: usize, tag: EvidenceTag) -> SourceEvidence {
        SourceEvidence {
            method: RangeMethod::Uwb,
            p90_link_error_m: p90,
            links,
            tag,
        }
    }

    #[test]
    fn gate_blocks_until_measured_and_accurate() {
        assert_eq!(fill_gate(None), Err(FillBlocked::NoEvidence));
        assert!(matches!(
            fill_gate(Some(&ev(0.1, 10, EvidenceTag::Claimed))),
            Err(FillBlocked::NotMeasured { .. })
        ));
        assert!(matches!(
            fill_gate(Some(&ev(0.1, 3, EvidenceTag::Measured))),
            Err(FillBlocked::TooFewLinks { .. })
        ));
        // FTM on the 2026-10-04 bench: errors of metres.
        assert!(matches!(
            fill_gate(Some(&ev(4.1, 8, EvidenceTag::Measured))),
            Err(FillBlocked::TooInaccurate { .. })
        ));
        assert!(fill_gate(Some(&ev(f64::NAN, 8, EvidenceTag::Measured))).is_err());
        assert_eq!(fill_gate(Some(&ev(0.3, 6, EvidenceTag::Measured))), Ok(()));
    }
}
