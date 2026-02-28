//! # Ruvector Collections
//!
//! Multi-collection management with aliases for organizing vector databases.
//!
//! ## Features
//!
//! - **Multiple Collections**: Organize vectors into separate collections
//! - **Alias Management**: Create aliases for collection names
//! - **Collection Statistics**: Track collection metrics
//! - **Thread-safe**: Concurrent access using DashMap
//! - **Persistence**: Store collections on disk
//!
//! ## Example
//!
//! ```no_run
//! use ruvector_collections::{CollectionManager, CollectionConfig};
//! use ruvector_core::types::{DistanceMetric, HnswConfig};
//! use std::path::PathBuf;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a collection manager
//! let manager = CollectionManager::new(PathBuf::from("./collections"))?;
//!
//! // Create a collection
//! let config = CollectionConfig {
//!     dimensions: 384,
//!     distance_metric: DistanceMetric::Cosine,
//!     hnsw_config: Some(HnswConfig::default()),
//!     quantization: None,
//!     on_disk_payload: true,
//! };
//!
//! manager.create_collection("documents", config)?;
//!
//! // Create an alias
//! manager.create_alias("current_docs", "documents")?;
//!
//! // Get collection by name or alias
//! let collection = manager.get_collection("current_docs").unwrap();
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod collection;
pub mod error;
pub mod manager;

pub use collection::{Collection, CollectionConfig, CollectionStats};
pub use error::{CollectionError, Result};
pub use manager::CollectionManager;
