//! Survivor track lifecycle management for the MAT crate.
//!
//! Implements three collaborating components:
//!
//! - **[`KalmanState`]** — constant-velocity 3-D position filter
//! - **[`CsiFingerprint`]** — biometric re-identification across signal gaps
//! - **[`TrackLifecycle`]** — state machine (Tentative→Active→Lost→Terminated)
//! - **[`SurvivorTracker`]** — aggregate root orchestrating all three
//!
//! # Example
//!
//! ```rust,no_run
//! use wifi_densepose_mat::tracking::{SurvivorTracker, TrackerConfig, DetectionObservation};
//!
//! let mut tracker = SurvivorTracker::with_defaults();
//! let observations = vec![]; // DetectionObservation instances from sensing pipeline
//! let result = tracker.update(observations, 0.5); // dt = 0.5s (2 Hz)
//! println!("Active survivors: {}", tracker.active_count());
//! ```

pub mod kalman;
pub mod fingerprint;
pub mod lifecycle;
pub mod tracker;

pub use kalman::KalmanState;
pub use fingerprint::CsiFingerprint;
pub use lifecycle::{TrackState, TrackLifecycle, TrackerConfig};
pub use tracker::{
    TrackId, TrackedSurvivor, SurvivorTracker,
    DetectionObservation, AssociationResult,
};
