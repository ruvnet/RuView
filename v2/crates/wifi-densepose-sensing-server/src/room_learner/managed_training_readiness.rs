//! Qualification capture preflight, independent from a model's diagnostic availability.
use super::*;
use crate::room_learner::{
    capture::PhaseStatus,
    qualification::{hash, CONDITIONS},
};
impl RoomLearningRuntime {
    pub(super) fn qualification_readiness_locked(&self, state: &Inner, now_ns: u64) -> Value {
        if state.require_legacy_kind().is_err() {
            return json!({"schema":"ruview.qualification-readiness.v1","eligible_for_held_out":false,"training_lineage_basis":"unavailable","managed_training_proof_sha256":null,"raw_train_rows_retained":false,"train_cohorts_per_condition":[0,0,0,0],"reason":"unsupported_capture_kind","detail":"Private temporal capture has no fitting or qualification authority."});
        }
        let retained = state.legacy_capture()
            .is_some_and(|c| c.cohorts().iter().any(|r| r.role == DataRole::Train));
        let result = (|| -> Result<(&str, [usize; 4], Option<String>), (&str, String)> {
            let model = state.model.as_ref().ok_or((
                "missing_candidate",
                "Train or restore a candidate first.".into(),
            ))?;
            use sha2::{Digest, Sha256};
            let model_bytes = model.export().map_err(|e| ("missing_candidate", e))?;
            self.store
                .load_exact_candidate(
                    model.context(),
                    &format!("{:x}", Sha256::digest(&model_bytes)),
                )
                .map_err(|e| {
                    (
                        "missing_candidate",
                        format!("Frozen candidate artifact could not be verified: {e}"),
                    )
                })?;
            let capture = state.legacy_capture().ok_or((
                "context_mismatch",
                "No current managed capture context.".into(),
            ))?;
            if capture.context() != model.context() {
                return Err((
                    "context_mismatch",
                    "Candidate does not match this room/source context.".into(),
                ));
            }
            if now_ns < capture.minimum_receiver_time_ns() {
                return Err(("receiver_clock_incomparable","The current receiver clock predates the original managed capture; no automatic clock/counter rebase is permitted.".into()));
            }
            let train: Vec<_> = capture
                .cohorts()
                .iter()
                .filter(|c| c.role == DataRole::Train)
                .collect();
            if !train.is_empty() {
                capture
                    .validated_dataset()
                    .map_err(|e| ("invalid_managed_training_proof", e))?;
                if hash(&train).map_err(|e| ("retained_train_mismatch", e))?
                    != model.training_manifest().training_data_sha256
                {
                    return Err((
                        "retained_train_mismatch",
                        "Retained Train rows differ from this frozen model.".into(),
                    ));
                }
                let counts =
                    CONDITIONS.map(|label| train.iter().filter(|c| c.label == label).count());
                if counts.iter().any(|n| *n < 4)
                    || train.iter().any(|row| {
                        let phases: Vec<_> = capture
                            .history()
                            .iter()
                            .filter(|p| {
                                p.role == DataRole::Train
                                    && p.status == PhaseStatus::Sealed
                                    && p.trial_id == row.trial_id
                                    && p.session_id == row.session_id
                                    && p.label == row.label
                            })
                            .collect();
                        phases.len() != 1
                            || phases[0].cohort_count
                                != train
                                    .iter()
                                    .filter(|r| {
                                        r.trial_id == row.trial_id
                                            && r.session_id == row.session_id
                                            && r.label == row.label
                                    })
                                    .count()
                    })
                {
                    return Err((
                        "insufficient_managed_train",
                        "Four original managed sealed Train cohorts per condition are required."
                            .into(),
                    ));
                }
                if let Some(proof) = self
                    .store
                    .load_managed_training_proof(model)
                    .map_err(|e| ("invalid_managed_training_proof", e))?
                {
                    let fence = self
                        .store
                        .load_managed_capture_fence(model, &proof)
                        .map_err(|e| ("invalid_managed_training_proof", e))?;
                    capture
                        .managed_fence_seed(&proof)
                        .and_then(|s| s.ensure_extends(&fence.seed))
                        .map_err(|e| ("invalid_managed_training_proof", e))?;
                }
                return Ok(("retained_exact_train", counts, None));
            }
            let proof=self.store.load_managed_training_proof(model).map_err(|e|("invalid_managed_training_proof",e))?
                .ok_or(("missing_managed_training_proof","This diagnostic model has no retained Train rows or verified managed training proof; new held-out captures cannot qualify it.".into()))?;
            let fence = self
                .store
                .load_managed_capture_fence(model, &proof)
                .map_err(|e| ("invalid_managed_training_proof", e))?;
            capture
                .managed_fence_seed(&proof)
                .and_then(|s| s.ensure_extends(&fence.seed))
                .map_err(|e| ("invalid_managed_training_proof", e))?;
            proof
                .validate(model)
                .map_err(|e| ("invalid_managed_training_proof", e))?;
            Ok((
                "managed_training_proof_v1",
                proof.seed.train_counts,
                Some(proof.proof_sha256),
            ))
        })();
        match result {
            Ok((basis, counts, sha)) => {
                json!({"schema":"ruview.qualification-readiness.v1","eligible_for_held_out":true,"training_lineage_basis":basis,"managed_training_proof_sha256":sha,"raw_train_rows_retained":retained,"train_cohorts_per_condition":counts,"reason":null,"detail":null})
            }
            Err((reason, detail)) => {
                json!({"schema":"ruview.qualification-readiness.v1","eligible_for_held_out":false,"training_lineage_basis":"unavailable","managed_training_proof_sha256":null,"raw_train_rows_retained":retained,"train_cohorts_per_condition":[0,0,0,0],"reason":reason,"detail":bounded(detail)})
            }
        }
    }
}
