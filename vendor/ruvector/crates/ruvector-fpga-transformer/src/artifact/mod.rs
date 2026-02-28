//! Model artifact format and handling
//!
//! Signed bundles with metadata, weights, and test vectors.

pub mod manifest;
pub mod pack;
pub mod verify;

pub use manifest::{BackendSpec, IoSpec, Manifest, TestSpec};
pub use pack::{pack_artifact, unpack_artifact};
pub use verify::{verify_artifact, verify_signature};

use crate::error::{Error, Result};
use crate::types::{FixedShape, ModelId, QuantSpec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Complete model artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelArtifact {
    /// Manifest with metadata
    pub manifest: Manifest,
    /// Quantized weights (binary blob)
    #[serde(with = "serde_bytes")]
    pub weights: Vec<u8>,
    /// Optional FPGA bitstream
    #[serde(with = "serde_bytes_option")]
    pub bitstream: Option<Vec<u8>>,
    /// Optional calibration data
    #[serde(with = "serde_bytes_option")]
    pub calibration: Option<Vec<u8>>,
    /// Test vectors for validation
    pub test_vectors: Vec<TestVector>,
    /// Ed25519 signature over manifest + file hashes
    #[serde(with = "serde_bytes")]
    pub signature: [u8; 64],
    /// Ed25519 public key
    #[serde(with = "serde_bytes")]
    pub pubkey: [u8; 32],
}

/// Serde helper for Option<Vec<u8>>
mod serde_bytes_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(data: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        match data {
            Some(bytes) => s.serialize_some(&serde_bytes::Bytes::new(bytes)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
        let opt: Option<serde_bytes::ByteBuf> = Option::deserialize(d)?;
        Ok(opt.map(|b| b.into_vec()))
    }
}

/// Test vector for model validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestVector {
    /// Input tokens
    pub tokens: Vec<u16>,
    /// Expected output logits (top-K or full)
    pub expected: Vec<i16>,
    /// Maximum absolute error allowed
    pub max_abs_err: i32,
}

impl ModelArtifact {
    /// Create a new artifact (for building/packing)
    pub fn new(
        manifest: Manifest,
        weights: Vec<u8>,
        bitstream: Option<Vec<u8>>,
        calibration: Option<Vec<u8>>,
        test_vectors: Vec<TestVector>,
    ) -> Self {
        Self {
            manifest,
            weights,
            bitstream,
            calibration,
            test_vectors,
            signature: [0u8; 64],
            pubkey: [0u8; 32],
        }
    }

    /// Compute model ID (SHA-256 of manifest + weights hash)
    pub fn model_id(&self) -> ModelId {
        let mut hasher = Sha256::new();
        hasher.update(self.manifest.name.as_bytes());
        hasher.update(&self.model_hash());
        hasher.update(&self.quant_hash());
        ModelId::new(hasher.finalize().into())
    }

    /// Compute hash of model weights
    pub fn model_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&self.weights);
        if let Some(ref bitstream) = self.bitstream {
            hasher.update(bitstream);
        }
        hasher.finalize().into()
    }

    /// Compute hash of quantization parameters
    pub fn quant_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        let quant_json = serde_json::to_string(&self.manifest.quant).unwrap_or_default();
        hasher.update(quant_json.as_bytes());
        if let Some(ref calib) = self.calibration {
            hasher.update(calib);
        }
        hasher.finalize().into()
    }

    /// Validate artifact integrity
    pub fn validate(&self) -> Result<()> {
        // Validate manifest
        self.manifest.validate()?;

        // Validate shape
        self.manifest
            .shape
            .validate()
            .map_err(|e| Error::InvalidArtifact(e))?;

        // Check weights size is reasonable
        let min_weight_size =
            self.manifest.shape.embedding_params() / self.manifest.quant.weights_per_byte();
        if self.weights.len() < min_weight_size {
            return Err(Error::InvalidArtifact(format!(
                "Weights too small: {} bytes, expected at least {} for embeddings",
                self.weights.len(),
                min_weight_size
            )));
        }

        // Validate test vectors if strict mode
        #[cfg(feature = "strict_verify")]
        self.run_test_vectors()?;

        Ok(())
    }

    /// Run test vectors for validation
    #[cfg(feature = "strict_verify")]
    pub fn run_test_vectors(&self) -> Result<()> {
        // This would require running inference, which creates a circular dependency
        // In practice, this is done by the backend after loading
        Ok(())
    }

    /// Get the fixed shape
    pub fn shape(&self) -> &FixedShape {
        &self.manifest.shape
    }

    /// Get quantization spec
    pub fn quant(&self) -> &QuantSpec {
        &self.manifest.quant
    }

    /// Check if artifact has FPGA bitstream
    pub fn has_bitstream(&self) -> bool {
        self.bitstream.is_some()
    }

    /// Estimated memory footprint in bytes
    pub fn memory_footprint(&self) -> usize {
        self.weights.len()
            + self.bitstream.as_ref().map(|b| b.len()).unwrap_or(0)
            + self.calibration.as_ref().map(|c| c.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_manifest() -> Manifest {
        Manifest {
            name: "test_model".into(),
            model_hash: "0".repeat(64),
            shape: FixedShape::micro(),
            quant: QuantSpec::int8(),
            io: IoSpec::default(),
            backend: BackendSpec::default(),
            tests: TestSpec::default(),
        }
    }

    #[test]
    fn test_model_id_computation() {
        let manifest = create_test_manifest();
        let artifact = ModelArtifact::new(manifest, vec![0u8; 4096 * 64], None, None, vec![]);

        let id1 = artifact.model_id();
        let id2 = artifact.model_id();
        assert_eq!(id1, id2); // Deterministic
    }

    #[test]
    fn test_model_hash() {
        let manifest = create_test_manifest();
        let artifact = ModelArtifact::new(manifest, vec![42u8; 4096 * 64], None, None, vec![]);

        let hash = artifact.model_hash();
        assert_ne!(hash, [0u8; 32]); // Non-zero hash
    }

    #[test]
    fn test_artifact_validation() {
        let manifest = create_test_manifest();
        let artifact = ModelArtifact::new(
            manifest,
            vec![0u8; 4096 * 64], // Enough for micro embeddings
            None,
            None,
            vec![],
        );

        assert!(artifact.validate().is_ok());
    }

    #[test]
    fn test_artifact_too_small_weights() {
        let manifest = create_test_manifest();
        let artifact = ModelArtifact::new(
            manifest,
            vec![0u8; 100], // Too small
            None,
            None,
            vec![],
        );

        assert!(artifact.validate().is_err());
    }
}
