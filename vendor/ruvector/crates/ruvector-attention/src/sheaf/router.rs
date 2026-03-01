//! Token Router for Coherence-Gated Transformer
//!
//! Routes tokens to different compute lanes based on coherence energy:
//!
//! - **Reflex** (Lane 0): E < theta_reflex, minimal compute (<0.1ms)
//! - **Standard** (Lane 1): E < theta_standard, normal compute (~1ms)
//! - **Deep** (Lane 2): E >= theta_standard, maximum compute (~5ms)
//! - **Escalate** (Lane 3): Irreconcilable incoherence, return uncertainty
//!
//! ## Routing Thresholds
//!
//! | Threshold | Default | Meaning |
//! |-----------|---------|---------|
//! | theta_reflex | 0.01 | Token highly coherent with context |
//! | theta_standard | 0.1 | Minor inconsistencies |
//! | theta_deep | 1.0 | Major inconsistencies |
//! | theta_escalate | 10.0 | Irreconcilable (escalate) |

use crate::error::{AttentionError, AttentionResult};
use crate::sheaf::SheafAttention;
use serde::{Deserialize, Serialize};

/// Compute lane for token processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ComputeLane {
    /// Minimal compute (<0.1ms): 1-2 layers, local attention, no FFN
    /// Use case: Common tokens, clear context
    Reflex = 0,

    /// Standard compute (~1ms): 6 layers, sparse sheaf attention
    /// Use case: Normal tokens requiring context integration
    Standard = 1,

    /// Deep compute (~5ms): 12+ layers, full sheaf + MoE
    /// Use case: Ambiguous, contradictory, or complex tokens
    Deep = 2,

    /// Escalate: Return uncertainty, request clarification
    /// Use case: Irreconcilable incoherence
    Escalate = 3,
}

impl ComputeLane {
    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::Reflex => "Reflex (minimal compute)",
            Self::Standard => "Standard (normal compute)",
            Self::Deep => "Deep (maximum compute)",
            Self::Escalate => "Escalate (return uncertainty)",
        }
    }

    /// Get typical latency in milliseconds
    pub fn typical_latency_ms(&self) -> f32 {
        match self {
            Self::Reflex => 0.1,
            Self::Standard => 1.0,
            Self::Deep => 5.0,
            Self::Escalate => 0.0, // Async/immediate return
        }
    }

    /// Get typical number of layers
    pub fn typical_layers(&self) -> usize {
        match self {
            Self::Reflex => 2,
            Self::Standard => 6,
            Self::Deep => 12,
            Self::Escalate => 0,
        }
    }

    /// Check if this lane requires full attention
    pub fn requires_full_attention(&self) -> bool {
        matches!(self, Self::Deep)
    }

    /// Check if this lane uses MoE routing
    pub fn uses_moe(&self) -> bool {
        matches!(self, Self::Deep)
    }
}

/// Configuration for token router
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRouterConfig {
    /// Energy threshold for reflex lane (E < theta_reflex -> Reflex)
    pub theta_reflex: f32,
    /// Energy threshold for standard lane (E < theta_standard -> Standard)
    pub theta_standard: f32,
    /// Energy threshold for deep lane (E < theta_deep -> Deep)
    pub theta_deep: f32,
    /// Energy threshold for escalation (E >= theta_escalate -> Escalate)
    pub theta_escalate: f32,
    /// Whether to use average energy (true) or total energy (false)
    pub use_average_energy: bool,
    /// Minimum context size for routing (smaller contexts default to Standard)
    pub min_context_size: usize,
}

impl Default for TokenRouterConfig {
    fn default() -> Self {
        Self {
            theta_reflex: 0.01,
            theta_standard: 0.1,
            theta_deep: 1.0,
            theta_escalate: 10.0,
            use_average_energy: true,
            min_context_size: 4,
        }
    }
}

impl TokenRouterConfig {
    /// Create config with custom thresholds
    pub fn new(theta_reflex: f32, theta_standard: f32, theta_deep: f32) -> Self {
        Self {
            theta_reflex,
            theta_standard,
            theta_deep,
            theta_escalate: theta_deep * 10.0,
            ..Default::default()
        }
    }

    /// Builder: set reflex threshold
    pub fn with_theta_reflex(mut self, theta: f32) -> Self {
        self.theta_reflex = theta;
        self
    }

    /// Builder: set standard threshold
    pub fn with_theta_standard(mut self, theta: f32) -> Self {
        self.theta_standard = theta;
        self
    }

    /// Builder: set deep threshold
    pub fn with_theta_deep(mut self, theta: f32) -> Self {
        self.theta_deep = theta;
        self
    }

    /// Builder: set escalate threshold
    pub fn with_theta_escalate(mut self, theta: f32) -> Self {
        self.theta_escalate = theta;
        self
    }

    /// Builder: set energy computation method
    pub fn with_average_energy(mut self, use_avg: bool) -> Self {
        self.use_average_energy = use_avg;
        self
    }

    /// Builder: set minimum context size
    pub fn with_min_context_size(mut self, size: usize) -> Self {
        self.min_context_size = size;
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> AttentionResult<()> {
        if self.theta_reflex <= 0.0 {
            return Err(AttentionError::InvalidConfig(
                "theta_reflex must be positive".to_string(),
            ));
        }
        if self.theta_standard <= self.theta_reflex {
            return Err(AttentionError::InvalidConfig(
                "theta_standard must be greater than theta_reflex".to_string(),
            ));
        }
        if self.theta_deep <= self.theta_standard {
            return Err(AttentionError::InvalidConfig(
                "theta_deep must be greater than theta_standard".to_string(),
            ));
        }
        if self.theta_escalate <= self.theta_deep {
            return Err(AttentionError::InvalidConfig(
                "theta_escalate must be greater than theta_deep".to_string(),
            ));
        }
        Ok(())
    }
}

/// Routing decision for a token
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// Token index in sequence
    pub token_idx: usize,
    /// Computed energy for the token
    pub energy: f32,
    /// Assigned compute lane
    pub lane: ComputeLane,
    /// Confidence in the routing decision (0-1)
    pub confidence: f32,
    /// Optional sparse mask indices (for Standard lane)
    pub sparse_indices: Option<Vec<usize>>,
}

impl RoutingDecision {
    /// Create a new routing decision
    pub fn new(token_idx: usize, energy: f32, lane: ComputeLane) -> Self {
        // Confidence based on how clearly the energy falls into a lane
        let confidence = 1.0; // Can be refined based on energy distance to thresholds

        Self {
            token_idx,
            energy,
            lane,
            confidence,
            sparse_indices: None,
        }
    }

    /// Set sparse indices for this decision
    pub fn with_sparse_indices(mut self, indices: Vec<usize>) -> Self {
        self.sparse_indices = Some(indices);
        self
    }

    /// Check if this token needs attention
    pub fn needs_attention(&self) -> bool {
        !matches!(self.lane, ComputeLane::Escalate)
    }
}

/// Token router for coherence-gated transformer
pub struct TokenRouter {
    config: TokenRouterConfig,
}

impl TokenRouter {
    /// Create a new token router
    pub fn new(config: TokenRouterConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn default_router() -> Self {
        Self::new(TokenRouterConfig::default())
    }

    /// Get configuration
    pub fn config(&self) -> &TokenRouterConfig {
        &self.config
    }

    /// Get mutable configuration (for SONA tuning)
    pub fn config_mut(&mut self) -> &mut TokenRouterConfig {
        &mut self.config
    }

    /// Route a single token based on energy
    ///
    /// # Arguments
    ///
    /// * `energy` - Pre-computed energy for the token
    ///
    /// # Returns
    ///
    /// Compute lane for this token
    pub fn route_by_energy(&self, energy: f32) -> ComputeLane {
        if energy < self.config.theta_reflex {
            ComputeLane::Reflex
        } else if energy < self.config.theta_standard {
            ComputeLane::Standard
        } else if energy < self.config.theta_escalate {
            ComputeLane::Deep
        } else {
            ComputeLane::Escalate
        }
    }

    /// Route a single token using sheaf attention
    ///
    /// # Arguments
    ///
    /// * `token` - Token embedding
    /// * `context` - Context embeddings (keys)
    /// * `attention` - Sheaf attention layer for energy computation
    ///
    /// # Returns
    ///
    /// Routing decision for this token
    pub fn route_token(
        &self,
        token_idx: usize,
        token: &[f32],
        context: &[&[f32]],
        attention: &SheafAttention,
    ) -> AttentionResult<RoutingDecision> {
        // Handle small contexts
        if context.len() < self.config.min_context_size {
            return Ok(RoutingDecision::new(token_idx, 0.0, ComputeLane::Standard));
        }

        // Compute energy
        let energy = if self.config.use_average_energy {
            attention.average_token_energy(token, context)?
        } else {
            attention.token_energy(token, context)?
        };

        let lane = self.route_by_energy(energy);

        Ok(RoutingDecision::new(token_idx, energy, lane))
    }

    /// Route a batch of tokens
    ///
    /// # Arguments
    ///
    /// * `tokens` - Token embeddings
    /// * `context` - Shared context embeddings
    /// * `attention` - Sheaf attention layer
    ///
    /// # Returns
    ///
    /// Vector of routing decisions
    pub fn route_batch(
        &self,
        tokens: &[&[f32]],
        context: &[&[f32]],
        attention: &SheafAttention,
    ) -> AttentionResult<Vec<RoutingDecision>> {
        tokens
            .iter()
            .enumerate()
            .map(|(idx, token)| self.route_token(idx, token, context, attention))
            .collect()
    }

    /// Group tokens by their assigned lane
    ///
    /// Returns (reflex_indices, standard_indices, deep_indices, escalate_indices)
    pub fn group_by_lane(
        decisions: &[RoutingDecision],
    ) -> (Vec<usize>, Vec<usize>, Vec<usize>, Vec<usize>) {
        let mut reflex = Vec::new();
        let mut standard = Vec::new();
        let mut deep = Vec::new();
        let mut escalate = Vec::new();

        for decision in decisions {
            match decision.lane {
                ComputeLane::Reflex => reflex.push(decision.token_idx),
                ComputeLane::Standard => standard.push(decision.token_idx),
                ComputeLane::Deep => deep.push(decision.token_idx),
                ComputeLane::Escalate => escalate.push(decision.token_idx),
            }
        }

        (reflex, standard, deep, escalate)
    }

    /// Compute lane statistics for a batch of decisions
    pub fn lane_statistics(decisions: &[RoutingDecision]) -> LaneStatistics {
        let total = decisions.len();
        let (reflex, standard, deep, escalate) = Self::group_by_lane(decisions);

        let avg_energy = if total > 0 {
            decisions.iter().map(|d| d.energy).sum::<f32>() / total as f32
        } else {
            0.0
        };

        let max_energy = decisions.iter().map(|d| d.energy).fold(0.0f32, f32::max);

        let min_energy = decisions
            .iter()
            .map(|d| d.energy)
            .fold(f32::INFINITY, f32::min);

        LaneStatistics {
            total_tokens: total,
            reflex_count: reflex.len(),
            standard_count: standard.len(),
            deep_count: deep.len(),
            escalate_count: escalate.len(),
            average_energy: avg_energy,
            max_energy,
            min_energy: if min_energy.is_infinite() {
                0.0
            } else {
                min_energy
            },
        }
    }

    /// Estimate total latency for a batch based on routing
    pub fn estimate_latency_ms(decisions: &[RoutingDecision]) -> f32 {
        decisions.iter().map(|d| d.lane.typical_latency_ms()).sum()
    }

    /// Update thresholds based on desired lane distribution
    ///
    /// This can be used by SONA for adaptive tuning.
    pub fn tune_thresholds(
        &mut self,
        current_stats: &LaneStatistics,
        target_reflex_ratio: f32,
        target_standard_ratio: f32,
    ) {
        let total = current_stats.total_tokens as f32;
        if total == 0.0 {
            return;
        }

        let current_reflex_ratio = current_stats.reflex_count as f32 / total;
        let current_standard_ratio = current_stats.standard_count as f32 / total;

        // Adjust thresholds to move towards target ratios
        // More reflex needed -> increase theta_reflex
        // Less reflex needed -> decrease theta_reflex
        let reflex_adjustment = (target_reflex_ratio - current_reflex_ratio) * 0.1;
        let standard_adjustment = (target_standard_ratio - current_standard_ratio) * 0.1;

        // Apply adjustments while maintaining ordering
        self.config.theta_reflex = (self.config.theta_reflex * (1.0 + reflex_adjustment))
            .max(0.001)
            .min(self.config.theta_standard * 0.9);

        self.config.theta_standard = (self.config.theta_standard * (1.0 + standard_adjustment))
            .max(self.config.theta_reflex * 1.1)
            .min(self.config.theta_deep * 0.9);
    }
}

/// Statistics about lane distribution
#[derive(Debug, Clone)]
pub struct LaneStatistics {
    /// Total number of tokens routed
    pub total_tokens: usize,
    /// Tokens routed to Reflex lane
    pub reflex_count: usize,
    /// Tokens routed to Standard lane
    pub standard_count: usize,
    /// Tokens routed to Deep lane
    pub deep_count: usize,
    /// Tokens escalated
    pub escalate_count: usize,
    /// Average energy across all tokens
    pub average_energy: f32,
    /// Maximum energy
    pub max_energy: f32,
    /// Minimum energy
    pub min_energy: f32,
}

impl LaneStatistics {
    /// Get ratio of tokens in reflex lane
    pub fn reflex_ratio(&self) -> f32 {
        if self.total_tokens == 0 {
            0.0
        } else {
            self.reflex_count as f32 / self.total_tokens as f32
        }
    }

    /// Get ratio of tokens in standard lane
    pub fn standard_ratio(&self) -> f32 {
        if self.total_tokens == 0 {
            0.0
        } else {
            self.standard_count as f32 / self.total_tokens as f32
        }
    }

    /// Get ratio of tokens in deep lane
    pub fn deep_ratio(&self) -> f32 {
        if self.total_tokens == 0 {
            0.0
        } else {
            self.deep_count as f32 / self.total_tokens as f32
        }
    }

    /// Get ratio of escalated tokens
    pub fn escalate_ratio(&self) -> f32 {
        if self.total_tokens == 0 {
            0.0
        } else {
            self.escalate_count as f32 / self.total_tokens as f32
        }
    }

    /// Estimated speedup compared to all-deep processing
    pub fn estimated_speedup(&self) -> f32 {
        if self.total_tokens == 0 {
            1.0
        } else {
            let deep_latency = self.total_tokens as f32 * ComputeLane::Deep.typical_latency_ms();
            let actual_latency = self.reflex_count as f32
                * ComputeLane::Reflex.typical_latency_ms()
                + self.standard_count as f32 * ComputeLane::Standard.typical_latency_ms()
                + self.deep_count as f32 * ComputeLane::Deep.typical_latency_ms();

            if actual_latency > 0.0 {
                deep_latency / actual_latency
            } else {
                1.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheaf::SheafAttentionConfig;

    #[test]
    fn test_compute_lane_ordering() {
        assert!(ComputeLane::Reflex < ComputeLane::Standard);
        assert!(ComputeLane::Standard < ComputeLane::Deep);
        assert!(ComputeLane::Deep < ComputeLane::Escalate);
    }

    #[test]
    fn test_lane_properties() {
        assert_eq!(ComputeLane::Reflex.typical_layers(), 2);
        assert_eq!(ComputeLane::Standard.typical_layers(), 6);
        assert_eq!(ComputeLane::Deep.typical_layers(), 12);

        assert!(!ComputeLane::Reflex.requires_full_attention());
        assert!(!ComputeLane::Standard.requires_full_attention());
        assert!(ComputeLane::Deep.requires_full_attention());

        assert!(!ComputeLane::Reflex.uses_moe());
        assert!(ComputeLane::Deep.uses_moe());
    }

    #[test]
    fn test_config_default() {
        let config = TokenRouterConfig::default();
        assert!(config.theta_reflex < config.theta_standard);
        assert!(config.theta_standard < config.theta_deep);
        assert!(config.theta_deep < config.theta_escalate);
    }

    #[test]
    fn test_config_validation() {
        assert!(TokenRouterConfig::default().validate().is_ok());

        let bad_config = TokenRouterConfig {
            theta_reflex: 0.1,
            theta_standard: 0.05, // Less than reflex
            ..Default::default()
        };
        assert!(bad_config.validate().is_err());
    }

    #[test]
    fn test_route_by_energy() {
        let router = TokenRouter::default_router();

        assert_eq!(router.route_by_energy(0.001), ComputeLane::Reflex);
        assert_eq!(router.route_by_energy(0.05), ComputeLane::Standard);
        assert_eq!(router.route_by_energy(0.5), ComputeLane::Deep);
        assert_eq!(router.route_by_energy(100.0), ComputeLane::Escalate);
    }

    #[test]
    fn test_route_token() {
        let router = TokenRouter::default_router();
        let config = SheafAttentionConfig::new(8);
        let attention = SheafAttention::new(config);

        let token = vec![1.0; 8];
        let c1 = vec![1.0; 8];
        let c2 = vec![1.0; 8];
        let c3 = vec![1.0; 8];
        let c4 = vec![1.0; 8];
        let context: Vec<&[f32]> = vec![&c1, &c2, &c3, &c4];

        let decision = router.route_token(0, &token, &context, &attention).unwrap();
        assert_eq!(decision.token_idx, 0);
        assert!(decision.energy >= 0.0);
    }

    #[test]
    fn test_route_batch() {
        let router = TokenRouter::default_router();
        let config = SheafAttentionConfig::new(8);
        let attention = SheafAttention::new(config);

        let t1 = vec![1.0; 8];
        let t2 = vec![0.5; 8];
        let tokens: Vec<&[f32]> = vec![&t1, &t2];

        let c1 = vec![1.0; 8];
        let c2 = vec![1.0; 8];
        let c3 = vec![1.0; 8];
        let c4 = vec![1.0; 8];
        let context: Vec<&[f32]> = vec![&c1, &c2, &c3, &c4];

        let decisions = router.route_batch(&tokens, &context, &attention).unwrap();
        assert_eq!(decisions.len(), 2);
    }

    #[test]
    fn test_group_by_lane() {
        let decisions = vec![
            RoutingDecision::new(0, 0.001, ComputeLane::Reflex),
            RoutingDecision::new(1, 0.05, ComputeLane::Standard),
            RoutingDecision::new(2, 0.5, ComputeLane::Deep),
            RoutingDecision::new(3, 0.002, ComputeLane::Reflex),
        ];

        let (reflex, standard, deep, escalate) = TokenRouter::group_by_lane(&decisions);

        assert_eq!(reflex, vec![0, 3]);
        assert_eq!(standard, vec![1]);
        assert_eq!(deep, vec![2]);
        assert!(escalate.is_empty());
    }

    #[test]
    fn test_lane_statistics() {
        let decisions = vec![
            RoutingDecision::new(0, 0.001, ComputeLane::Reflex),
            RoutingDecision::new(1, 0.05, ComputeLane::Standard),
            RoutingDecision::new(2, 0.5, ComputeLane::Deep),
            RoutingDecision::new(3, 0.002, ComputeLane::Reflex),
        ];

        let stats = TokenRouter::lane_statistics(&decisions);

        assert_eq!(stats.total_tokens, 4);
        assert_eq!(stats.reflex_count, 2);
        assert_eq!(stats.standard_count, 1);
        assert_eq!(stats.deep_count, 1);
        assert_eq!(stats.escalate_count, 0);

        assert!((stats.reflex_ratio() - 0.5).abs() < 1e-6);
        assert!(stats.estimated_speedup() > 1.0);
    }

    #[test]
    fn test_routing_decision_builder() {
        let decision =
            RoutingDecision::new(0, 0.1, ComputeLane::Standard).with_sparse_indices(vec![1, 3, 5]);

        assert!(decision.sparse_indices.is_some());
        assert_eq!(decision.sparse_indices.unwrap(), vec![1, 3, 5]);
    }

    #[test]
    fn test_small_context_default() {
        let router = TokenRouter::default_router();
        let config = SheafAttentionConfig::new(8);
        let attention = SheafAttention::new(config);

        let token = vec![1.0; 8];
        let c1 = vec![1.0; 8];
        let context: Vec<&[f32]> = vec![&c1]; // Small context

        let decision = router.route_token(0, &token, &context, &attention).unwrap();
        assert_eq!(decision.lane, ComputeLane::Standard); // Default for small context
    }
}
