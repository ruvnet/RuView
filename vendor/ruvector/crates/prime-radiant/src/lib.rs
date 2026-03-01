//! # Prime-Radiant: Universal Coherence Engine
//!
//! The Prime-Radiant crate implements a **universal coherence engine** using sheaf
//! Laplacian mathematics to provide structural consistency guarantees across domains.
//!
//! ## Vision
//!
//! > "Most systems try to get smarter by making better guesses. Prime-Radiant takes a
//! > different route: systems that stay stable under uncertainty by proving when the
//! > world still fits together and when it does not."
//!
//! **This is not prediction.** It is a continuously updated field of coherence that
//! shows where action is safe and where action must stop.
//!
//! ## The Universal Coherence Object
//!
//! The power of this approach lies in a **single underlying coherence object**. Once
//! the math is fixed, everything else becomes interpretation:
//!
//! | Domain | Nodes Are | Edges Are | Residual Becomes | Gate Becomes |
//! |--------|-----------|-----------|------------------|--------------|
//! | **AI Agents** | Facts, hypotheses, beliefs | Citations, logical implication | Contradiction energy | Hallucination refusal |
//! | **Finance** | Trades, positions, signals | Market dependencies, arbitrage | Regime mismatch | Trading throttle |
//! | **Medical** | Vitals, diagnoses, treatments | Physiological causality | Clinical disagreement | Escalation trigger |
//! | **Robotics** | Sensor readings, goals, plans | Physics, kinematics | Motion impossibility | Safety stop |
//! | **Security** | Identities, permissions, actions | Policy rules, trust chains | Authorization violation | Access denial |
//! | **Science** | Hypotheses, observations, models | Experimental evidence | Theory inconsistency | Pruning signal |
//!
//! ## Architecture Overview
//!
//! ```text
//! +-----------------------------------------------------------------------------+
//! |                           APPLICATION LAYER                                  |
//! |  LLM Guards | Fraud Detection | Compliance Proofs | Robotics Safety         |
//! +-----------------------------------------------------------------------------+
//!                                     |
//! +-----------------------------------------------------------------------------+
//! |                           COHERENCE GATE                                     |
//! |  Lane 0 (Reflex) | Lane 1 (Retrieval) | Lane 2 (Heavy) | Lane 3 (Human)     |
//! +-----------------------------------------------------------------------------+
//!                                     |
//! +-----------------------------------------------------------------------------+
//! |                           COHERENCE COMPUTATION                              |
//! |  Residual Calculator | Energy Aggregator | Spectral Analyzer | Fingerprints |
//! +-----------------------------------------------------------------------------+
//!                                     |
//! +-----------------------------------------------------------------------------+
//! |                           GOVERNANCE LAYER                                   |
//! |  Policy Bundles | Witness Records | Lineage Records | Threshold Tuning      |
//! +-----------------------------------------------------------------------------+
//!                                     |
//! +-----------------------------------------------------------------------------+
//! |                           KNOWLEDGE SUBSTRATE                                |
//! |  Sheaf Graph | Node States | Edge Constraints | Restriction Maps           |
//! +-----------------------------------------------------------------------------+
//!                                     |
//! +-----------------------------------------------------------------------------+
//! |                           STORAGE LAYER                                      |
//! |  PostgreSQL (Authority) | ruvector (Graph/Vector) | Event Log (Audit)       |
//! +-----------------------------------------------------------------------------+
//! ```
//!
//! ## Key Mathematical Concepts
//!
//! | Concept | Mathematical Definition | System Interpretation |
//! |---------|------------------------|----------------------|
//! | **Node** | Vertex v with state x_v | Entity with fixed-dimensional state vector |
//! | **Edge** | (u, v) connection | Constraint between entities |
//! | **Restriction Map** | rho: F(U) -> F(V) | How one state constrains another |
//! | **Residual** | r_e = rho_u(x_u) - rho_v(x_v) | **Contradiction energy** at edge |
//! | **Energy** | E(S) = sum(w_e * ||r_e||^2) | Global incoherence measure |
//! | **Gate** | E < threshold | **Refusal mechanism with witness** |
//!
//! ## Compute Ladder
//!
//! Most updates remain in a **low-latency reflex lane**, while **sustained or growing**
//! incoherence triggers escalation:
//!
//! - **Lane 0 (Reflex)**: Local residual updates, simple aggregates (<1ms)
//! - **Lane 1 (Retrieval)**: Evidence fetching, lightweight reasoning (~10ms)
//! - **Lane 2 (Heavy)**: Multi-step planning, spectral analysis (~100ms)
//! - **Lane 3 (Human)**: Human escalation for sustained incoherence
//!
//! ## Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `tiles` | Yes | cognitum-gate-kernel 256-tile fabric |
//! | `sona` | Yes | Self-optimizing threshold tuning |
//! | `learned-rho` | No | GNN-learned restriction maps |
//! | `hyperbolic` | No | Hierarchy-aware Poincare energy |
//! | `mincut` | No | Subpolynomial graph partitioning |
//! | `neural-gate` | Yes | Nervous-system CoherenceGatedSystem |
//! | `attention` | No | Attention-weighted residuals (MoE, PDE) |
//! | `distributed` | No | Raft-based multi-node coherence |
//! | `postgres` | No | PostgreSQL governance storage |
//!
//! ## Example
//!
//! ```rust,ignore
//! use prime_radiant::{
//!     substrate::{SheafGraph, SheafNode, SheafEdge, RestrictionMap},
//!     coherence::{CoherenceEngine, CoherenceEnergy},
//!     execution::{CoherenceGate, ComputeLane, GateDecision},
//!     governance::{PolicyBundle, WitnessRecord},
//! };
//!
//! // Create a sheaf graph
//! let mut graph = SheafGraph::new();
//!
//! // Add nodes with state vectors
//! let node1 = SheafNode::new(vec![1.0, 0.0, 0.0]);
//! let node2 = SheafNode::new(vec![0.9, 0.1, 0.0]);
//! graph.add_node(node1);
//! graph.add_node(node2);
//!
//! // Add edge with restriction maps
//! let rho = RestrictionMap::identity(3);
//! let edge = SheafEdge::new(node1.id, node2.id, rho.clone(), rho, 1.0);
//! graph.add_edge(edge)?;
//!
//! // Compute coherence energy
//! let mut engine = CoherenceEngine::new();
//! let energy = engine.compute_energy(&graph);
//!
//! // Gate an action
//! let gate = CoherenceGate::new(policy);
//! let decision = gate.evaluate(&action, &energy);
//!
//! match decision.lane {
//!     ComputeLane::Reflex => println!("Action allowed in reflex lane"),
//!     ComputeLane::Human => println!("Escalating to human review"),
//!     _ => println!("Additional processing required"),
//! }
//! ```
//!
//! ## References
//!
//! 1. Hansen, J., & Ghrist, R. (2019). "Toward a spectral theory of cellular sheaves."
//! 2. ADR-014: Coherence Engine Architecture
//! 3. Robinson, M. (2014). "Topological Signal Processing."

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// ============================================================================
// MODULE DECLARATIONS
// ============================================================================

// -----------------------------------------------------------------------------
// Core Bounded Contexts
// -----------------------------------------------------------------------------

/// Signal ingestion - validates and normalizes incoming events
pub mod signal;

/// Knowledge substrate - sheaf graph with nodes, edges, and restriction maps
pub mod substrate;

/// Coherence computation - residuals, energy aggregation, spectral analysis
pub mod coherence;

/// Governance - policy bundles, witness records, lineage tracking
pub mod governance;

/// Action execution - coherence gate with compute ladder
pub mod execution;

/// Storage layer - PostgreSQL authority, ruvector graph/vector, event log
pub mod storage;

/// Security module - input validation, resource limits, path sanitization
pub mod security;

/// Cohomology computation - sheaf cohomology, obstruction detection, sheaf neural networks
pub mod cohomology;

// -----------------------------------------------------------------------------
// Ecosystem Integration Modules
// -----------------------------------------------------------------------------

/// Tile fabric - 256-tile WASM coherence fabric (cognitum-gate-kernel)
#[cfg(feature = "tiles")]
#[cfg_attr(docsrs, doc(cfg(feature = "tiles")))]
pub mod tiles;

/// SONA tuning - self-optimizing threshold management
#[cfg(feature = "sona")]
#[cfg_attr(docsrs, doc(cfg(feature = "sona")))]
pub mod sona_tuning;

/// Neural gate - biologically-inspired gating (ruvector-nervous-system)
#[cfg(feature = "neural-gate")]
#[cfg_attr(docsrs, doc(cfg(feature = "neural-gate")))]
pub mod neural_gate;

/// Learned restriction maps - GNN-based rho learning (ruvector-gnn)
#[cfg(feature = "learned-rho")]
#[cfg_attr(docsrs, doc(cfg(feature = "learned-rho")))]
pub mod learned_rho;

/// Hyperbolic coherence - hierarchy-aware Poincare energy
#[cfg(feature = "hyperbolic")]
#[cfg_attr(docsrs, doc(cfg(feature = "hyperbolic")))]
pub mod hyperbolic;

/// MinCut isolation - subpolynomial incoherent region isolation
#[cfg(feature = "mincut")]
#[cfg_attr(docsrs, doc(cfg(feature = "mincut")))]
pub mod mincut;

/// Attention weighting - topology-gated, MoE, PDE diffusion
#[cfg(feature = "attention")]
#[cfg_attr(docsrs, doc(cfg(feature = "attention")))]
pub mod attention;

/// Distributed consensus - Raft-based multi-node coherence
#[cfg(feature = "distributed")]
#[cfg_attr(docsrs, doc(cfg(feature = "distributed")))]
pub mod distributed;

/// RuvLLM integration - coherence-to-confidence mapping and LLM gating
#[cfg(feature = "ruvllm")]
#[cfg_attr(docsrs, doc(cfg(feature = "ruvllm")))]
pub mod ruvllm_integration;

/// GPU acceleration - wgpu-based parallel coherence computation
#[cfg(feature = "gpu")]
#[cfg_attr(docsrs, doc(cfg(feature = "gpu")))]
pub mod gpu;

/// SIMD optimizations - explicit SIMD intrinsics for high-performance computation
#[cfg(feature = "simd")]
#[cfg_attr(docsrs, doc(cfg(feature = "simd")))]
pub mod simd;

// -----------------------------------------------------------------------------
// Shared Types and Errors
// -----------------------------------------------------------------------------

/// Domain events across all bounded contexts
pub mod events;

/// Error types for the coherence engine
pub mod error;

/// Shared types (IDs, timestamps, hashes)
pub mod types;

// ============================================================================
// PUBLIC API EXPORTS
// ============================================================================

// Re-export core types for convenience
pub use types::{
    ActorId,
    ApproverId,
    EdgeId,
    GraphId,
    Hash,
    LineageId,
    NamespaceId,
    // Identifiers
    NodeId,
    PolicyBundleId,
    ScopeId,
    // Primitives
    Timestamp,
    Version,
    WitnessId,
};

pub use error::{CoherenceError, ExecutionError, GovernanceError, StorageError, SubstrateError};

// Re-export security types
pub use security::{
    GraphLimits, InputValidator, PathValidator, ResourceLimits, SecurityConfig, StateValidator,
    ValidationError, ValidationResult,
};

pub use events::DomainEvent;

// Re-export substrate types
pub use substrate::{
    NodeMetadata, RestrictionMap, SheafEdge, SheafGraph, SheafNode, SheafSubgraph,
};

// Re-export coherence types
pub use coherence::{
    CoherenceConfig, CoherenceEnergy, CoherenceEngine, EnergyHistory, ResidualCache,
};

// Re-export cohomology types
pub use cohomology::{
    Activation,
    BettiNumbers,
    Chain,
    Coboundary,
    Cochain,
    // Cocycle and coboundary
    Cocycle,
    CocycleBuilder,
    CohomologyComputer,
    CohomologyConfig,
    // Cohomology groups
    CohomologyGroup,
    CohomologyPooling,
    DiffusionResult,
    HarmonicRepresentative,
    LaplacianConfig,
    LaplacianSpectrum,
    LocalSection,
    Obstruction,
    // Obstruction detection
    ObstructionDetector,
    ObstructionIndicator,
    ObstructionReport,
    ObstructionSeverity,
    PoolingMethod,
    // Sheaf types
    Sheaf,
    SheafBuilder,
    SheafConvolution,
    // Diffusion
    SheafDiffusion,
    SheafDiffusionConfig,
    // Laplacian
    SheafLaplacian,
    SheafNeuralConfig,
    // Neural network layers
    SheafNeuralLayer,
    SheafSection,
    // Simplex and simplicial complex
    Simplex,
    SimplexId,
    SimplicialComplex,
    Stalk,
};

// Re-export governance types
pub use governance::{
    ApprovalSignature,
    ApproverId as GovApproverId,
    EntityRef,
    EscalationRule,
    // Top-level error
    GovernanceError as GovError,
    // Common types
    Hash as GovHash,
    LineageError,
    LineageId as GovLineageId,
    // Lineage types
    LineageRecord,
    LineageRepository,
    Operation,
    // Policy types
    PolicyBundle,
    PolicyBundleBuilder,
    PolicyBundleRef,
    PolicyBundleStatus,
    PolicyError,
    // Repository traits
    PolicyRepository,
    ThresholdConfig,
    Timestamp as GovTimestamp,
    Version as GovVersion,
    WitnessChainError,
    WitnessError,
    WitnessId as GovWitnessId,
    // Witness types (governance's own witness format)
    WitnessRecord as GovWitnessRecord,
    WitnessRepository,
};

// Re-export execution types (coherence gate and compute ladder)
pub use execution::{
    // Actions
    Action,
    ActionExecutor,
    ActionId,
    ActionImpact,
    ActionMetadata,
    ActionResult,
    // Gate and ladder
    CoherenceGate,
    ComputeLane,
    EnergySnapshot,
    EscalationReason,
    ExecutionContext,
    ExecutionResult,
    ExecutorConfig,
    ExecutorStats,
    GateDecision,
    LaneThresholds,
    PolicyBundleRef as ExecutionPolicyRef,
    // Scope
    ScopeId as ExecutionScopeId,
    WitnessId as ExecWitnessId,
    // Witness (execution's witness format - aliased to avoid conflict with types::WitnessId)
    WitnessRecord as ExecWitnessRecord,
};

// Conditional re-exports based on features

#[cfg(feature = "tiles")]
pub use tiles::{CoherenceFabric, FabricReport, ShardMap, TileAdapter};

#[cfg(feature = "sona")]
pub use sona_tuning::{
    SonaThresholdTuner, ThresholdAdjustment, ThresholdConfig as SonaThresholdConfig,
};

#[cfg(feature = "neural-gate")]
pub use neural_gate::{NeuralCoherenceGate, NeuralDecision, WitnessEncoding};

#[cfg(feature = "learned-rho")]
pub use learned_rho::{LearnedRestrictionMap, RestrictionMapConfig, TrainingBatch};

#[cfg(feature = "hyperbolic")]
pub use hyperbolic::{
    DepthComputer, HierarchyLevel, HyperbolicAdapter, HyperbolicCoherence,
    HyperbolicCoherenceConfig, HyperbolicEnergy, WeightedResidual,
};

#[cfg(feature = "mincut")]
pub use mincut::{
    IncoherenceIsolator, IsolationMetrics, IsolationRegion, IsolationResult, MinCutAdapter,
    MinCutConfig,
};

#[cfg(feature = "attention")]
pub use attention::{
    AttentionAdapter, AttentionCoherence, AttentionCoherenceConfig, AttentionEnergyAnalysis,
    DiffusionSmoothing, ExpertRouting, MoEResidualProcessor, SmoothedEnergy, TopologyGate,
    TopologyGateResult, WeightedEdgeResidual,
};

#[cfg(feature = "distributed")]
pub use distributed::{
    ClusterStatus, CoherenceStateMachine, CoherenceStatus, DistributedCoherence,
    DistributedCoherenceConfig, NodeRole, RaftAdapter,
};

#[cfg(feature = "ruvllm")]
pub use ruvllm_integration::{
    CoherenceConfidence, ConfidenceLevel, ConfidenceScore, EnergyContributor,
};

#[cfg(feature = "gpu")]
pub use gpu::{
    BindingDesc,
    BindingType,
    BufferKey,
    BufferUsage,
    BufferUsageFlags,
    ComputeEnergyKernel,
    // Pipeline management
    ComputePipeline,
    // Kernel types
    ComputeResidualsKernel,
    DispatchBuilder,
    DispatchConfig,
    // Buffer management
    GpuBuffer,
    GpuBufferManager,
    GpuBufferPool,
    GpuCapabilities,
    GpuCoherenceEnergy,
    // GPU coherence engine
    GpuCoherenceEngine,
    GpuConfig,
    // Device management
    GpuDevice,
    GpuDeviceInfo,
    GpuDeviceOptions,
    // Dispatch and synchronization
    GpuDispatcher,
    // Errors
    GpuError,
    GpuResult,
    PipelineCache,
    SheafAttentionKernel,
    TokenRoutingKernel,
};

#[cfg(feature = "simd")]
pub use simd::{
    batch_lane_assignment_simd, batch_residuals_simd, best_simd_width, dot_product_simd,
    matmul_simd, matvec_simd, norm_squared_simd, scale_simd, subtract_simd,
    weighted_energy_sum_simd, SimdContext, SimdWidth,
};

// ============================================================================
// PRELUDE MODULE
// ============================================================================

/// Convenient imports for common use cases
pub mod prelude {
    pub use crate::{
        CoherenceEnergy,

        // Coherence
        CoherenceEngine,
        // Errors
        CoherenceError,
        // Execution
        CoherenceGate,
        CohomologyComputer,
        CohomologyGroup,
        ComputeLane,

        // Events
        DomainEvent,
        EdgeId,
        GateDecision,
        GovWitnessRecord as WitnessRecord, // Re-export governance witness as default

        GraphId,
        Hash,
        // Security
        InputValidator,
        // Core types
        NodeId,
        ObstructionDetector,
        // Governance
        PolicyBundle,
        RestrictionMap,

        ScopeId,
        SecurityConfig,

        SheafDiffusion,
        SheafEdge,
        // Substrate
        SheafGraph,
        // Cohomology
        SheafLaplacian,
        SheafNeuralLayer,

        SheafNode,
        ThresholdConfig,
        Timestamp,
        ValidationError,

        Version,
    };
}

// ============================================================================
// CRATE-LEVEL CONSTANTS
// ============================================================================

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default dimension for state vectors
pub const DEFAULT_STATE_DIM: usize = 64;

/// Default number of tiles in the fabric
pub const DEFAULT_TILE_COUNT: usize = 256;

/// Default persistence window for threshold detection (seconds)
pub const DEFAULT_PERSISTENCE_WINDOW_SECS: u64 = 300;

// ============================================================================
// PERFORMANCE TARGETS (ADR-014)
// ============================================================================

/// Performance targets from ADR-014
pub mod targets {
    /// Single residual calculation target: < 1us
    pub const RESIDUAL_CALC_US: u64 = 1;

    /// Full graph energy (10K nodes) target: < 10ms
    pub const FULL_ENERGY_MS: u64 = 10;

    /// Incremental update (1 node) target: < 100us
    pub const INCREMENTAL_UPDATE_US: u64 = 100;

    /// Gate evaluation target: < 500us
    pub const GATE_EVAL_US: u64 = 500;

    /// Witness persistence target: < 5ms
    pub const WITNESS_PERSIST_MS: u64 = 5;

    /// Tile tick (256 tiles parallel) target: < 1ms
    pub const TILE_TICK_MS: u64 = 1;

    /// SONA instant adaptation target: < 0.05ms (50us)
    pub const SONA_ADAPT_US: u64 = 50;

    /// MinCut update (amortized) target: n^o(1)
    pub const MINCUT_SUBPOLY: bool = true;

    /// HDC witness encoding target: < 10us
    pub const HDC_ENCODE_US: u64 = 10;

    /// Hyperbolic distance target: < 500ns
    pub const HYPERBOLIC_DIST_NS: u64 = 500;

    /// Attention-weighted energy target: < 5ms
    pub const ATTENTION_ENERGY_MS: u64 = 5;

    /// Distributed consensus target: < 50ms
    pub const CONSENSUS_MS: u64 = 50;
}
