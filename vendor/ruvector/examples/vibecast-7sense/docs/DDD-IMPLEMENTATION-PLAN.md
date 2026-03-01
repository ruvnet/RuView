# 7sense DDD Implementation Plan

## Executive Summary

This document defines the Domain-Driven Design implementation plan for 7sense, a bioacoustics platform that transforms audio signals into navigable geometric spaces. The architecture follows hexagonal/clean architecture principles with six bounded contexts communicating through domain events.

---

## 1. Project Structure

```
sevensense/
├── Cargo.toml                          # Workspace manifest
├── crates/
│   ├── sevensense-core/                  # Shared domain primitives
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── types/                  # Value objects (AudioId, EmbeddingId, etc.)
│   │   │   ├── events/                 # Domain event definitions
│   │   │   ├── errors/                 # Domain error types
│   │   │   └── traits/                 # Shared trait definitions
│   │   └── Cargo.toml
│   │
│   ├── sevensense-audio/                 # Audio Ingestion Context
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── domain/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── aggregates/         # Recording, CallSegment
│   │   │   │   ├── entities/           # AudioSource, Sensor
│   │   │   │   ├── value_objects/      # SampleRate, Duration, AudioFormat
│   │   │   │   ├── events.rs           # AudioIngested, SegmentDetected
│   │   │   │   └── services.rs         # SegmentationService
│   │   │   ├── application/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── commands/           # IngestAudio, DetectSegments
│   │   │   │   ├── queries/            # GetRecording, ListSegments
│   │   │   │   └── handlers.rs
│   │   │   ├── infrastructure/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── repositories/       # PostgreSQL, file storage
│   │   │   │   ├── audio_decoder.rs    # symphonia integration
│   │   │   │   └── segmentation/       # WhisperSeg, energy-based
│   │   │   └── ports/
│   │   │       ├── mod.rs
│   │   │       ├── inbound.rs          # API traits
│   │   │       └── outbound.rs         # Repository traits
│   │   └── Cargo.toml
│   │
│   ├── sevensense-embedding/             # Embedding Context
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── domain/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── aggregates/         # EmbeddingJob, EmbeddingBatch
│   │   │   │   ├── entities/           # Embedding, ModelVersion
│   │   │   │   ├── value_objects/      # EmbeddingVector, ModelConfig
│   │   │   │   ├── events.rs           # EmbeddingGenerated, BatchCompleted
│   │   │   │   └── services.rs         # EmbeddingNormalization
│   │   │   ├── application/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── commands/           # GenerateEmbedding, BatchEmbed
│   │   │   │   ├── queries/            # GetEmbedding, CompareEmbeddings
│   │   │   │   └── handlers.rs
│   │   │   ├── infrastructure/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── repositories/
│   │   │   │   └── models/             # ONNX runtime, Perch adapter
│   │   │   └── ports/
│   │   │       ├── mod.rs
│   │   │       ├── inbound.rs
│   │   │       └── outbound.rs
│   │   └── Cargo.toml
│   │
│   ├── sevensense-vector/                # Vector Space Context
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── domain/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── aggregates/         # VectorIndex, NeighborGraph
│   │   │   │   ├── entities/           # IndexedVector, SimilarityEdge
│   │   │   │   ├── value_objects/      # Distance, HNSWConfig
│   │   │   │   ├── events.rs           # VectorIndexed, NeighborsFound
│   │   │   │   └── services.rs         # SimilaritySearch, GraphBuilder
│   │   │   ├── application/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── commands/           # IndexVector, RebuildGraph
│   │   │   │   ├── queries/            # FindNeighbors, GetCluster
│   │   │   │   └── handlers.rs
│   │   │   ├── infrastructure/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── repositories/
│   │   │   │   └── hnsw/               # HNSW implementation
│   │   │   └── ports/
│   │   │       ├── mod.rs
│   │   │       ├── inbound.rs
│   │   │       └── outbound.rs
│   │   └── Cargo.toml
│   │
│   ├── sevensense-learning/              # Learning Context
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── domain/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── aggregates/         # LearningSession, TrainingRun
│   │   │   │   ├── entities/           # GNNModel, AttentionWeights
│   │   │   │   ├── value_objects/      # LearningRate, LossValue
│   │   │   │   ├── events.rs           # ModelTrained, WeightsUpdated
│   │   │   │   └── services.rs         # ContrastiveLearning, GraphAttention
│   │   │   ├── application/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── commands/           # TrainModel, RefineEmbeddings
│   │   │   │   ├── queries/            # GetModelStatus, EvaluateModel
│   │   │   │   └── handlers.rs
│   │   │   ├── infrastructure/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── repositories/
│   │   │   │   └── gnn/                # GNN layers, optimizers
│   │   │   └── ports/
│   │   │       ├── mod.rs
│   │   │       ├── inbound.rs
│   │   │       └── outbound.rs
│   │   └── Cargo.toml
│   │
│   ├── sevensense-analysis/              # Analysis Context
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── domain/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── aggregates/         # AnalysisSession, ClusterSet
│   │   │   │   ├── entities/           # Cluster, Motif, Trajectory
│   │   │   │   ├── value_objects/      # ClusterId, MotifPattern
│   │   │   │   ├── events.rs           # ClustersDiscovered, MotifDetected
│   │   │   │   └── services.rs         # ClusteringService, MotifMining
│   │   │   ├── application/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── commands/           # RunClustering, DetectMotifs
│   │   │   │   ├── queries/            # GetClusters, GetTrajectories
│   │   │   │   └── handlers.rs
│   │   │   ├── infrastructure/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── repositories/
│   │   │   │   └── algorithms/         # HDBSCAN, DTW, TDA
│   │   │   └── ports/
│   │   │       ├── mod.rs
│   │   │       ├── inbound.rs
│   │   │       └── outbound.rs
│   │   └── Cargo.toml
│   │
│   └── sevensense-interpretation/        # Interpretation Context (RAB)
│       ├── src/
│       │   ├── lib.rs
│       │   ├── domain/
│       │   │   ├── mod.rs
│       │   │   ├── aggregates/         # EvidencePack, Interpretation
│       │   │   ├── entities/           # Citation, Hypothesis
│       │   │   ├── value_objects/      # Confidence, StructuralDescription
│       │   │   ├── events.rs           # InterpretationGenerated
│       │   │   └── services.rs         # EvidenceAssembly, RABGeneration
│       │   ├── application/
│       │   │   ├── mod.rs
│       │   │   ├── commands/           # GenerateInterpretation
│       │   │   ├── queries/            # GetEvidencePack
│       │   │   └── handlers.rs
│       │   ├── infrastructure/
│       │   │   ├── mod.rs
│       │   │   ├── repositories/
│       │   │   └── rab/                # RAB engine, citation builder
│       │   └── ports/
│       │       ├── mod.rs
│       │       ├── inbound.rs
│       │       └── outbound.rs
│       └── Cargo.toml
│
├── services/
│   ├── api-gateway/                    # HTTP/gRPC API
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── routes/
│   │   │   ├── middleware/
│   │   │   └── handlers/
│   │   └── Cargo.toml
│   │
│   └── worker/                         # Background processing
│       ├── src/
│       │   ├── main.rs
│       │   ├── jobs/
│       │   └── scheduler/
│       └── Cargo.toml
│
├── apps/
│   ├── cli/                            # Command-line interface
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   └── commands/
│   │   └── Cargo.toml
│   │
│   └── web/                            # WASM web interface
│       ├── src/
│       │   ├── lib.rs
│       │   └── components/
│       └── Cargo.toml
│
├── tests/
│   ├── integration/
│   └── e2e/
│
└── docs/
    ├── architecture/
    └── api/
```

---

## 2. Domain Events

### 2.1 Event Catalog

```rust
// sevensense-core/src/events/mod.rs

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Base trait for all domain events
pub trait DomainEvent: Send + Sync {
    fn event_id(&self) -> Uuid;
    fn occurred_at(&self) -> DateTime<Utc>;
    fn aggregate_id(&self) -> String;
    fn event_type(&self) -> &'static str;
}

// ============================================
// AUDIO INGESTION CONTEXT EVENTS
// ============================================

/// Emitted when raw audio is successfully ingested
#[derive(Clone, Debug)]
pub struct AudioIngested {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub recording_id: RecordingId,
    pub source: AudioSource,
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub channels: u8,
    pub metadata: RecordingMetadata,
}

/// Emitted when a call segment is detected in audio
#[derive(Clone, Debug)]
pub struct SegmentDetected {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub segment_id: SegmentId,
    pub recording_id: RecordingId,
    pub start_ms: u64,
    pub end_ms: u64,
    pub snr: f32,
    pub energy: f32,
}

/// Emitted when segmentation of a recording completes
#[derive(Clone, Debug)]
pub struct SegmentationCompleted {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub recording_id: RecordingId,
    pub segment_count: usize,
    pub method: SegmentationMethod,
}

// ============================================
// EMBEDDING CONTEXT EVENTS
// ============================================

/// Emitted when an embedding is generated for a segment
#[derive(Clone, Debug)]
pub struct EmbeddingGenerated {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub embedding_id: EmbeddingId,
    pub segment_id: SegmentId,
    pub model: ModelVersion,
    pub dimensions: usize,
    pub norm: f32,
}

/// Emitted when a batch embedding job completes
#[derive(Clone, Debug)]
pub struct BatchEmbeddingCompleted {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub batch_id: BatchId,
    pub embedding_count: usize,
    pub failed_count: usize,
    pub duration_ms: u64,
}

// ============================================
// VECTOR SPACE CONTEXT EVENTS
// ============================================

/// Emitted when a vector is indexed in HNSW
#[derive(Clone, Debug)]
pub struct VectorIndexed {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub vector_id: VectorId,
    pub embedding_id: EmbeddingId,
    pub layer_count: usize,
    pub neighbor_count: usize,
}

/// Emitted when neighbors are computed for a vector
#[derive(Clone, Debug)]
pub struct NeighborsComputed {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub vector_id: VectorId,
    pub neighbors: Vec<(VectorId, f32)>, // (id, distance)
    pub k: usize,
}

/// Emitted when similarity edges are created
#[derive(Clone, Debug)]
pub struct SimilarityEdgesCreated {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub source_id: VectorId,
    pub edges: Vec<SimilarityEdge>,
}

// ============================================
// LEARNING CONTEXT EVENTS
// ============================================

/// Emitted when GNN training iteration completes
#[derive(Clone, Debug)]
pub struct TrainingIterationCompleted {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub session_id: SessionId,
    pub iteration: u64,
    pub loss: f32,
    pub learning_rate: f32,
}

/// Emitted when embeddings are refined by GNN
#[derive(Clone, Debug)]
pub struct EmbeddingsRefined {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub session_id: SessionId,
    pub refined_count: usize,
    pub avg_shift: f32,
}

/// Emitted when attention weights are updated
#[derive(Clone, Debug)]
pub struct AttentionWeightsUpdated {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub model_id: ModelId,
    pub layer: String,
    pub sparsity: f32,
}

// ============================================
// ANALYSIS CONTEXT EVENTS
// ============================================

/// Emitted when clusters are discovered
#[derive(Clone, Debug)]
pub struct ClustersDiscovered {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub session_id: AnalysisSessionId,
    pub cluster_count: usize,
    pub noise_count: usize,
    pub method: ClusteringMethod,
}

/// Emitted when a motif pattern is detected
#[derive(Clone, Debug)]
pub struct MotifDetected {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub motif_id: MotifId,
    pub pattern: Vec<ClusterId>,
    pub occurrences: usize,
    pub confidence: f32,
}

/// Emitted when a trajectory is identified
#[derive(Clone, Debug)]
pub struct TrajectoryIdentified {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub trajectory_id: TrajectoryId,
    pub segments: Vec<SegmentId>,
    pub entropy: f32,
}

// ============================================
// INTERPRETATION CONTEXT EVENTS
// ============================================

/// Emitted when an evidence pack is assembled
#[derive(Clone, Debug)]
pub struct EvidencePackAssembled {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub pack_id: EvidencePackId,
    pub query_segment_id: SegmentId,
    pub neighbor_count: usize,
    pub exemplar_count: usize,
}

/// Emitted when interpretation is generated
#[derive(Clone, Debug)]
pub struct InterpretationGenerated {
    pub event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub interpretation_id: InterpretationId,
    pub evidence_pack_id: EvidencePackId,
    pub citation_count: usize,
    pub confidence: f32,
}
```

### 2.2 Event Flow Diagram

```
┌─────────────────┐     AudioIngested      ┌─────────────────┐
│  Audio Ingest   │ ─────────────────────► │    Embedding    │
│    Context      │                        │     Context     │
└────────┬────────┘                        └────────┬────────┘
         │                                          │
         │ SegmentDetected                          │ EmbeddingGenerated
         │                                          │
         ▼                                          ▼
┌─────────────────┐                        ┌─────────────────┐
│    Analysis     │ ◄──────────────────────│  Vector Space   │
│    Context      │    VectorIndexed       │     Context     │
└────────┬────────┘    NeighborsComputed   └────────┬────────┘
         │                                          │
         │ ClustersDiscovered                       │
         │ MotifDetected                            │
         ▼                                          │
┌─────────────────┐                                 │
│   Learning      │ ◄───────────────────────────────┘
│    Context      │    SimilarityEdgesCreated
└────────┬────────┘
         │
         │ EmbeddingsRefined
         │
         ▼
┌─────────────────┐
│ Interpretation  │
│    Context      │
└─────────────────┘
```

### 2.3 Event Bus Implementation

```rust
// sevensense-core/src/events/bus.rs

use async_trait::async_trait;
use tokio::sync::broadcast;

#[async_trait]
pub trait EventPublisher: Send + Sync {
    async fn publish<E: DomainEvent + 'static>(&self, event: E) -> Result<(), EventError>;
}

#[async_trait]
pub trait EventSubscriber: Send + Sync {
    async fn subscribe<E: DomainEvent + 'static>(
        &self,
        handler: Box<dyn EventHandler<E>>,
    ) -> Result<SubscriptionId, EventError>;

    async fn unsubscribe(&self, id: SubscriptionId) -> Result<(), EventError>;
}

#[async_trait]
pub trait EventHandler<E: DomainEvent>: Send + Sync {
    async fn handle(&self, event: &E) -> Result<(), EventError>;
}

/// In-memory event bus for single-process deployment
pub struct InMemoryEventBus {
    sender: broadcast::Sender<Box<dyn DomainEvent>>,
}

/// Distributed event bus using NATS or Redis Streams
pub struct DistributedEventBus {
    // Connection to message broker
}
```

---

## 3. Aggregate Roots Per Context

### 3.1 Audio Ingestion Context

```rust
// sevensense-audio/src/domain/aggregates/recording.rs

/// Recording is the aggregate root for audio ingestion
#[derive(Debug)]
pub struct Recording {
    id: RecordingId,
    source: AudioSource,
    metadata: RecordingMetadata,
    segments: Vec<CallSegment>,
    status: RecordingStatus,
    created_at: DateTime<Utc>,

    // Domain events to be published
    events: Vec<Box<dyn DomainEvent>>,
}

impl Recording {
    /// Factory method - validates invariants
    pub fn create(
        id: RecordingId,
        source: AudioSource,
        audio_data: &[u8],
        metadata: RecordingMetadata,
    ) -> Result<Self, AudioDomainError> {
        // Validate: audio must be 32kHz mono for Perch compatibility
        if metadata.sample_rate != 32000 {
            return Err(AudioDomainError::InvalidSampleRate(metadata.sample_rate));
        }

        let mut recording = Self {
            id,
            source,
            metadata,
            segments: Vec::new(),
            status: RecordingStatus::Ingested,
            created_at: Utc::now(),
            events: Vec::new(),
        };

        recording.events.push(Box::new(AudioIngested {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            recording_id: id,
            source,
            duration_ms: metadata.duration_ms,
            sample_rate: metadata.sample_rate,
            channels: metadata.channels,
            metadata: metadata.clone(),
        }));

        Ok(recording)
    }

    /// Add detected segment - enforces business rules
    pub fn add_segment(&mut self, segment: CallSegment) -> Result<(), AudioDomainError> {
        // Invariant: segments must not overlap
        for existing in &self.segments {
            if segment.overlaps(existing) {
                return Err(AudioDomainError::OverlappingSegments);
            }
        }

        // Invariant: segment must be within recording bounds
        if segment.end_ms > self.metadata.duration_ms {
            return Err(AudioDomainError::SegmentOutOfBounds);
        }

        self.events.push(Box::new(SegmentDetected {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            segment_id: segment.id,
            recording_id: self.id,
            start_ms: segment.start_ms,
            end_ms: segment.end_ms,
            snr: segment.snr,
            energy: segment.energy,
        }));

        self.segments.push(segment);
        Ok(())
    }

    /// Complete segmentation
    pub fn complete_segmentation(&mut self, method: SegmentationMethod) {
        self.status = RecordingStatus::Segmented;
        self.events.push(Box::new(SegmentationCompleted {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            recording_id: self.id,
            segment_count: self.segments.len(),
            method,
        }));
    }

    /// Drain and return pending events
    pub fn take_events(&mut self) -> Vec<Box<dyn DomainEvent>> {
        std::mem::take(&mut self.events)
    }
}

/// CallSegment entity within Recording aggregate
#[derive(Debug, Clone)]
pub struct CallSegment {
    pub id: SegmentId,
    pub start_ms: u64,
    pub end_ms: u64,
    pub snr: f32,
    pub energy: f32,
    pub features: SegmentFeatures,
}
```

### 3.2 Embedding Context

```rust
// sevensense-embedding/src/domain/aggregates/embedding_job.rs

/// EmbeddingJob is the aggregate root for embedding generation
#[derive(Debug)]
pub struct EmbeddingJob {
    id: JobId,
    segments: Vec<SegmentId>,
    model: ModelVersion,
    status: JobStatus,
    embeddings: Vec<Embedding>,
    config: EmbeddingConfig,
    events: Vec<Box<dyn DomainEvent>>,
}

impl EmbeddingJob {
    pub fn create(
        id: JobId,
        segments: Vec<SegmentId>,
        model: ModelVersion,
        config: EmbeddingConfig,
    ) -> Result<Self, EmbeddingDomainError> {
        // Validate model compatibility
        if !model.supports_dimensions(config.target_dimensions) {
            return Err(EmbeddingDomainError::IncompatibleDimensions);
        }

        Ok(Self {
            id,
            segments,
            model,
            status: JobStatus::Pending,
            embeddings: Vec::new(),
            config,
            events: Vec::new(),
        })
    }

    /// Process a segment and generate embedding
    pub fn process_segment(
        &mut self,
        segment_id: SegmentId,
        vector: Vec<f32>,
    ) -> Result<EmbeddingId, EmbeddingDomainError> {
        // Validate: vector dimensions must match model output (1536 for Perch 2.0)
        if vector.len() != self.model.output_dimensions() {
            return Err(EmbeddingDomainError::DimensionMismatch {
                expected: self.model.output_dimensions(),
                actual: vector.len(),
            });
        }

        // Validate: no NaN or Inf values
        if vector.iter().any(|v| v.is_nan() || v.is_infinite()) {
            return Err(EmbeddingDomainError::InvalidVector);
        }

        let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        let embedding_id = EmbeddingId::new();

        let embedding = Embedding {
            id: embedding_id,
            segment_id,
            vector,
            norm,
            model: self.model.clone(),
        };

        self.events.push(Box::new(EmbeddingGenerated {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            embedding_id,
            segment_id,
            model: self.model.clone(),
            dimensions: self.model.output_dimensions(),
            norm,
        }));

        self.embeddings.push(embedding);
        Ok(embedding_id)
    }

    pub fn complete(&mut self) {
        self.status = JobStatus::Completed;
        self.events.push(Box::new(BatchEmbeddingCompleted {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            batch_id: BatchId::from(self.id),
            embedding_count: self.embeddings.len(),
            failed_count: self.segments.len() - self.embeddings.len(),
            duration_ms: 0, // Set by caller
        }));
    }
}
```

### 3.3 Vector Space Context

```rust
// sevensense-vector/src/domain/aggregates/vector_index.rs

/// VectorIndex is the aggregate root for vector space operations
#[derive(Debug)]
pub struct VectorIndex {
    id: IndexId,
    config: HNSWConfig,
    vectors: HashMap<VectorId, IndexedVector>,
    graph: NeighborGraph,
    stats: IndexStats,
    events: Vec<Box<dyn DomainEvent>>,
}

impl VectorIndex {
    pub fn create(id: IndexId, config: HNSWConfig) -> Self {
        Self {
            id,
            config,
            vectors: HashMap::new(),
            graph: NeighborGraph::new(config.m, config.ef_construction),
            stats: IndexStats::default(),
            events: Vec::new(),
        }
    }

    /// Index a new vector
    pub fn index_vector(
        &mut self,
        embedding_id: EmbeddingId,
        vector: Vec<f32>,
    ) -> Result<VectorId, VectorDomainError> {
        let vector_id = VectorId::new();

        // Insert into HNSW graph
        let layer_count = self.graph.insert(&vector, vector_id)?;
        let neighbors = self.graph.get_neighbors(vector_id, self.config.k);

        let indexed = IndexedVector {
            id: vector_id,
            embedding_id,
            vector,
            layer: layer_count,
        };

        self.vectors.insert(vector_id, indexed);
        self.stats.vector_count += 1;

        self.events.push(Box::new(VectorIndexed {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            vector_id,
            embedding_id,
            layer_count,
            neighbor_count: neighbors.len(),
        }));

        // Create similarity edges
        let edges: Vec<SimilarityEdge> = neighbors
            .iter()
            .map(|(neighbor_id, distance)| SimilarityEdge {
                source: vector_id,
                target: *neighbor_id,
                distance: *distance,
                edge_type: EdgeType::Acoustic,
            })
            .collect();

        if !edges.is_empty() {
            self.events.push(Box::new(SimilarityEdgesCreated {
                event_id: Uuid::new_v4(),
                occurred_at: Utc::now(),
                source_id: vector_id,
                edges,
            }));
        }

        Ok(vector_id)
    }

    /// Query k-nearest neighbors
    pub fn query_neighbors(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(VectorId, f32)>, VectorDomainError> {
        self.graph.search(query, k)
    }
}
```

### 3.4 Learning Context

```rust
// sevensense-learning/src/domain/aggregates/learning_session.rs

/// LearningSession is the aggregate root for GNN training
#[derive(Debug)]
pub struct LearningSession {
    id: SessionId,
    model: GNNModel,
    config: TrainingConfig,
    status: SessionStatus,
    iterations: Vec<TrainingIteration>,
    events: Vec<Box<dyn DomainEvent>>,
}

impl LearningSession {
    pub fn create(
        id: SessionId,
        model: GNNModel,
        config: TrainingConfig,
    ) -> Result<Self, LearningDomainError> {
        // Validate learning rate bounds
        if config.learning_rate <= 0.0 || config.learning_rate > 1.0 {
            return Err(LearningDomainError::InvalidLearningRate);
        }

        Ok(Self {
            id,
            model,
            config,
            status: SessionStatus::Ready,
            iterations: Vec::new(),
            events: Vec::new(),
        })
    }

    /// Execute one training iteration
    pub fn train_iteration(
        &mut self,
        graph_batch: &GraphBatch,
    ) -> Result<f32, LearningDomainError> {
        self.status = SessionStatus::Training;

        // Forward pass through GNN layers
        let embeddings = self.model.forward(graph_batch)?;

        // Compute contrastive loss (InfoNCE)
        let loss = self.model.compute_loss(&embeddings, graph_batch)?;

        // Backward pass
        self.model.backward(loss)?;

        // Update weights
        self.model.update_weights(self.config.learning_rate)?;

        let iteration = TrainingIteration {
            number: self.iterations.len() as u64 + 1,
            loss,
            learning_rate: self.config.learning_rate,
            timestamp: Utc::now(),
        };

        self.events.push(Box::new(TrainingIterationCompleted {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            session_id: self.id,
            iteration: iteration.number,
            loss,
            learning_rate: self.config.learning_rate,
        }));

        self.iterations.push(iteration);
        Ok(loss)
    }

    /// Refine embeddings using trained model
    pub fn refine_embeddings(
        &mut self,
        embeddings: &mut [Embedding],
    ) -> Result<RefinementStats, LearningDomainError> {
        let mut total_shift = 0.0;

        for embedding in embeddings.iter_mut() {
            let refined = self.model.refine(&embedding.vector)?;
            let shift = cosine_distance(&embedding.vector, &refined);
            total_shift += shift;
            embedding.vector = refined;
        }

        let avg_shift = total_shift / embeddings.len() as f32;

        self.events.push(Box::new(EmbeddingsRefined {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            session_id: self.id,
            refined_count: embeddings.len(),
            avg_shift,
        }));

        Ok(RefinementStats {
            count: embeddings.len(),
            avg_shift,
        })
    }
}
```

### 3.5 Analysis Context

```rust
// sevensense-analysis/src/domain/aggregates/analysis_session.rs

/// AnalysisSession is the aggregate root for pattern discovery
#[derive(Debug)]
pub struct AnalysisSession {
    id: AnalysisSessionId,
    clusters: Vec<Cluster>,
    motifs: Vec<Motif>,
    trajectories: Vec<Trajectory>,
    status: AnalysisStatus,
    events: Vec<Box<dyn DomainEvent>>,
}

impl AnalysisSession {
    /// Run clustering algorithm on embeddings
    pub fn run_clustering(
        &mut self,
        embeddings: &[Embedding],
        config: ClusteringConfig,
    ) -> Result<ClusteringResult, AnalysisDomainError> {
        let method = config.method.clone();

        // Run HDBSCAN or other clustering
        let (clusters, noise) = match &config.method {
            ClusteringMethod::HDBSCAN { min_cluster_size, min_samples } => {
                self.hdbscan_cluster(embeddings, *min_cluster_size, *min_samples)?
            }
            ClusteringMethod::KMeans { k } => {
                self.kmeans_cluster(embeddings, *k)?
            }
        };

        self.clusters = clusters.clone();

        self.events.push(Box::new(ClustersDiscovered {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            session_id: self.id,
            cluster_count: clusters.len(),
            noise_count: noise.len(),
            method,
        }));

        Ok(ClusteringResult { clusters, noise })
    }

    /// Detect motif patterns in sequences
    pub fn detect_motifs(
        &mut self,
        sequences: &[Sequence],
        config: MotifConfig,
    ) -> Result<Vec<Motif>, AnalysisDomainError> {
        let mut detected_motifs = Vec::new();

        // Build transition matrix
        let transitions = self.build_transition_matrix(sequences)?;

        // Find frequently occurring patterns
        for pattern in self.find_frequent_patterns(&transitions, config.min_support)? {
            let occurrences = self.count_occurrences(&pattern, sequences);
            let confidence = occurrences as f32 / sequences.len() as f32;

            if confidence >= config.min_confidence {
                let motif_id = MotifId::new();
                let motif = Motif {
                    id: motif_id,
                    pattern: pattern.clone(),
                    occurrences,
                    confidence,
                };

                self.events.push(Box::new(MotifDetected {
                    event_id: Uuid::new_v4(),
                    occurred_at: Utc::now(),
                    motif_id,
                    pattern,
                    occurrences,
                    confidence,
                }));

                detected_motifs.push(motif);
            }
        }

        self.motifs = detected_motifs.clone();
        Ok(detected_motifs)
    }
}
```

### 3.6 Interpretation Context

```rust
// sevensense-interpretation/src/domain/aggregates/evidence_pack.rs

/// EvidencePack is the aggregate root for RAB interpretation
#[derive(Debug)]
pub struct EvidencePack {
    id: EvidencePackId,
    query_segment: SegmentId,
    neighbors: Vec<NeighborEvidence>,
    exemplars: Vec<ExemplarEvidence>,
    sequence_context: SequenceContext,
    signal_quality: SignalQuality,
    interpretations: Vec<Interpretation>,
    events: Vec<Box<dyn DomainEvent>>,
}

impl EvidencePack {
    /// Assemble evidence pack for a query segment
    pub fn assemble(
        query_segment: SegmentId,
        neighbors: Vec<(SegmentId, f32)>,
        exemplars: Vec<Exemplar>,
        context: SequenceContext,
        quality: SignalQuality,
    ) -> Result<Self, InterpretationDomainError> {
        let id = EvidencePackId::new();

        let neighbor_evidence: Vec<NeighborEvidence> = neighbors
            .into_iter()
            .map(|(seg_id, distance)| NeighborEvidence {
                segment_id: seg_id,
                distance,
                relevance: 1.0 / (1.0 + distance), // Higher relevance for closer neighbors
            })
            .collect();

        let exemplar_evidence: Vec<ExemplarEvidence> = exemplars
            .into_iter()
            .map(|ex| ExemplarEvidence {
                exemplar_id: ex.id,
                cluster_id: ex.cluster_id,
                similarity: ex.similarity,
            })
            .collect();

        let mut pack = Self {
            id,
            query_segment,
            neighbors: neighbor_evidence.clone(),
            exemplars: exemplar_evidence.clone(),
            sequence_context: context,
            signal_quality: quality,
            interpretations: Vec::new(),
            events: Vec::new(),
        };

        pack.events.push(Box::new(EvidencePackAssembled {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            pack_id: id,
            query_segment_id: query_segment,
            neighbor_count: neighbor_evidence.len(),
            exemplar_count: exemplar_evidence.len(),
        }));

        Ok(pack)
    }

    /// Generate interpretation with citations (RAB approach)
    pub fn generate_interpretation(
        &mut self,
        generator: &dyn InterpretationGenerator,
    ) -> Result<InterpretationId, InterpretationDomainError> {
        let interpretation_id = InterpretationId::new();

        // Build structural description from evidence
        let description = generator.describe_structure(
            &self.neighbors,
            &self.exemplars,
            &self.sequence_context,
        )?;

        // Generate citations for each claim
        let citations: Vec<Citation> = self.build_citations(&description)?;

        // Compute confidence based on evidence quality
        let confidence = self.compute_confidence(&citations)?;

        let interpretation = Interpretation {
            id: interpretation_id,
            description,
            citations: citations.clone(),
            confidence,
            generated_at: Utc::now(),
        };

        self.events.push(Box::new(InterpretationGenerated {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            interpretation_id,
            evidence_pack_id: self.id,
            citation_count: citations.len(),
            confidence,
        }));

        self.interpretations.push(interpretation);
        Ok(interpretation_id)
    }

    /// Build citations linking claims to evidence
    fn build_citations(&self, description: &StructuralDescription) -> Result<Vec<Citation>, InterpretationDomainError> {
        let mut citations = Vec::new();

        // Cite neighbor similarities
        for claim in &description.similarity_claims {
            for neighbor in &self.neighbors {
                if neighbor.relevance > 0.5 {
                    citations.push(Citation {
                        claim: claim.clone(),
                        evidence_type: EvidenceType::Neighbor,
                        evidence_id: neighbor.segment_id.to_string(),
                        strength: neighbor.relevance,
                    });
                }
            }
        }

        // Cite cluster membership
        for claim in &description.cluster_claims {
            for exemplar in &self.exemplars {
                if exemplar.similarity > 0.7 {
                    citations.push(Citation {
                        claim: claim.clone(),
                        evidence_type: EvidenceType::Exemplar,
                        evidence_id: exemplar.exemplar_id.to_string(),
                        strength: exemplar.similarity,
                    });
                }
            }
        }

        Ok(citations)
    }
}
```

---

## 4. Repository Interfaces

```rust
// sevensense-core/src/traits/repository.rs

use async_trait::async_trait;

/// Generic repository trait for aggregate persistence
#[async_trait]
pub trait Repository<T, Id>: Send + Sync {
    async fn find_by_id(&self, id: &Id) -> Result<Option<T>, RepositoryError>;
    async fn save(&self, aggregate: &T) -> Result<(), RepositoryError>;
    async fn delete(&self, id: &Id) -> Result<(), RepositoryError>;
}

// ============================================
// AUDIO CONTEXT REPOSITORIES
// ============================================

#[async_trait]
pub trait RecordingRepository: Repository<Recording, RecordingId> {
    async fn find_by_source(&self, source: &AudioSource) -> Result<Vec<Recording>, RepositoryError>;
    async fn find_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Recording>, RepositoryError>;
    async fn find_with_segments(&self, id: &RecordingId) -> Result<Option<Recording>, RepositoryError>;
}

#[async_trait]
pub trait SegmentRepository: Repository<CallSegment, SegmentId> {
    async fn find_by_recording(&self, recording_id: &RecordingId) -> Result<Vec<CallSegment>, RepositoryError>;
    async fn find_by_time_range(
        &self,
        recording_id: &RecordingId,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<CallSegment>, RepositoryError>;
}

// ============================================
// EMBEDDING CONTEXT REPOSITORIES
// ============================================

#[async_trait]
pub trait EmbeddingRepository: Repository<Embedding, EmbeddingId> {
    async fn find_by_segment(&self, segment_id: &SegmentId) -> Result<Option<Embedding>, RepositoryError>;
    async fn find_by_model(&self, model: &ModelVersion) -> Result<Vec<Embedding>, RepositoryError>;
    async fn batch_save(&self, embeddings: &[Embedding]) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait EmbeddingJobRepository: Repository<EmbeddingJob, JobId> {
    async fn find_pending(&self, limit: usize) -> Result<Vec<EmbeddingJob>, RepositoryError>;
    async fn find_by_status(&self, status: JobStatus) -> Result<Vec<EmbeddingJob>, RepositoryError>;
}

// ============================================
// VECTOR SPACE CONTEXT REPOSITORIES
// ============================================

#[async_trait]
pub trait VectorIndexRepository: Repository<VectorIndex, IndexId> {
    async fn find_active(&self) -> Result<Option<VectorIndex>, RepositoryError>;
    async fn snapshot(&self, index: &VectorIndex) -> Result<SnapshotId, RepositoryError>;
    async fn restore(&self, snapshot_id: &SnapshotId) -> Result<VectorIndex, RepositoryError>;
}

#[async_trait]
pub trait SimilarityEdgeRepository {
    async fn find_edges(&self, vector_id: &VectorId) -> Result<Vec<SimilarityEdge>, RepositoryError>;
    async fn find_by_type(
        &self,
        vector_id: &VectorId,
        edge_type: EdgeType,
    ) -> Result<Vec<SimilarityEdge>, RepositoryError>;
    async fn batch_save(&self, edges: &[SimilarityEdge]) -> Result<(), RepositoryError>;
}

// ============================================
// LEARNING CONTEXT REPOSITORIES
// ============================================

#[async_trait]
pub trait LearningSessionRepository: Repository<LearningSession, SessionId> {
    async fn find_active(&self) -> Result<Option<LearningSession>, RepositoryError>;
    async fn find_by_model(&self, model_id: &ModelId) -> Result<Vec<LearningSession>, RepositoryError>;
}

#[async_trait]
pub trait GNNModelRepository: Repository<GNNModel, ModelId> {
    async fn find_latest(&self) -> Result<Option<GNNModel>, RepositoryError>;
    async fn save_checkpoint(&self, model: &GNNModel) -> Result<CheckpointId, RepositoryError>;
    async fn load_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<GNNModel, RepositoryError>;
}

// ============================================
// ANALYSIS CONTEXT REPOSITORIES
// ============================================

#[async_trait]
pub trait ClusterRepository: Repository<Cluster, ClusterId> {
    async fn find_by_session(&self, session_id: &AnalysisSessionId) -> Result<Vec<Cluster>, RepositoryError>;
    async fn find_containing(&self, segment_id: &SegmentId) -> Result<Option<Cluster>, RepositoryError>;
}

#[async_trait]
pub trait MotifRepository: Repository<Motif, MotifId> {
    async fn find_by_pattern(&self, pattern: &[ClusterId]) -> Result<Vec<Motif>, RepositoryError>;
    async fn find_by_confidence(&self, min_confidence: f32) -> Result<Vec<Motif>, RepositoryError>;
}

// ============================================
// INTERPRETATION CONTEXT REPOSITORIES
// ============================================

#[async_trait]
pub trait EvidencePackRepository: Repository<EvidencePack, EvidencePackId> {
    async fn find_by_segment(&self, segment_id: &SegmentId) -> Result<Vec<EvidencePack>, RepositoryError>;
    async fn find_with_interpretations(
        &self,
        pack_id: &EvidencePackId,
    ) -> Result<Option<EvidencePack>, RepositoryError>;
}

#[async_trait]
pub trait InterpretationRepository: Repository<Interpretation, InterpretationId> {
    async fn find_by_evidence_pack(
        &self,
        pack_id: &EvidencePackId,
    ) -> Result<Vec<Interpretation>, RepositoryError>;
    async fn find_by_confidence(&self, min_confidence: f32) -> Result<Vec<Interpretation>, RepositoryError>;
}
```

---

## 5. Application Services

```rust
// sevensense-audio/src/application/services.rs

/// Audio ingestion application service
pub struct AudioIngestionService {
    recording_repo: Arc<dyn RecordingRepository>,
    segment_repo: Arc<dyn SegmentRepository>,
    segmenter: Arc<dyn AudioSegmenter>,
    event_publisher: Arc<dyn EventPublisher>,
}

impl AudioIngestionService {
    /// Ingest audio from a source
    pub async fn ingest_audio(
        &self,
        command: IngestAudioCommand,
    ) -> Result<RecordingId, ApplicationError> {
        // Load audio data
        let audio_data = self.load_audio(&command.source).await?;

        // Create recording aggregate
        let mut recording = Recording::create(
            RecordingId::new(),
            command.source,
            &audio_data,
            command.metadata,
        )?;

        // Run segmentation if requested
        if command.auto_segment {
            let segments = self.segmenter.detect_segments(&audio_data).await?;
            for segment in segments {
                recording.add_segment(segment)?;
            }
            recording.complete_segmentation(SegmentationMethod::WhisperSeg);
        }

        // Persist
        self.recording_repo.save(&recording).await?;

        // Publish events
        for event in recording.take_events() {
            self.event_publisher.publish(event).await?;
        }

        Ok(recording.id())
    }
}

// sevensense-embedding/src/application/services.rs

/// Embedding generation application service
pub struct EmbeddingService {
    embedding_repo: Arc<dyn EmbeddingRepository>,
    job_repo: Arc<dyn EmbeddingJobRepository>,
    model_adapter: Arc<dyn EmbeddingModelAdapter>,
    event_publisher: Arc<dyn EventPublisher>,
}

impl EmbeddingService {
    /// Generate embeddings for segments
    pub async fn generate_embeddings(
        &self,
        command: GenerateEmbeddingsCommand,
    ) -> Result<JobId, ApplicationError> {
        let mut job = EmbeddingJob::create(
            JobId::new(),
            command.segment_ids.clone(),
            command.model,
            command.config,
        )?;

        // Process each segment
        for segment_id in &command.segment_ids {
            // Load audio for segment
            let audio = self.load_segment_audio(segment_id).await?;

            // Generate embedding via model adapter (Perch 2.0)
            let vector = self.model_adapter.embed(&audio).await?;

            job.process_segment(*segment_id, vector)?;
        }

        job.complete();

        // Persist embeddings
        for embedding in job.embeddings() {
            self.embedding_repo.save(embedding).await?;
        }

        // Publish events
        for event in job.take_events() {
            self.event_publisher.publish(event).await?;
        }

        Ok(job.id())
    }
}

// sevensense-vector/src/application/services.rs

/// Vector space management application service
pub struct VectorSpaceService {
    index_repo: Arc<dyn VectorIndexRepository>,
    edge_repo: Arc<dyn SimilarityEdgeRepository>,
    event_publisher: Arc<dyn EventPublisher>,
}

impl VectorSpaceService {
    /// Index embeddings into vector space
    pub async fn index_embeddings(
        &self,
        command: IndexEmbeddingsCommand,
    ) -> Result<usize, ApplicationError> {
        let mut index = self.index_repo.find_active().await?
            .ok_or(ApplicationError::NoActiveIndex)?;

        let mut indexed_count = 0;

        for embedding in &command.embeddings {
            index.index_vector(embedding.id, embedding.vector.clone())?;
            indexed_count += 1;
        }

        // Persist index state
        self.index_repo.save(&index).await?;

        // Publish events
        for event in index.take_events() {
            self.event_publisher.publish(event).await?;
        }

        Ok(indexed_count)
    }

    /// Query nearest neighbors
    pub async fn find_neighbors(
        &self,
        query: FindNeighborsQuery,
    ) -> Result<Vec<NeighborResult>, ApplicationError> {
        let index = self.index_repo.find_active().await?
            .ok_or(ApplicationError::NoActiveIndex)?;

        let neighbors = index.query_neighbors(&query.vector, query.k)?;

        Ok(neighbors
            .into_iter()
            .map(|(id, dist)| NeighborResult { vector_id: id, distance: dist })
            .collect())
    }
}

// sevensense-interpretation/src/application/services.rs

/// RAB interpretation application service
pub struct InterpretationService {
    evidence_repo: Arc<dyn EvidencePackRepository>,
    interpretation_repo: Arc<dyn InterpretationRepository>,
    vector_service: Arc<VectorSpaceService>,
    analysis_service: Arc<AnalysisService>,
    generator: Arc<dyn InterpretationGenerator>,
    event_publisher: Arc<dyn EventPublisher>,
}

impl InterpretationService {
    /// Generate evidence-based interpretation for a segment
    pub async fn interpret_segment(
        &self,
        command: InterpretSegmentCommand,
    ) -> Result<InterpretationId, ApplicationError> {
        // Get neighbors from vector space
        let neighbors = self.vector_service
            .find_neighbors(FindNeighborsQuery {
                vector: command.embedding.clone(),
                k: command.config.neighbor_count,
            })
            .await?;

        // Get cluster exemplars
        let exemplars = self.analysis_service
            .get_cluster_exemplars(command.segment_id)
            .await?;

        // Get sequence context
        let context = self.get_sequence_context(command.segment_id).await?;

        // Get signal quality
        let quality = self.assess_signal_quality(command.segment_id).await?;

        // Assemble evidence pack
        let mut evidence_pack = EvidencePack::assemble(
            command.segment_id,
            neighbors.into_iter().map(|n| (n.segment_id, n.distance)).collect(),
            exemplars,
            context,
            quality,
        )?;

        // Generate interpretation
        let interpretation_id = evidence_pack.generate_interpretation(&*self.generator)?;

        // Persist
        self.evidence_repo.save(&evidence_pack).await?;

        // Publish events
        for event in evidence_pack.take_events() {
            self.event_publisher.publish(event).await?;
        }

        Ok(interpretation_id)
    }
}
```

---

## 6. Anti-Corruption Layers

### 6.1 Perch 2.0 ACL

```rust
// sevensense-embedding/src/infrastructure/models/perch_adapter.rs

use ort::{Session, Environment};

/// Anti-corruption layer for Perch 2.0 ONNX model
pub struct PerchModelAdapter {
    session: Session,
    config: PerchConfig,
}

/// Domain model for Perch configuration
#[derive(Clone, Debug)]
pub struct PerchConfig {
    pub sample_rate: u32,         // Must be 32000
    pub window_samples: usize,    // 160000 (5 seconds)
    pub mel_bins: usize,          // 128
    pub output_dimensions: usize, // 1536
}

impl Default for PerchConfig {
    fn default() -> Self {
        Self {
            sample_rate: 32000,
            window_samples: 160000,
            mel_bins: 128,
            output_dimensions: 1536,
        }
    }
}

/// Perch-specific types that don't leak into domain
mod external {
    pub struct PerchInput {
        pub audio: ndarray::Array2<f32>, // [batch, 160000]
    }

    pub struct PerchOutput {
        pub embedding: ndarray::Array2<f32>,  // [batch, 1536]
        pub spectrogram: ndarray::Array3<f32>, // [batch, 500, 128]
        pub logits: ndarray::Array2<f32>,      // [batch, num_classes]
    }
}

#[async_trait]
impl EmbeddingModelAdapter for PerchModelAdapter {
    /// Transform domain audio to embedding
    async fn embed(&self, audio: &AudioData) -> Result<Vec<f32>, EmbeddingError> {
        // Validate input matches Perch requirements
        if audio.sample_rate != self.config.sample_rate {
            return Err(EmbeddingError::InvalidSampleRate {
                expected: self.config.sample_rate,
                actual: audio.sample_rate,
            });
        }

        // Convert domain audio to Perch input format
        let perch_input = self.to_perch_input(audio)?;

        // Run inference
        let perch_output = self.run_inference(perch_input).await?;

        // Convert Perch output back to domain embedding
        let embedding = self.from_perch_output(perch_output)?;

        Ok(embedding)
    }

    fn model_version(&self) -> ModelVersion {
        ModelVersion {
            name: "perch".to_string(),
            version: "2.0".to_string(),
            output_dimensions: self.config.output_dimensions,
        }
    }
}

impl PerchModelAdapter {
    pub fn new(model_path: &Path) -> Result<Self, EmbeddingError> {
        let environment = Environment::builder()
            .with_name("sevensense")
            .build()?;

        let session = Session::builder()?
            .with_optimization_level(ort::GraphOptimizationLevel::Level3)?
            .with_model_from_file(model_path)?;

        Ok(Self {
            session,
            config: PerchConfig::default(),
        })
    }

    /// Convert domain AudioData to Perch tensor format
    fn to_perch_input(&self, audio: &AudioData) -> Result<external::PerchInput, EmbeddingError> {
        // Ensure mono
        let mono = if audio.channels > 1 {
            audio.to_mono()?
        } else {
            audio.samples.clone()
        };

        // Pad or trim to 5 seconds
        let samples = if mono.len() < self.config.window_samples {
            let mut padded = mono;
            padded.resize(self.config.window_samples, 0.0);
            padded
        } else {
            mono[..self.config.window_samples].to_vec()
        };

        let array = ndarray::Array2::from_shape_vec(
            (1, self.config.window_samples),
            samples,
        )?;

        Ok(external::PerchInput { audio: array })
    }

    /// Convert Perch output to domain embedding
    fn from_perch_output(&self, output: external::PerchOutput) -> Result<Vec<f32>, EmbeddingError> {
        let embedding = output.embedding.row(0).to_vec();

        // Validate dimensions
        if embedding.len() != self.config.output_dimensions {
            return Err(EmbeddingError::DimensionMismatch {
                expected: self.config.output_dimensions,
                actual: embedding.len(),
            });
        }

        // Validate no NaN/Inf
        if embedding.iter().any(|v| v.is_nan() || v.is_infinite()) {
            return Err(EmbeddingError::InvalidEmbeddingValues);
        }

        Ok(embedding)
    }

    async fn run_inference(&self, input: external::PerchInput) -> Result<external::PerchOutput, EmbeddingError> {
        let outputs = self.session.run(ort::inputs!["audio" => input.audio]?)?;

        Ok(external::PerchOutput {
            embedding: outputs["embedding"].extract_tensor()?.to_owned(),
            spectrogram: outputs["spectrogram"].extract_tensor()?.to_owned(),
            logits: outputs["logits"].extract_tensor()?.to_owned(),
        })
    }
}
```

### 6.2 RuVector ACL

```rust
// sevensense-vector/src/infrastructure/ruvector_adapter.rs

use ruvector_core::{VectorDB, HnswIndex, CypherQuery};
use ruvector_gnn::{GnnLayer, GraphAttention};

/// Anti-corruption layer for RuVector integration
pub struct RuVectorAdapter {
    db: VectorDB,
    hnsw_config: RuVectorHnswConfig,
}

/// RuVector-specific configuration (external types)
mod external {
    use ruvector_core::*;

    pub struct RuVectorConfig {
        pub m: usize,
        pub ef_construction: usize,
        pub ef_search: usize,
        pub distance_type: DistanceType,
    }

    pub struct RuVectorNode {
        pub id: String,
        pub embedding: Vec<f32>,
        pub metadata: serde_json::Value,
    }

    pub struct RuVectorEdge {
        pub source: String,
        pub target: String,
        pub distance: f32,
        pub edge_type: String,
    }
}

#[async_trait]
impl VectorIndexAdapter for RuVectorAdapter {
    /// Index a domain vector
    async fn index(&self, vector: &IndexedVector) -> Result<(), VectorError> {
        // Convert domain vector to RuVector node
        let node = self.to_ruvector_node(vector)?;

        // Insert into RuVector
        self.db.insert_node(node).await?;

        Ok(())
    }

    /// Query nearest neighbors
    async fn query_neighbors(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(VectorId, f32)>, VectorError> {
        // Run RuVector HNSW search
        let results = self.db.search(query, k, self.hnsw_config.ef_search).await?;

        // Convert to domain types
        Ok(results
            .into_iter()
            .map(|r| (VectorId::from_str(&r.id).unwrap(), r.distance))
            .collect())
    }

    /// Execute Cypher query for graph operations
    async fn execute_cypher(&self, query: &str) -> Result<CypherResult, VectorError> {
        let ruvector_result = self.db.cypher(query).await?;
        self.from_cypher_result(ruvector_result)
    }
}

impl RuVectorAdapter {
    pub fn new(config: VectorIndexConfig) -> Result<Self, VectorError> {
        let ruvector_config = external::RuVectorConfig {
            m: config.m,
            ef_construction: config.ef_construction,
            ef_search: config.ef_search,
            distance_type: match config.distance {
                DistanceMetric::Cosine => ruvector_core::DistanceType::Cosine,
                DistanceMetric::Euclidean => ruvector_core::DistanceType::Euclidean,
                DistanceMetric::Poincare => ruvector_core::DistanceType::Poincare,
            },
        };

        let db = VectorDB::new(ruvector_config)?;

        Ok(Self {
            db,
            hnsw_config: RuVectorHnswConfig::from(config),
        })
    }

    /// Convert domain vector to RuVector node
    fn to_ruvector_node(&self, vector: &IndexedVector) -> Result<external::RuVectorNode, VectorError> {
        Ok(external::RuVectorNode {
            id: vector.id.to_string(),
            embedding: vector.vector.clone(),
            metadata: serde_json::json!({
                "embedding_id": vector.embedding_id.to_string(),
                "layer": vector.layer,
            }),
        })
    }

    /// Convert RuVector result to domain CypherResult
    fn from_cypher_result(&self, result: ruvector_core::CypherResult) -> Result<CypherResult, VectorError> {
        // Transform external types to domain types
        Ok(CypherResult {
            nodes: result.nodes.into_iter().map(|n| self.to_domain_node(n)).collect(),
            edges: result.edges.into_iter().map(|e| self.to_domain_edge(e)).collect(),
        })
    }
}

/// Adapter for RuVector GNN features
pub struct RuVectorGnnAdapter {
    gnn: ruvector_gnn::GnnModel,
}

#[async_trait]
impl GnnModelAdapter for RuVectorGnnAdapter {
    async fn forward(&self, batch: &GraphBatch) -> Result<Vec<Vec<f32>>, LearningError> {
        // Convert domain batch to RuVector format
        let ruvector_batch = self.to_ruvector_batch(batch)?;

        // Run forward pass
        let output = self.gnn.forward(&ruvector_batch).await?;

        // Convert back to domain
        self.from_ruvector_output(output)
    }

    async fn train_step(
        &self,
        batch: &GraphBatch,
        learning_rate: f32,
    ) -> Result<f32, LearningError> {
        let ruvector_batch = self.to_ruvector_batch(batch)?;
        let loss = self.gnn.train_step(&ruvector_batch, learning_rate).await?;
        Ok(loss)
    }
}
```

---

## 7. Integration Patterns

### 7.1 Event Sourcing

```rust
// sevensense-core/src/events/sourcing.rs

/// Event store for event sourcing
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, stream: &str, events: Vec<Box<dyn DomainEvent>>) -> Result<u64, EventStoreError>;
    async fn read(&self, stream: &str, from: u64) -> Result<Vec<StoredEvent>, EventStoreError>;
    async fn read_all(&self, from: GlobalPosition) -> Result<Vec<StoredEvent>, EventStoreError>;
}

/// Stored event with metadata
#[derive(Debug, Clone)]
pub struct StoredEvent {
    pub position: u64,
    pub global_position: GlobalPosition,
    pub stream: String,
    pub event_type: String,
    pub data: serde_json::Value,
    pub metadata: EventMetadata,
    pub timestamp: DateTime<Utc>,
}

/// Event-sourced aggregate trait
pub trait EventSourcedAggregate: Sized {
    type Event: DomainEvent;

    fn apply(&mut self, event: &Self::Event);
    fn pending_events(&self) -> &[Self::Event];
    fn clear_events(&mut self);

    /// Reconstitute aggregate from event history
    fn from_events(events: impl IntoIterator<Item = Self::Event>) -> Self;
}

/// Example: Event-sourced Recording aggregate
impl EventSourcedAggregate for Recording {
    type Event = AudioDomainEvent;

    fn apply(&mut self, event: &Self::Event) {
        match event {
            AudioDomainEvent::AudioIngested(e) => {
                self.id = e.recording_id;
                self.source = e.source.clone();
                self.status = RecordingStatus::Ingested;
            }
            AudioDomainEvent::SegmentDetected(e) => {
                self.segments.push(CallSegment {
                    id: e.segment_id,
                    start_ms: e.start_ms,
                    end_ms: e.end_ms,
                    snr: e.snr,
                    energy: e.energy,
                    features: SegmentFeatures::default(),
                });
            }
            AudioDomainEvent::SegmentationCompleted(_) => {
                self.status = RecordingStatus::Segmented;
            }
        }
    }

    fn from_events(events: impl IntoIterator<Item = Self::Event>) -> Self {
        let mut recording = Self::default();
        for event in events {
            recording.apply(&event);
        }
        recording
    }

    fn pending_events(&self) -> &[Self::Event] {
        &self.events
    }

    fn clear_events(&mut self) {
        self.events.clear();
    }
}
```

### 7.2 CQRS Pattern

```rust
// sevensense-core/src/cqrs/mod.rs

/// Command trait
pub trait Command: Send + Sync {
    type Result;
}

/// Query trait
pub trait Query: Send + Sync {
    type Result;
}

/// Command handler trait
#[async_trait]
pub trait CommandHandler<C: Command>: Send + Sync {
    async fn handle(&self, command: C) -> Result<C::Result, ApplicationError>;
}

/// Query handler trait
#[async_trait]
pub trait QueryHandler<Q: Query>: Send + Sync {
    async fn handle(&self, query: Q) -> Result<Q::Result, ApplicationError>;
}

/// Command bus for routing commands to handlers
pub struct CommandBus {
    handlers: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl CommandBus {
    pub fn register<C, H>(&mut self, handler: H)
    where
        C: Command + 'static,
        H: CommandHandler<C> + 'static,
    {
        self.handlers.insert(TypeId::of::<C>(), Box::new(handler));
    }

    pub async fn dispatch<C: Command + 'static>(&self, command: C) -> Result<C::Result, ApplicationError> {
        let handler = self.handlers.get(&TypeId::of::<C>())
            .ok_or(ApplicationError::NoHandlerRegistered)?
            .downcast_ref::<Box<dyn CommandHandler<C>>>()
            .unwrap();

        handler.handle(command).await
    }
}

/// Read model for CQRS queries
pub struct ReadModel {
    pool: sqlx::PgPool,
}

impl ReadModel {
    /// Project events to read model
    pub async fn project(&self, event: &StoredEvent) -> Result<(), ProjectionError> {
        match event.event_type.as_str() {
            "AudioIngested" => self.project_audio_ingested(event).await?,
            "SegmentDetected" => self.project_segment_detected(event).await?,
            "EmbeddingGenerated" => self.project_embedding_generated(event).await?,
            "ClustersDiscovered" => self.project_clusters_discovered(event).await?,
            _ => {} // Unknown event types are ignored
        }
        Ok(())
    }

    async fn project_audio_ingested(&self, event: &StoredEvent) -> Result<(), ProjectionError> {
        let data: AudioIngestedData = serde_json::from_value(event.data.clone())?;

        sqlx::query!(
            r#"
            INSERT INTO recordings_read (id, source, duration_ms, sample_rate, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            data.recording_id.to_string(),
            serde_json::to_value(&data.source)?,
            data.duration_ms as i64,
            data.sample_rate as i32,
            event.timestamp
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
```

### 7.3 Saga Pattern for Cross-Context Operations

```rust
// sevensense-core/src/saga/mod.rs

/// Saga for coordinating cross-context operations
#[async_trait]
pub trait Saga: Send + Sync {
    type Context;

    async fn execute(&self, ctx: Self::Context) -> Result<(), SagaError>;
    async fn compensate(&self, ctx: Self::Context) -> Result<(), SagaError>;
}

/// Embedding pipeline saga
/// Coordinates: Audio Ingestion -> Embedding -> Vector Space -> Analysis
pub struct EmbeddingPipelineSaga {
    audio_service: Arc<AudioIngestionService>,
    embedding_service: Arc<EmbeddingService>,
    vector_service: Arc<VectorSpaceService>,
    analysis_service: Arc<AnalysisService>,
}

#[async_trait]
impl Saga for EmbeddingPipelineSaga {
    type Context = EmbeddingPipelineContext;

    async fn execute(&self, ctx: Self::Context) -> Result<(), SagaError> {
        // Step 1: Ingest audio
        let recording_id = self.audio_service
            .ingest_audio(IngestAudioCommand {
                source: ctx.source.clone(),
                metadata: ctx.metadata.clone(),
                auto_segment: true,
            })
            .await
            .map_err(|e| SagaError::StepFailed("ingest", e.into()))?;

        // Step 2: Generate embeddings for all segments
        let segments = self.audio_service.get_segments(recording_id).await?;
        let job_id = self.embedding_service
            .generate_embeddings(GenerateEmbeddingsCommand {
                segment_ids: segments.iter().map(|s| s.id).collect(),
                model: ctx.model,
                config: ctx.embedding_config,
            })
            .await
            .map_err(|e| SagaError::StepFailed("embed", e.into()))?;

        // Step 3: Index embeddings in vector space
        let embeddings = self.embedding_service.get_job_embeddings(job_id).await?;
        self.vector_service
            .index_embeddings(IndexEmbeddingsCommand { embeddings })
            .await
            .map_err(|e| SagaError::StepFailed("index", e.into()))?;

        // Step 4: Run initial analysis (clustering)
        if ctx.auto_cluster {
            self.analysis_service
                .run_clustering(RunClusteringCommand {
                    embedding_ids: embeddings.iter().map(|e| e.id).collect(),
                    config: ctx.clustering_config,
                })
                .await
                .map_err(|e| SagaError::StepFailed("cluster", e.into()))?;
        }

        Ok(())
    }

    async fn compensate(&self, ctx: Self::Context) -> Result<(), SagaError> {
        // Compensating actions in reverse order
        // This is a simplified example - real implementation would track
        // completed steps and only compensate those

        // Remove from vector index
        // Delete embeddings
        // Delete segments
        // Delete recording

        Ok(())
    }
}
```

---

## 8. Testing Strategy

### 8.1 Testing Pyramid

```
                    /\
                   /  \
                  / E2E \        <- 10% - Full system tests
                 /______\
                /        \
               /Integration\     <- 20% - Cross-context tests
              /______________\
             /                \
            /    Unit Tests    \ <- 70% - Domain logic tests
           /____________________\
```

### 8.2 Unit Tests (Domain Layer)

```rust
// sevensense-audio/src/domain/aggregates/recording_tests.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_creation_validates_sample_rate() {
        let result = Recording::create(
            RecordingId::new(),
            AudioSource::file("test.wav"),
            &[0u8; 1000],
            RecordingMetadata {
                sample_rate: 44100, // Invalid - must be 32000
                channels: 1,
                duration_ms: 5000,
                ..Default::default()
            },
        );

        assert!(matches!(result, Err(AudioDomainError::InvalidSampleRate(44100))));
    }

    #[test]
    fn test_recording_emits_audio_ingested_event() {
        let mut recording = Recording::create(
            RecordingId::new(),
            AudioSource::file("test.wav"),
            &[0u8; 160000],
            RecordingMetadata {
                sample_rate: 32000,
                channels: 1,
                duration_ms: 5000,
                ..Default::default()
            },
        ).unwrap();

        let events = recording.take_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "AudioIngested");
    }

    #[test]
    fn test_segment_overlap_detection() {
        let mut recording = create_test_recording();

        recording.add_segment(CallSegment {
            id: SegmentId::new(),
            start_ms: 0,
            end_ms: 1000,
            snr: 10.0,
            energy: 0.5,
            features: SegmentFeatures::default(),
        }).unwrap();

        // This segment overlaps with the first
        let result = recording.add_segment(CallSegment {
            id: SegmentId::new(),
            start_ms: 500,
            end_ms: 1500,
            snr: 10.0,
            energy: 0.5,
            features: SegmentFeatures::default(),
        });

        assert!(matches!(result, Err(AudioDomainError::OverlappingSegments)));
    }

    #[test]
    fn test_evidence_pack_citation_generation() {
        let pack = EvidencePack::assemble(
            SegmentId::new(),
            vec![
                (SegmentId::new(), 0.1), // Close neighbor
                (SegmentId::new(), 0.8), // Far neighbor
            ],
            vec![create_test_exemplar(0.9)],
            SequenceContext::default(),
            SignalQuality { snr: 20.0, ..Default::default() },
        ).unwrap();

        let description = StructuralDescription {
            similarity_claims: vec!["Similar to alarm calls".to_string()],
            cluster_claims: vec!["Member of cluster A".to_string()],
            ..Default::default()
        };

        let citations = pack.build_citations(&description).unwrap();

        // Only high-relevance evidence should be cited
        assert!(citations.iter().any(|c| c.strength > 0.5));
        assert!(citations.iter().all(|c| c.strength > 0.0));
    }
}
```

### 8.3 Integration Tests

```rust
// tests/integration/embedding_pipeline_test.rs

#[tokio::test]
async fn test_full_embedding_pipeline() {
    let ctx = TestContext::new().await;

    // Create test audio file
    let audio_path = ctx.create_test_audio(32000, 5.0).await;

    // Run through audio ingestion
    let recording_id = ctx.audio_service
        .ingest_audio(IngestAudioCommand {
            source: AudioSource::file(&audio_path),
            metadata: RecordingMetadata::default(),
            auto_segment: true,
        })
        .await
        .unwrap();

    // Verify segments were detected
    let segments = ctx.segment_repo.find_by_recording(&recording_id).await.unwrap();
    assert!(!segments.is_empty());

    // Generate embeddings
    let job_id = ctx.embedding_service
        .generate_embeddings(GenerateEmbeddingsCommand {
            segment_ids: segments.iter().map(|s| s.id).collect(),
            model: ModelVersion::perch_v2(),
            config: EmbeddingConfig::default(),
        })
        .await
        .unwrap();

    // Verify embeddings were generated
    for segment in &segments {
        let embedding = ctx.embedding_repo.find_by_segment(&segment.id).await.unwrap();
        assert!(embedding.is_some());
        assert_eq!(embedding.unwrap().vector.len(), 1536); // Perch 2.0 dimensions
    }

    // Verify events were published
    let events = ctx.event_store.read_all(GlobalPosition(0)).await.unwrap();
    assert!(events.iter().any(|e| e.event_type == "AudioIngested"));
    assert!(events.iter().any(|e| e.event_type == "SegmentDetected"));
    assert!(events.iter().any(|e| e.event_type == "EmbeddingGenerated"));
}

#[tokio::test]
async fn test_vector_space_neighbor_query() {
    let ctx = TestContext::new().await;

    // Index test vectors
    let vectors: Vec<IndexedVector> = (0..100)
        .map(|i| IndexedVector {
            id: VectorId::new(),
            embedding_id: EmbeddingId::new(),
            vector: create_test_vector(i),
            layer: 0,
        })
        .collect();

    for vector in &vectors {
        ctx.vector_service
            .index_embeddings(IndexEmbeddingsCommand {
                embeddings: vec![vector.clone()],
            })
            .await
            .unwrap();
    }

    // Query neighbors
    let query = create_test_vector(50);
    let neighbors = ctx.vector_service
        .find_neighbors(FindNeighborsQuery {
            vector: query.clone(),
            k: 10,
        })
        .await
        .unwrap();

    assert_eq!(neighbors.len(), 10);

    // Verify closest neighbor is vector 50 itself or nearby
    let closest = &neighbors[0];
    assert!(closest.distance < 0.1);
}
```

### 8.4 End-to-End Tests

```rust
// tests/e2e/interpretation_test.rs

#[tokio::test]
async fn test_end_to_end_interpretation() {
    let app = TestApp::spawn().await;

    // Upload audio via API
    let response = app.client
        .post("/api/v1/recordings")
        .multipart(
            reqwest::multipart::Form::new()
                .file("audio", "tests/fixtures/bird_call.wav")
                .await
                .unwrap()
        )
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 201);
    let recording: RecordingResponse = response.json().await.unwrap();

    // Wait for processing
    app.wait_for_processing(recording.id, Duration::from_secs(30)).await;

    // Request interpretation for first segment
    let segments_response = app.client
        .get(&format!("/api/v1/recordings/{}/segments", recording.id))
        .send()
        .await
        .unwrap();

    let segments: Vec<SegmentResponse> = segments_response.json().await.unwrap();
    let first_segment = &segments[0];

    let interpretation_response = app.client
        .post(&format!("/api/v1/segments/{}/interpret", first_segment.id))
        .json(&InterpretRequest {
            neighbor_count: 10,
            include_exemplars: true,
        })
        .send()
        .await
        .unwrap();

    assert_eq!(interpretation_response.status(), 200);
    let interpretation: InterpretationResponse = interpretation_response.json().await.unwrap();

    // Verify interpretation has citations
    assert!(!interpretation.citations.is_empty());
    assert!(interpretation.confidence > 0.0);

    // Verify evidence pack was assembled
    assert!(interpretation.evidence.neighbor_count > 0);
}
```

### 8.5 Test Fixtures and Factories

```rust
// tests/support/factories.rs

pub struct TestFactory;

impl TestFactory {
    pub fn recording() -> RecordingBuilder {
        RecordingBuilder::new()
    }

    pub fn segment() -> SegmentBuilder {
        SegmentBuilder::new()
    }

    pub fn embedding() -> EmbeddingBuilder {
        EmbeddingBuilder::new()
    }
}

pub struct RecordingBuilder {
    id: Option<RecordingId>,
    source: Option<AudioSource>,
    sample_rate: u32,
    duration_ms: u64,
}

impl RecordingBuilder {
    pub fn new() -> Self {
        Self {
            id: None,
            source: None,
            sample_rate: 32000,
            duration_ms: 5000,
        }
    }

    pub fn with_id(mut self, id: RecordingId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    pub fn build(self) -> Recording {
        Recording::create(
            self.id.unwrap_or_else(RecordingId::new),
            self.source.unwrap_or_else(|| AudioSource::file("test.wav")),
            &vec![0u8; (self.sample_rate * self.duration_ms / 1000) as usize],
            RecordingMetadata {
                sample_rate: self.sample_rate,
                channels: 1,
                duration_ms: self.duration_ms,
                ..Default::default()
            },
        ).unwrap()
    }
}

// Mock implementations for testing
pub struct MockEmbeddingModelAdapter;

#[async_trait]
impl EmbeddingModelAdapter for MockEmbeddingModelAdapter {
    async fn embed(&self, _audio: &AudioData) -> Result<Vec<f32>, EmbeddingError> {
        // Return deterministic mock embedding
        Ok(vec![0.1; 1536])
    }

    fn model_version(&self) -> ModelVersion {
        ModelVersion {
            name: "mock".to_string(),
            version: "1.0".to_string(),
            output_dimensions: 1536,
        }
    }
}
```

---

## 9. Implementation Timeline

| Phase | Duration | Focus |
|-------|----------|-------|
| **Phase 1** | 2 weeks | Core domain models, value objects, events |
| **Phase 2** | 3 weeks | Audio Ingestion + Embedding contexts |
| **Phase 3** | 3 weeks | Vector Space context + RuVector ACL |
| **Phase 4** | 2 weeks | Learning context + GNN integration |
| **Phase 5** | 2 weeks | Analysis context (clustering, motifs) |
| **Phase 6** | 2 weeks | Interpretation context (RAB) |
| **Phase 7** | 2 weeks | API Gateway + CLI |
| **Phase 8** | 2 weeks | Integration testing + E2E |

---

## 10. Key Dependencies

```toml
# Cargo.toml (workspace)
[workspace]
members = [
    "crates/sevensense-core",
    "crates/sevensense-audio",
    "crates/sevensense-embedding",
    "crates/sevensense-vector",
    "crates/sevensense-learning",
    "crates/sevensense-analysis",
    "crates/sevensense-interpretation",
    "services/api-gateway",
    "services/worker",
    "apps/cli",
]

[workspace.dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }
async-trait = "0.1"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Database
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "uuid", "chrono"] }

# UUID and time
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

# Audio processing
symphonia = { version = "0.5", features = ["all"] }
hound = "3.5"

# ONNX runtime for Perch
ort = { version = "1.16", features = ["cuda"] }

# Numerical computing
ndarray = "0.15"
nalgebra = "0.32"

# RuVector integration
ruvector-core = { git = "https://github.com/ruvnet/ruvector" }
ruvector-gnn = { git = "https://github.com/ruvnet/ruvector" }

# HTTP framework
axum = "0.7"
tower = "0.4"

# Testing
proptest = "1.4"
fake = "2.9"
```

---

## Conclusion

This DDD implementation plan provides a comprehensive blueprint for building 7sense as a modular, testable, and maintainable system. The bounded contexts ensure clear separation of concerns while domain events enable loose coupling between contexts. The anti-corruption layers protect domain integrity when integrating with external systems like Perch 2.0 and RuVector.

Key architectural decisions:
1. **Event-driven architecture** enables asynchronous processing and eventual consistency
2. **CQRS** separates read and write concerns for performance optimization
3. **Hexagonal architecture** per context ensures testability and flexibility
4. **Anti-corruption layers** isolate domain from external system changes
5. **Saga pattern** coordinates complex cross-context operations

The testing strategy ensures reliability at all levels, from unit tests for domain logic to end-to-end tests for the complete pipeline.
