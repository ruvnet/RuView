//! Neural DAG learning for PostgreSQL query optimization
//!
//! This module integrates the SONA (Scalable On-device Neural Adaptation) engine
//! with PostgreSQL's query planner to provide learned query optimization.

pub mod functions;
pub mod state;

pub use state::{DagConfig, DagState, DAG_STATE};
