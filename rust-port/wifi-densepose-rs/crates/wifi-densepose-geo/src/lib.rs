//! wifi-densepose-geo — geospatial satellite integration for RuView.
//!
//! Provides: IP geolocation, satellite tile fetching (Sentinel-2),
//! SRTM elevation, OSM buildings/roads, coordinate transforms,
//! temporal change tracking, and brain memory integration.

pub mod types;
pub mod coord;
pub mod locate;
pub mod cache;
pub mod tiles;
pub mod terrain;
pub mod osm;
pub mod register;
pub mod fuse;
pub mod brain;
pub mod temporal;

pub use types::*;
