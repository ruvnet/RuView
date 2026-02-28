//! Core types and traits for 7sense bioacoustic analysis.
//!
//! This crate provides foundational types shared across all bounded contexts:
//! - Entity identifiers (strongly-typed IDs)
//! - Value objects (GeoLocation, Timestamp, AudioMetadata)
//! - Common error types
//! - Domain entities and events

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod identifiers;
pub mod values;
pub mod error;
pub mod domain;

// Re-export commonly used types
pub use identifiers::*;
pub use values::*;
pub use error::*;
