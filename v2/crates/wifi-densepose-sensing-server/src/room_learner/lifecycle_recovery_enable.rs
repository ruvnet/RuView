//! Explicitly protect an existing NEW RAM run without reconfiguring it.
//! Eligibility is process-local and consumed after success: restored/expired
//! captures cannot mint a fresh recovery deadline, even after explicit discard.
use super::recovery_control::{checkpoint_payload, model_sha, RuntimeCheckpoint, SCHEMA};
use super::*;
use crate::room_learner::recovery_store::{RecoveryDescriptor, RECOVERY_SCHEMA, RECOVERY_TTL_MS};

#[derive(Clone)]
pub(super) struct CurrentNewRun {
    id: String,
    capture_run: String,
    context_digest: String,
    created_at_ms: u64,
    expires_at_ms: u64,
}
impl CurrentNewRun {
    pub(super) fn new(capture: &CaptureSession, now_ns: u64) -> Option<Self> {
        let created_at_ms = now_ns / 1_000_000;
        if created_at_ms == 0 {
            return None;
        }
        Some(Self {
            id: crate::companion::secret(),
            capture_run: capture.checkpoint(now_ns).ok()?.run_id().into(),
            context_digest: capture.context().digest(),
            created_at_ms,
            expires_at_ms: created_at_ms.checked_add(RECOVERY_TTL_MS)?,
        })
    }
}
impl RoomLearningRuntime {
    pub(super) fn recovery_enable_status(&self, state: &Inner, now_ns: u64) -> Value {
        let Some(run) = &state.recovery_enable else {
            return json!({"schema":"ruview.room-learner.enable-current-recovery.v1","eligible":false,"run_id":null,"reason":"not_a_new_ram_run"});
        };
        let now_ms = now_ns.max(state.recovery.last_observed_ns()) / 1_000_000;
        let reason = if now_ms >= run.expires_at_ms {
            Some("new_run_recovery_deadline_expired")
        } else if state.recovery.enabled {
            Some("recovery_already_enabled")
        } else if busy(&state.job) || state.capture.as_ref().is_some_and(|c| c.phase_active()) {
            Some("finish_active_operation")
        } else if self.recovery_store.as_ref().is_none_or(|s| !s.available()) {
            Some("recovery_unavailable")
        } else if state
            .recovery
            .descriptor
            .as_ref()
            .is_some_and(|d| d.id != run.id)
        {
            Some("learner_recovery_pending")
        } else {
            None
        };
        json!({"schema":"ruview.room-learner.enable-current-recovery.v1","eligible":reason.is_none(),"run_id":run.id,
            "created_at_ms":run.created_at_ms,"expires_at_ms":run.expires_at_ms,"reason":reason,
            "consent_required":true,"deadline_basis":"new_run_created_at","reduced_samples_only":true})
    }
    /// Saves the exact current capture under its original new-run deadline.
    /// No reset, fit, role change, selection or renewal occurs. A failed write
    /// may have installed this SAME ticket; retry never creates a second one.
    pub fn enable_current_capture_recovery(
        &self,
        expected: &MutationGuard,
        run_id: &str,
        consent: bool,
        now_ns: u64,
    ) -> Result<(), String> {
        if !consent {
            return Err("recovery_consent_required".into());
        }
        let mut state = self.command_state(expected)?;
        let now_ns = state.recovery.observe_clock(now_ns);
        let run = state.recovery_enable.clone().ok_or("not_a_new_ram_run")?;
        if run.id != run_id {
            return Err("recovery_run_identity_mismatch".into());
        }
        if now_ns / 1_000_000 >= run.expires_at_ms {
            return Err("new_run_recovery_deadline_expired".into());
        }
        if state.recovery.enabled {
            return Err("recovery_already_enabled".into());
        }
        if busy(&state.job) || state.capture.as_ref().is_some_and(|c| c.phase_active()) {
            return Err("finish_active_operation".into());
        }
        let capture = state.capture.as_ref().ok_or("recovery_capture_missing")?;
        if capture.context().digest() != run.context_digest {
            return Err("recovery_context_mismatch".into());
        }
        let snapshot = capture.checkpoint(now_ns)?;
        if snapshot.run_id() != run.capture_run {
            return Err("recovery_run_identity_mismatch".into());
        }
        let summary = snapshot.summary();
        let payload = RuntimeCheckpoint {
            schema: SCHEMA.into(),
            capture: snapshot,
            frozen_training: state.frozen_training,
            job_phase: state.job.phase,
            model_sha256: state.model.as_ref().map(|m| model_sha(m)).transpose()?,
        };
        let bytes = serde_json::to_vec(&payload).map_err(|_| "recovery_payload_encode_failed")?;
        let descriptor = RecoveryDescriptor {
            schema: RECOVERY_SCHEMA.into(),
            id: run.id,
            owner_scope: self.owner.clone(),
            installation_id: self.installation.clone(),
            context_digest: run.context_digest,
            created_at_ms: run.created_at_ms,
            saved_at_ms: now_ns / 1_000_000,
            expires_at_ms: run.expires_at_ms,
        };
        let store = self
            .recovery_store
            .as_ref()
            .filter(|s| s.available())
            .ok_or("recovery_unavailable")?;
        let inspected = match store.inspect() {
            Ok(value) => value,
            Err(error) => {
                state.recovery.error = Some(bounded(error.clone()));
                return Err(error);
            }
        };
        if let Some(previous) = inspected {
            // Only a partial durable write by this very action may be retried.
            if previous.id != descriptor.id
                || previous.owner_scope != descriptor.owner_scope
                || previous.installation_id != descriptor.installation_id
                || previous.context_digest != descriptor.context_digest
                || previous.created_at_ms != descriptor.created_at_ms
                || previous.expires_at_ms != descriptor.expires_at_ms
            {
                return Err("learner_recovery_pending".into());
            }
            let loaded = store.load(now_ns / 1_000_000)?.ok_or("recovery_missing")?;
            let prior = checkpoint_payload(
                &loaded.plaintext,
                &loaded.descriptor,
                &self.owner,
                &self.installation,
                now_ns,
            )?;
            if prior.capture.run_id() != run.capture_run {
                return Err("recovery_run_identity_mismatch".into());
            }
        }
        checkpoint_payload(&bytes, &descriptor, &self.owner, &self.installation, now_ns)?;
        // save includes private atomic write, fsync and exact authenticated
        // readback under the store lock; errors do not enable persistence.
        if let Err(error) = store.save(&descriptor, &bytes, now_ns / 1_000_000) {
            // A failed durability/readback barrier can still have installed
            // ciphertext. Unknown retention must never be reported as absent.
            state.recovery.error = Some(bounded(error.clone()));
            return Err(error);
        }
        state.recovery = RecoveryState::current_run_saved(descriptor, summary, now_ns);
        state.recovery_enable = None;
        Ok(())
    }
}
