//! Default-inactive native Train capture bridge. No HTTP constructor or fit authority.
use super::*;
use crate::room_learner::{
    managed_schedule_contract::{LeadInPolicy, ManagedScheduleContract},
    native::CanonicalPacket,
    temporal_model_input::managed::execution::{TemporalCaptureStage, TemporalTrainCapture},
};

const TRAIN_LEAD_IN_NS: u64 = 5_000_000_000;
const PRIVATE_KIND: &str = "learner_temporal_capture_private_unsupported_command";

pub(super) struct RuntimeTemporalCapture {
    context: CaptureContext,
    core: Option<TemporalTrainCapture>,
    lead_in_until: Option<u64>,
    failure: Option<String>,
    last_now_ns: u64,
}
impl RuntimeTemporalCapture {
    pub(super) fn context(&self) -> &CaptureContext {
        &self.context
    }
    pub(super) fn phase_identity(&self) -> Option<PhaseIdentity> {
        self.core
            .as_ref()?
            .phase_identity()
            .map(|(trial_id, session_id)| PhaseIdentity {
                trial_id,
                session_id,
            })
    }
    pub(super) fn active(&self) -> bool {
        self.lead_in_until.is_some()
            || self.core.as_ref().is_some_and(|c| {
                matches!(
                    c.stage(),
                    TemporalCaptureStage::Observing | TemporalCaptureStage::NeedsConfirmation
                )
            })
    }
    fn check_time(&mut self, now: u64) -> Result<(), String> {
        if now < self.last_now_ns {
            self.lead_in_until = None;
            self.failure = Some("temporal_receiver_clock_reversed".into());
            if let Some(core) = self.core.as_mut() {
                core.invalidate("temporal_receiver_clock_reversed");
            }
            return Err("temporal_receiver_clock_reversed".into());
        }
        self.core
            .as_mut()
            .ok_or("temporal_initial_barrier_unconfirmed")?
            .check_time(now)?;
        self.last_now_ns = now;
        Ok(())
    }
    fn arm(
        &mut self,
        store: &super::super::recovery_store::RecoveryStore,
        epoch: &str,
        now: u64,
    ) -> Result<(), String> {
        if let Some(at) = self.lead_in_until {
            if now >= at {
                self.lead_in_until = None;
                self.core
                    .as_mut()
                    .ok_or("temporal_initial_barrier_unconfirmed")?
                    .start_segment(epoch, at, store)?;
            }
        }
        Ok(())
    }
    pub(super) fn ingest(
        &mut self,
        store: &super::super::recovery_store::RecoveryStore,
        epoch: &str,
        packet: &CanonicalPacket,
        now: u64,
    ) -> Result<(), String> {
        self.check_time(now)?;
        self.arm(store, epoch, now)?;
        if self.lead_in_until.is_some() {
            return self
                .core
                .as_mut()
                .ok_or("temporal_initial_barrier_unconfirmed")?
                .prime_fence(packet, now);
        } // Preparation never credits RF observation.
        let core = self
            .core
            .as_mut()
            .ok_or("temporal_initial_barrier_unconfirmed")?;
        match core.stage() {
            TemporalCaptureStage::Observing => core.ingest(packet, now, store),
            TemporalCaptureStage::Failed | TemporalCaptureStage::Blocked => {
                Err("temporal_capture_unavailable".into())
            }
            TemporalCaptureStage::Ready | TemporalCaptureStage::Paused => {
                core.prime_fence(packet, now)
            }
            _ => Ok(()), // No automatic start, confirmation or new phase from packet arrival.
        }
    }
    pub(super) fn reject(&mut self, source_id: u8, grid_id: &str, reason: &str) {
        if self
            .context
            .sources
            .iter()
            .any(|s| s.source_id == source_id && s.grid_id == grid_id)
        {
            self.lead_in_until = None;
            self.failure = Some(bounded(reason.into()));
            if let Some(core) = self.core.as_mut() {
                core.invalidate(reason);
            }
        }
    }
    fn snapshot(&self, now: u64) -> Value {
        let mut value = self.core.as_ref().map_or_else(
            || json!({"stage":"blocked","error":self.failure}),
            |c| c.snapshot(),
        );
        value["lead_in_remaining_ms"] = json!(self
            .lead_in_until
            .map_or(0, |end| end.saturating_sub(now) / 1_000_000));
        value["runtime_state"] = json!(if self.lead_in_until.is_some() {
            "lead_in"
        } else {
            "capture"
        });
        value["runtime_error"] = json!(self.failure);
        value["public_activation"] = json!(false);
        value["fit_authority"] = json!(false);
        value
    }
}
impl RoomLearningRuntime {
    pub(super) fn temporal_store(
        &self,
    ) -> Result<&super::super::recovery_store::RecoveryStore, String> {
        self.recovery_store
            .as_ref()
            .ok_or("temporal_recovery_unavailable".into())
    }
    /// Deliberately PRIVATE: current public configure always constructs Legacy.
    /// Refuses to discard existing capture/candidate/selection/recovery state.
    fn configure_temporal_capture(
        &self,
        expected: &MutationGuard,
        context: CaptureContext,
        run: String,
        now: u64,
    ) -> Result<(), String> {
        self.configure_temporal_capture_with(
            expected,
            context,
            run,
            now,
            TemporalTrainCapture::prepare_retained,
        )
    }
    fn configure_temporal_capture_with(
        &self,
        expected: &MutationGuard,
        context: CaptureContext,
        run: String,
        now: u64,
        prepare: impl FnOnce(
            ManagedScheduleContract,
            String,
            FitConfig,
            u64,
            &super::super::recovery_store::RecoveryStore,
        ) -> Result<(TemporalTrainCapture, Option<String>), String>,
    ) -> Result<(), String> {
        context.validate()?;
        if context.owner_scope != self.owner
            || context.installation_id != self.installation
            || now == 0
        {
            return Err("temporal_context_owner_or_clock".into());
        }
        let schedule = ManagedScheduleContract::build(
            context.clone(),
            LeadInPolicy::Train5HeldOut15EngineeringV1,
        )?;
        let fit = FitConfig::for_source_count(context.sources.len());
        fit.validate_context(&context)?;
        let mut state = self.command_state(expected)?;
        if state.capture.is_some()
            || state.model.is_some()
            || state.selection.has_history()
            || state.job.phase != JobPhase::Idle
            || state.recovery.descriptor.is_some()
        {
            return Err("temporal_setup_would_replace_existing_work".into());
        }
        let store = self.temporal_store()?;
        let next_generation = state
            .generation
            .checked_add(1)
            .ok_or("learner generation exhausted")?;
        let result = prepare(schedule, run, fit, now, store);
        let (core, failure) = match result {
            Ok((core, barrier_error)) => (Some(core), barrier_error),
            Err(error) => (None, Some(error)),
        };
        state.capture = Some(CaptureWorkflow::Temporal(Box::new(
            RuntimeTemporalCapture {
                context,
                core,
                lead_in_until: None,
                failure: failure.clone(),
                last_now_ns: now,
            },
        )));
        state.generation = next_generation;
        if let Some(error) = failure {
            state.input_error = Some(bounded(error.clone()));
            return Err(error);
        }
        Ok(())
    }
    fn start_temporal_phase(&self, expected: &MutationGuard, now: u64) -> Result<(), String> {
        let mut state = self.command_state(expected)?;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
            return Err(PRIVATE_KIND.into());
        };
        capture.check_time(now)?;
        let core = capture
            .core
            .as_ref()
            .ok_or("temporal_initial_barrier_unconfirmed")?;
        if capture.lead_in_until.is_some()
            || !matches!(
                core.stage(),
                TemporalCaptureStage::Ready | TemporalCaptureStage::Paused
            )
        {
            return Err("temporal_phase_not_startable".into());
        }
        // The leaf checks the original descriptor/floors when this preparation ends.
        capture.lead_in_until = Some(
            now.checked_add(TRAIN_LEAD_IN_NS)
                .ok_or("temporal_time_overflow")?,
        );
        Ok(())
    }
    fn tick_temporal_capture(&self, expected: &MutationGuard, now: u64) -> Result<(), String> {
        let store = self.temporal_store()?;
        let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        if *expected != self.guard_for(&state) {
            return Err("learner_state_changed".into());
        }
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
            return Err(PRIVATE_KIND.into());
        };
        capture.check_time(now)?;
        capture.arm(store, &self.runtime_epoch, now)?;
        if capture.lead_in_until.is_some() {
            return Ok(());
        }
        let core = capture
            .core
            .as_mut()
            .ok_or("temporal_initial_barrier_unconfirmed")?;
        if core.stage() == TemporalCaptureStage::Observing {
            core.tick(now, store)
        } else {
            Ok(())
        }
    }
    fn temporal_confirmation_challenge(&self, expected: &MutationGuard) -> Result<String, String> {
        let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        if *expected != self.guard_for(&state) {
            return Err("learner_state_changed".into());
        }
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_ref() else {
            return Err(PRIVATE_KIND.into());
        };
        capture
            .core
            .as_ref()
            .ok_or("temporal_initial_barrier_unconfirmed")?
            .confirmation_challenge()
    }
    fn confirm_temporal_phase(
        &self,
        expected: &MutationGuard,
        challenge: &str,
        now: u64,
    ) -> Result<(), String> {
        let store = self.temporal_store()?;
        let mut state = self.command_state(expected)?;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
            return Err(PRIVATE_KIND.into());
        };
        capture.check_time(now)?;
        if capture.lead_in_until.is_some() {
            return Err("temporal_phase_incomplete".into());
        }
        capture
            .core
            .as_mut()
            .ok_or("temporal_initial_barrier_unconfirmed")?
            .confirm(challenge, store, now)
    }
    fn pause_temporal_capture(&self, expected: &MutationGuard, now: u64) -> Result<(), String> {
        let store = self.temporal_store()?;
        let mut state = self.command_state(expected)?;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
            return Err(PRIVATE_KIND.into());
        };
        capture.check_time(now)?;
        if capture.lead_in_until.is_some() {
            return Err("temporal_preparation_has_no_observation_to_pause".into());
        }
        capture
            .core
            .as_mut()
            .ok_or("temporal_initial_barrier_unconfirmed")?
            .pause(now, store)
    }
    fn restore_temporal_capture(
        &self,
        expected: &MutationGuard,
        context: &CaptureContext,
        now: u64,
    ) -> Result<(), String> {
        context.validate()?;
        if context.owner_scope != self.owner || context.installation_id != self.installation {
            return Err("temporal_context_owner_or_clock".into());
        }
        let store = self.temporal_store()?;
        let mut state = self.command_state(expected)?;
        if state.model.is_some()
            || state.selection.has_history()
            || state.job.phase != JobPhase::Idle
            || state.recovery.descriptor.is_some()
        {
            return Err("temporal_restore_would_replace_existing_work".into());
        }
        // A same-process reload must never lower known RAM replay/priming
        // floors. Resume Paused directly; repair Blocked by its exact intent.
        // A cold restore is admitted only into a fresh, unconfigured Runtime.
        if state.capture.is_some() {
            return Err("temporal_restore_requires_fresh_runtime".into());
        }
        let next_generation = state
            .generation
            .checked_add(1)
            .ok_or("learner generation exhausted")?;
        let core =
            TemporalTrainCapture::cold_restore(store, context, now, &self.runtime_epoch, now)?;
        state.capture = Some(CaptureWorkflow::Temporal(Box::new(
            RuntimeTemporalCapture {
                context: context.clone(),
                core: Some(core),
                lead_in_until: None,
                failure: None,
                last_now_ns: now,
            },
        )));
        state.generation = next_generation;
        Ok(())
    }
    fn repair_temporal_capture(&self, expected: &MutationGuard, now: u64) -> Result<(), String> {
        let store = self.temporal_store()?;
        let mut state = self.command_state(expected)?;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
            return Err(PRIVATE_KIND.into());
        };
        capture
            .core
            .as_mut()
            .ok_or("temporal_initial_barrier_unconfirmed")?
            .repair_pending(store, now)?;
        capture.failure = None;
        state.input_error = None;
        Ok(())
    }
    fn temporal_capture_snapshot(&self, now: u64) -> Result<Value, String> {
        let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_ref() else {
            return Err(PRIVATE_KIND.into());
        };
        Ok(
            json!({"schema":"ruview.room-learner.private-temporal-capture.v1","mutation_guard":self.guard_for(&state),"capture":capture.snapshot(now),"fit_authority":false,"public_activation":false}),
        )
    }
}

#[cfg(test)]
#[path = "lifecycle_temporal_tests.rs"]
mod tests;
