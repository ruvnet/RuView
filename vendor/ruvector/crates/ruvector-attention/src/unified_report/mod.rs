//! Unified Geometry Report
//!
//! Combines all geometric metrics into a single diagnostic surface.
//!
//! ## Metrics Included
//!
//! 1. **OT Distance**: Sliced Wasserstein mean absolute distance
//! 2. **Topology Coherence**: k-NN boundary mass ratio
//! 3. **H0 Persistence**: Sum of death times (structural complexity)
//! 4. **IB KL**: Information bottleneck compression term
//! 5. **Diffusion Energy**: Smoothness on key graph
//!
//! ## Use Cases
//!
//! - Routing decisions in MoE
//! - Gating signals for attention modes
//! - Monitoring attention health
//! - Debugging attention patterns

mod metrics;
mod report;

pub use metrics::{MetricType, MetricValue};
pub use report::{AttentionRecommendation, GeometryReport, ReportBuilder, ReportConfig};

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_exists() {
        assert!(true);
    }
}
