# Feature 19: Consensus Attention

## Overview

### Problem Statement
Single attention computations can be unreliable due to noise, model uncertainty, or edge cases in the embedding space. Traditional attention provides no confidence measure or fault tolerance. Production systems need robust attention that can quantify uncertainty and resist failures or adversarial perturbations through redundancy and agreement.

### Proposed Solution
Consensus Attention runs K independent attention computations (potentially with different parameters, initializations, or subsets of data) and requires agreement before returning results. Uses Byzantine fault-tolerant majority voting to ensure robustness. Provides uncertainty quantification through vote distribution and enables detection of ambiguous or borderline queries.

### Expected Benefits
- **Robustness**: 70-90% reduction in erroneous results
- **Uncertainty Quantification**: Confidence scores for each result
- **Byzantine Fault Tolerance**: Tolerates up to ⌊K/3⌋ faulty/adversarial nodes
- **Ambiguity Detection**: Identify queries with low consensus
- **Quality Assurance**: Higher precision on confident predictions
- **Interpretability**: Understand agreement patterns

### Novelty Claim
**Unique Contribution**: First GNN attention mechanism with Byzantine fault-tolerant consensus and uncertainty quantification through multi-node voting. Unlike ensemble methods (which average predictions), Consensus Attention requires explicit agreement and provides formal fault tolerance guarantees.

**Differentiators**:
1. Byzantine fault tolerance with formal guarantees
2. Uncertainty quantification via vote distribution
3. Adaptive K based on query complexity
4. Hierarchical consensus for efficiency
5. Integration with other attention mechanisms

## Technical Design

### Architecture Diagram

```
                    Input Query (q)
                         |
         +---------------+---------------+
         |               |               |
    Attention        Attention      Attention
    Node 1           Node 2         Node K
    (variant 1)      (variant 2)    (variant K)
         |               |               |
    ┌────────┐      ┌────────┐      ┌────────┐
    │ Param  │      │ Param  │      │ Param  │
    │ Set 1  │      │ Set 2  │      │ Set K  │
    └───┬────┘      └───┬────┘      └───┬────┘
        |               |               |
        v               v               v
    Results_1       Results_2       Results_K
    [i1,i2,i3]      [i2,i1,i4]      [i1,i2,i5]
    [s1,s2,s3]      [s2,s1,s4]      [s1,s2,s5]
        |               |               |
        +-------+-------+-------+-------+
                |
         Voting Protocol
         (Byzantine Fault Tolerant)
                |
         +------+------+
         |             |
    Vote Counting  Threshold Check
         |             |
         v             v
    Per-Item       Minimum Votes
    Vote Count     Required: ⌈2K/3⌉
         |             |
         +------+------+
                |
         Consensus Results
         + Confidence Scores
                |
         +------+------+
         |             |
    High Confidence  Low Confidence
    (unanimous)      (split votes)
         |             |
         v             v
    Return           Flag as
    Results          Uncertain


Voting Detail:

Item Votes Table:
┌──────┬────────┬────────┬────────┬─────────┐
│ Item │ Node 1 │ Node 2 │ Node K │ Votes   │
├──────┼────────┼────────┼────────┼─────────┤
│  i1  │   ✓    │   ✓    │   ✓    │ 3/3 ⭐  │
│  i2  │   ✓    │   ✓    │   ✓    │ 3/3 ⭐  │
│  i3  │   ✓    │        │        │ 1/3     │
│  i4  │        │   ✓    │        │ 1/3     │
│  i5  │        │        │   ✓    │ 1/3     │
└──────┴────────┴────────┴────────┴─────────┘

Consensus: {i1, i2} (both have ≥ ⌈2K/3⌉ votes)
Confidence: i1 = 1.0, i2 = 1.0


Byzantine Fault Tolerance:

Total Nodes: K = 7
Faulty Nodes: f ≤ ⌊K/3⌋ = 2
Minimum Votes for Consensus: ⌈2K/3⌉ = 5

Honest Nodes (5): All agree on item X
Faulty Nodes (2): Vote for item Y

Result: Item X gets 5 votes, Item Y gets 2 votes
Consensus: X (exceeds threshold of 5)
Y is rejected (below threshold)


Hierarchical Consensus (for efficiency):

Level 1: Local Consensus (groups of 3)
┌─────────┐  ┌─────────┐  ┌─────────┐
│ Node1-3 │  │ Node4-6 │  │ Node7-9 │
│Consensus│  │Consensus│  │Consensus│
└────┬────┘  └────┬────┘  └────┬────┘
     │            │            │
     v            v            v
  Result_1     Result_2     Result_3

Level 2: Global Consensus
     │            │            │
     +------+-----+-----+------+
            │
      Final Consensus


Adaptive K Selection:

Query Complexity → K Selection

┌──────────────────┬─────┐
│ Simple/Confident │ K=3 │
│ (low entropy)    │     │
└──────────────────┴─────┘

┌──────────────────┬─────┐
│ Medium           │ K=5 │
│ (moderate)       │     │
└──────────────────┴─────┘

┌──────────────────┬─────┐
│ Complex/Uncertain│ K=7 │
│ (high entropy)   │     │
└──────────────────┴─────┘

┌──────────────────┬──────┐
│ Critical/Security│ K=9  │
│ (max robustness) │      │
└──────────────────┴──────┘
```

### Core Data Structures

```rust
/// Configuration for Consensus Attention
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Number of independent attention nodes
    pub num_nodes: usize,

    /// Voting threshold (fraction of nodes required for consensus)
    /// Typically 2/3 for Byzantine fault tolerance
    pub vote_threshold: f32,

    /// Node variant strategy
    pub variant_strategy: VariantStrategy,

    /// Enable adaptive K based on query
    pub adaptive_k: bool,

    /// Minimum K for adaptive mode
    pub min_k: usize,

    /// Maximum K for adaptive mode
    pub max_k: usize,

    /// Enable hierarchical consensus
    pub hierarchical: bool,

    /// Group size for hierarchical consensus
    pub group_size: usize,

    /// Uncertainty threshold
    pub uncertainty_threshold: f32,
}

/// Strategy for creating node variants
#[derive(Debug, Clone, PartialEq)]
pub enum VariantStrategy {
    /// Different random initializations
    RandomInit,

    /// Different hyperparameters (temperature, etc.)
    HyperparamVariation,

    /// Different attention mechanisms
    MechanismVariation,

    /// Different data subsets (bootstrap)
    Bootstrap,

    /// Combination of above
    Hybrid,
}

/// Single attention node in consensus
#[derive(Debug)]
pub struct AttentionNode {
    /// Node identifier
    pub id: usize,

    /// Underlying attention mechanism
    pub attention: Box<dyn AttentionLayer>,

    /// Node-specific parameters
    pub params: NodeParams,

    /// Node health status
    pub status: NodeStatus,

    /// Performance metrics
    pub metrics: NodeMetrics,
}

#[derive(Debug, Clone)]
pub struct NodeParams {
    /// Temperature for attention softmax
    pub temperature: f32,

    /// Random seed (for reproducibility)
    pub seed: u64,

    /// Top-k parameter
    pub top_k: usize,

    /// Additional variant-specific params
    pub variant_params: HashMap<String, f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeStatus {
    /// Node is healthy and responding
    Healthy,

    /// Node is suspected faulty
    Suspected,

    /// Node is confirmed faulty
    Faulty,

    /// Node is offline/unavailable
    Offline,
}

#[derive(Debug, Default)]
pub struct NodeMetrics {
    /// Total queries processed
    pub queries_processed: usize,

    /// Average latency
    pub avg_latency_ms: f32,

    /// Agreement rate with consensus
    pub agreement_rate: f32,

    /// Error count
    pub errors: usize,
}

/// Vote record for a single item
#[derive(Debug, Clone)]
pub struct ItemVote {
    /// Item index
    pub item_idx: usize,

    /// Nodes that voted for this item
    pub voters: HashSet<usize>,

    /// Vote count
    pub vote_count: usize,

    /// Average score across voters
    pub avg_score: f32,

    /// Score variance (for uncertainty)
    pub score_variance: f32,
}

/// Consensus result
#[derive(Debug, Clone)]
pub struct ConsensusResult {
    /// Consensus items (indices)
    pub consensus_indices: Vec<usize>,

    /// Consensus scores
    pub consensus_scores: Vec<f32>,

    /// Confidence per item (vote count / total nodes)
    pub confidence: Vec<f32>,

    /// Overall consensus strength
    pub consensus_strength: f32,

    /// Uncertain items (low consensus)
    pub uncertain_indices: Vec<usize>,

    /// Detailed voting record
    pub vote_details: Vec<ItemVote>,

    /// Number of nodes that participated
    pub participating_nodes: usize,
}

/// Voting protocol
pub trait VotingProtocol: Send + Sync {
    /// Collect votes from all nodes
    fn collect_votes(
        &self,
        node_results: Vec<(usize, Vec<usize>, Vec<f32>)>
    ) -> Vec<ItemVote>;

    /// Apply consensus rules to determine final result
    fn apply_consensus(
        &self,
        votes: Vec<ItemVote>,
        threshold: usize
    ) -> ConsensusResult;

    /// Detect Byzantine/faulty nodes
    fn detect_faulty_nodes(
        &self,
        node_results: Vec<(usize, Vec<usize>, Vec<f32>)>
    ) -> Vec<usize>;
}

/// Byzantine fault-tolerant voting
#[derive(Debug)]
pub struct ByzantineVoting {
    /// Total number of nodes
    num_nodes: usize,

    /// Maximum tolerable faults
    max_faults: usize,

    /// Minimum votes required (2f + 1)
    min_votes: usize,
}

impl VotingProtocol for ByzantineVoting {
    fn collect_votes(
        &self,
        node_results: Vec<(usize, Vec<usize>, Vec<f32>)>
    ) -> Vec<ItemVote> {

        // Aggregate votes across all nodes
        let mut vote_map: HashMap<usize, ItemVote> = HashMap::new();

        for (node_id, indices, scores) in node_results {
            for (&idx, &score) in indices.iter().zip(scores.iter()) {
                vote_map.entry(idx)
                    .and_modify(|v| {
                        v.voters.insert(node_id);
                        v.vote_count += 1;

                        // Update average score incrementally
                        let n = v.vote_count as f32;
                        v.avg_score = ((n - 1.0) * v.avg_score + score) / n;
                    })
                    .or_insert_with(|| {
                        let mut voters = HashSet::new();
                        voters.insert(node_id);
                        ItemVote {
                            item_idx: idx,
                            voters,
                            vote_count: 1,
                            avg_score: score,
                            score_variance: 0.0,
                        }
                    });
            }
        }

        // Compute variance
        for vote in vote_map.values_mut() {
            let mut score_sum = 0.0;
            let mut count = 0;

            for (node_id, indices, scores) in &node_results {
                if vote.voters.contains(node_id) {
                    if let Some(pos) = indices.iter().position(|&i| i == vote.item_idx) {
                        let diff = scores[pos] - vote.avg_score;
                        score_sum += diff * diff;
                        count += 1;
                    }
                }
            }

            vote.score_variance = if count > 1 {
                score_sum / (count - 1) as f32
            } else {
                0.0
            };
        }

        vote_map.into_values().collect()
    }

    fn apply_consensus(
        &self,
        mut votes: Vec<ItemVote>,
        threshold: usize
    ) -> ConsensusResult {

        // Sort by vote count (descending)
        votes.sort_by(|a, b| b.vote_count.cmp(&a.vote_count));

        // Separate consensus vs. uncertain items
        let mut consensus_indices = Vec::new();
        let mut consensus_scores = Vec::new();
        let mut confidence = Vec::new();
        let mut uncertain_indices = Vec::new();

        for vote in &votes {
            if vote.vote_count >= threshold {
                // Consensus reached
                consensus_indices.push(vote.item_idx);
                consensus_scores.push(vote.avg_score);
                confidence.push(vote.vote_count as f32 / self.num_nodes as f32);
            } else if vote.vote_count >= self.num_nodes / 2 {
                // Partial consensus (uncertain)
                uncertain_indices.push(vote.item_idx);
            }
        }

        // Compute overall consensus strength
        let consensus_strength = if !consensus_indices.is_empty() {
            confidence.iter().sum::<f32>() / consensus_indices.len() as f32
        } else {
            0.0
        };

        ConsensusResult {
            consensus_indices,
            consensus_scores,
            confidence,
            consensus_strength,
            uncertain_indices,
            vote_details: votes,
            participating_nodes: self.num_nodes,
        }
    }

    fn detect_faulty_nodes(
        &self,
        node_results: Vec<(usize, Vec<usize>, Vec<f32>)>
    ) -> Vec<usize> {

        let mut faulty = Vec::new();

        // Compute pairwise agreement between nodes
        let num_nodes = node_results.len();
        let mut agreement_matrix = vec![vec![0.0; num_nodes]; num_nodes];

        for i in 0..num_nodes {
            for j in (i+1)..num_nodes {
                let (_, indices_i, _) = &node_results[i];
                let (_, indices_j, _) = &node_results[j];

                // Jaccard similarity
                let set_i: HashSet<_> = indices_i.iter().collect();
                let set_j: HashSet<_> = indices_j.iter().collect();
                let intersection = set_i.intersection(&set_j).count();
                let union = set_i.union(&set_j).count();
                let similarity = intersection as f32 / union as f32;

                agreement_matrix[i][j] = similarity;
                agreement_matrix[j][i] = similarity;
            }
        }

        // Identify nodes with low average agreement
        for i in 0..num_nodes {
            let avg_agreement: f32 = agreement_matrix[i].iter().sum::<f32>() / (num_nodes - 1) as f32;

            // If node disagrees with majority, mark as faulty
            if avg_agreement < 0.3 {
                faulty.push(node_results[i].0);
            }
        }

        faulty
    }
}

/// Main Consensus Attention layer
pub struct ConsensusAttention {
    /// Configuration
    config: ConsensusConfig,

    /// Attention nodes
    nodes: Vec<AttentionNode>,

    /// Voting protocol
    voting: Box<dyn VotingProtocol>,

    /// Suspected faulty nodes
    suspected_faulty: HashSet<usize>,

    /// Metrics
    metrics: ConsensusMetrics,
}

#[derive(Debug, Default)]
pub struct ConsensusMetrics {
    /// Total queries processed
    pub total_queries: usize,

    /// Queries with full consensus
    pub full_consensus_count: usize,

    /// Queries with partial consensus
    pub partial_consensus_count: usize,

    /// Queries with no consensus
    pub no_consensus_count: usize,

    /// Average consensus strength
    pub avg_consensus_strength: f32,

    /// Average number of uncertain items
    pub avg_uncertain_items: f32,

    /// Detected faulty node incidents
    pub faulty_node_detections: usize,

    /// Average latency
    pub avg_latency_ms: f32,
}
```

### Key Algorithms

#### 1. Consensus Forward Pass

```rust
/// Forward pass with consensus
async fn forward_consensus(
    &mut self,
    query: &[f32],
    k: usize
) -> Result<ConsensusResult, ConsensusError> {

    let start_time = Instant::now();

    // Step 1: Determine number of nodes (adaptive K)
    let num_active_nodes = if self.config.adaptive_k {
        self.compute_adaptive_k(query)
    } else {
        self.config.num_nodes
    };

    // Step 2: Run attention on all nodes in parallel
    let node_futures: Vec<_> = self.nodes.iter_mut()
        .take(num_active_nodes)
        .filter(|n| n.status == NodeStatus::Healthy)
        .map(|node| {
            let query = query.to_vec();
            async move {
                let start = Instant::now();
                let result = node.attention.forward(&query, k);
                let latency = start.elapsed();

                match result {
                    Ok((indices, scores)) => {
                        node.metrics.queries_processed += 1;
                        node.metrics.avg_latency_ms =
                            0.9 * node.metrics.avg_latency_ms +
                            0.1 * latency.as_secs_f32() * 1000.0;
                        Some((node.id, indices, scores))
                    },
                    Err(_) => {
                        node.metrics.errors += 1;
                        None
                    }
                }
            }
        })
        .collect();

    let node_results: Vec<_> = futures::future::join_all(node_futures)
        .await
        .into_iter()
        .flatten()
        .collect();

    // Step 3: Check if we have enough responses
    let min_nodes = ((2.0 * num_active_nodes as f32) / 3.0).ceil() as usize;
    if node_results.len() < min_nodes {
        return Err(ConsensusError::InsufficientNodes {
            required: min_nodes,
            available: node_results.len(),
        });
    }

    // Step 4: Detect faulty nodes
    let faulty_nodes = self.voting.detect_faulty_nodes(node_results.clone());
    for &node_id in &faulty_nodes {
        self.suspected_faulty.insert(node_id);
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            node.status = NodeStatus::Suspected;
        }
    }

    // Step 5: Filter out faulty node results
    let filtered_results: Vec<_> = node_results.into_iter()
        .filter(|(node_id, _, _)| !faulty_nodes.contains(node_id))
        .collect();

    // Step 6: Collect votes
    let votes = self.voting.collect_votes(filtered_results.clone());

    // Step 7: Apply consensus
    let threshold = ((2.0 * num_active_nodes as f32) / 3.0).ceil() as usize;
    let mut consensus = self.voting.apply_consensus(votes, threshold);

    // Step 8: Update node agreement metrics
    self.update_node_agreements(&filtered_results, &consensus);

    // Step 9: Update metrics
    let latency = start_time.elapsed();
    self.update_metrics(&consensus, latency);

    Ok(consensus)
}

/// Compute adaptive K based on query characteristics
fn compute_adaptive_k(&self, query: &[f32]) -> usize {
    // Compute query complexity metrics
    let entropy = compute_entropy(query);
    let norm = compute_norm(query);
    let sparsity = compute_sparsity(query);

    // Higher complexity -> more nodes needed
    let complexity_score = 0.4 * entropy + 0.3 * (norm / 10.0) + 0.3 * (1.0 - sparsity);

    // Map complexity to K
    let k = if complexity_score < 0.3 {
        self.config.min_k
    } else if complexity_score < 0.6 {
        (self.config.min_k + self.config.max_k) / 2
    } else {
        self.config.max_k
    };

    k.max(self.config.min_k).min(self.config.max_k)
}

/// Update node agreement rates
fn update_node_agreements(
    &mut self,
    node_results: &[(usize, Vec<usize>, Vec<f32>)],
    consensus: &ConsensusResult
) {
    let consensus_set: HashSet<_> = consensus.consensus_indices.iter().collect();

    for (node_id, indices, _) in node_results {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == *node_id) {
            let node_set: HashSet<_> = indices.iter().collect();
            let agreement = node_set.intersection(&consensus_set).count() as f32 /
                          consensus_set.len() as f32;

            // EMA update
            node.metrics.agreement_rate = 0.9 * node.metrics.agreement_rate + 0.1 * agreement;
        }
    }
}
```

#### 2. Hierarchical Consensus

```rust
/// Hierarchical consensus for efficiency
async fn forward_hierarchical(
    &mut self,
    query: &[f32],
    k: usize
) -> Result<ConsensusResult, ConsensusError> {

    let group_size = self.config.group_size;
    let num_groups = (self.nodes.len() + group_size - 1) / group_size;

    // Level 1: Local consensus in each group
    let mut group_results = Vec::new();

    for group_idx in 0..num_groups {
        let start_idx = group_idx * group_size;
        let end_idx = (start_idx + group_size).min(self.nodes.len());

        // Run consensus within group
        let group_nodes = &mut self.nodes[start_idx..end_idx];
        let local_consensus = self.run_local_consensus(query, k, group_nodes).await?;

        group_results.push(local_consensus);
    }

    // Level 2: Global consensus across group results
    let global_consensus = self.merge_group_results(group_results)?;

    Ok(global_consensus)
}

/// Run consensus within a group of nodes
async fn run_local_consensus(
    &self,
    query: &[f32],
    k: usize,
    nodes: &mut [AttentionNode]
) -> Result<ConsensusResult, ConsensusError> {

    // Similar to forward_consensus but only for subset of nodes
    let node_futures: Vec<_> = nodes.iter_mut()
        .filter(|n| n.status == NodeStatus::Healthy)
        .map(|node| {
            let query = query.to_vec();
            async move {
                node.attention.forward(&query, k)
                    .ok()
                    .map(|(indices, scores)| (node.id, indices, scores))
            }
        })
        .collect();

    let node_results: Vec<_> = futures::future::join_all(node_futures)
        .await
        .into_iter()
        .flatten()
        .collect();

    let votes = self.voting.collect_votes(node_results);
    let threshold = (nodes.len() * 2) / 3;
    Ok(self.voting.apply_consensus(votes, threshold))
}

/// Merge results from multiple groups
fn merge_group_results(
    &self,
    group_results: Vec<ConsensusResult>
) -> Result<ConsensusResult, ConsensusError> {

    // Treat each group's consensus as a "vote"
    let mut global_votes: HashMap<usize, usize> = HashMap::new();
    let mut global_scores: HashMap<usize, Vec<f32>> = HashMap::new();

    for group_result in &group_results {
        for (&idx, &score) in group_result.consensus_indices.iter()
            .zip(group_result.consensus_scores.iter()) {
            *global_votes.entry(idx).or_insert(0) += 1;
            global_scores.entry(idx).or_insert_with(Vec::new).push(score);
        }
    }

    // Require majority of groups to agree
    let threshold = (group_results.len() + 1) / 2;

    let mut consensus_indices = Vec::new();
    let mut consensus_scores = Vec::new();
    let mut confidence = Vec::new();

    for (idx, vote_count) in global_votes {
        if vote_count >= threshold {
            let scores = &global_scores[&idx];
            let avg_score = scores.iter().sum::<f32>() / scores.len() as f32;

            consensus_indices.push(idx);
            consensus_scores.push(avg_score);
            confidence.push(vote_count as f32 / group_results.len() as f32);
        }
    }

    Ok(ConsensusResult {
        consensus_indices,
        consensus_scores,
        confidence,
        consensus_strength: confidence.iter().sum::<f32>() / confidence.len() as f32,
        uncertain_indices: Vec::new(),
        vote_details: Vec::new(),
        participating_nodes: group_results.len(),
    })
}
```

#### 3. Node Variant Creation

```rust
/// Create attention node variants based on strategy
fn create_node_variants(
    base_attention: &dyn AttentionLayer,
    config: &ConsensusConfig
) -> Vec<AttentionNode> {

    let mut nodes = Vec::new();

    for i in 0..config.num_nodes {
        let params = match config.variant_strategy {
            VariantStrategy::RandomInit => NodeParams {
                temperature: 1.0,
                seed: i as u64,
                top_k: 10,
                variant_params: HashMap::new(),
            },

            VariantStrategy::HyperparamVariation => {
                // Vary temperature across nodes
                let temp = 0.5 + (i as f32 / config.num_nodes as f32) * 1.5;
                NodeParams {
                    temperature: temp,
                    seed: 42,
                    top_k: 10,
                    variant_params: HashMap::new(),
                }
            },

            VariantStrategy::MechanismVariation => {
                // Different attention mechanisms
                // (would need polymorphism)
                NodeParams::default()
            },

            VariantStrategy::Bootstrap => {
                // Different data subsets
                NodeParams {
                    temperature: 1.0,
                    seed: i as u64,
                    top_k: 10,
                    variant_params: [("subset_ratio".to_string(), 0.8)].into(),
                }
            },

            VariantStrategy::Hybrid => {
                // Combination
                let temp = 0.8 + (i as f32 / config.num_nodes as f32) * 0.4;
                NodeParams {
                    temperature: temp,
                    seed: i as u64,
                    top_k: 10,
                    variant_params: [("subset_ratio".to_string(), 0.9)].into(),
                }
            },
        };

        nodes.push(AttentionNode {
            id: i,
            attention: base_attention.clone_box(),
            params,
            status: NodeStatus::Healthy,
            metrics: NodeMetrics::default(),
        });
    }

    nodes
}
```

### API Design

```rust
/// Public API for Consensus Attention
pub trait ConsensusLayer {
    /// Create consensus layer
    fn new(
        config: ConsensusConfig,
        base_attention: Box<dyn AttentionLayer>
    ) -> Self;

    /// Forward with consensus
    async fn forward(
        &mut self,
        query: &[f32],
        k: usize
    ) -> Result<ConsensusResult, ConsensusError>;

    /// Get high-confidence results only
    async fn forward_confident(
        &mut self,
        query: &[f32],
        k: usize,
        min_confidence: f32
    ) -> Result<(Vec<usize>, Vec<f32>), ConsensusError>;

    /// Get uncertainty estimate
    fn estimate_uncertainty(&self, query: &[f32]) -> f32;

    /// Report node failure
    fn report_node_failure(&mut self, node_id: usize);

    /// Get node health status
    fn get_node_status(&self) -> Vec<(usize, NodeStatus)>;

    /// Get metrics
    fn get_metrics(&self) -> &ConsensusMetrics;
}

#[derive(Debug, thiserror::Error)]
pub enum ConsensusError {
    #[error("Insufficient nodes: required {required}, available {available}")]
    InsufficientNodes { required: usize, available: usize },

    #[error("No consensus reached")]
    NoConsensus,

    #[error("All nodes failed")]
    AllNodesFailed,

    #[error("Attention error: {0}")]
    AttentionError(String),
}
```

## Integration Points

### Affected Crates/Modules
1. **`ruvector-gnn-core/src/attention/`**
   - Add consensus as meta-attention layer

### New Modules to Create
```
ruvector-gnn-core/src/attention/consensus/
├── mod.rs
├── config.rs
├── node.rs
├── voting/
│   ├── mod.rs
│   ├── byzantine.rs
│   └── majority.rs
├── variants.rs
└── metrics.rs
```

### Dependencies on Other Features
- Can wrap ANY attention mechanism (ESA, PPA, Morphological, etc.)
- Especially useful with Feature 18 (ARL) for security

## Implementation Phases

### Phase 1: Core Consensus (2 weeks)
- Basic voting protocol
- Node management
- Simple majority consensus

### Phase 2: Byzantine Tolerance (2 weeks)
- Byzantine voting protocol
- Faulty node detection
- Recovery mechanisms

### Phase 3: Optimization (1 week)
- Hierarchical consensus
- Adaptive K
- Performance tuning

### Phase 4: Integration (1 week)
- Integrate with all attention types
- Production testing

## Success Metrics

| Metric | Target |
|--------|--------|
| Error Reduction | 70-90% |
| Byzantine Tolerance | ⌊K/3⌋ faults |
| Consensus Rate | >95% |
| Latency Overhead | <3x single node |
| Uncertainty Calibration | <0.1 error |

## Risks and Mitigations

1. **Risk: High Latency**
   - Mitigation: Hierarchical consensus, parallel execution

2. **Risk: Low Consensus Rate**
   - Mitigation: Adaptive K, better node variants

3. **Risk: Node Failures**
   - Mitigation: Health monitoring, redundancy

4. **Risk: Cost (Multiple Attention Calls)**
   - Mitigation: Cache results, adaptive K based on criticality
