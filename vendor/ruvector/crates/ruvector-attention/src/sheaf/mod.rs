//! Sheaf Attention Module
//!
//! Implements Coherence-Gated Transformer (CGT) attention mechanisms based on ADR-015.
//!
//! ## Key Concepts
//!
//! - **Sheaf Attention**: Attention weights inversely proportional to residual energy
//! - **Restriction Maps**: Replace learned W_q, W_k, W_v projections with geometric maps
//! - **Token Routing**: Route tokens to compute lanes based on coherence energy
//! - **Residual-Sparse Attention**: Only attend to high-residual (incoherent) pairs
//! - **Energy-Based Early Exit**: Exit when energy converges, not confidence threshold
//!
//! ## Mathematical Foundation
//!
//! Given tokens X = {x_1, ..., x_N} and restriction maps rho_i, rho_j:
//!
//! ```text
//! Residual:        r_ij = rho_i(x_i) - rho_j(x_j)
//! Edge energy:     E_ij = w_ij * ||r_ij||^2
//! Token energy:    E_i = sum_j E_ij
//! Attention:       A_ij = exp(-beta * E_ij) / Z
//! ```
//!
//! ## Example
//!
//! ```rust
//! use ruvector_attention::sheaf::{
//!     SheafAttention, SheafAttentionConfig,
//!     RestrictionMap, ComputeLane, TokenRouter,
//! };
//!
//! // Create sheaf attention with default config
//! let config = SheafAttentionConfig::default();
//! let attention = SheafAttention::new(config);
//!
//! // Create restriction maps for QKV
//! let rho_q = RestrictionMap::new(64, 64);
//! let rho_k = RestrictionMap::new(64, 64);
//! let rho_v = RestrictionMap::new(64, 64);
//! ```

mod attention;
mod early_exit;
mod restriction;
mod router;
mod sparse;

pub use attention::{SheafAttention, SheafAttentionConfig};
pub use early_exit::{
    process_with_early_exit, EarlyExit, EarlyExitConfig, EarlyExitResult, EarlyExitStatistics,
    ExitReason,
};
pub use restriction::{RestrictionMap, RestrictionMapConfig};
pub use router::{ComputeLane, LaneStatistics, RoutingDecision, TokenRouter, TokenRouterConfig};
pub use sparse::{
    ResidualSparseMask, SparseResidualAttention, SparseResidualConfig, SparsityStatistics,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all public types are accessible
        let config = SheafAttentionConfig::default();
        assert!(config.beta > 0.0);

        let rmap_config = RestrictionMapConfig::default();
        assert!(rmap_config.input_dim > 0);

        let router_config = TokenRouterConfig::default();
        assert!(router_config.theta_reflex > 0.0);

        let early_exit_config = EarlyExitConfig::default();
        assert!(early_exit_config.epsilon > 0.0);

        let sparse_config = SparseResidualConfig::default();
        assert!(sparse_config.residual_threshold > 0.0);
    }

    #[test]
    fn test_compute_lane_ordering() {
        assert!(ComputeLane::Reflex < ComputeLane::Standard);
        assert!(ComputeLane::Standard < ComputeLane::Deep);
        assert!(ComputeLane::Deep < ComputeLane::Escalate);
    }
}
