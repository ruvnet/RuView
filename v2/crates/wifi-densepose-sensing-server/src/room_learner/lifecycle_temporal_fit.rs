//! Private fit-only adapter into the EXISTING Job, fitter and ArtifactStore.
//! No public route, fitted recovery, HeldOut, proof, selection or live authority.
use super::*;
use crate::room_learner::{
    artifact::{TemporalCandidateReceipt, TemporalFitDiagnostics, TemporalFitSaveResult},
    recovery_store::RecoveryStore,
    temporal_model_input::{
        managed::execution::{PendingTemporalFitInput, TemporalFitCaptureIdentity,VerifiedTemporalFitSource},
        TemporalModel,
    },
};
use std::sync::{atomic::AtomicU64, Condvar, Weak};
use std::time::Instant;

#[derive(Clone, PartialEq, Eq)]
struct TemporalFitIdentity {
    runtime_epoch: String,
    generation: u64,
    // The consumed start revision identifies THIS job. Later denied commands
    // may consume global revisions without replacing this active operation.
    attempt: u64,
    capture: TemporalFitCaptureIdentity,
}
pub(super) struct TemporalDiagnosticFit {
    model: Arc<TemporalModel>,
    saved: TemporalFitSaveResult,
    accepted: Option<AcceptedTemporalFit>,
}
/// Minted ONLY in the actual current C3 result-admission branch. Not serde,
/// Clone, a candidate-import constructor, or a report/claims permission.
pub(crate) struct AcceptedTemporalFit {
    runtime_epoch:String,generation:u64,fit_attempt:u64,
    model:Arc<TemporalModel>,source:VerifiedTemporalFitSource,
}
impl AcceptedTemporalFit {
    pub(crate) fn consume_for_owner(self,epoch:&str,generation:u64,binding:&TemporalFitCaptureIdentity)->Result<(Arc<TemporalModel>,VerifiedTemporalFitSource),String>{
        if self.runtime_epoch!=epoch||self.generation!=generation||self.fit_attempt==0||self.source.binding()!=binding{
            return Err("temporal_accepted_fit_owner_changed".into());
        }
        Ok((self.model,self.source))
    }
}
/// One lifetime barrier for the SAME existing Job/executor, not a new queue or
/// trainer. Cancellation is atomic; waiting/publication never holds inner.
pub(super) struct FitLifetime {
    pub(super) cancel: Arc<AtomicBool>,
    pub(super) publication: Mutex<()>,
    pub(super) completed: Mutex<bool>,
    pub(super) completion: Condvar,
    pub(super) anchor: u64,
    pub(super) origin: Instant,
    pub(super) observed: AtomicU64,
    pub(super) expires_at_ms: u64,
    #[cfg(test)]
    pub(super) hook: Option<Arc<dyn Fn(&'static str) + Send + Sync>>,
    #[cfg(test)]
    pub(super) io_hook: Option<Arc<dyn Fn(&'static str)->Result<(),String>+Send+Sync>>,
}
impl FitLifetime {
    pub(super) fn io(&self,stage:&'static str)->Result<(),String>{
        #[cfg(test)] if let Some(hook)=&self.io_hook{return hook(stage);}
        #[cfg(not(test))] let _=stage;
        Ok(())
    }
    pub(super) fn now(&self) -> Result<u64, String> {
        let elapsed: u64 = self
            .origin
            .elapsed()
            .as_nanos()
            .try_into()
            .map_err(|_| "temporal_fit_clock_overflow")?;
        Ok(self.observed.load(Ordering::Acquire).max(
            self.anchor
                .checked_add(elapsed)
                .ok_or("temporal_fit_clock_overflow")?,
        ))
    }
    pub(super) fn allowed(&self) -> Result<u64, String> {
        if self.cancel.load(Ordering::Acquire) {
            return Err("temporal_fit_cancelled".into());
        }
        let now = self.now()?;
        if now / 1_000_000 >= self.expires_at_ms {
            return Err("temporal_fit_original_deadline_expired".into());
        }
        Ok(now)
    }
    pub(super) fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
    pub(super) fn done(&self) -> bool {
        *self.completed.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub(super) fn drain(&self) {
        self.cancel();
        let mut complete = self.completed.lock().unwrap_or_else(|e| e.into_inner());
        while !*complete {
            complete = self
                .completion
                .wait(complete)
                .unwrap_or_else(|e| e.into_inner());
        }
    }
    pub(super) fn at(&self, stage: &'static str) {
        #[cfg(test)]
        if let Some(hook) = &self.hook {
            hook(stage);
        }
        #[cfg(not(test))]
        let _ = stage;
    }
}
#[derive(Clone)]
pub(super) struct TemporalFitJob {
    identity: TemporalFitIdentity,
    lifetime: Arc<FitLifetime>,
}
impl TemporalFitJob {
    pub(super) fn uncancelled_completed(&self)->bool{self.done()&&!self.lifetime.cancel.load(Ordering::Acquire)}
    pub(super) fn attempt_matches(&self,attempt:u64,cancel:&Arc<AtomicBool>)->bool{self.identity.attempt==attempt&&self.uses_cancel(cancel)&&!self.lifetime.cancel.load(Ordering::Acquire)}
    pub(super) fn uses_cancel(&self,cancel:&Arc<AtomicBool>)->bool{Arc::ptr_eq(cancel,&self.lifetime.cancel)}
    pub(super) fn observe_now(&self, now: u64) {
        self.lifetime.observed.fetch_max(now, Ordering::AcqRel);
    }
    pub(super) fn cancel(&self) {
        self.lifetime.cancel();
    }
    pub(super) fn drain(&self) {
        self.lifetime.drain();
    }
    pub(super) fn done(&self) -> bool { self.lifetime.done() }
}
/// Created before scheduling. Dropping an unstarted/discarded closure also
/// releases waiters. Worker outcome/raw ownership is destroyed before this.
struct FitCompletion {
    runtime: Weak<RoomLearningRuntime>,
    // An upgraded owner is held only AFTER all fit/storage work. The done
    // signal precedes its release, or the last Arc could self-drain here.
    owner: Option<Arc<RoomLearningRuntime>>,
    identity: TemporalFitIdentity,
    lifetime: Arc<FitLifetime>,
}
impl Drop for FitCompletion {
    fn drop(&mut self) {
        let fallback = if self.owner.is_none() {
            self.runtime.upgrade()
        } else {
            None
        };
        if let Some(runtime) = self.owner.as_ref().or(fallback.as_ref()) {
            let mut state = runtime.inner.lock().unwrap_or_else(|e| e.into_inner());
            let current = Arc::ptr_eq(&state.job.cancel, &self.lifetime.cancel)
                && matches!(state.capture.as_ref(),Some(CaptureWorkflow::Temporal(c))
                if c.fit_job.as_ref().is_some_and(|j|j.identity==self.identity&&Arc::ptr_eq(&j.lifetime,&self.lifetime)));
            if current && busy(&state.job) {
                state.job.phase = if self.lifetime.cancel.load(Ordering::Acquire) {
                    JobPhase::Cancelled
                } else {
                    JobPhase::Failed
                };
                state.job.error = Some(
                    if state.job.phase == JobPhase::Cancelled {
                        "temporal_fit_cancelled"
                    } else {
                        "temporal_fit_worker_unavailable"
                    }
                    .into(),
                );
            }
        }
        *self
            .lifetime
            .completed
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = true;
        self.lifetime.completion.notify_all();
        // No fit/store operation follows this signal. A final Runtime Arc may
        // now run Drop safely because its exact fit lifetime is complete.
        drop(fallback);
    }
}
impl RuntimeTemporalCapture {
    pub(super) fn cancel_fit(&self) {
        if let Some(job) = &self.fit_job { job.cancel(); }
        if let Some(job) = &self.evidence_job { job.cancel(); }
    }
    pub(super) fn take_current_accepted_fit(&mut self,epoch:&str,generation:u64,now:u64)->Result<AcceptedTemporalFit,String>{
        if self.owner_generation!=generation||self.failure.is_some(){return Err("temporal_accepted_fit_owner_changed".into());}
        let job=self.fit_job.as_ref().ok_or("temporal_actual_current_fit_required")?;
        if job.identity.runtime_epoch!=epoch||job.lifetime.cancel.load(Ordering::Acquire)||!job.done(){return Err("temporal_actual_current_fit_required".into());}
        let current=self.core.as_ref().ok_or("temporal_initial_barrier_unconfirmed")?.fit_capture_identity(now)?;
        let accepted=self.diagnostic_fit.as_ref().and_then(|d|d.accepted.as_ref()).ok_or("temporal_full_run_actual_accepted_fit_required")?;
        if accepted.source.binding()!=&current{return Err("temporal_accepted_fit_capture_changed".into());}
        Ok(self.diagnostic_fit.as_mut().unwrap().accepted.take().unwrap())
    }
}
impl RoomLearningRuntime {
    /// Private cancellation of the SAME Job, preserving confirmed Train data.
    /// Unlike an AtomicBool-only response, completion means publication drained.
    pub(super) fn cancel_temporal_fit(&self, expected: &MutationGuard) -> Result<JobPhase, String> {
        let job = {
            let mut state = self.command_state(expected)?;
            let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_ref() else {
                return Err(PRIVATE_KIND.into());
            };
            let job = capture.fit_job.clone().ok_or("temporal_fit_job_missing")?;
            // Ready is published only after actual fit/store validation. A
            // late compute-only cancel cannot revoke that accepted result or
            // its separately owned Frozen transaction, even before the final
            // completion notification. Capture/owner cancellation still uses
            // cancel_fit + drain and revokes the entire owner operation.
            if state.job.phase == JobPhase::Ready
                && job.identity.runtime_epoch == self.runtime_epoch
                && job.identity.generation == state.generation
                && Arc::ptr_eq(&state.job.cancel, &job.lifetime.cancel)
                && capture.owner_generation == state.generation
                && capture.failure.is_none()
                && capture.core.as_ref().is_some_and(|c| c.is_full_run())
                && capture.diagnostic_fit.is_some()
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
    /// Private entry only. The existing public start_training remains Legacy.
    pub(super) fn start_temporal_fit(
        self: &Arc<Self>,
        expected: &MutationGuard,
        now: u64,
    ) -> Result<u64, String> {
        let executor = tokio::runtime::Handle::try_current()
            .map_err(|_| "temporal_fit_executor_unavailable")?;
        let store = self.store.clone();
        let recovery = self
            .recovery_store
            .clone()
            .ok_or("temporal_recovery_unavailable")?;
        let retired_job;
        let retired_fit;
        let (input, identity, lifetime) = {
            let mut state = self.command_state(expected)?;
            if busy(&state.job) || state.job.phase == JobPhase::Ready {
                return Err("temporal_fit_job_busy_or_frozen".into());
            }
            let generation = state.generation;
            let attempt = state.workflow_revision;
            let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
                return Err(PRIVATE_KIND.into());
            };
            if capture.owner_generation != generation {
                capture.invalidate_owner();
                return Err("temporal_owner_generation_changed".into());
            }
            if capture.lead_in_until.is_some()
                || capture.failure.is_some()
                || capture.fit_job.as_ref().is_some_and(|j| !j.done())
            {
                return Err("temporal_fit_capture_not_ready".into());
            }
            capture.check_time(now)?;
            let input = capture
                .core
                .as_ref()
                .ok_or("temporal_initial_barrier_unconfirmed")?
                .prepare_fit_input(now)?;
            let binding = input.identity().clone();
            if binding.owner() != self.owner
                || binding.installation() != self.installation
                || binding.context() != capture.context.digest()
            {
                return Err("temporal_fit_owner_binding_changed".into());
            }
            let cancel = Arc::new(AtomicBool::new(false));
            let identity = TemporalFitIdentity {
                runtime_epoch: self.runtime_epoch.clone(),
                generation,
                attempt,
                capture: binding.clone(),
            };
            let lifetime = Arc::new(FitLifetime {
                cancel: cancel.clone(),
                publication: Mutex::new(()),
                completed: Mutex::new(false),
                completion: Condvar::new(),
                anchor: now,
                origin: Instant::now(),
                observed: AtomicU64::new(now),
                expires_at_ms: binding.expires_at_ms(),
                #[cfg(test)]
                io_hook:None,
                #[cfg(test)]
                hook: self
                    .temporal_fit_hook
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            });
            retired_fit = capture.fit_job.replace(TemporalFitJob {
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
            state.frozen_training = true;
            (input, identity, lifetime)
        };
        drop(retired_fit);
        drop(retired_job);
        let attempt = identity.attempt;
        let completion = FitCompletion {
            runtime: Arc::downgrade(self),
            owner: None,
            identity: identity.clone(),
            lifetime: lifetime.clone(),
        };
        executor.spawn_blocking(move || {
            // This guard outlives every outcome/session/prefix in the called
            // body, including panic unwinding and rejected result admission.
            let mut completion = completion;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                Self::compute_temporal_fit(&store, &recovery, input, &identity, &lifetime)
            }))
            .unwrap_or_else(|_| Err("temporal_fit_worker_failed_safely".into()));
            completion.owner = completion.runtime.upgrade();
            lifetime.at("completion_owner_upgraded");
            if let Some(runtime) = &completion.owner {
                runtime.admit_temporal_fit(&identity, &lifetime, result);
                #[cfg(test)]
                lifetime.at("accepted_ready_published");
            } else {
                drop(result);
            }
        });
        Ok(attempt)
    }
    fn compute_temporal_fit(
        store: &ArtifactStore,
        recovery: &RecoveryStore,
        input: PendingTemporalFitInput,
        identity: &TemporalFitIdentity,
        lifetime: &FitLifetime,
    ) -> Result<(Arc<TemporalModel>, TrainingReport, TemporalFitSaveResult,Option<VerifiedTemporalFitSource>), String> {
        lifetime.at("before_validate");
        let validated = input.validate(recovery, lifetime.allowed()?)?;
        let (mut session, permit,mut source) = validated.into_session_admission_and_source()?;
        lifetime.at("before_fit");
        lifetime.allowed()?;
        let configured = session.bound_managed_fit_config()?;
        let (model, report) = session.fit_managed_with_cancel(permit, &lifetime.cancel)?;
        // Session owns raw refs; release it on the worker, never in admission.
        drop(session);
        lifetime.at("before_save");
        lifetime.allowed()?;
        let saved = {
            let _publication = lifetime
                .publication
                .lock()
                .map_err(|_| "temporal_fit_publication_unavailable")?;
            let now = lifetime.allowed()?;
            identity.capture.recertify_current(recovery, now)?;
            lifetime.at("publication_entered");
            lifetime.allowed()?;
            #[cfg(not(test))]
            let saved =
                store.save_temporal_candidate_with_fit_summary(&model, &report, &configured)?;
            #[cfg(test)]
            let saved = store.save_temporal_candidate_with_fit_summary_probe(
                &model,
                &report,
                &configured,
                |_| lifetime.at("model_saved"),
            )?;
            lifetime.at("after_save");
            saved
        };
        source.bind_saved_fit(&model,&saved)?;
        identity
            .capture
            .recertify_current(recovery, lifetime.allowed()?)?;
        lifetime.at("before_admission");
        lifetime.allowed()?;
        Ok((Arc::new(model), report, saved,Some(source)))
    }
    fn admit_temporal_fit(
        &self,
        identity: &TemporalFitIdentity,
        lifetime: &Arc<FitLifetime>,
        mut outcome: Result<(Arc<TemporalModel>, TrainingReport, TemporalFitSaveResult,Option<VerifiedTemporalFitSource>), String>,
    ) {
        // Outcome and retired owners precede the guard: even rejection/panic
        // releases inner before destroying model/evidence/report allocations.
        let mut retired_diagnostic = None;
        let mut retired_report = None;
        let mut state = match self.inner.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        let same = identity.runtime_epoch == self.runtime_epoch
            && identity.generation == state.generation
            && Arc::ptr_eq(&state.job.cancel, &lifetime.cancel)
            && matches!(state.capture.as_ref(),Some(CaptureWorkflow::Temporal(c)) if c.owner_generation==state.generation
                && c.context.digest()==identity.capture.context()
                && c.context.owner_scope==self.owner && c.context.installation_id==self.installation
                && c.fit_job.as_ref().is_some_and(|j|j.identity==*identity&&Arc::ptr_eq(&j.lifetime,lifetime)));
        if !same {
            return;
        }
        let now = lifetime.now();
        let capture_valid=now.as_ref().is_ok_and(|now|{
            identity.capture.check_deadline(*now).is_ok()&&matches!(state.capture.as_ref(),Some(CaptureWorkflow::Temporal(c))
                if c.failure.is_none() && c.core.as_ref().and_then(|core|core.fit_capture_identity(*now).ok()).as_ref()==Some(&identity.capture))
        });
        if lifetime.cancel.load(Ordering::Acquire) || !capture_valid {
            state.job.phase = JobPhase::Cancelled;
            state.job.error = Some("temporal_fit_cancelled_or_capture_changed".into());
            return;
        }
        state.workflow_revision = state.workflow_revision.saturating_add(1);
        match &mut outcome {
            Ok((model, report, saved,source)) => {
                if let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() {
                    retired_diagnostic = capture.diagnostic_fit.take();
                    capture.diagnostic_fit = Some(TemporalDiagnosticFit {
                        model: model.clone(),
                        saved: TemporalFitSaveResult {
                            candidate: saved.candidate.clone(),
                            fit_diagnostics: saved.fit_diagnostics.clone(),
                        },
                        accepted:if source.as_ref().is_some_and(|s|s.is_full_run()){
                            Some(AcceptedTemporalFit{runtime_epoch:identity.runtime_epoch.clone(),generation:identity.generation,
                                fit_attempt:identity.attempt,model:model.clone(),source:source.take().unwrap()})
                        }else{None},
                    });
                }
                retired_report = state.job.report.replace(report.clone());
                state.job.phase = JobPhase::Ready;
                state.job.error = None;
            }
            Err(error) => {
                state.job.phase = JobPhase::Failed;
                state.job.error = Some(bounded(error.clone()));
            }
        }
        drop(state);
        drop(retired_report);
        drop(retired_diagnostic);
        drop(outcome);
    }
    pub(super) fn temporal_diagnostic_fit(
        &self,
    ) -> Result<
        Option<(
            Arc<TemporalModel>,
            TemporalCandidateReceipt,
            TemporalFitDiagnostics,
        )>,
        String,
    > {
        let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_ref() else {
            return Err(PRIVATE_KIND.into());
        };
        if capture.owner_generation != state.generation {
            return Err("temporal_owner_generation_changed".into());
        }
        if capture.diagnostic_fit.is_some()
            && (capture.failure.is_some()
                || state.job.phase != JobPhase::Ready
                || capture.context.owner_scope != self.owner
                || capture.context.installation_id != self.installation
                || capture.fit_job.as_ref().is_none_or(|job| {
                    job.lifetime.cancel.load(Ordering::Acquire)
                        || job.identity.capture.context() != capture.context.digest()
                }))
        {
            return Err("temporal_fit_owner_or_context_unavailable".into());
        }
        Ok(capture.diagnostic_fit.as_ref().map(|fit| {
            (
                fit.model.clone(),
                fit.saved.candidate.clone(),
                fit.saved.fit_diagnostics.clone(),
            )
        }))
    }
}

#[cfg(all(test, unix))]
#[path = "lifecycle_temporal_fit_tests.rs"]
mod tests;
