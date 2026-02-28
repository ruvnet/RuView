//! Core traits for backend abstraction
//!
//! This module defines the primary traits that all substrate backends must implement,
//! enabling hardware-agnostic development across classical, neuromorphic, photonic,
//! and processing-in-memory architectures.

use crate::types::*;
use async_trait::async_trait;

/// Backend trait for substrate compute operations
///
/// This trait abstracts over different hardware backends (classical, neuromorphic,
/// photonic, PIM) providing a unified interface for cognitive substrate operations.
///
/// # Type Parameters
///
/// * `Error` - Backend-specific error type
///
/// # Examples
///
/// ```rust,ignore
/// use exo_core::{SubstrateBackend, Pattern};
///
/// struct MyBackend;
///
/// #[async_trait]
/// impl SubstrateBackend for MyBackend {
///     type Error = std::io::Error;
///
///     async fn similarity_search(
///         &self,
///         query: &[f32],
///         k: usize,
///         filter: Option<&Filter>,
///     ) -> Result<Vec<SearchResult>, Self::Error> {
///         // Implementation
///         Ok(vec![])
///     }
///
///     // ... other methods
/// }
/// ```
#[async_trait]
pub trait SubstrateBackend: Send + Sync {
    /// Backend-specific error type
    type Error: std::error::Error + Send + Sync + 'static;

    /// Execute similarity search on substrate
    ///
    /// Finds the k-nearest neighbors to the query vector in the substrate's
    /// learned manifold. Optionally applies metadata filters.
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector embedding
    /// * `k` - Number of nearest neighbors to retrieve
    /// * `filter` - Optional metadata filter
    ///
    /// # Returns
    ///
    /// Vector of search results ordered by similarity (descending)
    async fn similarity_search(
        &self,
        query: &[f32],
        k: usize,
        filter: Option<&Filter>,
    ) -> Result<Vec<SearchResult>, Self::Error>;

    /// Deform manifold to incorporate new pattern
    ///
    /// For continuous manifold backends (neural implicit representations),
    /// this performs gradient-based deformation. For discrete backends,
    /// this performs an insert operation.
    ///
    /// # Arguments
    ///
    /// * `pattern` - Pattern to integrate into substrate
    /// * `learning_rate` - Deformation strength (0.0-1.0)
    ///
    /// # Returns
    ///
    /// ManifoldDelta describing the change applied
    async fn manifold_deform(
        &self,
        pattern: &Pattern,
        learning_rate: f32,
    ) -> Result<ManifoldDelta, Self::Error>;

    /// Execute hyperedge query
    ///
    /// Performs topological queries on the substrate's hypergraph structure,
    /// supporting persistent homology, Betti numbers, and sheaf consistency.
    ///
    /// # Arguments
    ///
    /// * `query` - Topological query specification
    ///
    /// # Returns
    ///
    /// HyperedgeResult containing query-specific results
    async fn hyperedge_query(
        &self,
        query: &TopologicalQuery,
    ) -> Result<HyperedgeResult, Self::Error>;
}

/// Temporal context for causal operations
///
/// This trait provides temporal memory operations with causal structure,
/// enabling queries constrained by light-cone causality and anticipatory
/// pre-fetching based on predicted future queries.
///
/// # Examples
///
/// ```rust,ignore
/// use exo_core::{TemporalContext, CausalCone};
///
/// async fn temporal_query<T: TemporalContext>(ctx: &T) {
///     let now = ctx.now();
///     let cone = CausalCone::past(now);
///     let results = ctx.causal_query(&query, &cone).await?;
/// }
/// ```
#[async_trait]
pub trait TemporalContext: Send + Sync {
    /// Get current substrate time
    ///
    /// Returns a monotonically increasing timestamp representing
    /// the current substrate clock.
    fn now(&self) -> SubstrateTime;

    /// Query with causal cone constraints
    ///
    /// Retrieves patterns within the specified causal cone,
    /// respecting temporal ordering and causal dependencies.
    ///
    /// # Arguments
    ///
    /// * `query` - Query specification
    /// * `cone` - Causal cone constraint (past, future, or light-cone)
    ///
    /// # Returns
    ///
    /// Vector of results with causal and temporal distance metrics
    async fn causal_query(
        &self,
        query: &Query,
        cone: &CausalCone,
    ) -> Result<Vec<CausalResult>, Error>;

    /// Predictive pre-fetch based on anticipated queries
    ///
    /// Warms cache with predicted future queries based on
    /// current context and usage patterns.
    ///
    /// # Arguments
    ///
    /// * `hints` - Anticipation hints for prediction
    async fn anticipate(&self, hints: &[AnticipationHint]) -> Result<(), Error>;
}

/// Optional trait for Processing-in-Memory backends
///
/// Future backend interface for PIM hardware (UPMEM, Samsung Aquabolt-XL)
#[async_trait]
pub trait PimBackend: SubstrateBackend {
    /// Execute operation directly in memory bank
    async fn execute_in_memory(&self, op: &MemoryOperation) -> Result<(), Error>;

    /// Query memory bank location for data
    fn data_location(&self, pattern_id: PatternId) -> MemoryBank;
}

/// Optional trait for Neuromorphic backends
///
/// Future backend interface for neuromorphic hardware (Intel Loihi, IBM TrueNorth)
#[async_trait]
pub trait NeuromorphicBackend: SubstrateBackend {
    /// Encode vector as spike train
    fn encode_spikes(&self, vector: &[f32]) -> SpikeTrain;

    /// Decode spike train to vector
    fn decode_spikes(&self, spikes: &SpikeTrain) -> Vec<f32>;

    /// Submit spike computation
    async fn submit_spike_compute(&self, input: SpikeTrain) -> Result<SpikeTrain, Error>;
}

/// Optional trait for Photonic backends
///
/// Future backend interface for photonic computing (Lightmatter, Luminous)
#[async_trait]
pub trait PhotonicBackend: SubstrateBackend {
    /// Optical matrix-vector multiply
    async fn optical_matmul(&self, matrix: &OpticalMatrix, vector: &[f32]) -> Vec<f32>;

    /// Configure Mach-Zehnder interferometer
    async fn configure_mzi(&self, config: &MziConfig) -> Result<(), Error>;
}

// Placeholder types for future backend traits
/// Memory operation specification for PIM backends
#[derive(Clone, Debug)]
pub struct MemoryOperation {
    pub operation_type: String,
    pub data: Vec<u8>,
}

/// Memory bank identifier for PIM backends
#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash)]
pub struct MemoryBank(pub u32);

/// Spike train for neuromorphic backends
#[derive(Clone, Debug)]
pub struct SpikeTrain {
    pub timestamps: Vec<f64>,
    pub neuron_ids: Vec<u32>,
}

/// Optical matrix for photonic backends
#[derive(Clone, Debug)]
pub struct OpticalMatrix {
    pub dimensions: (usize, usize),
    pub phase_shifts: Vec<f32>,
}

/// MZI configuration for photonic backends
#[derive(Clone, Debug)]
pub struct MziConfig {
    pub phase: f32,
    pub attenuation: f32,
}
