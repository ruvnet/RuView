//! Input Validation for Multi-Tenancy Security
//!
//! Provides strict validation for tenant IDs, table names, and other identifiers
//! to prevent SQL injection attacks.

use std::fmt;

/// Maximum length for tenant IDs
pub const MAX_TENANT_ID_LENGTH: usize = 64;

/// Maximum length for identifiers (tables, schemas, partitions)
pub const MAX_IDENTIFIER_LENGTH: usize = 63; // PostgreSQL limit

/// Validation error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Identifier is empty
    Empty,
    /// Identifier is too long
    TooLong { max: usize, actual: usize },
    /// Identifier contains invalid characters
    InvalidCharacters { position: usize, char: char },
    /// Identifier doesn't start with a valid character
    InvalidStart { char: char },
    /// Identifier is a reserved word
    ReservedWord(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "Identifier cannot be empty"),
            Self::TooLong { max, actual } => {
                write!(f, "Identifier too long: {} chars (max {})", actual, max)
            }
            Self::InvalidCharacters { position, char } => {
                write!(f, "Invalid character '{}' at position {}", char, position)
            }
            Self::InvalidStart { char } => {
                write!(f, "Identifier cannot start with '{}'", char)
            }
            Self::ReservedWord(word) => {
                write!(f, "Cannot use reserved word: {}", word)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Reserved PostgreSQL words that cannot be used as identifiers
const RESERVED_WORDS: &[&str] = &[
    "select",
    "insert",
    "update",
    "delete",
    "drop",
    "create",
    "alter",
    "grant",
    "revoke",
    "table",
    "schema",
    "index",
    "cascade",
    "restrict",
    "null",
    "true",
    "false",
    "and",
    "or",
    "not",
    "in",
    "exists",
    "between",
    "like",
    "is",
    "as",
    "from",
    "where",
    "order",
    "by",
    "group",
    "having",
    "limit",
    "offset",
    "join",
    "inner",
    "outer",
    "left",
    "right",
    "cross",
    "on",
    "using",
    "union",
    "except",
    "intersect",
    "all",
    "distinct",
    "case",
    "when",
    "then",
    "else",
    "end",
    "cast",
    "coalesce",
    "nullif",
    "primary",
    "key",
    "foreign",
    "references",
    "unique",
    "check",
    "default",
    "constraint",
    "trigger",
    "function",
    "procedure",
    "view",
    "sequence",
    "type",
    "domain",
    "role",
    "user",
    "database",
    "tablespace",
    "extension",
    "operator",
    "policy",
    "rule",
    "security",
    "definer",
    "invoker",
];

/// Validate a tenant ID
///
/// Tenant IDs must:
/// - Be 1-64 characters long
/// - Start with a letter or underscore
/// - Contain only letters, numbers, underscores, and hyphens
/// - Not be a reserved SQL keyword
///
/// # Examples
///
/// ```
/// use ruvector_postgres::tenancy::validation::validate_tenant_id;
///
/// assert!(validate_tenant_id("acme-corp").is_ok());
/// assert!(validate_tenant_id("tenant_123").is_ok());
/// assert!(validate_tenant_id("DROP TABLE users;--").is_err());
/// ```
pub fn validate_tenant_id(tenant_id: &str) -> Result<(), ValidationError> {
    // Check empty
    if tenant_id.is_empty() {
        return Err(ValidationError::Empty);
    }

    // Check length
    if tenant_id.len() > MAX_TENANT_ID_LENGTH {
        return Err(ValidationError::TooLong {
            max: MAX_TENANT_ID_LENGTH,
            actual: tenant_id.len(),
        });
    }

    // Check first character (must be letter or underscore)
    let first_char = tenant_id.chars().next().unwrap();
    if !first_char.is_ascii_alphabetic() && first_char != '_' {
        return Err(ValidationError::InvalidStart { char: first_char });
    }

    // Check all characters
    for (i, c) in tenant_id.chars().enumerate() {
        if !is_valid_identifier_char(c) && c != '-' {
            return Err(ValidationError::InvalidCharacters {
                position: i,
                char: c,
            });
        }
    }

    // Check reserved words (lowercase comparison)
    let lower = tenant_id.to_lowercase();
    if RESERVED_WORDS.contains(&lower.as_str()) {
        return Err(ValidationError::ReservedWord(tenant_id.to_string()));
    }

    Ok(())
}

/// Validate a SQL identifier (table name, schema name, column name)
///
/// Identifiers must:
/// - Be 1-63 characters long (PostgreSQL limit)
/// - Start with a letter or underscore
/// - Contain only letters, numbers, and underscores
/// - Not be a reserved SQL keyword
pub fn validate_identifier(identifier: &str) -> Result<(), ValidationError> {
    // Check empty
    if identifier.is_empty() {
        return Err(ValidationError::Empty);
    }

    // Check length
    if identifier.len() > MAX_IDENTIFIER_LENGTH {
        return Err(ValidationError::TooLong {
            max: MAX_IDENTIFIER_LENGTH,
            actual: identifier.len(),
        });
    }

    // Check first character (must be letter or underscore)
    let first_char = identifier.chars().next().unwrap();
    if !first_char.is_ascii_alphabetic() && first_char != '_' {
        return Err(ValidationError::InvalidStart { char: first_char });
    }

    // Check all characters (stricter than tenant_id - no hyphens)
    for (i, c) in identifier.chars().enumerate() {
        if !is_valid_identifier_char(c) {
            return Err(ValidationError::InvalidCharacters {
                position: i,
                char: c,
            });
        }
    }

    // Check reserved words (lowercase comparison)
    let lower = identifier.to_lowercase();
    if RESERVED_WORDS.contains(&lower.as_str()) {
        return Err(ValidationError::ReservedWord(identifier.to_string()));
    }

    Ok(())
}

/// Check if a character is valid for SQL identifiers
#[inline]
fn is_valid_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Sanitize a tenant ID for use in partition/schema names
///
/// Converts hyphens and dots to underscores, validates the result.
pub fn sanitize_for_identifier(input: &str) -> Result<String, ValidationError> {
    // First validate the input as a tenant ID
    validate_tenant_id(input)?;

    // Convert to valid identifier format
    let sanitized = input.replace('-', "_").replace('.', "_");

    // Validate the result as an identifier
    validate_identifier(&sanitized)?;

    Ok(sanitized)
}

/// Escape a string for use in SQL string literals
///
/// This function properly escapes single quotes by doubling them.
/// Use this only for string values, NOT for identifiers!
pub fn escape_string_literal(input: &str) -> String {
    input.replace('\'', "''")
}

/// Quote an identifier for safe use in SQL
///
/// This function wraps the identifier in double quotes and escapes
/// any double quotes within it. This is the PostgreSQL-safe way to
/// use dynamic identifiers.
///
/// # Examples
///
/// ```
/// use ruvector_postgres::tenancy::validation::quote_identifier;
///
/// assert_eq!(quote_identifier("my_table"), "\"my_table\"");
/// assert_eq!(quote_identifier("weird\"name"), "\"weird\"\"name\"");
/// ```
pub fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// Validate and quote a partition name
///
/// Returns a safely quoted partition name or an error.
pub fn safe_partition_name(tenant_id: &str, parent_table: &str) -> Result<String, ValidationError> {
    // Validate both inputs
    validate_tenant_id(tenant_id)?;
    validate_identifier(parent_table)?;

    // Create sanitized partition name
    let sanitized_tenant = sanitize_for_identifier(tenant_id)?;
    let partition_name = format!("{}_{}", parent_table, sanitized_tenant);

    // Validate the combined name
    validate_identifier(&partition_name)?;

    Ok(partition_name)
}

/// Validate and quote a schema name
pub fn safe_schema_name(tenant_id: &str) -> Result<String, ValidationError> {
    validate_tenant_id(tenant_id)?;
    let sanitized = sanitize_for_identifier(tenant_id)?;
    let schema_name = format!("tenant_{}", sanitized);
    validate_identifier(&schema_name)?;
    Ok(schema_name)
}

/// Validate an IP address format (basic check)
pub fn validate_ip_address(ip: &str) -> bool {
    // Allow IPv4 and IPv6
    ip.parse::<std::net::IpAddr>().is_ok()
}

/// Sanitize an IP address or return None if invalid
pub fn sanitize_ip_address(ip: Option<&str>) -> Option<String> {
    ip.and_then(|i| {
        if validate_ip_address(i) {
            Some(i.to_string())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_tenant_ids() {
        assert!(validate_tenant_id("acme-corp").is_ok());
        assert!(validate_tenant_id("tenant_123").is_ok());
        assert!(validate_tenant_id("my-tenant-id").is_ok());
        assert!(validate_tenant_id("_private").is_ok());
        assert!(validate_tenant_id("a").is_ok());
    }

    #[test]
    fn test_invalid_tenant_ids() {
        // Empty
        assert!(matches!(
            validate_tenant_id(""),
            Err(ValidationError::Empty)
        ));

        // Too long
        let long = "a".repeat(100);
        assert!(matches!(
            validate_tenant_id(&long),
            Err(ValidationError::TooLong { .. })
        ));

        // Invalid start
        assert!(matches!(
            validate_tenant_id("123tenant"),
            Err(ValidationError::InvalidStart { .. })
        ));
        assert!(matches!(
            validate_tenant_id("-tenant"),
            Err(ValidationError::InvalidStart { .. })
        ));

        // Invalid characters
        assert!(matches!(
            validate_tenant_id("tenant'id"),
            Err(ValidationError::InvalidCharacters { .. })
        ));
        assert!(matches!(
            validate_tenant_id("tenant;drop"),
            Err(ValidationError::InvalidCharacters { .. })
        ));
        assert!(matches!(
            validate_tenant_id("tenant id"),
            Err(ValidationError::InvalidCharacters { .. })
        ));

        // Reserved words
        assert!(matches!(
            validate_tenant_id("select"),
            Err(ValidationError::ReservedWord(_))
        ));
        assert!(matches!(
            validate_tenant_id("DROP"),
            Err(ValidationError::ReservedWord(_))
        ));
    }

    #[test]
    fn test_sql_injection_attempts() {
        // Common SQL injection patterns
        assert!(validate_tenant_id("'; DROP TABLE users;--").is_err());
        assert!(validate_tenant_id("tenant' OR '1'='1").is_err());
        assert!(validate_tenant_id("tenant\"; DELETE FROM").is_err());
        assert!(validate_tenant_id("tenant$(whoami)").is_err());
        assert!(validate_tenant_id("tenant`id`").is_err());
    }

    #[test]
    fn test_valid_identifiers() {
        assert!(validate_identifier("my_table").is_ok());
        assert!(validate_identifier("embeddings").is_ok());
        assert!(validate_identifier("_private_table").is_ok());
        assert!(validate_identifier("table123").is_ok());
    }

    #[test]
    fn test_invalid_identifiers() {
        // Hyphens not allowed in identifiers
        assert!(validate_identifier("my-table").is_err());

        // Special characters
        assert!(validate_identifier("my.table").is_err());
        assert!(validate_identifier("my table").is_err());
    }

    #[test]
    fn test_sanitize_for_identifier() {
        assert_eq!(sanitize_for_identifier("acme-corp").unwrap(), "acme_corp");
        assert_eq!(
            sanitize_for_identifier("my.tenant.id").unwrap(),
            "my_tenant_id"
        );
        assert_eq!(sanitize_for_identifier("simple").unwrap(), "simple");
    }

    #[test]
    fn test_quote_identifier() {
        assert_eq!(quote_identifier("my_table"), "\"my_table\"");
        assert_eq!(quote_identifier("weird\"name"), "\"weird\"\"name\"");
        assert_eq!(quote_identifier("UPPERCASE"), "\"UPPERCASE\"");
    }

    #[test]
    fn test_escape_string_literal() {
        assert_eq!(escape_string_literal("hello"), "hello");
        assert_eq!(escape_string_literal("it's"), "it''s");
        assert_eq!(escape_string_literal("O'Brien's"), "O''Brien''s");
    }

    #[test]
    fn test_safe_partition_name() {
        assert_eq!(
            safe_partition_name("acme-corp", "embeddings").unwrap(),
            "embeddings_acme_corp"
        );
        assert!(safe_partition_name("'; DROP TABLE", "embeddings").is_err());
    }

    #[test]
    fn test_safe_schema_name() {
        assert_eq!(safe_schema_name("acme-corp").unwrap(), "tenant_acme_corp");
        assert!(safe_schema_name("'; DROP SCHEMA").is_err());
    }

    #[test]
    fn test_validate_ip_address() {
        assert!(validate_ip_address("192.168.1.1"));
        assert!(validate_ip_address("10.0.0.1"));
        assert!(validate_ip_address("::1"));
        assert!(validate_ip_address("2001:db8::1"));

        assert!(!validate_ip_address("not-an-ip"));
        assert!(!validate_ip_address("192.168.1.256"));
        assert!(!validate_ip_address("'; DROP TABLE"));
    }
}
