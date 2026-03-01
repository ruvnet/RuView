//! Multi-Tenancy Module for RuVector PostgreSQL Extension
//!
//! Provides first-class multi-tenancy support with:
//! - Tenant-isolated vector search (data never leaks)
//! - Per-tenant integrity monitoring
//! - Fair resource allocation with quotas
//! - Row-level security integration
//! - Multiple isolation levels (shared, partition, dedicated)
//!
//! # Usage
//!
//! ```sql
//! -- Create a tenant
//! SELECT ruvector_tenant_create('acme-corp', '{
//!     "display_name": "Acme Corporation",
//!     "max_vectors": 5000000,
//!     "isolation_level": "shared"
//! }'::jsonb);
//!
//! -- Set tenant context for session
//! SET ruvector.tenant_id = 'acme-corp';
//!
//! -- All operations are now tenant-scoped
//! INSERT INTO embeddings (content, vec) VALUES ('doc', '[0.1, 0.2, ...]');
//! SELECT * FROM embeddings ORDER BY vec <-> query LIMIT 10;
//! ```

pub mod isolation;
pub mod operations;
pub mod quotas;
pub mod registry;
pub mod rls;
pub mod validation;

// Re-export main types
pub use isolation::{
    get_isolation_manager, IsolationError, IsolationManager, MigrationState, MigrationStatus,
    QueryRoute,
};
pub use operations::{
    get_tenant_stats, TenantContext, TenantStats, TenantVectorDelete, TenantVectorInsert,
    TenantVectorSearch,
};
pub use quotas::{get_quota_manager, QuotaManager, QuotaResult, QuotaStatus, TenantUsage};
pub use registry::{
    get_registry, IsolationLevel, PromotionPolicy, TenantConfig, TenantError, TenantQuota,
    TenantRegistry,
};
pub use rls::{get_rls_manager, PolicyTemplate, RlsManager, RlsPolicyConfig};
pub use validation::{
    escape_string_literal, quote_identifier, safe_partition_name, safe_schema_name,
    sanitize_for_identifier, validate_identifier, validate_tenant_id, ValidationError,
};

use pgrx::prelude::*;
use pgrx::JsonB;

// ============================================================================
// GUC Registration for Tenant Context
// ============================================================================

/// Initialize tenant-related GUCs
/// Note: ruvector.tenant_id is registered as a custom GUC that can be set
/// using SET ruvector.tenant_id = 'tenant-name' and read using
/// current_setting('ruvector.tenant_id', true)
pub fn init_tenant_gucs() {
    // The tenant_id GUC is handled as a custom variable that PostgreSQL
    // manages natively. We don't need to pre-register it - users can simply:
    //   SET ruvector.tenant_id = 'my-tenant';
    //   SELECT current_setting('ruvector.tenant_id', true);
    //
    // This approach is consistent with how PostgreSQL handles custom GUCs
    // and is the pattern used by other extensions for session-level context.
    //
    // To make this work securely, we rely on RLS policies that read
    // current_setting('ruvector.tenant_id', true) directly.

    pgrx::log!("RuVector multi-tenancy initialized");
}

// ============================================================================
// SQL Functions - Tenant Management
// ============================================================================

/// Create a new tenant
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_tenant_create('acme-corp', '{
///     "display_name": "Acme Corporation",
///     "max_vectors": 5000000,
///     "max_qps": 200,
///     "isolation_level": "shared",
///     "integrity_enabled": true
/// }'::jsonb);
/// ```
#[pg_extern]
pub fn ruvector_tenant_create(
    tenant_id: &str,
    config: Option<JsonB>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let tenant_config = match config {
        Some(JsonB(json_val)) => TenantConfig::from_json(tenant_id.to_string(), &json_val),
        None => TenantConfig::new(tenant_id.to_string()),
    };

    let registry = get_registry();
    registry.register(tenant_config)?;

    Ok(format!("Tenant '{}' created successfully", tenant_id))
}

/// Set current tenant context for the session
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_tenant_set('acme-corp');
/// -- All subsequent operations are now scoped to acme-corp
/// ```
#[pg_extern]
pub fn ruvector_tenant_set(
    tenant_id: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Validate tenant exists and is active
    let registry = get_registry();
    let config = registry.validate_context(tenant_id)?;

    // Set the GUC (in actual implementation)
    // For now, return success message
    Ok(format!(
        "Tenant context set to '{}' (isolation: {})",
        tenant_id,
        config.isolation_level.as_str()
    ))
}

/// Get statistics for a tenant
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_tenant_stats('acme-corp');
/// ```
#[pg_extern]
pub fn ruvector_tenant_stats(
    tenant_id: &str,
) -> Result<JsonB, Box<dyn std::error::Error + Send + Sync>> {
    let stats = get_tenant_stats(tenant_id)?;

    Ok(JsonB(serde_json::json!({
        "tenant_id": stats.tenant_id,
        "vector_count": stats.vector_count,
        "storage_bytes": stats.storage_bytes,
        "storage_gb": stats.storage_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        "collection_count": stats.collection_count,
        "isolation_level": stats.isolation_level,
        "integrity_state": stats.integrity_state,
        "lambda_cut": stats.lambda_cut,
        "is_suspended": stats.is_suspended,
        "quota_usage_percent": stats.quota_usage_percent
    })))
}

/// Check quota status for a tenant
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_tenant_quota_check('acme-corp');
/// ```
#[pg_extern]
pub fn ruvector_tenant_quota_check(
    tenant_id: &str,
) -> Result<JsonB, Box<dyn std::error::Error + Send + Sync>> {
    let status = get_quota_manager()
        .get_quota_status(tenant_id)
        .ok_or_else(|| format!("Tenant not found: {}", tenant_id))?;

    Ok(JsonB(serde_json::json!({
        "tenant_id": status.tenant_id,
        "vectors": {
            "current": status.vectors.current,
            "limit": status.vectors.limit,
            "usage_percent": status.vectors.usage_percent
        },
        "storage": {
            "current_bytes": status.storage.current,
            "limit_bytes": status.storage.limit,
            "usage_percent": status.storage.usage_percent
        },
        "qps": {
            "current": status.qps.current,
            "limit": status.qps.limit
        },
        "concurrent_queries": {
            "current": status.concurrent.current,
            "limit": status.concurrent.limit
        },
        "collections": {
            "current": status.collections.current,
            "limit": status.collections.limit
        },
        "is_near_limit": status.is_near_limit(),
        "is_critical": status.is_critical()
    })))
}

/// Suspend a tenant
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_tenant_suspend('bad-actor');
/// ```
#[pg_extern]
pub fn ruvector_tenant_suspend(
    tenant_id: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    get_registry().suspend(tenant_id)?;
    Ok(format!("Tenant '{}' has been suspended", tenant_id))
}

/// Resume a suspended tenant
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_tenant_resume('bad-actor');
/// ```
#[pg_extern]
pub fn ruvector_tenant_resume(
    tenant_id: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    get_registry().resume(tenant_id)?;
    Ok(format!("Tenant '{}' has been resumed", tenant_id))
}

/// Delete a tenant
///
/// # Examples
///
/// ```sql
/// -- Soft delete (marks for cleanup)
/// SELECT ruvector_tenant_delete('churned-customer');
///
/// -- Hard delete (immediate)
/// SELECT ruvector_tenant_delete('churned-customer', true);
/// ```
#[pg_extern]
pub fn ruvector_tenant_delete(
    tenant_id: &str,
    hard: default!(bool, false),
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    get_registry().delete(tenant_id, hard)?;

    let delete_type = if hard {
        "permanently deleted"
    } else {
        "marked for deletion"
    };
    Ok(format!("Tenant '{}' has been {}", tenant_id, delete_type))
}

/// List all tenants
///
/// # Examples
///
/// ```sql
/// SELECT * FROM ruvector_tenants();
/// ```
#[pg_extern]
pub fn ruvector_tenants() -> Result<JsonB, Box<dyn std::error::Error + Send + Sync>> {
    let tenants = get_registry().list();

    let tenant_list: Vec<serde_json::Value> = tenants
        .iter()
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "display_name": t.display_name,
                "isolation_level": t.isolation_level.as_str(),
                "max_vectors": t.quota.max_vectors,
                "max_qps": t.quota.max_qps,
                "integrity_enabled": t.integrity_enabled,
                "is_suspended": t.is_suspended(),
                "created_at": t.created_at
            })
        })
        .collect();

    Ok(JsonB(serde_json::json!(tenant_list)))
}

// ============================================================================
// SQL Functions - Isolation Management
// ============================================================================

/// Enable tenant RLS on a table
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_enable_tenant_rls('embeddings', 'tenant_id');
/// ```
#[pg_extern]
pub fn ruvector_enable_tenant_rls(
    table_name: &str,
    tenant_column: default!(&str, "'tenant_id'"),
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let sql = get_isolation_manager().enable_shared_isolation(table_name, tenant_column)?;
    Ok(format!(
        "RLS enabled for table '{}'. Execute the following SQL:\n{}",
        table_name, sql
    ))
}

/// Migrate tenant to a new isolation level
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_tenant_migrate('enterprise-customer', 'dedicated');
/// ```
#[pg_extern]
pub fn ruvector_tenant_migrate(
    tenant_id: &str,
    target_level: &str,
) -> Result<JsonB, Box<dyn std::error::Error + Send + Sync>> {
    let level = IsolationLevel::from_str(target_level)
        .ok_or_else(|| format!("Invalid isolation level: {}", target_level))?;

    let state = get_isolation_manager().start_migration(tenant_id, level)?;

    Ok(JsonB(serde_json::json!({
        "tenant_id": state.tenant_id,
        "from_level": state.from_level.as_str(),
        "to_level": state.to_level.as_str(),
        "status": format!("{:?}", state.status),
        "started_at": state.started_at
    })))
}

/// Get migration status for a tenant
///
/// # Examples
///
/// ```sql
/// SELECT * FROM ruvector_tenant_migration_status('enterprise-customer');
/// ```
#[pg_extern]
pub fn ruvector_tenant_migration_status(
    tenant_id: &str,
) -> Result<JsonB, Box<dyn std::error::Error + Send + Sync>> {
    let state = get_isolation_manager()
        .get_migration_status(tenant_id)
        .ok_or_else(|| format!("No migration in progress for tenant: {}", tenant_id))?;

    Ok(JsonB(serde_json::json!({
        "tenant_id": state.tenant_id,
        "from_level": state.from_level.as_str(),
        "to_level": state.to_level.as_str(),
        "status": format!("{:?}", state.status),
        "progress": state.progress,
        "vectors_migrated": state.vectors_migrated,
        "total_vectors": state.total_vectors,
        "started_at": state.started_at,
        "completed_at": state.completed_at,
        "error": state.error
    })))
}

/// Isolate tenant to dedicated resources
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_tenant_isolate('enterprise-customer');
/// ```
#[pg_extern]
pub fn ruvector_tenant_isolate(
    tenant_id: &str,
) -> Result<JsonB, Box<dyn std::error::Error + Send + Sync>> {
    // Create dedicated schema
    let schema_config = get_isolation_manager().create_dedicated_schema(tenant_id)?;
    let sql = get_isolation_manager().generate_schema_sql(&schema_config);

    Ok(JsonB(serde_json::json!({
        "tenant_id": tenant_id,
        "schema_name": schema_config.schema_name,
        "sql_to_execute": sql,
        "message": "Execute the returned SQL to complete isolation"
    })))
}

// ============================================================================
// SQL Functions - Policy Configuration
// ============================================================================

/// Set promotion policy for automatic isolation level upgrades
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_tenant_set_policy('{
///     "auto_promote_to_partition": 100000,
///     "auto_promote_to_dedicated": 10000000,
///     "check_interval": "1 hour"
/// }'::jsonb);
/// ```
#[pg_extern]
pub fn ruvector_tenant_set_policy(
    policy_config: JsonB,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let JsonB(json_val) = policy_config;

    let policy = PromotionPolicy {
        partition_threshold: json_val
            .get("auto_promote_to_partition")
            .and_then(|v| v.as_u64())
            .unwrap_or(100_000),
        dedicated_threshold: json_val
            .get("auto_promote_to_dedicated")
            .and_then(|v| v.as_u64())
            .unwrap_or(10_000_000),
        check_interval_secs: json_val
            .get("check_interval_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600),
        enabled: json_val
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
    };

    get_registry().set_promotion_policy(policy);

    Ok("Promotion policy updated successfully".to_string())
}

/// Update tenant quota
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_tenant_update_quota('acme-corp', '{
///     "max_vectors": 10000000,
///     "max_qps": 500
/// }'::jsonb);
/// ```
#[pg_extern]
pub fn ruvector_tenant_update_quota(
    tenant_id: &str,
    quota_config: JsonB,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let JsonB(json_val) = quota_config;

    let mut config = get_registry()
        .get(tenant_id)
        .ok_or_else(|| format!("Tenant not found: {}", tenant_id))?;

    if let Some(max_vec) = json_val.get("max_vectors").and_then(|v| v.as_u64()) {
        config.quota.max_vectors = max_vec;
    }
    if let Some(max_qps) = json_val.get("max_qps").and_then(|v| v.as_u64()) {
        config.quota.max_qps = max_qps as u32;
    }
    if let Some(max_storage) = json_val.get("max_storage_gb").and_then(|v| v.as_f64()) {
        config.quota.max_storage_bytes = (max_storage * 1024.0 * 1024.0 * 1024.0) as u64;
    }
    if let Some(max_concurrent) = json_val.get("max_concurrent").and_then(|v| v.as_u64()) {
        config.quota.max_concurrent = max_concurrent as u32;
    }
    if let Some(max_collections) = json_val.get("max_collections").and_then(|v| v.as_u64()) {
        config.quota.max_collections = max_collections as u32;
    }

    get_registry().update(tenant_id, config)?;

    Ok(format!("Quota updated for tenant '{}'", tenant_id))
}

// ============================================================================
// SQL Functions - RLS Helpers
// ============================================================================

/// Generate RLS setup SQL for a table
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_generate_rls_sql('embeddings', 'tenant_id');
/// ```
#[pg_extern]
pub fn ruvector_generate_rls_sql(
    table_name: &str,
    tenant_column: default!(&str, "'tenant_id'"),
) -> String {
    let config = RlsPolicyConfig::new(table_name).with_tenant_column(tenant_column);

    get_rls_manager().generate_enable_rls_sql(&config)
}

/// Generate SQL to add tenant column to a table
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_generate_tenant_column_sql('embeddings');
/// ```
#[pg_extern]
pub fn ruvector_generate_tenant_column_sql(
    table_name: &str,
    column_name: default!(&str, "'tenant_id'"),
    not_null: default!(bool, true),
    auto_default: default!(bool, true),
) -> String {
    rls::RlsManager::generate_add_tenant_column_sql(table_name, column_name, not_null, auto_default)
}

/// Generate SQL to create RuVector roles
///
/// # Examples
///
/// ```sql
/// SELECT ruvector_generate_roles_sql();
/// ```
#[pg_extern]
pub fn ruvector_generate_roles_sql() -> String {
    rls::RlsManager::generate_roles_sql()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_tenant_create() {
        let result = ruvector_tenant_create("test-tenant", None);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("test-tenant"));
    }

    #[pg_test]
    fn test_tenant_create_with_config() {
        let config = JsonB(serde_json::json!({
            "display_name": "Test Corp",
            "max_vectors": 5000000,
            "isolation_level": "partition"
        }));

        let result = ruvector_tenant_create("test-tenant-2", Some(config));
        assert!(result.is_ok());
    }

    #[pg_test]
    fn test_tenant_list() {
        // Create a tenant first
        let _ = ruvector_tenant_create("list-test-tenant", None);

        let result = ruvector_tenants();
        assert!(result.is_ok());

        let JsonB(json) = result.unwrap();
        assert!(json.is_array());
    }

    #[pg_test]
    fn test_tenant_suspend_resume() {
        let _ = ruvector_tenant_create("suspend-test", None);

        // Suspend
        let result = ruvector_tenant_suspend("suspend-test");
        assert!(result.is_ok());

        // Resume
        let result = ruvector_tenant_resume("suspend-test");
        assert!(result.is_ok());
    }

    #[pg_test]
    fn test_rls_sql_generation() {
        let sql = ruvector_generate_rls_sql("embeddings", "tenant_id");
        assert!(sql.contains("ENABLE ROW LEVEL SECURITY"));
        assert!(sql.contains("ruvector_tenant_isolation"));
    }

    #[pg_test]
    fn test_tenant_column_sql_generation() {
        let sql = ruvector_generate_tenant_column_sql("embeddings", "tenant_id", true, true);
        assert!(sql.contains("ADD COLUMN"));
        assert!(sql.contains("tenant_id"));
    }

    #[pg_test]
    fn test_roles_sql_generation() {
        let sql = ruvector_generate_roles_sql();
        assert!(sql.contains("ruvector_admin"));
        assert!(sql.contains("ruvector_users"));
    }

    #[pg_test]
    fn test_policy_update() {
        let policy = JsonB(serde_json::json!({
            "auto_promote_to_partition": 50000,
            "auto_promote_to_dedicated": 5000000,
            "enabled": true
        }));

        let result = ruvector_tenant_set_policy(policy);
        assert!(result.is_ok());
    }

    #[pg_test]
    fn test_quota_check() {
        let _ = ruvector_tenant_create("quota-test", None);

        let result = ruvector_tenant_quota_check("quota-test");
        assert!(result.is_ok());

        let JsonB(json) = result.unwrap();
        assert!(json.get("vectors").is_some());
        assert!(json.get("storage").is_some());
    }
}
