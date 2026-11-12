//! Whole Temporal evaluation DATA. No decoded producer or runtime/store permit,
//! model accessor, evaluator invocation or qualification/adoption authority.
//! Successful decoding validates only bounded metadata and its integrity links.
use super::{
    observation::{CaptureProvenance, ClockKind, DataRole},
    training::EvaluationReport,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(super) const TRAIN_SCHEMA: &str = "ruview.room-learner.temporal-training-proof.v1";
pub(super) const HELD_OUT_SCHEMA: &str = "ruview.room-learner.temporal-heldout-evidence.v1";
pub(super) const EVALUATION_SCHEMA: &str = "ruview.room-learner.temporal-evaluation.v1";
pub(super) const OPERATOR_TRAIN_SCHEMA: &str = "ruview.room-learner.temporal-training-proof.operator-eight-step.v1";
pub(super) const OPERATOR_CHECK_SCHEMA: &str = "ruview.room-learner.temporal-validation-evidence.operator-eight-step.v1";
pub(super) const OPERATOR_EVALUATION_SCHEMA: &str = "ruview.room-learner.temporal-evaluation.operator-eight-step.v1";
const MODEL_KIND: &str = "ruview.room-learner.managed-temporal-input.v3";
const PREFIX_SCHEMA: &str = "ruview.room-learner.temporal-runtime-prefix.v3";
const METADATA_KIND: &str = "temporal-prefix-commitment-v2";
pub(super) const MAX_METADATA_BYTES: usize = 64 * 1024;
const TTL_MS: u64 = 86_400_000;
fn invalid<T>() -> Result<T, String> {
    Err("temporal_evaluation_data_invalid".into())
}
fn sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.is_ascii()
        && value.trim() == value
        && !value.bytes().any(|b| b.is_ascii_control())
}
fn decimal(value: &str) -> Result<u64, String> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|b| b.is_ascii_digit())
        || (value.starts_with('0') && value != "0")
    {
        return invalid();
    }
    value
        .parse()
        .map_err(|_| "temporal_evaluation_data_integer".into())
}
fn decimal_frame(value: &str) -> Result<u128, String> {
    if value.is_empty()
        || value.len() > 39
        || !value.bytes().all(|b| b.is_ascii_digit())
        || (value.starts_with('0') && value != "0")
    {
        return invalid();
    }
    value
        .parse()
        .map_err(|_| "temporal_evaluation_data_frame_integer".into())
}
fn data_sha<T: Serialize>(schema: &str, body: &T) -> Result<String, String> {
    let bytes =
        serde_json::to_vec(&(schema, body)).map_err(|_| "temporal_evaluation_data_encoding")?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn decode_data<T: DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T, String> {
    if bytes.is_empty() || bytes.len() > MAX_METADATA_BYTES {
        return invalid();
    }
    let value: T = serde_json::from_slice(bytes).map_err(|_| "temporal_evaluation_data_schema")?;
    if serde_json::to_vec(&value).map_err(|_| "temporal_evaluation_data_encoding")? != bytes {
        return invalid();
    }
    Ok(value)
}

/// Stable original identity ONLY; intentionally excludes mutable saved_at_ms.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct OriginalDeadlineData {
    pub id: String,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}
impl OriginalDeadlineData {
    fn validate_data(&self) -> Result<(), String> {
        if !identifier(&self.id)
            || self.created_at_unix_ms == 0
            || self.expires_at_unix_ms > 253_402_300_799_999
            || self.created_at_unix_ms.checked_add(TTL_MS) != Some(self.expires_at_unix_ms)
        {
            return invalid();
        }
        Ok(())
    }
    fn contains_receiver_ns(&self, ns: u64) -> bool {
        ns > 0
            && ns / 1_000_000 >= self.created_at_unix_ms
            && ns / 1_000_000 < self.expires_at_unix_ms
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct TemporalEvaluationIdentityData {
    pub owner_scope: String,
    pub installation_id: String,
    pub context_digest: String,
    pub room_digest: String,
    pub run: String,
    pub original_deadline: OriginalDeadlineData,
    pub artifact_kind: String,
    pub artifact_sha256: String,
    pub verified_fit_record_sha256: String,
    pub prefix_schema: String,
    pub metadata_kind: String,
    pub continuation_policy_sha256: String,
    pub input_policy_sha256: String,
    pub execution_policy_sha256: String,
    pub schedule_sha256: String,
    pub configured_fit_sha256: String,
    pub source_ids: Vec<u8>,
}
impl TemporalEvaluationIdentityData {
    fn validate_data(&self) -> Result<(), String> {
        self.original_deadline.validate_data()?;
        if [&self.owner_scope, &self.installation_id, &self.run]
            .iter()
            .any(|s| !identifier(s))
            || [
                &self.context_digest,
                &self.room_digest,
                &self.artifact_sha256,
                &self.verified_fit_record_sha256,
                &self.continuation_policy_sha256,
                &self.input_policy_sha256,
                &self.execution_policy_sha256,
                &self.schedule_sha256,
                &self.configured_fit_sha256,
            ]
            .iter()
            .any(|s| !sha(s))
            || self.artifact_kind != MODEL_KIND
            || self.prefix_schema != PREFIX_SCHEMA
            || self.metadata_kind != METADATA_KIND
            || !(2..=8).contains(&self.source_ids.len())
            || self.source_ids.windows(2).any(|w| w[0] >= w[1])
        {
            return invalid();
        }
        Ok(())
    }
}
/// Projection, never a deserializable TemporalToken/storage permission.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct CommitmentData {
    pub generation: u64,
    pub revision: u16,
    pub run: String,
    pub id: String,
    pub commitment_sha256: String,
}
impl CommitmentData {
    fn validate_data(&self, identity: &TemporalEvaluationIdentityData) -> Result<(), String> {
        if self.revision >= 433
            || self.run != identity.run
            || self.id != identity.original_deadline.id
            || !sha(&self.commitment_sha256)
        {
            return invalid();
        }
        Ok(())
    }
}
/// Negative descriptive claims only; no field can grant authority.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct DiagnosticClaimsData {
    pub independence_verified: bool,
    pub physically_validated: bool,
    pub qualification_granted: bool,
    pub adoption_authorized: bool,
    pub measurement_clock_available: bool,
}
impl DiagnosticClaimsData {
    fn validate_data(&self) -> Result<(), String> {
        if self.independence_verified
            || self.physically_validated
            || self.qualification_granted
            || self.adoption_authorized
            || self.measurement_clock_available
        {
            return invalid();
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct FrozenSourceFloorData {
    pub source_id: u8,
    pub sequence: u64,
    pub receiver_ns: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct TemporalTrainProofBodyData {
    pub identity: TemporalEvaluationIdentityData,
    pub training_data_sha256: String,
    pub training_evidence_sha256: String,
    pub training_identities_sha256: String,
    pub train_seal_sha256s: Vec<String>,
    pub train_seal_chain_sha256: String,
    pub train_windows: u16,
    pub windows_per_condition: [u16; 4],
    pub declared_episodes_per_condition: [u16; 4],
    pub accepted_train_commitment: CommitmentData,
    pub freeze_prepare_receiver_ns: String,
    pub source_floors: Vec<FrozenSourceFloorData>,
    pub claims: DiagnosticClaimsData,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct TemporalTrainProofData {
    pub schema: String,
    pub body: TemporalTrainProofBodyData,
    pub sha256: String,
}
impl TemporalTrainProofData {
    pub(super) fn is_operator_eight_step(&self) -> bool { self.schema == OPERATOR_TRAIN_SCHEMA }
    pub(super) fn decode_data(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = decode_data(bytes)?;
        value.validate_data()?;
        Ok(value)
    }
    pub(super) fn validate_data(&self) -> Result<(), String> {
        let b = &self.body;
        b.identity.validate_data()?;
        b.claims.validate_data()?;
        b.accepted_train_commitment.validate_data(&b.identity)?;
        let cut = decimal(&b.freeze_prepare_receiver_ns)?;
        if ![TRAIN_SCHEMA, OPERATOR_TRAIN_SCHEMA].contains(&self.schema.as_str())
            || !sha(&self.sha256)
            || self.sha256 != data_sha(&self.schema, b)?
            || [
                &b.training_data_sha256,
                &b.training_evidence_sha256,
                &b.training_identities_sha256,
                &b.train_seal_chain_sha256,
            ]
            .iter()
            .any(|s| !sha(s))
            || b.train_seal_sha256s.iter().any(|s| !sha(s))
            || b.train_seal_sha256s
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != if self.is_operator_eight_step() { 4 } else { 7 }
            || b.train_seal_sha256s.len() != if self.is_operator_eight_step() { 4 } else { 7 }
            || b.source_floors.len() != b.identity.source_ids.len()
            || b.train_windows != 64
            || b.windows_per_condition != [16; 4]
            || b.declared_episodes_per_condition != if self.is_operator_eight_step() { [1; 4] } else { [1, 2, 2, 2] }
            || !b.identity.original_deadline.contains_receiver_ns(cut)
        {
            return invalid();
        }
        for (floor, id) in b.source_floors.iter().zip(b.identity.source_ids.iter().copied()) {
            let ns = decimal(&floor.receiver_ns)?;
            if floor.source_id != id || floor.sequence > u32::MAX as u64 || ns == 0 || ns > cut {
                return invalid();
            }
        }
        Ok(())
    }
}
/// DATA from the acknowledged Complete prefix's whole-period source_high.
/// This is not a future ACK/RAM floor, hardware epoch or live continuation permit.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct FinalSourceFenceData {
    pub source_id: u8,
    pub sequence: u64,
    pub receiver_ns: String,
    pub native_frame_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct TemporalHeldOutBodyData {
    pub identity: TemporalEvaluationIdentityData,
    pub training_proof_sha256: String,
    pub held_out_data_sha256: String,
    pub held_out_evidence_sha256: String,
    pub held_out_identities_sha256: String,
    pub complete_rows_sha256: String,
    pub complete_evidence_sha256: String,
    pub complete_identities_sha256: String,
    pub complete_ledger_sha256: String,
    pub held_out_seal_sha256s: Vec<String>,
    pub held_out_seal_chain_sha256: String,
    pub complete_record_sha256: String,
    pub complete_commitment: CommitmentData,
    pub complete_prepare_receiver_ns: String,
    pub complete_persisted_receiver_ns: String,
    pub final_source_fences: Vec<FinalSourceFenceData>,
    pub held_out_windows: u16,
    pub windows_per_condition: [u16; 4],
    pub declared_episodes_per_condition: [u16; 4],
    pub claims: DiagnosticClaimsData,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct TemporalHeldOutEvidenceData {
    pub schema: String,
    pub body: TemporalHeldOutBodyData,
    pub sha256: String,
}
impl TemporalHeldOutEvidenceData {
    pub(super) fn is_operator_eight_step(&self) -> bool { self.schema == OPERATOR_CHECK_SCHEMA }
    pub(super) fn decode_data(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = decode_data(bytes)?;
        value.validate_data()?;
        Ok(value)
    }
    pub(super) fn validate_data(&self) -> Result<(), String> {
        let b = &self.body;
        b.identity.validate_data()?;
        b.claims.validate_data()?;
        b.complete_commitment.validate_data(&b.identity)?;
        if ![HELD_OUT_SCHEMA, OPERATOR_CHECK_SCHEMA].contains(&self.schema.as_str())
            || !sha(&self.sha256)
            || self.sha256 != data_sha(&self.schema, b)?
            || [
                &b.training_proof_sha256,
                &b.held_out_data_sha256,
                &b.held_out_evidence_sha256,
                &b.held_out_identities_sha256,
                &b.complete_rows_sha256,
                &b.complete_evidence_sha256,
                &b.complete_identities_sha256,
                &b.complete_ledger_sha256,
                &b.held_out_seal_chain_sha256,
                &b.complete_record_sha256,
            ]
            .iter()
            .any(|s| !sha(s))
            || b.held_out_seal_sha256s.iter().any(|s| !sha(s))
            || b.held_out_seal_sha256s
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != if self.is_operator_eight_step() { 4 } else { 8 }
            || b.held_out_seal_sha256s.len() != if self.is_operator_eight_step() { 4 } else { 8 }
            || b.final_source_fences.len() != b.identity.source_ids.len()
            || b.held_out_windows != if self.is_operator_eight_step() { 16 } else { 64 }
            || b.windows_per_condition != if self.is_operator_eight_step() { [4; 4] } else { [16; 4] }
            || b.declared_episodes_per_condition != if self.is_operator_eight_step() { [1; 4] } else { [2; 4] }
            || !b
                .identity
                .original_deadline
                .contains_receiver_ns(decimal(&b.complete_prepare_receiver_ns)?)
        {
            return invalid();
        }
        let persisted = decimal(&b.complete_persisted_receiver_ns)?;
        if !b.identity.original_deadline.contains_receiver_ns(persisted)
            || persisted != decimal(&b.complete_prepare_receiver_ns)?
        {
            return invalid();
        }
        for (floor, id) in b.final_source_fences.iter().zip(b.identity.source_ids.iter().copied()) {
            let ns = decimal(&floor.receiver_ns)?;
            let frame = decimal_frame(&floor.native_frame_id)?;
            let expected =
                (u128::from(ns) << 40) | (u128::from(id) << 32) | u128::from(floor.sequence);
            if floor.source_id != id
                || floor.sequence > u32::MAX as u64
                || ns == 0
                || ns > persisted
                || frame != expected
            {
                return invalid();
            }
        }
        Ok(())
    }
    /// Metadata equality/integrity only, NOT a complete capture or evaluation permit.
    pub(super) fn validate_data_links(&self, train: &TemporalTrainProofData) -> Result<(), String> {
        self.validate_data()?;
        train.validate_data()?;
        if self.is_operator_eight_step() != train.is_operator_eight_step()
            || self.body.identity != train.body.identity
            || self.body.training_proof_sha256 != train.sha256
            || decimal(&self.body.complete_prepare_receiver_ns)?
                < decimal(&train.body.freeze_prepare_receiver_ns)?
            || self.body.complete_commitment.generation
                != train.body.accepted_train_commitment.generation
            || self.body.complete_commitment.revision
                <= train.body.accepted_train_commitment.revision
        {
            return invalid();
        }
        for (final_floor, train_floor) in self
            .body
            .final_source_fences
            .iter()
            .zip(&train.body.source_floors)
        {
            if final_floor.source_id != train_floor.source_id
                || final_floor.sequence < train_floor.sequence
                || decimal(&final_floor.receiver_ns)? < decimal(&train_floor.receiver_ns)?
            {
                return invalid();
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct TemporalEvaluationBodyData {
    pub identity: TemporalEvaluationIdentityData,
    pub training_proof_sha256: String,
    pub held_out_evidence_sha256: String,
    pub complete_record_sha256: String,
    pub training_data_sha256: String,
    pub held_out_data_sha256: String,
    pub role: DataRole,
    pub window_unit: String,
    pub evaluated_at_receiver_ns: String,
    pub evaluated_at_unix_ms: u64,
    pub provenance: Vec<CaptureProvenance>,
    pub clock_claims: Vec<ClockKind>,
    pub train_lineage_checked: bool,
    pub report: EvaluationReport,
    pub claims: DiagnosticClaimsData,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct TemporalEvaluationReceiptData {
    pub schema: String,
    pub body: TemporalEvaluationBodyData,
    pub sha256: String,
}
impl TemporalEvaluationReceiptData {
    pub(super) fn is_operator_eight_step(&self) -> bool { self.schema == OPERATOR_EVALUATION_SCHEMA }
    pub(super) fn decode_data(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = decode_data(bytes)?;
        value.validate_data()?;
        Ok(value)
    }
    pub(super) fn validate_data(&self) -> Result<(), String> {
        let b = &self.body;
        b.identity.validate_data()?;
        b.claims.validate_data()?;
        b.report.validate()?;
        let ns = decimal(&b.evaluated_at_receiver_ns)?;
        if ![EVALUATION_SCHEMA, OPERATOR_EVALUATION_SCHEMA].contains(&self.schema.as_str())
            || !sha(&self.sha256)
            || self.sha256 != data_sha(&self.schema, b)?
            || [
                &b.training_proof_sha256,
                &b.held_out_evidence_sha256,
                &b.complete_record_sha256,
                &b.training_data_sha256,
                &b.held_out_data_sha256,
            ]
            .iter()
            .any(|s| !sha(s))
            || b.role != if self.is_operator_eight_step() { DataRole::Validation } else { DataRole::HeldOut }
            || b.window_unit != "native_receiver_2s_window"
            || !b.train_lineage_checked
            || ns / 1_000_000 != b.evaluated_at_unix_ms
            || !b.identity.original_deadline.contains_receiver_ns(ns)
            || b.provenance != [CaptureProvenance::LiveReceiverObserved]
            || b.clock_claims != [ClockKind::ReceiverOnly]
            || b.report.samples != if self.is_operator_eight_step() { 16 } else { 64 }
            || b.report.declared_trial_count != if self.is_operator_eight_step() { 4 } else { 8 }
            || b.report
                .per_condition
                .iter()
                .any(|c| c.samples != if self.is_operator_eight_step() { 4 } else { 16 } || c.declared_trial_count != if self.is_operator_eight_step() { 1 } else { 2 })
        {
            return invalid();
        }
        Ok(())
    }
    pub(super) fn validate_data_links(
        &self,
        train: &TemporalTrainProofData,
        held: &TemporalHeldOutEvidenceData,
    ) -> Result<(), String> {
        self.validate_data()?;
        held.validate_data_links(train)?;
        let b = &self.body;
        if self.is_operator_eight_step() != train.is_operator_eight_step()
            || b.identity != held.body.identity
            || b.training_proof_sha256 != train.sha256
            || b.held_out_evidence_sha256 != held.sha256
            || b.complete_record_sha256 != held.body.complete_record_sha256
            || b.training_data_sha256 != train.body.training_data_sha256
            || b.held_out_data_sha256 != held.body.held_out_data_sha256
            || decimal(&b.evaluated_at_receiver_ns)?
                < decimal(&held.body.complete_persisted_receiver_ns)?
        {
            return invalid();
        }
        Ok(())
    }
}

// These constructors compute bounded DATA integrity only. They do not issue a
// producer, current-store, evaluation, qualification or selection permission.
impl TemporalTrainProofData {
    pub(super) fn from_body_data(body: TemporalTrainProofBodyData) -> Result<Self, String> {
        let schema = if body.train_seal_sha256s.len() == 4 && body.declared_episodes_per_condition == [1; 4] { OPERATOR_TRAIN_SCHEMA } else { TRAIN_SCHEMA };
        let value = Self {
            schema: schema.into(),
            sha256: data_sha(schema, &body)?,
            body,
        };
        value.validate_data()?;
        value.encode_data()?;
        Ok(value)
    }
    pub(super) fn encode_data(&self) -> Result<Vec<u8>, String> {
        self.validate_data()?;
        bounded_encoding(self)
    }
}
impl TemporalHeldOutEvidenceData {
    pub(super) fn from_body_data(body: TemporalHeldOutBodyData) -> Result<Self, String> {
        let schema = if body.held_out_windows == 16 && body.held_out_seal_sha256s.len() == 4 { OPERATOR_CHECK_SCHEMA } else { HELD_OUT_SCHEMA };
        let value = Self {
            schema: schema.into(),
            sha256: data_sha(schema, &body)?,
            body,
        };
        value.validate_data()?;
        value.encode_data()?;
        Ok(value)
    }
    pub(super) fn encode_data(&self) -> Result<Vec<u8>, String> {
        self.validate_data()?;
        bounded_encoding(self)
    }
}
impl TemporalEvaluationReceiptData {
    pub(super) fn from_body_data(body: TemporalEvaluationBodyData) -> Result<Self, String> {
        let schema = if body.role == DataRole::Validation { OPERATOR_EVALUATION_SCHEMA } else { EVALUATION_SCHEMA };
        let value = Self {
            schema: schema.into(),
            sha256: data_sha(schema, &body)?,
            body,
        };
        value.validate_data()?;
        value.encode_data()?;
        Ok(value)
    }
    pub(super) fn encode_data(&self) -> Result<Vec<u8>, String> {
        self.validate_data()?;
        bounded_encoding(self)
    }
}
impl DiagnosticClaimsData {
    pub(super) fn diagnostic_only() -> Self {
        Self {
            independence_verified: false,
            physically_validated: false,
            qualification_granted: false,
            adoption_authorized: false,
            measurement_clock_available: false,
        }
    }
}
fn bounded_encoding<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "temporal_evaluation_data_encoding")?;
    if bytes.is_empty() || bytes.len() > MAX_METADATA_BYTES {
        return invalid();
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "temporal_evaluation_data_tests.rs"]
mod tests;
