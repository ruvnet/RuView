# Hexagonal Architecture - Genomic Analysis Platform

## Overview

The DNA analyzer follows hexagonal (ports and adapters) architecture to maintain domain logic independence from infrastructure concerns. The core domain remains pure Rust with no external dependencies, while adapters integrate with ruvector components.

## Hexagonal Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                        PRIMARY ACTORS (Inbound)                      │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐           │
│  │  CLI Client   │  │  REST API     │  │  Web UI       │           │
│  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘           │
│          │                   │                   │                    │
└──────────┼───────────────────┼───────────────────┼────────────────────┘
           │                   │                   │
           ▼                   ▼                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      PRIMARY PORTS (Inbound)                         │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  PipelinePort trait                                          │  │
│  │    - run_analysis(input: SequenceData) -> Result            │  │
│  │    - get_status() -> PipelineStatus                         │  │
│  │    - get_results() -> AnalysisResult                        │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         CORE DOMAIN (Pure)                           │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  Domain Model (types.rs, error.rs)                          │  │
│  │    - GenomicPosition, QualityScore, Nucleotide             │  │
│  │    - No external dependencies                               │  │
│  │    - Pure business logic                                    │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                       │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  Domain Services (7 Bounded Contexts)                       │  │
│  │    ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │  │
│  │    │  Sequence   │  │  Alignment  │  │  Variant    │       │  │
│  │    │  (kmer.rs)  │  │ (align.rs)  │  │(variant.rs) │       │  │
│  │    └─────────────┘  └─────────────┘  └─────────────┘       │  │
│  │    ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │  │
│  │    │  Protein    │  │ Epigenomic  │  │  Pharma     │       │  │
│  │    │(protein.rs) │  │(epigen.rs)  │  │ (pharma.rs) │       │  │
│  │    └─────────────┘  └─────────────┘  └─────────────┘       │  │
│  │                                                               │  │
│  │    ┌──────────────────────────────────────────────┐         │  │
│  │    │  Pipeline Orchestrator (pipeline.rs)        │         │  │
│  │    │    - Coordinates all contexts               │         │  │
│  │    │    - Manages workflow execution             │         │  │
│  │    └──────────────────────────────────────────────┘         │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    SECONDARY PORTS (Outbound)                        │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  VectorStoragePort trait                                     │  │
│  │    - store_embedding(key, vec) -> Result                    │  │
│  │    - search_similar(query, k) -> Vec<Match>                 │  │
│  └──────────────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  AttentionPort trait                                         │  │
│  │    - compute_attention(Q, K, V) -> Tensor                   │  │
│  │    - flash_attention(Q, K, V) -> Tensor                     │  │
│  └──────────────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  GraphNeuralPort trait                                       │  │
│  │    - gnn_inference(graph) -> Predictions                    │  │
│  │    - graph_search(query) -> Vec<Node>                       │  │
│  └──────────────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  PersistencePort trait                                       │  │
│  │    - save(data) -> Result                                   │  │
│  │    - load(id) -> Result<Data>                               │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   SECONDARY ADAPTERS (Outbound)                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │
│  │  RuVector   │  │  RuVector   │  │  RuVector   │                 │
│  │  Core       │  │  Attention  │  │  GNN        │                 │
│  │  (HNSW)     │  │  (Flash)    │  │  (Graph)    │                 │
│  └─────────────┘  └─────────────┘  └─────────────┘                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │
│  │  SQLite     │  │  PostgreSQL │  │  File       │                 │
│  │  Adapter    │  │  Adapter    │  │  System     │                 │
│  └─────────────┘  └─────────────┘  └─────────────┘                 │
└─────────────────────────────────────────────────────────────────────┘

DEPENDENCY RULE: Dependencies point INWARD
  Core Domain ← Secondary Ports ← Secondary Adapters
  Core Domain ← Primary Ports ← Primary Adapters
```

## Layer Definitions

### 1. Core Domain Layer

**Location**: `/src/types.rs`, `/src/error.rs`

**Characteristics**:
- Zero external dependencies (except std)
- Pure business logic
- No knowledge of infrastructure
- Immutable value objects
- Rich domain model

**Example Types**:

```rust
// types.rs - Pure domain types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GenomicPosition {
    pub chromosome: String,
    pub position: usize,
}

impl GenomicPosition {
    pub fn new(chromosome: String, position: usize) -> Result<Self, DomainError> {
        if position == 0 {
            return Err(DomainError::InvalidPosition);
        }
        Ok(Self { chromosome, position })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct QualityScore(pub f64);

impl QualityScore {
    pub fn from_phred(score: f64) -> Result<Self, DomainError> {
        if score < 0.0 {
            return Err(DomainError::InvalidQuality);
        }
        Ok(Self(score))
    }

    pub fn error_probability(&self) -> f64 {
        10_f64.powf(-self.0 / 10.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nucleotide {
    A, C, G, T,
}

impl Nucleotide {
    pub fn complement(&self) -> Self {
        match self {
            Nucleotide::A => Nucleotide::T,
            Nucleotide::T => Nucleotide::A,
            Nucleotide::C => Nucleotide::G,
            Nucleotide::G => Nucleotide::C,
        }
    }
}

// error.rs - Domain errors
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Invalid genomic position")]
    InvalidPosition,

    #[error("Invalid quality score")]
    InvalidQuality,

    #[error("Invalid sequence: {0}")]
    InvalidSequence(String),
}
```

### 2. Domain Services Layer

**Location**: 7 bounded context modules

**Characteristics**:
- Implements business logic using domain types
- Depends on ports (traits), not implementations
- Orchestrates domain operations
- No infrastructure code

**Example Services**:

```rust
// kmer.rs - Sequence Context service
pub struct KmerEncoder {
    k: usize,
    alphabet_size: usize,
}

impl KmerEncoder {
    pub fn new(k: usize) -> Result<Self, DomainError> {
        if k < 3 || k > 32 {
            return Err(DomainError::InvalidKmerSize);
        }
        Ok(Self { k, alphabet_size: 4 })
    }

    // Pure domain logic - no infrastructure
    pub fn encode(&self, kmer: &[u8]) -> Result<u64, DomainError> {
        if kmer.len() != self.k {
            return Err(DomainError::InvalidKmerLength);
        }

        let mut hash = 0u64;
        for &base in kmer {
            let encoded = match base {
                b'A' | b'a' => 0,
                b'C' | b'c' => 1,
                b'G' | b'g' => 2,
                b'T' | b't' => 3,
                _ => return Err(DomainError::InvalidNucleotide),
            };
            hash = hash * self.alphabet_size as u64 + encoded;
        }
        Ok(hash)
    }
}

// variant.rs - Variant Context service (depends on ports)
pub struct VariantCaller<G: GraphNeuralPort> {
    min_quality: f64,
    min_depth: usize,
    gnn_service: Arc<G>, // Port dependency
}

impl<G: GraphNeuralPort> VariantCaller<G> {
    pub fn call_variants(
        &self,
        alignments: &[Alignment],
    ) -> Result<Vec<Variant>, DomainError> {
        // Business logic using port abstraction
        let candidate_positions = self.identify_candidates(alignments)?;

        // Use GNN port for variant classification
        let predictions = self.gnn_service.classify_variants(candidate_positions)?;

        // Apply business rules
        predictions
            .into_iter()
            .filter(|v| v.quality >= self.min_quality && v.depth >= self.min_depth)
            .collect()
    }
}
```

### 3. Primary Ports (Inbound)

**Location**: `pipeline.rs` trait definitions

**Characteristics**:
- Define application API
- Trait-based contracts
- Technology-agnostic
- Used by primary adapters (CLI, API, UI)

**Example Ports**:

```rust
// Primary port for pipeline orchestration
pub trait PipelinePort {
    fn run_analysis(&mut self, input: SequenceData) -> Result<AnalysisResult, Error>;
    fn get_status(&self) -> PipelineStatus;
    fn get_results(&self) -> Option<&AnalysisResult>;
    fn checkpoint(&self) -> Result<String, Error>;
    fn restore(&mut self, checkpoint_id: &str) -> Result<(), Error>;
}

// Primary port for variant analysis
pub trait VariantAnalysisPort {
    fn call_variants(&self, sequence: &[u8], reference: &[u8])
        -> Result<Vec<Variant>, Error>;
    fn annotate_variant(&self, variant: &Variant)
        -> Result<Annotation, Error>;
}

// Primary port for pharmacogenomics
pub trait PharmacogenomicsPort {
    fn analyze_drug_response(&self, variants: &[Variant])
        -> Result<Vec<DrugResponse>, Error>;
    fn get_recommendations(&self, drug: &str, diplotype: &Diplotype)
        -> Result<ClinicalRecommendation, Error>;
}
```

### 4. Secondary Ports (Outbound)

**Location**: Trait definitions in each bounded context module

**Characteristics**:
- Define infrastructure abstractions
- Implemented by secondary adapters
- Enable dependency inversion
- Mock-friendly for testing

**Example Ports**:

```rust
// Port for vector storage (HNSW)
pub trait VectorStoragePort: Send + Sync {
    fn store_embedding(&self, key: String, embedding: Vec<f32>)
        -> Result<(), Error>;

    fn search_similar(&self, query: Vec<f32>, k: usize)
        -> Result<Vec<SimilarityMatch>, Error>;

    fn delete_embedding(&self, key: &str) -> Result<(), Error>;
}

#[derive(Debug, Clone)]
pub struct SimilarityMatch {
    pub key: String,
    pub similarity: f64,
    pub metadata: Option<String>,
}

// Port for attention mechanisms
pub trait AttentionPort: Send + Sync {
    fn compute_attention(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Result<Vec<f32>, Error>;

    fn flash_attention(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Result<Vec<f32>, Error>;
}

// Port for graph neural networks
pub trait GraphNeuralPort: Send + Sync {
    fn gnn_inference(&self, graph: &Graph) -> Result<Vec<Prediction>, Error>;

    fn graph_search(&self, query_node: Node, k: usize)
        -> Result<Vec<Node>, Error>;

    fn classify_variants(&self, candidates: Vec<VariantCandidate>)
        -> Result<Vec<Variant>, Error>;
}

#[derive(Debug, Clone)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<(usize, usize, f64)>,
}

// Port for persistence
pub trait PersistencePort: Send + Sync {
    fn save_results(&self, results: &AnalysisResult) -> Result<String, Error>;
    fn load_results(&self, id: &str) -> Result<AnalysisResult, Error>;
    fn save_checkpoint(&self, pipeline: &GenomicPipeline) -> Result<String, Error>;
    fn load_checkpoint(&self, id: &str) -> Result<GenomicPipeline, Error>;
}
```

### 5. Primary Adapters (Inbound)

**Location**: Binary crates or API modules

**Characteristics**:
- Convert external requests to domain calls
- Implement framework-specific code
- Handle serialization/deserialization
- Map errors to appropriate responses

**Example Adapters**:

```rust
// CLI adapter
pub struct CliAdapter {
    pipeline: Box<dyn PipelinePort>,
}

impl CliAdapter {
    pub fn run(&mut self, args: CliArgs) -> Result<(), Error> {
        // Convert CLI args to domain input
        let input = SequenceData {
            sequence: std::fs::read_to_string(&args.input)?,
            quality: None,
        };

        // Call domain through port
        let result = self.pipeline.run_analysis(input)?;

        // Format output for CLI
        self.print_results(&result);
        Ok(())
    }
}

// REST API adapter (hypothetical)
pub struct RestApiAdapter {
    pipeline: Box<dyn PipelinePort>,
}

impl RestApiAdapter {
    pub async fn analyze_handler(&self, req: Request) -> Response {
        // Parse JSON request
        let input: SequenceData = match serde_json::from_slice(req.body()) {
            Ok(data) => data,
            Err(e) => return Response::error(400, e.to_string()),
        };

        // Call domain
        match self.pipeline.run_analysis(input) {
            Ok(result) => Response::ok(serde_json::to_string(&result).unwrap()),
            Err(e) => Response::error(500, e.to_string()),
        }
    }
}
```

### 6. Secondary Adapters (Outbound)

**Location**: Infrastructure modules or separate crates

**Characteristics**:
- Implement secondary ports
- Integrate with external libraries (ruvector)
- Handle technical concerns (networking, storage, etc.)
- Isolate infrastructure code

**Example Adapters**:

```rust
// RuVector HNSW adapter
pub struct RuVectorAdapter {
    db: Arc<AgentDB>,
}

impl VectorStoragePort for RuVectorAdapter {
    fn store_embedding(&self, key: String, embedding: Vec<f32>)
        -> Result<(), Error>
    {
        self.db.store(&key, &embedding)
            .map_err(|e| Error::StorageError(e.to_string()))
    }

    fn search_similar(&self, query: Vec<f32>, k: usize)
        -> Result<Vec<SimilarityMatch>, Error>
    {
        let results = self.db.search(&query, k)
            .map_err(|e| Error::SearchError(e.to_string()))?;

        Ok(results.into_iter().map(|r| SimilarityMatch {
            key: r.key,
            similarity: r.distance,
            metadata: r.metadata,
        }).collect())
    }

    fn delete_embedding(&self, key: &str) -> Result<(), Error> {
        self.db.delete(key)
            .map_err(|e| Error::StorageError(e.to_string()))
    }
}

// RuVector Attention adapter
pub struct RuVectorAttentionAdapter {
    attention_service: Arc<AttentionService>,
}

impl AttentionPort for RuVectorAttentionAdapter {
    fn compute_attention(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Result<Vec<f32>, Error> {
        // Convert to ruvector tensor format
        let q_tensor = Tensor::from_slice(query);
        let k_tensor = Tensor::from_matrix(keys);
        let v_tensor = Tensor::from_matrix(values);

        // Call ruvector attention
        let output = self.attention_service
            .scaled_dot_product(&q_tensor, &k_tensor, &v_tensor)
            .map_err(|e| Error::AttentionError(e.to_string()))?;

        // Convert back to Vec<f32>
        Ok(output.to_vec())
    }

    fn flash_attention(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Result<Vec<f32>, Error> {
        // Use ruvector flash attention for efficiency
        let q_tensor = Tensor::from_slice(query);
        let k_tensor = Tensor::from_matrix(keys);
        let v_tensor = Tensor::from_matrix(values);

        let output = self.attention_service
            .flash_attention(&q_tensor, &k_tensor, &v_tensor)
            .map_err(|e| Error::AttentionError(e.to_string()))?;

        Ok(output.to_vec())
    }
}

// RuVector GNN adapter
pub struct RuVectorGnnAdapter {
    gnn_service: Arc<GnnService>,
}

impl GraphNeuralPort for RuVectorGnnAdapter {
    fn gnn_inference(&self, graph: &Graph) -> Result<Vec<Prediction>, Error> {
        // Convert domain graph to ruvector format
        let nodes: Vec<Vec<f32>> = graph.nodes.iter()
            .map(|n| n.features.clone())
            .collect();

        let edges: Vec<(usize, usize)> = graph.edges.iter()
            .map(|(i, j, _)| (*i, *j))
            .collect();

        // Call ruvector GNN
        let predictions = self.gnn_service
            .predict(&nodes, &edges)
            .map_err(|e| Error::GnnError(e.to_string()))?;

        Ok(predictions)
    }

    fn classify_variants(&self, candidates: Vec<VariantCandidate>)
        -> Result<Vec<Variant>, Error>
    {
        // Build graph from variant candidates
        let graph = self.build_variant_graph(&candidates);

        // Use GNN to classify
        let predictions = self.gnn_inference(&graph)?;

        // Convert predictions back to variants
        candidates.into_iter()
            .zip(predictions)
            .filter(|(_, pred)| pred.confidence > 0.8)
            .map(|(cand, pred)| self.to_variant(cand, pred))
            .collect()
    }
}

// File system persistence adapter
pub struct FileSystemAdapter {
    output_dir: PathBuf,
}

impl PersistencePort for FileSystemAdapter {
    fn save_results(&self, results: &AnalysisResult) -> Result<String, Error> {
        let id = Uuid::new_v4().to_string();
        let path = self.output_dir.join(format!("{}.json", id));

        let json = serde_json::to_string_pretty(results)
            .map_err(|e| Error::SerializationError(e.to_string()))?;

        std::fs::write(&path, json)
            .map_err(|e| Error::IoError(e.to_string()))?;

        Ok(id)
    }

    fn load_results(&self, id: &str) -> Result<AnalysisResult, Error> {
        let path = self.output_dir.join(format!("{}.json", id));

        let json = std::fs::read_to_string(&path)
            .map_err(|e| Error::IoError(e.to_string()))?;

        serde_json::from_str(&json)
            .map_err(|e| Error::DeserializationError(e.to_string()))
    }
}
```

## Dependency Injection

**Construction at Application Startup**:

```rust
// main.rs or application initialization
pub fn build_pipeline() -> Result<impl PipelinePort, Error> {
    // Create secondary adapters (infrastructure)
    let vector_store = Arc::new(RuVectorAdapter::new()?);
    let attention = Arc::new(RuVectorAttentionAdapter::new()?);
    let gnn = Arc::new(RuVectorGnnAdapter::new()?);
    let persistence = Arc::new(FileSystemAdapter::new("./output")?);

    // Create domain services with port dependencies
    let kmer_encoder = KmerEncoder::new(21)?;

    let aligner = AttentionAligner::new(
        attention.clone(),
        -1.0, // gap penalty
        2.0,  // match bonus
    );

    let variant_caller = VariantCaller::new(
        30.0,  // min quality
        10,    // min depth
        gnn.clone(),
    );

    let protein_predictor = ContactPredictor::new(
        gnn.clone(),
        attention.clone(),
        8.0, // distance threshold
    );

    // Create pipeline (aggregates all services)
    let pipeline = GenomicPipeline::new(
        kmer_encoder,
        aligner,
        variant_caller,
        protein_predictor,
        persistence,
    )?;

    Ok(pipeline)
}
```

## Testing Strategy by Layer

### 1. Core Domain Testing

**Strategy**: Pure unit tests, no mocks needed

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nucleotide_complement() {
        assert_eq!(Nucleotide::A.complement(), Nucleotide::T);
        assert_eq!(Nucleotide::G.complement(), Nucleotide::C);
    }

    #[test]
    fn test_quality_score_error_probability() {
        let q30 = QualityScore::from_phred(30.0).unwrap();
        assert!((q30.error_probability() - 0.001).abs() < 1e-6);
    }

    #[test]
    fn test_genomic_position_validation() {
        let valid = GenomicPosition::new("chr1".to_string(), 1000);
        assert!(valid.is_ok());

        let invalid = GenomicPosition::new("chr1".to_string(), 0);
        assert!(invalid.is_err());
    }
}
```

### 2. Domain Service Testing

**Strategy**: Use mock implementations of ports

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use mockall::mock;

    // Mock GNN port
    mock! {
        GnnService {}

        impl GraphNeuralPort for GnnService {
            fn classify_variants(&self, candidates: Vec<VariantCandidate>)
                -> Result<Vec<Variant>, Error>;
        }
    }

    #[test]
    fn test_variant_caller_filters_low_quality() {
        // Setup mock
        let mut mock_gnn = MockGnnService::new();
        mock_gnn.expect_classify_variants()
            .returning(|_| Ok(vec![
                Variant { quality: 35.0, depth: 15, ..Default::default() },
                Variant { quality: 20.0, depth: 15, ..Default::default() }, // Below threshold
            ]));

        // Test service
        let caller = VariantCaller::new(30.0, 10, Arc::new(mock_gnn));
        let results = caller.call_variants(&alignments).unwrap();

        // Only high-quality variant should pass
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].quality, 35.0);
    }
}
```

### 3. Adapter Testing

**Strategy**: Integration tests with real infrastructure or test doubles

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ruvector_adapter_roundtrip() {
        // Use in-memory ruvector instance
        let adapter = RuVectorAdapter::new_in_memory().unwrap();

        // Store embedding
        let embedding = vec![0.1, 0.2, 0.3, 0.4];
        adapter.store_embedding("test_key".to_string(), embedding.clone()).unwrap();

        // Search should find it
        let results = adapter.search_similar(embedding, 1).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "test_key");
        assert!(results[0].similarity > 0.99);
    }
}
```

### 4. End-to-End Testing

**Strategy**: Full pipeline with real or test infrastructure

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_full_pipeline() {
        // Build pipeline with real adapters
        let pipeline = build_pipeline().unwrap();

        // Load test data
        let input = SequenceData {
            sequence: include_str!("../test_data/sample.fasta").to_string(),
            quality: None,
        };

        // Run analysis
        let result = pipeline.run_analysis(input).unwrap();

        // Verify results
        assert!(result.variants.len() > 0);
        assert!(result.protein_structures.len() > 0);
    }
}
```

## Benefits of Hexagonal Architecture

### 1. Testability
- Domain logic testable without infrastructure
- Ports enable easy mocking
- Fast unit tests (no I/O)

### 2. Maintainability
- Clear separation of concerns
- Changes to infrastructure don't affect domain
- Easy to understand dependencies

### 3. Flexibility
- Swap implementations without changing domain
- Support multiple adapters (CLI, API, UI)
- Easy to add new infrastructure

### 4. Domain Focus
- Business logic remains pure
- Rich domain model
- Ubiquitous language preserved

## Adapter Implementation Matrix

| Port | RuVector Adapter | Alternative Adapter | Test Adapter |
|------|------------------|---------------------|--------------|
| VectorStoragePort | RuVectorAdapter (HNSW) | PostgreSQL pgvector | InMemoryVectorStore |
| AttentionPort | RuVectorAttentionAdapter | PyTorch bindings | MockAttention |
| GraphNeuralPort | RuVectorGnnAdapter | DGL bindings | MockGNN |
| PersistencePort | FileSystemAdapter | PostgreSQL | InMemoryPersistence |

## Configuration Management

```rust
// Configuration for adapter selection
pub struct AdapterConfig {
    pub vector_backend: VectorBackend,
    pub persistence_backend: PersistenceBackend,
    pub enable_flash_attention: bool,
}

pub enum VectorBackend {
    RuVector,
    PgVector,
    InMemory,
}

pub enum PersistenceBackend {
    FileSystem { path: PathBuf },
    PostgreSQL { connection_string: String },
    InMemory,
}

// Factory for building adapters
pub struct AdapterFactory;

impl AdapterFactory {
    pub fn build_vector_storage(config: &AdapterConfig)
        -> Result<Box<dyn VectorStoragePort>, Error>
    {
        match config.vector_backend {
            VectorBackend::RuVector => {
                Ok(Box::new(RuVectorAdapter::new()?))
            }
            VectorBackend::PgVector => {
                Ok(Box::new(PgVectorAdapter::new(&config.db_url)?))
            }
            VectorBackend::InMemory => {
                Ok(Box::new(InMemoryVectorStore::new()))
            }
        }
    }

    pub fn build_persistence(config: &AdapterConfig)
        -> Result<Box<dyn PersistencePort>, Error>
    {
        match &config.persistence_backend {
            PersistenceBackend::FileSystem { path } => {
                Ok(Box::new(FileSystemAdapter::new(path)?))
            }
            PersistenceBackend::PostgreSQL { connection_string } => {
                Ok(Box::new(PostgresAdapter::new(connection_string)?))
            }
            PersistenceBackend::InMemory => {
                Ok(Box::new(InMemoryPersistence::new()))
            }
        }
    }
}
```

## Summary

The hexagonal architecture provides:

1. **Pure Core Domain**: Business logic independent of infrastructure (types.rs, error.rs)
2. **Domain Services**: Seven bounded contexts implementing genomic analysis
3. **Primary Ports**: Application API (pipeline.rs traits)
4. **Secondary Ports**: Infrastructure abstractions (VectorStoragePort, AttentionPort, etc.)
5. **Primary Adapters**: CLI, API, UI interfaces
6. **Secondary Adapters**: RuVector integrations (HNSW, Flash Attention, GNN)

All dependencies point inward toward the core domain, enabling testability, maintainability, and flexibility in implementation choices.
