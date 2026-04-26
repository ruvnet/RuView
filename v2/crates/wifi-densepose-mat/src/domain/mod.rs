//! Domain module containing core entities, value objects, and domain events.
//!
//! This module follows Domain-Driven Design principles with:
//! - **Entities**: Objects with identity (Survivor, DisasterEvent, ScanZone)
//! - **Value Objects**: Immutable objects without identity (VitalSignsReading, Coordinates3D)
//! - **Domain Events**: Events that capture domain significance
//! - **Aggregates**: Consistency boundaries (DisasterEvent is the root)

pub mod alert;
pub mod coordinates;
pub mod disaster_event;
pub mod events;
pub mod scan_zone;
pub mod survivor;
pub mod triage;
pub mod vital_signs;

// Re-export all domain types
pub use alert::*;
pub use coordinates::*;
pub use disaster_event::*;
pub use events::*;
pub use scan_zone::*;
pub use survivor::*;
pub use triage::*;
pub use vital_signs::*;
