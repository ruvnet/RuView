//! Artifact verification and signature validation

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::artifact::ModelArtifact;
use crate::error::{Error, Result};

/// Verify artifact signature
pub fn verify_signature(artifact: &ModelArtifact) -> Result<bool> {
    // Compute the message to verify (manifest hash + file hashes)
    let message = compute_signing_message(artifact);

    // Load public key
    let pubkey = VerifyingKey::from_bytes(&artifact.pubkey)
        .map_err(|e| Error::SignatureError(format!("Invalid public key: {}", e)))?;

    // Load signature
    let signature = Signature::from_bytes(&artifact.signature);

    // Verify
    pubkey
        .verify(&message, &signature)
        .map(|_| true)
        .map_err(|e| Error::SignatureError(format!("Verification failed: {}", e)))
}

/// Verify complete artifact integrity
pub fn verify_artifact(artifact: &ModelArtifact) -> Result<()> {
    // 1. Validate manifest
    artifact.manifest.validate()?;

    // 2. Verify model hash matches manifest
    let computed_hash = hex::encode(artifact.model_hash());
    if !artifact.manifest.model_hash.is_empty() && computed_hash != artifact.manifest.model_hash {
        return Err(Error::InvalidArtifact(format!(
            "Model hash mismatch: expected {}, got {}",
            artifact.manifest.model_hash, computed_hash
        )));
    }

    // 3. Verify signature (if present)
    if artifact.pubkey != [0u8; 32] {
        verify_signature(artifact)?;
    }

    // 4. Verify weights size
    let expected_min =
        artifact.manifest.shape.embedding_params() / artifact.manifest.quant.weights_per_byte();
    if artifact.weights.len() < expected_min {
        return Err(Error::InvalidArtifact(format!(
            "Weights too small: {} < {}",
            artifact.weights.len(),
            expected_min
        )));
    }

    Ok(())
}

/// Compute the message that was signed
fn compute_signing_message(artifact: &ModelArtifact) -> Vec<u8> {
    let mut hasher = Sha256::new();

    // Hash manifest
    let manifest_json = serde_json::to_string(&artifact.manifest).unwrap_or_default();
    hasher.update(manifest_json.as_bytes());

    // Hash weights
    let weights_hash = artifact.model_hash();
    hasher.update(&weights_hash);

    // Hash quant params
    let quant_hash = artifact.quant_hash();
    hasher.update(&quant_hash);

    // Hash bitstream if present
    if let Some(ref bitstream) = artifact.bitstream {
        let mut h = Sha256::new();
        h.update(bitstream);
        hasher.update(&h.finalize());
    }

    // Hash calibration if present
    if let Some(ref calib) = artifact.calibration {
        let mut h = Sha256::new();
        h.update(calib);
        hasher.update(&h.finalize());
    }

    hasher.finalize().to_vec()
}

/// Sign an artifact with Ed25519 private key
#[cfg(feature = "sign")]
pub fn sign_artifact(artifact: &mut ModelArtifact, secret_key: &[u8; 32]) -> Result<()> {
    use ed25519_dalek::{Signer, SigningKey};

    let signing_key = SigningKey::from_bytes(secret_key);
    let message = compute_signing_message(artifact);

    let signature = signing_key.sign(&message);

    artifact.signature = signature.to_bytes();
    artifact.pubkey = signing_key.verifying_key().to_bytes();

    Ok(())
}

/// Verify test vectors against model output
pub fn verify_test_vectors(
    artifact: &ModelArtifact,
    infer_fn: impl Fn(&[u16]) -> Result<Vec<i16>>,
) -> Result<()> {
    let max_err = artifact.manifest.tests.max_abs_err;

    for (i, vector) in artifact.test_vectors.iter().enumerate() {
        let output = infer_fn(&vector.tokens)?;

        // Compare outputs
        let actual_max_err = output
            .iter()
            .zip(&vector.expected)
            .map(|(&a, &b)| (a as i32 - b as i32).abs())
            .max()
            .unwrap_or(0);

        if actual_max_err > max_err {
            return Err(Error::TestVectorError {
                expected: max_err,
                actual: actual_max_err,
            });
        }
    }

    Ok(())
}

/// Generate test vectors for an artifact
pub fn generate_test_vectors(
    artifact: &mut ModelArtifact,
    infer_fn: impl Fn(&[u16]) -> Result<Vec<i16>>,
    count: usize,
) -> Result<()> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let seq_len = artifact.manifest.shape.seq_len as usize;
    let vocab = artifact.manifest.shape.vocab as u16;

    artifact.test_vectors.clear();

    for _ in 0..count {
        // Generate random input
        let tokens: Vec<u16> = (0..seq_len).map(|_| rng.gen_range(0..vocab)).collect();

        // Run inference
        let expected = infer_fn(&tokens)?;

        artifact.test_vectors.push(crate::artifact::TestVector {
            tokens,
            expected,
            max_abs_err: artifact.manifest.tests.max_abs_err,
        });
    }

    artifact.manifest.tests.vectors = count as u32;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::Manifest;
    use crate::types::{FixedShape, QuantSpec};

    fn create_test_artifact() -> ModelArtifact {
        let manifest = Manifest {
            name: "test".into(),
            model_hash: String::new(),
            shape: FixedShape::micro(),
            quant: QuantSpec::int8(),
            io: Default::default(),
            backend: Default::default(),
            tests: Default::default(),
        };

        ModelArtifact::new(manifest, vec![0u8; 4096 * 64], None, None, vec![])
    }

    #[test]
    fn test_verify_artifact() {
        let artifact = create_test_artifact();
        assert!(verify_artifact(&artifact).is_ok());
    }

    #[test]
    fn test_compute_signing_message() {
        let artifact = create_test_artifact();
        let msg = compute_signing_message(&artifact);
        assert_eq!(msg.len(), 32); // SHA-256 output
    }
}
