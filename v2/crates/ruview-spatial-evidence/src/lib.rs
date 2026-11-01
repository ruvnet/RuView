//! # `ruview-spatial-evidence` — RuView output as `spatial.evidence.v1` JSONL
//!
//! `spatial.evidence.v1` is the versioned, one-record-per-line wire format
//! of the WeftOS spatial evidence engine. ADR-382 records the RF part of the
//! contract in this repository; WeftOS ADR-107 §7 is its origin. This crate is
//! the RuView-side emitter for the two RF record types RuView can produce:
//!
//! - [`gaussian`]: `ruview_unified` [`RfGaussian`](ruview_unified::gaussian::RfGaussian)s
//!   (one by one or a whole [`GaussianMap`](ruview_unified::gaussian::GaussianMap))
//!   become `rf_gaussian` records.
//! - [`link`]: a TX/RX link with a measured and a free-space amplitude
//!   becomes an `rf_link_observation` record carrying `excess_loss_db`.
//!
//! The wire shapes live in [`wire`] as local serde structs. Nothing here
//! depends on WeftOS: the contract is the JSON, not a shared crate.
//! Every record is validated against the v1 rules before it is serialised
//! ([`to_line`]), so a bad value fails here rather than in the consumer.
//!
//! ## Proof tags
//!
//! Each record carries `provenance.proof` (`MEASURED` / `CODE` /
//! `SYNTHETIC`). A Gaussian whose RuView provenance says `synthetic` is
//! always emitted as `SYNTHETIC`, whatever the caller asks for; the tag can
//! be lowered by the caller, never raised. The default tag is `CODE`
//! ([`GaussianExport::new`]); `MEASURED` needs a non-empty reproducer id
//! ([`GaussianExport::measured`]) or the export fails with
//! [`EvidenceError::MissingReproducer`]. The link path still takes the proof
//! tag from the caller's [`RecordMeta`] unchecked.
//!
//! ## Golden fixtures
//!
//! The golden lines in `tests/evidence.rs` are copied from WeftOS ADR-107 §7.
//! Origin: WeftOS ADR-107 contract, no in-repo spec beyond ADR-382's summary
//! of it.

pub mod gaussian;
pub mod link;
pub mod wire;

pub use gaussian::{gaussian_to_record, map_to_records, GaussianExport};
pub use link::{free_space_amplitude, LinkMeasurement};
pub use wire::{
    parse_line, to_jsonl, to_line, EvidenceError, EvidenceRecord, Frame, Motion, ProofTag,
    Provenance, RecordBody, RecordMeta, RfGaussianRecord, RfLinkObservation, RfRole,
    MAX_LINE_BYTES, SCHEMA_V1,
};
