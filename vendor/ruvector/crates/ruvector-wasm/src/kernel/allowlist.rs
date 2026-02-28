//! Trusted Kernel Allowlist
//!
//! Maintains a list of approved kernel hashes for additional security.
//! This provides defense-in-depth beyond signature verification.

use crate::kernel::error::VerifyError;
use std::collections::{HashMap, HashSet};

/// Trusted kernel allowlist
///
/// Maintains approved kernel hashes organized by kernel ID.
/// Even if a kernel has a valid signature, it must be in the allowlist
/// to be executed (when allowlist enforcement is enabled).
#[derive(Debug, Clone)]
pub struct TrustedKernelAllowlist {
    /// Set of approved kernel hashes (format: "sha256:...")
    approved_hashes: HashSet<String>,

    /// Map of kernel_id -> approved hashes for that kernel
    kernel_hashes: HashMap<String, HashSet<String>>,

    /// Whether to enforce allowlist (can be disabled for development)
    enforce: bool,

    /// Allowlist version/update timestamp
    version: String,
}

impl TrustedKernelAllowlist {
    /// Create a new empty allowlist
    pub fn new() -> Self {
        TrustedKernelAllowlist {
            approved_hashes: HashSet::new(),
            kernel_hashes: HashMap::new(),
            enforce: true,
            version: "1.0.0".to_string(),
        }
    }

    /// Create an allowlist that doesn't enforce checks (for development)
    ///
    /// # Warning
    /// This should NEVER be used in production.
    pub fn insecure_allow_all() -> Self {
        TrustedKernelAllowlist {
            approved_hashes: HashSet::new(),
            kernel_hashes: HashMap::new(),
            enforce: false,
            version: "dev".to_string(),
        }
    }

    /// Load allowlist from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        #[derive(serde::Deserialize)]
        struct AllowlistJson {
            version: String,
            kernels: HashMap<String, Vec<String>>,
        }

        let parsed: AllowlistJson = serde_json::from_str(json)?;

        let mut allowlist = TrustedKernelAllowlist::new();
        allowlist.version = parsed.version;

        for (kernel_id, hashes) in parsed.kernels {
            for hash in hashes {
                allowlist.add_kernel_hash(&kernel_id, &hash);
            }
        }

        Ok(allowlist)
    }

    /// Serialize allowlist to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        #[derive(serde::Serialize)]
        struct AllowlistJson {
            version: String,
            kernels: HashMap<String, Vec<String>>,
        }

        let kernels: HashMap<String, Vec<String>> = self
            .kernel_hashes
            .iter()
            .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
            .collect();

        let json = AllowlistJson {
            version: self.version.clone(),
            kernels,
        };

        serde_json::to_string_pretty(&json)
    }

    /// Add a hash to the global approved set
    pub fn add_hash(&mut self, hash: &str) {
        self.approved_hashes.insert(hash.to_lowercase());
    }

    /// Add a hash for a specific kernel ID
    pub fn add_kernel_hash(&mut self, kernel_id: &str, hash: &str) {
        let lowercase_hash = hash.to_lowercase();
        self.approved_hashes.insert(lowercase_hash.clone());

        self.kernel_hashes
            .entry(kernel_id.to_string())
            .or_insert_with(HashSet::new)
            .insert(lowercase_hash);
    }

    /// Remove a hash from the allowlist
    pub fn remove_hash(&mut self, hash: &str) {
        let lowercase_hash = hash.to_lowercase();
        self.approved_hashes.remove(&lowercase_hash);

        for hashes in self.kernel_hashes.values_mut() {
            hashes.remove(&lowercase_hash);
        }
    }

    /// Check if a hash is in the allowlist
    pub fn is_allowed(&self, hash: &str) -> bool {
        if !self.enforce {
            return true;
        }
        self.approved_hashes.contains(&hash.to_lowercase())
    }

    /// Check if a hash is allowed for a specific kernel ID
    pub fn is_allowed_for_kernel(&self, kernel_id: &str, hash: &str) -> bool {
        if !self.enforce {
            return true;
        }

        let lowercase_hash = hash.to_lowercase();

        // Check kernel-specific allowlist first
        if let Some(kernel_hashes) = self.kernel_hashes.get(kernel_id) {
            return kernel_hashes.contains(&lowercase_hash);
        }

        // Fall back to global allowlist
        self.approved_hashes.contains(&lowercase_hash)
    }

    /// Verify a kernel is in the allowlist
    pub fn verify(&self, kernel_id: &str, hash: &str) -> Result<(), VerifyError> {
        if self.is_allowed_for_kernel(kernel_id, hash) {
            Ok(())
        } else {
            Err(VerifyError::NotInAllowlist {
                kernel_id: kernel_id.to_string(),
            })
        }
    }

    /// Get number of approved hashes
    pub fn hash_count(&self) -> usize {
        self.approved_hashes.len()
    }

    /// Get all approved hashes for a kernel ID
    pub fn get_kernel_hashes(&self, kernel_id: &str) -> Option<&HashSet<String>> {
        self.kernel_hashes.get(kernel_id)
    }

    /// List all kernel IDs with approved hashes
    pub fn kernel_ids(&self) -> Vec<&str> {
        self.kernel_hashes.keys().map(|s| s.as_str()).collect()
    }

    /// Get allowlist version
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Set allowlist version
    pub fn set_version(&mut self, version: &str) {
        self.version = version.to_string();
    }

    /// Check if enforcement is enabled
    pub fn is_enforced(&self) -> bool {
        self.enforce
    }

    /// Merge another allowlist into this one
    pub fn merge(&mut self, other: &TrustedKernelAllowlist) {
        for hash in &other.approved_hashes {
            self.approved_hashes.insert(hash.clone());
        }

        for (kernel_id, hashes) in &other.kernel_hashes {
            let entry = self
                .kernel_hashes
                .entry(kernel_id.clone())
                .or_insert_with(HashSet::new);
            for hash in hashes {
                entry.insert(hash.clone());
            }
        }
    }
}

impl Default for TrustedKernelAllowlist {
    fn default() -> Self {
        Self::new()
    }
}

/// Built-in allowlist of official RuvLLM kernels
///
/// This provides a starting point with known-good kernel hashes.
/// Production deployments should maintain their own allowlist.
pub fn builtin_allowlist() -> TrustedKernelAllowlist {
    let mut allowlist = TrustedKernelAllowlist::new();
    allowlist.set_version("0.1.0-builtin");

    // Add placeholders for official kernels
    // These would be replaced with actual hashes in production
    // allowlist.add_kernel_hash("rope_f32", "sha256:...");
    // allowlist.add_kernel_hash("rmsnorm_f32", "sha256:...");
    // allowlist.add_kernel_hash("swiglu_f32", "sha256:...");

    allowlist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_check_hash() {
        let mut allowlist = TrustedKernelAllowlist::new();
        let hash = "sha256:abc123def456";

        assert!(!allowlist.is_allowed(hash));

        allowlist.add_hash(hash);
        assert!(allowlist.is_allowed(hash));

        // Case insensitive
        assert!(allowlist.is_allowed("SHA256:ABC123DEF456"));
    }

    #[test]
    fn test_kernel_specific_hash() {
        let mut allowlist = TrustedKernelAllowlist::new();

        allowlist.add_kernel_hash("rope_f32", "sha256:rope_hash");
        allowlist.add_kernel_hash("rmsnorm_f32", "sha256:rmsnorm_hash");

        assert!(allowlist.is_allowed_for_kernel("rope_f32", "sha256:rope_hash"));
        assert!(!allowlist.is_allowed_for_kernel("rope_f32", "sha256:rmsnorm_hash"));
        assert!(allowlist.is_allowed_for_kernel("rmsnorm_f32", "sha256:rmsnorm_hash"));
    }

    #[test]
    fn test_verify() {
        let mut allowlist = TrustedKernelAllowlist::new();
        allowlist.add_kernel_hash("rope_f32", "sha256:valid_hash");

        assert!(allowlist.verify("rope_f32", "sha256:valid_hash").is_ok());
        assert!(matches!(
            allowlist.verify("rope_f32", "sha256:invalid_hash"),
            Err(VerifyError::NotInAllowlist { .. })
        ));
    }

    #[test]
    fn test_insecure_allow_all() {
        let allowlist = TrustedKernelAllowlist::insecure_allow_all();

        // Should allow any hash when not enforcing
        assert!(allowlist.is_allowed("sha256:anything"));
        assert!(allowlist.is_allowed_for_kernel("any_kernel", "sha256:anything"));
        assert!(!allowlist.is_enforced());
    }

    #[test]
    fn test_remove_hash() {
        let mut allowlist = TrustedKernelAllowlist::new();
        allowlist.add_kernel_hash("kernel", "sha256:hash");

        assert!(allowlist.is_allowed("sha256:hash"));

        allowlist.remove_hash("sha256:hash");
        assert!(!allowlist.is_allowed("sha256:hash"));
    }

    #[test]
    fn test_json_roundtrip() {
        let mut original = TrustedKernelAllowlist::new();
        original.set_version("1.2.3");
        original.add_kernel_hash("rope_f32", "sha256:hash1");
        original.add_kernel_hash("rope_f32", "sha256:hash2");
        original.add_kernel_hash("rmsnorm_f32", "sha256:hash3");

        let json = original.to_json().unwrap();
        let restored = TrustedKernelAllowlist::from_json(&json).unwrap();

        assert_eq!(restored.version(), "1.2.3");
        assert!(restored.is_allowed_for_kernel("rope_f32", "sha256:hash1"));
        assert!(restored.is_allowed_for_kernel("rope_f32", "sha256:hash2"));
        assert!(restored.is_allowed_for_kernel("rmsnorm_f32", "sha256:hash3"));
    }

    #[test]
    fn test_merge() {
        let mut allowlist1 = TrustedKernelAllowlist::new();
        allowlist1.add_kernel_hash("kernel1", "sha256:hash1");

        let mut allowlist2 = TrustedKernelAllowlist::new();
        allowlist2.add_kernel_hash("kernel2", "sha256:hash2");

        allowlist1.merge(&allowlist2);

        assert!(allowlist1.is_allowed_for_kernel("kernel1", "sha256:hash1"));
        assert!(allowlist1.is_allowed_for_kernel("kernel2", "sha256:hash2"));
    }

    #[test]
    fn test_kernel_ids() {
        let mut allowlist = TrustedKernelAllowlist::new();
        allowlist.add_kernel_hash("kernel_a", "sha256:a");
        allowlist.add_kernel_hash("kernel_b", "sha256:b");

        let ids = allowlist.kernel_ids();
        assert!(ids.contains(&"kernel_a"));
        assert!(ids.contains(&"kernel_b"));
    }
}
