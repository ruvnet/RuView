//! Local, privacy reduced empty room bootstrap model persistence.
//!
//! A bootstrap image is deliberately lower authority than a completed room
//! calibration. It can suppress background perturbations while the server
//! starts, but restoring it never authorizes calibrated evidence or vitals.

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

pub const BOOTSTRAP_BASELINE_SCHEMA: &str = "ruview.bootstrap-empty-field-model.v2";
pub const BOOTSTRAP_BASELINE_AUTHORITY: &str = "bootstrap_only";
pub const BOOTSTRAP_VALIDATION_SAMPLES: usize = 12;
pub const BOOTSTRAP_VALIDATION_MIN_EMPTY: usize = 10;
pub const BOOTSTRAP_VALIDATION_MIN_SPACING_MS: u64 = 1_000;
pub const BOOTSTRAP_VALIDATION_SAMPLE_TIMEOUT_MS: u64 = 4_000;
const BOOTSTRAP_MAX_FILE_BYTES: u64 = 1_048_576;

/// Exact CSI layout used to train the persisted empty room model.
///
/// Restoring code must require incoming frames to match this binding before
/// the bootstrap image can influence inference.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BootstrapCsiGrid {
    pub n_subcarriers: u16,
    pub ppdu_type: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct BootstrapPayloadV2 {
    schema: String,
    authority: String,
    installation_binding_sha256: String,
    source_node_ids: Vec<u8>,
    source_grid: BootstrapCsiGrid,
    source_model_id: String,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    field_model: FieldModelSnapshotV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct BootstrapImageV2 {
    payload: BootstrapPayloadV2,
    content_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapImageRawV2 {
    payload: Box<RawValue>,
    content_sha256: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BootstrapBaselineMetadata {
    pub authority: &'static str,
    pub source_node_ids: Vec<u8>,
    pub source_grid: BootstrapCsiGrid,
    pub source_model_id: String,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootstrapValidationSample {
    pub fresh_tick: bool,
    pub calibrated_empty: bool,
    pub vital_signs_absent: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct BootstrapValidationResult {
    pub sample_count: usize,
    pub empty_sample_count: usize,
    pub stale_sample_count: usize,
    pub vital_sign_sample_count: usize,
    pub passed: bool,
}

/// Independent ADR-355 requirements. Refinement does not count empty votes.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BootstrapValidationRequirements {
    pub sample_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_empty_sample_count: Option<usize>,
    pub max_stale_sample_count: usize,
    pub max_vital_sign_sample_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BootstrapValidationFailure {
    pub check: &'static str,
    pub observed: usize,
    pub required: usize,
    pub comparison: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BootstrapValidationDiagnostics {
    pub requirements: BootstrapValidationRequirements,
    pub failures: Vec<BootstrapValidationFailure>,
}

impl BootstrapValidationResult {
    pub fn diagnostics(&self) -> BootstrapValidationDiagnostics {
        validation_diagnostics(
            self.sample_count,
            Some(self.empty_sample_count),
            self.stale_sample_count,
            self.vital_sign_sample_count,
        )
    }
}

impl BootstrapValidationDiagnostics {
    /// Describe every failed check, without interpreting estimates as people.
    pub fn failure_summary(&self) -> String {
        self.failures
            .iter()
            .map(|failure| match failure.check {
                "sample_count" => format!(
                    "{} samples collected (exactly {} required); check the CSI stream",
                    failure.observed, failure.required,
                ),
                "empty_sample_count" => format!(
                    "{} samples reported empty (at least {} required); vacate the room and check for moving objects",
                    failure.observed, failure.required,
                ),
                "stale_sample_count" => format!(
                    "{} stale samples ({} allowed); check the CSI stream",
                    failure.observed, failure.required,
                ),
                _ => format!(
                    "numeric vital-sign estimates in {}/{} samples ({} allowed); vacate the room and check for moving objects",
                    failure.observed, self.requirements.sample_count, failure.required,
                ),
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// Explain existing gate counters without changing how samples are counted.
/// Promotion supplies all attempted samples; refinement supplies only fresh
/// residuals. Stale samples are a separate check, never added to sample_count.
pub fn validation_diagnostics(
    sample_count: usize,
    empty_sample_count: Option<usize>,
    stale_sample_count: usize,
    vital_sign_sample_count: usize,
) -> BootstrapValidationDiagnostics {
    let requirements = BootstrapValidationRequirements {
        sample_count: BOOTSTRAP_VALIDATION_SAMPLES,
        min_empty_sample_count: empty_sample_count.map(|_| BOOTSTRAP_VALIDATION_MIN_EMPTY),
        max_stale_sample_count: 0,
        max_vital_sign_sample_count: 0,
    };
    let mut failures = Vec::new();
    let checks = [
        (
            "sample_count",
            sample_count,
            requirements.sample_count,
            "equal",
            sample_count != requirements.sample_count,
        ),
        (
            "empty_sample_count",
            empty_sample_count.unwrap_or(0),
            BOOTSTRAP_VALIDATION_MIN_EMPTY,
            "at_least",
            empty_sample_count.is_some_and(|count| count < BOOTSTRAP_VALIDATION_MIN_EMPTY),
        ),
        (
            "stale_sample_count",
            stale_sample_count,
            requirements.max_stale_sample_count,
            "at_most",
            stale_sample_count != 0,
        ),
        (
            "vital_sign_sample_count",
            vital_sign_sample_count,
            requirements.max_vital_sign_sample_count,
            "at_most",
            vital_sign_sample_count != 0,
        ),
    ];
    for (check, observed, required, comparison, failed) in checks {
        if failed {
            failures.push(BootstrapValidationFailure {
                check,
                observed,
                required,
                comparison,
            });
        }
    }
    BootstrapValidationDiagnostics {
        requirements,
        failures,
    }
}

#[derive(Debug, Error)]
pub enum BootstrapBaselineError {
    #[error("a stable installation_id is required for a local bootstrap baseline")]
    MissingInstallationId,
    #[error("bootstrap baseline path is invalid")]
    InvalidPath,
    #[error("bootstrap baseline file is too large")]
    FileTooLarge,
    #[error("bootstrap baseline image is malformed: {0}")]
    Malformed(String),
    #[error("bootstrap baseline belongs to another installation")]
    InstallationMismatch,
    #[error("bootstrap baseline has expired")]
    Expired,
    #[error("bootstrap baseline digest does not verify")]
    DigestMismatch,
    #[error("field model snapshot is invalid: {0}")]
    FieldModel(#[from] FieldModelError),
    #[error("bootstrap baseline storage failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("bootstrap baseline serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn path_in(data_dir: &Path) -> PathBuf {
    data_dir.join("calibration").join("bootstrap-empty-v1.json")
}

pub fn installation_binding(installation_id: &str) -> Result<String, BootstrapBaselineError> {
    let installation_id = installation_id.trim();
    if installation_id.is_empty() || installation_id.len() > 128 {
        return Err(BootstrapBaselineError::MissingInstallationId);
    }
    Ok(hex_sha256(installation_id.as_bytes()))
}

pub fn evaluate_validation(samples: &[BootstrapValidationSample]) -> BootstrapValidationResult {
    let empty_sample_count = samples
        .iter()
        .filter(|sample| sample.fresh_tick && sample.calibrated_empty)
        .count();
    let stale_sample_count = samples.iter().filter(|sample| !sample.fresh_tick).count();
    let vital_sign_sample_count = samples
        .iter()
        .filter(|sample| !sample.vital_signs_absent)
        .count();
    BootstrapValidationResult {
        sample_count: samples.len(),
        empty_sample_count,
        stale_sample_count,
        vital_sign_sample_count,
        passed: samples.len() == BOOTSTRAP_VALIDATION_SAMPLES
            && empty_sample_count >= BOOTSTRAP_VALIDATION_MIN_EMPTY
            && stale_sample_count == 0
            && vital_sign_sample_count == 0,
    }
}

pub fn store(
    path: &Path,
    installation_id: &str,
    source_node_ids: &[u8],
    source_grid: BootstrapCsiGrid,
    source_model_id: &str,
    created_at_unix_ms: u64,
    field_model: &FieldModel,
) -> Result<BootstrapBaselineMetadata, BootstrapBaselineError> {
    let snapshot = field_model.export_snapshot()?;
    let calibrated_at_ms = snapshot.modes.calibrated_at_us / 1_000;
    let expiry_ms = (snapshot.config.baseline_expiry_s * 1_000.0) as u64;
    let payload = BootstrapPayloadV2 {
        schema: BOOTSTRAP_BASELINE_SCHEMA.to_string(),
        authority: BOOTSTRAP_BASELINE_AUTHORITY.to_string(),
        installation_binding_sha256: installation_binding(installation_id)?,
        source_node_ids: source_node_ids.to_vec(),
        source_grid,
        source_model_id: source_model_id.to_string(),
        created_at_unix_ms,
        expires_at_unix_ms: calibrated_at_ms.saturating_add(expiry_ms),
        field_model: snapshot,
    };
    validate_payload(&payload, installation_id, created_at_unix_ms)?;
    let content_sha256 = payload_digest(&payload)?;
    let image = BootstrapImageV2 {
        payload,
        content_sha256,
    };
    let bytes = serde_json::to_vec(&image)?;
    if bytes.len() as u64 > BOOTSTRAP_MAX_FILE_BYTES {
        return Err(BootstrapBaselineError::FileTooLarge);
    }
    atomic_write_private(path, &bytes)?;
    Ok(metadata(&image))
}

pub fn load(
    path: &Path,
    installation_id: &str,
    current_unix_ms: u64,
) -> Result<(FieldModel, BootstrapBaselineMetadata), BootstrapBaselineError> {
    let file_metadata = fs::symlink_metadata(path)?;
    if file_metadata.file_type().is_symlink() || !file_metadata.is_file() {
        return Err(BootstrapBaselineError::InvalidPath);
    }
    if file_metadata.len() > BOOTSTRAP_MAX_FILE_BYTES {
        return Err(BootstrapBaselineError::FileTooLarge);
    }
    let mut bytes = Vec::with_capacity(file_metadata.len() as usize);
    OpenOptions::new()
        .read(true)
        .open(path)?
        .take(BOOTSTRAP_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > BOOTSTRAP_MAX_FILE_BYTES {
        return Err(BootstrapBaselineError::FileTooLarge);
    }
    // Verify the exact payload bytes written by storage before decoding the
    // floating point model. Equivalent JSON number spellings may serialize
    // differently, so decode and reserialize is not an integrity contract.
    let raw_image: BootstrapImageRawV2 = serde_json::from_slice(&bytes)?;
    if hex_sha256(raw_image.payload.get().as_bytes()) != raw_image.content_sha256 {
        return Err(BootstrapBaselineError::DigestMismatch);
    }
    let payload: BootstrapPayloadV2 = serde_json::from_str(raw_image.payload.get())?;
    validate_payload(&payload, installation_id, current_unix_ms)?;
    let image = BootstrapImageV2 {
        payload,
        content_sha256: raw_image.content_sha256,
    };
    let model = FieldModel::from_snapshot(
        image.payload.field_model.clone(),
        current_unix_ms.saturating_mul(1_000),
    )?;
    Ok((model, metadata(&image)))
}

pub fn remove(path: &Path) -> Result<bool, BootstrapBaselineError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(BootstrapBaselineError::InvalidPath);
            }
            fs::remove_file(path)?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn validate_payload(
    payload: &BootstrapPayloadV2,
    installation_id: &str,
    current_unix_ms: u64,
) -> Result<(), BootstrapBaselineError> {
    if payload.schema != BOOTSTRAP_BASELINE_SCHEMA
        || payload.authority != BOOTSTRAP_BASELINE_AUTHORITY
        || payload.source_model_id.is_empty()
        || payload.source_model_id.len() > 128
        || payload.source_node_ids.len() != 1
        || !(1..=2_048).contains(&payload.source_grid.n_subcarriers)
        || payload.source_grid.ppdu_type > 3
        || payload.created_at_unix_ms == 0
        || payload.expires_at_unix_ms <= payload.created_at_unix_ms
    {
        return Err(BootstrapBaselineError::Malformed(
            "invalid schema, authority, identity, node set, source grid, or lifetime".into(),
        ));
    }
    if payload.installation_binding_sha256 != installation_binding(installation_id)? {
        return Err(BootstrapBaselineError::InstallationMismatch);
    }
    if current_unix_ms > payload.expires_at_unix_ms {
        return Err(BootstrapBaselineError::Expired);
    }
    Ok(())
}

fn payload_digest(payload: &BootstrapPayloadV2) -> Result<String, serde_json::Error> {
    Ok(hex_sha256(&serde_json::to_vec(payload)?))
}

fn metadata(image: &BootstrapImageV2) -> BootstrapBaselineMetadata {
    BootstrapBaselineMetadata {
        authority: BOOTSTRAP_BASELINE_AUTHORITY,
        source_node_ids: image.payload.source_node_ids.clone(),
        source_grid: image.payload.source_grid,
        source_model_id: image.payload.source_model_id.clone(),
        created_at_unix_ms: image.payload.created_at_unix_ms,
        expires_at_unix_ms: image.payload.expires_at_unix_ms,
        content_sha256: image.content_sha256.clone(),
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn atomic_write_private(path: &Path, bytes: &[u8]) -> Result<(), BootstrapBaselineError> {
    let parent = path.parent().ok_or(BootstrapBaselineError::InvalidPath)?;
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(BootstrapBaselineError::InvalidPath)?;
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
    fn round_trip_is_installation_bound_and_contains_no_raw_frames() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let now_ms = 1_000_000;
        let model = completed_model(now_ms * 1_000);
        let stored = store(&path, "home-a", &[5], TEST_GRID, "model", now_ms, &model).unwrap();
        let text = String::from_utf8(fs::read(&path).unwrap()).unwrap();
        assert!(!text.contains("home-a"));
        assert!(!text.contains("raw_csi"));
        assert_eq!(stored.source_grid, TEST_GRID);

        let (restored, loaded) = load(&path, "home-a", now_ms + 1).unwrap();
        assert_eq!(stored, loaded);
        assert_eq!(
            restored.export_snapshot().unwrap(),
            model.export_snapshot().unwrap()
        );
        assert!(matches!(
            load(&path, "home-b", now_ms + 1),
            Err(BootstrapBaselineError::InstallationMismatch)
        ));
    }

    #[test]
    fn tamper_and_expiry_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let now_ms = 1_000_000;
        let model = completed_model(now_ms * 1_000);
        store(&path, "home-a", &[5], TEST_GRID, "model", now_ms, &model).unwrap();

        let mut image: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        image["payload"]["source_grid"]["n_subcarriers"] = serde_json::json!(64);
        fs::write(&path, serde_json::to_vec(&image).unwrap()).unwrap();
        assert!(matches!(
            load(&path, "home-a", now_ms + 1),
            Err(BootstrapBaselineError::DigestMismatch)
        ));

        store(&path, "home-a", &[5], TEST_GRID, "model", now_ms, &model).unwrap();
        assert!(matches!(
            load(&path, "home-a", now_ms + 3_600_001),
            Err(BootstrapBaselineError::Expired)
                | Err(BootstrapBaselineError::FieldModel(
                    FieldModelError::BaselineExpired { .. }
                ))
        ));
    }

    #[test]
    fn store_rejects_invalid_source_grid() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let now_ms = 1_000_000;
        let model = completed_model(now_ms * 1_000);

        for source_grid in [
            BootstrapCsiGrid {
                n_subcarriers: 0,
                ppdu_type: 0,
            },
            BootstrapCsiGrid {
                n_subcarriers: 2_049,
                ppdu_type: 0,
            },
            BootstrapCsiGrid {
                n_subcarriers: 192,
                ppdu_type: 4,
            },
        ] {
            assert!(matches!(
                store(&path, "home-a", &[5], source_grid, "model", now_ms, &model,),
                Err(BootstrapBaselineError::Malformed(_))
            ));
        }
    }

    #[test]
    fn store_requires_exactly_one_source_node() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let now_ms = 1_000_000;
        let model = completed_model(now_ms * 1_000);

        for source_node_ids in [&[][..], &[5, 7][..]] {
            assert!(matches!(
                store(
                    &path,
                    "home-a",
                    source_node_ids,
                    TEST_GRID,
                    "model",
                    now_ms,
                    &model,
                ),
                Err(BootstrapBaselineError::Malformed(_))
            ));
        }
    }

    #[test]
    fn source_grid_is_required_when_loading_a_v2_image() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let now_ms = 1_000_000;
        let model = completed_model(now_ms * 1_000);
        store(&path, "home-a", &[5], TEST_GRID, "model", now_ms, &model).unwrap();

        let mut image: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        image["payload"]
            .as_object_mut()
            .unwrap()
            .remove("source_grid");
        let payload = serde_json::to_vec(&image["payload"]).unwrap();
        let rewritten = format!(
            "{{\"payload\":{},\"content_sha256\":\"{}\"}}",
            String::from_utf8(payload.clone()).unwrap(),
            hex_sha256(&payload)
        );
        fs::write(&path, rewritten).unwrap();

        assert!(matches!(
            load(&path, "home-a", now_ms + 1),
            Err(BootstrapBaselineError::Json(_))
        ));
    }

    #[test]
    fn promotion_gate_requires_twelve_fresh_samples_and_zero_vitals() {
        assert_eq!(BOOTSTRAP_VALIDATION_MIN_SPACING_MS, 1_000);
        assert_eq!(BOOTSTRAP_VALIDATION_SAMPLE_TIMEOUT_MS, 4_000);
        assert!(BOOTSTRAP_VALIDATION_SAMPLE_TIMEOUT_MS >= BOOTSTRAP_VALIDATION_MIN_SPACING_MS);
        let passing = vec![
            BootstrapValidationSample {
                fresh_tick: true,
                calibrated_empty: true,
                vital_signs_absent: true,
            };
            BOOTSTRAP_VALIDATION_SAMPLES
        ];
        assert!(evaluate_validation(&passing).passed);

        let mut stale = passing.clone();
        stale[0].fresh_tick = false;
        assert!(!evaluate_validation(&stale).passed);
        let mut vital = passing;
        vital[0].vital_signs_absent = false;
        assert!(!evaluate_validation(&vital).passed);
    }

    fn empty_validation_samples(count: usize) -> Vec<BootstrapValidationSample> {
        vec![
            BootstrapValidationSample {
                fresh_tick: true,
                calibrated_empty: true,
                vital_signs_absent: true,
            };
            count
        ]
    }

    #[test]
    fn validation_diagnostics_keep_empty_tolerance_separate_from_vitals() {
        let mut samples = empty_validation_samples(12);
        samples[0].calibrated_empty = false;
        samples[1].calibrated_empty = false;
        let passing = evaluate_validation(&samples);
        assert!(passing.passed);
        assert_eq!(passing.empty_sample_count, 10);
        assert!(passing.diagnostics().failures.is_empty());
        assert!(passing.diagnostics().failure_summary().is_empty());

        samples[2].calibrated_empty = false;
        let rejected = evaluate_validation(&samples);
        assert!(!rejected.passed);
        assert_eq!(
            rejected.diagnostics().failures,
            vec![BootstrapValidationFailure {
                check: "empty_sample_count",
                observed: 9,
                required: 10,
                comparison: "at_least",
            }]
        );
    }

    #[test]
    fn validation_diagnostics_explain_numeric_vital_veto_even_when_all_empty() {
        for count in [1, 5] {
            let mut samples = empty_validation_samples(12);
            for sample in samples.iter_mut().take(count) {
                sample.vital_signs_absent = false;
            }
            let result = evaluate_validation(&samples);
            assert!(!result.passed);
            assert_eq!(result.empty_sample_count, 12);
            let diagnostics = result.diagnostics();
            let json = serde_json::to_value(&diagnostics).unwrap();
            assert_eq!(
                json["requirements"],
                serde_json::json!({
                    "sample_count": 12,
                    "min_empty_sample_count": 10,
                    "max_stale_sample_count": 0,
                    "max_vital_sign_sample_count": 0,
                })
            );
            assert_eq!(
                json["failures"],
                serde_json::json!([{
                    "check": "vital_sign_sample_count",
                    "observed": count,
                    "required": 0,
                    "comparison": "at_most",
                }])
            );
            assert!(diagnostics.failure_summary().contains(&format!(
                "numeric vital-sign estimates in {count}/12 samples (0 allowed)"
            )));
        }
    }

    #[test]
    fn validation_diagnostics_reject_stale_incomplete_and_oversized_windows() {
        for count in [0, 11, 13] {
            let result = evaluate_validation(&empty_validation_samples(count));
            assert!(!result.passed);
            let diagnostics = result.diagnostics();
            assert_eq!(diagnostics.failures[0].check, "sample_count");
            assert_eq!(diagnostics.failures[0].observed, count);
            assert_eq!(diagnostics.failures[0].required, 12);
            assert!(diagnostics
                .failure_summary()
                .contains("check the CSI stream"));
        }
        let mut samples = empty_validation_samples(12);
        samples[0].fresh_tick = false;
        let result = evaluate_validation(&samples);
        assert!(!result.passed);
        assert_eq!(result.sample_count, 12);
        assert_eq!(result.empty_sample_count, 11);
        assert_eq!(
            result.diagnostics().failures,
            vec![BootstrapValidationFailure {
                check: "stale_sample_count",
                observed: 1,
                required: 0,
                comparison: "at_most",
            }]
        );
    }

    #[test]
    fn validation_diagnostics_report_all_simultaneous_failures() {
        let mut samples = empty_validation_samples(11);
        samples[0].fresh_tick = false;
        samples[1].calibrated_empty = false;
        samples[2].vital_signs_absent = false;
        let result = evaluate_validation(&samples);
        assert!(!result.passed);
        let diagnostics = result.diagnostics();
        assert_eq!(
            diagnostics
                .failures
                .iter()
                .map(|failure| failure.check)
                .collect::<Vec<_>>(),
            [
                "sample_count",
                "empty_sample_count",
                "stale_sample_count",
                "vital_sign_sample_count",
            ]
        );
        let summary = diagnostics.failure_summary();
        for expected in [
            "11 samples collected (exactly 12 required)",
            "9 samples reported empty (at least 10 required)",
            "1 stale samples (0 allowed)",
            "numeric vital-sign estimates in 1/12 samples (0 allowed)",
        ] {
            assert!(summary.contains(expected), "missing {expected}: {summary}");
        }
    }

    #[test]
    fn refinement_diagnostics_keep_fresh_count_and_omit_empty_vote_requirement() {
        let diagnostics = validation_diagnostics(11, None, 1, 5);
        let json = serde_json::to_value(&diagnostics).unwrap();
        assert!(json["requirements"].get("min_empty_sample_count").is_none());
        assert_eq!(
            diagnostics
                .failures
                .iter()
                .map(|failure| failure.check)
                .collect::<Vec<_>>(),
            [
                "sample_count",
                "stale_sample_count",
                "vital_sign_sample_count",
            ]
        );
        assert_eq!(diagnostics.failures[0].observed, 11);
        assert!(diagnostics
            .failure_summary()
            .contains("5/12 samples (0 allowed)"));
        let passing = validation_diagnostics(12, None, 0, 0);
        assert!(passing.failures.is_empty());
        assert!(passing.failure_summary().is_empty());
    }

    #[test]
    fn restart_verifies_the_exact_stored_payload_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = path_in(dir.path());
        let now_ms = 1_000_000;
        let model = completed_model(now_ms * 1_000);
        store(&path, "home-a", &[5], TEST_GRID, "model", now_ms, &model).unwrap();

        let original = fs::read_to_string(&path).unwrap();
        let image: BootstrapImageRawV2 = serde_json::from_str(&original).unwrap();
        let rewritten_payload = image.payload.get().replacen(
            "\"min_calibration_duration_s\":0.0",
            "\"min_calibration_duration_s\":0.00",
            1,
        );
        assert_ne!(rewritten_payload, image.payload.get());
        let digest = hex_sha256(rewritten_payload.as_bytes());
        let rewritten =
            format!("{{\"payload\":{rewritten_payload},\"content_sha256\":\"{digest}\"}}");
        fs::write(&path, rewritten).unwrap();

        let (_, loaded) = load(&path, "home-a", now_ms + 1).unwrap();
        assert_eq!(loaded.content_sha256, digest);
    }
}
