//! Bounded vital publication provenance and read-only diagnostic DTOs.
//! Candidate rates stay private; diagnostics contain gate outcomes only.

use crate::vital_signs::VitalSigns;
use serde::{Deserialize, Serialize};

pub const DIAGNOSTICS_SCHEMA: &str = "ruview.vital-publication-diagnostics.v1";
pub const CANDIDATE_MAX_AGE_MS: u64 = 3_000;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateKind {
    RawCsiSpectral,
    FirmwareEdge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationIdentity {
    pub boot_epoch: String,
    pub session_id: String,
    pub model_id: String,
    pub binding_digest: String,
    pub source_node_ids: Vec<u8>,
    pub completed_at_unix_ms: u64,
}

#[derive(Debug, Clone)]
pub struct NodeVitalCandidate {
    pub kind: CandidateKind,
    pub tick: u64,
    pub observed_at_unix_ms: u64,
    pub received_at: std::time::Instant,
    pub rates: VitalSigns,
    pub calibration_identity: Option<CalibrationIdentity>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Published,
    PrivacyRedacted,
    NoCandidate,
    CandidateStale,
    CandidateCalibrationChanged,
    NotPublishedForSourceTick,
    RoomNotSingleOccupant,
    SequenceFault,
    ExplicitCalibrationRequired,
    BoundModelUnavailable,
    BoundWindowNotReady,
    CalibratedPresenceUnavailable,
    CalibratedNotSingleOccupant,
    LowSignalQuality,
    NoCandidateRate,
    InvalidCandidateRate,
    LowRateConfidence,
}

#[derive(Debug, Clone, Serialize)]
pub struct BufferStatus {
    pub breathing_samples: usize,
    pub breathing_capacity: usize,
    pub heartbeat_samples: usize,
    pub heartbeat_capacity: usize,
}
impl From<(usize, usize, usize, usize)> for BufferStatus {
    fn from(
        (breathing_samples, breathing_capacity, heartbeat_samples, heartbeat_capacity): (
            usize,
            usize,
            usize,
            usize,
        ),
    ) -> Self {
        Self {
            breathing_samples,
            breathing_capacity,
            heartbeat_samples,
            heartbeat_capacity,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Decision {
    pub node_id: u8,
    pub candidate_kind: Option<CandidateKind>,
    pub source_tick: Option<u64>,
    pub observed_at_unix_ms: Option<u64>,
    pub candidate_age_ms: Option<u64>,
    pub occupancy_source_node_id: Option<u8>,
    pub room_person_count: usize,
    pub calibrated_person_count: Option<usize>,
    pub explicit_calibration_fresh: bool,
    pub sequence_continuity_ok: bool,
    pub bound_window_ready: bool,
    pub breathing_rate_present: bool,
    pub heart_rate_present: bool,
    pub signal_quality_passed: bool,
    pub breathing_confidence_passed: bool,
    pub heartbeat_confidence_passed: bool,
    pub buffers: BufferStatus,
    pub sample_rate_hz: f64,
    pub observed_sample_rate_hz: f64,
    pub sample_rate_resets: u64,
    pub reason: Reason,
}

/// Exact v1 envelope already consumed by the native clients. Inventory and
/// occupancy-source IDs are deliberately not used as rate contributors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitalEvidence {
    pub schema: String,
    pub capability: String,
    pub source: String,
    pub source_tick: u64,
    pub source_node_ids: Vec<u8>,
    pub observed_at_unix_ms: u64,
    #[serde(flatten)]
    pub rates: VitalSigns,
}

pub fn evidence(
    node_id: u8,
    candidate: &NodeVitalCandidate,
    published: &VitalSigns,
) -> Option<VitalEvidence> {
    Some(VitalEvidence {
        schema: "ruview.vital-evidence.v1".into(),
        capability: match candidate.kind {
            CandidateKind::RawCsiSpectral => "esp32_csi_spectral_vitals_v1",
            CandidateKind::FirmwareEdge => "esp32_firmware_edge_vitals_v1",
        }
        .into(),
        source: "esp32".into(),
        source_tick: candidate.tick,
        source_node_ids: vec![node_id],
        observed_at_unix_ms: candidate.observed_at_unix_ms,
        rates: published.clone(),
    })
}
