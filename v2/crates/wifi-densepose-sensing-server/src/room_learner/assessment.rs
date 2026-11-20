//! Latest completed operator evaluation withdraws stale positive authority without erasing history.
use super::{
    artifact::EvaluationReceipt,
    model::RoomModel,
    observation::{CaptureProvenance, DataRole, MAX_COHORTS},
    qualification::{criteria, hash, ManagedEvidence, QualificationEnvelope, QualificationPolicy},
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AssessmentReceipt {
    pub schema: String,
    pub owner_scope: String,
    pub installation_id: String,
    pub context_digest: String,
    pub model_sha256: String,
    pub evaluation_receipt_sha256: String,
    pub policy_sha256: String,
    pub evaluated_at_unix_ms: u64,
    pub last_held_out_observed_ns: u64,
    pub train_cohorts_per_condition: [usize; 4],
    pub sealed_history_sha256: String,
    pub qualification_sha256: Option<String>,
    pub failure_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AssessmentEnvelope {
    pub receipt_sha256: String,
    pub receipt: AssessmentReceipt,
}
impl AssessmentEnvelope {
    pub(crate) fn issue(
        model: &RoomModel,
        evaluation: &EvaluationReceipt,
        evidence: &ManagedEvidence,
        qualified: Option<&QualificationEnvelope>,
    ) -> Result<Self, String> {
        let outcome = criteria(&evaluation.report, &evidence.counts);
        match (&outcome, qualified) {
            (Ok(_), None) | (Err(_), Some(_)) => {
                return Err("qualification outcome mismatch".into())
            }
            _ => {}
        }
        let c = model.context();
        let receipt = AssessmentReceipt {
            schema: "ruview.local-operational-assessment.v1".into(),
            owner_scope: c.owner_scope.clone(),
            installation_id: c.installation_id.clone(),
            context_digest: c.digest(),
            model_sha256: evaluation.model_sha256.clone(),
            evaluation_receipt_sha256: evaluation.sha256()?,
            policy_sha256: hash(&QualificationPolicy::default())?,
            evaluated_at_unix_ms: evaluation.evaluated_at_unix_ms,
            last_held_out_observed_ns: evidence.last_held_out_observed_ns,
            train_cohorts_per_condition: evidence.counts,
            sealed_history_sha256: evidence.sealed_history_sha256.clone(),
            qualification_sha256: qualified.map(|q| q.receipt_sha256.clone()),
            failure_reason: outcome.err(),
        };
        let a = Self {
            receipt_sha256: hash(&receipt)?,
            receipt,
        };
        a.validate(model, evaluation)?;
        Ok(a)
    }
    pub(crate) fn validate(
        &self,
        model: &RoomModel,
        eval: &EvaluationReceipt,
    ) -> Result<(), String> {
        let r = &self.receipt;
        let c = model.context();
        let hex = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        if self.receipt_sha256 != hash(r)?
            || r.schema != "ruview.local-operational-assessment.v1"
            || r.owner_scope != c.owner_scope
            || r.installation_id != c.installation_id
            || r.context_digest != c.digest()
            || r.model_sha256 != eval.model_sha256
            || r.evaluation_receipt_sha256 != eval.sha256()?
            || r.policy_sha256 != hash(&QualificationPolicy::default())?
            || r.evaluated_at_unix_ms != eval.evaluated_at_unix_ms
            || r.last_held_out_observed_ns == 0
            || !hex(&r.sealed_history_sha256)
            || eval.role != DataRole::HeldOut
            || eval.provenance != [CaptureProvenance::LiveReceiverObserved]
            || model.training_manifest().training_provenance
                != [CaptureProvenance::LiveReceiverObserved]
            || r.train_cohorts_per_condition
                .iter()
                .any(|n| *n < 4 || *n > MAX_COHORTS)
            || r.train_cohorts_per_condition.iter().sum::<usize>()
                != model.training_manifest().training_cohorts
            || eval.report.per_condition.iter().any(|v| v.samples < 4)
        {
            return Err("assessment identity/lineage mismatch".into());
        }
        match criteria(&eval.report, &r.train_cohorts_per_condition) {
            Ok(_) => {
                if r.failure_reason.is_some() || !r.qualification_sha256.as_deref().is_some_and(hex)
                {
                    return Err("assessment pass invalid".into());
                }
            }
            Err(e) => {
                if r.failure_reason.as_deref() != Some(e.as_str())
                    || r.qualification_sha256.is_some()
                {
                    return Err("assessment failure invalid".into());
                }
            }
        }
        Ok(())
    }
}
