//! Utility functions for numerical operations

mod numerical;
mod sorting;

pub use numerical::*;
pub use sorting::*;

/// Small epsilon for numerical stability
pub const EPS: f64 = 1e-10;

/// Small epsilon for f32
pub const EPS_F32: f32 = 1e-7;

/// Log of minimum positive f64
pub const LOG_MIN: f64 = -700.0;

/// Log of maximum positive f64
pub const LOG_MAX: f64 = 700.0;
