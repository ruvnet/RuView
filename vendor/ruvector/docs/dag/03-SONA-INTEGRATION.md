# SONA Integration Specification

## Overview

SONA (Self-Optimizing Neural Architecture) provides the core learning infrastructure for the Neural DAG system. This document specifies how SONA integrates with RuVector-Postgres for continuous query optimization.

## SONA Architecture Review

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           SONA ENGINE                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    INSTANT LOOP (<100μs per query)                   │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  MicroLoRA  │  │ Trajectory  │  │   Auto-     │                  │   │
│  │  │  (rank 1-2) │  │   Buffer    │  │   Flush     │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼ (hourly)                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                  BACKGROUND LOOP (1-5s per cycle)                    │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐  │   │
│  │  │   K-means   │  │  Pattern    │  │   EWC++     │  │  BaseLoRA  │  │   │
│  │  │  Clustering │  │ Extraction  │  │ Constraints │  │  (rank 8)  │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      REASONING BANK                                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   Pattern   │  │  Similarity │  │  Eviction   │                  │   │
│  │  │   Storage   │  │   Search    │  │   Policy    │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Integration Design

### DagSonaEngine Wrapper

```rust
/// Main integration point between DAG system and SONA
pub struct DagSonaEngine {
    /// Core SONA engine (from sona crate)
    inner: SonaEngine,

    /// DAG-specific configuration
    config: DagSonaConfig,

    /// DAG trajectory buffer (specialized for query plans)
    dag_trajectory_buffer: Arc<DagTrajectoryBuffer>,

    /// DAG reasoning bank (pattern storage)
    dag_reasoning_bank: Arc<RwLock<DagReasoningBank>>,

    /// Attention selector integration
    attention_selector: Arc<AttentionSelector>,

    /// MinCut engine integration (optional)
    mincut_engine: Option<Arc<SubpolynomialMinCut>>,

    /// Background worker handle
    worker_handle: Option<JoinHandle<()>>,

    /// Metrics collector
    metrics: Arc<DagSonaMetrics>,
}

impl DagSonaEngine {
    /// Create new engine for a table
    pub fn new(table_name: &str, config: DagSonaConfig) -> Result<Self, SonaError> {
        let inner = SonaEngine::new(SonaConfig {
            hidden_dim: config.hidden_dim,
            micro_lora_rank: config.micro_lora_rank,
            base_lora_rank: config.base_lora_rank,
            ewc_lambda: config.ewc_lambda,
            ..Default::default()
        })?;

        Ok(Self {
            inner,
            config,
            dag_trajectory_buffer: Arc::new(DagTrajectoryBuffer::new(
                config.max_trajectories
            )),
            dag_reasoning_bank: Arc::new(RwLock::new(DagReasoningBank::new(
                config.max_patterns
            ))),
            attention_selector: Arc::new(AttentionSelector::new(
                config.ucb_c,
                config.epsilon,
            )),
            mincut_engine: None,
            worker_handle: None,
            metrics: Arc::new(DagSonaMetrics::new()),
        })
    }

    /// Enable MinCut integration
    pub fn with_mincut(mut self, engine: Arc<SubpolynomialMinCut>) -> Self {
        self.mincut_engine = Some(engine);
        self
    }

    /// Start background learning worker
    pub fn start_background_worker(&mut self) -> Result<(), SonaError> {
        let engine = self.clone_for_worker();
        let interval = self.config.background_interval;

        let handle = std::thread::spawn(move || {
            loop {
                std::thread::sleep(interval);

                if let Err(e) = engine.run_background_cycle() {
                    log::error!("Background learning cycle failed: {}", e);
                }
            }
        });

        self.worker_handle = Some(handle);
        Ok(())
    }
}
```

### DAG-Specific Trajectory

```rust
/// Trajectory specialized for DAG query plans
#[derive(Clone, Debug)]
pub struct DagTrajectory {
    /// Unique trajectory ID
    pub id: u64,

    /// Query plan embedding (256-dim)
    pub plan_embedding: Vec<f32>,

    /// Operator-level embeddings
    pub operator_embeddings: Vec<Vec<f32>>,

    /// Attention weights used
    pub attention_weights: Vec<Vec<f32>>,

    /// Attention type used
    pub attention_type: DagAttentionType,

    /// Execution parameters
    pub params: ExecutionParams,

    /// Execution metrics
    pub metrics: ExecutionMetrics,

    /// Quality score (computed)
    pub quality: f32,

    /// Timestamp
    pub timestamp: SystemTime,
}

#[derive(Clone, Debug)]
pub struct ExecutionParams {
    /// HNSW ef_search if applicable
    pub ef_search: Option<usize>,

    /// IVFFlat probes if applicable
    pub probes: Option<usize>,

    /// Parallelism level
    pub parallelism: usize,

    /// Selected attention type
    pub attention_type: DagAttentionType,

    /// Custom parameters
    pub custom: HashMap<String, f64>,
}

#[derive(Clone, Debug)]
pub struct ExecutionMetrics {
    /// Total execution time (microseconds)
    pub latency_us: u64,

    /// Planning time (microseconds)
    pub planning_us: u64,

    /// Execution time (microseconds)
    pub execution_us: u64,

    /// Rows processed
    pub rows_processed: u64,

    /// Memory used (bytes)
    pub memory_bytes: u64,

    /// Cache hit rate
    pub cache_hit_rate: f32,

    /// MinCut criticality (if computed)
    pub max_criticality: Option<f32>,
}

impl DagTrajectory {
    /// Compute quality score from metrics
    pub fn compute_quality(&mut self) {
        // Multi-objective quality function
        let latency_score = 1.0 / (1.0 + self.metrics.latency_us as f32 / 1000.0);
        let memory_score = 1.0 / (1.0 + self.metrics.memory_bytes as f32 / 1_000_000.0);
        let cache_score = self.metrics.cache_hit_rate;

        // Weighted combination
        self.quality = 0.5 * latency_score + 0.3 * memory_score + 0.2 * cache_score;
    }
}
```

### DAG Trajectory Buffer

```rust
/// Lock-free buffer for DAG trajectories
pub struct DagTrajectoryBuffer {
    /// Lock-free queue
    buffer: ArrayQueue<DagTrajectory>,

    /// Maximum capacity
    capacity: usize,

    /// Dropped count (for monitoring)
    dropped: AtomicU64,

    /// Total seen (for statistics)
    total_seen: AtomicU64,
}

impl DagTrajectoryBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: ArrayQueue::new(capacity),
            capacity,
            dropped: AtomicU64::new(0),
            total_seen: AtomicU64::new(0),
        }
    }

    /// Record a trajectory (non-blocking)
    pub fn record(&self, trajectory: DagTrajectory) -> bool {
        self.total_seen.fetch_add(1, Ordering::Relaxed);

        match self.buffer.push(trajectory) {
            Ok(()) => true,
            Err(_) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    /// Drain all trajectories for learning
    pub fn drain(&self) -> Vec<DagTrajectory> {
        let mut trajectories = Vec::with_capacity(self.capacity);
        while let Some(t) = self.buffer.pop() {
            trajectories.push(t);
        }
        trajectories
    }

    /// Get buffer statistics
    pub fn stats(&self) -> BufferStats {
        BufferStats {
            current_size: self.buffer.len(),
            capacity: self.capacity,
            total_seen: self.total_seen.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
        }
    }
}
```

### DAG Reasoning Bank

```rust
/// Pattern storage specialized for DAG query plans
pub struct DagReasoningBank {
    /// Learned patterns
    patterns: DashMap<PatternId, LearnedDagPattern>,

    /// Pattern index for similarity search
    pattern_index: Vec<(Vec<f32>, PatternId)>,

    /// Maximum patterns
    max_patterns: usize,

    /// Quality threshold for storing
    quality_threshold: f32,

    /// Pattern ID generator
    next_id: AtomicU64,
}

/// Learned pattern for DAG queries
#[derive(Clone, Debug)]
pub struct LearnedDagPattern {
    /// Pattern ID
    pub id: PatternId,

    /// Centroid embedding (cluster center)
    pub centroid: Vec<f32>,

    /// Optimal execution parameters
    pub optimal_params: ExecutionParams,

    /// Optimal attention type
    pub optimal_attention: DagAttentionType,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,

    /// Sample count (trajectories in this cluster)
    pub sample_count: usize,

    /// Average metrics
    pub avg_metrics: AverageMetrics,

    /// Last updated
    pub updated_at: SystemTime,
}

#[derive(Clone, Debug)]
pub struct AverageMetrics {
    pub latency_us: f64,
    pub memory_bytes: f64,
    pub quality: f64,
}

impl DagReasoningBank {
    pub fn new(max_patterns: usize) -> Self {
        Self {
            patterns: DashMap::new(),
            pattern_index: Vec::new(),
            max_patterns,
            quality_threshold: 0.3,
            next_id: AtomicU64::new(1),
        }
    }

    /// Find k most similar patterns
    pub fn find_similar(&self, query: &[f32], k: usize) -> Vec<&LearnedDagPattern> {
        let mut similarities: Vec<(PatternId, f32)> = self.pattern_index.iter()
            .map(|(centroid, id)| (*id, cosine_similarity(query, centroid)))
            .collect();

        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        similarities.into_iter()
            .take(k)
            .filter_map(|(id, _)| self.patterns.get(&id).map(|r| r.value()))
            .collect()
    }

    /// Store a new pattern
    pub fn store(&self, pattern: LearnedDagPattern) -> PatternId {
        // Check capacity and evict if needed
        if self.patterns.len() >= self.max_patterns {
            self.evict_lowest_confidence();
        }

        let id = pattern.id;
        self.patterns.insert(id, pattern.clone());
        self.pattern_index.push((pattern.centroid.clone(), id));

        id
    }

    /// Consolidate similar patterns
    pub fn consolidate(&mut self, similarity_threshold: f32) {
        let mut to_merge: Vec<(PatternId, PatternId)> = Vec::new();

        // Find pairs to merge
        for i in 0..self.pattern_index.len() {
            for j in (i + 1)..self.pattern_index.len() {
                let sim = cosine_similarity(
                    &self.pattern_index[i].0,
                    &self.pattern_index[j].0,
                );
                if sim > similarity_threshold {
                    to_merge.push((self.pattern_index[i].1, self.pattern_index[j].1));
                }
            }
        }

        // Merge patterns
        for (keep_id, remove_id) in to_merge {
            if let (Some(keep), Some(remove)) = (
                self.patterns.get(&keep_id),
                self.patterns.get(&remove_id),
            ) {
                // Merge into keep (weighted average)
                let total_samples = keep.sample_count + remove.sample_count;
                let keep_weight = keep.sample_count as f32 / total_samples as f32;
                let remove_weight = remove.sample_count as f32 / total_samples as f32;

                let merged_centroid: Vec<f32> = keep.centroid.iter()
                    .zip(remove.centroid.iter())
                    .map(|(a, b)| a * keep_weight + b * remove_weight)
                    .collect();

                drop(keep);
                drop(remove);

                if let Some(mut keep) = self.patterns.get_mut(&keep_id) {
                    keep.centroid = merged_centroid;
                    keep.sample_count = total_samples;
                    keep.confidence = (keep.confidence + self.patterns.get(&remove_id)
                        .map(|r| r.confidence).unwrap_or(0.0)) / 2.0;
                }

                self.patterns.remove(&remove_id);
            }
        }

        // Rebuild index
        self.rebuild_index();
    }

    fn evict_lowest_confidence(&self) {
        if let Some(min_entry) = self.patterns.iter()
            .min_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(Ordering::Equal))
        {
            let id = *min_entry.key();
            drop(min_entry);
            self.patterns.remove(&id);
        }
    }

    fn rebuild_index(&mut self) {
        self.pattern_index = self.patterns.iter()
            .map(|entry| (entry.centroid.clone(), *entry.key()))
            .collect();
    }
}
```

## Instant Loop Integration

### Per-Query Flow

```rust
impl DagSonaEngine {
    /// Called before query execution
    pub fn pre_query(&self, plan: &mut NeuralDagPlan) -> PreQueryResult {
        let start = Instant::now();

        // 1. Embed the query plan
        let plan_embedding = self.embed_plan(plan);

        // 2. Find similar patterns
        let similar = {
            let bank = self.dag_reasoning_bank.read();
            bank.find_similar(&plan_embedding, 5)
        };

        // 3. Check for high-confidence match
        if let Some(best) = similar.first() {
            if best.confidence > 0.8 {
                // Apply learned configuration
                plan.params = best.optimal_params.clone();
                plan.attention_type = best.optimal_attention.clone();

                self.metrics.pattern_hits.fetch_add(1, Ordering::Relaxed);

                return PreQueryResult::PatternApplied {
                    pattern_id: best.id,
                    confidence: best.confidence,
                    planning_time: start.elapsed(),
                };
            }
        }

        // 4. No good pattern - use defaults with micro-LoRA adaptation
        let adapted_costs = self.inner.apply_micro_lora(&plan_embedding);
        plan.learned_costs = Some(adapted_costs);

        // 5. Select attention type via UCB bandit
        let attention_type = self.attention_selector.select(&plan_embedding);
        plan.attention_type = attention_type;

        self.metrics.pattern_misses.fetch_add(1, Ordering::Relaxed);

        PreQueryResult::DefaultWithAdaptation {
            attention_type,
            planning_time: start.elapsed(),
        }
    }

    /// Called after query execution
    pub fn post_query(&self, plan: &NeuralDagPlan, metrics: ExecutionMetrics) {
        // 1. Build trajectory
        let mut trajectory = DagTrajectory {
            id: self.generate_trajectory_id(),
            plan_embedding: self.embed_plan(plan),
            operator_embeddings: plan.operator_embeddings.clone(),
            attention_weights: plan.attention_weights.clone(),
            attention_type: plan.attention_type.clone(),
            params: plan.params.clone(),
            metrics: metrics.clone(),
            quality: 0.0,
            timestamp: SystemTime::now(),
        };

        // 2. Compute quality
        trajectory.compute_quality();

        // 3. Record trajectory (non-blocking)
        self.dag_trajectory_buffer.record(trajectory.clone());

        // 4. Instant learning signal
        let signal = LearningSignal::from_dag_trajectory(&trajectory);
        self.inner.instant_loop().on_signal(signal);

        // 5. Update attention selector
        self.attention_selector.update(
            &trajectory.plan_embedding,
            trajectory.attention_type.clone(),
            trajectory.quality,
        );

        // 6. Update metrics
        self.metrics.queries_processed.fetch_add(1, Ordering::Relaxed);
        self.metrics.total_latency_us.fetch_add(metrics.latency_us, Ordering::Relaxed);
    }

    /// Embed a query plan into vector space
    fn embed_plan(&self, plan: &NeuralDagPlan) -> Vec<f32> {
        // Combine operator embeddings with attention
        let mut embedding = vec![0.0; self.config.hidden_dim];

        for (i, op_emb) in plan.operator_embeddings.iter().enumerate() {
            let weight = 1.0 / (i + 1) as f32;  // Decay by position
            for (j, &val) in op_emb.iter().enumerate() {
                if j < embedding.len() {
                    embedding[j] += weight * val;
                }
            }
        }

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        embedding
    }
}
```

## Background Loop Integration

### Learning Cycle

```rust
impl DagSonaEngine {
    /// Run one background learning cycle
    pub fn run_background_cycle(&self) -> Result<BackgroundCycleResult, SonaError> {
        let start = Instant::now();

        // 1. Drain trajectory buffer
        let trajectories = self.dag_trajectory_buffer.drain();
        if trajectories.len() < self.config.min_trajectories {
            return Ok(BackgroundCycleResult::Skipped {
                reason: "Insufficient trajectories".to_string(),
                count: trajectories.len(),
            });
        }

        // 2. Filter by quality threshold
        let quality_trajectories: Vec<_> = trajectories.into_iter()
            .filter(|t| t.quality >= self.config.quality_threshold)
            .collect();

        // 3. K-means++ clustering
        let patterns = self.extract_patterns(&quality_trajectories)?;

        // 4. Apply EWC++ constraints
        let gradients = self.compute_pattern_gradients(&patterns);
        let constrained_gradients = self.inner.ewc().apply_constraints(&gradients);

        // 5. Check for task boundary
        if self.inner.ewc().detect_task_boundary(&gradients) {
            self.inner.ewc_mut().start_new_task();
            log::info!("Detected task boundary, starting new EWC task");
        }

        // 6. Update Fisher information
        self.inner.ewc_mut().update_fisher(&constrained_gradients);

        // 7. Update BaseLoRA
        self.inner.base_lora_mut().update(&constrained_gradients, self.config.base_lora_lr);

        // 8. Store patterns in ReasoningBank
        let stored_count = {
            let mut bank = self.dag_reasoning_bank.write();
            let mut count = 0;
            for pattern in patterns {
                if pattern.confidence >= self.config.pattern_confidence_threshold {
                    bank.store(pattern);
                    count += 1;
                }
            }
            count
        };

        // 9. Consolidate similar patterns periodically
        if self.should_consolidate() {
            let mut bank = self.dag_reasoning_bank.write();
            bank.consolidate(self.config.consolidation_threshold);
        }

        let result = BackgroundCycleResult::Completed {
            trajectories_processed: quality_trajectories.len(),
            patterns_extracted: stored_count,
            duration: start.elapsed(),
        };

        self.metrics.background_cycles.fetch_add(1, Ordering::Relaxed);

        Ok(result)
    }

    /// K-means++ pattern extraction
    fn extract_patterns(&self, trajectories: &[DagTrajectory]) -> Result<Vec<LearnedDagPattern>, SonaError> {
        let k = self.config.pattern_clusters.min(trajectories.len());
        if k == 0 {
            return Ok(Vec::new());
        }

        // Extract embeddings
        let embeddings: Vec<Vec<f32>> = trajectories.iter()
            .map(|t| t.plan_embedding.clone())
            .collect();

        // K-means++ initialization
        let mut centroids = self.kmeans_plusplus_init(&embeddings, k);

        // K-means iterations
        for _ in 0..self.config.kmeans_max_iterations {
            // Assign points to clusters
            let assignments: Vec<usize> = embeddings.iter()
                .map(|e| self.nearest_centroid(e, &centroids))
                .collect();

            // Update centroids
            let mut new_centroids = vec![vec![0.0; self.config.hidden_dim]; k];
            let mut counts = vec![0usize; k];

            for (i, embedding) in embeddings.iter().enumerate() {
                let cluster = assignments[i];
                counts[cluster] += 1;
                for (j, &val) in embedding.iter().enumerate() {
                    new_centroids[cluster][j] += val;
                }
            }

            for (i, centroid) in new_centroids.iter_mut().enumerate() {
                if counts[i] > 0 {
                    for val in centroid.iter_mut() {
                        *val /= counts[i] as f32;
                    }
                }
            }

            // Check convergence
            let max_shift: f32 = centroids.iter()
                .zip(new_centroids.iter())
                .map(|(old, new)| euclidean_distance(old, new))
                .fold(0.0, f32::max);

            centroids = new_centroids;

            if max_shift < self.config.kmeans_convergence {
                break;
            }
        }

        // Build patterns from clusters
        let assignments: Vec<usize> = embeddings.iter()
            .map(|e| self.nearest_centroid(e, &centroids))
            .collect();

        let mut patterns = Vec::with_capacity(k);

        for (cluster_idx, centroid) in centroids.into_iter().enumerate() {
            let members: Vec<&DagTrajectory> = trajectories.iter()
                .enumerate()
                .filter(|(i, _)| assignments[*i] == cluster_idx)
                .map(|(_, t)| t)
                .collect();

            if members.is_empty() {
                continue;
            }

            // Compute optimal parameters (mode of discrete, mean of continuous)
            let optimal_attention = self.mode_attention_type(&members);
            let optimal_params = self.average_params(&members);

            // Compute average metrics
            let avg_metrics = AverageMetrics {
                latency_us: members.iter().map(|t| t.metrics.latency_us as f64).sum::<f64>() / members.len() as f64,
                memory_bytes: members.iter().map(|t| t.metrics.memory_bytes as f64).sum::<f64>() / members.len() as f64,
                quality: members.iter().map(|t| t.quality as f64).sum::<f64>() / members.len() as f64,
            };

            // Confidence based on sample count and quality variance
            let quality_variance = self.compute_variance(members.iter().map(|t| t.quality as f64));
            let confidence = (1.0 - quality_variance.sqrt()).max(0.0) *
                (1.0 - 1.0 / (members.len() as f32 + 1.0));

            patterns.push(LearnedDagPattern {
                id: self.generate_pattern_id(),
                centroid,
                optimal_params,
                optimal_attention,
                confidence,
                sample_count: members.len(),
                avg_metrics,
                updated_at: SystemTime::now(),
            });
        }

        Ok(patterns)
    }

    fn kmeans_plusplus_init(&self, embeddings: &[Vec<f32>], k: usize) -> Vec<Vec<f32>> {
        let mut centroids = Vec::with_capacity(k);

        // First centroid: deterministic (index 0)
        centroids.push(embeddings[0].clone());

        // Remaining centroids: D² weighted
        for _ in 1..k {
            let distances: Vec<f32> = embeddings.iter()
                .map(|e| {
                    centroids.iter()
                        .map(|c| euclidean_distance(e, c))
                        .fold(f32::INFINITY, f32::min)
                })
                .collect();

            let sum: f32 = distances.iter().map(|d| d * d).sum();
            let threshold = rand::random::<f32>() * sum;

            let mut cumsum = 0.0;
            for (i, &d) in distances.iter().enumerate() {
                cumsum += d * d;
                if cumsum >= threshold {
                    centroids.push(embeddings[i].clone());
                    break;
                }
            }
        }

        centroids
    }
}
```

## EWC++ Integration

### Configuration

```rust
pub struct EwcConfig {
    /// Initial regularization strength
    pub lambda: f32,                    // 2000.0

    /// Maximum lambda after adaptation
    pub max_lambda: f32,                // 15000.0

    /// Minimum lambda
    pub min_lambda: f32,                // 100.0

    /// Fisher information EMA decay
    pub fisher_decay: f32,              // 0.999

    /// Task boundary detection threshold (z-score)
    pub boundary_threshold: f32,        // 2.0

    /// Maximum remembered tasks
    pub max_tasks: usize,               // 10

    /// Gradient history size for boundary detection
    pub gradient_history_size: usize,   // 100
}
```

### Task Boundary Detection

```rust
impl DagSonaEngine {
    /// Detect if query patterns have shifted significantly
    fn detect_pattern_shift(&self, recent_trajectories: &[DagTrajectory]) -> bool {
        if recent_trajectories.len() < 50 {
            return false;
        }

        // Compute embedding distribution statistics
        let embeddings: Vec<&Vec<f32>> = recent_trajectories.iter()
            .map(|t| &t.plan_embedding)
            .collect();

        let mean = self.compute_mean_embedding(&embeddings);
        let variance = self.compute_embedding_variance(&embeddings, &mean);

        // Compare with historical statistics
        let historical_mean = self.get_historical_mean();
        let historical_variance = self.get_historical_variance();

        // Z-score test
        let diff_norm = euclidean_distance(&mean, &historical_mean);
        let std = (historical_variance + variance).sqrt() / 2.0;

        let z_score = diff_norm / std;

        z_score > self.config.ewc_boundary_threshold
    }
}
```

## MicroLoRA Integration

### Per-Query Adaptation

```rust
pub struct DagMicroLoRA {
    /// Down projection: hidden_dim × rank
    down_proj: Vec<f32>,

    /// Up projection: rank × hidden_dim
    up_proj: Vec<f32>,

    /// Rank (1-2 for efficiency)
    rank: usize,

    /// Hidden dimension
    hidden_dim: usize,

    /// Accumulated gradients
    grad_down: Vec<f32>,
    grad_up: Vec<f32>,

    /// Update count for averaging
    update_count: usize,

    /// Scale factor: 1.0 / sqrt(rank)
    scale: f32,
}

impl DagMicroLoRA {
    /// Apply LoRA to plan embedding
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        let mut output = input.to_vec();

        // Down projection: input → intermediate
        let mut intermediate = vec![0.0; self.rank];
        for r in 0..self.rank {
            for i in 0..self.hidden_dim {
                intermediate[r] += input[i] * self.down_proj[r * self.hidden_dim + i];
            }
        }

        // Up projection: intermediate → output delta
        for i in 0..self.hidden_dim {
            let mut delta = 0.0;
            for r in 0..self.rank {
                delta += intermediate[r] * self.up_proj[r * self.hidden_dim + i];
            }
            output[i] += self.scale * delta;
        }

        output
    }

    /// Accumulate gradient from learning signal
    pub fn accumulate_gradient(&mut self, signal: &LearningSignal) {
        // Simplified REINFORCE-style gradient
        let quality = signal.quality_score;

        for r in 0..self.rank {
            for i in 0..self.hidden_dim {
                self.grad_up[r * self.hidden_dim + i] +=
                    signal.gradient_estimate[i] * quality;
            }
        }

        self.update_count += 1;
    }

    /// Apply accumulated gradients
    pub fn apply_accumulated(&mut self, learning_rate: f32) {
        if self.update_count == 0 {
            return;
        }

        let scale = learning_rate / self.update_count as f32;

        for (w, g) in self.up_proj.iter_mut().zip(self.grad_up.iter()) {
            *w += g * scale;
        }

        // Reset accumulators
        self.grad_up.fill(0.0);
        self.grad_down.fill(0.0);
        self.update_count = 0;
    }
}
```

## Metrics and Monitoring

```rust
pub struct DagSonaMetrics {
    /// Total queries processed
    pub queries_processed: AtomicU64,

    /// Pattern cache hits
    pub pattern_hits: AtomicU64,

    /// Pattern cache misses
    pub pattern_misses: AtomicU64,

    /// Total latency (microseconds)
    pub total_latency_us: AtomicU64,

    /// Background cycles completed
    pub background_cycles: AtomicU64,

    /// Patterns currently stored
    pub patterns_stored: AtomicU64,

    /// Trajectories dropped (buffer full)
    pub trajectories_dropped: AtomicU64,

    /// EWC tasks
    pub ewc_tasks: AtomicU64,
}

impl DagSonaMetrics {
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "queries_processed": self.queries_processed.load(Ordering::Relaxed),
            "pattern_hit_rate": self.hit_rate(),
            "avg_latency_us": self.avg_latency(),
            "background_cycles": self.background_cycles.load(Ordering::Relaxed),
            "patterns_stored": self.patterns_stored.load(Ordering::Relaxed),
            "trajectories_dropped": self.trajectories_dropped.load(Ordering::Relaxed),
            "ewc_tasks": self.ewc_tasks.load(Ordering::Relaxed),
        })
    }

    fn hit_rate(&self) -> f64 {
        let hits = self.pattern_hits.load(Ordering::Relaxed) as f64;
        let misses = self.pattern_misses.load(Ordering::Relaxed) as f64;
        if hits + misses > 0.0 {
            hits / (hits + misses)
        } else {
            0.0
        }
    }

    fn avg_latency(&self) -> f64 {
        let total = self.total_latency_us.load(Ordering::Relaxed) as f64;
        let count = self.queries_processed.load(Ordering::Relaxed) as f64;
        if count > 0.0 {
            total / count
        } else {
            0.0
        }
    }
}
```

## Configuration Summary

| Parameter | Default | Description |
|-----------|---------|-------------|
| `hidden_dim` | 256 | Embedding dimension |
| `micro_lora_rank` | 2 | MicroLoRA rank |
| `micro_lora_lr` | 0.002 | MicroLoRA learning rate |
| `base_lora_rank` | 8 | BaseLoRA rank |
| `base_lora_lr` | 0.001 | BaseLoRA learning rate |
| `max_trajectories` | 10000 | Trajectory buffer size |
| `min_trajectories` | 100 | Min trajectories for learning |
| `pattern_clusters` | 100 | K-means cluster count |
| `quality_threshold` | 0.3 | Min quality for learning |
| `pattern_confidence_threshold` | 0.5 | Min confidence to store |
| `consolidation_threshold` | 0.95 | Similarity for merging |
| `ewc_lambda` | 2000.0 | EWC regularization |
| `ewc_max_lambda` | 15000.0 | Max EWC lambda |
| `ewc_boundary_threshold` | 2.0 | Task boundary z-score |
| `background_interval` | 1 hour | Learning cycle interval |
| `kmeans_max_iterations` | 100 | K-means iterations |
| `kmeans_convergence` | 0.001 | K-means convergence threshold |
