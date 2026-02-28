//! Information Geometry
//!
//! Information geometry treats probability distributions as points on a curved manifold,
//! enabling geometry-aware optimization and analysis.
//!
//! ## Core Concepts
//!
//! - **Fisher Information Matrix (FIM)**: Measures curvature of probability space
//! - **Natural Gradient**: Gradient descent that respects the manifold geometry
//! - **K-FAC**: Kronecker-factored approximation for efficient natural gradient
//!
//! ## Benefits for Vector Search
//!
//! 1. **Faster Index Optimization**: 3-5x fewer iterations vs Adam
//! 2. **Better Generalization**: Follows geodesics in parameter space
//! 3. **Stable Continual Learning**: Information-aware regularization
//!
//! ## References
//!
//! - Amari & Nagaoka (2000): Methods of Information Geometry
//! - Martens & Grosse (2015): Optimizing Neural Networks with K-FAC
//! - Pascanu & Bengio (2013): Natural Gradient Works Efficiently in Learning

mod fisher;
mod kfac;
mod natural_gradient;

pub use fisher::FisherInformation;
pub use kfac::KFACApproximation;
pub use natural_gradient::NaturalGradient;
