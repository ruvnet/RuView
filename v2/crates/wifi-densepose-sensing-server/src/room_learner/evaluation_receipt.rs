//! Hashed metadata only. Input provenance/declared trials do not grant adoption.
use super::super::{digest, encode, hex_digest};
use crate::room_learner::{
    model::{RoomModel, RoomPrediction},
    observation::{CaptureCohort, CaptureProvenance, ClockKind, DataRole},
    training::EvaluationReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Write};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvaluationReceipt {
    pub schema: String,
    pub owner_scope: String,
    pub installation_id: String,
    pub room_digest: String,
    pub context_digest: String,
    pub model_sha256: String,
    pub model_version: u32,
    pub role: DataRole,
    pub dataset_sha256: String,
    pub evaluated_at_unix_ms: u64,
    pub provenance: Vec<CaptureProvenance>,
    pub clock_claims: Vec<ClockKind>,
    pub measurement_clock_available: bool,
    /// True only when the retained Train set matched this model's manifest and
    /// the dataset validator checked role separation. Not statistical independence.
    pub train_lineage_checked: bool,
    pub training_data_sha256: String,
    pub report: EvaluationReport,
    pub independence_verified: bool,
    pub physically_validated: bool,
    pub adoption_authorized: bool,
}
struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn data_sha256(rows: &[CaptureCohort]) -> Result<String, String> {
    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(&mut writer, rows).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", writer.0.finalize()))
}
fn provenance_rank(p: &CaptureProvenance) -> u8 {
    match p {
        CaptureProvenance::Synthetic => 0,
        CaptureProvenance::CapturedReplay => 1,
        CaptureProvenance::LiveReceiverObserved => 2,
    }
}
fn clock_rank(p: &ClockKind) -> u8 {
    match p {
        ClockKind::Unknown => 0,
        ClockKind::ReceiverOnly => 1,
        ClockKind::SourceMeasurement => 2,
    }
}
impl EvaluationReceipt {
    pub fn sha256(&self) -> Result<String, String> {
        Ok(digest(&encode(self)?))
    }
    pub(super) fn new(
        model: &RoomModel,
        rows: &[CaptureCohort],
        role: DataRole,
        at: u64,
        train_checked: bool,
    ) -> Result<Self, String> {
        Self::from_report(model, rows, role, at, train_checked, model.evaluate(rows)?)
    }
    pub(super) fn new_recorded(
        model: &RoomModel,
        rows: &[CaptureCohort],
        role: DataRole,
        at: u64,
        train_checked: bool,
    ) -> Result<(Self, Vec<RoomPrediction>), String> {
        let (report, predictions) = EvaluationReport::compute_recorded(model, rows)?;
        Ok((
            Self::from_report(model, rows, role, at, train_checked, report)?,
            predictions,
        ))
    }
    // Only the actual evaluator above may supply this report. No caller/API
    // report or fabricated per-source inference enters a durable receipt.
    fn from_report(
        model: &RoomModel,
        rows: &[CaptureCohort],
        role: DataRole,
        at: u64,
        train_checked: bool,
        report: EvaluationReport,
    ) -> Result<Self, String> {
        if rows.is_empty()
            || rows.iter().any(|r| {
                r.role != role
                    || r.views.iter().any(|v| {
                        v.provenance != r.provenance
                            || (v.signal_kind == ruview_unified::RfModality::Synthetic
                                && v.provenance != CaptureProvenance::Synthetic)
                    })
            })
        {
            return Err("invalid evaluation role/provenance".into());
        }
        let mut provenance: Vec<_> = rows.iter().map(|r| r.provenance).collect();
        provenance.sort_by_key(provenance_rank);
        provenance.dedup();
        let mut clocks: Vec<_> = rows
            .iter()
            .flat_map(|r| r.views.iter().map(|v| v.clock.kind))
            .collect();
        clocks.sort_by_key(clock_rank);
        clocks.dedup();
        let available = !provenance.contains(&CaptureProvenance::Synthetic)
            && clocks == [ClockKind::SourceMeasurement];
        let c = model.context();
        let result = Self {
            schema: "ruview.room-learner.evaluation.v1".into(),
            owner_scope: c.owner_scope.clone(),
            installation_id: c.installation_id.clone(),
            room_digest: c.room_digest.clone(),
            context_digest: c.digest(),
            model_sha256: digest(&model.export()?),
            model_version: model.version,
            role,
            dataset_sha256: data_sha256(rows)?,
            evaluated_at_unix_ms: at,
            provenance,
            clock_claims: clocks,
            measurement_clock_available: available,
            train_lineage_checked: train_checked,
            training_data_sha256: model.training_manifest().training_data_sha256.clone(),
            report,
            independence_verified: false,
            physically_validated: false,
            adoption_authorized: false,
        };
        result.validate(model, role)?;
        Ok(result)
    }
    pub(super) fn validate(&self, model: &RoomModel, role: DataRole) -> Result<(), String> {
        let c = model.context();
        if self.schema != "ruview.room-learner.evaluation.v1"
            || self.owner_scope != c.owner_scope
            || self.installation_id != c.installation_id
            || self.room_digest != c.room_digest
            || self.context_digest != c.digest()
            || self.model_sha256 != digest(&model.export()?)
            || self.model_version != model.version
            || self.role != role
            || !matches!(role, DataRole::Validation | DataRole::HeldOut)
            || !hex_digest(&self.dataset_sha256)
            || self.evaluated_at_unix_ms == 0
            || self.evaluated_at_unix_ms > 253_402_300_799_999
            || self.training_data_sha256 != model.training_manifest().training_data_sha256
            || self.independence_verified
            || self.physically_validated
            || self.adoption_authorized
        {
            return Err("evaluation receipt/candidate binding mismatch".into());
        }
        if self.provenance.is_empty()
            || self.provenance.len() > 3
            || self
                .provenance
                .windows(2)
                .any(|w| provenance_rank(&w[0]) >= provenance_rank(&w[1]))
            || self.clock_claims.is_empty()
            || self.clock_claims.len() > 3
            || self
                .clock_claims
                .windows(2)
                .any(|w| clock_rank(&w[0]) >= clock_rank(&w[1]))
            || self.measurement_clock_available
                != (!self.provenance.contains(&CaptureProvenance::Synthetic)
                    && self.clock_claims == [ClockKind::SourceMeasurement])
            || (c
                .sources
                .iter()
                .any(|s| s.signal_kind == ruview_unified::RfModality::Synthetic)
                && self.provenance != [CaptureProvenance::Synthetic])
        {
            return Err("invalid evaluation provenance/clock claims".into());
        }
        self.report.validate()
    }
}
