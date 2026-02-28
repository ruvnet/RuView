//! Hyperbolic Attention Module
//!
//! Implements attention mechanisms in hyperbolic space using:
//! - Poincaré ball model (traditional)
//! - Lorentz hyperboloid model (novel - faster, more stable)

pub mod hyperbolic_attention;
pub mod lorentz_cascade;
pub mod mixed_curvature;
pub mod poincare;

pub use poincare::{
    exp_map, frechet_mean, log_map, mobius_add, mobius_scalar_mult, poincare_distance,
    project_to_ball,
};

pub use hyperbolic_attention::{HyperbolicAttention, HyperbolicAttentionConfig};

pub use mixed_curvature::{MixedCurvatureAttention, MixedCurvatureConfig};

// Novel Lorentz Cascade Attention (LCA)
pub use lorentz_cascade::{
    busemann_score, einstein_midpoint, horosphere_attention_weights, lorentz_distance,
    lorentz_inner, project_hyperboloid, CascadeHead, LCAConfig, LorentzCascadeAttention,
};
