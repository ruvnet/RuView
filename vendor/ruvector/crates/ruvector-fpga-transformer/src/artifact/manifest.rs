//! Manifest schema for model artifacts

use crate::error::{Error, Result};
use crate::types::{FixedShape, Layout, QuantSpec};
use serde::{Deserialize, Serialize};

/// Model manifest containing all metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Model name
    pub name: String,
    /// SHA-256 hash of model (hex string)
    pub model_hash: String,
    /// Fixed shape specification
    pub shape: FixedShape,
    /// Quantization specification
    pub quant: QuantSpec,
    /// I/O configuration
    pub io: IoSpec,
    /// Backend configuration
    pub backend: BackendSpec,
    /// Test vector specification
    pub tests: TestSpec,
}

impl Manifest {
    /// Create a new manifest
    pub fn new(name: impl Into<String>, shape: FixedShape, quant: QuantSpec) -> Self {
        Self {
            name: name.into(),
            model_hash: String::new(),
            shape,
            quant,
            io: IoSpec::default(),
            backend: BackendSpec::default(),
            tests: TestSpec::default(),
        }
    }

    /// Validate manifest consistency
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(Error::InvalidArtifact("Model name is empty".into()));
        }

        // Validate shape
        self.shape
            .validate()
            .map_err(|e| Error::InvalidArtifact(e))?;

        // Validate quantization bits
        if !matches!(self.quant.w_bits, 1 | 2 | 4 | 8 | 16) {
            return Err(Error::InvalidArtifact(format!(
                "Invalid weight bits: {}",
                self.quant.w_bits
            )));
        }
        if !matches!(self.quant.a_bits, 4 | 8 | 16 | 32) {
            return Err(Error::InvalidArtifact(format!(
                "Invalid activation bits: {}",
                self.quant.a_bits
            )));
        }

        Ok(())
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(Into::into)
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(Into::into)
    }
}

/// I/O type specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoSpec {
    /// Input token type (typically "u16")
    pub tokens: String,
    /// Output logit type (typically "i16" or "i32")
    pub logits: String,
    /// Top-K count (0 for full logits)
    pub topk: u16,
}

impl Default for IoSpec {
    fn default() -> Self {
        Self {
            tokens: "u16".into(),
            logits: "i16".into(),
            topk: 16,
        }
    }
}

/// Backend-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendSpec {
    /// Backend kind ("fpga_pcie", "fpga_daemon", "native_sim", "wasm_sim")
    pub kind: String,
    /// Protocol version
    pub protocol: u16,
    /// Backend-specific options
    #[serde(default)]
    pub options: BackendOptions,
}

impl Default for BackendSpec {
    fn default() -> Self {
        Self {
            kind: "native_sim".into(),
            protocol: 1,
            options: BackendOptions::default(),
        }
    }
}

/// Backend-specific options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendOptions {
    /// Enable batch processing
    #[serde(default)]
    pub batch_enabled: bool,
    /// Maximum batch size
    #[serde(default)]
    pub max_batch: u16,
    /// Enable early exit
    #[serde(default)]
    pub early_exit: bool,
    /// Minimum coherence threshold for early exit
    #[serde(default)]
    pub early_exit_threshold: i16,
    /// FPGA clock frequency in MHz (for cycle estimation)
    #[serde(default)]
    pub clock_mhz: u16,
}

/// Test vector specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSpec {
    /// Number of test vectors
    pub vectors: u32,
    /// Maximum absolute error allowed
    pub max_abs_err: i32,
    /// Whether test vectors must pass before activation
    #[serde(default = "default_true")]
    pub require_pass: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TestSpec {
    fn default() -> Self {
        Self {
            vectors: 0,
            max_abs_err: 2,
            require_pass: true,
        }
    }
}

/// Manifest builder for convenient construction
pub struct ManifestBuilder {
    manifest: Manifest,
}

impl ManifestBuilder {
    /// Create a new builder with name and shape
    pub fn new(name: impl Into<String>, shape: FixedShape) -> Self {
        Self {
            manifest: Manifest::new(name, shape, QuantSpec::int8()),
        }
    }

    /// Set quantization spec
    pub fn quant(mut self, quant: QuantSpec) -> Self {
        self.manifest.quant = quant;
        self
    }

    /// Set model hash
    pub fn model_hash(mut self, hash: impl Into<String>) -> Self {
        self.manifest.model_hash = hash.into();
        self
    }

    /// Set I/O spec
    pub fn io(mut self, io: IoSpec) -> Self {
        self.manifest.io = io;
        self
    }

    /// Set backend spec
    pub fn backend(mut self, backend: BackendSpec) -> Self {
        self.manifest.backend = backend;
        self
    }

    /// Set test spec
    pub fn tests(mut self, tests: TestSpec) -> Self {
        self.manifest.tests = tests;
        self
    }

    /// Enable top-K only output
    pub fn topk_only(mut self, k: u16) -> Self {
        self.manifest.io.topk = k;
        self
    }

    /// Enable early exit
    pub fn early_exit(mut self, threshold: i16) -> Self {
        self.manifest.backend.options.early_exit = true;
        self.manifest.backend.options.early_exit_threshold = threshold;
        self
    }

    /// Build the manifest
    pub fn build(self) -> Result<Manifest> {
        self.manifest.validate()?;
        Ok(self.manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_builder() {
        let manifest = ManifestBuilder::new("test", FixedShape::micro())
            .quant(QuantSpec::int4_int8())
            .topk_only(16)
            .early_exit(100)
            .build()
            .unwrap();

        assert_eq!(manifest.name, "test");
        assert_eq!(manifest.quant.w_bits, 4);
        assert_eq!(manifest.io.topk, 16);
        assert!(manifest.backend.options.early_exit);
    }

    #[test]
    fn test_manifest_json_roundtrip() {
        let manifest = Manifest::new("test", FixedShape::micro(), QuantSpec::int8());
        let json = manifest.to_json().unwrap();
        let parsed = Manifest::from_json(&json).unwrap();
        assert_eq!(manifest.name, parsed.name);
    }

    #[test]
    fn test_manifest_validation() {
        let mut manifest = Manifest::new("test", FixedShape::micro(), QuantSpec::int8());
        assert!(manifest.validate().is_ok());

        manifest.name = String::new();
        assert!(manifest.validate().is_err());
    }
}
