# RuvLLM: Algorithm Design

## SPARC Phase 2: Pseudocode

---

## 1. Core Request Flow

### 1.1 Main Orchestrator

```pseudocode
ALGORITHM ProcessQuery(query: String, session: Session) -> Response:
    INPUT:
        query: User query string
        session: Session containing user context, history, constraints
    OUTPUT:
        response: Generated response with metadata

    // Step 1: Preprocessing and Embedding
    tokens ← Tokenize(query)
    query_embedding ← EmbedQuery(query)
    query_features ← ExtractQueryFeatures(tokens, query_embedding)

    // Step 2: Memory Retrieval via HNSW
    candidates ← HNSWSearch(
        vector: query_embedding,
        k: 64,
        ef_search: GetAdaptiveEfSearch(session.latency_budget)
    )

    // Step 3: Graph Attention over Neighborhood
    graph_context ← GraphAttention(
        center_node: query_embedding,
        neighbors: candidates,
        hops: 2,
        attention_heads: 4
    )

    // Step 4: Feature Extraction for Router
    router_features ← BuildRouterFeatures(
        query_features,
        candidates.statistics(),
        graph_context.summary(),
        session.constraints
    )

    // Step 5: FastGRNN Routing Decision
    routing_decision ← FastGRNNRoute(router_features, session.hidden_state)
    session.hidden_state ← routing_decision.new_hidden

    // Step 6: Context Construction
    context ← BuildContext(
        graph_context.ranked_nodes,
        max_tokens: routing_decision.context_size,
        dedup: TRUE
    )

    // Step 7: LFM2 Generation
    response ← LFM2Generate(
        model: routing_decision.model_selection,
        prompt: FormatPrompt(query, context),
        temperature: routing_decision.temperature,
        top_p: routing_decision.top_p,
        max_tokens: GetMaxTokens(routing_decision.model_selection)
    )

    // Step 8: Quality Evaluation
    quality_score ← EvaluateQuality(query, response, context)

    // Step 9: Optional Writeback
    IF quality_score > QUALITY_THRESHOLD:
        MemoryWriteback(query, response, quality_score)

    // Step 10: Telemetry
    LogTelemetry(
        routing_decision,
        candidates.stats,
        latency_breakdown,
        quality_score
    )

    RETURN Response {
        text: response,
        confidence: quality_score,
        sources: context.sources,
        routing_info: routing_decision
    }
```

### 1.2 Adaptive efSearch Selection

```pseudocode
ALGORITHM GetAdaptiveEfSearch(latency_budget_ms: f32) -> u32:
    // Dynamic HNSW parameter based on latency constraints

    IF latency_budget_ms < 100:
        RETURN 32    // Fast mode, lower recall
    ELSE IF latency_budget_ms < 300:
        RETURN 64    // Balanced mode
    ELSE IF latency_budget_ms < 500:
        RETURN 128   // High recall mode
    ELSE:
        RETURN 256   // Maximum recall mode
```

---

## 2. FastGRNN Router

### 2.1 Core FastGRNN Cell

```pseudocode
ALGORITHM FastGRNNCell(x: Vector, h: Vector, params: FastGRNNParams) -> Vector:
    INPUT:
        x: Input feature vector [input_dim]
        h: Hidden state [hidden_dim]
        params: {W_z, U_z, b_z, W_h, U_h, b_h, zeta, nu}
    OUTPUT:
        h_new: Updated hidden state [hidden_dim]

    // Update gate
    z_pre ← MatMul(params.W_z, x) + MatMul(params.U_z, h) + params.b_z
    z ← Sigmoid(z_pre)

    // Candidate hidden state
    h_tilde_pre ← MatMul(params.W_h, x) + MatMul(params.U_h, h) + params.b_h
    h_tilde ← Tanh(h_tilde_pre)

    // FastGRNN update with learned scalars
    h_new ← (params.zeta * (1 - z) + params.nu) ⊙ h_tilde + z ⊙ h

    RETURN h_new
```

### 2.2 Router Forward Pass

```pseudocode
ALGORITHM FastGRNNRoute(features: Vector, hidden: Vector) -> RoutingDecision:
    INPUT:
        features: Router input features [128]
        hidden: Previous hidden state [64]
    OUTPUT:
        decision: RoutingDecision with model, context, temperature, top_p

    // Normalize input
    features_norm ← LayerNorm(features)

    // FastGRNN cell update
    h_new ← FastGRNNCell(features_norm, hidden, ROUTER_PARAMS)

    // Output heads
    model_logits ← Linear(h_new, W_model)           // [4] for 4 model sizes
    context_logits ← Linear(h_new, W_context)       // [5] for context bins
    temp_raw ← Linear(h_new, W_temp)                // [1] scalar
    top_p_raw ← Linear(h_new, W_top_p)              // [1] scalar
    confidence_raw ← Linear(h_new, W_confidence)    // [1] scalar

    // Activations
    model_probs ← Softmax(model_logits)
    context_probs ← Softmax(context_logits)
    temperature ← Sigmoid(temp_raw) * 2.0           // Scale to [0, 2]
    top_p ← Sigmoid(top_p_raw)                      // Scale to [0, 1]
    confidence ← Sigmoid(confidence_raw)

    // Decoding with confidence threshold
    IF confidence < CONFIDENCE_THRESHOLD:
        // Fall back to safe defaults
        model_idx ← 2  // 1.2B model
        context_idx ← 3  // 2048 tokens
    ELSE:
        model_idx ← ArgMax(model_probs)
        context_idx ← ArgMax(context_probs)

    RETURN RoutingDecision {
        model_selection: MODEL_SIZES[model_idx],
        context_size: CONTEXT_BINS[context_idx],
        temperature: temperature,
        top_p: top_p,
        confidence: confidence,
        new_hidden: h_new
    }

CONSTANTS:
    MODEL_SIZES = [350M, 700M, 1.2B, 2.6B]
    CONTEXT_BINS = [256, 512, 1024, 2048, 4096]
    CONFIDENCE_THRESHOLD = 0.7
```

### 2.3 Feature Extraction

```pseudocode
ALGORITHM BuildRouterFeatures(
    query_features: QueryFeatures,
    search_stats: SearchStatistics,
    graph_summary: GraphSummary,
    constraints: SystemConstraints
) -> Vector:
    OUTPUT: features [128]

    features ← EmptyVector(128)
    offset ← 0

    // Query features [32 dims]
    features[offset:offset+1] ← Normalize(query_features.token_count, 0, 512)
    offset += 1
    features[offset:offset+8] ← query_features.language_one_hot
    offset += 8
    features[offset:offset+16] ← query_features.domain_embedding
    offset += 16
    features[offset:offset+1] ← Normalize(query_features.user_frequency, 0, 1000)
    offset += 1
    features[offset:offset+6] ← query_features.query_type_probs
    offset += 6

    // Embedding statistics [16 dims]
    features[offset:offset+1] ← Normalize(query_features.embedding_l2_norm, 0, 10)
    offset += 1
    features[offset:offset+8] ← query_features.pca_components[:8]
    offset += 8
    features[offset:offset+1] ← query_features.embedding_entropy
    offset += 1
    features[offset:offset+1] ← query_features.embedding_sparsity
    offset += 1
    features[offset:offset+4] ← query_features.cluster_soft_assignment
    offset += 4
    features[offset:offset+1] ← 0  // padding
    offset += 1

    // Search statistics [48 dims]
    features[offset:offset+1] ← Normalize(search_stats.k_retrieved, 0, 64)
    offset += 1
    features[offset:offset+4] ← [
        Normalize(search_stats.distance_mean, 0, 2),
        Normalize(search_stats.distance_std, 0, 1),
        Normalize(search_stats.distance_min, 0, 2),
        Normalize(search_stats.distance_max, 0, 2)
    ]
    offset += 4
    features[offset:offset+1] ← search_stats.distance_entropy
    offset += 1
    features[offset:offset+1] ← Normalize(search_stats.graph_depth, 0, 10)
    offset += 1
    features[offset:offset+1] ← search_stats.recall_estimate
    offset += 1
    features[offset:offset+16] ← graph_summary.neighborhood_density_histogram
    offset += 16
    features[offset:offset+24] ← graph_summary.semantic_coherence_features
    offset += 24

    // System constraints [32 dims]
    features[offset:offset+1] ← Normalize(constraints.latency_budget_ms, 0, 5000)
    offset += 1
    features[offset:offset+4] ← constraints.device_class_one_hot
    offset += 4
    features[offset:offset+4] ← constraints.privacy_level_one_hot
    offset += 4
    features[offset:offset+1] ← Normalize(constraints.memory_available_mb, 0, 16000)
    offset += 1
    features[offset:offset+1] ← Normalize(constraints.battery_level, 0, 100)
    offset += 1
    features[offset:offset+1] ← Normalize(constraints.concurrent_requests, 0, 100)
    offset += 1
    features[offset:offset+16] ← constraints.historical_accuracy_per_domain
    offset += 16
    features[offset:offset+4] ← [0, 0, 0, 0]  // padding
    offset += 4

    ASSERT offset == 128
    RETURN features
```

---

## 3. Graph Attention Engine

### 3.1 Two-Hop Neighborhood Expansion

```pseudocode
ALGORITHM ExpandNeighborhood(
    center_nodes: List<Node>,
    db: VectorDB,
    max_hops: u32,
    max_per_hop: u32
) -> SubGraph:
    INPUT:
        center_nodes: Initial retrieved nodes
        db: Vector database with graph structure
        max_hops: Maximum expansion hops (typically 2)
        max_per_hop: Maximum neighbors per node per hop
    OUTPUT:
        subgraph: Expanded subgraph with nodes and edges

    visited ← HashSet<NodeID>()
    frontier ← center_nodes
    all_nodes ← center_nodes.clone()
    all_edges ← List<Edge>()

    FOR hop IN 1..=max_hops:
        next_frontier ← List<Node>()

        FOR node IN frontier:
            IF node.id IN visited:
                CONTINUE
            visited.add(node.id)

            // Get outgoing edges
            edges ← db.get_edges(node.id, limit: max_per_hop)
            all_edges.extend(edges)

            FOR edge IN edges:
                IF edge.dst NOT IN visited:
                    neighbor ← db.get_node(edge.dst)
                    next_frontier.append(neighbor)
                    all_nodes.append(neighbor)

        frontier ← next_frontier

    RETURN SubGraph {
        nodes: all_nodes,
        edges: all_edges,
        center_ids: center_nodes.map(n => n.id)
    }
```

### 3.2 Graph Attention Mechanism

```pseudocode
ALGORITHM GraphAttention(
    center_embedding: Vector,
    subgraph: SubGraph,
    config: GraphAttentionConfig
) -> GraphContext:
    INPUT:
        center_embedding: Query embedding
        subgraph: Expanded neighborhood
        config: {num_heads, head_dim, dropout}
    OUTPUT:
        context: Attended graph context

    // Build attention inputs
    node_embeddings ← subgraph.nodes.map(n => n.vector)
    edge_features ← BuildEdgeFeatures(subgraph.edges)
    adjacency ← BuildAdjacencyMatrix(subgraph)

    // Multi-head graph attention
    attended_embeddings ← []
    attention_weights ← []

    FOR head IN 0..config.num_heads:
        // Project Q, K, V for this head
        Q ← Linear(center_embedding, W_Q[head])
        K ← Linear_batch(node_embeddings, W_K[head])
        V ← Linear_batch(node_embeddings, W_V[head])

        // Compute attention scores with edge features
        scores ← []
        FOR i, node IN enumerate(node_embeddings):
            // Base attention
            score ← Dot(Q, K[i]) / Sqrt(config.head_dim)

            // Edge-aware modulation
            IF EdgeExists(center_id, node.id, subgraph):
                edge ← GetEdge(center_id, node.id, subgraph)
                edge_emb ← EdgeEmbed(edge.rel, edge.weight)
                score += Dot(Q, edge_emb)

            // Distance decay
            hop_distance ← GetHopDistance(center_id, node.id, subgraph)
            score *= Exp(-config.distance_decay * hop_distance)

            scores.append(score)

        // Normalize with softmax (masked for disconnected nodes)
        weights ← MaskedSoftmax(scores, adjacency)
        attention_weights.append(weights)

        // Weighted aggregation
        head_output ← WeightedSum(V, weights)
        attended_embeddings.append(head_output)

    // Concatenate heads and project
    concatenated ← Concat(attended_embeddings)
    output ← Linear(concatenated, W_O) + center_embedding  // Residual

    // Rank nodes by attention weight
    avg_weights ← Mean(attention_weights, axis=0)
    ranked_indices ← ArgSort(avg_weights, descending=TRUE)

    RETURN GraphContext {
        embedding: output,
        ranked_nodes: subgraph.nodes[ranked_indices],
        attention_weights: avg_weights[ranked_indices],
        summary: ExtractGraphSummary(subgraph, avg_weights)
    }
```

### 3.3 Edge Feature Encoding

```pseudocode
ALGORITHM BuildEdgeFeatures(edges: List<Edge>) -> EdgeFeatures:
    // Encode edge relationships and metadata

    features ← List<Vector>()

    FOR edge IN edges:
        // Relationship type embedding
        rel_emb ← RELATION_EMBEDDINGS[edge.rel]  // Learned embeddings

        // Temporal features
        age_days ← (NOW - edge.metadata.timestamp) / SECONDS_PER_DAY
        recency ← Exp(-age_days / DECAY_CONSTANT)

        // Confidence and weight
        confidence ← edge.metadata.confidence
        weight ← edge.weight

        // Combine features
        edge_feature ← Concat([
            rel_emb,                    // [16]
            [recency],                  // [1]
            [confidence],               // [1]
            [weight],                   // [1]
            [Log(1 + age_days) / 10]    // [1]
        ])

        features.append(edge_feature)

    RETURN EdgeFeatures { vectors: features, dim: 20 }

CONSTANTS:
    RELATION_EMBEDDINGS = LearnedEmbedding(num_relations=10, dim=16)
    DECAY_CONSTANT = 30.0  // days
```

---

## 4. Self-Learning Algorithms

### 4.1 Memory Writeback

```pseudocode
ALGORITHM MemoryWriteback(
    query: String,
    response: String,
    quality_score: f32,
    db: VectorDB
) -> Result<Option<NodeID>>:
    INPUT:
        query, response: Q&A pair
        quality_score: Judge-evaluated quality [0, 1]
        db: Vector database
    OUTPUT:
        inserted_id: ID of new node, or None if skipped

    // Quality gate
    IF quality_score < QUALITY_THRESHOLD:
        RETURN None

    // Create embedding
    combined_text ← Format("Q: {query}\nA: {response}")
    embedding ← EmbedText(combined_text)

    // Deduplication check
    similar ← db.search(embedding, k=5, threshold=0.95)
    IF similar.len() > 0:
        // Near-duplicate found
        best_match ← similar[0]

        IF quality_score > best_match.metadata.quality:
            // Update existing entry (better quality)
            db.update_metadata(best_match.id, {
                quality: quality_score,
                updated_at: NOW,
                update_count: best_match.metadata.update_count + 1
            })
            RETURN Some(best_match.id)
        ELSE:
            // Skip - existing entry is better
            RETURN None

    // Insert new entry
    node ← Node {
        id: NewUUID(),
        vector: embedding,
        text: combined_text,
        type: NodeType::QAPair,
        source: "self_learning",
        metadata: {
            timestamp: NOW,
            quality: quality_score,
            domain: ClassifyDomain(query),
            version: 1,
            update_count: 0
        }
    }

    inserted_id ← db.insert(node)

    // Create edges to similar existing nodes
    FOR neighbor IN similar:
        edge ← Edge {
            src: inserted_id,
            dst: neighbor.id,
            rel: EdgeType::SameTopic,
            weight: neighbor.score,
            metadata: {
                timestamp: NOW,
                created_by: "self_learning"
            }
        }
        db.insert_edge(edge)

    RETURN Some(inserted_id)

CONSTANTS:
    QUALITY_THRESHOLD = 0.75  // 3.75/5.0
```

### 4.2 Experience Replay Buffer

```pseudocode
ALGORITHM ReservoirSampling:
    // Maintain fixed-size buffer with uniform sampling

    STRUCT ReplayBuffer:
        entries: List<ReplayEntry>
        capacity: u32
        total_seen: u64

    FUNCTION new(capacity: u32) -> ReplayBuffer:
        RETURN ReplayBuffer {
            entries: [],
            capacity: capacity,
            total_seen: 0
        }

    FUNCTION add(self, entry: ReplayEntry):
        self.total_seen += 1

        IF self.entries.len() < self.capacity:
            self.entries.append(entry)
        ELSE:
            // Reservoir sampling: replace with probability capacity/total_seen
            idx ← RandomInt(0, self.total_seen)
            IF idx < self.capacity:
                self.entries[idx] ← entry

    FUNCTION sample(self, batch_size: u32) -> List<ReplayEntry>:
        IF self.entries.len() < batch_size:
            RETURN self.entries.clone()

        indices ← RandomSample(0, self.entries.len(), batch_size, replace=FALSE)
        RETURN indices.map(i => self.entries[i].clone())

    FUNCTION distribution_stats(self) -> DistributionStats:
        // Analyze distribution for curriculum balancing
        domain_counts ← CountBy(self.entries, e => e.domain)
        quality_hist ← Histogram(self.entries.map(e => e.quality), bins=10)
        complexity_hist ← Histogram(self.entries.map(e => e.complexity), bins=10)

        RETURN DistributionStats {
            domain_counts,
            quality_hist,
            complexity_hist,
            coverage: domain_counts.len() / TOTAL_DOMAINS
        }
```

### 4.3 EWC Training Update

```pseudocode
ALGORITHM EWCTrainingStep(
    model: RouterModel,
    batch: List<TrainingSample>,
    ewc: ElasticWeightConsolidation,
    optimizer: Optimizer
) -> TrainingMetrics:
    INPUT:
        model: FastGRNN router model
        batch: Training samples with labels
        ewc: EWC state with Fisher info and optimal weights
        optimizer: Adam optimizer
    OUTPUT:
        metrics: Loss and accuracy metrics

    // Forward pass
    predictions ← []
    FOR sample IN batch:
        features ← BuildRouterFeatures(sample)
        pred ← model.forward(features, sample.hidden_state)
        predictions.append(pred)

    // Task loss
    model_loss ← CrossEntropy(
        predictions.map(p => p.model_probs),
        batch.map(s => s.label_model)
    )

    context_loss ← CrossEntropy(
        predictions.map(p => p.context_probs),
        batch.map(s => s.label_context)
    )

    temp_loss ← SmoothL1(
        predictions.map(p => p.temperature),
        batch.map(s => s.label_temperature)
    )

    top_p_loss ← SmoothL1(
        predictions.map(p => p.top_p),
        batch.map(s => s.label_top_p)
    )

    task_loss ← model_loss + context_loss + ALPHA * temp_loss + BETA * top_p_loss

    // EWC regularization loss
    current_weights ← model.get_weights()
    ewc_loss ← ewc.regularization_loss(current_weights)

    // Total loss
    total_loss ← task_loss + ewc_loss

    // Backward pass
    gradients ← Backward(total_loss, model.parameters())

    // Optimizer step
    optimizer.step(model.parameters(), gradients)

    // Compute metrics
    accuracy ← ComputeAccuracy(predictions, batch)

    RETURN TrainingMetrics {
        total_loss,
        task_loss,
        ewc_loss,
        model_accuracy: accuracy.model,
        context_accuracy: accuracy.context
    }

CONSTANTS:
    ALPHA = 0.1  // Temperature loss weight
    BETA = 0.1   // Top-p loss weight
```

### 4.4 Fisher Information Update

```pseudocode
ALGORITHM UpdateFisherInformation(
    model: RouterModel,
    dataset: List<Sample>,
    ewc: ElasticWeightConsolidation,
    num_samples: u32
) -> ElasticWeightConsolidation:
    // Compute Fisher information diagonal approximation

    // Sample subset for efficiency
    samples ← RandomSample(dataset, num_samples)

    // Accumulate squared gradients
    fisher_accum ← ZeroVector(model.num_parameters())

    FOR sample IN samples:
        features ← BuildRouterFeatures(sample)
        pred ← model.forward(features, sample.hidden_state)

        // Log-likelihood gradient (for correctly classified samples)
        log_prob ← Log(pred.model_probs[sample.label_model])
        gradients ← Backward(log_prob, model.parameters())

        // Accumulate squared gradients
        FOR i IN 0..model.num_parameters():
            fisher_accum[i] += gradients[i] ** 2

    // Average
    fisher_diag ← fisher_accum / num_samples

    // Update EWC state
    ewc.fisher_info ← fisher_diag
    ewc.optimal_weights ← model.get_weights().clone()

    RETURN ewc
```

---

## 5. LFM2 Inference

### 5.1 Generation with KV Cache

```pseudocode
ALGORITHM LFM2Generate(
    model: LFM2Model,
    prompt: String,
    config: GenerationConfig,
    kv_cache: Option<KVCache>
) -> (String, KVCache):
    INPUT:
        model: Loaded LFM2 model (350M/700M/1.2B/2.6B)
        prompt: Formatted prompt with context
        config: {temperature, top_p, max_tokens}
        kv_cache: Optional cached KV states from previous turn
    OUTPUT:
        response: Generated text
        updated_cache: KV cache for reuse

    // Tokenize prompt
    tokens ← Tokenize(prompt)

    // Determine cache reuse
    IF kv_cache IS NOT None AND prompt.starts_with(kv_cache.prefix):
        // Reuse cached KV states
        new_tokens ← tokens[kv_cache.prefix_len:]
        cache ← kv_cache.states
    ELSE:
        // Start fresh
        new_tokens ← tokens
        cache ← None

    // Prefill phase (process prompt)
    cache ← model.prefill(new_tokens, cache)

    // Decode phase (generate tokens)
    output_tokens ← []
    FOR _ IN 0..config.max_tokens:
        // Get next token logits
        logits ← model.decode_step(cache)

        // Apply temperature
        logits ← logits / config.temperature

        // Top-p (nucleus) sampling
        sorted_idx ← ArgSort(logits, descending=TRUE)
        cumsum ← CumulativeSum(Softmax(logits[sorted_idx]))
        cutoff_idx ← FirstWhere(cumsum > config.top_p)
        valid_idx ← sorted_idx[:cutoff_idx + 1]

        // Sample from valid tokens
        probs ← Softmax(logits[valid_idx])
        next_token ← Sample(valid_idx, probs)

        // Check for EOS
        IF next_token == EOS_TOKEN:
            BREAK

        output_tokens.append(next_token)

        // Update cache
        cache ← model.update_cache(cache, next_token)

    // Decode to text
    response ← Detokenize(output_tokens)

    // Build updated cache
    updated_cache ← KVCache {
        prefix: prompt,
        prefix_len: tokens.len(),
        states: cache
    }

    RETURN (response, updated_cache)
```

### 5.2 Model Selection and Loading

```pseudocode
ALGORITHM SelectAndLoadModel(
    model_size: ModelSize,
    device: DeviceType,
    memory_budget: u64
) -> LFM2Model:
    INPUT:
        model_size: Enum {350M, 700M, 1.2B, 2.6B}
        device: Enum {CPU, GPU, NPU}
        memory_budget: Available memory in bytes
    OUTPUT:
        model: Loaded and optimized model

    // Determine quantization based on device and memory
    quantization ← SelectQuantization(model_size, device, memory_budget)

    // Model paths
    model_path ← MODEL_PATHS[model_size][quantization]

    // Load model
    MATCH device:
        CPU:
            model ← LlamaCpp.load(model_path, {
                n_ctx: GetContextSize(model_size),
                n_threads: GetOptimalThreads(),
                use_mmap: TRUE,
                use_mlock: FALSE
            })

        GPU:
            model ← VLLM.load(model_path, {
                tensor_parallel: GetGPUCount(),
                dtype: quantization.dtype,
                max_model_len: GetContextSize(model_size)
            })

        NPU:
            // ExecuTorch for edge devices
            model ← ExecuTorch.load(model_path + ".pte")

    RETURN model


ALGORITHM SelectQuantization(
    model_size: ModelSize,
    device: DeviceType,
    memory_budget: u64
) -> Quantization:
    // Memory requirements (approximate)
    base_memory ← MODEL_BASE_MEMORY[model_size]

    IF device == GPU:
        IF memory_budget >= base_memory:
            RETURN Quantization::FP16
        ELSE IF memory_budget >= base_memory / 2:
            RETURN Quantization::INT8
        ELSE:
            RETURN Quantization::INT4

    ELSE:  // CPU
        IF memory_budget >= base_memory / 2:
            RETURN Quantization::Q5_K_M
        ELSE IF memory_budget >= base_memory / 4:
            RETURN Quantization::Q4_K_M
        ELSE:
            RETURN Quantization::Q2_K

CONSTANTS:
    MODEL_BASE_MEMORY = {
        350M: 700_000_000,    // ~700MB FP16
        700M: 1_400_000_000,  // ~1.4GB FP16
        1.2B: 2_400_000_000,  // ~2.4GB FP16
        2.6B: 5_200_000_000   // ~5.2GB FP16
    }
```

---

## 6. Utility Algorithms

### 6.1 Quality Evaluation

```pseudocode
ALGORITHM EvaluateQuality(
    query: String,
    response: String,
    context: List<Document>
) -> f32:
    INPUT:
        query: Original user query
        response: Generated response
        context: Retrieved context documents
    OUTPUT:
        score: Quality score [0, 1]

    // Build evaluation prompt
    context_text ← context.map(d => d.text).join("\n---\n")

    eval_prompt ← Format("""
        Evaluate the following response on a scale of 1-5.

        === Context ===
        {context_text}

        === Query ===
        {query}

        === Response ===
        {response}

        === Evaluation Criteria ===
        1. Factual Accuracy: Is the response grounded in the context?
        2. Completeness: Does it fully address the query?
        3. Coherence: Is the response logically structured?
        4. Conciseness: Is it appropriately brief without being incomplete?

        Provide your evaluation as a single integer from 1 to 5:
    """)

    // Use judge model (typically 2.6B)
    judge_response ← JUDGE_MODEL.generate(eval_prompt, max_tokens=10)

    // Parse score
    score_int ← ParseInteger(judge_response.trim())
    IF score_int IS None OR score_int < 1 OR score_int > 5:
        score_int ← 3  // Default to neutral on parse failure

    // Normalize to [0, 1]
    score ← (score_int - 1) / 4.0

    RETURN score
```

### 6.2 Context Building

```pseudocode
ALGORITHM BuildContext(
    ranked_nodes: List<Node>,
    max_tokens: u32,
    deduplicate: bool
) -> ContextResult:
    INPUT:
        ranked_nodes: Attention-ranked nodes
        max_tokens: Maximum context token budget
        deduplicate: Whether to remove near-duplicate content
    OUTPUT:
        context: Constructed context with sources

    selected_nodes ← []
    seen_hashes ← HashSet<u64>()
    total_tokens ← 0

    FOR node IN ranked_nodes:
        // Token count
        node_tokens ← CountTokens(node.text)

        // Check budget
        IF total_tokens + node_tokens > max_tokens:
            CONTINUE

        // Deduplication
        IF deduplicate:
            text_hash ← MinHash(node.text, num_hashes=128)
            similar_seen ← seen_hashes.any(h => JaccardSimilarity(h, text_hash) > 0.8)
            IF similar_seen:
                CONTINUE
            seen_hashes.add(text_hash)

        selected_nodes.append(node)
        total_tokens += node_tokens

    // Format context
    context_text ← selected_nodes.enumerate()
        .map((i, node) => Format("[{i+1}] {node.text}"))
        .join("\n\n")

    sources ← selected_nodes.map(n => Source {
        id: n.id,
        text_preview: n.text[:100],
        confidence: n.metadata.confidence
    })

    RETURN ContextResult {
        text: context_text,
        sources: sources,
        token_count: total_tokens,
        nodes_used: selected_nodes.len()
    }
```

### 6.3 Telemetry Logging

```pseudocode
ALGORITHM LogTelemetry(
    routing: RoutingDecision,
    search_stats: SearchStatistics,
    latency: LatencyBreakdown,
    quality: f32
):
    entry ← TelemetryEntry {
        timestamp: NOW,
        request_id: CurrentRequestID(),

        // Routing
        model_selected: routing.model_selection,
        model_probs: routing.model_probs,
        context_size: routing.context_size,
        temperature: routing.temperature,
        top_p: routing.top_p,
        router_confidence: routing.confidence,

        // Retrieval
        k_retrieved: search_stats.k_retrieved,
        distance_stats: search_stats.distances,
        graph_depth: search_stats.graph_depth,

        // Latency
        total_ms: latency.total,
        retrieval_ms: latency.retrieval,
        routing_ms: latency.routing,
        generation_ms: latency.generation,
        writeback_ms: latency.writeback,

        // Quality
        quality_score: quality,

        // System
        device_class: CurrentDevice(),
        memory_used: GetMemoryUsage()
    }

    // Async write to metrics store
    METRICS_CHANNEL.send(entry)

    // Prometheus metrics
    HISTOGRAM_LATENCY.observe(latency.total)
    COUNTER_REQUESTS.inc()
    GAUGE_QUALITY.set(quality)
    HISTOGRAM_MODEL.observe(ModelSizeToInt(routing.model_selection))
```

---

## 7. Initialization and Shutdown

### 7.1 System Initialization

```pseudocode
ALGORITHM InitializeRuvLLM(config: RuvLLMConfig) -> RuvLLMSystem:
    // 1. Initialize vector database
    db ← VectorDB.open(config.db_path, {
        dimensions: config.embedding_dim,
        hnsw_m: config.hnsw_m,
        hnsw_ef_construction: config.hnsw_ef_construction
    })

    // 2. Load embedding model
    embedder ← EmbeddingAdapter.load(config.embedding_model_path)

    // 3. Initialize router
    router ← FastGRNNRouter.load(config.router_model_path)

    // 4. Load LFM2 models (lazy loading for memory efficiency)
    models ← LazyModelLoader {
        paths: config.lfm2_paths,
        loaded: HashMap::new(),
        max_loaded: config.max_concurrent_models
    }

    // 5. Initialize graph attention
    graph_attention ← GraphAttentionEngine.new({
        num_heads: config.attention_heads,
        head_dim: config.attention_head_dim
    })

    // 6. Initialize self-learning components
    replay_buffer ← ReplayBuffer.new(config.replay_capacity)
    ewc ← ElasticWeightConsolidation.load_or_new(config.ewc_path)
    optimizer ← Adam.new(router.parameters(), lr=config.learning_rate)

    // 7. Initialize quality judge
    judge ← QualityJudge.new(models.get(ModelSize::2.6B))

    // 8. Start background services
    telemetry_service ← TelemetryService.start(config.metrics_endpoint)
    training_service ← TrainingService.start(
        router, replay_buffer, ewc, optimizer,
        config.training_interval
    )

    RETURN RuvLLMSystem {
        db, embedder, router, models,
        graph_attention, replay_buffer, ewc,
        judge, telemetry_service, training_service
    }
```

### 7.2 Graceful Shutdown

```pseudocode
ALGORITHM ShutdownRuvLLM(system: RuvLLMSystem):
    // 1. Stop accepting new requests
    system.accepting_requests ← FALSE

    // 2. Wait for in-flight requests (with timeout)
    WaitWithTimeout(system.request_counter == 0, timeout=30s)

    // 3. Flush replay buffer
    system.replay_buffer.persist(config.replay_path)

    // 4. Save EWC state
    system.ewc.persist(config.ewc_path)

    // 5. Save router checkpoint
    system.router.save_checkpoint(config.router_checkpoint_path)

    // 6. Flush metrics
    system.telemetry_service.flush()

    // 7. Close database
    system.db.sync()
    system.db.close()

    // 8. Unload models
    system.models.unload_all()

    LOG("RuvLLM shutdown complete")
```

---

*Document Version: 1.0*
*Last Updated: 2025-12-02*
*Author: RuvLLM Architecture Team*
