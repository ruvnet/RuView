//! Information Geometry for Attention
//!
//! Natural gradient methods using Fisher information metric.
//!
//! ## Key Concepts
//!
//! 1. **Fisher Metric**: F = diag(p) - p*p^T on probability simplex
//! 2. **Natural Gradient**: Solve F*delta = grad, then update params -= lr*delta
//! 3. **Conjugate Gradient**: Efficient solver for Fisher system
//!
//! ## Use Cases
//!
//! - Training attention weights with proper geometry
//! - Routing probabilities in MoE
//! - Softmax logit optimization

mod fisher;
mod natural_gradient;

pub use fisher::{FisherConfig, FisherMetric};
pub use natural_gradient::{NaturalGradient, NaturalGradientConfig};

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_exists() {
        assert!(true);
    }
}
