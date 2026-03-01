//! Ed25519 Cryptographic Verification for Simulation Results
//!
//! Provides provenance verification for simulation benchmarks using Ed25519 signatures.
//! This ensures that benchmark results are authentic and have not been tampered with.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Benchmark result with cryptographic provenance
#[derive(Debug, Clone)]
pub struct VerifiedBenchmarkResult {
    /// Benchmark name
    pub name: String,
    /// Total simulations performed
    pub total_simulations: u64,
    /// Elapsed time
    pub elapsed: Duration,
    /// Throughput (simulations/second)
    pub throughput: f64,
    /// Timestamp (Unix epoch)
    pub timestamp: u64,
    /// Platform info
    pub platform: String,
    /// SHA-256 hash of result data
    pub result_hash: [u8; 32],
    /// Ed25519 signature
    pub signature: [u8; 64],
}

impl VerifiedBenchmarkResult {
    /// Create new verified result (generates hash and signature)
    pub fn new(
        name: &str,
        total_simulations: u64,
        elapsed: Duration,
        signing_key: &SigningKey,
    ) -> Self {
        let throughput = total_simulations as f64 / elapsed.as_secs_f64();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let platform = detect_platform();

        // Create canonical data for hashing
        let data = format!(
            "{}|{}|{}|{}|{}",
            name, total_simulations, elapsed.as_nanos(), timestamp, platform
        );

        // SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        let result_hash: [u8; 32] = hasher.finalize().into();

        // Ed25519 signature
        let signature_obj = signing_key.sign(&result_hash);
        let signature: [u8; 64] = signature_obj.to_bytes();

        Self {
            name: name.to_string(),
            total_simulations,
            elapsed,
            throughput,
            timestamp,
            platform,
            result_hash,
            signature,
        }
    }

    /// Verify the signature against the result hash
    pub fn verify(&self, verifying_key: &VerifyingKey) -> bool {
        // Reconstruct data and hash
        let data = format!(
            "{}|{}|{}|{}|{}",
            self.name,
            self.total_simulations,
            self.elapsed.as_nanos(),
            self.timestamp,
            self.platform
        );

        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        let expected_hash: [u8; 32] = hasher.finalize().into();

        // Check hash matches
        if expected_hash != self.result_hash {
            return false;
        }

        // Verify signature
        match Signature::try_from(&self.signature[..]) {
            Ok(sig) => verifying_key.verify(&self.result_hash, &sig).is_ok(),
            Err(_) => false,
        }
    }

    /// Get hex-encoded signature
    pub fn signature_hex(&self) -> String {
        hex::encode(&self.signature)
    }

    /// Get hex-encoded hash
    pub fn hash_hex(&self) -> String {
        hex::encode(&self.result_hash)
    }
}

/// Benchmark suite with Ed25519 key pair
pub struct VerifiedBenchmarkSuite {
    /// Signing key (private)
    signing_key: SigningKey,
    /// Verifying key (public)
    verifying_key: VerifyingKey,
    /// Collected results
    results: Vec<VerifiedBenchmarkResult>,
}

impl VerifiedBenchmarkSuite {
    /// Create new suite with random key pair
    pub fn new() -> Self {
        let mut rng = rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key = signing_key.verifying_key();

        Self {
            signing_key,
            verifying_key,
            results: Vec::new(),
        }
    }

    /// Create suite from seed (deterministic)
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        Self {
            signing_key,
            verifying_key,
            results: Vec::new(),
        }
    }

    /// Record a benchmark result
    pub fn record(&mut self, name: &str, total_simulations: u64, elapsed: Duration) {
        let result = VerifiedBenchmarkResult::new(
            name,
            total_simulations,
            elapsed,
            &self.signing_key,
        );
        self.results.push(result);
    }

    /// Verify all results
    pub fn verify_all(&self) -> bool {
        self.results.iter().all(|r| r.verify(&self.verifying_key))
    }

    /// Get public key hex
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key.as_bytes())
    }

    /// Get all results
    pub fn results(&self) -> &[VerifiedBenchmarkResult] {
        &self.results
    }

    /// Print verification report
    pub fn print_report(&self) {
        println!();
        println!("═══════════════════════════════════════════════════════════════════");
        println!("  ED25519 CRYPTOGRAPHIC VERIFICATION REPORT");
        println!("═══════════════════════════════════════════════════════════════════");
        println!();
        println!("🔑 Public Key: {}", self.public_key_hex());
        println!();

        for (i, result) in self.results.iter().enumerate() {
            let verified = result.verify(&self.verifying_key);
            let status = if verified { "✅ VERIFIED" } else { "❌ FAILED" };

            println!("{}. {}", i + 1, result.name);
            println!("   Simulations: {:.3e}", result.total_simulations as f64);
            println!("   Throughput:  {:.3e} sims/sec", result.throughput);
            println!("   Hash:        {}...", &result.hash_hex()[..16]);
            println!("   Signature:   {}...", &result.signature_hex()[..32]);
            println!("   Status:      {}", status);
            println!();
        }

        let all_verified = self.verify_all();
        if all_verified {
            println!("🔒 ALL RESULTS CRYPTOGRAPHICALLY VERIFIED");
        } else {
            println!("⚠️  SOME RESULTS FAILED VERIFICATION");
        }
        println!("═══════════════════════════════════════════════════════════════════");
    }
}

impl Default for VerifiedBenchmarkSuite {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect platform for provenance
fn detect_platform() -> String {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    #[cfg(target_arch = "x86_64")]
    let simd = {
        if is_x86_feature_detected!("avx512f") {
            "AVX-512"
        } else if is_x86_feature_detected!("avx2") {
            "AVX2"
        } else {
            "SSE"
        }
    };

    #[cfg(target_arch = "aarch64")]
    let simd = "NEON";

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    let simd = "Scalar";

    format!("{}-{}-{}", arch, os, simd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verified_result() {
        let mut rng = rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key = signing_key.verifying_key();

        let result = VerifiedBenchmarkResult::new(
            "test_benchmark",
            1_000_000,
            Duration::from_millis(100),
            &signing_key,
        );

        assert!(result.verify(&verifying_key));
    }

    #[test]
    fn test_tamper_detection() {
        let mut rng = rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key = signing_key.verifying_key();

        let mut result = VerifiedBenchmarkResult::new(
            "test_benchmark",
            1_000_000,
            Duration::from_millis(100),
            &signing_key,
        );

        // Tamper with simulations count
        result.total_simulations = 999_999_999;

        // Should fail verification
        assert!(!result.verify(&verifying_key));
    }

    #[test]
    fn test_benchmark_suite() {
        let mut suite = VerifiedBenchmarkSuite::new();

        suite.record("bench1", 1_000_000, Duration::from_millis(50));
        suite.record("bench2", 5_000_000, Duration::from_millis(100));

        assert_eq!(suite.results().len(), 2);
        assert!(suite.verify_all());
    }

    #[test]
    fn test_deterministic_key() {
        let seed = [42u8; 32];
        let suite1 = VerifiedBenchmarkSuite::from_seed(seed);
        let suite2 = VerifiedBenchmarkSuite::from_seed(seed);

        assert_eq!(suite1.public_key_hex(), suite2.public_key_hex());
    }
}
