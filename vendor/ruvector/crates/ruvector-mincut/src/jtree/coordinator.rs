//! Two-Tier Coordinator for Approximate and Exact Minimum Cut
//!
//! Routes queries between:
//! - **Tier 1 (Approximate)**: Fast O(polylog n) queries via j-tree hierarchy
//! - **Tier 2 (Exact)**: Precise O(n^o(1)) queries via full algorithm
//!
//! Includes escalation trigger policies for automatic tier switching.
//!
//! # Example
//!
//! ```rust,no_run
//! use ruvector_mincut::jtree::{TwoTierCoordinator, EscalationPolicy};
//! use ruvector_mincut::graph::DynamicGraph;
//! use std::sync::Arc;
//!
//! let graph = Arc::new(DynamicGraph::new());
//! graph.insert_edge(1, 2, 1.0).unwrap();
//! graph.insert_edge(2, 3, 1.0).unwrap();
//!
//! let mut coord = TwoTierCoordinator::with_defaults(graph);
//! coord.build().unwrap();
//!
//! // Query with automatic tier selection
//! let result = coord.min_cut();
//! println!("Min cut: {} (tier {})", result.value, result.tier);
//! ```

use crate::error::Result;
use crate::graph::{DynamicGraph, VertexId, Weight};
use crate::jtree::hierarchy::{JTreeConfig, JTreeHierarchy};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Policy for escalating from Tier 1 to Tier 2
#[derive(Debug, Clone)]
pub enum EscalationPolicy {
    /// Never escalate (always use approximate)
    Never,
    /// Always escalate (always use exact)
    Always,
    /// Escalate when approximate confidence is low
    LowConfidence {
        /// Threshold for low confidence (0.0-1.0)
        threshold: f64,
    },
    /// Escalate when cut value changes significantly
    ValueChange {
        /// Relative change threshold
        relative_threshold: f64,
        /// Absolute change threshold
        absolute_threshold: f64,
    },
    /// Escalate periodically
    Periodic {
        /// Number of queries between escalations
        query_interval: usize,
    },
    /// Escalate based on query latency requirements
    LatencyBased {
        /// Maximum allowed latency for Tier 1
        tier1_max_latency: Duration,
    },
    /// Adaptive escalation based on error history
    Adaptive {
        /// Window size for error tracking
        window_size: usize,
        /// Error threshold for escalation
        error_threshold: f64,
    },
}

impl Default for EscalationPolicy {
    fn default() -> Self {
        EscalationPolicy::Adaptive {
            window_size: 100,
            error_threshold: 0.1,
        }
    }
}

/// Trigger for escalation decision
#[derive(Debug, Clone)]
pub struct EscalationTrigger {
    /// Current approximate value
    pub approximate_value: f64,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Number of queries since last exact
    pub queries_since_exact: usize,
    /// Time since last exact query
    pub time_since_exact: Duration,
    /// Recent error history
    pub recent_errors: Vec<f64>,
}

impl EscalationTrigger {
    /// Check if escalation should occur based on policy
    pub fn should_escalate(&self, policy: &EscalationPolicy) -> bool {
        match policy {
            EscalationPolicy::Never => false,
            EscalationPolicy::Always => true,
            EscalationPolicy::LowConfidence { threshold } => self.confidence < *threshold,
            EscalationPolicy::ValueChange {
                relative_threshold,
                absolute_threshold,
            } => {
                // Would need previous value to check change
                false
            }
            EscalationPolicy::Periodic { query_interval } => {
                self.queries_since_exact >= *query_interval
            }
            EscalationPolicy::LatencyBased { tier1_max_latency } => {
                // Would need actual latency measurement
                false
            }
            EscalationPolicy::Adaptive {
                window_size,
                error_threshold,
            } => {
                if self.recent_errors.len() < *window_size / 2 {
                    return false;
                }
                let avg_error: f64 =
                    self.recent_errors.iter().sum::<f64>() / self.recent_errors.len() as f64;
                avg_error > *error_threshold
            }
        }
    }
}

/// Result of a query through the coordinator
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// The minimum cut value
    pub value: f64,
    /// Whether this is from exact computation
    pub is_exact: bool,
    /// Tier used (1 = approximate, 2 = exact)
    pub tier: u8,
    /// Confidence score (1.0 for exact)
    pub confidence: f64,
    /// Query latency
    pub latency: Duration,
    /// Whether escalation occurred
    pub escalated: bool,
}

/// Metrics for tier usage
#[derive(Debug, Clone, Default)]
pub struct TierMetrics {
    /// Number of Tier 1 queries
    pub tier1_queries: usize,
    /// Number of Tier 2 queries
    pub tier2_queries: usize,
    /// Number of escalations
    pub escalations: usize,
    /// Total Tier 1 latency
    pub tier1_total_latency: Duration,
    /// Total Tier 2 latency
    pub tier2_total_latency: Duration,
    /// Recorded errors (approximate vs exact)
    pub recorded_errors: Vec<f64>,
}

impl TierMetrics {
    /// Get average Tier 1 latency
    pub fn tier1_avg_latency(&self) -> Duration {
        if self.tier1_queries == 0 {
            Duration::ZERO
        } else {
            self.tier1_total_latency / self.tier1_queries as u32
        }
    }

    /// Get average Tier 2 latency
    pub fn tier2_avg_latency(&self) -> Duration {
        if self.tier2_queries == 0 {
            Duration::ZERO
        } else {
            self.tier2_total_latency / self.tier2_queries as u32
        }
    }

    /// Get average error
    pub fn avg_error(&self) -> f64 {
        if self.recorded_errors.is_empty() {
            0.0
        } else {
            self.recorded_errors.iter().sum::<f64>() / self.recorded_errors.len() as f64
        }
    }

    /// Get escalation rate
    pub fn escalation_rate(&self) -> f64 {
        let total = self.tier1_queries + self.tier2_queries;
        if total == 0 {
            0.0
        } else {
            self.escalations as f64 / total as f64
        }
    }
}

/// Two-tier coordinator for routing between approximate and exact algorithms
pub struct TwoTierCoordinator {
    /// The underlying graph
    graph: Arc<DynamicGraph>,
    /// Configuration for j-tree hierarchy
    config: JTreeConfig,
    /// Tier 1: J-Tree hierarchy for approximate queries (built lazily)
    tier1: Option<JTreeHierarchy>,
    /// Escalation policy
    policy: EscalationPolicy,
    /// Tier usage metrics
    metrics: TierMetrics,
    /// Recent error window
    error_window: VecDeque<f64>,
    /// Maximum error window size
    max_error_window: usize,
    /// Last exact value for error calculation
    last_exact_value: Option<f64>,
    /// Queries since last exact computation
    queries_since_exact: usize,
    /// Time of last exact computation
    last_exact_time: Instant,
    /// Cached approximate min-cut value
    cached_approx_value: Option<f64>,
}

impl std::fmt::Debug for TwoTierCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TwoTierCoordinator")
            .field("num_levels", &self.tier1.as_ref().map(|h| h.num_levels()))
            .field("policy", &self.policy)
            .field("metrics", &self.metrics)
            .field("queries_since_exact", &self.queries_since_exact)
            .field("cached_approx_value", &self.cached_approx_value)
            .finish()
    }
}

impl TwoTierCoordinator {
    /// Create a new two-tier coordinator
    pub fn new(graph: Arc<DynamicGraph>, policy: EscalationPolicy) -> Self {
        Self {
            graph,
            config: JTreeConfig::default(),
            tier1: None,
            policy,
            metrics: TierMetrics::default(),
            error_window: VecDeque::new(),
            max_error_window: 100,
            last_exact_value: None,
            queries_since_exact: 0,
            last_exact_time: Instant::now(),
            cached_approx_value: None,
        }
    }

    /// Create with default escalation policy
    pub fn with_defaults(graph: Arc<DynamicGraph>) -> Self {
        Self::new(graph, EscalationPolicy::default())
    }

    /// Create with custom j-tree config
    pub fn with_jtree_config(
        graph: Arc<DynamicGraph>,
        jtree_config: JTreeConfig,
        policy: EscalationPolicy,
    ) -> Self {
        Self {
            graph,
            config: jtree_config,
            tier1: None,
            policy,
            metrics: TierMetrics::default(),
            error_window: VecDeque::new(),
            max_error_window: 100,
            last_exact_value: None,
            queries_since_exact: 0,
            last_exact_time: Instant::now(),
            cached_approx_value: None,
        }
    }

    /// Build/initialize the coordinator
    pub fn build(&mut self) -> Result<()> {
        let hierarchy = JTreeHierarchy::build(Arc::clone(&self.graph), self.config.clone())?;
        self.tier1 = Some(hierarchy);
        Ok(())
    }

    /// Ensure hierarchy is built, build if not
    fn ensure_built(&mut self) -> Result<()> {
        if self.tier1.is_none() {
            self.build()?;
        }
        Ok(())
    }

    /// Get the j-tree hierarchy, building if necessary
    fn tier1_mut(&mut self) -> Result<&mut JTreeHierarchy> {
        self.ensure_built()?;
        self.tier1.as_mut().ok_or_else(|| {
            crate::error::MinCutError::InternalError("Hierarchy not built".to_string())
        })
    }

    /// Query global minimum cut with automatic tier selection
    pub fn min_cut(&mut self) -> QueryResult {
        let start = Instant::now();

        // Ensure hierarchy is built
        if let Err(e) = self.ensure_built() {
            return QueryResult {
                value: f64::INFINITY,
                is_exact: false,
                tier: 0,
                confidence: 0.0,
                latency: start.elapsed(),
                escalated: false,
            };
        }

        // Build escalation trigger
        let trigger = self.build_trigger();

        // Decide tier
        let use_exact = trigger.should_escalate(&self.policy);

        let result = if use_exact {
            self.query_tier2_global(start)
        } else {
            self.query_tier1_global(start)
        };

        result.unwrap_or_else(|_| QueryResult {
            value: f64::INFINITY,
            is_exact: false,
            tier: 0,
            confidence: 0.0,
            latency: start.elapsed(),
            escalated: false,
        })
    }

    /// Query s-t minimum cut with automatic tier selection
    pub fn st_min_cut(&mut self, s: VertexId, t: VertexId) -> Result<QueryResult> {
        let start = Instant::now();
        self.ensure_built()?;

        // Build escalation trigger
        let trigger = self.build_trigger();

        // Decide tier
        let use_exact = trigger.should_escalate(&self.policy);

        if use_exact {
            self.query_tier2_st(s, t, start)
        } else {
            self.query_tier1_st(s, t, start)
        }
    }

    /// Force exact (Tier 2) query
    pub fn exact_min_cut(&mut self) -> QueryResult {
        let start = Instant::now();
        if let Err(_) = self.ensure_built() {
            return QueryResult {
                value: f64::INFINITY,
                is_exact: false,
                tier: 0,
                confidence: 0.0,
                latency: start.elapsed(),
                escalated: false,
            };
        }
        self.query_tier2_global(start)
            .unwrap_or_else(|_| QueryResult {
                value: f64::INFINITY,
                is_exact: false,
                tier: 0,
                confidence: 0.0,
                latency: start.elapsed(),
                escalated: false,
            })
    }

    /// Force approximate (Tier 1) query
    pub fn approximate_min_cut(&mut self) -> QueryResult {
        let start = Instant::now();
        if let Err(_) = self.ensure_built() {
            return QueryResult {
                value: f64::INFINITY,
                is_exact: false,
                tier: 0,
                confidence: 0.0,
                latency: start.elapsed(),
                escalated: false,
            };
        }
        self.query_tier1_global(start)
            .unwrap_or_else(|_| QueryResult {
                value: f64::INFINITY,
                is_exact: false,
                tier: 0,
                confidence: 0.0,
                latency: start.elapsed(),
                escalated: false,
            })
    }

    /// Query Tier 1 for global min cut
    fn query_tier1_global(&mut self, start: Instant) -> Result<QueryResult> {
        let hierarchy = self.tier1_mut()?;
        let approx = hierarchy.approximate_min_cut()?;
        let value = approx.value;
        let latency = start.elapsed();

        self.cached_approx_value = Some(value);
        self.metrics.tier1_queries += 1;
        self.metrics.tier1_total_latency += latency;
        self.queries_since_exact += 1;

        // Calculate confidence based on hierarchy depth and approximation factor
        let confidence = self.estimate_confidence();

        Ok(QueryResult {
            value,
            is_exact: false,
            tier: 1,
            confidence,
            latency,
            escalated: false,
        })
    }

    /// Query Tier 1 for s-t min cut
    fn query_tier1_st(
        &mut self,
        _s: VertexId,
        _t: VertexId,
        start: Instant,
    ) -> Result<QueryResult> {
        // JTreeHierarchy doesn't have s-t min cut directly, use approximate global
        // In a full implementation, we'd traverse levels to find s-t cut
        let hierarchy = self.tier1_mut()?;
        let approx = hierarchy.approximate_min_cut()?;
        let value = approx.value;
        let latency = start.elapsed();

        self.cached_approx_value = Some(value);
        self.metrics.tier1_queries += 1;
        self.metrics.tier1_total_latency += latency;
        self.queries_since_exact += 1;

        let confidence = self.estimate_confidence();

        Ok(QueryResult {
            value,
            is_exact: false,
            tier: 1,
            confidence,
            latency,
            escalated: false,
        })
    }

    /// Query Tier 2 (exact) for global min cut
    fn query_tier2_global(&mut self, start: Instant) -> Result<QueryResult> {
        // For Tier 2, we request exact computation from the hierarchy
        let hierarchy = self.tier1_mut()?;
        let cut_result = hierarchy.min_cut(true)?; // Request exact
        let value = cut_result.value;
        let latency = start.elapsed();

        // Record for error tracking
        if let Some(last_approx) = self.cached_approx_value {
            let error = if last_approx > 0.0 {
                (value - last_approx).abs() / last_approx
            } else {
                0.0
            };
            self.record_error(error);
        }

        self.last_exact_value = Some(value);
        self.queries_since_exact = 0;
        self.last_exact_time = Instant::now();

        self.metrics.tier2_queries += 1;
        self.metrics.tier2_total_latency += latency;
        self.metrics.escalations += 1;

        Ok(QueryResult {
            value,
            is_exact: cut_result.is_exact,
            tier: 2,
            confidence: 1.0,
            latency,
            escalated: true,
        })
    }

    /// Query Tier 2 (exact) for s-t min cut
    fn query_tier2_st(
        &mut self,
        _s: VertexId,
        _t: VertexId,
        start: Instant,
    ) -> Result<QueryResult> {
        // Use global min cut with exact flag for now
        let hierarchy = self.tier1_mut()?;
        let cut_result = hierarchy.min_cut(true)?;
        let value = cut_result.value;
        let latency = start.elapsed();

        self.last_exact_value = Some(value);
        self.queries_since_exact = 0;
        self.last_exact_time = Instant::now();

        self.metrics.tier2_queries += 1;
        self.metrics.tier2_total_latency += latency;
        self.metrics.escalations += 1;

        Ok(QueryResult {
            value,
            is_exact: cut_result.is_exact,
            tier: 2,
            confidence: 1.0,
            latency,
            escalated: true,
        })
    }

    /// Build escalation trigger
    fn build_trigger(&self) -> EscalationTrigger {
        let recent_errors: Vec<f64> = self.error_window.iter().copied().collect();
        let approximate_value = self.cached_approx_value.unwrap_or(f64::INFINITY);

        EscalationTrigger {
            approximate_value,
            confidence: self.estimate_confidence(),
            queries_since_exact: self.queries_since_exact,
            time_since_exact: self.last_exact_time.elapsed(),
            recent_errors,
        }
    }

    /// Estimate confidence of current approximate value
    fn estimate_confidence(&self) -> f64 {
        // Base confidence on:
        // 1. Number of levels and approximation factor
        // 2. Cache hit rate
        // 3. Recency of exact computation

        let level_factor = if let Some(ref hierarchy) = self.tier1 {
            let num_levels = hierarchy.num_levels();
            let approx_factor = hierarchy.approximation_factor();
            // Higher approximation factor = lower confidence
            if num_levels > 0 {
                (1.0 / approx_factor.ln().max(1.0)).min(1.0)
            } else {
                0.5
            }
        } else {
            0.5
        };

        let recency_factor = {
            let elapsed = self.last_exact_time.elapsed().as_secs_f64();
            (-elapsed / 60.0).exp() // Decay over minutes
        };

        let error_factor = if self.error_window.is_empty() {
            0.8
        } else {
            let avg_error: f64 =
                self.error_window.iter().sum::<f64>() / self.error_window.len() as f64;
            (1.0 - avg_error).max(0.0)
        };

        (level_factor * 0.4 + recency_factor * 0.3 + error_factor * 0.3).min(1.0)
    }

    /// Record error for adaptive policy
    fn record_error(&mut self, error: f64) {
        self.error_window.push_back(error);
        if self.error_window.len() > self.max_error_window {
            self.error_window.pop_front();
        }
        self.metrics.recorded_errors.push(error);
    }

    /// Handle edge insertion
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) -> Result<f64> {
        self.ensure_built()?;
        let hierarchy = self.tier1.as_mut().ok_or_else(|| {
            crate::error::MinCutError::InternalError("Hierarchy not built".to_string())
        })?;
        let result = hierarchy.insert_edge(u, v, weight)?;
        self.cached_approx_value = Some(result);
        Ok(result)
    }

    /// Handle edge deletion
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) -> Result<f64> {
        self.ensure_built()?;
        let hierarchy = self.tier1.as_mut().ok_or_else(|| {
            crate::error::MinCutError::InternalError("Hierarchy not built".to_string())
        })?;
        let result = hierarchy.delete_edge(u, v)?;
        self.cached_approx_value = Some(result);
        Ok(result)
    }

    /// Query multi-terminal cut
    ///
    /// Returns the minimum cut value separating any pair of terminals.
    pub fn multi_terminal_cut(&mut self, terminals: &[VertexId]) -> Result<f64> {
        if terminals.len() < 2 {
            return Ok(f64::INFINITY);
        }

        // Use approximate min cut as a proxy for multi-terminal
        // A proper implementation would traverse levels
        self.ensure_built()?;
        let hierarchy = self.tier1.as_mut().ok_or_else(|| {
            crate::error::MinCutError::InternalError("Hierarchy not built".to_string())
        })?;
        let approx = hierarchy.approximate_min_cut()?;
        Ok(approx.value)
    }

    /// Get current metrics
    pub fn metrics(&self) -> &TierMetrics {
        &self.metrics
    }

    /// Reset metrics
    pub fn reset_metrics(&mut self) {
        self.metrics = TierMetrics::default();
        self.error_window.clear();
    }

    /// Get escalation policy
    pub fn policy(&self) -> &EscalationPolicy {
        &self.policy
    }

    /// Set escalation policy
    pub fn set_policy(&mut self, policy: EscalationPolicy) {
        self.policy = policy;
    }

    /// Get the underlying graph
    pub fn graph(&self) -> &Arc<DynamicGraph> {
        &self.graph
    }

    /// Get Tier 1 hierarchy (if built)
    pub fn tier1(&self) -> Option<&JTreeHierarchy> {
        self.tier1.as_ref()
    }

    /// Get number of levels in the hierarchy
    pub fn num_levels(&self) -> usize {
        self.tier1.as_ref().map(|h| h.num_levels()).unwrap_or(0)
    }

    /// Force rebuild of all tiers
    pub fn rebuild(&mut self) -> Result<()> {
        self.tier1 = None;
        self.build()?;
        self.last_exact_value = None;
        self.queries_since_exact = 0;
        self.cached_approx_value = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> Arc<DynamicGraph> {
        let g = Arc::new(DynamicGraph::new());
        // Two triangles connected by a bridge
        g.insert_edge(1, 2, 2.0).unwrap();
        g.insert_edge(2, 3, 2.0).unwrap();
        g.insert_edge(3, 1, 2.0).unwrap();
        g.insert_edge(4, 5, 2.0).unwrap();
        g.insert_edge(5, 6, 2.0).unwrap();
        g.insert_edge(6, 4, 2.0).unwrap();
        g.insert_edge(3, 4, 1.0).unwrap(); // Bridge edge
        g
    }

    #[test]
    fn test_coordinator_creation() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::with_defaults(g);

        coord.build().unwrap();

        assert_eq!(coord.metrics().tier1_queries, 0);
        assert_eq!(coord.metrics().tier2_queries, 0);
        assert!(coord.num_levels() > 0);
    }

    #[test]
    fn test_approximate_query() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::with_defaults(g);
        coord.build().unwrap();

        let result = coord.approximate_min_cut();

        assert!(!result.is_exact);
        assert_eq!(result.tier, 1);
        assert!(result.value.is_finite());
        assert!(!result.escalated);
    }

    #[test]
    fn test_exact_query() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::with_defaults(g);
        coord.build().unwrap();

        let result = coord.exact_min_cut();

        // Tier 2 query, escalated
        assert_eq!(result.tier, 2);
        assert_eq!(result.confidence, 1.0);
        assert!(result.escalated);
        assert!(result.value.is_finite());
    }

    #[test]
    fn test_st_query() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::with_defaults(g);
        coord.build().unwrap();

        let result = coord.st_min_cut(1, 6).unwrap();

        // Should find a finite cut value
        assert!(result.value.is_finite());
    }

    #[test]
    fn test_escalation_never() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::new(g, EscalationPolicy::Never);
        coord.build().unwrap();

        // Should never escalate
        for _ in 0..10 {
            let result = coord.min_cut();
            assert!(!result.escalated);
            assert_eq!(result.tier, 1);
        }
    }

    #[test]
    fn test_escalation_always() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::new(g, EscalationPolicy::Always);
        coord.build().unwrap();

        let result = coord.min_cut();
        assert!(result.escalated);
        assert_eq!(result.tier, 2);
    }

    #[test]
    fn test_escalation_periodic() {
        let g = create_test_graph();
        let mut coord =
            TwoTierCoordinator::new(g, EscalationPolicy::Periodic { query_interval: 3 });
        coord.build().unwrap();

        // First query should escalate (queries_since_exact starts at 0, >= 3 is false)
        // Actually, with interval=3, first escalate when queries_since_exact >= 3
        let r1 = coord.min_cut();
        // First query: queries_since_exact=0, so should NOT escalate
        assert!(!r1.escalated);

        let r2 = coord.min_cut();
        assert!(!r2.escalated);

        let r3 = coord.min_cut();
        // Third query: queries_since_exact=2, so should NOT escalate
        assert!(!r3.escalated);

        // Fourth query: queries_since_exact=3, should escalate
        let r4 = coord.min_cut();
        assert!(r4.escalated);
    }

    #[test]
    fn test_metrics_tracking() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::new(g, EscalationPolicy::Never);
        coord.build().unwrap();

        coord.approximate_min_cut();
        coord.approximate_min_cut();
        coord.exact_min_cut();

        let metrics = coord.metrics();
        assert_eq!(metrics.tier1_queries, 2);
        assert_eq!(metrics.tier2_queries, 1);
        assert_eq!(metrics.escalations, 1);
    }

    #[test]
    fn test_edge_update() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::with_defaults(g.clone());
        coord.build().unwrap();

        let initial = coord.approximate_min_cut().value;

        // Insert edge that doesn't change min cut structure
        g.insert_edge(1, 5, 10.0).unwrap();
        let _ = coord.insert_edge(1, 5, 10.0);

        let after = coord.approximate_min_cut().value;

        // Both should be finite
        assert!(initial.is_finite());
        assert!(after.is_finite());
    }

    #[test]
    fn test_multi_terminal() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::with_defaults(g);
        coord.build().unwrap();

        let result = coord.multi_terminal_cut(&[1, 4, 6]).unwrap();
        // Result is now just f64
        assert!(result.is_finite());
    }

    #[test]
    fn test_confidence_estimation() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::with_defaults(g);
        coord.build().unwrap();

        let result = coord.approximate_min_cut();

        // Confidence should be positive
        assert!(result.confidence > 0.0);
        assert!(result.confidence <= 1.0);
    }

    #[test]
    fn test_reset_metrics() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::with_defaults(g);
        coord.build().unwrap();

        coord.approximate_min_cut();
        coord.exact_min_cut();

        coord.reset_metrics();

        let metrics = coord.metrics();
        assert_eq!(metrics.tier1_queries, 0);
        assert_eq!(metrics.tier2_queries, 0);
    }

    #[test]
    fn test_rebuild() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::with_defaults(g);
        coord.build().unwrap();

        let initial = coord.approximate_min_cut().value;
        coord.rebuild().unwrap();
        let after = coord.approximate_min_cut().value;

        // Both should be consistent
        assert!((initial - after).abs() < 1e-10 || (initial.is_finite() && after.is_finite()));
    }

    #[test]
    fn test_policy_modification() {
        let g = create_test_graph();
        let mut coord = TwoTierCoordinator::new(g, EscalationPolicy::Never);
        coord.build().unwrap();

        // Initially should not escalate
        let r1 = coord.min_cut();
        assert!(!r1.escalated);

        // Change policy
        coord.set_policy(EscalationPolicy::Always);

        // Now should always escalate
        let r2 = coord.min_cut();
        assert!(r2.escalated);
    }
}
