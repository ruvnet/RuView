//! Input Validation Utilities
//!
//! Provides comprehensive validation for all external inputs to prevent
//! security issues like path traversal, resource exhaustion, and invalid data.

use super::limits::{SecurityConfig, DEFAULT_MAX_NODE_ID_LEN, DEFAULT_MAX_STATE_DIM};
use std::path::{Component, Path};
use thiserror::Error;

/// Validation error types
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ValidationError {
    /// Node ID is too long
    #[error("Node ID too long: {len} bytes (max: {max})")]
    NodeIdTooLong { len: usize, max: usize },

    /// Node ID contains invalid characters
    #[error("Node ID contains invalid characters: {0}")]
    InvalidNodeIdChars(String),

    /// Node ID is empty
    #[error("Node ID cannot be empty")]
    EmptyNodeId,

    /// State vector is too large
    #[error("State dimension too large: {dim} (max: {max})")]
    StateDimensionTooLarge { dim: usize, max: usize },

    /// State vector is empty
    #[error("State vector cannot be empty")]
    EmptyState,

    /// State contains invalid float value (NaN or Infinity)
    #[error("State contains invalid float at index {index}: {value}")]
    InvalidFloat { index: usize, value: String },

    /// Matrix dimension too large
    #[error("Matrix dimension too large: {dim} (max: {max})")]
    MatrixDimensionTooLarge { dim: usize, max: usize },

    /// Dimension mismatch
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Path traversal attempt detected
    #[error("Path traversal detected in: {0}")]
    PathTraversal(String),

    /// Path contains invalid characters
    #[error("Path contains invalid characters: {0}")]
    InvalidPathChars(String),

    /// Payload too large
    #[error("Payload too large: {size} bytes (max: {max})")]
    PayloadTooLarge { size: usize, max: usize },

    /// Resource limit exceeded
    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    /// Custom validation error
    #[error("{0}")]
    Custom(String),
}

/// Result type for validation operations
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Input validator with configurable limits
#[derive(Debug, Clone)]
pub struct InputValidator {
    config: SecurityConfig,
}

impl Default for InputValidator {
    fn default() -> Self {
        Self::new(SecurityConfig::default())
    }
}

impl InputValidator {
    /// Create a new validator with the given configuration
    #[must_use]
    pub fn new(config: SecurityConfig) -> Self {
        Self { config }
    }

    /// Create a validator with strict settings
    #[must_use]
    pub fn strict() -> Self {
        Self::new(SecurityConfig::strict())
    }

    /// Validate a node ID
    ///
    /// Checks:
    /// - Non-empty
    /// - Length within limits
    /// - Contains only allowed characters (alphanumeric, dash, underscore, dot)
    pub fn validate_node_id(&self, id: &str) -> ValidationResult<()> {
        if id.is_empty() {
            return Err(ValidationError::EmptyNodeId);
        }

        if id.len() > self.config.max_node_id_len {
            return Err(ValidationError::NodeIdTooLong {
                len: id.len(),
                max: self.config.max_node_id_len,
            });
        }

        if !is_valid_identifier(id) {
            return Err(ValidationError::InvalidNodeIdChars(id.to_string()));
        }

        Ok(())
    }

    /// Validate a state vector
    ///
    /// Checks:
    /// - Non-empty
    /// - Dimension within limits
    /// - No NaN or Infinity values
    pub fn validate_state(&self, state: &[f32]) -> ValidationResult<()> {
        if state.is_empty() {
            return Err(ValidationError::EmptyState);
        }

        if state.len() > self.config.graph_limits.max_state_dim {
            return Err(ValidationError::StateDimensionTooLarge {
                dim: state.len(),
                max: self.config.graph_limits.max_state_dim,
            });
        }

        // Check for NaN/Infinity
        for (i, &val) in state.iter().enumerate() {
            if val.is_nan() {
                return Err(ValidationError::InvalidFloat {
                    index: i,
                    value: "NaN".to_string(),
                });
            }
            if val.is_infinite() {
                return Err(ValidationError::InvalidFloat {
                    index: i,
                    value: if val.is_sign_positive() {
                        "+Infinity"
                    } else {
                        "-Infinity"
                    }
                    .to_string(),
                });
            }
        }

        Ok(())
    }

    /// Validate matrix dimensions
    pub fn validate_matrix_dims(&self, rows: usize, cols: usize) -> ValidationResult<()> {
        let max = self.config.resource_limits.max_matrix_dim;

        if rows > max {
            return Err(ValidationError::MatrixDimensionTooLarge { dim: rows, max });
        }
        if cols > max {
            return Err(ValidationError::MatrixDimensionTooLarge { dim: cols, max });
        }

        // Also check total elements to prevent memory exhaustion
        let total = rows.saturating_mul(cols);
        let max_elements = self.config.resource_limits.max_matrix_elements();
        if total > max_elements {
            return Err(ValidationError::ResourceLimitExceeded(format!(
                "Matrix elements: {} (max: {})",
                total, max_elements
            )));
        }

        Ok(())
    }

    /// Validate payload size
    pub fn validate_payload_size(&self, size: usize) -> ValidationResult<()> {
        if size > self.config.resource_limits.max_payload_size {
            return Err(ValidationError::PayloadTooLarge {
                size,
                max: self.config.resource_limits.max_payload_size,
            });
        }
        Ok(())
    }

    /// Check if graph can accept more nodes
    pub fn check_node_limit(&self, current_count: usize) -> ValidationResult<()> {
        if !self.config.graph_limits.can_add_node(current_count) {
            return Err(ValidationError::ResourceLimitExceeded(format!(
                "Maximum nodes: {}",
                self.config.graph_limits.max_nodes
            )));
        }
        Ok(())
    }

    /// Check if graph can accept more edges
    pub fn check_edge_limit(&self, current_count: usize) -> ValidationResult<()> {
        if !self.config.graph_limits.can_add_edge(current_count) {
            return Err(ValidationError::ResourceLimitExceeded(format!(
                "Maximum edges: {}",
                self.config.graph_limits.max_edges
            )));
        }
        Ok(())
    }
}

/// Path validator for file storage operations
#[derive(Debug, Clone, Default)]
pub struct PathValidator;

impl PathValidator {
    /// Validate a path component to prevent traversal attacks
    ///
    /// Rejects:
    /// - Empty components
    /// - "." or ".." components
    /// - Absolute paths or drive letters
    /// - Components with path separators
    /// - Components starting with "~"
    pub fn validate_path_component(component: &str) -> ValidationResult<()> {
        if component.is_empty() {
            return Err(ValidationError::InvalidPathChars(
                "empty component".to_string(),
            ));
        }

        // Check for traversal attempts
        if component == "." || component == ".." {
            return Err(ValidationError::PathTraversal(component.to_string()));
        }

        // Check for absolute paths
        if component.starts_with('/') || component.starts_with('\\') {
            return Err(ValidationError::PathTraversal(component.to_string()));
        }

        // Check for Windows drive letters (C:, D:, etc.)
        if component.len() >= 2 && component.chars().nth(1) == Some(':') {
            return Err(ValidationError::PathTraversal(component.to_string()));
        }

        // Check for home directory reference
        if component.starts_with('~') {
            return Err(ValidationError::PathTraversal(component.to_string()));
        }

        // Check for path separators within the component
        if component.contains('/') || component.contains('\\') {
            return Err(ValidationError::PathTraversal(component.to_string()));
        }

        // Check for null bytes
        if component.contains('\0') {
            return Err(ValidationError::InvalidPathChars("null byte".to_string()));
        }

        Ok(())
    }

    /// Validate a complete path stays within a base directory
    pub fn validate_path_within_base(base: &Path, path: &Path) -> ValidationResult<()> {
        // Normalize both paths
        let base_canonical = match base.canonicalize() {
            Ok(p) => p,
            Err(_) => base.to_path_buf(),
        };

        // Build the full path
        let full_path = base.join(path);

        // Check each component
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    return Err(ValidationError::PathTraversal(path.display().to_string()));
                }
                Component::Normal(s) => {
                    if let Some(s_str) = s.to_str() {
                        Self::validate_path_component(s_str)?;
                    }
                }
                Component::Prefix(_) | Component::RootDir => {
                    return Err(ValidationError::PathTraversal(path.display().to_string()));
                }
                Component::CurDir => {}
            }
        }

        // Final check: resolved path should start with base
        if let Ok(resolved) = full_path.canonicalize() {
            if !resolved.starts_with(&base_canonical) {
                return Err(ValidationError::PathTraversal(path.display().to_string()));
            }
        }

        Ok(())
    }
}

/// State vector validator
#[derive(Debug, Clone)]
pub struct StateValidator {
    max_dim: usize,
}

impl Default for StateValidator {
    fn default() -> Self {
        Self {
            max_dim: DEFAULT_MAX_STATE_DIM,
        }
    }
}

impl StateValidator {
    /// Create a validator with custom max dimension
    #[must_use]
    pub fn new(max_dim: usize) -> Self {
        Self { max_dim }
    }

    /// Validate state vector and return validated copy
    pub fn validate(&self, state: &[f32]) -> ValidationResult<Vec<f32>> {
        if state.is_empty() {
            return Err(ValidationError::EmptyState);
        }

        if state.len() > self.max_dim {
            return Err(ValidationError::StateDimensionTooLarge {
                dim: state.len(),
                max: self.max_dim,
            });
        }

        // Check for and handle invalid floats
        let mut validated = Vec::with_capacity(state.len());
        for (i, &val) in state.iter().enumerate() {
            if val.is_nan() {
                return Err(ValidationError::InvalidFloat {
                    index: i,
                    value: "NaN".to_string(),
                });
            }
            if val.is_infinite() {
                return Err(ValidationError::InvalidFloat {
                    index: i,
                    value: format!("{}", val),
                });
            }
            validated.push(val);
        }

        Ok(validated)
    }

    /// Validate and clamp state values to a range
    pub fn validate_and_clamp(
        &self,
        state: &[f32],
        min: f32,
        max: f32,
    ) -> ValidationResult<Vec<f32>> {
        if state.is_empty() {
            return Err(ValidationError::EmptyState);
        }

        if state.len() > self.max_dim {
            return Err(ValidationError::StateDimensionTooLarge {
                dim: state.len(),
                max: self.max_dim,
            });
        }

        let mut result = Vec::with_capacity(state.len());
        for (i, &val) in state.iter().enumerate() {
            if val.is_nan() {
                return Err(ValidationError::InvalidFloat {
                    index: i,
                    value: "NaN".to_string(),
                });
            }
            // Clamp infinite values to min/max
            let clamped = if val.is_infinite() {
                if val.is_sign_positive() {
                    max
                } else {
                    min
                }
            } else {
                val.clamp(min, max)
            };
            result.push(clamped);
        }

        Ok(result)
    }
}

// ============================================================================
// Standalone validation functions
// ============================================================================

/// Check if a string is a valid identifier (alphanumeric, dash, underscore, dot)
#[must_use]
pub fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    // First character must be alphanumeric
    let first_char = s.chars().next().unwrap();
    if !first_char.is_ascii_alphanumeric() {
        return false;
    }

    // Rest can be alphanumeric, dash, underscore, or dot
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Check if a state vector is valid (no NaN/Infinity)
#[must_use]
pub fn is_valid_state(state: &[f32]) -> bool {
    !state.is_empty() && state.iter().all(|&x| x.is_finite())
}

/// Sanitize a path component by removing unsafe characters
///
/// Returns None if the component cannot be sanitized safely
pub fn sanitize_path_component(component: &str) -> Option<String> {
    if component.is_empty() || component == "." || component == ".." {
        return None;
    }

    // Filter to only safe characters
    let sanitized: String = component
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();

    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        return None;
    }

    Some(sanitized)
}

/// Validate a dimension value
pub fn validate_dimension(dim: usize, max: usize) -> ValidationResult<()> {
    if dim == 0 {
        return Err(ValidationError::Custom(
            "Dimension cannot be zero".to_string(),
        ));
    }
    if dim > max {
        return Err(ValidationError::MatrixDimensionTooLarge { dim, max });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_identifier() {
        assert!(is_valid_identifier("node1"));
        assert!(is_valid_identifier("my-node"));
        assert!(is_valid_identifier("my_node"));
        assert!(is_valid_identifier("node.v1"));
        assert!(is_valid_identifier("Node123"));

        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("-node"));
        assert!(!is_valid_identifier("_node"));
        assert!(!is_valid_identifier(".node"));
        assert!(!is_valid_identifier("node/path"));
        assert!(!is_valid_identifier("node\\path"));
        assert!(!is_valid_identifier("node with space"));
    }

    #[test]
    fn test_valid_state() {
        assert!(is_valid_state(&[1.0, 2.0, 3.0]));
        assert!(is_valid_state(&[0.0]));
        assert!(is_valid_state(&[-1.0, 0.0, 1.0]));

        assert!(!is_valid_state(&[]));
        assert!(!is_valid_state(&[f32::NAN]));
        assert!(!is_valid_state(&[f32::INFINITY]));
        assert!(!is_valid_state(&[f32::NEG_INFINITY]));
        assert!(!is_valid_state(&[1.0, f32::NAN, 3.0]));
    }

    #[test]
    fn test_input_validator_node_id() {
        let validator = InputValidator::default();

        assert!(validator.validate_node_id("valid-node").is_ok());
        assert!(validator.validate_node_id("node123").is_ok());

        assert!(validator.validate_node_id("").is_err());
        assert!(validator.validate_node_id("../traversal").is_err());
        assert!(validator.validate_node_id("with space").is_err());
    }

    #[test]
    fn test_input_validator_state() {
        let validator = InputValidator::default();

        assert!(validator.validate_state(&[1.0, 2.0, 3.0]).is_ok());

        assert!(validator.validate_state(&[]).is_err());
        assert!(validator.validate_state(&[f32::NAN]).is_err());
        assert!(validator.validate_state(&[f32::INFINITY]).is_err());
    }

    #[test]
    fn test_path_validator() {
        assert!(PathValidator::validate_path_component("valid_name").is_ok());
        assert!(PathValidator::validate_path_component("file.txt").is_ok());

        assert!(PathValidator::validate_path_component("").is_err());
        assert!(PathValidator::validate_path_component(".").is_err());
        assert!(PathValidator::validate_path_component("..").is_err());
        assert!(PathValidator::validate_path_component("../etc").is_err());
        assert!(PathValidator::validate_path_component("/etc").is_err());
        assert!(PathValidator::validate_path_component("C:\\").is_err());
        assert!(PathValidator::validate_path_component("~user").is_err());
    }

    #[test]
    fn test_sanitize_path() {
        assert_eq!(
            sanitize_path_component("valid_name"),
            Some("valid_name".to_string())
        );
        assert_eq!(
            sanitize_path_component("file.txt"),
            Some("file.txt".to_string())
        );
        assert_eq!(
            sanitize_path_component("bad/path"),
            Some("badpath".to_string())
        );
        assert_eq!(
            sanitize_path_component("bad\\path"),
            Some("badpath".to_string())
        );

        assert_eq!(sanitize_path_component(""), None);
        assert_eq!(sanitize_path_component("."), None);
        assert_eq!(sanitize_path_component(".."), None);
        assert_eq!(sanitize_path_component("///"), None);
    }

    #[test]
    fn test_state_validator() {
        let validator = StateValidator::new(100);

        assert!(validator.validate(&[1.0, 2.0]).is_ok());
        assert!(validator.validate(&[]).is_err());
        assert!(validator.validate(&[f32::NAN]).is_err());

        let large: Vec<f32> = (0..101).map(|x| x as f32).collect();
        assert!(validator.validate(&large).is_err());
    }

    #[test]
    fn test_state_validator_clamp() {
        let validator = StateValidator::new(100);

        let result = validator.validate_and_clamp(&[f32::INFINITY, -1.0, 0.5], -1.0, 1.0);
        assert!(result.is_ok());
        let clamped = result.unwrap();
        assert_eq!(clamped, vec![1.0, -1.0, 0.5]);
    }

    #[test]
    fn test_matrix_validation() {
        let validator = InputValidator::default();

        assert!(validator.validate_matrix_dims(100, 100).is_ok());
        assert!(validator.validate_matrix_dims(8192, 8192).is_ok());
        assert!(validator.validate_matrix_dims(10000, 10000).is_err());
    }

    #[test]
    fn test_dimension_validation() {
        assert!(validate_dimension(100, 1000).is_ok());
        assert!(validate_dimension(0, 1000).is_err());
        assert!(validate_dimension(1001, 1000).is_err());
    }
}
