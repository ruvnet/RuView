//! Tenant-Aware Operations for RuVector Multi-Tenancy
//!
//! Wraps index operations with tenant context validation, quota enforcement,
//! and proper routing based on isolation level.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::isolation::{get_isolation_manager, QueryRoute};
use super::quotas::{get_quota_manager, QuotaResult};
use super::registry::{get_registry, TenantConfig, TenantError};
use super::validation::{escape_string_literal, validate_ip_address, validate_tenant_id};

/// Result of a tenant-aware operation
#[derive(Debug, Clone)]
pub enum OperationResult<T> {
    /// Operation succeeded
    Success(T),
    /// Operation denied due to quota
    QuotaDenied(QuotaResult),
    /// Operation denied due to tenant error
    TenantError(TenantError),
    /// Operation failed with error
    Error(String),
}

impl<T> OperationResult<T> {
    /// Check if operation succeeded
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    /// Get success value or panic
    pub fn unwrap(self) -> T {
        match self {
            Self::Success(v) => v,
            Self::QuotaDenied(q) => panic!("Quota denied: {:?}", q),
            Self::TenantError(e) => panic!("Tenant error: {}", e),
            Self::Error(e) => panic!("Operation error: {}", e),
        }
    }

    /// Get success value or return error message
    pub fn into_result(self) -> Result<T, String> {
        match self {
            Self::Success(v) => Ok(v),
            Self::QuotaDenied(q) => Err(q
                .error_message()
                .unwrap_or_else(|| "Quota denied".to_string())),
            Self::TenantError(e) => Err(e.to_string()),
            Self::Error(e) => Err(e),
        }
    }
}

/// Tenant context for operations
#[derive(Debug, Clone)]
pub struct TenantContext {
    /// Tenant ID (validated)
    pub tenant_id: String,
    /// Tenant configuration
    pub config: TenantConfig,
    /// Query routing information
    pub route: QueryRoute,
    /// Whether this is an admin context
    pub is_admin: bool,
}

/// Represents a validated tenant ID
#[derive(Debug, Clone)]
pub struct ValidatedTenantId(String);

impl ValidatedTenantId {
    /// Create a new validated tenant ID
    pub fn new(tenant_id: &str) -> Result<Self, TenantError> {
        validate_tenant_id(tenant_id).map_err(|e| TenantError::InvalidId(format!("{}", e)))?;
        Ok(Self(tenant_id.to_string()))
    }

    /// Get the tenant ID as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TenantContext {
    /// Get current tenant context from GUC
    pub fn current() -> Result<Self, TenantError> {
        // Get tenant_id from PostgreSQL GUC
        let tenant_id = get_current_tenant_id()?;

        // Special handling for admin wildcard
        if tenant_id == "*" {
            return Ok(Self {
                tenant_id: "*".to_string(),
                config: TenantConfig::new("*".to_string()),
                route: QueryRoute::SharedWithFilter {
                    table: "".to_string(),
                    filter: "true".to_string(), // No filter for admin
                    tenant_param: None,         // Admin doesn't need tenant param
                },
                is_admin: true,
            });
        }

        // Validate tenant context
        let config = get_registry().validate_context(&tenant_id)?;

        // Get routing for this tenant
        let route = get_isolation_manager().route_query(&tenant_id, "embeddings");

        Ok(Self {
            tenant_id,
            config,
            route,
            is_admin: false,
        })
    }

    /// Create context for a specific tenant (bypassing GUC)
    pub fn for_tenant(tenant_id: &str) -> Result<Self, TenantError> {
        let config = get_registry().validate_context(tenant_id)?;
        let route = get_isolation_manager().route_query(tenant_id, "embeddings");

        Ok(Self {
            tenant_id: tenant_id.to_string(),
            config,
            route,
            is_admin: false,
        })
    }

    /// Get table reference for SQL queries
    pub fn table_ref(&self, base_table: &str) -> String {
        let route = get_isolation_manager().route_query(&self.tenant_id, base_table);
        route.table_reference()
    }

    /// Get WHERE clause for tenant filtering (if needed)
    pub fn where_clause(&self, base_table: &str) -> Option<String> {
        let route = get_isolation_manager().route_query(&self.tenant_id, base_table);
        route.where_clause()
    }
}

/// Get current tenant ID from PostgreSQL GUC
pub fn get_current_tenant_id() -> Result<String, TenantError> {
    // In actual pgrx implementation, this would use:
    // Spi::get_one::<String>("SELECT current_setting('ruvector.tenant_id', true)")

    // For now, provide a placeholder that would be replaced with actual GUC access
    #[cfg(feature = "pg_test")]
    {
        // In tests, use a thread-local for testing
        thread_local! {
            static MOCK_TENANT_ID: std::cell::RefCell<String> = std::cell::RefCell::new(String::new());
        }

        MOCK_TENANT_ID.with(|id| {
            let tenant_id = id.borrow().clone();
            if tenant_id.is_empty() {
                Err(TenantError::NoContext)
            } else {
                Ok(tenant_id)
            }
        })
    }

    #[cfg(not(feature = "pg_test"))]
    {
        // Actual PostgreSQL GUC access would go here
        // This is a placeholder for the actual implementation
        Err(TenantError::NoContext)
    }
}

/// Set mock tenant ID for testing
#[cfg(feature = "pg_test")]
pub fn set_mock_tenant_id(tenant_id: &str) {
    thread_local! {
        static MOCK_TENANT_ID: std::cell::RefCell<String> = std::cell::RefCell::new(String::new());
    }

    MOCK_TENANT_ID.with(|id| {
        *id.borrow_mut() = tenant_id.to_string();
    });
}

/// Tenant-aware vector insert operation
pub struct TenantVectorInsert<'a> {
    ctx: &'a TenantContext,
    vectors: Vec<(Vec<f32>, Option<serde_json::Value>)>,
    table_name: String,
    estimated_bytes_per_vector: usize,
}

impl<'a> TenantVectorInsert<'a> {
    /// Create a new tenant-aware insert
    pub fn new(ctx: &'a TenantContext, table_name: &str) -> Self {
        Self {
            ctx,
            vectors: Vec::new(),
            table_name: table_name.to_string(),
            estimated_bytes_per_vector: 4 * 1536 + 100, // Default for 1536-dim + metadata
        }
    }

    /// Add a vector to insert
    pub fn add(&mut self, vector: Vec<f32>, metadata: Option<serde_json::Value>) -> &mut Self {
        self.vectors.push((vector, metadata));
        self
    }

    /// Add multiple vectors
    pub fn add_batch(&mut self, vectors: Vec<(Vec<f32>, Option<serde_json::Value>)>) -> &mut Self {
        self.vectors.extend(vectors);
        self
    }

    /// Execute the insert with quota enforcement
    pub fn execute(self) -> OperationResult<InsertResult> {
        let quota_manager = get_quota_manager();

        // Calculate estimated bytes
        let total_bytes = self.vectors.len() as u64 * self.estimated_bytes_per_vector as u64;

        // Check quota before insert
        let quota_check = quota_manager.check_vector_insert(
            &self.ctx.tenant_id,
            self.vectors.len() as u64,
            total_bytes,
        );

        if !quota_check.is_allowed() {
            return OperationResult::QuotaDenied(quota_check);
        }

        // Get proper table reference
        let table_ref = self.ctx.table_ref(&self.table_name);

        // Execute insert (placeholder - actual implementation would use SPI)
        let start = Instant::now();
        let inserted_count = self.vectors.len();

        // Record successful insert
        quota_manager.record_vector_insert(&self.ctx.tenant_id, inserted_count as u64, total_bytes);

        OperationResult::Success(InsertResult {
            inserted_count,
            table_used: table_ref,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

/// Result of an insert operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertResult {
    /// Number of vectors inserted
    pub inserted_count: usize,
    /// Table that was used
    pub table_used: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Tenant-aware vector search operation
pub struct TenantVectorSearch<'a> {
    ctx: &'a TenantContext,
    query: Vec<f32>,
    k: usize,
    table_name: String,
    ef_search: Option<usize>,
    filter: Option<String>,
}

impl<'a> TenantVectorSearch<'a> {
    /// Create a new tenant-aware search
    pub fn new(ctx: &'a TenantContext, query: Vec<f32>, k: usize, table_name: &str) -> Self {
        Self {
            ctx,
            query,
            k,
            table_name: table_name.to_string(),
            ef_search: None,
            filter: None,
        }
    }

    /// Set ef_search parameter
    pub fn with_ef_search(mut self, ef: usize) -> Self {
        self.ef_search = Some(ef);
        self
    }

    /// Add additional WHERE filter
    pub fn with_filter(mut self, filter: &str) -> Self {
        self.filter = Some(filter.to_string());
        self
    }

    /// Execute the search with rate limiting
    pub fn execute(self) -> OperationResult<SearchResult> {
        let quota_manager = get_quota_manager();

        // Check rate limit
        let rate_check = quota_manager.check_query(&self.ctx.tenant_id);
        if !rate_check.is_allowed() {
            return OperationResult::QuotaDenied(rate_check);
        }

        // Start concurrent query tracking
        quota_manager.start_query(&self.ctx.tenant_id);

        let start = Instant::now();

        // Get proper table reference and filters
        let table_ref = self.ctx.table_ref(&self.table_name);
        let tenant_filter = self.ctx.where_clause(&self.table_name);

        // Combine filters
        let combined_filter = match (&tenant_filter, &self.filter) {
            (Some(tf), Some(f)) => Some(format!("({}) AND ({})", tf, f)),
            (Some(tf), None) => Some(tf.clone()),
            (None, Some(f)) => Some(f.clone()),
            (None, None) => None,
        };

        // Execute search (placeholder - actual implementation would use SPI)
        let results: Vec<(i64, f32)> = Vec::new(); // Would be populated by actual search

        // End concurrent query tracking
        quota_manager.end_query(&self.ctx.tenant_id);

        OperationResult::Success(SearchResult {
            results,
            k: self.k,
            table_used: table_ref,
            filter_applied: combined_filter,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

/// Result of a search operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Search results (id, distance)
    pub results: Vec<(i64, f32)>,
    /// Requested k
    pub k: usize,
    /// Table that was searched
    pub table_used: String,
    /// Filter that was applied
    pub filter_applied: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Tenant-aware delete operation
pub struct TenantVectorDelete<'a> {
    ctx: &'a TenantContext,
    ids: Vec<i64>,
    table_name: String,
}

impl<'a> TenantVectorDelete<'a> {
    /// Create a new tenant-aware delete
    pub fn new(ctx: &'a TenantContext, ids: Vec<i64>, table_name: &str) -> Self {
        Self {
            ctx,
            ids,
            table_name: table_name.to_string(),
        }
    }

    /// Execute the delete with quota tracking
    pub fn execute(self) -> OperationResult<DeleteResult> {
        let quota_manager = get_quota_manager();

        let start = Instant::now();
        let table_ref = self.ctx.table_ref(&self.table_name);

        // Execute delete (placeholder - actual implementation would use SPI)
        let deleted_count = self.ids.len();
        let deleted_bytes = (deleted_count * 4 * 1536) as u64; // Estimate

        // Record deletion in quota manager
        quota_manager.record_vector_delete(
            &self.ctx.tenant_id,
            deleted_count as u64,
            deleted_bytes,
        );

        OperationResult::Success(DeleteResult {
            deleted_count,
            table_used: table_ref,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

/// Result of a delete operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResult {
    /// Number of vectors deleted
    pub deleted_count: usize,
    /// Table that was used
    pub table_used: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Statistics for a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantStats {
    /// Tenant ID
    pub tenant_id: String,
    /// Vector count
    pub vector_count: u64,
    /// Storage used in bytes
    pub storage_bytes: u64,
    /// Collection count
    pub collection_count: u32,
    /// Isolation level
    pub isolation_level: String,
    /// Integrity state
    pub integrity_state: String,
    /// Lambda cut value
    pub lambda_cut: f32,
    /// Is suspended
    pub is_suspended: bool,
    /// Quota usage percentage
    pub quota_usage_percent: f32,
}

/// Get comprehensive statistics for a tenant
pub fn get_tenant_stats(tenant_id: &str) -> Result<TenantStats, TenantError> {
    let config = get_registry()
        .get(tenant_id)
        .ok_or_else(|| TenantError::NotFound(tenant_id.to_string()))?;

    let usage = get_quota_manager().get_usage(tenant_id).unwrap_or_default();

    let shared_state = get_registry().get_shared_state(tenant_id);

    let (integrity_state, lambda_cut) = match shared_state {
        Some(state) => {
            let integrity = match state
                .integrity_state
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                0 => "normal",
                1 => "stress",
                2 => "critical",
                _ => "unknown",
            };
            (integrity.to_string(), state.lambda_cut())
        }
        None => ("unknown".to_string(), 1.0),
    };

    Ok(TenantStats {
        tenant_id: tenant_id.to_string(),
        vector_count: usage.vector_count,
        storage_bytes: usage.storage_bytes,
        collection_count: usage.collection_count,
        isolation_level: config.isolation_level.as_str().to_string(),
        integrity_state,
        lambda_cut,
        is_suspended: config.is_suspended(),
        quota_usage_percent: (usage.vector_count as f64 / config.quota.max_vectors as f64 * 100.0)
            as f32,
    })
}

/// Audit log entry for tenant operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Tenant ID
    pub tenant_id: String,
    /// Operation type
    pub operation: String,
    /// User ID (from application context)
    pub user_id: Option<String>,
    /// Details about the operation
    pub details: serde_json::Value,
    /// Timestamp
    pub timestamp: i64,
    /// IP address (if available)
    pub ip_address: Option<String>,
    /// Success status
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

impl AuditLogEntry {
    /// Create a new audit log entry
    pub fn new(tenant_id: &str, operation: &str) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
            operation: operation.to_string(),
            user_id: None,
            details: serde_json::json!({}),
            timestamp: chrono_now_millis(),
            ip_address: None,
            success: true,
            error: None,
        }
    }

    /// Set user ID
    pub fn with_user(mut self, user_id: &str) -> Self {
        self.user_id = Some(user_id.to_string());
        self
    }

    /// Set details
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    /// Mark as failed
    pub fn failed(mut self, error: &str) -> Self {
        self.success = false;
        self.error = Some(error.to_string());
        self
    }

    /// Generate SQL to insert this audit entry (parameterized version)
    ///
    /// Returns the SQL with $1-$7 placeholders and the parameter values to bind.
    /// This prevents SQL injection by using parameterized queries.
    pub fn insert_sql_parameterized(&self) -> (String, Vec<Option<String>>) {
        let sql = r#"
INSERT INTO ruvector.tenant_audit_log (tenant_id, operation, user_id, details, ip_address, success, error)
VALUES ($1, $2, $3, $4, $5, $6, $7)
"#.to_string();

        let params = vec![
            Some(self.tenant_id.clone()),
            Some(self.operation.clone()),
            self.user_id.clone(),
            Some(serde_json::to_string(&self.details).unwrap_or_else(|_| "{}".to_string())),
            // Only include IP if it's a valid IP address (defense in depth)
            self.ip_address.as_ref().and_then(|ip| {
                if validate_ip_address(ip) {
                    Some(ip.clone())
                } else {
                    None
                }
            }),
            Some(self.success.to_string()),
            self.error.clone(),
        ];

        (sql, params)
    }

    /// Generate SQL to insert this audit entry (legacy - properly escaped)
    ///
    /// Note: Prefer `insert_sql_parameterized()` for new code.
    /// This method properly escapes all values but parameterized queries are safer.
    pub fn insert_sql(&self) -> String {
        // Validate tenant_id format
        if validate_tenant_id(&self.tenant_id).is_err() {
            // Log the attempt but don't execute with invalid tenant_id
            return "SELECT 1 WHERE false".to_string(); // No-op query
        }

        // Escape all string values
        let escaped_tenant_id = escape_string_literal(&self.tenant_id);
        let escaped_operation = escape_string_literal(&self.operation);
        let escaped_user_id = self
            .user_id
            .as_ref()
            .map(|u| format!("'{}'", escape_string_literal(u)))
            .unwrap_or_else(|| "NULL".to_string());
        let escaped_details = escape_string_literal(
            &serde_json::to_string(&self.details).unwrap_or_else(|_| "{}".to_string()),
        );
        let escaped_ip = self
            .ip_address
            .as_ref()
            .and_then(|ip| {
                // Only include if valid IP format
                if validate_ip_address(ip) {
                    Some(format!("'{}'", escape_string_literal(ip)))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "NULL".to_string());
        let escaped_error = self
            .error
            .as_ref()
            .map(|e| format!("'{}'", escape_string_literal(e)))
            .unwrap_or_else(|| "NULL".to_string());

        format!(
            r#"
INSERT INTO ruvector.tenant_audit_log (tenant_id, operation, user_id, details, ip_address, success, error)
VALUES ('{}', '{}', {}, '{}', {}, {}, {})
"#,
            escaped_tenant_id,
            escaped_operation,
            escaped_user_id,
            escaped_details,
            escaped_ip,
            self.success,
            escaped_error
        )
    }
}

// Helper functions
fn chrono_now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Cross-tenant prevention check
pub fn validate_cross_tenant(
    context_tenant: &str,
    request_tenant: Option<&str>,
) -> Result<(), TenantError> {
    if let Some(req_tenant) = request_tenant {
        if req_tenant != context_tenant && context_tenant != "*" {
            // Log security event
            let entry = AuditLogEntry::new(context_tenant, "cross_tenant_attempt")
                .with_details(serde_json::json!({
                    "requested_tenant": req_tenant,
                    "context_tenant": context_tenant
                }))
                .failed("Cross-tenant access denied");

            // Would log to audit table here

            return Err(TenantError::TenantMismatch {
                context: context_tenant.to_string(),
                request: req_tenant.to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::registry::TenantConfig;
    use super::*;

    fn setup_test_tenant(id: &str) {
        let registry = get_registry();
        let config = TenantConfig::new(id.to_string());
        let _ = registry.register(config);
    }

    #[test]
    fn test_operation_result() {
        let success: OperationResult<i32> = OperationResult::Success(42);
        assert!(success.is_success());
        assert_eq!(success.unwrap(), 42);

        let denied = OperationResult::<i32>::QuotaDenied(QuotaResult::RateLimited {
            retry_after_ms: 100,
        });
        assert!(!denied.is_success());
        assert!(denied.into_result().is_err());
    }

    #[test]
    fn test_cross_tenant_validation() {
        // Same tenant should pass
        assert!(validate_cross_tenant("tenant-a", Some("tenant-a")).is_ok());

        // Different tenant should fail
        assert!(validate_cross_tenant("tenant-a", Some("tenant-b")).is_err());

        // Admin wildcard should pass
        assert!(validate_cross_tenant("*", Some("tenant-b")).is_ok());

        // No request tenant should pass
        assert!(validate_cross_tenant("tenant-a", None).is_ok());
    }

    #[test]
    fn test_audit_log_entry() {
        let entry = AuditLogEntry::new("acme-corp", "vector_insert")
            .with_user("user123")
            .with_details(serde_json::json!({"count": 100}));

        assert_eq!(entry.tenant_id, "acme-corp");
        assert_eq!(entry.operation, "vector_insert");
        assert!(entry.success);

        let failed_entry =
            AuditLogEntry::new("acme-corp", "vector_insert").failed("Quota exceeded");

        assert!(!failed_entry.success);
        assert!(failed_entry.error.is_some());
    }

    #[test]
    fn test_insert_result() {
        let result = InsertResult {
            inserted_count: 100,
            table_used: "embeddings".to_string(),
            duration_ms: 50,
        };

        assert_eq!(result.inserted_count, 100);
    }

    #[test]
    fn test_search_result() {
        let result = SearchResult {
            results: vec![(1, 0.1), (2, 0.2)],
            k: 10,
            table_used: "embeddings".to_string(),
            filter_applied: Some("category = 'test'".to_string()),
            duration_ms: 25,
        };

        assert_eq!(result.results.len(), 2);
        assert!(result.filter_applied.is_some());
    }
}
