//! Optional private comparison sidecar from the SAME retained rows/predictions.
//! Production reference clocks and ROI are unresolved: every cohort stays Unknown.
use super::comparison_ledger::{install, read};
use super::private_subdirectory;
use super::*;
use crate::companion_references::{ReferenceSnapshot, TrackingState};
use crate::room_learner::{
    model::{PresenceDecision, RoomPrediction},
    observation::{CaptureCohort, DataRole, ValidatedDataset},
};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceComparisonSummary {
    pub schema: String,
    pub sidecar_sha256: String,
    pub receipt_sha256: String,
    pub ledger_sha256: String,
    pub cohort_count: usize,
    pub unknown_cohorts: usize,
    pub associated_reference_rows: usize,
    pub comparison_only: bool,
    pub physically_validated: bool,
    pub adoption_authorized: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    cohort_sha256: String,
    decision: PresenceDecision,
    reference_state: String,
    associated_rows: usize,
    reference_commitments: Vec<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sidecar {
    schema: String,
    context_digest: String,
    model_sha256: String,
    role: DataRole,
    receipt_sha256: String,
    ledger_sha256: String,
    dataset_sha256: String,
    purpose_generation: u64,
    device_commitment: String,
    outcomes: Vec<Outcome>,
    comparison_only: bool,
    physically_validated: bool,
    adoption_authorized: bool,
}
fn sha<T: Serialize>(value: &T) -> Result<String, String> {
    Ok(digest(
        &serde_json::to_vec(value).map_err(|_| "reference_encoding_failed")?,
    ))
}
fn outcomes(
    rows: &[CaptureCohort],
    predictions: &[RoomPrediction],
    snapshot: &ReferenceSnapshot,
) -> Result<Vec<Outcome>, String> {
    if rows.len() != predictions.len() {
        return Err("reference_prediction_count_mismatch".into());
    }
    rows.iter()
        .zip(predictions)
        .map(|(row, prediction)| {
            if prediction.physically_validated {
                return Err("reference_prediction_authority_invalid".into());
            }
            let references = snapshot
                .rows
                .iter()
                .filter(|r| {
                    r.scope.trial_id == row.trial_id
                        && r.scope.session_id == row.session_id
                        && r.scope.role == row.role
                })
                .collect::<Vec<_>>();
            // Receiver-domain overlap is only a provenance association, NOT source-clock
            // alignment or independently validated room coverage. Never infer Empty
            // from visibleBodyCount=0 or reuse operator labels as teacher truth.
            let first = row
                .views
                .iter()
                .map(|v| v.first_time_ns)
                .max()
                .ok_or("reference_rf_window_missing")?;
            let last = row
                .views
                .iter()
                .map(|v| v.last_time_ns)
                .min()
                .ok_or("reference_rf_window_missing")?;
            let mut commitments = vec![];
            for r in references
                .into_iter()
                .filter(|r| r.received_ns >= first && r.received_ns <= last)
            {
                commitments.push(sha(&(
                    r.received_ns.to_string(),
                    &r.source_time,
                    r.captured_ms,
                    r.detector_completed,
                    r.visible_body_count,
                    match r.tracking {
                        TrackingState::Normal => "normal",
                        TrackingState::Limited => "limited",
                        TrackingState::Unavailable => "unavailable",
                    },
                ))?);
            }
            Ok(Outcome {
                cohort_sha256: sha(row)?,
                decision: prediction.decision,
                reference_state: if commitments.is_empty() {
                    "unknown_no_associated_reference"
                } else {
                    "unknown_unverified_clock_and_roi"
                }
                .into(),
                associated_rows: commitments.len(),
                reference_commitments: commitments,
            })
        })
        .collect()
}
impl ArtifactStore {
    pub(super) fn save_reference_comparison(
        &self,
        model: &RoomModel,
        dataset: &ValidatedDataset,
        receipt: &EvaluationReceipt,
        ledger: &ComparisonLedgerSummary,
        predictions: &[RoomPrediction],
        snapshot: &ReferenceSnapshot,
    ) -> Result<ReferenceComparisonSummary, String> {
        self.validate_context(model.context())?;
        if dataset.context() != model.context()
            || snapshot.rows.iter().any(|r| {
                r.scope.context_digest != receipt.context_digest
                    || r.scope.model_sha256 != receipt.model_sha256
                    || r.scope.room_digest != receipt.room_digest
            })
            || ledger.receipt_sha256 != receipt.sha256()?
            || !ledger.comparison_only
            || ledger.physically_validated
            || ledger.adoption_authorized
        {
            return Err("reference_comparison_context_changed".into());
        }
        let rows = match receipt.role {
            DataRole::HeldOut => dataset.held_out(),
            DataRole::Validation => dataset.validation(),
            _ => return Err("reference_comparison_role_invalid".into()),
        };
        let out = outcomes(rows, predictions, snapshot)?;
        let sidecar = Sidecar {
            schema: "ruview.room-learner.reference-comparison.v1".into(),
            context_digest: receipt.context_digest.clone(),
            model_sha256: receipt.model_sha256.clone(),
            role: receipt.role,
            receipt_sha256: receipt.sha256()?,
            ledger_sha256: ledger.ledger_sha256.clone(),
            dataset_sha256: receipt.dataset_sha256.clone(),
            purpose_generation: snapshot.generation,
            device_commitment: sha(&snapshot.device_id)?,
            outcomes: out,
            comparison_only: true,
            physically_validated: false,
            adoption_authorized: false,
        };
        let content = serde_json::to_vec(&sidecar).map_err(|_| "reference_encoding_failed")?;
        // References can be revoked during computation; check immediately before
        // optional persistence. RF receipt/model remain valid on this failure.
        snapshot.ensure_live()?;
        let parent = self
            .evaluation_directory(model, receipt.role, true)?
            .ok_or("reference_directory_missing")?;
        let dir = private_subdirectory(&parent, "reference-comparisons-v1", true)?
            .ok_or("reference_directory_missing")?;
        let hash = digest(&content);
        let path = dir.join(format!("{hash}.json"));
        install(&path, &content)?;
        if read(&path)? != content {
            return Err("reference_sidecar_readback_failed".into());
        }
        Ok(ReferenceComparisonSummary {
            schema: sidecar.schema,
            sidecar_sha256: hash,
            receipt_sha256: sidecar.receipt_sha256,
            ledger_sha256: sidecar.ledger_sha256,
            cohort_count: sidecar.outcomes.len(),
            unknown_cohorts: sidecar.outcomes.len(),
            associated_reference_rows: sidecar.outcomes.iter().map(|o| o.associated_rows).sum(),
            comparison_only: true,
            physically_validated: false,
            adoption_authorized: false,
        })
    }
}
