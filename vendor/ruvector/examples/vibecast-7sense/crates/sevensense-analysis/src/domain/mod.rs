//! Domain layer for the Analysis bounded context.
//!
//! Contains core domain entities, value objects, repository traits, and domain events.

pub mod entities;
pub mod events;
pub mod repository;
pub mod value_objects;

// Re-export commonly used types
pub use entities::*;
pub use events::*;
pub use repository::*;
pub use value_objects::*;
