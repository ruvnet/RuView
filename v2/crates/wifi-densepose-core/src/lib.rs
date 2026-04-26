//! # WiFi-DensePose Core
//!
//! Core types, traits, and utilities for the WiFi-DensePose pose estimation system.
//!
//! This crate provides the foundational building blocks used throughout the
//! WiFi-DensePose ecosystem, including:
//!
//! - **Core Data Types**: [`CsiFrame`], [`ProcessedSignal`], [`PoseEstimate`],
//!   [`PersonPose`], and [`Keypoint`] for representing `WiFi` CSI data and pose
//!   estimation results.
//!
//! - **Error Types**: Comprehensive error handling via the [`error`] module,
//!   with specific error types for different subsystems.
//!
//! - **Traits**: Core abstractions like [`SignalProcessor`], [`NeuralInference`],
//!   and [`DataStore`] that define the contracts for signal processing, neural
//!   network inference, and data persistence.
//!
//! - **Utilities**: Common helper functions and types used across the codebase.
//!
//! ## Feature Flags
//!
//! - `std` (default): Enable standard library support
//! - `serde`: Enable serialization/deserialization via serde
//! - `async`: Enable async trait definitions
//!
//! ## Example
//!
//! ```rust
//! use wifi_densepose_core::{CsiFrame, Keypoint, KeypointType, Confidence};
//!
//! // Create a keypoint with high confidence
//! let keypoint = Keypoint::new(
//!     KeypointType::Nose,
//!     0.5,
//!     0.3,
//!     Confidence::new(0.95).unwrap(),
//! );
//!
//! assert!(keypoint.is_visible());
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod error;
pub mod traits;
pub mod types;
pub mod utils;

// Re-export commonly used types at the crate root
pub use error::{CoreError, CoreResult, SignalError, InferenceError, StorageError};
pub use traits::{SignalProcessor, NeuralInference, DataStore};
pub use types::{
    // CSI types
    CsiFrame, CsiMetadata, AntennaConfig,
    // Signal types
    ProcessedSignal, SignalFeatures, FrequencyBand,
    // Pose types
    PoseEstimate, PersonPose, Keypoint, KeypointType,
    // Common types
    Confidence, Timestamp, FrameId, DeviceId,
    // Bounding box
    BoundingBox,
};

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Maximum number of keypoints per person (COCO format)
pub const MAX_KEYPOINTS: usize = 17;

/// Maximum number of subcarriers typically used in `WiFi` CSI
pub const MAX_SUBCARRIERS: usize = 256;

/// Default confidence threshold for keypoint visibility
pub const DEFAULT_CONFIDENCE_THRESHOLD: f32 = 0.5;

/// Prelude module for convenient imports.
///
/// Convenient re-exports of commonly used types and traits.
///
/// ```rust
/// use wifi_densepose_core::prelude::*;
/// ```
pub mod prelude {

    pub use crate::error::{CoreError, CoreResult};
    pub use crate::traits::{DataStore, NeuralInference, SignalProcessor};
    pub use crate::types::{
        AntennaConfig, BoundingBox, Confidence, CsiFrame, CsiMetadata, DeviceId, FrameId,
        FrequencyBand, Keypoint, KeypointType, PersonPose, PoseEstimate, ProcessedSignal,
        SignalFeatures, Timestamp,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_is_valid() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_KEYPOINTS, 17);
        assert!(MAX_SUBCARRIERS > 0);
        assert!(DEFAULT_CONFIDENCE_THRESHOLD > 0.0);
        assert!(DEFAULT_CONFIDENCE_THRESHOLD < 1.0);
    }
}
