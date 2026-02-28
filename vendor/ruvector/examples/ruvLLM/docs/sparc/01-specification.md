# RuvLLM: Self-Learning LLM with LFM2 and Ruvector Integration

## SPARC Phase 1: Specification

---

## 1. Executive Summary

RuvLLM is a self-learning LLM architecture that integrates **Liquid Foundation Models (LFM2)** with **ruvector** as the world model and memory substrate. The system uses **FastGRNN** as an intelligent router to dynamically allocate computational resources based on query complexity, enabling efficient on-device inference with continuous learning capabilities.

### Core Innovation

The architecture treats:
- **LFM2** as the reasoning head (inference engine)
- **Ruvector** as the world model and episodic memory
- **FastGRNN** as the control circuit (routing decisions)

This triad creates a self-learning system where:
1. Queries are semantically embedded and matched against memory
2. Graph attention extracts relevant neighborhood context
3. FastGRNN routes to optimal model configuration
4. LFM2 generates responses with retrieved context
5. Successful interactions are written back to memory (self-improvement)

---

## 2. Technical Requirements

### 2.1 Functional Requirements

#### FR-001: LFM2 Model Integration
- **Description**: Support LFM2 model family (350M, 700M, 1.2B, 2.6B parameters)
- **Acceptance Criteria**:
  - Load models via llama.cpp (CPU) or vLLM (server)
  - Support quantization: Q4/Q5 (CPU), 8-bit/4-bit weight-only (GPU)
  - Enable KV cache for context reuse
  - Achieve <500ms median latency (CPU), <100ms (GPU)

#### FR-002: Ruvector Memory Service
- **Description**: Implement semantic memory with graph structure
- **Storage Schema**:
  ```
  Nodes: {
    id: UUID,
    vector: [f32; D],      // D = embedding dimension
    text: String,
    type: NodeType,        // Query | Document | AgentStep | Fact
    source: String,
    metadata: {
      timestamp: i64,
      tags: Vec<String>,
      domain: String,
      version: u32,
      confidence: f32
    }
  }

  Edges: {
    id: UUID,
    src: UUID,
    dst: UUID,
    rel: EdgeType,         // Cites | Follows | SameTopic | AgentStep | Derived
    weight: f32,
    metadata: {
      timestamp: i64,
      created_by: String,
      confidence: f32
    }
  }
  ```
- **Acceptance Criteria**:
  - HNSW index with M=32, efConstruction=200, efSearch=64
  - Sub-millisecond retrieval for k≤64
  - Graph attention over 2-hop neighborhoods
  - Support billion-scale corpora

#### FR-003: FastGRNN Router
- **Description**: Implement gated recurrent router for intelligent resource allocation
- **Architecture** (per Kusupati et al.):
  - Hidden size: 32-64 units
  - Input: Fixed-length feature vector (~128 dims)
  - Outputs: model_selection, context_size, temperature, top_p
- **Feature Vector Components** (128 dimensions):
  ```
  Query Stats [32 dims]:
    - token_count: f32
    - language_id: [f32; 8] (one-hot)
    - domain_encoding: [f32; 16]
    - user_frequency: f32
    - query_type: [f32; 6] (factual/reasoning/creative/...)

  Embedding Stats [16 dims]:
    - l2_norm: f32
    - principal_components: [f32; 8]
    - entropy: f32
    - sparsity: f32
    - cluster_assignment: [f32; 4]

  HNSW Search Stats [48 dims]:
    - k_retrieved: f32
    - distances: { mean, std, min, max }: [f32; 4]
    - entropy: f32
    - graph_depth: f32
    - recall_estimate: f32
    - neighborhood_density: [f32; 16]
    - semantic_coherence: [f32; 24]

  System Constraints [32 dims]:
    - latency_budget: f32
    - device_class: [f32; 4] (edge/mobile/server/cluster)
    - privacy_level: [f32; 4]
    - memory_available: f32
    - battery_level: f32 (for mobile)
    - concurrent_requests: f32
    - historical_accuracy: [f32; 16]
  ```

#### FR-004: Self-Learning Pipeline
- **Description**: Implement continuous learning with forgetting mitigation
- **Components**:
  - Online learning from successful interactions
  - Elastic Weight Consolidation (EWC) for catastrophic forgetting prevention
  - Experience replay with reservoir sampling
  - Curriculum learning for progressive complexity
- **Acceptance Criteria**:
  - Quality regret <0.1 points vs. always-big baseline
  - No measurable forgetting over 10K update cycles
  - Router accuracy >95% for seen patterns

#### FR-005: Graph Attention Engine
- **Description**: Context extraction via graph-aware attention
- **Mechanism**:
  - Multi-head attention over retrieved nodes
  - Edge-weighted aggregation (confidence, recency)
  - Hyperbolic embeddings for hierarchical relationships
  - 2-hop neighborhood expansion
- **Integration with existing ruvector-attention**:
  - Leverage `EdgeFeaturedAttention` for edge attributes
  - Use `GraphRoPE` for positional encoding on graphs
  - Apply `DualSpaceAttention` for multi-manifold reasoning

### 2.2 Non-Functional Requirements

#### NFR-001: Performance
| Metric | Tier A (Server) | Tier B (Edge) | Tier C (Mobile) |
|--------|-----------------|---------------|-----------------|
| P50 Latency | <200ms | <500ms | <800ms |
| P99 Latency | <1s | <2s | <5s |
| Throughput | 100 QPS | 20 QPS | 5 QPS |
| Memory | <16GB | <4GB | <1GB |

#### NFR-002: Quality
- **Accuracy**: F1 >0.85 on QA benchmarks
- **Retrieval**: R@10 >0.90 for relevant documents
- **Router**: Decision accuracy >95%
- **Judge Rating**: 4.2+/5.0 on LLM-as-judge evaluations

#### NFR-003: Scalability
- Support 10M+ vectors in memory
- Support 1B+ vectors with hybrid indexing
- Linear scaling with node count in cluster mode

#### NFR-004: Reliability
- Zero data loss on graceful shutdown
- Recovery from OOM within 30s
- Automatic failover in cluster mode

---

## 3. LFM2 Deep Dive

### 3.1 Architecture Analysis

LFM2 employs a **hybrid backbone** combining:

1. **Gated Short Convolutions**: Lightweight local feature processing
   - O(n) complexity vs O(n²) for attention
   - Captures local patterns efficiently
   - Enables 2x faster prefill on CPUs

2. **Grouped Query Attention (GQA)**: Reduced KV heads
   - 4-8 KV heads vs 32+ in standard attention
   - Maintains quality with 4x memory reduction
   - Critical for edge deployment

### 3.2 Training Methodology

LFM2's training is relevant for our self-learning pipeline:

1. **Knowledge Distillation**: Tempered, decoupled Top-K
   - Teacher: Large model (70B+)
   - Student: LFM2 variants
   - **Insight**: We can distill router decisions from expensive oracle

2. **Curriculum Learning**: Progressive complexity
   - Start with simple factual queries
   - Graduate to multi-step reasoning
   - **Application**: Router training follows same progression

3. **Three-Stage Post-Training**:
   - SFT: Supervised fine-tuning on quality data
   - DPO: Direct preference optimization
   - Model merging: Combine specialists
   - **Application**: We merge domain-specific adapters

### 3.3 Multimodal Extensions (Future)

- **LFM2-VL**: Vision-language (image understanding)
- **LFM2-Audio**: Speech I/O
- **LFM2-ColBERT**: Low-latency retrieval encoder

---

## 4. Ruvector Integration Analysis

### 4.1 Existing Capabilities

| Component | Status | Integration Plan |
|-----------|--------|------------------|
| ruvector-core | ✅ Production | Primary vector store |
| ruvector-gnn | ✅ Production | Graph neural layer |
| ruvector-attention | ✅ Production | Attention mechanisms |
| ruvector-router-core | ✅ Production | Base routing |
| ruvector-graph | ✅ Production | Knowledge graph |

### 4.2 Required Extensions

#### 4.2.1 Embedding Adapter
```rust
pub struct EmbeddingAdapter {
    /// LFM2 encoder for query embedding
    lfm2_encoder: Lfm2Encoder,
    /// Dimension alignment layer
    projection: Linear,
    /// Normalization
    layer_norm: LayerNorm,
}

impl EmbeddingAdapter {
    pub fn embed(&self, text: &str) -> Vec<f32> {
        let raw = self.lfm2_encoder.encode(text);
        let projected = self.projection.forward(&raw);
        self.layer_norm.forward(&projected)
    }
}
```

#### 4.2.2 Memory Writeback Service
```rust
pub struct MemoryWriteback {
    /// Quality threshold for writeback
    quality_threshold: f32,
    /// Deduplication via MinHash
    dedup_hasher: MinHasher,
    /// Conflict resolution
    merger: ConflictMerger,
}

impl MemoryWriteback {
    pub async fn maybe_write(
        &self,
        query: &str,
        response: &str,
        quality_score: f32,
        db: &VectorDB,
    ) -> Result<Option<UUID>> {
        if quality_score < self.quality_threshold {
            return Ok(None);
        }

        // Check for near-duplicates
        let embedding = embed(query, response);
        let similar = db.search_threshold(&embedding, 0.95)?;
        if !similar.is_empty() {
            return self.merger.resolve(similar, query, response);
        }

        // Insert new memory
        let entry = VectorEntry::new(embedding)
            .with_text(format!("Q: {}\nA: {}", query, response))
            .with_metadata(json!({
                "type": "qa_pair",
                "quality": quality_score,
                "timestamp": now(),
            }));

        Ok(Some(db.insert(entry)?))
    }
}
```

### 4.3 HNSW Parameter Tuning

Based on arxiv:2511.23404v1 insights on retrieval efficiency:

| Corpus Size | M | efConstruction | efSearch | Recall@10 |
|-------------|---|----------------|----------|-----------|
| <100K | 16 | 100 | 32 | 0.98 |
| 100K-1M | 32 | 200 | 64 | 0.96 |
| 1M-10M | 48 | 300 | 128 | 0.94 |
| 10M-100M | 64 | 400 | 256 | 0.92 |
| >100M | Hybrid | Tiered | Adaptive | 0.90 |

---

## 5. FastGRNN Router Specification

### 5.1 Mathematical Formulation

FastGRNN (Fast, Accurate, Stable, and Tiny GRU):

```
z_t = σ(W_z · x_t + U_z · h_{t-1} + b_z)
h̃_t = tanh(W_h · x_t + U_h · (r_t ⊙ h_{t-1}) + b_h)
h_t = (ζ · (1 - z_t) + ν) ⊙ h̃_t + z_t ⊙ h_{t-1}

where:
  - ζ, ν: Learned scalars (typically ζ≈1, ν≈0.5)
  - W_z, W_h: Input weight matrices (sparse)
  - U_z, U_h: Recurrent weight matrices (low-rank)
  - r_t: Optional reset gate (can be fixed to 1)
```

### 5.2 Output Heads

```rust
pub struct RouterOutputs {
    /// Model selection: [350M, 700M, 1.2B, 2.6B] probabilities
    pub model_probs: [f32; 4],
    /// Context size bins: [256, 512, 1024, 2048, 4096] tokens
    pub context_probs: [f32; 5],
    /// Temperature: continuous [0.0, 2.0]
    pub temperature: f32,
    /// Top-p: continuous [0.0, 1.0]
    pub top_p: f32,
    /// Confidence score
    pub confidence: f32,
}
```

### 5.3 Training Protocol

**Phase 1: Data Collection**
```
For each query q:
  1. Run all model configurations (expensive baseline)
  2. Collect quality metrics Q, latency L, cost C
  3. Compute utility: U = Q - λ·L - μ·C
  4. Label: y_model = argmax(U), y_ctx = min viable context
```

**Phase 2: Supervised Training**
```
Loss = CE(model_pred, y_model)
     + CE(ctx_pred, y_ctx)
     + α·SmoothL1(temp_pred, y_temp)
     + β·SmoothL1(top_p_pred, y_top_p)
```

**Phase 3: Online Refinement**
```
Every N requests:
  1. Sample exploration (ε-greedy or Thompson)
  2. Compute regret vs. oracle
  3. Update weights with importance sampling
  4. Apply EWC regularization
```

---

## 6. Self-Learning Mechanisms

### 6.1 Continual Learning Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Self-Learning Pipeline                     │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐   │
│  │ Query   │───▶│ Retrieve│───▶│ Generate│───▶│ Evaluate│   │
│  └─────────┘    └─────────┘    └─────────┘    └─────────┘   │
│       │              │              │              │         │
│       │              │              │              ▼         │
│       │              │              │        ┌─────────┐     │
│       │              │              │        │ Quality │     │
│       │              │              │        │ > θ ?   │     │
│       │              │              │        └────┬────┘     │
│       │              │              │             │          │
│       │              │              │      ┌──────┴──────┐   │
│       │              │              │      ▼             ▼   │
│       │              │              │  ┌───────┐   ┌───────┐ │
│       │              │              │  │ Write │   │ Skip  │ │
│       │              │              │  │ Back  │   │       │ │
│       │              │              │  └───┬───┘   └───────┘ │
│       │              │              │      │                 │
│       ▼              ▼              ▼      ▼                 │
│  ┌─────────────────────────────────────────────┐             │
│  │            Replay Buffer (Reservoir)         │             │
│  │  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐   │             │
│  │  │ E_1 │ │ E_2 │ │ ... │ │E_n-1│ │ E_n │   │             │
│  │  └─────┘ └─────┘ └─────┘ └─────┘ └─────┘   │             │
│  └──────────────────────┬──────────────────────┘             │
│                         │                                    │
│                         ▼                                    │
│  ┌─────────────────────────────────────────────┐             │
│  │           EWC Regularization Layer           │             │
│  │                                               │             │
│  │  L_total = L_task + λ·Σ F_i·(θ_i - θ*_i)²   │             │
│  │                                               │             │
│  │  F_i = Fisher Information (importance)        │             │
│  │  θ*_i = Optimal weights from previous task   │             │
│  └─────────────────────────────────────────────┘             │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 Quality Evaluation

**LLM-as-Judge Protocol**:
```rust
pub struct QualityJudge {
    judge_model: Lfm2, // Use 2.6B for judging
    rubric: JudgeRubric,
}

impl QualityJudge {
    pub fn evaluate(&self, query: &str, response: &str, context: &[&str]) -> f32 {
        let prompt = format!(r#"
            Evaluate the response quality on a scale of 1-5:

            Query: {query}
            Retrieved Context: {context:?}
            Response: {response}

            Criteria:
            1. Factual accuracy (grounded in context)
            2. Completeness (addresses the query fully)
            3. Coherence (logical flow)
            4. Conciseness (no unnecessary verbosity)

            Score (1-5):
        "#);

        let score_str = self.judge_model.generate(&prompt, 10);
        parse_score(&score_str)
    }
}
```

### 6.3 Forgetting Mitigation

**Elastic Weight Consolidation (EWC)**:

```rust
// From ruvector-gnn ewc module
pub struct ElasticWeightConsolidation {
    lambda: f32,                    // Regularization strength
    fisher_info: Vec<f32>,          // Fisher information diagonal
    optimal_weights: Vec<f32>,      // θ* from previous task
}

impl ElasticWeightConsolidation {
    pub fn regularization_loss(&self, current_weights: &[f32]) -> f32 {
        self.fisher_info.iter()
            .zip(current_weights.iter())
            .zip(self.optimal_weights.iter())
            .map(|((f, w), w_star)| f * (w - w_star).powi(2))
            .sum::<f32>() * self.lambda / 2.0
    }

    pub fn update_fisher(&mut self, gradients: &[Vec<f32>]) {
        // Fisher = E[∇logP(y|x;θ)²]
        for (i, grad_samples) in gradients.iter().enumerate() {
            self.fisher_info[i] = grad_samples.iter()
                .map(|g| g.powi(2))
                .sum::<f32>() / grad_samples.len() as f32;
        }
    }
}
```

---

## 7. Performance Optimization Strategy

### 7.1 LFM2 Level

| Optimization | Speedup | Quality Impact | Implementation |
|--------------|---------|----------------|----------------|
| Model selection | 2-4x | <1% | FastGRNN router |
| KV cache reuse | 1.5-2x | 0% | llama.cpp native |
| Q4 quantization | 2-3x | <2% | GGUF format |
| Speculative decode | 1.3-1.5x | 0% | Draft model |
| Continuous batching | 2-4x | 0% | vLLM |

### 7.2 Ruvector Level

| Optimization | Speedup | Quality Impact | Implementation |
|--------------|---------|----------------|----------------|
| HNSW tuning | Variable | Recall tradeoff | efSearch adjustment |
| Product quantization | 4-8x memory | <5% | PQ in ruvector-core |
| Graph pruning | 1.2-1.5x | <1% | Edge weight threshold |
| Batch retrieval | 2-3x | 0% | Parallel HNSW |
| Caching | 10x+ (hits) | 0% | LRU with TTL |

### 7.3 Router Level

| Optimization | Speedup | Quality Impact | Implementation |
|--------------|---------|----------------|----------------|
| Sparse weights | 10-50x | <0.5% | Magnitude pruning |
| Low-rank U | 2-4x | <0.5% | SVD decomposition |
| Int8 quantization | 2-4x | <0.1% | Post-training quant |
| Cascade routing | 1.5-2x | 0% | Early exit |

---

## 8. Success Metrics

### 8.1 Primary Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| End-to-end latency P50 | <500ms | Timer instrumentation |
| Quality (LLM judge) | 4.2+/5.0 | Automated evaluation |
| Router accuracy | >95% | Oracle comparison |
| Memory efficiency | <4GB (edge) | RSS monitoring |
| Throughput | 20 QPS (edge) | Load testing |

### 8.2 Secondary Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Retrieval R@10 | >0.90 | Benchmark suite |
| Forgetting rate | <5%/10K updates | Periodic eval |
| Cost reduction | >50% vs baseline | Token counting |
| Writeback rate | 10-30% | Database metrics |

### 8.3 Regret Analysis

```
Quality Regret = E[Q_baseline - Q_routed]
Latency Regret = E[L_routed - L_oracle]
Cost Regret = E[C_routed - C_oracle]

Targets:
- Quality Regret < 0.1 points (1-5 scale)
- Latency Regret < 50ms
- Cost Regret < 10%
```

---

## 9. Risk Analysis

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Router misprediction | Medium | High | Confidence thresholds, fallback |
| Catastrophic forgetting | Low | Critical | EWC, replay buffer, checkpoints |
| Memory exhaustion | Medium | High | Streaming, tiered storage |
| Quality degradation | Medium | High | A/B testing, rollback |
| Latency spikes | High | Medium | Caching, async processing |

---

## 10. Dependencies

### 10.1 Internal Dependencies

```toml
[dependencies]
ruvector-core = { path = "../ruvector-core" }
ruvector-gnn = { path = "../ruvector-gnn" }
ruvector-attention = { path = "../ruvector-attention" }
ruvector-graph = { path = "../ruvector-graph" }
ruvector-router-core = { path = "../ruvector-router-core" }
```

### 10.2 External Dependencies

```toml
[dependencies]
# LLM runtime
llama-cpp-rs = "0.3"        # CPU inference
tokenizers = "0.15"         # Fast tokenization

# Async runtime
tokio = { version = "1.41", features = ["full"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }

# Metrics
prometheus = "0.13"
tracing = "0.1"
```

---

## 11. References

1. **LFM2 Technical Report**: arxiv:2511.23404v1
2. **FastGRNN**: Kusupati et al., "FastGRNN: A Fast, Accurate, Stable and Tiny Kilobyte Sized Gated Recurrent Neural Network"
3. **EWC**: Kirkpatrick et al., "Overcoming catastrophic forgetting in neural networks"
4. **HNSW**: Malkov & Yashunin, "Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs"
5. **Graph Attention**: Veličković et al., "Graph Attention Networks"

---

*Document Version: 1.0*
*Last Updated: 2025-12-02*
*Author: RuvLLM Architecture Team*
