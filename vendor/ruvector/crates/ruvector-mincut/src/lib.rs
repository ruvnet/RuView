//! # RuVector MinCut
//!
//! Subpolynomial-time dynamic minimum cut algorithm with real-time monitoring.
//!
//! This crate provides efficient algorithms for maintaining minimum cuts in
//! dynamic graphs with edge insertions and deletions.
//!
//! ## Features
//!
//! - **Exact Algorithm**: O(n^{o(1)}) amortized update time for cuts up to 2^{O((log n)^{3/4})}
//! - **Approximate Algorithm**: (1+ε)-approximate cuts via graph sparsification
//! - **Real-Time Monitoring**: Event-driven notifications with configurable thresholds
//! - **Thread-Safe**: Concurrent reads with exclusive writes
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ruvector_mincut::{MinCutBuilder, DynamicMinCut};
//!
//! // Create a dynamic minimum cut structure
//! let mut mincut = MinCutBuilder::new()
//!     .exact()
//!     .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)])
//!     .build()
//!     .expect("Failed to build");
//!
//! // Query the minimum cut
//! println!("Min cut: {}", mincut.min_cut_value());
//!
//! // Insert a new edge
//! mincut.insert_edge(3, 4, 1.0).expect("Insert failed");
//!
//! // Delete an existing edge
//! let _ = mincut.delete_edge(2, 3);
//! ```
//!
//! ## Architecture
//!
//! The crate is organized into several modules:
//!
//! - [`graph`]: Dynamic graph representation with efficient operations
//! - [`algorithm`]: Core minimum cut algorithms (exact and approximate)
//! - [`tree`]: Hierarchical decomposition for subpolynomial updates
//! - [`linkcut`]: Link-cut trees for dynamic connectivity
//! - [`euler`]: Euler tour trees for tree operations
//! - [`sparsify`]: Graph sparsification for approximate cuts
//! - [`expander`]: Expander decomposition for subpolynomial updates
//! - `monitoring`: Real-time event monitoring (feature-gated)
//!
//! ## Feature Flags
//!
//! - `exact` - Exact minimum cut algorithm (enabled by default)
//! - `approximate` - (1+ε)-approximate algorithm (enabled by default)
//! - `monitoring` - Real-time monitoring with callbacks (optional)
//! - `integration` - GraphDB integration (optional)
//! - `simd` - SIMD optimizations (optional)
//!
//! ## Examples
//!
//! ### Basic Usage
//!
//! ```rust
//! use ruvector_mincut::prelude::*;
//!
//! let mut mincut = MinCutBuilder::new()
//!     .with_edges(vec![
//!         (1, 2, 1.0),
//!         (2, 3, 1.0),
//!         (3, 1, 1.0),
//!     ])
//!     .build()
//!     .unwrap();
//!
//! assert_eq!(mincut.min_cut_value(), 2.0);
//! ```
//!
//! ### Approximate Algorithm
//!
//! ```rust
//! use ruvector_mincut::prelude::*;
//!
//! let mincut = MinCutBuilder::new()
//!     .approximate(0.1) // 10% approximation
//!     .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0)])
//!     .build()
//!     .unwrap();
//!
//! let result = mincut.min_cut();
//! assert!(!result.is_exact);
//! assert_eq!(result.approximation_ratio, 1.1);
//! ```
//!
//! ### Real-Time Monitoring
//!
//! ```rust,ignore
//! #[cfg(feature = "monitoring")]
//! use ruvector_mincut::{MinCutBuilder, MonitorBuilder, EventType};
//!
//! let mut mincut = MinCutBuilder::new()
//!     .with_edges(vec![(1, 2, 1.0)])
//!     .build()
//!     .unwrap();
//!
//! let monitor = MonitorBuilder::new()
//!     .threshold("low", 0.5, true)
//!     .on_event(|event| {
//!         println!("Cut changed: {:?}", event.event_type);
//!     })
//!     .build();
//!
//! // Monitor will fire callbacks on updates
//! mincut.insert_edge(2, 3, 1.0).unwrap();
//! ```

#![deny(missing_docs)]
#![cfg_attr(not(feature = "wasm"), deny(unsafe_code))]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

// Core modules
pub mod algorithm;
pub mod certificate;
pub mod cluster;
pub mod compact;
pub mod connectivity;
pub mod error;
pub mod euler;
pub mod expander;
pub mod fragment;
pub mod fragmentation;
pub mod graph;
pub mod instance;
pub mod integration;
pub mod linkcut;
pub mod localkcut;
pub mod parallel;
pub mod pool;
pub mod sparsify;
pub mod tree;
pub mod witness;
pub mod wrapper;

/// Performance optimizations for j-Tree + BMSSP implementation.
///
/// Provides SOTA optimizations achieving 10x combined speedup.
pub mod optimization;

/// Spiking Neural Network integration for deep MinCut optimization.
///
/// This module implements a six-layer integration architecture combining
/// neuromorphic computing with subpolynomial graph algorithms:
///
/// 1. **Temporal Attractors**: Energy landscapes for graph optimization
/// 2. **Strange Loop**: Self-modifying meta-cognitive protocols
/// 3. **Causal Discovery**: Spike-timing based inference
/// 4. **Time Crystal CPG**: Central pattern generators for coordination
/// 5. **Morphogenetic Networks**: Bio-inspired self-organizing growth
/// 6. **Neural Optimizer**: Reinforcement learning on graph structures
///
/// ## Quick Start
///
/// ```rust,no_run
/// use ruvector_mincut::snn::{CognitiveMinCutEngine, EngineConfig, OperationMode};
/// use ruvector_mincut::graph::DynamicGraph;
///
/// // Create a graph
/// let graph = DynamicGraph::new();
/// graph.insert_edge(0, 1, 1.0).unwrap();
/// graph.insert_edge(1, 2, 1.0).unwrap();
///
/// // Create the cognitive engine
/// let config = EngineConfig::default();
/// let mut engine = CognitiveMinCutEngine::new(graph, config);
///
/// // Run optimization
/// engine.set_mode(OperationMode::Optimize);
/// let spikes = engine.run(100);
/// ```
pub mod snn;

/// Subpolynomial-time dynamic minimum cut algorithm.
///
/// This module implements the December 2024 breakthrough achieving n^{o(1)} update time.
/// Integrates multi-level hierarchy, deterministic LocalKCut, and fragmenting algorithm.
pub mod subpolynomial;

/// Dynamic Hierarchical j-Tree Decomposition for Approximate Cut Structure
///
/// This module implements the two-tier dynamic cut architecture from ADR-002:
///
/// - **Tier 1 (j-Tree)**: O(n^ε) amortized updates, poly-log approximation
/// - **Tier 2 (Exact)**: SubpolynomialMinCut for exact verification
///
/// Key features:
/// - BMSSP WASM integration for O(m·log^(2/3) n) path-cut duality queries
/// - Vertex-split-tolerant cut sparsifier with O(log² n / ε²) recourse
/// - Lazy hierarchical evaluation (demand-paging)
///
/// ## Example
///
/// ```rust,no_run
/// use ruvector_mincut::jtree::{JTreeHierarchy, JTreeConfig};
/// use ruvector_mincut::graph::DynamicGraph;
/// use std::sync::Arc;
///
/// let graph = Arc::new(DynamicGraph::new());
/// graph.insert_edge(1, 2, 1.0).unwrap();
/// graph.insert_edge(2, 3, 1.0).unwrap();
///
/// let mut jtree = JTreeHierarchy::build(graph, JTreeConfig::default()).unwrap();
/// let approx = jtree.approximate_min_cut().unwrap();
/// ```
#[cfg(feature = "jtree")]
pub mod jtree;

// Internal modules
mod core;

// Optional feature-gated modules
#[cfg(feature = "monitoring")]
pub mod monitoring;

#[cfg(feature = "canonical")]
pub mod canonical;

#[cfg(feature = "wasm")]
pub mod wasm;

// Re-exports for convenient access
pub use algorithm::approximate::{
    ApproxMinCut, ApproxMinCutConfig, ApproxMinCutResult, ApproxMinCutStats,
};
pub use algorithm::{AlgorithmStats, DynamicMinCut, MinCutBuilder, MinCutConfig, MinCutResult};
pub use certificate::{
    AuditData, AuditEntry, AuditEntryType, AuditLogger, CertLocalKCutQuery, CertificateError,
    CutCertificate, LocalKCutResponse, LocalKCutResultSummary, UpdateTrigger, UpdateType,
};
pub use cluster::hierarchy::{
    Expander, HierarchyCluster, HierarchyConfig, HierarchyStats, MirrorCut, Precluster,
    ThreeLevelHierarchy,
};
pub use cluster::{Cluster, ClusterHierarchy};
pub use compact::{
    BitSet256, CompactAdjacency, CompactCoreState, CompactEdge, CompactEdgeId, CompactVertexId,
    CompactWitness, CoreResult, MAX_EDGES_PER_CORE, MAX_VERTICES_PER_CORE,
};
pub use connectivity::polylog::{PolylogConnectivity, PolylogStats};
pub use connectivity::DynamicConnectivity;
pub use error::{MinCutError, Result};
pub use euler::EulerTourTree;
pub use expander::{Conductance, ExpanderComponent, ExpanderDecomposition};
pub use fragment::{Fragment, FragmentResult, FragmentingAlgorithm};
pub use fragmentation::{
    Fragment as FragmentationFragment, Fragmentation, FragmentationConfig, TrimResult,
};
pub use graph::{DynamicGraph, Edge, EdgeId, GraphStats, VertexId, Weight};
pub use instance::{
    BoundedInstance, InstanceResult, ProperCutInstance, StubInstance, WitnessHandle,
};
pub use integration::{CommunityDetector, GraphPartitioner, RuVectorGraphAnalyzer};
pub use linkcut::LinkCutTree;
pub use localkcut::{
    ColorMask, DeterministicFamilyGenerator, DeterministicLocalKCut, EdgeColor, ForestPacking,
    LocalCutResult, LocalKCut, LocalKCutOracle, LocalKCutQuery,
    LocalKCutResult as PaperLocalKCutResult,
};
pub use parallel::{
    compute_core_range, CoreDistributor, CoreExecutor, CoreMessage, CoreStrategy, ResultAggregator,
    SharedCoordinator, WorkItem, NUM_CORES, RANGES_PER_CORE, RANGE_FACTOR, TOTAL_RANGES,
};
pub use sparsify::{SparseGraph, SparsifyConfig};
pub use subpolynomial::{
    HierarchyLevel, HierarchyStatistics, LevelExpander, MinCutQueryResult, RecourseStats,
    SubpolyConfig, SubpolynomialMinCut,
};
pub use tree::{DecompositionNode, HierarchicalDecomposition, LevelInfo};
pub use witness::{EdgeWitness, LazyWitnessTree, WitnessTree};
pub use wrapper::MinCutWrapper;

// Optimization re-exports (SOTA j-Tree + BMSSP performance improvements)
pub use optimization::{
    BatchConfig,
    BenchmarkResult,
    // Benchmarking
    BenchmarkSuite,
    CacheConfig,
    CacheStats,
    // DSpar: 5.9x speedup via degree-based presparse
    DegreePresparse,
    DistanceArray,
    LazyLevel,
    // Pool: 50-75% memory reduction
    LevelPool,
    OptimizationBenchmark,
    ParallelConfig,
    // Parallel: Rayon-based work-stealing
    ParallelLevelUpdater,
    // Cache: 10x for repeated distance queries
    PathDistanceCache,
    PoolConfig,
    PoolStats,
    PrefetchHint,
    PresparseConfig,
    PresparseResult,
    PresparseStats,
    // SIMD: 2-4x for distance operations
    SimdDistanceOps,
    TypedArrayTransfer,
    // WASM Batch: 10x FFI overhead reduction
    WasmBatchOps,
    WorkStealingScheduler,
};

// J-Tree re-exports (feature-gated)
#[cfg(feature = "jtree")]
pub use jtree::{
    ApproximateCut, BmsspJTreeLevel, ContractedGraph, CutResult as JTreeCutResult,
    DynamicCutSparsifier, EscalationPolicy, EscalationTrigger, JTreeConfig, JTreeError,
    JTreeHierarchy, JTreeLevel, JTreeStatistics, LevelConfig, LevelStatistics, PathCutResult,
    QueryResult as CoordinatorQueryResult, RecourseTracker, SparsifierConfig, SparsifierStatistics,
    Tier, TierMetrics, TwoTierCoordinator, VertexSplitResult,
};

// Re-export ForestPacking with explicit disambiguation (also defined in localkcut)
#[cfg(feature = "jtree")]
pub use jtree::ForestPacking as JTreeForestPacking;

// SNN Integration re-exports
pub use snn::{
    AttractorConfig,
    // Layer 1: Attractors
    AttractorDynamics,
    CPGConfig,
    CausalConfig,
    // Layer 3: Causal Discovery
    CausalDiscoverySNN,
    CausalGraph,
    CausalRelation,
    // Unified Engine
    CognitiveMinCutEngine,
    EnergyLandscape,
    EngineConfig,
    EngineMetrics,
    GrowthRules,
    // Core SNN types
    LIFNeuron,
    LayerConfig,
    MetaAction,
    // Layer 2: Strange Loop
    MetaCognitiveMinCut,
    MetaLevel,
    MorphConfig,
    // Layer 5: Morphogenetic
    MorphogeneticSNN,
    NetworkConfig,
    // Layer 6: Neural Optimizer
    NeuralGraphOptimizer,
    NeuronConfig,
    NeuronState,
    OptimizationResult,
    OptimizerConfig,
    OscillatorNeuron,
    PhaseTopology,
    PolicySNN,
    SNNMinCutConfig,
    STDPConfig,
    SimTime,
    // Utilities
    Spike,
    SpikeTrain,
    SpikingNetwork,
    StrangeLoopConfig,
    Synapse,
    SynapseMatrix,
    // Layer 4: Time Crystal
    TimeCrystalCPG,
    TuringPattern,
    ValueNetwork,
};

#[cfg(feature = "agentic")]
pub use integration::AgenticAnalyzer;

#[cfg(feature = "canonical")]
pub use canonical::{
    CactusCycle, CactusEdge, CactusGraph, CactusVertex, CanonicalCutResult, CanonicalMinCut,
    CanonicalMinCutImpl, FixedWeight, WitnessReceipt,
};

#[cfg(feature = "monitoring")]
pub use monitoring::{
    EventType, MinCutEvent, MinCutMonitor, MonitorBuilder, MonitorConfig, MonitorMetrics, Threshold,
};

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Prelude module for convenient imports
///
/// Import this module to get all the commonly used types and traits:
///
/// ```rust
/// use ruvector_mincut::prelude::*;
///
/// let mincut = MinCutBuilder::new()
///     .exact()
///     .build()
///     .unwrap();
/// ```
pub mod prelude {
    //! Prelude module with commonly used types

    pub use crate::{
        compute_core_range,
        AlgorithmStats,
        ApproxMinCut,
        ApproxMinCutConfig,
        AttractorConfig,
        AttractorDynamics,
        AuditLogger,
        BitSet256,
        BoundedInstance,
        CPGConfig,
        CertificateError,
        Cluster,
        ClusterHierarchy,
        // SNN Integration types
        CognitiveMinCutEngine,
        ColorMask,
        CommunityDetector,
        CompactAdjacency,
        CompactCoreState,
        CompactEdge,
        CompactEdgeId,
        CompactVertexId,
        CompactWitness,
        Conductance,
        CoreDistributor,
        CoreExecutor,
        CoreResult,
        CoreStrategy,
        CutCertificate,
        DeterministicFamilyGenerator,
        DeterministicLocalKCut,
        DynamicConnectivity,
        DynamicGraph,
        DynamicMinCut,
        Edge,
        EdgeColor,
        EdgeId,
        EngineConfig,
        EngineMetrics,
        ExpanderComponent,
        ExpanderDecomposition,
        ForestPacking,
        Fragment,
        FragmentResult,
        FragmentingAlgorithm,
        GraphPartitioner,
        InstanceResult,
        LocalCutResult,
        LocalKCut,
        LocalKCutOracle,
        LocalKCutQuery,
        MinCutBuilder,
        MinCutConfig,
        MinCutError,
        MinCutResult,
        MinCutWrapper,
        NeuralGraphOptimizer,
        OptimizerConfig,
        PaperLocalKCutResult,
        PolylogConnectivity,
        PolylogStats,
        ProperCutInstance,
        RecourseStats,
        Result,
        ResultAggregator,
        RuVectorGraphAnalyzer,
        SharedCoordinator,
        SimTime,
        Spike,
        StubInstance,
        SubpolyConfig,
        // Subpolynomial min-cut
        SubpolynomialMinCut,
        TimeCrystalCPG,
        VertexId,
        Weight,
        WitnessHandle,
        MAX_EDGES_PER_CORE,
        MAX_VERTICES_PER_CORE,
        NUM_CORES,
        RANGES_PER_CORE,
    };

    #[cfg(feature = "agentic")]
    pub use crate::AgenticAnalyzer;

    #[cfg(feature = "monitoring")]
    pub use crate::{EventType, MinCutEvent, MinCutMonitor, MonitorBuilder};

    #[cfg(feature = "canonical")]
    pub use crate::{
        CactusCycle, CactusEdge, CactusGraph, CactusVertex, CanonicalCutResult, CanonicalMinCut,
        CanonicalMinCutImpl, FixedWeight, WitnessReceipt,
    };

    #[cfg(feature = "jtree")]
    pub use crate::{
        ApproximateCut, BmsspJTreeLevel, ContractedGraph, CoordinatorQueryResult,
        DynamicCutSparsifier, EscalationPolicy, EscalationTrigger, JTreeConfig, JTreeCutResult,
        JTreeError, JTreeForestPacking, JTreeHierarchy, JTreeLevel, JTreeStatistics, LevelConfig,
        LevelStatistics, PathCutResult, RecourseTracker, SparsifierConfig, SparsifierStatistics,
        Tier, TierMetrics, TwoTierCoordinator, VertexSplitResult,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constant() {
        assert!(!VERSION.is_empty());
        assert!(!NAME.is_empty());
        assert_eq!(NAME, "ruvector-mincut");
    }

    #[test]
    fn test_basic_workflow() {
        // Test the main API works correctly
        let mut mincut = MinCutBuilder::new()
            .exact()
            .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)])
            .build()
            .unwrap();

        // Verify initial state
        assert_eq!(mincut.num_vertices(), 3);
        assert_eq!(mincut.num_edges(), 3);
        assert_eq!(mincut.min_cut_value(), 2.0);

        // Test insert
        let new_cut = mincut.insert_edge(1, 4, 2.0).unwrap();
        assert!(new_cut.is_finite());
        assert_eq!(mincut.num_edges(), 4);

        // Test delete
        let new_cut = mincut.delete_edge(1, 2).unwrap();
        assert!(new_cut.is_finite());
        assert_eq!(mincut.num_edges(), 3);
    }

    #[test]
    fn test_prelude_imports() {
        use crate::prelude::*;

        // Ensure all prelude items are accessible
        let mincut = MinCutBuilder::new().build().unwrap();

        assert_eq!(mincut.min_cut_value(), f64::INFINITY);
    }

    #[test]
    fn test_approximate_mode() {
        let mincut = MinCutBuilder::new()
            .approximate(0.1)
            .with_edges(vec![(1, 2, 1.0)])
            .build()
            .unwrap();

        let result = mincut.min_cut();
        assert!(!result.is_exact);
        assert_eq!(result.approximation_ratio, 1.1);
    }

    #[test]
    fn test_exact_mode() {
        let mincut = MinCutBuilder::new()
            .exact()
            .with_edges(vec![(1, 2, 1.0)])
            .build()
            .unwrap();

        let result = mincut.min_cut();
        assert!(result.is_exact);
        assert_eq!(result.approximation_ratio, 1.0);
    }

    #[test]
    fn test_min_cut_result() {
        let mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)])
            .build()
            .unwrap();

        let result = mincut.min_cut();
        assert_eq!(result.value, 2.0);
        assert!(result.cut_edges.is_some());
        assert!(result.partition.is_some());

        if let Some((s, t)) = result.partition {
            assert_eq!(s.len() + t.len(), 3);
            assert!(!s.is_empty());
            assert!(!t.is_empty());
        }
    }

    #[test]
    fn test_graph_stats() {
        let mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 2.0), (2, 3, 3.0), (3, 1, 1.0)])
            .build()
            .unwrap();

        let graph = mincut.graph();
        let stats = graph.read().stats();

        assert_eq!(stats.num_vertices, 3);
        assert_eq!(stats.num_edges, 3);
        assert_eq!(stats.total_weight, 6.0);
        assert_eq!(stats.min_degree, 2);
        assert_eq!(stats.max_degree, 2);
    }

    #[test]
    fn test_algorithm_stats() {
        let mut mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 1.0)])
            .build()
            .unwrap();

        mincut.insert_edge(2, 3, 1.0).unwrap();
        mincut.delete_edge(1, 2).unwrap();
        let _ = mincut.min_cut_value();

        let stats = mincut.stats();
        assert_eq!(stats.insertions, 1);
        assert_eq!(stats.deletions, 1);
        assert_eq!(stats.queries, 1);
        assert!(stats.avg_update_time_us > 0.0);
    }

    #[test]
    fn test_dynamic_updates() {
        let mut mincut = MinCutBuilder::new().build().unwrap();

        // Start empty
        assert_eq!(mincut.min_cut_value(), f64::INFINITY);

        // Add edges dynamically
        mincut.insert_edge(1, 2, 1.0).unwrap();
        assert_eq!(mincut.min_cut_value(), 1.0);

        mincut.insert_edge(2, 3, 1.0).unwrap();
        assert_eq!(mincut.min_cut_value(), 1.0);

        mincut.insert_edge(3, 1, 1.0).unwrap();
        assert_eq!(mincut.min_cut_value(), 2.0);
    }

    #[test]
    fn test_disconnected_graph() {
        let mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 1.0), (3, 4, 1.0)])
            .build()
            .unwrap();

        assert!(!mincut.is_connected());
        assert_eq!(mincut.min_cut_value(), 0.0);
    }

    #[test]
    fn test_builder_pattern() {
        let mincut = MinCutBuilder::new()
            .exact()
            .max_cut_size(500)
            .parallel(true)
            .with_edges(vec![(1, 2, 1.0)])
            .build()
            .unwrap();

        assert!(!mincut.config().approximate);
        assert_eq!(mincut.config().max_exact_cut_size, 500);
        assert!(mincut.config().parallel);
    }

    #[test]
    fn test_weighted_graph() {
        let mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 5.0), (2, 3, 3.0), (3, 1, 2.0)])
            .build()
            .unwrap();

        assert_eq!(mincut.min_cut_value(), 5.0);
    }

    #[test]
    fn test_error_handling() {
        let mut mincut = MinCutBuilder::new()
            .with_edges(vec![(1, 2, 1.0)])
            .build()
            .unwrap();

        // Try to insert duplicate edge
        let result = mincut.insert_edge(1, 2, 2.0);
        assert!(result.is_err());
        assert!(matches!(result, Err(MinCutError::EdgeExists(1, 2))));

        // Try to delete non-existent edge
        let result = mincut.delete_edge(3, 4);
        assert!(result.is_err());
        assert!(matches!(result, Err(MinCutError::EdgeNotFound(3, 4))));
    }

    #[test]
    fn test_large_graph() {
        let mut edges = Vec::new();
        for i in 0..99 {
            edges.push((i, i + 1, 1.0));
        }

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        assert_eq!(mincut.num_vertices(), 100);
        assert_eq!(mincut.num_edges(), 99);
        assert_eq!(mincut.min_cut_value(), 1.0);
    }

    #[cfg(feature = "monitoring")]
    #[test]
    fn test_monitoring_feature() {
        use crate::monitoring::EventType;

        // Ensure monitoring types are accessible when feature is enabled
        let _ = EventType::CutIncreased;
        let _ = EventType::CutDecreased;
    }
}
