# RuvLLM: System Architecture

## SPARC Phase 3: Architecture

---

## 1. High-Level Architecture

### 1.1 System Overview Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              RuvLLM System                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐                                                            │
│  │   Client     │                                                            │
│  │   Request    │                                                            │
│  └──────┬───────┘                                                            │
│         │                                                                    │
│         ▼                                                                    │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                        Orchestrator Layer                             │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐    │   │
│  │  │ Request │  │ Session │  │ Metrics │  │ Limiter │  │  Cache  │    │   │
│  │  │ Router  │  │ Manager │  │Collector│  │         │  │ Manager │    │   │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘    │   │
│  └──────────────────────────────┬───────────────────────────────────────┘   │
│                                 │                                            │
│         ┌───────────────────────┼───────────────────────┐                   │
│         │                       │                       │                   │
│         ▼                       ▼                       ▼                   │
│  ┌─────────────┐        ┌─────────────┐        ┌─────────────┐             │
│  │  Embedding  │        │   FastGRNN  │        │    Graph    │             │
│  │   Service   │        │   Router    │        │  Attention  │             │
│  │             │        │             │        │   Engine    │             │
│  │ ┌─────────┐ │        │ ┌─────────┐ │        │ ┌─────────┐ │             │
│  │ │  LFM2   │ │        │ │  Gated  │ │        │ │MultiHead│ │             │
│  │ │ Encoder │ │        │ │   RNN   │ │        │ │Attention│ │             │
│  │ └─────────┘ │        │ └─────────┘ │        │ └─────────┘ │             │
│  │ ┌─────────┐ │        │ ┌─────────┐ │        │ ┌─────────┐ │             │
│  │ │Dimension│ │        │ │ Output  │ │        │ │  Edge   │ │             │
│  │ │ Adapter │ │        │ │  Heads  │ │        │ │Features │ │             │
│  │ └─────────┘ │        │ └─────────┘ │        │ └─────────┘ │             │
│  └──────┬──────┘        └──────┬──────┘        └──────┬──────┘             │
│         │                      │                      │                     │
│         └──────────────────────┼──────────────────────┘                     │
│                                │                                            │
│                                ▼                                            │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                        Memory Layer (Ruvector)                        │   │
│  │                                                                        │   │
│  │  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐   ┌──────────┐  │   │
│  │  │    HNSW     │   │   Graph     │   │  Metadata   │   │ Writeback│  │   │
│  │  │   Index     │   │   Store     │   │   Store     │   │  Queue   │  │   │
│  │  │             │   │             │   │             │   │          │  │   │
│  │  │  M=32       │   │  Nodes +    │   │  JSON/BSON  │   │ Async    │  │   │
│  │  │  efC=200    │   │  Edges      │   │  Filters    │   │ Persist  │  │   │
│  │  └─────────────┘   └─────────────┘   └─────────────┘   └──────────┘  │   │
│  │                                                                        │   │
│  │  Storage Backend: redb (embedded) | PostgreSQL (cluster)              │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│                                ▼                                            │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                        Inference Layer (LFM2)                         │   │
│  │                                                                        │   │
│  │  ┌─────────────────────────────────────────────────────────────────┐  │   │
│  │  │                      Model Pool                                  │  │   │
│  │  │                                                                   │  │   │
│  │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐            │  │   │
│  │  │  │  350M   │  │  700M   │  │  1.2B   │  │  2.6B   │            │  │   │
│  │  │  │  Q4_K   │  │  Q4_K   │  │  Q5_K   │  │  FP16   │            │  │   │
│  │  │  │ (Edge)  │  │(Mobile) │  │(Server) │  │ (Judge) │            │  │   │
│  │  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘            │  │   │
│  │  └─────────────────────────────────────────────────────────────────┘  │   │
│  │                                                                        │   │
│  │  Backend: llama.cpp (CPU) | vLLM (GPU) | ExecuTorch (NPU)            │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│                                ▼                                            │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                       Self-Learning Layer                             │   │
│  │                                                                        │   │
│  │  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐   ┌──────────┐  │   │
│  │  │   Quality   │   │   Replay    │   │    EWC      │   │ Training │  │   │
│  │  │    Judge    │   │   Buffer    │   │ Regularizer │   │  Loop    │  │   │
│  │  │             │   │             │   │             │   │          │  │   │
│  │  │ LLM-as-     │   │ Reservoir   │   │ Fisher Info │   │ Online   │  │   │
│  │  │ Judge       │   │ Sampling    │   │ + θ*        │   │ Updates  │  │   │
│  │  └─────────────┘   └─────────────┘   └─────────────┘   └──────────┘  │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Component Interaction Flow

```
┌──────┐      ┌───────────┐      ┌────────┐      ┌───────┐      ┌──────────┐
│Client│      │Orchestrator│     │Embedder│      │Ruvector│     │ Router   │
└──┬───┘      └─────┬─────┘      └───┬────┘      └───┬───┘      └────┬─────┘
   │                │                │              │               │
   │  Query         │                │              │               │
   │───────────────▶│                │              │               │
   │                │                │              │               │
   │                │  Embed Query   │              │               │
   │                │───────────────▶│              │               │
   │                │                │              │               │
   │                │  Embedding     │              │               │
   │                │◀───────────────│              │               │
   │                │                │              │               │
   │                │  HNSW Search   │              │               │
   │                │───────────────────────────────▶              │
   │                │                │              │               │
   │                │  Candidates    │              │               │
   │                │◀──────────────────────────────│               │
   │                │                │              │               │
   │                │  Build Features│              │               │
   │                │───────────────────────────────────────────────▶
   │                │                │              │               │
   │                │  Routing Decision             │               │
   │                │◀──────────────────────────────────────────────│
   │                │                │              │               │

┌──────┐      ┌───────────┐      ┌─────────┐      ┌───────┐      ┌──────────┐
│Client│      │Orchestrator│     │  Graph  │      │  LFM2 │      │ Learning │
└──┬───┘      └─────┬─────┘      │Attention│      └───┬───┘      └────┬─────┘
   │                │            └────┬────┘          │               │
   │                │                 │               │               │
   │                │  Graph Attention│               │               │
   │                │────────────────▶│               │               │
   │                │                 │               │               │
   │                │  Context        │               │               │
   │                │◀────────────────│               │               │
   │                │                 │               │               │
   │                │  Generate       │               │               │
   │                │─────────────────────────────────▶               │
   │                │                 │               │               │
   │                │  Response       │               │               │
   │                │◀────────────────────────────────│               │
   │                │                 │               │               │
   │                │  Evaluate + Learn               │               │
   │                │─────────────────────────────────────────────────▶
   │                │                 │               │               │
   │  Response      │                 │               │               │
   │◀───────────────│                 │               │               │
   │                │                 │               │               │
```

---

## 2. Component Architecture

### 2.1 Orchestrator Layer

```rust
/// Main orchestrator coordinating all system components
pub struct Orchestrator {
    /// Request routing and load balancing
    request_router: RequestRouter,
    /// Session state management
    session_manager: SessionManager,
    /// Metrics collection and export
    metrics_collector: MetricsCollector,
    /// Rate limiting and throttling
    rate_limiter: RateLimiter,
    /// Response caching
    cache_manager: CacheManager,
    /// Component references
    components: OrchestratorComponents,
}

pub struct OrchestratorComponents {
    embedder: Arc<EmbeddingService>,
    router: Arc<FastGRNNRouter>,
    memory: Arc<RuvectorMemory>,
    graph_attention: Arc<GraphAttentionEngine>,
    inference: Arc<LFM2InferencePool>,
    learning: Arc<SelfLearningService>,
}

impl Orchestrator {
    pub async fn process(&self, request: Request) -> Result<Response> {
        // Rate limiting
        self.rate_limiter.check(&request)?;

        // Cache check
        if let Some(cached) = self.cache_manager.get(&request).await {
            return Ok(cached);
        }

        // Get or create session
        let session = self.session_manager.get_or_create(&request.session_id);

        // Core processing pipeline
        let response = self.process_pipeline(request, session).await?;

        // Cache response
        self.cache_manager.put(&request, &response).await;

        // Metrics
        self.metrics_collector.record(&response);

        Ok(response)
    }
}
```

### 2.2 Embedding Service

```rust
/// Service for converting text to vector embeddings
pub struct EmbeddingService {
    /// LFM2 encoder model
    encoder: LFM2Encoder,
    /// Dimension projection layer
    projector: Linear,
    /// Normalization layer
    layer_norm: LayerNorm,
    /// Token count estimator
    tokenizer: Tokenizer,
    /// Configuration
    config: EmbeddingConfig,
}

pub struct EmbeddingConfig {
    /// Input dimension from encoder
    pub encoder_dim: usize,
    /// Output dimension for ruvector
    pub output_dim: usize,
    /// Maximum token length
    pub max_tokens: usize,
    /// Batch size for efficiency
    pub batch_size: usize,
}

impl EmbeddingService {
    pub fn embed(&self, text: &str) -> Result<Embedding> {
        // Tokenize and truncate
        let tokens = self.tokenizer.encode(text)?;
        let tokens = tokens.truncate(self.config.max_tokens);

        // Encode via LFM2
        let raw_embedding = self.encoder.encode(&tokens)?;

        // Project to output dimension
        let projected = self.projector.forward(&raw_embedding);

        // Normalize
        let normalized = self.layer_norm.forward(&projected);

        Ok(Embedding {
            vector: normalized,
            token_count: tokens.len(),
            truncated: tokens.len() >= self.config.max_tokens,
        })
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        texts.par_chunks(self.config.batch_size)
            .flat_map(|batch| {
                batch.iter().map(|t| self.embed(t)).collect::<Vec<_>>()
            })
            .collect()
    }
}
```

### 2.3 FastGRNN Router Architecture

```rust
/// FastGRNN-based intelligent router for resource allocation
pub struct FastGRNNRouter {
    /// FastGRNN cell weights
    cell: FastGRNNCell,
    /// Output projection heads
    output_heads: RouterOutputHeads,
    /// Feature normalization
    input_norm: LayerNorm,
    /// Configuration
    config: RouterConfig,
}

pub struct FastGRNNCell {
    /// Input-to-update gate weights
    w_z: SparseMatrix,
    /// Recurrent-to-update gate weights (low-rank)
    u_z: LowRankMatrix,
    /// Update gate bias
    b_z: Vector,
    /// Input-to-hidden weights
    w_h: SparseMatrix,
    /// Recurrent-to-hidden weights (low-rank)
    u_h: LowRankMatrix,
    /// Hidden bias
    b_h: Vector,
    /// FastGRNN scalars
    zeta: f32,
    nu: f32,
}

pub struct RouterOutputHeads {
    /// Model selection head: [hidden_dim] -> [4]
    model_head: Linear,
    /// Context size head: [hidden_dim] -> [5]
    context_head: Linear,
    /// Temperature head: [hidden_dim] -> [1]
    temperature_head: Linear,
    /// Top-p head: [hidden_dim] -> [1]
    top_p_head: Linear,
    /// Confidence head: [hidden_dim] -> [1]
    confidence_head: Linear,
}

pub struct RouterConfig {
    pub input_dim: usize,     // 128
    pub hidden_dim: usize,    // 64
    pub sparsity: f32,        // 0.9 for W matrices
    pub rank: usize,          // 8 for U matrices
    pub confidence_threshold: f32,
}

impl FastGRNNRouter {
    pub fn forward(
        &self,
        features: &[f32],
        hidden: &[f32],
    ) -> Result<(RoutingDecision, Vec<f32>)> {
        // Normalize input
        let x = self.input_norm.forward(features);

        // FastGRNN cell
        let h_new = self.cell.forward(&x, hidden);

        // Output heads
        let model_logits = self.output_heads.model_head.forward(&h_new);
        let context_logits = self.output_heads.context_head.forward(&h_new);
        let temp_raw = self.output_heads.temperature_head.forward(&h_new);
        let top_p_raw = self.output_heads.top_p_head.forward(&h_new);
        let conf_raw = self.output_heads.confidence_head.forward(&h_new);

        // Activations
        let model_probs = softmax(&model_logits);
        let context_probs = softmax(&context_logits);
        let temperature = sigmoid(temp_raw[0]) * 2.0;
        let top_p = sigmoid(top_p_raw[0]);
        let confidence = sigmoid(conf_raw[0]);

        // Decode decisions
        let decision = if confidence >= self.config.confidence_threshold {
            RoutingDecision {
                model: ModelSize::from_index(argmax(&model_probs)),
                context_size: CONTEXT_BINS[argmax(&context_probs)],
                temperature,
                top_p,
                confidence,
                model_probs: model_probs.try_into()?,
            }
        } else {
            RoutingDecision::default_safe()
        };

        Ok((decision, h_new))
    }
}
```

### 2.4 Ruvector Memory Layer

```rust
/// Unified memory interface combining vector search and graph
pub struct RuvectorMemory {
    /// Core vector database
    vector_db: VectorDB,
    /// Graph store for relationships
    graph_store: GraphStore,
    /// Metadata index for filtering
    metadata_index: MetadataIndex,
    /// Async writeback queue
    writeback_queue: WritebackQueue,
    /// Configuration
    config: MemoryConfig,
}

pub struct MemoryConfig {
    pub hnsw_m: usize,
    pub hnsw_ef_construction: usize,
    pub default_ef_search: usize,
    pub max_graph_hops: usize,
    pub writeback_batch_size: usize,
    pub writeback_interval_ms: u64,
}

impl RuvectorMemory {
    /// Semantic search with graph expansion
    pub async fn search_with_graph(
        &self,
        query: &[f32],
        k: usize,
        ef_search: usize,
        expand_hops: usize,
    ) -> Result<SearchResult> {
        // HNSW search
        let candidates = self.vector_db.search(&SearchQuery {
            vector: query.to_vec(),
            k,
            filter: None,
            include_vectors: true,
        })?;

        // Expand to subgraph
        let subgraph = self.expand_neighborhood(
            &candidates.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
            expand_hops,
        )?;

        Ok(SearchResult {
            candidates,
            subgraph,
            stats: self.compute_stats(&candidates),
        })
    }

    /// Expand neighborhood via graph traversal
    fn expand_neighborhood(
        &self,
        node_ids: &[String],
        max_hops: usize,
    ) -> Result<SubGraph> {
        let mut visited = HashSet::new();
        let mut frontier: Vec<String> = node_ids.to_vec();
        let mut all_nodes = Vec::new();
        let mut all_edges = Vec::new();

        for _hop in 0..max_hops {
            let next_frontier = Vec::new();

            for node_id in &frontier {
                if visited.contains(node_id) {
                    continue;
                }
                visited.insert(node_id.clone());

                // Get node
                if let Some(node) = self.vector_db.get(node_id)? {
                    all_nodes.push(node);
                }

                // Get edges
                let edges = self.graph_store.get_edges(node_id)?;
                for edge in edges {
                    all_edges.push(edge.clone());
                    if !visited.contains(&edge.dst) {
                        next_frontier.push(edge.dst.clone());
                    }
                }
            }

            frontier = next_frontier;
        }

        Ok(SubGraph {
            nodes: all_nodes,
            edges: all_edges,
            center_ids: node_ids.to_vec(),
        })
    }

    /// Queue node for async writeback
    pub fn queue_writeback(&self, entry: WritebackEntry) {
        self.writeback_queue.push(entry);
    }
}
```

### 2.5 Graph Attention Engine

```rust
/// Graph attention for context extraction
pub struct GraphAttentionEngine {
    /// Multi-head attention layers
    attention_layers: Vec<GraphAttentionLayer>,
    /// Edge embedding lookup
    edge_embeddings: EdgeEmbeddings,
    /// Output projection
    output_projection: Linear,
    /// Configuration
    config: GraphAttentionConfig,
}

pub struct GraphAttentionLayer {
    /// Query projection per head
    w_q: Vec<Linear>,
    /// Key projection per head
    w_k: Vec<Linear>,
    /// Value projection per head
    w_v: Vec<Linear>,
    /// Edge attention bias
    edge_bias: Linear,
    /// Layer normalization
    layer_norm: LayerNorm,
}

pub struct GraphAttentionConfig {
    pub num_heads: usize,
    pub head_dim: usize,
    pub num_layers: usize,
    pub dropout: f32,
    pub distance_decay: f32,
    pub edge_dim: usize,
}

impl GraphAttentionEngine {
    pub fn attend(
        &self,
        query_embedding: &[f32],
        subgraph: &SubGraph,
    ) -> Result<GraphContext> {
        let mut current = query_embedding.to_vec();
        let node_embeddings: Vec<Vec<f32>> = subgraph.nodes
            .iter()
            .map(|n| n.vector.clone())
            .collect();

        let mut all_attention_weights = Vec::new();

        // Apply attention layers
        for layer in &self.attention_layers {
            let (output, weights) = layer.forward(
                &current,
                &node_embeddings,
                &subgraph.edges,
                &self.edge_embeddings,
                &self.config,
            )?;
            current = output;
            all_attention_weights.push(weights);
        }

        // Final projection
        let output = self.output_projection.forward(&current);

        // Aggregate attention weights across layers
        let avg_weights = aggregate_attention_weights(&all_attention_weights);

        // Rank nodes by attention
        let ranked_indices = argsort_descending(&avg_weights);

        Ok(GraphContext {
            embedding: output,
            ranked_nodes: ranked_indices.iter()
                .map(|&i| subgraph.nodes[i].clone())
                .collect(),
            attention_weights: ranked_indices.iter()
                .map(|&i| avg_weights[i])
                .collect(),
            summary: self.extract_summary(subgraph, &avg_weights),
        })
    }
}

impl GraphAttentionLayer {
    fn forward(
        &self,
        query: &[f32],
        node_embeddings: &[Vec<f32>],
        edges: &[Edge],
        edge_embed: &EdgeEmbeddings,
        config: &GraphAttentionConfig,
    ) -> Result<(Vec<f32>, Vec<f32>)> {
        let mut head_outputs = Vec::new();
        let mut all_weights = vec![0.0; node_embeddings.len()];

        for head_idx in 0..config.num_heads {
            // Project query
            let q = self.w_q[head_idx].forward(query);

            // Project keys and values
            let keys: Vec<Vec<f32>> = node_embeddings.iter()
                .map(|e| self.w_k[head_idx].forward(e))
                .collect();
            let values: Vec<Vec<f32>> = node_embeddings.iter()
                .map(|e| self.w_v[head_idx].forward(e))
                .collect();

            // Compute attention scores
            let mut scores = Vec::new();
            for (i, k) in keys.iter().enumerate() {
                let mut score = dot(&q, k) / (config.head_dim as f32).sqrt();

                // Add edge bias if edge exists
                if let Some(edge) = find_edge_to(edges, i) {
                    let edge_emb = edge_embed.get(edge.rel, edge.weight);
                    score += self.edge_bias.forward(&edge_emb)[0];
                }

                scores.push(score);
            }

            // Softmax
            let weights = softmax(&scores);

            // Accumulate weights
            for (i, w) in weights.iter().enumerate() {
                all_weights[i] += w / config.num_heads as f32;
            }

            // Weighted sum of values
            let head_output = weighted_sum(&values, &weights);
            head_outputs.push(head_output);
        }

        // Concatenate heads
        let concatenated = concat(&head_outputs);

        // Residual + LayerNorm
        let output = self.layer_norm.forward(&add(query, &concatenated));

        Ok((output, all_weights))
    }
}
```

### 2.6 LFM2 Inference Pool

```rust
/// Pool of LFM2 models with lazy loading and management
pub struct LFM2InferencePool {
    /// Model instances by size
    models: RwLock<HashMap<ModelSize, Arc<LFM2Model>>>,
    /// Model paths
    model_paths: HashMap<ModelSize, PathBuf>,
    /// Maximum concurrent models
    max_loaded: usize,
    /// LRU for model eviction
    lru: Mutex<LruCache<ModelSize, Instant>>,
    /// Device configuration
    device_config: DeviceConfig,
}

pub struct LFM2Model {
    /// Underlying model (llama.cpp or vLLM)
    inner: LFM2Backend,
    /// KV cache manager
    kv_cache: KVCacheManager,
    /// Model size
    size: ModelSize,
    /// Quantization
    quantization: Quantization,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelSize {
    M350,
    M700,
    B1_2,
    B2_6,
}

impl LFM2InferencePool {
    pub async fn generate(
        &self,
        model_size: ModelSize,
        prompt: &str,
        config: GenerationConfig,
        session_id: Option<&str>,
    ) -> Result<GenerationResult> {
        // Get or load model
        let model = self.get_or_load(model_size).await?;

        // Get KV cache for session
        let kv_cache = session_id
            .map(|id| model.kv_cache.get(id))
            .flatten();

        // Generate
        let (response, new_cache) = model.generate(prompt, config, kv_cache)?;

        // Update cache
        if let Some(id) = session_id {
            model.kv_cache.put(id, new_cache);
        }

        Ok(GenerationResult {
            text: response,
            tokens_generated: count_tokens(&response),
            model_used: model_size,
            cache_hit: kv_cache.is_some(),
        })
    }

    async fn get_or_load(&self, size: ModelSize) -> Result<Arc<LFM2Model>> {
        // Check if loaded
        {
            let models = self.models.read().await;
            if let Some(model) = models.get(&size) {
                // Update LRU
                self.lru.lock().await.put(size, Instant::now());
                return Ok(model.clone());
            }
        }

        // Load model
        let mut models = self.models.write().await;

        // Double-check
        if let Some(model) = models.get(&size) {
            return Ok(model.clone());
        }

        // Evict if necessary
        while models.len() >= self.max_loaded {
            let oldest = self.lru.lock().await.pop_lru();
            if let Some((evict_size, _)) = oldest {
                models.remove(&evict_size);
            }
        }

        // Load new model
        let model = self.load_model(size)?;
        let model = Arc::new(model);
        models.insert(size, model.clone());
        self.lru.lock().await.put(size, Instant::now());

        Ok(model)
    }

    fn load_model(&self, size: ModelSize) -> Result<LFM2Model> {
        let path = self.model_paths.get(&size)
            .ok_or_else(|| Error::ModelNotFound(size))?;

        let quantization = self.select_quantization(size);

        let inner = match &self.device_config.device_type {
            DeviceType::Cpu => {
                LFM2Backend::LlamaCpp(LlamaCppModel::load(path, &self.device_config)?)
            }
            DeviceType::Gpu => {
                LFM2Backend::VLLM(VLLMModel::load(path, &self.device_config)?)
            }
            DeviceType::Npu => {
                LFM2Backend::ExecuTorch(ExecuTorchModel::load(path)?)
            }
        };

        Ok(LFM2Model {
            inner,
            kv_cache: KVCacheManager::new(self.device_config.cache_size),
            size,
            quantization,
        })
    }
}
```

### 2.7 Self-Learning Service

```rust
/// Service managing continuous learning from interactions
pub struct SelfLearningService {
    /// Quality evaluation judge
    quality_judge: QualityJudge,
    /// Experience replay buffer
    replay_buffer: ReplayBuffer,
    /// EWC regularization state
    ewc: ElasticWeightConsolidation,
    /// Router optimizer
    optimizer: Adam,
    /// Router model reference
    router: Arc<RwLock<FastGRNNRouter>>,
    /// Training configuration
    config: LearningConfig,
    /// Background training handle
    training_handle: Option<JoinHandle<()>>,
}

pub struct LearningConfig {
    pub quality_threshold: f32,
    pub replay_capacity: usize,
    pub batch_size: usize,
    pub learning_rate: f32,
    pub ewc_lambda: f32,
    pub training_interval_ms: u64,
    pub min_samples_for_update: usize,
}

impl SelfLearningService {
    pub async fn on_interaction(
        &self,
        query: &str,
        response: &str,
        context: &[Document],
        routing_decision: &RoutingDecision,
        latency: Duration,
    ) -> Result<LearningOutcome> {
        // Evaluate quality
        let quality_score = self.quality_judge.evaluate(query, response, context).await?;

        // Create training sample
        let sample = TrainingSample {
            features: routing_decision.features.clone(),
            label_model: routing_decision.model as usize,
            label_context: routing_decision.context_bin(),
            label_temperature: routing_decision.temperature,
            label_top_p: routing_decision.top_p,
            quality: quality_score,
            latency_ms: latency.as_millis() as f32,
        };

        // Add to replay buffer
        self.replay_buffer.add(sample.clone());

        // Check for writeback
        let should_write = quality_score >= self.config.quality_threshold;

        Ok(LearningOutcome {
            quality_score,
            added_to_replay: true,
            should_writeback: should_write,
        })
    }

    pub fn start_background_training(&mut self) {
        let replay_buffer = self.replay_buffer.clone();
        let ewc = self.ewc.clone();
        let optimizer = self.optimizer.clone();
        let router = self.router.clone();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_millis(config.training_interval_ms)
            );

            loop {
                interval.tick().await;

                // Check if enough samples
                if replay_buffer.len() < config.min_samples_for_update {
                    continue;
                }

                // Sample batch
                let batch = replay_buffer.sample(config.batch_size);

                // Training step
                let mut router = router.write().await;
                let metrics = training_step(
                    &mut router,
                    &batch,
                    &ewc,
                    &optimizer,
                );

                tracing::info!(
                    "Training step: loss={:.4}, accuracy={:.2}%",
                    metrics.total_loss,
                    metrics.model_accuracy * 100.0
                );
            }
        });

        self.training_handle = Some(handle);
    }
}

pub struct QualityJudge {
    /// Judge model (typically 2.6B)
    model: Arc<LFM2Model>,
    /// Evaluation prompt template
    prompt_template: String,
}

impl QualityJudge {
    pub async fn evaluate(
        &self,
        query: &str,
        response: &str,
        context: &[Document],
    ) -> Result<f32> {
        let context_text = context.iter()
            .map(|d| d.text.as_str())
            .collect::<Vec<_>>()
            .join("\n---\n");

        let prompt = self.prompt_template
            .replace("{query}", query)
            .replace("{response}", response)
            .replace("{context}", &context_text);

        let result = self.model.generate(
            &prompt,
            GenerationConfig {
                max_tokens: 10,
                temperature: 0.0,  // Deterministic
                top_p: 1.0,
            },
            None,
        )?;

        // Parse score
        let score_str = result.text.trim();
        let score: i32 = score_str.parse().unwrap_or(3);
        let normalized = ((score.clamp(1, 5) - 1) as f32) / 4.0;

        Ok(normalized)
    }
}
```

---

## 3. Data Flow Architecture

### 3.1 Request Processing Pipeline

```
┌────────────────────────────────────────────────────────────────────────┐
│                        Request Processing Pipeline                      │
├────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  Stage 1: Input Processing                                              │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │  Request → Validate → Session Lookup → Rate Check → Cache Check  │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  Stage 2: Embedding & Retrieval                                        │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │  Tokenize → Embed (LFM2) → HNSW Search → Graph Expansion         │  │
│  │                                                                    │  │
│  │  Latency: ~50ms (embed) + ~10ms (search) + ~20ms (expand)        │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  Stage 3: Routing                                                       │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │  Extract Features → FastGRNN Forward → Decode Decision           │  │
│  │                                                                    │  │
│  │  Latency: ~2ms                                                    │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  Stage 4: Context Building                                             │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │  Graph Attention → Rank Nodes → Deduplicate → Truncate           │  │
│  │                                                                    │  │
│  │  Latency: ~30ms                                                   │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  Stage 5: Generation                                                    │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │  Format Prompt → Load Model → Prefill → Decode → Post-process    │  │
│  │                                                                    │  │
│  │  Latency: 100-500ms (varies by model)                            │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  Stage 6: Learning (Async)                                             │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │  Quality Judge → Replay Buffer → Conditional Writeback           │  │
│  │                                                                    │  │
│  │  Latency: ~100ms (async, non-blocking)                           │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                          │
└────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Memory Write Path

```
┌────────────────────────────────────────────────────────────────────────┐
│                          Memory Write Path                              │
├────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────┐                                                        │
│  │  Quality    │  score >= 0.75?                                       │
│  │  Evaluation │────────────────┐                                       │
│  └─────────────┘                │                                       │
│                                 ▼                                       │
│  ┌─────────────┐  ┌─────────────────────────────────────────────────┐  │
│  │   Skip      │◀─│  Deduplication Check (MinHash + HNSW threshold) │  │
│  │             │  └────────────────────────┬────────────────────────┘  │
│  └─────────────┘                           │                           │
│                                            │ unique?                   │
│                                            ▼                           │
│                              ┌─────────────────────────┐               │
│                              │   Create Node Entry     │               │
│                              │                         │               │
│                              │  - Generate UUID        │               │
│                              │  - Embed Q+A combined   │               │
│                              │  - Classify domain      │               │
│                              │  - Set metadata         │               │
│                              └────────────┬────────────┘               │
│                                           │                            │
│                                           ▼                            │
│                              ┌─────────────────────────┐               │
│                              │   Create Edge Links     │               │
│                              │                         │               │
│                              │  - Link to similar      │               │
│                              │    existing nodes       │               │
│                              │  - Set edge weights     │               │
│                              │    based on similarity  │               │
│                              └────────────┬────────────┘               │
│                                           │                            │
│                                           ▼                            │
│                              ┌─────────────────────────┐               │
│                              │   Writeback Queue       │               │
│                              │                         │               │
│                              │  - Batch writes         │               │
│                              │  - Background flush     │               │
│                              │  - HNSW index update    │               │
│                              └─────────────────────────┘               │
│                                                                          │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Deployment Architecture

### 4.1 Single-Node Deployment (Edge/Mobile)

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Single-Node Deployment (Edge)                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │                         RuvLLM Process                               ││
│  │                                                                       ││
│  │  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐        ││
│  │  │Orchestrator│  │ Embedder │  │  Router   │  │  Memory   │        ││
│  │  │           │  │  (ONNX)  │  │ (FastGRNN)│  │ (redb)    │        ││
│  │  └───────────┘  └───────────┘  └───────────┘  └───────────┘        ││
│  │                                                                       ││
│  │  ┌───────────────────────────────────────────────────────────────┐  ││
│  │  │              LFM2 Models (llama.cpp)                           │  ││
│  │  │                                                                 │  ││
│  │  │  ┌─────────┐  ┌─────────┐  (load on demand)                   │  ││
│  │  │  │  350M   │  │  700M   │                                      │  ││
│  │  │  │  Q4_K   │  │  Q4_K   │                                      │  ││
│  │  │  └─────────┘  └─────────┘                                      │  ││
│  │  └───────────────────────────────────────────────────────────────┘  ││
│  │                                                                       ││
│  │  Memory: 2-4GB | CPU: 4-8 cores | Storage: 4-8GB                    ││
│  └─────────────────────────────────────────────────────────────────────┘│
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Multi-Node Deployment (Server)

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Multi-Node Deployment (Server)                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  ┌─────────────────────────────────────┐                                │
│  │          Load Balancer              │                                │
│  │           (HAProxy)                 │                                │
│  └─────────────────┬───────────────────┘                                │
│                    │                                                     │
│         ┌──────────┼──────────┐                                         │
│         │          │          │                                         │
│         ▼          ▼          ▼                                         │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐                                   │
│  │Gateway 1│ │Gateway 2│ │Gateway 3│  (Orchestrator instances)         │
│  └────┬────┘ └────┬────┘ └────┬────┘                                   │
│       │           │           │                                         │
│       └───────────┼───────────┘                                         │
│                   │                                                      │
│       ┌───────────┴───────────┐                                         │
│       │                       │                                         │
│       ▼                       ▼                                         │
│  ┌─────────────────┐   ┌─────────────────┐                             │
│  │   Memory Tier   │   │  Inference Tier │                             │
│  │                 │   │                 │                             │
│  │ ┌─────────────┐ │   │ ┌─────────────┐ │                             │
│  │ │  Ruvector   │ │   │ │   vLLM      │ │                             │
│  │ │  Primary    │ │   │ │   Pool      │ │                             │
│  │ │  (redb)     │ │   │ │             │ │                             │
│  │ └─────────────┘ │   │ │ ┌───┐ ┌───┐ │ │                             │
│  │ ┌─────────────┐ │   │ │ │1.2│ │2.6│ │ │                             │
│  │ │  Replicas   │ │   │ │ │ B │ │ B │ │ │                             │
│  │ │  (read)     │ │   │ │ └───┘ └───┘ │ │                             │
│  │ └─────────────┘ │   │ └─────────────┘ │                             │
│  └─────────────────┘   └─────────────────┘                             │
│                                                                           │
│  Coordination: Redis (pub/sub) | PostgreSQL (metadata)                  │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

### 4.3 Hybrid Cloud-Edge Deployment

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      Hybrid Cloud-Edge Deployment                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │                            Cloud Tier                                ││
│  │                                                                       ││
│  │  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐        ││
│  │  │  vLLM Cluster  │  │  Central DB    │  │  Training      │        ││
│  │  │  (2.6B models) │  │  (PostgreSQL)  │  │  Service       │        ││
│  │  │                │  │                │  │                │        ││
│  │  │  Escalation    │  │  Aggregated    │  │  Federated     │        ││
│  │  │  endpoint      │  │  knowledge     │  │  learning      │        ││
│  │  └────────────────┘  └────────────────┘  └────────────────┘        ││
│  └─────────────────────────────────────────────────────────────────────┘│
│                                    ▲                                     │
│                                    │ Sync                                │
│                                    ▼                                     │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │                            Edge Tier                                 ││
│  │                                                                       ││
│  │  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐           ││
│  │  │  Edge Node 1  │  │  Edge Node 2  │  │  Edge Node N  │           ││
│  │  │               │  │               │  │               │           ││
│  │  │  350M-700M    │  │  350M-700M    │  │  350M-700M    │           ││
│  │  │  Local redb   │  │  Local redb   │  │  Local redb   │           ││
│  │  │               │  │               │  │               │           ││
│  │  │  Offline      │  │  Offline      │  │  Offline      │           ││
│  │  │  capable      │  │  capable      │  │  capable      │           ││
│  │  └───────────────┘  └───────────────┘  └───────────────┘           ││
│  └─────────────────────────────────────────────────────────────────────┘│
│                                                                           │
│  Sync Protocol:                                                          │
│  - Edge → Cloud: High-quality interactions, router telemetry            │
│  - Cloud → Edge: Updated router weights, knowledge deltas               │
│  - Interval: Configurable (hourly/daily/weekly)                         │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Integration with Existing Ruvector Crates

### 5.1 Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        RuvLLM Dependency Graph                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│                           ┌───────────────┐                              │
│                           │   ruvector-   │                              │
│                           │     llm       │                              │
│                           │   (NEW)       │                              │
│                           └───────┬───────┘                              │
│                                   │                                      │
│           ┌───────────────────────┼───────────────────────┐             │
│           │                       │                       │             │
│           ▼                       ▼                       ▼             │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐     │
│  │  ruvector-core  │    │ ruvector-graph  │    │ruvector-attention│    │
│  │                 │    │                 │    │                 │     │
│  │  - VectorDB     │    │  - GraphStore   │    │  - MultiHead    │     │
│  │  - HNSW         │    │  - Edges/Nodes  │    │  - GraphAttention│    │
│  │  - Distance     │    │  - Traversal    │    │  - Edge features │     │
│  └────────┬────────┘    └────────┬────────┘    └────────┬────────┘     │
│           │                      │                      │              │
│           │                      ▼                      │              │
│           │            ┌─────────────────┐              │              │
│           │            │  ruvector-gnn   │◀─────────────┘              │
│           │            │                 │                             │
│           │            │  - GNN Layers   │                             │
│           │            │  - EWC          │                             │
│           │            │  - Replay       │                             │
│           │            │  - Optimizer    │                             │
│           │            └────────┬────────┘                             │
│           │                     │                                      │
│           └─────────────────────┼─────────────────────────────────────│
│                                 │                                      │
│                                 ▼                                      │
│                       ┌─────────────────┐                              │
│                       │ ruvector-router │                              │
│                       │     -core       │                              │
│                       │                 │                              │
│                       │  - Quantization │                              │
│                       │  - Storage      │                              │
│                       │  - Index        │                              │
│                       └─────────────────┘                              │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

### 5.2 API Integration Points

```rust
// Integration with ruvector-core
use ruvector_core::{VectorDB, VectorEntry, SearchQuery, DbOptions};

// Integration with ruvector-gnn
use ruvector_gnn::{
    ElasticWeightConsolidation,
    ReplayBuffer,
    Optimizer, OptimizerType,
    LearningRateScheduler, SchedulerType,
};

// Integration with ruvector-attention
use ruvector_attention::{
    MultiHeadAttention,
    GraphAttention, GraphAttentionConfig,
    EdgeFeaturedAttention,
    Adam, AdamW,
    InfoNCELoss,
};

// Integration with ruvector-graph
use ruvector_graph::{GraphStore, Node, Edge, EdgeType};

// New types for RuvLLM
pub struct RuvLLMConfig {
    /// Core database options
    pub db_options: DbOptions,
    /// Graph attention configuration
    pub attention_config: GraphAttentionConfig,
    /// Router configuration
    pub router_config: FastGRNNConfig,
    /// Learning configuration
    pub learning_config: LearningConfig,
    /// LFM2 model paths
    pub model_paths: HashMap<ModelSize, PathBuf>,
}
```

---

## 6. Security Architecture

### 6.1 Data Protection

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          Security Architecture                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  Input Validation                                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │  - Query sanitization (XSS, injection prevention)                   ││
│  │  - Token limit enforcement                                           ││
│  │  - Content policy filtering (optional)                               ││
│  └─────────────────────────────────────────────────────────────────────┘│
│                                                                           │
│  Memory Isolation                                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │  - Per-tenant namespace isolation (multi-tenant mode)               ││
│  │  - PII detection and masking before storage                         ││
│  │  - Encryption at rest (AES-256)                                      ││
│  │  - Encryption in transit (TLS 1.3)                                   ││
│  └─────────────────────────────────────────────────────────────────────┘│
│                                                                           │
│  Model Security                                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │  - Model integrity verification (SHA256 checksums)                  ││
│  │  - Sandboxed inference (seccomp, AppArmor)                          ││
│  │  - Output filtering for harmful content                              ││
│  └─────────────────────────────────────────────────────────────────────┘│
│                                                                           │
│  Audit Trail                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │  - Request/response logging (configurable retention)                ││
│  │  - Router decision audit                                             ││
│  │  - Writeback provenance tracking                                     ││
│  └─────────────────────────────────────────────────────────────────────┘│
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Monitoring and Observability

### 7.1 Metrics Architecture

```rust
pub struct MetricsExporter {
    /// Prometheus registry
    registry: prometheus::Registry,
    /// Latency histograms
    latency_histograms: LatencyMetrics,
    /// Counter metrics
    counters: CounterMetrics,
    /// Gauge metrics
    gauges: GaugeMetrics,
}

pub struct LatencyMetrics {
    pub total_latency: Histogram,
    pub embedding_latency: Histogram,
    pub retrieval_latency: Histogram,
    pub routing_latency: Histogram,
    pub generation_latency: Histogram,
    pub quality_eval_latency: Histogram,
}

pub struct CounterMetrics {
    pub requests_total: IntCounterVec,      // by status
    pub cache_hits: IntCounter,
    pub cache_misses: IntCounter,
    pub writebacks_total: IntCounterVec,    // by outcome
    pub model_selections: IntCounterVec,    // by model size
}

pub struct GaugeMetrics {
    pub active_requests: IntGauge,
    pub memory_usage_bytes: IntGauge,
    pub models_loaded: IntGauge,
    pub replay_buffer_size: IntGauge,
    pub avg_quality_score: Gauge,
}
```

### 7.2 Distributed Tracing

```
Request Trace Example:
─────────────────────────────────────────────────────────────────────────

Trace ID: abc123
Span: orchestrator.process [450ms]
├── Span: rate_limiter.check [1ms]
├── Span: cache.lookup [2ms] → miss
├── Span: embedder.embed [52ms]
│   └── Span: lfm2_encoder.forward [48ms]
├── Span: memory.search [28ms]
│   ├── Span: hnsw.search [12ms]
│   └── Span: graph.expand [16ms]
├── Span: router.forward [3ms]
├── Span: graph_attention.attend [35ms]
│   └── Span: attention_layer.forward [32ms] x3
├── Span: context.build [8ms]
├── Span: lfm2.generate [298ms]
│   ├── Span: model.load [0ms] → cached
│   ├── Span: model.prefill [85ms]
│   └── Span: model.decode [213ms]
└── Span: learning.on_interaction [async]
    ├── Span: quality_judge.evaluate [95ms]
    └── Span: replay_buffer.add [1ms]
```

---

*Document Version: 1.0*
*Last Updated: 2025-12-02*
*Author: RuvLLM Architecture Team*
