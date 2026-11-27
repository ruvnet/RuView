//! Persisted explicit room calibration.
//!
//! Unlike [`crate::bootstrap_baseline`], a calibration restored from this file
//! is allowed to authorize calibrated occupancy evidence and numeric vitals
//! until it expires. The file contains the field-model snapshot plus the
//! immutable calibration receipt, and is installation-bound and integrity
//! checked so a copied file from another installation fails closed.

use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use wifi_densepose_signal::ruvsense::field_model::{
    FieldModel, FieldModelError, FieldModelSnapshotV1,
};

use crate::bootstrap_baseline::{installation_binding, BootstrapCsiGrid};

pub const EXPLICIT_CALIBRATION_SCHEMA: &str = "ruview.explicit-calibration.v1";
pub const EXPLICIT_CALIBRATION_AUTHORITY: &str = "explicit_calibration";
/// Kept in sync with `field_bridge::CALIBRATION_MODEL_RECEIPT_SCHEMA`; this
/// module lives in the library while `field_bridge` is binary-local.
pub const EXPLICIT_RECEIPT_SCHEMA: &str = "ruview.calibration.field-model-receipt.v1";
const EXPLICIT_MAX_FILE_BYTES: u64 = 1_048_576;

/// The persisted subset of `CalibrationModelReceipt` needed to restore the
/// calibration identity after a restart.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExplicitReceipt {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct ExplicitPayloadV1 {
    schema: String,
    authority: String,
    installation_binding_sha256: String,
    source_node_ids: Vec<u8>,
    bound_source_node_id: u8,
    source_grid: BootstrapCsiGrid,
    source_model_id: String,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    receipt: ExplicitReceipt,
    field_model: FieldModelSnapshotV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct ExplicitImageV1 {
    payload: ExplicitPayloadV1,
    content_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplicitImageRawV1 {
    payload: Box<RawValue>,
    content_sha256: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExplicitMetadata {
    pub source_node_ids: Vec<u8>,
    pub bound_source_node_id: u8,
    pub source_grid: BootstrapCsiGrid,
    pub source_model_id: String,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub receipt: ExplicitReceipt,
    pub content_sha256: String,
}

#[derive(Debug, Error)]
pub enum ExplicitCalibrationError {
    #[error("explicit calibration persistence requires a stable installation_id")]
    MissingInstallationId,
    #[error("explicit calibration file is not a regular file")]
    InvalidPath,
    #[error("explicit calibration file is too large")]
    FileTooLarge,
    #[error("explicit calibration file is malformed: {0}")]
    Malformed(String),
    #[error("explicit calibration was created for a different installation")]
    InstallationMismatch,
    #[error("explicit calibration has expired")]
    Expired,
    #[error("explicit calibration integrity check failed")]
    DigestMismatch,
    #[error("explicit calibration field model is invalid: {0}")]
    FieldModel(#[from] FieldModelError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn path_in(data_dir: &Path) -> PathBuf {
    data_dir.join("calibration").join("explicit-v1.json")
}

/// Persist a completed calibration. Takes the already-exported field-model
/// snapshot so callers holding a `&mut FieldModel` borrow can drop it before
/// touching shared state.
pub fn store(
    path: &Path,
    installation_id: &str,
    source_node_ids: &[u8],
    bound_source_node_id: u8,
    source_grid: BootstrapCsiGrid,
    source_model_id: &str,
    receipt: &ExplicitReceipt,
    created_at_unix_ms: u64,
    snapshot: FieldModelSnapshotV1,
) -> Result<ExplicitMetadata, ExplicitCalibrationError> {
    let calibrated_at_ms = snapshot.modes.calibrated_at_us / 1_000;
    let expiry_ms = (snapshot.config.baseline_expiry_s * 1_000.0) as u64;
    let payload = ExplicitPayloadV1 {
        schema: EXPLICIT_CALIBRATION_SCHEMA.to_string(),
        authority: EXPLICIT_CALIBRATION_AUTHORITY.to_string(),
        installation_binding_sha256: installation_binding(installation_id)
            .map_err(|_| ExplicitCalibrationError::MissingInstallationId)?,
        source_node_ids: source_node_ids.to_vec(),
        bound_source_node_id,
        source_grid,
        source_model_id: source_model_id.to_string(),
        created_at_unix_ms,
        expires_at_unix_ms: calibrated_at_ms.saturating_add(expiry_ms),
        receipt: receipt.clone(),
        field_model: snapshot,
    };
    validate_payload(&payload, installation_id, created_at_unix_ms)?;
    let content_sha256 = payload_digest(&payload)?;
    let image = ExplicitImageV1 {
        payload,
        content_sha256,
    };
    let bytes = serde_json::to_vec(&image)?;
    if bytes.len() as u64 > EXPLICIT_MAX_FILE_BYTES {
        return Err(ExplicitCalibrationError::FileTooLarge);
    }
    atomic_write_private(path, &bytes)?;
    Ok(metadata(&image))
}

pub fn load(
    path: &Path,
    installation_id: &str,
    current_unix_ms: u64,
) -> Result<(FieldModel, ExplicitMetadata), ExplicitCalibrationError> {
    let file_metadata = fs::symlink_metadata(path)?;
    if file_metadata.file_type().is_symlink() || !file_metadata.is_file() {
        return Err(ExplicitCalibrationError::InvalidPath);
    }
    if file_metadata.len() > EXPLICIT_MAX_FILE_BYTES {
        return Err(ExplicitCalibrationError::FileTooLarge);
    }
    let mut bytes = Vec::with_capacity(file_metadata.len() as usize);
    OpenOptions::new()
        .read(true)
        .open(path)?
        .take(EXPLICIT_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > EXPLICIT_MAX_FILE_BYTES {
        return Err(ExplicitCalibrationError::FileTooLarge);
    }
    let raw_image: ExplicitImageRawV1 = serde_json::from_slice(&bytes)?;
    if hex_sha256(raw_image.payload.get().as_bytes()) != raw_image.content_sha256 {
        return Err(ExplicitCalibrationError::DigestMismatch);
    }
    let payload: ExplicitPayloadV1 = serde_json::from_str(raw_image.payload.get())?;
    validate_payload(&payload, installation_id, current_unix_ms)?;
    let image = ExplicitImageV1 {
        payload,
        content_sha256: raw_image.content_sha256,
    };
    let model = FieldModel::from_snapshot(
        image.payload.field_model.clone(),
        current_unix_ms.saturating_mul(1_000),
    )?;
    Ok((model, metadata(&image)))
}

pub fn remove(path: &Path) -> Result<bool, ExplicitCalibrationError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(ExplicitCalibrationError::InvalidPath);
            }
            fs::remove_file(path)?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn validate_payload(
    payload: &ExplicitPayloadV1,
    installation_id: &str,
    current_unix_ms: u64,
) -> Result<(), ExplicitCalibrationError> {
    let mut sorted_nodes = payload.source_node_ids.clone();
    sorted_nodes.sort_unstable();
    sorted_nodes.dedup();
    if payload.schema != EXPLICIT_CALIBRATION_SCHEMA
        || payload.authority != EXPLICIT_CALIBRATION_AUTHORITY
        || payload.source_model_id.is_empty()
        || payload.source_model_id.len() > 128
        || payload.source_node_ids.is_empty()
        || payload.source_node_ids.len() > 16
        || sorted_nodes != payload.source_node_ids
        || !payload.source_node_ids.contains(&payload.bound_source_node_id)
        || !(1..=2_048).contains(&payload.source_grid.n_subcarriers)
        || payload.source_grid.ppdu_type > 3
        || payload.created_at_unix_ms == 0
        || payload.expires_at_unix_ms <= payload.created_at_unix_ms
    {
        return Err(ExplicitCalibrationError::Malformed(
            "invalid schema, authority, identity, node set, source grid, or lifetime".into(),
        ));
    }
    let receipt = &payload.receipt;
    if receipt.schema != EXPLICIT_RECEIPT_SCHEMA
        || receipt.model_id != payload.source_model_id
        || receipt.source_node_ids != payload.source_node_ids
        || receipt.boot_epoch.is_empty()
        || receipt.session_id.is_empty()
        || receipt.binding_digest.len() != 64
        || !receipt
            .binding_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || receipt.frame_count == 0
        || !receipt.variance_explained.is_finite()
        || receipt.completed_at_unix_ms == 0
        || receipt.completed_at_unix_ms > payload.created_at_unix_ms
    {
        return Err(ExplicitCalibrationError::Malformed(
            "receipt does not match the persisted model".into(),
        ));
    }
    if payload.installation_binding_sha256
        != installation_binding(installation_id)
            .map_err(|_| ExplicitCalibrationError::MissingInstallationId)?
    {
        return Err(ExplicitCalibrationError::InstallationMismatch);
    }
    if current_unix_ms > payload.expires_at_unix_ms {
        return Err(ExplicitCalibrationError::Expired);
    }
    Ok(())
}

fn payload_digest(payload: &ExplicitPayloadV1) -> Result<String, serde_json::Error> {
    Ok(hex_sha256(&serde_json::to_vec(payload)?))
}

fn metadata(image: &ExplicitImageV1) -> ExplicitMetadata {
    ExplicitMetadata {
        source_node_ids: image.payload.source_node_ids.clone(),
        bound_source_node_id: image.payload.bound_source_node_id,
        source_grid: image.payload.source_grid,
        source_model_id: image.payload.source_model_id.clone(),
        created_at_unix_ms: image.payload.created_at_unix_ms,
        expires_at_unix_ms: image.payload.expires_at_unix_ms,
        receipt: image.payload.receipt.clone(),
        content_sha256: image.content_sha256.clone(),
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn atomic_write_private(path: &Path, bytes: &[u8]) -> Result<(), ExplicitCalibrationError> {
    let parent = path
        .parent()
        .ok_or(ExplicitCalibrationError::InvalidPath)?;
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ExplicitCalibrationError::InvalidPath)?;
    let mut nonce = [0_u8; 8];
    OsRng.fill_bytes(&mut nonce);
    let temporary = parent.join(format!(
        ".{file_name}.{:016x}.tmp",
        u64::from_le_bytes(nonce)
    ));

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let write_result = (|| -> Result<(), std::io::Error> {
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result.map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wifi_densepose_signal::ruvsense::field_model::FieldModelConfig;

    const TEST_GRID: BootstrapCsiGrid = BootstrapCsiGrid {
        n_subcarriers: 192,
        ppdu_type: 2,
    };

    fn test_receipt(model_id: &str, nodes: &[u8], completed_at_unix_ms: u64) -> ExplicitReceipt {
        ExplicitReceipt {
            schema: EXPLICIT_RECEIPT_SCHEMA.to_string(),
            boot_epoch: "cal-boot-test".to_string(),
            session_id: "cal-session-test".to_string(),
            model_id: model_id.to_string(),
            binding_digest: "ab".repeat(32),
            source_node_ids: nodes.to_vec(),
            frame_count: 1_200,
            variance_explained: 0.91,
            baseline_eigenvalue_count: 3,
            completed_at_unix_ms,
        }
    }

    fn completed_model(now_us: u64) -> FieldModel {
        let mut model = FieldModel::new(FieldModelConfig {
            n_links: 1,
            n_subcarriers: 4,
            n_modes: 2,
            min_calibration_frames: 3,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 3_600.0,
        })
        .unwrap();
        for index in 0..3 {
            model
                .feed_calibration(&[vec![1.0 + index as f64, 2.0, 3.0, 4.0]])
                .unwrap();
        }
        model.finalize_calibration(now_us, 7).unwrap();
        model
    }

    #[test]
    fn round_trip_restores_the_model_and_receipt() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let now_ms = 1_000_000;
        let model = completed_model(now_ms * 1_000);
        let receipt = test_receipt("cal-model-test", &[5], now_ms);
        let stored = store(
            &path,
            "home-a",
            &[5],
            5,
            TEST_GRID,
            "cal-model-test",
            &receipt,
            now_ms,
            model.export_snapshot().unwrap(),
        )
        .unwrap();
        assert_eq!(stored.source_grid, TEST_GRID);
        assert_eq!(stored.bound_source_node_id, 5);
        assert_eq!(stored.receipt, receipt);

        let (restored, loaded) = load(&path, "home-a", now_ms + 1).unwrap();
        assert_eq!(loaded.receipt, receipt);
        assert_eq!(
            restored.export_snapshot().unwrap(),
            model.export_snapshot().unwrap()
        );

        // A different installation must fail closed.
        assert!(matches!(
            load(&path, "home-b", now_ms + 1),
            Err(ExplicitCalibrationError::InstallationMismatch)
        ));
    }

    #[test]
    fn tamper_expiry_and_receipt_mismatch_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let now_ms = 1_000_000;
        let model = completed_model(now_ms * 1_000);
        let receipt = test_receipt("cal-model-test", &[5], now_ms);
        store(
            &path,
            "home-a",
            &[5],
            5,
            TEST_GRID,
            "cal-model-test",
            &receipt,
            now_ms,
            model.export_snapshot().unwrap(),
        )
        .unwrap();

        let mut image: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        image["payload"]["receipt"]["frame_count"] = serde_json::json!(1);
        fs::write(&path, serde_json::to_vec(&image).unwrap()).unwrap();
        assert!(matches!(
            load(&path, "home-a", now_ms + 1),
            Err(ExplicitCalibrationError::DigestMismatch)
        ));

        let receipt = test_receipt("cal-model-test", &[5], now_ms);
        store(
            &path,
            "home-a",
            &[5],
            5,
            TEST_GRID,
            "cal-model-test",
            &receipt,
            now_ms,
            model.export_snapshot().unwrap(),
        )
        .unwrap();
        assert!(matches!(
            load(&path, "home-a", now_ms + 3_600_001),
            Err(ExplicitCalibrationError::Expired)
                | Err(ExplicitCalibrationError::FieldModel(
                    FieldModelError::BaselineExpired { .. }
                ))
        ));

        // A receipt whose model_id disagrees with the stored model identity
        // must be rejected, not silently restored.
        let forged = test_receipt("cal-model-other", &[5], now_ms);
        assert!(matches!(
            store(
                &path,
                "home-a",
                &[5],
                5,
                TEST_GRID,
                "cal-model-test",
                &forged,
                now_ms,
                model.export_snapshot().unwrap(),
            ),
            Err(ExplicitCalibrationError::Malformed(_))
        ));
    }
}
