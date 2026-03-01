//! MinCut Configuration
//!
//! Configuration for the subpolynomial dynamic minimum cut algorithm.

use serde::{Deserialize, Serialize};

/// Configuration for the mincut incoherence isolator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinCutConfig {
    /// Expansion parameter phi = 2^{-Theta(log^{3/4} n)}
    pub phi: f64,

    /// Maximum cut size to support exactly
    /// lambda_max = 2^{Theta(log^{3/4-c} n)}
    pub lambda_max: u64,

    /// Approximation parameter epsilon
    pub epsilon: f64,

    /// Target number of hierarchy levels: O(log^{1/4} n)
    pub target_levels: usize,

    /// Enable recourse tracking
    pub track_recourse: bool,

    /// Enable cut certification
    pub certify_cuts: bool,

    /// Enable parallel processing
    pub parallel: bool,

    /// Default isolation threshold
    pub default_threshold: f64,

    /// Maximum iterations for isolation refinement
    pub max_isolation_iters: usize,
}

impl Default for MinCutConfig {
    fn default() -> Self {
        Self {
            phi: 0.01,
            lambda_max: 1000,
            epsilon: 0.1,
            target_levels: 4,
            track_recourse: true,
            certify_cuts: true,
            parallel: true,
            default_threshold: 1.0,
            max_isolation_iters: 10,
        }
    }
}

impl MinCutConfig {
    /// Create configuration optimized for graph of size n
    pub fn for_size(n: usize) -> Self {
        let log_n = (n.max(2) as f64).ln();

        // phi = 2^{-Theta(log^{3/4} n)}
        let phi = 2.0_f64.powf(-log_n.powf(0.75) / 4.0);

        // lambda_max = 2^{Theta(log^{3/4-c} n)} with c = 0.1
        let lambda_max = 2.0_f64.powf(log_n.powf(0.65)).min(1e9) as u64;

        // Target levels = O(log^{1/4} n)
        let target_levels = (log_n.powf(0.25).ceil() as usize).max(2).min(10);

        Self {
            phi,
            lambda_max,
            epsilon: 0.1,
            target_levels,
            track_recourse: true,
            certify_cuts: true,
            parallel: n > 10000,
            default_threshold: 1.0,
            max_isolation_iters: 10,
        }
    }

    /// Create configuration for small graphs (< 1K vertices)
    pub fn small() -> Self {
        Self {
            phi: 0.1,
            lambda_max: 100,
            target_levels: 2,
            parallel: false,
            ..Default::default()
        }
    }

    /// Create configuration for large graphs (> 100K vertices)
    pub fn large() -> Self {
        Self::for_size(100_000)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.phi <= 0.0 || self.phi >= 1.0 {
            return Err(format!("phi must be in (0, 1), got {}", self.phi));
        }
        if self.lambda_max == 0 {
            return Err("lambda_max must be positive".to_string());
        }
        if self.epsilon <= 0.0 || self.epsilon >= 1.0 {
            return Err(format!("epsilon must be in (0, 1), got {}", self.epsilon));
        }
        if self.target_levels == 0 {
            return Err("target_levels must be positive".to_string());
        }
        Ok(())
    }

    /// Compute theoretical subpolynomial bound for graph of size n
    pub fn theoretical_bound(&self, n: usize) -> f64 {
        if n < 2 {
            return f64::INFINITY;
        }
        let log_n = (n as f64).ln();
        // 2^{O(log^{1-c} n)} with c = 0.1
        2.0_f64.powf(log_n.powf(0.9))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MinCutConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_for_size() {
        let small_config = MinCutConfig::for_size(100);
        let large_config = MinCutConfig::for_size(1_000_000);

        // Larger graphs should have smaller phi
        assert!(large_config.phi < small_config.phi);

        // Larger graphs should have more levels
        assert!(large_config.target_levels >= small_config.target_levels);
    }

    #[test]
    fn test_theoretical_bound() {
        let config = MinCutConfig::default();

        let bound_100 = config.theoretical_bound(100);
        let bound_1m = config.theoretical_bound(1_000_000);

        // Bound should increase with n, but subpolynomially
        assert!(bound_1m > bound_100);

        // Should be much smaller than n
        assert!(bound_1m < 1_000_000.0);
    }
}
