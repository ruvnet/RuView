//! Strict whole-Temporal public DTOs. These are read models/command inputs,
//! never Complete/evaluation/selection capabilities. V1 producers are unchanged.
pub const VARIABLE_SOURCE_PROFILE_ID: &str = super::managed_schedule_contract::OPERATOR_VARIABLE_PROFILE;
use super::{
    lifecycle::MutationGuard,
    observation::{DataRole, RoomLabel},
    training::EvaluationReport,
};
use serde::{Deserialize, Serialize};
pub const STATUS_SCHEMA: &str = "ruview.room-learner.status.v2";
pub const SELECTION_SCHEMA: &str = "ruview.room-learner.selection.v2";
pub const SELECTION_STATUS_SCHEMA: &str = "ruview.room-learner.model-selection-status.v2";
pub const SELECTED_BINARY_SCHEMA: &str = "ruview.room-learner.selected-binary-evidence.v2";
pub const QUALIFICATION_SCHEMA: &str = "ruview.local-operational-qualification.v3";
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WholeTemporalKind {
    #[serde(rename = "ruview.room-learner.managed-temporal-input.v3")]
    ManagedTemporalV3,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalModelIdentityWire {
    pub model_kind: WholeTemporalKind,
    pub model_sha256: String,
    pub verified_fit_record_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalCompleteRefsWire {
    pub model: TemporalModelIdentityWire,
    pub training_proof_sha256: String,
    pub complete_record_sha256: String,
    pub complete_commitment_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalEvaluationRefsWire {
    pub complete: TemporalCompleteRefsWire,
    pub held_out_evidence_sha256: String,
    pub evaluation_receipt_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalQualificationRefsWire {
    pub evaluation: TemporalEvaluationRefsWire,
    pub qualification_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalOriginalDeadlineWire {
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalEpisodeWire {
    pub index: u8,
    pub role: DataRole,
    pub label: RoomLabel,
    pub target_ms: u64,
    pub lead_in_ms: u64,
    pub native_windows: u16,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalProfileWire {
    pub profile_id: String,
    pub window_unit: String,
    pub native_window_ms: u16,
    pub train_windows: u16,
    pub held_out_windows: u16,
    pub train_episodes: u8,
    pub held_out_episodes: u8,
    pub windows_per_condition: [u16; 4],
    pub train_episodes_per_condition: [u8; 4],
    pub held_out_episodes_per_condition: [u8; 4],
    pub episodes: Vec<TemporalEpisodeWire>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalCaptureWire {
    pub capture_generation_id: String,
    pub schedule_sha256: String,
    pub continuation_policy_sha256: String,
    pub durable_commitment_sha256: Option<String>,
    pub original_deadline: TemporalOriginalDeadlineWire,
    pub receiver_epoch: String,
    pub stage: String,
    pub current_episode: Option<u8>,
    pub role: Option<DataRole>,
    pub label: Option<RoomLabel>,
    pub target_ms: Option<u64>,
    pub lead_in_remaining_ms: u64,
    pub observed_ms: u64,
    pub retained_windows: u16,
    pub durable_windows: u16,
    pub sealed_train_episodes: u8,
    pub sealed_held_out_episodes: u8,
    pub durability_pending: bool,
    pub awaiting_ground_truth: bool,
    pub confirmation_challenge: Option<String>,
    pub failure: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemporalEvaluationWire {
    pub refs: TemporalEvaluationRefsWire,
    pub window_unit: String,
    pub evaluated_at_unix_ms: u64,
    pub report: EvaluationReport,
    pub durable: bool,
    pub independence_verified: bool,
    pub physically_validated: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowKindWire {
    Idle,
    Legacy,
    ManagedTemporalV3,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemporalLearnerStatusWire {
    pub schema: String,
    pub enabled: bool,
    pub available_model_kinds: Vec<WholeTemporalKind>,
    pub temporal_profile: TemporalProfileWire,
    pub workflow_kind: WorkflowKindWire,
    pub mutation_guard: MutationGuard,
    pub context_digest: Option<String>,
    pub room_digest: Option<String>,
    pub source_node_ids: Vec<u8>,
    pub recovery: Option<TemporalRecoveryStatusWire>,
    pub capture: Option<TemporalCaptureWire>,
    pub candidate: Option<TemporalCandidateIdentityWire>,
    pub training_proof_sha256: Option<String>,
    pub complete: Option<TemporalCompleteRefsWire>,
    pub evaluation: Option<TemporalEvaluationWire>,
    pub assessment: Option<TemporalAssessmentWire>,
    pub model_selection: Option<TemporalSelectionStatusWire>,
    pub job_state: String,
    pub job_operation: Option<String>,
    pub error_code: Option<String>,
    pub error: Option<String>,
    pub legacy_status: Option<serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalConfigureRequestV2 {
    pub schema: String,
    pub model_kind: WholeTemporalKind,
    pub expected: MutationGuard,
    pub binding_digest: String,
    pub revision: u64,
    pub source_node_ids: Vec<u8>,
    pub new_training: bool,
    pub recovery_consent: bool,
    #[serde(default)]
    pub profile_id: Option<String>,
}

pub const RECOVERY_RETRY_REQUEST_SCHEMA: &str = "ruview.room-learner.recovery-retry.v2";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalRetryRequestV2 {
    pub schema: String,
    pub model_kind: WholeTemporalKind,
    pub expected: MutationGuard,
    pub capture_generation_id: String,
    pub retry_confirmed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalPhaseStartRequestV2 {
    pub schema: String,
    pub expected: MutationGuard,
    pub capture_generation_id: String,
    pub schedule_sha256: String,
    pub episode_index: u8,
    pub conditions_confirmed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalPhaseFinishRequestV2 {
    pub schema: String,
    pub expected: MutationGuard,
    pub capture_generation_id: String,
    pub schedule_sha256: String,
    pub episode_index: u8,
    pub confirmation_challenge: String,
    pub conditions_confirmed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalEvaluateRequestV2 {
    pub schema: String,
    pub expected: MutationGuard,
    pub capture_generation_id: String,
    pub refs: TemporalCompleteRefsWire,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalQualifyRequestV2 {
    pub schema: String,
    pub expected: MutationGuard,
    pub capture_generation_id: String,
    pub refs: TemporalEvaluationRefsWire,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalSelectRequestV2 {
    pub schema: String,
    pub expected: MutationGuard,
    pub capture_generation_id: String,
    pub refs: TemporalQualificationRefsWire,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalCandidateIdentityWire {
    pub model_kind: WholeTemporalKind,
    pub model_sha256: String,
    pub verified_fit_record_sha256: String,
    pub context_digest: String,
    pub room_digest: String,
    pub persisted: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemporalQualificationPolicyWire {
    pub id: String,
    pub window_unit: String,
    pub minimum_windows_per_condition: u16,
    pub minimum_unconditional_sensitivity: f64,
    pub maximum_false_positive_rate: f64,
    pub minimum_evaluated_window_coverage: f64,
    pub coverage_scope: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemporalConditionCriterionWire {
    pub condition: RoomLabel,
    pub train_windows: u16,
    pub held_out_windows: u16,
    pub scored_windows: u16,
    pub abstained_windows: u16,
    /// Subset of abstained, not an additional exclusion from its denominator.
    pub missing_windows: u16,
    pub declared_train_episodes: u8,
    pub declared_held_out_episodes: u8,
    pub evaluated_window_coverage: f64,
    pub unconditional_sensitivity: Option<f64>,
    pub false_positive_rate: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemporalQualificationSummaryWire {
    pub schema: String,
    pub receipt_sha256: String,
    pub model_kind: WholeTemporalKind,
    pub model_sha256: String,
    pub verified_fit_record_sha256: String,
    pub training_lineage_basis: String,
    pub training_proof_sha256: String,
    pub held_out_evidence_sha256: String,
    pub complete_record_sha256: String,
    pub evaluation_receipt_sha256: String,
    pub context_digest: String,
    pub room_digest: String,
    pub evaluated_at_unix_ms: u64,
    pub policy: TemporalQualificationPolicyWire,
    pub policy_sha256: String,
    pub label_provenance: String,
    pub criteria: Vec<TemporalConditionCriterionWire>,
    pub authority_scope: String,
    pub window_unit: String,
    pub train_windows: u16,
    pub held_out_windows: u16,
    pub train_episodes: u8,
    pub held_out_episodes: u8,
    pub independence_verified: bool,
    pub physically_validated: bool,
    pub measurement_clock_available: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalSelectedIdentityWire {
    pub selection_sha256: String,
    pub selection_revision: u64,
    pub model_kind: WholeTemporalKind,
    pub model_sha256: String,
    pub verified_fit_record_sha256: String,
    pub training_proof_sha256: String,
    pub held_out_evidence_sha256: String,
    pub complete_record_sha256: String,
    pub evaluation_receipt_sha256: String,
    pub qualification_sha256: String,
    pub policy_sha256: String,
    pub context_digest: String,
    pub room_digest: String,
    pub source_node_ids: Vec<u8>,
    pub selected_at_unix_ms: u64,
    pub evaluated_at_unix_ms: u64,
    pub authority_scope: String,
}

pub const CONFIGURE_REQUEST_SCHEMA: &str = "ruview.room-learner.configure.v2";
pub const PHASE_START_REQUEST_SCHEMA: &str = "ruview.room-learner.phase-start.v2";
pub const PHASE_FINISH_REQUEST_SCHEMA: &str = "ruview.room-learner.phase-finish.v2";
pub const TRAIN_REQUEST_SCHEMA: &str = "ruview.room-learner.train.v2";
pub const EVALUATE_REQUEST_SCHEMA: &str = "ruview.room-learner.evaluate.v2";
pub const QUALIFY_REQUEST_SCHEMA: &str = "ruview.room-learner.qualify.v2";
pub const SELECT_REQUEST_SCHEMA: &str = "ruview.room-learner.select.v2";
pub const RECOVERY_REQUEST_SCHEMA: &str = "ruview.room-learner.recovery.v2";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalTrainRequestV2 {
    pub schema: String,
    pub expected: MutationGuard,
    pub capture_generation_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalRecoveryRequestV2 {
    pub schema: String,
    pub model_kind: WholeTemporalKind,
    pub expected: MutationGuard,
    pub binding_digest: String,
    pub revision: u64,
    pub source_node_ids: Vec<u8>,
    pub capture_generation_id: String,
    #[serde(default)]
    pub profile_id: Option<String>,
}

pub const TEMPORAL_POLICY_ID: &str = "ruview.local-operational-temporal-policy.v1";
pub const OPERATOR_EIGHT_STEP_TEMPORAL_POLICY_ID: &str = "ruview.local-operational-temporal-policy.operator-eight-step.v1";
impl TemporalQualificationPolicyWire {
    pub(crate) fn for_operator_eight_step(operator: bool) -> Self {
        let mut policy = Self::current();
        if operator {
            policy.id = OPERATOR_EIGHT_STEP_TEMPORAL_POLICY_ID.into();
            policy.minimum_windows_per_condition = 4;
        }
        policy
    }
    pub(crate) fn current() -> Self {
        let numeric = super::qualification::QualificationPolicy::default();
        Self {
            id: TEMPORAL_POLICY_ID.into(),
            window_unit: "native_receiver_2s_window".into(),
            minimum_windows_per_condition: 16,
            minimum_unconditional_sensitivity: numeric.minimum_unconditional_sensitivity,
            maximum_false_positive_rate: numeric.maximum_false_positive_rate,
            minimum_evaluated_window_coverage: numeric.minimum_evaluated_cohort_coverage,
            coverage_scope: "declared_native_windows_not_wall_time".into(),
        }
    }
}
#[cfg(test)]
#[path = "temporal_wire_tests.rs"]
mod tests;

pub const PHASE_CANCEL_REQUEST_SCHEMA: &str = "ruview.room-learner.phase-cancel.v2";
pub const TRAIN_CANCEL_REQUEST_SCHEMA: &str = "ruview.room-learner.train-cancel.v2";
pub const RECOVERY_RESTORE_REQUEST_SCHEMA: &str = "ruview.room-learner.recovery-restore.v2";
pub const RECOVERY_DISCARD_REQUEST_SCHEMA: &str = "ruview.room-learner.recovery-discard.v2";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalCurrentActionRequestV2 {
    pub schema: String,
    pub expected: MutationGuard,
    pub capture_generation_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalDiscardRequestV2 {
    pub schema: String,
    pub model_kind: WholeTemporalKind,
    pub expected: MutationGuard,
    pub capture_generation_id: String,
}

impl TemporalProfileWire {
    pub(crate) fn variable_sources() -> Self {
        let mut profile = Self::operator_eight_step();
        profile.profile_id = super::managed_schedule_contract::OPERATOR_VARIABLE_PROFILE.into();
        profile
    }
    pub(crate) fn operator_eight_step() -> Self {
        let labels=[RoomLabel::Empty,RoomLabel::OccupiedStatic,RoomLabel::OccupiedMoving,RoomLabel::AdjacentOnly];
        Self {profile_id:super::managed_schedule_contract::OPERATOR_EIGHT_PROFILE.into(),window_unit:"native_receiver_2s_window".into(),native_window_ms:2000,
            train_windows:64,held_out_windows:16,train_episodes:4,held_out_episodes:4,windows_per_condition:[16;4],train_episodes_per_condition:[1;4],held_out_episodes_per_condition:[1;4],
            episodes:labels.into_iter().flat_map(|label| [DataRole::Train,DataRole::Validation].into_iter().map(move |role|(label,role))).enumerate().map(|(index,(label,role))|TemporalEpisodeWire{
                index:index as u8,role,label,target_ms:if role==DataRole::Validation{15_000}else if label==RoomLabel::Empty{600_000}else{60_000},lead_in_ms:if matches!(label,RoomLabel::Empty|RoomLabel::AdjacentOnly){15_000}else{5_000},native_windows:if role==DataRole::Train{16}else{4}
            }).collect()}
    }
    pub(crate) fn engineering() -> Self {
        let labels = [
            RoomLabel::Empty,
            RoomLabel::OccupiedStatic,
            RoomLabel::OccupiedMoving,
            RoomLabel::AdjacentOnly,
        ];
        let order = [0, 1, 1, 2, 2, 3, 3, 0, 0, 1, 1, 2, 2, 3, 3];
        Self {
            profile_id: "native-2s-three-source-56bin-option-a-engineering-v1".into(),
            window_unit: "native_receiver_2s_window".into(),
            native_window_ms: 2000,
            train_windows: 64,
            held_out_windows: 64,
            train_episodes: 7,
            held_out_episodes: 8,
            windows_per_condition: [16; 4],
            train_episodes_per_condition: [1, 2, 2, 2],
            held_out_episodes_per_condition: [2; 4],
            episodes: order
                .iter()
                .enumerate()
                .map(|(i, n)| TemporalEpisodeWire {
                    index: i as u8,
                    role: if i < 7 {
                        DataRole::Train
                    } else {
                        DataRole::HeldOut
                    },
                    label: labels[*n],
                    target_ms: if i == 0 { 600000 } else { 30000 },
                    lead_in_ms: if i < 7 { 5000 } else { 15000 },
                    native_windows: if i == 0 { 16 } else { 8 },
                })
                .collect(),
        }
    }
}

pub const ASSESSMENT_SCHEMA: &str = "ruview.local-operational-assessment.v2";
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemporalConditionFailureWire {
    pub condition: RoomLabel,
    pub codes: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemporalAssessmentWire {
    pub schema: String,
    pub receipt_sha256: String,
    pub model_kind: WholeTemporalKind,
    pub model_sha256: String,
    pub verified_fit_record_sha256: String,
    pub training_proof_sha256: String,
    pub held_out_evidence_sha256: String,
    pub complete_record_sha256: String,
    pub evaluation_receipt_sha256: String,
    pub context_digest: String,
    pub room_digest: String,
    pub evaluated_at_unix_ms: u64,
    pub assessed_at_unix_ms: u64,
    pub policy: TemporalQualificationPolicyWire,
    pub policy_sha256: String,
    pub window_unit: String,
    pub outcome: String,
    pub failure_reason: Option<String>,
    pub qualification_sha256: Option<String>,
    pub criteria: Vec<TemporalConditionCriterionWire>,
    pub condition_failures: Vec<TemporalConditionFailureWire>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum TemporalCurrentEvidenceWire {
    None,
    Unavailable {
        identity: Option<TemporalSelectedIdentityWire>,
        reason: String,
        boot_epoch: Option<String>,
        receiver_epoch: Option<String>,
    },
    Ready {
        identity: TemporalSelectedIdentityWire,
        decision: super::model::PresenceDecision,
        presence: bool,
        occupied_probability: f32,
        observed_at_unix_ms: u64,
        observation_digest: String,
        boot_epoch: String,
        receiver_epoch: String,
        window_start_unix_ms: u64,
        window_end_unix_ms: u64,
        valid_until_unix_ms: u64,
        source_observed_at_unix_ms: u64,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemporalSelectionStatusWire {
    pub schema: String,
    pub revision: u64,
    pub selected: Option<TemporalSelectedIdentityWire>,
    pub previous_available: bool,
    pub qualification: Option<TemporalQualificationSummaryWire>,
    pub qualification_error: Option<String>,
    pub integrity_error: Option<String>,
    pub current: TemporalCurrentEvidenceWire,
}

pub const PAIRED_PROGRESS_SCHEMA: &str = "ruview.room-learner.progress.v2";
#[derive(Debug, Clone, Serialize)]
pub struct TemporalPairedCaptureProgressWire {
    pub capture_generation_id: String,
    pub schedule_sha256: String,
    pub stage: String,
    pub current_episode: Option<u8>,
    pub role: Option<DataRole>,
    pub label: Option<RoomLabel>,
    pub target_ms: Option<u64>,
    pub lead_in_remaining_ms: u64,
    pub observed_ms: u64,
    pub retained_windows: u16,
    pub durable_windows: u16,
    pub sealed_train_episodes: u8,
    pub sealed_held_out_episodes: u8,
    pub durability_pending: bool,
    pub awaiting_ground_truth: bool,
    pub original_deadline: TemporalOriginalDeadlineWire,
}
#[derive(Debug, Clone, Serialize)]
pub struct TemporalPairedSourcesWire {
    pub required: u8,
    pub ready: u8,
}
#[derive(Debug, Clone, Serialize)]
pub struct TemporalPairedProgressWire {
    pub schema: String,
    pub success: bool,
    pub installation_id: String,
    /// Existing public calibration boot identity, never owner MutationGuard.
    pub runtime_epoch: String,
    pub observed_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub room: Option<super::lifecycle::PublishedProgressRoom>,
    pub context_match: bool,
    pub stage: String,
    pub state: String,
    pub model_kind: Option<WholeTemporalKind>,
    pub profile: Option<TemporalProfileWire>,
    pub capture: Option<TemporalPairedCaptureProgressWire>,
    pub candidate: Option<TemporalCandidateIdentityWire>,
    pub evaluation: Option<TemporalEvaluationWire>,
    pub qualification: Option<TemporalQualificationSummaryWire>,
    pub selection: Option<TemporalSelectionStatusWire>,
    pub sources: Option<TemporalPairedSourcesWire>,
    pub save_status: String,
    pub blocker: Option<String>,
}

pub const DESELECT_REQUEST_SCHEMA: &str = "ruview.room-learner.deselect.v2";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalDeselectRequestV2 {
    pub schema: String,
    pub model_kind: WholeTemporalKind,
    pub expected: MutationGuard,
    pub selection_sha256: String,
    pub selection_revision: u64,
}

pub const RECOVERY_STATUS_SCHEMA: &str = "ruview.room-learner.recovery-status.v2";
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalRecoveryStatusWire {
    pub schema: String,
    pub state: String,
    pub capture_generation_id: Option<String>,
    pub context_digest: Option<String>,
    pub original_deadline: Option<TemporalOriginalDeadlineWire>,
}
