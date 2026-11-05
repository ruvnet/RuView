//! Durable diagnostics only. Historical lineage checks are not current identity.
use super::*;
use crate::room_learner::artifact::EvaluationReceipt;
pub(super) fn presentation(
    receipt: &EvaluationReceipt,
    restored: bool,
    current_train_checked: bool,
) -> Result<Value, String> {
    Ok(
        json!({"schema":"ruview.room-learner.evaluation.v1","role":receipt.role,
        "context_digest":receipt.context_digest,"model_sha256":receipt.model_sha256,"report":receipt.report,
        "measurement_clock_available":receipt.measurement_clock_available,
        "receipt":receipt,"receipt_sha256":receipt.sha256()?,"durable":true,"restored":restored,
        "train_lineage_checked":current_train_checked,
        "historical_train_lineage_checked":receipt.train_lineage_checked,
        "independence_verified":false,"physically_validated":false,"adoption_authorized":false}),
    )
}
impl RoomLearningRuntime {
    pub(super) fn restored_evaluation(&self, model: &RoomModel) -> (Option<Value>, Option<String>) {
        let mut receipts = Vec::new();
        for role in [DataRole::Validation, DataRole::HeldOut] {
            match self.store.load_latest_evaluation(model,role) {
                Ok(Some(receipt))=>receipts.push(receipt),Ok(None)=>{},
                Err(_)=>return (None,Some("Saved evaluation could not be verified for this candidate; evidence files were retained.".into())),
            }
        }
        let Some(receipt) = receipts.into_iter().max_by_key(|r| {
            (
                r.evaluated_at_unix_ms,
                usize::from(r.role == DataRole::HeldOut),
            )
        }) else {
            return (None, None);
        };
        match presentation(&receipt, true, false) {
            Ok(value) => (Some(value), None),
            Err(_) => (
                None,
                Some(
                    "Saved evaluation metadata could not be read; the candidate is still loaded."
                        .into(),
                ),
            ),
        }
    }
    /// Only a frozen private candidate and a validated role split can produce a
    /// durable diagnostic. This never adopts the candidate or changes control.
    pub fn evaluate(&self, expected: &MutationGuard, role: DataRole) -> Result<Value, String> {
        self.evaluate_with_comparison(expected, role, false)
    }
    /// Derived prediction recording is a separate explicit opt-in. A failed
    /// sidecar never revokes a successfully committed RF evaluation receipt.
    pub fn evaluate_with_comparison(
        &self,
        expected: &MutationGuard,
        role: DataRole,
        record_comparison_evidence: bool,
    ) -> Result<Value, String> {
        self.evaluate_reference_optional(expected, role, record_comparison_evidence, None)
    }
    pub fn evaluate_with_references(
        &self,expected:&MutationGuard,role:DataRole,
        references:&crate::companion_references::ReferenceSnapshot,
    )->Result<Value,String>{
        self.evaluate_reference_optional(expected,role,true,Some(references))
    }
    fn evaluate_reference_optional(
        &self, expected:&MutationGuard,role:DataRole,record_comparison_evidence:bool,
        references:Option<&crate::companion_references::ReferenceSnapshot>,
    )->Result<Value,String>{
        if !matches!(role, DataRole::Validation | DataRole::HeldOut) {
            return Err("evaluate only validation or held-out data".into());
        }
        let mut state = self.command_state(expected)?;
        if busy(&state.job) || !state.frozen_training {
            return Err("wait for the frozen candidate".into());
        }
        let model = state.model.as_ref().ok_or("no frozen candidate")?.clone();
        let capture = state.capture.as_ref().ok_or("no capture context")?;
        if capture.phase_active() {
            return Err("finish the current phase before evaluation".into());
        }
        let dataset = capture.validated_dataset()?;
        let result = if let Some(references)=references {
            if references.rows.iter().any(|r|r.scope.runtime_epoch!=self.runtime_epoch || r.scope.generation!=state.generation || r.scope.context_digest!=capture.context().digest() || r.scope.model_sha256!=state.receipt.as_ref().map(|r|r.model_sha256.as_str()).unwrap_or("")) {
                return Err("reference_runtime_context_changed".into());
            }
            references.ensure_live()?;
            self.store.evaluate_candidate_with_references(&model,&dataset,role,references).and_then(|(receipt,optional)|{
                let mut value=presentation(&receipt,false,!dataset.train().is_empty())?;
                value["reference_recording_requested"]=json!(true);
                match optional {
                    Ok((ledger,reference))=>{value["comparison_ledger"]=json!(ledger);value["reference_comparison"]=json!(reference);}
                    Err(_)=>{value["reference_comparison"]=Value::Null;value["comparison_warning"]=json!("Optional reference comparison was not recorded; the RF evaluation receipt remains valid.");}
                }
                Ok(value)
            })
        } else if record_comparison_evidence {
            self.store.evaluate_candidate_with_comparison(&model, &dataset, role)
                .and_then(|(receipt, comparison)| {
                    let mut value = presentation(&receipt, false, !dataset.train().is_empty())?;
                    value["comparison_recording_requested"] = json!(true);
                    match comparison {
                        Ok(summary) => { value["comparison_ledger"] = json!(summary); }
                        Err(_) => {
                            value["comparison_ledger"] = Value::Null;
                            value["comparison_warning"] = json!("Optional comparison evidence was not recorded; the RF evaluation receipt remains valid. Existing comparison files were retained.");
                        }
                    }
                    Ok(value)
                })
        } else {
            self.store
                .evaluate_candidate(&model, &dataset, role)
                .and_then(|receipt| presentation(&receipt, false, !dataset.train().is_empty()))
        };
        match result {
            Ok(value) => {
                state.last_evaluation = Some(value.clone());
                state.evaluation_warning = None;
                Ok(value)
            }
            Err(error) => {
                state.last_evaluation = None;
                state.evaluation_warning=Some("Evaluation did not complete durably; previous evidence and the candidate were retained.".into());
                Err(bounded(error))
            }
        }
    }
}
