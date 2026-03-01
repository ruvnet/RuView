//! Domain entities for the learning bounded context.
//!
//! This module defines the core domain entities including:
//! - Learning sessions for tracking training state
//! - GNN model types and training metrics
//! - Transition graphs for embedding relationships
//! - Refined embeddings as output of the learning process

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for an embedding
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbeddingId(pub String);

impl EmbeddingId {
    /// Create a new embedding ID
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a new random embedding ID
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Get the inner string value
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for EmbeddingId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for EmbeddingId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Timestamp type alias for consistency
pub type Timestamp = DateTime<Utc>;

/// Types of GNN models supported by the learning system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GnnModelType {
    /// Graph Convolutional Network
    /// Uses spectral convolutions on graph-structured data
    Gcn,
    /// GraphSAGE (SAmple and aggreGatE)
    /// Learns node embeddings through neighborhood sampling and aggregation
    GraphSage,
    /// Graph Attention Network
    /// Uses attention mechanisms to weight neighbor contributions
    Gat,
}

impl Default for GnnModelType {
    fn default() -> Self {
        Self::Gcn
    }
}

impl GnnModelType {
    /// Get the number of learnable parameters per layer (approximate)
    #[must_use]
    pub fn params_per_layer(&self, input_dim: usize, output_dim: usize) -> usize {
        match self {
            Self::Gcn => input_dim * output_dim + output_dim,
            Self::GraphSage => 2 * input_dim * output_dim + output_dim,
            Self::Gat => input_dim * output_dim + 2 * output_dim,
        }
    }

    /// Get recommended number of attention heads (only relevant for GAT)
    #[must_use]
    pub fn recommended_heads(&self) -> usize {
        match self {
            Self::Gat => 8,
            _ => 1,
        }
    }
}

/// Status of a training session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingStatus {
    /// Session created but training not started
    Pending,
    /// Training is currently running
    Running,
    /// Training completed successfully
    Completed,
    /// Training failed with an error
    Failed,
    /// Training was paused
    Paused,
    /// Training was cancelled by user
    Cancelled,
}

impl TrainingStatus {
    /// Check if the status represents a terminal state
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Check if training can be resumed from this status
    #[must_use]
    pub fn can_resume(&self) -> bool {
        matches!(self, Self::Paused)
    }

    /// Check if training is active
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running)
    }
}

/// Metrics collected during training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingMetrics {
    /// Current loss value
    pub loss: f32,
    /// Training accuracy (0.0 to 1.0)
    pub accuracy: f32,
    /// Current epoch number
    pub epoch: usize,
    /// Current learning rate
    pub learning_rate: f32,
    /// Validation loss (if validation set provided)
    pub validation_loss: Option<f32>,
    /// Validation accuracy
    pub validation_accuracy: Option<f32>,
    /// Gradient norm (for monitoring stability)
    pub gradient_norm: Option<f32>,
    /// Time taken for this epoch in milliseconds
    pub epoch_time_ms: u64,
    /// Additional custom metrics
    #[serde(default)]
    pub custom_metrics: HashMap<String, f32>,
}

impl Default for TrainingMetrics {
    fn default() -> Self {
        Self {
            loss: f32::INFINITY,
            accuracy: 0.0,
            epoch: 0,
            learning_rate: 0.001,
            validation_loss: None,
            validation_accuracy: None,
            gradient_norm: None,
            epoch_time_ms: 0,
            custom_metrics: HashMap::new(),
        }
    }
}

impl TrainingMetrics {
    /// Create new metrics for an epoch
    #[must_use]
    pub fn new(epoch: usize, loss: f32, accuracy: f32, learning_rate: f32) -> Self {
        Self {
            loss,
            accuracy,
            epoch,
            learning_rate,
            ..Default::default()
        }
    }

    /// Set validation metrics
    #[must_use]
    pub fn with_validation(mut self, loss: f32, accuracy: f32) -> Self {
        self.validation_loss = Some(loss);
        self.validation_accuracy = Some(accuracy);
        self
    }

    /// Add a custom metric
    pub fn add_custom_metric(&mut self, name: impl Into<String>, value: f32) {
        self.custom_metrics.insert(name.into(), value);
    }

    /// Check if training is converging (loss is decreasing)
    #[must_use]
    pub fn is_improving(&self, previous: &Self) -> bool {
        self.loss < previous.loss
    }
}

/// Hyperparameters for training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperParameters {
    /// Initial learning rate
    pub learning_rate: f32,
    /// Weight decay (L2 regularization)
    pub weight_decay: f32,
    /// Dropout probability
    pub dropout: f32,
    /// Number of training epochs
    pub epochs: usize,
    /// Batch size for training
    pub batch_size: usize,
    /// Early stopping patience (epochs without improvement)
    pub early_stopping_patience: Option<usize>,
    /// Gradient clipping threshold
    pub gradient_clip: Option<f32>,
    /// Temperature for contrastive loss
    pub temperature: f32,
    /// Margin for triplet loss
    pub triplet_margin: f32,
    /// EWC lambda (importance of old task knowledge)
    pub ewc_lambda: f32,
    /// Number of GNN layers
    pub num_layers: usize,
    /// Hidden dimension size
    pub hidden_dim: usize,
    /// Number of attention heads (for GAT)
    pub num_heads: usize,
    /// Negative sample ratio for contrastive learning
    pub negative_ratio: usize,
}

impl Default for HyperParameters {
    fn default() -> Self {
        Self {
            learning_rate: 0.001,
            weight_decay: 5e-4,
            dropout: 0.5,
            epochs: 200,
            batch_size: 32,
            early_stopping_patience: Some(20),
            gradient_clip: Some(1.0),
            temperature: 0.07,
            triplet_margin: 1.0,
            ewc_lambda: 5000.0,
            num_layers: 2,
            hidden_dim: 256,
            num_heads: 8,
            negative_ratio: 5,
        }
    }
}

/// Configuration for the learning service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningConfig {
    /// Type of GNN model to use
    pub model_type: GnnModelType,
    /// Input embedding dimension
    pub input_dim: usize,
    /// Output embedding dimension
    pub output_dim: usize,
    /// Training hyperparameters
    pub hyperparameters: HyperParameters,
    /// Enable mixed precision training
    pub mixed_precision: bool,
    /// Device to use for training
    pub device: Device,
    /// Random seed for reproducibility
    pub seed: Option<u64>,
    /// Enable gradient checkpointing to save memory
    pub gradient_checkpointing: bool,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            model_type: GnnModelType::Gcn,
            input_dim: 768,
            output_dim: 256,
            hyperparameters: HyperParameters::default(),
            mixed_precision: false,
            device: Device::Cpu,
            seed: None,
            gradient_checkpointing: false,
        }
    }
}

/// Device for computation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Device {
    /// CPU computation
    #[default]
    Cpu,
    /// CUDA GPU computation
    Cuda(usize),
    /// Metal GPU (Apple Silicon)
    Metal,
}

/// A learning session tracking the state of a training run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSession {
    /// Unique session identifier
    pub id: String,
    /// Type of GNN model being trained
    pub model_type: GnnModelType,
    /// Current training status
    pub status: TrainingStatus,
    /// Current training metrics
    pub metrics: TrainingMetrics,
    /// When the session was started
    pub started_at: Timestamp,
    /// When the session was last updated
    pub updated_at: Timestamp,
    /// When the session completed (if applicable)
    pub completed_at: Option<Timestamp>,
    /// Configuration used for this session
    pub config: LearningConfig,
    /// History of metrics per epoch
    #[serde(default)]
    pub metrics_history: Vec<TrainingMetrics>,
    /// Best metrics achieved during training
    pub best_metrics: Option<TrainingMetrics>,
    /// Error message if training failed
    pub error_message: Option<String>,
    /// Number of checkpoints saved
    pub checkpoint_count: usize,
}

impl LearningSession {
    /// Create a new learning session
    #[must_use]
    pub fn new(config: LearningConfig) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            model_type: config.model_type,
            status: TrainingStatus::Pending,
            metrics: TrainingMetrics::default(),
            started_at: now,
            updated_at: now,
            completed_at: None,
            config,
            metrics_history: Vec::new(),
            best_metrics: None,
            error_message: None,
            checkpoint_count: 0,
        }
    }

    /// Start the training session
    pub fn start(&mut self) {
        self.status = TrainingStatus::Running;
        self.updated_at = Utc::now();
    }

    /// Update metrics for a completed epoch
    pub fn update_metrics(&mut self, metrics: TrainingMetrics) {
        // Update best metrics if this is an improvement
        if self.best_metrics.is_none()
            || metrics.loss < self.best_metrics.as_ref().unwrap().loss
        {
            self.best_metrics = Some(metrics.clone());
        }

        self.metrics = metrics.clone();
        self.metrics_history.push(metrics);
        self.updated_at = Utc::now();
    }

    /// Mark the session as completed
    pub fn complete(&mut self) {
        self.status = TrainingStatus::Completed;
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Mark the session as failed
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = TrainingStatus::Failed;
        self.error_message = Some(error.into());
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Pause the training session
    pub fn pause(&mut self) {
        if self.status == TrainingStatus::Running {
            self.status = TrainingStatus::Paused;
            self.updated_at = Utc::now();
        }
    }

    /// Resume a paused session
    pub fn resume(&mut self) {
        if self.status == TrainingStatus::Paused {
            self.status = TrainingStatus::Running;
            self.updated_at = Utc::now();
        }
    }

    /// Get the training duration
    #[must_use]
    pub fn duration(&self) -> chrono::Duration {
        let end = self.completed_at.unwrap_or_else(Utc::now);
        end - self.started_at
    }

    /// Check if training should stop early
    #[must_use]
    pub fn should_early_stop(&self) -> bool {
        if let Some(patience) = self.config.hyperparameters.early_stopping_patience {
            if self.metrics_history.len() <= patience {
                return false;
            }

            let best_epoch = self
                .metrics_history
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.loss.partial_cmp(&b.loss).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0);

            self.metrics_history.len() - best_epoch > patience
        } else {
            false
        }
    }
}

/// A node in the transition graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// Embedding ID for this node
    pub id: EmbeddingId,
    /// The embedding vector
    pub embedding: Vec<f32>,
    /// Optional node features
    pub features: Option<Vec<f32>>,
    /// Node label (for supervised learning)
    pub label: Option<usize>,
    /// Metadata associated with this node
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl GraphNode {
    /// Create a new graph node
    #[must_use]
    pub fn new(id: EmbeddingId, embedding: Vec<f32>) -> Self {
        Self {
            id,
            embedding,
            features: None,
            label: None,
            metadata: HashMap::new(),
        }
    }

    /// Get the embedding dimension
    #[must_use]
    pub fn dim(&self) -> usize {
        self.embedding.len()
    }
}

/// An edge in the transition graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Source node index
    pub from: usize,
    /// Target node index
    pub to: usize,
    /// Edge weight (e.g., similarity score)
    pub weight: f32,
    /// Edge type for heterogeneous graphs
    pub edge_type: Option<String>,
}

impl GraphEdge {
    /// Create a new edge
    #[must_use]
    pub fn new(from: usize, to: usize, weight: f32) -> Self {
        Self {
            from,
            to,
            weight,
            edge_type: None,
        }
    }

    /// Create a typed edge
    #[must_use]
    pub fn typed(from: usize, to: usize, weight: f32, edge_type: impl Into<String>) -> Self {
        Self {
            from,
            to,
            weight,
            edge_type: Some(edge_type.into()),
        }
    }
}

/// A graph representing transitions between embeddings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionGraph {
    /// Nodes in the graph (embedding IDs)
    pub nodes: Vec<EmbeddingId>,
    /// Node embeddings (parallel to nodes)
    pub embeddings: Vec<Vec<f32>>,
    /// Edges as (from_index, to_index, weight) tuples
    pub edges: Vec<(usize, usize, f32)>,
    /// Optional node labels for supervised learning
    #[serde(default)]
    pub labels: Vec<Option<usize>>,
    /// Number of unique classes (if labeled)
    pub num_classes: Option<usize>,
    /// Whether the graph is directed
    pub directed: bool,
}

impl Default for TransitionGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl TransitionGraph {
    /// Create a new empty transition graph
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            embeddings: Vec::new(),
            edges: Vec::new(),
            labels: Vec::new(),
            num_classes: None,
            directed: true,
        }
    }

    /// Create an undirected graph
    #[must_use]
    pub fn undirected() -> Self {
        Self {
            directed: false,
            ..Self::new()
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, id: EmbeddingId, embedding: Vec<f32>, label: Option<usize>) {
        self.nodes.push(id);
        self.embeddings.push(embedding);
        self.labels.push(label);
    }

    /// Add an edge to the graph
    pub fn add_edge(&mut self, from: usize, to: usize, weight: f32) {
        assert!(from < self.nodes.len(), "Invalid 'from' node index");
        assert!(to < self.nodes.len(), "Invalid 'to' node index");
        self.edges.push((from, to, weight));

        // For undirected graphs, add reverse edge
        if !self.directed {
            self.edges.push((to, from, weight));
        }
    }

    /// Get the number of nodes
    #[must_use]
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of edges
    #[must_use]
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// Get the embedding dimension (assumes all embeddings have same dimension)
    #[must_use]
    pub fn embedding_dim(&self) -> Option<usize> {
        self.embeddings.first().map(Vec::len)
    }

    /// Get neighbors of a node
    #[must_use]
    pub fn neighbors(&self, node_idx: usize) -> Vec<(usize, f32)> {
        self.edges
            .iter()
            .filter(|(from, _, _)| *from == node_idx)
            .map(|(_, to, weight)| (*to, *weight))
            .collect()
    }

    /// Get the adjacency list representation
    #[must_use]
    pub fn adjacency_list(&self) -> Vec<Vec<(usize, f32)>> {
        let mut adj = vec![Vec::new(); self.nodes.len()];
        for &(from, to, weight) in &self.edges {
            adj[from].push((to, weight));
        }
        adj
    }

    /// Compute node degrees
    #[must_use]
    pub fn degrees(&self) -> Vec<usize> {
        let mut degrees = vec![0; self.nodes.len()];
        for &(from, to, _) in &self.edges {
            degrees[from] += 1;
            if !self.directed {
                degrees[to] += 1;
            }
        }
        degrees
    }

    /// Validate the graph structure
    pub fn validate(&self) -> Result<(), String> {
        if self.nodes.len() != self.embeddings.len() {
            return Err("Nodes and embeddings count mismatch".to_string());
        }
        if !self.labels.is_empty() && self.labels.len() != self.nodes.len() {
            return Err("Labels count mismatch".to_string());
        }
        for &(from, to, _) in &self.edges {
            if from >= self.nodes.len() || to >= self.nodes.len() {
                return Err(format!("Invalid edge: ({from}, {to})"));
            }
        }
        Ok(())
    }
}

/// A refined embedding produced by the learning process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefinedEmbedding {
    /// ID of the original embedding that was refined
    pub original_id: EmbeddingId,
    /// The refined embedding vector
    pub refined_vector: Vec<f32>,
    /// Score indicating quality of refinement (0.0 to 1.0)
    pub refinement_score: f32,
    /// The session that produced this refinement
    pub session_id: Option<String>,
    /// Timestamp of refinement
    pub refined_at: Timestamp,
    /// Delta from original (optional, for analysis)
    pub delta_norm: Option<f32>,
    /// Confidence in the refinement
    pub confidence: f32,
}

impl RefinedEmbedding {
    /// Create a new refined embedding
    #[must_use]
    pub fn new(
        original_id: EmbeddingId,
        refined_vector: Vec<f32>,
        refinement_score: f32,
    ) -> Self {
        Self {
            original_id,
            refined_vector,
            refinement_score,
            session_id: None,
            refined_at: Utc::now(),
            delta_norm: None,
            confidence: refinement_score,
        }
    }

    /// Compute the delta norm from original embedding
    pub fn compute_delta(&mut self, original: &[f32]) {
        if original.len() != self.refined_vector.len() {
            return;
        }
        let delta: f32 = original
            .iter()
            .zip(&self.refined_vector)
            .map(|(a, b)| (a - b).powi(2))
            .sum();
        self.delta_norm = Some(delta.sqrt());
    }

    /// Get the embedding dimension
    #[must_use]
    pub fn dim(&self) -> usize {
        self.refined_vector.len()
    }

    /// Normalize the refined vector to unit length
    pub fn normalize(&mut self) {
        let norm: f32 = self.refined_vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-10 {
            for x in &mut self.refined_vector {
                *x /= norm;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_id() {
        let id = EmbeddingId::new("test-123");
        assert_eq!(id.as_str(), "test-123");

        let generated = EmbeddingId::generate();
        assert!(!generated.as_str().is_empty());
    }

    #[test]
    fn test_gnn_model_type() {
        assert_eq!(GnnModelType::default(), GnnModelType::Gcn);
        assert_eq!(GnnModelType::Gat.recommended_heads(), 8);
        assert_eq!(GnnModelType::Gcn.recommended_heads(), 1);
    }

    #[test]
    fn test_training_status() {
        assert!(!TrainingStatus::Running.is_terminal());
        assert!(TrainingStatus::Completed.is_terminal());
        assert!(TrainingStatus::Failed.is_terminal());
        assert!(TrainingStatus::Paused.can_resume());
        assert!(!TrainingStatus::Completed.can_resume());
    }

    #[test]
    fn test_training_metrics() {
        let metrics = TrainingMetrics::new(1, 0.5, 0.8, 0.001);
        assert_eq!(metrics.epoch, 1);
        assert_eq!(metrics.loss, 0.5);

        let better = TrainingMetrics::new(2, 0.3, 0.9, 0.001);
        assert!(better.is_improving(&metrics));
    }

    #[test]
    fn test_learning_session() {
        let config = LearningConfig::default();
        let mut session = LearningSession::new(config);

        assert_eq!(session.status, TrainingStatus::Pending);

        session.start();
        assert_eq!(session.status, TrainingStatus::Running);

        let metrics = TrainingMetrics::new(1, 0.5, 0.8, 0.001);
        session.update_metrics(metrics);
        assert_eq!(session.metrics_history.len(), 1);

        session.complete();
        assert_eq!(session.status, TrainingStatus::Completed);
        assert!(session.completed_at.is_some());
    }

    #[test]
    fn test_transition_graph() {
        let mut graph = TransitionGraph::new();

        let emb1 = vec![0.1, 0.2, 0.3];
        let emb2 = vec![0.4, 0.5, 0.6];

        graph.add_node(EmbeddingId::new("n1"), emb1, Some(0));
        graph.add_node(EmbeddingId::new("n2"), emb2, Some(1));
        graph.add_edge(0, 1, 0.8);

        assert_eq!(graph.num_nodes(), 2);
        assert_eq!(graph.num_edges(), 1);
        assert_eq!(graph.embedding_dim(), Some(3));

        let neighbors = graph.neighbors(0);
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0], (1, 0.8));

        assert!(graph.validate().is_ok());
    }

    #[test]
    fn test_refined_embedding() {
        let original = vec![1.0, 0.0, 0.0];
        let refined = vec![0.9, 0.1, 0.0];

        let mut re = RefinedEmbedding::new(
            EmbeddingId::new("test"),
            refined,
            0.95,
        );

        re.compute_delta(&original);
        assert!(re.delta_norm.is_some());

        re.normalize();
        let norm: f32 = re.refined_vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_early_stopping() {
        let mut config = LearningConfig::default();
        config.hyperparameters.early_stopping_patience = Some(3);

        let mut session = LearningSession::new(config);
        session.start();

        // Improving metrics
        for i in 0..5 {
            let loss = 1.0 - (i as f32 * 0.1);
            session.update_metrics(TrainingMetrics::new(i, loss, 0.8, 0.001));
        }
        assert!(!session.should_early_stop());

        // Non-improving metrics
        for i in 5..10 {
            session.update_metrics(TrainingMetrics::new(i, 0.6, 0.8, 0.001));
        }
        assert!(session.should_early_stop());
    }
}
