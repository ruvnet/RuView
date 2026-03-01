//! Modern Hopfield Networks
//!
//! This module implements the modern Hopfield network formulation from
//! Ramsauer et al. (2020), which provides exponential storage capacity
//! and is mathematically equivalent to transformer attention.
//!
//! ## Components
//!
//! - [`ModernHopfield`]: Main network structure
//! - [`retrieval`]: Softmax-weighted retrieval implementation
//! - [`capacity`]: Capacity calculations and β tuning

mod capacity;
mod network;
mod retrieval;

pub use capacity::{optimal_beta, theoretical_capacity};
pub use network::ModernHopfield;
pub use retrieval::{compute_attention, softmax};

#[cfg(test)]
mod tests;
