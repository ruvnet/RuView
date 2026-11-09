//! Managed-v3 INPUT integration inside the SAME opaque temporal model/session.
//! The private Runtime may issue fit-only admission from its exact durable Train
//! producer. Neither this INPUT codec nor its claims issue managed qualification.
use super::*;
use crate::room_learner::{
    managed_schedule_contract::{FrozenModelIdentity, ManagedScheduleContract},
    temporal_acquisition::{ManagedWindowIdentity, ManagedWindowTarget, WindowInputPolicy},
};
use std::sync::atomic::AtomicBool;
#[path = "temporal_execution.rs"]
pub(crate) mod execution;
pub(crate) const SCHEMA_V3: &str = "ruview.room-learner.managed-temporal-input.v3";
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LivePolicy {
    id: String,
    target_horizon_ms: u64,
    maximum_first_row_age_ns: u64,
    source_gap_policy: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManagedBinding {
    schedule_json: String,
    schedule_sha256: String,
    pub(super) native: WindowInputPolicy,
    encoder: EncoderConfig,
    fit: FitConfig,
    live: LivePolicy,
    target_mapping: String,
}
impl ManagedBinding {
    fn build(schedule: &ManagedScheduleContract, fit: FitConfig) -> Result<Self, String> {
        fit.validate_context(schedule.context())?;
        let encoder = EncoderConfig {
            bins: 56,
            source_count: schedule.context().sources.len(),
            hidden_dim: fit.hidden_dim,
            embedding_dim: fit.embedding_dim,
            temporal_tokens: fit.temporal_tokens,
            temperature: fit.temperature,
        };
        Ok(Self {
            schedule_json: String::from_utf8(schedule.export()?)
                .map_err(|_| "managed_binding_encoding")?,
            schedule_sha256: schedule.sha256().into(),
            native: WindowInputPolicy::from_managed(schedule),
            encoder,
            fit,
            live: LivePolicy {
                id: "nonoverlap-native-oldest-first-v1".into(),
                target_horizon_ms: 2000,
                maximum_first_row_age_ns: MAX_AGE_NS,
                source_gap_policy: "existing-native-profile-every-source-max-gap".into(),
            },
            target_mapping: execution::EXECUTION_POLICY.into(),
        })
    }
    fn contract(&self) -> Result<ManagedScheduleContract, String> {
        let schedule = ManagedScheduleContract::import(self.schedule_json.as_bytes())?;
        let expected = Self::build(&schedule, self.fit.clone())?;
        if serde_json::to_vec(self).map_err(|_| "managed_binding_encoding")?
            != serde_json::to_vec(&expected).map_err(|_| "managed_binding_encoding")?
        {
            return Err("managed_binding_policy_changed".into());
        }
        Ok(schedule)
    }
    pub(super) fn validation_binding(&self) -> Result<Binding, String> {
        self.contract()?;
        Ok(Binding {
            plan_id: "managed-v3-native-validation-only".into(),
            native_frame_identity: "existing-esp32-receiver-ns-node-u32-sequence-v1".into(),
            plan_sha256: hash(self)?,
            context: self.native.context().clone(),
            encoder: self.encoder.clone(),
            target_span_ms: self.native.target_span_ms(),
            max_span_ms: self.native.max_observed_span_ms(),
            max_pairwise_skew_ms: self.native.max_pairwise_receiver_skew_ms(),
            max_first_delay_ms: self.native.max_first_packet_delay_ms(),
            sources: self
                .native
                .source_policies()
                .iter()
                .map(|p| SourcePolicy {
                    source_id: p.source_id,
                    band: format!("{:?}", p.receiver_cadence),
                    min_rows: p.min_rows,
                    max_rows: p.max_rows,
                    min_interval_ns: p.min_packet_interval_ns,
                    max_gap_ns: p.max_gap_ns,
                })
                .collect(),
        })
    }
}
pub(super) struct ManagedSession {
    pub(super) binding: ManagedBinding,
    pub(super) windows: Vec<ManagedWindowIdentity>,
    frozen_sha256: Option<String>,
}
#[derive(Clone)]
pub(super) struct ManagedModel {
    pub(super) binding: ManagedBinding,
    windows: Vec<ManagedWindowIdentity>,
}
/// The private owner Runtime can obtain this only by consuming the producer's
/// exact confirmed/quality-checked/durable Train prefix. Permission to compute a
/// diagnostic candidate is not physical qualification or selection authority.
/// It cannot be uploaded/deserialized or synthesized from generic model metadata.
pub(crate) struct ManagedFitAdmission {
    policy: String,
    data: String,
    evidence: String,
    identities: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Claims {
    diagnostic_only: bool,
    managed_proof_issued: bool,
    whole_episode_quality_issued: bool,
    conditions_confirmed_by_runtime: bool,
}
impl Claims {
    fn diagnostic() -> Self {
        Self {
            diagnostic_only: true,
            managed_proof_issued: false,
            whole_episode_quality_issued: false,
            conditions_confirmed_by_runtime: false,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadV3 {
    binding: ManagedBinding,
    run_id: String,
    training_data_sha256: String,
    input_evidence_sha256: String,
    identity_evidence_sha256: String,
    evidence: Vec<WindowEvidence>,
    identities: Vec<ManagedWindowIdentity>,
    model_sha256: String,
    model_json: String,
    claims: Claims,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvelopeV3 {
    schema: String,
    payload_sha256: String,
    payload: PayloadV3,
}
fn identity(
    schedule: &ManagedScheduleContract,
    run: &str,
    record: &ManagedWindowIdentity,
) -> Result<ManagedWindowTarget, String> {
    let frozen = record
        .frozen_artifact_sha256
        .as_ref()
        .map(|sha| FrozenModelIdentity::from_runtime_verified_artifact(schedule, sha.clone()))
        .transpose()?;
    let bound = schedule.bind_episode_start(
        run,
        record.episode_index,
        record.attempt_ordinal,
        record.segment_ordinal,
        frozen.as_ref(),
    )?;
    if record.collection_started_ns != record.mapping.receiver_start_ns.to_string() {
        return Err("managed_segment_start_changed".into());
    }
    let expected = ManagedWindowTarget::from_observed_episode(
        schedule,
        &bound,
        run,
        record.slot_index,
        record.mapping.clone(),
    )?;
    if serde_json::to_vec(expected.identity()).map_err(|_| "managed_identity_encoding")?
        != serde_json::to_vec(record).map_err(|_| "managed_identity_encoding")?
    {
        return Err("managed_identity_changed".into());
    }
    Ok(expected)
}
fn expected_position(
    schedule: &ManagedScheduleContract,
    index: usize,
) -> Result<(usize, usize), String> {
    let mut cursor = 0;
    for episode in schedule.episodes() {
        if index < cursor + episode.slots.len() {
            return Ok((episode.index, index - cursor));
        }
        cursor += episode.slots.len();
    }
    Err("managed_window_schedule_exhausted".into())
}
fn validate_records(
    schedule: &ManagedScheduleContract,
    run: &str,
    evidence: &[impl std::borrow::Borrow<WindowEvidence>],
    records: &[ManagedWindowIdentity],
    binding: &Binding,
    train_only: bool,
) -> Result<(), String> {
    if evidence.len() != records.len() {
        return Err("managed_identity_count_changed".into());
    }
    let mut fences = Fences::default();
    let mut previous: Option<&ManagedWindowIdentity> = None;
    for (index, (e, r)) in evidence.iter().zip(records).enumerate() {
        let e = e.borrow();
        let position = expected_position(schedule, index)?;
        if (r.episode_index, r.slot_index) != position
            || (train_only && (r.role != DataRole::Train || r.frozen_artifact_sha256.is_some()))
        {
            return Err("managed_window_role_or_order".into());
        }
        let target = identity(schedule, run, r)?;
        if e.role != r.role
            || e.label != r.label
            || e.trial_id != r.trial_id
            || e.session_id != r.session_id
            || e.target_ns != r.target_ns
        {
            return Err("managed_cohort_identity_changed".into());
        }
        let expected_id = format!(
            "window-{:x}",
            Sha256::digest(serde_json::to_vec(r).map_err(|_| "managed_identity_encoding")?)
        );
        if e.cohort_id != expected_id || integer::<u64>(&e.sealed_at_ns)? >= target.end_ns() {
            return Err("managed_window_mapping_changed".into());
        }
        if let Some(old) = previous {
            if old.episode_index == r.episode_index {
                if old.attempt_id != r.attempt_id {
                    return Err("managed_attempt_changed".into());
                }
                if old.segment_ordinal == r.segment_ordinal {
                    if old.mapping != r.mapping || old.session_id != r.session_id {
                        return Err("managed_segment_changed".into());
                    }
                } else {
                    let last = schedule.episodes()[old.episode_index].slots[old.slot_index]
                        .deadline_offset_ms
                        * 1_000_000;
                    if r.segment_ordinal != old.segment_ordinal + 1
                        || r.mapping.observed_start_ns != last
                        || r.mapping.receiver_start_ns < integer::<u64>(&old.end_ns)?
                        || r.mapping.clock_anchor_ns < integer::<u64>(&old.end_ns)?
                    {
                        return Err("managed_segment_overlap_or_clock".into());
                    }
                }
            } else if r.mapping.observed_start_ns != 0
                || r.mapping.receiver_start_ns < integer::<u64>(&old.end_ns)?
            {
                return Err("managed_episode_time_overlap".into());
            }
        }
        e.validate_sources(binding)?;
        fences.add(e)?;
        previous = Some(r);
    }
    Ok(())
}
impl TemporalTrainingSession {
    pub(crate) fn bound_managed_fit_config(&self) -> Result<FitConfig, String> {
        let managed=self.managed.as_ref().ok_or("managed_session_required")?;
        managed.binding.contract()?;
        Ok(managed.binding.fit.clone())
    }
    pub fn managed(
        schedule: ManagedScheduleContract,
        run_id: String,
        config: FitConfig,
    ) -> Result<Self, String> {
        let binding = ManagedBinding::build(&schedule, config.clone())?;
        let policy = hash(&binding)?;
        // Only the unchanged legacy fields' native-validation carrier is built;
        // managed identity/hash/schedule NEVER comes from this seed.
        let bands = schedule
            .context()
            .sources
            .iter()
            .map(|s| (s.source_id, ReceiverCadenceBand::Hz30To36))
            .collect::<Vec<_>>();
        let plan = AcquisitionPlan::build(
            schedule.context().clone(),
            ExperimentalPlanId::Native2000msSlots8,
            &bands,
        )?;
        let assembler = NativeWindowAssembler::managed(schedule, policy, run_id.clone())?;
        Ok(Self {
            plan,
            binding: binding.validation_binding()?,
            run_id,
            assembler,
            rows: vec![],
            evidence: vec![],
            config,
            fitted: false,
            failed: false,
            active: false,
            target_ns: None,
            managed: Some(ManagedSession {
                binding,
                windows: vec![],
                frozen_sha256: None,
            }),
        })
    }
    pub fn begin_managed_window(&mut self, target: &ManagedWindowTarget) -> Result<(), String> {
        let managed = self.managed.as_ref().ok_or("managed_session_required")?;
        let r = target.identity();
        if self.failed || self.active {
            return Err("temporal_session_failed_or_active".into());
        }
        if r.role == DataRole::HeldOut
            && (!self.fitted || r.frozen_artifact_sha256.as_ref() != managed.frozen_sha256.as_ref())
        {
            return Err("managed_frozen_whole_model_required".into());
        }
        if r.role == DataRole::Train && self.fitted {
            return Err("managed_training_is_frozen".into());
        }
        self.assembler.begin_managed_window(target)?;
        self.active = true;
        self.target_ns = Some(target.target_ns());
        Ok(())
    }
    pub(crate) fn fit_managed_with_cancel(
        &mut self,
        permit: ManagedFitAdmission,
        cancel: &AtomicBool,
    ) -> Result<(TemporalModel, TrainingReport), String> {
        let managed = self.managed.as_ref().ok_or("managed_session_required")?;
        let schedule = managed.binding.contract()?;
        if self.failed
            || self.active
            || self.fitted
            || self.rows.len() != schedule.counts().train_windows
            || self.rows.iter().any(|r| r.role != DataRole::Train)
        {
            return Err("managed_complete_train_schedule_required".into());
        }
        validate_records(
            &schedule,
            &self.run_id,
            &self.evidence,
            &managed.windows,
            &self.binding,
            true,
        )?;
        let dataset = ValidatedDataset::new(schedule.context().clone(), owned_records(&self.rows))?;
        if permit.policy != hash(&managed.binding)?
            || permit.data != hash(&dataset.train())?
            || permit.evidence != hash(&self.evidence)?
            || permit.identities != hash(&managed.windows)?
        {
            return Err("managed_fit_admission_mismatch".into());
        }
        let (model, report) = RoomModel::fit_with_cancel(&dataset, self.config.clone(), cancel)?;
        if model.encoder_config() != &managed.binding.encoder {
            return Err("managed_encoder_changed".into());
        }
        let result = TemporalModel {
            binding: self.binding.clone(),
            run_id: self.run_id.clone(),
            model,
            evidence: owned_records(&self.evidence),
            managed: Some(ManagedModel {
                binding: managed.binding.clone(),
                windows: managed.windows.clone(),
            }),
        };
        result.validate_managed()?;
        let frozen = bytes_hash(&result.export()?);
        self.managed.as_mut().unwrap().frozen_sha256 = Some(frozen);
        self.fitted = true;
        Ok((result, report))
    }
    #[cfg(test)]
    fn synthetic_test_fit_admission(&self) -> Result<ManagedFitAdmission, String> {
        let managed = self.managed.as_ref().ok_or("managed_session_required")?;
        let dataset = self.dataset()?;
        Ok(ManagedFitAdmission {
            policy: hash(&managed.binding)?,
            data: hash(&dataset.train())?,
            evidence: hash(&self.evidence)?,
            identities: hash(&managed.windows)?,
        })
    }
}
impl TemporalModel {
    pub(super) fn validate_managed(&self) -> Result<ManagedScheduleContract, String> {
        let m = self.managed.as_ref().ok_or("managed_model_required")?;
        let schedule = m.binding.contract()?;
        self.model.validate()?;
        if self.model.context() != schedule.context()
            || self.model.encoder_config() != &m.binding.encoder
            || self.model.training_manifest().training_cohorts != schedule.counts().train_windows
            || self.evidence.len() != schedule.counts().train_windows
            || self.model.training_manifest().measurement_clock_available
            || self.model.training_manifest().clock_claims != [ClockKind::ReceiverOnly]
        {
            return Err("managed_model_binding_changed".into());
        }
        let expected = m.binding.validation_binding()?;
        if serde_json::to_vec(&self.binding).map_err(|_| "managed_binding_encoding")?
            != serde_json::to_vec(&expected).map_err(|_| "managed_binding_encoding")?
        {
            return Err("managed_native_binding_changed".into());
        }
        let mut provenance = self
            .evidence
            .iter()
            .map(|e| e.provenance)
            .collect::<Vec<_>>();
        provenance.sort_by_key(|p| match p {
            CaptureProvenance::Synthetic => 0,
            CaptureProvenance::CapturedReplay => 1,
            CaptureProvenance::LiveReceiverObserved => 2,
        });
        provenance.dedup();
        if provenance != self.model.training_manifest().training_provenance {
            return Err("managed_provenance_changed".into());
        }
        validate_records(
            &schedule,
            &self.run_id,
            &self.evidence,
            &m.windows,
            &self.binding,
            true,
        )?;
        Ok(schedule)
    }
    pub(super) fn managed_fit_diagnostic_policy(&self)->Result<(String,String,String,FitConfig),String>{
        self.validate_managed()?;
        let model=self.managed.as_ref().ok_or("managed_model_required")?;
        Ok((model.binding.schedule_sha256.clone(),hash(&model.binding.target_mapping)?,hash(&model.windows)?,model.binding.fit.clone()))
    }
    pub(super) fn export_managed(&self) -> Result<Vec<u8>, String> {
        self.validate_managed()?;
        let managed = self.managed.as_ref().unwrap();
        let bytes = self.model.export()?;
        let payload = PayloadV3 {
            binding: managed.binding.clone(),
            run_id: self.run_id.clone(),
            training_data_sha256: self.model.training_manifest().training_data_sha256.clone(),
            input_evidence_sha256: hash(&self.evidence)?,
            identity_evidence_sha256: hash(&managed.windows)?,
            evidence: self.evidence.clone(),
            identities: managed.windows.clone(),
            model_sha256: bytes_hash(&bytes),
            model_json: String::from_utf8(bytes).map_err(|_| "managed_model_encoding")?,
            claims: Claims::diagnostic(),
        };
        let e = EnvelopeV3 {
            schema: SCHEMA_V3.into(),
            payload_sha256: hash(&payload)?,
            payload,
        };
        let bytes = serde_json::to_vec(&e).map_err(|_| "managed_model_encoding")?;
        if bytes.len() > MAX_BYTES {
            return Err("temporal_input_envelope_budget".into());
        }
        Ok(bytes)
    }
    pub(super) fn import_managed(bytes: &[u8]) -> Result<Self, String> {
        let e: EnvelopeV3 = serde_json::from_slice(bytes).map_err(|_| "managed_model_schema")?;
        if e.schema != SCHEMA_V3
            || e.payload_sha256 != hash(&e.payload)?
            || serde_json::to_vec(&e).map_err(|_| "managed_model_encoding")? != bytes
        {
            return Err("managed_model_integrity".into());
        }
        let p = e.payload;
        if !p.claims.diagnostic_only
            || p.claims.managed_proof_issued
            || p.claims.whole_episode_quality_issued
            || p.claims.conditions_confirmed_by_runtime
        {
            return Err("managed_input_is_not_proof".into());
        }
        if p.input_evidence_sha256 != hash(&p.evidence)?
            || p.identity_evidence_sha256 != hash(&p.identities)?
            || p.model_sha256 != bytes_hash(p.model_json.as_bytes())
        {
            return Err("managed_evidence_hash_changed".into());
        }
        let model = RoomModel::import(p.model_json.as_bytes())?;
        if model.export()? != p.model_json.as_bytes()
            || model.training_manifest().training_data_sha256 != p.training_data_sha256
        {
            return Err("managed_training_hash_changed".into());
        }
        let binding = p.binding.validation_binding()?;
        let result = Self {
            binding,
            run_id: p.run_id,
            model,
            evidence: p.evidence,
            managed: Some(ManagedModel {
                binding: p.binding,
                windows: p.identities,
            }),
        };
        result.validate_managed()?;
        Ok(result)
    }
    pub(super) fn managed_live_input(&self, run_id: String) -> Result<TemporalLiveInput, String> {
        self.validate_managed()?;
        let m = self.managed.as_ref().unwrap();
        let mut fences = Fences::default();
        for e in &self.evidence {
            fences.add(e)?;
        }
        Ok(TemporalLiveInput {
            binding: self.binding.clone(),
            assembler: NativeWindowAssembler::inference(
                m.binding.native.clone(),
                hash(&m.binding)?,
                run_id,
            )?,
            fences,
            failed: false,
            target_ns: None,
        })
    }
    pub(super) fn predict_managed(
        &self,
        input: TemporalLiveObservation,
        now_ns: u64,
    ) -> Result<RoomPrediction, String> {
        self.validate_managed()?;
        if input.window.cohort.role != DataRole::Inference
            || input.window.cohort.label != RoomLabel::Unknown
            || input.window.cohort.teacher.is_some()
            || input.window.managed_identity.is_some()
        {
            return Err("temporal_live_must_be_unlabelled".into());
        }
        let e =
            WindowEvidence::from_binding(&self.binding, &input.window, now_ns, input.target_ns)?;
        let mut fences = Fences::default();
        for old in &self.evidence {
            fences.add(old)?;
        }
        fences.add(&e)?;
        if serde_json::to_vec(&input.binding).map_err(|_| "managed_binding_encoding")?
            != serde_json::to_vec(&self.binding).map_err(|_| "managed_binding_encoding")?
        {
            return Err("temporal_live_policy_changed".into());
        }
        Ok(self.model.infer(&input.window.cohort))
    }
}
#[cfg(test)]
#[path = "temporal_model_input_managed_tests.rs"]
mod tests;

#[cfg(test)]
#[path="temporal_fit_summary_managed_tests.rs"]
mod fit_summary_tests;

impl TemporalModel {
    /// Whole-model evaluation only. The protected snapshot has authenticated
    /// original native/HeldOut lineage; no imported diagnostic or inner model
    /// accessor can enter this boundary. Existing infer/tokens/math is reused.
    pub(crate) fn evaluate_managed_held_out(
        &self,
        source:&execution::full_run::VerifiedTemporalEvaluationSnapshot,
    )->Result<crate::room_learner::training::EvaluationReport,String>{
        self.validate_evaluation_source(source)?;
        let rows=source.evaluation_rows();
        crate::room_learner::training::EvaluationReport::compute(&self.model,&rows)
    }
    /// Recording reuses the SAME bounded inference traversal. This private
    /// method requires the actual opaque whole-model source, not imported DATA.
    pub(crate) fn evaluate_managed_held_out_recorded(
        &self,
        source:&execution::full_run::VerifiedTemporalEvaluationSnapshot,
    )->Result<(crate::room_learner::training::EvaluationReport,Vec<RoomPrediction>),String>{
        self.validate_evaluation_source(source)?;
        let rows=source.evaluation_rows();
        crate::room_learner::training::EvaluationReport::compute_recorded(&self.model,&rows)
    }
    fn validate_evaluation_source(
        &self,
        source:&execution::full_run::VerifiedTemporalEvaluationSnapshot,
    )->Result<(),String>{
        source.require_same_model(self)?;
        self.validate_managed()?;
        source.held_out().validate_data_links(source.proof())?;
        let binding=self.fit_diagnostic_binding()?;
        let identity=&source.proof().body.identity;
        if binding.artifact_kind!=SCHEMA_V3||binding.artifact_sha256!=identity.artifact_sha256
            ||binding.context_digest!=identity.context_digest
            ||binding.training_data_sha256!=source.proof().body.training_data_sha256
            ||binding.input_evidence_sha256!=source.proof().body.training_evidence_sha256
            ||binding.identity_evidence_sha256.as_ref()!=Some(&source.proof().body.training_identities_sha256)
        {return Err("temporal_evaluation_whole_model_lineage_changed".into());}
        Ok(())
    }
}
