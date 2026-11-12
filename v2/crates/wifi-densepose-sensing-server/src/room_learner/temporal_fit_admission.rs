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
    prefix_schema: &'static str,
    metadata_kind: &'static str,
    continuation_policy: Option<String>,
    operator_profile: Option<String>,
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
    pub(crate) fn run_id(&self)->&str{&self.run}
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
    train_seals_sha256:String,
}
/// Off-lock strict producer validation, later bound to an ACTUAL saved fit.
/// This is not serializable/importable, and has no public constructor.
pub(crate) struct VerifiedTemporalFitSource {
    identity:TemporalFitCaptureIdentity,
    train_data:String,evidence:String,identities:String,seals:String,
    actual_candidate:Option<crate::room_learner::artifact::TemporalCandidateReceipt>,
    actual_summary:full_run::FitSummaryReference,
}
impl VerifiedTemporalFitSource {
    pub(crate) fn is_full_run(&self)->bool{self.identity.continuation_policy.is_some()}
    pub(crate) fn binding(&self)->&TemporalFitCaptureIdentity{&self.identity}
    pub(crate) fn bind_saved_fit(&mut self,model:&TemporalModel,saved:&crate::room_learner::artifact::TemporalFitSaveResult)->Result<(),String>{
        let b=model.fit_diagnostic_binding()?;
        if b.artifact_kind!=full_run::MODEL_KIND||b.context_digest!=self.identity.context
            ||b.training_data_sha256!=self.train_data||b.input_evidence_sha256!=self.evidence
            ||b.identity_evidence_sha256.as_ref()!=Some(&self.identities)||b.schedule_sha256.as_ref()!=Some(&self.identity.schedule)
            ||b.input_policy_sha256!=self.identity.policy
            ||b.bound_fit_config.as_ref().map(hash).transpose()?.as_ref()!=Some(&self.identity.configuration)
            ||saved.candidate.artifact_sha256!=b.artifact_sha256||saved.candidate.artifact_kind!=b.artifact_kind{
            return Err("temporal_actual_fit_source_binding_changed".into());
        }
        self.actual_candidate=Some(saved.candidate.clone());
        self.actual_summary=match &saved.fit_diagnostics.summary{
            Some(summary)=>full_run::FitSummaryReference::Recorded{sha256:hash(summary)?},
            None=>match saved.fit_diagnostics.state{
                crate::room_learner::artifact::FitDiagnosticState::NotRecorded=>full_run::FitSummaryReference::NotRecorded,
                _=>full_run::FitSummaryReference::Unavailable,
            }
        };
        Ok(())
    }
    pub(super) fn record_data(self,now:u64,floors:Vec<(u8,u64,u64)>)->Result<full_run::VerifiedFitRecordData,String>{
        self.identity.check_deadline(now)?;
        let candidate=self.actual_candidate.ok_or("temporal_actual_saved_fit_required")?;
        let continuation=self.identity.continuation_policy.ok_or("temporal_full_run_no_upconversion")?;
        let body=full_run::VerifiedFitRecordBody{
            owner_scope:self.identity.owner,installation_id:self.identity.installation,context_digest:self.identity.context,run:self.identity.run,
            prefix_schema:self.identity.prefix_schema.into(),metadata_kind:self.identity.metadata_kind.into(),continuation_policy_sha256:continuation,
            artifact_kind:candidate.artifact_kind.clone(),artifact_sha256:candidate.artifact_sha256.clone(),
            input_policy_sha256:self.identity.policy,execution_policy_sha256:hash(&EXECUTION_POLICY)?,schedule_sha256:self.identity.schedule,
            configured_fit_sha256:self.identity.configuration,training_data_sha256:self.train_data,input_evidence_sha256:self.evidence,
            identity_evidence_sha256:self.identities,train_seal_chain_sha256:self.seals,accepted_train_prefix_sha256:self.identity.prefix,
            accepted_train_commitment:self.identity.token.data(),saved_candidate:candidate,actual_summary:self.actual_summary,
            freeze_prepare_receiver_ns:now,source_floors:floors,operator_profile:self.identity.operator_profile,
        };
        let sha256=hash(&(full_run::FIT_RECORD_DOMAIN,&body))?;
        Ok(full_run::VerifiedFitRecordData{schema:full_run::FIT_RECORD_DOMAIN.into(),body,sha256})
    }
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
    let prefit_episodes=if schedule.is_operator_eight_step(){schedule.episodes().len()}else{counts.train_episodes};
    let prefit_windows=if schedule.is_operator_eight_step(){counts.total_windows}else{counts.train_windows};
    if now < w.last_now_ns || now / 1_000_000 >= w.descriptor.expires_at_ms {
        return Err("temporal_fit_original_deadline_or_clock".into());
    }
    if bridge.disposition != bridge::Disposition::Complete
        || bridge.seals.len() != prefit_episodes
        || !bridge.parts.is_empty()
        || w.episode != prefit_episodes
        || w.mapping.is_some()
        || w.pending_window.is_some()
        || w.session.active
        || w.session.failed
        || w.session.fitted
        || w.session.rows.len() != prefit_windows
        || w.session.evidence.len() != prefit_windows
        || managed.windows.len() != prefit_windows
        || w.ledger.spans.len() != prefit_windows
        || w.durable_windows != prefit_windows
        || w.session.rows.iter().any(|row| row.role != DataRole::Train && !(schedule.is_operator_eight_step() && row.role == DataRole::Validation))
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
        prefix_schema:if w.full_run.is_some(){full_run::PREFIX_SCHEMA}else{"ruview.room-learner.temporal-runtime-prefix.v2"},
        metadata_kind:if w.full_run.is_some(){full_run::META_KIND}else{"temporal-prefix-commitment-v1"},
        continuation_policy:w.full_run.as_ref().map(|m|m.policy.sha256().into()),
        operator_profile:schedule.is_operator_eight_step().then(||schedule.profile_id().into()),
    })
}
pub(super) fn current_identity(
    w: &TemporalWorkflow,
    schedule: &ManagedScheduleContract,
    now: u64,
) -> Result<TemporalFitCaptureIdentity, String> {
    complete_train(w, schedule, now)?;
    if w.full_run.as_ref().is_some_and(|m|m.owner.is_none()){return Err("temporal_full_run_owner_custody_required".into());}
    // The async writer supplies a cached verified digest. Never hash a growing
    // raw prefix under the receiver lock or silently fall back to a sync path.
    let state = w
        .async_state
        .as_ref()
        .ok_or("temporal_fit_async_durability_required")?;
    if state.pending.is_some()
        || state.busy_pause
        || state.error.is_some()
        || state.accepted_seals != if schedule.is_operator_eight_step(){schedule.episodes().len()}else{schedule.counts().train_episodes}
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
            train_seals_sha256:hash(&restored.bridge.as_ref().ok_or("temporal_fit_actual_bridge_required")?.seals.iter().filter(|seal|schedule.episodes().get(seal.episode).is_some_and(|e|e.role==DataRole::Train)).collect::<Vec<_>>())?,
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
        let(session,permit,_)=self.into_session_admission_and_source()?;Ok((session,permit))
    }
    pub(crate) fn into_session_admission_and_source(mut self)->Result<(TemporalTrainingSession,ManagedFitAdmission,VerifiedTemporalFitSource),String>{
        self.session.retain_managed_train_only()?;
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
        // Strict seal validation happened in the recertified producer decoder;
        // recover its exact immutable chain from the accepted prefix again on
        // the worker, never hash growing source data under Runtime.inner.
        let source=VerifiedTemporalFitSource{identity:self.identity,train_data:permit.data.clone(),evidence:permit.evidence.clone(),
            identities:permit.identities.clone(),seals:self.train_seals_sha256,actual_candidate:None,actual_summary:full_run::FitSummaryReference::Unavailable};
        // Full validated data is intentionally dropped here, on the worker.
        drop(dataset);
        Ok((self.session, permit,source))
    }
}
