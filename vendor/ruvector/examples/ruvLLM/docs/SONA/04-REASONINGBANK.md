# SONA ReasoningBank: Pattern-Driven Self-Optimization

## Learning from Experience Through Trajectory Analysis

---

## 1. Overview

ReasoningBank is SONA's long-term pattern memory, learning what works and applying that knowledge to optimize future decisions.

```
┌─────────────────────────────────────────────────────────────────────┐
│                      REASONINGBANK CONCEPT                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│    Query → [What worked before?] → Pattern Match → Optimized Params │
│                      ↑                                              │
│                      │                                              │
│              ┌───────┴────────┐                                     │
│              │ REASONINGBANK  │                                     │
│              │                │                                     │
│              │ • Trajectories │  ← Record every query               │
│              │ • Patterns     │  ← Extract from clusters            │
│              │ • Verdicts     │  ← What params worked best          │
│              │ • Confidence   │  ← How certain we are               │
│              └────────────────┘                                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Core Data Structures

### Trajectory: Recording Every Interaction

```rust
/// A single query trajectory with outcomes
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryTrajectory {
    /// Unique trajectory ID
    pub id: u64,
    /// Query embedding vector
    pub query_embedding: Vec<f32>,
    /// Search parameters used
    pub search_params: SearchParams,
    /// Retrieved result IDs
    pub retrieved_ids: Vec<String>,
    /// Precision (relevant / retrieved)
    pub precision: f32,
    /// Recall (retrieved_relevant / total_relevant)
    pub recall: f32,
    /// Latency in microseconds
    pub latency_us: u64,
    /// User feedback if provided
    pub feedback: Option<UserFeedback>,
    /// Timestamp
    pub timestamp: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchParams {
    /// ef_search parameter for HNSW
    pub ef_search: usize,
    /// Number of probes for IVF
    pub n_probes: usize,
    /// Model tier selected
    pub model_tier: ModelTier,
    /// Context window size
    pub context_tokens: usize,
    /// Temperature
    pub temperature: f32,
}
```

### Pattern: Learned Behavior Clusters

```rust
/// A learned pattern extracted from trajectory clusters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearnedPattern {
    /// Pattern ID
    pub id: u64,
    /// Centroid embedding (cluster center)
    pub centroid: Vec<f32>,
    /// Optimal search parameters for this pattern
    pub optimal_params: SearchParams,
    /// Confidence score (0-1)
    pub confidence: f32,
    /// Number of trajectories in cluster
    pub support_count: usize,
    /// Average precision for pattern
    pub avg_precision: f32,
    /// Average recall for pattern
    pub avg_recall: f32,
    /// Average latency
    pub avg_latency_us: u64,
    /// Pattern creation timestamp
    pub created_at: i64,
    /// Last update timestamp
    pub updated_at: i64,
    /// Abstraction level (0 = concrete, higher = more abstract)
    pub abstraction_level: u32,
    /// Child pattern IDs (for hierarchical patterns)
    pub children: Vec<u64>,
}
```

### Verdict: Decision Judgments

```rust
/// Verdict on what parameters worked best
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Verdict {
    /// Pattern this verdict applies to
    pub pattern_id: u64,
    /// Recommended parameters
    pub recommended_params: SearchParams,
    /// Confidence in recommendation
    pub confidence: f32,
    /// Evidence supporting this verdict
    pub evidence: VerdictEvidence,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerdictEvidence {
    /// Number of supporting trajectories
    pub support_count: usize,
    /// Average improvement over default
    pub avg_improvement: f32,
    /// Statistical significance (p-value)
    pub p_value: f32,
    /// Consistency score (low variance = high consistency)
    pub consistency: f32,
}
```

---

## 3. ReasoningBank Implementation

### Core Storage and Retrieval

```rust
use dashmap::DashMap;
use parking_lot::RwLock;

/// ReasoningBank: Pattern-based learning and optimization
pub struct ReasoningBank {
    /// Trajectory ring buffer (recent interactions)
    trajectories: RwLock<CircularBuffer<QueryTrajectory>>,
    /// Learned patterns (concurrent hashmap)
    patterns: DashMap<u64, LearnedPattern>,
    /// Pattern index for fast similarity lookup
    pattern_index: RwLock<HNSWIndex>,
    /// Verdicts per pattern
    verdicts: DashMap<u64, Verdict>,
    /// Configuration
    config: ReasoningBankConfig,
    /// Pattern ID counter
    next_pattern_id: AtomicU64,
    /// Statistics
    stats: RwLock<ReasoningBankStats>,
}

impl ReasoningBank {
    /// Create new ReasoningBank
    pub fn new(config: ReasoningBankConfig) -> Self {
        Self {
            trajectories: RwLock::new(CircularBuffer::new(config.trajectory_capacity)),
            patterns: DashMap::new(),
            pattern_index: RwLock::new(HNSWIndex::new(config.embedding_dim, config.ef_construction)),
            verdicts: DashMap::new(),
            config,
            next_pattern_id: AtomicU64::new(0),
            stats: RwLock::new(ReasoningBankStats::default()),
        }
    }

    /// Record a new trajectory
    #[inline]
    pub fn record_trajectory(&self, trajectory: QueryTrajectory) {
        let mut trajectories = self.trajectories.write();
        trajectories.push(trajectory);

        // Update stats
        let mut stats = self.stats.write();
        stats.total_trajectories += 1;
    }

    /// Find most similar pattern to query
    pub fn find_similar_pattern(&self, query_embedding: &[f32], k: usize) -> Vec<PatternMatch> {
        let index = self.pattern_index.read();
        let neighbors = index.search(query_embedding, k, self.config.ef_search);

        neighbors.iter()
            .filter_map(|&(id, distance)| {
                self.patterns.get(&id).map(|p| PatternMatch {
                    pattern: p.clone(),
                    similarity: 1.0 - distance, // Convert distance to similarity
                })
            })
            .collect()
    }

    /// Get optimized parameters for query
    pub fn get_optimized_params(&self, query_embedding: &[f32]) -> OptimizedParams {
        // Find similar patterns
        let matches = self.find_similar_pattern(query_embedding, self.config.top_k_patterns);

        if matches.is_empty() {
            // No matching patterns - use defaults
            return OptimizedParams {
                params: SearchParams::default(),
                confidence: 0.0,
                source: ParamSource::Default,
            };
        }

        // Interpolate parameters based on similarity and confidence
        let mut weighted_params = SearchParams::default();
        let mut total_weight = 0.0f32;

        for m in &matches {
            let weight = m.similarity * m.pattern.confidence;
            total_weight += weight;

            weighted_params.ef_search += (m.pattern.optimal_params.ef_search as f32 * weight) as usize;
            weighted_params.n_probes += (m.pattern.optimal_params.n_probes as f32 * weight) as usize;
            weighted_params.temperature += m.pattern.optimal_params.temperature * weight;
            // ... other params
        }

        if total_weight > 0.0 {
            weighted_params.ef_search = (weighted_params.ef_search as f32 / total_weight) as usize;
            weighted_params.n_probes = (weighted_params.n_probes as f32 / total_weight) as usize;
            weighted_params.temperature /= total_weight;
        }

        OptimizedParams {
            params: weighted_params,
            confidence: total_weight / matches.len() as f32,
            source: ParamSource::Pattern(matches[0].pattern.id),
        }
    }

    /// Record feedback for trajectory
    pub fn record_feedback(&self, trajectory_id: u64, feedback: UserFeedback) {
        // Find trajectory and update
        let mut trajectories = self.trajectories.write();
        if let Some(traj) = trajectories.iter_mut().find(|t| t.id == trajectory_id) {
            traj.feedback = Some(feedback.clone());
        }

        // Update related pattern confidence
        // Higher feedback = higher confidence in that pattern's params
        if let Some(pattern_id) = self.find_pattern_for_trajectory(trajectory_id) {
            if let Some(mut pattern) = self.patterns.get_mut(&pattern_id) {
                let feedback_delta = feedback.rating as f32 / 5.0 - 0.5; // -0.5 to +0.5
                pattern.confidence = (pattern.confidence + 0.1 * feedback_delta).clamp(0.0, 1.0);
            }
        }
    }
}
```

---

## 4. Pattern Extraction

### K-Means++ Clustering for Pattern Discovery

```rust
/// Pattern extractor using K-means++ clustering
pub struct PatternExtractor {
    /// Number of clusters to extract
    k: usize,
    /// Maximum iterations
    max_iter: usize,
    /// Convergence threshold
    epsilon: f32,
}

impl PatternExtractor {
    /// Extract patterns from trajectories
    pub fn extract(&self, trajectories: &[QueryTrajectory]) -> Vec<LearnedPattern> {
        if trajectories.len() < self.k {
            return Vec::new();
        }

        // Collect embeddings
        let embeddings: Vec<&[f32]> = trajectories.iter()
            .map(|t| t.query_embedding.as_slice())
            .collect();

        // K-means++ initialization
        let mut centroids = self.kmeans_plus_plus_init(&embeddings);

        // K-means iteration
        let mut assignments = vec![0usize; trajectories.len()];
        for _ in 0..self.max_iter {
            // Assignment step
            let old_assignments = assignments.clone();
            for (i, emb) in embeddings.iter().enumerate() {
                let mut min_dist = f32::MAX;
                let mut min_idx = 0;
                for (c_idx, centroid) in centroids.iter().enumerate() {
                    let dist = euclidean_distance(emb, centroid);
                    if dist < min_dist {
                        min_dist = dist;
                        min_idx = c_idx;
                    }
                }
                assignments[i] = min_idx;
            }

            // Check convergence
            if assignments == old_assignments {
                break;
            }

            // Update step
            centroids = self.compute_centroids(&embeddings, &assignments);
        }

        // Create patterns from clusters
        let mut patterns = Vec::new();
        for cluster_id in 0..self.k {
            let cluster_trajectories: Vec<_> = trajectories.iter()
                .zip(assignments.iter())
                .filter(|(_, &a)| a == cluster_id)
                .map(|(t, _)| t)
                .collect();

            if cluster_trajectories.len() < 3 {
                continue; // Skip small clusters
            }

            let pattern = self.create_pattern_from_cluster(
                cluster_id as u64,
                &centroids[cluster_id],
                &cluster_trajectories,
            );
            patterns.push(pattern);
        }

        patterns
    }

    fn kmeans_plus_plus_init(&self, embeddings: &[&[f32]]) -> Vec<Vec<f32>> {
        let mut centroids = Vec::with_capacity(self.k);
        let mut rng = rand::thread_rng();

        // First centroid: random
        let first_idx = rng.gen_range(0..embeddings.len());
        centroids.push(embeddings[first_idx].to_vec());

        // Remaining centroids: D² weighting
        for _ in 1..self.k {
            let mut distances: Vec<f32> = embeddings.iter()
                .map(|emb| {
                    centroids.iter()
                        .map(|c| euclidean_distance(emb, c))
                        .fold(f32::MAX, f32::min)
                })
                .collect();

            // Square distances for D² sampling
            let total: f32 = distances.iter().map(|d| d * d).sum();
            let threshold = rng.gen::<f32>() * total;

            let mut cumsum = 0.0;
            let mut selected = 0;
            for (i, d) in distances.iter().enumerate() {
                cumsum += d * d;
                if cumsum >= threshold {
                    selected = i;
                    break;
                }
            }

            centroids.push(embeddings[selected].to_vec());
        }

        centroids
    }

    fn create_pattern_from_cluster(
        &self,
        id: u64,
        centroid: &[f32],
        trajectories: &[&QueryTrajectory],
    ) -> LearnedPattern {
        // Compute optimal params as weighted average by quality
        let mut total_weight = 0.0f32;
        let mut ef_sum = 0.0f32;
        let mut probes_sum = 0.0f32;
        let mut temp_sum = 0.0f32;
        let mut precision_sum = 0.0f32;
        let mut recall_sum = 0.0f32;
        let mut latency_sum = 0u64;

        for t in trajectories {
            let weight = t.precision * t.recall; // Quality as weight
            total_weight += weight;

            ef_sum += t.search_params.ef_search as f32 * weight;
            probes_sum += t.search_params.n_probes as f32 * weight;
            temp_sum += t.search_params.temperature * weight;
            precision_sum += t.precision;
            recall_sum += t.recall;
            latency_sum += t.latency_us;
        }

        let n = trajectories.len() as f32;

        LearnedPattern {
            id,
            centroid: centroid.to_vec(),
            optimal_params: SearchParams {
                ef_search: (ef_sum / total_weight).round() as usize,
                n_probes: (probes_sum / total_weight).round() as usize,
                model_tier: ModelTier::Auto, // Determined separately
                context_tokens: 2048, // Default
                temperature: temp_sum / total_weight,
            },
            confidence: (total_weight / n).clamp(0.0, 1.0),
            support_count: trajectories.len(),
            avg_precision: precision_sum / n,
            avg_recall: recall_sum / n,
            avg_latency_us: latency_sum / trajectories.len() as u64,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
            abstraction_level: 0,
            children: Vec::new(),
        }
    }
}
```

---

## 5. Verdict Judgment System

### Evaluating What Works Best

```rust
/// Verdict judge for parameter optimization
pub struct VerdictJudge {
    /// Minimum samples for statistical significance
    min_samples: usize,
    /// Significance level (p-value threshold)
    alpha: f32,
}

impl VerdictJudge {
    /// Judge optimal parameters for a pattern
    pub fn judge(&self, pattern: &LearnedPattern, trajectories: &[&QueryTrajectory]) -> Option<Verdict> {
        if trajectories.len() < self.min_samples {
            return None; // Not enough evidence
        }

        // Group trajectories by parameter configuration
        let mut param_groups: HashMap<ParamKey, Vec<&QueryTrajectory>> = HashMap::new();
        for t in trajectories {
            let key = ParamKey::from(&t.search_params);
            param_groups.entry(key).or_default().push(t);
        }

        // Find best performing configuration
        let mut best_config: Option<(ParamKey, f32, Vec<&QueryTrajectory>)> = None;

        for (key, group) in &param_groups {
            if group.len() < 3 {
                continue;
            }

            // Compute quality score (F1 of precision and recall)
            let avg_quality: f32 = group.iter()
                .map(|t| 2.0 * t.precision * t.recall / (t.precision + t.recall + 1e-6))
                .sum::<f32>() / group.len() as f32;

            match &best_config {
                None => best_config = Some((key.clone(), avg_quality, group.clone())),
                Some((_, best_quality, _)) if avg_quality > *best_quality => {
                    best_config = Some((key.clone(), avg_quality, group.clone()));
                }
                _ => {}
            }
        }

        let (best_key, best_quality, best_group) = best_config?;

        // Statistical significance test
        let p_value = self.compute_significance(&best_group, trajectories);
        if p_value > self.alpha {
            return None; // Not significant
        }

        // Compute consistency (inverse of coefficient of variation)
        let qualities: Vec<f32> = best_group.iter()
            .map(|t| 2.0 * t.precision * t.recall / (t.precision + t.recall + 1e-6))
            .collect();
        let mean = qualities.iter().sum::<f32>() / qualities.len() as f32;
        let variance = qualities.iter()
            .map(|q| (q - mean).powi(2))
            .sum::<f32>() / qualities.len() as f32;
        let std_dev = variance.sqrt();
        let consistency = 1.0 / (1.0 + std_dev / mean);

        // Compute improvement over default
        let default_quality = self.compute_default_quality(trajectories);
        let improvement = (best_quality - default_quality) / default_quality;

        Some(Verdict {
            pattern_id: pattern.id,
            recommended_params: best_key.to_params(),
            confidence: best_quality * consistency,
            evidence: VerdictEvidence {
                support_count: best_group.len(),
                avg_improvement: improvement,
                p_value,
                consistency,
            },
        })
    }

    fn compute_significance(&self, best: &[&QueryTrajectory], all: &[&QueryTrajectory]) -> f32 {
        // Welch's t-test for comparing means
        let best_qualities: Vec<f32> = best.iter()
            .map(|t| t.precision * t.recall)
            .collect();
        let all_qualities: Vec<f32> = all.iter()
            .map(|t| t.precision * t.recall)
            .collect();

        welch_t_test(&best_qualities, &all_qualities)
    }

    fn compute_default_quality(&self, trajectories: &[&QueryTrajectory]) -> f32 {
        // Assume first configuration or most common is "default"
        let default_group: Vec<_> = trajectories.iter()
            .filter(|t| t.search_params.ef_search == SearchParams::default().ef_search)
            .collect();

        if default_group.is_empty() {
            0.5 // Baseline assumption
        } else {
            default_group.iter()
                .map(|t| t.precision * t.recall)
                .sum::<f32>() / default_group.len() as f32
        }
    }
}
```

---

## 6. Integration with Router

### Using ReasoningBank to Optimize Router Decisions

```rust
impl FastGRNNRouter {
    /// Forward pass with ReasoningBank optimization
    pub fn forward_with_reasoning(
        &self,
        features: &[f32],
        reasoning_bank: &ReasoningBank,
    ) -> RouterDecision {
        // Get pattern-based parameter suggestions
        let pattern_params = reasoning_bank.get_optimized_params(features);

        // Standard router forward
        let mut decision = self.forward(features);

        // Blend router decision with pattern suggestions
        if pattern_params.confidence > 0.5 {
            let blend_factor = pattern_params.confidence * 0.3; // Max 30% influence

            // Interpolate temperature
            decision.temperature = (1.0 - blend_factor) * decision.temperature
                + blend_factor * pattern_params.params.temperature;

            // Context token suggestion influences context selection
            let suggested_context = pattern_params.params.context_tokens;
            let router_context = decision.context_tokens;
            decision.context_tokens = ((1.0 - blend_factor) * router_context as f32
                + blend_factor * suggested_context as f32) as usize;

            decision.reasoning_confidence = pattern_params.confidence;
            decision.reasoning_pattern_id = pattern_params.source.pattern_id();
        }

        decision
    }
}
```

---

## 7. Pattern Consolidation and Pruning

### Managing Pattern Memory

```rust
impl ReasoningBank {
    /// Consolidate similar patterns
    pub fn consolidate_patterns(&mut self) {
        // Find similar pattern pairs
        let pattern_ids: Vec<u64> = self.patterns.iter()
            .map(|p| *p.key())
            .collect();

        let mut to_merge: Vec<(u64, u64)> = Vec::new();

        for i in 0..pattern_ids.len() {
            for j in (i+1)..pattern_ids.len() {
                let p1 = self.patterns.get(&pattern_ids[i]).unwrap();
                let p2 = self.patterns.get(&pattern_ids[j]).unwrap();

                let similarity = cosine_similarity(&p1.centroid, &p2.centroid);
                if similarity > 0.95 {
                    // Very similar - merge
                    to_merge.push((pattern_ids[i], pattern_ids[j]));
                }
            }
        }

        // Merge patterns
        for (keep_id, remove_id) in to_merge {
            if let (Some(mut keep), Some(remove)) = (
                self.patterns.get_mut(&keep_id),
                self.patterns.get(&remove_id)
            ) {
                // Weighted average of centroids
                let total_support = keep.support_count + remove.support_count;
                let w1 = keep.support_count as f32 / total_support as f32;
                let w2 = remove.support_count as f32 / total_support as f32;

                for (c, (c1, c2)) in keep.centroid.iter_mut()
                    .zip(keep.centroid.iter().zip(remove.centroid.iter()))
                {
                    *c = w1 * c1 + w2 * c2;
                }

                // Update support count
                keep.support_count = total_support;
                keep.confidence = (keep.confidence * w1 + remove.confidence * w2).min(1.0);
                keep.updated_at = chrono::Utc::now().timestamp();
            }

            // Remove merged pattern
            self.patterns.remove(&remove_id);
        }
    }

    /// Prune low-confidence patterns
    pub fn prune_patterns(&mut self, min_confidence: f32, min_support: usize) {
        let to_remove: Vec<u64> = self.patterns.iter()
            .filter(|p| p.confidence < min_confidence || p.support_count < min_support)
            .map(|p| *p.key())
            .collect();

        for id in to_remove {
            self.patterns.remove(&id);
            self.verdicts.remove(&id);
        }
    }

    /// Build pattern hierarchy (abstraction levels)
    pub fn build_hierarchy(&mut self) {
        // Hierarchical clustering on existing patterns
        let patterns: Vec<_> = self.patterns.iter()
            .map(|p| (p.key().clone(), p.centroid.clone()))
            .collect();

        let hierarchy = HierarchicalClustering::new()
            .linkage(Linkage::Ward)
            .fit(&patterns);

        // Create meta-patterns at each level
        for level in 1..=3 {
            let clusters = hierarchy.clusters_at_level(level);

            for cluster in clusters {
                if cluster.size() > 1 {
                    let child_ids: Vec<u64> = cluster.member_ids();
                    let meta_centroid = cluster.centroid();

                    // Average params from children
                    let children: Vec<_> = child_ids.iter()
                        .filter_map(|id| self.patterns.get(id))
                        .collect();

                    let meta_params = self.average_params(&children);

                    let meta_pattern = LearnedPattern {
                        id: self.next_pattern_id.fetch_add(1, Ordering::SeqCst),
                        centroid: meta_centroid,
                        optimal_params: meta_params,
                        confidence: children.iter().map(|c| c.confidence).sum::<f32>() / children.len() as f32,
                        support_count: children.iter().map(|c| c.support_count).sum(),
                        avg_precision: children.iter().map(|c| c.avg_precision).sum::<f32>() / children.len() as f32,
                        avg_recall: children.iter().map(|c| c.avg_recall).sum::<f32>() / children.len() as f32,
                        avg_latency_us: children.iter().map(|c| c.avg_latency_us).sum::<u64>() / children.len() as u64,
                        created_at: chrono::Utc::now().timestamp(),
                        updated_at: chrono::Utc::now().timestamp(),
                        abstraction_level: level as u32,
                        children: child_ids,
                    };

                    self.patterns.insert(meta_pattern.id, meta_pattern);
                }
            }
        }
    }
}
```

---

## 8. Statistics and Monitoring

```rust
#[derive(Default, Debug)]
pub struct ReasoningBankStats {
    /// Total trajectories recorded
    pub total_trajectories: u64,
    /// Total patterns stored
    pub total_patterns: usize,
    /// Total verdicts issued
    pub total_verdicts: usize,
    /// Pattern match hit rate
    pub pattern_hit_rate: f32,
    /// Average confidence in recommendations
    pub avg_recommendation_confidence: f32,
    /// Improvement from pattern optimization
    pub avg_improvement_percent: f32,
}

impl ReasoningBank {
    /// Get current statistics
    pub fn stats(&self) -> ReasoningBankStats {
        let stats = self.stats.read();
        ReasoningBankStats {
            total_trajectories: stats.total_trajectories,
            total_patterns: self.patterns.len(),
            total_verdicts: self.verdicts.len(),
            pattern_hit_rate: stats.pattern_hit_rate,
            avg_recommendation_confidence: stats.avg_recommendation_confidence,
            avg_improvement_percent: stats.avg_improvement_percent,
        }
    }

    /// Export all patterns for persistence
    pub fn export(&self) -> ReasoningBankExport {
        ReasoningBankExport {
            patterns: self.patterns.iter()
                .map(|p| p.value().clone())
                .collect(),
            verdicts: self.verdicts.iter()
                .map(|v| v.value().clone())
                .collect(),
        }
    }

    /// Import patterns from persistence
    pub fn import(&mut self, export: ReasoningBankExport) {
        for pattern in export.patterns {
            let id = pattern.id;
            self.patterns.insert(id, pattern.clone());
            self.pattern_index.write().insert(id, &pattern.centroid);
        }
        for verdict in export.verdicts {
            self.verdicts.insert(verdict.pattern_id, verdict);
        }
    }
}
```

---

## Summary

ReasoningBank enables SONA to:

1. **Learn from every query** through trajectory recording
2. **Discover patterns** via K-means++ clustering
3. **Judge what works** through statistical verdict analysis
4. **Optimize future decisions** by interpolating from similar patterns
5. **Build abstractions** through hierarchical pattern consolidation

This creates a continuously improving system where past experience directly enhances future performance.
