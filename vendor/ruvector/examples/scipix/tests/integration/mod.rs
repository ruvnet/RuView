// Integration test module organization
//
// This module provides integration tests for the ruvector-scipix OCR system.
// Tests are organized by functionality area.

pub mod accuracy_tests;
pub mod api_tests;
pub mod cache_tests;
pub mod cli_tests;
pub mod performance_tests;
pub mod pipeline_tests;

// Re-export common test utilities
pub use crate::common::*;
