//! Experimental input-policy adapter over the EXISTING RoomModel/fitter.
//! No API/runtime/storage/managed-proof/selection authority. Imported metadata
//! is integrity-checked historical input claims, not physical attestation.
use super::{
    model::{EncoderConfig, RoomModel, RoomPrediction},
    native::CanonicalPacket,
    observation::*,
    temporal_acquisition::{
        AcquisitionPlan, ExperimentalPlanId, NativeWindowAssembler, ReceiverCadenceBand,
        TemporalWindow, WindowAdmission,
    },
    training::{FitConfig, TrainingReport},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
pub(crate) const SCHEMA: &str = "ruview.experimental.temporal-model-input.v2";
const MAX_BYTES: usize = 4 * 1024 * 1024;
fn hash<T: Serialize>(v: &T) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(v).map_err(|_| "temporal_encoding")?)
    ))
}
fn bytes_hash(v: &[u8]) -> String {
    format!("{:x}", Sha256::digest(v))
}
fn sha(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn integer<T: std::str::FromStr + ToString>(s: &str) -> Result<T, String> {
    let n = s.parse::<T>().map_err(|_| "temporal_integer")?;
    if n.to_string() != s {
        return Err("temporal_noncanonical_integer".into());
    }
    Ok(n)
}
fn plan_id(s: &str) -> Result<ExperimentalPlanId, String> {
    use ExperimentalPlanId::*;
    Ok(match s {
        "Native1000msSlots8" => Native1000msSlots8,
        "Native1000msSlots4" => Native1000msSlots4,
        "Native1000msSlots2" => Native1000msSlots2,
        "Native2000msSlots8" => Native2000msSlots8,
        "Native2000msSlots4" => Native2000msSlots4,
        "Native2000msSlots2" => Native2000msSlots2,
        "Native3900msMax4000Slots8" => Native3900msMax4000Slots8,
        "Native3900msMax4000Slots4" => Native3900msMax4000Slots4,
        "Native3900msMax4000Slots2" => Native3900msMax4000Slots2,
        _ => return Err("temporal_plan_id".into()),
    })
}
fn band(s: &str) -> Result<ReceiverCadenceBand, String> {
    match s {
        "Arrival18To22Burst200MsV1" => Ok(ReceiverCadenceBand::Arrival18To22Burst200MsV1),
        "Hz30To36" => Ok(ReceiverCadenceBand::Hz30To36),
        "Hz45To55" => Ok(ReceiverCadenceBand::Hz45To55),
        _ => Err("temporal_cadence_band".into()),
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct SourcePolicy {
    source_id: u8,
    band: String,
    min_rows: usize,
    max_rows: usize,
    min_interval_ns: u64,
    max_gap_ns: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Binding {
    plan_id: String,
    native_frame_identity: String,
    plan_sha256: String,
    context: CaptureContext,
    encoder: EncoderConfig,
    target_span_ms: u64,
    max_span_ms: u64,
    max_pairwise_skew_ms: u64,
    max_first_delay_ms: u64,
    sources: Vec<SourcePolicy>,
}
impl Binding {
    fn from_plan(plan: &AcquisitionPlan) -> Self {
        Self {
            plan_id: format!("{:?}", plan.id()),
            native_frame_identity: "existing-esp32-receiver-ns-node-u32-sequence-v1".into(),
            plan_sha256: plan.sha256().into(),
            context: plan.context().clone(),
            encoder: plan.encoder_config().clone(),
            target_span_ms: plan.target_span_ms(),
            max_span_ms: plan.max_observed_span_ms(),
            max_pairwise_skew_ms: plan.max_pairwise_receiver_skew_ms(),
            max_first_delay_ms: plan.max_first_packet_delay_ms(),
            sources: plan
                .source_policies()
                .iter()
                .map(|s| SourcePolicy {
                    source_id: s.source_id,
                    band: format!("{:?}", s.receiver_cadence),
                    min_rows: s.min_rows,
                    max_rows: s.max_rows,
                    min_interval_ns: s.min_packet_interval_ns,
                    max_gap_ns: s.max_gap_ns,
                })
                .collect(),
        }
    }
    fn rebuild(&self) -> Result<AcquisitionPlan, String> {
        let bands = self
            .sources
            .iter()
            .map(|s| Ok((s.source_id, band(&s.band)?)))
            .collect::<Result<Vec<_>, String>>()?;
        let p = AcquisitionPlan::build(self.context.clone(), plan_id(&self.plan_id)?, &bands)?;
        if *self != Self::from_plan(&p) {
            return Err("temporal_policy_binding_changed".into());
        }
        Ok(p)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceEvidence {
    source_id: u8,
    source_spec_sha256: String,
    times_ns: Vec<String>,
    frame_ids: Vec<String>,
    sequence_ids: Vec<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WindowEvidence {
    cohort_id: String,
    trial_id: String,
    session_id: String,
    role: DataRole,
    label: RoomLabel,
    cohort_sha256: String,
    target_ns: String,
    sealed_at_ns: String,
    provenance: CaptureProvenance,
    sources: Vec<SourceEvidence>,
}
impl WindowEvidence {
    fn from_assembled(
        plan: &AcquisitionPlan,
        w: &TemporalWindow,
        sealed_at: u64,
        target_ns: u64,
    ) -> Result<Self, String> {
        Self::from_binding(&Binding::from_plan(plan), w, sealed_at, target_ns)
    }
    fn from_binding(
        binding: &Binding,
        w: &TemporalWindow,
        sealed_at: u64,
        target_ns: u64,
    ) -> Result<Self, String> {
        if w.policy_sha256 != binding.plan_sha256 {
            return Err("temporal_window_policy_changed".into());
        }
        validate_cohort(&binding.context, &w.cohort)?;
        if w.row_times_ns.len() != w.cohort.views.len() || w.cohort.teacher.is_some() {
            return Err("temporal_window_evidence_changed".into());
        }
        let mut sources = vec![];
        for ((spec, view), (source, times)) in binding
            .context
            .sources
            .iter()
            .zip(&w.cohort.views)
            .zip(&w.row_times_ns)
        {
            view.validate(spec)?;
            if *source != spec.source_id
                || view.source_id != *source
                || times.len() != view.samples.len()
                || times.first() != Some(&view.first_time_ns)
                || times.last() != Some(&view.last_time_ns)
                || view.clock.kind != ClockKind::ReceiverOnly
                || view.clock.uncertainty_ns != u64::MAX
                || view.phase != PhaseAvailability::MagnitudeOnly
            {
                return Err("temporal_source_evidence_changed".into());
            }
            sources.push(SourceEvidence {
                source_id: *source,
                source_spec_sha256: hash(spec)?,
                times_ns: times.iter().map(ToString::to_string).collect(),
                frame_ids: view.frame_ids.iter().map(ToString::to_string).collect(),
                sequence_ids: view.sequence_ids.clone(),
            });
        }
        let e = Self {
            cohort_id: w.cohort.id.clone(),
            trial_id: w.cohort.trial_id.clone(),
            session_id: w.cohort.session_id.clone(),
            role: w.cohort.role,
            label: w.cohort.label,
            cohort_sha256: hash(&w.cohort)?,
            target_ns: target_ns.to_string(),
            sealed_at_ns: sealed_at.to_string(),
            provenance: w.cohort.provenance,
            sources,
        };
        e.validate_sources(binding)?;
        Ok(e)
    }
    fn validate_sources(&self, b: &Binding) -> Result<(), String> {
        if !sha(&self.cohort_sha256) || self.sources.len() != b.sources.len() {
            return Err("temporal_evidence_shape".into());
        }
        let sealed = integer::<u64>(&self.sealed_at_ns)?;
        let target = integer::<u64>(&self.target_ns)?;
        if target == 0 {
            return Err("temporal_target_missing".into());
        }
        let mut first_min = u64::MAX;
        let mut first_max = 0;
        let mut last_min = u64::MAX;
        let mut last_max = 0;
        for ((s, p), spec) in self.sources.iter().zip(&b.sources).zip(&b.context.sources) {
            let n = s.times_ns.len();
            if s.source_id != p.source_id
                || s.source_id != spec.source_id
                || s.source_spec_sha256 != hash(spec)?
                || n < p.min_rows
                || n > p.max_rows
                || s.frame_ids.len() != n
                || s.sequence_ids.len() != n
            {
                return Err("temporal_source_shape_or_policy".into());
            }
            let times = s
                .times_ns
                .iter()
                .map(|s| integer::<u64>(s))
                .collect::<Result<Vec<_>, _>>()?;
            let frames = s
                .frame_ids
                .iter()
                .map(|s| integer::<u128>(s))
                .collect::<Result<Vec<_>, _>>()?;
            if times.contains(&0)
                || frames.contains(&0)
                || frames.iter().collect::<HashSet<_>>().len() != n
                || s.sequence_ids.iter().any(|s| *s > u32::MAX as u64)
                || s.sequence_ids.windows(2).any(|w| w[1] <= w[0])
            {
                return Err("temporal_frame_or_sequence".into());
            }
            for w in times.windows(2) {
                let gap = w[1].checked_sub(w[0]).ok_or("temporal_time_reversed")?;
                if gap < p.min_interval_ns || gap > p.max_gap_ns {
                    return Err("temporal_gap_or_cadence".into());
                }
            }
            // Check the EXACT identity codec already used by native::canonicalize_esp32.
            // This links original receiver timestamps/counters to their recorded IDs;
            // it does not infer a source clock or mint replacement frame identities.
            for ((time, frame), sequence) in times.iter().zip(&frames).zip(&s.sequence_ids) {
                let expected = (u128::from(*time) << 40)
                    | (u128::from(s.source_id) << 32)
                    | u128::from(*sequence);
                if *frame != expected {
                    return Err("temporal_native_frame_identity_changed".into());
                }
            }
            let first = times[0];
            let last = times[n - 1];
            let span = last - first;
            if first < target || first - target > b.max_first_delay_ms * 1_000_000 {
                return Err("temporal_first_arrival_changed".into());
            }
            if span < b.target_span_ms * 1_000_000
                || span > b.max_span_ms * 1_000_000
                || sealed < last
                || sealed - first > MAX_AGE_NS
            {
                return Err("temporal_horizon_or_age".into());
            }
            let (lo, hi) = band(&p.band)?.rate_bounds_millihz();
            let (lo, hi) = (u128::from(lo), u128::from(hi));
            let rate = (n - 1) as u128 * 1_000_000_000_000u128;
            if rate < lo * span as u128 || rate > hi * span as u128 {
                return Err("temporal_observed_cadence".into());
            }
            first_min = first_min.min(first);
            first_max = first_max.max(first);
            last_min = last_min.min(last);
            last_max = last_max.max(last);
        }
        if first_max - first_min > b.max_pairwise_skew_ms * 1_000_000
            || last_max - last_min > b.max_pairwise_skew_ms * 1_000_000
            || first_max >= last_min
        {
            return Err("temporal_source_alignment".into());
        }
        Ok(())
    }
}
#[derive(Clone, Default)]
struct Fences {
    frames: HashSet<(u8, u128)>,
    last: HashMap<u8, (u64, u64)>,
}
impl Fences {
    fn add(&mut self, e: &WindowEvidence) -> Result<(), String> {
        for s in &e.sources {
            for ((t, f), seq) in s.times_ns.iter().zip(&s.frame_ids).zip(&s.sequence_ids) {
                let t = integer::<u64>(t)?;
                let f = integer::<u128>(f)?;
                if self.frames.contains(&(s.source_id, f))
                    || self
                        .last
                        .get(&s.source_id)
                        .is_some_and(|(last_seq, last_t)| *seq <= *last_seq || t <= *last_t)
                {
                    return Err("temporal_cross_window_replay".into());
                }
                self.frames.insert((s.source_id, f));
                self.last.insert(s.source_id, (*seq, t));
            }
        }
        Ok(())
    }
    fn check_packet(&self, p: &CanonicalPacket) -> Result<(), String> {
        if self
            .frames
            .contains(&(p.source_id, p.native_frame.frame_id))
            || self.last.get(&p.source_id).is_some_and(|(seq, time)| {
                u64::from(p.sequence) <= *seq || p.native_frame.timestamp_ns <= *time
            })
        {
            return Err("temporal_training_frame_or_time_overlap".into());
        }
        Ok(())
    }
}
/// Immutable retained records shared with one bounded durability snapshot.
/// Serialization delegates exactly to the existing record; no new wire shape.
struct SharedRecordValue<T> {
    value:T,
    // In-memory integrity cache only. Never serialized or accepted from JSON.
    sha256:std::sync::OnceLock<String>,
}
#[derive(Clone)]
struct SharedRecord<T>(std::sync::Arc<SharedRecordValue<T>>);
impl<T> SharedRecord<T> {
    fn new(value:T)->Self {Self(std::sync::Arc::new(SharedRecordValue{value,sha256:std::sync::OnceLock::new()}))}
    fn same_record(&self,other:&Self)->bool {std::sync::Arc::ptr_eq(&self.0,&other.0)}
}
impl<T:Serialize> SharedRecord<T> {
    fn canonical_sha256(&self)->Result<&str,String>{
        if self.0.sha256.get().is_none(){let digest=hash(&self.0.value)?;let _=self.0.sha256.set(digest);}
        self.0.sha256.get().map(String::as_str).ok_or("temporal_record_digest_unavailable".into())
    }
}
impl<T> std::ops::Deref for SharedRecord<T> {
    type Target=T;
    fn deref(&self)->&T {&self.0.value}
}
impl<T> std::borrow::Borrow<T> for SharedRecord<T> {
    fn borrow(&self)->&T {&self.0.value}
}
impl<T:Serialize> Serialize for SharedRecord<T> {
    fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error>{self.0.value.serialize(serializer)}
}
fn owned_records<T: Clone>(records: &[SharedRecord<T>]) -> Vec<T> {
    records.iter().map(|record| (**record).clone()).collect()
}

/// Raw cohorts are private and assembled here; there is intentionally no API to
/// fit a caller-supplied temporal-evidence flag or caller-created window.
pub struct TemporalTrainingSession {
    plan: AcquisitionPlan,
    binding: Binding,
    run_id: String,
    assembler: NativeWindowAssembler,
    rows: Vec<SharedRecord<CaptureCohort>>,
    evidence: Vec<SharedRecord<WindowEvidence>>,
    config: FitConfig,
    fitted: bool,
    failed: bool,
    active: bool,
    target_ns: Option<u64>,
    managed: Option<managed::ManagedSession>,
}
fn preflight(
    context: &CaptureContext,
    plan: &AcquisitionPlan,
    config: &FitConfig,
) -> Result<usize, String> {
    if !(2..=8).contains(&context.sources.len()) {
        return Err("temporal_model_supports_2_to_8_sources".into());
    }
    let encoder = plan.encoder_config();
    if config.hidden_dim != encoder.hidden_dim
        || config.embedding_dim != encoder.embedding_dim
        || config.temporal_tokens != encoder.temporal_tokens
        || config.temperature != encoder.temperature
    {
        return Err("temporal_encoder_policy_changed".into());
    }
    config.validate_context(context)
}
impl TemporalTrainingSession {
    pub fn new(
        context: CaptureContext,
        id: ExperimentalPlanId,
        cadences: &[(u8, ReceiverCadenceBand)],
        run_id: String,
        config: FitConfig,
    ) -> Result<Self, String> {
        if !(2..=8).contains(&context.sources.len()) {
            return Err("temporal_model_supports_2_to_8_sources".into());
        }
        let plan = AcquisitionPlan::build(context, id, cadences)?;
        preflight(plan.context(), &plan, &config)?;
        let binding = Binding::from_plan(&plan);
        let assembler = NativeWindowAssembler::new(plan.clone(), run_id.clone())?;
        Ok(Self {
            plan,
            binding,
            run_id,
            assembler,
            rows: vec![],
            evidence: vec![],
            config,
            fitted: false,
            failed: false,
            active: false,
            target_ns: None,
            managed: None,
        })
    }
    pub fn begin_window(&mut self, target_ns: u64, end_ns: u64) -> Result<(), String> {
        if self.managed.is_some() {
            return Err("managed_window_target_required".into());
        }
        if self.failed {
            return Err("temporal_session_failed".into());
        }
        let train_count = self.plan.resources().cohorts / 2;
        if self.rows.len() >= train_count && !self.fitted {
            return Err("temporal_fit_before_heldout".into());
        }
        self.assembler.begin_window(target_ns, end_ns)?;
        self.active = true;
        self.target_ns = Some(target_ns);
        Ok(())
    }
    pub fn ingest(&mut self, p: &CanonicalPacket) -> Result<WindowAdmission, String> {
        if self.failed {
            return Err("temporal_session_failed".into());
        }
        let result = self.assembler.ingest(p);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub fn finish_window(&mut self, now_ns: u64) -> Result<(), String> {
        if self.failed {
            return Err("temporal_session_failed".into());
        }
        // The assembler consumes/advances its window before adapter evidence is
        // checked. Any subsequent failure must terminate this wrapper run;
        // previous valid windows remain available for diagnostics, never credit
        // an unvalidated window or let its cursor be silently reused.
        let result = (|| {
            let window = self.assembler.finish_window(now_ns)?;
            let evidence = if let Some(managed) = &self.managed {
                WindowEvidence::from_binding(
                    &managed.binding.validation_binding()?,
                    &window,
                    now_ns,
                    self.target_ns.ok_or("temporal_target_missing")?,
                )?
            } else {
                WindowEvidence::from_assembled(
                    &self.plan,
                    &window,
                    now_ns,
                    self.target_ns.ok_or("temporal_target_missing")?,
                )?
            };
            let managed_identity = window.managed_identity.clone();
            if self.managed.is_some() && managed_identity.is_none() {
                return Err("managed_window_identity_missing".into());
            }
            Ok::<_, String>((window, evidence, managed_identity))
        })();
        self.active = false;
        self.target_ns = None;
        match result {
            Ok((window, evidence, identity)) => {
                if let Some(managed) = &mut self.managed {
                    managed
                        .windows
                        .push(identity.ok_or("managed_window_identity_missing")?);
                }
                self.rows.push(SharedRecord::new(window.cohort));
                self.evidence.push(SharedRecord::new(evidence));
                Ok(())
            }
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }
    pub fn fit(&mut self) -> Result<(TemporalModel, TrainingReport), String> {
        if self.managed.is_some() {
            return Err("managed_fit_admission_required".into());
        }
        if self.failed
            || self.active
            || self.fitted
            || self.rows.len() != self.plan.resources().cohorts / 2
            || self.rows.iter().any(|r| r.role != DataRole::Train)
        {
            return Err("temporal_complete_train_schedule_required".into());
        }
        preflight(self.plan.context(), &self.plan, &self.config)?;
        let dataset = ValidatedDataset::new(self.plan.context().clone(), owned_records(&self.rows))?;
        let (model, report) = RoomModel::fit(&dataset, self.config.clone())?;
        if model.encoder_config() != self.plan.encoder_config() {
            self.failed = true;
            return Err("temporal_fitter_encoder_changed".into());
        }
        let input = TemporalModel {
            binding: self.binding.clone(),
            run_id: self.run_id.clone(),
            model,
            evidence: owned_records(&self.evidence),
            managed: None,
        };
        if let Err(error) = input.validate() {
            self.failed = true;
            return Err(error);
        }
        self.fitted = true;
        Ok((input, report))
    }
    pub fn dataset(&self) -> Result<ValidatedDataset, String> {
        ValidatedDataset::new(self.plan.context().clone(), owned_records(&self.rows))
    }
    pub fn plan_sha256(&self) -> &str {
        &self.binding.plan_sha256
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    binding: Binding,
    run_id: String,
    training_data_sha256: String,
    input_evidence_sha256: String,
    evidence: Vec<WindowEvidence>,
    model_sha256: String,
    model_json: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: String,
    payload_sha256: String,
    payload: Payload,
}
/// Crate-private aggregate diagnostic identity. Not provenance or an inner model
/// accessor; checked against the WHOLE validated temporal codec on every use.
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemporalFitBinding {
    pub artifact_kind:String,pub artifact_sha256:String,pub context_digest:String,
    pub training_data_sha256:String,pub input_evidence_sha256:String,
    pub identity_evidence_sha256:Option<String>,pub input_policy_sha256:String,
    pub execution_policy_sha256:Option<String>,pub schedule_sha256:Option<String>,
    pub source_ids:Vec<u8>,pub parameter_count:usize,pub training_cohorts:usize,
    pub encoder:EncoderConfig,pub bound_fit_config:Option<FitConfig>,
}
/// Existing learned network, with a new, separate versioned INPUT envelope.
/// Neither a checksum nor historical imported metadata is managed provenance.
#[derive(Clone)]
pub struct TemporalModel {
    managed: Option<managed::ManagedModel>,
    binding: Binding,
    run_id: String,
    model: RoomModel,
    evidence: Vec<WindowEvidence>,
}
impl TemporalModel {
    fn validate(&self) -> Result<AcquisitionPlan, String> {
        if self.managed.is_some() {
            return Err("managed_v3_requires_policy_dispatch".into());
        }
        let p = self.binding.rebuild()?;
        self.model.validate()?;
        if !(2..=8).contains(&p.context().sources.len())
            || self.model.context() != p.context()
            || self.model.encoder_config() != p.encoder_config()
            || self.evidence.len() != p.resources().cohorts / 2
            || self.model.training_manifest().training_cohorts != self.evidence.len()
            || self.model.training_manifest().clock_claims != [ClockKind::ReceiverOnly]
            || self.model.training_manifest().measurement_clock_available
        {
            return Err("temporal_model_input_binding".into());
        }
        let _ = NativeWindowAssembler::new(p.clone(), self.run_id.clone())?;
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
            return Err("temporal_training_provenance_changed".into());
        }
        let mut fences = Fences::default();
        for (index, e) in self.evidence.iter().enumerate() {
            let step = &p.schedule()[index / p.slots_per_trial()];
            let slot = index % p.slots_per_trial();
            let trial = format!("{}-{}", self.run_id, step.step_id);
            if e.role != DataRole::Train
                || e.role != step.role
                || e.label != step.label
                || e.trial_id != trial
                || e.session_id != trial
                || e.cohort_id != format!("{trial}-{slot}")
            {
                return Err("temporal_training_role_or_identity".into());
            }
            e.validate_sources(&self.binding)?;
            fences.add(e)?;
        }
        Ok(p)
    }
    pub fn export(&self) -> Result<Vec<u8>, String> {
        if self.managed.is_some() {
            return self.export_managed();
        }
        self.validate()?;
        let model = self.model.export()?;
        let payload = Payload {
            binding: self.binding.clone(),
            run_id: self.run_id.clone(),
            training_data_sha256: self.model.training_manifest().training_data_sha256.clone(),
            input_evidence_sha256: hash(&self.evidence)?,
            evidence: self.evidence.clone(),
            model_sha256: bytes_hash(&model),
            model_json: String::from_utf8(model).map_err(|_| "temporal_model_encoding")?,
        };
        let envelope = Envelope {
            schema: SCHEMA.into(),
            payload_sha256: hash(&payload)?,
            payload,
        };
        let bytes = serde_json::to_vec(&envelope).map_err(|_| "temporal_encoding")?;
        if bytes.len() > MAX_BYTES {
            return Err("temporal_input_envelope_budget".into());
        }
        Ok(bytes)
    }
    pub fn import(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("temporal_input_envelope_budget".into());
        }
        let version: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| "temporal_input_envelope_schema")?;
        if version["schema"] == managed::SCHEMA_V3 {
            return Self::import_managed(bytes);
        }
        let envelope: Envelope =
            serde_json::from_slice(bytes).map_err(|_| "temporal_input_envelope_schema")?;
        if envelope.schema != SCHEMA
            || envelope.payload_sha256 != hash(&envelope.payload)?
            || serde_json::to_vec(&envelope).map_err(|_| "temporal_encoding")? != bytes
        {
            return Err("temporal_input_envelope_integrity".into());
        }
        let payload = envelope.payload;
        if payload.model_sha256 != bytes_hash(payload.model_json.as_bytes())
            || payload.input_evidence_sha256 != hash(&payload.evidence)?
        {
            return Err("temporal_model_or_evidence_hash".into());
        }
        let model = RoomModel::import(payload.model_json.as_bytes())?;
        if model.export()? != payload.model_json.as_bytes()
            || model.training_manifest().training_data_sha256 != payload.training_data_sha256
        {
            return Err("temporal_model_training_binding".into());
        }
        let result = Self {
            managed: None,
            binding: payload.binding,
            run_id: payload.run_id,
            model,
            evidence: payload.evidence,
        };
        result.validate()?;
        Ok(result)
    }
    pub fn live_input(&self, run_id: String) -> Result<TemporalLiveInput, String> {
        if self.managed.is_some() {
            return self.managed_live_input(run_id);
        }
        let p = self.validate()?;
        let mut fences = Fences::default();
        for e in &self.evidence {
            fences.add(e)?;
        }
        Ok(TemporalLiveInput {
            binding: self.binding.clone(),
            assembler: NativeWindowAssembler::new(p, run_id)?,
            fences,
            failed: false,
            target_ns: None,
        })
    }
    pub fn predict(
        &self,
        input: TemporalLiveObservation,
        now_ns: u64,
    ) -> Result<RoomPrediction, String> {
        if self.managed.is_some() {
            return self.predict_managed(input, now_ns);
        }
        let plan = self.validate()?;
        if input.window.cohort.role != DataRole::Inference
            || input.window.cohort.label != RoomLabel::Unknown
            || input.window.cohort.teacher.is_some()
        {
            return Err("temporal_live_must_be_unlabelled".into());
        }
        let evidence =
            WindowEvidence::from_assembled(&plan, &input.window, now_ns, input.target_ns)?;
        let mut fences = Fences::default();
        for e in &self.evidence {
            fences.add(e)?;
        }
        fences.add(&evidence)?;
        if input.binding != self.binding || input.window.policy_sha256 != self.binding.plan_sha256 {
            return Err("temporal_live_policy_changed".into());
        }
        let cohort = input.window.cohort;
        validate_cohort(&self.binding.context, &cohort)?;
        for v in &cohort.views {
            if now_ns < v.last_time_ns
                || now_ns
                    .checked_sub(v.first_time_ns)
                    .filter(|a| *a <= MAX_AGE_NS)
                    .is_none()
            {
                return Err("temporal_live_input_stale".into());
            }
        }
        Ok(self.model.infer(&cohort))
    }
    /// Read-only immutable input policy for a diagnostic window driver.
    /// Does not expose the inner v1 network or issue managed provenance.
    pub fn input_policy(&self) -> Result<AcquisitionPlan, String> {
        self.validate()
    }
    pub fn window_input_policy(
        &self,
    ) -> Result<super::temporal_acquisition::WindowInputPolicy, String> {
        if let Some(managed) = &self.managed {
            self.validate_managed()?;
            Ok(managed.binding.native.clone())
        } else {
            Ok(super::temporal_acquisition::WindowInputPolicy::from_seed(
                &self.validate()?,
            ))
        }
    }
    pub fn artifact_kind(&self) -> &'static str {
        if self.managed.is_some() {
            managed::SCHEMA_V3
        } else {
            SCHEMA
        }
    }
    /// Read-only historical context for owner-scoped diagnostic persistence.
    /// This does not expose the inner model or establish managed provenance.
    pub fn context(&self) -> &CaptureContext {
        &self.binding.context
    }
    pub(crate) fn fit_diagnostic_binding(&self)->Result<TemporalFitBinding,String>{
        let bytes=self.export()?;
        let (schedule,execution,identities,configured)=if self.managed.is_some(){
            let (s,e,i,c)=self.managed_fit_diagnostic_policy()?;(Some(s),Some(e),Some(i),Some(c))
        }else{(None,None,None,None)};
        Ok(TemporalFitBinding{artifact_kind:self.artifact_kind().into(),artifact_sha256:bytes_hash(&bytes),context_digest:self.context().digest(),training_data_sha256:self.model.training_manifest().training_data_sha256.clone(),input_evidence_sha256:hash(&self.evidence)?,identity_evidence_sha256:identities,input_policy_sha256:self.plan_sha256().into(),execution_policy_sha256:execution,schedule_sha256:schedule,source_ids:self.context().sources.iter().map(|s|s.source_id).collect(),parameter_count:self.parameter_count(),training_cohorts:self.model.training_manifest().training_cohorts,encoder:self.model.encoder_config().clone(),bound_fit_config:configured})
    }
    pub fn parameter_count(&self) -> usize {
        self.model.parameter_count()
    }
    pub fn plan_sha256(&self) -> &str {
        &self.binding.plan_sha256
    }
}
/// Bounded one-window inference input seed. Runtime warmup/continuous replay
/// custody remains a future integration; this does not replace a live service.
pub struct TemporalLiveInput {
    binding: Binding,
    assembler: NativeWindowAssembler,
    fences: Fences,
    failed: bool,
    target_ns: Option<u64>,
}
pub struct TemporalLiveObservation {
    binding: Binding,
    window: TemporalWindow,
    target_ns: u64,
}
impl TemporalLiveInput {
    pub fn begin_window(&mut self, target_ns: u64, end_ns: u64) -> Result<(), String> {
        if self.failed {
            return Err("temporal_live_failed".into());
        }
        self.assembler.begin_window(target_ns, end_ns)?;
        self.target_ns = Some(target_ns);
        Ok(())
    }
    pub fn ingest(&mut self, p: &CanonicalPacket) -> Result<WindowAdmission, String> {
        if self.failed {
            return Err("temporal_live_failed".into());
        }
        if let Err(e) = self.fences.check_packet(p) {
            self.failed = true;
            return Err(e);
        }
        let result = self.assembler.ingest(p);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub fn finish(self, now_ns: u64) -> Result<TemporalLiveObservation, String> {
        if self.failed {
            return Err("temporal_live_failed".into());
        }
        let mut assembler = self.assembler;
        let mut window = assembler.finish_window(now_ns)?;
        // Fresh inference observations carry no operator schedule label. This
        // never relabels retained Train/HeldOut rows or creates teacher truth.
        window.cohort.role = DataRole::Inference;
        window.cohort.label = RoomLabel::Unknown;
        window.cohort.teacher = None;
        WindowEvidence::from_binding(
            &self.binding,
            &window,
            now_ns,
            self.target_ns.ok_or("temporal_target_missing")?,
        )?;
        Ok(TemporalLiveObservation {
            binding: self.binding,
            window,
            target_ns: self.target_ns.ok_or("temporal_target_missing")?,
        })
    }
}
#[cfg(test)]
#[path = "temporal_model_input_tests.rs"]
mod tests;

#[path = "temporal_model_input_managed.rs"]
pub(crate) mod managed;
