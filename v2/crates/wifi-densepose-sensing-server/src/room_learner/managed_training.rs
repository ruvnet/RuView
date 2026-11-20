//! Metadata-only provenance issued by the managed native capture path, not model imports.
use super::{
    capture::{PhaseStatus, COHORTS_PER_PHASE},
    model::RoomModel,
    observation::{CaptureContext, CaptureProvenance, DataRole, RoomLabel, ValidatedDataset},
    qualification::{hash, CONDITIONS},
};
use serde::{Deserialize, Serialize};

pub(crate) const PROOF_SCHEMA: &str = "ruview.managed-training-proof.v1";
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrainingSourceRange {
    pub first_ordinal: u64,
    pub last_ordinal: u64,
    pub first_time_ns: u64,
    pub last_time_ns: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceWatermark {
    pub source_id: u8,
    pub sequence: u32,
    pub ordinal: u64,
    pub last_time_ns: u64,
    pub admitted_packets: u64,
    pub train_range: TrainingSourceRange,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct PhaseClaim {
    pub trial_sha256: String,
    pub session_sha256: String,
    pub role: DataRole,
    pub label: RoomLabel,
    pub status: PhaseStatus,
    pub cohort_count: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrainingProofSeed {
    pub context: CaptureContext,
    pub run_id: String,
    pub training_data_sha256: String,
    pub training_metadata_sha256: String,
    pub sealed_train_history_sha256: String,
    pub train_counts: [usize; 4],
    pub phase_claims: Vec<PhaseClaim>,
    pub sources: Vec<SourceWatermark>,
    pub first_observed_ns: u64,
    pub last_seen_ns: u64,
    /// Receiver observations never establish an authenticated transmitter epoch.
    pub source_epoch_verified: bool,
    pub receiver_clock_domain: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManagedTrainingProof {
    pub schema: String,
    pub model_sha256: String,
    pub seed: TrainingProofSeed,
    pub proof_sha256: String,
}
pub(crate) fn commitment(domain: &str, value: &str) -> Result<String, String> {
    hash(&("ruview.managed-training-role.v1", domain, value))
}
fn hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl TrainingProofSeed {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        let _ = super::capture::CaptureSession::new(self.context.clone(), self.run_id.clone())?;
        if !hex(&self.training_data_sha256)
            || !hex(&self.training_metadata_sha256)
            || !hex(&self.sealed_train_history_sha256)
            || self.source_epoch_verified
            || self.receiver_clock_domain != "utc_anchored_monotonic_v1"
            || self
                .train_counts
                .iter()
                .any(|n| *n < COHORTS_PER_PHASE || *n > 64)
            || self.train_counts.iter().sum::<usize>() > 64
            || self.phase_claims.is_empty()
            || self.phase_claims.len() > 128
            || self
                .phase_claims
                .iter()
                .any(|p| p.cohort_count > COHORTS_PER_PHASE)
            || self
                .phase_claims
                .iter()
                .map(|p| p.cohort_count)
                .sum::<usize>()
                > super::observation::MAX_COHORTS
            || self.sources.len() != self.context.sources.len()
            || self.first_observed_ns == 0
            || self.last_seen_ns < self.first_observed_ns
        {
            return Err("invalid managed training metadata bounds".into());
        }
        let mut trials = std::collections::HashMap::new();
        let mut sessions = std::collections::HashMap::new();
        for p in &self.phase_claims {
            if !hex(&p.trial_sha256)
                || !hex(&p.session_sha256)
                || p.role == DataRole::Inference
                || p.label == RoomLabel::Unknown
                || p.cohort_count
                    != if p.status == PhaseStatus::Sealed {
                        COHORTS_PER_PHASE
                    } else {
                        0
                    }
                || trials
                    .insert(&p.trial_sha256, p.role)
                    .is_some_and(|r| r != p.role)
                || sessions
                    .insert(&p.session_sha256, p.role)
                    .is_some_and(|r| r != p.role)
            {
                return Err("managed training role/phase commitment mismatch".into());
            }
        }
        for (i, label) in CONDITIONS.iter().enumerate() {
            let count: usize = self
                .phase_claims
                .iter()
                .filter(|p| {
                    p.role == DataRole::Train
                        && p.label == *label
                        && p.status == PhaseStatus::Sealed
                })
                .map(|p| p.cohort_count)
                .sum();
            if count != self.train_counts[i] {
                return Err("managed training condition counts lack sealed acknowledgement".into());
            }
        }
        for (s, w) in self.context.sources.iter().zip(&self.sources) {
            if s.source_id != w.source_id
                || w.ordinal == 0
                || w.ordinal >= 1u64 << 56
                || w.admitted_packets == 0
                || w.admitted_packets > w.ordinal
                || w.last_time_ns == 0
                || w.last_time_ns > self.last_seen_ns
                || w.train_range.first_ordinal == 0
                || w.train_range.last_ordinal < w.train_range.first_ordinal
                || w.train_range.last_ordinal > w.ordinal
                || w.train_range.first_time_ns == 0
                || w.train_range.last_time_ns <= w.train_range.first_time_ns
                || w.train_range.last_time_ns > w.last_time_ns
            {
                return Err("invalid managed source watermark".into());
            }
        }
        if self.sources.iter().map(|s| s.last_time_ns).max() != Some(self.last_seen_ns)
            || self
                .sources
                .iter()
                .map(|s| s.train_range.first_time_ns)
                .min()
                != Some(self.first_observed_ns)
        {
            return Err("managed receiver watermark mismatch".into());
        }
        Ok(())
    }
    pub fn check_role(&self, trial: &str, session: &str, role: DataRole) -> Result<(), String> {
        let trial = commitment("trial", trial)?;
        let session = commitment("session", session)?;
        if self.phase_claims.iter().any(|p| {
            (p.trial_sha256 == trial || p.session_sha256 == session)
                && (p.role != role || p.status == PhaseStatus::Sealed)
        }) {
            return Err("phase identity crosses original managed training roles".into());
        }
        Ok(())
    }
    pub fn prefix(&self) -> Result<u64, String> {
        use sha2::{Digest, Sha256};
        let d = Sha256::digest(format!("{}:{}", self.context.digest(), self.run_id).as_bytes());
        Ok(u64::from_be_bytes(
            d[..8].try_into().map_err(|_| "managed frame prefix")?,
        ))
    }
}
impl ManagedTrainingProof {
    pub fn issue(model: &RoomModel, seed: TrainingProofSeed) -> Result<Self, String> {
        let mut p = Self {
            schema: PROOF_SCHEMA.into(),
            model_sha256: String::new(),
            seed,
            proof_sha256: String::new(),
        };
        // Model identity hashes its exact exported bytes, never a JSON Value roundtrip.
        use sha2::{Digest, Sha256};
        p.model_sha256 = format!("{:x}", Sha256::digest(model.export()?));
        p.proof_sha256 = hash(&(&p.schema, &p.model_sha256, &p.seed))?;
        p.validate(model)?;
        Ok(p)
    }
    pub fn validate(&self, model: &RoomModel) -> Result<(), String> {
        use sha2::{Digest, Sha256};
        self.seed.validate()?;
        if self.schema != PROOF_SCHEMA
            || self.proof_sha256 != hash(&(&self.schema, &self.model_sha256, &self.seed))?
            || self.model_sha256 != format!("{:x}", Sha256::digest(model.export()?))
            || &self.seed.context != model.context()
            || self.seed.training_data_sha256 != model.training_manifest().training_data_sha256
            || self.seed.train_counts.iter().sum::<usize>()
                != model.training_manifest().training_cohorts
            || model.training_manifest().training_provenance
                != [CaptureProvenance::LiveReceiverObserved]
        {
            return Err("managed training proof model/context/Train identity mismatch".into());
        }
        Ok(())
    }
    pub fn check_held_out(
        &self,
        model: &RoomModel,
        dataset: &ValidatedDataset,
    ) -> Result<(), String> {
        self.validate(model)?;
        if dataset.context() != model.context() {
            return Err("managed proof evaluation context mismatch".into());
        }
        let prefix = self.seed.prefix()?;
        for c in dataset.held_out() {
            self.seed.check_role(&c.trial_id, &c.session_id, c.role)?;
            if c.provenance != CaptureProvenance::LiveReceiverObserved || c.teacher.is_some() {
                return Err("managed proof requires fresh native held-out capture".into());
            }
            for v in &c.views {
                let w = self
                    .seed
                    .sources
                    .iter()
                    .find(|w| w.source_id == v.source_id)
                    .ok_or("unknown managed held-out source")?;
                if v.provenance != CaptureProvenance::LiveReceiverObserved
                    || v.first_time_ns <= w.last_time_ns
                    || v.sequence_ids
                        .iter()
                        .any(|n| *n <= w.ordinal || *n >= 1u64 << 56)
                    || v.frame_ids.iter().zip(&v.sequence_ids).any(|(f, n)| {
                        *f != ((u128::from(prefix) << 64)
                            | (u128::from(v.source_id) << 56)
                            | u128::from(*n))
                    })
                {
                    return Err(
                        "held-out capture overlaps or replays original managed source lineage"
                            .into(),
                    );
                }
            }
        }
        Ok(())
    }
}
