# ADR-007: ML Pipeline and Inference Architecture

## Status

**Accepted**

## Date

2025-01-15

## Context

7sense requires robust machine learning inference capabilities to transform raw bioacoustic recordings into meaningful embeddings for species identification, similarity search, and ecological analysis. The system must process continuous audio streams from field sensors while maintaining low latency and high reliability.

### Key Requirements

1. **Model**: Perch 2.0 (EfficientNet-B3 backbone) for bioacoustic embeddings
2. **Input**: 5-second mono audio segments at 32kHz (160,000 samples)
3. **Output**: 1536-dimensional embeddings suitable for HNSW indexing
4. **Runtime**: ONNX Runtime in Rust for performance and safety
5. **Scale**: Support for continuous processing of multi-sensor networks

### Technical Constraints

- Field devices may have limited compute (CPU-only inference)
- Network connectivity may be intermittent
- Embeddings must be stable for HNSW neighbor consistency
- Must integrate with RuVector for vector storage and graph queries

## Decision

We will implement a multi-stage ML inference pipeline in Rust using ONNX Runtime, with the following architecture:

### 1. Audio Preprocessing Pipeline

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Audio Preprocessing Pipeline                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Raw Audio     Resample      Window        Normalize      Model     │
│  ─────────► ──────────► ──────────► ─────────────► ──────────►     │
│  (any SR)     (32kHz)     (5s seg)    (peak norm)     (ONNX)        │
│                              │                                       │
│                              ▼                                       │
│                    ┌─────────────────┐                              │
│                    │ Overlap Buffer  │                              │
│                    │  (configurable) │                              │
│                    └─────────────────┘                              │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

#### 1.1 Resampling Strategy

```rust
/// Audio resampling configuration for Perch 2.0 compatibility
pub struct ResampleConfig {
    /// Target sample rate (Perch 2.0 expects 32kHz)
    pub target_sr: u32,  // 32000
    /// Resampling quality (higher = better but slower)
    pub quality: ResampleQuality,
    /// Anti-aliasing filter cutoff
    pub lowpass_cutoff: f32,  // 0.95 * Nyquist
}

pub enum ResampleQuality {
    /// Linear interpolation (fastest, lowest quality)
    Linear,
    /// Windowed sinc with 16-tap filter
    Medium,
    /// Windowed sinc with 64-tap Kaiser filter (recommended)
    High,
    /// Polyphase with 256-tap filter (highest quality)
    Audiophile,
}
```

**Recommended Implementation**: Use `rubato` crate for high-quality asynchronous resampling:

```rust
use rubato::{FftFixedInOut, Resampler};

pub fn resample_to_32khz(audio: &[f32], source_sr: u32) -> Vec<f32> {
    if source_sr == 32000 {
        return audio.to_vec();
    }

    let resampler = FftFixedInOut::<f32>::new(
        source_sr as usize,
        32000,
        audio.len(),
        1,  // mono
    ).expect("Failed to create resampler");

    let waves_in = vec![audio.to_vec()];
    let mut waves_out = resampler.process(&waves_in, None)
        .expect("Resampling failed");

    waves_out.remove(0)
}
```

#### 1.2 Windowing and Segmentation

Perch 2.0 requires exactly 160,000 samples (5 seconds at 32kHz). For continuous recordings, we implement overlapping windows:

```rust
/// Windowing configuration for continuous audio processing
pub struct WindowConfig {
    /// Window duration in samples (160,000 for Perch 2.0)
    pub window_size: usize,
    /// Hop size between windows (overlap = window_size - hop_size)
    pub hop_size: usize,
    /// Minimum audio energy to process (skip silence)
    pub energy_threshold: f32,
    /// Padding strategy for incomplete windows
    pub padding: PaddingStrategy,
}

pub enum PaddingStrategy {
    /// Zero-pad incomplete windows
    ZeroPad,
    /// Reflect audio at boundaries
    Reflect,
    /// Discard incomplete windows
    Discard,
    /// Overlap with previous window to fill
    OverlapFill,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            window_size: 160_000,  // 5s at 32kHz
            hop_size: 80_000,      // 2.5s hop = 50% overlap
            energy_threshold: 1e-6,
            padding: PaddingStrategy::ZeroPad,
        }
    }
}
```

**Overlap Strategy Rationale**:

| Overlap | Hop Size | Use Case | Latency | Throughput |
|---------|----------|----------|---------|------------|
| 0% | 5.0s | Batch processing | 5.0s | 1x |
| 25% | 3.75s | Low-resource devices | 3.75s | 1.33x |
| 50% | 2.5s | **Recommended** | 2.5s | 2x |
| 75% | 1.25s | High-resolution temporal | 1.25s | 4x |

**Recommendation**: 50% overlap (2.5s hop) provides good temporal resolution while maintaining reasonable compute load. Calls at window boundaries are captured by overlapping segments.

#### 1.3 Normalization

```rust
/// Audio normalization before inference
pub fn normalize_audio(audio: &mut [f32], config: &NormConfig) {
    match config.method {
        NormMethod::PeakNormalize => {
            let peak = audio.iter()
                .map(|x| x.abs())
                .fold(0.0f32, f32::max);
            if peak > 1e-8 {
                let scale = config.target_peak / peak;
                audio.iter_mut().for_each(|x| *x *= scale);
            }
        }
        NormMethod::RmsNormalize => {
            let rms = (audio.iter().map(|x| x * x).sum::<f32>()
                / audio.len() as f32).sqrt();
            if rms > 1e-8 {
                let scale = config.target_rms / rms;
                audio.iter_mut().for_each(|x| *x *= scale);
            }
        }
        NormMethod::None => {}
    }

    // Clip to [-1.0, 1.0] to prevent model instability
    audio.iter_mut().for_each(|x| *x = x.clamp(-1.0, 1.0));
}
```

### 2. ONNX Integration in Rust

#### 2.1 Model Loading and Caching

```rust
use ort::{Environment, Session, SessionBuilder, Value};
use std::sync::Arc;
use parking_lot::RwLock;

/// Thread-safe model session manager with caching
pub struct ModelManager {
    /// Shared ONNX runtime environment
    env: Arc<Environment>,
    /// Cached model sessions by version
    sessions: RwLock<HashMap<ModelVersion, Arc<Session>>>,
    /// Configuration for inference
    config: InferenceConfig,
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ModelVersion {
    pub name: String,      // "perch-v2"
    pub version: String,   // "2.0.0"
    pub variant: String,   // "base" | "quantized" | "pruned"
}

pub struct InferenceConfig {
    /// Number of threads for intra-op parallelism
    pub intra_op_threads: usize,
    /// Number of threads for inter-op parallelism
    pub inter_op_threads: usize,
    /// Memory optimization level
    pub optimization_level: OptimizationLevel,
    /// Execution provider priority
    pub providers: Vec<ExecutionProvider>,
    /// Maximum batch size
    pub max_batch_size: usize,
}

impl ModelManager {
    pub fn new(config: InferenceConfig) -> Result<Self, ModelError> {
        let env = Environment::builder()
            .with_name("sevensense-ml")
            .with_log_level(ort::LoggingLevel::Warning)
            .build()?
            .into_arc();

        Ok(Self {
            env,
            sessions: RwLock::new(HashMap::new()),
            config,
        })
    }

    /// Load or retrieve cached model session
    pub fn get_session(&self, version: &ModelVersion) -> Result<Arc<Session>, ModelError> {
        // Check cache first
        if let Some(session) = self.sessions.read().get(version) {
            return Ok(Arc::clone(session));
        }

        // Load model
        let model_path = self.resolve_model_path(version)?;
        let session = self.create_session(&model_path)?;
        let session = Arc::new(session);

        // Cache for future use
        self.sessions.write().insert(version.clone(), Arc::clone(&session));

        Ok(session)
    }

    fn create_session(&self, path: &Path) -> Result<Session, ModelError> {
        let mut builder = SessionBuilder::new(&self.env)?;

        // Configure thread pool
        builder = builder
            .with_intra_threads(self.config.intra_op_threads)?
            .with_inter_threads(self.config.inter_op_threads)?
            .with_optimization_level(self.config.optimization_level.into())?;

        // Add execution providers in priority order
        for provider in &self.config.providers {
            match provider {
                ExecutionProvider::CUDA { device_id } => {
                    builder = builder.with_cuda(*device_id)?;
                }
                ExecutionProvider::CoreML => {
                    builder = builder.with_coreml(0)?;
                }
                ExecutionProvider::CPU => {
                    // CPU is always available as fallback
                }
            }
        }

        builder.with_model_from_file(path)
    }
}
```

#### 2.2 Batch Inference Optimization

```rust
/// Efficient batch inference for multiple audio segments
pub struct BatchInference {
    model: Arc<ModelManager>,
    /// Pre-allocated input buffer
    input_buffer: Vec<f32>,
    /// Maximum segments per batch
    max_batch: usize,
}

impl BatchInference {
    /// Process multiple audio segments efficiently
    pub async fn infer_batch(
        &self,
        segments: &[AudioSegment],
        version: &ModelVersion,
    ) -> Result<Vec<InferenceOutput>, InferenceError> {
        let session = self.model.get_session(version)?;

        // Dynamic batching: group segments up to max_batch
        let mut results = Vec::with_capacity(segments.len());

        for chunk in segments.chunks(self.max_batch) {
            let batch_results = self.run_batch(&session, chunk)?;
            results.extend(batch_results);
        }

        Ok(results)
    }

    fn run_batch(
        &self,
        session: &Session,
        segments: &[AudioSegment],
    ) -> Result<Vec<InferenceOutput>, InferenceError> {
        let batch_size = segments.len();

        // Prepare input tensor: [batch, 160000]
        let mut input_data = vec![0.0f32; batch_size * 160_000];
        for (i, segment) in segments.iter().enumerate() {
            let start = i * 160_000;
            input_data[start..start + segment.samples.len()]
                .copy_from_slice(&segment.samples);
        }

        let input_shape = [batch_size as i64, 160_000i64];
        let input_tensor = Value::from_array(
            session.allocator(),
            &input_shape,
            &input_data,
        )?;

        // Run inference
        let outputs = session.run(vec![input_tensor])?;

        // Parse outputs: embedding [batch, 1536], spectrogram, logits
        let embeddings = outputs[0].try_extract::<f32>()?;
        let spectrograms = outputs.get(1).map(|v| v.try_extract::<f32>());
        let logits = outputs.get(2).map(|v| v.try_extract::<f32>());

        // Split batch results
        let mut results = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let emb_start = i * 1536;
            let embedding: [f32; 1536] = embeddings.view()
                .as_slice()?[emb_start..emb_start + 1536]
                .try_into()?;

            results.push(InferenceOutput {
                embedding,
                spectrogram: spectrograms.as_ref().map(|s| {
                    extract_spectrogram(s, i)
                }),
                logits: logits.as_ref().map(|l| {
                    extract_logits(l, i)
                }),
                metadata: InferenceMetadata {
                    model_version: version.clone(),
                    inference_time_ms: 0.0,  // Filled by caller
                    batch_index: i,
                },
            });
        }

        Ok(results)
    }
}
```

#### 2.3 GPU vs CPU Tradeoffs

| Factor | CPU | GPU (CUDA) | GPU (CoreML) |
|--------|-----|------------|--------------|
| **Latency (single)** | ~150ms | ~15ms | ~20ms |
| **Throughput (batch=8)** | ~800ms | ~40ms | ~50ms |
| **Memory** | ~500MB | ~2GB VRAM | ~1GB unified |
| **Power** | ~15W | ~150W | ~30W |
| **Availability** | Always | NVIDIA only | Apple only |
| **Field deployment** | Yes | Rarely | Yes (M-series) |

**Recommended Configuration**:

```rust
impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            intra_op_threads: num_cpus::get().min(4),
            inter_op_threads: 1,
            optimization_level: OptimizationLevel::All,
            providers: vec![
                // Try GPU first, fall back to CPU
                ExecutionProvider::CUDA { device_id: 0 },
                ExecutionProvider::CoreML,
                ExecutionProvider::CPU,
            ],
            max_batch_size: 8,
        }
    }
}

/// Field device configuration (CPU-optimized)
pub fn field_config() -> InferenceConfig {
    InferenceConfig {
        intra_op_threads: 2,
        inter_op_threads: 1,
        optimization_level: OptimizationLevel::All,
        providers: vec![ExecutionProvider::CPU],
        max_batch_size: 1,  // Process sequentially to reduce memory
    }
}

/// Server configuration (GPU-optimized)
pub fn server_config() -> InferenceConfig {
    InferenceConfig {
        intra_op_threads: 4,
        inter_op_threads: 2,
        optimization_level: OptimizationLevel::All,
        providers: vec![
            ExecutionProvider::CUDA { device_id: 0 },
            ExecutionProvider::CPU,
        ],
        max_batch_size: 32,  // Maximize GPU utilization
    }
}
```

### 3. Embedding Post-Processing

#### 3.1 L2 Normalization

All embeddings are L2-normalized before storage to enable cosine similarity via dot product:

```rust
/// L2 normalize embedding in-place
pub fn l2_normalize(embedding: &mut [f32; 1536]) {
    let norm = embedding.iter()
        .map(|x| x * x)
        .sum::<f32>()
        .sqrt();

    if norm > 1e-12 {
        embedding.iter_mut().for_each(|x| *x /= norm);
    } else {
        // Handle near-zero embeddings (likely silent input)
        embedding.fill(0.0);
        embedding[0] = 1.0;  // Unit vector in first dimension
    }
}

/// Verify embedding quality
pub fn validate_embedding(embedding: &[f32; 1536]) -> EmbeddingQuality {
    let norm = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    let has_nan = embedding.iter().any(|x| x.is_nan());
    let has_inf = embedding.iter().any(|x| x.is_infinite());
    let sparsity = embedding.iter().filter(|&&x| x.abs() < 1e-6).count() as f32
        / 1536.0;

    EmbeddingQuality {
        norm,
        has_nan,
        has_inf,
        sparsity,
        is_valid: !has_nan && !has_inf && (0.99..1.01).contains(&norm),
    }
}
```

#### 3.2 Dimensionality Reduction (Optional)

For storage-constrained scenarios, we support PCA reduction:

```rust
/// PCA-based dimensionality reduction
pub struct PCAReducer {
    /// Principal components matrix [target_dim, 1536]
    components: Array2<f32>,
    /// Mean vector for centering [1536]
    mean: Array1<f32>,
    /// Target dimensionality
    target_dim: usize,
}

impl PCAReducer {
    /// Reduce 1536-D embedding to target dimension
    pub fn reduce(&self, embedding: &[f32; 1536]) -> Vec<f32> {
        let centered: Array1<f32> = Array1::from_vec(embedding.to_vec()) - &self.mean;
        let reduced = self.components.dot(&centered);

        // L2 normalize the reduced embedding
        let norm = reduced.iter().map(|x| x * x).sum::<f32>().sqrt();
        reduced.iter().map(|x| x / norm).collect()
    }
}
```

| Target Dim | Memory Reduction | Retrieval Quality (mAP) |
|------------|------------------|------------------------|
| 1536 (full) | 1.0x | 100% baseline |
| 768 | 2.0x | ~98% |
| 384 | 4.0x | ~95% |
| 256 | 6.0x | ~92% |
| 128 | 12.0x | ~85% |

**Recommendation**: Use full 1536-D for server deployments; consider 384-D for edge devices.

#### 3.3 Hyperbolic Projection (Euclidean to Poincare)

For hierarchical species relationships, project to Poincare ball:

```rust
/// Project Euclidean embedding to Poincare ball
pub struct HyperbolicProjector {
    /// Curvature of the Poincare ball (typically -1.0)
    curvature: f32,
    /// Maximum norm in Poincare ball (< 1.0 for stability)
    max_norm: f32,
}

impl HyperbolicProjector {
    pub fn new(curvature: f32) -> Self {
        Self {
            curvature,
            max_norm: 0.999,  // Avoid boundary instability
        }
    }

    /// Exponential map from tangent space at origin to Poincare ball
    pub fn exp_map_zero(&self, v: &[f32]) -> Vec<f32> {
        let c = -self.curvature;
        let v_norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();

        if v_norm < 1e-10 {
            return vec![0.0; v.len()];
        }

        let sqrt_c = c.sqrt();
        let coeff = (sqrt_c * v_norm).tanh() / (sqrt_c * v_norm);

        let mut result: Vec<f32> = v.iter().map(|x| x * coeff).collect();

        // Clamp to max_norm for numerical stability
        let result_norm = result.iter().map(|x| x * x).sum::<f32>().sqrt();
        if result_norm > self.max_norm {
            let scale = self.max_norm / result_norm;
            result.iter_mut().for_each(|x| *x *= scale);
        }

        result
    }

    /// Poincare distance between two points
    pub fn distance(&self, x: &[f32], y: &[f32]) -> f32 {
        let c = -self.curvature;

        let x_norm_sq: f32 = x.iter().map(|v| v * v).sum();
        let y_norm_sq: f32 = y.iter().map(|v| v * v).sum();
        let xy_diff_sq: f32 = x.iter().zip(y.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum();

        let numerator = 2.0 * xy_diff_sq;
        let denominator = (1.0 - x_norm_sq) * (1.0 - y_norm_sq);

        (1.0 / c.sqrt()) * (1.0 + numerator / denominator).acosh()
    }
}
```

**Use Cases for Hyperbolic Embeddings**:
- Taxonomic hierarchy preservation (genus -> species -> subspecies)
- Call type hierarchies (alarm -> aerial predator alarm)
- Geographic clustering with nested regions

### 4. Model Versioning and Updates

#### 4.1 Version Management

```rust
/// Model registry with version control
pub struct ModelRegistry {
    /// Base directory for model storage
    models_dir: PathBuf,
    /// Available model versions
    versions: HashMap<String, Vec<ModelMetadata>>,
    /// Currently active version per model
    active: HashMap<String, ModelVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub version: ModelVersion,
    pub checksum: String,         // SHA-256
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
    pub performance: PerformanceMetrics,
    pub compatibility: CompatibilityInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityInfo {
    /// Minimum ONNX Runtime version
    pub min_ort_version: String,
    /// Required execution providers
    pub required_providers: Vec<String>,
    /// Expected input shape
    pub input_shape: Vec<i64>,
    /// Expected output shapes
    pub output_shapes: HashMap<String, Vec<i64>>,
}

impl ModelRegistry {
    /// Hot-swap model version without restart
    pub async fn switch_version(
        &mut self,
        model_name: &str,
        new_version: &str,
    ) -> Result<(), ModelError> {
        // Validate new version exists
        let metadata = self.get_metadata(model_name, new_version)?;

        // Verify checksum
        let path = self.model_path(model_name, new_version);
        let actual_checksum = compute_sha256(&path)?;
        if actual_checksum != metadata.checksum {
            return Err(ModelError::ChecksumMismatch);
        }

        // Pre-load new model to catch errors early
        let new_session = self.load_session(&path).await?;

        // Atomic swap
        self.active.insert(
            model_name.to_string(),
            ModelVersion {
                name: model_name.to_string(),
                version: new_version.to_string(),
                variant: metadata.version.variant.clone(),
            },
        );

        Ok(())
    }
}
```

#### 4.2 A/B Testing Support

```rust
/// Traffic splitting for model comparison
pub struct ModelRouter {
    /// Model versions with traffic weights
    routes: Vec<(ModelVersion, f32)>,
    /// RNG for consistent routing
    rng: StdRng,
}

impl ModelRouter {
    /// Route request to model version based on weights
    pub fn route(&mut self, request_id: &str) -> &ModelVersion {
        // Use request_id hash for consistent routing
        let hash = seahash::hash(request_id.as_bytes());
        let sample = (hash % 10000) as f32 / 10000.0;

        let mut cumulative = 0.0;
        for (version, weight) in &self.routes {
            cumulative += weight;
            if sample < cumulative {
                return version;
            }
        }

        // Fallback to last version
        &self.routes.last().unwrap().0
    }
}
```

### 5. Fallback Strategies

#### 5.1 Graceful Degradation

```rust
/// Fallback chain for inference failures
pub struct FallbackChain {
    primary: Arc<ModelManager>,
    fallbacks: Vec<FallbackStrategy>,
}

pub enum FallbackStrategy {
    /// Use quantized model (faster, less accurate)
    QuantizedModel(ModelVersion),
    /// Use cached embedding for similar audio
    CachedEmbedding { similarity_threshold: f32 },
    /// Return zero vector with error flag
    ZeroVector,
    /// Queue for later processing
    DeferredQueue(mpsc::Sender<DeferredRequest>),
}

impl FallbackChain {
    pub async fn infer_with_fallback(
        &self,
        segment: &AudioSegment,
    ) -> InferenceResult {
        // Try primary model
        match self.primary.infer(segment).await {
            Ok(output) => return InferenceResult::Success(output),
            Err(e) => {
                tracing::warn!("Primary inference failed: {}", e);
            }
        }

        // Try fallbacks in order
        for fallback in &self.fallbacks {
            match fallback {
                FallbackStrategy::QuantizedModel(version) => {
                    if let Ok(output) = self.primary.infer_version(segment, version).await {
                        return InferenceResult::Fallback {
                            output,
                            strategy: "quantized".to_string(),
                        };
                    }
                }
                FallbackStrategy::CachedEmbedding { similarity_threshold } => {
                    if let Some(cached) = self.find_similar_cached(segment, *similarity_threshold) {
                        return InferenceResult::Cached {
                            output: cached,
                            similarity: cached.similarity,
                        };
                    }
                }
                FallbackStrategy::ZeroVector => {
                    return InferenceResult::ZeroVector {
                        reason: "All inference strategies failed".to_string(),
                    };
                }
                FallbackStrategy::DeferredQueue(sender) => {
                    let _ = sender.send(DeferredRequest {
                        segment: segment.clone(),
                        timestamp: Utc::now(),
                    });
                    return InferenceResult::Deferred;
                }
            }
        }

        InferenceResult::Failed("All fallbacks exhausted".to_string())
    }
}
```

#### 5.2 Circuit Breaker Pattern

```rust
/// Circuit breaker to prevent cascade failures
pub struct InferenceCircuitBreaker {
    state: AtomicU8,  // 0=Closed, 1=Open, 2=HalfOpen
    failure_count: AtomicU32,
    last_failure: AtomicU64,
    config: CircuitBreakerConfig,
}

pub struct CircuitBreakerConfig {
    /// Failures before opening circuit
    pub failure_threshold: u32,
    /// Time before attempting recovery (ms)
    pub recovery_timeout: u64,
    /// Successes needed to close circuit
    pub success_threshold: u32,
}

impl InferenceCircuitBreaker {
    pub fn allow_request(&self) -> bool {
        match self.state.load(Ordering::SeqCst) {
            0 => true,  // Closed - allow all
            1 => {      // Open - check if recovery timeout elapsed
                let elapsed = Utc::now().timestamp_millis() as u64
                    - self.last_failure.load(Ordering::SeqCst);
                if elapsed > self.config.recovery_timeout {
                    self.state.store(2, Ordering::SeqCst);  // Half-open
                    true
                } else {
                    false
                }
            }
            2 => true,  // Half-open - allow probe request
            _ => false,
        }
    }

    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
        self.state.store(0, Ordering::SeqCst);  // Close circuit
    }

    pub fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
        self.last_failure.store(Utc::now().timestamp_millis() as u64, Ordering::SeqCst);

        if failures >= self.config.failure_threshold {
            self.state.store(1, Ordering::SeqCst);  // Open circuit
        }
    }
}
```

### 6. Quality Metrics

#### 6.1 Embedding Stability Monitoring

```rust
/// Track embedding quality over time
pub struct EmbeddingQualityMonitor {
    /// Rolling window of embedding norms
    norm_history: VecDeque<f32>,
    /// Rolling window of inter-embedding distances
    distance_history: VecDeque<f32>,
    /// Anomaly detection threshold (standard deviations)
    anomaly_threshold: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct QualityReport {
    /// Average embedding norm (should be ~1.0 after normalization)
    pub mean_norm: f32,
    pub std_norm: f32,
    /// Average pairwise distance in recent batch
    pub mean_distance: f32,
    pub std_distance: f32,
    /// Percentage of embeddings flagged as anomalous
    pub anomaly_rate: f32,
    /// Distribution statistics
    pub percentiles: NormPercentiles,
}

#[derive(Debug, Clone, Serialize)]
pub struct NormPercentiles {
    pub p5: f32,
    pub p25: f32,
    pub p50: f32,
    pub p75: f32,
    pub p95: f32,
}

impl EmbeddingQualityMonitor {
    /// Check if embedding is anomalous
    pub fn is_anomalous(&self, embedding: &[f32; 1536]) -> bool {
        let norm = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();

        if self.norm_history.len() < 100 {
            return false;  // Not enough history
        }

        let mean: f32 = self.norm_history.iter().sum::<f32>()
            / self.norm_history.len() as f32;
        let variance: f32 = self.norm_history.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f32>() / self.norm_history.len() as f32;
        let std = variance.sqrt();

        (norm - mean).abs() > self.anomaly_threshold * std
    }

    /// Generate quality report
    pub fn report(&self) -> QualityReport {
        let norms: Vec<f32> = self.norm_history.iter().copied().collect();
        let mean_norm = norms.iter().sum::<f32>() / norms.len() as f32;
        let std_norm = (norms.iter()
            .map(|x| (x - mean_norm).powi(2))
            .sum::<f32>() / norms.len() as f32)
            .sqrt();

        let mut sorted_norms = norms.clone();
        sorted_norms.sort_by(|a, b| a.partial_cmp(b).unwrap());

        QualityReport {
            mean_norm,
            std_norm,
            mean_distance: self.mean_distance(),
            std_distance: self.std_distance(),
            anomaly_rate: self.anomaly_rate(),
            percentiles: NormPercentiles {
                p5: percentile(&sorted_norms, 5),
                p25: percentile(&sorted_norms, 25),
                p50: percentile(&sorted_norms, 50),
                p75: percentile(&sorted_norms, 75),
                p95: percentile(&sorted_norms, 95),
            },
        }
    }
}
```

#### 6.2 Inference Performance Metrics

```rust
/// Prometheus-compatible metrics
pub struct InferenceMetrics {
    /// Histogram of inference latencies
    pub latency_histogram: Histogram,
    /// Counter of successful inferences
    pub success_count: Counter,
    /// Counter of failed inferences
    pub failure_count: Counter,
    /// Gauge of current batch size
    pub batch_size_gauge: Gauge,
    /// Histogram of embedding norms
    pub norm_histogram: Histogram,
}

impl InferenceMetrics {
    pub fn record_inference(&self, result: &InferenceResult, duration: Duration) {
        self.latency_histogram.observe(duration.as_secs_f64());

        match result {
            InferenceResult::Success(output) => {
                self.success_count.inc();
                let norm = output.embedding.iter()
                    .map(|x| x * x)
                    .sum::<f32>()
                    .sqrt();
                self.norm_histogram.observe(norm as f64);
            }
            _ => {
                self.failure_count.inc();
            }
        }
    }
}
```

### 7. Integration with birdnet-onnx Crate

For verification and cross-validation, integrate with the existing `birdnet-onnx` crate:

```rust
use birdnet_onnx::{BirdNet, BirdNetConfig};

/// Verification harness using birdnet-onnx
pub struct VerificationHarness {
    /// Our Perch 2.0 inference pipeline
    perch: Arc<BatchInference>,
    /// BirdNET-ONNX for cross-validation
    birdnet: Option<BirdNet>,
    /// Verification configuration
    config: VerificationConfig,
}

pub struct VerificationConfig {
    /// Enable BirdNET cross-validation
    pub enable_birdnet: bool,
    /// Minimum confidence for BirdNET predictions
    pub birdnet_threshold: f32,
    /// Log discrepancies above this threshold
    pub discrepancy_threshold: f32,
}

impl VerificationHarness {
    /// Run parallel inference and compare results
    pub async fn verify(
        &self,
        audio: &[f32],
    ) -> VerificationResult {
        // Run Perch 2.0
        let perch_result = self.perch.infer_single(audio).await;

        // Run BirdNET if enabled
        let birdnet_result = if self.config.enable_birdnet {
            self.birdnet.as_ref().map(|bn| {
                bn.predict(audio, self.config.birdnet_threshold)
            })
        } else {
            None
        };

        // Compare top predictions
        let agreement = self.compute_agreement(&perch_result, &birdnet_result);

        VerificationResult {
            perch: perch_result,
            birdnet: birdnet_result,
            agreement_score: agreement,
            discrepancies: self.find_discrepancies(&perch_result, &birdnet_result),
        }
    }

    fn compute_agreement(
        &self,
        perch: &InferenceOutput,
        birdnet: &Option<Vec<BirdNetPrediction>>,
    ) -> f32 {
        // Compare top-k species predictions
        // Returns 1.0 for perfect agreement, 0.0 for no overlap
        match birdnet {
            Some(predictions) => {
                let perch_species: HashSet<_> = perch.top_species(5)
                    .iter()
                    .map(|s| s.species_id.clone())
                    .collect();
                let birdnet_species: HashSet<_> = predictions
                    .iter()
                    .take(5)
                    .map(|p| p.species_id.clone())
                    .collect();

                let overlap = perch_species.intersection(&birdnet_species).count();
                overlap as f32 / 5.0
            }
            None => 1.0,  // No comparison available
        }
    }
}
```

### 8. Pipeline Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                        7sense ML Inference Pipeline                        │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐  │
│  │   Audio     │    │  Preprocess │    │    ONNX     │    │    Post     │  │
│  │   Input     │───►│   Pipeline  │───►│   Runtime   │───►│  Process    │  │
│  │             │    │             │    │             │    │             │  │
│  └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘  │
│        │                  │                  │                  │          │
│        ▼                  ▼                  ▼                  ▼          │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐  │
│  │ • WAV/FLAC  │    │ • Resample  │    │ • GPU/CPU   │    │ • L2 Norm   │  │
│  │ • Opus      │    │   32kHz     │    │   routing   │    │ • PCA       │  │
│  │ • Real-time │    │ • Window    │    │ • Batching  │    │ • Poincare  │  │
│  │   stream    │    │   5s/50%    │    │ • Caching   │    │   project   │  │
│  │             │    │ • Normalize │    │             │    │             │  │
│  └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘  │
│                                                                  │          │
│                                                                  ▼          │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                         Output: InferenceOutput                       │  │
│  │  • embedding: [f32; 1536]  - L2 normalized                           │  │
│  │  • spectrogram: [500, 128] - Log-mel (optional)                      │  │
│  │  • logits: [N_species]     - Classification scores (optional)        │  │
│  │  • metadata: InferenceMetadata                                        │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                         │                                   │
│                                         ▼                                   │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐  │
│  │   RuVector  │    │    HNSW     │    │   Quality   │    │  BirdNET    │  │
│  │   Storage   │◄───│   Index     │    │   Monitor   │    │  Verify     │  │
│  │             │    │             │    │             │    │  (optional) │  │
│  └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘  │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Consequences

### Positive

1. **Performance**: ONNX Runtime provides near-native inference speed in Rust
2. **Flexibility**: Support for CPU and GPU execution with automatic fallback
3. **Reliability**: Circuit breaker and fallback strategies prevent cascade failures
4. **Observability**: Comprehensive metrics for embedding quality and inference performance
5. **Versioning**: Hot-swap model updates without service restart
6. **Verification**: BirdNET integration provides independent validation

### Negative

1. **Complexity**: Multiple execution providers require careful configuration
2. **Memory**: Full 1536-D embeddings consume more storage than reduced variants
3. **Dependencies**: ONNX Runtime adds significant binary size (~50MB)
4. **GPU Support**: CUDA requires NVIDIA hardware; not portable to all field devices

### Risks

1. **Model Drift**: Embedding distributions may shift with model updates
   - Mitigation: Version embeddings with model version; re-index on major updates
2. **Latency Spikes**: Batch processing can introduce variable latency
   - Mitigation: Adaptive batching with timeout guarantees
3. **Memory Exhaustion**: Large batches can exhaust GPU memory
   - Mitigation: Dynamic batch sizing based on available memory

## References

- [Perch 2.0 Paper (arXiv)](https://arxiv.org/abs/2508.04665)
- [Perch ONNX Models (Hugging Face)](https://huggingface.co/justinchuby/Perch-onnx)
- [birdnet-onnx Crate (Docs.rs)](https://docs.rs/birdnet-onnx)
- [ONNX Runtime Rust Bindings](https://github.com/pykeio/ort)
- [Rubato Resampling Crate](https://docs.rs/rubato)
- [RuVector Repository](https://github.com/ruvnet/ruvector)

## Appendix A: Configuration Examples

### A.1 Field Device (Raspberry Pi 4)

```toml
[inference]
provider = "cpu"
intra_threads = 2
inter_threads = 1
max_batch_size = 1
model_variant = "quantized"

[preprocessing]
window_overlap = 0.25  # 25% to reduce compute
energy_threshold = 1e-5

[fallback]
strategies = ["deferred_queue"]
```

### A.2 Edge Server (NVIDIA Jetson)

```toml
[inference]
provider = "cuda"
device_id = 0
intra_threads = 4
inter_threads = 2
max_batch_size = 16
model_variant = "base"

[preprocessing]
window_overlap = 0.5
energy_threshold = 1e-6

[fallback]
strategies = ["quantized_model", "cached_embedding", "zero_vector"]
```

### A.3 Cloud Server (Multi-GPU)

```toml
[inference]
provider = "cuda"
device_ids = [0, 1, 2, 3]
intra_threads = 8
inter_threads = 4
max_batch_size = 64
model_variant = "base"

[preprocessing]
window_overlap = 0.75  # High resolution for research
energy_threshold = 1e-7

[fallback]
strategies = ["quantized_model", "cached_embedding"]

[verification]
enable_birdnet = true
birdnet_threshold = 0.5
```

## Appendix B: Embedding Quality Checklist

Before deploying embeddings to production:

- [ ] Embedding norms are within [0.99, 1.01] after L2 normalization
- [ ] No NaN or Inf values in any embedding
- [ ] Duplicate audio produces embeddings with cosine similarity > 0.99
- [ ] Silent audio produces consistent "silence" embedding
- [ ] Embedding distribution is roughly isotropic (no collapsed dimensions)
- [ ] Inter-batch consistency: same audio produces same embedding across batches
- [ ] Model version is recorded with each embedding
- [ ] BirdNET cross-validation shows > 80% top-5 agreement on known species
