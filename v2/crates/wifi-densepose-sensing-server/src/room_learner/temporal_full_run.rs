//! Fresh-genesis full-run recovery DATA and private owner custody.
//! No public constructor, migration, HeldOut producer or qualification authority.
use super::*;
use crate::room_learner::artifact::ArtifactStore;
use crate::room_learner::temporal_acquisition::{ReceiverCadenceBand, WindowInputPolicy};
use std::sync::Arc;

pub(crate) const PREFIX_SCHEMA: &str = "ruview.room-learner.temporal-runtime-prefix.v3";
pub(crate) const META_KIND: &str = "temporal-prefix-commitment-v2";
pub(crate) const POLICY_DOMAIN: &str = "ruview.room-learner.temporal-full-run-policy.v1";
pub(crate) const FIT_RECORD_DOMAIN: &str = "ruview.room-learner.temporal-verified-fit-record.v1";
pub(crate) const MODEL_KIND: &str = "ruview.room-learner.managed-temporal-input.v3";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyBody {
    prefix_schema: String,
    metadata_kind: String,
    model_artifact_kind: String,
    context_digest: String,
    input_policy_sha256: String,
    execution_policy_sha256: String,
    schedule_sha256: String,
    configured_fit_sha256: String,
    native_profile: WindowInputPolicy,
    train_episodes: usize,
    held_out_episodes: usize,
    train_windows: usize,
    held_out_windows: usize,
    total_windows: usize,
    max_total_windows: usize,
    max_numeric_values: usize,
    max_recovery_bytes: usize,
    max_global_gaps: usize,
    max_quality_parts: usize,
    transition_limit: usize,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ContinuationPolicy {
    schema: String,
    body: PolicyBody,
    sha256: String,
}
impl ContinuationPolicy {
    pub(super) fn build(binding: &ManagedBinding) -> Result<Self, String> {
        let schedule = binding.contract()?;
        let c = schedule.counts();
        let context = schedule.context();
        if context.sources.len() != 3
            || context.sources.iter().any(|s| s.feature_count != 56)
            || binding.native.target_span_ms() != 2000
            || binding.native.source_policies().len() != 3
            || binding
                .native
                .source_policies()
                .iter()
                .any(|s| s.receiver_cadence != ReceiverCadenceBand::Hz30To36 || s.max_rows != 77)
            || (
                c.train_episodes,
                c.held_out_episodes,
                c.train_windows,
                c.held_out_windows,
                c.total_windows,
            ) != (7, 8, 64, 64, 128)
            || schedule.resources().max_numeric_values != 1_655_808
            || schedule.resources().max_numeric_values > 2_000_000
        {
            return Err("temporal_full_run_engineering_profile_unsupported".into());
        }
        let episodes = c
            .train_episodes
            .checked_add(c.held_out_episodes)
            .ok_or("temporal_full_run_capacity")?;
        let segments = episodes
            .checked_add(128)
            .ok_or("temporal_full_run_capacity")?;
        // STATES, revision0..432: genesis; each initial/resumed segment; at
        // most one accepted pause per segment (including terminal/deferred
        // pauses); windows; separate confirmed seals; Frozen; reserved final
        // completion; retirement. Recertification/SAME-intent retry add zero.
        let transition_limit = 1usize
            .checked_add(segments)
            .and_then(|n| n.checked_add(segments))
            .and_then(|n| n.checked_add(c.total_windows))
            .and_then(|n| n.checked_add(episodes))
            .and_then(|n| n.checked_add(3))
            .ok_or("temporal_full_run_capacity")?;
        if transition_limit != 433 || transition_limit > 512 {
            return Err("temporal_full_run_transition_capacity".into());
        }
        let body = PolicyBody {
            prefix_schema: PREFIX_SCHEMA.into(),
            metadata_kind: META_KIND.into(),
            model_artifact_kind: MODEL_KIND.into(),
            context_digest: context.digest(),
            input_policy_sha256: hash(binding)?,
            execution_policy_sha256: hash(&EXECUTION_POLICY)?,
            schedule_sha256: schedule.sha256().into(),
            configured_fit_sha256: hash(&binding.fit)?,
            native_profile: binding.native.clone(),
            train_episodes: c.train_episodes,
            held_out_episodes: c.held_out_episodes,
            train_windows: c.train_windows,
            held_out_windows: c.held_out_windows,
            total_windows: c.total_windows,
            max_total_windows: 128,
            max_numeric_values: 2_000_000,
            max_recovery_bytes: crate::room_learner::recovery_store::MAX_RECOVERY_BYTES,
            max_global_gaps: 128,
            max_quality_parts: segments,
            transition_limit,
        };
        let sha256 = hash(&(POLICY_DOMAIN, &body))?;
        Ok(Self {
            schema: POLICY_DOMAIN.into(),
            body,
            sha256,
        })
    }
    pub(super) fn validate(&self, binding: &ManagedBinding) -> Result<(), String> {
        let expected = Self::build(binding)?;
        if serde_json::to_vec(self).map_err(|_| "temporal_full_run_policy_encoding")?
            != serde_json::to_vec(&expected).map_err(|_| "temporal_full_run_policy_encoding")?
        {
            return Err("temporal_full_run_policy_changed".into());
        }
        Ok(())
    }
    pub(super) fn sha256(&self) -> &str {
        &self.sha256
    }
    pub(super) fn transition_limit(&self) -> usize {
        self.body.transition_limit
    }
}

/// Non-serde custody is never reconstructed by a raw prefix decoder.
pub(super) struct FullRunOwnerCustody {
    runtime_epoch: String,
    generation: u64,
}
pub(super) struct FullRunState {
    pub policy: ContinuationPolicy,
    pub owner: Option<FullRunOwnerCustody>,
    // Only DATA after raw decode. Verified session installation is separate.
    pub frozen: Option<VerifiedFitRecordData>,
    pub local_start_not_before_ns: Option<u64>,
    pending: Option<PendingFrozenTransition>,
    verified: Option<VerifiedFrozenSession>,
}
impl FullRunState {
    pub(super) fn fresh(
        binding: &ManagedBinding,
        run: &str,
        witness: crate::room_learner::lifecycle::FullRunGenesisWitness,
    ) -> Result<Self, String> {
        let (epoch, generation) = witness.consume(binding.native.context(), run)?;
        Ok(Self {
            policy: ContinuationPolicy::build(binding)?,
            owner: Some(FullRunOwnerCustody {
                runtime_epoch: epoch,
                generation,
            }),
            frozen: None,
            local_start_not_before_ns: None,
            pending: None,
            verified: None,
        })
    }
    pub(super) fn structural(
        policy: ContinuationPolicy,
        frozen: Option<VerifiedFitRecordData>,
    ) -> Self {
        Self {
            policy,
            owner: None,
            frozen,
            local_start_not_before_ns: None,
            pending: None,
            verified: None,
        }
    }
    pub(super) fn snapshot_permit(
        &self,
        expected: Option<TemporalToken>,
    ) -> Result<FullRunSnapshotPermit, String> {
        let owner = self
            .owner
            .as_ref()
            .ok_or("temporal_full_run_owner_custody_required")?;
        if self.frozen.is_some() && self.pending.is_none() {
            return Err("temporal_frozen_write_requires_current_producer".into());
        }
        Ok(FullRunSnapshotPermit {
            runtime_epoch: owner.runtime_epoch.clone(),
            generation: owner.generation,
            expected,
            model: self.pending.as_ref().map(|p| p.model.clone()),
        })
    }
}
/// Producer-only snapshot permission; serialized DATA never contains this.
pub(super) struct FullRunSnapshotPermit {
    runtime_epoch: String,
    generation: u64,
    expected: Option<TemporalToken>,
    model: Option<FrozenModelVerifier>,
}
#[derive(Clone)]
pub(crate) struct FullRunWriteGrant {
    payload_sha256: String,
    expected: Option<TemporalToken>,
    runtime_epoch: String,
    generation: u64,
    model: Option<FrozenModelVerifier>,
}
impl FullRunSnapshotPermit {
    pub(super) fn encoded(self, bytes: &[u8]) -> FullRunWriteGrant {
        FullRunWriteGrant {
            payload_sha256: bytes_hash(bytes),
            expected: self.expected,
            runtime_epoch: self.runtime_epoch,
            generation: self.generation,
            model: self.model,
        }
    }
}
impl FullRunWriteGrant {
    pub(crate) fn permits(&self, bytes: &[u8], expected: Option<&TemporalToken>) -> bool {
        !self.runtime_epoch.is_empty()
            && self.generation > 0
            && self.expected.as_ref() == expected
            && self.payload_sha256 == bytes_hash(bytes)
    }
    pub(crate) fn verify_frozen_model(&self) -> Result<(), String> {
        match &self.model {
            Some(m) => m.verify().map(|_| ()),
            None => Ok(()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum FitSummaryReference {
    Recorded { sha256: String },
    NotRecorded,
    Unavailable,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct VerifiedFitRecordBody {
    pub owner_scope: String,
    pub installation_id: String,
    pub context_digest: String,
    pub run: String,
    pub prefix_schema: String,
    pub metadata_kind: String,
    pub continuation_policy_sha256: String,
    pub artifact_kind: String,
    pub artifact_sha256: String,
    pub input_policy_sha256: String,
    pub execution_policy_sha256: String,
    pub schedule_sha256: String,
    pub configured_fit_sha256: String,
    pub training_data_sha256: String,
    pub input_evidence_sha256: String,
    pub identity_evidence_sha256: String,
    pub train_seal_chain_sha256: String,
    pub accepted_train_prefix_sha256: String,
    pub accepted_train_commitment:
        crate::room_learner::recovery_store::temporal_commitment::TemporalCommitmentData,
    pub saved_candidate: crate::room_learner::artifact::TemporalCandidateReceipt,
    pub actual_summary: FitSummaryReference,
    pub freeze_prepare_receiver_ns: u64,
    pub source_floors: Vec<(u8, u64, u64)>,
}
/// Serializable DATA, not a permit. SHA binds data, never producer authority.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct VerifiedFitRecordData {
    pub schema: String,
    pub body: VerifiedFitRecordBody,
    pub sha256: String,
}
impl VerifiedFitRecordData {
    pub(super) fn validate_data(
        &self,
        binding: &ManagedBinding,
        policy: &ContinuationPolicy,
    ) -> Result<(), String> {
        policy.validate(binding)?;
        let b = &self.body;
        let p = &policy.body;
        let c = binding.native.context();
        let sha = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|x| x.is_ascii_digit() || (b'a'..=b'f').contains(&x))
        };
        if self.schema != FIT_RECORD_DOMAIN
            || self.sha256 != hash(&(FIT_RECORD_DOMAIN, &self.body))?
            || b.owner_scope != c.owner_scope
            || b.installation_id != c.installation_id
            || b.context_digest != c.digest()
            || b.run.is_empty()
            || b.run.len() > 256
            || !b.run.is_ascii()
            || b.prefix_schema != PREFIX_SCHEMA
            || b.metadata_kind != META_KIND
            || b.continuation_policy_sha256 != policy.sha256
            || b.artifact_kind != MODEL_KIND
            || b.input_policy_sha256 != p.input_policy_sha256
            || b.execution_policy_sha256 != p.execution_policy_sha256
            || b.schedule_sha256 != p.schedule_sha256
            || b.configured_fit_sha256 != p.configured_fit_sha256
            || [
                &b.artifact_sha256,
                &b.training_data_sha256,
                &b.input_evidence_sha256,
                &b.identity_evidence_sha256,
                &b.train_seal_chain_sha256,
                &b.accepted_train_prefix_sha256,
            ]
            .iter()
            .any(|s| !sha(s))
            || b.freeze_prepare_receiver_ns == 0
            || b.source_floors.len() != c.sources.len()
            || b.saved_candidate.artifact_kind != MODEL_KIND
            || b.saved_candidate.artifact_sha256 != b.artifact_sha256
            || b.saved_candidate.schema != "ruview.room-learner.temporal-candidate.v1"
            || b.saved_candidate.owner_scope != b.owner_scope
            || b.saved_candidate.installation_id != b.installation_id
            || b.saved_candidate.context_digest != b.context_digest
            || b.saved_candidate.room_digest != c.room_digest
            || b.saved_candidate.input_policy_sha256 != b.input_policy_sha256
            || !(1..=512).contains(&b.saved_candidate.parameter_count)
            || !b.saved_candidate.diagnostic_only
            || b.saved_candidate.qualification_granted
            || b.saved_candidate.physically_validated
        {
            return Err("temporal_verified_fit_record_binding".into());
        }
        if matches!(&b.actual_summary,FitSummaryReference::Recorded{sha256}if !sha(sha256)) {
            return Err("temporal_verified_fit_summary_binding".into());
        }
        Ok(())
    }
}

#[derive(Clone)]
struct FrozenModelVerifier {
    store: Arc<ArtifactStore>,
    record: VerifiedFitRecordData,
    context: CaptureContext,
    actual: Arc<TemporalModel>,
}
impl FrozenModelVerifier {
    fn verify(&self) -> Result<Arc<TemporalModel>, String> {
        let loaded = self
            .store
            .load_exact_temporal_candidate(&self.context, &self.record.body.artifact_sha256)?;
        verify_model_binding(&self.record, &loaded)?;
        Ok(Arc::new(loaded))
    }
}
fn verify_model_binding(
    record: &VerifiedFitRecordData,
    model: &TemporalModel,
) -> Result<(), String> {
    let b = model.fit_diagnostic_binding()?;
    let r = &record.body;
    if b.artifact_kind != MODEL_KIND
        || b.artifact_sha256 != r.artifact_sha256
        || b.context_digest != r.context_digest
        || b.input_policy_sha256 != r.input_policy_sha256
        || b.execution_policy_sha256.as_ref() != Some(&r.execution_policy_sha256)
        || b.schedule_sha256.as_ref() != Some(&r.schedule_sha256)
        || b.training_data_sha256 != r.training_data_sha256
        || b.input_evidence_sha256 != r.input_evidence_sha256
        || b.identity_evidence_sha256.as_ref() != Some(&r.identity_evidence_sha256)
        || b.bound_fit_config.as_ref().map(hash).transpose()?.as_ref()
            != Some(&r.configured_fit_sha256)
        || b.parameter_count != r.saved_candidate.parameter_count
    {
        return Err("temporal_frozen_exact_model_kind_or_binding".into());
    }
    Ok(())
}
/// One current owner operation, separate from a completed fitting Job.cancel.
pub(crate) struct PendingFrozenTransition {
    operation: (String, u64, u64),
    record: VerifiedFitRecordData,
    model: FrozenModelVerifier,
    rows: Vec<SharedRecord<CaptureCohort>>,
    evidence: Vec<SharedRecord<WindowEvidence>>,
}
/// Neither raw DATA nor latest model pointers can construct this install value.
pub(crate) struct DurableFrozenTransition {
    install: VerifiedFrozenSession,
    admitted_at_ns: u64,
}
pub(crate) struct RecoveredVerifiedFrozen {
    install: VerifiedFrozenSession,
    recertified_at_ns: u64,
}
struct VerifiedFrozenSession {
    model: Arc<TemporalModel>,
    record: VerifiedFitRecordData,
    rows: Vec<SharedRecord<CaptureCohort>>,
    evidence: Vec<SharedRecord<WindowEvidence>>,
}
impl VerifiedFrozenSession {
    fn check(&self, session: &TemporalTrainingSession) -> Result<(), String> {
        if session.active
            || session.failed
            || session.fitted
            || session.rows.len() != 64
            || session.evidence.len() != 64
            || session
                .rows
                .iter()
                .zip(&self.rows)
                .any(|(a, b)| !a.same_record(b))
            || session
                .evidence
                .iter()
                .zip(&self.evidence)
                .any(|(a, b)| !a.same_record(b))
            || session.plan_sha256() != self.record.body.input_policy_sha256
            || session.run_id != self.record.body.run
            || session.binding.context.digest() != self.record.body.context_digest
            || hash(&session.config)? != self.record.body.configured_fit_sha256
        {
            return Err("temporal_verified_frozen_session_changed".into());
        }
        Ok(())
    }
    fn apply(&self, session: &mut TemporalTrainingSession) {
        session.managed.as_mut().unwrap().frozen_sha256 =
            Some(self.record.body.artifact_sha256.clone());
        session.fitted = true;
    }
}
impl FullRunState {
    pub(super) fn is_frozen(&self) -> bool {
        self.verified.is_some()
    }
    pub(super) fn is_freezing(&self) -> bool {
        self.pending.is_some()
    }
    pub(super) fn pending_record(&self) -> Option<&VerifiedFitRecordData> {
        self.pending.as_ref().map(|p| &p.record)
    }
    pub(super) fn verified_model(&self) -> Option<Arc<TemporalModel>> {
        self.verified.as_ref().map(|v| v.model.clone())
    }
}
/// The ONLY cold Frozen authority producer calls the real CURRENT store and
/// recertifies its barriers itself; no raw decoded object is accepted as input.
pub(super) fn recover_current(
    store: &RecoveryStore,
    artifact: Arc<ArtifactStore>,
    expected: &CaptureContext,
    now: u64,
    epoch: &str,
    anchor: u64,
    witness: crate::room_learner::lifecycle::FullRunRecoveryWitness,
) -> Result<TemporalWorkflow, String> {
    let mut w = TemporalWorkflow::cold_restore(store, expected, now, epoch, anchor)?;
    let (owner_epoch, generation) = witness.consume(expected)?;
    let mode = w
        .full_run
        .as_ref()
        .ok_or("temporal_full_run_no_upconversion")?;
    mode.policy.validate(
        &w.session
            .managed
            .as_ref()
            .ok_or("managed_session_required")?
            .binding,
    )?;
    let record = mode.frozen.clone();
    let verified = if let Some(record) = record {
        if now < record.body.freeze_prepare_receiver_ns {
            return Err("temporal_frozen_recovery_clock_reversed".into());
        }
        let model = Arc::new(
            artifact.load_exact_temporal_candidate(expected, &record.body.artifact_sha256)?,
        );
        verify_model_binding(&record, &model)?;
        let install = VerifiedFrozenSession {
            model,
            record,
            rows: w.session.rows.clone(),
            evidence: w.session.evidence.clone(),
        };
        install.check(&w.session)?;
        let recovered = RecoveredVerifiedFrozen {
            install,
            recertified_at_ns: now,
        };
        recovered.install.apply(&mut w.session);
        Some(recovered)
    } else {
        None
    };
    let mode = w.full_run.as_mut().unwrap();
    mode.owner = Some(FullRunOwnerCustody {
        runtime_epoch: owner_epoch,
        generation,
    });
    if let Some(recovered) = verified {
        mode.local_start_not_before_ns = Some(recovered.recertified_at_ns);
        mode.verified = Some(recovered.install);
    }
    Ok(w)
}
impl TemporalWorkflow {
    pub(super) fn request_frozen(
        &mut self,
        accepted: crate::room_learner::lifecycle::AcceptedTemporalFit,
        witness: crate::room_learner::lifecycle::FrozenOwnerWitness,
        store: Arc<ArtifactStore>,
        now: u64,
    ) -> Result<(), String> {
        let identity = fit_admission::current_identity(self, &self.schedule()?, now)?;
        let mode = self
            .full_run
            .as_ref()
            .ok_or("temporal_full_run_no_upconversion")?;
        let owner = mode
            .owner
            .as_ref()
            .ok_or("temporal_full_run_owner_custody_required")?;
        if mode.pending.is_some() || mode.frozen.is_some() || mode.verified.is_some() {
            return Err("temporal_frozen_operation_already_present".into());
        }
        let operation = witness.consume(self.context(), &self.session.run_id)?;
        if operation.0 != owner.runtime_epoch || operation.1 != owner.generation {
            return Err("temporal_frozen_owner_changed".into());
        }
        let (model, source) =
            accepted.consume_for_owner(&owner.runtime_epoch, owner.generation, &identity)?;
        let record = source.record_data(now, self.source_high.clone())?;
        record.validate_data(
            &self.session.managed.as_ref().unwrap().binding,
            &mode.policy,
        )?;
        if model.context() != self.context() {
            return Err("temporal_frozen_exact_model_context".into());
        }
        let verifier = FrozenModelVerifier {
            store,
            record: record.clone(),
            context: self.context().clone(),
            actual: model,
        };
        let pending = PendingFrozenTransition {
            operation,
            record: record.clone(),
            model: verifier,
            rows: self.session.rows.clone(),
            evidence: self.session.evidence.clone(),
        };
        self.check_clock(now)?;
        self.descriptor.saved_at_ms = now / 1_000_000;
        let mode = self.full_run.as_mut().unwrap();
        mode.pending = Some(pending);
        mode.frozen = Some(record);
        self.submit_async_commit(false)
    }
    pub(super) fn prepare_frozen_ack(
        &self,
        candidate: &TemporalWorkflow,
        now: u64,
    ) -> Result<Option<DurableFrozenTransition>, String> {
        let Some(mode) = &self.full_run else {
            return Ok(None);
        };
        let Some(pending) = &mode.pending else {
            return Ok(None);
        };
        let owner = mode
            .owner
            .as_ref()
            .ok_or("temporal_full_run_owner_custody_required")?;
        let data = candidate
            .full_run
            .as_ref()
            .and_then(|m| m.frozen.as_ref())
            .ok_or("temporal_frozen_ack_record_missing")?;
        if data != &pending.record
            || pending.operation.0 != owner.runtime_epoch
            || pending.operation.1 != owner.generation
            || now < data.body.freeze_prepare_receiver_ns
            || now < self.last_now_ns
            || now / 1_000_000 >= self.descriptor.expires_at_ms
            || self.source_high.iter().any(|(id, seq, time)| {
                data.body
                    .source_floors
                    .iter()
                    .any(|(s, q, t)| s == id && (seq < q || time < t))
            })
        {
            return Err("temporal_frozen_ack_owner_cut_or_record".into());
        }
        let install = VerifiedFrozenSession {
            model: pending.model.actual.clone(),
            record: data.clone(),
            rows: pending.rows.clone(),
            evidence: pending.evidence.clone(),
        };
        // Model I/O must be done on the writer, not here. The exact same model
        // Arc is carried by the pending capability after writer readback.
        install.check(&self.session)?;
        Ok(Some(DurableFrozenTransition {
            install,
            admitted_at_ns: now,
        }))
    }
    pub(super) fn apply_frozen_ack(&mut self, transition: DurableFrozenTransition) {
        transition.install.apply(&mut self.session);
        let mode = self.full_run.as_mut().unwrap();
        mode.pending = None;
        mode.local_start_not_before_ns = Some(transition.admitted_at_ns);
        mode.verified = Some(transition.install);
        self.stage = Stage::DurableReady;
    }
}
