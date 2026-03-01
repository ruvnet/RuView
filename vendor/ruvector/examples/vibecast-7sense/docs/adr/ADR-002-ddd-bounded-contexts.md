# ADR-002: Domain-Driven Design Bounded Contexts

## Status

Accepted

## Date

2026-01-15

## Context

7sense is a bioacoustics analysis platform that transforms bird audio recordings into navigable geometric spaces. The system processes audio through Perch 2.0 embeddings (1536-dimensional vectors), stores them in RuVector with HNSW indexing, and applies GNN learning to discover patterns, motifs, and sequences. The output feeds into RAB (Retrieval-Augmented Bioacoustics) evidence packs for transparent, citation-backed interpretations.

The complexity of this domain requires clear separation of concerns to:
- Enable independent evolution of subsystems
- Maintain clear ownership boundaries
- Reduce coupling between technical and analytical components
- Support distributed team development
- Facilitate testing and validation at context boundaries

## Decision

We adopt Domain-Driven Design (DDD) with six bounded contexts that represent distinct subdomains of the bioacoustics analysis pipeline:

1. **Audio Ingestion Context**
2. **Embedding Context**
3. **Vector Space Context**
4. **Learning Context**
5. **Analysis Context**
6. **Interpretation Context**

---

## Bounded Context Definitions

### 1. Audio Ingestion Context

**Purpose**: Capture, segment, and preprocess raw audio recordings into analysis-ready call segments.

#### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Recording** | A continuous audio capture from a sensor at a specific location and time |
| **Sensor** | A physical audio capture device with known characteristics (sample rate, gain, location) |
| **Call Segment** | An isolated vocalization extracted from a recording (typically 5 seconds at 32kHz) |
| **Segmentation** | The process of detecting and extracting individual vocalizations from continuous audio |
| **SNR (Signal-to-Noise Ratio)** | Quality metric indicating clarity of vocalization above background noise |
| **Preprocessing** | Normalization, resampling, and filtering applied before embedding |
| **Habitat** | Environmental classification of the recording location |
| **Soundscape** | The full acoustic environment including all sound sources |

#### Aggregates and Entities

```
Aggregate: Recording
├── Entity: Recording (Aggregate Root)
│   ├── id: RecordingId (UUID)
│   ├── sensorId: SensorId
│   ├── location: GeoLocation {lat, lon, altitude}
│   ├── startTimestamp: DateTime
│   ├── duration: Duration
│   ├── habitat: HabitatType
│   ├── weather: WeatherConditions
│   ├── format: AudioFormat {sampleRate, channels, bitDepth}
│   └── status: IngestionStatus
│
├── Value Object: AudioFormat
│   ├── sampleRate: u32 (target: 32000 Hz)
│   ├── channels: u8 (target: 1 mono)
│   └── bitDepth: u8
│
└── Value Object: WeatherConditions
    ├── temperature: f32
    ├── humidity: f32
    ├── windSpeed: f32
    └── precipitation: PrecipitationType

Aggregate: CallSegment
├── Entity: CallSegment (Aggregate Root)
│   ├── id: SegmentId (UUID)
│   ├── recordingId: RecordingId
│   ├── startOffset: Duration (t0_ms)
│   ├── endOffset: Duration (t1_ms)
│   ├── snr: f32
│   ├── energy: f32
│   ├── clippingScore: f32
│   ├── overlapScore: f32
│   └── qualityGrade: QualityGrade
│
└── Value Object: SegmentMetrics
    ├── peakAmplitude: f32
    ├── rmsEnergy: f32
    ├── zeroCrossingRate: f32
    └── spectralCentroid: f32

Aggregate: Sensor
├── Entity: Sensor (Aggregate Root)
│   ├── id: SensorId
│   ├── model: String
│   ├── location: GeoLocation
│   ├── calibration: CalibrationProfile
│   └── status: SensorStatus
│
└── Value Object: CalibrationProfile
    ├── frequencyResponse: Vec<(f32, f32)>
    ├── noiseFloor: f32
    └── lastCalibrated: DateTime
```

#### Domain Events

| Event | Payload | Published When |
|-------|---------|----------------|
| `RecordingReceived` | recordingId, sensorId, timestamp, duration | New audio file uploaded/streamed |
| `RecordingValidated` | recordingId, format, qualityScore | Format and quality checks pass |
| `RecordingRejected` | recordingId, reason, details | Recording fails validation |
| `SegmentationStarted` | recordingId, algorithm, parameters | Segmentation process begins |
| `SegmentExtracted` | segmentId, recordingId, timeRange, snr | Individual call isolated |
| `SegmentationCompleted` | recordingId, segmentCount, duration | All segments extracted |
| `PreprocessingCompleted` | segmentId, normalizedFormat | Segment ready for embedding |

#### Services

```rust
// Domain Services
trait SegmentationService {
    fn segment_recording(recording: &Recording, config: SegmentationConfig)
        -> Result<Vec<CallSegment>, SegmentationError>;
    fn detect_vocalizations(audio: &AudioBuffer) -> Vec<TimeRange>;
}

trait PreprocessingService {
    fn normalize(segment: &CallSegment) -> NormalizedAudio;
    fn resample(audio: &AudioBuffer, targetRate: u32) -> AudioBuffer;
    fn apply_bandpass(audio: &AudioBuffer, lowHz: f32, highHz: f32) -> AudioBuffer;
}

trait QualityAssessmentService {
    fn compute_snr(segment: &CallSegment) -> f32;
    fn detect_clipping(segment: &CallSegment) -> f32;
    fn assess_quality(segment: &CallSegment) -> QualityGrade;
}
```

---

### 2. Embedding Context

**Purpose**: Transform preprocessed audio segments into 1536-dimensional Perch 2.0 embeddings suitable for vector space operations.

#### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Embedding** | A 1536-dimensional vector representation of a call segment |
| **Perch 2.0** | Google DeepMind's bioacoustic embedding model (EfficientNet-B3 backbone) |
| **Mel Spectrogram** | Time-frequency representation using mel-scaled frequency bins (500 frames x 128 bins) |
| **Inference** | The process of generating an embedding from audio input |
| **Normalization** | L2 normalization of embedding vectors for cosine similarity |
| **Model Version** | Specific checkpoint/version of the embedding model |
| **Batch** | Collection of segments processed together for efficiency |
| **Embedding Stability** | Consistency of embeddings for identical/similar inputs |

#### Aggregates and Entities

```
Aggregate: Embedding
├── Entity: Embedding (Aggregate Root)
│   ├── id: EmbeddingId (UUID)
│   ├── segmentId: SegmentId
│   ├── vector: Vec<f32> (dim=1536)
│   ├── modelVersion: ModelVersion
│   ├── norm: f32
│   ├── createdAt: DateTime
│   └── metadata: EmbeddingMetadata
│
└── Value Object: EmbeddingMetadata
    ├── inferenceLatency: Duration
    ├── batchId: Option<BatchId>
    └── gpuUsed: bool

Aggregate: EmbeddingModel
├── Entity: EmbeddingModel (Aggregate Root)
│   ├── id: ModelId
│   ├── name: "perch2"
│   ├── version: SemanticVersion
│   ├── dimensions: u32 (1536)
│   ├── inputSpec: InputSpecification
│   └── status: ModelStatus
│
├── Value Object: InputSpecification
│   ├── sampleRate: 32000
│   ├── windowDuration: 5.0 seconds
│   ├── windowSamples: 160000
│   ├── melBins: 128
│   └── frequencyRange: (60, 16000) Hz
│
└── Value Object: ModelCheckpoint
    ├── path: String
    ├── format: ModelFormat (ONNX)
    └── checksum: String

Aggregate: EmbeddingBatch
├── Entity: EmbeddingBatch (Aggregate Root)
│   ├── id: BatchId
│   ├── segmentIds: Vec<SegmentId>
│   ├── status: BatchStatus
│   ├── startedAt: DateTime
│   ├── completedAt: Option<DateTime>
│   └── metrics: BatchMetrics
│
└── Value Object: BatchMetrics
    ├── totalSegments: u32
    ├── successCount: u32
    ├── failureCount: u32
    ├── avgLatencyMs: f32
    └── throughput: f32
```

#### Domain Events

| Event | Payload | Published When |
|-------|---------|----------------|
| `EmbeddingRequested` | segmentId, modelVersion, priority | Segment queued for embedding |
| `BatchCreated` | batchId, segmentIds, modelVersion | Batch assembled for processing |
| `InferenceStarted` | embeddingId/batchId, modelVersion | Model inference begins |
| `EmbeddingGenerated` | embeddingId, segmentId, vector, norm | Single embedding computed |
| `BatchCompleted` | batchId, successCount, failureCount | Batch processing finishes |
| `EmbeddingFailed` | segmentId, error, retryable | Inference failure |
| `ModelVersionChanged` | oldVersion, newVersion, migrationRequired | Model updated |
| `EmbeddingNormalized` | embeddingId, originalNorm, normalizedVector | L2 normalization applied |

#### Services

```rust
// Domain Services
trait EmbeddingService {
    fn embed_segment(segment: &NormalizedAudio, model: &EmbeddingModel)
        -> Result<Embedding, EmbeddingError>;
    fn embed_batch(segments: Vec<&NormalizedAudio>, model: &EmbeddingModel)
        -> Vec<Result<Embedding, EmbeddingError>>;
}

trait SpectrogramService {
    fn compute_mel_spectrogram(audio: &AudioBuffer) -> MelSpectrogram;
    fn validate_spectrogram(spectrogram: &MelSpectrogram) -> ValidationResult;
}

trait NormalizationService {
    fn l2_normalize(embedding: &Embedding) -> NormalizedEmbedding;
    fn validate_norm_stability(embeddings: &[Embedding]) -> StabilityReport;
}

trait ModelManagementService {
    fn load_model(version: &ModelVersion) -> Result<EmbeddingModel, ModelError>;
    fn validate_model_output(embedding: &Embedding) -> ValidationResult;
    fn compare_model_versions(v1: &ModelVersion, v2: &ModelVersion, samples: &[AudioBuffer])
        -> VersionComparisonReport;
}
```

---

### 3. Vector Space Context

**Purpose**: Index embeddings using HNSW, manage similarity search, and maintain the navigable neighbor graph that forms the geometric foundation.

#### Ubiquitous Language

| Term | Definition |
|------|------------|
| **HNSW Index** | Hierarchical Navigable Small World graph for approximate nearest neighbor search |
| **Neighbor Graph** | Network of similarity edges connecting acoustically related embeddings |
| **k-NN Query** | Search for k nearest neighbors to a query vector |
| **Similarity Edge** | Weighted connection between two embeddings based on distance |
| **Distance Metric** | Function measuring dissimilarity (cosine, euclidean, Poincare) |
| **Index Layer** | One level in the HNSW hierarchical structure |
| **Entry Point** | Starting node for graph traversal in search |
| **ef (Search)** | Exploration factor controlling search accuracy vs. speed |
| **M (Construction)** | Maximum number of connections per node per layer |

#### Aggregates and Entities

```
Aggregate: VectorIndex
├── Entity: VectorIndex (Aggregate Root)
│   ├── id: IndexId
│   ├── name: String
│   ├── dimensions: u32 (1536)
│   ├── distanceMetric: DistanceMetric
│   ├── hnswConfig: HnswConfiguration
│   ├── vectorCount: u64
│   ├── layerCount: u32
│   └── status: IndexStatus
│
├── Value Object: HnswConfiguration
│   ├── m: u32 (max connections per layer)
│   ├── efConstruction: u32
│   ├── efSearch: u32
│   └── maxLayers: u32
│
└── Value Object: IndexStatistics
    ├── memoryUsage: u64
    ├── avgDegree: f32
    ├── layerDistribution: Vec<u32>
    └── searchLatencyP99: Duration

Aggregate: IndexedVector
├── Entity: IndexedVector (Aggregate Root)
│   ├── id: VectorId
│   ├── embeddingId: EmbeddingId
│   ├── indexId: IndexId
│   ├── layerMembership: Vec<u32>
│   ├── neighborIds: Vec<VectorId>
│   └── insertedAt: DateTime
│
└── Value Object: VectorPosition
    ├── entryDistance: f32
    └── layerDistances: Vec<f32>

Aggregate: SimilarityEdge
├── Entity: SimilarityEdge (Aggregate Root)
│   ├── id: EdgeId
│   ├── sourceId: VectorId
│   ├── targetId: VectorId
│   ├── distance: f32
│   ├── edgeType: EdgeType (SIMILAR, HNSW_NEIGHBOR)
│   └── weight: f32
│
└── Value Object: EdgeMetadata
    ├── createdAt: DateTime
    ├── lastAccessed: DateTime
    └── accessCount: u32

Aggregate: SearchQuery
├── Entity: SearchQuery (Aggregate Root)
│   ├── id: QueryId
│   ├── queryVector: Vec<f32>
│   ├── k: u32
│   ├── efSearch: u32
│   ├── filters: Vec<SearchFilter>
│   └── results: Option<SearchResults>
│
└── Value Object: SearchResults
    ├── neighbors: Vec<(VectorId, f32)>
    ├── searchLatency: Duration
    ├── nodesVisited: u32
    └── distanceComputations: u32
```

#### Domain Events

| Event | Payload | Published When |
|-------|---------|----------------|
| `IndexCreated` | indexId, config, distanceMetric | New HNSW index initialized |
| `VectorInserted` | vectorId, embeddingId, indexId, layerAssignment | Embedding added to index |
| `VectorRemoved` | vectorId, indexId | Embedding removed from index |
| `NeighborGraphUpdated` | indexId, affectedVectors, newEdges | Graph structure modified |
| `SimilarityEdgeCreated` | edgeId, sourceId, targetId, distance | New similarity link established |
| `SearchExecuted` | queryId, k, latency, resultsCount | k-NN search completed |
| `IndexRebuildStarted` | indexId, reason, estimatedDuration | Index reconstruction begins |
| `IndexRebuildCompleted` | indexId, vectorCount, duration | Index reconstruction finishes |
| `IndexOptimized` | indexId, beforeStats, afterStats | Index compaction/optimization |

#### Services

```rust
// Domain Services
trait VectorIndexService {
    fn create_index(config: IndexConfiguration) -> Result<VectorIndex, IndexError>;
    fn insert_vector(index: &mut VectorIndex, embedding: &Embedding)
        -> Result<IndexedVector, InsertionError>;
    fn remove_vector(index: &mut VectorIndex, vectorId: VectorId)
        -> Result<(), RemovalError>;
    fn rebuild_index(index: &mut VectorIndex) -> Result<IndexStatistics, RebuildError>;
}

trait SimilaritySearchService {
    fn knn_search(index: &VectorIndex, query: &[f32], k: u32, ef: u32)
        -> SearchResults;
    fn range_search(index: &VectorIndex, query: &[f32], radius: f32)
        -> Vec<(VectorId, f32)>;
    fn batch_search(index: &VectorIndex, queries: &[Vec<f32>], k: u32)
        -> Vec<SearchResults>;
}

trait NeighborGraphService {
    fn get_neighbors(vectorId: VectorId, depth: u32) -> NeighborGraph;
    fn compute_similarity_edges(index: &VectorIndex, topK: u32)
        -> Vec<SimilarityEdge>;
    fn prune_edges(index: &mut VectorIndex, threshold: f32) -> u32;
}

trait DistanceService {
    fn cosine_distance(a: &[f32], b: &[f32]) -> f32;
    fn euclidean_distance(a: &[f32], b: &[f32]) -> f32;
    fn poincare_distance(a: &[f32], b: &[f32], curvature: f32) -> f32;
}
```

---

### 4. Learning Context

**Purpose**: Train and apply GNN models to refine embeddings, learn transition patterns, and enable continuous self-improvement of the vector space.

#### Ubiquitous Language

| Term | Definition |
|------|------------|
| **GNN (Graph Neural Network)** | Neural network operating on graph-structured data |
| **Message Passing** | GNN mechanism where nodes aggregate information from neighbors |
| **Graph Attention (GAT)** | Attention-weighted message passing for learnable edge importance |
| **Training Epoch** | One complete pass through the training data |
| **Contrastive Loss** | Loss function pulling similar pairs together, pushing dissimilar apart |
| **InfoNCE** | Information Noise-Contrastive Estimation loss for self-supervised learning |
| **Embedding Refinement** | GNN-driven adjustment of embedding positions in vector space |
| **Transition Edge** | Temporal connection between sequential call segments |
| **EWC (Elastic Weight Consolidation)** | Technique preventing catastrophic forgetting during updates |

#### Aggregates and Entities

```
Aggregate: LearningModel
├── Entity: LearningModel (Aggregate Root)
│   ├── id: ModelId
│   ├── architecture: GnnArchitecture (GAT, GraphSAGE, GCN)
│   ├── layers: Vec<LayerConfig>
│   ├── version: SemanticVersion
│   ├── trainedAt: DateTime
│   ├── metrics: TrainingMetrics
│   └── status: ModelStatus
│
├── Value Object: LayerConfig
│   ├── layerType: LayerType
│   ├── inputDim: u32
│   ├── outputDim: u32
│   ├── heads: u32 (for attention)
│   └── dropout: f32
│
└── Value Object: TrainingMetrics
    ├── epochs: u32
    ├── finalLoss: f32
    ├── validationScore: f32
    └── trainingDuration: Duration

Aggregate: TrainingSession
├── Entity: TrainingSession (Aggregate Root)
│   ├── id: SessionId
│   ├── modelId: ModelId
│   ├── config: TrainingConfiguration
│   ├── currentEpoch: u32
│   ├── status: SessionStatus
│   └── checkpoints: Vec<Checkpoint>
│
├── Value Object: TrainingConfiguration
│   ├── learningRate: f32
│   ├── batchSize: u32
│   ├── maxEpochs: u32
│   ├── lossFunction: LossType (InfoNCE, Triplet, Contrastive)
│   ├── optimizer: OptimizerType
│   └── ewcEnabled: bool
│
└── Value Object: Checkpoint
    ├── epoch: u32
    ├── loss: f32
    ├── weightsPath: String
    └── timestamp: DateTime

Aggregate: TransitionGraph
├── Entity: TransitionGraph (Aggregate Root)
│   ├── id: GraphId
│   ├── nodeCount: u32
│   ├── edgeCount: u32
│   ├── edgeTypes: Vec<EdgeType>
│   └── statistics: GraphStatistics
│
├── Entity: TransitionEdge
│   ├── id: EdgeId
│   ├── sourceSegmentId: SegmentId
│   ├── targetSegmentId: SegmentId
│   ├── edgeType: EdgeType (NEXT, SIMILAR, CO_OCCURRENCE)
│   ├── weight: f32
│   └── metadata: EdgeMetadata
│
└── Value Object: GraphStatistics
    ├── avgDegree: f32
    ├── clusteringCoefficient: f32
    ├── diameter: u32
    └── componentCount: u32

Aggregate: RefinedEmbedding
├── Entity: RefinedEmbedding (Aggregate Root)
│   ├── id: RefinedEmbeddingId
│   ├── originalEmbeddingId: EmbeddingId
│   ├── refinedVector: Vec<f32>
│   ├── modelVersion: ModelVersion
│   ├── refinementDelta: f32
│   └── createdAt: DateTime
│
└── Value Object: RefinementMetadata
    ├── neighborInfluence: Vec<(EmbeddingId, f32)>
    ├── attentionWeights: Vec<f32>
    └── iterations: u32
```

#### Domain Events

| Event | Payload | Published When |
|-------|---------|----------------|
| `TrainingSessionStarted` | sessionId, modelId, config | GNN training begins |
| `EpochCompleted` | sessionId, epoch, loss, metrics | Training epoch finishes |
| `CheckpointSaved` | sessionId, epoch, path | Model weights saved |
| `TrainingCompleted` | sessionId, finalMetrics | Training session ends |
| `ModelDeployed` | modelId, version | New model activated |
| `EmbeddingsRefined` | batchId, vectorCount, avgDelta | GNN refinement applied |
| `TransitionEdgeDiscovered` | edgeId, source, target, type | New temporal relationship |
| `GraphStructureUpdated` | graphId, addedEdges, removedEdges | Transition graph modified |
| `LearningRateAdjusted` | sessionId, oldLr, newLr | Adaptive LR change |
| `EwcConsolidated` | sessionId, importantWeights | EWC protection updated |

#### Services

```rust
// Domain Services
trait GnnTrainingService {
    fn start_training(model: &LearningModel, graph: &TransitionGraph, config: TrainingConfiguration)
        -> Result<TrainingSession, TrainingError>;
    fn run_epoch(session: &mut TrainingSession, batch: &GraphBatch)
        -> EpochResult;
    fn save_checkpoint(session: &TrainingSession) -> Result<Checkpoint, IoError>;
    fn apply_ewc(session: &mut TrainingSession, importanceMatrix: &ImportanceMatrix);
}

trait EmbeddingRefinementService {
    fn refine_embeddings(embeddings: &[Embedding], model: &LearningModel, graph: &TransitionGraph)
        -> Vec<RefinedEmbedding>;
    fn compute_refinement_delta(original: &Embedding, refined: &RefinedEmbedding) -> f32;
}

trait TransitionGraphService {
    fn build_transition_graph(segments: &[CallSegment], recordings: &[Recording])
        -> TransitionGraph;
    fn add_temporal_edges(graph: &mut TransitionGraph, sequences: &[SegmentSequence]);
    fn add_similarity_edges(graph: &mut TransitionGraph, index: &VectorIndex, topK: u32);
    fn compute_graph_statistics(graph: &TransitionGraph) -> GraphStatistics;
}

trait AttentionService {
    fn compute_attention_weights(query: &Embedding, neighbors: &[Embedding]) -> Vec<f32>;
    fn apply_graph_attention(node: &GraphNode, neighbors: &[GraphNode], model: &GatLayer)
        -> AttentionOutput;
}
```

---

### 5. Analysis Context

**Purpose**: Perform clustering, motif detection, sequence mining, and pattern discovery on the refined vector space.

#### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Cluster** | Group of acoustically similar call segments |
| **Prototype** | Representative embedding for a cluster (centroid or medoid) |
| **Exemplar** | Actual call segment that best represents a cluster |
| **Motif** | Recurring pattern or phrase in vocalization sequences |
| **Sequence** | Ordered series of call segments from a recording |
| **Transition Matrix** | Probability matrix of call-to-call transitions |
| **Entropy Rate** | Measure of unpredictability in vocalization sequences |
| **Call Type** | Functional category of vocalization (alarm, contact, song) |
| **Dialect** | Regional variation in vocalization patterns |

#### Aggregates and Entities

```
Aggregate: Cluster
├── Entity: Cluster (Aggregate Root)
│   ├── id: ClusterId
│   ├── method: ClusteringMethod (HDBSCAN, KMeans, Spectral)
│   ├── parameters: ClusteringParameters
│   ├── memberCount: u32
│   ├── cohesion: f32
│   ├── separation: f32
│   └── status: ClusterStatus
│
├── Entity: Prototype
│   ├── id: PrototypeId
│   ├── clusterId: ClusterId
│   ├── centroidVector: Vec<f32>
│   ├── exemplarIds: Vec<SegmentId>
│   └── stability: f32
│
└── Value Object: ClusteringParameters
    ├── minClusterSize: u32
    ├── minSamples: u32
    ├── epsilon: Option<f32>
    └── metric: DistanceMetric

Aggregate: ClusterAssignment
├── Entity: ClusterAssignment (Aggregate Root)
│   ├── id: AssignmentId
│   ├── segmentId: SegmentId
│   ├── clusterId: ClusterId
│   ├── confidence: f32
│   ├── distance_to_centroid: f32
│   └── assignedAt: DateTime
│
└── Value Object: SoftAssignment
    ├── clusterProbabilities: Vec<(ClusterId, f32)>
    └── isAmbiguous: bool

Aggregate: Motif
├── Entity: Motif (Aggregate Root)
│   ├── id: MotifId
│   ├── pattern: Vec<ClusterId>
│   ├── occurrenceCount: u32
│   ├── avgDuration: Duration
│   ├── confidence: f32
│   └── context: MotifContext
│
├── Value Object: MotifOccurrence
│   ├── recordingId: RecordingId
│   ├── startSegmentId: SegmentId
│   ├── segmentIds: Vec<SegmentId>
│   └── timestamp: DateTime
│
└── Value Object: MotifContext
    ├── typicalHabitat: Vec<HabitatType>
    ├── timeOfDay: Vec<TimeRange>
    └── associatedBehavior: Option<String>

Aggregate: SequenceAnalysis
├── Entity: SequenceAnalysis (Aggregate Root)
│   ├── id: AnalysisId
│   ├── recordingId: RecordingId
│   ├── segmentSequence: Vec<SegmentId>
│   ├── clusterSequence: Vec<ClusterId>
│   ├── transitionMatrix: TransitionMatrix
│   └── metrics: SequenceMetrics
│
├── Value Object: TransitionMatrix
│   ├── clusterIds: Vec<ClusterId>
│   ├── probabilities: Vec<Vec<f32>>
│   └── observations: Vec<Vec<u32>>
│
└── Value Object: SequenceMetrics
    ├── entropyRate: f32
    ├── stereotypy: f32
    ├── motifDensity: f32
    └── uniqueTransitions: u32

Aggregate: Anomaly
├── Entity: Anomaly (Aggregate Root)
│   ├── id: AnomalyId
│   ├── segmentId: SegmentId
│   ├── anomalyType: AnomalyType (Rare, Novel, Artifact)
│   ├── score: f32
│   ├── nearestCluster: Option<ClusterId>
│   └── detectedAt: DateTime
│
└── Value Object: AnomalyContext
    ├── neighborDistances: Vec<f32>
    ├── localDensity: f32
    └── globalRarity: f32
```

#### Domain Events

| Event | Payload | Published When |
|-------|---------|----------------|
| `ClusteringStarted` | clusterId, method, parameters | Clustering analysis begins |
| `ClusteringCompleted` | clusterId, clusterCount, metrics | Clustering finishes |
| `ClusterAssigned` | assignmentId, segmentId, clusterId, confidence | Segment assigned to cluster |
| `PrototypeUpdated` | prototypeId, clusterId, newCentroid | Cluster representative changed |
| `MotifDiscovered` | motifId, pattern, occurrenceCount | New recurring pattern found |
| `MotifOccurrenceFound` | motifId, recordingId, segmentIds | Motif instance detected |
| `SequenceAnalyzed` | analysisId, recordingId, entropyRate | Sequence metrics computed |
| `AnomalyDetected` | anomalyId, segmentId, score, type | Unusual vocalization found |
| `TransitionMatrixUpdated` | recordingId, entropyChange | Transition probabilities recalculated |
| `DialectIdentified` | clusterId, region, distinctiveness | Regional variant discovered |

#### Services

```rust
// Domain Services
trait ClusteringService {
    fn cluster_embeddings(embeddings: &[Embedding], method: ClusteringMethod, params: ClusteringParameters)
        -> ClusteringResult;
    fn assign_to_cluster(embedding: &Embedding, clusters: &[Cluster])
        -> ClusterAssignment;
    fn compute_prototype(cluster: &Cluster, members: &[Embedding]) -> Prototype;
    fn evaluate_clustering(clusters: &[Cluster], assignments: &[ClusterAssignment])
        -> ClusteringMetrics;
}

trait MotifDetectionService {
    fn discover_motifs(sequences: &[SequenceAnalysis], minSupport: u32, maxLength: u32)
        -> Vec<Motif>;
    fn find_motif_occurrences(motif: &Motif, sequence: &SequenceAnalysis)
        -> Vec<MotifOccurrence>;
    fn validate_motif_dtw(motif: &Motif, occurrences: &[MotifOccurrence])
        -> ValidationResult;
}

trait SequenceAnalysisService {
    fn analyze_sequence(recording: &Recording, segments: &[CallSegment], clusters: &[Cluster])
        -> SequenceAnalysis;
    fn compute_transition_matrix(clusterSequence: &[ClusterId]) -> TransitionMatrix;
    fn compute_entropy_rate(matrix: &TransitionMatrix) -> f32;
    fn compute_stereotypy(matrix: &TransitionMatrix) -> f32;
}

trait AnomalyDetectionService {
    fn detect_anomalies(embeddings: &[Embedding], index: &VectorIndex, threshold: f32)
        -> Vec<Anomaly>;
    fn classify_anomaly(anomaly: &Anomaly, context: &AnalysisContext) -> AnomalyType;
    fn compute_local_outlier_factor(embedding: &Embedding, neighbors: &[Embedding]) -> f32;
}
```

---

### 6. Interpretation Context

**Purpose**: Generate RAB (Retrieval-Augmented Bioacoustics) evidence packs and constrained interpretations with full citation and transparency.

#### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Evidence Pack** | Structured collection of supporting data for an interpretation |
| **Citation** | Reference to specific retrieved calls supporting a statement |
| **Constrained Generation** | Output limited to evidence-backed structural descriptions |
| **Structural Descriptor** | Objective characterization (pitch contour, rhythm, spectral texture) |
| **Interpretation** | Evidence-backed analysis of vocalization meaning/context |
| **Confidence Level** | Certainty measure based on evidence quality and quantity |
| **Attribution** | Clear link between interpretation statements and source evidence |
| **Hypothesis** | Testable suggestion generated from pattern analysis |

#### Aggregates and Entities

```
Aggregate: EvidencePack
├── Entity: EvidencePack (Aggregate Root)
│   ├── id: EvidencePackId
│   ├── querySegmentId: SegmentId
│   ├── queryType: QueryType (Segment, TimeInterval, Habitat)
│   ├── retrievedNeighbors: Vec<RetrievedNeighbor>
│   ├── clusterExemplars: Vec<Exemplar>
│   ├── sequenceContext: SequenceContext
│   ├── signalQuality: SignalQuality
│   └── generatedAt: DateTime
│
├── Value Object: RetrievedNeighbor
│   ├── segmentId: SegmentId
│   ├── distance: f32
│   ├── clusterId: Option<ClusterId>
│   ├── spectrogramThumbnail: Option<ThumbnailId>
│   └── metadata: SegmentMetadata
│
├── Value Object: SequenceContext
│   ├── previousSegments: Vec<SegmentId>
│   ├── nextSegments: Vec<SegmentId>
│   ├── positionInRecording: f32
│   └── localMotifs: Vec<MotifId>
│
└── Value Object: SignalQuality
    ├── snr: f32
    ├── clippingScore: f32
    ├── overlapScore: f32
    └── qualityGrade: QualityGrade

Aggregate: Interpretation
├── Entity: Interpretation (Aggregate Root)
│   ├── id: InterpretationId
│   ├── evidencePackId: EvidencePackId
│   ├── interpretationType: InterpretationType
│   ├── statements: Vec<InterpretationStatement>
│   ├── overallConfidence: f32
│   ├── hypotheses: Vec<Hypothesis>
│   └── generatedAt: DateTime
│
├── Entity: InterpretationStatement
│   ├── id: StatementId
│   ├── content: String
│   ├── statementType: StatementType
│   ├── citations: Vec<Citation>
│   ├── confidence: f32
│   └── constraints: Vec<Constraint>
│
├── Value Object: Citation
│   ├── sourceType: CitationSource (Neighbor, Exemplar, Motif, Cluster)
│   ├── sourceId: String
│   ├── relevance: f32
│   └── excerpt: Option<String>
│
└── Value Object: Hypothesis
    ├── statement: String
    ├── testability: TestabilityLevel
    ├── supportingEvidence: Vec<CitationId>
    └── suggestedExperiment: Option<String>

Aggregate: StructuralDescriptor
├── Entity: StructuralDescriptor (Aggregate Root)
│   ├── id: DescriptorId
│   ├── segmentId: SegmentId
│   ├── pitchContour: PitchContourStats
│   ├── rhythmProfile: RhythmProfile
│   ├── spectralTexture: SpectralTexture
│   └── sequenceRole: SequenceRole
│
├── Value Object: PitchContourStats
│   ├── minFrequency: f32
│   ├── maxFrequency: f32
│   ├── meanFrequency: f32
│   ├── contourShape: ContourShape
│   └── bandwidth: f32
│
├── Value Object: RhythmProfile
│   ├── duration: Duration
│   ├── syllableCount: u32
│   ├── interSyllableIntervals: Vec<Duration>
│   └── rhythmRegularity: f32
│
├── Value Object: SpectralTexture
│   ├── harmonicity: f32
│   ├── spectralCentroid: f32
│   ├── spectralFlatness: f32
│   └── wienerEntropy: f32
│
└── Value Object: SequenceRole
    ├── typicalPredecessors: Vec<ClusterId>
    ├── typicalSuccessors: Vec<ClusterId>
    ├── positionDistribution: PositionDistribution
    └── contextualFrequency: f32

Aggregate: MonitoringSummary
├── Entity: MonitoringSummary (Aggregate Root)
│   ├── id: SummaryId
│   ├── timeRange: TimeRange
│   ├── location: GeoLocation
│   ├── callCounts: HashMap<ClusterId, u32>
│   ├── diversityMetrics: DiversityMetrics
│   ├── anomalies: Vec<AnomalyId>
│   └── interpretations: Vec<InterpretationId>
│
└── Value Object: DiversityMetrics
    ├── speciesRichness: u32
    ├── shannonIndex: f32
    ├── simpsonIndex: f32
    └── evenness: f32
```

#### Domain Events

| Event | Payload | Published When |
|-------|---------|----------------|
| `EvidencePackRequested` | querySegmentId, queryType, parameters | Analysis request initiated |
| `EvidencePackAssembled` | evidencePackId, neighborCount, exemplarCount | Evidence gathering complete |
| `InterpretationGenerated` | interpretationId, evidencePackId, statementCount | Interpretation created |
| `StatementCited` | statementId, citations | Statement linked to evidence |
| `HypothesisProposed` | hypothesisId, interpretationId, testability | Testable hypothesis generated |
| `StructuralDescriptorComputed` | descriptorId, segmentId | Acoustic features extracted |
| `MonitoringSummaryGenerated` | summaryId, timeRange, location | Period summary created |
| `AnnotationSuggested` | segmentId, suggestedLabel, confidence | Label recommendation made |
| `InterpretationValidated` | interpretationId, validationResult | Expert review completed |

#### Services

```rust
// Domain Services
trait EvidencePackService {
    fn assemble_evidence_pack(
        querySegment: &CallSegment,
        index: &VectorIndex,
        clusters: &[Cluster],
        sequences: &[SequenceAnalysis],
        config: EvidencePackConfig
    ) -> EvidencePack;

    fn retrieve_neighbors(segment: &CallSegment, index: &VectorIndex, k: u32)
        -> Vec<RetrievedNeighbor>;
    fn get_sequence_context(segment: &CallSegment, recording: &Recording)
        -> SequenceContext;
}

trait InterpretationService {
    fn generate_interpretation(evidencePack: &EvidencePack, constraints: &[Constraint])
        -> Interpretation;
    fn create_statement(content: &str, citations: &[Citation], statementType: StatementType)
        -> InterpretationStatement;
    fn generate_hypotheses(evidencePack: &EvidencePack, interpretation: &Interpretation)
        -> Vec<Hypothesis>;
}

trait StructuralDescriptorService {
    fn compute_descriptors(segment: &CallSegment) -> StructuralDescriptor;
    fn extract_pitch_contour(audio: &AudioBuffer) -> PitchContourStats;
    fn analyze_rhythm(segments: &[CallSegment]) -> RhythmProfile;
    fn compute_spectral_texture(spectrogram: &MelSpectrogram) -> SpectralTexture;
}

trait MonitoringService {
    fn generate_summary(
        recordings: &[Recording],
        timeRange: TimeRange,
        location: GeoLocation
    ) -> MonitoringSummary;
    fn compute_diversity_metrics(clusterAssignments: &[ClusterAssignment])
        -> DiversityMetrics;
    fn detect_temporal_patterns(summaries: &[MonitoringSummary])
        -> Vec<TemporalPattern>;
}

trait CitationService {
    fn create_citation(source: CitationSource, sourceId: &str, relevance: f32)
        -> Citation;
    fn validate_citation(citation: &Citation, evidencePack: &EvidencePack)
        -> ValidationResult;
    fn format_attribution(statement: &InterpretationStatement) -> String;
}
```

---

## Context Mapping

### Relationships Between Contexts

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                           CONTEXT MAP                                            │
└─────────────────────────────────────────────────────────────────────────────────┘

                    ┌──────────────────┐
                    │  Audio Ingestion │
                    │     Context      │
                    └────────┬─────────┘
                             │
                             │ [U/D] CallSegment
                             │ Published Language
                             ▼
                    ┌──────────────────┐
                    │    Embedding     │
                    │     Context      │
                    └────────┬─────────┘
                             │
                             │ [U/D] Embedding
                             │ Published Language
                             ▼
                    ┌──────────────────┐
                    │   Vector Space   │◄──────────────────────┐
                    │     Context      │                       │
                    └────────┬─────────┘                       │
                             │                                 │
            ┌────────────────┼────────────────┐                │
            │                │                │                │
            │ [ACL]          │ [ACL]          │ [ACL]          │
            ▼                ▼                ▼                │
   ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐       │
   │   Learning  │  │   Analysis  │  │ Interpretation  │       │
   │   Context   │  │   Context   │  │    Context      │       │
   └──────┬──────┘  └──────┬──────┘  └────────┬────────┘       │
          │                │                   │                │
          │ [Partnership]  │ [Customer/        │ [Customer/     │
          │                │  Supplier]        │  Supplier]     │
          └────────────────┴───────────────────┘                │
                           │                                    │
                           │ RefinedEmbedding                   │
                           └────────────────────────────────────┘


LEGEND:
  [U/D]  = Upstream/Downstream (Published Language)
  [ACL]  = Anti-Corruption Layer
  [Partnership] = Shared development, mutual dependency
  [Customer/Supplier] = Clear provider/consumer relationship
```

### Integration Patterns

| Upstream | Downstream | Pattern | Shared Kernel |
|----------|------------|---------|---------------|
| Audio Ingestion | Embedding | Published Language | `CallSegment`, `SegmentId`, `QualityGrade` |
| Embedding | Vector Space | Published Language | `Embedding`, `EmbeddingId`, `Vec<f32>` |
| Vector Space | Learning | ACL + Partnership | `VectorIndex`, `NeighborGraph` |
| Vector Space | Analysis | ACL + Customer/Supplier | `SearchResults`, `SimilarityEdge` |
| Vector Space | Interpretation | ACL + Customer/Supplier | `SearchResults`, `RetrievedNeighbor` |
| Learning | Vector Space | Partnership | `RefinedEmbedding` (feedback loop) |
| Analysis | Interpretation | Customer/Supplier | `Cluster`, `Motif`, `SequenceAnalysis` |

---

## Anti-Corruption Layers

### Learning Context ACL

```rust
/// Translates Vector Space concepts to Learning domain
mod learning_acl {
    use crate::vector_space::{VectorIndex, IndexedVector, SimilarityEdge};
    use crate::learning::{TransitionGraph, GraphNode, GraphEdge};

    pub struct VectorSpaceAdapter {
        index: Arc<VectorIndex>,
    }

    impl VectorSpaceAdapter {
        /// Convert HNSW neighbor graph to GNN-compatible format
        pub fn to_transition_graph(&self, max_neighbors: u32) -> TransitionGraph {
            let nodes: Vec<GraphNode> = self.index
                .iter_vectors()
                .map(|v| GraphNode {
                    id: v.id.into(),
                    embedding: v.embedding_id,
                    features: self.extract_node_features(&v),
                })
                .collect();

            let edges: Vec<GraphEdge> = self.index
                .iter_similarity_edges()
                .filter(|e| e.distance < self.distance_threshold())
                .map(|e| GraphEdge {
                    source: e.source_id.into(),
                    target: e.target_id.into(),
                    edge_type: EdgeType::Similarity,
                    weight: 1.0 - e.distance, // Convert distance to similarity
                })
                .collect();

            TransitionGraph::new(nodes, edges)
        }

        /// Query neighbors without exposing HNSW internals
        pub fn get_trainable_neighbors(&self, vector_id: VectorId, k: u32)
            -> Vec<(GraphNodeId, f32)>
        {
            self.index
                .knn_search_by_id(vector_id, k)
                .map(|(vid, dist)| (vid.into(), 1.0 - dist))
                .collect()
        }
    }
}
```

### Analysis Context ACL

```rust
/// Translates Vector Space results to Analysis domain
mod analysis_acl {
    use crate::vector_space::{SearchResults, VectorIndex};
    use crate::analysis::{ClusterCandidate, SimilarityMatrix};

    pub struct SearchResultsAdapter;

    impl SearchResultsAdapter {
        /// Convert k-NN results to clustering input
        pub fn to_similarity_matrix(
            index: &VectorIndex,
            embeddings: &[EmbeddingId],
            k: u32
        ) -> SimilarityMatrix {
            let n = embeddings.len();
            let mut matrix = SimilarityMatrix::new(n);

            for (i, emb_id) in embeddings.iter().enumerate() {
                let neighbors = index.knn_search_by_embedding_id(*emb_id, k);
                for (neighbor_id, distance) in neighbors {
                    if let Some(j) = embeddings.iter().position(|e| *e == neighbor_id) {
                        matrix.set(i, j, 1.0 - distance);
                    }
                }
            }

            matrix
        }

        /// Extract cluster candidates from dense regions
        pub fn identify_dense_regions(
            index: &VectorIndex,
            min_density: f32
        ) -> Vec<ClusterCandidate> {
            index.iter_vectors()
                .filter_map(|v| {
                    let local_density = index.compute_local_density(v.id);
                    if local_density >= min_density {
                        Some(ClusterCandidate {
                            center_id: v.embedding_id,
                            density: local_density,
                            estimated_size: (local_density * 100.0) as u32,
                        })
                    } else {
                        None
                    }
                })
                .collect()
        }
    }
}
```

### Interpretation Context ACL

```rust
/// Translates Analysis results to Interpretation domain
mod interpretation_acl {
    use crate::analysis::{Cluster, Motif, SequenceAnalysis, ClusterAssignment};
    use crate::interpretation::{
        EvidencePack, RetrievedNeighbor, Exemplar, SequenceContext
    };

    pub struct AnalysisAdapter {
        clusters: Arc<HashMap<ClusterId, Cluster>>,
        motifs: Arc<HashMap<MotifId, Motif>>,
    }

    impl AnalysisAdapter {
        /// Build evidence pack from analysis artifacts
        pub fn build_evidence_pack(
            &self,
            query_segment: &CallSegment,
            neighbors: Vec<(SegmentId, f32)>,
            sequence: &SequenceAnalysis,
        ) -> EvidencePack {
            let retrieved_neighbors: Vec<RetrievedNeighbor> = neighbors
                .into_iter()
                .map(|(seg_id, distance)| {
                    let cluster_id = self.find_cluster_for_segment(seg_id);
                    RetrievedNeighbor {
                        segment_id: seg_id,
                        distance,
                        cluster_id,
                        spectogram_thumbnail: self.generate_thumbnail(seg_id),
                        metadata: self.get_segment_metadata(seg_id),
                    }
                })
                .collect();

            let exemplars: Vec<Exemplar> = self.get_relevant_exemplars(
                &retrieved_neighbors,
                5 // top 5 exemplars
            );

            let sequence_context = SequenceContext {
                previous_segments: sequence.get_predecessors(query_segment.id, 3),
                next_segments: sequence.get_successors(query_segment.id, 3),
                position_in_recording: sequence.relative_position(query_segment.id),
                local_motifs: self.find_local_motifs(query_segment.id, sequence),
            };

            EvidencePack {
                id: EvidencePackId::new(),
                query_segment_id: query_segment.id,
                query_type: QueryType::Segment,
                retrieved_neighbors,
                cluster_exemplars: exemplars,
                sequence_context,
                signal_quality: self.assess_quality(query_segment),
                generated_at: Utc::now(),
            }
        }

        /// Convert cluster to citable evidence
        pub fn cluster_to_citation(&self, cluster_id: ClusterId) -> Citation {
            let cluster = self.clusters.get(&cluster_id)
                .expect("Cluster not found");

            Citation {
                source_type: CitationSource::Cluster,
                source_id: cluster_id.to_string(),
                relevance: cluster.cohesion,
                excerpt: Some(format!(
                    "Cluster {} with {} members (cohesion: {:.2})",
                    cluster_id, cluster.member_count, cluster.cohesion
                )),
            }
        }
    }
}
```

---

## Shared Kernel

The following types are shared across multiple contexts and form the ubiquitous language foundation:

```rust
/// Shared identifiers
pub mod shared_kernel {
    use uuid::Uuid;

    // Core identifiers shared across all contexts
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct RecordingId(Uuid);

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct SegmentId(Uuid);

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct EmbeddingId(Uuid);

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct ClusterId(Uuid);

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct MotifId(Uuid);

    // Shared value objects
    #[derive(Clone, Debug)]
    pub struct GeoLocation {
        pub latitude: f64,
        pub longitude: f64,
        pub altitude: Option<f32>,
    }

    #[derive(Clone, Debug)]
    pub struct TimeRange {
        pub start: DateTime<Utc>,
        pub end: DateTime<Utc>,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum QualityGrade {
        Excellent,  // SNR > 20dB, no clipping
        Good,       // SNR > 10dB, minimal issues
        Fair,       // SNR > 5dB, some artifacts
        Poor,       // SNR < 5dB or significant issues
        Unusable,   // Too degraded for analysis
    }

    // Embedding vector type (1536-D for Perch 2.0)
    pub type EmbeddingVector = Vec<f32>;
    pub const EMBEDDING_DIM: usize = 1536;

    // Audio format constants for Perch 2.0
    pub const TARGET_SAMPLE_RATE: u32 = 32000;
    pub const TARGET_WINDOW_SECONDS: f32 = 5.0;
    pub const TARGET_WINDOW_SAMPLES: usize = 160000;
    pub const MEL_BINS: usize = 128;
    pub const MEL_FRAMES: usize = 500;
}
```

---

## Event Flow

```
Recording Upload
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│ AUDIO INGESTION CONTEXT                                       │
│   RecordingReceived → RecordingValidated → SegmentExtracted  │
└──────────────────────────────────────────────────────────────┘
       │ CallSegment (Published Language)
       ▼
┌──────────────────────────────────────────────────────────────┐
│ EMBEDDING CONTEXT                                             │
│   EmbeddingRequested → InferenceStarted → EmbeddingGenerated │
└──────────────────────────────────────────────────────────────┘
       │ Embedding (Published Language)
       ▼
┌──────────────────────────────────────────────────────────────┐
│ VECTOR SPACE CONTEXT                                          │
│   VectorInserted → NeighborGraphUpdated → SimilarityEdgeCreated│
└──────────────────────────────────────────────────────────────┘
       │                    │                         │
       │                    │                         │
       ▼                    ▼                         ▼
┌─────────────┐    ┌─────────────────┐    ┌─────────────────────┐
│  LEARNING   │    │    ANALYSIS     │    │   INTERPRETATION    │
│   CONTEXT   │    │     CONTEXT     │    │      CONTEXT        │
│             │    │                 │    │                     │
│ Training-   │    │ Clustering-     │    │ EvidencePack-       │
│ Started     │    │ Completed       │    │ Assembled           │
│     │       │    │     │           │    │     │               │
│     ▼       │    │     ▼           │    │     ▼               │
│ Embeddings- │    │ MotifDiscovered │    │ Interpretation-     │
│ Refined     │    │                 │    │ Generated           │
└─────────────┘    └─────────────────┘    └─────────────────────┘
       │
       │ RefinedEmbedding (feedback to Vector Space)
       └──────────────────────────────────────────────────────────┐
                                                                  ▼
                                                     ┌──────────────────┐
                                                     │ VECTOR SPACE     │
                                                     │ (Index Update)   │
                                                     └──────────────────┘
```

---

## Consequences

### Benefits

1. **Clear Ownership**: Each bounded context has explicit responsibilities and can be developed by independent teams
2. **Reduced Coupling**: Anti-corruption layers prevent domain model pollution across boundaries
3. **Testability**: Each context can be tested in isolation with well-defined interfaces
4. **Scalability**: Contexts can be deployed and scaled independently
5. **Evolvability**: Internal implementations can change without affecting other contexts
6. **Domain Alignment**: Ubiquitous language matches the bioacoustics domain

### Risks

1. **Complexity**: Six contexts introduce coordination overhead
2. **Data Duplication**: Some data may be replicated across context boundaries
3. **Event Consistency**: Eventual consistency between contexts requires careful handling
4. **Learning Curve**: Team must understand DDD concepts and context boundaries

### Mitigations

1. Use event sourcing for cross-context communication
2. Implement saga patterns for multi-context transactions
3. Maintain comprehensive integration tests at context boundaries
4. Document context mappings and keep them updated

---

## References

- Evans, Eric. "Domain-Driven Design: Tackling Complexity in the Heart of Software" (2003)
- Vernon, Vaughn. "Implementing Domain-Driven Design" (2013)
- Perch 2.0 Paper: arXiv:2508.04665
- RuVector Documentation: https://github.com/ruvnet/ruvector

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-15 | Architecture Team | Initial ADR |
