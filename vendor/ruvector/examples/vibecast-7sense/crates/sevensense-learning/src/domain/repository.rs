//! Repository traits for the learning domain.
//!
//! Defines the persistence abstraction for learning sessions,
//! refined embeddings, and transition graphs.

use async_trait::async_trait;
use std::sync::Arc;

use super::entities::{
    EmbeddingId, LearningSession, RefinedEmbedding, TrainingStatus, TransitionGraph,
};

/// Error type for repository operations
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    /// Session not found
    #[error("Learning session not found: {0}")]
    SessionNotFound(String),

    /// Embedding not found
    #[error("Embedding not found: {0}")]
    EmbeddingNotFound(String),

    /// Graph not found or empty
    #[error("Transition graph not found")]
    GraphNotFound,

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Validation error
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Concurrent modification error
    #[error("Concurrent modification detected for: {0}")]
    ConcurrentModification(String),

    /// Internal error
    #[error("Internal repository error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for RepositoryError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializationError(e.to_string())
    }
}

/// Result type for repository operations
pub type RepositoryResult<T> = Result<T, RepositoryError>;

/// Repository trait for learning persistence operations.
///
/// Implementors should provide durable storage for:
/// - Learning sessions and their state
/// - Refined embeddings
/// - Transition graphs
#[async_trait]
pub trait LearningRepository: Send + Sync {
    // =========== Session Operations ===========

    /// Save a learning session
    async fn save_session(&self, session: &LearningSession) -> RepositoryResult<()>;

    /// Get a learning session by ID
    async fn get_session(&self, id: &str) -> RepositoryResult<Option<LearningSession>>;

    /// Update an existing session
    async fn update_session(&self, session: &LearningSession) -> RepositoryResult<()>;

    /// Delete a session
    async fn delete_session(&self, id: &str) -> RepositoryResult<()>;

    /// List sessions with optional status filter
    async fn list_sessions(
        &self,
        status: Option<TrainingStatus>,
        limit: Option<usize>,
    ) -> RepositoryResult<Vec<LearningSession>>;

    // =========== Embedding Operations ===========

    /// Save refined embeddings (batch)
    async fn save_refined_embeddings(
        &self,
        embeddings: &[RefinedEmbedding],
    ) -> RepositoryResult<()>;

    /// Get a refined embedding by original ID
    async fn get_refined_embedding(
        &self,
        original_id: &EmbeddingId,
    ) -> RepositoryResult<Option<RefinedEmbedding>>;

    /// Get multiple refined embeddings
    async fn get_refined_embeddings(
        &self,
        ids: &[EmbeddingId],
    ) -> RepositoryResult<Vec<RefinedEmbedding>>;

    /// Delete refined embeddings for a session
    async fn delete_refined_embeddings(&self, session_id: &str) -> RepositoryResult<usize>;

    // =========== Graph Operations ===========

    /// Get the current transition graph
    async fn get_transition_graph(&self) -> RepositoryResult<TransitionGraph>;

    /// Save a transition graph
    async fn save_transition_graph(&self, graph: &TransitionGraph) -> RepositoryResult<()>;

    /// Update the transition graph (incremental)
    async fn update_transition_graph(&self, graph: &TransitionGraph) -> RepositoryResult<()>;

    /// Clear the transition graph
    async fn clear_transition_graph(&self) -> RepositoryResult<()>;

    // =========== Checkpoint Operations ===========

    /// Save a model checkpoint
    async fn save_checkpoint(
        &self,
        session_id: &str,
        epoch: usize,
        data: &[u8],
    ) -> RepositoryResult<String>;

    /// Load a model checkpoint
    async fn load_checkpoint(
        &self,
        session_id: &str,
        epoch: Option<usize>,
    ) -> RepositoryResult<Option<Vec<u8>>>;

    /// List available checkpoints for a session
    async fn list_checkpoints(&self, session_id: &str) -> RepositoryResult<Vec<(usize, String)>>;

    /// Delete checkpoints for a session
    async fn delete_checkpoints(&self, session_id: &str) -> RepositoryResult<usize>;
}

/// Extension trait for repository operations
#[async_trait]
pub trait LearningRepositoryExt: LearningRepository {
    /// Get the latest session for a model type
    async fn get_latest_session(
        &self,
        model_type: crate::GnnModelType,
    ) -> RepositoryResult<Option<LearningSession>> {
        let sessions = self.list_sessions(None, Some(100)).await?;
        Ok(sessions
            .into_iter()
            .filter(|s| s.model_type == model_type)
            .max_by_key(|s| s.started_at))
    }

    /// Get all completed sessions
    async fn get_completed_sessions(&self) -> RepositoryResult<Vec<LearningSession>> {
        self.list_sessions(Some(TrainingStatus::Completed), None).await
    }

    /// Check if any session is currently running
    async fn has_running_session(&self) -> RepositoryResult<bool> {
        let sessions = self.list_sessions(Some(TrainingStatus::Running), Some(1)).await?;
        Ok(!sessions.is_empty())
    }

    /// Get embeddings refined in a specific session
    async fn get_session_embeddings(
        &self,
        session_id: &str,
    ) -> RepositoryResult<Vec<RefinedEmbedding>> {
        // Default implementation - may be overridden for efficiency
        let session = self.get_session(session_id).await?;
        if session.is_none() {
            return Err(RepositoryError::SessionNotFound(session_id.to_string()));
        }

        // This would need to be implemented properly in concrete implementations
        Ok(Vec::new())
    }
}

// Blanket implementation
impl<T: LearningRepository + ?Sized> LearningRepositoryExt for T {}

/// A thread-safe repository handle
pub type DynLearningRepository = Arc<dyn LearningRepository>;

/// Unit of work pattern for transactional operations
#[async_trait]
pub trait UnitOfWork: Send + Sync {
    /// Begin a transaction
    async fn begin(&self) -> RepositoryResult<()>;

    /// Commit the transaction
    async fn commit(&self) -> RepositoryResult<()>;

    /// Rollback the transaction
    async fn rollback(&self) -> RepositoryResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    /// In-memory implementation for testing
    struct InMemoryRepository {
        sessions: RwLock<HashMap<String, LearningSession>>,
        embeddings: RwLock<HashMap<String, RefinedEmbedding>>,
        graph: RwLock<Option<TransitionGraph>>,
        checkpoints: RwLock<HashMap<String, Vec<(usize, Vec<u8>)>>>,
    }

    impl InMemoryRepository {
        fn new() -> Self {
            Self {
                sessions: RwLock::new(HashMap::new()),
                embeddings: RwLock::new(HashMap::new()),
                graph: RwLock::new(None),
                checkpoints: RwLock::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl LearningRepository for InMemoryRepository {
        async fn save_session(&self, session: &LearningSession) -> RepositoryResult<()> {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session.id.clone(), session.clone());
            Ok(())
        }

        async fn get_session(&self, id: &str) -> RepositoryResult<Option<LearningSession>> {
            let sessions = self.sessions.read().await;
            Ok(sessions.get(id).cloned())
        }

        async fn update_session(&self, session: &LearningSession) -> RepositoryResult<()> {
            self.save_session(session).await
        }

        async fn delete_session(&self, id: &str) -> RepositoryResult<()> {
            let mut sessions = self.sessions.write().await;
            sessions.remove(id);
            Ok(())
        }

        async fn list_sessions(
            &self,
            status: Option<TrainingStatus>,
            limit: Option<usize>,
        ) -> RepositoryResult<Vec<LearningSession>> {
            let sessions = self.sessions.read().await;
            let mut result: Vec<_> = sessions
                .values()
                .filter(|s| status.map_or(true, |st| s.status == st))
                .cloned()
                .collect();
            result.sort_by(|a, b| b.started_at.cmp(&a.started_at));
            if let Some(limit) = limit {
                result.truncate(limit);
            }
            Ok(result)
        }

        async fn save_refined_embeddings(
            &self,
            embeddings: &[RefinedEmbedding],
        ) -> RepositoryResult<()> {
            let mut store = self.embeddings.write().await;
            for emb in embeddings {
                store.insert(emb.original_id.0.clone(), emb.clone());
            }
            Ok(())
        }

        async fn get_refined_embedding(
            &self,
            original_id: &EmbeddingId,
        ) -> RepositoryResult<Option<RefinedEmbedding>> {
            let store = self.embeddings.read().await;
            Ok(store.get(&original_id.0).cloned())
        }

        async fn get_refined_embeddings(
            &self,
            ids: &[EmbeddingId],
        ) -> RepositoryResult<Vec<RefinedEmbedding>> {
            let store = self.embeddings.read().await;
            Ok(ids
                .iter()
                .filter_map(|id| store.get(&id.0).cloned())
                .collect())
        }

        async fn delete_refined_embeddings(&self, _session_id: &str) -> RepositoryResult<usize> {
            let mut store = self.embeddings.write().await;
            let count = store.len();
            store.clear();
            Ok(count)
        }

        async fn get_transition_graph(&self) -> RepositoryResult<TransitionGraph> {
            let graph = self.graph.read().await;
            graph.clone().ok_or(RepositoryError::GraphNotFound)
        }

        async fn save_transition_graph(&self, graph: &TransitionGraph) -> RepositoryResult<()> {
            let mut store = self.graph.write().await;
            *store = Some(graph.clone());
            Ok(())
        }

        async fn update_transition_graph(&self, graph: &TransitionGraph) -> RepositoryResult<()> {
            self.save_transition_graph(graph).await
        }

        async fn clear_transition_graph(&self) -> RepositoryResult<()> {
            let mut store = self.graph.write().await;
            *store = None;
            Ok(())
        }

        async fn save_checkpoint(
            &self,
            session_id: &str,
            epoch: usize,
            data: &[u8],
        ) -> RepositoryResult<String> {
            let mut store = self.checkpoints.write().await;
            let checkpoints = store.entry(session_id.to_string()).or_default();
            checkpoints.push((epoch, data.to_vec()));
            Ok(format!("{session_id}-{epoch}"))
        }

        async fn load_checkpoint(
            &self,
            session_id: &str,
            epoch: Option<usize>,
        ) -> RepositoryResult<Option<Vec<u8>>> {
            let store = self.checkpoints.read().await;
            if let Some(checkpoints) = store.get(session_id) {
                if let Some(epoch) = epoch {
                    return Ok(checkpoints
                        .iter()
                        .find(|(e, _)| *e == epoch)
                        .map(|(_, d)| d.clone()));
                }
                return Ok(checkpoints.last().map(|(_, d)| d.clone()));
            }
            Ok(None)
        }

        async fn list_checkpoints(
            &self,
            session_id: &str,
        ) -> RepositoryResult<Vec<(usize, String)>> {
            let store = self.checkpoints.read().await;
            if let Some(checkpoints) = store.get(session_id) {
                return Ok(checkpoints
                    .iter()
                    .map(|(e, _)| (*e, format!("{session_id}-{e}")))
                    .collect());
            }
            Ok(Vec::new())
        }

        async fn delete_checkpoints(&self, session_id: &str) -> RepositoryResult<usize> {
            let mut store = self.checkpoints.write().await;
            if let Some(checkpoints) = store.remove(session_id) {
                return Ok(checkpoints.len());
            }
            Ok(0)
        }
    }

    #[tokio::test]
    async fn test_in_memory_repository() {
        let repo = InMemoryRepository::new();
        let config = crate::LearningConfig::default();
        let session = crate::LearningSession::new(config);

        // Save and retrieve session
        repo.save_session(&session).await.unwrap();
        let retrieved = repo.get_session(&session.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, session.id);

        // List sessions
        let sessions = repo.list_sessions(None, None).await.unwrap();
        assert_eq!(sessions.len(), 1);

        // Delete session
        repo.delete_session(&session.id).await.unwrap();
        let retrieved = repo.get_session(&session.id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_transition_graph_operations() {
        let repo = InMemoryRepository::new();

        // Graph should not exist initially
        assert!(repo.get_transition_graph().await.is_err());

        // Save graph
        let mut graph = TransitionGraph::new();
        graph.add_node(
            crate::EmbeddingId::new("n1"),
            vec![0.1, 0.2, 0.3],
            None,
        );
        repo.save_transition_graph(&graph).await.unwrap();

        // Retrieve graph
        let retrieved = repo.get_transition_graph().await.unwrap();
        assert_eq!(retrieved.num_nodes(), 1);

        // Clear graph
        repo.clear_transition_graph().await.unwrap();
        assert!(repo.get_transition_graph().await.is_err());
    }

    #[tokio::test]
    async fn test_checkpoint_operations() {
        let repo = InMemoryRepository::new();
        let session_id = "test-session";

        // Save checkpoints
        repo.save_checkpoint(session_id, 1, b"data1").await.unwrap();
        repo.save_checkpoint(session_id, 2, b"data2").await.unwrap();

        // List checkpoints
        let checkpoints = repo.list_checkpoints(session_id).await.unwrap();
        assert_eq!(checkpoints.len(), 2);

        // Load specific checkpoint
        let data = repo.load_checkpoint(session_id, Some(1)).await.unwrap();
        assert_eq!(data, Some(b"data1".to_vec()));

        // Load latest checkpoint
        let data = repo.load_checkpoint(session_id, None).await.unwrap();
        assert_eq!(data, Some(b"data2".to_vec()));

        // Delete checkpoints
        let count = repo.delete_checkpoints(session_id).await.unwrap();
        assert_eq!(count, 2);
    }
}
