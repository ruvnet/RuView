//! CPU inference model profiles and model metadata.
//!
//! Profiles name a `(architecture-size, precision, runtime)` deployment
//! target. The first two are implemented; the remaining three reserve the
//! API surface (names, precision mapping, metadata) so they can be added
//! without breaking callers. Requesting an unimplemented profile fails with
//! [`NnError::Unsupported`], never with a silent fallback.
//!
//! [`NnError::Unsupported`]: crate::NnError::Unsupported
//!
//! # Honesty contract
//!
//! [`ModelMetadata`] separates three facts that must never be conflated:
//!
//! - what data trained the weights ([`TrainingData`]),
//! - what data validated them ([`RealWorldValidation`]),
//! - whether RuView's own hardware validated them
//!   ([`LocalHardwareValidation`]).
//!
//! A model trained on MM-Fi and evaluated on MM-Fi stays
//! `real_world_validation = PublicDatasetOnly` and
//! `local_hardware_validation = NotAvailable` until real ESP32 captures say
//! otherwise.
//!
//! # Example
//!
//! ```rust
//! use wifi_densepose_nn::profile::{CpuProfile, Precision};
//!
//! assert!(CpuProfile::CpuMicroFp32.is_supported());
//! assert!(!CpuProfile::CpuMicroInt4Qat.is_supported());
//! assert_eq!(CpuProfile::CpuMicroInt8.precision(), Precision::Int8);
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{NnError, NnResult};

/// Numerical precision of a deployed model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Precision {
    /// 32-bit floating point (numerical reference).
    #[default]
    Fp32,
    /// 8-bit integer (first CPU deployment target).
    Int8,
    /// 4-bit integer. Reserved for future quantization-aware-training work:
    /// no supported profile or runtime path uses it yet (see crate docs).
    Int4,
}

impl std::fmt::Display for Precision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Fp32 => "fp32",
            Self::Int8 => "int8",
            Self::Int4 => "int4",
        };
        f.write_str(s)
    }
}

/// CPU deployment profiles.
///
/// Profiles whose runtime path is implemented return `true` from
/// [`Self::is_supported`]. The rest exist so training scripts, benchmarks,
/// and metadata can name them; [`Self::require_supported`] rejects them with
/// [`NnError::Unsupported`].
///
/// [`NnError::Unsupported`]: crate::NnError::Unsupported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuProfile {
    /// Reference FP32 profile (implemented).
    CpuMicroFp32,
    /// First INT8 CPU deployment target (implemented).
    CpuMicroInt8,
    /// Smaller INT8 variant (reserved: not implemented).
    CpuNanoInt8,
    /// Larger INT8 variant (reserved: not implemented).
    CpuTinyInt8,
    /// INT4 quantization-aware-training variant (reserved: not implemented;
    /// naive post-training INT4 and QAT INT4 must be measured separately
    /// before either ships).
    CpuMicroInt4Qat,
}

impl CpuProfile {
    /// Canonical profile name used in configs, benchmarks, and reports.
    pub fn name(&self) -> &'static str {
        match self {
            Self::CpuMicroFp32 => "cpu-micro-fp32",
            Self::CpuMicroInt8 => "cpu-micro-int8",
            Self::CpuNanoInt8 => "cpu-nano-int8",
            Self::CpuTinyInt8 => "cpu-tiny-int8",
            Self::CpuMicroInt4Qat => "cpu-micro-int4-qat",
        }
    }

    /// Precision this profile executes at.
    pub fn precision(&self) -> Precision {
        match self {
            Self::CpuMicroFp32 => Precision::Fp32,
            Self::CpuMicroInt8 | Self::CpuNanoInt8 | Self::CpuTinyInt8 => Precision::Int8,
            Self::CpuMicroInt4Qat => Precision::Int4,
        }
    }

    /// Whether the runtime path for this profile is implemented.
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::CpuMicroFp32 | Self::CpuMicroInt8)
    }

    /// Fail unless the profile's runtime path is implemented.
    ///
    /// # Errors
    ///
    /// [`NnError::Unsupported`] naming the profile and the reason.
    pub fn require_supported(&self) -> NnResult<()> {
        if self.is_supported() {
            Ok(())
        } else {
            Err(NnError::Unsupported(format!(
                "profile `{}` is reserved but not implemented; implemented profiles are \
                 `cpu-micro-fp32` and `cpu-micro-int8`",
                self.name()
            )))
        }
    }

    /// Parse a canonical profile name (see [`Self::name`]).
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "cpu-micro-fp32" => Some(Self::CpuMicroFp32),
            "cpu-micro-int8" => Some(Self::CpuMicroInt8),
            "cpu-nano-int8" => Some(Self::CpuNanoInt8),
            "cpu-tiny-int8" => Some(Self::CpuTinyInt8),
            "cpu-micro-int4-qat" => Some(Self::CpuMicroInt4Qat),
            _ => None,
        }
    }

    /// All known profiles, implemented and reserved.
    pub fn all() -> [Self; 5] {
        [
            Self::CpuMicroFp32,
            Self::CpuMicroInt8,
            Self::CpuNanoInt8,
            Self::CpuTinyInt8,
            Self::CpuMicroInt4Qat,
        ]
    }
}

impl std::fmt::Display for CpuProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// What data trained the weights. Public-dataset weights carry the dataset
/// keys so license obligations (e.g. MM-Fi non-commercial) stay attached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingData {
    /// Trained on real public CSI (dataset keys, e.g. `["mmfi"]`).
    RealPublicCsi {
        /// Dataset keys from the dataset manifests.
        datasets: Vec<String>,
    },
    /// Trained on synthetic data only (software smoke tests, never a
    /// capability claim).
    SyntheticOnly,
    /// Mixed real and synthetic training data.
    Mixed {
        /// Real dataset keys involved.
        datasets: Vec<String>,
    },
    /// Randomly initialized / architecture only — not trained.
    Untrained,
}

/// What data the reported accuracy numbers were measured on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealWorldValidation {
    /// Evaluated on public datasets only. Says nothing about RuView ESP32
    /// hardware or any new room.
    PublicDatasetOnly {
        /// Dataset keys evaluated on.
        datasets: Vec<String>,
    },
    /// Evaluated on the deployment hardware stack.
    LocalHardware,
    /// No evaluation run yet.
    None,
}

/// Whether RuView's own ESP32 hardware validated this model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalHardwareValidation {
    /// No local-hardware validation exists. The default for every
    /// public-dataset model.
    NotAvailable,
    /// Validated on RuView hardware (requires a captured evidence log).
    Validated,
}

/// Accuracy status of the model artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccuracyStatus {
    /// No accuracy numbers exist.
    Untrained,
    /// Software smoke test on synthetic data only.
    SyntheticSmokeOnly,
    /// Measured on public datasets (metric + value recorded alongside,
    /// not here — see the evaluation report).
    PublicDatasetEvaluated,
    /// Measured on RuView hardware.
    HardwareEvaluated,
}

/// Deployment metadata for one model artifact.
///
/// Built from the `cpu_micro_fp32` / `cpu_micro_int8` templates (which pin
/// the honesty defaults) and checked by [`Self::validate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Deployment profile.
    pub profile: CpuProfile,
    /// Model architecture identifier, e.g. `"wiflow-std-half"`.
    pub architecture: String,
    /// Model input shape, e.g. `[1, 3, 3, 56]`.
    pub input_shape: Vec<usize>,
    /// Model output shape.
    pub output_shape: Vec<usize>,
    /// Parameter count.
    pub parameter_count: u64,
    /// Execution precision (must agree with the profile).
    pub precision: Precision,
    /// Quantization method, e.g. `"torch-fx-qat"` / `"onnx-static-qdq"`.
    /// `None` for unquantized FP32.
    pub quantization_method: Option<String>,
    /// Model file size in bytes (measured, not estimated).
    pub model_size_bytes: u64,
    /// Supported device strings, e.g. `["cpu-x86_64", "cpu-aarch64"]`.
    pub supported_devices: Vec<String>,
    /// Training data provenance.
    pub training_data: TrainingData,
    /// Validation datasets / split policy description.
    pub validation_dataset: String,
    /// Preprocessing pipeline version (see `real-csi-v1`).
    pub preprocessing_version: String,
    /// Accuracy status.
    pub accuracy_status: AccuracyStatus,
    /// What the accuracy numbers were measured on.
    pub real_world_validation: RealWorldValidation,
    /// RuView ESP32 hardware validation state.
    pub local_hardware_validation: LocalHardwareValidation,
    /// License / redistribution notes (dataset licenses attach here).
    pub license_notes: String,
}

impl ModelMetadata {
    /// Template for the FP32 reference profile with the honesty defaults:
    /// public-dataset-only validation and no local-hardware validation.
    pub fn cpu_micro_fp32(
        architecture: impl Into<String>,
        input_shape: Vec<usize>,
        output_shape: Vec<usize>,
        parameter_count: u64,
        model_size_bytes: u64,
    ) -> Self {
        Self::cpu_template(
            CpuProfile::CpuMicroFp32,
            architecture,
            input_shape,
            output_shape,
            parameter_count,
            None,
            model_size_bytes,
        )
    }

    /// Template for the INT8 CPU deployment profile with the honesty
    /// defaults. `quantization_method` names how the INT8 artifact was
    /// produced (e.g. `"torch-fx-qat"`, `"onnx-static-qdq"`).
    pub fn cpu_micro_int8(
        architecture: impl Into<String>,
        input_shape: Vec<usize>,
        output_shape: Vec<usize>,
        parameter_count: u64,
        quantization_method: impl Into<String>,
        model_size_bytes: u64,
    ) -> Self {
        Self::cpu_template(
            CpuProfile::CpuMicroInt8,
            architecture,
            input_shape,
            output_shape,
            parameter_count,
            Some(quantization_method.into()),
            model_size_bytes,
        )
    }

    fn cpu_template(
        profile: CpuProfile,
        architecture: impl Into<String>,
        input_shape: Vec<usize>,
        output_shape: Vec<usize>,
        parameter_count: u64,
        quantization_method: Option<String>,
        model_size_bytes: u64,
    ) -> Self {
        Self {
            precision: profile.precision(),
            profile,
            architecture: architecture.into(),
            input_shape,
            output_shape,
            parameter_count,
            quantization_method,
            model_size_bytes,
            supported_devices: vec!["cpu-x86_64".to_string(), "cpu-aarch64".to_string()],
            training_data: TrainingData::Untrained,
            validation_dataset: String::new(),
            preprocessing_version: "real-csi-v1".to_string(),
            accuracy_status: AccuracyStatus::Untrained,
            real_world_validation: RealWorldValidation::None,
            local_hardware_validation: LocalHardwareValidation::NotAvailable,
            license_notes: String::new(),
        }
    }

    /// Check metadata consistency.
    ///
    /// Fails on empty shapes/devices, precision/profile disagreement, an
    /// INT8 profile without a named quantization method, accuracy claims
    /// without a validation basis, hardware claims without hardware
    /// validation, and missing license notes.
    pub fn validate(&self) -> NnResult<()> {
        self.profile.require_supported()?;
        if self.precision != self.profile.precision() {
            return Err(NnError::config(format!(
                "profile `{}` executes at {} but metadata claims {}",
                self.profile,
                self.profile.precision(),
                self.precision
            )));
        }
        if self.precision != Precision::Fp32 && self.quantization_method.is_none() {
            return Err(NnError::config(format!(
                "profile `{}` is quantized but names no quantization_method",
                self.profile
            )));
        }
        if self.input_shape.is_empty()
            || self.output_shape.is_empty()
            || self.input_shape.contains(&0)
            || self.output_shape.contains(&0)
        {
            return Err(NnError::config(format!(
                "profile `{}` has an empty or degenerate shape (in={:?}, out={:?})",
                self.profile, self.input_shape, self.output_shape
            )));
        }
        if self.supported_devices.is_empty() {
            return Err(NnError::config(format!(
                "profile `{}` lists no supported devices",
                self.profile
            )));
        }
        match (&self.accuracy_status, &self.real_world_validation) {
            (AccuracyStatus::PublicDatasetEvaluated, RealWorldValidation::None) => {
                return Err(NnError::config(
                    "accuracy claims public-dataset evaluation but no validation dataset is recorded",
                ));
            }
            (AccuracyStatus::HardwareEvaluated, _) => {
                if self.local_hardware_validation != LocalHardwareValidation::Validated {
                    return Err(NnError::config(
                        "accuracy claims hardware evaluation without local hardware validation",
                    ));
                }
                if self.real_world_validation != RealWorldValidation::LocalHardware {
                    return Err(NnError::config(
                        "accuracy claims hardware evaluation but real_world_validation is not local hardware",
                    ));
                }
            }
            _ => {}
        }
        if self.license_notes.trim().is_empty() {
            return Err(NnError::config(format!(
                "profile `{}` has no license notes",
                self.profile
            )));
        }
        Ok(())
    }

    /// Serialize the metadata as pretty JSON.
    pub fn to_json(&self) -> NnResult<String> {
        serde_json::to_string_pretty(self).map_err(NnError::Serialization)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn licensed(meta: &mut ModelMetadata) {
        meta.license_notes = "test fixture".to_string();
    }

    #[test]
    fn profile_names_round_trip() {
        for profile in CpuProfile::all() {
            assert_eq!(CpuProfile::from_name(profile.name()), Some(profile));
        }
        assert_eq!(CpuProfile::from_name("cpu-micro-fp16"), None);
    }

    #[test]
    fn reserved_profiles_are_rejected_loudly() {
        assert!(CpuProfile::CpuMicroFp32.require_supported().is_ok());
        assert!(CpuProfile::CpuMicroInt8.require_supported().is_ok());
        for reserved in [
            CpuProfile::CpuNanoInt8,
            CpuProfile::CpuTinyInt8,
            CpuProfile::CpuMicroInt4Qat,
        ] {
            assert!(!reserved.is_supported());
            let err = reserved.require_supported().expect_err("must fail");
            assert!(err.to_string().contains(reserved.name()), "got: {err}");
        }
    }

    #[test]
    fn profile_precision_mapping() {
        assert_eq!(CpuProfile::CpuMicroFp32.precision(), Precision::Fp32);
        assert_eq!(CpuProfile::CpuMicroInt8.precision(), Precision::Int8);
        assert_eq!(CpuProfile::CpuNanoInt8.precision(), Precision::Int8);
        assert_eq!(CpuProfile::CpuTinyInt8.precision(), Precision::Int8);
        assert_eq!(CpuProfile::CpuMicroInt4Qat.precision(), Precision::Int4);
    }

    #[test]
    fn templates_carry_honesty_defaults() {
        let fp32 =
            ModelMetadata::cpu_micro_fp32("test-arch", vec![1, 3, 3, 56], vec![1, 17, 2], 10, 40);
        assert_eq!(fp32.precision, Precision::Fp32);
        assert_eq!(fp32.real_world_validation, RealWorldValidation::None);
        assert_eq!(
            fp32.local_hardware_validation,
            LocalHardwareValidation::NotAvailable
        );
        let int8 = ModelMetadata::cpu_micro_int8(
            "test-arch",
            vec![1, 3, 3, 56],
            vec![1, 17, 2],
            10,
            "torch-fx-qat",
            12,
        );
        assert_eq!(int8.precision, Precision::Int8);
        assert_eq!(int8.quantization_method.as_deref(), Some("torch-fx-qat"));
    }

    #[test]
    fn validate_accepts_a_complete_public_dataset_model() {
        let mut meta = ModelMetadata::cpu_micro_int8(
            "wiflow-std-half",
            vec![1, 3, 3, 56],
            vec![1, 17, 2],
            843_834,
            "torch-fx-qat",
            1_043_000,
        );
        meta.training_data = TrainingData::RealPublicCsi {
            datasets: vec!["mmfi".to_string()],
        };
        meta.validation_dataset = "mmfi subject-disjoint seed 42".to_string();
        meta.accuracy_status = AccuracyStatus::PublicDatasetEvaluated;
        meta.real_world_validation = RealWorldValidation::PublicDatasetOnly {
            datasets: vec!["mmfi".to_string()],
        };
        meta.license_notes = "MM-Fi CC-BY-NC-4.0: no commercial deployment.".to_string();
        meta.validate().expect("complete metadata must validate");
        let json = meta.to_json().expect("serialize");
        let back: ModelMetadata = serde_json::from_str(&json).expect("deserialize");
        back.validate()
            .expect("round-tripped metadata must validate");
    }

    #[test]
    fn validate_rejects_precision_profile_mismatch() {
        let mut meta = ModelMetadata::cpu_micro_fp32("a", vec![1], vec![1], 1, 4);
        licensed(&mut meta);
        meta.precision = Precision::Int8;
        let err = meta.validate().expect_err("mismatch must fail");
        assert!(err.to_string().contains("cpu-micro-fp32"), "got: {err}");
    }

    #[test]
    fn validate_rejects_anonymous_quantization() {
        let mut meta = ModelMetadata::cpu_micro_fp32("a", vec![1], vec![1], 1, 4);
        licensed(&mut meta);
        // FP32 template with the INT8 profile but no method named.
        meta.profile = CpuProfile::CpuMicroInt8;
        meta.precision = Precision::Int8;
        let err = meta.validate().expect_err("missing method must fail");
        assert!(
            err.to_string().contains("quantization_method"),
            "got: {err}"
        );
    }

    #[test]
    fn validate_rejects_hardware_claims_without_hardware() {
        let mut meta = ModelMetadata::cpu_micro_fp32("a", vec![1], vec![1], 1, 4);
        licensed(&mut meta);
        meta.accuracy_status = AccuracyStatus::HardwareEvaluated;
        meta.real_world_validation = RealWorldValidation::LocalHardware;
        let err = meta.validate().expect_err("must fail");
        assert!(err.to_string().contains("hardware"), "got: {err}");
    }

    #[test]
    fn validate_rejects_missing_license_notes() {
        let meta = ModelMetadata::cpu_micro_fp32("a", vec![1], vec![1], 1, 4);
        let err = meta.validate().expect_err("must fail");
        assert!(err.to_string().contains("license"), "got: {err}");
    }
}
