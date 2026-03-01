//! Core trait definitions and types for EXO-AI cognitive substrate
//!
//! This crate provides the foundational abstractions that all other EXO-AI
//! crates build upon, including backend traits, pattern representations,
//! and core error types.
//!
//! # Theoretical Framework Modules
//!
//! - [`consciousness`]: Integrated Information Theory (IIT 4.0) implementation
//!   for computing Φ (phi) - the measure of integrated information
//! - [`thermodynamics`]: Landauer's Principle tracking for measuring
//!   computational efficiency relative to fundamental physics limits

pub mod backends;
pub mod coherence_router;
pub mod consciousness;
pub mod genomic;
pub mod plasticity_engine;
pub mod thermodynamics;
pub mod witness;

pub use genomic::{GenomicPatternStore, HorvathClock, NeurotransmitterProfile, RvDnaPattern};

pub use backends::{
    NeuromorphicBackend, QuantumStubBackend, SubstrateBackend as ComputeSubstrateBackend,
};
pub use coherence_router::{ActionContext, CoherenceBackend, CoherenceRouter, GateDecision};
pub use plasticity_engine::{PlasticityDelta, PlasticityEngine, PlasticityMode};
pub use witness::WitnessDecision as CoherenceDecision;
pub use witness::{CrossParadigmWitness, WitnessChain, WitnessDecision};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

/// Pattern representation in substrate
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    /// Unique identifier
    pub id: PatternId,
    /// Vector embedding
    pub embedding: Vec<f32>,
    /// Metadata
    pub metadata: Metadata,
    /// Temporal origin
    pub timestamp: SubstrateTime,
    /// Causal antecedents
    pub antecedents: Vec<PatternId>,
    /// Salience score (importance)
    pub salience: f32,
}

/// Pattern identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PatternId(pub Uuid);

impl PatternId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PatternId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PatternId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Substrate time representation (nanoseconds since epoch)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SubstrateTime(pub i64);

impl SubstrateTime {
    pub const MIN: Self = Self(i64::MIN);
    pub const MAX: Self = Self(i64::MAX);

    pub fn now() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        Self(duration.as_nanos() as i64)
    }

    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }
}

impl std::ops::Sub for SubstrateTime {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

/// Metadata for patterns
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub fields: HashMap<String, MetadataValue>,
}

impl Metadata {
    /// Create empty metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// Create metadata with a single field
    pub fn with_field(key: impl Into<String>, value: MetadataValue) -> Self {
        let mut fields = HashMap::new();
        fields.insert(key.into(), value);
        Self { fields }
    }

    /// Add a field
    pub fn insert(&mut self, key: impl Into<String>, value: MetadataValue) -> &mut Self {
        self.fields.insert(key.into(), value);
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MetadataValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<MetadataValue>),
}

/// Search result
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub pattern: Pattern,
    pub score: f32,
    pub distance: f32,
}

/// Filter for search operations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Filter {
    pub conditions: Vec<FilterCondition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilterCondition {
    pub field: String,
    pub operator: FilterOperator,
    pub value: MetadataValue,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FilterOperator {
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    Contains,
}

/// Manifold delta result from deformation
#[derive(Clone, Debug)]
pub enum ManifoldDelta {
    /// Continuous deformation applied
    ContinuousDeform {
        embedding: Vec<f32>,
        salience: f32,
        loss: f32,
    },
    /// Classical discrete insert (for classical backend)
    DiscreteInsert { id: PatternId },
}

/// Entity identifier (for hypergraph)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub Uuid);

impl EntityId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Hyperedge identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HyperedgeId(pub Uuid);

impl HyperedgeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for HyperedgeId {
    fn default() -> Self {
        Self::new()
    }
}

/// Section identifier (for sheaf structures)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SectionId(pub Uuid);

impl SectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SectionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Relation type for hyperedges
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationType(pub String);

impl RelationType {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

/// Relation between entities in hyperedge
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relation {
    pub relation_type: RelationType,
    pub properties: serde_json::Value,
}

/// Topological query specification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TopologicalQuery {
    /// Find persistent homology features
    PersistentHomology {
        dimension: usize,
        epsilon_range: (f32, f32),
    },
    /// Find Betti numbers
    BettiNumbers { max_dimension: usize },
    /// Sheaf consistency check
    SheafConsistency { local_sections: Vec<SectionId> },
}

/// Result from hyperedge query
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HyperedgeResult {
    PersistenceDiagram(Vec<(f32, f32)>),
    BettiNumbers(Vec<usize>),
    SheafConsistency(SheafConsistencyResult),
    NotSupported,
}

/// Sheaf consistency result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SheafConsistencyResult {
    Consistent,
    Inconsistent(Vec<String>),
    NotConfigured,
}

/// Error types
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Pattern not found: {0}")]
    PatternNotFound(PatternId),

    #[error("Invalid embedding dimension: expected {expected}, got {got}")]
    InvalidDimension { expected: usize, got: usize },

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("Convergence failed")]
    ConvergenceFailed,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Backend trait for substrate compute operations
pub trait SubstrateBackend: Send + Sync {
    /// Execute similarity search on substrate
    fn similarity_search(
        &self,
        query: &[f32],
        k: usize,
        filter: Option<&Filter>,
    ) -> Result<Vec<SearchResult>>;

    /// Deform manifold to incorporate new pattern
    fn manifold_deform(&self, pattern: &Pattern, learning_rate: f32) -> Result<ManifoldDelta>;

    /// Get embedding dimension
    fn dimension(&self) -> usize;
}

/// Configuration for manifold operations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManifoldConfig {
    /// Embedding dimension
    pub dimension: usize,
    /// Maximum gradient descent steps
    pub max_descent_steps: usize,
    /// Learning rate for gradient descent
    pub learning_rate: f32,
    /// Convergence threshold for gradient norm
    pub convergence_threshold: f32,
    /// Number of hidden layers
    pub hidden_layers: usize,
    /// Hidden dimension size
    pub hidden_dim: usize,
    /// Omega_0 for SIREN (frequency parameter)
    pub omega_0: f32,
}

impl Default for ManifoldConfig {
    fn default() -> Self {
        Self {
            dimension: 768,
            max_descent_steps: 100,
            learning_rate: 0.01,
            convergence_threshold: 1e-4,
            hidden_layers: 3,
            hidden_dim: 256,
            omega_0: 30.0,
        }
    }
}
