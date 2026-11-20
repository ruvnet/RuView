//! Whole-kind selected state; same protected pointer/journal and live input.
use super::*;
use crate::room_learner::{
    artifact::StoredTemporalSelection, temporal_live_driver::ContinuousTemporalInput,
    temporal_wire::*,
};
pub(in crate::room_learner::lifecycle) struct TemporalSelectedRuntime {
    pub(in crate::room_learner::lifecycle) stored: StoredTemporalSelection,
    pub(in crate::room_learner::lifecycle) driver: Mutex<ContinuousTemporalInput>,
    pub(super) invalid: AtomicBool,
}
impl TemporalSelectedRuntime {
    pub(in crate::room_learner::lifecycle) fn new(
        stored: StoredTemporalSelection,
    ) -> Result<Self, String> {
        let driver = ContinuousTemporalInput::from_selected(&stored)?;
        Ok(Self {
            stored,
            driver: Mutex::new(driver),
            invalid: AtomicBool::new(false),
        })
    }
}
impl RoomLearningRuntime {
    pub fn temporal_progress_context(&self) -> Option<CaptureContext> {
        let state = self.inner.lock().ok()?;
        state.capture_context().cloned().or_else(|| {
            state
                .selection
                .temporal
                .as_ref()
                .map(|s| s.stored.envelope().receipt.selected.context.clone())
        })
    }
    pub fn temporal_v2_state_present(&self) -> bool {
        self.inner.lock().map(|s|matches!(s.capture.as_ref(),Some(CaptureWorkflow::Temporal(c))if c.core.as_ref().is_some_and(|core|core.is_full_run()))||s.selection.temporal.is_some()).unwrap_or(true)
    }
    pub fn temporal_selection_v2(
        &self,
        now: u64,
        expected_room_binding: Option<&str>,
        boot_epoch: Option<&str>,
    ) -> Result<Value, String> {
        self.temporal_selection_snapshot(now, expected_room_binding, boot_epoch, true)
    }
    pub fn temporal_selected_binary_v2(
        &self,
        now: u64,
        expected_room_binding: Option<&str>,
        boot_epoch: Option<&str>,
    ) -> Result<Value, String> {
        let snapshot =
            self.temporal_selection_snapshot(now, expected_room_binding, boot_epoch, false)?;
        let mut current = snapshot
            .pointer("/model_selection/current")
            .cloned()
            .ok_or("selected_projection_unavailable")?;
        current
            .as_object_mut()
            .ok_or("selected_projection_unavailable")?
            .insert(
                "schema".into(),
                Value::String(SELECTED_BINARY_SCHEMA.into()),
            );
        Ok(current)
    }
    fn temporal_selection_snapshot(
        &self,
        now: u64,
        expected_room_binding: Option<&str>,
        boot_epoch: Option<&str>,
        recertify: bool,
    ) -> Result<Value, String> {
        let (selected, assessment, integrity, revision) = {
            let s = self.inner.lock().map_err(|_| "learner state unavailable")?;
            (
                s.selection.temporal.clone(),
                match s.capture.as_ref() {
                    Some(CaptureWorkflow::Temporal(c)) => c.assessment.clone(),
                    _ => None,
                },
                s.selection.error.clone(),
                s.selection.known_revision(),
            )
        };
        let mut result = TemporalSelectionStatusWire {
            schema: SELECTION_STATUS_SCHEMA.into(),
            revision,
            selected: None,
            previous_available: false,
            qualification: assessment
                .as_ref()
                .and_then(|a| a.qualification.as_ref().map(|q| q.receipt.summary.clone())),
            qualification_error: assessment
                .as_ref()
                .filter(|a| a.qualification.is_none())
                .and_then(|a| a.assessment.receipt.summary.failure_reason.clone()),
            integrity_error: integrity,
            current: TemporalCurrentEvidenceWire::None,
        };
        if let Some(selected) = selected {
            let identity = selected.stored.identity();
            if expected_room_binding.is_some_and(|room| room != identity.room_digest) {
                result.qualification = None;
                result.qualification_error = Some("selected_room_context_mismatch".into());
                return Ok(
                    json!({"schema":SELECTION_SCHEMA,"success":true,"model_selection":result}),
                );
            }
            result.revision = identity.selection_revision;
            result.selected = Some(identity.clone());
            result.qualification = Some(selected.stored.qualification().receipt.summary.clone());
            let unavailable = |reason: &str| TemporalCurrentEvidenceWire::Unavailable {
                identity: Some(identity.clone()),
                reason: bounded(reason.into()),
                boot_epoch: boot_epoch.map(str::to_owned),
                receiver_epoch: Some(self.runtime_epoch.clone()),
            };
            let has_integrity_error = result.integrity_error.is_some();
            let check = (|| -> Result<TemporalCurrentEvidenceWire, String> {
                if has_integrity_error {
                    return Err("selected_integrity_unavailable".into());
                }
                if expected_room_binding != Some(identity.room_digest.as_str()) {
                    return Err("selected_room_context_mismatch".into());
                }
                let boot = boot_epoch
                    .filter(|s| !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control))
                    .ok_or("selected_boot_epoch_unavailable")?;
                // Protected current pointer/journal and complete immutable DAG,
                // not hashes returned by a status/JSON import.
                if recertify {
                    let verified = self
                        .store
                        .load_temporal_selection()
                        .and_then(|current| {
                            current.ok_or("temporal_selection_pointer_missing".into())
                        })
                        .and_then(|current| {
                            if current.identity() == identity {
                                Ok(())
                            } else {
                                Err("selected_pointer_changed".into())
                            }
                        });
                    match verified {
                        Ok(()) => {
                            let state =
                                self.inner.lock().map_err(|_| "learner state unavailable")?;
                            if state
                                .selection
                                .temporal
                                .as_ref()
                                .is_some_and(|s| Arc::ptr_eq(s, &selected))
                                && state.selection.error.is_none()
                                && !assessment_blocks(&state, &identity)
                            {
                                selected.invalid.store(false, Ordering::Release);
                            }
                        }
                        Err(error) => {
                            let state =
                                self.inner.lock().map_err(|_| "learner state unavailable")?;
                            if state
                                .selection
                                .temporal
                                .as_ref()
                                .is_some_and(|s| Arc::ptr_eq(s, &selected))
                            {
                                selected.invalid.store(true, Ordering::Release);
                            }
                            return Err(error);
                        }
                    }
                }
                if selected.invalid.load(Ordering::Acquire) {
                    return Err("selected_integrity_unavailable".into());
                }
                if assessment.as_ref().is_some_and(|a| {
                    a.assessment.receipt.summary.model_sha256 == identity.model_sha256
                        && (a.qualification.is_none()
                            || a.qualification
                                .as_ref()
                                .is_some_and(|q| q.receipt_sha256 != identity.qualification_sha256))
                }) {
                    return Err("newer_qualification_failed_or_requires_selection".into());
                }
                let o = {
                    let mut driver = selected
                        .driver
                        .lock()
                        .map_err(|_| "selected_native_input_unavailable")?;
                    let current_now = self.receiver_now_ns();
                    if current_now < now {
                        return Err("selected_receiver_clock_reversed".into());
                    }
                    driver.selected_observation(current_now)?
                };
                let probability = o
                    .prediction
                    .occupied_probability
                    .filter(|p| p.is_finite() && (0.0..=1.0).contains(p))
                    .ok_or("selected_probability_unavailable")?;
                let s = self.inner.lock().map_err(|_| "learner state unavailable")?;
                if s.selection.error.is_some()
                    || selected.invalid.load(Ordering::Acquire)
                    || assessment_blocks(&s, &identity)
                    || !s
                        .selection
                        .temporal
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &selected))
                {
                    return Err("selected_pointer_changed".into());
                }
                if s.capture_context().is_some_and(|context| {
                    context != &selected.stored.envelope().receipt.selected.context
                }) {
                    return Err("selected_source_context_changed".into());
                }
                Ok(TemporalCurrentEvidenceWire::Ready {
                    identity: identity.clone(),
                    presence: o.prediction.decision
                        == crate::room_learner::model::PresenceDecision::Occupied,
                    decision: o.prediction.decision,
                    occupied_probability: probability,
                    observed_at_unix_ms: o.end_ns / 1_000_000,
                    observation_digest: o.digest,
                    boot_epoch: boot.into(),
                    receiver_epoch: self.runtime_epoch.clone(),
                    window_start_unix_ms: o.first_ns / 1_000_000,
                    window_end_unix_ms: o.end_ns / 1_000_000,
                    valid_until_unix_ms: o.expires_ns / 1_000_000,
                    source_observed_at_unix_ms: o.source_latest_ns / 1_000_000,
                })
            })();
            result.current = check.unwrap_or_else(|error| unavailable(&error));
            if selected.invalid.load(Ordering::Acquire) && result.integrity_error.is_none() {
                result.integrity_error = Some("selected_integrity_unavailable".into());
            }
        }
        Ok(json!({"schema":SELECTION_SCHEMA,"success":true,"model_selection":result}))
    }
    pub fn temporal_select_v2(
        self: &Arc<Self>,
        request: TemporalSelectRequestV2,
        now: u64,
    ) -> Result<Value, String> {
        if request.schema != SELECT_REQUEST_SCHEMA {
            return Err("learner_unsupported_schema".into());
        }
        let prior = {
            self.inner
                .lock()
                .map_err(|_| "learner state unavailable")?
                .selection
                .temporal
                .clone()
        };
        if let Some(selected) = prior {
            let current = selected.stored.identity();
            let refs = &request.refs;
            let complete = &refs.evaluation.complete;
            let exact = current.model_kind == complete.model.model_kind
                && current.model_sha256 == complete.model.model_sha256
                && current.verified_fit_record_sha256 == complete.model.verified_fit_record_sha256
                && current.training_proof_sha256 == complete.training_proof_sha256
                && current.complete_record_sha256 == complete.complete_record_sha256
                && selected
                    .stored
                    .held()
                    .body
                    .complete_commitment
                    .commitment_sha256
                    == complete.complete_commitment_sha256
                && current.held_out_evidence_sha256 == refs.evaluation.held_out_evidence_sha256
                && current.evaluation_receipt_sha256 == refs.evaluation.evaluation_receipt_sha256
                && current.qualification_sha256 == refs.qualification_sha256;
            if !exact {
                return Err("selected_target_requires_explicit_deselect".into());
            }
            if selected.stored.capture_generation_id()? != request.capture_generation_id {
                return Err("learner_capture_identity_changed".into());
            }
            {
                let state = self.command_state(&request.expected)?;
                if busy(&state.job) {
                    return Err("learner_job_busy".into());
                }
                if !state
                    .selection
                    .temporal
                    .as_ref()
                    .is_some_and(|s| Arc::ptr_eq(s, &selected))
                {
                    return Err("selected_pointer_changed".into());
                }
                if state.selection.error.is_some()
                    || selected.invalid.load(Ordering::Acquire)
                    || assessment_blocks(&state, &current)
                {
                    return Err("selection_reconciliation_required".into());
                }
            }
            // Exact already-admitted choice, including a lost ACK or expired
            // raw lease: no new adoption, IO, timestamp, revision or driver.
            return self.temporal_selection_snapshot(self.receiver_now_ns(), None, None, false);
        }
        {
            let s = self.inner.lock().map_err(|_| "learner state unavailable")?;
            let Some(CaptureWorkflow::Temporal(c)) = &s.capture else {
                return Err(PRIVATE_KIND.into());
            };
            if c.core
                .as_ref()
                .is_none_or(|core| core.public_generation_id() != request.capture_generation_id)
            {
                return Err("learner_capture_identity_changed".into());
            }
            let a = c
                .assessment
                .as_ref()
                .ok_or("temporal_qualification_required")?;
            let q = a
                .qualification
                .as_ref()
                .ok_or("temporal_candidate_not_qualified")?;
            let e = &request.refs.evaluation;
            if q.receipt_sha256 != request.refs.qualification_sha256
                || q.receipt.summary.evaluation_receipt_sha256 != e.evaluation_receipt_sha256
                || q.receipt.summary.held_out_evidence_sha256 != e.held_out_evidence_sha256
                || c.core
                    .as_ref()
                    .and_then(|core| {
                        core.public_complete_refs(&q.receipt.summary.training_proof_sha256)
                    })
                    .as_ref()
                    != Some(&e.complete)
            {
                return Err("learner_qualification_identity_changed".into());
            }
        }
        self.run_temporal_selection(&request.expected, now)?;
        // The actual operation already read back the durable DAG. Its drain
        // signal has fired; ACK enrichment must not start another store read
        // that could outlive an owner-erasure barrier. Poll/Save uses REST's
        // separately deep-validated read path.
        self.temporal_selection_snapshot(now, None, None, false)
    }
}

#[derive(Clone)]
pub(in crate::room_learner::lifecycle) struct TemporalWithdrawalJob {
    lifetime: Arc<super::fit_control::FitLifetime>,
}
impl TemporalWithdrawalJob {
    pub(in crate::room_learner::lifecycle) fn cancel(&self) {
        self.lifetime.cancel();
    }
    pub(in crate::room_learner::lifecycle) fn drain(&self) {
        self.lifetime.drain();
    }
}
struct WithdrawalCompletion {
    runtime: std::sync::Weak<RoomLearningRuntime>,
    job: TemporalWithdrawalJob,
}
impl Drop for WithdrawalCompletion {
    fn drop(&mut self) {
        let runtime = self.runtime.upgrade();
        if let Some(runtime) = &runtime {
            let mut state = runtime.inner.lock().unwrap_or_else(|e| e.into_inner());
            if Arc::ptr_eq(&state.job.cancel, &self.job.lifetime.cancel) && busy(&state.job) {
                state.job.phase = if self.job.lifetime.cancel.load(Ordering::Acquire) {
                    JobPhase::Cancelled
                } else {
                    JobPhase::Failed
                };
                state.job.error = Some("selection_withdrawal_unavailable".into());
                state.selection.error=Some("Selection withdrawal outcome unavailable; restart and verify the saved choice.".into());
            }
        }
        *self
            .job
            .lifetime
            .completed
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = true;
        self.job.lifetime.completion.notify_all();
        drop(runtime);
    }
}
impl RoomLearningRuntime {
    pub fn temporal_deselect_v2(
        self: &Arc<Self>,
        request: TemporalDeselectRequestV2,
        now: u64,
    ) -> Result<Value, String> {
        use std::sync::{atomic::AtomicU64, Condvar};
        use std::time::Instant;
        if request.schema != DESELECT_REQUEST_SCHEMA {
            return Err("learner_unsupported_schema".into());
        }
        let retired_job;
        let retired_withdrawal;
        let (job, generation) = {
            let mut state = self.command_state(&request.expected)?;
            if busy(&state.job) {
                return Err("learner_job_busy".into());
            }
            let current = state
                .selection
                .temporal
                .as_ref()
                .ok_or("temporal_selection_missing")?;
            let identity = current.stored.identity();
            if identity.selection_sha256 != request.selection_sha256
                || identity.selection_revision != request.selection_revision
            {
                return Err("selected_pointer_changed".into());
            }
            let cancel = Arc::new(AtomicBool::new(false));
            let lifetime = Arc::new(super::fit_control::FitLifetime {
                cancel: cancel.clone(),
                publication: Mutex::new(()),
                completed: Mutex::new(false),
                completion: Condvar::new(),
                anchor: now,
                origin: Instant::now(),
                observed: AtomicU64::new(now),
                expires_at_ms: u64::MAX,
                #[cfg(test)]
                hook: self
                    .temporal_fit_hook
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
                #[cfg(test)]
                io_hook: self
                    .temporal_evidence_io_hook
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            });
            let job = TemporalWithdrawalJob { lifetime };
            state.selection.error=Some("Selection withdrawal pending; current evidence is unavailable until durable reconciliation.".into());
            retired_withdrawal = state.selection.withdrawal.replace(job.clone());
            retired_job = std::mem::replace(
                &mut state.job,
                Job {
                    phase: JobPhase::Running,
                    cancel,
                    report: None,
                    error: None,
                },
            );
            (job, state.generation)
        };
        drop(retired_job);
        drop(retired_withdrawal);
        let completion = WithdrawalCompletion {
            runtime: Arc::downgrade(self),
            job: job.clone(),
        };
        let result = (|| -> Result<crate::room_learner::selection::SelectionEnvelope, String> {
            let _publication = job
                .lifetime
                .publication
                .lock()
                .map_err(|_| "selection_publication_unavailable")?;
            job.lifetime.allowed()?;
            let envelope = self.store.withdraw_temporal_selection(
                &request.selection_sha256,
                request.selection_revision,
                now,
                &mut || {
                    job.lifetime.allowed()?;
                    Ok(())
                },
                &mut || {
                    job.lifetime.io("deselect_before_pointer_install")?;
                    job.lifetime.allowed()?;
                    Ok(())
                },
                &mut || job.lifetime.io("deselect_after_pointer_rename"),
            )?;
            job.lifetime.allowed()?;
            Ok(envelope)
        })();
        let mut retired_selected = None;
        {
            let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
            if state.generation != generation
                || !Arc::ptr_eq(&state.job.cancel, &job.lifetime.cancel)
            {
                return Err("selection_withdrawal_not_adopted".into());
            }
            match &result {
                Ok(envelope) => {
                    retired_selected = state.selection.temporal.take();
                    state
                        .selection
                        .install_negative_temporal_tombstone(envelope.clone());
                    state.selection.error = None;
                    state.job.phase = JobPhase::Ready;
                    state.job.error = None;
                }
                Err(error) => {
                    state.selection.error=Some("Selection withdrawal outcome unavailable; reread the saved choice before continuing.".into());
                    state.job.phase = if job.lifetime.cancel.load(Ordering::Acquire) {
                        JobPhase::Cancelled
                    } else {
                        JobPhase::Failed
                    };
                    state.job.error = Some(bounded(error.clone()));
                }
            }
        }
        if let Some(selected) = retired_selected {
            selected
                .driver
                .lock()
                .map_err(|_| "selected_driver_unavailable")?
                .deactivate();
            drop(selected);
        }
        drop(completion);
        result?;
        self.temporal_selection_v2(self.receiver_now_ns(), None, None)
    }
}

fn assessment_blocks(state: &Inner, identity: &TemporalSelectedIdentityWire) -> bool {
    match state.capture.as_ref() {
        Some(CaptureWorkflow::Temporal(c)) => c.assessment.as_ref().is_some_and(|a| {
            a.assessment.receipt.summary.model_sha256 == identity.model_sha256
                && (a.qualification.is_none()
                    || a.qualification
                        .as_ref()
                        .is_some_and(|q| q.receipt_sha256 != identity.qualification_sha256))
        }),
        _ => false,
    }
}
