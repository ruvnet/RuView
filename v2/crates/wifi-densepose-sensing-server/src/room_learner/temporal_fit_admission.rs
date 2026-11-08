//! Private producer-owned permission to COMPUTE one diagnostic candidate.
//! Existing durable Train seals are not physical/scientific qualification.
use super::*;
use std::sync::Arc;

/// Fixed binding to the capture's accepted prefix, not a wire proof or a
/// caller-supplied permission. Only this producer module can construct it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TemporalFitCaptureIdentity {
    owner: String,
    installation: String,
    context: String,
    run: String,
    schedule: String,
    policy: String,
    configuration: String,
    descriptor: RecoveryDescriptor,
    token: TemporalToken,
    prefix: String,
}
impl TemporalFitCaptureIdentity {
    pub(crate) fn owner(&self) -> &str {
        &self.owner
    }
    pub(crate) fn installation(&self) -> &str {
        &self.installation
    }
    pub(crate) fn context(&self) -> &str {
        &self.context
    }
    pub(crate) fn expires_at_ms(&self) -> u64 {
        self.descriptor.expires_at_ms
    }
    pub(crate) fn check_deadline(&self, now: u64) -> Result<(), String> {
        if now == 0 || now / 1_000_000 >= self.descriptor.expires_at_ms {
            Err("temporal_fit_original_deadline_expired".into())
        } else {
            Ok(())
        }
    }
    /// Off-lock only. Same-store observation; never publishes or renews TTL.
    pub(crate) fn recertify_current(&self, store: &RecoveryStore, now: u64) -> Result<(), String> {
        self.check_deadline(now)?;
        let current = store.recertify_temporal(&self.token, now)?;
        if current.token != self.token
            || current.prefix.descriptor() != &self.descriptor
            || bytes_hash(current.prefix.bytes()) != self.prefix
        {
            return Err("temporal_fit_accepted_prefix_changed".into());
        }
        Ok(())
    }
}

/// A bounded ref snapshot of ACTUAL accepted Runtime data. No Clone or serde:
/// imported models, JSON claims and generic sessions cannot create this value.
pub(crate) struct PendingTemporalFitInput {
    identity: TemporalFitCaptureIdentity,
    accepted_bytes: Arc<Vec<u8>>,
    records: checkpoint::SharedRecords,
    not_before_ns: u64,
}
/// Constructed only after exact same-store recertification and strict decoding,
/// off the receiver lock. It owns a fit session, never the live capture owner.
pub(crate) struct ValidatedTemporalFitInput {
    identity: TemporalFitCaptureIdentity,
    session: TemporalTrainingSession,
}

fn complete_train(
    w: &TemporalWorkflow,
    schedule: &ManagedScheduleContract,
    now: u64,
) -> Result<(), String> {
    let bridge = w
        .bridge
        .as_ref()
        .ok_or("temporal_fit_actual_bridge_required")?;
    let managed = w
        .session
        .managed
        .as_ref()
        .ok_or("managed_session_required")?;
    let counts = schedule.counts();
    if now < w.last_now_ns || now / 1_000_000 >= w.descriptor.expires_at_ms {
        return Err("temporal_fit_original_deadline_or_clock".into());
    }
    if bridge.disposition != bridge::Disposition::Complete
        || bridge.seals.len() != counts.train_episodes
        || !bridge.parts.is_empty()
        || w.episode != counts.train_episodes
        || w.mapping.is_some()
        || w.pending_window.is_some()
        || w.session.active
        || w.session.failed
        || w.session.fitted
        || w.session.rows.len() != counts.train_windows
        || w.session.evidence.len() != counts.train_windows
        || managed.windows.len() != counts.train_windows
        || w.ledger.spans.len() != counts.train_windows
        || w.durable_windows != counts.train_windows
        || w.session.rows.iter().any(|row| row.role != DataRole::Train)
        || w.pending_intent.is_some()
        || w.pending_prefix.is_some()
        || w.stage == Stage::BlockedDurability
    {
        return Err("temporal_fit_confirmed_durable_train_required".into());
    }
    Ok(())
}
fn identity(
    w: &TemporalWorkflow,
    schedule: &ManagedScheduleContract,
    token: TemporalToken,
    prefix: String,
) -> Result<TemporalFitCaptureIdentity, String> {
    Ok(TemporalFitCaptureIdentity {
        owner: w.descriptor.owner_scope.clone(),
        installation: w.descriptor.installation_id.clone(),
        context: w.context().digest(),
        run: w.session.run_id.clone(),
        schedule: schedule.sha256().into(),
        policy: w.session.plan_sha256().into(),
        configuration: hash(&w.session.config)?,
        descriptor: w.descriptor.clone(),
        token,
        prefix,
    })
}
pub(super) fn current_identity(
    w: &TemporalWorkflow,
    schedule: &ManagedScheduleContract,
    now: u64,
) -> Result<TemporalFitCaptureIdentity, String> {
    complete_train(w, schedule, now)?;
    // The async writer supplies a cached verified digest. Never hash a growing
    // raw prefix under the receiver lock or silently fall back to a sync path.
    let state = w
        .async_state
        .as_ref()
        .ok_or("temporal_fit_async_durability_required")?;
    if state.pending.is_some()
        || state.busy_pause
        || state.error.is_some()
        || state.accepted_seals != schedule.counts().train_episodes
        || w.durable.is_none()
    {
        return Err("temporal_fit_durability_unconfirmed".into());
    }
    identity(
        w,
        schedule,
        w.commit_token
            .clone()
            .ok_or("temporal_fit_commitment_required")?,
        w.durable_digest().ok_or("temporal_fit_digest_required")?,
    )
}
impl PendingTemporalFitInput {
    pub(super) fn capture(
        w: &TemporalWorkflow,
        schedule: &ManagedScheduleContract,
        now: u64,
    ) -> Result<Self, String> {
        let identity = current_identity(w, schedule, now)?;
        Ok(Self {
            identity,
            accepted_bytes: w
                .durable
                .as_ref()
                .ok_or("temporal_fit_prefix_required")?
                .clone(),
            records: checkpoint::workflow_records(w),
            not_before_ns: now,
        })
    }
    pub(crate) fn identity(&self) -> &TemporalFitCaptureIdentity {
        &self.identity
    }
    pub(crate) fn validate(
        self,
        store: &RecoveryStore,
        now: u64,
    ) -> Result<ValidatedTemporalFitInput, String> {
        self.identity.check_deadline(now)?;
        if now < self.not_before_ns {
            return Err("temporal_fit_receiver_clock_reversed".into());
        }
        let recertified = store.recertify_temporal(&self.identity.token, now)?;
        if recertified.token != self.identity.token
            || recertified.prefix.descriptor() != &self.identity.descriptor
            || bytes_hash(recertified.prefix.bytes()) != self.identity.prefix
            || recertified.prefix.bytes() != self.accepted_bytes.as_slice()
        {
            return Err("temporal_fit_accepted_prefix_changed".into());
        }
        // The existing decoder validates all raw rows, temporal identities,
        // source floors, native quality periods and the immutable Train seals.
        let restored = checkpoint::decode_shared(
            recertified.prefix.bytes(),
            recertified.prefix.descriptor(),
            now,
            &self.records,
        )?;
        let schedule = restored.schedule()?;
        complete_train(&restored, &schedule, now)?;
        if identity(
            &restored,
            &schedule,
            recertified.token,
            self.identity.prefix.clone(),
        )? != self.identity
        {
            return Err("temporal_fit_input_binding_changed".into());
        }
        Ok(ValidatedTemporalFitInput {
            identity: self.identity,
            session: restored.session,
        })
    }
}
impl ValidatedTemporalFitInput {
    /// SAME existing permit/hash formulas and SAME fit session implementation.
    /// Consumption prevents a cloned generic permit from being reused on a
    /// second fit job. A retry requires fresh owner-command admission.
    pub(crate) fn into_session_and_admission(
        self,
    ) -> Result<(TemporalTrainingSession, ManagedFitAdmission), String> {
        let managed = self
            .session
            .managed
            .as_ref()
            .ok_or("managed_session_required")?;
        let dataset = self.session.dataset()?;
        let permit = ManagedFitAdmission {
            policy: hash(&managed.binding)?,
            data: hash(&dataset.train())?,
            evidence: hash(&self.session.evidence)?,
            identities: hash(&managed.windows)?,
        };
        // Full validated data is intentionally dropped here, on the worker.
        drop(dataset);
        let _ = self.identity;
        Ok((self.session, permit))
    }
}
