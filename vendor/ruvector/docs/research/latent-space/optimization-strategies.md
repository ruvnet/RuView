# Optimization Strategies: Latent Space ↔ Graph Reality

## Executive Summary

This document explores optimization strategies for training GNNs that effectively bridge latent space and graph topology. We examine loss functions, training procedures, regularization techniques, and multi-objective optimization methods specific to the graph learning domain.

**Core Challenge**: Jointly optimize for graph structure preservation, downstream task performance, and latent space quality

---

## 1. Loss Function Taxonomy

### 1.1 Classification of Graph Learning Losses

```
Graph Learning Losses
│
├─ Reconstruction Losses
│  ├─ Link Prediction
│  ├─ Node Feature Reconstruction
│  └─ Graph Structure Reconstruction
│
├─ Contrastive Losses
│  ├─ InfoNCE (current in RuVector)
│  ├─ Local Contrastive (current)
│  ├─ Triplet Loss
│  └─ Deep Graph Infomax
│
├─ Regularization Losses
│  ├─ Spectral (Laplacian)
│  ├─ Sparsity
│  ├─ EWC (current in RuVector)
│  └─ Embedding Normalization
│
├─ Task-Specific Losses
│  ├─ Node Classification (Cross-Entropy)
│  ├─ Link Prediction (BCE)
│  └─ Graph Classification
│
└─ Geometric Losses
   ├─ Distance Preservation
   ├─ Angle Preservation
   └─ Curvature-Based
```

---

## 2. Reconstruction Losses

### 2.1 Link Prediction Loss

**Goal**: Predict edge existence from latent embeddings

**Current RuVector Approach** (Implicit in search.rs):
```rust
// Cosine similarity for neighbor selection
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32
```

**Binary Cross-Entropy Link Loss**:
```rust
pub fn link_prediction_loss(
    embeddings: &[Vec<f32>],
    positive_edges: &[(usize, usize)],
    negative_edges: &[(usize, usize)],
) -> f32 {
    let mut loss = 0.0;

    // Positive edges: should have high similarity
    for &(i, j) in positive_edges {
        let sim = cosine_similarity(&embeddings[i], &embeddings[j]);
        let prob = sigmoid(sim);
        loss -= (prob + 1e-10).ln();  // -log P(edge exists)
    }

    // Negative edges: should have low similarity
    for &(i, j) in negative_edges {
        let sim = cosine_similarity(&embeddings[i], &embeddings[j]);
        let prob = sigmoid(sim);
        loss -= (1.0 - prob + 1e-10).ln();  // -log P(edge doesn't exist)
    }

    loss / (positive_edges.len() + negative_edges.len()) as f32
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}
```

**Negative Sampling Strategies**:

**1. Random Negatives**
```rust
fn sample_random_negatives(
    num_nodes: usize,
    positive_edges: &[(usize, usize)],
    ratio: usize,  // Negatives per positive
) -> Vec<(usize, usize)> {
    let mut negatives = Vec::new();
    let positive_set: HashSet<_> = positive_edges.iter().collect();

    while negatives.len() < positive_edges.len() * ratio {
        let i = rand::random::<usize>() % num_nodes;
        let j = rand::random::<usize>() % num_nodes;

        if i != j && !positive_set.contains(&(i, j)) {
            negatives.push((i, j));
        }
    }

    negatives
}
```

**2. Hard Negatives (Distance-Based)**
```rust
fn sample_hard_negatives_distance(
    embeddings: &[Vec<f32>],
    positive_edges: &[(usize, usize)],
    k_hops: usize,  // Graph distance threshold
    graph: &Graph,
) -> Vec<(usize, usize)> {
    let mut hard_negatives = Vec::new();

    for &(i, _) in positive_edges {
        // Find nodes that are:
        // 1. Latent-close (high embedding similarity)
        // 2. Graph-far (> k_hops away)
        let candidates: Vec<_> = (0..embeddings.len())
            .filter(|&j| {
                let dist = graph.shortest_path_distance(i, j);
                dist > k_hops
            })
            .map(|j| (j, cosine_similarity(&embeddings[i], &embeddings[j])))
            .collect();

        // Sort by similarity (most similar = hardest negative)
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        if let Some((j, _)) = candidates.first() {
            hard_negatives.push((i, *j));
        }
    }

    hard_negatives
}
```

**3. Degree-Corrected Negatives**
```rust
// Sample negatives with similar degree distribution to positives
fn sample_degree_corrected_negatives(
    degrees: &[usize],
    positive_edges: &[(usize, usize)],
    tolerance: f32,
) -> Vec<(usize, usize)> {
    let mut negatives = Vec::new();

    for &(i, j) in positive_edges {
        let target_deg_i = degrees[i] as f32;
        let target_deg_j = degrees[j] as f32;

        // Sample i', j' with similar degrees
        let candidates: Vec<_> = (0..degrees.len())
            .filter(|&k| {
                let deg_k = degrees[k] as f32;
                (deg_k - target_deg_i).abs() / target_deg_i < tolerance
            })
            .collect();

        if candidates.len() >= 2 {
            let idx1 = rand::random::<usize>() % candidates.len();
            let idx2 = rand::random::<usize>() % candidates.len();
            if idx1 != idx2 {
                negatives.push((candidates[idx1], candidates[idx2]));
            }
        }
    }

    negatives
}
```

### 2.2 Node Feature Reconstruction Loss

**Goal**: Reconstruct original node features from embeddings (autoencoder)

```rust
pub struct GraphAutoencoder {
    encoder: RuvectorLayer,
    decoder: Linear,
}

impl GraphAutoencoder {
    fn reconstruction_loss(
        &self,
        node_features: &[Vec<f32>],
        neighbor_structure: &[Vec<Vec<f32>>],
    ) -> f32 {
        let mut total_loss = 0.0;

        for (i, features) in node_features.iter().enumerate() {
            // Encode
            let embedding = self.encoder.forward(
                features,
                &neighbor_structure[i],
                &vec![1.0; neighbor_structure[i].len()],
            );

            // Decode
            let reconstructed = self.decoder.forward(&embedding);

            // MSE loss
            let mse = features.iter()
                .zip(reconstructed.iter())
                .map(|(f, r)| (f - r).powi(2))
                .sum::<f32>();

            total_loss += mse;
        }

        total_loss / node_features.len() as f32
    }
}
```

**Variants**:
- **MSE**: Mean Squared Error (continuous features)
- **BCE**: Binary Cross-Entropy (binary features)
- **Categorical CE**: For discrete features

### 2.3 Graph Structure Reconstruction

**Adjacency Matrix Reconstruction**:
```rust
pub fn adjacency_reconstruction_loss(
    embeddings: &[Vec<f32>],
    true_adjacency: &SparseMatrix,
) -> f32 {
    let n = embeddings.len();
    let mut loss = 0.0;

    // Predicted adjacency: A'[i,j] = σ(h_i^T h_j)
    for i in 0..n {
        for j in i+1..n {
            let score = embeddings[i].iter()
                .zip(embeddings[j].iter())
                .map(|(a, b)| a * b)
                .sum::<f32>();

            let pred_adj = sigmoid(score);
            let true_adj = if true_adjacency.has_edge(i, j) { 1.0 } else { 0.0 };

            // Binary cross-entropy
            if true_adj == 1.0 {
                loss -= (pred_adj + 1e-10).ln();
            } else {
                loss -= (1.0 - pred_adj + 1e-10).ln();
            }
        }
    }

    loss / (n * (n-1) / 2) as f32
}
```

**Sparse Variant** (For large graphs):
```rust
// Only compute loss on edges + sampled non-edges
pub fn sparse_adjacency_loss(
    embeddings: &[Vec<f32>],
    edges: &[(usize, usize)],
    num_negative_samples: usize,
) -> f32 {
    let mut loss = 0.0;

    // Positive samples
    for &(i, j) in edges {
        let score = dot_product(&embeddings[i], &embeddings[j]);
        loss -= sigmoid(score).ln();
    }

    // Negative samples
    let negatives = sample_random_negatives(embeddings.len(), edges, num_negative_samples);
    for (i, j) in negatives {
        let score = dot_product(&embeddings[i], &embeddings[j]);
        loss -= (1.0 - sigmoid(score) + 1e-10).ln();
    }

    loss / (edges.len() + num_negative_samples) as f32
}
```

---

## 3. Contrastive Losses (Current in RuVector)

### 3.1 InfoNCE Loss (Deep Dive)

**Current Implementation** (`training.rs:362-411`):
```rust
pub fn info_nce_loss(
    anchor: &[f32],
    positives: &[&[f32]],
    negatives: &[&[f32]],
    temperature: f32,
) -> f32
```

**Mathematical Properties**:

**1. Temperature Scaling**:
```
τ → 0: Hard selection (argmax-like)
τ → ∞: Uniform (all samples weighted equally)
τ = 0.07: Standard for vision (SimCLR)
τ = 0.1-0.5: Common for graphs
```

**Temperature Effect**:
```rust
fn analyze_temperature_effect() {
    let anchor = vec![1.0, 0.0, 0.0];
    let positive = vec![0.9, 0.1, 0.0];
    let negative = vec![0.0, 1.0, 0.0];

    for temp in [0.01, 0.07, 0.1, 0.5, 1.0] {
        let loss = info_nce_loss(&anchor, &[&positive], &[&negative], temp);
        println!("Temperature {}: Loss = {}", temp, loss);
    }
}
```

**2. Gradient Analysis**:
```
∂L_InfoNCE / ∂h_v ∝ (h_+ - Σ_i w_i h_i)

where w_i = exp(sim(h_v, h_i) / τ) / Z
```

**Interpretation**: Gradient pulls anchor toward positive, pushes away from weighted average of negatives

### 3.2 Local Contrastive Loss (Graph-Specific)

**Current Implementation** (`training.rs:444-462`):
```rust
pub fn local_contrastive_loss(
    node_embedding: &[f32],
    neighbor_embeddings: &[Vec<f32>],
    non_neighbor_embeddings: &[Vec<f32>],
    temperature: f32,
) -> f32
```

**Enhancement: Multi-Hop Contrastive**:
```rust
pub fn multi_hop_contrastive_loss(
    node_embedding: &[f32],
    k_hop_neighbors: &HashMap<usize, Vec<Vec<f32>>>,  // k -> neighbors at distance k
    non_neighbors: &[Vec<f32>],
    temperature: f32,
    hop_weights: &[f32],  // Weight for each hop distance
) -> f32 {
    let mut total_loss = 0.0;

    for (k, neighbors) in k_hop_neighbors {
        if neighbors.is_empty() {
            continue;
        }

        let positives: Vec<&[f32]> = neighbors.iter().map(|n| n.as_slice()).collect();
        let negatives: Vec<&[f32]> = non_neighbors.iter().map(|n| n.as_slice()).collect();

        let loss = info_nce_loss(node_embedding, &positives, &negatives, temperature);

        // Weight by hop distance (closer = more important)
        total_loss += hop_weights[*k] * loss;
    }

    total_loss
}
```

### 3.3 Triplet Loss

**Alternative to InfoNCE**:
```
L_triplet = max(0, ||h_v - h_+||² - ||h_v - h_-||² + margin)
```

**Implementation**:
```rust
pub fn triplet_loss(
    anchor: &[f32],
    positive: &[f32],
    negative: &[f32],
    margin: f32,
) -> f32 {
    let pos_dist = l2_distance_squared(anchor, positive);
    let neg_dist = l2_distance_squared(anchor, negative);

    (pos_dist - neg_dist + margin).max(0.0)
}

pub fn batch_triplet_loss(
    anchors: &[Vec<f32>],
    positives: &[Vec<f32>],
    negatives: &[Vec<f32>],
    margin: f32,
) -> f32 {
    anchors.iter()
        .zip(positives.iter())
        .zip(negatives.iter())
        .map(|((a, p), n)| triplet_loss(a, p, n, margin))
        .sum::<f32>() / anchors.len() as f32
}
```

**Hard Triplet Mining**:
```rust
fn mine_hard_triplets(
    embeddings: &[Vec<f32>],
    edges: &[(usize, usize)],
    k: usize,  // Top-k hardest
) -> Vec<(usize, usize, usize)> {  // (anchor, positive, negative)
    let mut triplets = Vec::new();

    for &(i, j) in edges {
        // j is positive for i
        // Find hardest negative: closest non-neighbor
        let mut candidates: Vec<(usize, f32)> = (0..embeddings.len())
            .filter(|&k| k != i && k != j && !edges.contains(&(i, k)))
            .map(|k| (k, l2_distance_squared(&embeddings[i], &embeddings[k])))
            .collect();

        // Sort by distance (ascending = closest = hardest)
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        for (neg_idx, _) in candidates.iter().take(k) {
            triplets.push((i, j, *neg_idx));
        }
    }

    triplets
}
```

---

## 4. Regularization Losses

### 4.1 Spectral Regularization (Laplacian)

**Goal**: Smooth embeddings along graph structure

**Laplacian Smoothness**:
```
L_Laplacian = Tr(H^T L H) = Σ_{(i,j) ∈ E} ||h_i - h_j||²
```

**Implementation**:
```rust
pub fn laplacian_regularization(
    embeddings: &[Vec<f32>],
    edges: &[(usize, usize)],
    edge_weights: Option<&[f32]>,  // Optional weighted graph
) -> f32 {
    let mut loss = 0.0;

    for (idx, &(i, j)) in edges.iter().enumerate() {
        let diff = subtract(&embeddings[i], &embeddings[j]);
        let norm_sq = l2_norm_squared(&diff);

        let weight = edge_weights.map(|w| w[idx]).unwrap_or(1.0);
        loss += weight * norm_sq;
    }

    loss / edges.len() as f32
}
```

**Normalized Laplacian**:
```rust
pub fn normalized_laplacian_regularization(
    embeddings: &[Vec<f32>],
    edges: &[(usize, usize)],
    degrees: &[usize],
) -> f32 {
    let mut loss = 0.0;

    for &(i, j) in edges {
        let diff = subtract(&embeddings[i], &embeddings[j]);
        let norm_sq = l2_norm_squared(&diff);

        // Normalize by sqrt of degrees
        let normalization = (degrees[i] as f32 * degrees[j] as f32).sqrt();
        loss += norm_sq / normalization;
    }

    loss / edges.len() as f32
}
```

### 4.2 Elastic Weight Consolidation (EWC) - Current

**Implementation** (`ewc.rs:1-584`):
```rust
pub struct ElasticWeightConsolidation {
    fisher_diag: Vec<f32>,      // Importance weights
    anchor_weights: Vec<f32>,   // Previous task optimal weights
    lambda: f32,                // Regularization strength
    active: bool,
}
```

**Loss Term**:
```
L_EWC = (λ/2) Σ_i F_i (θ_i - θ*_i)²
```

**Enhanced EWC for GNNs**:
```rust
pub struct GNNElasticWeightConsolidation {
    ewc_per_layer: Vec<ElasticWeightConsolidation>,
    layer_importance: Vec<f32>,  // Different λ per layer
}

impl GNNElasticWeightConsolidation {
    fn compute_total_penalty(&self, current_weights: &[Vec<f32>]) -> f32 {
        self.ewc_per_layer.iter()
            .zip(self.layer_importance.iter())
            .zip(current_weights.iter())
            .map(|((ewc, &importance), weights)| {
                importance * ewc.penalty(weights)
            })
            .sum()
    }

    fn adaptive_lambda_by_importance(&mut self) {
        // Adjust lambda based on Fisher information magnitude
        for ewc in &mut self.ewc_per_layer {
            let avg_fisher = ewc.fisher_diag().iter().sum::<f32>()
                / ewc.fisher_diag().len() as f32;

            // Higher Fisher = more important = higher lambda
            ewc.set_lambda(1000.0 * avg_fisher);
        }
    }
}
```

### 4.3 Embedding Normalization

**Unit Sphere Constraint**:
```
||h_v|| = 1 for all v
```

**Soft Constraint (Regularization)**:
```rust
pub fn embedding_norm_regularization(
    embeddings: &[Vec<f32>],
    target_norm: f32,
) -> f32 {
    embeddings.iter()
        .map(|h| {
            let norm = l2_norm(h);
            (norm - target_norm).powi(2)
        })
        .sum::<f32>() / embeddings.len() as f32
}
```

**Hard Constraint (Projection)**:
```rust
pub fn normalize_embeddings(embeddings: &mut [Vec<f32>], target_norm: f32) {
    for h in embeddings.iter_mut() {
        let norm = l2_norm(h);
        if norm > 1e-10 {
            let scale = target_norm / norm;
            for x in h.iter_mut() {
                *x *= scale;
            }
        }
    }
}
```

**Benefits**:
- Cosine similarity becomes dot product
- Prevents embedding collapse
- Bounded optimization landscape

### 4.4 Orthogonality Regularization

**Goal**: Encourage diverse, non-redundant features

```rust
pub fn orthogonality_regularization(
    embeddings: &[Vec<f32>],
) -> f32 {
    // H^T H should be close to identity
    let n = embeddings.len();
    let d = embeddings[0].len();

    let mut gram_matrix = vec![vec![0.0; n]; n];

    for i in 0..n {
        for j in i..n {
            let dot = dot_product(&embeddings[i], &embeddings[j]);
            gram_matrix[i][j] = dot;
            gram_matrix[j][i] = dot;
        }
    }

    // Measure deviation from identity
    let mut loss = 0.0;
    for i in 0..n {
        for j in 0..n {
            let target = if i == j { 1.0 } else { 0.0 };
            loss += (gram_matrix[i][j] - target).powi(2);
        }
    }

    loss / (n * n) as f32
}
```

---

## 5. Multi-Objective Optimization

### 5.1 Weighted Combination

**Total Loss**:
```
L_total = λ_task L_task
        + λ_contrast L_contrast
        + λ_recon L_recon
        + λ_spectral L_spectral
        + λ_ewc L_ewc
```

**Implementation**:
```rust
pub struct MultiObjectiveLoss {
    lambda_task: f32,
    lambda_contrast: f32,
    lambda_reconstruction: f32,
    lambda_spectral: f32,
    lambda_ewc: f32,
}

impl MultiObjectiveLoss {
    fn compute_total_loss(
        &self,
        task_loss: f32,
        contrastive_loss: f32,
        reconstruction_loss: f32,
        spectral_loss: f32,
        ewc_penalty: f32,
    ) -> f32 {
        self.lambda_task * task_loss
            + self.lambda_contrast * contrastive_loss
            + self.lambda_reconstruction * reconstruction_loss
            + self.lambda_spectral * spectral_loss
            + self.lambda_ewc * ewc_penalty
    }

    // Dynamic weight adjustment based on loss magnitudes
    fn balance_weights(&mut self, losses: &LossComponents) {
        // Normalize so all losses contribute roughly equally initially
        let total_weighted = self.lambda_task * losses.task
            + self.lambda_contrast * losses.contrastive
            + self.lambda_reconstruction * losses.reconstruction
            + self.lambda_spectral * losses.spectral
            + self.lambda_ewc * losses.ewc;

        // Adjust lambdas to balance contributions
        let target_contribution = total_weighted / 5.0;

        self.lambda_task *= target_contribution / (self.lambda_task * losses.task).max(1e-10);
        self.lambda_contrast *= target_contribution / (self.lambda_contrast * losses.contrastive).max(1e-10);
        // ... etc
    }
}
```

### 5.2 Curriculum Learning

**Idea**: Schedule loss weights over training

**Example Schedule**:
```
Early training (epochs 0-10):
  - High λ_reconstruction (learn basic features)
  - Low λ_contrast (don't force structure yet)

Mid training (epochs 10-50):
  - Increase λ_contrast (encode graph topology)
  - Introduce λ_spectral (smooth embeddings)

Late training (epochs 50+):
  - High λ_task (optimize for downstream)
  - Introduce λ_ewc (if continual learning)
```

**Implementation**:
```rust
pub struct CurriculumSchedule {
    current_epoch: usize,
    schedules: HashMap<String, Box<dyn Fn(usize) -> f32>>,
}

impl CurriculumSchedule {
    fn new() -> Self {
        let mut schedules: HashMap<String, Box<dyn Fn(usize) -> f32>> = HashMap::new();

        // Reconstruction: start high, decrease
        schedules.insert(
            "reconstruction".to_string(),
            Box::new(|epoch| {
                1.0 * (-(epoch as f32) / 50.0).exp()
            })
        );

        // Contrastive: start low, increase, plateau
        schedules.insert(
            "contrastive".to_string(),
            Box::new(|epoch| {
                if epoch < 10 {
                    0.1 + 0.9 * (epoch as f32 / 10.0)
                } else {
                    1.0
                }
            })
        );

        // Task: start low, ramp up late
        schedules.insert(
            "task".to_string(),
            Box::new(|epoch| {
                if epoch < 50 {
                    0.1
                } else {
                    0.1 + 0.9 * ((epoch - 50) as f32 / 50.0).min(1.0)
                }
            })
        );

        Self {
            current_epoch: 0,
            schedules,
        }
    }

    fn get_weight(&self, loss_name: &str) -> f32 {
        self.schedules.get(loss_name)
            .map(|f| f(self.current_epoch))
            .unwrap_or(1.0)
    }

    fn step(&mut self) {
        self.current_epoch += 1;
    }
}
```

### 5.3 Gradient Surgery (Conflicting Objectives)

**Problem**: Gradients from different losses may conflict

**PCGrad (Projected Conflicting Gradients)**:
```rust
pub fn project_conflicting_gradients(
    grad_task: &[f32],
    grad_contrast: &[f32],
) -> (Vec<f32>, Vec<f32>) {
    let dot = dot_product(grad_task, grad_contrast);

    if dot < 0.0 {
        // Gradients conflict, project
        let grad_contrast_norm_sq = l2_norm_squared(grad_contrast);

        // Project grad_task away from grad_contrast
        let projection_scale = dot / grad_contrast_norm_sq;
        let grad_task_projected: Vec<f32> = grad_task.iter()
            .zip(grad_contrast.iter())
            .map(|(&gt, &gc)| gt - projection_scale * gc)
            .collect();

        (grad_task_projected, grad_contrast.to_vec())
    } else {
        // No conflict
        (grad_task.to_vec(), grad_contrast.to_vec())
    }
}
```

---

## 6. Training Strategies

### 6.1 Online Learning (Current in RuVector)

**Config** (`training.rs:313-328`):
```rust
pub struct OnlineConfig {
    local_steps: usize,
    propagate_updates: bool,
}
```

**Enhanced Online Training**:
```rust
pub struct OnlineGNNTrainer {
    model: RuvectorLayer,
    optimizer: Optimizer,
    config: OnlineConfig,
    replay_buffer: ReplayBuffer,
}

impl OnlineGNNTrainer {
    fn update_on_new_node(
        &mut self,
        new_node_features: &[f32],
        neighbors: &[Vec<f32>],
        labels: Option<usize>,
    ) {
        // 1. Forward pass with current model
        let embedding = self.model.forward(new_node_features, neighbors, &[]);

        // 2. Compute loss
        let loss = if let Some(label) = labels {
            // Supervised: classification loss
            self.classification_loss(&embedding, label)
        } else {
            // Unsupervised: contrastive loss
            self.contrastive_loss(&embedding, neighbors)
        };

        // 3. Local optimization steps
        for _ in 0..self.config.local_steps {
            let grads = self.compute_gradients(loss);
            self.optimizer.step(&mut self.model.weights, &grads);
        }

        // 4. Store in replay buffer (prevent catastrophic forgetting)
        self.replay_buffer.add(new_node_features, neighbors, labels);

        // 5. Periodic replay
        if self.replay_buffer.should_replay() {
            self.replay_past_experiences();
        }
    }

    fn replay_past_experiences(&mut self) {
        let samples = self.replay_buffer.sample(32);

        for (features, neighbors, label) in samples {
            // Re-train on past data
            self.update_on_new_node(&features, &neighbors, label);
        }
    }
}
```

### 6.2 Batch Training with Graph Sampling

**Challenge**: Full-batch training on large graphs is expensive

**Solution**: Sample subgraphs for each batch

**Neighbor Sampling**:
```rust
pub fn sample_neighbors(
    node: usize,
    all_neighbors: &[Vec<usize>],
    sample_size: usize,
) -> Vec<usize> {
    let neighbors = &all_neighbors[node];

    if neighbors.len() <= sample_size {
        return neighbors.clone();
    }

    // Uniform sampling
    let mut sampled = Vec::new();
    let mut rng = rand::thread_rng();

    while sampled.len() < sample_size {
        let idx = rng.gen_range(0..neighbors.len());
        if !sampled.contains(&neighbors[idx]) {
            sampled.push(neighbors[idx]);
        }
    }

    sampled
}
```

**Layer-wise Sampling** (GraphSAINT):
```rust
pub fn layer_wise_sampling(
    root_nodes: &[usize],
    all_neighbors: &[Vec<usize>],
    num_layers: usize,
    sample_sizes: &[usize],  // Sample size per layer
) -> Vec<Vec<Vec<usize>>> {  // [layer][node][neighbors]
    let mut sampled_neighborhoods = vec![Vec::new(); num_layers];

    let mut current_frontier = root_nodes.to_vec();

    for layer in 0..num_layers {
        let mut next_frontier = Vec::new();

        for &node in &current_frontier {
            let neighbors = sample_neighbors(node, all_neighbors, sample_sizes[layer]);
            sampled_neighborhoods[layer].push(neighbors.clone());
            next_frontier.extend(neighbors);
        }

        current_frontier = next_frontier;
    }

    sampled_neighborhoods
}
```

### 6.3 Meta-Learning for Few-Shot Graph Learning

**MAML (Model-Agnostic Meta-Learning) for Graphs**:
```rust
pub struct GraphMAML {
    model: RuvectorLayer,
    meta_lr: f32,
    inner_lr: f32,
    inner_steps: usize,
}

impl GraphMAML {
    fn meta_train_step(
        &mut self,
        tasks: &[GraphTask],  // Multiple graph learning tasks
    ) -> f32 {
        let mut meta_gradients = vec![0.0; self.model.num_parameters()];

        for task in tasks {
            // 1. Clone model for inner loop
            let mut task_model = self.model.clone();

            // 2. Inner loop: adapt to this task
            for _ in 0..self.inner_steps {
                let loss = self.compute_task_loss(&task_model, &task.support_set);
                let grads = self.compute_gradients(loss);

                // Update task model
                task_model.update_parameters(&grads, self.inner_lr);
            }

            // 3. Compute loss on query set with adapted model
            let query_loss = self.compute_task_loss(&task_model, &task.query_set);
            let query_grads = self.compute_gradients(query_loss);

            // 4. Accumulate meta-gradients
            for (i, &grad) in query_grads.iter().enumerate() {
                meta_gradients[i] += grad;
            }
        }

        // 5. Meta-update
        let avg_meta_grads: Vec<f32> = meta_gradients.iter()
            .map(|&g| g / tasks.len() as f32)
            .collect();

        self.model.update_parameters(&avg_meta_grads, self.meta_lr);

        // Return average meta-loss
        tasks.iter()
            .map(|task| self.compute_task_loss(&self.model, &task.query_set))
            .sum::<f32>() / tasks.len() as f32
    }
}
```

---

## 7. Hyperparameter Optimization

### 7.1 Key Hyperparameters for GNNs

**Architecture**:
- `num_layers`: 2-8 typical
- `hidden_dim`: 64-512
- `num_heads`: 1-8 (attention)
- `dropout`: 0.0-0.5

**Training**:
- `learning_rate`: 1e-4 to 1e-2
- `temperature`: 0.01-1.0 (contrastive)
- `lambda_*`: Loss weights (0.0-10.0)
- `batch_size`: 32-512

**HNSW-Specific**:
- `M`: Neighbors per layer (16-64)
- `ef_construction`: Search depth (100-500)
- `num_hnsw_layers`: Typically log(N)

### 7.2 Grid Search

```rust
pub fn grid_search_hyperparameters(
    param_grid: &HashMap<String, Vec<f32>>,
    validation_set: &Dataset,
) -> HashMap<String, f32> {
    let mut best_params = HashMap::new();
    let mut best_score = f32::NEG_INFINITY;

    // Generate all combinations
    let combinations = generate_combinations(param_grid);

    for params in combinations {
        // Train model with these params
        let model = train_model_with_params(&params, validation_set);

        // Evaluate
        let score = evaluate_model(&model, validation_set);

        if score > best_score {
            best_score = score;
            best_params = params.clone();
        }
    }

    best_params
}
```

### 7.3 Bayesian Optimization

**Use Gaussian Process to model hyperparameter → performance**:
```rust
pub struct BayesianHyperparamOptimizer {
    gp: GaussianProcess,
    acquisition_fn: AcquisitionFunction,
    evaluated_points: Vec<(HashMap<String, f32>, f32)>,
}

impl BayesianHyperparamOptimizer {
    fn suggest_next_params(&self) -> HashMap<String, f32> {
        // Maximize acquisition function (e.g., Expected Improvement)
        self.acquisition_fn.maximize(&self.gp)
    }

    fn observe(&mut self, params: HashMap<String, f32>, score: f32) {
        self.evaluated_points.push((params.clone(), score));
        self.gp.update(&self.evaluated_points);
    }

    fn optimize(&mut self, num_iterations: usize) -> HashMap<String, f32> {
        for _ in 0..num_iterations {
            let params = self.suggest_next_params();
            let score = train_and_evaluate(&params);
            self.observe(params, score);
        }

        // Return best observed
        self.evaluated_points.iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(p, _)| p.clone())
            .unwrap()
    }
}
```

---

## 8. Distributed and Efficient Training

### 8.1 Data Parallelism

**Distribute batches across GPUs/nodes**:
```rust
pub fn distributed_training(
    model: &RuvectorLayer,
    dataset: &GraphDataset,
    num_workers: usize,
) {
    // 1. Replicate model to all workers
    let models: Vec<_> = (0..num_workers)
        .map(|_| model.clone())
        .collect();

    // 2. Split data
    let batches_per_worker = dataset.len() / num_workers;

    // 3. Train in parallel
    let gradients: Vec<_> = (0..num_workers)
        .into_par_iter()
        .map(|worker_id| {
            let start = worker_id * batches_per_worker;
            let end = start + batches_per_worker;
            let local_data = &dataset[start..end];

            // Local forward + backward
            train_on_subset(&models[worker_id], local_data)
        })
        .collect();

    // 4. Aggregate gradients (AllReduce)
    let avg_gradients = average_gradients(&gradients);

    // 5. Update global model
    model.apply_gradients(&avg_gradients);
}
```

### 8.2 Model Parallelism (Large Graphs)

**Partition graph across devices**:
```rust
pub struct DistributedGraph {
    partitions: Vec<GraphPartition>,
    partition_mapping: HashMap<usize, usize>,  // node_id -> partition_id
}

impl DistributedGraph {
    fn forward_distributed(
        &self,
        node_id: usize,
        models: &[RuvectorLayer],
    ) -> Vec<f32> {
        let partition_id = self.partition_mapping[&node_id];
        let partition = &self.partitions[partition_id];

        // Get local neighbors
        let local_neighbors = partition.get_neighbors(node_id);

        // Get remote neighbors (cross-partition edges)
        let remote_neighbors = partition.get_remote_neighbors(node_id);

        // Fetch remote embeddings via communication
        let remote_embeddings = self.fetch_remote_embeddings(&remote_neighbors);

        // Combine local and remote
        let all_neighbors = [&local_neighbors[..], &remote_embeddings[..]].concat();

        // Forward pass on local model
        models[partition_id].forward(
            &partition.get_features(node_id),
            &all_neighbors,
            &vec![1.0; all_neighbors.len()],
        )
    }
}
```

---

## 9. Implementation Recommendations for RuVector

### 9.1 Immediate Enhancements (Week 1-2)

**1. Hard Negative Sampling**
```rust
// Modify local_contrastive_loss to use hard negatives
let hard_negatives = sample_hard_negatives_distance(
    node_embedding,
    all_embeddings,
    neighbor_indices,
    k_negatives,
);
```

**2. Spectral Regularization**
```rust
// Add to total loss
let spectral_loss = laplacian_regularization(embeddings, edges, None);
total_loss += lambda_spectral * spectral_loss;
```

**3. Dynamic Loss Weight Balancing**
```rust
// Automatically balance loss contributions
let mut loss_config = MultiObjectiveLoss::new();
loss_config.balance_weights(&current_losses);
```

### 9.2 Short-Term (Month 1)

**4. Curriculum Learning Schedule**
```rust
let mut curriculum = CurriculumSchedule::new();

for epoch in 0..num_epochs {
    let lambda_contrast = curriculum.get_weight("contrastive");
    let lambda_recon = curriculum.get_weight("reconstruction");

    // Train with scheduled weights
    train_epoch(lambda_contrast, lambda_recon);

    curriculum.step();
}
```

**5. Online Learning with Replay**
```rust
let mut trainer = OnlineGNNTrainer::new(model, replay_buffer_size=1000);

for new_node in streaming_data {
    trainer.update_on_new_node(&new_node.features, &new_node.neighbors, None);
}
```

### 9.3 Medium-Term (Quarter 1)

**6. Meta-Learning for Few-Shot**
```rust
let mut maml = GraphMAML::new(meta_lr=0.001, inner_lr=0.01);

for epoch in 0..meta_epochs {
    let tasks = sample_tasks(task_distribution, k_shot=5);
    maml.meta_train_step(&tasks);
}
```

**7. Distributed Training**
```rust
let distributed_graph = DistributedGraph::partition(graph, num_partitions=4);
let models = replicate_model(base_model, num_partitions);

distributed_training(&distributed_graph, &models);
```

---

## 10. Benchmarking and Evaluation

### 10.1 Loss Tracking

```rust
pub struct LossTracker {
    history: HashMap<String, Vec<f32>>,
}

impl LossTracker {
    fn log(&mut self, loss_name: &str, value: f32) {
        self.history.entry(loss_name.to_string())
            .or_insert_with(Vec::new)
            .push(value);
    }

    fn plot_losses(&self) {
        // Visualization: loss curves over training
        for (name, values) in &self.history {
            println!("{}: {:?}", name, values);
        }
    }

    fn detect_overfitting(&self) -> bool {
        // Compare train vs. validation loss
        let train_loss = self.history.get("train").unwrap();
        let val_loss = self.history.get("validation").unwrap();

        // If validation loss increasing while train decreasing
        train_loss.last().unwrap() < train_loss[0]
            && val_loss.last().unwrap() > val_loss[val_loss.len() / 2]
    }
}
```

### 10.2 Metrics

**1. Latent Space Quality**:
- Embedding norm distribution
- Pairwise distance distribution
- Nearest neighbor recall

**2. Graph Preservation**:
- Link prediction AUC
- Triangle closure accuracy
- Community detection modularity

**3. Downstream Performance**:
- Node classification accuracy
- Graph classification accuracy
- Search quality (Recall@K)

---

## References

### Papers
1. **Contrastive Learning**: Chen et al. (2020) - SimCLR, Oord et al. (2018) - CPC
2. **Meta-Learning**: Finn et al. (2017) - MAML
3. **EWC**: Kirkpatrick et al. (2017) - Overcoming Catastrophic Forgetting
4. **Curriculum**: Bengio et al. (2009) - Curriculum Learning
5. **Hard Negatives**: Schroff et al. (2015) - FaceNet (Triplet Loss)
6. **Spectral**: Belkin & Niyogi (2003) - Laplacian Eigenmaps
7. **Multi-Objective**: Yu et al. (2020) - Gradient Surgery

### RuVector Code
- `crates/ruvector-gnn/src/training.rs` - Current losses and optimizers
- `crates/ruvector-gnn/src/ewc.rs` - Elastic Weight Consolidation

---

**Document Version**: 1.0
**Last Updated**: 2025-11-30
**Author**: RuVector Research Team
