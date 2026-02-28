//! Tenant Registry for RuVector Multi-Tenancy
//!
//! Provides tenant management with isolation levels, quotas, and metadata.
//! Integrates with PostgreSQL's system tables for persistent storage.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Maximum number of tenants in shared memory (for fixed-size arrays)
pub const MAX_TENANTS: usize = 10_000;

/// Isolation level for tenant data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum IsolationLevel {
    /// Shared index with tenant filter - most memory efficient
    Shared = 0,
    /// Dedicated partition within shared index structure
    Partition = 1,
    /// Completely separate physical index - maximum isolation
    Dedicated = 2,
}

impl IsolationLevel {
    /// Parse isolation level from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "shared" => Some(Self::Shared),
            "partition" => Some(Self::Partition),
            "dedicated" => Some(Self::Dedicated),
            _ => None,
        }
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Partition => "partition",
            Self::Dedicated => "dedicated",
        }
    }

    /// Get recommended vector count threshold for this level
    pub fn recommended_vector_count(&self) -> u64 {
        match self {
            Self::Shared => 100_000,       // < 100K vectors
            Self::Partition => 10_000_000, // 100K - 10M vectors
            Self::Dedicated => u64::MAX,   // > 10M vectors
        }
    }
}

impl Default for IsolationLevel {
    fn default() -> Self {
        Self::Shared
    }
}

/// Tenant quota configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantQuota {
    /// Maximum number of vectors
    pub max_vectors: u64,
    /// Maximum storage in bytes
    pub max_storage_bytes: u64,
    /// Maximum queries per second
    pub max_qps: u32,
    /// Maximum concurrent queries
    pub max_concurrent: u32,
    /// Maximum collections
    pub max_collections: u32,
}

impl Default for TenantQuota {
    fn default() -> Self {
        Self {
            max_vectors: 1_000_000,
            max_storage_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
            max_qps: 100,
            max_concurrent: 10,
            max_collections: 10,
        }
    }
}

/// Tenant configuration and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantConfig {
    /// Unique tenant identifier
    pub id: String,
    /// Display name
    pub display_name: Option<String>,
    /// Isolation level
    pub isolation_level: IsolationLevel,
    /// Resource quotas
    pub quota: TenantQuota,
    /// Whether integrity monitoring is enabled
    pub integrity_enabled: bool,
    /// Custom integrity policy ID
    pub integrity_policy_id: Option<i64>,
    /// Arbitrary metadata
    pub metadata: serde_json::Value,
    /// Creation timestamp (epoch millis)
    pub created_at: i64,
    /// Suspension timestamp (None = active)
    pub suspended_at: Option<i64>,
}

impl TenantConfig {
    /// Create a new tenant with default settings
    pub fn new(id: String) -> Self {
        Self {
            id,
            display_name: None,
            isolation_level: IsolationLevel::default(),
            quota: TenantQuota::default(),
            integrity_enabled: true,
            integrity_policy_id: None,
            metadata: serde_json::json!({}),
            created_at: chrono_now_millis(),
            suspended_at: None,
        }
    }

    /// Create tenant from JSONB configuration
    pub fn from_json(id: String, config: &serde_json::Value) -> Self {
        let mut tenant = Self::new(id);

        if let Some(name) = config.get("display_name").and_then(|v| v.as_str()) {
            tenant.display_name = Some(name.to_string());
        }

        if let Some(level) = config.get("isolation_level").and_then(|v| v.as_str()) {
            tenant.isolation_level = IsolationLevel::from_str(level).unwrap_or_default();
        }

        if let Some(max_vec) = config.get("max_vectors").and_then(|v| v.as_u64()) {
            tenant.quota.max_vectors = max_vec;
        }

        if let Some(max_qps) = config.get("max_qps").and_then(|v| v.as_u64()) {
            tenant.quota.max_qps = max_qps as u32;
        }

        if let Some(max_storage) = config.get("max_storage_gb").and_then(|v| v.as_f64()) {
            tenant.quota.max_storage_bytes = (max_storage * 1024.0 * 1024.0 * 1024.0) as u64;
        }

        if let Some(enabled) = config.get("integrity_enabled").and_then(|v| v.as_bool()) {
            tenant.integrity_enabled = enabled;
        }

        if let Some(meta) = config.get("metadata") {
            tenant.metadata = meta.clone();
        }

        tenant
    }

    /// Check if tenant is suspended
    pub fn is_suspended(&self) -> bool {
        self.suspended_at.is_some()
    }

    /// Check if tenant is active
    pub fn is_active(&self) -> bool {
        !self.is_suspended()
    }
}

/// Shared state for a tenant (in shared memory)
#[repr(C)]
pub struct TenantSharedState {
    /// Hash of tenant ID for fast lookup
    pub tenant_id_hash: AtomicU64,
    /// Current integrity state (0=normal, 1=stress, 2=critical)
    pub integrity_state: AtomicU32,
    /// Current lambda cut value (fixed-point: value * 1000)
    pub lambda_cut_fp: AtomicU32,
    /// Request count for rate limiting
    pub request_count: AtomicU32,
    /// Last request epoch (seconds)
    pub last_request_epoch: AtomicU64,
    /// Flags (bit 0: suspended, bit 1: migrating, etc.)
    pub flags: AtomicU32,
}

impl TenantSharedState {
    /// Create new shared state
    pub fn new(tenant_id_hash: u64) -> Self {
        Self {
            tenant_id_hash: AtomicU64::new(tenant_id_hash),
            integrity_state: AtomicU32::new(0),
            lambda_cut_fp: AtomicU32::new(1000), // 1.0 in fixed point
            request_count: AtomicU32::new(0),
            last_request_epoch: AtomicU64::new(0),
            flags: AtomicU32::new(0),
        }
    }

    /// Reset shared state for a new tenant (atomically reinitialize fields)
    pub fn reset(&self, tenant_id_hash: u64) {
        self.tenant_id_hash.store(tenant_id_hash, Ordering::Relaxed);
        self.integrity_state.store(0, Ordering::Relaxed);
        self.lambda_cut_fp.store(1000, Ordering::Relaxed); // 1.0 in fixed point
        self.request_count.store(0, Ordering::Relaxed);
        self.last_request_epoch.store(0, Ordering::Relaxed);
        self.flags.store(0, Ordering::Relaxed);
    }

    /// Check if tenant is suspended
    pub fn is_suspended(&self) -> bool {
        (self.flags.load(Ordering::Relaxed) & 1) != 0
    }

    /// Set suspended flag
    pub fn set_suspended(&self, suspended: bool) {
        if suspended {
            self.flags.fetch_or(1, Ordering::Relaxed);
        } else {
            self.flags.fetch_and(!1, Ordering::Relaxed);
        }
    }

    /// Check if tenant is migrating
    pub fn is_migrating(&self) -> bool {
        (self.flags.load(Ordering::Relaxed) & 2) != 0
    }

    /// Set migrating flag
    pub fn set_migrating(&self, migrating: bool) {
        if migrating {
            self.flags.fetch_or(2, Ordering::Relaxed);
        } else {
            self.flags.fetch_and(!2, Ordering::Relaxed);
        }
    }

    /// Get lambda cut as f32
    pub fn lambda_cut(&self) -> f32 {
        self.lambda_cut_fp.load(Ordering::Relaxed) as f32 / 1000.0
    }

    /// Set lambda cut from f32
    pub fn set_lambda_cut(&self, value: f32) {
        self.lambda_cut_fp
            .store((value * 1000.0) as u32, Ordering::Relaxed);
    }

    /// Increment request count and check rate limit
    pub fn check_rate_limit(&self, max_qps: u32) -> bool {
        let now = current_epoch_seconds();
        let last_epoch = self.last_request_epoch.load(Ordering::Relaxed);

        if now > last_epoch {
            // New second, reset counter
            self.last_request_epoch.store(now, Ordering::Relaxed);
            self.request_count.store(1, Ordering::Relaxed);
            true
        } else {
            // Same second, increment and check
            let count = self.request_count.fetch_add(1, Ordering::Relaxed);
            count < max_qps
        }
    }
}

/// Tenant Registry - manages all tenants
pub struct TenantRegistry {
    /// Tenant configurations (heap-based for flexibility)
    configs: DashMap<String, TenantConfig>,
    /// Tenant ID to index mapping for shared memory lookup
    id_to_index: DashMap<String, usize>,
    /// Shared states (fixed-size for shared memory compatibility)
    shared_states: Vec<TenantSharedState>,
    /// Next available index
    next_index: AtomicU32,
    /// Promotion policy configuration
    promotion_policy: RwLock<PromotionPolicy>,
}

/// Policy for automatic tenant isolation level promotion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionPolicy {
    /// Vector count threshold to promote from shared to partition
    pub partition_threshold: u64,
    /// Vector count threshold to promote from partition to dedicated
    pub dedicated_threshold: u64,
    /// Check interval in seconds
    pub check_interval_secs: u64,
    /// Whether auto-promotion is enabled
    pub enabled: bool,
}

impl Default for PromotionPolicy {
    fn default() -> Self {
        Self {
            partition_threshold: 100_000,
            dedicated_threshold: 10_000_000,
            check_interval_secs: 3600, // 1 hour
            enabled: true,
        }
    }
}

impl TenantRegistry {
    /// Create a new tenant registry
    pub fn new() -> Self {
        let mut shared_states = Vec::with_capacity(MAX_TENANTS);
        for _ in 0..MAX_TENANTS {
            shared_states.push(TenantSharedState::new(0));
        }

        Self {
            configs: DashMap::new(),
            id_to_index: DashMap::new(),
            shared_states,
            next_index: AtomicU32::new(0),
            promotion_policy: RwLock::new(PromotionPolicy::default()),
        }
    }

    /// Register a new tenant
    pub fn register(&self, config: TenantConfig) -> Result<usize, TenantError> {
        let tenant_id = config.id.clone();

        // Check if tenant already exists
        if self.configs.contains_key(&tenant_id) {
            return Err(TenantError::AlreadyExists(tenant_id));
        }

        // Allocate index
        let index = self.next_index.fetch_add(1, Ordering::Relaxed) as usize;
        if index >= MAX_TENANTS {
            return Err(TenantError::MaxTenantsReached);
        }

        // Initialize shared state (atomically reset the pre-allocated slot)
        let id_hash = hash_tenant_id(&tenant_id);
        self.shared_states[index].reset(id_hash);

        // Store mappings
        self.id_to_index.insert(tenant_id.clone(), index);
        self.configs.insert(tenant_id, config);

        Ok(index)
    }

    /// Get tenant configuration
    pub fn get(&self, tenant_id: &str) -> Option<TenantConfig> {
        self.configs.get(tenant_id).map(|r| r.value().clone())
    }

    /// Get tenant shared state
    pub fn get_shared_state(&self, tenant_id: &str) -> Option<&TenantSharedState> {
        self.id_to_index
            .get(tenant_id)
            .map(|idx| &self.shared_states[*idx.value()])
    }

    /// Update tenant configuration
    pub fn update(&self, tenant_id: &str, config: TenantConfig) -> Result<(), TenantError> {
        if !self.configs.contains_key(tenant_id) {
            return Err(TenantError::NotFound(tenant_id.to_string()));
        }
        self.configs.insert(tenant_id.to_string(), config);
        Ok(())
    }

    /// Suspend a tenant
    pub fn suspend(&self, tenant_id: &str) -> Result<(), TenantError> {
        let mut config = self
            .get(tenant_id)
            .ok_or_else(|| TenantError::NotFound(tenant_id.to_string()))?;

        config.suspended_at = Some(chrono_now_millis());
        self.configs.insert(tenant_id.to_string(), config);

        // Update shared state
        if let Some(state) = self.get_shared_state(tenant_id) {
            state.set_suspended(true);
        }

        Ok(())
    }

    /// Resume a suspended tenant
    pub fn resume(&self, tenant_id: &str) -> Result<(), TenantError> {
        let mut config = self
            .get(tenant_id)
            .ok_or_else(|| TenantError::NotFound(tenant_id.to_string()))?;

        config.suspended_at = None;
        self.configs.insert(tenant_id.to_string(), config);

        // Update shared state
        if let Some(state) = self.get_shared_state(tenant_id) {
            state.set_suspended(false);
        }

        Ok(())
    }

    /// Delete a tenant (soft delete by default)
    pub fn delete(&self, tenant_id: &str, hard: bool) -> Result<(), TenantError> {
        if !self.configs.contains_key(tenant_id) {
            return Err(TenantError::NotFound(tenant_id.to_string()));
        }

        if hard {
            // Hard delete: remove immediately
            self.configs.remove(tenant_id);
            self.id_to_index.remove(tenant_id);
            // Note: shared state index remains allocated (could implement recycling)
        } else {
            // Soft delete: mark as suspended with deletion flag
            let mut config = self.get(tenant_id).unwrap();
            config.suspended_at = Some(chrono_now_millis());
            config.metadata["deleted"] = serde_json::json!(true);
            self.configs.insert(tenant_id.to_string(), config);
        }

        Ok(())
    }

    /// List all tenants
    pub fn list(&self) -> Vec<TenantConfig> {
        self.configs.iter().map(|r| r.value().clone()).collect()
    }

    /// List active tenants only
    pub fn list_active(&self) -> Vec<TenantConfig> {
        self.configs
            .iter()
            .filter(|r| r.value().is_active())
            .map(|r| r.value().clone())
            .collect()
    }

    /// Get tenant count
    pub fn count(&self) -> usize {
        self.configs.len()
    }

    /// Validate tenant context for operations
    pub fn validate_context(&self, tenant_id: &str) -> Result<TenantConfig, TenantError> {
        if tenant_id.is_empty() {
            return Err(TenantError::NoContext);
        }

        // Special wildcard for admin operations
        if tenant_id == "*" {
            // Check if caller has admin privileges (would check PostgreSQL roles)
            return Err(TenantError::AdminContextRequired);
        }

        let config = self
            .get(tenant_id)
            .ok_or_else(|| TenantError::NotFound(tenant_id.to_string()))?;

        if config.is_suspended() {
            return Err(TenantError::Suspended(tenant_id.to_string()));
        }

        Ok(config)
    }

    /// Check rate limit for tenant
    pub fn check_rate_limit(&self, tenant_id: &str) -> Result<bool, TenantError> {
        let config = self
            .get(tenant_id)
            .ok_or_else(|| TenantError::NotFound(tenant_id.to_string()))?;

        let state = self
            .get_shared_state(tenant_id)
            .ok_or_else(|| TenantError::NotFound(tenant_id.to_string()))?;

        Ok(state.check_rate_limit(config.quota.max_qps))
    }

    /// Get promotion policy
    pub fn get_promotion_policy(&self) -> PromotionPolicy {
        self.promotion_policy.read().clone()
    }

    /// Set promotion policy
    pub fn set_promotion_policy(&self, policy: PromotionPolicy) {
        *self.promotion_policy.write() = policy;
    }

    /// Check if tenant should be promoted to higher isolation level
    pub fn check_promotion(&self, tenant_id: &str, vector_count: u64) -> Option<IsolationLevel> {
        let config = self.get(tenant_id)?;
        let policy = self.promotion_policy.read();

        if !policy.enabled {
            return None;
        }

        match config.isolation_level {
            IsolationLevel::Shared if vector_count > policy.dedicated_threshold => {
                Some(IsolationLevel::Dedicated)
            }
            IsolationLevel::Shared if vector_count > policy.partition_threshold => {
                Some(IsolationLevel::Partition)
            }
            IsolationLevel::Partition if vector_count > policy.dedicated_threshold => {
                Some(IsolationLevel::Dedicated)
            }
            _ => None,
        }
    }
}

impl Default for TenantRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Tenant operation errors
#[derive(Debug, Clone)]
pub enum TenantError {
    /// Tenant already exists
    AlreadyExists(String),
    /// Tenant not found
    NotFound(String),
    /// Tenant is suspended
    Suspended(String),
    /// No tenant context set
    NoContext,
    /// Admin context required for operation
    AdminContextRequired,
    /// Maximum number of tenants reached
    MaxTenantsReached,
    /// Rate limit exceeded
    RateLimitExceeded(String),
    /// Quota exceeded
    QuotaExceeded(String, String),
    /// Tenant mismatch (security violation)
    TenantMismatch { context: String, request: String },
    /// Invalid tenant ID format (validation error)
    InvalidId(String),
}

impl std::fmt::Display for TenantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyExists(id) => write!(f, "Tenant '{}' already exists", id),
            Self::NotFound(id) => write!(f, "Tenant '{}' not found", id),
            Self::Suspended(id) => write!(f, "Tenant '{}' is suspended", id),
            Self::NoContext => write!(f, "No tenant context set (use SET ruvector.tenant_id)"),
            Self::AdminContextRequired => write!(f, "Admin context required for this operation"),
            Self::MaxTenantsReached => write!(f, "Maximum number of tenants reached"),
            Self::RateLimitExceeded(id) => write!(f, "Rate limit exceeded for tenant '{}'", id),
            Self::QuotaExceeded(id, resource) => {
                write!(f, "Quota exceeded for tenant '{}': {}", id, resource)
            }
            Self::TenantMismatch { context, request } => {
                write!(
                    f,
                    "Tenant mismatch: context='{}', request='{}'",
                    context, request
                )
            }
            Self::InvalidId(msg) => write!(f, "Invalid tenant ID: {}", msg),
        }
    }
}

impl std::error::Error for TenantError {}

/// Global tenant registry instance
static TENANT_REGISTRY: once_cell::sync::Lazy<TenantRegistry> =
    once_cell::sync::Lazy::new(TenantRegistry::new);

/// Get the global tenant registry
pub fn get_registry() -> &'static TenantRegistry {
    &TENANT_REGISTRY
}

// Helper functions

/// Hash a tenant ID for fast lookup
fn hash_tenant_id(id: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}

/// Get current timestamp in milliseconds
fn chrono_now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Get current epoch in seconds
fn current_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_level_parse() {
        assert_eq!(
            IsolationLevel::from_str("shared"),
            Some(IsolationLevel::Shared)
        );
        assert_eq!(
            IsolationLevel::from_str("partition"),
            Some(IsolationLevel::Partition)
        );
        assert_eq!(
            IsolationLevel::from_str("dedicated"),
            Some(IsolationLevel::Dedicated)
        );
        assert_eq!(IsolationLevel::from_str("invalid"), None);
    }

    #[test]
    fn test_tenant_config_from_json() {
        let json = serde_json::json!({
            "display_name": "Test Corp",
            "isolation_level": "dedicated",
            "max_vectors": 5000000,
            "max_qps": 200,
            "integrity_enabled": false
        });

        let config = TenantConfig::from_json("test-tenant".to_string(), &json);
        assert_eq!(config.display_name, Some("Test Corp".to_string()));
        assert_eq!(config.isolation_level, IsolationLevel::Dedicated);
        assert_eq!(config.quota.max_vectors, 5000000);
        assert_eq!(config.quota.max_qps, 200);
        assert!(!config.integrity_enabled);
    }

    #[test]
    fn test_tenant_registry_register() {
        let registry = TenantRegistry::new();
        let config = TenantConfig::new("test-tenant".to_string());

        let result = registry.register(config.clone());
        assert!(result.is_ok());

        // Should fail for duplicate
        let result2 = registry.register(config);
        assert!(matches!(result2, Err(TenantError::AlreadyExists(_))));
    }

    #[test]
    fn test_tenant_suspension() {
        let registry = TenantRegistry::new();
        let config = TenantConfig::new("test-tenant".to_string());
        registry.register(config).unwrap();

        // Suspend
        registry.suspend("test-tenant").unwrap();
        let config = registry.get("test-tenant").unwrap();
        assert!(config.is_suspended());

        // Resume
        registry.resume("test-tenant").unwrap();
        let config = registry.get("test-tenant").unwrap();
        assert!(!config.is_suspended());
    }

    #[test]
    fn test_shared_state_rate_limiting() {
        let state = TenantSharedState::new(12345);

        // First request should pass
        assert!(state.check_rate_limit(10));

        // Subsequent requests within limit should pass
        for _ in 0..8 {
            assert!(state.check_rate_limit(10));
        }

        // 10th request should fail (at limit)
        assert!(!state.check_rate_limit(10));
    }

    #[test]
    fn test_promotion_check() {
        let registry = TenantRegistry::new();
        let mut config = TenantConfig::new("test-tenant".to_string());
        config.isolation_level = IsolationLevel::Shared;
        registry.register(config).unwrap();

        // Below threshold - no promotion
        assert!(registry.check_promotion("test-tenant", 50_000).is_none());

        // Above partition threshold
        assert_eq!(
            registry.check_promotion("test-tenant", 500_000),
            Some(IsolationLevel::Partition)
        );

        // Above dedicated threshold (jumps directly)
        assert_eq!(
            registry.check_promotion("test-tenant", 15_000_000),
            Some(IsolationLevel::Dedicated)
        );
    }
}
