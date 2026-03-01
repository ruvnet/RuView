# Sparse Inference Engine Architecture
## PowerInfer-style Activation Locality for Ruvector

**Version**: 1.0.0
**Date**: 2026-01-05
**Status**: Design Phase

---

## Executive Summary

This document defines the architecture for a sparse inference engine that exploits **activation locality** in transformer models. The system achieves 2-10x speedup with <1% accuracy loss by:

1. **Predicting** which neurons will activate using low-rank matrices (P·Q)
2. **Computing** only active neurons in FFN layers
3. **Caching** hot neurons in fast memory
4. **Offloading** cold neurons to slower storage

**Key Innovation**: Unlike model-wide quantization or pruning, we perform **neuron-level sparse computation** at runtime based on learned activation patterns.

---

## 1. System Architecture Overview

### 1.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Sparse Inference Engine                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  ┌───────────────┐      ┌──────────────┐      ┌─────────────────┐  │
│  │ Model Loader  │─────▶│  Calibrator  │─────▶│ Execution Engine│  │
│  │               │      │              │      │                 │  │
│  │ • GGUF Parser │      │ • P·Q Learn  │      │ • Layer Exec    │  │
│  │ • HF Loader   │      │ • Threshold  │      │ • Sparse Compute│  │
│  │ • Safetensors │      │ • Neuron Map │      │ • Backend Route │  │
│  └───────────────┘      └──────────────┘      └─────────────────┘  │
│         │                       │                       │            │
│         ▼                       ▼                       ▼            │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                      Neuron Cache Manager                      │  │
│  │  ┌──────────────┐  ┌───────────────┐  ┌──────────────────┐   │  │
│  │  │ Hot Neurons  │  │ Predictor Map │  │ Cold Neurons     │   │  │
│  │  │ (GPU/Memory) │  │ (P·Q Matrices)│  │ (Disk/Offload)   │   │  │
│  │  └──────────────┘  └───────────────┘  └──────────────────┘   │  │
│  └───────────────────────────────────────────────────────────────┘  │
│         │                       │                       │            │
│         ▼                       ▼                       ▼            │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                     Backend Abstraction                        │  │
│  │  ┌─────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐   │  │
│  │  │ CPU SIMD│  │ WASM SIMD│  │ GPU/Metal│  │ NPU (future) │   │  │
│  │  │ (AVX512)│  │ (128-bit)│  │ (Compute)│  │              │   │  │
│  │  └─────────┘  └──────────┘  └──────────┘  └──────────────┘   │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                       │
├─────────────────────────────────────────────────────────────────────┤
│                      Integration Layer                               │
│  ┌──────────────────────────┐    ┌──────────────────────────────┐  │
│  │   Ruvector Integration   │    │     RuvLLM Integration       │  │
│  │ • EmbeddingProvider      │    │ • InferenceBackend trait     │  │
│  │ • Sparse embed() calls   │    │ • generate() with sparsity   │  │
│  └──────────────────────────┘    └──────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 Component Interaction Flow

```
User Request
    │
    ▼
┌─────────────────┐
│ Model Selection │ (LFM2, sentence-bert, Llama GGUF)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Model Loader   │ Parse GGUF/HF → Extract layers, weights, config
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Calibration   │ Feed sample data → Learn P·Q matrices → Classify neurons
└────────┬────────┘         (Optional: Skip if pre-calibrated model)
         │
         ▼
┌─────────────────┐
│ Inference Setup │ Load hot neurons → Offload cold → Build predictor
└────────┬────────┘
         │
         ▼
┌─────────────────────────────────────────────┐
│        Runtime Inference Loop               │
│                                             │
│  Input → Embedding                          │
│     │                                       │
│     ▼                                       │
│  For each layer:                            │
│     1. Predictor(x) → Active neuron mask   │
│     2. Sparse Attention (full or sparse)   │
│     3. Sparse FFN (only active neurons)    │
│     4. LayerNorm + Residual                │
│     │                                       │
│     ▼                                       │
│  Output (embeddings/logits)                │
└─────────────────────────────────────────────┘
         │
         ▼
    User Result
```

---

## 2. Core Components

### 2.1 Model Loader

**Responsibility**: Parse and load transformer models from various formats.

#### Supported Formats

| Format | Use Case | Priority |
|--------|----------|----------|
| **GGUF** | Quantized Llama models (q4_0, q8_0) | P0 |
| **HuggingFace** | Sentence transformers (LFM2, BERT) | P0 |
| **Safetensors** | Modern PyTorch exports | P1 |
| **ONNX** | Cross-platform inference | P2 |

#### GGUF Parser Details

```rust
pub struct GGUFLoader {
    /// File handle to .gguf model
    file: File,
    /// Parsed metadata
    metadata: GGUFMetadata,
    /// Tensor mappings
    tensor_index: HashMap<String, TensorInfo>,
}

impl GGUFLoader {
    /// Parse header and build tensor index
    pub fn open(path: &Path) -> Result<Self>;

    /// Load specific layer weights
    pub fn load_layer(&self, layer_idx: usize) -> Result<LayerWeights>;

    /// Extract model config (n_layers, hidden_size, etc.)
    pub fn config(&self) -> ModelConfig;

    /// Check quantization type (Q4_0, Q8_0, F16)
    pub fn quantization(&self) -> QuantizationType;
}

pub struct LayerWeights {
    pub attention_qkv: Tensor,       // Combined Q,K,V weights
    pub attention_output: Tensor,
    pub ffn_gate: Tensor,            // FFN up-projection
    pub ffn_up: Tensor,
    pub ffn_down: Tensor,            // FFN down-projection
    pub norm1: Tensor,               // Pre-attention norm
    pub norm2: Tensor,               // Pre-FFN norm
}
```

#### HuggingFace Loader

```rust
pub struct HFLoader {
    model_id: String,
    cache_dir: PathBuf,
    tokenizer: Tokenizer,
}

impl HFLoader {
    /// Download or load cached model
    pub fn from_pretrained(model_id: &str) -> Result<Self>;

    /// Load full model into memory
    pub fn load_model(&self) -> Result<TransformerModel>;

    /// Stream-load layer by layer (for large models)
    pub fn load_layer_stream(&self) -> impl Iterator<Item = LayerWeights>;
}
```

#### Model Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub model_type: ModelType,        // Llama, BERT, GPT
    pub num_layers: usize,
    pub hidden_size: usize,           // Embedding dimension
    pub intermediate_size: usize,     // FFN intermediate dimension
    pub num_attention_heads: usize,
    pub vocab_size: usize,
    pub max_position_embeddings: usize,
    pub quantization: Option<QuantizationType>,
}

#[derive(Debug, Clone)]
pub enum ModelType {
    Llama,
    LlamaRoPE,
    BERT,
    SentenceBERT,
    LFM2,
}
```

---

### 2.2 Activation Predictor

**Responsibility**: Predict which neurons will activate without computing full FFN.

#### Low-Rank Predictor Architecture

The predictor uses **P·Q decomposition** where:
- **P ∈ ℝ^(hidden_size × r)**: Projection from hidden state
- **Q ∈ ℝ^(r × intermediate_size)**: Projection to neuron scores
- **r << hidden_size**: Rank (typically 64-256)

```
Input: x ∈ ℝ^hidden_size
Score: s = (P·x)·Q ∈ ℝ^intermediate_size  (low-rank computation)
Mask: m = (s > threshold)  (binary mask for active neurons)
```

#### Predictor Implementation

```rust
pub struct LowRankPredictor {
    /// P matrix: [hidden_size, rank]
    p_matrix: Tensor,
    /// Q matrix: [rank, intermediate_size]
    q_matrix: Tensor,
    /// Activation threshold per neuron
    thresholds: Vec<f32>,
    /// Neuron statistics (for threshold tuning)
    neuron_stats: Vec<NeuronStats>,
}

impl LowRankPredictor {
    /// Predict active neurons from hidden state
    pub fn predict(&self, hidden_state: &Tensor) -> NeuronMask {
        // scores = (hidden_state @ P) @ Q
        let proj = hidden_state.matmul(&self.p_matrix);  // [batch, rank]
        let scores = proj.matmul(&self.q_matrix);        // [batch, intermediate]

        // Apply thresholds
        let mask = scores.iter()
            .zip(&self.thresholds)
            .map(|(score, threshold)| score > threshold)
            .collect();

        NeuronMask::new(mask)
    }

    /// Get predicted sparsity ratio
    pub fn sparsity_ratio(&self, hidden_state: &Tensor) -> f32 {
        let mask = self.predict(hidden_state);
        mask.active_ratio()
    }
}

#[derive(Debug, Clone)]
pub struct NeuronMask {
    /// Boolean mask: true = compute, false = skip
    mask: Vec<bool>,
    /// Precomputed active indices (for sparse kernels)
    active_indices: Vec<usize>,
}

impl NeuronMask {
    pub fn active_count(&self) -> usize {
        self.active_indices.len()
    }

    pub fn active_ratio(&self) -> f32 {
        self.active_count() as f32 / self.mask.len() as f32
    }

    pub fn iter_active(&self) -> impl Iterator<Item = usize> + '_ {
        self.active_indices.iter().copied()
    }
}
```

#### Calibration Process

```rust
pub struct Calibrator {
    model: TransformerModel,
    config: CalibrationConfig,
}

#[derive(Debug, Clone)]
pub struct CalibrationConfig {
    /// Number of calibration samples
    pub num_samples: usize,
    /// Target sparsity (e.g., 0.2 = 80% neurons skipped)
    pub target_sparsity: f32,
    /// Predictor rank
    pub predictor_rank: usize,
    /// Calibration data source
    pub data_source: DataSource,
}

impl Calibrator {
    /// Run calibration to learn P, Q matrices
    pub fn calibrate(&mut self) -> Result<PredictorSet> {
        let samples = self.load_calibration_data()?;
        let mut predictors = Vec::new();

        for layer_idx in 0..self.model.num_layers() {
            // 1. Collect activation statistics
            let activations = self.collect_activations(layer_idx, &samples)?;

            // 2. Classify hot/cold neurons
            let classification = self.classify_neurons(&activations)?;

            // 3. Learn low-rank predictor
            let predictor = self.learn_predictor(
                layer_idx,
                &activations,
                &classification
            )?;

            predictors.push(predictor);
        }

        Ok(PredictorSet { predictors })
    }

    /// Collect FFN activations for layer
    fn collect_activations(
        &self,
        layer_idx: usize,
        samples: &[Tensor]
    ) -> Result<ActivationData> {
        let mut hidden_states = Vec::new();
        let mut ffn_activations = Vec::new();

        for input in samples {
            let hidden = self.model.forward_to_layer(input, layer_idx)?;
            let ffn_out = self.model.compute_ffn(layer_idx, &hidden)?;

            hidden_states.push(hidden);
            ffn_activations.push(ffn_out);
        }

        Ok(ActivationData {
            hidden_states,
            ffn_activations,
        })
    }

    /// Classify neurons as hot/cold based on activation frequency
    fn classify_neurons(&self, data: &ActivationData) -> Result<NeuronClassification> {
        let intermediate_size = data.ffn_activations[0].shape()[1];
        let mut activation_counts = vec![0usize; intermediate_size];

        // Count how often each neuron activates
        for activations in &data.ffn_activations {
            for (i, value) in activations.iter().enumerate() {
                if value.abs() > 1e-6 {  // Non-zero threshold
                    activation_counts[i] += 1;
                }
            }
        }

        // Compute activation frequency
        let total_samples = data.ffn_activations.len();
        let frequencies: Vec<f32> = activation_counts.iter()
            .map(|&count| count as f32 / total_samples as f32)
            .collect();

        // Classify: hot if frequency > threshold
        let hot_threshold = self.config.target_sparsity;
        let classification: Vec<NeuronType> = frequencies.iter()
            .map(|&freq| {
                if freq > hot_threshold {
                    NeuronType::Hot
                } else {
                    NeuronType::Cold
                }
            })
            .collect();

        Ok(NeuronClassification {
            types: classification,
            frequencies,
        })
    }

    /// Learn P, Q matrices via gradient descent
    fn learn_predictor(
        &self,
        layer_idx: usize,
        data: &ActivationData,
        classification: &NeuronClassification
    ) -> Result<LowRankPredictor> {
        let hidden_size = data.hidden_states[0].shape()[0];
        let intermediate_size = classification.types.len();
        let rank = self.config.predictor_rank;

        // Initialize P, Q with Xavier
        let mut p_matrix = Tensor::randn(&[hidden_size, rank]) * (2.0 / hidden_size as f32).sqrt();
        let mut q_matrix = Tensor::randn(&[rank, intermediate_size]) * (2.0 / rank as f32).sqrt();

        let optimizer = Adam::new(0.001);

        // Training loop
        for epoch in 0..100 {
            let mut total_loss = 0.0;

            for (hidden, target) in data.hidden_states.iter().zip(&data.ffn_activations) {
                // Forward: scores = (hidden @ P) @ Q
                let proj = hidden.matmul(&p_matrix);
                let scores = proj.matmul(&q_matrix);

                // Loss: binary cross-entropy on active/inactive neurons
                let loss = self.predictor_loss(&scores, target, classification);
                total_loss += loss.item();

                // Backward
                loss.backward();
                optimizer.step(&mut [&mut p_matrix, &mut q_matrix]);
            }

            if total_loss < 0.01 {
                break;  // Converged
            }
        }

        // Learn thresholds (per-neuron calibration)
        let thresholds = self.compute_thresholds(&p_matrix, &q_matrix, data)?;

        Ok(LowRankPredictor {
            p_matrix,
            q_matrix,
            thresholds,
            neuron_stats: self.compute_neuron_stats(classification),
        })
    }
}

#[derive(Debug, Clone)]
pub enum NeuronType {
    Hot,   // Frequently activates (>80% samples)
    Cold,  // Rarely activates (<20% samples)
}
```

---

### 2.3 Sparse FFN Computation

**Responsibility**: Compute FFN layer with only active neurons.

#### Standard FFN vs Sparse FFN

**Standard FFN**:
```
FFN(x) = down(activation(gate(x) ⊙ up(x)))
  where gate, up, down are full matrix multiplications
```

**Sparse FFN**:
```
1. mask = Predictor(x)  (predict active neurons)
2. gate_active = gate(x)[mask]  (sparse matmul: only active columns)
3. up_active = up(x)[mask]
4. hidden = activation(gate_active ⊙ up_active)
5. output = down_active(hidden)  (sparse matmul: only active rows)
```

#### Implementation

```rust
pub struct SparseFFN {
    /// Gate projection weights: [hidden_size, intermediate_size]
    gate_weights: Tensor,
    /// Up projection weights: [hidden_size, intermediate_size]
    up_weights: Tensor,
    /// Down projection weights: [intermediate_size, hidden_size]
    down_weights: Tensor,
    /// Activation function (SiLU, GELU, ReLU)
    activation: ActivationType,
    /// Predictor for this layer
    predictor: LowRankPredictor,
}

impl SparseFFN {
    pub fn forward(&self, hidden_state: &Tensor, backend: &dyn Backend) -> Result<Tensor> {
        // 1. Predict active neurons
        let mask = self.predictor.predict(hidden_state);

        if mask.active_ratio() > 0.8 {
            // Fallback to dense computation if too many neurons active
            return self.forward_dense(hidden_state, backend);
        }

        // 2. Sparse gate projection: only compute active columns
        let gate_active = backend.sparse_matmul_cols(
            hidden_state,
            &self.gate_weights,
            &mask
        )?;

        // 3. Sparse up projection
        let up_active = backend.sparse_matmul_cols(
            hidden_state,
            &self.up_weights,
            &mask
        )?;

        // 4. Activation: gate ⊙ up (element-wise)
        let activated = self.activation.apply(&gate_active.mul(&up_active)?)?;

        // 5. Sparse down projection: only active rows matter
        let output = backend.sparse_matmul_rows(
            &activated,
            &self.down_weights,
            &mask
        )?;

        Ok(output)
    }

    fn forward_dense(&self, hidden_state: &Tensor, backend: &dyn Backend) -> Result<Tensor> {
        // Standard dense FFN (fallback)
        let gate = hidden_state.matmul(&self.gate_weights)?;
        let up = hidden_state.matmul(&self.up_weights)?;
        let activated = self.activation.apply(&gate.mul(&up)?)?;
        activated.matmul(&self.down_weights)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ActivationType {
    SiLU,    // Llama models
    GELU,    // BERT models
    ReLU,    // Legacy models
}

impl ActivationType {
    pub fn apply(&self, x: &Tensor) -> Result<Tensor> {
        match self {
            Self::SiLU => x.mul(&x.sigmoid()?),  // x * σ(x)
            Self::GELU => x.gelu(),
            Self::ReLU => x.relu(),
        }
    }
}
```

#### Sparse Attention (Optional)

For very large models, attention can also be sparsified:

```rust
pub struct SparseAttention {
    /// Query, Key, Value weights (combined or separate)
    qkv_weights: Tensor,
    output_weights: Tensor,
    num_heads: usize,
    head_dim: usize,
    /// Attention mask pattern (e.g., local, strided)
    sparsity_pattern: AttentionPattern,
}

#[derive(Debug, Clone)]
pub enum AttentionPattern {
    /// Full attention (no sparsity)
    Full,
    /// Local attention (window size)
    Local { window_size: usize },
    /// Strided attention (BigBird style)
    Strided { stride: usize, window: usize },
    /// Learned sparse pattern
    Learned { mask: Tensor },
}
```

---

### 2.4 Neuron Cache Manager

**Responsibility**: Manage hot/cold neuron weights in memory hierarchy.

#### Cache Architecture

```
┌──────────────────────────────────────────────┐
│          Neuron Cache Hierarchy              │
├──────────────────────────────────────────────┤
│  L1: Hot Neurons (GPU Memory / Fast RAM)     │
│      - 10-20% most active neurons            │
│      - Always resident                       │
│      - FP16/FP32 precision                   │
├──────────────────────────────────────────────┤
│  L2: Warm Neurons (System RAM)               │
│      - 30-40% moderately active              │
│      - Loaded on-demand                      │
│      - INT8/FP16 quantized                   │
├──────────────────────────────────────────────┤
│  L3: Cold Neurons (Disk / Compressed)        │
│      - 40-60% rarely active                  │
│      - Lazy load if predicted               │
│      - INT4/INT8 quantized                   │
└──────────────────────────────────────────────┘
```

#### Implementation

```rust
pub struct NeuronCache {
    /// Model configuration
    config: ModelConfig,
    /// Per-layer cache
    layers: Vec<LayerCache>,
    /// Memory budget (bytes)
    memory_budget: usize,
    /// Current memory usage
    memory_used: usize,
}

pub struct LayerCache {
    /// Hot neuron indices
    hot_neurons: Vec<usize>,
    /// Hot neuron weights (gate, up, down)
    hot_weights: HotWeights,
    /// Cold neuron weights (memory-mapped or compressed)
    cold_weights: ColdWeights,
    /// Neuron statistics
    stats: Vec<NeuronStats>,
}

#[derive(Debug, Clone)]
pub struct HotWeights {
    /// Gate weights for hot neurons: [hidden_size, num_hot]
    gate: Tensor,
    /// Up weights for hot neurons: [hidden_size, num_hot]
    up: Tensor,
    /// Down weights for hot neurons: [num_hot, hidden_size]
    down: Tensor,
}

pub enum ColdWeights {
    /// Memory-mapped file (lazy load)
    MemoryMapped {
        file: Mmap,
        offsets: Vec<usize>,
    },
    /// Compressed in-memory
    Compressed {
        data: Vec<u8>,
        codec: CompressionCodec,
    },
    /// Quantized INT4
    Quantized {
        data: Vec<u8>,
        scales: Vec<f32>,
    },
}

impl NeuronCache {
    /// Build cache from calibration results
    pub fn from_calibration(
        model: &TransformerModel,
        predictors: &PredictorSet,
        config: CacheConfig
    ) -> Result<Self> {
        let mut layers = Vec::new();

        for (layer_idx, predictor) in predictors.iter().enumerate() {
            // Extract hot/cold neurons
            let hot_neurons: Vec<usize> = predictor.neuron_stats.iter()
                .enumerate()
                .filter(|(_, stats)| stats.neuron_type == NeuronType::Hot)
                .map(|(idx, _)| idx)
                .collect();

            // Load hot neuron weights into fast memory
            let layer_weights = model.get_layer_weights(layer_idx)?;
            let hot_weights = Self::extract_hot_weights(&layer_weights, &hot_neurons)?;

            // Compress cold neuron weights
            let cold_weights = Self::compress_cold_weights(
                &layer_weights,
                &hot_neurons,
                config.compression
            )?;

            layers.push(LayerCache {
                hot_neurons,
                hot_weights,
                cold_weights,
                stats: predictor.neuron_stats.clone(),
            });
        }

        Ok(Self {
            config: model.config.clone(),
            layers,
            memory_budget: config.memory_budget,
            memory_used: Self::calculate_memory(&layers),
        })
    }

    /// Get weights for active neurons (hot cached, cold loaded on-demand)
    pub fn get_active_weights(
        &self,
        layer_idx: usize,
        mask: &NeuronMask
    ) -> Result<ActiveWeights> {
        let cache = &self.layers[layer_idx];

        // Separate hot and cold neurons in mask
        let (hot_indices, cold_indices) = self.split_hot_cold(cache, mask);

        // Hot neurons: direct lookup
        let hot_weights = self.gather_hot_weights(cache, &hot_indices)?;

        // Cold neurons: lazy load
        let cold_weights = if !cold_indices.is_empty() {
            self.load_cold_weights(cache, &cold_indices)?
        } else {
            None
        };

        Ok(ActiveWeights {
            hot: hot_weights,
            cold: cold_weights,
        })
    }

    fn load_cold_weights(
        &self,
        cache: &LayerCache,
        indices: &[usize]
    ) -> Result<Option<Tensor>> {
        match &cache.cold_weights {
            ColdWeights::MemoryMapped { file, offsets } => {
                // Lazy load from disk
                let mut weights = Vec::new();
                for &idx in indices {
                    let offset = offsets[idx];
                    let data = &file[offset..offset + self.weight_size()];
                    weights.extend_from_slice(data);
                }
                Ok(Some(Tensor::from_bytes(&weights)?))
            }
            ColdWeights::Quantized { data, scales } => {
                // Dequantize on-the-fly
                let weights = Self::dequantize(data, scales, indices)?;
                Ok(Some(weights))
            }
            _ => Ok(None),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Memory budget in bytes
    pub memory_budget: usize,
    /// Compression for cold neurons
    pub compression: CompressionCodec,
    /// Whether to use memory-mapped files
    pub use_mmap: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum CompressionCodec {
    None,
    Quantize4Bit,
    Quantize8Bit,
    ZSTD,
}
```

---

### 2.5 Execution Engine

**Responsibility**: Orchestrate layer-by-layer inference with sparse computation.

```rust
pub struct ExecutionEngine {
    /// Model configuration
    config: ModelConfig,
    /// Neuron cache
    cache: NeuronCache,
    /// Predictors (one per layer)
    predictors: PredictorSet,
    /// Backend for computation
    backend: Arc<dyn Backend>,
    /// Performance metrics
    metrics: Metrics,
}

impl ExecutionEngine {
    /// Run inference on input
    pub fn forward(&mut self, input: &Tensor) -> Result<Tensor> {
        let batch_size = input.shape()[0];

        // 1. Embedding layer (always dense)
        let mut hidden = self.embed(input)?;

        // 2. Transformer layers
        for layer_idx in 0..self.config.num_layers {
            let start = std::time::Instant::now();

            // Attention (dense or sparse)
            hidden = self.run_attention(layer_idx, &hidden)?;

            // Sparse FFN
            hidden = self.run_sparse_ffn(layer_idx, &hidden)?;

            self.metrics.record_layer(layer_idx, start.elapsed());
        }

        // 3. Output layer
        let output = self.output_projection(&hidden)?;

        Ok(output)
    }

    fn run_sparse_ffn(&mut self, layer_idx: usize, hidden: &Tensor) -> Result<Tensor> {
        // 1. Predict active neurons
        let predictor = &self.predictors[layer_idx];
        let mask = predictor.predict(hidden);

        self.metrics.record_sparsity(layer_idx, mask.active_ratio());

        // 2. Get active neuron weights from cache
        let weights = self.cache.get_active_weights(layer_idx, &mask)?;

        // 3. Sparse FFN computation
        let ffn = SparseFFN::new(weights, predictor.clone());
        let output = ffn.forward(hidden, self.backend.as_ref())?;

        Ok(output)
    }

    /// Get inference statistics
    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }
}

#[derive(Debug, Default)]
pub struct Metrics {
    /// Per-layer latency
    layer_latency: Vec<Duration>,
    /// Per-layer sparsity ratio
    layer_sparsity: Vec<f32>,
    /// Total tokens processed
    tokens_processed: usize,
    /// Cache hits/misses
    cache_hits: usize,
    cache_misses: usize,
}

impl Metrics {
    pub fn average_sparsity(&self) -> f32 {
        self.layer_sparsity.iter().sum::<f32>() / self.layer_sparsity.len() as f32
    }

    pub fn total_latency(&self) -> Duration {
        self.layer_latency.iter().sum()
    }

    pub fn tokens_per_second(&self) -> f32 {
        let total_secs = self.total_latency().as_secs_f32();
        self.tokens_processed as f32 / total_secs
    }
}
```

---

### 2.6 Backend Abstraction

**Responsibility**: Provide SIMD-optimized sparse operations across platforms.

```rust
pub trait Backend: Send + Sync {
    /// Sparse matrix multiplication: A @ B[:, mask]
    fn sparse_matmul_cols(
        &self,
        a: &Tensor,
        b: &Tensor,
        col_mask: &NeuronMask
    ) -> Result<Tensor>;

    /// Sparse matrix multiplication: A[mask, :] @ B
    fn sparse_matmul_rows(
        &self,
        a: &Tensor,
        b: &Tensor,
        row_mask: &NeuronMask
    ) -> Result<Tensor>;

    /// Dense matrix multiplication (fallback)
    fn matmul(&self, a: &Tensor, b: &Tensor) -> Result<Tensor>;

    /// Element-wise operations
    fn add(&self, a: &Tensor, b: &Tensor) -> Result<Tensor>;
    fn mul(&self, a: &Tensor, b: &Tensor) -> Result<Tensor>;

    /// Activation functions
    fn silu(&self, x: &Tensor) -> Result<Tensor>;
    fn gelu(&self, x: &Tensor) -> Result<Tensor>;

    /// Quantization
    fn quantize(&self, x: &Tensor, bits: u8) -> Result<(Tensor, Vec<f32>)>;
    fn dequantize(&self, x: &Tensor, scales: &[f32]) -> Result<Tensor>;
}
```

#### CPU Backend (AVX512 SIMD)

```rust
pub struct CpuBackend {
    num_threads: usize,
    simd_features: SimdFeatures,
}

#[derive(Debug, Clone)]
pub struct SimdFeatures {
    pub avx512: bool,
    pub avx2: bool,
    pub fma: bool,
    pub vnni: bool,  // INT8 acceleration
}

impl Backend for CpuBackend {
    fn sparse_matmul_cols(
        &self,
        a: &Tensor,
        b: &Tensor,
        col_mask: &NeuronMask
    ) -> Result<Tensor> {
        // A: [batch, hidden_size]
        // B: [hidden_size, intermediate_size]
        // Output: [batch, active_neurons]

        let active_cols = col_mask.iter_active().collect::<Vec<_>>();

        if self.simd_features.avx512 {
            self.sparse_matmul_avx512(a, b, &active_cols)
        } else if self.simd_features.avx2 {
            self.sparse_matmul_avx2(a, b, &active_cols)
        } else {
            self.sparse_matmul_scalar(a, b, &active_cols)
        }
    }
}

impl CpuBackend {
    #[target_feature(enable = "avx512f")]
    unsafe fn sparse_matmul_avx512(
        &self,
        a: &Tensor,
        b: &Tensor,
        active_cols: &[usize]
    ) -> Result<Tensor> {
        // AVX-512: 16x f32 per vector
        // Optimized sparse GEMM kernel

        let batch = a.shape()[0];
        let hidden = a.shape()[1];
        let num_active = active_cols.len();

        let mut output = Tensor::zeros(&[batch, num_active]);

        for row in 0..batch {
            for (out_idx, &col) in active_cols.iter().enumerate() {
                // Dot product: a[row, :] · b[:, col]
                let mut sum = _mm512_setzero_ps();

                for k in (0..hidden).step_by(16) {
                    let a_vec = _mm512_loadu_ps(&a.data()[row * hidden + k]);
                    let b_vec = _mm512_loadu_ps(&b.data()[col * hidden + k]);
                    sum = _mm512_fmadd_ps(a_vec, b_vec, sum);
                }

                output.data_mut()[row * num_active + out_idx] =
                    _mm512_reduce_add_ps(sum);
            }
        }

        Ok(output)
    }
}
```

#### WASM Backend (Portable SIMD)

```rust
#[cfg(target_arch = "wasm32")]
pub struct WasmBackend {
    simd_enabled: bool,
}

#[cfg(target_arch = "wasm32")]
impl Backend for WasmBackend {
    fn sparse_matmul_cols(
        &self,
        a: &Tensor,
        b: &Tensor,
        col_mask: &NeuronMask
    ) -> Result<Tensor> {
        use std::arch::wasm32::*;

        if !self.simd_enabled {
            return self.sparse_matmul_scalar(a, b, col_mask);
        }

        // WASM SIMD: 4x f32 per v128
        let active_cols = col_mask.iter_active().collect::<Vec<_>>();
        let batch = a.shape()[0];
        let hidden = a.shape()[1];
        let num_active = active_cols.len();

        let mut output = Tensor::zeros(&[batch, num_active]);

        for row in 0..batch {
            for (out_idx, &col) in active_cols.iter().enumerate() {
                let mut sum = f32x4_splat(0.0);

                for k in (0..hidden).step_by(4) {
                    let a_vec = v128_load(&a.data()[row * hidden + k] as *const f32 as *const v128);
                    let b_vec = v128_load(&b.data()[col * hidden + k] as *const f32 as *const v128);
                    sum = f32x4_add(sum, f32x4_mul(a_vec, b_vec));
                }

                // Horizontal sum
                let result = f32x4_extract_lane::<0>(sum)
                    + f32x4_extract_lane::<1>(sum)
                    + f32x4_extract_lane::<2>(sum)
                    + f32x4_extract_lane::<3>(sum);

                output.data_mut()[row * num_active + out_idx] = result;
            }
        }

        Ok(output)
    }
}
```

---

## 3. Data Flow Architecture

### 3.1 Model Loading Flow

```
User: model_path
    │
    ▼
┌─────────────────────────────┐
│  Detect Model Format        │ (check extension: .gguf, .safetensors, .bin)
└────────┬────────────────────┘
         │
         ├──── .gguf ────────▶ GGUFLoader::open()
         │                       │
         │                       ├─ Parse header
         │                       ├─ Build tensor index
         │                       └─ Extract config
         │
         ├──── HF/ST ────────▶ HFLoader::from_pretrained()
         │                       │
         │                       ├─ Download if needed
         │                       ├─ Load safetensors
         │                       └─ Parse config.json
         │
         ▼
┌─────────────────────────────┐
│  TransformerModel           │
│  - Config                   │
│  - Layer weights            │
│  - Tokenizer                │
└─────────────────────────────┘
```

### 3.2 Calibration Flow

```
TransformerModel
    │
    ▼
┌─────────────────────────────┐
│  Load Calibration Dataset   │ (WikiText, C4, custom)
│  - Sample 512-2048 examples │
└────────┬────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│  For each layer:                        │
│  1. Forward samples → collect           │
│     - Hidden states (input to FFN)      │
│     - FFN activations (output)          │
│                                         │
│  2. Analyze activations:                │
│     - Compute activation frequency      │
│     - Classify hot/cold neurons         │
│                                         │
│  3. Learn predictor:                    │
│     - Initialize P, Q matrices          │
│     - Train on (hidden → activation)    │
│     - Optimize thresholds               │
└────────┬────────────────────────────────┘
         │
         ▼
┌─────────────────────────────┐
│  PredictorSet + NeuronCache │
│  - P·Q matrices per layer   │
│  - Hot neuron weights       │
│  - Cold neuron offload      │
└─────────────────────────────┘
```

### 3.3 Inference Flow (Single Token)

```
Input Token(s)
    │
    ▼
┌─────────────────────────────┐
│  Tokenizer + Embedding      │
└────────┬────────────────────┘
         │
         ▼
┌───────────────────────────────────────────────────────┐
│  Layer 0:                                             │
│  ┌─────────────────────────────────────────────────┐ │
│  │ 1. Attention (dense)                            │ │
│  │    - Q, K, V projections                        │ │
│  │    - Scaled dot-product attention               │ │
│  │    - Output projection                          │ │
│  └─────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────┐ │
│  │ 2. Sparse FFN                                   │ │
│  │    a) Predictor(hidden) → mask [T/F/F/T/T/F...] │ │
│  │    b) Load weights for active neurons only      │ │
│  │    c) Sparse gate/up projections                │ │
│  │    d) Activation (SiLU/GELU)                    │ │
│  │    e) Sparse down projection                    │ │
│  └─────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────┐ │
│  │ 3. Residual + LayerNorm                         │ │
│  └─────────────────────────────────────────────────┘ │
└───────────────────┬───────────────────────────────────┘
                    │
                    ▼
                (Repeat for layers 1..N-1)
                    │
                    ▼
┌─────────────────────────────┐
│  Output Projection          │
│  - Linear(hidden, vocab)    │
│  - Softmax (for generation) │
└────────┬────────────────────┘
         │
         ▼
    Logits / Embedding
```

---

## 4. Rust Module Structure

### 4.1 Crate Layout

```
crates/ruvector-sparse-inference/
├── Cargo.toml
├── build.rs                     # Build-time feature detection
├── README.md
└── src/
    ├── lib.rs                   # Public API
    ├── config.rs                # Configuration types
    ├── error.rs                 # Error types
    │
    ├── predictor/
    │   ├── mod.rs               # Predictor API
    │   ├── lowrank.rs           # P·Q low-rank predictor
    │   ├── calibration.rs       # Calibration logic
    │   └── threshold.rs         # Threshold optimization
    │
    ├── sparse/
    │   ├── mod.rs               # Sparse operations API
    │   ├── ffn.rs               # Sparse FFN layer
    │   ├── attention.rs         # Sparse attention (optional)
    │   └── kernels.rs           # SIMD kernels
    │
    ├── model/
    │   ├── mod.rs               # Model loading API
    │   ├── gguf.rs              # GGUF parser
    │   ├── hf.rs                # HuggingFace loader
    │   ├── loader.rs            # Generic loader trait
    │   └── runners.rs           # Model-specific runners (Llama, BERT)
    │
    ├── memory/
    │   ├── mod.rs               # Memory management API
    │   ├── cache.rs             # Neuron cache
    │   ├── quantization.rs      # Quantization utilities
    │   └── compression.rs       # Compression codecs
    │
    ├── backend/
    │   ├── mod.rs               # Backend trait
    │   ├── cpu.rs               # CPU SIMD backend
    │   ├── wasm.rs              # WASM SIMD backend
    │   └── gpu.rs               # GPU backend (future)
    │
    ├── integration/
    │   ├── mod.rs               # Integration API
    │   ├── ruvector.rs          # EmbeddingProvider impl
    │   └── ruvllm.rs            # InferenceBackend impl
    │
    └── utils/
        ├── mod.rs
        ├── tensor.rs            # Tensor utilities
        └── metrics.rs           # Performance tracking
```

### 4.2 Key Module Responsibilities

| Module | Responsibility | Dependencies |
|--------|---------------|-------------|
| `lib.rs` | Public API, re-exports | All modules |
| `config` | Configuration types | None |
| `error` | Error handling | None |
| `predictor` | Neuron prediction | `tensor`, `backend` |
| `sparse` | Sparse computation | `predictor`, `backend`, `memory` |
| `model` | Model loading | `config`, `error` |
| `memory` | Cache management | `model`, `predictor` |
| `backend` | SIMD operations | `tensor` |
| `integration` | Ruvector/RuvLLM | All |

---

## 5. Key Traits and Interfaces

### 5.1 ModelRunner Trait

```rust
pub trait ModelRunner: Send + Sync {
    /// Get model configuration
    fn config(&self) -> &ModelConfig;

    /// Run inference on input tokens
    fn forward(&mut self, input_ids: &[u32]) -> Result<Tensor>;

    /// Encode text to embeddings (for embedding models)
    fn encode(&mut self, text: &str) -> Result<Vec<f32>>;

    /// Generate text (for language models)
    fn generate(&mut self, prompt: &str, max_tokens: usize) -> Result<String>;

    /// Get inference metrics
    fn metrics(&self) -> &Metrics;
}

// Implementations
pub struct LlamaRunner { /* ... */ }
pub struct BertRunner { /* ... */ }
pub struct LFM2Runner { /* ... */ }

impl ModelRunner for LlamaRunner { /* ... */ }
impl ModelRunner for BertRunner { /* ... */ }
impl ModelRunner for LFM2Runner { /* ... */ }
```

### 5.2 Predictor Trait

```rust
pub trait Predictor: Send + Sync {
    /// Predict active neurons from hidden state
    fn predict(&self, hidden_state: &Tensor) -> NeuronMask;

    /// Get predicted sparsity ratio
    fn sparsity_ratio(&self, hidden_state: &Tensor) -> f32;

    /// Get neuron statistics
    fn neuron_stats(&self) -> &[NeuronStats];
}

#[derive(Debug, Clone)]
pub struct NeuronStats {
    pub neuron_type: NeuronType,
    pub activation_frequency: f32,
    pub average_magnitude: f32,
}
```

### 5.3 Cache Trait

```rust
pub trait Cache: Send + Sync {
    /// Get weights for active neurons
    fn get_active_weights(
        &self,
        layer_idx: usize,
        mask: &NeuronMask
    ) -> Result<ActiveWeights>;

    /// Get memory usage statistics
    fn memory_usage(&self) -> MemoryStats;

    /// Evict least-recently-used cold neurons
    fn evict(&mut self, size: usize) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub hot_neurons_bytes: usize,
    pub cold_neurons_bytes: usize,
    pub predictor_bytes: usize,
    pub total_bytes: usize,
}
```

---

## 6. Integration Architecture

### 6.1 Ruvector EmbeddingProvider Integration

```rust
// In ruvector-core/src/embeddings.rs
pub trait EmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}

// New implementation in sparse-inference
impl EmbeddingProvider for SparseInferenceEngine {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // 1. Tokenize
        let tokens = self.tokenizer.encode(text)?;

        // 2. Run sparse inference
        let output = self.runner.forward(&tokens)?;

        // 3. Mean pooling (for sentence embeddings)
        let embedding = self.mean_pool(&output)?;

        Ok(embedding.to_vec())
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|text| self.embed(text)).collect()
    }
}

// Usage
let engine = SparseInferenceEngine::from_pretrained(
    "TaylorAI/gte-tiny",
    SparseConfig::default()
)?;

let rv = RuVector::builder()
    .embedding_provider(Box::new(engine))
    .build()?;

rv.insert("test", "Hello world")?;
```

### 6.2 RuvLLM InferenceBackend Integration

```rust
// In ruvllm/src/backend.rs
pub trait InferenceBackend {
    fn generate(&mut self, prompt: &str, config: GenerateConfig) -> Result<String>;
    fn logits(&mut self, prompt: &str) -> Result<Vec<f32>>;
}

// Implementation
impl InferenceBackend for SparseInferenceEngine {
    fn generate(&mut self, prompt: &str, config: GenerateConfig) -> Result<String> {
        let mut tokens = self.tokenizer.encode(prompt)?;
        let mut output = String::new();

        for _ in 0..config.max_tokens {
            // Sparse inference
            let logits = self.runner.forward(&tokens)?;

            // Sample next token
            let next_token = self.sample(&logits, config.temperature)?;
            tokens.push(next_token);

            // Decode
            let text = self.tokenizer.decode(&[next_token])?;
            output.push_str(&text);

            if next_token == self.tokenizer.eos_token() {
                break;
            }
        }

        Ok(output)
    }
}

// Usage
let engine = SparseInferenceEngine::from_pretrained(
    "TheBloke/Llama-2-7B-GGUF",
    SparseConfig::default()
)?;

let llm = RuvLLM::builder()
    .backend(Box::new(engine))
    .build()?;

let response = llm.generate("Explain quantum computing", Default::default())?;
```

---

## 7. Performance Targets

### 7.1 Latency Targets

| Model | Operation | Target Latency | Baseline | Speedup |
|-------|-----------|----------------|----------|---------|
| **LFM2-350M** | Sentence embedding | 5-10ms | 25ms | 2.5-5x |
| **BERT-base** | Sentence embedding | 8-15ms | 40ms | 2.7-5x |
| **Llama-7B** | Token generation | 50-100ms | 500ms | 5-10x |
| **Llama-13B** | Token generation | 100-200ms | 1.2s | 6-12x |

### 7.2 Memory Targets

| Model | Baseline RAM | Sparse RAM | Reduction |
|-------|--------------|------------|-----------|
| **LFM2-350M** | 1.4 GB | 700 MB | 2x |
| **Llama-7B (FP16)** | 14 GB | 7-9 GB | 1.5-2x |
| **Llama-7B (Q4)** | 4 GB | 2.5-3 GB | 1.3-1.6x |

### 7.3 Accuracy Targets

- **Embedding similarity**: >0.99 cosine similarity to dense baseline
- **Generation quality**: <1% perplexity increase
- **Classification accuracy**: <0.5% drop on downstream tasks

### 7.4 Sparsity Targets

| Layer Type | Target Sparsity | Active Neurons |
|------------|-----------------|----------------|
| **Early layers** | 60-70% | 30-40% compute |
| **Middle layers** | 70-85% | 15-30% compute |
| **Late layers** | 50-60% | 40-50% compute |
| **Average** | 70-80% | 20-30% compute |

---

## 8. Deployment Architecture

### 8.1 CPU Deployment

```
┌─────────────────────────────────────┐
│  Application Process                │
├─────────────────────────────────────┤
│  ┌───────────────────────────────┐  │
│  │  Sparse Inference Engine      │  │
│  │  - Hot neurons: RAM (1-2 GB)  │  │
│  │  - Cold neurons: mmap disk    │  │
│  │  - SIMD: AVX-512 / AVX2       │  │
│  └───────────────────────────────┘  │
│  ┌───────────────────────────────┐  │
│  │  Thread Pool (rayon)          │  │
│  │  - Parallel batch processing  │  │
│  └───────────────────────────────┘  │
└─────────────────────────────────────┘
```

### 8.2 WASM Deployment

```
┌─────────────────────────────────────┐
│  Browser / Node.js                  │
├─────────────────────────────────────┤
│  ┌───────────────────────────────┐  │
│  │  WASM Module                  │  │
│  │  - Hot neurons: ArrayBuffer   │  │
│  │  - SIMD: wasm128 (if avail)   │  │
│  │  - Memory limit: 2-4 GB       │  │
│  └───────────────────────────────┘  │
│  ┌───────────────────────────────┐  │
│  │  Worker Pool                  │  │
│  │  - Parallel inference         │  │
│  └───────────────────────────────┘  │
└─────────────────────────────────────┘
```

### 8.3 Hybrid Deployment

```
┌─────────────────────────────────────┐
│  Cloud GPU (Hot path)               │
│  - 20% most frequent queries        │
│  - Full dense inference             │
│  - <10ms latency                    │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│  Edge CPU (Cold path)               │
│  - 80% long-tail queries            │
│  - Sparse inference                 │
│  - 20-50ms latency                  │
│  - 10x lower cost                   │
└─────────────────────────────────────┘
```

---

## 9. Future Enhancements

### 9.1 Phase 2 Features

- **Dynamic sparsity**: Adjust predictor thresholds at runtime
- **Multi-modal**: Support vision-language models (CLIP, LLaVA)
- **Quantization-aware**: INT8/INT4 predictor matrices
- **GPU kernels**: CUDA/Metal sparse kernels
- **NPU support**: Apple Neural Engine, Qualcomm Hexagon

### 9.2 Phase 3 Features

- **Learned sparsity patterns**: Train end-to-end with sparsity loss
- **Mixture-of-experts**: Combine with MoE models
- **Speculative decoding**: Sparse draft models + dense verification
- **Cross-layer optimization**: Share predictors across layers

---

## 10. References and Inspiration

1. **PowerInfer** (SOSP'23): Fast LLM serving with activation locality
2. **DejaVu** (MLSys'23): Contextual sparsity in transformers
3. **CATS** (ICLR'24): Context-aware token selection
4. **FlashAttention**: Memory-efficient attention
5. **GPTQ/AWQ**: Weight quantization for LLMs

---

## Appendix A: Configuration Examples

### A.1 LFM2 Embedding Configuration

```rust
let config = SparseConfig {
    model_path: "TaylorAI/gte-tiny".to_string(),
    predictor_rank: 128,
    target_sparsity: 0.75,
    cache_config: CacheConfig {
        memory_budget: 1024 * 1024 * 1024,  // 1 GB
        use_mmap: false,
        compression: CompressionCodec::None,
    },
    backend: BackendType::CpuAvx2,
    calibration: Some(CalibrationConfig {
        num_samples: 1024,
        data_source: DataSource::WikiText,
    }),
};
```

### A.2 Llama-7B Generation Configuration

```rust
let config = SparseConfig {
    model_path: "TheBloke/Llama-2-7B-GGUF".to_string(),
    predictor_rank: 256,
    target_sparsity: 0.80,
    cache_config: CacheConfig {
        memory_budget: 8 * 1024 * 1024 * 1024,  // 8 GB
        use_mmap: true,
        compression: CompressionCodec::Quantize4Bit,
    },
    backend: BackendType::CpuAvx512,
    calibration: Some(CalibrationConfig {
        num_samples: 2048,
        data_source: DataSource::C4,
    }),
};
```

---

## Appendix B: Benchmarking Protocol

### B.1 Latency Benchmarking

```bash
# Embedding models
cargo bench --bench embeddings -- \
  --model gte-tiny \
  --batch-sizes 1,8,32 \
  --sequence-lengths 16,64,256

# Generation models
cargo bench --bench generation -- \
  --model llama-7b \
  --prompt-lengths 32,128,512 \
  --generate-lengths 32,128
```

### B.2 Accuracy Evaluation

```bash
# STS-B (semantic similarity)
cargo run --release --bin eval-sts \
  --model gte-tiny \
  --sparse \
  --dataset data/stsbenchmark

# MMLU (language understanding)
cargo run --release --bin eval-mmlu \
  --model llama-7b \
  --sparse \
  --subset abstract_algebra,anatomy
```

---

**End of Architecture Document**
