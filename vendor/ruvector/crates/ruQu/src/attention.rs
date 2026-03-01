//! Mincut-Gated Attention Integration
//!
//! This module bridges ruQu's coherence gate with the `ruvector-mincut-gated-transformer`
//! crate's attention optimization mechanisms:
//!
//! 1. **GatePacket Bridge** - Convert ruQu's `TileReport` aggregates into `GatePacket`
//! 2. **MincutDepthRouter** - λ-based Mixture-of-Depths routing for 50% FLOPs reduction
//! 3. **CoherenceEarlyExit** - Layer skipping based on coherence stability
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruqu::attention::{CoherenceAttention, AttentionConfig};
//! use ruqu::tile::{TileReport, GateThresholds};
//!
//! // Create attention optimizer
//! let config = AttentionConfig::default();
//! let mut attention = CoherenceAttention::new(config);
//!
//! // Process syndrome patterns with coherence-optimized attention
//! let reports: Vec<TileReport> = collect_worker_reports();
//! let (gate_packet, routing) = attention.optimize(&reports);
//!
//! // Use routing decisions for efficient syndrome analysis
//! for (i, route) in routing.iter().enumerate() {
//!     if route.requires_compute() {
//!         // Full analysis for this syndrome entry
//!     } else {
//!         // Skip - coherence is stable, use cached result
//!     }
//! }
//! ```

#[cfg(feature = "attention")]
use ruvector_mincut_gated_transformer::{
    CoherenceEarlyExit, EarlyExitConfig, EarlyExitDecision, ExitReason, GatePacket,
    MincutDepthRouter, ModRoutingConfig, RoutingStats, TokenRoute,
};

use crate::tile::{GateDecision, TileReport};

/// Configuration for coherence-optimized attention
#[derive(Clone, Debug)]
pub struct AttentionConfig {
    /// Target FLOPs reduction (0.0-0.9), default 0.5 for 50%
    pub flops_reduction: f32,

    /// Minimum entries that must be processed per round
    pub min_entries_per_round: u16,

    /// λ-delta threshold for skipping (Q15 scale)
    /// Lower = more aggressive skipping
    pub lambda_delta_skip_threshold: i32,

    /// Enable adaptive capacity based on coherence stability
    pub adaptive_capacity: bool,

    /// Enable early exit when coherence is very stable
    pub enable_early_exit: bool,

    /// Early exit confidence threshold (0.0-1.0)
    pub early_exit_threshold: f32,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            flops_reduction: 0.5,
            min_entries_per_round: 4,
            lambda_delta_skip_threshold: 3276, // ~10% of Q15 range
            adaptive_capacity: true,
            enable_early_exit: true,
            early_exit_threshold: 0.95,
        }
    }
}

impl AttentionConfig {
    /// Configuration optimized for real-time coherence gating
    pub fn realtime() -> Self {
        Self {
            flops_reduction: 0.6, // More aggressive skip
            min_entries_per_round: 2,
            lambda_delta_skip_threshold: 2000, // More aggressive
            adaptive_capacity: true,
            enable_early_exit: true,
            early_exit_threshold: 0.9,
        }
    }

    /// Configuration optimized for accuracy (less skipping)
    pub fn accurate() -> Self {
        Self {
            flops_reduction: 0.3,
            min_entries_per_round: 8,
            lambda_delta_skip_threshold: 5000, // Less aggressive
            adaptive_capacity: false,
            enable_early_exit: false,
            early_exit_threshold: 0.99,
        }
    }
}

/// Bridge between ruQu's TileReport and GatePacket
///
/// Converts aggregated tile metrics into the format expected by
/// the mincut-gated-transformer system.
#[derive(Clone, Copy, Debug, Default)]
pub struct GatePacketBridge {
    /// Previous lambda for trend detection
    prev_lambda: u32,
    /// Smoothed boundary edge count
    smoothed_boundary: u16,
}

impl GatePacketBridge {
    /// Create a new bridge
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert tile reports into a GatePacket
    ///
    /// # Arguments
    /// * `reports` - Aggregated worker tile reports
    ///
    /// # Returns
    /// A `GatePacket` suitable for mincut-gated-transformer
    #[cfg(feature = "attention")]
    pub fn to_gate_packet(&mut self, reports: &[TileReport]) -> GatePacket {
        if reports.is_empty() {
            return GatePacket::default();
        }

        // Aggregate metrics from reports
        let mut min_cut = f64::MAX;
        let mut max_shift = 0.0f64;
        let mut total_boundary = 0u32;
        let mut max_boundary_concentration = 0u32;

        for report in reports {
            if report.local_cut < min_cut && report.local_cut > 0.0 {
                min_cut = report.local_cut;
            }
            if report.shift_score > max_shift {
                max_shift = report.shift_score;
            }
            // Use boundary candidate count as proxy for boundary edges
            total_boundary += report
                .boundary_candidates
                .iter()
                .filter(|&&c| c != 0)
                .count() as u32;

            // Higher shift = more concentrated boundaries
            let concentration = (report.shift_score * 32767.0) as u32;
            if concentration > max_boundary_concentration {
                max_boundary_concentration = concentration;
            }
        }

        // Convert min_cut to lambda (Q15-ish scale)
        // Higher min_cut = more coherent = higher lambda
        let lambda = (min_cut.clamp(0.0, 1000.0) * 32.767) as u32;

        // Smooth boundary edges
        let boundary_edges = ((total_boundary as u32 + self.smoothed_boundary as u32) / 2) as u16;
        self.smoothed_boundary = boundary_edges;

        // Build packet
        let packet = GatePacket {
            lambda,
            lambda_prev: self.prev_lambda,
            boundary_edges,
            boundary_concentration_q15: max_boundary_concentration.min(32767) as u16,
            partition_count: reports.len() as u16,
            flags: 0,
        };

        // Update history
        self.prev_lambda = lambda;

        packet
    }

    /// Convert a GatePacket back to approximate metrics
    #[cfg(feature = "attention")]
    pub fn from_gate_packet(packet: &GatePacket) -> (f64, f64, usize) {
        let min_cut = packet.lambda as f64 / 32.767;
        let shift_score = packet.boundary_concentration_q15 as f64 / 32767.0;
        let partition_count = packet.partition_count as usize;
        (min_cut, shift_score, partition_count)
    }
}

/// Coherence-optimized attention processor
///
/// Uses mincut signals to dynamically route syndrome entries through
/// the analysis pipeline, achieving up to 50% FLOPs reduction while
/// maintaining accuracy on critical boundary patterns.
#[cfg(feature = "attention")]
pub struct CoherenceAttention {
    config: AttentionConfig,
    router: MincutDepthRouter,
    bridge: GatePacketBridge,
    stats: AttentionStats,
}

#[cfg(feature = "attention")]
impl CoherenceAttention {
    /// Create a new coherence attention processor
    pub fn new(config: AttentionConfig) -> Self {
        let mod_config = ModRoutingConfig {
            lambda_delta_skip_threshold: config.lambda_delta_skip_threshold,
            boundary_token_force_compute: true,
            layer_capacity_ratio: 1.0 - config.flops_reduction,
            min_tokens_per_layer: config.min_entries_per_round,
            adaptive_capacity: config.adaptive_capacity,
        };

        Self {
            config,
            router: MincutDepthRouter::new(mod_config).unwrap_or_default(),
            bridge: GatePacketBridge::new(),
            stats: AttentionStats::default(),
        }
    }

    /// Optimize syndrome entry processing based on coherence
    ///
    /// # Arguments
    /// * `reports` - Worker tile reports with syndrome data
    ///
    /// # Returns
    /// Tuple of (GatePacket, routing decisions for each entry)
    pub fn optimize(&mut self, reports: &[TileReport]) -> (GatePacket, Vec<TokenRoute>) {
        let gate = self.bridge.to_gate_packet(reports);

        // Generate position indices for routing
        let positions: Vec<u16> = (0..reports.len() as u16).collect();

        // Route entries based on coherence
        let routes = self.router.route_tokens(&gate, &positions);

        // Update stats
        let routing_stats = self.router.routing_stats(&routes);
        self.stats.total_entries += routing_stats.total_tokens;
        self.stats.computed_entries += routing_stats.compute_tokens;
        self.stats.skipped_entries += routing_stats.skip_tokens;
        self.stats.boundary_entries += routing_stats.boundary_tokens;
        self.stats.decisions += 1;

        (gate, routes)
    }

    /// Check if early exit is warranted based on coherence stability
    ///
    /// # Arguments
    /// * `gate` - Current gate packet
    /// * `current_layer` - Current processing layer
    /// * `max_layers` - Maximum number of layers
    ///
    /// # Returns
    /// Early exit decision
    pub fn check_early_exit(
        &self,
        gate: &GatePacket,
        current_layer: usize,
        max_layers: usize,
    ) -> EarlyExitDecision {
        if !self.config.enable_early_exit {
            return EarlyExitDecision {
                should_exit: false,
                confidence: 0.0,
                reason: ExitReason::None,
            };
        }

        // Calculate coherence stability
        let lambda_delta_abs = gate.lambda_delta().abs() as f32;
        let stability = 1.0 - (lambda_delta_abs / 32768.0).min(1.0);

        // Calculate progress through layers
        let progress = current_layer as f32 / max_layers as f32;

        // Exit if very stable AND past midpoint
        let should_exit = stability > self.config.early_exit_threshold && progress > 0.5;

        EarlyExitDecision {
            should_exit,
            confidence: stability,
            reason: if should_exit {
                ExitReason::HighConfidence
            } else {
                ExitReason::None
            },
        }
    }

    /// Get accumulated statistics
    pub fn stats(&self) -> &AttentionStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = AttentionStats::default();
    }
}

/// Statistics for coherence attention
#[derive(Clone, Copy, Debug, Default)]
pub struct AttentionStats {
    /// Total entries processed
    pub total_entries: usize,
    /// Entries that required full computation
    pub computed_entries: usize,
    /// Entries that were skipped
    pub skipped_entries: usize,
    /// Boundary entries (always computed)
    pub boundary_entries: usize,
    /// Number of routing decisions made
    pub decisions: usize,
}

impl AttentionStats {
    /// Calculate FLOPs reduction ratio
    pub fn flops_reduction(&self) -> f32 {
        if self.total_entries == 0 {
            return 0.0;
        }
        self.skipped_entries as f32 / self.total_entries as f32
    }

    /// Calculate compute ratio
    pub fn compute_ratio(&self) -> f32 {
        if self.total_entries == 0 {
            return 0.0;
        }
        self.computed_entries as f32 / self.total_entries as f32
    }
}

/// Fallback types when attention feature is disabled
#[cfg(not(feature = "attention"))]
pub mod fallback {
    use super::*;

    /// Stub TokenRoute for when attention feature is disabled
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum TokenRoute {
        /// Process through full computation
        Compute,
        /// Skip - use cached result
        Skip,
        /// Boundary token - always compute
        Boundary,
    }

    impl TokenRoute {
        /// Check if this route requires computation
        pub fn requires_compute(&self) -> bool {
            !matches!(self, TokenRoute::Skip)
        }
    }

    /// Stub GatePacket for when attention feature is disabled
    #[derive(Clone, Copy, Debug, Default)]
    pub struct GatePacket {
        /// Current lambda (coherence metric)
        pub lambda: u32,
        /// Previous lambda for trend detection
        pub lambda_prev: u32,
        /// Number of boundary edges
        pub boundary_edges: u16,
        /// Boundary concentration (Q15 scale)
        pub boundary_concentration_q15: u16,
        /// Number of partitions
        pub partition_count: u16,
        /// Policy flags
        pub flags: u16,
    }

    impl GatePacket {
        /// Calculate lambda delta
        pub fn lambda_delta(&self) -> i32 {
            (self.lambda as i32) - (self.lambda_prev as i32)
        }
    }

    /// Simplified attention processor without transformer dependency
    pub struct CoherenceAttention {
        #[allow(dead_code)]
        config: AttentionConfig,
        bridge: GatePacketBridge,
        stats: AttentionStats,
    }

    impl CoherenceAttention {
        /// Create a new coherence attention processor
        pub fn new(config: AttentionConfig) -> Self {
            Self {
                config,
                bridge: GatePacketBridge::new(),
                stats: AttentionStats::default(),
            }
        }

        /// Optimize syndrome entry processing based on coherence
        pub fn optimize(&mut self, reports: &[TileReport]) -> (GatePacket, Vec<TokenRoute>) {
            let gate = self.bridge.to_gate_packet_fallback(reports);

            // Simple heuristic routing without transformer
            let routes: Vec<TokenRoute> = reports
                .iter()
                .enumerate()
                .map(|(i, report)| {
                    // Boundary tokens always compute
                    if report.boundary_candidates.iter().any(|&c| c != 0) {
                        return TokenRoute::Boundary;
                    }

                    // Skip if shift score is low (stable)
                    if report.shift_score < 0.1 && i % 2 == 0 {
                        return TokenRoute::Skip;
                    }

                    TokenRoute::Compute
                })
                .collect();

            // Update stats
            self.stats.total_entries += routes.len();
            self.stats.computed_entries += routes.iter().filter(|r| r.requires_compute()).count();
            self.stats.skipped_entries += routes
                .iter()
                .filter(|r| matches!(r, TokenRoute::Skip))
                .count();
            self.stats.boundary_entries += routes
                .iter()
                .filter(|r| matches!(r, TokenRoute::Boundary))
                .count();
            self.stats.decisions += 1;

            (gate, routes)
        }

        /// Get accumulated statistics
        pub fn stats(&self) -> &AttentionStats {
            &self.stats
        }

        /// Reset statistics
        pub fn reset_stats(&mut self) {
            self.stats = AttentionStats::default();
        }
    }

    impl GatePacketBridge {
        /// Convert tile reports to gate packet (fallback implementation)
        pub fn to_gate_packet_fallback(&mut self, reports: &[TileReport]) -> GatePacket {
            if reports.is_empty() {
                return GatePacket::default();
            }

            let mut min_cut = f64::MAX;
            let mut max_shift = 0.0f64;

            for report in reports {
                if report.local_cut < min_cut && report.local_cut > 0.0 {
                    min_cut = report.local_cut;
                }
                if report.shift_score > max_shift {
                    max_shift = report.shift_score;
                }
            }

            let lambda = (min_cut.clamp(0.0, 1000.0) * 32.767) as u32;

            let packet = GatePacket {
                lambda,
                lambda_prev: self.prev_lambda,
                boundary_edges: 0,
                boundary_concentration_q15: (max_shift * 32767.0) as u16,
                partition_count: reports.len() as u16,
                flags: 0,
            };

            self.prev_lambda = lambda;
            packet
        }
    }
}

#[cfg(not(feature = "attention"))]
pub use fallback::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_config_default() {
        let config = AttentionConfig::default();
        assert_eq!(config.flops_reduction, 0.5);
        assert!(config.enable_early_exit);
    }

    #[test]
    fn test_attention_config_realtime() {
        let config = AttentionConfig::realtime();
        assert!(config.flops_reduction > 0.5);
    }

    #[test]
    fn test_gate_packet_bridge() {
        let mut bridge = GatePacketBridge::new();

        // First call establishes baseline
        let reports = vec![
            {
                let mut r = TileReport::new(1);
                r.local_cut = 10.0;
                r.shift_score = 0.2;
                r
            },
            {
                let mut r = TileReport::new(2);
                r.local_cut = 15.0;
                r.shift_score = 0.1;
                r
            },
        ];

        #[cfg(feature = "attention")]
        {
            let packet = bridge.to_gate_packet(&reports);
            assert!(packet.lambda > 0);
            assert_eq!(packet.partition_count, 2);
        }

        #[cfg(not(feature = "attention"))]
        {
            let packet = bridge.to_gate_packet_fallback(&reports);
            assert!(packet.lambda > 0);
            assert_eq!(packet.partition_count, 2);
        }
    }

    #[test]
    fn test_attention_stats() {
        let mut stats = AttentionStats::default();
        stats.total_entries = 100;
        stats.computed_entries = 60;
        stats.skipped_entries = 40;

        assert_eq!(stats.flops_reduction(), 0.4);
        assert_eq!(stats.compute_ratio(), 0.6);
    }
}
