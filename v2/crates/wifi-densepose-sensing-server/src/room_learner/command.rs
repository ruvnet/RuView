//! Optimistic command identity, not authentication or qualification authority.
use super::*;
use serde::Deserialize;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PhaseIdentity {
    pub trial_id: String,
    pub session_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MutationGuard {
    pub runtime_epoch: String,
    pub generation: u64,
    pub workflow_revision: u64,
    pub context_digest: Option<String>,
    pub phase: Option<PhaseIdentity>,
}
impl RoomLearningRuntime {
    pub(super) fn guard_for(&self, state: &Inner) -> MutationGuard {
        MutationGuard {
            runtime_epoch: self.runtime_epoch.clone(),
            generation: state.generation,
            workflow_revision: state.workflow_revision,
            context_digest: state.capture_context().map(CaptureContext::digest),
            phase: match state.capture.as_ref() {
                Some(CaptureWorkflow::Legacy(c)) => c.progress(0).phase.map(|p| PhaseIdentity { trial_id: p.trial_id, session_id: p.session_id }),
                Some(CaptureWorkflow::Temporal(c)) => c.phase_identity(),
                None => None,
            },
        }
    }
    pub fn mutation_guard(&self) -> Result<MutationGuard, String> {
        let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        Ok(self.guard_for(&state))
    }
    /// Checks and consumes one expected workflow revision while holding the same
    /// mutex used by the actual mutation. Raw packet arrival does not consume it.
    pub(super) fn command_state(
        &self,
        expected: &MutationGuard,
    ) -> Result<std::sync::MutexGuard<'_, Inner>, String> {
        let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        if *expected != self.guard_for(&state) {
            return Err("learner_state_changed".into());
        }
        state.workflow_revision = state
            .workflow_revision
            .checked_add(1)
            .ok_or("learner command revision exhausted")?;
        Ok(state)
    }
}
