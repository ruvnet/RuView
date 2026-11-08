//! Whole-kind LOCAL operational qualification. Existing numeric criteria are
//! reused; no four-cohort aliases, physical validity or automatic selection.
use super::{
    artifact::StoredTemporalEvaluation,
    temporal_model_input::managed::execution::full_run::VerifiedTemporalEvaluationSnapshot,
    temporal_wire::*,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemporalQualificationEnvelope {
    pub receipt_sha256: String,
    pub receipt: TemporalQualificationReceipt,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemporalQualificationReceipt {
    pub schema: String,
    pub owner_scope: String,
    pub installation_id: String,
    pub qualified_at_unix_ms: u64,
    pub summary: TemporalQualificationSummaryWire,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemporalAssessmentEnvelope {
    pub receipt_sha256: String,
    pub receipt: TemporalAssessmentReceipt,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemporalAssessmentReceipt {
    pub schema: String,
    pub owner_scope: String,
    pub installation_id: String,
    pub summary: TemporalAssessmentWire,
}
/// Constructed only against the current protected Complete snapshot and exact
/// current-source verified evaluation. Imported DATA is not this result.
pub(crate) struct ComputedTemporalAssessment {
    pub(super) qualification: Option<TemporalQualificationEnvelope>,
    pub(super) assessment: TemporalAssessmentEnvelope,
}
fn h<T: Serialize>(v: &T) -> Result<String, String> {
    super::qualification::hash(v)
}
impl ComputedTemporalAssessment {
    pub(crate) fn issue_current(
        source: &VerifiedTemporalEvaluationSnapshot,
        evaluation: &StoredTemporalEvaluation,
        now: u64,
    ) -> Result<Self, String> {
        if source.binding() != evaluation.binding() {
            return Err("temporal_assessment_current_complete_changed".into());
        }
        let proof = evaluation.train();
        let held = evaluation.held_out();
        let eval = evaluation.receipt();
        eval.validate_data_links(proof, held)?;
        let id = &proof.body.identity;
        if now / 1_000_000 >= id.original_deadline.expires_at_unix_ms
            || now
                < eval
                    .body
                    .evaluated_at_receiver_ns
                    .parse::<u64>()
                    .map_err(|_| "temporal_evaluation_data_integer")?
        {
            return Err("temporal_assessment_clock_or_original_deadline".into());
        }
        if proof.body.windows_per_condition != [16; 4]
            || held.body.windows_per_condition != [16; 4]
            || eval.body.report.samples != 64
            || eval.body.report.per_condition.len() != 4
            || eval
                .body
                .report
                .per_condition
                .iter()
                .any(|c| c.samples != 16 || c.declared_trial_count != 2)
        {
            return Err("temporal_assessment_exact_native_evidence_required".into());
        }
        let policy = TemporalQualificationPolicyWire::current();
        let policy_sha = h(&policy)?;
        let criteria = native_criteria(&eval.body.report);
        // Authoritative pass/failure is the established integer criteria,
        // with exact16 input shapes enforced above. No metric is replaced.
        let outcome = super::qualification::criteria(&eval.body.report, &[16; 4]);
        let qualification = if outcome.is_ok() {
            let mut summary = TemporalQualificationSummaryWire {
                schema: QUALIFICATION_SCHEMA.into(),
                receipt_sha256: String::new(),
                model_kind: WholeTemporalKind::ManagedTemporalV3,
                model_sha256: id.artifact_sha256.clone(),
                verified_fit_record_sha256: id.verified_fit_record_sha256.clone(),
                training_lineage_basis: "managed_temporal_training_proof_v1".into(),
                training_proof_sha256: proof.sha256.clone(),
                held_out_evidence_sha256: held.sha256.clone(),
                complete_record_sha256: held.body.complete_record_sha256.clone(),
                evaluation_receipt_sha256: eval.sha256.clone(),
                context_digest: id.context_digest.clone(),
                room_digest: id.room_digest.clone(),
                evaluated_at_unix_ms: eval.body.evaluated_at_unix_ms,
                policy: policy.clone(),
                policy_sha256: policy_sha.clone(),
                label_provenance: "temporal-training-proof-v1+sealed-native-heldout-v1".into(),
                criteria: criteria.clone(),
                authority_scope: "local_operator_binary_presence".into(),
                window_unit: "native_receiver_2s_window".into(),
                train_windows: 64,
                held_out_windows: 64,
                train_episodes: 7,
                held_out_episodes: 8,
                independence_verified: false,
                physically_validated: false,
                measurement_clock_available: false,
            };
            let receipt = TemporalQualificationReceipt {
                schema: QUALIFICATION_SCHEMA.into(),
                owner_scope: id.owner_scope.clone(),
                installation_id: id.installation_id.clone(),
                qualified_at_unix_ms: now / 1_000_000,
                summary: summary.clone(),
            };
            let sha = h(&receipt)?;
            summary.receipt_sha256 = sha.clone();
            Some(TemporalQualificationEnvelope {
                receipt_sha256: sha,
                receipt: TemporalQualificationReceipt { summary, ..receipt },
            })
        } else {
            None
        };
        let condition_failures = eval
            .body
            .report
            .per_condition
            .iter()
            .filter_map(|c| {
                let codes = super::qualification::native_condition_failure_codes(c);
                (!codes.is_empty()).then_some(TemporalConditionFailureWire {
                    condition: c.condition,
                    codes,
                })
            })
            .collect();
        let summary = TemporalAssessmentWire {
            schema: ASSESSMENT_SCHEMA.into(),
            receipt_sha256: String::new(),
            model_kind: WholeTemporalKind::ManagedTemporalV3,
            model_sha256: id.artifact_sha256.clone(),
            verified_fit_record_sha256: id.verified_fit_record_sha256.clone(),
            training_proof_sha256: proof.sha256.clone(),
            held_out_evidence_sha256: held.sha256.clone(),
            complete_record_sha256: held.body.complete_record_sha256.clone(),
            evaluation_receipt_sha256: eval.sha256.clone(),
            context_digest: id.context_digest.clone(),
            room_digest: id.room_digest.clone(),
            evaluated_at_unix_ms: eval.body.evaluated_at_unix_ms,
            assessed_at_unix_ms: now / 1_000_000,
            policy,
            policy_sha256: policy_sha,
            window_unit: "native_receiver_2s_window".into(),
            outcome: if qualification.is_some() {
                "qualified"
            } else {
                "not_qualified"
            }
            .into(),
            failure_reason: outcome.err(),
            qualification_sha256: qualification.as_ref().map(|q| q.receipt_sha256.clone()),
            criteria,
            condition_failures,
        };
        let mut receipt = TemporalAssessmentReceipt {
            schema: ASSESSMENT_SCHEMA.into(),
            owner_scope: id.owner_scope.clone(),
            installation_id: id.installation_id.clone(),
            summary,
        };
        let sha = h(&receipt)?;
        receipt.summary.receipt_sha256 = sha.clone();
        Ok(Self {
            qualification,
            assessment: TemporalAssessmentEnvelope {
                receipt_sha256: sha,
                receipt,
            },
        })
    }
}

// The public summary echoes the envelope digest; that echo is excluded from
// the canonical receipt hash to avoid a self-referential commitment.
impl TemporalQualificationEnvelope {
    fn canonical_hash(&self) -> Result<String, String> {
        let mut receipt = self.receipt.clone();
        receipt.summary.receipt_sha256.clear();
        h(&receipt)
    }
    pub(crate) fn validate(&self, evaluation: &StoredTemporalEvaluation) -> Result<(), String> {
        self.validate_links(
            evaluation.train(),
            evaluation.held_out(),
            evaluation.receipt(),
        )
    }
    pub(crate) fn validate_links(
        &self,
        proof: &super::temporal_evaluation::TemporalTrainProofData,
        held: &super::temporal_evaluation::TemporalHeldOutEvidenceData,
        e: &super::temporal_evaluation::TemporalEvaluationReceiptData,
    ) -> Result<(), String> {
        e.validate_data_links(proof, held)?;
        let q = &self.receipt.summary;
        let id = &e.body.identity;
        if self.receipt_sha256 != self.canonical_hash()?
            || q.receipt_sha256 != self.receipt_sha256
            || self.receipt.schema != QUALIFICATION_SCHEMA
            || q.schema != QUALIFICATION_SCHEMA
            || self.receipt.owner_scope != id.owner_scope
            || self.receipt.installation_id != id.installation_id
            || q.model_sha256 != id.artifact_sha256
            || q.verified_fit_record_sha256 != id.verified_fit_record_sha256
            || q.training_proof_sha256 != proof.sha256
            || q.held_out_evidence_sha256 != held.sha256
            || q.complete_record_sha256 != held.body.complete_record_sha256
            || q.evaluation_receipt_sha256 != e.sha256
            || q.context_digest != id.context_digest
            || q.room_digest != id.room_digest
            || q.policy != TemporalQualificationPolicyWire::current()
            || q.policy_sha256 != h(&q.policy)?
            || q.criteria != native_criteria(&e.body.report)
            || q.evaluated_at_unix_ms != e.body.evaluated_at_unix_ms
            || self.receipt.qualified_at_unix_ms < e.body.evaluated_at_unix_ms
            || self.receipt.qualified_at_unix_ms >= id.original_deadline.expires_at_unix_ms
        {
            return Err("temporal_qualification_identity_changed".into());
        }
        super::qualification::criteria(&e.body.report, &[16; 4]).map(|_| ())
    }
}
impl TemporalAssessmentEnvelope {
    fn canonical_hash(&self) -> Result<String, String> {
        let mut receipt = self.receipt.clone();
        receipt.summary.receipt_sha256.clear();
        h(&receipt)
    }
    pub(crate) fn validate(
        &self,
        evaluation: &StoredTemporalEvaluation,
        qualification: Option<&TemporalQualificationEnvelope>,
    ) -> Result<(), String> {
        self.validate_links(
            evaluation.train(),
            evaluation.held_out(),
            evaluation.receipt(),
            qualification,
        )
    }
    pub(crate) fn validate_links(
        &self,
        proof: &super::temporal_evaluation::TemporalTrainProofData,
        held: &super::temporal_evaluation::TemporalHeldOutEvidenceData,
        e: &super::temporal_evaluation::TemporalEvaluationReceiptData,
        qualification: Option<&TemporalQualificationEnvelope>,
    ) -> Result<(), String> {
        e.validate_data_links(proof, held)?;
        let a = &self.receipt.summary;
        let id = &e.body.identity;
        if self.receipt_sha256 != self.canonical_hash()?
            || a.receipt_sha256 != self.receipt_sha256
            || self.receipt.schema != ASSESSMENT_SCHEMA
            || a.schema != ASSESSMENT_SCHEMA
            || self.receipt.owner_scope != id.owner_scope
            || self.receipt.installation_id != id.installation_id
            || a.model_sha256 != id.artifact_sha256
            || a.verified_fit_record_sha256 != id.verified_fit_record_sha256
            || a.training_proof_sha256 != proof.sha256
            || a.held_out_evidence_sha256 != held.sha256
            || a.complete_record_sha256 != held.body.complete_record_sha256
            || a.evaluation_receipt_sha256 != e.sha256
            || a.policy != TemporalQualificationPolicyWire::current()
            || a.policy_sha256 != h(&a.policy)?
            || a.criteria != native_criteria(&e.body.report)
            || a.evaluated_at_unix_ms != e.body.evaluated_at_unix_ms
            || a.assessed_at_unix_ms < e.body.evaluated_at_unix_ms
            || a.assessed_at_unix_ms >= id.original_deadline.expires_at_unix_ms
        {
            return Err("temporal_assessment_identity_changed".into());
        }
        match qualification {
            Some(q) => {
                q.validate_links(proof, held, e)?;
                if a.outcome != "qualified"
                    || a.qualification_sha256.as_deref() != Some(&q.receipt_sha256)
                {
                    return Err("temporal_assessment_outcome_changed".into());
                }
            }
            None => {
                if a.outcome != "not_qualified"
                    || a.qualification_sha256.is_some()
                    || super::qualification::criteria(&e.body.report, &[16; 4]).is_ok()
                {
                    return Err("temporal_assessment_outcome_changed".into());
                }
            }
        }
        Ok(())
    }
}

fn native_criteria(
    report: &super::training::EvaluationReport,
) -> Vec<TemporalConditionCriterionWire> {
    report
        .per_condition
        .iter()
        .map(|c| {
            let occupied = matches!(
                c.condition,
                super::observation::RoomLabel::OccupiedStatic
                    | super::observation::RoomLabel::OccupiedMoving
            );
            TemporalConditionCriterionWire {
                condition: c.condition,
                train_windows: 16,
                held_out_windows: c.samples as u16,
                scored_windows: c.scored as u16,
                abstained_windows: c.abstained as u16,
                missing_windows: c.missing as u16,
                declared_train_episodes: if c.condition == super::observation::RoomLabel::Empty {
                    1
                } else {
                    2
                },
                declared_held_out_episodes: 2,
                evaluated_window_coverage: c.scored as f64 / c.samples as f64,
                unconditional_sensitivity: occupied
                    .then_some(c.confusion.true_occupied as f64 / c.samples as f64),
                false_positive_rate: (!occupied)
                    .then_some(c.confusion.false_occupied as f64 / c.samples as f64),
            }
        })
        .collect()
}
