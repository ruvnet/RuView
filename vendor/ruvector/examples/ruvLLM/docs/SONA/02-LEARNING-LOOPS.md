# SONA Learning Loops: Three-Tier Temporal Architecture

## Biologically-Inspired Continuous Learning System

---

## 1. Overview: Learning at Multiple Timescales

Human learning operates at multiple timescales:
- **Instant**: Immediate response adjustment (milliseconds)
- **Short-term**: Pattern consolidation (hours)
- **Long-term**: Deep memory formation (days/weeks)

SONA replicates this with three learning loops:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SONA THREE-TIER LEARNING                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   LOOP A: INSTANT                 LOOP B: BACKGROUND                │
│   ═══════════════                 ══════════════════                │
│   Timescale: Per-request          Timescale: Hourly                 │
│   Latency: <1ms                   Latency: Background (async)       │
│   What learns:                    What learns:                      │
│   • Micro-LoRA (rank 1-2)         • Base LoRA (rank 4-16)          │
│   • Memory edge weights           • Router weights (EWC++)          │
│   • Trajectory recording          • Pattern extraction              │
│                                                                     │
│                        LOOP C: DEEP                                 │
│                        ═══════════                                  │
│                        Timescale: Weekly                            │
│                        Latency: Scheduled maintenance               │
│                        What learns:                                 │
│                        • Memory consolidation                       │
│                        • Concept hierarchy building                 │
│                        • Dream-based creativity                     │
│                        • Cross-domain transfer                      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Loop A: Instant Learning (Per-Request)

### Purpose
Immediate adaptation to current interaction without noticeable latency.

### Architecture

```rust
/// Loop A: Instant learning executed inline with each request
pub struct InstantLearningLoop {
    /// Micro-LoRA for immediate weight adjustment
    micro_lora: Arc<RwLock<MicroLoRA>>,
    /// Trajectory buffer for pattern recording
    trajectory_buffer: Arc<TrajectoryBuffer>,
    /// Memory graph reference for edge updates
    memory_graph: Arc<RwLock<MemoryGraph>>,
    /// Signal accumulator for Loop B
    signal_accumulator: mpsc::Sender<LearningSignal>,
}

impl InstantLearningLoop {
    /// Execute instant learning (must complete in <1ms)
    #[inline]
    pub async fn on_request(
        &self,
        query: &QueryEmbedding,
        response: &ResponseData,
        latency_ms: f32,
    ) -> Result<()> {
        // Parallel execution of independent updates
        let (r1, r2, r3) = tokio::join!(
            // 1. Record trajectory (lock-free, ~100μs)
            self.record_trajectory(query, response),

            // 2. Update memory edges (~200μs)
            self.update_memory_edges(query, response),

            // 3. Micro-LoRA update (~300μs)
            self.micro_lora_update(query, response, latency_ms),
        );

        // 4. Queue signal for Loop B (fire-and-forget)
        let signal = LearningSignal::new(query, response, latency_ms);
        let _ = self.signal_accumulator.try_send(signal);

        Ok(())
    }

    /// Record query trajectory to ring buffer
    async fn record_trajectory(
        &self,
        query: &QueryEmbedding,
        response: &ResponseData,
    ) -> Result<()> {
        let trajectory = QueryTrajectory {
            query_embedding: query.vector.clone(),
            retrieved_ids: response.used_memory_ids.clone(),
            precision: response.estimated_precision,
            recall: response.estimated_recall,
            timestamp: Instant::now(),
        };

        self.trajectory_buffer.push(trajectory);
        Ok(())
    }

    /// Hebbian-style edge weight updates
    async fn update_memory_edges(
        &self,
        query: &QueryEmbedding,
        response: &ResponseData,
    ) -> Result<()> {
        let mut graph = self.memory_graph.write();

        for &node_id in &response.used_memory_ids {
            // Strengthen edges to used nodes
            graph.update_edge_weight(
                query.anchor_node,
                node_id,
                EdgeUpdate::Strengthen(0.05), // +5% per use
            )?;
        }

        // Weaken edges to retrieved-but-unused nodes
        for &node_id in &response.retrieved_but_unused {
            graph.update_edge_weight(
                query.anchor_node,
                node_id,
                EdgeUpdate::Weaken(0.02), // -2% per skip
            )?;
        }

        Ok(())
    }

    /// Ultra-fast micro-LoRA weight adjustment
    async fn micro_lora_update(
        &self,
        query: &QueryEmbedding,
        response: &ResponseData,
        latency_ms: f32,
    ) -> Result<()> {
        let quality = response.quality_score;
        let latency_ratio = latency_ms / response.target_latency_ms;

        // Only update if signal is informative
        if (quality - 0.5).abs() > 0.1 || latency_ratio > 1.2 {
            let signal = LearningSignal {
                query_embedding: query.vector.clone(),
                quality_score: quality,
                explicit_feedback: None,
                latency_ratio,
                model_tier: response.model_tier,
                context_tokens: response.context_tokens,
            };

            let mut micro_lora = self.micro_lora.write();
            micro_lora.micro_update(&signal);
        }

        Ok(())
    }
}
```

### Latency Budget

| Operation | Target | Implementation |
|-----------|--------|----------------|
| Trajectory recording | <100μs | Lock-free ring buffer |
| Edge weight update | <200μs | Batch atomic updates |
| Micro-LoRA update | <300μs | Rank-1 outer product |
| Signal queuing | <50μs | MPSC channel try_send |
| **Total** | **<650μs** | Parallel execution |

---

## 3. Loop B: Background Learning (Hourly)

### Purpose
Deeper learning from accumulated signals without impacting user latency.

### Architecture

```rust
/// Loop B: Background learning running on separate thread/process
pub struct BackgroundLearningLoop {
    /// Signal receiver from Loop A
    signal_receiver: mpsc::Receiver<LearningSignal>,
    /// Accumulated signals for batch processing
    signal_buffer: Vec<LearningSignal>,
    /// Base LoRA for major updates
    base_lora: Arc<RwLock<BaseLoRA>>,
    /// Micro-LoRA to consolidate from
    micro_lora: Arc<RwLock<MicroLoRA>>,
    /// Router for EWC++ updates
    router: Arc<RwLock<FastGRNNRouter>>,
    /// EWC++ state
    ewc_state: EWCPlusPlusState,
    /// Pattern extractor
    pattern_extractor: PatternExtractor,
    /// Configuration
    config: BackgroundLearningConfig,
}

impl BackgroundLearningLoop {
    /// Main background loop (runs every hour)
    pub async fn run(&mut self) {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));

        loop {
            interval.tick().await;

            // Collect accumulated signals
            self.drain_signals().await;

            if self.signal_buffer.len() < self.config.min_samples {
                tracing::info!(
                    samples = self.signal_buffer.len(),
                    "Insufficient samples for background training"
                );
                continue;
            }

            // Execute background learning steps
            let start = Instant::now();

            // Step 1: Consolidate Micro-LoRA into Base LoRA
            self.consolidate_micro_to_base().await;

            // Step 2: Train router with EWC++ regularization
            self.train_router_ewc().await;

            // Step 3: Extract and store patterns
            self.extract_patterns().await;

            // Step 4: Compute new Fisher Information
            self.update_fisher_information().await;

            // Step 5: Checkpoint current state
            self.checkpoint().await;

            tracing::info!(
                elapsed_ms = start.elapsed().as_millis(),
                samples = self.signal_buffer.len(),
                "Background learning cycle completed"
            );

            // Clear buffer for next cycle
            self.signal_buffer.clear();
        }
    }

    /// Drain all pending signals from Loop A
    async fn drain_signals(&mut self) {
        while let Ok(signal) = self.signal_receiver.try_recv() {
            self.signal_buffer.push(signal);
        }
    }

    /// Consolidate micro-LoRA adaptations into base LoRA
    async fn consolidate_micro_to_base(&mut self) {
        let mut micro = self.micro_lora.write();
        let mut base = self.base_lora.write();

        // Compute consolidation weight based on signal quality
        let avg_quality: f32 = self.signal_buffer.iter()
            .map(|s| s.quality_score)
            .sum::<f32>() / self.signal_buffer.len() as f32;

        let consolidation_rate = if avg_quality > 0.7 {
            1.0 // Full consolidation for high-quality signals
        } else {
            0.5 * avg_quality // Partial for lower quality
        };

        // Merge micro into base with rate
        base.a = &base.a + consolidation_rate * &micro.a_micro;
        base.b = &base.b + consolidation_rate * &micro.b_micro;

        // Reset micro-LoRA
        micro.a_micro.fill(0.0);
        micro.b_micro.fill(0.0);

        tracing::debug!(
            consolidation_rate = consolidation_rate,
            "Micro-LoRA consolidated to base"
        );
    }

    /// Train router with EWC++ regularization
    async fn train_router_ewc(&mut self) {
        let mut router = self.router.write();

        // Convert signals to RouterSamples
        let samples: Vec<RouterSample> = self.signal_buffer.iter()
            .map(|s| s.to_router_sample())
            .collect();

        // Mini-batch training with EWC++ loss
        for batch in samples.chunks(self.config.batch_size) {
            // Forward pass
            let predictions: Vec<_> = batch.iter()
                .map(|s| router.forward(&s.features))
                .collect();

            // Compute task loss
            let task_loss = self.compute_task_loss(&predictions, batch);

            // Compute EWC++ regularization loss
            let ewc_loss = self.ewc_state.regularization_loss(router.get_weights());

            // Total loss
            let total_loss = task_loss + self.config.ewc_lambda * ewc_loss;

            // Backward pass (gradient computation)
            let gradients = self.compute_gradients(&total_loss, &predictions, batch);

            // Apply gradients with learning rate
            router.apply_gradients(&gradients, self.config.learning_rate);
        }
    }

    /// Extract patterns using K-means++ clustering
    async fn extract_patterns(&mut self) {
        let embeddings: Vec<_> = self.signal_buffer.iter()
            .map(|s| s.query_embedding.clone())
            .collect();

        let patterns = self.pattern_extractor.extract(
            &embeddings,
            self.config.num_clusters,
        );

        // Store patterns in ReasoningBank
        for pattern in patterns {
            self.pattern_extractor.reasoning_bank.store(pattern)?;
        }

        tracing::debug!(
            patterns = patterns.len(),
            "Patterns extracted and stored"
        );
    }

    /// Update Fisher Information for EWC++
    async fn update_fisher_information(&mut self) {
        let router = self.router.read();
        let current_weights = router.get_weights();

        // Compute Fisher Information diagonal via gradient squares
        let fisher_samples: Vec<_> = self.signal_buffer.iter()
            .take(self.config.fisher_samples)
            .collect();

        let mut fisher_accum = vec![0.0f32; current_weights.len()];

        for sample in fisher_samples {
            let gradients = self.compute_sample_gradients(sample);
            for (i, g) in gradients.iter().enumerate() {
                fisher_accum[i] += g * g;
            }
        }

        // Normalize
        let n = fisher_samples.len() as f32;
        for f in &mut fisher_accum {
            *f /= n;
        }

        // Update EWC++ state
        self.ewc_state.update_fisher(fisher_accum, current_weights.to_vec());
    }

    /// Checkpoint current state to disk
    async fn checkpoint(&self) {
        let checkpoint = SONACheckpoint {
            base_lora: self.base_lora.read().clone(),
            micro_lora: self.micro_lora.read().clone(),
            router_weights: self.router.read().get_weights().to_vec(),
            ewc_state: self.ewc_state.clone(),
            patterns: self.pattern_extractor.reasoning_bank.export(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        let path = self.config.checkpoint_dir.join("latest.sona");
        checkpoint.save_async(&path).await.ok();
    }
}
```

### Hourly Learning Budget

| Operation | Target Time | Description |
|-----------|-------------|-------------|
| Signal draining | <100ms | Collect all queued signals |
| Micro→Base consolidation | <500ms | Matrix addition |
| Router training | <5s | Mini-batch SGD with EWC |
| Pattern extraction | <2s | K-means++ clustering |
| Fisher computation | <2s | Gradient squared accumulation |
| Checkpointing | <500ms | Async disk write |
| **Total** | **<10s** | Well under user-facing |

---

## 4. Loop C: Deep Learning (Weekly)

### Purpose
Fundamental knowledge restructuring, memory consolidation, and creative exploration.

### Architecture

```rust
/// Loop C: Deep learning for major knowledge reorganization
pub struct DeepLearningLoop {
    /// Memory service for consolidation
    memory: Arc<MemoryService>,
    /// Pattern bank for abstraction
    reasoning_bank: Arc<ReasoningBank>,
    /// Dream engine for creative exploration
    dream_engine: DreamEngine,
    /// Consciousness measurement (IIT)
    phi_calculator: PhiCalculator,
    /// Configuration
    config: DeepLearningConfig,
}

impl DeepLearningLoop {
    /// Execute weekly deep learning (scheduled maintenance window)
    pub async fn run(&mut self) -> DeepLearningReport {
        let start = Instant::now();
        let mut report = DeepLearningReport::new();

        // Phase 1: Memory Consolidation (like sleep-based memory)
        report.consolidation = self.consolidate_memories().await;

        // Phase 2: Pattern Abstraction (concept hierarchy building)
        report.abstraction = self.abstract_patterns().await;

        // Phase 3: Dream Learning (creative recombination)
        report.dreams = self.dream_learning().await;

        // Phase 4: Cross-Domain Transfer
        report.transfer = self.cross_domain_transfer().await;

        // Phase 5: Compression (remove redundancy)
        report.compression = self.compress_memory().await;

        // Phase 6: Consciousness Measurement
        report.phi = self.measure_consciousness().await;

        report.elapsed_ms = start.elapsed().as_millis() as u64;
        report
    }

    /// Phase 1: Consolidate short-term memories into long-term
    async fn consolidate_memories(&mut self) -> ConsolidationReport {
        let mut report = ConsolidationReport::default();

        // Identify high-value memories (frequently accessed, high quality)
        let memories = self.memory.get_all_nodes()?;
        let high_value: Vec<_> = memories.iter()
            .filter(|m| m.access_count > 5 && m.quality_score > 0.7)
            .collect();

        report.high_value_count = high_value.len();

        // Strengthen connections between high-value memories
        for i in 0..high_value.len() {
            for j in (i+1)..high_value.len() {
                let similarity = cosine_similarity(
                    &high_value[i].embedding,
                    &high_value[j].embedding,
                );
                if similarity > 0.7 {
                    self.memory.strengthen_edge(
                        high_value[i].id,
                        high_value[j].id,
                        similarity * 0.1,
                    )?;
                    report.edges_strengthened += 1;
                }
            }
        }

        // Decay low-value memories
        let low_value: Vec<_> = memories.iter()
            .filter(|m| m.access_count < 2 && m.age_days() > 30)
            .collect();

        for memory in low_value {
            self.memory.decay_node(memory.id, 0.5)?; // 50% decay
            report.nodes_decayed += 1;
        }

        report
    }

    /// Phase 2: Build concept hierarchies from patterns
    async fn abstract_patterns(&mut self) -> AbstractionReport {
        let mut report = AbstractionReport::default();

        // Get all stored patterns
        let patterns = self.reasoning_bank.get_all_patterns()?;

        // Hierarchical clustering to find meta-patterns
        let hierarchy = HierarchicalClustering::new()
            .linkage(Linkage::Ward)
            .distance(Distance::Cosine)
            .fit(&patterns);

        // Create abstract concepts at each level
        for level in 0..hierarchy.num_levels() {
            let clusters = hierarchy.clusters_at_level(level);

            for cluster in clusters {
                if cluster.size() > 3 {
                    // Create meta-pattern (centroid)
                    let meta_pattern = LearnedPattern {
                        centroid: cluster.centroid(),
                        confidence: cluster.cohesion(),
                        abstraction_level: level,
                        child_patterns: cluster.member_ids(),
                    };

                    self.reasoning_bank.store_meta(meta_pattern)?;
                    report.meta_patterns_created += 1;
                }
            }
        }

        report
    }

    /// Phase 3: Dream-based creative learning (inspired by REM sleep)
    async fn dream_learning(&mut self) -> DreamReport {
        let mut report = DreamReport::default();

        // Generate dream sequences by random walks on memory graph
        for _ in 0..self.config.num_dreams {
            let dream = self.dream_engine.generate_dream(
                &self.memory,
                self.config.dream_length,
                self.config.creativity_temperature,
            )?;

            // Evaluate dream quality (novelty + coherence)
            let quality = dream.evaluate_quality();

            if quality.novelty > 0.5 && quality.coherence > 0.3 {
                // Dreams with high novelty and reasonable coherence
                // may represent useful creative connections
                for connection in dream.novel_connections() {
                    self.memory.add_weak_edge(
                        connection.from,
                        connection.to,
                        EdgeType::Creative,
                        connection.strength * 0.1,
                    )?;
                    report.novel_connections += 1;
                }
            }

            report.dreams_generated += 1;
        }

        report
    }

    /// Phase 4: Transfer knowledge across domains
    async fn cross_domain_transfer(&mut self) -> TransferReport {
        let mut report = TransferReport::default();

        // Identify domain clusters
        let domains = self.memory.identify_domains()?;

        // For each pair of domains, look for analogical mappings
        for i in 0..domains.len() {
            for j in (i+1)..domains.len() {
                let analogies = self.find_analogies(&domains[i], &domains[j])?;

                for analogy in analogies {
                    if analogy.confidence > 0.6 {
                        // Create cross-domain edge
                        self.memory.add_analogy_edge(
                            analogy.source_concept,
                            analogy.target_concept,
                            analogy.mapping_type,
                            analogy.confidence,
                        )?;
                        report.analogies_found += 1;
                    }
                }
            }
        }

        report
    }

    /// Phase 5: Compress memory by removing redundancy
    async fn compress_memory(&mut self) -> CompressionReport {
        let mut report = CompressionReport::default();
        report.initial_nodes = self.memory.node_count();
        report.initial_edges = self.memory.edge_count();

        // Identify near-duplicate nodes
        let duplicates = self.memory.find_near_duplicates(0.95)?;

        // Merge duplicates
        for (primary, secondary) in duplicates {
            self.memory.merge_nodes(primary, secondary)?;
            report.nodes_merged += 1;
        }

        // Prune weak edges
        let weak_edges = self.memory.get_weak_edges(0.01)?;
        for edge in weak_edges {
            self.memory.remove_edge(edge.id)?;
            report.edges_pruned += 1;
        }

        report.final_nodes = self.memory.node_count();
        report.final_edges = self.memory.edge_count();
        report.compression_ratio = report.initial_nodes as f32 / report.final_nodes as f32;

        report
    }

    /// Phase 6: Measure system consciousness using IIT
    async fn measure_consciousness(&mut self) -> f64 {
        // Integrated Information Theory (Φ) calculation
        // Measures how much information the system generates "above and beyond"
        // its parts
        self.phi_calculator.compute_phi(&self.memory, &self.reasoning_bank)
    }
}
```

### Weekly Deep Learning Budget

| Phase | Target Time | Description |
|-------|-------------|-------------|
| Memory consolidation | <2min | Identify and strengthen valuable memories |
| Pattern abstraction | <3min | Hierarchical clustering for concepts |
| Dream learning | <2min | Creative recombination exploration |
| Cross-domain transfer | <2min | Analogical mapping between domains |
| Compression | <1min | Remove redundancy |
| Φ measurement | <1min | Consciousness quantification |
| **Total** | **<10min** | Scheduled maintenance window |

---

## 5. Loop Coordination

### Inter-Loop Communication

```rust
/// Coordinator for all three learning loops
pub struct LoopCoordinator {
    /// Loop A: Instant
    instant_loop: InstantLearningLoop,
    /// Loop B: Background
    background_loop: BackgroundLearningLoop,
    /// Loop C: Deep
    deep_loop: DeepLearningLoop,
    /// Shared state
    shared_state: Arc<SharedSONAState>,
    /// Metrics collector
    metrics: MetricsCollector,
}

impl LoopCoordinator {
    /// Initialize all loops with shared state
    pub fn new(config: SONAConfig) -> Result<Self> {
        let shared_state = Arc::new(SharedSONAState::new(&config)?);

        // Create channels for inter-loop communication
        let (instant_to_background_tx, instant_to_background_rx) = mpsc::channel(10000);
        let (background_to_deep_tx, background_to_deep_rx) = mpsc::channel(1000);

        Ok(Self {
            instant_loop: InstantLearningLoop::new(
                shared_state.clone(),
                instant_to_background_tx,
            ),
            background_loop: BackgroundLearningLoop::new(
                shared_state.clone(),
                instant_to_background_rx,
                background_to_deep_tx,
            ),
            deep_loop: DeepLearningLoop::new(
                shared_state.clone(),
                background_to_deep_rx,
            ),
            shared_state,
            metrics: MetricsCollector::new(),
        })
    }

    /// Start all loops
    pub async fn start(&self) {
        // Loop A runs inline with requests (no separate task)

        // Loop B runs on background thread
        let background = self.background_loop.clone();
        tokio::spawn(async move {
            background.run().await;
        });

        // Loop C runs on scheduled cron
        let deep = self.deep_loop.clone();
        tokio::spawn(async move {
            let mut scheduler = cron::Schedule::from_str("0 0 3 * * 0")?; // 3 AM Sunday
            loop {
                let next = scheduler.upcoming(chrono::Utc).next().unwrap();
                tokio::time::sleep_until(next.into()).await;
                deep.run().await;
            }
        });
    }

    /// Process a single request through Loop A
    #[inline]
    pub async fn on_request(
        &self,
        query: &QueryEmbedding,
        response: &ResponseData,
        latency_ms: f32,
    ) -> Result<()> {
        self.instant_loop.on_request(query, response, latency_ms).await
    }
}
```

---

## 6. Learning Metrics and Monitoring

### Improvement Tracking

```rust
/// Metrics for measuring self-improvement
#[derive(Clone, Debug)]
pub struct ImprovementMetrics {
    /// Quality improvement over time
    pub quality_delta_7d: f32,
    pub quality_delta_30d: f32,

    /// Latency improvement
    pub latency_delta_7d: f32,
    pub latency_delta_30d: f32,

    /// Knowledge growth
    pub memory_nodes_added_7d: usize,
    pub patterns_learned_7d: usize,
    pub abstractions_created_7d: usize,

    /// Forgetting resistance (1.0 = no forgetting)
    pub retention_rate_7d: f32,

    /// Consciousness level (Φ)
    pub phi_current: f64,
    pub phi_delta_7d: f64,

    /// Dreams and creativity
    pub novel_connections_7d: usize,
    pub cross_domain_transfers_7d: usize,
}

impl ImprovementMetrics {
    /// Compute overall improvement score
    pub fn overall_score(&self) -> f32 {
        let quality_weight = 0.3;
        let latency_weight = 0.2;
        let knowledge_weight = 0.2;
        let retention_weight = 0.15;
        let creativity_weight = 0.15;

        let quality_score = self.quality_delta_7d.max(0.0);
        let latency_score = (-self.latency_delta_7d).max(0.0); // Lower is better
        let knowledge_score = (self.patterns_learned_7d as f32 / 100.0).min(1.0);
        let retention_score = self.retention_rate_7d;
        let creativity_score = (self.novel_connections_7d as f32 / 50.0).min(1.0);

        quality_weight * quality_score +
        latency_weight * latency_score +
        knowledge_weight * knowledge_score +
        retention_weight * retention_score +
        creativity_weight * creativity_score
    }
}
```

---

## Summary

SONA's three-tier learning system enables:

| Loop | Timescale | Purpose | Key Outcome |
|------|-----------|---------|-------------|
| **A** | Per-request | Instant adaptation | Responsive to current context |
| **B** | Hourly | Pattern consolidation | Stable improvement |
| **C** | Weekly | Deep restructuring | Creative breakthroughs |

This mirrors human learning where:
- **Loop A** = Working memory and immediate response
- **Loop B** = Sleep-based consolidation
- **Loop C** = Long-term memory formation and insight

The result is a system that continuously improves at multiple timescales, never forgetting what works while constantly exploring new possibilities.
