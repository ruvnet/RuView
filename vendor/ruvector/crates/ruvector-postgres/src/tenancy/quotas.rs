//! Resource Quotas for RuVector Multi-Tenancy
//!
//! Provides per-tenant resource limits and enforcement:
//! - Vector count limits
//! - Storage limits (bytes)
//! - Query rate limiting (QPS)
//! - Concurrent query limits
//! - Background worker allocation

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use super::registry::{get_registry, TenantQuota};

/// Current resource usage for a tenant
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TenantUsage {
    /// Current vector count
    pub vector_count: u64,
    /// Current storage in bytes
    pub storage_bytes: u64,
    /// Queries in the current rate window
    pub queries_this_second: u32,
    /// Current concurrent queries
    pub concurrent_queries: u32,
    /// Collections count
    pub collection_count: u32,
    /// Last updated timestamp (epoch millis)
    pub last_updated: i64,
}

impl TenantUsage {
    /// Create new empty usage
    pub fn new() -> Self {
        Self {
            vector_count: 0,
            storage_bytes: 0,
            queries_this_second: 0,
            concurrent_queries: 0,
            collection_count: 0,
            last_updated: chrono_now_millis(),
        }
    }

    /// Calculate storage in GB
    pub fn storage_gb(&self) -> f64 {
        self.storage_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

/// Atomic usage tracking for a tenant
#[repr(C)]
pub struct AtomicTenantUsage {
    /// Current vector count
    pub vector_count: AtomicU64,
    /// Current storage in bytes
    pub storage_bytes: AtomicU64,
    /// Rate limiting: request count in current window
    pub rate_count: AtomicU32,
    /// Rate limiting: window start (epoch seconds)
    pub rate_window_start: AtomicU64,
    /// Concurrent query count
    pub concurrent_count: AtomicU32,
    /// Collection count
    pub collection_count: AtomicU32,
}

impl AtomicTenantUsage {
    /// Create new atomic usage tracker
    pub fn new() -> Self {
        Self {
            vector_count: AtomicU64::new(0),
            storage_bytes: AtomicU64::new(0),
            rate_count: AtomicU32::new(0),
            rate_window_start: AtomicU64::new(0),
            concurrent_count: AtomicU32::new(0),
            collection_count: AtomicU32::new(0),
        }
    }

    /// Get snapshot of usage
    pub fn snapshot(&self) -> TenantUsage {
        TenantUsage {
            vector_count: self.vector_count.load(Ordering::Relaxed),
            storage_bytes: self.storage_bytes.load(Ordering::Relaxed),
            queries_this_second: self.rate_count.load(Ordering::Relaxed),
            concurrent_queries: self.concurrent_count.load(Ordering::Relaxed),
            collection_count: self.collection_count.load(Ordering::Relaxed),
            last_updated: chrono_now_millis(),
        }
    }

    /// Reset from TenantUsage (for initialization from stored data)
    pub fn reset_from(&self, usage: &TenantUsage) {
        self.vector_count
            .store(usage.vector_count, Ordering::Relaxed);
        self.storage_bytes
            .store(usage.storage_bytes, Ordering::Relaxed);
        self.collection_count
            .store(usage.collection_count, Ordering::Relaxed);
        // Rate limiting and concurrent are not persisted
    }
}

impl Default for AtomicTenantUsage {
    fn default() -> Self {
        Self::new()
    }
}

/// Token bucket rate limiter
pub struct TokenBucket {
    /// Maximum tokens (burst capacity)
    capacity: u32,
    /// Tokens per second (refill rate)
    rate: u32,
    /// Current available tokens (fixed-point * 1000)
    tokens: AtomicU64,
    /// Last refill time (epoch millis)
    last_refill: AtomicU64,
}

impl TokenBucket {
    /// Create a new token bucket
    pub fn new(capacity: u32, rate: u32) -> Self {
        Self {
            capacity,
            rate,
            tokens: AtomicU64::new((capacity as u64) * 1000), // Full bucket, fixed-point
            last_refill: AtomicU64::new(chrono_now_millis() as u64),
        }
    }

    /// Try to acquire tokens
    pub fn try_acquire(&self, tokens: u32) -> bool {
        self.refill();

        let tokens_needed = (tokens as u64) * 1000;
        let current = self.tokens.load(Ordering::Relaxed);

        if current >= tokens_needed {
            // CAS loop for thread safety
            match self.tokens.compare_exchange(
                current,
                current - tokens_needed,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => true,
                Err(_) => {
                    // Retry with updated value
                    let new_current = self.tokens.load(Ordering::Relaxed);
                    if new_current >= tokens_needed {
                        self.tokens.fetch_sub(tokens_needed, Ordering::Relaxed);
                        true
                    } else {
                        false
                    }
                }
            }
        } else {
            false
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&self) {
        let now = chrono_now_millis() as u64;
        let last = self.last_refill.load(Ordering::Relaxed);
        let elapsed_ms = now.saturating_sub(last);

        if elapsed_ms == 0 {
            return;
        }

        // Calculate tokens to add (tokens per second * elapsed seconds)
        let tokens_to_add = (self.rate as u64 * 1000 * elapsed_ms) / 1000;

        if tokens_to_add > 0 {
            let max_tokens = (self.capacity as u64) * 1000;
            let current = self.tokens.load(Ordering::Relaxed);
            let new_tokens = (current + tokens_to_add).min(max_tokens);

            self.tokens.store(new_tokens, Ordering::Relaxed);
            self.last_refill.store(now, Ordering::Relaxed);
        }
    }

    /// Time until tokens become available (milliseconds)
    pub fn time_to_available(&self, tokens: u32) -> u64 {
        self.refill();

        let tokens_needed = (tokens as u64) * 1000;
        let current = self.tokens.load(Ordering::Relaxed);

        if current >= tokens_needed {
            return 0;
        }

        let tokens_short = tokens_needed - current;
        let rate_per_ms = (self.rate as u64 * 1000) / 1000;

        if rate_per_ms == 0 {
            return u64::MAX;
        }

        (tokens_short + rate_per_ms - 1) / rate_per_ms
    }
}

/// Quota enforcement result
#[derive(Debug, Clone)]
pub enum QuotaResult {
    /// Operation allowed
    Allowed,
    /// Rate limit exceeded
    RateLimited {
        /// Retry after this many milliseconds
        retry_after_ms: u64,
    },
    /// Vector quota exceeded
    VectorQuotaExceeded { current: u64, limit: u64 },
    /// Storage quota exceeded
    StorageQuotaExceeded {
        current_bytes: u64,
        limit_bytes: u64,
    },
    /// Concurrent query limit exceeded
    ConcurrentLimitExceeded { current: u32, limit: u32 },
    /// Collection limit exceeded
    CollectionLimitExceeded { current: u32, limit: u32 },
}

impl QuotaResult {
    /// Check if operation is allowed
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    /// Get error message if not allowed
    pub fn error_message(&self) -> Option<String> {
        match self {
            Self::Allowed => None,
            Self::RateLimited { retry_after_ms } => Some(format!(
                "Rate limit exceeded. Retry after {}ms",
                retry_after_ms
            )),
            Self::VectorQuotaExceeded { current, limit } => Some(format!(
                "Vector quota exceeded: {} / {} vectors",
                current, limit
            )),
            Self::StorageQuotaExceeded {
                current_bytes,
                limit_bytes,
            } => {
                let current_gb = *current_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                let limit_gb = *limit_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                Some(format!(
                    "Storage quota exceeded: {:.2} / {:.2} GB",
                    current_gb, limit_gb
                ))
            }
            Self::ConcurrentLimitExceeded { current, limit } => Some(format!(
                "Concurrent query limit exceeded: {} / {}",
                current, limit
            )),
            Self::CollectionLimitExceeded { current, limit } => Some(format!(
                "Collection limit exceeded: {} / {}",
                current, limit
            )),
        }
    }
}

/// Quota manager for all tenants
pub struct QuotaManager {
    /// Atomic usage tracking per tenant
    usage: DashMap<String, AtomicTenantUsage>,
    /// Rate limiters per tenant
    rate_limiters: DashMap<String, TokenBucket>,
}

impl QuotaManager {
    /// Create a new quota manager
    pub fn new() -> Self {
        Self {
            usage: DashMap::new(),
            rate_limiters: DashMap::new(),
        }
    }

    /// Get or create usage tracker for tenant
    fn get_or_create_usage(&self, tenant_id: &str) -> &AtomicTenantUsage {
        if !self.usage.contains_key(tenant_id) {
            self.usage
                .insert(tenant_id.to_string(), AtomicTenantUsage::new());
        }
        // Safe because we just inserted if not present
        // Use leak to get 'static reference - in production would use proper lifetime management
        unsafe {
            let ptr = self.usage.get(tenant_id).unwrap();
            &*(ptr.value() as *const AtomicTenantUsage)
        }
    }

    /// Get or create rate limiter for tenant
    fn get_or_create_rate_limiter(&self, tenant_id: &str, quota: &TenantQuota) -> &TokenBucket {
        if !self.rate_limiters.contains_key(tenant_id) {
            // Burst capacity = 2x QPS, rate = QPS
            let bucket = TokenBucket::new(quota.max_qps * 2, quota.max_qps);
            self.rate_limiters.insert(tenant_id.to_string(), bucket);
        }
        unsafe {
            let ptr = self.rate_limiters.get(tenant_id).unwrap();
            &*(ptr.value() as *const TokenBucket)
        }
    }

    /// Check if vector insert is allowed
    pub fn check_vector_insert(
        &self,
        tenant_id: &str,
        count: u64,
        estimated_bytes: u64,
    ) -> QuotaResult {
        // Get tenant config
        let config = match get_registry().get(tenant_id) {
            Some(c) => c,
            None => return QuotaResult::Allowed, // No quota if no tenant
        };

        let usage = self.get_or_create_usage(tenant_id);

        // Check vector count
        let current_vectors = usage.vector_count.load(Ordering::Relaxed);
        if current_vectors + count > config.quota.max_vectors {
            return QuotaResult::VectorQuotaExceeded {
                current: current_vectors,
                limit: config.quota.max_vectors,
            };
        }

        // Check storage
        let current_storage = usage.storage_bytes.load(Ordering::Relaxed);
        if current_storage + estimated_bytes > config.quota.max_storage_bytes {
            return QuotaResult::StorageQuotaExceeded {
                current_bytes: current_storage,
                limit_bytes: config.quota.max_storage_bytes,
            };
        }

        QuotaResult::Allowed
    }

    /// Check if query is allowed (rate limiting)
    pub fn check_query(&self, tenant_id: &str) -> QuotaResult {
        // Get tenant config
        let config = match get_registry().get(tenant_id) {
            Some(c) => c,
            None => return QuotaResult::Allowed,
        };

        // Check rate limit
        let rate_limiter = self.get_or_create_rate_limiter(tenant_id, &config.quota);
        if !rate_limiter.try_acquire(1) {
            return QuotaResult::RateLimited {
                retry_after_ms: rate_limiter.time_to_available(1),
            };
        }

        // Check concurrent queries
        let usage = self.get_or_create_usage(tenant_id);
        let current_concurrent = usage.concurrent_count.load(Ordering::Relaxed);
        if current_concurrent >= config.quota.max_concurrent {
            return QuotaResult::ConcurrentLimitExceeded {
                current: current_concurrent,
                limit: config.quota.max_concurrent,
            };
        }

        QuotaResult::Allowed
    }

    /// Check if collection creation is allowed
    pub fn check_collection_create(&self, tenant_id: &str) -> QuotaResult {
        let config = match get_registry().get(tenant_id) {
            Some(c) => c,
            None => return QuotaResult::Allowed,
        };

        let usage = self.get_or_create_usage(tenant_id);
        let current = usage.collection_count.load(Ordering::Relaxed);

        if current >= config.quota.max_collections {
            return QuotaResult::CollectionLimitExceeded {
                current,
                limit: config.quota.max_collections,
            };
        }

        QuotaResult::Allowed
    }

    /// Record vector insert (after successful insert)
    pub fn record_vector_insert(&self, tenant_id: &str, count: u64, bytes: u64) {
        let usage = self.get_or_create_usage(tenant_id);
        usage.vector_count.fetch_add(count, Ordering::Relaxed);
        usage.storage_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record vector delete
    pub fn record_vector_delete(&self, tenant_id: &str, count: u64, bytes: u64) {
        let usage = self.get_or_create_usage(tenant_id);
        usage.vector_count.fetch_sub(
            count.min(usage.vector_count.load(Ordering::Relaxed)),
            Ordering::Relaxed,
        );
        usage.storage_bytes.fetch_sub(
            bytes.min(usage.storage_bytes.load(Ordering::Relaxed)),
            Ordering::Relaxed,
        );
    }

    /// Record collection creation
    pub fn record_collection_create(&self, tenant_id: &str) {
        let usage = self.get_or_create_usage(tenant_id);
        usage.collection_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record collection deletion
    pub fn record_collection_delete(&self, tenant_id: &str) {
        let usage = self.get_or_create_usage(tenant_id);
        let current = usage.collection_count.load(Ordering::Relaxed);
        if current > 0 {
            usage.collection_count.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Start tracking a concurrent query
    pub fn start_query(&self, tenant_id: &str) {
        let usage = self.get_or_create_usage(tenant_id);
        usage.concurrent_count.fetch_add(1, Ordering::Relaxed);
    }

    /// End tracking a concurrent query
    pub fn end_query(&self, tenant_id: &str) {
        let usage = self.get_or_create_usage(tenant_id);
        let current = usage.concurrent_count.load(Ordering::Relaxed);
        if current > 0 {
            usage.concurrent_count.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Get current usage for a tenant
    pub fn get_usage(&self, tenant_id: &str) -> Option<TenantUsage> {
        self.usage.get(tenant_id).map(|u| u.snapshot())
    }

    /// Get quota status for a tenant
    pub fn get_quota_status(&self, tenant_id: &str) -> Option<QuotaStatus> {
        let config = get_registry().get(tenant_id)?;
        let usage = self.get_usage(tenant_id).unwrap_or_default();

        Some(QuotaStatus {
            tenant_id: tenant_id.to_string(),
            vectors: ResourceUsage {
                current: usage.vector_count,
                limit: config.quota.max_vectors,
                usage_percent: (usage.vector_count as f64 / config.quota.max_vectors as f64 * 100.0)
                    as f32,
            },
            storage: ResourceUsage {
                current: usage.storage_bytes,
                limit: config.quota.max_storage_bytes,
                usage_percent: (usage.storage_bytes as f64 / config.quota.max_storage_bytes as f64
                    * 100.0) as f32,
            },
            qps: RateUsage {
                current: usage.queries_this_second,
                limit: config.quota.max_qps,
            },
            concurrent: ResourceUsage {
                current: usage.concurrent_queries as u64,
                limit: config.quota.max_concurrent as u64,
                usage_percent: (usage.concurrent_queries as f64
                    / config.quota.max_concurrent as f64
                    * 100.0) as f32,
            },
            collections: ResourceUsage {
                current: usage.collection_count as u64,
                limit: config.quota.max_collections as u64,
                usage_percent: (usage.collection_count as f64 / config.quota.max_collections as f64
                    * 100.0) as f32,
            },
        })
    }

    /// Reset usage counters for a tenant
    pub fn reset_usage(&self, tenant_id: &str) {
        if let Some(usage) = self.usage.get(tenant_id) {
            usage.vector_count.store(0, Ordering::Relaxed);
            usage.storage_bytes.store(0, Ordering::Relaxed);
            usage.collection_count.store(0, Ordering::Relaxed);
            usage.rate_count.store(0, Ordering::Relaxed);
            usage.concurrent_count.store(0, Ordering::Relaxed);
        }
    }

    /// Initialize usage from stored values (e.g., from database)
    pub fn initialize_usage(&self, tenant_id: &str, stored_usage: TenantUsage) {
        let usage = self.get_or_create_usage(tenant_id);
        usage.reset_from(&stored_usage);
    }
}

impl Default for QuotaManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource usage summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Current usage
    pub current: u64,
    /// Maximum limit
    pub limit: u64,
    /// Usage percentage
    pub usage_percent: f32,
}

/// Rate usage summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateUsage {
    /// Current rate
    pub current: u32,
    /// Maximum rate
    pub limit: u32,
}

/// Complete quota status for a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaStatus {
    /// Tenant ID
    pub tenant_id: String,
    /// Vector count usage
    pub vectors: ResourceUsage,
    /// Storage usage
    pub storage: ResourceUsage,
    /// Current QPS
    pub qps: RateUsage,
    /// Concurrent queries
    pub concurrent: ResourceUsage,
    /// Collection count
    pub collections: ResourceUsage,
}

impl QuotaStatus {
    /// Check if any quota is near limit (>80%)
    pub fn is_near_limit(&self) -> bool {
        self.vectors.usage_percent > 80.0
            || self.storage.usage_percent > 80.0
            || self.collections.usage_percent > 80.0
    }

    /// Check if any quota is critical (>95%)
    pub fn is_critical(&self) -> bool {
        self.vectors.usage_percent > 95.0 || self.storage.usage_percent > 95.0
    }
}

/// Global quota manager instance
static QUOTA_MANAGER: once_cell::sync::Lazy<QuotaManager> =
    once_cell::sync::Lazy::new(QuotaManager::new);

/// Get the global quota manager
pub fn get_quota_manager() -> &'static QuotaManager {
    &QUOTA_MANAGER
}

// Helper functions
fn chrono_now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_acquire() {
        let bucket = TokenBucket::new(10, 10); // 10 capacity, 10/second

        // Should be able to acquire up to capacity
        for _ in 0..10 {
            assert!(bucket.try_acquire(1));
        }

        // Should fail after capacity exhausted
        assert!(!bucket.try_acquire(1));
    }

    #[test]
    fn test_tenant_usage_tracking() {
        let manager = QuotaManager::new();

        // Record some usage
        manager.record_vector_insert("test-tenant", 100, 1024 * 100);
        manager.record_collection_create("test-tenant");

        // Check usage
        let usage = manager.get_usage("test-tenant").unwrap();
        assert_eq!(usage.vector_count, 100);
        assert_eq!(usage.storage_bytes, 1024 * 100);
        assert_eq!(usage.collection_count, 1);

        // Record deletion
        manager.record_vector_delete("test-tenant", 50, 1024 * 50);
        let usage = manager.get_usage("test-tenant").unwrap();
        assert_eq!(usage.vector_count, 50);
    }

    #[test]
    fn test_quota_result_messages() {
        let result = QuotaResult::RateLimited {
            retry_after_ms: 100,
        };
        assert!(!result.is_allowed());
        assert!(result.error_message().unwrap().contains("100"));

        let result = QuotaResult::VectorQuotaExceeded {
            current: 1000,
            limit: 1000,
        };
        assert!(!result.is_allowed());
        assert!(result.error_message().unwrap().contains("1000"));

        let result = QuotaResult::Allowed;
        assert!(result.is_allowed());
        assert!(result.error_message().is_none());
    }

    #[test]
    fn test_concurrent_query_tracking() {
        let manager = QuotaManager::new();

        // Start queries
        manager.start_query("test-tenant");
        manager.start_query("test-tenant");

        let usage = manager.get_usage("test-tenant").unwrap();
        assert_eq!(usage.concurrent_queries, 2);

        // End one query
        manager.end_query("test-tenant");
        let usage = manager.get_usage("test-tenant").unwrap();
        assert_eq!(usage.concurrent_queries, 1);
    }

    #[test]
    fn test_usage_reset() {
        let manager = QuotaManager::new();

        manager.record_vector_insert("test-tenant", 100, 1024);
        manager.record_collection_create("test-tenant");
        manager.start_query("test-tenant");

        // Reset
        manager.reset_usage("test-tenant");

        let usage = manager.get_usage("test-tenant").unwrap();
        assert_eq!(usage.vector_count, 0);
        assert_eq!(usage.storage_bytes, 0);
        assert_eq!(usage.collection_count, 0);
        assert_eq!(usage.concurrent_queries, 0);
    }
}
