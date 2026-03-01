# Semantic Holography

## Overview

### Problem Statement
Current embeddings are single-resolution representations: they capture meaning at one granularity level. This creates several limitations:
1. **Fixed granularity**: Cannot adjust detail level for different queries
2. **Information loss**: Fine details lost in compression to fixed dimensions
3. **Inefficient storage**: Store separate embeddings for different resolutions
4. **No multi-scale reasoning**: Cannot reason about both "forest" and "trees"

### Proposed Solution
Encode multi-resolution semantic information in a single vector using frequency decomposition, inspired by holography:
- **Low frequencies**: Coarse semantic meaning (topic, category)
- **Mid frequencies**: Structural information (relationships, patterns)
- **High frequencies**: Fine-grained details (specific terms, entities)

Queries can select their desired resolution by filtering frequency bands, similar to how holographic images reveal different information at different viewing angles.

### Expected Benefits
- **Multi-scale queries**: Single embedding serves all granularities
- **50% storage reduction**: One embedding instead of multiple scales
- **Adaptive detail**: Query coarse categories or fine details from same vector
- **Information preservation**: Lossless storage across scales
- **Hierarchical reasoning**: Natural zoom in/out capability

### Novelty Claim
First application of holographic principles to semantic embeddings. Unlike:
- **Hierarchical embeddings**: Require separate vectors per level
- **Compressed sensing**: Random projections, no semantic structure
- **Wavelet transforms**: Domain-agnostic, not optimized for semantics

Semantic Holography uses learned frequency decomposition to pack multi-scale semantic information into a single vector.

## Technical Design

### Architecture Diagram
```
┌────────────────────────────────────────────────────────────────────┐
│                      Semantic Holography                            │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │              Frequency Decomposition                        │   │
│  │                                                             │   │
│  │   Input Text: "The quick brown fox jumps..."              │   │
│  │         │                                                   │   │
│  │         ▼                                                   │   │
│  │   ┌──────────────────────────────┐                         │   │
│  │   │   Standard Embedding Model   │                         │   │
│  │   │   (e.g., BERT, Sentence-T5)  │                         │   │
│  │   └──────────────────────────────┘                         │   │
│  │         │                                                   │   │
│  │         ▼                                                   │   │
│  │   Base Embedding: e ∈ ℝ^d                                  │   │
│  │   [0.23, -0.45, 0.67, -0.12, ...]                          │   │
│  │         │                                                   │   │
│  │         ▼                                                   │   │
│  │   ┌──────────────────────────────────────────┐            │   │
│  │   │  Holographic Encoding Transform (HET)    │            │   │
│  │   │                                           │            │   │
│  │   │  FFT(e) = [E₀, E₁, E₂, ..., E_{d-1}]    │            │   │
│  │   │                                           │            │   │
│  │   │  Low freq:  E₀...E_{d/8}   (coarse)     │            │   │
│  │   │  Mid freq:  E_{d/8}...E_{d/2} (struct)  │            │   │
│  │   │  High freq: E_{d/2}...E_d   (detail)    │            │   │
│  │   └──────────────────────────────────────────┘            │   │
│  └────────────────────────────────────────────────────────────┘   │
│                            │                                        │
│                            ▼                                        │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │           Multi-Resolution Query Interface                  │   │
│  │                                                             │   │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌────────────┐│   │
│  │  │  Coarse Query   │  │  Balanced Query │  │ Fine Query ││   │
│  │  │  (Topic-level)  │  │  (Standard)     │  │ (Precise)  ││   │
│  │  │                 │  │                 │  │            ││   │
│  │  │  Use: 0-12.5%   │  │  Use: 0-50%     │  │ Use: all   ││   │
│  │  │  frequencies    │  │  frequencies    │  │ freqs      ││   │
│  │  │                 │  │                 │  │            ││   │
│  │  │  ~~~~~~~~~~~~   │  │  ~~~~~~~~~~     │  │ ~~~~~~~~   ││   │
│  │  │                 │  │     ~~~~~~      │  │  ~~~~~~    ││   │
│  │  │  (smooth)       │  │        ~~~      │  │   ~~~~     ││   │
│  │  │                 │  │          ~      │  │    ~~      ││   │
│  │  │                 │  │                 │  │     ~      ││   │
│  │  └─────────────────┘  └─────────────────┘  └────────────┘│   │
│  └────────────────────────────────────────────────────────────┘   │
│                            │                                        │
│                            ▼                                        │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │              Holographic Reconstruction                     │   │
│  │                                                             │   │
│  │  Query: "machine learning" at COARSE resolution            │   │
│  │     │                                                       │   │
│  │     ▼                                                       │   │
│  │  1. Transform query to frequency domain: Q = FFT(q)        │   │
│  │  2. Filter: Q_low = Q[0:d/8], zero out rest               │   │
│  │  3. Compare: similarity(Q_low, E_low) for all docs        │   │
│  │     │                                                       │   │
│  │     ▼                                                       │   │
│  │  Results: [                                                │   │
│  │    "AI and machine learning overview" (0.92)              │   │
│  │    "Deep learning fundamentals" (0.89)                    │   │
│  │    "Neural networks" (0.85)                               │   │
│  │  ]                                                         │   │
│  │     ⬆ All about ML topic, ignore specific algorithms      │   │
│  │                                                             │   │
│  │  Query: "gradient descent optimization" at FINE resolution │   │
│  │     ▼                                                       │   │
│  │  Results: [                                                │   │
│  │    "Adam optimizer implementation" (0.94)                 │   │
│  │    "SGD with momentum tutorial" (0.91)                    │   │
│  │    "Learning rate scheduling" (0.88)                      │   │
│  │  ]                                                         │   │
│  │     ⬆ Specific optimization techniques, not general ML    │   │
│  └────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────┘
```

### Core Data Structures

```rust
/// Holographic embedding with multi-resolution information
#[derive(Clone, Debug)]
pub struct HolographicEmbedding {
    /// Frequency domain representation
    pub frequency_domain: Vec<Complex<f32>>,

    /// Spatial domain (original embedding)
    pub spatial_domain: Vec<f32>,

    /// Frequency band boundaries
    pub bands: FrequencyBands,

    /// Metadata
    pub metadata: HolographicMetadata,
}

/// Frequency band configuration
#[derive(Clone, Debug)]
pub struct FrequencyBands {
    /// Low frequency band (coarse semantics)
    pub low: (usize, usize),  // (start_idx, end_idx)

    /// Mid frequency band (structural information)
    pub mid: (usize, usize),

    /// High frequency band (fine details)
    pub high: (usize, usize),

    /// Total dimensions
    pub dimensions: usize,
}

impl FrequencyBands {
    /// Standard 12.5%-50%-100% split
    pub fn standard(dimensions: usize) -> Self {
        Self {
            low: (0, dimensions / 8),
            mid: (dimensions / 8, dimensions / 2),
            high: (dimensions / 2, dimensions),
            dimensions,
        }
    }

    /// Custom band configuration
    pub fn custom(low_pct: f32, mid_pct: f32, dimensions: usize) -> Self {
        let low_end = (dimensions as f32 * low_pct) as usize;
        let mid_end = (dimensions as f32 * mid_pct) as usize;

        Self {
            low: (0, low_end),
            mid: (low_end, mid_end),
            high: (mid_end, dimensions),
            dimensions,
        }
    }
}

/// Holographic metadata
#[derive(Clone, Debug)]
pub struct HolographicMetadata {
    /// Energy distribution across frequencies
    pub energy_spectrum: Vec<f32>,

    /// Dominant frequencies
    pub dominant_frequencies: Vec<usize>,

    /// Information content by band
    pub band_entropy: [f32; 3],  // [low, mid, high]

    /// Reconstruction quality
    pub reconstruction_error: f32,
}

/// Query resolution level
#[derive(Clone, Debug)]
pub enum Resolution {
    /// Coarse: Only low frequencies (topic-level)
    Coarse,

    /// Balanced: Low + mid frequencies (standard search)
    Balanced,

    /// Fine: All frequencies (precise matching)
    Fine,

    /// Custom: Specify frequency range
    Custom { bands: Vec<(usize, usize)> },
}

/// Holographic encoder configuration
#[derive(Clone, Debug)]
pub struct HolographicConfig {
    /// Base embedding model
    pub base_model: BaseEmbeddingModel,

    /// Frequency band configuration
    pub bands: FrequencyBands,

    /// Transform type
    pub transform: TransformType,

    /// Enable learned frequency allocation
    pub learned_bands: bool,

    /// Training configuration (if learned)
    pub training: Option<TrainingConfig>,
}

#[derive(Clone, Debug)]
pub enum BaseEmbeddingModel {
    /// Use existing embedding model
    External,

    /// BERT-based
    Bert { model_name: String },

    /// Sentence Transformers
    SentenceTransformer { model_name: String },

    /// Custom model
    Custom { model_path: String },
}

#[derive(Clone, Debug)]
pub enum TransformType {
    /// Fast Fourier Transform
    FFT,

    /// Discrete Cosine Transform
    DCT,

    /// Wavelet Transform
    Wavelet { wavelet_type: String },

    /// Learned transform (neural network)
    Learned { encoder: LearnedEncoder },
}

#[derive(Clone, Debug)]
pub struct LearnedEncoder {
    /// Neural network weights
    pub weights: Vec<Vec<f32>>,

    /// Activation functions
    pub activations: Vec<Activation>,
}

#[derive(Clone, Debug)]
pub enum Activation {
    ReLU,
    Tanh,
    Sigmoid,
    GELU,
}

/// Training configuration for learned frequency decomposition
#[derive(Clone, Debug)]
pub struct TrainingConfig {
    /// Training dataset
    pub dataset: String,

    /// Loss function
    pub loss: LossFunction,

    /// Number of epochs
    pub epochs: usize,

    /// Learning rate
    pub learning_rate: f32,

    /// Batch size
    pub batch_size: usize,
}

#[derive(Clone, Debug)]
pub enum LossFunction {
    /// Reconstruction loss (MSE between original and reconstructed)
    Reconstruction,

    /// Multi-scale contrastive loss
    MultiScaleContrastive {
        temperature: f32,
        weights: [f32; 3],  // [low, mid, high]
    },

    /// Information preservation loss
    InformationPreservation,

    /// Combined loss
    Combined(Vec<(LossFunction, f32)>),
}

/// Holographic search state
pub struct HolographicIndex {
    /// Holographic embeddings for all documents
    embeddings: Vec<HolographicEmbedding>,

    /// Configuration
    config: HolographicConfig,

    /// Fast frequency-domain similarity index
    frequency_index: FrequencyIndex,

    /// Cached reconstructions
    reconstruction_cache: LruCache<(NodeId, Resolution), Vec<f32>>,
}

/// Frequency-domain similarity index
pub struct FrequencyIndex {
    /// Band-specific HNSW graphs
    band_graphs: [HnswGraph; 3],  // [low, mid, high]

    /// Combined graph for full-spectrum search
    combined_graph: HnswGraph,
}
```

### Key Algorithms

```rust
// Pseudocode for semantic holography

/// Encode embedding into holographic representation
fn encode_holographic(
    spatial_embedding: &[f32],
    config: &HolographicConfig
) -> HolographicEmbedding {
    // Step 1: Transform to frequency domain
    let frequency_domain = match &config.transform {
        TransformType::FFT => {
            fft(spatial_embedding)
        },

        TransformType::DCT => {
            dct(spatial_embedding)
        },

        TransformType::Wavelet { wavelet_type } => {
            wavelet_transform(spatial_embedding, wavelet_type)
        },

        TransformType::Learned { encoder } => {
            learned_transform(spatial_embedding, encoder)
        },
    };

    // Step 2: Compute energy spectrum
    let energy_spectrum: Vec<f32> = frequency_domain.iter()
        .map(|c| c.norm_sqr())
        .collect();

    // Step 3: Find dominant frequencies
    let mut freq_energy: Vec<(usize, f32)> = energy_spectrum.iter()
        .enumerate()
        .map(|(i, &e)| (i, e))
        .collect();
    freq_energy.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let dominant_frequencies: Vec<usize> = freq_energy.iter()
        .take(10)
        .map(|(i, _)| *i)
        .collect();

    // Step 4: Compute band entropy (information content)
    let band_entropy = [
        compute_entropy(&energy_spectrum[config.bands.low.0..config.bands.low.1]),
        compute_entropy(&energy_spectrum[config.bands.mid.0..config.bands.mid.1]),
        compute_entropy(&energy_spectrum[config.bands.high.0..config.bands.high.1]),
    ];

    // Step 5: Verify reconstruction quality
    let reconstructed = inverse_transform(&frequency_domain, &config.transform);
    let reconstruction_error = mse(spatial_embedding, &reconstructed);

    HolographicEmbedding {
        frequency_domain,
        spatial_domain: spatial_embedding.to_vec(),
        bands: config.bands.clone(),
        metadata: HolographicMetadata {
            energy_spectrum,
            dominant_frequencies,
            band_entropy,
            reconstruction_error,
        },
    }
}

/// Query with specified resolution
fn holographic_search(
    query: &[f32],
    index: &HolographicIndex,
    k: usize,
    resolution: Resolution
) -> Vec<SearchResult> {
    // Step 1: Transform query to frequency domain
    let query_freq = encode_holographic(query, &index.config);

    // Step 2: Extract relevant frequency bands
    let (query_filtered, band_indices) = match resolution {
        Resolution::Coarse => {
            // Only low frequencies
            filter_bands(&query_freq, &[index.config.bands.low])
        },

        Resolution::Balanced => {
            // Low + mid frequencies
            filter_bands(&query_freq, &[
                index.config.bands.low,
                index.config.bands.mid,
            ])
        },

        Resolution::Fine => {
            // All frequencies
            (query_freq.frequency_domain.clone(), vec![])
        },

        Resolution::Custom { bands } => {
            filter_bands(&query_freq, &bands)
        },
    };

    // Step 3: Search in appropriate frequency bands
    let mut results = Vec::new();

    for (i, embedding) in index.embeddings.iter().enumerate() {
        // Filter document embedding to same bands as query
        let doc_filtered = if band_indices.is_empty() {
            embedding.frequency_domain.clone()
        } else {
            filter_bands_explicit(&embedding.frequency_domain, &band_indices)
        };

        // Compute frequency-domain similarity
        let similarity = frequency_similarity(&query_filtered, &doc_filtered);

        results.push((i, similarity));
    }

    // Step 4: Sort and return top-k
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    results.into_iter()
        .take(k)
        .map(|(id, score)| SearchResult {
            node_id: id,
            score,
            resolution: resolution.clone(),
        })
        .collect()
}

/// Filter to specific frequency bands
fn filter_bands(
    holographic: &HolographicEmbedding,
    bands: &[(usize, usize)]
) -> (Vec<Complex<f32>>, Vec<(usize, usize)>) {
    let mut filtered = vec![Complex::zero(); holographic.frequency_domain.len()];

    for &(start, end) in bands {
        for i in start..end {
            filtered[i] = holographic.frequency_domain[i];
        }
    }

    (filtered, bands.to_vec())
}

/// Frequency-domain similarity (handles phase and magnitude)
fn frequency_similarity(a: &[Complex<f32>], b: &[Complex<f32>]) -> f32 {
    assert_eq!(a.len(), b.len());

    let mut magnitude_similarity = 0.0;
    let mut phase_similarity = 0.0;

    let mut a_mag_sum = 0.0;
    let mut b_mag_sum = 0.0;

    for i in 0..a.len() {
        // Magnitude similarity (cosine of magnitudes)
        let a_mag = a[i].norm();
        let b_mag = b[i].norm();

        magnitude_similarity += a_mag * b_mag;
        a_mag_sum += a_mag * a_mag;
        b_mag_sum += b_mag * b_mag;

        // Phase similarity (cosine of phase difference)
        if a_mag > 1e-6 && b_mag > 1e-6 {
            let phase_diff = (a[i] / b[i]).arg();
            phase_similarity += phase_diff.cos();
        }
    }

    // Normalize magnitude similarity (cosine)
    magnitude_similarity /= (a_mag_sum * b_mag_sum).sqrt();

    // Normalize phase similarity
    let nonzero_count = a.iter()
        .zip(b.iter())
        .filter(|(a, b)| a.norm() > 1e-6 && b.norm() > 1e-6)
        .count();

    if nonzero_count > 0 {
        phase_similarity /= nonzero_count as f32;
    }

    // Combined similarity (weighted average)
    0.7 * magnitude_similarity + 0.3 * phase_similarity
}

/// Train learned frequency decomposition
fn train_learned_decomposition(
    training_data: &[(Vec<f32>, MultiScaleLabels)],
    config: &TrainingConfig
) -> LearnedEncoder {
    // Initialize encoder network
    let mut encoder = LearnedEncoder::random_init(config);

    for epoch in 0..config.epochs {
        let mut epoch_loss = 0.0;

        for batch in training_data.chunks(config.batch_size) {
            // Forward pass
            let mut batch_loss = 0.0;

            for (embedding, labels) in batch {
                // Encode to frequency domain
                let freq = encoder.forward(embedding);

                // Compute multi-scale loss
                let loss = match &config.loss {
                    LossFunction::Reconstruction => {
                        let reconstructed = encoder.backward(&freq);
                        mse(embedding, &reconstructed)
                    },

                    LossFunction::MultiScaleContrastive { temperature, weights } => {
                        compute_contrastive_loss(
                            &freq,
                            labels,
                            *temperature,
                            weights
                        )
                    },

                    LossFunction::InformationPreservation => {
                        compute_information_loss(&freq, embedding)
                    },

                    LossFunction::Combined(losses) => {
                        losses.iter()
                            .map(|(loss_fn, weight)| {
                                weight * compute_loss(loss_fn, &freq, embedding, labels)
                            })
                            .sum()
                    },
                };

                batch_loss += loss;
            }

            // Backward pass and update
            batch_loss /= batch.len() as f32;
            encoder.update_weights(batch_loss, config.learning_rate);

            epoch_loss += batch_loss;
        }

        println!("Epoch {}: loss = {}", epoch, epoch_loss);
    }

    encoder
}

/// Compute multi-scale contrastive loss
fn compute_contrastive_loss(
    freq: &[Complex<f32>],
    labels: &MultiScaleLabels,
    temperature: f32,
    weights: &[f32; 3]
) -> f32 {
    let mut total_loss = 0.0;

    // Low frequency (coarse labels)
    let low_freq = &freq[0..freq.len()/8];
    total_loss += weights[0] * contrastive_loss_at_scale(
        low_freq,
        &labels.coarse,
        temperature
    );

    // Mid frequency (structural labels)
    let mid_freq = &freq[freq.len()/8..freq.len()/2];
    total_loss += weights[1] * contrastive_loss_at_scale(
        mid_freq,
        &labels.structural,
        temperature
    );

    // High frequency (fine labels)
    let high_freq = &freq[freq.len()/2..];
    total_loss += weights[2] * contrastive_loss_at_scale(
        high_freq,
        &labels.fine,
        temperature
    );

    total_loss
}

/// Multi-scale labels for training
#[derive(Clone, Debug)]
pub struct MultiScaleLabels {
    /// Coarse label (e.g., topic category)
    pub coarse: String,

    /// Structural label (e.g., document type)
    pub structural: String,

    /// Fine label (e.g., specific entities)
    pub fine: Vec<String>,
}
```

### API Design

```rust
/// Public API for Semantic Holography
pub trait SemanticHolography {
    /// Create holographic index from embeddings
    fn new(
        embeddings: Vec<Vec<f32>>,
        config: HolographicConfig,
    ) -> Result<Self, HolographicError> where Self: Sized;

    /// Encode single embedding holographically
    fn encode(
        &self,
        embedding: &[f32],
    ) -> Result<HolographicEmbedding, HolographicError>;

    /// Search at specified resolution
    fn search(
        &self,
        query: &[f32],
        k: usize,
        resolution: Resolution,
    ) -> Result<Vec<SearchResult>, HolographicError>;

    /// Multi-resolution search (return results at all scales)
    fn search_multi_scale(
        &self,
        query: &[f32],
        k_per_scale: usize,
    ) -> Result<MultiScaleResults, HolographicError>;

    /// Reconstruct embedding from frequency domain
    fn reconstruct(
        &self,
        holographic: &HolographicEmbedding,
        resolution: Resolution,
    ) -> Result<Vec<f32>, HolographicError>;

    /// Add new embeddings (incremental)
    fn add_embeddings(
        &mut self,
        embeddings: &[Vec<f32>],
    ) -> Result<(), HolographicError>;

    /// Get frequency spectrum for embedding
    fn get_spectrum(
        &self,
        node_id: NodeId,
    ) -> Result<&[f32], HolographicError>;

    /// Analyze frequency content
    fn analyze_frequencies(
        &self,
    ) -> FrequencyAnalysis;

    /// Export visualization data
    fn export_spectrum(
        &self,
        node_ids: &[NodeId],
    ) -> SpectrumVisualization;

    /// Train learned frequency decomposition
    fn train_decomposition(
        training_data: &[(Vec<f32>, MultiScaleLabels)],
        config: TrainingConfig,
    ) -> Result<LearnedEncoder, HolographicError>;
}

/// Multi-scale search results
#[derive(Clone, Debug)]
pub struct MultiScaleResults {
    pub coarse: Vec<SearchResult>,
    pub balanced: Vec<SearchResult>,
    pub fine: Vec<SearchResult>,
}

/// Frequency analysis
#[derive(Clone, Debug)]
pub struct FrequencyAnalysis {
    /// Average energy by frequency band
    pub avg_energy_by_band: [f32; 3],

    /// Entropy by frequency band
    pub entropy_by_band: [f32; 3],

    /// Most informative frequencies
    pub top_frequencies: Vec<usize>,

    /// Reconstruction error statistics
    pub reconstruction_stats: ReconstructionStats,
}

#[derive(Clone, Debug)]
pub struct ReconstructionStats {
    pub mean_error: f32,
    pub std_error: f32,
    pub max_error: f32,
    pub error_by_band: [f32; 3],
}

/// Spectrum visualization export
#[derive(Clone, Debug, Serialize)]
pub struct SpectrumVisualization {
    pub embeddings: Vec<SpectrumData>,
    pub frequency_labels: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpectrumData {
    pub node_id: NodeId,
    pub magnitudes: Vec<f32>,
    pub phases: Vec<f32>,
    pub dominant_bands: Vec<usize>,
}

/// Enhanced search result with resolution info
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub node_id: NodeId,
    pub score: f32,
    pub resolution: Resolution,
}
```

## Integration Points

### Affected Crates/Modules

1. **`crates/ruvector-core/src/embeddings/`**
   - Add holographic embedding support
   - Integrate with existing embedding pipelines

2. **`crates/ruvector-gnn/src/holography/`**
   - New module for holographic operations
   - Frequency-domain processing

3. **`crates/ruvector-core/src/index/`**
   - Add frequency-indexed search
   - Multi-resolution query support

### New Modules to Create

1. **`crates/ruvector-gnn/src/holography/`**
   - `encoding.rs` - Holographic encoding/decoding
   - `frequency.rs` - Frequency domain operations (FFT, DCT, etc.)
   - `search.rs` - Multi-resolution search
   - `training.rs` - Learned decomposition training
   - `visualization.rs` - Spectrum visualization

2. **`crates/ruvector-core/src/transform/`**
   - `fft.rs` - Fast Fourier Transform
   - `dct.rs` - Discrete Cosine Transform
   - `wavelet.rs` - Wavelet transforms
   - `learned.rs` - Learned transform networks

### Dependencies on Other Features

- **Feature 10 (Gravitational Fields)**: Multi-resolution mass (coarse vs. fine importance)
- **Feature 11 (Causal Networks)**: Temporal frequencies (event rates)
- **Feature 13 (Crystallization)**: Crystal hierarchy matches frequency bands

## Regression Prevention

### Existing Functionality at Risk

1. **Standard Search Performance**
   - Risk: Frequency transforms add overhead
   - Prevention: Cache transformed embeddings, optional feature

2. **Embedding Quality**
   - Risk: Frequency decomposition loses information
   - Prevention: Monitor reconstruction error, adaptive bands

3. **Memory Usage**
   - Risk: Complex-valued frequency domain (2x storage)
   - Prevention: Magnitude-only storage option, lazy computation

### Test Cases to Prevent Regressions

```rust
#[cfg(test)]
mod regression_tests {
    /// Reconstruction accuracy
    #[test]
    fn test_perfect_reconstruction() {
        let embedding = random_vector(256);
        let holographic = encode_holographic(&embedding, &config);

        let reconstructed = inverse_transform(
            &holographic.frequency_domain,
            &config.transform
        );

        let error = mse(&embedding, &reconstructed);
        assert!(error < 1e-4, "Reconstruction error too high: {}", error);
    }

    /// Multi-scale consistency
    #[test]
    fn test_resolution_hierarchy() {
        let index = create_test_holographic_index();
        let query = random_vector(256);

        let coarse = index.search(&query, 10, Resolution::Coarse);
        let balanced = index.search(&query, 10, Resolution::Balanced);
        let fine = index.search(&query, 10, Resolution::Fine);

        // Coarse results should be subset of balanced
        // (lower resolution is more general)
        for result in &coarse {
            assert!(balanced.iter().any(|r| {
                similar_topics(r.node_id, result.node_id)
            }));
        }
    }

    /// Storage efficiency
    #[test]
    fn test_single_embedding_storage() {
        let n_docs = 10000;
        let embeddings = generate_test_embeddings(n_docs);

        // Standard approach: 3 separate embeddings per document
        let standard_storage = n_docs * 3 * 256 * size_of::<f32>();

        // Holographic: 1 complex embedding per document
        let holographic_storage = n_docs * 256 * size_of::<Complex<f32>>();

        assert!(holographic_storage < standard_storage);
        let reduction = 1.0 - (holographic_storage as f32 / standard_storage as f32);
        assert!(reduction > 0.33, "Storage reduction: {:.1}%", reduction * 100.0);
    }

    /// Frequency band information content
    #[test]
    fn test_band_information_distribution() {
        let index = create_test_holographic_index();
        let analysis = index.analyze_frequencies();

        // Low frequencies should contain most energy (coarse info)
        assert!(analysis.avg_energy_by_band[0] > analysis.avg_energy_by_band[1]);
        assert!(analysis.avg_energy_by_band[0] > analysis.avg_energy_by_band[2]);

        // All bands should have nonzero entropy
        for &entropy in &analysis.entropy_by_band {
            assert!(entropy > 0.0, "Band has zero entropy");
        }
    }
}
```

### Backward Compatibility Strategy

1. **Optional Feature**: Holography behind `semantic-holography` feature flag
2. **Fallback Mode**: If transform fails, use spatial domain directly
3. **Gradual Migration**: Support both holographic and standard embeddings
4. **Conversion Tools**: Convert existing embeddings to holographic format

## Implementation Phases

### Phase 1: Research Validation (3 weeks)
**Goal**: Validate holographic encoding on real embeddings

- Implement FFT/DCT transforms
- Test on benchmark datasets (MSMARCO, NQ)
- Measure reconstruction quality vs. frequency bands
- Compare multi-resolution search to standard search
- **Deliverable**: Research report with accuracy/efficiency analysis

### Phase 2: Core Implementation (4 weeks)
**Goal**: Production-ready holographic encoding

- Implement all transform types (FFT, DCT, Wavelet)
- Build frequency-domain similarity functions
- Develop multi-resolution search API
- Add caching and optimization
- Implement learned decomposition training
- **Deliverable**: Working holography module with unit tests

### Phase 3: Integration (2 weeks)
**Goal**: Integrate with RuVector ecosystem

- Add holographic embedding support to core
- Integrate with HNSW index
- Create API bindings (Python, Node.js)
- Implement visualization tools
- Write integration tests
- **Deliverable**: Integrated holographic search feature

### Phase 4: Optimization (2 weeks)
**Goal**: Production performance and tuning

- Profile and optimize transforms
- Implement parallel frequency computation
- Add GPU acceleration (optional)
- Create benchmarks and examples
- Write comprehensive documentation
- **Deliverable**: Production-ready, documented feature

## Success Metrics

### Performance Benchmarks

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Storage reduction | 0% | >50% | vs. 3 separate embeddings |
| Reconstruction error | N/A | <0.01 | MSE, average |
| Coarse search latency | 1.0x | <1.2x | vs. standard search |
| Fine search latency | 1.0x | <1.5x | vs. standard search |
| Transform time | N/A | <1ms | Per embedding, 256-dim |

### Accuracy Metrics

1. **Multi-Scale Consistency**: Coarse results generalize fine results
   - Target: 80% topic overlap between coarse and fine top-10

2. **Resolution Separation**: Different resolutions find different aspects
   - Target: <60% overlap between coarse-only and fine-only results

3. **Information Preservation**: Frequency bands capture distinct semantics
   - Target: Mutual information between bands <0.3

### Comparison to Baselines

Test against:
1. **Standard embeddings**: Single-resolution search
2. **Multiple embeddings**: Separate embeddings per granularity
3. **Hierarchical clustering**: Post-hoc hierarchy construction

Datasets:
- MSMARCO (passage retrieval, multi-scale relevance)
- Natural Questions (topic vs. entity queries)
- Wikipedia (hierarchical categories)
- arXiv (coarse=topic, fine=specific methods)

## Risks and Mitigations

### Technical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Information loss in compression | High | Medium | Monitor reconstruction error, adaptive bands |
| Poor frequency separation | High | Medium | Learn optimal frequency allocation |
| Transform overhead | Medium | High | Cache, optimize FFT, GPU acceleration |
| Complex number storage | Medium | High | Magnitude-only option, compression |
| Unclear frequency semantics | Medium | Medium | Visualization tools, learned decomposition |

### Detailed Mitigations

1. **Information Loss**
   - Monitor reconstruction error per embedding
   - Adaptive band allocation based on content
   - Fallback to spatial domain if error too high
   - **Fallback**: Disable holography for critical applications

2. **Poor Frequency Separation**
   - Train learned decomposition on labeled data
   - Use contrastive loss to separate scales
   - Validate on multi-scale benchmarks
   - **Fallback**: Use standard frequency bands (12.5%, 50%, 100%)

3. **Transform Overhead**
   - Use FFT libraries (FFTW, cuFFT)
   - Cache frequency-domain representations
   - Parallelize transforms across embeddings
   - **Fallback**: Pre-compute transforms offline

4. **Storage Overhead**
   - Store magnitude-only (discard phase)
   - Quantize frequency coefficients
   - Use sparse representation (zero out small coefficients)
   - **Fallback**: Store only most important frequencies

5. **Unclear Semantics**
   - Build visualization tools (spectrum plots)
   - Provide example queries at each resolution
   - Train learned decomposition with interpretable labels
   - **Fallback**: Use simple resolution names (coarse/fine)

## Applications

### Multi-Granularity Search
- **Coarse queries**: "machine learning papers" → topic-level results
- **Fine queries**: "BERT attention mechanism" → specific technique results
- **Adaptive**: Start coarse, refine to fine based on user feedback

### Hierarchical Navigation
- Browse corpus at multiple scales
- Zoom in/out on semantic clusters
- Drill-down from topics to subtopics to documents

### Efficient Storage
- Store one embedding instead of multiple
- On-demand reconstruction at query time
- Reduce index size by 50%+

### Query Reformulation
- Coarse search for topic exploration
- Fine search for precision
- Balanced search for production

## References

### Signal Processing
- Fourier analysis and frequency decomposition
- Wavelet transforms for multi-resolution analysis
- Holographic principles in optics

### Machine Learning
- Multi-scale representation learning
- Learned compression and decomposition
- Contrastive learning at multiple scales

### Information Retrieval
- Query expansion and reformulation
- Hierarchical search and navigation
- Multi-granularity relevance

### Implementation
- FFTW (Fastest Fourier Transform in the West)
- PyTorch/TensorFlow for learned transforms
- Sparse frequency representations
