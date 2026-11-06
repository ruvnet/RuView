//! Local operator qualification and explicit persisted selection; no automatic promotion.
use super::*;
use crate::room_learner::{
    qualification::{hash, QualificationEnvelope},
    selection::*,
};
use sha2::{Digest, Sha256};

#[derive(Default)]
pub(super) struct SelectionState {
    envelope: Option<SelectionEnvelope>,
    model: Option<Arc<RoomModel>>,
    qualification: Option<QualificationEnvelope>,
    error: Option<String>,
    assessment: Option<crate::room_learner::assessment::AssessmentEnvelope>,
}
impl SelectionState {
    pub(super) fn has_history(&self) -> bool {
        self.envelope.is_some() || self.model.is_some() || self.qualification.is_some()
            || self.assessment.is_some() || self.error.is_some()
    }
}
impl RoomLearningRuntime {
    pub(super) fn restore_selection(&self, state: &mut Inner) {
        let restored = (|| -> Result<(), String> {
            let Some(envelope) = self.store.load_selection()? else {
                return Ok(());
            };
            state.selection.envelope = Some(envelope.clone());
            if let Some(target) = &envelope.receipt.selected {
                let (model, q) = self.store.resolve_selection_target(target)?;
                let model = Arc::new(model);
                state.capture = Some(CaptureWorkflow::Legacy(if let Some(proof)=self.store.load_managed_training_proof(&model)? {
                    let fence=self.store.load_managed_capture_fence(&model,&proof)?;
                    CaptureSession::resume_managed_fence(&proof,&fence)?
                } else {CaptureSession::new(target.context.clone(),format!("selected-{}",envelope.receipt.revision))?}));
                state.model = Some(model.clone());
                state.frozen_training = true;
                state.job.phase = JobPhase::Ready;
                state.qualification = self.store.load_latest_qualification(&model)?;
                state.selection.assessment = self.store.load_latest_assessment(&model)?;
                state.selection.model = Some(model);
                state.selection.qualification = Some(q);
            }
            Ok(())
        })();
        if let Err(e) = restored {
            state.selection.error = Some(bounded(e));
        }
    }
    pub fn qualify(&self, expected: &MutationGuard) -> Result<QualificationEnvelope, String> {
        let mut state = self.command_state(expected)?;
        state.require_legacy_kind()?;
        let result = (|| -> Result<QualificationEnvelope, String> {
            if busy(&state.job) || !state.frozen_training {
                return Err("wait for the frozen candidate".into());
            }
            let model = state.model.as_ref().ok_or("no candidate")?.clone();
            let capture = state.legacy_capture().ok_or("no capture context")?;
            if capture.phase_active() {
                return Err("finish the current capture phase first".into());
            }
            let _policy = crate::room_learner::qualification::QualificationPolicy::default();
            let dataset = capture.validated_dataset()?;
            let evaluation = self
                .store
                .evaluate_candidate(&model, &dataset, DataRole::HeldOut)?;
            // Incomplete data or missing lineage is not a completed failed assessment.
            let proof=self.store.load_managed_training_proof(&model)?;
            let fence=if let Some(proof)=&proof {
                let fence=self.store.load_managed_capture_fence(&model,proof)?;
                let current=capture.managed_fence_seed(proof)?;
                current.ensure_extends(&fence.seed)?;
                if current.phase_claims!=fence.seed.phase_claims {return Err("managed sealed phases are not durably published".into());}
                Some(fence)
            } else {None};
            let evidence = crate::room_learner::qualification::managed_evidence_with_fence(
                &model,
                &dataset,
                capture.history(),
                &evaluation,
                proof.as_ref(),
                fence.as_ref().map(|f|f.fence_sha256.as_str()),
            )?;
            let outcome =
                QualificationEnvelope::issue_with_proof(&model, &dataset, capture.history(), &evaluation,proof.as_ref(),fence.as_ref().map(|f|f.fence_sha256.as_str()));
            let assessment = crate::room_learner::assessment::AssessmentEnvelope::issue(
                &model,
                &evaluation,
                &evidence,
                outcome.as_ref().ok(),
            )?;
            state.last_evaluation = Some(super::evaluation_control::presentation(
                &evaluation,
                false,
                !dataset.train().is_empty(),
            )?);
            state.evaluation_warning = None;
            if let Ok(q) = &outcome {
                self.store.save_qualification(&model, q)?;
            }
            if let Err(e) = self.store.save_assessment(&model, &assessment) {
                if state
                    .selection
                    .envelope
                    .as_ref()
                    .and_then(|e| e.receipt.selected.as_ref())
                    .is_some_and(|t| t.model_sha256 == evaluation.model_sha256)
                {
                    state.selection.error=Some("Latest qualification persistence could not be verified; restart and inspect retained evidence.".into());
                }
                return Err(e);
            }
            if state
                .selection
                .envelope
                .as_ref()
                .and_then(|e| e.receipt.selected.as_ref())
                .is_some_and(|t| t.model_sha256 == evaluation.model_sha256)
            {
                state.selection.assessment = Some(assessment);
            }
            outcome
        })();
        match result {
            Ok(q) => {
                state.qualification = Some(q.clone());
                state.qualification_error = None;
                Ok(q)
            }
            Err(e) => {
                state.qualification = None;
                state.qualification_error = Some(bounded(e.clone()));
                Err(e)
            }
        }
    }
    fn publish_selection(
        &self,
        state: &mut Inner,
        target: Option<SelectionTarget>,
        previous: Option<SelectionTarget>,
    ) -> Result<SelectionEnvelope, String> {
        let resolved = target
            .as_ref()
            .map(|t| self.store.resolve_selection_target(t))
            .transpose()?;
        let assessment = resolved
            .as_ref()
            .map(|(model, q)| self.store.current_qualification_assessment(model, q))
            .transpose()?;
        let old_revision = state
            .selection
            .envelope
            .as_ref()
            .map_or(0, |s| s.receipt.revision);
        let receipt = SelectionReceipt {
            schema: "ruview.room-learner.model-selection.v1".into(),
            owner_scope: self.owner.clone(),
            installation_id: self.installation.clone(),
            revision: old_revision
                .checked_add(1)
                .ok_or("selection revision exhausted")?,
            selected_at_unix_ms: self.receiver_now_ns() / 1_000_000,
            selected: target,
            previous,
        };
        let envelope = SelectionEnvelope {
            receipt_sha256: hash(&receipt)?,
            receipt,
        };
        if let Err(error) = self.store.save_selection(&envelope, old_revision) {
            state.selection.error=Some("Selection persistence outcome is unavailable; restart and verify the saved selection before continuing.".into());
            return Err(error);
        }
        // Only publish in-memory authority after durable pointer and readback succeed.
        state.selection = SelectionState {
            envelope: Some(envelope.clone()),
            model: resolved.as_ref().map(|(m, _)| Arc::new(m.clone())),
            qualification: resolved.map(|(_, q)| q),
            error: None,
            assessment,
        };
        Ok(envelope)
    }
    pub fn select_model(
        &self,
        expected: &MutationGuard,
        model_sha256: &str,
        qualification_sha256: &str,
    ) -> Result<SelectionEnvelope, String> {
        let mut state = self.command_state(expected)?;
        state.require_legacy_kind()?;
        if busy(&state.job) || state.legacy_capture().is_some_and(|c| c.phase_active()) {
            return Err("finish active capture/training before selecting".into());
        }
        let model = state
            .model
            .as_ref()
            .ok_or("no current candidate to select")?;
        let sha = format!("{:x}", Sha256::digest(model.export()?));
        if sha != model_sha256 {
            return Err("candidate changed before selection".into());
        }
        let qualification = self.store.load_qualification(model, qualification_sha256)?;
        self.store
            .current_qualification_assessment(model, &qualification)?;
        let target = SelectionTarget {
            context: model.context().clone(),
            model_sha256: sha,
            model_version: model.version,
            qualification_sha256: qualification_sha256.into(),
        };
        let previous = state
            .selection
            .envelope
            .as_ref()
            .and_then(|e| e.receipt.selected.clone());
        if previous.as_ref() == Some(&target) {
            if state.selection.error.is_some()
                || self.store.load_selection()?.as_ref() != state.selection.envelope.as_ref()
            {
                state.selection.error = Some(
                    "Saved selection differs from runtime; restart before selecting again.".into(),
                );
                return Err("saved selection changed".into());
            }
            return Ok(state
                .selection
                .envelope
                .as_ref()
                .ok_or("selection unavailable")?
                .clone());
        }
        self.publish_selection(&mut state, Some(target), previous)
    }
    pub fn deselect_model(&self, expected: &MutationGuard) -> Result<SelectionEnvelope, String> {
        let mut state = self.command_state(expected)?;
        state.require_legacy_kind()?;
        if busy(&state.job) || state.legacy_capture().is_some_and(|c| c.phase_active()) {
            return Err("finish active capture/training before changing selection".into());
        }
        if let Some(e) = &state.selection.envelope {
            if e.receipt.selected.is_none() && state.selection.error.is_none() {
                if self.store.load_selection()?.as_ref() != Some(e) {
                    return Err("saved selection changed".into());
                }
                return Ok(e.clone());
            }
        }
        let previous = state
            .selection
            .envelope
            .as_ref()
            .and_then(|e| e.receipt.selected.clone());
        self.publish_selection(&mut state, None, previous)
    }
    pub fn rollback_model(&self, expected: &MutationGuard) -> Result<SelectionEnvelope, String> {
        let mut state = self.command_state(expected)?;
        state.require_legacy_kind()?;
        if busy(&state.job) || state.legacy_capture().is_some_and(|c| c.phase_active()) {
            return Err("finish active capture/training before rollback".into());
        }
        let previous = state
            .selection
            .envelope
            .as_ref()
            .and_then(|e| e.receipt.previous.clone())
            .ok_or("no prior selection")?;
        let context = state.legacy_capture()
            .map(|c| c.context())
            .ok_or("no current context")?;
        if *context != previous.context {
            return Err("rollback context does not match current installation".into());
        }
        let current = state
            .selection
            .envelope
            .as_ref()
            .and_then(|e| e.receipt.selected.clone());
        self.publish_selection(&mut state, Some(previous), current)
    }
    fn selected_identity(state: &Inner) -> Option<SelectedModelIdentity> {
        let e = state.selection.envelope.as_ref()?;
        let t = e.receipt.selected.as_ref()?;
        let q = state.selection.qualification.as_ref()?;
        Some(SelectedModelIdentity {
            selection_sha256: e.receipt_sha256.clone(),
            selection_revision: e.receipt.revision,
            model_sha256: t.model_sha256.clone(),
            model_version: t.model_version,
            qualification_sha256: t.qualification_sha256.clone(),
            policy_sha256: q.receipt.policy_sha256.clone(),
            context_digest: t.context.digest(),
            room_digest: t.context.room_digest.clone(),
            source_node_ids: t.context.sources.iter().map(|s| s.source_id).collect(),
            selected_at_unix_ms: e.receipt.selected_at_unix_ms,
            evaluated_at_unix_ms: q.receipt.evaluated_at_unix_ms,
            authority_scope: "local_operator_binary_presence".into(),
        })
    }
    fn selected_evidence_locked(
        &self,
        state: &Inner,
        now_ns: u64,
        expected_room_binding: Option<&str>,
    ) -> SelectedBinaryEvidence {
        let identity = Self::selected_identity(state);
        let unavailable = |reason: String| SelectedBinaryEvidence::Unavailable {
            identity: identity.clone(),
            reason: bounded(reason),
        };
        if let Some(e) = &state.selection.error {
            return unavailable(e.clone());
        }
        let Some(target) = state
            .selection
            .envelope
            .as_ref()
            .and_then(|e| e.receipt.selected.as_ref())
        else {
            return SelectedBinaryEvidence::None;
        };
        let result = (|| -> Result<SelectedBinaryEvidence, String> {
            if expected_room_binding != Some(target.context.room_digest.as_str()) {
                return Err("selected model does not match the current published room".into());
            }
            let identity = identity
                .clone()
                .ok_or("selected qualification unavailable")?;
            let model = state
                .selection
                .model
                .as_ref()
                .ok_or("selected model unavailable")?;
            let q = state
                .selection
                .qualification
                .as_ref()
                .ok_or("selected qualification unavailable")?;
            if q.receipt.policy
                != crate::room_learner::qualification::QualificationPolicy::default()
            {
                return Err("selected policy is not current".into());
            }
            let assessment = state
                .selection
                .assessment
                .as_ref()
                .ok_or("latest completed qualification assessment unavailable")?;
            if assessment.receipt.failure_reason.is_some() {
                return Err("newer_qualification_failed".into());
            }
            if assessment.receipt.qualification_sha256.as_deref() != Some(q.receipt_sha256.as_str())
            {
                return Err("new qualification requires explicit selection".into());
            }
            if let Some(error) = &state.input_error {
                return Err(format!("selected input is unverified: {error}"));
            }
            let capture = state.legacy_capture()
                .ok_or("awaiting current selected sources")?;
            if capture.context() != &target.context {
                return Err("selected context differs from current RF source contract".into());
            }
            let cohort = capture.live_cohort(now_ns)?;
            let prediction = model.infer(&cohort);
            if prediction.decision == crate::room_learner::model::PresenceDecision::Abstain {
                return Err(prediction
                    .reason
                    .unwrap_or_else(|| "selected model abstained".into()));
            }
            let probability = prediction
                .occupied_probability
                .filter(|x| x.is_finite() && (0.0..=1.0).contains(x))
                .ok_or("selected probability unavailable")?;
            let observed = cohort
                .views
                .iter()
                .map(|v| v.last_time_ns)
                .min()
                .ok_or("no selected source window")?
                / 1_000_000;
            // Source identities, not poll time/live-cohort id/age, define repeated observations.
            let windows: Vec<_> = cohort
                .views
                .iter()
                .map(|v| {
                    (
                        v.source_id,
                        &v.frame_ids,
                        &v.sequence_ids,
                        v.first_time_ns,
                        v.last_time_ns,
                    )
                })
                .collect();
            let observation_digest = hash(&(target.context.digest(), windows))?;
            Ok(SelectedBinaryEvidence::Ready {
                identity,
                presence: prediction.decision
                    == crate::room_learner::model::PresenceDecision::Occupied,
                decision: prediction.decision,
                occupied_probability: probability,
                observed_at_unix_ms: observed,
                observation_digest,
            })
        })();
        result.unwrap_or_else(unavailable)
    }
    pub fn selected_binary_evidence(
        &self,
        now_ns: u64,
        expected_room_binding: Option<&str>,
    ) -> SelectedBinaryEvidence {
        match self.inner.lock() {
            Ok(state) if state.require_legacy_kind().is_err() => SelectedBinaryEvidence::Unavailable { identity: None, reason: "learner_temporal_capture_private_unsupported_command".into() },
            Ok(state) => self.selected_evidence_locked(&state, now_ns, expected_room_binding),
            Err(_) => SelectedBinaryEvidence::Unavailable {
                identity: None,
                reason: "learner state unavailable".into(),
            },
        }
    }
    pub(super) fn selection_status_locked(
        &self,
        state: &Inner,
        now_ns: u64,
        binding: Option<&str>,
    ) -> ModelSelectionStatus {
        ModelSelectionStatus {
            schema: "ruview.room-learner.model-selection-status.v1",
            revision: state
                .selection
                .envelope
                .as_ref()
                .map_or(0, |e| e.receipt.revision),
            selected: Self::selected_identity(state),
            previous_available: state
                .selection
                .envelope
                .as_ref()
                .is_some_and(|e| e.receipt.previous.is_some()),
            qualification: state.qualification.as_ref().map(QualificationSummary::from),
            qualification_error: state.qualification_error.clone(),
            integrity_error: state.selection.error.clone(),
            current: self.selected_evidence_locked(state, now_ns, binding),
        }
    }
    pub fn selection_snapshot(
        &self,
        now_ns: u64,
        expected_room_binding: Option<&str>,
    ) -> ModelSelectionStatus {
        match self.inner.lock() {
            Ok(state) => {
                let mut snapshot =
                    self.selection_status_locked(&state, now_ns, expected_room_binding);
                if let Err(reason) = state.require_legacy_kind() {
                    snapshot.integrity_error = Some(reason.clone());
                    snapshot.current = SelectedBinaryEvidence::Unavailable { identity: None, reason };
                    return snapshot;
                }
                let verified = (|| -> Result<(), String> {
                    if self.store.load_selection()?.as_ref() != state.selection.envelope.as_ref() {
                        return Err("saved selection differs from current runtime".into());
                    }
                    if let Some(target) = state
                        .selection
                        .envelope
                        .as_ref()
                        .and_then(|e| e.receipt.selected.as_ref())
                    {
                        let (model, q) = self.store.resolve_selection_target(target)?;
                        self.store.current_qualification_assessment(&model, &q)?;
                    }
                    Ok(())
                })();
                if let Err(e) = verified {
                    snapshot.integrity_error = Some(bounded(e.clone()));
                    snapshot.current = SelectedBinaryEvidence::Unavailable {
                        identity: snapshot.selected.clone(),
                        reason: bounded(e),
                    };
                }
                snapshot
            }
            Err(_) => ModelSelectionStatus {
                schema: "ruview.room-learner.model-selection-status.v1",
                revision: 0,
                selected: None,
                previous_available: false,
                qualification: None,
                qualification_error: None,
                integrity_error: Some("learner state unavailable".into()),
                current: SelectedBinaryEvidence::Unavailable {
                    identity: None,
                    reason: "learner state unavailable".into(),
                },
            },
        }
    }
    pub(super) fn candidate_identity(&self, state: &Inner) -> Option<Value> {
        let model = state.model.as_ref()?;
        let sha = format!("{:x}", Sha256::digest(model.export().ok()?));
        Some(
            json!({"model_sha256":sha,"model_version":model.version,"context_digest":model.context().digest(),
            "room_digest":model.context().room_digest,"persisted":self.store.load_exact_candidate(model.context(),&sha).is_ok()}),
        )
    }
}
