//! Boundary errors for the ground-truth validation plane (ADR-303).
//!
//! No variant panics: malformed reference/estimate input is always a returned
//! error, and UNKNOWN/uncertainty are represented as first-class *values*
//! elsewhere (an inconclusive [`crate::AgreementReport`] with zero pairs), not
//! as errors. `EvidenceError` from the ledger boundary is wrapped transparently
//! so a caller sees one error type.

/// Maximum accepted string-handle length, in bytes. Mirrors
/// [`ruview_ontology::MAX_ID_LEN`] and bounds allocation on untrusted input.
pub const MAX_STR_LEN: usize = ruview_ontology::MAX_ID_LEN;

/// Errors raised while ingesting references/estimates, aligning them, or
/// emitting an evidence record. Every variant is a returned error, never a
/// panic (CLAUDE.md).
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum GroundTruthError {
    /// A required string field was empty.
    #[error("field `{field}` must not be empty")]
    EmptyField {
        /// The offending field name.
        field: &'static str,
    },
    /// A string field exceeded [`MAX_STR_LEN`] bytes.
    #[error("field `{field}` is {len} bytes, exceeds max {max}")]
    TooLong {
        /// The offending field name.
        field: &'static str,
        /// Actual byte length.
        len: usize,
        /// Enforced maximum.
        max: usize,
    },
    /// A series carried no samples; a reference/estimate must have at least one.
    #[error("series has no samples")]
    EmptySeries,
    /// A series exceeded the bounded sample cap.
    #[error("series has {len} samples, exceeds max {max}")]
    TooManySamples {
        /// Actual sample count.
        len: usize,
        /// Enforced maximum.
        max: usize,
    },
    /// Timestamps were not strictly increasing — rejected, never silently
    /// sorted (ADR-293 ingest discipline).
    #[error("non-monotonic timestamp at sample {index}: {this_ms} does not follow {prev_ms}")]
    NonMonotonic {
        /// Index of the offending sample.
        index: usize,
        /// Previous sample timestamp.
        prev_ms: i64,
        /// Offending sample timestamp.
        this_ms: i64,
    },
    /// A continuous reading carried a non-finite value.
    #[error("non-finite value at sample {index}")]
    NonFiniteValue {
        /// Index of the offending sample.
        index: usize,
    },
    /// A sample's reading kind (scalar vs label) did not match the measurand's
    /// family.
    #[error("sample {index}: reading kind does not match measurand `{measurand}`")]
    ReadingKindMismatch {
        /// Index of the offending sample.
        index: usize,
        /// The declared measurand.
        measurand: &'static str,
    },
    /// The estimate and reference described different measurands, so they
    /// cannot be compared.
    #[error("measurand mismatch: estimate `{estimate}` vs reference `{reference}`")]
    MeasurandMismatch {
        /// The estimate measurand.
        estimate: &'static str,
        /// The reference measurand.
        reference: &'static str,
    },
    /// The alignment configuration was invalid (e.g. a non-positive grid step).
    #[error("invalid alignment config: {reason}")]
    InvalidConfig {
        /// Human-readable reason.
        reason: &'static str,
    },
    /// The resampling grid would exceed the bounded point cap.
    #[error("grid would exceed {max} points; widen the grid step or narrow the range")]
    GridTooLarge {
        /// Enforced maximum.
        max: usize,
    },
    /// The lag search window would exceed the bounded step cap.
    #[error("lag window would exceed {max} steps; narrow max_lag_ms or widen grid_ms")]
    LagWindowTooLarge {
        /// Enforced maximum.
        max: usize,
    },
    /// A tolerance was negative or non-finite.
    #[error("tolerance must be finite and non-negative, got {value}")]
    InvalidTolerance {
        /// The rejected value.
        value: f64,
    },
    /// A coverage threshold was outside `[0, 1]` or non-finite.
    #[error("min_coverage must be within [0, 1], got {value}")]
    InvalidCoverage {
        /// The rejected value.
        value: f64,
    },
    /// The mandatory subject count exceeded the bounded maximum.
    #[error("subject_count {count} exceeds max {max}")]
    SubjectCountTooLarge {
        /// The rejected count.
        count: u32,
        /// Enforced maximum.
        max: u32,
    },
    /// The requested evidence grade was inconsistent with its level/reproducer.
    #[error("grade/level conflict: {reason}")]
    GradeLevelConflict {
        /// Human-readable reason.
        reason: &'static str,
    },
    /// A row of an untrusted input file could not be read.
    #[error("row {row}: {reason}")]
    InvalidRow {
        /// 1-based row (line) number.
        row: usize,
        /// Human-readable reason.
        reason: String,
    },
    /// A train/test split could not be made session-disjoint, or a side was
    /// empty.
    #[error("invalid split: {reason}")]
    InvalidSplit {
        /// Human-readable reason.
        reason: &'static str,
    },
    /// A failure raised by the [`ruview_evidence`] ledger boundary when
    /// emitting a record.
    #[error(transparent)]
    Evidence(#[from] ruview_evidence::EvidenceError),
}

/// Reject an over-length string field at the boundary.
pub(crate) fn check_bound(field: &'static str, value: &str) -> Result<(), GroundTruthError> {
    if value.len() > MAX_STR_LEN {
        return Err(GroundTruthError::TooLong {
            field,
            len: value.len(),
            max: MAX_STR_LEN,
        });
    }
    Ok(())
}

/// Reject an empty required string field at the boundary.
pub(crate) fn check_nonempty(field: &'static str, value: &str) -> Result<(), GroundTruthError> {
    if value.is_empty() {
        return Err(GroundTruthError::EmptyField { field });
    }
    Ok(())
}
