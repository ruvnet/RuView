//! Managed capture -> genuine learning -> private cold-loadable candidate.
//! Candidate predictions never grant physical qualification or replace control.
use super::{
    artifact::{ArtifactStore, CandidateReceipt},
    capture::{CaptureSession, PhaseReceipt, PhaseRequest, RawSourceFrame},
    model::{RoomModel, RoomPrediction},
    observation::{CaptureContext, DataRole, RoomLabel},
    training::{EvaluationReport, FitConfig, TrainingReport},
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
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
    input_error: Option<String>,
}
pub struct RoomLearningRuntime {
    runtime_epoch: String,
    owner: String,
    installation: String,
    store: ArtifactStore,
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
        let clock_origin = std::time::Instant::now();
        let clock_epoch_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "receiver clock unavailable")?
            .as_nanos()
            .try_into()
            .map_err(|_| "receiver clock outside bounds")?;
        Ok(Self {
            clock_origin,
            clock_epoch_ns,
            runtime_epoch: crate::companion::secret(),
            owner: owner.into(),
            installation: installation.into(),
            store: ArtifactStore::new(directory, owner, installation)?,
            inner: Mutex::new(Inner {
                generation: 0,
                workflow_revision: 0,
                capture: None,
                model: None,
                receipt: None,
                frozen_training: false,
                job: Job::default(),
                last_evaluation: None,
                input_error: None,
            }),
        })
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
        state.last_evaluation = None;
        state.input_error = None;
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
    /// Evaluation does not mutate model parameters and never promotes physical
    /// authority. The held-out result is a candidate diagnostic, not a certificate.
    pub fn evaluate(&self, expected: &MutationGuard, role: DataRole) -> Result<Value, String> {
        if !matches!(role, DataRole::Validation | DataRole::HeldOut) {
            return Err("evaluate only validation or held-out data".into());
        }
        let mut state = self.command_state(expected)?;
        if busy(&state.job) {
            return Err("wait for the frozen candidate".into());
        }
        let model = state.model.as_ref().ok_or("no frozen candidate")?;
        let capture = state.capture.as_ref().ok_or("no capture context")?;
        if capture.phase_active() {
            return Err("finish the current phase before evaluation".into());
        }
        let dataset = capture.validated_dataset()?;
        let rows = if role == DataRole::HeldOut {
            dataset.held_out()
        } else {
            dataset.validation()
        };
        if rows.is_empty() {
            return Err("no independent rows for the selected evaluation role".into());
        }
        let report: EvaluationReport = model.evaluate(rows)?;
        let result = json!({"schema":"ruview.room-learner.evaluation.v1","role":role,
            "context_digest":capture.context().digest(),"report":report,
            "measurement_clock_available":dataset.measurement_clock_available(),
            "physically_validated":false,"adoption_authorized":false});
        state.last_evaluation = Some(result.clone());
        Ok(result)
    }
    pub fn status(&self, now_ns: u64) -> Value {
        let Ok(state) = self.inner.lock() else {
            return json!({"enabled":true,"state":"unavailable"});
        };
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
            "phase_lead_in_defaults":{"Empty":15000,"OccupiedStatic":5000,"OccupiedMoving":5000,"AdjacentOnly":15000},
            "untrained_capture_progress_is_volatile":true,
            "generation":state.generation,"state":state.job.phase,"context_digest":state.capture.as_ref().map(|c|c.context().digest()),
            "room_digest":state.capture.as_ref().map(|c| &c.context().room_digest),
            "revision":state.capture.as_ref().map(|c|c.context().revision),
            "sources":state.capture.as_ref().map(|c| &c.context().sources),
            "capture":state.capture.as_ref().map(|c|c.progress(now_ns)),
            "training_frozen":state.frozen_training,"training_defaults":FitConfig::for_source_count(state.capture.as_ref().map_or(0, |c| c.context().sources.len())),"training_report":state.job.report,
            "error":state.job.error,"input_error":state.input_error,
            "candidate":state.receipt,"candidate_loaded":state.model.is_some(),
            "last_evaluation":state.last_evaluation,"physically_validated":false,
            "control_detector_changed":false,"raw_observations_persisted":false,
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
