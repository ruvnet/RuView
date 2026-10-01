//! Public real-CSI dataset manifests, split manifests, and sample metadata.
//!
//! This module is the bookkeeping layer for training and evaluating on **real
//! public WiFi CSI datasets** (MM-Fi primary, Wi-Pose secondary; see ADR-015).
//! It contains no signal processing and downloads nothing: it records *what*
//! was prepared, *where* it came from, *under which license*, and *how* it was
//! split, so that every reported number can be traced back to its data.
//!
//! # Honesty contract
//!
//! - [`DatasetManifest::validate`] refuses manifests whose provenance fields
//!   are missing. A manifest that has not been checked against real downloaded
//!   files keeps `characteristics_verified == false`, and validation reports
//!   that explicitly.
//! - Full datasets are never committed to Git. The manifest records a
//!   `local_path` outside the repository plus optional SHA-256 checksums.
//! - [`PreparedSplit`] records the split policy, seed, and subject lists so a
//!   result can be reproduced. Prefer [`SplitPolicy::SubjectDisjoint`] whenever
//!   subject IDs exist (see [`MmFiDataset::subject_disjoint_split`]).
//!
//! [`MmFiDataset::subject_disjoint_split`]: crate::dataset::MmFiDataset::subject_disjoint_split
//!
//! # Example
//!
//! ```rust
//! use wifi_densepose_train::public_dataset::DatasetManifest;
//!
//! let mut manifest = DatasetManifest::mmfi_template();
//! assert!(!manifest.characteristics_verified);
//! // `validate` fails until the template is filled in from real files.
//! assert!(manifest.validate().is_err());
//! ```

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::DatasetError;

/// Preprocessing pipeline version recorded in every manifest and sample.
///
/// Bumped whenever the [`crate::real_csi`] stage order, defaults, or
/// subcarrier-conversion method changes.
pub const MANIFEST_PREPROCESSING_VERSION: &str = "real-csi-v1";

/// Train/validation/test split policies.
///
/// Subject-disjoint is the default whenever subject IDs are available:
/// adjacent frames from one recording are highly correlated, so splitting
/// them across train and test makes the result unrealistically easy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SplitPolicy {
    /// Whole subjects go to exactly one partition (default; no subject or
    /// window leakage by construction).
    #[default]
    SubjectDisjoint,
    /// Whole environments/rooms go to exactly one partition.
    EnvironmentDisjoint,
    /// Whole recordings go to exactly one partition.
    RecordingDisjoint,
    /// Random window split. In-domain only; useful for comparison against
    /// published numbers but optimistic — always report alongside a
    /// disjoint split.
    RandomInDomain,
}

impl std::fmt::Display for SplitPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::SubjectDisjoint => "subject-disjoint",
            Self::EnvironmentDisjoint => "environment-disjoint",
            Self::RecordingDisjoint => "recording-disjoint",
            Self::RandomInDomain => "random-in-domain",
        };
        f.write_str(s)
    }
}

/// Provenance + content manifest for one prepared public dataset.
///
/// Built in two stages: a [`DatasetManifest::mmfi_template`] /
/// [`DatasetManifest::wi_pose_template`] carries the *expected* characteristics
/// from the dataset's own documentation (with `characteristics_verified ==
/// false`); the preparation workflow fills in the measured fields from the
/// downloaded files and flips the flag only after checking them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetManifest {
    /// Short dataset key, e.g. `"mmfi"` or `"wi-pose"`.
    pub name: String,
    /// Canonical source URL (paper repo / release page).
    pub source_url: String,
    /// SPDX-style license label, e.g. `"CC-BY-NC-4.0"`.
    pub license: String,
    /// Redistribution consequences of the license (commercial-use flags, …).
    pub license_notes: String,
    /// Required academic citation(s), one string per paper.
    pub citation: Vec<String>,
    /// Local directory holding the prepared files (outside Git).
    /// `None` until preparation has run.
    pub local_path: Option<PathBuf>,
    /// Number of data files discovered.
    pub file_count: usize,
    /// Number of windowed samples produced.
    pub sample_count: usize,
    /// Number of distinct subjects (0 when the dataset has no subject IDs).
    pub subject_count: usize,
    /// Number of distinct activities / action classes.
    pub activity_count: usize,
    /// Number of distinct environments / rooms (0 when unknown).
    pub environment_count: usize,
    /// Native subcarrier count of the raw files (114 for MM-Fi, 30 for
    /// Wi-Pose). Must be measured from the files, not assumed.
    pub original_subcarrier_count: usize,
    /// Antenna layout, e.g. `"1TXx3RX"` (MM-Fi) or `"3TXx3RX"` (Wi-Pose).
    pub antenna_configuration: String,
    /// Capture sampling rate in Hz, when stated by the dataset authors.
    pub sampling_rate_hz: Option<f32>,
    /// Label kinds present, e.g. `["activity", "pose-17-coco"]`.
    pub label_types: Vec<String>,
    /// Preprocessing pipeline version (see [`MANIFEST_PREPROCESSING_VERSION`]).
    pub preprocessing_version: String,
    /// Optional SHA-256 checksums, keyed by path relative to `local_path`.
    pub checksums: BTreeMap<String, String>,
    /// `true` only after every expected characteristic above was checked
    /// against the actual downloaded files and documentation.
    pub characteristics_verified: bool,
    /// Free-form notes (subset selections, manual-download steps taken, …).
    pub notes: String,
}

impl DatasetManifest {
    /// Expected-characteristic template for MM-Fi (NeurIPS 2023).
    ///
    /// Values follow ADR-015 and the MM-Fi release documentation; they are
    /// *expected*, not measured — `characteristics_verified` is `false` until
    /// the preparation workflow checks them against real files.
    pub fn mmfi_template() -> Self {
        Self {
            name: "mmfi".to_string(),
            source_url: "https://github.com/ybhbingo/MMFi_dataset".to_string(),
            license: "CC-BY-NC-4.0".to_string(),
            license_notes: "Non-commercial: weights trained on MM-Fi must not \
                be deployed commercially. Custom data collection is required \
                before any commercial release."
                .to_string(),
            citation: vec!["Yang et al., \"MM-Fi: Multi-Modal Non-Intrusive 4D Human \
                 Dataset for Versatile Wireless Sensing\", NeurIPS 2023 \
                 Datasets & Benchmarks (arXiv:2305.10345)."
                .to_string()],
            local_path: None,
            file_count: 0,
            sample_count: 0,
            subject_count: 40,
            activity_count: 27,
            environment_count: 4,
            original_subcarrier_count: 114,
            antenna_configuration: "1TXx3RX".to_string(),
            sampling_rate_hz: Some(100.0),
            label_types: vec![
                "activity".to_string(),
                "pose-17-coco-3d".to_string(),
                "densepose-uv".to_string(),
            ],
            preprocessing_version: MANIFEST_PREPROCESSING_VERSION.to_string(),
            checksums: BTreeMap::new(),
            characteristics_verified: false,
            notes: String::new(),
        }
    }

    /// Expected-characteristic template for the Wi-Pose dataset (NjtechCVLab).
    ///
    /// Same verified/unverified contract as [`Self::mmfi_template`].
    pub fn wi_pose_template() -> Self {
        Self {
            name: "wi-pose".to_string(),
            source_url: "https://github.com/NjtechCVLab/Wi-PoseDataset".to_string(),
            license: "research-use".to_string(),
            license_notes: "Research use per the dataset release; commercial \
                redistribution is not permitted. Confirm the current license \
                text in the Wi-Pose repository before any use beyond research."
                .to_string(),
            citation: vec!["NjtechCVLab, \"Wi-Pose Dataset\" \
                 (https://github.com/NjtechCVLab/Wi-PoseDataset); see also \
                 CSI-Former (MDPI Entropy 2023) and related works."
                .to_string()],
            local_path: None,
            file_count: 0,
            sample_count: 0,
            subject_count: 12,
            activity_count: 12,
            environment_count: 0,
            original_subcarrier_count: 30,
            antenna_configuration: "3TXx3RX".to_string(),
            sampling_rate_hz: None,
            label_types: vec!["activity".to_string(), "pose-18-alphapose".to_string()],
            preprocessing_version: MANIFEST_PREPROCESSING_VERSION.to_string(),
            checksums: BTreeMap::new(),
            characteristics_verified: false,
            notes: String::new(),
        }
    }

    /// Check that this manifest describes real, prepared data.
    ///
    /// Fails when provenance is incomplete (empty source/license/citation),
    /// when no `local_path` was recorded, or when no files/samples were
    /// counted. Unverified expected characteristics are reported in the
    /// error so they cannot be mistaken for measurements.
    pub fn validate(&self) -> Result<(), DatasetError> {
        if self.name.trim().is_empty() {
            return Err(DatasetError::Format(
                "manifest has an empty dataset name".into(),
            ));
        }
        if self.source_url.trim().is_empty() {
            return Err(DatasetError::Format(format!(
                "manifest `{}` has an empty source_url",
                self.name
            )));
        }
        if self.license.trim().is_empty() {
            return Err(DatasetError::Format(format!(
                "manifest `{}` has an empty license",
                self.name
            )));
        }
        if self.citation.is_empty() {
            return Err(DatasetError::Format(format!(
                "manifest `{}` records no citation",
                self.name
            )));
        }
        let local = self.local_path.as_ref().ok_or_else(|| {
            DatasetError::Format(format!(
                "manifest `{}` has no local_path: preparation has not run",
                self.name
            ))
        })?;
        if self.file_count == 0 {
            return Err(DatasetError::Format(format!(
                "manifest `{}` counts zero files under `{}`",
                self.name,
                local.display()
            )));
        }
        if self.sample_count == 0 {
            return Err(DatasetError::Format(format!(
                "manifest `{}` counts zero samples",
                self.name
            )));
        }
        if self.original_subcarrier_count == 0 {
            return Err(DatasetError::Format(format!(
                "manifest `{}` has no measured subcarrier count",
                self.name
            )));
        }
        if !self.characteristics_verified {
            return Err(DatasetError::Format(format!(
                "manifest `{}` is unverified: expected characteristics were \
                 not checked against the downloaded files",
                self.name
            )));
        }
        Ok(())
    }

    /// Serialize the manifest as pretty JSON.
    pub fn to_json(&self) -> Result<String, DatasetError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| DatasetError::Format(format!("manifest serialization failed: {e}")))
    }

    /// Write the manifest to `<local_path>/dataset_manifest.json`.
    ///
    /// Returns the path written.
    pub fn save(&self) -> Result<PathBuf, DatasetError> {
        let local = self.local_path.clone().ok_or_else(|| {
            DatasetError::Format(format!(
                "manifest `{}` has no local_path; refusing to save",
                self.name
            ))
        })?;
        let path = local.join("dataset_manifest.json");
        std::fs::write(&path, self.to_json()?).map_err(DatasetError::Io)?;
        Ok(path)
    }

    /// Load a manifest written by [`Self::save`].
    pub fn load(path: &Path) -> Result<Self, DatasetError> {
        let bytes = std::fs::read(path).map_err(DatasetError::Io)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| DatasetError::Format(format!("manifest parse failed: {e}")))
    }

    /// Record a SHA-256 checksum for a file relative to `local_path`.
    pub fn record_checksum(&mut self, rel_path: String, sha256_hex: String) {
        self.checksums.insert(rel_path, sha256_hex);
    }
}

/// A reproducible train/test split record.
///
/// Complements (but does not replace) the in-memory
/// [`MmFiDataset::subject_disjoint_split`] / [`assert_split_leak_free`]
/// guards: this is the on-disk artifact that ties a reported metric back to
/// the exact subjects and windows it was computed on.
///
/// [`MmFiDataset::subject_disjoint_split`]: crate::dataset::MmFiDataset::subject_disjoint_split
/// [`assert_split_leak_free`]: crate::dataset::assert_split_leak_free
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedSplit {
    /// Dataset key this split belongs to.
    pub dataset: String,
    /// Split policy used.
    pub policy: SplitPolicy,
    /// Deterministic seed used for subject assignment.
    pub seed: u64,
    /// Subjects assigned to train (sorted).
    pub train_subjects: Vec<u32>,
    /// Subjects assigned to test (sorted).
    pub test_subjects: Vec<u32>,
    /// Number of train windows.
    pub train_windows: usize,
    /// Number of test windows.
    pub test_windows: usize,
}

impl PreparedSplit {
    /// Verify the split is non-degenerate and leak-free.
    ///
    /// Checks: both sides non-empty, subject lists sorted, and no subject on
    /// both sides. (Window-level disjointness is enforced by construction in
    /// the loader; see [`assert_split_leak_free`].)
    ///
    /// [`assert_split_leak_free`]: crate::dataset::assert_split_leak_free
    pub fn verify(&self) -> Result<(), DatasetError> {
        if self.train_subjects.is_empty() || self.test_subjects.is_empty() {
            return Err(DatasetError::InvalidSplit(format!(
                "split for `{}` has an empty partition",
                self.dataset
            )));
        }
        if self.train_windows == 0 || self.test_windows == 0 {
            return Err(DatasetError::InvalidSplit(format!(
                "split for `{}` counts zero windows on a partition",
                self.dataset
            )));
        }
        let mut train = self.train_subjects.clone();
        let mut test = self.test_subjects.clone();
        train.sort_unstable();
        test.sort_unstable();
        if train != self.train_subjects || test != self.test_subjects {
            return Err(DatasetError::InvalidSplit(format!(
                "split for `{}`: subject lists must be sorted",
                self.dataset
            )));
        }
        if let Some(shared) = train.iter().find(|s| test.contains(s)) {
            return Err(DatasetError::InvalidSplit(format!(
                "subject {shared} appears in both train and test (subject leakage)"
            )));
        }
        Ok(())
    }

    /// Serialize the split as pretty JSON.
    pub fn to_json(&self) -> Result<String, DatasetError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| DatasetError::Format(format!("split serialization failed: {e}")))
    }

    /// Write the split next to the dataset manifest.
    ///
    /// Returns the path written.
    pub fn save(&self, dir: &Path, file_name: &str) -> Result<PathBuf, DatasetError> {
        let path = dir.join(file_name);
        std::fs::write(&path, self.to_json()?).map_err(DatasetError::Io)?;
        Ok(path)
    }

    /// Load a split written by [`Self::save`].
    pub fn load(path: &Path) -> Result<Self, DatasetError> {
        let bytes = std::fs::read(path).map_err(DatasetError::Io)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| DatasetError::Format(format!("split parse failed: {e}")))
    }
}

/// Per-sample provenance carried through the real-CSI pipeline.
///
/// Every processed window retains where it came from and what was done to it,
/// so converted data is never mistaken for native captures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedSampleMeta {
    /// Dataset key, e.g. `"mmfi"`.
    pub source_dataset: String,
    /// Subject identifier.
    pub subject_id: u32,
    /// Environment / room identifier, when the layout provides one.
    pub environment_id: Option<u32>,
    /// Recording / action identifier string (e.g. `"S01/A01"`).
    pub recording_id: String,
    /// Activity / action class identifier.
    pub activity_id: u32,
    /// Absolute frame index within the original recording.
    pub frame_index: u64,
    /// Native subcarrier count of the raw file.
    pub original_subcarrier_count: usize,
    /// Subcarrier count after conversion.
    pub processed_subcarrier_count: usize,
    /// Antenna layout string, e.g. `"1TXx3RX"`.
    pub antenna_configuration: String,
    /// Capture sampling rate in Hz, when known.
    pub sampling_rate_hz: Option<f32>,
    /// Preprocessing pipeline version (see [`MANIFEST_PREPROCESSING_VERSION`]).
    pub preprocessing_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared_mmfi() -> DatasetManifest {
        let mut m = DatasetManifest::mmfi_template();
        m.local_path = Some(PathBuf::from("/data/public/mmfi"));
        m.file_count = 12;
        m.sample_count = 96;
        m.characteristics_verified = true;
        m
    }

    #[test]
    fn templates_are_unverified_until_checked() {
        assert!(!DatasetManifest::mmfi_template().characteristics_verified);
        assert!(!DatasetManifest::wi_pose_template().characteristics_verified);
        // Templates carry no local data, so validation must fail loudly
        // rather than silently describe nothing.
        assert!(DatasetManifest::mmfi_template().validate().is_err());
        assert!(DatasetManifest::wi_pose_template().validate().is_err());
    }

    #[test]
    fn prepared_manifest_validates() {
        prepared_mmfi()
            .validate()
            .expect("filled manifest must validate");
    }

    #[test]
    fn validation_rejects_missing_provenance() {
        let mut m = prepared_mmfi();
        m.license.clear();
        assert!(m.validate().is_err(), "empty license must fail");

        let mut m = prepared_mmfi();
        m.citation.clear();
        assert!(m.validate().is_err(), "missing citation must fail");

        let mut m = prepared_mmfi();
        m.characteristics_verified = false;
        let err = m.validate().expect_err("unverified must fail");
        assert!(err.to_string().contains("unverified"), "got: {err}");
    }

    #[test]
    fn manifest_json_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut m = prepared_mmfi();
        m.local_path = Some(dir.path().to_path_buf());
        m.record_checksum("S01/A01/wifi_csi.npy".to_string(), "ab".repeat(32));
        let path = m.save().expect("save");
        let loaded = DatasetManifest::load(&path).expect("load");
        loaded.validate().expect("reloaded manifest must validate");
        assert_eq!(loaded.checksums.len(), 1);
        assert_eq!(loaded.preprocessing_version, MANIFEST_PREPROCESSING_VERSION);
    }

    #[test]
    fn split_verify_accepts_clean_subject_disjoint_split() {
        let split = PreparedSplit {
            dataset: "mmfi".to_string(),
            policy: SplitPolicy::SubjectDisjoint,
            seed: 42,
            train_subjects: vec![1, 2, 3],
            test_subjects: vec![4, 5],
            train_windows: 300,
            test_windows: 200,
        };
        split.verify().expect("clean split must verify");
        let json = split.to_json().expect("serialize");
        let back: PreparedSplit = serde_json::from_str(&json).expect("deserialize");
        back.verify().expect("round-tripped split must verify");
    }

    #[test]
    fn split_verify_rejects_shared_subject() {
        let split = PreparedSplit {
            dataset: "mmfi".to_string(),
            policy: SplitPolicy::SubjectDisjoint,
            seed: 42,
            train_subjects: vec![1, 2],
            test_subjects: vec![2, 3],
            train_windows: 100,
            test_windows: 100,
        };
        let err = split.verify().expect_err("shared subject must fail");
        assert!(err.to_string().contains('2'), "got: {err}");
    }

    #[test]
    fn split_verify_rejects_empty_partition() {
        let split = PreparedSplit {
            dataset: "mmfi".to_string(),
            policy: SplitPolicy::SubjectDisjoint,
            seed: 42,
            train_subjects: vec![],
            test_subjects: vec![1],
            train_windows: 0,
            test_windows: 10,
        };
        assert!(split.verify().is_err());
    }
}
