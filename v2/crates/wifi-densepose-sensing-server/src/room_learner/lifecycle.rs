//! Managed capture -> genuine learning -> private cold-loadable candidate.
//! Candidate predictions never grant physical qualification or replace control.
use super::{
    artifact::{ArtifactStore, CandidateReceipt},
    capture::{CaptureSession, PhaseReceipt, PhaseRequest, RawSourceFrame},
    model::{RoomModel, RoomPrediction},
    observation::{CaptureContext, DataRole, RoomLabel},
    training::{FitConfig, TrainingReport},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobPhase {
    Idle,
    Running,
    Cancelling,
    Ready,
    Failed,
    Cancelled,
}
struct Job {
    phase: JobPhase,
    cancel: Arc<AtomicBool>,
    report: Option<TrainingReport>,
    error: Option<String>,
}
impl Default for Job {
    fn default() -> Self {
        Self {
            phase: JobPhase::Idle,
            cancel: Arc::new(AtomicBool::new(false)),
            report: None,
            error: None,
        }
    }
}
struct Inner {
    generation: u64,
    workflow_revision: u64,
    capture: Option<CaptureSession>,
    model: Option<Arc<RoomModel>>,
    receipt: Option<CandidateReceipt>,
    frozen_training: bool,
    job: Job,
    last_evaluation: Option<Value>,
    evaluation_warning: Option<String>,
    input_error: Option<String>,
    recovery: RecoveryState,
    selection: selection_control::SelectionState,
    qualification: Option<super::qualification::QualificationEnvelope>,
    qualification_error: Option<String>,
}
pub struct RoomLearningRuntime {
    runtime_epoch: String,
    owner: String,
    installation: String,
    store: ArtifactStore,
    recovery_store: Option<super::recovery_store::RecoveryStore>,
    inner: Mutex<Inner>,
    clock_origin: std::time::Instant,
    clock_epoch_ns: u64,
}
fn busy(job: &Job) -> bool {
    matches!(job.phase, JobPhase::Running | JobPhase::Cancelling)
}
fn bounded(error: String) -> String {
    error
        .chars()
        .filter(|c| !c.is_control())
        .take(256)
        .collect()
}
impl RoomLearningRuntime {
    pub fn new(directory: &Path, owner: &str, installation: &str) -> Result<Self, String> {
        Self::build(directory, owner, installation, None)
    }
    pub fn new_with_recovery(
        directory: &Path,
        owner: &str,
        installation: &str,
        recovery: super::recovery_store::RecoveryStore,
    ) -> Result<Self, String> {
        Self::build(directory, owner, installation, Some(recovery))
    }
    pub fn new_with_recovery_unavailable(
        directory: &Path,
        owner: &str,
        installation: &str,
        reason: String,
    ) -> Result<Self, String> {
        let runtime = Self::new(directory, owner, installation)?;
        {
            let mut state = runtime
                .inner
                .lock()
                .map_err(|_| "learner state unavailable")?;
            state.recovery.supported = true;
            state.recovery.error = Some(bounded(reason));
        }
        Ok(runtime)
    }
    /// Explicit time domain for deterministic fixtures; production uses the managed clock.
    pub fn new_with_recovery_at(
        directory: &Path,
        owner: &str,
        installation: &str,
        recovery: super::recovery_store::RecoveryStore,
        now_ns: u64,
    ) -> Result<Self, String> {
        if now_ns == 0 {
            return Err("recovery_clock_invalid".into());
        }
        let mut runtime = Self::build(directory, owner, installation, Some(recovery))?;
        runtime.clock_epoch_ns = now_ns;
        runtime.clock_origin = std::time::Instant::now();
        {
            let mut state = runtime
                .inner
                .lock()
                .map_err(|_| "learner state unavailable")?;
            state.recovery = RecoveryState::default();
            runtime.inspect_recovery(&mut state, now_ns);
        }
        Ok(runtime)
    }
    fn build(
        directory: &Path,
        owner: &str,
        installation: &str,
        recovery_store: Option<super::recovery_store::RecoveryStore>,
    ) -> Result<Self, String> {
        let clock_origin = std::time::Instant::now();
        let clock_epoch_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "receiver clock unavailable")?
            .as_nanos()
            .try_into()
            .map_err(|_| "receiver clock outside bounds")?;
        let runtime = Self {
            clock_origin,
            clock_epoch_ns,
            runtime_epoch: crate::companion::secret(),
            owner: owner.into(),
            installation: installation.into(),
            store: ArtifactStore::new(directory, owner, installation)?,
            recovery_store,
            inner: Mutex::new(Inner {
                generation: 0,
                workflow_revision: 0,
                capture: None,
                model: None,
                receipt: None,
                frozen_training: false,
                job: Job::default(),
                last_evaluation: None,
                evaluation_warning: None,
                input_error: None,
                recovery: RecoveryState::default(),
                selection: selection_control::SelectionState::default(),
                qualification: None,
                qualification_error: None,
            }),
        };
        {
            let mut state = runtime
                .inner
                .lock()
                .map_err(|_| "learner state unavailable")?;
            runtime.restore_selection(&mut state);
            runtime.inspect_recovery(&mut state, runtime.receiver_now_ns());
        }
        Ok(runtime)
    }
    /// UTC-anchored monotonic receiver time. Wall-clock adjustments cannot
    /// shorten an operator capture or turn a countdown into labelled RF data.
    pub fn receiver_now_ns(&self) -> u64 {
        self.clock_epoch_ns
            .saturating_add(self.clock_origin.elapsed().as_nanos().min(u64::MAX as u128) as u64)
    }
    pub fn saved_candidates(
        &self,
        room_digest: &str,
    ) -> Result<super::artifact::CandidateCatalog, String> {
        self.store.catalog_for_room(room_digest)
    }
    /// Explicit new capture versus resuming an exact persisted candidate. Neither
    /// path deletes old models, room receipts, pairing or artifact history.
    pub fn configure(
        &self,
        expected: &MutationGuard,
        context: CaptureContext,
        run_id: String,
        new_training: bool,
    ) -> Result<(), String> {
        self.configure_for_recovery(
            expected,
            context,
            run_id,
            new_training,
            false,
            self.receiver_now_ns(),
        )
    }
    pub fn configure_for_recovery(
        &self,
        expected: &MutationGuard,
        context: CaptureContext,
        run_id: String,
        new_training: bool,
        recovery_consent: bool,
        now_ns: u64,
    ) -> Result<(), String> {
        context.validate()?;
        if context.owner_scope != self.owner || context.installation_id != self.installation {
            return Err("learner context does not belong to this managed profile".into());
        }
        FitConfig::for_source_count(context.sources.len()).validate_context(&context)?;
        let session = CaptureSession::new(context.clone(), run_id)?;
        let mut state = self.command_state(expected)?;
        if busy(&state.job) || state.capture.as_ref().is_some_and(|c| c.phase_active()) {
            return Err(
                "finish/cancel the active capture or training job before changing context".into(),
            );
        }
        let restored = self.store.load_candidate(&context)?;
        if !new_training && restored.is_none() {
            return Err(
                "no stored candidate for this exact context; start a new training capture".into(),
            );
        }
        let (restored_evaluation, evaluation_warning) = if !new_training {
            restored
                .as_ref()
                .map(|model| self.restored_evaluation(model))
                .unwrap_or((None, None))
        } else {
            (None, None)
        };
        if recovery_consent {
            self.prepare_recovery(&mut state, &context, now_ns)?;
        } else {
            state.recovery.enabled = false;
            state.recovery.marker = None;
        }
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or("learner generation exhausted")?;
        state.capture = Some(session);
        state.model = if new_training {
            None
        } else {
            restored.map(Arc::new)
        };
        state.frozen_training = !new_training;
        state.receipt = None;
        state.job = Job::default();
        if state.model.is_some() {
            state.job.phase = JobPhase::Ready;
        }
        state.last_evaluation = restored_evaluation;
        state.evaluation_warning = evaluation_warning;
        state.input_error = None;
        state.qualification = None;
        state.qualification_error = None;
        if let Some(model)=state.model.clone() {
            match self.store.load_latest_qualification(&model) {
                Ok(q)=>{
                    state.qualification=q;
                    match self.store.load_latest_assessment(&model) {
                        Ok(Some(a))=>state.qualification_error=a.receipt.failure_reason,
                        Ok(None)=>{},Err(e)=>state.qualification_error=Some(bounded(e)),
                    }
                },
                Err(e)=>state.qualification_error=Some(bounded(e)),
            }
        }
        self.checkpoint_recovery(&mut state, now_ns, true)?;
        Ok(())
    }
    /// Called from an explicit protected action. Completion commits only a
    /// candidate, with cancellation and context generation checked under lock.
    pub fn start_training(
        self: &Arc<Self>,
        expected: &MutationGuard,
        config: FitConfig,
    ) -> Result<u64, String> {
        config.validate()?;
        let (generation, dataset, cancel) = {
            let mut state = self.command_state(expected)?;
            self.require_recovery_ready(&state)?;
            if busy(&state.job)
                || (state.frozen_training
                    && !matches!(state.job.phase, JobPhase::Failed | JobPhase::Cancelled))
            {
                return Err("this run is already training or frozen".into());
            }
            let capture = state
                .capture
                .as_ref()
                .ok_or("no learner capture configured")?;
            config.validate_context(capture.context())?;
            if capture.phase_active() {
                return Err("finish or cancel the capture phase first".into());
            }
            let dataset = capture.validated_dataset()?;
            for label in [
                RoomLabel::Empty,
                RoomLabel::OccupiedStatic,
                RoomLabel::OccupiedMoving,
                RoomLabel::AdjacentOnly,
            ] {
                if !dataset.train().iter().any(|c| c.label == label) {
                    return Err("capture Train empty, occupied-static, occupied-moving and adjacent-only trials before training".into());
                }
            }
            let cancel = Arc::new(AtomicBool::new(false));
            state.job = Job {
                phase: JobPhase::Running,
                cancel: cancel.clone(),
                report: None,
                error: None,
            };
            state.frozen_training = true;
            let now = self.checkpoint_now(&state);
            if let Err(error) = self.checkpoint_recovery(&mut state, now, true) {
                state.job.phase = JobPhase::Failed;
                state.job.error = Some(bounded(error.clone()));
                return Err(error);
            }
            (state.generation, dataset, cancel)
        };
        let runtime = self.clone();
        tokio::task::spawn_blocking(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                RoomModel::fit_with_cancel(&dataset, config, &cancel)
            }))
            .unwrap_or_else(|_| Err("learner worker failed safely".into()));
            let Ok(mut state) = runtime.inner.lock() else {
                return;
            };
            if state.generation != generation {
                return;
            }
            state.workflow_revision = state.workflow_revision.saturating_add(1);
            if cancel.load(Ordering::Acquire) {
                state.job.phase = JobPhase::Cancelled;
                state.job.error =
                    Some("training cancelled; previous stored candidates preserved".into());
                let now = runtime.checkpoint_now(&state);
                let _ = runtime.checkpoint_recovery(&mut state, now, true);
                return;
            }
            match outcome {
                Ok((model, report)) => match runtime.store.save_candidate(&model) {
                    Ok(receipt) => {
                        state.model = Some(Arc::new(model));
                        state.receipt = Some(receipt);
                        state.job.report = Some(report);
                        state.job.phase = JobPhase::Ready;
                    }
                    Err(error) => {
                        state.job.phase = JobPhase::Failed;
                        state.job.error = Some(bounded(error));
                    }
                },
                Err(error) => {
                    state.job.phase = JobPhase::Failed;
                    state.job.error = Some(bounded(error));
                }
            }
            let now = runtime.checkpoint_now(&state);
            let _ = runtime.checkpoint_recovery(&mut state, now, true);
        });
        Ok(generation)
    }
    pub fn prediction(&self, now_ns: u64) -> Result<(MutationGuard, RoomPrediction), String> {
        let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        let model = state
            .model
            .as_ref()
            .ok_or("no trained/restored candidate for this context")?;
        let cohort = state
            .capture
            .as_ref()
            .ok_or("no current capture context")?
            .live_cohort(now_ns)?;
        Ok((self.guard_for(&state), model.infer(&cohort)))
    }
    pub fn status(&self, now_ns: u64) -> Value {
        let Ok(mut state) = self.inner.lock() else {
            return json!({"enabled":true,"state":"unavailable"});
        };
        let _ = self.checkpoint_recovery(&mut state, now_ns, false);
        let required_ids: Vec<_> = state
            .capture
            .as_ref()
            .map(|c| c.context().sources.iter().map(|s| s.source_id).collect())
            .unwrap_or_default();
        let trained_ids: Vec<_> = state
            .model
            .as_ref()
            .map(|m| m.context().sources.iter().map(|s| s.source_id).collect())
            .unwrap_or_default();
        let prediction = state.model.as_ref().and_then(|m| {
            state
                .capture
                .as_ref()
                .and_then(|c| c.live_cohort(now_ns).ok())
                .map(|c| m.infer(&c))
        });
        let scored_ids = if prediction
            .as_ref()
            .is_some_and(|p| p.occupied_probability.is_some())
        {
            trained_ids.clone()
        } else {
            Vec::new()
        };
        json!({"schema":"ruview.room-learner.status.v1","enabled":true,"mutation_guard":self.guard_for(&state),
            "required_source_ids":required_ids,"trained_source_ids":trained_ids,"scored_source_ids":scored_ids,
            "candidate_prediction":prediction,"training_manifest":state.model.as_ref().map(|m|m.training_manifest()),
            "phase_history":state.capture.as_ref().map(|c|c.history()),
            "phase_duration_defaults":{"Empty":600000,"OccupiedStatic":12000,"OccupiedMoving":12000,"AdjacentOnly":12000},
            "held_out_phase_duration_defaults":{"Empty":12000,"OccupiedStatic":12000,"OccupiedMoving":12000,"AdjacentOnly":12000},
            "phase_lead_in_defaults":{"Empty":15000,"OccupiedStatic":5000,"OccupiedMoving":5000,"AdjacentOnly":15000},
            "untrained_capture_progress_is_volatile":!state.recovery.enabled || state.recovery.error.is_some(),"recovery":self.recovery_status(&state, now_ns),
            "generation":state.generation,"state":state.job.phase,"context_digest":state.capture.as_ref().map(|c|c.context().digest()),
            "room_digest":state.capture.as_ref().map(|c| &c.context().room_digest),
            "revision":state.capture.as_ref().map(|c|c.context().revision),
            "sources":state.capture.as_ref().map(|c| &c.context().sources),
            "capture":state.capture.as_ref().map(|c|c.progress(now_ns)),
            "training_frozen":state.frozen_training,"training_defaults":FitConfig::for_source_count(state.capture.as_ref().map_or(0, |c| c.context().sources.len())),"training_report":state.job.report,
            "error":state.job.error,"input_error":state.input_error,
            "candidate":state.receipt,"candidate_loaded":state.model.is_some(),"candidate_identity":self.candidate_identity(&state),
            "model_selection":self.selection_status_locked(&state,now_ns,None),
            "last_evaluation":state.last_evaluation,"evaluation_warning":state.evaluation_warning,"physically_validated":false,
            "control_detector_changed":false,"raw_observations_persisted":state.recovery.derived_persisted(),"derived_samples_persisted":state.recovery.derived_persisted(),"raw_iq_persisted":false,
            "limitations":["receiver-observed magnitude path; no coherent phase claim",
                "legacy UDP transmitter identity is unverified","independent teacher registry not configured",
                "candidate training is not physical qualification"]})
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;

#[path = "lifecycle_capture.rs"]
mod capture_control;

#[path = "command.rs"]
mod command;
pub use command::{MutationGuard, PhaseIdentity};

#[path = "lifecycle_recovery.rs"]
mod recovery_control;
use recovery_control::RecoveryState;

#[cfg(all(test, unix))]
#[path = "lifecycle_recovery_tests.rs"]
mod recovery_tests;

#[path = "lifecycle_evaluation.rs"]
mod evaluation_control;

#[cfg(test)]
#[path = "lifecycle_evaluation_tests.rs"]
mod evaluation_tests;

#[path = "lifecycle_selection.rs"]
mod selection_control;

#[cfg(test)]
#[path="lifecycle_selection_tests.rs"]
mod selection_tests;

#[cfg(test)]
#[path="lifecycle_assessment_tests.rs"]
mod assessment_tests;
