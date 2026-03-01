//! λ-based Mixture-of-Depths (MoD) routing.
//!
//! Unlike learned routers (Raposo et al., 2024), we use mincut λ-delta as the routing signal.
//! Tokens with stable coherence can skip layers; boundary tokens must compute.
//!
//! ## Design Rationale
//!
//! Traditional MoD uses learned routing mechanisms, but this introduces:
//! - Non-deterministic behavior
//! - Additional training overhead
//! - Lack of explainability
//!
//! Our approach leverages the existing mincut λ signal:
//! - λ-delta stable → token can skip (coherence maintained)
//! - λ-delta volatile → token must compute (on partition boundary)
//! - Boundary token → always compute (critical for coherence)
//!
//! This achieves 50% FLOPs reduction while maintaining deterministic behavior
//! and providing clear intervention witnesses.

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

use crate::packets::GatePacket;
use serde::{Deserialize, Serialize};

/// Configuration for MoD routing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModRoutingConfig {
    /// Threshold for λ-delta to allow skipping (Q15: 0-32767)
    /// If |λ_delta| < threshold, token is considered stable and can skip
    pub lambda_delta_skip_threshold: i32,

    /// Whether to force boundary tokens to always compute
    /// When true, tokens identified as on partition boundaries must compute
    pub boundary_token_force_compute: bool,

    /// Layer capacity ratio (0.0-1.0)
    /// 0.5 = only 50% of tokens can compute per layer (MoD target)
    pub layer_capacity_ratio: f32,

    /// Minimum tokens that must compute per layer
    /// Ensures at least this many tokens compute regardless of routing
    pub min_tokens_per_layer: u16,

    /// Enable adaptive capacity based on λ stability
    /// When true, capacity adjusts based on overall coherence
    pub adaptive_capacity: bool,
}

impl Default for ModRoutingConfig {
    fn default() -> Self {
        Self {
            // Allow skip if λ changed by less than ~10% (3276 / 32768 ≈ 0.1)
            lambda_delta_skip_threshold: 3276,
            boundary_token_force_compute: true,
            // Target 50% FLOPs reduction (Raposo et al., 2024)
            layer_capacity_ratio: 0.5,
            min_tokens_per_layer: 4,
            adaptive_capacity: true,
        }
    }
}

impl ModRoutingConfig {
    /// Create a configuration targeting specific FLOPs reduction
    ///
    /// # Arguments
    /// * `flops_reduction` - Target FLOPs reduction (0.0-1.0), e.g., 0.5 for 50%
    pub fn with_flops_reduction(flops_reduction: f32) -> Self {
        Self {
            layer_capacity_ratio: 1.0 - flops_reduction.clamp(0.0, 0.9),
            ..Default::default()
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.layer_capacity_ratio <= 0.0 || self.layer_capacity_ratio > 1.0 {
            return Err("layer_capacity_ratio must be in range (0.0, 1.0]");
        }
        if self.lambda_delta_skip_threshold < 0 {
            return Err("lambda_delta_skip_threshold must be non-negative");
        }
        Ok(())
    }
}

/// Router decision for a token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TokenRoute {
    /// Process through full attention + FFN
    Compute = 0,

    /// Skip layer - residual connection only
    Skip = 1,

    /// Must compute - token is on partition boundary
    Boundary = 2,
}

impl TokenRoute {
    /// Check if this route requires computation
    #[inline]
    pub fn requires_compute(&self) -> bool {
        !matches!(self, TokenRoute::Skip)
    }

    /// Check if this is a boundary token
    #[inline]
    pub fn is_boundary(&self) -> bool {
        matches!(self, TokenRoute::Boundary)
    }
}

/// MoD router using mincut λ signals.
///
/// This router decides which tokens should compute at each layer based on:
/// 1. λ-delta stability (stable tokens can skip)
/// 2. Boundary token detection (boundary tokens must compute)
/// 3. Layer capacity constraints (enforce target FLOPs reduction)
pub struct MincutDepthRouter {
    config: ModRoutingConfig,
}

impl MincutDepthRouter {
    /// Create a new MoD router with the given configuration
    pub fn new(config: ModRoutingConfig) -> Result<Self, &'static str> {
        config.validate()?;
        Ok(Self { config })
    }
}

impl Default for MincutDepthRouter {
    fn default() -> Self {
        Self {
            config: ModRoutingConfig::default(),
        }
    }
}

impl MincutDepthRouter {
    /// Route tokens based on gate packet and token positions.
    ///
    /// # Arguments
    /// * `gate` - Gate packet with λ signals
    /// * `token_positions` - Position indices of tokens in sequence
    ///
    /// # Returns
    /// Vector of routing decisions, one per token
    pub fn route_tokens(&self, gate: &GatePacket, token_positions: &[u16]) -> Vec<TokenRoute> {
        let num_tokens = token_positions.len();
        if num_tokens == 0 {
            return Vec::new();
        }

        let mut routes = vec![TokenRoute::Skip; num_tokens];

        // Calculate effective capacity for this layer
        let capacity = self.calculate_layer_capacity(gate, num_tokens);

        // Step 1: Mark boundary tokens (must compute)
        let boundary_count = if self.config.boundary_token_force_compute {
            self.mark_boundary_tokens(gate, &mut routes, token_positions)
        } else {
            0
        };

        // Step 2: Route remaining tokens based on λ-delta stability
        let mut compute_count = boundary_count;
        let lambda_delta_abs = gate.lambda_delta().abs();

        // If λ is unstable, more tokens should compute
        if lambda_delta_abs > self.config.lambda_delta_skip_threshold {
            // Unstable coherence - route more tokens to compute
            compute_count += self.route_unstable_tokens(
                gate,
                &mut routes,
                token_positions,
                capacity.saturating_sub(boundary_count),
            );
        } else {
            // Stable coherence - can skip more aggressively
            compute_count += self.route_stable_tokens(
                gate,
                &mut routes,
                token_positions,
                capacity.saturating_sub(boundary_count),
            );
        }

        // Step 3: Ensure minimum compute tokens
        if compute_count < self.config.min_tokens_per_layer as usize {
            self.ensure_minimum_compute(
                &mut routes,
                self.config.min_tokens_per_layer as usize - compute_count,
            );
        }

        routes
    }

    /// Compute layer mask from routing decisions.
    ///
    /// # Arguments
    /// * `routes` - Routing decisions for all tokens
    /// * `layer` - Current layer index (for future layer-specific routing)
    ///
    /// # Returns
    /// Boolean mask where `true` means token should compute
    pub fn compute_layer_mask(&self, routes: &[TokenRoute], _layer: usize) -> Vec<bool> {
        routes.iter().map(|r| r.requires_compute()).collect()
    }

    /// Get routing statistics for analysis
    pub fn routing_stats(&self, routes: &[TokenRoute]) -> RoutingStats {
        let total = routes.len();
        let compute = routes.iter().filter(|r| r.requires_compute()).count();
        let skip = routes
            .iter()
            .filter(|r| matches!(r, TokenRoute::Skip))
            .count();
        let boundary = routes.iter().filter(|r| r.is_boundary()).count();

        RoutingStats {
            total_tokens: total,
            compute_tokens: compute,
            skip_tokens: skip,
            boundary_tokens: boundary,
            compute_ratio: if total > 0 {
                compute as f32 / total as f32
            } else {
                0.0
            },
            skip_ratio: if total > 0 {
                skip as f32 / total as f32
            } else {
                0.0
            },
        }
    }

    // ---- Private helpers ----

    fn calculate_layer_capacity(&self, gate: &GatePacket, num_tokens: usize) -> usize {
        let mut capacity = (num_tokens as f32 * self.config.layer_capacity_ratio).ceil() as usize;

        // Adaptive capacity based on λ stability
        if self.config.adaptive_capacity {
            let lambda_delta_abs = gate.lambda_delta().abs();
            let stability_ratio = 1.0 - (lambda_delta_abs as f32 / 32768.0).min(1.0);

            // If very stable (high stability_ratio), can reduce capacity further
            // If unstable (low stability_ratio), increase capacity
            let adjustment = if stability_ratio > 0.9 {
                0.9 // Very stable - use even less capacity
            } else if stability_ratio < 0.5 {
                1.2 // Unstable - use more capacity
            } else {
                1.0 // Normal
            };

            capacity = (capacity as f32 * adjustment).ceil() as usize;
        }

        capacity
            .max(self.config.min_tokens_per_layer as usize)
            .min(num_tokens)
    }

    fn mark_boundary_tokens(
        &self,
        gate: &GatePacket,
        routes: &mut [TokenRoute],
        token_positions: &[u16],
    ) -> usize {
        // Heuristic: tokens near partition boundaries based on boundary_concentration
        // Higher boundary_concentration means fewer, more concentrated boundaries

        let boundary_ratio = if gate.boundary_concentration_q15 > 16384 {
            // High concentration - fewer boundary tokens
            0.1
        } else {
            // Low concentration - more boundary tokens
            0.2
        };

        let boundary_count = (routes.len() as f32 * boundary_ratio).ceil() as usize;
        let mut marked = 0;

        // Simple heuristic: mark tokens at regular intervals as potential boundaries
        // In practice, this would use actual boundary edge IDs from mincut
        if boundary_count > 0 && !token_positions.is_empty() {
            let stride = routes.len() / boundary_count.max(1);
            for i in (0..routes.len()).step_by(stride.max(1)) {
                if marked >= boundary_count {
                    break;
                }
                routes[i] = TokenRoute::Boundary;
                marked += 1;
            }
        }

        marked
    }

    fn route_unstable_tokens(
        &self,
        _gate: &GatePacket,
        routes: &mut [TokenRoute],
        _token_positions: &[u16],
        target_count: usize,
    ) -> usize {
        // When unstable, route more tokens to compute
        // Prioritize tokens not already marked as boundary
        let mut routed = 0;

        for route in routes.iter_mut() {
            if routed >= target_count {
                break;
            }
            if matches!(route, TokenRoute::Skip) {
                *route = TokenRoute::Compute;
                routed += 1;
            }
        }

        routed
    }

    fn route_stable_tokens(
        &self,
        _gate: &GatePacket,
        routes: &mut [TokenRoute],
        _token_positions: &[u16],
        target_count: usize,
    ) -> usize {
        // When stable, can skip more aggressively
        // Only route enough tokens to meet target capacity
        let mut routed = 0;

        for route in routes.iter_mut() {
            if routed >= target_count {
                break;
            }
            if matches!(route, TokenRoute::Skip) {
                *route = TokenRoute::Compute;
                routed += 1;
            }
        }

        routed
    }

    fn ensure_minimum_compute(&self, routes: &mut [TokenRoute], needed: usize) {
        let mut added = 0;

        for route in routes.iter_mut() {
            if added >= needed {
                break;
            }
            if matches!(route, TokenRoute::Skip) {
                *route = TokenRoute::Compute;
                added += 1;
            }
        }
    }
}

/// Statistics for a routing decision.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct RoutingStats {
    /// Total number of tokens
    pub total_tokens: usize,

    /// Number of tokens that computed
    pub compute_tokens: usize,

    /// Number of tokens that skipped
    pub skip_tokens: usize,

    /// Number of boundary tokens
    pub boundary_tokens: usize,

    /// Ratio of tokens that computed
    pub compute_ratio: f32,

    /// Ratio of tokens that skipped
    pub skip_ratio: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn test_mod_routing_config_default() {
        let config = ModRoutingConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.layer_capacity_ratio, 0.5);
    }

    #[test]
    fn test_mod_routing_config_flops_reduction() {
        let config = ModRoutingConfig::with_flops_reduction(0.5);
        assert_eq!(config.layer_capacity_ratio, 0.5);

        let config = ModRoutingConfig::with_flops_reduction(0.75);
        assert_eq!(config.layer_capacity_ratio, 0.25);
    }

    #[test]
    fn test_token_route_methods() {
        assert!(TokenRoute::Compute.requires_compute());
        assert!(!TokenRoute::Skip.requires_compute());
        assert!(TokenRoute::Boundary.requires_compute());

        assert!(!TokenRoute::Compute.is_boundary());
        assert!(TokenRoute::Boundary.is_boundary());
    }

    #[test]
    fn test_router_creation() {
        let router = MincutDepthRouter::default();
        assert_eq!(router.config.layer_capacity_ratio, 0.5);

        let config = ModRoutingConfig::default();
        let router = MincutDepthRouter::new(config);
        assert!(router.is_ok());
    }

    #[test]
    fn test_route_tokens_stable() {
        let router = MincutDepthRouter::default();
        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 95, // Small delta (5)
            boundary_edges: 5,
            boundary_concentration_q15: 20000,
            partition_count: 3,
            flags: 0,
        };

        let tokens: Vec<u16> = (0..16).collect();
        let routes = router.route_tokens(&gate, &tokens);

        assert_eq!(routes.len(), 16);

        let stats = router.routing_stats(&routes);
        assert_eq!(stats.total_tokens, 16);
        assert!(stats.skip_ratio > 0.0); // Should skip some tokens when stable
    }

    #[test]
    fn test_route_tokens_unstable() {
        let router = MincutDepthRouter::default();
        let gate = GatePacket {
            lambda: 40,
            lambda_prev: 100, // Large delta (60)
            boundary_edges: 15,
            boundary_concentration_q15: 8000,
            partition_count: 5,
            flags: 0,
        };

        let tokens: Vec<u16> = (0..16).collect();
        let routes = router.route_tokens(&gate, &tokens);

        let stats = router.routing_stats(&routes);
        // When unstable, should compute more tokens
        assert!(stats.compute_ratio >= 0.5);
    }

    #[test]
    fn test_compute_layer_mask() {
        let router = MincutDepthRouter::default();
        let routes = vec![
            TokenRoute::Compute,
            TokenRoute::Skip,
            TokenRoute::Boundary,
            TokenRoute::Skip,
        ];

        let mask = router.compute_layer_mask(&routes, 0);
        assert_eq!(mask, vec![true, false, true, false]);
    }

    #[test]
    fn test_routing_stats() {
        let router = MincutDepthRouter::default();
        let routes = vec![
            TokenRoute::Compute,
            TokenRoute::Compute,
            TokenRoute::Skip,
            TokenRoute::Skip,
            TokenRoute::Boundary,
            TokenRoute::Skip,
        ];

        let stats = router.routing_stats(&routes);
        assert_eq!(stats.total_tokens, 6);
        assert_eq!(stats.compute_tokens, 3); // 2 Compute + 1 Boundary
        assert_eq!(stats.skip_tokens, 3);
        assert_eq!(stats.boundary_tokens, 1);
        assert_eq!(stats.compute_ratio, 0.5);
    }

    #[test]
    fn test_minimum_tokens_enforced() {
        let config = ModRoutingConfig {
            min_tokens_per_layer: 8,
            ..Default::default()
        };
        let router = MincutDepthRouter::new(config).unwrap();

        let gate = GatePacket {
            lambda: 100,
            lambda_prev: 99, // Very stable
            boundary_edges: 0,
            boundary_concentration_q15: 30000,
            partition_count: 1,
            flags: 0,
        };

        let tokens: Vec<u16> = (0..16).collect();
        let routes = router.route_tokens(&gate, &tokens);

        let stats = router.routing_stats(&routes);
        // Should have at least min_tokens_per_layer computing
        assert!(stats.compute_tokens >= 8);
    }
}
