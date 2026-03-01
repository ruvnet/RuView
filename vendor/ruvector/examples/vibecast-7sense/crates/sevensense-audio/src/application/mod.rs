//! Application layer for the audio ingestion bounded context.
//!
//! This module contains application services that orchestrate
//! domain operations and infrastructure components.

pub mod services;
pub mod error;

pub use services::*;
pub use error::*;
