//! Dendritic coincidence detection and integration
//!
//! This module implements reduced compartment dendritic models that detect
//! temporal coincidence of synaptic inputs within 10-50ms windows.
//!
//! ## Architecture
//!
//! - **Compartment**: Single compartment with membrane and calcium dynamics
//! - **Dendrite**: NMDA coincidence detector with plateau potential
//! - **DendriticTree**: Multi-branch dendritic tree with soma integration
//!
//! ## NMDA-like Nonlinearity
//!
//! When 5-35 synapses fire simultaneously within the coincidence window:
//! 1. Mg2+ block is removed by depolarization
//! 2. Ca2+ influx triggers plateau potential
//! 3. 100-500ms plateau duration enables BTSP
//!
//! ## Performance
//!
//! - Compartment update: <1μs
//! - Coincidence detection: <10μs for 100 synapses
//! - Suitable for real-time Cognitum deployment

mod coincidence;
mod compartment;
mod plateau;
mod tree;

pub use coincidence::Dendrite;
pub use compartment::Compartment;
pub use plateau::PlateauPotential;
pub use tree::DendriticTree;
