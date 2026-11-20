//! Immutable managed provenance is committed before the candidate pointer.
use super::*;
use crate::room_learner::managed_training::{ManagedTrainingProof, TrainingProofSeed};
impl ArtifactStore {
    pub(crate) fn load_managed_training_proof(
        &self,
        model: &RoomModel,
    ) -> Result<Option<ManagedTrainingProof>, String> {
        self.validate_context(model.context())?;
        let Some(dir) = self.context_dir(model.context(), false)? else {
            return Ok(None);
        };
        let sha = digest(&model.export()?);
        let path = dir.join(format!("managed-training-{sha}.json"));
        if !regular_or_missing(&path)? {
            return Ok(None);
        }
        let proof: ManagedTrainingProof =
            serde_json::from_slice(&private_read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        proof.validate(model)?;
        Ok(Some(proof))
    }
    pub(crate) fn save_managed_candidate(
        &self,
        model: &RoomModel,
        seed: TrainingProofSeed,
    ) -> Result<CandidateReceipt, String> {
        self.validate_context(model.context())?;
        seed.validate()?;
        let proposed = ManagedTrainingProof::issue(model, seed)?;
        let proof = if let Some(original) = self.load_managed_training_proof(model)? {
            // Retries use the first committed proof, never re-attest its original
            // source watermark with observations received during the fit/retry.
            if original.seed.context != proposed.seed.context
                || original.seed.run_id != proposed.seed.run_id
                || original.seed.training_data_sha256 != proposed.seed.training_data_sha256
                || original.seed.training_metadata_sha256 != proposed.seed.training_metadata_sha256
                || original.seed.sealed_train_history_sha256
                    != proposed.seed.sealed_train_history_sha256
                || original.seed.train_counts != proposed.seed.train_counts
                || original.seed.phase_claims != proposed.seed.phase_claims
                || original
                    .seed
                    .sources
                    .iter()
                    .zip(&proposed.seed.sources)
                    .any(|(a, b)| {
                        a.source_id != b.source_id
                            || a.ordinal > b.ordinal
                            || a.last_time_ns > b.last_time_ns
                    })
            {
                return Err(
                    "existing managed training proof differs from the original fit lineage".into(),
                );
            }
            proposed.seed.ensure_extends(&original.seed)?;
            original
        } else {
            proposed
        };
        let dir = self
            .context_dir(model.context(), true)?
            .ok_or("missing managed model context")?;
        let path = dir.join(format!("managed-training-{}.json", proof.model_sha256));
        let bytes = encode(&proof)?;
        evaluation_store::write_immutable(&path, &bytes)?;
        if private_read(&path).map_err(|e| e.to_string())? != bytes {
            return Err("managed training proof readback mismatch".into());
        }
        self.load_managed_training_proof(model)?
            .ok_or("managed training proof was not retained")?;
        self.initialize_managed_capture_fence(model, &proof)?;
        // Generic save_candidate remains diagnostic-only for all import callers.
        self.save_candidate(model)
    }
}
