//! Application layer for the Vector Space bounded context.
//!
//! Contains:
//! - Services: Use case implementations
//! - DTOs: Data transfer objects
//! - Commands/Queries: CQRS patterns

pub mod services;

pub use services::*;
