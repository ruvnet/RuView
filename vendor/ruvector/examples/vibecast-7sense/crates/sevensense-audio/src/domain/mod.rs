//! Domain layer for the audio ingestion bounded context.
//!
//! This module contains the core domain model:
//! - Entities: Recording, CallSegment
//! - Value objects: SignalQuality
//! - Repository traits: RecordingRepository

pub mod entities;
pub mod repository;

pub use entities::*;
pub use repository::*;
