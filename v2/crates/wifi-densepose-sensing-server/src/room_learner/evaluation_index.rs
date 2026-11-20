//! Direct immutable lookup by exact model/role/dataset evidence identity.
use super::*;
const DIRECTORY: &str = "by-dataset";
const SCHEMA_INDEX: &str = "ruview.room-learner.evaluation-evidence-index.v1";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceIndex {
    schema: String,
    owner_scope: String,
    installation_id: String,
    room_digest: String,
    context_digest: String,
    model_sha256: String,
    model_version: u32,
    role: DataRole,
    dataset_sha256: String,
    receipt_sha256: String,
}
impl ArtifactStore {
    pub(super) fn read_evidence_index(
        &self,
        directory: &Path,
        data_sha: &str,
        model: &RoomModel,
        role: DataRole,
    ) -> Result<Option<EvaluationReceipt>, String> {
        if !hex_digest(data_sha) {
            return Err("invalid evaluation dataset digest".into());
        }
        let Some(index_dir) = private_subdirectory(directory, DIRECTORY, false)? else {
            return Ok(None);
        };
        let path = index_dir.join(format!("{data_sha}.json"));
        if !regular_or_missing(&path)? {
            return Ok(None);
        }
        let index: EvidenceIndex =
            serde_json::from_slice(&private_read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let context = model.context();
        if index.schema != SCHEMA_INDEX
            || index.owner_scope != context.owner_scope
            || index.installation_id != context.installation_id
            || index.room_digest != context.room_digest
            || index.context_digest != context.digest()
            || index.model_sha256 != digest(&model.export()?)
            || index.model_version != model.version
            || index.role != role
            || index.dataset_sha256 != data_sha
            || !hex_digest(&index.receipt_sha256)
        {
            return Err("evaluation evidence index binding mismatch".into());
        }
        let receipt = self.read_evaluation(directory, &index.receipt_sha256, model, role)?;
        if receipt.dataset_sha256 != data_sha {
            return Err("evaluation index references another dataset".into());
        }
        Ok(Some(receipt))
    }
    pub(super) fn save_evidence_index(
        &self,
        directory: &Path,
        receipt: &EvaluationReceipt,
        model: &RoomModel,
        role: DataRole,
    ) -> Result<(), String> {
        receipt.validate(model, role)?;
        let index_dir = private_subdirectory(directory, DIRECTORY, true)?
            .ok_or("missing evaluation evidence index")?;
        let index = EvidenceIndex {
            schema: SCHEMA_INDEX.into(),
            owner_scope: receipt.owner_scope.clone(),
            installation_id: receipt.installation_id.clone(),
            room_digest: receipt.room_digest.clone(),
            context_digest: receipt.context_digest.clone(),
            model_sha256: receipt.model_sha256.clone(),
            model_version: receipt.model_version,
            role,
            dataset_sha256: receipt.dataset_sha256.clone(),
            receipt_sha256: receipt.sha256()?,
        };
        write_immutable(
            &index_dir.join(format!("{}.json", receipt.dataset_sha256)),
            &encode(&index)?,
        )?;
        let restored = self
            .read_evidence_index(directory, &receipt.dataset_sha256, model, role)?
            .ok_or("evaluation evidence index readback missing")?;
        if restored.sha256()? != index.receipt_sha256 {
            return Err("evaluation evidence index readback mismatch".into());
        }
        Ok(())
    }
}
