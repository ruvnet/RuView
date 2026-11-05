//! Explicit, encrypted recovery. No receiver buffers or automatic work resume.
use super::*;
use crate::room_learner::{
    capture::{CaptureCheckpoint, CaptureCheckpointSummary, RecoveryPhaseDisposition},
    recovery_store::{RecoveryDescriptor, RECOVERY_SCHEMA, RECOVERY_TTL_MS},
};
use sha2::{Digest, Sha256};

#[derive(Default)]
pub(super) struct RecoveryState {
    pub(super) supported: bool,
    pub(super) enabled: bool,
    pub(super) descriptor: Option<RecoveryDescriptor>,
    summary: Option<CaptureCheckpointSummary>,
    pub(super) marker: Option<String>,
    pub(super) error: Option<String>,
    pub(super) disposition: Option<RecoveryPhaseDisposition>,
    last_now_ns: u64,
    last_attempt_ns: u64,
    volatile_after_expiry: bool,
}
impl RecoveryState {
    pub(super) fn observe_clock(&mut self, now_ns: u64) -> u64 {
        self.last_now_ns = self.last_now_ns.max(now_ns);
        self.last_now_ns
    }
    pub(super) fn last_observed_ns(&self) -> u64 { self.last_now_ns }
    pub(super) fn current_run_saved(descriptor: RecoveryDescriptor, summary: CaptureCheckpointSummary, now_ns: u64) -> Self {
        Self { enabled:true, descriptor:Some(descriptor), summary:Some(summary), last_now_ns:now_ns, ..Default::default() }
    }
    pub(super) fn derived_persisted(&self) -> Option<bool> {
        // Unreadable retained contents are unknown, never evidence of absence.
        // An error also invalidates any older cached summary for this claim.
        if self.error.is_some() || (self.descriptor.is_some() && self.summary.is_none()) {
            return None;
        }
        Some(self.summary.as_ref().is_some_and(|s| {
            s.sealed_cohorts > 0
                || s.phase
                    .as_ref()
                    .is_some_and(|p| p.representative_cohorts > 0)
        }))
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RuntimeCheckpoint {
    pub(super) schema: String,
    pub(super) capture: CaptureCheckpoint,
    pub(super) frozen_training: bool,
    pub(super) job_phase: JobPhase,
    pub(super) model_sha256: Option<String>,
}
pub(super) const SCHEMA: &str = "ruview.room-learner.runtime-recovery.v1";
pub(super) fn model_sha(model: &RoomModel) -> Result<String, String> {
    Ok(format!("{:x}", Sha256::digest(model.export()?)))
}
pub(super) fn checkpoint_payload(
    bytes: &[u8],
    d: &RecoveryDescriptor,
    owner: &str,
    installation: &str,
    now_ns: u64,
) -> Result<RuntimeCheckpoint, String> {
    let p: RuntimeCheckpoint =
        serde_json::from_slice(bytes).map_err(|_| "recovery_payload_invalid")?;
    if p.schema != SCHEMA
        || p.capture.context().owner_scope != owner
        || p.capture.context().installation_id != installation
        || p.capture.context().digest() != d.context_digest
        || p.capture.saved_at_ns() / 1_000_000 != d.saved_at_ms
        || p.capture.saved_at_ns() < d.created_at_ms.saturating_mul(1_000_000)
        || (p.model_sha256.is_some() && (!p.frozen_training || p.job_phase != JobPhase::Ready))
        || (p.job_phase == JobPhase::Ready && p.model_sha256.is_none())
        || (!p.frozen_training && p.job_phase != JobPhase::Idle)
        || (p.frozen_training && p.job_phase == JobPhase::Idle)
    {
        return Err("recovery_payload_binding_mismatch".into());
    }
    CaptureSession::restore_checkpoint(p.capture.clone(), p.capture.context(), now_ns)?;
    Ok(p)
}
impl RoomLearningRuntime {
    pub(super) fn require_recovery_ready(&self, state: &Inner) -> Result<(), String> {
        if state.recovery.enabled && state.recovery.error.is_some() {
            Err(
                "recovery_save_failed; recover, discard, or explicitly configure RAM-only capture"
                    .into(),
            )
        } else {
            Ok(())
        }
    }
    /// Recovery availability is a disk deadline, not a lifetime for samples
    /// already loaded into this active process. Authenticate the unchanged copy
    /// before transitioning; other persistence failures never become RAM mode.
    pub(super) fn expire_loaded_recovery(
        &self,
        state: &mut Inner,
        now_ns: u64,
    ) -> Result<(), String> {
        state.recovery.last_now_ns = state.recovery.last_now_ns.max(now_ns);
        let now_ns = state.recovery.last_now_ns;
        if !state.recovery.enabled || state.capture.is_none() {
            return Ok(());
        }
        let Some(descriptor) = state.recovery.descriptor.as_ref() else {
            return Ok(()); // Existing checkpoint validation reports missing state.
        };
        if now_ns / 1_000_000 < descriptor.expires_at_ms {
            return Ok(());
        }
        if let Some(error) = &state.recovery.error {
            return Err(error.clone());
        }
        let verified = (|| -> Result<(), String> {
            if state.capture.as_ref().map(|capture| capture.context().digest())
                .as_deref() != Some(descriptor.context_digest.as_str())
            {
                return Err("recovery_context_mismatch".into());
            }
            let retained = self.recovery_store.as_ref().ok_or("recovery_unavailable")?
                .inspect()?.ok_or("recovery_missing")?;
            if retained != *descriptor {
                return Err("recovery_descriptor_changed".into());
            }
            Ok(())
        })();
        if let Err(error) = verified {
            state.recovery.error = Some(bounded(error.clone()));
            return Err(error);
        }
        state.recovery.enabled = false;
        state.recovery.marker = None;
        state.recovery.volatile_after_expiry = true;
        Ok(())
    }
    pub(super) fn checkpoint_now(&self, state: &Inner) -> u64 {
        self.receiver_now_ns().max(state.recovery.last_now_ns)
    }
    pub(super) fn inspect_recovery(&self, state: &mut Inner, now_ns: u64) {
        state.recovery.last_now_ns = state.recovery.last_now_ns.max(now_ns);
        let now_ns = state.recovery.last_now_ns;
        let Some(store) = &self.recovery_store else {
            return;
        };
        match store.inspect() {
            Ok(None) => {
                state.recovery.descriptor = None;
                state.recovery.summary = None;
            }
            Ok(Some(d)) => {
                if d.owner_scope != self.owner || d.installation_id != self.installation {
                    state.recovery.error = Some("recovery_owner_mismatch".into());
                    return;
                }
                state.recovery.descriptor = Some(d.clone());
                match store
                    .load(now_ns / 1_000_000)
                    .and_then(|v| v.ok_or_else(|| "recovery_missing".into()))
                    .and_then(|loaded| {
                        checkpoint_payload(
                            &loaded.plaintext,
                            &d,
                            &self.owner,
                            &self.installation,
                            now_ns,
                        )
                    }) {
                    Ok(p) => {
                        state.recovery.summary = Some(p.capture.summary());
                        state.recovery.error = None;
                    }
                    Err(error) => state.recovery.error = Some(bounded(error)),
                }
            }
            Err(error) => state.recovery.error = Some(bounded(error)),
        }
    }
    pub(super) fn prepare_recovery(
        &self,
        state: &mut Inner,
        context: &CaptureContext,
        now_ns: u64,
    ) -> Result<(), String> {
        let store = self
            .recovery_store
            .as_ref()
            .filter(|s| s.available())
            .ok_or("recovery_unavailable")?;
        match store.inspect() {
            Ok(Some(_)) => return Err("learner_recovery_pending".into()),
            Ok(None) => {}
            Err(error) => {
                state.recovery.error = Some(bounded(error.clone()));
                return Err(error);
            }
        }
        let created = now_ns / 1_000_000;
        let expires = created
            .checked_add(RECOVERY_TTL_MS)
            .ok_or("recovery_clock_invalid")?;
        state.recovery = RecoveryState {
            enabled: true,
            descriptor: Some(RecoveryDescriptor {
                schema: RECOVERY_SCHEMA.into(),
                id: crate::companion::secret(),
                owner_scope: self.owner.clone(),
                installation_id: self.installation.clone(),
                context_digest: context.digest(),
                created_at_ms: created,
                saved_at_ms: created,
                expires_at_ms: expires,
            }),
            ..Default::default()
        };
        Ok(())
    }
    pub(super) fn checkpoint_recovery(
        &self,
        state: &mut Inner,
        now_ns: u64,
        force: bool,
    ) -> Result<(), String> {
        state.recovery.last_now_ns = state.recovery.last_now_ns.max(now_ns);
        self.expire_loaded_recovery(state, now_ns)?;
        if !state.recovery.enabled {
            return Ok(());
        }
        let result: Result<bool, String> = (|| {
            let capture = state.capture.as_ref().ok_or("recovery_capture_missing")?;
            let marker = format!(
                "{}:{:?}:{}",
                capture.recovery_marker(now_ns),
                state.job.phase,
                state.model.is_some()
            );
            if !force
                && state.recovery.marker.as_ref() == Some(&marker)
                && (state.recovery.error.is_none()
                    || now_ns.saturating_sub(state.recovery.last_attempt_ns) < 5_000_000_000)
            {
                return Ok(false);
            }
            // Record attempts too, so one disk failure cannot cause packet-rate writes.
            state.recovery.marker = Some(marker);
            state.recovery.last_attempt_ns = now_ns;
            let snapshot = capture.checkpoint(now_ns)?;
            let summary = snapshot.summary();
            let payload = RuntimeCheckpoint {
                schema: SCHEMA.into(),
                capture: snapshot,
                frozen_training: state.frozen_training,
                job_phase: state.job.phase,
                model_sha256: state.model.as_ref().map(|m| model_sha(m)).transpose()?,
            };
            let mut descriptor = state
                .recovery
                .descriptor
                .clone()
                .ok_or("recovery_descriptor_missing")?;
            descriptor.saved_at_ms = now_ns / 1_000_000;
            let bytes =
                serde_json::to_vec(&payload).map_err(|_| "recovery_payload_encode_failed")?;
            let store = self.recovery_store.as_ref().ok_or("recovery_unavailable")?;
            if let Err(error) = store.save(&descriptor, &bytes, now_ns / 1_000_000) {
                // A rename may commit before directory fsync reports failure.
                // Read back exactly this attempt; never replay a mutation or
                // advertise durable success after the failed durability barrier.
                if let Ok(Some(loaded)) = store.load(now_ns / 1_000_000) {
                    if loaded.descriptor == descriptor && loaded.plaintext == bytes {
                        state.recovery.descriptor = Some(descriptor);
                        state.recovery.summary = Some(summary);
                    }
                }
                return Err(error);
            }
            state.recovery.descriptor = Some(descriptor);
            state.recovery.summary = Some(summary);
            Ok(true)
        })();
        match result {
            Ok(true) => {
                state.recovery.error = None;
                Ok(())
            }
            Ok(false) => Ok(()),
            Err(error) => {
                state.recovery.error = Some(bounded(error.clone()));
                Err(error)
            }
        }
    }
    pub(super) fn recovery_status(&self, state: &Inner, now_ns: u64) -> Value {
        let supported = self.recovery_store.is_some() || state.recovery.supported;
        let available = self.recovery_store.as_ref().is_some_and(|s| s.available());
        // A request may have sampled receiver time before waiting for this
        // mutex. Known expiry cannot be undone by that older observation.
        let now_ns = now_ns.max(state.recovery.last_now_ns);
        let expired = state.recovery.volatile_after_expiry || state
            .recovery
            .descriptor
            .as_ref()
            .is_some_and(|d| now_ns / 1_000_000 >= d.expires_at_ms);
        let saved = state
            .recovery
            .descriptor
            .as_ref()
            .zip(state.recovery.summary.as_ref())
            .filter(|_| !expired)
            .map(|(d, s)| {
                #[derive(Serialize)]
                struct Saved<'a> {
                    #[serde(flatten)]
                    descriptor: &'a RecoveryDescriptor,
                    #[serde(flatten)]
                    summary: &'a CaptureCheckpointSummary,
                }
                Saved {
                    descriptor: d,
                    summary: s,
                }
            });
        let status = if !supported {
            "disabled"
        } else if !available {
            "unavailable"
        } else if expired {
            "expired"
        } else if state.recovery.error.is_some() {
            "error"
        } else if state.recovery.summary.is_some() {
            if state.recovery.enabled {
                "saved"
            } else {
                "restore_required"
            }
        } else {
            "disabled"
        };
        let error = state.recovery.error.clone().or_else(|| {
            state.recovery.volatile_after_expiry.then(|| {
                "Recovery expired. This loaded run continues in memory only; new progress is not saved. Closing the helper loses these samples. The expired encrypted copy remains until explicitly discarded.".into()
            })
        }).or_else(|| {
            (supported && !available).then(|| {
                "recovery_unavailable: stable managed encryption key is unavailable".to_string()
            })
        });
        json!({"supported":supported,"enabled":state.recovery.enabled,"state":status,"error":error,
            "saved":saved,"discard_id":state.recovery.descriptor.as_ref().map(|d| &d.id),
            "restored_disposition":state.recovery.disposition,"raw_iq_persisted":false,
            "derived_samples_persisted":state.recovery.derived_persisted(),"automatic_resume":false,
            "enable_current_run":self.recovery_enable_status(state, now_ns)})
    }
    pub fn saved_recovery_context(
        &self,
        recovery_id: &str,
        now_ns: u64,
    ) -> Result<CaptureContext, String> {
        let mut state = self.inner.lock().map_err(|_| "learner state unavailable")?;
        state.recovery.last_now_ns = state.recovery.last_now_ns.max(now_ns);
        let now_ns = state.recovery.last_now_ns;
        let loaded = self
            .recovery_store
            .as_ref()
            .ok_or("recovery_unavailable")?
            .load(now_ns / 1_000_000)?
            .ok_or("recovery_missing")?;
        if loaded.descriptor.id != recovery_id {
            return Err("recovery_identity_mismatch".into());
        }
        let payload = checkpoint_payload(
            &loaded.plaintext,
            &loaded.descriptor,
            &self.owner,
            &self.installation,
            now_ns,
        )?;
        Ok(payload.capture.context().clone())
    }
    pub fn restore_capture_recovery(
        &self,
        expected: &MutationGuard,
        recovery_id: &str,
        context: &CaptureContext,
        now_ns: u64,
    ) -> Result<(), String> {
        let mut state = self.command_state(expected)?;
        state.recovery.last_now_ns = state.recovery.last_now_ns.max(now_ns);
        let now_ns = state.recovery.last_now_ns;
        if busy(&state.job) || state.capture.as_ref().is_some_and(|c| c.phase_active()) {
            return Err("finish or cancel the active operation first".into());
        }
        let loaded = self
            .recovery_store
            .as_ref()
            .ok_or("recovery_unavailable")?
            .load(now_ns / 1_000_000)?
            .ok_or("recovery_missing")?;
        if loaded.descriptor.id != recovery_id {
            return Err("recovery_identity_mismatch".into());
        }
        let payload = checkpoint_payload(
            &loaded.plaintext,
            &loaded.descriptor,
            &self.owner,
            &self.installation,
            now_ns,
        )?;
        if payload.capture.context() != context {
            return Err("recovery_context_mismatch".into());
        }
        let restored = CaptureSession::restore_checkpoint(payload.capture, context, now_ns)?;
        let model = payload
            .model_sha256
            .as_ref()
            .map(|sha| self.store.load_exact_candidate(context, sha))
            .transpose()?;
        if let Some(seed)=restored.session.metadata_lineage() {
            let candidate=model.as_ref().ok_or("managed recovery has no exact candidate")?;
            let proof=self.store.load_managed_training_proof(candidate)?.ok_or("managed recovery training proof missing")?;
            seed.ensure_extends(&proof.seed)?;
            let fence=self.store.load_managed_capture_fence(candidate,&proof)?;
            restored.session.managed_fence_seed(&proof)?.ensure_extends(&fence.seed)?;
        }
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or("learner generation exhausted")?;
        state.recovery_enable = None;
        state.capture = Some(restored.session);
        state.model = model.map(Arc::new);
        state.receipt = None;
        state.frozen_training = payload.frozen_training;
        state.job = Job::default();
        state.job.phase = if busy(&Job {
            phase: payload.job_phase,
            ..Job::default()
        }) {
            JobPhase::Failed
        } else {
            payload.job_phase
        };
        if state.job.phase == JobPhase::Failed {
            state.job.error = Some(
                "Saved training was interrupted or failed; retry explicitly using the frozen data."
                    .into(),
            );
        }
        state.last_evaluation = None;
        state.evaluation_warning = None;
        state.input_error = None;
        state.recovery = RecoveryState {
            enabled: true,
            descriptor: Some(loaded.descriptor),
            disposition: Some(restored.disposition),
            ..Default::default()
        };
        self.checkpoint_recovery(&mut state, now_ns, true)
    }
    pub fn discard_capture_recovery(
        &self,
        expected: &MutationGuard,
        recovery_id: &str,
    ) -> Result<(), String> {
        let mut state = self.command_state(expected)?;
        // Fence every later ingest/status/worker save before touching the file.
        state.recovery.enabled = false;
        state.recovery.marker = None;
        let result = self
            .recovery_store
            .as_ref()
            .ok_or("recovery_unavailable")?
            .discard(recovery_id);
        match result {
            Ok(()) => {
                state.recovery = RecoveryState::default();
                Ok(())
            }
            Err(error) => {
                state.recovery.error = Some(bounded(error.clone()));
                Err(error)
            }
        }
    }
}
