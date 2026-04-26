//! Pipeline trait definitions that downstream crates implement.

use crate::embedding::NeuralEmbedding;
use crate::error::Result;
use crate::graph::BrainGraph;
use crate::rvf::RvfFile;
use crate::sensor::SensorType;
use crate::signal::MultiChannelTimeSeries;
use crate::topology::{CognitiveState, MincutResult, TopologyMetrics};

/// Trait for sensor data sources (hardware or simulated).
pub trait SensorSource {
    /// The sensor technology used by this source.
    fn sensor_type(&self) -> SensorType;

    /// Number of channels available.
    fn num_channels(&self) -> usize;

    /// Sampling rate in Hz.
    fn sample_rate_hz(&self) -> f64;

    /// Read a chunk of `num_samples` from the source.
    fn read_chunk(&mut self, num_samples: usize) -> Result<MultiChannelTimeSeries>;
}

/// Trait for signal processors (filters, artifact removal, etc.).
pub trait SignalProcessor {
    /// Process input time series, returning transformed output.
    fn process(&self, input: &MultiChannelTimeSeries) -> Result<MultiChannelTimeSeries>;
}

/// Trait for graph constructors (builds connectivity graphs from signals).
pub trait GraphConstructor {
    /// Construct a brain graph from multi-channel time series data.
    fn construct(&self, signals: &MultiChannelTimeSeries) -> Result<BrainGraph>;
}

/// Trait for topology analyzers (computes graph-theoretic metrics).
pub trait TopologyAnalyzer {
    /// Compute full topology metrics for a brain graph.
    fn analyze(&self, graph: &BrainGraph) -> Result<TopologyMetrics>;

    /// Compute the minimum cut of a brain graph.
    fn mincut(&self, graph: &BrainGraph) -> Result<MincutResult>;
}

/// Trait for embedding generators (maps brain graphs to vector space).
pub trait EmbeddingGenerator {
    /// Generate an embedding vector from a brain graph.
    fn embed(&self, graph: &BrainGraph) -> Result<NeuralEmbedding>;

    /// Dimensionality of the output embedding.
    fn embedding_dim(&self) -> usize;
}

/// Trait for state decoders (classifies cognitive state from embeddings).
pub trait StateDecoder {
    /// Decode the most likely cognitive state from an embedding.
    fn decode(&self, embedding: &NeuralEmbedding) -> Result<CognitiveState>;

    /// Decode with a confidence score in [0, 1].
    fn decode_with_confidence(
        &self,
        embedding: &NeuralEmbedding,
    ) -> Result<(CognitiveState, f64)>;
}

/// Trait for neural state memory (stores and queries embedding history).
pub trait NeuralMemory {
    /// Store an embedding in memory.
    fn store(&mut self, embedding: &NeuralEmbedding) -> Result<()>;

    /// Find the k nearest embeddings to the query.
    fn query_nearest(
        &self,
        embedding: &NeuralEmbedding,
        k: usize,
    ) -> Result<Vec<NeuralEmbedding>>;

    /// Find all stored embeddings matching a cognitive state.
    fn query_by_state(&self, state: CognitiveState) -> Result<Vec<NeuralEmbedding>>;
}

/// Trait for RVF serialization support.
pub trait RvfSerializable {
    /// Serialize this value to an RVF file.
    fn to_rvf(&self) -> Result<RvfFile>;

    /// Deserialize from an RVF file.
    fn from_rvf(file: &RvfFile) -> Result<Self>
    where
        Self: Sized;
}
