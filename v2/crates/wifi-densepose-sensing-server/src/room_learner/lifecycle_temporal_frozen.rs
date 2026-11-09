//! Private owner transition from actual C3 acceptance to a durable Frozen ACK.
//! No public fit/HeldOut/qualification/selection/Room Save activation.
use super::*;
impl RoomLearningRuntime {
    pub(super) fn prepare_frozen_transition(
        &self,
        expected: &MutationGuard,
        now: u64,
    ) -> Result<(), String> {
        let store = self.store.clone();
        let mut state = self.command_state(expected)?;
        if state.job.phase != JobPhase::Ready {
            return Err("temporal_actual_current_fit_required".into());
        }
        let generation = state.generation;
        let revision = state.workflow_revision;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
            return Err(PRIVATE_KIND.into());
        };
        if capture.owner_generation != generation {
            capture.invalidate_owner();
            return Err("temporal_owner_generation_changed".into());
        }
        capture.check_time(now)?;
        let core = capture
            .core
            .as_ref()
            .ok_or("temporal_initial_barrier_unconfirmed")?;
        if !core.is_full_run() {
            return Err("temporal_full_run_no_upconversion".into());
        }
        if core.stage() != TemporalCaptureStage::Complete {
            return Err("temporal_full_run_train_complete_required".into());
        }
        let accepted = capture.take_current_accepted_fit(&self.runtime_epoch, generation, now)?;
        let run = capture
            .core
            .as_ref()
            .unwrap()
            .fit_capture_identity(now)?
            .run_id()
            .to_owned();
        let witness = FrozenOwnerWitness {
            runtime_epoch: self.runtime_epoch.clone(),
            generation,
            revision,
            context: capture.context.clone(),
            run,
        };
        capture
            .core
            .as_mut()
            .unwrap()
            .request_frozen(accepted, witness, store, now)
    }
    pub(super) fn current_verified_frozen_model(
        &self,
    ) -> Result<Option<Arc<crate::room_learner::temporal_model_input::TemporalModel>>, String> {
        let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_ref() else {
            return Err(PRIVATE_KIND.into());
        };
        if capture.owner_generation != state.generation || capture.failure.is_some() {
            return Err("temporal_frozen_owner_unavailable".into());
        }
        Ok(capture
            .core
            .as_ref()
            .and_then(|c| c.verified_frozen_model()))
    }
}
