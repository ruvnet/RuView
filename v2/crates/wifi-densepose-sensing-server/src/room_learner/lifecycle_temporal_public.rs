//! Versioned owner workflow over the existing tagged Runtime. No imported
//! JSON report/proof/Complete Boolean creates producer authority.
use super::evidence_control::EvidencePurpose;
use super::*;
use crate::room_learner::temporal_wire::*;
fn schema(actual: &str, expected: &str) -> Result<(), String> {
    if actual != expected {
        return Err("learner_unsupported_schema".into());
    }
    Ok(())
}
fn sha(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl RoomLearningRuntime {
    pub fn temporal_configure_v2(
        &self,
        expected: &MutationGuard,
        context: CaptureContext,
        run: String,
        recovery_consent: bool,
        now: u64,
    ) -> Result<(), String> {
        if !recovery_consent {
            return Err("recovery_consent_required".into());
        }
        if context.sources.len() != 3 {
            return Err("learner_temporal_three_sources_required".into());
        }
        let store = self
            .recovery_store
            .clone()
            .filter(|s| s.available())
            .ok_or("recovery_unavailable")?;
        if store.inspect()?.is_some() {
            return Err("learner_recovery_pending".into());
        }
        self.configure_temporal_capture_with_custody_mode(
            expected,
            context,
            run,
            now,
            true,
            move |s, r, f, n, _, w| {
                TemporalTrainCapture::prepare_full_run_async(s, r, f, n, store, w)
            },
        )
    }
    pub fn temporal_begin_phase_v2(
        &self,
        request: TemporalPhaseStartRequestV2,
        now: u64,
    ) -> Result<(), String> {
        schema(&request.schema, PHASE_START_REQUEST_SCHEMA)?;
        if !request.conditions_confirmed {
            return Err("learner_conditions_confirmation_required".into());
        }
        self.start_temporal_phase_checked(&request.expected, now, |c| {
            let core = c
                .core
                .as_ref()
                .ok_or("temporal_initial_barrier_unconfirmed")?;
            if !core.public_phase_matches(
                &request.capture_generation_id,
                &request.schedule_sha256,
                request.episode_index as usize,
            ) {
                return Err("learner_phase_identity_changed".into());
            }
            if core.stage() == TemporalCaptureStage::Frozen && c.training_proof.is_none() {
                return Err("learner_training_proof_pending".into());
            }
            Ok(())
        })
    }
    pub fn temporal_finish_phase_v2(
        &self,
        request: TemporalPhaseFinishRequestV2,
        now: u64,
    ) -> Result<(), String> {
        schema(&request.schema, PHASE_FINISH_REQUEST_SCHEMA)?;
        if !request.conditions_confirmed {
            return Err("learner_conditions_confirmation_required".into());
        }
        self.confirm_temporal_phase_checked(
            &request.expected,
            &request.confirmation_challenge,
            now,
            |c| {
                if c.core.as_ref().is_none_or(|core| {
                    !core.public_phase_matches(
                        &request.capture_generation_id,
                        &request.schedule_sha256,
                        request.episode_index as usize,
                    )
                }) {
                    return Err("learner_phase_identity_changed".into());
                }
                Ok(())
            },
        )
    }
    fn public_generation_matches(&self, id: &str) -> Result<(), String> {
        if !sha(id) {
            return Err("learner_capture_identity_invalid".into());
        }
        let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        if !matches!(state.capture.as_ref(),Some(CaptureWorkflow::Temporal(c))if c.core.as_ref().is_some_and(|core|core.is_full_run()&&core.public_generation_id()==id))
        {
            return Err("learner_capture_identity_changed".into());
        }
        Ok(())
    }
    pub fn temporal_pause_phase_v2(
        &self,
        request: TemporalCurrentActionRequestV2,
        now: u64,
    ) -> Result<(), String> {
        schema(&request.schema, PHASE_CANCEL_REQUEST_SCHEMA)?;
        self.public_generation_matches(&request.capture_generation_id)?;
        self.pause_temporal_capture(&request.expected, now)
    }
    pub fn temporal_cancel_operation_v2(
        &self,
        request: TemporalCurrentActionRequestV2,
        _now: u64,
    ) -> Result<(), String> {
        schema(&request.schema, TRAIN_CANCEL_REQUEST_SCHEMA)?;
        let (fit, evidence) = {
            let mut state = self.command_state(&request.expected)?;
            let phase = state.job.phase;
            let cancel = state.job.cancel.clone();
            let Some(CaptureWorkflow::Temporal(c)) = state.capture.as_mut() else {
                return Err(PRIVATE_KIND.into());
            };
            if c.core
                .as_ref()
                .is_none_or(|core| core.public_generation_id() != request.capture_generation_id)
            {
                return Err("learner_capture_identity_changed".into());
            }
            if phase == JobPhase::Ready {
                return Ok(());
            }
            c.public_train_requested = false;
            let fit = c
                .fit_job
                .as_ref()
                .filter(|j| j.uses_cancel(&cancel))
                .cloned();
            let evidence = c
                .evidence_job
                .as_ref()
                .filter(|j| j.uses_cancel(&cancel))
                .cloned();
            if let Some(j) = &fit {
                j.cancel();
            }
            if let Some(j) = &evidence {
                j.cancel();
            }
            if busy(&state.job) {
                state.job.phase = JobPhase::Cancelling;
            }
            (fit, evidence)
        };
        if let Some(j) = fit {
            j.drain();
        }
        if let Some(j) = evidence {
            j.drain();
        }
        Ok(())
    }
    pub fn temporal_train_v2(
        self: &Arc<Self>,
        request: TemporalTrainRequestV2,
        now: u64,
    ) -> Result<String, String> {
        schema(&request.schema, TRAIN_REQUEST_SCHEMA)?;
        self.public_generation_matches(&request.capture_generation_id)?;
        let retry = {
            let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
            let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
                return Err(PRIVATE_KIND.into());
            };
            let core = c
                .core
                .as_ref()
                .ok_or("temporal_initial_barrier_unconfirmed")?;
            if core.public_candidate().is_some() {
                if core.stage() == TemporalCaptureStage::FullComplete {
                    4
                } else {
                    3
                }
            } else if c
                .fit_job
                .as_ref()
                .is_some_and(|job| job.uncancelled_completed())
                && c.diagnostic_fit.is_some()
            {
                if core.stage() == TemporalCaptureStage::Blocked {
                    2
                } else if core
                    .public_capture(&self.runtime_epoch, c.lead_in_until, now)
                    .durability_pending
                {
                    5
                } else {
                    1
                }
            } else {
                0
            }
        };
        if retry >= 3 && retry <= 4 {
            let attempt = self.start_temporal_evidence(
                &request.expected,
                now,
                if retry == 4 {
                    EvidencePurpose::RestoreComplete
                } else {
                    EvidencePurpose::TrainProof
                },
            )?;
            return Ok(attempt.to_string());
        }
        if retry != 0 {
            if retry == 2 {
                self.repair_temporal_capture(&request.expected, now)?;
            } else if retry == 5 {
                self.tick_temporal_capture(&request.expected, now)?;
            } else {
                self.prepare_frozen_transition(&request.expected, now)?;
            }
            {
                let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
                if let Some(CaptureWorkflow::Temporal(c)) = state.capture.as_mut() {
                    if c.failure.is_none()
                        && c.fit_job
                            .as_ref()
                            .is_some_and(|j| j.uncancelled_completed())
                        && c.core.as_ref().is_some_and(|core| {
                            core.public_generation_id() == request.capture_generation_id
                        })
                    {
                        c.public_train_requested = true;
                    }
                }
            }
            self.spawn_temporal_training_progress(request.capture_generation_id);
            return Ok(request
                .expected
                .workflow_revision
                .saturating_add(1)
                .to_string());
        }
        let attempt = self.start_temporal_fit(&request.expected, now)?;
        {
            let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
            let cancel = state.job.cancel.clone();
            if let Some(CaptureWorkflow::Temporal(c)) = state.capture.as_mut() {
                if c.core.as_ref().is_some_and(|core| {
                    core.public_generation_id() == request.capture_generation_id
                }) && c
                    .fit_job
                    .as_ref()
                    .is_some_and(|j| j.attempt_matches(attempt, &cancel))
                {
                    c.public_train_requested = true;
                }
            }
        }
        self.spawn_temporal_training_progress(request.capture_generation_id);
        Ok(attempt.to_string())
    }
    fn spawn_temporal_training_progress(self: &Arc<Self>, id: String) {
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                let Some(runtime) = weak.upgrade() else {
                    return;
                };
                let now = runtime.receiver_now_ns();
                let action = {
                    let state = match runtime.inner.lock() {
                        Ok(s) => s,
                        Err(_) => return,
                    };
                    let Some(CaptureWorkflow::Temporal(c)) = state.capture.as_ref() else {
                        return;
                    };
                    if !c.public_train_requested
                        || c.failure.is_some()
                        || c.core
                            .as_ref()
                            .is_none_or(|core| core.public_generation_id() != id)
                        || matches!(state.job.phase, JobPhase::Failed | JobPhase::Cancelled)
                    {
                        return;
                    }
                    if c.training_proof.is_some() {
                        return;
                    }
                    if c.core
                        .as_ref()
                        .unwrap()
                        .public_capture(&runtime.runtime_epoch, c.lead_in_until, now)
                        .durability_pending
                    {
                        0
                    } else {
                        match c.core.as_ref().unwrap().stage() {
                            TemporalCaptureStage::Complete
                                if state.job.phase == JobPhase::Ready
                                    && c.fit_job.as_ref().is_some_and(|j| j.done()) =>
                            {
                                1
                            }
                            TemporalCaptureStage::FullComplete if !busy(&state.job) => 3,
                            TemporalCaptureStage::Frozen
                            | TemporalCaptureStage::Paused
                            | TemporalCaptureStage::Ready
                                if !busy(&state.job)
                                    && c.core.as_ref().unwrap().public_candidate().is_some() =>
                            {
                                2
                            }
                            _ => 0,
                        }
                    }
                };
                let result = (|| -> Result<(), String> {
                    if action == 1 {
                        runtime.prepare_frozen_transition(&runtime.mutation_guard()?, now)?;
                    } else if action == 2 {
                        runtime.start_temporal_evidence(
                            &runtime.mutation_guard()?,
                            now,
                            EvidencePurpose::TrainProof,
                        )?;
                    } else if action == 3 {
                        runtime.start_temporal_evidence(
                            &runtime.mutation_guard()?,
                            now,
                            EvidencePurpose::RestoreComplete,
                        )?;
                    }
                    // Only already-authorized post-Train ACK progress; no
                    // capture/configuration/ground-truth is started by GET.
                    let _ = runtime.tick_temporal_capture(&runtime.mutation_guard()?, now);
                    Ok(())
                })();
                if let Err(error) = result {
                    if error != "learner_state_changed" && error != "learner_job_busy" {
                        if let Ok(mut state) = runtime.inner.lock() {
                            state.input_error = Some(bounded(error));
                        }
                        return;
                    }
                }
                drop(runtime);
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        });
    }
    pub fn temporal_evaluate_v2(
        self: &Arc<Self>,
        request: TemporalEvaluateRequestV2,
        now: u64,
    ) -> Result<String, String> {
        if request.record_comparison_evidence { return Err("reference_recording_scope_required".into()); }
        self.temporal_evaluate_with_references_v2(request, now, None)
    }
    pub fn temporal_evaluate_with_references_v2(
        self:&Arc<Self>, request:TemporalEvaluateRequestV2, now:u64,
        references:Option<crate::companion_references::ReferenceSnapshot>,
    )->Result<String,String>{
        if request.record_comparison_evidence != references.is_some() {
            return Err("reference_recording_scope_required".into());
        }
        schema(&request.schema, EVALUATE_REQUEST_SCHEMA)?;
        self.public_generation_matches(&request.capture_generation_id)?;
        {
            let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
            let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
                return Err(PRIVATE_KIND.into());
            };
            let proof = c
                .training_proof
                .as_ref()
                .ok_or("learner_training_proof_pending")?;
            let refs = c
                .core
                .as_ref()
                .and_then(|core| core.public_complete_refs(&proof.sha256))
                .ok_or("temporal_actual_complete_required")?;
            if refs != request.refs {
                return Err("learner_evidence_identity_changed".into());
            }
        }
        self.start_temporal_evidence_with_references(&request.expected, now, EvidencePurpose::HeldOutEvaluation, references)
            .map(|attempt| attempt.to_string())
    }
    pub fn temporal_status_v2(self: &Arc<Self>, now: u64) -> Result<Value, String> {
        // ACKs may already be ready. This tick does not issue new consent,
        // source samples, fit/proof/evaluation jobs or immutable metadata writes.
        let temporal = {
            let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
            matches!(state.capture, Some(CaptureWorkflow::Temporal(_)))
        };
        if temporal {
            let _ = self.tick_temporal_capture(&self.mutation_guard()?, now);
        }
        let generation = {
            self.inner
                .lock()
                .map_err(|_| "learner state unavailable")?
                .generation
        };
        let recovery = self
            .recovery_store
            .as_ref()
            .and_then(|store| TemporalTrainCapture::recovery_discovery(store, now));
        let mut response = {
            let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
            if state.generation != generation {
                return Err("learner_state_changed".into());
            }
            let capture = match state.capture.as_ref() {
                Some(CaptureWorkflow::Temporal(c)) => Some(c),
                _ => None,
            };
            let context = state.capture_context().cloned();
            let mut profile = TemporalProfileWire::engineering();
            let mut wire_capture = None;
            let mut candidate = None;
            let mut train_proof = None;
            let mut complete = None;
            let mut evaluation = None;
            let mut operation = None;
            if let Some(c) = capture {
                if let Some(core) = &c.core {
                    profile = core.public_profile();
                    wire_capture =
                        Some(core.public_capture(&self.runtime_epoch, c.lead_in_until, now));
                    candidate = core.public_candidate();
                    train_proof = c.training_proof.as_ref().map(|p| p.sha256.clone());
                    complete = train_proof
                        .as_ref()
                        .and_then(|sha| core.public_complete_refs(sha));
                }
                if let Some(saved) = &c.evaluation {
                    evaluation = Some(evaluation_wire(saved));
                }
                if let Some(job) = &c.evidence_job {
                    operation = Some(job.operation_name().into());
                } else if c.fit_job.is_some() {
                    operation = Some("train".into());
                }
            }
            TemporalLearnerStatusWire {
                schema: STATUS_SCHEMA.into(),
                enabled: true,
                available_model_kinds: if self
                    .recovery_store
                    .as_ref()
                    .is_some_and(|s| s.available())
                {
                    vec![WholeTemporalKind::ManagedTemporalV3]
                } else {
                    vec![]
                },
                temporal_profile: profile,
                workflow_kind: if capture.is_some() {
                    WorkflowKindWire::ManagedTemporalV3
                } else if state.capture.is_some() {
                    WorkflowKindWire::Legacy
                } else {
                    WorkflowKindWire::Idle
                },
                mutation_guard: self.guard_for(&state),
                context_digest: context.as_ref().map(CaptureContext::digest),
                room_digest: context.as_ref().map(|c| c.room_digest.clone()),
                source_node_ids: context
                    .as_ref()
                    .map(|c| c.sources.iter().map(|s| s.source_id).collect())
                    .unwrap_or_default(),
                recovery,
                capture: wire_capture,
                candidate,
                training_proof_sha256: train_proof,
                complete,
                evaluation,
                assessment: capture.and_then(|c| {
                    c.assessment
                        .as_ref()
                        .map(|a| a.assessment.receipt.summary.clone())
                }),
                model_selection: None,
                job_state: if capture.is_some_and(|c| {
                    c.public_train_requested
                        && c.training_proof.is_none()
                        && c.failure.is_none()
                        && !matches!(
                            state.job.phase,
                            JobPhase::Failed | JobPhase::Cancelled | JobPhase::Cancelling
                        )
                }) {
                    "running".into()
                } else {
                    serde_json::to_value(state.job.phase)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .into()
                },
                job_operation: operation,
                error_code: state
                    .job
                    .error
                    .as_ref()
                    .map(|_| "learner_operation_failed".into()),
                error: state
                    .job
                    .error
                    .clone()
                    .or_else(|| state.input_error.clone()),
                legacy_status: None,
            }
        };
        if response.workflow_kind != WorkflowKindWire::ManagedTemporalV3 {
            response.legacy_status = Some(self.status_without_checkpoint(now));
        }
        let selection = self.temporal_selection_v2(now, None, None)?;
        response.model_selection = Some(
            serde_json::from_value(
                selection
                    .get("model_selection")
                    .cloned()
                    .ok_or("learner_selection_projection")?,
            )
            .map_err(|_| "learner_selection_projection")?,
        );
        serde_json::to_value(response).map_err(|_| "learner_status_encoding".into())
    }
}

impl RoomLearningRuntime {
    pub fn temporal_qualify_v2(
        self: &Arc<Self>,
        request: TemporalQualifyRequestV2,
        now: u64,
    ) -> Result<String, String> {
        schema(&request.schema, QUALIFY_REQUEST_SCHEMA)?;
        self.public_generation_matches(&request.capture_generation_id)?;
        {
            let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
            let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
                return Err(PRIVATE_KIND.into());
            };
            let saved = c
                .evaluation
                .as_ref()
                .ok_or("temporal_evaluation_required")?;
            if request.refs.evaluation_receipt_sha256 != saved.receipt().sha256
                || request.refs.held_out_evidence_sha256 != saved.held_out().sha256
                || request.refs.complete.training_proof_sha256 != saved.train().sha256
                || c.core
                    .as_ref()
                    .and_then(|core| core.public_complete_refs(&saved.train().sha256))
                    .as_ref()
                    != Some(&request.refs.complete)
            {
                return Err("learner_evidence_identity_changed".into());
            }
        }
        self.start_temporal_evidence(&request.expected, now, EvidencePurpose::Qualification)
            .map(|a| a.to_string())
    }
    pub fn temporal_recovery_context_v2(
        &self,
        binding: &str,
        revision: u64,
        sources: &[u8],
        now: u64,
    ) -> Result<CaptureContext, String> {
        let store = self.temporal_store()?;
        let (context, _) = TemporalTrainCapture::saved_full_context(store, now)?;
        let mut ids: Vec<_> = context.sources.iter().map(|s| s.source_id).collect();
        ids.sort();
        let mut supplied = sources.to_vec();
        supplied.sort();
        if context.owner_scope != self.owner
            || context.installation_id != self.installation
            || context.room_digest != binding
            || context.revision != revision
            || ids != supplied
        {
            return Err("recovery_context_mismatch".into());
        }
        Ok(context)
    }
    pub fn temporal_restore_v2(
        self: &Arc<Self>,
        expected: &MutationGuard,
        context: &CaptureContext,
        capture_generation: &str,
        now: u64,
    ) -> Result<(), String> {
        let (_, id) = TemporalTrainCapture::saved_full_context(self.temporal_store()?, now)?;
        if id != capture_generation {
            return Err("learner_capture_identity_changed".into());
        }
        self.restore_temporal_full_run(expected, context, now)?;
        let reconcile = {
            let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
            if let Some(CaptureWorkflow::Temporal(c)) = state.capture.as_mut() {
                let needed = c
                    .core
                    .as_ref()
                    .is_some_and(|core| core.public_candidate().is_some());
                c.public_train_requested = needed;
                needed
            } else {
                false
            }
        };
        if reconcile {
            self.spawn_temporal_training_progress(capture_generation.into());
        }
        Ok(())
    }
    pub fn temporal_discard_v2(
        &self,
        expected: &MutationGuard,
        capture_generation: &str,
        now: u64,
    ) -> Result<(), String> {
        let store = self.temporal_store()?;
        if TemporalTrainCapture::retirement_generation(store, now)? != capture_generation {
            return Err("learner_capture_identity_changed".into());
        }
        // Stop/drain actual accepted work before the retirement commit. No
        // late accepted write may recreate the retired recovery head.
        let (writer, fit, evidence, generation) = {
            let mut state = self.command_state(expected)?;
            if let Some(CaptureWorkflow::Temporal(c)) = state.capture.as_mut() {
                c.public_train_requested = false;
                c.invalidate_owner();
                let writer = c.core.as_ref().and_then(|c| c.writer());
                if let Some(w) = &writer {
                    w.cancel();
                }
                (
                    writer,
                    c.fit_job.clone(),
                    c.evidence_job.clone(),
                    state.generation,
                )
            } else {
                (None, None, None, state.generation)
            }
        };
        if let Some(w) = writer {
            w.drain()?;
        }
        if let Some(j) = fit {
            j.drain();
        }
        if let Some(j) = evidence {
            j.drain();
        }
        let inspection = store
            .inspect_temporal_retirement(now)?
            .ok_or("recovery_missing")?;
        let intent = store.prepare_inspected_retirement(&inspection, now, false)?;
        store.publish_temporal(&intent, None, now)?;
        let retired = {
            let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
            if state.generation != generation {
                return Err("learner_state_changed".into());
            }
            state.generation = state
                .generation
                .checked_add(1)
                .ok_or("learner generation exhausted")?;
            state.workflow_revision = state.workflow_revision.saturating_add(1);
            state.job.phase = JobPhase::Idle;
            state.input_error = None;
            state.capture.take()
        };
        drop(retired);
        Ok(())
    }
}

impl RoomLearningRuntime {
    pub fn paired_temporal_progress_snapshot(
        &self,
        installation_id: &str,
        boot_epoch: &str,
        room: Option<crate::room_learner::lifecycle::PublishedProgressRoom>,
        ready_ids: &[u8],
        ready_context: Option<&str>,
        now: u64,
    ) -> Result<TemporalPairedProgressWire, String> {
        const MAX_SAFE: u64 = 9_007_199_254_740_991;
        let observed = now / 1_000_000;
        if installation_id.is_empty()
            || installation_id.len() > 128
            || installation_id.chars().any(char::is_control)
            || boot_epoch.is_empty()
            || boot_epoch.len() > 128
            || boot_epoch.chars().any(char::is_control)
            || observed == 0
            || observed > MAX_SAFE - 10_000
            || room.as_ref().is_some_and(|r| {
                !sha(&r.binding_digest) || r.revision == 0 || r.revision > MAX_SAFE
            })
        {
            return Err("progress_identity_or_time_unavailable".into());
        }
        let mut result = TemporalPairedProgressWire {
            schema: PAIRED_PROGRESS_SCHEMA.into(),
            success: true,
            installation_id: installation_id.into(),
            runtime_epoch: boot_epoch.into(),
            observed_at_unix_ms: observed,
            expires_at_unix_ms: observed + 10_000,
            room,
            context_match: false,
            stage: "setup".into(),
            state: "idle".into(),
            model_kind: None,
            profile: None,
            capture: None,
            candidate: None,
            evaluation: None,
            qualification: None,
            selection: None,
            sources: None,
            save_status: "not_reported".into(),
            blocker: Some("room_not_published".into()),
        };
        {
            let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
            let context = state.capture_context().or_else(|| {
                state
                    .selection
                    .temporal
                    .as_ref()
                    .map(|s| &s.stored.envelope().receipt.selected.context)
            });
            if let (Some(published), Some(context)) = (result.room.as_ref(), context) {
                // The full binding digest includes the monotonic publication revision.
                // Context.revision is a separate learner structural fingerprint.
                if installation_id != self.installation
                    || context.installation_id != installation_id
                    || context.room_digest != published.binding_digest
                    || state.selection.temporal.as_ref().is_some_and(|selected| {
                        let c = &selected.stored.envelope().receipt.selected.context;
                        c.room_digest != published.binding_digest
                            || c.digest() != context.digest()
                    })
                {
                    result.stage = "blocked".into();
                    result.blocker = Some("context_mismatch".into());
                } else {
                    result.context_match = true;
                    result.model_kind = Some(WholeTemporalKind::ManagedTemporalV3);
                    result.state = serde_json::to_value(state.job.phase)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .into();
                    if ready_context != Some(context.digest().as_str()) {
                        return Err("progress_context_changed".into());
                    }
                    result.sources = Some(TemporalPairedSourcesWire {
                        required: context
                            .sources
                            .len()
                            .try_into()
                            .map_err(|_| "progress_source_count")?,
                        ready: context
                            .sources
                            .iter()
                            .filter(|s| ready_ids.contains(&s.source_id))
                            .count() as u8,
                    });
                    result.blocker = None;
                    if let Some(CaptureWorkflow::Temporal(c)) = &state.capture {
                        if let Some(core) = &c.core {
                            let capture =
                                core.public_capture(&self.runtime_epoch, c.lead_in_until, now);
                            result.stage = capture.stage.clone();
                            result.profile = Some(core.public_profile());
                            result.candidate = core.public_candidate();
                            result.capture = Some(TemporalPairedCaptureProgressWire {
                                capture_generation_id: capture.capture_generation_id,
                                schedule_sha256: capture.schedule_sha256,
                                stage: capture.stage,
                                current_episode: capture.current_episode,
                                role: capture.role,
                                label: capture.label,
                                target_ms: capture.target_ms,
                                lead_in_remaining_ms: capture.lead_in_remaining_ms,
                                observed_ms: capture.observed_ms,
                                retained_windows: capture.retained_windows,
                                durable_windows: capture.durable_windows,
                                sealed_train_episodes: capture.sealed_train_episodes,
                                sealed_held_out_episodes: capture.sealed_held_out_episodes,
                                durability_pending: capture.durability_pending,
                                awaiting_ground_truth: capture.awaiting_ground_truth,
                                original_deadline: capture.original_deadline,
                            });
                        } else {
                            result.stage = "preparing".into();
                        }
                        if let Some(saved) = &c.evaluation {
                            result.evaluation = Some(evaluation_wire(saved));
                        }
                        if let Some(a) = &c.assessment {
                            result.qualification =
                                a.qualification.as_ref().map(|q| q.receipt.summary.clone());
                        }
                    } else if state.capture.is_some() {
                        result.stage = "unsupported_mixed_capture".into();
                        result.blocker = Some("legacy_capture_with_temporal_selection".into());
                    } else {
                        result.stage = "selected".into();
                    }
                }
            }
        }
        if !result.context_match {
            result.room = None;
            return Ok(result);
        }
        let selection = self.temporal_selection_v2(
            now,
            result.room.as_ref().map(|r| r.binding_digest.as_str()),
            Some(boot_epoch),
        )?;
        let summary: TemporalSelectionStatusWire = serde_json::from_value(
            selection
                .get("model_selection")
                .cloned()
                .ok_or("progress_selection_projection")?,
        )
        .map_err(|_| "progress_selection_projection")?;
        if summary.selected.is_some() {
            result.model_kind = Some(WholeTemporalKind::ManagedTemporalV3);
            if result.qualification.is_none() {
                result.qualification = summary.qualification.clone();
            }
        }
        result.selection = Some(summary);
        Ok(result)
    }
}

fn evaluation_wire(
    saved: &crate::room_learner::artifact::StoredTemporalEvaluation,
) -> TemporalEvaluationWire {
    let identity = &saved.receipt().body.identity;
    let refs = TemporalEvaluationRefsWire {
        complete: TemporalCompleteRefsWire {
            model: TemporalModelIdentityWire {
                model_kind: WholeTemporalKind::ManagedTemporalV3,
                model_sha256: identity.artifact_sha256.clone(),
                verified_fit_record_sha256: identity.verified_fit_record_sha256.clone(),
            },
            training_proof_sha256: saved.train().sha256.clone(),
            complete_record_sha256: saved.held_out().body.complete_record_sha256.clone(),
            complete_commitment_sha256: saved
                .held_out()
                .body
                .complete_commitment
                .commitment_sha256
                .clone(),
        },
        held_out_evidence_sha256: saved.held_out().sha256.clone(),
        evaluation_receipt_sha256: saved.receipt().sha256.clone(),
    };
    TemporalEvaluationWire {
        refs,
        window_unit: "native_receiver_2s_window".into(),
        evaluated_at_unix_ms: saved.receipt().body.evaluated_at_unix_ms,
        report: saved.receipt().body.report.clone(),
        durable: true,
        independence_verified: false,
        physically_validated: false,
    }
}

#[cfg(all(test, unix))]
#[path = "lifecycle_temporal_public_acceptance_tests.rs"]
mod acceptance;
