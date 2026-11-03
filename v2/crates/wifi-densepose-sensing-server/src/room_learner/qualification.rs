//! Narrow operator-labelled local qualification; never statistical independence.
use super::{
    capture::{PhaseReceipt, PhaseStatus},
    model::RoomModel,
    observation::{CaptureProvenance, DataRole, RoomLabel, ValidatedDataset},
    training::EvaluationReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const POLICY_ID: &str = "ruview.local-operational-policy.v1";
pub(crate) fn hash<T: Serialize>(value: &T) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).map_err(|e| e.to_string())?)
    ))
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QualificationPolicy {
    pub id: String,
    pub minimum_cohorts_per_condition: usize,
    pub minimum_unconditional_sensitivity: f32,
    pub maximum_false_positive_rate: f32,
    pub minimum_evaluated_cohort_coverage: f32,
    pub coverage_scope: String,
}
impl Default for QualificationPolicy {
    fn default() -> Self {
        Self {
            id: POLICY_ID.into(),
            minimum_cohorts_per_condition: 4,
            minimum_unconditional_sensitivity: 0.9,
            maximum_false_positive_rate: 0.05,
            minimum_evaluated_cohort_coverage: 0.9,
            coverage_scope: "representative_cohorts_not_wall_time".into(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConditionCriterion {
    pub condition: RoomLabel,
    pub train_cohorts: usize,
    pub held_out_cohorts: usize,
    pub evaluated_cohort_coverage: f32,
    pub unconditional_sensitivity: Option<f32>,
    pub false_positive_rate: Option<f32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QualificationReceipt {
    pub schema: String,
    pub owner_scope: String,
    pub installation_id: String,
    pub room_digest: String,
    pub context_digest: String,
    pub model_sha256: String,
    pub model_version: u32,
    pub training_data_sha256: String,
    pub held_out_dataset_sha256: String,
    pub evaluation_receipt_sha256: String,
    pub evaluated_at_unix_ms: u64,
    pub policy: QualificationPolicy,
    pub policy_sha256: String,
    pub label_provenance: String,
    pub sealed_history_sha256: String,
    pub first_observed_at_ns: u64,
    pub last_observed_at_ns: u64,
    pub criteria: Vec<ConditionCriterion>,
    pub report: EvaluationReport,
    pub independence_verified: bool,
    pub physically_validated: bool,
    pub authority_scope: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QualificationEnvelope {
    pub receipt_sha256: String,
    pub receipt: QualificationReceipt,
}

pub(crate) const CONDITIONS: [RoomLabel; 4] = [
    RoomLabel::Empty,
    RoomLabel::OccupiedStatic,
    RoomLabel::OccupiedMoving,
    RoomLabel::AdjacentOnly,
];

pub(crate) fn criteria(
    report: &EvaluationReport,
    train_counts: &[usize; 4],
) -> Result<Vec<ConditionCriterion>, String> {
    report.validate()?;
    let policy = QualificationPolicy::default();
    CONDITIONS
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let c = report
                .per_condition
                .iter()
                .find(|c| c.condition == *label)
                .ok_or("missing required held-out condition")?;
            if train_counts[i] < policy.minimum_cohorts_per_condition
                || c.samples < policy.minimum_cohorts_per_condition
            {
                return Err(format!(
                    "qualification requires four Train and HeldOut cohorts for {label:?}"
                ));
            }
            let coverage = c
                .rates
                .evaluated_cohort_coverage
                .ok_or("held-out coverage unavailable")?;
            if !coverage.is_finite() || coverage < policy.minimum_evaluated_cohort_coverage {
                return Err(format!("held-out coverage failed for {label:?}"));
            }
            let occupied = matches!(label, RoomLabel::OccupiedStatic | RoomLabel::OccupiedMoving);
            let (sensitivity, false_positive) = if occupied {
                let s = c
                    .rates
                    .unconditional_sensitivity
                    .ok_or("occupied sensitivity unavailable")?;
                if !s.is_finite() || s < policy.minimum_unconditional_sensitivity {
                    return Err(format!("occupied sensitivity failed for {label:?}"));
                }
                (Some(s), None)
            } else {
                let f = c.confusion.false_occupied as f32 / c.samples as f32;
                if !f.is_finite() || f > policy.maximum_false_positive_rate {
                    return Err(format!("false presence failed for {label:?}"));
                }
                (None, Some(f))
            };
            Ok(ConditionCriterion {
                condition: *label,
                train_cohorts: train_counts[i],
                held_out_cohorts: c.samples,
                evaluated_cohort_coverage: coverage,
                unconditional_sensitivity: sensitivity,
                false_positive_rate: false_positive,
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
pub(crate) struct ManagedEvidence {
    pub counts: [usize; 4],
    pub sealed_history_sha256: String,
    pub first_observed_at_ns: u64,
    pub last_observed_at_ns: u64,
    pub last_held_out_observed_ns: u64,
}
pub(crate) fn managed_evidence(
    model: &RoomModel,
    dataset: &ValidatedDataset,
    history: &[PhaseReceipt],
    evaluation: &super::artifact::EvaluationReceipt,
) -> Result<ManagedEvidence, String> {
    if dataset.context() != model.context()
        || dataset.train().is_empty()
        || hash(&dataset.train())? != model.training_manifest().training_data_sha256
    {
        return Err("qualification requires retained exact Train manifest lineage".into());
    }
    let rows: Vec<_> = dataset.train().iter().chain(dataset.held_out()).collect();
    if rows.iter().any(|c| {
        c.provenance != CaptureProvenance::LiveReceiverObserved
            || c.views
                .iter()
                .any(|v| v.provenance != CaptureProvenance::LiveReceiverObserved)
    }) {
        return Err(
            "qualification requires live receiver-observed Train and HeldOut sources".into(),
        );
    }
    if model.training_manifest().training_provenance != [CaptureProvenance::LiveReceiverObserved] {
        return Err("candidate training provenance is not live receiver-observed".into());
    }
    let mut relevant = Vec::new();
    for row in &rows {
        let matching: Vec<_> = history
            .iter()
            .filter(|p| {
                p.trial_id == row.trial_id
                    && p.session_id == row.session_id
                    && p.role == row.role
                    && p.label == row.label
                    && p.status == PhaseStatus::Sealed
            })
            .collect();
        if matching.len() != 1 {
            return Err("required managed sealed phase history is missing or ambiguous".into());
        }
        let phase = matching[0];
        let count = rows
            .iter()
            .filter(|r| {
                r.trial_id == row.trial_id
                    && r.session_id == row.session_id
                    && r.role == row.role
                    && r.label == row.label
            })
            .count();
        if phase.cohort_count != count {
            return Err("sealed phase history does not match exact cohorts".into());
        }
        if !relevant.iter().any(|p: &&PhaseReceipt| {
            p.trial_id == phase.trial_id && p.session_id == phase.session_id
        }) {
            relevant.push(phase);
        }
    }
    let counts =
        CONDITIONS.map(|label| dataset.train().iter().filter(|c| c.label == label).count());
    evaluation.report.validate()?;
    for (i, label) in CONDITIONS.iter().enumerate() {
        if counts[i] < 4
            || dataset
                .held_out()
                .iter()
                .filter(|c| c.label == *label)
                .count()
                < 4
        {
            return Err(format!(
                "qualification requires four Train and HeldOut cohorts for {label:?}"
            ));
        }
    }
    let c = model.context();
    if evaluation.role != DataRole::HeldOut
        || evaluation.context_digest != c.digest()
        || evaluation.training_data_sha256 != model.training_manifest().training_data_sha256
        || evaluation.dataset_sha256 != hash(&dataset.held_out())?
    {
        return Err("qualification evaluation lineage mismatch".into());
    }
    let first = rows
        .iter()
        .flat_map(|c| &c.views)
        .map(|v| v.first_time_ns)
        .min()
        .ok_or("no source time")?;
    let last = rows
        .iter()
        .flat_map(|c| &c.views)
        .map(|v| v.last_time_ns)
        .max()
        .ok_or("no source time")?;

    Ok(ManagedEvidence {
        counts,
        sealed_history_sha256: hash(&relevant)?,
        first_observed_at_ns: first,
        last_observed_at_ns: last,
        last_held_out_observed_ns: dataset
            .held_out()
            .iter()
            .flat_map(|c| &c.views)
            .map(|v| v.last_time_ns)
            .max()
            .ok_or("no held-out source time")?,
    })
}

impl QualificationEnvelope {
    pub(crate) fn issue(
        model: &RoomModel,
        dataset: &ValidatedDataset,
        history: &[PhaseReceipt],
        evaluation: &super::artifact::EvaluationReceipt,
    ) -> Result<Self, String> {
        let evidence = managed_evidence(model, dataset, history, evaluation)?;
        let checks = criteria(&evaluation.report, &evidence.counts)?;
        let c = model.context();
        let policy = QualificationPolicy::default();
        let receipt = QualificationReceipt {
            schema: "ruview.local-operational-qualification.v1".into(),
            owner_scope: c.owner_scope.clone(),
            installation_id: c.installation_id.clone(),
            room_digest: c.room_digest.clone(),
            context_digest: c.digest(),
            model_sha256: evaluation.model_sha256.clone(),
            model_version: evaluation.model_version,
            training_data_sha256: evaluation.training_data_sha256.clone(),
            held_out_dataset_sha256: evaluation.dataset_sha256.clone(),
            evaluation_receipt_sha256: evaluation.sha256()?,
            evaluated_at_unix_ms: evaluation.evaluated_at_unix_ms,
            policy_sha256: hash(&policy)?,
            policy,
            label_provenance: "managed-sealed-history-v1".into(),
            sealed_history_sha256: evidence.sealed_history_sha256,
            first_observed_at_ns: evidence.first_observed_at_ns,
            last_observed_at_ns: evidence.last_observed_at_ns,
            criteria: checks,
            report: evaluation.report.clone(),
            independence_verified: false,
            physically_validated: false,
            authority_scope: "local_operator_binary_presence".into(),
        };
        let envelope = Self {
            receipt_sha256: hash(&receipt)?,
            receipt,
        };
        envelope.validate(model, evaluation)?;
        Ok(envelope)
    }
    pub(crate) fn validate(
        &self,
        model: &RoomModel,
        eval: &super::artifact::EvaluationReceipt,
    ) -> Result<(), String> {
        let r = &self.receipt;
        let c = model.context();
        let hex = |x: &str| {
            x.len() == 64
                && x.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        if r.schema != "ruview.local-operational-qualification.v1"
            || self.receipt_sha256 != hash(r)?
            || r.owner_scope != c.owner_scope
            || r.installation_id != c.installation_id
            || r.room_digest != c.room_digest
            || r.context_digest != c.digest()
            || r.model_sha256 != eval.model_sha256
            || r.model_version != eval.model_version
            || r.training_data_sha256 != model.training_manifest().training_data_sha256
            || r.held_out_dataset_sha256 != eval.dataset_sha256
            || r.evaluation_receipt_sha256 != eval.sha256()?
            || r.evaluated_at_unix_ms != eval.evaluated_at_unix_ms
            || r.report != eval.report
            || eval.role != DataRole::HeldOut
            || r.policy != QualificationPolicy::default()
            || r.policy_sha256 != hash(&r.policy)?
            || r.label_provenance != "managed-sealed-history-v1"
            || !hex(&r.sealed_history_sha256)
            || r.first_observed_at_ns == 0
            || r.last_observed_at_ns < r.first_observed_at_ns
            || r.independence_verified
            || r.physically_validated
            || r.authority_scope != "local_operator_binary_presence"
            || model.training_manifest().training_provenance
                != [CaptureProvenance::LiveReceiverObserved]
            || eval.provenance != [CaptureProvenance::LiveReceiverObserved]
            || r.criteria.len() != 4
        {
            return Err("local qualification identity, provenance or policy mismatch".into());
        }
        let counts = std::array::from_fn(|i| r.criteria[i].train_cohorts);
        if counts.iter().any(|n| *n > super::observation::MAX_COHORTS)
            || counts.iter().sum::<usize>() != model.training_manifest().training_cohorts
            || criteria(&r.report, &counts)? != r.criteria
        {
            return Err("local qualification criterion mismatch".into());
        }
        Ok(())
    }
}
