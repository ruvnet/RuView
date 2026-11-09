//! C4C uses the SAME Runtime Job/executor/lifetime. No JSON authority issuer.
use super::fit_control::FitLifetime;
use super::*;
use crate::room_learner::artifact::StoredTemporalEvaluation;
use crate::room_learner::temporal_evaluation::TemporalTrainProofData;
use crate::room_learner::temporal_model_input::managed::execution::full_run::{
    PendingTemporalEvidenceSource, TemporalEvaluationBinding,
};
use std::sync::{atomic::AtomicU64, Condvar, Weak};
use std::time::Instant;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EvidencePurpose {
    TrainProof,
    HeldOutEvaluation,
    Qualification,
    Selection,
    RestoreComplete,
}
#[derive(Clone, PartialEq)]
struct EvidenceIdentity {
    epoch: String,
    generation: u64,
    attempt: u64,
    purpose: EvidencePurpose,
    binding: TemporalEvaluationBinding,
}
#[derive(Clone)]
pub(super) struct TemporalEvidenceJob {
    identity: EvidenceIdentity,
    lifetime: Arc<FitLifetime>,
}
impl TemporalEvidenceJob {
    pub(super) fn uses_cancel(&self, cancel: &Arc<AtomicBool>) -> bool {
        Arc::ptr_eq(cancel, &self.lifetime.cancel)
    }
    pub(super) fn observe_now(&self, now: u64) {
        self.lifetime.observed.fetch_max(now, Ordering::AcqRel);
    }
    pub(super) fn cancel(&self) {
        self.lifetime.cancel();
    }
    pub(super) fn drain(&self) {
        self.lifetime.drain();
    }
    pub(super) fn done(&self) -> bool {
        self.lifetime.done()
    }
    pub(super) fn operation_name(&self) -> &'static str {
        match self.identity.purpose {
            EvidencePurpose::TrainProof => "train_proof",
            EvidencePurpose::HeldOutEvaluation => "evaluate",
            EvidencePurpose::Qualification => "qualify",
            EvidencePurpose::Selection => "select",
            EvidencePurpose::RestoreComplete => "restore",
        }
    }
}
enum EvidenceOutcome {
    Train(TemporalTrainProofData),
    Evaluation(StoredTemporalEvaluation),
    Assessment(crate::room_learner::artifact::StoredTemporalAssessment),
    Selection(Arc<super::selected_control::TemporalSelectedRuntime>),
    Restored(TemporalTrainProofData, Option<StoredTemporalEvaluation>),
}
struct EvidenceCompletion {
    runtime: Weak<RoomLearningRuntime>,
    owner: Option<Arc<RoomLearningRuntime>>,
    identity: EvidenceIdentity,
    lifetime: Arc<FitLifetime>,
}
impl Drop for EvidenceCompletion {
    fn drop(&mut self) {
        let fallback = if self.owner.is_none() {
            self.runtime.upgrade()
        } else {
            None
        };
        if let Some(runtime) = self.owner.as_ref().or(fallback.as_ref()) {
            let mut state = runtime.inner.lock().unwrap_or_else(|e| e.into_inner());
            let current = Arc::ptr_eq(&state.job.cancel, &self.lifetime.cancel)
                && matches!(state.capture.as_ref(),Some(CaptureWorkflow::Temporal(c))if c.evidence_job.as_ref().is_some_and(|j|j.identity==self.identity&&Arc::ptr_eq(&j.lifetime,&self.lifetime)));
            if current && busy(&state.job) {
                state.job.phase = if self.lifetime.cancel.load(Ordering::Acquire) {
                    JobPhase::Cancelled
                } else {
                    JobPhase::Failed
                };
                state.job.error = Some("temporal_evidence_worker_unavailable".into());
            }
        }
        // Every result/raw snapshot and IO operation has completed before this
        // signal. A last upgraded Runtime Arc may now safely enter its Drop.
        *self
            .lifetime
            .completed
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = true;
        self.lifetime.completion.notify_all();
        drop(fallback);
    }
}
impl RoomLearningRuntime {
    pub(super) fn start_temporal_evidence(
        self: &Arc<Self>,
        expected: &MutationGuard,
        now: u64,
        purpose: EvidencePurpose,
    ) -> Result<u64, String> {
        self.start_temporal_evidence_with_references(expected, now, purpose, None)
    }
    pub(super) fn start_temporal_evidence_with_references(
        self:&Arc<Self>, expected:&MutationGuard, now:u64, purpose:EvidencePurpose,
        references:Option<crate::companion_references::ReferenceSnapshot>,
    )->Result<u64,String>{
        let executor = tokio::runtime::Handle::try_current()
            .map_err(|_| "temporal_evaluation_executor_unavailable")?;
        let recovery = self
            .recovery_store
            .clone()
            .ok_or("temporal_recovery_unavailable")?;
        let store = self.store.clone();
        let (input, identity, lifetime) = self.reserve_temporal_evidence(expected, now, purpose)?;
        let attempt = identity.attempt;
        let completion = EvidenceCompletion {
            runtime: Arc::downgrade(self),
            owner: None,
            identity: identity.clone(),
            lifetime: lifetime.clone(),
        };
        executor.spawn_blocking(move || {
            let mut completion = completion;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                Self::compute_temporal_evidence(&store, &recovery, input, &identity, &lifetime, references)
            }))
            .unwrap_or_else(|_| Err("temporal_evaluation_worker_failed_safely".into()));
            completion.owner = completion.runtime.upgrade();
            if let Some(runtime) = &completion.owner {
                runtime.admit_temporal_evidence(&identity, &lifetime, result);
            } else {
                drop(result);
            }
        });
        Ok(attempt)
    }
    fn reserve_temporal_evidence(
        &self,
        expected: &MutationGuard,
        now: u64,
        purpose: EvidencePurpose,
    ) -> Result<
        (
            PendingTemporalEvidenceSource,
            EvidenceIdentity,
            Arc<FitLifetime>,
        ),
        String,
    > {
        let mut retired_job = None;
        let mut retired_evidence = None;
        let (input, identity, lifetime) = {
            let mut state = self.command_state(expected)?;
            if busy(&state.job) {
                return Err("learner_job_busy".into());
            }
            let generation = state.generation;
            let attempt = state.workflow_revision;
            let Some(CaptureWorkflow::Temporal(c)) = state.capture.as_mut() else {
                return Err(PRIVATE_KIND.into());
            };
            if c.owner_generation != generation
                || c.failure.is_some()
                || c.evidence_job.as_ref().is_some_and(|j| !j.done())
                || c.fit_job.as_ref().is_some_and(|j| !j.done())
            {
                return Err("temporal_evaluation_owner_or_job_busy".into());
            }
            c.check_time(now)?;
            let input = c
                .core
                .as_ref()
                .ok_or("temporal_initial_barrier_unconfirmed")?
                .prepare_evaluation_source(now, purpose != EvidencePurpose::TrainProof)?;
            if !input
                .binding()
                .owner_matches(&self.runtime_epoch, generation)
            {
                return Err("temporal_evaluation_owner_changed".into());
            }
            let identity = EvidenceIdentity {
                epoch: self.runtime_epoch.clone(),
                generation,
                attempt,
                purpose,
                binding: input.binding().clone(),
            };
            let cancel = Arc::new(AtomicBool::new(false));
            let lifetime = Arc::new(FitLifetime {
                cancel: cancel.clone(),
                publication: Mutex::new(()),
                completed: Mutex::new(false),
                completion: Condvar::new(),
                anchor: now,
                origin: Instant::now(),
                observed: AtomicU64::new(now),
                expires_at_ms: input.expires_at_ms(),
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
            retired_evidence = c.evidence_job.replace(TemporalEvidenceJob {
                identity: identity.clone(),
                lifetime: lifetime.clone(),
            });
            retired_job = Some(std::mem::replace(
                &mut state.job,
                Job {
                    phase: JobPhase::Running,
                    cancel,
                    report: None,
                    error: None,
                },
            ));
            if purpose == EvidencePurpose::Selection {
                state.selection.error=Some("Selection transition pending; current evidence is unavailable until durable reconciliation.".into());
            }
            (input, identity, lifetime)
        };
        drop(retired_evidence);
        drop(retired_job);
        Ok((input, identity, lifetime))
    }
    pub(super) fn run_temporal_selection(
        self: &Arc<Self>,
        expected: &MutationGuard,
        now: u64,
    ) -> Result<(), String> {
        let recovery = self
            .recovery_store
            .clone()
            .ok_or("temporal_recovery_unavailable")?;
        let store = self.store.clone();
        let (input, identity, lifetime) =
            self.reserve_temporal_evidence(expected, now, EvidencePurpose::Selection)?;
        let completion = EvidenceCompletion {
            runtime: Arc::downgrade(self),
            owner: Some(self.clone()),
            identity: identity.clone(),
            lifetime: lifetime.clone(),
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::compute_temporal_evidence(&store, &recovery, input, &identity, &lifetime, None)
        }))
        .unwrap_or_else(|_| Err("temporal_selection_worker_failed_safely".into()));
        let failure = result.as_ref().err().cloned();
        self.admit_temporal_evidence(&identity, &lifetime, result);
        // Finish/drain tracks the actual store operation, not an HTTP ACK.
        drop(completion);
        if let Some(error) = failure {
            return Err(error);
        }
        let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        if state.job.phase != JobPhase::Ready || !Arc::ptr_eq(&state.job.cancel, &lifetime.cancel) {
            return Err("temporal_selection_not_adopted".into());
        }
        Ok(())
    }
    fn compute_temporal_evidence(
        store: &crate::room_learner::artifact::ArtifactStore,
        recovery: &crate::room_learner::recovery_store::RecoveryStore,
        input: PendingTemporalEvidenceSource,
        identity: &EvidenceIdentity,
        lifetime: &FitLifetime,
        references: Option<crate::companion_references::ReferenceSnapshot>,
    ) -> Result<EvidenceOutcome, String> {
        lifetime.at("evidence_before_validate");
        let now = lifetime.allowed()?;
        let outcome: Result<EvidenceOutcome, String> = match identity.purpose {
            EvidencePurpose::TrainProof => {
                let source = input.recertify_train(recovery, now, &lifetime.cancel)?;
                let _publication = lifetime
                    .publication
                    .lock()
                    .map_err(|_| "temporal_evaluation_publication_unavailable")?;
                lifetime.at("evidence_publication_entered");
                let proof = store.persist_current_temporal_train_proof(
                    &source,
                    &lifetime.cancel,
                    &mut || lifetime.allowed(),
                    &mut |stage| {
                        lifetime.allowed()?;
                        lifetime.io(stage)
                    },
                )?;
                lifetime.allowed()?;
                Ok(EvidenceOutcome::Train(proof))
            }
            EvidencePurpose::RestoreComplete => {
                let source = input.recertify_evaluation(recovery, now, &lifetime.cancel)?;
                let _publication = lifetime
                    .publication
                    .lock()
                    .map_err(|_| "temporal_restore_publication_unavailable")?;
                let (proof, evaluation) = store.reconcile_temporal_snapshot(
                    source,
                    &lifetime.cancel,
                    &mut || lifetime.allowed(),
                    &mut |stage| lifetime.io(stage),
                )?;
                Ok(EvidenceOutcome::Restored(proof, evaluation))
            }
            EvidencePurpose::Selection => {
                let source = input.recertify_evaluation(recovery, now, &lifetime.cancel)?;
                let _publication = lifetime
                    .publication
                    .lock()
                    .map_err(|_| "temporal_selection_publication_unavailable")?;
                let revision = store.selection_revision_any()?;
                let assessment = store.qualify_temporal_snapshot(
                    &source,
                    &lifetime.cancel,
                    &mut || lifetime.allowed(),
                    &mut |stage| lifetime.io(stage),
                )?;
                let stored = store.save_current_temporal_selection(
                    &source,
                    &assessment,
                    revision,
                    lifetime.allowed()?,
                    &lifetime.cancel,
                    &mut || lifetime.allowed(),
                    &mut || {
                        lifetime.io("selection_before_pointer_install")?;
                        identity
                            .binding
                            .recertify_current(recovery, lifetime.allowed()?)?;
                        lifetime.allowed()?;
                        Ok(())
                    },
                    &mut || lifetime.io("selection_after_pointer_rename"),
                )?;
                Ok(EvidenceOutcome::Selection(Arc::new(
                    super::selected_control::TemporalSelectedRuntime::new(stored)?,
                )))
            }
            EvidencePurpose::Qualification => {
                let source = input.recertify_evaluation(recovery, now, &lifetime.cancel)?;
                let _publication = lifetime
                    .publication
                    .lock()
                    .map_err(|_| "temporal_evaluation_publication_unavailable")?;
                lifetime.at("evidence_publication_entered");
                let receipt = store.qualify_temporal_snapshot(
                    &source,
                    &lifetime.cancel,
                    &mut || lifetime.allowed(),
                    &mut |stage| {
                        lifetime.allowed()?;
                        lifetime.io(stage)
                    },
                )?;
                lifetime.allowed()?;
                Ok(EvidenceOutcome::Assessment(receipt))
            }
            EvidencePurpose::HeldOutEvaluation => {
                let source = input.recertify_evaluation(recovery, now, &lifetime.cancel)?;
                let _publication = lifetime
                    .publication
                    .lock()
                    .map_err(|_| "temporal_evaluation_publication_unavailable")?;
                lifetime.at("evidence_publication_entered");
                let receipt = match references {
                    Some(snapshot) => store.evaluate_temporal_snapshot_with_comparison(
                        source, snapshot, &lifetime.cancel, &mut || lifetime.allowed(),
                        &mut |stage| { lifetime.allowed()?; lifetime.io(stage) },
                    )?,
                    None => store.evaluate_temporal_snapshot(
                        source, &lifetime.cancel, &mut || lifetime.allowed(),
                        &mut |stage| { lifetime.allowed()?; lifetime.io(stage) },
                    )?,
                };
                lifetime.allowed()?;
                Ok(EvidenceOutcome::Evaluation(receipt))
            }
        };
        let outcome = outcome?;
        identity
            .binding
            .recertify_current(recovery, lifetime.allowed()?)?;
        let model = store.load_exact_temporal_candidate(
            identity.binding.context(),
            identity.binding.model_sha256(),
        )?;
        if model.artifact_kind() != crate::room_learner::temporal_model_input::managed::SCHEMA_V3 {
            return Err("temporal_evaluation_model_kind_changed".into());
        }
        lifetime.allowed()?;
        Ok(outcome)
    }
    fn admit_temporal_evidence(
        &self,
        identity: &EvidenceIdentity,
        lifetime: &Arc<FitLifetime>,
        outcome: Result<EvidenceOutcome, String>,
    ) {
        // Outcome and replaced metadata are destroyed after inner is released.
        let mut retired_proof = None;
        let mut retired_evaluation = None;
        let mut retired_assessment = None;
        let mut retired_selection = None;
        let mut state = match self.inner.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let same = identity.epoch == self.runtime_epoch
            && identity.generation == state.generation
            && Arc::ptr_eq(&state.job.cancel, &lifetime.cancel)
            && matches!(state.capture.as_ref(),Some(CaptureWorkflow::Temporal(c))if c.owner_generation==identity.generation&&c.failure.is_none()&&c.evidence_job.as_ref().is_some_and(|j|j.identity==*identity&&Arc::ptr_eq(&j.lifetime,lifetime))&&c.core.as_ref().is_some_and(|core|core.evaluation_binding_matches(&identity.binding)));
        if !same {
            return;
        }
        if lifetime.allowed().is_err() {
            state.job.phase = JobPhase::Cancelled;
            state.job.error = Some("temporal_evaluation_cancelled_or_expired".into());
            return;
        }
        state.workflow_revision = state.workflow_revision.saturating_add(1);
        match outcome {
            Ok(EvidenceOutcome::Train(proof)) => {
                if let Some(CaptureWorkflow::Temporal(c)) = state.capture.as_mut() {
                    retired_proof = c.training_proof.replace(proof);
                }
                state.job.phase = JobPhase::Ready;
                state.job.error = None;
            }
            Ok(EvidenceOutcome::Evaluation(receipt)) => {
                if receipt.binding() != &identity.binding {
                    state.job.phase = JobPhase::Failed;
                    state.job.error = Some("temporal_evaluation_result_identity_changed".into());
                    return;
                }
                if let Some(CaptureWorkflow::Temporal(c)) = state.capture.as_mut() {
                    retired_proof = c.training_proof.replace(receipt.train().clone());
                    retired_evaluation = c.evaluation.replace(Arc::new(receipt));
                }
                state.job.phase = JobPhase::Ready;
                state.job.error = None;
            }
            Ok(EvidenceOutcome::Assessment(receipt)) => {
                if receipt.binding() != &identity.binding {
                    state.job.phase = JobPhase::Failed;
                    state.job.error = Some("temporal_assessment_result_identity_changed".into());
                    return;
                }
                if let Some(CaptureWorkflow::Temporal(c)) = state.capture.as_mut() {
                    retired_assessment = c.assessment.replace(Arc::new(receipt));
                }
                state.job.phase = JobPhase::Ready;
                state.job.error = None;
            }
            Ok(EvidenceOutcome::Restored(proof, evaluation)) => {
                if let Some(CaptureWorkflow::Temporal(c)) = state.capture.as_mut() {
                    retired_proof = c.training_proof.replace(proof);
                    if let Some(evaluation) = evaluation {
                        retired_evaluation = c.evaluation.replace(Arc::new(evaluation));
                    }
                }
                state.job.phase = JobPhase::Ready;
                state.job.error = None;
            }
            Ok(EvidenceOutcome::Selection(selected)) => {
                let floors = match state.capture.as_ref() {
                    Some(CaptureWorkflow::Temporal(c)) => {
                        c.core.as_ref().map(|core| core.current_source_floors())
                    }
                    _ => None,
                };
                if let Some((floors, receiver)) = floors {
                    let seeded = selected
                        .driver
                        .lock()
                        .map_err(|_| "selected driver unavailable".to_string())
                        .and_then(|mut d| d.advance_selected_floors(&floors, receiver));
                    if let Err(error) = seeded {
                        state.job.phase = JobPhase::Failed;
                        state.job.error = Some(error);
                        return;
                    }
                }
                retired_selection = state.selection.temporal.replace(selected);
                state.selection.error = None;
                state.job.phase = JobPhase::Ready;
                state.job.error = None;
            }
            Err(error) => {
                state.job.phase = if lifetime.cancel.load(Ordering::Acquire) {
                    JobPhase::Cancelled
                } else {
                    JobPhase::Failed
                };
                state.job.error = Some(bounded(error));
            }
        }
    }
}
impl RoomLearningRuntime {
    pub(super) fn cancel_temporal_evidence(
        &self,
        expected: &MutationGuard,
    ) -> Result<JobPhase, String> {
        let job = {
            let mut state = self.command_state(expected)?;
            let Some(CaptureWorkflow::Temporal(c)) = state.capture.as_ref() else {
                return Err(PRIVATE_KIND.into());
            };
            let job = c
                .evidence_job
                .clone()
                .ok_or("temporal_evaluation_job_missing")?;
            if state.job.phase == JobPhase::Ready
                && Arc::ptr_eq(&state.job.cancel, &job.lifetime.cancel)
                && c.failure.is_none()
            {
                return Ok(JobPhase::Ready);
            }
            job.cancel();
            if busy(&state.job) {
                state.job.phase = JobPhase::Cancelling;
            }
            job
        };
        job.drain();
        let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        Ok(state.job.phase)
    }
}
