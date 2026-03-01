//! # Thermodynamic Learning: Physics-Based Intelligence Research
//!
//! This library implements cutting-edge thermodynamic learning algorithms
//! that approach the Landauer limit: **kT ln(2) ≈ 2.9 × 10⁻²¹ J per bit**.
//!
//! ## Modules
//!
//! - [`landauer_learning`]: Near-Landauer-limit optimization with energy accounting
//! - [`equilibrium_propagation`]: Thermodynamic backpropagation via energy minimization
//! - [`free_energy_agent`]: Karl Friston's Free Energy Principle and active inference
//! - [`reversible_neural`]: Reversible neural networks for near-zero dissipation
//!
//! ## Key Features
//!
//! - **Energy-aware optimization**: Track thermodynamic efficiency in real-time
//! - **Physics-based learning**: Energy minimization, equilibrium propagation
//! - **Reversible computation**: Approach zero dissipation through bijective layers
//! - **Active inference**: Minimize variational free energy for intelligent behavior
//! - **SIMD optimizations**: Accelerated energy calculations for performance
//!
//! ## Example
//!
//! ```rust
//! use thermodynamic_learning::landauer_learning::{LandauerOptimizer, constants};
//!
//! let mut optimizer = LandauerOptimizer::new(0.01, constants::ROOM_TEMP);
//! optimizer.use_reversible = true;
//! optimizer.adiabatic_factor = 100.0;
//!
//! let gradient = vec![1.0, -0.5, 0.3];
//! let mut params = vec![1.0, 2.0, 3.0];
//!
//! optimizer.step(&gradient, &mut params);
//!
//! println!("{}", optimizer.efficiency_report());
//! // Output: Operating at 10-100× Landauer limit (vs 10⁹× for GPUs)
//! ```

#![warn(missing_docs)]
#![allow(dead_code)]

/// Landauer-optimal learning: energy-aware optimization approaching thermodynamic limits
pub mod landauer_learning;

/// Equilibrium propagation: physics-based learning via energy minimization
pub mod equilibrium_propagation;

/// Free energy principle: Karl Friston's active inference framework
pub mod free_energy_agent;

/// Reversible neural networks: near-zero dissipation through bijective transformations
pub mod reversible_neural;

/// SIMD-accelerated energy calculations and optimizations
#[cfg(feature = "simd")]
pub mod simd_ops;

/// Novel thermodynamic learning algorithms discovered through research
pub mod novel_algorithms;

// Re-export commonly used items
pub use equilibrium_propagation::EnergyBasedNetwork;
pub use free_energy_agent::FreeEnergyAgent;
pub use landauer_learning::{constants, LandauerOptimizer, ThermodynamicState};
pub use reversible_neural::ReversibleNetwork;
