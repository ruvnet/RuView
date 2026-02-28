//! Policy Bundle Aggregate
//!
//! Implements versioned, signed policy bundles with multi-signature threshold configurations.
//!
//! # Lifecycle
//!
//! 1. **Draft**: Initial creation, can be modified
//! 2. **Pending**: Awaiting required approvals
//! 3. **Active**: Fully approved and immutable
//! 4. **Superseded**: Replaced by a newer version
//! 5. **Revoked**: Explicitly invalidated
//!
//! # Immutability Invariant
//!
//! Once a policy bundle reaches `Active` status, it becomes immutable.
//! Any changes require creating a new version.

use super::{Hash, Timestamp, Version};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

/// Unique identifier for a policy bundle
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolicyBundleId(pub Uuid);

impl PolicyBundleId {
    /// Generate a new random ID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from a UUID
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get as bytes
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for PolicyBundleId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PolicyBundleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Lightweight reference to a policy bundle for embedding in other records
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolicyBundleRef {
    /// Bundle ID
    pub id: PolicyBundleId,
    /// Version at time of reference
    pub version: Version,
    /// Content hash for integrity verification
    pub content_hash: Hash,
}

impl PolicyBundleRef {
    /// Create a reference from a policy bundle
    #[must_use]
    pub fn from_bundle(bundle: &PolicyBundle) -> Self {
        Self {
            id: bundle.id,
            version: bundle.version.clone(),
            content_hash: bundle.content_hash(),
        }
    }

    /// Get as bytes for hashing
    #[must_use]
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(48 + 12);
        bytes.extend_from_slice(self.id.as_bytes());
        bytes.extend_from_slice(&self.version.major.to_le_bytes());
        bytes.extend_from_slice(&self.version.minor.to_le_bytes());
        bytes.extend_from_slice(&self.version.patch.to_le_bytes());
        bytes.extend_from_slice(self.content_hash.as_bytes());
        bytes
    }
}

/// Status of a policy bundle in its lifecycle
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PolicyBundleStatus {
    /// Initial creation, can be modified
    Draft,
    /// Awaiting required approvals
    Pending,
    /// Fully approved and immutable
    Active,
    /// Replaced by a newer version
    Superseded,
    /// Explicitly invalidated
    Revoked,
}

impl PolicyBundleStatus {
    /// Check if the policy is in an editable state
    #[must_use]
    pub const fn is_editable(&self) -> bool {
        matches!(self, Self::Draft)
    }

    /// Check if the policy is currently enforceable
    #[must_use]
    pub const fn is_enforceable(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Check if the policy is in a terminal state
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Superseded | Self::Revoked)
    }
}

/// Unique identifier for an approver (could be a user, service, or key)
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApproverId(pub String);

impl ApproverId {
    /// Create a new approver ID
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get as string slice
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ApproverId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ApproverId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ApproverId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Digital signature for policy approval
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalSignature {
    /// The approver who signed
    pub approver_id: ApproverId,
    /// Timestamp of approval
    pub timestamp: Timestamp,
    /// Signature bytes (format depends on signing algorithm)
    pub signature: Vec<u8>,
    /// Algorithm used (e.g., "ed25519", "secp256k1")
    pub algorithm: String,
    /// Optional comment from approver
    pub comment: Option<String>,
}

impl ApprovalSignature {
    /// Create a new approval signature
    #[must_use]
    pub fn new(approver_id: ApproverId, signature: Vec<u8>, algorithm: impl Into<String>) -> Self {
        Self {
            approver_id,
            timestamp: Timestamp::now(),
            signature,
            algorithm: algorithm.into(),
            comment: None,
        }
    }

    /// Add a comment to the approval
    #[must_use]
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Create a placeholder signature for testing (NOT for production)
    #[must_use]
    pub fn placeholder(approver_id: ApproverId) -> Self {
        Self {
            approver_id,
            timestamp: Timestamp::now(),
            signature: vec![0u8; 64],
            algorithm: "placeholder".to_string(),
            comment: Some("Test signature".to_string()),
        }
    }
}

/// Threshold configuration for a scope
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThresholdConfig {
    /// Energy threshold for Lane 0 (Reflex) - allow without additional checks
    pub reflex: f32,
    /// Energy threshold for Lane 1 (Retrieval) - require evidence fetching
    pub retrieval: f32,
    /// Energy threshold for Lane 2 (Heavy) - require deep reasoning
    pub heavy: f32,
    /// Duration for which incoherence must persist before escalation
    pub persistence_window: Duration,
    /// Optional custom thresholds for specific metrics
    pub custom_thresholds: HashMap<String, f32>,
}

impl ThresholdConfig {
    /// Create a new threshold config with defaults
    #[must_use]
    pub fn new(reflex: f32, retrieval: f32, heavy: f32) -> Self {
        Self {
            reflex,
            retrieval,
            heavy,
            persistence_window: Duration::from_secs(30),
            custom_thresholds: HashMap::new(),
        }
    }

    /// Create a strict threshold config (lower thresholds = more escalations)
    #[must_use]
    pub fn strict() -> Self {
        Self {
            reflex: 0.1,
            retrieval: 0.3,
            heavy: 0.6,
            persistence_window: Duration::from_secs(10),
            custom_thresholds: HashMap::new(),
        }
    }

    /// Create a permissive threshold config (higher thresholds = fewer escalations)
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            reflex: 0.5,
            retrieval: 0.8,
            heavy: 0.95,
            persistence_window: Duration::from_secs(60),
            custom_thresholds: HashMap::new(),
        }
    }

    /// Set a custom threshold
    #[must_use]
    pub fn with_custom(mut self, name: impl Into<String>, value: f32) -> Self {
        self.custom_thresholds.insert(name.into(), value);
        self
    }

    /// Set persistence window
    #[must_use]
    pub const fn with_persistence_window(mut self, window: Duration) -> Self {
        self.persistence_window = window;
        self
    }

    /// Validate threshold ordering (reflex < retrieval < heavy)
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.reflex >= 0.0
            && self.reflex <= self.retrieval
            && self.retrieval <= self.heavy
            && self.heavy <= 1.0
    }
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            reflex: 0.3,
            retrieval: 0.6,
            heavy: 0.9,
            persistence_window: Duration::from_secs(30),
            custom_thresholds: HashMap::new(),
        }
    }
}

/// Rule for automatic escalation under certain conditions
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EscalationRule {
    /// Unique name for this rule
    pub name: String,
    /// Condition expression (simplified DSL)
    pub condition: EscalationCondition,
    /// Target lane to escalate to
    pub target_lane: u8,
    /// Optional notification channels
    pub notify: Vec<String>,
    /// Whether this rule is enabled
    pub enabled: bool,
    /// Priority (lower = higher priority)
    pub priority: u32,
}

impl EscalationRule {
    /// Create a new escalation rule
    #[must_use]
    pub fn new(name: impl Into<String>, condition: EscalationCondition, target_lane: u8) -> Self {
        Self {
            name: name.into(),
            condition,
            target_lane,
            notify: Vec::new(),
            enabled: true,
            priority: 100,
        }
    }

    /// Add a notification channel
    #[must_use]
    pub fn with_notify(mut self, channel: impl Into<String>) -> Self {
        self.notify.push(channel.into());
        self
    }

    /// Set the priority
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Disable the rule
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Condition for triggering an escalation
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EscalationCondition {
    /// Energy exceeds threshold
    EnergyAbove(f32),
    /// Energy persists above threshold for duration
    PersistentEnergy { threshold: f32, duration_secs: u64 },
    /// Spectral drift detected
    SpectralDrift { magnitude: f32 },
    /// Multiple consecutive rejections
    ConsecutiveRejections { count: u32 },
    /// Compound condition (all must be true)
    All(Vec<EscalationCondition>),
    /// Compound condition (any must be true)
    Any(Vec<EscalationCondition>),
}

/// Policy error types
#[derive(Debug, Error)]
pub enum PolicyError {
    /// Policy is not in an editable state
    #[error("Policy is not editable (status: {0:?})")]
    NotEditable(PolicyBundleStatus),

    /// Policy is not active
    #[error("Policy is not active (status: {0:?})")]
    NotActive(PolicyBundleStatus),

    /// Insufficient approvals
    #[error("Insufficient approvals: {current} of {required}")]
    InsufficientApprovals { current: usize, required: usize },

    /// Duplicate approver
    #[error("Duplicate approval from: {0}")]
    DuplicateApprover(ApproverId),

    /// Invalid threshold configuration
    #[error("Invalid threshold configuration: {0}")]
    InvalidThreshold(String),

    /// Scope not found
    #[error("Scope not found: {0}")]
    ScopeNotFound(String),

    /// Policy already exists
    #[error("Policy already exists: {0}")]
    AlreadyExists(PolicyBundleId),

    /// Content hash mismatch
    #[error("Content hash mismatch")]
    HashMismatch,
}

/// Versioned, signed policy bundle for threshold configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolicyBundle {
    /// Unique bundle identifier
    pub id: PolicyBundleId,
    /// Semantic version
    pub version: Version,
    /// Human-readable name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Current lifecycle status
    pub status: PolicyBundleStatus,
    /// Threshold configurations by scope pattern
    pub thresholds: HashMap<String, ThresholdConfig>,
    /// Escalation rules
    pub escalation_rules: Vec<EscalationRule>,
    /// Approvals collected
    pub approvals: Vec<ApprovalSignature>,
    /// Minimum required approvals for activation
    pub required_approvals: usize,
    /// Allowed approvers (if empty, any approver is valid)
    pub allowed_approvers: Vec<ApproverId>,
    /// Creation timestamp
    pub created_at: Timestamp,
    /// Last modification timestamp
    pub updated_at: Timestamp,
    /// Optional reference to superseded bundle
    pub supersedes: Option<PolicyBundleId>,
    /// Activation timestamp (when status became Active)
    pub activated_at: Option<Timestamp>,
    /// Cached content hash (recomputed on access if None)
    #[serde(skip)]
    cached_hash: Option<Hash>,
}

impl PolicyBundle {
    /// Create a new policy bundle in Draft status
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let now = Timestamp::now();
        Self {
            id: PolicyBundleId::new(),
            version: Version::initial(),
            name: name.into(),
            description: None,
            status: PolicyBundleStatus::Draft,
            thresholds: HashMap::new(),
            escalation_rules: Vec::new(),
            approvals: Vec::new(),
            required_approvals: 1,
            allowed_approvers: Vec::new(),
            created_at: now,
            updated_at: now,
            supersedes: None,
            activated_at: None,
            cached_hash: None,
        }
    }

    /// Compute the content hash of this bundle
    #[must_use]
    pub fn content_hash(&self) -> Hash {
        let mut hasher = blake3::Hasher::new();

        // Hash identifying fields
        hasher.update(self.id.as_bytes());
        hasher.update(&self.version.major.to_le_bytes());
        hasher.update(&self.version.minor.to_le_bytes());
        hasher.update(&self.version.patch.to_le_bytes());
        hasher.update(self.name.as_bytes());

        // Hash thresholds (sorted for determinism)
        let mut scope_keys: Vec<_> = self.thresholds.keys().collect();
        scope_keys.sort();
        for key in scope_keys {
            hasher.update(key.as_bytes());
            if let Some(config) = self.thresholds.get(key) {
                hasher.update(&config.reflex.to_le_bytes());
                hasher.update(&config.retrieval.to_le_bytes());
                hasher.update(&config.heavy.to_le_bytes());
                hasher.update(&config.persistence_window.as_secs().to_le_bytes());
            }
        }

        // Hash escalation rules
        for rule in &self.escalation_rules {
            hasher.update(rule.name.as_bytes());
            hasher.update(&rule.target_lane.to_le_bytes());
            hasher.update(&rule.priority.to_le_bytes());
        }

        // Hash governance params
        hasher.update(&self.required_approvals.to_le_bytes());

        Hash::from_blake3(hasher.finalize())
    }

    /// Get a reference to this bundle
    #[must_use]
    pub fn reference(&self) -> PolicyBundleRef {
        PolicyBundleRef::from_bundle(self)
    }

    /// Add a threshold configuration for a scope
    ///
    /// # Errors
    ///
    /// Returns error if policy is not editable or threshold is invalid
    pub fn add_threshold(
        &mut self,
        scope: impl Into<String>,
        config: ThresholdConfig,
    ) -> Result<(), PolicyError> {
        if !self.status.is_editable() {
            return Err(PolicyError::NotEditable(self.status));
        }

        if !config.is_valid() {
            return Err(PolicyError::InvalidThreshold(
                "Thresholds must be ordered: reflex <= retrieval <= heavy".to_string(),
            ));
        }

        self.thresholds.insert(scope.into(), config);
        self.updated_at = Timestamp::now();
        self.cached_hash = None;
        Ok(())
    }

    /// Add an escalation rule
    ///
    /// # Errors
    ///
    /// Returns error if policy is not editable
    pub fn add_escalation_rule(&mut self, rule: EscalationRule) -> Result<(), PolicyError> {
        if !self.status.is_editable() {
            return Err(PolicyError::NotEditable(self.status));
        }

        self.escalation_rules.push(rule);
        self.escalation_rules.sort_by_key(|r| r.priority);
        self.updated_at = Timestamp::now();
        self.cached_hash = None;
        Ok(())
    }

    /// Get threshold config for a scope (with fallback to "default")
    #[must_use]
    pub fn get_threshold(&self, scope: &str) -> Option<&ThresholdConfig> {
        self.thresholds
            .get(scope)
            .or_else(|| self.thresholds.get("default"))
    }

    /// Set the number of required approvals
    ///
    /// # Errors
    ///
    /// Returns error if policy is not editable
    pub fn set_required_approvals(&mut self, count: usize) -> Result<(), PolicyError> {
        if !self.status.is_editable() {
            return Err(PolicyError::NotEditable(self.status));
        }

        self.required_approvals = count;
        self.updated_at = Timestamp::now();
        Ok(())
    }

    /// Add an allowed approver
    ///
    /// # Errors
    ///
    /// Returns error if policy is not editable
    pub fn add_allowed_approver(&mut self, approver: ApproverId) -> Result<(), PolicyError> {
        if !self.status.is_editable() {
            return Err(PolicyError::NotEditable(self.status));
        }

        if !self.allowed_approvers.contains(&approver) {
            self.allowed_approvers.push(approver);
            self.updated_at = Timestamp::now();
        }
        Ok(())
    }

    /// Submit the bundle for approval (Draft -> Pending)
    ///
    /// # Errors
    ///
    /// Returns error if not in Draft status
    pub fn submit_for_approval(&mut self) -> Result<(), PolicyError> {
        if self.status != PolicyBundleStatus::Draft {
            return Err(PolicyError::NotEditable(self.status));
        }

        self.status = PolicyBundleStatus::Pending;
        self.updated_at = Timestamp::now();
        Ok(())
    }

    /// Add an approval signature
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Policy is not pending
    /// - Approver is not allowed
    /// - Approver has already signed
    pub fn add_approval(&mut self, approval: ApprovalSignature) -> Result<(), PolicyError> {
        if self.status != PolicyBundleStatus::Pending {
            return Err(PolicyError::NotEditable(self.status));
        }

        // Check if approver is allowed (if list is not empty)
        if !self.allowed_approvers.is_empty()
            && !self.allowed_approvers.contains(&approval.approver_id)
        {
            return Err(PolicyError::DuplicateApprover(approval.approver_id));
        }

        // Check for duplicate
        if self
            .approvals
            .iter()
            .any(|a| a.approver_id == approval.approver_id)
        {
            return Err(PolicyError::DuplicateApprover(approval.approver_id));
        }

        self.approvals.push(approval);
        self.updated_at = Timestamp::now();

        // Auto-activate if we have enough approvals
        if self.approvals.len() >= self.required_approvals {
            self.status = PolicyBundleStatus::Active;
            self.activated_at = Some(Timestamp::now());
        }

        Ok(())
    }

    /// Check if the bundle has sufficient approvals
    #[must_use]
    pub fn has_sufficient_approvals(&self) -> bool {
        self.approvals.len() >= self.required_approvals
    }

    /// Force activation (for testing or emergency)
    ///
    /// # Errors
    ///
    /// Returns error if already active or insufficient approvals
    pub fn activate(&mut self) -> Result<(), PolicyError> {
        if self.status == PolicyBundleStatus::Active {
            return Ok(());
        }

        if !self.has_sufficient_approvals() {
            return Err(PolicyError::InsufficientApprovals {
                current: self.approvals.len(),
                required: self.required_approvals,
            });
        }

        self.status = PolicyBundleStatus::Active;
        self.activated_at = Some(Timestamp::now());
        self.updated_at = Timestamp::now();
        Ok(())
    }

    /// Mark this bundle as superseded by another
    ///
    /// # Errors
    ///
    /// Returns error if not active
    pub fn supersede(&mut self, successor_id: PolicyBundleId) -> Result<(), PolicyError> {
        if self.status != PolicyBundleStatus::Active {
            return Err(PolicyError::NotActive(self.status));
        }

        self.status = PolicyBundleStatus::Superseded;
        self.updated_at = Timestamp::now();
        // Note: supersedes field is on the successor, not here
        Ok(())
    }

    /// Revoke this bundle (emergency invalidation)
    ///
    /// # Errors
    ///
    /// Returns error if already in terminal state
    pub fn revoke(&mut self) -> Result<(), PolicyError> {
        if self.status.is_terminal() {
            return Err(PolicyError::NotEditable(self.status));
        }

        self.status = PolicyBundleStatus::Revoked;
        self.updated_at = Timestamp::now();
        Ok(())
    }

    /// Create a new version based on this bundle
    #[must_use]
    pub fn create_new_version(&self) -> Self {
        let now = Timestamp::now();
        Self {
            id: PolicyBundleId::new(),
            version: self.version.clone().bump_minor(),
            name: self.name.clone(),
            description: self.description.clone(),
            status: PolicyBundleStatus::Draft,
            thresholds: self.thresholds.clone(),
            escalation_rules: self.escalation_rules.clone(),
            approvals: Vec::new(),
            required_approvals: self.required_approvals,
            allowed_approvers: self.allowed_approvers.clone(),
            created_at: now,
            updated_at: now,
            supersedes: Some(self.id),
            activated_at: None,
            cached_hash: None,
        }
    }
}

/// Builder for creating policy bundles
#[derive(Default)]
pub struct PolicyBundleBuilder {
    name: Option<String>,
    description: Option<String>,
    thresholds: HashMap<String, ThresholdConfig>,
    escalation_rules: Vec<EscalationRule>,
    required_approvals: usize,
    allowed_approvers: Vec<ApproverId>,
}

impl PolicyBundleBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
            thresholds: HashMap::new(),
            escalation_rules: Vec::new(),
            required_approvals: 1,
            allowed_approvers: Vec::new(),
        }
    }

    /// Set the policy name
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the description
    #[must_use]
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a threshold configuration
    #[must_use]
    pub fn with_threshold(mut self, scope: impl Into<String>, config: ThresholdConfig) -> Self {
        self.thresholds.insert(scope.into(), config);
        self
    }

    /// Add an escalation rule
    #[must_use]
    pub fn with_escalation_rule(mut self, rule: EscalationRule) -> Self {
        self.escalation_rules.push(rule);
        self
    }

    /// Set required approvals
    #[must_use]
    pub const fn with_required_approvals(mut self, count: usize) -> Self {
        self.required_approvals = count;
        self
    }

    /// Add an allowed approver
    #[must_use]
    pub fn with_approver(mut self, approver: ApproverId) -> Self {
        self.allowed_approvers.push(approver);
        self
    }

    /// Build the policy bundle
    ///
    /// # Errors
    ///
    /// Returns error if name is not set or thresholds are invalid
    pub fn build(self) -> Result<PolicyBundle, PolicyError> {
        let name = self
            .name
            .ok_or_else(|| PolicyError::InvalidThreshold("Policy name is required".to_string()))?;

        // Validate all thresholds
        for (scope, config) in &self.thresholds {
            if !config.is_valid() {
                return Err(PolicyError::InvalidThreshold(format!(
                    "Invalid threshold for scope '{scope}'"
                )));
            }
        }

        let now = Timestamp::now();
        Ok(PolicyBundle {
            id: PolicyBundleId::new(),
            version: Version::initial(),
            name,
            description: self.description,
            status: PolicyBundleStatus::Draft,
            thresholds: self.thresholds,
            escalation_rules: self.escalation_rules,
            approvals: Vec::new(),
            required_approvals: self.required_approvals,
            allowed_approvers: self.allowed_approvers,
            created_at: now,
            updated_at: now,
            supersedes: None,
            activated_at: None,
            cached_hash: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_bundle_creation() {
        let policy = PolicyBundle::new("test-policy");
        assert_eq!(policy.name, "test-policy");
        assert_eq!(policy.status, PolicyBundleStatus::Draft);
        assert!(policy.status.is_editable());
    }

    #[test]
    fn test_threshold_config_validation() {
        let valid = ThresholdConfig::new(0.3, 0.6, 0.9);
        assert!(valid.is_valid());

        let invalid = ThresholdConfig::new(0.9, 0.6, 0.3); // Wrong order
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_policy_lifecycle() -> Result<(), PolicyError> {
        let mut policy = PolicyBundle::new("test");
        policy.add_threshold("default", ThresholdConfig::default())?;
        policy.set_required_approvals(2)?;

        // Submit for approval
        policy.submit_for_approval()?;
        assert_eq!(policy.status, PolicyBundleStatus::Pending);

        // Add approvals
        policy.add_approval(ApprovalSignature::placeholder(ApproverId::new("approver1")))?;
        assert_eq!(policy.status, PolicyBundleStatus::Pending); // Still pending

        policy.add_approval(ApprovalSignature::placeholder(ApproverId::new("approver2")))?;
        assert_eq!(policy.status, PolicyBundleStatus::Active); // Auto-activated

        Ok(())
    }

    #[test]
    fn test_duplicate_approver_rejected() -> Result<(), PolicyError> {
        let mut policy = PolicyBundle::new("test");
        // Require 2 approvals so policy stays pending after first approval
        policy.set_required_approvals(2)?;
        policy.submit_for_approval()?;

        let approver = ApproverId::new("same-approver");
        policy.add_approval(ApprovalSignature::placeholder(approver.clone()))?;

        // Second approval from same approver should fail
        let result = policy.add_approval(ApprovalSignature::placeholder(approver));
        assert!(matches!(result, Err(PolicyError::DuplicateApprover(_))));

        Ok(())
    }

    #[test]
    fn test_immutability_after_activation() -> Result<(), PolicyError> {
        let mut policy = PolicyBundle::new("test");
        policy.submit_for_approval()?;
        policy.add_approval(ApprovalSignature::placeholder(ApproverId::new("approver")))?;

        assert_eq!(policy.status, PolicyBundleStatus::Active);

        // Trying to modify should fail
        let result = policy.add_threshold("new-scope", ThresholdConfig::default());
        assert!(matches!(result, Err(PolicyError::NotEditable(_))));

        Ok(())
    }

    #[test]
    fn test_content_hash_determinism() {
        let mut policy1 = PolicyBundle::new("test");
        let _ = policy1.add_threshold("scope1", ThresholdConfig::default());

        let mut policy2 = PolicyBundle::new("test");
        let _ = policy2.add_threshold("scope1", ThresholdConfig::default());

        // Same content should produce same hash (ignoring ID)
        // Note: IDs are different, so hashes will differ
        // But hashing the same bundle twice should be deterministic
        let hash1 = policy1.content_hash();
        let hash2 = policy1.content_hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_builder() -> Result<(), PolicyError> {
        let policy = PolicyBundleBuilder::new()
            .name("my-policy")
            .description("A test policy")
            .with_threshold("default", ThresholdConfig::default())
            .with_threshold("strict", ThresholdConfig::strict())
            .with_required_approvals(2)
            .with_approver(ApproverId::new("admin1"))
            .with_approver(ApproverId::new("admin2"))
            .build()?;

        assert_eq!(policy.name, "my-policy");
        assert_eq!(policy.thresholds.len(), 2);
        assert_eq!(policy.required_approvals, 2);
        assert_eq!(policy.allowed_approvers.len(), 2);

        Ok(())
    }

    #[test]
    fn test_new_version_creation() -> Result<(), PolicyError> {
        let mut original = PolicyBundle::new("test");
        original.add_threshold("default", ThresholdConfig::default())?;
        original.submit_for_approval()?;
        original.add_approval(ApprovalSignature::placeholder(ApproverId::new("approver")))?;

        let new_version = original.create_new_version();

        assert_ne!(new_version.id, original.id);
        assert_eq!(new_version.supersedes, Some(original.id));
        assert_eq!(new_version.version, Version::new(1, 1, 0));
        assert_eq!(new_version.status, PolicyBundleStatus::Draft);
        assert!(new_version.approvals.is_empty());

        Ok(())
    }
}
