//! Artifact packing and unpacking

use std::io::{Read, Write};
use std::path::Path;

use crate::artifact::{ModelArtifact, TestVector};
use crate::error::{Error, Result};

/// Magic bytes for artifact file format
const ARTIFACT_MAGIC: &[u8; 4] = b"RVAT"; // RuVector ArTifact
const ARTIFACT_VERSION: u16 = 1;

// Security: Maximum size limits to prevent DoS via unbounded allocations
/// Maximum manifest size (1 MB)
const MAX_MANIFEST_SIZE: usize = 1024 * 1024;
/// Maximum weights size (1 GB)
const MAX_WEIGHTS_SIZE: usize = 1024 * 1024 * 1024;
/// Maximum bitstream/calibration size (100 MB)
const MAX_BLOB_SIZE: usize = 100 * 1024 * 1024;
/// Maximum number of test vectors
const MAX_TEST_VECTORS: usize = 10_000;
/// Maximum tokens per test vector
const MAX_TOKENS_PER_VECTOR: usize = 65_536;
/// Maximum expected values per test vector
const MAX_EXPECTED_PER_VECTOR: usize = 1_000_000;

/// Pack an artifact to bytes
pub fn pack_artifact(artifact: &ModelArtifact) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();

    // Write magic and version
    buffer.extend_from_slice(ARTIFACT_MAGIC);
    buffer.extend_from_slice(&ARTIFACT_VERSION.to_le_bytes());

    // Write manifest as JSON with length prefix
    let manifest_json = serde_json::to_string(&artifact.manifest)?;
    let manifest_bytes = manifest_json.as_bytes();
    buffer.extend_from_slice(&(manifest_bytes.len() as u32).to_le_bytes());
    buffer.extend_from_slice(manifest_bytes);

    // Write weights with length prefix
    buffer.extend_from_slice(&(artifact.weights.len() as u64).to_le_bytes());
    buffer.extend_from_slice(&artifact.weights);

    // Write optional bitstream
    if let Some(ref bitstream) = artifact.bitstream {
        buffer.push(1); // Present flag
        buffer.extend_from_slice(&(bitstream.len() as u64).to_le_bytes());
        buffer.extend_from_slice(bitstream);
    } else {
        buffer.push(0); // Not present
    }

    // Write optional calibration
    if let Some(ref calibration) = artifact.calibration {
        buffer.push(1);
        buffer.extend_from_slice(&(calibration.len() as u64).to_le_bytes());
        buffer.extend_from_slice(calibration);
    } else {
        buffer.push(0);
    }

    // Write test vectors
    buffer.extend_from_slice(&(artifact.test_vectors.len() as u32).to_le_bytes());
    for vector in &artifact.test_vectors {
        // Write tokens
        buffer.extend_from_slice(&(vector.tokens.len() as u16).to_le_bytes());
        for &token in &vector.tokens {
            buffer.extend_from_slice(&token.to_le_bytes());
        }
        // Write expected
        buffer.extend_from_slice(&(vector.expected.len() as u32).to_le_bytes());
        for &exp in &vector.expected {
            buffer.extend_from_slice(&exp.to_le_bytes());
        }
        // Write max_abs_err
        buffer.extend_from_slice(&vector.max_abs_err.to_le_bytes());
    }

    // Write signature and pubkey
    buffer.extend_from_slice(&artifact.signature);
    buffer.extend_from_slice(&artifact.pubkey);

    Ok(buffer)
}

/// Unpack an artifact from bytes
pub fn unpack_artifact(data: &[u8]) -> Result<ModelArtifact> {
    let mut cursor = std::io::Cursor::new(data);
    let mut read_buf = [0u8; 8];

    // Read and verify magic
    cursor.read_exact(&mut read_buf[..4])?;
    if &read_buf[..4] != ARTIFACT_MAGIC {
        return Err(Error::InvalidArtifact("Invalid magic bytes".into()));
    }

    // Read version
    cursor.read_exact(&mut read_buf[..2])?;
    let version = u16::from_le_bytes([read_buf[0], read_buf[1]]);
    if version != ARTIFACT_VERSION {
        return Err(Error::InvalidArtifact(format!(
            "Unsupported version: {}",
            version
        )));
    }

    // Read manifest
    cursor.read_exact(&mut read_buf[..4])?;
    let manifest_len = u32::from_le_bytes(read_buf[..4].try_into().unwrap()) as usize;
    if manifest_len > MAX_MANIFEST_SIZE {
        return Err(Error::InvalidArtifact(format!(
            "Manifest size {} exceeds maximum {}",
            manifest_len, MAX_MANIFEST_SIZE
        )));
    }
    let mut manifest_bytes = vec![0u8; manifest_len];
    cursor.read_exact(&mut manifest_bytes)?;
    let manifest = serde_json::from_slice(&manifest_bytes)?;

    // Read weights
    cursor.read_exact(&mut read_buf)?;
    let weights_len = u64::from_le_bytes(read_buf) as usize;
    if weights_len > MAX_WEIGHTS_SIZE {
        return Err(Error::InvalidArtifact(format!(
            "Weights size {} exceeds maximum {}",
            weights_len, MAX_WEIGHTS_SIZE
        )));
    }
    let mut weights = vec![0u8; weights_len];
    cursor.read_exact(&mut weights)?;

    // Read optional bitstream
    cursor.read_exact(&mut read_buf[..1])?;
    let bitstream = if read_buf[0] == 1 {
        cursor.read_exact(&mut read_buf)?;
        let len = u64::from_le_bytes(read_buf) as usize;
        if len > MAX_BLOB_SIZE {
            return Err(Error::InvalidArtifact(format!(
                "Bitstream size {} exceeds maximum {}",
                len, MAX_BLOB_SIZE
            )));
        }
        let mut data = vec![0u8; len];
        cursor.read_exact(&mut data)?;
        Some(data)
    } else {
        None
    };

    // Read optional calibration
    cursor.read_exact(&mut read_buf[..1])?;
    let calibration = if read_buf[0] == 1 {
        cursor.read_exact(&mut read_buf)?;
        let len = u64::from_le_bytes(read_buf) as usize;
        if len > MAX_BLOB_SIZE {
            return Err(Error::InvalidArtifact(format!(
                "Calibration size {} exceeds maximum {}",
                len, MAX_BLOB_SIZE
            )));
        }
        let mut data = vec![0u8; len];
        cursor.read_exact(&mut data)?;
        Some(data)
    } else {
        None
    };

    // Read test vectors
    cursor.read_exact(&mut read_buf[..4])?;
    let num_vectors = u32::from_le_bytes(read_buf[..4].try_into().unwrap()) as usize;
    if num_vectors > MAX_TEST_VECTORS {
        return Err(Error::InvalidArtifact(format!(
            "Test vector count {} exceeds maximum {}",
            num_vectors, MAX_TEST_VECTORS
        )));
    }
    let mut test_vectors = Vec::with_capacity(num_vectors);

    for _ in 0..num_vectors {
        // Read tokens
        cursor.read_exact(&mut read_buf[..2])?;
        let num_tokens = u16::from_le_bytes([read_buf[0], read_buf[1]]) as usize;
        if num_tokens > MAX_TOKENS_PER_VECTOR {
            return Err(Error::InvalidArtifact(format!(
                "Token count {} exceeds maximum {}",
                num_tokens, MAX_TOKENS_PER_VECTOR
            )));
        }
        let mut tokens = Vec::with_capacity(num_tokens);
        for _ in 0..num_tokens {
            cursor.read_exact(&mut read_buf[..2])?;
            tokens.push(u16::from_le_bytes([read_buf[0], read_buf[1]]));
        }

        // Read expected
        cursor.read_exact(&mut read_buf[..4])?;
        let num_expected = u32::from_le_bytes(read_buf[..4].try_into().unwrap()) as usize;
        if num_expected > MAX_EXPECTED_PER_VECTOR {
            return Err(Error::InvalidArtifact(format!(
                "Expected values count {} exceeds maximum {}",
                num_expected, MAX_EXPECTED_PER_VECTOR
            )));
        }
        let mut expected = Vec::with_capacity(num_expected);
        for _ in 0..num_expected {
            cursor.read_exact(&mut read_buf[..2])?;
            expected.push(i16::from_le_bytes([read_buf[0], read_buf[1]]));
        }

        // Read max_abs_err
        cursor.read_exact(&mut read_buf[..4])?;
        let max_abs_err = i32::from_le_bytes(read_buf[..4].try_into().unwrap());

        test_vectors.push(TestVector {
            tokens,
            expected,
            max_abs_err,
        });
    }

    // Read signature and pubkey
    let mut signature = [0u8; 64];
    cursor.read_exact(&mut signature)?;
    let mut pubkey = [0u8; 32];
    cursor.read_exact(&mut pubkey)?;

    Ok(ModelArtifact {
        manifest,
        weights,
        bitstream,
        calibration,
        test_vectors,
        signature,
        pubkey,
    })
}

/// Save artifact to file
pub fn save_artifact(artifact: &ModelArtifact, path: impl AsRef<Path>) -> Result<()> {
    let data = pack_artifact(artifact)?;
    std::fs::write(path, data)?;
    Ok(())
}

/// Load artifact from file
pub fn load_artifact(path: impl AsRef<Path>) -> Result<ModelArtifact> {
    let data = std::fs::read(path)?;
    unpack_artifact(&data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::Manifest;
    use crate::types::{FixedShape, QuantSpec};

    fn create_test_artifact() -> ModelArtifact {
        let manifest = Manifest {
            name: "test_pack".into(),
            model_hash: "abc123".into(),
            shape: FixedShape::micro(),
            quant: QuantSpec::int8(),
            io: Default::default(),
            backend: Default::default(),
            tests: Default::default(),
        };

        ModelArtifact {
            manifest,
            weights: (0..5000).map(|i| (i % 256) as u8).collect(),
            bitstream: Some(vec![0xFF; 100]),
            calibration: None,
            test_vectors: vec![TestVector {
                tokens: vec![1, 2, 3],
                expected: vec![100, 200, 300],
                max_abs_err: 5,
            }],
            signature: [0x42u8; 64],
            pubkey: [0x24u8; 32],
        }
    }

    #[test]
    fn test_pack_unpack_roundtrip() {
        let original = create_test_artifact();
        let packed = pack_artifact(&original).unwrap();
        let unpacked = unpack_artifact(&packed).unwrap();

        assert_eq!(original.manifest.name, unpacked.manifest.name);
        assert_eq!(original.weights, unpacked.weights);
        assert_eq!(original.bitstream, unpacked.bitstream);
        assert_eq!(original.calibration, unpacked.calibration);
        assert_eq!(original.test_vectors.len(), unpacked.test_vectors.len());
        assert_eq!(original.signature, unpacked.signature);
        assert_eq!(original.pubkey, unpacked.pubkey);
    }

    #[test]
    fn test_invalid_magic() {
        let data = b"XXXX0000";
        assert!(unpack_artifact(data).is_err());
    }
}
