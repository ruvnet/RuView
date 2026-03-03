//! AIMDS Core - Shared types, utilities, and error handling
//!
//! This crate provides the foundational types and utilities used across
//! all AIMDS components.

pub mod config;
pub mod error;
pub mod types;

pub use config::AimdsConfig;
pub use error::{AimdsError, Result};
pub use types::*;

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
