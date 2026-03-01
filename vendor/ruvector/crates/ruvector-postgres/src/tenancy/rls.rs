//! Row-Level Security Integration for RuVector Multi-Tenancy
//!
//! Provides automatic RLS policy generation and management for tenant isolation.
//! Integrates with PostgreSQL's native RLS capabilities.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

/// RLS policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlsPolicyConfig {
    /// Table name (fully qualified)
    pub table_name: String,
    /// Tenant ID column name
    pub tenant_column: String,
    /// Policy name
    pub policy_name: String,
    /// Whether to create admin bypass policy
    pub admin_bypass: bool,
    /// Whether to create wildcard policy for admin queries
    pub wildcard_policy: bool,
    /// Custom USING clause (optional)
    pub custom_using: Option<String>,
    /// Custom WITH CHECK clause (optional)
    pub custom_with_check: Option<String>,
}

impl Default for RlsPolicyConfig {
    fn default() -> Self {
        Self {
            table_name: String::new(),
            tenant_column: "tenant_id".to_string(),
            policy_name: "ruvector_tenant_isolation".to_string(),
            admin_bypass: true,
            wildcard_policy: true,
            custom_using: None,
            custom_with_check: None,
        }
    }
}

impl RlsPolicyConfig {
    /// Create a new RLS policy config for a table
    pub fn new(table_name: &str) -> Self {
        Self {
            table_name: table_name.to_string(),
            ..Default::default()
        }
    }

    /// Set tenant column name
    pub fn with_tenant_column(mut self, column: &str) -> Self {
        self.tenant_column = column.to_string();
        self
    }

    /// Set policy name
    pub fn with_policy_name(mut self, name: &str) -> Self {
        self.policy_name = name.to_string();
        self
    }

    /// Disable admin bypass
    pub fn without_admin_bypass(mut self) -> Self {
        self.admin_bypass = false;
        self
    }

    /// Disable wildcard policy
    pub fn without_wildcard(mut self) -> Self {
        self.wildcard_policy = false;
        self
    }

    /// Set custom USING clause
    pub fn with_custom_using(mut self, using: &str) -> Self {
        self.custom_using = Some(using.to_string());
        self
    }

    /// Set custom WITH CHECK clause
    pub fn with_custom_check(mut self, check: &str) -> Self {
        self.custom_with_check = Some(check.to_string());
        self
    }
}

/// Policy template for common patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyTemplate {
    /// Standard tenant isolation (tenant_id = current_setting)
    Standard,
    /// Read-only for other tenants with write for own
    ReadSharedWriteOwn,
    /// Hierarchical (tenant can see child tenants)
    Hierarchical {
        /// Path column for hierarchy (e.g., "tenant_path")
        path_column: String,
    },
    /// Time-based access (tenant_id + time window)
    TimeBased {
        /// Timestamp column
        time_column: String,
        /// Retention period in days
        retention_days: i32,
    },
    /// Custom template
    Custom {
        /// Custom USING expression
        using_expr: String,
        /// Custom WITH CHECK expression
        check_expr: String,
    },
}

impl PolicyTemplate {
    /// Generate USING clause for this template
    pub fn using_clause(&self, tenant_column: &str) -> String {
        match self {
            Self::Standard => {
                format!(
                    "{} = current_setting('ruvector.tenant_id', true)",
                    tenant_column
                )
            }
            Self::ReadSharedWriteOwn => {
                format!(
                    "{} = current_setting('ruvector.tenant_id', true) OR is_public = true",
                    tenant_column
                )
            }
            Self::Hierarchical { path_column } => {
                format!(
                    "{} LIKE current_setting('ruvector.tenant_id', true) || '%'",
                    path_column
                )
            }
            Self::TimeBased {
                time_column,
                retention_days,
            } => {
                format!(
                    "{} = current_setting('ruvector.tenant_id', true) AND {} > NOW() - INTERVAL '{} days'",
                    tenant_column, time_column, retention_days
                )
            }
            Self::Custom { using_expr, .. } => using_expr.clone(),
        }
    }

    /// Generate WITH CHECK clause for this template
    pub fn check_clause(&self, tenant_column: &str) -> String {
        match self {
            Self::Standard | Self::ReadSharedWriteOwn => {
                format!(
                    "{} = current_setting('ruvector.tenant_id', true)",
                    tenant_column
                )
            }
            Self::Hierarchical { .. } => {
                format!(
                    "{} = current_setting('ruvector.tenant_id', true)",
                    tenant_column
                )
            }
            Self::TimeBased {
                time_column: _,
                retention_days: _,
            } => {
                format!(
                    "{} = current_setting('ruvector.tenant_id', true)",
                    tenant_column
                )
            }
            Self::Custom { check_expr, .. } => check_expr.clone(),
        }
    }
}

/// RLS policy manager
pub struct RlsManager {
    /// Active policies by table
    policies: DashMap<String, RlsPolicyConfig>,
    /// Tables with RLS enabled
    enabled_tables: DashMap<String, bool>,
}

impl RlsManager {
    /// Create a new RLS manager
    pub fn new() -> Self {
        Self {
            policies: DashMap::new(),
            enabled_tables: DashMap::new(),
        }
    }

    /// Generate SQL to enable RLS on a table with tenant isolation
    pub fn generate_enable_rls_sql(&self, config: &RlsPolicyConfig) -> String {
        let using_clause = config.custom_using.clone().unwrap_or_else(|| {
            format!(
                "{} = current_setting('ruvector.tenant_id', true)",
                config.tenant_column
            )
        });

        let check_clause = config.custom_with_check.clone().unwrap_or_else(|| {
            format!(
                "{} = current_setting('ruvector.tenant_id', true)",
                config.tenant_column
            )
        });

        let mut sql = format!(
            r#"
-- Enable Row-Level Security on {table}
ALTER TABLE {table} ENABLE ROW LEVEL SECURITY;

-- Force RLS even for table owners (recommended for security)
ALTER TABLE {table} FORCE ROW LEVEL SECURITY;

-- Drop existing RuVector policies if any
DROP POLICY IF EXISTS {policy} ON {table};
DROP POLICY IF EXISTS {policy}_admin ON {table};
DROP POLICY IF EXISTS {policy}_wildcard ON {table};

-- Create tenant isolation policy (applies to all operations)
CREATE POLICY {policy} ON {table}
    FOR ALL
    USING ({using})
    WITH CHECK ({check});
"#,
            table = config.table_name,
            policy = config.policy_name,
            using = using_clause,
            check = check_clause
        );

        if config.admin_bypass {
            sql.push_str(&format!(
                r#"
-- Create admin bypass policy
-- Requires role: ruvector_admin
CREATE POLICY {policy}_admin ON {table}
    FOR ALL
    TO ruvector_admin
    USING (true)
    WITH CHECK (true);
"#,
                table = config.table_name,
                policy = config.policy_name
            ));
        }

        if config.wildcard_policy {
            sql.push_str(&format!(
                r#"
-- Create wildcard policy for cross-tenant admin queries
-- Only applies when tenant_id is set to '*'
CREATE POLICY {policy}_wildcard ON {table}
    FOR SELECT
    USING (current_setting('ruvector.tenant_id', true) = '*');
"#,
                table = config.table_name,
                policy = config.policy_name
            ));
        }

        sql
    }

    /// Generate SQL to disable RLS on a table
    pub fn generate_disable_rls_sql(&self, table_name: &str) -> String {
        format!(
            r#"
-- Disable Row-Level Security on {table}
ALTER TABLE {table} NO FORCE ROW LEVEL SECURITY;
ALTER TABLE {table} DISABLE ROW LEVEL SECURITY;

-- Drop all RuVector policies
DROP POLICY IF EXISTS ruvector_tenant_isolation ON {table};
DROP POLICY IF EXISTS ruvector_tenant_isolation_admin ON {table};
DROP POLICY IF EXISTS ruvector_tenant_isolation_wildcard ON {table};
"#,
            table = table_name
        )
    }

    /// Generate SQL using a policy template
    pub fn generate_from_template(
        &self,
        table_name: &str,
        tenant_column: &str,
        template: &PolicyTemplate,
    ) -> String {
        let using_clause = template.using_clause(tenant_column);
        let check_clause = template.check_clause(tenant_column);

        let config = RlsPolicyConfig {
            table_name: table_name.to_string(),
            tenant_column: tenant_column.to_string(),
            custom_using: Some(using_clause),
            custom_with_check: Some(check_clause),
            ..Default::default()
        };

        self.generate_enable_rls_sql(&config)
    }

    /// Generate SQL to set tenant context for a session
    pub fn generate_set_tenant_sql(tenant_id: &str, local: bool) -> String {
        let set_cmd = if local { "SET LOCAL" } else { "SET" };
        format!("{} ruvector.tenant_id = '{}';", set_cmd, tenant_id)
    }

    /// Generate SQL to clear tenant context
    pub fn generate_clear_tenant_sql() -> String {
        "RESET ruvector.tenant_id;".to_string()
    }

    /// Generate SQL to get current tenant context
    pub fn generate_get_tenant_sql() -> String {
        "SELECT current_setting('ruvector.tenant_id', true);".to_string()
    }

    /// Register a policy configuration
    pub fn register_policy(&self, config: RlsPolicyConfig) {
        let table_name = config.table_name.clone();
        self.policies.insert(table_name.clone(), config);
        self.enabled_tables.insert(table_name, true);
    }

    /// Get policy for a table
    pub fn get_policy(&self, table_name: &str) -> Option<RlsPolicyConfig> {
        self.policies.get(table_name).map(|r| r.value().clone())
    }

    /// Check if RLS is enabled for a table
    pub fn is_enabled(&self, table_name: &str) -> bool {
        self.enabled_tables
            .get(table_name)
            .map(|r| *r.value())
            .unwrap_or(false)
    }

    /// List all tables with RLS enabled
    pub fn list_enabled_tables(&self) -> Vec<String> {
        self.enabled_tables
            .iter()
            .filter(|r| *r.value())
            .map(|r| r.key().clone())
            .collect()
    }

    /// Generate SQL to create default tenant column with proper constraints
    pub fn generate_add_tenant_column_sql(
        table_name: &str,
        column_name: &str,
        not_null: bool,
        default_current: bool,
    ) -> String {
        let mut sql = format!(
            "ALTER TABLE {} ADD COLUMN IF NOT EXISTS {} TEXT",
            table_name, column_name
        );

        if not_null {
            sql.push_str(" NOT NULL");
        }

        if default_current {
            sql.push_str(" DEFAULT current_setting('ruvector.tenant_id')");
        }

        sql.push_str(";\n");

        // Add foreign key constraint to tenants table
        sql.push_str(&format!(
            r#"
-- Add foreign key to tenants table (optional, depends on schema)
-- ALTER TABLE {} ADD CONSTRAINT fk_{}_tenant
--     FOREIGN KEY ({}) REFERENCES ruvector.tenants(id) ON DELETE CASCADE;

-- Create index on tenant column for efficient filtering
CREATE INDEX IF NOT EXISTS idx_{}_{} ON {} ({});
"#,
            table_name,
            table_name.replace('.', "_"),
            column_name,
            table_name.replace('.', "_"),
            column_name,
            table_name,
            column_name
        ));

        sql
    }

    /// Generate SQL to create roles for RLS
    pub fn generate_roles_sql() -> String {
        r#"
-- Create RuVector roles for RLS
DO $$
BEGIN
    -- Admin role (bypasses RLS)
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'ruvector_admin') THEN
        CREATE ROLE ruvector_admin;
    END IF;

    -- Standard user role (subject to RLS)
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'ruvector_users') THEN
        CREATE ROLE ruvector_users;
    END IF;

    -- Read-only role
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'ruvector_readonly') THEN
        CREATE ROLE ruvector_readonly;
    END IF;
END
$$;

-- Grant basic permissions
GRANT USAGE ON SCHEMA public TO ruvector_users, ruvector_readonly;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO ruvector_readonly;
GRANT ALL ON ALL TABLES IN SCHEMA public TO ruvector_users;
"#
        .to_string()
    }

    /// Generate SQL for tenant context validation trigger
    pub fn generate_context_validation_trigger(table_name: &str, tenant_column: &str) -> String {
        format!(
            r#"
-- Create function to validate tenant context before insert/update
CREATE OR REPLACE FUNCTION ruvector_validate_tenant_context_{table_safe}()
RETURNS TRIGGER AS $$
DECLARE
    v_tenant_id TEXT;
BEGIN
    -- Get current tenant context
    v_tenant_id := current_setting('ruvector.tenant_id', true);

    -- Validate context is set
    IF v_tenant_id IS NULL OR v_tenant_id = '' THEN
        RAISE EXCEPTION 'No tenant context set. Use SET ruvector.tenant_id = ''your-tenant-id''';
    END IF;

    -- Validate tenant matches (prevent cross-tenant writes)
    IF NEW.{column} IS NOT NULL AND NEW.{column} != v_tenant_id AND v_tenant_id != '*' THEN
        RAISE EXCEPTION 'Cannot write to different tenant: context=%, row=%',
            v_tenant_id, NEW.{column};
    END IF;

    -- Auto-set tenant_id if not provided
    IF NEW.{column} IS NULL THEN
        NEW.{column} := v_tenant_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger
DROP TRIGGER IF EXISTS trg_ruvector_validate_tenant_{table_safe} ON {table};
CREATE TRIGGER trg_ruvector_validate_tenant_{table_safe}
    BEFORE INSERT OR UPDATE ON {table}
    FOR EACH ROW
    EXECUTE FUNCTION ruvector_validate_tenant_context_{table_safe}();
"#,
            table = table_name,
            table_safe = table_name.replace('.', "_").replace('"', ""),
            column = tenant_column
        )
    }

    /// Generate SQL to check tenant existence before operations
    pub fn generate_tenant_existence_check(table_name: &str, tenant_column: &str) -> String {
        format!(
            r#"
-- Create function to check tenant exists
CREATE OR REPLACE FUNCTION ruvector_check_tenant_exists_{table_safe}()
RETURNS TRIGGER AS $$
BEGIN
    -- Check tenant exists (skip for admin wildcard)
    IF NEW.{column} != '*' THEN
        IF NOT EXISTS (SELECT 1 FROM ruvector.tenants WHERE id = NEW.{column}) THEN
            RAISE EXCEPTION 'Tenant does not exist: %', NEW.{column};
        END IF;

        -- Check tenant is not suspended
        IF EXISTS (SELECT 1 FROM ruvector.tenants WHERE id = NEW.{column} AND suspended_at IS NOT NULL) THEN
            RAISE EXCEPTION 'Tenant is suspended: %', NEW.{column};
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger (runs after tenant context validation)
DROP TRIGGER IF EXISTS trg_ruvector_check_tenant_{table_safe} ON {table};
CREATE TRIGGER trg_ruvector_check_tenant_{table_safe}
    BEFORE INSERT OR UPDATE ON {table}
    FOR EACH ROW
    EXECUTE FUNCTION ruvector_check_tenant_exists_{table_safe}();
"#,
            table = table_name,
            table_safe = table_name.replace('.', "_").replace('"', ""),
            column = tenant_column
        )
    }
}

impl Default for RlsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global RLS manager instance
static RLS_MANAGER: once_cell::sync::Lazy<RlsManager> = once_cell::sync::Lazy::new(RlsManager::new);

/// Get the global RLS manager
pub fn get_rls_manager() -> &'static RlsManager {
    &RLS_MANAGER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_config_builder() {
        let config = RlsPolicyConfig::new("embeddings")
            .with_tenant_column("org_id")
            .with_policy_name("custom_policy")
            .without_admin_bypass();

        assert_eq!(config.table_name, "embeddings");
        assert_eq!(config.tenant_column, "org_id");
        assert_eq!(config.policy_name, "custom_policy");
        assert!(!config.admin_bypass);
    }

    #[test]
    fn test_standard_policy_template() {
        let template = PolicyTemplate::Standard;
        let using = template.using_clause("tenant_id");
        let check = template.check_clause("tenant_id");

        assert!(using.contains("tenant_id"));
        assert!(using.contains("current_setting"));
        assert!(check.contains("tenant_id"));
    }

    #[test]
    fn test_hierarchical_template() {
        let template = PolicyTemplate::Hierarchical {
            path_column: "org_path".to_string(),
        };
        let using = template.using_clause("tenant_id");

        assert!(using.contains("org_path"));
        assert!(using.contains("LIKE"));
    }

    #[test]
    fn test_time_based_template() {
        let template = PolicyTemplate::TimeBased {
            time_column: "created_at".to_string(),
            retention_days: 30,
        };
        let using = template.using_clause("tenant_id");

        assert!(using.contains("created_at"));
        assert!(using.contains("30 days"));
    }

    #[test]
    fn test_generate_enable_rls_sql() {
        let manager = RlsManager::new();
        let config = RlsPolicyConfig::new("embeddings");
        let sql = manager.generate_enable_rls_sql(&config);

        assert!(sql.contains("ENABLE ROW LEVEL SECURITY"));
        assert!(sql.contains("ruvector_tenant_isolation"));
        assert!(sql.contains("ruvector.tenant_id"));
        assert!(sql.contains("ruvector_admin")); // Admin bypass
        assert!(sql.contains("wildcard")); // Wildcard policy
    }

    #[test]
    fn test_generate_disable_rls_sql() {
        let manager = RlsManager::new();
        let sql = manager.generate_disable_rls_sql("embeddings");

        assert!(sql.contains("DISABLE ROW LEVEL SECURITY"));
        assert!(sql.contains("DROP POLICY"));
    }

    #[test]
    fn test_set_tenant_sql() {
        let sql = RlsManager::generate_set_tenant_sql("acme-corp", false);
        assert!(sql.contains("SET ruvector.tenant_id"));
        assert!(sql.contains("acme-corp"));

        let sql_local = RlsManager::generate_set_tenant_sql("acme-corp", true);
        assert!(sql_local.contains("SET LOCAL"));
    }

    #[test]
    fn test_add_tenant_column_sql() {
        let sql = RlsManager::generate_add_tenant_column_sql("embeddings", "tenant_id", true, true);

        assert!(sql.contains("ADD COLUMN"));
        assert!(sql.contains("NOT NULL"));
        assert!(sql.contains("DEFAULT current_setting"));
        assert!(sql.contains("CREATE INDEX"));
    }

    #[test]
    fn test_roles_sql() {
        let sql = RlsManager::generate_roles_sql();

        assert!(sql.contains("ruvector_admin"));
        assert!(sql.contains("ruvector_users"));
        assert!(sql.contains("ruvector_readonly"));
    }

    #[test]
    fn test_context_validation_trigger() {
        let sql = RlsManager::generate_context_validation_trigger("embeddings", "tenant_id");

        assert!(sql.contains("CREATE OR REPLACE FUNCTION"));
        assert!(sql.contains("TRIGGER"));
        assert!(sql.contains("BEFORE INSERT OR UPDATE"));
    }
}
