//! Per-node state machine and data structures.
//!
//! Complies with ELMAS.md: <= 350 lines, modular architecture.

pub mod constants;
pub mod eviction;
pub mod features;
pub mod grid;
pub mod inference;
pub mod position;
pub mod state;
pub mod sync;

#[cfg(test)]
mod tests_sync_serialization;
#[cfg(test)]
mod tests_novelty;

pub use constants::*;
pub use eviction::*;
pub use features::build_node_features;
pub use inference::*;
pub use position::*;
pub use state::NodeState;
