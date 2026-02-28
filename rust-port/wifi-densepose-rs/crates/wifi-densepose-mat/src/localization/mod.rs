//! Localization module for survivor position estimation.
//!
//! This module provides:
//! - Triangulation from multiple access points
//! - Depth estimation through debris
//! - Position fusion combining multiple techniques

mod triangulation;
mod depth;
mod fusion;

pub use triangulation::{Triangulator, TriangulationConfig, solve_tdoa_triangulation};
pub use depth::{DepthEstimator, DepthEstimatorConfig};
pub use fusion::{PositionFuser, LocalizationService};
