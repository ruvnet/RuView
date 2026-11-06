//! Whole temporal-input diagnostics in the EXISTING owner-bound ArtifactStore.
//! No runtime caller, managed proof, qualification, selection or inner-v1 access.
use super::*;
use crate::room_learner::temporal_model_input::TemporalModel;
use std::{fs, io::Read};

// Stable kind namespace; codec kind/pointer are versioned inside it.
// This reader supports only the existing d4a v2 codec, with no upconversion.
pub(super) const DIRECTORY: &str = "temporal-models";
const SCHEMA: &str = "ruview.room-learner.temporal-candidate.v1";
const KIND: &str = "ruview.experimental.temporal-model-input.v2";
const MANAGED_KIND: &str = "ruview.room-learner.managed-temporal-input.v3";
const MANAGED_POINTER: &str = "candidate-managed-temporal-v3.json";
const BLOB_LIMIT: usize = 4 * 1024 * 1024;
const POINTER: &str = "candidate-temporal-v2.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TemporalCandidateReceipt {
    pub schema: String,
    pub artifact_kind: String,
    pub owner_scope: String,
    pub installation_id: String,
    pub context_digest: String,
    pub room_digest: String,
    pub artifact_sha256: String,
    pub input_policy_sha256: String,
    pub parameter_count: usize,
    pub diagnostic_only: bool,
    pub physically_validated: bool,
    pub qualification_granted: bool,
}
fn metadata(meta: &fs::Metadata, directory: bool) -> Result<(), String> {
    if meta.file_type().is_symlink()
        || (directory && !meta.is_dir())
        || (!directory && !meta.is_file())
    {
        return Err("temporal_artifact_unsafe_path".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        extern "C" {
            fn geteuid() -> u32;
        }
        // SAFETY: geteuid has no arguments or memory side effects.
        if meta.uid() != unsafe { geteuid() }
            || meta.permissions().mode() & 0o7777 != if directory { 0o700 } else { 0o600 }
        {
            return Err("temporal_artifact_private_permissions".into());
        }
    }
    #[cfg(not(unix))]
    {
        return Err("temporal_artifact_private_permissions_unsupported".into());
    }
    #[allow(unreachable_code)]
    Ok(())
}
fn directory(path: &Path, create: bool) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            metadata(&meta, true)?;
            return Ok(true);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound && !create => return Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err("temporal_artifact_directory_unavailable".into()),
    }
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err("temporal_artifact_directory_unavailable".into()),
    }
    metadata(
        &fs::symlink_metadata(path).map_err(|_| "temporal_artifact_directory_unavailable")?,
        true,
    )?;
    Ok(true)
}
// Only fixed callers select 64KiB pointer or 4MiB temporal-codec limits. This is
// not a replacement/global enlargement of companion::private_read.
pub(super) fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let check = |meta: &fs::Metadata| -> Result<(), String> {
        metadata(meta, false)?;
        if meta.len() > limit as u64 {
            return Err("temporal_artifact_size_limit".into());
        }
        Ok(())
    };
    check(&fs::symlink_metadata(path).map_err(|_| "temporal_artifact_unavailable")?)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x100);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x20000);
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        return Err("temporal_artifact_nofollow_unsupported".into());
    }
    let file = options
        .open(path)
        .map_err(|_| "temporal_artifact_unavailable")?;
    check(
        &file
            .metadata()
            .map_err(|_| "temporal_artifact_unavailable")?,
    )?;
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "temporal_artifact_read_failed")?;
    if bytes.len() > limit {
        return Err("temporal_artifact_size_limit".into());
    }
    Ok(bytes)
}
pub(super) fn immutable(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > BLOB_LIMIT {
        return Err("temporal_artifact_size_limit".into());
    }
    if regular_or_missing(path)? {
        return if bounded_read(path, BLOB_LIMIT)? == bytes {
            Ok(())
        } else {
            Err("temporal_artifact_immutable_mismatch".into())
        };
    }
    let parent = path
        .parent()
        .ok_or("temporal_artifact_directory_unavailable")?;
    let stage = parent.join(format!("temporal-stage-{}", crate::companion::secret()));
    atomic_private_write(&stage, bytes).map_err(|_| "temporal_artifact_stage_failed")?;
    let result = fs::hard_link(&stage, path);
    let cleanup = fs::remove_file(&stage);
    match result {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if bounded_read(path, BLOB_LIMIT)? != bytes {
                return Err("temporal_artifact_immutable_mismatch".into());
            }
        }
        Err(_) => return Err("temporal_artifact_install_failed".into()),
    }
    cleanup.map_err(|_| "temporal_artifact_stage_cleanup_failed")?;
    fs::File::open(parent)
        .and_then(|file| file.sync_all())
        .map_err(|_| "temporal_artifact_sync_failed")?;
    Ok(())
}
impl ArtifactStore {
    pub(super) fn temporal_directory(
        &self,
        context: &CaptureContext,
        create: bool,
    ) -> Result<Option<PathBuf>, String> {
        self.verify_owner()?;
        self.validate_context(context)?;
        let kind = self.root.join(DIRECTORY);
        if !directory(&kind, create)? {
            return Ok(None);
        }
        let path = kind.join(context.digest());
        if !directory(&path, create)? {
            return Ok(None);
        }
        Ok(Some(path))
    }
    fn temporal_receipt(&self, model: &TemporalModel, bytes: &[u8]) -> TemporalCandidateReceipt {
        TemporalCandidateReceipt {
            schema: SCHEMA.into(),
            artifact_kind: model.artifact_kind().into(),
            owner_scope: self.owner_scope.clone(),
            installation_id: self.installation_id.clone(),
            context_digest: model.context().digest(),
            room_digest: model.context().room_digest.clone(),
            artifact_sha256: digest(bytes),
            input_policy_sha256: model.plan_sha256().into(),
            parameter_count: model.parameter_count(),
            diagnostic_only: true,
            physically_validated: false,
            qualification_granted: false,
        }
    }
    fn validate_temporal_receipt(
        &self,
        receipt: &TemporalCandidateReceipt,
        context: &CaptureContext,
    ) -> Result<(), String> {
        self.validate_context(context)?;
        if receipt.schema != SCHEMA
            || !matches!(receipt.artifact_kind.as_str(), KIND | MANAGED_KIND)
            || receipt.owner_scope != self.owner_scope
            || receipt.installation_id != self.installation_id
            || receipt.context_digest != context.digest()
            || receipt.room_digest != context.room_digest
            || !hex_digest(&receipt.artifact_sha256)
            || !hex_digest(&receipt.input_policy_sha256)
            || !(1..=512).contains(&receipt.parameter_count)
            || !receipt.diagnostic_only
            || receipt.physically_validated
            || receipt.qualification_granted
        {
            return Err("temporal_artifact_receipt_mismatch".into());
        }
        Ok(())
    }
    fn temporal_read_model(
        &self,
        path: &Path,
        receipt: &TemporalCandidateReceipt,
        context: &CaptureContext,
    ) -> Result<TemporalModel, String> {
        self.validate_temporal_receipt(receipt, context)?;
        let bytes = bounded_read(
            &path.join(format!("{}.json", receipt.artifact_sha256)),
            BLOB_LIMIT,
        )?;
        if digest(&bytes) != receipt.artifact_sha256 {
            return Err("temporal_artifact_hash_mismatch".into());
        }
        let model = TemporalModel::import(&bytes)?;
        if model.context() != context || self.temporal_receipt(&model, &model.export()?) != *receipt
        {
            return Err("temporal_artifact_context_or_policy_mismatch".into());
        }
        Ok(model)
    }
    fn temporal_read_pointer(
        &self,
        path: &Path,
        context: &CaptureContext,
    ) -> Result<Option<TemporalCandidateReceipt>, String> {
        self.temporal_read_pointer_kind(path, context, KIND)
    }
    fn temporal_read_pointer_kind(
        &self,
        path: &Path,
        context: &CaptureContext,
        kind: &str,
    ) -> Result<Option<TemporalCandidateReceipt>, String> {
        let pointer = path.join(match kind {
            KIND => POINTER,
            MANAGED_KIND => MANAGED_POINTER,
            _ => return Err("temporal_artifact_kind_unsupported".into()),
        });
        if !regular_or_missing(&pointer)? {
            return Ok(None);
        }
        let receipt: TemporalCandidateReceipt =
            serde_json::from_slice(&bounded_read(&pointer, LIMIT)?)
                .map_err(|_| "temporal_artifact_pointer_invalid")?;
        if receipt.artifact_kind != kind {
            return Err("temporal_artifact_pointer_kind_changed".into());
        }
        self.temporal_read_model(path, &receipt, context)?;
        Ok(Some(receipt))
    }
    /// Diagnostic persistence only; it cannot create managed proof or selection.
    pub fn save_temporal_candidate(
        &self,
        model: &TemporalModel,
    ) -> Result<TemporalCandidateReceipt, String> {
        self.validate_context(model.context())?;
        let bytes = model.export()?;
        let receipt = self.temporal_receipt(model, &bytes);
        self.validate_temporal_receipt(&receipt, model.context())?;
        let pointer = encode(&receipt)?;
        let path = self
            .temporal_directory(model.context(), true)?
            .ok_or("temporal_artifact_directory_missing")?;
        self.temporal_read_pointer_kind(&path, model.context(), model.artifact_kind())?;
        immutable(
            &path.join(format!("{}.json", receipt.artifact_sha256)),
            &bytes,
        )?;
        self.temporal_read_model(&path, &receipt, model.context())?;
        let pointer_name = if model.artifact_kind() == KIND {
            POINTER
        } else if model.artifact_kind() == MANAGED_KIND {
            MANAGED_POINTER
        } else {
            return Err("temporal_artifact_kind_unsupported".into());
        };
        atomic_private_write(&path.join(pointer_name), &pointer)
            .map_err(|_| "temporal_artifact_pointer_write_failed")?;
        if bounded_read(&path.join(pointer_name), LIMIT)? != pointer {
            return Err("temporal_artifact_pointer_readback_failed".into());
        }
        self.temporal_read_pointer_kind(&path, model.context(), model.artifact_kind())?;
        Ok(receipt)
    }
    pub fn temporal_candidate_receipt(
        &self,
        context: &CaptureContext,
    ) -> Result<Option<TemporalCandidateReceipt>, String> {
        let Some(path) = self.temporal_directory(context, false)? else {
            return Ok(None);
        };
        self.temporal_read_pointer(&path, context)
    }
    pub fn load_temporal_candidate(
        &self,
        context: &CaptureContext,
    ) -> Result<Option<TemporalModel>, String> {
        let Some(path) = self.temporal_directory(context, false)? else {
            return Ok(None);
        };
        let Some(receipt) = self.temporal_read_pointer(&path, context)? else {
            return Ok(None);
        };
        self.temporal_read_model(&path, &receipt, context).map(Some)
    }
    pub fn load_managed_temporal_candidate(
        &self,
        context: &CaptureContext,
    ) -> Result<Option<TemporalModel>, String> {
        let Some(path) = self.temporal_directory(context, false)? else {
            return Ok(None);
        };
        let Some(receipt) = self.temporal_read_pointer_kind(&path, context, MANAGED_KIND)? else {
            return Ok(None);
        };
        self.temporal_read_model(&path, &receipt, context).map(Some)
    }
    pub fn load_exact_temporal_candidate(
        &self,
        context: &CaptureContext,
        artifact_sha256: &str,
    ) -> Result<TemporalModel, String> {
        if !hex_digest(artifact_sha256) {
            return Err("temporal_artifact_invalid_hash".into());
        }
        let path = self
            .temporal_directory(context, false)?
            .ok_or("temporal_artifact_directory_missing")?;
        let bytes = bounded_read(&path.join(format!("{artifact_sha256}.json")), BLOB_LIMIT)?;
        if digest(&bytes) != artifact_sha256 {
            return Err("temporal_artifact_hash_mismatch".into());
        }
        let model = TemporalModel::import(&bytes)?;
        if model.context() != context {
            return Err("temporal_artifact_context_mismatch".into());
        }
        Ok(model)
    }
}
#[cfg(all(test, unix))]
#[path = "temporal_artifact_store_tests.rs"]
mod tests;
