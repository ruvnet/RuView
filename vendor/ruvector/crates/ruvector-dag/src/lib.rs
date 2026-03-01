//! RuVector DAG - Directed Acyclic Graph structures for query plan optimization
//!
//! This crate provides efficient DAG data structures and algorithms for representing
//! and manipulating query execution plans with neural learning capabilities.
//!
//! ## Features
//!
//! - **DAG Data Structures**: Efficient directed acyclic graph representation for query plans
//! - **7 Attention Mechanisms**: Topological, Causal Cone, Critical Path, MinCut Gated, and more
//! - **SONA Learning**: Self-Optimizing Neural Architecture with MicroLoRA adaptation (non-WASM only)
//! - **MinCut Optimization**: Subpolynomial O(n^0.12) bottleneck detection
//! - **Self-Healing**: Autonomous anomaly detection and repair (non-WASM only)
//! - **QuDAG Integration**: Quantum-resistant distributed pattern learning (non-WASM only)
//!
//! ## Quick Start
//!
//! ```rust
//! use ruvector_dag::{QueryDag, OperatorNode, OperatorType};
//! use ruvector_dag::attention::{TopologicalAttention, DagAttention};
//!
//! // Build a query DAG
//! let mut dag = QueryDag::new();
//! let scan = dag.add_node(OperatorNode::seq_scan(0, "users"));
//! let filter = dag.add_node(OperatorNode::filter(1, "age > 18"));
//! dag.add_edge(scan, filter).unwrap();
//!
//! // Compute attention scores
//! let attention = TopologicalAttention::new(Default::default());
//! let scores = attention.forward(&dag).unwrap();
//! ```
//!
//! ## Modules
//!
//! - [`dag`] - Core DAG data structures and algorithms
//! - [`attention`] - Neural attention mechanisms for node importance
//! - [`sona`] - Self-Optimizing Neural Architecture with adaptive learning (requires `full` feature)
//! - [`mincut`] - Subpolynomial bottleneck detection and optimization
//! - [`healing`] - Self-healing system with anomaly detection (requires `full` feature)
//! - [`qudag`] - QuDAG network integration for distributed learning (requires `full` feature)

// Core modules (always available)
pub mod attention;
pub mod dag;
pub mod mincut;

// Modules requiring async runtime (non-WASM only)
#[cfg(feature = "full")]
pub mod healing;
#[cfg(feature = "full")]
pub mod qudag;
#[cfg(feature = "full")]
pub mod sona;

pub use dag::{
    BfsIterator, DagDeserializer, DagError, DagSerializer, DfsIterator, OperatorNode, OperatorType,
    QueryDag, TopologicalIterator,
};

pub use mincut::{
    Bottleneck, BottleneckAnalysis, DagMinCutEngine, FlowEdge, LocalKCut, MinCutConfig,
    MinCutResult, RedundancyStrategy, RedundancySuggestion,
};

pub use attention::{
    AttentionConfig, AttentionError, AttentionScores, CausalConeAttention, CausalConeConfig,
    CriticalPathAttention, CriticalPathConfig, DagAttention, FlowCapacity,
    MinCutConfig as AttentionMinCutConfig, MinCutGatedAttention, TopologicalAttention,
    TopologicalConfig,
};

#[cfg(feature = "full")]
pub use qudag::QuDagClient;

// Re-export crypto security functions for easy access (requires full feature)
#[cfg(feature = "full")]
pub use qudag::crypto::{
    check_crypto_security, is_production_ready, security_status, SecurityStatus,
};

#[cfg(feature = "full")]
pub use healing::{
    Anomaly, AnomalyConfig, AnomalyDetector, AnomalyType, DriftMetric, DriftTrend,
    HealingCycleResult, HealingOrchestrator, HealthStatus, IndexCheckResult, IndexHealth,
    IndexHealthChecker, IndexThresholds, IndexType, LearningDriftDetector, RepairResult,
    RepairStrategy,
};

#[cfg(feature = "full")]
pub use sona::{
    DagPattern, DagReasoningBank, DagSonaEngine, DagTrajectory, DagTrajectoryBuffer, EwcConfig,
    EwcPlusPlus, MicroLoRA, MicroLoRAConfig, ReasoningBankConfig,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_dag_creation() {
        let dag = QueryDag::new();
        assert_eq!(dag.node_count(), 0);
        assert_eq!(dag.edge_count(), 0);
    }
}
