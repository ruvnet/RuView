//! Security Module for Prime-Radiant Coherence Engine
//!
//! Provides input validation, resource limits, and security utilities.
//!
//! # Security Features
//!
//! - **Input Validation**: Validates node IDs, state vectors, dimensions
//! - **Resource Limits**: Configurable caps on graph size, matrix dimensions
//! - **Path Sanitization**: Prevents path traversal attacks
//! - **Float Validation**: Detects NaN/Infinity in numeric inputs
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::security::{SecurityConfig, InputValidator};
//!
//! let config = SecurityConfig::default();
//! let validator = InputValidator::new(config);
//!
//! // Validate a node ID
//! validator.validate_node_id("my-node-123")?;
//!
//! // Validate a state vector
//! validator.validate_state(&[1.0, 2.0, 3.0])?;
//! ```

mod limits;
mod validation;

pub use limits::{GraphLimits, ResourceLimits, SecurityConfig};
pub use validation::{
    InputValidator, PathValidator, StateValidator, ValidationError, ValidationResult,
};

/// Re-export common validation functions
pub mod prelude {
    pub use super::validation::{
        is_valid_identifier, is_valid_state, sanitize_path_component, validate_dimension,
    };
}
