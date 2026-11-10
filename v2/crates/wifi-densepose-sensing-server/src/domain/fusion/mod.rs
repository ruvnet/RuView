//! Multi-node feature fusion, correlation-based person estimation, and occupancy aggregation.

pub mod aggregate;
pub mod correlation;
pub mod multi_node;
pub mod person_score;

pub use aggregate::aggregate_person_count;
pub use correlation::{corr_persons_to_score, estimate_persons_from_correlation};
pub use multi_node::fuse_multi_node_features;
pub use person_score::compute_person_score;
