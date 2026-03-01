//! # RuVector Data Discovery Framework
//!
//! Core traits and types for building dataset integrations with RuVector's
//! vector memory, graph structures, and dynamic minimum cut algorithms.
//!
//! ## Architecture
//!
//! The framework provides three core abstractions:
//!
//! 1. **DataIngester**: Streaming data ingestion with batched graph/vector updates
//! 2. **CoherenceEngine**: Real-time coherence signal computation using min-cut
//! 3. **DiscoveryEngine**: Pattern detection for emerging structures and anomalies
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ruvector_data_framework::{
//!     DataIngester, CoherenceEngine, DiscoveryEngine,
//!     IngestionConfig, CoherenceConfig, DiscoveryConfig,
//! };
//!
//! // Configure the discovery pipeline
//! let ingester = DataIngester::new(ingestion_config);
//! let coherence = CoherenceEngine::new(coherence_config);
//! let discovery = DiscoveryEngine::new(discovery_config);
//!
//! // Stream data and detect patterns
//! let stream = ingester.stream_from_source(source).await?;
//! let signals = coherence.compute_signals(stream).await?;
//! let patterns = discovery.detect_patterns(signals).await?;
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod academic_clients;
pub mod api_clients;
pub mod arxiv_client;
pub mod biorxiv_client;
pub mod coherence;
pub mod crossref_client;
pub mod discovery;
pub mod dynamic_mincut;
pub mod economic_clients;
pub mod export;
pub mod finance_clients;
pub mod forecasting;
pub mod genomics_clients;
pub mod geospatial_clients;
pub mod government_clients;
pub mod hnsw;
pub mod cut_aware_hnsw;
pub mod ingester;
pub mod mcp_server;
pub mod medical_clients;
pub mod ml_clients;
pub mod news_clients;
pub mod optimized;
pub mod patent_clients;
pub mod persistence;
pub mod physics_clients;
pub mod realtime;
pub mod ruvector_native;
pub mod semantic_scholar;
pub mod space_clients;
pub mod streaming;
pub mod transportation_clients;
pub mod utils;
pub mod visualization;
pub mod wiki_clients;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Re-exports
pub use academic_clients::{CoreClient, EricClient, UnpaywallClient};
pub use api_clients::{EdgarClient, Embedder, NoaaClient, OpenAlexClient, SimpleEmbedder};
#[cfg(feature = "onnx-embeddings")]
pub use api_clients::OnnxEmbedder;
#[cfg(feature = "onnx-embeddings")]
pub use ruvector_onnx_embeddings::{PretrainedModel, EmbedderConfig, PoolingStrategy};
pub use arxiv_client::ArxivClient;
pub use biorxiv_client::{BiorxivClient, MedrxivClient};
pub use crossref_client::CrossRefClient;
pub use economic_clients::{AlphaVantageClient, FredClient, WorldBankClient};
pub use finance_clients::{BlsClient, CoinGeckoClient, EcbClient, FinnhubClient, TwelveDataClient};
pub use genomics_clients::{EnsemblClient, GwasClient, NcbiClient, UniProtClient};
pub use geospatial_clients::{GeonamesClient, NominatimClient, OpenElevationClient, OverpassClient};
pub use government_clients::{
    CensusClient, DataGovClient, EuOpenDataClient, UkGovClient, UNDataClient,
    WorldBankClient as WorldBankGovClient,
};
pub use medical_clients::{ClinicalTrialsClient, FdaClient, PubMedClient};
pub use ml_clients::{
    HuggingFaceClient, HuggingFaceDataset, HuggingFaceModel, OllamaClient, OllamaModel,
    PapersWithCodeClient, PaperWithCodeDataset, PaperWithCodePaper, ReplicateClient,
    ReplicateModel, TogetherAiClient, TogetherModel,
};
pub use news_clients::{GuardianClient, HackerNewsClient, NewsDataClient, RedditClient};
pub use patent_clients::{EpoClient, UsptoPatentClient};
pub use physics_clients::{ArgoClient, CernOpenDataClient, GeoUtils, MaterialsProjectClient, UsgsEarthquakeClient};
pub use semantic_scholar::SemanticScholarClient;
pub use space_clients::{AstronomyClient, ExoplanetClient, NasaClient, SpaceXClient};
pub use transportation_clients::{GtfsClient, MobilityDatabaseClient, OpenChargeMapClient, OpenRouteServiceClient};
pub use wiki_clients::{WikidataClient, WikidataEntity, WikipediaClient};
pub use coherence::{
    CoherenceBoundary, CoherenceConfig, CoherenceEngine, CoherenceEvent, CoherenceSignal,
};
pub use cut_aware_hnsw::{
    CutAwareHNSW, CutAwareConfig, CutAwareMetrics, CoherenceZone,
    SearchResult as CutAwareSearchResult, EdgeUpdate as CutAwareEdgeUpdate, UpdateKind, LayerCutStats,
};
pub use discovery::{
    DiscoveryConfig, DiscoveryEngine, DiscoveryPattern, PatternCategory, PatternStrength,
};
pub use dynamic_mincut::{
    CutGatedSearch, CutWatcherConfig, DynamicCutWatcher, DynamicMinCutError,
    EdgeUpdate as DynamicEdgeUpdate, EdgeUpdateType, EulerTourTree, HNSWGraph,
    LocalCut, LocalMinCutProcedure, WatcherStats,
};
pub use export::{
    export_all, export_coherence_csv, export_dot, export_graphml, export_patterns_csv,
    export_patterns_with_evidence_csv, ExportFilter,
};
pub use forecasting::{CoherenceForecaster, CrossDomainForecaster, Forecast, Trend};
pub use ingester::{DataIngester, IngestionConfig, IngestionStats, SourceConfig};
pub use realtime::{FeedItem, FeedSource, NewsAggregator, NewsSource, RealTimeEngine};
pub use ruvector_native::{
    CoherenceHistoryEntry, CoherenceSnapshot, Domain, DiscoveredPattern,
    GraphExport, NativeDiscoveryEngine, NativeEngineConfig, SemanticVector,
};
pub use streaming::{StreamingConfig, StreamingEngine, StreamingEngineBuilder, StreamingMetrics};

/// Framework error types
#[derive(Error, Debug)]
pub enum FrameworkError {
    /// Data ingestion failed
    #[error("Ingestion error: {0}")]
    Ingestion(String),

    /// Coherence computation failed
    #[error("Coherence error: {0}")]
    Coherence(String),

    /// Discovery algorithm failed
    #[error("Discovery error: {0}")]
    Discovery(String),

    /// Network/API error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Graph operation failed
    #[error("Graph error: {0}")]
    Graph(String),

    /// Configuration error
    #[error("Config error: {0}")]
    Config(String),
}

/// Result type for framework operations
pub type Result<T> = std::result::Result<T, FrameworkError>;

/// A timestamped data record from any source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRecord {
    /// Unique identifier
    pub id: String,

    /// Source dataset (e.g., "openalex", "noaa", "edgar")
    pub source: String,

    /// Record type within source (e.g., "work", "author", "filing")
    pub record_type: String,

    /// Timestamp when data was observed/published
    pub timestamp: DateTime<Utc>,

    /// Raw data payload
    pub data: serde_json::Value,

    /// Pre-computed embedding vector (optional)
    pub embedding: Option<Vec<f32>>,

    /// Relationships to other records
    pub relationships: Vec<Relationship>,
}

/// A relationship between two records
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    /// Target record ID
    pub target_id: String,

    /// Relationship type (e.g., "cites", "authored_by", "filed_by")
    pub rel_type: String,

    /// Relationship weight/strength
    pub weight: f64,

    /// Additional properties
    pub properties: HashMap<String, serde_json::Value>,
}

/// Trait for data sources that can be ingested
#[async_trait]
pub trait DataSource: Send + Sync {
    /// Source identifier
    fn source_id(&self) -> &str;

    /// Fetch a batch of records starting from cursor
    async fn fetch_batch(
        &self,
        cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)>;

    /// Get total record count (if known)
    async fn total_count(&self) -> Result<Option<u64>>;

    /// Check if source is available
    async fn health_check(&self) -> Result<bool>;
}

/// Trait for computing embeddings from records
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Compute embedding for a single record
    async fn embed_record(&self, record: &DataRecord) -> Result<Vec<f32>>;

    /// Compute embeddings for a batch of records
    async fn embed_batch(&self, records: &[DataRecord]) -> Result<Vec<Vec<f32>>>;

    /// Embedding dimension
    fn dimension(&self) -> usize;
}

/// Trait for graph building from records
pub trait GraphBuilder: Send + Sync {
    /// Add a node from a data record
    fn add_node(&mut self, record: &DataRecord) -> Result<u64>;

    /// Add an edge between nodes
    fn add_edge(&mut self, source: u64, target: u64, weight: f64, rel_type: &str) -> Result<()>;

    /// Get node count
    fn node_count(&self) -> usize;

    /// Get edge count
    fn edge_count(&self) -> usize;
}

/// Temporal window for time-series analysis
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TemporalWindow {
    /// Window start
    pub start: DateTime<Utc>,

    /// Window end
    pub end: DateTime<Utc>,

    /// Window identifier (for sliding windows)
    pub window_id: u64,
}

impl TemporalWindow {
    /// Create a new temporal window
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>, window_id: u64) -> Self {
        Self {
            start,
            end,
            window_id,
        }
    }

    /// Duration in seconds
    pub fn duration_secs(&self) -> i64 {
        (self.end - self.start).num_seconds()
    }

    /// Check if timestamp falls within window
    pub fn contains(&self, timestamp: DateTime<Utc>) -> bool {
        timestamp >= self.start && timestamp < self.end
    }
}

/// Statistics for a discovery session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscoveryStats {
    /// Records processed
    pub records_processed: u64,

    /// Nodes in graph
    pub nodes_created: u64,

    /// Edges in graph
    pub edges_created: u64,

    /// Coherence signals computed
    pub signals_computed: u64,

    /// Patterns discovered
    pub patterns_discovered: u64,

    /// Processing duration in milliseconds
    pub duration_ms: u64,

    /// Peak memory usage in bytes
    pub peak_memory_bytes: u64,
}

/// Configuration for the entire discovery pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Ingestion configuration
    pub ingestion: IngestionConfig,

    /// Coherence engine configuration
    pub coherence: CoherenceConfig,

    /// Discovery engine configuration
    pub discovery: DiscoveryConfig,

    /// Enable parallel processing
    pub parallel: bool,

    /// Checkpoint interval (records)
    pub checkpoint_interval: u64,

    /// Output directory for results
    pub output_dir: String,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            ingestion: IngestionConfig::default(),
            coherence: CoherenceConfig::default(),
            discovery: DiscoveryConfig::default(),
            parallel: true,
            checkpoint_interval: 10_000,
            output_dir: "./discovery_output".to_string(),
        }
    }
}

/// Main discovery pipeline orchestrator
pub struct DiscoveryPipeline {
    config: PipelineConfig,
    ingester: DataIngester,
    coherence: CoherenceEngine,
    discovery: DiscoveryEngine,
    stats: Arc<std::sync::RwLock<DiscoveryStats>>,
}

impl DiscoveryPipeline {
    /// Create a new discovery pipeline
    pub fn new(config: PipelineConfig) -> Self {
        let ingester = DataIngester::new(config.ingestion.clone());
        let coherence = CoherenceEngine::new(config.coherence.clone());
        let discovery = DiscoveryEngine::new(config.discovery.clone());

        Self {
            config,
            ingester,
            coherence,
            discovery,
            stats: Arc::new(std::sync::RwLock::new(DiscoveryStats::default())),
        }
    }

    /// Run the discovery pipeline on a data source
    pub async fn run<S: DataSource>(&mut self, source: S) -> Result<Vec<DiscoveryPattern>> {
        let start_time = std::time::Instant::now();

        // Phase 1: Ingest data
        tracing::info!("Starting ingestion from source: {}", source.source_id());
        let records = self.ingester.ingest_all(&source).await?;

        {
            let mut stats = self.stats.write().unwrap();
            stats.records_processed = records.len() as u64;
        }

        // Phase 2: Build graph and compute coherence
        tracing::info!("Computing coherence signals over {} records", records.len());
        let signals = self.coherence.compute_from_records(&records)?;

        {
            let mut stats = self.stats.write().unwrap();
            stats.signals_computed = signals.len() as u64;
            stats.nodes_created = self.coherence.node_count() as u64;
            stats.edges_created = self.coherence.edge_count() as u64;
        }

        // Phase 3: Detect patterns
        tracing::info!("Detecting discovery patterns");
        let patterns = self.discovery.detect(&signals)?;

        {
            let mut stats = self.stats.write().unwrap();
            stats.patterns_discovered = patterns.len() as u64;
            stats.duration_ms = start_time.elapsed().as_millis() as u64;
        }

        tracing::info!(
            "Discovery complete: {} patterns found in {}ms",
            patterns.len(),
            start_time.elapsed().as_millis()
        );

        Ok(patterns)
    }

    /// Get current statistics
    pub fn stats(&self) -> DiscoveryStats {
        self.stats.read().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_window() {
        let start = Utc::now();
        let end = start + chrono::Duration::hours(1);
        let window = TemporalWindow::new(start, end, 1);

        assert_eq!(window.duration_secs(), 3600);
        assert!(window.contains(start + chrono::Duration::minutes(30)));
        assert!(!window.contains(start - chrono::Duration::minutes(1)));
        assert!(!window.contains(end + chrono::Duration::minutes(1)));
    }

    #[test]
    fn test_default_pipeline_config() {
        let config = PipelineConfig::default();
        assert!(config.parallel);
        assert_eq!(config.checkpoint_interval, 10_000);
    }

    #[test]
    fn test_data_record_serialization() {
        let record = DataRecord {
            id: "test-1".to_string(),
            source: "test".to_string(),
            record_type: "document".to_string(),
            timestamp: Utc::now(),
            data: serde_json::json!({"title": "Test"}),
            embedding: Some(vec![0.1, 0.2, 0.3]),
            relationships: vec![],
        };

        let json = serde_json::to_string(&record).unwrap();
        let parsed: DataRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, record.id);
    }
}
