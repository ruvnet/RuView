# Feature 25: Self-Organizing Graph Transformers

## Overview

### Problem Statement

Current graph transformers operate on fixed, manually designed topologies. The graph structure is either given as input (e.g., molecule graphs, social networks) or constructed once via nearest-neighbor heuristics (e.g., HNSW). In either case, the topology is static during inference and training: it does not grow, differentiate, or reorganize in response to the data distribution. This rigidity creates three fundamental bottlenecks:

1. **Topology-data mismatch**: A graph constructed for one data distribution becomes suboptimal as the distribution shifts.
2. **No specialization**: Every node and edge in the graph plays the same generic role -- there is no mechanism for nodes to develop distinct functional identities.
3. **No self-repair**: When parts of the graph become corrupted or irrelevant, there is no process for replacing or regenerating damaged regions.

Biology solved these problems billions of years ago. Morphogenesis builds complex structures from simple rules. Embryonic development differentiates a single cell into hundreds of specialized types. Autopoiesis maintains living systems by continuously rebuilding their own components. These principles have been largely ignored in graph neural network design.

### Proposed Solution

Self-Organizing Graph Transformers (SOGTs) are graph attention networks that grow, differentiate, and maintain their own topology through biologically-inspired developmental programs. The approach has three pillars:

1. **Morphogenetic Graph Networks**: Turing pattern formation on graphs drives reaction-diffusion attention, creating spatially structured activation patterns that guide message passing and edge formation.
2. **Developmental Graph Programs**: Graph grammars encode growth rules as L-system productions. Generic seed nodes differentiate into specialized types (hub nodes, boundary nodes, relay nodes) through a developmental program conditioned on local graph statistics.
3. **Autopoietic Graph Transformers**: The network continuously rebuilds its own topology -- pruning dead edges, spawning new nodes, and adjusting attention weights -- to maintain a target coherence level, analogous to homeostasis in living systems.

### Expected Benefits

- **Adaptive Topology**: 30-50% improvement in retrieval quality on distribution-shifting workloads
- **Self-Specialization**: Nodes develop distinct roles (hub, boundary, relay) reducing routing overhead by 40-60%
- **Self-Repair**: Automatic recovery from node/edge corruption with <5% transient degradation
- **Architecture Search**: Morphogenetic NAS discovers attention patterns 10x faster than random search
- **Emergent Computation**: Local attention rules give rise to global computational patterns (sorting, clustering, routing)

### Novelty Claim

**Unique Contribution**: First graph transformer architecture that grows its own topology through morphogenetic, developmental, and autopoietic processes. Unlike neural architecture search (which optimizes a fixed search space), SOGTs develop continuously through biologically-grounded growth rules that operate at runtime.

**Differentiators**:
1. Reaction-diffusion attention creates Turing patterns on graphs for structured activation
2. L-system graph grammars encode developmental programs for node specialization
3. Autopoietic maintenance loop continuously rebuilds topology to maintain coherence
4. Cellular automata attention rules produce emergent global computation from local rules
5. Morphogenetic NAS discovers novel attention architectures through growth processes

---

## Biological Foundations

### Morphogenesis and Turing Patterns

Alan Turing's 1952 paper "The Chemical Basis of Morphogenesis" demonstrated that two diffusing chemicals (an activator and an inhibitor) with different diffusion rates can spontaneously form stable spatial patterns: spots, stripes, and spirals. These reaction-diffusion systems explain leopard spots, zebrafish stripes, and fingerprint ridges.

On a graph, the Turing instability generalizes naturally. Each node holds concentrations of an activator `a` and inhibitor `h`. The dynamics follow the graph Laplacian:

```
da/dt = f(a, h) + D_a * L * a
dh/dt = g(a, h) + D_h * L * h
```

where `L` is the graph Laplacian, `D_h >> D_a` (inhibitor diffuses faster), and `f`, `g` encode local reaction kinetics. The key insight is that **Turing patterns on graphs create natural attention masks**: regions of high activator concentration attend to each other, while inhibitor barriers create boundaries between attention clusters.

### Embryonic Development and Differentiation

A single fertilized cell becomes a human body with 200+ cell types through a developmental program. Key principles:

- **Positional information**: Cells read chemical gradients to determine their position and fate.
- **Inductive signaling**: Cells signal neighbors to change type.
- **Competence windows**: Cells can only respond to certain signals during specific developmental stages.
- **Canalization**: Development is robust to perturbations -- the same endpoint is reached from varied starting conditions.

For graph transformers, these principles translate to: nodes read local graph statistics (degree, centrality, neighborhood composition) to determine their functional role; they signal neighbors through message passing to coordinate specialization; and developmental stages gate which transformations are available at each growth step.

### Autopoiesis and Self-Maintenance

Autopoiesis (Maturana and Varela, 1972) describes systems that continuously produce and replace their own components. A living cell is autopoietic: it synthesizes the membrane that bounds it, the enzymes that catalyze reactions, and the DNA that encodes those enzymes. The system maintains itself through circular causality.

For graph transformers, autopoiesis means: the attention mechanism produces the topology that shapes the attention mechanism. Dead edges are pruned. Overloaded nodes are split. Missing connections are grown. The graph maintains a target coherence level (measurable via `ruvector-coherence`) through continuous self-modification.

---

## Technical Design

### Architecture Diagram

```
                      Data Distribution
                             |
                    +--------v--------+
                    |   Seed Graph    |
                    |  (initial K     |
                    |   nodes)        |
                    +--------+--------+
                             |
              +--------------+--------------+
              |              |              |
     +--------v-------+ +---v----+ +-------v--------+
     | Morphogenetic  | | Devel- | | Autopoietic    |
     | Field Engine   | | opment | | Maintenance    |
     |                | | Program| | Loop           |
     | Turing pattern | | L-sys  | | Coherence-     |
     | on graph       | | grammar| | gated rebuild  |
     +--------+-------+ +---+----+ +-------+--------+
              |              |              |
              +------+-------+------+-------+
                     |              |
              +------v------+ +----v-------+
              | Topology    | | Node Type  |
              | Growth      | | Specialize |
              | (new edges/ | | (hub/relay/|
              |  nodes)     | |  boundary) |
              +------+------+ +----+-------+
                     |              |
                     +------+-------+
                            |
                   +--------v--------+
                   | Self-Organizing |
                   | Graph Attention |
                   | Layer           |
                   +--------+--------+
                            |
                   +--------v--------+
                   | Query / Embed   |
                   | / Route         |
                   +-----------------+


Morphogenetic Field Detail:

  Node Activator (a)     Node Inhibitor (h)
  +---+---+---+---+     +---+---+---+---+
  |0.9|0.1|0.8|0.2|     |0.1|0.8|0.2|0.9|
  +---+---+---+---+     +---+---+---+---+
  |0.2|0.7|0.1|0.9|     |0.7|0.2|0.8|0.1|
  +---+---+---+---+     +---+---+---+---+

  Attention Mask = sigma(a - threshold)
  High-a nodes form attention clusters
  High-h boundaries separate clusters
```

### Core Data Structures

```rust
/// Configuration for Self-Organizing Graph Transformer
#[derive(Debug, Clone)]
pub struct SelfOrganizingConfig {
    /// Initial seed graph size
    pub seed_nodes: usize,

    /// Maximum graph size (growth limit)
    pub max_nodes: usize,

    /// Embedding dimension
    pub embed_dim: usize,

    /// Morphogenetic field parameters
    pub morpho: MorphogeneticConfig,

    /// Developmental program parameters
    pub development: DevelopmentalConfig,

    /// Autopoietic maintenance parameters
    pub autopoiesis: AutopoieticConfig,

    /// Growth phase schedule
    pub phases: Vec<GrowthPhase>,
}

/// Morphogenetic field configuration (Turing patterns on graphs)
#[derive(Debug, Clone)]
pub struct MorphogeneticConfig {
    /// Activator diffusion rate
    pub d_activator: f32,

    /// Inhibitor diffusion rate (must be > d_activator)
    pub d_inhibitor: f32,

    /// Reaction kinetics: activator self-enhancement rate
    pub rho_a: f32,

    /// Reaction kinetics: inhibitor production rate
    pub rho_h: f32,

    /// Activator decay rate
    pub mu_a: f32,

    /// Inhibitor decay rate
    pub mu_h: f32,

    /// Number of reaction-diffusion steps per forward pass
    pub rd_steps: usize,

    /// Threshold for activator-based attention gating
    pub attention_threshold: f32,
}

impl Default for MorphogeneticConfig {
    fn default() -> Self {
        Self {
            d_activator: 0.01,
            d_inhibitor: 0.1, // 10x faster diffusion
            rho_a: 0.08,
            rho_h: 0.12,
            mu_a: 0.03,
            mu_h: 0.06,
            rd_steps: 10,
            attention_threshold: 0.5,
        }
    }
}

/// Node functional types arising from developmental specialization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeType {
    /// Undifferentiated seed node
    Stem,
    /// High-degree hub node (routes between clusters)
    Hub,
    /// Cluster boundary node (separates attention groups)
    Boundary,
    /// Internal relay node (local message passing)
    Relay,
    /// Sensory node (interfaces with external data)
    Sensory,
    /// Memory node (long-term information storage)
    Memory,
}

/// Developmental program configuration
#[derive(Debug, Clone)]
pub struct DevelopmentalConfig {
    /// L-system axiom (initial production)
    pub axiom: Vec<NodeType>,

    /// Production rules: (predecessor, condition, successor pattern)
    pub rules: Vec<ProductionRule>,

    /// Maximum developmental steps
    pub max_steps: usize,

    /// Competence window: (min_step, max_step) per rule
    pub competence_windows: Vec<(usize, usize)>,
}

/// A production rule in the developmental graph grammar
#[derive(Debug, Clone)]
pub struct ProductionRule {
    /// Node type that this rule applies to
    pub predecessor: NodeType,

    /// Condition: local graph statistic threshold
    pub condition: DevelopmentalCondition,

    /// Successor: what the node becomes + new nodes spawned
    pub successor: Vec<NodeType>,

    /// Edge pattern for newly created nodes
    pub edge_pattern: EdgePattern,
}

/// Conditions for developmental rule activation
#[derive(Debug, Clone)]
pub enum DevelopmentalCondition {
    /// Node degree exceeds threshold
    DegreeAbove(usize),
    /// Node betweenness centrality exceeds threshold
    CentralityAbove(f32),
    /// Activator concentration exceeds threshold
    ActivatorAbove(f32),
    /// Inhibitor concentration exceeds threshold
    InhibitorAbove(f32),
    /// Neighbor composition: fraction of type T exceeds threshold
    NeighborFraction(NodeType, f32),
    /// Always applies
    Always,
}

/// Edge creation patterns for developmental rules
#[derive(Debug, Clone)]
pub enum EdgePattern {
    /// Connect to parent only
    ParentOnly,
    /// Connect to parent and all parent neighbors
    InheritNeighborhood,
    /// Connect to k nearest nodes by embedding distance
    KNearest(usize),
    /// Connect to nodes with matching activator pattern
    MorphogeneticAffinity,
}

/// Autopoietic maintenance configuration
#[derive(Debug, Clone)]
pub struct AutopoieticConfig {
    /// Target coherence level (from ruvector-coherence)
    pub target_coherence: f32,

    /// Coherence tolerance band (maintain within +/- tolerance)
    pub coherence_tolerance: f32,

    /// Edge pruning threshold: remove edges with attention < threshold
    pub prune_threshold: f32,

    /// Node splitting threshold: split nodes with degree > threshold
    pub split_degree_threshold: usize,

    /// Edge growth rate: max new edges per maintenance cycle
    pub max_new_edges_per_cycle: usize,

    /// Maintenance cycle interval (every N forward passes)
    pub cycle_interval: usize,
}

/// Growth phase in the developmental schedule
#[derive(Debug, Clone)]
pub struct GrowthPhase {
    /// Phase name
    pub name: String,

    /// Duration in forward passes
    pub duration: usize,

    /// Which subsystems are active
    pub morpho_active: bool,
    pub development_active: bool,
    pub autopoiesis_active: bool,

    /// Growth rate multiplier
    pub growth_rate: f32,
}
```

### Key Algorithms

#### 1. Morphogenetic Field Update (Reaction-Diffusion on Graph)

```rust
/// Morphogenetic field state for the graph
pub struct MorphogeneticField {
    /// Activator concentration per node
    activator: Vec<f32>,
    /// Inhibitor concentration per node
    inhibitor: Vec<f32>,
    /// Graph Laplacian (sparse)
    laplacian: Vec<(usize, usize, f32)>,
    /// Configuration
    config: MorphogeneticConfig,
}

impl MorphogeneticField {
    /// Run one step of reaction-diffusion on the graph.
    ///
    /// Uses the Gierer-Meinhardt model:
    ///   da/dt = rho_a * (a^2 / h) - mu_a * a + D_a * L * a
    ///   dh/dt = rho_h * a^2 - mu_h * h + D_h * L * h
    fn step(&mut self, dt: f32) {
        let n = self.activator.len();
        let mut da = vec![0.0f32; n];
        let mut dh = vec![0.0f32; n];

        // Reaction kinetics (Gierer-Meinhardt)
        for i in 0..n {
            let a = self.activator[i];
            let h = self.inhibitor[i].max(1e-6); // avoid division by zero
            da[i] += self.config.rho_a * (a * a / h) - self.config.mu_a * a;
            dh[i] += self.config.rho_h * a * a - self.config.mu_h * h;
        }

        // Diffusion via graph Laplacian
        for &(src, dst, weight) in &self.laplacian {
            let diff_a = self.activator[dst] - self.activator[src];
            let diff_h = self.inhibitor[dst] - self.inhibitor[src];
            da[src] += self.config.d_activator * weight * diff_a;
            dh[src] += self.config.d_inhibitor * weight * diff_h;
        }

        // Euler integration
        for i in 0..n {
            self.activator[i] = (self.activator[i] + dt * da[i]).max(0.0);
            self.inhibitor[i] = (self.inhibitor[i] + dt * dh[i]).max(0.0);
        }
    }

    /// Compute attention mask from activator field.
    /// Nodes with activator above threshold attend to each other.
    fn attention_mask(&self) -> Vec<bool> {
        self.activator.iter()
            .map(|&a| a > self.config.attention_threshold)
            .collect()
    }

    /// Compute morphogenetic affinity between two nodes.
    /// Nodes with similar activator/inhibitor ratios have high affinity.
    fn affinity(&self, i: usize, j: usize) -> f32 {
        let ratio_i = self.activator[i] / self.inhibitor[i].max(1e-6);
        let ratio_j = self.activator[j] / self.inhibitor[j].max(1e-6);
        let diff = (ratio_i - ratio_j).abs();
        (-diff * diff).exp() // Gaussian affinity
    }
}
```

#### 2. Developmental Program (L-System Graph Grammar)

```rust
/// Developmental program executor
pub struct DevelopmentalProgram {
    /// Current developmental step
    step: usize,
    /// Production rules
    rules: Vec<ProductionRule>,
    /// Competence windows per rule
    competence: Vec<(usize, usize)>,
    /// Node type assignments
    node_types: Vec<NodeType>,
    /// Graph adjacency (mutable during development)
    adjacency: Vec<Vec<usize>>,
    /// Node embeddings
    embeddings: Vec<Vec<f32>>,
}

impl DevelopmentalProgram {
    /// Execute one developmental step.
    ///
    /// For each node, check if any production rule applies:
    /// 1. The node type matches the rule predecessor
    /// 2. The condition is satisfied
    /// 3. The current step is within the competence window
    ///
    /// If so, apply the rule: change node type and/or spawn new nodes.
    fn develop_step(
        &mut self,
        field: &MorphogeneticField,
        max_nodes: usize,
    ) -> Vec<DevelopmentalEvent> {
        let mut events = Vec::new();
        let current_n = self.node_types.len();

        // Collect applicable rules (avoid borrow conflicts)
        let mut applications: Vec<(usize, usize)> = Vec::new(); // (node_idx, rule_idx)

        for node_idx in 0..current_n {
            for (rule_idx, rule) in self.rules.iter().enumerate() {
                // Check competence window
                let (min_step, max_step) = self.competence[rule_idx];
                if self.step < min_step || self.step > max_step {
                    continue;
                }

                // Check predecessor type
                if self.node_types[node_idx] != rule.predecessor {
                    continue;
                }

                // Check condition
                if self.check_condition(node_idx, &rule.condition, field) {
                    applications.push((node_idx, rule_idx));
                    break; // one rule per node per step
                }
            }
        }

        // Apply rules
        for (node_idx, rule_idx) in applications {
            if self.node_types.len() >= max_nodes {
                break;
            }

            let rule = &self.rules[rule_idx];

            // First element of successor replaces the node's type
            if let Some(&new_type) = rule.successor.first() {
                let old_type = self.node_types[node_idx];
                self.node_types[node_idx] = new_type;
                events.push(DevelopmentalEvent::Differentiate {
                    node: node_idx,
                    from: old_type,
                    to: new_type,
                });
            }

            // Remaining elements spawn new nodes
            for &spawn_type in rule.successor.iter().skip(1) {
                let new_idx = self.node_types.len();
                if new_idx >= max_nodes { break; }

                self.node_types.push(spawn_type);

                // Create embedding as perturbation of parent
                let parent_emb = self.embeddings[node_idx].clone();
                let new_emb = perturb_embedding(&parent_emb, 0.01);
                self.embeddings.push(new_emb);

                // Create edges based on pattern
                let new_edges = match &rule.edge_pattern {
                    EdgePattern::ParentOnly => vec![node_idx],
                    EdgePattern::InheritNeighborhood => {
                        let mut edges = vec![node_idx];
                        edges.extend_from_slice(&self.adjacency[node_idx]);
                        edges
                    }
                    EdgePattern::KNearest(k) => {
                        self.k_nearest(new_idx, *k)
                    }
                    EdgePattern::MorphogeneticAffinity => {
                        self.morpho_nearest(new_idx, field, 4)
                    }
                };

                self.adjacency.push(new_edges.clone());
                for &neighbor in &new_edges {
                    if neighbor < self.adjacency.len() {
                        self.adjacency[neighbor].push(new_idx);
                    }
                }

                events.push(DevelopmentalEvent::Spawn {
                    parent: node_idx,
                    child: new_idx,
                    child_type: spawn_type,
                });
            }
        }

        self.step += 1;
        events
    }

    /// Check whether a developmental condition is satisfied for a node.
    fn check_condition(
        &self,
        node_idx: usize,
        condition: &DevelopmentalCondition,
        field: &MorphogeneticField,
    ) -> bool {
        match condition {
            DevelopmentalCondition::DegreeAbove(threshold) => {
                self.adjacency[node_idx].len() > *threshold
            }
            DevelopmentalCondition::ActivatorAbove(threshold) => {
                field.activator[node_idx] > *threshold
            }
            DevelopmentalCondition::InhibitorAbove(threshold) => {
                field.inhibitor[node_idx] > *threshold
            }
            DevelopmentalCondition::NeighborFraction(target_type, threshold) => {
                let neighbors = &self.adjacency[node_idx];
                if neighbors.is_empty() { return false; }
                let count = neighbors.iter()
                    .filter(|&&n| self.node_types[n] == *target_type)
                    .count();
                (count as f32 / neighbors.len() as f32) > *threshold
            }
            DevelopmentalCondition::CentralityAbove(_threshold) => {
                // Approximated via degree centrality for efficiency
                let degree = self.adjacency[node_idx].len() as f32;
                let max_degree = self.adjacency.iter()
                    .map(|adj| adj.len())
                    .max()
                    .unwrap_or(1) as f32;
                (degree / max_degree) > 0.5
            }
            DevelopmentalCondition::Always => true,
        }
    }
}

/// Events produced by the developmental program
#[derive(Debug, Clone)]
pub enum DevelopmentalEvent {
    /// A node changed its functional type
    Differentiate { node: usize, from: NodeType, to: NodeType },
    /// A new node was spawned
    Spawn { parent: usize, child: usize, child_type: NodeType },
    /// An edge was pruned
    Prune { src: usize, dst: usize },
}

/// Perturb an embedding with small Gaussian noise
fn perturb_embedding(emb: &[f32], scale: f32) -> Vec<f32> {
    emb.iter().enumerate()
        .map(|(i, &v)| {
            // Deterministic pseudo-noise based on index
            let noise = ((i as f32 * 0.618033988) % 1.0 - 0.5) * 2.0 * scale;
            v + noise
        })
        .collect()
}
```

#### 3. Autopoietic Maintenance Loop

```rust
/// Autopoietic maintenance system
pub struct AutopoieticMaintainer {
    config: AutopoieticConfig,
    /// Forward pass counter
    pass_count: usize,
    /// Running coherence history
    coherence_history: Vec<f32>,
}

impl AutopoieticMaintainer {
    /// Execute one maintenance cycle if due.
    ///
    /// Measures current coherence (via ruvector-coherence metrics),
    /// then adjusts topology to stay within the target band.
    fn maybe_maintain(
        &mut self,
        adjacency: &mut Vec<Vec<usize>>,
        node_types: &mut Vec<NodeType>,
        attention_weights: &[Vec<(usize, f32)>],
        embeddings: &[Vec<f32>],
    ) -> Vec<MaintenanceAction> {
        self.pass_count += 1;
        if self.pass_count % self.config.cycle_interval != 0 {
            return Vec::new();
        }

        let mut actions = Vec::new();
        let coherence = self.measure_coherence(attention_weights);
        self.coherence_history.push(coherence);

        let target = self.config.target_coherence;
        let tol = self.config.coherence_tolerance;

        if coherence < target - tol {
            // Coherence too low: grow edges to increase connectivity
            let new_edges = self.grow_edges(adjacency, embeddings);
            actions.extend(new_edges);
        } else if coherence > target + tol {
            // Coherence too high: prune weak edges
            let pruned = self.prune_edges(adjacency, attention_weights);
            actions.extend(pruned);
        }

        // Always check for overloaded nodes
        let splits = self.split_overloaded(adjacency, node_types, embeddings);
        actions.extend(splits);

        actions
    }

    /// Measure coherence as mean attention weight across active edges.
    fn measure_coherence(&self, attention_weights: &[Vec<(usize, f32)>]) -> f32 {
        let mut total_weight = 0.0f32;
        let mut edge_count = 0usize;

        for node_weights in attention_weights {
            for &(_neighbor, weight) in node_weights {
                total_weight += weight;
                edge_count += 1;
            }
        }

        if edge_count == 0 { return 0.0; }
        total_weight / edge_count as f32
    }

    /// Prune edges with attention weight below threshold.
    fn prune_edges(
        &self,
        adjacency: &mut Vec<Vec<usize>>,
        attention_weights: &[Vec<(usize, f32)>],
    ) -> Vec<MaintenanceAction> {
        let mut actions = Vec::new();

        for (src, node_weights) in attention_weights.iter().enumerate() {
            let to_prune: Vec<usize> = node_weights.iter()
                .filter(|&&(_, w)| w < self.config.prune_threshold)
                .map(|&(dst, _)| dst)
                .collect();

            for dst in to_prune {
                adjacency[src].retain(|&n| n != dst);
                actions.push(MaintenanceAction::PruneEdge { src, dst });
            }
        }

        actions
    }

    /// Split nodes whose degree exceeds the threshold.
    fn split_overloaded(
        &self,
        adjacency: &mut Vec<Vec<usize>>,
        node_types: &mut Vec<NodeType>,
        embeddings: &[Vec<f32>],
    ) -> Vec<MaintenanceAction> {
        let mut actions = Vec::new();
        let n = adjacency.len();

        for i in 0..n {
            if adjacency[i].len() > self.config.split_degree_threshold {
                // Split: new node takes half the edges
                let mid = adjacency[i].len() / 2;
                let split_edges: Vec<usize> = adjacency[i].drain(mid..).collect();

                let new_idx = adjacency.len();
                adjacency.push(split_edges.clone());
                node_types.push(node_types[i]);

                // Reconnect transferred edges
                for &neighbor in &split_edges {
                    if neighbor < adjacency.len() {
                        // Replace old -> new in neighbor lists
                        if let Some(pos) = adjacency[neighbor].iter().position(|&n| n == i) {
                            adjacency[neighbor][pos] = new_idx;
                        }
                    }
                }

                // Connect the two halves
                adjacency[i].push(new_idx);
                adjacency[new_idx].push(i);

                actions.push(MaintenanceAction::SplitNode {
                    original: i,
                    new_node: new_idx,
                    edges_transferred: split_edges.len(),
                });
            }
        }

        actions
    }

    /// Grow new edges to increase coherence.
    fn grow_edges(
        &self,
        adjacency: &mut Vec<Vec<usize>>,
        embeddings: &[Vec<f32>],
    ) -> Vec<MaintenanceAction> {
        let mut actions = Vec::new();
        let mut added = 0;

        // Find pairs with high embedding similarity but no edge
        for i in 0..adjacency.len() {
            if added >= self.config.max_new_edges_per_cycle { break; }

            for j in (i + 1)..adjacency.len() {
                if added >= self.config.max_new_edges_per_cycle { break; }
                if adjacency[i].contains(&j) { continue; }

                let sim = cosine_similarity(&embeddings[i], &embeddings[j]);
                if sim > 0.8 {
                    adjacency[i].push(j);
                    adjacency[j].push(i);
                    added += 1;
                    actions.push(MaintenanceAction::GrowEdge { src: i, dst: j, similarity: sim });
                }
            }
        }

        actions
    }
}

/// Actions taken by the autopoietic maintainer
#[derive(Debug, Clone)]
pub enum MaintenanceAction {
    PruneEdge { src: usize, dst: usize },
    GrowEdge { src: usize, dst: usize, similarity: f32 },
    SplitNode { original: usize, new_node: usize, edges_transferred: usize },
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a < 1e-8 || norm_b < 1e-8 { return 0.0; }
    dot / (norm_a * norm_b)
}
```

#### 4. Cellular Automata Attention Rules

```rust
/// Cellular automaton rule for graph attention updates.
///
/// Each node updates its attention state based on the attention states
/// of its neighbors, analogous to Conway's Game of Life on a graph.
pub struct CellularAttentionRule {
    /// Birth threshold: node activates if >= birth neighbors are active
    pub birth_threshold: usize,
    /// Survival range: node stays active if neighbors in [lo, hi]
    pub survival_lo: usize,
    pub survival_hi: usize,
    /// Refractory period: steps before reactivation after deactivation
    pub refractory: usize,
}

impl CellularAttentionRule {
    /// Update attention states for all nodes.
    fn update(
        &self,
        states: &mut Vec<CellState>,
        adjacency: &[Vec<usize>],
    ) {
        let n = states.len();
        let old_states: Vec<CellState> = states.clone();

        for i in 0..n {
            let active_neighbors = adjacency[i].iter()
                .filter(|&&j| old_states[j].active)
                .count();

            match &mut states[i] {
                s if s.active => {
                    // Survival check
                    if active_neighbors < self.survival_lo
                        || active_neighbors > self.survival_hi
                    {
                        s.active = false;
                        s.refractory_remaining = self.refractory;
                    }
                }
                s if s.refractory_remaining > 0 => {
                    s.refractory_remaining -= 1;
                }
                s => {
                    // Birth check
                    if active_neighbors >= self.birth_threshold {
                        s.active = true;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CellState {
    pub active: bool,
    pub refractory_remaining: usize,
}
```

---

## RuVector Integration Points

### Affected Crates/Modules

1. **`ruvector-domain-expansion`**: The `DomainExpansionEngine` already implements cross-domain transfer with `MetaThompsonEngine`. Morphogenetic fields extend this with spatial structure over the domain graph -- each domain node carries activator/inhibitor concentrations that influence the transfer policy selection. The `PolicyKernel` population search can be guided by developmental programs that specialize kernels into domain-specific roles.

2. **`ruvector-attention`**: The existing 18+ attention mechanisms (morphological, topology, sheaf, PDE, transport, curvature, sparse, flash, hyperbolic, MoE) serve as the building blocks that the self-organizing system selects and composes. The `topology/` module's gated attention maps directly to morphogenetic field gating. The `sheaf/` module's restriction maps provide the mathematical framework for boundary-creating attention between differentiated node types.

3. **`ruvector-coherence`**: The coherence engine (`spectral.rs`, `quality.rs`, `metrics.rs`) provides the feedback signal for the autopoietic loop. The target coherence from `AutopoieticConfig` corresponds directly to the spectral coherence thresholds used in the mincut-gated-transformer. Coherence measurements drive the grow/prune/split decisions.

4. **`ruvector-mincut`**: Topology optimization via mincut provides the theoretical foundation for the pruning phase of autopoiesis. The mincut-gated-transformer's `GateController` (energy gates, early exit) directly corresponds to morphogenetic field gating -- both decide which computation paths are active based on a learned signal.

5. **`ruvector-nervous-system`**: The dendritic coincidence detection (`Dendrite`, `DendriticTree`, `PlateauPotential`) maps directly to the developmental differentiation model. Neurons differentiate based on their dendritic input patterns, just as graph nodes differentiate based on local topology. The `plasticity/eprop` module's e-prop learning rule can guide morphogenetic field parameter adaptation. The `GlobalWorkspace` and `OscillatoryRouter` provide the coordination substrate for cellular automata attention.

6. **`ruvector-gnn`**: The core GNN layer (`layer.rs`), training loop (`training.rs`), and elastic weight consolidation (`ewc.rs`) provide the foundation. EWC is essential for developmental programs: when a node differentiates, the weights associated with its old type must be protected via Fisher-information-weighted regularization, preventing catastrophic forgetting of learned representations.

### New Modules to Create

```
ruvector-gnn/src/self_organizing/
  mod.rs
  morphogenetic.rs     # Reaction-diffusion field on graph
  developmental.rs     # L-system graph grammar executor
  autopoietic.rs       # Self-maintenance loop
  cellular_automata.rs # CA-based attention rules
  growth_phase.rs      # Phase scheduling
  metrics.rs           # Growth statistics and visualization
```

---

## Future Roadmap

### 2030: Self-Growing Graph Architectures

By 2030, the developmental program becomes a learned object rather than a hand-designed grammar. The production rules themselves are parameterized by neural networks trained via reinforcement learning on downstream task performance. Key milestones:

- **Learned Growth Rules**: A meta-network predicts which production rule to apply at each developmental step, conditioned on global graph statistics and task loss gradients.
- **Topology-Aware Data Distribution Matching**: The morphogenetic field parameters are optimized so that the resulting attention cluster structure matches the data distribution's intrinsic geometry (e.g., manifold structure, cluster hierarchy).
- **Federated Self-Organization**: Multiple SOGT instances running on different data partitions exchange developmental signals (activator/inhibitor concentrations) to coordinate topology across distributed deployments.
- **Morphogenetic Architecture Search**: Instead of NAS over a fixed search space, the search space itself grows through morphogenetic processes. Novel attention mechanisms emerge as stable Turing patterns on the architecture search graph.

### 2036: Autonomous Graph Systems

By 2036, the self-organizing graph transformer becomes a fully autonomous system that evolves new attention mechanisms through its developmental program:

- **Open-Ended Evolution**: The graph system exhibits open-ended evolution -- it continuously produces novel structures that are not repetitions of previous states. New node types, edge types, and attention mechanisms emerge without human intervention.
- **Developmental Canalization**: The system develops robust developmental trajectories that reliably produce high-performing topologies despite environmental perturbation, analogous to biological canalization.
- **Morphogenetic Memory**: Growth histories are stored as compressed developmental programs (analogous to DNA) that can be replayed, mutated, and recombined for evolutionary search over architectures.
- **Autopoietic Resilience at Scale**: Production graph systems with millions of nodes self-repair within milliseconds of node failure, maintaining 99.999% coherence through continuous autopoietic maintenance without human intervention.

---

## Implementation Phases

### Phase 1: Morphogenetic Fields (3 weeks)
- Implement reaction-diffusion on graph using graph Laplacian
- Integrate Turing pattern attention masking with existing ruvector-attention
- Validate pattern formation on synthetic graphs
- Unit tests for stability and convergence

### Phase 2: Developmental Programs (4 weeks)
- Implement L-system graph grammar with production rules
- Add competence windows and node differentiation
- Integrate with morphogenetic fields for condition checking
- Test developmental trajectories on benchmark graphs

### Phase 3: Autopoietic Maintenance (3 weeks)
- Implement coherence-gated topology maintenance using ruvector-coherence
- Add edge pruning, node splitting, and edge growth
- Integrate with existing HNSW index maintenance
- Stress tests for self-repair under node deletion

### Phase 4: Integration and Evaluation (2 weeks)
- Combine all three subsystems into unified SOGT layer
- Benchmark against static graph transformers on distribution-shifting workloads
- Measure self-repair latency and coherence maintenance
- Document growth phase scheduling heuristics

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Topology Adaptation Speed | <100ms to respond to distribution shift |
| Node Specialization Accuracy | >85% correct functional type assignment |
| Self-Repair Recovery Time | <50ms to recover from 10% node deletion |
| Coherence Maintenance | Within +/-5% of target coherence |
| Retrieval Quality (shifting workload) | 30-50% improvement over static topology |
| Growth Overhead | <15% additional computation per forward pass |
| Morphogenetic Pattern Stability | Converge within 50 reaction-diffusion steps |

---

## Risks and Mitigations

1. **Risk: Uncontrolled Growth**
   - Mitigation: Hard `max_nodes` cap, growth rate limits per phase, energy-based cost for node creation

2. **Risk: Developmental Instability**
   - Mitigation: Canalization through competence windows, EWC-protected weight consolidation during differentiation

3. **Risk: Morphogenetic Pattern Collapse**
   - Mitigation: Validated Turing parameter regimes (D_h/D_a > 5), stochastic perturbation to break symmetry

4. **Risk: Autopoietic Oscillation**
   - Mitigation: Hysteresis in coherence thresholds (different thresholds for grow vs. prune), exponential moving average smoothing

5. **Risk: Performance Overhead**
   - Mitigation: Amortize maintenance over many forward passes, sparse Laplacian operations, early-exit from growth phases when targets are met
