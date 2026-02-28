# ADR-001: System Architecture Overview

**Status:** Accepted
**Date:** 2026-01-15
**Decision Makers:** 7sense Architecture Team
**Technical Area:** System Architecture

---

## Context and Problem Statement

7sense aims to transform bioacoustic signals (primarily bird calls) into a navigable geometric space where meaningful structure emerges. The system must process audio recordings, generate high-dimensional embeddings using Perch 2.0 (1536-D vectors), organize them with HNSW indexing in RuVector, and apply GNN-based learning to surface patterns such as call types, motifs, and behavioral contexts.

The core challenge is designing an architecture that:

1. **Handles diverse data pipelines** - From raw 32kHz audio to queryable vector embeddings
2. **Scales to millions of call segments** - Real-world bioacoustic monitoring generates vast datasets
3. **Supports scientific workflows** - Researchers need reproducibility, transparency, and evidence-backed interpretations (RAB pattern)
4. **Enables real-time and batch processing** - Field deployments require streaming; research requires bulk analysis
5. **Integrates ML inference efficiently** - ONNX-based Perch 2.0 inference in Rust for performance

### Current State

This is a greenfield project building upon:
- **Perch 2.0**: Google DeepMind's bioacoustic embedding model (EfficientNet-B3 backbone, 1536-D output)
- **RuVector**: Rust-based vector database with HNSW indexing and self-learning GNN layers
- **RAB Pattern**: Retrieval-Augmented Bioacoustics for evidence-backed interpretation

---

## Decision Drivers

### Performance Requirements
- **Embedding generation**: Process 5-second audio segments at >100 segments/second
- **Vector search**: Sub-millisecond kNN queries on 1M+ vectors (HNSW target: ~100us)
- **Batch ingestion**: 1M vectors/minute build speed (RuVector baseline)
- **Memory efficiency**: Support 32x compression for cold data tiers

### Scalability Requirements
- **Data volume**: Support 10K to 10M+ call segments per deployment
- **Concurrent users**: Multiple researchers querying simultaneously
- **Geographic distribution**: Sensor networks across multiple sites
- **Temporal depth**: Years of historical recordings

### Scientific Rigor Requirements
- **Reproducibility**: Deterministic pipelines with versioned models and parameters
- **Transparency**: RAB-style evidence packs citing retrieved calls for any interpretation
- **Auditability**: Full provenance tracking from raw audio to conclusions
- **Validation**: Built-in verification against ground truth labels

### Operational Requirements
- **Deployment flexibility**: Edge (sensor), cloud, and hybrid deployments
- **Monitoring**: Health metrics, processing throughput, index quality
- **Updates**: Hot-swap embedding models without full reindexing
- **Recovery**: Graceful degradation and disaster recovery

---

## Considered Options

### Option A: Monolithic Architecture

A single application handling all concerns: audio processing, embedding generation, vector storage, GNN learning, API serving, and visualization.

**Pros:**
- Simplest deployment model
- No inter-service communication overhead
- Single codebase to maintain

**Cons:**
- Cannot scale components independently
- Single point of failure
- Difficult to update individual components
- Memory pressure from co-located ML models
- Not suitable for distributed sensor networks

### Option B: Microservices Architecture

Fully decomposed services: Audio Ingest Service, Embedding Service, Vector Store Service, GNN Learning Service, Query Service, Visualization Service, etc.

**Pros:**
- Independent scaling per service
- Technology flexibility per service
- Fault isolation
- Team parallelization

**Cons:**
- Significant operational complexity
- Network latency between services
- Data consistency challenges
- Overkill for initial team size
- Complex debugging across service boundaries

### Option C: Modular Monolith Architecture

A single deployable unit with clearly separated internal modules, designed for future extraction into services if needed.

**Pros:**
- Maintains deployment simplicity
- Clear module boundaries enable future splitting
- In-process communication for performance-critical paths
- Easier debugging and testing
- Appropriate for current team/project scale
- Can evolve toward microservices as needs emerge

**Cons:**
- Requires discipline to maintain module boundaries
- All modules share the same runtime resources
- Scaling requires scaling the entire application

---

## Decision Outcome

**Chosen Option: Option C - Modular Monolith Architecture**

We adopt a modular monolith architecture with clearly defined domain boundaries, designed with explicit seams that allow future extraction to services. This balances immediate development velocity with long-term architectural flexibility.

### Rationale

1. **Right-sized for current needs**: A small team building a new product benefits from deployment simplicity
2. **Performance-critical paths stay in-process**: Audio-to-embedding-to-index flow benefits from zero network hops
3. **Scientific workflow alignment**: Researchers prefer reproducible, debuggable systems over distributed complexity
4. **Evolution path preserved**: Module boundaries are designed as potential service boundaries
5. **RuVector integration**: RuVector is designed as an embeddable library, making monolith integration natural

---

## Technical Specifications

### Module Architecture

```
sevensense/
├── core/                      # Domain-agnostic foundations
│   ├── config/               # Configuration management
│   ├── error/                # Error types and handling
│   ├── telemetry/            # Logging, metrics, tracing
│   └── storage/              # Abstract storage interfaces
│
├── audio/                     # Audio Processing Domain
│   ├── ingest/               # Audio file reading, streaming
│   ├── segment/              # Call detection and segmentation
│   ├── features/             # Acoustic feature extraction
│   └── spectrogram/          # Mel spectrogram generation
│
├── embedding/                 # Embedding Generation Domain
│   ├── perch/                # Perch 2.0 ONNX inference
│   ├── models/               # Model versioning and registry
│   ├── batch/                # Batch embedding pipelines
│   └── normalize/            # Vector normalization (L2, etc.)
│
├── vectordb/                  # Vector Storage Domain (RuVector)
│   ├── index/                # HNSW index management
│   ├── graph/                # Graph structure (nodes, edges)
│   ├── query/                # Similarity search, Cypher queries
│   └── hyperbolic/           # Poincare ball embeddings
│
├── learning/                  # GNN Learning Domain
│   ├── gnn/                  # GNN layers (GCN, GAT, GraphSAGE)
│   ├── attention/            # Attention mechanisms
│   ├── training/             # Self-supervised training loops
│   └── refinement/           # Embedding refinement pipelines
│
├── analysis/                  # Analysis Domain
│   ├── clustering/           # HDBSCAN, prototype extraction
│   ├── sequence/             # Motif detection, transition analysis
│   ├── entropy/              # Sequence entropy metrics
│   └── validation/           # Ground truth comparison
│
├── rab/                       # Retrieval-Augmented Bioacoustics
│   ├── evidence/             # Evidence pack construction
│   ├── retrieval/            # Adaptive retrieval depth
│   ├── interpretation/       # Constrained interpretation generation
│   └── citation/             # Source attribution
│
├── api/                       # API Layer
│   ├── rest/                 # REST endpoints
│   ├── graphql/              # GraphQL schema and resolvers
│   ├── websocket/            # Real-time streaming
│   └── grpc/                 # gRPC for inter-service (future)
│
├── visualization/             # Visualization Domain
│   ├── projection/           # UMAP/t-SNE dimensionality reduction
│   ├── graph_viz/            # Network visualization
│   ├── spectrogram_viz/      # Spectrogram rendering
│   └── export/               # Export formats (JSON, PNG, etc.)
│
└── cli/                       # Command Line Interface
    ├── ingest/               # Batch ingestion commands
    ├── query/                # Query commands
    ├── train/                # Training commands
    └── export/               # Export commands
```

### Data Model

#### Core Entities (Graph Nodes)

```rust
/// Raw audio recording from a sensor
struct Recording {
    id: Uuid,
    sensor_id: String,
    location: GeoPoint,          // lat, lon, elevation
    start_timestamp: DateTime,
    duration_ms: u32,
    sample_rate: u32,            // 32000 Hz for Perch 2.0
    channels: u8,
    habitat: Option<String>,
    weather: Option<WeatherData>,
    file_path: PathBuf,
    checksum: String,            // SHA-256 for reproducibility
}

/// Detected call segment within a recording
struct CallSegment {
    id: Uuid,
    recording_id: Uuid,
    start_ms: u32,
    end_ms: u32,
    snr_db: f32,                 // Signal-to-noise ratio
    peak_frequency_hz: f32,
    energy: f32,
    detection_confidence: f32,
    detection_method: String,    // "energy_threshold", "whisper_seg", etc.
}

/// Embedding vector for a call segment
struct Embedding {
    id: Uuid,
    segment_id: Uuid,
    model_id: String,            // "perch2_v1.0"
    dimensions: u16,             // 1536 for Perch 2.0
    vector: Vec<f32>,
    normalized: bool,
    created_at: DateTime,
}

/// Cluster prototype (centroid of similar calls)
struct Prototype {
    id: Uuid,
    cluster_id: Uuid,
    centroid_vector: Vec<f32>,
    exemplar_ids: Vec<Uuid>,     // Representative segments
    member_count: u32,
    coherence_score: f32,
}

/// Cluster of similar call segments
struct Cluster {
    id: Uuid,
    method: String,              // "hdbscan", "kmeans", etc.
    parameters: HashMap<String, Value>,
    created_at: DateTime,
    validation_score: Option<f32>,
}

/// Optional taxonomic reference
struct Taxon {
    id: Uuid,
    scientific_name: String,
    common_name: String,
    inat_id: Option<u64>,        // iNaturalist ID
    ebird_code: Option<String>,  // eBird species code
}
```

#### Relationships (Graph Edges)

```rust
/// Recording contains segments
edge HAS_SEGMENT: Recording -> CallSegment

/// Temporal sequence within recording
edge NEXT: CallSegment -> CallSegment {
    delta_ms: u32,               // Time gap between calls
}

/// Acoustic similarity from HNSW
edge SIMILAR: CallSegment -> CallSegment {
    distance: f32,               // Cosine or Euclidean
    rank: u8,                    // kNN rank (1 = nearest)
}

/// Cluster membership
edge ASSIGNED_TO: CallSegment -> Cluster

/// Prototype ownership
edge HAS_PROTOTYPE: Cluster -> Prototype

/// Species identification (when available)
edge IDENTIFIED_AS: CallSegment -> Taxon {
    confidence: f32,
    method: String,              // "manual", "model", "consensus"
}
```

### Processing Pipeline

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         INGESTION PIPELINE                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐          │
│  │  Audio   │───▶│ Segment  │───▶│   Mel    │───▶│ Perch2.0 │          │
│  │  Input   │    │Detection │    │Spectrogram│   │  ONNX    │          │
│  │(32kHz,5s)│    │          │    │(500x128) │    │          │          │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘          │
│       │               │               │               │                  │
│       │               │               │               ▼                  │
│       │               │               │         ┌──────────┐            │
│       │               │               │         │Embedding │            │
│       │               │               │         │ (1536-D) │            │
│       │               │               │         └──────────┘            │
│       │               │               │               │                  │
└───────┼───────────────┼───────────────┼───────────────┼──────────────────┘
        │               │               │               │
        ▼               ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         STORAGE LAYER                                    │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────┐       │
│  │                        RuVector                               │       │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │       │
│  │  │   HNSW      │  │   Graph     │  │   Metadata Store    │  │       │
│  │  │   Index     │  │   Store     │  │   (Recordings,      │  │       │
│  │  │             │  │   (Edges)   │  │    Segments, etc.)  │  │       │
│  │  └─────────────┘  └─────────────┘  └─────────────────────┘  │       │
│  └──────────────────────────────────────────────────────────────┘       │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
        │               │               │               │
        ▼               ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         LEARNING LAYER                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐              │
│  │    GNN       │    │  Attention   │    │  Hyperbolic  │              │
│  │  Reranker    │───▶│   Layers     │───▶│  Refinement  │              │
│  │(GCN/GAT/SAGE)│    │              │    │  (Poincare)  │              │
│  └──────────────┘    └──────────────┘    └──────────────┘              │
│         │                   │                   │                        │
│         └───────────────────┴───────────────────┘                        │
│                             │                                            │
│                             ▼                                            │
│                    ┌──────────────┐                                     │
│                    │   Refined    │                                     │
│                    │  Embeddings  │                                     │
│                    └──────────────┘                                     │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
        │               │               │               │
        ▼               ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         ANALYSIS LAYER                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐ │
│  │Clustering│  │ Sequence │  │ Anomaly  │  │  Entropy │  │   RAB    │ │
│  │(HDBSCAN) │  │  Mining  │  │Detection │  │  Metrics │  │ Evidence │ │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  └──────────┘ │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         API / PRESENTATION                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐ │
│  │   REST   │  │ GraphQL  │  │WebSocket │  │   CLI    │  │   WASM   │ │
│  │   API    │  │   API    │  │(Streaming)│ │          │  │ (Browser)│ │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  └──────────┘ │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Key Interfaces Between Modules

```rust
// Audio -> Embedding interface
trait AudioEmbedder {
    fn embed_segment(&self, audio: &AudioSegment) -> Result<Embedding>;
    fn embed_batch(&self, segments: &[AudioSegment]) -> Result<Vec<Embedding>>;
    fn model_info(&self) -> ModelInfo;
}

// Embedding -> VectorDB interface
trait VectorStore {
    fn insert(&mut self, embedding: &Embedding) -> Result<()>;
    fn search_knn(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>>;
    fn get_neighbors(&self, id: Uuid) -> Result<Vec<Neighbor>>;
    fn build_similarity_edges(&mut self, k: usize) -> Result<usize>;
}

// VectorDB -> Learning interface
trait GraphLearner {
    fn train_step(&mut self, graph: &Graph) -> Result<TrainMetrics>;
    fn refine_embeddings(&self, embeddings: &mut [Embedding]) -> Result<()>;
    fn attention_weights(&self, node_id: Uuid) -> Result<Vec<(Uuid, f32)>>;
}

// Learning -> Analysis interface
trait PatternAnalyzer {
    fn cluster(&self, embeddings: &[Embedding]) -> Result<Vec<Cluster>>;
    fn find_motifs(&self, sequences: &[Sequence]) -> Result<Vec<Motif>>;
    fn compute_entropy(&self, transitions: &TransitionMatrix) -> f32;
}

// Analysis -> RAB interface
trait EvidenceBuilder {
    fn build_pack(&self, query: &Query) -> Result<EvidencePack>;
    fn generate_interpretation(&self, pack: &EvidencePack) -> Result<Interpretation>;
    fn cite_sources(&self, interpretation: &Interpretation) -> Vec<Citation>;
}
```

### Configuration Structure

```yaml
# sevensense.yaml
sevensense:
  # Audio processing settings
  audio:
    sample_rate: 32000          # Perch 2.0 requirement
    segment_duration_ms: 5000   # 5 seconds
    segment_overlap_ms: 500     # Overlap for continuity
    min_snr_db: 10.0           # Minimum signal-to-noise
    detection_method: "energy"  # or "whisper_seg", "tweety"

  # Embedding generation
  embedding:
    model: "perch2_v1.0"
    onnx_path: "./models/perch2.onnx"
    dimensions: 1536
    normalize: true
    batch_size: 32

  # Vector database (RuVector)
  vectordb:
    index_type: "hnsw"
    hnsw:
      m: 16                     # Connections per node
      ef_construction: 200      # Build-time search width
      ef_search: 100           # Query-time search width
    distance_metric: "cosine"   # or "euclidean", "poincare"
    enable_hyperbolic: false    # Experimental
    compression:
      hot_tier: "none"
      warm_tier: "pq_8"        # Product quantization
      cold_tier: "pq_4"        # Aggressive compression

  # GNN learning
  learning:
    enabled: true
    gnn_type: "gat"            # GCN, GAT, or GraphSAGE
    hidden_dim: 256
    num_layers: 2
    attention_heads: 4
    learning_rate: 0.001
    training_interval_hours: 24

  # Analysis settings
  analysis:
    clustering:
      method: "hdbscan"
      min_cluster_size: 10
      min_samples: 5
    sequence:
      max_gap_ms: 2000         # Max silence between calls
      min_motif_length: 3

  # RAB settings
  rab:
    retrieval_k: 10            # Neighbors to retrieve
    min_confidence: 0.7
    cite_exemplars: true

  # API settings
  api:
    host: "0.0.0.0"
    port: 8080
    enable_graphql: true
    enable_websocket: true
    cors_origins: ["*"]

  # Telemetry
  telemetry:
    log_level: "info"
    metrics_port: 9090
    tracing_enabled: true
    tracing_endpoint: "http://localhost:4317"
```

---

## Consequences

### Positive Consequences

1. **Development velocity**: Single deployment simplifies CI/CD and local development
2. **Performance**: Critical audio-to-index path has zero network overhead
3. **Debugging**: Stack traces span the entire flow; no distributed tracing required initially
4. **Testing**: Integration tests run in-process without container orchestration
5. **Scientific reproducibility**: Single binary with pinned dependencies ensures consistent results
6. **Resource efficiency**: Shared memory pools and caches across modules
7. **Evolution path**: Clear module boundaries allow extraction to services when justified

### Negative Consequences

1. **Scaling limitations**: Cannot scale embedding generation independently from query serving
2. **Deployment coupling**: Updates to any module require full redeployment
3. **Resource contention**: GNN training may compete with query serving for CPU/memory
4. **Technology constraints**: All modules must work within Rust ecosystem (mitigated by FFI)

### Mitigation Strategies

| Risk | Mitigation |
|------|------------|
| Scaling limitations | Design async job queues that could become external workers |
| Deployment coupling | Blue-green deployments with health checks |
| Resource contention | Configurable resource limits per module; background training scheduling |
| Technology constraints | ONNX runtime for ML; FFI bindings for specialized libraries |

---

## Related Decisions

- **ADR-002**: Perch 2.0 Integration Strategy (ONNX vs. birdnet-onnx crate)
- **ADR-003**: HNSW vs. Hyperbolic Space Configuration
- **ADR-004**: GNN Training Strategy (Online vs. Batch)
- **ADR-005**: RAB Evidence Pack Schema
- **ADR-006**: API Design (REST/GraphQL/gRPC)

---

## Compliance and Standards

### Scientific Standards
- All embeddings include model version and parameters for reproducibility
- Evidence packs include full retrieval citations per RAB methodology
- Validation metrics align with published benchmarks (V-measure, silhouette scores)

### Data Standards
- Audio metadata follows Darwin Core / TDWG standards where applicable
- Taxonomic references link to iNaturalist and eBird identifiers
- Geospatial data uses WGS84 coordinates

### Security Considerations
- No PII in bioacoustic data (sensor IDs are pseudonymous)
- API authentication via JWT tokens
- Audit logging for all data modifications

---

## References

1. Perch 2.0 Paper: "The Bittern Lesson for Bioacoustics" (arXiv:2508.04665)
2. RuVector Documentation: https://github.com/ruvnet/ruvector
3. HNSW Paper: "Efficient and Robust Approximate Nearest Neighbor Search"
4. RAB Pattern: Retrieval-Augmented Bioacoustics methodology
5. AVN Deep Learning Study: "A deep learning approach for the analysis of birdsong" (eLife 2025)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-15 | 7sense Architecture Team | Initial version |
