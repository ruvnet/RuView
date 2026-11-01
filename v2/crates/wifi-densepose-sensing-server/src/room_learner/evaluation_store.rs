//! Per-model, per-role immutable evaluation history; no activation authority.
use super::*;
use crate::room_learner::observation::{DataRole, ValidatedDataset};
#[path = "evaluation_receipt.rs"]
mod receipt;
use receipt::data_sha256;
#[path = "evaluation_index.rs"]
mod index;
pub use receipt::EvaluationReceipt;
const SCHEMA: &str = "ruview.room-learner.evaluation-artifact.v1";
const POINTER_SCHEMA_EVAL: &str = "ruview.room-learner.evaluation-pointer.v1";
const LATEST: &str = "latest-v1.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluationEnvelope {
    schema: String,
    receipt_sha256: String,
    receipt: EvaluationReceipt,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluationPointer {
    schema: String,
    receipt_sha256: String,
    model_sha256: String,
    context_digest: String,
    role: DataRole,
}
fn role_directory(role: DataRole) -> Result<&'static str, String> {
    match role {
        DataRole::Validation => Ok("validation"),
        DataRole::HeldOut => Ok("held-out"),
        _ => Err("evaluation only accepts validation or held-out data".into()),
    }
}
fn private_subdirectory(
    parent: &Path,
    name: &str,
    create: bool,
) -> Result<Option<PathBuf>, String> {
    let path = parent.join(name);
    match std::fs::symlink_metadata(&path) {
        Ok(m) if !m.is_dir() || m.file_type().is_symlink() => {
            return Err("invalid private evaluation directory".into())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound && !create => return Ok(None),
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.to_string()),
        _ => {}
    }
    private_dir(&path).map_err(|e| e.to_string())?;
    Ok(Some(path))
}
fn write_immutable(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if regular_or_missing(path)? {
        return if private_read(path).map_err(|e| e.to_string())? == bytes {
            Ok(())
        } else {
            Err("immutable evaluation collision/corruption".into())
        };
    }
    let directory = path.parent().ok_or("missing evaluation parent")?;
    let staging = directory.join(format!("evaluation-stage-{}", crate::companion::secret()));
    atomic_private_write(&staging, bytes).map_err(|e| e.to_string())?;
    let installed = std::fs::hard_link(&staging, path);
    let cleanup = std::fs::remove_file(&staging);
    match installed {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            if private_read(path).map_err(|e| e.to_string())? != bytes {
                return Err("immutable evaluation collision/corruption".into());
            }
        }
        Err(e) => return Err(e.to_string()),
    }
    cleanup.map_err(|e| e.to_string())?;
    std::fs::File::open(directory)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}
impl ArtifactStore {
    fn evaluation_directory(
        &self,
        model: &RoomModel,
        role: DataRole,
        create: bool,
    ) -> Result<Option<PathBuf>, String> {
        let role_name = role_directory(role)?;
        self.validate_context(model.context())?;
        let Some(context) = self.context_dir(model.context(), false)? else {
            return Err("persist the frozen candidate before evaluation".into());
        };
        let bytes = model.export()?;
        let model_receipt = self.receipt_for(model, &bytes);
        self.read_model(&context, &model_receipt, model.context())?;
        let Some(evaluations) = private_subdirectory(&context, "evaluations", create)? else {
            return Ok(None);
        };
        let Some(model_dir) =
            private_subdirectory(&evaluations, &model_receipt.model_sha256, create)?
        else {
            return Ok(None);
        };
        private_subdirectory(&model_dir, role_name, create)
    }
    fn read_evaluation(
        &self,
        directory: &Path,
        key: &str,
        model: &RoomModel,
        role: DataRole,
    ) -> Result<EvaluationReceipt, String> {
        if !hex_digest(key) {
            return Err("invalid evaluation digest".into());
        }
        let bytes =
            private_read(&directory.join(format!("{key}.json"))).map_err(|e| e.to_string())?;
        let envelope: EvaluationEnvelope =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if envelope.schema != SCHEMA
            || envelope.receipt_sha256 != key
            || envelope.receipt.sha256()? != key
        {
            return Err("evaluation integrity mismatch".into());
        }
        envelope.receipt.validate(model, role)?;
        Ok(envelope.receipt)
    }
    fn read_latest_evaluation(
        &self,
        directory: &Path,
        model: &RoomModel,
        role: DataRole,
    ) -> Result<Option<EvaluationReceipt>, String> {
        let path = directory.join(LATEST);
        if !regular_or_missing(&path)? {
            return Ok(None);
        }
        let pointer: EvaluationPointer =
            serde_json::from_slice(&private_read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if pointer.schema != POINTER_SCHEMA_EVAL
            || pointer.context_digest != model.context().digest()
            || pointer.role != role
            || pointer.model_sha256 != digest(&model.export()?)
            || !hex_digest(&pointer.receipt_sha256)
        {
            return Err("evaluation pointer/candidate mismatch".into());
        }
        let receipt = self.read_evaluation(directory, &pointer.receipt_sha256, model, role)?;
        let indexed = self
            .read_evidence_index(directory, &receipt.dataset_sha256, model, role)?
            .ok_or("evaluation evidence index missing")?;
        if indexed.sha256()? != pointer.receipt_sha256 {
            return Err("evaluation latest pointer differs from original evidence".into());
        }
        Ok(Some(receipt))
    }
    /// Compute metrics from a validated role split, then persist. No supplied
    /// report, physical-qualification claim or caller-controlled filename enters.
    pub fn evaluate_candidate(
        &self,
        model: &RoomModel,
        dataset: &ValidatedDataset,
        role: DataRole,
    ) -> Result<EvaluationReceipt, String> {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "evaluation clock unavailable")?
            .as_millis()
            .try_into()
            .map_err(|_| "evaluation clock outside bounds")?;
        self.evaluate_candidate_at(model, dataset, role, at)
    }
    fn evaluate_candidate_at(
        &self,
        model: &RoomModel,
        dataset: &ValidatedDataset,
        role: DataRole,
        at: u64,
    ) -> Result<EvaluationReceipt, String> {
        role_directory(role)?;
        if dataset.context() != model.context() {
            return Err("evaluation dataset/candidate context mismatch".into());
        }
        let train_checked = !dataset.train().is_empty();
        if train_checked
            && data_sha256(dataset.train())? != model.training_manifest().training_data_sha256
        {
            return Err("retained Train set differs from the frozen candidate".into());
        }
        let rows = if role == DataRole::HeldOut {
            dataset.held_out()
        } else {
            dataset.validation()
        };
        if rows.is_empty() {
            return Err("no cohorts for the selected evaluation role".into());
        }
        let directory = self
            .evaluation_directory(model, role, true)?
            .ok_or("missing evaluation directory")?;
        let previous = self.read_latest_evaluation(&directory, model, role)?;
        let data_sha = data_sha256(rows)?;
        if let Some(original) = self.read_evidence_index(&directory, &data_sha, model, role)? {
            // Selecting old evidence can move latest, but never changes its
            // immutable receipt identity, original timestamp or lineage proof.
            if previous
                .as_ref()
                .map(EvaluationReceipt::sha256)
                .transpose()?
                .as_deref()
                != Some(original.sha256()?.as_str())
            {
                self.publish_evaluation_pointer(&directory, &original)?;
            }
            return Ok(original);
        }
        let receipt = EvaluationReceipt::new(model, rows, role, at, train_checked)?;
        let key = receipt.sha256()?;
        let envelope = EvaluationEnvelope {
            schema: SCHEMA.into(),
            receipt_sha256: key.clone(),
            receipt: receipt.clone(),
        };
        let bytes = encode(&envelope)?;
        write_immutable(&directory.join(format!("{key}.json")), &bytes)?;
        self.read_evaluation(&directory, &key, model, role)?;
        self.save_evidence_index(&directory, &receipt, model, role)?;
        self.publish_evaluation_pointer(&directory, &receipt)?;
        Ok(receipt)
    }
    fn publish_evaluation_pointer(
        &self,
        directory: &Path,
        receipt: &EvaluationReceipt,
    ) -> Result<(), String> {
        let pointer = EvaluationPointer {
            schema: POINTER_SCHEMA_EVAL.into(),
            receipt_sha256: receipt.sha256()?,
            model_sha256: receipt.model_sha256.clone(),
            context_digest: receipt.context_digest.clone(),
            role: receipt.role,
        };
        let bytes = encode(&pointer)?;
        atomic_private_write(&directory.join(LATEST), &bytes).map_err(|e| e.to_string())?;
        if private_read(&directory.join(LATEST)).map_err(|e| e.to_string())? != bytes {
            return Err("evaluation pointer readback failed".into());
        }
        Ok(())
    }
    pub fn load_latest_evaluation(
        &self,
        model: &RoomModel,
        role: DataRole,
    ) -> Result<Option<EvaluationReceipt>, String> {
        let Some(directory) = self.evaluation_directory(model, role, false)? else {
            return Ok(None);
        };
        self.read_latest_evaluation(&directory, model, role)
    }
}

#[cfg(test)]
#[path = "evaluation_store_tests.rs"]
mod tests;
