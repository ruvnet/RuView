//! Private immutable phase-fence chain and atomically published verified head.
use super::*;
use crate::room_learner::{
    managed_capture_fences::ManagedCaptureFence,
    managed_training::{ManagedTrainingProof, TrainingProofSeed},
};
const HEAD: &str = "managed-sealed-phase-head-v1.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FenceHead {
    schema: String,
    model_sha256: String,
    proof_sha256: String,
    fence_sha256: String,
    revision: u64,
}
impl ArtifactStore {
    fn fence_dir(&self, model: &RoomModel, create: bool) -> Result<Option<PathBuf>, String> {
        self.validate_context(model.context())?;
        let Some(dir) = self.context_dir(model.context(), create)? else {
            return Ok(None);
        };
        evaluation_store::private_subdirectory(
            &dir,
            &format!("managed-fences-{}", digest(&model.export()?)),
            create,
        )
    }
    fn read_fence_chain(
        &self,
        model: &RoomModel,
        proof: &ManagedTrainingProof,
        allow_pending: bool,
    ) -> Result<Vec<ManagedCaptureFence>, String> {
        proof.validate(model)?;
        let dir = self
            .fence_dir(model, false)?
            .ok_or("managed sealed phase ledger missing")?;
        let path = dir.join(HEAD);
        if !regular_or_missing(&path)? {
            return Err("managed sealed phase ledger head missing".into());
        }
        let head: FenceHead =
            serde_json::from_slice(&private_read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if head.schema != "ruview.managed-sealed-phase-head.v1"
            || head.model_sha256 != proof.model_sha256
            || head.proof_sha256 != proof.proof_sha256
            || head.revision >= 128
            || !hex_digest(&head.fence_sha256)
        {
            return Err("managed sealed phase ledger head mismatch".into());
        }
        let mut result = Vec::new();
        let mut sha = head.fence_sha256;
        let mut revision = head.revision;
        loop {
            if !hex_digest(&sha) {
                return Err("invalid managed sealed phase ledger hash".into());
            }
            let value: ManagedCaptureFence = serde_json::from_slice(
                &private_read(&dir.join(format!("{sha}.json"))).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            value.validate(proof)?;
            if value.fence_sha256 != sha || value.revision != revision {
                return Err("managed sealed phase ledger chain changed".into());
            }
            let previous = value.previous_sha256.clone();
            result.push(value);
            if revision == 0 {
                break;
            }
            sha = previous.ok_or("managed sealed phase ledger link missing")?;
            revision -= 1;
        }
        result.reverse();
        for pair in result.windows(2) {
            pair[1].seed.ensure_extends(&pair[0].seed)?;
            let expected = ManagedCaptureFence::issue(proof, Some(&pair[0]), pair[1].seed.clone())?;
            if expected != pair[1] {
                return Err("managed phase ledger did not advance exact sealed history".into());
            }
        }
        let mut entries = 0;
        for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            entries += 1;
            if entries > 130 {
                return Err("managed sealed phase ledger exceeds capacity".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "invalid managed ledger path")?;
            if name == HEAD {
                continue;
            }
            let sha = name
                .strip_suffix(".json")
                .filter(|s| hex_digest(s))
                .ok_or("unknown pending managed phase artifact")?;
            if !regular_or_missing(&entry.path())? {
                return Err("managed ledger record disappeared".into());
            }
            if !allow_pending && !result.iter().any(|f| f.fence_sha256 == sha) {
                return Err("managed sealed phase publication is pending or conflicting".into());
            }
        }
        Ok(result)
    }
    pub(crate) fn load_managed_capture_fence(
        &self,
        model: &RoomModel,
        proof: &ManagedTrainingProof,
    ) -> Result<ManagedCaptureFence, String> {
        self.read_fence_chain(model, proof, false)?
            .pop()
            .ok_or("managed capture fence unavailable".into())
    }
    /// Resolve historical comparison provenance against the already verified
    /// bounded append-only chain. This never moves or repairs its head.
    pub(crate) fn load_managed_capture_fence_by_sha(
        &self,
        model: &RoomModel,
        proof: &ManagedTrainingProof,
        sha: &str,
    ) -> Result<ManagedCaptureFence, String> {
        if !hex_digest(sha) {
            return Err("invalid managed fence digest".into());
        }
        self.read_fence_chain(model, proof, false)?
            .into_iter()
            .find(|fence| fence.fence_sha256 == sha)
            .ok_or("referenced managed capture fence unavailable".into())
    }
    fn commit_fence(
        &self,
        model: &RoomModel,
        proof: &ManagedTrainingProof,
        fence: &ManagedCaptureFence,
    ) -> Result<(), String> {
        let dir = self
            .fence_dir(model, true)?
            .ok_or("managed fence directory unavailable")?;
        evaluation_store::write_immutable(
            &dir.join(format!("{}.json", fence.fence_sha256)),
            &encode(fence)?,
        )?;
        let head = FenceHead {
            schema: "ruview.managed-sealed-phase-head.v1".into(),
            model_sha256: proof.model_sha256.clone(),
            proof_sha256: proof.proof_sha256.clone(),
            fence_sha256: fence.fence_sha256.clone(),
            revision: fence.revision,
        };
        let bytes = encode(&head)?;
        atomic_private_write(&dir.join(HEAD), &bytes).map_err(|e| e.to_string())?;
        if private_read(&dir.join(HEAD)).map_err(|e| e.to_string())? != bytes {
            return Err("managed capture fence head readback mismatch".into());
        }
        if self.load_managed_capture_fence(model, proof)? != *fence {
            return Err("managed capture fence chain readback mismatch".into());
        }
        Ok(())
    }
    pub(crate) fn initialize_managed_capture_fence(
        &self,
        model: &RoomModel,
        proof: &ManagedTrainingProof,
    ) -> Result<(), String> {
        if let Some(dir) = self.fence_dir(model, false)? {
            if regular_or_missing(&dir.join(HEAD))? {
                self.load_managed_capture_fence(model, proof)?;
                return Ok(());
            }
            // Retry only the exact initial inode before any model was installed.
            // An existing candidate or another record means historical authority
            // may have existed: never replace its missing head with an empty one.
            let context = self
                .context_dir(model.context(), false)?
                .ok_or("managed context missing")?;
            if regular_or_missing(&context.join(format!("{}.json", proof.model_sha256)))? {
                return Err("existing candidate has an incomplete managed phase ledger".into());
            }
            let initial = ManagedCaptureFence::issue(proof, None, proof.seed.clone())?;
            let expected = format!("{}.json", initial.fence_sha256);
            for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.file_name().to_str() != Some(expected.as_str())
                    || private_read(&entry.path()).map_err(|e| e.to_string())? != encode(&initial)?
                {
                    return Err("managed initial phase publication conflicts".into());
                }
            }
        }
        let initial = ManagedCaptureFence::issue(proof, None, proof.seed.clone())?;
        self.commit_fence(model, proof, &initial)
    }
    pub(crate) fn save_managed_capture_fence(
        &self,
        model: &RoomModel,
        proof: &ManagedTrainingProof,
        seed: TrainingProofSeed,
    ) -> Result<ManagedCaptureFence, String> {
        let chain = self.read_fence_chain(model, proof, true)?;
        let previous = chain
            .last()
            .ok_or("managed capture fence unavailable")?
            .clone();
        seed.ensure_extends(&previous.seed)?;
        if seed.phase_claims == previous.seed.phase_claims {
            // The same sealed phase may have committed before fsync reported
            // failure. Retry its durability barrier, not its capture metadata.
            self.load_managed_capture_fence(model, proof)?;
            let dir = self
                .fence_dir(model, false)?
                .ok_or("managed fence directory unavailable")?;
            std::fs::File::open(dir)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
            return Ok(previous);
        }
        let dir = self
            .fence_dir(model, false)?
            .ok_or("managed fence directory unavailable")?;
        let mut pending = None;
        for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "invalid managed ledger path")?;
            if name == HEAD {
                continue;
            }
            let sha = name
                .strip_suffix(".json")
                .ok_or("invalid managed phase record")?;
            if chain.iter().any(|f| f.fence_sha256 == sha) {
                continue;
            }
            let record: ManagedCaptureFence =
                serde_json::from_slice(&private_read(&entry.path()).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            record.validate(proof)?;
            if record.fence_sha256 != sha
                || pending.is_some()
                || record.previous_sha256.as_deref() != Some(previous.fence_sha256.as_str())
                || record.revision != previous.revision + 1
                || record.seed.phase_claims != seed.phase_claims
            {
                return Err(
                    "pending managed phase publication conflicts with this exact phase".into(),
                );
            }
            seed.ensure_extends(&record.seed)?;
            pending = Some(record);
        }
        let next = match pending {
            Some(original) => original,
            None => ManagedCaptureFence::issue(proof, Some(&previous), seed)?,
        };
        self.commit_fence(model, proof, &next)?;
        Ok(next)
    }
    pub(crate) fn require_managed_capture_fence(
        &self,
        model: &RoomModel,
        proof: &ManagedTrainingProof,
        sha: &str,
    ) -> Result<(), String> {
        if self
            .read_fence_chain(model, proof, false)?
            .iter()
            .any(|f| f.fence_sha256 == sha)
        {
            Ok(())
        } else {
            Err("qualification sealed phase fence is not retained".into())
        }
    }
}
