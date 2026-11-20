//! `spatial.evidence.v1` envelope, the two RF record bodies, and the
//! boundary validation rules (ADR-382, which records WeftOS ADR-107 §7).
//!
//! Only the RF types RuView emits are mirrored. A line of any other `type`
//! (`shell_measure`, `pose`, `radar_track_point`, `uwb_echo`, `imu_event`,
//! `human_confirm`, `tof_depth`, `radar_range`) is rejected by
//! [`parse_line`] here as an unknown type, which is correct for an emitter:
//! it never needs to read those lines back.
//!
//! Angles: every yaw in v1 is degrees counter-clockwise from room +x
//! (east), so 0 faces +x and 90 faces +y. The RF types carry no yaw field;
//! an `rf_gaussian` orientation is a room-ENU rotation quaternion, whose
//! rotation about +z follows the same counter-clockwise sense.

use serde::{Deserialize, Serialize};

/// The only schema string v1 accepts.
pub const SCHEMA_V1: &str = "spatial.evidence.v1";
/// Longest accepted line, bytes.
pub const MAX_LINE_BYTES: usize = 16 * 1024;

const MAX_COORD_M: f64 = 1_000.0;
const MAX_UNCERTAINTY_M: f64 = 100.0;
const MAX_ID_LEN: usize = 128;

/// Why a record was rejected.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EvidenceError {
    /// Not JSON, wrong shape, or an unknown `type`.
    #[error("parse error: {0}")]
    Parse(String),
    /// `schema` missing or not `spatial.evidence.v1`.
    #[error("unknown schema version {0:?}")]
    UnknownVersion(String),
    /// A numeric field is NaN or infinite.
    #[error("non-finite value in {0}")]
    NonFinite(&'static str),
    /// A numeric field is outside its accepted range.
    #[error("value out of range in {0}")]
    OutOfRange(&'static str),
    /// An id is empty, too long, or has disallowed characters.
    #[error("invalid text in {0}")]
    BadText(&'static str),
    /// Line exceeds [`MAX_LINE_BYTES`].
    #[error("line is {0} bytes, max {MAX_LINE_BYTES}")]
    LineTooLong(usize),
    /// `MEASURED` was requested without a non-empty reproducer id.
    #[error("MEASURED proof requires a non-empty reproducer id")]
    MissingReproducer,
}

type R = Result<(), EvidenceError>;

/// Coordinate frame. v1 has one: room-local ENU, origin at the room's SW
/// floor corner, x east, y north, z up, metres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frame {
    /// Room ENU.
    RoomEnu,
}

/// Proof discipline tag, ordered weakest to strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProofTag {
    /// Simulated or generated.
    Synthetic,
    /// Derived by code from other evidence.
    Code,
    /// Direct physical measurement with a reproducer.
    Measured,
}

/// Where a record came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Unique receipt id for this record.
    pub receipt: String,
    /// Tool that produced the line (`ruview-spatial-evidence@0.1`, ...).
    pub producer: String,
    /// MEASURED / CODE / SYNTHETIC.
    pub proof: ProofTag,
}

/// One evidence line: the common envelope plus a flattened typed body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    /// Always [`SCHEMA_V1`].
    pub schema: String,
    /// Observation time, ns since the Unix epoch.
    pub t_ns: u64,
    /// Coordinate frame.
    pub frame: Frame,
    /// Urth region id (`region/urth/meso/<room>`).
    pub region: String,
    /// Device or operator id (never a person).
    pub source_id: String,
    /// 1σ position uncertainty, metres, in (0, 100].
    pub uncertainty_m: f64,
    /// Provenance envelope.
    pub provenance: Provenance,
    /// `type` tag and type-specific fields.
    #[serde(flatten)]
    pub body: RecordBody,
}

/// Envelope fields a caller supplies for one record.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordMeta {
    /// Observation time, ns since the Unix epoch.
    pub t_ns: u64,
    /// Urth region id.
    pub region: String,
    /// Device id.
    pub source_id: String,
    /// 1σ position uncertainty, metres.
    pub uncertainty_m: f64,
    /// Provenance envelope.
    pub provenance: Provenance,
}

impl EvidenceRecord {
    /// Assemble a v1 record and validate it.
    pub fn new(meta: RecordMeta, body: RecordBody) -> Result<Self, EvidenceError> {
        let rec = Self {
            schema: SCHEMA_V1.to_string(),
            t_ns: meta.t_ns,
            frame: Frame::RoomEnu,
            region: meta.region,
            source_id: meta.source_id,
            uncertainty_m: meta.uncertainty_m,
            provenance: meta.provenance,
            body,
        };
        rec.validate()?;
        Ok(rec)
    }

    /// Check every field against the v1 rules.
    pub fn validate(&self) -> R {
        if self.schema != SCHEMA_V1 {
            return Err(EvidenceError::UnknownVersion(self.schema.clone()));
        }
        id(&self.region, "region")?;
        id(&self.source_id, "source_id")?;
        id(&self.provenance.receipt, "provenance.receipt")?;
        id(&self.provenance.producer, "provenance.producer")?;
        finite(self.uncertainty_m, "uncertainty_m")?;
        if !(self.uncertainty_m > 0.0 && self.uncertainty_m <= MAX_UNCERTAINTY_M) {
            return Err(EvidenceError::OutOfRange("uncertainty_m"));
        }
        match &self.body {
            RecordBody::RfLinkObservation(l) => l.validate(),
            RecordBody::RfGaussian(g) => g.validate(),
        }
    }
}

/// The record bodies this crate emits, tagged by `"type"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordBody {
    /// One RF link measurement.
    RfLinkObservation(RfLinkObservation),
    /// One RF Gaussian (ADR-275 shape).
    RfGaussian(RfGaussianRecord),
}

/// Excess attenuation over free space between two antennas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RfLinkObservation {
    /// Transmitter antenna position, metres.
    pub tx: [f64; 3],
    /// Receiver antenna position, metres.
    pub rx: [f64; 3],
    /// Carrier frequency, Hz.
    pub freq_hz: f64,
    /// Loss beyond Friis, dB (negative = constructive multipath).
    pub excess_loss_db: f64,
}

impl RfLinkObservation {
    fn validate(&self) -> R {
        point(&self.tx, "tx")?;
        point(&self.rx, "rx")?;
        in_range(self.freq_hz, 1e8, 3e11, "freq_hz")?;
        in_range(self.excess_loss_db, -60.0, 200.0, "excess_loss_db")
    }
}

/// Coarse motion class (mirrors `ruview_unified::gaussian::MotionState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motion {
    /// Structural / furniture.
    Static,
    /// Breathing, posture shifts.
    Slow,
    /// Walking speed or faster.
    Fast,
}

/// Role of a Gaussian exported as an RF prior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RfRole {
    /// Attenuates links passing through it.
    Absorber,
    /// Dominant reflecting surface.
    Reflector,
}

/// Neutral RF Gaussian shape (the fields of `RfGaussian` that cross the wire).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RfGaussianRecord {
    /// Centre, metres.
    pub position: [f64; 3],
    /// Per-axis σ, metres.
    pub scale: [f64; 3],
    /// Orientation quaternion `[w, x, y, z]`, body to room ENU (a positive
    /// rotation about +z turns the local x axis counter-clockwise from east).
    pub orientation: [f64; 4],
    /// Peak extinction, nepers/metre.
    pub occupancy: f64,
    /// Confidence in `[0, 1]`.
    pub confidence: f64,
    /// Motion class.
    pub motion: Motion,
    /// Prior role; absent on records RuView emits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<RfRole>,
}

impl RfGaussianRecord {
    fn validate(&self) -> R {
        point(&self.position, "position")?;
        self.scale
            .iter()
            .try_for_each(|s| in_range(*s, 1e-6, 1e4, "scale"))?;
        self.orientation
            .iter()
            .try_for_each(|q| finite(*q, "orientation"))?;
        let n = self.orientation.iter().map(|q| q * q).sum::<f64>().sqrt();
        if n <= 1e-6 {
            return Err(EvidenceError::OutOfRange("orientation"));
        }
        in_range(self.occupancy, 0.0, 1e6, "occupancy")?;
        in_range(self.confidence, 0.0, 1.0, "confidence")
    }
}

fn finite(v: f64, field: &'static str) -> R {
    if v.is_finite() {
        Ok(())
    } else {
        Err(EvidenceError::NonFinite(field))
    }
}

fn in_range(v: f64, lo: f64, hi: f64, field: &'static str) -> R {
    finite(v, field)?;
    if (lo..=hi).contains(&v) {
        Ok(())
    } else {
        Err(EvidenceError::OutOfRange(field))
    }
}

fn point(p: &[f64; 3], field: &'static str) -> R {
    p.iter()
        .try_for_each(|v| in_range(*v, -MAX_COORD_M, MAX_COORD_M, field))
}

/// Ids: ASCII alphanumerics plus `._:/-@`, 1..=128 bytes.
fn id(s: &str, field: &'static str) -> R {
    if is_valid_id(s) {
        Ok(())
    } else {
        Err(EvidenceError::BadText(field))
    }
}

pub(crate) fn is_id_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"._:/-@".contains(&b)
}

pub(crate) fn is_valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_ID_LEN && s.bytes().all(is_id_byte)
}

/// Map arbitrary text to a valid id: disallowed bytes become `-`, the
/// result is cut to 128 bytes, and empty input yields `fallback`.
pub(crate) fn sanitize_id(s: &str, fallback: &str) -> String {
    let out: String = s
        .bytes()
        .take(MAX_ID_LEN)
        .map(|b| if is_id_byte(b) { b as char } else { '-' })
        .collect();
    if out.is_empty() {
        fallback.to_string()
    } else {
        out
    }
}

/// Validate and serialise one record as a single JSONL line (no newline).
pub fn to_line(rec: &EvidenceRecord) -> Result<String, EvidenceError> {
    rec.validate()?;
    let line = serde_json::to_string(rec).map_err(|e| EvidenceError::Parse(e.to_string()))?;
    if line.len() > MAX_LINE_BYTES {
        return Err(EvidenceError::LineTooLong(line.len()));
    }
    Ok(line)
}

/// Serialise records as a JSONL document, one line each with a trailing
/// newline. Fails on the first invalid record, so no partial file is built.
pub fn to_jsonl(records: &[EvidenceRecord]) -> Result<String, EvidenceError> {
    let mut out = String::new();
    for rec in records {
        out.push_str(&to_line(rec)?);
        out.push('\n');
    }
    Ok(out)
}

/// Parse and validate one line of a type this crate mirrors.
pub fn parse_line(line: &str) -> Result<EvidenceRecord, EvidenceError> {
    if line.len() > MAX_LINE_BYTES {
        return Err(EvidenceError::LineTooLong(line.len()));
    }
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| EvidenceError::Parse(e.to_string()))?;
    match value.get("schema").and_then(|s| s.as_str()) {
        Some(SCHEMA_V1) => {}
        Some(other) => return Err(EvidenceError::UnknownVersion(other.to_string())),
        None => return Err(EvidenceError::UnknownVersion(String::new())),
    }
    let rec: EvidenceRecord =
        serde_json::from_value(value).map_err(|e| EvidenceError::Parse(e.to_string()))?;
    rec.validate()?;
    Ok(rec)
}
