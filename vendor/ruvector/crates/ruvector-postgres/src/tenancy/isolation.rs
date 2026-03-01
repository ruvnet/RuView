//! Tenant Isolation Enforcement for RuVector Multi-Tenancy
//!
//! Provides three isolation levels:
//! - Shared: RLS policies on tenant_id column
//! - Partition: Separate partitions per tenant
//! - Dedicated: Schema-level isolation with separate indexes

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use super::registry::{get_registry, IsolationLevel};
use super::validation::{
    escape_string_literal, quote_identifier, safe_partition_name, safe_schema_name,
    validate_identifier, validate_tenant_id,
};

/// Partition configuration for tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionConfig {
    /// Tenant ID
    pub tenant_id: String,
    /// Partition name (e.g., "embeddings_acme_corp")
    pub partition_name: String,
    /// Parent table name
    pub parent_table: String,
    /// Partition key value (tenant_id)
    pub partition_key: String,
    /// Creation timestamp
    pub created_at: i64,
}

/// Dedicated schema configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedicatedSchemaConfig {
    /// Tenant ID
    pub tenant_id: String,
    /// Schema name (e.g., "tenant_acme_corp")
    pub schema_name: String,
    /// Tables in this schema
    pub tables: Vec<String>,
    /// Indexes in this schema
    pub indexes: Vec<String>,
    /// Creation timestamp
    pub created_at: i64,
}

/// Isolation enforcement manager
pub struct IsolationManager {
    /// Partition configurations by tenant
    partitions: DashMap<String, Vec<PartitionConfig>>,
    /// Dedicated schema configurations by tenant
    dedicated_schemas: DashMap<String, DedicatedSchemaConfig>,
    /// Tables with RLS enabled (table_name -> tenant_column)
    rls_tables: DashMap<String, String>,
    /// Migration state tracking
    migration_state: DashMap<String, MigrationState>,
}

/// State of tenant isolation migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationState {
    /// Tenant ID
    pub tenant_id: String,
    /// Source isolation level
    pub from_level: IsolationLevel,
    /// Target isolation level
    pub to_level: IsolationLevel,
    /// Migration status
    pub status: MigrationStatus,
    /// Progress percentage (0-100)
    pub progress: u8,
    /// Vectors migrated so far
    pub vectors_migrated: u64,
    /// Total vectors to migrate
    pub total_vectors: u64,
    /// Start timestamp
    pub started_at: i64,
    /// Completion timestamp
    pub completed_at: Option<i64>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Migration status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationStatus {
    /// Migration pending
    Pending,
    /// Migration in progress
    InProgress,
    /// Migration completed
    Completed,
    /// Migration failed
    Failed,
    /// Migration cancelled
    Cancelled,
}

impl IsolationManager {
    /// Create a new isolation manager
    pub fn new() -> Self {
        Self {
            partitions: DashMap::new(),
            dedicated_schemas: DashMap::new(),
            rls_tables: DashMap::new(),
            migration_state: DashMap::new(),
        }
    }

    // =========================================================================
    // Shared Isolation (RLS-based)
    // =========================================================================

    /// Enable shared isolation for a table (RLS policies)
    pub fn enable_shared_isolation(
        &self,
        table_name: &str,
        tenant_column: &str,
    ) -> Result<String, IsolationError> {
        // Validate identifiers to prevent SQL injection
        validate_identifier(table_name)
            .map_err(|e| IsolationError::SqlError(format!("Invalid table name: {}", e)))?;
        validate_identifier(tenant_column)
            .map_err(|e| IsolationError::SqlError(format!("Invalid column name: {}", e)))?;

        // Use quoted identifiers for safety
        let quoted_table = quote_identifier(table_name);
        let quoted_column = quote_identifier(tenant_column);

        // Generate SQL for RLS setup with quoted identifiers
        let sql = format!(
            r#"
-- Enable RLS on the table
ALTER TABLE {table} ENABLE ROW LEVEL SECURITY;

-- Drop existing policies if any
DROP POLICY IF EXISTS ruvector_tenant_isolation ON {table};
DROP POLICY IF EXISTS ruvector_admin_bypass ON {table};

-- Create tenant isolation policy
CREATE POLICY ruvector_tenant_isolation ON {table}
    USING ({column} = current_setting('ruvector.tenant_id', true))
    WITH CHECK ({column} = current_setting('ruvector.tenant_id', true));

-- Create admin bypass policy (for ruvector_admin role)
CREATE POLICY ruvector_admin_bypass ON {table}
    FOR ALL
    TO ruvector_admin
    USING (true)
    WITH CHECK (true);

-- Create wildcard policy for admin queries
CREATE POLICY ruvector_admin_wildcard ON {table}
    FOR SELECT
    USING (current_setting('ruvector.tenant_id', true) = '*');
"#,
            table = quoted_table,
            column = quoted_column
        );

        self.rls_tables
            .insert(table_name.to_string(), tenant_column.to_string());

        Ok(sql)
    }

    /// Check if a table has RLS enabled
    pub fn is_rls_enabled(&self, table_name: &str) -> bool {
        self.rls_tables.contains_key(table_name)
    }

    /// Get tenant column for RLS table
    pub fn get_tenant_column(&self, table_name: &str) -> Option<String> {
        self.rls_tables.get(table_name).map(|r| r.value().clone())
    }

    // =========================================================================
    // Partition Isolation
    // =========================================================================

    /// Create partition for a tenant
    pub fn create_partition(
        &self,
        tenant_id: &str,
        parent_table: &str,
    ) -> Result<PartitionConfig, IsolationError> {
        // Validate inputs to prevent SQL injection
        validate_tenant_id(tenant_id)
            .map_err(|e| IsolationError::SqlError(format!("Invalid tenant ID: {}", e)))?;
        validate_identifier(parent_table)
            .map_err(|e| IsolationError::SqlError(format!("Invalid table name: {}", e)))?;

        // Generate safe partition name
        let partition_name = safe_partition_name(tenant_id, parent_table)
            .map_err(|e| IsolationError::SqlError(format!("Invalid partition name: {}", e)))?;

        let config = PartitionConfig {
            tenant_id: tenant_id.to_string(),
            partition_name,
            parent_table: parent_table.to_string(),
            partition_key: tenant_id.to_string(),
            created_at: chrono_now_millis(),
        };

        // Store partition config
        self.partitions
            .entry(tenant_id.to_string())
            .or_insert_with(Vec::new)
            .push(config.clone());

        Ok(config)
    }

    /// Generate SQL for creating a partition
    pub fn generate_partition_sql(&self, config: &PartitionConfig) -> String {
        // Use quoted identifiers for safety
        let quoted_partition = quote_identifier(&config.partition_name);
        let quoted_parent = quote_identifier(&config.parent_table);
        let escaped_tenant_id = escape_string_literal(&config.partition_key);
        let safe_index_name = format!("idx_{}_vec", config.partition_name);

        format!(
            r#"
-- Create partition for tenant
CREATE TABLE IF NOT EXISTS {partition} PARTITION OF {parent}
    FOR VALUES IN ('{tenant_id}');

-- Create indexes on partition
CREATE INDEX IF NOT EXISTS {index_name}
    ON {partition} USING ruhnsw (vec vector_cosine_ops);
"#,
            partition = quoted_partition,
            parent = quoted_parent,
            tenant_id = escaped_tenant_id,
            index_name = quote_identifier(&safe_index_name)
        )
    }

    /// Get partitions for a tenant
    pub fn get_partitions(&self, tenant_id: &str) -> Vec<PartitionConfig> {
        self.partitions
            .get(tenant_id)
            .map(|r| r.value().clone())
            .unwrap_or_default()
    }

    /// Drop partition for a tenant
    pub fn drop_partition(
        &self,
        tenant_id: &str,
        partition_name: &str,
    ) -> Result<String, IsolationError> {
        // Validate inputs to prevent SQL injection
        validate_tenant_id(tenant_id)
            .map_err(|e| IsolationError::SqlError(format!("Invalid tenant ID: {}", e)))?;
        validate_identifier(partition_name)
            .map_err(|e| IsolationError::SqlError(format!("Invalid partition name: {}", e)))?;

        // Verify partition belongs to this tenant (security check)
        let partition_exists = self
            .partitions
            .get(tenant_id)
            .map(|partitions| {
                partitions
                    .iter()
                    .any(|p| p.partition_name == partition_name)
            })
            .unwrap_or(false);

        if !partition_exists {
            return Err(IsolationError::PartitionNotFound(
                partition_name.to_string(),
            ));
        }

        // Remove from tracking
        if let Some(mut partitions) = self.partitions.get_mut(tenant_id) {
            partitions.retain(|p| p.partition_name != partition_name);
        }

        // Use quoted identifier for safety
        Ok(format!(
            "DROP TABLE IF EXISTS {} CASCADE;",
            quote_identifier(partition_name)
        ))
    }

    // =========================================================================
    // Dedicated Isolation (Schema-level)
    // =========================================================================

    /// Create dedicated schema for a tenant
    pub fn create_dedicated_schema(
        &self,
        tenant_id: &str,
    ) -> Result<DedicatedSchemaConfig, IsolationError> {
        // Validate tenant ID to prevent SQL injection
        validate_tenant_id(tenant_id)
            .map_err(|e| IsolationError::SqlError(format!("Invalid tenant ID: {}", e)))?;

        // Generate safe schema name
        let schema_name = safe_schema_name(tenant_id)
            .map_err(|e| IsolationError::SqlError(format!("Invalid schema name: {}", e)))?;

        let config = DedicatedSchemaConfig {
            tenant_id: tenant_id.to_string(),
            schema_name,
            tables: Vec::new(),
            indexes: Vec::new(),
            created_at: chrono_now_millis(),
        };

        self.dedicated_schemas
            .insert(tenant_id.to_string(), config.clone());

        Ok(config)
    }

    /// Generate SQL for creating dedicated schema
    pub fn generate_schema_sql(&self, config: &DedicatedSchemaConfig) -> String {
        // Use quoted identifiers for safety
        let quoted_schema = quote_identifier(&config.schema_name);
        let index_name = format!("idx_{}_embeddings_vec", config.schema_name);
        let quoted_index = quote_identifier(&index_name);

        format!(
            r#"
-- Create dedicated schema for tenant
CREATE SCHEMA IF NOT EXISTS {schema};

-- Set search path to include tenant schema
-- (Application should SET search_path = {schema}, public;)

-- Create embeddings table in tenant schema
CREATE TABLE IF NOT EXISTS {schema}."embeddings" (
    id          BIGSERIAL PRIMARY KEY,
    content     TEXT,
    vec         vector(1536),
    metadata    JSONB DEFAULT '{{}}',
    created_at  TIMESTAMPTZ DEFAULT NOW()
);

-- Create HNSW index
CREATE INDEX IF NOT EXISTS {index_name}
    ON {schema}."embeddings" USING ruhnsw (vec vector_cosine_ops);

-- Grant usage to tenant role
GRANT USAGE ON SCHEMA {schema} TO ruvector_users;
GRANT ALL ON ALL TABLES IN SCHEMA {schema} TO ruvector_users;
GRANT ALL ON ALL SEQUENCES IN SCHEMA {schema} TO ruvector_users;
"#,
            schema = quoted_schema,
            index_name = quoted_index
        )
    }

    /// Get dedicated schema for a tenant
    pub fn get_dedicated_schema(&self, tenant_id: &str) -> Option<DedicatedSchemaConfig> {
        self.dedicated_schemas
            .get(tenant_id)
            .map(|r| r.value().clone())
    }

    /// Add table to dedicated schema tracking
    pub fn register_schema_table(
        &self,
        tenant_id: &str,
        table_name: &str,
    ) -> Result<(), IsolationError> {
        if let Some(mut schema) = self.dedicated_schemas.get_mut(tenant_id) {
            schema.tables.push(table_name.to_string());
            Ok(())
        } else {
            Err(IsolationError::SchemaNotFound(tenant_id.to_string()))
        }
    }

    /// Add index to dedicated schema tracking
    pub fn register_schema_index(
        &self,
        tenant_id: &str,
        index_name: &str,
    ) -> Result<(), IsolationError> {
        if let Some(mut schema) = self.dedicated_schemas.get_mut(tenant_id) {
            schema.indexes.push(index_name.to_string());
            Ok(())
        } else {
            Err(IsolationError::SchemaNotFound(tenant_id.to_string()))
        }
    }

    /// Drop dedicated schema
    pub fn drop_dedicated_schema(
        &self,
        tenant_id: &str,
        cascade: bool,
    ) -> Result<String, IsolationError> {
        // Validate tenant ID
        validate_tenant_id(tenant_id)
            .map_err(|e| IsolationError::SqlError(format!("Invalid tenant ID: {}", e)))?;

        let config = self
            .dedicated_schemas
            .remove(tenant_id)
            .map(|(_, v)| v)
            .ok_or_else(|| IsolationError::SchemaNotFound(tenant_id.to_string()))?;

        let cascade_clause = if cascade { "CASCADE" } else { "RESTRICT" };

        // Use quoted identifier for safety
        Ok(format!(
            "DROP SCHEMA IF EXISTS {} {};",
            quote_identifier(&config.schema_name),
            cascade_clause
        ))
    }

    // =========================================================================
    // Migration Between Isolation Levels
    // =========================================================================

    /// Start migration to a new isolation level
    pub fn start_migration(
        &self,
        tenant_id: &str,
        target_level: IsolationLevel,
    ) -> Result<MigrationState, IsolationError> {
        // Check if migration already in progress
        if let Some(state) = self.migration_state.get(tenant_id) {
            if state.status == MigrationStatus::InProgress {
                return Err(IsolationError::MigrationInProgress(tenant_id.to_string()));
            }
        }

        // Get current tenant config
        let config = get_registry()
            .get(tenant_id)
            .ok_or_else(|| IsolationError::TenantNotFound(tenant_id.to_string()))?;

        let state = MigrationState {
            tenant_id: tenant_id.to_string(),
            from_level: config.isolation_level,
            to_level: target_level,
            status: MigrationStatus::Pending,
            progress: 0,
            vectors_migrated: 0,
            total_vectors: 0, // Will be set during migration
            started_at: chrono_now_millis(),
            completed_at: None,
            error: None,
        };

        self.migration_state
            .insert(tenant_id.to_string(), state.clone());

        // Mark tenant as migrating
        if let Some(shared_state) = get_registry().get_shared_state(tenant_id) {
            shared_state.set_migrating(true);
        }

        Ok(state)
    }

    /// Update migration progress
    pub fn update_migration_progress(
        &self,
        tenant_id: &str,
        vectors_migrated: u64,
        total_vectors: u64,
    ) -> Result<(), IsolationError> {
        let mut state = self
            .migration_state
            .get_mut(tenant_id)
            .ok_or_else(|| IsolationError::NoMigrationInProgress(tenant_id.to_string()))?;

        state.vectors_migrated = vectors_migrated;
        state.total_vectors = total_vectors;
        state.progress = if total_vectors > 0 {
            ((vectors_migrated as f64 / total_vectors as f64) * 100.0) as u8
        } else {
            100
        };
        state.status = MigrationStatus::InProgress;

        Ok(())
    }

    /// Complete migration
    pub fn complete_migration(&self, tenant_id: &str) -> Result<MigrationState, IsolationError> {
        let mut state = self
            .migration_state
            .get_mut(tenant_id)
            .ok_or_else(|| IsolationError::NoMigrationInProgress(tenant_id.to_string()))?;

        state.status = MigrationStatus::Completed;
        state.progress = 100;
        state.completed_at = Some(chrono_now_millis());

        // Clear migrating flag
        if let Some(shared_state) = get_registry().get_shared_state(tenant_id) {
            shared_state.set_migrating(false);
        }

        Ok(state.clone())
    }

    /// Fail migration
    pub fn fail_migration(&self, tenant_id: &str, error: &str) -> Result<(), IsolationError> {
        let mut state = self
            .migration_state
            .get_mut(tenant_id)
            .ok_or_else(|| IsolationError::NoMigrationInProgress(tenant_id.to_string()))?;

        state.status = MigrationStatus::Failed;
        state.error = Some(error.to_string());
        state.completed_at = Some(chrono_now_millis());

        // Clear migrating flag
        if let Some(shared_state) = get_registry().get_shared_state(tenant_id) {
            shared_state.set_migrating(false);
        }

        Ok(())
    }

    /// Get migration status
    pub fn get_migration_status(&self, tenant_id: &str) -> Option<MigrationState> {
        self.migration_state
            .get(tenant_id)
            .map(|r| r.value().clone())
    }

    // =========================================================================
    // Query Routing
    // =========================================================================

    /// Get the appropriate table/schema for a tenant's query
    ///
    /// Returns a QueryRoute that uses parameterized placeholders ($1) instead of
    /// directly interpolating tenant_id values to prevent SQL injection.
    pub fn route_query(&self, tenant_id: &str, base_table: &str) -> QueryRoute {
        // Validate tenant_id to prevent SQL injection even when using parameterized queries
        // This provides defense-in-depth
        if validate_tenant_id(tenant_id).is_err() {
            // Invalid tenant_id - return a safe filter that will match nothing
            return QueryRoute::SharedWithFilter {
                table: base_table.to_string(),
                filter: "false".to_string(), // Safe - matches nothing
                tenant_param: None,
            };
        }

        let config = match get_registry().get(tenant_id) {
            Some(c) => c,
            None => {
                return QueryRoute::SharedWithFilter {
                    table: base_table.to_string(),
                    // Use parameterized query placeholder - caller must bind tenant_id
                    filter: "tenant_id = $1".to_string(),
                    tenant_param: Some(tenant_id.to_string()),
                };
            }
        };

        match config.isolation_level {
            IsolationLevel::Shared => QueryRoute::SharedWithFilter {
                table: base_table.to_string(),
                // Use parameterized query placeholder
                filter: "tenant_id = $1".to_string(),
                tenant_param: Some(tenant_id.to_string()),
            },
            IsolationLevel::Partition => {
                // Check if partition exists
                if let Some(partitions) = self.partitions.get(tenant_id) {
                    if let Some(partition) =
                        partitions.iter().find(|p| p.parent_table == base_table)
                    {
                        return QueryRoute::Partition {
                            partition_table: partition.partition_name.clone(),
                        };
                    }
                }
                // Fall back to shared with filter (parameterized)
                QueryRoute::SharedWithFilter {
                    table: base_table.to_string(),
                    filter: "tenant_id = $1".to_string(),
                    tenant_param: Some(tenant_id.to_string()),
                }
            }
            IsolationLevel::Dedicated => {
                // Check if dedicated schema exists
                if let Some(schema) = self.dedicated_schemas.get(tenant_id) {
                    return QueryRoute::DedicatedSchema {
                        schema: schema.schema_name.clone(),
                        table: base_table.to_string(),
                    };
                }
                // Fall back to shared with filter (parameterized)
                QueryRoute::SharedWithFilter {
                    table: base_table.to_string(),
                    filter: "tenant_id = $1".to_string(),
                    tenant_param: Some(tenant_id.to_string()),
                }
            }
        }
    }
}

impl Default for IsolationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Query routing result
#[derive(Debug, Clone)]
pub enum QueryRoute {
    /// Use shared table with tenant filter (RLS handles this automatically)
    ///
    /// The filter uses parameterized query placeholders ($1) for safety.
    /// The tenant_param contains the actual value to bind.
    SharedWithFilter {
        table: String,
        /// SQL filter clause using $1 placeholder for tenant_id
        filter: String,
        /// The tenant_id value to bind to $1 (None if filter is static like "false")
        tenant_param: Option<String>,
    },
    /// Use dedicated partition table
    Partition { partition_table: String },
    /// Use dedicated schema
    DedicatedSchema { schema: String, table: String },
}

impl QueryRoute {
    /// Get the full table reference for SQL
    pub fn table_reference(&self) -> String {
        match self {
            Self::SharedWithFilter { table, .. } => table.clone(),
            Self::Partition { partition_table } => partition_table.clone(),
            Self::DedicatedSchema { schema, table } => {
                format!("{}.{}", quote_identifier(schema), quote_identifier(table))
            }
        }
    }

    /// Get additional WHERE clause if needed (parameterized)
    ///
    /// Returns the filter clause and the parameter value to bind.
    /// The filter uses $1 placeholder for the tenant_id.
    pub fn where_clause(&self) -> Option<String> {
        match self {
            Self::SharedWithFilter { filter, .. } => Some(filter.clone()),
            _ => None,
        }
    }

    /// Get the tenant parameter value to bind to $1
    pub fn tenant_param(&self) -> Option<String> {
        match self {
            Self::SharedWithFilter { tenant_param, .. } => tenant_param.clone(),
            _ => None,
        }
    }

    /// Get WHERE clause and parameter together for convenience
    pub fn where_clause_with_param(&self) -> Option<(String, Option<String>)> {
        match self {
            Self::SharedWithFilter {
                filter,
                tenant_param,
                ..
            } => Some((filter.clone(), tenant_param.clone())),
            _ => None,
        }
    }
}

/// Isolation operation errors
#[derive(Debug, Clone)]
pub enum IsolationError {
    /// Tenant not found
    TenantNotFound(String),
    /// Schema not found
    SchemaNotFound(String),
    /// Partition not found
    PartitionNotFound(String),
    /// Migration already in progress
    MigrationInProgress(String),
    /// No migration in progress
    NoMigrationInProgress(String),
    /// Invalid isolation level transition
    InvalidTransition {
        from: IsolationLevel,
        to: IsolationLevel,
    },
    /// SQL execution error
    SqlError(String),
}

impl std::fmt::Display for IsolationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TenantNotFound(id) => write!(f, "Tenant not found: {}", id),
            Self::SchemaNotFound(id) => write!(f, "Dedicated schema not found for tenant: {}", id),
            Self::PartitionNotFound(name) => write!(f, "Partition not found: {}", name),
            Self::MigrationInProgress(id) => {
                write!(f, "Migration already in progress for tenant: {}", id)
            }
            Self::NoMigrationInProgress(id) => {
                write!(f, "No migration in progress for tenant: {}", id)
            }
            Self::InvalidTransition { from, to } => {
                write!(
                    f,
                    "Invalid isolation transition from {} to {}",
                    from.as_str(),
                    to.as_str()
                )
            }
            Self::SqlError(msg) => write!(f, "SQL error: {}", msg),
        }
    }
}

impl std::error::Error for IsolationError {}

/// Global isolation manager instance
static ISOLATION_MANAGER: once_cell::sync::Lazy<IsolationManager> =
    once_cell::sync::Lazy::new(IsolationManager::new);

/// Get the global isolation manager
pub fn get_isolation_manager() -> &'static IsolationManager {
    &ISOLATION_MANAGER
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
    fn test_create_partition_config() {
        let manager = IsolationManager::new();
        let config = manager.create_partition("acme-corp", "embeddings").unwrap();

        assert_eq!(config.tenant_id, "acme-corp");
        assert_eq!(config.partition_name, "embeddings_acme_corp");
        assert_eq!(config.parent_table, "embeddings");
    }

    #[test]
    fn test_create_dedicated_schema() {
        let manager = IsolationManager::new();
        let config = manager.create_dedicated_schema("acme-corp").unwrap();

        assert_eq!(config.tenant_id, "acme-corp");
        assert_eq!(config.schema_name, "tenant_acme_corp");
    }

    #[test]
    fn test_query_routing() {
        let manager = IsolationManager::new();

        // Default routing (no config) should use shared with filter
        let route = manager.route_query("unknown_tenant", "embeddings");
        match route {
            QueryRoute::SharedWithFilter {
                table,
                filter,
                tenant_param,
            } => {
                assert_eq!(table, "embeddings");
                // Filter should use parameterized placeholder
                assert_eq!(filter, "tenant_id = $1");
                // Tenant param should contain the tenant_id
                assert_eq!(tenant_param, Some("unknown_tenant".to_string()));
            }
            _ => panic!("Expected SharedWithFilter"),
        }
    }

    #[test]
    fn test_query_routing_invalid_tenant() {
        let manager = IsolationManager::new();

        // Invalid tenant_id should return safe "false" filter
        let route = manager.route_query("'; DROP TABLE users;--", "embeddings");
        match route {
            QueryRoute::SharedWithFilter {
                filter,
                tenant_param,
                ..
            } => {
                assert_eq!(filter, "false");
                assert!(tenant_param.is_none());
            }
            _ => panic!("Expected SharedWithFilter with false filter"),
        }
    }

    #[test]
    fn test_rls_tracking() {
        let manager = IsolationManager::new();

        // Enable RLS
        manager
            .enable_shared_isolation("embeddings", "tenant_id")
            .unwrap();

        // Check tracking
        assert!(manager.is_rls_enabled("embeddings"));
        assert_eq!(
            manager.get_tenant_column("embeddings"),
            Some("tenant_id".to_string())
        );
        assert!(!manager.is_rls_enabled("other_table"));
    }

    #[test]
    fn test_migration_state() {
        let manager = IsolationManager::new();

        // Register a tenant first
        let registry = get_registry();
        let config = super::super::registry::TenantConfig::new("test-tenant".to_string());
        let _ = registry.register(config);

        // Start migration
        let state = manager
            .start_migration("test-tenant", IsolationLevel::Partition)
            .unwrap();
        assert_eq!(state.status, MigrationStatus::Pending);
        assert_eq!(state.from_level, IsolationLevel::Shared);
        assert_eq!(state.to_level, IsolationLevel::Partition);

        // Should fail if trying to start another migration
        let result = manager.start_migration("test-tenant", IsolationLevel::Dedicated);
        assert!(result.is_err());

        // Update progress
        manager
            .update_migration_progress("test-tenant", 50, 100)
            .unwrap();
        let state = manager.get_migration_status("test-tenant").unwrap();
        assert_eq!(state.progress, 50);

        // Complete migration
        let state = manager.complete_migration("test-tenant").unwrap();
        assert_eq!(state.status, MigrationStatus::Completed);
        assert_eq!(state.progress, 100);
    }
}
