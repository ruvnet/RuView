//! Full room receipt companion to a separately validated aggregate model.
//! No CSI/history is stored. A bootstrap image alone never gains this authority.
use crate::bootstrap_baseline::{self, BootstrapBaselineError, BootstrapBaselineMetadata};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 65_536;
const SCHEMA: &str = "ruview.saved-room-calibration.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomReceipt {
    pub schema: String,
    pub boot_epoch: String,
    pub session_id: String,
    pub model_id: String,
    pub binding_digest: String,
    pub source_node_ids: Vec<u8>,
    pub frame_count: u64,
    pub variance_explained: f64,
    pub baseline_eigenvalue_count: usize,
    pub completed_at_unix_ms: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    schema: String,
    installation_binding_sha256: String,
    bootstrap_content_sha256: String,
    receipt: RoomReceipt,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: Box<RawValue>,
    content_sha256: String,
}
pub fn path_in(data_dir: &Path) -> PathBuf {
    data_dir.join("calibration").join("room-receipt-v1.json")
}
fn validate(
    payload: &Payload,
    installation: &str,
    model: &BootstrapBaselineMetadata,
) -> Result<(), BootstrapBaselineError> {
    let r = &payload.receipt;
    let valid_id = |id: &str, prefix: &str| {
        id.starts_with(prefix)
            && id.len() <= 80
            && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    let digest = |value: &str| {
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if payload.schema != SCHEMA
        || payload.installation_binding_sha256
            != bootstrap_baseline::installation_binding(installation)?
        || payload.bootstrap_content_sha256 != model.content_sha256
        || r.schema != "ruview.calibration.field-model-receipt.v1"
        || !valid_id(&r.boot_epoch, "cal-boot-")
        || !valid_id(&r.session_id, "cal-session-")
        || !valid_id(&r.model_id, "cal-model-")
        || r.model_id != model.source_model_id
        || !digest(&r.binding_digest)
        || r.frame_count < 1000
        || !r.variance_explained.is_finite()
        || !(0.0..=1.0).contains(&r.variance_explained)
        || r.baseline_eigenvalue_count > 2048
        || r.source_node_ids.is_empty()
        || r.source_node_ids.len() > 16
        || !r.source_node_ids.windows(2).all(|pair| pair[0] < pair[1])
        || model.source_node_ids.len() != 1
        || !r.source_node_ids.contains(&model.source_node_ids[0])
        || r.completed_at_unix_ms == 0
        || r.completed_at_unix_ms > model.created_at_unix_ms
        || r.completed_at_unix_ms >= model.expires_at_unix_ms
    {
        return Err(BootstrapBaselineError::InvalidPath);
    }
    Ok(())
}
pub fn store(
    path: &Path,
    installation: &str,
    model: &BootstrapBaselineMetadata,
    receipt: &RoomReceipt,
) -> Result<(), BootstrapBaselineError> {
    let payload = Payload {
        schema: SCHEMA.into(),
        installation_binding_sha256: bootstrap_baseline::installation_binding(installation)?,
        bootstrap_content_sha256: model.content_sha256.clone(),
        receipt: receipt.clone(),
    };
    validate(&payload, installation, model)?;
    let raw = serde_json::value::to_raw_value(&payload)?;
    let content_sha256 = bootstrap_baseline::hex_sha256(raw.get().as_bytes());
    let bytes = serde_json::to_vec(&Envelope {
        payload: raw,
        content_sha256,
    })?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(BootstrapBaselineError::FileTooLarge);
    }
    bootstrap_baseline::atomic_write_private(path, &bytes)
}
pub fn load(
    path: &Path,
    installation: &str,
    model: &BootstrapBaselineMetadata,
) -> Result<RoomReceipt, BootstrapBaselineError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(BootstrapBaselineError::InvalidPath);
    }
    if metadata.len() > MAX_BYTES {
        return Err(BootstrapBaselineError::FileTooLarge);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(BootstrapBaselineError::FileTooLarge);
    }
    let envelope: Envelope = serde_json::from_slice(&bytes)?;
    if bootstrap_baseline::hex_sha256(envelope.payload.get().as_bytes()) != envelope.content_sha256
    {
        return Err(BootstrapBaselineError::InvalidPath);
    }
    let payload: Payload = serde_json::from_str(envelope.payload.get())?;
    validate(&payload, installation, model)?;
    Ok(payload.receipt)
}
pub fn remove(data_dir: &Path) -> Result<(), BootstrapBaselineError> {
    match fs::remove_file(path_in(data_dir)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap_baseline::BootstrapCsiGrid;
    fn fixture() -> (BootstrapBaselineMetadata, RoomReceipt) {
        (
            BootstrapBaselineMetadata {
                authority: "bootstrap_only",
                source_node_ids: vec![15],
                source_grid: BootstrapCsiGrid {
                    n_subcarriers: 56,
                    ppdu_type: 1,
                },
                source_model_id: "cal-model-abc".into(),
                created_at_unix_ms: 2000,
                expires_at_unix_ms: 9000,
                content_sha256: "ab".repeat(32),
            },
            RoomReceipt {
                schema: "ruview.calibration.field-model-receipt.v1".into(),
                boot_epoch: "cal-boot-abc".into(),
                session_id: "cal-session-abc".into(),
                model_id: "cal-model-abc".into(),
                binding_digest: "cd".repeat(32),
                source_node_ids: vec![15, 16, 17],
                frame_count: 18942,
                variance_explained: 0.9800000000000001,
                baseline_eigenvalue_count: 5,
                completed_at_unix_ms: 1900,
            },
        )
    }
    #[test]
    fn exact_roundtrip_and_foreign_owner_or_replaced_model_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let (metadata, receipt) = fixture();
        store(&path, "A", &metadata, &receipt).unwrap();
        let read = load(&path, "A", &metadata).unwrap();
        assert_eq!(read.source_node_ids, vec![15, 16, 17]);
        assert_eq!(read.frame_count, 18942);
        assert!(load(&path, "B", &metadata).is_err());
        let mut replaced = metadata.clone();
        replaced.content_sha256 = "ef".repeat(32);
        assert!(load(&path, "A", &replaced).is_err());
        let mut changed = receipt;
        changed.source_node_ids = vec![16, 17];
        assert!(store(&path, "A", &metadata, &changed).is_err());
        remove(dir.path()).unwrap();
        assert!(!path.exists());
    }
    #[test]
    fn corrupt_oversized_and_unfinished_receipts_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let (metadata, mut receipt) = fixture();
        store(&path, "A", &metadata, &receipt).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        bytes[20] ^= 1;
        fs::write(&path, bytes).unwrap();
        assert!(load(&path, "A", &metadata).is_err());
        fs::write(&path, vec![b' '; MAX_BYTES as usize + 1]).unwrap();
        assert!(load(&path, "A", &metadata).is_err());
        receipt.frame_count = 999;
        assert!(store(&path, "A", &metadata, &receipt).is_err());
    }
}
