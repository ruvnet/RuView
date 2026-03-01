# SONA Memory Dreams: Offline Consolidation Engine

## Creativity Through Neural Replay and Recombination

---

## 1. Biological Inspiration

### Why Dreams Matter for Learning

```
HUMAN SLEEP-BASED LEARNING
══════════════════════════

Awake:                    Sleep (REM):              Next Day:
─────────────────         ─────────────────         ─────────────────
• New experiences         • Replay memories         • Consolidated knowledge
• Pattern matching        • Recombine ideas         • Novel insights
• Working memory          • Strengthen important    • Creative connections
                          • Prune unimportant
```

Research shows that:
- **Memory consolidation** happens during sleep
- **Creative insights** emerge from random memory replay
- **Neural pruning** removes low-value connections
- **Analogical reasoning** connects distant concepts

SONA's Dream Engine replicates these mechanisms for AI self-improvement.

---

## 2. Dream Engine Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                      DREAM ENGINE ARCHITECTURE                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌───────────────┐                                                 │
│   │ MEMORY GRAPH  │──────┐                                          │
│   └───────────────┘      │                                          │
│                          ▼                                          │
│   ┌─────────────────────────────────────┐                          │
│   │        DREAM GENERATOR              │                          │
│   │                                     │                          │
│   │  ┌─────────┐  ┌─────────┐          │                          │
│   │  │ Random  │  │Weighted │          │                          │
│   │  │ Walks   │  │ Sampling│          │                          │
│   │  └────┬────┘  └────┬────┘          │                          │
│   │       │            │               │                          │
│   │       ▼            ▼               │                          │
│   │  ┌──────────────────────┐          │                          │
│   │  │   Dream Sequence     │          │                          │
│   │  │   [M₁→M₂→M₃→...→Mₙ] │          │                          │
│   │  └──────────┬───────────┘          │                          │
│   └─────────────┼───────────────────────┘                          │
│                 │                                                   │
│                 ▼                                                   │
│   ┌─────────────────────────────────────┐                          │
│   │       DREAM EVALUATOR               │                          │
│   │                                     │                          │
│   │  • Novelty Score (new connections?) │                          │
│   │  • Coherence Score (makes sense?)   │                          │
│   │  • Utility Score (useful insight?)  │                          │
│   └─────────────────────────────────────┘                          │
│                 │                                                   │
│                 ▼                                                   │
│   ┌─────────────────────────────────────┐                          │
│   │       DREAM INTEGRATOR              │                          │
│   │                                     │                          │
│   │  • Add weak creative edges          │                          │
│   │  • Update pattern associations      │                          │
│   │  • Generate novel hypotheses        │                          │
│   └─────────────────────────────────────┘                          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Dream Generation

### Random Walk Memory Replay

```rust
/// Dream generator using random walks on memory graph
pub struct DreamGenerator {
    /// Temperature for random walk (higher = more random)
    temperature: f32,
    /// Maximum dream length
    max_length: usize,
    /// Minimum coherence threshold
    min_coherence: f32,
    /// Creativity bias (prefer novel connections)
    creativity_bias: f32,
}

impl DreamGenerator {
    /// Generate a single dream sequence
    pub fn generate_dream(
        &self,
        memory: &MemoryGraph,
        start_node: Option<NodeId>,
    ) -> Dream {
        let mut sequence = Vec::new();
        let mut visited = HashSet::new();

        // Start from random high-activation node if not specified
        let current = start_node.unwrap_or_else(|| {
            memory.sample_by_activation()
        });

        sequence.push(current);
        visited.insert(current);

        // Random walk with creativity-weighted transitions
        for _ in 0..self.max_length {
            let neighbors = memory.get_neighbors(current);

            if neighbors.is_empty() {
                break;
            }

            // Compute transition probabilities
            let probs: Vec<f32> = neighbors.iter()
                .map(|&(neighbor, edge_weight)| {
                    let novelty_bonus = if visited.contains(&neighbor) {
                        0.1 // Discourage revisits
                    } else {
                        1.0 + self.creativity_bias * (1.0 - memory.get_access_frequency(neighbor))
                    };

                    (edge_weight * novelty_bonus).powf(1.0 / self.temperature)
                })
                .collect();

            // Sample next node
            let next = sample_weighted(&neighbors, &probs);

            if let Some((next_node, _)) = next {
                sequence.push(next_node);
                visited.insert(next_node);
            } else {
                break;
            }
        }

        Dream {
            sequence,
            temperature: self.temperature,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    /// Generate creative jump dream (non-local connections)
    pub fn generate_creative_dream(
        &self,
        memory: &MemoryGraph,
        num_jumps: usize,
    ) -> Dream {
        let mut sequence = Vec::new();

        // Sample diverse starting points
        let anchors = memory.sample_diverse(num_jumps, 0.3);

        for anchor in anchors {
            sequence.push(anchor);

            // Short local walk from each anchor
            let local_walk = self.generate_dream(memory, Some(anchor));
            sequence.extend(local_walk.sequence.iter().skip(1).take(3));
        }

        Dream {
            sequence,
            temperature: self.temperature * 2.0, // Higher temperature for creative dreams
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

/// A dream sequence
pub struct Dream {
    /// Sequence of visited memory nodes
    pub sequence: Vec<NodeId>,
    /// Temperature used for generation
    pub temperature: f32,
    /// Generation timestamp
    pub timestamp: i64,
}
```

---

## 4. Dream Evaluation

### Measuring Dream Quality

```rust
/// Evaluator for dream quality
pub struct DreamEvaluator {
    /// Memory graph reference
    memory: Arc<MemoryGraph>,
    /// Novelty detection threshold
    novelty_threshold: f32,
}

impl DreamEvaluator {
    /// Evaluate dream quality across multiple dimensions
    pub fn evaluate(&self, dream: &Dream) -> DreamQuality {
        DreamQuality {
            novelty: self.compute_novelty(dream),
            coherence: self.compute_coherence(dream),
            utility: self.compute_utility(dream),
            diversity: self.compute_diversity(dream),
        }
    }

    /// Novelty: How many new connections are suggested?
    fn compute_novelty(&self, dream: &Dream) -> f32 {
        let mut novel_pairs = 0;
        let mut total_pairs = 0;

        for i in 0..dream.sequence.len() {
            for j in (i+1)..dream.sequence.len() {
                total_pairs += 1;

                let node_a = dream.sequence[i];
                let node_b = dream.sequence[j];

                // Check if edge exists
                if !self.memory.has_edge(node_a, node_b) {
                    // Check semantic similarity
                    let emb_a = self.memory.get_embedding(node_a);
                    let emb_b = self.memory.get_embedding(node_b);
                    let sim = cosine_similarity(&emb_a, &emb_b);

                    // Novel = no edge but moderate similarity
                    if sim > 0.3 && sim < 0.8 {
                        novel_pairs += 1;
                    }
                }
            }
        }

        novel_pairs as f32 / total_pairs.max(1) as f32
    }

    /// Coherence: Does the dream sequence make semantic sense?
    fn compute_coherence(&self, dream: &Dream) -> f32 {
        if dream.sequence.len() < 2 {
            return 1.0;
        }

        let mut coherence_sum = 0.0f32;

        for window in dream.sequence.windows(2) {
            let emb_a = self.memory.get_embedding(window[0]);
            let emb_b = self.memory.get_embedding(window[1]);
            coherence_sum += cosine_similarity(&emb_a, &emb_b);
        }

        coherence_sum / (dream.sequence.len() - 1) as f32
    }

    /// Utility: Are the suggested connections potentially useful?
    fn compute_utility(&self, dream: &Dream) -> f32 {
        // Based on node quality scores and access patterns
        let avg_quality: f32 = dream.sequence.iter()
            .map(|&id| self.memory.get_node_quality(id))
            .sum::<f32>() / dream.sequence.len() as f32;

        // Higher utility if connecting high-quality nodes
        avg_quality
    }

    /// Diversity: How diverse are the visited nodes?
    fn compute_diversity(&self, dream: &Dream) -> f32 {
        // Average pairwise distance in embedding space
        let embeddings: Vec<_> = dream.sequence.iter()
            .map(|&id| self.memory.get_embedding(id))
            .collect();

        let mut total_dist = 0.0f32;
        let mut count = 0;

        for i in 0..embeddings.len() {
            for j in (i+1)..embeddings.len() {
                total_dist += 1.0 - cosine_similarity(&embeddings[i], &embeddings[j]);
                count += 1;
            }
        }

        total_dist / count.max(1) as f32
    }
}

#[derive(Debug, Clone)]
pub struct DreamQuality {
    /// How many novel connections suggested (0-1)
    pub novelty: f32,
    /// How semantically coherent (0-1)
    pub coherence: f32,
    /// How useful the connections might be (0-1)
    pub utility: f32,
    /// How diverse the dream content (0-1)
    pub diversity: f32,
}

impl DreamQuality {
    /// Overall quality score
    pub fn overall(&self) -> f32 {
        // Weighted combination favoring novelty and coherence
        0.4 * self.novelty + 0.3 * self.coherence + 0.2 * self.utility + 0.1 * self.diversity
    }

    /// Is this dream worth integrating?
    pub fn is_valuable(&self, threshold: f32) -> bool {
        self.novelty > 0.3 && self.coherence > 0.4 && self.overall() > threshold
    }
}
```

---

## 5. Dream Integration

### Applying Dream Insights to Memory

```rust
/// Integrates valuable dreams into memory graph
pub struct DreamIntegrator {
    /// Memory graph to update
    memory: Arc<RwLock<MemoryGraph>>,
    /// Strength of new creative edges
    creative_edge_strength: f32,
    /// Decay factor for dream-derived edges
    dream_edge_decay: f32,
}

impl DreamIntegrator {
    /// Integrate a valuable dream into memory
    pub fn integrate(&self, dream: &Dream, quality: &DreamQuality) -> IntegrationResult {
        let mut result = IntegrationResult::default();

        if !quality.is_valuable(0.5) {
            return result; // Skip low-quality dreams
        }

        let mut memory = self.memory.write();

        // Extract novel connections from dream
        let novel_connections = self.extract_novel_connections(dream, &memory);

        for (node_a, node_b, strength) in novel_connections {
            // Add weak creative edge
            let edge_strength = self.creative_edge_strength * strength * quality.overall();

            memory.add_edge(
                node_a,
                node_b,
                EdgeType::Creative,
                edge_strength,
            );

            result.edges_added += 1;
        }

        // Update node associations based on dream co-occurrence
        for window in dream.sequence.windows(3) {
            memory.update_association(window[0], window[2], 0.01);
        }

        result.dream_quality = quality.overall();
        result
    }

    fn extract_novel_connections(
        &self,
        dream: &Dream,
        memory: &MemoryGraph,
    ) -> Vec<(NodeId, NodeId, f32)> {
        let mut connections = Vec::new();

        for i in 0..dream.sequence.len() {
            for j in (i+1)..dream.sequence.len().min(i+5) { // Only nearby in sequence
                let node_a = dream.sequence[i];
                let node_b = dream.sequence[j];

                if !memory.has_edge(node_a, node_b) {
                    let emb_a = memory.get_embedding(node_a);
                    let emb_b = memory.get_embedding(node_b);
                    let sim = cosine_similarity(&emb_a, &emb_b);

                    if sim > 0.3 {
                        // Connection strength based on similarity and sequence proximity
                        let proximity_factor = 1.0 / (j - i) as f32;
                        let strength = sim * proximity_factor;
                        connections.push((node_a, node_b, strength));
                    }
                }
            }
        }

        connections
    }
}

#[derive(Default)]
pub struct IntegrationResult {
    pub edges_added: usize,
    pub associations_updated: usize,
    pub dream_quality: f32,
}
```

---

## 6. Memory Consolidation

### Strengthening Important Memories

```rust
/// Consolidation engine for memory pruning and strengthening
pub struct ConsolidationEngine {
    /// Memory graph reference
    memory: Arc<RwLock<MemoryGraph>>,
    /// Minimum access frequency for retention
    min_access_frequency: f32,
    /// Age decay factor (older = more decay)
    age_decay: f32,
    /// Quality threshold for preservation
    quality_threshold: f32,
}

impl ConsolidationEngine {
    /// Run full consolidation pass
    pub fn consolidate(&self) -> ConsolidationReport {
        let mut report = ConsolidationReport::default();

        // Phase 1: Identify memories by value
        let (high_value, medium_value, low_value) = self.categorize_memories();
        report.high_value_count = high_value.len();
        report.medium_value_count = medium_value.len();
        report.low_value_count = low_value.len();

        // Phase 2: Strengthen high-value memories
        for &node_id in &high_value {
            self.strengthen_memory(node_id);
            report.memories_strengthened += 1;
        }

        // Phase 3: Decay low-value memories
        for &node_id in &low_value {
            let retained = self.decay_memory(node_id);
            if retained {
                report.memories_decayed += 1;
            } else {
                report.memories_removed += 1;
            }
        }

        // Phase 4: Prune weak edges
        let pruned = self.prune_weak_edges();
        report.edges_pruned = pruned;

        // Phase 5: Merge similar memories
        let merged = self.merge_similar_memories();
        report.memories_merged = merged;

        report
    }

    fn categorize_memories(&self) -> (Vec<NodeId>, Vec<NodeId>, Vec<NodeId>) {
        let memory = self.memory.read();
        let mut high = Vec::new();
        let mut medium = Vec::new();
        let mut low = Vec::new();

        for node in memory.iter_nodes() {
            let value_score = self.compute_value_score(node);

            if value_score > 0.7 {
                high.push(node.id);
            } else if value_score > 0.3 {
                medium.push(node.id);
            } else {
                low.push(node.id);
            }
        }

        (high, medium, low)
    }

    fn compute_value_score(&self, node: &MemoryNode) -> f32 {
        let memory = self.memory.read();

        // Factors:
        // 1. Access frequency (more access = more valuable)
        let freq_score = (node.access_count as f32 / 100.0).min(1.0);

        // 2. Recency (recent = more valuable)
        let age_days = (chrono::Utc::now().timestamp() - node.last_accessed) / 86400;
        let recency_score = (-self.age_decay * age_days as f32).exp();

        // 3. Quality (explicit quality score)
        let quality_score = node.quality_score;

        // 4. Connectivity (well-connected = more valuable)
        let degree = memory.node_degree(node.id);
        let connectivity_score = (degree as f32 / 10.0).min(1.0);

        // Weighted combination
        0.3 * freq_score + 0.2 * recency_score + 0.3 * quality_score + 0.2 * connectivity_score
    }

    fn strengthen_memory(&self, node_id: NodeId) {
        let mut memory = self.memory.write();

        // Increase edge weights to this node
        for edge in memory.get_edges_to(node_id) {
            memory.update_edge_weight(edge.from, node_id, EdgeUpdate::Multiply(1.1));
        }

        // Mark as consolidated
        if let Some(node) = memory.get_node_mut(node_id) {
            node.consolidation_count += 1;
            node.last_consolidated = chrono::Utc::now().timestamp();
        }
    }

    fn decay_memory(&self, node_id: NodeId) -> bool {
        let mut memory = self.memory.write();

        // Reduce edge weights
        for edge in memory.get_edges_to(node_id) {
            memory.update_edge_weight(edge.from, node_id, EdgeUpdate::Multiply(0.5));
        }

        // Check if node should be removed entirely
        let total_incoming_weight: f32 = memory.get_edges_to(node_id)
            .iter()
            .map(|e| e.weight)
            .sum();

        if total_incoming_weight < 0.01 {
            // Remove isolated or nearly-isolated node
            memory.remove_node(node_id);
            false // Not retained
        } else {
            true // Retained but weakened
        }
    }

    fn prune_weak_edges(&self) -> usize {
        let mut memory = self.memory.write();
        let weak_edges: Vec<_> = memory.iter_edges()
            .filter(|e| e.weight < 0.01)
            .map(|e| e.id)
            .collect();

        for edge_id in &weak_edges {
            memory.remove_edge(*edge_id);
        }

        weak_edges.len()
    }

    fn merge_similar_memories(&self) -> usize {
        let mut memory = self.memory.write();
        let mut merged_count = 0;

        // Find highly similar node pairs
        let nodes: Vec<_> = memory.iter_nodes().collect();

        for i in 0..nodes.len() {
            for j in (i+1)..nodes.len() {
                let sim = cosine_similarity(&nodes[i].embedding, &nodes[j].embedding);

                if sim > 0.98 {
                    // Merge j into i
                    memory.merge_nodes(nodes[i].id, nodes[j].id);
                    merged_count += 1;
                }
            }
        }

        merged_count
    }
}

#[derive(Default)]
pub struct ConsolidationReport {
    pub high_value_count: usize,
    pub medium_value_count: usize,
    pub low_value_count: usize,
    pub memories_strengthened: usize,
    pub memories_decayed: usize,
    pub memories_removed: usize,
    pub memories_merged: usize,
    pub edges_pruned: usize,
}
```

---

## 7. Full Dream Cycle

### Orchestrating the Dream Process

```rust
/// Complete dream cycle orchestrator
pub struct DreamCycle {
    generator: DreamGenerator,
    evaluator: DreamEvaluator,
    integrator: DreamIntegrator,
    consolidator: ConsolidationEngine,
    config: DreamCycleConfig,
}

impl DreamCycle {
    /// Run complete dream cycle (weekly maintenance)
    pub async fn run(&self) -> DreamCycleReport {
        let start = Instant::now();
        let mut report = DreamCycleReport::default();

        // Phase 1: Generate dreams
        tracing::info!("Starting dream generation phase");
        let dreams = self.generate_dreams();
        report.dreams_generated = dreams.len();

        // Phase 2: Evaluate dreams
        tracing::info!("Evaluating {} dreams", dreams.len());
        let evaluated: Vec<_> = dreams.iter()
            .map(|d| (d, self.evaluator.evaluate(d)))
            .collect();

        // Phase 3: Integrate valuable dreams
        tracing::info!("Integrating valuable dreams");
        for (dream, quality) in &evaluated {
            if quality.is_valuable(self.config.dream_threshold) {
                let result = self.integrator.integrate(dream, quality);
                report.edges_added += result.edges_added;
                report.dreams_integrated += 1;
            }
        }

        // Phase 4: Memory consolidation
        tracing::info!("Running memory consolidation");
        report.consolidation = self.consolidator.consolidate();

        report.elapsed_ms = start.elapsed().as_millis() as u64;
        report.timestamp = chrono::Utc::now().timestamp();

        tracing::info!(
            dreams = report.dreams_generated,
            integrated = report.dreams_integrated,
            edges = report.edges_added,
            elapsed_ms = report.elapsed_ms,
            "Dream cycle completed"
        );

        report
    }

    fn generate_dreams(&self) -> Vec<Dream> {
        let mut dreams = Vec::new();

        // Regular random walk dreams
        for _ in 0..self.config.num_regular_dreams {
            let dream = self.generator.generate_dream(&self.memory, None);
            dreams.push(dream);
        }

        // Creative jump dreams
        for _ in 0..self.config.num_creative_dreams {
            let dream = self.generator.generate_creative_dream(
                &self.memory,
                self.config.creative_jump_count,
            );
            dreams.push(dream);
        }

        dreams
    }
}

#[derive(Default)]
pub struct DreamCycleReport {
    pub dreams_generated: usize,
    pub dreams_integrated: usize,
    pub edges_added: usize,
    pub consolidation: ConsolidationReport,
    pub elapsed_ms: u64,
    pub timestamp: i64,
}
```

---

## 8. Integration with exo-exotic Dreams Module

SONA integrates with the exo-ai-2025 dream experiments:

```rust
// From exo-exotic crate
use exo_exotic::experiments::dreams::{
    DreamExperiment,
    DreamConfig,
    NoveltyMeasure,
};

impl DreamCycle {
    /// Run advanced dream experiments from exo-exotic
    pub async fn run_exotic_dreams(&self) -> ExoticDreamReport {
        let dream_experiment = DreamExperiment::new(DreamConfig {
            memory_count: self.memory.node_count(),
            replay_probability: 0.7,
            recombination_rate: 0.3,
            novelty_threshold: 0.5,
        });

        let result = dream_experiment.run(&self.memory).await;

        ExoticDreamReport {
            novelty_score: result.novelty,
            coherence_score: result.coherence,
            creative_insights: result.insights.len(),
            new_hypotheses: result.hypotheses,
        }
    }
}
```

---

## Summary

SONA's Dream Engine enables:

| Feature | Mechanism | Outcome |
|---------|-----------|---------|
| **Memory Replay** | Random walks on memory graph | Strengthens important connections |
| **Creative Recombination** | High-temperature sampling | Discovers novel associations |
| **Quality Filtering** | Novelty + coherence metrics | Only valuable dreams integrated |
| **Weak Edge Creation** | Dream-derived connections | Enables creative retrieval |
| **Memory Consolidation** | Value-based pruning | Efficient memory usage |

Dreams allow SONA to:
1. **Discover** connections it wouldn't find through normal operation
2. **Explore** the hypothesis space without user cost
3. **Consolidate** valuable knowledge
4. **Prune** low-value information
5. **Remain creative** while staying grounded
