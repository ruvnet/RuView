//! # thermorust
//!
//! A minimal thermodynamic neural-motif crate for Rust.
//!
//! Treats computation as **energy-driven state transitions** with
//! Landauer-style dissipation and Langevin/Metropolis noise baked in.
//!
//! ## Core abstractions
//!
//! | Module | What it provides |
//! |--------|-----------------|
//! | [`state`] | `State` – activation vector + dissipated-joules counter |
//! | [`energy`] | `EnergyModel` trait, `Ising`, `SoftSpin`, `Couplings` |
//! | [`dynamics`] | `step_discrete` (MH), `step_continuous` (Langevin), annealers |
//! | [`noise`] | Langevin & Poisson spike noise sources |
//! | [`metrics`] | Magnetisation, overlap, entropy, free energy, `Trace` |
//! | [`motifs`] | Pre-wired ring / fully-connected / Hopfield / soft-spin motifs |
//!
//! ## Quick start
//!
//! ```no_run
//! use thermorust::{motifs::IsingMotif, dynamics::{Params, anneal_discrete}};
//! use rand::SeedableRng;
//!
//! let mut motif = IsingMotif::ring(16, 0.2);
//! let params    = Params::default_n(16);
//! let mut rng   = rand::rngs::StdRng::seed_from_u64(42);
//!
//! let trace = anneal_discrete(&motif.model, &mut motif.state, &params, 10_000, 100, &mut rng);
//! println!("Mean energy: {:.3}", trace.mean_energy());
//! println!("Heat shed:   {:.3e} J", trace.total_dissipation());
//! ```

pub mod dynamics;
pub mod energy;
pub mod metrics;
pub mod motifs;
pub mod noise;
pub mod state;

// Re-export the most commonly used items at the crate root.
pub use dynamics::{anneal_continuous, anneal_discrete, step_continuous, step_discrete, Params};
pub use energy::{Couplings, EnergyModel, Ising, SoftSpin};
pub use metrics::{magnetisation, overlap, Trace};
pub use state::State;
