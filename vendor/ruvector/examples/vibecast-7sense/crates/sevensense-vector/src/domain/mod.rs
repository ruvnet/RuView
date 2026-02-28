//! Domain layer for the Vector Space bounded context.
//!
//! Contains:
//! - Entities: Core domain objects with identity
//! - Value Objects: Immutable objects defined by their attributes
//! - Repository Traits: Abstractions for persistence
//! - Domain Errors: Error types specific to this context

pub mod entities;
pub mod repository;
pub mod error;

pub use entities::*;
pub use repository::*;
pub use error::*;
