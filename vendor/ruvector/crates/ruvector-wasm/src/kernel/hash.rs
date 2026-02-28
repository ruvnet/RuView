//! SHA256 Hash Verification
//!
//! Provides hash verification for WASM kernel files to ensure integrity.

use crate::kernel::error::VerifyError;
use sha2::{Digest, Sha256};

/// Hash verifier for kernel files
#[derive(Debug, Clone)]
pub struct HashVerifier {
    /// Expected hash format prefix (e.g., "sha256:")
    prefix: String,
}

impl HashVerifier {
    /// Create a new SHA256 hash verifier
    pub fn sha256() -> Self {
        HashVerifier {
            prefix: "sha256:".to_string(),
        }
    }

    /// Compute SHA256 hash of data
    pub fn compute_hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        format!("sha256:{:x}", result)
    }

    /// Verify kernel data against expected hash
    ///
    /// # Arguments
    /// * `kernel_bytes` - The raw WASM kernel bytes
    /// * `expected_hash` - Expected hash string (format: "sha256:...")
    ///
    /// # Returns
    /// * `Ok(())` if hash matches
    /// * `Err(VerifyError::HashMismatch)` if hash doesn't match
    pub fn verify(&self, kernel_bytes: &[u8], expected_hash: &str) -> Result<(), VerifyError> {
        // Validate expected hash format
        if !expected_hash.starts_with(&self.prefix) {
            return Err(VerifyError::InvalidManifest {
                message: format!(
                    "Invalid hash format: expected '{}' prefix, got '{}'",
                    self.prefix,
                    expected_hash.get(..10).unwrap_or(expected_hash)
                ),
            });
        }

        let actual_hash = Self::compute_hash(kernel_bytes);

        if actual_hash.eq_ignore_ascii_case(expected_hash) {
            Ok(())
        } else {
            Err(VerifyError::HashMismatch {
                expected: expected_hash.to_string(),
                actual: actual_hash,
            })
        }
    }

    /// Verify multiple kernels in batch
    ///
    /// # Arguments
    /// * `kernels` - Iterator of (kernel_bytes, expected_hash) pairs
    ///
    /// # Returns
    /// * `Ok(())` if all hashes match
    /// * `Err` with first mismatch
    pub fn verify_batch<'a>(
        &self,
        kernels: impl Iterator<Item = (&'a [u8], &'a str)>,
    ) -> Result<(), VerifyError> {
        for (bytes, expected) in kernels {
            self.verify(bytes, expected)?;
        }
        Ok(())
    }
}

impl Default for HashVerifier {
    fn default() -> Self {
        Self::sha256()
    }
}

/// Compute hash for a kernel file and return formatted string
pub fn hash_kernel(kernel_bytes: &[u8]) -> String {
    HashVerifier::compute_hash(kernel_bytes)
}

/// Verify a kernel file against expected hash (convenience function)
pub fn verify_kernel_hash(kernel_bytes: &[u8], expected_hash: &str) -> Result<(), VerifyError> {
    HashVerifier::sha256().verify(kernel_bytes, expected_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash() {
        let data = b"hello world";
        let hash = HashVerifier::compute_hash(data);
        assert!(hash.starts_with("sha256:"));
        // Known SHA256 of "hello world"
        assert!(hash.contains("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"));
    }

    #[test]
    fn test_verify_success() {
        let data = b"test kernel data";
        let hash = HashVerifier::compute_hash(data);

        let verifier = HashVerifier::sha256();
        assert!(verifier.verify(data, &hash).is_ok());
    }

    #[test]
    fn test_verify_case_insensitive() {
        let data = b"test kernel data";
        let hash = HashVerifier::compute_hash(data);
        let upper_hash = hash.to_uppercase();

        let verifier = HashVerifier::sha256();
        assert!(verifier.verify(data, &upper_hash).is_ok());
    }

    #[test]
    fn test_verify_mismatch() {
        let data = b"actual data";
        let wrong_hash = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

        let verifier = HashVerifier::sha256();
        let result = verifier.verify(data, wrong_hash);

        assert!(matches!(result, Err(VerifyError::HashMismatch { .. })));
    }

    #[test]
    fn test_verify_invalid_format() {
        let data = b"test data";
        let invalid_hash = "md5:abc123";

        let verifier = HashVerifier::sha256();
        let result = verifier.verify(data, invalid_hash);

        assert!(matches!(result, Err(VerifyError::InvalidManifest { .. })));
    }

    #[test]
    fn test_verify_batch() {
        let data1 = b"kernel1";
        let data2 = b"kernel2";
        let hash1 = HashVerifier::compute_hash(data1);
        let hash2 = HashVerifier::compute_hash(data2);

        let verifier = HashVerifier::sha256();
        let kernels = vec![
            (data1.as_slice(), hash1.as_str()),
            (data2.as_slice(), hash2.as_str()),
        ];

        assert!(verifier.verify_batch(kernels.into_iter()).is_ok());
    }

    #[test]
    fn test_convenience_function() {
        let data = b"convenience test";
        let hash = hash_kernel(data);

        assert!(verify_kernel_hash(data, &hash).is_ok());
    }
}
