//! Sheaf Cohomology Module for Prime-Radiant
//!
//! This module implements sheaf cohomology computations for detecting global
//! inconsistencies (obstructions) in the coherence graph. Sheaf cohomology
//! provides powerful tools for understanding when local consistency cannot
//! be extended to global consistency.
//!
//! # Mathematical Background
//!
//! For a sheaf F on a graph G, the cohomology groups H^n(G, F) measure
//! obstructions to extending local sections to global ones:
//!
//! - **H^0(G, F)**: Global sections (globally consistent assignments)
//! - **H^1(G, F)**: First cohomology (obstructions to patching local data)
//!
//! The key computational tool is the **coboundary operator** delta:
//! ```text
//! delta^0: C^0(G, F) -> C^1(G, F)
//! (delta^0 f)(e) = rho_t(f(t(e))) - rho_s(f(s(e)))
//! ```
//!
//! where rho_s, rho_t are the restriction maps on edge e.
//!
//! # Sheaf Laplacian
//!
//! The **sheaf Laplacian** L = delta^T delta generalizes the graph Laplacian:
//! ```text
//! L_F = sum_e w_e (rho_s - rho_t)^T (rho_s - rho_t)
//! ```
//!
//! Its spectrum reveals global structure:
//! - Zero eigenvalues correspond to cohomology classes
//! - Small eigenvalues indicate near-obstructions
//!
//! # References
//!
//! 1. Hansen, J., & Ghrist, R. (2019). "Toward a spectral theory of cellular sheaves."
//! 2. Robinson, M. (2014). "Topological Signal Processing."
//! 3. Curry, J. (2014). "Sheaves, Cosheaves, and Applications."

mod cocycle;
mod cohomology_group;
mod diffusion;
mod laplacian;
mod neural;
mod obstruction;
mod sheaf;
mod simplex;

pub use cocycle::{Coboundary, Cocycle, CocycleBuilder};
pub use cohomology_group::{BettiNumbers, CohomologyComputer, CohomologyConfig, CohomologyGroup};
pub use diffusion::{DiffusionResult, ObstructionIndicator, SheafDiffusion, SheafDiffusionConfig};
pub use laplacian::{HarmonicRepresentative, LaplacianConfig, LaplacianSpectrum, SheafLaplacian};
pub use neural::{
    Activation, CohomologyPooling, PoolingMethod, SheafConvolution, SheafNeuralConfig,
    SheafNeuralLayer,
};
pub use obstruction::{Obstruction, ObstructionDetector, ObstructionReport, ObstructionSeverity};
pub use sheaf::{LocalSection, Sheaf, SheafBuilder, SheafSection, Stalk};
pub use simplex::{Chain, Cochain, Simplex, SimplexId, SimplicialComplex};
