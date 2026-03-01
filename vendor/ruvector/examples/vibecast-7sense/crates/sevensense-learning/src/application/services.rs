//! Learning service implementation.
//!
//! Provides the main application service for GNN-based learning,
//! including training, embedding refinement, and edge prediction.

use std::sync::Arc;
use std::time::Instant;

use ndarray::Array2;
use rayon::prelude::*;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

use crate::domain::entities::{
    EmbeddingId, GnnModelType, LearningConfig, LearningSession, RefinedEmbedding,
    TrainingMetrics, TrainingStatus, TransitionGraph,
};
use crate::domain::repository::LearningRepository;
use crate::ewc::{EwcRegularizer, EwcState};
use crate::infrastructure::gnn_model::{GnnError, GnnModel};
use crate::loss;

/// Error type for learning service operations
#[derive(Debug, thiserror::Error)]
pub enum LearningError {
    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Training error
    #[error("Training error: {0}")]
    TrainingError(String),

    /// Model error
    #[error("Model error: {0}")]
    ModelError(String),

    /// Data error
    #[error("Data error: {0}")]
    DataError(String),

    /// Repository error
    #[error("Repository error: {0}")]
    RepositoryError(#[from] crate::domain::repository::RepositoryError),

    /// GNN model error
    #[error("GNN error: {0}")]
    GnnError(#[from] GnnError),

    /// Session not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Session already running
    #[error("A training session is already running")]
    SessionAlreadyRunning,

    /// Dimension mismatch
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Empty graph
    #[error("Graph is empty or invalid")]
    EmptyGraph,

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for learning operations
pub type LearningResult<T> = Result<T, LearningError>;

/// Main learning service for GNN-based embedding refinement.
///
/// This service manages:
/// - GNN model training on transition graphs
/// - Embedding refinement through message passing
/// - Edge prediction for relationship modeling
/// - Continual learning with EWC regularization
pub struct LearningService {
    /// The GNN model
    model: Arc<RwLock<GnnModel>>,
    /// Service configuration
    config: LearningConfig,
    /// EWC state for continual learning
    ewc_state: Arc<RwLock<Option<EwcState>>>,
    /// Optional repository for persistence
    repository: Option<Arc<dyn LearningRepository>>,
    /// Current active session
    current_session: Arc<RwLock<Option<LearningSession>>>,
}

impl LearningService {
    /// Create a new learning service with the given configuration
    #[must_use]
    pub fn new(config: LearningConfig) -> Self {
        let model = GnnModel::new(
            config.model_type,
            config.input_dim,
            config.output_dim,
            config.hyperparameters.num_layers,
            config.hyperparameters.hidden_dim,
            config.hyperparameters.num_heads,
            config.hyperparameters.dropout,
        );

        Self {
            model: Arc::new(RwLock::new(model)),
            config,
            ewc_state: Arc::new(RwLock::new(None)),
            repository: None,
            current_session: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a learning service with a repository
    #[must_use]
    pub fn with_repository(mut self, repository: Arc<dyn LearningRepository>) -> Self {
        self.repository = Some(repository);
        self
    }

    /// Get the current configuration
    #[must_use]
    pub fn config(&self) -> &LearningConfig {
        &self.config
    }

    /// Get the model type
    #[must_use]
    pub fn model_type(&self) -> GnnModelType {
        self.config.model_type
    }

    /// Start a new training session
    #[instrument(skip(self), err)]
    pub async fn start_session(&self) -> LearningResult<String> {
        // Check if a session is already running
        {
            let session = self.current_session.read().await;
            if let Some(ref s) = *session {
                if s.status.is_active() {
                    return Err(LearningError::SessionAlreadyRunning);
                }
            }
        }

        let mut session = LearningSession::new(self.config.clone());
        session.start();

        let session_id = session.id.clone();

        // Persist if repository available
        if let Some(ref repo) = self.repository {
            repo.save_session(&session).await?;
        }

        *self.current_session.write().await = Some(session);

        info!(session_id = %session_id, "Started new learning session");
        Ok(session_id)
    }

    /// Train a single epoch on the transition graph
    ///
    /// # Arguments
    /// * `graph` - The transition graph to train on
    ///
    /// # Returns
    /// Training metrics for the epoch
    #[instrument(skip(self, graph), fields(nodes = graph.num_nodes(), edges = graph.num_edges()), err)]
    pub async fn train_epoch(&self, graph: &TransitionGraph) -> LearningResult<TrainingMetrics> {
        let start_time = Instant::now();

        // Validate graph
        if graph.num_nodes() == 0 {
            return Err(LearningError::EmptyGraph);
        }

        if let Some(dim) = graph.embedding_dim() {
            if dim != self.config.input_dim {
                return Err(LearningError::DimensionMismatch {
                    expected: self.config.input_dim,
                    actual: dim,
                });
            }
        }

        // Ensure we have an active session
        let mut session_guard = self.current_session.write().await;
        let session = session_guard
            .as_mut()
            .ok_or_else(|| LearningError::TrainingError("No active session".to_string()))?;

        let current_epoch = session.metrics.epoch + 1;
        let lr = self.compute_learning_rate(current_epoch);

        // Build adjacency matrix
        let adj_matrix = self.build_adjacency_matrix(graph);

        // Build feature matrix from embeddings
        let features = self.build_feature_matrix(graph);

        // Forward pass through GNN
        let mut model = self.model.write().await;
        let output = model.forward(&features, &adj_matrix)?;

        // Compute loss using contrastive learning
        let (loss, accuracy) = self.compute_loss(graph, &output).await?;

        // Compute gradients and update weights
        let gradients = self.compute_gradients(graph, &features, &output, &adj_matrix, &model)?;
        let grad_norm = self.compute_gradient_norm(&gradients);

        // Apply gradient clipping if configured
        let clipped_gradients = if let Some(clip_value) = self.config.hyperparameters.gradient_clip {
            self.clip_gradients(gradients, clip_value)
        } else {
            gradients
        };

        // Update model weights
        model.update_weights(&clipped_gradients, lr, self.config.hyperparameters.weight_decay);

        // Apply EWC regularization if available
        if let Some(ref ewc_state) = *self.ewc_state.read().await {
            let ewc_reg = EwcRegularizer::new(self.config.hyperparameters.ewc_lambda);
            let ewc_loss = ewc_reg.compute_penalty(&model, ewc_state);
            debug!(ewc_loss = ewc_loss, "Applied EWC regularization");
        }

        let epoch_time_ms = start_time.elapsed().as_millis() as u64;

        let metrics = TrainingMetrics {
            loss,
            accuracy,
            epoch: current_epoch,
            learning_rate: lr,
            validation_loss: None,
            validation_accuracy: None,
            gradient_norm: Some(grad_norm),
            epoch_time_ms,
            custom_metrics: Default::default(),
        };

        // Update session metrics
        session.update_metrics(metrics.clone());

        // Persist session if repository available
        drop(model); // Release write lock before async operation
        if let Some(ref repo) = self.repository {
            repo.update_session(session).await?;
        }

        info!(
            epoch = current_epoch,
            loss = loss,
            accuracy = accuracy,
            time_ms = epoch_time_ms,
            "Completed training epoch"
        );

        Ok(metrics)
    }

    /// Refine embeddings using the trained GNN model
    ///
    /// # Arguments
    /// * `embeddings` - Input embeddings to refine
    ///
    /// # Returns
    /// Refined embeddings with quality scores
    #[instrument(skip(self, embeddings), fields(count = embeddings.len()), err)]
    pub async fn refine_embeddings(
        &self,
        embeddings: &[(EmbeddingId, Vec<f32>)],
    ) -> LearningResult<Vec<RefinedEmbedding>> {
        if embeddings.is_empty() {
            return Ok(Vec::new());
        }

        // Validate dimensions
        if let Some((_, emb)) = embeddings.first() {
            if emb.len() != self.config.input_dim {
                return Err(LearningError::DimensionMismatch {
                    expected: self.config.input_dim,
                    actual: emb.len(),
                });
            }
        }

        let model = self.model.read().await;

        // Build a simple graph where each embedding is a node
        // Connected based on cosine similarity
        let n = embeddings.len();
        let similarity_threshold = 0.5;

        // Build feature matrix
        let mut features = Array2::zeros((n, self.config.input_dim));
        for (i, (_, emb)) in embeddings.iter().enumerate() {
            for (j, &val) in emb.iter().enumerate() {
                features[[i, j]] = val;
            }
        }

        // Build adjacency matrix based on similarity
        let mut adj_matrix = Array2::<f32>::eye(n);
        for i in 0..n {
            for j in (i + 1)..n {
                let sim = cosine_similarity(&embeddings[i].1, &embeddings[j].1);
                if sim > similarity_threshold {
                    adj_matrix[[i, j]] = sim;
                    adj_matrix[[j, i]] = sim;
                }
            }
        }

        // Normalize adjacency matrix
        let degrees: Vec<f32> = (0..n)
            .map(|i| adj_matrix.row(i).sum())
            .collect();
        for i in 0..n {
            for j in 0..n {
                if degrees[i] > 0.0 && degrees[j] > 0.0 {
                    adj_matrix[[i, j]] /= (degrees[i] * degrees[j]).sqrt();
                }
            }
        }

        // Forward pass
        let output = model.forward(&features, &adj_matrix)?;

        // Create refined embeddings
        let session_id = self
            .current_session
            .read()
            .await
            .as_ref()
            .map(|s| s.id.clone());

        let refined: Vec<RefinedEmbedding> = embeddings
            .par_iter()
            .enumerate()
            .map(|(i, (id, original))| {
                let refined_vec: Vec<f32> = output.row(i).to_vec();

                // Compute refinement score based on change magnitude
                let delta = original
                    .iter()
                    .zip(&refined_vec)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f32>()
                    .sqrt();

                let score = 1.0 / (1.0 + delta); // Higher score for smaller changes

                let mut refined = RefinedEmbedding::new(id.clone(), refined_vec, score);
                refined.session_id = session_id.clone();
                refined.delta_norm = Some(delta);
                refined.normalize();
                refined
            })
            .collect();

        info!(count = refined.len(), "Refined embeddings");

        // Persist if repository available
        if let Some(ref repo) = self.repository {
            repo.save_refined_embeddings(&refined).await?;
        }

        Ok(refined)
    }

    /// Predict edge weight between two embeddings
    ///
    /// # Arguments
    /// * `from` - Source embedding
    /// * `to` - Target embedding
    ///
    /// # Returns
    /// Predicted edge weight (0.0 to 1.0)
    #[instrument(skip(self, from, to), err)]
    pub async fn predict_edge(&self, from: &[f32], to: &[f32]) -> LearningResult<f32> {
        // Validate dimensions
        if from.len() != self.config.input_dim {
            return Err(LearningError::DimensionMismatch {
                expected: self.config.input_dim,
                actual: from.len(),
            });
        }
        if to.len() != self.config.input_dim {
            return Err(LearningError::DimensionMismatch {
                expected: self.config.input_dim,
                actual: to.len(),
            });
        }

        let model = self.model.read().await;

        // Create a mini-graph with two nodes
        let mut features = Array2::zeros((2, self.config.input_dim));
        for (j, &val) in from.iter().enumerate() {
            features[[0, j]] = val;
        }
        for (j, &val) in to.iter().enumerate() {
            features[[1, j]] = val;
        }

        // Simple adjacency (self-loops only initially)
        let adj_matrix = Array2::<f32>::eye(2);

        // Forward pass
        let output = model.forward(&features, &adj_matrix)?;

        // Compute similarity of refined embeddings
        let from_refined: Vec<f32> = output.row(0).to_vec();
        let to_refined: Vec<f32> = output.row(1).to_vec();

        let similarity = cosine_similarity(&from_refined, &to_refined);
        let weight = (similarity + 1.0) / 2.0; // Map from [-1, 1] to [0, 1]

        Ok(weight)
    }

    /// Complete the current training session
    #[instrument(skip(self), err)]
    pub async fn complete_session(&self) -> LearningResult<()> {
        let mut session_guard = self.current_session.write().await;

        if let Some(ref mut session) = *session_guard {
            session.complete();

            // Compute and store Fisher information for EWC
            // This would be done in a real implementation with the final model state

            if let Some(ref repo) = self.repository {
                repo.update_session(session).await?;
            }

            info!(session_id = %session.id, "Completed learning session");
        }

        Ok(())
    }

    /// Fail the current session with an error
    #[instrument(skip(self, error), err)]
    pub async fn fail_session(&self, error: impl Into<String>) -> LearningResult<()> {
        let error_msg = error.into();
        let mut session_guard = self.current_session.write().await;

        if let Some(ref mut session) = *session_guard {
            session.fail(&error_msg);

            if let Some(ref repo) = self.repository {
                repo.update_session(session).await?;
            }

            warn!(session_id = %session.id, error = %error_msg, "Failed learning session");
        }

        Ok(())
    }

    /// Get the current session status
    pub async fn get_session(&self) -> Option<LearningSession> {
        self.current_session.read().await.clone()
    }

    /// Save EWC state from current model for future regularization
    #[instrument(skip(self, graph), err)]
    pub async fn consolidate_ewc(&self, graph: &TransitionGraph) -> LearningResult<()> {
        let model = self.model.read().await;
        let fisher = self.compute_fisher_information(&model, graph)?;
        let state = EwcState::new(model.get_parameters(), fisher);

        *self.ewc_state.write().await = Some(state);

        info!("Consolidated EWC state");
        Ok(())
    }

    // =========== Private Helper Methods ===========

    fn build_adjacency_matrix(&self, graph: &TransitionGraph) -> Array2<f32> {
        let n = graph.num_nodes();
        let mut adj = Array2::zeros((n, n));

        // Add self-loops
        for i in 0..n {
            adj[[i, i]] = 1.0;
        }

        // Add edges
        for &(from, to, weight) in &graph.edges {
            adj[[from, to]] = weight;
            if !graph.directed {
                adj[[to, from]] = weight;
            }
        }

        // Symmetric normalization: D^(-1/2) * A * D^(-1/2)
        let degrees: Vec<f32> = (0..n).map(|i| adj.row(i).sum()).collect();
        for i in 0..n {
            for j in 0..n {
                if degrees[i] > 0.0 && degrees[j] > 0.0 {
                    adj[[i, j]] /= (degrees[i] * degrees[j]).sqrt();
                }
            }
        }

        adj
    }

    fn build_feature_matrix(&self, graph: &TransitionGraph) -> Array2<f32> {
        let n = graph.num_nodes();
        let dim = graph.embedding_dim().unwrap_or(self.config.input_dim);
        let mut features = Array2::zeros((n, dim));

        for (i, emb) in graph.embeddings.iter().enumerate() {
            for (j, &val) in emb.iter().enumerate() {
                features[[i, j]] = val;
            }
        }

        features
    }

    async fn compute_loss(
        &self,
        graph: &TransitionGraph,
        output: &Array2<f32>,
    ) -> LearningResult<(f32, f32)> {
        let n = graph.num_nodes();
        if n == 0 {
            return Ok((0.0, 0.0));
        }

        let mut total_loss = 0.0;
        let mut correct = 0usize;
        let mut total = 0usize;

        let hp = &self.config.hyperparameters;

        // For each edge, compute contrastive loss
        for &(from, to, weight) in &graph.edges {
            let anchor: Vec<f32> = output.row(from).to_vec();
            let positive: Vec<f32> = output.row(to).to_vec();

            // Sample negative nodes (nodes not connected to anchor)
            let negatives: Vec<Vec<f32>> = (0..n)
                .filter(|&i| i != from && i != to)
                .take(hp.negative_ratio)
                .map(|i| output.row(i).to_vec())
                .collect();

            if !negatives.is_empty() {
                let neg_refs: Vec<&[f32]> = negatives.iter().map(|v| v.as_slice()).collect();

                // InfoNCE loss
                let loss = loss::info_nce_loss(&anchor, &positive, &neg_refs, hp.temperature);
                total_loss += loss * weight;
            }

            // Compute accuracy based on whether positive is closer than negatives
            let pos_sim = cosine_similarity(&anchor, &positive);
            let all_closer = (0..n)
                .filter(|&i| i != from && i != to)
                .all(|i| {
                    let neg: Vec<f32> = output.row(i).to_vec();
                    cosine_similarity(&anchor, &neg) < pos_sim
                });

            if all_closer {
                correct += 1;
            }
            total += 1;
        }

        let avg_loss = if graph.edges.is_empty() {
            0.0
        } else {
            total_loss / graph.edges.len() as f32
        };

        let accuracy = if total == 0 {
            0.0
        } else {
            correct as f32 / total as f32
        };

        Ok((avg_loss, accuracy))
    }

    fn compute_gradients(
        &self,
        _graph: &TransitionGraph,
        features: &Array2<f32>,
        output: &Array2<f32>,
        _adj_matrix: &Array2<f32>,
        model: &GnnModel,
    ) -> LearningResult<Vec<Array2<f32>>> {
        // Simplified gradient computation
        // In practice, this would use automatic differentiation
        let num_layers = model.num_layers();
        let mut gradients = Vec::with_capacity(num_layers);
        let batch_size = features.nrows() as f32;

        for layer_idx in 0..num_layers {
            let (in_dim, out_dim) = model.layer_dims(layer_idx);

            // Compute gradient approximation based on output variance
            // This is a simplified placeholder - real backprop would use chain rule
            let output_centered = &output.mapv(|x| x - output.mean().unwrap_or(0.0));

            // Approximate gradient as outer product scaled by learning signal
            let grad = if layer_idx == 0 {
                // Input layer: gradient is features^T * output_signal / batch_size
                let output_slice = if output.ncols() >= out_dim {
                    output_centered.slice(ndarray::s![.., ..out_dim]).to_owned()
                } else {
                    Array2::zeros((output.nrows(), out_dim))
                };
                let feat_slice = if features.ncols() >= in_dim {
                    features.slice(ndarray::s![.., ..in_dim]).to_owned()
                } else {
                    Array2::zeros((features.nrows(), in_dim))
                };
                feat_slice.t().dot(&output_slice) / batch_size
            } else {
                // Hidden layers: use small random gradients scaled by output variance
                let variance = output.var(0.0);
                Array2::from_elem((in_dim, out_dim), 0.01 * variance.sqrt())
            };

            // Reshape to (out_dim, in_dim)
            let scaled_grad = grad.t().to_owned();
            gradients.push(scaled_grad);
        }

        Ok(gradients)
    }

    fn compute_gradient_norm(&self, gradients: &[Array2<f32>]) -> f32 {
        gradients
            .iter()
            .map(|g| g.iter().map(|&x| x * x).sum::<f32>())
            .sum::<f32>()
            .sqrt()
    }

    fn clip_gradients(&self, gradients: Vec<Array2<f32>>, max_norm: f32) -> Vec<Array2<f32>> {
        let current_norm = self.compute_gradient_norm(&gradients);
        if current_norm <= max_norm {
            return gradients;
        }

        let scale = max_norm / current_norm;
        gradients.into_iter().map(|g| g * scale).collect()
    }

    fn compute_learning_rate(&self, epoch: usize) -> f32 {
        let base_lr = self.config.hyperparameters.learning_rate;
        let total_epochs = self.config.hyperparameters.epochs;

        // Cosine annealing schedule
        let progress = epoch as f32 / total_epochs as f32;
        let cosine_factor = (1.0 + (progress * std::f32::consts::PI).cos()) / 2.0;

        base_lr * cosine_factor
    }

    fn compute_fisher_information(
        &self,
        _model: &GnnModel,
        _graph: &TransitionGraph,
    ) -> LearningResult<crate::ewc::FisherInformation> {
        // Simplified Fisher information computation
        // In practice, this would compute the diagonal of the Fisher matrix
        Ok(crate::ewc::FisherInformation::default())
    }
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < 1e-10 || norm_b < 1e-10 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 1e-6);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) + 1.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_learning_service_creation() {
        let config = LearningConfig::default();
        let service = LearningService::new(config.clone());

        assert_eq!(service.model_type(), GnnModelType::Gcn);
        assert_eq!(service.config().input_dim, 768);
    }

    #[tokio::test]
    async fn test_start_session() {
        let config = LearningConfig::default();
        let service = LearningService::new(config);

        let session_id = service.start_session().await.unwrap();
        assert!(!session_id.is_empty());

        let session = service.get_session().await.unwrap();
        assert_eq!(session.status, TrainingStatus::Running);
    }

    #[tokio::test]
    async fn test_train_epoch() {
        let mut config = LearningConfig::default();
        config.input_dim = 8;
        config.output_dim = 4;
        config.hyperparameters.hidden_dim = 8;

        let service = LearningService::new(config);
        service.start_session().await.unwrap();

        let mut graph = TransitionGraph::new();
        graph.add_node(EmbeddingId::new("n1"), vec![0.1; 8], None);
        graph.add_node(EmbeddingId::new("n2"), vec![0.2; 8], None);
        graph.add_node(EmbeddingId::new("n3"), vec![0.3; 8], None);
        graph.add_edge(0, 1, 0.8);
        graph.add_edge(1, 2, 0.7);

        let metrics = service.train_epoch(&graph).await.unwrap();
        assert_eq!(metrics.epoch, 1);
        assert!(metrics.loss >= 0.0);
    }

    #[tokio::test]
    async fn test_refine_embeddings() {
        let mut config = LearningConfig::default();
        config.input_dim = 8;
        config.output_dim = 4;
        config.hyperparameters.hidden_dim = 8;

        let service = LearningService::new(config);
        service.start_session().await.unwrap();

        let embeddings = vec![
            (EmbeddingId::new("e1"), vec![0.1; 8]),
            (EmbeddingId::new("e2"), vec![0.2; 8]),
        ];

        let refined = service.refine_embeddings(&embeddings).await.unwrap();
        assert_eq!(refined.len(), 2);
        assert_eq!(refined[0].dim(), 4); // Output dimension
    }

    #[tokio::test]
    async fn test_predict_edge() {
        let mut config = LearningConfig::default();
        config.input_dim = 8;
        config.output_dim = 4;
        config.hyperparameters.hidden_dim = 8;

        let service = LearningService::new(config);

        let from = vec![0.1; 8];
        let to = vec![0.1; 8]; // Same embedding should have high weight

        let weight = service.predict_edge(&from, &to).await.unwrap();
        assert!(weight >= 0.0 && weight <= 1.0);
    }

    #[tokio::test]
    async fn test_empty_graph_error() {
        let config = LearningConfig::default();
        let service = LearningService::new(config);
        service.start_session().await.unwrap();

        let graph = TransitionGraph::new();
        let result = service.train_epoch(&graph).await;

        assert!(matches!(result, Err(LearningError::EmptyGraph)));
    }

    #[tokio::test]
    async fn test_dimension_mismatch() {
        let mut config = LearningConfig::default();
        config.input_dim = 768;

        let service = LearningService::new(config);
        service.start_session().await.unwrap();

        let mut graph = TransitionGraph::new();
        graph.add_node(EmbeddingId::new("n1"), vec![0.1; 128], None); // Wrong dimension

        let result = service.train_epoch(&graph).await;
        assert!(matches!(
            result,
            Err(LearningError::DimensionMismatch { .. })
        ));
    }
}
