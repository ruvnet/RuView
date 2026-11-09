//! Private C4B role DATA and bounded shape checks.
//!
//! Nothing here issues a conditions, fit, write, evaluation or
//! qualification permission. The capture owner must also run the unchanged
//! receiver-quality/native/ledger predicates and actual writer-ACK admission.
use super::bridge::{Disposition, QualityPart};
use super::full_run::VerifiedFitRecordData;
use super::*;
use crate::room_learner::recovery_store::temporal_commitment::TemporalCommitmentData;

pub(super) const HELD_OUT_BRIDGE_DOMAIN: &str = "ruview.room-learner.temporal-heldout-bridge.v1";
pub(super) const HELD_OUT_SEAL_DOMAIN: &str = "ruview.room-learner.temporal-heldout-seal.v1";
pub(super) const HELD_OUT_CONDITIONS_DOMAIN: &str =
    "ruview.room-learner.temporal-heldout-conditions.v1";
pub(super) const FULL_COMPLETE_DOMAIN: &str = "ruview.room-learner.temporal-full-complete.v1";
const WHOLE_MODEL_KIND: &str = "ruview.room-learner.managed-temporal-input.v3";
const TRAIN_EPISODES: usize = 7;
const TRAIN_WINDOWS: usize = 64;
const HELD_OUT_EPISODES: usize = 8;
const WINDOWS_PER_HELD_OUT_EPISODE: usize = 8;
const FULL_WINDOWS: usize = 128;
const MAX_GLOBAL_GAPS: usize = 128;
const MAX_GLOBAL_PARTS: usize = 143;
const MAX_PARTS_PER_EPISODE: usize = 128;

fn digest64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Checks fixed schedule DATA only. It does not assert user conditions, actual
/// fitting, source-clock precision, independent trials or storage custody.
fn fixed_schedule(schedule: &ManagedScheduleContract) -> Result<(), String> {
    let counts = schedule.counts();
    if counts.train_episodes != TRAIN_EPISODES
        || counts.train_windows != TRAIN_WINDOWS
        || counts.held_out_episodes != HELD_OUT_EPISODES
        || counts.held_out_windows != TRAIN_WINDOWS
        || schedule.episodes().len() != TRAIN_EPISODES + HELD_OUT_EPISODES
        || schedule.episodes()[TRAIN_EPISODES..].iter().any(|episode| {
            episode.role != DataRole::HeldOut
                || episode.target_active_recording_ms != 30_000
                || episode.lead_in_ms != 15_000
                || episode.slots.len() != WINDOWS_PER_HELD_OUT_EPISODE
        })
    {
        return Err("temporal_heldout_fixed_schedule_changed".into());
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct HeldOutSealBody {
    pub(super) role: DataRole,
    pub(super) episode: usize,
    pub(super) trial_id: String,
    pub(super) context_sha256: String,
    pub(super) schedule_sha256: String,
    pub(super) continuation_policy_sha256: String,
    pub(super) verified_fit_sha256: String,
    pub(super) artifact_kind: String,
    pub(super) artifact_sha256: String,
    pub(super) row_prefix_sha256: String,
    pub(super) quality_sha256: String,
    pub(super) parts: Vec<QualityPart>,
    pub(super) confirmed_at_ns: u64,
    pub(super) previous_sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct HeldOutSealData {
    schema: String,
    pub(super) body: HeldOutSealBody,
    pub(super) sha256: String,
}
impl HeldOutSealData {
    /// Canonical DATA constructor, NOT a seal/conditions authority issuer.
    pub(super) fn from_body(body: HeldOutSealBody) -> Result<Self, String> {
        let sha256 = hash(&(HELD_OUT_SEAL_DOMAIN, &body))?;
        Ok(Self {
            schema: HELD_OUT_SEAL_DOMAIN.into(),
            body,
            sha256,
        })
    }
    /// Caller still has to verify exact native/raw prefixes and reused whole-
    /// period quality, then authenticate the actual current writer barrier.
    pub(super) fn validate_binding_data(
        &self,
        schedule: &ManagedScheduleContract,
        run: &str,
        fit: &VerifiedFitRecordData,
        now: u64,
    ) -> Result<(), String> {
        fixed_schedule(schedule)?;
        let body = &self.body;
        if self.schema != HELD_OUT_SEAL_DOMAIN
            || body.role != DataRole::HeldOut
            || !(TRAIN_EPISODES..TRAIN_EPISODES + HELD_OUT_EPISODES).contains(&body.episode)
            || fit.body.run != run
            || body.context_sha256 != schedule.context().digest()
            || body.context_sha256 != fit.body.context_digest
            || body.schedule_sha256 != schedule.sha256()
            || body.schedule_sha256 != fit.body.schedule_sha256
            || body.continuation_policy_sha256 != fit.body.continuation_policy_sha256
            || body.verified_fit_sha256 != fit.sha256
            || body.artifact_kind != WHOLE_MODEL_KIND
            || body.artifact_kind != fit.body.artifact_kind
            || body.artifact_sha256 != fit.body.artifact_sha256
            || !digest64(&body.row_prefix_sha256)
            || body.parts.is_empty()
            || body.parts.len() > MAX_PARTS_PER_EPISODE
            || body.parts.iter().any(|part| {
                part.sources.len() > schedule.context().sources.len()
                    || part.mapping.segment_ordinal >= MAX_PARTS_PER_EPISODE as u16
            })
            || body.quality_sha256 != hash(&body.parts)?
            || body.confirmed_at_ns < fit.body.freeze_prepare_receiver_ns
            || body.confirmed_at_ns > now
            || body
                .previous_sha256
                .as_ref()
                .is_some_and(|sha| !digest64(sha))
            || body.trial_id.len() > 128
            || !digest64(&self.sha256)
            || self.sha256 != hash(&(HELD_OUT_SEAL_DOMAIN, body))?
        {
            return Err("temporal_heldout_seal_data_binding".into());
        }
        // This hash carrier is used ONLY to check expected identity DATA.
        // Its existing contract explicitly grants no fitted/filesystem custody.
        let frozen = crate::room_learner::managed_schedule_contract::FrozenModelIdentity::from_runtime_verified_artifact(
            schedule,
            body.artifact_sha256.clone(),
        )?;
        let expected = schedule.bind_episode_start(run, body.episode, 0, 0, Some(&frozen))?;
        if body.trial_id != expected.trial_id() {
            return Err("temporal_heldout_seal_trial_changed".into());
        }
        Ok(())
    }
    pub(super) fn validate_actual_prefix_digest(&self, actual: &str) -> Result<(), String> {
        if self.body.row_prefix_sha256 != actual {
            return Err("temporal_heldout_seal_prefix_changed".into());
        }
        Ok(())
    }
}

/// Role-local capture DATA in the existing owner, never a second actor.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct HeldOutBridgeState {
    policy: String,
    pub(super) disposition: Disposition,
    pub(super) seals: Vec<HeldOutSealData>,
    pub(super) parts: Vec<QualityPart>,
}
impl HeldOutBridgeState {
    pub(super) fn new() -> Self {
        Self {
            policy: HELD_OUT_BRIDGE_DOMAIN.into(),
            disposition: Disposition::Ready,
            seals: vec![],
            parts: vec![],
        }
    }
    /// Bounded shape/binding check ONLY. The caller must separately validate
    /// complete reservations, original native identity and whole-period quality.
    pub(super) fn validate_shape_data(
        &self,
        schedule: &ManagedScheduleContract,
        run: &str,
        fit: &VerifiedFitRecordData,
        episode: usize,
        accepted_windows: usize,
        train_parts: usize,
        global_gaps: usize,
        now: u64,
    ) -> Result<(), String> {
        fixed_schedule(schedule)?;
        let sealed_windows = TRAIN_WINDOWS + self.seals.len() * WINDOWS_PER_HELD_OUT_EPISODE;
        if self.policy != HELD_OUT_BRIDGE_DOMAIN
            || self.seals.len() > HELD_OUT_EPISODES
            || episode != TRAIN_EPISODES + self.seals.len()
            || global_gaps > MAX_GLOBAL_GAPS
            || self.parts.len() > MAX_PARTS_PER_EPISODE
            || train_parts
                .checked_add(self.parts.len())
                .and_then(|value| {
                    self.seals
                        .iter()
                        .try_fold(value, |sum, seal| sum.checked_add(seal.body.parts.len()))
                })
                .is_none_or(|parts| parts > MAX_GLOBAL_PARTS)
            || accepted_windows < sealed_windows
            || accepted_windows > FULL_WINDOWS
            || (self.seals.len() < HELD_OUT_EPISODES
                && accepted_windows > sealed_windows + WINDOWS_PER_HELD_OUT_EPISODE)
        {
            return Err("temporal_heldout_bridge_shape".into());
        }
        let mut previous = None;
        let mut last_confirmation = fit.body.freeze_prepare_receiver_ns;
        for (index, seal) in self.seals.iter().enumerate() {
            seal.validate_binding_data(schedule, run, fit, now)?;
            if seal.body.episode != TRAIN_EPISODES + index
                || seal.body.previous_sha256 != previous
                || seal.body.confirmed_at_ns < last_confirmation
            {
                return Err("temporal_heldout_seal_order".into());
            }
            previous = Some(seal.sha256.clone());
            last_confirmation = seal.body.confirmed_at_ns;
        }
        if self.seals.len() == HELD_OUT_EPISODES {
            if self.disposition != Disposition::Complete
                || !self.parts.is_empty()
                || accepted_windows != FULL_WINDOWS
            {
                return Err("temporal_heldout_last_seal_shape".into());
            }
        } else if self.disposition == Disposition::Complete
            || (self.disposition == Disposition::Ready
                && (!self.parts.is_empty() || accepted_windows != sealed_windows))
        {
            return Err("temporal_heldout_early_complete_or_ready".into());
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct FullCompleteBody {
    pub(super) context_sha256: String,
    pub(super) run: String,
    pub(super) descriptor_id: String,
    pub(super) created_at_ms: u64,
    pub(super) expires_at_ms: u64,
    pub(super) continuation_policy_sha256: String,
    pub(super) verified_fit_sha256: String,
    pub(super) artifact_kind: String,
    pub(super) artifact_sha256: String,
    pub(super) held_out_chain_sha256: String,
    pub(super) raw_sha256: String,
    pub(super) evidence_sha256: String,
    pub(super) identity_sha256: String,
    pub(super) ledger_sha256: String,
    pub(super) windows: usize,
    pub(super) held_out_windows: usize,
    pub(super) held_out_seals: usize,
    pub(super) accepted_held_out_commitment: TemporalCommitmentData,
    pub(super) complete_prepare_receiver_ns: u64,
    pub(super) source_floors: Vec<(u8, u64, u64)>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct FullCompleteRecordData {
    schema: String,
    pub(super) body: FullCompleteBody,
    pub(super) sha256: String,
}
impl FullCompleteRecordData {
    /// Computes a DATA digest. Only actual last-SEAL ACK/current-store recovery
    /// may later supply the separate non-serde Complete write/install witness.
    pub(super) fn from_body(body: FullCompleteBody) -> Result<Self, String> {
        let sha256 = hash(&(FULL_COMPLETE_DOMAIN, &body))?;
        Ok(Self {
            schema: FULL_COMPLETE_DOMAIN.into(),
            body,
            sha256,
        })
    }
    pub(super) fn validate_binding_data(
        &self,
        fit: &VerifiedFitRecordData,
        descriptor: &RecoveryDescriptor,
        now: u64,
    ) -> Result<(), String> {
        let body = &self.body;
        let hashes = [
            &body.continuation_policy_sha256,
            &body.verified_fit_sha256,
            &body.artifact_sha256,
            &body.held_out_chain_sha256,
            &body.raw_sha256,
            &body.evidence_sha256,
            &body.identity_sha256,
            &body.ledger_sha256,
        ];
        if self.schema != FULL_COMPLETE_DOMAIN
            || hashes.iter().any(|value| !digest64(value))
            || body.context_sha256 != descriptor.context_digest
            || body.context_sha256 != fit.body.context_digest
            || body.run != fit.body.run
            || body.descriptor_id != descriptor.id
            || body.descriptor_id != fit.body.accepted_train_commitment.id
            || body.created_at_ms != descriptor.created_at_ms
            || body.expires_at_ms != descriptor.expires_at_ms
            || body.continuation_policy_sha256 != fit.body.continuation_policy_sha256
            || body.verified_fit_sha256 != fit.sha256
            || body.artifact_kind != WHOLE_MODEL_KIND
            || body.artifact_kind != fit.body.artifact_kind
            || body.artifact_sha256 != fit.body.artifact_sha256
            || body.windows != FULL_WINDOWS
            || body.held_out_windows != TRAIN_WINDOWS
            || body.held_out_seals != HELD_OUT_EPISODES
            || body.accepted_held_out_commitment.id != descriptor.id
            || body.accepted_held_out_commitment.run != body.run
            || !digest64(&body.accepted_held_out_commitment.commitment)
            || body.accepted_held_out_commitment.revision > 430
            || body.complete_prepare_receiver_ns < fit.body.freeze_prepare_receiver_ns
            || body.complete_prepare_receiver_ns > now
            || now / 1_000_000 >= descriptor.expires_at_ms
            || body.source_floors.len() != fit.body.source_floors.len()
            || !digest64(&self.sha256)
            || self.sha256 != hash(&(FULL_COMPLETE_DOMAIN, body))?
        {
            return Err("temporal_full_complete_data_binding".into());
        }
        let mut seen = std::collections::HashSet::new();
        for (source, sequence, time) in &body.source_floors {
            if !seen.insert(*source)
                || *sequence == 0
                || *sequence > u64::from(u32::MAX)
                || *time > body.complete_prepare_receiver_ns
                || !fit
                    .body
                    .source_floors
                    .iter()
                    .any(|(id, before_sequence, before_time)| {
                        id == source && sequence >= before_sequence && time >= before_time
                    })
            {
                return Err("temporal_full_complete_floor_regressed".into());
            }
        }
        Ok(())
    }
}
