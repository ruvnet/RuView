//! Inactive engineering schedule contract for the EXISTING managed learner.
//! No capture/runtime/model/storage caller and no quality/adequacy authority.
use super::{
    observation::{CaptureContext, DataRole, RoomLabel, MAX_COHORTS, MAX_ROWS, MAX_TOTAL_VALUES},
    temporal_acquisition::{
        AcquisitionPlan, ExperimentalPlanId, ReceiverCadenceBand, ResourcePreflight,
        SourceWindowPolicy,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
const SCHEMA: &str = "ruview.room-learner.trial-schedule.v2";
const PROFILE: &str = "native-2s-three-source-56bin-option-a-engineering-v1";
pub(crate) const OPERATOR_EIGHT_PROFILE: &str = "native-2s-three-source-56bin-operator-eight-step-v1";
pub(crate) const OPERATOR_VARIABLE_PROFILE: &str = "native-2s-variable-source-56bin-receiver20-operator-eight-step-v2";
const MAX_BYTES: usize = 65_536;
const MAX_PROFILE_ROWS: usize = 77;
const MAX_TRAIN_WINDOWS: usize = 64;

/// Explicit engineering choice, never a user-facing or scientific default.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeadInPolicy {
    Train5HeldOut15EngineeringV1,
    OperatorEightStepV1,
    OperatorVariableSourcesV2,
}
impl LeadInPolicy {
    pub(crate) fn is_operator(self) -> bool { matches!(self, Self::OperatorEightStepV1 | Self::OperatorVariableSourcesV2) }
    fn milliseconds(self, role: DataRole, condition: RoomLabel) -> u64 {
        if self.is_operator() {
            return if matches!(condition, RoomLabel::Empty | RoomLabel::AdjacentOnly) { 15_000 } else { 5_000 };
        }
        match role {
            DataRole::Train => 5_000,
            DataRole::HeldOut => 15_000,
            _ => unreachable!(),
        }
    }
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WindowSlot {
    pub index: usize,
    pub target_offset_ms: u64,
    pub deadline_offset_ms: u64,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Episode {
    pub index: usize,
    pub episode_key: String,
    pub role: DataRole,
    pub condition: RoomLabel,
    pub target_active_recording_ms: u64,
    pub lead_in_ms: u64,
    pub requires_explicit_operator_start: bool,
    pub requires_conditions_held_confirmation: bool,
    pub separately_executed_episode_required: bool,
    pub slots: Vec<WindowSlot>,
    pub retained_nominal_window_ms: u64,
    pub retained_maximum_window_ms: u64,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ScheduleCounts {
    pub train_episodes: usize,
    pub held_out_episodes: usize,
    pub train_windows: usize,
    pub held_out_windows: usize,
    pub total_windows: usize,
    pub train_active_recording_ms: u64,
    pub held_out_active_recording_ms: u64,
    pub active_recording_ms: u64,
    pub train_lead_in_ms: u64,
    pub held_out_lead_in_ms: u64,
    pub total_lead_in_ms: u64,
    pub nominal_recording_and_lead_in_ms: u64,
    pub nominal_joint_window_ms: u64,
    pub nominal_source_view_window_ms: u64,
    pub verified_independent_trial_count: Option<usize>,
}
#[derive(Debug, Clone, Serialize)]
struct NativeInputPolicy {
    id: &'static str,
    original_frame_identity: &'static str,
    projection: &'static str,
    target_span_ms: u64,
    maximum_span_ms: u64,
    maximum_first_packet_delay_ms: u64,
    maximum_pairwise_receiver_skew_ms: u64,
    sources: Vec<SourceWindowPolicy>,
}
#[derive(Debug, Clone, Serialize)]
struct ContractClaims {
    capture_ready: bool,
    full_episode_quality_monitoring_implemented: bool,
    independence_verified: bool,
    scientific_adequacy_claimed: bool,
    user_facing_default: bool,
}
#[derive(Debug, Clone, Serialize)]
struct Body {
    profile_id: &'static str,
    context: CaptureContext,
    lead_in_policy: LeadInPolicy,
    native_input: NativeInputPolicy,
    episodes: Vec<Episode>,
    counts: ScheduleCounts,
    resources: ResourcePreflight,
    serialized_checkpoint_preflight_is_estimate_only: bool,
    claims: ContractClaims,
}
#[derive(Serialize)]
struct Export<'a> {
    schema: &'static str,
    contract_sha256: &'a str,
    body: &'a Body,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Import {
    schema: String,
    contract_sha256: String,
    body: serde_json::Value,
}
/// Immutable metadata, not a preflight/checkpoint receipt or a capture controller.
#[derive(Debug, Clone)]
pub struct ManagedScheduleContract {
    body: Body,
    sha256: String,
}
fn digest(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}
fn hash(body: &Body) -> Result<String, String> {
    let bytes = serde_json::to_vec(body).map_err(|_| "managed_schedule_encoding")?;
    let mut h = Sha256::new();
    h.update(SCHEMA);
    h.update([0]);
    h.update(bytes);
    Ok(format!("{:x}", h.finalize()))
}
fn add(a: usize, b: usize) -> Result<usize, String> {
    a.checked_add(b)
        .ok_or("managed_schedule_capacity_overflow".into())
}
fn mul(a: usize, b: usize) -> Result<usize, String> {
    a.checked_mul(b)
        .ok_or("managed_schedule_capacity_overflow".into())
}
fn bounded_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.is_ascii()
        && !value
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
}
fn sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn capacity(
    context: &CaptureContext,
    sources: &[SourceWindowPolicy],
    train: usize,
    total: usize,
) -> Result<(usize, usize), String> {
    if train > MAX_TRAIN_WINDOWS
        || total > MAX_COHORTS
        || train == 0
        || total <= train
        || sources.len() != context.sources.len()
    {
        return Err("managed_schedule_window_capacity".into());
    }
    let mut rows = 0;
    let mut values = 0;
    for (spec, source) in context.sources.iter().zip(sources) {
        if source.source_id != spec.source_id
            || source.max_rows > MAX_ROWS
            || source.max_rows > MAX_PROFILE_ROWS
            || source.min_rows < 2
            || source.min_rows > source.max_rows
        {
            return Err("managed_schedule_row_capacity".into());
        }
        rows = add(rows, source.max_rows)?;
        values = add(values, mul(source.max_rows, spec.feature_count)?)?;
    }
    let rows = mul(rows, total)?;
    let values = mul(values, total)?;
    if values > MAX_TOTAL_VALUES {
        return Err("managed_schedule_numeric_capacity".into());
    }
    Ok((rows, values))
}
fn slots(duration: u64, count: usize, reserved_ms: u64) -> Result<Vec<WindowSlot>, String> {
    if count < 2
        || count > 16
        || reserved_ms == 0
        || duration < reserved_ms
        || duration
            < reserved_ms
                .checked_mul(count as u64)
                .ok_or("managed_schedule_slot_overflow")?
    {
        return Err("managed_schedule_slot_bounds".into());
    }
    let stride_span = duration - reserved_ms;
    let result = (0..count)
        .map(|index| {
            let start = stride_span
                .checked_mul(index as u64)
                .ok_or("managed_schedule_slot_overflow")?
                / (count - 1) as u64;
            Ok(WindowSlot {
                index,
                target_offset_ms: start,
                deadline_offset_ms: start + reserved_ms,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if result
        .windows(2)
        .any(|pair| pair[0].deadline_offset_ms > pair[1].target_offset_ms)
    {
        return Err("managed_schedule_slot_overlap".into());
    }
    Ok(result)
}
impl ManagedScheduleContract {
    pub fn build(context: CaptureContext, lead_in_policy: LeadInPolicy) -> Result<Self, String> {
        context.validate()?;
        // Reuse the existing actual-native 2s validators/capacity calculation,
        // but NEVER its different schedule, serialized plan or plan hash.
        let variable = lead_in_policy == LeadInPolicy::OperatorVariableSourcesV2;
        let bands = context
            .sources
            .iter()
            .map(|source| (source.source_id, if variable { ReceiverCadenceBand::Arrival18To22Burst200MsV1 } else { ReceiverCadenceBand::Hz30To36 }))
            .collect::<Vec<_>>();
        let native = AcquisitionPlan::build(
            context.clone(),
            if variable { ExperimentalPlanId::Native2000msSlots4 } else { ExperimentalPlanId::Native2000msSlots8 },
            &bands,
        )?;
        if (if variable { !(2..=8).contains(&context.sources.len()) } else { context.sources.len() != 3 })
            || context
                .sources
                .iter()
                .any(|source| source.feature_count != 56)
        {
            return Err(if variable { "managed_schedule_profile_source_count_or_format" } else { "managed_schedule_profile_supports_three_native56_sources" }.into());
        }
        let reserved = native.max_observed_span_ms() + native.max_first_packet_delay_ms();
        let mut episodes = Vec::new();
        if lead_in_policy.is_operator() {
            for condition in [RoomLabel::Empty, RoomLabel::OccupiedStatic, RoomLabel::OccupiedMoving, RoomLabel::AdjacentOnly] {
                for role in [DataRole::Train, DataRole::Validation] {
                    let duration = if role == DataRole::Validation { 15_000 } else if condition == RoomLabel::Empty { 600_000 } else { 60_000 };
                    let count = if role == DataRole::Train { 16 } else { 4 };
                    episodes.push(Episode {
                        index: episodes.len(),
                        episode_key: format!("operator-{role:?}-{condition:?}-episode-0"),
                        role,
                        condition,
                        target_active_recording_ms: duration,
                        lead_in_ms: lead_in_policy.milliseconds(role, condition),
                        requires_explicit_operator_start: true,
                        requires_conditions_held_confirmation: true,
                        separately_executed_episode_required: true,
                        slots: slots(duration, count, reserved)?,
                        retained_nominal_window_ms: count as u64 * native.target_span_ms(),
                        retained_maximum_window_ms: count as u64 * native.max_observed_span_ms(),
                    });
                }
            }
        } else {
        for role in [DataRole::Train, DataRole::HeldOut] {
            for condition in [
                RoomLabel::Empty,
                RoomLabel::OccupiedStatic,
                RoomLabel::OccupiedMoving,
                RoomLabel::AdjacentOnly,
            ] {
                let empty_train = role == DataRole::Train && condition == RoomLabel::Empty;
                for repeat in 0..if empty_train { 1 } else { 2 } {
                    let duration = if empty_train { 600_000 } else { 30_000 };
                    let count = if empty_train { 16 } else { 8 };
                    episodes.push(Episode {
                        index: episodes.len(),
                        episode_key: format!("{role:?}-{condition:?}-episode-{repeat}"),
                        role,
                        condition,
                        target_active_recording_ms: duration,
                        lead_in_ms: lead_in_policy.milliseconds(role, condition),
                        requires_explicit_operator_start: true,
                        requires_conditions_held_confirmation: true,
                        separately_executed_episode_required: true,
                        slots: slots(duration, count, reserved)?,
                        retained_nominal_window_ms: count as u64 * native.target_span_ms(),
                        retained_maximum_window_ms: count as u64 * native.max_observed_span_ms(),
                    });
                }
            }
        }
        }
        let role_counts = |role| episodes.iter().filter(|e| e.role == role).count();
        let role_windows = |role| {
            episodes
                .iter()
                .filter(|e| e.role == role)
                .map(|e| e.slots.len())
                .sum::<usize>()
        };
        let role_time = |role| {
            episodes
                .iter()
                .filter(|e| e.role == role)
                .map(|e| e.target_active_recording_ms)
                .sum::<u64>()
        };
        let role_lead = |role| {
            episodes
                .iter()
                .filter(|e| e.role == role)
                .map(|e| e.lead_in_ms)
                .sum::<u64>()
        };
        let train = role_windows(DataRole::Train);
        let check_role = if lead_in_policy.is_operator() { DataRole::Validation } else { DataRole::HeldOut };
        let held = role_windows(check_role);
        let total = add(train, held)?;
        let (rows, values) = capacity(&context, native.source_policies(), train, total)?;
        let resources = if lead_in_policy.is_operator() {
            let views = mul(total, context.sources.len())?;
            let serialized = add(add(add(mul(values, 25)?, mul(rows, 86)?)?, mul(views, 4096)?)?, add(mul(total, 4096)?, 8 * 1024 * 1024)?)?;
            ResourcePreflight { cohorts: total, source_views: views, max_retained_rows: rows, max_numeric_values: values, serialized_upper_bound_bytes: serialized }
        } else { native.resources().clone() };
        if variable && resources.serialized_upper_bound_bytes > 64 * 1024 * 1024 {
            return Err("managed_schedule_serialized_capacity".into());
        }
        if resources.cohorts != total
            || resources.max_retained_rows != rows
            || resources.max_numeric_values != values
        {
            return Err("managed_schedule_resource_contract_changed".into());
        }
        let active = role_time(DataRole::Train) + role_time(check_role);
        let lead = role_lead(DataRole::Train) + role_lead(check_role);
        let counts = ScheduleCounts {
            train_episodes: role_counts(DataRole::Train),
            held_out_episodes: role_counts(check_role),
            train_windows: train,
            held_out_windows: held,
            total_windows: total,
            train_active_recording_ms: role_time(DataRole::Train),
            held_out_active_recording_ms: role_time(check_role),
            active_recording_ms: active,
            train_lead_in_ms: role_lead(DataRole::Train),
            held_out_lead_in_ms: role_lead(check_role),
            total_lead_in_ms: lead,
            nominal_recording_and_lead_in_ms: active + lead,
            nominal_joint_window_ms: total as u64 * native.target_span_ms(),
            nominal_source_view_window_ms: total as u64
                * native.target_span_ms()
                * context.sources.len() as u64,
            verified_independent_trial_count: None,
        };
        let body = Body {
            profile_id: if variable { OPERATOR_VARIABLE_PROFILE } else if lead_in_policy.is_operator() { OPERATOR_EIGHT_PROFILE } else { PROFILE },
            context,
            lead_in_policy,
            native_input: NativeInputPolicy {
                id: if variable { "native-2s-56bin-receiver-arrival18to22-burst200-v1" } else { "existing-native-2s-56bin-receiver-30to36-v2" },
                original_frame_identity: "existing-esp32-receiver-ns-node-u32-sequence-v1",
                projection: "native-esp32-drop2-linear56-ordered-magnitude-v2-experimental",
                target_span_ms: native.target_span_ms(),
                maximum_span_ms: native.max_observed_span_ms(),
                maximum_first_packet_delay_ms: native.max_first_packet_delay_ms(),
                maximum_pairwise_receiver_skew_ms: native.max_pairwise_receiver_skew_ms(),
                sources: native.source_policies().to_vec(),
            },
            episodes,
            counts,
            resources,
            serialized_checkpoint_preflight_is_estimate_only: true,
            claims: ContractClaims {
                capture_ready: false,
                full_episode_quality_monitoring_implemented: false,
                independence_verified: false,
                scientific_adequacy_claimed: false,
                user_facing_default: lead_in_policy.is_operator(),
            },
        };
        let result = Self {
            sha256: hash(&body)?,
            body,
        };
        result.export()?;
        Ok(result)
    }
    pub fn export(&self) -> Result<Vec<u8>, String> {
        let bytes = serde_json::to_vec(&Export {
            schema: SCHEMA,
            contract_sha256: &self.sha256,
            body: &self.body,
        })
        .map_err(|_| "managed_schedule_encoding")?;
        if bytes.len() > MAX_BYTES {
            return Err("managed_schedule_size_limit".into());
        }
        Ok(bytes)
    }
    pub fn import(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("managed_schedule_size_limit".into());
        }
        let value: Import =
            serde_json::from_slice(bytes).map_err(|_| "managed_schedule_invalid_schema")?;
        if value.schema != SCHEMA || (value.body["profile_id"] != PROFILE && value.body["profile_id"] != OPERATOR_EIGHT_PROFILE && value.body["profile_id"] != OPERATOR_VARIABLE_PROFILE) {
            return Err("managed_schedule_unsupported_profile".into());
        }
        let context = serde_json::from_value(value.body["context"].clone())
            .map_err(|_| "managed_schedule_invalid_context")?;
        let lead = serde_json::from_value(value.body["lead_in_policy"].clone())
            .map_err(|_| "managed_schedule_invalid_lead_in")?;
        let rebuilt = Self::build(context, lead)?;
        if value.contract_sha256 != rebuilt.sha256
            || value.body
                != serde_json::to_value(&rebuilt.body).map_err(|_| "managed_schedule_encoding")?
            || rebuilt.export() != Ok(bytes.to_vec())
        {
            return Err("managed_schedule_contract_mismatch".into());
        }
        Ok(rebuilt)
    }
    pub(crate) fn native_window_policy(&self)->super::temporal_acquisition::WindowInputPolicy {
        let p=&self.body.native_input;super::temporal_acquisition::WindowInputPolicy::from_contract_fields(self.body.context.clone(),p.sources.clone(),p.target_span_ms,p.maximum_span_ms,p.maximum_first_packet_delay_ms,p.maximum_pairwise_receiver_skew_ms)
    }
    pub(crate) fn bound_matches_run(&self,bound:&BoundEpisode,run:&str)->bool {
        let model=bound.frozen_artifact_sha256.as_ref().and_then(|sha|FrozenModelIdentity::from_runtime_verified_artifact(self,sha.clone()).ok());
        self.bind_episode_start(run,bound.episode.index,bound.attempt_ordinal,bound.segment_ordinal,model.as_ref()).ok().is_some_and(|expected|serde_json::to_vec(&expected).ok()==serde_json::to_vec(bound).ok())
    }
    pub fn is_operator_eight_step(&self) -> bool { self.body.lead_in_policy.is_operator() }
    pub fn is_variable_sources(&self) -> bool { self.body.profile_id == OPERATOR_VARIABLE_PROFILE }
    pub fn profile_id(&self) -> &str { self.body.profile_id }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn context(&self) -> &CaptureContext {
        &self.body.context
    }
    pub fn episodes(&self) -> &[Episode] {
        &self.body.episodes
    }
    pub fn counts(&self) -> &ScheduleCounts {
        &self.body.counts
    }
    pub fn resources(&self) -> &ResourcePreflight {
        &self.body.resources
    }
    /// Identity binding only. Future Runtime MUST verify owner/checkpoint/current
    /// step/conditions and the actual frozen managed artifact before calling.
    pub fn bind_episode_start(
        &self,
        run_id: &str,
        index: usize,
        attempt: u16,
        segment: u16,
        frozen: Option<&FrozenModelIdentity>,
    ) -> Result<BoundEpisode, String> {
        if !bounded_id(run_id) {
            return Err("managed_schedule_invalid_run".into());
        }
        let episode = self
            .body
            .episodes
            .get(index)
            .ok_or("managed_schedule_episode_missing")?;
        match episode.role {
            DataRole::Train if frozen.is_some() => {
                return Err("managed_schedule_training_is_frozen".into())
            }
            DataRole::HeldOut => {
                let model = frozen.ok_or("managed_schedule_frozen_model_required")?;
                if model.contract_sha256 != self.sha256
                    || model.context_digest != self.body.context.digest()
                    || !sha(&model.artifact_sha256)
                {
                    return Err("managed_schedule_frozen_model_mismatch".into());
                }
            }
            DataRole::Train => {}
            DataRole::Validation if self.is_operator_eight_step() && frozen.is_none() => {}
            _ => return Err("managed_schedule_invalid_role".into()),
        }
        let identity = |kind: &str, parts: &[serde_json::Value]| -> Result<String, String> {
            let bytes = serde_json::to_vec(&(
                SCHEMA,
                kind,
                &self.sha256,
                run_id,
                &episode.episode_key,
                parts,
            ))
            .map_err(|_| "managed_schedule_encoding")?;
            Ok(format!("{kind}-{}", digest(&bytes)))
        };
        let trial = identity("trial", &[])?;
        Ok(BoundEpisode {
            contract_sha256: self.sha256.clone(),
            episode: episode.clone(),
            episode_id: identity("episode", &[])?,
            trial_id: trial,
            session_id: identity("session", &[attempt.into(), segment.into()])?,
            attempt_id: identity("attempt", &[attempt.into()])?,
            attempt_ordinal: attempt,
            segment_ordinal: segment,
            frozen_artifact_sha256: frozen.map(|m| m.artifact_sha256.clone()),
        })
    }
}
/// Opaque, non-Deserialize binding supplied only by the future managed Runtime.
/// Hash validity is not proof of fitting, frozen state or filesystem custody.
#[derive(Debug, Clone)]
pub struct FrozenModelIdentity {
    artifact_sha256: String,
    contract_sha256: String,
    context_digest: String,
}
impl FrozenModelIdentity {
    pub(crate) fn from_runtime_verified_artifact(
        plan: &ManagedScheduleContract,
        artifact_sha256: String,
    ) -> Result<Self, String> {
        if !sha(&artifact_sha256) {
            return Err("managed_schedule_invalid_artifact_identity".into());
        }
        Ok(Self {
            artifact_sha256,
            contract_sha256: plan.sha256.clone(),
            context_digest: plan.body.context.digest(),
        })
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct BoundEpisode {
    contract_sha256: String,
    episode: Episode,
    episode_id: String,
    trial_id: String,
    session_id: String,
    attempt_id: String,
    attempt_ordinal: u16,
    segment_ordinal: u16,
    frozen_artifact_sha256: Option<String>,
}
impl BoundEpisode {
    pub fn contract_sha256(&self) -> &str {
        &self.contract_sha256
    }
    pub fn episode(&self) -> &Episode {
        &self.episode
    }
    pub fn episode_id(&self) -> &str {
        &self.episode_id
    }
    pub fn trial_id(&self) -> &str {
        &self.trial_id
    }
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }
    pub fn attempt_ordinal(&self) -> u16 {
        self.attempt_ordinal
    }
    pub fn segment_ordinal(&self) -> u16 {
        self.segment_ordinal
    }
    pub fn frozen_artifact_sha256(&self) -> Option<&str> {
        self.frozen_artifact_sha256.as_deref()
    }
}

#[cfg(test)]
#[path = "managed_schedule_contract_tests.rs"]
mod tests;
