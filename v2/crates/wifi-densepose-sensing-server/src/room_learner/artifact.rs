//! Private, owner-bound candidate persistence. This store grants no qualification.
use super::{model::RoomModel, observation::CaptureContext};
use crate::companion::{atomic_private_write, private_dir, private_read};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io,
    path::{Path, PathBuf},
};

const POINTER_SCHEMA: &str = "ruview.room-learner.candidate.v1";
const MODEL_SCHEMA: &str = "ruview.room-learner.model.v1";
const LIMIT: usize = 65_536; // Same bound as the actual private_read primitive.
const POINTER: &str = "candidate-v1.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CandidateReceipt {
    pub schema: String,
    pub owner_scope: String,
    pub installation_id: String,
    pub room_digest: String,
    pub context_digest: String,
    pub model_sha256: String,
    pub model_version: u32,
    pub parameter_count: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: String,
    receipt: CandidateReceipt,
    /// Exact RoomModel::export bytes, escaped as JSON without losing float bits.
    learned_model_json: String,
}
#[derive(Debug)]
pub struct ArtifactStore {
    data_dir: PathBuf,
    root: PathBuf,
    owner_scope: String,
    installation_id: String,
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn bounded_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.is_ascii()
        && value.trim() == value
        && !value.bytes().any(|c| c.is_ascii_control())
}
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > LIMIT {
        return Err("candidate artifact exceeds private storage bound".into());
    }
    Ok(bytes)
}
fn regular_or_missing(path: &Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => Ok(true),
        Ok(_) => Err("candidate path is not a private regular file".into()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}
impl ArtifactStore {
    /// Load an immutable historical model by exact hash; never use today's pointer.
    pub fn load_exact_candidate(
        &self,
        context: &CaptureContext,
        sha: &str,
    ) -> Result<RoomModel, String> {
        self.validate_context(context)?;
        if !hex_digest(sha) {
            return Err("invalid exact candidate digest".into());
        }
        let dir = self
            .context_dir(context, false)?
            .ok_or("exact candidate missing")?;
        let envelope: Envelope = serde_json::from_slice(
            &private_read(&dir.join(format!("{sha}.json"))).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if envelope.receipt.model_sha256 != sha {
            return Err("exact candidate hash mismatch".into());
        }
        self.read_model(&dir, &envelope.receipt, context)
    }
    pub fn new(
        data_dir: &Path,
        expected_owner_scope: &str,
        expected_installation: &str,
    ) -> Result<Self, String> {
        if !bounded_id(expected_installation) {
            return Err("invalid expected installation".into());
        }
        let data_dir = crate::account_profile::prepare_directory(expected_owner_scope, data_dir)
            .map_err(|e| e.to_string())?;
        let root = data_dir.join("room-learner");
        private_dir(&root).map_err(|e| e.to_string())?;
        Ok(Self {
            data_dir,
            root,
            owner_scope: expected_owner_scope.into(),
            installation_id: expected_installation.into(),
        })
    }
    fn verify_owner(&self) -> Result<(), String> {
        let actual = crate::account_profile::prepare_directory(&self.owner_scope, &self.data_dir)
            .map_err(|e| e.to_string())?;
        if actual != self.data_dir {
            return Err("owner directory changed".into());
        }
        private_dir(&self.root).map_err(|e| e.to_string())
    }
    fn validate_context(&self, context: &CaptureContext) -> Result<(), String> {
        context.validate()?;
        if context.owner_scope != self.owner_scope
            || context.installation_id != self.installation_id
        {
            return Err("candidate owner or installation mismatch".into());
        }
        Ok(())
    }
    fn context_dir(
        &self,
        context: &CaptureContext,
        create: bool,
    ) -> Result<Option<PathBuf>, String> {
        self.verify_owner()?;
        self.validate_context(context)?;
        let key = context.digest();
        if !hex_digest(&key) {
            return Err("invalid context digest".into());
        }
        let dir = self.root.join(key);
        if !create {
            match std::fs::symlink_metadata(&dir) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(e.to_string()),
                Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
                    return Err("invalid candidate context directory".into())
                }
                _ => {}
            }
        }
        private_dir(&dir).map_err(|e| e.to_string())?;
        Ok(Some(dir))
    }
    fn receipt_for(&self, model: &RoomModel, bytes: &[u8]) -> CandidateReceipt {
        CandidateReceipt {
            schema: POINTER_SCHEMA.into(),
            owner_scope: self.owner_scope.clone(),
            installation_id: self.installation_id.clone(),
            room_digest: model.context().room_digest.clone(),
            context_digest: model.context().digest(),
            model_sha256: digest(bytes),
            model_version: model.version,
            parameter_count: model.parameter_count(),
        }
    }
    fn validate_receipt(
        &self,
        receipt: &CandidateReceipt,
        context: &CaptureContext,
    ) -> Result<(), String> {
        if receipt.schema != POINTER_SCHEMA
            || receipt.owner_scope != self.owner_scope
            || receipt.installation_id != self.installation_id
            || receipt.room_digest != context.room_digest
            || receipt.context_digest != context.digest()
            || !hex_digest(&receipt.model_sha256)
            || receipt.model_version != 1
            || !(1..=512).contains(&receipt.parameter_count)
        {
            return Err("candidate receipt/context mismatch".into());
        }
        Ok(())
    }
    fn read_model(
        &self,
        dir: &Path,
        receipt: &CandidateReceipt,
        context: &CaptureContext,
    ) -> Result<RoomModel, String> {
        self.validate_receipt(receipt, context)?;
        let bytes = private_read(&dir.join(format!("{}.json", receipt.model_sha256)))
            .map_err(|e| e.to_string())?;
        let envelope: Envelope = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if envelope.schema != MODEL_SCHEMA
            || &envelope.receipt != receipt
            || digest(envelope.learned_model_json.as_bytes()) != receipt.model_sha256
        {
            return Err("candidate immutable artifact integrity mismatch".into());
        }
        let model = RoomModel::import(envelope.learned_model_json.as_bytes())?;
        if model.context() != context || self.receipt_for(&model, &model.export()?) != *receipt {
            return Err("candidate learned model/context mismatch".into());
        }
        Ok(model)
    }
    fn read_pointer(
        &self,
        dir: &Path,
        context: &CaptureContext,
    ) -> Result<Option<CandidateReceipt>, String> {
        let path = dir.join(POINTER);
        if !regular_or_missing(&path)? {
            return Ok(None);
        }
        let receipt: CandidateReceipt =
            serde_json::from_slice(&private_read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        self.validate_receipt(&receipt, context)?;
        // A corrupt existing pointer/artifact is never silently overwritten.
        self.read_model(dir, &receipt, context)?;
        Ok(Some(receipt))
    }
    pub fn save_candidate(&self, model: &RoomModel) -> Result<CandidateReceipt, String> {
        self.validate_context(model.context())?;
        let bytes = model.export()?;
        let receipt = self.receipt_for(model, &bytes);
        let envelope = Envelope {
            schema: MODEL_SCHEMA.into(),
            receipt: receipt.clone(),
            learned_model_json: String::from_utf8(bytes).map_err(|e| e.to_string())?,
        };
        let encoded = encode(&envelope)?;
        let pointer = encode(&receipt)?;
        let dir = self
            .context_dir(model.context(), true)?
            .ok_or("missing context directory")?;
        self.read_pointer(&dir, model.context())?;
        let immutable = dir.join(format!("{}.json", receipt.model_sha256));
        if regular_or_missing(&immutable)? {
            if private_read(&immutable).map_err(|e| e.to_string())? != encoded {
                return Err("existing immutable candidate differs".into());
            }
        } else {
            // Actual private writer supplies 0600 + fsync. An exclusive link
            // installs the complete inode without replacing any previous model.
            let staging = dir.join(format!("model-stage-{}", crate::companion::secret()));
            atomic_private_write(&staging, &encoded).map_err(|e| e.to_string())?;
            let installed = std::fs::hard_link(&staging, &immutable);
            let cleanup = std::fs::remove_file(&staging);
            match installed {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    if private_read(&immutable).map_err(|e| e.to_string())? != encoded {
                        return Err("immutable candidate collision".into());
                    }
                }
                Err(e) => return Err(e.to_string()),
            }
            cleanup.map_err(|e| e.to_string())?;
            std::fs::File::open(&dir)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
        }
        self.read_model(&dir, &receipt, model.context())?;
        atomic_private_write(&dir.join(POINTER), &pointer).map_err(|e| e.to_string())?;
        if private_read(&dir.join(POINTER)).map_err(|e| e.to_string())? != pointer {
            return Err("candidate pointer readback mismatch".into());
        }
        Ok(receipt)
    }
    pub fn load_candidate(
        &self,
        expected_context: &CaptureContext,
    ) -> Result<Option<RoomModel>, String> {
        let Some(dir) = self.context_dir(expected_context, false)? else {
            return Ok(None);
        };
        let Some(receipt) = self.read_pointer(&dir, expected_context)? else {
            return Ok(None);
        };
        self.read_model(&dir, &receipt, expected_context).map(Some)
    }
}

#[cfg(test)]
#[path = "artifact_tests.rs"]
mod tests;

#[path="artifact_catalog.rs"]
mod catalog;
pub use catalog::{CandidateCatalog,SavedCandidate};

#[path="evaluation_store.rs"]
mod evaluation_store;
pub use evaluation_store::EvaluationReceipt;

#[path="selection_store.rs"]
mod selection_store;

#[path="assessment_store.rs"]
mod assessment_store;

#[path="managed_training_store.rs"]
mod managed_training_store;

#[path="managed_capture_fence_store.rs"]
mod managed_capture_fence_store;

#[path = "temporal_artifact_store.rs"]
mod temporal_artifacts;
pub use temporal_artifacts::TemporalCandidateReceipt;

#[path="temporal_fit_summary.rs"]
mod temporal_fit_summary;
pub(crate) use temporal_fit_summary::{TemporalFitDiagnostics,TemporalFitSaveResult};
pub(crate) use temporal_fit_summary::FitDiagnosticState;

#[cfg(test)]
pub(crate) use temporal_fit_summary::rehash_fixture as rehash_temporal_fit_fixture;

#[path="temporal_evaluation_store.rs"]
mod temporal_evaluation_store;
pub(crate) use temporal_evaluation_store::{StoredTemporalEvaluation,StoredTemporalAssessment};

#[path="temporal_selection_store.rs"]
mod temporal_selection_store;
pub(crate) use temporal_selection_store::StoredTemporalSelection;

#[path="temporal_comparison.rs"]
mod temporal_comparison;
pub(crate) use temporal_comparison::RecordedTemporalComparison;
