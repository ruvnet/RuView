//! Explicit durable selection is distinct from candidate discovery and current usability.
use super::{
    model::PresenceDecision, observation::CaptureContext, qualification::QualificationEnvelope,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SelectionTarget {
    pub context: CaptureContext,
    pub model_sha256: String,
    pub model_version: u32,
    pub qualification_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SelectionReceipt {
    pub schema: String,
    pub owner_scope: String,
    pub installation_id: String,
    pub revision: u64,
    pub selected_at_unix_ms: u64,
    pub selected: Option<SelectionTarget>,
    pub previous: Option<SelectionTarget>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SelectionEnvelope {
    pub receipt_sha256: String,
    pub receipt: SelectionReceipt,
}

/// Bounded identity suitable for read-scoped paired phones; no owner identifier/raw context.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SelectedModelIdentity {
    pub selection_sha256: String,
    pub selection_revision: u64,
    pub model_sha256: String,
    pub model_version: u32,
    pub qualification_sha256: String,
    pub policy_sha256: String,
    pub context_digest: String,
    pub room_digest: String,
    pub source_node_ids: Vec<u8>,
    pub selected_at_unix_ms: u64,
    pub evaluated_at_unix_ms: u64,
    pub authority_scope: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SelectedBinaryEvidence {
    None,
    Unavailable {
        identity: Option<SelectedModelIdentity>,
        reason: String,
    },
    Ready {
        identity: SelectedModelIdentity,
        decision: PresenceDecision,
        presence: bool,
        occupied_probability: f32,
        observed_at_unix_ms: u64,
        observation_digest: String,
    },
}
#[derive(Debug, Clone, Serialize)]
pub struct QualificationSummary {
    #[serde(skip_serializing_if="Option::is_none")]
    pub schema: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub training_lineage_basis: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub managed_training_proof_sha256: Option<String>,
    pub receipt_sha256: String,
    pub model_sha256: String,
    pub context_digest: String,
    pub room_digest: String,
    pub evaluated_at_unix_ms: u64,
    pub policy: super::qualification::QualificationPolicy,
    pub policy_sha256: String,
    pub label_provenance: String,
    pub criteria: Vec<super::qualification::ConditionCriterion>,
    pub authority_scope: String,
}
impl From<&QualificationEnvelope> for QualificationSummary {
    fn from(q: &QualificationEnvelope) -> Self {
        let r = &q.receipt;
        Self {
            schema: (r.schema=="ruview.local-operational-qualification.v2").then(||r.schema.clone()),
            training_lineage_basis:r.training_lineage_basis.clone(),
            managed_training_proof_sha256:r.managed_training_proof_sha256.clone(),
            receipt_sha256: q.receipt_sha256.clone(),
            model_sha256: r.model_sha256.clone(),
            context_digest: r.context_digest.clone(),
            room_digest: r.room_digest.clone(),
            evaluated_at_unix_ms: r.evaluated_at_unix_ms,
            policy: r.policy.clone(),
            policy_sha256: r.policy_sha256.clone(),
            label_provenance: r.label_provenance.clone(),
            criteria: r.criteria.clone(),
            authority_scope: r.authority_scope.clone(),
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct ModelSelectionStatus {
    pub schema: &'static str,
    pub revision: u64,
    pub selected: Option<SelectedModelIdentity>,
    pub previous_available: bool,
    pub qualification: Option<QualificationSummary>,
    pub qualification_error: Option<String>,
    pub integrity_error: Option<String>,
    pub current: SelectedBinaryEvidence,
}
