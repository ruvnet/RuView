//! Source-only v2 acquisition seed. Not wired to capture, inference, artifacts or
//! qualification. All policy arms are experimental engineering choices.
use super::managed_schedule_contract::{BoundEpisode, ManagedScheduleContract};
use super::{
    native::{self, CanonicalPacket},
    observation::*,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

const NS_PER_MS: u64 = 1_000_000;
const NS_PER_MILLI_HZ: u64 = 1_000_000_000_000;
const MAX_NATIVE_SPAN_NS: u64 = 4_000_000_000;
const SERIALIZED_LIMIT: usize = 64 * 1024 * 1024;
const SERIALIZED_METADATA_RESERVE: usize = 8 * 1024 * 1024;
const PREPROCESSING: &str = "native-esp32-drop2-linear56-ordered-magnitude-v2-experimental";

/// Finite, visible alternatives. Selecting a smaller variant is explicit; there
/// is no automatic truncation or silent fallback to fewer slots/shorter input.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum ExperimentalPlanId {
    Native1000msSlots8,
    Native1000msSlots4,
    Native1000msSlots2,
    Native2000msSlots8,
    Native2000msSlots4,
    Native2000msSlots2,
    Native3900msMax4000Slots8,
    Native3900msMax4000Slots4,
    Native3900msMax4000Slots2,
}
impl ExperimentalPlanId {
    fn dimensions(self) -> (u64, usize) {
        use ExperimentalPlanId::*;
        match self {
            Native1000msSlots8 => (1000, 8),
            Native1000msSlots4 => (1000, 4),
            Native1000msSlots2 => (1000, 2),
            Native2000msSlots8 => (2000, 8),
            Native2000msSlots4 => (2000, 4),
            Native2000msSlots2 => (2000, 2),
            Native3900msMax4000Slots8 => (3900, 8),
            Native3900msMax4000Slots4 => (3900, 4),
            Native3900msMax4000Slots2 => (3900, 2),
        }
    }
}
/// A frozen receiver-cadence study band, NOT a source measurement clock or
/// scientific adequacy claim. Actual packet intervals/rate must also fit it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReceiverCadenceBand {
    Hz30To36,
    Hz45To55,
}
impl ReceiverCadenceBand {
    /// Read-only frozen-rate bounds for versioned input validation.
    pub fn rate_bounds_millihz(self) -> (u64, u64) {
        let (lo, hi, _) = self.range();
        (lo, hi)
    }
    fn range(self) -> (u64, u64, u64) {
        match self {
            Self::Hz30To36 => (30_000, 36_000, 50),
            Self::Hz45To55 => (45_000, 55_000, 35),
        }
    }
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ResourcePreflight {
    pub cohorts: usize,
    pub source_views: usize,
    pub max_retained_rows: usize,
    pub max_numeric_values: usize,
    pub serialized_upper_bound_bytes: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct TrialStep {
    pub step_id: String,
    pub role: DataRole,
    pub label: RoomLabel,
    pub duration_ms: u64,
    pub lead_in_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceWindowPolicy {
    pub source_id: u8,
    pub receiver_cadence: ReceiverCadenceBand,
    pub min_rows: usize,
    pub max_rows: usize,
    pub min_packet_interval_ns: u64,
    pub max_gap_ns: u64,
}
#[derive(Debug, Clone, Serialize)]
struct HardLimits {
    rows_per_source: usize,
    cohorts: usize,
    numeric_values: usize,
    serialized_bytes: usize,
    metadata_reserve_bytes: usize,
    max_native_span_ns: u64,
    max_first_packet_age_ns: u64,
}
#[derive(Debug, Clone, Serialize)]
struct PlanData {
    schema: &'static str,
    id: ExperimentalPlanId,
    context_digest: String,
    preprocessing_id: &'static str,
    quality_policy_id: &'static str,
    limits: HardLimits,
    encoder_id: &'static str,
    encoder: super::model::EncoderConfig,
    target_span_ms: u64,
    min_observed_span_ms: u64,
    max_observed_span_ms: u64,
    max_pairwise_receiver_skew_ms: u64,
    max_first_packet_delay_ms: u64,
    slots_per_trial: usize,
    schedule: Vec<TrialStep>,
    sources: Vec<SourceWindowPolicy>,
    resources: ResourcePreflight,
}
/// Private fields and no Deserialize: network input cannot supply thresholds or
/// issue a new policy by changing serialized bytes. Context/spec validators are
/// the existing learner's; this is not another model/dataset implementation.
#[derive(Debug, Clone)]
pub struct AcquisitionPlan {
    context: CaptureContext,
    data: PlanData,
    sha256: String,
}
fn add(a: usize, b: usize) -> Result<usize, String> {
    a.checked_add(b)
        .ok_or_else(|| "acquisition_resource_overflow".into())
}
fn mul(a: usize, b: usize) -> Result<usize, String> {
    a.checked_mul(b)
        .ok_or_else(|| "acquisition_resource_overflow".into())
}
fn ceil_div(a: u64, b: u64) -> Result<u64, String> {
    a.checked_add(b - 1)
        .map(|v| v / b)
        .ok_or_else(|| "acquisition_resource_overflow".into())
}
impl AcquisitionPlan {
    pub fn build(
        context: CaptureContext,
        id: ExperimentalPlanId,
        cadences: &[(u8, ReceiverCadenceBand)],
    ) -> Result<Self, String> {
        context.validate()?;
        if cadences.len() != context.sources.len()
            || cadences
                .iter()
                .map(|s| s.0)
                .ne(context.sources.iter().map(|s| s.source_id))
        {
            return Err("acquisition_cadence_source_binding".into());
        }
        let (target_ms, slots) = id.dimensions();
        let max_ms = if target_ms == 3900 {
            4000
        } else {
            target_ms + 50
        };
        let target_ns = target_ms * NS_PER_MS;
        let max_ns = max_ms * NS_PER_MS;
        if max_ns > MAX_NATIVE_SPAN_NS {
            return Err("acquisition_native_horizon_limit".into());
        }
        let mut sources = vec![];
        for (spec, (_, band)) in context.sources.iter().zip(cadences) {
            if spec.feature_count != native::CANONICAL_PACKET_BINS
                || spec.signal_kind != ruview_unified::RfModality::WifiCsi
                || spec.device_id != format!("esp32-node-{}", spec.source_id)
                || spec.link_index != 0
                || !native::is_supported_frequency_hz(spec.center_frequency_hz)
                || spec.bandwidth_hz != 20_000_000
                || !(0..=3).any(|ppdu| {
                    spec.grid_id
                        == format!(
                            "ant1-bins{}-ppdu{ppdu}-usable-drop2-linear-v2",
                            if ppdu == 0 { 64 } else { 256 }
                        )
                })
            {
                return Err("acquisition_requires_existing_native56_source".into());
            }
            let (lo, hi, gap_ms) = band.range();
            let max_rows = usize::try_from(
                ceil_div(
                    max_ns
                        .checked_mul(hi)
                        .ok_or("acquisition_resource_overflow")?,
                    NS_PER_MILLI_HZ,
                )? + 3,
            )
            .map_err(|_| "acquisition_resource_overflow")?;
            let min_rows = usize::try_from(
                target_ns
                    .checked_mul(lo)
                    .ok_or("acquisition_resource_overflow")?
                    / NS_PER_MILLI_HZ
                    + 1,
            )
            .map_err(|_| "acquisition_resource_overflow")?;
            if max_rows > MAX_ROWS || min_rows < 2 || min_rows > max_rows {
                return Err("acquisition_row_limit".into());
            }
            sources.push(SourceWindowPolicy {
                source_id: spec.source_id,
                receiver_cadence: *band,
                min_rows,
                max_rows,
                min_packet_interval_ns: NS_PER_MILLI_HZ / hi,
                max_gap_ns: gap_ms * NS_PER_MS,
            });
        }
        let mut schedule = vec![];
        for (role_index, role) in [DataRole::Train, DataRole::HeldOut].into_iter().enumerate() {
            for (condition, label) in [
                RoomLabel::Empty,
                RoomLabel::OccupiedStatic,
                RoomLabel::OccupiedMoving,
                RoomLabel::AdjacentOnly,
            ]
            .into_iter()
            .enumerate()
            {
                for trial in 0..2 {
                    // Whole-trial duration is distinct from the retained model
                    // horizon. Space each sparse slot so its window can finish.
                    let duration_ms = if label == RoomLabel::Empty {
                        600_000
                    } else {
                        (slots as u64 * 2 * (max_ms + 100)).div_ceil(1000).max(12) * 1000
                    };
                    schedule.push(TrialStep {
                        step_id: format!("v2-r{role_index}-c{condition}-t{trial}"),
                        role,
                        label,
                        duration_ms,
                        lead_in_ms: if matches!(label, RoomLabel::Empty | RoomLabel::AdjacentOnly) {
                            15_000
                        } else {
                            5_000
                        },
                    });
                }
            }
        }
        let cohorts = mul(schedule.len(), slots)?;
        if cohorts > MAX_COHORTS {
            return Err("acquisition_cohort_limit".into());
        }
        let rows_per_cohort = sources.iter().try_fold(0, |sum, s| add(sum, s.max_rows))?;
        let values_per_cohort = sources
            .iter()
            .zip(&context.sources)
            .try_fold(0, |sum, (s, spec)| {
                add(sum, mul(s.max_rows, spec.feature_count)?)
            })?;
        let rows = mul(cohorts, rows_per_cohort)?;
        let values = mul(cohorts, values_per_cohort)?;
        if values > MAX_TOTAL_VALUES {
            return Err("acquisition_numeric_capacity".into());
        }
        let views = mul(cohorts, sources.len())?;
        // Conservative JSON upper bound: <=24-byte finite f32 text + comma;
        // original u128 frame/u64 sequence/u64 row time + array delimiters;
        // escaped bounded SourceView/identities, cohort wrappers, and reserved
        // future policy/quality/checkpoint metadata. Uses MAX rows, never mean.
        let serialized = add(
            add(add(mul(values, 25)?, mul(rows, 86)?)?, mul(views, 4096)?)?,
            add(mul(cohorts, 4096)?, SERIALIZED_METADATA_RESERVE)?,
        )?;
        if serialized > SERIALIZED_LIMIT {
            return Err("acquisition_serialized_capacity".into());
        }
        let resources = ResourcePreflight {
            cohorts,
            source_views: views,
            max_retained_rows: rows,
            max_numeric_values: values,
            serialized_upper_bound_bytes: serialized,
        };
        let data = PlanData {
            schema: "ruview.experimental.native-temporal-acquisition.v2",
            id,
            context_digest: context.digest(),
            preprocessing_id: PREPROCESSING,
            quality_policy_id: "receiver-cadence-gap-skew-first-age-v1-experimental",
            limits: HardLimits {
                rows_per_source: MAX_ROWS,
                cohorts: MAX_COHORTS,
                numeric_values: MAX_TOTAL_VALUES,
                serialized_bytes: SERIALIZED_LIMIT,
                metadata_reserve_bytes: SERIALIZED_METADATA_RESERVE,
                max_native_span_ns: MAX_NATIVE_SPAN_NS,
                max_first_packet_age_ns: MAX_AGE_NS,
            },
            encoder_id: "existing-aether-ordered-block-means-v1",
            encoder: super::model::EncoderConfig {
                source_count: context.sources.len(),
                ..Default::default()
            },
            target_span_ms: target_ms,
            min_observed_span_ms: target_ms,
            max_observed_span_ms: max_ms,
            max_pairwise_receiver_skew_ms: 50,
            max_first_packet_delay_ms: 100,
            slots_per_trial: slots,
            schedule,
            sources,
            resources,
        };
        let sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&data).map_err(|_| "acquisition_plan_encoding")?)
        );
        Ok(Self {
            context,
            data,
            sha256,
        })
    }
    /// Read-only adapter seam; does not change serialized policy or its hash.
    pub fn context(&self) -> &CaptureContext {
        &self.context
    }
    pub fn id(&self) -> ExperimentalPlanId {
        self.data.id
    }
    pub fn slots_per_trial(&self) -> usize {
        self.data.slots_per_trial
    }
    pub fn max_pairwise_receiver_skew_ms(&self) -> u64 {
        self.data.max_pairwise_receiver_skew_ms
    }
    pub fn max_first_packet_delay_ms(&self) -> u64 {
        self.data.max_first_packet_delay_ms
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn resources(&self) -> &ResourcePreflight {
        &self.data.resources
    }
    pub fn schedule(&self) -> &[TrialStep] {
        &self.data.schedule
    }
    pub fn source_policies(&self) -> &[SourceWindowPolicy] {
        &self.data.sources
    }
    pub fn encoder_config(&self) -> &super::model::EncoderConfig {
        &self.data.encoder
    }
    pub fn target_span_ms(&self) -> u64 {
        self.data.target_span_ms
    }
    pub fn max_observed_span_ms(&self) -> u64 {
        self.data.max_observed_span_ms
    }
}

struct PacketRow {
    received_ns: u64,
    frame_id: u128,
    sequence: u64,
    values: Vec<f32>,
}
struct ActiveWindow {
    target_ns: u64,
    end_ns: u64,
    rows: HashMap<u8, Vec<PacketRow>>,
    provenance: Option<CaptureProvenance>,
    failed: bool,
    ready: bool,
}
/// Read-only native window rules shared by seed-v2, managed-v3 and live input.
/// No experimental seed schedule/hash is returned for a managed contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowInputPolicy {
    context: CaptureContext,
    sources: Vec<SourceWindowPolicy>,
    target_span_ms: u64,
    max_observed_span_ms: u64,
    max_first_packet_delay_ms: u64,
    max_pairwise_receiver_skew_ms: u64,
}
impl WindowInputPolicy {
    pub(crate) fn from_seed(plan: &AcquisitionPlan) -> Self {
        Self {
            context: plan.context.clone(),
            sources: plan.data.sources.clone(),
            target_span_ms: plan.data.target_span_ms,
            max_observed_span_ms: plan.data.max_observed_span_ms,
            max_first_packet_delay_ms: plan.data.max_first_packet_delay_ms,
            max_pairwise_receiver_skew_ms: plan.data.max_pairwise_receiver_skew_ms,
        }
    }
    pub(crate) fn from_managed(contract: &ManagedScheduleContract) -> Self {
        contract.native_window_policy()
    }
    pub(crate) fn from_contract_fields(
        context: CaptureContext,
        sources: Vec<SourceWindowPolicy>,
        target: u64,
        max: u64,
        delay: u64,
        skew: u64,
    ) -> Self {
        Self {
            context,
            sources,
            target_span_ms: target,
            max_observed_span_ms: max,
            max_first_packet_delay_ms: delay,
            max_pairwise_receiver_skew_ms: skew,
        }
    }
    /// Same stateless native-source checks for retained windows and whole-period
    /// observation. This does not admit a window, establish replay freshness or
    /// create a measured clock/teacher claim.
    pub(crate) fn validate_packet_source(
        &self,
        p: &CanonicalPacket,
    ) -> Result<(&SourceSpec, &SourceWindowPolicy), String> {
        let spec = self
            .context
            .sources
            .iter()
            .find(|s| s.source_id == p.source_id)
            .ok_or("acquisition_unbound_source")?;
        let policy = self
            .sources
            .iter()
            .find(|s| s.source_id == p.source_id)
            .ok_or("acquisition_unbound_source")?;
        if p.grid_id != spec.grid_id
            || p.frequency_hz != spec.center_frequency_hz
            || p.bandwidth_hz != spec.bandwidth_hz
            || p.native_frame.modality != spec.signal_kind
            || p.native_frame.provenance.device_id != spec.device_id
            || p.native_frame.centre_frequency_hz != spec.center_frequency_hz as f64
            || p.native_frame.bandwidth_hz != spec.bandwidth_hz as f64
            || p.amplitudes.len() != spec.feature_count
            || p.amplitudes
                .iter()
                .any(|x| !x.is_finite() || *x < 0. || *x > f32::MAX as f64)
            || p.native_frame.frame_id == 0
            || p.clock.kind != ClockKind::ReceiverOnly
            || p.clock.uncertainty_ns != u64::MAX
            || p.phase_consumed
        {
            return Err("acquisition_native_source_changed".into());
        }
        if let Some(g) = spec.geometry {
            if p.native_frame.transmitter_pose.map(|p| p.position_m) != Some(g.tx_pos)
                || p.native_frame.receiver_pose.map(|p| p.position_m) != Some(g.rx_pos)
            {
                return Err("acquisition_native_geometry_changed".into());
            }
        }
        let (lo, hi, _) = policy.receiver_cadence.range();
        if !p.receiver_sample_rate_hz.is_finite()
            || p.receiver_sample_rate_hz < lo as f64 / 1000.
            || p.receiver_sample_rate_hz > hi as f64 / 1000.
        {
            return Err("acquisition_cadence_changed".into());
        }
        Ok((spec, policy))
    }
    pub fn context(&self) -> &CaptureContext {
        &self.context
    }
    pub fn source_policies(&self) -> &[SourceWindowPolicy] {
        &self.sources
    }
    pub fn target_span_ms(&self) -> u64 {
        self.target_span_ms
    }
    pub fn max_observed_span_ms(&self) -> u64 {
        self.max_observed_span_ms
    }
    pub fn max_first_packet_delay_ms(&self) -> u64 {
        self.max_first_packet_delay_ms
    }
    pub fn max_pairwise_receiver_skew_ms(&self) -> u64 {
        self.max_pairwise_receiver_skew_ms
    }
}
/// Historical mapping, not operator confirmation, quality or managed proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManagedWindowIdentity {
    pub contract_sha256: String,
    pub episode_index: usize,
    pub slot_index: usize,
    pub episode_id: String,
    pub trial_id: String,
    pub session_id: String,
    pub attempt_id: String,
    pub attempt_ordinal: u16,
    pub segment_ordinal: u16,
    pub role: DataRole,
    pub label: RoomLabel,
    pub collection_started_ns: String,
    pub target_ns: String,
    pub end_ns: String,
    pub frozen_artifact_sha256: Option<String>,
    pub mapping: crate::room_learner::temporal_model_input::managed::execution::ObservedSegmentMap,
}
/// Only a future Runtime mapping (or explicit SYNTHETIC unit fixture) can issue
/// this non-Deserialize target. No client timestamps/roles are accepted here.
#[derive(Debug, Clone)]
pub struct ManagedWindowTarget {
    run_id: String,
    identity: ManagedWindowIdentity,
}
impl ManagedWindowTarget {
    #[cfg(test)]
    pub(crate) fn from_runtime_episode(
        contract: &ManagedScheduleContract,
        bound: &BoundEpisode,
        run: &str,
        slot: usize,
        collection_started_ns: u64,
    ) -> Result<Self, String> {
        let mapping = crate::room_learner::temporal_model_input::managed::execution::ObservedSegmentMap::synthetic_uninterrupted(bound, collection_started_ns);
        Self::from_observed_episode(contract, bound, run, slot, mapping)
    }
    pub(crate) fn from_observed_episode(
        contract: &ManagedScheduleContract,
        bound: &BoundEpisode,
        run: &str,
        slot: usize,
        mapping: crate::room_learner::temporal_model_input::managed::execution::ObservedSegmentMap,
    ) -> Result<Self, String> {
        if !contract.bound_matches_run(bound, run) {
            return Err("managed_window_episode_binding".into());
        }
        let spec = bound
            .episode()
            .slots
            .get(slot)
            .ok_or("managed_window_slot_missing")?;
        let (target, end) = mapping.slot_bounds(
            bound,
            spec.target_offset_ms * NS_PER_MS,
            spec.deadline_offset_ms * NS_PER_MS,
        )?;
        Ok(Self {
            run_id: run.into(),
            identity: ManagedWindowIdentity {
                contract_sha256: contract.sha256().into(),
                episode_index: bound.episode().index,
                slot_index: slot,
                episode_id: bound.episode_id().into(),
                trial_id: bound.trial_id().into(),
                session_id: bound.session_id().into(),
                attempt_id: bound.attempt_id().into(),
                attempt_ordinal: bound.attempt_ordinal(),
                segment_ordinal: bound.segment_ordinal(),
                role: bound.episode().role,
                label: bound.episode().condition,
                collection_started_ns: mapping.receiver_start_ns.to_string(),
                target_ns: target.to_string(),
                end_ns: end.to_string(),
                frozen_artifact_sha256: bound.frozen_artifact_sha256().map(str::to_owned),
                mapping,
            },
        })
    }
    pub(crate) fn identity(&self) -> &ManagedWindowIdentity {
        &self.identity
    }
    pub fn target_ns(&self) -> u64 {
        self.identity
            .target_ns
            .parse()
            .expect("internally bounded target")
    }
    pub fn end_ns(&self) -> u64 {
        self.identity
            .end_ns
            .parse()
            .expect("internally bounded target")
    }
}
#[derive(Clone)]
enum WindowIdentityMode {
    Seed(AcquisitionPlan),
    Managed {
        contract: ManagedScheduleContract,
        binding_sha256: String,
        pending: Option<ManagedWindowIdentity>,
    },
    Inference {
        binding_sha256: String,
    },
}
/// Internal exact original row times are not a serialized/public progress DTO.
pub struct TemporalWindow {
    pub policy_sha256: String,
    pub cohort: CaptureCohort,
    pub row_times_ns: Vec<(u8, Vec<u64>)>,
    pub(crate) managed_identity: Option<ManagedWindowIdentity>,
}
/// A bounded single-run seed. Runtime authorization, whole-trial quality/seal,
/// recovery and artifact version integration are deliberately not implemented.
/// Source watermarks survive successful windows/role transitions within this
/// object. A failure is terminal for this seed run: it cannot relabel/retry it.
pub struct NativeWindowAssembler {
    policy: WindowInputPolicy,
    identity: WindowIdentityMode,
    run_id: String,
    next: usize,
    active: Option<ActiveWindow>,
    watermarks: HashMap<u8, (u64, u64)>,
    frames: HashSet<(u8, u128)>,
    last_end_ns: u64,
    failed: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub enum WindowAdmission {
    Collecting,
    Ready,
}
impl NativeWindowAssembler {
    pub fn new(plan: AcquisitionPlan, run_id: String) -> Result<Self, String> {
        if run_id.is_empty()
            || run_id.len() > 128
            || !run_id.is_ascii()
            || run_id
                .bytes()
                .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
        {
            return Err("acquisition_run_id".into());
        }
        Ok(Self {
            policy: WindowInputPolicy::from_seed(&plan),
            identity: WindowIdentityMode::Seed(plan),
            run_id,
            next: 0,
            active: None,
            watermarks: HashMap::new(),
            frames: HashSet::new(),
            last_end_ns: 0,
            failed: false,
        })
    }
    pub(crate) fn managed(
        contract: ManagedScheduleContract,
        binding_sha256: String,
        run_id: String,
    ) -> Result<Self, String> {
        let seed = AcquisitionPlan::build(
            contract.context().clone(),
            ExperimentalPlanId::Native2000msSlots8,
            &contract
                .context()
                .sources
                .iter()
                .map(|s| (s.source_id, ReceiverCadenceBand::Hz30To36))
                .collect::<Vec<_>>(),
        )?;
        let mut result = Self::new(seed, run_id)?;
        result.policy = WindowInputPolicy::from_managed(&contract);
        result.identity = WindowIdentityMode::Managed {
            contract,
            binding_sha256,
            pending: None,
        };
        Ok(result)
    }
    pub(crate) fn inference(
        policy: WindowInputPolicy,
        binding_sha256: String,
        run_id: String,
    ) -> Result<Self, String> {
        // Run-id validation and native rules are identical; the seed is only an
        // internal rules constructor and never supplies inference role/hash.
        let bands = policy
            .sources
            .iter()
            .map(|p| (p.source_id, p.receiver_cadence))
            .collect::<Vec<_>>();
        let seed = AcquisitionPlan::build(
            policy.context.clone(),
            ExperimentalPlanId::Native2000msSlots8,
            &bands,
        )?;
        let mut result = Self::new(seed, run_id)?;
        result.policy = policy;
        result.identity = WindowIdentityMode::Inference { binding_sha256 };
        Ok(result)
    }
    pub fn begin_managed_window(&mut self, target: &ManagedWindowTarget) -> Result<(), String> {
        let WindowIdentityMode::Managed {
            contract, pending, ..
        } = &mut self.identity
        else {
            return Err("managed_window_mode_required".into());
        };
        if pending.is_some()
            || target.run_id != self.run_id
            || target.identity.contract_sha256 != contract.sha256()
        {
            return Err("managed_window_contract_changed".into());
        }
        let expected = contract
            .episodes()
            .iter()
            .take(target.identity.episode_index)
            .map(|e| e.slots.len())
            .sum::<usize>()
            + target.identity.slot_index;
        if expected != self.next || expected >= contract.counts().total_windows {
            return Err("managed_window_schedule_order".into());
        }
        *pending = Some(target.identity.clone());
        if let Err(e) = self.begin_checked(target.target_ns(), target.end_ns()) {
            if let WindowIdentityMode::Managed { pending, .. } = &mut self.identity {
                *pending = None;
            }
            return Err(e);
        }
        Ok(())
    }
    /// Target and end come from future server slot scheduling, not model scores.
    /// First packets must begin near target, and the COMPLETE horizon must fit.
    pub fn begin_window(&mut self, target_ns: u64, end_ns: u64) -> Result<(), String> {
        if matches!(self.identity, WindowIdentityMode::Managed { .. }) {
            return Err("managed_window_target_required".into());
        }
        self.begin_checked(target_ns, end_ns)
    }
    fn begin_checked(&mut self, target_ns: u64, end_ns: u64) -> Result<(), String> {
        let capacity = match &self.identity {
            WindowIdentityMode::Seed(p) => p.resources().cohorts,
            WindowIdentityMode::Managed { contract, .. } => contract.counts().total_windows,
            WindowIdentityMode::Inference { .. } => 1,
        };
        if self.failed || self.active.is_some() || self.next >= capacity {
            return Err("acquisition_window_state".into());
        }
        if target_ns == 0
            || target_ns <= self.last_end_ns
            || end_ns <= target_ns
            || end_ns - target_ns
                < self.policy.max_observed_span_ms * NS_PER_MS
                    + self.policy.max_first_packet_delay_ms * NS_PER_MS
        {
            return Err("acquisition_slot_bounds".into());
        }
        self.active = Some(ActiveWindow {
            target_ns,
            end_ns,
            rows: HashMap::new(),
            provenance: None,
            failed: false,
            ready: false,
        });
        Ok(())
    }
    pub fn ingest(&mut self, packet: &CanonicalPacket) -> Result<WindowAdmission, String> {
        let result = self.ingest_checked(packet);
        if result.is_err() {
            self.failed = true;
            if let Some(a) = &mut self.active {
                a.failed = true;
            }
        }
        result
    }
    fn ingest_checked(&mut self, p: &CanonicalPacket) -> Result<WindowAdmission, String> {
        if self.failed {
            return Err("acquisition_window_state".into());
        }
        let a = self
            .active
            .as_mut()
            .filter(|a| !a.failed && !a.ready)
            .ok_or("acquisition_window_not_collecting")?;
        let (_spec, policy) = self.policy.validate_packet_source(p)?;
        let t = p.native_frame.timestamp_ns;
        if t < a.target_ns || t >= a.end_ns {
            return Err("acquisition_packet_outside_slot".into());
        }
        if self
            .watermarks
            .get(&p.source_id)
            .is_some_and(|(seq, last)| u64::from(p.sequence) <= *seq || t <= *last)
            || self
                .frames
                .contains(&(p.source_id, p.native_frame.frame_id))
        {
            return Err("acquisition_replayed_or_reversed".into());
        }
        if a.provenance.is_some_and(|v| v != p.provenance) {
            return Err("acquisition_provenance_changed".into());
        }
        let rows = a.rows.entry(p.source_id).or_default();
        if rows.len() >= policy.max_rows {
            return Err("acquisition_row_capacity".into());
        }
        if let Some(last) = rows.last() {
            let gap = t - last.received_ns;
            if gap > policy.max_gap_ns || gap < policy.min_packet_interval_ns {
                return Err("acquisition_packet_gap_or_rate".into());
            }
        } else if t - a.target_ns > self.policy.max_first_packet_delay_ms * NS_PER_MS {
            return Err("acquisition_late_first_packet".into());
        }
        if let Some(first) = rows.first() {
            if t - first.received_ns > self.policy.max_observed_span_ms * NS_PER_MS {
                return Err("acquisition_window_span_limit".into());
            }
        }
        a.provenance = Some(p.provenance);
        rows.push(PacketRow {
            received_ns: t,
            frame_id: p.native_frame.frame_id,
            sequence: u64::from(p.sequence),
            values: p.amplitudes.iter().map(|v| *v as f32).collect(),
        });
        self.watermarks
            .insert(p.source_id, (u64::from(p.sequence), t));
        self.frames.insert((p.source_id, p.native_frame.frame_id));
        a.ready = self.policy.sources.iter().all(|s| {
            a.rows.get(&s.source_id).is_some_and(|r| {
                r.len() >= s.min_rows
                    && r.last().unwrap().received_ns - r[0].received_ns
                        >= self.policy.target_span_ms * NS_PER_MS
            })
        });
        if a.ready {
            Ok(WindowAdmission::Ready)
        } else {
            Ok(WindowAdmission::Collecting)
        }
    }
    pub fn finish_window(&mut self, now_ns: u64) -> Result<TemporalWindow, String> {
        let result = self.finish_checked(now_ns);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn finish_checked(&mut self, now_ns: u64) -> Result<TemporalWindow, String> {
        if self.failed {
            return Err("acquisition_window_state".into());
        }
        let a = self
            .active
            .as_ref()
            .filter(|a| a.ready && !a.failed)
            .ok_or("acquisition_horizon_incomplete")?;
        if now_ns >= a.end_ns {
            return Err("acquisition_slot_expired".into());
        }

        let mut views = vec![];
        let mut times = vec![];
        let mut first_min = u64::MAX;
        let mut first_max = 0;
        let mut last_min = u64::MAX;
        let mut last_max = 0;
        for (spec, policy) in self.policy.context.sources.iter().zip(&self.policy.sources) {
            let rows = a
                .rows
                .get(&spec.source_id)
                .ok_or("acquisition_missing_source")?;
            let first = rows
                .first()
                .ok_or("acquisition_missing_source")?
                .received_ns;
            let last = rows.last().unwrap().received_ns;
            let age = now_ns
                .checked_sub(first)
                .filter(|age| *age <= MAX_AGE_NS)
                .ok_or("acquisition_first_packet_age")?;
            if now_ns < last {
                return Err("acquisition_future_packet".into());
            }
            let span = last - first;
            if span < self.policy.target_span_ms * NS_PER_MS
                || span > self.policy.max_observed_span_ms * NS_PER_MS
                || span > MAX_NATIVE_SPAN_NS
            {
                return Err("acquisition_observed_span".into());
            }
            let (lo, hi, _) = policy.receiver_cadence.range();
            let scaled = (rows.len() - 1) as u128 * NS_PER_MILLI_HZ as u128;
            if scaled < (lo as u128 * span as u128) || scaled > (hi as u128 * span as u128) {
                return Err("acquisition_observed_cadence".into());
            }
            first_min = first_min.min(first);
            first_max = first_max.max(first);
            last_min = last_min.min(last);
            last_max = last_max.max(last);
            let view = SourceView {
                source_id: spec.source_id,
                device_id: spec.device_id.clone(),
                grid_id: spec.grid_id.clone(),
                signal_kind: spec.signal_kind,
                center_frequency_hz: spec.center_frequency_hz,
                bandwidth_hz: spec.bandwidth_hz,
                link_id: spec.link_id.clone(),
                link_index: spec.link_index,
                geometry: spec.geometry,
                samples: rows.iter().map(|r| r.values.clone()).collect(),
                frame_ids: rows.iter().map(|r| r.frame_id).collect(),
                sequence_ids: rows.iter().map(|r| r.sequence).collect(),
                first_time_ns: first,
                last_time_ns: last,
                clock: ClockInfo {
                    kind: ClockKind::ReceiverOnly,
                    uncertainty_ns: u64::MAX,
                },
                sample_age_ns: age,
                phase: PhaseAvailability::MagnitudeOnly,
                provenance: a.provenance.ok_or("acquisition_provenance_missing")?,
            };
            view.validate(spec)?;
            views.push(view);
            times.push((spec.source_id, rows.iter().map(|r| r.received_ns).collect()));
        }
        let skew = self.policy.max_pairwise_receiver_skew_ms * NS_PER_MS;
        if first_max - first_min > skew || last_max - last_min > skew || first_max >= last_min {
            return Err("acquisition_source_skew".into());
        }
        let (id, trial_id, session_id, role, label, policy_sha256, managed_identity) = match &self
            .identity
        {
            WindowIdentityMode::Seed(plan) => {
                let step = &plan.data.schedule[self.next / plan.data.slots_per_trial];
                let slot = self.next % plan.data.slots_per_trial;
                (
                    format!("{}-{}-{slot}", self.run_id, step.step_id),
                    format!("{}-{}", self.run_id, step.step_id),
                    format!("{}-{}", self.run_id, step.step_id),
                    step.role,
                    step.label,
                    plan.sha256.clone(),
                    None,
                )
            }
            WindowIdentityMode::Managed {
                binding_sha256,
                pending,
                ..
            } => {
                let p = pending.as_ref().ok_or("managed_window_target_missing")?;
                let id = format!(
                    "window-{:x}",
                    Sha256::digest(
                        serde_json::to_vec(p).map_err(|_| "managed_window_identity_encoding")?
                    )
                );
                (
                    id,
                    p.trial_id.clone(),
                    p.session_id.clone(),
                    p.role,
                    p.label,
                    binding_sha256.clone(),
                    Some(p.clone()),
                )
            }
            WindowIdentityMode::Inference { binding_sha256 } => (
                format!("{}-inference-0", self.run_id),
                format!("{}-inference", self.run_id),
                format!("{}-inference", self.run_id),
                DataRole::Inference,
                RoomLabel::Unknown,
                binding_sha256.clone(),
                None,
            ),
        };
        let cohort = CaptureCohort {
            id,
            context_digest: self.policy.context.digest(),
            trial_id,
            session_id,
            role,
            label,
            views,
            teacher: None,
            provenance: a.provenance.unwrap(),
        };
        validate_cohort(&self.policy.context, &cohort)?;
        let result = TemporalWindow {
            policy_sha256,
            cohort,
            row_times_ns: times,
            managed_identity,
        };
        if let WindowIdentityMode::Managed { pending, .. } = &mut self.identity {
            *pending = None;
        }
        self.last_end_ns = last_max;
        self.next += 1;
        self.active = None;
        Ok(result)
    }
}
#[cfg(test)]
#[path = "temporal_acquisition_tests.rs"]
mod tests;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssemblerCheckpoint {
    next: usize,
    last_end_ns: u64,
    watermarks: Vec<(u8, u64, u64)>,
    frames: Vec<(u8, String)>,
}
impl AssemblerCheckpoint {
    pub(crate) fn for_prefix(mut self, n: usize) -> Result<Self, String> {
        if n > self.next {
            return Err("temporal_prefix_ahead".into());
        }
        self.next = n;
        Ok(self)
    }
}
impl NativeWindowAssembler {
    pub(crate) fn fence_checkpoint(&self) -> Result<AssemblerCheckpoint, String> {
        if self.failed {
            return Err("temporal_failed_fences".into());
        }
        let mut watermarks = self
            .watermarks
            .iter()
            .map(|(id, (seq, t))| (*id, *seq, *t))
            .collect::<Vec<_>>();
        watermarks.sort();
        let mut frames = self
            .frames
            .iter()
            .map(|(id, f)| (*id, f.to_string()))
            .collect::<Vec<_>>();
        frames.sort();
        Ok(AssemblerCheckpoint {
            next: self.next,
            last_end_ns: self.last_end_ns,
            watermarks,
            frames,
        })
    }
    pub(crate) fn restore_prefix_fences(
        &mut self,
        s: AssemblerCheckpoint,
        expected_next: usize,
    ) -> Result<(), String> {
        let max = self
            .policy
            .sources
            .iter()
            .map(|s| s.max_rows)
            .sum::<usize>()
            * 128;
        if s.next != expected_next
            || s.next > 128
            || s.frames.len() > max
            || s.watermarks.len() > self.policy.sources.len()
        {
            return Err("temporal_checkpoint_fence_bounds".into());
        }
        let mut watermarks = HashMap::new();
        let mut frames = HashSet::new();
        for (id, seq, t) in s.watermarks {
            if !self
                .policy
                .context
                .sources
                .iter()
                .any(|x| x.source_id == id)
                || seq == 0
                || t == 0
                || watermarks.insert(id, (seq, t)).is_some()
            {
                return Err("temporal_checkpoint_fence_source".into());
            }
        }
        for (id, f) in s.frames {
            let f = f.parse::<u128>().map_err(|_| "temporal_checkpoint_frame")?;
            let (sequence, time) = watermarks
                .get(&id)
                .copied()
                .ok_or("temporal_checkpoint_frame")?;
            if ((f >> 32) & 255) != u128::from(id)
                || (f >> 40) > u128::from(time)
                || (f & u128::from(u32::MAX)) > u128::from(sequence)
                || !frames.insert((id, f))
            {
                return Err("temporal_checkpoint_frame".into());
            }
        }
        self.next = s.next;
        self.last_end_ns = s.last_end_ns;
        self.watermarks = watermarks;
        self.frames = frames;
        self.active = None;
        self.failed = false;
        if let WindowIdentityMode::Managed { pending, .. } = &mut self.identity {
            *pending = None;
        }
        Ok(())
    }
    pub(crate) fn fences_cover(&self, other: &AssemblerCheckpoint) -> bool {
        other.watermarks.iter().all(|(id, seq, t)| {
            self.watermarks
                .get(id)
                .is_some_and(|(s, n)| s >= seq && n >= t)
        }) && other.frames.iter().all(|(id, f)| {
            f.parse()
                .ok()
                .is_some_and(|f| self.frames.contains(&(*id, f)))
        })
    }
}

impl NativeWindowAssembler {
    pub(crate) fn retained_fences_cover(&self, rows: &[(u8, u64, u64, u128)]) -> bool {
        rows.iter().all(|(id, seq, t, f)| {
            self.watermarks
                .get(id)
                .is_some_and(|(s, n)| s >= seq && n >= t)
                && self.frames.contains(&(*id, *f))
        })
    }
}
