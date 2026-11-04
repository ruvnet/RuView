//! Bounded immutable metadata for sealed post-fit phases. No RF values.
use super::{
    managed_training::{ManagedTrainingProof, TrainingProofSeed},
    qualification::hash,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManagedCaptureFence {
    pub schema: String,
    pub revision: u64,
    pub previous_sha256: Option<String>,
    pub model_sha256: String,
    pub managed_training_proof_sha256: String,
    pub seed: TrainingProofSeed,
    pub fence_sha256: String,
}
impl TrainingProofSeed {
    pub(crate) fn ensure_extends(&self, original: &Self) -> Result<(), String> {
        self.validate()?;
        original.validate()?;
        if self.context != original.context
            || self.run_id != original.run_id
            || self.training_data_sha256 != original.training_data_sha256
            || self.training_metadata_sha256 != original.training_metadata_sha256
            || self.sealed_train_history_sha256 != original.sealed_train_history_sha256
            || self.train_counts != original.train_counts
            || self.first_observed_ns != original.first_observed_ns
            || self.receiver_clock_domain != original.receiver_clock_domain
            || self.phase_claims.len() < original.phase_claims.len()
            || self.phase_claims[..original.phase_claims.len()] != original.phase_claims
            || self.last_seen_ns < original.last_seen_ns
        {
            return Err(
                "managed sealed capture lineage regressed or changed original training".into(),
            );
        }
        for (old, new) in original.sources.iter().zip(&self.sources) {
            if new.source_id != old.source_id
                || new.train_range != old.train_range
                || new.ordinal < old.ordinal
                || new.last_time_ns < old.last_time_ns
                || new.admitted_packets < old.admitted_packets
                || new.sequence.wrapping_sub(old.sequence) != (new.ordinal - old.ordinal) as u32
            {
                return Err("managed sealed capture source counter/clock regressed".into());
            }
        }
        Ok(())
    }
}
impl ManagedCaptureFence {
    pub(crate) fn issue(
        proof: &ManagedTrainingProof,
        previous: Option<&Self>,
        seed: TrainingProofSeed,
    ) -> Result<Self, String> {
        seed.ensure_extends(&proof.seed)?;
        if let Some(old) = previous {
            old.validate(proof)?;
            seed.ensure_extends(&old.seed)?;
            let added = &seed.phase_claims[old.seed.phase_claims.len()..];
            if added
                .iter()
                .filter(|p| p.status == super::capture::PhaseStatus::Sealed)
                .count()
                != 1
                || added
                    .iter()
                    .any(|p| p.role == super::observation::DataRole::Train)
            {
                return Err(
                    "managed ledger requires exactly one newly sealed post-fit phase".into(),
                );
            }
        }
        let mut value = Self {
            schema: "ruview.managed-sealed-phase-fence.v1".into(),
            revision: previous.map_or(0, |p| p.revision + 1),
            previous_sha256: previous.map(|p| p.fence_sha256.clone()),
            model_sha256: proof.model_sha256.clone(),
            managed_training_proof_sha256: proof.proof_sha256.clone(),
            seed,
            fence_sha256: String::new(),
        };
        value.fence_sha256 = value.digest()?;
        value.validate(proof)?;
        Ok(value)
    }
    fn digest(&self) -> Result<String, String> {
        hash(&(
            &self.schema,
            self.revision,
            &self.previous_sha256,
            &self.model_sha256,
            &self.managed_training_proof_sha256,
            &self.seed,
        ))
    }
    pub(crate) fn validate(&self, proof: &ManagedTrainingProof) -> Result<(), String> {
        self.seed.ensure_extends(&proof.seed)?;
        if self.schema != "ruview.managed-sealed-phase-fence.v1"
            || self.revision >= 128
            || self.model_sha256 != proof.model_sha256
            || self.managed_training_proof_sha256 != proof.proof_sha256
            || self.fence_sha256 != self.digest()?
            || (self.revision == 0) != self.previous_sha256.is_none()
            || (self.revision == 0 && self.seed != proof.seed)
        {
            return Err("managed sealed phase fence identity/integrity mismatch".into());
        }
        Ok(())
    }
}
