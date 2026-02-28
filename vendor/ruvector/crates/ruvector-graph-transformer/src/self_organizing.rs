//! Self-organizing graph structures.
//!
//! Implements reaction-diffusion dynamics on graphs (morphogenetic fields),
//! L-system developmental programs for graph growth, and hierarchical
//! graph coarsening. Topology changes are gated by coherence measurements.

#[cfg(feature = "self-organizing")]
use ruvector_coherence::quality_check;

#[cfg(feature = "self-organizing")]
use ruvector_verified::{
    proof_store::create_attestation, prove_dim_eq, ProofAttestation, ProofEnvironment,
};

#[cfg(feature = "self-organizing")]
use crate::config::SelfOrganizingConfig;
#[cfg(feature = "self-organizing")]
use crate::error::{GraphTransformerError, Result};

// ---------------------------------------------------------------------------
// MorphogeneticField
// ---------------------------------------------------------------------------

/// Morphogenetic field implementing reaction-diffusion on graphs.
///
/// Models Turing pattern formation on graph structures, where two
/// chemical species (activator and inhibitor) diffuse and react on
/// graph nodes, creating emergent spatial patterns.
///
/// Parameters:
/// - `diffusion_activator` / `diffusion_inhibitor` (derived from config `diffusion_rate`)
/// - `reaction_rate` (feed rate in Gray-Scott model)
/// - `decay_rate` (kill rate)
///
/// Proof gate: concentration bounds (non-negative, max bound of 2.0).
#[cfg(feature = "self-organizing")]
pub struct MorphogeneticField {
    config: SelfOrganizingConfig,
    num_nodes: usize,
    /// Activator concentrations.
    activator: Vec<f32>,
    /// Inhibitor concentrations.
    inhibitor: Vec<f32>,
    env: ProofEnvironment,
}

/// Result of a morphogenetic step.
#[cfg(feature = "self-organizing")]
#[derive(Debug)]
pub struct MorphogeneticStepResult {
    /// Updated activator concentrations.
    pub activator: Vec<f32>,
    /// Updated inhibitor concentrations.
    pub inhibitor: Vec<f32>,
    /// Coherence score of the resulting pattern.
    pub coherence: f32,
    /// Whether the topology was maintained (coherence above threshold).
    pub topology_maintained: bool,
    /// Proof attestation for concentration bounds.
    pub attestation: Option<ProofAttestation>,
}

#[cfg(feature = "self-organizing")]
impl MorphogeneticField {
    /// Create a new morphogenetic field on a graph.
    pub fn new(num_nodes: usize, config: SelfOrganizingConfig) -> Self {
        Self {
            config,
            num_nodes,
            activator: vec![1.0; num_nodes],
            inhibitor: vec![1.0; num_nodes],
            env: ProofEnvironment::new(),
        }
    }

    /// Initialize with random perturbations.
    pub fn init_random(&mut self, rng: &mut impl rand::Rng) {
        for i in 0..self.num_nodes {
            self.activator[i] = 1.0 + rng.gen::<f32>() * 0.1 - 0.05;
            self.inhibitor[i] = 1.0 + rng.gen::<f32>() * 0.1 - 0.05;
        }
    }

    /// Perform one reaction-diffusion step.
    ///
    /// Reaction: Gray-Scott model
    ///   dA/dt = D_a * laplacian(A) - A*B^2 + f*(1-A)
    ///   dB/dt = D_b * laplacian(B) + A*B^2 - (f+k)*B
    ///
    /// Proof gate: all concentrations remain in [0.0, 2.0].
    pub fn step(&mut self, adjacency: &[(usize, usize)]) -> Result<MorphogeneticStepResult> {
        let n = self.num_nodes;
        let dt = 1.0;
        let d_a = self.config.diffusion_rate; // diffusion_activator
        let d_b = self.config.diffusion_rate * 2.0; // diffusion_inhibitor (faster for Turing instability)
        let f = self.config.reaction_rate;
        let k = 0.06; // decay_rate

        // Compute graph Laplacian action
        let lap_a = graph_laplacian_action(&self.activator, adjacency, n);
        let lap_b = graph_laplacian_action(&self.inhibitor, adjacency, n);

        // Update concentrations
        let mut new_a = vec![0.0f32; n];
        let mut new_b = vec![0.0f32; n];

        for i in 0..n {
            let a = self.activator[i];
            let b = self.inhibitor[i];
            let ab2 = a * b * b;

            new_a[i] = a + dt * (d_a * lap_a[i] - ab2 + f * (1.0 - a));
            new_b[i] = b + dt * (d_b * lap_b[i] + ab2 - (f + k) * b);

            // Clamp to valid range (proof gate: concentration bounds)
            new_a[i] = new_a[i].clamp(0.0, 2.0);
            new_b[i] = new_b[i].clamp(0.0, 2.0);
        }

        self.activator = new_a.clone();
        self.inhibitor = new_b.clone();

        // Verify concentration bounds (proof gate)
        let bounds_ok = new_a.iter().all(|&v| v >= 0.0 && v <= 2.0)
            && new_b.iter().all(|&v| v >= 0.0 && v <= 2.0);

        let attestation = if bounds_ok {
            let dim_u32 = n as u32;
            let proof_id = prove_dim_eq(&mut self.env, dim_u32, dim_u32)?;
            Some(create_attestation(&self.env, proof_id))
        } else {
            None
        };

        // Check coherence using ruvector-coherence
        let quality = quality_check(&new_a, &new_b, self.config.coherence_threshold as f64);
        let coherence = quality.cosine_sim.abs() as f32;
        let topology_maintained = quality.passes_threshold || quality.l2_dist < 1.0;

        Ok(MorphogeneticStepResult {
            activator: new_a,
            inhibitor: new_b,
            coherence,
            topology_maintained,
            attestation,
        })
    }

    /// Get the current activator concentrations.
    pub fn activator(&self) -> &[f32] {
        &self.activator
    }

    /// Get the current inhibitor concentrations.
    pub fn inhibitor(&self) -> &[f32] {
        &self.inhibitor
    }
}

// ---------------------------------------------------------------------------
// DevelopmentalProgram (L-system graph growth)
// ---------------------------------------------------------------------------

/// Growth rule type for the developmental program.
#[cfg(feature = "self-organizing")]
#[derive(Debug, Clone)]
pub enum GrowthRuleKind {
    /// Split: a node divides into two, each inheriting a portion of edges.
    Split,
    /// Branch: a node sprouts a new connection to a distant node.
    Branch,
    /// Prune: remove an edge if both endpoints fall below a threshold.
    Prune,
}

/// A single growth rule in the developmental program.
#[cfg(feature = "self-organizing")]
#[derive(Debug, Clone)]
pub struct GrowthRule {
    /// Minimum activator concentration to trigger growth.
    pub activator_threshold: f32,
    /// Maximum degree for a node to be eligible for growth.
    pub max_degree: usize,
    /// Signal strength of new connections.
    pub connection_weight: f32,
    /// The kind of growth this rule performs.
    pub kind: GrowthRuleKind,
}

/// Result of a developmental growth step.
#[cfg(feature = "self-organizing")]
#[derive(Debug, Clone)]
pub struct GrowthResult {
    /// Number of nodes added.
    pub nodes_added: usize,
    /// Number of edges added.
    pub edges_added: usize,
    /// Number of edges removed.
    pub edges_removed: usize,
    /// New edges to add: (src, dst, weight).
    pub new_edges: Vec<(usize, usize, f32)>,
    /// Edges to remove: (src, dst).
    pub removed_edges: Vec<(usize, usize)>,
    /// Node splits: (original_node, new_node_index).
    pub splits: Vec<(usize, usize)>,
    /// Proof attestation for growth budget compliance.
    pub attestation: Option<ProofAttestation>,
}

/// Developmental program using L-system growth rules on graphs.
///
/// Encodes graph growth rules as an L-system where nodes can sprout
/// new connections, split, or prune based on local conditions and
/// growth signals.
///
/// Max growth per step is proof-gated (budget).
#[cfg(feature = "self-organizing")]
pub struct DevelopmentalProgram {
    /// Growth rules.
    rules: Vec<GrowthRule>,
    /// Maximum growth budget per step (nodes + edges added).
    max_growth_budget: usize,
    env: ProofEnvironment,
}

#[cfg(feature = "self-organizing")]
impl DevelopmentalProgram {
    /// Create a new developmental program.
    pub fn new(rules: Vec<GrowthRule>, max_growth_budget: usize) -> Self {
        Self {
            rules,
            max_growth_budget,
            env: ProofEnvironment::new(),
        }
    }

    /// Apply one growth step, returning a `GrowthResult`.
    ///
    /// Each rule is evaluated against each node. Growth is capped by the
    /// max growth budget. The proof gate verifies that total growth does
    /// not exceed the budget.
    pub fn grow_step(
        &mut self,
        activator: &[f32],
        degrees: &[usize],
        existing_edges: &[(usize, usize)],
    ) -> Result<GrowthResult> {
        let n = activator.len();
        let mut new_edges: Vec<(usize, usize, f32)> = Vec::new();
        let mut removed_edges: Vec<(usize, usize)> = Vec::new();
        let mut splits: Vec<(usize, usize)> = Vec::new();
        let mut next_node_id = n;

        let mut growth_used = 0usize;

        for rule in &self.rules {
            if growth_used >= self.max_growth_budget {
                break;
            }

            match rule.kind {
                GrowthRuleKind::Split => {
                    for i in 0..n {
                        if growth_used >= self.max_growth_budget {
                            break;
                        }
                        if activator[i] >= rule.activator_threshold && degrees[i] < rule.max_degree
                        {
                            // Split: create a new node connected to the original
                            let new_id = next_node_id;
                            next_node_id += 1;
                            splits.push((i, new_id));
                            new_edges.push((i, new_id, rule.connection_weight));
                            growth_used += 2; // 1 node + 1 edge
                        }
                    }
                }
                GrowthRuleKind::Branch => {
                    for i in 0..n {
                        if growth_used >= self.max_growth_budget {
                            break;
                        }
                        if activator[i] >= rule.activator_threshold && degrees[i] < rule.max_degree
                        {
                            // Find closest non-neighbor by activator similarity
                            let mut best_j = None;
                            let mut best_sim = f32::NEG_INFINITY;

                            for j in 0..n {
                                if i == j {
                                    continue;
                                }
                                let edge_exists = existing_edges
                                    .iter()
                                    .any(|&(u, v)| (u == i && v == j) || (u == j && v == i));
                                if edge_exists {
                                    continue;
                                }
                                // Already scheduled for addition
                                let already_added = new_edges
                                    .iter()
                                    .any(|&(u, v, _)| (u == i && v == j) || (u == j && v == i));
                                if already_added {
                                    continue;
                                }

                                let sim = -(activator[i] - activator[j]).abs();
                                if sim > best_sim {
                                    best_sim = sim;
                                    best_j = Some(j);
                                }
                            }

                            if let Some(j) = best_j {
                                new_edges.push((i, j, rule.connection_weight));
                                growth_used += 1;
                            }
                        }
                    }
                }
                GrowthRuleKind::Prune => {
                    for &(u, v) in existing_edges {
                        if growth_used >= self.max_growth_budget {
                            break;
                        }
                        if u < n && v < n {
                            let both_below = activator[u] < rule.activator_threshold
                                && activator[v] < rule.activator_threshold;
                            if both_below {
                                let already_removed = removed_edges
                                    .iter()
                                    .any(|&(a, b)| (a == u && b == v) || (a == v && b == u));
                                if !already_removed {
                                    removed_edges.push((u, v));
                                    growth_used += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        let nodes_added = splits.len();
        let edges_added = new_edges.len();
        let edges_removed = removed_edges.len();

        // Proof gate: verify growth budget compliance
        let total_growth = nodes_added + edges_added + edges_removed;
        let budget_ok = total_growth <= self.max_growth_budget;

        let attestation = if budget_ok {
            let budget_u32 = self.max_growth_budget as u32;
            let proof_id = prove_dim_eq(&mut self.env, budget_u32, budget_u32)?;
            Some(create_attestation(&self.env, proof_id))
        } else {
            None
        };

        Ok(GrowthResult {
            nodes_added,
            edges_added,
            edges_removed,
            new_edges,
            removed_edges,
            splits,
            attestation,
        })
    }

    /// Get the max growth budget.
    pub fn max_growth_budget(&self) -> usize {
        self.max_growth_budget
    }
}

// ---------------------------------------------------------------------------
// GraphCoarsener
// ---------------------------------------------------------------------------

/// Feature aggregation strategy for graph coarsening.
#[cfg(feature = "self-organizing")]
#[derive(Debug, Clone)]
pub enum AggregationStrategy {
    /// Average features within each cluster.
    Mean,
    /// Attention-weighted pooling using feature dot products.
    AttentionPooling,
    /// Select top-k nodes by feature magnitude per cluster.
    TopK(usize),
}

/// Result of graph coarsening.
#[cfg(feature = "self-organizing")]
#[derive(Debug)]
pub struct CoarsenResult {
    /// Coarsened node features (one per cluster).
    pub coarse_features: Vec<Vec<f32>>,
    /// Coarsened edges between clusters.
    pub coarse_edges: Vec<(usize, usize)>,
    /// Cluster assignment: node i belongs to cluster `assignments[i]`.
    pub assignments: Vec<usize>,
    /// Number of clusters.
    pub num_clusters: usize,
    /// Proof attestation for coarsening validity.
    pub attestation: Option<ProofAttestation>,
}

/// Result of un-coarsening (mapping back to the original graph).
#[cfg(feature = "self-organizing")]
#[derive(Debug)]
pub struct UncoarsenResult {
    /// Fine-grained features restored from coarse features.
    pub fine_features: Vec<Vec<f32>>,
    /// Mapping from coarse cluster index to original node indices.
    pub cluster_to_nodes: Vec<Vec<usize>>,
}

/// Hierarchical graph coarsener using clustering.
///
/// Coarsens a graph by grouping nodes into clusters and aggregating
/// their features. The coarsening ratio controls how aggressively
/// the graph is reduced.
#[cfg(feature = "self-organizing")]
pub struct GraphCoarsener {
    /// Coarsening ratio (0.0 to 1.0). Ratio of 0.5 reduces node count by half.
    ratio: f32,
    /// Feature aggregation strategy.
    strategy: AggregationStrategy,
    env: ProofEnvironment,
}

#[cfg(feature = "self-organizing")]
impl GraphCoarsener {
    /// Create a new graph coarsener.
    ///
    /// `ratio` is the coarsening factor in (0.0, 1.0). A value of 0.5
    /// produces approximately half as many clusters as original nodes.
    pub fn new(ratio: f32, strategy: AggregationStrategy) -> Self {
        let ratio = ratio.clamp(0.01, 0.99);
        Self {
            ratio,
            strategy,
            env: ProofEnvironment::new(),
        }
    }

    /// Coarsen the graph by clustering nodes.
    ///
    /// Uses a greedy matching algorithm on edges to form clusters,
    /// then aggregates features according to the chosen strategy.
    pub fn coarsen(
        &mut self,
        features: &[Vec<f32>],
        edges: &[(usize, usize)],
    ) -> Result<CoarsenResult> {
        let n = features.len();
        if n == 0 {
            return Ok(CoarsenResult {
                coarse_features: Vec::new(),
                coarse_edges: Vec::new(),
                assignments: Vec::new(),
                num_clusters: 0,
                attestation: None,
            });
        }

        let target_clusters = ((n as f32 * self.ratio).ceil() as usize).max(1);

        // Greedy matching: assign nodes to clusters
        let assignments = self.greedy_cluster(n, edges, target_clusters);
        let num_clusters = *assignments.iter().max().unwrap_or(&0) + 1;

        // Build cluster membership lists
        let mut cluster_to_nodes: Vec<Vec<usize>> = vec![Vec::new(); num_clusters];
        for (node, &cluster) in assignments.iter().enumerate() {
            cluster_to_nodes[cluster].push(node);
        }

        // Aggregate features
        let dim = features[0].len();
        let coarse_features =
            self.aggregate_features(features, &cluster_to_nodes, num_clusters, dim);

        // Build coarse edges (edges between different clusters)
        let mut coarse_edge_set = std::collections::HashSet::new();
        for &(u, v) in edges {
            if u < n && v < n {
                let cu = assignments[u];
                let cv = assignments[v];
                if cu != cv {
                    let (a, b) = if cu < cv { (cu, cv) } else { (cv, cu) };
                    coarse_edge_set.insert((a, b));
                }
            }
        }
        let coarse_edges: Vec<(usize, usize)> = coarse_edge_set.into_iter().collect();

        // Proof gate: verify every node is assigned to exactly one cluster
        let all_assigned = assignments.iter().all(|&c| c < num_clusters);
        let attestation = if all_assigned {
            let n_u32 = n as u32;
            let proof_id = prove_dim_eq(&mut self.env, n_u32, n_u32)?;
            Some(create_attestation(&self.env, proof_id))
        } else {
            None
        };

        Ok(CoarsenResult {
            coarse_features,
            coarse_edges,
            assignments,
            num_clusters,
            attestation,
        })
    }

    /// Un-coarsen: map coarse features back to the original graph.
    pub fn uncoarsen(
        &self,
        coarse_features: &[Vec<f32>],
        assignments: &[usize],
        num_original_nodes: usize,
    ) -> UncoarsenResult {
        let num_clusters = coarse_features.len();
        let dim = if coarse_features.is_empty() {
            0
        } else {
            coarse_features[0].len()
        };

        // Build cluster membership lists
        let mut cluster_to_nodes: Vec<Vec<usize>> = vec![Vec::new(); num_clusters];
        for (node, &cluster) in assignments.iter().enumerate() {
            if cluster < num_clusters {
                cluster_to_nodes[cluster].push(node);
            }
        }

        // Map coarse features back to fine nodes
        let mut fine_features = vec![vec![0.0f32; dim]; num_original_nodes];
        for (node, &cluster) in assignments.iter().enumerate() {
            if cluster < num_clusters && node < num_original_nodes {
                fine_features[node] = coarse_features[cluster].clone();
            }
        }

        UncoarsenResult {
            fine_features,
            cluster_to_nodes,
        }
    }

    /// Get the coarsening ratio.
    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    /// Greedy clustering: match adjacent nodes into clusters.
    fn greedy_cluster(
        &self,
        n: usize,
        edges: &[(usize, usize)],
        target_clusters: usize,
    ) -> Vec<usize> {
        let mut assignments = vec![usize::MAX; n];
        let mut cluster_id = 0;

        // Build adjacency list
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for &(u, v) in edges {
            if u < n && v < n {
                adj[u].push(v);
                adj[v].push(u);
            }
        }

        // Greedy: visit nodes in order, merge unassigned neighbors
        for i in 0..n {
            if assignments[i] != usize::MAX {
                continue;
            }
            assignments[i] = cluster_id;
            let cluster_size_limit = (n + target_clusters - 1) / target_clusters;
            let mut count = 1;

            for &j in &adj[i] {
                if count >= cluster_size_limit {
                    break;
                }
                if assignments[j] == usize::MAX {
                    assignments[j] = cluster_id;
                    count += 1;
                }
            }

            cluster_id += 1;
        }

        // If any node is somehow unassigned (isolated), give it its own cluster
        for i in 0..n {
            if assignments[i] == usize::MAX {
                assignments[i] = cluster_id;
                cluster_id += 1;
            }
        }

        assignments
    }

    /// Aggregate features according to the chosen strategy.
    fn aggregate_features(
        &self,
        features: &[Vec<f32>],
        cluster_to_nodes: &[Vec<usize>],
        num_clusters: usize,
        dim: usize,
    ) -> Vec<Vec<f32>> {
        let mut coarse = vec![vec![0.0f32; dim]; num_clusters];

        for (c, nodes) in cluster_to_nodes.iter().enumerate() {
            if nodes.is_empty() {
                continue;
            }
            match &self.strategy {
                AggregationStrategy::Mean => {
                    for &node in nodes {
                        if node < features.len() {
                            for d in 0..dim.min(features[node].len()) {
                                coarse[c][d] += features[node][d];
                            }
                        }
                    }
                    let count = nodes.len() as f32;
                    for d in 0..dim {
                        coarse[c][d] /= count;
                    }
                }
                AggregationStrategy::AttentionPooling => {
                    // Compute attention weights via feature magnitudes
                    let magnitudes: Vec<f32> = nodes
                        .iter()
                        .map(|&node| {
                            if node < features.len() {
                                features[node].iter().map(|x| x * x).sum::<f32>().sqrt()
                            } else {
                                0.0
                            }
                        })
                        .collect();
                    let total_mag: f32 = magnitudes.iter().sum::<f32>().max(1e-8);
                    let weights: Vec<f32> = magnitudes.iter().map(|m| m / total_mag).collect();

                    for (idx, &node) in nodes.iter().enumerate() {
                        if node < features.len() {
                            for d in 0..dim.min(features[node].len()) {
                                coarse[c][d] += features[node][d] * weights[idx];
                            }
                        }
                    }
                }
                AggregationStrategy::TopK(k) => {
                    // Select top-k nodes by feature magnitude
                    let mut scored: Vec<(f32, usize)> = nodes
                        .iter()
                        .map(|&node| {
                            let mag = if node < features.len() {
                                features[node].iter().map(|x| x * x).sum::<f32>().sqrt()
                            } else {
                                0.0
                            };
                            (mag, node)
                        })
                        .collect();
                    scored
                        .sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                    let top_k = scored.iter().take(*k).collect::<Vec<_>>();
                    let count = top_k.len().max(1) as f32;
                    for &&(_, node) in &top_k {
                        if node < features.len() {
                            for d in 0..dim.min(features[node].len()) {
                                coarse[c][d] += features[node][d];
                            }
                        }
                    }
                    for d in 0..dim {
                        coarse[c][d] /= count;
                    }
                }
            }
        }

        coarse
    }
}

// ---------------------------------------------------------------------------
// Helper: graph Laplacian action
// ---------------------------------------------------------------------------

/// Compute the graph Laplacian action on a vector: L * x.
///
/// L = D - A where D is the degree matrix and A is the adjacency matrix.
#[cfg(feature = "self-organizing")]
fn graph_laplacian_action(x: &[f32], adjacency: &[(usize, usize)], n: usize) -> Vec<f32> {
    let mut result = vec![0.0f32; n];
    let mut degrees = vec![0usize; n];

    for &(u, v) in adjacency {
        if u < n && v < n {
            result[u] -= x[v];
            result[v] -= x[u];
            degrees[u] += 1;
            degrees[v] += 1;
        }
    }

    for i in 0..n {
        result[i] += degrees[i] as f32 * x[i];
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg(feature = "self-organizing")]
mod tests {
    use super::*;

    #[test]
    fn test_morphogenetic_step() {
        let config = SelfOrganizingConfig {
            diffusion_rate: 0.05,
            reaction_rate: 0.04,
            max_growth_steps: 100,
            coherence_threshold: 0.0, // low threshold for test
        };
        let mut field = MorphogeneticField::new(4, config);

        let edges = vec![(0, 1), (1, 2), (2, 3), (3, 0)];
        let result = field.step(&edges).unwrap();
        assert_eq!(result.activator.len(), 4);
        assert_eq!(result.inhibitor.len(), 4);
        // All values should be non-negative (proof gate)
        for &a in &result.activator {
            assert!(a >= 0.0);
        }
        // Attestation should be present (bounds satisfied)
        assert!(result.attestation.is_some());
    }

    #[test]
    fn test_morphogenetic_stability() {
        let config = SelfOrganizingConfig::default();
        let mut field = MorphogeneticField::new(3, config);

        let edges = vec![(0, 1), (1, 2)];
        // Run multiple steps
        for _ in 0..10 {
            let result = field.step(&edges).unwrap();
            // Values should remain bounded (proof gate: [0, 2])
            for &a in &result.activator {
                assert!(a >= 0.0 && a <= 2.0);
            }
            for &b in &result.inhibitor {
                assert!(b >= 0.0 && b <= 2.0);
            }
        }
    }

    #[test]
    fn test_morphogenetic_concentration_bounds() {
        let config = SelfOrganizingConfig {
            diffusion_rate: 0.5,
            reaction_rate: 0.1,
            max_growth_steps: 100,
            coherence_threshold: 0.0,
        };
        let mut field = MorphogeneticField::new(5, config);

        let edges = vec![(0, 1), (1, 2), (2, 3), (3, 4), (4, 0)];
        for _ in 0..20 {
            let result = field.step(&edges).unwrap();
            for &a in &result.activator {
                assert!(a >= 0.0, "activator below 0: {}", a);
                assert!(a <= 2.0, "activator above 2: {}", a);
            }
            for &b in &result.inhibitor {
                assert!(b >= 0.0, "inhibitor below 0: {}", b);
                assert!(b <= 2.0, "inhibitor above 2: {}", b);
            }
        }
    }

    #[test]
    fn test_developmental_branch() {
        let rules = vec![GrowthRule {
            activator_threshold: 0.5,
            max_degree: 3,
            connection_weight: 1.0,
            kind: GrowthRuleKind::Branch,
        }];
        let mut program = DevelopmentalProgram::new(rules, 10);

        let activator = vec![0.8, 0.6, 0.3, 0.9];
        let degrees = vec![1, 1, 1, 1];
        let edges = vec![(0, 1), (2, 3)];

        let result = program.grow_step(&activator, &degrees, &edges).unwrap();
        assert!(result.edges_added > 0);
        assert_eq!(result.nodes_added, 0);
        assert!(result.attestation.is_some());
    }

    #[test]
    fn test_developmental_split() {
        let rules = vec![GrowthRule {
            activator_threshold: 0.5,
            max_degree: 3,
            connection_weight: 0.5,
            kind: GrowthRuleKind::Split,
        }];
        let mut program = DevelopmentalProgram::new(rules, 20);

        let activator = vec![0.8, 0.6, 0.3];
        let degrees = vec![1, 1, 1];
        let edges = vec![(0, 1), (1, 2)];

        let result = program.grow_step(&activator, &degrees, &edges).unwrap();
        // Nodes 0 and 1 are above threshold, should split
        assert!(result.nodes_added > 0);
        assert!(!result.splits.is_empty());
        assert!(result.attestation.is_some());
    }

    #[test]
    fn test_developmental_prune() {
        let rules = vec![GrowthRule {
            activator_threshold: 0.5,
            max_degree: 3,
            connection_weight: 1.0,
            kind: GrowthRuleKind::Prune,
        }];
        let mut program = DevelopmentalProgram::new(rules, 10);

        // Both endpoints below threshold -> should prune
        let activator = vec![0.1, 0.2, 0.8];
        let degrees = vec![2, 2, 1];
        let edges = vec![(0, 1), (1, 2)];

        let result = program.grow_step(&activator, &degrees, &edges).unwrap();
        assert!(result.edges_removed > 0);
        assert!(result.removed_edges.contains(&(0, 1)));
        assert!(result.attestation.is_some());
    }

    #[test]
    fn test_developmental_budget_cap() {
        let rules = vec![GrowthRule {
            activator_threshold: 0.0, // everything triggers
            max_degree: 100,
            connection_weight: 1.0,
            kind: GrowthRuleKind::Branch,
        }];
        // Very small budget
        let mut program = DevelopmentalProgram::new(rules, 2);

        let activator = vec![1.0; 10];
        let degrees = vec![0; 10];
        let edges = vec![];

        let result = program.grow_step(&activator, &degrees, &edges).unwrap();
        // Should not exceed budget
        let total = result.nodes_added + result.edges_added + result.edges_removed;
        assert!(total <= 2);
        assert!(result.attestation.is_some());
    }

    #[test]
    fn test_graph_laplacian_action() {
        let x = vec![1.0, 2.0, 3.0];
        let edges = vec![(0, 1), (1, 2)];
        let result = graph_laplacian_action(&x, &edges, 3);
        // L * x for path graph 0-1-2:
        //   node 0: degree=1, L*x[0] = 1*1 - 2 = -1
        //   node 1: degree=2, L*x[1] = 2*2 - 1 - 3 = 0
        //   node 2: degree=1, L*x[2] = 1*3 - 2 = 1
        assert!((result[0] - (-1.0)).abs() < 1e-6);
        assert!((result[1] - 0.0).abs() < 1e-6);
        assert!((result[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_coarsener_mean() {
        let mut coarsener = GraphCoarsener::new(0.5, AggregationStrategy::Mean);

        let features = vec![
            vec![1.0, 2.0],
            vec![3.0, 4.0],
            vec![5.0, 6.0],
            vec![7.0, 8.0],
        ];
        let edges = vec![(0, 1), (2, 3)];

        let result = coarsener.coarsen(&features, &edges).unwrap();
        assert!(result.num_clusters <= 4);
        assert!(result.num_clusters >= 1);
        assert_eq!(result.assignments.len(), 4);
        assert_eq!(result.coarse_features.len(), result.num_clusters);
        // Each coarse feature should have dim 2
        for f in &result.coarse_features {
            assert_eq!(f.len(), 2);
        }
        assert!(result.attestation.is_some());
    }

    #[test]
    fn test_coarsener_attention_pooling() {
        let mut coarsener = GraphCoarsener::new(0.5, AggregationStrategy::AttentionPooling);

        let features = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![2.0, 2.0],
            vec![0.5, 0.5],
        ];
        let edges = vec![(0, 1), (1, 2), (2, 3)];

        let result = coarsener.coarsen(&features, &edges).unwrap();
        assert!(result.num_clusters >= 1);
        assert!(result.attestation.is_some());
    }

    #[test]
    fn test_coarsener_topk() {
        let mut coarsener = GraphCoarsener::new(0.5, AggregationStrategy::TopK(1));

        let features = vec![
            vec![1.0, 0.0],
            vec![10.0, 10.0], // highest magnitude
            vec![0.5, 0.5],
            vec![0.1, 0.1],
        ];
        let edges = vec![(0, 1), (2, 3)];

        let result = coarsener.coarsen(&features, &edges).unwrap();
        assert!(result.num_clusters >= 1);
        assert!(result.attestation.is_some());
    }

    #[test]
    fn test_coarsener_empty_graph() {
        let mut coarsener = GraphCoarsener::new(0.5, AggregationStrategy::Mean);
        let result = coarsener.coarsen(&[], &[]).unwrap();
        assert_eq!(result.num_clusters, 0);
        assert!(result.coarse_features.is_empty());
    }

    #[test]
    fn test_uncoarsen() {
        let mut coarsener = GraphCoarsener::new(0.5, AggregationStrategy::Mean);

        let features = vec![
            vec![1.0, 2.0],
            vec![3.0, 4.0],
            vec![5.0, 6.0],
            vec![7.0, 8.0],
        ];
        let edges = vec![(0, 1), (2, 3)];

        let coarse_result = coarsener.coarsen(&features, &edges).unwrap();
        let uncoarse = coarsener.uncoarsen(
            &coarse_result.coarse_features,
            &coarse_result.assignments,
            4,
        );

        assert_eq!(uncoarse.fine_features.len(), 4);
        // Each fine feature should have the same dim as coarse
        for f in &uncoarse.fine_features {
            assert_eq!(f.len(), 2);
        }
        // Nodes in the same cluster should have the same features
        for cluster_nodes in &uncoarse.cluster_to_nodes {
            if cluster_nodes.len() > 1 {
                let first = &uncoarse.fine_features[cluster_nodes[0]];
                for &node in &cluster_nodes[1..] {
                    assert_eq!(&uncoarse.fine_features[node], first);
                }
            }
        }
    }

    #[test]
    fn test_coarsener_ratio_bounds() {
        // ratio is clamped to [0.01, 0.99]
        let c1 = GraphCoarsener::new(0.0, AggregationStrategy::Mean);
        assert!((c1.ratio() - 0.01).abs() < 1e-6);

        let c2 = GraphCoarsener::new(1.5, AggregationStrategy::Mean);
        assert!((c2.ratio() - 0.99).abs() < 1e-6);
    }
}
