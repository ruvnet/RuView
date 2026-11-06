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
    owner_generation: u64,
    core: Option<TemporalTrainCapture>,
    lead_in_until: Option<u64>,
    failure: Option<String>,
    last_now_ns: u64,
}
impl RuntimeTemporalCapture {
    pub(super) fn owner_generation(&self) -> u64 {
        self.owner_generation
    }
    pub(super) fn invalidate_owner(&mut self) {
        self.lead_in_until = None;
        if let Some(core) = self.core.as_mut() {
            core.invalidate("temporal_owner_generation_changed");
        }
    }
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
        retired: &mut DeferredReclamation,
    ) -> Result<(), String> {
        self.check_time(now)?;
        let admission = self
            .core
            .as_mut()
            .ok_or("temporal_initial_barrier_unconfirmed")?
            .poll_async(now, retired)?;
        let now = admission.map_or(now, |(_, at)| at);
        self.last_now_ns = now;
        if admission.is_some_and(|(start, _)| start) {
            self.lead_in_until = Some(
                now.checked_add(TRAIN_LEAD_IN_NS)
                    .ok_or("temporal_time_overflow")?,
            );
            return self.core.as_mut().unwrap().prime_fence(packet, now);
        }
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
            TemporalCaptureStage::Failed => Err("temporal_capture_unavailable".into()),
            TemporalCaptureStage::Blocked => core.prime_fence(packet, now),
            TemporalCaptureStage::Ready
            | TemporalCaptureStage::Paused
            | TemporalCaptureStage::Preparing => core.prime_fence(packet, now),
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
            .as_deref()
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
        let store = self
            .recovery_store
            .clone()
            .ok_or("temporal_recovery_unavailable")?;
        self.configure_temporal_capture_with(
            expected,
            context,
            run,
            now,
            move |schedule, run, fit, now, _| {
                TemporalTrainCapture::prepare_async(schedule, run, fit, now, store)
            },
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
        state.capture = Some(CaptureWorkflow::Temporal(Box::new(
            RuntimeTemporalCapture {
                context: context.clone(),
                owner_generation: next_generation,
                core: None,
                lead_in_until: None,
                failure: None,
                last_now_ns: now,
            },
        )));
        state.generation = next_generation;
        drop(state);
        // Initial crypto/filesystem execution never holds the receiver lock.
        let result = prepare(schedule, run, fit, now, store);
        let (core, failure) = match result {
            Ok((core, error)) => (Some(core), error),
            Err(error) => (None, Some(error)),
        };
        let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        if state.generation != next_generation || state.capture_context() != Some(&context) {
            // A discarded asynchronous constructor may already have a queued
            // initial transaction. Drop/drain it only outside the ingress lock.
            drop(state);
            drop(core);
            return Err("learner_state_changed".into());
        }
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
            return Err(PRIVATE_KIND.into());
        };
        capture.core = core;
        capture.failure = failure.clone();
        if let Some(error) = failure {
            state.input_error = Some(bounded(error.clone()));
            return Err(error);
        }
        Ok(())
    }
    fn start_temporal_phase(&self, expected: &MutationGuard, now: u64) -> Result<(), String> {
        let mut state = self.command_state(expected)?;
        let owner_generation = state.generation;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
            return Err(PRIVATE_KIND.into());
        };
        if capture.owner_generation != owner_generation {
            capture
                .core
                .as_mut()
                .map(|c| c.invalidate("temporal_owner_generation_changed"));
            return Err("temporal_owner_generation_changed".into());
        }
        capture.check_time(now)?;
        let core = capture
            .core
            .as_mut()
            .ok_or("temporal_initial_barrier_unconfirmed")?;
        if capture.lead_in_until.is_some()
            || !matches!(
                core.stage(),
                TemporalCaptureStage::Ready | TemporalCaptureStage::Paused
            )
        {
            return Err("temporal_phase_not_startable".into());
        }
        if core.async_start(now)? {
            return Ok(());
        }
        // Synchronous reference path retains its original behavior.
        capture.lead_in_until = Some(
            now.checked_add(TRAIN_LEAD_IN_NS)
                .ok_or("temporal_time_overflow")?,
        );
        Ok(())
    }
    fn tick_temporal_capture(&self, expected: &MutationGuard, now: u64) -> Result<(), String> {
        let store = self.temporal_store()?;
        // Must precede MutexGuard for success, rejection and unwinding alike.
        let mut retired = self.deferred_reclamation();
        let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        if *expected != self.guard_for(&state) {
            return Err("learner_state_changed".into());
        }
        let owner_generation = state.generation;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
            return Err(PRIVATE_KIND.into());
        };
        if capture.owner_generation != owner_generation {
            capture
                .core
                .as_mut()
                .map(|c| c.invalidate("temporal_owner_generation_changed"));
            return Err("temporal_owner_generation_changed".into());
        }
        capture.check_time(now)?;
        let admission = capture
            .core
            .as_mut()
            .ok_or("temporal_initial_barrier_unconfirmed")?
            .poll_async(now, &mut retired)?;
        #[cfg(test)]
        if retired.has_ack()
            && self
                .panic_after_reclamation_ack
                .swap(false, Ordering::AcqRel)
        {
            panic!("synthetic panic after actual ACK admission");
        }
        let now = admission.map_or(now, |(_, at)| at);
        capture.last_now_ns = now;
        if admission.is_some_and(|(start, _)| start) {
            capture.lead_in_until = Some(
                now.checked_add(TRAIN_LEAD_IN_NS)
                    .ok_or("temporal_time_overflow")?,
            );
            return Ok(());
        }
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
        let owner_generation = state.generation;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
            return Err(PRIVATE_KIND.into());
        };
        if capture.owner_generation != owner_generation {
            capture
                .core
                .as_mut()
                .map(|c| c.invalidate("temporal_owner_generation_changed"));
            return Err("temporal_owner_generation_changed".into());
        }
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
        let owner_generation = state.generation;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
            return Err(PRIVATE_KIND.into());
        };
        if capture.owner_generation != owner_generation {
            capture
                .core
                .as_mut()
                .map(|c| c.invalidate("temporal_owner_generation_changed"));
            return Err("temporal_owner_generation_changed".into());
        }
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
        state.capture = Some(CaptureWorkflow::Temporal(Box::new(
            RuntimeTemporalCapture {
                context: context.clone(),
                owner_generation: next_generation,
                core: None,
                lead_in_until: None,
                failure: None,
                last_now_ns: now,
            },
        )));
        state.generation = next_generation;
        drop(state);
        let mut core =
            TemporalTrainCapture::cold_restore(store, context, now, &self.runtime_epoch, now)?;
        core.enable_async_restore(
            self.recovery_store
                .clone()
                .ok_or("temporal_recovery_unavailable")?,
            now,
        )?;
        let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        if state.generation != next_generation {
            drop(state);
            drop(core);
            return Err("learner_state_changed".into());
        }
        state.capture = Some(CaptureWorkflow::Temporal(Box::new(
            RuntimeTemporalCapture {
                context: context.clone(),
                owner_generation: next_generation,
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
        let owner_generation = state.generation;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
            return Err(PRIVATE_KIND.into());
        };
        if capture.owner_generation != owner_generation {
            capture.invalidate_owner();
            return Err("temporal_owner_generation_changed".into());
        }
        capture
            .core
            .as_mut()
            .ok_or("temporal_initial_barrier_unconfirmed")?
            .repair_pending(store, now)?;
        capture.failure = None;
        state.input_error = None;
        Ok(())
    }
    fn cancel_temporal_durability(&self, expected: &MutationGuard) -> Result<(), String> {
        let writer = {
            let mut state = self.command_state(expected)?;
            let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
                return Err(PRIVATE_KIND.into());
            };
            capture.lead_in_until = None;
            let core = capture
                .core
                .as_mut()
                .ok_or("temporal_initial_barrier_unconfirmed")?;
            let writer = core.writer();
            core.invalidate("temporal_owner_write_cancelled");
            if let Some(writer) = &writer {
                writer.cancel();
            }
            writer
        };
        if let Some(writer) = writer {
            writer.drain()?;
        }
        Ok(())
    }
    fn temporal_capture_snapshot(&self, now: u64) -> Result<Value, String> {
        let state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_ref() else {
            return Err(PRIVATE_KIND.into());
        };
        if capture.owner_generation != state.generation {
            return Err("temporal_owner_generation_changed".into());
        }
        Ok(
            json!({"schema":"ruview.room-learner.private-temporal-capture.v1","mutation_guard":self.guard_for(&state),"capture":capture.snapshot(now),"fit_authority":false,"public_activation":false}),
        )
    }
}

#[cfg(test)]
#[path = "lifecycle_temporal_tests.rs"]
mod tests;

impl Drop for RoomLearningRuntime {
    fn drop(&mut self) {
        // The last Runtime owner has exclusive access; get_mut takes no ingress
        // lock. Join before owner/store release so an old writer cannot outlive
        // Runtime shutdown and recreate an erased profile later.
        let state = match self.inner.get_mut() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        let writer = match state.capture.as_mut() {
            Some(CaptureWorkflow::Temporal(capture)) => capture.core.as_mut().and_then(|core| {
                let writer = core.writer();
                core.invalidate("temporal_runtime_dropped");
                writer
            }),
            _ => None,
        };
        if let Some(writer) = writer {
            let _ = writer.drain();
        }
    }
}
