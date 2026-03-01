//! Attention Pattern Learning Framework
//!
//! Four complementary attention mechanisms for intelligent code assistance:
//!
//! | Attention Type | Question Answered | Application |
//! |---------------|-------------------|-------------|
//! | **Neural** | What words matter? | Token/semantic relevance |
//! | **DAG** | What steps matter? | Execution order, dependencies |
//! | **Graph** | What relationships matter? | Code structure, call graphs |
//! | **State Space** | What history still matters? | Context persistence |

use std::collections::HashMap;

// ============================================================================
// NEURAL ATTENTION - "What words matter?"
// ============================================================================

/// Neural attention focuses on token-level and semantic relevance.
/// Used for: Code completion, error messages, documentation search.
#[derive(Debug, Clone)]
pub struct NeuralAttention {
    /// Attention weights per token position
    weights: Vec<f32>,
    /// Token importance scores
    token_scores: HashMap<String, f32>,
    /// Semantic embeddings dimension
    dim: usize,
}

impl NeuralAttention {
    pub fn new(dim: usize) -> Self {
        Self {
            weights: Vec::new(),
            token_scores: HashMap::new(),
            dim,
        }
    }

    /// Compute attention weights for tokens
    /// Q: Query (what we're looking for)
    /// K: Keys (what we're comparing against)
    /// V: Values (what we extract)
    pub fn attend(&mut self, query: &[f32], keys: &[Vec<f32>], values: &[String]) -> Vec<(String, f32)> {
        if keys.is_empty() || keys.len() != values.len() {
            return Vec::new();
        }

        // Scaled dot-product attention
        let scale = (self.dim as f32).sqrt();
        self.weights = keys.iter().map(|k| {
            let dot: f32 = query.iter().zip(k.iter()).map(|(q, k)| q * k).sum();
            dot / scale
        }).collect();

        // Softmax normalization
        let max_weight = self.weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_weights: Vec<f32> = self.weights.iter().map(|w| (w - max_weight).exp()).collect();
        let sum: f32 = exp_weights.iter().sum();
        self.weights = exp_weights.iter().map(|w| w / sum).collect();

        // Return weighted values
        values.iter()
            .zip(self.weights.iter())
            .map(|(v, w)| (v.clone(), *w))
            .collect()
    }

    /// Learn which tokens are important from successful completions
    pub fn learn_token_importance(&mut self, token: &str, success: bool) {
        let score = self.token_scores.entry(token.to_string()).or_insert(0.5);
        let reward = if success { 1.0 } else { 0.0 };
        *score = *score + 0.1 * (reward - *score); // Q-learning update
    }

    /// Get importance score for a token
    pub fn token_importance(&self, token: &str) -> f32 {
        *self.token_scores.get(token).unwrap_or(&0.5)
    }
}

// ============================================================================
// DAG ATTENTION - "What steps matter?"
// ============================================================================

/// DAG (Directed Acyclic Graph) attention for execution order and dependencies.
/// Used for: Build systems, test ordering, refactoring sequences.
#[derive(Debug, Clone)]
pub struct DagAttention {
    /// Nodes in the DAG (tasks/steps)
    nodes: Vec<DagNode>,
    /// Edges (dependencies)
    edges: Vec<(usize, usize, f32)>, // (from, to, weight)
    /// Topological order cache
    topo_order: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct DagNode {
    pub id: String,
    pub step_type: StepType,
    pub importance: f32,
    pub completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepType {
    /// Configuration/setup step
    Config,
    /// Source code modification
    Source,
    /// Test execution
    Test,
    /// Build/compile step
    Build,
    /// Deployment step
    Deploy,
}

impl DagAttention {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            topo_order: Vec::new(),
        }
    }

    /// Add a step to the DAG
    pub fn add_step(&mut self, id: &str, step_type: StepType) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(DagNode {
            id: id.to_string(),
            step_type,
            importance: 0.5,
            completed: false,
        });
        self.invalidate_topo();
        idx
    }

    /// Add a dependency edge
    pub fn add_dependency(&mut self, from: usize, to: usize, weight: f32) {
        self.edges.push((from, to, weight));
        self.invalidate_topo();
    }

    /// Invalidate topological order cache
    fn invalidate_topo(&mut self) {
        self.topo_order.clear();
    }

    /// Compute topological order (what order to execute steps)
    pub fn compute_order(&mut self) -> Vec<&DagNode> {
        if self.topo_order.is_empty() {
            self.topo_order = self.kahn_sort();
        }
        self.topo_order.iter().map(|&i| &self.nodes[i]).collect()
    }

    /// Kahn's algorithm for topological sort
    fn kahn_sort(&self) -> Vec<usize> {
        let n = self.nodes.len();
        let mut in_degree = vec![0usize; n];
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];

        for &(from, to, _) in &self.edges {
            adj[from].push(to);
            in_degree[to] += 1;
        }

        let mut queue: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
        let mut result = Vec::new();

        while let Some(node) = queue.pop() {
            result.push(node);
            for &next in &adj[node] {
                in_degree[next] -= 1;
                if in_degree[next] == 0 {
                    queue.push(next);
                }
            }
        }

        result
    }

    /// Get critical path (most important sequence of steps)
    pub fn critical_path(&self) -> Vec<&DagNode> {
        // Find path with highest total importance
        let order = self.kahn_sort();
        let mut max_path = Vec::new();
        let mut max_importance = 0.0f32;

        // Simple greedy: follow highest importance edges
        if let Some(&start) = order.first() {
            let mut path = vec![start];
            let mut current = start;
            let mut importance = self.nodes[start].importance;

            while let Some(&(_, to, weight)) = self.edges.iter()
                .filter(|(from, _, _)| *from == current)
                .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
            {
                path.push(to);
                importance += self.nodes[to].importance * weight;
                current = to;
            }

            if importance > max_importance {
                max_importance = importance;
                max_path = path;
            }
        }

        max_path.iter().map(|&i| &self.nodes[i]).collect()
    }

    /// Learn step importance from execution outcomes
    pub fn learn_step_importance(&mut self, step_id: &str, success: bool) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == step_id) {
            let reward = if success { 1.0 } else { 0.0 };
            node.importance = node.importance + 0.1 * (reward - node.importance);
        }
    }
}

impl Default for DagAttention {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// GRAPH ATTENTION - "What relationships matter?"
// ============================================================================

/// Graph attention for code structure and relationships.
/// Used for: Call graphs, module dependencies, refactoring impact analysis.
#[derive(Debug, Clone)]
pub struct GraphAttention {
    /// Nodes (functions, modules, files)
    nodes: HashMap<String, GraphNode>,
    /// Edges with attention weights
    edges: Vec<GraphEdge>,
    /// Multi-head attention heads
    num_heads: usize,
}

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub node_type: NodeType,
    pub features: Vec<f32>,
    pub attention_score: f32,
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub edge_type: EdgeType,
    pub weight: f32,
    pub attention_weights: Vec<f32>, // Per head
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Function,
    Module,
    File,
    Crate,
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    Calls,
    Imports,
    DependsOn,
    Tests,
    Contains,
}

impl GraphAttention {
    pub fn new(num_heads: usize) -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            num_heads,
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, id: &str, node_type: NodeType, features: Vec<f32>) {
        self.nodes.insert(id.to_string(), GraphNode {
            id: id.to_string(),
            node_type,
            features,
            attention_score: 0.0,
        });
    }

    /// Add an edge with relationship type
    pub fn add_edge(&mut self, source: &str, target: &str, edge_type: EdgeType) {
        self.edges.push(GraphEdge {
            source: source.to_string(),
            target: target.to_string(),
            edge_type,
            weight: 1.0,
            attention_weights: vec![1.0 / self.num_heads as f32; self.num_heads],
        });
    }

    /// Compute graph attention (simplified GAT-style)
    pub fn compute_attention(&mut self, focus_node: &str) {
        // Reset attention scores
        for node in self.nodes.values_mut() {
            node.attention_score = 0.0;
        }

        // Aggregate attention from edges
        for edge in &self.edges {
            if edge.source == focus_node || edge.target == focus_node {
                let other = if edge.source == focus_node { &edge.target } else { &edge.source };
                if let Some(node) = self.nodes.get_mut(other) {
                    // Sum multi-head attention
                    let attention: f32 = edge.attention_weights.iter().sum();
                    node.attention_score += attention * edge.weight;
                }
            }
        }

        // Normalize
        let max_score = self.nodes.values()
            .map(|n| n.attention_score)
            .fold(0.0f32, f32::max);
        if max_score > 0.0 {
            for node in self.nodes.values_mut() {
                node.attention_score /= max_score;
            }
        }
    }

    /// Get most important neighbors of a node
    pub fn important_neighbors(&self, node_id: &str, top_k: usize) -> Vec<(&GraphNode, f32)> {
        let mut neighbors: Vec<_> = self.edges.iter()
            .filter(|e| e.source == node_id || e.target == node_id)
            .filter_map(|e| {
                let other = if e.source == node_id { &e.target } else { &e.source };
                self.nodes.get(other).map(|n| (n, e.weight))
            })
            .collect();

        neighbors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        neighbors.truncate(top_k);
        neighbors
    }

    /// Learn relationship importance from interaction patterns
    pub fn learn_edge_importance(&mut self, source: &str, target: &str, success: bool) {
        if let Some(edge) = self.edges.iter_mut()
            .find(|e| e.source == source && e.target == target)
        {
            let reward = if success { 1.0 } else { 0.0 };
            edge.weight = edge.weight + 0.1 * (reward - edge.weight);
        }
    }
}

// ============================================================================
// STATE SPACE ATTENTION - "What history still matters?"
// ============================================================================

/// State space model for context persistence and history relevance.
/// Used for: Session memory, conversation context, learning trajectories.
#[derive(Debug, Clone)]
pub struct StateSpaceAttention {
    /// Hidden state dimension
    state_dim: usize,
    /// Current hidden state
    hidden_state: Vec<f32>,
    /// State transition matrix (learned)
    transition: Vec<Vec<f32>>,
    /// Input projection
    input_proj: Vec<Vec<f32>>,
    /// Output projection
    output_proj: Vec<Vec<f32>>,
    /// History buffer with decay
    history: Vec<HistoryEntry>,
    /// Maximum history length
    max_history: usize,
    /// Decay factor for old history
    decay: f32,
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub content: String,
    pub state: Vec<f32>,
    pub relevance: f32,
    pub timestamp: u64,
}

impl StateSpaceAttention {
    pub fn new(state_dim: usize, max_history: usize) -> Self {
        // Initialize with identity-like transition
        let mut transition = vec![vec![0.0; state_dim]; state_dim];
        for i in 0..state_dim {
            transition[i][i] = 0.9; // Slight decay
        }

        Self {
            state_dim,
            hidden_state: vec![0.0; state_dim],
            transition,
            input_proj: vec![vec![0.1; state_dim]; state_dim],
            output_proj: vec![vec![0.1; state_dim]; state_dim],
            history: Vec::new(),
            max_history,
            decay: 0.95,
        }
    }

    /// Update state with new input
    pub fn update(&mut self, input: &[f32], content: &str) {
        // State transition: h_t = A * h_{t-1} + B * x_t
        let mut new_state = vec![0.0; self.state_dim];

        // Apply transition matrix
        for i in 0..self.state_dim {
            for j in 0..self.state_dim {
                new_state[i] += self.transition[i][j] * self.hidden_state[j];
            }
        }

        // Add input contribution
        let input_len = input.len().min(self.state_dim);
        for i in 0..input_len {
            for j in 0..self.state_dim {
                new_state[j] += self.input_proj[j][i % self.state_dim] * input[i];
            }
        }

        // Normalize
        let norm: f32 = new_state.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut new_state {
                *x /= norm;
            }
        }

        // Store in history
        self.history.push(HistoryEntry {
            content: content.to_string(),
            state: self.hidden_state.clone(),
            relevance: 1.0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });

        // Trim history
        while self.history.len() > self.max_history {
            self.history.remove(0);
        }

        // Apply decay to old entries
        for entry in &mut self.history {
            entry.relevance *= self.decay;
        }

        self.hidden_state = new_state;
    }

    /// Query what history is still relevant
    pub fn relevant_history(&self, query: &[f32], top_k: usize) -> Vec<&HistoryEntry> {
        let mut scored: Vec<_> = self.history.iter()
            .map(|entry| {
                // Cosine similarity between query and historical state
                let dot: f32 = query.iter()
                    .zip(entry.state.iter())
                    .map(|(q, s)| q * s)
                    .sum();
                let query_norm: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
                let state_norm: f32 = entry.state.iter().map(|x| x * x).sum::<f32>().sqrt();
                let similarity = if query_norm > 0.0 && state_norm > 0.0 {
                    dot / (query_norm * state_norm)
                } else {
                    0.0
                };
                (entry, similarity * entry.relevance)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(top_k);
        scored.into_iter().map(|(e, _)| e).collect()
    }

    /// Get current state
    pub fn current_state(&self) -> &[f32] {
        &self.hidden_state
    }

    /// Learn from successful context usage
    pub fn reinforce_history(&mut self, content: &str, success: bool) {
        if let Some(entry) = self.history.iter_mut().find(|e| e.content == content) {
            let reward = if success { 1.5 } else { 0.5 };
            entry.relevance = (entry.relevance * reward).min(1.0);
        }
    }
}

// ============================================================================
// UNIFIED ATTENTION ORCHESTRATOR
// ============================================================================

/// Combines all four attention mechanisms for comprehensive code understanding.
pub struct AttentionOrchestrator {
    pub neural: NeuralAttention,
    pub dag: DagAttention,
    pub graph: GraphAttention,
    pub state_space: StateSpaceAttention,
}

impl AttentionOrchestrator {
    pub fn new(embedding_dim: usize, state_dim: usize, max_history: usize) -> Self {
        Self {
            neural: NeuralAttention::new(embedding_dim),
            dag: DagAttention::new(),
            graph: GraphAttention::new(4), // 4 attention heads
            state_space: StateSpaceAttention::new(state_dim, max_history),
        }
    }

    /// Answer all four attention questions for a given context
    pub fn analyze(&mut self, query: &str, file: &str) -> AttentionAnalysis {
        AttentionAnalysis {
            words_that_matter: self.analyze_words(query),
            steps_that_matter: self.analyze_steps(file),
            relationships_that_matter: self.analyze_relationships(file),
            history_that_matters: self.analyze_history(query),
        }
    }

    fn analyze_words(&self, query: &str) -> Vec<(String, f32)> {
        query.split_whitespace()
            .map(|word| (word.to_string(), self.neural.token_importance(word)))
            .collect()
    }

    fn analyze_steps(&self, file: &str) -> Vec<String> {
        self.dag.compute_order()
            .iter()
            .filter(|node| !node.completed)
            .take(5)
            .map(|node| node.id.clone())
            .collect()
    }

    fn analyze_relationships(&self, file: &str) -> Vec<String> {
        self.graph.important_neighbors(file, 5)
            .iter()
            .map(|(node, _)| node.id.clone())
            .collect()
    }

    fn analyze_history(&self, query: &str) -> Vec<String> {
        // Simple embedding from query
        let query_embedding: Vec<f32> = query.chars()
            .take(64)
            .map(|c| (c as u8 as f32) / 255.0)
            .collect();

        self.state_space.relevant_history(&query_embedding, 5)
            .iter()
            .map(|entry| entry.content.clone())
            .collect()
    }
}

/// Result of attention analysis
#[derive(Debug, Clone)]
pub struct AttentionAnalysis {
    /// Neural attention: What words matter?
    pub words_that_matter: Vec<(String, f32)>,
    /// DAG attention: What steps matter?
    pub steps_that_matter: Vec<String>,
    /// Graph attention: What relationships matter?
    pub relationships_that_matter: Vec<String>,
    /// State space: What history still matters?
    pub history_that_matters: Vec<String>,
}

impl std::fmt::Display for AttentionAnalysis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "🧠 Attention Analysis")?;
        writeln!(f, "━━━━━━━━━━━━━━━━━━━━")?;

        writeln!(f, "\n📝 Neural Attention (What words matter?):")?;
        for (word, score) in &self.words_that_matter {
            writeln!(f, "   • {} ({:.2})", word, score)?;
        }

        writeln!(f, "\n📊 DAG Attention (What steps matter?):")?;
        for step in &self.steps_that_matter {
            writeln!(f, "   → {}", step)?;
        }

        writeln!(f, "\n🔗 Graph Attention (What relationships matter?):")?;
        for rel in &self.relationships_that_matter {
            writeln!(f, "   ↔ {}", rel)?;
        }

        writeln!(f, "\n📚 State Space (What history still matters?):")?;
        for hist in &self.history_that_matters {
            writeln!(f, "   ◷ {}", hist)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neural_attention() {
        let mut attn = NeuralAttention::new(64);
        attn.learn_token_importance("fn", true);
        attn.learn_token_importance("fn", true);
        assert!(attn.token_importance("fn") > 0.5);
    }

    #[test]
    fn test_dag_attention() {
        let mut dag = DagAttention::new();
        let config = dag.add_step("config", StepType::Config);
        let build = dag.add_step("build", StepType::Build);
        let test = dag.add_step("test", StepType::Test);

        dag.add_dependency(config, build, 1.0);
        dag.add_dependency(build, test, 1.0);

        let order = dag.compute_order();
        assert_eq!(order.len(), 3);
        assert_eq!(order[0].id, "config");
    }

    #[test]
    fn test_graph_attention() {
        let mut graph = GraphAttention::new(4);
        graph.add_node("main.rs", NodeType::File, vec![1.0; 64]);
        graph.add_node("lib.rs", NodeType::File, vec![0.5; 64]);
        graph.add_edge("main.rs", "lib.rs", EdgeType::Imports);

        graph.compute_attention("main.rs");
        let neighbors = graph.important_neighbors("main.rs", 5);
        assert!(!neighbors.is_empty());
    }

    #[test]
    fn test_state_space_attention() {
        let mut ssm = StateSpaceAttention::new(32, 100);
        ssm.update(&[0.5; 32], "First context");
        ssm.update(&[0.7; 32], "Second context");

        let relevant = ssm.relevant_history(&[0.6; 32], 2);
        assert!(!relevant.is_empty());
    }

    #[test]
    fn test_attention_orchestrator() {
        let mut orch = AttentionOrchestrator::new(64, 32, 100);
        let analysis = orch.analyze("implement error handling", "src/lib.rs");

        assert!(!analysis.words_that_matter.is_empty());
        println!("{}", analysis);
    }
}
