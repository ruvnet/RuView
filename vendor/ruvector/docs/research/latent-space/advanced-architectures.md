# Advanced GNN Architectures for Latent-Graph Interplay

## Executive Summary

This document surveys cutting-edge GNN architectures that push beyond traditional message passing to better capture the interplay between latent space representations and graph topology. We focus on architectures particularly relevant to hierarchical graphs like HNSW.

**Key Themes**: Graph Transformers, Hyperbolic GNNs, Neural ODEs, Equivariant Networks, Generative Models

---

## 1. Graph Transformers

### 1.1 Motivation

**Limitations of Message Passing**:
- Limited receptive field (k-hop with k layers)
- Over-smoothing with many layers
- Difficulty capturing long-range dependencies

**Solution**: Replace message passing with full attention

### 1.2 Graphormer Architecture

**Key Innovation**: Structural encodings + Transformer attention

**Paper**: Ying et al. (2021) - "Do Transformers Really Perform Bad for Graph Representation?"

**Architecture**:
```
Input: Graph G = (V, E) with features X
Output: Node embeddings H

1. Centrality Encoding:
   z_v = Embed(degree(v))

2. Spatial Encoding (Shortest Path):
   b_ij = Embed(SP_distance(i, j))

3. Edge Encoding:
   e_ij = Embed(edge_features(i, j))

4. Transformer Attention:
   Attention(Q, K, V) = softmax((QK^T + B) / √d) V

   where B[i,j] = b_ij (spatial bias)

5. Multi-layer stacking with LayerNorm
```

**Implementation Sketch**:
```rust
pub struct Graphormer {
    num_layers: usize,
    hidden_dim: usize,
    num_heads: usize,

    // Encoding layers
    centrality_embedding: Embedding,
    spatial_embedding: Embedding,
    edge_embedding: Embedding,

    // Transformer layers
    transformer_layers: Vec<GraphTransformerLayer>,
}

pub struct GraphTransformerLayer {
    attention: MultiHeadAttention,
    ffn: FeedForwardNetwork,
    norm1: LayerNorm,
    norm2: LayerNorm,
}

impl Graphormer {
    fn forward(
        &self,
        node_features: &[Vec<f32>],
        edge_index: &[(usize, usize)],
        edge_features: &[Vec<f32>],
        shortest_paths: &Array2<usize>,  // Precomputed SP distances
        degrees: &[usize],
    ) -> Vec<Vec<f32>> {
        // 1. Add centrality encoding
        let mut h: Vec<Vec<f32>> = node_features.iter()
            .zip(degrees.iter())
            .map(|(feat, &deg)| {
                let cent_enc = self.centrality_embedding.forward(deg);
                concatenate(feat, &cent_enc)
            })
            .collect();

        // 2. Compute spatial and edge biases
        let spatial_bias = self.compute_spatial_bias(shortest_paths);
        let edge_bias = self.compute_edge_bias(edge_index, edge_features);

        // 3. Transformer layers
        for layer in &self.transformer_layers {
            h = layer.forward(&h, &spatial_bias, &edge_bias);
        }

        h
    }

    fn compute_spatial_bias(&self, shortest_paths: &Array2<usize>) -> Array2<f32> {
        let n = shortest_paths.nrows();
        let mut bias = Array2::zeros((n, n));

        for i in 0..n {
            for j in 0..n {
                let sp_dist = shortest_paths[(i, j)];
                let sp_encoding = self.spatial_embedding.forward(sp_dist);
                bias[(i, j)] = sp_encoding[0];  // Scalar bias
            }
        }

        bias
    }

    fn compute_edge_bias(
        &self,
        edge_index: &[(usize, usize)],
        edge_features: &[Vec<f32>],
    ) -> HashMap<(usize, usize), f32> {
        edge_index.iter()
            .zip(edge_features.iter())
            .map(|(&(i, j), feat)| {
                let edge_enc = self.edge_embedding.forward_features(feat);
                ((i, j), edge_enc[0])
            })
            .collect()
    }
}

impl GraphTransformerLayer {
    fn forward(
        &self,
        x: &[Vec<f32>],
        spatial_bias: &Array2<f32>,
        edge_bias: &HashMap<(usize, usize), f32>,
    ) -> Vec<Vec<f32>> {
        // 1. Multi-head attention with structural biases
        let attn_out = self.attention.forward_with_bias(x, spatial_bias, edge_bias);

        // 2. Residual + Norm
        let x_norm1 = self.norm1.forward(&add_residual(x, &attn_out));

        // 3. Feed-forward
        let ffn_out = self.ffn.forward(&x_norm1);

        // 4. Residual + Norm
        self.norm2.forward(&add_residual(&x_norm1, &ffn_out))
    }
}
```

**Benefits for HNSW**:
- **Global Attention**: All nodes can attend to all others
- **Structural Encoding**: Shortest paths encode HNSW layer information
- **Edge Features**: Naturally incorporates edge weights/attributes

**Challenges**:
- **O(n²) complexity**: Expensive for large graphs
- **Memory**: Quadratic attention matrix
- **Loss of Inductive Bias**: Needs more data than message passing

### 1.3 GPS (General, Powerful, Scalable Graph Transformer)

**Paper**: Rampášek et al. (2022)

**Key Idea**: Combine message passing + attention

```
GPS Layer = Message Passing + Global Attention + FFN

h_v^{l+1} = h_v^l + MLP(MP(h_v^l) || GlobalAttn(h_v^l))
```

**Advantages**:
- Best of both worlds (local + global)
- More efficient than pure attention
- Strong inductive bias from message passing

**Implementation**:
```rust
pub struct GPSLayer {
    local_mp: RuvectorLayer,          // Local message passing
    global_attn: MultiHeadAttention,  // Global attention
    fusion: Linear,                   // Combine local + global
    ffn: FeedForwardNetwork,
    norm: LayerNorm,
}

impl GPSLayer {
    fn forward(
        &self,
        node_features: &[Vec<f32>],
        neighbor_indices: &[Vec<usize>],
        all_node_features: &[Vec<f32>],  // For global attention
    ) -> Vec<Vec<f32>> {
        let n = node_features.len();
        let mut outputs = Vec::new();

        for (i, features) in node_features.iter().enumerate() {
            // 1. Local message passing
            let neighbors: Vec<Vec<f32>> = neighbor_indices[i].iter()
                .map(|&j| all_node_features[j].clone())
                .collect();

            let local_out = self.local_mp.forward(
                features,
                &neighbors,
                &vec![1.0; neighbors.len()],
            );

            // 2. Global attention (attend to all nodes)
            let global_out = self.global_attn.forward(
                features,
                all_node_features,
                all_node_features,
            );

            // 3. Fusion
            let combined = self.fusion.forward(
                &concatenate(&local_out, &global_out)
            );

            // 4. FFN + Residual
            let ffn_out = self.ffn.forward(&combined);
            let output = self.norm.forward(&add(features, &ffn_out));

            outputs.push(output);
        }

        outputs
    }
}
```

---

## 2. Hyperbolic Graph Neural Networks

### 2.1 Motivation

**Hierarchical Graphs** (like HNSW) are better represented in hyperbolic space:
- Tree-like structures
- Exponential growth (volume ∝ e^r)
- Low distortion embeddings

**Euclidean Space**: Volume ∝ r³ (polynomial)
**Hyperbolic Space**: Volume ∝ e^r (exponential)

### 2.2 HGCN (Hyperbolic GCN)

**Paper**: Chami et al. (2019) - "Hyperbolic Graph Convolutional Neural Networks"

**Key Operations in Poincaré Ball**:

**1. Möbius Addition** (⊕):
```
x ⊕ y = [(1 + 2⟨x,y⟩ + ||y||²)x + (1 - ||x||²)y] / [1 + 2⟨x,y⟩ + ||x||²||y||²]
```

**2. Exponential Map** (exp_x):
```
exp_x(v) = x ⊕ [tanh(λ_x ||v|| / 2) · v / ||v||]
where λ_x = 2 / (1 - ||x||²)  # Conformal factor
```

**3. Logarithmic Map** (log_x):
```
log_x(y) = (2 / λ_x) · arctanh(||−x ⊕ y||) · (−x ⊕ y) / ||−x ⊕ y||
```

**Implementation**:
```rust
pub struct HyperbolicGCN {
    curvature: f32,  // Negative curvature (e.g., -1.0)
    layers: Vec<HyperbolicLayer>,
}

pub struct HyperbolicLayer {
    weight: Array2<f32>,
    bias: Vec<f32>,
    curvature: f32,
}

impl HyperbolicLayer {
    // Hyperbolic linear transformation
    fn linear(&self, x: &[f32]) -> Vec<f32> {
        // 1. Map to tangent space at origin
        let x_tangent = self.log_map_origin(x);

        // 2. Apply Euclidean linear transformation
        let y_tangent = self.weight.dot(&Array1::from_vec(x_tangent)).to_vec();

        // 3. Map back to hyperbolic space
        self.exp_map_origin(&y_tangent)
    }

    // Hyperbolic aggregation
    fn aggregate(&self, neighbors: &[Vec<f32>]) -> Vec<f32> {
        if neighbors.is_empty() {
            return vec![0.0; neighbors[0].len()];
        }

        // Use Einstein midpoint (hyperbolic mean)
        self.einstein_midpoint(neighbors)
    }

    // Exponential map from origin
    fn exp_map_origin(&self, v: &[f32]) -> Vec<f32> {
        let norm = l2_norm(v);
        let c = self.curvature.abs();

        if norm < 1e-10 {
            return v.to_vec();
        }

        let coef = (c.sqrt() * norm).tanh() / (c.sqrt() * norm);
        v.iter().map(|&x| coef * x).collect()
    }

    // Logarithmic map to origin
    fn log_map_origin(&self, x: &[f32]) -> Vec<f32> {
        let norm = l2_norm(x);
        let c = self.curvature.abs();

        if norm < 1e-10 {
            return x.to_vec();
        }

        let coef = (c.sqrt() * norm).atanh() / (c.sqrt() * norm);
        x.iter().map(|&xi| coef * xi).collect()
    }

    // Möbius addition
    fn mobius_add(&self, x: &[f32], y: &[f32]) -> Vec<f32> {
        let c = self.curvature.abs();
        let x_norm_sq = l2_norm_squared(x);
        let y_norm_sq = l2_norm_squared(y);
        let xy_dot = dot_product(x, y);

        let numerator_x_coef = 1.0 + 2.0 * c * xy_dot + c * y_norm_sq;
        let numerator_y_coef = 1.0 - c * x_norm_sq;
        let denominator = 1.0 + 2.0 * c * xy_dot + c * c * x_norm_sq * y_norm_sq;

        x.iter().zip(y.iter())
            .map(|(&xi, &yi)| {
                (numerator_x_coef * xi + numerator_y_coef * yi) / denominator
            })
            .collect()
    }

    // Einstein midpoint (hyperbolic mean)
    fn einstein_midpoint(&self, points: &[Vec<f32>]) -> Vec<f32> {
        if points.is_empty() {
            return vec![];
        }

        let dim = points[0].len();
        let mut mean = vec![0.0; dim];

        for point in points {
            mean = self.mobius_add(&mean, point);
        }

        // Scale by 1/n in tangent space
        let mean_tangent = self.log_map_origin(&mean);
        let scaled_tangent: Vec<f32> = mean_tangent.iter()
            .map(|&x| x / points.len() as f32)
            .collect();

        self.exp_map_origin(&scaled_tangent)
    }

    fn forward(
        &self,
        node_embedding: &[f32],
        neighbor_embeddings: &[Vec<f32>],
    ) -> Vec<f32> {
        // 1. Aggregate neighbors in hyperbolic space
        let aggregated = self.aggregate(neighbor_embeddings);

        // 2. Combine with self (Möbius addition)
        let combined = self.mobius_add(node_embedding, &aggregated);

        // 3. Hyperbolic linear transformation
        let transformed = self.linear(&combined);

        // 4. Hyperbolic activation (e.g., identity, or hyperbolic ReLU)
        transformed
    }
}
```

**Benefits for HNSW**:
- **Natural Hierarchies**: Higher HNSW layers = closer to origin (root)
- **Exponential Capacity**: Fit large trees with low distortion
- **Distance Preservation**: Hyperbolic distance ≈ tree distance

**Challenges**:
- **Numerical Instability**: Near boundary (||x|| → 1)
- **Complex Gradients**: Riemannian optimization required
- **Full Pipeline**: All operations must be hyperbolic-aware

### 2.3 Mixed-Curvature Product Manifolds

**Idea**: Different graph components have different geometries

```
Embedding space: R^d₁ × H^d₂ × S^d₃

where:
  R^d₁: Euclidean (local, grid-like structures)
  H^d₂: Hyperbolic (hierarchies)
  S^d₃: Spherical (cyclic, clustered)
```

**Implementation**:
```rust
pub enum ManifoldComponent {
    Euclidean(Vec<f32>),
    Hyperbolic(Vec<f32>),
    Spherical(Vec<f32>),
}

pub struct ProductManifoldEmbedding {
    components: Vec<ManifoldComponent>,
}

impl ProductManifoldEmbedding {
    fn distance(&self, other: &Self) -> f32 {
        self.components.iter()
            .zip(other.components.iter())
            .map(|(c1, c2)| match (c1, c2) {
                (ManifoldComponent::Euclidean(x), ManifoldComponent::Euclidean(y)) =>
                    l2_distance(x, y),
                (ManifoldComponent::Hyperbolic(x), ManifoldComponent::Hyperbolic(y)) =>
                    hyperbolic_distance(x, y, -1.0),
                (ManifoldComponent::Spherical(x), ManifoldComponent::Spherical(y)) =>
                    spherical_distance(x, y),
                _ => panic!("Mismatched manifold types"),
            })
            .sum::<f32>()
    }

    // Aggregate in product space
    fn aggregate(&self, embeddings: &[ProductManifoldEmbedding]) -> ProductManifoldEmbedding {
        let mut aggregated_components = Vec::new();

        for (i, component) in self.components.iter().enumerate() {
            let component_values: Vec<_> = embeddings.iter()
                .map(|emb| &emb.components[i])
                .collect();

            let aggregated = match component {
                ManifoldComponent::Euclidean(_) =>
                    ManifoldComponent::Euclidean(euclidean_mean(&component_values)),
                ManifoldComponent::Hyperbolic(_) =>
                    ManifoldComponent::Hyperbolic(hyperbolic_mean(&component_values)),
                ManifoldComponent::Spherical(_) =>
                    ManifoldComponent::Spherical(spherical_mean(&component_values)),
            };

            aggregated_components.push(aggregated);
        }

        ProductManifoldEmbedding {
            components: aggregated_components,
        }
    }
}
```

---

## 3. Neural ODEs for Graphs

### 3.1 Graph Neural ODE (Continuous Depth)

**Motivation**: GNN layers are discrete steps of a continuous diffusion process

**Standard GNN**:
```
h^{(l+1)} = h^{(l)} + GNN(h^{(l)}, G)
```

**Neural ODE**:
```
dh/dt = f(h(t), G, θ)
h(T) = h(0) + ∫₀^T f(h(t), G, θ) dt
```

**Benefits**:
- **Adaptive Depth**: Network learns optimal "time" T
- **Memory Efficient**: Backprop via adjoint method
- **Smooth Representations**: Continuous trajectory in latent space

**Implementation**:
```rust
pub struct GraphNeuralODE {
    dynamics: RuvectorLayer,  // f(h, G, θ)
    ode_solver: ODESolver,
}

impl GraphNeuralODE {
    fn forward(
        &self,
        initial_embeddings: &[Vec<f32>],
        graph_structure: &GraphStructure,
        time_horizon: f32,
    ) -> Vec<Vec<f32>> {
        // Solve ODE: h(T) = h(0) + ∫₀^T f(h(t), G) dt
        self.ode_solver.solve(
            initial_embeddings,
            |h, t| self.dynamics_function(h, graph_structure),
            0.0,
            time_horizon,
        )
    }

    fn dynamics_function(
        &self,
        h: &[Vec<f32>],
        graph: &GraphStructure,
    ) -> Vec<Vec<f32>> {
        // dh/dt = GNN(h, G)
        h.iter()
            .enumerate()
            .map(|(i, embedding)| {
                let neighbors: Vec<_> = graph.neighbors(i)
                    .iter()
                    .map(|&j| h[j].clone())
                    .collect();

                self.dynamics.forward(
                    embedding,
                    &neighbors,
                    &vec![1.0; neighbors.len()],
                )
            })
            .collect()
    }
}

// ODE Solver (e.g., Runge-Kutta 4th order)
pub struct ODESolver;

impl ODESolver {
    fn solve<F>(
        &self,
        y0: &[Vec<f32>],
        f: F,
        t0: f32,
        tf: f32,
    ) -> Vec<Vec<f32>>
    where
        F: Fn(&[Vec<f32>], f32) -> Vec<Vec<f32>>,
    {
        let num_steps = 10;
        let dt = (tf - t0) / num_steps as f32;
        let mut y = y0.to_vec();

        for step in 0..num_steps {
            let t = t0 + step as f32 * dt;

            // RK4: k1 = f(t, y)
            let k1 = f(&y, t);

            // k2 = f(t + dt/2, y + k1*dt/2)
            let y_k1 = add_scaled(&y, &k1, dt / 2.0);
            let k2 = f(&y_k1, t + dt / 2.0);

            // k3 = f(t + dt/2, y + k2*dt/2)
            let y_k2 = add_scaled(&y, &k2, dt / 2.0);
            let k3 = f(&y_k2, t + dt / 2.0);

            // k4 = f(t + dt, y + k3*dt)
            let y_k3 = add_scaled(&y, &k3, dt);
            let k4 = f(&y_k3, t + dt);

            // y_{n+1} = y_n + (dt/6) * (k1 + 2k2 + 2k3 + k4)
            y = add_rk4_increment(&y, &k1, &k2, &k3, &k4, dt);
        }

        y
    }
}
```

**Adjoint Method for Memory-Efficient Backprop**:
```rust
// Instead of storing all intermediate states, solve backwards ODE
fn backward_ode(
    &self,
    final_state: &[Vec<f32>],
    adjoint_final: &[Vec<f32>],
    time_horizon: f32,
) -> (Vec<Vec<f32>>, Vec<f32>) {  // (gradients, parameter gradients)
    // Solve backward: da/dt = -∂f/∂h · a
    // where a is adjoint variable
    self.ode_solver.solve_backward(adjoint_final, time_horizon, 0.0)
}
```

---

## 4. Equivariant Graph Networks

### 4.1 E(n)-Equivariant GNNs

**Motivation**: Geometric graphs (molecules, point clouds) require invariance to rotations/translations

**Equivariance Property**:
```
f(T(x)) = T(f(x))

where T is a transformation (e.g., rotation)
```

**EGNN (E(n) Equivariant GNN)**:

**Paper**: Satorras et al. (2021)

```
Node features: h_v ∈ R^d (invariant)
Node positions: x_v ∈ R^3 (equivariant)

Message: m_ij = φ_e(h_i, h_j, ||x_i - x_j||², a_ij)
Aggregation: m_i = Σ_j m_ij
Update features: h'_i = φ_h(h_i, m_i)
Update positions: x'_i = x_i + Σ_j (x_i - x_j) φ_x(m_ij)
```

**Key**: Distances and relative positions are used (rotationally invariant)

**Implementation**:
```rust
pub struct EquivariantGNN {
    message_mlp: MLP,
    node_mlp: MLP,
    coord_mlp: MLP,
}

impl EquivariantGNN {
    fn forward(
        &self,
        node_features: &[Vec<f32>],
        node_positions: &[Vec<f32>],  // 3D coordinates
        edges: &[(usize, usize)],
    ) -> (Vec<Vec<f32>>, Vec<Vec<f32>>) {  // (updated features, updated positions)
        let n = node_features.len();
        let mut new_features = node_features.to_vec();
        let mut new_positions = node_positions.to_vec();

        for &(i, j) in edges {
            // 1. Compute edge features (rotationally invariant)
            let rel_pos = subtract(&node_positions[i], &node_positions[j]);
            let dist_sq = l2_norm_squared(&rel_pos);

            let edge_input = concatenate3(
                &node_features[i],
                &node_features[j],
                &[dist_sq],
            );

            // 2. Message
            let message = self.message_mlp.forward(&edge_input);

            // 3. Update features (invariant)
            new_features[i] = self.node_mlp.forward(
                &concatenate(&node_features[i], &message)
            );

            // 4. Update positions (equivariant)
            let coord_weight = self.coord_mlp.forward(&message)[0];
            for k in 0..3 {
                new_positions[i][k] += coord_weight * rel_pos[k];
            }
        }

        (new_features, new_positions)
    }
}
```

**Application to HNSW**:
- If embeddings have geometric interpretation
- Preserves graph structure under transformations
- Useful for 3D data (e.g., protein structures)

---

## 5. Generative Models for Graphs

### 5.1 Graph Variational Autoencoders (GVAE)

**Goal**: Learn latent distribution of graphs, enable generation

```
Encoder: G → q(z | G)
Decoder: z → p(G | z)

Loss: ELBO = E[log p(G | z)] - β KL(q(z | G) || p(z))
```

**Implementation**:
```rust
pub struct GraphVAE {
    encoder: RuvectorLayer,
    mu_layer: Linear,
    logvar_layer: Linear,
    decoder: GraphDecoder,
}

impl GraphVAE {
    fn encode(&self, graph: &Graph) -> (Vec<f32>, Vec<f32>) {
        // Encode each node
        let node_embeddings: Vec<_> = (0..graph.num_nodes())
            .map(|v| {
                let neighbors = graph.neighbor_embeddings(v);
                self.encoder.forward(&graph.features(v), &neighbors, &[])
            })
            .collect();

        // Pool to graph-level
        let graph_embedding = mean_pool(&node_embeddings);

        // Reparameterization parameters
        let mu = self.mu_layer.forward(&graph_embedding);
        let logvar = self.logvar_layer.forward(&graph_embedding);

        (mu, logvar)
    }

    fn reparameterize(&self, mu: &[f32], logvar: &[f32]) -> Vec<f32> {
        let std: Vec<f32> = logvar.iter().map(|&lv| (lv / 2.0).exp()).collect();
        let eps: Vec<f32> = (0..mu.len())
            .map(|_| rand::thread_rng().sample(StandardNormal))
            .collect();

        mu.iter().zip(std.iter()).zip(eps.iter())
            .map(|((&m, &s), &e)| m + s * e)
            .collect()
    }

    fn decode(&self, z: &[f32], num_nodes: usize) -> AdjacencyMatrix {
        // Generate node embeddings from latent z
        let node_embeddings = self.decoder.generate_node_embeddings(z, num_nodes);

        // Generate edges via pairwise scoring
        let mut adj = AdjacencyMatrix::new(num_nodes);
        for i in 0..num_nodes {
            for j in i+1..num_nodes {
                let score = dot_product(&node_embeddings[i], &node_embeddings[j]);
                let prob = sigmoid(score);

                if rand::random::<f32>() < prob {
                    adj.add_edge(i, j);
                }
            }
        }

        adj
    }

    fn loss(
        &self,
        graph: &Graph,
        beta: f32,  // KL weight
    ) -> f32 {
        // Encode
        let (mu, logvar) = self.encode(graph);

        // Sample
        let z = self.reparameterize(&mu, &logvar);

        // Decode
        let reconstructed_adj = self.decode(&z, graph.num_nodes());

        // Reconstruction loss
        let recon_loss = bce_loss(&reconstructed_adj, graph.adjacency());

        // KL divergence
        let kl_loss: f32 = mu.iter().zip(logvar.iter())
            .map(|(&m, &lv)| -0.5 * (1.0 + lv - m*m - lv.exp()))
            .sum();

        recon_loss + beta * kl_loss
    }
}
```

### 5.2 Diffusion Models for Graphs

**Idea**: Gradually denoise from Gaussian noise to graph structure

```
Forward process: G → G_1 → ... → G_T (pure noise)
Reverse process: G_T → ... → G_1 → G (denoised graph)
```

**Graph Denoising Diffusion**:
```rust
pub struct GraphDiffusionModel {
    denoiser: RuvectorLayer,
    num_steps: usize,
    beta_schedule: Vec<f32>,  // Noise schedule
}

impl GraphDiffusionModel {
    fn forward_diffusion(
        &self,
        graph: &Graph,
        t: usize,
    ) -> NoisyGraph {
        // Add noise to graph structure
        let beta_t = self.beta_schedule[t];
        let alpha_t = 1.0 - beta_t;

        // Perturb adjacency matrix
        let noisy_adj = graph.adjacency()
            .mapv(|a| a * alpha_t.sqrt() + rand::random::<f32>() * beta_t.sqrt());

        NoisyGraph::new(noisy_adj, t)
    }

    fn reverse_diffusion_step(
        &self,
        noisy_graph: &NoisyGraph,
    ) -> NoisyGraph {
        // Predict noise using GNN
        let predicted_noise = self.denoiser.forward_graph(noisy_graph);

        // Denoise
        let t = noisy_graph.timestep;
        let beta_t = self.beta_schedule[t];
        let alpha_t = 1.0 - beta_t;

        let denoised_adj = (noisy_graph.adjacency - predicted_noise * beta_t.sqrt())
            / alpha_t.sqrt();

        NoisyGraph::new(denoised_adj, t - 1)
    }

    fn generate(&self, num_nodes: usize) -> Graph {
        // Start from pure noise
        let mut noisy_graph = NoisyGraph::random(num_nodes, self.num_steps);

        // Iteratively denoise
        for t in (0..self.num_steps).rev() {
            noisy_graph = self.reverse_diffusion_step(&noisy_graph);
        }

        noisy_graph.to_graph()
    }
}
```

---

## 6. Architecture Comparison and Recommendations

### 6.1 Comparison Matrix

| Architecture | Receptive Field | Complexity | Geometry | Use Case |
|--------------|----------------|------------|----------|----------|
| **RuVector (Current)** | K-hop | O(d·h²) | Euclidean | General HNSW |
| **Graphormer** | Global | O(n²·h) | Euclidean | Small-medium graphs |
| **GPS** | Global + Local | O(n²·h + d·h²) | Euclidean | Best of both |
| **HGCN** | K-hop | O(d·h²) | Hyperbolic | Hierarchical HNSW |
| **Mixed-Curvature** | K-hop | O(d·h²) | Mixed | Heterogeneous |
| **Neural ODE** | Continuous | O(T·d·h²) | Euclidean | Smooth dynamics |
| **EGNN** | 1-hop | O(d·h²) | Geometric | 3D geometric data |
| **GVAE** | K-hop | O(d·h²) | Euclidean | Generative tasks |
| **Diffusion** | Iterative | O(T·d·h²) | Euclidean | High-quality generation |

### 6.2 Recommendations for RuVector

**Immediate (1-2 months)**:
1. **GPS Layers**: Add global attention to current message passing
2. **Hyperbolic Embeddings**: For HNSW higher layers (hierarchical structure)

**Short-Term (3-6 months)**:
3. **Graph Transformers**: Full Graphormer implementation for comparison
4. **Neural ODE**: Continuous-depth variant for adaptive receptive field

**Long-Term (6-12 months)**:
5. **Mixed-Curvature**: Product manifolds for heterogeneous graph patterns
6. **Generative Models**: GVAE for data augmentation, anomaly detection

---

## 7. Implementation Roadmap

### Phase 1: GPS Integration (Month 1-2)
```rust
// Extend RuvectorLayer with global attention
pub struct GPSRuvectorLayer {
    local: RuvectorLayer,
    global: MultiHeadAttention,
    fusion: Linear,
}
```

### Phase 2: Hyperbolic Variant (Month 3-4)
```rust
// Hyperbolic version for upper HNSW layers
pub enum RuvectorLayerVariant {
    Euclidean(RuvectorLayer),
    Hyperbolic(HyperbolicLayer),
}
```

### Phase 3: Neural ODE (Month 5-6)
```rust
// Continuous-depth GNN
pub struct ContinuousRuvector {
    dynamics: RuvectorLayer,
    ode_solver: ODESolver,
    learnable_time: f32,  // Adaptive depth
}
```

---

## References

### Papers

**Graph Transformers**:
1. Ying et al. (2021) - "Do Transformers Really Perform Bad for Graph Representation?" (Graphormer)
2. Rampášek et al. (2022) - "Recipe for a General, Powerful, Scalable Graph Transformer" (GPS)
3. Kreuzer et al. (2021) - "Rethinking Graph Transformers with Spectral Attention"

**Hyperbolic GNNs**:
4. Chami et al. (2019) - "Hyperbolic Graph Convolutional Neural Networks" (HGCN)
5. Liu et al. (2019) - "Hyperbolic Graph Attention Network"
6. Gu et al. (2018) - "Learning Mixed-Curvature Representations in Product Spaces"

**Neural ODEs**:
7. Chen et al. (2018) - "Neural Ordinary Differential Equations"
8. Poli et al. (2019) - "Graph Neural Ordinary Differential Equations"

**Equivariant Networks**:
9. Satorras et al. (2021) - "E(n) Equivariant Graph Neural Networks" (EGNN)
10. Thomas et al. (2018) - "Tensor Field Networks"

**Generative Models**:
11. Kipf & Welling (2016) - "Variational Graph Auto-Encoders" (GVAE)
12. Vignac et al. (2022) - "DiGress: Discrete Denoising Diffusion for Graph Generation"

---

**Document Version**: 1.0
**Last Updated**: 2025-11-30
**Author**: RuVector Research Team
