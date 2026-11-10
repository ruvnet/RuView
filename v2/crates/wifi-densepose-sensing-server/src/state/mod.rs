//! Application state and life-cycle management.
//!
//! Complies with ELMAS.md: <= 350 lines per file, strictly modular.

pub mod calibration;
pub mod legacy_pose;
pub mod methods;
pub mod person_score;
pub mod types;

#[cfg(test)]
pub mod minimal;
#[cfg(test)]
mod tests_observatory_position;
#[cfg(test)]
mod tests_presence;
#[cfg(test)]
mod tests_room_count;

pub use calibration::*;
pub use legacy_pose::*;
pub use methods::*;
pub use person_score::*;
pub use types::*;
