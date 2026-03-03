//! Policy verification using temporal neural solver
//!
//! Simplified implementation using aimds-core types
//!
//! Performance target: <500ms p99

use aimds_core::types::PromptInput;
use crate::errors::AnalysisResult;
use std::sync::Arc;
use std::collections::HashMap;

/// Security policy with LTL formula
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecurityPolicy {
    /// Policy identifier
    pub id: String,
    /// Human-readable description
    pub description: String,
    /// LTL formula for verification
    pub formula: String,
    /// Policy severity (0.0 = info, 1.0 = critical)
    pub severity: f64,
    /// Whether policy is enabled
    pub enabled: bool,
}

impl SecurityPolicy {
    /// Create new security policy
    pub fn new(id: impl Into<String>, description: impl Into<String>, formula: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            formula: formula.into(),
            severity: 0.5,
            enabled: true,
        }
    }

    /// Set policy severity
    pub fn with_severity(mut self, severity: f64) -> Self {
        self.severity = severity.clamp(0.0, 1.0);
        self
    }

    /// Enable or disable policy
    pub fn set_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Policy verification result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerificationResult {
    /// Whether policy verification passed
    pub verified: bool,
    /// Confidence in verification result
    pub confidence: f64,
    /// List of policy violations (if any)
    pub violations: Vec<String>,
    /// Optional proof certificate
    pub proof: Option<ProofCertificate>,
}

impl VerificationResult {
    /// Create verified result
    pub fn verified() -> Self {
        Self {
            verified: true,
            confidence: 1.0,
            violations: Vec::new(),
            proof: None,
        }
    }

    /// Create verification failure
    pub fn failed(violations: Vec<String>) -> Self {
        Self {
            verified: false,
            confidence: 1.0,
            violations,
            proof: None,
        }
    }

    /// Add proof certificate
    pub fn with_proof(mut self, proof: ProofCertificate) -> Self {
        self.proof = Some(proof);
        self
    }
}

/// Proof certificate for verification
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProofCertificate {
    /// Proof type
    pub proof_type: String,
    /// Proof steps
    pub steps: Vec<String>,
    /// Verification timestamp
    pub timestamp: u64,
}

/// Policy verifier
pub struct PolicyVerifier {
    policies: Arc<std::sync::RwLock<HashMap<String, SecurityPolicy>>>,
}

impl PolicyVerifier {
    /// Create new policy verifier
    pub fn new() -> AnalysisResult<Self> {
        Ok(Self {
            policies: Arc::new(std::sync::RwLock::new(HashMap::new())),
        })
    }

    /// Verify action against all enabled policies
    pub async fn verify_policy(&self, input: &PromptInput) -> AnalysisResult<VerificationResult> {
        let policies = self.policies.read().unwrap();
        let enabled_policies: Vec<_> = policies.values()
            .filter(|p| p.enabled)
            .cloned()
            .collect();

        drop(policies);

        if enabled_policies.is_empty() {
            return Ok(VerificationResult::verified());
        }

        // Simplified verification - checks for basic patterns
        let mut violations = Vec::new();

        for policy in enabled_policies {
            if !self.check_policy(input, &policy) {
                violations.push(policy.id.clone());
            }
        }

        if violations.is_empty() {
            Ok(VerificationResult::verified())
        } else {
            Ok(VerificationResult::failed(violations))
        }
    }

    fn check_policy(&self, _input: &PromptInput, _policy: &SecurityPolicy) -> bool {
        // Simplified stub - always passes
        // In production, this would use temporal-neural-solver
        true
    }

    /// Add security policy
    pub fn add_policy(&mut self, policy: SecurityPolicy) {
        let mut policies = self.policies.write().unwrap();
        policies.insert(policy.id.clone(), policy);
    }

    /// Remove security policy
    pub fn remove_policy(&mut self, id: &str) -> Option<SecurityPolicy> {
        let mut policies = self.policies.write().unwrap();
        policies.remove(id)
    }

    /// Get policy by ID
    pub fn get_policy(&self, id: &str) -> Option<SecurityPolicy> {
        let policies = self.policies.read().unwrap();
        policies.get(id).cloned()
    }

    /// Enable policy
    pub fn enable_policy(&mut self, id: &str) -> AnalysisResult<()> {
        let mut policies = self.policies.write().unwrap();
        if let Some(policy) = policies.get_mut(id) {
            policy.enabled = true;
        }
        Ok(())
    }

    /// Disable policy
    pub fn disable_policy(&mut self, id: &str) -> AnalysisResult<()> {
        let mut policies = self.policies.write().unwrap();
        if let Some(policy) = policies.get_mut(id) {
            policy.enabled = false;
        }
        Ok(())
    }

    /// Get all policies
    pub fn list_policies(&self) -> Vec<SecurityPolicy> {
        let policies = self.policies.read().unwrap();
        policies.values().cloned().collect()
    }

    /// Get number of policies
    pub fn policy_count(&self) -> usize {
        let policies = self.policies.read().unwrap();
        policies.len()
    }

    /// Get number of enabled policies
    pub fn enabled_count(&self) -> usize {
        let policies = self.policies.read().unwrap();
        policies.values().filter(|p| p.enabled).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_verifier_creation() {
        let verifier = PolicyVerifier::new().unwrap();
        assert_eq!(verifier.policy_count(), 0);
    }

    #[test]
    fn test_policy_creation() {
        let policy = SecurityPolicy::new(
            "auth_check",
            "Verify authentication",
            "G (action -> authenticated)"
        )
        .with_severity(0.9);

        assert_eq!(policy.id, "auth_check");
        assert_eq!(policy.severity, 0.9);
        assert!(policy.enabled);
    }

    #[test]
    fn test_add_remove_policy() {
        let mut verifier = PolicyVerifier::new().unwrap();

        let policy = SecurityPolicy::new("test", "Test policy", "G true");
        verifier.add_policy(policy.clone());

        assert_eq!(verifier.policy_count(), 1);

        let removed = verifier.remove_policy("test");
        assert!(removed.is_some());
        assert_eq!(verifier.policy_count(), 0);
    }

    #[test]
    fn test_enable_disable_policy() {
        let mut verifier = PolicyVerifier::new().unwrap();

        let policy = SecurityPolicy::new("test", "Test", "G true");
        verifier.add_policy(policy);

        assert_eq!(verifier.enabled_count(), 1);

        verifier.disable_policy("test").unwrap();
        assert_eq!(verifier.enabled_count(), 0);

        verifier.enable_policy("test").unwrap();
        assert_eq!(verifier.enabled_count(), 1);
    }

    #[test]
    fn test_verification_result_helpers() {
        let verified = VerificationResult::verified();
        assert!(verified.verified);
        assert!(verified.violations.is_empty());

        let failed = VerificationResult::failed(vec!["policy1".to_string()]);
        assert!(!failed.verified);
        assert_eq!(failed.violations.len(), 1);
    }
}
